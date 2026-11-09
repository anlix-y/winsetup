//! Каталог программ: встроенный (в exe) + пользовательский (%AppData%\winsetup\catalog.toml).

use crate::config::Software;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub category: String,
    pub name: String,
    pub winget: Option<String>,
    pub url: Option<String>,
    pub args: Option<String>,
    // SHA256 отключён: pub sha256: Option<String>,
    pub check_path: Option<String>,
    pub extract_to: Option<String>,
}

impl Item {
    pub fn to_software(&self) -> Software {
        Software {
            name: Some(self.name.clone()),
            winget: self.winget.clone(),
            version: None,
            url: self.url.clone(),
            args: self.args.clone(),
            // SHA256 отключён: sha256: self.sha256.clone(),
            check_path: self.check_path.clone(),
            extract_to: self.extract_to.clone(),
        }
    }

    /// Та же программа уже есть в конфиге? (сравнение по winget-ID или URL)
    pub fn matches(&self, s: &Software) -> bool {
        (self.winget.is_some() && self.winget == s.winget) || (self.url.is_some() && self.url == s.url)
    }
}

/// Встроенное приложение Windows, которое можно удалить.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub name: String,
    pub package: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    pub software: Vec<Item>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<App>,
}

pub fn builtin() -> Catalog {
    toml::from_str(include_str!("catalog.toml")).expect(t!("встроенный каталог корректен (проверено тестом)"))
}

fn user_path() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("winsetup").join("catalog.toml"))
}

/// Пользовательский каталог; нет файла — пустой. Битый файл — ошибка (не затираем молча).
pub fn load_user() -> Result<Catalog, String> {
    let Some(p) = user_path() else { return Ok(Catalog::default()) };
    match std::fs::read_to_string(&p) {
        Ok(t) => toml::from_str(&t).map_err(|e| format!("{}: {e}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Catalog::default()),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

pub fn save_user(c: &Catalog) -> Result<(), String> {
    let p = user_path().ok_or(t!("не задана переменная APPDATA"))?;
    if let Some(dir) = p.parent() { std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    let text = toml::to_string_pretty(c).map_err(|e| e.to_string())?;
    std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn builtin_parses() {
        let c = super::builtin();
        assert_eq!(c.software.len(), 23);
        for i in &c.software {
            let cfg = crate::config::Config { software: vec![i.to_software()], ..Default::default() };
            assert!(crate::config::validate(&cfg).is_empty(), "{}", i.name);
        }
        assert!(c.remove.len() >= 15);
        let cfg = crate::config::Config { remove_apps: c.remove.iter().map(|a| a.package.clone()).collect(), ..Default::default() };
        assert!(crate::config::validate(&cfg).is_empty());
    }
}
