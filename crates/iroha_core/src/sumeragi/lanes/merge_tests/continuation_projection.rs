//! Native schedule custody binds authenticated lane expansion and survives overlay rollback.

use super::*;
use crate::{
    block::{BlockValidationError, ValidBlock},
    sumeragi::{
        certified_chain::{
            ParentServicePreparation, SuccessorContextPreparation, admit_parent_service_fixture,
        },
        network_topology::Topology,
        schedule::ScheduleStep,
    },
};
use iroha_data_model::parameter::system::ConsensusMode;

#[test]
fn merged_continuation_uses_expanded_header_and_retains_original_graph_on_retry() {
    let _epoch = crossbeam_epoch::pin();
    let mut fixture = Fixture::start();
    fixture.chain.commit(Vec::new());
    fixture.chain.commit(Vec::new());
    let routed = fixture.log(&lane_user(), "retained expanded schedule", GENESIS_MS - 10);
    fixture.certify(1, 3, vec![routed.clone()]);
    let mut proposal = fixture.chain.proposal(None, Vec::new());
    let original_header = proposal.header();
    assert_eq!(proposal.merged_entrypoint_count(), 0);
    let state = Arc::clone(fixture.chain.state());
    let pool = state.ivm_execution_budget();
    admit_parent_service_fixture(&mut proposal, &pool).unwrap();
    let available = fixture.chain.author_proposal(
        proposal.clone(),
        iroha_sumeragi::types::ControlWitness::empty(),
    );
    let view = state.view();
    let height = original_header.height().get();
    let cadence = Duration::from_millis(
        view.world()
            .consensus_schedule()
            .ready(height)
            .unwrap()
            .params
            .block_time_ms,
    );
    let mut parent_service = ParentServicePreparation::default();
    parent_service.prepare(&view, height - 1).unwrap();
    parent_service
        .prepare_proposal_qc(&view, &proposal)
        .unwrap();
    drop(view);
    let topology = Topology::new(
        fixture
            .chain
            .validators()
            .iter()
            .map(|(peer, _)| peer.clone()),
    );
    let bindings =
        crate::state::asset_uid::binding_custody::retry::BindingPreparations::try_new(&pool)
            .unwrap();
    let controls = crate::execution_attempt::PolicyDecodeControls::try_new(&pool).unwrap();
    let mut successor = SuccessorContextPreparation::default();
    let mut validate = |candidate: SignedBlock| {
        let expansion = expand(
            &state,
            &candidate,
            &*fixture.stores,
            Duration::ZERO,
            &mut crate::sumeragi::lanes::ProposalReadScope::new(),
        )
        .unwrap();
        let mut events = Vec::new();
        let result = ValidBlock::validate_sumeragi_block(
            candidate,
            &topology,
            fixture.chain.genesis_account(),
            cadence,
            ConsensusMode::Permissioned,
            fixture.chain.root_scope(),
            expansion,
            available.header(),
            available.payload().as_slice(),
            &state,
            None,
            &bindings,
            Some(&parent_service),
            &mut successor,
            &controls,
        )
        .unpack(|event| events.push(event));
        (result, events)
    };

    // This first real validation fails before the fix: the retained graph names the
    // unexpanded proposal header, while the pristine World receives the expanded one.
    let (result, _) = validate(proposal.clone());
    let (first, overlay) = result.expect("actual merged inputs must admit their expanded schedule");
    assert_eq!(first.as_ref().merged_entrypoint_count(), 1);
    let expanded_header = first.as_ref().header();
    assert_ne!(expanded_header.merkle_root(), original_header.merkle_root());
    assert_ne!(
        expanded_header.execution_context_hash(),
        original_header.execution_context_hash()
    );
    assert_eq!(
        first.as_ref().canonical_resultless_header().unwrap(),
        original_header
    );
    let ScheduleStep::Done(inputs) = &overlay.sumeragi_schedule else {
        panic!("actual merged execution completes the original schedule");
    };
    let original_committee = inputs.get().schedule.current.committee.as_ptr() as usize;
    let original_schedule = norito::encode_canonical(&inputs.get().schedule).unwrap();
    drop(first);
    drop(overlay);
    assert_eq!(fixture.chain.height(), height - 1);

    // A valid but different original proposal cannot borrow the retained graph under
    // the first native header. Refusal precedes effects and emits no authenticated event.
    let changed = fixture.chain.proposal(
        None,
        vec![fixture.log(&other_user(), "substituted original work", GENESIS_MS - 9)],
    );
    assert_ne!(changed.header(), original_header);
    let (result, events) = validate(changed);
    let (_, error) = result
        .err()
        .expect("original native wire must remain exact");
    assert!(matches!(
        *error,
        BlockValidationError::ExecutionContextInvalid(_)
    ));
    assert!(events.is_empty());

    let (result, _) = validate(proposal.clone());
    let (second, overlay) = result.expect("rollback must return the same expanded schedule graph");
    let ScheduleStep::Done(inputs) = &overlay.sumeragi_schedule else {
        panic!("retried merged execution completes the retained schedule");
    };
    assert_eq!(
        inputs.get().schedule.current.committee.as_ptr() as usize,
        original_committee
    );
    assert_eq!(
        norito::encode_canonical(&inputs.get().schedule).unwrap(),
        original_schedule
    );
    assert_eq!(second.as_ref().header(), expanded_header);
    drop(second);
    drop(overlay);

    // The ordinary worker also authenticates, executes, certifies and publishes this
    // same original lane input; no fabricated execution output establishes success.
    let committed = fixture.chain.commit_proposal(
        proposal,
        crate::sumeragi::test_chain::Signers::Quorum,
        iroha_sumeragi::types::ControlWitness::empty(),
    );
    assert_eq!(committed.height(), height);
    assert_eq!(committed.block().merged_entrypoint_count(), 1);
    assert!(fixture.committed(&routed));
    let input = committed
        .block()
        .network_input_hashes()
        .position(|hash| hash == routed.hash_as_entrypoint())
        .unwrap();
    assert!(
        committed
            .block()
            .network_output_at(u32::try_from(input).unwrap())
            .unwrap()
            .1
            .result
            .0
            .is_ok()
    );
}
