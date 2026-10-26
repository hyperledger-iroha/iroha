//! Native owner, funded MV custody, deterministic history and restore assertions.
use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
}

#[test]
fn authenticated_root_scope_requires_native_execution_and_survives_publication() {
    use iroha_data_model::block::consensus::SumeragiRootScope;
    use iroha_model_base::topology::DataSpaceId;

    let parent_network = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA7; 32])),
    );
    for scope in [
        SumeragiRootScope::Global,
        SumeragiRootScope::Dataspace {
            parent_network_id: parent_network,
            dataspace_id: DataSpaceId::new(u64::MAX - 1),
        },
    ] {
        let mut config = TestChainConfig::new(World::new(), 1_000);
        config.root_scope = scope;
        if let SumeragiRootScope::Dataspace { dataspace_id, .. } = scope {
            use iroha_data_model::nexus::{
                DataSpaceCatalog, DataSpaceMetadata, LaneCatalog, LaneConfig,
            };
            let mut nexus = iroha_config::parameters::actual::Nexus::default();
            nexus.lane_catalog = LaneCatalog::new(
                1_u32.try_into().unwrap(),
                vec![LaneConfig {
                    dataspace_id,
                    alias: "native-root-scope".into(),
                    ..LaneConfig::default()
                }],
            )
            .unwrap();
            nexus.configured_lane_catalog = nexus.lane_catalog.clone();
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
            nexus.dataspace_catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
                id: dataspace_id,
                alias: "native-root-scope".into(),
                description: None,
                fault_tolerance: 1,
            }])
            .unwrap();
            nexus.configured_dataspace_catalog = nexus.dataspace_catalog.clone();
            nexus.routing_policy.default_dataspace = dataspace_id;
            config.nexus = Some(nexus);
            let fee_domain =
                iroha_model_base::domain::DomainId::try_new("fees", "native-root-scope").unwrap();
            config.genesis_instructions.extend([
                iroha_data_model::isi::Register::domain(iroha_data_model::domain::Domain::new(
                    fee_domain.clone(),
                ))
                .into(),
                iroha_data_model::isi::Register::asset_definition(
                    iroha_data_model::asset::AssetDefinition::numeric(
                        iroha_config::parameters::defaults::nexus::fees::genesis_policy()
                            .fee_asset_id,
                        "Native root fixture fee",
                        iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
                        Some(fee_domain),
                    ),
                )
                .into(),
            ]);
            config
                .genesis_parameters
                .push(iroha_data_model::parameter::Parameter::Custom(
                    iroha_data_model::block::consensus::RootFeePolicy {
                        base_fee: 1_u32.into(),
                        per_gas_unit_fee: 1_u32.into(),
                        ..iroha_config::parameters::defaults::nexus::fees::genesis_policy()
                    }
                    .into_custom_parameter()
                    .unwrap(),
                ));
        }
        let prepared = CertifiedTestChain::prepare(config).unwrap();
        assert_eq!(prepared.state.authenticated_root_scope(), None);
        let genesis_account = AccountId::new(prepared.genesis.public_key().clone());
        crate::sumeragi::startup::apply_genesis(
            &prepared.state,
            prepared.genesis.block().clone(),
            &genesis_account,
            prepared.genesis.consensus_metadata().mode.into(),
            None,
        )
        .unwrap();
        let state = prepared.state;
        assert_eq!(state.authenticated_root_scope(), Some(scope));
        let (result, reader) = state.with_held_view_publication_for_reader_test(|_| {
            assert!(matches!(
                state.try_view_once(),
                Err(StateViewError::Busy(_))
            ));
            let (send, receive) = std::sync::mpsc::channel();
            let retained = Arc::clone(&state);
            let reader = std::thread::spawn(move || {
                send.send(retained.authenticated_root_scope()).unwrap();
            });
            (
                receive.recv_timeout(std::time::Duration::from_secs(1)),
                reader,
            )
        });
        reader.join().unwrap();
        assert_eq!(result.unwrap(), Some(scope));
        state.with_held_header_for_reader_test(|_| {
            assert!(matches!(
                state.try_view_once(),
                Err(StateViewError::Busy(_))
            ));
            assert_eq!(state.authenticated_root_scope(), Some(scope));
        });
    }
}

#[test]
fn original_genesis_and_worker_publish_exact_tip_with_undo() {
    let mut chain = chain();
    let root_scope = chain.state().authenticated_root_scope();
    let old_view = chain.state().view();
    let genesis = old_view.native_execution_tip().unwrap();
    let genesis_receipt = crate::sumeragi::certified_chain::committed_block(&old_view, 1).unwrap();
    assert_eq!(genesis.core_hash(), genesis_receipt.core_hash());
    assert_eq!(genesis.result(), genesis_receipt.result());
    assert_eq!(*old_view.native_execution_tip_predecessor.get(), Some(None));
    let state = Arc::clone(chain.state());
    // Release the chain borrow while retaining the actual old EBR cut through State.
    drop(old_view);
    let old_view = state.view();
    chain.commit(Vec::new());
    assert_eq!(state.authenticated_root_scope(), root_scope);
    let view = state.view();
    let current = view.native_execution_tip().unwrap();
    let receipt = chain.committed(2);
    assert_eq!(current.height(), 2);
    assert_eq!(current.iroha_hash(), receipt.block_hash());
    assert_eq!(current.core_hash(), receipt.core_hash());
    assert_eq!(current.result(), receipt.result());
    assert_eq!(
        *view.native_execution_tip_predecessor.get(),
        Some(Some(genesis))
    );
    assert_eq!(old_view.native_execution_tip(), Some(genesis));
    assert_eq!(old_view.height(), 1);
}

#[test]
fn rejected_original_preparation_and_discard_preserve_committed_tip() {
    let mut chain = chain();
    let state = Arc::clone(chain.state());
    let before = state.view().native_execution_tip();
    let proposal = chain.proposal(None, Vec::new());
    let mut original = chain
        .begin_proposal(proposal, iroha_sumeragi::types::ControlWitness::default())
        .unwrap();
    assert_eq!(
        original
            .inspect(|owner| owner.state.native_execution_tip())
            .unwrap(),
        before
    );
    assert!(original.prepare(Signers::BelowQuorum).is_err());
    assert_eq!(state.view().native_execution_tip(), before);
    drop(original);
    assert_eq!(state.view().native_execution_tip(), before);
    chain.commit(Vec::new());
    assert_eq!(state.view().native_execution_tip().unwrap().height(), 2);
}

#[test]
fn replacement_reads_exact_tip_undo_and_abandonment_restores_original_cut() {
    let mut chain = chain();
    let previous = chain.state().view().native_execution_tip();
    chain.commit(Vec::new());
    let current = chain.state().view().native_execution_tip();
    let header = chain.committed(2).block().header();
    let mut replacement = chain.state().block_and_revert(header);
    assert_eq!(replacement.native_execution_tip(), previous);
    assert_eq!(replacement.transaction().native_execution_tip(), previous);
    drop(replacement);
    assert_eq!(chain.state().view().native_execution_tip(), current);
}

#[test]
fn execution_history_admits_each_actual_source_before_read() {
    let mut chain = chain();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    let mut admitted = Vec::new();
    let genesis = view
        .canonical_history()
        .executed_block(NonZeroUsize::MIN, |count, bytes| {
            admitted.push((count, bytes));
            Ok(())
        })
        .unwrap();
    assert_eq!(genesis.hash(), chain.genesis().hash());
    assert_eq!(admitted.len(), 3);
    assert!(
        admitted
            .iter()
            .all(|&(count, bytes)| count == 1 && bytes > 0)
    );
    let mut attempts = 0;
    assert!(
        view.canonical_history()
            .executed_block(NonZeroUsize::MIN, |_, _| {
                attempts += 1;
                if attempts == 2 {
                    Err(crate::execution_attempt::ExecutionAttemptError::Deferred(
                        ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into(),
                    ))
                } else {
                    Ok(())
                }
            })
            .is_err()
    );
    assert_eq!(attempts, 2);
    chain
        .kura()
        .corrupt_native_frame_for_test(NonZeroUsize::new(2).unwrap());
    assert!(
        view.canonical_history()
            .executed_block(NonZeroUsize::MIN, |_, _| Ok(()))
            .is_err()
    );
}

fn snapshot_claim(state: &State) -> NativeExecutionTipSnapshot {
    let view = state.view();
    let claim = NativeExecutionTipSnapshot::from_original(
        *view.native_execution_tip.get(),
        *view.native_execution_tip_predecessor.get(),
    );
    norito::json::from_str(&norito::json::to_json(&claim).unwrap()).unwrap()
}

#[test]
fn restore_reauthenticates_native_current_and_undo_and_rejects_claim_substitution() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    // Every substitution starts from the claim captured at the snapshot tip.
    let captured = norito::json::to_json(&snapshot_claim(&state)).unwrap();
    let claim = || norito::json::from_str::<NativeExecutionTipSnapshot>(&captured).unwrap();
    // The certified successor binds the World published at the snapshot tip.
    chain.commit(Vec::new());
    let restore = |claim: NativeExecutionTipSnapshot| {
        claim.restore(
            &chain.state().ivm_execution_budget(),
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        )
    };
    let restored = restore(claim()).unwrap();
    assert_eq!(*restored.cell.view().get(), view.native_execution_tip());
    assert_eq!(
        *restored.cell.predecessor_view().get(),
        *view.native_execution_tip_predecessor.get()
    );
    assert_eq!(
        restored.world_root,
        Some(
            chain
                .committed(3)
                .commitment()
                .execution
                .parent_world_state_root
        )
    );
    assert_eq!(
        restored.world_root,
        Some(view.world.state_accumulator.get().root().unwrap())
    );
    let mut wrong_result = claim();
    wrong_result.blocks.as_mut().unwrap().result[0] ^= 1;
    assert!(restore(wrong_result).is_err());
    let mut wrong_scope = claim();
    wrong_scope.blocks.as_mut().unwrap().root_scope =
        iroha_data_model::block::consensus::SumeragiRootScope::Dataspace {
            parent_network_id: *view.network_id(),
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(19),
        };
    assert!(
        matches!(restore(wrong_scope), Err(TipRestoreError::History(_))),
        "a decoded scope cannot replace the authenticated genesis scope"
    );
    let mut wrong_undo_scope = claim();
    wrong_undo_scope
        .revert
        .as_mut()
        .unwrap()
        .value
        .as_mut()
        .unwrap()
        .root_scope = iroha_data_model::block::consensus::SumeragiRootScope::Dataspace {
        parent_network_id: *view.network_id(),
        dataspace_id: iroha_model_base::topology::DataSpaceId::new(19),
    };
    assert!(
        matches!(restore(wrong_undo_scope), Err(TipRestoreError::History(_))),
        "the predecessor scope is checked against the same authenticated prefix"
    );
    let mut wrong_undo = claim();
    wrong_undo
        .revert
        .as_mut()
        .unwrap()
        .value
        .as_mut()
        .unwrap()
        .core_hash[0] ^= 1;
    assert!(restore(wrong_undo).is_err());
    assert!(
        claim()
            .restore(
                &chain.state().ivm_execution_budget(),
                &"another-native-instance".parse().unwrap(),
                view.network_id(),
                &hashes,
                chain.kura(),
                &mut |_| Ok(()),
            )
            .is_err()
    );
}

#[test]
fn restore_rebuilds_sparse_history_checkpoints_from_verified_snapshot_prefix() {
    use crate::kura::history_checkpoints::HISTORY_CHECKPOINT_INTERVAL;

    let mut chain = chain();
    let checkpoint_height = HISTORY_CHECKPOINT_INTERVAL;
    let tip_height = checkpoint_height + 2;
    for _ in 1..tip_height {
        chain.commit(Vec::new());
    }
    let checkpoint = checkpoint_of(record(&chain.committed(checkpoint_height)));
    let target_height = checkpoint_height - 1;
    let target_hash = chain.committed(target_height).block_hash();
    let claim = snapshot_claim(chain.state());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    // The certified successor binds the World published at the snapshot tip.
    chain.commit(Vec::new());
    let checkpoints = chain.kura().history_checkpoints();
    // A restart has the authenticated journal and decoded snapshot, but no
    // node-local checkpoints retained from the original executions.
    checkpoints.clear();
    assert!(checkpoints.candidates(1, tip_height).is_empty());
    let restored = claim
        .restore(
            &chain.state().ivm_execution_budget(),
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(*restored.cell.view().get(), view.native_execution_tip());
    assert_eq!(
        *restored.cell.predecessor_view().get(),
        *view.native_execution_tip_predecessor.get()
    );
    assert!(restored.world_root.is_some());
    assert_eq!(checkpoints.sparse_len(), 1);
    assert_eq!(
        checkpoints.recent_len(),
        0,
        "restore retains only sparse identities"
    );
    assert!(
        checkpoints
            .candidates(1, tip_height)
            .iter()
            .copied()
            .eq([(checkpoint_height, checkpoint)])
    );

    let target = NonZeroUsize::new(usize::try_from(target_height).unwrap()).unwrap();
    let mut source_reads = 0;
    let mut visited = Vec::new();
    view.canonical_history()
        .visit_executed_backwards_from_checkpoints(
            target,
            target,
            |count, _| {
                source_reads += count;
                Ok(())
            },
            |block| {
                visited.push(block.block_hash());
                Ok(core::ops::ControlFlow::Continue(()))
            },
        )
        .unwrap();
    assert_eq!(
        source_reads, 2,
        "the restored checkpoint bounds the cold read"
    );
    assert_eq!(visited, [target_hash]);
}

#[test]
fn restore_requires_the_certified_successor_of_the_snapshot_tip() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let view = chain.state().view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    assert!(matches!(
        snapshot_claim(chain.state()).restore(
            &chain.state().ivm_execution_budget(),
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        ),
        Err(TipRestoreError::SuccessorRequired { height: 2 })
    ));
}

#[test]
fn restore_passes_each_certified_height_once_to_its_observer() {
    let mut chain = chain();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let claim = snapshot_claim(chain.state());
    let refused_claim = snapshot_claim(chain.state());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    chain.commit(Vec::new());
    let mut observed = Vec::new();
    claim
        .restore(
            &chain.state().ivm_execution_budget(),
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |certified| {
                observed.push(certified.height());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        observed,
        [1, 2, 3],
        "genesis through the tip, not the successor"
    );
    let refusal = refused_claim.restore(
        &chain.state().ivm_execution_budget(),
        view.chain_id(),
        view.network_id(),
        &hashes,
        chain.kura(),
        &mut |certified| {
            if certified.height() == 2 {
                return Err(TipRestoreError::Field {
                    field: "world.validator_committee",
                    message: "refused".to_owned(),
                });
            }
            Ok(())
        },
    );
    assert!(matches!(
        refusal,
        Err(TipRestoreError::Field {
            field: "world.validator_committee",
            ..
        })
    ));
}

#[test]
fn genesis_only_snapshot_cannot_decode_its_unsigned_result_into_authority() {
    let chain = chain();
    let view = chain.state().view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    assert!(
        snapshot_claim(chain.state())
            .restore(
                &chain.state().ivm_execution_budget(),
                view.chain_id(),
                view.network_id(),
                &hashes,
                chain.kura(),
                &mut |_| Ok(()),
            )
            .is_err()
    );
}

#[test]
fn funded_tip_admission_is_atomic_and_original_pool_bound() {
    use mv::BlockAcquisition as _;
    let initial_bytes =
        mv::cell::CellInitialization::<Option<NativeExecutionTip>>::allocation_layouts()
            .iter()
            .map(std::alloc::Layout::size)
            .sum::<usize>();
    let too_small = AllocationBudget::new(initial_bytes - 1);
    assert!(empty_cell(&too_small).is_err());
    assert_eq!(too_small.reserved_bytes(), 0);
    let budget = AllocationBudget::new(64 * 1024);
    let cell = empty_cell(&budget).unwrap();
    let before = budget.reserved_bytes();
    let successor = mv::cell::CellPublicationSuccessor::allocation_layout();
    let foreign = AllocationBudget::new(64 * 1024);
    let mut wrong_parent = foreign.try_reserve_layouts([successor]).unwrap();
    assert!(original_cell(&cell, &budget, &mut wrong_parent).is_err());
    assert_eq!(budget.reserved_bytes(), before);
    let mut parent = budget.try_reserve_layouts([successor]).unwrap();
    let mut original = original_cell(&cell, &budget, &mut parent).unwrap();
    assert_eq!(parent.remaining_bytes(), 0);
    original.initialize(mv::BlockMode::Ordinary);
    drop(original);
    assert_eq!(*cell.view().get(), None);
}

#[test]
fn snapshot_json_preserves_genesis_undo_absence_distinction() {
    let chain = chain();
    let view = chain.state().view();
    let claim = snapshot_claim(chain.state());
    assert!(claim.matches_original(view.native_execution_tip(), Some(None)));
    assert!(!claim.matches_original(view.native_execution_tip(), None));
    let empty = NativeExecutionTipSnapshot::from_original(None, None);
    let empty: NativeExecutionTipSnapshot =
        norito::json::from_str(&norito::json::to_json(&empty).unwrap()).unwrap();
    assert!(empty.matches_original(None, None));
    assert!(!empty.matches_original(None, Some(None)));
}

#[test]
fn restore_preserves_original_pool_refusal_release_and_same_source_retry() {
    // Unrelated tests may pin the process-global reclamation epoch.
    if crate::unit_test_support::run_in_isolated_harness(
        "state::native_execution_tip::tests::restore_preserves_original_pool_refusal_release_and_same_source_retry",
    ) {
        return;
    }
    use crate::{execution_attempt::ExecutionAttemptError, state::deserialize::StateRestoreError};
    use iroha_allocation::AllocationRefusal;
    use std::task::{Context, Waker};

    let mut chain = chain();
    chain.commit(Vec::new());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    let refused_claim = snapshot_claim(&state);
    let retry_claim = snapshot_claim(&state);
    // The certified successor binds the World published at the snapshot tip.
    chain.commit(Vec::new());
    let pool = chain.state().ivm_execution_budget();
    let mut registration = crate::unit_test_support::release_registration(&pool);
    // The waiter is an original fixture owner, admitted before measuring request work.
    let reserved = pool.reserved_bytes();
    let blocker = pool
        .try_reserve_bytes(pool.limit_bytes().checked_sub(reserved).unwrap())
        .unwrap();
    assert!(blocker.belongs_to(&pool));
    let original = CertifiedChain::from_pinned(
        view.chain_id(),
        view.network_id(),
        &hashes,
        chain.kura(),
        &pool,
    )
    .expect_err("the actual original native source must refuse its full pool");
    let ExecutionAttemptError::Deferred(original) = original else {
        panic!("fixture must obtain the original source's typed local refusal: {original:?}");
    };
    let Some(AllocationRefusal::Capacity {
        reserved_bytes,
        limit_bytes,
        release,
        ..
    }) = original.allocation_refusal()
    else {
        panic!("fixture must retain the exact original pool release: {original:?}");
    };
    assert_eq!(*reserved_bytes, pool.limit_bytes());
    assert_eq!(*limit_bytes, pool.limit_bytes());
    let release = release.clone();
    chain.kura().reset_canonical_query_reads_for_test();
    let refused = refused_claim
        .restore(
            &pool,
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        )
        .map_err(StateRestoreError::from);
    let occupied = pool.reserved_bytes();
    let reads = chain.kura().canonical_query_reads_for_test();
    let pending = registration.poll_wait(&release, &mut Context::from_waker(Waker::noop()));
    drop(blocker);
    let poll_outcome = registration.poll_wait(&release, &mut Context::from_waker(Waker::noop()));
    // Retire contention before any assertion, including the causal before-fix assertion.
    assert_eq!(occupied, pool.limit_bytes());
    assert_eq!(reads, (0, 0), "refused backing never reads a native body");
    assert!(pending.is_pending());
    assert!(poll_outcome.is_ready());
    let error = refused
        .err()
        .expect("full original pool must refuse restore");
    let StateRestoreError::ExecutionDeferred(actual) = error else {
        panic!(
            "native tip restore must preserve the original pool refusal as ExecutionDeferred: {error:?}"
        );
    };
    assert_eq!(actual.reason(), original.reason());
    assert_eq!(actual.phase(), original.phase());
    assert_eq!(
        actual.allocation_refusal(),
        original.allocation_refusal(),
        "original release and every allocation field survive restore"
    );
    assert!(actual.decode_refusal().is_none());
    assert!(actual.verification_refusal().is_none());
    let crate::snapshot::TryReadError::StateExecutionDeferred(exported) =
        crate::snapshot::TryReadError::from(StateRestoreError::ExecutionDeferred(actual))
    else {
        panic!("the snapshot boundary must preserve the typed original refusal");
    };
    assert_eq!(exported.reason(), original.reason());
    assert_eq!(exported.phase(), original.phase());
    assert_eq!(exported.allocation_refusal(), original.allocation_refusal());
    assert!(exported.decode_refusal().is_none());
    assert!(exported.verification_refusal().is_none());
    assert_eq!(pool.reserved_bytes(), reserved);
    let restored = retry_claim
        .restore(
            &pool,
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        )
        .map_err(StateRestoreError::from)
        .expect("identical native source retries after original release");
    assert_eq!(*restored.cell.view().get(), view.native_execution_tip());
    assert_eq!(
        *restored.cell.predecessor_view().get(),
        *view.native_execution_tip_predecessor.get()
    );
    assert!(
        pool.reserved_bytes() > reserved,
        "the restored Cell owns its original pool charge"
    );
    drop(restored);
    let retired = TipCell::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum::<usize>();
    // The original State view still pins both retired EBR generations. Their
    // physical backing must remain charged until that reader releases its epoch.
    assert_eq!(pool.reserved_bytes(), reserved + retired);
    assert!(
        view.block_hashes()
            .iter()
            .copied()
            .eq(hashes.iter().copied())
    );
    drop(view);
    drop((chain, state, registration, release, exported, original));
    collect_retired_restore_owners(&pool);
}

fn collect_retired_restore_owners(pool: &AllocationBudget) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while pool.reserved_bytes() != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "restored native owners did not reclaim after the original readers released: {}",
            pool.reserved_bytes()
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
    assert_eq!(pool.reserved_bytes(), 0);
}

fn restore_decode_context(
    pool: &AllocationBudget,
    bytes: usize,
) -> norito::core::DecodeBudgetContext {
    norito::core::DecodeBudgetContext::try_new_owned(
        norito::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            bytes,
            norito::core::MAX_VALUE_NESTING_DEPTH,
        ),
        pool,
    )
    .expect("the original fixture pool funds its cumulative decoder control")
}

#[test]
fn restore_preserves_cumulative_prefix_decode_refusal_and_same_source_retry() {
    // Unrelated tests may pin the process-global reclamation epoch.
    if crate::unit_test_support::run_in_isolated_harness(
        "state::native_execution_tip::tests::restore_preserves_cumulative_prefix_decode_refusal_and_same_source_retry",
    ) {
        return;
    }
    use crate::{execution_attempt::ExecutionAttemptError, state::deserialize::StateRestoreError};
    let mut chain = chain();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    let refused_claim = snapshot_claim(&state);
    let retry_claim = snapshot_claim(&state);
    // The certified successor binds the World published at the snapshot tip.
    chain.commit(Vec::new());
    let pool = chain.state().ivm_execution_budget();
    let baseline = pool.reserved_bytes();
    // Calibrate only the actual canonical constructor, under its original pool and codec.
    // H3 restoration then enters walk(2, 3), so its next genuine prefix decode must refuse.
    let calibration = restore_decode_context(&pool, usize::MAX);
    calibration.with(|| {
        let _reader = CertifiedChain::from_pinned(
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &pool,
        )
        .expect("actual original constructor establishes its own canonical demand");
    });
    let constructor_bytes = usize::try_from(calibration.consumed_allocated_bytes()).unwrap();
    assert!(constructor_bytes > 0);
    drop(calibration);
    assert_eq!(pool.reserved_bytes(), baseline);
    let reference = restore_decode_context(&pool, constructor_bytes);
    let original = reference.with(|| {
        let reader = CertifiedChain::from_pinned(
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &pool,
        )
        .expect("finite allowance must complete the original constructor before prefix refusal");
        reader
            .walk(2, 3)
            .next()
            .unwrap()
            .expect_err("next actual prefix decode must refuse")
    });
    let ExecutionAttemptError::Deferred(original) = original else {
        panic!("fixture must obtain a typed original prefix refusal: {original:?}");
    };
    assert_eq!(
        original.reason(),
        ivm::error::ExecutionDeferral::ActiveMemoryCapacity
    );
    assert!(
        original.allocation_refusal().is_none(),
        "codec accounting has no invented pool release"
    );
    assert_eq!(
        reference.consumed_allocated_bytes(),
        u64::try_from(constructor_bytes).unwrap()
    );
    drop(reference);
    let counter = restore_decode_context(&pool, constructor_bytes);
    let reserved = pool.reserved_bytes();
    let refused = counter.with(|| {
        refused_claim
            .restore(
                &pool,
                view.chain_id(),
                view.network_id(),
                &hashes,
                chain.kura(),
                &mut |_| Ok(()),
            )
            .map_err(StateRestoreError::from)
    });
    assert_eq!(
        counter.consumed_allocated_bytes(),
        u64::try_from(constructor_bytes).unwrap()
    );
    assert_eq!(
        pool.reserved_bytes(),
        reserved,
        "all incomplete native graphs retire"
    );
    let error = refused
        .err()
        .expect("the actual finite prefix allowance must refuse restore");
    let StateRestoreError::ExecutionDeferred(actual) = error else {
        panic!(
            "native tip restore must preserve the cumulative prefix decode refusal as ExecutionDeferred: {error:?}"
        );
    };
    // Independent invocations own different scopes; compare every resource fact,
    // then require each captured identity to stay distinct.
    assert_eq!(actual.reason(), original.reason());
    assert_eq!(actual.phase(), original.phase());
    assert!(actual.allocation_refusal().is_none());
    assert!(actual.verification_refusal().is_none());
    let actual_decode = actual
        .decode_refusal()
        .expect("actual captured decoder owner");
    let reference_decode = original
        .decode_refusal()
        .expect("reference captured decoder owner");
    assert_eq!(
        actual_decode.kind(),
        norito::core::DecodeLocalRefusalKind::EnclosingLimit
    );
    assert_eq!(actual_decode.kind(), reference_decode.kind());
    assert!(
        matches!(actual_decode.resource_error(), norito::core::DecodeResourceError::TotalAllocationExceeded { attempted, limit } if attempted > limit && limit == u64::try_from(constructor_bytes).unwrap())
    );
    assert!(actual.allocator_refusal().is_none());
    assert!(original.allocator_refusal().is_none());
    assert_eq!(
        actual_decode.resource_error(),
        reference_decode.resource_error()
    );
    assert_ne!(
        actual_decode, reference_decode,
        "fresh attempts never share scoped identity"
    );
    let captured = actual.decode_refusal().unwrap().clone();
    let crate::snapshot::TryReadError::StateExecutionDeferred(exported) =
        crate::snapshot::TryReadError::from(StateRestoreError::ExecutionDeferred(actual))
    else {
        panic!("snapshot boundary retains original decoder owner")
    };
    assert_eq!(exported.decode_refusal(), Some(&captured));
    let restored = retry_claim
        .restore(
            &pool,
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        )
        .map_err(StateRestoreError::from)
        .expect("identical native prefix retries after caller decode scope");
    assert_eq!(*restored.cell.view().get(), view.native_execution_tip());
    assert_eq!(
        *restored.cell.predecessor_view().get(),
        *view.native_execution_tip_predecessor.get()
    );
    drop(restored);
    let retired = TipCell::allocation_layouts()
        .into_iter()
        .map(|layout| layout.size())
        .sum::<usize>();
    assert_eq!(pool.reserved_bytes(), reserved + retired);
    drop(counter);
    // Captured refusals retain both original funded counter controls after their
    // contexts retire; the snapshot adapter moves the same physical owner.
    assert_eq!(pool.reserved_bytes(), reserved + retired);
    assert_eq!(exported.decode_refusal(), Some(&captured));
    drop((exported, captured, original));
    assert_eq!(pool.reserved_bytes(), baseline + retired);
    drop(view);
    drop((chain, state));
    collect_retired_restore_owners(&pool);
}

#[test]
fn restore_keeps_corrupt_native_history_a_completed_schema_rejection() {
    use crate::state::deserialize::StateRestoreError;
    let mut chain = chain();
    chain.commit(Vec::new());
    let state = Arc::clone(chain.state());
    let view = state.view();
    let claim = snapshot_claim(&state);
    let hashes: Vec<_> = view.block_hashes().iter().copied().collect();
    // The certified successor binds the World published at the snapshot tip.
    chain.commit(Vec::new());
    let pool = chain.state().ivm_execution_budget();
    let reserved = pool.reserved_bytes();
    // This mutates real durable H2 bytes, leaving its pinned slot/hash and cached body intact.
    chain
        .kura()
        .corrupt_native_frame_for_test(NonZeroUsize::new(2).unwrap());
    let error = claim
        .restore(
            &pool,
            view.chain_id(),
            view.network_id(),
            &hashes,
            chain.kura(),
            &mut |_| Ok(()),
        )
        .map_err(StateRestoreError::from)
        .err()
        .expect("corrupt native history cannot establish authority");
    let StateRestoreError::Serialization(norito::json::Error::InvalidField { field, message }) =
        error
    else {
        panic!("corrupt native history remains a completed schema rejection: {error:?}");
    };
    assert_eq!(field, "native_execution_tip");
    assert!(!message.is_empty());
    assert_eq!(pool.reserved_bytes(), reserved);
    assert_eq!(view.native_execution_tip().unwrap().height(), 2);
}

#[test]
fn native_tip_snapshot_scope_is_mandatory_and_objects_are_closed() {
    let chain = chain();
    let original = norito::json::to_value(&snapshot_claim(chain.state())).unwrap();
    let mut missing_scope = original.clone();
    missing_scope
        .get_mut("blocks")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("root_scope");
    assert!(norito::json::from_value::<NativeExecutionTipSnapshot>(missing_scope).is_err());
    for path in [None, Some("blocks")] {
        let mut unknown = original.clone();
        let object = match path {
            None => unknown.as_object_mut().unwrap(),
            Some(key) => unknown.get_mut(key).unwrap().as_object_mut().unwrap(),
        };
        object.insert("retired_scope".into(), norito::json::Value::Null);
        assert!(norito::json::from_value::<NativeExecutionTipSnapshot>(unknown).is_err());
    }
}
