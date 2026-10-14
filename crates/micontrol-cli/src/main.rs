//! micontrol — консольный клиент демона и инструмент отладки протокола.

use micontrol_core::backend::{AcpiCall, Backend, Mock};
use micontrol_core::ipc::{self, Request, Status};
use micontrol_core::protocol::{
    build_request, hex_dump, ChargeLimit, Cmd, Op, PerfMode, BATTERY_HEALTH_SUBCMD, CHARGE_SUBCMD,
    MIC_SUBCMD,
};

const USAGE: &str = "\
использование: micontrol КОМАНДА

через демон micontrold:
  status                         текущее состояние
  perf silent|balance|turbo|full_speed|auto
  charge off|90|80|70|60|50|40   лимит заряда (on = 80, значение 0x01 прошивки)
  mic on|off                     состояние микрофона в прошивке (индикатор mute)
  reapply                        повторно применить лимит заряда
  pl1sync on|off                 копировать лимит мощности режима (DTT) в MSR PL1
  touchpad on|off|toggle         тачпад; toggle удобно повесить на сочетание клавиш

напрямую через acpi_call (нужен root, демон не нужен):
  probe [--mock]                 прочитать все известные регистры и показать сырые ответы
  raw get|set CMD ARG [ARG2] [--mock]
                                 произвольный вызов, числа в hex или dec (0x10 / 16)

через демон, запущенный с --debug (root не нужен):
  dbg get|set CMD ARG [ARG2]     произвольный вызов, сырой ответ

переменная MICONTROL_SOCKET задаёт путь к сокету демона";

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("ошибка: {msg}");
    std::process::exit(1)
}

fn num(s: &str) -> u8 {
    let r = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u8::from_str_radix(h, 16),
        None => s.parse(),
    };
    r.unwrap_or_else(|_| die(format!("неверное число: {s}")))
}

fn print_status(s: &Status) {
    println!("Бэкенд:        {}", s.backend);
    println!("Устройство:    {} {} (SKU {})", s.sysinfo.vendor, s.sysinfo.product, s.sysinfo.sku);
    println!("BIOS:          {} ({})", s.sysinfo.bios_version, s.sysinfo.bios_date);
    let perf = match s.perf_mode {
        Some(v) => match PerfMode::from_raw(v) {
            Some(m) => m.name().to_string(),
            None => format!("неизвестный (0x{v:02X})"),
        },
        None => "—".into(),
    };
    println!("Режим:         {perf}");
    let mic = s.mic_enabled.map(|v| if v { "вкл" } else { "выкл" }).unwrap_or("—");
    println!("Микрофон:      {mic}");
    let raw = s
        .charge_limit_raw
        .map(|v| format!("0x{v:02X}"))
        .unwrap_or_else(|| "—".into());
    let fw = s
        .charge_limit_raw
        .and_then(ChargeLimit::from_raw)
        .map_or("?", ChargeLimit::name);
    println!("Лимит заряда:  {fw} (прошивка {raw}, сохранено у демона: {})", s.charge_limit.name());
    if let (Some(m), Some(d)) = (s.pl1_msr_w, s.pl1_dtt_w) {
        let sync = if s.pl1_sync { "вкл" } else { "выкл" };
        println!("PL1 CPU:       MSR {m} Вт, DTT {d} Вт (синхронизация {sync})");
    }
    if let Some(p) = s.pl2_w {
        println!("PL2 CPU:       {p} Вт");
    }
    if let Some(p) = s.cpu_power_w {
        println!("Мощность CPU:  {p:.1} Вт");
    }
    if let Some(t) = s.touchpad_enabled {
        println!("Тачпад:        {}", if t { "вкл" } else { "выкл" });
    }
    if let Some(h) = s.battery_health {
        println!("Здоровье АКБ:  {h}%");
    }
    for e in &s.errors {
        println!("! {e}");
    }
}

/// Всплывающее сообщение KDE о тачпаде (как от штатной клавиши); вне KDE — ничего.
fn touchpad_osd(enabled: bool) {
    let _ = std::process::Command::new("busctl")
        .args(["--user", "call", "org.kde.plasmashell", "/org/kde/osdService"])
        .args(["org.kde.osdService", "touchpadEnabledChanged", "b"])
        .arg(enabled.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn direct_backend(mock: bool) -> Box<dyn Backend> {
    if mock {
        return Box::new(Mock::default());
    }
    Box::new(AcpiCall::new(None).unwrap_or_else(|e| die(e)))
}

fn call_and_dump(b: &mut dyn Backend, op: Op, cmd: u8, arg: u8, arg2: u8) {
    let label = if op == Op::Get { "get" } else { "set" };
    print!("{label} cmd=0x{cmd:02X} arg=0x{arg:02X} arg2=0x{arg2:02X} -> ");
    // Без проверки статуса: здесь важен сырой ответ целиком.
    match b.call(&build_request(op, cmd, arg, arg2)) {
        Ok(r) => println!("[{}]", hex_dump(&r)),
        Err(e) => println!("ошибка: {e}"),
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        println!("{USAGE}");
        return;
    }
    let mock = match args.iter().position(|a| a == "--mock") {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let cmd = args.remove(0);
    let arg = |i: usize| args.get(i).map(String::as_str);

    let req = match cmd.as_str() {
        "status" => Request::Status,
        "perf" => {
            let s = arg(0).unwrap_or_else(|| die(USAGE));
            let s = if s == "full" { "full_speed" } else { s };
            Request::SetPerfMode {
                mode: PerfMode::from_name(s).unwrap_or_else(|| die(format!("неизвестный режим: {s}"))),
            }
        }
        "charge" => {
            let s = arg(0).unwrap_or_else(|| die(USAGE));
            Request::SetChargeLimit {
                limit: ChargeLimit::from_name(s).unwrap_or_else(|| die(format!("неизвестный лимит: {s}"))),
            }
        }
        "mic" => Request::SetMic {
            enabled: match arg(0) {
                Some("on") => true,
                Some("off") => false,
                _ => die(USAGE),
            },
        },
        "reapply" => Request::Reapply,
        "pl1sync" => Request::SetPl1Sync {
            enabled: match arg(0) {
                Some("on") => true,
                Some("off") => false,
                _ => die(USAGE),
            },
        },
        "touchpad" => {
            let enabled = match arg(0) {
                Some("on") => true,
                Some("off") => false,
                Some("toggle") => match ipc::request(&Request::Status) {
                    Ok(s) => !s.touchpad_enabled.unwrap_or_else(|| die("тачпад не найден")),
                    Err(e) => die(e),
                },
                _ => die(USAGE),
            };
            match ipc::request(&Request::SetTouchpad { enabled }) {
                Ok(_) => touchpad_osd(enabled),
                Err(e) => die(e),
            }
            return;
        }
        "probe" => {
            let mut b = direct_backend(mock);
            println!("бэкенд: {}", b.describe());
            call_and_dump(b.as_mut(), Op::Get, Cmd::PerfMode as u8, 0, 0);
            call_and_dump(b.as_mut(), Op::Get, Cmd::Mic as u8, MIC_SUBCMD, 0);
            call_and_dump(b.as_mut(), Op::Get, Cmd::ChargeProtect as u8, CHARGE_SUBCMD, 0);
            call_and_dump(b.as_mut(), Op::Get, Cmd::ChargeProtect as u8, BATTERY_HEALTH_SUBCMD, 0);
            println!("(байт 1: 80 — успех, E0 — не поддерживается; режим в байте 4, остальное — в байте 6)");
            return;
        }
        "raw" => {
            let mut b = direct_backend(mock);
            let op = match arg(0) {
                Some("get") => Op::Get,
                Some("set") => Op::Set,
                _ => die(USAGE),
            };
            let c = num(arg(1).unwrap_or_else(|| die(USAGE)));
            let a = num(arg(2).unwrap_or("0"));
            let a2 = num(arg(3).unwrap_or("0"));
            call_and_dump(b.as_mut(), op, c, a, a2);
            return;
        }
        "dbg" => {
            let set = match arg(0) {
                Some("get") => false,
                Some("set") => true,
                _ => die(USAGE),
            };
            let (c, a, a2) = (
                num(arg(1).unwrap_or_else(|| die(USAGE))),
                num(arg(2).unwrap_or("0")),
                num(arg(3).unwrap_or("0")),
            );
            match ipc::raw(set, c, a, a2) {
                Ok(r) => println!("[{}]", hex_dump(&r)),
                Err(e) => die(e),
            }
            return;
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            return;
        }
        other => die(format!("неизвестная команда: {other}\n\n{USAGE}")),
    };

    match ipc::request(&req) {
        Ok(status) => print_status(&status),
        Err(e) => die(e),
    }
}
