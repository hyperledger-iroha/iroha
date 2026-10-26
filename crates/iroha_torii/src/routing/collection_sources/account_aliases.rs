//! Alias bindings joined onto account rows: `alias_scopes` and `aliases`.
//!
//! The accounts collection stores no alias data; the world indexes the labels of every
//! account (`account_aliases_by_account`). A query pays for the join only when it mentions one
//! of the two fields, so ordinary account reads keep their bytes and cost. An account's bindings
//! are the labels that are active and still resolve to it, shown on the same terms as
//! `POST /v1/aliases/by-account`: an alias of a public dataspace needs nothing, any other needs
//! the signed caller's `CanResolveAccountAlias` for that alias, its domain or (domainless) its
//! dataspace. `alias_scopes` is computed from the same disclosed bindings, so a filter on it
//! cannot reveal an alias the caller may not read.
use super::*;

use iroha_data_model::{
    account::rekey::AccountAlias,
    alias_setup::{AccountAliasName, ResolvedAccountAliasV1},
};
use iroha_torii_shared::list_query::FilterExpr;
use std::cell::Cell;

// Canonical aliases contain at most three bounded Names. Keep room for the
// temporary literal, resolved scope and permission selector before retaining any
// output. SNS record decoding remains charged to the current producer owner.
const ALIAS_TRANSIENT_BYTES: usize = 16 * iroha_model_base::name::MAX_NAME_BYTES + 64;
const MAX_SCOPE_BYTES: usize = 2 * iroha_model_base::name::MAX_NAME_BYTES + 1;

/// Preserve the SNS decoder's typed admission refusal before its HTTP projection.
/// SNS classifies its own record/format ceilings as completed errors. An enclosing
/// decoder refusal here came from the admitted collection worker's cumulative
/// owner; only that case can be served again under a larger owner.
pub(super) fn alias_producer_sns_error(error: iroha_core::sns::SnsError) -> Error {
    if error
        .deferral()
        .and_then(|reason| reason.decode_refusal())
        .is_some_and(|refusal| {
            refusal.kind() == norito::core::DecodeLocalRefusalKind::EnclosingLimit
        })
        && let Ok(Some(owner)) =
            crate::history_producer::HistoryProducerOwner::current_if_admitted()
    {
        owner.note_owner_limit_refused();
    }
    crate::live_dataspace_resolution_error(error)
}

/// Row fields only this join produces.
const ALIAS_FIELDS: [&str; 2] = ["alias_scopes", "aliases"];

/// Whether `path` names a joined field or a path below one.
fn is_alias_field(path: &str) -> bool {
    ALIAS_FIELDS.iter().any(|field| {
        path.strip_prefix(field)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
    })
}

fn filter_reads_alias_fields(filter: &FilterExpr) -> bool {
    match filter {
        FilterExpr::And(list) | FilterExpr::Or(list) => list.iter().any(filter_reads_alias_fields),
        FilterExpr::Not(inner) => filter_reads_alias_fields(inner),
        FilterExpr::Eq(path, _)
        | FilterExpr::Ne(path, _)
        | FilterExpr::Lt(path, _)
        | FilterExpr::Lte(path, _)
        | FilterExpr::Gt(path, _)
        | FilterExpr::Gte(path, _)
        | FilterExpr::In(path, _)
        | FilterExpr::Nin(path, _)
        | FilterExpr::Exists(path)
        | FilterExpr::IsNull(path) => is_alias_field(path.as_str()),
    }
}

/// Whether `query` filters, selects, sorts or aggregates on a joined field.
pub(super) fn query_reads_alias_fields(query: &ListQuery) -> bool {
    query.filter.as_ref().is_some_and(filter_reads_alias_fields)
        || query
            .select
            .iter()
            .flatten()
            .any(|path| is_alias_field(path.as_str()))
        || query
            .sort
            .iter()
            .any(|key| is_alias_field(key.key.as_str()))
        || query.aggregate.as_ref().is_some_and(|aggregate| {
            aggregate
                .group_by
                .iter()
                .chain(
                    aggregate
                        .metrics
                        .iter()
                        .filter_map(|metric| metric.field.as_ref()),
                )
                .any(|path| is_alias_field(path.as_str()))
        })
}

/// What one accounts read needs to join aliases besides the world it scans.
pub(super) struct AccountAliasJoin {
    pub(super) now_ms: u64,
    /// Dataspaces whose aliases anyone may read.
    pub(super) public_dataspaces: BTreeSet<DataSpaceId>,
    /// The signed caller whose resolve permissions open other dataspaces' aliases.
    pub(super) resolver: Option<AccountId>,
    /// One cumulative work allowance across every account and both join passes.
    pub(super) remaining_visits: Cell<usize>,
}

impl AccountAliasJoin {
    /// The join for `query`, or `None` when it reads no joined field.
    pub(super) fn for_query(
        state: &CoreState,
        query: &ListQuery,
        visibility: &DataspaceReadVisibility,
        limits: &Limits,
    ) -> Option<Self> {
        query_reads_alias_fields(query).then(|| Self {
            now_ms: asset_alias_observation_time_ms(state),
            public_dataspaces: crate::torii_public_dataspace_ids_in(&state.view()),
            resolver: visibility.alias_resolver().cloned(),
            remaining_visits: Cell::new(limits.max_scanned_rows.min(
                limits.bytes.source_frame_bytes / core::mem::size_of::<AccountAlias>().max(1),
            )),
        })
    }

    /// Exact disclosed scopes for partial filter evaluation, before projecting aliases.
    /// A nonmatching account's alias output count cannot fail the whole inventory.
    pub(super) fn scope_fields(
        &self,
        world: &impl WorldReadOnly,
        account_id: &AccountId,
        bytes: collections::memory::BytePolicy,
    ) -> Result<Map> {
        let Some(labels) = world.account_aliases_by_account().get(account_id) else {
            return bytes
                .row_fields([("alias_scopes", &Vec::<String>::new())])
                .map_err(Into::into);
        };
        collections::memory::ensure(ALIAS_TRANSIENT_BYTES, bytes.scratch_bytes, "alias scratch")?;
        let capacity = labels.len().min(
            (bytes.scratch_bytes - ALIAS_TRANSIENT_BYTES)
                / (core::mem::size_of::<String>() + MAX_SCOPE_BYTES),
        );
        let mut scopes =
            collections::memory::vector::<String>(capacity, bytes.scratch_bytes, "alias scopes")?;
        self.visit_disclosed(world, account_id, bytes.scratch_bytes, |_, alias| {
            let Some((_, scope)) = alias.split_once('@') else {
                return Err(conversion_error("canonical alias has no scope".to_owned()));
            };
            if let Err(index) = scopes.binary_search_by(|known| known.as_str().cmp(scope)) {
                collections::memory::ensure(scope.len(), MAX_SCOPE_BYTES, "alias scope")?;
                if scopes.len() == capacity {
                    return Err(collections::memory::capacity("alias scopes").into());
                }
                // Exact capacity and maximum string size were admitted above.
                scopes.insert(index, scope.to_owned());
            }
            Ok(())
        })?;
        bytes
            .row_fields([("alias_scopes", &scopes)])
            .map_err(Into::into)
    }

    /// Project alias output only after the exact disclosed scopes may reject a row.
    pub(super) fn fields(
        &self,
        world: &impl WorldReadOnly,
        (account_id, details): (&AccountId, &iroha_data_model::account::AccountDetails),
        bytes: collections::memory::BytePolicy,
    ) -> Result<Map> {
        let items = self.bindings(world, account_id, details, bytes)?;
        // Borrow the already bounded literals; do not allocate another copy of
        // every scope while the DTO strings are still alive.
        let mut scopes = collections::memory::vector::<&str>(
            items.len(),
            bytes.scratch_bytes / 2,
            "alias scope references",
        )?;
        scopes.extend(
            items
                .iter()
                .filter_map(|item| item.alias.split_once('@').map(|(_, scope)| scope)),
        );
        scopes.sort_unstable();
        scopes.dedup();
        bytes
            .row_fields([("alias_scopes", &scopes), ("aliases", &items)])
            .map_err(Into::into)
    }

    /// Visit exact disclosed bindings under one cumulative query work allowance.
    fn visit_disclosed(
        &self,
        world: &impl WorldReadOnly,
        account_id: &AccountId,
        scratch_bytes: usize,
        mut visit: impl FnMut(&AccountAlias, String) -> Result<()>,
    ) -> Result<()> {
        let Some(labels) = world.account_aliases_by_account().get(account_id) else {
            return Ok(());
        };
        collections::memory::ensure(ALIAS_TRANSIENT_BYTES, scratch_bytes, "alias scratch")?;
        let catalog = world.dataspace_catalog();
        for label in labels {
            let remaining = self
                .remaining_visits
                .get()
                .checked_sub(1)
                .ok_or_else(|| collections::memory::capacity("alias scan work"))?;
            self.remaining_visits.set(remaining);
            let bound =
                iroha_core::sns::resolve_active_account_alias(world, catalog, label, self.now_ms)
                    .map_err(alias_producer_sns_error)?;
            if bound.as_ref() != Some(account_id) {
                continue;
            }
            let alias =
                iroha_core::sns::active_account_alias_literal(world, catalog, label, self.now_ms)
                    .map_err(alias_producer_sns_error)?;
            if self.discloses(world, label, &alias)? {
                visit(label, alias)?;
            }
        }
        Ok(())
    }

    /// The active output bindings in by-account order, bounded before allocation.
    pub(super) fn bindings(
        &self,
        world: &impl WorldReadOnly,
        account_id: &AccountId,
        details: &iroha_data_model::account::AccountDetails,
        bytes: collections::memory::BytePolicy,
    ) -> Result<Vec<AliasLookupByAccountItemDto>> {
        let Some(labels) = world.account_aliases_by_account().get(account_id) else {
            return Ok(Vec::new());
        };
        let ceiling = bytes.scratch_bytes / 2;
        let capacity = labels.len().min(crate::EXACT_ALIAS_LOOKUP_MAX_ITEMS);
        let mut retained = collections::memory::slots::<AliasLookupByAccountItemDto>(capacity)?;
        collections::memory::ensure(
            collections::memory::add(retained, ALIAS_TRANSIENT_BYTES)?,
            ceiling,
            "alias output",
        )?;
        let mut items = collections::memory::vector::<AliasLookupByAccountItemDto>(
            capacity,
            ceiling,
            "alias output",
        )?;
        self.visit_disclosed(world, account_id, ceiling, |label, alias| {
            if items.len() == crate::EXACT_ALIAS_LOOKUP_MAX_ITEMS {
                return Err(collections::memory::capacity("account alias list").into());
            }
            // Reserve the maximum canonical strings before resolving or copying
            // them; the temporary allowance remains available for the next visit.
            let next = collections::memory::add(retained, ALIAS_TRANSIENT_BYTES)?;
            collections::memory::ensure(
                collections::memory::add(next, ALIAS_TRANSIENT_BYTES)?,
                ceiling,
                "alias output",
            )?;
            let dataspace = iroha_core::sns::resolve_active_dataspace_alias_by_id(
                world,
                world.dataspace_catalog(),
                label.dataspace,
                self.now_ms,
            )
            .map_err(alias_producer_sns_error)?;
            let domain = label.domain.as_ref().map(ToString::to_string);
            let strings = collections::memory::add(alias.capacity(), dataspace.capacity())?;
            let strings =
                collections::memory::add(strings, domain.as_ref().map_or(0, String::capacity))?;
            collections::memory::ensure(strings, ALIAS_TRANSIENT_BYTES, "alias strings")?;
            retained = collections::memory::add(retained, strings)?;
            items.push(AliasLookupByAccountItemDto {
                alias,
                dataspace,
                domain,
                is_primary: details.label() == Some(label),
            });
            Ok(())
        })?;
        items.sort_by(|left, right| {
            left.alias
                .cmp(&right.alias)
                .then_with(|| left.dataspace.cmp(&right.dataspace))
                .then_with(|| left.domain.cmp(&right.domain))
                .then_with(|| right.is_primary.cmp(&left.is_primary))
        });
        Ok(items)
    }

    /// Whether the caller may read this binding: its dataspace is public, or the caller
    /// holds the exact, domain or dataspace resolve permission that covers it.
    fn discloses(
        &self,
        world: &impl WorldReadOnly,
        label: &AccountAlias,
        alias: &str,
    ) -> Result<bool> {
        if self.public_dataspaces.contains(&label.dataspace) {
            return Ok(true);
        }
        let Some(caller) = &self.resolver else {
            return Ok(false);
        };
        let canonical_name = alias.parse::<AccountAliasName>().map_err(|error| {
            conversion_error(format!("invalid canonical account alias: {error}"))
        })?;
        let resolved = ResolvedAccountAliasV1::new(canonical_name, label.dataspace);
        iroha_core::alias::authority_can_resolve_resolved_account_alias(world, caller, &resolved)
            .map_err(alias_producer_sns_error)
    }
}
