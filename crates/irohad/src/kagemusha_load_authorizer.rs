//! Optional supervised online publisher. Original finalized issuance authorizes signing;
//! normal queue admission authorizes submission. Neither queue success nor timeout is finality.
use std::sync::Arc;

use iroha_config::parameters::actual::KagemushaLoadAuthorizer;
use iroha_core::{
    kagemusha_wallet_v1::{FinalizedLedger, PublicationWorker},
    queue::Queue,
    state::State,
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{
    account::AccountId,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
#[cfg(test)]
mod tests;

/// This worker never has access to provider receipt, enrollment, payment or arbitrary signing.
pub(crate) struct Service {
    worker: PublicationWorker,
    submitter: KeyPair,
    config: KagemushaLoadAuthorizer,
    state: Arc<State>,
    queue: Arc<Queue>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickError {
    SourceUnavailable,
    SubmissionUnavailable,
}
impl Service {
    /// Preflight before supervision; malformed custody fails startup rather than disabling an
    /// explicitly configured monetary service. No secret contents are returned in the error.
    pub(crate) fn new(
        mut config: KagemushaLoadAuthorizer,
        state: Arc<State>,
        queue: Arc<Queue>,
    ) -> Result<Option<Self>, &'static str> {
        let Some(custody) = config.custody.take() else {
            return Ok(None);
        };
        if config.finality_limits.validate().is_err()
            || !(1..=iroha_core::kagemusha_wallet_v1::MAX_PENDING_PAGE).contains(&config.page_size)
            || config.poll_interval.is_zero()
            || config.transaction_ttl.is_zero()
            || FeePaymentIntent::authority(config.charge_limits.clone(), None)
                .validate()
                .is_err()
        {
            return Err("invalid KAGEMUSHA publisher limits");
        }
        let worker = PublicationWorker::from_canonical_keyring(&custody.keyring)
            .map_err(|_| "invalid KAGEMUSHA publisher custody binding")?;
        worker
            .require_network(*state.network_id_ref().as_bytes())
            .map_err(|_| "KAGEMUSHA publisher keys belong to another network")?;
        Ok(Some(Self {
            worker,
            submitter: custody.submitter,
            config,
            state,
            queue,
        }))
    }

    fn tick(&mut self) -> Result<usize, TickError> {
        let view = self.state.view();
        let prepared = norito::core::with_decode_limits_scope(
            self.config
                .finality_limits
                .decode_limits()
                .map_err(|_| TickError::SourceUnavailable)?,
            || {
                let source = FinalizedLedger::new(&view, self.config.finality_limits)
                    .map_err(|_| TickError::SourceUnavailable)?;
                self.worker
                    .prepare_page(&source, self.config.page_size)
                    .map_err(|_| TickError::SourceUnavailable)
            },
        )?;
        let (drift, limits) = self.state.transaction_admission_limits();
        let crypto = self.state.crypto();
        let count = prepared.len();
        let mut unavailable = false;
        for publication in prepared {
            let command = publication
                .into_publication_instruction()
                .map_err(|_| TickError::SourceUnavailable)?;
            let mut builder = TransactionBuilder::new(
                *self.state.network_id_ref(),
                AccountId::new(self.submitter.public_key().clone()),
                FeePaymentIntent::authority(self.config.charge_limits.clone(), None),
            )
            .with_instructions([command]);
            builder.set_ttl(self.config.transaction_ttl);
            let transaction = builder
                .try_sign(self.submitter.private_key())
                .map_err(|_| TickError::SubmissionUnavailable)?;
            let accepted = AcceptedTransaction::accept(
                transaction,
                self.state.network_id_ref(),
                drift,
                limits,
                &crypto,
            )
            .map_err(|_| TickError::SubmissionUnavailable)?;
            // Unknown/full/duplicate queue outcomes leave the authenticated pending row intact.
            // Next visits reread the original finalized source, including a publication made by
            // another worker. Original account permissions and online fees remain enforced.
            unavailable |= self.queue.push_in_view(accepted, &view).is_err();
        }
        if unavailable {
            Err(TickError::SubmissionUnavailable)
        } else {
            Ok(count)
        }
    }

    pub(crate) fn start(self, shutdown: ShutdownSignal) -> Child {
        let task = tokio::spawn(async move {
            let mut worker = self;
            let mut interval = tokio::time::interval(worker.config.poll_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut last_error = None;
            loop {
                tokio::select! {
                    biased;
                    () = shutdown.receive() => break,
                    _ = interval.tick() => {}
                }
                // One admitted finite tick owns the worker until it returns. Shutdown does not
                // cancel a queue mutation or mistake a lost response for failed publication.
                let joined = tokio::task::spawn_blocking(move || {
                    let result = worker.tick();
                    (worker, result)
                })
                .await;
                let (returned, result) = match joined {
                    Ok(result) => result,
                    Err(_) => panic!("KAGEMUSHA publisher worker exited unexpectedly"),
                };
                worker = returned;
                let error = result.err();
                if error != last_error {
                    if let Some(error) = error {
                        iroha_logger::warn!(
                            ?error,
                            "KAGEMUSHA publisher retained pending ledger work for retry"
                        );
                    }
                    last_error = error;
                }
            }
        });
        Child::new(task, OnShutdown::Drain)
    }
}
