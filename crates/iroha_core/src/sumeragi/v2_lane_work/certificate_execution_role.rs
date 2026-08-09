fn validate_winning_lane_qc(
    qc: &LaneBlockQcV1,
    proposal: &LaneBlockProposalV1,
    signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), String> {
    if !matches!(qc.body.phase, CertPhase::Prepare | CertPhase::Commit)
        || qc.body != proposal.vote_body(qc.body.phase)
        || qc.validator_set != proposal.descriptor.validator_set
    {
        return Err("winning lane QC differs from the exact durable proposal".to_owned());
    }
    validate_lane_block_qc_aggregate(qc, signer_pops).map_err(|error| error.to_string())
}

/// Bind the certificate's execution role to the kind of canonical anchor.
///
/// READY is the immutable autonomous-role marker. A missing autonomous anchor
/// must not redirect that certificate into ordinary direct-receipt recovery,
/// and an ordinary certificate must not borrow an autonomous payload anchor.
fn require_lane_certificate_execution_role_matches_anchor(
    prepare_qc: &LaneBlockQcV1,
    autonomous_anchor: bool,
) -> Result<bool, V2LaneWorkError> {
    let autonomous_certificate = prepare_qc.payload_availability_qc.is_some();
    if autonomous_certificate != autonomous_anchor {
        return Err(V2LaneWorkError::Persistence(
            "lane certificate execution role differs from its canonical carrier anchor"
                .to_owned(),
        ));
    }
    Ok(autonomous_certificate)
}
