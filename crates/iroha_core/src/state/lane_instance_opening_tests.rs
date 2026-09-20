// Physical opening/replay workers; no production table or worker pool activation.

fn prepare_lane_opening_for_test(
    fixture: &LaneContextVerifiedFixture,
    observed: &VerifiedLaneContexts,
    lane: &VerifiedLaneContext,
    signer: usize,
    guard: Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
    now: std::time::Instant,
) -> (
    crate::sumeragi::v2_lane_instance::LaneOpening,
    crate::sumeragi::v2_lane_instance::LaneOpeningJob,
) {
    use crate::sumeragi::{v2_core, v2_lane_instance::LaneInstance};
    let key = fixture
        .validators
        .iter()
        .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
        .unwrap()
        .clone();
    LaneInstance::prepare_opening(
        &fixture.state,
        observed,
        lane,
        fixture.state.kura_handle(),
        key,
        guard,
        now,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_millis(100),
        3 * v2_core::MAX_EFFECTS_PER_STEP,
    )
    .unwrap()
}

fn run_lane_opening_for_test(
    job: crate::sumeragi::v2_lane_instance::LaneOpeningJob,
) -> crate::sumeragi::v2_lane_instance::LaneOpeningCompletion {
    std::thread::scope(|scope| scope.spawn(move || job.run()).join().unwrap())
}

fn run_lane_opening_drain_for_test(
    drain: crate::sumeragi::v2_lane_instance::LaneOpeningDrain,
) -> crate::sumeragi::v2_lane_instance::LaneOpeningDrained {
    std::thread::scope(|scope| scope.spawn(move || drain.run()).join().unwrap())
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_opening_worker_replays_exact_wal_while_another_lane_times_out
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_core as core,
        v2_lane_instance::{LaneOpeningAdoption,LaneInputOutcome,LaneService}};
    use std::{sync::mpsc,time::{Duration,Instant}};
    let fixture=all_route_input_fixture(false);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(observed.contexts().len(),2);
    let now=Instant::now();
    let due=now+Duration::from_secs(1);
    // Real completed fsync with its Persisted event deliberately not acknowledged.
    // Drop all old handles before a new process-scope guard reopens that signer.
    let mut previous=open_lane_instance_for_test(&fixture,&observed,&observed.contexts()[0],0,now);
    previous.poll_clock(state,&observed,due).unwrap();
    previous.service_with_worker(state,&observed,due).unwrap();
    let exact=previous.native_records().to_vec();
    assert_eq!(exact.len(),1);
    assert!(previous.held_effects().next().is_none());
    drop(previous);
    let guard=ConsensusOutputGuard::isolated();
    let mut other=open_guarded_lane_instance_for_test(&fixture,&observed,&observed.contexts()[1],0,Arc::clone(&guard),now);
    let (opening,job)=prepare_lane_opening_for_test(&fixture,&observed,&observed.contexts()[0],0,Arc::clone(&guard),now);
    assert_eq!(opening.instance_id(),observed.contexts()[0].instance_id());
    assert_eq!(opening.signer(),0);
    let (entered_send,entered_recv)=mpsc::sync_channel(0);
    let (release_send,release_recv)=mpsc::sync_channel(0);
    let job=job.after_wal_open_for_test(move || {
        entered_send.send(()).unwrap();
        release_recv.recv().unwrap();
    });
    let worker=std::thread::spawn(move || job.run());
    entered_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!guard.restart_required());
    assert!(matches!(other.poll_clock(state,&observed,due).unwrap(),LaneInputOutcome::Stepped(_)));
    assert!(matches!(other.service_with_worker(state,&observed,due).unwrap(),LaneService::PersistedAwaitingAck));
    assert!(matches!(other.service_one(state,&observed,due).unwrap(),LaneService::Completion(_)));
    assert!(matches!(other.service_one(state,&observed,due).unwrap(),LaneService::SignedAwaitingAck),
        "a blocked real WAL opener holds no global publication/output permit");
    release_send.send(()).unwrap();
    let completed=worker.join().unwrap();
    let LaneOpeningAdoption::Opened(mut owner)=opening.adopt(state,&observed,completed).unwrap()
        else {panic!("fresh exact instance adopts the replayed physical owners");};
    assert_eq!(owner.native_records(),exact);
    assert_eq!(owner.timeout_deadline(),Some(due),"queue/open delay does not reset the admitted clock");
    assert!(matches!(owner.held_effects().collect::<Vec<_>>().as_slice(),
        [core::Effect::Sign {message:core::SignableMessage::TimeoutVote(_),..}]),
        "actual WAL replay, not a fresh reducer, resumes the unacknowledged intent");
    assert!(matches!(owner.service_one(state,&observed,due).unwrap(),LaneService::SignedAwaitingAck));
    assert!(!guard.restart_required());
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_opening_queue_full_and_foreign_completion_preserve_exact_owners
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_instance::{LaneOpeningAdoption,LaneOpeningJob,LaneInstance}};
    use std::{sync::mpsc::{self,TrySendError},time::Instant};
    let fixture=finalized_lane_wal_fixture();
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let guard=ConsensusOutputGuard::isolated();
    let now=Instant::now();
    let (left,left_job)=prepare_lane_opening_for_test(&fixture,&observed,lane,0,Arc::clone(&guard),now);
    let (right,right_job)=prepare_lane_opening_for_test(&fixture,&observed,lane,1,Arc::clone(&guard),now);
    let (send,recv)=mpsc::sync_channel::<LaneOpeningJob>(1);
    assert!(send.try_send(left_job).is_ok());
    let right_job=match send.try_send(right_job) {
        Err(TrySendError::Full(job))=>job,
        _=>panic!("the exact rejected handoff must return its owned job"),
    };
    assert!(!guard.restart_required());
    let left_done=run_lane_opening_for_test(recv.recv().unwrap());
    assert!(send.try_send(right_job).is_ok());
    let right_done=run_lane_opening_for_test(recv.recv().unwrap());
    let (error,left,right_done)=match left.adopt(state,&observed,right_done) {
        Err(owned)=>owned,
        Ok(_)=>panic!("same instance is insufficient: each local signer/ticket is exact"),
    };
    assert!(error.to_string().contains("foreign native opening completion"));
    assert_eq!(left.signer(),0);
    assert_eq!(right.signer(),1);
    let other_state=finalized_lane_wal_fixture();
    let (error,left,left_done)=match left.adopt(&other_state.state,&observed,left_done) {
        Err(owned)=>owned,
        Ok(_)=>panic!("even authenticated contexts cannot cross the physical Kura owner"),
    };
    assert!(error.to_string().contains("foreign State storage owner"));
    let LaneOpeningAdoption::Opened(left)=left.adopt(state,&observed,left_done).unwrap() else {panic!("left");};
    let LaneOpeningAdoption::Opened(right)=right.adopt(state,&observed,right_done).unwrap() else {panic!("right");};
    assert!(left.native_records().is_empty() && right.native_records().is_empty());
    assert!(!guard.restart_required());
    let key=fixture.validators.iter().find(|key| key.public_key()==lane.frozen().committee[2].public_key()).unwrap().clone();
    assert!(LaneInstance::prepare_opening(state,&observed,lane,other_state.state.kura_handle(),key,
        Arc::clone(&guard),now,std::time::Duration::from_secs(1),std::time::Duration::from_millis(100),
        3*crate::sumeragi::v2_core::MAX_EFFECTS_PER_STEP).is_err());
    assert!(!guard.restart_required(),"rejected pre-admission arguments created no armed physical job");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_opening_authenticated_closure_retains_observation_then_drains_without_resume
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::LaneOpeningAdoption};
    use std::{sync::mpsc,time::{Duration,Instant}};
    let (fixture,_)=first_lane_input_fixture(0xB4);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let guard=ConsensusOutputGuard::isolated();
    let (opening,job)=prepare_lane_opening_for_test(&fixture,&observed,lane,0,Arc::clone(&guard),Instant::now());
    let abandoned_guard=ConsensusOutputGuard::isolated();
    let (abandoned,abandoned_job)=prepare_lane_opening_for_test(&fixture,&observed,lane,1,Arc::clone(&abandoned_guard),Instant::now());
    let abandoned_completion=run_lane_opening_for_test(abandoned_job);
    let (entered_send,entered_recv)=mpsc::sync_channel(0);
    let (release_send,release_recv)=mpsc::sync_channel(0);
    let worker=std::thread::spawn(move || job.after_wal_open_for_test(move || {
        entered_send.send(()).unwrap();release_recv.recv().unwrap();
    }).run());
    entered_recv.recv_timeout(Duration::from_secs(5)).unwrap();
    let parent=state.kura.v2_finality_artifact(fixture.block.header().height().get()).unwrap().unwrap();
    let context=crate::sumeragi::v2_context::build_successor_height_context(&parent,parent.height_context.nexus_amx_context_hash,None).unwrap();
    let block=empty_global_block_after(Some(&fixture.block));
    let mut overlay=state.block(block.header());
    assert!(State::resolve_queue_plan_pending_obligation_in_storage(&mut overlay.world.smart_contract_state,
        fixture.binding.network_id_digest,fixture.binding.entrypoint_hash).unwrap());
    overlay.finalize_lane_consensus_contexts(&block,Some(&context)).unwrap();
    let mut witness=ExecWitness::default();overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.block_hashes.push(block.hash());insert_empty_transaction_block_for_state_commit(&mut overlay,&block);
    overlay.commit().unwrap();state.kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact,receipt)=stage_lane_context_fixture_finality(state,&block,context,witness);
    state.kura.promote_kagemusha_finality_sidecar(&artifact,&receipt).unwrap();
    let closed=state.verified_lane_consensus_contexts().unwrap().unwrap();assert!(closed.contexts().is_empty());
    release_send.send(()).unwrap();let completed=worker.join().unwrap();
    let LaneOpeningAdoption::ObservationChanged {opening,completion}=opening.adopt(state,&observed,completed).unwrap()
        else {panic!("stale observation retains both exact owners");};
    assert!(!guard.restart_required());
    let LaneOpeningAdoption::Closed(drain)=opening.adopt(state,&closed,completion).unwrap()
        else {panic!("authenticated absence closes rather than resumes the recovered signer");};
    let root=state.kura.sumeragi_v2_storage_root().join("wal");
    let before=std::fs::read_dir(&root).unwrap().map(|e|e.unwrap().path())
        .filter(|p|p.extension().is_some_and(|ext|ext=="wal")).map(|p|{let b=std::fs::read(&p).unwrap();(p,b)}).collect::<Vec<_>>();
    assert_eq!(before.len(),2);
    let drained=run_lane_opening_drain_for_test(drain);
    assert_eq!(drained.instance_id(),lane.instance_id());assert_eq!(drained.signer(),0);
    for (path,bytes) in before {assert_eq!(std::fs::read(path).unwrap(),bytes,"closure writes no resumed vote or timeout intent");}
    assert!(!guard.restart_required(),"a fully drained authenticated closure is ordinary ownership transfer");
    let LaneOpeningAdoption::Closed(abandoned_drain)=abandoned.adopt(state,&closed,abandoned_completion).unwrap()
        else {panic!("second exact signer also closes");};
    assert!(!abandoned_guard.restart_required());
    drop(abandoned_drain);
    assert!(abandoned_guard.restart_required(),"discarding a required handle-drain job is not a completed closure");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_opening_error_and_dropped_transit_fence_process_without_live_signer
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::LaneOpeningAdoption};
    use std::time::{Duration,Instant};
    for cut in 0..4 {
        let fixture=finalized_lane_wal_fixture();let state=&fixture.state;
        let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();let lane=&observed.contexts()[0];
        let guard=ConsensusOutputGuard::isolated();let now=Instant::now();let due=now+Duration::from_secs(1);
        let mut other=open_guarded_lane_instance_for_test(&fixture,&observed,lane,1,Arc::clone(&guard),now);
        other.poll_clock(state,&observed,due).unwrap();other.service_with_worker(state,&observed,due).unwrap();other.service_one(state,&observed,due).unwrap();
        assert!(other.held_effects().next().is_some());
        let (opening,job)=prepare_lane_opening_for_test(&fixture,&observed,lane,0,Arc::clone(&guard),now);
        match cut {
            0=>{drop(job);drop(opening);},
            1=>{drop(run_lane_opening_for_test(job));drop(opening);},
            2=>{drop(opening);drop(run_lane_opening_for_test(job));},
            _=>{
                let worker=std::thread::spawn(move ||job.after_wal_open_for_test(||panic!("test opening interruption after physical WAL acquisition")).run());
                assert!(worker.join().is_err());drop(opening);
            },
        }
        assert!(guard.restart_required());
        assert!(other.service_one(state,&observed,due).is_err(),"another signer cannot escape lost opening custody");
        assert!(other.held_effects().next().is_some());
    }
    // Occupied corrupt durable history is a terminal result, not a new empty WAL.
    let fixture=finalized_lane_wal_fixture();let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();let lane=&observed.contexts()[0];
    let now=Instant::now();let owner=open_lane_instance_for_test(&fixture,&observed,lane,0,now);drop(owner);
    let path=std::fs::read_dir(state.kura.sumeragi_v2_storage_root().join("wal")).unwrap()
        .map(|e|e.unwrap().path()).find(|p|p.extension().is_some_and(|ext|ext=="wal")).unwrap();
    let mut corrupt=std::fs::read(&path).unwrap();corrupt[0]^=1;std::fs::write(&path,&corrupt).unwrap();
    let guard=ConsensusOutputGuard::isolated();
    let (opening,job)=prepare_lane_opening_for_test(&fixture,&observed,lane,0,Arc::clone(&guard),now);
    let completed=run_lane_opening_for_test(job);assert!(guard.restart_required(),"physical error fences before completion delivery");
    let LaneOpeningAdoption::Failed {error,drain}=opening.adopt(state,&observed,completed).unwrap()
        else {panic!("corruption cannot mint a dormant or live signer");};
    assert!(!error.to_string().is_empty());
    let drained=run_lane_opening_drain_for_test(drain);assert_eq!(drained.instance_id(),lane.instance_id());
    assert_eq!(std::fs::read(path).unwrap(),corrupt,"opening failure never rewrites occupied foreign/corrupt history");
    assert!(guard.restart_required());
}
