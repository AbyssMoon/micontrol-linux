# MI Control для Linux

Управление функциями ноутбуков Xiaomi / REDMI Book в Linux: режимы производительности,
лимит заряда батареи, индикатор микрофона. Порт
[MI Control Utility](https://github.com/loginsinex/micontrol) (Windows) на Rust + Qt 6 / QML.

*Linux port of MI Control Utility for Xiaomi / REDMI Book laptops: performance modes,
battery charge limit, microphone LED. Rust + Qt 6.*

| Производительность | Батарея | Микрофон |
|---|---|---|
| ![](docs/screenshots/performance.png) | ![](docs/screenshots/battery.png) | ![](docs/screenshots/microphone.png) |

## Возможности

- **Режимы производительности**: тихий, авто, турбо, максимум (на старых моделях — баланс).
  Опция «Полная мощность режимов» снимает ограничение BIOS PL1 = 28 Вт, без неё режимы
  меняют в основном работу вентиляторов.
- **Лимит заряда**: 40–90% или без ограничения; демон применяет его снова после сна и
  переподключения зарядки.
- **Микрофон**: mute в PipeWire и индикатор на клавише.
- **Мониторинг**: потребление и частота CPU, температура, мощность заряда/разряда, здоровье батареи.
- **Значок в трее**: текущее состояние и быстрое переключение по правому клику.

## Проверенные модели

| Модель | SKU | BIOS | Статус |
|---|---|---|---|
| REDMI Book Pro 14 2025 | TM2411 | RMAAR4B0P0505 | всё работает |

Другие ноутбуки Xiaomi с WMI-интерфейсом `MiInterface` скорее всего тоже подойдут
(в оригинальном MIControl — RedmiBook / Xiaomi Book разных лет), но не проверены.
Помогите расширить список — см. [«Проверка своей модели»](#проверка-своей-модели).

## Требования и ограничения

- Модуль ядра [`acpi_call`](https://github.com/nix-community/acpi_call): через него идут вызовы
  прошивки. Исходники лежат в `third_party/acpi_call`, нужны заголовки ядра (`kernel-devel` /
  `linux-headers`).
- **Secure Boot**: собранный вручную модуль не загрузится, пока он не подписан ключом, внесённым
  в MOK. С выключенным Secure Boot всё работает как есть.
- Демон `micontrold` работает от root; GUI и CLI — от обычного пользователя через Unix-сокет.

## Быстрый старт (переносная сборка)

Ничего не устанавливает в систему:

```sh
tar xf micontrol-portable.tar.xz && cd micontrol-portable
./build-acpi_call.sh   # один раз: собрать модуль под своё ядро
./run.sh probe         # только чтение: проверить, что прошивка отвечает
./run.sh               # демон (sudo) + окно; выход — через меню в трее
./run.sh --mock        # интерфейс без железа и без root
```

## Сборка из исходников

Тулчейн ставится в `.toolchain/` внутри проекта (Rust через rustup, Qt 6.8.3 через aqtinstall).
Нужны `curl`, `python3`, `gcc`/`g++`, `make` и системная `libGL`.

```sh
./scripts/setup-toolchain.sh
source env.sh
cargo build --workspace
cargo test --workspace

./scripts/build-acpi_call.sh      # → build/acpi_call/acpi_call.ko
./scripts/make-portable.sh        # → dist/micontrol-portable
```

Разработка без ноутбука Xiaomi:

```sh
export MICONTROL_SOCKET=$XDG_RUNTIME_DIR/micontrol-dev.sock
./target/debug/micontrold --mock --socket $MICONTROL_SOCKET --state /tmp/mc-state.json &
./target/debug/micontrol-gui
```

Отладка на ноутбуке: `sudo ./scripts/dev-daemon.sh` запускает демон с `--debug`
(сокет доступен только вашему пользователю), после чего `micontrol dbg get|set …` делает
произвольные вызовы прошивки без root.

## Проверка своей модели

```sh
./scripts/build-acpi_call.sh
sudo ./scripts/probe-hw.sh        # ТОЛЬКО ЧТЕНИЕ, ничего не меняет → build/probe/
```

Откройте issue с файлом `build/probe/calls.txt` и архивом `build/probe/tables/`
(ACPI-таблицы — по ним видно, какие команды понимает прошивка).

## Как это работает

Вызовы идут в ACPI-метод `\_SB.PC00.WMID.WMAA` (WMI `MiInterface`, GUID
`B60BFB48-3E5B-49E4-A0E9-8CFFE1B3434B`) через `acpi_call`. Буфер 32 байта:
`{00, FB|FA, 00, CMD, ARG, 00, ARG2, 00, …}` (`FB` — запись, `FA` — чтение). В ответе байт 1 —
статус (`80` — успех, `E0` — не поддерживается), значения — в байтах 4 и 6.

| Функция | CMD | Значения |
|---|---|---|
| Режим производительности | `0x08` | 02 тихий, 03 турбо, 04 максимум, 09 авто (01 баланс — старые модели) |
| Микрофон | `0x0A`, ARG `0x05` | ARG2: 1 — включён, 0 — выключен |
| Лимит заряда | `0x10`, ARG `0x02` | ARG2: 00 выкл, 04 90%, 01 80%, 05 70%, 06 60%, 07 50%, 08 40% |
| Здоровье батареи | `0x10`, ARG `0x01` | ARG2: проценты (только чтение) |

Значения сверены с ACPI-таблицами REDMI Book Pro 14 2025 (SSDT `XMCC1806`). Прошивка
отвечает «успех» и на неподдерживаемые значения (например, режим 01 на моделях 2025),
поэтому демон перечитывает значение после записи.

Режим прошивка передаёт в Intel DTT (`ODVP1`), и `thermald --adaptive` выставляет PL1 из
таблиц Xiaomi — но только в MMIO-домен RAPL. В MSR-домене остаётся 28 Вт от BIOS, а действует
меньший из лимитов; «Полная мощность режимов» копирует PL1 из DTT в MSR.

Драйвер ядра `bitland-mifs-wmi` работает с тем же интерфейсом, но с чужими значениями режимов:
его `platform_profile` на этих моделях не читается, а запись `low-power` (профиль
«Энергосбережение» в tuned-ppd / power-profiles-daemon) включает тихий режим.

## Устройство

- `crates/micontrol-core` — протокол, бэкенды (`acpi_call`, `mock`), IPC, powercap, DMI.
- `crates/micontrold` — демон (root): вызовы прошивки, сохранение настроек, повторное
  применение лимита после сна и смены питания, синхронизация PL1.
- `crates/micontrol-cli` — `micontrol`: клиент демона, `probe` / `raw` / `dbg` для отладки.
- `crates/micontrol-gui` — интерфейс на QML (cxx-qt) со значком в трее.

## Документация

- [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) — архитектура, IPC, отладка на железе, как добавить
  функцию или модель, релиз, известные проблемы.
- [docs/FIRMWARE.md](docs/FIRMWARE.md) — справочник по `WMAA`, событиям WMI, DTT и лимитам
  мощности, открытые вопросы.
- [CONTRIBUTING.md](CONTRIBUTING.md) — как проверить свою модель и прислать данные.

## Лицензия

GNU GPL v3 или новее (как и оригинальный MIControl). `third_party/acpi_call` — GPL v3,
© авторы acpi_call.

Проект не связан с Xiaomi. Используйте на свой страх и риск.
