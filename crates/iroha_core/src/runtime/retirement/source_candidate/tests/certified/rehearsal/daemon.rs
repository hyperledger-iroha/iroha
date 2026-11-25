//! Actual removed-source daemon startup from privately copied exact oldest checkpoints.
//!
//! This is test orchestration only. Normal Strict startup authenticates the checkpoint,
//! replays its certified suffix and starts native services. Observed process events are
//! engineering checks, not a replacement for the enclosing original all-root evidence.

use super::*;
use iroha_crypto::ExposedPrivateKey;
use iroha_data_model::nexus::{LaneStorageProfile, LaneVisibility};
use iroha_fs::{
    FileSnapshot, PrivateDirectory, ProcessStdio, PublishMode, RetainedBuildInput, RetainedFile,
};
use norito::json::Value;
use std::{
    io::{Read as _, Seek as _},
    net::TcpListener,
    time::{Duration, Instant},
};

const LOG_LIMIT: u64 = 16 * 1024 * 1024;
const STARTUP_LIMIT: Duration = Duration::from_secs(300);
const SHUTDOWN_LIMIT: Duration = Duration::from_secs(30);

// Make the pre-existing fixture governance override explicit before native signing. This
// does not invent a policy from a snapshot or relax any daemon validator.
pub(super) fn parent_configuration() -> TestChainConfig {
    use iroha_data_model::{
        alias_setup::{
            AliasDataSpaceIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasQuoteGuardV1,
            ResolvedDataSpaceV1,
        },
        isi::alias_setup::EnsureAlias,
    };
    // The signed EnsureAlias instruction creates this fixture's lease during genesis.
    let mut config = explicit_governance(parent_config(|_| {}));
    config.genesis_instructions.push(
        EnsureAlias::new(
            AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                dataspace: ResolvedDataSpaceV1::new(
                    ALIAS.parse().unwrap(),
                    crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap(),
                ),
                owner: AccountId::new(config.genesis_key.public_key().clone()),
            }),
            AliasLeaseAcquisitionV1::new(1, None),
            AliasQuoteGuardV1 {
                expected_policy_version: 0,
                expected_payment_asset: defaults::nexus::fees::genesis_policy().fee_asset_id,
                max_amount: iroha_primitives::numeric::Quantity::zero(),
                valid_until_ms: 0,
            },
        )
        .into(),
    );
    config
}
pub(super) fn child_configuration(
    parent: NetworkId,
    home: iroha_model_base::topology::DataSpaceId,
) -> TestChainConfig {
    let mut config = explicit_governance(child_fixture::config_for_root(parent, home));
    config.genesis_parameters.push(checkpoint_policy());
    let nexus = config.nexus.as_mut().unwrap();
    let mut lanes = nexus.lane_catalog.lanes().to_vec();
    for lane in &mut lanes {
        lane.visibility = LaneVisibility::Restricted;
        lane.storage = LaneStorageProfile::FullReplica;
    }
    nexus.lane_catalog =
        iroha_data_model::nexus::LaneCatalog::new(1_u32.try_into().unwrap(), lanes).unwrap();
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    nexus.lane_config = actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.validate_private_root_geometry(home).unwrap();
    config
}
fn explicit_governance(mut config: TestChainConfig) -> TestChainConfig {
    assert!(config.governance.is_none());
    let mut governance = actual::Governance::default();
    governance.citizenship_bond_amount = iroha_primitives::numeric::Quantity::zero();
    config.governance = Some(governance);
    config
}

struct OriginalInput {
    file: RetainedFile,
    snapshot: FileSnapshot,
}
impl OriginalInput {
    fn open(path: &Path) -> Self {
        let file = RetainedFile::open_regular(path).unwrap();
        let snapshot = file.snapshot().unwrap();
        Self { file, snapshot }
    }
    fn recheck(&self) -> std::io::Result<()> {
        if self.file.snapshot()? != self.snapshot {
            return Err(std::io::Error::other("original fixture input changed"));
        }
        Ok(())
    }
}
pub(super) struct DaemonInput {
    directory: Arc<iroha_fs::ReaderDirectory>,
    config: PathBuf,
    config_blake3: String,
    originals: Vec<OriginalInput>,
    floor: u64,
    tip: u64,
    tip_hash: [u8; 32],
    configured_nexus: actual::Nexus,
    execution_bytes: usize,
    reserved_ports: [Option<TcpListener>; 2],
}
impl DaemonInput {
    fn recheck(&self) {
        self.directory.revalidate().unwrap();
        for original in &self.originals {
            original.recheck().unwrap();
        }
    }
}

pub(super) fn export(
    directory: &Path,
    parent: &CertifiedTestChain,
    child: &CertifiedTestChain,
    inputs: &Inputs,
) -> [DaemonInput; 2] {
    [
        export_root(
            directory,
            "parent",
            parent,
            &inputs.parent,
            parent_configuration(),
        ),
        export_root(
            directory,
            "child",
            child,
            &inputs.child,
            child_configuration(
                parent.network_id(),
                crate::sns::dataspace_id_for_sns_alias(ALIAS).unwrap(),
            ),
        ),
    ]
}

fn export_root(
    directory: &Path,
    name: &str,
    chain: &CertifiedTestChain,
    input: &RootInput,
    configuration: TestChainConfig,
) -> DaemonInput {
    assert_eq!(chain.network_id(), input.network);
    assert_eq!(chain.height(), input.tip);
    let root = PrivateDirectory::open_or_create(directory.join(format!("{name}-daemon"))).unwrap();
    let kura = root.create_child("kura").unwrap();
    copy_store_files(&chain.kura().store_root(), kura.path());
    // Strict startup creates its own original lock; no lock is copied from the producer.
    let snapshots = root.create_child("snapshots").unwrap();
    let checkpoints = snapshots
        .create_child(snapshot::checkpoint_store::CHECKPOINTS_DIR_NAME)
        .unwrap();
    let selected: Vec<_> =
        snapshot::checkpoint_store::entries(&directory.join(name).join("snapshots"))
            .unwrap()
            .into_iter()
            .filter(|entry| entry.0 == input.floor)
            .collect();
    assert_eq!(
        selected.len(),
        1,
        "one exact original oldest checkpoint is required"
    );
    let (height, committed_ms, source) = &selected[0];
    assert_eq!(*height, input.floor);
    assert_eq!(*committed_ms, chain.committed(*height).block_time_ms());
    let target = checkpoints
        .create_child(source.file_name().unwrap())
        .unwrap();
    let mut originals = Vec::new();
    // Copy the retained writer's complete bounded directory, without guessing an
    // alternate snapshot layout. The ordinary decoder still owns exact artifact-name,
    // signature, digest and complete-State authentication at process startup.
    let source_directory = iroha_fs::OwnerDirectory::open(source).unwrap();
    let leaves = source_directory.entries(5).unwrap();
    assert_eq!(leaves.len(), 5, "complete original checkpoint inventory");
    for leaf in &leaves {
        let (original, copied) = copy_original(
            &source.join(leaf),
            &target.path().join(leaf),
            u64::try_from(defaults::snapshot::MAX_READ_BUFFER_BYTES.get()).unwrap(),
        )
        .unwrap();
        originals.extend([original, copied]);
    }
    assert_eq!(source_directory.entries(5).unwrap(), leaves);
    source_directory.revalidate().unwrap();
    target.sync().unwrap();
    checkpoints.sync().unwrap();
    snapshots.sync().unwrap();
    // There is deliberately no CURRENT, rolling generation, newer checkpoint or genesis
    // State fallback in this input directory. Normal strict discovery must select this H.
    assert_eq!(
        snapshot::checkpoint_store::entries(snapshots.path())
            .unwrap()
            .len(),
        1
    );
    root.write_atomic(
        "genesis.nrt",
        &chain.genesis().encode_wire().unwrap(),
        PublishMode::CreateNew,
    )
    .unwrap();
    originals.push(OriginalInput::open(&root.path().join("genesis.nrt")));
    // Reserve candidate ports while authoring. A collision after release is a failed run,
    // never permission to substitute another listener/configuration.
    let p2p = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let torii = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let text = configuration_text(
        root.path(),
        chain,
        &configuration,
        iroha_primitives::addr::SocketAddr::from(p2p.local_addr().unwrap()),
        iroha_primitives::addr::SocketAddr::from(torii.local_addr().unwrap()),
    );
    let config = root.path().join("node.toml");
    root.write_atomic("node.toml", text.as_bytes(), PublishMode::CreateNew)
        .unwrap();
    validate_configuration(&config, chain, &configuration);
    originals.push(OriginalInput::open(&config));
    DaemonInput {
        directory: Arc::new(iroha_fs::ReaderDirectory::open(root.path()).unwrap()),
        config,
        config_blake3: blake3::hash(text.as_bytes()).to_hex().to_string(),
        originals,
        floor: input.floor,
        tip: input.tip,
        tip_hash: input.tip_hash,
        configured_nexus: configuration.nexus.unwrap_or_default(),
        execution_bytes: configuration.pipeline.ivm_execution_max_bytes,
        reserved_ports: [Some(p2p), Some(torii)],
    }
}

fn copy_exact(
    source: &mut impl std::io::Read,
    output: &mut impl std::io::Write,
    length: u64,
) -> std::io::Result<()> {
    if std::io::copy(&mut (&mut *source).take(length), output)? != length {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "truncated original checkpoint",
        ));
    }
    let mut trailing = [0_u8; 1];
    if source.read(&mut trailing)? != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "grown original checkpoint",
        ));
    }
    Ok(())
}

fn copy_original(
    source: &Path,
    target: &Path,
    maximum: u64,
) -> std::io::Result<(OriginalInput, OriginalInput)> {
    let mut file = RetainedFile::open_regular(source)?;
    let snapshot = file.snapshot()?;
    let length = file.file().metadata()?.len();
    if length > maximum {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "fixture artifact exceeds the original snapshot reader byte bound",
        ));
    }
    let mut output = RetainedFile::create_new_private(target)?;
    let result = copy_exact(file.file_mut(), output.file_mut(), length);
    // Original namespace, identity, extent and bytes remain admitted through the copy,
    // including every ordinary read error. No partially copied path is used on failure.
    let original = OriginalInput { file, snapshot };
    original.recheck()?;
    result?;
    let file = output.seal()?;
    let snapshot = file.snapshot()?;
    Ok((original, OriginalInput { file, snapshot }))
}

fn validate_configuration(
    path: &Path,
    chain: &CertifiedTestChain,
    configuration: &TestChainConfig,
) {
    let parsed = actual::Root::from_toml_source(
        iroha_config::base::toml::TomlSource::from_file(path).unwrap(),
    )
    .unwrap();
    assert_eq!(parsed.common.chain, configuration.chain_id);
    assert_eq!(parsed.genesis.expected_hash, chain.genesis().hash());
    assert_eq!(
        parsed.genesis.public_key,
        *configuration.genesis_key.public_key()
    );
    assert_eq!(
        parsed.common.key_pair.public_key(),
        fixture_keys()[0].public_key()
    );
    assert_eq!(
        parsed.gov.citizenship_bond_amount,
        configuration
            .governance
            .as_ref()
            .unwrap()
            .citizenship_bond_amount
    );
    let expected_nexus = configuration.nexus.clone().unwrap_or_default();
    assert_eq!(
        parsed.nexus.configured_lane_catalog,
        expected_nexus.configured_lane_catalog
    );
    assert_eq!(
        parsed.nexus.configured_dataspace_catalog,
        expected_nexus.configured_dataspace_catalog
    );
    assert_eq!(parsed.nexus.routing_policy, expected_nexus.routing_policy);
    assert_eq!(
        parsed.pipeline.ivm_execution_max_bytes,
        configuration.pipeline.ivm_execution_max_bytes
    );
    assert!(matches!(parsed.kura.init_mode, InitMode::Strict));
    assert!(parsed.lifecycle.exit_on_stdin_close);
}

fn quoted(value: impl std::fmt::Display) -> String {
    norito::json::to_json(&value.to_string()).unwrap()
}
fn configuration_text(
    root: &Path,
    chain: &CertifiedTestChain,
    config: &TestChainConfig,
    p2p: iroha_primitives::addr::SocketAddr,
    torii: iroha_primitives::addr::SocketAddr,
) -> String {
    let validator = fixture_keys().remove(0);
    let transport = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
    let streaming = KeyPair::from_seed(vec![0xA6; 32], Algorithm::Ed25519);
    let mut text = format!(
        r#"chain = {chain}
public_key = {validator}
private_key = {validator_secret}
soranet_transport_public_key = {transport}
soranet_transport_private_key = {transport_secret}
trusted_peers_pop = [{{ public_key = {validator}, pop_hex = {validator_pop} }}]
[network]
address = {p2p}
public_address = {p2p}
[torii]
address = {torii}
[genesis]
file = {genesis}
public_key = {genesis_key}
expected_hash = {genesis_hash}
[kura]
store_dir = {kura}
init_mode = "strict"
[snapshot]
store_dir = {snapshots}
mode = "readonly"
verification_public_key = {snapshot_key}
merkle_chunk_size_bytes = 4096
[gov]
citizenship_bond_amount = "0"
[lifecycle]
exit_on_stdin_close = true
[logger]
level = "DEBUG"
format = "json"
terminal_colors = false
[sumeragi]
records_dir = {records}
[streaming]
identity_public_key = {streaming}
identity_private_key = {streaming_secret}
session_store_dir = {streaming_store}
[nexus.storage]
local_budget_bytes = 1073741824
"#,
        chain = quoted(&config.chain_id),
        validator = quoted(validator.public_key()),
        validator_secret = quoted(ExposedPrivateKey(validator.private_key().clone())),
        validator_pop = quoted(hex::encode(
            iroha_crypto::bls_normal_pop_prove(validator.private_key()).unwrap()
        )),
        transport = quoted(transport.public_key()),
        transport_secret = quoted(ExposedPrivateKey(transport.private_key().clone())),
        p2p = quoted(p2p),
        torii = quoted(torii),
        genesis = quoted(root.join("genesis.nrt").display()),
        genesis_key = quoted(config.genesis_key.public_key()),
        genesis_hash = quoted(chain.genesis().hash()),
        kura = quoted(root.join("kura").display()),
        snapshots = quoted(root.join("snapshots").display()),
        snapshot_key = quoted(snapshot_key().public_key()),
        records = quoted(root.join("sumeragi-records").display()),
        streaming = quoted(streaming.public_key()),
        streaming_secret = quoted(ExposedPrivateKey(streaming.private_key().clone())),
        streaming_store = quoted(root.join("streaming").display())
    );
    if let Some(nexus) = &config.nexus {
        let manifest = crate::sns::selector_for_dataspace_alias(ALIAS)
            .unwrap()
            .name_hash();
        let [lane] = nexus.configured_lane_catalog.lanes() else {
            panic!("exact one-lane child fixture");
        };
        let [space] = nexus.configured_dataspace_catalog.entries() else {
            panic!("exact one-data-space child fixture");
        };
        assert_eq!(
            iroha_model_base::topology::DataSpaceId::from_hash(&manifest),
            space.id
        );
        use std::fmt::Write as _;
        write!(
            text,
            r#"
[nexus]
lane_count = 1
[[nexus.lane_catalog]]
index = 0
alias = {lane}
dataspace = {space}
visibility = "restricted"
storage = "full_replica"
[[nexus.dataspace_catalog]]
alias = {space}
manifest_hash = {manifest}
fault_tolerance = 1
[nexus.routing_policy]
default_lane = 0
default_dataspace = {space}
"#,
            lane = quoted(&lane.alias),
            space = quoted(&space.alias),
            manifest = quoted(hex::encode(manifest))
        )
        .unwrap();
    }
    text
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Progress {
    restored: bool,
    replayed: bool,
    started: bool,
    listening: bool,
    stopped: bool,
}
impl Progress {
    fn observe(&mut self, line: &str, floor: u64, tip: u64) -> Result<(), &'static str> {
        let Ok(value) = norito::json::from_str::<Value>(line) else {
            return Ok(());
        };
        let Some(fields) = value.get("fields") else {
            return Ok(());
        };
        let message = fields.get("message").and_then(Value::as_str);
        let expected_target = match message {
            Some(
                "Successfully loaded the state from a snapshot"
                | "Sumeragi rebuilt the state from genesis and Kura"
                | "Didn't find a state snapshot; creating an empty state",
            ) => "irohad::startup_preparation",
            Some("startup trace" | "Iroha shutdown normally") => "irohad",
            Some("Torii bound and listening") => "iroha_torii",
            _ => return Ok(()),
        };
        if value.get("target").and_then(Value::as_str) != Some(expected_target) {
            return Ok(());
        }
        match message {
            Some("Successfully loaded the state from a snapshot") => {
                if self.restored || fields.get("at_height").and_then(Value::as_u64) != Some(floor) {
                    return Err("daemon restored a different or repeated checkpoint");
                }
                self.restored = true;
            }
            Some("Sumeragi rebuilt the state from genesis and Kura") => {
                if !self.restored
                    || self.replayed
                    || fields.get("height").and_then(Value::as_u64) != Some(tip)
                {
                    return Err(
                        "daemon replay did not reach the original tip from the oldest floor",
                    );
                }
                self.replayed = true;
            }
            Some("startup trace")
                if fields.get("stage").and_then(Value::as_str)
                    == Some("irohad.sumeragi.started") =>
            {
                if !self.replayed {
                    return Err("native startup preceded certified suffix replay");
                }
                self.started = true;
            }
            Some("Torii bound and listening") => {
                if !self.started || self.listening {
                    return Err("Torii did not start after native consensus");
                }
                self.listening = true;
            }
            Some("Iroha shutdown normally") => {
                if !self.listening || self.stopped {
                    return Err("shutdown preceded complete ordinary startup");
                }
                self.stopped = true;
            }
            Some("Didn't find a state snapshot; creating an empty state") => {
                return Err("genesis fallback is forbidden");
            }
            _ => {}
        }
        Ok(())
    }
}

fn check_terminal_store(input: &DaemonInput) {
    input.recheck();
    // This is an additional stopped-store invariant after the actual Strict process has
    // completed. It cannot stand in for startup or the original all-root replay evidence.
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&input.directory.path().join("kura")),
        &input.configured_nexus.lane_config,
        &input.configured_nexus.configured_lane_catalog,
    )
    .unwrap();
    assert!(kura.is_locked_read_only_source());
    assert_eq!(kura.exact_durable_blocks_count().unwrap() as u64, input.tip);
    let budget = AllocationBudget::new(input.execution_bytes);
    let block = kura
        .get_block(
            NonZeroUsize::new(input.tip.try_into().unwrap()).unwrap(),
            &budget,
        )
        .unwrap()
        .unwrap();
    assert_eq!(block.hash().as_ref(), &input.tip_hash);
    input.recheck();
}

fn read_progress(reader: &mut RetainedFile, floor: u64, tip: u64) -> Result<Progress, String> {
    reader.revalidate().map_err(|error| error.to_string())?;
    if reader
        .file()
        .metadata()
        .map_err(|error| error.to_string())?
        .len()
        > LOG_LIMIT
    {
        return Err("bounded daemon log exceeded".into());
    }
    reader
        .file_mut()
        .rewind()
        .map_err(|error| error.to_string())?;
    let mut log = String::new();
    reader
        .file_mut()
        .take(LOG_LIMIT + 1)
        .read_to_string(&mut log)
        .map_err(|error| error.to_string())?;
    reader.revalidate().map_err(|error| error.to_string())?;
    if log.len() as u64 > LOG_LIMIT {
        return Err("bounded daemon log exceeded".into());
    }
    let mut progress = Progress::default();
    // The final partial record is not an observed event until its newline arrives.
    for line in log
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
    {
        progress.observe(line, floor, tip)?;
    }
    Ok(progress)
}

pub(super) fn start_originals(binary: &Arc<RetainedBuildInput>, inputs: [DaemonInput; 2]) {
    let binary_snapshot = binary.snapshot().unwrap();
    for mut input in inputs {
        input.recheck();
        binary.revalidate().unwrap();
        let path = input.directory.path().join("daemon.jsonl");
        let output_owner = RetainedFile::create_new_private(&path).unwrap();
        let output = output_owner.file().try_clone().unwrap();
        let mut reader = RetainedFile::open_private(&path).unwrap();
        let mut command = binary.command().unwrap();
        command.current_dir_retained(&input.directory).unwrap();
        command.env_clear();
        command
            .args(["--config"])
            .arg(&input.config)
            .arg("--config-blake3")
            .arg(&input.config_blake3)
            .stdin(ProcessStdio::Piped)
            .stdout(output.try_clone().unwrap())
            .stderr(output)
            .kill_on_drop(true);
        for port in &mut input.reserved_ports {
            drop(port.take());
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + STARTUP_LIMIT;
        let mut complete = false;
        while Instant::now() < deadline {
            assert!(
                child.try_wait().unwrap().is_none(),
                "daemon exited before native startup"
            );
            let progress = read_progress(&mut reader, input.floor, input.tip).unwrap();
            if progress.listening {
                complete = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // Close only this owned input pipe; the existing node lifecycle requests graceful
        // shutdown. kill_on_drop is solely a failure cleanup for this launched daemon.
        drop(child.stdin.take());
        let deadline = Instant::now() + SHUTDOWN_LIMIT;
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "owned daemon did not join graceful shutdown"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(
            complete,
            "normal Strict native startup was not observed within its deadline"
        );
        assert!(status.success(), "daemon's graceful shutdown failed");
        assert!(
            read_progress(&mut reader, input.floor, input.tip)
                .unwrap()
                .stopped
        );
        output_owner.revalidate().unwrap();
        check_terminal_store(&input);
        input.recheck();
        binary.revalidate().unwrap();
        assert_eq!(binary.snapshot().unwrap(), binary_snapshot);
    }
}

#[cfg(test)]
mod tests;
