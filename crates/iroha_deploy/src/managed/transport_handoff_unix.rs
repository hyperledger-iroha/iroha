// Native descriptor transfer over the original owner-authenticated control stream.

impl Connection {
    pub(crate) fn peer_process_id(&self) -> Result<u32> {
        authenticate_peer(&self.0)?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            Ok(rustix::net::sockopt::socket_peercred(&self.0)
                .map_err(std::io::Error::from)?
                .pid
                .as_raw_nonzero()
                .get() as u32)
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd as _;
            let mut pid: libc::pid_t = 0;
            let mut length = std::mem::size_of_val(&pid) as libc::socklen_t;
            #[allow(
                unsafe_code,
                reason = "getsockopt reads the live Unix peer PID into a valid bounded output"
            )]
            let result = unsafe {
                libc::getsockopt(
                    self.0.as_raw_fd(),
                    libc::SOL_LOCAL,
                    libc::LOCAL_PEERPID,
                    (&raw mut pid).cast(),
                    &raw mut length,
                )
            };
            if result != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            if length as usize != std::mem::size_of_val(&pid) || pid <= 0 {
                return Err(Error::Invalid("managed Unix peer PID is invalid".into()));
            }
            Ok(pid as u32)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos")))]
        Err(Error::Invalid(
            "managed peer PID is unavailable on this Unix platform".into(),
        ))
    }

    pub(crate) fn send_tcp_listeners(&mut self, listeners: &[std::net::TcpListener]) -> Result<()> {
        use rustix::{
            event::PollFlags,
            io::Errno,
            net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg},
        };
        use std::{io::IoSlice, mem::MaybeUninit, os::fd::AsFd as _};
        listener_count(listeners.len())?;
        self.peer_process_id()?;
        for listener in listeners {
            validate_tcp_listener(listener)?;
        }
        let deadline = self.io_deadline();
        let bounded = DeadlineStream::new(self.0.try_clone()?, deadline)?;
        let descriptors: Vec<_> = listeners.iter().map(|listener| listener.as_fd()).collect();
        let count = [u8::try_from(listeners.len()).expect("bounded listener count")];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_LISTENERS))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(Error::Invalid(
                "managed descriptor frame exceeds its bound".into(),
            ));
        }
        let flags = SendFlags::DONTWAIT;
        #[cfg(not(any(target_vendor = "apple", target_os = "redox", target_os = "vita")))]
        let flags = flags | SendFlags::NOSIGNAL;
        loop {
            remaining_io(deadline)?;
            match sendmsg(
                &bounded.stream,
                &[IoSlice::new(&count)],
                &mut ancillary,
                flags,
            ) {
                Ok(1) => {
                    remaining_io(deadline)?;
                    return Ok(());
                }
                Ok(_) => {
                    return Err(Error::Invalid(
                        "incomplete managed descriptor transfer".into(),
                    ));
                }
                Err(Errno::AGAIN) => bounded.wait_ready(PollFlags::OUT)?,
                Err(Errno::INTR) => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
    }

    pub(crate) fn receive_tcp_listeners(
        &mut self,
        expected_count: usize,
    ) -> Result<Vec<std::net::TcpListener>> {
        use rustix::{
            event::PollFlags,
            io::{Errno, FdFlags, fcntl_setfd},
            net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg},
        };
        use std::{io::IoSliceMut, mem::MaybeUninit};
        listener_count(expected_count)?;
        self.peer_process_id()?;
        let deadline = self.io_deadline();
        let bounded = DeadlineStream::new(self.0.try_clone()?, deadline)?;
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(MAX_LISTENERS))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let mut count = [0_u8];
        let flags = RecvFlags::DONTWAIT;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let flags = flags | RecvFlags::CMSG_CLOEXEC;
        let received = loop {
            remaining_io(deadline)?;
            match recvmsg(
                &bounded.stream,
                &mut [IoSliceMut::new(&mut count)],
                &mut ancillary,
                flags,
            ) {
                Ok(received) => break received,
                Err(Errno::AGAIN) => bounded.wait_ready(PollFlags::IN)?,
                Err(Errno::INTR) => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        };
        // Own every received descriptor before validation; every early return drops all
        // of them, including extra descriptors and truncated ancillary messages.
        let mut descriptors = Vec::new();
        let mut unexpected = false;
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => unexpected = true,
            }
        }
        for descriptor in &descriptors {
            fcntl_setfd(descriptor, FdFlags::CLOEXEC).map_err(std::io::Error::from)?;
        }
        if received.bytes != 1
            || received
                .flags
                .intersects(ReturnFlags::CTRUNC | ReturnFlags::TRUNC)
            || unexpected
            || usize::from(count[0]) != expected_count
            || descriptors.len() != expected_count
        {
            return Err(Error::Invalid(
                "managed descriptor frame has an invalid listener count".into(),
            ));
        }
        let mut listeners = Vec::with_capacity(expected_count);
        for descriptor in descriptors {
            let listener = std::net::TcpListener::from(descriptor);
            validate_tcp_listener(&listener)?;
            listener.set_nonblocking(true)?;
            listeners.push(listener);
        }
        remaining_io(deadline)?;
        Ok(listeners)
    }
}

fn validate_tcp_listener(listener: &std::net::TcpListener) -> Result<()> {
    if rustix::net::sockopt::socket_type(listener).map_err(std::io::Error::from)?
        != rustix::net::SocketType::STREAM
        || listener.local_addr()?.port() == 0
    {
        return Err(Error::Invalid(
            "managed descriptor is not a bound TCP listener".into(),
        ));
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    if rustix::net::sockopt::socket_protocol(listener).map_err(std::io::Error::from)?
        != Some(rustix::net::ipproto::TCP)
    {
        return Err(Error::Invalid(
            "managed listener protocol is not TCP".into(),
        ));
    }
    #[cfg(not(target_vendor = "apple"))]
    let listening =
        rustix::net::sockopt::socket_acceptconn(listener).map_err(std::io::Error::from)?;
    #[cfg(target_os = "macos")]
    let listening = {
        use std::os::fd::AsRawFd as _;
        // Only the first u_int8_t, tcpi_state, is needed. Read its documented prefix
        // directly: libc's full record expands C bitfields and differs from Darwin's ABI.
        let mut state = 0_u8;
        let mut length = std::mem::size_of_val(&state) as libc::socklen_t;
        #[allow(
            unsafe_code,
            reason = "getsockopt initializes the bounded TCP state output before its state byte is read"
        )]
        unsafe {
            if libc::getsockopt(
                listener.as_raw_fd(),
                libc::IPPROTO_TCP,
                libc::TCP_CONNECTION_INFO,
                (&raw mut state).cast(),
                &raw mut length,
            ) != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
            if length as usize != std::mem::size_of_val(&state) {
                return Err(Error::Invalid(
                    "managed TCP state output is incomplete".into(),
                ));
            }
            // Darwin does not implement SO_ACCEPTCONN. Its native TCP FSM declares
            // TCPS_LISTEN = 1; inspect that state without accepting any queued connection.
            state == 1
        }
    };
    #[cfg(all(target_vendor = "apple", not(target_os = "macos")))]
    let listening = false;
    if !listening {
        return Err(Error::Invalid("managed TCP socket is not listening".into()));
    }
    Ok(())
}

#[cfg(test)]
mod listener_handoff_tests {
    use super::*;
    use std::{
        net::{Ipv4Addr, TcpListener},
        os::fd::AsFd as _,
    };

    fn pair() -> (Connection, Connection) {
        let (left, right) = UnixStream::pair().unwrap();
        let deadline = Instant::now() + IO_TIMEOUT;
        (
            Connection(left, Some(deadline)),
            Connection(right, Some(deadline)),
        )
    }

    #[test]
    fn listener_roundtrip_retains_bound_ports_until_every_owner_closes() {
        let (mut sender, mut receiver) = pair();
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
        drop(originals);
        for address in &addresses {
            assert!(TcpListener::bind(address).is_err());
        }
        let adopted = receiver.receive_tcp_listeners(8).unwrap();
        for (listener, address) in adopted.iter().zip(&addresses) {
            assert_eq!(listener.local_addr().unwrap(), *address);
            assert!(
                rustix::io::fcntl_getfd(listener)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
            assert!(TcpListener::bind(address).is_err());
        }
        drop(adopted);
        for address in addresses {
            assert!(TcpListener::bind(address).is_ok());
        }
    }

    #[test]
    fn extra_descriptors_and_non_listener_descriptors_are_closed_on_refusal() {
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
        use std::{io::IoSlice, mem::MaybeUninit};
        let (mut sender, mut receiver) = pair();
        let originals = (0..2)
            .map(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap())
            .collect::<Vec<_>>();
        let addresses = originals
            .iter()
            .map(|listener| listener.local_addr().unwrap())
            .collect::<Vec<_>>();
        sender.send_tcp_listeners(&originals).unwrap();
        drop(originals);
        assert!(receiver.receive_tcp_listeners(1).is_err());
        for address in addresses {
            assert!(TcpListener::bind(address).is_ok());
        }

        let (sender, mut receiver) = pair();
        let file = std::fs::File::open("/dev/null").unwrap();
        let descriptors = [file.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
        assert_eq!(
            sendmsg(
                &sender.0,
                &[IoSlice::new(&[1])],
                &mut ancillary,
                SendFlags::empty()
            )
            .unwrap(),
            1
        );
        assert!(receiver.receive_tcp_listeners(1).is_err());
        assert!(sender.0.peer_addr().is_ok());
    }

    #[test]
    fn invalid_counts_and_original_deadline_refuse_before_transfer() {
        let (mut sender, mut receiver) = pair();
        assert!(sender.send_tcp_listeners(&[]).is_err());
        assert!(receiver.receive_tcp_listeners(0).is_err());
        assert!(receiver.receive_tcp_listeners(9).is_err());
        assert!(sender.set_deadline(Instant::now()).is_err());
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        assert!(sender.send_tcp_listeners(&[listener]).is_err());
    }

    #[test]
    fn owned_endpoint_exchanges_requests_and_keeps_one_absolute_deadline() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("handoff")).unwrap();
        let server = Listener::bind(&directory).unwrap();
        let mut client = connect_until(&directory, Instant::now() + IO_TIMEOUT).unwrap();
        let mut accepted = server.accept().unwrap().unwrap();
        let request = ControlRequest {
            token: "a".repeat(64),
            action: ControlAction::Status,
        };
        client.send_request(&request).unwrap();
        let received = accepted.receive().unwrap();
        assert_eq!(received.token, request.token);
        accepted.reply(&received).unwrap();
        let reply: ControlRequest = client.receive_reply().unwrap();
        assert_eq!(reply.action, request.action);
        let original = client.1.unwrap();
        client.set_deadline(original + IO_TIMEOUT).unwrap();
        assert_eq!(client.1, Some(original));
        assert!(connect_until(&directory, Instant::now()).is_err());
    }

    #[test]
    fn truncated_rights_close_all_received_descriptors_and_unlistening_tcp_refuses() {
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
        use std::{io::IoSlice, mem::MaybeUninit};
        let (sender, mut receiver) = pair();
        let originals = (0..9)
            .map(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap())
            .collect::<Vec<_>>();
        let addresses = originals
            .iter()
            .map(|listener| listener.local_addr().unwrap())
            .collect::<Vec<_>>();
        let descriptors = originals
            .iter()
            .map(|listener| listener.as_fd())
            .collect::<Vec<_>>();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(9))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
        assert_eq!(
            sendmsg(
                &sender.0,
                &[IoSlice::new(&[9])],
                &mut ancillary,
                SendFlags::empty()
            )
            .unwrap(),
            1
        );
        drop(ancillary);
        drop(descriptors);
        drop(originals);
        assert!(receiver.receive_tcp_listeners(8).is_err());
        for address in addresses {
            assert!(TcpListener::bind(address).is_ok());
        }

        let socket = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .unwrap();
        socket
            .bind(&std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, 0)).into())
            .unwrap();
        let unlistening = TcpListener::from(socket);
        assert!(validate_tcp_listener(&unlistening).is_err());
        socket2::SockRef::from(&unlistening).listen(1).unwrap();
        validate_tcp_listener(&unlistening).unwrap();
    }
}
