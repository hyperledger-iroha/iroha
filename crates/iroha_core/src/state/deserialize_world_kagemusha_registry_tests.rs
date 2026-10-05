//! First-release snapshot custody of finalized KAGEMUSHA verifier release authority.

use super::*;
use crate::query::store::LiveQueryStore;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_RELEASE_ACTIVE_V1, KAGEMUSHA_RELEASE_STANDBY_V1, KAGEMUSHA_WIRE_VERSION_V1,
    KagemushaGovernedVerifierRegistryV1, KagemushaGovernedVerifierReleaseV1,
    KagemushaReleaseAuthorityPolicyV1,
};
use std::{num::NonZeroU64, sync::atomic::Ordering};

fn governed_policy() -> KagemushaReleaseAuthorityPolicyV1 {
    let signer = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519)
        .expect("deterministic release authority");
    KagemushaReleaseAuthorityPolicyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_set_id: [0x72; 32],
        threshold: 1,
        authorized_signers: vec![signer.public_key().clone()],
    }
}

fn active_registry() -> KagemushaGovernedVerifierRegistryV1 {
    let policy = governed_policy();
    let policy_digest = policy.canonical_digest().unwrap();
    let mut governed = KagemushaGovernedVerifierRegistryV1::default();
    governed.initialize_authority_policy(policy).unwrap();
    governed.active_release_id = Some([0x74; 32]);
    governed.releases.push(KagemushaGovernedVerifierReleaseV1 {
        release_id: [0x74; 32],
        status: KAGEMUSHA_RELEASE_ACTIVE_V1,
        profile_digest: [2; 32],
        artifact_manifest_digest: [3; 32],
        receipt_digest: [4; 32],
        attestation_digest: [5; 32],
        authority_policy_digest: policy_digest,
        hardware_policy_digest: [6; 32],
        native_profile_digest: [7; 32],
        provider_policy_root: [8; 32],
        suite_id: [9; 32],
        vk_set_digest: [10; 32],
    });
    governed.validate().unwrap();
    governed
}

fn standby_registry() -> KagemushaGovernedVerifierRegistryV1 {
    let (mut registry, manifest, receipt, attestation) =
        crate::smartcontracts::isi::kagemusha::release_evidence_tests::release_evidence();
    registry
        .install_authenticated_release(&manifest, &receipt, &attestation)
        .expect("threshold-authenticated inactive standby");
    registry
}

fn first_header() -> BlockHeader {
    BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0)
}

fn set_registry(world: &World, registry: KagemushaGovernedVerifierRegistryV1) {
    let mut block = world.block();
    *block.kagemusha_verifier_registry.get_mut() = registry;
    block.commit();
}

fn restore(world: &World) -> Result<World, json::Error> {
    let encoded = json::to_json(world).unwrap();
    let ivm = IVM::new(0);
    let operation_index_budget = crate::state::kagemusha_operation_indexes::default_budget();
    let operation_index_refusal = std::cell::RefCell::new(None);
    let seed = IvmSeed {
        operation_index_budget: &operation_index_budget,
        operation_index_refusal: &operation_index_refusal,
        ivm: &ivm,
        _marker: PhantomData,
    };
    parse_world(
        &iroha_allocation::AllocationBudget::new(
            iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
        ),
        SnapshotJsonMap::parse(&encoded, "world")?,
        &seed,
    )
    .map_err(crate::state::deserialize::snapshot_format_error_for_test)
}

#[test]
fn snapshot_roundtrip_keeps_current_and_actual_predecessor_authority() {
    let world = World::default();
    let mut predecessor = KagemushaGovernedVerifierRegistryV1::default();
    predecessor
        .initialize_authority_policy(governed_policy())
        .unwrap();
    set_registry(&world, predecessor.clone());
    let mut current = predecessor.clone();
    current.authority_policy.as_mut().unwrap().authority_set_id = [0x73; 32];
    current.validate().unwrap();
    set_registry(&world, current.clone());
    let restored = restore(&world).unwrap();
    assert_eq!(*restored.kagemusha_verifier_registry.view().get(), current);
    assert_eq!(
        *restored
            .kagemusha_verifier_registry
            .block_and_revert()
            .get(),
        predecessor
    );
}

#[test]
fn snapshot_roundtrip_preserves_inactive_standby_and_reject_all_recovery() {
    let world = World::default();
    let standby = standby_registry();
    let mut predecessor = standby.clone();
    predecessor.releases.clear();
    set_registry(&world, predecessor.clone());
    set_registry(&world, standby.clone());
    let restored = restore(&world).expect("standby snapshot is canonical");
    assert_eq!(*restored.kagemusha_verifier_registry.view().get(), standby);
    assert_eq!(
        *restored
            .kagemusha_verifier_registry
            .block_and_revert()
            .get(),
        predecessor
    );
    let state = State::new_for_testing(
        restored,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        !state
            .kagemusha_v1_runtime_reload_head()
            .has_active_release()
    );
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("standby alone cannot require an active local verifier");
    state
        .block(first_header())
        .commit_empty_block_for_testing()
        .expect("unchanged standby registry remains fail-closed");
}

#[test]
fn snapshot_roundtrip_preserves_certified_activation_and_standby_predecessor() {
    let world = World::default();
    let standby = standby_registry();
    let release_id = standby.releases[0].release_id;
    set_registry(&world, standby.clone());
    let mut active = standby.clone();
    active
        .activate_standby(None, release_id)
        .expect("first exact activation");
    set_registry(&world, active.clone());
    let restored = restore(&world).expect("active snapshot is canonical");
    assert_eq!(*restored.kagemusha_verifier_registry.view().get(), active);
    assert_eq!(
        *restored
            .kagemusha_verifier_registry
            .block_and_revert()
            .get(),
        standby
    );
    let state = State::new_for_testing(
        restored,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        state
            .kagemusha_v1_runtime_reload_head()
            .has_active_release()
    );
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("active authority replays with reject-all until local reload");
    state
        .block(first_header())
        .commit_empty_block_for_testing()
        .expect("unchanged active authority remains fail-closed");
}

#[test]
fn missing_first_release_registry_field_is_rejected() {
    let encoded = json::to_json(&World::default()).unwrap();
    let mut map = SnapshotJsonMap::parse(&encoded, "world").unwrap();
    assert!(map.remove("kagemusha_verifier_registry").is_some());
    let ivm = IVM::new(0);
    let operation_index_budget = crate::state::kagemusha_operation_indexes::default_budget();
    let operation_index_refusal = std::cell::RefCell::new(None);
    let seed = IvmSeed {
        operation_index_budget: &operation_index_budget,
        operation_index_refusal: &operation_index_refusal,
        ivm: &ivm,
        _marker: PhantomData,
    };
    let error = parse_world(
        &iroha_allocation::AllocationBudget::new(
            iroha_config::parameters::defaults::pipeline::IVM_EXECUTION_MAX_BYTES,
        ),
        map,
        &seed,
    )
    .map_err(crate::state::deserialize::snapshot_format_error_for_test)
    .err()
    .expect("missing authority must fail");
    assert!(
        error.to_string().contains("kagemusha_verifier_registry"),
        "{error}"
    );
}

#[test]
fn malformed_current_and_actual_predecessor_registry_are_rejected() {
    let world = World::default();
    let mut invalid = KagemushaGovernedVerifierRegistryV1::default();
    invalid.version = 7;
    set_registry(&world, invalid.clone());
    let error = restore(&world).err().expect("invalid current must fail");
    assert!(
        error.to_string().contains("kagemusha_verifier_registry"),
        "{error}"
    );
    set_registry(&world, KagemushaGovernedVerifierRegistryV1::default());
    let error = restore(&world)
        .err()
        .expect("invalid MV predecessor must fail");
    assert!(
        error
            .to_string()
            .contains("invalid predecessor verifier authority"),
        "{error}"
    );
}

#[test]
fn semantically_invalid_current_and_predecessor_authority_are_rejected() {
    let valid = active_registry();
    let mut bad_threshold = valid.clone();
    bad_threshold.authority_policy.as_mut().unwrap().threshold = 0;
    let mut bad_pointer = valid.clone();
    bad_pointer.active_release_id = Some([0x75; 32]);
    let mut bad_digest = valid.clone();
    bad_digest.releases[0].authority_policy_digest = [0x76; 32];
    let mut bad_status = valid.clone();
    bad_status.releases[0].status = u8::MAX;
    let mut duplicate_release = valid.clone();
    duplicate_release.releases.push(valid.releases[0].clone());
    for invalid in [
        bad_threshold,
        bad_pointer,
        bad_digest,
        bad_status,
        duplicate_release,
    ] {
        assert!(invalid.validate().is_err());
        let world = World::default();
        set_registry(&world, valid.clone());
        set_registry(&world, invalid);
        let current = restore(&world).err().expect("invalid current authority");
        assert!(
            current
                .to_string()
                .contains("invalid current verifier authority"),
            "{current}"
        );
        set_registry(&world, valid.clone());
        let predecessor = restore(&world)
            .err()
            .expect("invalid actual predecessor authority");
        assert!(
            predecessor
                .to_string()
                .contains("invalid predecessor verifier authority"),
            "{predecessor}"
        );
    }
}

#[test]
fn replayed_governed_release_remains_reject_all_until_local_authentication() {
    let world = World::default();
    let mut governed = KagemushaGovernedVerifierRegistryV1::default();
    governed
        .initialize_authority_policy(governed_policy())
        .unwrap();
    set_registry(&world, governed.clone());
    let state = State::new_for_testing(
        restore(&world).unwrap(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(state.validate_kagemusha_v1_runtime_for_startup().is_ok());

    governed = active_registry();
    // Model a replay transition after the process-local reject-all verifier was installed.
    set_registry(&state.world, governed);
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("active authority can replay with a fail-closed local verifier");
    let installed = state.kagemusha_v1_runtime_verifier();
    let runtime: &dyn std::any::Any = installed.as_ref();
    assert!(
        runtime.is::<crate::smartcontracts::isi::kagemusha::RejectAllKagemushaV1RuntimeVerifier>()
    );
}

#[test]
fn unchanged_governed_registry_commits_with_matching_reject_all_runtime() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let header = first_header();
    state
        .block(header)
        .commit_empty_block_for_testing()
        .expect("empty governed registry matches reject-all runtime");
    assert_eq!(state.latest_block_hash_fast(), Some(header.hash()));
}

#[test]
fn touching_governed_registry_without_changing_it_does_not_require_a_transition() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut block = state.block(first_header());
    let unchanged = block.world.kagemusha_verifier_registry.get().clone();
    *block.world.kagemusha_verifier_registry.get_mut() = unchanged;
    block
        .commit_empty_block_for_testing()
        .expect("equal governed registry is not a transition");
    assert!(state.latest_block_hash_fast().is_some());
}

#[test]
fn retired_governance_surface_cannot_publish_a_signer_policy_change() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let before_generation = state.view_generation.load(Ordering::Acquire);
    let before_registry = state.world.kagemusha_verifier_registry.view().get().clone();
    let mut block = state.block(first_header());
    block
        .world
        .kagemusha_verifier_registry
        .get_mut()
        .initialize_authority_policy(governed_policy())
        .unwrap();
    assert!(
        crate::smartcontracts::isi::kagemusha::runtime_matches_governed_registry(
            block.kagemusha_v1_runtime_verifier.as_ref(),
            block.world.kagemusha_verifier_registry.get(),
        )
        .is_ok(),
        "a signer-policy-only mutation still matches the reject-all runtime"
    );
    let error = block
        .commit_empty_block_for_testing()
        .expect_err("retired governance surface cannot authorize a signer-policy mutation");
    assert!(matches!(
        error,
        TransactionsBlockError::KagemushaGovernanceUnavailable
    ));
    assert_eq!(state.latest_block_hash_fast(), None);
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &before_registry
    );
    assert_eq!(
        state.view_generation.load(Ordering::Acquire),
        before_generation
    );
}

#[test]
fn unsigned_standby_install_is_rejected_before_state_publication() {
    let world = World::default();
    let standby = standby_registry();
    let mut policy_only = standby.clone();
    policy_only.releases.clear();
    set_registry(&world, policy_only.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let before_generation = state.view_generation.load(Ordering::Acquire);
    let mut block = state.block(first_header());
    *block.world.kagemusha_verifier_registry.get_mut() = standby;
    assert!(
        crate::smartcontracts::isi::kagemusha::runtime_matches_governed_registry(
            block.kagemusha_v1_runtime_verifier.as_ref(),
            block.world.kagemusha_verifier_registry.get(),
        )
        .is_ok(),
        "standby install must leave the runtime fail-closed"
    );
    assert!(matches!(
        block.commit_empty_block_for_testing(),
        Err(TransactionsBlockError::KagemushaGovernanceUnavailable)
    ));
    assert_eq!(state.latest_block_hash_fast(), None);
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &policy_only
    );
    assert_eq!(
        state.view_generation.load(Ordering::Acquire),
        before_generation
    );
}

#[test]
fn unsigned_standby_activation_is_rejected_before_state_publication() {
    let world = World::default();
    let standby = standby_registry();
    let release_id = standby.releases[0].release_id;
    set_registry(&world, standby.clone());
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let before_generation = state.view_generation.load(Ordering::Acquire);
    let mut block = state.block(first_header());
    block
        .world
        .kagemusha_verifier_registry
        .get_mut()
        .activate_standby(None, release_id)
        .expect("model permits a valid activation shape");
    assert_eq!(
        block.world.kagemusha_verifier_registry.get().releases[0].status,
        KAGEMUSHA_RELEASE_ACTIVE_V1
    );
    assert!(matches!(
        block.commit_empty_block_for_testing(),
        Err(TransactionsBlockError::KagemushaGovernanceUnavailable)
    ));
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &standby
    );
    assert_eq!(standby.releases[0].status, KAGEMUSHA_RELEASE_STANDBY_V1);
    assert_eq!(
        state.view_generation.load(Ordering::Acquire),
        before_generation
    );
}

#[test]
fn staged_release_change_is_rejected_without_state_publication() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let before_generation = state.view_generation.load(Ordering::Acquire);
    let before_registry = state.world.kagemusha_verifier_registry.view().get().clone();
    let mut block = state.block(first_header());
    *block.world.kagemusha_verifier_registry.get_mut() = active_registry();
    let error = block
        .commit_empty_block_for_testing()
        .expect_err("staged release has no Parliament authorization");
    assert!(matches!(
        error,
        TransactionsBlockError::KagemushaGovernanceUnavailable
    ));
    assert_eq!(state.latest_block_hash_fast(), None);
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &before_registry
    );
    assert_eq!(
        state.view_generation.load(Ordering::Acquire),
        before_generation,
        "a rejected overlay must not open the State publication interval"
    );
}

#[test]
fn restored_active_release_replays_fail_closed_until_local_reload() {
    let world = World::default();
    let governed = active_registry();
    set_registry(&world, governed.clone());
    let state = State::new_for_testing(
        restore(&world).unwrap(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    state
        .validate_kagemusha_v1_runtime_for_startup()
        .expect("active authority is durable without local artifacts");
    state
        .block(first_header())
        .commit_empty_block_for_testing()
        .expect("unchanged active authority replays under reject-all");
    assert!(state.latest_block_hash_fast().is_some());
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &governed
    );
}

#[test]
fn runtime_reload_head_rejects_changed_registry_tip_and_network_without_replacement() {
    use crate::smartcontracts::isi::kagemusha::{
        KagemushaV1RuntimeVerifier, RejectAllKagemushaV1RuntimeVerifier,
    };

    let mut state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let original = state.kagemusha_v1_runtime_verifier();
    let candidate: Arc<dyn KagemushaV1RuntimeVerifier> =
        Arc::new(RejectAllKagemushaV1RuntimeVerifier);

    let stale_registry = state.kagemusha_v1_runtime_reload_head();
    let mut governed = KagemushaGovernedVerifierRegistryV1::default();
    governed
        .initialize_authority_policy(governed_policy())
        .unwrap();
    set_registry(&state.world, governed);
    assert!(
        state
            .install_kagemusha_v1_runtime_verifier_checked(stale_registry, Arc::clone(&candidate))
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &original,
        &state.kagemusha_v1_runtime_verifier()
    ));

    let stale_tip = state.kagemusha_v1_runtime_reload_head();
    state
        .block(first_header())
        .commit_empty_block_for_testing()
        .expect("policy-only registry remains fail-closed at a new block tip");
    assert!(
        state
            .install_kagemusha_v1_runtime_verifier_checked(stale_tip, Arc::clone(&candidate))
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &original,
        &state.kagemusha_v1_runtime_verifier()
    ));

    let stale_generation = state.kagemusha_v1_runtime_reload_head();
    state.view_generation.fetch_add(2, Ordering::AcqRel);
    assert!(
        state
            .install_kagemusha_v1_runtime_verifier_checked(
                stale_generation,
                Arc::clone(&candidate),
            )
            .is_err(),
        "even the same registry and tip cannot reuse a head after publication"
    );
    assert!(Arc::ptr_eq(
        &original,
        &state.kagemusha_v1_runtime_verifier()
    ));

    let stale_network = state.kagemusha_v1_runtime_reload_head();
    state.network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::prehashed([0x7a; 32]),
    ));
    assert!(
        state
            .install_kagemusha_v1_runtime_verifier_checked(stale_network, candidate)
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &original,
        &state.kagemusha_v1_runtime_verifier()
    ));
}

#[test]
fn restored_policy_reload_accepts_exact_head_and_rejects_unavailable_release() {
    use crate::smartcontracts::isi::kagemusha::{
        KagemushaV1RuntimeVerifier, RejectAllKagemushaV1RuntimeVerifier,
    };

    let world = World::default();
    let mut governed = KagemushaGovernedVerifierRegistryV1::default();
    governed
        .initialize_authority_policy(governed_policy())
        .unwrap();
    set_registry(&world, governed);
    let state = Arc::new(State::new_for_testing(
        restore(&world).expect("restore the exact signer-policy predecessor"),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let retained_state = Arc::clone(&state);
    let previous_runtime = retained_state.kagemusha_v1_runtime_verifier();
    let candidate: Arc<dyn KagemushaV1RuntimeVerifier> =
        Arc::new(RejectAllKagemushaV1RuntimeVerifier);
    let head = state.kagemusha_v1_runtime_reload_head();
    state
        .install_kagemusha_v1_runtime_verifier_checked(head, Arc::clone(&candidate))
        .expect("policy-only recovery accepts the exact fail-closed runtime");
    assert!(Arc::ptr_eq(
        &candidate,
        &retained_state.kagemusha_v1_runtime_verifier()
    ));
    assert!(!Arc::ptr_eq(&previous_runtime, &candidate));
    assert!(Arc::ptr_eq(
        &candidate,
        &state.kagemusha_v1_runtime_verifier()
    ));

    set_registry(&state.world, active_registry());
    let head = state.kagemusha_v1_runtime_reload_head();
    assert!(
        state
            .install_kagemusha_v1_runtime_verifier_checked(head, Arc::clone(&candidate))
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &candidate,
        &state.kagemusha_v1_runtime_verifier()
    ));
}

#[test]
fn runtime_reload_world_reader_notifications_follow_commit_fence_release() {
    use crate::smartcontracts::isi::kagemusha::{
        KagemushaV1RuntimeVerifier, RejectAllKagemushaV1RuntimeVerifier,
    };
    use std::{
        future::Future as _,
        sync::atomic::{AtomicBool, AtomicUsize},
        task::{Context, Wake, Waker},
    };
    struct Probe {
        state: Arc<State>,
        calls: AtomicUsize,
        blocked: AtomicBool,
    }
    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.blocked.store(
                self.state.state_commit_lock.try_lock().is_none(),
                Ordering::SeqCst,
            );
        }
    }
    for operation in 0..3 {
        let state = Arc::new(State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let mut registration =
            crate::unit_test_support::release_registration(&state.ivm_execution_budget());
        let candidate: Arc<dyn KagemushaV1RuntimeVerifier> =
            Arc::new(RejectAllKagemushaV1RuntimeVerifier);
        let previous = state.kagemusha_v1_runtime_verifier();
        let mut head = state.kagemusha_v1_runtime_reload_head();
        let probe = Arc::new(Probe {
            state: Arc::clone(&state),
            calls: AtomicUsize::new(0),
            blocked: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&probe));
        let mut context = Context::from_waker(&waker);
        let mut wait = std::pin::pin!(
            state
                .world
                .domains
                .observe_reader_release()
                .wait_for_release(&mut registration)
        );
        assert!(wait.as_mut().poll(&mut context).is_pending());
        match operation {
            0 => {
                let captured = state.kagemusha_v1_runtime_reload_head();
                assert_eq!(captured.view_generation, head.view_generation);
                assert_eq!(captured.registry, head.registry);
            }
            1 => {
                state
                    .install_kagemusha_v1_runtime_verifier_checked(head, Arc::clone(&candidate))
                    .unwrap();
                assert!(Arc::ptr_eq(
                    &state.kagemusha_v1_runtime_verifier(),
                    &candidate
                ));
            }
            _ => {
                head.view_generation = head.view_generation.checked_add(2).unwrap();
                assert!(
                    state
                        .install_kagemusha_v1_runtime_verifier_checked(head, candidate)
                        .is_err()
                );
                assert!(Arc::ptr_eq(
                    &state.kagemusha_v1_runtime_verifier(),
                    &previous
                ));
            }
        }
        assert_eq!(probe.calls.load(Ordering::SeqCst), 1);
        assert!(!probe.blocked.load(Ordering::SeqCst));
        assert!(wait.as_mut().poll(&mut context).is_ready());
    }
}

#[test]
fn direct_standby_retirement_without_certified_original_owner_never_publishes() {
    let predecessor = standby_registry();
    let target = predecessor.releases[0].release_id;
    let (_, manifest, _, _) =
        crate::smartcontracts::isi::kagemusha::release_evidence_tests::release_evidence();
    let world = World::default();
    set_registry(&world, predecessor.clone());
    let state = State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        "generic-testnet".parse().unwrap(),
        manifest.network_id,
    );
    let before_generation = state.view_generation.load(Ordering::Acquire);
    let mut block = state.block(first_header());
    block
        .world
        .kagemusha_verifier_registry
        .get_mut()
        .retire_standby(target)
        .unwrap();
    assert!(
        block
            .world
            .kagemusha_verifier_registry
            .get()
            .releases
            .is_empty()
    );
    assert!(matches!(
        block.commit_empty_block_for_testing(),
        Err(TransactionsBlockError::KagemushaGovernanceUnavailable)
    ));
    assert_eq!(state.latest_block_hash_fast(), None);
    assert_eq!(
        state.world.kagemusha_verifier_registry.view().get(),
        &predecessor
    );
    assert_eq!(
        state.view_generation.load(Ordering::Acquire),
        before_generation
    );
}
