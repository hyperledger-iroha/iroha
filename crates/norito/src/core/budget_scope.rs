//! One allocation-free active scope chain for ordinary and prepared decoding.
//!
//! Contexts own their counters; this synchronous chain only borrows their layer
//! slices. No pointer escapes the closure or survives its unwind guard. Captured
//! resource errors separately retain the original counter control and attempt.

use super::*;

#[derive(Clone, Debug)]
pub(super) enum CounterOwner {
    Owned(Arc<DecodeBudgetCounters>),
    Prepared(iroha_allocation::ChargedShared<DecodeBudgetCounters>),
}
impl std::ops::Deref for CounterOwner {
    type Target = DecodeBudgetCounters;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(owner) => owner,
            Self::Prepared(owner) => owner,
        }
    }
}
impl CounterOwner {
    pub(super) fn ptr_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Owned(left), Self::Owned(right)) => Arc::ptr_eq(left, right),
            (Self::Prepared(left), Self::Prepared(right)) => {
                iroha_allocation::ChargedShared::ptr_eq(left, right)
            }
            _ => false,
        }
    }
}

/// Deduplicated layers cached in a scope node for constant-time lookup.
const VISIBLE_LAYERS: usize = 8;

pub(super) struct ScopeNode<'a> {
    previous: Option<&'a ScopeNode<'a>>,
    layers: &'a [ActiveDecodeBudgetLayer],
    prior_count: usize,
    count: usize,
    // The complete source-order view, computed once when the immutable chain
    // is installed. Valid when `count <= VISIBLE_LAYERS`; longer chains walk.
    visible: [Option<&'a ActiveDecodeBudgetLayer>; VISIBLE_LAYERS],
    // The tightest stateless limits of the whole chain, computed with it.
    tightest: TightestLimits,
}

/// The tightest stateless limits across every layer of the active chain.
///
/// A field length, sequence length or nesting depth is admitted exactly when
/// every layer admits it, so a value within these bounds needs no layer walk.
/// A value outside them takes the ordered walk, which names the refusing
/// layer. Cumulative element and allocation counters are never summarized.
#[derive(Clone, Copy)]
pub(super) struct TightestLimits {
    /// Largest field body every layer admits.
    pub(super) field_bytes: u64,
    /// Largest single sequence every layer admits.
    pub(super) sequence_elements: u64,
    /// Deepest absolute nesting depth every layer admits.
    pub(super) depth_ceiling: usize,
}
impl TightestLimits {
    const UNBOUNDED: Self = Self {
        field_bytes: u64::MAX,
        sequence_elements: u64::MAX,
        depth_ceiling: usize::MAX,
    };

    fn tightened_by(self, layer: &ActiveDecodeBudgetLayer) -> Self {
        let limits = layer.budget.limits;
        Self {
            field_bytes: self.field_bytes.min(limit_to_u64(limits.max_field_bytes())),
            sequence_elements: self
                .sequence_elements
                .min(limit_to_u64(limits.max_sequence_elements())),
            // A layer refuses `depth - base_depth > max_nesting_depth`.
            depth_ceiling: self
                .depth_ceiling
                .min(layer.base_depth.saturating_add(limits.max_nesting_depth())),
        }
    }
}

/// Tightest stateless limits of the active chain; unbounded without a scope.
#[inline]
pub(super) fn tightest_limits() -> TightestLimits {
    ACTIVE.with(|slot| tightest_limits_of(slot.get()))
}
/// Tightest stateless limits of the chain headed by this thread's active `node`.
#[inline]
pub(super) fn tightest_limits_of(node: *const ScopeNode<'static>) -> TightestLimits {
    // SAFETY: `node` was read from this thread's active slot inside the
    // synchronous scope that installed it, so it is null or still live, as in
    // `with_active`. Only a copy of its plain limits is returned.
    unsafe { node.as_ref() }.map_or(TightestLimits::UNBOUNDED, |node| node.tightest)
}

/// The innermost installed scope node of this thread, kept with the rest of
/// the decoder state so a field decode reads its limits without another lookup.
static ACTIVE: DecodeCell<*const ScopeNode<'static>> = DecodeCell(|tls| &tls.budget);

/// Borrowed source-order view; each original counter is charged once even when
/// a lazy context is reapplied inside the same context.
#[derive(Clone, Copy)]
pub(super) struct BudgetLayers<'a> {
    head: Option<&'a ScopeNode<'a>>,
}
impl<'a> BudgetLayers<'a> {
    pub(super) fn len(self) -> usize {
        self.head.map_or(0, |head| head.count)
    }
    pub(super) fn is_empty(self) -> bool {
        self.len() == 0
    }
    fn contains(self, candidate: &ActiveDecodeBudgetLayer) -> bool {
        let mut node = self.head;
        while let Some(current) = node {
            if current
                .layers
                .iter()
                .any(|layer| layer.budget.counters.ptr_eq(&candidate.budget.counters))
            {
                return true;
            }
            node = current.previous;
        }
        false
    }
    fn get(self, index: usize) -> Option<&'a ActiveDecodeBudgetLayer> {
        if let Some(head) = self.head
            && head.count <= VISIBLE_LAYERS
        {
            return head.visible.get(index).copied().flatten();
        }
        self.walk(index)
    }
    /// Resolve `index` by walking the chain; the reference for the cached view.
    fn walk(self, index: usize) -> Option<&'a ActiveDecodeBudgetLayer> {
        let mut node = self.head;
        while let Some(current) = node {
            if index >= current.prior_count {
                let prior = Self {
                    head: current.previous,
                };
                let mut visible = current.prior_count;
                for (position, candidate) in current.layers.iter().enumerate() {
                    if prior.contains(candidate)
                        || current.layers[..position]
                            .iter()
                            .any(|layer| layer.budget.counters.ptr_eq(&candidate.budget.counters))
                    {
                        continue;
                    }
                    if visible == index {
                        return Some(candidate);
                    }
                    visible += 1;
                }
                return None;
            }
            node = current.previous;
        }
        None
    }
    pub(super) fn iter(self) -> impl ExactSizeIterator<Item = &'a ActiveDecodeBudgetLayer> {
        (0..self.len()).map(move |index| {
            self.get(index)
                .expect("active layer index was counted from the same borrowed chain")
        })
    }
}

pub(super) fn with_active<R>(body: impl for<'a> FnOnce(BudgetLayers<'a>) -> R) -> R {
    ACTIVE.with(|slot| {
        // SAFETY: only `with_layers` installs this pointer. Its stack node and
        // borrowed context remain live until the closure ends; the unwind guard
        // unlinks before either can drop. HRTB prevents returning a borrowed node.
        let head = unsafe { slot.get().as_ref() };
        body(BudgetLayers { head })
    })
}

/// Run `body` with `layers` appended to the active scope chain.
///
/// Only the call of `body` depends on the caller's types. Building and
/// installing the node live in the non-generic helpers below, so the many
/// decode call sites share one copy of that code instead of instantiating it
/// (and its thread-local accessors) once per closure type.
pub(super) fn with_layers<R>(
    layers: &[ActiveDecodeBudgetLayer],
    depth: usize,
    body: impl FnOnce() -> R,
) -> R {
    let previous = ActiveScope::current();
    // SAFETY: the previous synchronous scope encloses this call and cannot end
    // until it returns/unwinds. No mutable reference to its node is created.
    let node = chained_node(layers, unsafe { previous.node.as_ref() });
    // The node stays in this stack place through `body`; `restore` unlinks it
    // from TLS on return and unwind, before the node can drop.
    let restore = install(&node, previous, depth);
    let value = body();
    drop(restore);
    value
}

/// The active node pointer and nesting depth saved while a nested scope runs.
#[derive(Clone, Copy)]
struct ActiveScope {
    node: *const ScopeNode<'static>,
    depth: usize,
}

impl ActiveScope {
    fn current() -> Self {
        Self {
            node: ACTIVE.with(Cell::get),
            depth: DECODE_NESTING_DEPTH.with(Cell::get),
        }
    }
}

/// Restores the saved scope when an installed node leaves scope, including on unwind.
struct Restore {
    previous: ActiveScope,
    _not_send: PhantomData<Rc<()>>,
}

impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| slot.set(self.previous.node));
        DECODE_NESTING_DEPTH.with(|slot| slot.set(self.previous.depth));
    }
}

/// Build the node that extends `parent` with the layers not already visible in it.
fn chained_node<'a>(
    layers: &'a [ActiveDecodeBudgetLayer],
    parent: Option<&'a ScopeNode<'a>>,
) -> ScopeNode<'a> {
    let prior = BudgetLayers { head: parent };
    let mut visible = [None; VISIBLE_LAYERS];
    for (slot, layer) in visible.iter_mut().zip(prior.iter()) {
        *slot = Some(layer);
    }
    let mut count = prior.len();
    for (position, candidate) in layers.iter().enumerate() {
        if !prior.contains(candidate)
            && !layers[..position]
                .iter()
                .any(|layer| layer.budget.counters.ptr_eq(&candidate.budget.counters))
        {
            if let Some(slot) = visible.get_mut(count) {
                *slot = Some(candidate);
            }
            count = count
                .checked_add(1)
                .expect("live layer slice count fits usize");
        }
    }
    // Every supplied layer tightens the summary, including one the view hides
    // as a duplicate counter. The summary is then never looser than the view,
    // and a value it turns away still reaches the exact ordered walk.
    let tightest = layers.iter().fold(
        parent.map_or(TightestLimits::UNBOUNDED, |parent| parent.tightest),
        TightestLimits::tightened_by,
    );
    ScopeNode {
        previous: parent,
        layers,
        prior_count: prior.len(),
        count,
        visible,
        tightest,
    }
}

/// Make `node` the active scope until the returned guard drops.
///
/// The caller keeps `node` in place and drops the guard before the node.
fn install(node: &ScopeNode<'_>, previous: ActiveScope, depth: usize) -> Restore {
    let restore = Restore {
        previous,
        _not_send: PhantomData,
    };
    // The erased lifetime is confined to TLS and every dereference is bounded
    // by the returned guard.
    ACTIVE.with(|slot| slot.set(core::ptr::from_ref(node).cast::<ScopeNode<'static>>()));
    DECODE_NESTING_DEPTH.with(|slot| slot.set(previous.depth.max(depth)));
    restore
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_cached_view_matches_walk() -> usize {
        with_active(|layers| {
            for index in 0..=layers.len() {
                let cached = layers.get(index).map(core::ptr::from_ref);
                let walked = layers.walk(index).map(core::ptr::from_ref);
                assert_eq!(cached, walked, "index {index} of {}", layers.len());
            }
            layers.len()
        })
    }

    fn nest(contexts: &[DecodeBudgetContext], visible: &mut Vec<usize>) {
        let Some((first, rest)) = contexts.split_first() else {
            return;
        };
        first.with(|| {
            visible.push(assert_cached_view_matches_walk());
            // Reapplying an active context adds no visible layer.
            first.with(|| visible.push(assert_cached_view_matches_walk()));
            nest(rest, visible);
        });
    }

    /// Recompute the summary from the visible layers of the active chain.
    fn walked_tightest_limits() -> (u64, u64, usize) {
        with_active(|layers| {
            layers.iter().fold(
                (u64::MAX, u64::MAX, usize::MAX),
                |(field, sequence, depth), layer| {
                    let limits = layer.budget.limits;
                    (
                        field.min(limit_to_u64(limits.max_field_bytes())),
                        sequence.min(limit_to_u64(limits.max_sequence_elements())),
                        depth.min(layer.base_depth.saturating_add(limits.max_nesting_depth())),
                    )
                },
            )
        })
    }

    fn assert_tightest_limits_match_walk() {
        let cached = tightest_limits();
        assert_eq!(
            (
                cached.field_bytes,
                cached.sequence_elements,
                cached.depth_ceiling
            ),
            walked_tightest_limits()
        );
    }

    #[test]
    fn tightest_limits_summarize_every_layer_and_restore_with_the_scope() {
        assert_tightest_limits_match_walk();
        assert_eq!(tightest_limits().field_bytes, u64::MAX);
        let wide = DecodeBudgetContext::new(DecodeLimits::new(100, 4_096, 1_000, 1 << 20, 9));
        let narrow_field = DecodeBudgetContext::new(DecodeLimits::new(500, 64, 1_000, 1 << 20, 30));
        let narrow_sequence =
            DecodeBudgetContext::new(DecodeLimits::new(7, 8_192, 1_000, 1 << 20, 3));
        wide.with(|| {
            assert_tightest_limits_match_walk();
            assert_eq!(tightest_limits().field_bytes, 4_096);
            assert_eq!(tightest_limits().depth_ceiling, 9);
            narrow_field.with(|| {
                assert_tightest_limits_match_walk();
                assert_eq!(tightest_limits().field_bytes, 64);
                assert_eq!(tightest_limits().sequence_elements, 100);
                narrow_sequence.with(|| {
                    assert_tightest_limits_match_walk();
                    assert_eq!(tightest_limits().field_bytes, 64);
                    assert_eq!(tightest_limits().sequence_elements, 7);
                    assert_eq!(tightest_limits().depth_ceiling, 3);
                    // A wider layer entered inside cannot relax the chain.
                    wide.with(assert_tightest_limits_match_walk);
                });
                assert_eq!(tightest_limits().sequence_elements, 100);
            });
            assert_eq!(tightest_limits().field_bytes, 4_096);
        });
        assert_eq!(tightest_limits().depth_ceiling, usize::MAX);
        // An unbounded layer saturates instead of wrapping its depth ceiling.
        let unbounded = DecodeBudgetContext::new(DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ));
        let nested = DecodeBudgetContext::new(DecodeLimits::new(1, 1, 1, 1, 1));
        nested.with(|| {
            let _depth = DecodeDepthGuard::enter().expect("one level fits");
            let inner = DecodeBudgetContext::new(DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            ));
            inner.with(assert_tightest_limits_match_walk);
        });
        unbounded.with(|| assert_eq!(tightest_limits().depth_ceiling, usize::MAX));
    }

    #[test]
    fn cached_layer_view_matches_chain_walk_with_duplicates_and_long_chains() {
        let limits = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 32);
        let contexts = (0..VISIBLE_LAYERS + 3)
            .map(|_| DecodeBudgetContext::new(limits))
            .collect::<Vec<_>>();
        let mut visible = Vec::new();
        nest(&contexts, &mut visible);
        let expected = (1..=contexts.len())
            .flat_map(|depth| [depth, depth])
            .collect::<Vec<_>>();
        assert_eq!(visible, expected);
        assert_eq!(assert_cached_view_matches_walk(), 0);
        // A multi-layer node mixes new, duplicated and previously active counters.
        let shared = DecodeBudgetContext::new(limits);
        shared.with(|| {
            let layers = [
                contexts[0].layer.clone(),
                shared.layer.clone(),
                contexts[0].layer.clone(),
                contexts[1].layer.clone(),
            ];
            with_layers(&layers, 0, || {
                assert_eq!(assert_cached_view_matches_walk(), 3);
            });
        });
    }
}
