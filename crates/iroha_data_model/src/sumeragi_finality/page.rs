//! Bounded consecutive native proof pages extending an independently selected checkpoint.
use std::sync::atomic;

use super::*;

/// Exact native decision identity `H(tag || core_hash || result)`.
/// This digest alone does not authenticate its inputs or create a trust root.
#[must_use]
pub fn certified_block_context_id(core_hash: &Hash32, result: &Hash32) -> Hash {
    const TAG: &[u8] = b"iroha/sumeragi/certified-block/v1";
    let mut bytes = Vec::with_capacity(TAG.len() + 64);
    bytes.extend_from_slice(TAG);
    bytes.extend_from_slice(&core_hash.0);
    bytes.extend_from_slice(&result.0);
    Hash::new(bytes)
}

/// Verified page tip and the restart checkpoint derived from that same authenticated prefix.
/// Neither field can be supplied by an unverified response.
#[derive(Debug, Clone)]
pub struct VerifiedFinalityPage {
    tip: VerifiedSumeragiBlock,
    checkpoint: SumeragiFinalityCheckpoint,
}
impl VerifiedFinalityPage {
    /// Last block authenticated from the caller's exact checkpoint.
    #[must_use]
    pub fn tip(&self) -> &VerifiedSumeragiBlock {
        &self.tip
    }
    /// Bounded checkpoint suitable for durable promotion after all application proofs succeed.
    #[must_use]
    pub fn checkpoint(&self) -> &SumeragiFinalityCheckpoint {
        &self.checkpoint
    }
    /// Consume this verified page to retain its authenticated checkpoint.
    #[must_use]
    pub fn into_checkpoint(self) -> SumeragiFinalityCheckpoint {
        self.checkpoint
    }
}

/// Verify a page beginning with the caller's exact independently authenticated checkpoint.
///
/// The first proof reauthenticates that decision, including any alternate quorum witness;
/// every following proof extends it consecutively. No supplied response field can select
/// the trust root, instance, authority generation, or previous result. Nothing mutates the
/// caller's checkpoint on failure. Application callers must verify their own witnesses
/// before persisting the returned checkpoint.
///
/// Long pages verify on worker threads when the process has them to spare. The outcome,
/// refusal and decoder charges are the same as verifying on this thread.
///
/// # Errors
/// Invalid bounds, wrong network, absent checkpoint, gaps, altered decisions or failed signatures.
pub fn verify_checkpoint_page(
    network_id: NetworkId,
    checkpoint: &SumeragiFinalityCheckpoint,
    proofs: &[SumeragiFinalityProof],
    max_proofs: usize,
    max_bytes: usize,
) -> Result<VerifiedFinalityPage, FinalityReadError> {
    let permit = PAGE_WORKERS.reserve(page_workers(proofs.len().saturating_sub(1)));
    verify_page_on_workers(
        network_id,
        checkpoint,
        proofs,
        max_proofs,
        max_bytes,
        permit.count,
    )
}

fn verify_page_on_workers(
    network_id: NetworkId,
    checkpoint: &SumeragiFinalityCheckpoint,
    proofs: &[SumeragiFinalityProof],
    max_proofs: usize,
    max_bytes: usize,
    workers: usize,
) -> Result<VerifiedFinalityPage, FinalityReadError> {
    need(
        max_proofs > 0
            && max_proofs <= crate::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_COUNT
            && max_bytes > 0
            && max_bytes <= crate::sumeragi::finality::NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        "native proof page limits are invalid",
    )?;
    need(
        !proofs.is_empty() && proofs.len() <= max_proofs,
        "native proof page is empty or exceeds its count bound",
    )?;
    check_page_byte_bound(proofs, max_bytes)?;
    norito::core::with_decode_limits_scope(norito::canonical_decode_limits(max_bytes), || {
        let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            checkpoint,
            &network_id,
            checkpoint.chain_id(),
        )?;
        let first = &proofs[0];
        need(
            first.height() == checkpoint.height(),
            "native proof page does not start at the pinned checkpoint",
        )?;
        let mut tip = verifier.verify_same_decision(checkpoint.tip(), first)?;
        let rest = &proofs[1..];
        if let Some(verified) = verifier.extend_on_workers(rest, workers) {
            tip = verified;
        } else {
            for proof in rest {
                tip = verifier.verify(proof)?;
            }
        }
        let checkpoint =
            verifier.export_checkpoint(proofs.last().expect("non-empty native page"))?;
        Ok(VerifiedFinalityPage { tip, checkpoint })
    })
}

/// Shorter runs stay on the caller's thread.
const PAGE_WORKER_MIN_FRAMES: usize = 128;
/// No worker gets fewer consecutive frames than this.
const PAGE_WORKER_MIN_CHUNK: usize = 64;
/// Across all pages this process is verifying at once.
const PAGE_WORKER_LIMIT: usize = 8;
/// Reserved, not committed; no caller's thread gives a frame's decoder more room than this.
const PAGE_WORKER_STACK_BYTES: usize = 16 * 1024 * 1024;

static PAGE_WORKERS: WorkerPool = WorkerPool::new(PAGE_WORKER_LIMIT);

fn page_workers(frames: usize) -> usize {
    if frames < PAGE_WORKER_MIN_FRAMES {
        return 1;
    }
    std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(PAGE_WORKER_LIMIT)
        .min(frames / PAGE_WORKER_MIN_CHUNK)
}

struct WorkerPool {
    active: atomic::AtomicUsize,
    limit: usize,
}

struct WorkerPermit<'a> {
    pool: &'a WorkerPool,
    count: usize,
}

impl WorkerPool {
    const fn new(limit: usize) -> Self {
        Self {
            active: atomic::AtomicUsize::new(0),
            limit,
        }
    }

    /// Grant as many workers as are free, or none when fewer than two are.
    fn reserve(&self, wanted: usize) -> WorkerPermit<'_> {
        let mut count = 0;
        let granted = self.active.try_update(
            atomic::Ordering::AcqRel,
            atomic::Ordering::Acquire,
            |active| {
                count = wanted.min(self.limit.saturating_sub(active));
                (count >= 2).then_some(active + count)
            },
        );
        WorkerPermit {
            pool: self,
            count: if granted.is_ok() { count } else { 0 },
        }
    }
}

impl Drop for WorkerPermit<'_> {
    fn drop(&mut self) {
        self.pool
            .active
            .fetch_sub(self.count, atomic::Ordering::AcqRel);
    }
}

/// Run `tasks` on one thread each and collect their results, or `None` if any task returned
/// `None`, panicked or could not start. Every thread is joined either way.
fn run_on_workers<T: Send>(
    stack_bytes: usize,
    stop: &atomic::AtomicBool,
    tasks: Vec<impl FnOnce() -> Option<T> + Send>,
) -> Option<Vec<T>> {
    std::thread::scope(|scope| {
        let handles = tasks
            .into_iter()
            .map(|task| {
                let handle = std::thread::Builder::new()
                    .stack_size(stack_bytes)
                    .spawn_scoped(scope, task);
                if handle.is_err() {
                    stop.store(true, atomic::Ordering::Relaxed);
                }
                handle
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|handle| handle.ok()?.join().ok()?)
            .collect::<Vec<_>>()
    })
    .into_iter()
    .collect()
}

struct WorkerFrames {
    parent: Option<(u64, Decision)>,
    decisions: BTreeMap<u64, Decision>,
    tip: VerifiedSumeragiBlock,
}

/// Each later worker's assumed parent must be what the worker before it retained. Decoding
/// is a pure function of a proof's bytes, so this only fails if that ever stops being true.
fn join_worker_frames(
    outcomes: Vec<WorkerFrames>,
) -> Option<(BTreeMap<u64, Decision>, VerifiedSumeragiBlock)> {
    let mut adopted = BTreeMap::new();
    let mut tip = None;
    for outcome in outcomes {
        if let Some((height, decision)) = outcome.parent {
            if adopted.get(&height) != Some(&decision) {
                return None;
            }
        }
        adopted.extend(outcome.decisions);
        tip = Some(outcome.tip);
    }
    Some((adopted, tip?))
}

impl SumeragiFinalityVerifier {
    /// Admit consecutive proofs on worker threads, keeping the sequential chain of parents.
    ///
    /// Every proof passes the same [`Self::verify`] as the sequential loop. A later worker
    /// starts from the decision decoded from the previous worker's last proof, which that
    /// worker verifies, so each parent is authenticated by induction from this verifier's
    /// own tip: the same trust root and the same checks in a different order.
    ///
    /// Workers share one private decoder budget holding what the caller's active scopes
    /// have left, so a run those scopes would refuse cannot pass here. Its charges move to
    /// those scopes only on success, and they equal what the sequential loop charges.
    /// `None` adopts and charges nothing. The caller then runs the sequential loop, which
    /// alone reports the outcome and exact refusal for anything workers could not verify.
    fn extend_on_workers(
        &mut self,
        proofs: &[SumeragiFinalityProof],
        workers: usize,
    ) -> Option<VerifiedSumeragiBlock> {
        if workers < 2 || proofs.is_empty() {
            return None;
        }
        let budget =
            norito::core::DecodeBudgetContext::new(norito::core::active_decode_limits_remaining()?);
        let discriminant = crate::account::address::chain_discriminant();
        let stop = atomic::AtomicBool::new(false);
        let chunk = proofs.len().div_ceil(workers);
        let base: &Self = self;
        let (budget_ref, stop_ref) = (&budget, &stop);
        let tasks = proofs
            .chunks(chunk)
            .enumerate()
            .map(|(index, frames)| {
                let parent = index.checked_sub(1).map(|_| &proofs[index * chunk - 1]);
                move || {
                    let _discriminant =
                        crate::account::address::ChainDiscriminantGuard::enter(discriminant);
                    let outcome = base.verify_worker_frames(parent, frames, budget_ref, stop_ref);
                    if outcome.is_none() {
                        stop_ref.store(true, atomic::Ordering::Relaxed);
                    }
                    outcome
                }
            })
            .collect::<Vec<_>>();
        let outcomes = run_on_workers(PAGE_WORKER_STACK_BYTES, stop_ref, tasks)?;
        let (adopted, tip) = join_worker_frames(outcomes)?;
        budget.charge_to_active().ok()?;
        self.decisions = adopted;
        Some(tip)
    }

    fn verify_worker_frames(
        &self,
        parent: Option<&SumeragiFinalityProof>,
        frames: &[SumeragiFinalityProof],
        budget: &norito::core::DecodeBudgetContext,
        stop: &atomic::AtomicBool,
    ) -> Option<WorkerFrames> {
        let mut worker = self.clone();
        // The previous worker's verify charges this proof; decoding it here again does not.
        let parent = match parent {
            None => None,
            Some(parent) => {
                let (decoded, _, _) = parent.decode_parts(worker.root_scope().into()).ok()?;
                let decision = Self::decision(&decoded);
                worker.decisions.clear();
                worker.decisions.insert(parent.height(), decision.clone());
                Some((parent.height(), decision))
            }
        };
        let tip = budget.with(|| {
            let mut tip = None;
            for proof in frames {
                if stop.load(atomic::Ordering::Relaxed) {
                    return None;
                }
                tip = Some(worker.verify(proof).ok()?);
            }
            tip
        })?;
        Some(WorkerFrames {
            parent,
            decisions: worker.decisions,
            tip,
        })
    }
}

// Measure the borrowed generic sequence with the codec's exact Vec framing. No
// payload buffer or proof clones are needed, and each prefix is checked before
// measuring another proof.
fn check_page_byte_bound(
    proofs: &[SumeragiFinalityProof],
    max_bytes: usize,
) -> Result<(), FinalityError> {
    let mut measured =
        norito::core::SequencePayloadLength::new(norito::core::default_encode_flags())
            .map_err(malformed)?;
    let max_payload = max_bytes
        .checked_sub(norito::core::Header::SIZE)
        .ok_or_else(|| FinalityError("native proof page exceeds its byte bound".into()))?;
    need(
        measured.len() <= max_payload,
        "native proof page exceeds its byte bound",
    )?;
    for proof in proofs {
        measured.push(proof).map_err(malformed)?;
        need(
            measured.len() <= max_payload,
            "native proof page exceeds its byte bound",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::tests::Fixture;
    use super::*;
    const CAP: usize = 3 * 1024 * 1024;

    fn checkpoint(f: &Fixture) -> SumeragiFinalityCheckpoint {
        let mut verifier = f.verifier();
        verifier.verify(&f.first).unwrap();
        verifier.export_checkpoint(&f.first).unwrap()
    }
    #[test]
    fn native_page_preserves_exact_context_and_promotes_authenticated_tip() {
        let f = Fixture::new();
        let checkpoint = checkpoint(&f);
        let page = verify_checkpoint_page(
            f.network,
            &checkpoint,
            &[f.first.clone(), f.second.clone()],
            64,
            CAP,
        )
        .unwrap();
        assert_eq!(page.tip().height(), 2);
        assert_eq!(
            page.tip().context_id(),
            certified_block_context_id(&page.tip().core_hash(), &page.tip().result())
        );
        assert_eq!(page.checkpoint().height(), 2);
        assert_eq!(checkpoint.height(), 1);
        let promoted = page.into_checkpoint();
        verify_checkpoint_page(
            f.network,
            &promoted,
            std::slice::from_ref(&f.second),
            64,
            CAP,
        )
        .unwrap();
    }
    #[test]
    fn native_page_rejects_missing_anchor_wrong_network_gaps_and_tampering() {
        let f = Fixture::new();
        let checkpoint = checkpoint(&f);
        for proofs in [
            vec![],
            vec![f.second.clone()],
            vec![f.first.clone(), f.first.clone()],
            vec![f.second.clone(), f.first.clone()],
        ] {
            assert!(verify_checkpoint_page(f.network, &checkpoint, &proofs, 64, CAP).is_err());
        }
        let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"foreign native network",
        )));
        assert!(
            verify_checkpoint_page(
                foreign,
                &checkpoint,
                std::slice::from_ref(&f.first),
                64,
                CAP
            )
            .is_err()
        );
        let mut changed = f.second.clone();
        let last = changed.block_wire.len() - 1;
        changed.block_wire[last] ^= 1;
        assert!(
            verify_checkpoint_page(f.network, &checkpoint, &[f.first.clone(), changed], 64, CAP)
                .is_err()
        );
        assert!(
            verify_checkpoint_page(
                f.network,
                &checkpoint,
                std::slice::from_ref(&f.first),
                0,
                CAP
            )
            .is_err()
        );
        assert!(
            verify_checkpoint_page(
                f.network,
                &checkpoint,
                std::slice::from_ref(&f.first),
                64,
                1
            )
            .is_err()
        );
        assert_eq!(checkpoint.height(), 1);
    }
    mod workers {
        use super::*;
        use crate::sumeragi_finality::test_fixtures::NativeFinalityFixture;

        const LIMIT: usize = 4096;
        const BYTES: usize = 16 << 20;

        fn scoped<T>(body: impl FnOnce() -> T) -> T {
            norito::core::with_decode_limits_scope(norito::canonical_decode_limits(BYTES), body)
        }

        fn extend(
            verifier: &mut SumeragiFinalityVerifier,
            proofs: &[SumeragiFinalityProof],
            workers: usize,
        ) -> bool {
            scoped(|| verifier.extend_on_workers(proofs, workers)).is_some()
        }

        struct Chain {
            native: NativeFinalityFixture,
            anchor: SumeragiFinalityVerifier,
            proofs: Vec<SumeragiFinalityProof>,
        }

        fn grow(native: &mut NativeFinalityFixture, frames: usize) -> Vec<SumeragiFinalityProof> {
            (0..frames)
                .map(|_| {
                    let block = native.block_with_submitted_work(native.next_header());
                    native.certify(block)
                })
                .collect()
        }

        fn chain(frames: usize) -> Chain {
            let mut native = NativeFinalityFixture::new();
            let anchor = native.verifier();
            let proofs = grow(&mut native, frames);
            Chain {
                native,
                anchor,
                proofs,
            }
        }

        fn sequential_refusal(
            anchor: &SumeragiFinalityVerifier,
            proofs: &[SumeragiFinalityProof],
        ) -> String {
            let mut verifier = anchor.clone();
            let error = proofs
                .iter()
                .try_for_each(|proof| verifier.verify(proof).map(|_| ()))
                .unwrap_err();
            format!("{error:?}")
        }

        fn assert_refused(anchor: &SumeragiFinalityVerifier, proofs: &[SumeragiFinalityProof]) {
            for workers in [2, 3, 4] {
                let mut parallel = anchor.clone();
                assert!(!extend(&mut parallel, proofs, workers), "{workers} workers");
                assert!(parallel.decisions == anchor.decisions);
            }
            sequential_refusal(anchor, proofs);
        }

        #[test]
        fn workers_reach_the_exact_state_of_the_sequential_loop() {
            let Chain { anchor, proofs, .. } = chain(9);
            let mut sequential = anchor.clone();
            let mut expected_tip = None;
            for proof in &proofs {
                expected_tip = Some(sequential.verify(proof).unwrap());
            }
            let expected_tip = expected_tip.unwrap();
            let tip = proofs.last().unwrap();
            let expected = sequential.export_checkpoint(tip).unwrap();
            for workers in [2, 3, 4, 9] {
                let mut parallel = anchor.clone();
                let verified = scoped(|| parallel.extend_on_workers(&proofs, workers)).unwrap();
                assert_eq!(verified.height(), expected_tip.height());
                assert_eq!(verified.core_hash(), expected_tip.core_hash());
                assert_eq!(verified.result(), expected_tip.result());
                assert!(
                    parallel.decisions == sequential.decisions,
                    "{workers} workers"
                );
                assert_eq!(parallel.export_checkpoint(tip).unwrap(), expected);
            }
        }

        #[test]
        fn a_worker_join_can_fall_anywhere_around_an_epoch_boundary() {
            let (fixture, proofs) = NativeFinalityFixture::short_npos_boundary_chain(4);
            let mut anchor = fixture.verifier().into_genesis_verifier();
            anchor.verify(&proofs[0]).unwrap();
            let rest = &proofs[1..];
            // H2 preboundary, H3 retention, H4 first frame of the next epoch.
            assert_eq!(rest.len(), 3);
            let mut sequential = anchor.clone();
            for proof in rest {
                sequential.verify(proof).unwrap();
            }
            // With one frame per worker, every height is a join.
            for workers in 2..=rest.len() {
                let mut parallel = anchor.clone();
                assert!(extend(&mut parallel, rest, workers), "{workers} workers");
                assert!(
                    parallel.decisions == sequential.decisions,
                    "{workers} workers"
                );
            }
        }

        #[test]
        fn a_certified_fork_is_refused_wherever_it_is_spliced_in() {
            let mut shared = NativeFinalityFixture::new();
            let anchor = shared.verifier();
            let prefix = grow(&mut shared, 3);
            let (mut one, mut two) = (shared.clone(), shared);
            // The same block with two certified results: each child binds its own parent.
            let block = one.block_with_submitted_work(one.next_header());
            let mut first = prefix.clone();
            let mut second = prefix.clone();
            first.push(one.certify_with_world_root(block.clone(), Hash::new(b"first World")));
            second.push(two.certify_with_world_root(block, Hash::new(b"second World")));
            first.extend(grow(&mut one, 5));
            second.extend(grow(&mut two, 5));
            for whole in [&first, &second] {
                assert!(extend(&mut anchor.clone(), whole, 3));
            }
            for cut in prefix.len() + 1..first.len() {
                let mut spliced = first[..cut].to_vec();
                spliced.extend_from_slice(&second[cut..]);
                assert_refused(&anchor, &spliced);
            }
        }

        #[test]
        fn a_validly_signed_frame_from_another_chain_is_refused() {
            let Chain { anchor, proofs, .. } = chain(6);
            let mut other = NativeFinalityFixture::start("another-chain");
            let block = other.block_with_submitted_work(other.next_header());
            other.certify(block);
            let foreign = grow(&mut other, proofs.len());
            for index in 0..proofs.len() {
                let mut changed = proofs.clone();
                changed[index] = foreign[index].clone();
                assert_eq!(changed[index].height(), proofs[index].height());
                assert_refused(&anchor, &changed);
            }
        }

        #[test]
        fn any_changed_byte_adopts_nothing_and_the_sequential_loop_refuses_it() {
            let Chain { anchor, proofs, .. } = chain(8);
            for index in 0..proofs.len() {
                let mut changed = proofs.clone();
                let last = changed[index].block_wire.len() - 1;
                changed[index].block_wire[last] ^= 1;
                assert_refused(&anchor, &changed);
            }
        }

        #[test]
        fn gaps_repeats_and_swaps_across_a_worker_boundary_adopt_nothing() {
            let Chain { anchor, proofs, .. } = chain(8);
            let mut gap = proofs.clone();
            gap.remove(4);
            let mut repeat = proofs.clone();
            repeat[4] = repeat[3].clone();
            let mut swap = proofs.clone();
            swap.swap(3, 4);
            let mut late = proofs.clone();
            late.remove(0);
            for changed in [gap, repeat, swap, late] {
                assert_refused(&anchor, &changed);
            }
        }

        fn page_outcome(
            native: &NativeFinalityFixture,
            checkpoint: &SumeragiFinalityCheckpoint,
            proofs: &[SumeragiFinalityProof],
            workers: usize,
        ) -> String {
            match verify_page_on_workers(
                native.network_id(),
                checkpoint,
                proofs,
                LIMIT,
                BYTES,
                workers,
            ) {
                Ok(page) => format!(
                    "ok {} {:?}",
                    page.tip().height(),
                    page.checkpoint().encode_canonical().unwrap()
                ),
                Err(error) => {
                    // Each refusal carries a per-call boundary id; everything else must match.
                    let refusal = format!("refused {error:?}");
                    match (
                        refusal.find("identity: BoundaryIdentity"),
                        refusal.rfind(" }), kind:"),
                    ) {
                        (Some(start), Some(end)) if start < end => {
                            format!("{}{}", &refusal[..start], &refusal[end..])
                        }
                        _ => refusal,
                    }
                }
            }
        }

        fn enclosing(allocated: usize) -> norito::core::DecodeBudgetContext {
            let limits = norito::canonical_decode_limits(BYTES);
            norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
                limits.max_sequence_elements(),
                limits.max_field_bytes(),
                limits.max_total_elements(),
                allocated,
                limits.max_nesting_depth(),
            ))
        }

        #[test]
        fn a_callers_budget_sees_the_same_outcome_and_charges_with_or_without_workers() {
            let Chain {
                native,
                anchor,
                proofs,
            } = chain(9);
            let mut before = anchor.clone();
            before.verify(&proofs[0]).unwrap();
            let checkpoint = before.export_checkpoint(&proofs[0]).unwrap();
            let run = |allocated, workers| {
                let budget = enclosing(allocated);
                let outcome = budget.with(|| page_outcome(&native, &checkpoint, &proofs, workers));
                (outcome, budget.consumed_allocated_bytes())
            };
            let (expected, spent) = run(usize::MAX, 1);
            assert!(expected.starts_with("ok "), "{expected}");
            assert!(spent > 0);
            for workers in [2, 3, 4] {
                assert_eq!(run(usize::MAX, workers), (expected.clone(), spent));
                let spent = usize::try_from(spent).unwrap();
                for allocated in [spent, spent - 1, spent / 2, spent / 4, spent / 16, 1] {
                    let (one, two) = (run(allocated, 1), run(allocated, workers));
                    assert_eq!(one, two, "{workers} workers, budget {allocated}");
                    assert_eq!(one.0.starts_with("ok "), allocated == spent);
                }
            }
        }

        #[test]
        fn a_long_page_uses_workers_and_matches_the_sequential_page_and_its_refusal() {
            let Chain {
                native,
                anchor,
                proofs,
            } = chain(PAGE_WORKER_MIN_FRAMES + 1);
            let mut before = anchor.clone();
            before.verify(&proofs[0]).unwrap();
            let checkpoint = before.export_checkpoint(&proofs[0]).unwrap();
            let cores = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
            assert_eq!(page_workers(proofs.len() - 1) >= 2, cores >= 2);
            assert!(extend(&mut before.clone(), &proofs[1..], 2));
            let page =
                verify_checkpoint_page(native.network_id(), &checkpoint, &proofs, LIMIT, BYTES)
                    .unwrap();
            assert_eq!(page.tip().height(), proofs.last().unwrap().height());
            assert_eq!(
                scoped(|| page_outcome(&native, &checkpoint, &proofs, 1)),
                format!(
                    "ok {} {:?}",
                    page.tip().height(),
                    page.checkpoint().encode_canonical().unwrap()
                )
            );
            let mut changed = proofs.clone();
            let target = changed.len() - 2;
            let last = changed[target].block_wire.len() - 1;
            changed[target].block_wire[last] ^= 1;
            let refused =
                verify_checkpoint_page(native.network_id(), &checkpoint, &changed, LIMIT, BYTES)
                    .unwrap_err();
            assert_eq!(
                format!("{refused:?}"),
                sequential_refusal(&before, &changed[1..])
            );
        }

        #[test]
        fn a_stopped_or_unstartable_or_panicking_worker_fails_the_whole_attempt() {
            let Chain { anchor, proofs, .. } = chain(2);
            let stop = atomic::AtomicBool::new(true);
            assert!(
                anchor
                    .verify_worker_frames(
                        None,
                        &proofs,
                        &norito::core::DecodeBudgetContext::new(norito::canonical_decode_limits(
                            BYTES
                        )),
                        &stop,
                    )
                    .is_none()
            );
            let stop = atomic::AtomicBool::new(false);
            let task = |value: Option<u8>| move || value;
            assert_eq!(
                run_on_workers(1 << 16, &stop, vec![task(Some(1)), task(Some(2))]),
                Some(vec![1, 2])
            );
            assert_eq!(
                run_on_workers(1 << 16, &stop, vec![task(Some(1)), task(None)]),
                None
            );
            let panics = |fail: bool| {
                move || {
                    assert!(!fail, "expected worker panic");
                    Some(1)
                }
            };
            assert_eq!(
                run_on_workers(1 << 16, &stop, vec![panics(false), panics(true)]),
                None
            );
            assert!(!stop.load(atomic::Ordering::Relaxed));
            assert_eq!(run_on_workers(1 << 50, &stop, vec![task(Some(1))]), None);
            assert!(stop.load(atomic::Ordering::Relaxed));
        }

        #[test]
        fn a_later_worker_must_start_from_what_the_earlier_one_retained() {
            let Chain { anchor, proofs, .. } = chain(4);
            let (head, tail) = proofs.split_at(2);
            let stop = atomic::AtomicBool::new(false);
            let budget =
                norito::core::DecodeBudgetContext::new(norito::canonical_decode_limits(BYTES));
            let run = |parent, frames| anchor.verify_worker_frames(parent, frames, &budget, &stop);
            let good = || vec![run(None, head).unwrap(), run(head.last(), tail).unwrap()];
            assert!(join_worker_frames(good()).is_some());
            let mut changed = good();
            changed[1].parent.as_mut().unwrap().1.executed_len += 1;
            assert!(join_worker_frames(changed).is_none());
        }

        #[test]
        fn short_runs_stay_on_the_caller_thread_and_workers_are_capped_across_the_process() {
            assert_eq!(page_workers(PAGE_WORKER_MIN_FRAMES - 1), 1);
            assert!(page_workers(4096) <= PAGE_WORKER_LIMIT);
            let Chain { anchor, proofs, .. } = chain(2);
            let mut one = anchor;
            assert!(!extend(&mut one, &proofs, 1));
            assert!(!extend(&mut one, &[], 4));
            assert!(one.extend_on_workers(&proofs, 2).is_none());

            let pool = WorkerPool::new(5);
            let first = pool.reserve(4);
            assert_eq!(first.count, 4);
            assert_eq!(pool.reserve(4).count, 0);
            drop(first);
            let second = pool.reserve(4);
            assert_eq!(second.count, 4);
            drop(second);
            let third = pool.reserve(3);
            assert_eq!((third.count, pool.reserve(4).count), (3, 2));
            drop(third);
            assert_eq!(pool.reserve(1).count, 0);
            assert_eq!(pool.active.load(atomic::Ordering::Acquire), 0);
        }
    }

    #[test]
    fn borrowed_page_measurement_matches_canonical_vec_frame_exactly() {
        let f = Fixture::new();
        for proofs in [vec![], vec![f.first.clone()], vec![f.first, f.second]] {
            let frame = norito::to_bytes(&proofs).unwrap();
            check_page_byte_bound(&proofs, frame.len()).unwrap();
            assert!(check_page_byte_bound(&proofs, frame.len() - 1).is_err());
        }
    }
    #[test]
    fn native_context_identity_binds_both_original_commitments() {
        let hash = Hash32([1; 32]);
        let result = Hash32([2; 32]);
        let id = certified_block_context_id(&hash, &result);
        assert_ne!(id, certified_block_context_id(&Hash32([3; 32]), &result));
        assert_ne!(id, certified_block_context_id(&hash, &Hash32([3; 32])));
    }
}
