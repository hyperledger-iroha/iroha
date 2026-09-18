//! Signed fresh-beacon bootstrap inside the native public-reset controller.
//!
//! The original release/configuration closure remains immutable. Only the exact
//! public transcript authorized here may derive `config/beacon.toml` and select
//! the separately signed FD200 unit. The live DKG child never owns the host action
//! lock. Losing it before its final bundle is a retained, non-resumable failure.

use super::*;
use crate::taira_dataspace_deploy::{
    AuthenticatedHeightObserverV1, DeploymentPeerV1, HeightObservationV1, VerifiedCommittedHeightV1,
};
use crate::taira_public_reset as reset;
use iroha::client::Client;
use iroha_core::beacon::{
    AdaptiveGlobalThresholdBeaconDkgCryptoV1, FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    GlobalThresholdBeaconDkgPhaseV1, GlobalThresholdBeaconDkgStateV1,
    global_threshold_beacon_roster_hash_v1,
};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    consensus::GlobalThresholdBeaconDkgSessionV1,
    isi::consensus_keys::ThresholdKeyLifecycleCertificateV1,
};
use std::process::{ChildStderr, ChildStdout};
use zeroize::Zeroizing;

const PLAN_SCHEMA: &str = "iroha.taira.public-reset.beacon-bootstrap-plan.v1";
const REQUEST_SCHEMA: &str = "iroha.global-beacon.bootstrap.request.v1";
const BUNDLE_SCHEMA: &str = "iroha.global-beacon.bootstrap.bundle.v1";
const MARKER_SCHEMA: &str = "iroha.taira.public-reset.beacon-ceremony-started.v1";
const CREDENTIAL_FILE: &str = "iroha-global-beacon-partial-signer-v1.norito";
const PUBLIC_LIMIT: u64 = 32 * 1024 * 1024;
const CONFIG_LIMIT: u64 = 1024 * 1024;
const PROVIDER_FIELDS: [&str; 3] = [
    "global_beacon_partial_signer_provider_handle",
    "global_beacon_partial_signer_provider_revision",
    "global_beacon_partial_signer_provider_policy_digest_hex",
];

/// A required part of the signed inventory, never populated after authorization.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(in super::super) struct BeaconBootstrapPlanV1 {
    pub(in super::super) schema: String,
    pub(in super::super) request: NativeRequestV1,
    /// Canonical public manifest bytes; native genesis validation binds them to the signed wire.
    pub(in super::super) genesis_manifest: Vec<u8>,
    pub(in super::super) genesis_public_key: PublicKey,
    /// Exact unit bytes rendered before DKG with the fixed output path and beacon.toml.
    pub(in super::super) final_units: Vec<FinalUnitV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(in super::super) struct NativeRequestV1 {
    pub(in super::super) schema: String,
    pub(in super::super) dkg_session: GlobalThresholdBeaconDkgSessionV1,
    pub(in super::super) target_roster: Vec<PeerId>,
    pub(in super::super) authorization_roster: Vec<PeerId>,
    pub(in super::super) provider_handles: Vec<String>,
    pub(in super::super) provider_revision: u64,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(in super::super) struct FinalUnitV1 {
    pub(in super::super) validator: String,
    pub(in super::super) bytes: Vec<u8>,
    pub(in super::super) sha256: String,
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ProviderV1 {
    signer_index: u16,
    validator: PeerId,
    handle: String,
    revision: u64,
    policy_digest: [u8; 32],
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GenesisProofV1 {
    manifest: iroha_genesis::RawGenesisTransaction,
    signed_wire: Vec<u8>,
    public_key: PublicKey,
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PublicBundleV1 {
    schema: String,
    genesis: GenesisProofV1,
    request: NativeRequestV1,
    finalized_observed_height: u64,
    record: FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    certificate: ThresholdKeyLifecycleCertificateV1,
    providers: Vec<ProviderV1>,
}

/// Public native preparation result; contains no signing or dealer material.
#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub(in super::super) struct PreparedBeaconInputsV1 {
    schema: String,
    authorization_nonce: String,
    request: NativeRequestV1,
    final_units: Vec<PreparedBeaconUnitV1>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedBeaconUnitV1 {
    validator: String,
    signer_index: u16,
    credential_path: String,
    config_file: String,
}

pub(in super::super) fn prepare_public_beacon_inputs(
    wire: &[u8],
    manifest_bytes: &[u8],
    public_key: &PublicKey,
    expected_hash: HashOf<BlockHeader>,
    nonce: &str,
    validators: &[ValidatorV1],
    clients: &[reset::ValidatorClientV1],
) -> Result<PreparedBeaconInputsV1> {
    let manifest = json::from_slice(manifest_bytes)?;
    let genesis = iroha_genesis::validate_prepared_genesis_bundle(
        wire,
        &manifest,
        public_key,
        expected_hash,
    )?;
    derive_public_beacon_inputs(&genesis, &manifest, nonce, validators, clients)
}

fn derive_public_beacon_inputs(
    genesis: &iroha_genesis::ValidatedGenesisBundle,
    manifest: &iroha_genesis::RawGenesisTransaction,
    nonce: &str,
    validators: &[ValidatorV1],
    clients: &[reset::ValidatorClientV1],
) -> Result<PreparedBeaconInputsV1> {
    use iroha_data_model::parameter::system::{SumeragiConsensusMode, SumeragiNposParameters};
    reset::validate_nonce(nonce)?;
    if genesis.block().header().height().get() != 1
        || genesis.consensus_metadata().mode != SumeragiConsensusMode::Npos
        || validators.len() != 4
        || clients.len() != 4
    {
        return Err(eyre!(
            "beacon preparation requires a fresh four-validator NPoS genesis"
        ));
    }
    let parameters = manifest.effective_parameters()?;
    let first_pulse = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .and_then(|npos| npos.epoch_length_blocks().get().checked_sub(1))
        .ok_or_else(|| {
            eyre!("beacon preparation requires explicit signed NPoS epoch parameters")
        })?;
    // Onboarding/faucet apply at 2/3; the QueuePlanSynced final canary uses
    // admission/proposal/merge carriers 4/5/6. Ordinary installation can then
    // execute at 7 and activate at 8. Actual observations remain authoritative.
    if first_pulse <= 7 {
        return Err(eyre!(
            "required operations and installation must precede the first mandatory beacon pulse after height 7"
        ));
    }
    let roster = iroha_core::sumeragi::signed_genesis_voting_peers(&iroha_genesis::GenesisBlock(
        genesis.block().clone(),
    ))?;
    if roster.len() != 4 || roster.iter().collect::<BTreeSet<_>>().len() != 4 {
        return Err(eyre!(
            "beacon preparation requires four exact native voting seats"
        ));
    }
    let selected = clients
        .iter()
        .map(|client| client.peer_id.parse::<PeerId>())
        .collect::<Result<Vec<_>, _>>()?;
    if selected.iter().collect::<BTreeSet<_>>() != roster.iter().collect()
        || validators
            .iter()
            .map(|validator| &validator.slug)
            .collect::<BTreeSet<_>>()
            .len()
            != 4
    {
        return Err(eyre!(
            "unsigned validator roles differ from the exact native genesis roster"
        ));
    }
    let network = NetworkId::from_genesis_hash(genesis.expected_hash());
    let mut digest = Sha256::new();
    digest.update(b"iroha:taira:public-reset:beacon-session:v1\0");
    digest.update(network.as_bytes());
    digest.update(nonce.as_bytes());
    let request = NativeRequestV1 {
        schema: REQUEST_SCHEMA.into(),
        dkg_session: GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id: network,
            session_id: digest.finalize().into(),
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            sharing_end_height: 2,
            complaints_end_height: 3,
            responses_end_height: 4,
        },
        target_roster: roster.clone(),
        authorization_roster: roster.clone(),
        provider_handles: (1..=4)
            .map(|seat| format!("taira-beacon-seat-{seat}"))
            .collect(),
        provider_revision: 1,
    };
    GlobalThresholdBeaconDkgStateV1::new(
        request.dkg_session,
        &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
    )
    .map_err(|error| eyre!("native fresh beacon request is invalid: {error:?}"))?;
    let mut final_units = Vec::new();
    for (validator, peer) in validators.iter().zip(selected) {
        reset::validate_slug("beacon validator role", &validator.slug)?;
        let seat = roster
            .iter()
            .position(|candidate| candidate == &peer)
            .ok_or_else(|| eyre!("validator has no native voting seat"))?
            + 1;
        final_units.push(PreparedBeaconUnitV1 {
            validator: validator.slug.clone(), signer_index: u16::try_from(seat)?,
            credential_path: format!("/var/lib/taira/.public-reset-control-v1/beacon/{nonce}/ceremony/seat-{seat}/{CREDENTIAL_FILE}"),
            config_file: "beacon.toml".into(),
        });
    }
    Ok(PreparedBeaconInputsV1 {
        schema: "iroha.taira.public-reset.beacon-inputs.v1".into(),
        authorization_nonce: nonce.into(),
        request,
        final_units,
    })
}

pub(in super::super) fn ceremony_root(inventory: &InventoryV1) -> PathBuf {
    Path::new("/var/lib/taira/.public-reset-control-v1/beacon").join(&inventory.authorization_nonce)
}

fn credential_path(inventory: &InventoryV1, index: usize) -> PathBuf {
    ceremony_root(inventory)
        .join("ceremony")
        .join(format!("seat-{}", index + 1))
        .join(CREDENTIAL_FILE)
}

pub(in super::super) fn validate_plan(inventory: &InventoryV1) -> Result<()> {
    let plan = &inventory.beacon_bootstrap;
    let request = &plan.request;
    let expected_roster = inventory
        .validator_clients
        .iter()
        .map(|client| client.peer_id.parse::<PeerId>())
        .collect::<Result<Vec<_>, _>>()?;
    if plan.schema != PLAN_SCHEMA
        || request.schema != REQUEST_SCHEMA
        || expected_roster.len() != 4
        || request.target_roster.len() != 4
        || request.target_roster.iter().collect::<BTreeSet<_>>() != expected_roster.iter().collect()
        || request.authorization_roster != request.target_roster
        || hex::encode(request.dkg_session.network_id.as_bytes()) != inventory.next_genesis_hash
        || request.dkg_session.committee_size != 4
        || request.dkg_session.threshold != 2
        || request.dkg_session.roster_hash
            != global_threshold_beacon_roster_hash_v1(&request.target_roster)
        || request.provider_handles.len() != 4
        || request.provider_revision == 0
        || request
            .provider_handles
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 4
        || request.provider_handles.iter().any(|handle| {
            iroha_config::parameters::validate_production_runtime_handle(handle).is_err()
        })
        || plan.genesis_manifest.is_empty()
        || plan.genesis_manifest.len() as u64 > PUBLIC_LIMIT
        || plan.final_units.len() != 4
    {
        return Err(eyre!(
            "beacon bootstrap plan differs from the exact signed four-validator deployment"
        ));
    }
    let state = GlobalThresholdBeaconDkgStateV1::new(
        request.dkg_session,
        &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
    )
    .map_err(|_| eyre!("beacon bootstrap has invalid native DKG windows"))?;
    if state.phase_at(request.dkg_session.start_height) != GlobalThresholdBeaconDkgPhaseV1::Sharing
    {
        return Err(eyre!("beacon bootstrap must begin in native Sharing"));
    }
    // Unit interpretation remains the native renderer/loaded-systemd contract. This
    // inventory binds the exact public bytes, including the fixed credential/config paths.
    for (index, (unit, validator)) in plan
        .final_units
        .iter()
        .zip(&inventory.validators)
        .enumerate()
    {
        if unit.validator != validator.slug
            || unit.bytes.is_empty()
            || unit.bytes.len() > 1024 * 1024
            || sha256_hex(&unit.bytes) != unit.sha256
        {
            return Err(eyre!(
                "beacon final unit is not its exact signed validator slot"
            ));
        }
        let text = std::str::from_utf8(&unit.bytes)
            .map_err(|_| eyre!("beacon final unit is not UTF-8"))?;
        let config = format!("{}/current/config/beacon.toml", validator.service_root);
        let selected_peer = inventory.validator_clients[index]
            .peer_id
            .parse::<PeerId>()?;
        let seat = request
            .target_roster
            .iter()
            .position(|peer| peer == &selected_peer)
            .ok_or_else(|| eyre!("beacon unit has no exact roster seat"))?;
        if !text.contains(&config)
            || !text.contains(
                credential_path(inventory, seat)
                    .to_str()
                    .ok_or_else(|| eyre!("beacon credential path is not UTF-8"))?,
            )
        {
            return Err(eyre!(
                "beacon final unit omits its exact config/credential paths"
            ));
        }
    }
    Ok(())
}

/// Only the three public provider fields can differ from the original private config.
/// This never interprets or opens any subsystem's private file selector.
fn derive_config(initial: &[u8], provider: &ProviderV1) -> Result<Zeroizing<Vec<u8>>> {
    if initial.is_empty() || initial.len() as u64 > CONFIG_LIMIT {
        return Err(eyre!("beacon config input exceeds its native bound"));
    }
    let text = Zeroizing::new(
        String::from_utf8(initial.to_vec())
            .map_err(|_| eyre!("beacon config input is not UTF-8"))?,
    );
    let mut table: toml::Table =
        toml::from_str(&text).map_err(|_| eyre!("beacon config input is not native TOML"))?;
    let result = (|| {
        if table.contains_key("extends") {
            return Err(eyre!(
                "beacon config projection forbids inherited mutable configuration"
            ));
        }
        let sumeragi = table
            .get_mut("sumeragi")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| eyre!("beacon config omits sumeragi"))?;
        if PROVIDER_FIELDS
            .iter()
            .any(|field| sumeragi.contains_key(*field))
        {
            return Err(eyre!(
                "fresh beacon projection found an existing provider binding"
            ));
        }
        sumeragi.insert(
            PROVIDER_FIELDS[0].into(),
            toml::Value::String(provider.handle.clone()),
        );
        sumeragi.insert(
            PROVIDER_FIELDS[1].into(),
            toml::Value::Integer(
                i64::try_from(provider.revision)
                    .map_err(|_| eyre!("beacon provider revision exceeds TOML integer range"))?,
            ),
        );
        sumeragi.insert(
            PROVIDER_FIELDS[2].into(),
            toml::Value::String(hex::encode(provider.policy_digest)),
        );
        let output = Zeroizing::new(
            toml::to_string(&table)
                .map_err(|_| eyre!("beacon config cannot be encoded"))?
                .into_bytes(),
        );
        if output.len() as u64 > CONFIG_LIMIT {
            return Err(eyre!("derived beacon config exceeds bound"));
        }
        Ok(output)
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct StartedV1 {
    schema: String,
    authorization_sha256: String,
    request_sha256: String,
    initial_height: u64,
    initial_evidence_sha256: String,
}

/// A retained marker is written before spawn. It never authorizes a new ceremony
/// after process loss, even when no ordinary mutation was yet observed.
fn mark_started(
    root: &Path,
    authorization: &str,
    request: &[u8],
    height: &VerifiedCommittedHeightV1,
) -> Result<()> {
    if root.join("started.json").try_exists()? {
        return Err(eyre!(
            "beacon ceremony was already started; absent final bundle requires explicit fresh preparation"
        ));
    }
    let evidence = canonical_json_report_bytes(&json::to_value(height)?)?;
    reset::inputs::write_new_private(&root.join("height-initial.json"), &evidence)?;
    let marker = StartedV1 {
        schema: MARKER_SCHEMA.into(),
        authorization_sha256: authorization.into(),
        request_sha256: sha256_hex(request),
        initial_height: height.committed_height().get(),
        initial_evidence_sha256: sha256_hex(&evidence),
    };
    reset::inputs::write_new_private(
        &root.join("started.json"),
        &canonical_json_report_bytes(&json::to_value(&marker)?)?,
    )
}

fn require_new_ceremony(root: &Path, next: usize) -> Result<()> {
    if next != 0 || root.join("started.json").try_exists()? {
        return Err(eyre!(
            "beacon ceremony process was lost before its final bundle; retain this attempt and prepare a fresh deployment explicitly"
        ));
    }
    Ok(())
}

/// Own only the child/process group created by this controller invocation.
/// Neither the shared host-action lock nor a remote HostAction spans its lifetime.
struct CeremonyChild {
    child: Child,
    height_writer: Option<File>,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stdout_bytes: Vec<u8>,
    stderr_bytes: Vec<u8>,
    deadline: Instant,
    last_height: u64,
    complete: bool,
}

impl Drop for CeremonyChild {
    fn drop(&mut self) {
        self.height_writer.take();
        if !self.complete {
            let _ = terminate_owned_child(&mut self.child);
        }
    }
}

impl CeremonyChild {
    #[cfg(unix)]
    #[allow(
        unsafe_code,
        reason = "only the controller-owned FIFO read descriptor survives into its owned child"
    )]
    fn spawn(
        program: &Path,
        mut args: Vec<OsString>,
        initial_height: u64,
        deadline: Instant,
    ) -> Result<Self> {
        ensure_local_deadline(Some(deadline))?;
        let (reader, writer) = std::io::pipe()?;
        let reader = File::from(std::os::fd::OwnedFd::from(reader));
        let writer = File::from(std::os::fd::OwnedFd::from(writer));
        let fd = reader.as_raw_fd();
        if fd < 3 || matches!(fd, 198..=200) {
            return Err(eyre!(
                "beacon height pipe overlaps reserved signer descriptors"
            ));
        }
        args.extend(["--height-fd".into(), fd.to_string().into()]);
        let mut command = Command::new(program);
        command
            .args(args)
            .env_clear()
            .env("LC_ALL", "C")
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        unsafe {
            command.pre_exec(move || {
                rustix::io::fcntl_setfd(
                    std::os::fd::BorrowedFd::borrow_raw(fd),
                    rustix::io::FdFlags::empty(),
                )
                .map_err(std::io::Error::from)
            });
        }
        let mut child = command
            .spawn()
            .wrap_err("could not spawn native beacon provisioner")?;
        drop(reader);
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = terminate_owned_child(&mut child);
            return Err(eyre!("beacon child public pipes missing"));
        };
        let result = Self {
            child,
            height_writer: Some(writer),
            stdout,
            stderr,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            deadline,
            last_height: initial_height,
            complete: false,
        };
        for fd in [
            result.stdout.as_fd(),
            result.stderr.as_fd(),
            result.height_writer.as_ref().unwrap().as_fd(),
        ] {
            let flags = rustix::fs::fcntl_getfl(fd)?;
            rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
        }
        Ok(result)
    }

    fn poll(&mut self) -> Result<Option<ExitStatus>> {
        ensure_local_deadline(Some(self.deadline))?;
        drain_public_pipe(&mut self.stdout, &mut self.stdout_bytes)?;
        drain_public_pipe(&mut self.stderr, &mut self.stderr_bytes)?;
        let status = self.child.try_wait()?;
        ensure_local_deadline(Some(self.deadline))?;
        if let Some(status) = status {
            if !status.success() {
                return Err(eyre!(
                    "native beacon provisioner failed; retain the ceremony marker and discard uninstalled secrets"
                ));
            }
        }
        Ok(status)
    }

    fn require_running(&mut self) -> Result<()> {
        if self.poll()?.is_some() {
            return Err(eyre!(
                "beacon provisioner exited before its final authenticated height"
            ));
        }
        Ok(())
    }

    fn wait_sharing(
        &mut self,
        root: &Path,
        expected: &GlobalThresholdBeaconDkgSessionV1,
    ) -> Result<()> {
        loop {
            if self.poll()?.is_some() {
                return Err(eyre!(
                    "beacon provisioner exited before the controller completed required ledger operations"
                ));
            }
            let path = root.join("ceremony/sharing-snapshot.json");
            if path.try_exists()? {
                let (file, snapshot) = open_pinned_regular(&path, "beacon Sharing snapshot")?;
                let bytes = read_pinned_bytes(
                    &path,
                    "beacon Sharing snapshot",
                    file,
                    &snapshot,
                    PUBLIC_LIMIT,
                )?;
                let snapshot: iroha_core::beacon::GlobalThresholdBeaconDkgSnapshotV1 =
                    json::from_slice(&bytes)?;
                snapshot
                    .validate()
                    .map_err(|error| eyre!("native Sharing snapshot is invalid: {error:?}"))?;
                if &snapshot.session != expected
                    || snapshot.last_updated_height != self.last_height
                    || snapshot.dealer_commitments.len() != 4
                    || !snapshot.complaints.is_empty()
                    || !snapshot.complaint_responses.is_empty()
                {
                    return Err(eyre!(
                        "native Sharing snapshot differs from the authorized fresh session"
                    ));
                }
                return Ok(());
            }
            std::thread::sleep(
                PROCESS_POLL_INTERVAL.min(self.deadline.saturating_duration_since(Instant::now())),
            );
        }
    }

    fn deliver(&mut self, root: &Path, height: &VerifiedCommittedHeightV1) -> Result<()> {
        let current = height.committed_height().get();
        if current <= self.last_height {
            return Err(eyre!(
                "beacon controller cannot repeat or invent an observed height"
            ));
        }
        let evidence = canonical_json_report_bytes(&json::to_value(height)?)?;
        // Durable full evidence precedes the only integer written to the FIFO.
        reset::inputs::write_new_private(&root.join(format!("height-{current}.json")), &evidence)?;
        self.require_running()?;
        let bytes = format!("{current}\n");
        let writer = self
            .height_writer
            .as_mut()
            .ok_or_else(|| eyre!("beacon height pipe already closed"))?;
        match writer.write(bytes.as_bytes()) {
            Ok(count) if count == bytes.len() => self.last_height = current,
            Ok(_) => {
                return Err(eyre!(
                    "beacon height FIFO did not accept the complete atomic record"
                ));
            }
            Err(error) => {
                return Err(error).wrap_err("beacon height FIFO rejected authenticated evidence");
            }
        }
        Ok(())
    }

    fn finish(mut self, root: &Path) -> Result<()> {
        self.height_writer.take();
        while self.poll()?.is_none() {
            std::thread::sleep(
                PROCESS_POLL_INTERVAL.min(self.deadline.saturating_duration_since(Instant::now())),
            );
        }
        let bundle = root.join("ceremony/public-bundle.json");
        let (file, snapshot) = open_pinned_regular(&bundle, "completed native beacon bundle")?;
        let bytes = read_pinned_bytes(
            &bundle,
            "completed native beacon bundle",
            file,
            &snapshot,
            PUBLIC_LIMIT,
        )?;
        let _: PublicBundleV1 = json::from_slice(&bytes)?;
        ensure_local_deadline(Some(self.deadline))?;
        self.complete = true;
        Ok(())
    }
}

fn drain_public_pipe(reader: &mut impl Read, output: &mut Vec<u8>) -> Result<()> {
    let mut buffer = [0; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => {
                if output.len().saturating_add(count) > MAX_PROCESS_OUTPUT {
                    return Err(eyre!("beacon child exceeded bounded public output"));
                }
                output.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error).wrap_err("cannot read beacon child public output"),
        }
    }
}

fn read_public<T: JsonDeserialize>(path: &Path, label: &str) -> Result<(T, Vec<u8>)> {
    let (file, snapshot) = open_pinned_regular(path, label)?;
    let bytes = read_pinned_bytes(path, label, file, &snapshot, PUBLIC_LIMIT)?;
    Ok((
        json::from_slice(&bytes).wrap_err_with(|| format!("{label} is not exact native JSON"))?,
        bytes,
    ))
}

fn plan_genesis(
    inventory: &InventoryV1,
    wire: &[u8],
) -> Result<iroha_genesis::ValidatedGenesisBundle> {
    let plan = &inventory.beacon_bootstrap;
    let manifest = json::from_slice(&plan.genesis_manifest)
        .wrap_err("signed beacon manifest is not native genesis JSON")?;
    let validated = iroha_genesis::validate_prepared_genesis_bundle(
        wire,
        &manifest,
        &plan.genesis_public_key,
        inventory.next_genesis_hash.parse()?,
    )?;
    let roster = validated
        .validator_pops()
        .iter()
        .map(|(key, _)| PeerId::new(key.clone()))
        .collect::<BTreeSet<_>>();
    if roster != plan.request.target_roster.iter().cloned().collect()
        || validated.consensus_metadata().mode
            != iroha_data_model::parameter::system::SumeragiConsensusMode::Npos
    {
        return Err(eyre!(
            "beacon bootstrap requires the exact prepared NPoS genesis roster"
        ));
    }
    let ordered = iroha_core::sumeragi::signed_genesis_voting_peers(&iroha_genesis::GenesisBlock(
        validated.block().clone(),
    ))?;
    if ordered != plan.request.target_roster || ordered != plan.request.authorization_roster {
        return Err(eyre!(
            "beacon request indices differ from the native signed-genesis voting order"
        ));
    }
    Ok(validated)
}

fn peers(inventory: &InventoryV1) -> Result<Vec<DeploymentPeerV1>> {
    inventory
        .validators
        .iter()
        .zip(&inventory.validator_clients)
        .map(|(validator, client)| {
            Ok(DeploymentPeerV1 {
                torii_origin: client.probe_origin.clone(),
                peer_id: client.peer_id.parse()?,
                node_fingerprint: validator.node_fingerprint.parse()?,
                build_fingerprint: validator.build_fingerprint.parse()?,
                config_fingerprint: validator.config_fingerprint.parse()?,
            })
        })
        .collect()
}

fn observe_new(
    observer: &mut AuthenticatedHeightObserverV1,
    clients: &[Client; 4],
    inventory: &InventoryV1,
    deadline: Instant,
) -> Result<VerifiedCommittedHeightV1> {
    loop {
        ensure_local_deadline(Some(deadline))?;
        match observer.observe(clients, inventory.chain_discriminant, deadline)? {
            HeightObservationV1::Verified(height) => return Ok(height),
            HeightObservationV1::Pending => std::thread::sleep(
                Duration::from_millis(100).min(deadline.saturating_duration_since(Instant::now())),
            ),
        }
    }
}

impl<R: ProcessRunner> OpenSshTransport<'_, R> {
    fn beacon_clients(&self, deadline: Instant) -> Result<[Client; 4]> {
        self.runtime
            .validator_client_configs
            .iter()
            .zip(&self.admitted.inventory.validator_clients)
            .map(|(input, selected)| {
                let mut config = load_client_config_for_inventory(
                    input,
                    "beacon observation client",
                    &self.admitted.inventory,
                )?;
                config.torii_api_url = selected.probe_origin.parse()?;
                Ok(Client::builder(config)
                    .build()?
                    .with_request_deadline(deadline))
            })
            .collect::<Result<Vec<_>>>()?
            .try_into()
            .map_err(|_| eyre!("beacon bootstrap needs exactly four independent client contexts"))
    }

    fn beacon_daemon(&self) -> Result<PathBuf> {
        let validator = &self.admitted.inventory.validators[0];
        let path = self.closure.file(&validator.slug, "iroha3d")?;
        validate_snapshot_file(path, artifact(&validator.artifacts, "iroha3d")?)?;
        Ok(path.to_path_buf())
    }

    fn run_beacon_native(
        &mut self,
        args: Vec<OsString>,
        inherited_files: Vec<File>,
        deadline: Instant,
    ) -> Result<Vec<u8>> {
        let program = self.beacon_daemon()?;
        let output = self.runner.run(&ProcessSpec {
            program,
            args,
            stdin_prefix: Vec::new(),
            stdin_file: None,
            stdin_files: Vec::new(),
            inherited_files,
            deadline,
        })?;
        if !output.status.success() {
            return Err(eyre!("same-release native beacon operation failed"));
        }
        self.beacon_daemon()?;
        Ok(output.stdout)
    }

    /// Start the one fresh process, execute the three existing canaries once, and
    /// hand it only the final authenticated height after all three applied.
    fn provision_beacon(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        next: usize,
        deadline: Instant,
    ) -> Result<()> {
        let inventory = &self.admitted.inventory;
        validate_plan(inventory)?;
        let root = ceremony_root(inventory);
        if root.join("complete.json").try_exists()? {
            validate_completed_ceremony(inventory, &self.admitted.authorization_sha256, &root)?;
            return Ok(());
        }
        require_new_ceremony(&root, next)?;
        // The authenticated local apply process already owns its journal lock.
        // This separate public-reset namespace is owner-private and never action.lock.
        ensure_private_directory(Path::new("/var/lib/taira/.public-reset-control-v1/beacon"))?;
        ensure_private_directory(&root)?;
        let first = &inventory.validators[0];
        let mut wire = self.closure.stream_file(&first.slug, "genesis")?;
        wire.rewind()?;
        let mut genesis_wire = Vec::new();
        wire.take(PUBLIC_LIMIT + 1).read_to_end(&mut genesis_wire)?;
        if genesis_wire.len() as u64 > PUBLIC_LIMIT
            || sha256_hex(&genesis_wire) != artifact(&first.artifacts, "genesis")?.sha256
        {
            return Err(eyre!("beacon genesis differs from its signed artifact"));
        }
        let genesis = plan_genesis(inventory, &genesis_wire)?;
        let clients = self.beacon_clients(deadline)?;
        let mut observer = AuthenticatedHeightObserverV1::new(&genesis, peers(inventory)?)?;
        let initial = observe_new(&mut observer, &clients, inventory, deadline)?;
        let request = &inventory.beacon_bootstrap.request;
        let state = GlobalThresholdBeaconDkgStateV1::new(
            request.dkg_session,
            &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
        )
        .map_err(|_| eyre!("invalid DKG session"))?;
        if state.phase_at(initial.committed_height().get())
            != GlobalThresholdBeaconDkgPhaseV1::Sharing
        {
            return Err(eyre!(
                "actual committed height is outside the signed fresh DKG sharing window"
            ));
        }
        let request_bytes = canonical_json_report_bytes(&json::to_value(request)?)?;
        reset::inputs::write_new_private(&root.join("request.json"), &request_bytes)?;
        reset::inputs::write_new_private(
            &root.join("genesis-manifest.json"),
            &inventory.beacon_bootstrap.genesis_manifest,
        )?;
        reset::inputs::write_new_private(&root.join("genesis.signed.nrt"), &genesis_wire)?;
        reset::inputs::write_new_private(
            &root.join("genesis.public-key"),
            format!("{}\n", inventory.beacon_bootstrap.genesis_public_key).as_bytes(),
        )?;
        mark_started(
            &root,
            &self.admitted.authorization_sha256,
            &request_bytes,
            &initial,
        )?;
        let args = vec![
            "beacon-bootstrap".into(),
            "provision".into(),
            "--request".into(),
            root.join("request.json").into_os_string(),
            "--genesis-manifest".into(),
            root.join("genesis-manifest.json").into_os_string(),
            "--genesis-signed".into(),
            root.join("genesis.signed.nrt").into_os_string(),
            "--genesis-public-key".into(),
            root.join("genesis.public-key").into_os_string(),
            "--observed-height".into(),
            initial.committed_height().to_string().into(),
            "--output".into(),
            root.join("ceremony").into_os_string(),
            "--timeout-ms".into(),
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
                .to_string()
                .into(),
        ];
        let mut child = CeremonyChild::spawn(
            &self.beacon_daemon()?,
            args,
            initial.committed_height().get(),
            deadline,
        )?;
        child.wait_sharing(
            &root,
            &self.admitted.inventory.beacon_bootstrap.request.dkg_session,
        )?;
        let mut final_height = None;
        for (index, kind) in ["onboarding", "faucet", "write_canary"]
            .into_iter()
            .enumerate()
        {
            child.require_running()?;
            self.run_journaled_write_canary_child(
                progress,
                index,
                remaining_seconds(deadline)?,
                "pre_edge",
                kind,
            )?;
            let observed =
                observe_new(&mut observer, &clients, &self.admitted.inventory, deadline)?;
            reset::inputs::write_new_private(
                &root.join(format!("after-{kind}.json")),
                &canonical_json_report_bytes(&json::to_value(&observed)?)?,
            )?;
            final_height = Some(observed);
        }
        let observed = final_height
            .ok_or_else(|| eyre!("beacon bootstrap omitted required ledger operations"))?;
        if observed.committed_height().get()
            < self
                .admitted
                .inventory
                .beacon_bootstrap
                .request
                .dkg_session
                .responses_end_height
        {
            return Err(eyre!(
                "the three required operations did not reach DKG response completion; no empty carrier is permitted"
            ));
        }
        child.deliver(&root, &observed)?;
        child.finish(&root)?;
        let (bundle, bytes) = read_public::<PublicBundleV1>(
            &root.join("ceremony/public-bundle.json"),
            "beacon public bundle",
        )?;
        validate_bundle_identity(&self.admitted.inventory, &bundle)?;
        let completed = CompletedV1 {
            schema: "iroha.taira.public-reset.beacon-completed.v1".into(),
            authorization_sha256: self.admitted.authorization_sha256.clone(),
            bundle_sha256: sha256_hex(&bytes),
            finalized_observed_height: observed.committed_height().get(),
            provisioner_exit_code: 0,
        };
        reset::inputs::write_new_private(
            &root.join("complete.json"),
            &canonical_json_report_bytes(&json::to_value(&completed)?)?,
        )
    }
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CompletedV1 {
    schema: String,
    authorization_sha256: String,
    bundle_sha256: String,
    finalized_observed_height: u64,
    provisioner_exit_code: i32,
}

fn validate_bundle_identity(inventory: &InventoryV1, bundle: &PublicBundleV1) -> Result<()> {
    let plan = &inventory.beacon_bootstrap;
    if bundle.schema != BUNDLE_SCHEMA
        || json::to_value(&bundle.request)? != json::to_value(&plan.request)?
        || json::to_value(&bundle.genesis.manifest)?
            != json::from_slice::<json::Value>(&plan.genesis_manifest)?
        || bundle.genesis.public_key != plan.genesis_public_key
        || bundle.providers.len() != 4
        || bundle.record.session.adaptive_dkg.session != plan.request.dkg_session
        || bundle.record.session.adaptive_dkg.finalized_at_height
            != bundle.finalized_observed_height
        || bundle.certificate.effective_height
            != bundle
                .finalized_observed_height
                .checked_add(1)
                .ok_or_else(|| eyre!("beacon install height overflow"))?
    {
        return Err(eyre!(
            "native beacon bundle differs from the exact signed bootstrap plan"
        ));
    }
    let _ = plan_genesis(inventory, &bundle.genesis.signed_wire)?;
    for (index, provider) in bundle.providers.iter().enumerate() {
        if provider.signer_index != u16::try_from(index + 1)?
            || provider.validator != plan.request.target_roster[index]
            || provider.handle != plan.request.provider_handles[index]
            || provider.revision != plan.request.provider_revision
        {
            return Err(eyre!(
                "native beacon provider differs from its exact signed seat"
            ));
        }
    }
    Ok(())
}

fn validate_completed_ceremony(
    inventory: &InventoryV1,
    authorization: &str,
    root: &Path,
) -> Result<PublicBundleV1> {
    let (completed, _) =
        read_public::<CompletedV1>(&root.join("complete.json"), "beacon completion")?;
    let (bundle, bytes) =
        read_public::<PublicBundleV1>(&root.join("ceremony/public-bundle.json"), "beacon bundle")?;
    if completed.schema != "iroha.taira.public-reset.beacon-completed.v1"
        || completed.authorization_sha256 != authorization
        || completed.bundle_sha256 != sha256_hex(&bytes)
        || completed.provisioner_exit_code != 0
        || completed.finalized_observed_height != bundle.finalized_observed_height
    {
        return Err(eyre!(
            "retained beacon bundle lacks its exact successful process receipt"
        ));
    }
    validate_bundle_identity(inventory, &bundle)?;
    Ok(bundle)
}

struct VerifiedInstall {
    bundle: PublicBundleV1,
    bundle_sha256: String,
    instructions: Vec<iroha_data_model::isi::InstructionBox>,
}

/// The same-release daemon remains the single public bundle/policy codec owner.
/// It revalidates the public transcript and every provider digest on each admission;
/// neither the receipt nor an arbitrary JSON digest is substituted for that check.
fn verify_native_install(
    inventory: &InventoryV1,
    authorization: &str,
    program: &Path,
    root: &Path,
    deadline: Instant,
    runner: &mut impl ProcessRunner,
) -> Result<VerifiedInstall> {
    let bundle = validate_completed_ceremony(inventory, authorization, root)?;
    let bundle_path = root.join("ceremony/public-bundle.json");
    let (_, before) = read_public::<PublicBundleV1>(&bundle_path, "beacon bundle")?;
    let validator = &inventory.validators[0];
    verify_regular_hash(program, &artifact(&validator.artifacts, "iroha3d")?.sha256)?;
    let temporary = tempfile::Builder::new()
        .prefix("native-bundle-check-")
        .tempdir_in(root)?;
    let output = temporary.path().join("instructions.json");
    let mut args = vec![
        "beacon-bootstrap".into(),
        "assemble-install".into(),
        "--bundle".into(),
        bundle_path.clone().into_os_string(),
    ];
    let mut certificate = bundle.certificate.clone();
    certificate.signatures.clear();
    for seat in 0..3 {
        let signature = root.join(format!("signature-{seat}.json"));
        let (signed, _) = read_public(&signature, "beacon lifecycle signature")?;
        certificate.signatures.push(signed);
        args.extend(["--signature".into(), signature.into_os_string()]);
    }
    args.extend(["--output".into(), output.clone().into_os_string()]);
    let result = runner.run(&ProcessSpec {
        program: program.to_path_buf(),
        args,
        stdin_prefix: Vec::new(),
        stdin_file: None,
        stdin_files: Vec::new(),
        inherited_files: Vec::new(),
        deadline,
    })?;
    if !result.status.success() {
        return Err(eyre!("native beacon certificate/bundle validation failed"));
    }
    let (instructions, _) = read_public::<Vec<iroha_data_model::isi::InstructionBox>>(
        &output,
        "native beacon instructions",
    )?;
    let expected = vec![iroha_data_model::isi::InstructionBox::from(
        iroha_data_model::isi::consensus_keys::ApplyThresholdKeyLifecycleCertificateV1 {
            certificate: certificate.clone(),
        },
    )];
    if instructions != expected {
        return Err(eyre!(
            "native beacon assembly changed the exact certified instruction"
        ));
    }
    iroha_core::state::verify_threshold_key_lifecycle_certificate_v1(
        &certificate,
        &inventory.beacon_bootstrap.request.dkg_session.network_id,
        certificate.effective_height,
        &inventory.beacon_bootstrap.request.authorization_roster,
    )
    .map_err(|_| eyre!("beacon install certificate has no exact native quorum"))?;
    let (_, after) = read_public::<PublicBundleV1>(&bundle_path, "beacon bundle")?;
    if before != after {
        return Err(eyre!("beacon bundle changed during native verification"));
    }
    verify_regular_hash(program, &artifact(&validator.artifacts, "iroha3d")?.sha256)?;
    Ok(VerifiedInstall {
        bundle,
        bundle_sha256: sha256_hex(&before),
        instructions,
    })
}

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct InstallEnvelopeV1 {
    schema: String,
    authorization_sha256: String,
    bundle_sha256: String,
    transaction_wire_hex: String,
    transaction_hash: String,
    fee_quote: FeeQuoteResponse,
}

impl InstallEnvelopeV1 {
    fn verify(
        &self,
        inventory: &InventoryV1,
        authorization: &str,
        native: &VerifiedInstall,
    ) -> Result<SignedTransaction> {
        self.verify_transaction(
            inventory,
            authorization,
            &native.bundle_sha256,
            &native.instructions,
        )
    }

    fn verify_transaction(
        &self,
        inventory: &InventoryV1,
        authorization: &str,
        native_bundle_sha256: &str,
        native_instructions: &[iroha_data_model::isi::InstructionBox],
    ) -> Result<SignedTransaction> {
        use iroha_data_model::transaction::Executable;
        let wire = hex::decode(&self.transaction_wire_hex)?;
        let transaction = SignedTransaction::decode_all_versioned(&wire)?;
        transaction.verify_signature()?;
        if self.schema != "iroha.taira.public-reset.beacon-install-envelope.v1"
            || self.authorization_sha256 != authorization
            || self.bundle_sha256 != native_bundle_sha256
            || transaction.encode_wire_v1()? != wire
            || hex::encode(&wire) != self.transaction_wire_hex
            || hex::encode(transaction.hash().as_ref()) != self.transaction_hash
            || transaction.instructions() != &Executable::from(native_instructions.to_vec())
            || transaction.network_id()
                != Some(&inventory.beacon_bootstrap.request.dkg_session.network_id)
            || transaction.authority().to_string() != inventory.canary_onboarding_request.account_id
            || transaction.fee_payment_intent() != &self.fee_quote.intent
            || !inventory_fee_payment_intent(inventory)?
                .has_same_payer_and_gas_bound(transaction.fee_payment_intent())
            || transaction.admission_intent()
                != iroha_data_model::transaction::TransactionAdmissionIntent::Ordinary
            || transaction.attachments().is_some()
            || transaction.multisig_signatures().is_some()
            || !transaction.metadata().is_empty()
        {
            return Err(eyre!(
                "beacon transaction differs from its exact retained installation envelope"
            ));
        }
        self.fee_quote
            .validate_for_signed_payload(transaction.payload())
            .map_err(|error| eyre!(error))?;
        Ok(transaction)
    }
}

fn validate_install_lifetime(
    transaction: &SignedTransaction,
    claims: &reset::AuthorizationClaimsV1,
    forward: bool,
) -> Result<()> {
    validate_prepared_transaction_time_window(
        u64::try_from(transaction.creation_time().as_millis())?,
        u64::try_from(
            transaction
                .time_to_live()
                .ok_or_else(|| eyre!("beacon install omits TTL"))?
                .as_millis(),
        )?,
        if forward { now_unix_ms()? } else { 0 },
        claims.not_before_unix_ms,
        claims.execution_expires_at_unix_ms,
        if forward {
            PreparedMutationLifetimeCheck::LiveForward
        } else {
            PreparedMutationLifetimeCheck::Structural
        },
    )
}

impl<R: ProcessRunner> OpenSshTransport<'_, R> {
    fn sign_beacon_certificate(&mut self, root: &Path, deadline: Instant) -> Result<()> {
        for seat in 0..3 {
            let output = root.join(format!("signature-{seat}.json"));
            if output.try_exists()? {
                continue;
            } // The native assembler verifies existing signatures.
            let peer = &self
                .admitted
                .inventory
                .beacon_bootstrap
                .request
                .authorization_roster[seat];
            let index = self
                .admitted
                .inventory
                .validator_clients
                .iter()
                .position(|client| client.peer_id == peer.to_string())
                .ok_or_else(|| eyre!("beacon authorization seat has no selected validator"))?;
            let validator = self.admitted.inventory.validators[index].clone();
            let initial = self.closure.file(&validator.slug, "config")?;
            validate_snapshot_file(initial, artifact(&validator.artifacts, "config")?)?;
            let (file, snapshot) = open_pinned_regular(initial, "beacon lifecycle config")?;
            let bytes = Zeroizing::new(read_pinned_bytes(
                initial,
                "beacon lifecycle config",
                file,
                &snapshot,
                CONFIG_LIMIT,
            )?);
            // The existing native loader scrubs/truncates this disposable copy. It
            // never consumes the persistent signed config or prints private bytes.
            let temporary = tempfile::Builder::new()
                .prefix("lifecycle-key-")
                .tempdir_in(root)?;
            let path = temporary.path().join("config.toml");
            reset::inputs::write_new_private(&path, &bytes)?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(
                    i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits())
                        .wrap_err("native no-follow open flag exceeds signed platform range")?,
                )
                .open(&path)?;
            let descriptor = File::from(rustix::io::fcntl_dupfd_cloexec(&file, 198)?);
            if descriptor.as_raw_fd() != 198 {
                return Err(eyre!(
                    "reserved beacon lifecycle descriptor 198 is occupied"
                ));
            }
            let args = vec![
                "beacon-bootstrap".into(),
                "sign-install".into(),
                "--bundle".into(),
                root.join("ceremony/public-bundle.json").into_os_string(),
                "--signer-index".into(),
                seat.to_string().into(),
                "--config-fd".into(),
                "198".into(),
                "--output".into(),
                output.into_os_string(),
            ];
            let result = self.run_beacon_native(args, vec![descriptor], deadline);
            // Child failure cannot leave a private launch copy available for reuse.
            let scrub = file.set_len(0).and_then(|_| file.sync_all());
            result?;
            scrub?;
            self.closure
                .revalidate_host(&validator.slug, &validator.artifacts)?;
        }
        Ok(())
    }

    fn install_beacon(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        recovery_only: bool,
        deadline: Instant,
    ) -> Result<()> {
        use iroha_data_model::transaction::Executable;
        use iroha_model_base::metadata::Metadata;
        let root = ceremony_root(&self.admitted.inventory);
        if !recovery_only {
            self.sign_beacon_certificate(&root, deadline)?;
        }
        let program = self.beacon_daemon()?;
        let native = verify_native_install(
            &self.admitted.inventory,
            &self.admitted.authorization_sha256,
            &program,
            &root,
            deadline,
            &mut self.runner,
        )?;
        let path = root.join("install.prepared.json");
        let client = self.beacon_clients(deadline)?;
        let mut config = load_client_config_for_inventory(
            &self.runtime.client_config,
            "beacon transaction signer",
            &self.admitted.inventory,
        )?;
        config.torii_api_url = self.admitted.inventory.validator_clients[0]
            .probe_origin
            .parse()?;
        let blocking = iroha::blocking::Client::from_client(
            Client::builder(config)
                .build()?
                .with_request_deadline(deadline),
        )?;
        if !path.try_exists()? {
            if recovery_only {
                return Err(eyre!("submitted beacon install has no retained envelope"));
            }
            let fees = inventory_fee_payment_intent(&self.admitted.inventory)?;
            let (transaction, fee_quote) =
                crate::quote_and_sign_transaction_with_admission_and_expiry(
                    &blocking,
                    Executable::from(native.instructions.clone()),
                    fees,
                    Metadata::default(),
                    iroha_data_model::transaction::TransactionAdmissionIntent::Ordinary,
                    self.admitted
                        .authorization
                        .claims
                        .execution_expires_at_unix_ms,
                )?;
            if fee_quote.observation.next_block_height != native.bundle.certificate.effective_height
            {
                return Err(eyre!(
                    "beacon install height advanced after DKG; the certified instruction cannot be replaced"
                ));
            }
            let retained = InstallEnvelopeV1 {
                schema: "iroha.taira.public-reset.beacon-install-envelope.v1".into(),
                authorization_sha256: self.admitted.authorization_sha256.clone(),
                bundle_sha256: native.bundle_sha256.clone(),
                transaction_wire_hex: hex::encode(transaction.encode_wire_v1()?),
                transaction_hash: hex::encode(transaction.hash().as_ref()),
                fee_quote,
            };
            retained.verify(
                &self.admitted.inventory,
                &self.admitted.authorization_sha256,
                &native,
            )?;
            reset::inputs::write_new_private(
                &path,
                &canonical_json_report_bytes(&json::to_value(&retained)?)?,
            )?;
        }
        let (retained, _) = read_public::<InstallEnvelopeV1>(&path, "beacon install envelope")?;
        let transaction = retained.verify(
            &self.admitted.inventory,
            &self.admitted.authorization_sha256,
            &native,
        )?;
        validate_install_lifetime(
            &transaction,
            &self.admitted.authorization.claims,
            !recovery_only,
        )?;
        if !recovery_only {
            progress.mark_submitted(3)?;
        }
        let result = (|| {
            let submitted = root.join("install.submitted.json");
            if !submitted.try_exists()? {
                if recovery_only {
                    return Err(eyre!("beacon recovery cannot create a submission claim"));
                }
                ensure_authorization_current(self.admitted)?;
                // Global cohost custody, create-new and fsync precede the sole POST.
                reset::inputs::write_new_private(
                    &submitted,
                    &canonical_json_report_bytes(&json::to_value(&retained)?)?,
                )?;
                let _accepted_or_ambiguous = blocking.submit_transaction(&transaction);
            } else {
                let (claimed, _) =
                    read_public::<InstallEnvelopeV1>(&submitted, "beacon submitted claim")?;
                if json::to_value(&claimed)? != json::to_value(&retained)? {
                    return Err(eyre!("beacon submission claim changed its exact envelope"));
                }
            }
            if root.join("install.applied.json").try_exists()? {
                let receipt = validate_installation_proof(
                    &root,
                    &self.admitted.inventory,
                    &self.admitted.authorization_sha256,
                    &self.admitted.authorization.claims,
                    &native,
                )?;
                ensure_local_deadline(Some(deadline))?;
                self.publish_local_receipt("beacon-install-pre_edge.json", &receipt)?;
                return progress.mark_applied(3);
            }
            let options = iroha::client::TransactionWaitOptions {
                timeout: deadline.saturating_duration_since(Instant::now()),
                poll_interval: Duration::from_millis(200),
            };
            let outcome = blocking
                .client()
                .wait_for_transaction_applied(transaction.hash(), options)?;
            if outcome.block_height != Some(native.bundle.certificate.effective_height) {
                return Err(eyre!(
                    "beacon installation applied outside its certified exact height"
                ));
            }
            let genesis =
                plan_genesis(&self.admitted.inventory, &native.bundle.genesis.signed_wire)?;
            let mut observer =
                AuthenticatedHeightObserverV1::new(&genesis, peers(&self.admitted.inventory)?)?;
            let height = observe_new(&mut observer, &client, &self.admitted.inventory, deadline)?;
            let carrier_height =
                std::num::NonZeroU64::new(native.bundle.certificate.effective_height)
                    .ok_or_else(|| eyre!("zero install height"))?;
            let proof = height.proof_at(carrier_height).ok_or_else(|| {
                eyre!("beacon install is ahead of authenticated durable finality")
            })?;
            let mut exact_wire = None;
            let mut committed_transaction = None;
            for peer in &client {
                let status = peer.wait_for_transaction_applied(
                    transaction.hash(),
                    iroha::client::TransactionWaitOptions {
                        timeout: deadline.saturating_duration_since(Instant::now()),
                        poll_interval: Duration::from_millis(200),
                    },
                )?;
                if status.block_height != Some(carrier_height.get()) {
                    return Err(eyre!(
                        "validator reports another beacon installation height"
                    ));
                }
                let details =
                    peer.get_successful_transaction_details(transaction.hash_as_entrypoint())?;
                if details.transaction.entrypoint()
                    != &iroha_data_model::transaction::TransactionEntrypoint::External(
                        transaction.clone(),
                    )
                    || details.transaction.block_hash() != &proof.block_header.hash()
                {
                    return Err(eyre!(
                        "beacon committed transaction differs from the retained envelope and authenticated carrier"
                    ));
                }
                let wire = peer.get_canonical_executed_block_wire(
                    carrier_height,
                    &details.transaction,
                    &proof.finality_artifact.commit_qc.execution_commitment,
                )?;
                if exact_wire
                    .as_ref()
                    .is_some_and(|previous| previous != &wire)
                {
                    return Err(eyre!(
                        "beacon validators disagree on canonical carrier bytes"
                    ));
                }
                committed_transaction = Some(json::to_value(&details.transaction)?);
                exact_wire = Some(wire);
            }
            let wire = exact_wire.ok_or_else(|| eyre!("beacon carrier evidence absent"))?;
            let applied_path = root.join("install.applied.json");
            let authorization_sha256 = self.admitted.authorization_sha256.clone();
            let transaction_hash = retained.transaction_hash;
            let bundle_sha256 = native.bundle_sha256.clone();
            let carrier_wire_hex = hex::encode(wire);
            let height_evidence = json::to_value(&height)?;
            let receipt = norito::json!({
                "schema": "iroha.taira.public-reset.beacon-install-applied.v1",
                "authorization_sha256": authorization_sha256,
                "transaction_hash": transaction_hash,
                "bundle_sha256": bundle_sha256,
                "carrier_wire_hex": carrier_wire_hex,
                "committed_transaction": committed_transaction,
                "height_evidence": height_evidence
            });
            if !applied_path.try_exists()? {
                reset::inputs::write_new_private(
                    &applied_path,
                    &canonical_json_report_bytes(&receipt)?,
                )?;
            }
            let receipt = validate_installation_proof(
                &root,
                &self.admitted.inventory,
                &self.admitted.authorization_sha256,
                &self.admitted.authorization.claims,
                &native,
            )?;
            ensure_local_deadline(Some(deadline))?;
            self.publish_local_receipt("beacon-install-pre_edge.json", &receipt)?;
            progress.mark_applied(3)
        })();
        result.map_err(|error: eyre::Report| {
            error.wrap_err(LocalMutationRecoveryPending {
                action: "beacon_install",
            })
        })
    }
}

fn validate_installation_proof(
    root: &Path,
    inventory: &InventoryV1,
    authorization: &str,
    claims: &reset::AuthorizationClaimsV1,
    native: &VerifiedInstall,
) -> Result<json::Value> {
    use iroha_data_model::{
        block::consensus_v2::{ConsensusMode, ValidatorPower},
        bridge::{BridgeFinalityProof, BridgeFinalityVerifier},
        query::CommittedTransaction,
    };
    let (envelope, _) = read_public::<InstallEnvelopeV1>(
        &root.join("install.prepared.json"),
        "beacon install envelope",
    )?;
    let transaction = envelope.verify(inventory, authorization, native)?;
    validate_install_lifetime(&transaction, claims, false)?;
    let (receipt, _) = read_public::<json::Value>(
        &root.join("install.applied.json"),
        "beacon installation proof",
    )?;
    let object = receipt
        .as_object()
        .ok_or_else(|| eyre!("beacon proof is not an object"))?;
    let string = |field: &str| {
        object
            .get(field)
            .and_then(json::Value::as_str)
            .ok_or_else(|| eyre!("beacon proof omits {field}"))
    };
    if string("schema")? != "iroha.taira.public-reset.beacon-install-applied.v1"
        || string("authorization_sha256")? != authorization
        || string("transaction_hash")? != envelope.transaction_hash
        || string("bundle_sha256")? != native.bundle_sha256
    {
        return Err(eyre!(
            "beacon installation proof belongs to another authorization"
        ));
    }
    let evidence = object
        .get("height_evidence")
        .and_then(json::Value::as_object)
        .ok_or_else(|| eyre!("beacon height evidence absent"))?;
    let proofs: Vec<BridgeFinalityProof> = json::from_value(
        evidence
            .get("proofs")
            .cloned()
            .ok_or_else(|| eyre!("beacon proof chain absent"))?,
    )?;
    let genesis = plan_genesis(inventory, &native.bundle.genesis.signed_wire)?;
    let first = proofs
        .first()
        .ok_or_else(|| eyre!("beacon proof chain is empty"))?;
    let peers = genesis
        .validator_pops()
        .iter()
        .map(|(key, pop)| (PeerId::new(key.clone()), pop.clone()))
        .collect::<BTreeMap<_, _>>();
    let (roster, pops): (Vec<_>, Vec<_>) = peers
        .into_iter()
        .map(|(validator, pop)| {
            (
                ValidatorPower {
                    validator,
                    power: 1,
                },
                pop,
            )
        })
        .unzip();
    if first.block_header.height().get() != 1
        || first.block_header.hash() != genesis.expected_hash()
    {
        return Err(eyre!(
            "beacon proof chain has no authenticated genesis anchor"
        ));
    }
    let network = inventory.beacon_bootstrap.request.dkg_session.network_id;
    let mut verifier =
        BridgeFinalityVerifier::with_context(network, first.finality_artifact.context_id());
    let mut carrier = None;
    for proof in &proofs {
        let artifact = &proof.finality_artifact;
        if artifact.height_context.network_id != network
            || artifact.height_context.mode != ConsensusMode::Npos
            || artifact.height_context.roster != roster
            || artifact.validator_set_pops != pops
            || artifact.height_context.snapshot_bootstrap.is_some()
            || artifact.commit_qc.signers.len() != 3
        {
            return Err(eyre!("beacon proof chain changes its exact genesis roster"));
        }
        verifier.verify(proof)?;
        if proof.block_header.height().get() == native.bundle.certificate.effective_height {
            carrier = Some(proof);
        }
    }
    let proof =
        carrier.ok_or_else(|| eyre!("beacon proof chain omits the installation carrier"))?;
    let committed: CommittedTransaction = json::from_value(
        object
            .get("committed_transaction")
            .cloned()
            .ok_or_else(|| eyre!("beacon committed transaction missing"))?,
    )?;
    let wire = hex::decode(string("carrier_wire_hex")?)?;
    let block = iroha_data_model::block::decode_framed_signed_block(&wire)?;
    if block.encode_wire()? != wire
        || block.header() != proof.block_header
        || committed.block_hash() != &block.hash()
        || committed.entrypoint()
            != &iroha_data_model::transaction::TransactionEntrypoint::External(transaction)
        || committed.result().is_err()
        || !committed.verify_inclusion_in_authenticated_execution(
            &block,
            &proof.finality_artifact.commit_qc.execution_commitment,
        )
    {
        return Err(eyre!(
            "beacon installation is not successful in the independently authenticated execution"
        ));
    }
    Ok(receipt)
}

#[derive(Clone, JsonSerialize, JsonDeserialize, PartialEq, Eq)]
#[norito(deny_unknown_fields)]
struct ProviderActivationV1 {
    schema: String,
    authorization_sha256: String,
    bundle_sha256: String,
    validator: String,
    session_id: [u8; 32],
    config_sha256: String,
    unit_sha256: String,
}

pub(super) struct ActiveBinding {
    pub(super) config: PathBuf,
    pub(super) config_sha256: String,
    pub(super) unit_sha256: String,
}

fn provider_projection(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<(ProviderActivationV1, Zeroizing<Vec<u8>>, PathBuf, Vec<u8>)> {
    let inventory = &admitted.inventory;
    let root = ceremony_root(inventory);
    let daemon =
        PathBuf::from(&artifact(&inventory.validators[0].artifacts, "iroha3d")?.remote_path);
    let native = verify_native_install(
        inventory,
        &admitted.authorization_sha256,
        &daemon,
        &root,
        admitted.action_deadline,
        &mut RealProcessRunner,
    )?;
    validate_installation_proof(
        &root,
        inventory,
        &admitted.authorization_sha256,
        &admitted.authorization.claims,
        &native,
    )?;
    let role = inventory
        .validators
        .iter()
        .position(|candidate| candidate.slug == validator.slug)
        .ok_or_else(|| eyre!("unknown beacon validator role"))?;
    let peer = inventory.validator_clients[role]
        .peer_id
        .parse::<PeerId>()?;
    let provider = native
        .bundle
        .providers
        .iter()
        .find(|provider| provider.validator == peer)
        .ok_or_else(|| eyre!("beacon public provider seat absent"))?;
    let source = artifact(&validator.artifacts, "config")?;
    verify_regular_hash(Path::new(&source.remote_path), &source.sha256)?;
    let (file, snapshot) =
        open_pinned_regular(Path::new(&source.remote_path), "original validator config")?;
    let bytes = Zeroizing::new(read_pinned_bytes(
        Path::new(&source.remote_path),
        "original validator config",
        file,
        &snapshot,
        CONFIG_LIMIT,
    )?);
    let config = derive_config(&bytes, provider)?;
    let unit = &inventory.beacon_bootstrap.final_units[role];
    let projected = Path::new(&source.remote_path).with_file_name("beacon.toml");
    reset::validate_validator_genesis_config(
        &config,
        Path::new(&artifact(&validator.artifacts, "genesis")?.remote_path),
        &inventory.next_genesis_hash,
    )?;
    reset::validate_validator_operator_config(&config, &inventory.operator_public_key)?;
    let marker = ProviderActivationV1 {
        schema: "iroha.taira.public-reset.beacon-provider-active.v1".into(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        bundle_sha256: native.bundle_sha256,
        validator: validator.slug.clone(),
        session_id: native.bundle.record.session.session_id,
        config_sha256: sha256_hex(&config),
        unit_sha256: unit.sha256.clone(),
    };
    Ok((marker, config, projected, unit.bytes.clone()))
}

pub(super) fn active_binding(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<Option<ActiveBinding>> {
    let root = ceremony_root(&admitted.inventory);
    let path = root.join(format!("{}.active.json", validator.slug));
    if !path.try_exists()? {
        return Ok(None);
    }
    let (retained, _) = read_public::<ProviderActivationV1>(&path, "beacon active binding")?;
    let (expected, config, projected, _) = provider_projection(admitted, validator)?;
    if retained != expected {
        return Err(eyre!(
            "beacon active config/unit record differs from exact signed derivation"
        ));
    }
    verify_regular_hash(&projected, &sha256_hex(&config))?;
    Ok(Some(ActiveBinding {
        config: projected,
        config_sha256: retained.config_sha256,
        unit_sha256: retained.unit_sha256,
    }))
}

/// Authenticate the optional derived unit even if publication stopped before
/// the active marker. Rollback may restore exactly this signed successor.
pub(super) fn prepared_unit_hash(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<Option<String>> {
    let path = ceremony_root(&admitted.inventory)
        .join(format!("{}.activation-prepared.json", validator.slug));
    if !path.try_exists()? {
        return Ok(None);
    }
    let (retained, _) = read_public::<ProviderActivationV1>(&path, "beacon provider intent")?;
    let (expected, _, _, _) = provider_projection(admitted, validator)?;
    if retained != expected {
        return Err(eyre!(
            "beacon rollback unit intent differs from signed derivation"
        ));
    }
    Ok(Some(expected.unit_sha256))
}

pub(super) fn recover_provider_host(
    admitted: &HostAdmission,
    receipt_dir: &Path,
    receipt_name: &str,
    progress: &mut HostProgressV1,
    decision: HostProgressDecision,
) -> Result<HostReceiptV1> {
    let action = HostAction::BeaconActivate;
    let existing = read_existing_host_receipt(receipt_dir, receipt_name, admitted, action)?;
    if decision == HostProgressDecision::Replay && existing.is_none() {
        return Err(eyre!(
            "provider progress has no immutable activation receipt"
        ));
    }
    if existing.is_none()
        && progress.prepared_action.as_ref() != Some(&host_action_key(admitted, action))
    {
        return Ok(host_recovery_receipt(
            admitted,
            action,
            "rejected",
            "provider_not_submitted",
        ));
    }
    let HostTarget::Validator(validator) = &admitted.target else {
        return Err(eyre!("provider recovery requires validator target"));
    };
    if revalidate_provider(admitted, validator).is_err() {
        if admitted.execution_expired
            || now_unix_ms()? >= admitted.authorization.claims.execution_expires_at_unix_ms
        {
            return Ok(host_recovery_receipt(
                admitted,
                action,
                "pending",
                "provider_activation_pending",
            ));
        }
        // Recompute the signed native derivation without publishing configuration,
        // replacing units or running manager operations. A subsequent forward
        // command owns those writes and re-verifies the original authorization.
        let (expected, _, projected, _) = provider_projection(admitted, validator)?;
        let root = ceremony_root(&admitted.inventory);
        for suffix in ["activation-prepared", "active"] {
            let path = root.join(format!("{}.{}.json", validator.slug, suffix));
            if path.try_exists()? {
                let (retained, _) =
                    read_public::<ProviderActivationV1>(&path, "provider continuation binding")?;
                if retained != expected {
                    return Err(eyre!("provider continuation binding changed"));
                }
            }
        }
        if projected.try_exists()? {
            verify_regular_hash(&projected, &expected.config_sha256)?;
        }
        let fragment = Path::new("/etc/systemd/system").join(&validator.systemd_unit);
        if occupied::verify_unit_fragment(&fragment, &validator.systemd_unit_sha256).is_err() {
            occupied::verify_unit_fragment(&fragment, &expected.unit_sha256)?;
        }
        if let Some(intent) = retained_provider_start(admitted, validator)? {
            match inspect_manager_operation(&intent, admitted.action_deadline)? {
                ManagerOperationEvidence::Rejected => {
                    return Err(eyre!("beacon start was definitively rejected"));
                }
                ManagerOperationEvidence::Pending | ManagerOperationEvidence::Applied => {
                    return Ok(host_recovery_receipt(
                        admitted,
                        action,
                        "pending",
                        "exact_beacon_start_or_process_pending",
                    ));
                }
                ManagerOperationEvidence::Absent => {}
            }
        }
        return Ok(host_recovery_receipt(
            admitted,
            action,
            "continue",
            "exact_provider_publication_requires_forward_authorization",
        ));
    }
    let mut receipt = existing.unwrap_or_else(|| {
        host_receipt(
            admitted,
            action,
            true,
            0,
            0,
            "provider activation recovered from exact retained configuration and manager evidence",
        )
    });
    receipt.idempotent = true;
    publish_host_receipt(receipt_dir, receipt_name, &receipt)?;
    if decision == HostProgressDecision::Advance {
        advance_host_progress(admitted, action, progress)?;
    }
    Ok(receipt)
}

// Inspect only the existing exact action; this never creates a manager intent.
fn retained_provider_start(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<Option<ManagerIntentV1>> {
    let path = ensure_host_receipt_dir(admitted)?.join(manager_intent_name("beacon-start")?);
    if !path.try_exists()? {
        return Ok(None);
    }
    let (intent, _) = read_private_json::<ManagerIntentV1>(&path, "beacon start intent")?;
    validate_manager_intent(
        admitted,
        "beacon-start",
        "start",
        &validator.systemd_unit,
        &intent,
    )?;
    Ok(Some(intent))
}

// A completed rename is an immutable publication. Repeating it would invalidate
// the already completed daemon-reload job even when the bytes stayed identical.
fn publish_provider_unit(
    source: &Path,
    destination: &Path,
    desired: &ArtifactV1,
    initial_sha256: &str,
    before_rename: impl Fn() -> Result<()>,
) -> Result<()> {
    if verify_regular_hash(destination, &desired.sha256).is_ok() {
        before_rename()?;
        return sync_existing_file_publication(
            destination,
            &desired.sha256,
            source
                .parent()
                .ok_or_else(|| eyre!("provider unit has no source parent"))?,
            sync_directory,
        );
    }
    occupied::publish_unit_bytes_with(
        source,
        destination,
        desired,
        initial_sha256,
        false,
        before_rename,
        sync_directory,
    )
}

pub(super) fn activate_provider(admitted: &HostAdmission, validator: &ValidatorV1) -> Result<()> {
    let (marker, config, projected, unit) = provider_projection(admitted, validator)?;
    let root = ceremony_root(&admitted.inventory);
    let prepared = root.join(format!("{}.activation-prepared.json", validator.slug));
    let bytes = canonical_json_report_bytes(&json::to_value(&marker)?)?;
    if prepared.try_exists()? {
        let (retained, _) =
            read_public::<ProviderActivationV1>(&prepared, "beacon provider intent")?;
        if retained != marker {
            return Err(eyre!("beacon provider intent changed after preparation"));
        }
    } else {
        reset::inputs::write_new_private(&prepared, &bytes)?;
    }
    let active = root.join(format!("{}.active.json", validator.slug));
    if active.try_exists()? {
        let (retained, _) = read_public::<ProviderActivationV1>(&active, "beacon active binding")?;
        if retained != marker {
            return Err(eyre!("beacon active binding changed"));
        }
    }
    let fragment = Path::new("/etc/systemd/system").join(&validator.systemd_unit);
    if occupied::verify_unit_fragment(&fragment, &validator.systemd_unit_sha256).is_err() {
        occupied::verify_unit_fragment(&fragment, &marker.unit_sha256)?;
    }
    if retained_provider_start(admitted, validator)?.is_some() {
        // Publication and stop were completed before the durable start intent.
        // Continue that exact start, never send the already-applied stop again.
        verify_regular_hash(&projected, &marker.config_sha256)?;
        occupied::verify_unit_fragment(&fragment, &marker.unit_sha256)?;
        let binding = active_binding(admitted, validator)?
            .ok_or_else(|| eyre!("retained beacon start has no active binding"))?;
        return finish_provider_start(admitted, validator, &binding);
    }
    stop_unit(admitted, "beacon-stop", &validator.systemd_unit)?;
    if projected.try_exists()? {
        verify_regular_hash(&projected, &marker.config_sha256)?;
    } else {
        reset::inputs::write_new_private(&projected, &config)?;
    }
    let unit_source = root.join(format!("{}.final-unit", validator.slug));
    if !unit_source.try_exists()? {
        reset::inputs::write_new_private(&unit_source, &unit)?;
    }
    verify_regular_hash(&unit_source, &marker.unit_sha256)?;
    let mut unit_artifact = artifact(&validator.artifacts, "validator_unit")?.clone();
    unit_artifact.sha256 = marker.unit_sha256.clone();
    unit_artifact.size = unit.len() as u64;
    publish_provider_unit(
        &unit_source,
        &fragment,
        &unit_artifact,
        &validator.systemd_unit_sha256,
        || ensure_action_deadline(admitted),
    )?;
    run_durable_manager_operation(admitted, "beacon-unit-reload", "daemon-reload", "")?;
    if !active.try_exists()? {
        reset::inputs::write_new_private(&active, &bytes)?;
    }
    let binding = active_binding(admitted, validator)?
        .ok_or_else(|| eyre!("beacon activation was not retained"))?;
    finish_provider_start(admitted, validator, &binding)
}

fn finish_provider_start(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
    binding: &ActiveBinding,
) -> Result<()> {
    attest_loaded_systemd_unit(validator, &binding.unit_sha256, admitted.action_deadline)?;
    run_durable_manager_operation(admitted, "beacon-start", "start", &validator.systemd_unit)?;
    require_unit_active(&validator.systemd_unit, admitted.action_deadline)?;
    let release = Path::new(&validator.service_root)
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    wait_for_validator_process(admitted.action_deadline, || {
        observe_validator_process(admitted, validator, &release, true)
    })
}

/// Vacant rollback restores the initial pre-provisioned unit before quarantining
/// the new release; only the durably bound beacon successor is an allowed source.
pub(super) fn restore_vacant_initial_unit(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
) -> Result<()> {
    let path = Path::new("/etc/systemd/system").join(&validator.systemd_unit);
    if occupied::verify_unit_fragment(&path, &validator.systemd_unit_sha256).is_ok() {
        return Ok(());
    }
    let derived = prepared_unit_hash(admitted, validator)?
        .ok_or_else(|| eyre!("vacant rollback has no exact beacon unit intent"))?;
    occupied::verify_unit_fragment(&path, &derived)?;
    require_unit_stopped(&validator.systemd_unit, admitted.action_deadline)?;
    let initial = artifact(&validator.artifacts, "validator_unit")?;
    atomic_replace_verified_file(admitted, Path::new(&initial.remote_path), &path, initial)?;
    run_durable_manager_operation(admitted, "beacon-unit-rollback-reload", "daemon-reload", "")?;
    attest_loaded_systemd_unit(
        validator,
        &validator.systemd_unit_sha256,
        admitted.action_deadline,
    )
}

pub(super) fn revalidate_provider(admitted: &HostAdmission, validator: &ValidatorV1) -> Result<()> {
    let binding = active_binding(admitted, validator)?
        .ok_or_else(|| eyre!("beacon provider activation is not retained"))?;
    require_session_manager_operation_applied(
        admitted,
        "beacon-start",
        "start",
        &validator.systemd_unit,
    )?;
    attest_loaded_systemd_unit(validator, &binding.unit_sha256, admitted.action_deadline)?;
    require_unit_active(&validator.systemd_unit, admitted.action_deadline)?;
    let release = Path::new(&validator.service_root)
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    attest_validator_process(admitted, validator, &release, true)
}

#[cfg(test)]
pub(in super::super) fn fixture_plan(
    validators: &[ValidatorV1],
    clients: &[reset::ValidatorClientV1],
) -> BeaconBootstrapPlanV1 {
    let mut roster = clients
        .iter()
        .map(|client| client.peer_id.parse::<PeerId>().unwrap())
        .collect::<Vec<_>>();
    roster.sort();
    let request = NativeRequestV1 {
        schema: REQUEST_SCHEMA.into(),
        dkg_session: GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"fixture next Taira genesis",
                )),
            ),
            session_id: [42; 32],
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: 4,
            threshold: 2,
            start_height: 1,
            sharing_end_height: 2,
            complaints_end_height: 3,
            responses_end_height: 4,
        },
        target_roster: roster.clone(),
        authorization_roster: roster.clone(),
        provider_handles: (1..=4)
            .map(|seat| format!("taira-beacon-seat-{seat}"))
            .collect(),
        provider_revision: 1,
    };
    let units = validators.iter().zip(clients).map(|(validator, client)| {
        let peer = client.peer_id.parse::<PeerId>().unwrap();
        let seat = roster.iter().position(|value| value == &peer).unwrap() + 1;
        let bytes = format!("[Service]\nExecStart=/fixture --config {}/current/config/beacon.toml --credential /var/lib/taira/.public-reset-control-v1/beacon/abcdefghijklmnopqrstuvwx12345678/ceremony/seat-{seat}/{CREDENTIAL_FILE}\n", validator.service_root).into_bytes();
        FinalUnitV1 { validator: validator.slug.clone(), sha256: sha256_hex(&bytes), bytes }
    }).collect();
    BeaconBootstrapPlanV1 {
        schema: PLAN_SCHEMA.into(),
        request,
        genesis_manifest: b"{}\n".to_vec(),
        genesis_public_key: iroha_test_samples::ALICE_KEYPAIR.public_key().clone(),
        final_units: units,
    }
}

impl<R: ProcessRunner> OpenSshTransport<'_, R> {
    pub(super) fn run_beacon_prefix(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        next: usize,
        resume_submitted_provider: bool,
        deadline: Instant,
    ) -> Result<()> {
        if resume_submitted_provider && !(4..8).contains(&next) {
            return Err(eyre!("submitted continuation is not a provider activation"));
        }
        if next < 3 {
            self.provision_beacon(progress, next, deadline)?;
        }
        if next <= 3 {
            self.install_beacon(progress, false, deadline)?;
        }
        for index in next.max(4)..8 {
            if !(resume_submitted_provider && index == next) {
                progress.mark_submitted(index)?;
            }
            let validator = self.admitted.inventory.validators[index - 4].clone();
            self.bootstrap_and_dispatch_validator(
                &validator,
                HostAction::BeaconActivate,
                remaining_seconds(deadline)?.min(self.admitted.inventory.timeouts.restart_secs),
            )
            .map_err(|error| {
                error.wrap_err(LocalMutationRecoveryPending {
                    action: "beacon_provider_activation",
                })
            })?;
            progress.mark_applied(index)?;
        }
        Ok(())
    }

    pub(super) fn recover_beacon_install(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        deadline: Instant,
    ) -> Result<()> {
        self.install_beacon(progress, true, deadline)
    }

    pub(super) fn recover_beacon_provider(
        &mut self,
        index: usize,
        deadline: Instant,
    ) -> Result<HostReceiptV1> {
        let validator = self
            .admitted
            .inventory
            .validators
            .get(index)
            .ok_or_else(|| eyre!("unknown beacon provider recovery slot"))?
            .clone();
        // Read-only reconciliation can prove completion or authorize an exact
        // forward continuation; this request itself never publishes provider files.
        self.dispatch(
            &validator.slug,
            &validator.endpoint,
            &validator.service_root,
            HostAction::BeaconActivate,
            None,
            true,
            None,
            remaining_seconds(deadline)?.min(self.admitted.inventory.timeouts.restart_secs),
        )
    }
}

pub(in super::super) fn load_plan(
    validators: &[ValidatorV1],
    inputs_path: &Path,
    manifest_path: &Path,
    unit_paths: &[PathBuf],
    genesis_public_key: &PublicKey,
) -> Result<BeaconBootstrapPlanV1> {
    let (inputs, _) = read_public::<PreparedBeaconInputsV1>(inputs_path, "native beacon inputs")?;
    let (_, manifest) = read_public::<iroha_genesis::RawGenesisTransaction>(
        manifest_path,
        "native beacon genesis manifest",
    )?;
    if unit_paths.len() != 4 {
        return Err(eyre!("beacon assembly requires exactly four final units"));
    }
    let mut units = Vec::new();
    for (validator, path) in validators.iter().zip(unit_paths) {
        let (file, snapshot) = open_pinned_regular(path, "beacon final unit")?;
        #[cfg(unix)]
        if snapshot.uid != rustix::process::geteuid().as_raw() || snapshot.mode & 0o7777 != 0o644 {
            return Err(eyre!("beacon final unit must be owned and mode 0644"));
        }
        let bytes = read_pinned_bytes(path, "beacon final unit", file, &snapshot, 1024 * 1024)?;
        units.push(FinalUnitV1 {
            validator: validator.slug.clone(),
            sha256: sha256_hex(&bytes),
            bytes,
        });
    }
    Ok(BeaconBootstrapPlanV1 {
        schema: PLAN_SCHEMA.into(),
        request: inputs.request,
        genesis_manifest: manifest,
        genesis_public_key: genesis_public_key.clone(),
        final_units: units,
    })
}

pub(in super::super) fn derive_plan(
    inventory: &mut InventoryV1,
    inputs_path: &Path,
    manifest_path: &Path,
    unit_paths: &[PathBuf],
    genesis_public_key: &PublicKey,
) -> Result<()> {
    let (inputs, _) = read_public::<PreparedBeaconInputsV1>(inputs_path, "native beacon inputs")?;
    inventory.beacon_bootstrap = load_plan(
        &inventory.validators,
        inputs_path,
        manifest_path,
        unit_paths,
        genesis_public_key,
    )?;
    validate_plan(inventory)?;
    let source = artifact(&inventory.validators[0].artifacts, "genesis")?;
    let path = Path::new(&source.local_path);
    let (file, snapshot) = open_pinned_regular(path, "beacon signed genesis")?;
    let bytes = read_pinned_bytes(path, "beacon signed genesis", file, &snapshot, PUBLIC_LIMIT)?;
    if sha256_hex(&bytes) != source.sha256 {
        return Err(eyre!("beacon signed genesis changed during assembly"));
    }
    let genesis = plan_genesis(inventory, &bytes)?;
    let manifest = json::from_slice(&inventory.beacon_bootstrap.genesis_manifest)?;
    let expected = derive_public_beacon_inputs(
        &genesis,
        &manifest,
        &inventory.authorization_nonce,
        &inventory.validators,
        &inventory.validator_clients,
    )?;
    if json::to_value(&inputs)? != json::to_value(&expected)? {
        return Err(eyre!(
            "public beacon inputs differ from native nonce/genesis/seat derivation"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beacon_install_envelope_requires_ordinary_exact_certificate() {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::{
            Level,
            isi::{
                InstructionBox, Log,
                consensus_keys::{
                    ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
                },
            },
            transaction::{TransactionAdmissionIntent, TransactionBuilder},
        };
        use iroha_torii_shared::{FeeQuoteDecision, FeeQuoteObservation};
        let _profile = ChainDiscriminantGuard::enter(reset::CHAIN_DISCRIMINANT);
        let inventory = reset::sample_inventory_fixture();
        let key = KeyPair::try_from_seed(vec![0x42; 32], Algorithm::Ed25519).unwrap();
        let authority = AccountId::new(key.public_key().clone());
        assert_eq!(
            authority.to_string(),
            inventory.canary_onboarding_request.account_id
        );
        let network = inventory.beacon_bootstrap.request.dkg_session.network_id;
        // Native transcript/QC verification precedes this envelope boundary.
        // Here the exact typed instruction bytes are the already-verified input.
        let certificate = ThresholdKeyLifecycleCertificateV1 {
            version: 1,
            action: ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
            expected_active_session_id: None,
            effective_height: 7,
            network_id: network,
            roster_hash: inventory.beacon_bootstrap.request.dkg_session.roster_hash,
            committee_size: 4,
            quorum: 3,
            session_id: [42; 32],
            transcript_hash: [9; 32],
            public_state: vec![1, 2, 3],
            signatures: Vec::new(),
        };
        let instructions = vec![InstructionBox::from(
            ApplyThresholdKeyLifecycleCertificateV1 { certificate },
        )];
        let fees = inventory_fee_payment_intent(&inventory).unwrap();
        let quote = FeeQuoteResponse {
            intent: fees.clone(),
            observation: FeeQuoteObservation {
                ledger_time_ms: 1,
                next_block_height: 7,
                route_dataspace_id: iroha_model_base::topology::DataSpaceId::UNIVERSAL,
            },
            components: Vec::new(),
            capacities: Vec::new(),
            decision: FeeQuoteDecision::Accepted {
                debit_source: iroha_data_model::nexus::FeeDebitSource::Account(authority.clone()),
                program_revision: None,
            },
        };
        for case in 0..4 {
            let mut selected = instructions.clone();
            if case == 2 {
                selected.push(Log::new(Level::INFO, "foreign mixed instruction".to_owned()).into());
            } else if case == 3 {
                selected.clear();
                selected.push(Log::new(Level::INFO, "substituted instruction".to_owned()).into());
            }
            let transaction = TransactionBuilder::new(network, authority.clone(), fees.clone())
                .with_admission_intent(if case == 1 {
                    TransactionAdmissionIntent::QueuePlanSynced
                } else {
                    TransactionAdmissionIntent::Ordinary
                })
                .with_instructions(selected)
                .try_sign(key.private_key())
                .unwrap();
            let envelope = InstallEnvelopeV1 {
                schema: "iroha.taira.public-reset.beacon-install-envelope.v1".into(),
                authorization_sha256: "a".repeat(64),
                bundle_sha256: "b".repeat(64),
                transaction_wire_hex: hex::encode(transaction.encode_wire_v1().unwrap()),
                transaction_hash: hex::encode(transaction.hash().as_ref()),
                fee_quote: quote.clone(),
            };
            let result = envelope.verify_transaction(
                &inventory,
                &"a".repeat(64),
                &"b".repeat(64),
                &instructions,
            );
            if case == 0 {
                assert_eq!(result.unwrap(), transaction);
            } else {
                assert!(result.is_err(), "case {case}");
            }
        }
    }

    #[test]
    fn signed_beacon_plan_binds_roster_seats_and_exact_final_units() {
        let inventory = reset::sample_inventory_fixture();
        validate_plan(&inventory).unwrap();
        let wire = json::to_value(&inventory.beacon_bootstrap).unwrap();
        let decoded: BeaconBootstrapPlanV1 = json::from_value(wire.clone()).unwrap();
        assert_eq!(json::to_value(&decoded).unwrap(), wire);
        for case in 0..6 {
            let mut changed = inventory.clone();
            match case {
                0 => changed.beacon_bootstrap.final_units.swap(0, 1),
                1 => changed.beacon_bootstrap.final_units[0].bytes.push(b' '),
                2 => changed
                    .beacon_bootstrap
                    .request
                    .authorization_roster
                    .swap(0, 1),
                3 => {
                    changed.beacon_bootstrap.request.provider_handles[0] =
                        changed.beacon_bootstrap.request.provider_handles[1].clone()
                }
                4 => changed.beacon_bootstrap.request.provider_revision = 0,
                _ => changed.authorization_nonce = "differentauthorizationnonce12345678".into(),
            }
            assert!(validate_plan(&changed).is_err(), "case {case}");
        }
        let mut absent = json::to_value(&inventory).unwrap();
        absent.as_object_mut().unwrap().remove("beacon_bootstrap");
        assert!(json::from_value::<InventoryV1>(absent).is_err());
    }

    #[test]
    fn beacon_config_projection_changes_only_exact_provider_fields() {
        let inventory = reset::sample_inventory_fixture();
        let provider = ProviderV1 {
            signer_index: 1,
            validator: inventory.validator_clients[0].peer_id.parse().unwrap(),
            handle: "taira-beacon-seat-1".into(),
            revision: 3,
            policy_digest: [7; 32],
        };
        let initial = b"private_key = 'fixture-only'\n[genesis]\nexpected_hash = 'unchanged'\n[sumeragi]\nmode = 'Npos'\n[sorafs]\nprivate_key_file = '/must-not-be-read'\n";
        let first = derive_config(initial, &provider).unwrap();
        assert_eq!(*first, *derive_config(initial, &provider).unwrap());
        let mut after: toml::Table = toml::from_str(std::str::from_utf8(&first).unwrap()).unwrap();
        let initial: toml::Table = toml::from_str(std::str::from_utf8(initial).unwrap()).unwrap();
        let values = after.get_mut("sumeragi").unwrap().as_table_mut().unwrap();
        assert_eq!(
            values.remove(PROVIDER_FIELDS[0]),
            Some(toml::Value::String(provider.handle.clone()))
        );
        assert_eq!(
            values.remove(PROVIDER_FIELDS[1]),
            Some(toml::Value::Integer(3))
        );
        assert_eq!(
            values.remove(PROVIDER_FIELDS[2]),
            Some(toml::Value::String(hex::encode([7; 32])))
        );
        assert_eq!(after, initial);
        assert!(
            derive_config(&first, &provider).is_err(),
            "existing provider cannot be overwritten"
        );
        assert!(derive_config(b"extends = '/untrusted'\n[sumeragi]\n", &provider).is_err());
        assert!(derive_config(b"private_key = 'fixture-only'\n", &provider).is_err());
    }

    #[test]
    fn lost_beacon_ceremony_cannot_restart_or_repeat_committed_canaries() {
        let directory = reset::private_custody_test_dir("beacon-controller-");
        require_new_ceremony(directory.path(), 0).unwrap();
        for next in 1..=3 {
            assert!(require_new_ceremony(directory.path(), next).is_err());
        }
        reset::inputs::write_new_private(
            &directory.path().join("started.json"),
            b"retained-attempt",
        )
        .unwrap();
        assert!(require_new_ceremony(directory.path(), 0).is_err());
        assert_eq!(
            fs::read(directory.path().join("started.json")).unwrap(),
            b"retained-attempt"
        );
    }

    #[test]
    fn beacon_owned_child_deadline_retains_private_attempt() {
        let directory = reset::private_custody_test_dir("beacon-controller-");
        let marker = directory.path().join("started.json");
        reset::inputs::write_new_private(&marker, b"retained-attempt").unwrap();
        let mut child = CeremonyChild::spawn(
            Path::new("/bin/sh"),
            vec![
                "-c".into(),
                "while :; do sleep 1; done".into(),
                "beacon-test".into(),
            ],
            1,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        child.deadline = Instant::now();
        assert!(child.poll().is_err());
        drop(child); // Terminates and reaps only this owned process group.
        assert_eq!(fs::read(marker).unwrap(), b"retained-attempt");
    }
    #[test]
    fn beacon_successful_early_child_exit_cannot_authorize_another_operation() {
        let mut child = CeremonyChild::spawn(
            Path::new("/bin/sh"),
            vec!["-c".into(), "exit 0".into(), "beacon-test".into()],
            1,
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        assert!(child.child.wait().unwrap().success());
        assert!(child.require_running().is_err());
        assert!(!child.complete);
    }

    #[cfg(unix)]
    #[test]
    fn beacon_unit_publication_preserves_completed_inode_and_rejects_substitution() {
        let directory = reset::private_custody_test_dir("beacon-unit-");
        let source = directory.path().join("final-unit");
        let destination = directory.path().join("installed.service");
        let initial = b"[Service]\nExecStart=/initial\n";
        let final_bytes = b"[Service]\nExecStart=/final\n";
        reset::inputs::write_new_private(&source, final_bytes).unwrap();
        fs::write(&destination, initial).unwrap();
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o644)).unwrap();
        let mut desired = artifact(
            &reset::sample_inventory_fixture().validators[0].artifacts,
            "validator_unit",
        )
        .unwrap()
        .clone();
        desired.sha256 = sha256_hex(final_bytes);
        desired.size = final_bytes.len() as u64;
        publish_provider_unit(
            &source,
            &destination,
            &desired,
            &sha256_hex(initial),
            || Ok(()),
        )
        .unwrap();
        let before = fs::metadata(&destination).unwrap();
        publish_provider_unit(
            &source,
            &destination,
            &desired,
            &sha256_hex(initial),
            || Ok(()),
        )
        .unwrap();
        let after = fs::metadata(&destination).unwrap();
        assert_eq!(
            (
                before.dev(),
                before.ino(),
                before.mtime(),
                before.mtime_nsec()
            ),
            (after.dev(), after.ino(), after.mtime(), after.mtime_nsec())
        );
        fs::write(&destination, b"foreign unit").unwrap();
        assert!(
            publish_provider_unit(
                &source,
                &destination,
                &desired,
                &sha256_hex(initial),
                || Ok(())
            )
            .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"foreign unit");
        fs::write(&destination, initial).unwrap();
        assert!(
            publish_provider_unit(
                &source,
                &destination,
                &desired,
                &sha256_hex(initial),
                || {
                    fs::write(&destination, b"changed after admission")?;
                    Ok(())
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"changed after admission");
    }
}
