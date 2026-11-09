//! Самоповышение прав через UAC.

/// Флаг, который получает перезапущенный процесс: окно консоли новое, его надо задержать в конце.
pub const ELEVATED_FLAG: &str = "--elevated";

#[cfg(windows)]
pub fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_admin() -> bool { true }

/// Перезапуск себя с запросом UAC. `Ok(())` — новый процесс запущен, текущий должен выйти.
#[cfg(windows)]
pub fn relaunch_as_admin(args: &[String]) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    fn wide(s: &str) -> Vec<u16> { s.encode_utf16().chain([0]).collect() }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let params: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).chain([ELEVATED_FLAG.into()]).collect();
    let (verb, file, params) = (wide("runas"), wide(&exe.to_string_lossy()), wide(&params.join(" ")));
    let r = unsafe {
        ShellExecuteW(std::ptr::null_mut(), verb.as_ptr(), file.as_ptr(), params.as_ptr(), std::ptr::null(), SW_SHOWNORMAL)
    };
    // ShellExecuteW: значение > 32 — успех; иначе отказ в UAC или ошибка
    if r as isize > 32 { Ok(()) } else { Err(t!("не удалось получить права администратора (отказ в UAC?)").into()) }
}

#[cfg(not(windows))]
pub fn relaunch_as_admin(_: &[String]) -> Result<(), String> { Ok(()) }
