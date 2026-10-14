//! Тачпад: отключение через `inhibited` устройства ввода в sysfs (нужен root).
//! Работает в любом окружении. Клавиши у тачпада нет: Fn+Пробел клавиатура
//! отдаёт как обычный пробел.

use std::fs;
use std::io;
use std::path::PathBuf;

/// Узлы ввода тачпада: сам `… Touchpad` и соседние узлы того же HID-устройства
/// (например, `… Mouse` — мышиный режим той же панели).
pub fn devices() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir("/sys/class/input") else { return Vec::new() };
    let inputs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("inhibited").exists())
        .collect();
    let parent = |p: &PathBuf| fs::canonicalize(p.join("device")).ok();
    let pads: Vec<PathBuf> = inputs
        .iter()
        .filter(|p| {
            fs::read_to_string(p.join("name")).is_ok_and(|n| n.trim_end().ends_with("Touchpad"))
        })
        .filter_map(parent)
        .collect();
    let mut out: Vec<PathBuf> = inputs
        .into_iter()
        .filter(|p| parent(p).is_some_and(|d| pads.contains(&d)))
        .collect();
    out.sort();
    out
}

/// `None` — тачпада нет.
pub fn enabled() -> Option<bool> {
    let devs = devices();
    if devs.is_empty() {
        return None;
    }
    Some(
        devs.iter()
            .all(|d| fs::read_to_string(d.join("inhibited")).is_ok_and(|v| v.trim() == "0")),
    )
}

pub fn set_enabled(enabled: bool) -> io::Result<()> {
    let devs = devices();
    if devs.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "тачпад не найден"));
    }
    for d in devs {
        fs::write(d.join("inhibited"), if enabled { "0" } else { "1" })?;
    }
    Ok(())
}
