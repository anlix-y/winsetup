/// Скачать файл целиком в память.
// shortcut: встроенный curl.exe (есть в Win10 1803+) вместо HTTP-библиотеки; заменить на ureq, если понадобится прогресс/прокси
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-fsSL", "--retry", "2", url])
        .output()
        .map_err(|e| t!("не удалось запустить curl: {e}", e = e))?;
    if !out.status.success() {
        return Err(t!("не удалось скачать {url}: {err}", url = url, err = String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

pub fn is_url(s: &str) -> bool { s.starts_with("http://") || s.starts_with("https://") }
