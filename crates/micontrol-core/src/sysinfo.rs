//! Сведения о системе из DMI (аналог страницы «System info» оригинала).

use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SysInfo {
    pub vendor: String,
    pub product: String,
    pub sku: String,
    pub bios_version: String,
    pub bios_date: String,
    /// Читается только от root, поэтому заполняется демоном.
    pub serial: String,
}

fn dmi(name: &str) -> String {
    fs::read_to_string(format!("/sys/class/dmi/id/{name}"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

impl SysInfo {
    pub fn read() -> Self {
        Self {
            vendor: dmi("sys_vendor"),
            product: dmi("product_name"),
            sku: dmi("product_sku"),
            bios_version: dmi("bios_version"),
            bios_date: dmi("bios_date"),
            serial: dmi("product_serial"),
        }
    }

    pub fn is_xiaomi(&self) -> bool {
        let v = self.vendor.to_lowercase();
        v.contains("xiaomi") || v.contains("timi")
    }

    /// RedmiBook 2025 (SKU `TM24*`): нет режима «баланс», есть «авто».
    pub fn is_redmibook_2025(&self) -> bool {
        self.sku.starts_with("TM24")
    }
}
