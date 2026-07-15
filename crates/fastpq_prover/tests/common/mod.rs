//! Shared safeguards for explicit fixture regeneration.

use std::ffi::OsStr;

pub(crate) fn fixture_update_requested_from(value: Option<&OsStr>) -> Result<bool, &'static str> {
    match value {
        None => Ok(false),
        Some(value) if value == OsStr::new("1") => Ok(true),
        Some(_) => Err("FASTPQ_UPDATE_FIXTURES must be absent or have the exact value `1`"),
    }
}

pub(crate) fn fixture_update_requested() -> bool {
    fixture_update_requested_from(std::env::var_os("FASTPQ_UPDATE_FIXTURES").as_deref())
        .unwrap_or_else(|message| panic!("{message}"))
}
