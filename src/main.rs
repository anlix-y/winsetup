#[macro_use]
mod i18n;
mod catalog;
mod config;
mod elevate;
mod engine;
mod forms;
mod gui;
mod modules;
mod net;
mod presets;
mod runner;
mod step;

use engine::Choice;
use std::io::Write;

/// `None` — ввод недоступен (конец потока или ошибка чтения), спрашивать дальше бессмысленно.
fn prompt(q: &str) -> Option<String> {
    print!("{q} ");
    std::io::stdout().flush().ok();
    let mut s = String::new();
    match std::io::stdin().read_line(&mut s) {
        Ok(0) => None,
        Ok(_) => Some(s.trim().to_lowercase()),
        Err(e) => { eprintln!("{}", t!("\nОшибка чтения ввода: {e}", e = e)); None }
    }
}

fn yes(q: &str) -> bool { matches!(prompt(q).as_deref(), Some("y" | "д")) }

fn flush() { std::io::stdout().flush().ok(); }

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let elevated = args.iter().any(|a| a == elevate::ELEVATED_FLAG);
    args.retain(|a| a != elevate::ELEVATED_FLAG);
    let lang = args.iter().position(|a| a == "--lang").and_then(|i| args.get(i + 1)).map(|l| l == "en");
    if let Some(i) = args.iter().position(|a| a == "--lang") { args.drain(i..(i + 2).min(args.len())); }
    i18n::set_english(lang.or_else(i18n::saved).unwrap_or_else(i18n::detect));

    if args.is_empty() {
        // без аргументов — GUI (тоже с правами администратора)
        if !elevate::is_admin() {
            if let Err(e) = elevate::relaunch_as_admin(&[]) { eprintln!("{e}"); std::process::exit(1); }
            return;
        }
        hide_own_console();
        if let Err(e) = gui::run() { eprintln!("{}", t!("Не удалось открыть окно: {e}", e = e)); std::process::exit(1); }
        return;
    }

    let code = match run(&args) {
        Ok(true) => 1,
        Ok(false) => 0,
        Err(e) => { eprintln!("{e}"); 1 }
    };
    // окно после UAC новое и закроется сразу — даём прочитать вывод
    if elevated { let _ = prompt(t!("\nНажмите Enter, чтобы закрыть окно.")); }
    std::process::exit(code);
}

/// `Ok(true)` — были ошибки шагов.
fn run(args: &[String]) -> Result<bool, String> {
    let dry_only = args.iter().any(|a| a == "--dry-run");
    let Some(source) = args.iter().find(|a| !a.starts_with("--")) else {
        return Err(t!("Использование: winsetup <config.toml | https://…/config.toml> [--dry-run]").into());
    };

    if !elevate::is_admin() {
        // после UAC рабочая папка станет system32 — передаём абсолютный путь
        let abs: Vec<String> = args.iter().map(|a| {
            if a == source && !net::is_url(a) {
                std::path::absolute(a).map(|p| p.display().to_string()).unwrap_or_else(|_| a.clone())
            } else { a.clone() }
        }).collect();
        println!("{}", t!("Нужны права администратора, запрашиваю UAC…"));
        elevate::relaunch_as_admin(&abs)?;
        return Ok(false);
    }

    let text = load_config(source)?;
    let cfg = config::parse(&text).map_err(|e| t!("Ошибка в конфиге:\n{e}", e = e))?;
    for w in config::warnings(&cfg) { println!("⚠ {w}"); }

    use modules::software::{install_winget, winget_available};
    if cfg.software.iter().any(|s| s.winget.is_some()) && !winget_available() {
        if yes(t!("winget не найден. Установить его? [y/N]")) {
            println!("{}", t!("Устанавливаю winget…"));
            match install_winget() {
                Ok(()) => println!("{}", t!("winget установлен.")),
                Err(e) => println!("✗ {e}"),
            }
        } else {
            println!("{}", t!("Без winget пакеты из winget будут пропущены с ошибкой."));
        }
    }

    let steps = engine::build_steps(&cfg);
    let total = steps.len();
    println!("{}", t!("\nПлан (проверка текущего состояния):"));
    for (i, step) in steps.iter().enumerate() {
        let d = step.describe();
        // короткая строка: длинная перенеслась бы, и \r её не затёр бы
        let waiting = format!("  [{}/{total}] {}", i + 1, t!("проверяю…"));
        print!("{waiting}");
        flush();
        let line = match step.check() {
            Ok(true) => t!("  ✓ уже в порядке   {d}", d = d),
            Ok(false) => t!("  → будет изменено  {d}", d = d),
            Err(e) => t!("  ✗ ошибка проверки {d}: {e}", d = d, e = e),
        };
        // \r + добивка пробелами; ESC-коды не все консоли Windows понимают
        let pad = waiting.chars().count().saturating_sub(line.chars().count());
        println!("\r{line}{}", " ".repeat(pad));
    }
    if dry_only { return Ok(false); }

    println!("{}", t!("\nПрименение:"));
    let report = engine::apply(
        &steps,
        &cfg.general.on_error,
        &mut |_, _, forceable| loop {
            // SHA256 отключён: let q = if forceable {
                // SHA256 отключён: "  Ошибка. 1 — остановить, 2 — пропустить, 3 — всегда пропускать,\n  \
                 // SHA256 отключён: 4 — всё равно установить (НЕБЕЗОПАСНО), 5 — всё равно устанавливать всё:"
            // SHA256 отключён: } else {
                // SHA256 отключён: "  Ошибка. 1 — остановить, 2 — пропустить, 3 — всегда пропускать:"
            // SHA256 отключён: };
            let _ = forceable;
            let q = t!("  Ошибка. 1 — остановить, 2 — пропустить, 3 — всегда пропускать:");
            match prompt(q).as_deref() {
                None => return Choice::Stop, // ввода нет — безопаснее остановиться
                Some("1") => return Choice::Stop,
                Some("2") => return Choice::Skip,
                Some("3") => return Choice::SkipAll,
                // SHA256 отключён: Some("4") if forceable => return Choice::Force,
                // SHA256 отключён: Some("5") if forceable => return Choice::ForceAll,
                Some(_) => {}
            }
        },
        &mut |i, n, d, out| match out {
            None => { println!("{}", t!("  [{i}/{n}] выполняю… {d}", i = i, n = n, d = d)); }
            Some(engine::Outcome::AlreadyOk) => println!("{}", t!("        ✓ уже в порядке")),
            Some(engine::Outcome::Applied) => println!("{}", t!("        ✓ готово")),
            Some(engine::Outcome::Failed(e)) => println!("        ✗ {e}"),
            Some(engine::Outcome::NotRun) => println!("{}", t!("  [{i}/{n}] не выполнено {d}", i = i, n = n, d = d)),
        },
    );
    let ok = report.results.iter().filter(|(_, o)| matches!(o, engine::Outcome::Applied | engine::Outcome::AlreadyOk)).count();
    println!("{}", t!("\nИтог: успешно {ok} из {total}.", ok = ok, total = total));

    // при ошибке лог сохраняется всегда, иначе — по желанию
    if report.has_errors() || yes(t!("Сохранить лог? [y/N]")) {
        match save_log(&report.log_text()) {
            Ok(p) => println!("{}", t!("Лог: {p}", p = p)),
            Err(e) => eprintln!("{}", t!("Не удалось сохранить лог: {e}", e = e)),
        }
    }

    if report.reboot_needed {
        use config::Reboot;
        let go = match cfg.general.reboot {
            Reboot::Always => true,
            Reboot::Never => false,
            Reboot::Ask => yes(t!("Нужна перезагрузка. Перезагрузить сейчас? [y/N]")),
        };
        if go {
            std::process::Command::new("shutdown").args(["/r", "/t", "0"]).status().ok();
        } else {
            println!("{}", t!("Не забудьте перезагрузить компьютер."));
        }
    }
    Ok(report.has_errors())
}

fn load_config(source: &str) -> Result<String, String> {
    if !net::is_url(source) {
        return std::fs::read_to_string(source).map_err(|e| t!("Не удалось прочитать {source}: {e}", source = source, e = e));
    }
    String::from_utf8(net::fetch(source)?).map_err(|_| t!("{source}: конфиг не в UTF-8", source = source))
}

pub fn save_log(text: &str) -> std::io::Result<String> {
    let base = std::env::var("ProgramData").unwrap_or_else(|_| ".".into());
    let dir = std::path::Path::new(&base).join("winsetup").join("logs");
    std::fs::create_dir_all(&dir)?;
    // shortcut: имя по unix-времени, без даты; сделать читаемую дату, если понадобится
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let file = dir.join(format!("winsetup-{secs}.log"));
    std::fs::write(&file, text)?;
    Ok(file.display().to_string())
}

/// Консоль, созданная только для нас (двойной щелчок по exe), — прячем.
/// Если запустили из PowerShell — консоль общая, не трогаем.
#[cfg(windows)]
fn hide_own_console() {
    use windows_sys::Win32::System::Console::{FreeConsole, GetConsoleProcessList};
    let mut ids = [0u32; 2];
    unsafe {
        if GetConsoleProcessList(ids.as_mut_ptr(), 2) == 1 { FreeConsole(); }
    }
}

#[cfg(not(windows))]
fn hide_own_console() {}
