fn validate_winning_lane_output(
    message: &BlockMessage,
    proposal: &LaneBlockProposalV1,
    signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), String> {
    match message {
        BlockMessage::LaneBlockProposal(output) => {
            validate_lane_block_proposal(output).map_err(|error| error.to_string())?;
            if output != proposal {
                return Err(
                    "winning lane proposal differs from the exact durable proposal".to_owned(),
                );
            }
        }
        BlockMessage::LaneBlockVote(vote) => {
            let phase = vote.body.phase;
            vote.validate_ingress(phase)
                .map_err(|error| error.to_string())?;
            if vote.body != proposal.vote_body(phase)
                || !proposal.descriptor.validator_set.contains(&vote.signer)
            {
                return Err("winning lane vote differs from the exact durable proposal".to_owned());
            }
        }
        BlockMessage::LaneBlockQc(qc) => {
            validate_winning_lane_qc(qc, proposal, signer_pops)?;
        }
        BlockMessage::LaneBlockCertificate(certificate) => {
            if certificate.proposal != *proposal {
                return Err(
                    "winning lane certificate differs from the exact durable proposal".to_owned(),
                );
            }
            validate_winning_lane_qc(&certificate.prepare_qc, proposal, signer_pops)?;
            validate_winning_lane_qc(&certificate.commit_qc, proposal, signer_pops)?;
        }
        _ => return Err("rollover authority received non-lane output".to_owned()),
    }
    Ok(())
}

/// Return whether two lane QCs carry the same Prepare/Commit decision while
/// differing only in quorum proof bytes.
fn lane_qcs_certify_same_decision(left: &LaneBlockQcV1, right: &LaneBlockQcV1) -> bool {
    left.body == right.body
        && left.validator_set_hash_version == right.validator_set_hash_version
        && left.validator_set_hash == right.validator_set_hash
        && left.validator_set == right.validator_set
        && left.payload_availability_qc == right.payload_availability_qc
}

/// Return whether a retained QC makes one still-backpressured vote redundant.
///
/// The exact vote body fixes proposal, phase, view, and subject. Autonomous
/// READY evidence is considered subsumed only after the retained QC carries
/// its completed availability certificate; otherwise the vote may still hold
/// unique availability progress and must cross rollover by an exact owner.
fn lane_qc_subsumes_vote(qc: &LaneBlockQcV1, vote: &LaneBlockVoteV1) -> bool {
    qc.body == vote.body
        && match (&vote.payload_availability_vote, &qc.payload_availability_qc) {
            (None, None) => true,
            (Some(vote_ready), Some(qc_ready)) => vote_ready.body == qc_ready.body,
            _ => false,
        }
}

/// Return the complete PoP authority embedded in an autonomous Prepare QC.
///
/// READY voters sign the exact lane roster together with an aligned PoP
/// vector before the Prepare QC can form. That vector remains valid after
/// rollover even when mutable State key indexes have advanced.
fn validated_autonomous_validator_pops(
    prepare_qc: &LaneBlockQcV1,
    expected_validator_set: &[PeerId],
) -> Result<Option<BTreeMap<PublicKey, Vec<u8>>>, String> {
    let Some(availability) = prepare_qc.payload_availability_qc.as_ref() else {
        return Ok(None);
    };
    crate::lane_consensus::validate_lane_payload_availability_qc(availability)
        .map_err(|error| format!("autonomous READY signer PoPs are invalid: {error}"))?;
    if prepare_qc.body.phase != CertPhase::Prepare
        || availability.validator_set != prepare_qc.validator_set
        || availability.validator_set.as_slice() != expected_validator_set
        || availability.validator_set_pops.len() != availability.validator_set.len()
    {
        return Err("autonomous READY PoPs differ from the lane certificate roster".to_owned());
    }
    Ok(Some(
        availability
            .validator_set
            .iter()
            .zip(&availability.validator_set_pops)
            .map(|(validator, pop)| (validator.public_key().clone(), pop.clone()))
            .collect(),
    ))
}

/// Project the exact Prepare/Commit signer union from a session's complete
/// autonomous READY authority for legacy certified-artifact storage.
fn autonomous_lane_session_signer_pops(
    session: &CommittedLaneBlockSession,
) -> Result<Option<BTreeMap<PublicKey, Vec<u8>>>, String> {
    let expected = &session.proposal.descriptor.validator_set;
    let Some(validator_pops) = validated_autonomous_validator_pops(&session.prepare_qc, expected)?
    else {
        return Ok(None);
    };
    if session.prepare_qc.validator_set != *expected || session.commit_qc.validator_set != *expected
    {
        return Err("autonomous READY PoPs differ from the lane certificate roster".to_owned());
    }
    let mut signer_pops = BTreeMap::new();
    for qc in [&session.prepare_qc, &session.commit_qc] {
        signer_pops.extend(project_qc_signer_pops(qc, &validator_pops)?);
    }
    Ok(Some(signer_pops))
}

fn project_qc_signer_pops(
    qc: &LaneBlockQcV1,
    validator_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<BTreeMap<PublicKey, Vec<u8>>, String> {
    qc.validator_set
        .iter()
        .enumerate()
        .filter(|(index, _)| bitmap_selects(&qc.signers_bitmap, *index))
        .map(|(_, validator)| {
            validator_pops
                .get(validator.public_key())
                .cloned()
                .map(|pop| (validator.public_key().clone(), pop))
                .ok_or_else(|| "autonomous READY PoP index is missing".to_owned())
        })
        .collect()
}

/// Validate an earlier-height lane output against its certified Kura source.
///
/// Once a full Prepare/Commit certificate is durable, proposal and QC output
/// can be reconstructed directly and any earlier valid vote is superseded by
/// that stronger exact decision. This lets a later global-height service
/// retire actor-backpressured historical output without depending on the
/// already-drained in-memory session cache.
pub(crate) fn durable_historical_lane_output_source_hash(
    kura: &Kura,
    message: &BlockMessage,
) -> Result<Option<Hash>, String> {
    let Some((_, proposal_hash)) = lane_output_identity(message) else {
        return Ok(None);
    };
    let (lane_id, lane_block_height) = match message {
        BlockMessage::LaneBlockProposal(proposal) => (
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        ),
        BlockMessage::LaneBlockVote(vote) => (vote.body.lane_id, vote.body.lane_block_height),
        BlockMessage::LaneBlockQc(qc) => (qc.body.lane_id, qc.body.lane_block_height),
        BlockMessage::LaneBlockCertificate(certificate) => (
            certificate.proposal.descriptor.lane_id,
            certificate.proposal.descriptor.lane_block_height,
        ),
        _ => return Ok(None),
    };
    let Some(durable) = kura.read_certified_lane_block_artifact(lane_id, lane_block_height) else {
        return Ok(None);
    };
    if durable.proposal.proposal_hash != proposal_hash {
        return Ok(None);
    }
    if let Err(retained_error) =
        validate_winning_lane_output(message, &durable.proposal, &durable.signer_pops)
    {
        // Preserve self-contained replay of the exact retained certificate.
        // Only an alternate quorum can require another signer from the full
        // immutable height authority.
        let signer_pops = durable_historical_lane_verification_pops(kura, &durable)?;
        if signer_pops == durable.signer_pops {
            return Err(retained_error);
        }
        validate_winning_lane_output(message, &durable.proposal, &signer_pops)?;
    }
    let durable_hash = HashOf::new(&durable);
    Ok(Some(Hash::new_from_chunks(&[
        b"iroha:sumeragi:v2:historical-lane-output-source:v1\0",
        durable_hash.as_ref(),
        HashOf::new(message).as_ref(),
    ])))
}

/// Reconstruct the immutable PoP authority for an earlier-height lane output.
///
/// A certified lane artifact intentionally retains only the union of the
/// signers in its exact Prepare/Commit QCs. A different valid quorum for the
/// same proposal may therefore select another member of the four-validator
/// committee. Its PoP comes from the cryptographically verified global
/// finality artifact which froze that proposal height's complete roster, not
/// from mutable current State.
fn durable_historical_lane_verification_pops(
    kura: &Kura,
    durable: &CertifiedLaneBlockArtifact,
) -> Result<BTreeMap<PublicKey, Vec<u8>>, String> {
    let mut pops = durable.signer_pops.clone();
    if let Some(validator_pops) = validated_autonomous_validator_pops(
        &durable.prepare_qc,
        &durable.proposal.descriptor.validator_set,
    )? {
        if durable.commit_qc.validator_set != durable.proposal.descriptor.validator_set {
            return Err("autonomous READY PoPs differ from the lane certificate roster".to_owned());
        }
        pops.extend(validator_pops);
        return Ok(pops);
    }
    let proposal_height = durable.proposal.descriptor.proposal_height;
    let Some(finality) = kura
        .v2_finality_artifact(proposal_height)
        .map_err(|error| format!("failed to read historical lane finality authority: {error}"))?
    else {
        // The exact retained QC remains independently verifiable without a
        // finality artifact. Only an alternate signer set needs the complete
        // frozen roster below.
        return Ok(pops);
    };
    let hint = durable
        .proposal
        .payload_block_hint
        .ok_or_else(|| "historical lane proposal has no canonical finality binding".to_owned())?;
    if finality.height != proposal_height
        || finality.height_context.height != proposal_height
        || hint.proposal_height != proposal_height
        || hint.proposal_block_hash != finality.block_hash
    {
        return Err(
            "historical lane proposal differs from its frozen finality authority".to_owned(),
        );
    }
    wire::finality::verify_validator_roster_pops(
        &finality.height_context,
        &finality.validator_set_pops,
    )
    .map_err(|error| format!("historical lane finality PoPs are invalid: {error}"))?;
    for (entry, pop) in finality
        .height_context
        .roster
        .iter()
        .zip(&finality.validator_set_pops)
    {
        if durable
            .proposal
            .descriptor
            .validator_set
            .contains(&entry.validator)
        {
            pops.insert(entry.validator.public_key().clone(), pop.clone());
        }
    }
    Ok(pops)
}
