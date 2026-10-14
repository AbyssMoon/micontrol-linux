//! Высокоуровневые операции поверх [`Backend`].

use std::thread;
use std::time::Duration;

use crate::backend::{reply_arg, reply_arg2, Backend, Error, Result};
use crate::protocol::{
    ChargeLimit, Cmd, Op, PerfMode, BATTERY_HEALTH_SUBCMD, CHARGE_SUBCMD, MIC_SUBCMD,
};

pub struct Device {
    backend: Box<dyn Backend>,
}

impl Device {
    pub fn new(backend: Box<dyn Backend>) -> Self {
        Self { backend }
    }

    pub fn describe(&self) -> String {
        self.backend.describe()
    }

    pub fn raw(&mut self, op: Op, cmd: u8, arg: u8, arg2: u8) -> Result<Vec<u8>> {
        self.backend.exec(op, cmd, arg, arg2)
    }

    /// Вызов без проверки статуса — для отладки.
    pub fn call_raw(&mut self, op: Op, cmd: u8, arg: u8, arg2: u8) -> Result<Vec<u8>> {
        self.backend.call(&crate::protocol::build_request(op, cmd, arg, arg2))
    }

    /// Сырое значение режима (может быть неизвестным для [`PerfMode`]).
    pub fn perf_mode(&mut self) -> Result<u8> {
        reply_arg(&self.backend.exec(Op::Get, Cmd::PerfMode as u8, 0, 0)?)
    }

    /// Ставит режим и перечитывает его: прошивка отвечает «успех» и на
    /// значения, которые не поддерживает (например, 0x01 на RedmiBook 2025).
    pub fn set_perf_mode(&mut self, mode: PerfMode) -> Result<()> {
        self.backend.exec(Op::Set, Cmd::PerfMode as u8, mode.raw(), 0)?;
        let now = self.perf_mode()?;
        if now != mode.raw() {
            return Err(Error::Acpi(format!(
                "режим «{}» не поддерживается этой моделью (прошивка осталась в 0x{now:02X})",
                mode.name()
            )));
        }
        Ok(())
    }

    /// Номер последнего WMI-события (см. [`crate::protocol::EVENT_FN_K`]).
    pub fn last_event(&mut self) -> Result<u8> {
        self.backend.last_event()
    }

    pub fn mic_enabled(&mut self) -> Result<bool> {
        Ok(reply_arg2(&self.backend.exec(Op::Get, Cmd::Mic as u8, MIC_SUBCMD, 0)?)? != 0)
    }

    pub fn set_mic_enabled(&mut self, enabled: bool) -> Result<()> {
        self.backend
            .exec(Op::Set, Cmd::Mic as u8, MIC_SUBCMD, enabled as u8)?;
        Ok(())
    }

    /// Здоровье батареи в процентах (прошивка RedmiBook 2025).
    pub fn battery_health(&mut self) -> Result<u8> {
        reply_arg2(&self.backend.exec(Op::Get, Cmd::ChargeProtect as u8, BATTERY_HEALTH_SUBCMD, 0)?)
    }

    pub fn charge_limit(&mut self) -> Result<u8> {
        reply_arg2(&self.backend.exec(Op::Get, Cmd::ChargeProtect as u8, CHARGE_SUBCMD, 0)?)
    }

    /// Ставит лимит так же, как `CPowerPowerChargeProtectGuard::UpdateStatus`:
    /// сначала сброс в 0, затем через 50 мс нужное значение. Прошивка
    /// забывает лимит после сна и смены источника питания, поэтому
    /// демон вызывает это повторно.
    pub fn apply_charge_limit(&mut self, limit: ChargeLimit) -> Result<()> {
        let cmd = Cmd::ChargeProtect as u8;
        self.backend.exec(Op::Set, cmd, CHARGE_SUBCMD, 0)?;
        if limit != ChargeLimit::Off {
            thread::sleep(Duration::from_millis(50));
            self.backend.exec(Op::Set, cmd, CHARGE_SUBCMD, limit.raw())?;
        }
        let now = self.charge_limit()?;
        if now != limit.raw() {
            return Err(Error::Acpi(format!(
                "лимит «{}» не применился (прошивка сообщает 0x{now:02X})",
                limit.name()
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Mock;

    #[test]
    fn unsupported_mode_is_reported() {
        let mut d = Device::new(Box::new(Mock::default()));
        assert!(d.set_perf_mode(PerfMode::Turbo).is_ok());
        assert!(d.set_perf_mode(PerfMode::Balance).is_err());
        assert_eq!(d.perf_mode().unwrap(), PerfMode::Turbo.raw());
    }

    #[test]
    fn charge_limit_roundtrip() {
        let mut d = Device::new(Box::new(Mock::default()));
        for c in ChargeLimit::ALL {
            d.apply_charge_limit(c).unwrap();
            assert_eq!(d.charge_limit().unwrap(), c.raw());
        }
    }
}
