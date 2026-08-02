//! Dashboard data aggregation for the Mochi desktop shell.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    SigningAuthority,
    torii::{
        ExplorerAccountRecord, ExplorerAccountsQuery, ExplorerAssetRecord, ExplorerAssetsQuery,
        ExplorerBlockRecord, ExplorerBlocksQuery, ToriiClient, ToriiError, ToriiErrorInfo,
    },
};

const DASHBOARD_BLOCK_LIMIT: u32 = 6;
const DASHBOARD_EXPLORER_PAGE_SIZE: u32 = 100;

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
    /// Optional I105 address returned by Explorer.
    pub i105_address: Option<String>,
    /// Number of owned assets reported by Explorer, if available.
    pub owned_assets: Option<u64>,
    /// Number of owned domains reported by Explorer, if available.
    pub owned_domains: Option<u64>,
    /// Number of owned NFTs reported by Explorer, if available.
    pub owned_nfts: Option<u64>,
    /// Recent balances for this account.
    pub balances: Vec<DashboardAssetBalance>,
}

/// Lightweight block summary for the dashboard activity rail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardRecentBlock {
    /// Block height.
    pub height: u64,
    /// RFC3339 creation timestamp.
    pub created_at: String,
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
            offset_height: None,
            limit: Some(DASHBOARD_BLOCK_LIMIT),
        })
        .await
        .map_err(|err| err.summarize())?;
    let signer_ids = signers
        .iter()
        .map(|signer| signer.account_id().to_string())
        .collect::<BTreeSet<_>>();
    let mut accounts_by_id = BTreeMap::new();
    let mut account_cursor = None;
    let mut seen_account_cursors = BTreeSet::new();
    loop {
        let accounts = client
            .fetch_explorer_accounts_page(ExplorerAccountsQuery {
                cursor: account_cursor,
                limit: Some(DASHBOARD_EXPLORER_PAGE_SIZE),
                domain: None,
                with_asset: None,
            })
            .await
            .map_err(|err| err.summarize())?;
        for record in accounts.items {
            if signer_ids.contains(&record.id) {
                accounts_by_id.insert(record.id.clone(), record);
            }
        }
        if accounts_by_id.len() == signer_ids.len() || !accounts.pagination.has_more {
            break;
        }
        let Some(next) = accounts.pagination.next_cursor else {
            return Err(ToriiError::Decode(
                "explorer accounts response omitted its continuation cursor".to_owned(),
            )
            .summarize());
        };
        if !seen_account_cursors.insert(next.clone()) {
            return Err(ToriiError::Decode(
                "explorer accounts response repeated a cursor".to_owned(),
            )
            .summarize());
        }
        account_cursor = Some(next);
    }

    let mut cards = Vec::with_capacity(signers.len());
    for signer in signers {
        let account_id = signer.account_id().to_string();
        let mut balances = Vec::new();
        let mut asset_cursor = None;
        let mut seen_asset_cursors = BTreeSet::new();
        loop {
            let page = client
                .fetch_explorer_assets_page(ExplorerAssetsQuery {
                    cursor: asset_cursor,
                    limit: Some(DASHBOARD_EXPLORER_PAGE_SIZE),
                    owned_by: Some(account_id.clone()),
                    definition: None,
                })
                .await
                .map_err(|err| err.summarize())?;
            balances.extend(page.items);
            if !page.pagination.has_more {
                break;
            }
            let Some(next) = page.pagination.next_cursor else {
                return Err(ToriiError::Decode(
                    "explorer assets response omitted its continuation cursor".to_owned(),
                )
                .summarize());
            };
            if !seen_asset_cursors.insert(next.clone()) {
                return Err(ToriiError::Decode(
                    "explorer assets response repeated a cursor".to_owned(),
                )
                .summarize());
            }
            asset_cursor = Some(next);
        }
        cards.push(map_account_card(
            signer.label(),
            &account_id,
            accounts_by_id.get(&account_id),
            balances,
        ));
    }

    Ok(DashboardSnapshot {
        peer_alias,
        api_base: client.base_url().to_owned(),
        accounts: cards,
        recent_blocks: blocks.items.into_iter().map(map_recent_block).collect(),
    })
}

fn map_account_card(
    label: &str,
    account_id: &str,
    explorer: Option<&ExplorerAccountRecord>,
    assets: Vec<ExplorerAssetRecord>,
) -> DashboardAccountCard {
    DashboardAccountCard {
        label: label.to_owned(),
        account_id: account_id.to_owned(),
        i105_address: explorer.map(|record| record.i105_address.clone()),
        owned_assets: explorer.map(|record| record.owned_assets),
        owned_domains: explorer.map(|record| record.owned_domains),
        owned_nfts: explorer.map(|record| record.owned_nfts),
        balances: assets
            .into_iter()
            .map(|asset| DashboardAssetBalance {
                definition_id: asset.definition_id,
                value: asset.value,
            })
            .collect(),
    }
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
    use httpmock::prelude::*;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
    use norito::json;

    use super::fetch_dashboard_snapshot;
    use crate::{SigningAuthority, torii::ToriiClient};

    fn signer() -> SigningAuthority {
        SigningAuthority::new("Alice", ALICE_ID.clone(), ALICE_KEYPAIR.clone())
    }

    #[tokio::test]
    async fn fetch_dashboard_snapshot_aggregates_signers_assets_and_blocks() {
        let server = MockServer::start();
        let alice_id = ALICE_ID.to_string();
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/blocks")
                .query_param("limit", "6");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    r#"{
  "pagination":{"page":1,"per_page":6,"total_pages":1,"total_items":1},
  "items":[{"hash":"aa11","height":42,"created_at":"2026-03-26T10:00:00Z","transactions_rejected":1,"transactions_total":3}]
}"#,
                );
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/v1/explorer/accounts")
                .query_param("limit", "100");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json::to_string(&json!({
                        "pagination": {
                            "limit": 100,
                            "next_cursor": null,
                            "has_more": false
                        },
                        "items": [{
                            "id": alice_id,
                            "i105_address": "i105:alice",
                            "network_prefix": 0,
                            "owned_domains": 1,
                            "owned_assets": 2,
                            "owned_nfts": 0,
                            "metadata": {}
                        }]
                    }))
                    .expect("serialize accounts body"),
                );
        });
        let owned_by = alice_id.clone();
        server.mock(move |when, then| {
            when.method(GET)
                .path("/v1/explorer/assets")
                .query_param("limit", "100")
                .query_param("owned_by", &owned_by);
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json::to_string(&json!({
                        "pagination": {
                            "limit": 100,
                            "next_cursor": null,
                            "has_more": false
                        },
                        "items": [{
                            "id": "rose##1",
                            "definition_id": "rose#wonderland",
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
        assert_eq!(snapshot.accounts[0].owned_assets, Some(2));
        assert_eq!(
            snapshot.accounts[0].balances[0].definition_id,
            "rose#wonderland"
        );
        assert_eq!(snapshot.recent_blocks[0].height, 42);
    }
}
