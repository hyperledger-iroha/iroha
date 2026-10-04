//! Row producers behind the collection engine ([`crate::collections`]).
//!
//! Each collection turns this node's state into JSON rows whose fields match
//! its [`CollectionSpec`]; the engine then filters, orders and pages them.
//! Execution here returns full rows: projection happens after fan-out merging.
use super::*;
use crate::collections::{self, CollectionError, CollectionSpec, Limits, RowPage, specs};
use iroha_torii_shared::list_query::ListQuery;

/// A collection addressed by a request path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CollectionTarget {
    /// `/v1/domains`
    Domains,
    /// `/v1/accounts`
    Accounts,
    /// `/v1/assets/definitions`
    AssetDefinitions,
    /// `/v1/nfts`
    Nfts,
    /// `/v1/rwas`
    Rwas,
    /// `/v1/accounts/{account_id}/assets` (path literal: I105 id or alias)
    AccountAssets(String),
    /// `/v1/assets/{definition_id}/holders` (path literal: id or alias)
    AssetHolders(String),
    /// `/v1/accounts/{account_id}/transactions` (path literal: I105 id or alias)
    AccountTransactions(String),
    /// `/v1/transactions/query`
    Transactions,
    /// `/v1/repo/agreements`
    RepoAgreements,
}

impl CollectionTarget {
    /// The collection's query surface.
    pub(crate) fn spec(&self) -> &'static CollectionSpec {
        match self {
            Self::Domains => &specs::DOMAINS,
            Self::Accounts => &specs::ACCOUNTS,
            Self::AssetDefinitions => &specs::ASSET_DEFINITIONS,
            Self::Nfts => &specs::NFTS,
            Self::Rwas => &specs::RWAS,
            Self::AccountAssets(_) => &specs::ACCOUNT_ASSETS,
            Self::AssetHolders(_) => &specs::ASSET_HOLDERS,
            Self::AccountTransactions(_) => &specs::ACCOUNT_TRANSACTIONS,
            Self::Transactions => &specs::TRANSACTIONS,
            Self::RepoAgreements => &specs::REPO_AGREEMENTS,
        }
    }

    /// The path argument scoping the collection (empty for top-level
    /// collections); cursors are bound to it.
    pub(crate) fn scope(&self) -> &str {
        match self {
            Self::AccountAssets(literal)
            | Self::AssetHolders(literal)
            | Self::AccountTransactions(literal) => literal,
            Self::Domains
            | Self::Accounts
            | Self::AssetDefinitions
            | Self::Nfts
            | Self::Rwas
            | Self::Transactions
            | Self::RepoAgreements => "",
        }
    }

    /// Endpoint label used for telemetry and literal diagnostics.
    pub(crate) const fn endpoint(&self) -> &'static str {
        match self {
            Self::Domains => ENDPOINT_DOMAINS_QUERY,
            Self::Accounts => ENDPOINT_ACCOUNTS_QUERY,
            Self::AssetDefinitions => ENDPOINT_ASSET_DEFINITIONS_QUERY,
            Self::Nfts => ENDPOINT_NFTS_QUERY,
            Self::Rwas => ENDPOINT_RWAS_QUERY,
            Self::AccountAssets(_) => ENDPOINT_ACCOUNTS_ASSETS_QUERY,
            Self::AssetHolders(_) => ENDPOINT_ASSET_HOLDERS_QUERY,
            Self::AccountTransactions(_) => ENDPOINT_ACCOUNTS_TRANSACTIONS_QUERY,
            Self::Transactions => ENDPOINT_TRANSACTIONS_QUERY,
            Self::RepoAgreements => ENDPOINT_REPO_AGREEMENTS_QUERY,
        }
    }
}

/// Engine bounds derived from the configured app-API page limits.
pub(crate) fn collection_limits() -> Limits {
    let limits = app_query_limits();
    Limits::from_page_limits(limits.default_page_limit, limits.max_page_limit)
}

/// Rewrite account literals in the filter (aliases, alternate spellings) to
/// the canonical identifiers the rows carry.
pub(crate) fn canonicalize_collection_query(
    state: &CoreState,
    target: &CollectionTarget,
    query: &mut ListQuery,
    telemetry: &MaybeTelemetry,
) -> Result<()> {
    let spec = target.spec();
    let Some(filter) = query.filter.as_mut() else {
        return Ok(());
    };
    record_account_literal_selection(telemetry, target.endpoint());
    for field in spec.account_fields() {
        canonicalize_filter_account_literals(
            filter,
            field,
            Some(state),
            telemetry,
            target.endpoint(),
        )
        .map_err(|err| {
            Error::from(
                CollectionError::new("invalid_filter", "filter", err.to_string())
                    .with_actual(field),
            )
        })?;
    }
    Ok(())
}

/// Drops global-scope asset rows a fan-out route is not authoritative for
/// before paging, so every row is served by exactly one route and route pages
/// merge without gaps or double counts.
struct RouteAuthority<'a> {
    route: Option<(&'a crate::AppState, iroha_core::queue::RoutingDecision)>,
    failure: Option<Error>,
}

impl<'a> RouteAuthority<'a> {
    fn new(
        app: Option<&'a crate::SharedAppState>,
        route: Option<iroha_core::queue::RoutingDecision>,
    ) -> Self {
        Self {
            route: app.zip(route).map(|(app, route)| (app.as_ref(), route)),
            failure: None,
        }
    }

    fn keeps(&mut self, row: &Map) -> bool {
        let Some((app, route)) = self.route else {
            return true;
        };
        if self.failure.is_some() {
            return false;
        }
        crate::should_keep_authoritative_global_row(app, route, row).unwrap_or_else(|err| {
            self.failure = Some(err);
            false
        })
    }

    fn finish(self, page: std::result::Result<RowPage, CollectionError>) -> Result<RowPage> {
        match self.failure {
            Some(err) => Err(err),
            None => page.map_err(Error::from),
        }
    }
}

/// Validate and execute `query` against this node's state, returning full rows.
///
/// `route` is the fan-out route this execution serves, if any.
pub(crate) async fn execute_collection_local(
    app: Option<&crate::SharedAppState>,
    state: &Arc<CoreState>,
    target: &CollectionTarget,
    mut query: ListQuery,
    telemetry: &MaybeTelemetry,
    visibility: &DataspaceReadVisibility,
    route: Option<iroha_core::queue::RoutingDecision>,
) -> Result<RowPage> {
    canonicalize_collection_query(state, target, &mut query, telemetry)?;
    let limits = collection_limits();
    let prepared = collections::prepare(target.spec(), target.scope(), &query, &limits)?;
    let page = match target {
        CollectionTarget::Domains => {
            let world = state.world_view();
            let rows = world
                .domains_iter()
                .filter(|domain| visibility.allows_domain(&world, domain.id()))
                .map(domain_row);
            prepared.execute(rows, &limits)
        }
        CollectionTarget::Accounts => {
            let world = state.world_view();
            let catalog = state.nexus_snapshot().dataspace_catalog;
            let accounts = collect_subject_accounts(&world);
            let rows = accounts
                .iter()
                .filter(|account| visibility.allows_account(&world, account.id()))
                .map(|account| account_row(account, &catalog));
            prepared.execute(rows, &limits)
        }
        CollectionTarget::AssetDefinitions => {
            let world = state.world_view();
            let now_ms = asset_alias_observation_time_ms(state);
            let world_ref = &world;
            // Exact `id`/alias constraints become direct lookups; the engine
            // still applies the whole filter.
            let rows = asset_definitions_for_filter(world_ref, query.filter.as_ref())
                .filter(|definition| visibility.allows_asset_definition(world_ref, definition.id()))
                .map(|definition| {
                    let binding =
                        asset_definition_alias_binding_for(world_ref, definition.id(), now_ms);
                    asset_definition_to_json_value(&definition, binding.as_ref())
                        .and_then(object_row_from_value)
                })
                .collect::<Result<Vec<_>>>()?;
            prepared.execute(rows, &limits)
        }
        CollectionTarget::Nfts => {
            let world = state.world_view();
            let world_ref = &world;
            let rows = nfts_for_filter(world_ref, query.filter.as_ref())
                .filter(|nft| visibility.allows_nft(world_ref, nft.id()))
                .map(|nft| nft_row(&nft));
            prepared.execute(rows, &limits)
        }
        CollectionTarget::Rwas => {
            let world = state.world_view();
            let rows = world
                .rwas_iter()
                .filter(|rwa| visibility.allows_rwa(&world, rwa.id()))
                .map(|rwa| dto_row(&crate::explorer::ExplorerRwaDto::from_entry(rwa)));
            prepared.execute(rows, &limits)
        }
        CollectionTarget::AccountAssets(account_literal) => {
            let (account, _) = parse_account_path_segment_with_state(
                state.as_ref(),
                account_literal,
                telemetry,
                ENDPOINT_ACCOUNTS_ASSETS_QUERY,
            )?;
            let world = state.world_view();
            let scoped_accounts = visibility.exact_account_id().map_or_else(
                || scoped_accounts_for_subject_sorted(&world, &account),
                |exact| vec![exact.clone()],
            );
            let items =
                collect_projected_account_assets(&world, &scoped_accounts, None, None, visibility);
            drop(world);
            let mut authority = RouteAuthority::new(app, route);
            let rows = items
                .iter()
                .map(account_asset_row)
                .filter(|row| authority.keeps(row));
            let page = prepared.execute(rows, &limits);
            return authority.finish(page);
        }
        CollectionTarget::AssetHolders(definition_literal) => {
            let now_ms = asset_alias_observation_time_ms(state);
            let (definition, asset_alias) = {
                let world = state.world_view();
                let definition =
                    resolve_asset_definition_selector(&world, definition_literal, now_ms)?;
                if !visibility.allows_asset_definition(&world, &definition) {
                    return Err(explorer_not_found());
                }
                let alias = world
                    .asset_definition(&definition)
                    .ok()
                    .and_then(|record| record.alias().as_ref().map(ToString::to_string));
                (definition, alias)
            };
            if query.aggregate.is_some() {
                // Aggregates over every holder of a popular asset run on the
                // published query projection; only unit tests scan live state.
                if let Some((rows, _snapshot, _source)) = asset_holder_projection_query_rows(
                    app,
                    state.as_ref(),
                    &definition,
                    asset_alias.as_ref(),
                    None,
                    visibility,
                )
                .await?
                {
                    let mut authority = RouteAuthority::new(app, route);
                    let rows = rows.into_iter().filter(|row| authority.keeps(row));
                    let page = prepared.execute(rows, &limits);
                    return authority.finish(page);
                }
                if !asset_holder_live_aggregate_enabled() {
                    return Err(projection_archive_unavailable_error(
                        "asset holder aggregates require a complete published query projection archive; live holder scans are disabled",
                    ));
                }
            }
            let world = state.world_view();
            let asset_literal = definition.to_string();
            let mut authority = RouteAuthority::new(app, route);
            let rows = world
                .asset_entries_by_definition_iter(&definition)
                .filter(|entry| visibility.allows_asset(&world, entry.id()))
                .map(|entry| {
                    asset_holder_row(&live_asset_holder_item(
                        entry.id(),
                        entry.value().as_ref(),
                        &asset_literal,
                        asset_alias.as_ref(),
                    ))
                })
                .filter(|row| authority.keeps(row));
            let page = prepared.execute(rows, &limits);
            return authority.finish(page);
        }
        CollectionTarget::AccountTransactions(account_literal) => {
            let (account, _) = parse_account_path_segment_with_state(
                state.as_ref(),
                account_literal,
                telemetry,
                ENDPOINT_ACCOUNTS_TRANSACTIONS_QUERY,
            )?;
            if !visibility.allows_account(&state.world_view(), &account) {
                return Ok(prepared.positioned_page(Vec::new(), None));
            }
            let allowed = app
                .map(|app| crate::resolve_tx_history_allowed_asset_definition_id(app))
                .transpose()?
                .flatten();
            return transaction_page(
                state,
                &prepared,
                Some(&account),
                allowed.as_ref(),
                visibility,
            );
        }
        CollectionTarget::Transactions => {
            let allowed = app
                .map(|app| crate::resolve_tx_history_allowed_asset_definition_id(app))
                .transpose()?
                .flatten();
            return transaction_page(state, &prepared, None, allowed.as_ref(), visibility);
        }
        CollectionTarget::RepoAgreements => {
            let world = state.world_view();
            let rows = repo_agreements_for_filter(&world, query.filter.as_ref()).map(|agreement| {
                repo_agreement_projection_to_query_row(&RepoAgreementProjection::from_agreement(
                    agreement,
                ))
            });
            prepared.execute(rows, &limits)
        }
    };
    Ok(page?)
}

/// One page of committed transactions, newest first, strictly before the
/// cursor's block coordinates.
///
/// Every page has its own history-scan budget. A page that spends it before
/// filling up ends early with a cursor at the last examined transaction.
/// History is authenticated downward from the newest block, so the budget
/// also pays for every block above the page's starting position; a start
/// deeper than the budget reaches is rejected as `query_scan_limit_exceeded`.
fn transaction_page(
    state: &Arc<CoreState>,
    prepared: &collections::Prepared<'_>,
    subject: Option<&AccountId>,
    allowed_definition: Option<&AssetDefinitionId>,
    visibility: &DataspaceReadVisibility,
) -> Result<RowPage> {
    use iroha_core::smartcontracts::isi::tx::{
        TransactionHistoryPageEnd, TransactionHistoryPosition, transaction_history_byte_limit,
        visit_committed_transaction_page,
    };
    let resume = prepared
        .resume_position()
        .map(|(height, index)| {
            TransactionHistoryPosition::new(height, index).ok_or_else(|| {
                Error::from(CollectionError::new(
                    "invalid_cursor",
                    "cursor",
                    "the cursor addresses a block beyond this node's range",
                ))
            })
        })
        .transpose()?;
    let (lowest, highest) = prepared.height_range();
    if lowest > highest {
        return Ok(prepared.positioned_page(Vec::new(), None));
    }
    // Start the walk at the top of the filter's height range.
    let ceiling = highest
        .checked_add(1)
        .and_then(|height| TransactionHistoryPosition::new(height, 0));
    let resume = match (resume, ceiling) {
        (Some(resume), Some(ceiling)) => Some(resume.min(ceiling)),
        (resume, ceiling) => resume.or(ceiling),
    };
    let allowed = allowed_definition
        .cloned()
        .map(TxHistoryAssetSelector::DefinitionId);
    // The walk's view is captured before the visibility reads so every visited
    // carrier lies inside the prefix the reads authenticate.
    let view = state.view();
    let reads = HistoryVisibilityReads::new(Arc::clone(state));
    let limit = prepared.limit();
    let mut items = Vec::new();
    let mut last_kept = None;
    let mut below_range = false;
    let work = app_query_limits().max_fetch_size;
    let end = visit_committed_transaction_page(
        &view,
        resume,
        work,
        transaction_history_byte_limit(work),
        |transaction, position| {
            if position.height() < lowest {
                below_range = true;
                return std::ops::ControlFlow::Break(());
            }
            let visible = visibility.can_read_all()
                || reads.allows(
                    visibility,
                    position.height(),
                    Some(transaction.block_hash),
                    *transaction.entrypoint_hash(),
                );
            if !visible
                || subject.is_some_and(|account| {
                    !tx_matches_account_history_subject(&transaction, account)
                })
                || allowed
                    .as_ref()
                    .is_some_and(|selector| !tx_matches_asset_selector(&transaction, selector))
            {
                return std::ops::ControlFlow::Continue(());
            }
            let row = transaction_row(&transaction, position);
            if !prepared.matches(&row) {
                return std::ops::ControlFlow::Continue(());
            }
            if items.len() == limit {
                return std::ops::ControlFlow::Break(());
            }
            items.push(row);
            last_kept = Some(position);
            std::ops::ControlFlow::Continue(())
        },
    )
    .map_err(|error| Error::Query(iroha_data_model::ValidationFail::QueryFailed(error)))?;
    drop(view);
    reads.finish()?;
    let resume = match end {
        TransactionHistoryPageEnd::Stopped(_) if below_range => None,
        TransactionHistoryPageEnd::Stopped(_) => last_kept,
        TransactionHistoryPageEnd::BudgetSpent(position) => Some(position),
        TransactionHistoryPageEnd::OutOfReach => {
            return Err(CollectionError::new(
                "query_scan_limit_exceeded",
                if prepared.resume_position().is_some() {
                    "cursor"
                } else {
                    "filter"
                },
                format!(
                    "this page would start deeper in history than one page's scan budget \
                     ({work} blocks and transactions) reaches below the newest block"
                ),
            )
            .with_hint(
                "history is read newest first from the newest block; \
                 deeper transactions are not reachable through this collection",
            )
            .into());
        }
        TransactionHistoryPageEnd::Exhausted => None,
    };
    Ok(prepared.positioned_page(
        items,
        resume.map(|position| (position.height(), position.block_index())),
    ))
}

/// The row of one committed transaction (`specs/torii/collection_queries.md`).
fn transaction_row(
    transaction: &iroha_data_model::query::CommittedTransaction,
    position: iroha_core::smartcontracts::isi::tx::TransactionHistoryPosition,
) -> Map {
    let projection = project_tx(transaction);
    let assets = tx_collect_asset_ids(transaction);
    let strings =
        |values: BTreeSet<String>| Value::Array(values.into_iter().map(Value::from).collect());
    let mut row = Map::new();
    row.insert(
        "entrypoint_hash".into(),
        Value::from(projection.entrypoint_hash),
    );
    row.insert("block_height".into(), Value::from(position.height()));
    row.insert("block_index".into(), Value::from(position.block_index()));
    row.insert(
        "block_hash".into(),
        Value::from(transaction.block_hash.to_string()),
    );
    row.insert(
        "authority".into(),
        projection.authority.map_or(Value::Null, Value::from),
    );
    row.insert(
        "timestamp_ms".into(),
        projection.timestamp_ms.map_or(Value::Null, Value::from),
    );
    row.insert(
        "entrypoint_kind".into(),
        Value::from(projection.entrypoint_kind),
    );
    row.insert("result_ok".into(), Value::from(projection.result_ok));
    row.insert(
        "asset_ids".into(),
        strings(assets.iter().map(ToString::to_string).collect()),
    );
    row.insert(
        "asset_definition_ids".into(),
        strings(
            assets
                .iter()
                .map(|asset| asset.definition().to_string())
                .collect(),
        ),
    );
    let metadata = match transaction.entrypoint() {
        TransactionEntrypoint::External(signed) => metadata_to_json(signed.metadata()),
        _ => Value::Object(Map::new()),
    };
    row.insert("metadata".into(), metadata);
    row
}

/// Execute locally and apply `select`, for collections served without fan-out.
pub(crate) async fn execute_collection_response(
    app: Option<&crate::SharedAppState>,
    state: &Arc<CoreState>,
    target: &CollectionTarget,
    query: ListQuery,
    telemetry: &MaybeTelemetry,
    visibility: &DataspaceReadVisibility,
) -> Result<Response> {
    let limits = collection_limits();
    let prepared = collections::prepare(target.spec(), target.scope(), &query, &limits)?;
    let page = execute_collection_local(
        app,
        state,
        target,
        query.clone(),
        telemetry,
        visibility,
        None,
    )
    .await?;
    row_page_response(prepared.project(page))
}

/// Serialize a page as the JSON page envelope.
pub(crate) fn row_page_response(page: RowPage) -> Result<Response> {
    let body = norito::json::to_json(&page.into_page()).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
            "failed to encode collection page: {err}"
        )))
    })?;
    let mut response = Response::new(axum::body::Body::from(body));
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(response)
}

fn domain_row(domain: &iroha_data_model::domain::Domain) -> Map {
    let mut row = Map::new();
    row.insert("id".into(), Value::from(domain.id().to_string()));
    row.insert(
        "owned_by".into(),
        Value::from(domain.owned_by().to_string()),
    );
    row.insert(
        "logo".into(),
        domain
            .logo()
            .as_ref()
            .map_or(Value::Null, |logo| Value::from(logo.to_string())),
    );
    row.insert(
        "metadata".into(),
        crate::explorer::metadata_to_json(&domain.metadata),
    );
    row
}

fn account_row(account: &iroha_data_model::account::Account, catalog: &DataSpaceCatalog) -> Map {
    let mut row = Map::new();
    row.insert("id".into(), Value::from(account.id().to_string()));
    row.insert(
        "label".into(),
        account
            .label
            .as_ref()
            .and_then(|label| label.to_literal(catalog).ok())
            .map_or(Value::Null, Value::from),
    );
    row.insert(
        "uaid".into(),
        account
            .uaid
            .as_ref()
            .map_or(Value::Null, |uaid| Value::from(uaid.to_string())),
    );
    row.insert(
        "metadata".into(),
        crate::explorer::metadata_to_json(&account.metadata),
    );
    row
}

fn nft_row(nft: &iroha_data_model::nft::Nft) -> Map {
    let mut row = Map::new();
    row.insert("id".into(), Value::from(nft.id().to_string()));
    row.insert("owned_by".into(), Value::from(nft.owned_by().to_string()));
    row.insert(
        "metadata".into(),
        crate::explorer::metadata_to_json(nft.content()),
    );
    row
}

fn dto_row<T: norito::json::JsonSerialize>(dto: &T) -> Map {
    match norito::json::to_value(dto) {
        Ok(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

fn account_asset_row(item: &AccountAssetListItem) -> Map {
    let mut row = Map::new();
    row.insert("account_id".into(), Value::from(item.account_id.clone()));
    row.insert("asset".into(), Value::from(item.asset.clone()));
    row.insert("asset_name".into(), Value::from(item.asset_name.clone()));
    row.insert(
        "asset_alias".into(),
        item.asset_alias
            .as_ref()
            .map_or(Value::Null, |alias| Value::from(alias.clone())),
    );
    row.insert("scope".into(), Value::from(item.scope.clone()));
    row.insert("quantity".into(), Value::from(item.quantity.to_string()));
    row
}

fn asset_holder_row(item: &AssetHolderListItem) -> Map {
    let mut row = Map::new();
    row.insert("account_id".into(), Value::from(item.canonical_id.clone()));
    row.insert("asset".into(), Value::from(item.asset.clone()));
    row.insert(
        "asset_alias".into(),
        item.asset_alias
            .as_ref()
            .map_or(Value::Null, |alias| Value::from(alias.clone())),
    );
    row.insert("scope".into(), Value::from(item.scope.clone()));
    row.insert("quantity".into(), Value::from(item.quantity.to_string()));
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::Algorithm;
    use iroha_data_model::prelude as dm;
    use iroha_primitives::json::Json;
    use iroha_torii_shared::list_query::{SortKey, field};

    fn authority(seed: u8) -> dm::AccountId {
        dm::AccountId::new(
            checked_routing_fixture_keypair(seed, Algorithm::Ed25519, "collection fixture key")
                .public_key()
                .clone(),
        )
    }

    fn domain_id(name: &str) -> DomainId {
        DomainId::try_new(name, "universal").expect("fixture domain")
    }

    fn fixture() -> (Arc<CoreState>, dm::AccountId, dm::AccountId) {
        let alice = authority(0xC1);
        let bob = authority(0xC2);
        let domain = |name: &str, owner: &dm::AccountId, tier: u64| {
            let mut metadata = Metadata::default();
            metadata.insert("tier".parse().expect("key"), Json::new(tier));
            dm::Domain::new(domain_id(name))
                .with_metadata(metadata)
                .build(owner)
        };
        let domains = vec![
            domain("delta", &bob, 2),
            domain("alpha", &alice, 1),
            domain("echo", &alice, 3),
            domain("charlie", &bob, 1),
            domain("bravo", &alice, 2),
        ];
        let accounts = vec![
            dm::Account::new(alice.account().clone()).build(&alice),
            dm::Account::new(bob.account().clone()).build(&bob),
        ];
        let state = Arc::new(State::new_for_testing(
            World::with(domains, accounts, []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        (state, alice, bob)
    }

    async fn run(
        state: &Arc<CoreState>,
        target: &CollectionTarget,
        query: ListQuery,
    ) -> Result<RowPage> {
        execute_collection_local(
            None,
            state,
            target,
            query,
            &MaybeTelemetry::for_tests(),
            &DataspaceReadVisibility::all_for_tests(),
            None,
        )
        .await
    }

    fn ids(page: &RowPage) -> Vec<String> {
        page.items
            .iter()
            .map(|row| row["id"].as_str().expect("id").to_owned())
            .collect()
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter()
            .map(|name| domain_id(name).to_string())
            .collect()
    }

    #[tokio::test]
    async fn domains_page_through_every_row_with_cursors() {
        let (state, _, _) = fixture();
        let query = ListQuery::new().limit(2);
        let mut seen = Vec::new();
        let mut current = query.clone();
        loop {
            let page = run(&state, &CollectionTarget::Domains, current.clone())
                .await
                .expect("page");
            assert!(page.items.len() <= 2);
            seen.extend(ids(&page));
            match page.next_cursor {
                Some(cursor) => current = query.clone().cursor(cursor),
                None => break,
            }
        }
        let mut expected = names(&["alpha", "bravo", "charlie", "delta", "echo"]);
        expected.sort();
        assert_eq!(seen, expected);
    }

    #[tokio::test]
    async fn domains_filter_sort_and_total() {
        let (state, alice, _) = fixture();
        let query = ListQuery::new()
            .filter(field("owned_by").eq(alice.to_string()) & field("metadata.tier").gte(2))
            .sort_by(SortKey::desc("metadata.tier"))
            .include_total();
        let page = run(&state, &CollectionTarget::Domains, query)
            .await
            .expect("page");
        assert_eq!(ids(&page), names(&["echo", "bravo"]));
        assert_eq!(page.total, Some(2));
        assert_eq!(page.next_cursor, None);
        let row = &page.items[0];
        assert_eq!(row["owned_by"].as_str(), Some(alice.to_string().as_str()));
        assert_eq!(row["metadata"]["tier"].as_u64(), Some(3));
        assert!(row["logo"].is_null());
    }

    #[tokio::test]
    async fn domains_reject_fields_they_do_not_have() {
        let (state, _, _) = fixture();
        let err = run(
            &state,
            &CollectionTarget::Domains,
            ListQuery::new().filter(field("owner").eq("x")),
        )
        .await
        .expect_err("unknown field");
        let Error::CollectionQuery(err) = err else {
            panic!("expected a collection error, got {err:?}");
        };
        assert_eq!(err.code, "invalid_filter");
        assert_eq!(err.actual.as_deref(), Some("owner"));
        assert!(
            err.expected
                .as_deref()
                .is_some_and(|fields| fields.contains("owned_by"))
        );
    }

    #[tokio::test]
    async fn accounts_rows_expose_identity_label_and_metadata() {
        let (state, alice, bob) = fixture();
        let page = run(
            &state,
            &CollectionTarget::Accounts,
            ListQuery::new().include_total(),
        )
        .await
        .expect("page");
        assert_eq!(page.total, Some(2));
        let mut expected = vec![alice.to_string(), bob.to_string()];
        expected.sort();
        assert_eq!(ids(&page), expected);
        for row in &page.items {
            assert!(row.contains_key("label"));
            assert!(row.contains_key("uaid"));
            assert!(row["metadata"].is_object());
        }
    }

    #[tokio::test]
    async fn account_transactions_page_newest_first_by_block_coordinates() {
        use iroha_core::{
            smartcontracts::Execute as _,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
            tx::AcceptedTransaction,
        };
        let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::default(), 1))
            .expect("fixture genesis");
        let state = Arc::clone(chain.state());
        let keys = checked_routing_fixture_keypair(0xC3, Algorithm::Ed25519, "history fixture key");
        let alice = dm::AccountId::new(keys.public_key().clone());
        {
            let latest = state.view().latest_block_hash();
            let mut block = state.block(iroha_data_model::block::BlockHeader::new(
                core::num::NonZeroU64::new(chain.height() + 1).unwrap(),
                latest,
                None,
                1_000,
                0,
            ));
            let mut tx = block.transaction();
            dm::Register::account(dm::Account::new(alice.account().clone()))
                .execute(alice.account(), &mut tx)
                .ok();
            tx.apply();
            block
                .commit_world_overlay_for_testing()
                .expect("seed fixture account");
        }
        let network_id = *state.network_id_ref();
        let signed = |millis: u64| {
            let mut builder = dm::TransactionBuilder::new(
                network_id,
                alice.clone().into(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            );
            builder.set_creation_time(core::time::Duration::from_millis(millis));
            let signed = builder
                .with_instructions::<dm::InstructionBox>([dm::Log::new(
                    dm::Level::INFO,
                    format!("tx {millis}"),
                )
                .into()])
                .sign(keys.private_key());
            AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(signed))
        };
        for pair in [[1_000, 1_001], [2_000, 2_001], [3_000, 3_001]] {
            crate::test_utils::commit_native_accepted_inputs(
                &mut chain,
                pair.into_iter().map(signed).collect(),
            );
        }
        let target = CollectionTarget::AccountTransactions(alice.to_string());
        let coordinates = |page: &RowPage| -> Vec<(u64, u64)> {
            page.items
                .iter()
                .map(|row| {
                    assert_eq!(row["authority"].as_str(), Some(alice.to_string().as_str()));
                    (
                        row["block_height"].as_u64().unwrap(),
                        row["block_index"].as_u64().unwrap(),
                    )
                })
                .collect()
        };
        let first = run(&state, &target, ListQuery::new().limit(4))
            .await
            .expect("page");
        let second = run(
            &state,
            &target,
            ListQuery::new()
                .limit(4)
                .cursor(first.next_cursor.clone().expect("more")),
        )
        .await
        .expect("page");
        assert_eq!(second.next_cursor, None);
        let walked: Vec<(u64, u64)> = [coordinates(&first), coordinates(&second)].concat();
        assert_eq!(walked.len(), 6);
        assert!(
            walked.windows(2).all(|pair| pair[0] > pair[1]),
            "newest first: {walked:?}"
        );
        let middle = walked[2].0;
        let ranged = run(
            &state,
            &target,
            ListQuery::new().filter(field("block_height").eq(middle)),
        )
        .await
        .expect("page");
        assert_eq!(coordinates(&ranged), walked[2..4].to_vec());
        assert_eq!(
            ranged.next_cursor, None,
            "the walk stops below the height range"
        );
        let every = run(
            &state,
            &CollectionTarget::Transactions,
            ListQuery::new().limit(100),
        )
        .await
        .expect("page");
        assert!(every.items.len() >= 6);
        let err = run(
            &state,
            &target,
            ListQuery::new().sort_by(SortKey::asc("timestamp_ms")),
        )
        .await
        .expect_err("history order is fixed");
        assert!(matches!(err, Error::CollectionQuery(err) if err.code == "invalid_sort"));
    }

    #[tokio::test]
    async fn account_filters_take_canonical_ids_and_never_resolve_aliases() {
        let (state, alice, _) = fixture();
        // Alias resolution is permissioned, so a generic filter must not
        // perform it: alias literals are rejected, canonical ids match.
        let err = run(
            &state,
            &CollectionTarget::Accounts,
            ListQuery::new().filter(field("id").eq("alice@wonderland.universal")),
        )
        .await
        .expect_err("alias literal");
        let Error::CollectionQuery(err) = err else {
            panic!("expected a collection error, got {err:?}");
        };
        assert_eq!(err.code, "invalid_filter");
        assert_eq!(err.actual.as_deref(), Some("id"));
        let page = run(
            &state,
            &CollectionTarget::Accounts,
            ListQuery::new().filter(field("id").eq(alice.to_string())),
        )
        .await
        .expect("canonical literal");
        assert_eq!(ids(&page), vec![alice.to_string()]);
    }

    #[tokio::test]
    async fn projection_happens_after_execution() {
        let (state, _, _) = fixture();
        let query = ListQuery::new().select(["id"]).limit(1);
        let limits = collection_limits();
        let prepared = collections::prepare(&specs::DOMAINS, "", &query, &limits).expect("valid");
        let page = run(&state, &CollectionTarget::Domains, prepared.route_query())
            .await
            .expect("page");
        assert!(
            page.items[0].contains_key("owned_by"),
            "routes return full rows"
        );
        let projected = prepared.project(page);
        assert_eq!(projected.items[0].len(), 1);
    }
}
