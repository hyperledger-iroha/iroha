impl ProductionV2Services {
    fn current_lane_output_rollover_claim(
        &self,
        message: &BlockMessage,
        target: &PeerId,
    ) -> Result<ExactOutputRolloverClaim, String> {
        let autonomous_identity = match message {
            BlockMessage::LaneExecutablePayload(payload) => Some((
                payload.origin_proposal.descriptor.proposal_height,
                payload.producer == self.local_peer,
            )),
            BlockMessage::LaneBlockNewViewVote(vote) => {
                Some((vote.body.proposal_height, vote.signer == self.local_peer))
            }
            BlockMessage::LaneBlockNewViewCertificate(certificate) => Some((
                certificate.body.proposal_height,
                certificate.validator_set.contains(&self.local_peer),
            )),
            _ => None,
        };
        if let Some((proposal_height, true)) = autonomous_identity
            && proposal_height > 0
            && proposal_height <= self.context.height
        {
            return Ok(ExactOutputRolloverClaim::AutonomousLane {
                scope: self.exact_output_scope(),
                local_peer: self.local_peer.clone(),
                proposal_height,
            });
        }
        if autonomous_identity.is_some() {
            return Ok(ExactOutputRolloverClaim::NonRetireableLaneTransport {
                target: target.clone(),
                message_hash: HashOf::new(message),
            });
        }
        let Some((proposal_height, proposal_hash)) = lane_output_identity(message) else {
            return Err("Sumeragi v2 lane output has no typed lane identity".to_owned());
        };
        if proposal_height > self.context.height {
            return Err(format!(
                "Sumeragi v2 lane output proposal height {proposal_height} is ahead of immutable height context {}",
                self.context.height
            ));
        }
        if proposal_height < self.context.height {
            let (lane_id, lane_block_height) = match message {
                BlockMessage::LaneBlockProposal(proposal) => (
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                ),
                BlockMessage::LaneBlockVote(vote) => {
                    (vote.body.lane_id, vote.body.lane_block_height)
                }
                BlockMessage::LaneBlockQc(qc) => (qc.body.lane_id, qc.body.lane_block_height),
                BlockMessage::LaneBlockCertificate(certificate) => (
                    certificate.proposal.descriptor.lane_id,
                    certificate.proposal.descriptor.lane_block_height,
                ),
                _ => {
                    return Err(
                        "historical autonomous lane output has no certification identity"
                            .to_owned(),
                    );
                }
            };
            return Ok(
                ExactOutputRolloverClaim::HistoricalAutonomousLaneCertification {
                    scope: self.exact_output_scope(),
                    target: target.clone(),
                    source_height: proposal_height,
                    lane_id,
                    lane_block_height,
                    proposal_hash,
                    message_hash: HashOf::new(message),
                },
            );
        }
        Ok(ExactOutputRolloverClaim::Lane(self.exact_output_scope()))
    }
}
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
