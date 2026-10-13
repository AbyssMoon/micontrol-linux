//! QObject `Controller`, через который QML общается с демоном micontrold.

use std::pin::Pin;
use std::fs;
use std::process::Command;

use cxx_qt::Threading;
use cxx_qt_lib::QString;
use micontrol_core::ipc::{self, IpcError, Request, Status};
use micontrol_core::protocol::{ChargeLimit, PerfMode};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, connected)]
        #[qproperty(QString, error_text)]
        /// Ошибка последнего действия (демон доступен, но команда не прошла).
        #[qproperty(QString, action_error)]
        #[qproperty(QString, backend)]
        /// Имя режима (`silent`, `turbo`, …) или пусто, если неизвестен.
        #[qproperty(QString, perf_mode)]
        #[qproperty(i32, perf_raw)]
        /// -1 — неизвестно, 0 — выключен, 1 — включён.
        #[qproperty(i32, mic_state)]
        /// -1 — wpctl недоступен, 0 — звук идёт, 1 — микрофон заглушён в системе.
        #[qproperty(i32, system_mic_muted)]
        /// Лимит, который сейчас стоит в прошивке (`off`, `80`, …), или пусто.
        #[qproperty(QString, charge_limit)]
        /// Лимит, который демон сохранил и поддерживает.
        #[qproperty(QString, charge_saved)]
        #[qproperty(i32, charge_raw)]
        #[qproperty(i32, battery_health)]
        #[qproperty(bool, pl1_sync)]
        /// PL1 в MSR и в DTT, Вт (-1 — неизвестно).
        #[qproperty(i32, pl1_msr)]
        #[qproperty(i32, pl1_dtt)]
        #[qproperty(i32, pl2)]
        /// Потребление пакета CPU, Вт (-1 — неизвестно).
        #[qproperty(f64, cpu_power)]
        /// Средняя частота ядер, МГц, и температура пакета, °C (-1 — неизвестно).
        #[qproperty(i32, cpu_freq)]
        #[qproperty(i32, cpu_temp)]
        #[qproperty(i32, battery_capacity)]
        /// Мощность заряда (+) или разряда (−), Вт; 0 — не идёт.
        #[qproperty(f64, battery_power)]
        /// Текущая и полная ёмкость, Вт·ч (-1 — неизвестно).
        #[qproperty(f64, battery_energy)]
        #[qproperty(f64, battery_energy_full)]
        /// Charging / Discharging / Not charging / Full.
        #[qproperty(QString, battery_status)]
        /// -1 — неизвестно, 0 — от батареи, 1 — от сети.
        #[qproperty(i32, ac_online)]
        #[qproperty(QString, device_info)]
        #[qproperty(bool, is_xiaomi)]
        #[qproperty(bool, redmibook2025)]
        type Controller = super::ControllerRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Controller>);

        #[qinvokable]
        fn set_perf(self: Pin<&mut Controller>, mode: &QString);

        #[qinvokable]
        fn set_charge(self: Pin<&mut Controller>, limit: &QString);

        #[qinvokable]
        fn set_mic(self: Pin<&mut Controller>, enabled: bool);

        #[qinvokable]
        fn request_pl1_sync(self: Pin<&mut Controller>, enabled: bool);

        /// Начать принимать запросы от повторных запусков интерфейса.
        #[qinvokable]
        fn listen_instances(self: Pin<&mut Controller>);

        /// Повторный запуск попросил показать окно.
        #[qsignal]
        fn show_requested(self: Pin<&mut Controller>);
    }

    impl cxx_qt::Threading for Controller {}
}

pub struct ControllerRust {
    connected: bool,
    error_text: QString,
    action_error: QString,
    backend: QString,
    perf_mode: QString,
    perf_raw: i32,
    mic_state: i32,
    system_mic_muted: i32,
    charge_limit: QString,
    charge_saved: QString,
    charge_raw: i32,
    battery_health: i32,
    pl1_sync: bool,
    pl1_msr: i32,
    pl1_dtt: i32,
    pl2: i32,
    cpu_power: f64,
    cpu_freq: i32,
    cpu_temp: i32,
    battery_capacity: i32,
    battery_power: f64,
    battery_energy: f64,
    battery_energy_full: f64,
    battery_status: QString,
    ac_online: i32,
    device_info: QString,
    is_xiaomi: bool,
    redmibook2025: bool,
}

impl Default for ControllerRust {
    fn default() -> Self {
        Self {
            connected: false,
            error_text: QString::default(),
            action_error: QString::default(),
            backend: QString::default(),
            perf_mode: QString::default(),
            perf_raw: -1,
            mic_state: -1,
            system_mic_muted: -1,
            charge_limit: QString::default(),
            charge_saved: QString::default(),
            charge_raw: -1,
            battery_health: -1,
            pl1_sync: false,
            pl1_msr: -1,
            pl1_dtt: -1,
            pl2: -1,
            cpu_power: -1.0,
            cpu_freq: -1,
            cpu_temp: -1,
            battery_capacity: -1,
            battery_power: 0.0,
            battery_energy: -1.0,
            battery_energy_full: -1.0,
            battery_status: QString::default(),
            ac_online: -1,
            device_info: QString::default(),
            is_xiaomi: false,
            redmibook2025: false,
        }
    }
}

const MIC_SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";

/// Состояние mute системного микрофона через PipeWire (`wpctl`).
fn system_mic_muted() -> Option<bool> {
    let out = Command::new("wpctl")
        .args(["get-volume", MIC_SOURCE])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).contains("[MUTED]"))
}

fn set_system_mic_muted(muted: bool) {
    let _ = Command::new("wpctl")
        .args(["set-mute", MIC_SOURCE, if muted { "1" } else { "0" }])
        .status();
}

#[derive(Default)]
struct PowerSupply {
    capacity: Option<i32>,
    status: Option<String>,
    ac: Option<bool>,
    /// Вт, со знаком: + заряд, − разряд.
    power: Option<f64>,
    energy: Option<f64>,
    energy_full: Option<f64>,
}

/// Батарея и сетевое питание из `/sys/class/power_supply` (root не нужен).
fn power_supply() -> PowerSupply {
    let mut ps = PowerSupply::default();
    let read = |p: std::path::PathBuf| fs::read_to_string(p).ok().map(|s| s.trim().to_string());
    let num = |p: std::path::PathBuf| read(p).and_then(|v| v.parse::<f64>().ok());
    for e in fs::read_dir("/sys/class/power_supply").into_iter().flatten().flatten() {
        let dir = e.path();
        match read(dir.join("type")).as_deref() {
            Some("Battery") if ps.capacity.is_none() => {
                ps.capacity = read(dir.join("capacity")).and_then(|v| v.parse().ok());
                ps.status = read(dir.join("status"));
                // power_now (мкВт) или current_now × voltage_now (мкА, мкВ).
                let watts = num(dir.join("power_now")).map(|v| v / 1e6).or_else(|| {
                    Some(num(dir.join("current_now"))? * num(dir.join("voltage_now"))? / 1e12)
                });
                let sign = match ps.status.as_deref() {
                    Some("Discharging") => -1.0,
                    Some("Charging") => 1.0,
                    _ => 0.0,
                };
                ps.power = watts.map(|w| w.abs() * sign);
                ps.energy = num(dir.join("energy_now")).map(|v| v / 1e6);
                ps.energy_full = num(dir.join("energy_full")).map(|v| v / 1e6);
            }
            Some("Mains") => {
                let online = read(dir.join("online")).as_deref() == Some("1");
                ps.ac = Some(ps.ac.unwrap_or(false) || online);
            }
            _ => {}
        }
    }
    ps
}

/// Средняя текущая частота ядер, МГц.
fn cpu_freq_mhz() -> Option<i32> {
    let (mut sum, mut n) = (0u64, 0u64);
    for e in fs::read_dir("/sys/devices/system/cpu").ok()?.flatten() {
        let p = e.path().join("cpufreq/scaling_cur_freq");
        if let Some(v) = fs::read_to_string(p).ok().and_then(|s| s.trim().parse::<u64>().ok()) {
            sum += v;
            n += 1;
        }
    }
    (n > 0).then(|| (sum / n / 1000) as i32)
}

/// Температура пакета CPU (`coretemp`, temp1), °C.
fn cpu_temp_c() -> Option<i32> {
    for e in fs::read_dir("/sys/class/hwmon").ok()?.flatten() {
        let dir = e.path();
        if fs::read_to_string(dir.join("name")).ok()?.trim() == "coretemp" {
            let v: i32 = fs::read_to_string(dir.join("temp1_input")).ok()?.trim().parse().ok()?;
            return Some(v / 1000);
        }
    }
    None
}

impl qobject::Controller {
    fn update_power(mut self: Pin<&mut Self>) {
        self.as_mut().set_cpu_freq(cpu_freq_mhz().unwrap_or(-1));
        self.as_mut().set_cpu_temp(cpu_temp_c().unwrap_or(-1));
        let ps = power_supply();
        self.as_mut().set_battery_capacity(ps.capacity.unwrap_or(-1));
        self.as_mut()
            .set_battery_status(QString::from(ps.status.unwrap_or_default().as_str()));
        self.as_mut().set_ac_online(ps.ac.map_or(-1, i32::from));
        self.as_mut().set_battery_power(ps.power.unwrap_or(0.0));
        self.as_mut().set_battery_energy(ps.energy.unwrap_or(-1.0));
        self.as_mut()
            .set_battery_energy_full(ps.energy_full.unwrap_or(-1.0));
    }

    /// Результат команды: ошибка демона показывается отдельно, состояние перечитывается.
    fn apply_action(mut self: Pin<&mut Self>, result: Result<Status, IpcError>) {
        match result {
            Err(IpcError::Daemon(e)) => {
                self.as_mut().set_action_error(QString::from(e.as_str()));
                let status = ipc::request(&Request::Status);
                self.apply(status);
            }
            other => {
                self.as_mut().set_action_error(QString::default());
                self.apply(other);
            }
        }
    }

    fn apply(mut self: Pin<&mut Self>, result: Result<Status, IpcError>) {
        self.as_mut().update_power();
        let s = match result {
            Ok(s) => s,
            Err(e) => {
                self.as_mut().set_connected(false);
                self.as_mut().set_error_text(QString::from(e.to_string().as_str()));
                return;
            }
        };
        self.as_mut().set_connected(true);
        self.as_mut()
            .set_error_text(QString::from(s.errors.join("\n").as_str()));
        self.as_mut().set_backend(QString::from(s.backend.as_str()));

        let perf_raw = s.perf_mode.map_or(-1, i32::from);
        let perf_name = s
            .perf_mode
            .and_then(PerfMode::from_raw)
            .map_or("", PerfMode::name);
        self.as_mut().set_perf_raw(perf_raw);
        self.as_mut().set_perf_mode(QString::from(perf_name));

        self.as_mut()
            .set_mic_state(s.mic_enabled.map_or(-1, i32::from));
        let fw = s.charge_limit_raw.and_then(ChargeLimit::from_raw);
        self.as_mut()
            .set_charge_limit(QString::from(fw.map_or("", ChargeLimit::name)));
        self.as_mut()
            .set_charge_saved(QString::from(s.charge_limit.name()));
        self.as_mut()
            .set_charge_raw(s.charge_limit_raw.map_or(-1, i32::from));
        self.as_mut()
            .set_battery_health(s.battery_health.map_or(-1, i32::from));
        self.as_mut().set_pl1_sync(s.pl1_sync);
        self.as_mut().set_pl1_msr(s.pl1_msr_w.map_or(-1, |v| v as i32));
        self.as_mut().set_pl1_dtt(s.pl1_dtt_w.map_or(-1, |v| v as i32));
        self.as_mut().set_pl2(s.pl2_w.map_or(-1, |v| v as i32));
        self.as_mut().set_cpu_power(s.cpu_power_w.unwrap_or(-1.0));

        let i = &s.sysinfo;
        let mut info = format!(
            "Производитель: {}\nМодель: {}\nSKU: {}\nBIOS: {} ({})",
            i.vendor, i.product, i.sku, i.bios_version, i.bios_date
        );
        if !i.serial.is_empty() {
            info.push_str(&format!("\nСерийный номер: {}", i.serial));
        }
        self.as_mut().set_device_info(QString::from(info.as_str()));
        self.as_mut().set_is_xiaomi(i.is_xiaomi());
        self.as_mut().set_redmibook2025(i.is_redmibook_2025());
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        let status = ipc::request(&Request::Status);

        // Fn+F4 на Linux глушит микрофон через PipeWire, а прошивка об этом
        // не знает — синхронизируем её состояние (и индикатор) с системным.
        let muted = system_mic_muted();
        self.as_mut()
            .set_system_mic_muted(muted.map_or(-1, i32::from));
        if let (Ok(st), Some(muted)) = (&status, muted) {
            if st.mic_enabled == Some(muted) {
                let synced = ipc::request(&Request::SetMic { enabled: !muted });
                return self.apply(synced);
            }
        }
        self.apply(status);
    }

    pub fn set_perf(self: Pin<&mut Self>, mode: &QString) {
        let Some(mode) = PerfMode::from_name(&mode.to_string()) else {
            return;
        };
        self.apply_action(ipc::request(&Request::SetPerfMode { mode }));
    }

    pub fn set_charge(self: Pin<&mut Self>, limit: &QString) {
        let Some(limit) = ChargeLimit::from_name(&limit.to_string()) else {
            return;
        };
        self.apply_action(ipc::request(&Request::SetChargeLimit { limit }));
    }

    pub fn set_mic(mut self: Pin<&mut Self>, enabled: bool) {
        if system_mic_muted().is_some() {
            set_system_mic_muted(!enabled);
            self.as_mut().set_system_mic_muted(i32::from(!enabled));
        }
        self.apply_action(ipc::request(&Request::SetMic { enabled }));
    }

    pub fn request_pl1_sync(self: Pin<&mut Self>, enabled: bool) {
        self.apply_action(ipc::request(&Request::SetPl1Sync { enabled }));
    }

    pub fn listen_instances(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        crate::single_instance::listen(move || {
            let _ = thread.queue(|ctl| ctl.show_requested());
        });
    }
}
