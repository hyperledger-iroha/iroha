//! Native, nonblocking process observations for test-network permit recovery.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::{
    Foundation::{ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
};

/// Observe an existing process without requesting query, mutation or termination rights.
pub(super) fn pid_alive(pid: u32) -> Option<bool> {
    // Zero identifies the system idle process, not a possible permit owner. Its
    // documented INVALID_PARAMETER result must not be interpreted as death.
    if pid == 0 {
        return None;
    }
    // SAFETY: OpenProcess takes a scalar PID and returns a new, noninherited
    // owned handle. No process state is changed by requesting SYNCHRONIZE.
    let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if raw.is_null() {
        return classify_open_error(std::io::Error::last_os_error().raw_os_error());
    }
    // SAFETY: The successful OpenProcess call transferred this unique handle.
    let process = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: The owned process handle remains live throughout this zero-time
    // wait and was opened with the required SYNCHRONIZE access.
    match unsafe { WaitForSingleObject(process.as_raw_handle(), 0) } {
        WAIT_TIMEOUT => Some(true),
        WAIT_OBJECT_0 => Some(false),
        _ => None,
    }
}

fn classify_open_error(error: Option<i32>) -> Option<bool> {
    match error.map(i32::cast_unsigned) {
        Some(ERROR_INVALID_PARAMETER) => Some(false),
        // Like Unix EPERM, denial is not evidence that the process died.
        Some(ERROR_ACCESS_DENIED) => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Read as _,
        process::{Command, Stdio},
    };

    use super::*;

    const CHILD_ENV: &str = "IROHA_TEST_NETWORK_LIVENESS_CHILD";

    #[test]
    fn current_process_is_alive_and_idle_is_not_a_permit_owner() {
        assert_eq!(pid_alive(std::process::id()), Some(true));
        assert_eq!(pid_alive(0), None);
    }

    #[test]
    fn observation_errors_never_classify_denial_or_unknown_as_death() {
        assert_eq!(
            classify_open_error(Some(ERROR_ACCESS_DENIED.cast_signed())),
            Some(true)
        );
        assert_eq!(
            classify_open_error(Some(ERROR_INVALID_PARAMETER.cast_signed())),
            Some(false)
        );
        assert_eq!(classify_open_error(Some(8)), None);
        assert_eq!(classify_open_error(None), None);
    }

    #[test]
    fn process_child() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        let mut input = Vec::new();
        std::io::stdin()
            .read_to_end(&mut input)
            .expect("parent EOF");
        // A real process may return STILL_ACTIVE as its exit code. Observation
        // must use the process object's signaled state, not this integer.
        std::process::exit(259);
    }

    #[test]
    fn original_child_changes_from_alive_to_dead_even_with_exit_code_259() {
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "windows_process::tests::process_child",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start native child");
        assert_eq!(pid_alive(child.id()), Some(true));
        drop(child.stdin.take());
        assert_eq!(child.wait().expect("native child exit").code(), Some(259));
        // Child retains its original process handle, so the PID still names
        // that terminated object for this observation, without PID reuse.
        assert_eq!(pid_alive(child.id()), Some(false));
    }
}
