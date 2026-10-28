//! Preserve the original bound loopback sockets through frontend, worker and daemon startup.

use super::{
    ControlAction, ControlRequest, Error, MAX_METADATA, POLL, PreparedLocalnet, Result, decode,
    encode, runtime, store, transport,
};
use iroha_fs::{PrivateDirectory, PublishMode};
use norito::json::{JsonDeserialize, JsonSerialize};
use std::{
    collections::BTreeSet,
    net::{SocketAddr, TcpListener},
    path::Path,
    process::Child,
    time::Instant,
};

const STARTUP: &str = "startup-listeners";
const PEERS: &str = "peer-listeners";
const OWNER: &str = "owner.json";
const PEER_COUNT: usize = 4;
const LISTENER_COUNT: usize = PEER_COUNT * 2;

#[derive(PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ListenerRecord {
    token: String,
    pid: u32,
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Adopted {
    count: usize,
}

struct Sender {
    directory: PrivateDirectory,
    listener: transport::Listener,
    record: ListenerRecord,
}

impl Sender {
    fn bind(parent: &PrivateDirectory, name: &str) -> Result<Self> {
        let directory = parent.ensure_child(name)?;
        // The caller already owns the original context's startup/runtime lock.
        // This dedicated endpoint is never shared with status or stop requests.
        transport::clear_stopped_endpoint(&directory)?;
        let listener = transport::Listener::bind(&directory)?;
        let record = ListenerRecord {
            token: store::random_token(),
            pid: std::process::id(),
        };
        directory.write_atomic(
            OWNER,
            &zeroize::Zeroizing::new(encode(&record)?),
            PublishMode::Replace,
        )?;
        Ok(Self {
            directory,
            listener,
            record,
        })
    }

    fn send(&self, child: &mut Child, listeners: &[TcpListener], deadline: Instant) -> Result<()> {
        validate_listener_addresses(listeners, None)?;
        loop {
            check_deadline(deadline)?;
            require_live_child(child)?;
            self.directory.revalidate()?;
            if let Some(mut connection) = self.listener.accept()? {
                connection.set_deadline(deadline)?;
                if connection.peer_process_id()? != child.id() {
                    return Err(invalid(
                        "listener adoption requester is not the owned child",
                    ));
                }
                let request = connection.receive()?;
                if request.action != ControlAction::ListenerAdoption
                    || !runtime::same_token(&request.token, &self.record.token)
                {
                    return Err(invalid("listener adoption request was not authenticated"));
                }
                require_live_child(child)?;
                require_record(&self.directory, &self.record)?;
                check_deadline(deadline)?;
                connection.send_tcp_listeners(listeners)?;
                let acknowledged: Adopted = connection.receive_reply()?;
                if acknowledged.count != listeners.len() {
                    return Err(invalid(
                        "listener adoption acknowledgement has the wrong count",
                    ));
                }
                require_record(&self.directory, &self.record)?;
                require_live_child(child)?;
                check_deadline(deadline)?;
                return Ok(());
            }
            wait_poll(deadline)?;
        }
    }
}

/// Frontend-owned handoff endpoint established before its native worker is spawned.
pub(super) struct FrontendListeners(Sender);

impl FrontendListeners {
    pub(super) fn bind(directory: &PrivateDirectory) -> Result<Self> {
        Ok(Self(Sender::bind(directory, STARTUP)?))
    }

    pub(super) fn send_to_worker(
        &self,
        child: &mut Child,
        listeners: &[TcpListener],
        deadline: Instant,
    ) -> Result<()> {
        if listeners.len() != LISTENER_COUNT {
            return Err(invalid(
                "managed startup requires exactly eight reserved listeners",
            ));
        }
        self.0.send(child, listeners, deadline)
    }
}

/// The worker retains all original listeners throughout its owned validator lifetime.
pub(super) struct WorkerListeners {
    sender: Sender,
    listeners: Vec<TcpListener>,
}

impl WorkerListeners {
    pub(super) fn receive(
        directory: &PrivateDirectory,
        prepared: &PreparedLocalnet,
        deadline_ns: u128,
    ) -> Result<Self> {
        let expected = expected_addresses(prepared)?;
        let source = directory.open_child(STARTUP)?;
        let listeners = receive_listeners(&source, &expected, deadline_ns)?;
        let sender = Sender::bind(directory, PEERS)?;
        continuous_deadline(deadline_ns)?;
        Ok(Self { sender, listeners })
    }

    pub(super) fn directory(&self) -> &Path {
        self.sender.directory.path()
    }

    pub(super) fn send_to_child(
        &self,
        index: usize,
        child: &mut Child,
        deadline: Instant,
    ) -> Result<()> {
        if index >= PEER_COUNT {
            return Err(invalid("managed listener peer index must be 0..3"));
        }
        // The original reservation uses Torii first; daemon adoption explicitly names P2P first.
        let listeners = [
            self.listeners[PEER_COUNT + index].try_clone()?,
            self.listeners[index].try_clone()?,
        ];
        self.sender.send(child, &listeners, deadline)
    }
}

/// Original bound TCP listeners adopted by one managed daemon, never rebound by address.
pub struct RuntimeListeners {
    /// The original P2P listener.
    pub p2p: TcpListener,
    /// The original Torii listener.
    pub torii: TcpListener,
}

/// Adopt the worker's two listeners through its owner-authenticated native endpoint.
///
/// # Errors
/// Refuses changed ownership, an expired original deadline, a foreign sender or mismatched sockets.
pub fn adopt_runtime_listeners(
    directory: &Path,
    p2p: SocketAddr,
    torii: SocketAddr,
    deadline_ns: u128,
) -> Result<RuntimeListeners> {
    validate_addresses(&[p2p, torii])?;
    let directory = PrivateDirectory::open(directory)?;
    let mut listeners = receive_listeners(&directory, &[p2p, torii], deadline_ns)?.into_iter();
    Ok(RuntimeListeners {
        p2p: listeners.next().expect("validated P2P listener"),
        torii: listeners.next().expect("validated Torii listener"),
    })
}

fn receive_listeners(
    directory: &PrivateDirectory,
    expected: &[SocketAddr],
    deadline_ns: u128,
) -> Result<Vec<TcpListener>> {
    validate_addresses(expected)?;
    let record = read_record(directory)?;
    let mut connection = loop {
        let deadline = continuous_deadline(deadline_ns)?;
        match transport::connect_until(directory, deadline) {
            Ok(connection) => break connection,
            Err(Error::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                wait_poll(deadline)?;
            }
            Err(error) => return Err(error),
        }
    };
    connection.set_deadline(continuous_deadline(deadline_ns)?)?;
    if connection.peer_process_id()? != record.pid {
        return Err(invalid(
            "listener adoption endpoint belongs to another process",
        ));
    }
    require_record(directory, &record)?;
    connection.send_request(&ControlRequest {
        token: record.token.clone(),
        action: ControlAction::ListenerAdoption,
    })?;
    let listeners = connection.receive_tcp_listeners(expected.len())?;
    validate_listener_addresses(&listeners, Some(expected))?;
    require_record(directory, &record)?;
    connection.set_deadline(continuous_deadline(deadline_ns)?)?;
    connection.reply(&Adopted {
        count: listeners.len(),
    })?;
    continuous_deadline(deadline_ns)?;
    Ok(listeners)
}

/// Read the exact managed configuration addresses in reservation order: Torii, then P2P.
pub(crate) fn expected_addresses(prepared: &PreparedLocalnet) -> Result<Vec<SocketAddr>> {
    if prepared.peers.len() != PEER_COUNT {
        return Err(invalid(
            "managed listener custody requires exactly four validators",
        ));
    }
    let mut torii = Vec::with_capacity(PEER_COUNT);
    let mut p2p = Vec::with_capacity(PEER_COUNT);
    for peer in &prepared.peers {
        let bytes = iroha_fs::read_private(&peer.config_path, MAX_METADATA)?;
        let source = std::str::from_utf8(&bytes)
            .map_err(|_| invalid("managed listener configuration is not UTF-8"))?;
        let table = crate::secret_toml::Table::new(
            crate::secret_toml::parse_table(source, "managed listener configuration")
                .map_err(|_| invalid("managed listener configuration is not valid TOML"))?,
        );
        let api = configuration_address(&table, "torii")?;
        let network = configuration_address(&table, "network")?;
        if peer.torii_url != format!("http://{api}/") {
            return Err(invalid(
                "managed Torii URL differs from its reserved listener",
            ));
        }
        torii.push(api);
        p2p.push(network);
    }
    torii.extend(p2p);
    validate_addresses(&torii)?;
    Ok(torii)
}

fn configuration_address(table: &toml::Table, section: &str) -> Result<SocketAddr> {
    let value = table
        .get(section)
        .and_then(toml::Value::as_table)
        .and_then(|section| section.get("address"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| invalid("managed listener configuration is missing its address"))?;
    let address: iroha_primitives::addr::SocketAddr =
        norito::json::from_value(norito::json::Value::String(value.to_owned()))
            .map_err(|_| invalid("managed listener address is not canonical"))?;
    let iroha_primitives::addr::SocketAddr::Ipv4(address) = address else {
        return Err(invalid(
            "managed listeners require numeric IPv4 loopback addresses",
        ));
    };
    let address = SocketAddr::V4(address.into());
    validate_addresses(&[address])?;
    Ok(address)
}

fn validate_addresses(addresses: &[SocketAddr]) -> Result<()> {
    let mut seen = BTreeSet::new();
    if addresses.is_empty() || addresses.len() > LISTENER_COUNT {
        return Err(invalid("managed listener address count is invalid"));
    }
    for address in addresses {
        if !address.is_ipv4() || !address.ip().is_loopback() || address.port() == 0 {
            return Err(invalid(
                "managed listeners require bound numeric IPv4 loopback addresses",
            ));
        }
        if !seen.insert(*address) {
            return Err(invalid("managed listener addresses must be distinct"));
        }
    }
    Ok(())
}

fn validate_listener_addresses(
    listeners: &[TcpListener],
    expected: Option<&[SocketAddr]>,
) -> Result<()> {
    let addresses = listeners
        .iter()
        .map(TcpListener::local_addr)
        .collect::<std::io::Result<Vec<_>>>()?;
    validate_addresses(&addresses)?;
    if expected.is_some_and(|expected| addresses != expected) {
        return Err(invalid(
            "adopted listeners differ from the exact generated addresses",
        ));
    }
    Ok(())
}

fn read_record(directory: &PrivateDirectory) -> Result<ListenerRecord> {
    let record: ListenerRecord = decode(&directory.read(OWNER, MAX_METADATA)?)?;
    if record.pid == 0
        || record.token.len() != 64
        || !record
            .token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("managed listener ownership record is invalid"));
    }
    Ok(record)
}

fn require_record(directory: &PrivateDirectory, expected: &ListenerRecord) -> Result<()> {
    let actual = read_record(directory)?;
    if actual.pid != expected.pid || !runtime::same_token(&actual.token, &expected.token) {
        return Err(invalid(
            "managed listener ownership changed during adoption",
        ));
    }
    Ok(())
}

fn require_live_child(child: &mut Child) -> Result<()> {
    if child.try_wait()?.is_some() {
        return Err(invalid(
            "owned child exited before listener adoption completed",
        ));
    }
    Ok(())
}

fn check_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::NativeDeadline)
    } else {
        Ok(())
    }
}

fn continuous_deadline(deadline_ns: u128) -> Result<Instant> {
    let observed = Instant::now();
    let remaining = runtime::continuous_remaining(deadline_ns)?.ok_or(Error::NativeDeadline)?;
    observed
        .checked_add(remaining)
        .ok_or_else(|| invalid("managed listener deadline overflow"))
}

fn wait_poll(deadline: Instant) -> Result<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or(Error::NativeDeadline)?;
    std::thread::sleep(POLL.min(remaining));
    check_deadline(deadline)
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::{ManagedContext, ManagedPeer};
    use std::{
        io::Read as _,
        net::Ipv4Addr,
        process::{Command, Stdio},
        time::Duration,
    };

    fn fixture(directory: &PrivateDirectory) -> (PreparedLocalnet, Vec<TcpListener>) {
        let listeners = (0..LISTENER_COUNT)
            .map(|_| TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap())
            .collect::<Vec<_>>();
        let addresses = listeners
            .iter()
            .map(|socket| socket.local_addr().unwrap())
            .collect::<Vec<_>>();
        let peers = (0..PEER_COUNT)
            .map(|index| {
                let filename = format!("peer{index}.toml");
                let api = iroha_primitives::addr::SocketAddr::from(addresses[index]).to_literal();
                let p2p = iroha_primitives::addr::SocketAddr::from(addresses[PEER_COUNT + index])
                    .to_literal();
                let config =
                    format!("[network]\naddress = \"{p2p}\"\n[torii]\naddress = \"{api}\"\n");
                directory
                    .write_atomic(&filename, config.as_bytes(), PublishMode::CreateNew)
                    .unwrap();
                ManagedPeer {
                    config_path: directory.path().join(filename),
                    torii_url: format!("http://{}/", addresses[index]),
                    log_name: format!("peer{index}.log"),
                }
            })
            .collect::<Vec<_>>();
        (
            PreparedLocalnet {
                service_profile: crate::localnet::LocalnetServiceProfile::Standard,
                context: ManagedContext {
                    name: "listener-fixture".into(),
                    chain_id: "fixture".into(),
                    network_id: "fixture".into(),
                    account_id: "fixture".into(),
                    dataspace_id: 0,
                    dataspace_alias: "universal".into(),
                    torii_url: peers[0].torii_url.clone(),
                    client_config: directory.path().join("client.toml"),
                },
                peers,
            },
            listeners,
        )
    }

    #[test]
    fn configuration_addresses_preserve_exact_reservation_order_and_refuse_substitution() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let (mut prepared, listeners) = fixture(&directory);
        let expected = listeners
            .iter()
            .map(|socket| socket.local_addr().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(expected_addresses(&prepared).unwrap(), expected);
        prepared.peers[0].torii_url = prepared.peers[1].torii_url.clone();
        assert!(expected_addresses(&prepared).is_err());
        prepared.peers[0].torii_url = format!("http://{}/", expected[0]);
        prepared.peers[1].config_path = prepared.peers[0].config_path.clone();
        prepared.peers[1].torii_url = prepared.peers[0].torii_url.clone();
        assert!(expected_addresses(&prepared).is_err());
        prepared.peers.pop();
        assert!(expected_addresses(&prepared).is_err());
    }

    #[test]
    fn listener_addresses_refuse_nonlocal_zero_ports_duplicates_and_foreign_order() {
        for address in [
            "0.0.0.0:1234",
            "127.0.0.1:0",
            "[::1]:1234",
            "192.0.2.1:1234",
        ] {
            assert!(validate_addresses(&[address.parse().unwrap()]).is_err());
        }
        assert!(validate_addresses(&[]).is_err());
        let first = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let second = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        assert!(validate_addresses(&[first.local_addr().unwrap(); 2]).is_err());
        let foreign = [second.local_addr().unwrap(), first.local_addr().unwrap()];
        assert!(validate_listener_addresses(&[first, second], Some(&foreign)).is_err());
        assert!(matches!(continuous_deadline(0), Err(Error::NativeDeadline)));
        assert!(matches!(
            check_deadline(Instant::now()),
            Err(Error::NativeDeadline)
        ));
        let native_now = iroha_primitives::time::native_continuous_clock_nanos().unwrap();
        let observed = Instant::now();
        let budget = Duration::from_secs(10);
        let deadline = continuous_deadline(native_now + budget.as_nanos()).unwrap();
        assert!(deadline > observed);
        assert!(deadline <= observed + budget);
    }

    #[test]
    fn listener_configuration_errors_never_expose_secret_toml() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let (prepared, _listeners) = fixture(&directory);
        directory
            .write_atomic(
                "peer0.toml",
                b"private_key = \"SECRET\n",
                PublishMode::Replace,
            )
            .unwrap();
        assert!(
            !expected_addresses(&prepared)
                .unwrap_err()
                .to_string()
                .contains("SECRET")
        );
        for literal in ["127.0.0.1:1234", "addr:localhost:1234#0000"] {
            let table = toml::Table::from_iter([(
                "network".into(),
                toml::Value::Table(toml::Table::from_iter([("address".into(), literal.into())])),
            )]);
            assert!(configuration_address(&table, "network").is_err());
        }
    }

    #[test]
    fn listener_records_reject_changed_ownership_and_invalid_tokens() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let sender = FrontendListeners::bind(&directory).unwrap();
        let record = read_record(&sender.0.directory).unwrap();
        require_record(&sender.0.directory, &record).unwrap();
        let changed = ListenerRecord {
            token: store::random_token(),
            pid: record.pid,
        };
        sender
            .0
            .directory
            .write_atomic(OWNER, &encode(&changed).unwrap(), PublishMode::Replace)
            .unwrap();
        assert!(require_record(&sender.0.directory, &record).is_err());
        let invalid = ListenerRecord {
            token: "short".into(),
            pid: 0,
        };
        sender
            .0
            .directory
            .write_atomic(OWNER, &encode(&invalid).unwrap(), PublishMode::Replace)
            .unwrap();
        assert!(read_record(&sender.0.directory).is_err());
    }

    #[test]
    fn native_listener_handoff_retains_original_ports_through_worker_and_daemon() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
        let (prepared, listeners) = fixture(&directory);
        directory
            .write_atomic(
                "fixture.json",
                &encode(&prepared).unwrap(),
                PublishMode::CreateNew,
            )
            .unwrap();
        let sender = FrontendListeners::bind(&directory).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "managed::listeners::tests::worker_receiver_child",
                "--ignored",
                "--nocapture",
            ])
            .env("IROHA_LISTENER_TEST_DIRECTORY", directory.path())
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let sent = sender.send_to_worker(
            &mut child,
            &listeners,
            Instant::now() + Duration::from_secs(10),
        );
        drop(child.stdin.take());
        let status = child.wait().unwrap();
        sent.unwrap();
        assert!(status.success());
        for socket in &listeners {
            assert!(
                TcpListener::bind(socket.local_addr().unwrap()).is_err(),
                "sender retains every original bound port"
            );
        }
    }

    #[test]
    #[ignore = "internal subprocess owned by native_listener_handoff_retains_original_ports_through_worker_and_daemon"]
    fn worker_receiver_child() {
        let path = std::env::var_os("IROHA_LISTENER_TEST_DIRECTORY").unwrap();
        let directory = PrivateDirectory::open(Path::new(&path)).unwrap();
        let prepared: PreparedLocalnet =
            decode(&directory.read("fixture.json", MAX_METADATA).unwrap()).unwrap();
        let deadline_ns = iroha_primitives::time::native_continuous_clock_nanos().unwrap()
            + Duration::from_secs(8).as_nanos();
        let held = WorkerListeners::receive(&directory, &prepared, deadline_ns).unwrap();
        let expected = expected_addresses(&prepared).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "managed::listeners::tests::runtime_receiver_child",
                "--ignored",
                "--nocapture",
            ])
            .env("IROHA_LISTENER_TEST_DIRECTORY", held.directory())
            .env("IROHA_LISTENER_TEST_P2P", expected[PEER_COUNT].to_string())
            .env("IROHA_LISTENER_TEST_TORII", expected[0].to_string())
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let sent = held.send_to_child(0, &mut child, Instant::now() + Duration::from_secs(5));
        drop(child.stdin.take());
        let status = child.wait().unwrap();
        sent.unwrap();
        assert!(status.success());
        assert!(
            held.send_to_child(
                PEER_COUNT,
                &mut child,
                Instant::now() + Duration::from_secs(1)
            )
            .is_err()
        );
        assert!(require_live_child(&mut child).is_err());
        std::io::stdin().read_to_end(&mut Vec::new()).unwrap();
    }

    #[test]
    #[ignore = "internal subprocess owned by worker_receiver_child"]
    fn runtime_receiver_child() {
        let directory = std::env::var_os("IROHA_LISTENER_TEST_DIRECTORY").unwrap();
        let p2p = std::env::var("IROHA_LISTENER_TEST_P2P")
            .unwrap()
            .parse()
            .unwrap();
        let torii = std::env::var("IROHA_LISTENER_TEST_TORII")
            .unwrap()
            .parse()
            .unwrap();
        let deadline_ns = iroha_primitives::time::native_continuous_clock_nanos().unwrap()
            + Duration::from_secs(4).as_nanos();
        let adopted =
            adopt_runtime_listeners(Path::new(&directory), p2p, torii, deadline_ns).unwrap();
        assert_eq!(adopted.p2p.local_addr().unwrap(), p2p);
        assert_eq!(adopted.torii.local_addr().unwrap(), torii);
        // Only the sender closes this inherited pipe, after it has checked the acknowledgement.
        std::io::stdin().read_to_end(&mut Vec::new()).unwrap();
    }
}
