use crate::config::Registry;
use crate::step::Step;

/// Одна или несколько записей реестра как один шаг (твик «реклама» = 4 значения).
pub struct RegistryStep {
    pub regs: Vec<Registry>,
    /// Человеческое описание вместо пути, например «Твик: показывать расширения файлов».
    pub label: Option<String>,
}

impl RegistryStep {
    pub fn new(reg: Registry) -> Self { Self { regs: vec![reg], label: None } }

    pub fn labeled(label: &str, regs: Vec<Registry>) -> Self { Self { regs, label: Some(label.into()) } }
}

pub fn dword(path: &str, name: &str, value: u32) -> Registry {
    Registry { path: path.into(), name: name.into(), kind: crate::config::RegType::Dword, value: (value as i64).into() }
}

impl Step for RegistryStep {
    fn describe(&self) -> String {
        match (&self.label, self.regs.as_slice()) {
            (Some(l), _) => l.clone(),
            (None, [r]) => t!("Реестр: {path}\\{name} = {value}", path = r.path, name = r.name, value = r.value),
            (None, _) => t!("Реестр: {n} значений", n = self.regs.len()),
        }
    }

    #[cfg(windows)]
    fn check(&self) -> Result<bool, String> {
        for r in &self.regs {
            if !win::check(r).map_err(|e| format!("{}\\{}: {e}", r.path, r.name))? { return Ok(false); }
        }
        Ok(true)
    }
    #[cfg(windows)]
    fn apply(&self) -> Result<(), String> {
        // запись идемпотентна — пишем все значения
        self.regs.iter().try_for_each(|r| win::apply(r).map_err(|e| format!("{}\\{}: {e}", r.path, r.name)))
    }

    #[cfg(not(windows))]
    fn check(&self) -> Result<bool, String> { Err(t!("доступно только в Windows").into()) }
    #[cfg(not(windows))]
    fn apply(&self) -> Result<(), String> { Err(t!("доступно только в Windows").into()) }
}

#[cfg(windows)]
mod win {
    use crate::config::{split_reg_path, Hive, RegType, Registry};
    use std::io::{ErrorKind, Result};
    use winreg::enums::{self as e, RegType as W};
    use winreg::{RegKey, RegValue};

    fn root(r: &Registry) -> (RegKey, &str) {
        // путь уже проверен в config::validate
        let (hive, sub) = split_reg_path(&r.path).expect("validated");
        let h = match hive {
            Hive::LocalMachine => e::HKEY_LOCAL_MACHINE,
            Hive::CurrentUser => e::HKEY_CURRENT_USER,
            Hive::ClassesRoot => e::HKEY_CLASSES_ROOT,
            Hive::Users => e::HKEY_USERS,
        };
        (RegKey::predef(h), sub)
    }

    /// Строка в UTF-16LE с завершающим нулём — так Windows хранит REG_SZ.
    fn utf16z(s: &str) -> Vec<u8> {
        s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect()
    }

    /// Значение из конфига в «сыром» виде реестра (типы проверены в config::validate).
    fn desired(r: &Registry) -> RegValue<'static> {
        let v = &r.value;
        let (bytes, vtype) = match r.kind {
            RegType::Dword => ((v.as_integer().unwrap() as u32).to_le_bytes().to_vec(), W::REG_DWORD),
            RegType::Qword => ((v.as_integer().unwrap() as u64).to_le_bytes().to_vec(), W::REG_QWORD),
            RegType::String => (utf16z(v.as_str().unwrap()), W::REG_SZ),
            RegType::ExpandString => (utf16z(v.as_str().unwrap()), W::REG_EXPAND_SZ),
            RegType::MultiString => {
                let mut b: Vec<u8> = v.as_array().unwrap().iter()
                    .flat_map(|s| utf16z(s.as_str().unwrap())).collect();
                b.extend([0, 0]); // список завершается пустой строкой
                (b, W::REG_MULTI_SZ)
            }
        };
        RegValue { bytes: bytes.into(), vtype }
    }

    pub fn check(r: &Registry) -> Result<bool> {
        let (root, sub) = root(r);
        let key = match root.open_subkey(sub) {
            Ok(k) => k,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(err),
        };
        match key.get_raw_value(&r.name) {
            Ok(cur) => {
                let want = desired(r);
                Ok(cur.vtype == want.vtype && cur.bytes == want.bytes)
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub fn apply(r: &Registry) -> Result<()> {
        let (root, sub) = root(r);
        let (key, _) = root.create_subkey(sub)?;
        key.set_raw_value(&r.name, &desired(r))
    }
}
