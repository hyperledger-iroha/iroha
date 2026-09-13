//! Sequential physical work and one retained publication wait, with cooperative draining.
use std::{future::Future, sync::Arc, time::Duration};

use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use tokio::{runtime::RuntimeFlavor, sync::mpsc};

use super::{PendingGossip, RetainedGossip, TransactionGossip, TransactionGossiperStartError};

/// One actor-owned step; no physical step runs concurrently with another.
pub(super) enum Work {
    /// Perform the next periodic queue fanout.
    Tick,
    /// Validate and admit one bounded incoming message.
    Incoming(RetainedGossip<Arc<TransactionGossip>>),
    /// Reclassify the unchanged body under its original transport and time owners.
    Retry(PendingGossip),
}

/// Start the owner on a runtime capable of handing off a blocked executor core.
pub(super) fn start<F, W, Wait>(
    period: Duration,
    messages: mpsc::Receiver<RetainedGossip<Arc<TransactionGossip>>>,
    shutdown: ShutdownSignal,
    wait_for_publication: W,
    handle: F,
) -> Result<Child, TransactionGossiperStartError>
where
    F: FnMut(Work) -> Option<PendingGossip> + Send + 'static,
    W: Fn(u64) -> Wait + Send + Sync + 'static,
    Wait: Future<Output = ()> + Send + 'static,
{
    if period.is_zero() {
        return Err(TransactionGossiperStartError::ZeroPeriod);
    }
    let runtime = tokio::runtime::Handle::try_current()
        .map_err(|_| TransactionGossiperStartError::MissingRuntime)?;
    if runtime.runtime_flavor() != RuntimeFlavor::MultiThread {
        return Err(TransactionGossiperStartError::UnsupportedRuntime);
    }
    Ok(Child::new(
        runtime.spawn(run(
            period,
            messages,
            shutdown,
            wait_for_publication,
            handle,
        )),
        OnShutdown::Drain,
    ))
}

async fn run<F, W, Wait>(
    period: Duration,
    mut messages: mpsc::Receiver<RetainedGossip<Arc<TransactionGossip>>>,
    shutdown: ShutdownSignal,
    wait_for_publication: W,
    mut handle: F,
) where
    F: FnMut(Work) -> Option<PendingGossip>,
    W: Fn(u64) -> Wait,
    Wait: Future<Output = ()>,
{
    let mut timer = tokio::time::interval(period);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut pending: Option<PendingGossip> = None;
    loop {
        let required_height = pending.as_ref().map(|work| work.required_height);
        let deadline = pending
            .as_ref()
            .map_or_else(tokio::time::Instant::now, |work| work.deadline);
        enum Ready {
            Work(Work),
            Retry,
            Expired,
        }
        // A pending batch occupies the existing active slot. The one-slot
        // incoming mailbox remains closed to dequeue until that owner retires.
        // Outgoing ticks remain serviceable while State publication is awaited.
        let ready = tokio::select! {
            biased;
            () = shutdown.receive() => break,
            ready = async {
                tokio::select! {
                    biased;
                    _ = tokio::time::sleep_until(deadline), if required_height.is_some() => Ready::Expired,
                    work = async {
                        tokio::select! {
                            _ = timer.tick() => Ready::Work(Work::Tick),
                            _ = async {
                                match required_height {
                                    Some(height) => wait_for_publication(height).await,
                                    None => std::future::pending::<()>().await,
                                }
                            }, if required_height.is_some() => Ready::Retry,
                            Some(message) = messages.recv(), if required_height.is_none() => Ready::Work(Work::Incoming(message)),
                        }
                    } => work,
                }
            } => ready,
        };
        let work = match ready {
            Ready::Work(work) => work,
            Ready::Retry => Work::Retry(
                pending
                    .take()
                    .expect("enabled publication wait owns its batch"),
            ),
            Ready::Expired => {
                drop(pending.take());
                iroha_logger::debug!(
                    "retained QueuePlan gossip reached its original queue TTL budget"
                );
                continue;
            }
        };
        // No spawned writer: the operation and its transport guard remain on
        // this stack until physical return. Shutdown then drops pending work
        // normally; it never waits indefinitely for an unpublished frontier.
        if let Some(retry) = tokio::task::block_in_place(|| handle(work)) {
            debug_assert!(
                pending.is_none(),
                "a tick cannot replace the active incoming owner"
            );
            pending = Some(retry);
        }
        tokio::task::yield_now().await;
    }
    iroha_logger::debug!("Shutting down transactions gossiper after draining current work");
}

#[cfg(test)]
mod tests;
