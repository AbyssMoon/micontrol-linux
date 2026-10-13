//! Лимит мощности CPU (PL1) через powercap.
//!
//! Режим производительности прошивка передаёт в Intel DTT (`ODVP1`), и
//! `thermald --adaptive` ставит PL1 из таблиц OEM — но только в MMIO-домен
//! (`intel-rapl-mmio`). В MSR-домене (`intel-rapl`) остаётся значение BIOS
//! (28 Вт на RedmiBook Pro 2025), а действует меньший из двух лимитов, поэтому
//! без синхронизации режимы почти не влияют на производительность.

use std::fs;

const MSR_PL1: &str = "/sys/class/powercap/intel-rapl:0/constraint_0_power_limit_uw";
const MMIO_PL1: &str = "/sys/class/powercap/intel-rapl-mmio:0/constraint_0_power_limit_uw";

fn read_uw(path: &str) -> Option<u64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

/// PL1 в MSR-домене (микроватты).
pub fn msr_pl1() -> Option<u64> {
    read_uw(MSR_PL1)
}

/// PL1, выставленный DTT в MMIO-домене (микроватты).
pub fn dtt_pl1() -> Option<u64> {
    read_uw(MMIO_PL1)
}

pub fn set_msr_pl1(uw: u64) -> std::io::Result<()> {
    fs::write(MSR_PL1, uw.to_string())
}

const MSR_PL2: &str = "/sys/class/powercap/intel-rapl:0/constraint_1_power_limit_uw";
const MSR_ENERGY: &str = "/sys/class/powercap/intel-rapl:0/energy_uj";
const MSR_ENERGY_MAX: &str = "/sys/class/powercap/intel-rapl:0/max_energy_range_uj";

/// Кратковременный лимит PL2 (микроватты).
pub fn msr_pl2() -> Option<u64> {
    read_uw(MSR_PL2)
}

/// Счётчик энергии пакета CPU (мкДж; читается только от root).
pub fn package_energy_uj() -> Option<u64> {
    read_uw(MSR_ENERGY)
}

/// Средняя мощность между двумя замерами счётчика, ватты.
pub fn average_power_w(prev: (u64, std::time::Instant), now: (u64, std::time::Instant)) -> Option<f64> {
    let dt = now.1.duration_since(prev.1).as_secs_f64();
    if dt < 0.1 {
        return None;
    }
    let de = if now.0 >= prev.0 {
        now.0 - prev.0
    } else {
        // Переполнение счётчика.
        read_uw(MSR_ENERGY_MAX)? - prev.0 + now.0
    };
    Some(de as f64 / 1e6 / dt)
}
