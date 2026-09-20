//! Native centralized fresh-DKG bootstrap under one deployment custody owner.
//!
//! Dealer secrets remain process-local while the controller supplies authenticated
//! committed heights. A failed ceremony is not resumable. Only public transcript
//! material and final per-seat supervisor credentials are exported; no dealer
//! polynomial or plaintext dealer-to-recipient exchange is persisted.

use crate::external_software_signer::{
    GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1, RuntimeGlobalBeaconShareProvisioningV1,
    encode_global_beacon_partial_signer_credential_v1,
    global_beacon_partial_signer_inventory_digest_v1,
    global_beacon_partial_signer_public_inventory_digest_v1,
};
use clap::{Parser, Subcommand};
use iroha_core::beacon::{
    AdaptiveGlobalThresholdBeaconDkgCryptoV1, FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    GlobalThresholdBeaconDkgPhaseV1, GlobalThresholdBeaconDkgStateV1,
    global_threshold_beacon_roster_hash_v1,
};
use iroha_core::state::{
    THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
    threshold_key_lifecycle_certificate_preimage_v1, verify_threshold_key_lifecycle_certificate_v1,
};
use iroha_crypto::{
    Algorithm, ExposedPrivateKey, KeyPair, PublicKey, Signature,
    threshold_bls::{
        AdaptiveThresholdBlsParameters, AdaptiveThresholdBlsPublicTranscript,
        AdaptiveThresholdBlsSecretShare, BeaconPurpose, DasRenDealerSecret, ThresholdBlsSession,
        ValidatedDealerCommitment,
    },
};
use iroha_data_model::{
    consensus::{
        GlobalThresholdBeaconDkgConstantProofV1, GlobalThresholdBeaconDkgDealerCommitmentV1,
        GlobalThresholdBeaconDkgSessionV1,
    },
    isi::{
        InstructionBox,
        consensus_keys::{
            ApplyThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleActionV1,
            ThresholdKeyLifecycleCertificateV1, ThresholdKeyLifecycleSignatureV1,
        },
    },
};
use iroha_model_base::peer::PeerId;
use norito::derive::{JsonDeserialize, JsonSerialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::{self, File},
    io::{Read as _, Write as _},
    os::{fd::BorrowedFd, unix::fs::MetadataExt as _},
    path::{Component, Path, PathBuf},
    str::FromStr as _,
    time::{Duration, Instant},
};
use zeroize::{Zeroize as _, Zeroizing};

const MAX_PUBLIC_BYTES: usize = 32 * 1024 * 1024;
const MAX_TIMEOUT_MS: u64 = 3_600_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Error {
    InvalidInput,
    InvalidCustody,
    Crypto,
    Height,
    Deadline,
    Io,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidInput => "beacon bootstrap public input is invalid",
            Self::InvalidCustody => "beacon bootstrap custody path is invalid",
            Self::Crypto => "beacon bootstrap cryptographic validation failed",
            Self::Height => "beacon bootstrap committed-height observation is invalid or closed",
            Self::Deadline => "beacon bootstrap operation deadline elapsed",
            Self::Io => "beacon bootstrap bounded I/O failed",
        })
    }
}
type Result<T> = std::result::Result<T, Error>;

#[derive(Parser)]
#[command(
    name = "iroha3d_taira beacon-bootstrap",
    about = "Prepare a fresh global beacon under centralized deployment custody; never submits a transaction"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Keep fresh dealer secrets in memory across actual committed DKG heights.
    Provision {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        genesis_manifest: PathBuf,
        #[arg(long)]
        genesis_signed: PathBuf,
        #[arg(long)]
        genesis_public_key: PathBuf,
        #[arg(long)]
        observed_height: u64,
        #[arg(long)]
        height_fd: i32,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 180_000)]
        timeout_ms: u64,
    },
    /// Sign the exact installation draft with one authorization-roster runtime key.
    SignInstall {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        signer_index: u16,
        /// Canonical BLS key record on the consumed supervisor descriptor 198.
        #[arg(
            long,
            conflicts_with = "config_fd",
            required_unless_present = "config_fd"
        )]
        key_fd: Option<i32>,
        /// Native validator configuration on the consumed supervisor descriptor 198.
        #[arg(long, conflicts_with = "key_fd", required_unless_present = "key_fd")]
        config_fd: Option<i32>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify exact-roster lifecycle signatures and emit the actual native instruction.
    AssembleInstall {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, required = true, num_args = 1..)]
        signature: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Request {
    schema: String,
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    target_roster: Vec<PeerId>,
    authorization_roster: Vec<PeerId>,
    provider_handles: Vec<String>,
    provider_revision: u64,
}
#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Provider {
    signer_index: u16,
    validator: PeerId,
    handle: String,
    revision: u64,
    policy_digest: [u8; 32],
}
#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GenesisProof {
    manifest: iroha_genesis::RawGenesisTransaction,
    signed_wire: Vec<u8>,
    public_key: PublicKey,
}
#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PublicBundle {
    schema: String,
    genesis: GenesisProof,
    request: Request,
    finalized_observed_height: u64,
    record: FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    certificate: ThresholdKeyLifecycleCertificateV1,
    providers: Vec<Provider>,
}

/// Dispatch only the explicit offline bootstrap subcommand, before runtime-key loading.
pub(crate) fn dispatch_if_requested() -> bool {
    let mut args = std::env::args_os();
    let _ = args.next();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("beacon-bootstrap")) {
        return false;
    }
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(
        crate::taira_runtime_signer::TAIRA_CHAIN_DISCRIMINANT_V1,
    );
    let parsed = Args::parse_from(std::iter::once(OsString::from("beacon-bootstrap")).chain(args));
    let result = match parsed.command {
        Command::Provision {
            request,
            genesis_manifest,
            genesis_signed,
            genesis_public_key,
            observed_height,
            height_fd,
            output,
            timeout_ms,
        } => provision_command(
            &request,
            &genesis_manifest,
            &genesis_signed,
            &genesis_public_key,
            observed_height,
            height_fd,
            &output,
            timeout_ms,
        ),
        Command::SignInstall {
            bundle,
            signer_index,
            key_fd,
            config_fd,
            output,
        } => sign_command(&bundle, signer_index, key_fd, config_fd, &output),
        Command::AssembleInstall {
            bundle,
            signature,
            output,
        } => assemble_command(&bundle, &signature, &output),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
    true
}

fn require_budget(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::Deadline)
    } else {
        Ok(())
    }
}
fn validate_roster(roster: &[PeerId]) -> Result<()> {
    // This owner is the current four-validator centralized deployment ceremony.
    if roster.len() != 4 || roster.iter().collect::<BTreeSet<_>>().len() != 4 {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn validate_request(
    request: &Request,
    observed_height: u64,
) -> Result<GlobalThresholdBeaconDkgStateV1> {
    validate_roster(&request.target_roster)?;
    validate_roster(&request.authorization_roster)?;
    let session = &request.dkg_session;
    if request.schema != "iroha.global-beacon.bootstrap.request.v1"
        || session.committee_size != 4
        || session.threshold != 2
        || session.roster_hash != global_threshold_beacon_roster_hash_v1(&request.target_roster)
        || request.provider_handles.len() != 4
        || request.provider_revision == 0
        || request
            .provider_handles
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != 4
        || request
            .provider_handles
            .iter()
            .any(|h| iroha_config::parameters::validate_production_runtime_handle(h).is_err())
    {
        return Err(Error::InvalidInput);
    }
    let state =
        GlobalThresholdBeaconDkgStateV1::new(*session, &AdaptiveGlobalThresholdBeaconDkgCryptoV1)
            .map_err(|_| Error::Crypto)?;
    if observed_height == 0
        || state.phase_at(observed_height) != GlobalThresholdBeaconDkgPhaseV1::Sharing
    {
        return Err(Error::Height);
    }
    Ok(state)
}
fn parameters(
    session: &GlobalThresholdBeaconDkgSessionV1,
) -> Result<AdaptiveThresholdBlsParameters<BeaconPurpose>> {
    let typed = ThresholdBlsSession::<BeaconPurpose>::new(
        *session.network_id.as_bytes(),
        session.session_id,
        session.roster_hash,
        session.committee_size,
        session.threshold,
    )
    .map_err(|_| Error::Crypto)?;
    AdaptiveThresholdBlsParameters::derive(&typed).map_err(|_| Error::Crypto)
}
fn dealer_dto(
    dealer: &ValidatedDealerCommitment<BeaconPurpose>,
) -> GlobalThresholdBeaconDkgDealerCommitmentV1 {
    GlobalThresholdBeaconDkgDealerCommitmentV1 {
        dealer_index: dealer.dealer_index(),
        coefficient_commitments: dealer
            .coefficients()
            .iter()
            .map(|c| *c.as_bytes())
            .collect(),
        constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
            commitment: *dealer.constant_proof().commitment_bytes(),
            response: *dealer.constant_proof().response_bytes(),
        },
    }
}

// Fresh private values never implement serialization, Debug or Clone. The callback
// runs only after all public commitments are validated and before finalization.
fn ceremony(
    request: Request,
    genesis: GenesisProof,
    observed_height: u64,
    deadline: Instant,
    mut progress: impl FnMut(&iroha_core::beacon::GlobalThresholdBeaconDkgSnapshotV1) -> Result<()>,
    mut next_height: impl FnMut(Instant) -> Result<u64>,
) -> Result<(PublicBundle, Vec<Zeroizing<Vec<u8>>>)> {
    require_budget(deadline)?;
    let mut state = validate_request(&request, observed_height)?;
    validate_genesis(&request, &genesis)?;
    let parameters = parameters(&request.dkg_session)?;
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let mut dealers = Vec::with_capacity(4);
    let mut commitments = Vec::with_capacity(4);
    for index in 1..=4 {
        require_budget(deadline)?;
        let (secret, commitment) =
            DasRenDealerSecret::generate(&parameters, index).map_err(|_| Error::Crypto)?;
        state
            .record_dealer_commitment(observed_height, dealer_dto(&commitment), &crypto)
            .map_err(|_| Error::Crypto)?;
        dealers.push(secret);
        commitments.push(commitment);
    }
    // Verify every private recipient contribution inside the sharing phase.
    // No complaint is suppressed: any invalid contribution aborts the ceremony.
    let mut recipient_shares = Vec::with_capacity(4);
    for index in 1_u16..=4 {
        require_budget(deadline)?;
        recipient_shares.push(
            dealers
                .iter()
                .zip(&commitments)
                .map(|(secret, commitment)| secret.private_share(&parameters, commitment, index))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| Error::Crypto)?,
        );
    }
    drop(dealers); // All polynomial secrets erase before waiting for phase heights.
    require_budget(deadline)?;
    progress(&state.public_snapshot().map_err(|_| Error::Crypto)?)?;
    let mut height = observed_height;
    while height < request.dkg_session.responses_end_height {
        let next = next_height(deadline)?;
        require_budget(deadline)?;
        if next <= height {
            return Err(Error::Height);
        }
        height = next;
    }
    let record = state
        .finalize(height, &crypto)
        .map_err(|_| Error::Crypto)?
        .clone();
    require_budget(deadline)?;
    let transcript = AdaptiveThresholdBlsPublicTranscript::from_qualified_dealers(
        &parameters,
        &commitments,
        &record.adaptive_dkg.qualified_dealers,
        record.adaptive_dkg.event_hash,
    )
    .map_err(|_| Error::Crypto)?;
    if transcript.transcript_hash() != &record.transcript_hash {
        return Err(Error::Crypto);
    }
    let mut credentials = Vec::with_capacity(4);
    let mut providers = Vec::with_capacity(4);
    for (offset, shares) in recipient_shares.into_iter().enumerate() {
        let index = u16::try_from(offset + 1).map_err(|_| Error::InvalidInput)?;
        require_budget(deadline)?;
        let aggregate = AdaptiveThresholdBlsSecretShare::from_dealer_shares(&transcript, &shares)
            .map_err(|_| Error::Crypto)?;
        drop(shares);
        let inventory = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            record.clone(),
            index,
            aggregate.into_components_for_runtime_custody(),
        )];
        let policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(record.network_id, &inventory)
                .map_err(|_| Error::Crypto)?;
        let handle = request.provider_handles[usize::from(index - 1)].clone();
        credentials.push(
            encode_global_beacon_partial_signer_credential_v1(
                record.network_id,
                &handle,
                request.provider_revision,
                policy_digest,
                inventory,
            )
            .map_err(|_| Error::Crypto)?,
        );
        providers.push(Provider {
            signer_index: index,
            validator: request.target_roster[usize::from(index - 1)].clone(),
            handle,
            revision: request.provider_revision,
            policy_digest,
        });
    }
    let record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(record).map_err(|_| Error::Crypto)?;
    let certificate = draft_certificate(&request, &record, height)?;
    let bundle = PublicBundle {
        schema: "iroha.global-beacon.bootstrap.bundle.v1".into(),
        genesis,
        request,
        finalized_observed_height: height,
        record,
        certificate,
        providers,
    };
    validate_bundle(&bundle)?;
    require_budget(deadline)?;
    Ok((bundle, credentials))
}

fn validate_genesis(request: &Request, proof: &GenesisProof) -> Result<()> {
    if proof.manifest.consensus_mode()
        != iroha_data_model::parameter::system::SumeragiConsensusMode::Npos
        || proof.manifest.chain_id().as_ref() != crate::taira_runtime_signer::TAIRA_CHAIN_ID_V1
        || proof.manifest.chain_discriminant()
            != crate::taira_runtime_signer::TAIRA_CHAIN_DISCRIMINANT_V1
    {
        return Err(Error::InvalidInput);
    }
    iroha_genesis::init_instruction_registry();
    let expected_hash = request.dkg_session.network_id.into_genesis_hash();
    let validated = iroha_genesis::validate_prepared_genesis_bundle(
        &proof.signed_wire,
        &proof.manifest,
        &proof.public_key,
        expected_hash,
    )
    .map_err(|_| Error::Crypto)?;
    // Use the exact native height-context owner. Topology insertion order and
    // caller ordering cannot select authorization indices.
    let ordered = iroha_core::sumeragi::signed_genesis_voting_peers(&iroha_genesis::GenesisBlock(
        validated.block().clone(),
    ))
    .map_err(|_| Error::Crypto)?;
    let required_pulse = first_required_pulse_height(proof)?;
    if ordered != request.target_roster
        || ordered != request.authorization_roster
        || request
            .dkg_session
            .responses_end_height
            .checked_add(1)
            .is_none_or(|installation| installation >= required_pulse)
    {
        return Err(Error::InvalidInput);
    }
    Ok(())
}

// Fresh-network ceremony authority is the genesis epoch roster only. Refuse an
// installation at or beyond its first mandatory pulse; retained rotation needs
// its own authenticated current context and is not this command's authority.
fn first_required_pulse_height(proof: &GenesisProof) -> Result<u64> {
    use iroha_data_model::parameter::system::SumeragiNposParameters;
    let parameters = proof
        .manifest
        .effective_parameters()
        .map_err(|_| Error::InvalidInput)?;
    let npos = parameters
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .ok_or(Error::InvalidInput)?;
    npos.epoch_length_blocks()
        .get()
        .checked_sub(1)
        .ok_or(Error::Height)
}

fn draft_certificate(
    request: &Request,
    record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    observed_height: u64,
) -> Result<ThresholdKeyLifecycleCertificateV1> {
    Ok(ThresholdKeyLifecycleCertificateV1 {
        version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        action: ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
        expected_active_session_id: None,
        effective_height: observed_height.checked_add(1).ok_or(Error::Height)?,
        network_id: record.session.network_id,
        roster_hash: global_threshold_beacon_roster_hash_v1(&request.authorization_roster),
        committee_size: 4,
        quorum: 3,
        session_id: record.session.session_id,
        transcript_hash: record.session.transcript_hash,
        public_state: norito::encode_canonical(record).map_err(|_| Error::Crypto)?,
        signatures: Vec::new(),
    })
}
fn validate_bundle(bundle: &PublicBundle) -> Result<()> {
    validate_genesis(&bundle.request, &bundle.genesis)?;
    validate_roster(&bundle.request.authorization_roster)?;
    let _ = validate_request(&bundle.request, bundle.request.dkg_session.start_height)?;
    bundle.record.validate().map_err(|_| Error::Crypto)?;
    if bundle.schema != "iroha.global-beacon.bootstrap.bundle.v1"
        || bundle.record.activated_at_height.is_some()
        || bundle.record.retired_at_height.is_some()
        || bundle.record.session.adaptive_dkg.session != bundle.request.dkg_session
        || bundle.record.session.adaptive_dkg.finalized_at_height
            != bundle.finalized_observed_height
        || bundle.certificate
            != draft_certificate(
                &bundle.request,
                &bundle.record,
                bundle.finalized_observed_height,
            )?
        || bundle.providers.len() != 4
        || bundle.certificate.effective_height >= first_required_pulse_height(&bundle.genesis)?
    {
        return Err(Error::InvalidInput);
    }
    for (i, provider) in bundle.providers.iter().enumerate() {
        if provider.signer_index != (i as u16) + 1
            || provider.validator != bundle.request.target_roster[i]
            || provider.handle != bundle.request.provider_handles[i]
            || provider.revision != bundle.request.provider_revision
            || provider.policy_digest
                != global_beacon_partial_signer_public_inventory_digest_v1(
                    bundle.record.session.network_id,
                    &[(bundle.record.session.clone(), provider.signer_index)],
                )
                .map_err(|_| Error::Crypto)?
        {
            return Err(Error::InvalidInput);
        }
    }
    Ok(())
}
fn sign_install(
    bundle: &PublicBundle,
    signer_index: u16,
    key: &KeyPair,
) -> Result<ThresholdKeyLifecycleSignatureV1> {
    validate_bundle(bundle)?;
    let peer = bundle
        .request
        .authorization_roster
        .get(usize::from(signer_index))
        .ok_or(Error::InvalidInput)?;
    if peer.public_key() != key.public_key() {
        return Err(Error::InvalidInput);
    }
    let preimage = threshold_key_lifecycle_certificate_preimage_v1(&bundle.certificate)
        .map_err(|_| Error::Crypto)?;
    let signature = Signature::try_new(key.private_key(), &preimage).map_err(|_| Error::Crypto)?;
    signature
        .verify(peer.public_key(), &preimage)
        .map_err(|_| Error::Crypto)?;
    Ok(ThresholdKeyLifecycleSignatureV1 {
        signer_index,
        signature,
    })
}
fn assemble_install(
    bundle: &PublicBundle,
    signatures: Vec<ThresholdKeyLifecycleSignatureV1>,
) -> Result<ThresholdKeyLifecycleCertificateV1> {
    validate_bundle(bundle)?;
    let mut certificate = bundle.certificate.clone();
    certificate.signatures = signatures;
    // Reject caller reordering, duplicates, extra/missing signatures and all altered bindings.
    verify_threshold_key_lifecycle_certificate_v1(
        &certificate,
        &bundle.request.dkg_session.network_id,
        certificate.effective_height,
        &bundle.request.authorization_roster,
    )
    .map_err(|_| Error::Crypto)?;
    Ok(certificate)
}

fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.uid() == b.uid()
        && a.gid() == b.gid()
        && a.mode() == b.mode()
        && a.nlink() == b.nlink()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
// Resolve each ancestor relative to its already opened directory. Holding the
// selected directory keeps a concurrent pathname replacement from redirecting
// credential writes; revalidation refuses to publish success after a replacement.
struct Directory {
    path: PathBuf,
    file: File,
}
impl Directory {
    fn open(path: &Path) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        if !path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
        {
            return Err(Error::InvalidCustody);
        }
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut file =
            File::from(rustix::fs::open("/", flags, Mode::empty()).map_err(|_| Error::Io)?);
        validate_directory(&file.metadata().map_err(|_| Error::Io)?)?;
        for component in path.components() {
            if let Component::Normal(name) = component {
                file = File::from(
                    rustix::fs::openat(&file, name, flags, Mode::empty())
                        .map_err(|_| Error::InvalidCustody)?,
                );
                validate_directory(&file.metadata().map_err(|_| Error::Io)?)?;
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }
    fn revalidate(&self) -> Result<()> {
        let current = Self::open(&self.path)?;
        let held = self.file.metadata().map_err(|_| Error::Io)?;
        let named = current.file.metadata().map_err(|_| Error::Io)?;
        validate_directory(&held)?;
        if held.dev() != named.dev()
            || held.ino() != named.ino()
            || held.uid() != named.uid()
            || held.gid() != named.gid()
            || held.mode() != named.mode()
        {
            return Err(Error::InvalidCustody);
        }
        Ok(())
    }
    fn child(&self, name: &std::ffi::OsStr) -> Result<Self> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        self.revalidate()?;
        rustix::fs::mkdirat(&self.file, name, Mode::from_raw_mode(0o700)).map_err(|_| Error::Io)?;
        let file = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Io)?,
        );
        let child = Self {
            path: self.path.join(name),
            file,
        };
        let m = child.file.metadata().map_err(|_| Error::Io)?;
        if m.uid() != rustix::process::geteuid().as_raw() || m.mode() & 0o7777 != 0o700 {
            return Err(Error::InvalidCustody);
        }
        self.file.sync_all().map_err(|_| Error::Io)?;
        self.revalidate()?;
        child.revalidate()?;
        Ok(child)
    }
    fn write_new(&self, name: &std::ffi::OsStr, bytes: &[u8], private: bool) -> Result<()> {
        use rustix::fs::{Mode, OFlags};
        single_name(name)?;
        if bytes.is_empty() || bytes.len() > MAX_PUBLIC_BYTES {
            return Err(Error::InvalidInput);
        }
        self.revalidate()?;
        let mut file = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(if private { 0o600 } else { 0o644 }),
            )
            .map_err(|_| Error::Io)?,
        );
        let before = file.metadata().map_err(|_| Error::Io)?;
        if before.uid() != rustix::process::geteuid().as_raw()
            || before.nlink() != 1
            || !before.is_file()
            || (private && before.mode() & 0o7777 != 0o600)
        {
            return Err(Error::InvalidCustody);
        }
        file.write_all(bytes).map_err(|_| Error::Io)?;
        file.sync_all().map_err(|_| Error::Io)?;
        let named = File::from(
            rustix::fs::openat(
                &self.file,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| Error::Io)?,
        );
        if !same_file(
            &file.metadata().map_err(|_| Error::Io)?,
            &named.metadata().map_err(|_| Error::Io)?,
        ) {
            return Err(Error::InvalidCustody);
        }
        self.file.sync_all().map_err(|_| Error::Io)?;
        self.revalidate()
    }
}
fn single_name(name: &std::ffi::OsStr) -> Result<()> {
    let mut parts = Path::new(name).components();
    if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(Error::InvalidCustody);
    }
    Ok(())
}
fn validate_directory(m: &fs::Metadata) -> Result<()> {
    if !m.is_dir()
        || (m.uid() != 0 && m.uid() != rustix::process::geteuid().as_raw())
        || m.mode() & 0o022 != 0
    {
        return Err(Error::InvalidCustody);
    }
    Ok(())
}
fn read_public_bytes(path: &Path) -> Result<Vec<u8>> {
    use rustix::fs::{Mode, OFlags};
    let parent = Directory::open(path.parent().ok_or(Error::InvalidCustody)?)?;
    let name = path.file_name().ok_or(Error::InvalidCustody)?;
    single_name(name)?;
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let mut file = File::from(
        rustix::fs::openat(&parent.file, name, flags, Mode::empty()).map_err(|_| Error::Io)?,
    );
    let before = file.metadata().map_err(|_| Error::Io)?;
    if !before.is_file()
        || before.nlink() != 1
        || before.mode() & 0o022 != 0
        || before.len() == 0
        || before.len() > MAX_PUBLIC_BYTES as u64
    {
        return Err(Error::InvalidInput);
    }
    let mut bytes = vec![0; before.len() as usize];
    file.read_exact(&mut bytes).map_err(|_| Error::Io)?;
    let mut end = [0];
    let named = File::from(
        rustix::fs::openat(&parent.file, name, flags, Mode::empty()).map_err(|_| Error::Io)?,
    );
    if file.read(&mut end).map_err(|_| Error::Io)? != 0
        || !same_file(&before, &file.metadata().map_err(|_| Error::Io)?)
        || !same_file(&before, &named.metadata().map_err(|_| Error::Io)?)
    {
        return Err(Error::InvalidInput);
    }
    parent.revalidate()?;
    Ok(bytes)
}
fn read_json<T: norito::json::JsonDeserialize>(path: &Path) -> Result<T> {
    norito::json::from_slice(&read_public_bytes(path)?).map_err(|_| Error::InvalidInput)
}
fn json_bytes<T: norito::json::JsonSerialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = norito::json::to_vec(value).map_err(|_| Error::InvalidInput)?;
    if bytes.is_empty() || bytes.len() > MAX_PUBLIC_BYTES {
        return Err(Error::InvalidInput);
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
    Directory::open(path.parent().ok_or(Error::InvalidCustody)?)?.write_new(
        path.file_name().ok_or(Error::InvalidCustody)?,
        bytes,
        private,
    )
}
fn create_private_directory(path: &Path) -> Result<Directory> {
    Directory::open(path.parent().ok_or(Error::InvalidCustody)?)?
        .child(path.file_name().ok_or(Error::InvalidCustody)?)
}
fn read_observed_height(fd: BorrowedFd<'_>, deadline: Instant) -> Result<u64> {
    let mut digits = [0u8; 20];
    let mut used = 0;
    loop {
        require_budget(deadline)?;
        let timeout =
            rustix::event::Timespec::try_from(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| Error::Deadline)?;
        let mut polls = [rustix::event::PollFd::new(
            &fd,
            rustix::event::PollFlags::IN,
        )];
        match rustix::event::poll(&mut polls, Some(&timeout)) {
            Ok(0) => return Err(Error::Deadline),
            Ok(_) if polls[0].revents().contains(rustix::event::PollFlags::NVAL) => {
                return Err(Error::Height);
            }
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Error::Io),
        }
        require_budget(deadline)?;
        let mut byte = [0u8];
        match rustix::io::read(fd, &mut byte) {
            Ok(0) => return Err(Error::Height),
            Ok(_) if byte[0] == b'\n' => {
                if used == 0 || (used > 1 && digits[0] == b'0') {
                    return Err(Error::Height);
                }
                return std::str::from_utf8(&digits[..used])
                    .map_err(|_| Error::Height)?
                    .parse()
                    .map_err(|_| Error::Height);
            }
            Ok(_) if used < digits.len() && byte[0].is_ascii_digit() => {
                digits[used] = byte[0];
                used += 1;
            }
            Ok(_) => return Err(Error::Height),
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Err(Error::Io),
        }
    }
}
#[allow(
    unsafe_code,
    reason = "the controller lends an inherited public-only height pipe for this bounded command"
)]
fn provision_command(
    request_path: &Path,
    manifest_path: &Path,
    wire_path: &Path,
    key_path: &Path,
    observed_height: u64,
    height_fd: i32,
    output: &Path,
    timeout_ms: u64,
) -> Result<()> {
    if height_fd < 3
        || matches!(height_fd, 198 | 199 | 200)
        || timeout_ms == 0
        || timeout_ms > MAX_TIMEOUT_MS
    {
        return Err(Error::InvalidInput);
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .ok_or(Error::Deadline)?;
    // Only a pipe dedicated to public observations is accepted; never borrow a
    // runtime credential descriptor or a regular file as a height stream.
    let fd = unsafe { BorrowedFd::borrow_raw(height_fd) };
    let metadata = rustix::fs::fstat(fd).map_err(|_| Error::InvalidInput)?;
    if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::Fifo {
        return Err(Error::InvalidInput);
    }
    iroha_genesis::init_instruction_registry();
    let request: Request = read_json(request_path)?;
    let public_key_bytes = read_public_bytes(key_path)?;
    let public_key_text =
        std::str::from_utf8(&public_key_bytes).map_err(|_| Error::InvalidInput)?;
    let key_text = public_key_text
        .strip_suffix('\n')
        .ok_or(Error::InvalidInput)?;
    let public_key = PublicKey::from_str(key_text).map_err(|_| Error::InvalidInput)?;
    if public_key.to_string() != key_text {
        return Err(Error::InvalidInput);
    }
    let genesis = GenesisProof {
        manifest: read_json(manifest_path)?,
        signed_wire: read_public_bytes(wire_path)?,
        public_key,
    };
    validate_request(&request, observed_height)?;
    validate_genesis(&request, &genesis)?;
    require_budget(deadline)?;
    let output = create_private_directory(output)?;
    let (bundle, credentials) = ceremony(
        request,
        genesis,
        observed_height,
        deadline,
        |snapshot| {
            output.write_new(
                std::ffi::OsStr::new("sharing-snapshot.json"),
                &json_bytes(snapshot)?,
                false,
            )?;
            println!(
                "{{\"schema\":\"iroha.global-beacon.bootstrap.progress.v1\",\"state\":\"sharing-ready\",\"observed_height\":{observed_height}}}"
            );
            std::io::stdout().flush().map_err(|_| Error::Io)
        },
        |limit| read_observed_height(fd, limit),
    )?;
    for (index, bytes) in credentials.iter().enumerate() {
        require_budget(deadline)?;
        let seat = output.child(std::ffi::OsStr::new(&format!("seat-{}", index + 1)))?;
        seat.write_new(
            std::ffi::OsStr::new(GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1),
            bytes,
            true,
        )?;
    }
    drop(credentials);
    require_budget(deadline)?;
    // The public bundle is the completion marker, published only after all four credentials.
    output.write_new(
        std::ffi::OsStr::new("public-bundle.json"),
        &json_bytes(&bundle)?,
        false,
    )?;
    require_budget(deadline)?;
    println!(
        "{{\"schema\":\"iroha.global-beacon.bootstrap.progress.v1\",\"state\":\"prepared\",\"observed_height\":{},\"install_height\":{}}}",
        bundle.finalized_observed_height, bundle.certificate.effective_height
    );
    Ok(())
}
fn load_lifecycle_key(file: File) -> Result<KeyPair> {
    use crate::taira_runtime_signer::{
        TairaRuntimeSignerErrorV1 as KeyError, load_private_record_from_file,
    };
    load_private_record_from_file(file, 71, |bytes| {
        let literal = std::str::from_utf8(bytes.strip_suffix(b"\n").ok_or(KeyError::InvalidKey)?)
            .map_err(|_| KeyError::InvalidKey)?;
        let exposed = literal
            .parse::<ExposedPrivateKey>()
            .map_err(|_| KeyError::InvalidKey)?;
        let canonical = Zeroizing::new(
            exposed
                .try_to_multihash_string()
                .map_err(|_| KeyError::InvalidKey)?,
        );
        if exposed.0.algorithm() != Algorithm::BlsNormal || canonical.as_str() != literal {
            return Err(KeyError::InvalidKey);
        }
        KeyPair::from_private_key(exposed.0).map_err(|_| KeyError::InvalidKey)
    })
    .map_err(|_| Error::InvalidCustody)
}
// A deliberately narrow native configuration view. Full Root parsing would
// reopen unrelated onboarding/faucet credentials and subsystem paths. Only this
// inline consensus identity participates in lifecycle signing; no environment,
// extends, file selector, or default can supply an authority here.
#[derive(iroha_config_base::ReadConfig)]
struct LifecycleConfigIdentity {
    chain: iroha_model_base::chain::ChainId,
    chain_discriminant: u16,
    public_key: PublicKey,
    private_key: iroha_crypto::PrivateKey,
    #[config(nested)]
    genesis: LifecycleConfigGenesis,
}
#[derive(iroha_config_base::ReadConfig)]
struct LifecycleConfigGenesis {
    expected_hash: iroha_data_model::NetworkId,
}
fn scrub_config_table(table: &mut toml::Table) {
    fn scrub(value: &mut toml::Value) {
        match value {
            toml::Value::String(value) => value.zeroize(),
            toml::Value::Array(values) => values.iter_mut().for_each(scrub),
            toml::Value::Table(table) => scrub_config_table(table),
            _ => {}
        }
    }
    table.iter_mut().for_each(|(_, value)| scrub(value));
}
fn lifecycle_key_from_config(
    bytes: &[u8],
    network: &iroha_data_model::NetworkId,
) -> Result<KeyPair> {
    use iroha_config_base::{read::ConfigReader, toml::TomlSource};
    let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidCustody)?;
    let table: toml::Table = toml::from_str(text).map_err(|_| Error::InvalidCustody)?;
    let origin = PathBuf::from("inherited-validator-config-fd198");
    let mut source = TomlSource::new_sensitive(origin.clone(), table, scrub_config_table);
    let root = source.table_mut();
    if root.contains_key("extends") || root.contains_key("private_key_file") {
        return Err(Error::InvalidCustody);
    }
    let genesis = root
        .get("genesis")
        .and_then(toml::Value::as_table)
        .ok_or(Error::InvalidCustody)?;
    if genesis.contains_key("expected_hash_file") {
        return Err(Error::InvalidCustody);
    }
    let expected = genesis
        .get("expected_hash")
        .ok_or(Error::InvalidCustody)?
        .clone();
    let mut projection = TomlSource::new_sensitive(origin, toml::Table::new(), scrub_config_table);
    for name in ["chain", "chain_discriminant", "public_key", "private_key"] {
        projection
            .table_mut()
            .insert(name.into(), root.remove(name).ok_or(Error::InvalidCustody)?);
    }
    let mut public_genesis = toml::Table::new();
    public_genesis.insert("expected_hash".into(), expected);
    projection
        .table_mut()
        .insert("genesis".into(), toml::Value::Table(public_genesis));
    let identity = ConfigReader::new()
        .without_env()
        .with_toml_source(projection)
        .read_and_complete::<LifecycleConfigIdentity>()
        .map_err(|_| Error::InvalidCustody)?;
    if identity.chain.to_string() != "fc56984b-2be7-431d-840e-21514d1883f0"
        || identity.chain_discriminant != 369
        || &identity.genesis.expected_hash != network
        || identity.public_key.try_algorithm() != Ok(Algorithm::BlsNormal)
        || identity.private_key.algorithm() != Algorithm::BlsNormal
    {
        return Err(Error::InvalidCustody);
    }
    KeyPair::new(identity.public_key, identity.private_key).map_err(|_| Error::InvalidCustody)
}
fn load_lifecycle_config(file: File, network: &iroha_data_model::NetworkId) -> Result<KeyPair> {
    use crate::taira_runtime_signer::{
        TairaRuntimeSignerErrorV1 as KeyError, load_private_record_from_file,
    };
    let length = file.metadata().map_err(|_| Error::InvalidCustody)?.len();
    if length == 0 || length > iroha_config_base::toml::MAX_TOML_SOURCE_BYTES {
        return Err(Error::InvalidCustody);
    }
    load_private_record_from_file(file, length, |bytes| {
        lifecycle_key_from_config(bytes, network).map_err(|_| KeyError::InvalidKey)
    })
    .map_err(|_| Error::InvalidCustody)
}
fn sign_command(
    bundle_path: &Path,
    signer_index: u16,
    key_fd: Option<i32>,
    config_fd: Option<i32>,
    output: &Path,
) -> Result<()> {
    let (fd, config) = match (key_fd, config_fd) {
        (Some(198), None) => (198, false),
        (None, Some(198)) => (198, true),
        _ => return Err(Error::InvalidInput),
    };
    iroha_genesis::init_instruction_registry();
    let bundle: PublicBundle = read_json(bundle_path)?;
    validate_bundle(&bundle)?;
    let file = crate::taira_runtime_signer::take_inherited_private_file(fd)
        .map_err(|_| Error::InvalidCustody)?;
    let key = if config {
        load_lifecycle_config(file, &bundle.request.dkg_session.network_id)?
    } else {
        load_lifecycle_key(file)?
    };
    let signed = sign_install(&bundle, signer_index, &key)?;
    write_new(output, &json_bytes(&signed)?, false)
}
fn assemble_command(bundle_path: &Path, signatures: &[PathBuf], output: &Path) -> Result<()> {
    iroha_genesis::init_instruction_registry();
    let bundle: PublicBundle = read_json(bundle_path)?;
    let signatures = signatures
        .iter()
        .map(|p| read_json(p))
        .collect::<Result<Vec<_>>>()?;
    let certificate = assemble_install(&bundle, signatures)?;
    let instructions = vec![InstructionBox::from(
        ApplyThresholdKeyLifecycleCertificateV1 { certificate },
    )];
    write_new(output, &json_bytes(&instructions)?, false)
}

#[cfg(test)]
#[path = "beacon_bootstrap_tests.rs"]
mod tests;
