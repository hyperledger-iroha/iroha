//! Genuine generated Begin source refusal, lexical scope and original outer admission.

use super::*;
use crate::managed::native_operation::test_support::UnavailablePeers;
use crate::managed::stream_token_custody::renewal_tests::{Fixture, wait_until};
use std::path::PathBuf;

fn fixture_with_renewal_body() -> Fixture {
    let fixture = Fixture::enrolled(4_000);
    wait_until(
        fixture.initial.issued_at_unix_ms + 2_000,
        Duration::from_secs(4),
    );
    let (checkpoint, current) = fixture.current();
    let utc = now_ms().unwrap() + 60_000;
    let terms = Terms::new(utc, &fixture.options).unwrap();
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
    let history = fixture.owner.bootstrap_native_body(
        &fixture.native,
        &current,
        CustodyPurpose::Renewal(2),
        unsigned,
        utc,
        &fixture.options,
    );
    assert_eq!(
        history.purpose(),
        Purpose::CustodyRenewal {
            provider: fixture.owner.authority.provider_id().unwrap(),
            sequence: 2,
        }
    );
    assert_eq!(fixture.native.chain.height(), 4);
    fixture
}

fn begin(
    fixture: &Fixture,
    floor: ManagedTransactionFinality,
    scoped: bool,
    deadline: Instant,
) -> Result<GeneratedRenewalTurn> {
    let fees = Fees::from_options(&fixture.options)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    if scoped {
        GeneratedRenewalTurn::begin(
            &fixture.owner,
            &fixture.policy,
            fees,
            floor,
            deadline,
            cancelled,
        )
    } else {
        // Exactly the same canonical Begin and inventory gates with independent imports.
        GeneratedRenewalTurn::begin_with_imports(
            &fixture.owner,
            &fixture.policy,
            fees,
            floor,
            deadline,
            cancelled,
            &mut CheckpointImports::new(&fixture.owner.authority, None),
        )
    }
}

fn assert_same(actual: &GeneratedRenewalTurn, expected: &GeneratedRenewalTurn) {
    assert_eq!(actual.prepared, expected.prepared);
    assert_eq!(actual.provider, expected.provider);
    assert_eq!(actual.policy, expected.policy);
    assert!(actual.fees == expected.fees);
    assert_eq!(actual.floor, expected.floor);
    assert_eq!(actual.deadline, expected.deadline);
    assert!(actual.authorization.is_none() && expected.authorization.is_none());
    assert!(!actual.issuance_consumed && !expected.issuance_consumed);
    assert!(!actual.cancelled.load(std::sync::atomic::Ordering::Acquire));
    assert!(!norito::core::decode_limits_active());
}

fn retained_sources(fixture: &Fixture) -> Vec<(PathBuf, &'static str, usize)> {
    let configured = fixture
        .owner
        .authority
        .directory
        .open_child("configure")
        .unwrap();
    let configured = journal::required_original(&configured).unwrap();
    let initial = BodyHistory::open(&fixture.owner, CustodyPurpose::InitialEnroll)
        .unwrap()
        .unwrap()
        .into_selected()
        .unwrap();
    let renewal = BodyHistory::open(&fixture.owner, CustodyPurpose::Renewal(2))
        .unwrap()
        .unwrap();
    vec![
        (
            configured.directory().path().to_path_buf(),
            "carrier.nrt",
            MAX_CHECKPOINT_BYTES,
        ),
        (
            initial.directory().path().to_path_buf(),
            "carrier.nrt",
            MAX_CHECKPOINT_BYTES,
        ),
        (
            initial.directory().path().join("transaction"),
            "operation.json",
            4 * 1024 * 1024,
        ),
        (
            renewal.root().path().join("bodies/0001"),
            "reserved.nrt",
            journal::MAX_ORIGINAL_BYTES + 128 * 1024,
        ),
    ]
}

#[test]
fn begin_epoch_workspace_preserves_paid_sources_inventory_error_order_and_retry() {
    let _guard = crate::managed::native_test_guard();
    let fixture = fixture_with_renewal_body();
    let mut peers = UnavailablePeers::start(&fixture.prepared);
    let deadline = fixture.options.deadline;
    // Capture the already authenticated initial floor before mutation or budget measurement.
    let floor = *fixture
        .owner
        .retained_initial_enrollment(&fixture.policy, fixture.initial, deadline)
        .unwrap()
        .finalized();
    let expected = begin(&fixture, floor, false, deadline).unwrap();
    assert_same(&begin(&fixture, floor, true, deadline).unwrap(), &expected);
    // These original paid Configure/Enroll and reserved renewal records stay independent
    // native reads even after the original exact checkpoint byte cache has been warmed.
    for (path, name, maximum) in retained_sources(&fixture) {
        let directory = PrivateDirectory::open_exact(&path).unwrap();
        let bytes = directory.read(name, maximum).unwrap();
        directory
            .write_atomic(name, &[0xff], PublishMode::Replace)
            .unwrap();
        let original = begin(&fixture, floor, false, deadline)
            .err()
            .expect("original source refuses");
        let actual = begin(&fixture, floor, true, deadline)
            .err()
            .expect("shared source refuses");
        assert_eq!(actual.to_string(), original.to_string());
        directory
            .write_atomic(name, &bytes, PublishMode::Replace)
            .unwrap();
        assert_eq!(directory.read(name, maximum).unwrap(), bytes);
        assert_same(&begin(&fixture, floor, true, deadline).unwrap(), &expected);
    }
    // A present later renewal namespace cannot be skipped after validating sequence two.
    let unexpected = fixture
        .owner
        .authority
        .directory
        .create_child(&CustodyPurpose::Renewal(3).directory_name().unwrap())
        .unwrap();
    let original = begin(&fixture, floor, false, deadline)
        .err()
        .expect("original full census refuses");
    let actual = begin(&fixture, floor, true, deadline)
        .err()
        .expect("shared full census refuses");
    assert_eq!(actual.to_string(), original.to_string());
    unexpected.remove_empty().unwrap();
    assert_same(&begin(&fixture, floor, true, deadline).unwrap(), &expected);
    let expired = Instant::now();
    assert!(matches!(
        begin(&fixture, floor, true, expired),
        Err(crate::managed::Error::NativeDeadline)
    ));
    assert_eq!(
        begin(&fixture, floor, true, expired)
            .err()
            .unwrap()
            .to_string(),
        begin(&fixture, floor, false, expired)
            .err()
            .unwrap()
            .to_string()
    );
    assert_same(&begin(&fixture, floor, true, deadline).unwrap(), &expected);
    assert_eq!(fixture.native.chain.height(), 4);
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}

#[test]
fn begin_epoch_workspace_preserves_late_finite_outer_charge_refusal_and_retry() {
    use norito::core::DecodeBudgetContext;
    fn limits(allocation: usize) -> norito::DecodeLimits {
        norito::DecodeLimits::new(
            1024 * 1024,
            MAX_CHECKPOINT_BYTES,
            8 * 1024 * 1024,
            allocation,
            64,
        )
    }
    let _guard = crate::managed::native_test_guard();
    let fixture = fixture_with_renewal_body();
    let mut peers = UnavailablePeers::start(&fixture.prepared);
    let deadline = fixture.options.deadline;
    // Capture the already authenticated initial floor before mutation or budget measurement.
    let floor = *fixture
        .owner
        .retained_initial_enrollment(&fixture.policy, fixture.initial, deadline)
        .unwrap()
        .finalized();
    let warm = begin(&fixture, floor, true, deadline).unwrap();
    let ceiling = 256 * 1024 * 1024;
    let baseline = DecodeBudgetContext::new(limits(ceiling));
    let expected = baseline
        .with(|| begin(&fixture, floor, false, deadline))
        .unwrap();
    let charge = baseline.consumed_allocated_bytes();
    assert!(charge > 0 && charge < ceiling as u64);
    let exact = DecodeBudgetContext::new(limits(usize::try_from(charge).unwrap()));
    let actual = exact
        .with(|| begin(&fixture, floor, true, deadline))
        .unwrap();
    assert_eq!(exact.consumed_allocated_bytes(), charge);
    assert_same(&actual, &expected);
    assert_same(&actual, &warm);
    for allocation in [0, 1] {
        let original = DecodeBudgetContext::new(limits(allocation));
        let expected = original
            .with(|| begin(&fixture, floor, false, deadline))
            .err()
            .expect("original budget refuses");
        let budget = DecodeBudgetContext::new(limits(allocation));
        let actual = budget
            .with(|| begin(&fixture, floor, true, deadline))
            .err()
            .expect("shared budget refuses");
        assert_eq!(actual.to_string(), expected.to_string());
        assert_eq!(
            budget.consumed_allocated_bytes(),
            original.consumed_allocated_bytes()
        );
        assert!(!norito::core::decode_limits_active());
        assert_same(&begin(&fixture, floor, true, deadline).unwrap(), &warm);
    }
    assert_eq!(fixture.native.chain.height(), 4);
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}
