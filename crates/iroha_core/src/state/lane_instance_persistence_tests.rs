// Physical WAL transfer controls around authentic State/Kura lane instances.
// No production scheduler, worker pool, transport or economic Apply claim.

fn take_native_persistence_job_for_test(
    owner: &mut crate::sumeragi::v2_lane_instance::LaneInstance,
) -> crate::sumeragi::v2_lane_instance::LanePersistenceJob {
    use crate::sumeragi::v2_lane_instance::LanePersistenceLaunch;
    let LanePersistenceLaunch::Job(job) = owner.take_persistence_job().unwrap() else {
        panic!("exact issued Persist owns a physical worker job");
    };
    job
}

fn run_native_persistence_job_for_test(
    job: crate::sumeragi::v2_lane_instance::LanePersistenceJob,
) -> crate::sumeragi::v2_lane_instance::LanePersistenceCompletion {
    std::thread::scope(|scope| scope.spawn(move || job.run()).join().unwrap())
}

fn open_guarded_lane_instance_for_test(
    fixture: &LaneContextVerifiedFixture,
    observed: &VerifiedLaneContexts,
    lane: &VerifiedLaneContext,
    signer: usize,
    guard: Arc<crate::sumeragi::output_guard::ConsensusOutputGuard>,
    now: std::time::Instant,
) -> crate::sumeragi::v2_lane_instance::LaneInstance {
    use crate::sumeragi::{v2_core, v2_lane_instance::LaneInstance};
    let key = fixture
        .validators
        .iter()
        .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
        .unwrap()
        .clone();
    LaneInstance::open_with_worker_for_test(
        &fixture.state,
        observed,
        lane,
        key,
        guard,
        now,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_millis(100),
        3 * v2_core::MAX_EFFECTS_PER_STEP,
    )
    .unwrap()
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_persistence_worker_keeps_custody_without_blocking_another_lane
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneInputOutcome, LanePersistenceLaunch, LanePersistenceWait, LaneService}, v2_lane_wire::LaneWalRecordV1};
    use std::{sync::mpsc, time::{Duration, Instant}};
    let fixture=all_route_input_fixture(false);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(observed.contexts().len(),2);
    let start=Instant::now();
    let due=start+Duration::from_secs(1);
    let mut blocked=open_lane_instance_for_test(&fixture,&observed,&observed.contexts()[0],0,start);
    let mut other=open_lane_instance_for_test(&fixture,&observed,&observed.contexts()[1],0,start);
    assert!(matches!(blocked.take_persistence_job().unwrap(),LanePersistenceLaunch::Wait(LanePersistenceWait::NoWork)));
    blocked.poll_clock(state,&observed,due).unwrap();
    let effect=blocked.held_effects().find(|effect| matches!(effect,core::Effect::Persist {..})).unwrap().clone();
    assert!(matches!(blocked.service_one(state,&observed,due).unwrap(),LaneService::NeedsPersistenceWorker));
    assert!(blocked.native_records().is_empty(),"control service performed no append");
    let job=take_native_persistence_job_for_test(&mut blocked);
    assert_eq!(blocked.persistence_in_flight(),Some(&effect));
    assert!(blocked.held_effects().next().is_none(),"the ticket now owns the original effect");
    assert!(matches!(blocked.take_persistence_job().unwrap(),LanePersistenceLaunch::Wait(LanePersistenceWait::WorkerInFlight)));
    let (started_send,started_recv)=mpsc::sync_channel(0);
    let (release_send,release_recv)=mpsc::sync_channel(0);
    let worker=std::thread::spawn(move || {
        started_send.send(()).unwrap();
        release_recv.recv().unwrap();
        job.run()
    });
    started_recv.recv().unwrap(); // The actual handle is on another thread.
    assert!(matches!(blocked.service_one(state,&observed,due).unwrap(),LaneService::PersistenceInFlight));
    assert!(blocked.native_records().is_empty());
    assert!(matches!(other.poll_clock(state,&observed,due).unwrap(),LaneInputOutcome::Stepped(_)));
    assert!(matches!(other.service_with_worker(state,&observed,due).unwrap(),LaneService::PersistedAwaitingAck));
    assert!(matches!(other.service_one(state,&observed,due).unwrap(),LaneService::Completion(_)));
    assert!(matches!(other.service_one(state,&observed,due).unwrap(),LaneService::SignedAwaitingAck),
        "another lane can fsync, acknowledge and sign while the first physical worker is held");
    assert_eq!(blocked.persistence_in_flight(),Some(&effect));
    release_send.send(()).unwrap();
    let completed=worker.join().unwrap();
    assert!(blocked.native_records().is_empty(),"result transit is still ticket-owned");
    assert!(matches!(blocked.finish_persistence_job(completed).unwrap(),LaneService::PersistedAwaitingAck));
    assert!(blocked.persistence_in_flight().is_none());
    assert!(matches!(blocked.take_persistence_job().unwrap(),LanePersistenceLaunch::Wait(LanePersistenceWait::ControlCompletion)));
    assert!(matches!(&blocked.native_records()[0].record,LaneWalRecordV1::TimeoutIntent {body,..} if body.highest_prepare.is_none()));
    assert!(blocked.held_effects().next().is_none(),"finishing the worker does not itself acknowledge the reducer");
    assert!(matches!(blocked.service_one(state,&observed,due).unwrap(),LaneService::Completion(_)));
    assert!(matches!(blocked.held_effects().collect::<Vec<_>>().as_slice(),[core::Effect::Sign {message:core::SignableMessage::TimeoutVote(_),..}]));
    assert!(matches!(blocked.service_one(state,&observed,due).unwrap(),LaneService::SignedAwaitingAck));
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_persistence_foreign_completion_returns_intact_and_replays_before_ack
    use crate::sumeragi::{v2_core as core,v2_lane_instance::LaneService};
    use std::time::{Duration,Instant};
    let fixture=finalized_lane_wal_fixture();
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let now=Instant::now();
    let due=now+Duration::from_secs(1);
    let mut left=open_lane_instance_for_test(&fixture,&observed,lane,0,now);
    let mut right=open_lane_instance_for_test(&fixture,&observed,lane,1,now);
    left.poll_clock(state,&observed,due).unwrap();
    right.poll_clock(state,&observed,due).unwrap();
    let left_job=take_native_persistence_job_for_test(&mut left);
    let right_job=take_native_persistence_job_for_test(&mut right);
    let left_effect=left.persistence_in_flight().unwrap().clone();
    let right_effect=right.persistence_in_flight().unwrap().clone();
    let left_result=run_native_persistence_job_for_test(left_job);
    let right_result=run_native_persistence_job_for_test(right_job);
    let (error,left_result)=right.finish_persistence_job(left_result).unwrap_err();
    assert!(error.to_string().contains("foreign native persistence completion"));
    assert_eq!(left.persistence_in_flight(),Some(&left_effect));
    assert_eq!(right.persistence_in_flight(),Some(&right_effect));
    assert!(left.native_records().is_empty() && right.native_records().is_empty());
    assert!(matches!(left.finish_persistence_job(left_result).unwrap(),LaneService::PersistedAwaitingAck));
    assert!(matches!(right.finish_persistence_job(right_result).unwrap(),LaneService::PersistedAwaitingAck));
    let exact=left.native_records().to_vec();
    assert_eq!(exact.len(),1);
    assert!(left.held_effects().next().is_none());
    drop(left); // Actual fsync completed; the exact Persisted event was never stepped.
    let mut reopened=open_lane_instance_for_test(&fixture,&observed,lane,0,due);
    assert_eq!(reopened.native_records(),exact,"recovery retains full exact native evidence");
    assert!(reopened.persistence_in_flight().is_none());
    assert!(matches!(reopened.held_effects().collect::<Vec<_>>().as_slice(),[core::Effect::Sign {message:core::SignableMessage::TimeoutVote(_),..}]));
    assert!(matches!(reopened.service_one(state,&observed,due).unwrap(),LaneService::SignedAwaitingAck));
    assert!(matches!(right.service_one(state,&observed,due).unwrap(),LaneService::Completion(_)));
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_persistence_completion_restores_physical_owner_after_authenticated_closure
    use crate::sumeragi::{v2_core as core,v2_lane_instance::{LaneCurrentGate,LaneInputOutcome,LaneService}};
    use std::time::{Duration,Instant};
    let (fixture,_)=first_lane_input_fixture(0xB1);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let now=Instant::now();
    let due=now+Duration::from_secs(1);
    let mut owner=open_lane_instance_for_test(&fixture,&observed,lane,0,now);
    owner.poll_clock(state,&observed,due).unwrap();
    let job=take_native_persistence_job_for_test(&mut owner);
    let completed=run_native_persistence_job_for_test(job);
    let parent=state.kura.v2_finality_artifact(fixture.block.header().height().get()).unwrap().unwrap();
    let opening=crate::sumeragi::v2_context::build_successor_height_context(&parent,parent.height_context.nexus_amx_context_hash,None).unwrap();
    let block=empty_global_block_after(Some(&fixture.block));
    let mut overlay=state.block(block.header());
    assert!(State::resolve_queue_plan_pending_obligation_in_storage(&mut overlay.world.smart_contract_state,fixture.binding.network_id_digest,fixture.binding.entrypoint_hash).unwrap());
    overlay.finalize_lane_consensus_contexts(&block,Some(&opening)).unwrap();
    let mut witness=ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay,&block);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact,receipt)=stage_lane_context_fixture_finality(state,&block,opening,witness);
    state.kura.promote_kagemusha_finality_sidecar(&artifact,&receipt).unwrap();
    let closed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    assert!(matches!(owner.finish_persistence_job(completed).unwrap(),LaneService::PersistedAwaitingAck),
        "closure cannot strand the real physical handle or fsync receipt");
    assert!(owner.persistence_in_flight().is_none());
    assert_eq!(owner.native_records().len(),1);
    assert!(matches!(owner.service_one(state,&closed,due).unwrap(),LaneService::Completion(_)),
        "physical acknowledgement drains without granting a productive lease");
    assert!(matches!(owner.held_effects().collect::<Vec<_>>().as_slice(),[core::Effect::Sign {..}]));
    assert!(matches!(owner.service_one(state,&closed,due).unwrap(),LaneService::Gate(LaneCurrentGate::InstanceClosed)));
    assert!(matches!(owner.poll_clock(state,&closed,due).unwrap(),LaneInputOutcome::Gate(LaneCurrentGate::InstanceClosed)));
    let (sender,receiver)=std::sync::mpsc::sync_channel(1);
    assert!(matches!(owner.flush_one(state,&closed,&sender).unwrap(),LaneService::Idle));
    assert!(receiver.try_recv().is_err(),"no stale signature or output was produced");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_persistence_dropped_job_or_result_fences_shared_output
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,v2_lane_instance::LaneService};
    use std::time::{Duration,Instant};
    for dropped_owner in 0..3 {
        let fixture=finalized_lane_wal_fixture();
        let state=&fixture.state;
        let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
        let lane=&observed.contexts()[0];
        let guard=ConsensusOutputGuard::isolated();
        let now=Instant::now();
        let due=now+Duration::from_secs(1);
        let mut owner=open_guarded_lane_instance_for_test(&fixture,&observed,lane,0,Arc::clone(&guard),now);
        let mut other=open_guarded_lane_instance_for_test(&fixture,&observed,lane,1,Arc::clone(&guard),now);
        // A different actual signer already has a durable but unsigned intent.
        other.poll_clock(state,&observed,due).unwrap();
        other.service_with_worker(state,&observed,due).unwrap();
        other.service_one(state,&observed,due).unwrap();
        owner.poll_clock(state,&observed,due).unwrap();
        let job=take_native_persistence_job_for_test(&mut owner);
        let exact=owner.persistence_in_flight().unwrap().clone();
        match dropped_owner {
            0 => drop(job),
            1 => drop(run_native_persistence_job_for_test(job)),
            _ => {
                drop(owner);
                assert!(guard.restart_required(),"dropping the control owner cannot orphan a live physical job");
                drop(run_native_persistence_job_for_test(job));
                assert!(other.service_one(state,&observed,due).is_err());
                continue;
            },
        }
        assert!(guard.restart_required(),"dropping any armed transit owner closes the process output fence");
        assert_eq!(owner.persistence_in_flight(),Some(&exact));
        assert!(owner.native_records().is_empty(),"no dropped result is fabricated into an acknowledgement");
        assert!(matches!(owner.service_one(state,&observed,due).unwrap(),LaneService::PersistenceInFlight));
        assert!(other.service_one(state,&observed,due).is_err(),"the shared fence prevents another signer from producing output");
        assert!(other.held_effects().next().is_some(),"refused signing retains its exact effect");
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_persistence_physical_leaf_substitution_is_terminal_with_exact_custody
    use crate::sumeragi::output_guard::ConsensusOutputGuard;
    use std::time::{Duration,Instant};
    let fixture=finalized_lane_wal_fixture();
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let guard=ConsensusOutputGuard::isolated();
    let now=Instant::now();
    let due=now+Duration::from_secs(1);
    let mut owner=open_guarded_lane_instance_for_test(&fixture,&observed,lane,0,Arc::clone(&guard),now);
    owner.poll_clock(state,&observed,due).unwrap();
    let job=take_native_persistence_job_for_test(&mut owner);
    let exact=owner.persistence_in_flight().unwrap().clone();
    let paths=std::fs::read_dir(state.kura.sumeragi_v2_storage_root().join("wal")).unwrap()
        .map(|entry|entry.unwrap().path()).filter(|path|path.extension().is_some_and(|extension|extension=="wal"))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(),1);
    let path=&paths[0];
    let original=std::fs::read(path).unwrap();
    let displaced=path.with_extension("displaced");
    std::fs::rename(path,&displaced).unwrap();
    std::fs::write(path,&original).unwrap(); // Same bytes, different physical identity.
    let completed=run_native_persistence_job_for_test(job);
    assert!(guard.restart_required(),"a known fatal append closes output before completion delivery");
    let (_,completed)=owner.finish_persistence_job(completed).unwrap_err();
    drop(completed); // Matching failure already returned the sole handle to owner.
    assert!(guard.restart_required());
    assert_eq!(owner.persistence_in_flight(),Some(&exact),"failed issued evidence is retained for diagnosis");
    assert!(owner.native_records().is_empty());
    assert_eq!(std::fs::read(path).unwrap(),original);
    assert_eq!(std::fs::read(&displaced).unwrap(),original,"descriptor binding rejected before writing either leaf");
    assert!(owner.take_persistence_job().is_err(),"physical uncertainty cannot become an ordinary retry");
    assert!(owner.service_one(state,&observed,due).is_err());
}
