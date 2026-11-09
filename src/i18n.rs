//! Перевод интерфейса. Русский текст в коде — ключ, английский — в `lang/en.toml` («ключ» = «перевод»).
//! Подстановки — `{имя}`: `t!("Служба {name}", name = s)`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

static ENGLISH: AtomicBool = AtomicBool::new(false);

static EN: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| toml::from_str(include_str!("lang/en.toml")).expect("lang/en.toml корректен (проверено тестом)"));

pub fn set_english(on: bool) { ENGLISH.store(on, Ordering::Relaxed) }
pub fn is_english() -> bool { ENGLISH.load(Ordering::Relaxed) }

/// Перевод строки-ключа. Нет перевода — исходная строка.
pub fn tr(key: &str) -> &str {
    if is_english() {
        if let Some(v) = EN.get(key) { return v; }
    }
    key
}

/// Перевод + подстановка `{имя}`.
pub fn fill(key: &str, args: &[(&str, String)]) -> String {
    let mut s = tr(key).to_string();
    for (k, v) in args { s = s.replace(&format!("{{{k}}}"), v); }
    s
}

/// `t!("текст")` → &str; `t!("текст {x}", x = val)` → String.
#[macro_export]
macro_rules! t {
    ($s:literal) => { $crate::i18n::tr($s) };
    ($s:literal, $($k:ident = $v:expr),+ $(,)?) => {
        $crate::i18n::fill($s, &[$((stringify!($k), ($v).to_string())),+])
    };
}

/// Язык по умолчанию: язык интерфейса Windows (русский → ru, иначе en); `--lang`/настройка GUI важнее.
pub fn detect() -> bool {
    #[cfg(windows)]
    {
        // младшие 10 бит LANGID — основной язык; 0x19 = русский
        let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        id & 0x3ff != 0x19
    }
    #[cfg(not(windows))]
    { !std::env::var("LANG").unwrap_or_default().starts_with("ru") }
}

fn settings_path() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|d| std::path::PathBuf::from(d).join("winsetup").join("lang.txt"))
}

/// Сохранённый в GUI выбор языка.
pub fn saved() -> Option<bool> {
    let s = std::fs::read_to_string(settings_path()?).ok()?;
    match s.trim() { "en" => Some(true), "ru" => Some(false), _ => None }
}

pub fn save(english: bool) {
    if let Some(p) = settings_path() {
        if let Some(d) = p.parent() { std::fs::create_dir_all(d).ok(); }
        std::fs::write(p, if english { "en" } else { "ru" }).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все ключи t!("…") из исходников, каталога и пресетов.
    fn used_keys() -> Vec<String> {
        let mut keys = Vec::new();
        let re_start = "t!(\"";
        for entry in walk(std::path::Path::new("src")) {
            let text = std::fs::read_to_string(&entry).unwrap();
            let code = text.split("#[cfg(test)]").next().unwrap(); // в тестах не переводим
            // строки-комментарии (в т.ч. закомментированный SHA256) пропускаем
            let code: String = code.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
            let mut rest = code.as_str();
            while let Some(i) = rest.find(re_start) {
                let after = &rest[i + re_start.len()..];
                // `format!(` тоже оканчивается на `t!(` — нужен отдельный t!
                let standalone = !rest[..i].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_');
                let end = find_end(after);
                if standalone { keys.push(unescape(&after[..end])); }
                rest = &after[end..];
            }
        }
        let cat = crate::catalog::builtin();
        for i in &cat.software { keys.push(i.name.clone()); keys.push(i.category.clone()); }
        for a in &cat.remove { keys.push(a.name.clone()); }
        for p in crate::presets::builtin() { keys.push(p.name.clone()); keys.push(p.group.clone()); keys.extend(p.about.clone()); }
        keys.sort();
        keys.dedup();
        keys.retain(|k| k.chars().any(|c| ('а'..='я').contains(&c.to_lowercase().next().unwrap_or(c)) || c == 'ё'));
        keys
    }

    fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() { out.extend(walk(&p)) } else if p.extension().is_some_and(|x| x == "rs") { out.push(p) }
        }
        out
    }

    /// Конец строкового литерала с учётом `\"`.
    fn find_end(s: &str) -> usize {
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\\' { i += 2; continue; }
            if b[i] == b'"' { return i; }
            i += 1;
        }
        b.len()
    }

    fn unescape(s: &str) -> String { s.replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\") }

    fn placeholders(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s.split('{').skip(1).filter_map(|x| x.split_once('}').map(|(a, _)| a.to_string())).collect();
        v.sort();
        v
    }

    #[test]
    fn every_key_translated() {
        let missing: Vec<String> = used_keys().into_iter().filter(|k| !EN.contains_key(k)).collect();
        assert!(missing.is_empty(), "нет перевода ({}):\n{}", missing.len(),
            missing.iter().map(|k| format!("{k:?} = \"\"")).collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn placeholders_match() {
        for (k, v) in EN.iter() {
            assert_eq!(placeholders(k), placeholders(v), "подстановки различаются: {k:?} → {v:?}");
        }
    }

    #[test]
    fn switch() {
        set_english(true);
        assert_eq!(tr("Сохранить…"), "Save…");
        assert_eq!(t!("Служба {name}: {parts}", name = "Spooler", parts = "x"), "Service Spooler: x");
        set_english(false);
        assert_eq!(tr("Сохранить…"), "Сохранить…");
    }
}
