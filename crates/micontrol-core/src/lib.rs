//! Управление функциями ноутбуков Xiaomi / RedmiBook в Linux.
//! Порт протокола из MIControl (https://github.com/loginsinex/micontrol).

pub mod acpi_events;
pub mod backend;
pub mod device;
pub mod ipc;
pub mod power;
pub mod protocol;
pub mod sysinfo;
