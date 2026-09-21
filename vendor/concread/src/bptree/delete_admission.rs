//! Complete deletion demand for the original removal engine and undo insertion.

use super::*;
use crate::internals::bptree::cursor::CursorCheckpoint;

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
