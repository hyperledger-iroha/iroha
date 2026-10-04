//! Initial-body replacement through the original generated parent and genuine native execution.
//! These component carriers do not qualify running services or the complete bootstrap sequence.

use super::*;
use crate::managed::{
    ManagedInitialReservePolicy,
    native_operation::{
        authorization::DispatchAuthorization,
        test_support::{
            UnavailablePeers,
            native_fixture::{NativeFixture, NativeReadHttp, quote_instructions},
            provider_id,
        },
        verify_carrier,
    },
    service_bootstrap::ManagedServiceBootstrap,
    stream_token_custody::{
        bootstrap_test_support::NativeEnrollmentReads, renewal_tests::wait_until,
    },
};
use iroha_data_model::{
    asset::AssetDefinitionId,
    isi::{InstructionBox, Log},
    transaction::FeePaymentIntent,
};
use iroha_primitives::numeric::Quantity;
use iroha_wallet::operations::NativePreparationPhase;
use std::{collections::BTreeMap, num::NonZeroU64};

#[test]
fn expired_initial_body_uses_fresh_parent_authorization_and_preserves_original_native_custody() {
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "initial-body-retirement",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let provider = provider_id(&prepared, 0);
    let asset = AssetDefinitionId::parse_address_literal(
        crate::genesis::profile::TAIRA_XOR_ASSET_DEFINITION_ID,
    )
    .unwrap();
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(64)),
        max_total_fees: BTreeMap::from([(asset, Quantity::from(1_000u64))]),
        deadline: Instant::now() + Duration::from_secs(300),
    };
    let mut parent = ManagedServiceBootstrap::open(&prepared).unwrap();
    let initial_authorization = parent.authorize_test_startup(&options).unwrap().unwrap();
    let first_ordinal = initial_authorization.test_ordinal();
    let policies = initial_authorization
        .test_child(Purpose::CustodyConfigure(provider))
        .unwrap()
        .policies()
        .clone();
    let policy = policies.provider(provider).unwrap().custody.clone();
    let mut owner = ManagedStreamTokenCustody::open(&prepared, provider).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &owner.authority);
    let log = quote_instructions(
        &native,
        &owner.authority.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "initial body prerequisites".into(),
        ))],
    );
    assert_eq!(native.chain.commit(vec![log]), vec![true]);
    assert_eq!(native.chain.height(), 2);
    drop(ports);
    // Preserve the real parent dependency order, including its network reserve prerequisite.
    let mut reserve = ManagedInitialReservePolicy::open(&prepared).unwrap();
    let reserve_result = reserve.bootstrap_native_generated(
        &mut native,
        &policies.network.reserve,
        &initial_authorization
            .test_child(Purpose::ReservePolicy)
            .unwrap(),
        &options,
    );
    assert_eq!(reserve_result.finalized.unwrap().height, 3);
    assert!(reserve_result.current.is_none());
    drop(reserve);
    let configured = owner.bootstrap_native_configure_generated(
        &mut native,
        &policy,
        &initial_authorization
            .test_child(Purpose::CustodyConfigure(provider))
            .unwrap(),
        &options,
    );
    let configured_finality = configured.finalized.unwrap();
    assert_eq!(configured_finality.height, 4);
    assert!(configured.current.is_none());
    let configured_root = owner.authority.directory.open_child("configure").unwrap();
    let configured_original = journal::required_original(&configured_root).unwrap();
    let configured_body = configured_root
        .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
        .unwrap();
    let configured_carrier = configured_original
        .directory()
        .read("carrier.nrt", MAX_CHECKPOINT_BYTES)
        .unwrap();
    let configured_operation = std::fs::read(
        configured_original
            .directory()
            .path()
            .join("transaction/operation.json"),
    )
    .unwrap();

    let checkpoint = native.observe(&owner.authority);
    let current = native.bootstrap_custody(&owner.authority, &policy, &checkpoint);
    let state = current.current().unwrap();
    assert_eq!(state.control().policy, policy);
    assert_eq!(state.record().execution_height, 4);
    assert_eq!(state.control().next_sequence, 1);
    assert!(state.control().active_head.is_none());
    let now = now_ms().unwrap();
    let short = ManagedCustodyEnrollmentInterval {
        issued_at_unix_ms: now,
        expires_at_unix_ms: now + 6_000,
        deadline_unix_ms: now + 3_000,
    };
    assert!(short.expires_at_unix_ms - short.issued_at_unix_ms < policy.max_validity_ms);
    let unsigned = owner
        .unsigned_enrollment(&policy, &current, &checkpoint, short, now)
        .unwrap();
    let original_history = owner.bootstrap_native_body(
        &native,
        CustodyPurpose::InitialEnroll,
        unsigned,
        short.deadline_unix_ms,
        &options,
    );
    let (body, original, scope) = original_history.dispatch().unwrap();
    let account = owner.wallet().unwrap();
    journal::explicit(
        body,
        original,
        short.deadline_unix_ms,
        &options,
        &account,
        scope,
    )
    .unwrap();
    let old = owner
        .required_enrollment(CustodyPurpose::InitialEnroll)
        .unwrap();
    let old_request = old.request(options.deadline).unwrap();
    let old_wallet = old.directory().path().join("transaction");
    let request_bytes = std::fs::read(old_wallet.join("preparation.json")).unwrap();
    let body_bytes = body
        .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
        .unwrap();
    let reserved_bytes = body.read("reserved.nrt", MAX_BODY_BYTES).unwrap();
    let outer_bytes = original_history
        .root
        .read("original.nrt", MAX_SELECTION_BYTES)
        .unwrap();
    assert_eq!(
        old_request.inspect(&account, &old_wallet).unwrap().phase(),
        NativePreparationPhase::RequestOnly
    );
    assert!(!old_wallet.join("payload.json").exists());
    assert!(!old_wallet.join("operation.json").exists());
    wait_until(short.expires_at_unix_ms, Duration::from_secs(8));
    let mut peers = UnavailablePeers::start(&prepared);
    let expired = owner
        .recover_enroll_selected_if_present(
            &policy,
            &Fees::from_options(&options).unwrap(),
            options.deadline,
        )
        .unwrap()
        .unwrap();
    assert_eq!(expired.transaction_status, OperationStatus::Expired);
    assert!(expired.finalized.is_none() && expired.current.is_none());
    // A new invocation goes through the actual parent census after releasing the child lock.
    drop(owner);
    drop(initial_authorization);
    let authorization = parent.authorize_test_startup(&options).unwrap().unwrap();
    assert_eq!(authorization.test_ordinal(), first_ordinal + 1);
    let child = authorization
        .test_child(Purpose::CustodyEnroll(provider))
        .unwrap();
    assert_eq!(child.policies().provider(provider).unwrap().custody, policy);
    let mut owner = ManagedStreamTokenCustody::open(&prepared, provider).unwrap();
    child
        .validate(
            &owner.authority,
            Purpose::CustodyEnroll(provider),
            options.deadline,
        )
        .unwrap();
    let terms = child
        .terms(options.deadline, Some(policy.active_until_unix_ms))
        .unwrap();
    let now = now_ms().unwrap();
    let interval = child
        .policies()
        .provider(provider)
        .unwrap()
        .initial_enrollment(now, terms.requested_deadline_unix_ms)
        .unwrap();
    let checkpoint = native.observe(&owner.authority);
    let current = native.bootstrap_custody(&owner.authority, &policy, &checkpoint);
    assert_eq!(current.current().unwrap().record(), state.record());
    let unsigned = owner
        .unsigned_enrollment(&policy, &current, &checkpoint, interval, now)
        .unwrap();
    let history = BodyHistory::open(&owner, CustodyPurpose::InitialEnroll)
        .unwrap()
        .unwrap()
        .reserve_successor(&owner, unsigned, &current, &child, options.deadline)
        .unwrap()
        .finish_pending_with_reads(
            &owner,
            &current,
            &SigningTurn::Generated(&child),
            options.deadline,
            &NativeEnrollmentReads(&native),
        )
        .unwrap();
    assert_eq!(history.anchor.highest, 2);
    assert_eq!(
        history
            .current_history()
            .unwrap()
            .cumulative_reserved_count(),
        1
    );
    assert_eq!(
        old_request.inspect(&account, &old_wallet).unwrap().phase(),
        NativePreparationPhase::Retired
    );
    let (operation, original, scope) = history.dispatch().unwrap();
    let account = child.bind_account(owner.wallet().unwrap()).unwrap();
    let historical = native.bootstrap_custody(&owner.authority, &policy, &checkpoint);
    attempts::generated(
        operation,
        Purpose::CustodyEnroll(provider),
        original.digest().unwrap(),
        scope,
        &child,
        options.deadline,
        Some(interval.expires_at_unix_ms),
        |attempt| {
            original
                .request(attempt.terms(), attempt.observation()?, options.deadline)?
                .inspect(&account, &attempt.wallet_path())
        },
        |attempt| {
            original
                .request(attempt.terms(), attempt.observation()?, options.deadline)?
                .retire(&account, &attempt.wallet_path())
        },
        |attempt, observation, deadline| {
            original
                .request(attempt.terms(), observation, deadline)?
                .retain(&account, &attempt.wallet_path())
        },
        |_, deadline| {
            let cut = native.observe(&owner.authority);
            let current = native.bootstrap_custody(&owner.authority, &policy, &cut);
            owner.fresh_predecessor_observation(original, &historical, &current, deadline)
        },
        |attempt| {
            let observed = attempt
                .observation()?
                .enrollment_observed_at_unix_ms
                .ok_or_else(|| invalid("initial dispatch observation absent"))?;
            let now = now_ms()?;
            Ok(observed <= now && now - observed <= policy.max_anchor_age_ms)
        },
    )
    .unwrap();
    let selected = owner
        .required_enrollment(CustodyPurpose::InitialEnroll)
        .unwrap();
    assert!(matches!(
        selected.attempt().origin(),
        attempts::Origin::Generated { .. }
    ));
    assert!(selected.attempt().origin() == &child.origin().unwrap());
    assert_ne!(selected.directory().path(), old.directory().path());
    assert_eq!(
        BodyHistory::open(&owner, CustodyPurpose::InitialEnroll)
            .unwrap()
            .unwrap()
            .current_history()
            .unwrap()
            .cumulative_reserved_count(),
        2
    );
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();

    let path = selected.directory().path().join("transaction");
    let mut http =
        NativeReadHttp::start_config(&owner.authority.config, Arc::clone(native.chain.state()));
    let journal::Request::Enroll(request) = selected.request(options.deadline).unwrap() else {
        unreachable!()
    };
    account
        .prepare_stream_token_custody_enroll(&request, &path)
        .unwrap();
    let signed = owner
        .verify_wallet(selected.directory(), &selected, options.deadline)
        .unwrap();
    http.finish();
    let wire = signed.encode_wire_v1().unwrap();
    let carrier = native.bootstrap_commit(&owner.authority, &signed);
    let finalized = verify_carrier(&carrier, &signed).unwrap();
    assert_eq!(finalized.height, 5);
    selected
        .directory()
        .write_atomic(
            "carrier.nrt",
            &checkpoint_bytes(&carrier).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    let current = native.bootstrap_custody(&owner.authority, &policy, &carrier);
    let state = current.current().unwrap();
    assert_eq!(state.control().policy, policy);
    assert_eq!(state.control().active_head.unwrap().sequence, 1);
    assert_eq!(state.control().next_sequence, 2);
    let retained = owner
        .retained_initial_enrollment(&policy, selected.interval().unwrap(), options.deadline)
        .unwrap();
    assert_eq!(retained.finalized(), &finalized);
    owner
        .verify_enrollment_at(
            &retained,
            &policy,
            finalized.height,
            *finalized.block_hash.as_ref(),
            &current,
            now_ms().unwrap(),
            options.deadline,
        )
        .unwrap();
    let mut peers = UnavailablePeers::start(&prepared);
    let recovered = owner
        .recover_enroll_selected_if_present(&policy, child.fees(), options.deadline)
        .unwrap()
        .unwrap();
    assert_eq!(recovered.transaction_status, OperationStatus::Applied);
    assert_eq!(recovered.finalized, Some(finalized));
    assert!(recovered.current.is_none());
    assert_eq!(
        owner
            .verify_wallet(selected.directory(), &selected, options.deadline)
            .unwrap()
            .encode_wire_v1()
            .unwrap(),
        wire
    );
    assert_eq!(
        body.read("original.nrt", journal::MAX_ORIGINAL_BYTES)
            .unwrap(),
        body_bytes
    );
    assert_eq!(
        body.read("reserved.nrt", MAX_BODY_BYTES).unwrap(),
        reserved_bytes
    );
    assert_eq!(
        history
            .root
            .read("original.nrt", MAX_SELECTION_BYTES)
            .unwrap(),
        outer_bytes
    );
    assert_eq!(
        std::fs::read(old_wallet.join("preparation.json")).unwrap(),
        request_bytes
    );
    assert!(
        !old_wallet.join("payload.json").exists() && !old_wallet.join("operation.json").exists()
    );
    assert!(!old_wallet.join("submission.json").exists() && !path.join("submission.json").exists());
    assert_eq!(
        configured_root
            .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
            .unwrap(),
        configured_body
    );
    assert_eq!(
        configured_original
            .directory()
            .read("carrier.nrt", MAX_CHECKPOINT_BYTES)
            .unwrap(),
        configured_carrier
    );
    assert_eq!(
        std::fs::read(
            configured_original
                .directory()
                .path()
                .join("transaction/operation.json")
        )
        .unwrap(),
        configured_operation
    );
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}
