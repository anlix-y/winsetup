# winsetup

Помощник системного администратора: быстрая установка софта и настройка Windows 10/11 по одному конфигу.

*A sysadmin helper: install software and configure Windows 10/11 from a single declarative config. UI in Russian and English.*

- **Один exe** без установки и зависимостей.
- **Декларативный TOML-конфиг**: описываете, *каким* должен быть ПК, а не *что делать*.
- **Повторный запуск безопасен**: перед каждым шагом проверяется текущее состояние, лишнее пропускается.
- **Пробный прогон**: сначала показывается план — что уже в порядке, что будет изменено.
- **GUI-конструктор** конфига: каталог программ, готовые наборы настроек, подсказки к каждому полю.

## Запуск

Открыть окно (PowerShell от администратора или с запросом UAC):

```powershell
irm https://raw.githubusercontent.com/anlix-y/winsetup/main/install.ps1 | iex
```

Сразу применить конфиг, без окна:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/anlix-y/winsetup/main/install.ps1))) https://example.com/office.toml
```

Или скачать `winsetup.exe` из [релизов](https://github.com/anlix-y/winsetup/releases/latest):

```
winsetup.exe                         открыть окно
winsetup.exe office.toml             применить конфиг
winsetup.exe https://…/office.toml   конфиг по ссылке
winsetup.exe office.toml --dry-run   только показать план
winsetup.exe office.toml --lang en   язык интерфейса
```

## Что умеет

| Раздел | Что настраивается |
|---|---|
| Софт | winget-пакеты, установщики по ссылке, zip-архивы; удаление встроенных приложений Windows |
| Сеть | IP, маска, шлюз, DNS, DHCP |
| Брандмауэр | профили, правила (TCP/UDP/ICMP) |
| Реестр | значения всех типов |
| Службы | тип запуска, состояние |
| Пользователи | локальные учётные записи, пароли, группы |
| Компоненты Windows | включение/отключение |
| Электропитание | схема, сон, отключение экрана |
| Windows Update | режим, отсрочка обновлений |
| Твики | телеметрия, реклама, расширения и скрытые файлы |
| Общие | имя ПК, поведение при ошибке, перезагрузка |

## Пример конфига

```toml
[general]
on_error = "ask"      # ask | continue | stop

[computer]
name = "OFFICE-PC-01"

[[software]]
winget = "Google.Chrome"

[[service]]
name = "Spooler"
startup = "disabled"
state = "stopped"

[tweaks]
show_file_extensions = true
```

Полный пример — [examples/office.toml](examples/office.toml).

## Сборка

Собирается на Linux под Windows:

```sh
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
cargo xwin build --release --target x86_64-pc-windows-msvc
cargo test
```

## Ограничения

- Проверка SHA256 скачиваемых файлов отключена: exe и установщики по ссылкам запускаются без проверки подлинности.
- Пароли пользователей хранятся в конфиге открытым текстом.
- Отката изменений нет — перед применением показывается план, а тестировать стоит на виртуальной машине.

## Лицензия

[MIT](LICENSE)
