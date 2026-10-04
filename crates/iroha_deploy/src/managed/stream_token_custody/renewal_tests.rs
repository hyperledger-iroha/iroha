//! Generated renewal through real original custody wallets and native executed carriers.
//! These component controls do not claim full worker renewal or running service qualification.

use super::*;
use crate::managed::native_operation::{
    test_support::{
        UnavailablePeers,
        native_fixture::{NativeFixture, NativeReadHttp, quote_instructions},
    },
    verify_carrier,
};
use iroha_data_model::{
    asset::AssetDefinitionId,
    executor::ValidationFail,
    isi::{
        InstructionBox, Log,
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsStreamTokenCustody,
    },
    sorafs::stream_token_custody::{
        SorafsStreamTokenCustodyActionV1, SorafsStreamTokenCustodyRevocationV1,
    },
    transaction::{FeePaymentIntent, error::TransactionRejectionReason},
};
use iroha_primitives::numeric::Quantity;
use std::{collections::BTreeMap, sync::Arc};

struct Fixture {
    _temporary: tempfile::TempDir,
    prepared: PreparedLocalnet,
    owner: ManagedStreamTokenCustody,
    native: NativeFixture,
    policy: SignerCustodyPolicyV1,
    options: BoundedTransactionOptions,
    initial: ManagedCustodyEnrollmentInterval,
}
impl Fixture {
    fn enrolled(validity_ms: u64) -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let ports = crate::managed::LocalnetPorts::reserve().unwrap();
        let prepared = crate::localnet::prepare_localnet_at(
            "custody-renewal",
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
        policy.active_until_unix_ms = now_ms().unwrap() + 600_000;
        policy.max_validity_ms = 120_000;
        policy.max_anchor_age_ms = 120_000;
        let mut native = NativeFixture::from_generated(&prepared, &owner.authority);
        let log = quote_instructions(
            &native,
            &owner.authority.config,
            [InstructionBox::from(Log::new(
                iroha_data_model::Level::INFO,
                "custody renewal prerequisites".into(),
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
            deadline: Instant::now() + Duration::from_secs(300),
        };
        drop(ports);
        assert_eq!(
            owner
                .bootstrap_native_configure(
                    &mut native,
                    &policy,
                    now_ms().unwrap() + 240_000,
                    &options
                )
                .finalized
                .unwrap()
                .height,
            3
        );
        let now = now_ms().unwrap();
        let initial = ManagedCustodyEnrollmentInterval {
            issued_at_unix_ms: now,
            expires_at_unix_ms: now + validity_ms,
            deadline_unix_ms: now + validity_ms,
        };
        assert_eq!(
            owner
                .bootstrap_native_enroll(&mut native, &policy, initial, &options)
                .finalized
                .unwrap()
                .height,
            4
        );
        Self {
            _temporary: temporary,
            prepared,
            owner,
            native,
            policy,
            options,
            initial,
        }
    }
    fn current(&self) -> (FinalityVerifier, VerifiedStreamTokenCustodyStateV1) {
        let checkpoint = self.native.observe(&self.owner.authority);
        let current =
            self.native
                .bootstrap_custody(&self.owner.authority, &self.policy, &checkpoint);
        (checkpoint, current)
    }
    fn select(&self, sequence: u64) -> Original {
        let (checkpoint, current) = self.current();
        self.owner
            .select_renewal_original(
                sequence,
                &self.policy,
                &current,
                &checkpoint,
                Terms::new(now_ms().unwrap() + 60_000, &self.options).unwrap(),
                self.options.deadline,
            )
            .unwrap()
    }
    fn prepare(
        &self,
        original: &Original,
    ) -> (PrivateDirectory, Selected<Original>, SignedTransaction) {
        let directory = self
            .owner
            .authority
            .directory
            .ensure_child(&renewal::directory_name(2).unwrap())
            .unwrap();
        journal::publish_intent(&directory, original).unwrap();
        let account = AccountService::new(self.owner.authority.config.clone()).unwrap();
        journal::explicit(
            &directory,
            original,
            now_ms().unwrap() + 60_000,
            &self.options,
            &account,
        )
        .unwrap();
        let original = journal::required_original(&directory).unwrap();
        let mut http = NativeReadHttp::start_config(
            &self.owner.authority.config,
            Arc::clone(self.native.chain.state()),
        );
        let journal::Request::Enroll(request) = original
            .request(
                original
                    .terms
                    .signing_deadline(self.options.deadline)
                    .unwrap(),
            )
            .unwrap()
        else {
            panic!("renewal must be Enroll")
        };
        account
            .prepare_stream_token_custody_enroll(
                &request,
                &original.directory().path().join("transaction"),
            )
            .unwrap();
        let signed = self
            .owner
            .verify_wallet(original.directory(), &original, self.options.deadline)
            .unwrap();
        http.finish();
        assert!(
            !original
                .directory()
                .path()
                .join("transaction/submission.json")
                .exists()
        );
        (directory, original, signed)
    }
}

fn wait_until(utc: u64, limit: Duration) {
    let deadline = Instant::now() + limit;
    while now_ms().unwrap() < utc {
        assert!(
            Instant::now() < deadline,
            "original finite clock did not advance in bounded wait"
        );
        std::thread::sleep(Duration::from_millis(
            utc.saturating_sub(now_ms().unwrap()).min(25),
        ));
    }
}

#[test]
fn generated_renewal_changes_native_head_and_preserves_initial_and_renewed_history_offline() {
    let _guard = crate::managed::native_test_guard();
    let mut fixture = Fixture::enrolled(20_000);
    let initial = fixture
        .owner
        .retained_initial_enrollment(&fixture.policy, fixture.initial, fixture.options.deadline)
        .unwrap();
    assert_eq!(initial.statement().sequence, 1);
    assert_eq!(initial.finalized().height, 4);
    let initial_bytes = initial.bytes().to_vec();
    // Arithmetic-only selections do not produce proof or modify the genuine native head.
    let old = initial.statement();
    let midpoint = old.issued_at_unix_ms + 10_000;
    assert!(
        renewal::renewal_interval(
            old,
            &fixture.policy,
            midpoint - 1,
            fixture.policy.active_until_unix_ms,
            midpoint + 1_000
        )
        .is_err()
    );
    let capped_end = old.expires_at_unix_ms + 1;
    let capped =
        renewal::renewal_interval(old, &fixture.policy, midpoint, capped_end, midpoint + 1_000)
            .unwrap();
    assert_eq!(capped.expires_at_unix_ms, capped_end);
    assert!(
        renewal::renewal_interval(
            old,
            &fixture.policy,
            midpoint,
            old.expires_at_unix_ms,
            midpoint + 1_000
        )
        .is_err()
    );
    assert!(
        renewal::renewal_interval(old, &fixture.policy, midpoint, capped_end, capped_end + 1)
            .is_err()
    );

    let (old_checkpoint, old_current) = fixture.current();
    let selected_initial = fixture
        .owner
        .select_retained_current_enrollment(
            &fixture.policy,
            fixture.initial,
            initial.finalized().height,
            *initial.finalized().block_hash.as_ref(),
            &old_current,
            now_ms().unwrap(),
            fixture.options.deadline,
        )
        .unwrap();
    assert_eq!(selected_initial.bytes(), initial.bytes());
    assert_eq!(selected_initial.finalized(), initial.finalized());

    assert!(
        fixture
            .owner
            .select_renewal_original(
                2,
                &fixture.policy,
                &old_current,
                &old_checkpoint,
                Terms::new(now_ms().unwrap() + 60_000, &fixture.options).unwrap(),
                fixture.options.deadline
            )
            .is_err()
    );
    assert!(
        !fixture
            .owner
            .authority
            .directory
            .path()
            .join(renewal::directory_name(2).unwrap())
            .exists()
    );
    wait_until(
        fixture.initial.issued_at_unix_ms + 10_000,
        Duration::from_secs(12),
    );
    let original = fixture.select(2);
    let (directory, original, signed) = fixture.prepare(&original);
    let wire = signed.encode_wire_v1().unwrap();
    let operation = std::fs::read(
        original
            .directory()
            .path()
            .join("transaction/operation.json"),
    )
    .unwrap();
    let original_bytes = directory
        .read("original.nrt", MAX_CHECKPOINT_BYTES + 128 * 1024)
        .unwrap();
    assert_eq!(
        fixture.native.chain.commit(vec![signed.clone()]),
        vec![true]
    );
    let (carrier, current) = fixture.current();
    let finalized = verify_carrier(&carrier, &signed).unwrap();
    assert_eq!(finalized.height, 5);
    let state = current.current().unwrap();
    assert_eq!(state.control().active_head.unwrap().sequence, 2);
    assert_eq!(state.control().next_sequence, 3);
    assert!(state.record().active_enrollment.as_ref().unwrap() != &initial_bytes);
    let Action::Enroll {
        validity,
        enrollment,
        ..
    } = &original.action
    else {
        unreachable!()
    };
    assert!(validity.expires_at_unix_ms > fixture.initial.expires_at_unix_ms);
    assert_eq!(
        state.record().active_enrollment.as_ref().unwrap(),
        enrollment
    );
    original
        .directory()
        .write_atomic(
            "carrier.nrt",
            &checkpoint_bytes(&carrier).unwrap(),
            PublishMode::CreateNew,
        )
        .unwrap();
    // A valid current proof and an unrelated original cut cannot be joined by shape alone.
    assert!(
        fixture
            .owner
            .select_renewal_original(
                3,
                &fixture.policy,
                &old_current,
                &carrier,
                Terms::new(now_ms().unwrap() + 60_000, &fixture.options).unwrap(),
                fixture.options.deadline
            )
            .is_err()
    );
    assert!(
        fixture
            .owner
            .select_renewal_original(
                2,
                &fixture.policy,
                &current,
                &carrier,
                Terms::new(now_ms().unwrap() + 60_000, &fixture.options).unwrap(),
                fixture.options.deadline
            )
            .is_err()
    );
    let renewed_for_runtime = fixture
        .owner
        .retained_renewed_enrollment(2, &fixture.policy, fixture.options.deadline)
        .unwrap();
    fixture
        .owner
        .verify_enrollment_at(
            &renewed_for_runtime,
            &fixture.policy,
            finalized.height,
            *finalized.block_hash.as_ref(),
            &current,
            now_ms().unwrap(),
            fixture.options.deadline,
        )
        .unwrap();
    // Reconstruction selects the exact native sequence, never the highest directory name.
    fixture
        .owner
        .authority
        .directory
        .ensure_child(&renewal::directory_name(64).unwrap())
        .unwrap();
    let selected = fixture
        .owner
        .select_retained_current_enrollment(
            &fixture.policy,
            fixture.initial,
            initial.finalized().height,
            *initial.finalized().block_hash.as_ref(),
            &current,
            now_ms().unwrap(),
            fixture.options.deadline,
        )
        .unwrap();
    assert_eq!(selected.statement().sequence, 2);
    assert_eq!(selected.bytes(), renewed_for_runtime.bytes());
    assert_eq!(selected.finalized(), renewed_for_runtime.finalized());
    assert_eq!(
        selected.finalized().transaction_hash,
        finalized.transaction_hash
    );
    // Missing exact selected history cannot fall back to initial or another numbered journal.
    let retained_original = directory.path().join("original.nrt");
    let moved_original = directory.path().join("original-held.nrt");
    std::fs::rename(&retained_original, &moved_original).unwrap();
    assert!(
        fixture
            .owner
            .select_retained_current_enrollment(
                &fixture.policy,
                fixture.initial,
                finalized.height,
                *finalized.block_hash.as_ref(),
                &current,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    std::fs::rename(&moved_original, &retained_original).unwrap();
    assert_eq!(
        directory
            .read("original.nrt", MAX_CHECKPOINT_BYTES + 128 * 1024)
            .unwrap(),
        original_bytes
    );
    assert!(
        fixture
            .owner
            .select_retained_current_enrollment(
                &fixture.policy,
                fixture.initial,
                finalized.height,
                *finalized.block_hash.as_ref(),
                &old_current,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    for (height, hash) in [(0, *finalized.block_hash.as_ref()), (2, [0; 32])] {
        assert!(
            fixture
                .owner
                .select_retained_current_enrollment(
                    &fixture.policy,
                    fixture.initial,
                    height,
                    hash,
                    &current,
                    now_ms().unwrap(),
                    fixture.options.deadline,
                )
                .is_err()
        );
    }
    // A genuine predecessor proof cannot select the newly committed runtime record.
    assert!(
        fixture
            .owner
            .verify_enrollment_at(
                &renewed_for_runtime,
                &fixture.policy,
                finalized.height,
                *finalized.block_hash.as_ref(),
                &old_current,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    // Conversely, the new native head cannot reactivate the old retained enrollment.
    assert!(
        fixture
            .owner
            .verify_enrollment_at(
                &initial,
                &fixture.policy,
                finalized.height,
                *finalized.block_hash.as_ref(),
                &current,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    for (height, hash) in [
        (finalized.height + 1, *finalized.block_hash.as_ref()),
        (finalized.height, [99; 32]),
    ] {
        assert!(
            fixture
                .owner
                .verify_enrollment_at(
                    &renewed_for_runtime,
                    &fixture.policy,
                    height,
                    hash,
                    &current,
                    now_ms().unwrap(),
                    fixture.options.deadline,
                )
                .is_err()
        );
    }
    let mut changed_policy = fixture.policy.clone();
    changed_policy.max_anchor_age_ms -= 1;
    assert!(
        fixture
            .owner
            .retained_renewed_enrollment(2, &changed_policy, fixture.options.deadline)
            .is_err()
    );
    let mut changed_interval = fixture.initial;
    changed_interval.expires_at_unix_ms += 1;
    assert!(
        fixture
            .owner
            .retained_initial_enrollment(
                &fixture.policy,
                changed_interval,
                fixture.options.deadline
            )
            .is_err()
    );
    // Real current revocation rejects an otherwise unexpired renewed record. Historical
    // recovery below must still return its exact successful H5 carrier without reading H6.
    assert!(now_ms().unwrap() < renewed_for_runtime.statement().expires_at_unix_ms);
    let revoke = quote_instructions(
        &fixture.native,
        &fixture.owner.authority.config,
        [InstructionBox::from(MutateSorafsStreamTokenCustody {
            provider_id: fixture.owner.authority.provider_id().unwrap(),
            expected_revision: state.record().revision,
            expected_digest: state.anchor().state_digest,
            action: SorafsStreamTokenCustodyActionV1::Revoke(
                SorafsStreamTokenCustodyRevocationV1 {
                    signer: true,
                    attester: false,
                },
            ),
        })],
    );
    assert_eq!(fixture.native.chain.commit(vec![revoke]), vec![true]);
    let (_, revoked_for_runtime) = fixture.current();
    assert!(
        revoked_for_runtime
            .current()
            .unwrap()
            .control()
            .signer_revoked
    );
    assert!(
        fixture
            .owner
            .verify_enrollment_at(
                &renewed_for_runtime,
                &fixture.policy,
                finalized.height,
                *finalized.block_hash.as_ref(),
                &revoked_for_runtime,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    assert!(
        fixture
            .owner
            .select_retained_current_enrollment(
                &fixture.policy,
                fixture.initial,
                finalized.height,
                *finalized.block_hash.as_ref(),
                &revoked_for_runtime,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    let mut changed_options = fixture.options.clone();
    changed_options.max_total_fees.clear();
    let mut peers = UnavailablePeers::start(&fixture.prepared);
    assert!(
        fixture
            .owner
            .renew(
                2,
                original.terms.requested_deadline_unix_ms,
                &changed_options
            )
            .is_err()
    );
    assert!(
        fixture
            .owner
            .renew(
                2,
                original.terms.requested_deadline_unix_ms + 1,
                &fixture.options
            )
            .is_err()
    );
    drop(fixture.owner);
    for _ in 0..2 {
        let mut owner = ManagedStreamTokenCustody::open(
            &fixture.prepared,
            crate::managed::native_operation::test_support::provider_id(&fixture.prepared, 0),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let progress = owner.recover_renewal(2, deadline).unwrap().unwrap();
        assert_eq!(progress.transaction_status, OperationStatus::Applied);
        assert_eq!(progress.finalized, Some(finalized));
        assert!(progress.current.is_none());
        let renewed = owner
            .retained_renewed_enrollment(2, &fixture.policy, deadline)
            .unwrap();
        assert_eq!(renewed.bytes(), enrollment);
        assert_eq!(renewed.statement().sequence, 2);
        assert_eq!(
            renewed.record_digest(),
            state.control().active_head.unwrap().record_digest
        );
        assert_eq!(*renewed.finalized(), finalized);
        assert_eq!(
            owner
                .retained_initial_enrollment(&fixture.policy, fixture.initial, deadline)
                .unwrap()
                .bytes(),
            initial_bytes
        );
        assert_eq!(
            directory
                .read("original.nrt", MAX_CHECKPOINT_BYTES + 128 * 1024)
                .unwrap(),
            original_bytes
        );
        assert_eq!(
            std::fs::read(
                original
                    .directory()
                    .path()
                    .join("transaction/operation.json")
            )
            .unwrap(),
            operation
        );
        assert_eq!(
            owner
                .verify_wallet(original.directory(), &original, deadline)
                .unwrap()
                .encode_wire_v1()
                .unwrap(),
            wire
        );
        assert!(
            !original
                .directory()
                .path()
                .join("transaction/submission.json")
                .exists()
        );
    }
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}

#[test]
fn expired_unrevoked_head_can_renew_but_real_revocation_invalidates_the_original_cas() {
    let _guard = crate::managed::native_test_guard();
    let mut fixture = Fixture::enrolled(8_000);
    wait_until(fixture.initial.expires_at_unix_ms, Duration::from_secs(10));
    let retained = fixture
        .owner
        .retained_initial_enrollment(&fixture.policy, fixture.initial, fixture.options.deadline)
        .unwrap();
    let (_, expired_current) = fixture.current();
    assert!(
        fixture
            .owner
            .select_retained_current_enrollment(
                &fixture.policy,
                fixture.initial,
                retained.finalized().height,
                *retained.finalized().block_hash.as_ref(),
                &expired_current,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );

    assert!(
        fixture
            .owner
            .verify_enrollment_at(
                &retained,
                &fixture.policy,
                retained.finalized().height,
                *retained.finalized().block_hash.as_ref(),
                &expired_current,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    ); // Genuine wall-clock expiry is not revived by fresh observation.
    let original = fixture.select(2); // Genuine elapsed old interval, never a clock rewrite.
    let (directory, original, signed) = fixture.prepare(&original);
    let (_, before) = fixture.current();
    let state = before.current().unwrap();
    let revoke = quote_instructions(
        &fixture.native,
        &fixture.owner.authority.config,
        [InstructionBox::from(MutateSorafsStreamTokenCustody {
            provider_id: fixture.owner.authority.provider_id().unwrap(),
            expected_revision: state.record().revision,
            expected_digest: state.anchor().state_digest,
            action: SorafsStreamTokenCustodyActionV1::Revoke(
                SorafsStreamTokenCustodyRevocationV1 {
                    signer: false,
                    attester: true,
                },
            ),
        })],
    );
    assert_eq!(fixture.native.chain.commit(vec![revoke]), vec![true]);
    let (revoked_checkpoint, revoked) = fixture.current();
    assert!(revoked.current().unwrap().control().attester_revoked);
    assert!(
        fixture
            .owner
            .verify_enrollment_at(
                &retained,
                &fixture.policy,
                retained.finalized().height,
                *retained.finalized().block_hash.as_ref(),
                &revoked,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );

    assert!(
        fixture
            .owner
            .select_renewal_original(
                2,
                &fixture.policy,
                &revoked,
                &revoked_checkpoint,
                Terms::new(now_ms().unwrap() + 60_000, &fixture.options).unwrap(),
                fixture.options.deadline
            )
            .is_err()
    );
    assert_eq!(
        fixture.native.chain.commit(vec![signed.clone()]),
        vec![false]
    );
    let (rejected_carrier, after) = fixture.current();
    let rejected = rejected_carrier.verified_tip().unwrap();
    assert_eq!(rejected.block().network_entrypoint_count(), 1);
    assert_eq!(
        rejected
            .block()
            .external_transactions()
            .next()
            .unwrap()
            .encode_wire_v1()
            .unwrap(),
        signed.encode_wire_v1().unwrap()
    );
    assert_eq!(
        rejected
            .block()
            .network_output_at(0)
            .unwrap()
            .1
            .result
            .as_ref()
            .unwrap_err(),
        &TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
            InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
                "StreamToken custody control rejected: Conflict".into()
            ))
        ))
    );
    assert!(verify_carrier(&rejected_carrier, &signed).is_err());
    assert_eq!(
        after.current().unwrap().record(),
        revoked.current().unwrap().record()
    );
    assert!(
        fixture
            .owner
            .retained_renewed_enrollment(2, &fixture.policy, fixture.options.deadline)
            .is_err()
    );
    assert!(!original.directory().path().join("carrier.nrt").exists());
    // A generated profile from another genuine signed genesis cannot consume this native proof.
    let other_ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let other = crate::localnet::prepare_localnet_at(
        "other-custody-renewal",
        &fixture._temporary.path().join("other"),
        &other_ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let other_owner = ManagedStreamTokenCustody::open(
        &other,
        crate::managed::native_operation::test_support::provider_id(&other, 0),
    )
    .unwrap();
    assert_ne!(
        other_owner.authority.config.network_id,
        fixture.owner.authority.config.network_id
    );
    assert!(
        other_owner
            .verify_enrollment_at(
                &retained,
                &fixture.policy,
                retained.finalized().height,
                *retained.finalized().block_hash.as_ref(),
                &revoked,
                now_ms().unwrap(),
                fixture.options.deadline,
            )
            .is_err()
    );
    assert!(
        other_owner
            .select_renewal_original(
                2,
                &fixture.policy,
                &revoked,
                &revoked_checkpoint,
                Terms::new(now_ms().unwrap() + 60_000, &fixture.options).unwrap(),
                fixture.options.deadline
            )
            .is_err()
    );
}

#[test]
fn missing_renewal_and_out_of_range_sequences_have_no_http_or_journal_effect() {
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "custody-renewal-empty",
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
    drop(ports);
    let mut peers = UnavailablePeers::start(&prepared);
    let deadline = Instant::now() + Duration::from_secs(30);
    assert!(owner.recover_renewal(2, deadline).unwrap().is_none());
    assert!(
        !owner
            .authority
            .directory
            .path()
            .join(renewal::directory_name(2).unwrap())
            .exists()
    );
    for sequence in [0, 1, 65, u64::MAX] {
        assert!(owner.recover_renewal(sequence, deadline).is_err());
        assert!(owner.advance_renewal(sequence, deadline).is_err());
    }
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}

#[test]
fn unprepared_renewal_expires_without_replacing_original_or_creating_wallet_on_recovery() {
    let _guard = crate::managed::native_test_guard();
    let mut fixture = Fixture::enrolled(8_000);
    wait_until(
        fixture.initial.issued_at_unix_ms + 4_000,
        Duration::from_secs(6),
    );
    let (checkpoint, current) = fixture.current();
    let utc = now_ms().unwrap() + 2_000;
    let original = fixture
        .owner
        .select_renewal_original(
            2,
            &fixture.policy,
            &current,
            &checkpoint,
            Terms::new(utc, &fixture.options).unwrap(),
            fixture.options.deadline,
        )
        .unwrap();
    let name = renewal::directory_name(2).unwrap();
    let directory = fixture
        .owner
        .authority
        .directory
        .ensure_child(&name)
        .unwrap();
    journal::publish_intent(&directory, &original).unwrap();
    let account = AccountService::new(fixture.owner.authority.config.clone()).unwrap();
    journal::explicit(&directory, &original, utc, &fixture.options, &account).unwrap();
    let original = journal::required_original(&directory).unwrap();
    let wallet = original.directory().open_child("transaction").unwrap();
    let request_bytes = wallet.read("preparation.json", 4 * 1024 * 1024).unwrap();
    let wallet_inventory = wallet.entries(8).unwrap();
    let bytes = directory
        .read("original.nrt", MAX_CHECKPOINT_BYTES + 128 * 1024)
        .unwrap();
    let mut peers = UnavailablePeers::start(&fixture.prepared);
    let report = fixture
        .owner
        .recover_renewal(2, Instant::now() + Duration::from_secs(10))
        .unwrap()
        .unwrap();
    assert_eq!(report.transaction_status, OperationStatus::Absent);
    assert!(report.finalized.is_none() && report.current.is_none());
    wait_until(utc, Duration::from_secs(3));
    drop(fixture.owner);
    let mut owner = ManagedStreamTokenCustody::open(
        &fixture.prepared,
        crate::managed::native_operation::test_support::provider_id(&fixture.prepared, 0),
    )
    .unwrap();
    let report = owner
        .recover_renewal(2, Instant::now() + Duration::from_secs(30))
        .unwrap()
        .unwrap();
    assert_eq!(report.transaction_status, OperationStatus::Expired);
    assert!(report.finalized.is_none() && report.current.is_none());
    assert_eq!(
        directory
            .read("original.nrt", MAX_CHECKPOINT_BYTES + 128 * 1024)
            .unwrap(),
        bytes
    );
    assert!(
        !wallet.path().join("payload.json").exists()
            && !wallet.path().join("operation.json").exists()
    );
    assert_eq!(wallet.entries(8).unwrap(), wallet_inventory);
    assert_eq!(
        wallet.read("preparation.json", 4 * 1024 * 1024).unwrap(),
        request_bytes
    );
    // Expired paid authorization is still inspectable locally; this accessor grants no advance.
    assert_eq!(
        owner
            .inspect_local_initial_interval(&fixture.policy)
            .unwrap(),
        fixture.initial
    );
    let restored = journal::required_original(&directory).unwrap();
    assert_eq!(restored.terms.requested_deadline_unix_ms, utc);
    assert!(
        restored
            .terms
            .signing_deadline(Instant::now() + Duration::from_secs(300))
            .is_err()
    );
    assert!(peers.requests.lock().unwrap().is_empty());
    peers.finish();
}
