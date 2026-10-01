//! Current-user named pipes with bounded overlapped I/O and native peer authentication.

#![allow(
    unsafe_code,
    reason = "Win32 pipe peer-process queries and scoped security attributes require FFI"
)]

use super::*;
use std::{
    io,
    os::windows::{ffi::OsStrExt as _, io::AsRawHandle as _, process::CommandExt as _},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
    runtime::{Builder, Runtime},
};
use windows_sys::Win32::System::{
    Pipes::{GetNamedPipeClientProcessId, GetNamedPipeServerProcessId},
    Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW},
};

pub(crate) fn supported() -> Result<()> {
    Ok(())
}

pub(crate) fn detach(command: &mut Command) {
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

fn address(directory: &PrivateDirectory) -> Result<String> {
    directory.revalidate()?;
    let mut hasher = blake3::Hasher::new();
    for code in directory.path().as_os_str().encode_wide() {
        hasher.update(&code.to_le_bytes());
    }
    Ok(format!(
        r"\\.\pipe\iroha-managed-{}",
        hasher.finalize().to_hex()
    ))
}

fn runtime() -> io::Result<Runtime> {
    Builder::new_current_thread().enable_all().build()
}

fn create_pipe(address: &str, first: bool) -> io::Result<NamedPipeServer> {
    let mut options = ServerOptions::new();
    options
        .first_pipe_instance(first)
        .reject_remote_clients(true);
    iroha_fs::windows::with_owner_security_attributes(|attributes| {
        // SAFETY: the filesystem custody owner retains a valid SECURITY_ATTRIBUTES record and
        // its protected current-user DACL for the complete duration of this callback.
        unsafe { options.create_with_security_attributes_raw(address, attributes) }
    })?
}

fn authenticate_peer(handle: std::os::windows::io::RawHandle, client: bool) -> Result<()> {
    let mut process = 0;
    // SAFETY: callers retain the live pipe handle; the process-id output is a valid local u32.
    let accepted = unsafe {
        if client {
            GetNamedPipeClientProcessId(handle, &mut process)
        } else {
            GetNamedPipeServerProcessId(handle, &mut process)
        }
    };
    if accepted == 0 {
        return Err(io::Error::last_os_error().into());
    }
    if !iroha_fs::windows::is_current_user_process(process)? {
        return Err(Error::Invalid(
            "managed pipe peer belongs to another user".into(),
        ));
    }
    Ok(())
}

pub(crate) struct Listener {
    address: String,
    runtime: Arc<Runtime>,
    pending: Mutex<NamedPipeServer>,
}

impl Listener {
    pub(crate) fn bind(directory: &PrivateDirectory) -> Result<Self> {
        let runtime = Arc::new(runtime()?);
        let address = address(directory)?;
        let pending = {
            let _entered = runtime.enter();
            create_pipe(&address, true)?
        };
        Ok(Self {
            address,
            runtime,
            pending: Mutex::new(pending),
        })
    }

    pub(crate) fn accept(&self) -> Result<Option<Connection>> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| Error::Invalid("managed pipe listener is poisoned".into()))?;
        match self.runtime.block_on(async {
            tokio::time::timeout(Duration::from_millis(1), pending.connect()).await
        }) {
            Err(_) => Ok(None),
            Ok(Err(error)) => Err(error.into()),
            Ok(Ok(())) => {
                authenticate_peer(pending.as_raw_handle(), true)?;
                let next = {
                    let _entered = self.runtime.enter();
                    create_pipe(&self.address, false)?
                };
                let stream = std::mem::replace(&mut *pending, next);
                Ok(Some(Connection {
                    stream,
                    runtime: Arc::clone(&self.runtime),
                }))
            }
        }
    }
}

pub(crate) struct Connection {
    stream: NamedPipeServer,
    runtime: Arc<Runtime>,
}

impl Connection {
    pub(crate) fn receive(&mut self) -> Result<ControlRequest> {
        let bytes = self
            .runtime
            .block_on(async {
                tokio::time::timeout(IO_TIMEOUT, read_async(&mut self.stream)).await
            })
            .map_err(|_| Error::Invalid("managed pipe read deadline expired".into()))??;
        decode(&bytes)
    }

    pub(crate) fn reply<T: JsonSerialize>(&mut self, status: &T) -> Result<()> {
        let bytes = encode(status)?;
        self.runtime
            .block_on(async {
                tokio::time::timeout(IO_TIMEOUT, write_async(&mut self.stream, &bytes)).await
            })
            .map_err(|_| Error::Invalid("managed pipe write deadline expired".into()))?
    }
}

pub(crate) fn request_as<T: JsonDeserialize>(
    directory: &PrivateDirectory,
    request: &ControlRequest,
) -> Result<T> {
    let runtime = runtime()?;
    let mut stream = {
        let _entered = runtime.enter();
        ClientOptions::new().open(address(directory)?)?
    };
    authenticate_peer(stream.as_raw_handle(), false)?;
    let bytes = encode(request)?;
    let timeout = if request.action == "down" {
        Duration::from_secs(10)
    } else {
        IO_TIMEOUT
    };
    let reply = runtime
        .block_on(async {
            tokio::time::timeout(timeout, async {
                write_async(&mut stream, &bytes).await?;
                read_async(&mut stream).await
            })
            .await
        })
        .map_err(|_| Error::Invalid("managed pipe control deadline expired".into()))??;
    decode(&reply)
}

async fn read_async(stream: &mut (impl AsyncRead + Unpin)) -> Result<Vec<u8>> {
    let length = stream.read_u32().await? as usize;
    if length == 0 || length > MAX_METADATA {
        return Err(Error::Invalid(
            "managed control frame exceeds its bound".into(),
        ));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes).await?;
    Ok(bytes)
}

async fn write_async(stream: &mut (impl AsyncWrite + Unpin), bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > MAX_METADATA {
        return Err(Error::Invalid(
            "managed control frame exceeds its bound".into(),
        ));
    }
    stream
        .write_u32(
            u32::try_from(bytes.len())
                .map_err(|_| Error::Invalid("managed frame length overflow".into()))?,
        )
        .await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}
