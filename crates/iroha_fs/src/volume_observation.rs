//! Path-based Windows volume observations for local capacity planning, without custody authority.
#![cfg_attr(
    windows,
    allow(unsafe_code, reason = "bounded Win32 volume query buffers require FFI")
)]

use std::io;
#[cfg(windows)]
use std::{os::windows::ffi::OsStrExt as _, path::Path};

#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
};

const BUFFER_UNITS: u32 = 32_768;

fn normalize_windows_volume_mount_point(volume_mount_point: &str) -> String {
    let mut normalized = volume_mount_point.replace('/', "\\");
    if !normalized.ends_with('\\') {
        normalized.push('\\');
    }
    normalized
}

fn normalize_windows_volume_identity(volume_name: &str) -> String {
    let mut normalized = normalize_windows_volume_mount_point(volume_name);
    normalized.make_ascii_lowercase();
    format!("volume:{normalized}")
}

fn windows_string_from_wide_buffer(buffer: &[u16]) -> Option<String> {
    let end = buffer.iter().position(|&unit| unit == 0)?;
    Some(String::from_utf16_lossy(&buffer[..end]))
}

fn terminated(mut units: Vec<u16>) -> io::Result<Vec<u16>> {
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "volume query path contains an embedded NUL",
        ));
    }
    units.push(0);
    Ok(units)
}

fn query_volume_string(
    query: impl FnOnce(&mut [u16], u32) -> io::Result<()>,
) -> io::Result<String> {
    let mut buffer = vec![0_u16; usize::try_from(BUFFER_UNITS).expect("bounded native buffer")];
    query(&mut buffer, BUFFER_UNITS)?;
    windows_string_from_wide_buffer(&buffer).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "volume query returned an unterminated native string",
        )
    })
}

/// Observe the caller's currently available bytes and total bytes for a Windows path.
///
/// Available space reflects the caller's quota. This path query does not retain an original
/// directory or authenticate its ancestry, and does not supply file-custody authority.
///
/// # Errors
/// Returns invalid input for an embedded NUL, or the native volume-query error.
#[cfg(windows)]
pub fn observe_volume_space(path: &Path) -> io::Result<(u64, u64)> {
    let path = terminated(path.as_os_str().encode_wide().collect())?;
    let (mut available, mut total) = (0_u64, 0_u64);
    // SAFETY: the path is NUL-terminated without embedded NULs, and both output pointers
    // refer to live u64 values. The unused third output is explicitly optional.
    let success = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &raw mut available,
            &raw mut total,
            std::ptr::null_mut(),
        )
    };
    if success == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((available, total))
}

/// Observe a normalized `volume:` GUID spelling for the volume containing a Windows path.
///
/// Mount-point aliases map to the native volume GUID. This path query supplies a planning
/// observation, not an original file identity or retained namespace authority.
///
/// # Errors
/// Returns invalid input for an embedded NUL, malformed output, or a native volume-query error.
#[cfg(windows)]
pub fn observe_volume_identity(path: &Path) -> io::Result<String> {
    let path = terminated(path.as_os_str().encode_wide().collect())?;
    let mount_point = query_volume_string(|buffer, length| {
        // SAFETY: path is terminated and the output slice has exactly length writable units.
        if unsafe { GetVolumePathNameW(path.as_ptr(), buffer.as_mut_ptr(), length) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })?;
    let mount_point = terminated(
        normalize_windows_volume_mount_point(&mount_point)
            .encode_utf16()
            .collect(),
    )?;
    let volume_name = query_volume_string(|buffer, length| {
        // SAFETY: mount_point is terminated and the output has exactly length writable units.
        if unsafe {
            GetVolumeNameForVolumeMountPointW(mount_point.as_ptr(), buffer.as_mut_ptr(), length)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    })?;
    Ok(normalize_windows_volume_identity(&volume_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_windows_volume_mount_point_adds_trailing_separator() {
        assert_eq!(
            normalize_windows_volume_mount_point(r"C:\nexus\storage"),
            r"C:\nexus\storage\"
        );
        assert_eq!(
            normalize_windows_volume_mount_point(
                r"\\?\Volume{ABCDEF12-3456-7890-ABCD-EF1234567890}\"
            ),
            r"\\?\Volume{ABCDEF12-3456-7890-ABCD-EF1234567890}\"
        );
    }

    #[test]
    fn normalize_windows_volume_identity_uses_lowercased_guid_path() {
        assert_eq!(
            normalize_windows_volume_identity(r"\\?\Volume{ABCDEF12-3456-7890-ABCD-EF1234567890}\"),
            r"volume:\\?\volume{abcdef12-3456-7890-abcd-ef1234567890}\"
        );
    }

    #[test]
    fn windows_string_from_wide_buffer_stops_at_first_nul() {
        let buffer: Vec<u16> = "Volume\0ignored".encode_utf16().collect();
        assert_eq!(
            windows_string_from_wide_buffer(&buffer).as_deref(),
            Some("Volume")
        );
        assert_eq!(windows_string_from_wide_buffer(&[]), None);
    }

    #[test]
    fn native_query_bounds_output_and_preserves_errors() {
        assert_eq!(
            query_volume_string(|buffer, length| {
                assert_eq!(buffer.len(), usize::try_from(length).unwrap());
                buffer[..3].copy_from_slice(&[b'C'.into(), b':'.into(), 0]);
                Ok(())
            })
            .unwrap(),
            "C:"
        );
        assert_eq!(
            query_volume_string(|_, _| Err(io::Error::from_raw_os_error(123)))
                .unwrap_err()
                .raw_os_error(),
            Some(123)
        );
        assert_eq!(
            query_volume_string(|buffer, _| {
                buffer.fill(1);
                Ok(())
            })
            .unwrap_err()
            .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn terminated_path_refuses_silent_nul_truncation() {
        assert_eq!(terminated(vec![1, 2]).unwrap(), vec![1, 2, 0]);
        assert_eq!(
            terminated(vec![1, 0, 2]).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[cfg(windows)]
    #[test]
    fn actual_native_volume_identity_and_space_are_observed() {
        let directory = tempfile::tempdir().unwrap();
        let identity = observe_volume_identity(directory.path()).unwrap();
        assert!(identity.starts_with(r"volume:\\?\volume{"));
        assert!(identity.ends_with("}\\"));
        assert_eq!(identity, identity.to_ascii_lowercase());
        let child = directory.path().join("nested");
        std::fs::create_dir(&child).unwrap();
        assert_eq!(observe_volume_identity(&child).unwrap(), identity);
        let (available, total) = observe_volume_space(&child).unwrap();
        assert!(total > 0);
        assert!(available <= total);
        let nul = Path::new("C:\\ignored\0suffix");
        assert_eq!(
            observe_volume_identity(nul).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            observe_volume_space(nul).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
