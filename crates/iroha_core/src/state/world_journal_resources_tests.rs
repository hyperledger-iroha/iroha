//! Concrete shell sizes, finite reservations and exact retry custody.

use super::*;
use crate::state::world_journals::publication::WorldPublicationError;

fn observed_capture<A>(world: &DetachedWorld<A>) -> (Layout, Vec<Layout>) {
    (
        Layout::array::<Box<dyn RetainedWorldField>>(world.fields.capacity()).unwrap(),
        world
            .fields
            .iter()
            .map(|field| Layout::for_value(field.as_ref()))
            .collect(),
    )
}

#[test]
fn world_shell_plan_matches_constructed_capture_and_installation_layouts() {
    let demand = WorldJournalShellDemand::plan().unwrap();
    for replacement in [false, true] {
        let world = World::default();
        world.block().commit();
        let original = if replacement {
            world.block_and_revert()
        } else {
            world.block()
        };
        // Only shell geometry is under test. Nested allocations in this isolated
        // World fixture are not a complete production carrier admission.
        let retained = original.try_detach_journals(|_| Ok::<_, ()>(())).unwrap();
        let retained_pointer = retained.fields.as_ptr();
        let retained_boxes = retained
            .fields
            .iter()
            .map(|field| std::ptr::from_ref(field.as_ref()).cast::<()>())
            .collect::<Vec<_>>();
        let (retained_vector, retained_layouts) = observed_capture(&retained);
        assert_eq!(retained.field_count(), demand.field_count());
        assert_eq!(retained.fields.capacity(), demand.field_count());
        assert_eq!(
            retained_vector.size() + retained_layouts.iter().map(Layout::size).sum::<usize>(),
            demand.capture_bytes()
        );
        let prepared = retained
            .try_prepare_publication(&world, None, |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|(_, error, _)| panic!("fixture preparation refused: {error:?}"));
        {
            let (retry_vector, prepared_vector, prepared_layouts) =
                prepared.observed_shell_layouts();
            assert_eq!(retry_vector, retained_vector, "original Vec coexists");
            assert_eq!(
                prepared_vector,
                publication::field_vector_layout(demand.field_count()).unwrap()
            );
            assert_eq!(prepared_layouts.len(), demand.field_count());
            let installation = prepared_vector.size()
                + prepared_layouts.map(|layout| layout.size()).sum::<usize>();
            assert_eq!(installation, demand.installation_bytes());
            assert_eq!(
                demand.total_bytes(),
                demand.capture_bytes() + installation,
                "original boxes remain allocated inside the prepared wrappers"
            );
        }
        let retained = prepared.abort().0;
        assert_eq!(retained.fields.as_ptr(), retained_pointer);
        assert_eq!(
            observed_capture(&retained),
            (retained_vector, retained_layouts)
        );
        assert_eq!(
            retained
                .fields
                .iter()
                .map(|field| std::ptr::from_ref(field.as_ref()).cast::<()>())
                .collect::<Vec<_>>(),
            retained_boxes,
            "abort preserves every original Box allocation"
        );
    }
}

#[test]
fn world_shell_reservation_holds_capture_abort_retry_and_refunds_after_drop() {
    // Neither planning nor reservation needs a World, State or writer guard.
    let demand = WorldJournalShellDemand::plan().unwrap();
    let too_small = AllocationBudget::new(demand.total_bytes() - 1);
    assert!(matches!(
        demand.try_reserve(&too_small),
        Err(AllocationRefusal::ExceedsLimit { requested_bytes, limit_bytes })
            if requested_bytes == demand.total_bytes() && limit_bytes + 1 == requested_bytes
    ));
    assert_eq!(too_small.reserved_bytes(), 0);
    let budget = AllocationBudget::new(demand.total_bytes());
    let reservation = demand.try_reserve(&budget).unwrap();
    assert_eq!(reservation.remaining_bytes(), demand.total_bytes());
    assert_eq!(budget.reserved_bytes(), demand.total_bytes());
    let world = World::default();
    let retained = world
        .block()
        .try_detach_journals(move |_| Ok::<_, ()>(reservation))
        .unwrap();
    assert_eq!(retained.admission().remaining_bytes(), demand.total_bytes());
    assert!(matches!(
        demand.try_reserve(&budget),
        Err(AllocationRefusal::Capacity { .. })
    ));
    let prepared = retained
        .try_prepare_publication(&world, None, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|(_, error, _)| panic!("fixture preparation refused: {error:?}"));
    assert_eq!(budget.reserved_bytes(), demand.total_bytes());
    let retained = prepared.abort().0;
    assert_eq!(budget.reserved_bytes(), demand.total_bytes());
    let held_writer = world.soradns_last_publish_ms.block();
    let (retained, error, _cleanup) = retained
        .try_prepare_publication(&world, None, |_, _| Ok::<_, ()>(()))
        .err()
        .expect("busy owner must preserve retained shells and reservation");
    drop(_cleanup);
    assert!(matches!(error, WorldPublicationError::Field(_)));
    assert_eq!(budget.reserved_bytes(), demand.total_bytes());
    drop(held_writer);
    let prepared = retained
        .try_prepare_publication(&world, None, |_, _| Ok::<_, ()>(()))
        .unwrap_or_else(|(_, error, _)| panic!("fixture retry refused: {error:?}"));
    assert_eq!(budget.reserved_bytes(), demand.total_bytes());
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), 0);
    let replacement = demand.try_reserve(&budget).unwrap();
    assert_eq!(budget.reserved_bytes(), demand.total_bytes());
    drop(replacement);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn world_shell_planning_never_reads_targets_or_acquires_held_writers() {
    let (retained, prepared) = field_layouts::<Cell<u64>>(|_| panic!("must not read target"));
    assert_eq!(retained, Layout::new::<RetainedCell<u64>>());
    assert_eq!(prepared, publication::cell_shell_layout::<u64>());
    let expected = WorldJournalShellDemand::plan().unwrap();
    let world = World::default();
    let writers = world.block();
    assert_eq!(WorldJournalShellDemand::plan().unwrap(), expected);
    drop(writers);
}

#[test]
fn world_shell_planning_checks_each_sum_count_and_vector_layout_overflow() {
    let planned = WorldJournalShellDemand::plan().unwrap();
    let layouts = field_layouts::<Cell<u64>>(|_| panic!("must not read target"));
    for (count, capture, installation) in [
        (
            usize::MAX,
            planned.capture_bytes,
            planned.installation_bytes,
        ),
        (planned.fields, usize::MAX, planned.installation_bytes),
        (planned.fields, planned.capture_bytes, usize::MAX),
    ] {
        let mut demand = WorldJournalShellDemand {
            fields: count,
            capture_bytes: capture,
            installation_bytes: installation,
            total_bytes: 0,
        };
        let original = demand;
        assert_eq!(
            demand.add_field(layouts),
            Err(AllocationRefusal::DemandOverflow)
        );
        assert_eq!(demand, original, "overflow cannot partially update a plan");
    }
    assert!(matches!(
        WorldJournalShellDemand {
            fields: usize::MAX,
            ..planned
        }
        .finish(),
        Err(AllocationRefusal::DemandOverflow)
    ));
    assert!(publication::field_vector_layout(usize::MAX).is_err());
    assert!(matches!(
        WorldJournalShellDemand {
            fields: 0,
            capture_bytes: usize::MAX,
            installation_bytes: 1,
            total_bytes: 0,
        }
        .finish(),
        Err(AllocationRefusal::DemandOverflow)
    ));
}
