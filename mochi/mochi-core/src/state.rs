//! Lightweight query helpers feeding the MOCHI state explorer views.
//!
//! Accounts, holdings, asset definitions and domains are Torii collection reads
//! (`specs/torii/collection_queries.md`), as is each validator's newest Explorer block.
//! Peers use the signed `FindPeers` query, the only listing that signed `POST /v1/query` admits.
use crate::torii::{ToriiClient, ToriiError};
use iroha::{
    client::AccountClient,
    collections::{Collection, ListQuery, Page},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    query::{
        QueryItemKind, QueryOutput, QueryOutputBatchBox, QueryRequest, QueryWithParams,
        dsl::{CompoundPredicate, SelectorTuple},
        parameters::{FetchSize, ForwardCursor, Pagination, QueryParams, Sorting},
        peer::prelude::FindPeers,
    },
};
use iroha_model_base::peer::PeerId;
use norito::json;
use std::{
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
    time::Duration,
};
/// Set of iterable queries currently supported by the state explorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateQueryKind {
    /// List registered accounts.
    Accounts,
    /// List holdings for all accounts.
    Assets,
    /// List asset definitions.
    AssetDefinitions,
    /// List domains.
    Domains,
    /// List registered peers participating in the network.
    Peers,
}
impl StateQueryKind {
    /// Human-readable label shown in the UI.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            StateQueryKind::Accounts => "Accounts",
            StateQueryKind::Assets => "Assets",
            StateQueryKind::AssetDefinitions => "Asset definitions",
            StateQueryKind::Domains => "Domains",
            StateQueryKind::Peers => "Peers",
        }
    }
    /// Enumerate all query kinds (used by UI selectors).
    #[must_use]
    pub fn all() -> [StateQueryKind; 5] {
        [
            StateQueryKind::Accounts,
            StateQueryKind::Assets,
            StateQueryKind::AssetDefinitions,
            StateQueryKind::Domains,
            StateQueryKind::Peers,
        ]
    }
    /// The collection read behind this listing; peers have none and use `FindPeers`.
    fn collection(self) -> Option<Collection> {
        match self {
            StateQueryKind::Accounts => Some(Collection::Accounts),
            StateQueryKind::Assets => Some(Collection::ExplorerAssets),
            StateQueryKind::AssetDefinitions => Some(Collection::AssetDefinitions),
            StateQueryKind::Domains => Some(Collection::Domains),
            StateQueryKind::Peers => None,
        }
    }
    /// Largest page the read accepts, and whether it can count every match.
    fn page_policy(self) -> (u32, bool) {
        match self {
            // Explorer feeds cap pages at 100 and never count.
            StateQueryKind::Assets => (100, false),
            _ => (500, true),
        }
    }
}
/// The signed `FindPeers` start: pass predicate, default selector, zero offset, no sorting.
fn peers_request(fetch_size: Option<NonZeroU64>) -> QueryRequest {
    QueryRequest::Start(QueryWithParams {
        query: (),
        query_payload: norito::codec::Encode::encode(&FindPeers),
        item: QueryItemKind::PeerId,
        predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<PeerId>::PASS),
        selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<PeerId>::default()),
        params: QueryParams::new(
            Pagination::default(),
            Sorting::default(),
            FetchSize::new(fetch_size),
        ),
    })
}
/// Resulting entries from a state explorer page.
#[derive(Debug, Clone)]
pub struct StateEntry {
    /// Primary identifier, e.g., account id.
    pub title: String,
    /// Short secondary line (owner, signatory, etc.).
    pub subtitle: String,
    /// Additional detail summarising metadata or numeric fields.
    pub detail: String,
    /// Full debug representation for inspection panels.
    pub raw: String,
    /// Owning domain (if applicable).
    pub domain: Option<String>,
    /// Lowercased domain cache for filtering.
    pub domain_lower: Option<String>,
    /// Owning account or authority (if applicable).
    pub owner: Option<String>,
    /// Lowercased owner cache for filtering.
    pub owner_lower: Option<String>,
    /// Asset definition identifier (if applicable).
    pub asset_definition: Option<String>,
    /// Lowercased asset definition cache.
    pub asset_definition_lower: Option<String>,
    /// Pretty-printed Norito JSON payload.
    pub json: Option<String>,
    /// Norito-encoded bytes of the entry payload.
    pub norito_bytes: Option<Vec<u8>>,
    /// Lowercased blob used by UI-side search filters.
    pub search_blob: String,
}
impl StateEntry {
    /// One collection row; Torii's JSON row is the inspectable payload.
    fn from_row(kind: StateQueryKind, row: &json::Value) -> Self {
        let text = |field: &str| {
            row.get(field).and_then(|value| match value.as_str() {
                Some(value) => Some(value.to_owned()),
                None if value.is_null() => None,
                None => json::to_json(value).ok(),
            })
        };
        let id = text("id").unwrap_or_default();
        let (subtitle, detail, domain, owner, asset_definition) = match kind {
            StateQueryKind::Accounts => (
                text("label")
                    .map_or_else(|| "Account".to_owned(), |label| format!("Label {label}")),
                row_metadata_summary(row),
                None,
                None,
                None,
            ),
            StateQueryKind::Assets => {
                let owner = text("account_id").unwrap_or_default();
                (
                    format!("Owner {owner}"),
                    format!("Value {}", text("value").unwrap_or_default()),
                    None,
                    Some(owner),
                    text("definition_id"),
                )
            }
            StateQueryKind::AssetDefinitions => {
                let owner = text("owned_by").unwrap_or_default();
                (
                    format!("Owner {owner}"),
                    text("name")
                        .map_or_else(|| row_metadata_summary(row), |name| format!("Name {name}")),
                    text("owning_domain"),
                    Some(owner),
                    Some(id.clone()),
                )
            }
            StateQueryKind::Domains => {
                let owner = text("owned_by").unwrap_or_default();
                (
                    format!("Owner {owner}"),
                    row_metadata_summary(row),
                    Some(id.clone()),
                    Some(owner),
                    None,
                )
            }
            StateQueryKind::Peers => ("Peer".to_owned(), String::new(), None, None, None),
        };
        let pretty = catch_unwind(AssertUnwindSafe(|| json::to_json_pretty(row)))
            .ok()
            .and_then(Result::ok);
        let search_blob = build_search_blob(&[
            &id,
            &subtitle,
            &detail,
            owner.as_deref().unwrap_or_default(),
            asset_definition.as_deref().unwrap_or_default(),
            domain.as_deref().unwrap_or_default(),
        ]);
        Self {
            title: id,
            subtitle,
            detail,
            raw: pretty
                .clone()
                .or_else(|| json::to_json(row).ok())
                .unwrap_or_default(),
            domain_lower: domain.as_ref().map(|value| value.to_ascii_lowercase()),
            domain,
            owner_lower: owner.as_ref().map(|value| value.to_ascii_lowercase()),
            owner,
            asset_definition_lower: asset_definition
                .as_ref()
                .map(|value| value.to_ascii_lowercase()),
            asset_definition,
            json: pretty,
            norito_bytes: None,
            search_blob,
        }
    }
    fn from_peer_id(peer_id: PeerId) -> Self {
        let title = peer_id.to_string();
        let subtitle = "Peer public key".to_owned();
        let detail = "Registered peer identifier".to_owned();
        let summary_json = build_summary_json(&title, &subtitle, &detail, None, None, None);
        let (json, norito_bytes) = encode_payload(&peer_id);
        let json = Some(json.unwrap_or(summary_json));
        let search_blob = build_search_blob(&[&title, &subtitle, &detail]);
        let raw = format!("{peer_id:#?}");
        Self {
            title,
            subtitle,
            detail,
            raw,
            domain: None,
            domain_lower: None,
            owner: None,
            owner_lower: None,
            asset_definition: None,
            asset_definition_lower: None,
            json,
            norito_bytes,
            search_blob,
        }
    }
}
/// The metadata keys a collection row carries, as the explorer lists them.
fn row_metadata_summary(row: &json::Value) -> String {
    let keys = row
        .get("metadata")
        .and_then(json::Value::as_object)
        .map(|metadata| metadata.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    if keys.is_empty() {
        "No metadata entries".to_owned()
    } else {
        format!("Metadata keys: {keys}")
    }
}
fn encode_payload<T>(value: &T) -> (Option<String>, Option<Vec<u8>>)
where
    T: json::JsonSerialize + norito::NoritoSerialize,
{
    let pretty = catch_unwind(AssertUnwindSafe(|| json::to_json_pretty(value)))
        .ok()
        .and_then(Result::ok);
    let bytes = catch_unwind(AssertUnwindSafe(|| norito::to_bytes(value)))
        .ok()
        .and_then(Result::ok);
    (pretty, bytes)
}
fn build_summary_json(
    title: &str,
    subtitle: &str,
    detail: &str,
    domain: Option<&str>,
    owner: Option<&str>,
    asset_definition: Option<&str>,
) -> String {
    let mut map = json::Map::new();
    map.insert("title".into(), json::Value::String(title.to_owned()));
    map.insert("subtitle".into(), json::Value::String(subtitle.to_owned()));
    map.insert("detail".into(), json::Value::String(detail.to_owned()));
    if let Some(domain) = domain {
        map.insert("domain".into(), json::Value::String(domain.to_owned()));
    }
    if let Some(owner) = owner {
        map.insert("owner".into(), json::Value::String(owner.to_owned()));
    }
    if let Some(asset_definition) = asset_definition {
        map.insert(
            "asset_definition".into(),
            json::Value::String(asset_definition.to_owned()),
        );
    }
    let value = json::Value::Object(map);
    json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_owned())
}
fn build_search_blob(parts: &[&str]) -> String {
    let mut blob = String::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !blob.is_empty() {
            blob.push(' ');
        }
        blob.push_str(&trimmed.to_ascii_lowercase());
    }
    blob
}
/// Cursor handle returned alongside paginated state responses.
#[derive(Debug, Clone)]
pub struct StateCursor {
    inner: CursorKind,
}
#[derive(Debug, Clone)]
enum CursorKind {
    /// A signed query's server-side continuation.
    Query(ForwardCursor),
    /// A collection read's `next_cursor`, with the number of rows already returned.
    Listing { next: String, seen: u64 },
}
/// Materialised page of explorer entries.
#[derive(Debug, Clone)]
pub struct StatePage {
    /// Query this page corresponds to.
    pub kind: StateQueryKind,
    /// Entries rendered in the UI.
    pub entries: Vec<StateEntry>,
    /// Remaining number of items on the server.
    pub remaining: u64,
    /// Cursor for requesting the next batch, if available.
    pub cursor: Option<StateCursor>,
}
/// Errors surfaced while preparing state explorer data.
#[derive(Debug, thiserror::Error)]
pub enum StateQueryError {
    /// HTTP or decoding failure while talking to Torii.
    #[error(transparent)]
    Torii(#[from] ToriiError),
    /// Torii refused or failed a collection read.
    #[error(transparent)]
    Collection(#[from] iroha::Error),
    /// The peer returned a batch variant that does not match the requested query.
    #[error("query {kind:?} returned unsupported batch `{actual}`")]
    UnexpectedBatch {
        /// Requested query kind.
        kind: StateQueryKind,
        /// Batch variant label.
        actual: &'static str,
    },
    /// A cursor from one kind of read was passed to another.
    #[error("the {kind:?} continuation belongs to a different read; query again from the start")]
    CursorMismatch {
        /// Requested query kind.
        kind: StateQueryKind,
    },
}
/// Execute one page of a state explorer query.
///
/// Each invocation either starts a new read (`cursor == None`) or continues one. Accounts,
/// holdings, asset definitions and domains are collection reads signed by `reader`; peers use
/// the signed `FindPeers` query sent through `client`.
///
/// # Errors
/// Torii refusals, transport and decode failures, or a cursor from another read.
pub async fn run_state_query(
    client: ToriiClient,
    reader: &AccountClient,
    signer: &crate::SigningAuthority,
    address_discriminant: u16,
    kind: StateQueryKind,
    cursor: Option<StateCursor>,
    fetch_size: Option<NonZeroU64>,
) -> Result<StatePage, StateQueryError> {
    let cursor = cursor.map(|cursor| cursor.inner);
    let Some(collection) = kind.collection() else {
        let request = match cursor {
            None => peers_request(fetch_size),
            Some(CursorKind::Query(cursor)) => QueryRequest::Continue(cursor),
            Some(CursorKind::Listing { .. }) => {
                return Err(StateQueryError::CursorMismatch { kind });
            }
        };
        let signed = client.sign_query(request, signer.account_id().clone(), signer.key_pair())?;
        let output = client.execute_query(&signed).await?;
        // A thread-local display profile must never be held across an await.
        let _profile =
            iroha_data_model::account::address::ChainDiscriminantGuard::enter(address_discriminant);
        return parse_peer_output(output);
    };
    let (next, seen) = match cursor {
        None => (None, 0),
        Some(CursorKind::Listing { next, seen }) => (Some(next), seen),
        Some(CursorKind::Query(_)) => return Err(StateQueryError::CursorMismatch { kind }),
    };
    let (maximum, counts) = kind.page_policy();
    let mut query = ListQuery::new();
    if let Some(size) = fetch_size {
        query = query.limit(u32::try_from(size.get()).map_or(maximum, |size| size.min(maximum)));
    }
    if counts {
        query = query.include_total();
    }
    if let Some(next) = next {
        query = query.cursor(next);
    }
    let page = reader.list_page(&collection, &query).await?;
    Ok(collection_page(kind, page, seen))
}
/// A collection page as explorer entries, counting what remains when Torii reports a total.
fn collection_page(kind: StateQueryKind, page: Page<json::Value>, seen: u64) -> StatePage {
    let entries = page
        .items
        .iter()
        .map(|row| StateEntry::from_row(kind, row))
        .collect::<Vec<_>>();
    let seen = seen.saturating_add(u64::try_from(entries.len()).unwrap_or(u64::MAX));
    StatePage {
        kind,
        remaining: page.total.map_or(0, |total| total.saturating_sub(seen)),
        cursor: page.next_cursor.map(|next| StateCursor {
            inner: CursorKind::Listing { next, seen },
        }),
        entries,
    }
}
/// A validator's newest block, as that validator's own Explorer reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockTip {
    /// Height of the block; genesis is 1.
    pub height: u64,
    /// Hash of the block; it displays as lowercase hex, like streamed blocks.
    pub hash: HashOf<BlockHeader>,
}
impl BlockTip {
    /// The block of one Explorer row, which spells its hash as a checksummed `hash:` literal.
    fn from_row(row: &json::Value) -> Result<Self, StateQueryError> {
        let malformed = |reason: &str| ToriiError::Decode(format!("Explorer block row: {reason}"));
        let height = row
            .get("height")
            .and_then(json::Value::as_u64)
            .filter(|height| *height > 0)
            .ok_or_else(|| malformed("height must be a positive integer"))?;
        let hash = row.get("hash").ok_or_else(|| malformed("missing hash"))?;
        let hash = json::from_value(hash.clone()).map_err(|error| malformed(&error.to_string()))?;
        Ok(Self { height, hash })
    }
}
/// The newest block `reader`'s validator has applied. Explorer lists blocks newest first, so
/// a one-row page holds it; an empty page means the validator has no block yet.
async fn newest_block(reader: &AccountClient) -> Result<Option<BlockTip>, StateQueryError> {
    let page = reader
        .list_page(&Collection::ExplorerBlocks, &ListQuery::new().limit(1))
        .await?;
    page.items.first().map(BlockTip::from_row).transpose()
}
/// Every reader's newest block, read concurrently. A validator that does not answer within
/// `timeout` fails alone, so the others can still be compared.
pub(crate) async fn newest_blocks(
    readers: &[AccountClient],
    timeout: Duration,
) -> Vec<Result<Option<BlockTip>, StateQueryError>> {
    futures::future::join_all(readers.iter().map(|reader| async move {
        tokio::time::timeout(timeout, newest_block(reader))
            .await
            .unwrap_or_else(|_| {
                Err(ToriiError::Timeout {
                    context: format!("no newest block within {timeout:?}"),
                }
                .into())
            })
    }))
    .await
}
fn parse_peer_output(output: QueryOutput) -> Result<StatePage, StateQueryError> {
    let kind = StateQueryKind::Peers;
    let (batch_tuple, remaining, cursor) = output.into_parts();
    let mut entries = Vec::new();
    for batch in batch_tuple.into_iter() {
        match batch {
            QueryOutputBatchBox::Peer(items) => {
                entries.extend(items.into_iter().map(StateEntry::from_peer_id));
            }
            other => {
                return Err(StateQueryError::UnexpectedBatch {
                    kind,
                    actual: batch_label(&other),
                });
            }
        }
    }
    Ok(StatePage {
        kind,
        entries,
        remaining,
        cursor: cursor.map(|cursor| StateCursor {
            inner: CursorKind::Query(cursor),
        }),
    })
}
fn batch_label(batch: &QueryOutputBatchBox) -> &'static str {
    match batch {
        QueryOutputBatchBox::Account(_) => "Account",
        QueryOutputBatchBox::AccountId(_) => "AccountId",
        QueryOutputBatchBox::Asset(_) => "Asset",
        QueryOutputBatchBox::AssetDefinition(_) => "AssetDefinition",
        QueryOutputBatchBox::Domain(_) => "Domain",
        QueryOutputBatchBox::PublicKey(_) => "PublicKey",
        QueryOutputBatchBox::String(_) => "String",
        QueryOutputBatchBox::Metadata(_) => "Metadata",
        QueryOutputBatchBox::Json(_) => "Json",
        QueryOutputBatchBox::Numeric(_) => "Numeric",
        QueryOutputBatchBox::Name(_) => "Name",
        QueryOutputBatchBox::DomainId(_) => "DomainId",
        QueryOutputBatchBox::AssetId(_) => "AssetId",
        QueryOutputBatchBox::AssetDefinitionId(_) => "AssetDefinitionId",
        QueryOutputBatchBox::RepoAgreement(_) => "RepoAgreement",
        QueryOutputBatchBox::NftId(_) => "NftId",
        QueryOutputBatchBox::Nft(_) => "Nft",
        QueryOutputBatchBox::RwaId(_) => "RwaId",
        QueryOutputBatchBox::Rwa(_) => "Rwa",
        QueryOutputBatchBox::Role(_) => "Role",
        QueryOutputBatchBox::Parameter(_) => "Parameter",
        QueryOutputBatchBox::Permission(_) => "Permission",
        QueryOutputBatchBox::CommittedTransaction(_) => "CommittedTransaction",
        QueryOutputBatchBox::TransactionResult(_) => "TransactionResult",
        QueryOutputBatchBox::TransactionResultHash(_) => "TransactionResultHash",
        QueryOutputBatchBox::TransactionEntrypoint(_) => "TransactionEntrypoint",
        QueryOutputBatchBox::TransactionEntrypointHash(_) => "TransactionEntrypointHash",
        QueryOutputBatchBox::Peer(_) => "Peer",
        QueryOutputBatchBox::RoleId(_) => "RoleId",
        QueryOutputBatchBox::TriggerId(_) => "TriggerId",
        QueryOutputBatchBox::Trigger(_) => "Trigger",
        QueryOutputBatchBox::Action(_) => "Action",
        QueryOutputBatchBox::Block(_) => "Block",
        QueryOutputBatchBox::BlockHeader(_) => "BlockHeader",
        QueryOutputBatchBox::BlockHeaderHash(_) => "BlockHeaderHash",
        QueryOutputBatchBox::ProofRecord(_) => "ProofRecord",
        QueryOutputBatchBox::OracleFeedConfig(_) => "OracleFeedConfig",
        QueryOutputBatchBox::OracleFeedEventRecord(_) => "OracleFeedEventRecord",
        QueryOutputBatchBox::OracleProviderStatsRecord(_) => "OracleProviderStatsRecord",
        QueryOutputBatchBox::OracleDispute(_) => "OracleDispute",
        QueryOutputBatchBox::OracleChangeProposal(_) => "OracleChangeProposal",
        QueryOutputBatchBox::TwitterBindingRecord(_) => "TwitterBindingRecord",
        QueryOutputBatchBox::DefiOracleAttestation(_) => "DefiOracleAttestation",
        QueryOutputBatchBox::AssetEscrowRecord(_) => "AssetEscrowRecord",
        QueryOutputBatchBox::FeeSponsorProgram(_) => "FeeSponsorProgram",
        QueryOutputBatchBox::FeeSponsorProgramId(_) => "FeeSponsorProgramId",
        QueryOutputBatchBox::BondRecord(_) => "BondRecord",
        QueryOutputBatchBox::AssetUidPolicyViewV1(_) => "AssetUidPolicyViewV1",
        QueryOutputBatchBox::AssetFiRelationshipViewV1(_) => "AssetFiRelationshipViewV1",
        QueryOutputBatchBox::AssetUidEnrollmentViewV1(_) => "AssetUidEnrollmentViewV1",
        QueryOutputBatchBox::AssetEscrowScopeOriginalV1(_) => "AssetEscrowScopeOriginalV1",
        QueryOutputBatchBox::AssetFiTransferReceiptV1(_) => "AssetFiTransferReceiptV1",
        QueryOutputBatchBox::AssetFiContextSnapshotV1(_) => "AssetFiContextSnapshotV1",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::POST, MockServer};
    use iroha_data_model::query::{
        QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryRequest, QueryResponse,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use norito::{json, to_bytes};
    fn try_start_mock_server() -> Option<MockServer> {
        std::panic::catch_unwind(MockServer::start)
            .ok()
            .or_else(|| {
                eprintln!(
                    "Skipping test: unable to bind mock HTTP server in sandboxed environment."
                );
                None
            })
    }
    /// A reader for collection reads against `base_url`, signing as Alice.
    fn reader(base_url: &str) -> AccountClient {
        iroha::client::Client::builder(iroha::config::Config {
            chain: "mochi-state-tests".into(),
            network_id: crate::torii::test_network_id(),
            account: ALICE_ID.clone(),
            account_chain_discriminant:
                iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT,
            key_pair: ALICE_KEYPAIR.clone(),
            basic_auth: None,
            api_token: None,
            http_transport_policy: iroha::config::HttpTransportPolicy::Ordinary,
            torii_api_url: base_url.parse().expect("reader endpoint"),
            torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            transaction_ttl: iroha::config::DEFAULT_TRANSACTION_TIME_TO_LIVE,
            transaction_status_timeout: iroha::config::DEFAULT_TRANSACTION_STATUS_TIMEOUT,
            transaction_add_nonce: iroha::config::DEFAULT_TRANSACTION_NONCE,
            sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
            sorafs_anonymity_policy: Default::default(),
            sorafs_rollout_phase: Default::default(),
        })
        .build()
        .expect("valid reader context")
        .account_client()
        .expect("explicit Alice authority")
    }
    fn signer() -> crate::SigningAuthority {
        crate::SigningAuthority::new("fixture", ALICE_ID.clone(), ALICE_KEYPAIR.clone())
    }
    #[test]
    fn peers_request_is_the_admitted_find_peers_start() {
        let QueryRequest::Start(query) = peers_request(None) else {
            panic!("the peers listing must use the iterable start variant");
        };
        assert_eq!(query.item, QueryItemKind::PeerId);
        assert_eq!(
            query.query_payload,
            norito::codec::Encode::encode(&FindPeers)
        );
        assert_eq!(
            query.predicate_bytes,
            norito::codec::Encode::encode(&CompoundPredicate::<PeerId>::PASS)
        );
        for kind in StateQueryKind::all() {
            assert_eq!(
                kind.collection().is_none(),
                kind == StateQueryKind::Peers,
                "only peers stay on signed /v1/query: {kind:?}"
            );
        }
    }
    #[test]
    fn summary_json_handles_optional_fields() {
        let json_text = build_summary_json(
            "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            "Signatory alice",
            "Metadata keys: foo",
            Some("test"),
            Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"),
            None,
        );
        let parsed: json::Value = json::from_str(&json_text).expect("summary json");
        let json::Value::Object(map) = parsed else {
            panic!("expected summary to be encoded as an object");
        };
        assert_eq!(
            map.get("title").and_then(|v| v.as_str()),
            Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        );
        assert_eq!(map.get("domain").and_then(|v| v.as_str()), Some("test"));
        assert_eq!(
            map.get("owner").and_then(|v| v.as_str()),
            Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        );
        assert!(!map.contains_key("asset_definition"));
        let json_text_with_definition = build_summary_json(
            "xor#wallet",
            "Definitions",
            "Identifier projection",
            None,
            None,
            Some("xor#wallet"),
        );
        let parsed: json::Value = json::from_str(&json_text_with_definition).expect("summary json");
        let json::Value::Object(map) = parsed else {
            panic!("expected summary with definition to be encoded as an object");
        };
        assert_eq!(
            map.get("asset_definition").and_then(|v| v.as_str()),
            Some("xor#wallet")
        );
        assert!(!map.contains_key("domain"));
        assert!(!map.contains_key("owner"));
    }
    #[test]
    fn collection_rows_become_entries_for_each_listing() {
        let account = norito::json!({
            "id": "sorau-alice", "label": "alice", "uaid": null, "metadata": {"tier": "gold"}
        });
        let entry = StateEntry::from_row(StateQueryKind::Accounts, &account);
        assert_eq!(entry.title, "sorau-alice");
        assert_eq!(entry.subtitle, "Label alice");
        assert_eq!(entry.detail, "Metadata keys: tier");
        assert!(
            entry
                .json
                .as_deref()
                .is_some_and(|json| json.contains("\"tier\""))
        );
        assert!(entry.norito_bytes.is_none());
        let unlabeled = norito::json!({"id": "sorau-bob", "label": null, "metadata": {}});
        let entry = StateEntry::from_row(StateQueryKind::Accounts, &unlabeled);
        assert_eq!(
            (entry.subtitle.as_str(), entry.detail.as_str()),
            ("Account", "No metadata entries")
        );

        let definition = norito::json!({
            "id": "62Fk4FPc", "name": "XOR", "owned_by": "sorau-alice",
            "owning_domain": "wonderland.universal", "mintable": "Infinitely", "metadata": {}
        });
        let entry = StateEntry::from_row(StateQueryKind::AssetDefinitions, &definition);
        assert_eq!(entry.detail, "Name XOR");
        assert_eq!(entry.asset_definition.as_deref(), Some("62Fk4FPc"));
        assert_eq!(entry.domain.as_deref(), Some("wonderland.universal"));
        assert_eq!(entry.owner.as_deref(), Some("sorau-alice"));
        // The ledger names assets from the row's own `name`.
        let named: json::Value = json::from_str(entry.json.as_deref().unwrap()).unwrap();
        assert_eq!(named.get("name").and_then(json::Value::as_str), Some("XOR"));

        let domain = norito::json!({
            "id": "wonderland.universal", "owned_by": "sorau-alice", "logo": null, "metadata": {}
        });
        let entry = StateEntry::from_row(StateQueryKind::Domains, &domain);
        assert_eq!(entry.subtitle, "Owner sorau-alice");
        assert_eq!(entry.domain_lower.as_deref(), Some("wonderland.universal"));
        assert!(entry.search_blob.contains("wonderland.universal"));
    }
    #[test]
    fn holdings_rows_keep_the_ledger_shape() {
        let holding = norito::json!({
            "id": "62Fk4FPc#sorau-alice", "definition_id": "62Fk4FPc",
            "account_id": "sorau-alice", "value": "999750"
        });
        let entry = StateEntry::from_row(StateQueryKind::Assets, &holding);
        assert_eq!(entry.title, "62Fk4FPc#sorau-alice");
        assert_eq!(entry.owner.as_deref(), Some("sorau-alice"));
        assert_eq!(entry.asset_definition.as_deref(), Some("62Fk4FPc"));
        assert_eq!(entry.detail, "Value 999750");
        let numeric = norito::json!({
            "id": "62Fk4FPc#sorau-bob", "definition_id": "62Fk4FPc",
            "account_id": "sorau-bob", "value": 5
        });
        assert_eq!(
            StateEntry::from_row(StateQueryKind::Assets, &numeric).detail,
            "Value 5"
        );
    }
    #[test]
    fn collection_pages_count_what_remains_and_carry_their_cursor() {
        let row = |id: &str| norito::json!({"id": id, "metadata": {}});
        let first = collection_page(
            StateQueryKind::Accounts,
            Page {
                items: vec![row("a"), row("b")],
                next_cursor: Some("next".into()),
                total: Some(3),
            },
            0,
        );
        assert_eq!(first.remaining, 1);
        let Some(StateCursor {
            inner: CursorKind::Listing { next, seen },
        }) = first.cursor.clone()
        else {
            panic!("a listing continuation");
        };
        assert_eq!((next.as_str(), seen), ("next", 2));
        let last = collection_page(StateQueryKind::Accounts, Page::last(vec![row("c")]), seen);
        assert_eq!(last.remaining, 0);
        assert!(last.cursor.is_none());
        // Explorer feeds never count; their continuation alone says more rows exist.
        let feed = collection_page(
            StateQueryKind::Assets,
            Page {
                items: vec![row("x")],
                next_cursor: Some("more".into()),
                total: None,
            },
            0,
        );
        assert_eq!(feed.remaining, 0);
        assert!(feed.cursor.is_some());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn accounts_page_through_the_signed_collection_read() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let first = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/accounts/query")
                .is_true(|request| {
                    let body: json::Value =
                        json::from_slice(request.body_ref()).unwrap_or(json::Value::Null);
                    body.get("cursor").is_none()
                        && body.get("include_total").and_then(json::Value::as_bool) == Some(true)
                        && body.get("limit").and_then(json::Value::as_u64) == Some(2)
                });
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"items": [{"id": "a", "metadata": {}}, {"id": "b", "metadata": {}}], "next_cursor": "c2", "total": 3}"#);
        });
        let second = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/accounts/query")
                .is_true(|request| {
                    let body: json::Value =
                        json::from_slice(request.body_ref()).unwrap_or(json::Value::Null);
                    body.get("cursor").and_then(json::Value::as_str) == Some("c2")
                });
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{"items": [{"id": "c", "metadata": {}}], "next_cursor": null, "total": 3}"#,
                );
        });
        let client = ToriiClient::new_for_network(server.url("/"), crate::torii::test_network_id())
            .expect("client");
        let reader = reader(&server.url("/"));
        let discriminant = iroha_data_model::account::address::chain_discriminant();
        let size = NonZeroU64::new(2);
        let page = run_state_query(
            client.clone(),
            &reader,
            &signer(),
            discriminant,
            StateQueryKind::Accounts,
            None,
            size,
        )
        .await
        .expect("first page");
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.remaining, 1);
        let next = run_state_query(
            client.clone(),
            &reader,
            &signer(),
            discriminant,
            StateQueryKind::Accounts,
            page.cursor.clone(),
            size,
        )
        .await
        .expect("second page");
        assert_eq!(next.entries[0].title, "c");
        assert_eq!((next.remaining, next.cursor.is_none()), (0, true));
        first.assert();
        second.assert();
        // A listing continuation never reaches the signed peers query.
        let crossed = run_state_query(
            client,
            &reader,
            &signer(),
            discriminant,
            StateQueryKind::Peers,
            page.cursor,
            size,
        )
        .await
        .expect_err("a crossed cursor is refused before any request");
        assert!(matches!(
            crossed,
            StateQueryError::CursorMismatch {
                kind: StateQueryKind::Peers
            }
        ));
    }
    #[tokio::test(flavor = "current_thread")]
    async fn collection_refusals_name_torii_code_and_message() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        server.mock(|when, then| {
            when.method(POST).path("/v1/assets/definitions/query");
            then.status(400)
                .header("content-type", "application/json")
                .body(r#"{"code": "invalid_limit", "message": "limit must be between 1 and 500", "details": {"field": "limit"}}"#);
        });
        let client = ToriiClient::new_for_network(server.url("/"), crate::torii::test_network_id())
            .expect("client");
        let error = run_state_query(
            client,
            &reader(&server.url("/")),
            &signer(),
            iroha_data_model::account::address::chain_discriminant(),
            StateQueryKind::AssetDefinitions,
            None,
            None,
        )
        .await
        .expect_err("refused read");
        let message = error.to_string();
        assert!(
            message.contains("invalid_limit")
                && message.contains("limit must be between 1 and 500"),
            "{message}"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn peers_query_decodes_entries() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let peer = PeerId::from(ALICE_KEYPAIR.public_key().clone());
        let output = QueryOutput::new(
            QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Peer(vec![peer])),
            0,
            None,
        );
        let encoded = to_bytes(&QueryResponse::Iterable(output)).expect("encode query output");
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/query");
            then.status(200).body(encoded.clone());
        });
        let client = ToriiClient::new_for_network(server.url("/"), crate::torii::test_network_id())
            .expect("client");
        let page = run_state_query(
            client,
            &reader(&server.url("/")),
            &crate::SigningAuthority::new("fixture", ALICE_ID.clone(), ALICE_KEYPAIR.clone()),
            iroha_data_model::account::address::chain_discriminant(),
            StateQueryKind::Peers,
            None,
            None,
        )
        .await
        .expect("state page");
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].subtitle, "Peer public key");
        mock.assert();
    }
    #[tokio::test(flavor = "current_thread")]
    async fn mismatched_peer_batch_produces_error() {
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let output = QueryOutput::new(
            QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Asset(Vec::new())),
            0,
            None,
        );
        let encoded = to_bytes(&QueryResponse::Iterable(output)).expect("encode query output");
        server.mock(|when, then| {
            when.method(POST).path("/v1/query");
            then.status(200).body(encoded.clone());
        });
        let client = ToriiClient::new_for_network(server.url("/"), crate::torii::test_network_id())
            .expect("client");
        let err = run_state_query(
            client,
            &reader(&server.url("/")),
            &signer(),
            iroha_data_model::account::address::chain_discriminant(),
            StateQueryKind::Peers,
            None,
            None,
        )
        .await
        .expect_err("unexpected batch kind should error");
        assert!(matches!(
            err,
            StateQueryError::UnexpectedBatch {
                kind: StateQueryKind::Peers,
                actual: "Asset"
            }
        ));
    }
    #[tokio::test(flavor = "current_thread")]
    async fn peers_query_uses_explicit_selected_authority() {
        use iroha_data_model::query::SignedQuery;
        use iroha_test_samples::{BOB_ID, BOB_KEYPAIR};
        use iroha_version::codec::DecodeVersioned as _;
        let Some(server) = try_start_mock_server() else {
            return;
        };
        let output = QueryOutput::new(
            QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Peer(Vec::new())),
            0,
            None,
        );
        let mock = server.mock(|when, then| {
            when.method(POST).path("/v1/query").is_true(|request| {
                SignedQuery::decode_all_versioned(request.body_ref()).is_ok_and(|query| {
                    query.authority() == &*BOB_ID
                        && query.payload.network_id() == crate::torii::test_network_id()
                        && query.verify_signature().is_ok()
                })
            });
            then.status(200)
                .body(to_bytes(&QueryResponse::Iterable(output)).unwrap());
        });
        let client =
            ToriiClient::new_for_network(server.url("/"), crate::torii::test_network_id()).unwrap();
        let signer = crate::SigningAuthority::new("selected", BOB_ID.clone(), BOB_KEYPAIR.clone());
        run_state_query(
            client,
            &reader(&server.url("/")),
            &signer,
            iroha_data_model::account::address::chain_discriminant(),
            StateQueryKind::Peers,
            None,
            None,
        )
        .await
        .unwrap();
        mock.assert();
    }
    #[test]
    fn bond_batches_never_become_a_peers_page() {
        let output = QueryOutput::new(
            QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::BondRecord(Vec::new())),
            0,
            None,
        );
        let error =
            parse_peer_output(output).expect_err("a bond batch cannot become an empty peers page");
        assert!(matches!(
            error,
            StateQueryError::UnexpectedBatch {
                kind: StateQueryKind::Peers,
                actual: "BondRecord"
            }
        ));
    }

    #[test]
    fn private_asset_batches_keep_exact_labels_and_never_become_peers() {
        let batches = [
            (
                QueryOutputBatchBox::AssetUidPolicyViewV1(Vec::new()),
                "AssetUidPolicyViewV1",
            ),
            (
                QueryOutputBatchBox::AssetFiRelationshipViewV1(Vec::new()),
                "AssetFiRelationshipViewV1",
            ),
            (
                QueryOutputBatchBox::AssetUidEnrollmentViewV1(Vec::new()),
                "AssetUidEnrollmentViewV1",
            ),
            (
                QueryOutputBatchBox::AssetEscrowScopeOriginalV1(Vec::new()),
                "AssetEscrowScopeOriginalV1",
            ),
            (
                QueryOutputBatchBox::AssetFiTransferReceiptV1(Vec::new()),
                "AssetFiTransferReceiptV1",
            ),
            (
                QueryOutputBatchBox::AssetFiContextSnapshotV1(Vec::new()),
                "AssetFiContextSnapshotV1",
            ),
        ];
        for (batch, expected_label) in batches {
            let output = QueryOutput::new(QueryOutputBatchBoxTuple::from_batch(batch), 0, None);
            let error = parse_peer_output(output)
                .expect_err("a private asset batch cannot become an empty peers page");
            assert!(matches!(
                error,
                StateQueryError::UnexpectedBatch { kind: StateQueryKind::Peers, actual }
                    if actual == expected_label
            ));
        }
    }

    #[test]
    fn batch_label_handles_rwa_and_escrow_variants() {
        assert_eq!(
            batch_label(&QueryOutputBatchBox::RwaId(Vec::new())),
            "RwaId"
        );
        assert_eq!(batch_label(&QueryOutputBatchBox::Rwa(Vec::new())), "Rwa");
        assert_eq!(
            batch_label(&QueryOutputBatchBox::AssetEscrowRecord(Vec::new())),
            "AssetEscrowRecord"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::OracleFeedConfig(Vec::new())),
            "OracleFeedConfig"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::OracleFeedEventRecord(Vec::new())),
            "OracleFeedEventRecord"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::OracleProviderStatsRecord(Vec::new())),
            "OracleProviderStatsRecord"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::OracleDispute(Vec::new())),
            "OracleDispute"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::OracleChangeProposal(Vec::new())),
            "OracleChangeProposal"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::TwitterBindingRecord(Vec::new())),
            "TwitterBindingRecord"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::DefiOracleAttestation(Vec::new())),
            "DefiOracleAttestation"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::FeeSponsorProgram(Vec::new())),
            "FeeSponsorProgram"
        );
        assert_eq!(
            batch_label(&QueryOutputBatchBox::FeeSponsorProgramId(Vec::new())),
            "FeeSponsorProgramId"
        );
    }

    /// The row a live localnet validator returned for its newest block.
    const LIVE_BLOCK_ROW: &str = r##"{"created_at":"2026-10-26T04:37:58.946Z","hash":"hash:81D69FAA5B0DAD48D772869DD78C3A90C35C4EF2CA8AE7D55C623E079B525D8D#A46D","height":4,"prev_block_hash":"hash:FA9F53A24F23328824BBFF57F924BB38634B76222E2AE5C72075F4B426ADA91B#34CF","transactions_hash":"hash:FBD1ED23D5C37AC258CBFA9EB4981E1F7A353792E12006ED4146DFF6D5218ED9#50F3","transactions_rejected":1,"transactions_total":1}"##;

    #[test]
    fn block_rows_need_a_positive_height_and_a_canonical_hash() {
        let tip = BlockTip::from_row(&json::from_str(LIVE_BLOCK_ROW).unwrap()).unwrap();
        assert_eq!(tip.height, 4);
        // Shown like streamed block hashes: lowercase hex, without the literal's checksum.
        assert_eq!(
            tip.hash.to_string(),
            "81d69faa5b0dad48d772869dd78c3a90c35c4ef2ca8ae7d55c623e079b525d8d"
        );
        let literal = "hash:81D69FAA5B0DAD48D772869DD78C3A90C35C4EF2CA8AE7D55C623E079B525D8D#A46D";
        for (original, replacement) in [
            (r#""height":4"#, r#""height":0"#),
            (r#""height":4"#, r#""height":"4""#),
            (r#""hash":"#, r#""block_hash":"#),
            ("hash:81D69FAA", "hash:81d69faa"),
            ("#A46D", "#A46E"),
            (literal, &literal[5..69]),
        ] {
            let row = LIVE_BLOCK_ROW.replacen(original, replacement, 1);
            assert_ne!(row, LIVE_BLOCK_ROW);
            let error = BlockTip::from_row(&json::from_str(&row).unwrap())
                .expect_err("a malformed newest-block row is refused");
            assert!(
                matches!(error, StateQueryError::Torii(ToriiError::Decode(_))),
                "{replacement}: {error}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn newest_blocks_are_read_from_every_validator_without_waiting_on_a_silent_one() {
        let (Some(answering), Some(fresh), Some(silent)) = (
            try_start_mock_server(),
            try_start_mock_server(),
            try_start_mock_server(),
        ) else {
            return;
        };
        let newest = answering.mock(|when, then| {
            when.method(POST)
                .path("/v1/explorer/blocks/query")
                .is_true(|request| {
                    let body: json::Value =
                        json::from_slice(request.body_ref()).unwrap_or(json::Value::Null);
                    body.get("limit").and_then(json::Value::as_u64) == Some(1)
                });
            then.status(200)
                .header("content-type", "application/json")
                .body(format!(
                    r#"{{"items": [{LIVE_BLOCK_ROW}], "next_cursor": "older"}}"#
                ));
        });
        fresh.mock(|when, then| {
            when.method(POST).path("/v1/explorer/blocks/query");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"items": [], "next_cursor": null}"#);
        });
        silent.mock(|when, then| {
            when.method(POST).path("/v1/explorer/blocks/query");
            then.status(200)
                .delay(Duration::from_secs(3))
                .header("content-type", "application/json")
                .body(r#"{"items": [], "next_cursor": null}"#);
        });
        let stopped = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}/", listener.local_addr().unwrap())
        };
        let readers = [
            reader(&answering.url("/")),
            reader(&fresh.url("/")),
            reader(&silent.url("/")),
            reader(&stopped),
        ];
        let started = std::time::Instant::now();
        let tips = newest_blocks(&readers, Duration::from_millis(300)).await;
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "validators are read concurrently and a silent one is cut off"
        );
        assert!(matches!(&tips[0], Ok(Some(tip)) if tip.height == 4));
        assert!(matches!(&tips[1], Ok(None)), "no block yet");
        assert!(matches!(
            &tips[2],
            Err(StateQueryError::Torii(ToriiError::Timeout { .. }))
        ));
        assert!(matches!(&tips[3], Err(StateQueryError::Collection(_))));
        newest.assert();
    }
}
