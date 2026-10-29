//! Retained authority for current attestations.
//!
//! Without it, each current attestation re-reads every native frame from its target
//! down to H2 and genesis. A node-local producer instead mints this authority with
//! one such complete walk, extends it through each newly published durable tip, and
//! keeps re-reading every retained frame in paced bounded chunks. A request then
//! reads only its target and parent, and checks its genesis frame against the
//! retained decision. Corruption of an older local frame is therefore detected within
//! [`MAX_COVERAGE_AGE`] rather than on the next challenge. A contradiction poisons the
//! authority until restart; an unminted, lagging or expired authority refuses.
//! Without an attached producer the complete walk is unchanged.

use std::{
    num::NonZeroUsize,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, consensus::SumeragiRootScope},
    query::error::QueryExecutionFail,
};

use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};

use super::{GenesisDecision, ProofError};
use crate::{
    execution_attempt::ExecutionAttemptError,
    kura::Kura,
    snapshot::StartupRecovery,
    state::{AuthenticatedExecutionIdentity, NativeExecutionTip, State, StateReadOnly},
    sumeragi::certified_chain::CertifiedChain,
};

/// Heights a request's view may lead the published tip; the producer closes the gap.
const MAX_LEAD: u64 = 16;
/// Verified heights kept up to the published tip, for views that trail a fresh advance.
const RECENT: u64 = 64;
/// A tip that moved further than this since the last advance is minted again.
const MAX_ADVANCE: u64 = 1_024;
/// Every retained frame must have been re-read within this age, or attestations refuse.
pub(crate) const MAX_COVERAGE_AGE: Duration = Duration::from_secs(30 * 60);
/// Time between the starts of consecutive scrub passes.
const SCRUB_INTERVAL: Duration = Duration::from_secs(10 * 60);
/// Frames one scrub step reads.
const SCRUB_CHUNK: u64 = 128;
/// Pause after each scrub step, bounding a pass to a fraction of one core.
const SCRUB_PAUSE: Duration = Duration::from_millis(250);
/// Pause before minting again after a refused complete walk.
const MINT_RETRY: Duration = Duration::from_secs(5);
/// Pause between producer steps when no work is pending.
const IDLE: Duration = Duration::from_millis(50);
/// Producer steps per tick, so shutdown and recovery failure stay responsive.
const STEPS_PER_TICK: usize = 64;

/// Node-local holder in [`Kura`]. Requests keep the complete walk until a producer
/// attaches; afterwards only a usable published authority admits a request.
#[derive(Debug, Default)]
pub(crate) struct RetainedAttestation {
    attached: AtomicBool,
    poisoned: AtomicBool,
    generation: AtomicU64,
    published: parking_lot::RwLock<Option<Arc<Published>>>,
}

/// One immutable published authority; requests borrow it under a short read lock.
#[derive(Clone, Debug)]
struct Published {
    generation: u64,
    network: NetworkId,
    root_scope: SumeragiRootScope,
    genesis: GenesisDecision,
    tip: AuthenticatedExecutionIdentity,
    // Ascending verified (height, Iroha hash) pairs ending at `tip`.
    recent: Arc<[(u64, HashOf<BlockHeader>)]>,
    // MAX_COVERAGE_AGE after the start of the last complete re-read; every frame up
    // to `tip` was read at or after that start.
    coverage_expires_at: Instant,
}

fn refused(reason: &str) -> ProofError {
    ProofError::NativeExecution(QueryExecutionFail::Conversion(format!(
        "retained attestation authority {reason}"
    )))
}

// Whether the view's journal holds exactly `hash` at `height`.
fn journal_has(view: &impl StateReadOnly, height: u64, hash: HashOf<BlockHeader>) -> bool {
    usize::try_from(height)
        .ok()
        .and_then(|height| height.checked_sub(1))
        .and_then(|index| view.block_hashes().get(index).copied())
        == Some(hash)
}

fn nonzero(height: u64) -> Option<NonZeroUsize> {
    usize::try_from(height).ok().and_then(NonZeroUsize::new)
}

impl RetainedAttestation {
    /// `Ok(true)` when the published authority covers this view, so the request reads
    /// only its target and parent; `Ok(false)` without an attached producer, so the
    /// request keeps the complete walk. A genesis frame that contradicts the retained
    /// decision poisons the authority.
    pub(super) fn admit(
        &self,
        view: &impl StateReadOnly,
        height: u64,
        genesis: &GenesisDecision,
    ) -> Result<bool, ProofError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(refused("is poisoned"));
        }
        if !self.attached.load(Ordering::Acquire) {
            return Ok(false);
        }
        if self.usable(view, height)?.genesis != *genesis {
            self.poison("the genesis frame contradicts the retained decision");
            return Err(refused("is poisoned"));
        }
        Ok(true)
    }

    /// Recheck before signing: H1/H2 have complete proofs; later captures require
    /// retained coverage. Poisoned history refuses at every height.
    pub(super) fn recheck(&self, view: &impl StateReadOnly, height: u64) -> Result<(), ProofError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(refused("is poisoned"));
        }
        if !matches!(height, 1 | 2) && self.attached.load(Ordering::Acquire) {
            self.usable(view, height)?;
        }
        Ok(())
    }

    fn usable(&self, view: &impl StateReadOnly, height: u64) -> Result<Arc<Published>, ProofError> {
        let published = self.current().ok_or_else(|| refused("is not minted"))?;
        if Instant::now() >= published.coverage_expires_at {
            return Err(refused("coverage expired"));
        }
        if published.network != *view.network_id()
            || view
                .native_execution_tip()
                .map(NativeExecutionTip::root_scope)
                != Some(published.root_scope)
            || !published.covers(view, height)
        {
            return Err(refused("does not cover this view"));
        }
        Ok(published)
    }

    /// The published authority of the current generation, unless poisoned.
    fn current(&self) -> Option<Arc<Published>> {
        if self.poisoned.load(Ordering::Acquire) {
            return None;
        }
        self.published
            .read()
            .clone()
            .filter(|published| published.generation == self.generation.load(Ordering::Acquire))
    }

    /// Publish only within the generation the work started in.
    fn publish(&self, published: Published) -> bool {
        let mut slot = self.published.write();
        if self.poisoned.load(Ordering::Acquire)
            || published.generation != self.generation.load(Ordering::Acquire)
        {
            return false;
        }
        *slot = Some(Arc::new(published));
        true
    }

    /// Record a complete re-read that started at `started_at` in `generation`.
    fn cover(&self, generation: u64, started_at: Instant) -> bool {
        let mut slot = self.published.write();
        let Some(published) = slot.as_ref() else {
            return false;
        };
        if self.poisoned.load(Ordering::Acquire)
            || published.generation != generation
            || generation != self.generation.load(Ordering::Acquire)
        {
            return false;
        }
        let mut covered = Published::clone(published);
        covered.coverage_expires_at = started_at + MAX_COVERAGE_AGE;
        *slot = Some(Arc::new(covered));
        true
    }

    /// Drop the authority after the State replaced or reverted its retained tip.
    fn revoke(&self) {
        let mut slot = self.published.write();
        self.generation.fetch_add(1, Ordering::AcqRel);
        *slot = None;
    }

    fn poison(&self, reason: &str) {
        let mut slot = self.published.write();
        self.poisoned.store(true, Ordering::Release);
        *slot = None;
        drop(slot);
        iroha_logger::error!(
            reason,
            "retained attestation authority poisoned; current attestations refuse until restart"
        );
    }
}

impl Published {
    // The view must contain the retained tip at most MAX_LEAD below its height, or its
    // height must be one of the recently verified ones with the same Iroha hash.
    fn covers(&self, view: &impl StateReadOnly, height: u64) -> bool {
        if self.tip.height() <= height {
            height - self.tip.height() <= MAX_LEAD
                && journal_has(view, self.tip.height(), self.tip.iroha_hash())
        } else {
            self.recent
                .iter()
                .any(|&(at, hash)| at == height && journal_has(view, at, hash))
        }
    }
}

/// Another producer already owns this store's retained authority.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("a retained attestation producer is already attached")]
pub struct RetainedAttestationProducerAttached;

/// The sole producer of one store's retained current-attestation authority.
/// Dropping it detaches the holder, returning requests to the complete walk.
pub struct RetainedAttestationProducer {
    state: Arc<State>,
    kura: Arc<Kura>,
    scrub: Option<ScrubPass>,
    next_scrub: Instant,
    next_scrub_chunk: Instant,
    next_mint: Instant,
    // A refused walk, judged on the next step's fresh view: revoke if the State
    // replaced the retained tip meanwhile, otherwise poison.
    fault: Option<String>,
}

#[derive(Clone, Copy)]
struct ScrubPass {
    generation: u64,
    started_at: Instant,
    genesis: GenesisDecision,
    covered: u64,
    next: AuthenticatedExecutionIdentity,
}

impl RetainedAttestationProducer {
    /// Attach the sole producer for this State's store.
    ///
    /// # Errors
    /// Another producer is already attached.
    pub fn attach(state: Arc<State>) -> Result<Self, RetainedAttestationProducerAttached> {
        let kura = state.kura_handle();
        kura.retained_attestation()
            .attached
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| RetainedAttestationProducerAttached)?;
        let now = Instant::now();
        Ok(Self {
            state,
            kura,
            scrub: None,
            next_scrub: now + SCRUB_INTERVAL,
            next_scrub_chunk: now,
            next_mint: now,
            fault: None,
        })
    }

    /// Start the supervised producer once startup recovery succeeds.
    ///
    /// The child never returns before shutdown or recovery failure. A refused attach
    /// or a panicking step leaves requests on the complete walk while it idles.
    pub fn start(
        state: Arc<State>,
        startup_recovery: StartupRecovery,
        shutdown_signal: ShutdownSignal,
    ) -> Child {
        Child::new(
            tokio::spawn(Self::run_supervised(
                state,
                startup_recovery,
                shutdown_signal,
            )),
            OnShutdown::Wait(Duration::from_secs(30)),
        )
    }

    async fn run_supervised(
        state: Arc<State>,
        mut startup_recovery: StartupRecovery,
        shutdown_signal: ShutdownSignal,
    ) {
        if !startup_recovery.wait_for_success(&shutdown_signal).await {
            return;
        }
        let mut producer = match Self::attach(state) {
            Ok(producer) => Some(producer),
            Err(error) => {
                iroha_logger::warn!(%error, "retained attestation producer not started");
                None
            }
        };
        let mut tick = tokio::time::interval(IDLE);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                () = startup_recovery.failed() => break,
                () = shutdown_signal.receive() => break,
                _ = tick.tick() => {
                    let Some(owned) = producer.as_mut() else {
                        continue;
                    };
                    let stepped = tokio::task::block_in_place(|| {
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            for _ in 0..STEPS_PER_TICK {
                                if !owned.step() {
                                    break;
                                }
                            }
                        }))
                    });
                    if stepped.is_err() {
                        iroha_logger::error!(
                            "retained attestation producer panicked; current attestations use the complete walk"
                        );
                        producer = None;
                    }
                }
            }
        }
    }

    /// One bounded unit of work: a mint, an advance or one scrub chunk.
    /// Returns whether more work is immediately pending.
    pub fn step(&mut self) -> bool {
        let state = Arc::clone(&self.state);
        let kura = Arc::clone(&self.kura);
        let holder = kura.retained_attestation();
        if holder.poisoned.load(Ordering::Acquire) {
            return false;
        }
        let Ok(view) = state.try_view_once() else {
            return false;
        };
        let Some(tip) = view.native_execution_tip() else {
            return false;
        };
        // Work only on frames Kura has made durable, so a pending write never reads as
        // corruption. Heights one and two keep their own complete proofs.
        if tip.height() <= 2
            || !view
                .kura()
                .exact_durable_blocks_count()
                .is_ok_and(|durable| durable >= view.block_hashes().len())
        {
            return false;
        }
        let Some(published) = holder.current() else {
            self.scrub = None;
            self.fault = None;
            if Instant::now() >= self.next_mint {
                self.mint(holder, &view, tip);
            }
            return false;
        };
        let replaced = !journal_has(&view, published.tip.height(), published.tip.iroha_hash());
        if let Some(reason) = self.fault.take() {
            if replaced {
                holder.revoke();
            } else {
                holder.poison(&reason);
            }
            return replaced;
        }
        if replaced {
            // The State replaced or reverted the retained tip: mint again from the new one.
            iroha_logger::info!(
                height = published.tip.height(),
                "retained attestation tip replaced; minting again"
            );
            holder.revoke();
            self.scrub = None;
            return true;
        }
        if tip.height() > published.tip.height() {
            self.advance(holder, &view, tip, &published);
            return true;
        }
        if self.scrub.is_none() && Instant::now() >= self.next_scrub {
            self.scrub = Some(ScrubPass {
                generation: published.generation,
                started_at: Instant::now(),
                genesis: published.genesis,
                covered: published.tip.height(),
                next: published.tip,
            });
        }
        // Scrub chunks are paced, never batched: advances and requests keep priority.
        if self.scrub.is_some() && Instant::now() >= self.next_scrub_chunk {
            self.scrub_chunk(holder, &view);
            self.next_scrub_chunk = Instant::now() + SCRUB_PAUSE;
        }
        false
    }

    fn mint(
        &mut self,
        holder: &RetainedAttestation,
        view: &impl StateReadOnly,
        tip: NativeExecutionTip,
    ) {
        let generation = holder.generation.load(Ordering::Acquire);
        let started_at = Instant::now();
        let height = tip.height();
        // The same complete walk a request performs without an attached producer.
        let minted = CertifiedChain::new(view)
            .map_err(ProofError::from)
            .and_then(|chain| {
                let (genesis, _) = super::attestation_genesis_proof(&chain)?;
                super::current_execution_proof(&chain, height, genesis)?;
                Ok(genesis)
            });
        let genesis = match minted {
            Ok(genesis) => genesis,
            Err(error) => {
                self.next_mint = Instant::now() + MINT_RETRY;
                iroha_logger::warn!(%error, height, "retained attestation authority mint refused; retrying");
                return;
            }
        };
        let recent = (height.saturating_sub(RECENT - 1).max(1)..=height)
            .filter_map(|at| {
                let index = usize::try_from(at).ok()?.checked_sub(1)?;
                view.block_hashes().get(index).map(|hash| (at, *hash))
            })
            .collect();
        let published = Published {
            generation,
            network: *view.network_id(),
            root_scope: tip.root_scope(),
            genesis,
            tip: AuthenticatedExecutionIdentity::tip(tip),
            recent,
            coverage_expires_at: started_at + MAX_COVERAGE_AGE,
        };
        if holder.publish(published) {
            self.next_scrub = started_at + SCRUB_INTERVAL;
            iroha_logger::info!(
                height,
                elapsed_ms = u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                "retained attestation authority minted"
            );
        }
    }

    fn advance(
        &mut self,
        holder: &RetainedAttestation,
        view: &impl StateReadOnly,
        tip: NativeExecutionTip,
        published: &Published,
    ) {
        let from = published.tip;
        let (Some(first), Some(last)) = (nonzero(from.height() + 1), nonzero(tip.height())) else {
            return;
        };
        if tip.height() - from.height() > MAX_ADVANCE {
            holder.revoke();
            self.scrub = None;
            return;
        }
        // Read every new frame down from the State tip; the lowest must name the
        // retained tip as its parent.
        let mut fresh = Vec::new();
        let mut parent = None;
        let walked = view.canonical_history().visit_executed_backwards_until(
            first,
            last,
            |_, _| Ok(()),
            |receipt| {
                fresh.push((receipt.height(), receipt.block_hash()));
                if receipt.height() == from.height() + 1 {
                    parent = AuthenticatedExecutionIdentity::parent_of(&receipt);
                }
                Ok(core::ops::ControlFlow::Continue(()))
            },
        );
        match walked {
            Ok(_) if parent == Some(from) => {}
            Ok(_) => return self.fault("published successors do not extend the retained tip"),
            Err(ExecutionAttemptError::Rejected(error)) => {
                return self.fault(&format!("advance walk refused: {error}"));
            }
            Err(ExecutionAttemptError::Deferred(_)) => return,
        }
        fresh.reverse();
        let keep_from = tip.height().saturating_sub(RECENT - 1);
        let recent = published
            .recent
            .iter()
            .copied()
            .chain(fresh)
            .filter(|&(at, _)| at >= keep_from)
            .collect();
        holder.publish(Published {
            tip: AuthenticatedExecutionIdentity::tip(tip),
            recent,
            ..published.clone()
        });
    }

    fn scrub_chunk(&mut self, holder: &RetainedAttestation, view: &impl StateReadOnly) {
        let Some(pass) = self.scrub else {
            return;
        };
        if pass.generation != holder.generation.load(Ordering::Acquire) {
            self.scrub = None;
            return;
        }
        let start = pass.next;
        let first = start.height().saturating_sub(SCRUB_CHUNK - 1).max(1);
        let Some(first_index) = nonzero(first) else {
            return;
        };
        let mut lowest = None;
        let walked = view
            .canonical_history()
            .visit_executed_backwards_from_identity(
                start,
                first_index,
                |_, _| Ok(()),
                |receipt| {
                    if receipt.height() == first {
                        lowest = Some((
                            AuthenticatedExecutionIdentity::of(&receipt),
                            AuthenticatedExecutionIdentity::parent_of(&receipt),
                        ));
                    }
                    Ok(())
                },
            );
        match walked {
            Ok(()) => {}
            Err(ExecutionAttemptError::Rejected(error)) => {
                return self.fault(&format!(
                    "scrub refused at or below {}: {error}",
                    start.height()
                ));
            }
            Err(ExecutionAttemptError::Deferred(_)) => return,
        }
        let Some((own, parent)) = lowest else {
            return self.fault("scrub chunk returned no receipt");
        };
        if first > 1 {
            match parent {
                Some(parent) => {
                    self.scrub = Some(ScrubPass {
                        next: parent,
                        ..pass
                    });
                }
                None => self.fault("scrub receipt names no parent above genesis"),
            }
            return;
        }
        // Genesis must still be the decision every request checks its own frame against.
        if own.iroha_hash() != pass.genesis.block_hash
            || own.core_hash() != pass.genesis.core_hash
            || own.result() != pass.genesis.result
        {
            return self.fault("scrubbed genesis differs from the retained decision");
        }
        self.scrub = None;
        self.next_scrub = pass.started_at + SCRUB_INTERVAL;
        if holder.cover(pass.generation, pass.started_at) {
            iroha_logger::info!(
                height = pass.covered,
                elapsed_ms =
                    u64::try_from(pass.started_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                "retained attestation scrub pass complete"
            );
        }
    }

    // A refused walk over a durable view either raced a replaced tip, which mints
    // again, or contradicts the retained history, which poisons until restart. The
    // next step decides on a fresh view; nothing is published meanwhile.
    fn fault(&mut self, reason: &str) {
        self.scrub = None;
        self.fault = Some(reason.to_owned());
    }
}

impl Drop for RetainedAttestationProducer {
    fn drop(&mut self) {
        let holder = self.kura.retained_attestation();
        let mut slot = holder.published.write();
        holder.attached.store(false, Ordering::Release);
        *slot = None;
    }
}

/// Whether a current attestation signed at `height` may still be released: an
/// attached retained authority must still cover later heights on a fresh view;
/// H1/H2 retain their complete proof capture. Poison refuses every height. Routes
/// call this after encoding, immediately before the response leaves the node.
pub fn retained_attestation_releasable(state: &State, height: u64) -> bool {
    let view = state.view();
    view.kura()
        .retained_attestation()
        .recheck(&view, height)
        .is_ok()
}

#[cfg(test)]
#[path = "retained/tests.rs"]
mod tests;
