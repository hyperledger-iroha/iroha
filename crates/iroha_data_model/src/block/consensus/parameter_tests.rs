//! Signed native consensus metadata and RS16 layout contracts.
use super::*;
use norito::codec::DecodeAll as _;
#[test]
fn consensus_modes_project_canonical_protocol_identities() {
    assert_eq!(ConsensusMode::Permissioned.tag(), PERMISSIONED_TAG);
    assert_eq!(ConsensusMode::Npos.tag(), NPOS_TAG);
    assert_eq!(
        ConsensusMode::Permissioned.bls_domain(),
        PERMISSIONED_BLS_DOMAIN
    );
    assert_eq!(ConsensusMode::Npos.bls_domain(), NPOS_BLS_DOMAIN);
    assert!(ConsensusMode::Permissioned.is_permissioned());
    assert!(!ConsensusMode::Npos.is_permissioned());
    for mode in [ConsensusMode::Permissioned, ConsensusMode::Npos] {
        let parameter_mode = crate::parameter::system::SumeragiConsensusMode::from(mode);
        assert_eq!(ConsensusMode::from(parameter_mode), mode);
    }
}
#[test]
fn genesis_context_roundtrips_and_rejects_unbound_policy() {
    let context = SumeragiGenesisContextParameters::recommended();
    assert_eq!(context.validate(), Ok(()));
    assert_eq!(
        SumeragiGenesisContextParameters::decode_all(&mut context.encode().as_slice()).unwrap(),
        context
    );
    let mut invalid = context;
    invalid.nexus_amx_context_hash = [0; 32];
    assert_eq!(
        invalid.validate(),
        Err(ValidationError::InvalidNexusAmxContextHash)
    );
    invalid = context;
    invalid.execution_policy_hash = [0; 32];
    assert_eq!(
        invalid.validate(),
        Err(ValidationError::InvalidExecutionPolicyHash)
    );
}
#[test]
fn committees_require_exact_three_f_plus_one_geometry() {
    for n in 0..=40 {
        assert_eq!(
            is_valid_committee_size(n),
            (4..=31).contains(&n) && n % 3 == 1
        );
    }
}
