//! Authoritative SoraFS orderbook policy and signed-payload ledger handlers.

use std::{str::FromStr, sync::OnceLock};

use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::{
            CancelSorafsOrderbookOrder, RecordSorafsOrderbookSettlementReceipt,
            SetSorafsOrderbookPolicy, SubmitSorafsOrderbookOrder,
        },
    },
    name::Name,
    query::{
        error::{FindError, QueryExecutionFail},
        sorafs::prelude::{
            FindSorafsOrderbookCancellationByOrderId, FindSorafsOrderbookOrderById,
            FindSorafsOrderbookOrders, FindSorafsOrderbookPolicy, FindSorafsOrderbookReceiptById,
            FindSorafsOrderbookReceipts, FindSorafsOrderbookStatus,
        },
    },
    sorafs::orderbook::{
        ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1, ORDERBOOK_QUERY_MAX_ITEMS_V1,
        OrderbookAdmissionPolicyRecord, OrderbookCancellationRecord, OrderbookLedgerStatusV1,
        OrderbookOrderPageV1, OrderbookOrderRecord, OrderbookOrderStatusV1,
        OrderbookOwnerNonceRecord, OrderbookSettlementIndexRecord, OrderbookSettlementRangeRecord,
        OrderbookSettlementReceiptPageV1, OrderbookSettlementReceiptRecord,
        orderbook_settlement_escrow_id,
    },
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;
use norito::{DecodeLimits, decode_from_bytes_with_limits};
use sorafs_manifest::{
    orderbook::{
        OrderCancelReasonV1, OrderRequestV1, OrderSideV1, OrderbookSignatureV1,
        decode_order_cancel_v1, decode_order_request_v1, decode_settlement_receipt_v1,
        verify_order_cancel_signature_v1, verify_order_request_signature_v1,
        verify_settlement_receipt_signature_v1,
    },
    provider_advert::SignatureAlgorithm,
};

use super::*;
use crate::smartcontracts::ValidSingularQuery;
use crate::state::{StateTransaction, WorldReadOnly};

const POLICY_STATE_KEY: &str = "sorafs_orderbook_policy_v1";
const STATUS_STATE_KEY: &str = "sorafs_orderbook_status_v1";
const ORDER_STATE_KEY_PREFIX: &str = "sorafs_orderbook_order_v1_";
const NONCE_STATE_KEY_PREFIX: &str = "sorafs_orderbook_nonce_v1_";
const RECEIPT_STATE_KEY_PREFIX: &str = "sorafs_orderbook_receipt_v1_";
const RECEIPT_INDEX_KEY_PREFIX: &str = "sorafs_orderbook_receipt_index_v1_";
const NONCE_KEY_DOMAIN_V1: &[u8] = b"sorafs.orderbook.owner-nonce-state.v1";
const STATE_MAX_BYTES: usize = 2 * 1024 * 1024;
const STATE_LIMITS: DecodeLimits = DecodeLimits::new(
    ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1 as usize,
    STATE_MAX_BYTES,
    ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1 as usize * 8,
    STATE_MAX_BYTES * 2,
    64,
);

fn invalid_parameter(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(
        message.into(),
    ))
}

fn corrupt_state(message: impl Into<String>) -> InstructionExecutionError {
    InstructionExecutionError::InvariantViolation(message.into().into())
}

fn has_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> bool {
    state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| {
            permissions
                .iter()
                .any(|candidate| candidate.name() == permission)
        })
}

fn require_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    permission: &str,
) -> Result<(), InstructionExecutionError> {
    if has_permission(state_transaction, authority, permission) {
        Ok(())
    } else {
        Err(invalid_parameter(format!(
            "permission {permission} required for authoritative SoraFS orderbook operation"
        )))
    }
}

fn policy_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| Name::from_str(POLICY_STATE_KEY).expect("static state key is valid"))
}

fn status_key() -> &'static Name {
    static KEY: OnceLock<Name> = OnceLock::new();
    KEY.get_or_init(|| Name::from_str(STATUS_STATE_KEY).expect("static state key is valid"))
}

fn digest_key(prefix: &str, digest: [u8; 32]) -> Name {
    Name::from_str(&format!("{prefix}{}", hex::encode(digest)))
        .expect("static prefix plus lowercase hex is a valid state key")
}

fn order_key(order_id: [u8; 32]) -> Name {
    digest_key(ORDER_STATE_KEY_PREFIX, order_id)
}

fn receipt_key(receipt_id: [u8; 32]) -> Name {
    digest_key(RECEIPT_STATE_KEY_PREFIX, receipt_id)
}

fn receipt_index_key(channel_id: [u8; 32]) -> Name {
    digest_key(RECEIPT_INDEX_KEY_PREFIX, channel_id)
}

fn nonce_key(owner: &AccountId) -> Name {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NONCE_KEY_DOMAIN_V1);
    hasher.update(owner.to_string().as_bytes());
    digest_key(NONCE_STATE_KEY_PREFIX, *hasher.finalize().as_bytes())
}

fn encode_state<T: norito::core::NoritoSerialize>(
    value: &T,
    label: &str,
) -> Result<Vec<u8>, InstructionExecutionError> {
    norito::to_bytes(value)
        .map_err(|error| corrupt_state(format!("failed to encode {label}: {error}")))
}

fn decode_state<T>(bytes: &[u8], label: &str) -> Result<T, InstructionExecutionError>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if bytes.len() > STATE_MAX_BYTES {
        return Err(corrupt_state(format!(
            "{label} state exceeds {STATE_MAX_BYTES} bytes"
        )));
    }
    let value = decode_from_bytes_with_limits::<T>(bytes, STATE_LIMITS)
        .map_err(|error| corrupt_state(format!("failed to decode {label}: {error}")))?;
    let canonical = encode_state(&value, label)?;
    if canonical != bytes {
        return Err(corrupt_state(format!(
            "{label} state is not exact canonical Norito"
        )));
    }
    Ok(value)
}

fn block_time_unix(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<u64, InstructionExecutionError> {
    let now = state_transaction.block_unix_timestamp_ms() / 1_000;
    if now == 0 {
        return Err(invalid_parameter(
            "authoritative orderbook operations require a non-zero block timestamp",
        ));
    }
    Ok(now)
}

fn read_policy(
    world: &impl WorldReadOnly,
) -> Result<Option<OrderbookAdmissionPolicyRecord>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(policy_key()) else {
        return Ok(None);
    };
    let record: OrderbookAdmissionPolicyRecord = decode_state(bytes, "orderbook policy")?;
    record
        .policy
        .validate()
        .map_err(|error| corrupt_state(format!("invalid stored orderbook policy: {error}")))?;
    let expected = record
        .policy
        .digest()
        .map_err(|error| corrupt_state(format!("failed to digest stored policy: {error}")))?;
    if expected != record.policy_digest || record.activated_at_unix == 0 {
        return Err(corrupt_state(
            "stored orderbook policy digest or activation timestamp is invalid",
        ));
    }
    Ok(Some(record))
}

fn read_status(
    world: &impl WorldReadOnly,
) -> Result<Option<OrderbookLedgerStatusV1>, InstructionExecutionError> {
    let Some(bytes) = world.smart_contract_state().get(status_key()) else {
        return Ok(None);
    };
    let status: OrderbookLedgerStatusV1 = decode_state(bytes, "orderbook ledger status")?;
    if status.updated_at_unix == 0
        || status.settlement_channels > status.settlement_receipts
        || status
            .open_orders
            .checked_add(status.cancelled_orders)
            .is_none()
    {
        return Err(corrupt_state(
            "stored orderbook ledger status counters or timestamp are invalid",
        ));
    }
    Ok(Some(status))
}

fn active_status(
    state_transaction: &StateTransaction<'_, '_>,
    now: u64,
) -> Result<OrderbookLedgerStatusV1, InstructionExecutionError> {
    let status = read_status(state_transaction.world())?
        .ok_or_else(|| corrupt_state("active orderbook policy has no ledger status record"))?;
    if status.updated_at_unix > now {
        return Err(corrupt_state(
            "stored orderbook ledger status is later than the current block",
        ));
    }
    Ok(status)
}

fn encode_status(status: &OrderbookLedgerStatusV1) -> Result<Vec<u8>, InstructionExecutionError> {
    encode_state(status, "orderbook ledger status")
}

fn active_policy(
    state_transaction: &StateTransaction<'_, '_>,
    supplied_digest: [u8; 32],
) -> Result<(OrderbookAdmissionPolicyRecord, u64), InstructionExecutionError> {
    let now = block_time_unix(state_transaction)?;
    let record = read_policy(state_transaction.world())?
        .ok_or_else(|| invalid_parameter("SoraFS orderbook admission policy is not configured"))?;
    if record.activated_at_unix > now {
        return Err(corrupt_state(
            "stored orderbook policy activation is later than the current block",
        ));
    }
    if supplied_digest != record.policy_digest {
        return Err(invalid_parameter(format!(
            "orderbook policy digest mismatch: supplied {}, active {}",
            hex::encode(supplied_digest),
            hex::encode(record.policy_digest)
        )));
    }
    Ok((record, now))
}

fn canonical_owner(
    owner_bytes: &[u8],
    authority: &AccountId,
) -> Result<AccountId, InstructionExecutionError> {
    let literal = core::str::from_utf8(owner_bytes)
        .map_err(|_| invalid_parameter("orderbook owner account must be canonical UTF-8 I105"))?;
    let parsed = AccountId::parse_encoded(literal).map_err(|error| {
        invalid_parameter(format!(
            "invalid orderbook owner account: {}",
            error.reason()
        ))
    })?;
    if parsed.canonical().as_bytes() != owner_bytes {
        return Err(invalid_parameter(
            "orderbook owner account must be exact canonical I105 bytes",
        ));
    }
    let owner = parsed.account_id().clone();
    if owner.subject_id() != authority.subject_id() {
        return Err(invalid_parameter(format!(
            "orderbook owner {owner} does not match transaction authority {authority}"
        )));
    }
    Ok(owner)
}

fn ensure_payload_signer(
    authority: &AccountId,
    signature: &OrderbookSignatureV1,
) -> Result<(), InstructionExecutionError> {
    if signature.algorithm != SignatureAlgorithm::Ed25519 {
        return Err(invalid_parameter(
            "orderbook payload signer must use Ed25519",
        ));
    }
    let public_key = authority.try_signatory().ok_or_else(|| {
        invalid_parameter(
            "first-release orderbook payload admission requires a single-signatory account",
        )
    })?;
    let (algorithm, payload) = public_key
        .try_to_bytes()
        .map_err(|error| invalid_parameter(format!("invalid authority public key: {error}")))?;
    if algorithm != Algorithm::Ed25519 || payload != signature.public_key.as_slice() {
        return Err(invalid_parameter(
            "embedded orderbook payload signer does not match the transaction authority",
        ));
    }
    Ok(())
}

fn read_nonce(
    world: &impl WorldReadOnly,
    owner: &AccountId,
) -> Result<Option<OrderbookOwnerNonceRecord>, InstructionExecutionError> {
    let key = nonce_key(owner);
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let record: OrderbookOwnerNonceRecord = decode_state(bytes, "orderbook owner nonce")?;
    if record.owner.subject_id() != owner.subject_id() || record.highest_nonce == 0 {
        return Err(corrupt_state(
            "stored orderbook owner nonce has an invalid owner or zero high-water",
        ));
    }
    Ok(Some(record))
}

fn ensure_nonce_advances(
    world: &impl WorldReadOnly,
    owner: &AccountId,
    nonce: u64,
) -> Result<(), InstructionExecutionError> {
    if let Some(current) = read_nonce(world, owner)?
        && nonce <= current.highest_nonce
    {
        return Err(invalid_parameter(format!(
            "orderbook nonce {nonce} is stale or replayed; highest committed nonce is {}",
            current.highest_nonce
        )));
    }
    Ok(())
}

fn write_nonce(
    state_transaction: &mut StateTransaction<'_, '_>,
    owner: &AccountId,
    nonce: u64,
) -> Result<(), InstructionExecutionError> {
    let record = OrderbookOwnerNonceRecord {
        owner: owner.clone(),
        highest_nonce: nonce,
    };
    let encoded = encode_state(&record, "orderbook owner nonce")?;
    state_transaction
        .world
        .smart_contract_state
        .insert(nonce_key(owner), encoded);
    Ok(())
}

fn read_order(
    world: &impl WorldReadOnly,
    order_id: [u8; 32],
) -> Result<Option<OrderbookOrderRecord>, InstructionExecutionError> {
    let key = order_key(order_id);
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let record: OrderbookOrderRecord = decode_state(bytes, "orderbook order")?;
    if record.order_id != order_id
        || record.order_id == [0; 32]
        || record.admitted_policy_digest == [0; 32]
        || record.admitted_at_unix == 0
    {
        return Err(corrupt_state("stored orderbook order metadata is invalid"));
    }
    match record.status {
        OrderbookOrderStatusV1::Open
            if record.canonical_cancel.is_none()
                && record.cancelled_at_unix.is_none()
                && record.cancelled_policy_digest.is_none() => {}
        OrderbookOrderStatusV1::Cancelled
            if record.canonical_cancel.is_some()
                && record.cancelled_at_unix.is_some()
                && record
                    .cancelled_policy_digest
                    .is_some_and(|digest| digest != [0; 32]) => {}
        _ => {
            return Err(corrupt_state(
                "stored orderbook order cancellation state is inconsistent",
            ));
        }
    }
    let order = decode_order_request_v1(&record.canonical_order)
        .map_err(|error| corrupt_state(format!("invalid stored order payload: {error}")))?;
    verify_order_request_signature_v1(&order)
        .map_err(|error| corrupt_state(format!("invalid stored order signature: {error}")))?;
    ensure_payload_signer(&record.owner, &order.signature).map_err(|error| {
        corrupt_state(format!(
            "stored order signer does not match its authoritative owner: {error}"
        ))
    })?;
    if order.expiry_unix <= record.admitted_at_unix {
        return Err(corrupt_state(
            "stored order expiry is not later than its admission timestamp",
        ));
    }
    let owner_literal = core::str::from_utf8(&order.owner_account)
        .map_err(|_| corrupt_state("stored order owner is not UTF-8"))?;
    let owner = AccountId::parse_encoded(owner_literal)
        .map_err(|error| corrupt_state(format!("invalid stored order owner: {error}")))?;
    if owner.canonical().as_bytes() != order.owner_account
        || owner.account_id().subject_id() != record.owner.subject_id()
        || order.order_id != record.order_id
    {
        return Err(corrupt_state(
            "stored order payload does not match authoritative order metadata",
        ));
    }
    if let Some(canonical_cancel) = record.canonical_cancel.as_ref() {
        let cancel = decode_order_cancel_v1(canonical_cancel)
            .map_err(|error| corrupt_state(format!("invalid stored cancellation: {error}")))?;
        verify_order_cancel_signature_v1(&cancel).map_err(|error| {
            corrupt_state(format!("invalid stored cancellation signature: {error}"))
        })?;
        ensure_payload_signer(&record.owner, &cancel.signature).map_err(|error| {
            corrupt_state(format!(
                "stored cancellation signer does not match its authoritative owner: {error}"
            ))
        })?;
        let cancel_owner_literal = core::str::from_utf8(&cancel.owner_account)
            .map_err(|_| corrupt_state("stored cancellation owner is not UTF-8"))?;
        let cancel_owner = AccountId::parse_encoded(cancel_owner_literal).map_err(|error| {
            corrupt_state(format!("invalid stored cancellation owner: {error}"))
        })?;
        if cancel_owner.canonical().as_bytes() != cancel.owner_account
            || cancel_owner.account_id().subject_id() != record.owner.subject_id()
            || cancel.order_id != record.order_id
            || cancel.nonce <= order.nonce
            || record
                .cancelled_at_unix
                .is_none_or(|cancelled_at| cancelled_at < record.admitted_at_unix)
            || matches!(cancel.reason, OrderCancelReasonV1::Expired)
                && record
                    .cancelled_at_unix
                    .is_none_or(|cancelled_at| cancelled_at <= order.expiry_unix)
        {
            return Err(corrupt_state(
                "stored cancellation does not match authoritative order metadata",
            ));
        }
    }
    Ok(Some(record))
}

fn validate_order_policy(
    order: &OrderRequestV1,
    policy: &OrderbookAdmissionPolicyRecord,
    now: u64,
) -> Result<(), InstructionExecutionError> {
    if policy.policy.paused {
        return Err(invalid_parameter(
            "new SoraFS orderbook submissions are paused by governance",
        ));
    }
    if order.remaining_gib != order.quantity_gib {
        return Err(invalid_parameter(
            "new order submission must start with remaining_gib equal to quantity_gib",
        ));
    }
    if order.quantity_gib < policy.policy.min_order_gib
        || order.quantity_gib > policy.policy.max_order_gib
    {
        return Err(invalid_parameter(format!(
            "order quantity {} is outside governed bounds {}..={} GiB",
            order.quantity_gib, policy.policy.min_order_gib, policy.policy.max_order_gib
        )));
    }
    if order.price_per_gib.as_micro() % u128::from(policy.policy.price_tick_micro_xor) != 0 {
        return Err(invalid_parameter(format!(
            "order price {} is not aligned to governed tick {} micro-XOR",
            order.price_per_gib.as_micro(),
            policy.policy.price_tick_micro_xor
        )));
    }
    if order.maker_fee_bps > policy.policy.max_maker_fee_bps
        || order.taker_fee_bps > policy.policy.max_taker_fee_bps
    {
        return Err(invalid_parameter(
            "order maker or taker fee exceeds the active governance policy",
        ));
    }
    let lifetime = order
        .expiry_unix
        .checked_sub(now)
        .filter(|lifetime| *lifetime > 0)
        .ok_or_else(|| invalid_parameter("order is expired at ledger admission"))?;
    if lifetime > policy.policy.max_order_lifetime_secs {
        return Err(invalid_parameter(format!(
            "order lifetime {lifetime} seconds exceeds governed maximum {}",
            policy.policy.max_order_lifetime_secs
        )));
    }
    Ok(())
}

fn owner_has_registered_provider(world: &impl WorldReadOnly, owner: &AccountId) -> bool {
    world
        .provider_owners()
        .iter()
        .any(|(_, registered)| registered.subject_id() == owner.subject_id())
}

fn read_receipt(
    world: &impl WorldReadOnly,
    receipt_id: [u8; 32],
) -> Result<Option<OrderbookSettlementReceiptRecord>, InstructionExecutionError> {
    let key = receipt_key(receipt_id);
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let record: OrderbookSettlementReceiptRecord =
        decode_state(bytes, "orderbook settlement receipt")?;
    if record.receipt_id != receipt_id
        || record.receipt_id == [0; 32]
        || record.channel_id == [0; 32]
        || record.trade_id == [0; 32]
        || record.admitted_policy_digest == [0; 32]
        || record.admitted_at_unix == 0
    {
        return Err(corrupt_state(
            "stored orderbook settlement receipt metadata is invalid",
        ));
    }
    let receipt = decode_settlement_receipt_v1(&record.canonical_receipt)
        .map_err(|error| corrupt_state(format!("invalid stored settlement receipt: {error}")))?;
    verify_settlement_receipt_signature_v1(&receipt).map_err(|error| {
        corrupt_state(format!(
            "invalid stored settlement receipt signature: {error}"
        ))
    })?;
    ensure_payload_signer(&record.recorded_by, &receipt.settlement_signature).map_err(|error| {
        corrupt_state(format!(
            "stored settlement signer does not match its recording authority: {error}"
        ))
    })?;
    if receipt.receipt_id != record.receipt_id
        || receipt.channel_id != record.channel_id
        || receipt.trade_id != record.trade_id
    {
        return Err(corrupt_state(
            "stored settlement receipt payload does not match authoritative metadata",
        ));
    }
    let index = read_receipt_index(world, record.channel_id)?.ok_or_else(|| {
        corrupt_state("stored settlement receipt has no authoritative channel index")
    })?;
    let indexed_range = index
        .ranges
        .iter()
        .find(|range| range.receipt_id == record.receipt_id)
        .ok_or_else(|| {
            corrupt_state("stored settlement receipt is absent from its channel index")
        })?;
    if index.trade_id != record.trade_id
        || indexed_range.start != receipt.range.start
        || indexed_range.end != receipt.range.end
        || indexed_range.issued_at_unix != receipt.issued_at_unix
    {
        return Err(corrupt_state(
            "stored settlement receipt does not match its channel index",
        ));
    }
    Ok(Some(record))
}

fn validate_receipt_index(
    index: &OrderbookSettlementIndexRecord,
    channel_id: [u8; 32],
) -> Result<(), InstructionExecutionError> {
    if index.channel_id != channel_id
        || index.channel_id == [0; 32]
        || index.trade_id == [0; 32]
        || index.ranges.is_empty()
        || index.ranges.len() > ORDERBOOK_MAX_RECEIPTS_PER_CHANNEL_V1 as usize
    {
        return Err(corrupt_state(
            "stored orderbook receipt index header is invalid",
        ));
    }
    let mut previous: Option<&OrderbookSettlementRangeRecord> = None;
    let mut receipt_ids = std::collections::BTreeSet::new();
    for range in &index.ranges {
        if range.receipt_id == [0; 32]
            || !receipt_ids.insert(range.receipt_id)
            || range.start >= range.end
            || range.issued_at_unix == 0
            || previous.is_some_and(|prior| {
                (prior.start, prior.end, prior.receipt_id)
                    >= (range.start, range.end, range.receipt_id)
                    || prior.end > range.start
            })
        {
            return Err(corrupt_state(
                "stored orderbook receipt index is unsorted, overlapping, or malformed",
            ));
        }
        previous = Some(range);
    }
    Ok(())
}

fn read_receipt_index(
    world: &impl WorldReadOnly,
    channel_id: [u8; 32],
) -> Result<Option<OrderbookSettlementIndexRecord>, InstructionExecutionError> {
    let key = receipt_index_key(channel_id);
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let index: OrderbookSettlementIndexRecord =
        decode_state(bytes, "orderbook settlement receipt index")?;
    validate_receipt_index(&index, channel_id)?;
    Ok(Some(index))
}

impl Execute for SetSorafsOrderbookPolicy {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_permission(state_transaction, authority, "CanSetSorafsPricing")?;
        self.policy.validate().map_err(|error| {
            invalid_parameter(format!("invalid SoraFS orderbook policy: {error}"))
        })?;
        let now = block_time_unix(state_transaction)?;
        let digest = self.policy.digest().map_err(|error| {
            invalid_parameter(format!("failed to digest orderbook policy: {error}"))
        })?;

        let current_policy = read_policy(state_transaction.world())?;
        let initial_status = match current_policy.as_ref() {
            None => {
                if self.policy.revision != 1 || self.policy.predecessor_policy_digest.is_some() {
                    return Err(invalid_parameter(
                        "first orderbook policy must be revision one without a predecessor",
                    ));
                }
                if read_status(state_transaction.world())?.is_some() {
                    return Err(corrupt_state(
                        "orderbook ledger status exists without an active policy",
                    ));
                }
                Some(OrderbookLedgerStatusV1 {
                    open_orders: 0,
                    cancelled_orders: 0,
                    settlement_receipts: 0,
                    settlement_channels: 0,
                    updated_at_unix: now,
                })
            }
            Some(current) => {
                active_status(state_transaction, now)?;
                let expected_revision =
                    current.policy.revision.checked_add(1).ok_or_else(|| {
                        corrupt_state("stored orderbook policy revision overflowed")
                    })?;
                if self.policy.revision != expected_revision {
                    return Err(invalid_parameter(format!(
                        "orderbook policy revision {} must exactly follow active revision {}",
                        self.policy.revision, current.policy.revision
                    )));
                }
                if self.policy.predecessor_policy_digest != Some(current.policy_digest) {
                    return Err(invalid_parameter(
                        "orderbook policy predecessor does not match the active policy digest",
                    ));
                }
                if self.policy.market_id != current.policy.market_id {
                    return Err(invalid_parameter(
                        "orderbook market id is immutable after first activation",
                    ));
                }
                None
            }
        };

        let record = OrderbookAdmissionPolicyRecord {
            policy: self.policy,
            policy_digest: digest,
            activated_at_unix: now,
            activated_by: authority.clone(),
        };
        let encoded = encode_state(&record, "orderbook policy")?;
        let encoded_status = initial_status.as_ref().map(encode_status).transpose()?;
        state_transaction
            .world
            .smart_contract_state
            .insert(policy_key().clone(), encoded);
        if let Some(encoded_status) = encoded_status {
            state_transaction
                .world
                .smart_contract_state
                .insert(status_key().clone(), encoded_status);
        }
        Ok(())
    }
}

impl Execute for SubmitSorafsOrderbookOrder {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let order = decode_order_request_v1(&self.order_payload).map_err(|error| {
            invalid_parameter(format!(
                "invalid canonical orderbook order payload: {error}"
            ))
        })?;
        verify_order_request_signature_v1(&order)
            .map_err(|error| invalid_parameter(format!("invalid order signature: {error}")))?;
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        validate_order_policy(&order, &policy, now)?;
        let owner = canonical_owner(&order.owner_account, authority)?;
        ensure_payload_signer(authority, &order.signature)?;
        if order.side == OrderSideV1::Ask
            && !owner_has_registered_provider(state_transaction.world(), &owner)
        {
            return Err(invalid_parameter(
                "ask order owner has no registered SoraFS provider binding",
            ));
        }
        if read_order(state_transaction.world(), order.order_id)?.is_some() {
            return Err(invalid_parameter(format!(
                "order {} is already recorded",
                hex::encode(order.order_id)
            )));
        }
        ensure_nonce_advances(state_transaction.world(), &owner, order.nonce)?;
        let mut status = active_status(state_transaction, now)?;
        status.open_orders = status
            .open_orders
            .checked_add(1)
            .ok_or_else(|| corrupt_state("orderbook open-order counter overflow"))?;
        status.updated_at_unix = now;

        let record = OrderbookOrderRecord {
            order_id: order.order_id,
            owner: owner.clone(),
            canonical_order: self.order_payload,
            admitted_policy_digest: policy.policy_digest,
            admitted_at_unix: now,
            status: OrderbookOrderStatusV1::Open,
            canonical_cancel: None,
            cancelled_at_unix: None,
            cancelled_policy_digest: None,
        };
        let encoded = encode_state(&record, "orderbook order")?;
        let encoded_status = encode_status(&status)?;
        write_nonce(state_transaction, &owner, order.nonce)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(order_key(order.order_id), encoded);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for CancelSorafsOrderbookOrder {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let cancel = decode_order_cancel_v1(&self.cancel_payload).map_err(|error| {
            invalid_parameter(format!(
                "invalid canonical order cancellation payload: {error}"
            ))
        })?;
        verify_order_cancel_signature_v1(&cancel).map_err(|error| {
            invalid_parameter(format!("invalid cancellation signature: {error}"))
        })?;
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        let owner = canonical_owner(&cancel.owner_account, authority)?;
        ensure_payload_signer(authority, &cancel.signature)?;
        let mut record =
            read_order(state_transaction.world(), cancel.order_id)?.ok_or_else(|| {
                invalid_parameter(format!("unknown order {}", hex::encode(cancel.order_id)))
            })?;
        if record.owner.subject_id() != owner.subject_id() {
            return Err(invalid_parameter(
                "order cancellation owner does not match stored order owner",
            ));
        }
        if record.status != OrderbookOrderStatusV1::Open {
            return Err(invalid_parameter("order is already cancelled"));
        }
        ensure_nonce_advances(state_transaction.world(), &owner, cancel.nonce)?;

        let stored_order = decode_order_request_v1(&record.canonical_order)
            .map_err(|error| corrupt_state(format!("invalid stored order: {error}")))?;
        match cancel.reason {
            OrderCancelReasonV1::Expired if now <= stored_order.expiry_unix => {
                return Err(invalid_parameter(
                    "expired cancellation reason is invalid before order expiry",
                ));
            }
            OrderCancelReasonV1::Governance
                if !has_permission(state_transaction, authority, "CanSetSorafsPricing") =>
            {
                return Err(invalid_parameter(
                    "governance cancellation reason requires CanSetSorafsPricing",
                ));
            }
            _ => {}
        }

        let mut status = active_status(state_transaction, now)?;
        status.open_orders = status
            .open_orders
            .checked_sub(1)
            .ok_or_else(|| corrupt_state("orderbook open-order counter underflow"))?;
        status.cancelled_orders = status
            .cancelled_orders
            .checked_add(1)
            .ok_or_else(|| corrupt_state("orderbook cancelled-order counter overflow"))?;
        status.updated_at_unix = now;

        record.status = OrderbookOrderStatusV1::Cancelled;
        record.canonical_cancel = Some(self.cancel_payload);
        record.cancelled_at_unix = Some(now);
        record.cancelled_policy_digest = Some(policy.policy_digest);
        let encoded = encode_state(&record, "orderbook order")?;
        let encoded_status = encode_status(&status)?;
        write_nonce(state_transaction, &owner, cancel.nonce)?;
        state_transaction
            .world
            .smart_contract_state
            .insert(order_key(cancel.order_id), encoded);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

impl Execute for RecordSorafsOrderbookSettlementReceipt {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        require_permission(
            state_transaction,
            authority,
            "CanCompleteSorafsReplicationOrder",
        )?;
        let receipt = decode_settlement_receipt_v1(&self.receipt_payload).map_err(|error| {
            invalid_parameter(format!("invalid canonical settlement receipt: {error}"))
        })?;
        verify_settlement_receipt_signature_v1(&receipt).map_err(|error| {
            invalid_parameter(format!("invalid settlement receipt signature: {error}"))
        })?;
        let (policy, now) = active_policy(state_transaction, self.policy_digest)?;
        ensure_payload_signer(authority, &receipt.settlement_signature)?;
        if receipt.bytes_delivered > policy.policy.max_receipt_bytes {
            return Err(invalid_parameter(format!(
                "settlement receipt covers {} bytes; governed maximum is {}",
                receipt.bytes_delivered, policy.policy.max_receipt_bytes
            )));
        }
        if receipt.issued_at_unix > now {
            let skew = receipt.issued_at_unix - now;
            if skew > policy.policy.max_clock_skew_secs {
                return Err(invalid_parameter(format!(
                    "settlement receipt is {skew} seconds in the future"
                )));
            }
        } else {
            let age = now - receipt.issued_at_unix;
            if age > policy.policy.max_receipt_age_secs {
                return Err(invalid_parameter(format!(
                    "settlement receipt age {age} exceeds governed maximum {} seconds",
                    policy.policy.max_receipt_age_secs
                )));
            }
        }
        if read_receipt(state_transaction.world(), receipt.receipt_id)?.is_some() {
            return Err(invalid_parameter(format!(
                "settlement receipt {} is already recorded",
                hex::encode(receipt.receipt_id)
            )));
        }

        let existing_index = read_receipt_index(state_transaction.world(), receipt.channel_id)?;
        let is_new_channel = existing_index.is_none();
        let mut index = existing_index.unwrap_or_else(|| OrderbookSettlementIndexRecord {
            channel_id: receipt.channel_id,
            trade_id: receipt.trade_id,
            ranges: Vec::new(),
        });
        if index.trade_id != receipt.trade_id {
            return Err(invalid_parameter(
                "settlement receipt trade does not match the channel replay index",
            ));
        }
        if index.ranges.len() >= policy.policy.max_receipts_per_channel as usize {
            return Err(invalid_parameter(
                "settlement channel reached the governed receipt-count ceiling",
            ));
        }
        if let Some(existing) = index.ranges.iter().find(|existing| {
            receipt.range.start < existing.end && existing.start < receipt.range.end
        }) {
            return Err(invalid_parameter(format!(
                "settlement receipt range overlaps receipt {}",
                hex::encode(existing.receipt_id)
            )));
        }
        index.ranges.push(OrderbookSettlementRangeRecord {
            receipt_id: receipt.receipt_id,
            start: receipt.range.start,
            end: receipt.range.end,
            issued_at_unix: receipt.issued_at_unix,
        });
        index
            .ranges
            .sort_by_key(|range| (range.start, range.end, range.receipt_id));
        validate_receipt_index(&index, receipt.channel_id)?;

        let record = OrderbookSettlementReceiptRecord {
            receipt_id: receipt.receipt_id,
            channel_id: receipt.channel_id,
            trade_id: receipt.trade_id,
            canonical_receipt: self.receipt_payload,
            admitted_policy_digest: policy.policy_digest,
            admitted_at_unix: now,
            recorded_by: authority.clone(),
        };
        let encoded_record = encode_state(&record, "orderbook settlement receipt")?;
        let encoded_index = encode_state(&index, "orderbook settlement receipt index")?;
        let mut status = active_status(state_transaction, now)?;
        status.settlement_receipts = status
            .settlement_receipts
            .checked_add(1)
            .ok_or_else(|| corrupt_state("orderbook settlement-receipt counter overflow"))?;
        if is_new_channel {
            status.settlement_channels = status
                .settlement_channels
                .checked_add(1)
                .ok_or_else(|| corrupt_state("orderbook settlement-channel counter overflow"))?;
        }
        status.updated_at_unix = now;
        let encoded_status = encode_status(&status)?;

        let escrow_id = orderbook_settlement_escrow_id(receipt.channel_id);
        let escrow = state_transaction
            .world
            .asset_escrows
            .get(&escrow_id)
            .cloned()
            .ok_or_else(|| {
                invalid_parameter(
                    "settlement receipt channel has no funded authoritative asset lock",
                )
            })?;
        if escrow.asset_definition != state_transaction.gov.sorafs_pin_fee_asset_id {
            return Err(invalid_parameter(
                "settlement asset lock does not use the configured SoraFS XOR fee asset",
            ));
        }
        let provider = escrow.buyer.as_ref().ok_or_else(|| {
            invalid_parameter("settlement asset lock has no provider destination")
        })?;
        if !owner_has_registered_provider(state_transaction.world(), provider) {
            return Err(invalid_parameter(
                "settlement asset-lock destination has no registered SoraFS provider binding",
            ));
        }
        let fee_recipient = state_transaction
            .gov
            .sorafs_pin_fee_treasury_account
            .clone();
        super::escrow::settle_orderbook_asset_lock(
            state_transaction,
            &escrow_id,
            authority,
            &fee_recipient,
            Numeric::new(receipt.provider_credit.as_micro(), 6),
            Numeric::new(receipt.fee_amount.as_micro(), 6),
        )
        .map_err(|error| {
            invalid_parameter(format!("settlement asset-lock mutation failed: {error}"))
        })?;
        state_transaction
            .world
            .smart_contract_state
            .insert(receipt_index_key(receipt.channel_id), encoded_index);
        state_transaction
            .world
            .smart_contract_state
            .insert(receipt_key(receipt.receipt_id), encoded_record);
        state_transaction
            .world
            .smart_contract_state
            .insert(status_key().clone(), encoded_status);
        Ok(())
    }
}

fn query_failure(error: impl core::fmt::Display) -> QueryExecutionFail {
    QueryExecutionFail::Conversion(error.to_string())
}

fn checked_query_limit(limit: u32) -> Result<usize, QueryExecutionFail> {
    if !(1..=ORDERBOOK_QUERY_MAX_ITEMS_V1).contains(&limit) {
        return Err(QueryExecutionFail::Conversion(format!(
            "SoraFS orderbook query limit {limit} is outside 1..={ORDERBOOK_QUERY_MAX_ITEMS_V1}"
        )));
    }
    usize::try_from(limit).map_err(|_| {
        QueryExecutionFail::Conversion("SoraFS orderbook query limit conversion failed".to_owned())
    })
}

fn query_order_page(
    query: &FindSorafsOrderbookOrders,
    world: &impl WorldReadOnly,
) -> Result<OrderbookOrderPageV1, QueryExecutionFail> {
    let limit = checked_query_limit(query.limit)?;
    let start = order_key(query.after_order_id.unwrap_or([0; 32]));
    let mut orders = Vec::with_capacity(limit.saturating_add(1));
    for (key, payload) in world.smart_contract_state().range(start..) {
        if !key.to_string().starts_with(ORDER_STATE_KEY_PREFIX) {
            break;
        }
        let candidate: OrderbookOrderRecord =
            decode_state(payload, "orderbook order").map_err(query_failure)?;
        if order_key(candidate.order_id) != *key {
            return Err(QueryExecutionFail::Conversion(
                "authoritative orderbook order key does not match its record".to_owned(),
            ));
        }
        if query
            .after_order_id
            .is_some_and(|cursor| candidate.order_id <= cursor)
        {
            continue;
        }
        let record = read_order(world, candidate.order_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "authoritative orderbook order disappeared during read".to_owned(),
                )
            })?;
        if query.status.is_some_and(|status| record.status != status) {
            continue;
        }
        orders.push(record);
        if orders.len() > limit {
            break;
        }
    }
    let has_more = orders.len() > limit;
    if has_more {
        orders.pop();
    }
    let next_after_order_id = if has_more {
        Some(
            orders
                .last()
                .ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "orderbook page cursor invariant failed".to_owned(),
                    )
                })?
                .order_id,
        )
    } else {
        None
    };
    Ok(OrderbookOrderPageV1 {
        orders,
        has_more,
        next_after_order_id,
    })
}

fn query_receipt_page(
    query: &FindSorafsOrderbookReceipts,
    world: &impl WorldReadOnly,
) -> Result<OrderbookSettlementReceiptPageV1, QueryExecutionFail> {
    let limit = checked_query_limit(query.limit)?;
    let start = receipt_key(query.after_receipt_id.unwrap_or([0; 32]));
    let mut receipts = Vec::with_capacity(limit.saturating_add(1));
    for (key, payload) in world.smart_contract_state().range(start..) {
        if !key.to_string().starts_with(RECEIPT_STATE_KEY_PREFIX) {
            break;
        }
        let candidate: OrderbookSettlementReceiptRecord =
            decode_state(payload, "orderbook settlement receipt").map_err(query_failure)?;
        if receipt_key(candidate.receipt_id) != *key {
            return Err(QueryExecutionFail::Conversion(
                "authoritative orderbook receipt key does not match its record".to_owned(),
            ));
        }
        if query
            .after_receipt_id
            .is_some_and(|cursor| candidate.receipt_id <= cursor)
        {
            continue;
        }
        let record = read_receipt(world, candidate.receipt_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Conversion(
                    "authoritative orderbook receipt disappeared during read".to_owned(),
                )
            })?;
        if query
            .channel_id
            .is_some_and(|channel_id| record.channel_id != channel_id)
        {
            continue;
        }
        receipts.push(record);
        if receipts.len() > limit {
            break;
        }
    }
    let has_more = receipts.len() > limit;
    if has_more {
        receipts.pop();
    }
    let next_after_receipt_id = if has_more {
        Some(
            receipts
                .last()
                .ok_or_else(|| {
                    QueryExecutionFail::Conversion(
                        "orderbook receipt-page cursor invariant failed".to_owned(),
                    )
                })?
                .receipt_id,
        )
    } else {
        None
    };
    Ok(OrderbookSettlementReceiptPageV1 {
        receipts,
        has_more,
        next_after_receipt_id,
    })
}

impl ValidSingularQuery for FindSorafsOrderbookPolicy {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookAdmissionPolicyRecord, QueryExecutionFail> {
        read_policy(state_ro.world())
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsOrderbookPolicy))
    }
}

impl ValidSingularQuery for FindSorafsOrderbookOrderById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookOrderRecord, QueryExecutionFail> {
        read_order(state_ro.world(), self.order_id)
            .map_err(query_failure)?
            .ok_or_else(|| QueryExecutionFail::Find(FindError::SorafsOrderbookOrder(self.order_id)))
    }
}

impl ValidSingularQuery for FindSorafsOrderbookCancellationByOrderId {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookCancellationRecord, QueryExecutionFail> {
        let order = read_order(state_ro.world(), self.order_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsOrderbookCancellation(self.order_id))
            })?;
        if order.status != OrderbookOrderStatusV1::Cancelled {
            return Err(QueryExecutionFail::Conversion(format!(
                "authoritative SoraFS orderbook order {} is not cancelled",
                hex::encode(self.order_id)
            )));
        }
        let (Some(canonical_cancel), Some(cancelled_at_unix), Some(cancelled_policy_digest)) = (
            order.canonical_cancel,
            order.cancelled_at_unix,
            order.cancelled_policy_digest,
        ) else {
            return Err(QueryExecutionFail::Conversion(
                "authoritative SoraFS cancellation state is inconsistent".to_owned(),
            ));
        };
        Ok(OrderbookCancellationRecord {
            order_id: order.order_id,
            owner: order.owner,
            canonical_cancel,
            cancelled_at_unix,
            cancelled_policy_digest,
        })
    }
}

impl ValidSingularQuery for FindSorafsOrderbookReceiptById {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookSettlementReceiptRecord, QueryExecutionFail> {
        read_receipt(state_ro.world(), self.receipt_id)
            .map_err(query_failure)?
            .ok_or_else(|| {
                QueryExecutionFail::Find(FindError::SorafsOrderbookReceipt(self.receipt_id))
            })
    }
}

impl ValidSingularQuery for FindSorafsOrderbookStatus {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookLedgerStatusV1, QueryExecutionFail> {
        let policy = read_policy(state_ro.world()).map_err(query_failure)?;
        let status = read_status(state_ro.world()).map_err(query_failure)?;
        match (policy, status) {
            (Some(_), Some(status)) => Ok(status),
            (None, None) => Err(QueryExecutionFail::Find(FindError::SorafsOrderbookStatus)),
            _ => Err(QueryExecutionFail::Conversion(
                "authoritative SoraFS orderbook policy/status state is inconsistent".to_owned(),
            )),
        }
    }
}

impl ValidSingularQuery for FindSorafsOrderbookOrders {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookOrderPageV1, QueryExecutionFail> {
        if read_policy(state_ro.world())
            .map_err(query_failure)?
            .is_none()
            || read_status(state_ro.world())
                .map_err(query_failure)?
                .is_none()
        {
            return Err(QueryExecutionFail::Find(FindError::SorafsOrderbookPolicy));
        }
        query_order_page(self, state_ro.world())
    }
}

impl ValidSingularQuery for FindSorafsOrderbookReceipts {
    fn execute(
        &self,
        state_ro: &impl crate::state::StateReadOnly,
    ) -> Result<OrderbookSettlementReceiptPageV1, QueryExecutionFail> {
        if read_policy(state_ro.world())
            .map_err(query_failure)?
            .is_none()
            || read_status(state_ro.world())
                .map_err(query_failure)?
                .is_none()
        {
            return Err(QueryExecutionFail::Find(FindError::SorafsOrderbookPolicy));
        }
        query_receipt_page(self, state_ro.world())
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, KeyPair, PrivateKey, Signature};
    use iroha_data_model::{
        IntoKeyValue, Registrable,
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::escrow::OpenAssetLock,
        permission::{Permission, Permissions},
        sorafs::{
            capacity::ProviderId,
            orderbook::{
                ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyV1,
                OrderbookOrderStatusV1, orderbook_settlement_escrow_id,
            },
        },
    };
    use iroha_primitives::{bigint::BigInt, json::Json};
    use nonzero_ext::nonzero;
    use sorafs_manifest::{
        deal::XorAmount,
        orderbook::{
            ByteRangeV1, ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1,
            OrderCancelReasonV1, OrderCancelV1, OrderRequestV1, OrderSideV1, OrderTierV1,
            OrderbookSignatureV1, SETTLEMENT_RECEIPT_VERSION_V1, SettlementReceiptV1,
            derive_orderbook_order_id_v1, order_cancel_signature_digest_v1,
            order_request_signature_digest_v1, settlement_receipt_signature_digest_v1,
        },
        provider_advert::SignatureAlgorithm,
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    const NOW: u64 = 10_000;

    fn keypair(seed: u8) -> KeyPair {
        let private = PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32])
            .expect("valid deterministic Ed25519 seed");
        KeyPair::from_private_key(private).expect("derive deterministic keypair")
    }

    fn account(keypair: &KeyPair) -> AccountId {
        AccountId::new(keypair.public_key().clone())
    }

    fn empty_signature(keypair: &KeyPair) -> OrderbookSignatureV1 {
        let (_, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("valid fixture public key");
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        }
    }

    fn sign_digest(keypair: &KeyPair, digest: [u8; 32]) -> Vec<u8> {
        Signature::try_new(keypair.private_key(), &digest)
            .expect("sign fixture digest")
            .payload()
            .to_vec()
    }

    fn sign_order(mut order: OrderRequestV1, keypair: &KeyPair) -> OrderRequestV1 {
        order.signature = empty_signature(keypair);
        let digest = order_request_signature_digest_v1(&order).expect("digest order");
        order.signature.signature = sign_digest(keypair, digest);
        verify_order_request_signature_v1(&order).expect("signed order verifies");
        order
    }

    fn sign_cancel(mut cancel: OrderCancelV1, keypair: &KeyPair) -> OrderCancelV1 {
        cancel.signature = empty_signature(keypair);
        let digest = order_cancel_signature_digest_v1(&cancel).expect("digest cancellation");
        cancel.signature.signature = sign_digest(keypair, digest);
        verify_order_cancel_signature_v1(&cancel).expect("signed cancellation verifies");
        cancel
    }

    fn sign_receipt(mut receipt: SettlementReceiptV1, keypair: &KeyPair) -> SettlementReceiptV1 {
        receipt.settlement_signature = empty_signature(keypair);
        let digest = settlement_receipt_signature_digest_v1(&receipt).expect("digest receipt");
        receipt.settlement_signature.signature = sign_digest(keypair, digest);
        verify_settlement_receipt_signature_v1(&receipt).expect("signed receipt verifies");
        receipt
    }

    fn policy() -> OrderbookAdmissionPolicyV1 {
        OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0xA5; 32],
            paused: false,
            min_order_gib: 2,
            max_order_gib: 1_024,
            price_tick_micro_xor: 10,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 200,
            max_order_lifetime_secs: 3_600,
            max_receipt_age_secs: 300,
            max_clock_skew_secs: 5,
            max_receipt_bytes: 1_024,
            max_receipts_per_channel: 2,
        }
    }

    fn order(keypair: &KeyPair, nonce: u64) -> OrderRequestV1 {
        let owner_account = account(keypair).to_string().into_bytes();
        sign_order(
            OrderRequestV1 {
                version: ORDERBOOK_ORDER_VERSION_V1,
                order_id: derive_orderbook_order_id_v1(&owner_account, nonce),
                side: OrderSideV1::Bid,
                tier: OrderTierV1::Hot,
                price_per_gib: XorAmount::from_micro(100),
                quantity_gib: 10,
                remaining_gib: 10,
                owner_account,
                expiry_unix: NOW + 100,
                nonce,
                maker_fee_bps: 10,
                taker_fee_bps: 20,
                signature: empty_signature(keypair),
            },
            keypair,
        )
    }

    fn cancel(keypair: &KeyPair, order_id: [u8; 32], nonce: u64) -> OrderCancelV1 {
        sign_cancel(
            OrderCancelV1 {
                version: ORDERBOOK_CANCEL_VERSION_V1,
                order_id,
                owner_account: account(keypair).to_string().into_bytes(),
                reason: OrderCancelReasonV1::OwnerRequested,
                nonce,
                signature: empty_signature(keypair),
            },
            keypair,
        )
    }

    fn receipt(
        keypair: &KeyPair,
        receipt_id: u8,
        channel_id: u8,
        trade_id: u8,
        start: u64,
        end: u64,
    ) -> SettlementReceiptV1 {
        let length = end - start;
        sign_receipt(
            SettlementReceiptV1 {
                version: SETTLEMENT_RECEIPT_VERSION_V1,
                receipt_id: [receipt_id; 32],
                channel_id: [channel_id; 32],
                trade_id: [trade_id; 32],
                range: ByteRangeV1 { start, end },
                chunk_hash: [0xC1; 32],
                bytes_delivered: length,
                xor_debited: XorAmount::from_micro(100),
                provider_credit: XorAmount::from_micro(90),
                fee_amount: XorAmount::from_micro(10),
                issued_at_unix: NOW - 1,
                settlement_signature: empty_signature(keypair),
            },
            keypair,
        )
    }

    fn block_header() -> BlockHeader {
        BlockHeader::new(nonzero!(1_u64), None, None, None, NOW * 1_000, 0)
    }

    fn state_with_accounts(keypairs: &[&KeyPair]) -> State {
        let mut world = World::new();
        for keypair in keypairs {
            let id = account(keypair);
            let (id, value) = Account::new(id.clone()).build(&id).into_key_value();
            world.accounts.insert(id, value);
        }
        let authority = account(keypairs[0]);
        let mut permissions = Permissions::new();
        for permission in ["CanSetSorafsPricing", "CanCompleteSorafsReplicationOrder"] {
            permissions.insert(Permission::new(permission.to_owned(), Json::new(())));
        }
        world.account_permissions.insert(authority, permissions);
        State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn settlement_asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("sorafs", "universal").expect("settlement domain"),
            "xor".parse().expect("settlement asset name"),
        )
    }

    fn state_with_settlement_accounts(
        settlement: &KeyPair,
        buyer: &KeyPair,
        provider: &KeyPair,
        treasury: &KeyPair,
        buyer_balance_micro: u128,
    ) -> State {
        let settlement_id = account(settlement);
        let buyer_id = account(buyer);
        let provider_id = account(provider);
        let treasury_id = account(treasury);
        let asset_definition = settlement_asset_definition();
        let domain = Domain::new(asset_definition.domain().clone()).build(&buyer_id);
        let definition = AssetDefinition::numeric(asset_definition.clone())
            .with_name("XOR".to_owned())
            .build(&buyer_id);
        let buyer_asset = Asset::new(
            AssetId::of(asset_definition.clone(), buyer_id.clone()),
            Numeric::new(buyer_balance_micro, 6),
        );
        let mut world = World::with_assets(
            [domain],
            [
                Account::new(settlement_id.clone()).build(&settlement_id),
                Account::new(buyer_id.clone()).build(&buyer_id),
                Account::new(provider_id.clone()).build(&provider_id),
                Account::new(treasury_id.clone()).build(&treasury_id),
            ],
            [definition],
            [buyer_asset],
            [],
        );
        let mut permissions = Permissions::new();
        for permission in ["CanSetSorafsPricing", "CanCompleteSorafsReplicationOrder"] {
            permissions.insert(Permission::new(permission.to_owned(), Json::new(())));
        }
        world.account_permissions.insert(settlement_id, permissions);
        let mut state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.gov.sorafs_pin_fee_asset_id = asset_definition;
        state.gov.sorafs_pin_fee_treasury_account = treasury_id;
        state
    }

    fn open_settlement_lock(
        state_transaction: &mut StateTransaction<'_, '_>,
        buyer: &AccountId,
        provider: &AccountId,
        settlement: &AccountId,
        channel_id: [u8; 32],
        amount_micro: u128,
    ) {
        OpenAssetLock::with_options(
            orderbook_settlement_escrow_id(channel_id),
            settlement_asset_definition(),
            provider.clone(),
            Numeric::new(amount_micro, 6),
            Some(settlement.clone()),
            None,
            Vec::new(),
        )
        .execute(buyer, state_transaction)
        .expect("open funded settlement lock");
        state_transaction
            .world
            .provider_owners
            .insert(ProviderId::new([0x71; 32]), provider.clone());
    }

    fn seed_test_call_hash(state_transaction: &mut StateTransaction<'_, '_>, byte: u8) {
        state_transaction.tx_call_hash = Some(Hash::prehashed([byte; Hash::LENGTH]));
    }

    fn asset_balance(state_transaction: &StateTransaction<'_, '_>, account: &AccountId) -> Numeric {
        state_transaction
            .world
            .assets
            .get(&AssetId::of(settlement_asset_definition(), account.clone()))
            .map(|value| value.as_ref().clone())
            .unwrap_or_else(Numeric::zero)
    }

    fn assert_no_receipt_status_mutation(state_transaction: &StateTransaction<'_, '_>) {
        let status = read_status(state_transaction.world())
            .expect("read orderbook status")
            .expect("active policy status");
        assert_eq!(status.settlement_receipts, 0);
        assert_eq!(status.settlement_channels, 0);
    }

    fn assert_order_status(
        state_transaction: &StateTransaction<'_, '_>,
        open_orders: u64,
        cancelled_orders: u64,
    ) {
        let status = read_status(state_transaction.world())
            .expect("read orderbook status")
            .expect("active policy status");
        assert_eq!(status.open_orders, open_orders);
        assert_eq!(status.cancelled_orders, cancelled_orders);
    }

    fn activate_policy(
        state_transaction: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> [u8; 32] {
        let policy = policy();
        let digest = policy.digest().expect("digest policy");
        SetSorafsOrderbookPolicy::new(policy)
            .execute(authority, state_transaction)
            .expect("activate policy");
        digest
    }

    fn encode<T: norito::core::NoritoSerialize>(value: &T) -> Vec<u8> {
        norito::to_bytes(value).expect("encode canonical fixture")
    }

    #[test]
    fn policy_activation_is_permissioned_and_exactly_chained() {
        let operator = keypair(0x11);
        let authority = account(&operator);
        let state = state_with_accounts(&[&operator]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();

        let first = policy();
        let first_digest = first.digest().expect("digest first policy");
        SetSorafsOrderbookPolicy::new(first.clone())
            .execute(&authority, &mut stx)
            .expect("first policy activates");
        let stored = read_policy(stx.world())
            .expect("read policy")
            .expect("policy");
        assert_eq!(stored.policy_digest, first_digest);
        assert_eq!(stored.activated_at_unix, NOW);

        for invalid in {
            let mut gap = first.clone();
            gap.revision = 3;
            gap.predecessor_policy_digest = Some(first_digest);
            let mut branch = first.clone();
            branch.revision = 2;
            branch.predecessor_policy_digest = Some([0x44; 32]);
            let mut market_swap = first.clone();
            market_swap.revision = 2;
            market_swap.predecessor_policy_digest = Some(first_digest);
            market_swap.market_id = [0xB5; 32];
            [gap, branch, market_swap]
        } {
            assert!(
                SetSorafsOrderbookPolicy::new(invalid.clone())
                    .execute(&authority, &mut stx)
                    .is_err()
            );
            assert_eq!(
                read_policy(stx.world())
                    .expect("read unchanged policy")
                    .expect("policy")
                    .policy_digest,
                first_digest
            );
        }

        let mut second = first;
        second.revision = 2;
        second.predecessor_policy_digest = Some(first_digest);
        second.paused = true;
        SetSorafsOrderbookPolicy::new(second.clone())
            .execute(&authority, &mut stx)
            .expect("exact successor activates");
        assert_eq!(
            read_policy(stx.world())
                .expect("read successor")
                .expect("policy")
                .policy_digest,
            second.digest().expect("digest successor")
        );
    }

    #[test]
    fn policy_activation_rejects_missing_permission_without_state_mutation() {
        let operator = keypair(0x12);
        let authority = account(&operator);
        let state = state_with_accounts(&[&operator]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        stx.world
            .account_permissions
            .get_mut(&authority)
            .expect("permissions")
            .retain(|permission| permission.name() != "CanSetSorafsPricing");

        assert!(
            SetSorafsOrderbookPolicy::new(policy())
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(read_policy(stx.world()).expect("read policy").is_none());
    }

    #[test]
    fn signed_order_submission_persists_authoritative_record_and_nonce() {
        let buyer = keypair(0x21);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let order = order(&buyer, 1);

        SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
            .execute(&authority, &mut stx)
            .expect("submit order");

        let stored = read_order(stx.world(), order.order_id)
            .expect("read order")
            .expect("stored order");
        assert_eq!(stored.owner, authority);
        assert_eq!(stored.status, OrderbookOrderStatusV1::Open);
        assert_eq!(stored.admitted_policy_digest, policy_digest);
        assert_eq!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .expect("stored nonce")
                .highest_nonce,
            1
        );
    }

    #[test]
    fn order_rejects_malformed_noncanonical_and_oversized_payloads_atomically() {
        let buyer = keypair(0x22);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let order = order(&buyer, 1);
        let mut noncanonical = encode(&order);
        noncanonical.push(0);

        for payload in [vec![0xFF; 32], noncanonical, vec![0; 64 * 1024 + 1]] {
            assert!(
                SubmitSorafsOrderbookOrder::new(payload, policy_digest)
                    .execute(&authority, &mut stx)
                    .is_err()
            );
            assert!(
                read_order(stx.world(), order.order_id)
                    .expect("read order")
                    .is_none()
            );
            assert!(
                read_nonce(stx.world(), &authority)
                    .expect("read nonce")
                    .is_none()
            );
        }
    }

    #[test]
    fn order_policy_failures_do_not_advance_nonce() {
        let buyer = keypair(0x23);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let base = order(&buyer, 1);

        let mut candidates = Vec::new();
        let mut candidate = base.clone();
        candidate.remaining_gib = 9;
        candidates.push(candidate);
        let mut candidate = base.clone();
        candidate.quantity_gib = 1;
        candidate.remaining_gib = 1;
        candidates.push(candidate);
        let mut candidate = base.clone();
        candidate.price_per_gib = XorAmount::from_micro(101);
        candidates.push(candidate);
        let mut candidate = base.clone();
        candidate.maker_fee_bps = 101;
        candidates.push(candidate);
        let mut candidate = base.clone();
        candidate.expiry_unix = NOW;
        candidates.push(candidate);
        let mut candidate = base.clone();
        candidate.expiry_unix = NOW + policy().max_order_lifetime_secs + 1;
        candidates.push(candidate);

        for candidate in candidates {
            let candidate = sign_order(candidate, &buyer);
            assert!(
                SubmitSorafsOrderbookOrder::new(encode(&candidate), policy_digest)
                    .execute(&authority, &mut stx)
                    .is_err()
            );
            assert!(
                read_order(stx.world(), base.order_id)
                    .expect("read order")
                    .is_none()
            );
            assert!(
                read_nonce(stx.world(), &authority)
                    .expect("read nonce")
                    .is_none()
            );
        }

        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&base), [0x99; 32])
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .is_none()
        );
        assert_order_status(&stx, 0, 0);
    }

    #[test]
    fn order_rejects_wrong_owner_signer_and_tampering_atomically() {
        let buyer = keypair(0x24);
        let attacker = keypair(0x25);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer, &attacker]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let base = order(&buyer, 1);

        let wrong_signer = sign_order(base.clone(), &attacker);
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&wrong_signer), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        let wrong_owner = order(&attacker, 1);
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&wrong_owner), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        let mut tampered = base.clone();
        tampered.quantity_gib += 1;
        tampered.remaining_gib += 1;
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&tampered), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .is_none()
        );
        assert!(
            read_order(stx.world(), base.order_id)
                .expect("read order")
                .is_none()
        );
    }

    #[test]
    fn ask_requires_registered_provider_owner_binding() {
        let provider = keypair(0x26);
        let authority = account(&provider);
        let state = state_with_accounts(&[&provider]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let mut ask = order(&provider, 1);
        ask.side = OrderSideV1::Ask;
        let ask = sign_order(ask, &provider);

        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&ask), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        stx.world
            .provider_owners
            .insert(ProviderId::new([0x61; 32]), authority.clone());
        SubmitSorafsOrderbookOrder::new(encode(&ask), policy_digest)
            .execute(&authority, &mut stx)
            .expect("registered provider can submit ask");
    }

    #[test]
    fn order_replay_and_nonce_rollback_are_rejected() {
        let buyer = keypair(0x27);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let first = order(&buyer, 1);
        SubmitSorafsOrderbookOrder::new(encode(&first), policy_digest)
            .execute(&authority, &mut stx)
            .expect("first order");
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&first), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        let third = order(&buyer, 3);
        SubmitSorafsOrderbookOrder::new(encode(&third), policy_digest)
            .execute(&authority, &mut stx)
            .expect("higher nonce order");
        let second = order(&buyer, 2);
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&second), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_order(stx.world(), second.order_id)
                .expect("read order")
                .is_none()
        );
        assert_eq!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .expect("nonce")
                .highest_nonce,
            3
        );
    }

    #[test]
    fn typed_order_queries_return_policy_cancellation_status_and_cursor_pages() {
        let buyer = keypair(0x28);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let first = order(&buyer, 1);
        let second = order(&buyer, 2);
        let third = order(&buyer, 3);
        for candidate in [&first, &second, &third] {
            SubmitSorafsOrderbookOrder::new(encode(candidate), policy_digest)
                .execute(&authority, &mut stx)
                .expect("submit query fixture order");
        }
        let cancellation = cancel(&buyer, second.order_id, 4);
        CancelSorafsOrderbookOrder::new(encode(&cancellation), policy_digest)
            .execute(&authority, &mut stx)
            .expect("cancel query fixture order");

        let found_policy = FindSorafsOrderbookPolicy
            .execute(&stx)
            .expect("query active policy");
        assert_eq!(found_policy.policy_digest, policy_digest);
        let found_order = FindSorafsOrderbookOrderById::new(first.order_id)
            .execute(&stx)
            .expect("query order by id");
        assert_eq!(found_order.order_id, first.order_id);
        let found_cancellation = FindSorafsOrderbookCancellationByOrderId::new(second.order_id)
            .execute(&stx)
            .expect("query cancellation by order id");
        assert_eq!(found_cancellation.canonical_cancel, encode(&cancellation));
        assert_eq!(found_cancellation.cancelled_policy_digest, policy_digest);
        assert!(
            FindSorafsOrderbookCancellationByOrderId::new(first.order_id)
                .execute(&stx)
                .is_err(),
            "an open order must not be exposed as a cancellation"
        );

        let status = FindSorafsOrderbookStatus
            .execute(&stx)
            .expect("query orderbook status");
        assert_eq!(status.open_orders, 2);
        assert_eq!(status.cancelled_orders, 1);
        assert_eq!(status.settlement_receipts, 0);
        assert_eq!(status.settlement_channels, 0);

        let first_page = FindSorafsOrderbookOrders::new(None, None, 2)
            .execute(&stx)
            .expect("query first order page");
        assert_eq!(first_page.orders.len(), 2);
        assert!(first_page.has_more);
        let cursor = first_page.next_after_order_id.expect("next cursor");
        let second_page = FindSorafsOrderbookOrders::new(None, Some(cursor), 2)
            .execute(&stx)
            .expect("query second order page");
        assert_eq!(second_page.orders.len(), 1);
        assert!(!second_page.has_more);
        assert!(second_page.next_after_order_id.is_none());

        let mut returned_ids = first_page
            .orders
            .iter()
            .chain(&second_page.orders)
            .map(|record| record.order_id)
            .collect::<Vec<_>>();
        assert!(returned_ids.windows(2).all(|ids| ids[0] < ids[1]));
        returned_ids.sort_unstable();
        let mut expected_ids = vec![first.order_id, second.order_id, third.order_id];
        expected_ids.sort_unstable();
        assert_eq!(returned_ids, expected_ids);

        let cancelled_page =
            FindSorafsOrderbookOrders::new(Some(OrderbookOrderStatusV1::Cancelled), None, 10)
                .execute(&stx)
                .expect("query cancelled order page");
        assert_eq!(cancelled_page.orders.len(), 1);
        assert_eq!(cancelled_page.orders[0].order_id, second.order_id);
    }

    #[test]
    fn typed_queries_reject_not_found_and_invalid_limits() {
        let operator = keypair(0x29);
        let authority = account(&operator);
        let state = state_with_accounts(&[&operator]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();

        assert_eq!(
            FindSorafsOrderbookPolicy.execute(&stx),
            Err(QueryExecutionFail::Find(FindError::SorafsOrderbookPolicy))
        );
        assert_eq!(
            FindSorafsOrderbookStatus.execute(&stx),
            Err(QueryExecutionFail::Find(FindError::SorafsOrderbookStatus))
        );
        assert_eq!(
            FindSorafsOrderbookOrderById::new([0xE1; 32]).execute(&stx),
            Err(QueryExecutionFail::Find(FindError::SorafsOrderbookOrder(
                [0xE1; 32]
            )))
        );
        assert_eq!(
            FindSorafsOrderbookCancellationByOrderId::new([0xE2; 32]).execute(&stx),
            Err(QueryExecutionFail::Find(
                FindError::SorafsOrderbookCancellation([0xE2; 32])
            ))
        );
        assert_eq!(
            FindSorafsOrderbookReceiptById::new([0xE3; 32]).execute(&stx),
            Err(QueryExecutionFail::Find(FindError::SorafsOrderbookReceipt(
                [0xE3; 32]
            )))
        );

        activate_policy(&mut stx, &authority);
        for limit in [0, ORDERBOOK_QUERY_MAX_ITEMS_V1 + 1] {
            assert!(
                FindSorafsOrderbookOrders::new(None, None, limit)
                    .execute(&stx)
                    .is_err()
            );
            assert!(
                FindSorafsOrderbookReceipts::new(None, None, limit)
                    .execute(&stx)
                    .is_err()
            );
        }
        assert!(
            FindSorafsOrderbookOrders::new(None, None, 1)
                .execute(&stx)
                .expect("empty configured orderbook query")
                .orders
                .is_empty()
        );
    }

    #[test]
    fn signed_cancellation_updates_order_and_shared_nonce() {
        let buyer = keypair(0x31);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let order = order(&buyer, 1);
        SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
            .execute(&authority, &mut stx)
            .expect("order");
        let cancel = cancel(&buyer, order.order_id, 2);
        CancelSorafsOrderbookOrder::new(encode(&cancel), policy_digest)
            .execute(&authority, &mut stx)
            .expect("cancel");

        let stored = read_order(stx.world(), order.order_id)
            .expect("read order")
            .expect("order");
        assert_eq!(stored.status, OrderbookOrderStatusV1::Cancelled);
        assert!(stored.canonical_cancel.is_some());
        assert_eq!(stored.cancelled_at_unix, Some(NOW));
        assert_eq!(stored.cancelled_policy_digest, Some(policy_digest));
        assert_eq!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .expect("nonce")
                .highest_nonce,
            2
        );
        assert!(
            CancelSorafsOrderbookOrder::new(encode(&cancel), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
    }

    #[test]
    fn cancellation_rejects_unknown_wrong_owner_wrong_policy_and_stale_nonce() {
        let buyer = keypair(0x32);
        let attacker = keypair(0x33);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer, &attacker]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let order = order(&buyer, 2);
        SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
            .execute(&authority, &mut stx)
            .expect("order");

        let unknown = cancel(&buyer, [0xFE; 32], 3);
        assert!(
            CancelSorafsOrderbookOrder::new(encode(&unknown), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        let wrong_owner = cancel(&attacker, order.order_id, 3);
        assert!(
            CancelSorafsOrderbookOrder::new(encode(&wrong_owner), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        let stale = cancel(&buyer, order.order_id, 1);
        assert!(
            CancelSorafsOrderbookOrder::new(encode(&stale), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        let valid = cancel(&buyer, order.order_id, 3);
        assert!(
            CancelSorafsOrderbookOrder::new(encode(&valid), [0xDD; 32])
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert_eq!(
            read_order(stx.world(), order.order_id)
                .expect("read order")
                .expect("order")
                .status,
            OrderbookOrderStatusV1::Open
        );
        assert_eq!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .expect("nonce")
                .highest_nonce,
            2
        );
        assert_order_status(&stx, 1, 0);
    }

    #[test]
    fn receipt_recording_is_bounded_non_overlapping_and_replay_safe() {
        let settlement = keypair(0x41);
        let buyer = keypair(0x42);
        let provider = keypair(0x43);
        let treasury = keypair(0x44);
        let authority = account(&settlement);
        let buyer_id = account(&buyer);
        let provider_id = account(&provider);
        let treasury_id = account(&treasury);
        let state =
            state_with_settlement_accounts(&settlement, &buyer, &provider, &treasury, 1_000);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xA1);
        let policy_digest = activate_policy(&mut stx, &authority);
        let first = receipt(&settlement, 1, 9, 8, 0, 10);
        open_settlement_lock(
            &mut stx,
            &buyer_id,
            &provider_id,
            &authority,
            first.channel_id,
            1_000,
        );
        RecordSorafsOrderbookSettlementReceipt::new(encode(&first), policy_digest)
            .execute(&authority, &mut stx)
            .expect("first receipt");
        assert!(
            read_receipt(stx.world(), first.receipt_id)
                .expect("read receipt")
                .is_some()
        );
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::new(90_u32, 6));
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::new(10_u32, 6));

        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&first), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        let overlap = receipt(&settlement, 2, 9, 8, 5, 15);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&overlap), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        let wrong_trade = receipt(&settlement, 3, 9, 7, 10, 20);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&wrong_trade), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::new(90_u32, 6));
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::new(10_u32, 6));
        let second = receipt(&settlement, 4, 9, 8, 10, 20);
        RecordSorafsOrderbookSettlementReceipt::new(encode(&second), policy_digest)
            .execute(&authority, &mut stx)
            .expect("second receipt");
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::new(180_u32, 6));
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::new(20_u32, 6));
        let third = receipt(&settlement, 5, 9, 8, 20, 30);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&third), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        let index = read_receipt_index(stx.world(), first.channel_id)
            .expect("read index")
            .expect("index");
        assert_eq!(index.ranges.len(), 2);
        assert_eq!(index.ranges[0].receipt_id, first.receipt_id);
        assert_eq!(index.ranges[1].receipt_id, second.receipt_id);
        assert!(
            read_receipt(stx.world(), third.receipt_id)
                .expect("read receipt")
                .is_none()
        );
        let escrow = stx
            .world
            .asset_escrows
            .get(&orderbook_settlement_escrow_id(first.channel_id))
            .expect("settlement lock");
        assert_eq!(escrow.remaining_amount, Numeric::new(800_u32, 6));
        assert_eq!(
            asset_balance(&stx, &escrow.custody),
            Numeric::new(800_u32, 6)
        );

        let queried = FindSorafsOrderbookReceiptById::new(first.receipt_id)
            .execute(&stx)
            .expect("query receipt by id");
        assert_eq!(queried.receipt_id, first.receipt_id);
        let first_page = FindSorafsOrderbookReceipts::new(Some(first.channel_id), None, 1)
            .execute(&stx)
            .expect("query first receipt page");
        assert_eq!(first_page.receipts.len(), 1);
        assert!(first_page.has_more);
        let second_page = FindSorafsOrderbookReceipts::new(
            Some(first.channel_id),
            first_page.next_after_receipt_id,
            1,
        )
        .execute(&stx)
        .expect("query second receipt page");
        assert_eq!(second_page.receipts.len(), 1);
        assert!(!second_page.has_more);
        let status = FindSorafsOrderbookStatus
            .execute(&stx)
            .expect("query receipt counters");
        assert_eq!(status.settlement_receipts, 2);
        assert_eq!(status.settlement_channels, 1);
    }

    #[test]
    fn receipt_rejects_permission_policy_signer_time_and_canonical_abuse_atomically() {
        let settlement = keypair(0x45);
        let attacker = keypair(0x46);
        let buyer = keypair(0x47);
        let provider = keypair(0x48);
        let treasury = keypair(0x49);
        let authority = account(&settlement);
        let buyer_id = account(&buyer);
        let provider_id = account(&provider);
        let treasury_id = account(&treasury);
        let mut state =
            state_with_settlement_accounts(&settlement, &buyer, &provider, &treasury, 1_000);
        let attacker_id = account(&attacker);
        let (attacker_id, attacker_value) = Account::new(attacker_id.clone())
            .build(&attacker_id)
            .into_key_value();
        state.world.accounts.insert(attacker_id, attacker_value);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xA2);
        let policy_digest = activate_policy(&mut stx, &authority);
        let base = receipt(&settlement, 1, 6, 7, 0, 10);
        open_settlement_lock(
            &mut stx,
            &buyer_id,
            &provider_id,
            &authority,
            base.channel_id,
            1_000,
        );

        let mut noncanonical = encode(&base);
        noncanonical.push(0);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(noncanonical, policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&base), [0x99; 32])
                .execute(&authority, &mut stx)
                .is_err()
        );
        let wrong_signer = sign_receipt(base.clone(), &attacker);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&wrong_signer), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        let mut stale = base.clone();
        stale.issued_at_unix = NOW - policy().max_receipt_age_secs - 1;
        let stale = sign_receipt(stale, &settlement);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&stale), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        let mut future = base.clone();
        future.issued_at_unix = NOW + policy().max_clock_skew_secs + 1;
        let future = sign_receipt(future, &settlement);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&future), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        stx.world
            .account_permissions
            .get_mut(&authority)
            .expect("permissions")
            .retain(|permission| permission.name() != "CanCompleteSorafsReplicationOrder");
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&base), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_receipt(stx.world(), base.receipt_id)
                .expect("read receipt")
                .is_none()
        );
        assert!(
            read_receipt_index(stx.world(), base.channel_id)
                .expect("read index")
                .is_none()
        );
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::zero());
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::zero());
        let escrow = stx
            .world
            .asset_escrows
            .get(&orderbook_settlement_escrow_id(base.channel_id))
            .expect("settlement lock");
        assert_eq!(escrow.remaining_amount, Numeric::new(1_000_u32, 6));
        assert_eq!(
            asset_balance(&stx, &escrow.custody),
            Numeric::new(1_000_u32, 6)
        );
        assert_no_receipt_status_mutation(&stx);
    }

    #[test]
    fn receipt_rejects_misauthorized_expired_unregistered_and_untraced_locks_atomically() {
        let settlement = keypair(0x53);
        let buyer = keypair(0x54);
        let provider = keypair(0x55);
        let treasury = keypair(0x56);
        let authority = account(&settlement);
        let buyer_id = account(&buyer);
        let provider_id = account(&provider);
        let treasury_id = account(&treasury);
        let state =
            state_with_settlement_accounts(&settlement, &buyer, &provider, &treasury, 1_000);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xA6);
        let policy_digest = activate_policy(&mut stx, &authority);
        let candidate = receipt(&settlement, 1, 16, 17, 0, 10);
        open_settlement_lock(
            &mut stx,
            &buyer_id,
            &provider_id,
            &authority,
            candidate.channel_id,
            1_000,
        );
        let escrow_id = orderbook_settlement_escrow_id(candidate.channel_id);
        let custody = stx
            .world
            .asset_escrows
            .get(&escrow_id)
            .expect("settlement lock")
            .custody
            .clone();

        let configured_asset = stx.gov.sorafs_pin_fee_asset_id.clone();
        stx.gov.sorafs_pin_fee_asset_id = AssetDefinitionId::new(
            configured_asset.domain().clone(),
            "not_xor".parse().expect("wrong asset name"),
        );
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        stx.gov.sorafs_pin_fee_asset_id = configured_asset;

        stx.world
            .asset_escrows
            .get_mut(&escrow_id)
            .expect("settlement lock")
            .remaining_amount = Numeric::new(999_u32, 6);
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        stx.world
            .asset_escrows
            .get_mut(&escrow_id)
            .expect("settlement lock")
            .remaining_amount = Numeric::new(1_000_u32, 6);
        stx.world
            .asset_escrows
            .get_mut(&escrow_id)
            .expect("settlement lock")
            .release_authority = Some(buyer_id.clone());
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        {
            let escrow = stx
                .world
                .asset_escrows
                .get_mut(&escrow_id)
                .expect("settlement lock");
            escrow.release_authority = Some(authority.clone());
            escrow.expires_at_ms = Some(NOW * 1_000);
        }
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        stx.world
            .asset_escrows
            .get_mut(&escrow_id)
            .expect("settlement lock")
            .expires_at_ms = None;
        stx.world
            .provider_owners
            .remove(ProviderId::new([0x71; 32]));
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );

        stx.world
            .provider_owners
            .insert(ProviderId::new([0x71; 32]), provider_id.clone());
        stx.tx_call_hash = None;
        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::zero());
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::zero());
        assert_eq!(asset_balance(&stx, &custody), Numeric::new(1_000_u32, 6));
        assert_eq!(
            stx.world
                .asset_escrows
                .get(&escrow_id)
                .expect("settlement lock")
                .remaining_amount,
            Numeric::new(1_000_u32, 6)
        );
        assert!(
            read_receipt(stx.world(), candidate.receipt_id)
                .expect("read receipt")
                .is_none()
        );
        assert!(
            read_receipt_index(stx.world(), candidate.channel_id)
                .expect("read index")
                .is_none()
        );
        assert_no_receipt_status_mutation(&stx);

        seed_test_call_hash(&mut stx, 0xA7);
        RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
            .execute(&authority, &mut stx)
            .expect("restored valid lock settles");
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::new(90_u32, 6));
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::new(10_u32, 6));
    }

    #[test]
    fn receipt_without_funded_lock_fails_closed() {
        let settlement = keypair(0x4A);
        let authority = account(&settlement);
        let state = state_with_accounts(&[&settlement]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xA3);
        let policy_digest = activate_policy(&mut stx, &authority);
        let candidate = receipt(&settlement, 1, 10, 11, 0, 10);

        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_receipt(stx.world(), candidate.receipt_id)
                .expect("read receipt")
                .is_none()
        );
        assert!(
            read_receipt_index(stx.world(), candidate.channel_id)
                .expect("read index")
                .is_none()
        );
        assert_no_receipt_status_mutation(&stx);
    }

    #[test]
    fn receipt_overdraw_rejects_without_asset_or_audit_mutation() {
        let settlement = keypair(0x4B);
        let buyer = keypair(0x4C);
        let provider = keypair(0x4D);
        let treasury = keypair(0x4E);
        let authority = account(&settlement);
        let buyer_id = account(&buyer);
        let provider_id = account(&provider);
        let treasury_id = account(&treasury);
        let state = state_with_settlement_accounts(&settlement, &buyer, &provider, &treasury, 50);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xA4);
        let policy_digest = activate_policy(&mut stx, &authority);
        let candidate = receipt(&settlement, 1, 12, 13, 0, 10);
        open_settlement_lock(
            &mut stx,
            &buyer_id,
            &provider_id,
            &authority,
            candidate.channel_id,
            50,
        );
        let escrow_id = orderbook_settlement_escrow_id(candidate.channel_id);
        let custody = stx
            .world
            .asset_escrows
            .get(&escrow_id)
            .expect("settlement lock")
            .custody
            .clone();

        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert_eq!(asset_balance(&stx, &provider_id), Numeric::zero());
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::zero());
        assert_eq!(asset_balance(&stx, &custody), Numeric::new(50_u32, 6));
        assert_eq!(
            stx.world
                .asset_escrows
                .get(&escrow_id)
                .expect("settlement lock")
                .remaining_amount,
            Numeric::new(50_u32, 6)
        );
        assert!(
            read_receipt(stx.world(), candidate.receipt_id)
                .expect("read receipt")
                .is_none()
        );
        assert!(
            read_receipt_index(stx.world(), candidate.channel_id)
                .expect("read index")
                .is_none()
        );
        assert_no_receipt_status_mutation(&stx);
    }

    #[test]
    fn receipt_destination_overflow_rejects_without_partial_fee_or_custody_mutation() {
        let settlement = keypair(0x4F);
        let buyer = keypair(0x50);
        let provider = keypair(0x51);
        let treasury = keypair(0x52);
        let authority = account(&settlement);
        let buyer_id = account(&buyer);
        let provider_id = account(&provider);
        let treasury_id = account(&treasury);
        let state =
            state_with_settlement_accounts(&settlement, &buyer, &provider, &treasury, 1_000);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        seed_test_call_hash(&mut stx, 0xA5);
        let policy_digest = activate_policy(&mut stx, &authority);
        let candidate = receipt(&settlement, 1, 14, 15, 0, 10);
        open_settlement_lock(
            &mut stx,
            &buyer_id,
            &provider_id,
            &authority,
            candidate.channel_id,
            1_000,
        );
        let escrow_id = orderbook_settlement_escrow_id(candidate.channel_id);
        let custody = stx
            .world
            .asset_escrows
            .get(&escrow_id)
            .expect("settlement lock")
            .custody
            .clone();
        let mut maximum_bytes = vec![0xFF; 64];
        maximum_bytes.push(0);
        let maximum = Numeric::new(
            BigInt::from_twos_bytes(&maximum_bytes).expect("512-bit positive maximum"),
            6,
        );
        assert!(
            maximum
                .clone()
                .checked_add(Numeric::new(90_u32, 6))
                .is_none()
        );
        let provider_asset = Asset::new(
            AssetId::of(settlement_asset_definition(), provider_id.clone()),
            maximum.clone(),
        );
        let (provider_asset_id, provider_asset_value) = provider_asset.into_key_value();
        stx.world
            .assets
            .insert(provider_asset_id, provider_asset_value);

        assert!(
            RecordSorafsOrderbookSettlementReceipt::new(encode(&candidate), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert_eq!(asset_balance(&stx, &provider_id), maximum);
        assert_eq!(asset_balance(&stx, &treasury_id), Numeric::zero());
        assert_eq!(asset_balance(&stx, &custody), Numeric::new(1_000_u32, 6));
        assert_eq!(
            stx.world
                .asset_escrows
                .get(&escrow_id)
                .expect("settlement lock")
                .remaining_amount,
            Numeric::new(1_000_u32, 6)
        );
        assert!(
            read_receipt(stx.world(), candidate.receipt_id)
                .expect("read receipt")
                .is_none()
        );
        assert!(
            read_receipt_index(stx.world(), candidate.channel_id)
                .expect("read index")
                .is_none()
        );
        assert_no_receipt_status_mutation(&stx);
    }

    #[test]
    fn corrupted_authoritative_state_fails_closed_before_new_mutation() {
        let buyer = keypair(0x51);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let order = order(&buyer, 1);
        stx.world
            .smart_contract_state
            .insert(order_key(order.order_id), vec![0xFF; 16]);

        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            FindSorafsOrderbookOrders::new(None, None, 10)
                .execute(&stx)
                .is_err(),
            "typed listings must fail closed on corrupt authoritative records"
        );
        assert!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .is_none()
        );
        assert_eq!(
            stx.world
                .smart_contract_state
                .get(&order_key(order.order_id))
                .expect("corrupt state remains"),
            &vec![0xFF; 16]
        );
    }

    #[test]
    fn missing_or_corrupt_status_fails_closed_before_order_mutation() {
        let buyer = keypair(0x57);
        let authority = account(&buyer);
        let state = state_with_accounts(&[&buyer]);
        let mut block = state.block(block_header());
        let mut stx = block.transaction();
        let policy_digest = activate_policy(&mut stx, &authority);
        let order = order(&buyer, 1);

        stx.world.smart_contract_state.remove(status_key().clone());
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_order(stx.world(), order.order_id)
                .expect("read order")
                .is_none()
        );
        assert!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .is_none()
        );

        let corrupt = OrderbookLedgerStatusV1 {
            open_orders: 0,
            cancelled_orders: 0,
            settlement_receipts: 0,
            settlement_channels: 1,
            updated_at_unix: NOW,
        };
        stx.world
            .smart_contract_state
            .insert(status_key().clone(), encode(&corrupt));
        assert!(FindSorafsOrderbookStatus.execute(&stx).is_err());
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
                .execute(&authority, &mut stx)
                .is_err()
        );
        assert!(
            read_order(stx.world(), order.order_id)
                .expect("read order")
                .is_none()
        );
        assert!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .is_none()
        );

        let saturated = OrderbookLedgerStatusV1 {
            open_orders: u64::MAX,
            cancelled_orders: 0,
            settlement_receipts: 0,
            settlement_channels: 0,
            updated_at_unix: NOW,
        };
        stx.world
            .smart_contract_state
            .insert(status_key().clone(), encode(&saturated));
        assert!(
            SubmitSorafsOrderbookOrder::new(encode(&order), policy_digest)
                .execute(&authority, &mut stx)
                .is_err(),
            "saturated counters must reject rather than wrap"
        );
        assert!(
            read_order(stx.world(), order.order_id)
                .expect("read order")
                .is_none()
        );
        assert!(
            read_nonce(stx.world(), &authority)
                .expect("read nonce")
                .is_none()
        );
    }
}
