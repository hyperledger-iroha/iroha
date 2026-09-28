//! Real per-seat DKG exchange for disposable multi-validator networks.

use super::*;
use color_eyre::eyre::ensure;
use iroha_core::{
    beacon::{
        AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgSnapshotV1,
        GlobalThresholdBeaconDkgStateV1, global_threshold_beacon_roster_hash_v1,
    },
    validator_committee_evidence::{
        ValidatorCommitteeProvisioningEvidenceV1, ValidatorCommitteeSelectionEvidenceV1,
        verify_validator_committee_provisioning_evidence_v1,
    },
};
use iroha_data_model::{
    NetworkId,
    block::consensus_v2::HeightContextId,
    consensus::{GlobalThresholdBeaconDkgSessionV1, GlobalThresholdBeaconKeySessionV1},
    sumeragi_finality::{FinalityValidator, SumeragiFinalityProof, SumeragiFinalityVerifier},
};
use norito::derive::JsonSerialize;
use std::{
    os::{
        fd::AsRawFd as _,
        unix::{
            fs::{MetadataExt as _, OpenOptionsExt as _},
            process::CommandExt as _,
        },
    },
    time::Instant,
};
use tempfile::TempDir;

const KEY_FD: i32 = 198;
const PUBLIC_FD: i32 = 201;
const FINALITY_FD: i32 = 202;
const MAX_PUBLIC_FRAME_BYTES: usize = 32 * 1024 * 1024;
const MAX_FINALITY_FRAME_BYTES: usize = 4 * 1024 * 1024;
const PROCESS_TIMEOUT: Duration = Duration::from_secs(600);
const POLL_INTERVAL: Duration = Duration::from_millis(25);
const GLOBAL_BEACON_CREDENTIAL_FILE: &str = "iroha-global-beacon-partial-signer-v1.norito";

/// Independently anchored arguments to the native per-seat rotation command.
#[derive(Clone, Copy, Debug)]
pub struct DisposableRotationProofInput {
    /// Signed-genesis network identity.
    pub network_id: NetworkId,
    /// Trusted context identifier pinned before the committee status query.
    pub trusted_context_id: CryptoHash,
    /// Positive first height of the contiguous proof chain.
    pub anchor_height: u64,
    /// Exact scheduling epoch of the frozen target.
    pub target_epoch: u64,
    /// Exact frozen transition, independently selected by the operator.
    pub transition_id: CryptoHash,
}

/// Independently pinned inputs to one native pending-custody preparation.
#[derive(Clone, Debug)]
pub struct DisposablePendingCustodyInput {
    /// Signed-genesis network identity.
    pub network_id: NetworkId,
    /// Trusted context identifier pinned before the committee status query.
    pub trusted_context_id: CryptoHash,
    /// First height of the contiguous authenticated proof chain.
    pub anchor_height: u64,
    /// Exact target scheduling epoch.
    pub target_epoch: u64,
    /// Exact immutable frozen attempt.
    pub transition_id: CryptoHash,
    /// Actual validator that owns the private pending share.
    pub local_validator: PeerId,
    /// Signed chain identifier consumed by the stock provider catalog.
    pub chain_id: String,
    /// Exact production provider handle for this seat.
    pub handle: String,
    /// New catalog revision after retaining incumbent custody.
    pub revision: u64,
}

/// One current credential and its authenticated public catalog for an incumbent seat.
pub struct DisposableRetainedBeaconCredential<'a> {
    /// Owner-private native credential file; never passed on argv.
    pub credential_path: &'a Path,
    /// Canonical public catalog qualifying that credential.
    pub catalog: &'a [u8],
}

/// Native atomic current-plus-pending generation for one actual target process.
pub struct DisposablePreparedBeaconCustody {
    /// Exact validator that owns the prepared credential.
    pub validator: PeerId,
    /// Production credential frame retained in the owner-private generation.
    pub credential_path: PathBuf,
    /// Canonical public catalog for the prepared credential.
    pub catalog_path: PathBuf,
    /// Native signed-context preparation receipt.
    pub receipt_path: PathBuf,
    _owner_root: Arc<TempDir>,
}

/// One real target seat's private output, retained only in its owner-private root.
pub struct DisposableRotationSeatOutput {
    /// Frozen validator identity served by this DKG process.
    pub validator: PeerId,
    /// One-based index in the exact frozen roster.
    pub signer_index: u16,
    /// Exact credential-free provider identity pinned by the native seat request.
    pub provider_handle: String,
    /// Exact public catalog revision pinned by the native seat request.
    pub provider_revision: u64,
    /// Exact canonical single-session credential for this seat alone.
    pub credential_path: PathBuf,
    /// Exact private share for production current-plus-pending custody preparation.
    pub pending_share_path: PathBuf,
    /// Non-secret public provider identity and qualification digest.
    pub provider_path: PathBuf,
    genesis_config_source: bool,
    _owner_root: Arc<TempDir>,
}

/// One signed-genesis voter and its owner-private native validator config.
///
/// The native seat child consumes a separate pinned copy through FD198. Its
/// private BLS key is never placed in a command argument or environment value.
#[derive(Clone, Debug)]
pub struct DisposableGenesisConfigSeat {
    /// Exact voter in the signed genesis roster.
    pub validator: PeerId,
    /// Direct owner-private generated validator config with this voter's key.
    pub config_path: PathBuf,
}

/// Public finalized DKG result and separate private output for each real seat.
pub struct DisposableRotationDkgOutput {
    /// All-edge finalized public transcript.
    pub public_session: GlobalThresholdBeaconKeySessionV1,
    /// One output per exact frozen roster seat, in roster order.
    pub seats: Vec<DisposableRotationSeatOutput>,
    /// Native operator bundle containing the independently verified public record.
    pub public_bundle_path: PathBuf,
    /// Native exact-incumbent-quorum finalization instructions, encoded as Norito JSON.
    pub finalization_instruction_path: PathBuf,
    _controller_root: Arc<TempDir>,
}

/// Four independently provisioned genesis shares and their quorum-signed install instruction.
pub struct DisposableGenesisDkgOutput {
    /// Finalized public session assembled from every signed edge.
    pub public_session: GlobalThresholdBeaconKeySessionV1,
    /// One private output for each signed-genesis voter, in exact roster order.
    pub seats: Vec<DisposableRotationSeatOutput>,
    /// Native operator bundle containing the independently verified public record.
    pub public_bundle_path: PathBuf,
    /// Native quorum-assembled installation instructions, encoded as Norito JSON.
    pub install_instruction_path: PathBuf,
    _controller_root: Arc<TempDir>,
}

#[derive(JsonSerialize)]
struct GenesisRequest {
    schema: String,
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    target_roster: Vec<PeerId>,
    authorization_roster: Vec<PeerId>,
    provider_handles: Vec<String>,
    provider_revision: u64,
}

struct SeatProcess {
    validator: PeerId,
    signer_index: u16,
    child: Child,
    public_writer: fs::File,
    finality_writer: fs::File,
    attempt_path: PathBuf,
    owner_root: Arc<TempDir>,
    genesis_config_source: bool,
}

fn attempt_child_name(session: &GlobalThresholdBeaconDkgSessionV1, signer_index: u16) -> String {
    use fmt::Write as _;
    let mut name = String::from("attempt-");
    for byte in session.attempt_id {
        write!(name, "{byte:02x}").expect("writing into a String cannot fail");
    }
    write!(name, "-seat-{signer_index}").expect("writing into a String cannot fail");
    name
}

fn provider_handle(peer: &PeerId) -> String {
    use fmt::Write as _;
    let identity: [u8; 32] = CryptoHash::new_from_chunks(&[
        b"sumeragi-v2:disposable-beacon-provider:v1",
        &peer.encode(),
    ])
    .into();
    let mut handle = String::from("software://iroha/global-beacon/disposable-validator-");
    for byte in identity {
        write!(handle, "{byte:02x}").expect("writing into a String cannot fail");
    }
    handle
}

fn verify_input(
    _seats: &[&NetworkPeer],
    _authorizing_seats: &[&NetworkPeer],
    _evidence: &ValidatorCommitteeSelectionEvidenceV1,
    _input: DisposableRotationProofInput,
) -> Result<(GlobalThresholdBeaconDkgSessionV1, SumeragiFinalityVerifier)> {
    Err(eyre!(
        "rotation requires current authenticated committee state evidence, which is unavailable"
    ))
}

fn write_frame(writer: &mut fs::File, bytes: &[u8], limit: usize) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= limit,
        "rotation frame has invalid size"
    );
    writer.write_all(&u32::try_from(bytes.len())?.to_be_bytes())?;
    writer.write_all(bytes)?;
    writer.flush()?;
    Ok(())
}

fn broadcast_public<T: norito::NoritoSerialize>(
    seats: &mut [SeatProcess],
    value: &T,
) -> Result<()> {
    let bytes = norito::encode_canonical(value)?;
    for seat in seats {
        write_frame(&mut seat.public_writer, &bytes, MAX_PUBLIC_FRAME_BYTES)?;
    }
    Ok(())
}

fn broadcast_finality(seats: &mut [SeatProcess], proof: &SumeragiFinalityProof) -> Result<()> {
    let bytes = norito::encode_canonical(proof)?;
    for seat in seats {
        write_frame(&mut seat.finality_writer, &bytes, MAX_FINALITY_FRAME_BYTES)?;
    }
    Ok(())
}

fn read_public_snapshot(
    path: &Path,
    maximum: usize,
) -> Result<Option<GlobalThresholdBeaconDkgSnapshotV1>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        bytes.len() <= maximum,
        "rotation public artifact exceeds its bound"
    );
    match norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
    {
        Ok(snapshot) => Ok(Some(snapshot)),
        // The daemon creates an output file before finishing its write; a
        // partial frame is not an observation and must be read again.
        Err(_) => Ok(None),
    }
}

async fn wait_for_snapshots(
    seats: &mut [SeatProcess],
    name: &str,
    deadline: Instant,
) -> Result<Vec<GlobalThresholdBeaconDkgSnapshotV1>> {
    let mut observed = (0..seats.len()).map(|_| None).collect::<Vec<_>>();
    loop {
        ensure!(Instant::now() < deadline, "rotation {name} phase timed out");
        for (index, seat) in seats.iter_mut().enumerate() {
            if observed[index].is_some() {
                continue;
            }
            if let Some(snapshot) =
                read_public_snapshot(&seat.attempt_path.join(name), MAX_PUBLIC_FRAME_BYTES)?
            {
                observed[index] = Some(snapshot);
            } else if let Some(status) = seat.child.try_wait()? {
                return Err(eyre!(
                    "rotation seat {} exited before {name}: {status}; private diagnostics: {}",
                    seat.signer_index,
                    seat.owner_root.path().display()
                ));
            }
        }
        if observed.iter().all(Option::is_some) {
            return observed
                .into_iter()
                .map(|snapshot| snapshot.ok_or_else(|| eyre!("missing rotation snapshot")))
                .collect();
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn merge_publications(
    session: GlobalThresholdBeaconDkgSessionV1,
    snapshots: &[GlobalThresholdBeaconDkgSnapshotV1],
    crypto: &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
) -> Result<GlobalThresholdBeaconDkgStateV1> {
    let mut state = GlobalThresholdBeaconDkgStateV1::new(session, crypto)?;
    for (index, snapshot) in snapshots.iter().enumerate() {
        let seat_index = u16::try_from(index + 1)?;
        ensure!(
            snapshot.session == session
                && snapshot.last_updated_height == session.start_height
                && snapshot.recipient_keys.len() == 1
                && snapshot.dealer_commitments.len() == 1
                && snapshot.recipient_keys[0].recipient_index == seat_index
                && snapshot.dealer_commitments[0].dealer_index == seat_index
                && snapshot.encrypted_shares.is_empty()
                && snapshot.share_acceptances.is_empty(),
            "rotation publication is not one exact signed target seat"
        );
        let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(snapshot.clone(), crypto)?;
        state.record_recipient_key(session.start_height, snapshot.recipient_keys[0].clone())?;
    }
    for snapshot in snapshots {
        state.record_dealer_commitment(
            session.start_height,
            snapshot.dealer_commitments[0].clone(),
            crypto,
        )?;
    }
    Ok(state)
}

fn merge_deliveries(
    state: &mut GlobalThresholdBeaconDkgStateV1,
    snapshots: &[GlobalThresholdBeaconDkgSnapshotV1],
    crypto: &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
) -> Result<()> {
    let previous = state.public_snapshot()?;
    let session = previous.session;
    for (index, snapshot) in snapshots.iter().enumerate() {
        ensure!(
            snapshot.session == session
                && snapshot.last_updated_height == session.commitments_end_height
                && snapshot.recipient_keys == previous.recipient_keys
                && snapshot.dealer_commitments == previous.dealer_commitments
                && snapshot.share_acceptances.is_empty()
                && snapshot.encrypted_shares.len() == usize::from(session.committee_size)
                && snapshot
                    .encrypted_shares
                    .iter()
                    .all(|edge| edge.dealer_index == u16::try_from(index + 1).unwrap_or(0)),
            "rotation delivery is not one exact dealer's full private-edge set"
        );
        let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(snapshot.clone(), crypto)?;
        for edge in &snapshot.encrypted_shares {
            state.record_encrypted_share(session.commitments_end_height, edge.clone())?;
        }
    }
    Ok(())
}

fn merge_acceptances(
    state: &mut GlobalThresholdBeaconDkgStateV1,
    snapshots: &[GlobalThresholdBeaconDkgSnapshotV1],
    crypto: &AdaptiveGlobalThresholdBeaconDkgCryptoV1,
) -> Result<()> {
    let previous = state.public_snapshot()?;
    let session = previous.session;
    for (index, snapshot) in snapshots.iter().enumerate() {
        ensure!(
            snapshot.session == session
                && snapshot.last_updated_height == session.deliveries_end_height
                && snapshot.recipient_keys == previous.recipient_keys
                && snapshot.dealer_commitments == previous.dealer_commitments
                && snapshot.encrypted_shares == previous.encrypted_shares
                && snapshot.share_acceptances.len() == usize::from(session.committee_size)
                && snapshot
                    .share_acceptances
                    .iter()
                    .all(|acceptance| acceptance.recipient_index
                        == u16::try_from(index + 1).unwrap_or(0)),
            "rotation acceptance is not one exact recipient's full edge set"
        );
        let _ = GlobalThresholdBeaconDkgStateV1::from_snapshot(snapshot.clone(), crypto)?;
        for acceptance in &snapshot.share_acceptances {
            state.record_share_acceptance(session.deliveries_end_height, acceptance.clone())?;
        }
    }
    Ok(())
}

fn write_owner_private_key(path: &Path, signer: &KeyPair) -> Result<fs::File> {
    ensure!(
        signer.public_key().algorithm() == Algorithm::BlsNormal,
        "rotation seat does not own a real BLS identity"
    );
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    writeln!(file, "{}", ExposedPrivateKey(signer.private_key().clone()))?;
    file.sync_all()?;
    ensure!(
        file.metadata()?.len() == 71,
        "native BLS private record is not canonical"
    );
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}

#[allow(
    unsafe_code,
    reason = "the disposable supervisor transfers three already opened owner-private descriptors to fixed native child FDs"
)]
fn inherit_rotation_descriptors(
    command: &mut tokio::process::Command,
    key: i32,
    public: i32,
    finality: i32,
) {
    unsafe {
        command.as_std_mut().pre_exec(move || {
            for (source, target) in [(key, KEY_FD), (public, PUBLIC_FD), (finality, FINALITY_FD)] {
                if nix::libc::dup2(source, target) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

fn spawn_seat(
    binary: &Path,
    seat: &NetworkPeer,
    signer_index: u16,
    session: &GlobalThresholdBeaconDkgSessionV1,
    evidence_path: &Path,
    input: DisposableRotationProofInput,
    provider_revision: u64,
) -> Result<SeatProcess> {
    let owner_root =
        super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let key = write_owner_private_key(
        &owner_root.path().join("identity.private"),
        seat.bls_key_pair()
            .ok_or_else(|| eyre!("target seat has no BLS identity"))?,
    )?;
    let public_path = owner_root.path().join("public.fifo");
    let finality_path = owner_root.path().join("finality.fifo");
    nix::unistd::mkfifo(
        &public_path,
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )?;
    nix::unistd::mkfifo(
        &finality_path,
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )?;
    let public_fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&public_path)?;
    let finality_fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&finality_path)?;
    let private_log = |name| -> Result<fs::File> {
        Ok(fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(owner_root.path().join(name))?)
    };
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("beacon-bootstrap")
        .arg("provision-rotation-seat")
        .arg("--selection-evidence")
        .arg(evidence_path)
        .arg("--network-id")
        .arg(input.network_id.to_string())
        .arg("--trusted-context-id")
        .arg(input.trusted_context_id.to_string())
        .arg("--anchor-height")
        .arg(input.anchor_height.to_string())
        .arg("--target-epoch")
        .arg(input.target_epoch.to_string())
        .arg("--transition-id")
        .arg(input.transition_id.to_string())
        .arg("--signer-index")
        .arg(signer_index.to_string())
        .arg("--key-fd")
        .arg(KEY_FD.to_string())
        .arg("--public-fd")
        .arg(PUBLIC_FD.to_string())
        .arg("--finality-fd")
        .arg(FINALITY_FD.to_string())
        .arg("--provider-handle")
        .arg(provider_handle(&seat.id()))
        .arg("--provider-revision")
        .arg(provider_revision.to_string())
        .arg("--attempt-root")
        .arg(owner_root.path())
        .arg("--timeout-ms")
        .arg(PROCESS_TIMEOUT.as_millis().to_string())
        .env_clear()
        .current_dir(owner_root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(private_log("stdout.log")?))
        .stderr(Stdio::from(private_log("stderr.log")?))
        .kill_on_drop(true);
    inherit_rotation_descriptors(
        &mut command,
        key.as_raw_fd(),
        public_fifo.as_raw_fd(),
        finality_fifo.as_raw_fd(),
    );
    let child = command.spawn()?;
    drop(key);
    drop(public_fifo);
    drop(finality_fifo);
    let public_writer = fs::OpenOptions::new().write(true).open(&public_path)?;
    let finality_writer = fs::OpenOptions::new().write(true).open(&finality_path)?;
    Ok(SeatProcess {
        validator: seat.id(),
        signer_index,
        child,
        public_writer,
        finality_writer,
        attempt_path: owner_root
            .path()
            .join(attempt_child_name(session, signer_index)),
        owner_root,
        genesis_config_source: false,
    })
}

fn verify_genesis_input(
    network: &Network,
    first_finality: &SumeragiFinalityProof,
) -> Result<(
    NativeGenesisProvisioningBundle,
    GlobalThresholdBeaconDkgSessionV1,
    Vec<PeerId>,
    SumeragiFinalityVerifier,
)> {
    ensure!(
        network.validators().len() == 4,
        "genesis DKG requires four exact voting seats"
    );
    let bundle = network.native_genesis_provisioning_bundle()?;
    let network_id = network.network_id();
    ensure!(
        network_id.into_genesis_hash() == bundle.block_hash
            && first_finality.block_header.hash() == bundle.block_hash
            && first_finality.block_header.height().get() == 1
            && first_finality.height() == 1,
        "genesis DKG anchor is not the exact retained signed genesis"
    );
    let genesis = network.genesis();
    let roster = iroha_core::sumeragi::startup::genesis_committee_peers(&genesis.0)?;
    let available = network
        .validators()
        .iter()
        .map(NetworkPeer::id)
        .collect::<BTreeSet<_>>();
    ensure!(
        roster.iter().cloned().collect::<BTreeSet<_>>() == available
            && available.len() == roster.len(),
        "disposable genesis DKG lacks an exact real process for each signed voter"
    );
    let validators = iroha_core::sumeragi::schedule::genesis_validators(&genesis)?
        .into_iter()
        .map(|(peer, proof_of_possession)| FinalityValidator {
            public_key: peer.public_key().clone(),
            proof_of_possession,
        })
        .collect();
    let mut verifier =
        SumeragiFinalityVerifier::new(&genesis.0, &network.chain_id().to_string(), validators)?;
    verifier.verify(first_finality)?;
    let session = genesis_dkg_session(network_id, &roster);
    let _ =
        GlobalThresholdBeaconDkgStateV1::new(session, &AdaptiveGlobalThresholdBeaconDkgCryptoV1)?;
    Ok((bundle, session, roster, verifier))
}

fn genesis_dkg_session(
    network_id: NetworkId,
    roster: &[PeerId],
) -> GlobalThresholdBeaconDkgSessionV1 {
    GlobalThresholdBeaconDkgSessionV1 {
        version: 1,
        network_id,
        session_id: CryptoHash::new_from_chunks(&[
            b"iroha.global-beacon.genesis-session.v1\0",
            network_id.as_bytes(),
        ])
        .into(),
        attempt_id: CryptoHash::new_from_chunks(&[
            b"iroha.global-beacon.genesis-attempt.v1\0",
            network_id.as_bytes(),
        ])
        .into(),
        authority_generation: 0,
        roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
        committee_size: 4,
        threshold: 2,
        start_height: 1,
        commitments_end_height: 2,
        deliveries_end_height: 3,
        acceptances_end_height: 4,
    }
}

fn spawn_genesis_seat(
    binary: &Path,
    seat: &NetworkPeer,
    signer_index: u16,
    session: &GlobalThresholdBeaconDkgSessionV1,
    public_paths: &[PathBuf; 5],
    chain_discriminant: u16,
) -> Result<SeatProcess> {
    let owner_root =
        super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let retained_key = write_owner_private_key(
        &owner_root.path().join("identity.private"),
        seat.bls_key_pair()
            .ok_or_else(|| eyre!("genesis voter has no real BLS identity"))?,
    )?;
    let provision_key = copy_owner_private_genesis_signer_input(
        &owner_root.path().join("identity.private"),
        &owner_root.path().join("provision.fd198"),
    )?;
    drop(retained_key);
    let public_path = owner_root.path().join("public.fifo");
    let finality_path = owner_root.path().join("finality.fifo");
    nix::unistd::mkfifo(
        &public_path,
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )?;
    nix::unistd::mkfifo(
        &finality_path,
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )?;
    let public_fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&public_path)?;
    let finality_fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&finality_path)?;
    let private_log = |name| -> Result<fs::File> {
        Ok(fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(owner_root.path().join(name))?)
    };
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("beacon-bootstrap")
        .arg("provision-genesis-seat")
        .arg("--network-id")
        .arg(session.network_id.to_string())
        .arg("--chain-discriminant")
        .arg(chain_discriminant.to_string())
        .arg("--request")
        .arg(&public_paths[0])
        .arg("--genesis-manifest")
        .arg(&public_paths[1])
        .arg("--genesis-signed")
        .arg(&public_paths[2])
        .arg("--genesis-public-key")
        .arg(&public_paths[3])
        .arg("--genesis-finality")
        .arg(&public_paths[4])
        .arg("--signer-index")
        .arg(signer_index.to_string())
        .arg("--key-fd")
        .arg(KEY_FD.to_string())
        .arg("--public-fd")
        .arg(PUBLIC_FD.to_string())
        .arg("--finality-fd")
        .arg(FINALITY_FD.to_string())
        .arg("--attempt-root")
        .arg(owner_root.path())
        .arg("--timeout-ms")
        .arg(PROCESS_TIMEOUT.as_millis().to_string())
        .env_clear()
        .current_dir(owner_root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(private_log("stdout.log")?))
        .stderr(Stdio::from(private_log("stderr.log")?))
        .kill_on_drop(true);
    inherit_rotation_descriptors(
        &mut command,
        provision_key.as_raw_fd(),
        public_fifo.as_raw_fd(),
        finality_fifo.as_raw_fd(),
    );
    let child = command.spawn().map_err(|error| {
        let _ = retire_one_shot_genesis_descriptor(&owner_root.path().join("provision.fd198"));
        error
    })?;
    drop(provision_key);
    drop(public_fifo);
    drop(finality_fifo);
    Ok(SeatProcess {
        validator: seat.id(),
        signer_index,
        child,
        public_writer: fs::OpenOptions::new().write(true).open(&public_path)?,
        finality_writer: fs::OpenOptions::new().write(true).open(&finality_path)?,
        attempt_path: owner_root
            .path()
            .join(attempt_child_name(session, signer_index)),
        owner_root,
        genesis_config_source: false,
    })
}

fn copy_owner_private_genesis_signer_input(source: &Path, target: &Path) -> Result<fs::File> {
    let before = fs::symlink_metadata(source)?;
    ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.uid() == nix::unistd::Uid::effective().as_raw()
            && before.mode() & 0o7777 == 0o600
            && before.nlink() == 1
            && (1..=1024 * 1024).contains(&before.len()),
        "genesis signer input is not a direct owner-private file"
    );
    let mut input = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(source)?;
    let opened = input.metadata()?;
    ensure!(
        (opened.dev(), opened.ino(), opened.len()) == (before.dev(), before.ino(), before.len()),
        "genesis signer input changed before descriptor handoff"
    );
    let mut output = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    ensure!(
        std::io::copy(&mut input, &mut output)? == before.len(),
        "genesis signer input changed during descriptor handoff"
    );
    output.sync_all()?;
    let after = input.metadata()?;
    ensure!(
        (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec()
        ) == (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec()
        ),
        "genesis signer input changed after descriptor handoff"
    );
    output.rewind()?;
    Ok(output)
}

fn retire_one_shot_genesis_descriptor(path: &Path) -> Result<bool> {
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.uid() == nix::unistd::Uid::effective().as_raw()
            && before.mode() & 0o7777 == 0o600
            && before.nlink() == 1
            && before.len() <= 1024 * 1024,
        "genesis one-shot descriptor lost private-file custody"
    );
    let consumed = before.len() == 0;
    if !consumed {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path)?;
        let opened = file.metadata()?;
        ensure!(
            (opened.dev(), opened.ino(), opened.len())
                == (before.dev(), before.ino(), before.len()),
            "genesis one-shot descriptor changed before retirement"
        );
        file.write_all(&vec![0_u8; usize::try_from(before.len())?])?;
        file.sync_data()?;
        file.set_len(0)?;
        file.sync_data()?;
    }
    fs::remove_file(path)?;
    Ok(consumed)
}

fn spawn_genesis_config_seat(
    binary: &Path,
    seat: &DisposableGenesisConfigSeat,
    signer_index: u16,
    session: &GlobalThresholdBeaconDkgSessionV1,
    public_paths: &[PathBuf; 5],
    chain_discriminant: u16,
) -> Result<SeatProcess> {
    let owner_root =
        super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let retained_config = copy_owner_private_genesis_signer_input(
        &seat.config_path,
        &owner_root.path().join("identity.private"),
    )?;
    let provision_config = copy_owner_private_genesis_signer_input(
        &owner_root.path().join("identity.private"),
        &owner_root.path().join("provision.fd198"),
    )?;
    drop(retained_config);
    let public_path = owner_root.path().join("public.fifo");
    let finality_path = owner_root.path().join("finality.fifo");
    nix::unistd::mkfifo(
        &public_path,
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )?;
    nix::unistd::mkfifo(
        &finality_path,
        nix::sys::stat::Mode::from_bits_truncate(0o600),
    )?;
    let public_fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&public_path)?;
    let finality_fifo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&finality_path)?;
    let private_log = |name| -> Result<fs::File> {
        Ok(fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(owner_root.path().join(name))?)
    };
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("beacon-bootstrap")
        .arg("provision-genesis-seat")
        .arg("--network-id")
        .arg(session.network_id.to_string())
        .arg("--chain-discriminant")
        .arg(chain_discriminant.to_string())
        .arg("--request")
        .arg(&public_paths[0])
        .arg("--genesis-manifest")
        .arg(&public_paths[1])
        .arg("--genesis-signed")
        .arg(&public_paths[2])
        .arg("--genesis-public-key")
        .arg(&public_paths[3])
        .arg("--genesis-finality")
        .arg(&public_paths[4])
        .arg("--signer-index")
        .arg(signer_index.to_string())
        .arg("--config-fd")
        .arg(KEY_FD.to_string())
        .arg("--public-fd")
        .arg(PUBLIC_FD.to_string())
        .arg("--finality-fd")
        .arg(FINALITY_FD.to_string())
        .arg("--attempt-root")
        .arg(owner_root.path())
        .arg("--timeout-ms")
        .arg(PROCESS_TIMEOUT.as_millis().to_string())
        .env_clear()
        .current_dir(owner_root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(private_log("stdout.log")?))
        .stderr(Stdio::from(private_log("stderr.log")?))
        .kill_on_drop(true);
    inherit_rotation_descriptors(
        &mut command,
        provision_config.as_raw_fd(),
        public_fifo.as_raw_fd(),
        finality_fifo.as_raw_fd(),
    );
    let child = command.spawn().map_err(|error| {
        let _ = retire_one_shot_genesis_descriptor(&owner_root.path().join("provision.fd198"));
        error
    })?;
    drop(provision_config);
    drop(public_fifo);
    drop(finality_fifo);
    Ok(SeatProcess {
        validator: seat.validator.clone(),
        signer_index,
        child,
        public_writer: fs::OpenOptions::new().write(true).open(&public_path)?,
        finality_writer: fs::OpenOptions::new().write(true).open(&finality_path)?,
        attempt_path: owner_root
            .path()
            .join(attempt_child_name(session, signer_index)),
        owner_root,
        genesis_config_source: true,
    })
}

fn genesis_public_args(paths: &[PathBuf; 5], network: NetworkId, discriminant: u16) -> Vec<String> {
    // The fixed public input order is shared with the native genesis parser.
    [
        "--network-id".to_owned(),
        network.to_string(),
        "--chain-discriminant".to_owned(),
        discriminant.to_string(),
        "--request".to_owned(),
        paths[0].display().to_string(),
        "--genesis-manifest".to_owned(),
        paths[1].display().to_string(),
        "--genesis-signed".to_owned(),
        paths[2].display().to_string(),
        "--genesis-public-key".to_owned(),
        paths[3].display().to_string(),
        "--genesis-finality".to_owned(),
        paths[4].display().to_string(),
    ]
    .into()
}

async fn run_genesis_public_command(binary: &Path, arguments: &[String]) -> Result<()> {
    let status = timeout(
        PROCESS_TIMEOUT,
        tokio::process::Command::new(binary)
            .args(arguments)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .status(),
    )
    .await??;
    ensure!(
        status.success(),
        "native genesis public command failed: {status}"
    );
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "the disposable supervisor passes one already opened owner-private BLS descriptor to the native signer"
)]
async fn sign_genesis_draft(
    binary: &Path,
    network: NetworkId,
    discriminant: u16,
    public_bundle: &Path,
    seat: &DisposableRotationSeatOutput,
    output: &Path,
) -> Result<()> {
    let one_shot_path = seat._owner_root.path().join("sign.fd198");
    let key = copy_owner_private_genesis_signer_input(
        &seat._owner_root.path().join("identity.private"),
        &one_shot_path,
    )?;
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("beacon-bootstrap")
        .arg("sign-genesis-install")
        .arg("--network-id")
        .arg(network.to_string())
        .arg("--chain-discriminant")
        .arg(discriminant.to_string())
        .arg("--bundle")
        .arg(public_bundle)
        .arg("--signer-index")
        .arg((seat.signer_index - 1).to_string())
        .arg(if seat.genesis_config_source {
            "--config-fd"
        } else {
            "--key-fd"
        })
        .arg(KEY_FD.to_string())
        .arg("--output")
        .arg(output)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    unsafe {
        command.as_std_mut().pre_exec(move || {
            if nix::libc::dup2(key.as_raw_fd(), KEY_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let waited = timeout(PROCESS_TIMEOUT, command.status()).await;
    let consumed = retire_one_shot_genesis_descriptor(&one_shot_path)?;
    let status = waited??;
    ensure!(
        status.success() && consumed,
        "native genesis seat signature failed: {status}"
    );
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "the disposable supervisor passes one already opened owner-private BLS descriptor to the native signer"
)]
async fn sign_rotation_draft(
    binary: &Path,
    seat: &NetworkPeer,
    public_bundle: &Path,
    proof_args: &[String],
    signer_index: usize,
    output: &Path,
) -> Result<()> {
    let root = super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let key = write_owner_private_key(
        &root.path().join("identity.private"),
        seat.bls_key_pair()
            .ok_or_else(|| eyre!("incumbent signer has no real BLS identity"))?,
    )?;
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("beacon-bootstrap")
        .arg("sign-rotation")
        .args(proof_args)
        .arg("--bundle")
        .arg(public_bundle)
        .arg("--signer-index")
        .arg(signer_index.to_string())
        .arg("--key-fd")
        .arg(KEY_FD.to_string())
        .arg("--output")
        .arg(output)
        .env_clear()
        .current_dir(root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    unsafe {
        command.as_std_mut().pre_exec(move || {
            if nix::libc::dup2(key.as_raw_fd(), KEY_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let status = timeout(PROCESS_TIMEOUT, command.status()).await??;
    ensure!(
        status.success(),
        "native incumbent signature failed: {status}"
    );
    Ok(())
}

/// Verify exact incumbent authorization, then import one seat's pending share
/// with the stock native current-plus-pending custody command.
///
/// The private share and optional incumbent credential travel only as inherited
/// owner-private descriptors. The native command validates and atomically
/// publishes the resulting generation; this helper has no alternate decoder.
///
/// # Errors
///
/// Rejects stale or foreign evidence, a different target seat, bad descriptor
/// custody, or any native generation-publishing failure.
#[allow(
    unsafe_code,
    reason = "the disposable supervisor transfers already opened owner-private custody files to fixed native child FDs"
)]
pub async fn prepare_disposable_pending_custody(
    evidence: &ValidatorCommitteeProvisioningEvidenceV1,
    pending_share_path: &Path,
    retained: Option<DisposableRetainedBeaconCredential<'_>>,
    input: DisposablePendingCustodyInput,
) -> Result<DisposablePreparedBeaconCustody> {
    ensure!(
        input.revision != 0,
        "pending custody revision must be positive"
    );
    iroha_config::parameters::validate_production_runtime_handle(&input.handle)
        .map_err(|error| eyre!("invalid production beacon provider handle: {error:?}"))?;
    let trusted = HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
        input.trusted_context_id,
    ));
    let verified = verify_validator_committee_provisioning_evidence_v1(
        evidence,
        input.network_id,
        trusted,
        input.anchor_height,
        input.target_epoch,
        input.transition_id.into(),
    )
    .map_err(|error| eyre!("pending custody evidence is invalid: {error}"))?;
    ensure!(
        verified
            .transition()
            .preparation
            .roster
            .iter()
            .any(|seat| seat.validator == input.local_validator),
        "pending custody owner is not one exact target seat"
    );
    let binary = Program::IrohadTaira.resolve_async().await?;
    let root = super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let evidence_path = root.path().join("custody-evidence.norito");
    fs::write(&evidence_path, norito::encode_canonical(evidence)?)?;
    let share = fs::File::open(pending_share_path)?;
    let current = retained
        .as_ref()
        .map(|retained| fs::File::open(retained.credential_path))
        .transpose()?;
    let catalog_path = retained
        .as_ref()
        .map(|retained| {
            let path = root.path().join("current-catalog.norito");
            fs::write(&path, retained.catalog)?;
            Ok::<_, Report>(path)
        })
        .transpose()?;
    let output = root.path().join("prepared-generation");
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("beacon-prepare-custody")
        .arg("--evidence")
        .arg(&evidence_path)
        .arg("--network-id")
        .arg(input.network_id.to_string())
        .arg("--trusted-context-id")
        .arg(input.trusted_context_id.to_string())
        .arg("--anchor-height")
        .arg(input.anchor_height.to_string())
        .arg("--target-epoch")
        .arg(input.target_epoch.to_string())
        .arg("--transition-id")
        .arg(input.transition_id.to_string())
        .arg("--local-validator")
        .arg(input.local_validator.to_string())
        .arg("--chain-id")
        .arg(&input.chain_id)
        .arg("--handle")
        .arg(&input.handle)
        .arg("--revision")
        .arg(input.revision.to_string())
        .arg("--output")
        .arg(&output)
        .env_clear()
        .current_dir(root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(path) = &catalog_path {
        command.arg("--current-catalog").arg(path);
    }
    let retained_fd = current.as_ref().map(std::os::fd::AsRawFd::as_raw_fd);
    unsafe {
        command.as_std_mut().pre_exec(move || {
            if nix::libc::dup2(share.as_raw_fd(), KEY_FD) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if let Some(source) = retained_fd {
                if nix::libc::dup2(source, 200) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let status = timeout(PROCESS_TIMEOUT, command.status()).await??;
    ensure!(
        status.success(),
        "native pending custody preparation failed: {status}"
    );
    Ok(DisposablePreparedBeaconCustody {
        validator: input.local_validator,
        credential_path: output.join(GLOBAL_BEACON_CREDENTIAL_FILE),
        catalog_path: output.join("runtime-provider-catalog.norito"),
        receipt_path: output.join("pending-beacon-custody.json"),
        _owner_root: root,
    })
}

/// Run four real signed-genesis dealers/recipients and assemble their install certificate.
///
/// The caller supplies the independently fetched h1 anchor and a live finality
/// source. This function requests h2, h3, and h4 only after each preceding
/// public phase is ready. Every seat owns its private DKG share and signing
/// descriptor; only signed public artifacts cross the coordinator.
///
/// # Errors
///
/// Rejects a mismatched signed genesis, non-contiguous finality, incomplete
/// private edge acceptance, failed native process, or invalid quorum assembly.
pub async fn run_disposable_genesis_dkg<F, Fut>(
    network: &Network,
    first_finality: &SumeragiFinalityProof,
    certificate_height: u64,
    next_finality: F,
) -> Result<DisposableGenesisDkgOutput>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<SumeragiFinalityProof>>,
{
    let (bundle, session, roster, verifier) = verify_genesis_input(network, first_finality)?;
    let binary = Program::IrohadTaira.resolve_async().await?;
    let ordered_seats = roster
        .iter()
        .map(|peer| {
            network
                .validators()
                .iter()
                .find(|seat| seat.id() == *peer)
                .ok_or_else(|| eyre!("signed genesis voter has no disposable process"))
        })
        .collect::<Result<Vec<_>>>()?;
    run_genesis_dkg_with_seats(
        bundle,
        session,
        roster,
        verifier,
        first_finality,
        certificate_height,
        next_finality,
        &binary,
        |binary, _validator, index, session, paths, discriminant| {
            spawn_genesis_seat(
                binary,
                ordered_seats[usize::from(index - 1)],
                index,
                session,
                paths,
                discriminant,
            )
        },
    )
    .await
}

/// Run the same real per-seat genesis DKG using independently generated native configs.
///
/// This accepts the exact signed manifest and four direct owner-private
/// validator configs from an external disposable localnet generator. The
/// signed voter order, h1 authority and every phase finality proof are
/// revalidated before any credential is returned. No signer key is read into
/// an argument or environment value.
///
/// # Errors
///
/// Rejects an altered signed genesis, reordered or foreign voter config,
/// unsafe private config file, incomplete public DKG, or missed phase cutoff.
pub async fn run_disposable_genesis_dkg_from_configs<F, Fut>(
    bundle: NativeGenesisProvisioningBundle,
    network_id: NetworkId,
    seats: &[DisposableGenesisConfigSeat],
    native_binary: &Path,
    first_finality: &SumeragiFinalityProof,
    certificate_height: u64,
    next_finality: F,
) -> Result<DisposableGenesisDkgOutput>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<SumeragiFinalityProof>>,
{
    ensure!(
        seats.len() == 4,
        "genesis DKG requires four exact voter configs"
    );
    ensure!(
        sha256(&bundle.manifest_json) == bundle.manifest_sha256
            && network_id.into_genesis_hash() == bundle.block_hash
            && first_finality.block_header.hash() == bundle.block_hash
            && first_finality.block_header.height().get() == 1
            && first_finality.height() == 1,
        "external genesis DKG anchor differs from retained signed genesis"
    );
    let manifest: RawGenesisTransaction = norito::json::from_slice(&bundle.manifest_json)?;
    let validated = iroha_genesis::validate_prepared_genesis_bundle(
        &bundle.signed_wire,
        &manifest,
        &bundle.public_key,
        bundle.block_hash,
    )?;
    let genesis = GenesisBlock(validated.block().clone());
    let roster = iroha_core::sumeragi::startup::genesis_committee_peers(&genesis.0)?;
    ensure!(
        roster.len() == 4 && seats.iter().map(|seat| &seat.validator).eq(roster.iter()),
        "native config seats differ from exact signed-genesis voter order"
    );
    let validators = validated
        .validator_pops()
        .iter()
        .map(|(public_key, proof_of_possession)| FinalityValidator {
            public_key: public_key.clone(),
            proof_of_possession: proof_of_possession.clone(),
        })
        .collect();
    let mut verifier = SumeragiFinalityVerifier::new(
        validated.block(),
        &manifest.chain_id().to_string(),
        validators,
    )?;
    verifier.verify(first_finality)?;
    let session = genesis_dkg_session(network_id, &roster);
    let _ =
        GlobalThresholdBeaconDkgStateV1::new(session, &AdaptiveGlobalThresholdBeaconDkgCryptoV1)?;
    run_genesis_dkg_with_seats(
        bundle,
        session,
        roster,
        verifier,
        first_finality,
        certificate_height,
        next_finality,
        native_binary,
        |binary, _validator, index, session, paths, discriminant| {
            spawn_genesis_config_seat(
                binary,
                &seats[usize::from(index - 1)],
                index,
                session,
                paths,
                discriminant,
            )
        },
    )
    .await
}

async fn run_genesis_dkg_with_seats<F, Fut, S>(
    bundle: NativeGenesisProvisioningBundle,
    session: GlobalThresholdBeaconDkgSessionV1,
    roster: Vec<PeerId>,
    mut verifier: SumeragiFinalityVerifier,
    first_finality: &SumeragiFinalityProof,
    certificate_height: u64,
    mut next_finality: F,
    binary: &Path,
    mut spawn_seat: S,
) -> Result<DisposableGenesisDkgOutput>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<SumeragiFinalityProof>>,
    S: FnMut(
        &Path,
        &PeerId,
        u16,
        &GlobalThresholdBeaconDkgSessionV1,
        &[PathBuf; 5],
        u16,
    ) -> Result<SeatProcess>,
{
    ensure!(
        certificate_height > 4,
        "genesis install cannot precede DKG finality"
    );
    let mut proofs = vec![first_finality.clone()];
    let controller =
        super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let controller_path = controller.path();
    let paths = [
        controller_path.join("request.json"),
        controller_path.join("genesis-manifest.json"),
        controller_path.join("genesis.signed.nrt"),
        controller_path.join("genesis.public-key"),
        controller_path.join("genesis-finality.norito"),
    ];
    let request = GenesisRequest {
        schema: "iroha.global-beacon.bootstrap.request.v1".to_owned(),
        dkg_session: session,
        target_roster: roster.clone(),
        authorization_roster: roster.clone(),
        provider_handles: roster.iter().map(provider_handle).collect(),
        provider_revision: 1,
    };
    fs::write(&paths[0], norito::json::to_vec(&request)?)?;
    fs::write(&paths[1], &bundle.manifest_json)?;
    ensure!(
        sha256(fs::read(&paths[1])?) == bundle.manifest_sha256,
        "staged raw manifest differs from the signed-genesis builder's exact bytes"
    );
    fs::write(&paths[2], &bundle.signed_wire)?;
    fs::write(&paths[3], format!("{}\n", bundle.public_key))?;
    fs::write(&paths[4], norito::encode_canonical(first_finality)?)?;
    let mut processes = roster
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            spawn_seat(
                binary,
                validator,
                u16::try_from(index + 1)?,
                &session,
                &paths,
                bundle.chain_discriminant,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;

    let publications = wait_for_snapshots(&mut processes, "publication.norito", deadline).await?;
    let mut public = merge_publications(session, &publications, &crypto)?;
    broadcast_public(&mut processes, &public.public_snapshot()?)?;
    let proof = next_finality(2).await?;
    ensure!(
        proof.height() == 2 && proof.block_header.height().get() == 2,
        "genesis commitments were not followed by exact h2 finality"
    );
    verifier.verify(&proof)?;
    broadcast_finality(&mut processes, &proof)?;
    proofs.push(proof);

    let deliveries = wait_for_snapshots(&mut processes, "deliveries.norito", deadline).await?;
    merge_deliveries(&mut public, &deliveries, &crypto)?;
    broadcast_public(&mut processes, &public.public_snapshot()?)?;
    let proof = next_finality(3).await?;
    ensure!(
        proof.height() == 3 && proof.block_header.height().get() == 3,
        "genesis deliveries were not followed by exact h3 finality"
    );
    verifier.verify(&proof)?;
    broadcast_finality(&mut processes, &proof)?;
    proofs.push(proof);

    let acceptances = wait_for_snapshots(&mut processes, "acceptances.norito", deadline).await?;
    merge_acceptances(&mut public, &acceptances, &crypto)?;
    let assembled = public
        .finalize(session.acceptances_end_height, &crypto)?
        .clone();
    broadcast_public(&mut processes, &assembled)?;
    let proof = next_finality(4).await?;
    ensure!(
        proof.height() == 4 && proof.block_header.height().get() == 4,
        "genesis acceptances were not followed by exact h4 finality"
    );
    verifier.verify(&proof)?;
    broadcast_finality(&mut processes, &proof)?;
    proofs.push(proof);

    let mut outputs = Vec::with_capacity(processes.len());
    for mut process in processes {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let waited = timeout(remaining, process.child.wait()).await;
        if !matches!(&waited, Ok(Ok(_))) {
            let _ = process.child.kill().await;
            let _ = process.child.wait().await;
        }
        let consumed =
            retire_one_shot_genesis_descriptor(&process.owner_root.path().join("provision.fd198"))?;
        let status = waited??;
        ensure!(
            status.success() && consumed,
            "genesis seat {} failed: {status}; private diagnostics: {}",
            process.signer_index,
            process.owner_root.path().display()
        );
        let bytes = fs::read(process.attempt_path.join("public-session.norito"))?;
        let observed: GlobalThresholdBeaconKeySessionV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )?;
        ensure!(
            observed == assembled,
            "genesis seat finalized another public transcript"
        );
        outputs.push(DisposableRotationSeatOutput {
            validator: process.validator.clone(),
            signer_index: process.signer_index,
            provider_handle: provider_handle(&process.validator),
            provider_revision: 1,
            credential_path: process.attempt_path.join(GLOBAL_BEACON_CREDENTIAL_FILE),
            pending_share_path: process.attempt_path.join("pending-share.bin"),
            provider_path: process.attempt_path.join("provider.json"),
            genesis_config_source: process.genesis_config_source,
            _owner_root: process.owner_root,
        });
    }
    let public_session_path = controller_path.join("public-session.norito");
    fs::write(&public_session_path, norito::encode_canonical(&assembled)?)?;
    let phase_paths = (1..=3)
        .map(|index| {
            let path = controller_path.join(format!("phase-{}.norito", index + 1));
            fs::write(&path, norito::encode_canonical(&proofs[index])?)?;
            Ok(path)
        })
        .collect::<Result<Vec<_>>>()?;
    let public_bundle_path = controller_path.join("genesis-public-bundle.json");
    let mut assemble_args = vec![
        "beacon-bootstrap".to_owned(),
        "assemble-genesis-dkg".to_owned(),
    ];
    assemble_args.extend(genesis_public_args(
        &paths,
        session.network_id,
        bundle.chain_discriminant,
    ));
    for phase in &phase_paths {
        assemble_args.push("--phase-proof".to_owned());
        assemble_args.push(phase.display().to_string());
    }
    assemble_args.extend([
        "--public-session".to_owned(),
        public_session_path.display().to_string(),
    ]);
    for seat in &outputs {
        assemble_args.push("--provider".to_owned());
        assemble_args.push(seat.provider_path.display().to_string());
    }
    assemble_args.extend([
        "--certificate-height".to_owned(),
        certificate_height.to_string(),
        "--output".to_owned(),
        public_bundle_path.display().to_string(),
    ]);
    run_genesis_public_command(&binary, &assemble_args).await?;

    let mut signatures = Vec::with_capacity(3);
    for seat in outputs.iter().take(3) {
        let signature =
            controller_path.join(format!("install-signature-{}.json", seat.signer_index));
        sign_genesis_draft(
            &binary,
            session.network_id,
            bundle.chain_discriminant,
            &public_bundle_path,
            seat,
            &signature,
        )
        .await?;
        signatures.push(signature);
    }
    let install_instruction_path = controller_path.join("install-instruction.json");
    let mut install_args = vec![
        "beacon-bootstrap".to_owned(),
        "assemble-genesis-install".to_owned(),
        "--network-id".to_owned(),
        session.network_id.to_string(),
        "--chain-discriminant".to_owned(),
        bundle.chain_discriminant.to_string(),
        "--bundle".to_owned(),
        public_bundle_path.display().to_string(),
    ];
    for signature in &signatures {
        install_args.push("--signature".to_owned());
        install_args.push(signature.display().to_string());
    }
    install_args.extend([
        "--output".to_owned(),
        install_instruction_path.display().to_string(),
    ]);
    run_genesis_public_command(&binary, &install_args).await?;
    Ok(DisposableGenesisDkgOutput {
        public_session: assembled,
        seats: outputs,
        public_bundle_path,
        install_instruction_path,
        _controller_root: controller,
    })
}

/// Exchange one exact frozen committee's real dealer and recipient messages.
///
/// Each child receives only its own BLS key through FD198. The coordinator
/// combines signed public frames and independently verified finality proofs;
/// it never reads, derives, or collects any seat's private DKG share. The
/// returned credential and pending-share paths remain separate per seat so
/// production custody preparation can retain current and pending keys.
///
/// # Errors
///
/// Rejects a non-exact roster, forged or discontinuous finality, incomplete
/// public edges, failed native child, or any missed preparation deadline.
pub async fn run_disposable_rotation_dkg<F, Fut>(
    seats: &[&NetworkPeer],
    authorizing_seats: &[&NetworkPeer],
    evidence: &ValidatorCommitteeSelectionEvidenceV1,
    input: DisposableRotationProofInput,
    provider_revision: u64,
    certificate_height: u64,
    mut next_finality: F,
) -> Result<DisposableRotationDkgOutput>
where
    F: FnMut(u64) -> Fut,
    Fut: Future<Output = Result<SumeragiFinalityProof>>,
{
    ensure!(provider_revision != 0, "provider revision must be positive");
    let (session, mut verifier) = verify_input(seats, authorizing_seats, evidence, input)?;
    ensure!(
        certificate_height > session.acceptances_end_height
            && certificate_height
                < evidence
                    .status
                    .selected
                    .as_ref()
                    .ok_or_else(|| eyre!("missing selected attempt"))?
                    .transition
                    .preparation
                    .first_height
                    - 1,
        "rotation certificate misses its exact preparation window"
    );
    let binary = Program::IrohadTaira.resolve_async().await?;
    let controller =
        super::disposable_runtime_provider_broker::new_disposable_owner_private_root()?;
    let evidence_path = controller.path().join("selection-evidence.norito");
    fs::write(&evidence_path, norito::encode_canonical(evidence)?)?;
    let mut processes = seats
        .iter()
        .enumerate()
        .map(|(index, seat)| {
            spawn_seat(
                &binary,
                seat,
                u16::try_from(index + 1)?,
                &session,
                &evidence_path,
                input,
                provider_revision,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let mut phase_proofs = Vec::with_capacity(3);

    let publications = wait_for_snapshots(&mut processes, "publication.norito", deadline).await?;
    let mut public = merge_publications(session, &publications, &crypto)?;
    broadcast_public(&mut processes, &public.public_snapshot()?)?;
    let proof = next_finality(session.commitments_end_height).await?;
    ensure!(
        proof.height() == session.commitments_end_height
            && proof.block_header.height().get() == session.commitments_end_height,
        "rotation commitments were not followed by exact phase finality"
    );
    verifier.verify(&proof)?;
    broadcast_finality(&mut processes, &proof)?;
    phase_proofs.push(proof);

    let deliveries = wait_for_snapshots(&mut processes, "deliveries.norito", deadline).await?;
    merge_deliveries(&mut public, &deliveries, &crypto)?;
    broadcast_public(&mut processes, &public.public_snapshot()?)?;
    let proof = next_finality(session.deliveries_end_height).await?;
    ensure!(
        proof.height() == session.deliveries_end_height
            && proof.block_header.height().get() == session.deliveries_end_height,
        "rotation deliveries were not followed by exact phase finality"
    );
    verifier.verify(&proof)?;
    broadcast_finality(&mut processes, &proof)?;
    phase_proofs.push(proof);

    let acceptances = wait_for_snapshots(&mut processes, "acceptances.norito", deadline).await?;
    merge_acceptances(&mut public, &acceptances, &crypto)?;
    let assembled = public
        .finalize(session.acceptances_end_height, &crypto)?
        .clone();
    broadcast_public(&mut processes, &assembled)?;
    let proof = next_finality(session.acceptances_end_height).await?;
    ensure!(
        proof.height() == session.acceptances_end_height
            && proof.block_header.height().get() == session.acceptances_end_height,
        "rotation acceptances were not followed by exact phase finality"
    );
    verifier.verify(&proof)?;
    broadcast_finality(&mut processes, &proof)?;
    phase_proofs.push(proof);

    let mut outputs = Vec::with_capacity(processes.len());
    for mut process in processes {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let status = timeout(remaining, process.child.wait()).await??;
        ensure!(
            status.success(),
            "rotation seat {} failed: {status}; private diagnostics: {}",
            process.signer_index,
            process.owner_root.path().display()
        );
        let public_bytes = fs::read(process.attempt_path.join("public-session.norito"))?;
        let seat_session: GlobalThresholdBeaconKeySessionV1 = norito::decode_canonical_with_limits(
            &public_bytes,
            norito::canonical_decode_limits(public_bytes.len()),
        )?;
        ensure!(
            seat_session == assembled,
            "seat finalized another public transcript"
        );
        outputs.push(DisposableRotationSeatOutput {
            validator: process.validator.clone(),
            signer_index: process.signer_index,
            provider_handle: provider_handle(&process.validator),
            provider_revision,
            credential_path: process.attempt_path.join(GLOBAL_BEACON_CREDENTIAL_FILE),
            pending_share_path: process.attempt_path.join("pending-share.bin"),
            provider_path: process.attempt_path.join("provider.json"),
            genesis_config_source: process.genesis_config_source,
            _owner_root: process.owner_root,
        });
    }
    let public_session_path = controller.path().join("public-session.norito");
    fs::write(&public_session_path, norito::encode_canonical(&assembled)?)?;
    let phase_paths = phase_proofs
        .iter()
        .map(|proof| {
            let path = controller
                .path()
                .join(format!("phase-{}.norito", proof.height()));
            fs::write(&path, norito::encode_canonical(proof)?)?;
            Ok(path)
        })
        .collect::<Result<Vec<_>>>()?;
    let public_bundle_path = controller.path().join("rotation-public-bundle.json");
    let mut proof_args = vec![
        "--selection-evidence".to_owned(),
        evidence_path.display().to_string(),
        "--network-id".to_owned(),
        input.network_id.to_string(),
        "--trusted-context-id".to_owned(),
        input.trusted_context_id.to_string(),
        "--anchor-height".to_owned(),
        input.anchor_height.to_string(),
        "--target-epoch".to_owned(),
        input.target_epoch.to_string(),
        "--transition-id".to_owned(),
        input.transition_id.to_string(),
    ];
    let mut assemble_args = vec![
        "beacon-bootstrap".to_owned(),
        "assemble-rotation-dkg".to_owned(),
    ];
    assemble_args.extend(proof_args.iter().cloned());
    assemble_args.extend([
        "--public-session".to_owned(),
        public_session_path.display().to_string(),
    ]);
    for phase in &phase_paths {
        assemble_args.extend(["--phase-proof".to_owned(), phase.display().to_string()]);
    }
    for seat in &outputs {
        assemble_args.extend([
            "--provider".to_owned(),
            seat.provider_path.display().to_string(),
        ]);
    }
    assemble_args.extend([
        "--certificate-height".to_owned(),
        certificate_height.to_string(),
        "--output".to_owned(),
        public_bundle_path.display().to_string(),
    ]);
    run_genesis_public_command(&binary, &assemble_args).await?;

    let quorum = 2 * ((authorizing_seats.len() - 1) / 3) + 1;
    let mut signatures = Vec::with_capacity(quorum);
    for (index, seat) in authorizing_seats.iter().take(quorum).enumerate() {
        let signature = controller
            .path()
            .join(format!("rotation-signature-{index}.json"));
        sign_rotation_draft(
            &binary,
            seat,
            &public_bundle_path,
            &proof_args,
            index,
            &signature,
        )
        .await?;
        signatures.push(signature);
    }
    let finalization_instruction_path = controller.path().join("rotation-finalization.json");
    let mut finalize_args = vec![
        "beacon-bootstrap".to_owned(),
        "assemble-rotation".to_owned(),
    ];
    finalize_args.append(&mut proof_args);
    finalize_args.extend([
        "--bundle".to_owned(),
        public_bundle_path.display().to_string(),
    ]);
    for signature in &signatures {
        finalize_args.extend(["--signature".to_owned(), signature.display().to_string()]);
    }
    finalize_args.extend([
        "--output".to_owned(),
        finalization_instruction_path.display().to_string(),
    ]);
    run_genesis_public_command(&binary, &finalize_args).await?;
    Ok(DisposableRotationDkgOutput {
        public_session: assembled,
        seats: outputs,
        public_bundle_path,
        finalization_instruction_path,
        _controller_root: controller,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attempt_roots_and_provider_handles_are_distinct_and_canonical() {
        let first = PeerId::new(
            KeyPair::try_from_seed(b"seat-a".to_vec(), Algorithm::BlsNormal)
                .unwrap()
                .public_key()
                .clone(),
        );
        let second = PeerId::new(
            KeyPair::try_from_seed(b"seat-b".to_vec(), Algorithm::BlsNormal)
                .unwrap()
                .public_key()
                .clone(),
        );
        assert_ne!(provider_handle(&first), provider_handle(&second));
        for peer in [&first, &second] {
            iroha_config::parameters::validate_production_runtime_handle(&provider_handle(peer))
                .unwrap();
        }
    }

    #[test]
    fn public_frame_rejects_empty_and_oversized_bytes() {
        let root =
            super::super::disposable_runtime_provider_broker::new_disposable_owner_private_root()
                .unwrap();
        let path = root.path().join("frame");
        let mut writer = fs::File::create(path).unwrap();
        assert!(write_frame(&mut writer, b"", 4).is_err());
        assert!(write_frame(&mut writer, b"oversized", 4).is_err());
    }

    #[test]
    fn native_genesis_config_descriptor_requires_direct_owner_private_file() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("validator.toml");
        let copied = root.path().join("fd198.toml");
        fs::write(&source, b"[common]\npeer = 'signed-seat'\n").unwrap();
        fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
        let mut descriptor = copy_owner_private_genesis_signer_input(&source, &copied).unwrap();
        let mut observed = Vec::new();
        descriptor.read_to_end(&mut observed).unwrap();
        assert_eq!(observed, fs::read(&source).unwrap());
        assert_eq!(descriptor.metadata().unwrap().mode() & 0o7777, 0o600);
        assert!(!retire_one_shot_genesis_descriptor(&copied).unwrap());
        assert!(!copied.exists());
        let one_shot = root.path().join("consumed.fd198");
        let consumed = copy_owner_private_genesis_signer_input(&source, &one_shot).unwrap();
        consumed.set_len(0).unwrap();
        assert!(retire_one_shot_genesis_descriptor(&one_shot).unwrap());
        assert!(!one_shot.exists());
        assert!(
            source.exists(),
            "retained signer source must survive one-shot children"
        );
        fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            copy_owner_private_genesis_signer_input(&source, &root.path().join("unsafe")).is_err()
        );
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        assert!(
            copy_owner_private_genesis_signer_input(&alias, &root.path().join("link")).is_err()
        );
    }
}
