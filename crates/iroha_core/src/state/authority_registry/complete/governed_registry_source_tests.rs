//! Original governed cell acquisition, canonical frames, limits and charged retirement.
use super::*;
use crate::{
    kura::Kura, query::store::LiveQueryStore, state::World, test_allocations::allocations_during,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::kagemusha::{KAGEMUSHA_WIRE_VERSION_V1, KagemushaReleaseAuthorityPolicyV1};

fn state() -> State {
    State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}
fn limits() -> RegistryCaptureLimits {
    RegistryCaptureLimits {
        max_current_bytes: 1_048_576,
        max_predecessor_bytes: 1_048_576,
        max_total_bytes: 2_097_152,
    }
}
fn policy() -> KagemushaReleaseAuthorityPolicyV1 {
    let signer = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    KagemushaReleaseAuthorityPolicyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        authority_set_id: [0x72; 32],
        threshold: 1,
        authorized_signers: vec![signer.public_key().clone()],
    }
}
fn install_policy(state: &State) {
    // Data-only source fixture. No finalized transition or runtime permit is claimed.
    let mut cell = state.world.kagemusha_verifier_registry.block();
    cell.get_mut()
        .initialize_authority_policy(policy())
        .unwrap();
    cell.commit();
}

#[test]
fn cold_native_governed_registry_acquisition_does_not_clone_or_pin() {
    let state = state();
    install_policy(&state);
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let mut captured = None;
                assert_eq!(
                    allocations_during(
                        || captured = Some(GovernedRegistrySource::try_capture(&state))
                    ),
                    0
                );
                let source = captured.unwrap().unwrap().unwrap();
                assert!(source.current().authority_policy.is_some());
                assert_eq!(
                    source.predecessor(),
                    Some(&KagemushaGovernedVerifierRegistryV1::default())
                );
                assert_eq!(allocations_during(|| drop(source)), 0);
            })
            .join()
            .unwrap()
    });
}

#[test]
fn native_registry_pair_keeps_absent_predecessor_and_exact_canonical_frames() {
    let state = state();
    let empty = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(
        empty.current_bytes(),
        norito::encode_canonical(&KagemushaGovernedVerifierRegistryV1::default()).unwrap()
    );
    assert!(empty.predecessor_bytes().is_none());
    install_policy(&state);
    assert!(!empty.try_matches_current().unwrap());
    let captured = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    let native = state
        .world
        .kagemusha_verifier_registry
        .try_committed_borrow()
        .unwrap();
    assert_eq!(
        captured.current_bytes(),
        norito::encode_canonical(native.current()).unwrap()
    );
    assert_eq!(
        captured.predecessor_bytes().unwrap(),
        norito::encode_canonical(native.undo().as_ref().unwrap()).unwrap()
    );
    assert_eq!(empty.current_bytes(), captured.predecessor_bytes().unwrap());
}

#[test]
fn registry_byte_limits_and_original_pool_refusal_leave_sources_unlocked() {
    let state = state();
    install_policy(&state);
    let exact = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    let sizes = (
        exact.current_bytes().len(),
        exact.predecessor_bytes().unwrap().len(),
    );
    drop(exact);
    let budget = state
        .pipeline_ivm_prepared_cache
        .read()
        .execution_budget()
        .clone();
    let before = budget.reserved_bytes();
    for dimension in 0..3 {
        let mut cap = RegistryCaptureLimits {
            max_current_bytes: sizes.0,
            max_predecessor_bytes: sizes.1,
            max_total_bytes: sizes.0 + sizes.1,
        };
        match dimension {
            0 => cap.max_current_bytes -= 1,
            1 => cap.max_predecessor_bytes -= 1,
            _ => cap.max_total_bytes -= 1,
        }
        assert!(matches!(
            CapturedGovernedRegistry::try_capture(&state, cap),
            Err(RegistryCaptureError::ByteLimit)
        ));
        assert_eq!(budget.reserved_bytes(), before);
        assert!(
            state
                .world
                .kagemusha_verifier_registry
                .try_committed_borrow()
                .is_ok()
        );
    }
    let limit = budget.limit_bytes();
    budget.set_limit_bytes(0);
    assert!(matches!(
        CapturedGovernedRegistry::try_capture(&state, limits()),
        Err(RegistryCaptureError::Admission(_))
    ));
    assert!(
        state
            .world
            .kagemusha_verifier_registry
            .try_committed_borrow()
            .is_ok()
    );
    assert_eq!(budget.reserved_bytes(), before);
    budget.set_limit_bytes(limit);
    let captured = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(budget.reserved_bytes() - before, sizes.0 + sizes.1);
    drop(captured);
    assert_eq!(budget.reserved_bytes(), before);
}

#[test]
fn complete_policy_capture_uses_only_its_two_charged_byte_buffers_on_cold_thread() {
    let state = state();
    install_policy(&state);
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let mut capture = None;
                let count = allocations_during(|| {
                    capture = Some(CapturedGovernedRegistry::try_capture(&state, limits()))
                });
                let capture = capture.unwrap().unwrap().unwrap();
                assert_eq!(
                    count, 2,
                    "only current and present undo byte buffers may allocate"
                );
                assert_eq!(
                    allocations_during(|| assert!(capture.try_matches_current().unwrap())),
                    0
                );
                assert_eq!(allocations_during(|| drop(capture)), 0);
            })
            .join()
            .unwrap()
    });
}

#[test]
fn malformed_native_current_and_predecessor_are_not_replaced_by_empty_authority() {
    let state = state();
    let mut original = state.world.kagemusha_verifier_registry.block();
    original.get_mut().version = 0;
    original.commit();
    assert!(matches!(
        GovernedRegistrySource::try_capture(&state),
        Err(RegistrySourceError::Invalid {
            predecessor: false,
            ..
        })
    ));
    let mut next = state.world.kagemusha_verifier_registry.block();
    *next.get_mut() = KagemushaGovernedVerifierRegistryV1::default();
    next.commit();
    assert!(matches!(
        GovernedRegistrySource::try_capture(&state),
        Err(RegistrySourceError::Invalid {
            predecessor: true,
            ..
        })
    ));
}

#[test]
fn equal_registry_republication_and_state_publication_retire_original_capture() {
    let state = state();
    install_policy(&state);
    let before = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    state.world.kagemusha_verifier_registry.block().commit();
    assert!(!before.try_matches_current().unwrap());
    let same = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(same.current_bytes(), before.current_bytes());
    let mut publication = state.state_view_publication();
    let fence = state.state_commit_lock.lock();
    let held = publication.begin();
    assert!(
        CapturedGovernedRegistry::try_capture(&state, limits())
            .unwrap()
            .is_none()
    );
    assert!(!same.try_matches_current().unwrap());
    drop(held);
    drop(fence);
    drop(publication);
    assert!(!same.try_matches_current().unwrap());
}

/// Real threshold authentication over synthetic reports and public fixture keys.
/// Fixture construction is outside the measured owner; it confers no finality or release claim.
fn authenticated_populated_pair() -> (
    KagemushaGovernedVerifierRegistryV1,
    KagemushaGovernedVerifierRegistryV1,
) {
    use iroha_crypto::SignatureOf;
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_RELEASE_ACTIVE_V1, KAGEMUSHA_RELEASE_STANDBY_V1,
        KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1, KagemushaReleaseApprovalV1,
        KagemushaReleaseAttestationV1,
    };
    let (mut predecessor, manifest, receipt, attestation) =
        crate::smartcontracts::isi::kagemusha::release_evidence_tests::release_evidence();
    predecessor
        .install_authenticated_release(&manifest, &receipt, &attestation)
        .expect("actual threshold-authenticated first standby");
    let first = manifest.release_id;
    predecessor.activate_standby(None, first).unwrap();
    let policy = predecessor.authority_policy.as_ref().unwrap();
    let mut keys: Vec<_> = [0x41_u8, 0x42, 0x43]
        .into_iter()
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap())
        .collect();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert_eq!(
        policy.authorized_signers,
        keys.iter()
            .map(|key| key.public_key().clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(policy.threshold, 2);
    let mut receipt = receipt.clone();
    receipt.source_tree_digest[0] ^= 0x5a;
    let mut manifest = manifest.clone();
    manifest.source_tree_digest = receipt.source_tree_digest;
    manifest.validation_receipt_digest = receipt.canonical_digest().unwrap();
    let manifest = manifest.seal().unwrap();
    assert_ne!(manifest.release_id, first);
    assert!(
        predecessor
            .clone()
            .install_authenticated_release(&manifest, &receipt, &attestation)
            .is_err(),
        "original approvals cannot authenticate the substituted release"
    );
    let subject = manifest
        .release_attestation_subject(&receipt, policy)
        .unwrap();
    let payload = subject.approval_payload();
    let attestation = KagemushaReleaseAttestationV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        subject,
        approvals: keys[..2]
            .iter()
            .map(|key| KagemushaReleaseApprovalV1 {
                public_key: key.public_key().clone(),
                signature: SignatureOf::try_new(key.private_key(), &payload).unwrap(),
            })
            .collect(),
    };
    predecessor
        .install_authenticated_release(&manifest, &receipt, &attestation)
        .expect("new exact two-of-three signatures authenticate nested release data");
    assert_eq!(predecessor.releases.len(), 2);
    assert_eq!(
        predecessor
            .releases
            .iter()
            .filter(|r| r.status == KAGEMUSHA_RELEASE_ACTIVE_V1)
            .count(),
        1
    );
    assert_eq!(
        predecessor
            .releases
            .iter()
            .filter(|r| r.status == KAGEMUSHA_RELEASE_STANDBY_V1)
            .count(),
        1
    );
    let mut current = predecessor.clone();
    current
        .activate_standby(Some(first), manifest.release_id)
        .unwrap();
    assert_eq!(
        current
            .releases
            .iter()
            .filter(|r| r.status == KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1)
            .count(),
        1
    );
    assert_eq!(
        current
            .releases
            .iter()
            .filter(|r| r.status == KAGEMUSHA_RELEASE_ACTIVE_V1)
            .count(),
        1
    );
    assert_ne!(current.active_release_id, predecessor.active_release_id);
    (predecessor, current)
}
fn install_pair(
    state: &State,
    predecessor: KagemushaGovernedVerifierRegistryV1,
    current: KagemushaGovernedVerifierRegistryV1,
) {
    for registry in [predecessor, current] {
        let mut native = state.world.kagemusha_verifier_registry.block();
        *native.get_mut() = registry;
        native.commit();
    }
}

#[test]
fn authenticated_populated_original_pair_retains_nested_releases_and_canonical_frames() {
    let (predecessor, current) = authenticated_populated_pair();
    let state = state();
    install_pair(&state, predecessor.clone(), current.clone());
    let capture = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    let expected_current = norito::encode_canonical(&current).unwrap();
    let expected_predecessor = norito::encode_canonical(&predecessor).unwrap();
    assert_eq!(capture.current_bytes(), expected_current);
    assert_eq!(capture.predecessor_bytes().unwrap(), expected_predecessor);
    assert_eq!(
        norito::decode_canonical::<KagemushaGovernedVerifierRegistryV1>(capture.current_bytes())
            .unwrap(),
        current
    );
    assert_eq!(
        norito::decode_canonical::<KagemushaGovernedVerifierRegistryV1>(
            capture.predecessor_bytes().unwrap()
        )
        .unwrap(),
        predecessor
    );
    let mut next = current.clone();
    next.releases[0].native_profile_digest[0] ^= 0x31;
    next.validate().unwrap();
    let mut native = state.world.kagemusha_verifier_registry.block();
    *native.get_mut() = next;
    native.commit();
    assert!(!capture.try_matches_current().unwrap());
    assert_eq!(capture.current_bytes(), expected_current);
    assert_eq!(capture.predecessor_bytes().unwrap(), expected_predecessor);
    let newer = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    assert_ne!(newer.current_bytes(), capture.current_bytes());
    assert_eq!(newer.predecessor_bytes().unwrap(), expected_current);
}

#[test]
fn cold_populated_registry_original_acquisition_and_encoding_charge_exactly_two_buffers() {
    let (predecessor, current) = authenticated_populated_pair();
    let expected_current = norito::encode_canonical(&current).unwrap();
    let expected_predecessor = norito::encode_canonical(&predecessor).unwrap();
    let state = state();
    install_pair(&state, predecessor, current);
    let budget = state
        .pipeline_ivm_prepared_cache
        .read()
        .execution_budget()
        .clone();
    let before = budget.reserved_bytes();
    std::thread::scope(|scope| {
        scope.spawn(|| {
        let mut source=None;
        assert_eq!(allocations_during(||source=Some(GovernedRegistrySource::try_capture(&state))),0);
        let source=source.unwrap().unwrap().unwrap();
        assert_eq!(source.current().releases.len(),2);assert_eq!(source.predecessor().unwrap().releases.len(),2);
        assert_eq!(allocations_during(||drop(source)),0);
        let mut captured=None;
        let count=allocations_during(||captured=Some(CapturedGovernedRegistry::try_capture(&state,limits())));
        let captured=captured.unwrap().unwrap().unwrap();
        assert_eq!(count,2,"complete populated current/undo registry encoding allocates only the two admitted buffers");
        assert_eq!(budget.reserved_bytes()-before,expected_current.len()+expected_predecessor.len());
        assert_eq!(captured.current_bytes(),expected_current);assert_eq!(captured.predecessor_bytes().unwrap(),expected_predecessor);
        assert_eq!(allocations_during(||assert!(captured.try_matches_current().unwrap())),0);
        assert_eq!(allocations_during(||drop(captured)),0);
        assert_eq!(budget.reserved_bytes(),before);
    }).join().unwrap()
    });
}

#[test]
fn each_malformed_nested_original_side_refuses_without_replacing_or_leaking_authority() {
    let (predecessor, current) = authenticated_populated_pair();
    for bad_predecessor in [false, true] {
        for mutation in 0..5 {
            let state = state();
            let mut old = predecessor.clone();
            let mut now = current.clone();
            let damaged = if bad_predecessor { &mut old } else { &mut now };
            match mutation {
                0 => damaged.releases[0].vk_set_digest = [0; 32],
                1 => damaged.releases[0].authority_policy_digest[0] ^= 1,
                2 => damaged.releases[0].status = 0,
                3 => damaged.releases.swap(0, 1),
                _ => damaged.active_release_id = Some([0x99; 32]),
            }
            assert!(damaged.validate().is_err());
            install_pair(&state, old, now);
            let budget = state
                .pipeline_ivm_prepared_cache
                .read()
                .execution_budget()
                .clone();
            let before = budget.reserved_bytes();
            assert!(
                matches!(CapturedGovernedRegistry::try_capture(&state,limits()),Err(RegistryCaptureError::Source(RegistrySourceError::Invalid{predecessor,..})) if predecessor==bad_predecessor)
            );
            assert_eq!(budget.reserved_bytes(), before);
            assert!(
                state
                    .world
                    .kagemusha_verifier_registry
                    .try_committed_borrow()
                    .is_ok(),
                "both original physical writers release on every malformed side"
            );
        }
    }
}

#[test]
fn standby_retirement_capture_retains_exact_original_predecessor_and_remaining_active_authority() {
    let (predecessor, _) = authenticated_populated_pair();
    let target = predecessor
        .releases
        .iter()
        .find(|row| row.status == iroha_data_model::kagemusha::KAGEMUSHA_RELEASE_STANDBY_V1)
        .unwrap()
        .release_id;
    let mut current = predecessor.clone();
    current.retire_standby(target).unwrap();
    let state = state();
    // Data-only source fixture: block publication rejects every registry mutation.
    install_pair(&state, predecessor.clone(), current.clone());
    let captured = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(
        captured.current_bytes(),
        norito::encode_canonical(&current).unwrap()
    );
    assert_eq!(
        captured.predecessor_bytes().unwrap(),
        norito::encode_canonical(&predecessor).unwrap()
    );
    assert_eq!(current.authority_policy, predecessor.authority_policy);
    assert_eq!(current.active_release_id, predecessor.active_release_id);
    assert_eq!(current.releases.len() + 1, predecessor.releases.len());
    assert!(captured.try_matches_current().unwrap());
    state.world.kagemusha_verifier_registry.block().commit();
    assert!(!captured.try_matches_current().unwrap());
    assert_eq!(
        captured.current_bytes(),
        norito::encode_canonical(&current).unwrap()
    );
}

#[test]
fn local_unavailable_handle_replacement_preserves_both_governed_source_images() {
    use crate::smartcontracts::isi::kagemusha::RejectAllKagemushaV1RuntimeVerifier;
    use std::sync::Arc;

    let state = state();
    install_policy(&state);
    let captured = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    let original_runtime = state.kagemusha_v1_runtime_verifier();
    state
        .install_kagemusha_v1_runtime_verifier_checked(
            state.kagemusha_v1_runtime_reload_head(),
            Arc::new(RejectAllKagemushaV1RuntimeVerifier),
        )
        .unwrap();
    assert!(!Arc::ptr_eq(
        &original_runtime,
        &state.kagemusha_v1_runtime_verifier()
    ));
    assert!(captured.try_matches_current().unwrap());
    let after = CapturedGovernedRegistry::try_capture(&state, limits())
        .unwrap()
        .unwrap();
    assert_eq!(captured.current_bytes(), after.current_bytes());
    assert_eq!(captured.predecessor_bytes(), after.predecessor_bytes());
    state.validate_kagemusha_v1_runtime_for_startup().unwrap();
}
