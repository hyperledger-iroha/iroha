//! TRON evidence builders (spec §4.13.3, §4.13.5, §7.2, §7.3).
//!
//! Self-authenticating header segments (one per maintenance boundary, then the newest blocks),
//! bootstraps and transaction proofs, built from the java-tron HTTP API ([`TronBuilder`]).
//! Headers are re-encoded from the printed `raw_data` fields and checked against the reported
//! block id; transactions are re-encoded from `raw_data_hex`, their signatures and their `ret`
//! objects and checked against `txTrieRoot`, so a lying or lossy endpoint is caught before
//! submission. Everything built is untrusted until `iroha_sccp` verifies it.
//!
//! **Evidence.** The event block `B` is proven from the first anchor that works:
//!
//! 1. a signed segment from `B` in which it is solid, when the light client stores the witness
//!    set of `B`'s maintenance period and that set is still fresh
//!    [`super::FRESHNESS_MARGIN_MS`] from now: until `ws_bound_ms` (7 d) after the period ends;
//! 2. the unsigned `raw_data` headers `B ..= C` up to the nearest retained checkpoint `C ≥ B`
//!    (`C − B ≤ 1 199`);
//! 3. otherwise `Backfill` segments of `max_backfill_headers` (256) `raw_data` headers from `C`
//!    down to within 1 199 blocks of `B`, then the headers up to the last backfilled checkpoint.

use iroha_data_model::{
    bridge::SccpNetworkV1,
    sccp::{
        inbound::SccpSourceProofBytesV1,
        light_client::{SccpLcAdvanceBytesV1, SccpLcBootstrapV1, SccpLcConsensusSetV1},
    },
};
use iroha_sccp::light_client::{
    profile::{
        SccpChainProfilesV1, TRON_MAX_ANCESTRY_HEADERS, TRON_MAX_SEGMENT_HEADERS,
        TronChainProfileV1,
    },
    proof::{SccpLcAdvanceV1, SccpLcBootstrapDataV1, SccpLcSegmentV1, SccpSourceProofV1},
    tron::{
        TronLcAdvanceV1, TronLcBootstrapV1, TronProofAnchorV1, TronRawSegmentV1, TronSegmentV1,
        TronSignedHeaderV1, TronSourceProofV1, TronTransactionProofV1, TronWitnessSetV1,
        TronWitnessV1, header_signer, header_summary, merkle_root_and_branch,
    },
};
use norito::json::Value;
use sha2::{Digest as _, Sha256};

use super::{
    AdvanceBudgetV1, BuildError, SourceChainBuilder, SourceEventRefV1, SourceEvidenceV1,
    TairaLightClientView, backfill_bytes, fit_advance, fresh_with_margin, plan_backfill,
};
use crate::{
    TronClient,
    tron::{MAX_BLOCKS_PER_RANGE, TronBlock, TronBlockHeader, TronTransaction},
};

/// Headers after a block that a segment carries so 19 distinct witnesses build on it.
pub const SOLIDITY_TAIL: u64 = 45;

/// Block-range windows [`first_block_at_or_after`] reads before giving up (about 25 000 missed
/// slots).
const MAX_SEARCH_WINDOWS: usize = 256;

/// The number of the first block timestamped at or after `start_ms` up to the head
/// `(number, timestamp)`, reading `(number, timestamp)` pairs of `low..=high` from `window`.
///
/// Blocks occupy distinct slots of `interval_ms`, so the head is at most one block per slot
/// after that block, and `head - (head_ms - start_ms) / interval_ms` is a lower bound for it:
/// missed slots only move it later. The search reads forward from the bound, one block early so
/// the predecessor proves the boundary, and steps back if the endpoint breaks the bound.
fn first_block_at_or_after(
    start_ms: u64,
    interval_ms: u64,
    head: (u64, u64),
    mut window: impl FnMut(u64, u64) -> Result<Vec<(u64, u64)>, BuildError>,
) -> Result<Option<u64>, BuildError> {
    let (head_number, head_ms) = head;
    if head_ms < start_ms || interval_ms == 0 {
        return Ok(None);
    }
    let span = MAX_BLOCKS_PER_RANGE - 1;
    let behind = (head_ms - start_ms) / interval_ms;
    let mut low = head_number.saturating_sub(behind).saturating_sub(1).max(1);
    for _ in 0..MAX_SEARCH_WINDOWS {
        let high = low.saturating_add(span).min(head_number);
        let blocks = window(low, high)?;
        let Some(&(_, first_ms)) = blocks.first() else {
            return Ok(None);
        };
        if first_ms >= start_ms {
            if low == 1 {
                return Ok(Some(1));
            }
            low = low.saturating_sub(span).max(1);
            continue;
        }
        if let Some(&(number, _)) = blocks.iter().find(|(_, time)| *time >= start_ms) {
            return Ok(Some(number));
        }
        if high >= head_number {
            return Ok(None);
        }
        low = high;
    }
    Ok(None)
}

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push(value.to_le_bytes()[0] | 0x80);
        value >>= 7;
    }
    out.push(value.to_le_bytes()[0]);
}

fn push_uint(out: &mut Vec<u8>, field: u64, value: u64) {
    if value != 0 {
        push_varint(out, field << 3);
        push_varint(out, value);
    }
}

fn push_bytes(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    if !value.is_empty() {
        push_varint(out, (field << 3) | 2);
        push_varint(out, u64::try_from(value.len()).unwrap_or(u64::MAX));
        out.extend_from_slice(value);
    }
}

/// Re-encode `BlockHeader.raw` from its printed fields (proto3: zero values omitted).
#[must_use]
pub fn raw_header(header: &TronBlockHeader) -> Vec<u8> {
    let mut raw = Vec::with_capacity(128);
    push_uint(&mut raw, 1, header.timestamp);
    push_bytes(&mut raw, 2, &header.tx_trie_root);
    push_bytes(&mut raw, 3, &header.parent_hash);
    push_uint(&mut raw, 7, header.number);
    push_uint(&mut raw, 8, header.witness_id);
    push_bytes(&mut raw, 9, &header.witness_address);
    push_uint(&mut raw, 10, u64::from(header.version));
    push_bytes(&mut raw, 11, &header.account_state_root);
    raw
}

/// The block id of `raw`: `sha256(raw)` with the big-endian number in its first eight bytes.
#[must_use]
pub fn block_id(raw: &[u8], number: u64) -> [u8; 32] {
    let mut id: [u8; 32] = Sha256::digest(raw).into();
    id[..8].copy_from_slice(&number.to_be_bytes());
    id
}

fn contract_ret_code(name: &str) -> Option<u64> {
    const NAMES: [&str; 16] = [
        "DEFAULT",
        "SUCCESS",
        "REVERT",
        "BAD_JUMP_DESTINATION",
        "OUT_OF_MEMORY",
        "PRECOMPILED_CONTRACT",
        "STACK_TOO_SMALL",
        "STACK_TOO_LARGE",
        "ILLEGAL_OPERATION",
        "STACK_OVERFLOW",
        "OUT_OF_ENERGY",
        "OUT_OF_TIME",
        "JVM_STACK_OVER_FLOW",
        "UNKNOWN",
        "TRANSFER_FAILED",
        "INVALID_CODE",
    ];
    NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .and_then(|code| u64::try_from(code).ok())
}

/// Re-encode one `Transaction.Result` from its printed JSON object.
///
/// # Errors
///
/// [`BuildError::Unavailable`] for a result field this builder cannot re-encode.
pub fn encode_result(result: &Value) -> Result<Vec<u8>, BuildError> {
    let map = result
        .as_object()
        .ok_or_else(|| BuildError::Json("a transaction result is not an object".into()))?;
    let mut fields: Vec<(u64, Vec<u8>)> = Vec::new();
    for (key, value) in map {
        let mut out = Vec::new();
        let field = match key.as_str() {
            "fee" => 1,
            "ret" => 2,
            "contractRet" => 3,
            "assetIssueID" => 14,
            "withdraw_amount" => 15,
            "unfreeze_amount" => 16,
            "exchange_received_amount" => 18,
            "exchange_inject_another_amount" => 19,
            "exchange_withdraw_another_amount" => 20,
            "exchange_id" => 21,
            "shielded_transaction_fee" => 22,
            "withdraw_expire_amount" => 27,
            other => {
                return Err(BuildError::Unavailable(format!(
                    "transaction result field `{other}` cannot be re-encoded yet"
                )));
            }
        };
        match (field, value) {
            (2, Value::String(name)) => {
                push_uint(&mut out, 2, u64::from(name.as_str() != "SUCESS"));
            }
            (3, Value::String(name)) => {
                let code = contract_ret_code(name)
                    .ok_or_else(|| BuildError::Json(format!("unknown contractRet `{name}`")))?;
                push_uint(&mut out, 3, code);
            }
            (14, Value::String(text)) => push_bytes(&mut out, 14, text.as_bytes()),
            (_, value) => {
                let number = value.as_u64().ok_or_else(|| {
                    BuildError::Json(format!(
                        "transaction result `{key}` is not a non-negative number"
                    ))
                })?;
                push_uint(&mut out, field, number);
            }
        }
        fields.push((field, out));
    }
    fields.sort_by_key(|(field, _)| *field);
    Ok(fields.into_iter().flat_map(|(_, bytes)| bytes).collect())
}

/// Re-encode a full `Transaction` (raw data, signatures, results).
///
/// # Errors
///
/// A result that cannot be re-encoded.
pub fn encode_transaction(transaction: &TronTransaction) -> Result<Vec<u8>, BuildError> {
    let mut out = Vec::with_capacity(transaction.raw_data_hex.len() + 128);
    push_bytes(&mut out, 1, &transaction.raw_data_hex);
    for signature in &transaction.signatures {
        push_bytes(&mut out, 2, signature);
    }
    for result in &transaction.ret {
        let encoded = encode_result(result)?;
        push_varint(&mut out, (5 << 3) | 2);
        push_varint(&mut out, u64::try_from(encoded.len()).unwrap_or(u64::MAX));
        out.extend_from_slice(&encoded);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Source
// ---------------------------------------------------------------------------------------------

/// TRON data an evidence builder reads. [`TronBuilder`] serves it from the java-tron HTTP API.
pub trait TronSource {
    /// The height of the block holding the solidified transaction `tx_id`, and the transaction's
    /// inclusion under the block's `txTrieRoot`.
    ///
    /// # Errors
    ///
    /// Any endpoint failure, a transaction that is not solidified, or an inconsistent response.
    fn transaction(&self, tx_id: &[u8; 32]) -> Result<(u64, TronTransactionProofV1), BuildError>;

    /// Height of the newest block.
    ///
    /// # Errors
    ///
    /// Any endpoint failure.
    fn head_number(&self) -> Result<u64, BuildError>;

    /// Id-checked signed headers `first..=last`.
    ///
    /// # Errors
    ///
    /// Any endpoint failure or a block that is not served.
    fn segment(&self, first: u64, last: u64) -> Result<TronSegmentV1, BuildError>;

    /// Id-checked unsigned `raw_data` headers `first..=last`.
    ///
    /// # Errors
    ///
    /// Any endpoint failure or a block that is not served.
    fn raw_headers(&self, first: u64, last: u64) -> Result<Vec<Vec<u8>>, BuildError>;
}

/// Check that `raw` headers are parent-linked one height at a time; returns the last block id.
fn check_raw_chain(raw: &[Vec<u8>]) -> Result<[u8; 32], BuildError> {
    let summary = |header: &[u8]| {
        header_summary(header)
            .map_err(|error| BuildError::Inconsistent(format!("TRON header: {error}")))
    };
    let first = raw
        .first()
        .ok_or_else(|| BuildError::Inconsistent("an empty TRON header chain".into()))?;
    let mut previous = summary(first)?;
    for header in &raw[1..] {
        let next = summary(header)?;
        if next.parent_id != previous.id || previous.number.checked_add(1) != Some(next.number) {
            return Err(BuildError::Inconsistent(format!(
                "TRON block {} does not follow block {}",
                next.number, previous.number
            )));
        }
        previous = next;
    }
    Ok(previous.id)
}

/// Whether the light client stores the witness set of `period` and it is still fresh
/// [`super::FRESHNESS_MARGIN_MS`] after `now_ms`: until `ws_bound_ms` after the period ends
/// (§4.13.3).
#[must_use]
pub fn period_set_fresh_with_margin(
    profile: &TronChainProfileV1,
    sets: &[SccpLcConsensusSetV1],
    period: u64,
    ws_bound_ms: u64,
    now_ms: u64,
) -> bool {
    sets.iter().any(|set| set.set_id == period)
        && profile
            .period_end_ms(period)
            .is_some_and(|end| fresh_with_margin(Some(end), ws_bound_ms, now_ms))
}

fn proof_bytes(
    anchor: TronProofAnchorV1,
    transaction: TronTransactionProofV1,
) -> Result<SccpSourceProofBytesV1, BuildError> {
    SccpSourceProofV1::Tron(TronSourceProofV1 {
        anchor,
        transaction,
    })
    .to_bytes()
    .map_err(|error| BuildError::Inconsistent(format!("proof frame: {error}")))
}

/// Build the evidence of the transaction `tx_id` against the light client `taira` stores (see
/// the module documentation for the anchor order), with sets fresh at `now_ms`.
///
/// # Errors
///
/// Any source failure, or a block that neither a fresh witness set nor a retained checkpoint
/// (with at most [`super::MAX_BACKFILL_SEGMENTS`] backfills) reaches.
pub fn build_evidence<S: TronSource + ?Sized>(
    source: &S,
    profile: &TronChainProfileV1,
    tx_id: &[u8; 32],
    taira: &dyn TairaLightClientView,
    now_ms: u64,
) -> Result<SourceEvidenceV1, BuildError> {
    let light_client = taira.light_client()?;
    let params = light_client.params;
    let bound = |param: u32| u64::from(param).min(TRON_MAX_ANCESTRY_HEADERS as u64);
    let max_headers = bound(params.max_ancestry_headers);
    let max_backfill = bound(params.max_backfill_headers);
    let (number, transaction) = source.transaction(tx_id)?;
    let event = source
        .raw_headers(number, number)?
        .first()
        .map(Vec::as_slice)
        .map(header_summary)
        .ok_or_else(|| BuildError::Unavailable(format!("TRON block {number} is not served")))?
        .map_err(|error| BuildError::Inconsistent(format!("TRON header: {error}")))?;
    let period = profile.period_at(event.time_ms);
    // 1. A solid segment under the fresh set of the block's period.
    let sets = taira.sets()?;
    let solid_failure = if period > light_client.head.latest_set_id {
        format!("the light client has not learned TRON period {period} yet; advance it first")
    } else if period_set_fresh_with_margin(profile, &sets, period, params.ws_bound_ms, now_ms) {
        let last = number
            .saturating_add(SOLIDITY_TAIL)
            .min(source.head_number()?);
        if last <= number {
            return Err(BuildError::Unavailable(format!(
                "TRON block {number} has no descendants yet"
            )));
        }
        return Ok(SourceEvidenceV1 {
            backfills: Vec::new(),
            proof: proof_bytes(
                TronProofAnchorV1::Solid(source.segment(number, last)?),
                transaction,
            )?,
        });
    } else {
        format!("the witness set of TRON period {period} is not stored or no longer fresh")
    };
    // 2. Raw headers up to the nearest retained checkpoint, backfilled when it is far.
    let Some(checkpoint) = taira.checkpoint_covering(number)? else {
        return Err(BuildError::Unavailable(format!(
            "{solid_failure}; Taira retains no checkpoint at or above TRON block {number}"
        )));
    };
    let top = checkpoint.data.source_height;
    let plan = plan_backfill(number, top, max_headers.saturating_sub(1), max_backfill)?;
    let raw = source.raw_headers(number, top)?;
    if check_raw_chain(&raw)? != checkpoint.data.block_hash {
        return Err(BuildError::Inconsistent(format!(
            "the served TRON block {top} is not the stored checkpoint"
        )));
    }
    let index = |height: u64| usize::try_from(height - number).unwrap_or(usize::MAX);
    let backfills = plan
        .segments
        .iter()
        .map(|(first, last)| {
            backfill_bytes(SccpLcSegmentV1::Tron(TronRawSegmentV1 {
                headers: raw[index(*first)..=index(*last)].to_vec(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SourceEvidenceV1 {
        backfills,
        proof: proof_bytes(
            TronProofAnchorV1::Checkpoint(TronRawSegmentV1 {
                headers: raw[..=index(plan.anchor)].to_vec(),
            }),
            transaction,
        )?,
    })
}

// ---------------------------------------------------------------------------------------------
// Public-RPC builder
// ---------------------------------------------------------------------------------------------

/// TRON evidence builder over one java-tron HTTP API endpoint.
pub struct TronBuilder {
    api: TronClient,
    profile: TronChainProfileV1,
}

impl TronBuilder {
    /// A builder over `api` under the newest compiled TRON profile version.
    ///
    /// TODO(B11): build under the version active on the target Taira (Torii capabilities)
    /// rather than the newest compiled one.
    #[must_use]
    pub fn new(api: TronClient) -> Self {
        Self {
            api,
            profile: SccpChainProfilesV1::latest().tron,
        }
    }

    /// Moves the HTTP API client to its next endpoint, for a caller whose build failed on the
    /// data it was served or whose verification rejected what was built.
    pub fn rotate_endpoints(&self) {
        self.api.transport().rotate_preferred();
    }

    /// Fail unless the endpoint's next maintenance time lies on the compiled grid.
    ///
    /// # Errors
    ///
    /// Any endpoint failure, or [`BuildError::Inconsistent`] for another grid.
    pub fn check_maintenance_grid(&self) -> Result<(), BuildError> {
        let next = self.api.next_maintenance_time()?;
        match self.profile.period_start_ms(self.profile.period_at(next)) {
            Some(start) if start == next => Ok(()),
            _ => Err(BuildError::Inconsistent(format!(
                "the TRON maintenance time {next} is off the compiled grid; this release cannot \
                 verify TRON"
            ))),
        }
    }

    fn signed(block: &TronBlock) -> Result<TronSignedHeaderV1, BuildError> {
        let raw = raw_header(&block.header);
        if block_id(&raw, block.header.number) != block.block_id {
            return Err(BuildError::Inconsistent(format!(
                "the re-encoded header of TRON block {} does not match its id",
                block.header.number
            )));
        }
        Ok(TronSignedHeaderV1 {
            raw_data: raw,
            witness_signature: block.header.witness_signature.clone(),
        })
    }

    /// Blocks `first..=last` in number order.
    fn blocks(&self, first: u64, last: u64) -> Result<Vec<TronBlock>, BuildError> {
        let mut blocks = Vec::new();
        let mut start = first;
        while start <= last {
            let end = last.min(start + MAX_BLOCKS_PER_RANGE - 1);
            let chunk = self.api.blocks_by_limit_next(start, end + 1)?;
            if u64::try_from(chunk.len()).ok() != Some(end - start + 1)
                || chunk
                    .iter()
                    .zip(start..)
                    .any(|(block, number)| block.header.number != number)
            {
                return Err(BuildError::Unavailable(format!(
                    "TRON blocks {start}..={end} are not all served"
                )));
            }
            blocks.extend(chunk);
            start = end + 1;
        }
        Ok(blocks)
    }

    /// The maintenance block of `period`: the first block timestamped at or after its start.
    fn maintenance_block(&self, period: u64, head: &TronBlock) -> Result<u64, BuildError> {
        let start = self
            .profile
            .period_start_ms(period)
            .ok_or_else(|| BuildError::Inconsistent("period start overflows".into()))?;
        first_block_at_or_after(
            start,
            self.profile.block_interval_ms,
            (head.header.number, head.header.timestamp),
            |low, high| {
                Ok(self
                    .blocks(low, high)?
                    .iter()
                    .map(|block| (block.header.number, block.header.timestamp))
                    .collect())
            },
        )?
        .ok_or_else(|| {
            BuildError::Unavailable(format!(
                "the maintenance block of TRON period {period} was not found"
            ))
        })
    }

    /// Slots of the witness-learning window after a maintenance block (two production rounds
    /// plus the skipped slots); a block at most this many heights after the maintenance block can
    /// lie in the window.
    fn window_blocks(&self) -> u64 {
        self.profile
            .learning_window_ms()
            .checked_div(self.profile.block_interval_ms)
            .unwrap_or(0)
    }

    /// The witness set of `period` learned from its window, as the light client learns it.
    fn learned_set(&self, period: u64, maintenance: u64) -> Result<TronWitnessSetV1, BuildError> {
        let window = self.profile.learning_window_ms();
        let blocks = self.blocks(maintenance, maintenance + self.window_blocks() + 1)?;
        let end = blocks[0].header.timestamp + window;
        let mut witnesses = std::collections::BTreeMap::new();
        for block in blocks
            .iter()
            .skip(1)
            .filter(|block| block.header.timestamp <= end)
        {
            let signed = Self::signed(block)?;
            let signing_address = header_signer(&signed.raw_data, &signed.witness_signature)
                .map_err(|error| BuildError::Inconsistent(format!("TRON header: {error}")))?;
            witnesses.insert(block.header.witness_address, signing_address);
        }
        Ok(TronWitnessSetV1 {
            period,
            witnesses: witnesses
                .into_iter()
                .map(|(account, signer)| TronWitnessV1 {
                    account_address: account.to_vec(),
                    signing_address: signer.to_vec(),
                })
                .collect(),
        })
    }
}

impl TronSource for TronBuilder {
    fn transaction(&self, tx_id: &[u8; 32]) -> Result<(u64, TronTransactionProofV1), BuildError> {
        let info = self
            .api
            .solidity_transaction_info(tx_id)?
            .ok_or_else(|| BuildError::Unavailable("the transaction is not solidified".into()))?;
        let number = info.block_number;
        let block = self
            .api
            .block_by_num(number)?
            .ok_or_else(|| BuildError::Unavailable(format!("TRON block {number} is not served")))?;
        let encoded = block
            .transactions
            .iter()
            .map(encode_transaction)
            .collect::<Result<Vec<_>, _>>()?;
        let leaves: Vec<[u8; 32]> = encoded
            .iter()
            .map(|transaction| Sha256::digest(transaction).into())
            .collect();
        let index = block
            .transactions
            .iter()
            .position(|transaction| {
                <[u8; 32]>::from(Sha256::digest(&transaction.raw_data_hex)) == *tx_id
            })
            .ok_or_else(|| {
                BuildError::Inconsistent("the transaction is not in its block".into())
            })?;
        let (root, merkle_branch) = merkle_root_and_branch(&leaves, index)
            .ok_or_else(|| BuildError::Inconsistent("empty TRON block".into()))?;
        if root != block.header.tx_trie_root {
            return Err(BuildError::Inconsistent(
                "the re-encoded transactions do not rebuild txTrieRoot".into(),
            ));
        }
        Ok((
            number,
            TronTransactionProofV1 {
                transaction_index: u32::try_from(index)
                    .map_err(|_| BuildError::Inconsistent("transaction index overflows".into()))?,
                transaction_count: u32::try_from(leaves.len())
                    .map_err(|_| BuildError::Inconsistent("transaction count overflows".into()))?,
                transaction: encoded[index].clone(),
                merkle_branch,
            },
        ))
    }

    fn head_number(&self) -> Result<u64, BuildError> {
        Ok(self.api.now_block()?.header.number)
    }

    fn segment(&self, first: u64, last: u64) -> Result<TronSegmentV1, BuildError> {
        Ok(TronSegmentV1 {
            headers: self
                .blocks(first, last)?
                .iter()
                .map(Self::signed)
                .collect::<Result<_, _>>()?,
        })
    }

    fn raw_headers(&self, first: u64, last: u64) -> Result<Vec<Vec<u8>>, BuildError> {
        self.blocks(first, last)?
            .iter()
            .map(|block| Self::signed(block).map(|signed| signed.raw_data))
            .collect()
    }
}

impl SourceChainBuilder for TronBuilder {
    fn network(&self) -> SccpNetworkV1 {
        SccpNetworkV1::TronMainnet
    }

    /// The bootstrap of the newest solidified block's period.
    fn bootstrap(&self) -> Result<SccpLcBootstrapV1, BuildError> {
        self.check_maintenance_grid()?;
        let solid = self.api.solidity_now_block()?;
        let head = self.api.now_block()?;
        let period = self.profile.period_at(solid.header.timestamp);
        let maintenance = self.maintenance_block(period, &head)?;
        if solid.header.number <= maintenance + self.window_blocks() {
            return Err(BuildError::Unavailable(
                "the current TRON period has just begun; retry in four minutes".into(),
            ));
        }
        SccpLcBootstrapDataV1::Tron(TronLcBootstrapV1 {
            set: self.learned_set(period, maintenance)?,
            checkpoint_header: Self::signed(&solid)?.raw_data,
        })
        .to_bootstrap()
        .map_err(|error| BuildError::Inconsistent(format!("bootstrap frame: {error}")))
    }

    /// One segment per later maintenance boundary, or one segment of the newest blocks, stepped
    /// to `budget`.
    fn advance(
        &self,
        latest_set_id: u64,
        budget: AdvanceBudgetV1,
    ) -> Result<SccpLcAdvanceBytesV1, BuildError> {
        self.check_maintenance_grid()?;
        let head = self.api.now_block()?;
        let current = self.profile.period_at(head.header.timestamp);
        let mut segments = Vec::new();
        let span = TRON_MAX_SEGMENT_HEADERS as u64 - 1;
        for period in latest_set_id + 1..=current {
            if segments.len() == budget.max_items {
                break;
            }
            let maintenance = self.maintenance_block(period, &head)?;
            let last =
                (maintenance + self.window_blocks() + SOLIDITY_TAIL).min(maintenance - 1 + span);
            if last > head.header.number {
                break;
            }
            segments.push(self.segment(maintenance - 1, last)?);
        }
        if segments.is_empty() && current == latest_set_id {
            let last = head.header.number;
            segments.push(self.segment(last - SOLIDITY_TAIL - 15, last)?);
        }
        if segments.is_empty() {
            return Err(BuildError::Unavailable(
                "the next TRON maintenance window is not produced yet".into(),
            ));
        }
        fit_advance(segments, budget, |segments| {
            SccpLcAdvanceV1::Tron(TronLcAdvanceV1 { segments })
        })
    }

    fn evidence(
        &self,
        event: &SourceEventRefV1,
        light_client: &dyn TairaLightClientView,
        now_ms: u64,
    ) -> Result<SourceEvidenceV1, BuildError> {
        match event {
            SourceEventRefV1::Tron { tx_id } => {
                build_evidence(self, &self.profile, tx_id, light_client, now_ms)
            }
            SourceEventRefV1::Evm { .. } | SourceEventRefV1::Ton { .. } => {
                Err(BuildError::Inconsistent("not a TRON event".into()))
            }
        }
    }
}

#[cfg(test)]
mod evidence_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotating_moves_the_client_to_its_next_endpoint() {
        let builder = TronBuilder::new(TronClient::new(
            crate::builders::test_support::two_endpoint_transport(),
        ));
        assert_eq!(builder.api.transport().endpoints().preferred(), 0);
        builder.rotate_endpoints();
        assert_eq!(builder.api.transport().endpoints().preferred(), 1);
        builder.rotate_endpoints();
        assert_eq!(builder.api.transport().endpoints().preferred(), 0);
    }

    /// Blocks `1..` on a 3 s slot grid from `origin_ms`, skipping every slot in `missed`.
    fn chain(origin_ms: u64, slots: u64, missed: &[u64]) -> Vec<(u64, u64)> {
        (0..slots)
            .filter(|slot| !missed.contains(slot))
            .zip(1..)
            .map(|(slot, number)| (number, origin_ms + slot * 3_000))
            .collect()
    }

    fn search(blocks: &[(u64, u64)], start_ms: u64) -> Option<u64> {
        let head = *blocks.last().unwrap();
        first_block_at_or_after(start_ms, 3_000, head, |low, high| {
            Ok(blocks
                .iter()
                .copied()
                .filter(|(number, _)| (low..=high).contains(number))
                .collect())
        })
        .unwrap()
    }

    #[test]
    fn maintenance_search_reads_forward_past_missed_slots() {
        let origin = 1_790_596_800_000 - 600 * 3_000;
        let start = 1_790_596_800_000;
        // No missed slots: the slot at `start` is block 601.
        assert_eq!(search(&chain(origin, 2_000, &[]), start), Some(601));
        // 150 missed slots before and 350 after the boundary: block numbers fall behind slots.
        let missed: Vec<u64> = (100..250).chain(700..1_050).collect();
        let blocks = chain(origin, 2_000, &missed);
        let found = search(&blocks, start).unwrap();
        let position = blocks
            .iter()
            .position(|(number, _)| *number == found)
            .unwrap();
        assert!(blocks[position].1 >= start && blocks[position - 1].1 < start);
        // A missed boundary slot moves the block to the next produced slot.
        let blocks = chain(origin, 2_000, &[600, 601]);
        let found = search(&blocks, start).unwrap();
        assert_eq!(
            blocks[usize::try_from(found).unwrap() - 1].1,
            start + 2 * 3_000
        );
        // A period that has not started, and the first block, are handled.
        assert_eq!(search(&chain(origin, 10, &[]), start), None);
        assert_eq!(search(&chain(start, 10, &[]), start), Some(1));
    }

    #[test]
    fn headers_reencode_with_proto3_defaults_omitted() {
        let header = TronBlockHeader {
            number: 300,
            timestamp: 1_700_000_000_000,
            tx_trie_root: [1; 32],
            parent_hash: [2; 32],
            witness_address: [0x41; 21],
            witness_id: 0,
            version: 31,
            account_state_root: Vec::new(),
            witness_signature: Vec::new(),
        };
        let raw = raw_header(&header);
        assert_eq!(raw[0], 0x08, "timestamp is field 1");
        assert!(raw.windows(3).any(|window| window == [0x38, 0xac, 0x02]));
        assert!(
            !raw.windows(1).any(|window| window == [0x40]),
            "no witness_id"
        );
        let summary = iroha_sccp::light_client::tron::header_summary(&raw).expect("decodes");
        assert_eq!(summary.number, 300);
        assert_eq!(summary.id, block_id(&raw, 300));
    }

    #[test]
    fn results_reencode_in_field_order() {
        let result = norito::json!({"contractRet": "SUCCESS", "fee": 345});
        assert_eq!(
            encode_result(&result).expect("encodes"),
            vec![0x08, 0xd9, 0x02, 0x18, 0x01]
        );
        let failed = norito::json!({"ret": "FAILED", "contractRet": "REVERT"});
        assert_eq!(
            encode_result(&failed).expect("encodes"),
            vec![0x10, 0x01, 0x18, 0x02]
        );
        assert!(encode_result(&norito::json!({"orderDetails": []})).is_err());
        assert_eq!(contract_ret_code("OUT_OF_ENERGY"), Some(10));
    }
}
