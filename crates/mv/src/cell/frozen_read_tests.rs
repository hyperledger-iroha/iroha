//! Frozen original cell reads retain source identity through real publication retries.

use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::SeqCst},
};

#[derive(Debug)]
struct Payload {
    value: String,
    copies: Arc<AtomicUsize>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        self.copies.fetch_add(1, SeqCst);
        Self {
            value: self.value.clone(),
            copies: Arc::clone(&self.copies),
        }
    }
}

fn payload(value: &str, copies: &Arc<AtomicUsize>) -> Payload {
    Payload {
        value: value.into(),
        copies: Arc::clone(copies),
    }
}

#[test]
fn frozen_cell_reads_original_payloads_with_free_writers_and_after_busy_retry() {
    let copies = Arc::new(AtomicUsize::new(0));
    let target = Cell::new(payload("before", &copies));
    let mut block = target.block();
    let identity = block.publication_identity();
    block.get_mut().value = "after".into();
    let before = block.get_before_block().value.as_ptr();
    let after = block.get().value.as_ptr();
    let original = block.try_detach(|_| Ok::<_, ()>(())).unwrap();
    copies.store(0, SeqCst);
    let assert_original = |journal: &Detached<Payload, ()>| {
        assert_eq!(journal.publication_identity(), identity);
        assert_eq!(journal.get().value, "after");
        assert_eq!(journal.get().value.as_ptr(), after);
        assert_eq!(journal.get_before_block().value, "before");
        assert_eq!(journal.get_before_block().value.as_ptr(), before);
    };
    assert_original(&original);
    {
        let undo = target
            .revert
            .try_write()
            .expect("original undo writer released");
        let current = target
            .blocks
            .try_write()
            .expect("original current writer released");
        // These reads neither acquire a current view nor wait for either writer.
        assert_original(&original);
        drop((undo, current));
    }
    let held = target.blocks.write();
    // The independent lock probes above are ordinary target writers and may
    // clone their own current value; none becomes the retained publication.
    copies.store(0, SeqCst);
    let (original, refusal, cleanup) = original
        .try_prepare_publication(&target, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("real current writer must refuse");
    assert!(matches!(refusal, PublicationPreparationError::Busy(_)));
    assert_original(&original);
    assert!(
        target.revert.try_write().is_some(),
        "partial undo acquisition released"
    );
    drop(held);
    drop(cleanup);
    let prepared = original
        .try_prepare_publication(&target, |journal, _| {
            assert_original(journal);
            Ok::<_, ()>(())
        })
        .unwrap_or_else(|(_, error, _)| panic!("same original retry: {error:?}"));
    drop(prepared.publish());
    assert_eq!(
        copies.load(SeqCst),
        0,
        "freeze/read/retry/publication copied no payload"
    );
    assert_eq!(target.view().value.as_ptr(), after);
    assert_eq!(
        target.predecessor_view().as_ref().unwrap().value.as_ptr(),
        before
    );
}

#[test]
fn frozen_cell_keeps_original_reads_when_equal_source_or_new_generation_is_refused() {
    let target = Cell::new(10_u64);
    let foreign = Cell::new(10_u64);
    let original = target.block().try_detach(|_| Ok::<_, ()>(())).unwrap();
    let identity = original.publication_identity();
    assert_eq!(*original.get_before_block(), 10);
    assert!(std::ptr::eq(original.get(), original.get_before_block()));
    let (original, refusal, cleanup) = original
        .try_prepare_publication(&foreign, |_, _| -> Result<(), ()> {
            panic!("foreign source must fail before admission")
        })
        .err()
        .expect("equal foreign source cannot replace original");
    assert_eq!(refusal, PublicationPreparationError::Changed);
    drop(cleanup);
    target.block().commit();
    assert_eq!(
        *target.view(),
        *original.get(),
        "equal values deliberately preserved"
    );
    assert_ne!(target.block().publication_identity(), identity);
    for _ in 0..2 {
        assert_eq!(original.publication_identity(), identity);
        assert_eq!(*original.get(), 10);
    }
    let (original, refusal, cleanup) = original
        .try_prepare_publication(&target, |_, _| {
            Err::<(), _>("must not reach admission after generation change")
        })
        .err()
        .expect("stale original generation must fail");
    assert_eq!(refusal, PublicationPreparationError::Changed);
    drop(cleanup);
    assert_eq!(*original.get(), 10);
    assert_eq!(original.publication_identity(), identity);
}

#[test]
fn frozen_replacement_cell_reads_the_original_cut_even_after_target_drops() {
    let original = {
        let target = Cell::new(String::from("base"));
        let mut tip = target.block();
        *tip.get_mut() = "discarded tip".into();
        tip.commit();
        let original = target
            .block_and_revert()
            .try_detach(|_| Ok::<_, ()>(()))
            .unwrap();
        assert_eq!(original.mode(), BlockMode::Replace);
        assert!(original.touched_value().is_none());
        assert_eq!(&*target.view(), "discarded tip");
        original
    };
    assert_eq!(original.get(), "base");
    assert!(std::ptr::eq(original.get_before_block(), original.get()));
}
