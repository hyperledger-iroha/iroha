//! Cross-family filter comparisons at the event stream boundary.

use super::*;
use crate::{
    ValidationFail,
    block::{
        BlockHeader,
        consensus::{ExecWitness, ExecWitnessMsg},
        error::BlockRejectionReason,
    },
    events::{
        EventBox, EventFilter, SharedDataEvent,
        data::{DataEvent, DataEventFilter, prelude::TriggerEvent},
        execute_trigger::ExecuteTriggerEvent,
        pipeline::{
            BlockEvent, BlockEventFilter, BlockStatus, PipelineEventBox, PipelineWarning,
            TransactionEventFilter, WitnessEventFilter,
        },
        time::{TimeEvent, TimeInterval},
        trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
    },
    executor::{IvmAdmissionError, VectorLengthTooLargeInfo},
    transaction::error::TransactionRejectionReason,
};
use core::time::Duration;

fn baseline_event(status: TransactionStatus) -> TransactionEvent {
    let shared = event(TransactionStatusC10V1::Queued);
    TransactionEvent {
        hash: shared.hash,
        block_height: shared.block_height,
        lane_id: shared.lane_id,
        dataspace_id: shared.dataspace_id,
        status,
    }
}

#[test]
fn transaction_filters_match_shared_statuses_and_keep_rejection_families_distinct() {
    for (baseline, c10) in [
        (TransactionStatus::Queued, TransactionStatusC10V1::Queued),
        (TransactionStatus::Expired, TransactionStatusC10V1::Expired),
        (
            TransactionStatus::Approved,
            TransactionStatusC10V1::Approved,
        ),
    ] {
        let filter: EventFilterBoxC10V1 = TransactionEventFilterC10V1::default()
            .for_hash(tx_hash(1))
            .for_block_height(NonZeroU64::new(7))
            .for_lane_id(LaneId::new(2))
            .for_dataspace_id(DataSpaceId::new(3))
            .for_status(c10.clone())
            .into();
        let baseline_filter = TransactionEventFilter::new()
            .for_hash(tx_hash(1))
            .for_status(baseline.clone())
            .into();
        assert!(
            PipelineEventBoxC10V1::Transaction(event(c10.clone()))
                .matches_baseline_filter(&baseline_filter)
        );
        let mut source = baseline_event(baseline);
        assert!(filter.matches_baseline(&EventBox::Pipeline(source.clone().into())));
        assert!(filter.matches(&event(c10).into()));
        source.lane_id = LaneId::new(9);
        assert!(!filter.matches_baseline(&EventBox::Pipeline(source.into())));
    }
    let baseline = EventBox::Pipeline(
        baseline_event(TransactionStatus::Rejected(Box::new(
            TransactionRejectionReason::Validation(ValidationFail::IvmAdmission(
                IvmAdmissionError::VectorLengthTooLarge(VectorLengthTooLargeInfo {
                    vector_length: 9,
                    max_allowed: 8,
                }),
            )),
        )))
        .into(),
    );
    let all: EventFilterBoxC10V1 = TransactionEventFilterC10V1::default().into();
    assert!(all.matches_baseline(&baseline));
    assert!(!rejected_filter().matches_baseline(&baseline));
    assert!(rejected_filter().matches(&event(c10_rejection()).into()));
    let EventBox::Pipeline(PipelineEventBox::Transaction(source)) = baseline else {
        unreachable!()
    };
    let original = PipelineEventBoxC10V1::Transaction(event(c10_rejection()));
    assert!(original.matches_baseline_filter(&TransactionEventFilter::new().into()));
    let exact_baseline = TransactionEventFilter::new()
        .for_status(source.status)
        .into();
    assert!(!original.matches_baseline_filter(&exact_baseline));
}

#[test]
fn pipeline_batches_match_any_member_in_its_own_family() {
    let filter: EventFilterBoxC10V1 = TransactionEventFilterC10V1::default()
        .for_status(TransactionStatusC10V1::Approved)
        .into();
    let mut c10 = vec![PipelineEventBoxC10V1::Transaction(event(
        TransactionStatusC10V1::Expired,
    ))];
    let mut baseline = vec![baseline_event(TransactionStatus::Expired).into()];
    assert!(!filter.matches(&EventBoxC10V1::PipelineBatch(c10.clone())));
    assert!(!filter.matches_baseline(&EventBox::PipelineBatch(baseline.clone())));
    c10.push(PipelineEventBoxC10V1::Transaction(event(
        TransactionStatusC10V1::Approved,
    )));
    baseline.push(baseline_event(TransactionStatus::Approved).into());
    assert!(filter.matches(&EventBoxC10V1::PipelineBatch(c10)));
    assert!(filter.matches_baseline(&EventBox::PipelineBatch(baseline)));
}

#[test]
fn common_pipeline_filters_preserve_block_wildcards_and_witness_constraints() {
    let height = NonZeroU64::new(7).unwrap();
    let header = BlockHeader::new(height, None, None, 0, 0);
    let block = BlockEvent {
        header,
        status: BlockStatus::Rejected(BlockRejectionReason::EmptyBlock),
    };
    let witness = ExecWitnessMsg {
        block_hash: header.hash(),
        height: height.get(),
        view: 3,
        epoch: 1,
        witness: ExecWitness {
            reads: vec![],
            writes: vec![],
            fastpq_transcripts: vec![],
            fastpq_batches: vec![],
        },
    };
    let warning = PipelineWarning {
        header,
        kind: "test".into(),
        details: "warning".into(),
    };
    let events = [
        (
            PipelineEventBoxC10V1::Block(block.clone()),
            PipelineEventBox::Block(block),
        ),
        (
            PipelineEventBoxC10V1::Witness(witness.clone()),
            PipelineEventBox::Witness(witness),
        ),
        (
            PipelineEventBoxC10V1::Warning(warning.clone()),
            PipelineEventBox::Warning(warning),
        ),
    ];
    let filters = [
        (
            PipelineEventFilterBoxC10V1::Block(
                BlockEventFilter::new()
                    .for_height(height)
                    .for_status(BlockStatus::Rejected(
                        BlockRejectionReason::ConsensusBlockRejection,
                    )),
            ),
            Some(0),
        ),
        (
            PipelineEventFilterBoxC10V1::Block(BlockEventFilter::new().for_status(
                BlockStatus::Rejected(BlockRejectionReason::MerkleRootMismatch),
            )),
            None,
        ),
        (
            PipelineEventFilterBoxC10V1::Witness(WitnessEventFilter::default().for_view(3)),
            Some(1),
        ),
        (
            PipelineEventFilterBoxC10V1::Witness(
                WitnessEventFilter::default().for_height(NonZeroU64::MIN),
            ),
            None,
        ),
    ];
    for (i, (c10, baseline)) in events.into_iter().enumerate() {
        let c10 = EventBoxC10V1::Pipeline(c10);
        let baseline = EventBox::Pipeline(baseline);
        for (filter, expected) in &filters {
            let filter = EventFilterBoxC10V1::Pipeline(filter.clone());
            assert_eq!(filter.matches(&c10), *expected == Some(i));
            assert_eq!(filter.matches_baseline(&baseline), *expected == Some(i));
        }
    }
}

#[test]
fn shared_events_use_their_original_filters_and_time_mintability() {
    let id: crate::trigger::TriggerId = "watch".parse().unwrap();
    let data: SharedDataEvent = DataEvent::Trigger(TriggerEvent::Deleted(id.clone())).into();
    let time = TimeEvent::new(TimeInterval::new(Duration::ZERO, Duration::from_millis(8)));
    let execute = ExecuteTriggerEvent {
        trigger_id: id.clone(),
        authority: account(4),
        args: Default::default(),
    };
    let completed = TriggerCompletedEvent::new(
        id,
        HashOf::from_untyped_unchecked(Hash::new(b"trigger")),
        0,
        TriggerCompletedOutcome::Success,
    );
    let cases = [
        (
            EventFilterBoxC10V1::Data(DataEventFilter::Any),
            EventBoxC10V1::Data(data.clone()),
            EventBox::Data(data),
        ),
        (
            EventFilterBoxC10V1::Time(schedule()),
            EventBoxC10V1::Time(time),
            EventBox::Time(time),
        ),
        (
            EventFilterBoxC10V1::ExecuteTrigger(ExecuteTriggerEventFilter::new()),
            EventBoxC10V1::ExecuteTrigger(execute.clone()),
            EventBox::ExecuteTrigger(execute),
        ),
        (
            EventFilterBoxC10V1::TriggerCompleted(TriggerCompletedEventFilter::new()),
            EventBoxC10V1::TriggerCompleted(completed.clone()),
            EventBox::TriggerCompleted(completed),
        ),
    ];
    for (i, (filter, _, _)) in cases.iter().enumerate() {
        assert_eq!(filter.mintable(), i != 1);
        for (j, (_, c10, baseline)) in cases.iter().enumerate() {
            assert_eq!(filter.matches(c10), i == j);
            assert_eq!(filter.matches_baseline(baseline), i == j);
        }
    }
}
