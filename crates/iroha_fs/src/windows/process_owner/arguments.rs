//! Explicit Unicode process settings, with no shell or PATH resolution.

use std::{ffi::OsStr, io, os::windows::ffi::OsStrExt as _, path::Path};

pub(super) fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut units: Vec<_> = value.encode_wide().collect();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native string contains NUL",
        ));
    }
    units.push(0);
    Ok(units)
}

pub(super) fn absolute(path: &Path) -> io::Result<Vec<u16>> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native process path must be absolute",
        ));
    }
    wide(path.as_os_str())
}

pub(super) fn command_line(
    program: &OsStr,
    arguments: std::process::CommandArgs<'_>,
) -> io::Result<Vec<u16>> {
    let application = wide(program)?;
    let application = &application[..application.len() - 1];
    if application.contains(&u16::from(b'"')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native executable name contains quote",
        ));
    }
    // argv[0] uses the executable-name rule: quote it without interpreting its backslashes.
    let mut result = vec![u16::from(b'"')];
    result.extend_from_slice(application);
    result.push(u16::from(b'"'));
    for argument in arguments {
        result.extend([u16::from(b' '), u16::from(b'"')]);
        let argument = wide(argument)?;
        let mut backslashes = 0;
        for &unit in &argument[..argument.len() - 1] {
            if unit == u16::from(b'\\') {
                backslashes += 1;
                continue;
            }
            let escaped = if unit == u16::from(b'"') {
                backslashes * 2 + 1
            } else {
                backslashes
            };
            result.extend(std::iter::repeat_n(u16::from(b'\\'), escaped));
            backslashes = 0;
            result.push(unit);
        }
        result.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes * 2));
        result.push(u16::from(b'"'));
        if result.len() >= 32_767 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native command line exceeds 32767 UTF-16 units",
            ));
        }
    }
    result.push(0);
    if result.len() > 32_767 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "native command line exceeds 32767 UTF-16 units",
        ));
    }
    Ok(result)
}
