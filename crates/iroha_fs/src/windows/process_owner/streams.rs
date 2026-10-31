//! Explicit original stream handles; only three temporary duplicates enter the child handle list.

use std::{
    ffi::OsStr,
    fmt::Write as _,
    io,
    os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{
        DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED,
        GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE,
    },
    Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom},
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE,
        GetFileType, OPEN_EXISTING, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
    },
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        IO::{GetOverlappedResult, OVERLAPPED},
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
            PIPE_REJECT_REMOTE_CLIENTS,
        },
        Threading::{CreateEventW, GetCurrentProcess, GetCurrentProcessId},
    },
};

use super::{Stream, arguments::wide};

pub(super) struct Prepared {
    pub(super) parent: Option<OwnedHandle>,
    pub(super) child: OwnedHandle,
}

fn original(raw: HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: callers pass only a successful native creation result here.
        Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
    }
}

pub(super) fn duplicate(raw: HANDLE, inherit: bool) -> io::Result<OwnedHandle> {
    let mut output = ptr::null_mut();
    // SAFETY: input remains a borrowed original through this duplication. No new
    // authority is obtained from a numeric PID or a reopened path.
    if unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            raw,
            GetCurrentProcess(),
            &raw mut output,
            0,
            i32::from(inherit),
            DUPLICATE_SAME_ACCESS,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    original(output)
}

fn require_stream(raw: HANDLE) -> io::Result<()> {
    // SAFETY: query the selected original before it can enter any inherited handle list.
    match unsafe { GetFileType(raw) } {
        FILE_TYPE_DISK | FILE_TYPE_PIPE | FILE_TYPE_CHAR => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected native stream is not a disk, pipe or character object",
        )),
    }
}

fn null(input: bool) -> io::Result<OwnedHandle> {
    let name = wide(OsStr::new("NUL"))?;
    // SAFETY: the fixed device name and flags describe a new non-inheritable stream original.
    original(unsafe {
        CreateFileW(
            name.as_ptr(),
            if input { GENERIC_READ } else { GENERIC_WRITE },
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    })
}

pub(super) fn prepare(stream: Stream, index: usize) -> io::Result<Prepared> {
    let input = index == 0;
    let child = match stream {
        Stream::Piped => return pipe(input),
        Stream::Null => null(input)?,
        Stream::Owned(handle) => {
            require_stream(handle.as_raw_handle())?;
            duplicate(handle.as_raw_handle(), false)?
        }
        Stream::Inherit => {
            let slot = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE][index];
            // SAFETY: query only the current process's inherited standard-stream slot.
            let handle = unsafe { GetStdHandle(slot) };
            if handle.is_null() || handle == INVALID_HANDLE_VALUE {
                null(input)?
            } else {
                require_stream(handle)?;
                duplicate(handle, false)?
            }
        }
    };
    require_stream(child.as_raw_handle())?;
    Ok(Prepared {
        parent: None,
        child,
    })
}

fn pipe(input: bool) -> io::Result<Prepared> {
    let mut nonce = [0_u8; 24];
    // SAFETY: OS cryptographic randomness fills the exact initialized nonce extent.
    let status = unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            nonce.as_mut_ptr(),
            u32::try_from(nonce.len()).unwrap(),
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
        return Err(io::Error::other(format!(
            "native pipe nonce failed with NTSTATUS {status:#x}"
        )));
    }
    let mut spelling = String::from(r"\\.\pipe\iroha-original-stream-");
    for byte in nonce {
        write!(&mut spelling, "{byte:02x}").unwrap();
    }
    let name = wide(OsStr::new(&spelling))?;
    let direction = if input {
        PIPE_ACCESS_OUTBOUND
    } else {
        PIPE_ACCESS_INBOUND
    };
    // Reuse the canonical effective-token owner/DACL constructor for this original pipe.
    let server = crate::windows::with_owner_security_attributes(|attributes| {
        // SAFETY: exact current-user protected security attributes live through creation.
        original(unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                direction | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_REJECT_REMOTE_CLIENTS,
                1,
                65_536,
                65_536,
                0,
                attributes.cast(),
            )
        })
    })??;
    // SAFETY: open the only just-created private instance; the child side is
    // synchronous, non-inheritable and retained until an explicit duplicate is made.
    let client = original(unsafe {
        CreateFileW(
            name.as_ptr(),
            if input { GENERIC_READ } else { GENERIC_WRITE },
            0,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    })?;
    // SAFETY: unnamed non-inheritable manual-reset event for this stack's one OVERLAPPED.
    let event = original(unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) })?;
    // SAFETY: native OVERLAPPED permits zero offsets/reserved fields for a pipe connection.
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event.as_raw_handle();
    // SAFETY: server, event and OVERLAPPED stay alive through completion observation.
    if unsafe { ConnectNamedPipe(server.as_raw_handle(), &raw mut overlapped) } == 0 {
        let error = io::Error::last_os_error();
        match error.raw_os_error().map(i64::from) {
            Some(value) if value == i64::from(ERROR_PIPE_CONNECTED) => {}
            Some(value) if value == i64::from(ERROR_IO_PENDING) => {
                let mut transferred = 0;
                // SAFETY: a pending connection must complete before its stack storage drops.
                if unsafe {
                    GetOverlappedResult(
                        server.as_raw_handle(),
                        &raw const overlapped,
                        &raw mut transferred,
                        1,
                    )
                } == 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            _ => return Err(error),
        }
    }
    let mut client_id = 0;
    // SAFETY: observe the connected original pipe, without opening or controlling a process by PID.
    if unsafe { GetNamedPipeClientProcessId(server.as_raw_handle(), &raw mut client_id) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: current process's diagnostic ID is compared only; it is never used for handle recovery.
    if client_id != unsafe { GetCurrentProcessId() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private pipe peer is not its original creator",
        ));
    }
    require_stream(client.as_raw_handle())?;
    Ok(Prepared {
        parent: Some(server),
        child: client,
    })
}
