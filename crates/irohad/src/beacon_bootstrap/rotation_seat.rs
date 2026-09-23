//! One-shot, proof-clocked rotation DKG custody for one exact target seat.

use super::*;
use norito::{NoritoDeserialize, NoritoSerialize};

#[derive(JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct SeatAttemptJournalV1 {
    schema: String,
    session: GlobalThresholdBeaconDkgSessionV1,
    signer_index: u16,
    trusted_context_id: Hash,
    anchor_height: u64,
}

fn read_public_frame<T>(fd: BorrowedFd<'_>, deadline: Instant) -> Result<T>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let mut length = [0_u8; 4];
    read_exact_until(fd, deadline, &mut length)?;
    let length = usize::try_from(u32::from_be_bytes(length)).map_err(|_| Error::InvalidInput)?;
    if length == 0 || length > MAX_PUBLIC_BYTES {
        return Err(Error::InvalidInput);
    }
    let mut bytes = vec![0_u8; length];
    read_exact_until(fd, deadline, &mut bytes)?;
    norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(|_| Error::Crypto)
}

fn write_public_frame<T: NoritoSerialize>(output: &Directory, name: &str, value: &T) -> Result<()> {
    let bytes = norito::encode_canonical(value).map_err(|_| Error::Crypto)?;
    output.write_new(std::ffi::OsStr::new(name), &bytes, false)
}

fn advance_verified_phase(
    fd: BorrowedFd<'_>,
    deadline: Instant,
    verifier: &mut BridgeFinalityVerifier,
    last_height: &mut u64,
    target_height: u64,
    cutoff_height: u64,
) -> Result<()> {
    while *last_height < target_height {
        read_rotation_phase_height(fd, deadline, verifier, last_height, cutoff_height)?;
    }
    if *last_height != target_height {
        return Err(Error::Height);
    }
    Ok(())
}

fn exact_rotation_seat_session(
    proof: &RotationProofArgs,
    selected: &VerifiedValidatorCommitteeSelectionV1,
) -> Result<(GlobalThresholdBeaconDkgSessionV1, Vec<PeerId>, u64)> {
    let preparation = selected.preparation();
    let roster = preparation
        .roster
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    let observed_height = selected.observed_height();
    let commitments_end_height = observed_height.checked_add(1).ok_or(Error::Height)?;
    let deliveries_end_height = observed_height.checked_add(2).ok_or(Error::Height)?;
    let acceptances_end_height = observed_height.checked_add(3).ok_or(Error::Height)?;
    let cutoff = preparation
        .first_height
        .checked_sub(1)
        .ok_or(Error::Height)?;
    if acceptances_end_height >= cutoff {
        return Err(Error::Height);
    }
    Ok((
        GlobalThresholdBeaconDkgSessionV1 {
            version: 1,
            network_id: proof.network_id,
            session_id: preparation
                .beacon_session_id()
                .map_err(|_| Error::InvalidInput)?,
            attempt_id: preparation
                .transition_id()
                .map_err(|_| Error::InvalidInput)?,
            authority_generation: preparation.authority_generation,
            roster_hash: global_threshold_beacon_roster_hash_v1(&roster),
            committee_size: u16::try_from(roster.len()).map_err(|_| Error::InvalidInput)?,
            threshold: u16::try_from((roster.len() - 1) / 3 + 1)
                .map_err(|_| Error::InvalidInput)?,
            start_height: observed_height,
            commitments_end_height,
            deliveries_end_height,
            acceptances_end_height,
        },
        roster,
        cutoff,
    ))
}

/// Derive the one durable child name for an exact attempt and seat.
pub(super) fn attempt_child_name(
    session: &GlobalThresholdBeaconDkgSessionV1,
    signer_index: u16,
) -> String {
    use std::fmt::Write as _;
    let mut name = String::with_capacity(86);
    name.push_str("attempt-");
    for byte in session.attempt_id {
        write!(name, "{byte:02x}").expect("writing into a String cannot fail");
    }
    write!(name, "-seat-{signer_index}").expect("writing into a String cannot fail");
    name
}

/// Exclusively claim one seat's attempt under a pinned owner-private root.
pub(super) fn claim_attempt_directory(
    root_path: &Path,
    session: &GlobalThresholdBeaconDkgSessionV1,
    signer_index: u16,
) -> Result<Directory> {
    use std::os::unix::fs::MetadataExt as _;
    let root = Directory::open(root_path)?;
    let metadata = root.file.metadata().map_err(|_| Error::Io)?;
    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o7777 != 0o700 {
        return Err(Error::InvalidCustody);
    }
    root.child(std::ffi::OsStr::new(&attempt_child_name(
        session,
        signer_index,
    )))
}

/// Encode only this seat's verified share for one exact public session.
pub(super) fn encode_local_seat_credential(
    public: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
    components: Zeroizing<[[u8; 32]; 3]>,
    handle: &str,
    revision: u64,
) -> Result<([u8; 32], Zeroizing<Vec<u8>>)> {
    let network = public.network_id;
    let inventory = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
        public,
        signer_index,
        components,
    )];
    let policy_digest = global_beacon_partial_signer_inventory_digest_v1(network, &inventory)
        .map_err(|_| Error::Crypto)?;
    let credential = encode_global_beacon_partial_signer_credential_v1(
        network,
        handle,
        revision,
        policy_digest,
        inventory,
    )
    .map_err(|_| Error::Crypto)?;
    Ok((policy_digest, credential))
}

/// Run a local dealer/recipient without ever constructing another seat's secret.
///
/// A supervisor must pin `attempt_root` to its durable private root. The
/// daemon derives the child name from the exact attempt and seat. Creating
/// that child before key generation is exclusive; a crash leaves it present
/// and this attempt must be cancelled by current-quorum retention, never rerolled.
#[allow(
    unsafe_code,
    reason = "the supervisor passes separate inherited public finality and signed-DKG frame FIFOs"
)]
pub(super) fn provision_rotation_seat_command(
    proof: &RotationProofArgs,
    signer_index: u16,
    key_fd: Option<i32>,
    config_fd: Option<i32>,
    public_fd: i32,
    finality_fd: i32,
    provider_handle: &str,
    provider_revision: u64,
    attempt_root: &Path,
    timeout_ms: u64,
) -> Result<()> {
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
        || provider_revision == 0
        || iroha_config::parameters::validate_production_runtime_handle(provider_handle).is_err()
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
    let (evidence, selected) = read_verified_rotation_selection(proof)?;
    let transition = &evidence
        .status
        .selected
        .as_ref()
        .ok_or(Error::InvalidInput)?
        .transition;
    if transition.credentials.is_some()
        || !transition.readiness.is_empty()
        || transition.outcome.is_some()
        || evidence.status.pending_beacon_session.is_some()
    {
        return Err(Error::InvalidInput);
    }
    let (session, roster, cutoff) = exact_rotation_seat_session(proof, &selected)?;
    if signer_index == 0 || usize::from(signer_index) > roster.len() {
        return Err(Error::InvalidInput);
    }
    let file = crate::taira_runtime_signer::take_inherited_private_file(key_descriptor)
        .map_err(|_| Error::InvalidCustody)?;
    let signer = if from_config {
        load_lifecycle_config(file, &session.network_id)?
    } else {
        load_lifecycle_key(file)?
    };
    if signer.public_key() != roster[usize::from(signer_index - 1)].public_key() {
        return Err(Error::InvalidCustody);
    }
    let verifier = rotation_phase_verifier(proof, &evidence)?;
    let output = claim_attempt_directory(attempt_root, &session, signer_index)?;
    run_seat_dkg(
        session,
        roster,
        signer_index,
        signer,
        public_input,
        finality_input,
        verifier,
        cutoff,
        proof.trusted_context_id,
        proof.anchor_height,
        provider_handle,
        provider_revision,
        output,
        deadline,
    )?;
    Ok(())
}

/// Drive one local secret owner from signed public edges and verified height proofs.
///
/// The exact attempt directory must have been claimed exclusively before this call.
pub(super) fn run_seat_dkg(
    session: GlobalThresholdBeaconDkgSessionV1,
    roster: Vec<PeerId>,
    signer_index: u16,
    signer: KeyPair,
    public_input: BorrowedFd<'_>,
    finality_input: BorrowedFd<'_>,
    mut verifier: BridgeFinalityVerifier,
    cutoff: u64,
    trusted_context_id: Hash,
    anchor_height: u64,
    provider_handle: &str,
    provider_revision: u64,
    output: Directory,
    deadline: Instant,
) -> Result<()> {
    output.write_new(
        std::ffi::OsStr::new("attempt-journal.json"),
        &json_bytes(&SeatAttemptJournalV1 {
            schema: "iroha.global-beacon.dkg-seat-attempt.v1".into(),
            session,
            signer_index,
            trusted_context_id,
            anchor_height,
        })?,
        true,
    )?;
    let mut local =
        LocalGlobalThresholdBeaconDkgSeatV1::new(session, &roster, signer_index, &signer)
            .map_err(|_| Error::Crypto)?;
    let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
    let mut own_public =
        GlobalThresholdBeaconDkgStateV1::new(session, &crypto).map_err(|_| Error::Crypto)?;
    let (key, commitment) = local.publication();
    own_public
        .record_recipient_key(session.start_height, key)
        .map_err(|_| Error::Crypto)?;
    own_public
        .record_dealer_commitment(session.start_height, commitment, &crypto)
        .map_err(|_| Error::Crypto)?;
    write_public_frame(
        &output,
        "publication.norito",
        &own_public.public_snapshot().map_err(|_| Error::Crypto)?,
    )?;

    let all_public: GlobalThresholdBeaconDkgSnapshotV1 = read_public_frame(public_input, deadline)?;
    if all_public.session != session
        || all_public.last_updated_height != session.start_height
        || all_public.recipient_keys.len() != roster.len()
        || all_public.dealer_commitments.len() != roster.len()
        || !all_public.encrypted_shares.is_empty()
        || !all_public.share_acceptances.is_empty()
    {
        return Err(Error::InvalidInput);
    }
    let mut public = GlobalThresholdBeaconDkgStateV1::from_snapshot(all_public, &crypto)
        .map_err(|_| Error::Crypto)?;
    let mut verified_height = session.start_height;
    advance_verified_phase(
        finality_input,
        deadline,
        &mut verifier,
        &mut verified_height,
        session.commitments_end_height,
        cutoff,
    )?;
    let snapshot = public.public_snapshot().map_err(|_| Error::Crypto)?;
    let edges = local
        .deliver(
            &snapshot.recipient_keys,
            &snapshot.dealer_commitments,
            verified_height,
            &signer,
        )
        .map_err(|_| Error::Crypto)?;
    for edge in edges {
        public
            .record_encrypted_share(verified_height, edge)
            .map_err(|_| Error::Crypto)?;
    }
    write_public_frame(
        &output,
        "deliveries.norito",
        &public.public_snapshot().map_err(|_| Error::Crypto)?,
    )?;

    let all_edges: GlobalThresholdBeaconDkgSnapshotV1 = read_public_frame(public_input, deadline)?;
    if all_edges.session != session
        || all_edges.last_updated_height != session.commitments_end_height
        || all_edges.encrypted_shares.len() != roster.len() * roster.len()
        || !all_edges.share_acceptances.is_empty()
    {
        return Err(Error::InvalidInput);
    }
    let mut accepted_public =
        GlobalThresholdBeaconDkgStateV1::from_snapshot(all_edges.clone(), &crypto)
            .map_err(|_| Error::Crypto)?;
    advance_verified_phase(
        finality_input,
        deadline,
        &mut verifier,
        &mut verified_height,
        session.deliveries_end_height,
        cutoff,
    )?;
    let acceptances = local
        .accept(&all_edges, verified_height, &signer)
        .map_err(|_| Error::Crypto)?;
    for acceptance in acceptances {
        accepted_public
            .record_share_acceptance(verified_height, acceptance)
            .map_err(|_| Error::Crypto)?;
    }
    write_public_frame(
        &output,
        "acceptances.norito",
        &accepted_public
            .public_snapshot()
            .map_err(|_| Error::Crypto)?,
    )?;

    let assembled: GlobalThresholdBeaconKeySessionV1 = read_public_frame(public_input, deadline)?;
    advance_verified_phase(
        finality_input,
        deadline,
        &mut verifier,
        &mut verified_height,
        session.acceptances_end_height,
        cutoff,
    )?;
    if assembled.adaptive_dkg.session != session
        || assembled.adaptive_dkg.finalized_at_height != verified_height
    {
        return Err(Error::InvalidInput);
    }
    let components = local
        .finalize_private_share(assembled.clone())
        .map_err(|_| Error::Crypto)?;
    let mut pending_share = Zeroizing::new(Vec::with_capacity(96));
    for component in components.iter() {
        pending_share.extend_from_slice(component);
    }
    let (policy_digest, credential) = encode_local_seat_credential(
        assembled.clone(),
        signer_index,
        components,
        provider_handle,
        provider_revision,
    )?;
    output.write_new(
        std::ffi::OsStr::new(GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1),
        &credential,
        true,
    )?;
    output.write_new(
        std::ffi::OsStr::new(ROTATION_PENDING_SHARE_NAME),
        &pending_share,
        true,
    )?;
    write_public_frame(&output, "public-session.norito", &assembled)?;
    let provider = Provider {
        signer_index,
        validator: roster[usize::from(signer_index - 1)].clone(),
        handle: provider_handle.to_owned(),
        revision: provider_revision,
        policy_digest,
    };
    output.write_new(
        std::ffi::OsStr::new("provider.json"),
        &json_bytes(&provider)?,
        false,
    )?;
    Ok(())
}

/// Assemble the all-seat public result without collecting any private shares.
pub(super) fn assemble_rotation_dkg_command(
    proof: &RotationProofArgs,
    public_session_path: &Path,
    phase_proof_paths: &[PathBuf],
    provider_paths: &[PathBuf],
    certificate_height: u64,
    output: &Path,
) -> Result<()> {
    let (evidence, selected) = read_verified_rotation_selection(proof)?;
    let preparation = selected.preparation();
    let (expected_session, _, cutoff) = exact_rotation_seat_session(proof, &selected)?;
    let bytes = read_public_bytes_bounded(public_session_path, MAX_PUBLIC_BYTES)?;
    let public: GlobalThresholdBeaconKeySessionV1 =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| Error::Crypto)?;
    let finalized_height = public.adaptive_dkg.finalized_at_height;
    if public.adaptive_dkg.session != expected_session
        || finalized_height != expected_session.acceptances_end_height
        || provider_paths.len() != preparation.roster.len()
        || certificate_height <= finalized_height
        || certificate_height >= cutoff
    {
        return Err(Error::InvalidInput);
    }
    let phase_proofs = phase_proof_paths
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
    validate_rotation_phase_chain(
        proof,
        &evidence,
        selected.observed_height(),
        finalized_height,
        cutoff,
        &phase_proofs,
    )?;
    let record =
        FinalizedGlobalThresholdBeaconKeySessionRecordV1::new(public).map_err(|_| Error::Crypto)?;
    let mut providers = provider_paths
        .iter()
        .map(|path| read_json::<Provider>(path))
        .collect::<Result<Vec<_>>>()?;
    providers.sort_by_key(|provider| provider.signer_index);
    let authorization_roster = selected
        .incumbent_authority()
        .validators
        .iter()
        .map(|seat| seat.validator.clone())
        .collect::<Vec<_>>();
    let certificate = draft_rotation_certificate(
        &authorization_roster,
        selected.incumbent_beacon().session_id,
        &record,
        finalized_height,
        certificate_height,
    )?;
    let bundle = RotationPublicBundle {
        schema: "iroha.validator-committee.rotation-dkg.v1".into(),
        preparation: preparation.clone(),
        dkg_session: record.session.adaptive_dkg.session,
        finalized_observed_height: finalized_height,
        phase_proofs,
        record,
        finalization_draft: certificate,
        providers,
    };
    validate_rotation_bundle(&bundle, proof, &evidence, &selected)?;
    write_new(output, &json_bytes(&bundle)?, false)
}
