//! Test-local observation of the actual process token used to create validators.
//!
//! This reads the primary token, not a PID lookup or a child tool's text output.
//! `TokenElevation` and `TokenUser` follow the native `GetTokenInformation` contract:
//! <https://learn.microsoft.com/windows/win32/api/securitybaseapi/nf-securitybaseapi-gettokeninformation>.

use std::{
    io,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr::null_mut,
};

use windows_sys::Win32::{
    Foundation::ERROR_INSUFFICIENT_BUFFER,
    Security::{
        GetSidIdentifierAuthority, GetSidSubAuthority, GetSidSubAuthorityCount,
        GetTokenInformation, IsValidSid, IsWellKnownSid, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
        TokenElevation, TokenUser, WinAnonymousSid, WinIUserSid, WinLocalServiceSid,
        WinLocalSystemSid, WinNetworkServiceSid,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

/// Observed identity and privilege of the token inherited by ordinary child creation.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Subject {
    /// Exact numeric SID of the process token's user.
    pub(super) sid: String,
    /// Whether native Windows reports this process as elevated.
    pub(super) elevated: bool,
    service_or_system: bool,
}

impl Subject {
    /// Require an unelevated ordinary account for the unprivileged-start proof.
    pub(super) fn require_unprivileged(&self) -> io::Result<()> {
        if self.elevated || self.service_or_system {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unprivileged-start proof requires an unelevated ordinary Windows account",
            ));
        }
        Ok(())
    }
}

/// Observe the retained original process token without changing its access or privileges.
pub(super) fn current() -> io::Result<Subject> {
    let mut raw = null_mut();
    // SAFETY: GetCurrentProcess is a borrowed kernel pseudo-handle; the valid output
    // receives a newly owned token handle with query-only access and no inheritance.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut raw) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful OpenProcessToken transfers this one valid token handle.
    let token = unsafe { OwnedHandle::from_raw_handle(raw) };
    observe(&token)
}

fn observe(token: &impl AsRawHandle) -> io::Result<Subject> {
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0_u32;
    let elevation_size = u32::try_from(size_of::<TOKEN_ELEVATION>()).unwrap();
    // SAFETY: the stack output is correctly aligned and its exact size is supplied;
    // the borrowed original handle remains live for the complete native call.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenElevation,
            (&raw mut elevation).cast(),
            elevation_size,
            &raw mut returned,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if returned != elevation_size || elevation.TokenIsElevated > 1 {
        return Err(io::Error::other("invalid native token elevation result"));
    }
    let mut bytes = 0_u32;
    // SAFETY: null output with zero length is the documented required-size query.
    let result = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            null_mut(),
            0,
            &raw mut bytes,
        )
    };
    let error = io::Error::last_os_error();
    if result != 0
        || error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER.cast_signed())
        || bytes < u32::try_from(size_of::<TOKEN_USER>()).unwrap()
        || bytes > 4096
    {
        return Err(io::Error::other("cannot obtain bounded native token user"));
    }
    let mut storage = vec![0_usize; (bytes as usize).div_ceil(size_of::<usize>())];
    let mut returned = 0_u32;
    // SAFETY: the allocation is naturally aligned and covers the exact queried size.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenUser,
            storage.as_mut_ptr().cast(),
            bytes,
            &raw mut returned,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if returned > bytes || returned < u32::try_from(size_of::<TOKEN_USER>()).unwrap() {
        return Err(io::Error::other("invalid native token user extent"));
    }
    // SAFETY: a successful TokenUser query initialized TOKEN_USER and its SID in
    // this still-live aligned allocation. Native SID accessors borrow that original.
    let (sid, service_or_system) = unsafe {
        let sid = (*storage.as_ptr().cast::<TOKEN_USER>()).User.Sid;
        if sid.is_null() || IsValidSid(sid) == 0 {
            return Err(io::Error::other("invalid native token user SID"));
        }
        let count = *GetSidSubAuthorityCount(sid);
        if count > 15 {
            return Err(io::Error::other("native user SID exceeds its bound"));
        }
        let authority = (*GetSidIdentifierAuthority(sid))
            .Value
            .iter()
            .fold(0_u64, |number, byte| (number << 8) | u64::from(*byte));
        // IsValidSid accepts only the supported SID revision. IdentifierAuthority
        // is represented numerically here; this string is diagnostic, not an ACL input.
        let mut text = format!("S-1-{authority}");
        for index in 0..count {
            use std::fmt::Write as _;
            write!(&mut text, "-{}", *GetSidSubAuthority(sid, u32::from(index)))
                .expect("writing to String cannot fail");
        }
        let well_known = [
            WinLocalSystemSid,
            WinLocalServiceSid,
            WinNetworkServiceSid,
            WinAnonymousSid,
            WinIUserSid,
        ]
        .into_iter()
        .any(|kind| IsWellKnownSid(sid, kind) != 0);
        // Named service, application-pool, virtual-machine, window-manager and
        // font-driver account namespaces; these are not ordinary interactive users.
        let service_namespace = authority == 5
            && count > 0
            && matches!(*GetSidSubAuthority(sid, 0), 80 | 82 | 83 | 90 | 96);
        (text, well_known || service_namespace)
    };
    Ok(Subject {
        sid,
        elevated: elevation.TokenIsElevated != 0,
        service_or_system,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_process_token_observes_stable_sid_and_actual_elevation() {
        let first = current().unwrap();
        let second = current().unwrap();
        assert_eq!(first, second);
        assert!(first.sid.starts_with("S-1-"));
        assert_eq!(
            first.require_unprivileged().is_ok(),
            !first.elevated && !first.service_or_system
        );
    }

    #[test]
    fn failed_native_observation_cannot_become_unprivileged_evidence() {
        let file = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
        assert!(observe(&file).is_err());
    }

    #[test]
    fn elevation_and_service_subjects_always_refuse() {
        for (elevated, service_or_system) in [(true, false), (false, true), (true, true)] {
            let subject = Subject {
                sid: "S-1-5-18".to_owned(),
                elevated,
                service_or_system,
            };
            assert_eq!(
                subject.require_unprivileged().unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
        }
    }
}
