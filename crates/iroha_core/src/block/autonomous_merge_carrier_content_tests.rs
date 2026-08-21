#[test]
fn autonomous_merge_carrier_content_gate_accepts_only_exact_empty_carrier() {
    let policy_only_block = raw_block_with_da_sidecars(None, None);
    assert!(
        policy_only_block.da_proof_policies().is_some(),
        "production-shaped signed blocks must carry mandatory DA proof-policy metadata"
    );
    assert!(
        !ValidBlock::autonomous_merge_carrier_has_da_effect(&policy_only_block),
        "mandatory DA proof-policy metadata is not an autonomous-carrier DA effect"
    );
    let commitments = raw_block_with_da_sidecars(Some(DaCommitmentBundle::default()), None);
    assert!(
        ValidBlock::autonomous_merge_carrier_has_da_effect(&commitments),
        "a DA commitment remains a forbidden autonomous-carrier effect"
    );
    let pin_intents = raw_block_with_da_sidecars(None, Some(DaPinIntentBundle::default()));
    assert!(
        ValidBlock::autonomous_merge_carrier_has_da_effect(&pin_intents),
        "a DA pin intent remains a forbidden autonomous-carrier effect"
    );
    ValidBlock::validate_autonomous_merge_carrier_content(AutonomousMergeCarrierContent::default())
        .expect("exact empty autonomous execution carrier is admissible");
    let incompatible = [
        (
            "ordinary entrypoint",
            AutonomousMergeCarrierContent {
                ordinary_entrypoints: 1,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "external context",
            AutonomousMergeCarrierContent {
                external_contexts: 1,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "DA",
            AutonomousMergeCarrierContent {
                has_da_effect: true,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "NPoS",
            AutonomousMergeCarrierContent {
                has_npos: true,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "AXT envelope",
            AutonomousMergeCarrierContent {
                has_axt_envelopes: true,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "AXT snapshot drift",
            AutonomousMergeCarrierContent {
                axt_snapshot_mismatch: true,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "autonomous lane payload",
            AutonomousMergeCarrierContent {
                autonomous_lane_payloads: 1,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "lane payload ownership",
            AutonomousMergeCarrierContent {
                lane_payload_ownerships: 1,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
        (
            "Native participant frontier",
            AutonomousMergeCarrierContent {
                has_native_participant_frontiers: true,
                ..AutonomousMergeCarrierContent::default()
            },
        ),
    ];
    for (label, content) in incompatible {
        assert!(
            matches!(
                ValidBlock::validate_autonomous_merge_carrier_content(content),
                Err(BlockValidationError::ExecutionContextInvalid(_))
            ),
            "{label} must be rejected before voting"
        );
    }
}
