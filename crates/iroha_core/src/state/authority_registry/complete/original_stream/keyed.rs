//! Materialize an immutable radix version from the original State publication journals.
//! Roots produced here are provisional until matched to an independently certified execution.

use super::*;
use crate::state::authority_registry::keyed_commitment::{
    TableShape, selected_schema::SelectedStateSchema,
};
use iroha_data_model::sumeragi_finality::keyed_state::{
    KeyedStateBuilderV1, KeyedStateError, KeyedStateTreeV1, StateKeyV1,
};

/// Keep resource refusal distinct from source or schema divergence.
#[derive(Debug, thiserror::Error)]
pub(in crate::state) enum KeyedCaptureError {
    /// Original State source refusal.
    #[error(transparent)]
    Source(#[from] CaptureError),
    /// The exact original bytes cannot be applied to this map.
    #[error(transparent)]
    Tree(#[from] KeyedStateError),
    /// An uninstalled table contains state, an installed owner has no source, or
    /// the supplied predecessor disagrees with an original undo entry.
    #[error("keyed State source mismatch at {0}")]
    SourceMismatch(String),
}

impl Collector {
    fn build_keyed(
        &self,
        schema: &SelectedStateSchema,
        storage_budget: &AllocationBudget,
    ) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
        let mut builder = KeyedStateBuilderV1::new(schema.digest, storage_budget);
        let mut installed = 0;
        for field in self.fields.as_slice() {
            let Some((_, descriptor)) = schema.schema.table(field.id) else {
                // Uninstalled tables must be empty. An uninstalled singleton may still
                // physically hold its implementation default; it has no selected key.
                let is_table = matches!(
                    Collector::canonical(field.id)?.map(|f| f.role),
                    Some(Role::Canonical(Canonical::Table { .. }))
                );
                if is_table && !field.current.as_slice().is_empty() {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                continue;
            };
            installed += 1;
            if descriptor.shape() == TableShape::Cell && field.current.as_slice().len() != 1 {
                return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
            }
            for row in field.current.as_slice() {
                schema
                    .schema
                    .check_key(field.id, row.key.bytes())
                    .map_err(|_| KeyedCaptureError::SourceMismatch(field.id.into()))?;
                let key = StateKeyV1 {
                    table: field.id.into(),
                    key: row.key.bytes().to_vec(),
                };
                let value = row
                    .value
                    .as_ref()
                    .ok_or_else(|| KeyedCaptureError::SourceMismatch(field.id.into()))?;
                builder.push(&key, value.bytes())?;
            }
        }
        if installed != schema.schema.tables().len() {
            return Err(KeyedCaptureError::SourceMismatch(
                "installed schema has no encoder".into(),
            ));
        }
        Ok(builder.finish()?)
    }

    fn advance_keyed(
        &self,
        schema: &SelectedStateSchema,
        predecessor: &KeyedStateTreeV1,
    ) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
        let mut next = predecessor.clone();
        for field in self.fields.as_slice() {
            if field.replacement {
                return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
            }
            if schema.schema.table(field.id).is_none() {
                if !field.undo.as_slice().is_empty() {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                continue;
            }
            if field.current.as_slice().len() != field.undo.as_slice().len() {
                return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
            }
            for (old, new) in field.undo.as_slice().iter().zip(field.current.as_slice()) {
                let bytes = old.key.bytes();
                if new.key.bytes() != bytes || schema.schema.check_key(field.id, bytes).is_err() {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                let key = StateKeyV1 {
                    table: field.id.into(),
                    key: bytes.to_vec(),
                };
                let before = old.value.as_ref().map(Frame::bytes);
                let after = new.value.as_ref().map(Frame::bytes);
                if predecessor.get(&key) != before {
                    return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
                }
                if before != after {
                    next.replace(&key, after)?;
                }
            }
        }
        Ok(next)
    }
}

/// Check the execution policy opened from the predecessor before a transaction can use
/// locally materialized policy. The complete selected State is authoritative after adoption.
/// Physical settings excluded by the registered semantic projection do not enter this check.
pub(in crate::state) fn verify_keyed_execution_policy(
    fields: &crate::state::StateBlockFields<'_>,
    predecessor: &KeyedStateTreeV1,
) -> Result<(), KeyedCaptureError> {
    let budget = fields.state_ref.ivm_execution_budget();
    let capacity = budget.limit_bytes();
    let limits = CaptureLimits {
        max_fields: FIELD_COUNT,
        max_rows: FIELD_COUNT * 2,
        max_payload_bytes: capacity,
        max_total_payload_bytes: capacity,
        max_source_visits: FIELD_COUNT * 2,
    };
    let mut policies = Collector::new(budget, limits, CaptureIntent::Complete)?;
    policies.executing = true;
    super::policies::append(fields, &mut policies)?;
    for field in policies.fields.as_slice() {
        let key = StateKeyV1 {
            table: field.id.into(),
            key: Vec::new(),
        };
        let [row] = field.current.as_slice() else {
            return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
        };
        if row.value.as_ref().map(Frame::bytes).is_none()
            || predecessor.get(&key) != row.value.as_ref().map(Frame::bytes)
        {
            return Err(KeyedCaptureError::SourceMismatch(field.id.into()));
        }
    }
    Ok(())
}

/// Capture the certified execution or publication cut from the same immutable original overlay.
/// Incremental callers must supply the matching committed predecessor at this exact block start.
pub(in crate::state) fn capture_keyed_image(
    fields: &crate::state::StateBlockFields<'_>,
    schema: &SelectedStateSchema,
    base: Option<&KeyedStateTreeV1>,
) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
    capture_image(fields, schema, base, membership::append_executing)
}

/// Capture the actual published State from a pristine ordinary native acquisition.
/// Committed transaction history is distinct from the acquired writer's next staged height.
pub(in crate::state) fn capture_committed_keyed_image(
    fields: &crate::state::StateBlockFields<'_>,
    schema: &SelectedStateSchema,
) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
    capture_image(fields, schema, None, membership::append_committed)
}

fn capture_image(
    fields: &crate::state::StateBlockFields<'_>,
    schema: &SelectedStateSchema,
    base: Option<&KeyedStateTreeV1>,
    read_membership: fn(
        &crate::state::StateBlockFields<'_>,
        &mut Collector,
    ) -> Result<(), CaptureError>,
) -> Result<KeyedStateTreeV1, KeyedCaptureError> {
    let executing = match fields.world.publication {
        AggregatePublication::Executing => true,
        AggregatePublication::Frozen => false,
        _ => return Err(CaptureError::Changed("keyed State source is not readable").into()),
    };
    let mode = fields.world.parameters.mode();
    let predecessor = base
        .filter(|tree| mode == mv::BlockMode::Ordinary && tree.schema_digest() == schema.digest);
    let intent = if predecessor.is_some() {
        CaptureIntent::Changes
    } else {
        CaptureIntent::Complete
    };
    let budget = fields.state_ref.ivm_execution_budget.clone();
    let limits = CaptureLimits {
        max_fields: usize::MAX,
        max_rows: usize::MAX,
        max_payload_bytes: budget.limit_bytes(),
        max_total_payload_bytes: budget.limit_bytes(),
        max_source_visits: usize::MAX,
    };
    let mut output = Collector::new(budget, limits, intent)?;
    output.executing = executing;
    world::append_world(&fields.world, &fields.state_ref.world, &mut output)?;
    cells::append_fields(fields, &mut output)?;
    policies::append(fields, &mut output)?;
    read_membership(fields, &mut output)?;
    output.require_complete()?;
    if let Some(tree) = predecessor {
        let base = output
            .membership_delta_base
            .as_ref()
            .ok_or(CaptureError::Missing("state.transactions"))?;
        if base.committed_frontier() != base.logical_predecessor_frontier() {
            return Err(CaptureError::Changed("state.transactions").into());
        }
        output.advance_keyed(schema, tree)
    } else {
        output.build_keyed(
            schema,
            fields
                .state_ref
                .world
                .sccp_committee_generations
                .allocation_budget(),
        )
    }
}
