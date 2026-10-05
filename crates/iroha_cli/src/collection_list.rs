//! `list` commands for Torii collections.
//!
//! Every collection (domains, accounts, asset definitions, NFTs, RWA lots, account assets, asset
//! holders, committed and account transactions, and repo agreements) is read with the same flags
//! and printed with the same page envelope `{items, next_cursor, total?}`. The query language and the wire
//! contract are `specs/torii/collection_queries.md`; flag values are validated by the shared
//! [`ListQuery`] implementation before any request is sent.
use std::fmt::Write as _;

use eyre::{Result, WrapErr as _};
use iroha::{
    blocking::Client as BlockingClient,
    collections::{Collection, ListQuery, ListQueryError, Page, field},
};
use norito::json::Value;

use crate::{CliOutputFormat, RunContext};

/// Filter, order, projection and paging flags shared by every collection `list` command.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct ListArgs {
    /// Rows to keep, written like a SQL `WHERE` clause.
    ///
    /// Example: `owned_by = "sorau…" and quantity >= 10.5`. Operators: `= != < <= > >=`,
    /// `in [..]`, `not in [..]`, `exists(field)`, `is null`, `is not null`, `and`, `or`, `not`;
    /// quote non-identifier path segments with backticks (``metadata.`ui-order` ``).
    #[arg(long, visible_alias = "where", value_name = "FILTER")]
    pub filter: Option<String>,
    /// Sort keys, comma-separated; prefix a key with `-` for descending order.
    ///
    /// Example: `-quantity,id`. At most 8 keys; the collection's default order applies otherwise.
    #[arg(long, value_name = "KEYS", allow_hyphen_values = true)]
    pub sort: Option<String>,
    /// Fields to return per row, comma-separated (all fields by default). Example: `id,owned_by`
    #[arg(long, value_name = "FIELDS")]
    pub select: Option<String>,
    /// Rows per page: at least 1 and at most the node's maximum (Torii's default is 100).
    #[arg(long, value_name = "N")]
    pub limit: Option<String>,
    /// Continue after a previous page by passing its `next_cursor`.
    #[arg(long, value_name = "CURSOR")]
    pub cursor: Option<String>,
    /// Follow `next_cursor` and read every page.
    ///
    /// With `-o jsonl`, rows are printed one per line as pages arrive; with `-o json`, all rows
    /// are printed as one page document whose `next_cursor` is null.
    #[arg(long)]
    pub all: bool,
    /// Also report the exact number of matching rows (`total`; costs a full scan on the node).
    #[arg(long)]
    pub include_total: bool,
}

impl ListArgs {
    /// Build and validate the list query described by the flags.
    ///
    /// # Errors
    /// Returns the [`ListQueryError`] naming the offending flag, as Torii would.
    pub fn to_query(&self) -> std::result::Result<ListQuery, ListQueryError> {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        for (name, value) in [
            ("filter", &self.filter),
            ("sort", &self.sort),
            ("select", &self.select),
            ("limit", &self.limit),
            ("cursor", &self.cursor),
        ] {
            if let Some(value) = value {
                pairs.push((name, value.clone()));
            }
        }
        if self.include_total {
            pairs.push(("include_total", "true".to_owned()));
        }
        ListQuery::from_query_pairs(pairs)
    }
}

/// An invalid list flag, rendered as an input error (exit code 4).
#[derive(Debug)]
pub(crate) struct ListInputError(pub(crate) ListQueryError);

impl std::fmt::Display for ListInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match flag_name(self.0.parameter) {
            Some(flag) => write!(f, "{} (flag `{flag}`)", self.0),
            None => write!(f, "{}", self.0),
        }
    }
}

impl std::error::Error for ListInputError {}

/// The command-line flag that sets a list-query control.
fn flag_name(parameter: &str) -> Option<&'static str> {
    Some(match parameter {
        "filter" => "--filter",
        "sort" => "--sort",
        "select" => "--select",
        "limit" => "--limit",
        "cursor" => "--cursor",
        "include_total" => "--include-total",
        _ => return None,
    })
}

/// Columns shown in text mode when `--select` is not given.
pub(crate) fn default_columns(collection: &Collection) -> &'static [&'static str] {
    match collection {
        Collection::Domains => &["id", "owned_by"],
        Collection::Accounts => &["id", "label", "uaid"],
        Collection::AssetDefinitions => &["id", "name", "alias", "owned_by"],
        Collection::Nfts => &["id", "owned_by"],
        Collection::Rwas => &["id", "owned_by", "quantity", "status"],
        Collection::AccountAssets(_) => &["asset", "asset_alias", "scope", "quantity"],
        Collection::AccountPermissions(_) => &["name", "payload"],
        Collection::AssetHolders(_) => &["account_id", "scope", "quantity"],
        Collection::Transactions | Collection::AccountTransactions(_) => {
            &["timestamp_ms", "entrypoint_hash", "result_ok", "authority"]
        }
        Collection::AccountHistory(_) => {
            &["timestamp_ms", "asset_id", "amount", "direction", "tx_hash"]
        }
        Collection::ExplorerAccounts => &["id", "owned_assets", "owned_domains", "owned_nfts"],
        Collection::ExplorerDomains => &["id", "owned_by", "accounts", "assets", "nfts"],
        Collection::ExplorerAssetDefinitions => &["id", "owned_by", "total_quantity"],
        Collection::ExplorerAssets => &["id", "account_id", "definition_id", "value"],
        Collection::ExplorerNfts => &["id", "owned_by"],
        Collection::ExplorerRwas => &["id", "owned_by", "quantity", "status"],
        Collection::ExplorerBlocks => &["height", "hash", "created_at", "transactions_total"],
        Collection::ExplorerTransactions => &["block", "hash", "authority", "status"],
        Collection::ExplorerTransactionsLatest => &["block", "hash", "authority", "status"],
        Collection::ExplorerInstructions => &["block", "transaction_hash", "index", "kind"],
        Collection::ExplorerInstructionsLatest => &["block", "transaction_hash", "index", "kind"],
        Collection::RepoAgreements => &["id", "status", "initiator", "counterparty"],
        Collection::ContractActivity => &[
            "block_height",
            "authority",
            "contract_alias",
            "contract_entrypoint",
            "result_ok",
            "entrypoint_hash",
        ],
        Collection::ContractEvents => &[
            "block_height",
            "event_kind",
            "module",
            "contract_address",
            "result_ok",
            "event_id",
        ],
        Collection::SubscriptionPlans => &["id", "provider", "billing", "pricing"],
        Collection::Subscriptions => &["id", "owned_by", "provider", "status"],
        Collection::UaidManifests(_) => {
            &["dataspace_id", "dataspace_alias", "status", "manifest_hash"]
        }
    }
}

/// Run one collection `list` command: a single page, or every page with `--all`.
pub(crate) fn run_list<C: RunContext>(
    context: &mut C,
    collection: Collection,
    args: &ListArgs,
) -> Result<()> {
    let query = args.to_query().map_err(ListInputError)?;
    let client = BlockingClient::from_client(context.client_from_config()?)
        .wrap_err("failed to initialize the collection client")?;
    let columns: Vec<String> = query.select.as_ref().map_or_else(
        || {
            default_columns(&collection)
                .iter()
                .map(|column| (*column).to_owned())
                .collect()
        },
        |fields| {
            fields
                .iter()
                .map(|field| field.as_str().to_owned())
                .collect()
        },
    );
    if !args.all {
        let page = client
            .list_page(&collection, &query)
            .wrap_err_with(|| format!("failed to list {collection}"))?;
        return print_page(context, &page, &columns);
    }
    let mut rows = client.list(collection.clone(), query);
    if context.output_format() == CliOutputFormat::Json && context.json_lines() {
        for row in rows.by_ref() {
            let row = row.wrap_err_with(|| format!("failed to list {collection}"))?;
            context.print_data(&row)?;
        }
        if let Some(total) = rows.total() {
            eprintln!("total: {total}");
        }
        return Ok(());
    }
    let mut items = Vec::new();
    for row in rows.by_ref() {
        items.push(row.wrap_err_with(|| format!("failed to list {collection}"))?);
    }
    let page = Page {
        items,
        next_cursor: None,
        total: rows.total(),
    };
    print_page(context, &page, &columns)
}

/// Print one page in the selected output format.
fn print_page<C: RunContext>(
    context: &mut C,
    page: &Page<Value>,
    columns: &[String],
) -> Result<()> {
    match context.output_format() {
        CliOutputFormat::Json if context.json_lines() => {
            for row in &page.items {
                context.print_data(row)?;
            }
            if let Some(cursor) = &page.next_cursor {
                eprintln!("more rows: pass `--cursor {cursor}` or `--all`");
            }
            if let Some(total) = page.total {
                eprintln!("total: {total}");
            }
            Ok(())
        }
        CliOutputFormat::Json => context.print_data(page),
        CliOutputFormat::Text => {
            let mut text = render_table(&page.items, columns);
            let mut footer = Vec::new();
            if let Some(total) = page.total {
                footer.push(format!("total: {total}"));
            }
            if let Some(cursor) = &page.next_cursor {
                footer.push(format!("next page: --cursor {cursor}"));
            }
            if !footer.is_empty() {
                let _ = write!(text, "\n\n{}", footer.join("\n"));
            }
            context.println_data(text)
        }
    }
}

/// Longest cell rendered in a text table before truncation.
const MAX_CELL_CHARS: usize = 64;

/// Render rows as an aligned text table of `columns`.
fn render_table(rows: &[Value], columns: &[String]) -> String {
    if rows.is_empty() {
        return "no rows".to_owned();
    }
    let header: Vec<String> = columns.iter().map(|column| column.to_uppercase()).collect();
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| columns.iter().map(|column| cell(row, column)).collect())
        .collect();
    let widths: Vec<usize> = (0..columns.len())
        .map(|index| {
            cells
                .iter()
                .map(|row| row[index].chars().count())
                .chain(std::iter::once(header[index].chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut table = String::new();
    for line in std::iter::once(&header).chain(cells.iter()) {
        let mut rendered = String::new();
        for (index, value) in line.iter().enumerate() {
            if index + 1 == line.len() {
                rendered.push_str(value);
            } else {
                let padding = widths[index].saturating_sub(value.chars().count());
                rendered.push_str(value);
                rendered.push_str(&" ".repeat(padding + 2));
            }
        }
        table.push_str(rendered.trim_end());
        table.push('\n');
    }
    table.pop();
    table
}

/// The value at a dotted field path of a row, rendered for a table cell.
fn cell(row: &Value, path: &str) -> String {
    let mut current = Some(row);
    for segment in path.split('.') {
        current = current.and_then(|value| value.get(segment.trim_matches('`')));
    }
    let rendered = match current {
        None | Some(Value::Null) => "-".to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(other) => norito::json::to_json(other).unwrap_or_else(|_| "?".to_owned()),
    };
    if rendered.chars().count() > MAX_CELL_CHARS {
        let mut truncated: String = rendered.chars().take(MAX_CELL_CHARS - 1).collect();
        truncated.push('…');
        truncated
    } else {
        rendered
    }
}

/// Read the one row of `collection` whose `id` equals `id`, or fail when it does not exist.
pub(crate) fn get_by_id<C: RunContext>(
    context: &mut C,
    collection: Collection,
    id: &str,
) -> Result<Value> {
    let client = BlockingClient::from_client(context.client_from_config()?)
        .wrap_err("failed to initialize the collection client")?;
    let query = ListQuery::new().filter(field("id").eq(id)).limit(1);
    let page = client
        .list_page(&collection, &query)
        .wrap_err_with(|| format!("failed to read {collection} `{id}`"))?;
    page.items
        .into_iter()
        .next()
        .ok_or_else(|| eyre::eyre!("{collection} `{id}` not found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> ListArgs {
        ListArgs::default()
    }

    #[test]
    fn history_collections_share_default_columns() {
        let account = iroha::data_model::account::AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![7; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        assert_eq!(
            default_columns(&Collection::Transactions),
            default_columns(&Collection::AccountTransactions(account))
        );
    }

    #[test]
    fn flags_build_the_canonical_query() {
        let query = ListArgs {
            filter: Some(r#"owned_by = "alice" and quantity >= 10.5"#.to_owned()),
            sort: Some("-quantity,id".to_owned()),
            select: Some("id,quantity".to_owned()),
            limit: Some("50".to_owned()),
            cursor: Some("q1".to_owned()),
            include_total: true,
            ..args()
        }
        .to_query()
        .expect("valid flags");
        let expected = ListQuery::new()
            .filter(field("owned_by").eq("alice") & field("quantity").gte("10.5"))
            .sort_by(iroha::collections::SortKey::desc("quantity"))
            .sort_by(iroha::collections::SortKey::asc("id"))
            .select(["id", "quantity"])
            .limit(50)
            .cursor("q1")
            .include_total();
        assert_eq!(
            query.filter.as_ref().map(ToString::to_string),
            expected.filter.as_ref().map(ToString::to_string)
        );
        assert_eq!(query.sort, expected.sort);
        assert_eq!(query.select, expected.select);
        assert_eq!(query.limit, Some(50));
        assert_eq!(query.cursor.as_deref(), Some("q1"));
        assert!(query.include_total);
        assert_eq!(args().to_query().unwrap(), ListQuery::new());
    }

    #[test]
    fn invalid_flags_name_the_flag_and_its_code() {
        for (flags, code, flag) in [
            (
                ListArgs {
                    filter: Some("owned_by && quantity".to_owned()),
                    ..args()
                },
                "invalid_filter",
                "--filter",
            ),
            (
                ListArgs {
                    sort: Some("id,id".to_owned()),
                    ..args()
                },
                "invalid_sort",
                "--sort",
            ),
            (
                ListArgs {
                    limit: Some("0".to_owned()),
                    ..args()
                },
                "invalid_limit",
                "--limit",
            ),
            (
                ListArgs {
                    cursor: Some("not a cursor".to_owned()),
                    ..args()
                },
                "invalid_cursor",
                "--cursor",
            ),
        ] {
            let error = flags.to_query().expect_err("invalid flags");
            assert_eq!(error.code(), code);
            let rendered = ListInputError(error).to_string();
            assert!(rendered.contains(flag), "{rendered}");
        }
    }

    #[test]
    fn committed_transactions_render_shared_history_columns_and_nullable_authority() {
        let account_history = Collection::AccountTransactions(iroha_test_samples::ALICE_ID.clone());
        let expected = ["timestamp_ms", "entrypoint_hash", "result_ok", "authority"];
        assert_eq!(default_columns(&Collection::Transactions), expected);
        assert_eq!(default_columns(&account_history), expected);
        let columns = default_columns(&Collection::Transactions)
            .iter()
            .map(|column| (*column).to_owned())
            .collect::<Vec<_>>();
        let rows = [norito::json!({
            "timestamp_ms": 42,
            "entrypoint_hash": "receipt",
            "result_ok": false,
            "authority": null
        })];
        let rendered = render_table(&rows, &columns);
        let lines = rendered.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            lines[0].split_whitespace().collect::<Vec<_>>(),
            ["TIMESTAMP_MS", "ENTRYPOINT_HASH", "RESULT_OK", "AUTHORITY"]
        );
        assert_eq!(
            lines[1].split_whitespace().collect::<Vec<_>>(),
            ["42", "receipt", "false", "-"]
        );
    }

    #[test]
    fn tables_align_columns_and_mark_missing_cells() {
        let rows = vec![
            norito::json!({"id": "wonderland.universal", "owned_by": "alice", "metadata": {"tier": 1}}),
            norito::json!({"id": "garden.universal", "owned_by": null}),
        ];
        let columns = vec![
            "id".to_owned(),
            "owned_by".to_owned(),
            "metadata.tier".to_owned(),
        ];
        assert_eq!(
            render_table(&rows, &columns),
            "ID                    OWNED_BY  METADATA.TIER\n\
             wonderland.universal  alice     1\n\
             garden.universal      -         -"
        );
        assert_eq!(render_table(&[], &columns), "no rows");
        let mut long = norito::json::Map::new();
        long.insert("id".into(), Value::String("x".repeat(100)));
        let long = Value::Object(long);
        let rendered = render_table(&[long], &["id".to_owned()]);
        assert!(rendered.lines().nth(1).unwrap().chars().count() == MAX_CELL_CHARS);
    }
}
