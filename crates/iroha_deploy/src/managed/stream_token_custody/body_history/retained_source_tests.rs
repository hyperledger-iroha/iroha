//! Original native custody survives unrelated provider progress without becoming current authority.

use super::*;
use crate::managed::native_operation::test_support::provider_id;
use crate::managed::stream_token_custody::renewal_tests::{Fixture, wait_until};
use iroha_data_model::sorafs::stream_token_custody::proof::StreamTokenCustodyProofV1;
use std::time::Duration;

fn assert_same_authenticated_cut(
    actual: &VerifiedStreamTokenCustodyStateV1,
    expected: &VerifiedStreamTokenCustodyStateV1,
) {
    assert_eq!(actual.network_id(), expected.network_id());
    assert_eq!(actual.provider_id(), expected.provider_id());
    assert_eq!(actual.owner(), expected.owner());
    assert_eq!(actual.height(), expected.height());
    assert_eq!(actual.context_id(), expected.context_id());
    let actual = actual.current().unwrap();
    let expected = expected.current().unwrap();
    assert_eq!(actual.record(), expected.record());
    assert_eq!(actual.control(), expected.control());
    assert_eq!(actual.anchor(), expected.anchor());
}

#[test]
fn retained_native_source_finishes_after_other_provider_progress_and_refuses_substitution() {
    let _guard = crate::managed::native_test_guard();
    let mut fixture = Fixture::enrolled_with_renewal_validity(4_000, 60_000);
    wait_until(
        fixture.initial.issued_at_unix_ms + 2_000,
        Duration::from_secs(4),
    );
    let (checkpoint, current, source) = fixture.current_with_source();
    let terms = Terms::new(now_ms().unwrap() + 60_000, &fixture.options).unwrap();
    let unsigned = fixture
        .owner
        .select_renewal_unsigned(
            2,
            &fixture.policy,
            &current,
            source.clone(),
            &checkpoint,
            &terms,
            fixture.options.deadline,
        )
        .unwrap();
    let selection = unsigned.selection.clone();
    let history = BodyHistory::initialize(
        &fixture.owner,
        CustodyPurpose::Renewal(2),
        unsigned,
        &terms.fees,
        &SigningTurn::Explicit(&terms),
        fixture.options.deadline,
    )
    .unwrap();
    assert!(history.original().is_none());

    // A different provider executes a real Configure wallet while this body is unsigned.
    let mut other =
        ManagedStreamTokenCustody::open(&fixture.prepared, provider_id(&fixture.prepared, 1))
            .unwrap();
    let other_policy = crate::managed::stream_token_custody::transport_tests::policy(&other);
    let progressed = other.bootstrap_native_configure(
        &mut fixture.native,
        &other_policy,
        now_ms().unwrap() + 60_000,
        &fixture.options,
    );
    assert!(progressed.finalized.unwrap().height > checkpoint.checkpoint().height());
    let (fresh_checkpoint, fresh_current, later_source) = fixture.current_with_source();
    assert!(fresh_current.height() > current.height());
    let historical = fixture
        .owner
        .verify_retained_source(&selection, &source, &checkpoint)
        .unwrap();
    assert_same_authenticated_cut(&historical, &current);
    let fresh = fixture
        .owner
        .verify_retained_source(&selection, &later_source, &fresh_checkpoint)
        .unwrap();
    assert_same_authenticated_cut(&fresh, &fresh_current);

    // Neither the new cut's proof nor a changed owner/binding can replace original evidence.
    assert!(
        fixture
            .owner
            .verify_retained_source(&selection, &later_source, &checkpoint)
            .is_err()
    );
    assert!(
        fixture
            .owner
            .verify_retained_source(&selection, &source, &fresh_checkpoint)
            .is_err()
    );
    let mut foreign = StreamTokenCustodyProofV1::decode_frame(&source).unwrap();
    foreign.owner = other
        .authority
        .provider_role(StreamTokenAuthorityRole::IssuerOperator)
        .unwrap()
        .clone();
    assert!(
        fixture
            .owner
            .verify_retained_source(
                &selection,
                &norito::encode_canonical(&foreign).unwrap(),
                &checkpoint,
            )
            .is_err()
    );
    let mut changed_selection = selection.clone();
    changed_selection.binding.policy_digest[0] ^= 1;
    assert!(
        fixture
            .owner
            .verify_retained_source(&changed_selection, &source, &checkpoint)
            .is_err()
    );
    assert!(
        fixture
            .owner
            .verify_retained_source(&selection, &[1], &checkpoint)
            .is_err()
    );
    assert!(
        fixture
            .owner
            .verify_retained_source(&selection, &[], &checkpoint)
            .is_err()
    );
    let mut noncanonical = source.clone();
    noncanonical.push(0);
    assert!(
        fixture
            .owner
            .verify_retained_source(&selection, &noncanonical, &checkpoint)
            .is_err()
    );
    let mut wrong_schema = StreamTokenCustodyProofV1::decode_frame(&source).unwrap();
    wrong_schema.world.schema_hash = Hash::new(b"foreign native schema");
    assert!(
        fixture
            .owner
            .verify_retained_source(
                &selection,
                &norito::encode_canonical(&wrong_schema).unwrap(),
                &checkpoint,
            )
            .is_err()
    );
    let mut wrong_world = StreamTokenCustodyProofV1::decode_frame(&source).unwrap();
    wrong_world.world.entries.clear();
    assert!(
        fixture
            .owner
            .verify_retained_source(
                &selection,
                &norito::encode_canonical(&wrong_world).unwrap(),
                &checkpoint,
            )
            .is_err()
    );

    // The production signing reader must use the retained proof. Historical HTTP is unavailable.
    let peers = fixture.unavailable_peers();
    assert!(
        fixture
            .owner
            .read_current(
                &fixture.policy.binding,
                &checkpoint,
                fixture.options.deadline,
            )
            .is_err()
    );
    let old_path = format!("/custody/{}", checkpoint.checkpoint().height());
    assert!(
        peers
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.path.ends_with(&old_path))
    );
    peers.requests.lock().unwrap().clear();
    let history = history
        .finish_pending(
            &fixture.owner,
            &fresh_current,
            &SigningTurn::Explicit(&terms),
            fixture.options.deadline,
        )
        .unwrap();
    assert!(peers.requests.lock().unwrap().is_empty());
    drop(peers);
    let original = history.original().unwrap().clone();
    assert_eq!(original.source, source);
    fixture
        .owner
        .fresh_predecessor_observation(
            &original,
            &historical,
            &fresh_current,
            fixture.options.deadline,
        )
        .unwrap();

    // Original evidence is still valid after this provider changes, but authorizes no new use.
    fixture.owner.bootstrap_native_enrollment_original(
        &mut fixture.native,
        CustodyPurpose::Renewal(2),
        history,
        terms.requested_deadline_unix_ms,
        &fixture.options,
    );
    let (_, changed_current) = fixture.current();
    assert!(
        fixture
            .owner
            .verify_retained_source(&selection, &source, &checkpoint)
            .is_ok()
    );
    assert!(
        fixture
            .owner
            .fresh_predecessor_observation(
                &original,
                &historical,
                &changed_current,
                fixture.options.deadline,
            )
            .is_err()
    );
}
