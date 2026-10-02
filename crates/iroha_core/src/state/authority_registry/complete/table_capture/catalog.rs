//! Exact static table groups, including the indivisible membership pair and cell.

use super::*;
use norito::NoritoSchema;

const MEMBERSHIP_OWNER: &str = "state.transactions";
const MEMBERSHIP_FRONTIER: &str = "state.transactions.frontier";
const CURRENT: &str = "state.transactions.current";
const ROLLBACK: &str = "state.transactions.rollback";

/// Reviewed one-table reader or the closed original-writer membership group.
#[derive(Clone, Copy)]
pub(super) enum TableMaterializer {
    /// One existing canonical State table callback.
    Single {
        id: &'static str,
        capture: fn(&State, LeafLimits) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError>,
    },
    /// One of three semantic readers sharing one validated native World borrow.
    MusubiSemantic(MusubiSemanticTable),
    /// One acquisition returns current, rollback, frontier and original identity.
    TransactionMembership,
}

impl TableMaterializer {
    pub(super) fn table_ids(self) -> impl Iterator<Item = &'static str> {
        match self {
            Self::Single { id, .. } => [Some(id), None],
            Self::MusubiSemantic(table) => [Some(table.id()), None],
            Self::TransactionMembership => [Some(CURRENT), Some(ROLLBACK)],
        }
        .into_iter()
        .flatten()
    }
}

fn is_canonical_table(fields: &'static [Field], id: &str) -> bool {
    let mut found = false;
    visit(fields, &mut |field| {
        if field.id == id && matches!(field.role, Role::Canonical(Canonical::Table { .. })) {
            found = true;
        }
    });
    found
}

fn is_exact_norito<T: NoritoSchema>(schema: super::super::Schema) -> bool {
    matches!(schema, super::super::Schema::Norito { nominal_name, layout }
        if layout == super::super::V1_LAYOUT && nominal_name() == norito::schema::identity::nominal_name::<T>())
}

fn exact_membership_owner(fields: &'static [Field]) -> bool {
    let mut owners = 0;
    let mut frontiers = 0;
    let mut valid = false;
    visit(fields, &mut |field| {
        if field.id == MEMBERSHIP_FRONTIER {
            frontiers += 1;
        }
        if field.id != MEMBERSHIP_OWNER {
            return;
        }
        owners += 1;
        let Role::Canonical(Canonical::Owner(children)) = field.role else {
            return;
        };
        if children.len() != 3
            || children[0].id != MEMBERSHIP_FRONTIER
            || children[1].id != CURRENT
            || children[2].id != ROLLBACK
        {
            return;
        }
        valid = matches!(children[0].role, Role::Canonical(Canonical::Cell(schema))
            if is_exact_norito::<u64>(schema))
            && children[1..].iter().all(|child| matches!(child.role,
                Role::Canonical(Canonical::Table { key, value })
                    if is_exact_norito::<iroha_crypto::HashOf<iroha_data_model::prelude::TransactionEntrypoint>>(key)
                        && is_exact_norito::<u64>(value)));
    });
    owners == 1 && frontiers == 1 && valid
}

/// Validate all flattened table identities and required group companions first.
pub(super) fn require_exact_table_materializers(
    fields: &'static [Field],
    materializers: &[TableMaterializer],
) -> Result<usize, TableCaptureError> {
    // A group contains at most two outputs, but no unchecked multiplication is
    // needed. The hard bound counts retained table slots, not callback entries.
    let count = materializers
        .iter()
        .flat_map(|owner| owner.table_ids())
        .take(MAX_TABLE_MATERIALIZERS + 1)
        .count();
    if count > MAX_TABLE_MATERIALIZERS {
        return Err(TableCaptureError::MaterializerLimit);
    }
    for materializer in materializers {
        for id in materializer.table_ids() {
            if !is_canonical_table(fields, id) {
                return Err(TableCaptureError::UnexpectedMaterializer(id));
            }
        }
        if matches!(
            materializer,
            TableMaterializer::Single {
                id: CURRENT | ROLLBACK,
                ..
            }
        ) {
            return Err(TableCaptureError::MalformedMembershipGroup);
        }
    }
    let mut error = None;
    let mut position = 0_usize;
    visit(fields, &mut |field| {
        if error.is_some() || !matches!(field.role, Role::Canonical(Canonical::Table { .. })) {
            return;
        }
        match materializers
            .iter()
            .flat_map(|owner| owner.table_ids())
            .filter(|id| *id == field.id)
            .count()
        {
            0 => error = Some(TableCaptureError::MissingMaterializer(field.id)),
            1 => {
                if materializers
                    .iter()
                    .flat_map(|owner| owner.table_ids())
                    .nth(position)
                    != Some(field.id)
                {
                    error = Some(TableCaptureError::DisplacedMaterializer(field.id));
                }
                position += 1;
            }
            _ => error = Some(TableCaptureError::DuplicateMaterializer(field.id)),
        }
    });
    if let Some(error) = error {
        return Err(error);
    }
    if materializers
        .iter()
        .any(|owner| matches!(owner, TableMaterializer::TransactionMembership))
        && !exact_membership_owner(fields)
    {
        return Err(TableCaptureError::MalformedMembershipGroup);
    }
    Ok(count)
}
