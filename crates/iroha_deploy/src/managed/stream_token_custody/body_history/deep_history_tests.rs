//! Genuine finite body expiries exercise the deepest retained unsigned predecessor graph.
//! These local histories do not qualify current signing eligibility or running services.

use super::*;
use crate::managed::{
    native_operation::test_support::native_fixture::quote_instructions,
    stream_token_custody::{
        bootstrap_test_support::NativeEnrollmentReads,
        renewal_tests::{Fixture, wait_until},
    },
};
use iroha_data_model::isi::{InstructionBox, Log};
use iroha_wallet::operations::NativePreparationPhase;
use std::{path::PathBuf, time::Duration};

struct OriginalBytes {
    body: PathBuf,
    original: zeroize::Zeroizing<Vec<u8>>,
    request: Vec<u8>,
}

fn require_full_census(history: &History) {
    let (result, census) = history.test_require_current(2 * 64);
    result.unwrap();
    assert_eq!(census.distinct_histories, 64);
    assert!(census.visits >= census.distinct_histories);
    assert!(census.visits <= 2 * 64);
}

// A genuine paid Log refreshes the certified cut when necessary. It does not mutate custody,
// set the fixture's clock, fabricate an empty block or replace any retained body checkpoint.
fn refresh_old_anchor(fixture: &mut Fixture) {
    let height = fixture.native.chain.height();
    let committed = fixture.native.chain.committed(height);
    let issued = u64::try_from(committed.block().header().creation_time().as_millis()).unwrap();
    if now_ms().unwrap().saturating_sub(issued) < fixture.policy.max_anchor_age_ms / 2 {
        return;
    }
    drop(committed);
    let log = quote_instructions(
        &fixture.native,
        &fixture.owner.authority.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "deep retained custody predecessor".into(),
        ))],
    );
    assert_eq!(fixture.native.chain.commit(vec![log]), vec![true]);
    assert_eq!(fixture.native.chain.height(), height + 1);
}

#[test]
fn sixty_four_genuine_bodies_bound_retained_visits_and_refuse_oldest_material_substitution() {
    let _guard = crate::managed::native_test_guard();
    let mut fixture = Fixture::enrolled_with_renewal_validity(4_000, 4_000);
    let original_policy = fixture.policy.clone();
    let original_fees = Fees::from_options(&fixture.options).unwrap();
    let complete_before = Instant::now() + Duration::from_secs(600);
    wait_until(
        fixture.initial.issued_at_unix_ms + 2_000,
        Duration::from_secs(4),
    );
    let (_, initial_current) = fixture.current();
    let original_native = initial_current.current().unwrap().record().clone();
    assert_eq!(original_native.execution_height, 4);
    assert_eq!(
        initial_current.current().unwrap().control().next_sequence,
        2
    );
    drop(initial_current);
    let initial = fixture
        .owner
        .required_enrollment(CustodyPurpose::InitialEnroll)
        .unwrap();
    let initial_carrier_path = initial.directory().path().join("carrier.nrt");
    let initial_carrier = std::fs::read(&initial_carrier_path).unwrap();
    drop(initial);
    let initial_history = BodyHistory::open(&fixture.owner, CustodyPurpose::InitialEnroll)
        .unwrap()
        .unwrap();
    let (result, census) = initial_history
        .current_history()
        .unwrap()
        .test_require_current(1);
    result.unwrap();
    assert_eq!(census.visits, 1);
    assert_eq!(census.distinct_histories, 1);
    drop(initial_history);

    let mut retained: Option<BodyHistory> = None;
    let mut originals = Vec::with_capacity(64);
    for ordinal in 1..=64u8 {
        if let Some(history) = retained.as_ref() {
            let expiry = history
                .bodies
                .last()
                .unwrap()
                .reservation
                .unsigned
                .statement
                .expires_at_unix_ms;
            wait_until(expiry, Duration::from_secs(6));
        }
        assert!(Instant::now() < complete_before);
        assert!(now_ms().unwrap() < original_policy.active_until_unix_ms);
        // Each separate startup has its own finite I/O budget. No retained body, epoch or
        // wallet Terms are changed, and the originally configured policy remains the cap.
        fixture.options.deadline = (Instant::now() + Duration::from_secs(120)).min(complete_before);
        assert!(Fees::from_options(&fixture.options).unwrap() == original_fees);
        refresh_old_anchor(&mut fixture);
        let mut turn = fixture.renewal_turn();
        let (checkpoint, current) = fixture.current();
        assert_eq!(current.current().unwrap().record(), &original_native);
        assert_eq!(current.current().unwrap().control().next_sequence, 2);

        let history = if let Some(previous) = retained.take() {
            let authorization = turn
                .authorize_retained(&fixture.owner, &previous, fixture.options.deadline)
                .unwrap();
            let terms = Terms::new(now_ms().unwrap() + 4_000, &fixture.options).unwrap();
            let unsigned = fixture
                .owner
                .select_renewal_unsigned(
                    2,
                    &fixture.policy,
                    &current,
                    &checkpoint,
                    &terms,
                    fixture.options.deadline,
                )
                .unwrap();
            previous
                .reserve_successor(
                    &fixture.owner,
                    unsigned,
                    &current,
                    authorization,
                    fixture.options.deadline,
                )
                .unwrap()
                .finish_pending_with_reads(
                    &fixture.owner,
                    &current,
                    &SigningTurn::Generated(authorization),
                    fixture.options.deadline,
                    &NativeEnrollmentReads(&fixture.native),
                )
                .unwrap()
        } else {
            let terms = Terms::new(now_ms().unwrap() + 4_000, &fixture.options).unwrap();
            let unsigned = fixture
                .owner
                .select_renewal_unsigned(
                    2,
                    &fixture.policy,
                    &current,
                    &checkpoint,
                    &terms,
                    fixture.options.deadline,
                )
                .unwrap();
            let history = BodyHistory::initialize(
                &fixture.owner,
                CustodyPurpose::Renewal(2),
                unsigned,
                &original_fees,
                &SigningTurn::RenewalSelection(&turn),
                fixture.options.deadline,
            )
            .unwrap();
            let authorization = turn
                .authorize_retained(&fixture.owner, &history, fixture.options.deadline)
                .unwrap();
            history
                .finish_pending_with_reads(
                    &fixture.owner,
                    &current,
                    &SigningTurn::Generated(authorization),
                    fixture.options.deadline,
                    &NativeEnrollmentReads(&fixture.native),
                )
                .unwrap()
        };
        assert_eq!(history.anchor.highest, ordinal);
        assert_eq!(history.bodies.len(), usize::from(ordinal));
        assert_eq!(history.anchor.active, Some(ordinal));
        assert!(!history.has_pending());
        let selected = fixture
            .retain_generated_attempt(&mut turn, &history, &current)
            .unwrap();
        assert!(matches!(
            selected.attempt().origin(),
            attempts::Origin::Generated { .. }
        ));
        assert!(selected.terms.fees == original_fees);
        let wallet = selected.directory().path().join("transaction");
        assert_eq!(
            selected
                .request(fixture.options.deadline)
                .unwrap()
                .inspect(&fixture.owner.wallet().unwrap(), &wallet)
                .unwrap()
                .phase(),
            NativePreparationPhase::RequestOnly
        );
        assert!(!wallet.join("payload.json").exists());
        assert!(!wallet.join("operation.json").exists());
        assert!(!wallet.join("submission.json").exists());
        originals.push(OriginalBytes {
            body: history.dispatch().unwrap().0.path().to_owned(),
            original: history
                .dispatch()
                .unwrap()
                .0
                .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
                .unwrap(),
            request: std::fs::read(wallet.join("preparation.json")).unwrap(),
        });
        drop(selected);
        drop(turn);
        let root = Arc::clone(&history.root);
        let container = Arc::clone(history.body_root.as_ref().unwrap());
        let oldest = Arc::clone(&history.bodies[0].directory);
        let reparsed = history.reopen(&fixture.owner).unwrap();
        assert!(Arc::ptr_eq(&root, &reparsed.root));
        assert!(Arc::ptr_eq(
            &container,
            reparsed.body_root.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(&oldest, &reparsed.bodies[0].directory));
        retained = Some(reparsed);
        assert_eq!(
            retained
                .as_ref()
                .unwrap()
                .current_history()
                .unwrap()
                .cumulative_reserved_count(),
            usize::from(ordinal)
        );
    }

    let history = retained.unwrap();
    assert_eq!(history.bodies.len(), 64);
    assert_eq!(
        history.current_history().unwrap().reserved_attempt_count(),
        1
    );
    assert_eq!(
        history
            .current_history()
            .unwrap()
            .cumulative_reserved_count(),
        64
    );
    require_full_census(history.current_history().unwrap());
    let anchor = history.root.read("anchor.nrt", MAX_BODY_BYTES).unwrap();
    let (body, _, _) = history.dispatch().unwrap();
    let attempt_names = body.open_child("attempts").unwrap().entries(64).unwrap();
    let last = history.current_history().unwrap().last().unwrap();
    assert!(matches!(
        history.current_history().unwrap().reserve(
            body,
            last.origin().clone(),
            last.terms().clone()
        ),
        Err(crate::managed::Error::Bootstrap(
            ManagedBootstrapFailure::EpochLimit
        ))
    ));
    assert_eq!(
        history.root.read("anchor.nrt", MAX_BODY_BYTES).unwrap(),
        anchor
    );
    assert_eq!(
        body.open_child("attempts").unwrap().entries(64).unwrap(),
        attempt_names
    );
    assert!(!body.path().join("attempts/0002").exists());
    assert!(!history.root.path().join("bodies/0065").exists());

    // Exercise the oldest original through the held deepest native graph, not a newly parsed
    // substitute or a constructed scope. Restoration is test corruption cleanup only.
    let oldest = &history.bodies[0].directory;
    oldest
        .write_atomic(
            "original.nrt",
            b"changed oldest original",
            PublishMode::Replace,
        )
        .unwrap();
    assert!(
        history
            .current_history()
            .unwrap()
            .test_require_current(128)
            .0
            .is_err()
    );
    assert!(BodyHistory::open(&fixture.owner, CustodyPurpose::Renewal(2)).is_err());
    oldest
        .write_atomic("original.nrt", &originals[0].original, PublishMode::Replace)
        .unwrap();
    require_full_census(history.current_history().unwrap());
    let closure = oldest.read("closed.nrt", 64 * 1024).unwrap();
    oldest
        .write_atomic(
            "closed.nrt",
            b"changed oldest closure",
            PublishMode::Replace,
        )
        .unwrap();
    assert!(
        history
            .current_history()
            .unwrap()
            .test_require_current(128)
            .0
            .is_err()
    );
    assert!(BodyHistory::open(&fixture.owner, CustodyPurpose::Renewal(2)).is_err());
    oldest
        .write_atomic("closed.nrt", &closure, PublishMode::Replace)
        .unwrap();
    require_full_census(history.current_history().unwrap());

    let account = fixture.owner.wallet().unwrap();
    for (index, original) in originals.iter().enumerate() {
        let body = &history.bodies[index];
        assert_eq!(body.directory.path(), original.body);
        assert_eq!(
            body.directory
                .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
                .unwrap(),
            original.original
        );
        let wallet = body.directory.path().join("attempts/0001/transaction");
        assert_eq!(
            std::fs::read(wallet.join("preparation.json")).unwrap(),
            original.request
        );
        assert!(!wallet.join("payload.json").exists());
        assert!(!wallet.join("operation.json").exists());
        assert!(!wallet.join("submission.json").exists());
        assert_eq!(wallet.join("retired.json").exists(), index < 63);
    }
    let selected = history.retained_selected(&fixture.owner).unwrap();
    assert_eq!(
        selected
            .request(fixture.options.deadline)
            .unwrap()
            .inspect(&account, &selected.directory().path().join("transaction"))
            .unwrap()
            .phase(),
        NativePreparationPhase::RequestOnly
    );
    assert_eq!(fixture.policy, original_policy);
    assert_eq!(
        std::fs::read(initial_carrier_path).unwrap(),
        initial_carrier
    );
    let (_, current) = fixture.current();
    assert_eq!(current.current().unwrap().record(), &original_native);
    assert_eq!(
        current
            .current()
            .unwrap()
            .control()
            .active_head
            .unwrap()
            .sequence,
        1
    );
    assert_eq!(current.current().unwrap().control().next_sequence, 2);
}
