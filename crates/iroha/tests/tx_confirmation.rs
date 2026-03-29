//! Tests for transaction confirmation stream behavior

use std::{num::NonZeroU64, time::Duration};

use eyre::Report;
use futures_util::stream;
use iroha::{
    client::listen_for_tx_confirmation_stream,
    crypto::{Hash, HashOf},
    data_model::{events::EventBox, transaction::SignedTransaction},
};
use iroha_data_model::{
    block::BlockHeader,
    events::pipeline::{
        BlockEvent, BlockStatus, PipelineEventBox, TransactionEvent, TransactionStatus,
    },
    nexus::{DataSpaceId, LaneId},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

fn make_hash() -> HashOf<SignedTransaction> {
    HashOf::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]))
}

fn queued_event(hash: HashOf<SignedTransaction>) -> EventBox {
    let event = TransactionEvent {
        hash,
        block_height: None,
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::GLOBAL,
        status: TransactionStatus::Queued,
    };

    EventBox::Pipeline(PipelineEventBox::Transaction(event))
}

fn approved_event(hash: HashOf<SignedTransaction>, height: NonZeroU64) -> EventBox {
    let event = TransactionEvent {
        hash,
        block_height: Some(height),
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::GLOBAL,
        status: TransactionStatus::Approved,
    };

    EventBox::Pipeline(PipelineEventBox::Transaction(event))
}

fn block_event(height: NonZeroU64, status: BlockStatus) -> EventBox {
    let header = BlockHeader {
        height,
        prev_block_hash: None,
        merkle_root: None,
        result_merkle_root: None,
        da_proof_policies_hash: None,
        da_commitments_hash: None,
        da_pin_intents_hash: None,
        prev_roster_evidence_hash: None,
        sccp_commitment_root: None,
        creation_time_ms: 0,
        view_change_index: 0,
        confidential_features: None,
    };
    let event = BlockEvent { header, status };
    EventBox::Pipeline(PipelineEventBox::Block(event))
}

#[test]
fn prolonged_queue_returns_error() {
    let hash = make_hash();
    let event = queued_event(hash);
    let delay = Duration::from_millis(600);
    let (tx, rx) = mpsc::unbounded_channel::<Result<EventBox, Report>>();
    let mut stream = UnboundedReceiverStream::new(rx);
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn({
        let event_clone = event.clone();
        async move {
            tx.send(Ok(event_clone)).unwrap();
            tokio::time::sleep(delay).await;
            tx.send(Ok(event)).unwrap();
        }
    });
    let result = rt.block_on(async {
        listen_for_tx_confirmation_stream(&mut stream, hash, Duration::from_millis(100)).await
    });
    assert!(result.is_err());
}

#[test]
fn duplicate_queue_before_approval_is_tolerated() {
    let hash = make_hash();
    let height = NonZeroU64::new(1).unwrap();
    let events = vec![
        Ok::<_, Report>(queued_event(hash)),
        Ok(queued_event(hash)),
        Ok(approved_event(hash, height)),
        Ok(block_event(height, BlockStatus::Applied)),
    ];
    let mut stream = stream::iter(events);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        listen_for_tx_confirmation_stream(&mut stream, hash, Duration::from_secs(1)).await
    });
    assert_eq!(result.unwrap(), hash);
}

#[test]
fn batched_pipeline_events_confirm_transaction() {
    let hash = make_hash();
    let height = NonZeroU64::new(1).unwrap();
    let batched = EventBox::PipelineBatch(vec![
        PipelineEventBox::Transaction(TransactionEvent {
            hash,
            block_height: None,
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Queued,
        }),
        PipelineEventBox::Transaction(TransactionEvent {
            hash,
            block_height: Some(height),
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::GLOBAL,
            status: TransactionStatus::Approved,
        }),
    ]);
    let events = vec![
        Ok::<_, Report>(batched),
        Ok(block_event(height, BlockStatus::Applied)),
    ];
    let mut stream = stream::iter(events);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        listen_for_tx_confirmation_stream(&mut stream, hash, Duration::from_secs(1)).await
    });
    assert_eq!(result.unwrap(), hash);
}

fn assert_confirmation(block_status: BlockStatus) {
    let hash = make_hash();
    let height = NonZeroU64::new(1).unwrap();
    let events = vec![
        Ok::<_, Report>(queued_event(hash)),
        Ok(approved_event(hash, height)),
        Ok(block_event(height, block_status)),
    ];
    let mut stream = stream::iter(events);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(async {
        listen_for_tx_confirmation_stream(&mut stream, hash, Duration::from_secs(1)).await
    });
    assert_eq!(result.unwrap(), hash);
}

#[test]
fn applied_block_confirms_transaction() {
    assert_confirmation(BlockStatus::Applied);
}

#[test]
fn committed_block_confirms_transaction() {
    assert_confirmation(BlockStatus::Committed);
}
