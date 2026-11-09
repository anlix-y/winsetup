//! Софт: winget-пакеты и прямые exe-установщики.

use crate::config::Software;
use crate::step::Step;
// SHA256 отключён: use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::process::Command;

pub struct WingetStep<'a>(pub &'a Software);

pub struct InstallerStep<'a> {
    pub sw: &'a Software,
    reboot: Cell<bool>,
    // SHA256 отключён: hash_failed: Cell<bool>,
    // SHA256 отключён: ignore_hash: Cell<bool>,
}

impl<'a> InstallerStep<'a> {
    pub fn new(sw: &'a Software) -> Self {
        // SHA256 отключён: Self { sw, reboot: Cell::new(false), hash_failed: Cell::new(false), ignore_hash: Cell::new(false) }
        Self { sw, reboot: Cell::new(false) }
    }
}

/// Один из двух шагов — в зависимости от того, winget или url (проверено в config::validate).
pub fn step(sw: &Software) -> Box<dyn Step + '_> {
    if sw.winget.is_some() { Box::new(WingetStep(sw)) } else { Box::new(InstallerStep::new(sw)) }
}

fn run(cmd: &mut Command) -> Result<(i32, String), String> {
    let out = cmd.output().map_err(|e| t!("не удалось запустить: {e}", e = e))?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    Ok((out.status.code().unwrap_or(-1), text))
}

impl Step for WingetStep<'_> {
    fn describe(&self) -> String {
        let id = self.0.winget.as_deref().unwrap_or_default();
        match &self.0.version {
            Some(v) => t!("Софт (winget): {id} {v}", id = id, v = v),
            None => t!("Софт (winget): {id}", id = id),
        }
    }

    fn check(&self) -> Result<bool, String> {
        let id = self.0.winget.as_deref().unwrap_or_default();
        let (code, out) = run(Command::new("winget").args([
            "list", "--id", id, "--exact", "--source", "winget", "--accept-source-agreements", "--disable-interactivity",
        ]))?;
        // --source winget: msstore часто сломан (урезанные сборки, нет Store)
        // winget list возвращает не 0, если пакет не найден
        // shortcut: версия ищется подстрокой в выводе; разбирать таблицу, если будут ложные совпадения
        Ok(code == 0 && self.0.version.as_deref().is_none_or(|v| out.contains(v)))
    }

    fn apply(&self) -> Result<(), String> {
        let id = self.0.winget.as_deref().unwrap_or_default();
        let mut cmd = Command::new("winget");
        cmd.args([
            "install", "--id", id, "--exact", "--source", "winget", "--silent",
            "--accept-package-agreements", "--accept-source-agreements", "--disable-interactivity",
        ]);
        if let Some(v) = &self.0.version { cmd.args(["--version", v]); }
        // вывод winget не перехватываем — пользователь видит его прогресс-бар
        let status = cmd.stdin(std::process::Stdio::null()).status().map_err(|e| t!("не удалось запустить winget: {e}", e = e))?;
        match status.code() {
            Some(0) => Ok(()),
            Some(c) => Err(t!("winget код {code}: {text}", code = format!("{:#x}", c as u32), text = winget_error_text(c))),
            None => Err(t!("winget прерван").into()),
        }
    }
}

impl Step for InstallerStep<'_> {
    fn describe(&self) -> String {
        let url = self.sw.url.as_deref().unwrap_or_default();
        let name = self.sw.name.as_deref().unwrap_or(url);
        match &self.sw.extract_to {
            Some(d) => t!("Софт (архив): {name} → {d}", name = name, d = d),
            None => t!("Софт (установщик): {name}", name = name),
        }
    }

    fn check(&self) -> Result<bool, String> {
        Ok(self.sw.check_path.as_deref().is_some_and(|p| std::path::Path::new(p).exists()))
    }

    fn apply(&self) -> Result<(), String> {
        let url = self.sw.url.as_deref().unwrap_or_default();
        crate::step::note(&t!("скачиваю {url}", url = url));
        let data = crate::net::fetch(url)?;
        // SHA256 отключён: self.hash_failed.set(false);
        // SHA256 отключён: if let Some(want) = &self.sw.sha256 {
            // SHA256 отключён: if self.ignore_hash.get() {
                // SHA256 отключён: println!("        ⚠ SHA256 не проверяется по решению пользователя");
            // SHA256 отключён: } else if let Err(e) = verify_sha256(&data, want) {
                // SHA256 отключён: self.hash_failed.set(true);
                // SHA256 отключён: return Err(e);
            // SHA256 отключён: }
        // SHA256 отключён: }
        let dir = std::env::temp_dir().join("winsetup");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        if let Some(dest) = &self.sw.extract_to {
            // архив (zapret и т.п.): распаковать, ничего не запуская
            let zip = dir.join("download.zip");
            std::fs::write(&zip, &data).map_err(|e| e.to_string())?;
            let r = super::ps(&format!("Expand-Archive -LiteralPath {} -DestinationPath {} -Force",
                super::q(&zip.display().to_string()), super::q(dest)));
            std::fs::remove_file(&zip).ok();
            return r.map(|_| ());
        }
        let file = dir.join(file_name(url));
        std::fs::write(&file, &data).map_err(|e| e.to_string())?;

        let mut cmd = Command::new(&file);
        #[cfg(windows)]
        if let Some(a) = &self.sw.args {
            // ключи установщика передаём как есть, без разбиения (кавычки внутри сохраняются)
            std::os::windows::process::CommandExt::raw_arg(&mut cmd, a);
        }
        let status = cmd.status().map_err(|e| t!("не удалось запустить установщик: {e}", e = e));
        std::fs::remove_file(&file).ok();
        match status?.code() {
            Some(0) => Ok(()),
            Some(3010) => { self.reboot.set(true); Ok(()) } // 3010 = успех, нужна перезагрузка
            c => Err(t!("установщик завершился с кодом {code}", code = format!("{c:?}"))),
        }
    }

    fn needs_reboot(&self) -> bool { self.reboot.get() }
    // SHA256 отключён: fn can_force(&self) -> bool { self.hash_failed.get() }
    // SHA256 отключён: fn set_force(&self) { self.ignore_hash.set(true) }
}

/// Описание кода ошибки от самого winget (`winget error`, есть в winget 1.7+).
fn winget_error_text(code: i32) -> String {
    match run(Command::new("winget").args(["error", "--input", &format!("{:#x}", code as u32)])) {
        Ok((0, out)) if !out.trim().is_empty() => last_line(&out).to_string(),
        _ => t!("описание недоступно").into(),
    }
}

fn last_line(s: &str) -> &str {
    s.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("")
}

// SHA256 отключён: pub fn verify_sha256(data: &[u8], want: &str) -> Result<(), String> {
    // SHA256 отключён: let got: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
    // SHA256 отключён: if got.eq_ignore_ascii_case(want) { Ok(()) } else { Err(format!("SHA256 не совпадает: ожидался {want}, получен {got}")) }
// SHA256 отключён: }

/// Имя файла из URL; без расширения .exe Windows не запустит файл.
fn file_name(url: &str) -> String {
    let name = url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or("");
    if name.to_ascii_lowercase().ends_with(".exe") { name.to_string() } else { "installer.exe".into() }
}

pub fn winget_available() -> bool {
    Command::new("winget").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// Официальный способ Microsoft: модуль Microsoft.WinGet.Client ставит winget со всеми зависимостями.
// не проверено на системе без winget — проверить на чистой ВМ
pub fn install_winget() -> Result<(), String> {
    let script = "$ErrorActionPreference='Stop'; \
        Install-PackageProvider -Name NuGet -Force | Out-Null; \
        Install-Module -Name Microsoft.WinGet.Client -Force -Repository PSGallery | Out-Null; \
        Repair-WinGetPackageManager -AllUsers";
    let (code, out) = run(Command::new("powershell").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script]))?;
    if code == 0 && winget_available() { Ok(()) } else { Err(t!("не удалось установить winget: {text}", text = last_line(&out))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA256 отключён: #[test]
    // SHA256 отключён: fn sha256_check() {
        // SHA256 отключён: // SHA256 пустой строки
        // SHA256 отключён: assert!(verify_sha256(b"", "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855").is_ok());
        // SHA256 отключён: assert!(verify_sha256(b"x", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855").is_err());
    // SHA256 отключён: }

    #[test]
    fn file_names() {
        assert_eq!(file_name("https://a.com/dl/setup.exe?v=2"), "setup.exe");
        assert_eq!(file_name("https://a.com/download"), "installer.exe");
    }

    #[test]
    fn check_path_decides() {
        let mut sw = Software { name: None, winget: None, version: None, url: Some("https://x/y.exe".into()),
            args: None, check_path: None, extract_to: None }; // SHA256 отключён: было ещё sha256: None
        assert_eq!(InstallerStep::new(&sw).check(), Ok(false)); // без check_path — всегда ставим
        sw.check_path = Some(std::env::current_exe().unwrap().display().to_string());
        assert_eq!(InstallerStep::new(&sw).check(), Ok(true));
    }
}
