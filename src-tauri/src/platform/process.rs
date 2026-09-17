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
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|error| format!("err_kill_process:{}", error))?;
    Ok(())
}
