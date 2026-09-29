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
    let mut invalid_mint_authority = context(&[1, 1, 1, 1]);
    invalid_mint_authority
        .kagemusha_mint_finality_authority
        .validators
        .clear();
    assert_eq!(
        invalid_mint_authority.validate(),
        Err(ValidationError::InvalidKagemushaMintFinalityAuthorization)
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

