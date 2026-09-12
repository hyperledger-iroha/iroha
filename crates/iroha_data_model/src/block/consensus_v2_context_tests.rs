//! Height-context roster, availability and parent-commit validation.

use super::*;

#[test]
fn height_context_rejects_noncanonical_rosters_and_quorums() {
    let mut empty = context(&[1, 1, 1, 1]);
    empty.roster.clear();
    assert_eq!(empty.leader(u64::MAX), 0);
    assert_eq!(
        DualQuorum::from_roster(&empty.roster),
        Err(ValidationError::EmptyRoster)
    );
    assert_eq!(empty.validate(), Err(ValidationError::EmptyRoster));
    let mut too_small = context(&[1, 1, 1, 1]);
    too_small.roster.truncate(MIN_VALIDATORS_PER_HEIGHT - 1);
    assert_eq!(
        DualQuorum::from_roster(&too_small.roster),
        Err(ValidationError::RosterTooSmall)
    );
    assert_eq!(too_small.validate(), Err(ValidationError::RosterTooSmall));
    let mut invalid_geometry = context(&[1, 1, 1, 1]);
    invalid_geometry.roster.push(ValidatorPower {
        validator: peer(0xFE),
        power: 1,
    });
    invalid_geometry.roster.sort();
    assert_eq!(
        DualQuorum::from_roster(&invalid_geometry.roster),
        Err(ValidationError::InvalidCommitteeGeometry)
    );
    assert_eq!(
        invalid_geometry.validate(),
        Err(ValidationError::InvalidCommitteeGeometry)
    );
    let mut invalid = context(&[1, 1, 1, 1]);
    invalid.roster[1].validator = invalid.roster[0].validator.clone();
    assert_eq!(
        DualQuorum::from_roster(&invalid.roster),
        Err(ValidationError::DuplicateValidator)
    );
    assert_eq!(invalid.validate(), Err(ValidationError::DuplicateValidator));
    let mut invalid_mint_roster = context(&[1, 1, 1, 1]);
    invalid_mint_roster
        .kagemusha_mint_finality_epoch_roster
        .validators
        .clear();
    assert_eq!(
        invalid_mint_roster.validate(),
        Err(ValidationError::InvalidKagemushaMintFinalityEpochRoster)
    );
    let mut invalid = context(&[1, 1, 1, 1]);
    invalid.quorum.min_signers = 2;
    assert_eq!(
        invalid.validate(),
        Err(ValidationError::CountThresholdMismatch)
    );
    let mut oversized = context(&[1, 1, 1, 1]);
    let repeated = oversized.roster[0].clone();
    oversized
        .roster
        .resize(MAX_VALIDATORS_PER_HEIGHT + 1, repeated);
    assert_eq!(
        DualQuorum::from_roster(&oversized.roster),
        Err(ValidationError::RosterTooLarge)
    );
    assert_eq!(oversized.validate(), Err(ValidationError::RosterTooLarge));
    let largest = context(&vec![1; MAX_VALIDATORS_PER_HEIGHT]);
    assert_eq!(largest.validate(), Ok(()));
}

#[test]
fn height_context_rejects_invalid_da_capacity() {
    let mut odd_rs16_symbols = context(&[1, 1, 1, 1]);
    odd_rs16_symbols.da_layout.chunk_size_bytes = 3;
    assert_eq!(
        odd_rs16_symbols.validate(),
        Err(ValidationError::InvalidDataAvailabilityLayout)
    );
    let mut insufficient_chunk_capacity = context(&[1, 1, 1, 1]);
    insufficient_chunk_capacity.da_layout.max_chunk_count -= 1;
    assert_eq!(
        insufficient_chunk_capacity.validate(),
        Err(ValidationError::InvalidDataAvailabilityLayout)
    );
}

#[test]
fn height_context_rejects_invalid_parent_execution_commitment() {
    let mut invalid_parent_execution = context(&[1, 1, 1, 1]);
    invalid_parent_execution.height = 2;
    let invalid_parent_round = ConsensusRound {
        context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"invalid parent execution context",
        ))),
        height: 1,
        view: 0,
    };
    let invalid_parent_executed_block_wire = b"executed block wire";
    invalid_parent_execution.parent_commit_qc = Some(QuorumCertificate {
        round: invalid_parent_round,
        proposal_round: invalid_parent_round,
        phase: GlobalPhase::Commit,
        subject: subject(0x61),
        execution_commitment: ExecutionCommitment {
            parent_state_root: Hash::new(b"parent state"),
            post_state_root: Hash::new(b"post state"),
            ordinary_writes_root: Hash::new(b"ordinary writes"),
            kagemusha_top_up_root: None,
            kagemusha_top_up_count: 1,
            native_amx_application_manifest_version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            native_amx_application_manifest_root: native_amx_application_manifest_empty_root(),
            native_amx_application_manifest_count: 0,
            lane_finality_manifest: None,
            merge_carrier: None,
            executed_block_wire_len: u64::try_from(invalid_parent_executed_block_wire.len())
                .expect("fixture wire length fits u64"),
            executed_block_wire_hash: Hash::new(invalid_parent_executed_block_wire),
        },
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0x62; 48],
    });
    assert_eq!(
        invalid_parent_execution.validate(),
        Err(ValidationError::InvalidExecutionCommitment)
    );
}
