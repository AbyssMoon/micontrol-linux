# Разработка

Как устроен проект, как его собирать, отлаживать на железе и расширять.
Протокол прошивки — в [FIRMWARE.md](FIRMWARE.md), как помочь проекту — в
[CONTRIBUTING.md](../CONTRIBUTING.md).

## Архитектура

```
 ┌──────────────┐   ┌──────────────┐
 │ micontrol-gui│   │  micontrol   │   пользователь
 │  (Qt / QML)  │   │    (CLI)     │
 └──────┬───────┘   └──────┬───────┘
        │  JSON по Unix-сокету  │
        └──────────┬───────────┘
            ┌──────┴───────┐
            │  micontrold  │   root
            └──┬────────┬──┘
   /proc/acpi/call    /sys/class/powercap (PL1, энергия CPU)
        │
  acpi_call.ko → ACPI \_SB.PC00.WMID.WMAA → EC ноутбука
```

| Крейт | Что внутри |
|---|---|
| `micontrol-core` | `protocol` — формат буфера и значения; `backend` — транспорт (`AcpiCall`, `Mock`); `device` — операции с проверкой; `ipc` — JSON-протокол; `power` — RAPL/powercap; `sysinfo` — DMI. |
| `micontrold` | Демон: обслуживает сокет, хранит настройки (`state.json`), раз в 2 с следит за сном, сменой питания, PL1 и считает мощность CPU. |
| `micontrol-cli` | Клиент демона и прямой доступ к `acpi_call` для отладки. |
| `micontrol-gui` | QObject `Controller` (cxx-qt) + QML. Батарею, частоту и температуру читает сам из sysfs — root для этого не нужен. |

Почему демон: `/proc/acpi/call` и запись `powercap` доступны только root, а `acpi_call`
хранит результат последнего вызова глобально — вызовы нужно сериализовать в одном процессе.

## Сборка

```sh
./scripts/setup-toolchain.sh   # Rust и Qt 6.8.3 в .toolchain/
source env.sh                  # пути к тулчейну; нужен в каждом новом терминале
cargo build --workspace
cargo test --workspace
```

`env.sh` выставляет `RUSTUP_HOME`, `CARGO_HOME`, `QMAKE`, `LD_LIBRARY_PATH` и
`RUSTFLAGS=-L .toolchain/linkfix` (там симлинк `libGL.so` — системный пакет `-devel`
ставить не нужно). `CXXFLAGS=-Wno-sfinae-incomplete` глушит предупреждение GCC 16 в
заголовках Qt.

Переносная сборка: `./scripts/make-portable.sh` → `dist/micontrol-portable/` и
`dist/micontrol-portable.tar.xz`. В неё входят только стиль Fusion и платформы
xcb/wayland; стиль закреплён в `micontrol-gui/src/main.rs`.

Иконки режимов, трея и микрофона — SVG из `scripts/gen-mode-icons.py`; после правки
запустите его и пересоберите GUI (иконки встроены в ресурсы, список — в `build.rs`).

## Запуск без ноутбука Xiaomi

```sh
export MICONTROL_SOCKET=$XDG_RUNTIME_DIR/micontrol-dev.sock
./target/debug/micontrold --mock --socket $MICONTROL_SOCKET --state /tmp/mc-state.json &
./target/debug/micontrol status
./target/debug/micontrol-gui
```

`Mock` повторяет поведение `WMAA` модели 2025: статус `0xE0` на неизвестные функции,
молча игнорирует режим 0x01 и неизвестные лимиты.

> **Важно: эмуляция не проверяет транспорт.** `Mock` подменяет весь `AcpiCall`, поэтому
> ошибки форматирования строки для `/proc/acpi/call` и чтения ответа в ней не видны.
> Так в первой версии прошёл незамеченным баг: `read_to_string` делает пробное чтение
> в 32 байта, `acpi_call` на слишком маленький буфер возвращает 0 — и демон получал
> пустые ответы. Любые изменения в `backend.rs` проверяйте на железе.

Скриншоты для README и проверка вёрстки без окна:

```sh
QT_QPA_PLATFORM=offscreen ./target/debug/micontrol-gui --page 1 --screenshot out.png
```

`--page N` — номер вкладки (0 производительность, 1 батарея, 2 микрофон, 3 система,
4 о программе). Сообщения «No native SystemTrayIcon» в offscreen — норма.

## Отладка на ноутбуке

| Задача | Команда | root |
|---|---|---|
| Собрать модуль под текущее ядро | `./scripts/build-acpi_call.sh` → `build/acpi_call/` | нет |
| Прочитать регистры и снять ACPI-таблицы | `sudo ./scripts/probe-hw.sh` → `build/probe/` | да, только чтение |
| Отладочный демон | `sudo ./scripts/dev-daemon.sh` (не закрывать) | да |
| Произвольный вызов через него | `micontrol dbg get 0x10 0x02` | нет |
| Перезапустить демон после пересборки | `touch build/dev-restart` | нет |
| Лог отладочного демона | `build/daemon.log` | нет |

`dev-daemon.sh` грузит `build/acpi_call/acpi_call.ko`, запускает `target/debug/micontrold
--debug` на `/run/micontrol-dev.sock` (права 0600, владелец — пользователь, вызвавший
sudo) и по Ctrl+C останавливает демон и выгружает модуль. С `--debug` демон пишет в лог
каждый запрос и ошибку и принимает `raw`.

Разбор ACPI-таблиц (`iasl` ставится в `.toolchain/acpica` — см. `dnf download acpica-tools`
+ `rpm2cpio`, или пакет вашего дистрибутива):

```sh
cd build/probe && mkdir -p dsl && cp tables/* dsl/ && cd dsl
iasl -d DSDT SSDT*      # затем grep -l WMAA *.dsl
```

Замеры режимов для таблицы в интерфейсе: `./scripts/measure-modes.sh` (нужен отладочный
демон; около 6 минут на прогон, ноутбук сильно греется).

## IPC

Одна JSON-строка запроса → одна JSON-строка ответа, `\n` в конце. Сокет по умолчанию
`/run/micontrol.sock` (`$MICONTROL_SOCKET` переопределяет), права 0666 — набор команд
ограничен; с `--debug` — 0600.

Запросы (`Request`, поле `cmd`):

```json
{"cmd": "status"}
{"cmd": "set_perf_mode", "mode": "silent"}        // silent|balance|turbo|full_speed|auto
{"cmd": "set_charge_limit", "limit": "p80"}       // off|p90|p80|p70|p60|p50|p40
{"cmd": "set_mic", "enabled": false}
{"cmd": "set_pl1_sync", "enabled": true}
{"cmd": "reapply"}                                // применить сохранённый лимит заряда
{"cmd": "raw", "set": false, "func": 8, "arg": 0, "arg2": 0}   // только с --debug
```

Ответы (`Response`, поле `result`):

```json
{"result": "ok", "status": {
  "backend": "acpi_call: \\_SB_.PC00.WMID.WMAA",
  "perf_mode": 9, "mic_enabled": true,
  "charge_limit_raw": 1, "charge_limit": "p80", "battery_health": 100,
  "pl1_sync": false, "pl1_msr_w": 28, "pl1_dtt_w": 65, "pl2_w": 115, "cpu_power_w": 4.0,
  "errors": [],
  "sysinfo": {"vendor": "XIAOMI", "product": "REDMI Book Pro 14 2025", "sku": "TM2411-63081",
              "bios_version": "RMAAR4B0P0505", "bios_date": "01/16/2025", "serial": "…"}}}
{"result": "raw", "reply": [0, 128, 0, 8, 9, 0, 0, 0, …]}
{"result": "error", "message": "ошибка ACPI: режим «balance» не поддерживается этой моделью …"}
```

- `perf_mode` и `charge_limit_raw` — сырые байты прошивки (неизвестные значения не теряются).
- `charge_limit` — что демон сохранил и поддерживает; в прошивке может стоять другое.
- `errors` — ошибки чтения отдельных функций; статус при этом всё равно отдаётся.
- Все новые поля `Status` помечайте `#[serde(default)]`, чтобы старые клиенты и демоны
  понимали друг друга.

## Как добавить функцию прошивки

На примере чтения нового регистра:

1. **Проверьте на железе**: `micontrol dbg get CMD ARG` через отладочный демон и найдите
   ветку в `WMAA` (см. FIRMWARE.md). Запишите находку в FIRMWARE.md.
2. `protocol.rs` — константа подкоманды / значения, с комментарием-источником.
3. `device.rs` — метод. Для записи — перечитайте значение после неё: прошивка отвечает
   «успех» и на то, что не умеет.
4. `backend.rs` — научите `Mock` новой функции, иначе эмуляция вернёт `0xE0`.
5. `ipc.rs` — поле `Status` (`#[serde(default)]`) и/или вариант `Request`.
6. `micontrold` — заполнить поле в `status()`, обработать запрос в `handle_inner()`.
7. `micontrol-cli` — строка в `print_status` / команда.
8. `micontrol-gui` — `#[qproperty]` в `controller.rs` (имя `foo_bar` в QML станет
   `fooBar`; `#[qinvokable]` не может называться как сеттер свойства `set_*`), затем QML.
9. Тест в `device.rs` на `Mock`; проверка на железе.

## Как добавить модель

- Модели различаются по DMI: `SysInfo::is_redmibook_2025()` (SKU `TM24*`) скрывает
  «Баланс» и показывает «Авто», и включает таблицу замеров `powerTable` в `Main.qml`.
- Для новой модели нужны вывод `probe-hw.sh` и разбор её `WMAA`: значения режимов
  и лимитов могут отличаться. Отличия заводите отдельными методами в `SysInfo`,
  а не проверками SKU по месту.
- Таблицу `powerTable` заполняйте только реальными замерами (`measure-modes.sh`) и
  показывайте только для модели, на которой мерили.

## Релиз

1. Обновите версию в `Cargo.toml` (`workspace.package.version`).
2. `./scripts/make-portable.sh` → `dist/micontrol-portable.tar.xz`.
3. Проверьте сборку на ноутбуке: `./run.sh probe`, `./run.sh`.
4. Тег `vX.Y.Z`, `git push --tags`, на GitHub — Releases → Draft a new release, архив —
   в поле «Attach binaries» (поле описания принимает не больше 25 МБ).

## Известные проблемы и долги

- `status` отдаёт серийный номер ноутбука через сокет с правами 0666 — любой локальный
  пользователь может его прочитать.
- Обработки SIGTERM нет: если демон остановлен с включённой синхронизацией, поднятый PL1
  в MSR остаётся до перезагрузки или до следующего запуска демона (тот вернёт значение
  BIOS, если синхронизация к тому времени выключена).
- `run.sh` запускает GUI; выход — только через меню трея. Без трея (среда без
  StatusNotifierItem) закрытие окна завершает программу.
- Интерфейс опрашивает демон каждые 2 с (5 с, когда окно скрыто), а не подписывается
  на события WMI — смена режима по Fn+K видна с задержкой.
- Нет службы systemd и пакетов — только переносная сборка.
