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
}

impl AccountAliasJoin {
    /// The join for `query`, or `None` when it reads no joined field.
    pub(super) fn for_query(
        state: &CoreState,
        query: &ListQuery,
        visibility: &DataspaceReadVisibility,
    ) -> Option<Self> {
        query_reads_alias_fields(query).then(|| Self {
            now_ms: asset_alias_observation_time_ms(state),
            public_dataspaces: crate::torii_public_dataspace_ids_in(&state.view()),
            resolver: visibility.alias_resolver().cloned(),
        })
    }

    /// The row fields `alias_scopes` and `aliases` of an account the caller can see. They are
    /// exact, so the row producer puts them in the cheap prefix the filter is tried on first:
    /// an account the filter rejects never serializes its metadata.
    pub(super) fn fields(
        &self,
        world: &impl WorldReadOnly,
        (account_id, details): (&AccountId, &iroha_data_model::account::AccountDetails),
        bytes: collections::memory::BytePolicy,
    ) -> Result<Map> {
        let items = self.bindings(world, account_id, details)?;
        let scopes = alias_scopes(&items);
        bytes
            .row_fields([("alias_scopes", &scopes), ("aliases", &items)])
            .map_err(Into::into)
    }

    /// The active bindings of `account_id` the caller may read, in the order of the by-account route.
    pub(super) fn bindings(
        &self,
        world: &impl WorldReadOnly,
        account_id: &AccountId,
        details: &iroha_data_model::account::AccountDetails,
    ) -> Result<Vec<AliasLookupByAccountItemDto>> {
        let Some(labels) = world.account_aliases_by_account().get(account_id) else {
            return Ok(Vec::new());
        };
        let catalog = world.dataspace_catalog();
        let mut items = Vec::with_capacity(labels.len());
        for label in labels {
            let bound =
                iroha_core::sns::resolve_active_account_alias(world, catalog, label, self.now_ms)
                    .map_err(crate::live_dataspace_resolution_error)?;
            if bound.as_ref() != Some(account_id) {
                continue;
            }
            let alias =
                iroha_core::sns::active_account_alias_literal(world, catalog, label, self.now_ms)
                    .map_err(crate::live_dataspace_resolution_error)?;
            if !self.discloses(world, label, &alias)? {
                continue;
            }
            let dataspace = iroha_core::sns::resolve_active_dataspace_alias_by_id(
                world,
                catalog,
                label.dataspace,
                self.now_ms,
            )
            .map_err(crate::live_dataspace_resolution_error)?;
            items.push(AliasLookupByAccountItemDto {
                alias,
                dataspace,
                domain: label.domain.as_ref().map(ToString::to_string),
                is_primary: details.label() == Some(label),
            });
        }
        if items.len() > crate::EXACT_ALIAS_LOOKUP_MAX_ITEMS {
            return Err(collections::memory::capacity("account alias list").into());
        }
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
            .map_err(crate::live_dataspace_resolution_error)
    }
}

/// The text after `@` of every binding's alias (`domain.dataspace`, or `dataspace` for a
/// domainless alias), each once, in order.
fn alias_scopes(items: &[AliasLookupByAccountItemDto]) -> Vec<String> {
    let scopes: BTreeSet<&str> = items
        .iter()
        .filter_map(|item| item.alias.split_once('@').map(|(_, scope)| scope))
        .collect();
    scopes.into_iter().map(str::to_owned).collect()
}
