// Profile-specific transport negotiated on the existing authenticated events route.

use iroha_data_model::{
    block::{
        SharedSignedBlock,
        execution_output::{ExecutionOutputV1, ExecutionSourceC10V1},
    },
    ivm_c10::{
        TransactionEventC10V1, TransactionStatusC10V1,
        events::{EventMessageC10V1, EventSubscriptionC10V1},
    },
};
use tokio::sync::broadcast;

pub(crate) enum EventReceiver {
    Baseline(broadcast::Receiver<iroha_data_model::events::EventBox>),
    C10(broadcast::Receiver<SharedSignedBlock>),
}

pub(crate) async fn handle_negotiated_event_stream(
    receiver: EventReceiver,
    socket: WebSocket,
    timeout: std::time::Duration,
    visibility: ToriiDataspaceReadContext,
) -> eyre::Result<()> {
    match receiver {
        EventReceiver::Baseline(receiver) => {
            handle_events_stream_with_receiver_visible(receiver, socket, timeout, visibility).await
        }
        EventReceiver::C10(mut blocks) => {
            let mut stream = WebSocketNorito::new(socket, timeout);
            let mut authorization =
                crate::stream_read_authorization::ToriiStreamReadAuthorization::new(visibility);
            match stream_c10_transactions(&mut blocks, &mut stream, &mut authorization).await {
                Ok(()) => stream.close().await.map_err(Into::into),
                Err(Error::Close) => Ok(()),
                Err(error) => {
                    let (code, reason) = close_frame_for_error(&error);
                    let _ = stream.close_with(code, reason).await;
                    Err(error.into())
                }
            }
        }
    }
}

/// Join the original certified result to its signed input and authenticated routing context.
fn published_c10_transaction(
    block: &iroha_data_model::block::SignedBlock,
    index: u32,
) -> Option<TransactionEventC10V1> {
    let i = usize::try_from(index).ok()?;
    let (_, ExecutionOutputV1::C10(row)) = block.network_execution_output_at(index)? else {
        return None;
    };
    let route = block.execution_context()?.external.get(i)?;
    let input = block.network_entrypoint_at(i)?;
    if route.entrypoint_hash != input.hash() {
        return None;
    }
    let signed = block.external_signed_transaction_ref_at(i)?;
    Some(TransactionEventC10V1 {
        hash: signed.hash(),
        block_height: Some(block.header().height()),
        lane_id: route.lane_id,
        dataspace_id: route.dataspace_id,
        status: match &row.result.0 {
            Ok(_) => TransactionStatusC10V1::Approved,
            Err(reason) => TransactionStatusC10V1::Rejected(Box::new(reason.clone())),
        },
    })
}

async fn stream_c10_transactions(
    blocks: &mut broadcast::Receiver<SharedSignedBlock>,
    stream: &mut WebSocketNorito,
    authorization: &mut crate::stream_read_authorization::ToriiStreamReadAuthorization,
) -> Result<()> {
    use crate::stream_read_authorization::{BlockTransactionScopes, StreamAuthorization as _};
    let request = stream
        .recv::<EventSubscriptionC10V1>()
        .await
        .map_err(event::Error::from)?;
    if request.filters.is_empty() {
        return Err(event::Error::InvalidSubscription(
            "at least one C10 transaction filter is required".into(),
        )
        .into());
    }
    let mut heartbeat = tokio::time::interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    heartbeat.tick().await;
    let mut authorization_check = tokio::time::interval(STREAM_AUTHORIZATION_CHECK_INTERVAL);
    authorization_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            closed = stream.closed() => {
                closed.map_err(event::Error::from)?;
                return Err(Error::Close);
            }
            block = blocks.recv() => {
                let block = block?;
                let height = usize::try_from(block.header().height().get()).ok()
                    .and_then(core::num::NonZeroUsize::new);
                let Some(height) = height else { continue };
                let scopes = BlockTransactionScopes::from_block(height, &block);
                for output in block.execution_outputs() {
                    let ExecutionOutputV1::C10(row) = output else { continue };
                    let ExecutionSourceC10V1::Network(index) = row.source else { continue };
                    if !authorization.authorization_is_current() {
                        return Err(Error::AuthorizationRevoked);
                    }
                    if let Some(tx) = published_c10_transaction(&block, index)
                        && scopes.scope(&tx.hash).allows(authorization.visibility())
                        && request.filters.iter().any(|filter| filter.matches(&tx))
                    {
                        stream.send(EventMessageC10V1(tx.into())).await.map_err(event::Error::from)?;
                    }
                }
            }
            _ = heartbeat.tick() => {
                stream.ping().await.map_err(event::Error::from)?;
            }
            _ = authorization_check.tick() => {
                if !authorization.revalidate() {
                    return Err(Error::AuthorizationRevoked);
                }
            }
        }
    }
}
