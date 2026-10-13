//! Один экземпляр интерфейса на пользователя.
//!
//! Первый экземпляр слушает сокет в `$XDG_RUNTIME_DIR`; повторный запуск
//! (ярлык в меню, когда интерфейс уже висит в трее) просит его показать окно
//! и завершается.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Mutex;

/// Слушающий сокет первого экземпляра, пока его не забрал `listen`.
static LISTENER: Mutex<Option<UnixListener>> = Mutex::new(None);
/// Путь к сокету, если он наш (удаляется при выходе).
static OWNED: Mutex<Option<PathBuf>> = Mutex::new(None);

pub enum Instance {
    /// Это первый экземпляр — запускаемся.
    Primary,
    /// Уже запущен другой экземпляр; если просили, ему отправлен `show`.
    Secondary,
}

/// `show` — попросить уже запущенный экземпляр показать окно.
pub fn acquire(show: bool) -> Instance {
    // Без $XDG_RUNTIME_DIR (не графический сеанс) работаем без защиты.
    let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") else { return Instance::Primary };
    let path = PathBuf::from(dir).join("micontrol-gui.sock");
    for _ in 0..2 {
        match UnixListener::bind(&path) {
            Ok(l) => {
                *LISTENER.lock().unwrap() = Some(l);
                *OWNED.lock().unwrap() = Some(path);
                return Instance::Primary;
            }
            Err(e) if e.kind() == ErrorKind::AddrInUse => match UnixStream::connect(&path) {
                Ok(mut s) => {
                    if show {
                        let _ = s.write_all(b"show\n");
                    }
                    return Instance::Secondary;
                }
                // Сокет остался от упавшего экземпляра.
                Err(_) => {
                    let _ = std::fs::remove_file(&path);
                }
            },
            Err(_) => return Instance::Primary,
        }
    }
    Instance::Primary
}

/// Ждать запросов от повторных запусков; `on_show` вызывается в фоновом потоке.
pub fn listen(on_show: impl Fn() + Send + 'static) {
    let Some(listener) = LISTENER.lock().unwrap().take() else { return };
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = String::new();
            let _ = stream.read_to_string(&mut buf);
            if buf.trim() == "show" {
                on_show();
            }
        }
    });
}

/// Удалить свой сокет при выходе.
pub fn release() {
    if let Some(path) = OWNED.lock().unwrap().take() {
        let _ = std::fs::remove_file(path);
    }
}
