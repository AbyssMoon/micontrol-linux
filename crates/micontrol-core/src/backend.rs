//! Транспорт до прошивки. В Linux нет драйвера для `MiCommonInterface`,
//! поэтому ACPI-метод `WMAA` вызывается напрямую через модуль `acpi_call`.

use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::protocol::{
    self, Op, BUF_LEN, REPLY_ARG, REPLY_ARG2, REPLY_STATUS, STATUS_OK, STATUS_UNSUPPORTED,
};

pub const ACPI_CALL_PATH: &str = "/proc/acpi/call";
pub const DEFAULT_METHOD: &str = r"\_SB.PC00.WMID.WMAA";
/// Больше `BUFFER_SIZE` модуля acpi_call (4 КиБ).
const ACPI_CALL_READ_BUF: usize = 8192;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Acpi(String),
    BadReply(String),
    /// Прошивка вернула не `0x80` в байте статуса.
    Rejected(u8),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "ошибка ввода-вывода: {e}"),
            Error::Acpi(e) => write!(f, "ошибка ACPI: {e}"),
            Error::BadReply(e) => write!(f, "непонятный ответ прошивки: {e}"),
            Error::Rejected(STATUS_UNSUPPORTED) => {
                write!(f, "прошивка не поддерживает эту команду (статус 0xE0)")
            }
            Error::Rejected(s) => write!(f, "прошивка отклонила команду (статус 0x{s:02X})"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub trait Backend: Send {
    /// Выполняет один вызов `WMAA` и возвращает выходной буфер.
    fn call(&mut self, input: &[u8; BUF_LEN]) -> Result<Vec<u8>>;
    fn describe(&self) -> String;

    /// Номер последнего WMI-события прошивки (поле `EVFN` устройства `WMID`).
    fn last_event(&mut self) -> Result<u8> {
        Err(Error::Acpi("события прошивки недоступны".into()))
    }

    /// Вызов с проверкой статуса ответа.
    fn exec(&mut self, op: Op, cmd: u8, arg: u8, arg2: u8) -> Result<Vec<u8>> {
        let reply = self.call(&protocol::build_request(op, cmd, arg, arg2))?;
        match reply_byte(&reply, REPLY_STATUS)? {
            STATUS_OK => Ok(reply),
            s => Err(Error::Rejected(s)),
        }
    }
}

/// Значение из ответа прошивки, с проверкой длины.
pub fn reply_byte(reply: &[u8], idx: usize) -> Result<u8> {
    reply.get(idx).copied().ok_or_else(|| {
        Error::BadReply(format!(
            "ожидалось не меньше {} байт, получено {}: [{}]",
            idx + 1,
            reply.len(),
            protocol::hex_dump(reply)
        ))
    })
}

pub fn reply_arg(reply: &[u8]) -> Result<u8> {
    reply_byte(reply, REPLY_ARG)
}

pub fn reply_arg2(reply: &[u8]) -> Result<u8> {
    reply_byte(reply, REPLY_ARG2)
}

/// Вызов через `/proc/acpi/call` (пакет `acpi_call` / `acpi_call-dkms`).
pub struct AcpiCall {
    method: String,
}

impl AcpiCall {
    pub fn new(method: Option<String>) -> Result<Self> {
        if !Path::new(ACPI_CALL_PATH).exists() {
            return Err(Error::Acpi(format!(
                "{ACPI_CALL_PATH} не найден — загрузите модуль acpi_call (modprobe acpi_call)"
            )));
        }
        let method = method
            .or_else(detect_wmid_method)
            .unwrap_or_else(|| DEFAULT_METHOD.to_string());
        Ok(Self { method })
    }

    pub fn format_call(method: &str, input: &[u8]) -> String {
        let bytes = input
            .iter()
            .map(|b| format!("0x{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ");
        // Аргументы WMAA: instance = 0, method id = 1, входной буфер.
        format!("{method} 0x0 0x1 {{{bytes}}}")
    }

    /// Одна строка в `/proc/acpi/call` и сырой ответ.
    fn eval(&self, line: &str) -> Result<String> {
        // acpi_call хранит результат последнего вызова глобально, поэтому
        // запись и чтение должны идти из одного места (демон сериализует вызовы).
        let mut f = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(ACPI_CALL_PATH)?;
        f.write_all(line.as_bytes())?;
        drop(f);

        // acpi_call отдаёт результат только целиком: если буфер чтения меньше
        // результата, read() возвращает 0. `read_to_string` начинает с пробного
        // чтения в 32 байта и получает пустую строку, поэтому читаем одним куском.
        let mut buf = vec![0u8; ACPI_CALL_READ_BUF];
        let n = fs::File::open(ACPI_CALL_PATH)?.read(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf[..n]).into_owned())
    }
}

impl Backend for AcpiCall {
    fn call(&mut self, input: &[u8; BUF_LEN]) -> Result<Vec<u8>> {
        parse_acpi_call_reply(&self.eval(&Self::format_call(&self.method, input))?)
    }

    fn last_event(&mut self) -> Result<u8> {
        // `QV20` кладёт тип и номер события в поля `EVTP`/`EVFN` рядом с `WMAA`.
        let dev = self.method.rsplit_once('.').map_or("", |(d, _)| d);
        let v = parse_acpi_call_int(&self.eval(&format!("{dev}.EVFN"))?)?;
        u8::try_from(v).map_err(|_| Error::BadReply(format!("EVFN = {v:#x}")))
    }

    fn describe(&self) -> String {
        format!("acpi_call: {}", self.method)
    }
}

/// Разбирает вывод `/proc/acpi/call`: `{0x00, 0xfa, ...}`, `0x0` или `Error: ...`.
pub fn parse_acpi_call_reply(out: &str) -> Result<Vec<u8>> {
    let out = out.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if out.starts_with("Error") || out == "not called" {
        return Err(Error::Acpi(out.to_string()));
    }
    if out.is_empty() {
        return Err(Error::Acpi("пустой ответ /proc/acpi/call".into()));
    }
    let Some(body) = out.strip_prefix('{') else {
        // Метод вернул не буфер (например, целое) — данных нет.
        return Ok(Vec::new());
    };
    let body = body.trim_end_matches('}');
    body.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| {
            let t = t.trim_start_matches("0x").trim_start_matches("0X");
            u8::from_str_radix(t, 16).map_err(|_| Error::BadReply(out.to_string()))
        })
        .collect()
}

/// Разбирает целое из `/proc/acpi/call` (`0x28`).
pub fn parse_acpi_call_int(out: &str) -> Result<u64> {
    let out = out.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if out.starts_with("Error") || out == "not called" {
        return Err(Error::Acpi(out.to_string()));
    }
    out.strip_prefix("0x")
        .and_then(|h| u64::from_str_radix(h, 16).ok())
        .ok_or_else(|| Error::BadReply(out.to_string()))
}

/// Ищет ACPI-устройство WMI (`PNP0C14`) с именем `WMID` и возвращает путь к `WMAA`.
/// На разных платформах корень может быть `PC00`, `PCI0` и т. п.
pub fn detect_wmid_method() -> Option<String> {
    let entries = fs::read_dir("/sys/bus/acpi/devices").ok()?;
    for e in entries.flatten() {
        let dir = e.path();
        let hid = fs::read_to_string(dir.join("hid")).unwrap_or_default();
        if hid.trim() != "PNP0C14" {
            continue;
        }
        let Ok(path) = fs::read_to_string(dir.join("path")) else {
            continue;
        };
        let path = path.trim();
        if path.ends_with(".WMID") {
            return Some(format!("{path}.WMAA"));
        }
    }
    None
}

/// Эмуляция прошивки — для разработки на машине без ноутбука Xiaomi.
pub struct Mock {
    regs: std::collections::HashMap<(u8, u8), u8>,
    perf: u8,
}

impl Default for Mock {
    fn default() -> Self {
        Self {
            regs: Default::default(),
            perf: protocol::PerfMode::Auto.raw(),
        }
    }
}

impl Backend for Mock {
    /// Ведёт себя как `WMAA` RedmiBook 2025: неизвестные функции — статус 0xE0,
    /// режим 0x01 и неизвестные лимиты запись молча игнорирует.
    fn call(&mut self, input: &[u8; BUF_LEN]) -> Result<Vec<u8>> {
        let (op, cmd, arg, arg2) = (input[1], input[3], input[4], input[6]);
        let mut reply = vec![0u8; BUF_LEN];
        reply[REPLY_STATUS] = STATUS_OK;
        reply[3] = cmd;
        reply[REPLY_ARG] = arg;
        let is_set = op == Op::Set as u8;
        match cmd {
            c if c == protocol::Cmd::PerfMode as u8 => {
                if is_set && matches!(arg, 0x02..=0x04 | 0x09) {
                    self.perf = arg;
                }
                reply[REPLY_ARG] = self.perf;
            }
            c if c == protocol::Cmd::Mic as u8 || c == protocol::Cmd::ChargeProtect as u8 => {
                let reg = self.regs.entry((cmd, arg)).or_insert(match (cmd, arg) {
                    (0x0A, 0x05) => 1,
                    (0x10, 0x01) => 97,
                    _ => 0,
                });
                if is_set {
                    if cmd == protocol::Cmd::ChargeProtect as u8
                        && protocol::ChargeLimit::from_raw(arg2).is_none()
                    {
                        reply[REPLY_STATUS] = 0;
                        return Ok(reply);
                    }
                    *reg = arg2;
                }
                reply[REPLY_ARG2] = *reg;
            }
            _ => {
                reply = vec![0u8; BUF_LEN];
                reply[REPLY_STATUS] = STATUS_UNSUPPORTED;
            }
        }
        Ok(reply)
    }

    fn describe(&self) -> String {
        "mock (эмуляция)".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_matches_wiki() {
        let buf = protocol::build_request(Op::Set, 0x10, 0x02, 0x05);
        let s = AcpiCall::format_call(DEFAULT_METHOD, &buf);
        assert!(s.starts_with(r"\_SB.PC00.WMID.WMAA 0x0 0x1 {0x00 0xfb 0x00 0x10 0x02 0x00 0x05 0x00 0x00"));
        assert!(s.ends_with("0x00}"));
    }

    #[test]
    fn parse_replies() {
        assert_eq!(
            parse_acpi_call_reply("{0x00, 0xfa, 0x10}\0").unwrap(),
            vec![0x00, 0xfa, 0x10]
        );
        assert!(parse_acpi_call_reply("0x0").unwrap().is_empty());
        assert!(parse_acpi_call_reply("Error: AE_NOT_FOUND").is_err());
    }

    #[test]
    fn parse_int_replies() {
        assert_eq!(parse_acpi_call_int("0x28\0").unwrap(), 0x28);
        assert!(parse_acpi_call_int("Error: AE_NOT_FOUND").is_err());
        assert!(parse_acpi_call_int("{0x01}").is_err());
    }

    #[test]
    fn status_byte_is_checked() {
        let mut m = Mock::default();
        assert!(matches!(
            m.exec(Op::Get, 0x0D, 0, 0),
            Err(Error::Rejected(STATUS_UNSUPPORTED))
        ));
        assert!(m.exec(Op::Get, 0x08, 0, 0).is_ok());
    }
}
