//! Низкоуровневый протокол Xiaomi `MiCommonInterface` (WMI-метод `MiInterface`,
//! он же ACPI-метод `\_SB.PC00.WMID.WMAA`).
//!
//! Формат входного буфера (32 байта), см. `CMiLowLevelCommand.cpp` оригинала:
//!
//! ```text
//! { 0x00, OP, 0x00, CMD, ARG, 0x00, ARG2, 0x00, 0x00 ... }
//!   OP: 0xFB — записать, 0xFA — прочитать
//! ```
//!
//! Ответ повторяет раскладку запроса: значение для `ARG` возвращается в байте 4,
//! для `ARG2` — в байте 6. Байты 0–1 — код результата (`SGER` в `WMAA`):
//! байт 1 = `0x80` — выполнено, `0xE0` — функция не поддерживается.

use serde::{Deserialize, Serialize};

pub const BUF_LEN: usize = 32;

/// Индекс байта ответа, в котором прошивка возвращает значение `ARG`.
pub const REPLY_ARG: usize = 4;
/// Индекс байта ответа, в котором прошивка возвращает значение `ARG2`.
pub const REPLY_ARG2: usize = 6;
/// Индекс байта ответа со статусом вызова.
pub const REPLY_STATUS: usize = 1;
pub const STATUS_OK: u8 = 0x80;
pub const STATUS_UNSUPPORTED: u8 = 0xE0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Op {
    Set = 0xFB,
    Get = 0xFA,
}

/// Команды, известные по оригинальному MIControl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cmd {
    /// Режим производительности (`llcPowerBalance`).
    PerfMode = 0x08,
    /// Состояние микрофона в прошивке (`llcMicrophone`), подкоманда 0x05:
    /// 1 — включён, 0 — выключен (прошивка зажигает индикатор mute).
    Mic = 0x0A,
    /// Защита батареи / лимит заряда (`llcChargeProtect`), подкоманда 0x02.
    ChargeProtect = 0x10,
}

pub const MIC_SUBCMD: u8 = 0x05;
pub const CHARGE_SUBCMD: u8 = 0x02;
/// Подкоманда 0x10/0x01: здоровье батареи в процентах (поле EC `SOH1`).
pub const BATTERY_HEALTH_SUBCMD: u8 = 0x01;

/// WMI-событие Fn+K на RedmiBook 2025 (`_Q28`). Режим прошивка не меняет —
/// переключать по кругу должен драйвер (на Windows — программа Xiaomi).
pub const EVENT_FN_K: u8 = 0x28;

pub fn build_request(op: Op, cmd: u8, arg: u8, arg2: u8) -> [u8; BUF_LEN] {
    let mut buf = [0u8; BUF_LEN];
    buf[1] = op as u8;
    buf[3] = cmd;
    buf[4] = arg;
    buf[6] = arg2;
    buf
}

/// Режимы производительности (значения из `CPagePerfomanceDlg.cpp`).
///
/// В прошивке RedmiBook 2025 (SSDT `XMCC1806`, метод `WMAA`) запись понимает
/// 0x02–0x04 и 0x09 (0x05–0x08 и 0x0A — служебные), значения 0x01 там нет:
/// вызов возвращает «успех», но ничего не меняет.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfMode {
    Balance,
    Silent,
    Turbo,
    FullSpeed,
    /// «Умный» режим, есть только на RedmiBook 2025 (SKU `TM24*`).
    Auto,
}

impl PerfMode {
    pub const ALL: [PerfMode; 5] = [
        PerfMode::Silent,
        PerfMode::Balance,
        PerfMode::Turbo,
        PerfMode::FullSpeed,
        PerfMode::Auto,
    ];

    pub fn raw(self) -> u8 {
        match self {
            PerfMode::Balance => 0x01,
            PerfMode::Silent => 0x02,
            PerfMode::Turbo => 0x03,
            PerfMode::FullSpeed => 0x04,
            PerfMode::Auto => 0x09,
        }
    }

    pub fn from_raw(v: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.raw() == v)
    }

    pub fn name(self) -> &'static str {
        match self {
            PerfMode::Balance => "balance",
            PerfMode::Silent => "silent",
            PerfMode::Turbo => "turbo",
            PerfMode::FullSpeed => "full_speed",
            PerfMode::Auto => "auto",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }

    /// Следующий режим по Fn+K; из неизвестного — тихий.
    pub fn next_fn_k(current: u8, redmibook2025: bool) -> Self {
        let cycle = if redmibook2025 {
            [PerfMode::Silent, PerfMode::Auto, PerfMode::Turbo, PerfMode::FullSpeed]
        } else {
            [PerfMode::Silent, PerfMode::Balance, PerfMode::Turbo, PerfMode::FullSpeed]
        };
        cycle
            .iter()
            .position(|m| m.raw() == current)
            .map_or(cycle[0], |i| cycle[(i + 1) % cycle.len()])
    }
}

/// Лимит заряда. Значения из метода `WMAA` прошивки RedmiBook 2025
/// (поле EC `HBDA` — порог в процентах): 0x01 = 80%, 0x04 = 90%, 0x05–0x08 = 70–40%.
/// В MIControl для старых моделей есть только «выкл / вкл» (0 / 1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeLimit {
    #[default]
    Off,
    P90,
    /// Значение 0x01: «защита» в MIControl; 80% на RedmiBook 2025.
    #[serde(alias = "default")]
    P80,
    P70,
    P60,
    P50,
    P40,
}

impl ChargeLimit {
    pub const ALL: [ChargeLimit; 7] = [
        ChargeLimit::Off,
        ChargeLimit::P90,
        ChargeLimit::P80,
        ChargeLimit::P70,
        ChargeLimit::P60,
        ChargeLimit::P50,
        ChargeLimit::P40,
    ];

    pub fn raw(self) -> u8 {
        match self {
            ChargeLimit::Off => 0x00,
            ChargeLimit::P80 => 0x01,
            ChargeLimit::P90 => 0x04,
            ChargeLimit::P70 => 0x05,
            ChargeLimit::P60 => 0x06,
            ChargeLimit::P50 => 0x07,
            ChargeLimit::P40 => 0x08,
        }
    }

    pub fn from_raw(v: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.raw() == v)
    }

    pub fn name(self) -> &'static str {
        match self {
            ChargeLimit::Off => "off",
            ChargeLimit::P90 => "90",
            ChargeLimit::P80 => "80",
            ChargeLimit::P70 => "70",
            ChargeLimit::P60 => "60",
            ChargeLimit::P50 => "50",
            ChargeLimit::P40 => "40",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "on" => Some(ChargeLimit::P80),
            "100" | "disable" => Some(ChargeLimit::Off),
            _ => Self::ALL.into_iter().find(|m| m.name() == s),
        }
    }
}

pub fn hex_dump(data: &[u8]) -> String {
    data.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_layout_matches_original() {
        let buf = build_request(Op::Set, Cmd::ChargeProtect as u8, CHARGE_SUBCMD, 1);
        assert_eq!(&buf[..8], &[0x00, 0xFB, 0x00, 0x10, 0x02, 0x00, 0x01, 0x00]);
        assert!(buf[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn roundtrip_names() {
        for m in PerfMode::ALL {
            assert_eq!(PerfMode::from_raw(m.raw()), Some(m));
            assert_eq!(PerfMode::from_name(m.name()), Some(m));
        }
        for c in ChargeLimit::ALL {
            assert_eq!(ChargeLimit::from_raw(c.raw()), Some(c));
            assert_eq!(ChargeLimit::from_name(c.name()), Some(c));
        }
    }

    #[test]
    fn fn_k_cycles_modes() {
        let mut m = PerfMode::Silent;
        let mut seen = vec![];
        for _ in 0..4 {
            m = PerfMode::next_fn_k(m.raw(), true);
            seen.push(m);
        }
        assert_eq!(seen, [PerfMode::Auto, PerfMode::Turbo, PerfMode::FullSpeed, PerfMode::Silent]);
        assert_eq!(PerfMode::next_fn_k(0x0A, true), PerfMode::Silent);
        assert_eq!(PerfMode::next_fn_k(0x01, false), PerfMode::Turbo);
    }

    #[test]
    fn old_state_name_still_parses() {
        let c: ChargeLimit = serde_json::from_str("\"default\"").unwrap();
        assert_eq!(c, ChargeLimit::P80);
    }
}
