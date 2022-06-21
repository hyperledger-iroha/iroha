#![allow(clippy::restriction)]

use once_cell::sync::Lazy;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::ops::Range;
use std::sync::Mutex;

static PORT_IDX: Lazy<Mutex<u16>> = Lazy::new(|| Mutex::new(1000));

#[macro_export]
macro_rules! generate_offset {
    () => {{
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        let name = &name[..name.len() - 3];
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        1000 + (hasher.finish() % ((u16::MAX - 1000) as u64)) as u16
    }};
}

pub fn set_offset(offset: u16) -> Result<(), String> {
    let mut port_idx = PORT_IDX
        .lock()
        .map_err(|_| "Failed to aquire the lock".to_owned())?;
    *port_idx = offset;

    Ok(())
}

/// Returns a free unique local port. Every time a call to this function during one run should
/// return a unique address.
///
/// # Examples
/// ```
/// use unique_port::get_unique_free_port;
///
/// let port_1 = get_unique_free_port().unwrap();
/// let port_2 = get_unique_free_port().unwrap();
/// assert_ne!(port_1, port_2);
/// ```
pub fn get_unique_free_port() -> Result<u16, String> {
    let mut port_idx = PORT_IDX
        .lock()
        .map_err(|_| "Failed to aquire the lock".to_owned())?;
    let result = get_free_port(*port_idx..u16::MAX);
    if let Ok(port) = result {
        *port_idx = port + 1;
    }
    result
}

/// Returns empty port from range. Can be not unique
fn get_free_port(ports: Range<u16>) -> Result<u16, String> {
    ports
        .into_iter()
        .find(|port| TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, *port)).is_ok())
        .ok_or_else(|| "Failed to get empty port".to_owned())
}
