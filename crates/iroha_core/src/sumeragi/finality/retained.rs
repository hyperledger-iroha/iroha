//! Retained authority for current attestations.
//!
//! Without it, each current attestation re-reads every native frame from its target
//! down to H2 and genesis. A node-local producer instead mints this authority with
//! one such complete walk, extends it through each newly published durable tip, and
//! keeps re-reading every retained frame in paced bounded chunks. A request then
//! reads only its target and parent (plus any short lead above the retained tip), and
//! checks its genesis frame against the retained decision. Corruption of an older
//! local frame is therefore detected within [`MAX_COVERAGE_AGE`] rather than on the
//! next challenge. A refused walk withdraws the authority at once; a contradiction then
//! poisons it until restart. An unminted, lagging, expired or withdrawn authority
//! refuses as temporarily unavailable. A request keeps the authority it was admitted
//! under through signing and response release. Without an attached producer the
//! complete walk is unchanged.

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

/// Heights a request's view may lead the published tip; the request authenticates
/// those frames itself and the producer closes the gap.
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

/// Why an attached retained authority cannot serve a current attestation now. Each is
/// temporary, because the producer mints, advances or renews the authority, so routes
/// answer it as unavailable. A contradiction poisons the authority instead, which
/// stays a conflict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RetainedUnavailable {
    /// The producer has not minted yet, or withdrew the authority after a refused walk.
    #[error("retained attestation authority is not minted")]
    NotMinted,
    /// No complete re-read started within the coverage age.
    #[error("retained attestation authority coverage expired")]
    CoverageExpired,
    /// The view is neither the retained tip, a recent retained height, nor a short
    /// lead of it.
    #[error("retained attestation authority does not cover this view")]
    NotCovered,
    /// The authority this request was admitted under was withdrawn, replaced or detached.
    #[error("retained attestation authority changed after admission")]
    Changed,
}

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
    // Ascending verified native identities ending at `tip`.
    recent: Arc<[AuthenticatedExecutionIdentity]>,
    // MAX_COVERAGE_AGE after the start of the last complete re-read; every frame up
    // to `tip` was read at or after that start.
    coverage_expires_at: Instant,
}

/// How one current attestation authenticated the history below its target. It is
/// taken with the proof and must still hold when the body is signed and when the
/// response leaves the node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetainedAdmission {
    /// The request read every frame itself: no producer was attached, the height is
    /// H1 or H2, or an enclosing caller keeps its full-prefix recipe.
    CompleteWalk,
    /// The request read only its target pair under this published generation.
    Retained { generation: u64 },
}

/// The release obligation of one signed current attestation. A route that encodes and
/// sends the statement later passes it to [`retained_attestation_releasable`]
/// immediately before the response leaves the node.
#[derive(Clone, Copy, Debug)]
#[must_use = "a current attestation leaves the node only after its release check"]
pub struct AttestationRelease {
    height: u64,
    admission: RetainedAdmission,
}

impl AttestationRelease {
    pub(super) const fn new(height: u64, admission: RetainedAdmission) -> Self {
        Self { height, admission }
    }

    pub(super) const fn admission(&self) -> RetainedAdmission {
        self.admission
    }

    /// The signed durable-tip height.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }
}

fn poisoned() -> ProofError {
    ProofError::NativeExecution(QueryExecutionFail::Conversion(
        "retained attestation authority is poisoned".to_owned(),
    ))
}

// Whether the view's journal holds exactly `hash` at `height`.
fn journal_has(view: &impl StateReadOnly, height: u64, hash: HashOf<BlockHeader>) -> bool {
    usize::try_from(height)
        .ok()
        .and_then(|height| height.checked_sub(1))
        .and_then(|index| view.block_hashes().get(index).copied())
        == Some(hash)
}

// Whether the State still holds `identity`: its own execution tip must equal it in
// full; below the tip its Iroha hash must stay journaled, and the next advance walk
// binds the full identity through the lowest new frame's parent.
fn holds(
    view: &impl StateReadOnly,
    tip: NativeExecutionTip,
    identity: AuthenticatedExecutionIdentity,
) -> bool {
    if tip.height() == identity.height() {
        AuthenticatedExecutionIdentity::tip(tip) == identity
    } else {
        tip.height() > identity.height()
            && journal_has(view, identity.height(), identity.iroha_hash())
    }
}

fn nonzero(height: u64) -> Option<NonZeroUsize> {
    usize::try_from(height).ok().and_then(NonZeroUsize::new)
}

impl RetainedAttestation {
    /// How a request at `height` on `view` authenticates its history: the complete walk
    /// without an attached producer, otherwise only through a published authority that
    /// covers this exact view. A genesis frame that contradicts the retained decision
    /// poisons the authority.
    pub(super) fn admit(
        &self,
        view: &impl StateReadOnly,
        height: u64,
        genesis: &GenesisDecision,
    ) -> Result<RetainedAdmission, ProofError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(poisoned());
        }
        if !self.attached.load(Ordering::Acquire) {
            return Ok(RetainedAdmission::CompleteWalk);
        }
        let published = self.current().ok_or(RetainedUnavailable::NotMinted)?;
        if Instant::now() >= published.coverage_expires_at {
            return Err(RetainedUnavailable::CoverageExpired.into());
        }
        if published.network != *view.network_id()
            || view
                .native_execution_tip()
                .map(NativeExecutionTip::root_scope)
                != Some(published.root_scope)
            || !published.covers(view, height)?
        {
            return Err(RetainedUnavailable::NotCovered.into());
        }
        if published.genesis != *genesis {
            self.poison("the genesis frame contradicts the retained decision");
            return Err(poisoned());
        }
        Ok(RetainedAdmission::Retained {
            generation: published.generation,
        })
    }

    /// Whether `admission` still holds, before signing and again at release. Poisoned
    /// history refuses every height. A retained admission also needs its own generation
    /// still attached, published and within coverage.
    pub(super) fn check(&self, admission: RetainedAdmission) -> Result<(), ProofError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(poisoned());
        }
        let RetainedAdmission::Retained { generation } = admission else {
            return Ok(());
        };
        if !self.attached.load(Ordering::Acquire) {
            return Err(RetainedUnavailable::Changed.into());
        }
        let published = self
            .current()
            .filter(|published| published.generation == generation)
            .ok_or(RetainedUnavailable::Changed)?;
        if Instant::now() >= published.coverage_expires_at {
            return Err(RetainedUnavailable::CoverageExpired.into());
        }
        Ok(())
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

    /// Withdraw the published authority: the State replaced or reverted its retained
    /// tip, or a refused walk awaits classification. Every admission of an earlier
    /// generation refuses from here on.
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
    // The view's own execution tip at `height` must be the retained tip or one of the
    // recently verified identities, in full; or it may lead the retained tip by at most
    // MAX_LEAD heights whose frames this request authenticates back to the retained tip.
    fn covers(&self, view: &impl StateReadOnly, height: u64) -> Result<bool, ProofError> {
        let Some(tip) = view
            .native_execution_tip()
            .filter(|tip| tip.height() == height)
        else {
            return Ok(false);
        };
        let identity = AuthenticatedExecutionIdentity::tip(tip);
        let retained = self.tip.height();
        if height == retained {
            return Ok(identity == self.tip);
        }
        if height < retained {
            return Ok(self.recent.contains(&identity));
        }
        let (Some(first), Some(last)) = (nonzero(retained + 1), nonzero(height)) else {
            return Ok(false);
        };
        if height - retained > MAX_LEAD {
            return Ok(false);
        }
        // Neither the producer nor the target-pair proof reads these newer frames.
        let mut parent = None;
        view.canonical_history()
            .visit_executed_backwards_until(
                first,
                last,
                |_, _| Ok(()),
                |receipt| {
                    if receipt.height() == retained + 1 {
                        parent = AuthenticatedExecutionIdentity::parent_of(&receipt);
                    }
                    Ok(core::ops::ControlFlow::Continue(()))
                },
            )
            .map_err(|error| match error {
                ExecutionAttemptError::Rejected(error) => ProofError::NativeExecution(error),
                ExecutionAttemptError::Deferred(local) => ProofError::Deferred(local),
            })?;
        Ok(parent == Some(self.tip))
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
    // A refused walk, already withdrawn from requests and judged on the next step's
    // fresh view: mint again if the State replaced the retained tip, otherwise poison.
    fault: Option<Fault>,
}

#[derive(Clone, Copy)]
struct ScrubPass {
    generation: u64,
    started_at: Instant,
    genesis: GenesisDecision,
    covered: u64,
    next: AuthenticatedExecutionIdentity,
}

struct Fault {
    reason: String,
    tip: AuthenticatedExecutionIdentity,
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
    /// or a panicking step leaves requests on the complete walk while it idles. A mint
    /// is one complete walk (about a millisecond per frame), which shutdown waits for.
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

    /// One bounded unit of work: a classification, a mint, an advance or one scrub
    /// chunk. Returns whether more work is immediately pending.
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
        if let Some(fault) = self.fault.take() {
            if holds(&view, tip, fault.tip) {
                holder.poison(&fault.reason);
                return false;
            }
            iroha_logger::info!(
                height = fault.tip.height(),
                "retained attestation tip replaced after a refused walk; minting again"
            );
            return true;
        }
        let Some(published) = holder.current() else {
            self.scrub = None;
            if Instant::now() >= self.next_mint {
                self.mint(holder, &view, tip);
            }
            return false;
        };
        if !holds(&view, tip, published.tip) {
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
            self.scrub_chunk(holder, &view, published.tip);
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
        let identity = AuthenticatedExecutionIdentity::tip(tip);
        let published = Published {
            generation,
            network: *view.network_id(),
            root_scope: tip.root_scope(),
            genesis,
            tip: identity,
            // Heights below the minted tip join `recent` only as later advances read them.
            recent: Arc::from([identity]),
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
                fresh.push(AuthenticatedExecutionIdentity::of(&receipt));
                if receipt.height() == from.height() + 1 {
                    parent = AuthenticatedExecutionIdentity::parent_of(&receipt);
                }
                Ok(core::ops::ControlFlow::Continue(()))
            },
        );
        match walked {
            Ok(_) if parent == Some(from) => {}
            Ok(_) => {
                // The State's authenticated chain names another identity at the retained
                // height, so it replaced that tip. Mint again from the State's own chain.
                iroha_logger::info!(
                    height = from.height(),
                    "retained attestation tip superseded by its successors; minting again"
                );
                holder.revoke();
                self.scrub = None;
                return;
            }
            Err(ExecutionAttemptError::Rejected(error)) => {
                return self.fault(holder, from, &format!("advance walk refused: {error}"));
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
            .filter(|identity| identity.height() >= keep_from)
            .collect();
        holder.publish(Published {
            tip: AuthenticatedExecutionIdentity::tip(tip),
            recent,
            ..published.clone()
        });
    }

    fn scrub_chunk(
        &mut self,
        holder: &RetainedAttestation,
        view: &impl StateReadOnly,
        retained: AuthenticatedExecutionIdentity,
    ) {
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
                return self.fault(
                    holder,
                    retained,
                    &format!("scrub refused at or below {}: {error}", start.height()),
                );
            }
            Err(ExecutionAttemptError::Deferred(_)) => return,
        }
        let Some((own, parent)) = lowest else {
            return self.fault(holder, retained, "scrub chunk returned no receipt");
        };
        if first > 1 {
            match parent {
                Some(parent) => {
                    self.scrub = Some(ScrubPass {
                        next: parent,
                        ..pass
                    });
                }
                None => self.fault(
                    holder,
                    retained,
                    "scrub receipt names no parent above genesis",
                ),
            }
            return;
        }
        // Genesis must still be the decision every request checks its own frame against.
        if own.iroha_hash() != pass.genesis.block_hash
            || own.core_hash() != pass.genesis.core_hash
            || own.result() != pass.genesis.result
        {
            return self.fault(
                holder,
                retained,
                "scrubbed genesis differs from the retained decision",
            );
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

    // A refused walk withdraws the authority at once, so no request admits, signs or
    // releases under it while it is classified. The next step decides on a fresh durable
    // view: a replaced tip mints again, a contradiction of the retained history poisons
    // until restart.
    fn fault(
        &mut self,
        holder: &RetainedAttestation,
        tip: AuthenticatedExecutionIdentity,
        reason: &str,
    ) {
        holder.revoke();
        self.scrub = None;
        self.fault = Some(Fault {
            reason: reason.to_owned(),
            tip,
        });
    }
}

impl Drop for RetainedAttestationProducer {
    fn drop(&mut self) {
        let holder = self.kura.retained_attestation();
        let mut slot = holder.published.write();
        holder.attached.store(false, Ordering::Release);
        // Admissions taken under this producer never outlive it.
        holder.generation.fetch_add(1, Ordering::AcqRel);
        *slot = None;
    }
}

/// Whether a signed current attestation may still leave the node. Routes call this
/// after encoding, immediately before the response is released. Poison refuses every
/// attestation; a retained admission also needs its own generation still attached,
/// published and within coverage.
#[must_use]
pub fn retained_attestation_releasable(state: &State, release: &AttestationRelease) -> bool {
    state
        .kura_handle()
        .retained_attestation()
        .check(release.admission())
        .is_ok()
}

#[cfg(test)]
#[path = "retained/tests.rs"]
mod tests;
