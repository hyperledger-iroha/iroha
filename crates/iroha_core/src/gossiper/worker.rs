//! Sequential physical gossip work with cooperative draining of the original envelope.
use super::{RetainedGossip, TransactionGossip, TransactionGossiperStartError};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use std::{sync::Arc, time::Duration};
use tokio::{runtime::RuntimeFlavor, sync::mpsc};

/// One actor-owned physical step; steps never run concurrently.
pub(super) enum Work {
    /// Perform the next periodic queue fanout.
    Tick,
    /// Validate and admit one bounded incoming message.
    Incoming(RetainedGossip<Arc<TransactionGossip>>),
}

/// Start on a runtime capable of handing off a blocked executor core.
pub(super) fn start<F>(
    period: Duration,
    messages: mpsc::Receiver<RetainedGossip<Arc<TransactionGossip>>>,
    shutdown: ShutdownSignal,
    handle: F,
) -> Result<Child, TransactionGossiperStartError>
where
    F: FnMut(Work) + Send + 'static,
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
        runtime.spawn(run(period, messages, shutdown, handle)),
        OnShutdown::Drain,
    ))
}

async fn run<F>(
    period: Duration,
    mut messages: mpsc::Receiver<RetainedGossip<Arc<TransactionGossip>>>,
    shutdown: ShutdownSignal,
    mut handle: F,
) where
    F: FnMut(Work),
{
    let mut timer = tokio::time::interval(period);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let work = tokio::select! {
            biased;
            () = shutdown.receive() => break,
            work = async {
                tokio::select! {
                    _ = timer.tick() => Work::Tick,
                    Some(message) = messages.recv() => Work::Incoming(message),
                }
            } => work,
        };
        // No detached writer. The same transport credit and envelope survive physical return,
        // then shutdown can retire the original mailbox without waiting on a future G height.
        tokio::task::block_in_place(|| handle(work));
        tokio::task::yield_now().await;
    }
    iroha_logger::debug!("Shutting down transactions gossiper after draining current work");
}

#[cfg(test)]
mod tests;
