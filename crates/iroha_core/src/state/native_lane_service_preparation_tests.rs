// Actual Native sources through the service that owns State, Kura and archives.
// These tests stop at the synchronous prepared owner; no publication or marker
// authority is supplied by a test reservation.

struct NativeServicePreparationFixture {
    state: Arc<State>,
    service: crate::sumeragi::v2_apply::V2ApplyService,
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
    let service = crate::sumeragi::v2_apply::V2ApplyService::new(
        Arc::clone(&state),
        queue,
        Arc::clone(&state.kura),
        Some(Arc::clone(&provider)),
        Some(Arc::clone(&reputation)),
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
    let (carrier, provider, reputation) = match prepared.try_into_parts() {
        Ok(parts) => parts,
        Err((_owner, error)) => {
            panic!("configured original post-execution dependencies: {error:?}")
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
