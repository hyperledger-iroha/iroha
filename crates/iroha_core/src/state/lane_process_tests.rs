// Actual authenticated State/Kura, physical queues and same-reducer custody.
// No production activation, P2P delivery or economic Apply claim.

fn native_process_three_route_source_fixture() -> Box<NativeProcessFixture> {
    let genesis = empty_global_block_after(None);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = LaneCatalog::new(
        nonzero!(3_u32),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "input-secondary".into(),
                ..LaneConfig::default()
            },
            LaneConfig {
                id: LaneId::new(2),
                alias: "process-tertiary".into(),
                ..LaneConfig::default()
            },
        ],
    )
    .unwrap();
    nexus.lane_config =
        iroha_config::parameters::actual::LaneConfig::from_catalog(&nexus.lane_catalog);
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    let config = strict_kura_config_for_testing(std::path::PathBuf::new());
    let kura = Kura::new_temporary_with_configured_lane_catalog(
        &config,
        &nexus.lane_config,
        &nexus.lane_catalog,
    )
    .unwrap();
    let mut state = State::try_new_with_chain_and_network_id(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        (*DEFAULT_TEST_CHAIN_ID).clone(),
        iroha_data_model::NetworkId::from_genesis_hash(genesis.hash()),
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
    .unwrap();
    state.configure_test_runtime_defaults();
    state.install_pre_genesis_nexus_for_testing(nexus);
    let state = Arc::new(state);
    let (ids, validators) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &validators);
    install_lane_manifest_registry(
        &state,
        &[
            (LaneId::SINGLE, DataSpaceId::UNIVERSAL, ids.clone()),
            (LaneId::new(1), DataSpaceId::UNIVERSAL, ids.clone()),
            (LaneId::new(2), DataSpaceId::UNIVERSAL, ids),
        ],
    );
    let _ = configure_commit_topology_preserving_world_peers(&state, 1);
    kura.store_block(Arc::new(genesis.clone())).unwrap();
    commit_block_metadata_with_genesis_checkpoint_to_state(&state, &genesis);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, genesis);
    let primary = crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let secondary = crate::queue::RoutingDecision::new(LaneId::new(1), DataSpaceId::UNIVERSAL);
    let plan = crate::queue::RoutingPlan::native_amx(
        primary,
        vec![
            crate::queue::RouteLeg::new(primary, crate::queue::RouteLegRole::Participant),
            crate::queue::RouteLeg::new(secondary, crate::queue::RouteLegRole::Participant),
            crate::queue::RouteLeg::new(
                crate::queue::RoutingDecision::new(LaneId::new(2), DataSpaceId::UNIVERSAL),
                crate::queue::RouteLegRole::Participant,
            ),
        ],
    );
    let (binding, complete) = queue_plan_admission_certificate_for_state_test(
        &state,
        plan,
        &validators,
        parent.header().height().get(),
        0x81,
    );
    let publish = |parent: &SignedBlock, controls: Vec<Vec<u8>>| {
        let mut block = empty_global_block_after(Some(parent));
        let mut execution = block.execution_context().cloned().unwrap_or_default();
        execution.queue_plan_admissions = controls.clone();
        block.set_execution_context(Some(execution));
        let opening = match state
            .kura
            .v2_finality_artifact(parent.header().height().get())
            .unwrap()
        {
            Some(exact_parent) => crate::sumeragi::v2_context::build_successor_height_context(
                &exact_parent,
                exact_parent.height_context.nexus_amx_context_hash,
                None,
            )
            .unwrap(),
            None => lane_opening_context_for_state_test(&state),
        };
        let mut overlay = Box::new(
            state
                .block_with_queue_plan_admissions(block.header(), &controls)
                .unwrap(),
        );
        overlay
            .finalize_lane_consensus_contexts(&block, Some(&opening))
            .unwrap();
        let mut witness = ExecWitness::default();
        overlay
            .capture_lane_consensus_contexts(&mut witness)
            .unwrap();
        overlay.block_hashes.push(block.hash());
        insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
        overlay.commit().unwrap();
        state.kura.store_block(Arc::new(block.clone())).unwrap();
        let (artifact, receipt) =
            stage_lane_context_fixture_finality(&state, &block, opening.clone(), witness.clone());
        state
            .kura
            .promote_kagemusha_finality_sidecar(&artifact, &receipt)
            .unwrap();
        (block, opening, witness)
    };
    let (block, _, _) = publish(&parent, vec![complete]);
    Box::new(NativeProcessFixture {
        state,
        keys: validators,
        block,
        binding,
        records: Vec::new(),
    })
}

struct NativeProcessFixture {
    state: Arc<State>,
    keys: Vec<KeyPair>,
    block: SignedBlock,
    binding: crate::torii_proxy::QueuePlanAdmissionBindingV1,
    records: Vec<crate::sumeragi::v2_lane_wire::LaneWalEnvelopeV1>,
}
// Outline the large State move; actual workers and tests use default stacks.
fn native_process_fixture(
    seed_replay: bool,
    start: std::time::Instant,
) -> Box<NativeProcessFixture> {
    let mut fixture = native_process_three_route_source_fixture();
    if seed_replay {
        let observed = fixture
            .state
            .verified_lane_consensus_contexts()
            .unwrap()
            .unwrap();
        let lane = &observed.contexts()[0];
        let mut old = crate::sumeragi::v2_lane_instance::LaneInstance::open_with_worker_for_test(
            &fixture.state,
            &observed,
            lane,
            native_process_key(&fixture, lane, 0),
            crate::sumeragi::output_guard::ConsensusOutputGuard::isolated(),
            start,
            Duration::from_secs(1),
            Duration::from_millis(100),
            3 * crate::sumeragi::v2_core::MAX_EFFECTS_PER_STEP,
        )
        .unwrap();
        let due = start + Duration::from_secs(1);
        old.poll_clock(&fixture.state, &observed, due).unwrap();
        old.service_with_worker(&fixture.state, &observed, due)
            .unwrap();
        fixture.records = old.native_records().to_vec();
        assert_eq!(fixture.records.len(), 1);
        drop(old);
    }
    fixture
}
fn native_process_limits_for_test() -> crate::sumeragi::v2_lane_instance::LaneProcessLimits {
    crate::sumeragi::v2_lane_instance::LaneProcessLimits {
        instances: nonzero!(3_usize),
        workers_per_class: nonzero!(1_usize),
        queued_per_class: nonzero!(1_usize),
        completed: nonzero!(1_usize),
        effect_limit: 3 * crate::sumeragi::v2_core::MAX_EFFECTS_PER_STEP,
        base_timeout: Duration::from_secs(1),
        retransmit: Duration::from_millis(100),
    }
}
fn native_process_key(
    fixture: &NativeProcessFixture,
    lane: &VerifiedLaneContext,
    signer: usize,
) -> KeyPair {
    fixture
        .keys
        .iter()
        .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
        .unwrap()
        .clone()
}
fn native_process_receive(
    pool: &crate::sumeragi::v2_lane_instance::LanePhysicalPool,
) -> crate::sumeragi::v2_lane_instance::LanePhysicalCompletion {
    let until = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(done) = pool.try_completion().unwrap() {
            return done;
        }
        assert!(
            std::time::Instant::now() < until,
            "physical worker completion deadline"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn native_process_open_for_test(
    table: &mut crate::sumeragi::v2_lane_instance::LaneProcessOwner,
    pool: &crate::sumeragi::v2_lane_instance::LanePhysicalPool,
    fixture: &NativeProcessFixture,
    observed: &VerifiedLaneContexts,
    lane: &VerifiedLaneContext,
    signer: usize,
    now: std::time::Instant,
) {
    use crate::sumeragi::v2_lane_instance::{LaneProcessProgress, LaneWorkerClass};
    table
        .reserve_opening(
            observed,
            lane,
            native_process_key(fixture, lane, signer),
            now,
        )
        .unwrap();
    assert!(matches!(
        table.dispatch_one(pool, LaneWorkerClass::Opening).unwrap(),
        LaneProcessProgress::Dispatched
    ));
    table
        .accept_completion(native_process_receive(pool), observed)
        .unwrap();
    assert!(matches!(
        table.settle_opening(lane.instance_id(), observed).unwrap(),
        LaneProcessProgress::OpeningAdopted
    ));
}
fn native_process_advance(fixture: &NativeProcessFixture, close: bool) -> SignedBlock {
    let state = &fixture.state;
    let parent = state
        .kura
        .v2_finality_artifact(fixture.block.header().height().get())
        .unwrap()
        .unwrap();
    let opening = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    let block = empty_global_block_after(Some(&fixture.block));
    let mut overlay = Box::new(state.block(block.header()));
    if close {
        assert!(
            State::resolve_queue_plan_pending_obligation_in_storage(
                &mut overlay.world.smart_contract_state,
                fixture.binding.network_id_digest,
                fixture.binding.entrypoint_hash
            )
            .unwrap()
        );
    }
    overlay
        .finalize_lane_consensus_contexts(&block, Some(&opening))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact, receipt) = stage_lane_context_fixture_finality(state, &block, opening, witness);
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    block
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_process_blocked_opening_replays_while_other_lane_fsyncs_and_signs
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::{LaneProcessOwner,LanePhysicalPool,LaneWorkerClass,LaneProcessProgress,LaneService,LaneInputOutcome}};
    use std::{sync::mpsc,time::Instant};
    let now=Instant::now();let due=now+Duration::from_secs(1);let fixture=native_process_fixture(true,now);
    let observed=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();assert_eq!(observed.contexts().len(),3);
    let guard=ConsensusOutputGuard::isolated();let limits=native_process_limits_for_test();
    let pool=LanePhysicalPool::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    let mut table=LaneProcessOwner::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    let a=&observed.contexts()[0];let b=&observed.contexts()[1];
    native_process_open_for_test(&mut table,&pool,&fixture,&observed,b,0,now);
    assert_eq!(table.instance_ids().collect::<Vec<_>>(),vec![b.instance_id()]);
    assert_eq!(table.next_deadline(),Some(now+Duration::from_millis(100)));
    table.reserve_opening(&observed,a,native_process_key(&fixture,a,0),now).unwrap();
    assert!(table.reserve_opening(&observed,a,native_process_key(&fixture,a,1),now).is_err(),"one frozen key per instance reservation");
    let (entered,entered_rx)=mpsc::sync_channel(0);let (release,release_rx)=mpsc::sync_channel(0);
    table.hold_next_job_for_test(a.instance_id(),LaneWorkerClass::Opening,move||{entered.send(()).unwrap();release_rx.recv().unwrap();}).unwrap();
    table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap();entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(table.poll_clock(b.instance_id(),&observed,due).unwrap(),LaneInputOutcome::Stepped(_)));
    table.prepare_persistence(b.instance_id()).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Wal).unwrap();
    assert!(matches!(table.accept_completion(native_process_receive(&pool),&observed).unwrap(),LaneProcessProgress::Persistence(LaneService::PersistedAwaitingAck)));
    assert!(matches!(table.service_one(b.instance_id(),&observed,due).unwrap(),LaneService::Completion(_)));
    assert!(matches!(table.service_one(b.instance_id(),&observed,due).unwrap(),LaneService::SignedAwaitingAck));
    assert_eq!(table.instance(b.instance_id()).unwrap().native_records().len(),1);
    release.send(()).unwrap();table.accept_completion(native_process_receive(&pool),&observed).unwrap();table.settle_opening(a.instance_id(),&observed).unwrap();
    assert_eq!(table.instance(a.instance_id()).unwrap().native_records(),fixture.records);
    assert_eq!(table.instance(a.instance_id()).unwrap().timeout_deadline(),Some(due));
    assert!(matches!(table.service_one(a.instance_id(),&observed,due).unwrap(),LaneService::SignedAwaitingAck),"actual replay resumes the persisted timeout intent");
    assert!(!guard.restart_required());drop(table);pool.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_process_full_queue_foreign_completion_and_global_advance_keep_exact_owner
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::{LaneProcessOwner,LanePhysicalPool,LaneWorkerClass,LaneProcessProgress,LaneService,LaneCurrentGate}};
    use std::{sync::mpsc,time::Instant};
    let now=Instant::now();let due=now+Duration::from_secs(1);let fixture=native_process_fixture(false,now);
    let observed=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let guard=ConsensusOutputGuard::isolated();let limits=native_process_limits_for_test();
    let pool=LanePhysicalPool::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    let mut table=LaneProcessOwner::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    let lane=&observed.contexts()[0];
    table.reserve_opening(&observed,lane,native_process_key(&fixture,lane,0),now).unwrap();
    let (entered,entered_rx)=mpsc::sync_channel(0);let (release,release_rx)=mpsc::sync_channel(0);
    table.hold_next_job_for_test(lane.instance_id(),LaneWorkerClass::Opening,move||{entered.send(()).unwrap();release_rx.recv().unwrap();}).unwrap();
    table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap();entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    for next in &observed.contexts()[1..] {table.reserve_opening(&observed,next,native_process_key(&fixture,next,0),now).unwrap();}
    assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap(),LaneProcessProgress::Dispatched));
    assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap(),LaneProcessProgress::QueueFull));
    let before:crate::sumeragi::v2_lane_instance::LaneProcessOccupancy=table.occupancy();assert_eq!((before.instances,before.queued,before.transferred),(3,1,2));
    for _ in 0..3 {assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap(),LaneProcessProgress::QueueFull));assert_eq!(table.occupancy(),before);}
    release.send(()).unwrap();let done=native_process_receive(&pool);
    let foreign_guard=ConsensusOutputGuard::isolated();
    let mut foreign=LaneProcessOwner::new(Arc::clone(&fixture.state),Arc::clone(&foreign_guard),limits).unwrap();
    let done=match foreign.accept_completion(done,&observed){Err((error,done))=>{assert!(error.to_string().contains("foreign"));done},Ok(_)=>panic!("foreign table must return result custody")};
    assert!(!guard.restart_required() && !foreign_guard.restart_required());
    table.accept_completion(done,&observed).unwrap();
    native_process_advance(&fixture,false);
    assert_eq!(table.reconcile(&observed),LaneCurrentGate::ObservationChanged);
    assert!(matches!(table.settle_opening(lane.instance_id(),&observed).unwrap(),LaneProcessProgress::OpeningRetained));
    let current=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(matches!(table.settle_opening(lane.instance_id(),&current).unwrap(),LaneProcessProgress::OpeningAdopted));
    assert_eq!(table.instance(lane.instance_id()).unwrap().timeout_deadline(),Some(due),"later global publication does not reset queued opening clock");
    // The queued third job transfers only after actual queue capacity returns.
    assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap(),LaneProcessProgress::Dispatched));
    for _ in 0..2 {table.accept_completion(native_process_receive(&pool),&current).unwrap();for other in current.contexts(){let _=table.settle_opening(other.instance_id(),&current).unwrap();}}
    assert_eq!(table.occupancy().queued+table.occupancy().transferred,0);
    let tag=table.instance(lane.instance_id()).unwrap().tag();
    table.poll_clock(lane.instance_id(),&current,due).unwrap();table.prepare_persistence(lane.instance_id()).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Wal).unwrap();
    table.accept_completion(native_process_receive(&pool),&current).unwrap();
    assert!(matches!(table.service_one(lane.instance_id(),&current,due).unwrap(),LaneService::Completion(_)));
    assert!(matches!(table.service_one(lane.instance_id(),&current,due).unwrap(),LaneService::SignedAwaitingAck));
    table.service_one(lane.instance_id(),&current,due).unwrap();
    let (send,recv)=mpsc::sync_channel(1);
    assert!(matches!(table.flush_one(lane.instance_id(),&current,&send).unwrap(),LaneService::Sent));
    table.poll_clock(lane.instance_id(),&current,due+Duration::from_millis(100)).unwrap();
    assert!(matches!(table.flush_one(lane.instance_id(),&current,&send).unwrap(),LaneService::OutboxFull));
    let packet=recv.recv().unwrap();
    let iroha_data_model::block::lane_consensus::LaneMessageV1::TimeoutVote(vote)=&packet.envelope.message else {panic!("actual frozen-key vote")};
    assert_eq!(vote.share.signer,0);crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(lane).event(&packet.envelope.message,tag).unwrap();
    assert_eq!(packet.destinations,lane.frozen().committee);
    assert_eq!(packet.canonical_bytes,norito::encode_canonical(&packet.envelope).unwrap());
    assert_eq!(table.instance(lane.instance_id()).unwrap().tag(),tag);
    assert!(matches!(table.flush_one(lane.instance_id(),&current,&send).unwrap(),LaneService::Sent));
    assert_eq!(recv.recv().unwrap().canonical_bytes,packet.canonical_bytes);
    assert!(!guard.restart_required());drop(foreign);drop(table);pool.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_process_authenticated_closure_drains_body_and_wal_without_apply_ack
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::{LaneProcessOwner,LanePhysicalPool,LaneWorkerClass,LaneProcessProgress,LaneService,LaneCurrentGate}};
    use std::{sync::mpsc,time::Instant};
    let now=Instant::now();let due=now+Duration::from_secs(1);let fixture=native_process_fixture(false,now);
    let observed=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();let lane=&observed.contexts()[0];
    let leader=lane.reducer_context().roster().iter().position(|v|v.id()==lane.reducer_context().leader(0)).unwrap();
    let guard=ConsensusOutputGuard::isolated();let limits=native_process_limits_for_test();
    let pool=LanePhysicalPool::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    let mut table=LaneProcessOwner::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    native_process_open_for_test(&mut table,&pool,&fixture,&observed,lane,leader,now);
    table.prepare_body(lane.instance_id(),&observed).unwrap();
    let (entered,entered_rx)=mpsc::sync_channel(0);let (release,release_rx)=mpsc::sync_channel(0);
    table.hold_next_job_for_test(lane.instance_id(),LaneWorkerClass::Body,move||{entered.send(()).unwrap();release_rx.recv().unwrap();}).unwrap();
    table.dispatch_one(&pool,LaneWorkerClass::Body).unwrap();entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    table.poll_clock(lane.instance_id(),&observed,due).unwrap();table.prepare_persistence(lane.instance_id()).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Wal).unwrap();
    assert!(matches!(table.accept_completion(native_process_receive(&pool),&observed).unwrap(),LaneProcessProgress::Persistence(LaneService::PersistedAwaitingAck)));
    assert_eq!(table.instance(lane.instance_id()).unwrap().native_records().len(),1);
    native_process_advance(&fixture,true);let closed=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();assert!(closed.contexts().is_empty());
    assert_eq!(table.reconcile(&closed),LaneCurrentGate::Current);
    assert!(table.poll_clock(lane.instance_id(),&closed,due).is_err());
    table.prepare_closed_drain(lane.instance_id()).unwrap();assert_eq!(table.occupancy().transferred,1,"actual body handle still belongs to worker");
    release.send(()).unwrap();table.accept_completion(native_process_receive(&pool),&closed).unwrap();
    table.prepare_closed_drain(lane.instance_id()).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Body).unwrap();
    assert!(matches!(table.accept_completion(native_process_receive(&pool),&closed).unwrap(),LaneProcessProgress::ClosedDrained));
    assert_eq!(table.occupancy().closed,1);assert_eq!(table.occupancy().instances,1,"unconsumed protocol custody remains capacity-accounted");
    let retired:crate::sumeragi::v2_lane_instance::LaneClosedInstance=table.take_closed(lane.instance_id()).unwrap();assert_eq!(table.occupancy().instances,0);
    assert_eq!(retired.instance().native_records().len(),1);assert_eq!(retired.instance().tag().view(),0);
    assert!(retired.unacknowledged_control().is_some(),"physical fsync does not fabricate a reducer/global Apply ack on closure");
    assert!(retired.instance().native_decision().unwrap().is_none());
    assert!(!guard.restart_required());drop(table);
    assert!(!guard.restart_required(),"explicit retirement consumer now owns all closed obligations");
    drop(retired);assert!(guard.restart_required(),"discard is not a retirement acknowledgement");
    let shutdown:crate::sumeragi::v2_lane_instance::LanePhysicalShutdown=pool.shutdown();shutdown.join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_process_corrupt_opening_and_dropped_completion_fence_output
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::{LaneProcessOwner,LanePhysicalPool,LaneWorkerClass,LaneProcessProgress}};
    use std::time::Instant;
    for corrupt in [false,true] {
        let now=Instant::now();let fixture=native_process_fixture(corrupt,now);
        let observed=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();let lane=&observed.contexts()[0];
        let bytes=if corrupt {
            let path=std::fs::read_dir(fixture.state.kura.sumeragi_v2_storage_root().join("wal")).unwrap().map(|entry|entry.unwrap().path()).find(|path|path.extension().is_some_and(|ext|ext=="wal")).unwrap();
            let mut bytes=std::fs::read(&path).unwrap();bytes[0]^=1;std::fs::write(&path,&bytes).unwrap();Some((path,bytes))
        } else {None};
        let guard=ConsensusOutputGuard::isolated();let limits=native_process_limits_for_test();
        let pool=LanePhysicalPool::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
        let mut table=LaneProcessOwner::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
        table.reserve_opening(&observed,lane,native_process_key(&fixture,lane,0),now).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap();
        let result=native_process_receive(&pool);
        if corrupt {
            assert!(guard.restart_required(),"physical error fences before result adoption");
            table.accept_completion(result,&observed).unwrap();
            assert!(matches!(table.settle_opening(lane.instance_id(),&observed).unwrap(),LaneProcessProgress::Failed(_)));
            table.dispatch_one(&pool,LaneWorkerClass::Opening).unwrap();
            assert!(matches!(table.accept_completion(native_process_receive(&pool),&observed).unwrap(),LaneProcessProgress::OpeningDrained));
            assert_eq!(table.occupancy().instances,0);let (path,bytes)=bytes.unwrap();assert_eq!(std::fs::read(path).unwrap(),bytes);
        } else {assert!(!guard.restart_required());drop(result);assert!(guard.restart_required(),"losing exact private result cannot leave a live process");}
        drop(table);pool.shutdown().join().unwrap();
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_process_actual_body_receipt_and_native_decision_remain_owned_for_global_consumer
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_core as core,v2_lane_instance::{LaneProcessOwner,LanePhysicalPool,LaneWorkerClass,LaneProcessProgress,LaneBodyProgress,LaneService},v2_lane_wire::LaneAuthenticator};
    use iroha_data_model::block::lane_consensus::{LaneMessageV1,LanePhaseV1,LaneQcV1,LaneRoundV1,LaneVoteStatementV1,LaneSignatureShareV1};
    use std::{sync::mpsc,time::Instant};
    let now=Instant::now();let fixture=native_process_fixture(false,now);
    let observed=fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();let lane=&observed.contexts()[0];let id=lane.instance_id();
    let signer=lane.reducer_context().roster().iter().position(|v|v.id()==lane.reducer_context().leader(0)).unwrap();
    let guard=ConsensusOutputGuard::isolated();let limits=native_process_limits_for_test();
    let pool=LanePhysicalPool::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    let mut table=LaneProcessOwner::new(Arc::clone(&fixture.state),Arc::clone(&guard),limits).unwrap();
    native_process_open_for_test(&mut table,&pool,&fixture,&observed,lane,signer,now);
    let before=crate::snapshot::canonical_state_snapshot_hash(&fixture.state);
    table.prepare_body(id,&observed).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Body).unwrap();
    assert!(matches!(table.accept_completion(native_process_receive(&pool),&observed).unwrap(),LaneProcessProgress::Body(LaneBodyProgress::Stepped(_))));
    // LocalProposalReady follows actual source recovery, RS16, fsync/readback and
    // private body receipt; the next physical write is exact ProposalIntent.
    table.prepare_persistence(id).unwrap();table.dispatch_one(&pool,LaneWorkerClass::Wal).unwrap();
    table.accept_completion(native_process_receive(&pool),&observed).unwrap();
    assert!(matches!(table.service_one(id,&observed,now).unwrap(),LaneService::Completion(_)));
    assert!(matches!(table.service_one(id,&observed,now).unwrap(),LaneService::SignedAwaitingAck));
    table.service_one(id,&observed,now).unwrap();
    let (send,recv)=mpsc::sync_channel(1);
    assert!(matches!(table.flush_one(id,&observed,&send).unwrap(),LaneService::Sent));
    let packet=recv.recv().unwrap();let LaneMessageV1::Proposal(proposal)=&packet.envelope.message else {panic!("actual native proposal")};
    LaneAuthenticator::new(lane).event(&packet.envelope.message,table.instance(id).unwrap().tag()).unwrap();
    let statement=LaneVoteStatementV1 {round:LaneRoundV1{instance_id:proposal.body.manifest.value.instance_id,lane_height:lane.frozen().next_lane_height,voting_view:0},phase:LanePhaseV1::Commit,value:proposal.body.manifest.value};
    let qc=LaneQcV1{statement,shares:(0..3).map(|index|{let key=native_process_key(&fixture,lane,index);LaneSignatureShareV1{signer:index as u32,signature:Signature::try_new(key.private_key(),&statement.signature_preimage().unwrap()).unwrap().payload().to_vec()}}).collect()};
    // Proposal completion can already own a Prepare Persist. Retain the exact
    // incoming QC across Busy instead of interpreting an ignored Backpressured
    // return as admission, or the next physical write as necessarily Decision.
    let mut admitted = false;
    let mut completion_due = false;
    for _ in 0..32 {
        if !admitted {
            match table.offer(id,&observed,&LaneMessageV1::QuorumCertificate(qc.clone())).unwrap() {
                crate::sumeragi::v2_lane_instance::LaneInputOutcome::Stepped(_) => admitted = true,
                crate::sumeragi::v2_lane_instance::LaneInputOutcome::Backpressured => {
                    assert!(completion_due || table.instance(id).unwrap().held_effects().any(|effect| matches!(effect,core::Effect::Persist{..})),
                        "the retained QC is blocked by real issued durability custody");
                },
                _ => panic!("current authenticated QC must be admitted or remain owned behind Persist"),
            }
        }
        if !completion_due && table.instance(id).unwrap().native_decision().unwrap().is_some() { break; }
        match table.service_one(id,&observed,now).unwrap() {
            LaneService::NeedsPersistenceWorker => {
                table.prepare_persistence(id).unwrap();
                assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Wal).unwrap(),LaneProcessProgress::Dispatched));
                assert!(matches!(table.accept_completion(native_process_receive(&pool),&observed).unwrap(),
                    LaneProcessProgress::Persistence(LaneService::PersistedAwaitingAck)));
                completion_due = true;
            },
            LaneService::NeedsBodyAdapter | LaneService::BodyWaiting(_) => {
                table.prepare_body(id,&observed).unwrap();
                assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Body).unwrap(),LaneProcessProgress::Dispatched));
                table.accept_completion(native_process_receive(&pool),&observed).unwrap();
            },
            LaneService::Completion(_) => completion_due = false,
            LaneService::SignedAwaitingAck => completion_due = true,
            LaneService::Idle => {},
            other => panic!("unexpected actual control progress {other:?}"),
        }
    }
    assert!(admitted,"exact QC remained owned until successful reducer admission");
    let decision=table.instance(id).unwrap().native_decision().unwrap().expect("durable Decision exposed without waiting for other routes/global execution");
    assert_eq!(decision.commit_qc,qc);
    for _ in 0..6 {
        if table.instance(id).unwrap().held_effects().any(|effect|matches!(effect,core::Effect::Apply{..})){break;}
        table.prepare_body(id,&observed).unwrap();
        assert!(matches!(table.dispatch_one(&pool,LaneWorkerClass::Body).unwrap(),LaneProcessProgress::Dispatched));
        table.accept_completion(native_process_receive(&pool),&observed).unwrap();
    }
    assert!(table.instance(id).unwrap().held_effects().any(|effect|matches!(effect,core::Effect::Apply{..})),"the sole global consumer still owns the future Apply acknowledgement");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state),before);
    assert!(table.instance(id).unwrap().source_recovery_requirement().is_none());
    assert!(!guard.restart_required());drop(table);pool.shutdown().join().unwrap();
    assert_eq!(packet.canonical_bytes,norito::encode_canonical(&packet.envelope).unwrap(),"already-transferred native packet stays transport-owned even when table closes");
}
