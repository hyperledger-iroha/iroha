//! Tie the shared listener handoff to the exact addresses of one prepared generation.

use super::{Error, MAX_METADATA, PreparedLocalnet, Result};
use iroha_fs::PrivateDirectory;
pub(super) use iroha_managed_ipc::{FrontendListeners, WorkerListeners};
use iroha_managed_ipc::{PEER_COUNT, validate_addresses};
use std::net::SocketAddr;

/// Adopt the frontend's listeners, refusing any socket but this generation's reserved addresses.
pub(super) fn receive_worker_listeners(
    directory: &PrivateDirectory,
    prepared: &PreparedLocalnet,
    deadline_ns: u128,
) -> Result<WorkerListeners> {
    let expected = expected_addresses(prepared)?;
    Ok(WorkerListeners::receive(directory, &expected, deadline_ns)?)
}

/// Read the exact managed configuration addresses in reservation order: Torii, then P2P.
pub(super) fn expected_addresses(prepared: &PreparedLocalnet) -> Result<Vec<SocketAddr>> {
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

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::managed::{ManagedContext, ManagedPeer, decode, encode};
    use iroha_fs::PublishMode;
    use iroha_managed_ipc::adopt_runtime_listeners;
    use std::{
        io::Read as _,
        net::{Ipv4Addr, TcpListener},
        path::Path,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    const LISTENER_COUNT: usize = PEER_COUNT * 2;

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
        let held = receive_worker_listeners(&directory, &prepared, deadline_ns).unwrap();
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
