//! Structural retry correlation and locator storage only; no production Core authority.
use super::*;

fn attempt(operation: u8) -> ReleaseAttempt {
    ReleaseAttempt {
        operation_id: [operation; 32],
        original_command: vec![operation, 1],
        intent_directory: PathBuf::from(format!("/native/release/{operation}")),
        request: vec![vec![operation]],
        response: vec![vec![operation, 2]],
        destination: KagemushaNativeCorePublicationDestinationV1 {
            directory: PathBuf::from(format!("/native/publication/{operation}")),
            checkpoint_operation_id: [operation + 1; 32],
        },
        completion: None,
    }
}

#[test]
fn uncertain_attempt_refuses_another_id_or_command_without_changing_original_completion() {
    let mut original = attempt(1);
    original
        .require_completion_tuple([1; 32], &[1, 1], &[1, 3])
        .unwrap();
    assert!(
        original
            .require_completion_tuple([2; 32], &[2, 1], &[2, 3])
            .is_err()
    );
    assert!(
        original
            .require_completion_tuple([1; 32], &[1, 2], &[1, 3])
            .is_err()
    );
    assert!(original.completion.is_none());
    original.completion = Some(vec![1, 3]);
    original
        .require_completion_tuple([1; 32], &[1, 1], &[1, 3])
        .unwrap();
    assert!(
        original
            .require_completion_tuple([1; 32], &[1, 1], &[1, 4])
            .is_err()
    );
    assert_eq!(original.completion, Some(vec![1, 3]));
}

#[test]
fn original_locator_storage_retains_a_then_b_and_is_bounded_without_deleting_wals() {
    let mut locators = std::collections::BTreeMap::new();
    for operation in 1..=2 {
        let original = attempt(operation);
        retain_completed_locator(
            &mut locators,
            original.operation_id,
            KagemushaNativeCompletedOutboxReleaseLocatorV1 {
                intent_directory: original.intent_directory,
                destination: original.destination,
            },
        );
    }
    assert_eq!(
        locators.get(&[1; 32]).unwrap().intent_directory,
        PathBuf::from("/native/release/1")
    );
    assert_eq!(
        locators
            .get(&[2; 32])
            .unwrap()
            .destination
            .checkpoint_operation_id,
        [3; 32]
    );
    for operation in 3..=40 {
        let original = attempt(operation);
        retain_completed_locator(
            &mut locators,
            original.operation_id,
            KagemushaNativeCompletedOutboxReleaseLocatorV1 {
                intent_directory: original.intent_directory,
                destination: original.destination,
            },
        );
        assert!(locators.len() <= MAX_COMPLETED_RELEASE_LOCATORS);
    }
    assert_eq!(locators.len(), 16);
    // A missed hint must use independent native source lookup and full actual WAL verification;
    // these path markers are neither a successful retry nor a Released record.
    assert!(!locators.contains_key(&[1; 32]));
}

#[test]
fn pending_a_blocks_b_and_completion_closes_prepare_until_published_proof() {
    let mut a = attempt(1);
    let b = attempt(2);
    assert_eq!(a.retry_response(&a.request).unwrap(), a.response);
    assert!(a.retry_response(&b.request).is_err());
    assert!(!release_retirement_ready(&a, false));
    assert!(!release_retirement_ready(&a, true));
    a.completion = Some(vec![1, 3]);
    assert!(a.retry_response(&a.request).is_err());
    assert!(a.retry_response(&b.request).is_err());
    assert!(!release_retirement_ready(&a, false));
    assert!(release_retirement_ready(&a, true));
    // This predicate only selects the production proof path. True alone performs no retirement;
    // actual Core must authenticate the exact completed WAL and native Released tombstone.
    a.require_completion_tuple([1; 32], &[1, 1], &[1, 3])
        .unwrap();
    assert!(
        a.require_completion_tuple([2; 32], &[2, 1], &[2, 3])
            .is_err()
    );
}
