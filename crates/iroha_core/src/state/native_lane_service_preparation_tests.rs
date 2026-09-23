// Actual Native sources through the service that owns State, Kura and archives.
// Test reservations fund exact phase custody; they do not supply production
// admission or publication authority.

struct NativeServicePreparationFixture {
    state: Arc<State>,
    service: crate::sumeragi::v2_apply::V2ApplyService,
    proposal: SignedBlock,
    context: crate::sumeragi::v2::VerifiedHeightContext,
    provider: Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
    reputation: Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
    reputation_enabled: bool,
    budget: mv::allocation::AllocationBudget,
    source_asset: AssetId,
    destination_asset: AssetId,
    pulse: iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
    _directory: tempfile::TempDir,
}

#[inline(never)]
fn native_service_preparation_fixture(atomic: bool) -> Box<NativeServicePreparationFixture> {
    native_service_preparation_from_original(native_publication_fixture(atomic), true)
}

#[inline(never)]
fn native_service_capture_fixture() -> Box<NativeServicePreparationFixture> {
    // The ordinary Native genesis does not publish the governed SoraFS policy
    // history required by a configured reputation archive. Keep that archive
    // disabled in positive capture tests; a separate test proves the actual
    // configured archive refuses without its authoritative policy.
    native_service_preparation_from_original(native_publication_fixture(false), false)
}

#[inline(never)]
fn native_service_preparation_from_original(
    original: Box<NativePublicationFixture>,
    reputation_enabled: bool,
) -> Box<NativeServicePreparationFixture> {
    use crate::query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveV1,
        },
        reputation_finalized::{ReputationFinalizedArchive, ReputationFinalizedArchiveBounds},
    };
    let proposal = original.carrier().clone();
    let context = original.verified_context();
    let source_asset = original.assets().0.clone();
    let destination_asset = original.assets().1.clone();
    let pulse = *original.pulse();
    let state = original.into_shared_state();
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let provider = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(
            root.join("provider"),
            ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
                .unwrap(),
        )
        .unwrap(),
    );
    let reputation = Arc::new(
        ReputationFinalizedArchive::try_open(
            root.join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    let queue = Arc::new(crate::queue::Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &iroha_primitives::time::TimeSource::new_system(),
    ));
    let (events, _) = tokio::sync::broadcast::channel(32);
    let pops = native_preparation_global_keys(context.context())
        .iter()
        .map(|key| bls_normal_pop_prove(key.private_key()).unwrap())
        .collect();
    let service = crate::sumeragi::v2_apply::V2ApplyService::new(
        Arc::clone(&state),
        queue,
        Arc::clone(&state.kura),
        Some(Arc::clone(&provider)),
        reputation_enabled.then(|| Arc::clone(&reputation)),
        state.sumeragi_block_cadence(),
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events,
        pops,
    );
    Box::new(NativeServicePreparationFixture {
        state,
        service,
        proposal,
        context,
        provider,
        reputation,
        reputation_enabled,
        budget: mv::allocation::AllocationBudget::new(16 << 20),
        source_asset,
        destination_asset,
        pulse,
        _directory: directory,
    })
}

impl NativeServicePreparationFixture {
    fn source(&self) -> super::PreparedNativeLaneBatchSourceV1<'_> {
        let groups = crate::block::native_lane_batch_for_execution(&self.proposal)
            .expect("fixture Native proposal")
            .groups
            .len();
        let admission =
            super::NativeExecutionResourceAdmission::try_reserve_source(&self.budget, groups)
                .expect("fixture source structure fits its shared finite pool");
        self.source_with_admission(admission)
    }

    fn source_with_admission(
        &self,
        admission: super::NativeExecutionResourceAdmission,
    ) -> super::PreparedNativeLaneBatchSourceV1<'_> {
        let super::NativeLaneBatchSourcePreparationV1::Ready(source) = self
            .state
            .prepare_proposed_native_lane_batch_source(self.proposal.clone(), &[], admission)
            .unwrap()
        else {
            panic!("real authenticated Native source must be ready");
        };
        source
    }

    fn reserve_provider(
        &self,
    ) -> Result<
        crate::query::provider_ingest_finalized::ProviderCandidateCapture,
        crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1,
    > {
        self.provider.try_reserve_candidate(
            crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveKeyV1::try_new(
                self.context.context().network_id,
                self.context.context().height,
                *self.proposal.hash().as_ref(),
                self.proposal.header().creation_time_ms,
            )
            .unwrap(),
            &self.state.kura,
        )
    }

    fn reserve_reputation(
        &self,
    ) -> Result<
        crate::query::reputation_finalized::ReputationCandidateCapture,
        crate::query::reputation_finalized::ReputationFinalizedArchiveError,
    > {
        self.reputation.try_reserve_candidate(
            crate::query::reputation_finalized::ReputationFinalizedArchiveKeyV1::try_new(
                self.context.context().network_id,
                self.context.context().height,
                *self.proposal.hash().as_ref(),
            )
            .unwrap(),
            self.proposal.header().creation_time_ms,
            &self.state.kura,
        )
    }

    fn assert_archives_free(&self) {
        let provider = self.reserve_provider().unwrap();
        let reputation = self.reserve_reputation().unwrap();
        drop((provider, reputation));
    }

    fn assert_archives_reserved(&self) {
        assert!(matches!(self.reserve_provider(), Err(
            crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::CaptureReserved { .. }
        )));
        if self.reputation_enabled {
            assert!(matches!(self.reserve_reputation(), Err(
                crate::query::reputation_finalized::ReputationFinalizedArchiveError::CaptureReserved { .. }
            )));
        } else {
            drop(self.reserve_reputation().unwrap());
        }
    }
}

fn native_service_test_shells(
    budget: &mv::allocation::AllocationBudget,
) -> super::CarrierJournalShellReservation<()> {
    super::PreparedCarrier::reserve_journal_shells(budget)
        .expect("finite original shell capacity before Native execution")
}

state_test! { sync native_service_preparation_single_preserves_original_sources_and_archives
    assert_native_service_preparation_success(native_service_preparation_fixture(false), false);
}
state_test! { sync native_service_preparation_atomic_preserves_original_sources_and_archives
    assert_native_service_preparation_success(native_service_preparation_fixture(true), true);
}

state_test! { consensus_stack native_service_capture_retains_original_execution_and_prepaid_shells
    let fixture = native_service_capture_fixture();
    let no_capacity = mv::allocation::AllocationBudget::new(0);
    assert!(super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&no_capacity).is_err());
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&budget)
        .expect("finite original shell capacity before execution");
    let shell_bytes = budget.reserved_bytes();
    assert!(shell_bytes > 0);
    let source = fixture.source();
    let source_allocation = source.groups_for_test().as_ptr();
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, source, fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    assert!(budget.reserved_bytes() > shell_bytes, "original source and output charges remain with the prepared execution");
    let retained = match prepared.try_capture_original::<(), _>(|inputs| {
        assert_eq!(inputs.context.as_ref(), fixture.context.context());
        assert_eq!(inputs.valid.as_ref().canonical_resultless_proposal(), fixture.proposal);
        budget.try_reserve_bytes(1)
    }) {
        Ok(retained) => retained,
        Err(_) => panic!("original Native capture must complete"),
    };
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    let retained = retained
        .resume_capture()
        .unwrap_or_else(|(_, error)| panic!("uncontended original capture: {error:?}"));
    let super::RetainedCarrier::Validated(journals) = retained else {
        panic!("uncontended capture must retain the completed original journals");
    };
    assert_eq!(
        journals.native_source_for_test().unwrap().sources_for_test().as_ptr(),
        source_allocation,
    );
    journals.execution_prefix_commitment().validate().unwrap();
    assert!(budget.reserved_bytes() >= 1);
    fixture.assert_archives_reserved();
    drop(journals);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { consensus_stack native_service_configured_reputation_refusal_pins_the_original
    let fixture = native_service_preparation_fixture(false);
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&budget)
        .expect("finite original shell capacity before execution");
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    let captured = match prepared.try_capture_original::<(), _>(|_| budget.try_reserve_bytes(1)) {
        Ok(captured) => captured,
        Err(_) => panic!("missing governed policy must refuse after detachment"),
    };
    assert!(matches!(&captured, super::RetainedCarrier::Capturing(_)));
    assert!(captured.matches_validation_candidate(fixture.context.context(), &fixture.proposal));
    fixture.assert_archives_reserved();
    let (captured, error) = match captured.resume_capture() {
        Err(refusal) => refusal,
        Ok(_) => panic!("missing governed reputation policy cannot complete capture"),
    };
    let super::CarrierArchivePreparationError::Reputation(error) = error else {
        panic!("configured reputation archive must identify its missing authority policy");
    };
    assert!(matches!(
        error.as_ref(),
        crate::query::reputation_finalized::ReputationFinalizedArchiveError::ProjectionCaptureQuery {
            projection: "reputation authority policy",
            ..
        }
    ));
    let (captured, error) = match captured.resume_capture() {
        Err(refusal) => refusal,
        Ok(_) => panic!("permanent refusal must keep the same detached owner pinned"),
    };
    assert!(matches!(error, super::CarrierArchivePreparationError::Reputation(_)));
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(captured);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { consensus_stack native_service_capture_admission_refusal_keeps_the_original_writer_and_shells
    use crate::sumeragi::v2_apply::NativeCandidateCaptureRefusal;
    let fixture = native_service_capture_fixture();
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&budget)
        .expect("finite original shell capacity before execution");
    let shell_bytes = budget.reserved_bytes();
    let source = fixture.source();
    let source_allocation = source.groups_for_test().as_ptr();
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, source, fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    let retained_bytes = budget.reserved_bytes();
    assert!(retained_bytes > shell_bytes);
    let refused = match prepared.try_capture_original::<(), _>(|_| Err::<mv::allocation::AllocationReservation, _>("pool exhausted")) {
        Err(refused) => refused,
        Ok(_) => panic!("aggregate journal admission must refuse"),
    };
    let NativeCandidateCaptureRefusal::Journals(
        super::CarrierJournalPreparationError::JournalAdmission {
            carrier,
            provider,
            reputation,
            error,
            journal_shells,
        },
    ) = refused else {
        panic!("admission refusal must return the original writer and captures");
    };
    assert_eq!(error, "pool exhausted");
    assert_eq!(
        carrier.native_source_for_test().unwrap().sources_for_test().as_ptr(),
        source_allocation,
    );
    assert_eq!(budget.reserved_bytes(), retained_bytes);
    fixture.assert_archives_reserved();
    let journals = match carrier.prepare_journals(
        journal_shells,
        provider,
        reputation,
        |_| budget.try_reserve_bytes(1),
    ) {
        Ok(journals) => journals,
        Err(error) => panic!("same writer and prepaid shells must support synchronous retry: {error:?}"),
    };
    assert_eq!(
        journals.native_source_for_test().unwrap().sources_for_test().as_ptr(),
        source_allocation,
    );
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(journals);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { consensus_stack native_service_capture_archive_busy_resumes_the_same_execution
    use crate::sumeragi::v2_apply::validation_custody::CarrierValidator;
    let fixture = native_service_capture_fixture();
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&budget)
        .expect("finite original shell capacity before execution");
    let source = fixture.source();
    let source_allocation = source.groups_for_test().as_ptr();
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, source, fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    let (mut validator, retained) = fixture.provider.with_index_reader_for_test(|| {
        let captured = match prepared.try_capture_original::<(), _>(|_| budget.try_reserve_bytes(1)) {
            Ok(retained) => retained,
            Err(_) => panic!("index contention must return the detached original owner"),
        };
        assert!(matches!(&captured, super::RetainedCarrier::Capturing(_)));
        let (captured, error) = match captured.resume_capture() {
            Err(refusal) => refusal,
            Ok(_) => panic!("held index must provide a typed retry dependency"),
        };
        let super::CarrierArchivePreparationError::Provider(error) = error else {
            panic!("the held provider index must be the blocking dependency: {error:?}");
        };
        assert!(matches!(
            error.as_ref(),
            crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::IndexBusy { .. }
        ));
        let mut validator = crate::sumeragi::v2_apply::ReadyNativeCarrierValidator::new_with_original_for_test(
            fixture.context.context(),
            &fixture.proposal,
            captured,
            std::task::Waker::noop().clone(),
        );
        let retained = match validator.prepare(fixture.context.context(), &fixture.proposal) {
            Ok(retained) => retained,
            Err(error) => panic!("exact original must enter the marker adapter: {error}"),
        };
        assert!(validator
            .prepare(fixture.context.context(), &fixture.proposal)
            .is_err());
        let (retained, refusal) = match validator.resume(retained) {
            Err(refusal) => refusal,
            Ok(_) => panic!("held archive index must defer the same original"),
        };
        let crate::sumeragi::v2_body_store::LocalValidationRefusal::PhysicalBusy(busy) = refusal else {
            panic!("actual archive reader must supply a typed wake dependency");
        };
        assert_eq!(busy.resource, "provider archive index");
        (validator, retained)
    });
    assert!(matches!(&retained, super::RetainedCarrier::Capturing(_)));
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    let retained = match validator.resume(retained) {
        Ok(retained) => retained,
        Err(_) => panic!("released index must admit original capture"),
    };
    let super::RetainedCarrier::Validated(journals) = retained else {
        panic!("capture retry must retain original completed journals");
    };
    assert_eq!(
        journals.native_source_for_test().unwrap().sources_for_test().as_ptr(),
        source_allocation,
    );
    drop(journals);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { consensus_stack native_service_detached_execution_crosses_body_store_marker_once
    use crate::sumeragi::{
        v2_apply::{
            NativeCarrierInstallError, NativePreExecutionRetainedSlots,
            ReadyNativeCarrierValidator,
            validation_custody::CarrierCustodyError,
        },
        v2_body_store::{
            BlockSignaturePolicy, V2BodyStore, V2BodyStoreCapacity, V2BodyStoreError,
            fail_next_marker_file_sync,
        },
        v2_chunks::encode_payload,
    };
    use iroha_data_model::block::consensus_v2 as wire;

    type NativeValidator = ReadyNativeCarrierValidator<mv::allocation::AllocationReservation, ()>;
    let fixture = native_service_capture_fixture();
    let context = fixture.context.context().clone();
    let bytes = fixture.proposal.encode_wire().unwrap();
    let subject = wire::BlockSubject {
        parent_block_hash: fixture.proposal.header().prev_block_hash(),
        block_hash: fixture.proposal.hash(),
        payload_hash: Hash::new(&bytes),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let manifest = encode_payload(&context, round, subject, &bytes)
        .unwrap()
        .manifest()
        .clone();
    let mut store = V2BodyStore::open_with_policy_and_capacity(
        fixture._directory.path().join("retained-native-bodies"),
        context.clone(),
        BlockSignaturePolicy::RotatingLeader,
        V2BodyStoreCapacity::for_test(1, 16 << 20).unwrap(),
    )
    .unwrap();
    let durable = store.store(manifest, bytes).unwrap();
    let budget = fixture.budget.clone();
    let descriptor_bytes = store.retained_validation_descriptor_bytes::<NativeValidator>().unwrap();
    let slots = NativePreExecutionRetainedSlots::<mv::allocation::AllocationReservation, ()>::try_reserve(
        &store, &budget, &context, &fixture.proposal, std::task::Waker::noop().clone(),
    ).expect("fixed original retention slots from one finite pool");
    assert!(budget.reserved_bytes() > descriptor_bytes);
    let (source_admission, shells, mut service) = slots.into_parts();
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    let source = fixture.source_with_admission(source_admission);
    let source_allocation = source.groups_for_test().as_ptr();
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, source, fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    let retained = match prepared.try_capture_original::<(), _>(|_| budget.try_reserve_bytes(1)) {
        Ok(retained) => retained,
        Err(_) => panic!("original Native execution must detach"),
    };
    let retained = retained
        .resume_capture()
        .unwrap_or_else(|(_, error)| panic!("Native marker capture: {error:?}"));
    let commitment = retained.ready_commitment().expect("complete original capture");
    let mut foreign_context = context.clone();
    foreign_context.height += 1;
    let (retained, error) = service.install_native_original(&foreign_context, &fixture.proposal, retained)
        .err().expect("foreign context must return the detached original");
    assert!(matches!(error, NativeCarrierInstallError::Identity));
    let mut foreign_proposal = fixture.proposal.clone();
    let key = native_preparation_global_keys(&context).remove(0);
    let signature =
        iroha_crypto::SignatureOf::try_from_hash(key.private_key(), foreign_proposal.header().hash())
            .unwrap();
    foreign_proposal
        .add_signature(iroha_data_model::block::BlockSignature::new(100, signature))
        .unwrap();
    assert_eq!(foreign_proposal.hash(), fixture.proposal.hash());
    let (retained, error) = service.install_native_original(&context, &foreign_proposal, retained)
        .err().expect("same block hash with different proposal wire must return the original");
    assert!(matches!(error, NativeCarrierInstallError::Identity));
    if let Err((_, error)) = service.install_native_original(&context, &fixture.proposal, retained) {
        panic!("exact detached execution must fill the original descriptor: {error}");
    }
    fail_next_marker_file_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(), durable.manifest_hash(), &mut service,
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    assert_eq!(service.marker_counts_for_test(), (1, 0));
    assert!(service.owner_for_test(subject).is_some());
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    let receipt = store
        .execute_retained_durable_validation(
            durable.clone(), durable.manifest_hash(), &mut service,
        )
        .unwrap()
        .into_validated_receipt()
        .unwrap();
    assert_eq!(receipt.execution_commitment(), commitment);
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    let original = service.owner_for_test(subject).expect("marker retains original execution");
    let super::RetainedCarrier::Validated(journals) = original else {
        panic!("marker must retain complete Native journals");
    };
    assert_eq!(journals.native_source_for_test().unwrap().sources_for_test().as_ptr(), source_allocation);
    let withheld = service.select(&receipt).unwrap().try_consume(|_, owner| {
        assert_eq!(owner.ready_commitment(), Some(commitment));
        Err::<(), _>((owner, "publisher not ready"))
    });
    assert_eq!(withheld, Err("publisher not ready"));
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    let same_receipt = store
        .execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service)
        .unwrap()
        .into_validated_receipt()
        .unwrap();
    assert_eq!(same_receipt, receipt);
    let mut completed_original = None;
    service.select(&receipt).unwrap().try_consume(|_, owner| {
        completed_original = Some(owner);
        Ok::<_, (super::RetainedCarrier<mv::allocation::AllocationReservation, ()>, ())>(())
    }).unwrap();
    let (completed_original, error) = service.install_native_original(
        &context,
        &fixture.proposal,
        completed_original.take().expect("the marker's original owner"),
    ).err().expect("a consumed descriptor cannot accept a second original");
    assert!(matches!(error, NativeCarrierInstallError::AlreadyInstalled));
    let super::RetainedCarrier::Validated(journals) = &completed_original else {
        panic!("failed second install must return the same complete original");
    };
    assert_eq!(journals.native_source_for_test().unwrap().sources_for_test().as_ptr(), source_allocation);
    drop(completed_original);
    assert!(matches!(
        store.execute_retained_durable_validation(durable.clone(), durable.manifest_hash(), &mut service),
        Err(V2BodyStoreError::CarrierCustody(CarrierCustodyError::MissingOwner))
    ));
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(service);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_service_descriptor_exhaustion_precedes_execution
    use crate::sumeragi::{
        v2_apply::{NativeCarrierInstallError, NativePreExecutionRetainedSlots, NativeRetainedSlotsError, ReadyNativeCarrierValidator, validation_custody::CarrierCustodyError},
        v2_body_store::{BlockSignaturePolicy, V2BodyStore, V2BodyStoreCapacity, V2BodyStoreError},
    };
    type NativeValidator = ReadyNativeCarrierValidator<mv::allocation::AllocationReservation, ()>;
    let fixture = native_service_capture_fixture();
    let context = fixture.context.context().clone();
    let store = V2BodyStore::open_with_policy_and_capacity(
        fixture._directory.path().join("descriptor-admission-bodies"),
        context.clone(),
        BlockSignaturePolicy::RotatingLeader,
        V2BodyStoreCapacity::for_test(1, 16 << 20).unwrap(),
    ).unwrap();
    let probe = mv::allocation::AllocationBudget::new(4 << 20);
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&probe)
        .expect("original shell layout");
    let shell_bytes = probe.reserved_bytes();
    drop(shells);
    assert_eq!(probe.reserved_bytes(), 0);
    assert!(shell_bytes > 0);
    let group_count = crate::block::native_lane_batch_for_execution(&fixture.proposal)
        .unwrap()
        .groups
        .len();
    let source_bytes = super::NativeSourceStructuralDemand::plan(group_count)
        .unwrap()
        .total_bytes();
    let source_shortage = mv::allocation::AllocationBudget::new(source_bytes - 1);
    assert!(matches!(
        NativePreExecutionRetainedSlots::<mv::allocation::AllocationReservation, ()>::try_reserve(
            &store, &source_shortage, &context, &fixture.proposal, std::task::Waker::noop().clone(),
        ),
        Err(NativeRetainedSlotsError::Source(mv::allocation::AllocationRefusal::ExceedsLimit { .. }))
    ));
    assert_eq!(source_shortage.reserved_bytes(), 0);
    let descriptor_bytes = store.retained_validation_descriptor_bytes::<NativeValidator>().unwrap();
    assert!(descriptor_bytes > 0);
    let shell_shortage = mv::allocation::AllocationBudget::new(source_bytes + shell_bytes - 1);
    assert!(matches!(
        NativePreExecutionRetainedSlots::<mv::allocation::AllocationReservation, ()>::try_reserve(
            &store, &shell_shortage, &context, &fixture.proposal, std::task::Waker::noop().clone(),
        ),
        Err(NativeRetainedSlotsError::JournalShells(mv::allocation::AllocationRefusal::Capacity { .. }))
    ));
    assert_eq!(shell_shortage.reserved_bytes(), 0);
    let budget = mv::allocation::AllocationBudget::new(source_bytes + shell_bytes + descriptor_bytes - 1);
    let mut foreign_context = context.clone();
    foreign_context.height += 1;
    assert!(matches!(
        NativePreExecutionRetainedSlots::<mv::allocation::AllocationReservation, ()>::try_reserve(
            &store, &budget, &foreign_context, &fixture.proposal, std::task::Waker::noop().clone(),
        ),
        Err(NativeRetainedSlotsError::Identity(NativeCarrierInstallError::Identity))
    ));
    assert_eq!(budget.reserved_bytes(), 0);
    let result_bearing = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    assert!(matches!(NativeValidator::new_pending(&context, &result_bearing, std::task::Waker::noop().clone()), Err(NativeCarrierInstallError::Identity)));
    let error = NativePreExecutionRetainedSlots::<mv::allocation::AllocationReservation, ()>::try_reserve(
        &store, &budget, &context, &fixture.proposal, std::task::Waker::noop().clone(),
    )
        .err().expect("descriptor shortage must refuse before Native execution");
    assert!(matches!(error, NativeRetainedSlotsError::Descriptors(V2BodyStoreError::CarrierCustody(CarrierCustodyError::DescriptorAdmission(mv::allocation::AllocationRefusal::Capacity { .. })))));
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

#[inline(never)]
fn assert_native_service_preparation_success(
    fixture: Box<NativeServicePreparationFixture>,
    atomic: bool,
) {
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let files = exact_test_tree_fingerprint(&fixture.state.kura.store_root());
    let generation = fixture.state.state_view_generation();
    let source = fixture.source();
    assert_eq!(source.groups_for_test().len(), 1);
    let groups = source.groups_for_test().as_ptr();
    let body = source.groups_for_test()[0]
        .body()
        .canonical_bytes()
        .as_ptr();
    let decisions = source.groups_for_test()[0].decisions().as_ptr();
    let contexts = source.groups_for_test()[0].contexts().as_ptr();
    let prepared = fixture
        .service
        .prepare_native_source(
            &fixture.proposal,
            source,
            fixture.context.clone(),
            native_service_test_shells(&fixture.budget),
        )
        .unwrap()
        .expect("same original applying source");
    fixture.assert_archives_reserved();
    // The logical archive owners exclude mutation without retaining index locks.
    fixture.provider.with_index_reader_for_test(|| ());
    fixture.reputation.with_index_reader_for_test(|| ());
    let (carrier, provider, reputation, _shells) = match prepared.try_into_parts() {
        Ok(parts) => parts,
        Err(refusal) => {
            panic!(
                "configured original post-execution dependencies: {:?}",
                refusal.error
            )
        }
    };
    assert!(provider.is_some() && reputation.is_some());
    let custody = carrier.native_source_for_test().unwrap();
    assert_eq!(custody.context().context(), fixture.context.context());
    assert_eq!(custody.sources_for_test().as_ptr(), groups);
    assert_eq!(
        custody.sources_for_test()[0]
            .body()
            .canonical_bytes()
            .as_ptr(),
        body
    );
    assert_eq!(
        custody.sources_for_test()[0].decisions().as_ptr(),
        decisions
    );
    assert_eq!(custody.sources_for_test()[0].contexts().as_ptr(), contexts);
    assert_eq!(
        custody.sources_for_test()[0].contexts().len(),
        if atomic { 2 } else { 1 }
    );
    assert_eq!(
        carrier.block().canonical_resultless_proposal(),
        fixture.proposal
    );
    carrier
        .block()
        .validate_execution_result_structure()
        .unwrap();
    carrier.block().validate_output_merkle_cache().unwrap();
    assert_eq!(carrier.block().execution_outputs().len(), 1);
    assert!(carrier.block().execution_outputs()[0].result().is_ok());
    let wire = carrier.block().encode_wire().unwrap();
    let commitment = carrier.execution_prefix_commitment();
    commitment.validate().unwrap();
    assert_eq!(commitment.executed_block_wire_len, wire.len() as u64);
    assert_eq!(commitment.executed_block_wire_hash, Hash::new(wire));
    assert_eq!(
        carrier
            .state()
            .world
            .assets
            .get(&fixture.source_asset)
            .unwrap()
            .0,
        Quantity::from(75u32)
    );
    assert_eq!(
        carrier
            .state()
            .world
            .assets
            .get(&fixture.destination_asset)
            .unwrap()
            .0,
        Quantity::from(25u32)
    );
    assert_eq!(
        carrier
            .state()
            .world
            .global_beacon_pulses
            .get(&fixture.pulse.pulse_id),
        Some(&fixture.pulse)
    );
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(carrier);
    fixture.assert_archives_reserved();
    drop((provider, reputation));
    fixture.assert_archives_free();
    // Abandoning the whole synchronous owner must also release both captures,
    // after its original State writers, without publishing this second attempt.
    let abandoned = fixture
        .service
        .prepare_native_source(
            &fixture.proposal,
            fixture.source(),
            fixture.context.clone(),
            native_service_test_shells(&fixture.budget),
        )
        .unwrap()
        .unwrap();
    fixture.assert_archives_reserved();
    drop(abandoned);
    fixture.assert_archives_free();
    assert_eq!(fixture.service.candidate_executions_for_test(), 2);
    assert_eq!(fixture.state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(),
        before
    );
    assert_eq!(
        exact_test_tree_fingerprint(&fixture.state.kura.store_root()),
        files
    );
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_service_preparation_index_busy_precedes_execution
    let fixture = native_service_preparation_fixture(false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let files = exact_test_tree_fingerprint(&fixture.state.kura.store_root());
    for provider_busy in [true, false] {
        let source = fixture.source();
        let probe = || fixture.service.prepare_native_source(
            &fixture.proposal, source, fixture.context.clone(), native_service_test_shells(&fixture.budget),
        ).err().expect("held original archive reader must refuse");
        let error = if provider_busy {
            fixture.provider.with_index_reader_for_test(probe)
        } else {
            fixture.reputation.with_index_reader_for_test(probe)
        };
        assert_native_service_local_busy(error, if provider_busy { "provider_archive_index" } else { "reputation_archive_index" });
        fixture.assert_archives_free();
        assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    }
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&fixture.state.kura.store_root()), files);
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

fn assert_native_service_local_busy(
    error: crate::sumeragi::v2_apply::V2ApplyError,
    resource: &'static str,
) {
    use crate::sumeragi::{
        v2_apply::V2ApplyError, v2_body_store::BodyValidationError,
        v2_body_store::LocalValidationRefusal,
    };
    assert!(error.rejection_identity().is_none());
    match error {
        V2ApplyError::LocalValidation(LocalValidationRefusal::PhysicalBusy(busy)) => {
            assert_eq!(busy.resource, resource)
        }
        error => panic!("expected {resource}, got {error:?}"),
    }
}

state_test! { sync native_service_preparation_capture_busy_releases_partial_owner
    let fixture = native_service_preparation_fixture(false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let first = fixture.reserve_provider().unwrap();
    let error = fixture.service.prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone(), native_service_test_shells(&fixture.budget))
        .err().expect("original provider capture excludes the candidate");
    assert_native_service_local_busy(error, "provider_archive_capture");
    drop(first);
    let second = fixture.reserve_reputation().unwrap();
    let error = fixture.service.prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone(), native_service_test_shells(&fixture.budget))
        .err().expect("original reputation capture excludes the candidate");
    assert_native_service_local_busy(error, "reputation_archive_capture");
    drop(fixture.reserve_provider().unwrap());
    drop(second);
    fixture.assert_archives_free();
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_service_preparation_stale_source_skips_archives_and_execution
    let fixture = native_service_preparation_fixture(false);
    let source = fixture.source();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let files = exact_test_tree_fingerprint(&fixture.state.kura.store_root());
    // Retire this genuine observation through the real publication sequence.
    // Equal State bytes cannot revive the old source's generation.
    {
        let mut publication = fixture.state.state_view_publication();
        drop(publication.begin());
    }
    let generation = fixture.state.state_view_generation();
    let provider = fixture.reserve_provider().unwrap();
    let reputation = fixture.reserve_reputation().unwrap();
    assert!(fixture.service.prepare_native_source(&fixture.proposal, source, fixture.context.clone(), native_service_test_shells(&fixture.budget)).unwrap().is_none());
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    fixture.assert_archives_reserved();
    drop((provider, reputation));
    fixture.assert_archives_free();
    assert_eq!(fixture.state.state_view_generation(), generation);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&fixture.state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_service_preparation_foreign_source_and_body_are_rejected
    use crate::sumeragi::v2_apply::V2ApplyError;
    let fixture = native_service_preparation_fixture(false);
    let foreign = native_service_preparation_fixture(false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let foreign_before = crate::snapshot::canonical_state_snapshot_hash(&foreign.state).unwrap();
    let error = fixture.service.prepare_native_source(&foreign.proposal, foreign.source(), foreign.context.clone(), native_service_test_shells(&fixture.budget))
        .err().expect("another original State cannot supply sources");
    assert!(matches!(error, V2ApplyError::TaskMismatch));
    let mut changed = fixture.proposal.clone();
    let key = native_preparation_global_keys(fixture.context.context()).remove(0);
    let signature = iroha_crypto::SignatureOf::try_from_hash(key.private_key(), changed.header().hash()).unwrap();
    changed.add_signature(iroha_data_model::block::BlockSignature::new(100, signature)).unwrap();
    assert_eq!(changed.hash(), fixture.proposal.hash());
    let error = fixture.service.prepare_native_source(&changed, fixture.source(), fixture.context.clone(), native_service_test_shells(&fixture.budget))
        .err().expect("same header with different wire cannot replace original input");
    assert!(matches!(error, V2ApplyError::TaskMismatch));
    fixture.assert_archives_free();
    foreign.assert_archives_free();
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    assert_eq!(foreign.service.candidate_executions_for_test(), 0);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&foreign.state).unwrap(), foreign_before);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_service_preparation_rejects_result_bearing_body_before_execution
    use crate::sumeragi::v2_apply::V2ApplyError;
    let fixture = native_service_preparation_fixture(false);
    let body = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    assert!(!body.is_resultless_proposal());
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<()>(&budget).unwrap();
    let error = fixture.service.prepare_native_source(
        &body,
        fixture.source(),
        fixture.context.clone(),
        shells,
    ).err().expect("result-bearing body cannot enter Native service execution");
    assert!(matches!(error, V2ApplyError::ResultBearingProposal));
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { consensus_stack native_service_physical_readiness_refusal_retries_the_same_writer_and_shells
    use crate::sumeragi::{v2_apply::V2ApplyError, v2_body_store::{BodyValidationBusy, LocalValidationRefusal}};
    let fixture = native_service_capture_fixture();
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&budget)
        .expect("finite original shell capacity before execution");
    let shell_bytes = budget.reserved_bytes();
    let source = fixture.source();
    let source_allocation = source.groups_for_test().as_ptr();
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, source, fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    let retained_bytes = budget.reserved_bytes();
    assert!(retained_bytes > shell_bytes);
    let release = concread::release::ReleaseNotification::default();
    let held = release.guard(());
    let refusal = prepared.refuse_readiness_for_test(V2ApplyError::LocalValidation(
        LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(
            "original Native readiness dependency",
            release.observe(),
            std::task::Waker::noop().clone(),
        )),
    ));
    assert!(matches!(&refusal.error, V2ApplyError::LocalValidation(LocalValidationRefusal::PhysicalBusy(_))));
    assert_eq!(budget.reserved_bytes(), retained_bytes);
    fixture.assert_archives_reserved();
    drop(held);
    let (carrier, provider, reputation, shells) = match refusal.retry() {
        Ok(parts) => parts,
        Err(refusal) => panic!("released local readiness must use the original candidate: {:?}", refusal.error),
    };
    assert_eq!(
        carrier.native_source_for_test().unwrap().sources_for_test().as_ptr(),
        source_allocation,
    );
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(carrier);
    drop((provider, reputation));
    drop(shells);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { consensus_stack native_service_fixed_readiness_refusal_keeps_original_until_recovery
    use crate::sumeragi::v2_apply::V2ApplyError;
    let fixture = native_service_capture_fixture();
    let budget = fixture.budget.clone();
    let shells = super::PreparedCarrier::reserve_journal_shells::<mv::allocation::AllocationReservation>(&budget)
        .expect("finite original shell capacity before execution");
    let shell_bytes = budget.reserved_bytes();
    let prepared = fixture.service
        .prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone(), shells)
        .unwrap()
        .expect("same original Native source");
    let retained_bytes = budget.reserved_bytes();
    assert!(retained_bytes > shell_bytes);
    let refusal = prepared.refuse_readiness_for_test(V2ApplyError::LocalEvidenceCapacity {
        required_bytes: 2,
        configured_bytes: 1,
    });
    assert_eq!(budget.reserved_bytes(), retained_bytes);
    fixture.assert_archives_reserved();
    let refusal = refusal.retry().err().expect("fixed Kura capacity has no release dependency");
    assert!(matches!(&refusal.error, V2ApplyError::LocalEvidenceCapacity { .. }));
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(refusal);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_service_preparation_recorder_conflict_releases_archives
    use crate::sumeragi::{v2_apply::V2ApplyError, v2_body_store::{BodyValidationError, LocalValidationRefusal}};
    let fixture = native_service_preparation_fixture(false);
    let source = fixture.source();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let files = exact_test_tree_fingerprint(&fixture.state.kura.store_root());
    let recorder = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
    // The existing witness owner must refuse before either source State reads
    // or archive probing, even when a real archive reader would also refuse.
    let error = fixture.provider.with_index_reader_for_test(|| fixture.service.prepare_native_source(
        &fixture.proposal, source, fixture.context.clone(), native_service_test_shells(&fixture.budget),
    )).err().expect("an existing recorder cannot enter State preparation");
    assert!(error.rejection_identity().is_none());
    assert!(matches!(error, V2ApplyError::LocalValidation(LocalValidationRefusal::RecoveryRequired(_))), "{error:?}");
    drop(recorder);
    fixture.assert_archives_free();
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&fixture.state.kura.store_root()), files);
    drop(fixture.state.block(fixture.proposal.header()));
    assert_native_economic_relay_recorder_released();
}
