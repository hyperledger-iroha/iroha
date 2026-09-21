//! Borrowed indexed preparation equals cold replay without claiming native authority or admission.
use super::*;
use std::cell::Cell;

#[derive(Debug, PartialEq, Eq)]
enum LocalRefusal {
    Busy,
    RefundPending,
}
#[derive(Clone, Copy)]
enum ReadFault {
    None,
    Operation,
    SecondOperation,
    Signer,
    Attester,
    MissingOperation,
    MissingSigner,
    MissingAttester,
    SubstitutedOperation,
}
struct Reads<'a> {
    model: &'a TopologyTransitionModelV1,
    fault: ReadFault,
    operations: Cell<usize>,
    keys: Cell<usize>,
}
impl<'a> Reads<'a> {
    fn new(model: &'a TopologyTransitionModelV1, fault: ReadFault) -> Self {
        Self {
            model,
            fault,
            operations: Cell::new(0),
            keys: Cell::new(0),
        }
    }
    fn view(&self) -> TopologyStateViewV1<'_, Self> {
        let original = self.model.view();
        TopologyStateViewV1 {
            deployment: original.deployment,
            network: original.network,
            chain: original.chain,
            chain_discriminant: original.chain_discriminant,
            root: original.root,
            control: original.control,
            state: original.state,
            index: self,
        }
    }
}
impl TopologyIndexedReadV1 for Reads<'_> {
    type Error = LocalRefusal;
    fn operation(&self, id: &[u8; 32]) -> Result<Option<&TopologyOperationRecordV1>, Self::Error> {
        self.operations.set(self.operations.get() + 1);
        match self.fault {
            ReadFault::Operation => Err(LocalRefusal::Busy),
            ReadFault::SecondOperation if self.operations.get() == 2 => {
                Err(LocalRefusal::RefundPending)
            }
            ReadFault::MissingOperation => Ok(None),
            ReadFault::SubstitutedOperation => Ok(self.model.operation(&[10; 32])),
            _ => Ok(self.model.operation(id)),
        }
    }
    fn signer_key_seen(&self, key: &[u8; 32]) -> Result<bool, Self::Error> {
        self.keys.set(self.keys.get() + 1);
        match self.fault {
            ReadFault::Signer => Err(LocalRefusal::RefundPending),
            ReadFault::MissingSigner => Ok(false),
            _ => Ok(self.model.signer_keys.contains(key)),
        }
    }
    fn attester_key_seen(&self, key: &[u8; 32]) -> Result<bool, Self::Error> {
        self.keys.set(self.keys.get() + 1);
        match self.fault {
            ReadFault::Attester => Err(LocalRefusal::Busy),
            ReadFault::MissingAttester => Ok(false),
            _ => Ok(self.model.attester_keys.contains(key)),
        }
    }
}
fn same_plan(f: &mut Fixture, action: TopologyActionV1, height: u64, time: u64, authority: u8) {
    let transition = f.transition(action.clone());
    let context = f.context(height, time, authority);
    let before = f.model.retained().clone();
    let reads = Reads::new(&f.model, ReadFault::None);
    let plan = reads.view().prepare_claimed(&transition, &context).unwrap();
    assert_eq!(
        f.model.retained(),
        &before,
        "preparation never mutates the borrowed owner"
    );
    assert!(
        reads.operations.get() <= 2,
        "no history or operation inventory traversal"
    );
    assert!(
        reads.keys.get() <= 4,
        "only current and proposed key tombstones are read"
    );
    let expected = f.apply(action, height, time, authority).unwrap();
    assert_eq!(plan.into_delta(), expected);
    let bytes = encode(f.model.retained(), TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap();
    assert_eq!(
        decode::<TopologyRetainedStateV1>(&bytes, TOPOLOGY_RECORD_MAX_BYTES_V1).unwrap(),
        *f.model.retained()
    );
}
#[test]
fn borrowed_plans_and_cold_replay_share_every_mutating_action_and_idempotent_result() {
    let mut f = Fixture::new();
    f.model = TopologyTransitionModelV1::new(
        "production-primary".into(),
        [1; 32],
        "topology-chain".into(),
        369,
    )
    .unwrap();
    f.frames.clear();
    let policy = norito::encode_canonical(&f.policy).unwrap();
    same_plan(&mut f, TopologyActionV1::Configure(policy), 1, 100_000, 31);
    let enrollment = f.enrollment(2, 110_000);
    same_plan(&mut f, TopologyActionV1::Enroll(enrollment), 2, 110_000, 31);
    let reviewed = f.reviewed(10, 3, 120_000);
    same_plan(
        &mut f,
        TopologyActionV1::Reserve(Box::new(reviewed.clone())),
        3,
        120_000,
        32,
    );
    same_plan(
        &mut f,
        TopologyActionV1::Reserve(Box::new(reviewed)),
        4,
        122_000,
        32,
    );
    let completion = f.completion(f.model.operation(&[10; 32]).unwrap());
    same_plan(
        &mut f,
        TopologyActionV1::Complete(Box::new(completion)),
        5,
        125_000,
        32,
    );
    same_plan(
        &mut f,
        TopologyActionV1::Complete(Box::new(completion)),
        6,
        126_000,
        32,
    );
    let reviewed = f.reviewed(11, 7, 130_000);
    same_plan(
        &mut f,
        TopologyActionV1::Reserve(Box::new(reviewed)),
        7,
        130_000,
        32,
    );
    let reservation = f.model.operation(&[11; 32]).unwrap().reservation;
    same_plan(
        &mut f,
        TopologyActionV1::Expire(TopologyExpireV1 {
            operation_id: [11; 32],
            reservation,
        }),
        8,
        reservation.expires_at_unix_ms,
        33,
    );
    same_plan(
        &mut f,
        TopologyActionV1::Revoke {
            signer: true,
            attester: false,
        },
        9,
        195_000,
        31,
    );
    let recovered = f.restore(f.frames.clone()).unwrap();
    assert_eq!(recovered.retained(), f.model.retained());
    assert_eq!(
        recovered.operation_inventory().collect::<Vec<_>>(),
        f.model.operation_inventory().collect::<Vec<_>>()
    );
}
#[test]
fn indexed_resource_and_refund_refusals_survive_without_becoming_invalid_transitions() {
    let f = Fixture::new();
    let transition = f.transition(TopologyActionV1::Reserve(Box::new(
        f.reviewed(10, 3, 120_000),
    )));
    let context = f.context(3, 120_000, 32);
    let before = f.model.retained().clone();
    for (fault, expected) in [
        (ReadFault::Operation, LocalRefusal::Busy),
        (ReadFault::Signer, LocalRefusal::RefundPending),
        (ReadFault::Attester, LocalRefusal::Busy),
    ] {
        let reads = Reads::new(&f.model, fault);
        assert_eq!(
            reads.view().prepare_claimed(&transition, &context),
            Err(TopologyPreparationErrorV1::Lookup(expected))
        );
        assert_eq!(f.model.retained(), &before);
    }
}
#[test]
fn missing_current_tombstones_and_active_rows_refuse_even_no_write_current_checks() {
    let mut f = Fixture::new();
    let first = f.reserve(10, 3, 120_000);
    f.apply(
        TopologyActionV1::Complete(Box::new(f.completion(&first))),
        4,
        125_000,
        32,
    )
    .unwrap();
    let row = f.reserve(11, 5, 130_000);
    // The faulty index returns the completed first operation for the distinct
    // active ID. Returning the active row itself is not a substitution.
    assert_ne!(
        first.reviewed.request.operation_id,
        row.reviewed.request.operation_id
    );
    let floor = TopologyFloorClaimV1 {
        height: 5,
        block_hash: [5; 32],
    };
    let transition = f.transition(TopologyActionV1::Check(Box::new(TopologyCheckV1 {
        challenge: [91; 32],
        network_id: [1; 32],
        floor,
        expected_operator: actor(32),
        reviewed: row.reviewed,
        phase: TopologyCheckPhaseV1::Current(Box::new(f.model.audit())),
    })));
    let mut context = f.context(6, 135_000, 33);
    context.floor = Some(floor);
    let reads = Reads::new(&f.model, ReadFault::None);
    assert_eq!(
        reads
            .view()
            .prepare_claimed(&transition, &context)
            .unwrap()
            .into_delta(),
        TopologyTransitionDeltaV1::unchanged()
    );
    for fault in [
        ReadFault::MissingOperation,
        ReadFault::MissingSigner,
        ReadFault::MissingAttester,
        ReadFault::SubstitutedOperation,
    ] {
        let reads = Reads::new(&f.model, fault);
        assert_eq!(
            reads.view().prepare_claimed(&transition, &context),
            Err(TopologyPreparationErrorV1::Transition(Error::History))
        );
    }
}
#[test]
fn retained_summary_and_control_substitution_fail_before_any_indexed_read() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let transition = f.transition(TopologyActionV1::Complete(Box::new(f.completion(&row))));
    let context = f.context(4, 125_000, 32);
    for alter in [
        |r: &mut TopologyRetainedStateV1| r.fence += 1,
        |r: &mut TopologyRetainedStateV1| r.operation_count = 0,
        |r: &mut TopologyRetainedStateV1| r.active = None,
        |r: &mut TopologyRetainedStateV1| r.signer_key_count = 0,
        |r: &mut TopologyRetainedStateV1| r.audit.sequence += 1,
    ] {
        let mut root = f.model.retained().clone();
        alter(&mut root);
        let reads = Reads::new(&f.model, ReadFault::None);
        let mut view = reads.view();
        view.root = &root;
        assert_eq!(
            view.prepare_claimed(&transition, &context),
            Err(TopologyPreparationErrorV1::Transition(Error::History))
        );
        assert_eq!(reads.operations.get() + reads.keys.get(), 0);
    }
    let mut control = f.model.control().unwrap().clone();
    control.control_state[0] ^= 1;
    let reads = Reads::new(&f.model, ReadFault::None);
    let mut view = reads.view();
    view.control = Some(&control);
    assert_eq!(
        view.prepare_claimed(&transition, &context),
        Err(TopologyPreparationErrorV1::Transition(Error::History))
    );
    assert_eq!(reads.operations.get() + reads.keys.get(), 0);
}
#[test]
fn exhausted_permanent_id_capacity_is_rejected_with_constant_indexed_work() {
    let mut f = Fixture::new();
    let reviewed = f.reviewed(10, 3, 120_000);
    // This is an explicit non-authoritative summary-boundary fixture, not a recovered native prefix.
    // The live API cannot enumerate history; native cold recovery must authenticate these counters.
    f.model.root.operation_count = TOPOLOGY_OPERATION_LIMIT_V1;
    f.model.root.fence = TOPOLOGY_OPERATION_LIMIT_V1;
    f.model.root.operation_head = TopologyHeadV1 {
        revision: 2 * TOPOLOGY_OPERATION_LIMIT_V1,
        digest: [98; 32],
    };
    f.model.root.history_head.revision =
        f.model.root.control_head.revision + f.model.root.operation_head.revision;
    let transition = f.transition(TopologyActionV1::Reserve(Box::new(reviewed)));
    let reads = Reads::new(&f.model, ReadFault::None);
    assert_eq!(
        reads
            .view()
            .prepare_claimed(&transition, &f.context(3, 120_000, 32)),
        Err(TopologyPreparationErrorV1::Transition(Error::Capacity))
    );
    assert_eq!(reads.operations.get(), 1);
    assert_eq!(reads.keys.get(), 2);
}
#[test]
fn dropping_a_prepared_delta_preserves_original_owner_for_retry() {
    let f = Fixture::new();
    let transition = f.transition(TopologyActionV1::Reserve(Box::new(
        f.reviewed(10, 3, 120_000),
    )));
    let context = f.context(3, 120_000, 32);
    let original = f.model.retained().clone();
    let first = f
        .model
        .view()
        .prepare_claimed(&transition, &context)
        .unwrap();
    let expected = norito::encode_canonical(first.delta().next.as_ref().unwrap()).unwrap();
    drop(first);
    assert_eq!(f.model.retained(), &original);
    assert!(f.model.operation(&[10; 32]).is_none());
    let retry = f
        .model
        .view()
        .prepare_claimed(&transition, &context)
        .unwrap();
    assert_eq!(
        norito::encode_canonical(retry.delta().next.as_ref().unwrap()).unwrap(),
        expected
    );
}

#[test]
fn substituted_indexed_operation_never_matches_another_active_id() {
    let mut f = Fixture::new();
    let first = f.reserve(10, 3, 120_000);
    f.apply(
        TopologyActionV1::Complete(Box::new(f.completion(&first))),
        4,
        125_000,
        32,
    )
    .unwrap();
    let second = f.reserve(11, 5, 130_000);
    let transition = f.transition(TopologyActionV1::Complete(Box::new(f.completion(&second))));
    let reads = Reads::new(&f.model, ReadFault::SubstitutedOperation);
    assert_eq!(
        reads
            .view()
            .prepare_claimed(&transition, &f.context(6, 135_000, 32)),
        Err(TopologyPreparationErrorV1::Transition(Error::History))
    );
    assert_eq!(f.model.operation(&[11; 32]), Some(&second));
}
#[test]
fn cold_recovery_requires_exact_summary_not_just_matching_terminal_history_hash() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    f.apply(
        TopologyActionV1::Complete(Box::new(f.completion(&row))),
        4,
        125_000,
        32,
    )
    .unwrap();
    for change in [
        |root: &mut TopologyRetainedStateV1| root.fence = 0,
        |root: &mut TopologyRetainedStateV1| root.operation_count = 0,
        |root: &mut TopologyRetainedStateV1| root.signer_key_count += 1,
        |root: &mut TopologyRetainedStateV1| root.attester_key_count += 1,
        |root: &mut TopologyRetainedStateV1| root.audit.digest[0] ^= 1,
        |root: &mut TopologyRetainedStateV1| root.last_execution = None,
    ] {
        let mut expected = f.model.retained().clone();
        change(&mut expected);
        assert_eq!(expected.history_head, f.model.history_head());
        let recovered = TopologyTransitionModelV1::restore_claimed(
            "production-primary".into(),
            [1; 32],
            "topology-chain".into(),
            369,
            &expected,
            f.frames.clone(),
        );
        assert!(matches!(recovered, Err(Error::History)));
    }
}

#[test]
fn indexed_refusal_after_control_planning_preserves_both_custody_and_active_reservation() {
    let mut f = Fixture::new();
    let row = f.reserve(10, 3, 120_000);
    let original = f.model.retained().clone();
    let control = f.model.control().unwrap().clone();
    let transition = f.transition(TopologyActionV1::Revoke {
        signer: true,
        attester: false,
    });
    let reads = Reads::new(&f.model, ReadFault::SecondOperation);
    assert_eq!(
        reads
            .view()
            .prepare_claimed(&transition, &f.context(4, 125_000, 31)),
        Err(TopologyPreparationErrorV1::Lookup(
            LocalRefusal::RefundPending
        ))
    );
    assert_eq!(reads.operations.get(), 2);
    assert_eq!(f.model.retained(), &original);
    assert_eq!(f.model.control(), Some(&control));
    assert_eq!(f.model.operation(&[10; 32]), Some(&row));
}
#[test]
fn mutations_in_one_block_require_one_exact_block_timestamp() {
    let f = Fixture::new();
    let transition = f.transition(TopologyActionV1::Revoke {
        signer: true,
        attester: false,
    });
    let mut context = f.context(2, 110_000, 31);
    context.execution.ordinal = 1;
    assert!(
        f.model
            .view()
            .prepare_claimed(&transition, &context)
            .is_ok()
    );
    context.execution.recorded_at_unix_ms += 1;
    assert_eq!(
        f.model.view().prepare_claimed(&transition, &context),
        Err(TopologyPreparationErrorV1::Transition(Error::Time))
    );
}
