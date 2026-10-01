//! Bounded owner-authenticated local control transport.

use super::*;
use iroha_fs::PrivateDirectory;
use std::{
    io::{Read, Write},
    process::Command,
};

const IO_TIMEOUT: Duration = Duration::from_secs(2);

fn write_frame(stream: &mut impl Write, bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_METADATA {
        return Err(Error::Invalid(
            "managed control frame exceeds its bound".into(),
        ));
    }
    let length = u32::try_from(bytes.len())
        .map_err(|_| Error::Invalid("managed frame is too long".into()))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(bytes)?;
    stream.flush()?;
    Ok(())
}

fn read_frame(stream: &mut impl Read) -> Result<Vec<u8>> {
    let mut header = [0_u8; 4];
    stream.read_exact(&mut header)?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > MAX_METADATA {
        return Err(Error::Invalid(
            "managed control frame exceeds its bound".into(),
        ));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(unix)]
mod native {
    use super::*;
    use std::{
        fs,
        os::unix::{
            ffi::OsStrExt as _,
            fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _},
            net::{UnixListener, UnixStream},
            process::CommandExt as _,
        },
        path::PathBuf,
    };

    pub(crate) fn supported() -> Result<()> {
        Ok(())
    }

    pub(crate) fn detach(command: &mut Command) {
        // The owned supervisor has a separate process group; its stdio is already retained
        // private files. Lifecycle control never signals a guessed process group or PID.
        command.process_group(0);
    }

    fn endpoint(directory: &PrivateDirectory) -> PathBuf {
        directory.path().join("s")
    }

    fn ipc_directory(store: &PrivateDirectory, create: bool) -> Result<PrivateDirectory> {
        store.revalidate()?;
        // macOS sockaddr_un permits only 104 pathname bytes. The socket's private directory is
        // independent of the (potentially very long) workspace path, but binds its complete hash.
        let temporary = if cfg!(target_os = "macos") {
            "/private/tmp"
        } else {
            "/tmp"
        };
        let owner = rustix::process::geteuid().as_raw();
        let digest = blake3::hash(store.path().as_os_str().as_bytes()).to_hex();
        let path = PathBuf::from(temporary)
            .join(format!("iroha-{owner}"))
            .join(digest.as_str());
        Ok(if create {
            PrivateDirectory::open_or_create(path)?
        } else {
            PrivateDirectory::open(path)?
        })
    }

    fn validate_endpoint(directory: &PrivateDirectory) -> Result<(u64, u64)> {
        directory.revalidate()?;
        let metadata = fs::symlink_metadata(endpoint(directory))?;
        if !metadata.file_type().is_socket()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.permissions().mode() & 0o777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(Error::Invalid(
                "managed socket must be one direct owner-only socket".into(),
            ));
        }
        Ok((metadata.dev(), metadata.ino()))
    }

    fn authenticate_peer(stream: &UnixStream) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let uid = rustix::net::sockopt::socket_peercred(stream)
            .map_err(std::io::Error::from)?
            .uid
            .as_raw();
        #[cfg(target_os = "macos")]
        let uid = {
            use std::os::fd::AsRawFd as _;
            let mut uid = 0;
            let mut gid = 0;
            #[allow(
                unsafe_code,
                reason = "getpeereid only reads kernel credentials from this live socket and writes two valid local outputs"
            )]
            let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            uid
        };
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        let uid = return Err(Error::Invalid(
            "managed peer credential verification is unavailable on this Unix platform".into(),
        ));
        if uid != rustix::process::geteuid().as_raw() {
            return Err(Error::Invalid(
                "managed control peer belongs to another user".into(),
            ));
        }
        Ok(())
    }

    pub(crate) struct Listener {
        listener: UnixListener,
        path: PathBuf,
        identity: (u64, u64),
        _directory: PrivateDirectory,
    }

    impl Listener {
        pub(crate) fn bind(directory: &PrivateDirectory) -> Result<Self> {
            let directory = ipc_directory(directory, true)?;
            directory.revalidate()?;
            let path = endpoint(&directory);
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    validate_endpoint(&directory)?;
                    fs::remove_file(&path)?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            let listener = UnixListener::bind(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            let identity = validate_endpoint(&directory)?;
            Ok(Self {
                listener,
                path,
                identity,
                _directory: directory,
            })
        }

        pub(crate) fn accept(&self) -> Result<Option<Connection>> {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    authenticate_peer(&stream)?;
                    // BSD/macOS accept inherits the listener's nonblocking mode. A client
                    // may not have written its frame yet, so restore blocking I/O before
                    // applying the finite per-connection timeouts below.
                    stream.set_nonblocking(false)?;
                    stream.set_read_timeout(Some(IO_TIMEOUT))?;
                    stream.set_write_timeout(Some(IO_TIMEOUT))?;
                    Ok(Some(Connection(stream)))
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
                Err(error) => Err(error.into()),
            }
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            if let Ok(metadata) = fs::symlink_metadata(&self.path)
                && (metadata.dev(), metadata.ino()) == self.identity
            {
                let _ = fs::remove_file(&self.path);
            }
        }
    }

    pub(crate) struct Connection(UnixStream);

    impl Connection {
        pub(crate) fn receive(&mut self) -> Result<ControlRequest> {
            decode(&read_frame(&mut self.0)?)
        }

        pub(crate) fn reply<T: JsonSerialize>(&mut self, status: &T) -> Result<()> {
            write_frame(&mut self.0, &encode(status)?)
        }
    }

    pub(crate) fn request_as<T: JsonDeserialize>(
        directory: &PrivateDirectory,
        request: &ControlRequest,
    ) -> Result<T> {
        let directory = ipc_directory(directory, false)?;
        let before = validate_endpoint(&directory)?;
        let mut stream = UnixStream::connect(endpoint(&directory))?;
        authenticate_peer(&stream)?;
        stream.set_read_timeout(Some(if request.action == "down" {
            Duration::from_secs(10)
        } else {
            IO_TIMEOUT
        }))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;
        if validate_endpoint(&directory)? != before {
            return Err(Error::Invalid(
                "managed control endpoint changed during connection".into(),
            ));
        }
        write_frame(&mut stream, &encode(request)?)?;
        decode(&read_frame(&mut stream)?)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn peer_credentials_authenticate_both_owned_socket_pair_ends() {
            let (left, right) = UnixStream::pair().unwrap();
            authenticate_peer(&left).unwrap();
            authenticate_peer(&right).unwrap();
        }

        #[test]
        fn long_workspace_uses_a_short_owner_bound_socket_and_cleans_it() {
            let temporary = tempfile::tempdir().unwrap();
            let path = temporary.path().join("x".repeat(80)).join("y".repeat(80));
            let directory = PrivateDirectory::open_or_create(path).unwrap();
            let listener = Listener::bind(&directory).unwrap();
            assert!(listener.path.as_os_str().as_bytes().len() < 104);
            let ipc = ipc_directory(&directory, false).unwrap();
            validate_endpoint(&ipc).unwrap();
            let stream = UnixStream::connect(&listener.path).unwrap();
            authenticate_peer(&stream).unwrap();
            assert!(listener.accept().unwrap().is_some());
            let socket = listener.path.clone();
            drop(listener);
            assert!(!socket.exists());
        }

        #[test]
        fn accepted_control_connection_waits_for_delayed_and_partial_frames() {
            let _resources = super::super::super::native_test_guard();
            let temporary = tempfile::tempdir().unwrap();
            let directory =
                PrivateDirectory::open_or_create(temporary.path().join("network")).unwrap();
            let listener = Listener::bind(&directory).unwrap();
            let mut client = UnixStream::connect(&listener.path).unwrap();
            let mut connection = listener.accept().unwrap().unwrap();
            assert_eq!(connection.0.read_timeout().unwrap(), Some(IO_TIMEOUT));
            assert_eq!(connection.0.write_timeout().unwrap(), Some(IO_TIMEOUT));
            // Accept remains a polling operation even though its accepted stream blocks.
            assert!(listener.accept().unwrap().is_none());
            let request = ControlRequest {
                token: "c".repeat(64),
                action: "status".into(),
            };
            let mut frame = Vec::new();
            write_frame(&mut frame, &encode(&request).unwrap()).unwrap();
            let (entered, ready) = std::sync::mpsc::channel();
            let (sender, received) = std::sync::mpsc::channel();
            let reader = std::thread::spawn(move || {
                entered.send(()).unwrap();
                sender.send(connection.receive()).unwrap();
            });
            ready.recv_timeout(IO_TIMEOUT).unwrap();
            let before_frame = received.recv_timeout(Duration::from_millis(50));
            let prefix = client.write_all(&frame[..2]);
            let partial_header = received.recv_timeout(Duration::from_millis(50));
            let remainder = client.write_all(&frame[2..]);
            let complete = received.recv_timeout(IO_TIMEOUT + Duration::from_secs(1));
            // Join before assertions so the pre-fix early WouldBlock path leaves no reader.
            reader.join().unwrap();
            assert!(matches!(
                before_frame,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            assert!(matches!(
                partial_header,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
            prefix.unwrap();
            remainder.unwrap();
            let actual = complete.unwrap().unwrap();
            assert_eq!(actual.action, request.action);
            assert_eq!(actual.token, request.token);
        }
    }
}

#[cfg(windows)]
#[path = "transport_windows.rs"]
mod native;

#[cfg(not(any(unix, windows)))]
mod native {
    use super::*;

    pub(crate) fn supported() -> Result<()> {
        Err(Error::Invalid(
            "this build does not yet provide owner-authenticated native managed IPC".into(),
        ))
    }
    pub(crate) fn detach(_: &mut Command) {}
    pub(crate) struct Listener;
    impl Listener {
        pub(crate) fn bind(_: &PrivateDirectory) -> Result<Self> {
            supported()?;
            Ok(Self)
        }
        pub(crate) fn accept(&self) -> Result<Option<Connection>> {
            supported()?;
            Ok(None)
        }
    }
    pub(crate) struct Connection;
    impl Connection {
        pub(crate) fn receive(&mut self) -> Result<ControlRequest> {
            Err(Error::Invalid("native IPC unavailable".into()))
        }
        pub(crate) fn reply<T: JsonSerialize>(&mut self, _: &T) -> Result<()> {
            supported()
        }
    }
    pub(crate) fn request_as<T: JsonDeserialize>(
        _: &PrivateDirectory,
        _: &ControlRequest,
    ) -> Result<T> {
        Err(Error::Invalid("native IPC unavailable".into()))
    }
}

pub(crate) use native::{Listener, detach, request_as, supported};

pub(crate) fn request(
    directory: &PrivateDirectory,
    request: &ControlRequest,
) -> Result<ManagedStatus> {
    request_as(directory, request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_control_frame_is_rejected_before_allocating_payload() {
        let bytes = ((MAX_METADATA + 1) as u32).to_be_bytes();
        assert!(read_frame(&mut bytes.as_slice()).is_err());
        assert!(read_frame(&mut [0_u8; 4].as_slice()).is_err());
    }

    #[test]
    fn framed_control_payload_roundtrips_and_truncation_fails() {
        let mut frame = Vec::new();
        write_frame(&mut frame, b"public-status").unwrap();
        assert_eq!(read_frame(&mut frame.as_slice()).unwrap(), b"public-status");
        frame.pop();
        assert!(read_frame(&mut frame.as_slice()).is_err());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn native_control_roundtrips_typed_attachment_observations() {
        let _resources = super::super::native_test_guard();
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("network")).unwrap();
        let listener = Listener::bind(&directory).unwrap();
        let expected = ManagedAttachmentStatus {
            network: "fixture".into(),
            stage: ManagedAttachmentPhase::Connecting,
            wallet_status: None,
            local_successor: None,
            parent_confirmed: None,
            failure: None,
        };
        let reply = expected.clone();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(mut connection) = listener.accept().unwrap() {
                    assert_eq!(connection.receive().unwrap().action, "attachment_status");
                    connection.reply(&reply).unwrap();
                    return;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let actual: ManagedAttachmentStatus = request_as(
            &directory,
            &ControlRequest {
                token: "c".repeat(64),
                action: "attachment_status".into(),
            },
        )
        .unwrap();
        assert_eq!(actual, expected);
        server.join().unwrap();
    }
}
