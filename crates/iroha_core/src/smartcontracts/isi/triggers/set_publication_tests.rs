//! Complete trigger publication and exact retry after late component refusal.

use super::super::publication::SetPublicationError;
use super::*;
use mv::PublicationPreparationError;

fn contract_touch_pointer<Admission>(
    journal: &DetachedSet<Admission>,
) -> *const HashOf<IvmBytecode> {
    std::ptr::from_ref(
        journal
            .contracts()
            .touched_entries()
            .next()
            .expect("actual contract touch")
            .key,
    )
}

fn prepare_trigger_publication<Admission>(
    journal: DetachedSet<Admission>,
    target: &Set,
) -> super::super::publication::PreparedSet<'_, Admission, ()> {
    journal
        .try_prepare_publication(target, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|(_, error)| panic!("trigger publication: {error:?}"))
}

#[test]
fn complete_trigger_publication_matches_all_ten_direct_current_and_undo_images() {
    let set = seeded_set();
    let direct = seeded_set();
    let before = images(&set);
    let mut original = set.block();
    let mut reference = direct.block();
    mutate_all(&mut original);
    mutate_all(&mut reference);
    reference.commit();
    let journal = capture(original);
    let pointer = contract_touch_pointer(&journal);
    let prepared = prepare_trigger_publication(journal, &set);
    assert_eq!(images(&set), before, "preparation publishes no component");
    let journal = prepared.abort();
    assert_eq!(contract_touch_pointer(&journal), pointer);
    assert!(journal.matches_current(&set));
    assert_all_writers_released(&set);
    prepare_trigger_publication(journal, &set).publish();
    assert_eq!(images(&set), images(&direct));
    assert_all_writers_released(&set);
}

#[test]
fn complete_trigger_replacement_restores_untouched_discarded_tip_owners() {
    for touched in [false, true] {
        let set = seeded_set();
        let direct = seeded_set();
        for target in [&set, &direct] {
            let mut block = target.block();
            mutate_all(&mut block);
            block.commit();
        }
        let mut candidate = set.block_and_revert();
        let mut reference = direct.block_and_revert();
        if touched {
            for block in [&mut candidate, &mut reference] {
                let mut tx = block.transaction();
                assert!(tx.remove(&id("call_a")));
                register_call(&mut tx, "replacement");
                tx.apply();
            }
        }
        reference.commit();
        prepare_trigger_publication(capture(candidate), &set).publish();
        assert_eq!(images(&set), images(&direct));
        set.block_and_revert().commit();
        direct.block_and_revert().commit();
        assert_eq!(
            images(&set),
            images(&direct),
            "replacement preserves real undo for later rollback"
        );
    }
}

#[test]
fn busy_at_each_trigger_component_returns_all_original_journals_and_releases_earlier_writers() {
    let set = seeded_set();
    let before = images(&set);
    let mut block = set.block();
    mutate_all(&mut block);
    let mut journal = capture(block);
    let pointer = contract_touch_pointer(&journal);
    macro_rules! busy {
        ($field:ident) => {{
            let busy = set.$field.block();
            let (returned, error) = journal
                .try_prepare_publication(&set, |_, _| Ok::<_, ()>(()))
                .err()
                .expect("busy component");
            assert!(
                matches!(
                    error,
                    SetPublicationError::Component {
                        field: stringify!($field),
                        cause: PublicationPreparationError::Busy(_),
                    }
                ),
                "{error:?}"
            );
            drop(busy);
            returned
        }};
    }
    for component in 0..10 {
        journal = match component {
            0 => busy!(data_triggers),
            1 => busy!(pipeline_triggers),
            2 => busy!(time_triggers),
            3 => busy!(by_call_triggers),
            4 => busy!(ids),
            5 => busy!(active_data_trigger_ids),
            6 => busy!(active_pipeline_trigger_ids),
            7 => busy!(active_time_trigger_ids),
            8 => busy!(active_by_call_trigger_ids),
            _ => busy!(contracts),
        };
        assert_eq!(contract_touch_pointer(&journal), pointer);
        assert_eq!(images(&set), before);
        assert!(journal.matches_current(&set));
        assert_all_writers_released(&set);
    }
    prepare_trigger_publication(journal, &set).publish();
    assert_ne!(images(&set), before);
}

#[test]
fn trigger_installation_refusal_and_late_identity_change_keep_every_original_delta() {
    let set = seeded_set();
    let mut block = set.block();
    mutate_all(&mut block);
    let journal = capture(block);
    let pointer = contract_touch_pointer(&journal);
    let (journal, error) = journal
        .try_prepare_publication(&set, |_, _| {
            assert_all_writers_released(&set);
            Err::<(), _>("capacity")
        })
        .err()
        .expect("resource refusal");
    assert!(matches!(error, SetPublicationError::Admission("capacity")));
    let installation_released = Arc::new(AtomicBool::new(false));
    let (journal, error) = journal
        .try_prepare_publication(&set, |_, target| {
            target.contracts.block().commit();
            Ok::<_, ()>(TriggerInstallation {
                set: Arc::clone(&set),
                released: Arc::clone(&installation_released),
            })
        })
        .err()
        .expect("late changed component");
    assert!(matches!(
        error,
        SetPublicationError::Component {
            field: "contracts",
            cause: PublicationPreparationError::Changed,
        }
    ));
    assert_eq!(contract_touch_pointer(&journal), pointer);
    assert_all_writers_released(&set);
    assert!(installation_released.load(Ordering::SeqCst));
    assert!(!journal.matches_current(&set));
    assert!(journal.data_triggers().matches_current(&set.data_triggers));
}

struct TriggerInstallation {
    set: Arc<Set>,
    released: Arc<AtomicBool>,
}
impl Drop for TriggerInstallation {
    fn drop(&mut self) {
        assert_all_writers_released(&self.set);
        self.released.store(true, Ordering::SeqCst);
    }
}

#[test]
fn trigger_resource_guards_outlive_all_writers_on_drop_abort_and_publication() {
    for action in 0..3 {
        let set = seeded_set();
        let before = images(&set);
        let capture_released = Arc::new(AtomicBool::new(false));
        let installation_released = Arc::new(AtomicBool::new(false));
        let mut original = set.block();
        mutate_all(&mut original);
        let journal = original
            .try_detach(|_| Ok::<_, ()>(Reservation(Arc::clone(&capture_released))))
            .unwrap();
        let prepared = journal
            .try_prepare_publication(&set, |_, _| {
                Ok::<_, ()>(TriggerInstallation {
                    set: Arc::clone(&set),
                    released: Arc::clone(&installation_released),
                })
            })
            .unwrap_or_else(|_| panic!("prepare complete trigger publication"));
        match action {
            0 => drop(prepared),
            1 => {
                let journal = prepared.abort();
                assert!(!capture_released.load(Ordering::SeqCst));
                assert!(installation_released.load(Ordering::SeqCst));
                assert!(journal.matches_current(&set));
                drop(journal);
            }
            _ => {
                let guards = prepared.publish();
                assert!(!capture_released.load(Ordering::SeqCst));
                assert!(!installation_released.load(Ordering::SeqCst));
                assert_ne!(images(&set), before);
                drop(guards);
            }
        }
        assert!(capture_released.load(Ordering::SeqCst));
        assert!(installation_released.load(Ordering::SeqCst));
        if action != 2 {
            assert_eq!(images(&set), before);
        }
        assert_all_writers_released(&set);
    }
}
