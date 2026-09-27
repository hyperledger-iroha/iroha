//! Signed-genesis trust root and one-owner beacon DKG provisioning.

use super::*;
use iroha_data_model::parameter::system::SumeragiNposParameters;

const REQUEST_SCHEMA: &str = "iroha.global-beacon.bootstrap.request.v1";
const BUNDLE_SCHEMA: &str = "iroha.global-beacon.bootstrap.bundle.v1";

/// Pin a fresh network to one genesis beacon transcript identity.
pub(super) fn canonical_genesis_session_id(network: NetworkId) -> [u8; 32] {
    Hash::new_from_chunks(&[
        b"iroha.global-beacon.genesis-session.v1\0",
        network.as_bytes(),
    ])
    .into()
}

/// Pin the network's only genesis dealer attempt across crash recovery.
pub(super) fn canonical_genesis_attempt_id(network: NetworkId) -> [u8; 32] {
    Hash::new_from_chunks(&[
        b"iroha.global-beacon.genesis-attempt.v1\0",
        network.as_bytes(),
    ])
    .into()
}

/// Reject a rerolled session under the same signed genesis or attempt root.
pub(super) fn genesis_session_identity_is_canonical(
    network: NetworkId,
    session: GlobalThresholdBeaconDkgSessionV1,
) -> bool {
    session.network_id == network
        && session.session_id == canonical_genesis_session_id(network)
        && session.attempt_id == canonical_genesis_attempt_id(network)
        && session.authority_generation == 0
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GenesisRequest {
    schema: String,
    dkg_session: GlobalThresholdBeaconDkgSessionV1,
    target_roster: Vec<PeerId>,
    authorization_roster: Vec<PeerId>,
    provider_handles: Vec<String>,
    provider_revision: u64,
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GenesisProof {
    manifest: iroha_genesis::RawGenesisTransaction,
    signed_wire: Vec<u8>,
    public_key: PublicKey,
    first_finality: BridgeFinalityProof,
}

#[derive(Clone, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GenesisPublicBundle {
    schema: String,
    request: GenesisRequest,
    genesis: GenesisProof,
    phase_proofs: Vec<BridgeFinalityProof>,
    finalized_observed_height: u64,
    record: FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    finalization_draft: ThresholdKeyLifecycleCertificateV1,
    providers: Vec<Provider>,
}

fn read_genesis_proof(
    manifest_path: &Path,
    wire_path: &Path,
    key_path: &Path,
    first_finality_path: &Path,
) -> Result<GenesisProof> {
    iroha_genesis::init_instruction_registry();
    let text = read_public_bytes_bounded(key_path, 512)?;
    let literal = std::str::from_utf8(&text)
        .map_err(|_| Error::InvalidInput)?
        .strip_suffix('\n')
        .ok_or(Error::InvalidInput)?;
    let public_key = literal
        .parse::<PublicKey>()
        .map_err(|_| Error::InvalidInput)?;
    if public_key.to_string() != literal {
        return Err(Error::InvalidInput);
    }
    let bytes = read_public_bytes_bounded(first_finality_path, MAX_ROTATION_PHASE_PROOF_BYTES)?;
    let first_finality =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| Error::Crypto)?;
    Ok(GenesisProof {
        manifest: read_json(manifest_path)?,
        signed_wire: read_public_bytes(wire_path)?,
        public_key,
        first_finality,
    })
}

fn first_required_pulse_height(genesis: &GenesisProof) -> Result<u64> {
    let parameters = genesis
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

fn verify_signed_genesis_attempt(
    network: NetworkId,
    chain_discriminant: u16,
    request: &GenesisRequest,
    genesis: &GenesisProof,
) -> Result<(Vec<PeerId>, BridgeFinalityVerifier, u64)> {
    iroha_genesis::init_instruction_registry();
    let session = request.dkg_session;
    let validated = iroha_genesis::validate_prepared_genesis_bundle(
        &genesis.signed_wire,
        &genesis.manifest,
        &genesis.public_key,
        network.into_genesis_hash(),
    )
    .map_err(|_| Error::Crypto)?;
    if genesis.manifest.consensus_mode()
        != iroha_data_model::parameter::system::SumeragiConsensusMode::Npos
        || genesis.manifest.chain_discriminant() != chain_discriminant
        || genesis.first_finality.block_header.height().get() != 1
        || genesis.first_finality.block_header.hash() != validated.block().hash()
    {
        return Err(Error::InvalidInput);
    }
    let signed_genesis = iroha_genesis::GenesisBlock(validated.block().clone());
    let roster = iroha_core::sumeragi::startup::genesis_committee_peers(&signed_genesis.0)
        .map_err(|_| Error::Crypto)?;
    if roster.len() != 4
        || request.schema != REQUEST_SCHEMA
        || request.target_roster != roster
        || request.authorization_roster != roster
        || request.provider_handles.len() != roster.len()
        || request
            .provider_handles
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != roster.len()
        || request.provider_handles.iter().any(|handle| {
            iroha_config::parameters::validate_production_runtime_handle(handle).is_err()
        })
        || request.provider_revision == 0
        || !genesis_session_identity_is_canonical(network, session)
        || session.roster_hash != global_threshold_beacon_roster_hash_v1(&roster)
        || session.committee_size != 4
        || session.threshold != 2
        || session.start_height != 1
        || session.commitments_end_height != 2
        || session.deliveries_end_height != 3
        || session.acceptances_end_height != 4
    {
        return Err(Error::InvalidInput);
    }
    GlobalThresholdBeaconDkgStateV1::new(session, &AdaptiveGlobalThresholdBeaconDkgCryptoV1)
        .map_err(|_| Error::Crypto)?;
    let cutoff = first_required_pulse_height(genesis)?;
    if session.acceptances_end_height >= cutoff {
        return Err(Error::Height);
    }
    let context = &genesis.first_finality.finality_artifact.height_context;
    iroha_core::sumeragi::validate_signed_genesis_v2_authority(
        &signed_genesis,
        context,
        &genesis.first_finality.finality_artifact.validator_set_pops,
    )
    .map_err(|_| Error::Crypto)?;
    let mut verifier = BridgeFinalityVerifier::with_context(
        network,
        genesis.first_finality.finality_artifact.context_id(),
    );
    verifier
        .verify(&genesis.first_finality)
        .map_err(|_| Error::Crypto)?;
    Ok((roster, verifier, cutoff))
}

#[allow(
    unsafe_code,
    reason = "the supervisor passes separate inherited public DKG and finality-proof FIFOs"
)]
/// Provision exactly one genesis voting seat under a signed-genesis trust root.
pub(super) fn provision_genesis_seat_command(
    network: NetworkId,
    chain_discriminant: u16,
    request_path: &Path,
    manifest_path: &Path,
    wire_path: &Path,
    key_path: &Path,
    first_finality_path: &Path,
    signer_index: u16,
    key_fd: Option<i32>,
    config_fd: Option<i32>,
    public_fd: i32,
    finality_fd: i32,
    attempt_root: &Path,
    timeout_ms: u64,
) -> Result<()> {
    let _profile =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
    let (key_descriptor, from_config) = match (key_fd, config_fd) {
        (Some(198), None) => (198, false),
        (None, Some(198)) => (198, true),
        _ => return Err(Error::InvalidInput),
    };
    if public_fd < 3
        || finality_fd < 3
        || public_fd == finality_fd
        || matches!(public_fd, 198 | 199 | 200)
        || matches!(finality_fd, 198 | 199 | 200)
        || timeout_ms == 0
        || timeout_ms > MAX_TIMEOUT_MS
    {
        return Err(Error::InvalidInput);
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(timeout_ms))
        .ok_or(Error::Deadline)?;
    let public_input = unsafe { BorrowedFd::borrow_raw(public_fd) };
    let finality_input = unsafe { BorrowedFd::borrow_raw(finality_fd) };
    for fd in [public_input, finality_input] {
        let metadata = rustix::fs::fstat(fd).map_err(|_| Error::InvalidInput)?;
        if rustix::fs::FileType::from_raw_mode(metadata.st_mode) != rustix::fs::FileType::Fifo {
            return Err(Error::InvalidInput);
        }
    }
    let request: GenesisRequest = read_json(request_path)?;
    let genesis = read_genesis_proof(manifest_path, wire_path, key_path, first_finality_path)?;
    let (roster, verifier, cutoff) =
        verify_signed_genesis_attempt(network, chain_discriminant, &request, &genesis)?;
    if signer_index == 0 || usize::from(signer_index) > roster.len() {
        return Err(Error::InvalidInput);
    }
    let file = crate::taira_runtime_signer::take_inherited_private_file(key_descriptor)
        .map_err(|_| Error::InvalidCustody)?;
    let signer = if from_config {
        load_lifecycle_config(file, &network)?
    } else {
        load_lifecycle_key(file)?
    };
    if signer.public_key() != roster[usize::from(signer_index - 1)].public_key() {
        return Err(Error::InvalidCustody);
    }
    let session = request.dkg_session;
    let handle = &request.provider_handles[usize::from(signer_index - 1)];
    let output = rotation_seat::claim_attempt_directory(attempt_root, &session, signer_index)?;
    rotation_seat::run_seat_dkg(
        session,
        roster,
        signer_index,
        signer,
        public_input,
        finality_input,
        verifier,
        cutoff,
        genesis
            .first_finality
            .finality_artifact
            .context_id()
            .0
            .into(),
        1,
        handle,
        request.provider_revision,
        output,
        deadline,
    )
}

fn validate_genesis_phase_chain(
    network: NetworkId,
    chain_discriminant: u16,
    request: &GenesisRequest,
    genesis: &GenesisProof,
    phases: &[BridgeFinalityProof],
) -> Result<Vec<PeerId>> {
    let (roster, mut verifier, cutoff) =
        verify_signed_genesis_attempt(network, chain_discriminant, request, genesis)?;
    if phases.len() != 3 {
        return Err(Error::Height);
    }
    let mut last = 1;
    for phase in phases {
        let height = phase.finality_artifact.height;
        check_rotation_phase_height(last, height, phase.block_header.height().get(), cutoff)?;
        verifier.verify(phase).map_err(|_| Error::Crypto)?;
        last = height;
    }
    if last != request.dkg_session.acceptances_end_height {
        return Err(Error::Height);
    }
    Ok(roster)
}

fn draft_genesis_certificate(
    roster: &[PeerId],
    record: &FinalizedGlobalThresholdBeaconKeySessionRecordV1,
    effective_height: u64,
) -> Result<ThresholdKeyLifecycleCertificateV1> {
    if roster.len() != 4 || effective_height <= record.session.adaptive_dkg.finalized_at_height {
        return Err(Error::Height);
    }
    Ok(ThresholdKeyLifecycleCertificateV1 {
        version: THRESHOLD_KEY_LIFECYCLE_CERTIFICATE_VERSION_V1,
        action: ThresholdKeyLifecycleActionV1::FinalizeGlobalBeaconKey,
        expected_active_session_id: None,
        effective_height,
        network_id: record.session.network_id,
        roster_hash: global_threshold_beacon_roster_hash_v1(roster),
        committee_size: 4,
        quorum: 3,
        session_id: record.session.session_id,
        transcript_hash: record.session.transcript_hash,
        public_state: norito::encode_canonical(record).map_err(|_| Error::Crypto)?,
        signatures: Vec::new(),
    })
}

fn validate_genesis_bundle(
    bundle: &GenesisPublicBundle,
    network: NetworkId,
    chain_discriminant: u16,
) -> Result<Vec<PeerId>> {
    let roster = validate_genesis_phase_chain(
        network,
        chain_discriminant,
        &bundle.request,
        &bundle.genesis,
        &bundle.phase_proofs,
    )?;
    let session = bundle.request.dkg_session;
    bundle.record.validate().map_err(|_| Error::Crypto)?;
    if bundle.schema != BUNDLE_SCHEMA
        || bundle.record.session.adaptive_dkg.session != session
        || bundle.record.session.adaptive_dkg.finalized_at_height != session.acceptances_end_height
        || bundle.finalized_observed_height != session.acceptances_end_height
        || bundle.record.activated_at_height.is_some()
        || bundle.record.retired_at_height.is_some()
        || bundle.finalization_draft.effective_height
            >= first_required_pulse_height(&bundle.genesis)?
        || bundle.finalization_draft
            != draft_genesis_certificate(
                &roster,
                &bundle.record,
                bundle.finalization_draft.effective_height,
            )?
        || bundle.providers.len() != roster.len()
    {
        return Err(Error::InvalidInput);
    }
    for (offset, provider) in bundle.providers.iter().enumerate() {
        let seat = u16::try_from(offset + 1).map_err(|_| Error::InvalidInput)?;
        if provider.signer_index != seat
            || provider.validator != roster[offset]
            || provider.handle != bundle.request.provider_handles[offset]
            || provider.revision != bundle.request.provider_revision
            || provider.policy_digest
                != global_beacon_partial_signer_public_inventory_digest_v1(
                    network,
                    &[(bundle.record.session.clone(), seat)],
                )
                .map_err(|_| Error::Crypto)?
        {
            return Err(Error::InvalidInput);
        }
    }
    Ok(roster)
}

/// Assemble the complete public genesis transcript after signed h1–h4 finality.
pub(super) fn assemble_genesis_dkg_command(
    network: NetworkId,
    chain_discriminant: u16,
    request_path: &Path,
    manifest_path: &Path,
    wire_path: &Path,
    key_path: &Path,
    first_finality_path: &Path,
    phase_paths: &[PathBuf],
    public_session_path: &Path,
    provider_paths: &[PathBuf],
    certificate_height: u64,
    output: &Path,
) -> Result<()> {
    let _profile =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
    let request: GenesisRequest = read_json(request_path)?;
    let genesis = read_genesis_proof(manifest_path, wire_path, key_path, first_finality_path)?;
    let phase_proofs = phase_paths
        .iter()
        .map(|path| {
            let bytes = read_public_bytes_bounded(path, MAX_ROTATION_PHASE_PROOF_BYTES)?;
            norito::decode_canonical_with_limits(
                &bytes,
                norito::canonical_decode_limits(bytes.len()),
            )
            .map_err(|_| Error::Crypto)
        })
        .collect::<Result<Vec<BridgeFinalityProof>>>()?;
    let roster = validate_genesis_phase_chain(
        network,
        chain_discriminant,
        &request,
        &genesis,
        &phase_proofs,
    )?;
    let bytes = read_public_bytes(public_session_path)?;
    let public: GlobalThresholdBeaconKeySessionV1 =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| Error::Crypto)?;
    let record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(public).map_err(|_| Error::Crypto)?;
    let mut providers = provider_paths
        .iter()
        .map(|path| read_json::<Provider>(path))
        .collect::<Result<Vec<_>>>()?;
    providers.sort_by_key(|provider| provider.signer_index);
    let draft = draft_genesis_certificate(&roster, &record, certificate_height)?;
    let bundle = GenesisPublicBundle {
        schema: BUNDLE_SCHEMA.into(),
        request,
        genesis,
        phase_proofs,
        finalized_observed_height: record.session.adaptive_dkg.finalized_at_height,
        record,
        finalization_draft: draft,
        providers,
    };
    validate_genesis_bundle(&bundle, network, chain_discriminant)?;
    write_new(output, &json_bytes(&bundle)?, false)
}

/// Sign the exact genesis-roster installation draft with one native identity.
pub(super) fn sign_genesis_install_command(
    network: NetworkId,
    chain_discriminant: u16,
    bundle_path: &Path,
    signer_index: u16,
    key_fd: Option<i32>,
    config_fd: Option<i32>,
    output: &Path,
) -> Result<()> {
    let _profile =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
    iroha_genesis::init_instruction_registry();
    let (fd, config) = match (key_fd, config_fd) {
        (Some(198), None) => (198, false),
        (None, Some(198)) => (198, true),
        _ => return Err(Error::InvalidInput),
    };
    let bundle: GenesisPublicBundle = read_json(bundle_path)?;
    let roster = validate_genesis_bundle(&bundle, network, chain_discriminant)?;
    let file = crate::taira_runtime_signer::take_inherited_private_file(fd)
        .map_err(|_| Error::InvalidCustody)?;
    let key = if config {
        load_lifecycle_config(file, &network)?
    } else {
        load_lifecycle_key(file)?
    };
    let signed = sign_rotation_draft(&bundle.finalization_draft, &roster, signer_index, &key)?;
    write_new(output, &json_bytes(&signed)?, false)
}

/// Emit an install instruction only after the exact genesis quorum signs.
pub(super) fn assemble_genesis_install_command(
    network: NetworkId,
    chain_discriminant: u16,
    bundle_path: &Path,
    signature_paths: &[PathBuf],
    output: &Path,
) -> Result<()> {
    let _profile =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(chain_discriminant);
    iroha_genesis::init_instruction_registry();
    let bundle: GenesisPublicBundle = read_json(bundle_path)?;
    let roster = validate_genesis_bundle(&bundle, network, chain_discriminant)?;
    let signatures = signature_paths
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<_>>>()?;
    let certificate = assemble_rotation_draft(&bundle.finalization_draft, &roster, signatures)?;
    let instructions = vec![InstructionBox::from(
        ApplyThresholdKeyLifecycleCertificateV1 { certificate },
    )];
    write_new(output, &json_bytes(&instructions)?, false)
}
