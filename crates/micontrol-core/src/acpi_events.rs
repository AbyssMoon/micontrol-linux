//! События ACPI из ядра: generic netlink, семейство `acpi_event`
//! (то же, что слушает `acpid`). WMI-уведомления приходят как
//! `wmi PNP0C14:00 0x20` — без данных, номер события читается из прошивки.

use std::io;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const FAMILY: &[u8] = b"acpi_event\0";
const GROUP: &[u8] = b"acpi_mc_group";
/// `ACPI_GENL_ATTR_EVENT`.
const ATTR_EVENT: u16 = 1;
const NLMSG_HDR: usize = 16;
const GENL_HDR: usize = 4;

pub struct AcpiEvent {
    pub device_class: String,
    pub bus_id: String,
    pub kind: u32,
}

pub struct Listener {
    fd: OwnedFd,
}

/// Атрибуты netlink: (тип, данные).
fn attrs(mut b: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    while b.len() >= 4 {
        let len = u16::from_ne_bytes([b[0], b[1]]) as usize;
        let typ = u16::from_ne_bytes([b[2], b[3]]) & 0x3fff;
        if len < 4 || len > b.len() {
            break;
        }
        out.push((typ, &b[4..len]));
        b = &b[((len + 3) & !3).min(b.len())..];
    }
    out
}

/// Сообщения netlink из одного `recv`: (тип, тело после заголовка).
fn messages(mut b: &[u8]) -> Vec<(u16, &[u8])> {
    let mut out = Vec::new();
    while b.len() >= NLMSG_HDR {
        let len = u32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let typ = u16::from_ne_bytes([b[4], b[5]]);
        if len < NLMSG_HDR || len > b.len() {
            break;
        }
        out.push((typ, &b[NLMSG_HDR..len]));
        b = &b[((len + 3) & !3).min(b.len())..];
    }
    out
}

fn c_str(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

impl Listener {
    pub fn open() -> io::Result<Self> {
        // SAFETY: обычные вызовы сокетного API; дескриптор сразу переходит в OwnedFd.
        let fd = unsafe {
            libc::socket(libc::AF_NETLINK, libc::SOCK_RAW | libc::SOCK_CLOEXEC, libc::NETLINK_GENERIC)
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let l = Listener { fd: unsafe { OwnedFd::from_raw_fd(fd) } };
        let mut addr: libc::sockaddr_nl = unsafe { mem::zeroed() };
        addr.nl_family = libc::AF_NETLINK as u16;
        let rc = unsafe {
            libc::bind(
                fd,
                &addr as *const _ as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_nl>() as u32,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        let group = l.resolve_group()?;
        let rc = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_NETLINK,
                libc::NETLINK_ADD_MEMBERSHIP,
                &group as *const u32 as *const libc::c_void,
                mem::size_of::<u32>() as u32,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(l)
    }

    /// Номер multicast-группы `acpi_mc_group` (CTRL_CMD_GETFAMILY).
    fn resolve_group(&self) -> io::Result<u32> {
        let mut attr = Vec::new();
        attr.extend_from_slice(&((4 + FAMILY.len()) as u16).to_ne_bytes());
        attr.extend_from_slice(&(libc::CTRL_ATTR_FAMILY_NAME as u16).to_ne_bytes());
        attr.extend_from_slice(FAMILY);
        attr.resize((attr.len() + 3) & !3, 0);

        let len = NLMSG_HDR + GENL_HDR + attr.len();
        let mut msg = Vec::with_capacity(len);
        msg.extend_from_slice(&(len as u32).to_ne_bytes());
        msg.extend_from_slice(&(libc::GENL_ID_CTRL as u16).to_ne_bytes());
        msg.extend_from_slice(&(libc::NLM_F_REQUEST as u16).to_ne_bytes());
        msg.extend_from_slice(&1u32.to_ne_bytes()); // seq
        msg.extend_from_slice(&0u32.to_ne_bytes()); // pid
        msg.extend_from_slice(&[libc::CTRL_CMD_GETFAMILY as u8, 1, 0, 0]);
        msg.extend_from_slice(&attr);
        let rc = unsafe { libc::send(self.fd.as_raw_fd(), msg.as_ptr().cast(), msg.len(), 0) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        let buf = self.recv()?;
        for (typ, body) in messages(&buf) {
            if typ == libc::NLMSG_ERROR as u16 {
                let errno = body.get(..4).map_or(0, |e| i32::from_ne_bytes([e[0], e[1], e[2], e[3]]));
                return Err(io::Error::from_raw_os_error(-errno));
            }
            let Some(body) = body.get(GENL_HDR..) else { continue };
            for (t, groups) in attrs(body) {
                if t != libc::CTRL_ATTR_MCAST_GROUPS as u16 {
                    continue;
                }
                for (_, g) in attrs(groups) {
                    let a = attrs(g);
                    let name = a.iter().find(|(t, _)| *t == libc::CTRL_ATTR_MCAST_GRP_NAME as u16);
                    let id = a.iter().find(|(t, _)| *t == libc::CTRL_ATTR_MCAST_GRP_ID as u16);
                    if let (Some((_, n)), Some((_, id))) = (name, id) {
                        if c_str(n).as_bytes() == GROUP && id.len() >= 4 {
                            return Ok(u32::from_ne_bytes([id[0], id[1], id[2], id[3]]));
                        }
                    }
                }
            }
        }
        Err(io::Error::new(io::ErrorKind::NotFound, "нет группы acpi_mc_group"))
    }

    fn recv(&self) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; 8192];
        let n = unsafe { libc::recv(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len(), 0) };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        buf.truncate(n as usize);
        Ok(buf)
    }

    /// Ждёт следующую пачку событий.
    pub fn next(&self) -> io::Result<Vec<AcpiEvent>> {
        let buf = self.recv()?;
        let mut out = Vec::new();
        for (_, body) in messages(&buf) {
            let Some(body) = body.get(GENL_HDR..) else { continue };
            for (t, ev) in attrs(body) {
                // struct acpi_genl_event { char device_class[20]; char bus_id[15]; u32 type; u32 data; }
                if t != ATTR_EVENT || ev.len() < 44 {
                    continue;
                }
                out.push(AcpiEvent {
                    device_class: c_str(&ev[..20]),
                    bus_id: c_str(&ev[20..35]),
                    kind: u32::from_ne_bytes([ev[36], ev[37], ev[38], ev[39]]),
                });
            }
        }
        Ok(out)
    }
}
