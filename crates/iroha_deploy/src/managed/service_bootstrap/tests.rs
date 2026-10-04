//! Parent intent custody and real HTTP refusal; these tests never stand in for native completion.

use super::*;
use crate::managed::native_operation::{now_ms, test_support::UnavailablePeers};
use iroha_data_model::transaction::FeePaymentIntent;
use std::sync::{Arc, atomic::AtomicBool};
use std::{collections::BTreeMap, io, time::Duration};

fn fixture(name: &str) -> (tempfile::TempDir, PreparedLocalnet) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        name,
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    (temporary, prepared)
}

fn options() -> BoundedTransactionOptions {
    BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::new(),
        deadline: Instant::now() + Duration::from_secs(90),
    }
}

fn retain(owner: &ManagedServiceBootstrap, original: &Original) -> PrivateDirectory {
    original.validate(&owner.authority).unwrap();
    let directory = owner.authority.directory.ensure_child("initial").unwrap();
    directory
        .write_atomic(
            "original.nrt",
            &encode(original, MAX_ORIGINAL_BYTES).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    directory
}

#[test]
fn bootstrap_retains_original_before_unavailable_preflight_and_reopens_without_reselection() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("bootstrap-original");
    let mut peers = UnavailablePeers::start(&prepared);
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    assert!(ManagedServiceBootstrap::open(&prepared).is_err());
    let opts = options();
    let authorization = owner.authorize_test_startup(&opts).unwrap().unwrap();
    let original_terms = encode(authorization.test_terms(), 64 * 1024).unwrap();
    assert_eq!(authorization.test_ordinal(), 1);
    let error = owner.advance(&authorization, opts.deadline).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cannot read original genesis result")
    );
    let directory = owner.authority.directory.open_child("initial").unwrap();
    let bytes = directory
        .read("original.nrt", MAX_ORIGINAL_BYTES)
        .unwrap()
        .to_vec();
    let original = read_original(&directory, &owner.authority)
        .unwrap()
        .unwrap();
    assert_eq!(
        encode(&owner.selected_policies().unwrap(), MAX_ORIGINAL_BYTES).unwrap(),
        encode(&original.policies, MAX_ORIGINAL_BYTES).unwrap()
    );
    assert!(original.fees == Fees::from_options(&opts).unwrap());
    assert!(authorization.test_terms().signing_deadline_unix_ms > now_ms().unwrap());
    assert!(
        authorization.test_terms().requested_deadline_unix_ms
            >= authorization.test_terms().signing_deadline_unix_ms
    );
    for (slot, plan) in prepared
        .provider_service_plans()
        .unwrap()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(&original.underwriting[slot], plan.reserve_terms());
        assert_eq!(
            original.policies.providers[slot].provider_id,
            plan.provider_id()
        );
        assert_eq!(original.policies.providers[slot].slot as usize, slot);
    }
    peers.finish();
    let count = peers.requests.lock().unwrap().len();
    assert!(count >= 4);
    for request in peers.requests.lock().unwrap().iter() {
        assert_eq!(request.method, "GET");
        assert!(matches!(
            request.path.as_str(),
            "/v1/node/capabilities" | "/v1/bridge/finality/1"
        ));
    }
    // The child may create an empty preflight directory. It cannot invent original proof or a wallet.
    let reserve = ManagedInitialReservePolicy::open(&prepared).unwrap();
    drop(reserve);
    let generation =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let child = generation
        .open_child("runtime")
        .unwrap()
        .open_child("service-operations")
        .unwrap()
        .open_child("network")
        .unwrap()
        .open_child("initial-reserve-policy")
        .unwrap()
        .open_child("set")
        .unwrap();
    require_empty(&child).unwrap();
    drop(owner);
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let recovered = owner
        .recover(Instant::now() + Duration::from_secs(5))
        .unwrap();
    assert!(matches!(
        recovered,
        ServiceBootstrapProgress::Pending {
            step: ServiceBootstrapStep::ReservePolicy,
            status: OperationStatus::Absent
        }
    ));
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        bytes.as_slice()
    );
    assert_eq!(peers.requests.lock().unwrap().len(), count);
    assert_eq!(
        child.open_child("transaction").err().unwrap().kind(),
        io::ErrorKind::NotFound
    );
    // A fresh invocation may authorize another unsigned epoch, never rewrite semantic intent.
    let replacement = owner.authorize_test_startup(&options()).unwrap().unwrap();
    assert_eq!(replacement.test_ordinal(), authorization.test_ordinal() + 1);
    assert_eq!(
        encode(authorization.test_terms(), 64 * 1024).unwrap(),
        original_terms
    );
    assert!(owner.advance(&authorization, options().deadline).is_err());
    let mut changed = options();
    changed.fee_payment = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
    assert!(owner.authorize_test_startup(&changed).is_err());
    assert_eq!(peers.requests.lock().unwrap().len(), count);
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        bytes.as_slice()
    );
}

#[test]
fn readonly_and_expired_bootstrap_never_create_child_transactions_or_renew_original() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("bootstrap-readonly");
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let mut opts = options();
    opts.deadline = Instant::now() + Duration::from_secs(2);
    let authorization = owner.authorize_test_startup(&opts).unwrap().unwrap();
    let directory = owner.authority.directory.open_child("initial").unwrap();
    let original = read_original(&directory, &owner.authority)
        .unwrap()
        .unwrap();
    let retained = encode(&original, MAX_ORIGINAL_BYTES).unwrap();
    let epoch_root = directory.open_child("epochs").unwrap();
    let epoch_names = epoch_root.entries(64).unwrap();
    let epoch_bytes = epoch_root.read("0001.nrt", 64 * 1024).unwrap();
    let terms_bytes = encode(authorization.test_terms(), 64 * 1024).unwrap();
    let mut peers = UnavailablePeers::start(&prepared);
    assert!(matches!(
        owner.recover(options().deadline).unwrap(),
        ServiceBootstrapProgress::Pending {
            step: ServiceBootstrapStep::ReservePolicy,
            status: OperationStatus::Absent
        }
    ));
    assert!(peers.requests.lock().unwrap().is_empty());
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        retained.as_slice()
    );

    // Let the actually issued epoch expire. Neither recovery nor an advance with the old
    // capability may mint another epoch, rewrite UTC, or fabricate an enrollment body.
    let utc = authorization.test_terms().signing_deadline_unix_ms;
    let wait_limit = Instant::now() + Duration::from_secs(3);
    loop {
        let now = now_ms().unwrap();
        if now >= utc {
            break;
        }
        assert!(
            Instant::now() < wait_limit,
            "original UTC did not elapse within bounded wait"
        );
        std::thread::sleep(Duration::from_millis((utc - now).min(20)));
    }
    assert!(matches!(
        owner.recover(options().deadline).unwrap(),
        ServiceBootstrapProgress::Pending {
            step: ServiceBootstrapStep::ReservePolicy,
            status: OperationStatus::Absent
        }
    ));
    let error = owner
        .advance(&authorization, options().deadline)
        .unwrap_err();
    assert!(matches!(
        error,
        crate::managed::Error::Bootstrap(
            crate::managed::ManagedBootstrapFailure::AuthorizationExpired
        )
    ));
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        retained.as_slice()
    );
    assert_eq!(epoch_root.entries(64).unwrap(), epoch_names);
    assert_eq!(epoch_root.read("0001.nrt", 64 * 1024).unwrap(), epoch_bytes);
    assert_eq!(
        encode(authorization.test_terms(), 64 * 1024).unwrap(),
        terms_bytes
    );
    peers.finish();
    assert!(peers.requests.lock().unwrap().is_empty());
    let after = read_original(&directory, &owner.authority)
        .unwrap()
        .unwrap();
    assert!(after.fees == original.fees);
    assert_eq!(
        encode(&after.policies, MAX_ORIGINAL_BYTES).unwrap(),
        encode(&original.policies, MAX_ORIGINAL_BYTES).unwrap()
    );
    assert_eq!(encode(&after, MAX_ORIGINAL_BYTES).unwrap(), retained);
    let generation =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let network = generation
        .open_child("runtime")
        .unwrap()
        .open_child("service-operations")
        .unwrap()
        .open_child("network")
        .unwrap();
    assert_eq!(
        network
            .open_child("initial-reserve-policy")
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::NotFound
    );
}

#[test]
fn original_partial_dirty_corrupt_and_foreign_generation_intents_are_refused() {
    let _guard = crate::managed::native_test_guard();
    let (_first, prepared) = fixture("bootstrap-exact");
    let owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    assert!(owner.selected_policies().is_err());
    let directory = owner.authority.directory.ensure_child("initial").unwrap();
    assert!(
        read_original(&directory, &owner.authority)
            .unwrap()
            .is_none()
    );
    directory
        .write_atomic("partial.nrt", b"not an original", PublishMode::CreateNew)
        .unwrap();
    assert!(read_original(&directory, &owner.authority).is_err());
    let original =
        Original::select(&owner.authority, Fees::from_options(&options()).unwrap()).unwrap();
    let bytes = encode(&original, MAX_ORIGINAL_BYTES).unwrap();
    directory
        .write_atomic("original.nrt", &bytes, PublishMode::CreateNew)
        .unwrap();
    original.validate(&owner.authority).unwrap();
    let mut truncated = bytes.clone();
    truncated.pop();
    directory
        .write_atomic("original.nrt", &truncated, PublishMode::Replace)
        .unwrap();
    assert!(read_original(&directory, &owner.authority).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    directory
        .write_atomic("original.nrt", &trailing, PublishMode::Replace)
        .unwrap();
    assert!(read_original(&directory, &owner.authority).is_err());
    let (_second, other) = fixture("bootstrap-foreign");
    let other = ManagedServiceBootstrap::open(&other).unwrap();
    assert_ne!(
        owner.authority.config.network_id,
        other.authority.config.network_id
    );
    assert!(original.validate(&other.authority).is_err());
    let mut changed = original.clone();
    changed.underwriting[0].capacity_gib += 1;
    assert!(changed.validate(&owner.authority).is_err());
    let mut changed = original;
    changed.profile[0] ^= 1;
    assert!(changed.validate(&owner.authority).is_err());
}

#[test]
fn carrier_order_check_never_accepts_equal_or_older_prerequisite() {
    // Arithmetic-only guard; these synthetic fields are never supplied to an actual child owner.
    let record = ManagedTransactionFinality {
        transaction_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"transaction",
        )),
        block_hash: iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"block")),
        height: 3,
        block_time_ms: 100,
    };
    require_after(&record, 2).unwrap();
    assert!(require_after(&record, 3).is_err());
    assert!(require_after(&record, 4).is_err());
}

#[test]
fn prepare_only_parent_retains_exact_original_without_http_or_child_custody() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("bootstrap-prepare-only");
    let mut peers = UnavailablePeers::start(&prepared);
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let opts = options();
    let first = owner.authorize_test_startup(&opts).unwrap().unwrap();
    let first_terms = encode(first.test_terms(), 64 * 1024).unwrap();
    let original = owner
        .authority
        .directory
        .open_child("initial")
        .unwrap()
        .read("original.nrt", MAX_ORIGINAL_BYTES)
        .unwrap()
        .to_vec();
    let generation =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let runtime = generation.open_child("runtime").unwrap();
    let operations = runtime.open_child("service-operations").unwrap();
    assert!(operations.open_child("providers").is_err());
    assert!(
        operations
            .open_child("network")
            .unwrap()
            .open_child("initial-reserve-policy")
            .is_err()
    );
    drop(owner);
    let mut reopened = ManagedServiceBootstrap::open(&prepared).unwrap();
    let mut replay = opts.clone();
    replay.deadline = Instant::now() + Duration::from_secs(300);
    let second = reopened.authorize_test_startup(&replay).unwrap().unwrap();
    assert_eq!(second.test_ordinal(), first.test_ordinal() + 1);
    assert_eq!(encode(first.test_terms(), 64 * 1024).unwrap(), first_terms);
    assert_eq!(
        reopened
            .authority
            .directory
            .open_child("initial")
            .unwrap()
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        original
    );
    let mut expired = replay.clone();
    expired.deadline = Instant::now() - Duration::from_secs(1);
    assert!(reopened.authorize_test_startup(&expired).is_err());
    let mut wrong = replay.clone();
    wrong.fee_payment = FeePaymentIntent::authority(Vec::new(), std::num::NonZeroU64::new(1));
    assert!(reopened.authorize_test_startup(&wrong).is_err());
    assert!(
        !reopened
            .selected_policies()
            .unwrap()
            .network
            .runtime_fee_payment
            .charge_limits()
            .is_empty()
    );
    peers.finish();
    assert!(peers.requests.lock().unwrap().is_empty());
}

#[test]
fn automatic_bootstrap_preparation_preserves_original_deadline_and_bounded_fees_without_http() {
    let _guard = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture("bootstrap-automatic");
    let mut peers = UnavailablePeers::start(&prepared);
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let first = owner
        .authorize_generated_startup(deadline, Arc::new(AtomicBool::new(false)))
        .unwrap()
        .unwrap();
    let first_terms = encode(first.test_terms(), 64 * 1024).unwrap();
    let directory = owner.authority.directory.open_child("initial").unwrap();
    let original = read_original(&directory, &owner.authority)
        .unwrap()
        .unwrap();
    let bytes = encode(&original, MAX_ORIGINAL_BYTES).unwrap();
    let options = original.fees.options(deadline);
    assert_eq!(options.max_total_fees.len(), 1);
    assert_eq!(
        options.max_total_fees.values().next(),
        Some(&iroha_primitives::numeric::Quantity::from(1_u64))
    );
    assert_eq!(
        options.fee_payment,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None)
    );
    assert!(first.test_terms().signing_deadline_unix_ms > now_ms().unwrap());
    drop(owner);
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let second = owner
        .authorize_generated_startup(
            deadline + Duration::from_secs(60),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(second.test_ordinal(), first.test_ordinal() + 1);
    assert_eq!(encode(first.test_terms(), 64 * 1024).unwrap(), first_terms);
    assert!(
        second.test_terms().signing_deadline_unix_ms > first.test_terms().signing_deadline_unix_ms
    );
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        bytes.as_slice()
    );
    assert!(
        owner
            .authorize_generated_startup(
                Instant::now() - Duration::from_secs(1),
                Arc::new(AtomicBool::new(false))
            )
            .is_err()
    );
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        bytes.as_slice()
    );
    peers.finish();
    assert!(peers.requests.lock().unwrap().is_empty());
}

#[test]
fn aggregate_original_pins_each_scope_interval_and_fee_terms_without_changing_semantic_policy() {
    let _guard = crate::managed::native_test_guard();
    let (_root, prepared) = fixture("bootstrap-all-scopes");
    let owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let opts = options();
    let original = Original::select(&owner.authority, Fees::from_options(&opts).unwrap()).unwrap();
    let directory = retain(&owner, &original);
    let retained = encode(&original, MAX_ORIGINAL_BYTES).unwrap();
    for slot in 0..3 {
        let id = original.policies.providers[slot].provider_id;
        assert_eq!(
            owner
                .selected_policies()
                .unwrap()
                .provider(id)
                .unwrap()
                .slot as usize,
            slot
        );
        let mut wrong = original.clone();
        wrong.policies.providers[slot].custody.max_validity_ms += 1;
        assert!(wrong.validate(&owner.authority).is_err());
        let mut wrong = original.clone();
        wrong.policies.providers[slot].custody.active_from_unix_ms += 1;
        assert!(wrong.validate(&owner.authority).is_err());
        let mut wrong = original.clone();
        wrong.policies.providers[slot].custody.active_until_unix_ms -= 1;
        assert!(wrong.validate(&owner.authority).is_err());
        let mut wrong = original.clone();
        wrong.underwriting[slot].capacity_gib += 1;
        assert!(wrong.validate(&owner.authority).is_err());
    }
    let mut wrong = original.clone();
    wrong.profile[0] ^= 1;
    assert!(wrong.validate(&owner.authority).is_err());
    let mut wrong = original.clone();
    wrong.policies.providers.swap(0, 1);
    assert!(wrong.validate(&owner.authority).is_err());
    let policies = GeneratedServicePolicies::select(&owner.authority).unwrap();
    assert_eq!(
        encode(&policies, MAX_ORIGINAL_BYTES).unwrap(),
        encode(&original.policies, MAX_ORIGINAL_BYTES).unwrap()
    );
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        retained
    );
}
