//! 操作系统级进程控制模块（终止进程树）

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
