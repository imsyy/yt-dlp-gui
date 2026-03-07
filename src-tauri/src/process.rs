//! 操作系统级进程控制模块（挂起/恢复/终止）

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// ========== Windows 进程控制 ==========

#[cfg(target_os = "windows")]
mod win32 {
    #[repr(C)]
    pub struct THREADENTRY32 {
        pub dw_size: u32,
        pub cnt_usage: u32,
        pub th32_thread_id: u32,
        pub th32_owner_process_id: u32,
        pub tp_base_pri: i32,
        pub tp_delta_pri: i32,
        pub dw_flags: u32,
    }

    pub const TH32CS_SNAPTHREAD: u32 = 0x00000004;
    pub const THREAD_SUSPEND_RESUME: u32 = 0x0002;

    extern "system" {
        pub fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> isize;
        pub fn Thread32First(h_snapshot: isize, lpte: *mut THREADENTRY32) -> i32;
        pub fn Thread32Next(h_snapshot: isize, lpte: *mut THREADENTRY32) -> i32;
        pub fn OpenThread(
            dw_desired_access: u32,
            b_inherit_handle: i32,
            dw_thread_id: u32,
        ) -> isize;
        pub fn SuspendThread(h_thread: isize) -> u32;
        pub fn ResumeThread(h_thread: isize) -> u32;
        pub fn CloseHandle(h_object: isize) -> i32;
    }
}

/// 挂起指定 PID 的进程（暂停所有线程）
#[cfg(target_os = "windows")]
pub fn suspend_process(pid: u32) -> Result<(), String> {
    unsafe {
        use win32::*;
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == -1 {
            return Err("创建线程快照失败".into());
        }
        let mut entry = std::mem::zeroed::<THREADENTRY32>();
        entry.dw_size = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32_owner_process_id == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32_thread_id);
                    if thread != 0 {
                        SuspendThread(thread);
                        CloseHandle(thread);
                    }
                }
                if Thread32Next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn suspend_process(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .args(["-STOP", &pid.to_string()])
        .output()
        .map_err(|e| format!("挂起进程失败: {}", e))?;
    Ok(())
}

/// 恢复指定 PID 的进程（恢复所有线程）
#[cfg(target_os = "windows")]
pub fn resume_process(pid: u32) -> Result<(), String> {
    unsafe {
        use win32::*;
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == -1 {
            return Err("创建线程快照失败".into());
        }
        let mut entry = std::mem::zeroed::<THREADENTRY32>();
        entry.dw_size = std::mem::size_of::<THREADENTRY32>() as u32;
        if Thread32First(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32_owner_process_id == pid {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32_thread_id);
                    if thread != 0 {
                        ResumeThread(thread);
                        CloseHandle(thread);
                    }
                }
                if Thread32Next(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn resume_process(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .args(["-CONT", &pid.to_string()])
        .output()
        .map_err(|e| format!("恢复进程失败: {}", e))?;
    Ok(())
}

/// 终止指定 PID 的进程及其子进程
#[cfg(target_os = "windows")]
pub fn kill_process(pid: u32) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("终止进程失败: {}", e))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn kill_process(pid: u32) -> Result<(), String> {
    std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .map_err(|e| format!("终止进程失败: {}", e))?;
    Ok(())
}
