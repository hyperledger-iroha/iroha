// Actual Native sources through the service that owns State, Kura and archives.
// These tests stop at the synchronous prepared owner; no publication or marker
// authority is supplied by a test reservation.

struct NativeServicePreparationFixture {
    state: Arc<State>,
    service: Arc<crate::sumeragi::v2_apply::V2ApplyService>,
    proposal: SignedBlock,
    context: crate::sumeragi::v2::VerifiedHeightContext,
    provider: Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
    reputation: Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
    source_asset: AssetId,
    destination_asset: AssetId,
    pulse: iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
    _directory: tempfile::TempDir,
}

#[inline(never)]
fn native_service_preparation_fixture(atomic: bool) -> Box<NativeServicePreparationFixture> {
    native_service_preparation_from_original(native_publication_fixture(atomic))
}

#[inline(never)]
fn native_service_preparation_from_original(
    original: Box<NativePublicationFixture>,
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
    let service = Arc::new(crate::sumeragi::v2_apply::V2ApplyService::new(
        Arc::clone(&state),
        queue,
        Arc::clone(&state.kura),
        Some(Arc::clone(&provider)),
        Some(Arc::clone(&reputation)),
        state.sumeragi_block_cadence(),
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events,
        pops,
    ));
    Box::new(NativeServicePreparationFixture {
        state,
        service,
        proposal,
        context,
        provider,
        reputation,
        source_asset,
        destination_asset,
        pulse,
        _directory: directory,
    })
}

impl NativeServicePreparationFixture {
    fn source(&self) -> super::PreparedNativeLaneBatchSourceV1<'_> {
        let super::NativeLaneBatchSourcePreparationV1::Ready(source) = self
            .state
            .prepare_proposed_native_lane_batch_source(&self.proposal, &[])
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
        assert!(matches!(self.reserve_reputation(), Err(
            crate::query::reputation_finalized::ReputationFinalizedArchiveError::CaptureReserved { .. }
        )));
    }
}

state_test! { sync native_service_preparation_single_preserves_original_sources_and_archives
    assert_native_service_preparation_success(native_service_preparation_fixture(false), false);
}
state_test! { sync native_service_preparation_atomic_preserves_original_sources_and_archives
    assert_native_service_preparation_success(native_service_preparation_fixture(true), true);
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
        .prepare_native_source(&fixture.proposal, source, fixture.context.clone())
        .unwrap()
        .expect("same original applying source");
    fixture.assert_archives_reserved();
    // The logical archive owners exclude mutation without retaining index locks.
    fixture.provider.with_index_reader_for_test(|| ());
    fixture.reputation.with_index_reader_for_test(|| ());
    let (carrier, provider, reputation, shell_admission) = prepared.into_parts();
    assert!(shell_admission.reserved_bytes() > 0);
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
    drop((provider, reputation, shell_admission));
    fixture.assert_archives_free();
    // Abandoning the whole synchronous owner must also release both captures,
    // after its original State writers, without publishing this second attempt.
    let abandoned = fixture
        .service
        .prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone())
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
            &fixture.proposal, source, fixture.context.clone(),
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
    let error = fixture.service.prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone())
        .err().expect("original provider capture excludes the candidate");
    assert_native_service_local_busy(error, "provider_archive_capture");
    drop(first);
    let second = fixture.reserve_reputation().unwrap();
    let error = fixture.service.prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone())
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
    assert!(fixture.service.prepare_native_source(&fixture.proposal, source, fixture.context.clone()).unwrap().is_none());
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
    let error = fixture.service.prepare_native_source(&foreign.proposal, foreign.source(), foreign.context.clone())
        .err().expect("another original State cannot supply sources");
    assert!(matches!(error, V2ApplyError::TaskMismatch));
    let mut changed = fixture.proposal.clone();
    let key = native_preparation_global_keys(fixture.context.context()).remove(0);
    let signature = iroha_crypto::SignatureOf::try_from_hash(key.private_key(), changed.header().hash()).unwrap();
    changed.add_signature(iroha_data_model::block::BlockSignature::new(100, signature)).unwrap();
    assert_eq!(changed.hash(), fixture.proposal.hash());
    let error = fixture.service.prepare_native_source(&changed, fixture.source(), fixture.context.clone())
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
        &fixture.proposal, source, fixture.context.clone(),
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

// Real governed archive feeds are installed before any admission or Native
// Decision. The existing economic genesis overlay executes the same policy
// instructions as the ordinary signed-genesis archive fixture.
#[inline(never)]
fn native_service_retention_fixture(atomic: bool) -> Box<NativeServicePreparationFixture> {
    let mut setup = native_economic_state_setup(None, |world| {
        let authority = iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone();
        let (id, account) = Account::new(authority.clone())
            .build(&authority)
            .into_key_value();
        world.accounts.insert(id, account);
        let domain_id = iroha_genesis::GENESIS_DOMAIN_ID.clone();
        let domain = Domain::new(domain_id.clone()).build(&authority);
        world.domains.insert(domain_id, domain);
    });
    setup.genesis_instructions = super::carrier_preparation::archive_fixture_instructions();
    let economic = native_economic_fixture_from_state(
        &[NativeEconomicCase::Transfer(25)],
        atomic,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
        None,
        setup,
    );
    native_service_preparation_from_original(native_publication_fixture_from_control(
        native_control_execution_fixture_from_economic(economic, atomic, true),
    ))
}

// Current first-admission carriers have useful authenticated controls but no
// Native economic batch. Their one common execution must use the same custody.
state_test! { sync native_service_control_only_admission_retains_one_execution_and_publishes
    use crate::sumeragi::{
        v2_body_store::{BlockSignaturePolicy, V2BodyStore, V2BodyStoreCapacity},
        v2_chunks::encode_payload,
    };
    let mut fixture = native_service_retention_fixture(false);
    let mut bundle = fixture.proposal.execution_context().unwrap().clone();
    assert!(!bundle.queue_plan_admissions.is_empty());
    bundle.native_lane_decisions = None;
    fixture.proposal.set_execution_context(Some(bundle));
    let context = fixture.context.context();
    let keys = native_preparation_global_keys(context);
    let leader = context.leader(fixture.proposal.header().view_change_index());
    fixture.proposal.replace_signatures(BTreeSet::from([
        iroha_data_model::block::BlockSignature::new(u64::from(leader),
            iroha_crypto::SignatureOf::from_hash(keys[leader as usize].private_key(), fixture.proposal.hash())),
    ])).unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    {
        let view = fixture.state.view();
        assert_eq!(view.world.assets.get(&fixture.source_asset).unwrap().0, Quantity::from(100u32));
        assert!(view.world.assets.get(&fixture.destination_asset).is_none(),
            "the admitted transfers have not created a destination balance");
    }
    let directory = tempfile::tempdir().unwrap();
    let mut store = V2BodyStore::open_with_policy_and_capacity(
        directory.path(), context.clone(), BlockSignaturePolicy::RotatingLeader,
        V2BodyStoreCapacity::for_test(1, 8 << 20).unwrap(),
    ).unwrap();
    let subject = BlockSubject {
        parent_block_hash: fixture.proposal.header().prev_block_hash(),
        block_hash: fixture.proposal.hash(),
        payload_hash: fixture.proposal.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound { context_id: context.id(), height: context.height,
        view: fixture.proposal.header().view_change_index() };
    let bytes = fixture.proposal.encode_wire().unwrap();
    let manifest = encode_payload(context, round, subject, &bytes).unwrap().manifest().clone();
    let durable = store.store(manifest, bytes).unwrap();
    let mut retained = fixture.service.retained_validation_service(&store, fixture.context.clone()).unwrap();
    let receipt = store.execute_retained_durable_validation(
        durable.clone(), durable.manifest_hash(), &mut retained,
    ).unwrap().into_validated_receipt().unwrap();
    let allocation = retained.owner_for_test(subject).unwrap().phase_allocation_for_test();
    let cached = store.execute_retained_durable_validation(
        durable.clone(), durable.manifest_hash(), &mut retained,
    ).unwrap().into_validated_receipt().unwrap();
    assert_eq!(cached, receipt);
    assert_eq!(retained.owner_for_test(subject).unwrap().phase_allocation_for_test(), allocation);
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    let finality = fixture.finality(&fixture.proposal, receipt.execution_commitment());
    let published = retained.select(&receipt).unwrap()
        .try_consume(|validator, owner| owner.try_publish(validator, finality))
        .unwrap_or_else(|error| panic!("current first-admission source must publish: {error}"));
    assert!(published.native_apply().is_none(), "control publication grants no economic Native Apply");
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    assert_eq!(fixture.state.committed_height() as u64, context.height);
    let view = fixture.state.view();
    assert_eq!(view.world.assets.get(&fixture.source_asset).unwrap().0, Quantity::from(100u32));
    assert!(
        view.world.assets.get(&fixture.destination_asset).is_none(),
        "without a Native transfer, no destination asset row is created"
    );
    drop(view);
    drop(published);
    drop(retained);
    assert_eq!(fixture.service.carrier_shell_budget_for_test().reserved_bytes(), 0);
}

// Real production admission and validator through BodyStore marker/cache custody.
type NativeServiceProductionPhase = crate::sumeragi::v2_apply::native_validation::NativeValidationCandidate;

fn native_service_retained_source_allocations(phase: &NativeServiceProductionPhase) -> [usize; 4] {
    let super::RetainedCarrier::Validated(journals) = phase.carrier_for_test().unwrap() else {
        panic!("ready original archive indexes must finish the retained capture");
    };
    let groups = journals.native_source_for_test().unwrap().sources_for_test();
    assert_eq!(groups.len(), 1);
    [groups.as_ptr() as usize, groups[0].body().canonical_bytes().as_ptr() as usize,
     groups[0].decisions().as_ptr() as usize, groups[0].contexts().as_ptr() as usize]
}

state_test! { sync native_service_single_body_store_retries_reuse_original_execution
    assert_native_service_body_store_retention(native_service_retention_fixture(false));
}

state_test! { sync native_service_atomic_body_store_retries_reuse_original_execution
    assert_native_service_body_store_retention(native_service_retention_fixture(true));
}

#[inline(never)]
fn assert_native_service_body_store_retention(fixture: Box<NativeServicePreparationFixture>) {
    use crate::sumeragi::{
        v2_body_store::{
            BlockSignaturePolicy, V2BodyStore, V2BodyStoreCapacity, V2BodyStoreError,
            fail_next_marker_directory_sync, fail_next_marker_file_sync,
        },
        v2_chunks::encode_payload,
    };
    use iroha_data_model::block::consensus_v2 as wire;
    use crate::sumeragi::v2_apply::validation_custody::RetainedValidationOwner;

    let fixture: Arc<NativeServicePreparationFixture> = fixture.into();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let files = exact_test_tree_fingerprint(&fixture.state.kura.store_root());
    let generation = fixture.state.state_view_generation();
    let context = fixture.context.context();
    let directory = tempfile::tempdir().unwrap();
    // Two real durable manifests and their markers; reserve the exact bounded
    // descriptor storage instead of the production 65,536-entry ceiling.
    let capacity = V2BodyStoreCapacity::for_test(2, 8 << 20).unwrap();
    let mut store = V2BodyStore::open_with_policy_and_capacity(
        directory.path(),
        context.clone(),
        BlockSignaturePolicy::RotatingLeader,
        capacity,
    )
    .unwrap();
    let subject = wire::BlockSubject {
        parent_block_hash: fixture.proposal.header().prev_block_hash(),
        block_hash: fixture.proposal.hash(),
        payload_hash: fixture.proposal.canonical_proposal_wire_hash().unwrap(),
    };
    let bytes = fixture.proposal.encode_wire().unwrap();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let manifest = encode_payload(context, round, subject, &bytes)
        .unwrap()
        .manifest()
        .clone();
    let durable = store.store(manifest, bytes.clone()).unwrap();
    let budget = fixture.service.carrier_shell_budget_for_test();
    let mut service = fixture.service.retained_validation_service(&store, fixture.context.clone()).unwrap();
    let descriptor_bytes = budget.reserved_bytes();
    assert!(descriptor_bytes > 0);
    fail_next_marker_file_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            durable.clone(),
            durable.manifest_hash(),
            &mut service,
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    let owner = service.owner_for_test(subject).unwrap();
    let allocations = native_service_retained_source_allocations(owner);
    let commitment = owner.ready_commitment().unwrap();
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    assert_eq!(service.marker_counts_for_test(), (1, 0));
    let retained_bytes = budget.reserved_bytes();
    assert!(retained_bytes > descriptor_bytes);
    fixture.assert_archives_reserved();
    // The marker retry owns journals, not State writers. This would deadlock
    // if the service retained the synchronous prepared carrier across the wait.
    drop(fixture.state.block(fixture.proposal.header()));
    for _ in 0..2 {
        let receipt = store
            .execute_retained_durable_validation(
                durable.clone(),
                durable.manifest_hash(),
                &mut service,
            )
            .unwrap()
            .into_validated_receipt()
            .unwrap();
        assert_eq!(receipt.execution_commitment(), commitment);
        assert_eq!(fixture.service.candidate_executions_for_test(), 1);
        assert_eq!(
            native_service_retained_source_allocations(service.owner_for_test(subject).unwrap()),
            allocations,
        );
    }
    let later_manifest = encode_payload(
        context,
        wire::ConsensusRound { view: 7, ..round },
        subject,
        &bytes,
    )
    .unwrap()
    .manifest()
    .clone();
    let later = store.store(later_manifest, bytes).unwrap();
    fail_next_marker_directory_sync();
    assert!(matches!(
        store.execute_retained_durable_validation(
            later.clone(),
            later.manifest_hash(),
            &mut service
        ),
        Err(V2BodyStoreError::Io { .. })
    ));
    assert_eq!(service.marker_counts_for_test(), (1, 1));
    let receipt = store
        .execute_retained_durable_validation(later.clone(), later.manifest_hash(), &mut service)
        .unwrap()
        .into_validated_receipt()
        .unwrap();
    assert_eq!(receipt.execution_commitment(), commitment);
    assert_eq!(service.marker_counts_for_test(), (0, 2));
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    assert_eq!(
        native_service_retained_source_allocations(service.owner_for_test(subject).unwrap()),
        allocations,
    );
    drop(service);
    drop(store);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    // A process restart reconstructs actual execution once, then every recovered
    // round and the live cache keep that same replay-created candidate owner.
    let mut reopened = V2BodyStore::open_with_policy_and_capacity(
        directory.path(), context.clone(), BlockSignaturePolicy::RotatingLeader, capacity,
    ).unwrap();
    let mut replay = fixture.service.retained_validation_service(&reopened, fixture.context.clone()).unwrap();
    reopened.revalidate_retained_markers(&mut replay).unwrap();
    let replay_allocations = native_service_retained_source_allocations(replay.owner_for_test(subject).unwrap());
    assert_eq!(fixture.service.candidate_executions_for_test(), 2);
    assert_eq!(replay.marker_counts_for_test(), (0, 2));
    let cached = reopened.execute_retained_durable_validation(later.clone(), later.manifest_hash(), &mut replay)
        .unwrap().into_validated_receipt().unwrap();
    assert_eq!(cached.execution_commitment(), commitment);
    assert_eq!(native_service_retained_source_allocations(replay.owner_for_test(subject).unwrap()), replay_allocations);
    assert_eq!(fixture.service.candidate_executions_for_test(), 2);
    drop(replay);
    assert_eq!(budget.reserved_bytes(), 0);
    fixture.assert_archives_free();
    assert_eq!(fixture.state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(),
        before
    );
    assert_eq!(
        exact_test_tree_fingerprint(&fixture.state.kura.store_root()),
        files
    );
}

state_test! { sync native_service_production_shell_pool_refuses_before_execution_and_retries
    use crate::sumeragi::v2_body_store::{BodyValidationError, LocalValidationRefusal};
    let fixture = native_service_retention_fixture(false);
    let budget = fixture.service.carrier_shell_budget_for_test();
    let held = budget.try_reserve_bytes(budget.limit_bytes()).unwrap();
    let error = fixture.service.prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone())
        .err().expect("full original pool refuses before execution");
    assert_eq!(fixture.service.candidate_executions_for_test(), 0);
    assert!(error.rejection_identity().is_none());
    let Some(LocalValidationRefusal::PhysicalBusy(busy)) = error.local_refusal() else {
        panic!("temporary capacity keeps original pool release");
    };
    let mut wait = busy.wait.clone().wait_for_release();
    assert!(std::future::Future::poll(std::pin::Pin::new(&mut wait),
        &mut std::task::Context::from_waker(busy.waker())).is_pending());
    let unrelated = mv::allocation::AllocationBudget::new(budget.limit_bytes());
    drop(unrelated.try_reserve_bytes(1).unwrap());
    assert!(std::future::Future::poll(std::pin::Pin::new(&mut wait),
        &mut std::task::Context::from_waker(busy.waker())).is_pending());
    drop(held);
    assert!(std::future::Future::poll(std::pin::Pin::new(&mut wait),
        &mut std::task::Context::from_waker(busy.waker())).is_ready());
    let prepared = fixture.service.prepare_native_source(&fixture.proposal, fixture.source(), fixture.context.clone())
        .unwrap().expect("original source resumes after original pool release");
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), 0);
}

impl NativeServicePreparationFixture {
    /// Authenticate the actual retained output commitment with this committee.
    fn finality(
        &self,
        block: &SignedBlock,
        execution_commitment: ExecutionCommitment,
    ) -> crate::block::VerifiedV2FinalityArtifact {
        let context = self.context.context().clone();
        let keys = native_preparation_global_keys(&context);
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block.canonical_proposal_wire_hash().unwrap(),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: block.header().view_change_index(),
        };
        let vote = iroha_data_model::block::consensus_v2::Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &vote.signature_preimage())
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let artifact = V2FinalityArtifact::new(
            context,
            subject,
            QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                    &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                )
                .unwrap(),
            },
            keys.iter()
                .map(|key| bls_normal_pop_prove(key.private_key()).unwrap())
                .collect(),
        );
        crate::block::VerifiedV2FinalityArtifact::verify(artifact)
            .expect("real three-of-four global finality joins the actual Native execution")
    }
}


state_test! { sync native_service_postpublication_refusal_retains_original_owner_and_notifies_once
    use crate::sumeragi::{
        v2_apply::native_validation::fail_next_post_publication_queue_tail_for_test,
        v2_body_store::{BlockSignaturePolicy, LocalValidationRefusal, V2BodyStore, V2BodyStoreCapacity},
        v2_chunks::encode_payload,
    };
    let fixture = native_service_retention_fixture(false);
    let directory = tempfile::tempdir().unwrap();
    let context = fixture.context.context();
    let mut store = V2BodyStore::open_with_policy_and_capacity(
        directory.path(), context.clone(), BlockSignaturePolicy::RotatingLeader,
        V2BodyStoreCapacity::for_test(1, 8 << 20).unwrap(),
    ).unwrap();
    let subject = BlockSubject {
        parent_block_hash: fixture.proposal.header().prev_block_hash(),
        block_hash: fixture.proposal.hash(),
        payload_hash: fixture.proposal.canonical_proposal_wire_hash().unwrap(),
    };
    let round = ConsensusRound {
        context_id: context.id(), height: context.height,
        view: fixture.proposal.header().view_change_index(),
    };
    let bytes = fixture.proposal.encode_wire().unwrap();
    let manifest = encode_payload(context, round, subject, &bytes).unwrap().manifest().clone();
    let durable = store.store(manifest, bytes).unwrap();
    let mut retained = fixture.service.retained_validation_service(&store, fixture.context.clone()).unwrap();
    let receipt = store.execute_retained_durable_validation(
        durable.clone(), durable.manifest_hash(), &mut retained,
    ).unwrap().into_validated_receipt().unwrap();
    let allocation = retained.owner_for_test(subject).unwrap().phase_allocation_for_test();
    let finality = fixture.finality(&fixture.proposal, receipt.execution_commitment());
    let mut events = fixture.service.events_for_test();
    let before_height = fixture.state.committed_height();
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    fail_next_post_publication_queue_tail_for_test();
    let refusal = retained.select(&receipt).unwrap()
        .try_consume(|validator, owner| owner.try_publish(validator, finality.clone()))
        .err().expect("real State publication must retain its failed completion tail");
    assert!(matches!(refusal, LocalValidationRefusal::RecoveryRequired(_)));
    assert_eq!(fixture.state.committed_height(), before_height + 1);
    assert_eq!(fixture.state.committed_height() as u64, context.height);
    assert_eq!(retained.owner_for_test(subject).unwrap().published_progress_for_test(), Some((allocation, true, false)));
    assert!(matches!(events.try_recv(), Err(tokio::sync::broadcast::error::TryRecvError::Empty)));
    let published_hash = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let published_generation = fixture.state.state_view_generation();
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    let published = retained.select(&receipt).unwrap()
        .try_consume(|validator, owner| owner.try_publish(validator, finality.clone()))
        .unwrap_or_else(|error| panic!("same actual published owner must finish its remaining tail: {error}"));
    assert!(published.matches_state(&fixture.state));
    assert_eq!(published.artifact(), finality.artifact());
    assert_eq!(fixture.state.state_view_generation(), published_generation);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), published_hash);
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    assert!(retained.owner_for_test(subject).is_none());
    let mut notifications = Vec::new();
    while let Ok(event) = events.try_recv() { notifications.push(event); }
    assert_eq!(notifications.len(), published.events().len() + 1);
    assert_eq!(notifications[0], iroha_data_model::events::EventBox::Pipeline(
        iroha_data_model::events::pipeline::PipelineEventBox::Block(published.committed_event().clone()),
    ));
    assert_eq!(&notifications[1..], published.events());
    // The subject tombstone prevents even a later view from reexecuting this application.
    assert!(retained.select(&receipt).is_err());
    assert_eq!(fixture.service.candidate_executions_for_test(), 1);
    drop(published);
    drop(retained);
    assert_eq!(fixture.service.carrier_shell_budget_for_test().reserved_bytes(), 0);
}


impl State {
    /// Genuine immutable first-admission authority for lifecycle wait-custody tests.
    /// The returned token survives its fixture State, as historical source tokens do.
    pub(crate) fn authenticated_native_source_for_lifecycle_test(
    ) -> Arc<super::AuthenticatedLaneAdmittedInputSourceV1> {
        let fixture = native_publication_fixture(false);
        let state = fixture.state();
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        let lane = observed.contexts().first().expect("fixture opens its actual Native lane");
        match state.first_lane_admitted_input(&observed, lane).unwrap() {
            super::FirstLaneAdmittedInputReadV1::Ready(input) => Arc::new(input.source().clone()),
            super::FirstLaneAdmittedInputReadV1::CanonicalBodyRecoveryRequired(source) => Arc::new(source),
            _ => panic!("genuine fixture source must retain its exact current observation"),
        }
    }
}
