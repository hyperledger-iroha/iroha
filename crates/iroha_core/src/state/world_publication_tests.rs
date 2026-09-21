//! Complete World acquisition, exact retry and current/undo publication controls.

use super::super::publication::{
    FieldRefusal, PreparedWorld, PreparedWorldField, WorldPublicationError,
};
use super::*;
use mv::PublicationPreparationError;

trait FieldImage {
    fn image(&self) -> String;
}

impl<K: Key, V: Value + std::fmt::Debug> FieldImage for Storage<K, V> {
    fn image(&self) -> String {
        let snapshot = self.snapshot();
        // Compare actual values and undo, excluding MV locks/owner identities.
        // Derived indexes need no invented public JSON key codecs for this test.
        format!(
            "{:?}",
            (
                snapshot.current().iter().collect::<Vec<_>>(),
                snapshot.revert_map().iter().collect::<Vec<_>>()
            )
        )
    }
}

impl<V: Value + std::fmt::Debug> FieldImage for Cell<V> {
    fn image(&self) -> String {
        format!("{:?}", (&*self.view(), &*self.predecessor_view()))
    }
}

impl FieldImage for TriggerSet {
    fn image(&self) -> String {
        norito::json::to_json(self).unwrap()
    }
}

fn all_images(world: &World) -> Vec<(&'static str, String)> {
    macro_rules! field_image {
        ($world:ident, nfts) => {
            norito::json::to_json(&$world.nfts).unwrap()
        };
        ($world:ident, rwas) => {
            norito::json::to_json(&$world.rwas).unwrap()
        };
        ($world:ident, $field:ident) => {
            $world.$field.image()
        };
    }
    macro_rules! images {
        ($world:ident; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
            vec![
                $((stringify!($prefix), field_image!($world, $prefix)),)*
                $((stringify!($privacy), field_image!($world, $privacy)),)*
                $((stringify!($suffix), field_image!($world, $suffix)),)*
            ]
        };
    }
    with_world_overlay_fields!(images, world)
}

fn prepare<'a, A>(journal: DetachedWorld<A>, target: &'a World) -> PreparedWorld<'a, A, ()> {
    journal
        .try_prepare_publication(target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|(_, error)| panic!("World preparation refused: {error:?}"))
}

fn physical_custody<A>(journal: &DetachedWorld<A>) -> (usize, usize, Vec<usize>) {
    (
        journal.fields.as_ptr() as usize,
        journal.fields.capacity(),
        journal
            .fields
            .iter()
            .map(|field| std::ptr::from_ref(field.as_ref()).cast::<()>() as usize)
            .collect(),
    )
}

fn mutate(original: &mut WorldBlock<'_>, value: u8, trigger: &str) {
    let mut child = original.transaction_without_telemetry(LaneConfig::default(), 1);
    child
        .smart_contract_state
        .insert(path("capture/value"), vec![value]);
    *child.soradns_last_publish_ms.get_mut() = Some(u64::from(value));
    child.apply();
    register_trigger(original, trigger);
    let mut aborted = original.transaction_without_telemetry(LaneConfig::default(), 1);
    aborted
        .smart_contract_state
        .insert(path("capture/aborted"), vec![99]);
}

#[test]
fn complete_world_preparation_holds_every_inventory_writer_and_matches_direct_commit() {
    let world = fixture();
    let direct = fixture();
    let before = all_images(&world);
    let reader = world.smart_contract_state.view();
    let undo = world.smart_contract_state.snapshot();
    let probes = capture(world.block()).fields;
    let mut original = world.block();
    let mut reference = direct.block();
    mutate(&mut original, 2, "world_publish");
    mutate(&mut reference, 2, "world_publish");
    let prepared = prepare(capture(original), &world);
    assert_eq!(all_images(&world), before);
    assert_eq!(probes.len(), 278);
    // Probe each original field separately, so an early busy field cannot hide
    // a missing writer later in the heterogeneous World inventory.
    for probe in probes {
        let name = probe.summary().name;
        let (_, refusal) = probe.try_prepare(&world).err().expect("all writers held");
        assert_eq!(refusal.field, name);
        assert!(matches!(
            refusal.cause,
            PublicationPreparationError::Busy(_)
        ));
    }
    reference.commit();
    prepared.publish();
    assert_eq!(all_images(&world), all_images(&direct));
    assert_eq!(reader.get(&path("capture/value")), Some(&vec![1]));
    assert_eq!(undo.revert_map().get(&path("capture/value")), Some(&None));
    assert_all_writers_released(&world);
}

#[test]
fn replacement_world_restores_untouched_tip_only_owners_and_exact_undo() {
    for touched in [false, true] {
        let world = fixture();
        let direct = fixture();
        for owner in [&world, &direct] {
            let mut tip = owner.block();
            mutate(&mut tip, 20, "discarded_world_tip");
            tip.smart_contract_state
                .insert(path("capture/tip_only"), vec![3]);
            tip.commit();
        }
        let mut original = world.block_and_revert();
        let mut reference = direct.block_and_revert();
        if touched {
            mutate(&mut original, 30, "replacement_world");
            mutate(&mut reference, 30, "replacement_world");
        }
        let journal = capture(original);
        assert_eq!(journal.mode(), BlockMode::Replace);
        let prepared = prepare(journal, &world);
        reference.commit();
        prepared.publish();
        assert_eq!(all_images(&world), all_images(&direct));
        assert!(
            world
                .smart_contract_state
                .view()
                .get(&path("capture/tip_only"))
                .is_none()
        );
        world.block_and_revert().commit();
        direct.block_and_revert().commit();
        assert_eq!(all_images(&world), all_images(&direct));
    }
}

#[test]
fn complete_world_abort_and_drop_keep_original_events_and_current_undo_cut() {
    let world = fixture();
    let before = all_images(&world);
    let mut original = world.block();
    mutate(&mut original, 7, "aborted_world");
    original.dataspace_catalog = catalog();
    original.push_pipeline_warning(
        BlockHeader::new(std::num::NonZeroU64::MIN, None, None, 1, 0),
        "world",
        "original event",
    );
    let events = original.external_event_buf.as_ptr();
    let journal = capture(original);
    let expected = journal.fields().collect::<Vec<_>>();
    let custody = physical_custody(&journal);
    let journal = prepare(journal, &world).abort();
    assert_eq!(physical_custody(&journal), custody);
    assert_eq!(journal.external_events().as_ptr(), events);
    assert_eq!(journal.dataspace_catalog(), &catalog());
    assert_eq!(journal.fields().collect::<Vec<_>>(), expected);
    assert!(journal.matches_current(&world));
    assert_all_writers_released(&world);
    assert_eq!(all_images(&world), before);
    drop(prepare(journal, &world));
    assert_all_writers_released(&world);
    assert_eq!(all_images(&world), before);
}

trait WriterHold {}
impl<T> WriterHold for T {}

#[test]
fn every_busy_world_field_releases_all_earlier_writers_and_retains_complete_retry() {
    macro_rules! holders {
        (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
            [
                $((stringify!($prefix), |world: &World| -> Box<dyn WriterHold + '_> { Box::new(world.$prefix.block()) }),)*
                $((stringify!($privacy), |world: &World| -> Box<dyn WriterHold + '_> { Box::new(world.$privacy.block()) }),)*
                $((stringify!($suffix), |world: &World| -> Box<dyn WriterHold + '_> { Box::new(world.$suffix.block()) }),)*
            ]
        };
    }
    let holders: [(&str, for<'a> fn(&'a World) -> Box<dyn WriterHold + 'a>); 278] =
        with_world_overlay_fields!(holders);
    let world = fixture();
    let before = all_images(&world);
    let mut original = world.block();
    mutate(&mut original, 4, "retry_world");
    let mut journal = capture(original);
    let expected = journal.fields().collect::<Vec<_>>();
    let custody = physical_custody(&journal);
    for (name, hold) in holders {
        let held = hold(&world);
        let (retained, error) = journal
            .try_prepare_publication(&world, |_, _| Ok::<_, ()>(()))
            .err()
            .expect("busy original field");
        let WorldPublicationError::Field(refusal) = error else {
            panic!("exact field refusal")
        };
        assert_eq!(refusal.field, name);
        assert!(matches!(
            refusal.cause,
            PublicationPreparationError::Busy(_)
        ));
        assert_eq!(refusal.trigger_component.is_some(), name == "triggers");
        journal = retained;
        assert_eq!(physical_custody(&journal), custody, "{name}");
        assert_eq!(journal.fields().collect::<Vec<_>>(), expected);
        drop(held);
        assert_all_writers_released(&world);
        assert!(journal.matches_current(&world));
    }
    assert_eq!(all_images(&world), before);
    prepare(journal, &world).publish();
    assert_eq!(
        world
            .smart_contract_state
            .view()
            .get(&path("capture/value")),
        Some(&vec![4])
    );
}

struct Installation {
    world: Arc<World>,
    dropped: Arc<AtomicBool>,
}
impl Drop for Installation {
    fn drop(&mut self) {
        assert_all_writers_released(&self.world);
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[test]
fn late_world_identity_change_and_capacity_refusal_preserve_journals_and_guard_order() {
    let world = fixture();
    let journal = capture(world.block());
    let expected = journal.fields().collect::<Vec<_>>();
    let custody = physical_custody(&journal);
    let (journal, error) = journal
        .try_prepare_publication(&world, |_, _| Err::<(), _>("capacity"))
        .err()
        .expect("capacity refusal");
    assert!(matches!(
        error,
        WorldPublicationError::Admission("capacity")
    ));
    assert_eq!(journal.fields().collect::<Vec<_>>(), expected);
    assert_eq!(physical_custody(&journal), custody);
    let last = *expected.last().unwrap();
    // Acquire/commit through the actual inventory-generated accessor, including
    // an untouched last field; equal values still rotate the exact undo owner.
    macro_rules! invalidators {
        (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
            [
                $((stringify!($prefix), |world: &World| world.$prefix.block().commit()),)*
                $((stringify!($privacy), |world: &World| world.$privacy.block().commit()),)*
                $((stringify!($suffix), |world: &World| world.$suffix.block().commit()),)*
            ]
        };
    }
    let invalidators: [(&str, fn(&World)); 278] = with_world_overlay_fields!(invalidators);
    let (name, invalidate) = invalidators.last().unwrap();
    assert_eq!(*name, last.name);
    let dropped = Arc::new(AtomicBool::new(false));
    let (journal, error) = journal
        .try_prepare_publication(&world, |_, target| {
            invalidate(target);
            Ok::<_, ()>(Installation {
                world: Arc::clone(&world),
                dropped: Arc::clone(&dropped),
            })
        })
        .err()
        .expect("last original owner changed during admission");
    let WorldPublicationError::Field(refusal) = error else {
        panic!("exact changed field")
    };
    assert_eq!(refusal.field, last.name);
    assert_eq!(refusal.cause, PublicationPreparationError::Changed);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(journal.fields().collect::<Vec<_>>(), expected);
    assert_eq!(physical_custody(&journal), custody);
    assert!(!journal.matches_current(&world));
}

#[test]
fn world_publication_hands_original_extras_and_both_resource_guards_to_aggregate() {
    for operation in ["drop", "abort", "publish"] {
        let world = fixture();
        let captured = Arc::new(AtomicBool::new(false));
        let installed = Arc::new(AtomicBool::new(false));
        let mut original = world.block();
        mutate(&mut original, 9, "guarded_world");
        original.dataspace_catalog = catalog();
        original.push_pipeline_warning(
            BlockHeader::new(std::num::NonZeroU64::MIN, None, None, 1, 0),
            "world",
            "retained original",
        );
        let events = original.external_event_buf.as_ptr();
        let journal = original
            .try_detach_journals(|_| Ok::<_, ()>(Reservation(Arc::clone(&captured))))
            .unwrap();
        let prepared = journal
            .try_prepare_publication(&world, |_, _| {
                Ok::<_, ()>(Installation {
                    world: Arc::clone(&world),
                    dropped: Arc::clone(&installed),
                })
            })
            .unwrap_or_else(|_| panic!("prepare actual World"));
        assert!(!captured.load(Ordering::SeqCst));
        assert!(!installed.load(Ordering::SeqCst));
        match operation {
            "drop" => drop(prepared),
            "abort" => {
                let journal = prepared.abort();
                assert!(installed.load(Ordering::SeqCst));
                assert!(!captured.load(Ordering::SeqCst));
                assert_eq!(journal.external_events().as_ptr(), events);
                drop(journal);
            }
            "publish" => {
                let (alias_context, returned_events, admission, installation) = prepared.publish();
                assert_eq!(alias_context, catalog());
                assert_eq!(returned_events.as_ptr(), events);
                assert!(!captured.load(Ordering::SeqCst));
                assert!(!installed.load(Ordering::SeqCst));
                drop(returned_events);
                drop(admission);
                drop(installation);
            }
            _ => unreachable!(),
        }
        assert!(captured.load(Ordering::SeqCst));
        assert!(installed.load(Ordering::SeqCst));
    }
}

#[derive(Clone, Copy, Debug)]
enum UnwindBoundary {
    Prepare,
    Abort,
}

// The witness follows a real original field through preparation and rollback.
// Field order drops that original owner before observing its destruction.
struct OriginalFieldDrop(Option<Arc<AtomicBool>>);
impl Drop for OriginalFieldDrop {
    fn drop(&mut self) {
        if let Some(dropped) = &self.0 {
            dropped.store(true, Ordering::SeqCst);
        }
    }
}

struct UnwindField {
    original: Box<dyn RetainedWorldField>,
    boundary: Option<UnwindBoundary>,
    dropped: OriginalFieldDrop,
}

struct UnwindPreparedField<'target> {
    original: Box<dyn PreparedWorldField + 'target>,
    boundary: Option<UnwindBoundary>,
    dropped: OriginalFieldDrop,
}

impl RetainedWorldField for UnwindField {
    fn summary(&self) -> FieldSummary {
        self.original.summary()
    }

    fn matches_current(&self, target: &World) -> bool {
        self.original.matches_current(target)
    }

    fn try_prepare<'target>(
        self: Box<Self>,
        target: &'target World,
    ) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)>
    {
        if matches!(self.boundary, Some(UnwindBoundary::Prepare)) {
            panic!("injected failure after the preceding real field acquired its writers");
        }
        let Self {
            original,
            boundary,
            dropped,
        } = *self;
        match original.try_prepare(target) {
            Ok(original) => Ok(Box::new(UnwindPreparedField {
                original,
                boundary,
                dropped,
            })),
            Err((original, error)) => Err((
                Box::new(Self {
                    original,
                    boundary,
                    dropped,
                }),
                error,
            )),
        }
    }
}

impl PreparedWorldField for UnwindPreparedField<'_> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        if matches!(self.boundary, Some(UnwindBoundary::Abort)) {
            panic!("injected failure after the preceding real field returned to retry custody");
        }
        let Self {
            original,
            boundary,
            dropped,
        } = *self;
        Box::new(UnwindField {
            original: original.abort(),
            boundary,
            dropped,
        })
    }

    fn publish(self: Box<Self>) {
        self.original.publish();
    }
}

struct UnwindAdmission {
    field_dropped: Arc<AtomicBool>,
    // Zero means live, one means an early refund, and two means payload first.
    refund_order: Arc<AtomicUsize>,
}
impl Drop for UnwindAdmission {
    fn drop(&mut self) {
        self.refund_order.store(
            1 + usize::from(self.field_dropped.load(Ordering::SeqCst)),
            Ordering::SeqCst,
        );
    }
}

#[test]
fn world_publication_unwind_retains_both_admissions_until_original_fields_drop() {
    for boundary in [UnwindBoundary::Prepare, UnwindBoundary::Abort] {
        let world = fixture();
        let before = all_images(&world);
        let field_dropped = Arc::new(AtomicBool::new(false));
        let capture_refund = Arc::new(AtomicUsize::new(0));
        let installation_refund = Arc::new(AtomicUsize::new(0));
        let mut journal = world
            .block()
            .try_detach_journals(|_| {
                Ok::<_, ()>(UnwindAdmission {
                    field_dropped: Arc::clone(&field_dropped),
                    refund_order: Arc::clone(&capture_refund),
                })
            })
            .unwrap();
        assert_eq!(journal.field_count(), 278);
        // A separate read-only capture holds exact original-cut probes for every
        // real field before any unwind can poison its physical writer.
        let probe = capture(world.block());
        // Preparation retains an untouched tail; abort already restored the first
        // original field into its retry vector when the second field panics.
        let observed = match boundary {
            UnwindBoundary::Prepare => journal.fields.len() - 1,
            UnwindBoundary::Abort => 0,
        };
        let original = journal.fields.remove(observed);
        journal.fields.insert(
            observed,
            Box::new(UnwindField {
                original,
                boundary: None,
                dropped: OriginalFieldDrop(Some(Arc::clone(&field_dropped))),
            }),
        );
        let original = journal.fields.remove(1);
        journal.fields.insert(
            1,
            Box::new(UnwindField {
                original,
                boundary: Some(boundary),
                dropped: OriginalFieldDrop(None),
            }),
        );
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let prepared = journal
                .try_prepare_publication(&world, |_, _| {
                    Ok::<_, ()>(UnwindAdmission {
                        field_dropped: Arc::clone(&field_dropped),
                        refund_order: Arc::clone(&installation_refund),
                    })
                })
                .unwrap_or_else(|_| panic!("actual World should prepare before injected failure"));
            drop(prepared.abort());
        }));
        let message = unwind.expect_err("the real field boundary must exercise unwind");
        assert!(
            message
                .downcast_ref::<&str>()
                .is_some_and(|message| message.starts_with("injected failure"))
        );
        assert!(field_dropped.load(Ordering::SeqCst));
        assert_eq!(
            capture_refund.load(Ordering::SeqCst),
            2,
            "{boundary:?}: capture admission refunded before its original field"
        );
        assert_eq!(
            installation_refund.load(Ordering::SeqCst),
            2,
            "{boundary:?}: installation admission refunded before its original field"
        );
        // Unwound std mutex writers deliberately remain poisoned. Check the
        // actual nonblocking publication result rather than trying to reopen a
        // blocking World writer that requires local recovery after that panic.
        for (index, field) in probe.fields.into_iter().enumerate() {
            let name = field.summary().name;
            let poisoned = match boundary {
                UnwindBoundary::Prepare => index == 0,
                UnwindBoundary::Abort => index != 0,
            };
            match field.try_prepare(&world) {
                Ok(prepared) => {
                    assert!(
                        !poisoned,
                        "{boundary:?}: unwound {name} must require recovery"
                    );
                    drop(prepared.abort());
                }
                Err((original, error)) => {
                    assert!(
                        poisoned,
                        "{boundary:?}: untouched/released {name} must remain usable"
                    );
                    assert_eq!(
                        error.cause,
                        PublicationPreparationError::Poisoned,
                        "{boundary:?}: {name} must be poisoned, not busy or changed"
                    );
                    drop(original);
                }
            }
        }
        assert_eq!(all_images(&world), before);
    }
}
