//! Deadline exhaustion cannot reopen socket work or retire a queued operation.

use super::*;

fn expired() -> BrokerDeadlineV1 {
    BrokerDeadlineV1 {
        expires_at: Instant::now().checked_sub(Duration::from_secs(1)).unwrap(),
    }
}

#[test]
fn invalid_or_expired_deadlines_reject_even_an_uncontended_lock() {
    assert!(BrokerDeadlineV1::new(Duration::ZERO).is_err());
    assert!(BrokerDeadlineV1::new(Duration::MAX).is_err());
    let state = Mutex::new(7);
    assert!(expired().remaining().is_err());
    assert!(expired().lock(&state).is_err());
    assert_eq!(*state.lock().unwrap(), 7);
    let deadline = BrokerDeadlineV1::new(Duration::from_secs(5)).unwrap();
    *deadline.lock(&state).unwrap() = 8;
    assert_eq!(*state.lock().unwrap(), 8);
}

#[test]
fn expired_socket_views_do_not_consume_input_or_emit_output() {
    let (mut local, mut peer) = UnixStream::pair().unwrap();
    peer.write_all(b"retained").unwrap();
    let mut view = DeadlineUnixStreamV1::new(&mut local, expired());
    let mut byte = [0_u8; 1];
    assert_eq!(
        view.read(&mut byte).unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(
        view.write(b"forbidden").unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
    assert_eq!(view.flush().unwrap_err().kind(), io::ErrorKind::TimedOut);
    let mut input = [0_u8; 8];
    local.read_exact(&mut input).unwrap();
    assert_eq!(&input, b"retained");
    peer.set_nonblocking(true).unwrap();
    assert_eq!(
        peer.read(&mut byte).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn one_deadline_survives_distinct_framing_views_without_renewal() {
    let (mut local, mut peer) = UnixStream::pair().unwrap();
    let deadline = BrokerDeadlineV1::new(Duration::from_secs(5)).unwrap();
    let original_endpoint = deadline.expires_at();
    DeadlineUnixStreamV1::new(&mut local, deadline)
        .write_all(b"one")
        .unwrap();
    let mut first = [0_u8; 3];
    peer.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"one");
    peer.write_all(b"two").unwrap();
    let mut second = [0_u8; 3];
    DeadlineUnixStreamV1::new(&mut local, deadline)
        .read_exact(&mut second)
        .unwrap();
    assert_eq!(&second, b"two");
    assert_eq!(deadline.expires_at(), original_endpoint);
}

#[test]
fn occupied_session_admission_ends_without_waiting_for_the_holder() {
    let state = Mutex::new(11);
    let held = state.lock().unwrap();
    let began = Instant::now();
    let deadline = BrokerDeadlineV1::new(Duration::from_millis(25)).unwrap();
    assert!(deadline.lock(&state).is_err());
    assert!(began.elapsed() < Duration::from_secs(5));
    assert_eq!(*held, 11);
}

#[test]
fn blocked_socket_read_and_write_share_the_original_expiration() {
    let (mut local, _peer) = UnixStream::pair().unwrap();
    let deadline = BrokerDeadlineV1::new(Duration::from_millis(25)).unwrap();
    let began = Instant::now();
    let mut byte = [0_u8; 1];
    assert!(
        DeadlineUnixStreamV1::new(&mut local, deadline)
            .read_exact(&mut byte)
            .is_err()
    );
    assert!(began.elapsed() < Duration::from_secs(5));
    // Some kernels round timeout granularity down; explicitly use the same now-expired endpoint
    // to prove that a later output view cannot allocate another independent timeout interval.
    while deadline.remaining().is_ok() {
        std::thread::park_timeout(Duration::from_millis(1));
    }
    assert_eq!(
        DeadlineUnixStreamV1::new(&mut local, deadline)
            .write(b"late")
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
}
