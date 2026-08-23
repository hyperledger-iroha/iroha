fn autonomous_new_view_body_matches_durable_payload(
    body: &crate::lane_consensus::LaneBlockNewViewBodyV1,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    expected_network_id: iroha_data_model::NetworkId,
    expected_epoch: u64,
) -> bool {
    let origin_view = payload.origin_proposal.descriptor.lane_block_view;
    if body.from_view < origin_view {
        return false;
    }
    let source = if body.from_view == origin_view {
        payload.origin_proposal.clone()
    } else {
        let Ok(source) = crate::lane_consensus::retarget_lane_block_proposal_exact_view(
            &payload.origin_proposal,
            body.from_view,
        ) else {
            return false;
        };
        source
    };
    crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &source,
        payload,
        body.target_view,
        expected_network_id,
        expected_epoch,
    )
    .is_ok_and(|expected| expected == *body)
}
fn autonomous_lane_output_matches_payload_identity(
    message: &BlockMessage,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
) -> bool {
    match message {
        BlockMessage::LaneExecutablePayload(candidate) => {
            candidate.origin_proposal.proposal_hash == payload.origin_proposal.proposal_hash
                && candidate.payload_hash == payload.payload_hash
        }
        BlockMessage::LaneBlockNewViewVote(vote) => {
            vote.body.executable_payload_hash == payload.payload_hash
        }
        BlockMessage::LaneBlockNewViewCertificate(certificate) => {
            certificate.body.executable_payload_hash == payload.payload_hash
        }
        _ => false,
    }
}

/// Compare one producer-authenticated payload with its canonical Kura carrier.
///
/// The global block hint is advisory and is attached only after finality. Exact
/// output can therefore still own the pre-finality, hint-free representation
/// while Kura exposes the otherwise byte-identical anchored representation.
/// Promotion is deliberately one-way: an existing or conflicting hint is never
/// discarded or replaced, and every non-advisory field must remain exactly
/// equal after the payload's own authenticated attachment check.
fn autonomous_payload_matches_canonical_carrier(
    candidate: &crate::lane_consensus::LaneExecutablePayloadV1,
    canonical: &crate::lane_consensus::LaneExecutablePayloadV1,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
) -> bool {
    if candidate == canonical {
        return true;
    }
    let Some(hint) = canonical.origin_proposal.payload_block_hint else {
        return false;
    };
    candidate.origin_proposal.payload_block_hint.is_none()
        && candidate
            .attach_global_hint_exact(hint, network_id, epoch)
            .is_ok_and(|anchored| anchored == *canonical)
}
fn autonomous_lane_output_has_exact_retirement_source(
    message: &BlockMessage,
    artifact: &wire::finality::V2FinalityArtifact,
    durable_lane_authority: &DurableLaneRolloverAuthority,
    kura: &Kura,
    local_peer: &PeerId,
    proposal_height: u64,
    network_id: iroha_data_model::NetworkId,
    epoch: u64,
) -> Result<(), String> {
    let (lane_id, lane_block_height) = match message {
        BlockMessage::LaneExecutablePayload(payload) => {
            let descriptor = &payload.origin_proposal.descriptor;
            (descriptor.lane_id, descriptor.lane_block_height)
        }
        BlockMessage::LaneBlockNewViewVote(vote) => {
            (vote.body.lane_id, vote.body.lane_block_height)
        }
        BlockMessage::LaneBlockNewViewCertificate(certificate) => {
            (certificate.body.lane_id, certificate.body.lane_block_height)
        }
        _ => {
            return Err("autonomous-lane retirement claim contains another output kind".to_owned());
        }
    };
    let retired = kura
        .read_autonomous_lane_retired_attempt(
            lane_id,
            lane_block_height,
            proposal_height,
            network_id,
            epoch,
        )
        .map_err(|error| format!("autonomous-lane exact retirement validation failed: {error}"))?
        .ok_or_else(|| {
            "current autonomous-lane output has no exact durable slot retirement".to_owned()
        })?;
    let durable_payload = &retired.artifact.executable_payload;
    let durable_proposal_hash = durable_payload.origin_proposal.proposal_hash;
    let bound_supersession_source = durable_lane_authority.covered_source_hash(
        artifact,
        &BlockMessage::LaneBlockProposal(durable_payload.origin_proposal.clone()),
    )?;
    if durable_payload.origin_proposal.descriptor.proposal_height != proposal_height
        || durable_lane_authority.winning_proposal_hash(durable_proposal_hash)
        || bound_supersession_source.is_none()
    {
        return Err(
            "retired autonomous-lane output is not bound to a nonwinning finalized carrier"
                .to_owned(),
        );
    }
    if retired.retirement
        != crate::kura::AutonomousLaneSlotRetirementV1::from_payload(durable_payload)
    {
        return Err(
            "autonomous-lane output differs from its exact durable slot retirement".to_owned(),
        );
    }
    match message {
        BlockMessage::LaneExecutablePayload(payload) => {
            payload
                .validate(network_id, epoch)
                .map_err(|error| error.to_string())?;
            if payload.producer != *local_peer
                || payload.origin_proposal.descriptor.proposal_height != proposal_height
                || payload != durable_payload
                || retired.retirement
                    != crate::kura::AutonomousLaneSlotRetirementV1::from_payload(payload)
            {
                return Err(
                    "autonomous-lane payload differs from its exact retired local attempt"
                        .to_owned(),
                );
            }
        }
        BlockMessage::LaneBlockNewViewVote(vote) => {
            vote.validate_ingress().map_err(|error| error.to_string())?;
            let body = &vote.body;
            if vote.signer != *local_peer
                || body.proposal_height != proposal_height
                || !autonomous_new_view_body_matches_durable_payload(
                    body,
                    durable_payload,
                    network_id,
                    epoch,
                )
                || !retired
                    .current_proposal
                    .descriptor
                    .validator_set
                    .contains(&vote.signer)
            {
                return Err(
                    "autonomous NewView vote differs from its exact retired local attempt"
                        .to_owned(),
                );
            }
            let current_view = retired.current_proposal.descriptor.lane_block_view;
            if current_view < body.from_view {
                return Err(
                    "autonomous NewView vote is ahead of its exact retired payload cursor"
                        .to_owned(),
                );
            }
            if current_view == body.from_view {
                let expected = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
                    &retired.current_proposal,
                    durable_payload,
                    body.target_view,
                    network_id,
                    epoch,
                )
                .map_err(|error| error.to_string())?;
                if expected != *body {
                    return Err(
                        "autonomous NewView vote cannot be regenerated from the exact retired state"
                            .to_owned(),
                    );
                }
            }
        }
        BlockMessage::LaneBlockNewViewCertificate(certificate) => {
            let body = &certificate.body;
            if body.proposal_height != proposal_height
                || !autonomous_new_view_body_matches_durable_payload(
                    body,
                    durable_payload,
                    network_id,
                    epoch,
                )
                || certificate.validator_set
                    != durable_payload.origin_proposal.descriptor.validator_set
            {
                return Err(
                    "autonomous NewView certificate differs from its exact retired lane state"
                        .to_owned(),
                );
            }
            let exact_durable = retired
                .artifact
                .new_view_certificates
                .iter()
                .find(|stored| stored.certificate == *certificate)
                .or_else(|| {
                    retired
                        .artifact
                        .view_checkpoint
                        .as_ref()
                        .and_then(|checkpoint| {
                            (checkpoint.certificate.certificate == *certificate)
                                .then_some(&checkpoint.certificate)
                        })
                });
            if !certificate.validator_set.contains(local_peer) || exact_durable.is_none() {
                return Err(
                    "autonomous NewView certificate lacks an exact retired local retransmit source"
                        .to_owned(),
                );
            }
        }
        _ => {
            return Err("autonomous-lane retirement claim contains another output kind".to_owned());
        }
    }
    Ok(())
}
fn autonomous_lane_output_has_durable_reconstruction_source(
    messages: &[NetworkMessage],
    artifact: &wire::finality::V2FinalityArtifact,
    durable_lane_authority: &DurableLaneRolloverAuthority,
    kura: &Kura,
    local_peer: &PeerId,
    proposal_height: u64,
) -> Result<(), String> {
    if proposal_height == 0 || proposal_height > artifact.height {
        return Err("autonomous-lane output names an invalid durable source height".to_owned());
    }
    let historical_artifact = if proposal_height == artifact.height {
        None
    } else {
        Some(
            kura.v2_finality_artifact(proposal_height)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "historical autonomous-lane output lost its Kura finality source".to_owned()
                })?,
        )
    };
    let source_artifact = historical_artifact.as_ref().unwrap_or(artifact);
    source_artifact
        .validate()
        .map_err(|error| error.to_string())?;
    if source_artifact.height != proposal_height
        || source_artifact.height_context.network_id != artifact.height_context.network_id
    {
        return Err(
            "autonomous-lane output differs from its exact historical height context".to_owned(),
        );
    }
    let source_height = usize::try_from(proposal_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| "autonomous-lane source height is not representable".to_owned())?;
    let source_block = kura
        .get_block(source_height)
        .ok_or_else(|| "autonomous-lane output lost its canonical Kura carrier".to_owned())?;
    if source_block.hash() != source_artifact.block_hash
        || source_artifact
            .validate_for_header(&source_block.header())
            .is_err()
        || source_artifact.verify().is_err()
    {
        return Err("autonomous-lane output differs from its canonical Kura carrier".to_owned());
    }
    let network_id = source_artifact.height_context.network_id;
    let epoch = source_artifact.height_context.epoch;
    let autonomous_envelopes = source_block
        .execution_context()
        .map(|bundle| bundle.autonomous_lane_payloads.as_slice())
        .unwrap_or_default();
    let canonical_payloads = autonomous_envelopes
        .iter()
        .map(|envelope| {
            crate::lane_consensus::decode_autonomous_lane_payload_envelope(
                envelope, network_id, epoch,
            )
            .and_then(|payload| {
                payload.attach_global_hint_exact(
                    LaneBlockProposalPayloadHintV1 {
                        proposal_height,
                        proposal_view: source_block.header().view_change_index(),
                        proposal_block_hash: source_artifact.block_hash,
                    },
                    network_id,
                    epoch,
                )
            })
            .map(|payload| {
                let descriptor = &payload.origin_proposal.descriptor;
                (
                    (
                        descriptor.lane_id,
                        descriptor.dataspace_id,
                        descriptor.lane_incarnation,
                        descriptor.lane_block_height,
                    ),
                    payload,
                )
            })
            .map_err(|error| error.to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    if canonical_payloads.len() != autonomous_envelopes.len() {
        return Err("canonical Kura carrier contains duplicate autonomous lane slots".to_owned());
    }
    for message in messages {
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return Err(
                "autonomous-lane output has no durable Sumeragi transport source".to_owned(),
            );
        };
        let route = match envelope.as_message() {
            BlockMessage::LaneExecutablePayload(payload) => {
                let descriptor = &payload.origin_proposal.descriptor;
                (
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    descriptor.lane_block_height,
                )
            }
            BlockMessage::LaneBlockNewViewVote(vote) => (
                vote.body.lane_id,
                vote.body.dataspace_id,
                vote.body.lane_incarnation,
                vote.body.lane_block_height,
            ),
            BlockMessage::LaneBlockNewViewCertificate(certificate) => (
                certificate.body.lane_id,
                certificate.body.dataspace_id,
                certificate.body.lane_incarnation,
                certificate.body.lane_block_height,
            ),
            _ => {
                return Err(
                    "autonomous-lane rollover claim contains another output kind".to_owned(),
                );
            }
        };
        let Some(canonical_payload) = canonical_payloads.get(&route).filter(|payload| {
            autonomous_lane_output_matches_payload_identity(envelope.as_message(), payload)
        }) else {
            if proposal_height != artifact.height {
                return Err(
                    "autonomous-lane output is not owned by the canonical Kura carrier".to_owned(),
                );
            }
            autonomous_lane_output_has_exact_retirement_source(
                envelope.as_message(),
                artifact,
                durable_lane_authority,
                kura,
                local_peer,
                proposal_height,
                network_id,
                epoch,
            )?;
            continue;
        };
        let proposal_hash = canonical_payload.origin_proposal.proposal_hash;
        if proposal_height == artifact.height
            && !durable_lane_authority.winning_proposal_hash(proposal_hash)
        {
            return Err(
                "autonomous-lane output is not owned by the finalized winning carrier".to_owned(),
            );
        }
        match envelope.as_message() {
            BlockMessage::LaneExecutablePayload(payload) => {
                payload
                    .validate(network_id, epoch)
                    .map_err(|error| error.to_string())?;
                let descriptor = &payload.origin_proposal.descriptor;
                if payload.producer != *local_peer
                    || descriptor.proposal_height != proposal_height
                    || !autonomous_payload_matches_canonical_carrier(
                        payload,
                        canonical_payload,
                        network_id,
                        epoch,
                    )
                {
                    return Err(
                        "autonomous-lane payload lacks the successor's local retransmit authority"
                            .to_owned(),
                    );
                }
                let durable = kura
                    .read_autonomous_lane_block_artifact(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                        network_id,
                        epoch,
                    )
                    .ok_or_else(|| {
                        "autonomous-lane payload has no durable reconstruction artifact".to_owned()
                    })?;
                if !autonomous_payload_matches_canonical_carrier(
                    &durable.executable_payload,
                    canonical_payload,
                    network_id,
                    epoch,
                ) {
                    return Err(
                        "autonomous-lane payload differs from its durable reconstruction artifact"
                            .to_owned(),
                    );
                }
            }
            BlockMessage::LaneBlockNewViewVote(vote) => {
                vote.validate_ingress().map_err(|error| error.to_string())?;
                let body = &vote.body;
                let (payload, current) = kura
                    .current_autonomous_lane_payload(
                        body.lane_id,
                        body.lane_block_height,
                        network_id,
                        epoch,
                    )
                    .ok_or_else(|| {
                        "autonomous NewView vote has no durable payload cursor".to_owned()
                    })?;
                if vote.signer != *local_peer
                    || !autonomous_new_view_body_matches_durable_payload(
                        body, &payload, network_id, epoch,
                    )
                    || body.proposal_height != proposal_height
                    || payload != *canonical_payload
                    || !current.descriptor.validator_set.contains(&vote.signer)
                {
                    return Err(
                        "autonomous NewView vote differs from its durable payload cursor"
                            .to_owned(),
                    );
                }
                let current_view = current.descriptor.lane_block_view;
                if current_view < body.from_view {
                    return Err(
                        "autonomous NewView vote is ahead of its durable payload cursor".to_owned(),
                    );
                }
                if current_view == body.from_view {
                    let expected = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
                        &current,
                        &payload,
                        body.target_view,
                        network_id,
                        epoch,
                    )
                    .map_err(|error| error.to_string())?;
                    if expected != *body {
                        return Err(
                            "autonomous NewView vote cannot be regenerated from durable state"
                                .to_owned(),
                        );
                    }
                }
            }
            BlockMessage::LaneBlockNewViewCertificate(certificate) => {
                let body = &certificate.body;
                let durable = kura
                    .read_autonomous_lane_block_artifact(
                        body.lane_id,
                        body.lane_block_height,
                        network_id,
                        epoch,
                    )
                    .ok_or_else(|| {
                        "autonomous NewView certificate has no durable payload cursor".to_owned()
                    })?;
                let payload = &durable.executable_payload;
                if !autonomous_new_view_body_matches_durable_payload(
                    body, payload, network_id, epoch,
                ) || body.proposal_height != proposal_height
                    || payload != canonical_payload
                    || certificate.validator_set != payload.origin_proposal.descriptor.validator_set
                {
                    return Err(
                        "autonomous NewView certificate differs from durable lane state".to_owned(),
                    );
                }
                let exact_durable = durable
                    .new_view_certificates
                    .iter()
                    .find(|stored| stored.certificate == *certificate)
                    .or_else(|| {
                        durable.view_checkpoint.as_ref().and_then(|checkpoint| {
                            (checkpoint.certificate.certificate == *certificate)
                                .then_some(&checkpoint.certificate)
                        })
                    });
                if !certificate.validator_set.contains(local_peer) || exact_durable.is_none() {
                    return Err(
                        "autonomous NewView certificate lacks an exact durable local retransmit source"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "autonomous-lane rollover claim contains another output kind".to_owned(),
                );
            }
        }
    }
    Ok(())
}
fn payload_chunk_output_has_applied_height_authority(
    messages: &[NetworkMessage],
    manifest: &wire::PayloadManifest,
    artifact: &wire::finality::V2FinalityArtifact,
) -> Result<(), String> {
    let context = &artifact.height_context;
    manifest.validate(context).map_err(|error| {
        format!("payload-chunk rollover manifest is invalid for the applied context: {error}")
    })?;
    let manifest_hash = HashOf::new(manifest);
    if messages.len() != manifest.chunk_hashes.len() {
        return Err("payload-chunk rollover changed the exact chunk count".to_owned());
    }
    for (expected_index, message) in messages.iter().enumerate() {
        if message.progress_reconstruction() != ProgressReconstruction::Retransmit {
            return Err(
                "payload-chunk rollover contains non-reconstructible transport traffic".to_owned(),
            );
        }
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return Err("payload-chunk rollover contains non-Sumeragi traffic".to_owned());
        };
        let BlockMessage::V2(message) = envelope.as_message() else {
            return Err("payload-chunk rollover contains lane traffic".to_owned());
        };
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload else {
            return Err("payload-chunk rollover contains another v2 payload".to_owned());
        };
        if chunk.manifest_hash != manifest_hash
            || usize::try_from(chunk.index).ok() != Some(expected_index)
        {
            return Err(
                "payload-chunk rollover differs from its exact manifest coordinates".to_owned(),
            );
        }
        chunk.validate(context, manifest).map_err(|error| {
            format!("payload-chunk rollover is invalid for its exact manifest: {error}")
        })?;
        let sender_index = usize::try_from(chunk.sender)
            .map_err(|_| "payload-chunk rollover sender is not representable".to_owned())?;
        let sender = context.roster.get(sender_index).ok_or_else(|| {
            "payload-chunk rollover sender is outside the applied roster".to_owned()
        })?;
        let preimage = chunk
            .signature_preimage(context, manifest)
            .map_err(|error| error.to_string())?;
        Signature::try_from_bytes(&chunk.signature)
            .map_err(|error| format!("payload-chunk rollover has an invalid signature: {error}"))?
            .verify(sender.validator.public_key(), &preimage)
            .map_err(|error| {
                format!("payload-chunk rollover signature is not owned by its sender: {error}")
            })?;
    }
    Ok(())
}
fn applied_height_reconstruction_covers(
    messages: &[NetworkMessage],
    peers: &[PeerId],
    rollover_claim: &ExactOutputRolloverClaim,
    artifact: &wire::finality::V2FinalityArtifact,
    durable_lane_authority: Option<&DurableLaneRolloverAuthority>,
    durable_history: Option<&Kura>,
) -> Result<(), String> {
    rollover_claim.validate_fanout(messages, peers)?;
    if matches!(
        rollover_claim,
        ExactOutputRolloverClaim::NonRetireableLaneTransport { .. }
    ) {
        return Err(
            "non-retireable lane transport must drain before applied-height handoff".to_owned(),
        );
    }
    let scope = rollover_claim.scope().ok_or_else(|| {
        "Sumeragi v2 exact output has no typed applied-height rollover claim".to_owned()
    })?;
    if !scope.covers(artifact) {
        return Err("Sumeragi v2 output claim belongs to another creation scope".to_owned());
    }
    if let ExactOutputRolloverClaim::PayloadChunks { manifest, .. } = rollover_claim {
        return payload_chunk_output_has_applied_height_authority(messages, manifest, artifact);
    }
    if let ExactOutputRolloverClaim::AutonomousLane {
        local_peer,
        proposal_height,
        ..
    } = rollover_claim
    {
        return autonomous_lane_output_has_durable_reconstruction_source(
            messages,
            artifact,
            durable_lane_authority.ok_or_else(|| {
                "autonomous-lane output lacks finalized winning-lane authority".to_owned()
            })?,
            durable_history.ok_or_else(|| {
                "autonomous-lane output lacks an independently readable Kura source".to_owned()
            })?,
            local_peer,
            *proposal_height,
        );
    }
    if matches!(
        rollover_claim,
        ExactOutputRolloverClaim::QueuePlanAdmission { .. }
    ) {
        return queue_plan_admission_reconstruction_covers(
            messages,
            rollover_claim,
            artifact,
            durable_history,
        );
    }
    if matches!(
        rollover_claim,
        ExactOutputRolloverClaim::DurableCommitCertificateResponse { .. }
            | ExactOutputRolloverClaim::DurableCertifiedBodyResponse { .. }
            | ExactOutputRolloverClaim::DurableLaneCertificateResponse { .. }
            | ExactOutputRolloverClaim::HistoricalLaneCertification { .. }
            | ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse { .. }
    ) {
        return durable_history_source_covers(
            messages,
            rollover_claim,
            &artifact.height_context.network_id,
            artifact.height,
            durable_history.ok_or_else(|| {
                "Sumeragi v2 durable response lacks an independently readable history source"
                    .to_owned()
            })?,
        );
    }
    if let ExactOutputRolloverClaim::DurableKuraReplicaAdvert { source_height, .. } = rollover_claim
    {
        let [NetworkMessage::SumeragiBlock(envelope)] = messages else {
            return Err("durable Kura replica advert rollover lost its exact message".to_owned());
        };
        let BlockMessage::KuraReplicaAdvert(advert) = envelope.as_message() else {
            return Err("durable Kura replica advert rollover changed output kind".to_owned());
        };
        if *source_height > artifact.height {
            return Err(
                "durable Kura replica advert belongs to a future applied height".to_owned(),
            );
        }
        let expected_peers = artifact
            .height_context
            .roster
            .iter()
            .filter(|entry| entry.validator != advert.keeper)
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        if peers != expected_peers.as_slice() {
            return Err("durable Kura replica advert changed its frozen roster fanout".to_owned());
        }
        durable_history
            .ok_or_else(|| {
                "durable Kura replica advert lacks an independently readable Kura source".to_owned()
            })?
            .revalidate_kura_replica_advert_source(advert)
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    if matches!(
        rollover_claim,
        ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest { .. }
            | ExactOutputRolloverClaim::NativeAmx { .. }
            | ExactOutputRolloverClaim::LaneDrainVote { .. }
            | ExactOutputRolloverClaim::MergeShare { .. }
            | ExactOutputRolloverClaim::CertifiedSidecarRequest { .. }
            | ExactOutputRolloverClaim::CertifiedSidecarControl { .. }
            | ExactOutputRolloverClaim::CertifiedSidecarChunk { .. }
    ) {
        return Ok(());
    }
    let context_id = artifact.context_id();
    let height = artifact.height;
    let round_matches =
        |round: wire::ConsensusRound| round.context_id == context_id && round.height == height;
    let lane_output_is_covered = |lane_message: &BlockMessage| -> Result<bool, String> {
        let authority = durable_lane_authority.ok_or_else(|| {
            "Sumeragi v2 lane output lacks a typed durable rollover authority".to_owned()
        })?;
        if authority
            .covered_source_hash(artifact, lane_message)?
            .is_some()
        {
            return Ok(true);
        }
        let Some((proposal_height, _)) = lane_output_identity(lane_message) else {
            return Ok(false);
        };
        if proposal_height >= artifact.height {
            return Ok(false);
        }
        durable_historical_lane_output_source_hash(
            durable_history.ok_or_else(|| {
                "historical lane output lacks an independently readable Kura source".to_owned()
            })?,
            lane_message,
        )
        .map(|source| source.is_some())
    };
    for message in messages {
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            return Err(
                "Sumeragi v2 exact output has no applied-height reconstruction source".to_owned(),
            );
        };
        match envelope.as_message() {
            BlockMessage::V2(message)
                if matches!(rollover_claim, ExactOutputRolloverClaim::GlobalV2(_)) =>
            {
                message
                    .validate_version()
                    .map_err(|error| error.to_string())?;
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::PayloadChunk(_)
                ) {
                    return Err(
                        "Sumeragi v2 payload chunks require an exact manifest rollover claim"
                            .to_owned(),
                    );
                }
            }
            lane_message @ (BlockMessage::LaneBlockProposal(_)
            | BlockMessage::LaneBlockVote(_)
            | BlockMessage::LaneBlockQc(_)
            | BlockMessage::LaneBlockCertificate(_))
                if matches!(rollover_claim, ExactOutputRolloverClaim::Lane(_)) =>
            {
                if !lane_output_is_covered(lane_message)? {
                    return Err(
                        "Sumeragi v2 lane output lacks an exact typed durable rollover witness"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Sumeragi v2 lane or legacy output lacks a typed durable rollover witness"
                        .to_owned(),
                );
            }
        }
    }
    for message in messages {
        if message.progress_reconstruction() != ProgressReconstruction::Retransmit {
            return Err(
                "Sumeragi v2 exact output has no applied-height reconstruction source".to_owned(),
            );
        }
        let NetworkMessage::SumeragiBlock(envelope) = message else {
            unreachable!("global rollover preflight rejected non-Sumeragi output")
        };
        let covered = match envelope.as_message() {
            BlockMessage::V2(message)
                if matches!(rollover_claim, ExactOutputRolloverClaim::GlobalV2(_)) =>
            {
                match &message.payload {
                    wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                        round_matches(proposal.round)
                    }
                    wire::ConsensusMessageV2Payload::Vote(vote) => round_matches(vote.round),
                    wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                        round_matches(certificate.round)
                    }
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote) => round_matches(vote.round),
                    wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => {
                        round_matches(certificate.round)
                    }
                    wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => {
                        round_matches(manifest.round)
                    }
                    wire::ConsensusMessageV2Payload::PayloadChunk(_) => false,
                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => {
                        round_matches(request.round)
                    }
                    wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) => {
                        round_matches(response.manifest.round)
                    }
                    wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) => {
                        request.context_id == context_id && request.height == height
                    }
                    wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
                        round_matches(response.certificate.round)
                    }
                    wire::ConsensusMessageV2Payload::VrfCommit(commit) => {
                        commit.epoch == artifact.height_context.epoch
                    }
                    wire::ConsensusMessageV2Payload::VrfReveal(reveal) => {
                        reveal.epoch == artifact.height_context.epoch
                    }
                }
            }
            lane_message @ (BlockMessage::LaneBlockProposal(_)
            | BlockMessage::LaneBlockVote(_)
            | BlockMessage::LaneBlockQc(_)
            | BlockMessage::LaneBlockCertificate(_))
                if matches!(rollover_claim, ExactOutputRolloverClaim::Lane(_)) =>
            {
                lane_output_is_covered(lane_message)?
            }
            _ => unreachable!("rollover preflight rejected an untyped block output"),
        };
        if !covered {
            return Err(
                "Sumeragi v2 output is not bound to the applied height authority".to_owned(),
            );
        }
    }
    Ok(())
}
#[cfg(test)]
pub(in crate::sumeragi) enum ExactOutputTestAdmission {
    /// Simulate a completed non-sidecar actor transfer.
    Admitted,
    /// Retain a sidecar response until the supplied writer completion resolves.
    SidecarFlush(NetworkReplyFlushAck),
    /// Simulate the tenure-cancellation race with no actor ownership.
    Retired,
}
/// Test-only RAII hold for one auxiliary physical I/O admission unit.
///
/// The hold changes only the shared admission counter. Dropping it releases
/// the exact unit and advances the ordinary lifecycle capacity generation.
#[cfg(test)]
#[must_use = "the auxiliary I/O admission hold must remain live for the intended test cut"]
pub(in crate::sumeragi) struct ProductionAuxiliaryIoAdmissionHoldV1 {
    admission: Arc<V2IoAdmission>,
}

#[cfg(test)]
impl Drop for ProductionAuxiliaryIoAdmissionHoldV1 {
    fn drop(&mut self) {
        self.admission.release();
    }
}

#[cfg(test)]
type ExactOutputAdmissionHook = Box<
    dyn FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        )
            -> Result<ExactOutputTestAdmission, NetworkActorAdmissionError<Post<NetworkMessage>>>
        + Send,
>;
