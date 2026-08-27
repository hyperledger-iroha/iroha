//! Dashboard data aggregation for the Mochi desktop shell.
use crate::{
    SigningAuthority,
    torii::{
        ExplorerAssetsQuery, ExplorerBlockRecord, ExplorerBlocksQuery, ToriiClient, ToriiErrorInfo,
    },
};
use futures::{StreamExt, TryStreamExt, stream};
const DASHBOARD_BLOCK_LIMIT: u64 = 6;
const DASHBOARD_ASSET_LIMIT: u32 = 4;
const DASHBOARD_ASSET_FETCH_CONCURRENCY: usize = 8;
/// An individual balance displayed under a dev account card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardAssetBalance {
    /// Asset definition identifier backing this balance.
    pub definition_id: String,
    /// Raw string value returned by Explorer.
    pub value: String,
}
/// A single account card shown on the dashboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardAccountCard {
    /// Friendly signer label.
    pub label: String,
    /// Canonical account identifier.
    pub account_id: String,
    /// Recent balances for this account.
    pub balances: Vec<DashboardAssetBalance>,
}
/// Lightweight block summary for the dashboard activity rail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardRecentBlock {
    /// Block height.
    pub height: u64,
    /// RFC3339 creation timestamp, when Explorer retained it.
    pub created_at: Option<String>,
    /// Included transaction count.
    pub transactions_total: u64,
    /// Rejected transaction count.
    pub transactions_rejected: u64,
}
/// A complete dashboard snapshot fetched from a target peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardSnapshot {
    /// Alias of the peer used to query the snapshot.
    pub peer_alias: String,
    /// API base URL used for explorer/status actions.
    pub api_base: String,
    /// Account-first cards for the current dev signers.
    pub accounts: Vec<DashboardAccountCard>,
    /// Recent blocks from the explorer API.
    pub recent_blocks: Vec<DashboardRecentBlock>,
}
/// Fetch a dashboard snapshot from a target peer.
pub async fn fetch_dashboard_snapshot(
    peer_alias: impl Into<String>,
    client: &ToriiClient,
    signers: &[SigningAuthority],
) -> Result<DashboardSnapshot, ToriiErrorInfo> {
    let peer_alias = peer_alias.into();
    let blocks = client
        .fetch_blocks_page(ExplorerBlocksQuery {
            page: Some(1),
            per_page: Some(DASHBOARD_BLOCK_LIMIT),
        })
        .await
        .map_err(|err| err.summarize())?;
    let cards = stream::iter(signers.iter().map(|signer| {
        let label = signer.label().to_owned();
        let account_id = signer.account_id().to_string();
        async move { fetch_account_card(client, label, account_id).await }
    }))
    .buffered(DASHBOARD_ASSET_FETCH_CONCURRENCY)
    .try_collect::<Vec<_>>()
    .await?;
    Ok(DashboardSnapshot {
        peer_alias,
        api_base: client.base_url().to_owned(),
        accounts: cards,
        recent_blocks: blocks.items.into_iter().map(map_recent_block).collect(),
    })
}
async fn fetch_account_card(
    client: &ToriiClient,
    label: String,
    account_id: String,
) -> Result<DashboardAccountCard, ToriiErrorInfo> {
    let assets = client
        .fetch_explorer_assets_page(ExplorerAssetsQuery {
            cursor: None,
            limit: Some(DASHBOARD_ASSET_LIMIT),
            owned_by: Some(account_id.clone()),
            definition: None,
        })
        .await
        .map_err(|err| err.summarize())?
        .items;
    Ok(DashboardAccountCard {
        label,
        account_id,
        balances: assets
            .into_iter()
            .map(|asset| DashboardAssetBalance {
                definition_id: asset.definition_id,
                value: asset.value,
            })
            .collect(),
    })
}
fn map_recent_block(block: ExplorerBlockRecord) -> DashboardRecentBlock {
    DashboardRecentBlock {
        height: block.height,
        created_at: block.created_at,
        transactions_total: block.transactions_total,
        transactions_rejected: block.transactions_rejected,
    }
}
#[cfg(test)]
mod tests {
    use super::fetch_dashboard_snapshot;
    use crate::{SigningAuthority, torii::ToriiClient};
    use httpmock::prelude::*;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
    use norito::json;
    use std::time::Duration;
    const EXPLORER_DEFINITION: &str = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    fn signer() -> SigningAuthority {
        SigningAuthority::new("Alice", ALICE_ID.clone(), ALICE_KEYPAIR.clone())
    }
    fn bob_signer() -> SigningAuthority {
        SigningAuthority::new("Bob", BOB_ID.clone(), BOB_KEYPAIR.clone())
    }
    #[tokio::test]
    async fn fetch_dashboard_snapshot_aggregates_signers_assets_and_blocks() {
        let server = MockServer::start();
        let alice_id = ALICE_ID.to_string();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/blocks")
                .query_param("page", "1")
                .query_param("per_page", "6");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
  "pagination":{"page":1,"per_page":6,"total_pages":1,"total_items":1},
  "items":[{"hash":"abababababababababababababababababababababababababababababababab","height":42,"created_at":"2026-03-26T10:00:00Z","prev_block_hash":null,"transactions_hash":null,"transactions_rejected":1,"transactions_total":3}]
}"#,
                );
        });
        let asset_id = format!("{EXPLORER_DEFINITION}#{alice_id}");
        let owned_by = alice_id.clone();
        server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/explorer/assets")
                .query_param("limit", "4")
                .query_param("owned_by", &owned_by);
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json::to_string(&json!({
                        "pagination": {
                            "limit": 4,
                            "next_cursor": null,
                            "has_more": false
                        },
                        "items": [{
                            "id": asset_id,
                            "definition_id": EXPLORER_DEFINITION,
                            "account_id": alice_id,
                            "value": "25"
                        }]
                    }))
                    .expect("serialize assets body"),
                );
        });
        let client = ToriiClient::new(server.url("/")).expect("client");
        let snapshot = fetch_dashboard_snapshot("peer0", &client, &[signer()])
            .await
            .expect("snapshot");
        assert_eq!(snapshot.peer_alias, "peer0");
        assert_eq!(snapshot.accounts.len(), 1);
        assert_eq!(snapshot.accounts[0].label, "Alice");
        assert_eq!(
            snapshot.accounts[0].balances[0].definition_id,
            EXPLORER_DEFINITION
        );
        assert_eq!(snapshot.recent_blocks[0].height, 42);
    }

    #[tokio::test]
    async fn fetch_dashboard_snapshot_preserves_signer_order_across_buffered_fetches() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/blocks")
                .query_param("page", "1")
                .query_param("per_page", "6");
            then.status(200).body(
                r#"{
  "pagination":{"page":1,"per_page":6,"total_pages":0,"total_items":0},
  "items":[]
}"#,
            );
        });
        let alice_id = ALICE_ID.to_string();
        let alice_asset_id = format!("{EXPLORER_DEFINITION}#{alice_id}");
        let alice_response_id = alice_id.clone();
        server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/explorer/assets")
                .query_param("limit", "4")
                .query_param("owned_by", &alice_id);
            then.status(200).delay(Duration::from_millis(50)).body(
                json::to_string(&json!({
                    "pagination": {
                        "limit": 4,
                        "next_cursor": null,
                        "has_more": false
                    },
                    "items": [{
                        "id": alice_asset_id,
                        "definition_id": EXPLORER_DEFINITION,
                        "account_id": alice_response_id,
                        "value": "1"
                    }]
                }))
                .expect("serialize Alice assets"),
            );
        });
        let bob_id = BOB_ID.to_string();
        let bob_asset_id = format!("{EXPLORER_DEFINITION}#{bob_id}");
        let bob_response_id = bob_id.clone();
        server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/explorer/assets")
                .query_param("limit", "4")
                .query_param("owned_by", &bob_id);
            then.status(200).body(
                json::to_string(&json!({
                    "pagination": {
                        "limit": 4,
                        "next_cursor": null,
                        "has_more": false
                    },
                    "items": [{
                        "id": bob_asset_id,
                        "definition_id": EXPLORER_DEFINITION,
                        "account_id": bob_response_id,
                        "value": "2"
                    }]
                }))
                .expect("serialize Bob assets"),
            );
        });
        let client = ToriiClient::new(server.url("/")).expect("client");
        let snapshot = fetch_dashboard_snapshot("peer0", &client, &[signer(), bob_signer()])
            .await
            .expect("snapshot");
        let labels = snapshot
            .accounts
            .iter()
            .map(|card| card.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, ["Alice", "Bob"]);
        assert_eq!(snapshot.accounts[0].balances[0].value, "1");
        assert_eq!(snapshot.accounts[1].balances[0].value, "2");
    }
    #[tokio::test]
    async fn fetch_dashboard_snapshot_rejects_assets_for_another_account() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/blocks")
                .query_param("page", "1")
                .query_param("per_page", "6");
            then.status(200).body(
                r#"{
  "pagination":{"page":1,"per_page":6,"total_pages":0,"total_items":0},
  "items":[]
}"#,
            );
        });
        let alice_id = ALICE_ID.to_string();
        let bob_id = BOB_ID.to_string();
        let bob_asset_id = format!("{EXPLORER_DEFINITION}#{bob_id}");
        server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/explorer/assets")
                .query_param("limit", "4")
                .query_param("owned_by", &alice_id);
            then.status(200).body(
                json::to_string(&json!({
                    "pagination": {
                        "limit": 4,
                        "next_cursor": null,
                        "has_more": false
                    },
                    "items": [{
                        "id": bob_asset_id,
                        "definition_id": EXPLORER_DEFINITION,
                        "account_id": bob_id,
                        "value": "2"
                    }]
                }))
                .expect("serialize mismatched asset response"),
            );
        });
        let client = ToriiClient::new(server.url("/")).expect("client");
        let error = fetch_dashboard_snapshot("peer0", &client, &[signer()])
            .await
            .expect_err("a filtered response must not cross account cards");
        assert_eq!(error.kind, crate::torii::ToriiErrorKind::Decode);
        assert_eq!(error.message, "Failed to decode Norito payload from Torii");
        assert!(
            error
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("owned_by filter"))
        );
    }
}
