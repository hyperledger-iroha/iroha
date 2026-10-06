//! Genuine generated parent composition, exact native children and offline original recovery.
//! The component commits wallet envelopes directly; it does not qualify running peers, the
//! complete HTTP bootstrap, native per-use eligibility, catalog admission or Serving readiness.

use super::*;
use crate::{
    managed::{
        native_operation::{
            MAX_CHECKPOINT_BYTES, now_ms,
            test_support::{
                UnavailablePeers,
                native_fixture::{NativeFixture, quote_instructions},
            },
            verify_carrier,
        },
        service_authority::ServiceAuthority,
    },
    verify::finality::FinalityVerifier,
};
use iroha_core::state::{AllocationBudget, State, StateReadOnly as _};
use iroha_data_model::{
    isi::{InstructionBox, Log},
    sorafs::{
        reserve::{
            ReserveAuthorityPolicyV1,
            account_proof::{ReserveAccountProofExpectedV1, VerifiedReserveAccountStateV1},
        },
        stream_token_custody::proof::{
            StreamTokenCustodyProofRefV1, StreamTokenCustodyProofV1,
            VerifiedStreamTokenCustodyStateV1,
        },
    },
    transaction::{FeePaymentIntent, SignedTransaction},
};
use iroha_primitives::numeric::Quantity;
use sorafs_manifest::signer::custody_control::SignerCustodyPolicyV1;
use std::{num::NonZeroU64, time::Duration};

impl NativeFixture {
    pub(in crate::managed) fn bootstrap_commit(
        &mut self,
        authority: &ServiceAuthority,
        signed: &SignedTransaction,
    ) -> FinalityVerifier {
        let height = self.chain.height() + 1;
        assert_eq!(
            self.chain.commit(vec![signed.clone()]),
            vec![true],
            "{}",
            self.bootstrap_commit_refusal_diagnostic(height),
        );
        let verifier = self.observe(authority);
        let tip = verifier.verified_tip().unwrap();
        assert_eq!(tip.height(), height);
        assert_eq!(tip.block().network_entrypoint_count(), 1);
        assert_eq!(
            tip.block()
                .external_transactions()
                .next()
                .unwrap()
                .encode_wire_v1()
                .unwrap(),
            signed.encode_wire_v1().unwrap()
        );
        assert_eq!(verify_carrier(&verifier, signed).unwrap().height, height);
        verifier
    }

    // Only the failed assertion evaluates this diagnostic. Keep the committed block and its
    // canonical output out of the successful commit/observe stack frame, with no extra clock.
    #[cold]
    #[inline(never)]
    fn bootstrap_commit_refusal_diagnostic(&self, height: u64) -> String {
        let committed = self.chain.committed(height);
        format!(
            "native bootstrap commit refused: height={height}; block_time_ms={}; network_output_at(0).result={:?}",
            committed.block().header().creation_time().as_millis(),
            committed
                .block()
                .network_output_at(0)
                .map(|(_, output)| &output.result),
        )
    }

    pub(in crate::managed) fn bootstrap_custody(
        &self,
        authority: &ServiceAuthority,
        policy: &SignerCustodyPolicyV1,
        checkpoint: &FinalityVerifier,
    ) -> VerifiedStreamTokenCustodyStateV1 {
        let tip = self.chain.committed(self.chain.height());
        let budget = AllocationBudget::new(32 * 1024 * 1024);
        let bytes = self
            .chain
            .state()
            .with_native_stream_token_custody_snapshot_v1(
                &tip,
                authority.provider_id().unwrap(),
                &budget,
                |world, owner, current| {
                    norito::encode_canonical(&StreamTokenCustodyProofRefV1::new(
                        world, owner, current,
                    ))
                    .map_err(|error| error.to_string())
                },
            )
            .unwrap();
        assert_eq!(budget.reserved_bytes(), 0);
        StreamTokenCustodyProofV1::decode_frame(&bytes).unwrap().verify(
            authority.config.network_id,
            authority.provider_id().unwrap(),
            authority.provider_role(crate::localnet::service_authorities::StreamTokenAuthorityRole::IssuerOperator).unwrap(),
            &policy.binding,
            State::native_world_schema_hash_v1().unwrap(),
            &checkpoint.verified_tip().unwrap(),
        ).unwrap()
    }

    pub(in crate::managed) fn bootstrap_reserve(
        &self,
        authority: &ServiceAuthority,
        policy: &ReserveAuthorityPolicyV1,
        checkpoint: &FinalityVerifier,
    ) -> VerifiedReserveAccountStateV1 {
        let operator = authority.reserve_operations_config().unwrap();
        let owner = authority.issuer_operator_config().unwrap();
        self.account_proof(&operator.account, authority.provider_id().unwrap())
            .verify(
                &ReserveAccountProofExpectedV1 {
                    chain: &authority.config.chain.to_string(),
                    network_id: authority.config.network_id,
                    operator: &operator.account,
                    provider_id: authority.provider_id().unwrap(),
                    owner: &owner.account,
                    policy,
                    schema: State::native_world_schema_hash_v1().unwrap(),
                },
                &checkpoint.verified_tip().unwrap(),
            )
            .unwrap()
    }
}

fn funding_finalities(report: ProviderFundingProgress) -> [ManagedTransactionFinality; 4] {
    let ProviderFundingProgress::Complete {
        request,
        approval,
        credit,
        capacity,
    } = report
    else {
        panic!("original funding child owners must report historical completion");
    };
    let request = request.unwrap();
    let approval = approval.unwrap();
    assert_eq!(request.movement_id(), approval.request().movement_id());
    assert_eq!(request.amount(), approval.request().amount());
    [*request.original(), *approval.original(), credit, capacity]
}

fn finalities(report: ServiceBootstrapProgress) -> Vec<ManagedTransactionFinality> {
    let ServiceBootstrapProgress::Complete(history) = report else {
        panic!("the parent must recover every original child independently");
    };
    history.ordered_carriers().unwrap()
}

#[test]
fn original_parent_recovers_all_native_children_offline_and_rejects_foreign_late_carrier() {
    // This native phase borrows the single original fixture and appends its authentic children.
    // No provider reports or fixture aggregate cross the function boundary.
    #[inline(never)]
    fn complete_native_providers(
        native: &mut NativeFixture,
        prepared: &PreparedLocalnet,
        original: &Original,
        authorization: &GeneratedBootstrapAuthorization,
        child_options: &BoundedTransactionOptions,
        utc: u64,
        expected: &mut Vec<ManagedTransactionFinality>,
    ) {
        let policies = &original.policies;
        for (slot, selected) in policies.providers.iter().enumerate() {
            let provider = selected.provider_id;
            let mut custody = ManagedStreamTokenCustody::open(prepared, provider).unwrap();
            let configure = if slot == 0 {
                custody.bootstrap_native_configure_generated(
                    native,
                    &selected.custody,
                    &authorization
                        .test_child(Purpose::CustodyConfigure(provider))
                        .unwrap(),
                    child_options,
                )
            } else {
                custody.bootstrap_native_configure(native, &selected.custody, utc, child_options)
            };
            let enroll = custody.bootstrap_native_enroll(
                native,
                &selected.custody,
                selected.initial_enrollment(now_ms().unwrap(), utc).unwrap(),
                child_options,
            );
            assert!(configure.current.is_none() && enroll.current.is_none());
            expected.extend([configure.finalized.unwrap(), enroll.finalized.unwrap()]);
            drop(custody);
            let mut account = ManagedReserveAccountRegistration::open(prepared, provider).unwrap();
            let registered = account.bootstrap_native(
                native,
                &policies.network.reserve,
                &original.underwriting[slot],
                utc,
                child_options,
            );
            assert!(registered.current.is_none());
            expected.push(registered.finalized.unwrap());
            drop(account);
            let mut funding = ProviderFundingBootstrap::open(prepared, provider).unwrap();
            expected.extend(funding_finalities(funding.bootstrap_native(
                native,
                &policies.network.reserve,
                utc,
                child_options,
            )));
            drop(funding);
            let mut ingest =
                ManagedInitialProviderIngestAuthority::open(prepared, provider).unwrap();
            expected.push(
                ingest
                    .bootstrap_native(native, &selected.provider_ingest, utc, child_options)
                    .finalized
                    .unwrap(),
            );
            drop(ingest);
            let mut gateway = ManagedInitialGatewaySetup::open(prepared, provider).unwrap();
            expected.push(
                gateway
                    .bootstrap_native(native, &selected.gateway, utc, child_options)
                    .finalized
                    .unwrap(),
            );
            drop(gateway);
        }
    }

    // All original offline recovery, refusal, restoration and worker cleanup stays sequential.
    // Borrow the existing snapshots so complete parent progress stays local to this phase.
    #[inline(never)]
    #[expect(
        clippy::too_many_arguments,
        reason = "Borrow the original owners without constructing another fixture aggregate"
    )]
    fn assert_original_offline_recovery(
        prepared: &PreparedLocalnet,
        original: &Original,
        authorization: &GeneratedBootstrapAuthorization,
        directory: &PrivateDirectory,
        parent_bytes: &[u8],
        requested_utc: u64,
        previous_terms: &crate::managed::native_operation::Terms,
        expected: &[ManagedTransactionFinality],
        enrollment_roots: &[(
            PrivateDirectory,
            PrivateDirectory,
            Vec<u8>,
            Vec<u8>,
            Vec<u8>,
        )],
        retained: &[(PrivateDirectory, Vec<u8>, Vec<u8>)],
        temporary: &tempfile::TempDir,
        peers: &mut UnavailablePeers,
    ) {
        let policies = &original.policies;
        for _ in 0..2 {
            let mut owner = ManagedServiceBootstrap::open(prepared).unwrap();
            let fresh = original
                .fees
                .options(Instant::now() + Duration::from_secs(30));
            assert_eq!(finalities(owner.recover(fresh.deadline).unwrap()), expected);
            assert_eq!(
                finalities(owner.advance(authorization, fresh.deadline).unwrap()),
                expected
            );
            let epoch_inventory = directory
                .open_child("epochs")
                .unwrap()
                .entries(128)
                .unwrap();
            assert!(owner.authorize_test_startup(&fresh).unwrap().is_none());
            assert_eq!(
                directory
                    .open_child("epochs")
                    .unwrap()
                    .entries(128)
                    .unwrap(),
                epoch_inventory
            );
            assert_eq!(
                encode(&owner.selected_policies().unwrap(), MAX_ORIGINAL_BYTES).unwrap(),
                encode(policies, MAX_ORIGINAL_BYTES).unwrap()
            );
            assert!(previous_terms.matches(requested_utc + 1, &fresh).is_err());
            let mut wrong = original.fees.options(fresh.deadline);
            wrong.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(65));
            assert!(owner.authorize_test_startup(&wrong).is_err());
            wrong = original.fees.options(fresh.deadline);
            wrong.max_total_fees.clear();
            assert!(owner.authorize_test_startup(&wrong).is_err());
            assert!(peers.requests.lock().unwrap().is_empty());
        }
        assert_eq!(
            directory
                .read("original.nrt", MAX_ORIGINAL_BYTES)
                .unwrap()
                .as_slice(),
            parent_bytes
        );
        for (owner, root, selection, anchor, reference) in enrollment_roots {
            assert_eq!(
                &std::fs::read(root.path().join("original.nrt")).unwrap(),
                selection
            );
            assert_eq!(
                &std::fs::read(root.path().join("anchor.nrt")).unwrap(),
                anchor
            );
            assert_eq!(
                &std::fs::read(owner.path().join("enroll-selection.nrt")).unwrap(),
                reference
            );
        }
        for (child, original, operation) in retained {
            assert_eq!(
                &std::fs::read(child.path().join("original.nrt")).unwrap(),
                original
            );
            let transaction = child
                .open_child("attempts")
                .unwrap()
                .open_child("0001")
                .unwrap()
                .open_child("transaction")
                .unwrap();
            assert_eq!(
                &std::fs::read(transaction.path().join("operation.json")).unwrap(),
                operation
            );
            assert!(!transaction.path().join("submission.json").exists());
        }
        let gateway = retained[27]
            .0
            .open_child("attempts")
            .unwrap()
            .open_child("0001")
            .unwrap();
        let reputation = retained[28]
            .0
            .open_child("attempts")
            .unwrap()
            .open_child("0001")
            .unwrap();
        let original_carrier = reputation
            .read("carrier.nrt", MAX_CHECKPOINT_BYTES)
            .unwrap();
        let foreign_carrier = gateway.read("carrier.nrt", MAX_CHECKPOINT_BYTES).unwrap();
        reputation
            .write_atomic("carrier.nrt", &foreign_carrier, PublishMode::Replace)
            .unwrap();
        let mut owner = ManagedServiceBootstrap::open(prepared).unwrap();
        assert!(
            owner
                .recover(Instant::now() + Duration::from_secs(30))
                .is_err()
        );
        assert!(peers.requests.lock().unwrap().is_empty());
        reputation
            .write_atomic("carrier.nrt", &[0xA5], PublishMode::Replace)
            .unwrap();
        assert!(
            owner
                .recover(Instant::now() + Duration::from_secs(30))
                .is_err()
        );
        assert!(peers.requests.lock().unwrap().is_empty());
        reputation
            .write_atomic("carrier.nrt", &original_carrier, PublishMode::Replace)
            .unwrap();
        assert_eq!(
            finalities(
                owner
                    .recover(Instant::now() + Duration::from_secs(30))
                    .unwrap()
            ),
            expected
        );
        assert!(peers.requests.lock().unwrap().is_empty());
        // Remove only parent custody after all 29 genuine wallet/native originals exist. Neither
        // a missing initial directory nor an empty replacement may reconstruct the paid parent.
        let saved = temporary.path().join("retained-parent-original");
        std::fs::rename(directory.path(), &saved).unwrap();
        for initial_exists in [false, true] {
            if initial_exists {
                owner.authority.directory.ensure_child("initial").unwrap();
            }
            let names = owner.authority.directory.entries(4).unwrap();
            let fresh = original
                .fees
                .options(Instant::now() + Duration::from_secs(30));
            assert!(owner.authorize_test_startup(&fresh).is_err());
            assert_eq!(owner.authority.directory.entries(4).unwrap(), names);
            if initial_exists {
                require_empty(&owner.authority.directory.open_child("initial").unwrap()).unwrap();
            } else {
                assert!(!owner.authority.directory.path().join("initial").exists());
            }
            assert!(peers.requests.lock().unwrap().is_empty());
            for (owner, root, selection, anchor, reference) in enrollment_roots {
                assert_eq!(
                    &std::fs::read(root.path().join("original.nrt")).unwrap(),
                    selection
                );
                assert_eq!(
                    &std::fs::read(root.path().join("anchor.nrt")).unwrap(),
                    anchor
                );
                assert_eq!(
                    &std::fs::read(owner.path().join("enroll-selection.nrt")).unwrap(),
                    reference
                );
            }
            for (child, original, operation) in retained {
                assert_eq!(
                    &std::fs::read(child.path().join("original.nrt")).unwrap(),
                    original
                );
                assert_eq!(
                    &std::fs::read(
                        child
                            .path()
                            .join("attempts/0001/transaction/operation.json")
                    )
                    .unwrap(),
                    operation
                );
            }
        }
        std::fs::remove_dir(owner.authority.directory.path().join("initial")).unwrap();
        std::fs::rename(&saved, owner.authority.directory.path().join("initial")).unwrap();
        assert_eq!(
            finalities(
                owner
                    .recover(Instant::now() + Duration::from_secs(30))
                    .unwrap()
            ),
            expected
        );
        assert!(peers.requests.lock().unwrap().is_empty());
        peers.finish();
    }

    // Inspect the actual signed carrier and committed charge, not a synthetic fee estimate.
    #[inline(never)]
    fn assert_native_fee(
        native: &NativeFixture,
        height: u64,
        options: &BoundedTransactionOptions,
        instruction_count: usize,
        gas: u64,
    ) {
        let committed = native.chain.committed(height);
        let block = committed.block();
        let input = block.network_entrypoint_at(0).unwrap();
        let iroha_data_model::transaction::TransactionEntrypoint::External(signed) = input else {
            panic!("bootstrap must retain its real wallet-signed external input");
        };
        let iroha_data_model::transaction::Executable::Instructions(instructions) =
            signed.instructions()
        else {
            panic!("bootstrap must execute its exact native instruction vector");
        };
        assert_eq!(instructions.len(), instruction_count);
        assert_eq!(iroha_core::gas::meter_instructions(instructions), gas);
        assert_eq!(
            signed.fee_payment_intent().gas_limit(),
            options.fee_payment.gas_limit()
        );
        let (_, output) = block.network_output_at(0).unwrap();
        let receipt = output
            .result
            .nexus_fee_receipt()
            .expect("actual native fee settlement");
        receipt.validate_for_network_input(input, height).unwrap();
        assert_eq!(receipt.schedule.instruction_count, instruction_count as u64);
        assert_eq!(receipt.schedule.gas_used, gas);
        assert!(receipt.fee_amount <= *options.max_total_fees.get(&receipt.fee_asset_id).unwrap());
        let view = native.chain.state().view();
        assert_eq!(
            receipt.schedule.per_instruction_fee,
            view.nexus().fees.per_instruction_fee
        );
        assert_eq!(
            receipt.schedule.per_gas_unit_fee,
            view.nexus().fees.per_gas_unit_fee
        );
    }
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "native-service-bootstrap",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let mut owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let deadline = Instant::now() + Duration::from_secs(600);
    // Exercise the supported generated native-only intent. Exact quote charge maxima and the
    // immutable parent fee ceiling still apply; gateway's three ISIs meter 320 gas.
    let options = generated_fees(deadline).unwrap().options(deadline);
    assert!(options.fee_payment.gas_limit().is_none());
    let mut authorization = owner.authorize_test_startup(&options).unwrap().unwrap();
    let requested_utc = authorization.test_terms().requested_deadline_unix_ms;
    let utc = authorization.test_terms().signing_deadline_unix_ms;
    assert!(utc <= requested_utc);
    let directory = owner.authority.directory.open_child("initial").unwrap();
    let original = read_original(&directory, &owner.authority)
        .unwrap()
        .unwrap();
    let parent_bytes = encode(&original, MAX_ORIGINAL_BYTES).unwrap();
    let child_options = original.fees.options(options.deadline);
    let policies = &original.policies;
    let mut native = NativeFixture::from_generated(&prepared, &owner.authority);
    let log = quote_instructions(
        &native,
        &owner.authority.config,
        [InstructionBox::from(Log::new(
            iroha_data_model::Level::INFO,
            "parent bootstrap prerequisites".into(),
        ))],
    );
    native.bootstrap_commit(&owner.authority, &log);
    drop(ports);

    let mut reserve = ManagedInitialReservePolicy::open(&prepared).unwrap();
    let reserve_result = reserve.bootstrap_native_generated(
        &mut native,
        &policies.network.reserve,
        &authorization.test_child(Purpose::ReservePolicy).unwrap(),
        &child_options,
    );
    assert!(reserve_result.current.is_none() && reserve_result.activation().is_none());
    let mut expected = vec![reserve_result.finalized.unwrap()];
    drop(reserve);
    let previous_terms = authorization.test_terms().clone();
    authorization = owner.authorize_test_startup(&options).unwrap().unwrap();
    assert_eq!(authorization.test_ordinal(), 2);
    assert_eq!(
        directory
            .open_child("epochs")
            .unwrap()
            .entries(128)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        directory
            .read("original.nrt", MAX_ORIGINAL_BYTES)
            .unwrap()
            .as_slice(),
        parent_bytes.as_slice()
    );
    assert!(previous_terms.fees == authorization.test_terms().fees);
    complete_native_providers(
        &mut native,
        &prepared,
        &original,
        &authorization,
        &child_options,
        utc,
        &mut expected,
    );
    let mut reputation = ManagedInitialReputationPolicy::open(&prepared).unwrap();
    expected.push(
        reputation
            .bootstrap_native(
                &mut native,
                &policies.gateway_labels(),
                &policies.network.reputation,
                utc,
                &child_options,
            )
            .finalized
            .unwrap(),
    );
    drop(reputation);
    assert_eq!(expected.len(), 29);
    assert_eq!(
        expected
            .iter()
            .map(|original| original.height)
            .collect::<Vec<_>>(),
        (3..=31).collect::<Vec<_>>()
    );
    for (index, finalized) in expected.iter().enumerate() {
        // Reserve, every single-ISI provider child and reputation use the fixed native cost;
        // each exact gateway vector also grants its operator and observer permissions.
        let gateway = matches!(index, 9 | 18 | 27);
        assert_native_fee(
            &native,
            finalized.height,
            &options,
            if gateway { 3 } else { 1 },
            if gateway { 320 } else { 128 },
        );
    }
    drop(owner);

    let generation =
        PrivateDirectory::open_exact(prepared.context.client_config.parent().unwrap()).unwrap();
    let runtime = generation
        .open_child("runtime")
        .unwrap()
        .open_child("service-operations")
        .unwrap();
    let mut children: Vec<(PrivateDirectory, &str)> = vec![(
        runtime
            .open_child("network")
            .unwrap()
            .open_child("initial-reserve-policy")
            .unwrap(),
        "set",
    )];
    for slot in 0..3 {
        let scope = runtime
            .open_child("providers")
            .unwrap()
            .open_child(&slot.to_string())
            .unwrap();
        for (owner, purpose) in [
            ("stream-token-custody", "configure"),
            ("stream-token-custody", "enroll"),
            ("reserve-account-registration", "register"),
            ("reserve-top-up-request", "request"),
            ("reserve-top-up-approval", "approval"),
            ("initial-provider-credit", "install"),
            ("provider-capacity-declaration", "declare"),
            ("initial-provider-ingest-authority", "setup"),
            ("initial-gateway-setup", "setup"),
        ] {
            children.push((scope.open_child(owner).unwrap(), purpose));
        }
    }
    children.push((
        runtime
            .open_child("network")
            .unwrap()
            .open_child("initial-reputation-policy")
            .unwrap(),
        "setup",
    ));
    let mut enrollment_roots = Vec::new();
    let retained: Vec<_> = children
        .into_iter()
        .map(|(owner, purpose)| {
            let root = owner.open_child(purpose).unwrap();
            let child = if purpose == "enroll" {
                let selection = std::fs::read(root.path().join("original.nrt")).unwrap();
                let anchor = std::fs::read(root.path().join("anchor.nrt")).unwrap();
                let reference = std::fs::read(owner.path().join("enroll-selection.nrt")).unwrap();
                let body = root
                    .open_child("bodies")
                    .unwrap()
                    .open_child("0001")
                    .unwrap();
                enrollment_roots.push((owner, root, selection, anchor, reference));
                body
            } else {
                root
            };
            let original = std::fs::read(child.path().join("original.nrt")).unwrap();
            let transaction = child
                .open_child("attempts")
                .unwrap()
                .open_child("0001")
                .unwrap()
                .open_child("transaction")
                .unwrap();
            let operation = std::fs::read(transaction.path().join("operation.json")).unwrap();
            assert!(!transaction.path().join("submission.json").exists());
            (child, original, operation)
        })
        .collect();
    let mut peers = UnavailablePeers::start(&prepared);
    assert_original_offline_recovery(
        &prepared,
        &original,
        &authorization,
        &directory,
        &parent_bytes,
        requested_utc,
        &previous_terms,
        &expected,
        &enrollment_roots,
        &retained,
        &temporary,
        &mut peers,
    );
}

#[test]
fn generated_runtime_fee_ceiling_pays_real_native_isi_fee_from_original_role() {
    use crate::managed::native_operation::test_support::native_fixture::balance;
    use iroha_data_model::{asset::AssetId, transaction::TransactionBuilder};
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "native-runtime-fees",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let owner = ManagedServiceBootstrap::open(&prepared).unwrap();
    let policies = GeneratedServicePolicies::select(&owner.authority).unwrap();
    let mut native = NativeFixture::from_generated(&prepared, &owner.authority);
    let provider = policies.providers[0].provider_id;
    let selected = ServiceAuthority::open_provider(
        &prepared,
        provider,
        super::super::service_authority::ProviderPurpose::ProviderCapacityDeclaration,
    )
    .unwrap();
    let issuer = selected.issuer_operator_config().unwrap();
    let asset = AssetId::new(
        policies.network.reserve.asset_definition.clone(),
        issuer.account.clone(),
    );
    let before = balance(native.chain.state(), &asset);
    let mut builder = TransactionBuilder::new(
        issuer.network_id,
        issuer.account.clone(),
        policies.network.runtime_fee_payment.clone(),
    )
    .with_instructions([Log::new(
        iroha_data_model::Level::INFO,
        "generated runtime fee component".into(),
    )]);
    builder.set_creation_time(Duration::from_millis(
        now_ms()
            .unwrap()
            .max(native.chain.genesis().header().creation_time_ms + 1),
    ));
    builder.set_ttl(Duration::from_secs(300));
    let signed = builder.try_sign(issuer.key_pair.private_key()).unwrap();
    assert_eq!(native.chain.commit(vec![signed.clone()]), vec![true]);
    let observed = native.observe(&owner.authority);
    assert_eq!(
        observed
            .verified_tip()
            .unwrap()
            .block()
            .network_entrypoint_count(),
        1
    );
    let original = verify_carrier(&observed, &signed).unwrap();
    assert_eq!(original.height, 2);
    let paid = before
        .checked_sub(&balance(native.chain.state(), &asset))
        .unwrap();
    assert!(!paid.is_zero());
    assert!(paid <= Quantity::from(1_u64));
    assert_eq!(
        signed.fee_payment_intent(),
        &policies.network.runtime_fee_payment
    );
    // Real generated native fee execution, not full stream-token operation or Serving evidence.
}
