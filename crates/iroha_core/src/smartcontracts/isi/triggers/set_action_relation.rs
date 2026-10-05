//! One borrowed action/ID/active inverse over both original images.
//! Admission is a conservative local source-event/full-byte proxy, not CPU or gas.

use super::contract_relation::{CheckedStrategy, Image, SemanticFailure, visit_original};
use super::*;
use crate::state::authority_registry::original_images::RawStorageImages;

/// The existing four canonical action projections; indexes add no authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ActionTable {
    /// Data-event actions.
    Data,
    /// Pipeline-event actions.
    Pipeline,
    /// Time-event actions with original retry fields.
    Time,
    /// Explicit trigger-call actions.
    ByCall,
}
impl ActionTable {
    pub(super) const ALL: [Self; 4] = [Self::Data, Self::Pipeline, Self::Time, Self::ByCall];
    fn kind(self) -> TriggeringEventType {
        match self {
            Self::Data => TriggeringEventType::Data,
            Self::Pipeline => TriggeringEventType::Pipeline,
            Self::Time => TriggeringEventType::Time,
            Self::ByCall => TriggeringEventType::ExecuteTrigger,
        }
    }
}
#[derive(Clone, Copy)]
struct ActionBits<'a> {
    repeats: &'a Repeats,
    metadata: &'a Metadata,
}
/// Only actual closed native readers can construct this borrowed source set.
pub(super) struct NativeActions<'a, D, P, T, C, I, AD, AP, AT, AC> {
    pub(super) data: &'a D,
    pub(super) pipeline: &'a P,
    pub(super) time: &'a T,
    pub(super) by_call: &'a C,
    pub(super) ids: &'a I,
    pub(super) active_data: &'a AD,
    pub(super) active_pipeline: &'a AP,
    pub(super) active_time: &'a AT,
    pub(super) active_by_call: &'a AC,
}
trait Sources {
    fn actions<'a>(
        &'a self,
        table: ActionTable,
        image: Image,
        work: &mut CheckedStrategy,
        inspect: impl FnMut(
            &'a TriggerId,
            ActionBits<'a>,
            &mut CheckedStrategy,
        ) -> Result<(), TriggerContractError>,
    ) -> Result<(), TriggerContractError>;
    fn ids<'a>(
        &'a self,
        image: Image,
        work: &mut CheckedStrategy,
        inspect: impl FnMut(
            &'a TriggerId,
            &'a TriggeringEventType,
            &mut CheckedStrategy,
        ) -> Result<(), TriggerContractError>,
    ) -> Result<(), TriggerContractError>;
    fn active<'a>(
        &'a self,
        table: ActionTable,
        image: Image,
        work: &mut CheckedStrategy,
        inspect: impl FnMut(&'a TriggerId, &mut CheckedStrategy) -> Result<(), TriggerContractError>,
    ) -> Result<(), TriggerContractError>;
}
impl<D, P, T, C, I, AD, AP, AT, AC> Sources for NativeActions<'_, D, P, T, C, I, AD, AP, AT, AC>
where
    D: RawStorageImages<TriggerId, LoadedAction<DataEventFilter>>,
    P: RawStorageImages<TriggerId, LoadedAction<PipelineEventFilterBox>>,
    T: RawStorageImages<TriggerId, LoadedAction<TimeEventFilter>>,
    C: RawStorageImages<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    I: RawStorageImages<TriggerId, TriggeringEventType>,
    AD: RawStorageImages<TriggerId, ()>,
    AP: RawStorageImages<TriggerId, ()>,
    AT: RawStorageImages<TriggerId, ()>,
    AC: RawStorageImages<TriggerId, ()>,
{
    fn actions<'a>(
        &'a self,
        table: ActionTable,
        image: Image,
        work: &mut CheckedStrategy,
        mut inspect: impl FnMut(
            &'a TriggerId,
            ActionBits<'a>,
            &mut CheckedStrategy,
        ) -> Result<(), TriggerContractError>,
    ) -> Result<(), TriggerContractError> {
        work.admit_action_work(1)?; // typed source dispatch before access
        macro_rules! rows {
            ($field:ident) => {
                visit_original(self.$field, image, work, |id, action, work| {
                    work.admit_action_work(3)?; // callback plus two field borrows
                    inspect(
                        id,
                        ActionBits {
                            repeats: &action.repeats,
                            metadata: &action.metadata,
                        },
                        work,
                    )
                })
            };
        }
        match table {
            ActionTable::Data => rows!(data),
            ActionTable::Pipeline => rows!(pipeline),
            ActionTable::Time => rows!(time),
            ActionTable::ByCall => rows!(by_call),
        }
    }
    fn ids<'a>(
        &'a self,
        image: Image,
        work: &mut CheckedStrategy,
        inspect: impl FnMut(
            &'a TriggerId,
            &'a TriggeringEventType,
            &mut CheckedStrategy,
        ) -> Result<(), TriggerContractError>,
    ) -> Result<(), TriggerContractError> {
        visit_original(self.ids, image, work, inspect)
    }
    fn active<'a>(
        &'a self,
        table: ActionTable,
        image: Image,
        work: &mut CheckedStrategy,
        mut inspect: impl FnMut(&'a TriggerId, &mut CheckedStrategy) -> Result<(), TriggerContractError>,
    ) -> Result<(), TriggerContractError> {
        work.admit_action_work(1)?;
        macro_rules! rows {
            ($field:ident) => {
                visit_original(self.$field, image, work, |id, _, work| inspect(id, work))
            };
        }
        match table {
            ActionTable::Data => rows!(active_data),
            ActionTable::Pipeline => rows!(active_pipeline),
            ActionTable::Time => rows!(active_time),
            ActionTable::ByCall => rows!(active_by_call),
        }
    }
}
fn fail(failure: SemanticFailure) -> TriggerContractError {
    TriggerContractError::Semantic(failure)
}
fn increment(count: &mut usize, work: &mut CheckedStrategy) -> Result<(), TriggerContractError> {
    work.admit_action_work(3 + 2 * std::mem::size_of::<usize>() as u64)?; // read/add/write, both scalar footprints
    *count = count
        .checked_add(1)
        .ok_or(TriggerContractError::CounterGeometry)?;
    Ok(())
}
fn find<'a>(
    sources: &'a impl Sources,
    id: &TriggerId,
    image: Image,
    work: &mut CheckedStrategy,
) -> Result<(usize, Option<(ActionTable, ActionBits<'a>)>), TriggerContractError> {
    work.admit_action_work(2)?; // scalar state construction and phase loop
    let mut count = 0;
    let mut found = None;
    for table in ActionTable::ALL {
        sources.actions(table, image, work, |candidate, action, work| {
            let equal = work.compare_action_ids(candidate, id)?;
            work.admit_action_work(1)?;
            if equal {
                increment(&mut count, work)?;
                work.admit_action_work(1)?;
                found = Some((table, action));
            }
            Ok(())
        })?;
    }
    Ok((count, found))
}
fn enabled(
    action: ActionBits<'_>,
    work: &mut CheckedStrategy,
) -> Result<bool, TriggerContractError> {
    work.admit_action_work(3)?; // helper, repeats predicate and short-circuit branch
    if action.repeats.is_depleted() {
        return Ok(false);
    }
    work.admit_action_work(1)?; // original borrowed Metadata iterator construction
    let mut rows = action.metadata.iter();
    let mut value = None;
    loop {
        work.admit_action_work(3)?; // loop, physical next, Option branch including terminal
        let Some((key, candidate)) = rows.next() else {
            break;
        };
        work.admit_action_work(4)?; // name borrow, length, literal borrow, equality
        let name: &str = key.as_ref();
        work.admit_action_work(
            u64::try_from(name.len()).map_err(|_| TriggerContractError::WorkLimit)?,
        )?;
        work.admit_action_work(9)?; // all bytes of the source-owned __enabled key
        let equal = name == super::super::TRIGGER_ENABLED_METADATA_KEY;
        work.admit_action_work(1)?;
        if equal {
            work.admit_action_work(1)?;
            value = Some(candidate);
        }
    }
    work.admit_action_work(11)?; // caller/entry/Option3, two callback/propagation/pattern triples6, raw predicate1, return1
    super::super::trigger_enabled_from_value(value, |scalar, value| {
        // Parser shared67 + checked callback10, bool fixed15+prefix27 or u64 fixed26, plus Json getters/length3.
        // Complete text scans: depth<=4N, whitespace<=3N, position<=N (+u64 digits<=N);
        // each visited byte contributes its footprint and one scan event.
        // These are conservative local capsules, not CPU or physical memory tariffs.
        let (fixed, multiplier) = match scalar {
            super::super::EnabledScalar::Bool => (122_u64, 16_u64),
            super::super::EnabledScalar::U64 => (106, 18),
        };
        // The fixed getter/length/dispatch envelope is admitted before observing the text length.
        work.admit_action_work(fixed)?;
        let bytes =
            u64::try_from(value.get().len()).map_err(|_| TriggerContractError::WorkLimit)?;
        let units = bytes
            .checked_mul(multiplier)
            .ok_or(TriggerContractError::WorkLimit)?;
        work.admit_action_work(units)
    })
}
fn forward(
    sources: &impl Sources,
    table: ActionTable,
    id: &TriggerId,
    action: ActionBits<'_>,
    image: Image,
    work: &mut CheckedStrategy,
) -> Result<(), TriggerContractError> {
    let (count, _) = find(sources, id, image, work)?;
    work.admit_action_work(1)?;
    if count != 1 {
        return Err(fail(SemanticFailure::ActionDuplicate));
    }
    let mut count = 0;
    let mut kind = None;
    sources.ids(image, work, |candidate, value, work| {
        let equal = work.compare_action_ids(candidate, id)?;
        work.admit_action_work(1)?;
        if equal {
            increment(&mut count, work)?;
            work.admit_action_work(1)?;
            kind = Some(*value);
        }
        Ok(())
    })?;
    work.admit_action_work(1)?;
    if count != 1 {
        return Err(fail(SemanticFailure::ActionIdMissing));
    }
    work.admit_action_work(3)?; // kind derivation, Option comparison and branch
    if kind != Some(table.kind()) {
        return Err(fail(SemanticFailure::ActionKind));
    }
    let expected = enabled(action, work)?;
    let mut count = 0;
    sources.active(table, image, work, |candidate, work| {
        let equal = work.compare_action_ids(candidate, id)?;
        work.admit_action_work(1)?;
        if equal {
            increment(&mut count, work)?;
        }
        Ok(())
    })?;
    work.admit_action_work(2)?; // complete inverse count/expected comparison and branch
    if count != usize::from(expected) {
        return Err(fail(if expected {
            SemanticFailure::ActionActiveMissing
        } else {
            SemanticFailure::ActionActiveUnexpected
        }));
    }
    Ok(())
}
/// Contract checks first, then forward typed actions, IDs and all active reverse tails.
fn validate_image(
    sources: &impl Sources,
    image: Image,
    work: &mut CheckedStrategy,
) -> Result<(), TriggerContractError> {
    work.admit_action_work(1)?;
    for table in ActionTable::ALL {
        sources.actions(table, image, work, |id, action, work| {
            forward(sources, table, id, action, image, work)
        })?;
    }
    sources.ids(image, work, |id, kind, work| {
        let (count, found) = find(sources, id, image, work)?;
        work.admit_action_work(1)?;
        if count == 0 {
            return Err(fail(SemanticFailure::ActionOrphan));
        }
        work.admit_action_work(1)?;
        if count != 1 {
            return Err(fail(SemanticFailure::ActionDuplicate));
        }
        work.admit_action_work(3)?;
        if found.map(|(table, _)| table.kind()) != Some(*kind) {
            return Err(fail(SemanticFailure::ActionKind));
        }
        Ok(())
    })?;
    for table in ActionTable::ALL {
        sources.active(table, image, work, |id, work| {
            let (count, found) = find(sources, id, image, work)?;
            work.admit_action_work(2)?;
            if count != 1 {
                return Err(fail(SemanticFailure::ActionActiveUnexpected));
            }
            let Some((actual, action)) = found else {
                return Err(fail(SemanticFailure::ActionActiveUnexpected));
            };
            work.admit_action_work(2)?;
            if actual != table {
                return Err(fail(SemanticFailure::ActionActiveUnexpected));
            }
            let eligible = enabled(action, work)?;
            work.admit_action_work(1)?;
            if !eligible {
                return Err(fail(SemanticFailure::ActionActiveUnexpected));
            }
            Ok(())
        })?;
    }
    Ok(())
}
#[cfg(test)]
#[path = "set_action_relation/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "set_action_relation/work_tests.rs"]
mod work_tests;

impl<D, P, T, C, I, AD, AP, AT, AC> NativeActions<'_, D, P, T, C, I, AD, AP, AT, AC>
where
    D: RawStorageImages<TriggerId, LoadedAction<DataEventFilter>>,
    P: RawStorageImages<TriggerId, LoadedAction<PipelineEventFilterBox>>,
    T: RawStorageImages<TriggerId, LoadedAction<TimeEventFilter>>,
    C: RawStorageImages<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    I: RawStorageImages<TriggerId, TriggeringEventType>,
    AD: RawStorageImages<TriggerId, ()>,
    AP: RawStorageImages<TriggerId, ()>,
    AT: RawStorageImages<TriggerId, ()>,
    AC: RawStorageImages<TriggerId, ()>,
{
    /// Validate only these original closed readers, retaining both-image call order at the owner.
    pub(super) fn validate(
        &self,
        image: Image,
        work: &mut CheckedStrategy,
    ) -> Result<(), TriggerContractError> {
        validate_image(self, image, work)
    }
}

/// Scheduling allowance only; actual complete physical scans debit the retained strategy.
/// This grants no new accepted row/byte limit and cannot infer a successful scan cost.
pub(crate) fn scheduled_work(limits: crate::state::authority_registry::leaf::LeafLimits) -> u64 {
    let rows = limits.max_rows.saturating_add(1);
    super::contract_source_work(limits)
        .saturating_add(
            rows.saturating_mul(rows)
                .saturating_mul(256)
                .saturating_mul(29 * usize::BITS as u64 + 44),
        )
        .saturating_add(limits.max_streamed_value_bytes.saturating_mul(64))
}
