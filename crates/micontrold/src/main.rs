//! micontrold — системный демон (запускается от root).
//!
//! * выполняет ACPI-вызовы по запросам клиентов через Unix-сокет;
//! * сохраняет выбранный лимит заряда и режим производительности;
//! * повторно применяет лимит после загрузки, выхода из сна и смены
//!   источника питания (как `CPowerPowerChargeProtectGuard` в оригинале);
//! * переключает режим производительности по Fn+K;
//! * отключает тачпад.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use micontrol_core::acpi_events;
use micontrol_core::backend::{self, AcpiCall, Backend, Mock};
use micontrol_core::device::Device;
use micontrol_core::ipc::{Request, Response, Status, DEFAULT_SOCKET};
use micontrol_core::protocol::{ChargeLimit, Op, PerfMode, EVENT_FN_K};
use micontrol_core::power;
use micontrol_core::sysinfo::SysInfo;
use micontrol_core::touchpad;
use serde::{Deserialize, Serialize};

const DEFAULT_STATE: &str = "/var/lib/micontrol/state.json";
const WATCH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    charge_limit: ChargeLimit,
    #[serde(default)]
    perf_mode: Option<PerfMode>,
    #[serde(default)]
    pl1_sync: bool,
    /// PL1 MSR от BIOS — для возврата при выключении синхронизации.
    #[serde(default)]
    bios_pl1_uw: Option<u64>,
}

struct Daemon {
    device: Device,
    saved: Saved,
    state_path: PathBuf,
    sysinfo: SysInfo,
    debug: bool,
    cpu_power_w: Option<f64>,
}

impl Daemon {
    fn save(&self) {
        if let Some(dir) = self.state_path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let json = serde_json::to_string_pretty(&self.saved).expect("serialize");
        if let Err(e) = fs::write(&self.state_path, json) {
            log(&format!("не удалось сохранить {}: {e}", self.state_path.display()));
        }
    }

    fn reapply_charge_limit(&mut self, reason: &str) {
        let limit = self.saved.charge_limit;
        match self.device.apply_charge_limit(limit) {
            Ok(()) => log(&format!("лимит заряда «{}» применён ({reason})", limit.name())),
            Err(e) => log(&format!("лимит заряда не применён ({reason}): {e}")),
        }
    }

    /// Подтягивает PL1 в MSR к значению DTT; при выключенной синхронизации
    /// возвращает значение BIOS.
    fn sync_pl1(&mut self) {
        let Some(msr) = power::msr_pl1() else { return };
        let target = if self.saved.pl1_sync {
            power::dtt_pl1()
        } else {
            self.saved.bios_pl1_uw
        };
        let Some(target) = target.filter(|&t| t > 0 && t != msr) else { return };
        match power::set_msr_pl1(target) {
            Ok(()) => log(&format!("PL1 MSR: {} → {} Вт", msr / 1_000_000, target / 1_000_000)),
            Err(e) => log(&format!("PL1 MSR не записан: {e}")),
        }
    }

    /// Fn+K: следующий режим по кругу.
    fn cycle_perf_mode(&mut self) {
        let now = match self.device.perf_mode() {
            Ok(v) => v,
            Err(e) => return log(&format!("Fn+K: режим не прочитан: {e}")),
        };
        let mode = PerfMode::next_fn_k(now, self.sysinfo.is_redmibook_2025());
        match self.device.set_perf_mode(mode) {
            Ok(()) => {
                self.saved.perf_mode = Some(mode);
                self.save();
                log(&format!("Fn+K: режим «{}»", mode.name()));
            }
            Err(e) => log(&format!("Fn+K: режим «{}» не включён: {e}", mode.name())),
        }
    }

    fn status(&mut self) -> Status {
        fn ok<T>(errors: &mut Vec<String>, r: backend::Result<T>, what: &str) -> Option<T> {
            r.map_err(|e| errors.push(format!("{what}: {e}"))).ok()
        }
        let mut errors = Vec::new();
        let perf_mode = ok(&mut errors, self.device.perf_mode(), "режим");
        let mic_enabled = ok(&mut errors, self.device.mic_enabled(), "микрофон");
        let charge_limit_raw = ok(&mut errors, self.device.charge_limit(), "лимит заряда");
        // Есть не на всех моделях — ошибку не показываем.
        let battery_health = self.device.battery_health().ok().filter(|&v| v > 0 && v <= 100);
        Status {
            backend: self.device.describe(),
            perf_mode,
            mic_enabled,
            charge_limit_raw,
            charge_limit: self.saved.charge_limit,
            battery_health,
            pl1_sync: self.saved.pl1_sync,
            pl1_msr_w: power::msr_pl1().map(|v| (v / 1_000_000) as u32),
            pl1_dtt_w: power::dtt_pl1().map(|v| (v / 1_000_000) as u32),
            pl2_w: power::msr_pl2().map(|v| (v / 1_000_000) as u32),
            cpu_power_w: self.cpu_power_w,
            touchpad_enabled: touchpad::enabled(),
            errors,
            sysinfo: self.sysinfo.clone(),
        }
    }

    fn handle(&mut self, req: Request) -> Response {
        if self.debug && !matches!(req, Request::Status) {
            log(&format!("запрос: {req:?}"));
        }
        let resp = self.handle_inner(req);
        if let Response::Error { message } = &resp {
            log(&format!("ошибка: {message}"));
        }
        resp
    }

    fn handle_inner(&mut self, req: Request) -> Response {
        let result = match req {
            Request::Raw { set, func, arg, arg2 } => {
                if !self.debug {
                    return Response::Error { message: "raw доступен только с --debug".into() };
                }
                let op = if set { Op::Set } else { Op::Get };
                return match self.device.call_raw(op, func, arg, arg2) {
                    Ok(reply) => Response::Raw { reply },
                    Err(e) => Response::Error { message: e.to_string() },
                };
            }
            Request::Status => Ok(()),
            Request::SetPerfMode { mode } => self.device.set_perf_mode(mode).map(|()| {
                self.saved.perf_mode = Some(mode);
                self.save();
            }),
            Request::SetMic { enabled } => self.device.set_mic_enabled(enabled),
            Request::SetChargeLimit { limit } => {
                self.device.apply_charge_limit(limit).map(|()| {
                    self.saved.charge_limit = limit;
                    self.save();
                })
            }
            Request::SetPl1Sync { enabled } => {
                if enabled && power::dtt_pl1().is_none() {
                    return Response::Error {
                        message: "нет intel-rapl-mmio: лимиты DTT недоступны".into(),
                    };
                }
                self.saved.pl1_sync = enabled;
                self.save();
                self.sync_pl1();
                Ok(())
            }
            Request::SetTouchpad { enabled } => {
                if let Err(e) = touchpad::set_enabled(enabled) {
                    return Response::Error { message: format!("тачпад: {e}") };
                }
                Ok(())
            }
            Request::Reapply => {
                self.reapply_charge_limit("по запросу");
                Ok(())
            }
        };
        match result {
            Ok(()) => Response::Ok { status: self.status() },
            Err(e) => Response::Error { message: e.to_string() },
        }
    }
}

fn log(msg: &str) {
    // stderr попадает в journald при запуске через systemd.
    eprintln!("{msg}");
}

fn usage() -> ! {
    eprintln!(
        "использование: micontrold [--mock] [--socket ПУТЬ] [--state ПУТЬ] [--method ACPI_ПУТЬ]\n\
         \n  --mock     эмуляция прошивки (для разработки без ноутбука)\n\
         \x20 --socket   путь к сокету (по умолчанию {DEFAULT_SOCKET})\n\
         \x20 --state    файл настроек (по умолчанию {DEFAULT_STATE})\n\
         \x20 --debug    журнал запросов и сырые вызовы через сокет (сокет 0600 для $SUDO_UID)\n\
         \x20 --method   ACPI-метод (по умолчанию ищется автоматически, обычно \\_SB.PC00.WMID.WMAA)"
    );
    std::process::exit(2)
}

/// Время с загрузки, включая сон (в отличие от `Instant`, который сон не считает).
fn boottime() -> Option<f64> {
    fs::read_to_string("/proc/uptime")
        .ok()?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// Подключено ли сетевое питание (`/sys/class/power_supply/*/type == Mains`).
fn ac_online() -> Option<bool> {
    let mut found = None;
    for e in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let dir = e.path();
        if fs::read_to_string(dir.join("type")).unwrap_or_default().trim() != "Mains" {
            continue;
        }
        let online = fs::read_to_string(dir.join("online")).unwrap_or_default().trim() == "1";
        found = Some(found.unwrap_or(false) || online);
    }
    found
}

/// Следит за сменой питания и выходом из сна.
fn watcher(daemon: Arc<Mutex<Daemon>>) {
    let start = Instant::now();
    let base = boottime().unwrap_or(0.0);
    let mut slept = 0.0;
    let mut ac = ac_online();
    let mut energy = power::package_energy_uj().map(|e| (e, Instant::now()));
    loop {
        thread::sleep(WATCH_INTERVAL);

        let now_energy = power::package_energy_uj().map(|e| (e, Instant::now()));
        let cpu_power = match (energy, now_energy) {
            (Some(a), Some(b)) => power::average_power_w(a, b),
            _ => None,
        };
        energy = now_energy;

        let mut reason = None;
        if let Some(now) = boottime() {
            let total_sleep = (now - base) - start.elapsed().as_secs_f64();
            if total_sleep - slept > 1.0 {
                reason = Some("выход из сна");
            }
            slept = total_sleep;
        }
        let new_ac = ac_online();
        if new_ac != ac {
            ac = new_ac;
            reason = Some("смена источника питания");
        }
        let mut d = daemon.lock().unwrap();
        d.cpu_power_w = cpu_power;
        if let Some(reason) = reason {
            if d.saved.charge_limit != ChargeLimit::Off {
                d.reapply_charge_limit(reason);
            }
        }
        // thermald меняет PL1 в DTT при смене режима и питания.
        if d.saved.pl1_sync {
            d.sync_pl1();
        }
    }
}

/// WMI-события прошивки. Ядро сообщает только факт уведомления, номер
/// события читается из прошивки; на RedmiBook 2025 Fn+K переключает режим
/// не сама прошивка, а драйвер (`redmi-wmi` это событие пропускает).
fn wmi_events(daemon: Arc<Mutex<Daemon>>) {
    let listener = match acpi_events::Listener::open() {
        Ok(l) => l,
        Err(e) => return log(&format!("события ACPI недоступны, Fn+K работать не будет: {e}")),
    };
    loop {
        let events = match listener.next() {
            Ok(v) => v,
            Err(e) => {
                log(&format!("события ACPI: {e}"));
                thread::sleep(Duration::from_secs(1));
                continue;
            }
        };
        for ev in events {
            if ev.device_class != "wmi" || ev.kind != 0x20 {
                continue;
            }
            let mut d = daemon.lock().unwrap();
            match d.device.last_event() {
                Ok(EVENT_FN_K) => d.cycle_perf_mode(),
                Ok(id) if d.debug => log(&format!("WMI-событие {id:#04x} ({})", ev.bus_id)),
                Ok(_) => {}
                Err(e) if d.debug => log(&format!("номер WMI-события не прочитан: {e}")),
                Err(_) => {}
            }
        }
    }
}

fn serve(stream: UnixStream, daemon: Arc<Mutex<Daemon>>) {
    let Ok(mut writer) = stream.try_clone() else { return };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => daemon.lock().unwrap().handle(req),
            Err(e) => Response::Error { message: format!("неверный запрос: {e}") },
        };
        let mut out = serde_json::to_string(&resp).expect("serialize");
        out.push('\n');
        if writer.write_all(out.as_bytes()).is_err() {
            break;
        }
    }
}

fn main() {
    let mut mock = false;
    let mut socket = DEFAULT_SOCKET.to_string();
    let mut state = DEFAULT_STATE.to_string();
    let mut method = None;
    let mut debug = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mock" => mock = true,
            "--socket" => socket = args.next().unwrap_or_else(|| usage()),
            "--state" => state = args.next().unwrap_or_else(|| usage()),
            "--method" => method = Some(args.next().unwrap_or_else(|| usage())),
            "--debug" => debug = true,
            _ => usage(),
        }
    }

    let backend: Box<dyn Backend> = if mock {
        Box::new(Mock::default())
    } else {
        match AcpiCall::new(method) {
            Ok(b) => Box::new(b),
            Err(e) => {
                log(&e.to_string());
                std::process::exit(1);
            }
        }
    };

    let state_path = PathBuf::from(state);
    let saved: Option<Saved> = fs::read_to_string(&state_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());
    let first_run = saved.is_none();

    let mut daemon = Daemon {
        device: Device::new(backend),
        saved: saved.unwrap_or_default(),
        state_path,
        sysinfo: SysInfo::read(),
        debug,
        cpu_power_w: None,
    };
    log(&format!("бэкенд: {}", daemon.device.describe()));
    if !mock && !daemon.sysinfo.is_xiaomi() {
        log(&format!(
            "внимание: производитель «{}» — это не Xiaomi, вызовы скорее всего не сработают",
            daemon.sysinfo.vendor
        ));
    }

    // Первый запуск: берём лимит, который уже стоит в прошивке, а не сбрасываем его.
    if first_run {
        if let Ok(limit) = daemon.device.charge_limit() {
            if let Some(limit) = ChargeLimit::from_raw(limit) {
                log(&format!("первый запуск: лимит заряда из прошивки — «{}»", limit.name()));
                daemon.saved.charge_limit = limit;
                daemon.save();
            }
        }
    }

    // Запоминаем PL1 от BIOS, пока его никто не трогал.
    if daemon.saved.bios_pl1_uw.is_none() && !daemon.saved.pl1_sync {
        if let Some(v) = power::msr_pl1() {
            daemon.saved.bios_pl1_uw = Some(v);
            daemon.save();
        }
    }
    // С синхронизацией — подтянуть лимит режима. Без неё — вернуть лимит BIOS,
    // если его оставил поднятым прошлый запуск (например, демон был убит).
    let raised = matches!(
        (power::msr_pl1(), daemon.saved.bios_pl1_uw),
        (Some(msr), Some(bios)) if msr > bios
    );
    if daemon.saved.pl1_sync || raised {
        daemon.sync_pl1();
    }

    // Восстановление настроек после загрузки.
    if daemon.saved.charge_limit != ChargeLimit::Off {
        daemon.reapply_charge_limit("запуск");
    }
    if let Some(mode) = daemon.saved.perf_mode {
        if let Err(e) = daemon.device.set_perf_mode(mode) {
            log(&format!("режим «{}» не восстановлен: {e}", mode.name()));
        }
    }

    let daemon = Arc::new(Mutex::new(daemon));
    {
        let d = daemon.clone();
        thread::spawn(move || watcher(d));
    }
    if !mock {
        let d = daemon.clone();
        thread::spawn(move || wmi_events(d));
    }

    let _ = fs::remove_file(&socket);
    if let Some(dir) = Path::new(&socket).parent() {
        let _ = fs::create_dir_all(dir);
    }
    let listener = UnixListener::bind(&socket).unwrap_or_else(|e| {
        log(&format!("не удалось открыть сокет {socket}: {e}"));
        std::process::exit(1);
    });
    // Клиенты работают от обычного пользователя; набор команд ограничен.
    // С --debug сокет доступен только пользователю, запустившему sudo:
    // через него можно делать произвольные ACPI-вызовы.
    let sudo_uid = std::env::var("SUDO_UID").ok().and_then(|v| v.parse().ok());
    if debug {
        let _ = std::os::unix::fs::chown(&socket, sudo_uid, None);
        let _ = fs::set_permissions(&socket, fs::Permissions::from_mode(0o600));
    } else {
        let _ = fs::set_permissions(&socket, fs::Permissions::from_mode(0o666));
    }
    log(&format!("слушаю {socket}"));

    for stream in listener.incoming().flatten() {
        let d = daemon.clone();
        thread::spawn(move || serve(stream, d));
    }
}
