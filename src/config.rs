//! Схема TOML-конфига. Любой раздел необязателен.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    pub computer: Option<Computer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub software: Vec<Software>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network: Vec<Network>,
    pub firewall: Option<Firewall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub registry: Vec<Registry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service: Vec<Service>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user: Vec<User>,
    pub features: Option<Features>,
    pub power: Option<Power>,
    pub windows_update: Option<WindowsUpdate>,
    pub tweaks: Option<Tweaks>,
    /// Встроенные приложения Windows (Appx) для удаления, например Microsoft.BingNews.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_apps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct General {
    pub name: Option<String>,
    #[serde(default)]
    pub on_error: OnError,
    #[serde(default)]
    pub reboot: Reboot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OnError {
    #[default]
    Ask,
    Continue,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Reboot {
    #[default]
    Ask,
    Never,
    Always,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Computer {
    pub name: String,
}

/// Либо `winget`, либо `url` (+ args, check_path, extract_to) — проверяется в `validate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Software {
    pub name: Option<String>,
    pub winget: Option<String>,
    pub version: Option<String>,
    pub url: Option<String>,
    pub args: Option<String>,
    // SHA256 отключён: pub sha256: Option<String>,
    pub check_path: Option<String>,
    /// Ссылка на .zip: распаковать в эту папку вместо запуска установщика.
    pub extract_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Network {
    pub adapter: String,
    pub dhcp: bool,
    pub ip: Option<Ipv4Addr>,
    pub prefix: Option<u8>,
    pub gateway: Option<Ipv4Addr>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns: Vec<Ipv4Addr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Firewall {
    pub domain: Option<bool>,
    pub private: Option<bool>,
    pub public: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule: Vec<FirewallRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FirewallRule {
    pub name: String,
    pub direction: Direction,
    pub action: Action,
    pub protocol: Protocol,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction { In, Out }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action { Allow, Block }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol { Tcp, Udp, Icmp, Any }

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub path: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: RegType,
    /// Соответствие типу проверяется в `validate`.
    pub value: toml::Value,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RegType { Dword, Qword, String, ExpandString, MultiString }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub name: String,
    pub startup: Option<Startup>,
    pub state: Option<ServiceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Startup { Auto, Manual, Disabled }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState { Running, Stopped }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct User {
    pub name: String,
    pub password: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Features {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disable: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Power {
    pub plan: Option<PowerPlan>,
    pub sleep_ac_minutes: Option<u32>,
    pub monitor_off_ac_minutes: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum PowerPlan { Balanced, HighPerformance, PowerSaver }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct WindowsUpdate {
    pub mode: Option<UpdateMode>,
    pub defer_feature_days: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode { Auto, Notify, Disabled }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Tweaks {
    pub telemetry: Option<bool>,
    pub ads: Option<bool>,
    pub show_file_extensions: Option<bool>,
    pub show_hidden_files: Option<bool>,
}

#[derive(Debug, PartialEq)]
pub enum Hive { LocalMachine, CurrentUser, ClassesRoot, Users }

/// `HKLM\SOFTWARE\X` → (LocalMachine, "SOFTWARE\X").
pub fn split_reg_path(path: &str) -> Option<(Hive, &str)> {
    let (h, rest) = path.split_once('\\')?;
    let hive = match h.to_ascii_uppercase().as_str() {
        "HKLM" | "HKEY_LOCAL_MACHINE" => Hive::LocalMachine,
        "HKCU" | "HKEY_CURRENT_USER" => Hive::CurrentUser,
        "HKCR" | "HKEY_CLASSES_ROOT" => Hive::ClassesRoot,
        "HKU" | "HKEY_USERS" => Hive::Users,
        _ => return None,
    };
    (!rest.is_empty()).then_some((hive, rest))
}

/// Разбор + проверки, которые не выразить типами. Ошибки синтаксиса содержат номер строки (от `toml`).
pub fn parse(text: &str) -> Result<Config, String> {
    let cfg: Config = toml::from_str(text).map_err(|e| e.to_string())?;
    let errors = validate(&cfg);
    if errors.is_empty() { Ok(cfg) } else { Err(errors.join("\n")) }
}

/// Предупреждения (не мешают применению).
pub fn warnings(cfg: &Config) -> Vec<String> {
    let mut w = Vec::new();
    for s in cfg.software.iter().filter(|s| s.url.is_some()) {
        let n = s.name.as_deref().or(s.url.as_deref()).unwrap_or_default();
        // SHA256 отключён: if s.sha256.is_none() { w.push(format!("software «{n}»: нет sha256, файл не будет проверен")); }
        if s.check_path.is_none() { w.push(t!("software «{n}»: нет check_path, установщик будет запускаться каждый раз", n = n)); }
    }
    w
}

pub fn validate(cfg: &Config) -> Vec<String> {
    let mut e = Vec::new();
    if let Some(c) = &cfg.computer {
        let n = &c.name;
        // правила NetBIOS-имени: до 15 символов, буквы/цифры/дефис, не только цифры
        if n.is_empty() || n.len() > 15 || !n.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
            || n.chars().all(|ch| ch.is_ascii_digit()) || n.starts_with('-') || n.ends_with('-') {
            e.push(t!("computer.name «{n}»: 1–15 латинских букв/цифр/дефисов, не только цифры", n = n));
        }
    }
    for s in &cfg.service {
        if s.startup.is_none() && s.state.is_none() {
            e.push(t!("service «{name}»: укажите startup и/или state", name = s.name));
        }
    }
    if let Some(d) = cfg.windows_update.as_ref().and_then(|w| w.defer_feature_days) {
        if d > 365 { e.push(t!("windows_update.defer_feature_days = {d}: максимум 365", d = d)); }
    }
    for r in cfg.firewall.iter().flat_map(|f| &f.rule) {
        if r.port.is_some() && matches!(r.protocol, Protocol::Any | Protocol::Icmp) {
            e.push(t!("firewall.rule «{name}»: port требует protocol = tcp или udp", name = r.name));
        }
    }
    for a in &cfg.remove_apps {
        if a.is_empty() || !a.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c)) {
            e.push(t!("remove_apps «{a}»: имя пакета — латиница, цифры, точки (Microsoft.BingNews)", a = a));
        }
    }
    for (i, s) in cfg.software.iter().enumerate() {
        let n = i + 1;
        match (&s.winget, &s.url) {
            (Some(_), Some(_)) => e.push(t!("software #{n}: укажите либо winget, либо url, не оба", n = n)),
            (None, None) => e.push(t!("software #{n}: нужен winget или url", n = n)),
            // SHA256 отключён: (Some(_), None) if s.args.is_some() || s.sha256.is_some() || s.check_path.is_some() =>
            (Some(_), None) if s.args.is_some() || s.check_path.is_some() || s.extract_to.is_some() =>
                e.push(t!("software #{n}: args/check_path/extract_to только для url", n = n)),
            _ => {}
        }
        // SHA256 отключён: if let Some(h) = &s.sha256 {
            // SHA256 отключён: if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
                // SHA256 отключён: e.push(format!("software #{n}: sha256 должен быть 64 hex-символа"));
            // SHA256 отключён: }
        // SHA256 отключён: }
    }
    for n in &cfg.network {
        if n.dhcp {
            if n.ip.is_some() || n.prefix.is_some() || n.gateway.is_some() {
                e.push(t!("network «{name}»: при dhcp = true ip/prefix/gateway не указываются", name = n.adapter));
            }
        } else {
            if n.ip.is_none() || n.prefix.is_none() {
                e.push(t!("network «{name}»: при dhcp = false нужны ip и prefix", name = n.adapter));
            }
            if n.prefix.is_some_and(|p| p == 0 || p > 32) {
                e.push(t!("network «{name}»: prefix должен быть 1..32", name = n.adapter));
            }
        }
    }
    for r in &cfg.registry {
        use toml::Value as V;
        let ok = match (&r.kind, &r.value) {
            (RegType::Dword, V::Integer(v)) => (0..=u32::MAX as i64).contains(v),
            (RegType::Qword, V::Integer(v)) => *v >= 0,
            (RegType::String | RegType::ExpandString, V::String(_)) => true,
            (RegType::MultiString, V::Array(a)) => a.iter().all(|x| x.is_str()),
            _ => false,
        };
        if !ok { e.push(t!("registry {path}\\{name}: значение не подходит к типу {kind}", path = r.path, name = r.name, kind = format!("{:?}", r.kind))); }
        if split_reg_path(&r.path).is_none() {
            e.push(t!("registry {path}: путь должен начинаться с HKLM, HKCU, HKCR или HKU", path = r.path));
        }
    }
    e
}

/// Конфиг → TOML-текст (для сохранения из GUI).
pub fn to_toml(cfg: &Config) -> Result<String, String> {
    toml::to_string_pretty(cfg).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let cfg = parse(include_str!("../examples/office.toml")).unwrap();
        let text = to_toml(&cfg).unwrap();
        let again = parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(to_toml(&again).unwrap(), text);
        assert!(!to_toml(&Config::default()).unwrap().contains("software"));
    }

    #[test]
    fn example_office_parses() {
        let cfg = parse(include_str!("../examples/office.toml")).unwrap();
        assert_eq!(cfg.software.len(), 3);
        assert_eq!(cfg.network[0].ip, Some(Ipv4Addr::new(192, 168, 1, 50)));
        assert_eq!(cfg.general.on_error, OnError::Ask);
        assert!(warnings(&cfg).is_empty());
    }

    #[test]
    fn empty_config_is_valid() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg.general.reboot, Reboot::Ask);
    }

    #[test]
    fn unknown_field_has_line_number() {
        let err = parse("[computer]\nname = \"PC\"\nnmae = \"x\"\n").unwrap_err();
        assert!(err.contains("line 3") && err.contains("nmae"), "{err}");
    }

    #[test]
    fn bad_values_rejected() {
        assert!(parse("[[network]]\nadapter=\"E\"\ndhcp=false\nip=\"300.1.1.1\"\nprefix=24").is_err());
        assert!(parse("[[network]]\nadapter=\"E\"\ndhcp=false").is_err());
        assert!(parse("[[software]]\nwinget=\"a\"\nurl=\"b\"").is_err());
        // SHA256 отключён: assert!(parse("[[software]]\nurl=\"b\"\nsha256=\"zz\"").is_err());
        assert!(parse("[[registry]]\npath='HKLM\\X'\nname=\"a\"\ntype=\"dword\"\nvalue=\"text\"").is_err());
        assert!(parse("[computer]\nname=\"ОФИС\"").is_err());
        assert!(parse("[computer]\nname=\"1234\"").is_err());
        assert!(parse("[computer]\nname=\"A-VERY-LONG-NAME-123\"").is_err());
        assert!(parse("[computer]\nname=\"PC-01\"").is_ok());
        assert!(parse("[[service]]\nname=\"x\"").is_err());
        assert!(parse("remove_apps = [\"Microsoft.BingNews\", \"Clipchamp.Clipchamp\"]").is_ok());
        assert!(parse("remove_apps = [\"Bing News\"]").is_err());
        assert!(parse("[[software]]\nwinget=\"a\"\nextract_to='C:\\x'").is_err());
        assert!(parse("[[firewall.rule]]\nname=\"x\"\ndirection=\"in\"\naction=\"allow\"\nprotocol=\"any\"\nport=80").is_err());
        assert!(parse("[[registry]]\npath='HKXX\\X'\nname=\"a\"\ntype=\"dword\"\nvalue=1").is_err());
        assert!(parse("[[registry]]\npath='hkey_local_machine\\X'\nname=\"a\"\ntype=\"dword\"\nvalue=1").is_ok());
    }

    #[test]
    fn url_without_check_path_warns() {
        let cfg = parse("[[software]]\nurl=\"https://x/y.exe\"").unwrap();
        assert_eq!(warnings(&cfg).len(), 1); // SHA256 отключён: было 2 (ещё «нет sha256»)
    }
}
