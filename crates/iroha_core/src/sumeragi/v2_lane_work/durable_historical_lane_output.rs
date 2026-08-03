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
    validate_winning_lane_output(message, &durable.proposal, &durable.signer_pops)?;
    let durable_hash = HashOf::new(&durable);
    Ok(Some(Hash::new_from_chunks(&[
        b"iroha:sumeragi:v2:historical-lane-output-source:v1\0",
        durable_hash.as_ref(),
        HashOf::new(message).as_ref(),
    ])))
}
