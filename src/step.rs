use std::sync::Mutex;

type Sink = Box<dyn Fn(&str) + Send>;
static NOTE_SINK: Mutex<Option<Sink>> = Mutex::new(None);

/// Куда шаги пишут промежуточные сообщения («скачиваю …»): консоль или журнал окна.
pub fn set_note_sink(sink: Option<Sink>) {
    *NOTE_SINK.lock().unwrap_or_else(|e| e.into_inner()) = sink;
}

pub fn note(msg: &str) {
    match &*NOTE_SINK.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(f) => f(msg),
        None => println!("        {msg}"),
    }
}

/// Один шаг настройки. Каждый модуль превращает свой раздел конфига в шаги.
pub trait Step {
    /// Строка для вывода/лога, например «Реестр: HKLM\...\X = 0».
    fn describe(&self) -> String;
    /// `Ok(true)` — уже в нужном состоянии, применять не надо.
    fn check(&self) -> Result<bool, String>;
    fn apply(&self) -> Result<(), String>;
    fn needs_reboot(&self) -> bool { false }
    /// Последнюю ошибку можно обойти по решению пользователя (например, неверный SHA256).
    fn can_force(&self) -> bool { false }
    /// Следующий `apply` выполняется в обход этой ошибки.
    fn set_force(&self) {}
}
