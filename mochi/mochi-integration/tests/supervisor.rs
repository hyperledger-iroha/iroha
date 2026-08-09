use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    net::{SocketAddr, TcpListener},
    path::{Path, PathBuf},
    time::Duration,
};

use color_eyre::{Result, eyre::eyre};
use iroha_data_model::{block::stream::BlockMessage, events::EventBox};
use mochi_core::{
    ProfilePreset, Supervisor, SupervisorBuilder, resolve_selected_peer_storage_paths,
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
    parse_socket_addr(supervisor.peers()[0].torii_address()).expect("parse torii address")
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
    let supervisor = build_supervisor(&temp, port, ProfilePreset::FourPeerBft)?;
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
    assert_eq!(sumeragi.leader, data.sumeragi.leader);

    let diagnostics = client.fetch_sumeragi_diagnostics().await?;
    assert_eq!(diagnostics, data.sumeragi_diagnostics);

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
    let supervisor = build_supervisor(&temp, port, ProfilePreset::FourPeerBft)?;
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
    let supervisor = build_supervisor(&temp, port, ProfilePreset::FourPeerBft)?;
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
    assert_eq!(sumeragi.height, 10);
    assert_eq!(sumeragi.view, 4);
    assert_eq!(sumeragi.last_committed_height, 9);
    let diagnostics = client.fetch_sumeragi_diagnostics().await?;
    assert_eq!(diagnostics.tx_queue_depth, 4);
    assert_eq!(diagnostics.tx_queue_capacity, 1024);

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

    let stream_data = MockToriiData::from_fixture_dir(&fixture_dir)?;
    let expected_block: BlockMessage = norito::decode_from_bytes(&stream_data.block_frame)?;
    let expected_event: EventBox = norito::decode_from_bytes::<
        iroha_data_model::events::stream::EventMessage,
    >(&stream_data.event_frame)?
    .into();

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
            assert_eq!(raw_len, stream_data.block_frame.len());
            assert_eq!(block.as_ref(), &expected_block.0);
            assert_eq!(summary.hash_hex, block.hash().to_string());
            assert_eq!(summary.height, block.header().height().get());
            assert_eq!(summary.transaction_count, block.external_entrypoint_count());
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
            assert_eq!(raw_len, stream_data.event_frame.len());
            assert_eq!(summary.category, EventCategory::Pipeline);
            assert_eq!(event.as_ref(), &expected_event);
        }
        other => panic!("unexpected event stream event: {other:?}"),
    }

    block_stream.abort();
    event_stream.abort();
    mock.shutdown().await?;
    Ok(())
}

fn parse_socket_addr(addr: &str) -> Result<SocketAddr> {
    if let Ok(addr) = addr.parse::<SocketAddr>() {
        return Ok(addr);
    }

    let literal = norito::literal::parse("addr", addr)
        .map_err(|err| eyre!("failed to parse socket literal `{addr}`: {err}"))?;
    literal
        .parse::<SocketAddr>()
        .map_err(|err| eyre!("failed to parse socket address `{addr}`: {err}"))
}

fn parse_port(addr: &str) -> Result<u16> {
    parse_socket_addr(addr).map(|addr| addr.port())
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
    let genesis_block_path = supervisor.genesis_block_file();
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
        assert_eq!(
            parse_socket_addr(public_address)?,
            parse_socket_addr(peer.p2p_address())?
        );

        let genesis_file = read_toml_str(&config, "genesis", "file")?;
        assert_eq!(
            Path::new(genesis_file),
            genesis_block_path,
            "peers should share a single signed genesis file"
        );
        let manifest_json = read_toml_str(&config, "genesis", "manifest_json")?;
        assert_eq!(Path::new(manifest_json), genesis_path);
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
    let mut all_ports_sorted: Vec<u16> = all_ports.into_iter().collect();
    all_ports_sorted.sort_unstable();

    assert_eq!(
        all_ports_sorted,
        (0..8).map(|offset| torii_base + offset).collect::<Vec<_>>(),
        "allocators should cover the contiguous free range without reusing or skipping ports"
    );
    assert_eq!(
        torii_ports.first().copied(),
        Some(torii_base),
        "torii allocator should start from the requested base"
    );
    assert_eq!(
        p2p_ports.first().copied(),
        Some(p2p_base),
        "p2p allocator should start from its requested base before collision avoidance"
    );

    Ok(())
}

#[test]
fn supervisor_genesis_matches_peer_counts() -> Result<()> {
    let presets = [ProfilePreset::FourPeerBft];

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
fn supervisor_builder_preserves_unmanaged_storage_and_selects_fresh_generation() -> Result<()> {
    let presets = [ProfilePreset::FourPeerBft];

    for preset in presets {
        let port = match reserve_port() {
            Ok(port) => port,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                eprintln!(
                    "skipping supervisor_builder_preserves_unmanaged_storage_and_selects_fresh_generation: {err}"
                );
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

        for peer in supervisor.peers() {
            let alias = peer.alias();
            let unmanaged_storage = paths.peer_dir(alias).join("storage");
            assert_eq!(
                fs::read(unmanaged_storage.join("junk.bin"))?,
                b"junk",
                "building a generation must preserve unmanaged storage for {alias}"
            );

            let selected = resolve_selected_peer_storage_paths(supervisor.paths().root(), alias)?
                .ok_or_else(|| eyre!("missing selected storage for {alias}"))?;
            assert_eq!(selected.config_generation_id(), supervisor.generation_id());
            assert_eq!(selected.storage_generation_id(), supervisor.generation_id());
            assert_eq!(selected.storage_dir(), peer.storage_dir());
            assert_eq!(selected.snapshot_dir(), peer.snapshot_dir());
            assert_ne!(selected.storage_dir(), unmanaged_storage);

            let storage_dir = selected.storage_dir();
            let mut names: Vec<String> = fs::read_dir(&storage_dir)?
                .map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect::<std::io::Result<Vec<_>>>()?;
            names.sort();
            assert_eq!(
                names,
                vec!["snapshot".to_string()],
                "storage dir should only contain snapshot for {alias}"
            );

            let snapshot_dir = selected.snapshot_dir();
            let snapshot_entries = fs::read_dir(&snapshot_dir)?
                .map(|entry| entry.map(|value| value.file_name()))
                .collect::<std::io::Result<Vec<_>>>()?;
            assert_eq!(snapshot_entries, vec!["generations"]);
            assert!(
                fs::read_dir(snapshot_dir.join("generations"))?
                    .next()
                    .is_none(),
                "snapshot generations should be empty"
            );
        }
    }

    Ok(())
}

#[test]
fn supervisor_wipe_and_regenerate_resets_storage_and_genesis() -> Result<()> {
    let presets = [ProfilePreset::FourPeerBft];

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
        let old_generation_id = supervisor.generation_id().to_owned();
        let old_genesis_path = supervisor.genesis_manifest().to_path_buf();
        let old_generation_root = old_genesis_path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| eyre!("genesis path has no immutable generation root"))?
            .to_path_buf();
        let old_genesis_bytes = fs::read(&old_genesis_path)?;
        let mut old_peer_paths = Vec::with_capacity(supervisor.peers().len());

        for peer in supervisor.peers() {
            let alias = peer.alias().to_owned();
            let storage_dir = peer.storage_dir().to_path_buf();
            let snapshot_dir = peer.snapshot_dir().to_path_buf();
            let config_path = peer.config_path().to_path_buf();
            let config_bytes = fs::read(&config_path)?;
            fs::write(storage_dir.join("junk.bin"), b"old-storage-state")?;
            fs::write(snapshot_dir.join("leftover.bin"), b"old-snapshot-state")?;
            old_peer_paths.push((alias, storage_dir, snapshot_dir, config_path, config_bytes));
        }

        supervisor.wipe_and_regenerate()?;

        let new_generation_id = supervisor.generation_id().to_owned();
        let new_genesis_path = supervisor.genesis_manifest().to_path_buf();
        let new_generation_root = new_genesis_path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| eyre!("regenerated genesis path has no immutable generation root"))?
            .to_path_buf();
        assert_ne!(new_generation_id, old_generation_id);
        assert_ne!(new_generation_root, old_generation_root);
        assert_eq!(
            fs::read(&old_genesis_path)?,
            old_genesis_bytes,
            "regeneration must retain the prior immutable genesis"
        );
        assert!(old_generation_root.is_dir());

        let genesis_bytes = fs::read(&new_genesis_path)?;
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

        for (peer, (alias, old_storage, old_snapshot, old_config, old_config_bytes)) in
            supervisor.peers().iter().zip(&old_peer_paths)
        {
            assert_eq!(peer.alias(), alias);
            assert_eq!(
                fs::read(old_config)?.as_slice(),
                old_config_bytes.as_slice()
            );
            assert!(old_config.starts_with(&old_generation_root));
            assert!(peer.config_path().starts_with(&new_generation_root));

            let selected = resolve_selected_peer_storage_paths(supervisor.paths().root(), alias)?
                .ok_or_else(|| eyre!("missing regenerated storage for {alias}"))?;
            assert_eq!(selected.config_generation_id(), new_generation_id);
            assert_eq!(selected.storage_generation_id(), new_generation_id);
            assert_eq!(selected.storage_dir(), peer.storage_dir());
            assert_eq!(selected.snapshot_dir(), peer.snapshot_dir());
            assert_ne!(selected.storage_dir(), old_storage);
            assert_ne!(selected.snapshot_dir(), old_snapshot);
            assert_eq!(
                fs::read(old_storage.join("junk.bin"))?,
                b"old-storage-state",
                "retired storage state must remain available for {alias}"
            );
            assert_eq!(
                fs::read(old_snapshot.join("leftover.bin"))?,
                b"old-snapshot-state",
                "retired snapshot state must remain available for {alias}"
            );

            let storage_dir = selected.storage_dir();
            assert!(
                !storage_dir.join("junk.bin").exists(),
                "fresh selected storage should not inherit junk for {alias}"
            );
            let snapshot_dir = selected.snapshot_dir();
            assert!(
                snapshot_dir.exists(),
                "snapshot directory should exist for {alias}"
            );
            assert!(
                !snapshot_dir.join("leftover.bin").exists(),
                "fresh selected snapshot should not inherit retired state for {alias}"
            );
            let entries = fs::read_dir(&snapshot_dir)?
                .map(|entry| entry.map(|value| value.file_name()))
                .collect::<std::io::Result<Vec<_>>>()?;
            assert_eq!(entries, vec!["generations"]);
            assert!(
                fs::read_dir(snapshot_dir.join("generations"))?
                    .next()
                    .is_none(),
                "snapshot generations should be empty for {alias}"
            );
        }
    }

    Ok(())
}
