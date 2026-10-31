//! Bounded C interface to the canonical retained process owner.
//!
//! No executable, module, source or release authority is conferred here. The caller
//! independently authenticates this library and its entry, retains both originals
//! and all source owners, and must not release them until original terminal
//! observation: the complete Job on Windows, the original root only on Unix.
//! Failed or unknown sessions remain in the process-local owner table. No API
//! attaches to an arbitrary PID or performs post-spawn Job assignment. The Linux
//! observer separately selects an already observed original FORK; it never owns
//! or reaps the caller's Popen, and its lifecycle result is not command success.
//! Artifact authentication belongs to the embedding release owner. This interface
//! never turns a freshly observed executable or library hash into trust.

mod root_observation;
pub use root_observation::{
    ROOT_OBSERVER_UNSUPPORTED, iroha_process_root_observer_bind_v1,
    iroha_process_root_observer_close_v1, iroha_process_root_observer_finish_v1,
    iroha_process_root_observer_poll_v1, iroha_process_root_observer_prepare_v1,
};

use iroha_fs::{ProcessChild, ProcessCommand, ProcessStdio};
use std::{
    collections::BTreeMap,
    io::Read as _,
    panic::{AssertUnwindSafe, catch_unwind},
    process::ChildStdout,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

/// The operation completed successfully.
pub const OK: i32 = 0;
/// The original stdout stream reached EOF.
pub const EOF: i32 = 1;
/// Original completion has not yet been observed (whole Job on Windows).
pub const PENDING: i32 = 2;
/// The request or writable output extent violates the explicit ABI contract.
pub const INVALID: i32 = -1;
/// No process-local session has this token.
pub const UNKNOWN: i32 = -2;
/// An original stream/process operation failed; custody remains owned.
pub const IO: i32 = -3;
/// Internal observation failed; callers must retain all higher-level custody.
pub const PANIC: i32 = -4;
const MAX_REQUEST: usize = 512 * 1024;
const MAX_READ: usize = 64 * 1024;

#[derive(norito::json::JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Request {
    program: String,
    directory: String,
    arguments: Vec<String>,
    environment: BTreeMap<String, String>,
}

struct Session {
    child: Mutex<Option<ProcessChild>>,
    stdout: Mutex<Option<ChildStdout>>,
    // Set only when spawn returned an explicit error without producing a child.
    refused: Mutex<bool>,
}

fn sessions() -> &'static Mutex<BTreeMap<u64, Arc<Session>>> {
    static VALUE: OnceLock<Mutex<BTreeMap<u64, Arc<Session>>>> = OnceLock::new();
    VALUE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn next_process_token(value: u64) -> Option<u64> {
    value
        .checked_add(1)
        .filter(|next| *next < root_observation::OBSERVER_TAG)
}

fn original(token: u64) -> Result<Arc<Session>, i32> {
    if token & root_observation::OBSERVER_TAG != 0 {
        return Err(UNKNOWN);
    }
    sessions()
        .lock()
        .map_err(|_| PANIC)?
        .get(&token)
        .cloned()
        .ok_or(UNKNOWN)
}

fn boundary(operation: impl FnOnce() -> Result<i32, i32>) -> i32 {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value) | Err(value)) => value,
        Err(_) => PANIC,
    }
}

fn request(bytes: &[u8]) -> Result<ProcessCommand, i32> {
    let request: Request = norito::json::from_slice(bytes).map_err(|_| INVALID)?;
    if request.arguments.len() > 1024
        || request.environment.len() > 128
        || request.arguments.iter().map(String::len).sum::<usize>() > 128 * 1024
        || request
            .environment
            .iter()
            .map(|(name, value)| name.len() + value.len())
            .sum::<usize>()
            > 256 * 1024
        || request.program.contains('\0')
        || request.directory.contains('\0')
        || request
            .arguments
            .iter()
            .any(|argument| argument.contains('\0'))
        || request.environment.iter().any(|(name, value)| {
            name.is_empty() || name.contains(['\0', '=']) || value.contains('\0')
        })
    {
        return Err(INVALID);
    }
    let mut command = ProcessCommand::new(request.program).map_err(|_| INVALID)?;
    command
        .current_dir(request.directory)
        .args(request.arguments)
        .env_clear()
        .envs(request.environment)
        .stdin(ProcessStdio::Null)
        .stdout(ProcessStdio::Piped)
        .stderr(ProcessStdio::Inherit)
        .kill_on_drop(false);
    Ok(command)
}

fn start_original(
    session: &Session,
    spawn: impl FnOnce() -> std::io::Result<ProcessChild>,
) -> Result<i32, i32> {
    // Admit every destination before creation. Once spawn returns an original,
    // no allocation or fallible lock acquisition separates it from table custody.
    let mut child = session.child.lock().map_err(|_| PANIC)?;
    let mut stdout = session.stdout.lock().map_err(|_| PANIC)?;
    let mut refused = session.refused.lock().map_err(|_| PANIC)?;
    if child.is_some() || stdout.is_some() || *refused {
        return Err(INVALID);
    }
    spawn().map_or_else(
        |_| {
            *refused = true;
            Err(IO)
        },
        |original| {
            let original = child.insert(original);
            *stdout = original.stdout.take();
            Ok(OK)
        },
    )
}

/// Create one process session using only explicit bounded UTF-8 JSON settings.
///
/// `out_token` is set before native creation. A nonzero token remains owned even
/// after an error and must be observed/closed or permanently retained by the caller.
/// No caller-supplied raw handle or arbitrary stream policy crosses this interface.
///
/// # Safety
/// `bytes` must reference `length` initialized readable bytes. `out_token` must be
/// an aligned writable `u64`, disjoint from the request. Both live for this call.
#[allow(
    unsafe_code,
    reason = "bounded C ABI validates extents and documents pointer ownership"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_process_spawn_v1(
    bytes: *const u8,
    length: usize,
    out_token: *mut u64,
) -> i32 {
    if out_token.is_null() {
        return INVALID;
    }
    // SAFETY: the caller supplies the documented initialized writable output.
    unsafe {
        out_token.write(0);
    }
    if bytes.is_null() || length == 0 || length > MAX_REQUEST {
        return INVALID;
    }
    boundary(|| {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        // SAFETY: the caller retains this bounded readable request for the call.
        let bytes = unsafe { std::slice::from_raw_parts(bytes, length) };
        let command = request(bytes)?;
        let token = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, next_process_token)
            .map_err(|_| PANIC)?;
        let session = Arc::new(Session {
            child: Mutex::new(None),
            stdout: Mutex::new(None),
            refused: Mutex::new(false),
        });
        // All session/table allocation precedes process creation. The token is
        // externally observable even if later observation unexpectedly panics.
        sessions()
            .lock()
            .map_err(|_| PANIC)?
            .insert(token, Arc::clone(&session));
        // SAFETY: output validity is part of this function's caller contract.
        unsafe {
            out_token.write(token);
        }
        start_original(&session, || command.spawn())
    })
}

/// Read at most 64 KiB from the original stdout pipe; this may block until data/EOF.
///
/// The caller must keep session and higher-level source/entry owners alive while
/// another thread is inside this call. An observation error never closes a session.
///
/// # Safety
/// `buffer` references `capacity` writable bytes, and `written` is an aligned writable
/// `usize`. These disjoint regions remain valid for the entire potentially blocking call.
#[allow(
    unsafe_code,
    reason = "bounded C ABI validates extents and documents pointer ownership"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_process_read_v1(
    token: u64,
    buffer: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> i32 {
    if buffer.is_null() || written.is_null() || capacity == 0 || capacity > MAX_READ {
        return INVALID;
    }
    // SAFETY: caller guarantees the output region and alignment.
    unsafe {
        written.write(0);
    }
    boundary(|| {
        let session = original(token)?;
        let mut output = session.stdout.lock().map_err(|_| PANIC)?;
        let output = output.as_mut().ok_or(IO)?;
        // SAFETY: caller owns this bounded writable region for the entire call.
        let bytes = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
        let count = output.read(bytes).map_err(|_| IO)?;
        // SAFETY: writable count output is independent of the data buffer.
        unsafe {
            written.write(count);
        }
        Ok(if count == 0 { EOF } else { OK })
    })
}

/// Observe the original root terminal state; Windows also requires every Job member to exit.
///
/// # Safety
/// `exit_code` is an aligned writable `i64` for this call. Its value is meaningful
/// only for `OK`; `PENDING` and errors confer no terminal observation.
#[allow(
    unsafe_code,
    reason = "bounded C ABI validates extents and documents pointer ownership"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn iroha_process_try_wait_v1(token: u64, exit_code: *mut i64) -> i32 {
    if exit_code.is_null() {
        return INVALID;
    }
    boundary(|| {
        let session = original(token)?;
        let mut child = session.child.lock().map_err(|_| PANIC)?;
        let status = child.as_mut().ok_or(IO)?.try_wait().map_err(|_| IO)?;
        status.map_or(Ok(PENDING), |status| {
            // SAFETY: caller guarantees the output storage for this call.
            unsafe {
                exit_code.write(status.code().map_or(-1, i64::from));
            }
            Ok(OK)
        })
    })
}

/// Explicitly request termination of only this session's original process owner.
///
/// This is never called automatically on cancellation, read failure or close. The
/// caller must separately possess authorization to terminate its actual workload.
#[allow(
    unsafe_code,
    reason = "bounded C ABI validates extents and documents pointer ownership"
)]
#[unsafe(no_mangle)]
pub extern "C" fn iroha_process_kill_v1(token: u64) -> i32 {
    boundary(|| {
        original(token)?
            .child
            .lock()
            .map_err(|_| PANIC)?
            .as_mut()
            .ok_or(IO)?
            .kill()
            .map_err(|_| IO)?;
        Ok(OK)
    })
}

/// Release a session only after original terminal observation or spawn refusal.
/// Windows requires every original Job member to exit; Unix observes the root only.
/// A live, poisoned or unobservable session remains owned and returns `PENDING`/error.
#[allow(
    unsafe_code,
    reason = "bounded C ABI validates extents and documents pointer ownership"
)]
#[unsafe(no_mangle)]
pub extern "C" fn iroha_process_close_v1(token: u64) -> i32 {
    boundary(|| {
        let session = original(token)?;
        let mut child = session.child.lock().map_err(|_| PANIC)?;
        if let Some(child) = child.as_mut() {
            if child.try_wait().map_err(|_| IO)?.is_none() {
                return Ok(PENDING);
            }
        } else if !*session.refused.lock().map_err(|_| PANIC)? {
            return Err(IO);
        }
        sessions()
            .lock()
            .map_err(|_| PANIC)?
            .remove(&token)
            .ok_or(UNKNOWN)?;
        Ok(OK)
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod token_boundary_tests {
    use super::*;
    #[test]
    fn process_tokens_never_enter_observer_namespace_or_wrap() {
        assert_eq!(next_process_token(1), Some(2));
        assert_eq!(
            next_process_token(root_observation::OBSERVER_TAG - 2),
            Some(root_observation::OBSERVER_TAG - 1)
        );
        assert_eq!(next_process_token(root_observation::OBSERVER_TAG - 1), None);
        assert_eq!(next_process_token(root_observation::OBSERVER_TAG), None);
        assert_eq!(next_process_token(u64::MAX), None);
        assert!(matches!(
            original(root_observation::OBSERVER_TAG),
            Err(UNKNOWN)
        ));
    }
}
