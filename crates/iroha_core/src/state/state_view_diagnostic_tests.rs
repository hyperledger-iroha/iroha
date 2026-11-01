//! A real retired history generation must be released before lifetime diagnostics.

use super::*;
use crate::query::store::LiveQueryStore;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tracing::{Event, Metadata, Subscriber, field::Field, span};

struct WarningProbe {
    pool: iroha_allocation::AllocationBudget,
    pinned_bytes: usize,
    warnings: AtomicUsize,
    generation_released: AtomicBool,
    backtrace_formatted: AtomicBool,
}

struct DiagnosticSubscriber(Arc<WarningProbe>);

impl Subscriber for DiagnosticSubscriber {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.fields().field("held_ms").is_some()
            && metadata.fields().field("backtrace").is_some()
    }
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }
    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
    fn enter(&self, _: &span::Id) {}
    fn exit(&self, _: &span::Id) {}
    fn event(&self, event: &Event<'_>) {
        self.0.warnings.fetch_add(1, Ordering::SeqCst);
        self.0.generation_released.store(
            self.0.pool.reserved_bytes() < self.0.pinned_bytes,
            Ordering::SeqCst,
        );
        event.record(&mut FormatBacktrace(&self.0.backtrace_formatted));
    }
}

struct FormatBacktrace<'a>(&'a AtomicBool);
impl tracing::field::Visit for FormatBacktrace<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
        if field.name() == "backtrace" {
            struct Sink;
            impl core::fmt::Write for Sink {
                fn write_str(&mut self, _: &str) -> core::fmt::Result {
                    Ok(())
                }
            }
            core::fmt::write(&mut Sink, format_args!("{value:?}")).unwrap();
            self.0.store(true, Ordering::SeqCst);
        }
    }
}

fn replace_hash(state: &State, value: u8) {
    let mut next = state.block_hashes.block_and_revert();
    next.push_for_tests(HashOf::from_untyped_unchecked(Hash::prehashed(
        [value; Hash::LENGTH],
    )));
    next.commit_for_tests();
}

#[test]
fn state_view_diagnostic_releases_retired_snapshot_before_backtrace() {
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    replace_hash(&state, 1);
    let pool = state.block_hashes.budget.clone();

    // Positive control: this is the actual history snapshot's retained charge,
    // not a proxy guard or an assertion about the struct's field ordering.
    let control = state.view();
    replace_hash(&state, 2);
    let pinned = pool.reserved_bytes();
    drop(control);
    assert!(pool.reserved_bytes() < pinned);

    let mut view = state.view();
    replace_hash(&state, 3);
    let probe = Arc::new(WarningProbe {
        pool,
        pinned_bytes: state.block_hashes.budget.reserved_bytes(),
        warnings: AtomicUsize::new(0),
        generation_released: AtomicBool::new(false),
        backtrace_formatted: AtomicBool::new(false),
    });
    // Exercise the real one-second warning without wall-clock sleeps. Into is
    // also the identity conversion for the original Instant representation.
    view._lifetime_diagnostic = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap()
        .into();
    tracing::subscriber::with_default(DiagnosticSubscriber(Arc::clone(&probe)), || {
        drop(view);
    });
    assert_eq!(probe.warnings.load(Ordering::SeqCst), 1);
    assert!(probe.backtrace_formatted.load(Ordering::SeqCst));
    assert!(
        probe.generation_released.load(Ordering::SeqCst),
        "StateView still pins the retired history generation when backtrace diagnostics run"
    );
    assert!(probe.pool.reserved_bytes() < probe.pinned_bytes);
}
