//! Closed native-authority retention uses actual validated material capacities.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::sumeragi::epoch::{MAX_VALIDATORS, ValidatorCommitteeMemberV1};

fn maximum_epoch() -> ValidatorEpochContextV1 {
    let mut config = TestChainConfig::new(World::new(), 10_000);
    config.validator_keys = Some(
        (0..MAX_VALIDATORS)
            .map(|index| KeyPair::from_seed(vec![index as u8; 32], Algorithm::BlsNormal))
            .collect(),
    );
    let prepared = CertifiedTestChain::prepare(config).unwrap();
    crate::sumeragi::epoch::authenticated_genesis(prepared.genesis.block())
        .unwrap()
        .into_parts()
        .0
}

#[test]
fn maximum_native_authority_fits_typed_retention_and_exact_boundary_refuses() {
    let epoch = maximum_epoch();
    assert_eq!(epoch.committee.len(), MAX_VALIDATORS);
    let maximum = native_finality_authority_bytes().unwrap();
    let actual = authority_owned_bytes(&epoch).unwrap();
    eprintln!("native authority maximum={maximum}, actual31={actual}");
    assert!(actual <= maximum);
    checked_authority_retention(&epoch, actual).unwrap();
    assert!(matches!(
        checked_authority_retention(&epoch, actual - 1),
        Err(ExecutionAttemptError::Deferred(_))
    ));
    let authority = VerifiedAuthority::new_query(epoch, 1, &mut EpochValidationScope::new())
        .expect("all 31 original BLS keys and proofs pass the ordinary native constructor");
    assert_eq!(authority.committee.members().len(), MAX_VALIDATORS);
    assert!(authority_owned_bytes(&authority.material).unwrap() <= maximum);
}

#[test]
fn validated_authority_cannot_escape_with_unbudgeted_material_capacity() {
    let epoch = maximum_epoch();
    let maximum = native_finality_authority_bytes().unwrap();
    for oversized_committee in [true, false] {
        let mut material = epoch.clone();
        if oversized_committee {
            material
                .committee
                .reserve_exact(maximum / std::mem::size_of::<ValidatorCommitteeMemberV1>() + 1);
        } else {
            material.committee[0]
                .proof_of_possession
                .reserve_exact(maximum);
        }
        assert_eq!(material, epoch, "only actual allocation capacity changed");
        material.validate().unwrap();
        assert!(authority_owned_bytes(&material).unwrap() > maximum);
        assert!(matches!(
            VerifiedAuthority::new_query(material, 1, &mut EpochValidationScope::new()),
            Err(ExecutionAttemptError::Deferred(_))
        ));
    }
}

#[test]
fn narrowed_retention_preserves_bad_pop_and_oversized_roster_rejections() {
    let epoch = maximum_epoch();
    let mut bad_pop = epoch.clone();
    bad_pop.committee[0].proof_of_possession[0] ^= 1;
    bad_pop.committee[0]
        .proof_of_possession
        .reserve_exact(native_finality_authority_bytes().unwrap());
    assert!(matches!(
        VerifiedAuthority::new_query(bad_pop, 1, &mut EpochValidationScope::new()),
        Err(ExecutionAttemptError::Rejected(
            ChainReadError::Committee { .. }
        ))
    ));
    let mut too_many = epoch;
    too_many.committee.push(too_many.committee[0].clone());
    assert_eq!(too_many.committee.len(), MAX_VALIDATORS + 1);
    assert!(matches!(
        VerifiedAuthority::new_query(too_many, 1, &mut EpochValidationScope::new()),
        Err(ExecutionAttemptError::Rejected(
            ChainReadError::Committee { .. }
        ))
    ));
}
