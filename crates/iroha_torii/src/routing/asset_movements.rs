//! Every committed movement of an asset definition, newest first, from a node-local index.
//!
//! `/v1/accounts/{account_id}/history` projects one account's movements by walking committed
//! blocks for every page, so following each account of an asset walks the chain once per
//! account. This index folds each committed block once instead, as blocks commit, into the rows
//! of `/v1/assets/{definition_id}/movements`: one row per movement, kept in chain order per asset
//! definition. Instruction legs, applied batch receipts and executed multisig movements use
//! exactly the account-history projection. KAGEMUSHA Loads, Unloads and fee claims move value
//! inside the executor, so they come from their immutable ledger records, and only for the
//! transaction a record names: a retry in a later transaction moved nothing. A quoted online
//! charge of a Load or an Unload is its own `OFFLINE_CHARGE` row, so `OFFLINE_LOAD` is only the
//! payer-to-reserve leg and `OFFLINE_UNLOAD` only the reserve-to-account leg. A failed
//! transaction keeps its attempted instruction legs, with `result_ok: false`.
//!
//! Like the multisig approval landings, the index changes nothing the chain executes: it ships
//! with an in-place node update and covers the existing history. A read folds a short tail of
//! new blocks itself. A longer gap, as after a restart, is left to one background pass, and
//! until it reaches the tip reads answer `503 asset_movements_indexing`, so a page is never
//! silently incomplete. At most `MAX_ROWS` rows are kept; past it the oldest blocks are dropped
//! and a read that needs them fails with `asset_movements_not_retained`.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::{
        Arc, LazyLock, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use axum::{
    extract::{ConnectInfo, Extension, Path, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse as _, Response},
};
use iroha_core::{
    kagemusha_wallet_v1::{Issuance, Registration, validate_row},
    smartcontracts::isi::tx::transaction_history_byte_limit,
    state::{State as CoreState, WorldReadOnly},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::{BlockHeader, SignedBlock},
    isi::{
        InstructionBox,
        kagemusha_wallet::{KagemushaWalletLedgerActionV1, KagemushaWalletLedgerV1},
    },
    kagemusha::{
        KagemushaWalletFeeClaimV1, KagemushaWalletLedgerKeyV1, KagemushaWalletPayoutKeyV1,
        KagemushaWalletPayoutRecordV1, KagemushaWalletUnloadChargeV1, KagemushaWalletUnloadClaimV1,
    },
    transaction::signed::TransactionEntrypoint,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_torii_shared::{ErrorDetails, ErrorEnvelope, list_query::ListQuery};
use mv::storage::StorageReadOnly as _;
use norito::json::{Map, Value};

use super::{
    AccountHistoryIndex, AccountHistoryProjection, BorrowedNetworkTransaction,
    DataspaceReadVisibility, HistoryTransaction,
    account_history_multisig::{MultisigHistoryBudget, append_executed_multisig_history},
    account_history_tx_base, append_account_history_projections_for_tx, conversion_error,
    reject_emergency_fast_unbounded_history, tx_field_value,
};
use crate::collections::{self, CollectionError, RowPage};

#[cfg(test)]
mod tests;

/// Most blocks one read folds in before answering. A longer gap is left to one background pass.
const REQUEST_TAIL_BLOCKS: usize = 64;
/// The background pass publishes its progress after this many folded blocks.
const PUBLISH_EVERY_BLOCKS: usize = 64;
/// Most rows kept. Past it the oldest blocks are dropped until a tenth of this is free again.
const MAX_ROWS: usize = 500_000;
/// Rows one page examines (visible or not) before it ends at the next block boundary.
const PAGE_EXAMINED_ROWS: usize = 20_000;
/// Work allowance for the multisig and batch originals of one block.
const BLOCK_ORIGINAL_WORK: u64 = 1 << 16;
/// How long a read waits for another fold of the short tail before it answers `503`.
const READ_FOLD_WAIT: Duration = Duration::from_secs(2);

/// Code of the retryable answer while the index is behind the committed tip.
pub(crate) const INDEXING_CODE: &str = "asset_movements_indexing";
/// Code of the answer for movements below the retained floor.
pub(crate) const NOT_RETAINED_CODE: &str = "asset_movements_not_retained";
/// Endpoint label of `/v1/assets/{definition_id}/movements`.
pub(crate) const ENDPOINT: &str = "/v1/assets/{definition_id}/movements";

/// KAGEMUSHA ledger row tags of the records this index reads (`iroha_core` ledger storage).
/// Payout rows are addressed by `KagemushaWalletPayoutKeyV1::ledger_key`.
const REGISTRATION_ROW: u8 = 1;
const ISSUANCE_ROW: u8 = 3;

/// What one row moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// An instruction, batch receipt or executed multisig transfer between two accounts.
    Transfer,
    /// Value created for an account.
    Mint,
    /// Value destroyed from an account.
    Burn,
    /// A KAGEMUSHA Load: payer to reserve.
    OfflineLoad,
    /// A paid KAGEMUSHA Unload: reserve to the unloading account.
    OfflineUnload,
    /// A paid KAGEMUSHA fee claim: reserve to the fee beneficiary.
    OfflineFee,
    /// The online charge of a KAGEMUSHA Load (payer to the charge beneficiary) or of a paid
    /// Unload (reserve to the charge beneficiary).
    OfflineCharge,
}

impl Kind {
    const fn label(self) -> &'static str {
        match self {
            Self::Transfer => "TRANSFER",
            Self::Mint => "MINT",
            Self::Burn => "BURN",
            Self::OfflineLoad => "OFFLINE_LOAD",
            Self::OfflineUnload => "OFFLINE_UNLOAD",
            Self::OfflineFee => "OFFLINE_FEE",
            Self::OfflineCharge => "OFFLINE_CHARGE",
        }
    }
}

/// One movement of one asset definition, as the fold projected it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Movement {
    definition: String,
    kind: Kind,
    from: Option<String>,
    to: Option<String>,
    amount: String,
}

/// The movements of one committed transaction, with what a read needs to show and gate them.
#[derive(Debug)]
struct FoldedTransaction {
    tx_hash: String,
    authority: String,
    timestamp_ms: u64,
    block_index: u64,
    result_ok: bool,
    /// Route legs of the external entrypoint; `None` keeps its rows to global readers.
    route: Option<BTreeSet<DataSpaceId>>,
    movements: Vec<Movement>,
}

/// One folded committed block. `transactions` is `None` for a block this node cannot project:
/// everything at or below it stops being retained, instead of reading as complete.
#[derive(Debug)]
struct FoldedBlock {
    height: usize,
    hash: HashOf<BlockHeader>,
    transactions: Option<Vec<FoldedTransaction>>,
}

/// Transaction fields shared by its rows.
#[derive(Debug)]
struct Entry {
    tx_hash: String,
    authority: Arc<str>,
    timestamp_ms: u64,
    height: u64,
    block_index: u64,
    result_ok: bool,
    route: Option<Arc<[DataSpaceId]>>,
}

/// One retained movement row.
#[derive(Clone, Debug)]
struct Row {
    entry: Arc<Entry>,
    movement_index: u64,
    kind: Kind,
    from: Option<Arc<str>>,
    to: Option<Arc<str>>,
    amount: Box<str>,
}

impl Row {
    fn coordinate(&self) -> (u64, u64, u64) {
        (
            self.entry.height,
            self.entry.block_index,
            self.movement_index,
        )
    }

    /// The collection row; `definition` is the asset definition literal it belongs to.
    fn to_json(&self, definition: &str) -> Map {
        let entry = &self.entry;
        let account = |account: Option<&Arc<str>>| {
            account.map_or(Value::Null, |account| Value::from(account.to_string()))
        };
        let mut row = Map::new();
        row.insert(
            "id".into(),
            Value::from(format!("{}:{}", entry.tx_hash, self.movement_index)),
        );
        row.insert("kind".into(), Value::from(self.kind.label().to_owned()));
        row.insert("timestamp_ms".into(), Value::from(entry.timestamp_ms));
        row.insert("from_account_id".into(), account(self.from.as_ref()));
        row.insert("to_account_id".into(), account(self.to.as_ref()));
        row.insert(
            "asset_definition_id".into(),
            Value::from(definition.to_owned()),
        );
        row.insert("amount".into(), Value::from(self.amount.to_string()));
        row.insert("tx_hash".into(), Value::from(entry.tx_hash.clone()));
        row.insert("authority".into(), Value::from(entry.authority.to_string()));
        row.insert("block_height".into(), Value::from(entry.height));
        row.insert("block_index".into(), Value::from(entry.block_index));
        row.insert("movement_index".into(), Value::from(self.movement_index));
        row.insert("result_ok".into(), Value::from(entry.result_ok));
        row
    }

    /// The same route check account history applies to the committed entrypoint.
    fn visible(&self, visibility: &DataspaceReadVisibility) -> bool {
        visibility.can_read_all()
            || self.entry.route.as_ref().is_some_and(|route| {
                route
                    .iter()
                    .all(|dataspace| visibility.allows_dataspace(*dataspace))
            })
    }
}

fn intern<T: Ord + ?Sized>(set: &mut BTreeSet<Arc<T>>, value: &T) -> Arc<T>
where
    Arc<T>: for<'a> From<&'a T>,
{
    if let Some(existing) = set.get(value) {
        return Arc::clone(existing);
    }
    let value = Arc::<T>::from(value);
    set.insert(Arc::clone(&value));
    value
}

/// Retained movements per asset definition, folded from committed blocks up to `height`.
#[derive(Debug)]
struct AssetMovementIndex {
    height: usize,
    tip: Option<HashOf<BlockHeader>>,
    /// Movements below this height are not retained; zero while nothing was dropped.
    not_retained_below: u64,
    rows: usize,
    capacity: usize,
    definitions: BTreeMap<Arc<str>, VecDeque<Row>>,
    accounts: BTreeSet<Arc<str>>,
    routes: BTreeSet<Arc<[DataSpaceId]>>,
}

/// How a page walk ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WalkEnd {
    /// Every retained row of the definition was examined.
    Exhausted,
    /// The walk reached a row below the requested height range.
    BelowRange,
    /// The page filled up before the next matching row.
    Full,
    /// The examination budget ended at a block boundary; continue below this height.
    Budget(u64),
}

impl AssetMovementIndex {
    fn new(capacity: usize) -> Self {
        Self {
            height: 0,
            tip: None,
            not_retained_below: 0,
            rows: 0,
            capacity,
            definitions: BTreeMap::new(),
            accounts: BTreeSet::new(),
            routes: BTreeSet::new(),
        }
    }

    /// Whether the committed chain still holds the block this index was folded up to.
    fn extends_committed_chain(&self, state: &CoreState) -> bool {
        self.height == 0
            || state.committed_block_hash_at_height(u64::try_from(self.height).unwrap_or(u64::MAX))
                == self.tip
    }

    /// Append one folded block's rows in chain order.
    fn append(&mut self, block: FoldedBlock) {
        let height = u64::try_from(block.height).unwrap_or(u64::MAX);
        match block.transactions {
            None => self.drop_below(height.saturating_add(1)),
            Some(transactions) => {
                for transaction in transactions {
                    self.append_transaction(height, transaction);
                }
            }
        }
        self.height = block.height;
        self.tip = Some(block.hash);
    }

    fn append_transaction(&mut self, height: u64, transaction: FoldedTransaction) {
        let route = transaction.route.map(|route| {
            let route: Vec<DataSpaceId> = route.into_iter().collect();
            intern(&mut self.routes, route.as_slice())
        });
        let entry = Arc::new(Entry {
            tx_hash: transaction.tx_hash,
            authority: intern(&mut self.accounts, transaction.authority.as_str()),
            timestamp_ms: transaction.timestamp_ms,
            height,
            block_index: transaction.block_index,
            result_ok: transaction.result_ok,
            route,
        });
        for (movement_index, movement) in transaction.movements.into_iter().enumerate() {
            let from = movement
                .from
                .map(|account| intern(&mut self.accounts, account.as_str()));
            let to = movement
                .to
                .map(|account| intern(&mut self.accounts, account.as_str()));
            let row = Row {
                entry: Arc::clone(&entry),
                movement_index: u64::try_from(movement_index).unwrap_or(u64::MAX),
                kind: movement.kind,
                from,
                to,
                amount: movement.amount.into_boxed_str(),
            };
            match self.definitions.get_mut(movement.definition.as_str()) {
                Some(rows) => rows.push_back(row),
                None => {
                    self.definitions.insert(
                        Arc::from(movement.definition.as_str()),
                        VecDeque::from([row]),
                    );
                }
            }
            self.rows += 1;
        }
    }

    /// Stop retaining every movement below `height`.
    fn drop_below(&mut self, height: u64) {
        if height <= self.not_retained_below {
            return;
        }
        for rows in self.definitions.values_mut() {
            let cut = rows.partition_point(|row| row.entry.height < height);
            rows.drain(..cut);
            self.rows -= cut;
        }
        self.definitions.retain(|_, rows| !rows.is_empty());
        self.not_retained_below = height;
        self.accounts
            .retain(|account| Arc::strong_count(account) > 1);
        self.routes.retain(|route| Arc::strong_count(route) > 1);
    }

    /// Past the capacity, drop the oldest whole blocks until a tenth of it is free; return the
    /// new floor when it moved.
    fn enforce_capacity(&mut self) -> Option<u64> {
        if self.rows <= self.capacity {
            return None;
        }
        let target = self.capacity - self.capacity / 10;
        let retained_from = |height: u64| -> usize {
            self.definitions
                .values()
                .map(|rows| rows.len() - rows.partition_point(|row| row.entry.height < height))
                .sum()
        };
        // The smallest height whose rows at or above it fit the target.
        let mut low = self.not_retained_below;
        let mut high = u64::try_from(self.height)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        while low < high {
            let middle = low + (high - low) / 2;
            if retained_from(middle) <= target {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        self.drop_below(low);
        Some(low)
    }

    /// One newest-first page of `definition`'s retained rows that `visibility` may read.
    ///
    /// Rows at or above the cursor (or above the filter's `block_height` range) are skipped by
    /// binary search, and the walk stops below the range, so `block_height > N` costs only the
    /// rows above `N`. A page that spends its examination budget ends at a public block
    /// boundary, so a continuation never names a hidden row.
    fn page(
        &self,
        prepared: &collections::Prepared<'_>,
        definition: &str,
        visibility: &DataspaceReadVisibility,
    ) -> Result<RowPage, crate::Error> {
        let (lowest, highest) = prepared.height_range();
        let Some(rows) = self.definitions.get(definition) else {
            return self.finish(prepared, Vec::new(), WalkEnd::Exhausted, None, lowest);
        };
        let mut start = rows.len();
        if highest < u64::MAX {
            let ceiling = (highest + 1, 0, 0);
            start = start.min(rows.partition_point(|row| row.coordinate() < ceiling));
        }
        if let Some(after) = prepared.resume_subrow_position() {
            start = start.min(rows.partition_point(|row| row.coordinate() < after));
        } else if let Some(below) = prepared.resume_before_block() {
            start = start.min(rows.partition_point(|row| row.entry.height < below));
        }
        let mut items = Vec::new();
        let mut examined = 0usize;
        let mut previous_height = None;
        let mut last_visible = None;
        let mut end = WalkEnd::Exhausted;
        for row in rows.range(..start).rev() {
            let coordinate = row.coordinate();
            if coordinate.0 < lowest {
                end = WalkEnd::BelowRange;
                break;
            }
            if examined >= PAGE_EXAMINED_ROWS
                && let Some(height) = previous_height
                && height != coordinate.0
            {
                end = WalkEnd::Budget(height);
                break;
            }
            examined += 1;
            previous_height = Some(coordinate.0);
            if !row.visible(visibility) {
                continue;
            }
            let json = row.to_json(definition);
            if prepared.matches(&json) {
                if items.len() == prepared.limit() {
                    end = WalkEnd::Full;
                    break;
                }
                items.push(json);
            }
            last_visible = Some(coordinate);
        }
        self.finish(prepared, items, end, last_visible, lowest)
    }

    fn finish(
        &self,
        prepared: &collections::Prepared<'_>,
        items: Vec<Map>,
        end: WalkEnd,
        last_visible: Option<(u64, u64, u64)>,
        lowest: u64,
    ) -> Result<RowPage, crate::Error> {
        let page = match end {
            WalkEnd::Exhausted if lowest < self.not_retained_below => {
                if items.is_empty() {
                    return Err(not_retained(self.not_retained_below));
                }
                // Return what was read; the next page reports the missing history.
                prepared.block_boundary_page(items, self.not_retained_below)
            }
            WalkEnd::Exhausted | WalkEnd::BelowRange => prepared.subrow_page(items, None),
            WalkEnd::Full => prepared.subrow_page(items, last_visible),
            WalkEnd::Budget(height) => prepared.block_boundary_page(items, height),
        };
        page.map_err(Into::into)
    }
}

fn not_retained(floor: u64) -> crate::Error {
    CollectionError::new(
        NOT_RETAINED_CODE,
        "filter",
        format!(
            "this node no longer retains the asset movements below block {floor}; the oldest blocks were dropped to bound the movement index"
        ),
    )
    .with_expected(floor.to_string())
    .with_hint(format!(
        "add `block_height >= {floor}` to the filter to read the retained movements"
    ))
    .into()
}

/// Where the ledger's KAGEMUSHA records are read from: the committed World, or a test table.
trait LedgerRecords {
    fn record(&self, key: &KagemushaWalletLedgerKeyV1) -> Option<&[u8]>;
}

struct WorldLedger<'world, W: ?Sized>(&'world W);

impl<W: WorldReadOnly + ?Sized> LedgerRecords for WorldLedger<'_, W> {
    fn record(&self, key: &KagemushaWalletLedgerKeyV1) -> Option<&[u8]> {
        self.0.kagemusha_wallet_ledger().get(key).map(Vec::as_slice)
    }
}

fn record_error(message: impl Into<String>) -> crate::Error {
    conversion_error(format!("KAGEMUSHA movement record: {}", message.into()))
}

/// One immutable ledger row, checked exactly as the ledger checks restored rows.
fn read_record<T>(
    ledger: &impl LedgerRecords,
    key: &KagemushaWalletLedgerKeyV1,
) -> Result<Option<T>, crate::Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let Some(bytes) = ledger.record(key) else {
        return Ok(None);
    };
    validate_row(key, bytes).map_err(|error| record_error(format!("invalid row: {error}")))?;
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map(Some)
        .map_err(|error| record_error(format!("undecodable row: {error}")))
}

fn registration(
    ledger: &impl LedgerRecords,
    scheme: [u8; 32],
    asset: [u8; 32],
) -> Result<Registration, crate::Error> {
    let key = KagemushaWalletLedgerKeyV1::from_parts(REGISTRATION_ROW, scheme, asset, [0; 32]);
    read_record(ledger, &key)?
        .ok_or_else(|| record_error("a paid movement names an unregistered scheme asset"))
}

/// Exact Numeric text of `amount` atomic units at `scale`, as the ledger moves it.
fn amount_text(amount: u128, scale: u32) -> Result<String, crate::Error> {
    Numeric::try_new(amount, scale)
        .ok()
        .and_then(|numeric| Quantity::try_from_numeric(numeric).ok())
        .map(|quantity| quantity.to_string())
        .ok_or_else(|| record_error(format!("amount {amount} at scale {scale}")))
}

/// Rows of one immutable Load issuance: payer to reserve, then its online charge from the payer
/// to the charge beneficiary as an `OFFLINE_CHARGE` row.
fn load_movements(
    issuance: &Issuance,
    registration: &Registration,
    out: &mut Vec<Movement>,
) -> Result<(), crate::Error> {
    let definition = registration.asset.asset.to_string();
    let scale = registration.asset.scale;
    let payer = issuance.payer.to_string();
    out.push(Movement {
        definition: definition.clone(),
        kind: Kind::OfflineLoad,
        from: Some(payer.clone()),
        to: Some(registration.reserve.to_string()),
        amount: amount_text(issuance.body.amount, scale)?,
    });
    if let Some(charge) = &issuance.command.charge
        && issuance.body.online_charge != 0
    {
        out.push(Movement {
            definition,
            kind: Kind::OfflineCharge,
            from: Some(payer),
            to: Some(charge.beneficiary.to_string()),
            amount: amount_text(issuance.body.online_charge, scale)?,
        });
    }
    Ok(())
}

/// A reserve payout one claim asks for: its exact-once key, the source and total its record
/// must carry, and the accounts it pays, each with the kind of its row.
struct PayoutClaim {
    scheme: [u8; 32],
    asset: [u8; 32],
    key: KagemushaWalletPayoutKeyV1,
    source: [u8; 32],
    amount: u128,
    payees: Vec<(AccountId, u128, Kind)>,
}

impl PayoutClaim {
    /// The payout of a canonical Unload claim: the account payout (`OFFLINE_UNLOAD`) and a quoted
    /// online charge (`OFFLINE_CHARGE`).
    fn unload(bytes: &[u8], scheme: &[u8; 32]) -> Result<Self, crate::Error> {
        let claim = KagemushaWalletUnloadClaimV1::decode_canonical(bytes, scheme)
            .map_err(|error| record_error(format!("Unload claim: {error}")))?;
        let paid = claim
            .payout()
            .map_err(|error| record_error(format!("Unload payout: {error}")))?;
        let mut payees = vec![(
            claim.account.clone(),
            paid.account_payout,
            Kind::OfflineUnload,
        )];
        if let KagemushaWalletUnloadChargeV1::Quoted { beneficiary, .. } = &claim.charge {
            payees.push((beneficiary.clone(), paid.online_charge, Kind::OfflineCharge));
        }
        Ok(Self {
            scheme: claim.credential.body.scheme_id,
            asset: claim.credential.body.asset_digest,
            key: KagemushaWalletPayoutKeyV1::Unload(paid.nullifier),
            source: paid.package,
            amount: paid.amount,
            payees,
        })
    }

    /// The payout of a canonical fee claim: the fee to its fixed beneficiary.
    fn fee(bytes: &[u8], scheme: &[u8; 32]) -> Result<Self, crate::Error> {
        let claim = KagemushaWalletFeeClaimV1::decode_canonical(bytes, scheme)
            .map_err(|error| record_error(format!("fee claim: {error}")))?;
        let request = claim
            .payment
            .signed_request()
            .map_err(|error| record_error(format!("fee claim request: {error}")))?;
        let digests = claim
            .payment
            .digests()
            .map_err(|error| record_error(format!("fee claim digests: {error}")))?;
        Ok(Self {
            scheme: request.body.scheme_id,
            asset: request.body.asset_digest,
            key: KagemushaWalletPayoutKeyV1::Fee(digests.credit_id),
            source: digests.payment,
            amount: request.body.fee,
            payees: vec![(claim.beneficiary, request.body.fee, Kind::OfflineFee)],
        })
    }

    /// Rows of this payout when its immutable record names `entrypoint`; a claim the record
    /// credits to an earlier transaction moved nothing here.
    fn movements(
        &self,
        entrypoint: [u8; 32],
        ledger: &impl LedgerRecords,
        credited: &mut BTreeSet<KagemushaWalletLedgerKeyV1>,
        out: &mut Vec<Movement>,
    ) -> Result<(), crate::Error> {
        let key = self.key.ledger_key(self.scheme);
        if !credited.insert(key) {
            return Ok(());
        }
        let Some(record) = read_record::<KagemushaWalletPayoutRecordV1>(ledger, &key)? else {
            return Ok(());
        };
        if record.transaction != entrypoint {
            return Ok(());
        }
        if record.key != self.key || record.source != self.source || record.amount != self.amount {
            return Err(record_error("payout record differs from its claim"));
        }
        let registration = registration(ledger, self.scheme, self.asset)?;
        let definition = registration.asset.asset.to_string();
        let reserve = registration.reserve.to_string();
        for (payee, value, kind) in &self.payees {
            if *value == 0 {
                continue;
            }
            out.push(Movement {
                definition: definition.clone(),
                kind: *kind,
                from: Some(reserve.clone()),
                to: Some(payee.to_string()),
                amount: amount_text(*value, registration.asset.scale)?,
            });
        }
        Ok(())
    }
}

/// KAGEMUSHA movements of one top-level ledger instruction of a successful transaction.
fn offline_instruction_movements(
    instruction: &KagemushaWalletLedgerV1,
    (entrypoint, height): ([u8; 32], u64),
    ledger: &impl LedgerRecords,
    credited: &mut BTreeSet<KagemushaWalletLedgerKeyV1>,
    out: &mut Vec<Movement>,
) -> Result<(), crate::Error> {
    let scheme = instruction.scheme;
    match &instruction.action {
        KagemushaWalletLedgerActionV1::IssueLoad {
            wallet, request_id, ..
        } => {
            let key =
                KagemushaWalletLedgerKeyV1::from_parts(ISSUANCE_ROW, scheme, *wallet, *request_id);
            if !credited.insert(key) {
                return Ok(());
            }
            let Some(issuance) = read_record::<Issuance>(ledger, &key)? else {
                return Ok(());
            };
            // Only the original execution issues; a later reuse of the request id fails.
            if issuance.body.transaction_hash != entrypoint || issuance.body.block_height != height
            {
                return Ok(());
            }
            let registration = registration(ledger, scheme, issuance.body.asset_digest)?;
            load_movements(&issuance, &registration, out)
        }
        KagemushaWalletLedgerActionV1::Unload(bytes) => {
            PayoutClaim::unload(bytes, &scheme)?.movements(entrypoint, ledger, credited, out)
        }
        KagemushaWalletLedgerActionV1::ClaimFee(bytes) => {
            PayoutClaim::fee(bytes, &scheme)?.movements(entrypoint, ledger, credited, out)
        }
        _ => Ok(()),
    }
}

/// KAGEMUSHA movements of a successful transaction's top-level instructions, in order.
fn offline_movements(
    transaction: &impl HistoryTransaction,
    height: u64,
    ledger: &impl LedgerRecords,
    out: &mut Vec<Movement>,
) -> Result<(), crate::Error> {
    let instructions: Box<dyn Iterator<Item = &InstructionBox> + '_> = match transaction
        .entrypoint()
    {
        TransactionEntrypoint::External(signed) => {
            Box::new(signed.instructions().explicit_instructions())
        }
        TransactionEntrypoint::SealedReveal(reveal) => Box::new(
            reveal
                .signed_transaction()
                .instructions()
                .explicit_instructions(),
        ),
        TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::AnonymousBallot(_) => {
            return Ok(());
        }
    };
    let entrypoint: [u8; 32] = *transaction.entrypoint_hash().as_ref();
    let mut credited = BTreeSet::new();
    for instruction in instructions {
        if let Some(ledger_instruction) = instruction
            .as_any()
            .downcast_ref::<KagemushaWalletLedgerV1>()
        {
            offline_instruction_movements(
                ledger_instruction,
                (entrypoint, height),
                ledger,
                &mut credited,
                out,
            )?;
        }
    }
    Ok(())
}

/// Movements of account-history legs: an outgoing leg with a counterparty is one transfer (its
/// incoming mirror is not another), an incoming leg without one is a mint, an outgoing leg
/// without one a burn, and a self leg one transfer to itself. Legs without value are skipped.
fn leg_movements(legs: &[AccountHistoryProjection]) -> impl Iterator<Item = Movement> + '_ {
    legs.iter().filter_map(|leg| {
        let definition = leg.asset_definition_id.clone()?;
        let amount = leg.amount.clone()?;
        let account = leg.account_id.clone();
        let (kind, from, to) = match (leg.direction.as_str(), leg.counterparty_account_id.as_ref())
        {
            ("outgoing", Some(counterparty)) => {
                (Kind::Transfer, Some(account), Some(counterparty.clone()))
            }
            ("self", _) => (Kind::Transfer, Some(account.clone()), Some(account)),
            ("incoming", None) => (Kind::Mint, None, Some(account)),
            ("outgoing", None) => (Kind::Burn, Some(account), None),
            _ => return None,
        };
        Some(Movement {
            definition,
            kind,
            from,
            to,
            amount,
        })
    })
}

/// Every movement of one committed transaction, in movement order: instruction legs, then
/// applied batch receipts and executed multisig movements, then KAGEMUSHA movements. A failed
/// transaction has only its attempted instruction legs. Returns the signer and the movements.
fn project_transaction(
    transaction: &BorrowedNetworkTransaction<'_>,
    height: u64,
    world: &impl WorldReadOnly,
    ledger: &impl LedgerRecords,
    network: &NetworkId,
    budget: &mut MultisigHistoryBudget,
) -> Result<Option<(String, Vec<Movement>)>, crate::Error> {
    let mut legs = AccountHistoryIndex::default();
    append_account_history_projections_for_tx(&mut legs, transaction, height);
    let result_ok = transaction.result().as_ref().is_ok();
    if result_ok {
        append_executed_multisig_history(&mut legs, transaction, height, world, network, budget)?;
    }
    let mut movements: Vec<Movement> = leg_movements(&legs.items).collect();
    if result_ok {
        offline_movements(transaction, height, ledger, &mut movements)?;
    }
    if movements.is_empty() {
        return Ok(None);
    }
    Ok(tx_field_value(transaction, "authority").map(|authority| (authority, movements)))
}

/// Why one block could not be folded.
#[derive(Debug)]
enum FoldRefusal {
    /// A busy history pool or an unpublished view: a later fold goes on from here.
    Transient(crate::Error),
    /// This node can never project the block: it stops being retained.
    Unprojectable(crate::Error),
    /// History is absent or inconsistent: the fold stops until a later read.
    Fatal(crate::Error),
}

impl FoldRefusal {
    fn of_read(error: crate::Error) -> Self {
        match crate::background_history_refusal(&error) {
            crate::BackgroundHistoryRefusal::Transient => Self::Transient(error),
            crate::BackgroundHistoryRefusal::Budget => Self::Unprojectable(error),
            crate::BackgroundHistoryRefusal::Fatal => Self::Fatal(error),
        }
    }

    fn of_projection(error: crate::Error) -> Self {
        match crate::background_history_refusal(&error) {
            crate::BackgroundHistoryRefusal::Transient => Self::Transient(error),
            crate::BackgroundHistoryRefusal::Budget | crate::BackgroundHistoryRefusal::Fatal => {
                Self::Unprojectable(error)
            }
        }
    }
}

/// Read one committed block under a fresh background history owner and project every movement
/// of its Network transactions against the committed World.
fn fold_block(
    app: &crate::SharedAppState,
    network: &NetworkId,
    height: NonZeroUsize,
    hash: HashOf<BlockHeader>,
) -> Result<FoldedBlock, FoldRefusal> {
    let original =
        crate::read_background_execution_carrier(app, height).map_err(FoldRefusal::of_read)?;
    let view = app.state.view();
    if view.height() < height.get() {
        return Err(FoldRefusal::Transient(conversion_error(
            "the committed World does not include the folded block yet".into(),
        )));
    }
    let world = view.world();
    let block: &SignedBlock = original.carrier.block();
    let block_height = u64::try_from(height.get()).unwrap_or(u64::MAX);
    let block_time_ms =
        u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX);
    let mut budget = MultisigHistoryBudget::new(
        BLOCK_ORIGINAL_WORK,
        transaction_history_byte_limit(BLOCK_ORIGINAL_WORK),
    );
    let mut transactions = Vec::new();
    let mut failure = None;
    let mut position = 0usize;
    let visited = original.owner.scope(|| {
        original
            .carrier
            .visit_network_transactions(hash, |entrypoint, result| {
                let block_index = position;
                position += 1;
                if failure.is_some() {
                    return;
                }
                let transaction = BorrowedNetworkTransaction {
                    entrypoint,
                    entrypoint_hash: entrypoint.hash(),
                    result,
                };
                match project_transaction(
                    &transaction,
                    block_height,
                    world,
                    &WorldLedger(world),
                    network,
                    &mut budget,
                ) {
                    Ok(Some((authority, movements))) => {
                        let base = account_history_tx_base(&transaction, block_height);
                        transactions.push(FoldedTransaction {
                            tx_hash: base.tx_hash,
                            authority,
                            timestamp_ms: base.timestamp_ms.unwrap_or(block_time_ms),
                            block_index: u64::try_from(block_index).unwrap_or(u64::MAX),
                            result_ok: base.result_ok,
                            route: DataspaceReadVisibility::external_entrypoint_dataspaces(
                                block,
                                block_index,
                            ),
                            movements,
                        });
                    }
                    Ok(None) => {}
                    Err(error) => failure = Some(error),
                }
            })
    });
    visited.map_err(|error| {
        FoldRefusal::of_read(crate::canonical_history::query_attempt_error(error.into()))
    })?;
    if let Some(error) = failure {
        return Err(FoldRefusal::of_projection(error));
    }
    Ok(FoldedBlock {
        height: height.get(),
        hash,
        transactions: Some(transactions),
    })
}

/// The committed height every fold and currency check measures against: the block journal,
/// capped by the published World the fold reads records from.
fn folded_tip(state: &CoreState) -> usize {
    state.committed_height().min(state.view().height())
}

/// The node's movement index and its folding owners.
struct MovementFeed {
    index: RwLock<AssetMovementIndex>,
    folding: Mutex<()>,
    backfilling: AtomicBool,
    capacity: usize,
}

/// Clears `backfilling` when the background pass ends, also by panic.
struct Backfilling(&'static MovementFeed);

impl Drop for Backfilling {
    fn drop(&mut self) {
        self.0.backfilling.store(false, Ordering::Release);
    }
}

static FEED: LazyLock<MovementFeed> = LazyLock::new(|| MovementFeed::new(MAX_ROWS));

impl MovementFeed {
    fn new(capacity: usize) -> Self {
        Self {
            index: RwLock::new(AssetMovementIndex::new(capacity)),
            folding: Mutex::new(()),
            backfilling: AtomicBool::new(false),
            capacity,
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, AssetMovementIndex> {
        self.index
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, AssetMovementIndex> {
        self.index
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The folded height and whether the committed chain still holds its tip.
    fn position(&self, state: &CoreState) -> (usize, bool) {
        let index = self.read();
        (index.height, index.extends_committed_chain(state))
    }

    fn publish(&self, blocks: Vec<FoldedBlock>) {
        if blocks.is_empty() {
            return;
        }
        let mut index = self.write();
        for block in blocks {
            index.append(block);
        }
        if let Some(floor) = index.enforce_capacity() {
            iroha_logger::warn!(
                floor,
                rows = index.rows,
                "asset movement index dropped its oldest blocks to stay within its row bound; reads below the floor answer asset_movements_not_retained"
            );
        }
    }

    /// Wait briefly for the fold owner; `None` when another fold keeps it.
    fn try_folding(&self, wait: Duration) -> Option<MutexGuard<'_, ()>> {
        let deadline = std::time::Instant::now() + wait;
        loop {
            match self.folding.try_lock() {
                Ok(guard) => return Some(guard),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    return Some(poisoned.into_inner());
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    if std::time::Instant::now() >= deadline {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }

    /// Fold committed blocks into the index from its folded height. A read folds at most
    /// `REQUEST_TAIL_BLOCKS` blocks and gives up while the history pool is busy or the chain no
    /// longer holds the folded tip. The background pass folds to the tip (from genesis again on
    /// a replaced chain), publishes its progress as it goes and retries a busy history pool. A
    /// block this node can never project stops everything at or below it being retained; any
    /// other refusal stops the pass until a later read.
    fn fold_to_tip(&self, app: &crate::SharedAppState, background: bool) {
        let _folding = if background {
            match self.folding.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            }
        } else {
            match self.try_folding(READ_FOLD_WAIT) {
                Some(guard) => guard,
                None => return,
            }
        };
        if app.state.emergency_fast_startup_enabled() {
            return;
        }
        let tip = folded_tip(&app.state);
        let (mut height, current) = self.position(&app.state);
        if !current {
            if !background {
                return;
            }
            iroha_logger::warn!(
                height,
                "asset movement index folds again from genesis: the committed chain no longer holds its folded tip"
            );
            *self.write() = AssetMovementIndex::new(self.capacity);
            height = 0;
        }
        let stop = if background {
            tip
        } else {
            tip.min(height.saturating_add(REQUEST_TAIL_BLOCKS))
        };
        let network = *app.state.network_id_ref();
        let mut pending = Vec::new();
        let mut busy_retries = 0_u32;
        while height < stop {
            let Some(next) = NonZeroUsize::new(height + 1) else {
                break;
            };
            let Some(hash) = app
                .state
                .committed_block_hash_at_height(u64::try_from(next.get()).unwrap_or(u64::MAX))
            else {
                break;
            };
            match fold_block(app, &network, next, hash) {
                Ok(block) => {
                    busy_retries = 0;
                    pending.push(block);
                    height = next.get();
                }
                Err(FoldRefusal::Transient(_)) if background && busy_retries < 50 => {
                    busy_retries += 1;
                    std::thread::sleep(Duration::from_millis(200));
                }
                Err(FoldRefusal::Transient(error)) => {
                    iroha_logger::debug!(
                        ?error,
                        height = next.get(),
                        "asset movement index pauses at a block the history pool cannot read yet"
                    );
                    break;
                }
                Err(FoldRefusal::Unprojectable(error)) => {
                    iroha_logger::error!(
                        ?error,
                        height = next.get(),
                        "asset movement index cannot project a committed block; movements at or below it are no longer retained"
                    );
                    pending.push(FoldedBlock {
                        height: next.get(),
                        hash,
                        transactions: None,
                    });
                    height = next.get();
                }
                Err(FoldRefusal::Fatal(error)) => {
                    iroha_logger::warn!(
                        ?error,
                        height = next.get(),
                        "asset movement index stops at a block this node could not read"
                    );
                    break;
                }
            }
            if pending.len() >= PUBLISH_EVERY_BLOCKS {
                self.publish(std::mem::take(&mut pending));
            }
        }
        self.publish(pending);
    }

    /// Start one background pass unless one runs.
    fn start_background(&'static self, app: &crate::SharedAppState) {
        if self.backfilling.swap(true, Ordering::AcqRel) {
            return;
        }
        let app = app.clone();
        let feed = Backfilling(self);
        let spawned = std::thread::Builder::new()
            .name("asset-movement-index".to_owned())
            .spawn(move || {
                let feed = feed;
                feed.0.fold_to_tip(&app, true);
            });
        if let Err(error) = spawned {
            iroha_logger::warn!(
                ?error,
                "asset movement index could not start its background pass"
            );
        }
    }

    /// Fold the short tail for a read, or start the background pass and answer `503` while the
    /// index is further behind. Emergency Fast mode reads no history.
    async fn ensure_current(&'static self, app: &crate::SharedAppState) -> Result<(), Response> {
        if let Err(error) = reject_emergency_fast_unbounded_history(
            app.state.as_ref(),
            false,
            "the asset movement index",
        ) {
            return Err(error.into_response());
        }
        let tip = folded_tip(&app.state);
        let (height, current) = self.position(&app.state);
        if current && height >= tip {
            return Ok(());
        }
        if current && tip - height <= REQUEST_TAIL_BLOCKS {
            let folder = app.clone();
            let _ = tokio::task::spawn_blocking(move || self.fold_to_tip(&folder, false)).await;
            let (height, current) = self.position(&app.state);
            if current && height >= tip {
                return Ok(());
            }
        }
        self.start_background(app);
        let (height, _) = self.position(&app.state);
        Err(indexing_response(height, tip))
    }

    /// One page of `definition`'s movements, after the read's `ensure_current`.
    fn page(
        &self,
        state: &CoreState,
        prepared: &collections::Prepared<'_>,
        definition: &AssetDefinitionId,
        visibility: &DataspaceReadVisibility,
    ) -> Result<RowPage, crate::Error> {
        let index = self.read();
        let tip = folded_tip(state);
        if !index.extends_committed_chain(state)
            || index.height.saturating_add(REQUEST_TAIL_BLOCKS) < tip
        {
            return Err(crate::Error::AppServiceUnavailable {
                code: INDEXING_CODE,
                message: format!(
                    "this node has indexed asset movements through block {} of {tip}",
                    index.height
                ),
            });
        }
        index.page(prepared, &definition.to_string(), visibility)
    }
}

/// `503 asset_movements_indexing`: `details.actual` is the indexed height, `details.expected`
/// the committed tip, and `Retry-After` grows with the gap.
fn indexing_response(indexed: usize, tip: usize) -> Response {
    let gap = tip.saturating_sub(indexed);
    let retry_after = u64::try_from(gap.div_ceil(100))
        .unwrap_or(u64::MAX)
        .clamp(1, 30);
    let envelope = ErrorEnvelope::new(
        INDEXING_CODE,
        format!(
            "this node has indexed asset movements through block {indexed} of {tip}; retry in {retry_after} s"
        ),
    )
    .with_details(ErrorDetails {
        field: Some("block_height".to_owned()),
        actual: Some(indexed.to_string()),
        expected: Some(tip.to_string()),
        retry_after_seconds: Some(retry_after),
        hint: Some(
            "the node is folding committed blocks into its asset movement index; retry after Retry-After seconds".to_owned(),
        ),
        ..ErrorDetails::default()
    });
    let mut response = crate::utils::respond_with_status_and_format(
        StatusCode::SERVICE_UNAVAILABLE,
        envelope,
        crate::utils::ResponseFormat::Json,
    );
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-iroha-reject-code"),
        HeaderValue::from_static(INDEXING_CODE),
    );
    if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
        headers.insert(header::RETRY_AFTER, value);
    }
    response
}

/// The collection page behind `CollectionTarget::AssetMovements`.
pub(super) fn collection_page(
    state: &CoreState,
    prepared: &collections::Prepared<'_>,
    definition: &AssetDefinitionId,
    visibility: &DataspaceReadVisibility,
) -> Result<RowPage, crate::Error> {
    FEED.page(state, prepared, definition, visibility)
}

/// Shared ingress of both routes: rate limit like account history, bring the index up to the
/// committed tip, then run the collection on this node under the caller's visibility.
async fn read(
    app: &crate::SharedAppState,
    headers: &HeaderMap,
    remote: std::net::SocketAddr,
    caller: &crate::ToriiAccountReadVisibility,
    definition: String,
    query: ListQuery,
) -> Result<Response, crate::Error> {
    let limits = super::app_query_limits();
    if !crate::limits::is_allowed_by_cidr(
        headers,
        Some(remote.ip()),
        &app.api_rate_limit_bypass_nets,
    ) {
        let enforce =
            app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
        let cost = limits.rate_limit_cost(crate::collection_page_limit(&query));
        crate::check_access_enforced_with_cost(
            app,
            headers,
            Some(remote.ip()),
            &definition,
            enforce,
            cost,
        )
        .await?;
    }
    if let Err(response) = FEED.ensure_current(app).await {
        return Ok(response);
    }
    let visibility = caller.dataspace_visibility(app.as_ref());
    crate::execute_scoped_direct_collection_read(
        app,
        super::collection_sources::CollectionTarget::AssetMovements(definition),
        query,
        visibility,
    )
    .await
}

/// `GET /v1/assets/{definition_id}/movements`
pub(crate) async fn handler_get(
    State(app): State<crate::SharedAppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    Path(definition): Path<String>,
) -> Result<Response, crate::Error> {
    let caller =
        crate::torii_visibility_account_from_headers(&app, &headers, &method, &uri, &[], ENDPOINT)
            .await?;
    let query = crate::list_query_from_query_string(uri.query())?;
    read(&app, &headers, remote, &caller, definition, query).await
}

/// `POST /v1/assets/{definition_id}/movements/query`
pub(crate) async fn handler_query(
    State(app): State<crate::SharedAppState>,
    Extension(caller): Extension<crate::ToriiAccountReadVisibility>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    Path(definition): Path<String>,
    crate::JsonOnly(body): crate::JsonOnly<norito::json::Value>,
) -> Result<Response, crate::Error> {
    let query = crate::list_query_from_json(body)?;
    read(&app, &headers, remote, &caller, definition, query).await
}
