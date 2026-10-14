//! Протокол между демоном (root) и клиентами (GUI, CLI):
//! одна JSON-строка запроса — одна JSON-строка ответа через Unix-сокет.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::protocol::{ChargeLimit, PerfMode};
use crate::sysinfo::SysInfo;

pub const DEFAULT_SOCKET: &str = "/run/micontrol.sock";

/// Путь к сокету: `$MICONTROL_SOCKET` или системный по умолчанию.
pub fn socket_path() -> String {
    std::env::var("MICONTROL_SOCKET").unwrap_or_else(|_| DEFAULT_SOCKET.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    SetPerfMode { mode: PerfMode },
    SetMic { enabled: bool },
    SetChargeLimit { limit: ChargeLimit },
    /// Повторно применить сохранённый лимит заряда.
    Reapply,
    /// Копировать PL1 из DTT в MSR (см. [`crate::power`]).
    SetPl1Sync { enabled: bool },
    SetTouchpad { enabled: bool },
    /// Произвольный вызов `WMAA` (только если демон запущен с `--debug`).
    Raw { set: bool, func: u8, arg: u8, arg2: u8 },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Status {
    pub backend: String,
    /// Сырое значение режима из прошивки (см. [`PerfMode::from_raw`]).
    pub perf_mode: Option<u8>,
    pub mic_enabled: Option<bool>,
    /// Значение, которое сейчас сообщает прошивка.
    pub charge_limit_raw: Option<u8>,
    /// Значение, которое демон поддерживает (сохранено на диск).
    pub charge_limit: ChargeLimit,
    /// Здоровье батареи в процентах, если прошивка его сообщает.
    #[serde(default)]
    pub battery_health: Option<u8>,
    /// Синхронизация PL1 включена.
    #[serde(default)]
    pub pl1_sync: bool,
    /// PL1 в MSR и в DTT (MMIO), ватты.
    #[serde(default)]
    pub pl1_msr_w: Option<u32>,
    #[serde(default)]
    pub pl1_dtt_w: Option<u32>,
    /// Кратковременный лимит PL2, ватты.
    #[serde(default)]
    pub pl2_w: Option<u32>,
    /// Потребление пакета CPU за последние ~2 с, ватты.
    #[serde(default)]
    pub cpu_power_w: Option<f64>,
    /// `None` — тачпад не найден.
    #[serde(default)]
    pub touchpad_enabled: Option<bool>,
    pub errors: Vec<String>,
    pub sysinfo: SysInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    Ok { status: Status },
    Raw { reply: Vec<u8> },
    Error { message: String },
}

#[derive(Debug, Clone)]
pub enum IpcError {
    /// Демон недоступен.
    Connect(String),
    /// Демон ответил ошибкой (связь есть).
    Daemon(String),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpcError::Connect(e) | IpcError::Daemon(e) => f.write_str(e),
        }
    }
}

pub fn request(req: &Request) -> Result<Status, IpcError> {
    match send(req)? {
        Response::Ok { status } => Ok(status),
        Response::Raw { .. } => Err(IpcError::Daemon("неожиданный ответ демона".into())),
        Response::Error { message } => Err(IpcError::Daemon(message)),
    }
}

/// Сырой ответ прошивки через демон (`--debug`).
pub fn raw(set: bool, cmd: u8, arg: u8, arg2: u8) -> Result<Vec<u8>, IpcError> {
    match send(&Request::Raw { set, func: cmd, arg, arg2 })? {
        Response::Raw { reply } => Ok(reply),
        Response::Ok { .. } => Err(IpcError::Daemon("неожиданный ответ демона".into())),
        Response::Error { message } => Err(IpcError::Daemon(message)),
    }
}

fn send(req: &Request) -> Result<Response, IpcError> {
    let path = socket_path();
    let conn = |e: std::io::Error| IpcError::Connect(format!("нет связи с демоном micontrold ({path}): {e}"));
    let stream = UnixStream::connect(&path).map_err(conn)?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut writer = stream.try_clone().map_err(conn)?;
    let mut line = serde_json::to_string(req).map_err(|e| IpcError::Daemon(e.to_string()))?;
    line.push('\n');
    writer.write_all(line.as_bytes()).map_err(conn)?;

    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).map_err(conn)?;
    serde_json::from_str::<Response>(&reply)
        .map_err(|e| IpcError::Connect(format!("непонятный ответ демона: {e}: {reply}")))
}
