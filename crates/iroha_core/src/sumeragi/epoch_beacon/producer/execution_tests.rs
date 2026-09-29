//! Actual pulse execution, refusal rollback and cold replay over an exact certified prefix.

use super::*;
use crate::sumeragi::{
    block_store::Staging,
    certified_chain::CertifiedChain,
    driver::traits::Executor as _,
    executor::{ExecutorContext, StateExecutor},
    payload,
    test_chain::Signers,
};
use iroha_sumeragi::{api::ExecOutcome, message::Block, preimage::payload_hash};

fn completed() -> (Fixture, ControlWitness) {
    let mut fixture = fixture();
    let source = fixture.chain.state().view();
    let applied = (8, fixture.context.parent_hash);
    let messages = fixture
        .producers
        .iter_mut()
        .map(|producer| ApplicationControl {
            context: fixture.context,
            bytes: producer
                .drive(&source, &fixture.context, applied)
                .unwrap()
                .unwrap(),
        })
        .collect::<Vec<_>>();
    for (key, message) in fixture.keys.iter().zip(&messages) {
        fixture.producers[0]
            .accept(&source, applied, key, message)
            .unwrap();
    }
    let witness = fixture.producers[0]
        .build(&build_context(&fixture.context, 0))
        .unwrap()
        .0;
    drop(source);
    (fixture, witness)
}

fn same_predecessor(fixture: &Fixture, demand: bool) -> CertifiedTestChain {
    let chain = prefix();
    assert_eq!(chain.committed(8).core_hash(), fixture.context.parent_hash);
    assert_eq!(chain.committed(8).result(), fixture.context.parent_result);
    let view = fixture.chain.state().view();
    let (id, record) = view
        .world()
        .global_beacon_key_sessions()
        .iter()
        .next()
        .map(|(id, record)| (*id, record.clone()))
        .unwrap();
    let slot = (BeaconSessionId::for_network_v1(&chain.network_id()), 9);
    let attempts = view
        .world()
        .parliament_required_beacon_pulse_slots()
        .get(&slot)
        .unwrap()
        .clone();
    chain.setup_world_at(2_000, |transaction| {
        transaction
            .world
            .global_beacon_key_sessions
            .insert(id, record);
        transaction
            .world
            .global_beacon_active_session
            .insert(GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, id);
        if demand {
            transaction
                .world
                .parliament_required_beacon_pulse_slots
                .insert(slot, attempts);
        }
    });
    chain
}

fn executor(chain: &CertifiedTestChain) -> StateExecutor {
    StateExecutor::spawn(ExecutorContext {
        state: Arc::clone(chain.state()),
        native_context_archive: Arc::new(
            crate::query::native_context_archive::NativeContextArchive::open(
                chain.state().kura(),
                chain.state().ivm_execution_budget(),
                chain.state().kura().native_context_archive_max_bytes(),
            )
            .expect("original-pool native context archive"),
        ),
        queue: None,
        staging: Staging::new(),
        events: tokio::sync::broadcast::channel(16).0,
        genesis_account: chain.genesis_account().clone(),
        consensus_mode: ConsensusMode::Permissioned,
        applied: (8, chain.committed(8).core_hash()),
        crypto: None,
        applied_watch: Arc::new(crate::sumeragi::lanes::global::AppliedWatch::new(
            8,
            Some(chain.committed(8).block_hash()),
        )),
        lane_blocks: Arc::new(crate::sumeragi::lanes::merge::NoLanes),
    })
    .unwrap()
}

fn invalid(executor: &mut StateExecutor, chain: &CertifiedTestChain, candidate: &Block) {
    let hash = candidate
        .header
        .hash(&crate::sumeragi::crypto::BlsCrypto::new());
    assert!(matches!(
        executor.execute(candidate, &hash),
        Some(ExecOutcome::Invalid(_))
    ));
    let view = chain.state().view();
    assert_eq!(view.height(), 8);
    assert!(view.world().global_beacon_pulses().is_empty());
    assert!(view.world().global_beacon_pulse_slots().is_empty());
}

#[test]
fn transported_pulse_executes_once_and_cold_replay_reproduces_the_certified_result() {
    let (mut fixture, witness) = completed();
    let replay = same_predecessor(&fixture, true);
    let pulse = control::decode(&witness).unwrap().unwrap();
    fixture
        .chain
        .commit_with_control(Some(50_000), Vec::new(), Signers::Quorum, witness);
    let stored = fixture.chain.committed(9);
    assert!(stored.block().has_consensus_work());
    assert!(stored.block().global_beacon_pulse().is_none());
    assert_eq!(stored.commitment().beacon, Some(pulse));
    assert_eq!(
        control::decode(&stored.header().unwrap().control_witness).unwrap(),
        Some(pulse)
    );
    let view = fixture.chain.state().view();
    let certified = CertifiedChain::new(&view).unwrap().certified(9).unwrap();
    let qc = certified.commit_qc().unwrap().clone();
    drop(view);
    let proposal = stored.block().canonical_resultless_proposal();
    let block = Block {
        header: stored.header().unwrap().clone(),
        payload: payload::encode(&proposal).unwrap(),
    };
    let mut worker = executor(&replay);
    // A correctly decoded control witness is still obligatory at this exact source.
    let mut missing = block.clone();
    missing.header.control_witness = ControlWitness::empty();
    invalid(&mut worker, &replay, &missing);
    replay.kura().store_block(stored.block().clone()).unwrap();
    worker
        .replay(&block, &qc)
        .expect("cold executor reproduces the original certified pulse writes");
    // Repeated completion is idempotent; no pulse or event is applied a second time.
    worker
        .replay(&block, &qc)
        .expect("completed publication is idempotent");
    let view = replay.state().view();
    assert_eq!(view.height(), 9);
    assert_eq!(
        view.world().global_beacon_pulses().get(&pulse.pulse_id),
        Some(&pulse)
    );
    assert_eq!(view.world().global_beacon_pulse_slots().len(), 1);
    assert_eq!(
        crate::sumeragi::certified_chain::committed_block(&view, 9)
            .unwrap()
            .result(),
        stored.result()
    );
}

#[test]
fn native_pulse_refusals_preserve_the_exact_predecessor_and_require_actual_work() {
    let (mut fixture, witness) = completed();
    let predecessor = same_predecessor(&fixture, true);
    let unrequested = same_predecessor(&fixture, false);
    let pulse = control::decode(&witness).unwrap().unwrap();
    fixture
        .chain
        .commit_with_control(Some(50_000), Vec::new(), Signers::Quorum, witness);
    let stored = fixture.chain.committed(9);
    let proposal = stored.block().canonical_resultless_proposal();
    let block = Block {
        header: stored.header().unwrap().clone(),
        payload: payload::encode(&proposal).unwrap(),
    };
    let mut worker = executor(&predecessor);
    for field in 0..15 {
        let mut wrong = pulse;
        match field {
            0 => wrong.signature[0] ^= 1,
            1 => wrong.session_id[0] ^= 1,
            2 => wrong.roster_hash[0] ^= 1,
            3 => wrong.transcript_hash[0] ^= 1,
            4 => wrong.height += 1,
            5 => wrong.round += 1,
            6 => {
                wrong.finalized_chain_anchor.block_hash =
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign parent"))
            }
            7 => wrong.seed[0] ^= 1,
            8 => wrong.pulse_id[0] ^= 1,
            9 => {
                wrong.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"foreign genesis")),
                )
            }
            10 => wrong.context.instance[0] ^= 1,
            11 => wrong.context.epoch += 1,
            12 => wrong.context.epoch_context_id[0] ^= 1,
            13 => wrong.context.parent_consensus_hash[0] ^= 1,
            _ => wrong.context.parent_result[0] ^= 1,
        }
        let mut candidate = block.clone();
        candidate.header.control_witness = control::encode(Some(wrong)).unwrap();
        invalid(&mut worker, &predecessor, &candidate);
    }
    let mut duplicate = proposal.clone();
    duplicate.set_global_beacon_pulse(Some(pulse));
    let mut candidate = block.clone();
    candidate.payload = payload::encode(&duplicate).unwrap();
    candidate.header.payload_len = u32::try_from(candidate.payload.len()).unwrap();
    candidate.header.payload_hash = payload_hash(
        &crate::sumeragi::crypto::BlsCrypto::new(),
        &candidate.payload,
    );
    invalid(&mut worker, &predecessor, &candidate);
    let mut empty = proposal;
    empty.set_external_entrypoints(Vec::new());
    empty.set_execution_context(None);
    assert!(
        payload::encode(&empty).is_err(),
        "control cannot substitute transaction work"
    );
    invalid(&mut executor(&unrequested), &unrequested, &block);
}
