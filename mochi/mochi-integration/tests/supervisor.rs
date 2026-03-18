use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    net::{SocketAddr, TcpListener},
    path::{Path, PathBuf},
    time::Duration,
};

use color_eyre::{Result, eyre::eyre};
use iroha_data_model::events::EventBox;
use mochi_core::{
    ProfilePreset, Supervisor, SupervisorBuilder,
    torii::{BlockStreamEvent, EventCategory, EventStreamEvent},
};
use mochi_integration::{MockToriiBuilder, MockToriiData};
use norito::json::Value;
use tempfile::TempDir;
use tokio::time::timeout;

fn reserve_port() -> std::io::Result<u16> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
}

fn build_supervisor_with_bases(
    temp: &TempDir,
    torii_base: u16,
    p2p_base: u16,
    preset: ProfilePreset,
) -> Result<Supervisor> {
    let kagami = env!("CARGO_BIN_EXE_kagami_mock");
    let binaries = mochi_core::BinaryPaths::default().kagami(kagami);
    let supervisor = SupervisorBuilder::new(preset)
        .data_root(temp.path())
        .torii_base_port(torii_base)
        .p2p_base_port(p2p_base)
        .binaries(binaries)
        .build()?;
    Ok(supervisor)
}

fn build_supervisor(temp: &TempDir, port: u16, preset: ProfilePreset) -> Result<Supervisor> {
    let p2p_base = port.checked_add(1_000).unwrap_or(10_000);
    build_supervisor_with_bases(temp, port, p2p_base, preset)
}

fn peer_addr(supervisor: &Supervisor) -> SocketAddr {
    supervisor.peers()[0]
        .torii_address()
        .parse()
        .expect("parse torii address")
}

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_reads_http_endpoints() -> Result<()> {
    let temp = TempDir::new()?;
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping supervisor_reads_status: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let supervisor = build_supervisor(&temp, port, ProfilePreset::SinglePeer)?;
    let addr = peer_addr(&supervisor);

    let data = MockToriiData::default();
    let mock = MockToriiBuilder::new(addr).spawn().await?;
    let client = supervisor
        .torii_client("peer0")
        .expect("supervisor exposes torii client");

    let status = client.fetch_status().await?;
    assert_eq!(status.peers, data.status.peers);

    let snapshot = client.fetch_status_snapshot().await?;
    assert_eq!(snapshot.status.blocks, data.status.blocks);

    let sumeragi = client.fetch_sumeragi_status().await?;
    assert_eq!(sumeragi.leader_index, data.sumeragi.leader_index);

    let config = client.fetch_configuration().await?;
    assert_eq!(config, data.configuration);

    let metrics = client.fetch_metrics().await?;
    assert_eq!(metrics, data.metrics);

    let query = client.submit_query(&[0xCA, 0xFE]).await?;
    assert_eq!(query, data.query_response);

    let _ = mock.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_streams_receive_binary_frames() -> Result<()> {
    let temp = TempDir::new()?;
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping supervisor_streams_receive_binary_frames: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let supervisor = build_supervisor(&temp, port, ProfilePreset::SinglePeer)?;
    let addr = peer_addr(&supervisor);

    let data = MockToriiData::default();
    let mock = MockToriiBuilder::new(addr).spawn().await?;

    let handle = tokio::runtime::Handle::current();
    let block_stream = supervisor
        .managed_block_stream("peer0", &handle)
        .expect("managed block stream");
    let mut block_rx = block_stream.subscribe();

    let event_stream = supervisor
        .managed_event_stream("peer0", &handle)
        .expect("managed event stream");
    let mut event_rx = event_stream.subscribe();

    let block_event = timeout(Duration::from_secs(1), block_rx.recv())
        .await
        .expect("block event timeout")?
        .clone();
    match block_event {
        BlockStreamEvent::Block { raw_len, .. } => {
            assert_eq!(raw_len, data.block_frame.len());
        }
        other => panic!("unexpected block stream event: {other:?}"),
    }

    let event = timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("event stream timeout")?
        .clone();
    match event {
        EventStreamEvent::Event { raw_len, .. } => {
            assert_eq!(raw_len, data.event_frame.len());
        }
        other => panic!("unexpected event stream event: {other:?}"),
    }

    block_stream.abort();
    event_stream.abort();
    let _ = mock.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn supervisor_replays_torii_fixture_streams() -> Result<()> {
    let temp = TempDir::new()?;
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping supervisor_replays_torii_fixture_streams: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let supervisor = build_supervisor(&temp, port, ProfilePreset::SinglePeer)?;
    let addr = peer_addr(&supervisor);

    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/torii_replay");
    let mock = MockToriiBuilder::new(addr)
        .fixture_dir(&fixture_dir)?
        .spawn()
        .await?;

    let client = supervisor
        .torii_client("peer0")
        .expect("supervisor exposes torii client");

    let status = client.fetch_status().await?;
    assert_eq!(status.blocks, 5);
    assert!(status.crypto.sm_helpers_available);
    assert_eq!(status.queue_size, 4);
    assert_eq!(status.governance.manifest_quorum.total_checks, 0);
    assert_eq!(status.governance.manifest_admission.total_checks, 0);

    let sumeragi = client.fetch_sumeragi_status().await?;
    assert_eq!(sumeragi.highest_qc_height, 10);
    assert_eq!(sumeragi.tx_queue_capacity, 100);

    let configuration = client.fetch_configuration().await?;
    assert_eq!(
        configuration
            .get("torii")
            .and_then(|v| v.get("address"))
            .and_then(Value::as_str),
        Some("127.0.0.1:5555")
    );

    let metrics = client.fetch_metrics().await?;
    assert!(
        metrics.contains("iroha_blocks_total"),
        "metrics fixture should surface canonical counter"
    );

    let query = client.submit_query(&[0xCA, 0xFE]).await?;
    assert_eq!(query, vec![0x13, 0x37]);

    let block_fixture = fs::read(fixture_dir.join("block.bin"))?;
    let event_fixture = fs::read(fixture_dir.join("event.bin"))?;

    let handle = tokio::runtime::Handle::current();
    let block_stream = supervisor
        .managed_block_stream("peer0", &handle)
        .expect("managed block stream");
    let mut block_rx = block_stream.subscribe();

    let event_stream = supervisor
        .managed_event_stream("peer0", &handle)
        .expect("managed event stream");
    let mut event_rx = event_stream.subscribe();

    let block_event = timeout(Duration::from_secs(1), block_rx.recv())
        .await
        .expect("block event timeout")?
        .clone();
    match block_event {
        BlockStreamEvent::Block {
            summary,
            block,
            raw_len,
        } => {
            assert_eq!(raw_len, block_fixture.len());
            let canonical = block
                .canonical_wire()
                .expect("canonical wire")
                .as_framed()
                .to_vec();
            assert_eq!(canonical, block_fixture);
            assert_eq!(summary.hash_hex, block.hash().to_string());
            assert_eq!(summary.height, block.header().height().get());
            assert_eq!(summary.transaction_count, block.transactions_vec().len());
        }
        other => panic!("unexpected block stream event: {other:?}"),
    }

    let event = timeout(Duration::from_secs(1), event_rx.recv())
        .await
        .expect("event stream timeout")?
        .clone();
    match event {
        EventStreamEvent::Event {
            summary,
            event,
            raw_len,
        } => {
            assert_eq!(raw_len, event_fixture.len());
            assert_eq!(summary.category, EventCategory::Time);
            assert!(matches!(event.as_ref(), EventBox::Time(_)));
        }
        other => panic!("unexpected event stream event: {other:?}"),
    }

    block_stream.abort();
    event_stream.abort();
    mock.shutdown().await?;
    Ok(())
}

fn parse_port(addr: &str) -> Result<u16> {
    addr.parse::<SocketAddr>()
        .map(|addr| addr.port())
        .map_err(|err| eyre!("failed to parse socket address `{addr}`: {err}"))
}

fn peer_trusted_entry(peer: &mochi_core::PeerHandle) -> String {
    format!("{}@{}", peer.peer_id(), peer.p2p_address())
}

fn read_toml_str<'a>(value: &'a toml::Value, table: &str, key: &str) -> Result<&'a str> {
    value
        .get(table)
        .and_then(|table| table.get(key))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| eyre!("missing `{table}.{key}` entry in rendered config"))
}

#[test]
fn supervisor_templates_four_peer_profile() -> Result<()> {
    let temp = TempDir::new()?;
    let port = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping supervisor_templates_four_peer_profile: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let supervisor = build_supervisor(&temp, port, ProfilePreset::FourPeerBft)?;

    assert_eq!(supervisor.profile().topology.peer_count, 4);
    let peers = supervisor.peers();
    assert_eq!(peers.len(), 4);

    let mut torii_ports = HashSet::new();
    let mut p2p_ports = HashSet::new();
    let mut expected_trusted = HashSet::new();
    for peer in peers {
        let torii_port = parse_port(peer.torii_address())?;
        assert!(
            torii_ports.insert(torii_port),
            "duplicate Torii port allocated in BFT profile"
        );

        let p2p_port = parse_port(peer.p2p_address())?;
        assert!(
            p2p_ports.insert(p2p_port),
            "duplicate P2P port allocated in BFT profile"
        );

        expected_trusted.insert(peer_trusted_entry(peer));
    }

    let genesis_path = supervisor.genesis_manifest();
    for peer in peers {
        let config_str = fs::read_to_string(peer.config_path())?;
        let config: toml::Value = toml::from_str(&config_str)?;

        let trusted_peers = config
            .get("trusted_peers")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| eyre!("missing trusted_peers array in rendered config"))?;
        let trusted_set: HashSet<String> = trusted_peers
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| eyre!("trusted_peers entries must be strings"))
                    .map(ToOwned::to_owned)
            })
            .collect::<Result<_>>()?;

        assert_eq!(
            trusted_set,
            expected_trusted,
            "peer {} should trust every generated peer",
            peer.alias()
        );

        let torii_address = read_toml_str(&config, "torii", "address")?;
        assert_eq!(
            parse_port(torii_address)?,
            parse_port(peer.torii_address())?
        );

        let public_address = read_toml_str(&config, "network", "public_address")?;
        assert_eq!(public_address, peer.p2p_address());

        let genesis_file = read_toml_str(&config, "genesis", "file")?;
        assert_eq!(
            Path::new(genesis_file),
            genesis_path,
            "peers should share a single genesis manifest"
        );
    }

    Ok(())
}

#[test]
fn supervisor_allocates_ports_when_wrapping() -> Result<()> {
    let temp = TempDir::new()?;
    // Force p2p allocator to collide with torii assignments and prove it keeps
    // advancing the shared PortAllocator without reusing ports.
    let torii_base = match reserve_port() {
        Ok(port) => port,
        Err(err) if err.kind() == ErrorKind::PermissionDenied => {
            eprintln!("skipping supervisor_allocates_ports_when_wrapping: {err}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let p2p_base = torii_base.checked_add(2).unwrap_or(10_000);
    let supervisor =
        build_supervisor_with_bases(&temp, torii_base, p2p_base, ProfilePreset::FourPeerBft)?;

    let mut torii_ports: Vec<u16> = Vec::new();
    let mut p2p_ports: Vec<u16> = Vec::new();
    let mut all_ports = HashSet::new();

    for peer in supervisor.peers() {
        let torii = parse_port(peer.torii_address())?;
        let p2p = parse_port(peer.p2p_address())?;
        torii_ports.push(torii);
        p2p_ports.push(p2p);
        assert!(
            all_ports.insert(torii) && all_ports.insert(p2p),
            "ports should remain unique across torii/p2p assignments even when wrapping"
        );
    }

    torii_ports.sort_unstable();
    p2p_ports.sort_unstable();

    assert_eq!(
        torii_ports,
        vec![torii_base, torii_base + 1, torii_base + 2, torii_base + 3],
        "torii allocator should hand out sequential ports starting from the base"
    );
    assert_eq!(
        p2p_ports,
        vec![
            torii_base + 4,
            torii_base + 5,
            torii_base + 6,
            torii_base + 7
        ],
        "p2p allocator should skip collisions with torii ports and keep advancing"
    );

    Ok(())
}

#[test]
fn supervisor_genesis_matches_peer_counts() -> Result<()> {
    let presets = [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft];

    for preset in presets {
        let port = match reserve_port() {
            Ok(port) => port,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping supervisor_genesis_matches_peer_counts: {err}");
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let temp = TempDir::new()?;
        let supervisor = build_supervisor(&temp, port, preset)?;
        let bytes = fs::read(supervisor.genesis_manifest())?;
        let value: norito::json::Value = norito::json::from_slice(&bytes)?;

        let chain = value
            .get("chain")
            .and_then(|c| c.as_str())
            .ok_or_else(|| eyre!("missing `chain` field"))?;
        assert_eq!(chain, supervisor.chain_id());

        let topology_len = value
            .get("transactions")
            .and_then(|txs| txs.as_array())
            .and_then(|txs| {
                txs.iter()
                    .filter_map(|tx| {
                        tx.get("topology")
                            .and_then(|topology| topology.as_array())
                            .filter(|entries| !entries.is_empty())
                    })
                    .next()
            })
            .map(|topology| topology.len())
            .unwrap_or_default();

        assert_eq!(
            topology_len,
            supervisor.peers().len(),
            "topology length mismatch for preset {preset:?}"
        );
    }

    Ok(())
}

#[test]
fn supervisor_builder_cleans_preexisting_storage() -> Result<()> {
    let presets = [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft];

    for preset in presets {
        let port = match reserve_port() {
            Ok(port) => port,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!("skipping supervisor_builder_cleans_preexisting_storage: {err}");
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let temp = TempDir::new()?;
        let profile = mochi_core::config::NetworkProfile::from_preset(preset);
        let paths = mochi_core::config::NetworkPaths::from_root(temp.path(), &profile);

        for idx in 0..profile.topology.peer_count {
            let alias = format!("peer{idx}");
            let storage_dir = paths.peer_dir(&alias).join("storage");
            fs::create_dir_all(&storage_dir)?;
            fs::write(storage_dir.join("junk.bin"), b"junk")?;
        }

        let supervisor = build_supervisor(&temp, port, preset)?;

        for idx in 0..supervisor.peers().len() {
            let alias = format!("peer{idx}");
            let storage_dir = supervisor.paths().peer_dir(&alias).join("storage");
            let mut names: Vec<String> = fs::read_dir(&storage_dir)?
                .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect::<std::io::Result<Vec<_>>>()?;
            names.sort();
            assert_eq!(
                names,
                vec!["snapshot".to_string()],
                "storage dir should only contain snapshot for {alias}"
            );

            let mut snapshot_entries = fs::read_dir(storage_dir.join("snapshot"))?;
            assert!(
                snapshot_entries.next().is_none(),
                "snapshot dir should be empty"
            );
        }
    }

    Ok(())
}

#[test]
fn supervisor_wipe_and_regenerate_resets_storage_and_genesis() -> Result<()> {
    let presets = [ProfilePreset::SinglePeer, ProfilePreset::FourPeerBft];

    for preset in presets {
        let port = match reserve_port() {
            Ok(port) => port,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!(
                    "skipping supervisor_wipe_and_regenerate_resets_storage_and_genesis: {err}"
                );
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let temp = TempDir::new()?;
        let mut supervisor = build_supervisor(&temp, port, preset)?;
        let genesis_path = supervisor.genesis_manifest().to_path_buf();
        let aliases: Vec<String> = supervisor
            .peers()
            .iter()
            .map(|peer| peer.alias().to_owned())
            .collect();

        for alias in &aliases {
            let storage_dir = supervisor.paths().peer_dir(alias).join("storage");
            let snapshot_dir = storage_dir.join("snapshot");
            fs::create_dir_all(&snapshot_dir)?;
            fs::write(storage_dir.join("junk.bin"), b"junk")?;
            fs::write(snapshot_dir.join("leftover.bin"), b"stale")?;
        }

        supervisor.wipe_and_regenerate()?;

        let genesis_bytes = fs::read(&genesis_path)?;
        let manifest: Value = norito::json::from_slice(&genesis_bytes)?;

        let chain = manifest
            .get("chain")
            .and_then(Value::as_str)
            .ok_or_else(|| eyre!("missing `chain` field after wipe"))?;
        assert_eq!(
            chain,
            supervisor.chain_id(),
            "genesis chain id should match supervisor for preset {preset:?}"
        );

        let topology_len = manifest
            .get("transactions")
            .and_then(Value::as_array)
            .and_then(|txs| {
                txs.iter()
                    .filter_map(|tx| tx.get("topology").and_then(Value::as_array))
                    .find(|entries| !entries.is_empty())
                    .map(Vec::len)
            })
            .unwrap_or_default();
        assert_eq!(
            topology_len,
            supervisor.peers().len(),
            "topology should match peer count for preset {preset:?}"
        );

        for alias in &aliases {
            let storage_dir = supervisor.paths().peer_dir(alias).join("storage");
            assert!(
                !storage_dir.join("junk.bin").exists(),
                "storage should not retain junk for {alias}"
            );
            let snapshot_dir = storage_dir.join("snapshot");
            assert!(
                snapshot_dir.exists(),
                "snapshot directory should exist for {alias}"
            );
            let mut entries = fs::read_dir(&snapshot_dir)?;
            assert!(
                entries.next().is_none(),
                "snapshot directory should be empty for {alias}"
            );
        }
    }

    Ok(())
}
