//! Complete deletion demand for the original removal engine and undo insertion.

use super::*;
use crate::internals::bptree::cursor::{remove_tracking_slots, CursorCheckpoint};

#[path = "borrowed_delete.rs"]
mod borrowed_delete;

/// Refusal before either original private map or its owned query changes.
#[derive(Debug)]
pub enum PairRemoveError<E> {
    /// A generation, payload bound or concrete allocation sum cannot be planned.
    Planning(PlanningError),
    /// The one complete current/undo demand was refused.
    Refused(E),
}

struct RemovePlan {
    current: EditPlan,
    undo: Option<EditPlan>,
    demand: AllocationDemand,
}

// SAFETY: each inspected node is immutable and retained by the original cursor.
// Count copies without allocating, cloning payloads or retaining tree pointers.
unsafe fn plan_node_copy<K, V, P>(
    node: *mut Node<K, V, P::Charge>,
    demand: &mut AllocationDemand,
    separator: &mut AllocationDemand,
) -> Result<(), PlanningError>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    if unsafe { &*node }.is_leaf() {
        let leaf = unsafe { &*node.cast::<Leaf<K, V, P::Charge>>() };
        demand.add_layout(Layout::new::<CachePadded<Leaf<K, V, P::Charge>>>())?;
        for index in 0..leaf.count() {
            let (key, value) = leaf
                .get_kv_idx_checked(index)
                .expect("initialized leaf prefix");
            let copied = key_demand::<K, V, P>(key)?;
            demand.add(copied, 1)?;
            separator.include_max(copied);
            P::plan_value(value, demand)?;
        }
    } else {
        let branch = unsafe { &*node.cast::<Branch<K, V, P::Charge>>() };
        demand.add_layout(Layout::new::<CachePadded<Branch<K, V, P::Charge>>>())?;
        for index in 0..branch.count() {
            let copied = key_demand::<K, V, P>(branch.key_at(index))?;
            demand.add(copied, 1)?;
            separator.include_max(copied);
        }
        // A repair may move a child whose minimum was not an old separator.
        for index in 0..=branch.count() {
            let minimum = unsafe { &*Node::min_raw(branch.get_idx_unchecked(index)) };
            separator.include_max(key_demand::<K, V, P>(minimum)?);
        }
    }
    Ok(())
}

fn plan_remove<K, V, P>(
    cursor: &CursorWrite<K, V, Prepaid<P>>,
    key: &K,
) -> Result<EditPlan, PlanningError>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V>,
{
    cursor.assert_operable();
    let mut demand = AllocationDemand::new();
    let mut separator = AllocationDemand::new();
    let mut height = 0usize;
    let mut node = cursor.get_root();
    loop {
        // SAFETY: exclusive cursor retains the entire original tree. Each path
        // node and the canonical adjacent sibling are considered once per level.
        unsafe { plan_node_copy::<K, V, P>(node, &mut demand, &mut separator)? };
        if unsafe { &*node }.is_leaf() {
            break;
        }
        height = height.checked_add(1).ok_or(PlanningError::Overflow)?;
        if height >= usize::BITS as usize {
            return Err(PlanningError::Overflow);
        }
        let branch = unsafe { &*node.cast::<Branch<K, V, P::Charge>>() };
        let selected = branch.locate_node(key);
        let sibling = if selected == 0 { 1 } else { selected - 1 };
        unsafe {
            plan_node_copy::<K, V, P>(
                branch.get_idx_unchecked(sibling),
                &mut demand,
                &mut separator,
            )?;
        }
        node = branch.get_idx_unchecked(selected);
    }
    // Same canonical deletion: a leaf merge needs no separator clone. Reserve
    // one conservative leaf-level rekey plus two per higher repair level.
    let repairs = if height == 0 {
        0
    } else {
        height
            .checked_mul(2)
            .and_then(|n| n.checked_sub(1))
            .ok_or(PlanningError::Overflow)?
    };
    demand.add(separator, repairs)?;
    let [first_required, last_required] =
        remove_tracking_slots(height).ok_or(PlanningError::Overflow)?;
    let [(first_len, first_capacity), (last_len, last_capacity)] = cursor.admitted_tracking();
    let first =
        plan_tracking_growth::<K, V, P>(first_len, first_capacity, first_required, &mut demand)?;
    let last =
        plan_tracking_growth::<K, V, P>(last_len, last_capacity, last_required, &mut demand)?;
    Ok(EditPlan {
        demand,
        first,
        last,
    })
}

fn plan_remove_pair<K, V, P>(
    current: &CursorWrite<K, V, Prepaid<P>>,
    undo: &CursorWrite<K, Option<V>, Prepaid<P>>,
    key: &K,
) -> Result<RemovePlan, PlanningError>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    current.assert_operable();
    undo.assert_operable();
    checked_next_generation(current.get_txid()).ok_or(PlanningError::Overflow)?;
    let missing = !undo.contains_key(key);
    if missing {
        checked_next_generation(undo.get_txid()).ok_or(PlanningError::Overflow)?;
    }
    let current_plan = plan_remove::<K, V, P>(current, key)?;
    let mut demand = current_plan.demand;
    let undo_plan = if missing {
        let plan = plan_edit::<K, Option<V>, P>(undo, key)?;
        demand.add(plan.demand, 1)?;
        <P as ClonePlanning<K, Option<V>>>::plan_key(key, &mut demand)?;
        if let Some(value) = current.search(key) {
            <P as ClonePlanning<K, V>>::plan_value(value, &mut demand)?;
        }
        Some(plan)
    } else {
        None
    };
    Ok(RemovePlan {
        current: current_plan,
        undo: undo_plan,
        demand,
    })
}

fn execute_remove_pair<K, V, P>(
    current: &mut CursorCheckpoint<'_, K, V, Prepaid<P>>,
    undo: Option<&mut CursorCheckpoint<'_, K, Option<V>, Prepaid<P>>>,
    plan: RemovePlan,
    key: K,
    mut provider: P,
) -> Option<V>
where
    K: Clone + Ord + Debug,
    V: Clone,
    P: ClonePlanning<K, V> + ClonePlanning<K, Option<V>>,
{
    // Preserve the original first value before deleting current, including an
    // explicit None for absent-key deletion. Existing undo is untouched.
    let undo_input = if plan.undo.is_some() {
        let copied_key = <P as NodeCloning<K, Option<V>>>::clone_key(&mut provider, &key);
        let previous = current
            .as_ref()
            .search(&key)
            .map(|value| <P as NodeCloning<K, V>>::clone_value(&mut provider, value));
        Some((copied_key, previous))
    } else {
        None
    };
    let (current_cursor, saved) = current.edit_parts();
    let first = allocate_tracking::<K, V, P>(plan.current.first, &mut provider);
    let last = allocate_tracking::<K, V, P>(plan.current.last, &mut provider);
    current_cursor.begin_admitted_edit();
    current_cursor.resume_admitted_funding(provider, first, last, Some(saved));
    let previous = current_cursor
        .try_remove(&key)
        .unwrap_or_else(|_| unreachable!("complete removal tracking bound under original writer"));
    provider = current_cursor.take_completed_admitted_funding();
    let undo_cursor = match (undo, plan.undo, undo_input) {
        (Some(checkpoint), Some(plan), Some((key, value))) => {
            let (cursor, saved) = checkpoint.edit_parts();
            let replaced = execute_edit(cursor, key, value, provider, plan, Some(saved));
            provider = cursor.take_completed_admitted_funding();
            assert!(
                replaced.is_none(),
                "planned first removal touch replaced undo"
            );
            Some(cursor)
        }
        (None, None, None) => None,
        _ => unreachable!("exact private undo plan and checkpoint"),
    };
    // Both children and the borrowed-pair guard remain armed through destruction
    // of the owned query and unused provider, including a caught user destructor.
    drop(key);
    drop(provider);
    current_cursor.seal_joined_admitted_edit();
    if let Some(cursor) = undo_cursor {
        cursor.seal_joined_admitted_edit();
    }
    previous
}
