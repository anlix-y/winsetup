pub mod apps;
pub mod computer;
pub mod features;
pub mod firewall;
pub mod network;
pub mod power;
pub mod registry;
pub mod services;
pub mod software;
pub mod tweaks;
pub mod users;

/// Строка в одинарных кавычках PowerShell: внутри экранируется только `'` → `''`.
pub fn q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Выполнить PowerShell-скрипт, вернуть stdout. Ошибка — текст из stderr.
pub fn ps(script: &str) -> Result<String, String> { ps_env(script, &[]) }

/// То же, с переменными окружения — так передаются секреты (пароли), чтобы их не было в командной строке процесса.
pub fn ps_env(script: &str, env: &[(&str, &str)]) -> Result<String, String> {
    // UTF-8 на выходе, иначе кириллица в ошибках приходит в cp866
    let full = format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; $ErrorActionPreference='Stop'; {script}");
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &full])
        .envs(env.iter().copied())
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| t!("не удалось запустить PowerShell: {e}", e = e))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or(t!("неизвестная ошибка PowerShell"));
        Err(first.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quoting() {
        assert_eq!(super::q("Spooler"), "'Spooler'");
        assert_eq!(super::q("a'; rm x; '"), "'a''; rm x; '''");
    }
}
