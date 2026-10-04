//! Torii collection reads: one query language and one page envelope for every collection.
//!
//! Domains, accounts, asset definitions, NFTs, RWA lots, account assets, asset
//! holders, permissions, transactions, subscription plans, subscriptions, UAID
//! manifests and repo agreements are all read
//! with a [`ListQuery`] and return a [`Page`] of JSON rows. The wire contract is
//! `specs/torii/collection_queries.md`; the query language itself is the shared
//! reference implementation re-exported here from
//! [`iroha_torii_shared::list_query`], so filters render and validate exactly
//! as Torii parses them.
//!
//! Reads use `POST <collection>/query` with the canonical JSON body and
//! `Accept: application/json`. [`Client`] reads are public and unsigned;
//! [`AccountClient`] reads carry the account's canonical request signature when
//! it can sign directly, which only widens visibility into restricted dataspaces.
//!
//! ```no_run
//! use iroha::collections::{Collection, ListQuery, SortKey, TryStreamExt as _, field};
//!
//! # async fn example(client: iroha::client::Client) -> iroha::Result<()> {
//! let query = ListQuery::new()
//!     .filter(field("owned_by").eq("sorau…") & field("metadata.tier").is_not_null())
//!     .sort_by(SortKey::desc("id"))
//!     .limit(50);
//!
//! // One page, with the cursor of the next one.
//! let page = client.list_page(&Collection::AssetDefinitions, &query).await?;
//! println!("{} rows, more: {}", page.items.len(), page.has_more());
//!
//! // Every matching row; further pages are fetched lazily.
//! let mut rows = client.list(Collection::AssetDefinitions, query);
//! while let Some(row) = rows.try_next().await? {
//!     println!("{:?}", row.get("id"));
//! }
//! # Ok(())
//! # }
//! ```
//!
//! A text filter parses into the same tree: `"owned_by = \"sorau…\"".parse::<FilterExpr>()`.
//! Rendering and re-parsing text yields the same tree for every filter without
//! object or array literals. Those literals, valid only against
//! `metadata.<key>` values, exist only in the JSON form, which is what the SDK
//! sends; decimals are exact strings and fractional JSON numbers are rejected,
//! also inside object literals.
//!
//! # History collections
//!
//! [`Collection::Transactions`], [`Collection::AccountTransactions`],
//! [`Collection::ContractActivity`] and [`Collection::ContractEvents`] are read
//! newest first by `block_height`, then `block_index`, and their cursors hold
//! block coordinates. They reject `sort`, `include_total` and `aggregate`
//! ([`Collection::validate_query`] rejects them before dispatch with Torii's
//! codes). Each page has a bounded scan budget, so a page may hold fewer than
//! `limit` rows, or none, and still carry a `next_cursor`; [`CollectionStream`]
//! and the blocking iterator keep following it until it is absent. Bounds on
//! `block_height` in the filter's top-level `and` also bound the scan:
//! `field("block_height").gte(1_200) & field("result_ok").eq(true)` reads only
//! heights from 1200 upwards.
//!
//! # Aggregates
//!
//! [`ListQuery::aggregate`] groups rows where they live. A read whose visible
//! rows span several dataspace routes is rejected with `invalid_aggregate`,
//! because routes may hold overlapping rows that cannot be summed exactly; page
//! through the rows without an aggregate instead.

use std::{
    borrow::Cow,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, ready},
};

use futures_util::stream::FusedStream;
pub use futures_util::stream::{Stream, StreamExt, TryStreamExt};
pub use iroha_torii_shared::list_query::*;
use norito::json::Value;
use url::Url;

use crate::{
    Error, Result,
    account_address::encode_account_id_to_i105,
    client::{AccountClient, Client},
    data_model::{account::AccountId, asset::AssetDefinitionId, nexus::UniversalAccountId},
};

/// One Torii collection endpoint.
///
/// Parameterised collections carry the account or asset definition whose rows
/// they list. Every collection is read through `POST <route>/query`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Collection {
    /// Domains: `/v1/domains`.
    Domains,
    /// Accounts: `/v1/accounts`.
    Accounts,
    /// Asset definitions: `/v1/assets/definitions`.
    AssetDefinitions,
    /// NFTs: `/v1/nfts`.
    Nfts,
    /// RWA lots: `/v1/rwas`.
    Rwas,
    /// Balances held by one account: `/v1/accounts/{account_id}/assets`.
    AccountAssets(AccountId),
    /// Effective permissions of one account: `/v1/accounts/{account_id}/permissions`.
    AccountPermissions(AccountId),
    /// Accounts holding one asset definition: `/v1/assets/{definition_id}/holders`.
    AssetHolders(AssetDefinitionId),
    /// Committed transactions, newest first: `/v1/transactions` (query only).
    ///
    /// A [history collection](self#history-collections). Rows carry
    /// `entrypoint_hash`, `block_height` and `block_index` (always present),
    /// `block_hash`, `authority` (string or null), `timestamp_ms` (number or
    /// null), `entrypoint_kind`, `result_ok`, `asset_ids`,
    /// `asset_definition_ids` (lists of strings, matched element-wise) and
    /// `metadata`.
    Transactions,
    /// Transactions one account signed or that reference it, newest first:
    /// `/v1/accounts/{account_id}/transactions`.
    ///
    /// A [history collection](self#history-collections) with the rows of
    /// [`Self::Transactions`].
    AccountTransactions(AccountId),
    /// Asset movement history of one account: `/v1/accounts/{account_id}/history`.
    AccountHistory(AccountId),
    /// Contract activity history: `/v1/contracts/activity`.
    ContractActivity,
    /// Contract event history: `/v1/contracts/events`.
    ContractEvents,
    /// Bounded Explorer accounts feed: `/v1/explorer/accounts`.
    ExplorerAccounts,
    /// Bounded Explorer domains feed: `/v1/explorer/domains`.
    ExplorerDomains,
    /// Bounded Explorer asset-definitions feed: `/v1/explorer/asset-definitions`.
    ExplorerAssetDefinitions,
    /// Bounded Explorer assets feed: `/v1/explorer/assets`.
    ExplorerAssets,
    /// Bounded Explorer nfts feed: `/v1/explorer/nfts`.
    ExplorerNfts,
    /// Bounded Explorer rwas feed: `/v1/explorer/rwas`.
    ExplorerRwas,
    /// Bounded Explorer blocks feed: `/v1/explorer/blocks`.
    ExplorerBlocks,
    /// Bounded Explorer transactions feed: `/v1/explorer/transactions`.
    ExplorerTransactions,
    /// Bounded Explorer transactions/latest feed: `/v1/explorer/transactions/latest`.
    ExplorerTransactionsLatest,
    /// Bounded Explorer instructions feed: `/v1/explorer/instructions`.
    ExplorerInstructions,
    /// Bounded Explorer instructions/latest feed: `/v1/explorer/instructions/latest`.
    ExplorerInstructionsLatest,
    /// Repo agreements: `/v1/repo/agreements`.
    RepoAgreements,
    /// Subscription plans: `/v1/subscriptions/plans`.
    SubscriptionPlans,
    /// Subscription state: `/v1/subscriptions`.
    Subscriptions,
    /// Capability manifests of one UAID: `/v1/space-directory/uaids/{uaid}/manifests`.
    UaidManifests(UniversalAccountId),
}

impl Collection {
    /// Stable `snake_case` name, for example `asset_definitions`.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Domains => "domains",
            Self::Accounts => "accounts",
            Self::AssetDefinitions => "asset_definitions",
            Self::Nfts => "nfts",
            Self::Rwas => "rwas",
            Self::AccountAssets(_) => "account_assets",
            Self::AccountPermissions(_) => "account_permissions",
            Self::AssetHolders(_) => "asset_holders",
            Self::Transactions => "transactions",
            Self::AccountTransactions(_) => "account_transactions",
            Self::AccountHistory(_) => "account_history",
            Self::ContractActivity => "contract_activity",
            Self::ContractEvents => "contract_events",
            Self::ExplorerAccounts => "explorer_accounts",
            Self::ExplorerDomains => "explorer_domains",
            Self::ExplorerAssetDefinitions => "explorer_asset_definitions",
            Self::ExplorerAssets => "explorer_assets",
            Self::ExplorerNfts => "explorer_nfts",
            Self::ExplorerRwas => "explorer_rwas",
            Self::ExplorerBlocks => "explorer_blocks",
            Self::ExplorerTransactions => "explorer_transactions",
            Self::ExplorerTransactionsLatest => "explorer_transactions_latest",
            Self::ExplorerInstructions => "explorer_instructions",
            Self::ExplorerInstructionsLatest => "explorer_instructions_latest",
            Self::RepoAgreements => "repo_agreements",
            Self::SubscriptionPlans => "subscription_plans",
            Self::Subscriptions => "subscriptions",
            Self::UaidManifests(_) => "uaid_manifests",
        }
    }

    /// Route template, for example `/v1/accounts/{account_id}/assets`.
    ///
    /// Queries go to `<route>/query`. Every route except `/v1/transactions`
    /// also serves `GET` list requests.
    pub const fn route(&self) -> &'static str {
        match self {
            Self::Domains => "/v1/domains",
            Self::Accounts => "/v1/accounts",
            Self::AssetDefinitions => "/v1/assets/definitions",
            Self::Nfts => "/v1/nfts",
            Self::Rwas => "/v1/rwas",
            Self::AccountAssets(_) => "/v1/accounts/{account_id}/assets",
            Self::AccountPermissions(_) => "/v1/accounts/{account_id}/permissions",
            Self::AssetHolders(_) => "/v1/assets/{definition_id}/holders",
            Self::Transactions => "/v1/transactions",
            Self::AccountTransactions(_) => "/v1/accounts/{account_id}/transactions",
            Self::AccountHistory(_) => "/v1/accounts/{account_id}/history",
            Self::ContractActivity => "/v1/contracts/activity",
            Self::ContractEvents => "/v1/contracts/events",
            Self::ExplorerAccounts => "/v1/explorer/accounts",
            Self::ExplorerDomains => "/v1/explorer/domains",
            Self::ExplorerAssetDefinitions => "/v1/explorer/asset-definitions",
            Self::ExplorerAssets => "/v1/explorer/assets",
            Self::ExplorerNfts => "/v1/explorer/nfts",
            Self::ExplorerRwas => "/v1/explorer/rwas",
            Self::ExplorerBlocks => "/v1/explorer/blocks",
            Self::ExplorerTransactions => "/v1/explorer/transactions",
            Self::ExplorerTransactionsLatest => "/v1/explorer/transactions/latest",
            Self::ExplorerInstructions => "/v1/explorer/instructions",
            Self::ExplorerInstructionsLatest => "/v1/explorer/instructions/latest",
            Self::RepoAgreements => "/v1/repo/agreements",
            Self::SubscriptionPlans => "/v1/subscriptions/plans",
            Self::Subscriptions => "/v1/subscriptions",
            Self::UaidManifests(_) => "/v1/space-directory/uaids/{uaid}/manifests",
        }
    }

    /// Rendered route, for example `/v1/accounts/<account>/assets`.
    ///
    /// Account ids are rendered as canonical I105 literals for `chain_discriminant`
    /// (use [`Client::account_chain_discriminant`]); asset definition ids are
    /// Base58 literals. The path is not percent-encoded; the client encodes each
    /// segment when it builds the request URL.
    ///
    /// # Errors
    /// Returns [`Error::InvalidRequest`] when an account id has no I105 form.
    pub fn path(&self, chain_discriminant: u16) -> Result<String> {
        let mut path = String::new();
        for segment in self.segments(chain_discriminant)? {
            path.push('/');
            path.push_str(&segment);
        }
        Ok(path)
    }

    /// Rendered `POST` query path: [`Self::path`] followed by `/query`.
    ///
    /// # Errors
    /// Returns [`Error::InvalidRequest`] when an account id has no I105 form.
    pub fn query_path(&self, chain_discriminant: u16) -> Result<String> {
        let mut path = self.path(chain_discriminant)?;
        path.push_str("/query");
        Ok(path)
    }

    /// Whether this is a [history collection](self#history-collections): read
    /// newest first by block coordinates, without `sort`, `include_total` or
    /// `aggregate`.
    pub const fn is_history(&self) -> bool {
        matches!(
            self,
            Self::Transactions
                | Self::AccountTransactions(_)
                | Self::AccountHistory(_)
                | Self::ContractActivity
                | Self::ContractEvents
                | Self::ExplorerBlocks
                | Self::ExplorerTransactions
                | Self::ExplorerTransactionsLatest
                | Self::ExplorerInstructions
                | Self::ExplorerInstructionsLatest
        )
    }

    /// Whether the collection uses a bounded fixed-order scanner and rejects
    /// sorting, aggregation and exact totals.
    pub const fn has_fixed_order(&self) -> bool {
        self.is_history()
            || matches!(
                self,
                Self::ExplorerAccounts
                    | Self::ExplorerDomains
                    | Self::ExplorerAssetDefinitions
                    | Self::ExplorerAssets
                    | Self::ExplorerNfts
                    | Self::ExplorerRwas
            )
    }

    /// Check `query` against the list-query rules and this collection's own
    /// restrictions without contacting Torii.
    ///
    /// Every read runs this check before dispatch.
    ///
    /// # Errors
    /// Returns the first invalid control; [`ListQueryError::code`] is the code
    /// Torii returns for it, for example `invalid_sort` for a sorted history read.
    pub fn validate_query(&self, query: &ListQuery) -> core::result::Result<(), ListQueryError> {
        query.validate()?;
        if !self.has_fixed_order() {
            return Ok(());
        }
        let rejected = if !query.sort.is_empty() {
            Some(("sort", "`sort`"))
        } else if query.include_total {
            Some(("include_total", "`include_total`"))
        } else if query.aggregate.is_some() {
            Some(("aggregate", "`aggregate`"))
        } else {
            None
        };
        rejected.map_or(Ok(()), |(control, spelling)| {
            Err(ListQueryError::new(
                control,
                format!(
                    "{spelling} is not supported by `{}`: this bounded collection has a fixed order; use filter, select, limit and cursor",
                    self.name()
                ),
            ))
        })
    }

    /// Canonical operation name used in errors, for example `collections.asset_definitions`.
    pub(crate) const fn operation(&self) -> &'static str {
        match self {
            Self::Domains => "collections.domains",
            Self::Accounts => "collections.accounts",
            Self::AssetDefinitions => "collections.asset_definitions",
            Self::Nfts => "collections.nfts",
            Self::Rwas => "collections.rwas",
            Self::AccountAssets(_) => "collections.account_assets",
            Self::AccountPermissions(_) => "collections.account_permissions",
            Self::AssetHolders(_) => "collections.asset_holders",
            Self::Transactions => "collections.transactions",
            Self::AccountTransactions(_) => "collections.account_transactions",
            Self::AccountHistory(_) => "collections.account_history",
            Self::ContractActivity => "collections.contract_activity",
            Self::ContractEvents => "collections.contract_events",
            Self::ExplorerAccounts => "collections.explorer_accounts",
            Self::ExplorerDomains => "collections.explorer_domains",
            Self::ExplorerAssetDefinitions => "collections.explorer_asset_definitions",
            Self::ExplorerAssets => "collections.explorer_assets",
            Self::ExplorerNfts => "collections.explorer_nfts",
            Self::ExplorerRwas => "collections.explorer_rwas",
            Self::ExplorerBlocks => "collections.explorer_blocks",
            Self::ExplorerTransactions => "collections.explorer_transactions",
            Self::ExplorerTransactionsLatest => "collections.explorer_transactions_latest",
            Self::ExplorerInstructions => "collections.explorer_instructions",
            Self::ExplorerInstructionsLatest => "collections.explorer_instructions_latest",
            Self::RepoAgreements => "collections.repo_agreements",
            Self::SubscriptionPlans => "collections.subscription_plans",
            Self::Subscriptions => "collections.subscriptions",
            Self::UaidManifests(_) => "collections.uaid_manifests",
        }
    }

    /// Percent-encoded `POST` query URL below the Torii API root `base`.
    pub(crate) fn query_url(&self, base: &Url, chain_discriminant: u16) -> Result<Url> {
        let segments = self.segments(chain_discriminant)?;
        let mut url = base.clone();
        url.path_segments_mut()
            .map_err(|()| Error::InvalidRequest {
                operation: self.operation(),
                details: "Torii endpoint cannot carry path segments".to_owned(),
            })?
            .pop_if_empty()
            .extend(segments.iter().map(AsRef::<str>::as_ref))
            .push("query");
        Ok(url)
    }

    fn segments(&self, chain_discriminant: u16) -> Result<Vec<Cow<'static, str>>> {
        let account = |id: &AccountId| -> Result<Cow<'static, str>> {
            encode_account_id_to_i105(id, chain_discriminant)
                .map(Cow::Owned)
                .map_err(|error| Error::InvalidRequest {
                    operation: self.operation(),
                    details: format!("account id has no I105 literal: {error}"),
                })
        };
        Ok(match self {
            Self::Domains => vec!["v1".into(), "domains".into()],
            Self::Accounts => vec!["v1".into(), "accounts".into()],
            Self::AssetDefinitions => vec!["v1".into(), "assets".into(), "definitions".into()],
            Self::Nfts => vec!["v1".into(), "nfts".into()],
            Self::Rwas => vec!["v1".into(), "rwas".into()],
            Self::AccountAssets(id) => {
                vec![
                    "v1".into(),
                    "accounts".into(),
                    account(id)?,
                    "assets".into(),
                ]
            }
            Self::AssetHolders(definition) => vec![
                "v1".into(),
                "assets".into(),
                Cow::Owned(definition.to_string()),
                "holders".into(),
            ],
            Self::AccountPermissions(id) => vec![
                "v1".into(),
                "accounts".into(),
                account(id)?,
                "permissions".into(),
            ],
            Self::Transactions => vec!["v1".into(), "transactions".into()],
            Self::AccountTransactions(id) => vec![
                "v1".into(),
                "accounts".into(),
                account(id)?,
                "transactions".into(),
            ],
            Self::AccountHistory(id) => vec![
                "v1".into(),
                "accounts".into(),
                account(id)?,
                "history".into(),
            ],
            Self::ExplorerAccounts => vec!["v1".into(), "explorer".into(), "accounts".into()],
            Self::ExplorerDomains => vec!["v1".into(), "explorer".into(), "domains".into()],
            Self::ExplorerAssetDefinitions => {
                vec!["v1".into(), "explorer".into(), "asset-definitions".into()]
            }
            Self::ExplorerAssets => vec!["v1".into(), "explorer".into(), "assets".into()],
            Self::ExplorerNfts => vec!["v1".into(), "explorer".into(), "nfts".into()],
            Self::ExplorerRwas => vec!["v1".into(), "explorer".into(), "rwas".into()],
            Self::ExplorerBlocks => vec!["v1".into(), "explorer".into(), "blocks".into()],
            Self::ExplorerTransactions => {
                vec!["v1".into(), "explorer".into(), "transactions".into()]
            }
            Self::ExplorerTransactionsLatest => vec![
                "v1".into(),
                "explorer".into(),
                "transactions".into(),
                "latest".into(),
            ],
            Self::ExplorerInstructions => {
                vec!["v1".into(), "explorer".into(), "instructions".into()]
            }
            Self::ExplorerInstructionsLatest => vec![
                "v1".into(),
                "explorer".into(),
                "instructions".into(),
                "latest".into(),
            ],
            Self::RepoAgreements => vec!["v1".into(), "repo".into(), "agreements".into()],
            Self::ContractActivity => vec!["v1".into(), "contracts".into(), "activity".into()],
            Self::ContractEvents => vec!["v1".into(), "contracts".into(), "events".into()],
            Self::SubscriptionPlans => vec!["v1".into(), "subscriptions".into(), "plans".into()],
            Self::Subscriptions => vec!["v1".into(), "subscriptions".into()],
            Self::UaidManifests(uaid) => vec![
                "v1".into(),
                "space-directory".into(),
                "uaids".into(),
                Cow::Owned(uaid.to_string()),
                "manifests".into(),
            ],
        })
    }
}

impl fmt::Display for Collection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// Context that reads one collection page: public and unsigned, or account-bound.
#[derive(Clone, Debug)]
pub(crate) enum CollectionReader {
    /// Public reads through a [`Client`].
    Public(Arc<Client>),
    /// Reads signed by an [`AccountClient`] when it can sign directly.
    Account(AccountClient),
}

impl CollectionReader {
    pub(crate) async fn page(
        &self,
        collection: &Collection,
        query: &ListQuery,
    ) -> Result<Page<Value>> {
        match self {
            Self::Public(client) => client.list_page(collection, query).await,
            Self::Account(account) => account.list_page(collection, query).await,
        }
    }
}

/// Cursor-following state shared by [`CollectionStream`] and the blocking iterator.
#[derive(Debug)]
pub(crate) struct Pager {
    pub(crate) reader: CollectionReader,
    pub(crate) collection: Collection,
    next: Option<ListQuery>,
    buffer: std::vec::IntoIter<Value>,
    total: Option<u64>,
}

impl Pager {
    pub(crate) fn new(reader: CollectionReader, collection: Collection, query: ListQuery) -> Self {
        Self {
            reader,
            collection,
            next: Some(query),
            buffer: Vec::new().into_iter(),
            total: None,
        }
    }

    /// Next buffered row of the current page.
    pub(crate) fn pop(&mut self) -> Option<Value> {
        self.buffer.next()
    }

    /// Request for the next page, or `None` after the last page or an error.
    pub(crate) fn take_request(&mut self) -> Option<ListQuery> {
        self.next.take()
    }

    /// Buffer `page` and position the next request after it.
    pub(crate) fn accept(&mut self, request: &ListQuery, page: Page<Value>) -> Result<()> {
        if page.next_cursor.is_some() && page.next_cursor == request.cursor {
            // A cursor that does not advance would repeat the same page forever.
            return Err(Error::ResponseBinding {
                operation: self.collection.operation(),
                field: "next_cursor",
            });
        }
        if page.total.is_some() {
            self.total = page.total;
        }
        self.next = request.next_page(&page);
        self.buffer = page.items.into_iter();
        Ok(())
    }

    /// Stop after an error: no further rows or requests.
    pub(crate) fn finish(&mut self) {
        self.next = None;
        self.buffer = Vec::new().into_iter();
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.next.is_none() && self.buffer.len() == 0
    }

    pub(crate) fn buffered(&self) -> usize {
        self.buffer.len()
    }

    pub(crate) const fn total(&self) -> Option<u64> {
        self.total
    }
}

type PendingPage = Pin<Box<dyn Future<Output = (ListQuery, Result<Page<Value>>)> + Send>>;

/// Every row of one collection query, fetched lazily page by page.
///
/// Created by [`Client::list`] or [`AccountClient::list`]. Rows arrive in the
/// requested order; the next page is requested only after the current one is
/// consumed, by passing its `next_cursor` back unchanged. A short or empty page
/// with a `next_cursor` is not the end: the stream ends only after a page
/// without one. The first error ends the stream, as does a `next_cursor` equal
/// to the cursor just sent. Dropping the stream cancels a pending page request.
pub struct CollectionStream {
    pager: Pager,
    pending: Option<PendingPage>,
}

impl CollectionStream {
    pub(crate) fn new(reader: CollectionReader, collection: Collection, query: ListQuery) -> Self {
        Self {
            pager: Pager::new(reader, collection, query),
            pending: None,
        }
    }

    /// The collection this stream reads.
    pub const fn collection(&self) -> &Collection {
        &self.pager.collection
    }

    /// Exact number of matching rows reported by the latest page that carried
    /// one; present only when the query asked for [`ListQuery::include_total`].
    pub const fn total(&self) -> Option<u64> {
        self.pager.total()
    }
}

impl fmt::Debug for CollectionStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollectionStream")
            .field("collection", &self.pager.collection)
            .field("buffered", &self.pager.buffered())
            .field("pending", &self.pending.is_some())
            .field("total", &self.pager.total())
            .finish_non_exhaustive()
    }
}

impl Stream for CollectionStream {
    type Item = Result<Value>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if let Some(row) = this.pager.pop() {
                return Poll::Ready(Some(Ok(row)));
            }
            let pending = match &mut this.pending {
                Some(pending) => pending,
                slot => {
                    let Some(request) = this.pager.take_request() else {
                        return Poll::Ready(None);
                    };
                    let reader = this.pager.reader.clone();
                    let collection = this.pager.collection.clone();
                    slot.insert(Box::pin(async move {
                        let page = reader.page(&collection, &request).await;
                        (request, page)
                    }))
                }
            };
            let (request, page) = ready!(pending.as_mut().poll(cx));
            this.pending = None;
            if let Err(error) = page.and_then(|page| this.pager.accept(&request, page)) {
                this.pager.finish();
                return Poll::Ready(Some(Err(error)));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let buffered = self.pager.buffered();
        if self.pending.is_none() && self.pager.next.is_none() {
            (buffered, Some(buffered))
        } else {
            (buffered, None)
        }
    }
}

impl FusedStream for CollectionStream {
    fn is_terminated(&self) -> bool {
        self.pending.is_none() && self.pager.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;

    fn account() -> AccountId {
        AccountId::new(
            KeyPair::try_random()
                .expect("collection test key")
                .public_key()
                .clone(),
        )
    }

    fn definition() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("asset name"),
        )
    }

    fn i105(account: &AccountId, discriminant: u16) -> String {
        encode_account_id_to_i105(account, discriminant).expect("I105 literal")
    }

    #[test]
    fn routes_names_and_paths_match_the_contract() {
        let account = account();
        let definition = definition();
        let discriminant = iroha_torii_shared::TAIRA_CHAIN_DISCRIMINANT;
        let literal = i105(&account, discriminant);
        for (collection, name, route, path) in [
            (
                Collection::Domains,
                "domains",
                "/v1/domains",
                "/v1/domains".to_owned(),
            ),
            (
                Collection::Accounts,
                "accounts",
                "/v1/accounts",
                "/v1/accounts".to_owned(),
            ),
            (
                Collection::AssetDefinitions,
                "asset_definitions",
                "/v1/assets/definitions",
                "/v1/assets/definitions".to_owned(),
            ),
            (Collection::Nfts, "nfts", "/v1/nfts", "/v1/nfts".to_owned()),
            (Collection::Rwas, "rwas", "/v1/rwas", "/v1/rwas".to_owned()),
            (
                Collection::AccountAssets(account.clone()),
                "account_assets",
                "/v1/accounts/{account_id}/assets",
                format!("/v1/accounts/{literal}/assets"),
            ),
            (
                Collection::AssetHolders(definition.clone()),
                "asset_holders",
                "/v1/assets/{definition_id}/holders",
                format!("/v1/assets/{definition}/holders"),
            ),
            (
                Collection::Transactions,
                "transactions",
                "/v1/transactions",
                "/v1/transactions".to_owned(),
            ),
            (
                Collection::AccountTransactions(account.clone()),
                "account_transactions",
                "/v1/accounts/{account_id}/transactions",
                format!("/v1/accounts/{literal}/transactions"),
            ),
            (
                Collection::AccountHistory(account.clone()),
                "account_history",
                "/v1/accounts/{account_id}/history",
                format!("/v1/accounts/{literal}/history"),
            ),
            (
                Collection::ExplorerAccounts,
                "explorer_accounts",
                "/v1/explorer/accounts",
                "/v1/explorer/accounts".to_owned(),
            ),
            (
                Collection::ExplorerDomains,
                "explorer_domains",
                "/v1/explorer/domains",
                "/v1/explorer/domains".to_owned(),
            ),
            (
                Collection::ExplorerAssetDefinitions,
                "explorer_asset_definitions",
                "/v1/explorer/asset-definitions",
                "/v1/explorer/asset-definitions".to_owned(),
            ),
            (
                Collection::ExplorerAssets,
                "explorer_assets",
                "/v1/explorer/assets",
                "/v1/explorer/assets".to_owned(),
            ),
            (
                Collection::ExplorerNfts,
                "explorer_nfts",
                "/v1/explorer/nfts",
                "/v1/explorer/nfts".to_owned(),
            ),
            (
                Collection::ExplorerRwas,
                "explorer_rwas",
                "/v1/explorer/rwas",
                "/v1/explorer/rwas".to_owned(),
            ),
            (
                Collection::ExplorerBlocks,
                "explorer_blocks",
                "/v1/explorer/blocks",
                "/v1/explorer/blocks".to_owned(),
            ),
            (
                Collection::ExplorerTransactions,
                "explorer_transactions",
                "/v1/explorer/transactions",
                "/v1/explorer/transactions".to_owned(),
            ),
            (
                Collection::ExplorerTransactionsLatest,
                "explorer_transactions_latest",
                "/v1/explorer/transactions/latest",
                "/v1/explorer/transactions/latest".to_owned(),
            ),
            (
                Collection::ExplorerInstructions,
                "explorer_instructions",
                "/v1/explorer/instructions",
                "/v1/explorer/instructions".to_owned(),
            ),
            (
                Collection::ExplorerInstructionsLatest,
                "explorer_instructions_latest",
                "/v1/explorer/instructions/latest",
                "/v1/explorer/instructions/latest".to_owned(),
            ),
            (
                Collection::AccountPermissions(account.clone()),
                "account_permissions",
                "/v1/accounts/{account_id}/permissions",
                format!("/v1/accounts/{literal}/permissions"),
            ),
            (
                Collection::SubscriptionPlans,
                "subscription_plans",
                "/v1/subscriptions/plans",
                "/v1/subscriptions/plans".to_owned(),
            ),
            (
                Collection::Subscriptions,
                "subscriptions",
                "/v1/subscriptions",
                "/v1/subscriptions".to_owned(),
            ),
            (
                Collection::UaidManifests(UniversalAccountId::from_hash(iroha_crypto::Hash::new(
                    b"collection-uaid",
                ))),
                "uaid_manifests",
                "/v1/space-directory/uaids/{uaid}/manifests",
                format!(
                    "/v1/space-directory/uaids/{}/manifests",
                    UniversalAccountId::from_hash(iroha_crypto::Hash::new(b"collection-uaid"))
                ),
            ),
            (
                Collection::ContractActivity,
                "contract_activity",
                "/v1/contracts/activity",
                "/v1/contracts/activity".to_owned(),
            ),
            (
                Collection::ContractEvents,
                "contract_events",
                "/v1/contracts/events",
                "/v1/contracts/events".to_owned(),
            ),
            (
                Collection::RepoAgreements,
                "repo_agreements",
                "/v1/repo/agreements",
                "/v1/repo/agreements".to_owned(),
            ),
        ] {
            assert_eq!(collection.name(), name);
            assert_eq!(collection.to_string(), name);
            assert_eq!(collection.route(), route);
            assert_eq!(collection.path(discriminant).expect("path"), path);
            assert_eq!(
                collection.query_path(discriminant).expect("query path"),
                format!("{path}/query")
            );
            assert_eq!(collection.operation(), format!("collections.{name}"));
        }
    }

    #[test]
    fn history_collections_reject_sort_total_and_aggregate_before_dispatch() {
        let aggregate = AggregateSpec {
            group_by: vec!["authority".into()],
            metrics: vec![AggregateMetric {
                alias: "transactions".into(),
                r#fn: AggregateFn::Count,
                field: None,
            }],
            having: None,
        };
        let bounded = ListQuery::new()
            .filter(field("block_height").gte(1_200) & field("result_ok").eq(true))
            .select(["entrypoint_hash", "block_height", "block_index"])
            .limit(50)
            .cursor("h1500_i0");
        for collection in [
            Collection::Transactions,
            Collection::AccountTransactions(account()),
            Collection::AccountHistory(account()),
            Collection::ExplorerAccounts,
            Collection::ExplorerDomains,
            Collection::ExplorerAssetDefinitions,
            Collection::ExplorerAssets,
            Collection::ExplorerNfts,
            Collection::ExplorerRwas,
            Collection::ExplorerBlocks,
            Collection::ExplorerTransactions,
            Collection::ExplorerTransactionsLatest,
            Collection::ExplorerInstructions,
            Collection::ExplorerInstructionsLatest,
            Collection::ContractActivity,
            Collection::ContractEvents,
        ] {
            assert!(collection.has_fixed_order());
            assert_eq!(collection.validate_query(&bounded), Ok(()));
            for (query, code) in [
                (
                    ListQuery::new().sort_by(SortKey::desc("block_height")),
                    "invalid_sort",
                ),
                (ListQuery::new().include_total(), "invalid_include_total"),
                (
                    ListQuery::new().aggregate(aggregate.clone()),
                    "invalid_aggregate",
                ),
            ] {
                let error = collection
                    .validate_query(&query)
                    .expect_err("history restriction");
                assert_eq!(error.code(), code, "{collection}: {error}");
                assert!(error.message.contains("fixed order"), "{error}");
            }
            assert_eq!(
                collection
                    .validate_query(&ListQuery::new().limit(0))
                    .expect_err("generic rule")
                    .code(),
                "invalid_limit"
            );
        }
        for collection in [
            Collection::Domains,
            Collection::AssetDefinitions,
            Collection::AssetHolders(definition()),
        ] {
            assert!(!collection.is_history());
            let query = ListQuery::new()
                .sort_by(SortKey::desc("id"))
                .include_total();
            assert_eq!(collection.validate_query(&query), Ok(()));
            assert_eq!(
                collection.validate_query(&ListQuery::new().aggregate(aggregate.clone())),
                Ok(())
            );
        }
    }

    #[test]
    fn builders_and_text_filters_agree() {
        let built = field("owned_by").eq("sorau…") & field("metadata.tier").is_not_null();
        let text = r#"owned_by = "sorau…" and metadata.tier is not null"#;
        assert_eq!(text.parse::<FilterExpr>().expect("text filter"), built);
        assert_eq!(built.to_string(), text);
    }

    #[test]
    fn query_urls_keep_the_endpoint_base_and_percent_encode_ids() {
        let account = account();
        let discriminant = iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT;
        let base = Url::parse("https://torii.example/peer-1/").expect("base URL");
        let url = Collection::AccountAssets(account.clone())
            .query_url(&base, discriminant)
            .expect("query URL");
        let literal = i105(&account, discriminant);
        let mut expected = base.clone();
        expected
            .path_segments_mut()
            .expect("base")
            .pop_if_empty()
            .extend(["v1", "accounts", literal.as_str(), "assets", "query"]);
        assert_eq!(url, expected);
        assert!(url.path().starts_with("/peer-1/v1/accounts/"));
        assert!(url.path().ends_with("/assets/query"));
        assert!(url.query().is_none());
        assert_eq!(
            Collection::Domains
                .query_url(
                    &Url::parse("http://mock.local/").expect("root"),
                    discriminant
                )
                .expect("domains URL")
                .as_str(),
            "http://mock.local/v1/domains/query"
        );
    }
}
