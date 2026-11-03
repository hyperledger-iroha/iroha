// Winsock duplicates are addressed to the kernel-authenticated receiving process.

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ListenerSockets {
    protocols: Vec<Vec<u8>>,
}

impl Connection {
    pub(crate) fn send_tcp_listeners(&mut self, listeners: &[std::net::TcpListener]) -> Result<()> {
        use std::os::windows::io::AsRawSocket as _;
        use windows_sys::Win32::Networking::WinSock::{WSADuplicateSocketW, WSAPROTOCOL_INFOW};
        listener_count(listeners.len())?;
        let deadline = self.io_deadline();
        self.set_deadline(deadline)?;
        let process = self.peer_process_id()?;
        let mut protocols = Vec::with_capacity(listeners.len());
        for listener in listeners {
            validate_tcp_listener(listener)?;
            // Keep this byte-initialized allocation in place: constructing or moving a typed
            // C record need not preserve initialized padding when its bytes are serialized.
            let mut protocol = std::mem::MaybeUninit::<WSAPROTOCOL_INFOW>::zeroed();
            // SAFETY: the original listener remains owned, the PID comes from this authenticated
            // pipe's kernel identity, and the complete output is initialized before the API call.
            if unsafe {
                WSADuplicateSocketW(
                    listener.as_raw_socket() as _,
                    process,
                    protocol.as_mut_ptr(),
                )
            } != 0
            {
                return Err(winsock_error().into());
            }
            // SAFETY: Winsock filled the initialized record successfully; inspecting through a
            // reference leaves its original initialized byte storage, including padding, intact.
            if unsafe { protocol.assume_init_ref() }.iProtocol
                != windows_sys::Win32::Networking::WinSock::IPPROTO_TCP
            {
                return Err(Error::Invalid(
                    "managed listener protocol is not TCP".into(),
                ));
            }
            // This opaque same-host OS record is runtime-only. Norito frames its exact bytes;
            // it is neither a portable codec nor persisted network configuration.
            // SAFETY: every byte was initialized in this allocation before the native call, and
            // the record has never been moved out as a typed value.
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    protocol.as_ptr().cast::<u8>(),
                    std::mem::size_of::<WSAPROTOCOL_INFOW>(),
                )
            };
            protocols.push(bytes.to_vec());
        }
        remaining_io(deadline)?;
        self.reply(&ListenerSockets { protocols })
    }

    pub(crate) fn receive_tcp_listeners(
        &mut self,
        expected_count: usize,
    ) -> Result<Vec<std::net::TcpListener>> {
        use std::os::windows::io::{FromRawSocket as _, OwnedSocket};
        use windows_sys::Win32::Networking::WinSock::{
            AF_INET, AF_INET6, FROM_PROTOCOL_INFO, INVALID_SOCKET, IPPROTO_TCP, SOCK_STREAM,
            WSA_FLAG_NO_HANDLE_INHERIT, WSA_FLAG_OVERLAPPED, WSAPROTOCOL_INFOW, WSASocketW,
        };
        listener_count(expected_count)?;
        let deadline = self.io_deadline();
        self.set_deadline(deadline)?;
        self.peer_process_id()?;
        let wire: ListenerSockets = self.receive_reply()?;
        listener_count(wire.protocols.len())?;
        // Use the existing socket owner to initialize process-wide Winsock through std. The
        // temporary socket is never bound and its release cannot close the adopted sockets.
        drop(socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )?);
        let mut listeners = Vec::with_capacity(wire.protocols.len());
        for bytes in wire.protocols {
            if bytes.len() != std::mem::size_of::<WSAPROTOCOL_INFOW>() {
                return Err(Error::Invalid(
                    "managed socket protocol record has an invalid length".into(),
                ));
            }
            let mut protocol = std::mem::MaybeUninit::<WSAPROTOCOL_INFOW>::zeroed();
            // SAFETY: the length is exactly the C record and the aligned destination remains
            // in place. Every field is integer/byte storage with no invalid Rust bit patterns.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    protocol.as_mut_ptr().cast::<u8>(),
                    bytes.len(),
                );
            }
            // SAFETY: all record bytes were copied above; Winsock validates opaque provider state.
            let fields = unsafe { protocol.assume_init_ref() };
            if ![i32::from(AF_INET), i32::from(AF_INET6)].contains(&fields.iAddressFamily)
                || fields.iSocketType != SOCK_STREAM
                || fields.iProtocol != IPPROTO_TCP
            {
                return Err(Error::Invalid(
                    "managed socket protocol record is not Internet TCP".into(),
                ));
            }
            // SAFETY: this exact initialized record is consumed only in the intended process;
            // neither shared HANDLE inheritance nor arbitrary process IDs are accepted.
            let socket = unsafe {
                WSASocketW(
                    FROM_PROTOCOL_INFO,
                    FROM_PROTOCOL_INFO,
                    FROM_PROTOCOL_INFO,
                    protocol.as_ptr(),
                    0,
                    WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT,
                )
            };
            if socket == INVALID_SOCKET {
                return Err(winsock_error().into());
            }
            // SAFETY: successful WSASocketW returned one newly owned socket; close it on every
            // validation/refusal path by immediately installing the standard RAII owner.
            let socket = unsafe { OwnedSocket::from_raw_socket(socket as _) };
            let listener = std::net::TcpListener::from(socket);
            validate_tcp_listener(&listener)?;
            listener.set_nonblocking(true)?;
            listeners.push(listener);
        }
        if listeners.len() != expected_count {
            return Err(Error::Invalid(
                "managed socket frame has an invalid listener count".into(),
            ));
        }
        remaining_io(deadline)?;
        Ok(listeners)
    }
}

fn winsock_error() -> io::Error {
    // SAFETY: WSAGetLastError reads this thread's last native socket error.
    io::Error::from_raw_os_error(unsafe {
        windows_sys::Win32::Networking::WinSock::WSAGetLastError()
    })
}

fn validate_tcp_listener(listener: &std::net::TcpListener) -> Result<()> {
    use std::os::windows::io::AsRawSocket as _;
    use windows_sys::Win32::Networking::WinSock::{SO_ACCEPTCONN, SOL_SOCKET, getsockopt};
    if socket2::SockRef::from(listener).r#type()? != socket2::Type::STREAM
        || listener.local_addr()?.port() == 0
    {
        return Err(Error::Invalid(
            "managed descriptor is not a bound TCP listener".into(),
        ));
    }
    let mut listening = 0_i32;
    let mut length = std::mem::size_of_val(&listening) as i32;
    // SAFETY: the live socket and the initialized, exactly-sized output remain owned throughout.
    if unsafe {
        getsockopt(
            listener.as_raw_socket() as _,
            SOL_SOCKET,
            SO_ACCEPTCONN,
            (&raw mut listening).cast(),
            &raw mut length,
        )
    } != 0
    {
        return Err(winsock_error().into());
    }
    if length as usize != std::mem::size_of_val(&listening) || listening == 0 {
        return Err(Error::Invalid("managed TCP socket is not listening".into()));
    }
    Ok(())
}

#[cfg(test)]
mod listener_handoff_tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};

    fn pair(directory: &PrivateDirectory) -> (Listener, Connection, Connection) {
        let server = Listener::bind(directory).unwrap();
        let client = connect_until(directory, Instant::now() + IO_TIMEOUT).unwrap();
        let deadline = Instant::now() + IO_TIMEOUT;
        let accepted = loop {
            if let Some(connection) = server.accept().unwrap() {
                break connection;
            }
            remaining_io(deadline).unwrap();
        };
        (server, client, accepted)
    }

    #[test]
    fn listener_roundtrip_keeps_ports_bound_across_winsock_adoption() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("handoff")).unwrap();
        let (_server, mut sender, mut receiver) = pair(&directory);
        assert_eq!(sender.peer_process_id().unwrap(), std::process::id());
        assert_eq!(receiver.peer_process_id().unwrap(), std::process::id());
        let originals = (0..8)
            .map(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap())
            .collect::<Vec<_>>();
        let addresses = originals
            .iter()
            .map(|listener| listener.local_addr().unwrap())
            .collect::<Vec<_>>();
        sender.send_tcp_listeners(&originals).unwrap();
        let adopted = receiver.receive_tcp_listeners(8).unwrap();
        drop(originals);
        for (listener, address) in adopted.iter().zip(&addresses) {
            assert_eq!(listener.local_addr().unwrap(), *address);
            assert!(TcpListener::bind(address).is_err());
        }
        drop(adopted);
        for address in addresses {
            assert!(TcpListener::bind(address).is_ok());
        }
    }

    #[test]
    fn wrong_counts_and_malformed_protocol_records_refuse() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("handoff")).unwrap();
        let (_server, mut sender, mut receiver) = pair(&directory);
        assert!(sender.send_tcp_listeners(&[]).is_err());
        assert!(receiver.receive_tcp_listeners(9).is_err());
        sender
            .reply(&ListenerSockets {
                protocols: vec![vec![0; 3]],
            })
            .unwrap();
        assert!(receiver.receive_tcp_listeners(1).is_err());
        let originals = (0..2)
            .map(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap())
            .collect::<Vec<_>>();
        let addresses = originals
            .iter()
            .map(|listener| listener.local_addr().unwrap())
            .collect::<Vec<_>>();
        sender.send_tcp_listeners(&originals).unwrap();
        assert!(receiver.receive_tcp_listeners(1).is_err());
        drop(originals);
        for address in addresses {
            assert!(TcpListener::bind(address).is_ok());
        }
        assert!(receiver.set_deadline(Instant::now()).is_err());
        assert!(receiver.receive_tcp_listeners(1).is_err());
    }
}
