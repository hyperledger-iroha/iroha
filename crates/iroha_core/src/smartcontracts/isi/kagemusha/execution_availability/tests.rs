//! Governed request precedence, local retry identity and actual rollback tests.

use super::super::{KagemushaVerifierReleaseLifecycleV1, RejectAllKagemushaV1RuntimeVerifier};
use super::*;
use crate::{
    kura::Kura,
    query::store::LiveQueryStore,
    state::{State, World},
};
use iroha_data_model::{
    Registrable,
    prelude::{Account, Domain},
};
use mv::storage::StorageReadOnly as _;
use std::collections::BTreeMap;

fn registry() -> KagemushaGovernedVerifierRegistryV1 {
    let instruction: iroha_data_model::isi::governance::ProposeKagemushaVerifierReleaseActivateV1 =
        norito::decode_canonical(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/governance/kagemusha_verifier_release_activate_v1.bin"
        )))
        .expect("original canonical threshold-authenticated activation fixture");
    instruction
        .proposal
        .successor()
        .expect("exact first activation")
}
fn network() -> NetworkId {
    NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"runtime availability test"),
    ))
}

#[test]
fn governed_eligibility_precedes_missing_local_artifacts() {
    let unavailable = RejectAllKagemushaV1RuntimeVerifier;
    let governed = registry();
    let release = governed.active_release_id.unwrap();
    for operation in [Operation::TopUp, Operation::Redemption] {
        assert_eq!(
            require(&unavailable, network(), &governed, release, operation),
            Err(Failure::Unavailable)
        );
        assert!(matches!(
            require(&unavailable, network(), &governed, [0; 32], operation),
            Err(Failure::Rejected(_))
        ));
        assert!(matches!(
            require(
                &unavailable,
                network(),
                &KagemushaGovernedVerifierRegistryV1::default(),
                release,
                operation
            ),
            Err(Failure::Rejected(_))
        ));
    }
    let mut standby = governed.clone();
    standby.active_release_id = None;
    standby.releases[0].status = iroha_data_model::kagemusha::KAGEMUSHA_RELEASE_STANDBY_V1;
    standby.validate().unwrap();
    for operation in [Operation::TopUp, Operation::Redemption] {
        assert!(matches!(
            require(&unavailable, network(), &standby, release, operation),
            Err(Failure::Rejected(_))
        ));
    }
}

#[test]
fn governed_historical_release_can_verify_but_cannot_issue() {
    let mut governed = registry();
    let old = governed.releases[0].release_id;
    governed.releases[0].status = KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1;
    let mut active = governed.releases[0].clone();
    active.release_id[0] ^= 0x80;
    active.status = KAGEMUSHA_RELEASE_ACTIVE_V1;
    governed.active_release_id = Some(active.release_id);
    governed.releases.push(active);
    governed.releases.sort_by_key(|row| row.release_id);
    governed.validate().unwrap();
    assert!(matches!(
        require(
            &RejectAllKagemushaV1RuntimeVerifier,
            network(),
            &governed,
            old,
            Operation::TopUp
        ),
        Err(Failure::Rejected(_))
    ));
    assert_eq!(
        require(
            &RejectAllKagemushaV1RuntimeVerifier,
            network(),
            &governed,
            old,
            Operation::Redemption
        ),
        Err(Failure::Unavailable)
    );
}

#[test]
fn incomplete_loaded_runtime_cannot_supply_governed_execution_authority() {
    let governed = registry();
    let runtime = AuthenticatedKagemushaV1RuntimeVerifier {
        releases: BTreeMap::new(),
        lifecycle: KagemushaVerifierReleaseLifecycleV1::default(),
    };
    for operation in [Operation::TopUp, Operation::Redemption] {
        assert_eq!(
            require(
                &runtime,
                network(),
                &governed,
                governed.active_release_id.unwrap(),
                operation
            ),
            Err(Failure::Unavailable)
        );
    }
    let mut malformed = governed;
    malformed.releases[0].native_profile_digest = [0; 32];
    assert!(matches!(
        require(
            &runtime,
            network(),
            &malformed,
            malformed.active_release_id.unwrap(),
            Operation::TopUp
        ),
        Err(Failure::Rejected(_))
    ));
}

#[test]
fn actual_transaction_retains_unavailable_owner_and_discards_caught_effects() {
    let owner = iroha_test_samples::ALICE_ID.clone();
    let world = World::with([], [Account::new(owner.clone()).build(&owner)], []);
    let governed = registry();
    let release = governed.active_release_id.unwrap();
    {
        let mut initial = world.block();
        *initial.kagemusha_verifier_registry.get_mut() = governed;
        initial.commit();
    }
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    for operation in [Operation::TopUp, Operation::Redemption] {
        let mut block = state.block(iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::MIN,
            None,
            None,
            0,
            0,
        ));
        let mut tx = block.transaction();
        let marker =
            iroha_model_base::domain::DomainId::try_new("unavailable", "universal").unwrap();
        tx.world
            .domains
            .insert(marker.clone(), Domain::new(marker.clone()).build(&owner));
        assert!(super::super::isi::require_execution_runtime(&mut tx, release, operation).is_err());
        let refusal = tx
            .execution_deferral()
            .expect("original sticky local refusal");
        assert_eq!(
            refusal.reason(),
            ivm::error::ExecutionDeferral::VerifierArtifactsUnavailable
        );
        assert!(
            refusal.allocation_refusal().is_none(),
            "no unrelated pool release is invented"
        );
        assert_eq!(tx.last_tx_gas_used, 0);
        tx.defer_execution(ivm::error::ExecutionDeferral::AllocationUnavailable);
        assert_eq!(tx.execution_deferral(), Some(refusal));
        tx.apply();
        assert!(
            block.world.domains.get(&marker).is_none(),
            "catching the local error cannot publish semantic effects"
        );
    }
}

#[test]
fn deterministic_governed_rejection_does_not_install_a_local_retry_owner() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(iroha_data_model::block::BlockHeader::new(
        std::num::NonZeroU64::MIN,
        None,
        None,
        0,
        0,
    ));
    let mut tx = block.transaction();
    assert!(
        super::super::isi::require_execution_runtime(&mut tx, [0x71; 32], Operation::TopUp)
            .is_err()
    );
    assert_eq!(tx.execution_deferral(), None);
    assert_eq!(tx.last_tx_gas_used, 0);
}
