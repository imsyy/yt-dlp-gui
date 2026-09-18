//! 操作系统级进程控制模块（终止进程树）

use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 长任务子进程的 pid 登记表（run_id -> pid）。
///
/// `output().await` 会阻塞到进程退出、拿不到 pid，进程一旦卡住就无法终止；
/// 因此长任务改为 spawn 并把 pid 记在这里，取消时才能定位并杀掉整棵进程树。
#[derive(Default)]
pub struct ProcessRegistry {
    processes: Mutex<HashMap<String, u32>>,
}

impl ProcessRegistry {
    pub fn register(&self, run_id: &str, pid: u32) {
        if let Ok(mut processes) = self.processes.lock() {
            processes.insert(run_id.to_owned(), pid);
        }
    }

    /// 取走并移除登记项，避免同一 run_id 被重复终止。
    pub fn take(&self, run_id: &str) -> Option<u32> {
        self.processes.lock().ok()?.remove(run_id)
    }

    /// 取出并清空全部登记项，用于应用退出时回收仍在运行的子进程。
    pub fn drain(&self) -> Vec<u32> {
        let Ok(mut processes) = self.processes.lock() else {
            return Vec::new();
        };
        processes.drain().map(|(_, pid)| pid).collect()
    }
}

/// 终止指定 PID 的进程及其子进程
#[cfg(target_os = "windows")]
pub fn kill_process(pid: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("err_kill_process:{}", error))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn kill_process(pid: u32) -> Result<(), String> {
    // yt-dlp 会按需拉起 ffmpeg 子进程，直接 kill 父进程会留下孤儿。
    // 先用 pkill 按父 PID 杀一遍子进程树（best-effort，子进程可能已退出），再杀父进程。
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-P", &pid.to_string()])
        .output();
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|error| format!("err_kill_process:{}", error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_take_is_single_use() {
        let registry = ProcessRegistry::default();
        registry.register("chapters_run_1", 4242);
        assert_eq!(registry.take("chapters_run_1"), Some(4242));
        // take 语义是取走：重复取消不得再拿到同一个 pid，否则 pid 被系统复用后会误杀无关进程
        assert_eq!(registry.take("chapters_run_1"), None);
    }

    #[test]
    fn test_registry_keeps_runs_isolated() {
        let registry = ProcessRegistry::default();
        registry.register("chapters_run_1", 100);
        registry.register("comments_run_1", 200);
        assert_eq!(registry.take("comments_run_1"), Some(200));
        // 取走一个 run 不影响另一个
        assert_eq!(registry.take("chapters_run_1"), Some(100));
        assert_eq!(registry.take("livechat_run_1"), None);
    }

    #[test]
    fn test_registry_drain_returns_all_and_clears() {
        let registry = ProcessRegistry::default();
        registry.register("chapters_run_1", 100);
        registry.register("livechat_run_1", 200);

        let mut drained = registry.drain();
        drained.sort_unstable();
        assert_eq!(drained, vec![100, 200]);
        // 清空后再次调用不应重复返回，避免退出路径被重复触发时误杀复用后的 pid
        assert!(registry.drain().is_empty());
    }

    /// 启动一个会持续运行的替身进程（约 60 秒），用于验证真实终止能力。
    fn spawn_long_running() -> std::process::Child {
        #[cfg(target_os = "windows")]
        let child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 60 127.0.0.1"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        #[cfg(not(target_os = "windows"))]
        let child = std::process::Command::new("sleep")
            .arg("60")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        child.expect("无法启动替身进程")
    }

    fn wait_for_exit(child: &mut std::process::Child, limit: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + limit;
        while std::time::Instant::now() < deadline {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        false
    }

    /// 用真实进程验证 kill_process：这是取消链路最后一环，只靠编译通过不足以确认它能真正终止进程。
    #[test]
    fn test_kill_process_terminates_real_process() {
        let mut child = spawn_long_running();
        let pid = child.id();
        // 先确认替身进程确实还活着，否则这个断言会在空转中通过
        assert!(
            matches!(child.try_wait(), Ok(None)),
            "替身进程启动后立刻退出了，测试无效"
        );

        kill_process(pid).expect("kill_process 调用失败");
        assert!(
            wait_for_exit(&mut child, std::time::Duration::from_secs(10)),
            "kill_process 之后进程仍在运行"
        );
    }
}
