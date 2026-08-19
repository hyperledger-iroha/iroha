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
                        "historical lane output has no certification identity"
                            .to_owned(),
                    );
                }
            };
            return Ok(
                ExactOutputRolloverClaim::HistoricalLaneCertification {
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
