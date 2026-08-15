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
                    || payload != canonical_payload
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
                if durable.executable_payload != *payload {
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
