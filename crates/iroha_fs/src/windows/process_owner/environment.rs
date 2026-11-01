//! Case-insensitive native environment snapshot, including inherited drive-current directories.

use std::{cmp::Ordering, ffi::OsStr, io, os::windows::ffi::OsStrExt as _};
use windows_sys::Win32::{
    Globalization::{CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN, CompareStringOrdinal},
    System::Environment::{FreeEnvironmentStringsW, GetEnvironmentStringsW},
};

const MAX_UNITS: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 16_384;

struct NativeEnvironment(*mut u16);
impl Drop for NativeEnvironment {
    fn drop(&mut self) {
        // SAFETY: this is precisely the original allocation returned by the native snapshot API.
        unsafe { FreeEnvironmentStringsW(self.0) };
    }
}

struct Entry {
    key: Vec<u16>,
    value: Vec<u16>,
}

fn compare(left: &[u16], right: &[u16]) -> io::Result<Ordering> {
    let left_length = i32::try_from(left.len()).map_err(|_| invalid())?;
    let right_length = i32::try_from(right.len()).map_err(|_| invalid())?;
    // SAFETY: the two bounded UTF-16 slices cover the explicitly supplied extents.
    match unsafe {
        CompareStringOrdinal(left.as_ptr(), left_length, right.as_ptr(), right_length, 1)
    } {
        CSTR_LESS_THAN => Ok(Ordering::Less),
        CSTR_EQUAL => Ok(Ordering::Equal),
        CSTR_GREATER_THAN => Ok(Ordering::Greater),
        _ => Err(io::Error::last_os_error()),
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid or excessive native environment",
    )
}

fn locate(entries: &[Entry], key: &[u16]) -> io::Result<Result<usize, usize>> {
    let (mut low, mut high) = (0, entries.len());
    while low < high {
        let mid = low + (high - low) / 2;
        match compare(&entries[mid].key, key)? {
            Ordering::Less => low = mid + 1,
            Ordering::Greater => high = mid,
            Ordering::Equal => return Ok(Ok(mid)),
        }
    }
    Ok(Err(low))
}

fn insert(entries: &mut Vec<Entry>, entry: Entry) -> io::Result<()> {
    match locate(entries, &entry.key)? {
        Ok(index) => entries[index] = entry,
        Err(index) if entries.len() < MAX_ENTRIES => entries.insert(index, entry),
        Err(_) => return Err(invalid()),
    }
    Ok(())
}

pub(super) fn capture<'a>(
    clear: bool,
    changes: impl Iterator<Item = (&'a OsStr, Option<&'a OsStr>)>,
) -> io::Result<Vec<u16>> {
    let mut entries = Vec::new();
    if !clear {
        // SAFETY: native API returns an independent double-NUL-terminated snapshot.
        let raw = unsafe { GetEnvironmentStringsW() };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let native = NativeEnvironment(raw);
        let mut offset = 0;
        loop {
            let mut line = Vec::new();
            loop {
                if offset >= MAX_UNITS {
                    return Err(invalid());
                }
                // SAFETY: offset advances only through entries in the documented
                // terminated snapshot, stopping at the first double terminator.
                let unit = unsafe { *native.0.add(offset) };
                offset += 1;
                if unit == 0 {
                    break;
                }
                line.push(unit);
            }
            if line.is_empty() {
                break;
            }
            // Native inherited =C: entries have a leading '=' which is part of their key.
            let delimiter = line
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(index, &unit)| (unit == u16::from(b'=')).then_some(index))
                .ok_or_else(invalid)?;
            insert(
                &mut entries,
                Entry {
                    key: line[..delimiter].to_vec(),
                    value: line[delimiter + 1..].to_vec(),
                },
            )?;
        }
    }
    for (key, value) in changes {
        let key: Vec<_> = key.encode_wide().collect();
        if key.is_empty()
            || key.len() > MAX_UNITS
            || key.iter().any(|&unit| unit == 0 || unit == u16::from(b'='))
        {
            return Err(invalid());
        }
        if let Some(value) = value {
            let value: Vec<_> = value.encode_wide().collect();
            if value.contains(&0) || value.len() > MAX_UNITS {
                return Err(invalid());
            }
            insert(&mut entries, Entry { key, value })?;
        } else if let Ok(index) = locate(&entries, &key)? {
            entries.remove(index);
        }
    }
    let mut result = Vec::new();
    for entry in entries {
        if result
            .len()
            .saturating_add(entry.key.len())
            .saturating_add(entry.value.len())
            .saturating_add(3)
            > MAX_UNITS
        {
            return Err(invalid());
        }
        result.extend(entry.key);
        result.push(u16::from(b'='));
        result.extend(entry.value);
        result.push(0);
    }
    if result.is_empty() {
        result.push(0);
    }
    result.push(0);
    Ok(result)
}
