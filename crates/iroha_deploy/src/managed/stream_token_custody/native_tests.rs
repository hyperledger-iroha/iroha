//! Original custody wallets and native carriers from the actual generated execution fixture.
//! Direct component commits do not qualify full coordinator HTTP or running service readiness.

use super::*;
use crate::managed::native_operation::{
    test_support::{
        UnavailablePeers,
        native_fixture::{NativeFixture, NativeReadHttp, quote_instructions},
    },
    verify_carrier,
};
use iroha_core::state::{AllocationBudget, State};
use iroha_data_model::{
    asset::AssetDefinitionId,
    isi::{InstructionBox, Log},
    sorafs::stream_token_custody::proof::{
        StreamTokenCustodyProofRefV1, StreamTokenCustodyProofV1,
    },
    transaction::FeePaymentIntent,
};
use iroha_primitives::numeric::Quantity;
use std::{collections::BTreeMap, sync::Arc};

fn current(
    native: &NativeFixture,
    owner: &ManagedStreamTokenCustody,
    policy: &SignerCustodyPolicyV1,
    checkpoint: &FinalityVerifier,
) -> VerifiedStreamTokenCustodyStateV1 {
    let tip = native.chain.committed(native.chain.height());
    let budget = AllocationBudget::new(32 * 1024 * 1024);
    let bytes = native
        .chain
        .state()
        .with_native_stream_token_custody_snapshot_v1(
            &tip,
            owner.authority.provider_id().unwrap(),
            &budget,
            |world, owner, current| {
                norito::encode_canonical(&StreamTokenCustodyProofRefV1::new(world, owner, current))
                    .map_err(|error| error.to_string())
            },
        )
        .unwrap();
    let proof = StreamTokenCustodyProofV1::decode_frame(&bytes).unwrap();
    assert_eq!(budget.reserved_bytes(), 0);
    proof
        .verify(
            owner.authority.config.network_id,
            owner.authority.provider_id().unwrap(),
            owner
                .authority
                .provider_role(StreamTokenAuthorityRole::IssuerOperator)
                .unwrap(),
            &policy.binding,
            State::native_world_schema_hash_v1().unwrap(),
            &checkpoint.verified_tip().unwrap(),
        )
        .unwrap()
}

fn selected(
    owner: &mut ManagedStreamTokenCustody,
    policy: &SignerCustodyPolicyV1,
    original: &Original,
    options: &BoundedTransactionOptions,
) -> Result<Option<ManagedCustodyProgress>> {
    match &original.action {
        Action::Configure(_) => owner.recover_configure_selected_if_present(
            policy,
            &Fees::from_options(options)?,
            options.deadline,
        ),
        Action::Enroll { .. } => owner.recover_enroll_selected_if_present(
            policy,
            &Fees::from_options(options)?,
            options.deadline,
        ),
    }
}

#[test]
fn selected_custody_recovery_keeps_unprepared_wallets_absent_and_exact_carriers_offline() {
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "custody-native-selected",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let mut owner = ManagedStreamTokenCustody::open(
        &prepared,
        crate::managed::native_operation::test_support::provider_id(&prepared, 0),
    )
    .unwrap();
    let mut policy = super::transport_tests::policy(&owner);
    // Wide original interval avoids timing-sensitive signing in this component fixture.
    policy.active_until_unix_ms = now_ms().unwrap() + 1_200_000;
    policy.max_validity_ms = 600_000;
    policy.max_anchor_age_ms = 600_000;
    owner.validate_policy(&policy).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &owner.authority);
    let manager = owner.authority.config.clone();
    let log = quote_instructions(
        &native,
        &manager,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "custody prerequisites".into(),
        ))],
    );
    assert_eq!(native.chain.commit(vec![log]), vec![true]);
    let asset = AssetDefinitionId::parse_address_literal(
        crate::genesis::profile::TAIRA_XOR_ASSET_DEFINITION_ID,
    )
    .unwrap();
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::from([(asset, Quantity::from(1_000u64))]),
        deadline: Instant::now() + Duration::from_secs(600),
    };
    drop(ports);
    let mut retained = Vec::new();
    for (name, height) in [("configure", 3), ("enroll", 4)] {
        let checkpoint = native.observe(&owner.authority);
        let state = current(&native, &owner, &policy, &checkpoint);
        let now = now_ms().unwrap();
        let (original, utc, body) = if name == "configure" {
            assert!(state.current().is_none());
            let original = Original {
                selection: owner.selection(&policy.binding, &state).unwrap(),
                action: Action::Configure(policy.clone()),
                checkpoint: checkpoint_bytes(&checkpoint).unwrap(),
            };
            let directory = owner.authority.directory.ensure_child(name).unwrap();
            journal::publish_intent(&directory, &original).unwrap();
            (original, now + 600_000, None)
        } else {
            let selected_state = state.current().unwrap();
            assert_eq!(selected_state.control().policy, policy);
            assert_eq!(selected_state.record().execution_height, 3);
            assert!(selected_state.control().active_head.is_none());
            let interval = ManagedCustodyEnrollmentInterval {
                issued_at_unix_ms: now,
                expires_at_unix_ms: now + 600_000,
                deadline_unix_ms: now + 600_000,
            };
            let unsigned = owner
                .unsigned_enrollment(&policy, &state, &checkpoint, interval, now)
                .unwrap();
            let history = owner.bootstrap_native_body(
                &native,
                CustodyPurpose::InitialEnroll,
                unsigned,
                interval.deadline_unix_ms,
                &options,
            );
            let original = history.dispatch().unwrap().1.clone();
            (original, interval.deadline_unix_ms, Some(history))
        };
        owner
            .validate_original(&original, CustodyPurpose::initial(&original.action))
            .unwrap();
        let directory = match &body {
            Some(history) => {
                PrivateDirectory::open_exact(history.dispatch().unwrap().0.path()).unwrap()
            }
            None => owner.authority.directory.open_child(name).unwrap(),
        };
        let outer = owner.authority.directory.open_child(name).unwrap();
        let outer_bytes = outer
            .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
            .unwrap();
        let mut peers = UnavailablePeers::start(&prepared);
        if name == "configure" {
            assert!(matches!(
                selected(&mut owner, &policy, &original, &options),
                Err(crate::managed::Error::Bootstrap(
                    ManagedBootstrapFailure::TransitionPending
                ))
            ));
        } else {
            // The anchored body is retained local intent; no wallet dispatch exists yet.
            let progress = selected(&mut owner, &policy, &original, &options)
                .unwrap()
                .unwrap();
            assert_eq!(progress.transaction_status, OperationStatus::Absent);
            assert!(progress.finalized.is_none() && progress.current.is_none());
        }
        assert!(!directory.path().join("attempts").exists());
        assert!(peers.requests.lock().unwrap().is_empty());
        peers.finish();
        let account = AccountService::new(manager.clone()).unwrap();
        let fixed_scope = HistoryScope::FixedBody;
        let scope = body
            .as_ref()
            .map_or(&fixed_scope, |history| history.dispatch().unwrap().2);
        journal::explicit(&directory, &original, utc, &options, &account, scope).unwrap();
        let original = match &body {
            Some(_) => owner
                .required_enrollment(CustodyPurpose::InitialEnroll)
                .unwrap(),
            None => journal::required_original(&directory).unwrap(),
        };
        let original_bytes = std::fs::read(directory.path().join("original.nrt")).unwrap();
        let path = original.directory().path().join("transaction");
        let mut peers = UnavailablePeers::start(&prepared);
        let progress = selected(&mut owner, &policy, &original, &options)
            .unwrap()
            .unwrap();
        assert_eq!(progress.transaction_status, OperationStatus::Absent);
        assert!(progress.finalized.is_none() && progress.current.is_none());
        let mut wrong_policy = policy.clone();
        wrong_policy.max_anchor_age_ms -= 1;
        assert!(selected(&mut owner, &wrong_policy, &original, &options).is_err());
        assert!(
            original
                .terms
                .matches(original.terms.requested_deadline_unix_ms + 1, &options)
                .is_err()
        );
        let mut wrong_options = original.terms.options(options.deadline);
        wrong_options.max_total_fees.clear();
        assert!(selected(&mut owner, &policy, &original, &wrong_options).is_err());
        assert!(
            !path.join("payload.json").exists() && !path.join("operation.json").exists(),
            "read-only recovery must not prepare or sign a wallet"
        );
        assert_eq!(
            std::fs::read(directory.path().join("original.nrt")).unwrap(),
            original_bytes
        );
        assert!(peers.requests.lock().unwrap().is_empty());
        peers.finish();

        let request = original
            .request(original.terms.signing_deadline(options.deadline).unwrap())
            .unwrap();
        let mut peers = UnavailablePeers::start(&prepared);
        let preparation = request.inspect(&account, &path).unwrap();
        assert_eq!(
            preparation.phase(),
            iroha_wallet::operations::NativePreparationPhase::RequestOnly
        );
        let wallet = PrivateDirectory::open_exact(&path).unwrap();
        let request_bytes = wallet.read("preparation.json", 4 * 1024 * 1024).unwrap();
        let inventory = wallet.entries(8).unwrap();
        assert!(
            match &request {
                journal::Request::Configure(request) =>
                    account.prepare_stream_token_custody_configure(request, &path),
                journal::Request::Enroll(request) =>
                    account.prepare_stream_token_custody_enroll(request, &path),
            }
            .is_err(),
            "actual fee preflight must fail"
        );
        assert!(!path.join("payload.json").exists() && !path.join("operation.json").exists());
        assert!(!path.join("submission.json").exists());
        peers.requests.lock().unwrap().clear();
        for _ in 0..2 {
            let progress = selected(&mut owner, &policy, &original, &options)
                .unwrap()
                .unwrap();
            assert_eq!(progress.transaction_status, OperationStatus::Absent);
            assert!(progress.finalized.is_none() && progress.current.is_none());
            assert_eq!(
                request.inspect(&account, &path).unwrap().phase(),
                iroha_wallet::operations::NativePreparationPhase::RequestOnly
            );
            assert_eq!(wallet.entries(8).unwrap(), inventory);
            assert_eq!(
                wallet.read("preparation.json", 4 * 1024 * 1024).unwrap(),
                request_bytes
            );
            assert!(peers.requests.lock().unwrap().is_empty());
        }
        peers.finish();
        let mut http = NativeReadHttp::start_config(&manager, Arc::clone(native.chain.state()));
        match original
            .request(original.terms.signing_deadline(options.deadline).unwrap())
            .unwrap()
        {
            journal::Request::Configure(request) => {
                account.prepare_stream_token_custody_configure(&request, &path)
            }
            journal::Request::Enroll(request) => {
                account.prepare_stream_token_custody_enroll(&request, &path)
            }
        }
        .unwrap();
        let signed = owner
            .verify_wallet(original.directory(), &original, options.deadline)
            .unwrap();
        http.finish();
        crate::managed::native_operation::test_support::preparation::payload_retained(
            &prepared,
            &path,
            || {
                match &request {
                    journal::Request::Configure(request) => {
                        account.inspect_stream_token_custody_configure_preparation(&path, request)
                    }
                    journal::Request::Enroll(request) => {
                        account.inspect_stream_token_custody_enroll_preparation(&path, request)
                    }
                }
                .unwrap()
            },
            |advance| {
                owner
                    .advance(
                        CustodyPurpose::initial(&original.action),
                        options.deadline,
                        if advance {
                            Mode::SubmitOriginal
                        } else {
                            Mode::ObserveOnly
                        },
                        false,
                    )
                    .map(|value| {
                        assert!(value.finalized.is_none() && value.current.is_none());
                        value.transaction_status
                    })
            },
            || {
                match &request {
                    journal::Request::Configure(request) => {
                        account.prepare_stream_token_custody_configure(request, &path)
                    }
                    journal::Request::Enroll(request) => {
                        account.prepare_stream_token_custody_enroll(request, &path)
                    }
                }
                .unwrap();
            },
        );
        assert_eq!(
            http.requests
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, path)| path == "/v1/fees/quote")
                .count(),
            1
        );
        let wire = signed.encode_wire_v1().unwrap();
        let operation = std::fs::read(path.join("operation.json")).unwrap();
        assert!(!path.join("submission.json").exists());
        assert_eq!(native.chain.commit(vec![signed.clone()]), vec![true]);
        let carrier = native.observe(&owner.authority);
        assert_eq!(carrier.checkpoint().height(), height);
        let finalized = verify_carrier(&carrier, &signed).unwrap();
        assert_eq!(finalized.height, height);
        original
            .directory()
            .write_atomic(
                "carrier.nrt",
                &checkpoint_bytes(&carrier).unwrap(),
                PublishMode::CreateNew,
            )
            .unwrap();
        retained.push((
            name,
            directory.path().join("original.nrt"),
            outer_bytes,
            original,
            original_bytes,
            wire,
            operation,
            finalized,
        ));
    }
    drop(owner);
    let mut peers = UnavailablePeers::start(&prepared);
    for _ in 0..2 {
        let mut owner = ManagedStreamTokenCustody::open(
            &prepared,
            crate::managed::native_operation::test_support::provider_id(&prepared, 0),
        )
        .unwrap();
        for (name, body_path, outer_bytes, original, original_bytes, wire, operation, finalized) in
            &retained
        {
            let options = original
                .terms
                .options(Instant::now() + Duration::from_secs(30));
            let report = selected(&mut owner, &policy, original, &options)
                .unwrap()
                .unwrap();
            assert_eq!(report.transaction_status, OperationStatus::Applied);
            assert_eq!(report.finalized, Some(*finalized));
            assert!(report.current.is_none());
            let directory = owner.authority.directory.open_child(name).unwrap();
            assert_eq!(
                &directory
                    .read("original.nrt", journal::MAX_ORIGINAL_BYTES)
                    .unwrap(),
                outer_bytes
            );
            assert_eq!(&std::fs::read(body_path).unwrap(), original_bytes);
            let path = original.directory().path().join("transaction");
            assert_eq!(
                &std::fs::read(path.join("operation.json")).unwrap(),
                operation
            );
            assert!(!path.join("submission.json").exists());
            assert_eq!(
                &owner
                    .verify_wallet(original.directory(), original, options.deadline)
                    .unwrap()
                    .encode_wire_v1()
                    .unwrap(),
                wire
            );
        }
        assert!(peers.requests.lock().unwrap().is_empty());
    }
    peers.finish();
}
