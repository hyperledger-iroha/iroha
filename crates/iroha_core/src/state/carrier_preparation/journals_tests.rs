//! Actual candidate journal ownership, drop and resource-admission controls.

use super::*;
use crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveBoundsV1;
use mv::storage::StorageReadOnly;

fn admit_runtime_for_test(
    inputs: RuntimeJournalInputs<'_, '_>,
) -> Result<(), std::convert::Infallible> {
    let mode = inputs.canonical_runtime().mode();
    assert_eq!(inputs.commit_topology().mode(), mode);
    assert_eq!(inputs.prev_commit_topology().mode(), mode);
    assert_eq!(inputs.lane_consensus_contexts().mode(), mode);
    Ok(())
}

fn admit_journals_for_test(
    inputs: CarrierJournalInputs<'_, '_>,
) -> Result<(), std::convert::Infallible> {
    let state = inputs.state;
    assert_eq!(inputs.prefix.sources().proposal(), state._curr_block.hash());
    inputs
        .prefix
        .inventory()
        .verify_ordinary_witness_bundles(&inputs.prefix.witness().fastpq_transcripts)
        .unwrap();
    let mode = state.canonical_runtime.mode();
    assert_eq!(state.commit_topology.mode(), mode);
    assert_eq!(state.prev_commit_topology.mode(), mode);
    assert_eq!(state.lane_consensus_contexts.mode(), mode);
    Ok(())
}

#[test]
fn journal_admission_refusal_drops_the_complete_original_carrier() {
    #[derive(Debug, PartialEq, Eq)]
    enum Capacity {
        Exhausted,
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let bounds = ProviderIngestFinalizedArchiveBoundsV1::try_new(1, 1, 1, 1, 1, 1, 1).unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(directory_path.join("archive"), bounds).unwrap(),
    );
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let mut called = false;
    // This archive cannot admit even an empty projection. The whole-journal
    // refusal must win before that projection or any retained-value capture.
    let result = prepared.prepare_journals(Some(&archive), None, |original| {
        assert!(!called);
        called = true;
        assert_eq!(
            original.state.canonical_runtime.mode(),
            mv::BlockMode::Ordinary
        );
        assert_eq!(
            original.state.world.musubi_resolver_index_checkpoints.len(),
            1
        );
        assert_eq!(
            original.state.commit_topology.get(),
            &context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
        );
        Err::<(), _>(Capacity::Exhausted)
    });
    assert!(matches!(
        result,
        Err(CarrierJournalPreparationError::JournalAdmission(
            Capacity::Exhausted
        ))
    ));
    assert!(called);
    assert_eq!(
        std::fs::read_dir(directory_path.join("archive/records"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    // Reacquire every State journal through the real candidate constructor.
    let retry = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare after journal refusal: {error}"));
    drop(
        retry
            .prepare_journals(None, None, admit_journals_for_test)
            .unwrap(),
    );
}

#[test]
fn admitted_runtime_owner_retains_guard_and_survives_static_worker_handoff() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reservation(Arc<AtomicUsize>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn assert_send<T: Send + 'static>() {}
    assert_send::<RuntimeJournals<Reservation>>();
    let (state, _, _, _) = super::super::tests::fixture();
    let released = Arc::new(AtomicUsize::new(0));
    let original = RuntimeJournals::capture(
        state.canonical_runtime.block(),
        state.commit_topology.block(),
        state.prev_commit_topology.block(),
        state.lane_consensus_contexts.block(),
        |inputs| {
            admit_runtime_for_test(inputs)?;
            Ok::<_, std::convert::Infallible>(Reservation(Arc::clone(&released)))
        },
    )
    .unwrap();
    assert!(original.matches_current(&state));
    assert!(Arc::ptr_eq(&original.admission().0, &released));
    drop(state.canonical_runtime.block());
    drop(state.commit_topology.block());
    drop(state.prev_commit_topology.block());
    drop(state.lane_consensus_contexts.block());
    drop(state);
    let returned = std::thread::spawn(move || {
        assert_eq!(original.canonical_runtime.mode(), mv::BlockMode::Ordinary);
        assert!(original.canonical_runtime.touched_value().is_none());
        original
    })
    .join()
    .unwrap();
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(returned);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn detached_runtime_detects_replacement_and_each_owner_change() {
    let (state, _, _, _) = super::super::tests::fixture();
    let capture = || {
        RuntimeJournals::capture(
            state.canonical_runtime.block_and_revert(),
            state.commit_topology.block_and_revert(),
            state.prev_commit_topology.block_and_revert(),
            state.lane_consensus_contexts.block_and_revert(),
            admit_runtime_for_test,
        )
        .unwrap()
    };
    for owner in 0..4 {
        let original = capture();
        assert!(original.matches_current(&state));
        assert_eq!(original.canonical_runtime.mode(), mv::BlockMode::Replace);
        // A no-op commit changes the exact current/undo pair even when its
        // observable value remains equal; every component must be checked.
        match owner {
            0 => state.canonical_runtime.block().commit(),
            1 => state.commit_topology.block().commit(),
            2 => state.prev_commit_topology.block().commit(),
            3 => state.lane_consensus_contexts.block().commit(),
            _ => unreachable!(),
        }
        assert!(!original.matches_current(&state));
    }
}

#[test]
fn prepared_journals_retain_the_original_cut_and_drop_without_publication() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Reservation(Arc<AtomicUsize>);
    impl Drop for Reservation {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let checkpoint = crate::snapshot::canonical_staged_state_snapshot_hash(prepared.state());
    let runtime_delta = prepared
        .state
        .canonical_runtime
        .touched_value()
        .map(|values| (values.before.clone(), values.after.clone()));
    let original_events = prepared.state.world.external_event_buf.clone();
    let released = Arc::new(AtomicUsize::new(0));
    let mut admissions = 0;
    let journals = prepared
        .prepare_journals(None, None, |original| {
            admissions += 1;
            let original_state = original.state;
            admit_journals_for_test(original)?;
            assert_eq!(original_state.world.external_event_buf, original_events);
            assert_eq!(
                original_state.world.musubi_resolver_index_checkpoints.len(),
                1
            );
            Ok::<_, std::convert::Infallible>(Reservation(Arc::clone(&released)))
        })
        .unwrap();
    assert_eq!(admissions, 1);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    assert!(Arc::ptr_eq(&journals.admission.0, &released));
    assert_eq!(journals.execution_prefix_commitment(), prefix);
    assert_eq!(journals.checkpoint, checkpoint);
    assert_eq!(journals.valid.as_ref().hash(), proposal.hash());
    assert_eq!(*journals.context, context);
    assert_eq!(
        journals
            .components
            .runtime
            .canonical_runtime
            .touched_value()
            .map(|values| (values.before.clone(), values.after.clone())),
        runtime_delta
    );
    assert!(journals.components.runtime.matches_current(&state));
    // All original World and runtime writers are free while their captured
    // values and archive plans remain owned by this candidate.
    drop(state.canonical_runtime.block());
    drop(state.commit_topology.block());
    drop(state.prev_commit_topology.block());
    drop(state.lane_consensus_contexts.block());
    assert_eq!(
        journals.components.block_hashes.as_slice().last(),
        Some(&proposal.hash())
    );
    assert!(
        journals
            .components
            .block_hashes
            .matches_current(&state.block_hashes)
    );
    assert!(state.block_hashes.inner.try_write().is_some());
    assert_eq!(
        journals.components.block_hashes.mode(),
        mv::BlockMode::Ordinary
    );
    assert_eq!(
        journals.components.block_hashes.pending(),
        &[proposal.hash()]
    );
    assert_eq!(
        journals
            .components
            .world
            .field("musubi_resolver_index_checkpoints")
            .unwrap()
            .touched_values,
        1
    );
    assert_eq!(journals.components.world.field_count(), 278);
    assert_eq!(journals.components.world.mode(), mv::BlockMode::Ordinary);
    assert!(journals.components.world.matches_current(&state.world));
    assert_eq!(
        journals.components.world.external_events(),
        original_events.as_slice()
    );
    drop(state.world.block());
    assert_eq!(
        journals.components.transactions.staged_membership().0.get(),
        1
    );
    assert_eq!(
        journals
            .components
            .transactions
            .observe_predecessor(&state.transactions),
        storage_transactions::MembershipPredecessorStatus::Current
    );
    // Membership's writer is already released while the other journals live.
    drop(state.transactions.block());
    assert_eq!(journals.source_prefix.sources().proposal(), proposal.hash());
    journals
        .source_prefix
        .inventory()
        .verify_ordinary_witness_bundles(&journals.source_prefix.witness().fastpq_transcripts)
        .unwrap();
    assert!(!journals.publication_events.is_empty());
    assert_eq!(state.transactions.latest_height(), 0);
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
    drop(journals);
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    // Every original writer must be released by drop, without replacement scopes.
    let retry = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare after dropped journals: {error}"));
    assert_eq!(retry.execution_prefix_commitment(), prefix);
    drop(retry);
}

#[test]
fn complete_carrier_journals_move_to_a_worker_after_the_original_state_is_dropped() {
    fn assert_static_send<T: Send + 'static>() {}
    assert_static_send::<PreparedCarrierJournals<()>>();
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let state: Arc<State> = Arc::from(state);
    let original = Arc::downgrade(&state);
    let kura = Arc::clone(&state.kura);
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    let prefix = prepared.execution_prefix_commitment();
    let journals = prepared
        .prepare_journals(None, None, admit_journals_for_test)
        .unwrap();
    drop(state);
    assert!(
        original.upgrade().is_none(),
        "the candidate owns no hidden State clone"
    );
    let returned = std::thread::spawn(move || {
        assert_eq!(journals.execution_prefix_commitment(), prefix);
        assert_eq!(journals.valid.as_ref().hash(), proposal.hash());
        assert_eq!(journals.components.world.field_count(), 278);
        assert!(!journals.publication_events.is_empty());
        journals
    })
    .join()
    .unwrap();
    assert!(Arc::ptr_eq(&returned.kura, &kura));
    assert_eq!(kura.blocks_count(), 0);
    drop(returned);
    assert_eq!(kura.blocks_count(), 0);
}

#[test]
fn archive_capacity_failure_drops_candidate_journals_without_artifact_writes() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Reservation<'a> {
        state: &'a State,
        released: Arc<AtomicUsize>,
        originals_released_first: Arc<AtomicBool>,
    }
    impl Drop for Reservation<'_> {
        fn drop(&mut self) {
            self.originals_released_first.store(
                self.state.block_hashes.inner.try_write().is_some(),
                Ordering::SeqCst,
            );
            self.released.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    // One byte cannot encode even the empty provider projection. The configured
    // archive accepts the bound, but capture must refuse before any durable write.
    let bounds = ProviderIngestFinalizedArchiveBoundsV1::try_new(1, 1, 1, 1, 1, 1, 1).unwrap();
    let archive = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(directory_path.join("archive"), bounds).unwrap(),
    );
    let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
    assert!(state.block_hashes.inner.try_write().is_none());
    let released = Arc::new(AtomicUsize::new(0));
    let originals_released_first = Arc::new(AtomicBool::new(false));
    let error = prepared
        .prepare_journals(Some(&archive), None, |original| {
            admit_journals_for_test(original)?;
            Ok::<_, std::convert::Infallible>(Reservation {
                state: &state,
                released: Arc::clone(&released),
                originals_released_first: Arc::clone(&originals_released_first),
            })
        })
        .err()
        .unwrap();
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(originals_released_first.load(Ordering::SeqCst));
    assert!(
        matches!(error, CarrierJournalPreparationError::Provider(
        crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1::RecordTooLarge {
            observed, maximum: 1,
        }
    ) if observed > 1),
        "{error}"
    );
    assert_eq!(
        std::fs::read_dir(directory_path.join("archive/records"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.transactions.latest_height(), 0);
    assert_eq!(state.kura.blocks_count(), 0);
    let retry = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("prepare after resource refusal: {error}"));
    retry
        .prepare_journals(None, None, admit_journals_for_test)
        .unwrap();
}

#[test]
fn prepared_archive_projections_survive_state_journal_decomposition() {
    use crate::query::reputation_finalized::ReputationFinalizedArchiveBounds;
    use iroha_data_model::{
        isi::{
            Grant, Register,
            sorafs::{
                SetSorafsOrderbookPolicy, SetSorafsReputationJournalAuthorityPolicy,
                SetSorafsReservePolicy,
            },
        },
        permission::Permission,
        sorafs::{
            orderbook::{ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1},
            reputation::{
                REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
                REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1, ReputationJournalAuthorityPolicyV1,
            },
            reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyV1, ReservePolicyV1,
            },
        },
    };
    let authority = iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone();
    let policy = ReputationJournalAuthorityPolicyV1 {
        version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        por_recorder_authority: authority.clone(),
        dispute_recorder_authority: authority.clone(),
        token_recorder_authority: authority.clone(),
        max_source_age_ms: REPUTATION_JOURNAL_MAX_SOURCE_AGE_MS_V1,
    };
    let orderbook_policy = OrderbookAdmissionPolicyV1 {
        version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        market_id: [0xA5; 32],
        matcher_authority: authority.clone(),
        settlement_authority: authority.clone(),
        paused: false,
        min_order_gib: 1,
        max_order_gib: 1024,
        price_tick_micro_xor: 10,
        max_maker_fee_bps: 100,
        max_taker_fee_bps: 200,
        max_order_lifetime_secs: 3600,
        max_receipt_age_secs: 300,
        max_clock_skew_secs: 5,
        max_receipt_bytes: 1024,
        max_receipts_per_channel: 2,
    };
    let custody = iroha_test_samples::ALICE_ID.clone();
    let asset_id = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        iroha_genesis::GENESIS_DOMAIN_ID.clone(),
        "reserve".parse().unwrap(),
    );
    let reserve_policy = ReserveAuthorityPolicyV1 {
        version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        economics: ReservePolicyV1::default(),
        asset_definition: asset_id.clone(),
        custody_account: custody.clone(),
        treasury_account: authority.clone(),
        operations_authority: authority.clone(),
        decision_authority: authority.clone(),
        grace_period_days: 7,
        default_after_days: 30,
        max_provider_debt: sorafs_manifest::deal::XorQuantity::try_from_micro(1_000_000_000)
            .unwrap(),
        max_pending_movements_per_provider: 4,
        max_open_appeals_per_provider: 2,
    };
    // Activate every governed feed required by the real reputation projection
    // through signed genesis, preserving its permissions and policy histories.
    let (state, proposal, topology, context) = super::super::tests::fixture_with_instructions(&[
        Grant::account_permission(
            Permission::new(
                "CanManageSorafsReputationJournalPolicy".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            authority.clone(),
        )
        .into(),
        SetSorafsReputationJournalAuthorityPolicy::new(policy).into(),
        Grant::account_permission(
            Permission::new(
                "CanSetSorafsPricing".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            authority.clone(),
        )
        .into(),
        SetSorafsOrderbookPolicy::new(orderbook_policy).into(),
        Register::account(iroha_data_model::account::Account::new(custody)).into(),
        Register::asset_definition(iroha_data_model::asset::AssetDefinition::numeric(
            asset_id,
            "Reserve".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
        Grant::account_permission(
            Permission::new(
                "CanSetSorafsReservePolicy".to_owned(),
                iroha_primitives::json::Json::new(()),
            ),
            authority,
        )
        .into(),
        SetSorafsReservePolicy::new(reserve_policy).into(),
    ]);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let provider = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(
            directory_path.join("provider"),
            ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
                .unwrap(),
        )
        .unwrap(),
    );
    let reputation = Arc::new(
        ReputationFinalizedArchive::try_open(
            directory_path.join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    for _ in 0..2 {
        let prepared = super::super::tests::prepare(&state, proposal.clone(), &topology, &context)
            .unwrap_or_else(|(_, error)| panic!("prepare candidate: {error}"));
        let journals = prepared
            .prepare_journals(Some(&provider), Some(&reputation), admit_journals_for_test)
            .unwrap();
        assert!(journals.provider_capture.is_some());
        assert!(journals.reputation_capture.is_some());
        assert!(provider.is_empty().unwrap());
        assert!(reputation.is_empty().unwrap());
        assert_eq!(state.committed_height(), 0);
        assert_eq!(state.kura.blocks_count(), 0);
        for relative in [
            "provider/records",
            "reputation/anchors",
            "reputation/policies",
        ] {
            assert_eq!(
                std::fs::read_dir(directory_path.join(relative))
                    .unwrap()
                    .count(),
                0
            );
        }
        // Dropping also releases both reservations, allowing the exact candidate
        // to be prepared again without inventing a receipt or replaying effects.
        drop(journals);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
    }
}

#[test]
fn journal_resource_refusal_precedes_geometry_projection() {
    #[derive(Debug, PartialEq, Eq)]
    enum Capacity {
        Exhausted,
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("actual candidate: {error}"));
    // Deliberate test-only projection drift would fail geometry capture. Whole
    // capture admission must nevertheless precede its allocating projections.
    prepared.state.nexus.autoscale.enabled = !prepared.state.nexus.autoscale.enabled;
    assert!(prepared.state.prepare_carrier_geometry().is_err());
    let mut called = false;
    let error = prepared
        .prepare_journals(None, None, |_| {
            called = true;
            Err::<(), _>(Capacity::Exhausted)
        })
        .err()
        .expect("local capture refusal");
    assert!(called);
    assert!(matches!(
        error,
        CarrierJournalPreparationError::JournalAdmission(Capacity::Exhausted)
    ));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert!(state.block_hashes.inner.try_write().is_some());
    assert_eq!(state.kura.blocks_count(), 0);
}

#[test]
fn geometry_refusal_drops_originals_before_capture_reservation() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Reservation<'a> {
        state: &'a State,
        released: Arc<AtomicUsize>,
        originals_released_first: Arc<AtomicBool>,
    }
    impl Drop for Reservation<'_> {
        fn drop(&mut self) {
            self.originals_released_first.store(
                self.state.block_hashes.inner.try_write().is_some(),
                Ordering::SeqCst,
            );
            self.released.fetch_add(1, Ordering::SeqCst);
        }
    }
    let (state, proposal, topology, context) = super::super::tests::fixture();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let mut prepared = super::super::tests::prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("actual candidate: {error}"));
    prepared.state.nexus.autoscale.enabled = !prepared.state.nexus.autoscale.enabled;
    assert!(prepared.state.prepare_carrier_geometry().is_err());
    let released = Arc::new(AtomicUsize::new(0));
    let originals_released_first = Arc::new(AtomicBool::new(false));
    let error = prepared
        .prepare_journals(None, None, |_| {
            Ok::<_, std::convert::Infallible>(Reservation {
                state: &state,
                released: Arc::clone(&released),
                originals_released_first: Arc::clone(&originals_released_first),
            })
        })
        .err()
        .expect("geometry drift refuses capture");
    assert!(matches!(error, CarrierJournalPreparationError::Geometry(_)));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert!(originals_released_first.load(Ordering::SeqCst));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    assert_eq!(state.kura.blocks_count(), 0);
}
