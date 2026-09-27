//! Original finite custody for the four fixed KAGEMUSHA operation indexes.
//!
//! Keys and values contain no nested allocations. Current nodes, undo preimages,
//! child touches and overlapping retained versions use the same original pool.
//! TODO: complete native mutex, pending-waiter and bootstrap-control admission
//! before treating this component as complete production N0 admission.

use super::{Storage, WorldData};
use concread::bptree::{
    AllocationDemand, ClonePlanning, NodeCloning, NodeFunding, PlanningError, Prepaid,
};
use mv::{
    allocation::{AllocationBudget, AllocationCharge, AllocationReservation},
    storage::{AdmittedStorageError, AdmittedStoragePolicy},
};
use std::alloc::Layout;

/// Closed copying policy for fixed 32-byte operation identifiers only.
pub(crate) struct OperationIndexPolicy(AllocationReservation);
impl NodeFunding for OperationIndexPolicy {
    type Charge = AllocationCharge;
    fn take_node_charge(&mut self, layout: Layout) -> Self::Charge {
        self.0
            .try_split(layout)
            .expect("exact original fixed-index demand")
    }
}
macro_rules! fixed_value_policy {
    ($value:ty) => {
        impl NodeCloning<[u8; 32], $value> for OperationIndexPolicy {
            fn clone_key(&mut self, key: &[u8; 32]) -> [u8; 32] {
                *key
            }
            fn clone_value(&mut self, value: &$value) -> $value {
                *value
            }
        }
        impl ClonePlanning<[u8; 32], $value> for OperationIndexPolicy {
            fn plan_key(_: &[u8; 32], _: &mut AllocationDemand) -> Result<(), PlanningError> {
                Ok(())
            }
            fn plan_value(_: &$value, _: &mut AllocationDemand) -> Result<(), PlanningError> {
                Ok(())
            }
        }
    };
}
fixed_value_policy!([u8; 32]);
fixed_value_policy!(Option<[u8; 32]>);
impl AdmittedStoragePolicy for OperationIndexPolicy {
    fn from_admission(admission: AllocationReservation) -> Self {
        Self(admission)
    }
    fn admission(&self) -> &AllocationReservation {
        &self.0
    }
}

pub(crate) type OperationIndexMode = Prepaid<OperationIndexPolicy>;
pub(crate) type OperationIndex = Storage<[u8; 32], [u8; 32], OperationIndexMode>;

/// Test-only empty block acquisition using the real finite pool and owned scope.
#[cfg(test)]
pub(super) trait OperationIndexFixtureBlock {
    /// Acquire a test writer with the same admitted production kernel.
    fn block(&self) -> mv::storage::Block<'_, [u8; 32], [u8; 32], OperationIndexMode>;
}

#[cfg(test)]
impl OperationIndexFixtureBlock for OperationIndex {
    fn block(&self) -> mv::storage::Block<'_, [u8; 32], [u8; 32], OperationIndexMode> {
        let scope = self
            .allocation_budget()
            .try_owned_refund_scope()
            .expect("fixture operation-index scope admission");
        let mut slot = self
            .try_block_acquisition_owned(&scope)
            .expect("fixture operation-index pool identity");
        slot.try_initialize(mv::BlockMode::Ordinary)
            .expect("fixture operation-index writer admission");
        slot.into_block()
    }
}

pub(crate) fn default_budget() -> AllocationBudget {
    let bytes =
        iroha_config::parameters::defaults::nexus::storage::KAGEMUSHA_OPERATION_INDEX_BYTES.get();
    AllocationBudget::new(
        usize::try_from(bytes).expect("fixed-index default fits supported targets"),
    )
}

macro_rules! initial_world_field {
    (kagemusha_mint_credit_operations, $budget:ident) => {
        OperationIndex::try_new_admitted($budget.clone())?
    };
    (kagemusha_issuance_operations, $budget:ident) => {
        OperationIndex::try_new_admitted($budget.clone())?
    };
    (kagemusha_redemption_id_operations, $budget:ident) => {
        OperationIndex::try_new_admitted($budget.clone())?
    };
    (kagemusha_terminal_nullifier_operations, $budget:ident) => {
        OperationIndex::try_new_admitted($budget.clone())?
    };
    ($field:ident, $budget:ident) => {
        Default::default()
    };
}
macro_rules! initial_world {
    ($budget:ident; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
        WorldData {
            $($prefix: initial_world_field!($prefix, $budget),)*
            $($privacy: initial_world_field!($privacy, $budget),)*
            $($suffix: initial_world_field!($suffix, $budget),)*
            external_event_buf: Default::default(),
        }
    };
}
impl WorldData {
    pub(super) fn try_new_with_operation_index_budget(
        budget: AllocationBudget,
    ) -> Result<Self, AdmittedStorageError> {
        Ok(with_world_overlay_fields!(initial_world, budget))
    }
}
impl Default for WorldData {
    fn default() -> Self {
        Self::try_new_with_operation_index_budget(default_budget())
            .expect("configured default admits four empty fixed operation indexes")
    }
}

/// Decode this fixed signed snapshot layout without a temporary map or String.
/// Uppercase digests, strict key ordering and exact punctuation are canonical.
#[derive(Debug, thiserror::Error)]
pub(super) enum OperationIndexRestoreError {
    #[error(transparent)]
    Encoding(#[from] norito::json::Error),
    #[error(transparent)]
    Admission(#[from] AdmittedStorageError),
}

/// Decode one canonical fixed-index field directly into its original finite pool.
pub(super) fn restore_json(
    source: &str,
    budget: AllocationBudget,
) -> Result<OperationIndex, OperationIndexRestoreError> {
    use mv::storage::AdmittedBlockError;
    let mut parser = norito::json::Parser::new(source);
    let result = OperationIndex::try_restore_admitted(budget, |current, undo| {
        literal(&mut parser, b"{\"revert\":{")?;
        entries(&mut parser, |parser, key| {
            let value = if parser.peek() == Some(b'n') {
                literal(parser, b"null")?;
                None
            } else {
                Some(digest(parser)?)
            };
            undo(key, value).map_err(OperationIndexRestoreError::Admission)
        })?;
        literal(&mut parser, b",\"blocks\":{")?;
        entries(&mut parser, |parser, key| {
            let value = digest(parser)?;
            current(key, value).map_err(OperationIndexRestoreError::Admission)
        })?;
        literal(&mut parser, b"}")?;
        if !parser.eof() {
            return Err(invalid_snapshot().into());
        }
        Ok(())
    });
    match result {
        Ok(storage) => Ok(storage),
        Err(AdmittedBlockError::Admission(error)) => {
            Err(OperationIndexRestoreError::Admission(error))
        }
        Err(AdmittedBlockError::Callback(error)) => Err(error),
    }
}

fn invalid_snapshot() -> norito::json::Error {
    norito::json::Error::Message("noncanonical fixed operation index snapshot".into())
}
fn literal(
    parser: &mut norito::json::Parser<'_>,
    expected: &[u8],
) -> Result<(), norito::json::Error> {
    for byte in expected {
        if parser.bump() != Some(*byte) {
            return Err(invalid_snapshot());
        }
    }
    Ok(())
}
fn digest(parser: &mut norito::json::Parser<'_>) -> Result<[u8; 32], norito::json::Error> {
    fn nibble(parser: &mut norito::json::Parser<'_>) -> Result<u8, norito::json::Error> {
        match parser.bump() {
            Some(byte @ b'0'..=b'9') => Ok(byte - b'0'),
            Some(byte @ b'A'..=b'F') => Ok(byte - b'A' + 10),
            _ => Err(invalid_snapshot()),
        }
    }
    literal(parser, b"\"")?;
    let mut value = [0; 32];
    for byte in &mut value {
        *byte = (nibble(parser)? << 4) | nibble(parser)?;
    }
    literal(parser, b"\"")?;
    Ok(value)
}
fn entries(
    parser: &mut norito::json::Parser<'_>,
    mut entry: impl FnMut(
        &mut norito::json::Parser<'_>,
        [u8; 32],
    ) -> Result<(), OperationIndexRestoreError>,
) -> Result<(), OperationIndexRestoreError> {
    let mut previous = None;
    if parser.peek() == Some(b'}') {
        return Ok(literal(parser, b"}")?);
    }
    loop {
        let key = digest(parser)?;
        if previous.is_some_and(|previous| previous >= key) {
            return Err(invalid_snapshot().into());
        }
        previous = Some(key);
        literal(parser, b":")?;
        entry(parser, key)?;
        match parser.bump() {
            Some(b',') => {}
            Some(b'}') => return Ok(()),
            _ => return Err(invalid_snapshot().into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_four_operation_indexes_share_the_original_finite_pool() {
        let too_small = AllocationBudget::new(1);
        assert!(WorldData::try_new_with_operation_index_budget(too_small.clone()).is_err());
        assert_eq!(too_small.reserved_bytes(), 0);

        let budget = default_budget();
        let world = WorldData::try_new_with_operation_index_budget(budget.clone()).unwrap();
        let baseline = budget.reserved_bytes();
        assert!(baseline > 0);
        let remaining = budget
            .try_reserve_bytes(budget.limit_bytes() - baseline)
            .unwrap();
        for index in [
            &world.kagemusha_mint_credit_operations,
            &world.kagemusha_issuance_operations,
            &world.kagemusha_redemption_id_operations,
            &world.kagemusha_terminal_nullifier_operations,
        ] {
            assert!(
                index
                    .try_with_admitted_block(|block| {
                        block.try_insert_admitted([7; 32], [8; 32])?;
                        Ok::<_, (([u8; 32], [u8; 32]), AdmittedStorageError)>(())
                    })
                    .is_err()
            );
            assert!(index.view().get(&[7; 32]).is_none());
        }
        drop(remaining);
        assert_eq!(budget.reserved_bytes(), baseline);
        drop(world);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    fn json<T: norito::json::JsonSerialize>(value: &T) -> String {
        norito::json::to_json(value).unwrap()
    }

    #[test]
    fn fixed_index_restore_is_canonical_and_preserves_current_and_absent_preimages() {
        let budget = default_budget();
        let key = [7; 32];
        let value = [11; 32];
        let canonical = format!(
            "{{\"revert\":{{{}:null}},\"blocks\":{{{}:{}}}}}",
            json(&key),
            json(&key),
            json(&value),
        );
        let target = restore_json(&canonical, budget.clone()).unwrap();
        assert_eq!(target.view().get(&key), Some(&value));
        assert_eq!(target.snapshot().revert_map().get(&key), Some(&None));
        assert_eq!(json(&target), canonical);
        drop(target);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn fixed_index_restore_rejects_aliases_duplicates_reordering_and_partial_input() {
        let budget = default_budget();
        let key = json(&[0xAB_u8; 32]);
        let other = json(&[0xCD_u8; 32]);
        let value = json(&[0xEF_u8; 32]);
        for source in [
            format!("{{\"revert\":{{}},\"blocks\":{{{key}:{value},{key}:{value}}}}}"),
            format!("{{\"revert\":{{}},\"blocks\":{{{other}:{value},{key}:{value}}}}}"),
            format!(
                "{{\"revert\":{{}},\"blocks\":{{{}:{value}}}}}",
                key.to_lowercase()
            ),
            format!("{{ \"revert\":{{}},\"blocks\":{{{key}:{value}}}}}"),
            format!("{{\"blocks\":{{{key}:{value}}},\"revert\":{{}}}}"),
            format!("{{\"revert\":{{{key}:null}},\"blocks\":{{{key}:"),
        ] {
            assert!(restore_json(&source, budget.clone()).is_err(), "{source}");
            assert_eq!(
                budget.reserved_bytes(),
                0,
                "private partial restore is retired"
            );
        }
    }
}
