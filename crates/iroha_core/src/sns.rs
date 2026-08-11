//! Ledger-backed SNS storage and mutation helpers.
//!
//! This module is the authoritative SNS read/write path used by account aliases,
//! domain-name lease checks, dataspace-name ownership checks, and the Torii SNS
//! HTTP API. SNS records and policies are stored in `World.smart_contract_state`
//! so the ledger-backed lifecycle model remains deterministic across peers.

#[cfg(test)]
use std::time::SystemTime;
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

#[cfg(test)]
use iroha_data_model::block::BlockHeader;
#[cfg(test)]
use iroha_data_model::transaction::Executable;
use iroha_data_model::{
    Identifiable,
    account::{AccountAddress, AccountId, rekey::AccountAlias},
    alias_setup::{AccountAliasName, AliasAutoRenewConfigV1, AliasAutoRenewStateV1, AliasTargetV1},
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    domain::DomainId,
    isi::{alias_setup::EnsureAlias, register::RegisterBox},
    metadata::Metadata,
    nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
    permission::Permission,
    sns::{
        AuctionKind, ControllerType, NameAuctionStateV1, NameControllerV1, NameRecordV1,
        NameSelectorError, NameSelectorV1, NameStatus, NameTombstoneStateV1, PriceTierV1,
        ReservedNameV1, SuffixFeeSplitV1, SuffixId, SuffixPolicyV1, SuffixStatus, TokenValue,
        fixtures,
    },
    state_path::StatePath,
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
#[cfg(test)]
use iroha_primitives::json::Json as IrohaJson;
use iroha_primitives::numeric::{Numeric, Quantity};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode as _, Encode as _};
use regex::Regex;
use thiserror::Error;

#[cfg(test)]
use crate::state::{State, StateReadOnly};
use crate::state::{StateBlock, StateTransaction, World, WorldReadOnly};

pub use iroha_data_model::sns::{
    ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID, DOMAIN_NAME_SUFFIX_ID,
};

const MS_PER_DAY: u64 = 86_400_000;
const MS_PER_YEAR: u64 = iroha_data_model::alias_setup::ALIAS_LEASE_YEAR_MS;
const EXPIRED_TOMBSTONE_REASON: &str = "expired";
const LEGACY_ACCOUNT_ALIAS_LABEL_REGEX: &str = r"^[a-z0-9@.-]{3,255}$";
const LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID: &str = "61CtjvNd9T3THAR65GsMVHr82Bjc";
fn default_namespace_lease_price() -> Quantity {
    "0.5"
        .parse()
        .expect("hard-coded SNS lease price is canonical")
}

/// Reserved dataspace alias that must stay permanently defined.
pub const RESERVED_UNIVERSAL_DATASPACE_ALIAS: &str = "universal";
/// Stable diagnostic code emitted when static and ledger-backed dataspace mappings disagree.
pub const ALIAS_CATALOG_MAPPING_CONFLICT_CODE: &str = "alias.catalog.mapping_conflict";
/// Name-record metadata key carrying the expected numeric id of a dataspace alias.
pub const SNS_DATASPACE_ID_METADATA_KEY: &str = "sns.dataspace_id";
const SNS_DYNAMIC_DATASPACE_FAULT_TOLERANCE: u32 = 1;

/// Maximum number of persisted alias auto-renew records examined in one block.
///
/// This consensus constant bounds native maintenance work. A durable cursor
/// advances through canonically ordered storage keys so larger registries remain fair.
pub const ALIAS_AUTO_RENEW_SWEEP_LIMIT: usize = 64;

/// Non-reusable proof that the SNS maintenance sweep admitted one exact renewal charge.
pub(crate) struct VerifiedSnsAutoRenewalCharge {
    selector: NameSelectorV1,
    owner: AccountId,
    current_expiry_ms: u64,
    target_expiry_ms: u64,
    source_id: AssetId,
    destination: AccountId,
    amount: Quantity,
}

impl VerifiedSnsAutoRenewalCharge {
    fn new(
        selector: NameSelectorV1,
        owner: AccountId,
        current_expiry_ms: u64,
        target_expiry_ms: u64,
        source_id: AssetId,
        destination: AccountId,
        amount: Quantity,
    ) -> Self {
        Self {
            selector,
            owner,
            current_expiry_ms,
            target_expiry_ms,
            source_id,
            destination,
            amount,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        NameSelectorV1,
        AccountId,
        u64,
        u64,
        AssetId,
        AccountId,
        Quantity,
    ) {
        (
            self.selector,
            self.owner,
            self.current_expiry_ms,
            self.target_expiry_ms,
            self.source_id,
            self.destination,
            self.amount,
        )
    }
}

/// Stable suspension code for a pinned SNS policy-version mismatch.
pub const ALIAS_AUTO_RENEW_POLICY_DRIFT_CODE: &str = "alias.auto_renew.policy_drift";
/// Stable suspension code for a pinned payment-asset mismatch.
pub const ALIAS_AUTO_RENEW_ASSET_DRIFT_CODE: &str = "alias.auto_renew.asset_drift";
/// Stable suspension code for a persisted/current resource-owner mismatch.
pub const ALIAS_AUTO_RENEW_OWNER_DRIFT_CODE: &str = "alias.auto_renew.owner_drift";
/// Stable suspension code for an invalid persisted auto-renew timing range.
pub const ALIAS_AUTO_RENEW_RANGE_INVALID_CODE: &str = "alias.auto_renew.range_invalid";
/// Stable suspension code after the configured consecutive-failure limit.
pub const ALIAS_AUTO_RENEW_FAILURES_EXHAUSTED_CODE: &str = "alias.auto_renew.failures_exhausted";

const ALIAS_AUTO_RENEW_CURSOR_VERSION: u8 = 1;
const ALIAS_AUTO_RENEW_STATE_PREFIX: &str = "sns/auto_renew/";
const ALIAS_AUTO_RENEW_CURSOR_KEY: &str = "sns/auto_renew_cursor/v1";

#[derive(Debug, Clone, PartialEq, Eq, norito::codec::Encode, norito::codec::Decode)]
struct AliasAutoRenewCursorV1 {
    version: u8,
    last_storage_key: StatePath,
}

/// Internal record proving that Core already debited the exact lease quote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LeasePayment {
    pub(crate) asset_id: String,
    pub(crate) gross_amount: Quantity,
    pub(crate) net_amount: Quantity,
}

/// Internal registrar input assembled only after native payment succeeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegisterNameInput {
    pub(crate) selector: NameSelectorV1,
    pub(crate) owner: AccountId,
    pub(crate) controllers: Vec<NameControllerV1>,
    pub(crate) term_years: u8,
    pub(crate) pricing_class_hint: Option<u8>,
    pub(crate) payment: LeasePayment,
    pub(crate) metadata: Metadata,
}

/// Errors returned by the ledger-backed SNS helpers.
#[derive(Debug, Error)]
pub enum SnsError {
    /// The requested entity is missing from authoritative state.
    #[error("{0}")]
    NotFound(String),
    /// The caller provided an invalid selector or payload.
    #[error("{0}")]
    BadRequest(String),
    /// The requested mutation conflicts with the authoritative SNS state.
    #[error("{0}")]
    Conflict(String),
    /// The state mutation could not be committed.
    #[error("{0}")]
    Internal(String),
}

/// SNS namespaces used by the authoritative name-record storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnsNamespace {
    /// Full account-alias key (`name@domain.dataspace` or `name@dataspace`).
    AccountAlias,
    /// Canonical `domain.dataspace` literal.
    Domain,
    /// Canonical dataspace alias.
    Dataspace,
}

/// Deterministic billing quote for a SNS lease operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseQuote {
    /// Canonical selector for the leased name.
    pub selector: NameSelectorV1,
    /// Pricing class that applies to the operation.
    pub pricing_class: u8,
    /// Canonical payment asset literal required by the policy.
    pub payment_asset_id: String,
    /// Asset definition charged for the operation.
    pub payment_asset_definition_id: AssetDefinitionId,
    /// Account receiving the lease payment.
    pub collector_account: AccountId,
    /// Exact non-negative gross/net charge for the operation.
    pub charge_amount: Quantity,
    /// Lease expiry after the operation succeeds.
    pub expires_at_ms: u64,
    /// Grace-period expiry after the operation succeeds.
    pub grace_expires_at_ms: u64,
    /// Redemption expiry after the operation succeeds.
    pub redemption_expires_at_ms: u64,
}

impl SnsNamespace {
    /// Stable suffix identifier assigned to this namespace.
    #[must_use]
    pub const fn suffix_id(self) -> SuffixId {
        match self {
            Self::AccountAlias => ACCOUNT_ALIAS_SUFFIX_ID,
            Self::Domain => DOMAIN_NAME_SUFFIX_ID,
            Self::Dataspace => DATASPACE_ALIAS_SUFFIX_ID,
        }
    }

    /// Canonical HTTP namespace literal.
    #[must_use]
    pub const fn as_path(self) -> &'static str {
        match self {
            Self::AccountAlias => "account-alias",
            Self::Domain => "domain",
            Self::Dataspace => "dataspace",
        }
    }

    /// Human-readable suffix string used by stored SNS policies.
    #[must_use]
    pub const fn policy_suffix(self) -> &'static str {
        match self {
            Self::AccountAlias => "account-alias",
            Self::Domain => "domain",
            Self::Dataspace => "dataspace",
        }
    }

    fn label_regex(self) -> &'static str {
        match self {
            Self::AccountAlias => r"^[a-z0-9_@.-]{3,255}$",
            Self::Domain => r"^[a-z0-9-]{1,63}\.[a-z0-9-]{1,63}$",
            Self::Dataspace => r"^[a-z0-9-]{1,63}$",
        }
    }

    /// Parse the canonical HTTP namespace literal.
    ///
    /// # Errors
    ///
    /// Returns [`SnsError::BadRequest`] when the namespace is unknown.
    pub fn from_path(path: &str) -> Result<Self, SnsError> {
        match path.trim().to_ascii_lowercase().as_str() {
            "account-alias" | "account_alias" => Ok(Self::AccountAlias),
            "domain" => Ok(Self::Domain),
            "dataspace" => Ok(Self::Dataspace),
            other => Err(SnsError::BadRequest(format!(
                "unknown SNS namespace `{other}`"
            ))),
        }
    }

    /// Resolve the namespace from its fixed suffix identifier.
    ///
    /// # Errors
    ///
    /// Returns [`SnsError::BadRequest`] when the suffix id is not one of the
    /// fixed on-chain namespace identifiers.
    pub fn from_suffix_id(suffix_id: SuffixId) -> Result<Self, SnsError> {
        match suffix_id {
            ACCOUNT_ALIAS_SUFFIX_ID => Ok(Self::AccountAlias),
            DOMAIN_NAME_SUFFIX_ID => Ok(Self::Domain),
            DATASPACE_ALIAS_SUFFIX_ID => Ok(Self::Dataspace),
            other => Err(SnsError::BadRequest(format!(
                "unsupported SNS suffix id `{other}`"
            ))),
        }
    }
}

/// Compute the durable smart-contract-state key for a SNS record selector.
#[must_use]
pub fn record_storage_key(selector: &NameSelectorV1) -> StatePath {
    StatePath::from_str(&format!(
        "sns/records/{}/{}",
        selector.suffix_id,
        hex::encode(selector.name_hash())
    ))
    .expect("static SNS storage key format is a valid StatePath")
}

/// Compute the durable smart-contract-state key for a SNS suffix policy.
#[must_use]
pub fn policy_storage_key(suffix_id: SuffixId) -> StatePath {
    StatePath::from_str(&format!("sns/policies/{suffix_id}"))
        .expect("static SNS policy storage key format is a valid StatePath")
}

/// Compute the durable key for one alias auto-renew configuration record.
///
/// # Errors
///
/// Returns [`SnsError`] if the resolved target is not canonical.
pub fn alias_auto_renew_storage_key(target: &AliasTargetV1) -> Result<StatePath, SnsError> {
    let selector = crate::alias_setup::selector_for_resolved_alias_target(target)
        .map_err(|error| SnsError::BadRequest(error.to_string()))?;
    StatePath::from_str(&format!(
        "sns/auto_renew/{}/{}",
        selector.suffix_id,
        hex::encode(selector.name_hash())
    ))
    .map_err(|error| SnsError::Internal(format!("invalid auto-renew storage key: {error}")))
}

/// Read and validate the persisted auto-renew state for a resolved target.
///
/// # Errors
///
/// Returns [`SnsError`] for malformed, unsupported, or mismatched persisted state.
pub fn alias_auto_renew_state(
    world: &impl WorldReadOnly,
    target: &AliasTargetV1,
) -> Result<Option<AliasAutoRenewStateV1>, SnsError> {
    let key = alias_auto_renew_storage_key(target)?;
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let mut cursor = bytes.as_slice();
    let state = AliasAutoRenewStateV1::decode(&mut cursor).map_err(|error| {
        SnsError::Internal(format!("failed to decode alias auto-renew state: {error}"))
    })?;
    if !cursor.is_empty() {
        return Err(SnsError::Internal(
            "alias auto-renew state contains trailing bytes".to_owned(),
        ));
    }
    if state.version != AliasAutoRenewStateV1::VERSION {
        return Err(SnsError::Conflict(format!(
            "unsupported alias auto-renew state version {}",
            state.version
        )));
    }
    if &state.target != target {
        return Err(SnsError::Conflict(
            "alias auto-renew storage key contains a different target".to_owned(),
        ));
    }
    Ok(Some(state))
}

fn alias_auto_renew_internal_key(literal: &str) -> StatePath {
    StatePath::from_str(literal).expect("hard-coded alias auto-renew key is a valid StatePath")
}

fn alias_auto_renew_cursor(
    world: &impl WorldReadOnly,
) -> Result<Option<AliasAutoRenewCursorV1>, SnsError> {
    let key = alias_auto_renew_internal_key(ALIAS_AUTO_RENEW_CURSOR_KEY);
    let Some(bytes) = world.smart_contract_state().get(&key) else {
        return Ok(None);
    };
    let mut cursor = bytes.as_slice();
    let state = AliasAutoRenewCursorV1::decode(&mut cursor).map_err(|error| {
        SnsError::Internal(format!("failed to decode alias auto-renew cursor: {error}"))
    })?;
    if !cursor.is_empty() {
        return Err(SnsError::Internal(
            "alias auto-renew cursor contains trailing bytes".to_owned(),
        ));
    }
    if state.version != ALIAS_AUTO_RENEW_CURSOR_VERSION {
        return Err(SnsError::Conflict(format!(
            "unsupported alias auto-renew cursor version {}",
            state.version
        )));
    }
    if !state
        .last_storage_key
        .as_ref()
        .starts_with(ALIAS_AUTO_RENEW_STATE_PREFIX)
    {
        return Err(SnsError::Conflict(
            "alias auto-renew cursor points outside the state namespace".to_owned(),
        ));
    }
    Ok(Some(state))
}

fn persist_alias_auto_renew_cursor(
    state_transaction: &mut StateTransaction<'_, '_>,
    last_storage_key: StatePath,
) {
    let key = alias_auto_renew_internal_key(ALIAS_AUTO_RENEW_CURSOR_KEY);
    state_transaction.world.smart_contract_state.insert(
        key,
        AliasAutoRenewCursorV1 {
            version: ALIAS_AUTO_RENEW_CURSOR_VERSION,
            last_storage_key,
        }
        .encode(),
    );
}

fn alias_auto_renew_candidate_keys(
    world: &impl WorldReadOnly,
    last_storage_key: Option<&StatePath>,
    limit: usize,
) -> Vec<StatePath> {
    if limit == 0 {
        return Vec::new();
    }
    let prefix = alias_auto_renew_internal_key(ALIAS_AUTO_RENEW_STATE_PREFIX);
    let start = last_storage_key.cloned().unwrap_or_else(|| prefix.clone());
    let mut keys = Vec::with_capacity(limit);
    for (key, _) in world.smart_contract_state().range(start..) {
        if !key.as_ref().starts_with(ALIAS_AUTO_RENEW_STATE_PREFIX) {
            break;
        }
        if last_storage_key.is_some_and(|last| last == key) {
            continue;
        }
        keys.push(key.clone());
        if keys.len() == limit {
            return keys;
        }
    }
    let Some(last_storage_key) = last_storage_key else {
        return keys;
    };
    for (key, _) in world.smart_contract_state().range(prefix..) {
        if !key.as_ref().starts_with(ALIAS_AUTO_RENEW_STATE_PREFIX) || key > last_storage_key {
            break;
        }
        keys.push(key.clone());
        if keys.len() == limit {
            break;
        }
    }
    keys
}

fn alias_auto_renew_state_by_storage_key(
    world: &impl WorldReadOnly,
    storage_key: &StatePath,
) -> Result<AliasAutoRenewStateV1, SnsError> {
    let bytes = world
        .smart_contract_state()
        .get(storage_key)
        .ok_or_else(|| {
            SnsError::NotFound(format!(
                "alias auto-renew state `{storage_key}` disappeared during maintenance"
            ))
        })?;
    let mut cursor = bytes.as_slice();
    let state = AliasAutoRenewStateV1::decode(&mut cursor).map_err(|error| {
        SnsError::Internal(format!(
            "failed to decode alias auto-renew state `{storage_key}`: {error}"
        ))
    })?;
    if !cursor.is_empty() {
        return Err(SnsError::Internal(format!(
            "alias auto-renew state `{storage_key}` contains trailing bytes"
        )));
    }
    if state.version != AliasAutoRenewStateV1::VERSION {
        return Err(SnsError::Conflict(format!(
            "unsupported alias auto-renew state version {} at `{storage_key}`",
            state.version
        )));
    }
    let expected_key = alias_auto_renew_storage_key(&state.target)?;
    if expected_key != *storage_key {
        return Err(SnsError::Conflict(format!(
            "alias auto-renew state target does not match storage key `{storage_key}`"
        )));
    }
    Ok(state)
}

/// Persist one validated alias auto-renew state record.
///
/// # Errors
///
/// Returns [`SnsError`] if the target cannot produce its canonical storage key.
pub(crate) fn persist_alias_auto_renew_state(
    state_transaction: &mut StateTransaction<'_, '_>,
    state: &AliasAutoRenewStateV1,
) -> Result<(), SnsError> {
    let key = alias_auto_renew_storage_key(&state.target)?;
    state_transaction
        .world
        .smart_contract_state
        .insert(key, state.encode());
    Ok(())
}

enum AliasAutoRenewAttempt {
    NotDue,
    Renewed,
    Retry(String),
    Suspend(&'static str),
}

fn alias_auto_renew_attempt(
    state_transaction: &mut StateTransaction<'_, '_>,
    state: &AliasAutoRenewStateV1,
    config: &AliasAutoRenewConfigV1,
    now_ms: u64,
) -> AliasAutoRenewAttempt {
    if crate::alias_setup::validate_alias_auto_renew_ranges(config).is_err() {
        return AliasAutoRenewAttempt::Suspend(ALIAS_AUTO_RENEW_RANGE_INVALID_CODE);
    }
    let selector = match crate::alias_setup::selector_for_resolved_alias_target(&state.target) {
        Ok(selector) => selector,
        Err(error) => return AliasAutoRenewAttempt::Retry(error.to_string()),
    };
    let policy = match policy_by_id(state_transaction.world(), selector.suffix_id) {
        Some(policy) => policy,
        None => {
            return AliasAutoRenewAttempt::Retry(format!(
                "SNS policy {} is missing or malformed",
                selector.suffix_id
            ));
        }
    };
    if policy.policy_version != config.policy_version {
        return AliasAutoRenewAttempt::Suspend(ALIAS_AUTO_RENEW_POLICY_DRIFT_CODE);
    }
    let policy_payment_asset = match payment_asset_definition_id(&policy) {
        Ok(asset) => asset,
        Err(error) => return AliasAutoRenewAttempt::Retry(error.to_string()),
    };
    if policy_payment_asset != config.payment_asset {
        return AliasAutoRenewAttempt::Suspend(ALIAS_AUTO_RENEW_ASSET_DRIFT_CODE);
    }

    if let Err(error) = crate::alias_setup::validate_resolved_alias_target(
        state_transaction.world(),
        &state_transaction.nexus.dataspace_catalog,
        &state.target,
        now_ms,
    ) {
        return AliasAutoRenewAttempt::Retry(error.to_string());
    }
    let record = match get_name_record_by_selector(state_transaction.world(), &selector, now_ms) {
        Ok(record) => record,
        Err(error) => return AliasAutoRenewAttempt::Retry(error.to_string()),
    };
    if record.owner != state.owner {
        return AliasAutoRenewAttempt::Suspend(ALIAS_AUTO_RENEW_OWNER_DRIFT_CODE);
    }
    if now_ms
        < record
            .expires_at_ms
            .saturating_sub(config.renew_before_expiry_ms)
    {
        return AliasAutoRenewAttempt::NotDue;
    }
    if state
        .next_retry_at_ms
        .is_some_and(|next_retry_at_ms| now_ms < next_retry_at_ms)
    {
        return AliasAutoRenewAttempt::NotDue;
    }

    let target_expiry_ms = record
        .expires_at_ms
        .saturating_add(years_to_ms(config.term_years));
    let quote = match quote_resolved_name_renewal(
        state_transaction.world(),
        selector.clone(),
        record.expires_at_ms,
        target_expiry_ms,
        now_ms,
    ) {
        Ok(quote) => quote,
        Err(error) => return AliasAutoRenewAttempt::Retry(error.to_string()),
    };
    if quote.payment_asset_definition_id != config.payment_asset {
        return AliasAutoRenewAttempt::Suspend(ALIAS_AUTO_RENEW_ASSET_DRIFT_CODE);
    }
    if quote.charge_amount > config.max_amount {
        return AliasAutoRenewAttempt::Retry(format!(
            "exact renewal quote {} exceeds configured cap {}",
            quote.charge_amount, config.max_amount
        ));
    }

    let charge = VerifiedSnsAutoRenewalCharge::new(
        selector.clone(),
        state.owner.clone(),
        record.expires_at_ms,
        target_expiry_ms,
        AssetId::of(config.payment_asset.clone(), state.owner.clone()),
        quote.collector_account.clone(),
        quote.charge_amount.clone(),
    );
    if let Err(error) =
        crate::smartcontracts::isi::asset::isi::execute_verified_sns_auto_renewal_charge(
            state_transaction,
            charge,
        )
    {
        return AliasAutoRenewAttempt::Retry(error.to_string());
    }
    let payment = native_payment_for_quote(&quote);
    match renew_resolved_name(
        state_transaction,
        selector,
        record.expires_at_ms,
        target_expiry_ms,
        payment,
    ) {
        Ok(_) => AliasAutoRenewAttempt::Renewed,
        Err(error) => AliasAutoRenewAttempt::Retry(error.to_string()),
    }
}

fn advance_alias_auto_renew_revision(state: &mut AliasAutoRenewStateV1) {
    state.revision = state.revision.saturating_add(1);
}

fn suspend_alias_auto_renew(
    state_block: &mut StateBlock<'_>,
    mut state: AliasAutoRenewStateV1,
    reason: &'static str,
) {
    advance_alias_auto_renew_revision(&mut state);
    state.next_retry_at_ms = None;
    state.suspended_reason = Some(reason.to_owned());
    let target = state.target.clone();
    let mut transaction = state_block.transaction();
    match persist_alias_auto_renew_state(&mut transaction, &state) {
        Ok(()) => {
            transaction.apply();
            iroha_logger::warn!(target = %target, reason, "alias auto-renew suspended");
        }
        Err(error) => {
            iroha_logger::error!(target = %target, %error, "failed to persist alias auto-renew suspension");
        }
    }
}

fn record_alias_auto_renew_failure(
    state_block: &mut StateBlock<'_>,
    mut state: AliasAutoRenewStateV1,
    config: &AliasAutoRenewConfigV1,
    now_ms: u64,
    error: &str,
) {
    advance_alias_auto_renew_revision(&mut state);
    state.failure_count = state.failure_count.saturating_add(1);
    if state.failure_count >= config.max_failures {
        state.next_retry_at_ms = None;
        state.suspended_reason = Some(ALIAS_AUTO_RENEW_FAILURES_EXHAUSTED_CODE.to_owned());
    } else {
        state.next_retry_at_ms = Some(now_ms.saturating_add(config.retry_backoff_ms));
        state.suspended_reason = None;
    }
    let target = state.target.clone();
    let suspended = state.suspended_reason.is_some();
    let failure_count = state.failure_count;
    let mut transaction = state_block.transaction();
    match persist_alias_auto_renew_state(&mut transaction, &state) {
        Ok(()) => {
            transaction.apply();
            if suspended {
                iroha_logger::warn!(
                    target = %target,
                    failure_count,
                    error,
                    "alias auto-renew suspended after repeated failures"
                );
            } else {
                iroha_logger::info!(
                    target = %target,
                    failure_count,
                    error,
                    "alias auto-renew scheduled a deterministic retry"
                );
            }
        }
        Err(persist_error) => {
            iroha_logger::error!(
                target = %target,
                %persist_error,
                "failed to persist alias auto-renew failure state"
            );
        }
    }
}

fn process_alias_auto_renew_storage_key(
    state_block: &mut StateBlock<'_>,
    storage_key: &StatePath,
    now_ms: u64,
) {
    let state = match alias_auto_renew_state_by_storage_key(&state_block.world, storage_key) {
        Ok(state) => state,
        Err(error) => {
            iroha_logger::error!(%storage_key, %error, "malformed alias auto-renew state skipped");
            return;
        }
    };
    let Some(config) = state.config.clone() else {
        return;
    };
    if state.suspended_reason.is_some() {
        return;
    }

    let mut transaction = state_block.transaction();
    match alias_auto_renew_attempt(&mut transaction, &state, &config, now_ms) {
        AliasAutoRenewAttempt::NotDue => {}
        AliasAutoRenewAttempt::Renewed => {
            let mut updated = state;
            advance_alias_auto_renew_revision(&mut updated);
            updated.failure_count = 0;
            updated.next_retry_at_ms = None;
            updated.suspended_reason = None;
            let target = updated.target.clone();
            match persist_alias_auto_renew_state(&mut transaction, &updated) {
                Ok(()) => {
                    transaction.apply();
                    iroha_logger::info!(target = %target, now_ms, "alias lease auto-renewed");
                }
                Err(error) => {
                    iroha_logger::error!(target = %target, %error, "failed to persist successful alias auto-renew state");
                }
            }
        }
        AliasAutoRenewAttempt::Suspend(reason) => {
            drop(transaction);
            suspend_alias_auto_renew(state_block, state, reason);
        }
        AliasAutoRenewAttempt::Retry(error) => {
            drop(transaction);
            record_alias_auto_renew_failure(state_block, state, &config, now_ms, &error);
        }
    }
}

/// Process a bounded, fair slice of enabled alias auto-renew records at block time.
///
/// This native maintenance path is intentionally infallible at the block level:
/// individual payment or renewal failures update deterministic retry/suspension
/// state, while malformed state or cursor records fail closed without mutation.
pub(crate) fn process_alias_auto_renewals(state_block: &mut StateBlock<'_>) {
    let cursor = match alias_auto_renew_cursor(&state_block.world) {
        Ok(cursor) => cursor,
        Err(error) => {
            iroha_logger::error!(%error, "alias auto-renew sweep skipped because its cursor is invalid");
            return;
        }
    };
    let storage_keys = alias_auto_renew_candidate_keys(
        &state_block.world,
        cursor.as_ref().map(|cursor| &cursor.last_storage_key),
        ALIAS_AUTO_RENEW_SWEEP_LIMIT,
    );
    let now_ms =
        u64::try_from(state_block._curr_block.creation_time().as_millis()).unwrap_or(u64::MAX);
    for storage_key in &storage_keys {
        process_alias_auto_renew_storage_key(state_block, storage_key, now_ms);
    }
    if let Some(last_storage_key) = storage_keys.last().cloned() {
        let mut transaction = state_block.transaction();
        persist_alias_auto_renew_cursor(&mut transaction, last_storage_key);
        transaction.apply();
    }
}

/// Build the selector used for a full account-alias lease record.
pub fn selector_for_account_alias(
    alias: &AccountAlias,
    catalog: &DataSpaceCatalog,
) -> Result<NameSelectorV1, iroha_data_model::error::ParseError> {
    Ok(NameSelectorV1 {
        version: NameSelectorV1::VERSION,
        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
        label: alias.to_literal(catalog)?,
    })
}

/// Build the selector used for a canonical domain-name lease record.
pub fn selector_for_domain(domain: &DomainId) -> Result<NameSelectorV1, NameSelectorError> {
    NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain.to_string())
}

/// Build the selector used for a canonical dataspace-alias lease record.
pub fn selector_for_dataspace_alias(alias: &str) -> Result<NameSelectorV1, NameSelectorError> {
    NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, alias)
}

/// Derive the deterministic dataspace id for a SNS dataspace alias.
///
/// Configured Nexus dataspaces keep their explicit catalog ids. SNS-only
/// dataspaces use the same stable name hash that keys the ledger record, so
/// every peer can route a newly registered dataspace without an out-of-band
/// catalog update.
#[must_use]
pub fn dataspace_id_for_sns_alias(alias: &str) -> Option<DataSpaceId> {
    let selector = selector_for_dataspace_alias(alias.trim()).ok()?;
    if selector.label == RESERVED_UNIVERSAL_DATASPACE_ALIAS {
        return Some(DataSpaceId::UNIVERSAL);
    }
    Some(DataSpaceId::from_hash(&selector.name_hash()))
}

fn selector_for_account_alias_literal(
    literal: &str,
    catalog: &DataSpaceCatalog,
) -> Result<NameSelectorV1, SnsError> {
    let alias = AccountAlias::from_literal(literal, catalog)
        .map_err(|err| SnsError::BadRequest(err.to_string()))?;
    selector_for_account_alias(&alias, catalog).map_err(|err| SnsError::BadRequest(err.to_string()))
}

/// Canonicalize a namespace-scoped literal into the fixed SNS selector.
///
/// # Errors
///
/// Returns [`SnsError::BadRequest`] when the namespace or literal is invalid.
pub fn selector_for_namespace_literal(
    namespace: SnsNamespace,
    literal: &str,
    catalog: &DataSpaceCatalog,
) -> Result<NameSelectorV1, SnsError> {
    match namespace {
        SnsNamespace::AccountAlias => selector_for_account_alias_literal(literal, catalog),
        SnsNamespace::Domain => {
            let domain = DomainId::parse_fully_qualified(literal.trim())
                .map_err(|err| SnsError::BadRequest(err.reason().to_owned()))?;
            selector_for_domain(&domain).map_err(|err| SnsError::BadRequest(err.to_string()))
        }
        SnsNamespace::Dataspace => selector_for_dataspace_alias(literal)
            .map_err(|err| SnsError::BadRequest(err.to_string())),
    }
}

/// Decode a SNS record from world state for the supplied selector.
#[must_use]
pub fn record_by_selector(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
) -> Option<NameRecordV1> {
    let key = record_storage_key(selector);
    let bytes = world.smart_contract_state().get(&key)?;
    decode_record_for_selector(bytes, selector).ok()
}

/// Decode a SNS policy from world state for the supplied suffix id.
#[must_use]
pub fn policy_by_id(world: &impl WorldReadOnly, suffix_id: SuffixId) -> Option<SuffixPolicyV1> {
    let key = policy_storage_key(suffix_id);
    let bytes = world.smart_contract_state().get(&key)?;
    decode_policy_for_suffix(bytes, suffix_id).ok()
}

fn decode_record_for_selector(
    bytes: &[u8],
    selector: &NameSelectorV1,
) -> Result<NameRecordV1, SnsError> {
    let decode = || {
        let mut slice = bytes;
        let record = NameRecordV1::decode(&mut slice)
            .map_err(|_| SnsError::Internal("failed to decode an SNS record".to_owned()))?;
        if !slice.is_empty() {
            return Err(SnsError::Internal(
                "SNS record contains trailing bytes".to_owned(),
            ));
        }
        if record.selector != *selector || record.name_hash != selector.name_hash() {
            return Err(SnsError::Internal(
                "SNS record identity mismatch".to_owned(),
            ));
        }
        Ok(record)
    };
    if !crate::smartcontracts::isi::query::singular_query_limits_active() {
        return decode();
    }
    let elements = bytes
        .len()
        .checked_mul(8)
        .ok_or_else(|| SnsError::Internal("SNS record exceeds query memory limits".to_owned()))?;
    let limits = crate::smartcontracts::isi::query::singular_query_decode_limits(
        bytes.len(),
        norito::DecodeLimits::new(elements, bytes.len(), elements, usize::MAX, 64),
    )
    .map_err(|_| SnsError::Internal("SNS record exceeds query memory limits".to_owned()))?;
    norito::with_decode_limits_scope(limits, decode)
}

fn decode_policy_for_suffix(bytes: &[u8], suffix_id: SuffixId) -> Result<SuffixPolicyV1, SnsError> {
    let mut slice = bytes;
    let policy = SuffixPolicyV1::decode(&mut slice).map_err(|err| {
        SnsError::Internal(format!(
            "failed to decode SNS suffix policy {suffix_id}: {err}"
        ))
    })?;
    if !slice.is_empty() {
        return Err(SnsError::Internal(format!(
            "SNS suffix policy {suffix_id} contains trailing bytes"
        )));
    }
    let namespace = SnsNamespace::from_suffix_id(suffix_id)?;
    if policy.suffix_id != suffix_id || policy.suffix != namespace.policy_suffix() {
        return Err(SnsError::Internal(format!(
            "SNS suffix policy identity mismatch for {suffix_id}"
        )));
    }
    Ok(policy)
}

fn bootstrap_steward_for_world(world: &impl WorldReadOnly) -> AccountId {
    world
        .domain(&iroha_genesis::GENESIS_DOMAIN_ID)
        .map(|domain| domain.owned_by().clone())
        .unwrap_or_else(|_| fixtures::steward_account())
}

fn active_account_alias_lease_record(
    state_transaction: &StateTransaction<'_, '_>,
    owner: &AccountId,
    label: &AccountAlias,
) -> Result<(StatePath, NameRecordV1), SnsError> {
    let selector = selector_for_account_alias(label, &state_transaction.nexus.dataspace_catalog)
        .map_err(|err| SnsError::BadRequest(err.to_string()))?;
    let storage_key = record_storage_key(&selector);
    let bytes = state_transaction
        .world
        .smart_contract_state
        .get(&storage_key)
        .ok_or_else(|| {
            SnsError::NotFound(format!(
                "active SNS lease required for account alias `{}`",
                selector.normalized_label()
            ))
        })?;
    let record = decode_record_for_selector(bytes, &selector)?;
    let now_ms = state_transaction.block_unix_timestamp_ms();
    if !matches!(effective_status(&record, now_ms), NameStatus::Active) {
        return Err(SnsError::Conflict(format!(
            "active SNS lease required for account alias `{}`",
            selector.normalized_label()
        )));
    }
    if record.owner != *owner {
        return Err(SnsError::Conflict(format!(
            "active SNS lease for account alias `{}` is owned by another account",
            selector.normalized_label()
        )));
    }
    Ok((storage_key, record))
}

/// Ensures an active SNS name lease exists and is owned by the exact alias target account.
pub fn ensure_account_alias_lease(
    state_transaction: &StateTransaction<'_, '_>,
    owner: &AccountId,
    label: &AccountAlias,
) -> Result<(), SnsError> {
    active_account_alias_lease_record(state_transaction, owner, label).map(|_| ())
}

fn prepare_account_alias_record_rekey(
    old_owner: &AccountId,
    new_owner: &AccountId,
    record: &mut NameRecordV1,
) -> Result<(), SnsError> {
    let old_address = AccountAddress::from_account_id(old_owner).map_err(|err| {
        SnsError::Internal(format!(
            "failed to derive current account-alias controller for `{}`: {err}",
            record.selector.normalized_label()
        ))
    })?;
    let new_address = AccountAddress::from_account_id(new_owner).map_err(|err| {
        SnsError::Internal(format!(
            "failed to derive replacement account-alias controller for `{}`: {err}",
            record.selector.normalized_label()
        ))
    })?;

    let mut owner_controller_count = 0_usize;
    for controller in &mut record.controllers {
        if controller.account_address.as_ref() == Some(&new_address) {
            return Err(SnsError::Conflict(format!(
                "SNS lease for account alias `{}` already contains the replacement account controller",
                record.selector.normalized_label()
            )));
        }
        if controller.account_address.as_ref() != Some(&old_address) {
            continue;
        }
        if controller.controller_type != ControllerType::Account {
            return Err(SnsError::Internal(format!(
                "SNS lease for account alias `{}` has an invalid owner controller type",
                record.selector.normalized_label()
            )));
        }
        controller.account_address = Some(new_address.clone());
        owner_controller_count = owner_controller_count.saturating_add(1);
    }
    if owner_controller_count != 1 {
        return Err(SnsError::Conflict(format!(
            "SNS lease for account alias `{}` must contain exactly one owner account controller (found {owner_controller_count})",
            record.selector.normalized_label(),
        )));
    }

    record.transfer_owner(new_owner.clone());
    Ok(())
}

/// Strictly enumerate and prepare every account-alias lease owned by `old_owner` for rekey.
///
/// Unlike the binding indexes, SNS ownership also covers acquired-but-unbound and previously
/// unbound leases. A controller replacement must migrate those records too or the old canonical
/// account id becomes an unrecoverable owner. Any malformed record in the account-alias namespace
/// rejects the entire rekey before state mutation.
pub(crate) fn prepare_all_account_alias_lease_rekeys(
    state_transaction: &StateTransaction<'_, '_>,
    old_owner: &AccountId,
    new_owner: &AccountId,
) -> Result<BTreeMap<AccountAlias, (StatePath, NameRecordV1)>, SnsError> {
    let prefix = StatePath::from_str(&format!("sns/records/{ACCOUNT_ALIAS_SUFFIX_ID}/"))
        .expect("static account-alias SNS record prefix is valid");
    let prefix_literal = prefix.as_ref().to_owned();
    let mut updates = BTreeMap::new();

    for (storage_key, bytes) in state_transaction
        .world
        .smart_contract_state
        .range(prefix.clone()..)
    {
        if !storage_key.as_ref().starts_with(&prefix_literal) {
            break;
        }

        let mut slice = bytes.as_slice();
        let mut record = NameRecordV1::decode(&mut slice).map_err(|err| {
            SnsError::Internal(format!(
                "failed to decode account-alias SNS record `{storage_key}`: {err}"
            ))
        })?;
        if !slice.is_empty() {
            return Err(SnsError::Internal(format!(
                "account-alias SNS record `{storage_key}` contains trailing bytes"
            )));
        }
        if record.selector.suffix_id != ACCOUNT_ALIAS_SUFFIX_ID
            || record.name_hash != record.selector.name_hash()
            || record_storage_key(&record.selector) != storage_key.clone()
        {
            return Err(SnsError::Internal(format!(
                "account-alias SNS record identity mismatch at `{storage_key}`"
            )));
        }
        let alias = AccountAlias::from_literal(
            record.selector.normalized_label(),
            &state_transaction.nexus.dataspace_catalog,
        )
        .map_err(|err| {
            SnsError::Internal(format!(
                "account-alias SNS record `{storage_key}` has a non-canonical selector: {err}"
            ))
        })?;
        let canonical_selector =
            selector_for_account_alias(&alias, &state_transaction.nexus.dataspace_catalog)
                .map_err(|err| SnsError::Internal(err.to_string()))?;
        if record.selector != canonical_selector {
            return Err(SnsError::Internal(format!(
                "account-alias SNS record `{storage_key}` has a non-canonical selector"
            )));
        }
        if record.owner != *old_owner {
            continue;
        }

        prepare_account_alias_record_rekey(old_owner, new_owner, &mut record)?;
        if updates
            .insert(alias, (storage_key.clone(), record))
            .is_some()
        {
            return Err(SnsError::Internal(
                "duplicate canonical account-alias SNS lease during rekey".to_owned(),
            ));
        }
    }

    Ok(updates)
}

fn seed_name_record_with_metadata_if_missing(
    world: &mut World,
    owner: &AccountId,
    selector: NameSelectorV1,
    metadata: Metadata,
) {
    let storage_key = record_storage_key(&selector);
    if world
        .smart_contract_state
        .view()
        .get(&storage_key)
        .is_some()
    {
        return;
    }

    let address = AccountAddress::from_account_id(owner)
        .expect("account id should convert to account address");
    let record = NameRecordV1::new(
        selector,
        owner.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        metadata,
    );
    world
        .smart_contract_state
        .insert(storage_key, record.encode());
}

fn seed_name_record_if_missing(world: &mut World, owner: &AccountId, selector: NameSelectorV1) {
    seed_name_record_with_metadata_if_missing(world, owner, selector, Metadata::default());
}

fn seed_alias_manage_permissions_if_missing(
    world: &mut World,
    authority: &AccountId,
    label: &AccountAlias,
    dataspace_catalog: &DataSpaceCatalog,
) {
    let mut permissions = world
        .account_permissions
        .view()
        .get(authority)
        .cloned()
        .unwrap_or_default();
    let dataspace_permission = Permission::from(CanManageAccountAlias {
        scope: AccountAliasPermissionScope::Dataspace(label.dataspace),
    });
    permissions.insert(dataspace_permission);
    if let Some(domain_id) = label
        .domain_id(dataspace_catalog)
        .expect("genesis alias dataspace must resolve")
    {
        let domain_permission = Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(domain_id),
        });
        permissions.insert(domain_permission);
    }
    world
        .account_permissions
        .insert(authority.clone(), permissions);
}

/// Seed bootstrap alias state required by aliases referenced directly in genesis instructions.
///
/// Genesis cannot rely on the normal registrar flow because the namespace policies and
/// bootstrap authority are only coming online while the block executes. This helper
/// pre-seeds the leases and alias-management permissions that the first block itself
/// consumes, mirroring how operators would pre-register those names before normal
/// operation.
pub fn seed_genesis_alias_bootstrap(
    world: &mut World,
    block: &iroha_data_model::block::SignedBlock,
    dataspace_catalog: &DataSpaceCatalog,
) {
    for transaction in block.external_transactions() {
        let authority = transaction.authority();
        for instruction in transaction.instructions().explicit_instructions() {
            if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
                match register {
                    RegisterBox::Domain(register) => {
                        let selector = selector_for_domain(&register.object().id)
                            .expect("genesis domain ids should be canonical");
                        seed_name_record_if_missing(world, authority, selector);
                    }
                    RegisterBox::Account(register) => {
                        if let Some(label) = register.object().label() {
                            seed_alias_manage_permissions_if_missing(
                                world,
                                authority,
                                label,
                                dataspace_catalog,
                            );
                            if let Ok(selector) =
                                selector_for_account_alias(label, dataspace_catalog)
                            {
                                seed_name_record_if_missing(
                                    world,
                                    register.object().id(),
                                    selector,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(ensure) = instruction.as_any().downcast_ref::<EnsureAlias>() {
                let target = ensure.intent.target();
                if let Ok(selector) =
                    crate::alias_setup::selector_for_resolved_alias_target(&target)
                    && let Ok(metadata) = crate::alias_setup::alias_registration_metadata(&target)
                {
                    seed_name_record_with_metadata_if_missing(
                        world,
                        crate::alias_setup::alias_intent_owner(&ensure.intent),
                        selector,
                        metadata,
                    );
                }
            }
        }
    }
}

fn default_namespace_policy(
    namespace: SnsNamespace,
    steward: &AccountId,
    payment_asset_id: &str,
) -> SuffixPolicyV1 {
    SuffixPolicyV1 {
        suffix_id: namespace.suffix_id(),
        suffix: namespace.policy_suffix().to_owned(),
        steward: steward.clone(),
        status: SuffixStatus::Active,
        min_term_years: 1,
        max_term_years: 5,
        grace_period_days: 30,
        redemption_period_days: 60,
        referral_cap_bps: 0,
        reserved_labels: match namespace {
            SnsNamespace::Domain => vec![ReservedNameV1 {
                normalized_label: "treasury".to_owned(),
                assigned_to: Some(steward.clone()),
                release_at_ms: None,
                note: "Protocol reserved domain label".to_owned(),
            }],
            SnsNamespace::Dataspace => vec![iroha_data_model::sns::ReservedNameV1 {
                normalized_label: RESERVED_UNIVERSAL_DATASPACE_ALIAS.to_owned(),
                assigned_to: Some(steward.clone()),
                release_at_ms: None,
                note: "Protocol reserved dataspace alias".to_owned(),
            }],
            _ => Vec::new(),
        },
        payment_asset_id: payment_asset_id.to_owned(),
        pricing: vec![PriceTierV1 {
            tier_id: 0,
            label_regex: namespace.label_regex().to_owned(),
            base_price: TokenValue::new(payment_asset_id, default_namespace_lease_price()),
            auction_kind: AuctionKind::VickreyCommitReveal,
            dutch_floor: None,
            min_duration_years: 1,
            max_duration_years: 5,
        }],
        fee_split: SuffixFeeSplitV1 {
            treasury_bps: 7000,
            steward_bps: 3000,
            referral_max_bps: 0,
            escrow_bps: 0,
        },
        fund_splitter_account: steward.clone(),
        policy_version: 1,
        metadata: Metadata::default(),
    }
}

fn upgrade_legacy_default_namespace_policy(
    namespace: SnsNamespace,
    policy: &mut SuffixPolicyV1,
) -> bool {
    if namespace != SnsNamespace::AccountAlias || policy.suffix_id != namespace.suffix_id() {
        return false;
    }

    let mut changed = false;
    for tier in &mut policy.pricing {
        if tier.label_regex == LEGACY_ACCOUNT_ALIAS_LABEL_REGEX {
            tier.label_regex = namespace.label_regex().to_owned();
            changed = true;
        }
    }
    if changed {
        policy.policy_version = policy.policy_version.saturating_add(1);
    }
    changed
}

fn resolve_configured_payment_asset_literal(
    world: &impl WorldReadOnly,
    selector: &str,
) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return None;
    }

    if let Ok(definition_id) = AssetDefinitionId::parse_address_literal(selector) {
        return Some(definition_id.to_string());
    }

    let alias = AssetDefinitionAlias::from_str(selector).ok()?;
    let definition_id = world.asset_definition_id_by_alias_at(&alias, 0)?;
    world
        .asset_definition(&definition_id)
        .is_ok()
        .then(|| definition_id.to_string())
}

fn retarget_namespace_policy_payment_asset(
    policy: &mut SuffixPolicyV1,
    payment_asset_id: &str,
) -> bool {
    let mut changed = false;
    if policy.payment_asset_id != payment_asset_id {
        policy.payment_asset_id = payment_asset_id.to_owned();
        changed = true;
    }
    for tier in &mut policy.pricing {
        if tier.base_price.asset_id != payment_asset_id {
            tier.base_price.asset_id = payment_asset_id.to_owned();
            changed = true;
        }
    }
    if changed {
        policy.policy_version = policy.policy_version.saturating_add(1);
    }
    changed
}

fn ensure_policy_matches_configured_payment_asset(
    world: &impl WorldReadOnly,
    policy: &SuffixPolicyV1,
    configured_fee_asset_selector: &str,
) -> Result<(), SnsError> {
    let configured_payment_asset_id =
        resolve_configured_payment_asset_literal(world, configured_fee_asset_selector)
            .ok_or_else(|| {
                SnsError::BadRequest(
                    "configured Nexus fee asset is not a canonical asset definition id or a registered alias"
                        .to_owned(),
                )
            })?;
    let configured_definition_id =
        AssetDefinitionId::parse_address_literal(&configured_payment_asset_id).map_err(|err| {
            SnsError::BadRequest(format!(
                "configured Nexus fee asset is not a canonical asset definition id: {err}"
            ))
        })?;
    if world.asset_definition(&configured_definition_id).is_err() {
        return Err(SnsError::NotFound(format!(
            "configured Nexus fee asset `{configured_payment_asset_id}` is not registered"
        )));
    }
    if policy.payment_asset_id != configured_payment_asset_id {
        return Err(SnsError::Conflict(format!(
            "SNS policy payment asset `{}` does not match configured Nexus fee asset `{configured_payment_asset_id}`",
            policy.payment_asset_id
        )));
    }
    if policy
        .pricing
        .iter()
        .any(|tier| tier.base_price.asset_id != configured_payment_asset_id)
    {
        return Err(SnsError::Conflict(format!(
            "SNS policy pricing tiers do not match configured Nexus fee asset `{configured_payment_asset_id}`"
        )));
    }
    Ok(())
}

/// Ensure one namespace policy is exactly pinned to the configured, registered Nexus fee asset.
pub fn ensure_namespace_policy_payment_asset_matches_configured(
    world: &impl WorldReadOnly,
    namespace: SnsNamespace,
    configured_fee_asset_selector: &str,
) -> Result<(), SnsError> {
    let policy = policy_or_not_found(world, namespace.suffix_id())?;
    ensure_policy_matches_configured_payment_asset(world, &policy, configured_fee_asset_selector)
}

/// Seed the fixed namespace policies required by the on-chain SNS model.
pub fn seed_default_namespace_policies(world: &mut World) {
    let steward = bootstrap_steward_for_world(&world.view());
    for namespace in [
        SnsNamespace::AccountAlias,
        SnsNamespace::Domain,
        SnsNamespace::Dataspace,
    ] {
        let policy = default_namespace_policy(
            namespace,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        let key = policy_storage_key(policy.suffix_id);
        let existing_policy = world
            .smart_contract_state
            .view()
            .get(&key)
            .map(|bytes| decode_policy_for_suffix(bytes, namespace.suffix_id()));
        match existing_policy {
            Some(Ok(mut existing_policy)) => {
                if upgrade_legacy_default_namespace_policy(namespace, &mut existing_policy) {
                    world
                        .smart_contract_state
                        .insert(key, existing_policy.encode());
                }
            }
            // Existing malformed or mis-keyed state is evidence of corruption. Never replace it
            // with a default value and accidentally turn a fail-closed read into valid policy.
            Some(Err(_)) => {}
            None => {
                world.smart_contract_state.insert(key, policy.encode());
            }
        }
    }
}

/// Align seeded default SNS namespace policy charges with the configured Nexus fee asset.
///
/// Default policies are seeded before deployment-specific Nexus configuration is applied.  When a
/// deployment uses a non-default XOR asset id, the fixed SNS policies must stop quoting leases in
/// the stale compile-time default or Torii onboarding will submit payments for a missing asset.
pub fn sync_default_namespace_policy_payment_asset(
    world: &mut World,
    configured_fee_asset_selector: &str,
) -> bool {
    let Some(configured_payment_asset_id) =
        resolve_configured_payment_asset_literal(&world.view(), configured_fee_asset_selector)
    else {
        return false;
    };

    let steward = bootstrap_steward_for_world(&world.view());
    let mut changed = false;
    for namespace in [
        SnsNamespace::AccountAlias,
        SnsNamespace::Domain,
        SnsNamespace::Dataspace,
    ] {
        let key = policy_storage_key(namespace.suffix_id());
        let existing_policy = world
            .smart_contract_state
            .view()
            .get(&key)
            .map(|bytes| decode_policy_for_suffix(bytes, namespace.suffix_id()));
        let mut policy = match existing_policy {
            Some(Ok(policy)) => policy,
            Some(Err(_)) => continue,
            None => {
                world.smart_contract_state.insert(
                    key,
                    default_namespace_policy(namespace, &steward, &configured_payment_asset_id)
                        .encode(),
                );
                changed = true;
                continue;
            }
        };

        let mut policy_changed = upgrade_legacy_default_namespace_policy(namespace, &mut policy);
        policy_changed |=
            retarget_namespace_policy_payment_asset(&mut policy, &configured_payment_asset_id);

        if policy_changed {
            world.smart_contract_state.insert(key, policy.encode());
            changed = true;
        }
    }
    changed
}

fn years_to_ms(years: u8) -> u64 {
    u64::from(years).saturating_mul(MS_PER_YEAR)
}

fn enforce_policy_active(policy: &SuffixPolicyV1) -> Result<(), SnsError> {
    match policy.status {
        SuffixStatus::Active => Ok(()),
        SuffixStatus::Paused => Err(SnsError::Conflict(format!(
            "suffix `{}` is paused",
            policy.suffix_key()
        ))),
        SuffixStatus::Revoked => Err(SnsError::Conflict(format!(
            "suffix `{}` is revoked",
            policy.suffix_key()
        ))),
    }
}

fn tier_regex(tier: &PriceTierV1) -> Result<Regex, SnsError> {
    Regex::new(&tier.label_regex).map_err(|err| {
        SnsError::Conflict(format!(
            "pricing tier {} has invalid label regex: {err}",
            tier.tier_id
        ))
    })
}

fn label_matches_tier(tier: &PriceTierV1, label: &str) -> Result<bool, SnsError> {
    Ok(tier_regex(tier)?.is_match(label))
}

fn pick_pricing_tier(
    policy: &SuffixPolicyV1,
    selector: &NameSelectorV1,
    pricing_class_hint: Option<u8>,
) -> Result<PriceTierV1, SnsError> {
    let label = selector.normalized_label();
    if let Some(hint) = pricing_class_hint {
        let tier = policy
            .pricing
            .iter()
            .find(|tier| tier.tier_id == hint)
            .ok_or_else(|| {
                SnsError::BadRequest(format!(
                    "pricing class {hint} is not offered for suffix `{}`",
                    policy.suffix_key()
                ))
            })?;
        if !label_matches_tier(tier, label)? {
            return Err(SnsError::BadRequest(format!(
                "label `{label}` does not satisfy pricing class {hint}"
            )));
        }
        return Ok(tier.clone());
    }

    for tier in &policy.pricing {
        if label_matches_tier(tier, label)? {
            return Ok(tier.clone());
        }
    }

    Err(SnsError::BadRequest(format!(
        "label `{label}` does not match any pricing tier for suffix `{}`",
        policy.suffix_key()
    )))
}

fn tier_by_pricing_class(
    policy: &SuffixPolicyV1,
    selector: &NameSelectorV1,
    pricing_class: u8,
) -> Result<PriceTierV1, SnsError> {
    let label = selector.normalized_label();
    let tier = policy
        .pricing
        .iter()
        .find(|tier| tier.tier_id == pricing_class)
        .ok_or_else(|| {
            SnsError::BadRequest(format!(
                "pricing class {pricing_class} is not offered for suffix `{}`",
                policy.suffix_key()
            ))
        })?;
    if !label_matches_tier(tier, label)? {
        return Err(SnsError::BadRequest(format!(
            "label `{label}` no longer satisfies pricing class {pricing_class}"
        )));
    }
    Ok(tier.clone())
}

fn validate_term_bounds(
    policy: &SuffixPolicyV1,
    tier: &PriceTierV1,
    term_years: u8,
) -> Result<(), SnsError> {
    let min_years = policy.min_term_years.max(tier.min_duration_years);
    let max_years = policy.max_term_years.min(tier.max_duration_years);
    if min_years > max_years {
        return Err(SnsError::Conflict(format!(
            "suffix `{}` has incompatible policy/tier term bounds",
            policy.suffix_key()
        )));
    }
    if term_years < min_years || term_years > max_years {
        return Err(SnsError::BadRequest(format!(
            "term_years must be between {min_years} and {max_years} (got {term_years})"
        )));
    }
    Ok(())
}

fn validate_payment_for_term(
    policy: &SuffixPolicyV1,
    tier: &PriceTierV1,
    term_years: u8,
    payment: &LeasePayment,
) -> Result<(), SnsError> {
    if payment.asset_id != policy.payment_asset_id {
        return Err(SnsError::BadRequest(format!(
            "payment asset `{}` does not match required asset `{}`",
            payment.asset_id, policy.payment_asset_id
        )));
    }
    if payment.net_amount > payment.gross_amount {
        return Err(SnsError::BadRequest(
            "net_amount must not exceed gross_amount".to_owned(),
        ));
    }
    let required = tier
        .base_price
        .amount
        .try_mul_decimal(&Numeric::from(u32::from(term_years)))
        .map_err(|_| {
            SnsError::Conflict(format!(
                "required payment overflowed for pricing class {}",
                tier.tier_id
            ))
        })?;
    if payment.gross_amount < required || payment.net_amount < required {
        return Err(SnsError::BadRequest(format!(
            "payment ({}/{} {}) does not meet required amount {} for term {term_years}",
            payment.net_amount, payment.gross_amount, payment.asset_id, required
        )));
    }
    Ok(())
}

fn required_payment_amount(tier: &PriceTierV1, term_years: u8) -> Result<Quantity, SnsError> {
    tier.base_price
        .amount
        .try_mul_decimal(&Numeric::from(u32::from(term_years)))
        .map_err(|_| {
            SnsError::Conflict(format!(
                "required payment overflowed for pricing class {}",
                tier.tier_id
            ))
        })
}

fn payment_asset_definition_id(policy: &SuffixPolicyV1) -> Result<AssetDefinitionId, SnsError> {
    if let Ok(asset_id) = AssetId::parse_literal(&policy.payment_asset_id) {
        return Ok(asset_id.definition().clone());
    }
    AssetDefinitionId::parse_address_literal(&policy.payment_asset_id).map_err(|err| {
        SnsError::Conflict(format!(
            "suffix `{}` has invalid payment asset `{}`: {err}",
            policy.suffix_key(),
            policy.payment_asset_id
        ))
    })
}

fn lease_quote(
    selector: NameSelectorV1,
    policy: &SuffixPolicyV1,
    tier: &PriceTierV1,
    term_years: u8,
    base_expires_at_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    validate_term_bounds(policy, tier, term_years)?;
    let charge_amount = required_payment_amount(tier, term_years)?;
    let payment_asset_definition_id = payment_asset_definition_id(policy)?;
    let expires_at_ms = base_expires_at_ms.saturating_add(years_to_ms(term_years));
    let grace_expires_at_ms =
        expires_at_ms.saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
    let redemption_expires_at_ms =
        grace_expires_at_ms.saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);
    Ok(LeaseQuote {
        selector,
        pricing_class: tier.tier_id,
        payment_asset_id: policy.payment_asset_id.clone(),
        payment_asset_definition_id,
        collector_account: policy.fund_splitter_account.clone(),
        charge_amount,
        expires_at_ms,
        grace_expires_at_ms,
        redemption_expires_at_ms,
    })
}

fn maybe_auction_state(tier: &PriceTierV1, now_ms: u64) -> Option<NameAuctionStateV1> {
    match tier.auction_kind {
        AuctionKind::VickreyCommitReveal => None,
        AuctionKind::DutchReopen => Some(NameAuctionStateV1 {
            kind: tier.auction_kind,
            opened_at_ms: now_ms,
            closes_at_ms: now_ms.saturating_add(3 * MS_PER_DAY),
            floor_price: tier
                .dutch_floor
                .clone()
                .unwrap_or_else(|| tier.base_price.clone()),
            highest_commitment: None,
            settlement_tx: None,
        }),
    }
}

fn refresh_lifecycle(record: &mut NameRecordV1, now_ms: u64) {
    if matches!(record.status, NameStatus::Tombstoned(_)) {
        return;
    }
    if let NameStatus::Frozen(frozen) = &record.status
        && now_ms < frozen.until_ms
    {
        return;
    }
    record.status = effective_status(record, now_ms);
}

/// Validate a controller set before any fee-bearing SNS mutation is attempted.
pub(crate) fn validate_name_controllers(controllers: &[NameControllerV1]) -> Result<(), SnsError> {
    if controllers.is_empty() {
        return Err(SnsError::BadRequest(
            "at least one controller must be provided".to_owned(),
        ));
    }
    for (index, controller) in controllers.iter().enumerate() {
        if controllers[..index].contains(controller) {
            return Err(SnsError::BadRequest(
                "controller list contains a duplicate entry".to_owned(),
            ));
        }
        let valid_shape = match controller.controller_type {
            ControllerType::Account | ControllerType::Multisig => {
                controller.account_address.is_some() && controller.resolver_template_id.is_none()
            }
            ControllerType::ResolverTemplate => {
                controller.account_address.is_none()
                    && controller
                        .resolver_template_id
                        .as_deref()
                        .is_some_and(|id| !id.trim().is_empty())
            }
            ControllerType::ExternalLink => {
                controller.account_address.is_none() && controller.resolver_template_id.is_none()
            }
        };
        if !valid_shape {
            return Err(SnsError::BadRequest(format!(
                "controller at index {index} does not match its controller type"
            )));
        }
    }
    Ok(())
}

fn registration_record(
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,
    payment: &LeasePayment,
    metadata: Metadata,
    policy: &SuffixPolicyV1,
    tier: &PriceTierV1,
    now_ms: u64,
) -> Result<NameRecordV1, SnsError> {
    enforce_policy_active(policy)?;
    validate_name_controllers(&controllers)?;
    validate_term_bounds(policy, tier, term_years)?;
    validate_payment_for_term(policy, tier, term_years, payment)?;
    let expires_at_ms = now_ms.saturating_add(years_to_ms(term_years));
    let grace_expires_at_ms =
        expires_at_ms.saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
    let redemption_expires_at_ms =
        grace_expires_at_ms.saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);

    Ok(NameRecordV1 {
        selector: selector.clone(),
        name_hash: selector.name_hash(),
        owner,
        ownership_generation: 1,
        controllers,
        status: NameStatus::Active,
        pricing_class: tier.tier_id,
        registered_at_ms: now_ms,
        expires_at_ms,
        grace_expires_at_ms,
        redemption_expires_at_ms,
        metadata,
        auction: maybe_auction_state(tier, now_ms),
    })
}

fn reserved_label_key(namespace: SnsNamespace, selector: &NameSelectorV1) -> &str {
    let literal = selector.normalized_label();
    match namespace {
        SnsNamespace::AccountAlias => literal.split_once('@').map_or(literal, |(label, _)| label),
        SnsNamespace::Domain => literal.split_once('.').map_or(literal, |(label, _)| label),
        SnsNamespace::Dataspace => literal,
    }
}

fn find_active_reserved_label<'a>(
    namespace: SnsNamespace,
    policy: &'a SuffixPolicyV1,
    selector: &NameSelectorV1,
    now_ms: u64,
) -> Option<&'a ReservedNameV1> {
    let literal = selector.normalized_label();
    let label_key = reserved_label_key(namespace, selector);
    policy.reserved_labels.iter().find(|reserved| {
        reserved
            .release_at_ms
            .is_none_or(|release_at_ms| now_ms < release_at_ms)
            && (reserved.normalized_label == literal || reserved.normalized_label == label_key)
    })
}

fn enforce_reserved_label_assignment(
    namespace: SnsNamespace,
    policy: &SuffixPolicyV1,
    selector: &NameSelectorV1,
    owner: &AccountId,
    now_ms: u64,
) -> Result<(), SnsError> {
    let Some(reserved) = find_active_reserved_label(namespace, policy, selector, now_ms) else {
        return Ok(());
    };

    match &reserved.assigned_to {
        Some(assignee) if assignee == owner => Ok(()),
        Some(assignee) => Err(SnsError::Conflict(format!(
            "label `{}` is reserved for `{assignee}`",
            reserved.normalized_label
        ))),
        None => Err(SnsError::Conflict(format!(
            "label `{}` is reserved",
            reserved.normalized_label
        ))),
    }
}

fn is_reserved_universal_selector(selector: &NameSelectorV1) -> bool {
    selector.suffix_id == DATASPACE_ALIAS_SUFFIX_ID
        && selector.normalized_label() == RESERVED_UNIVERSAL_DATASPACE_ALIAS
}

fn ensure_selector_is_mutable(selector: &NameSelectorV1) -> Result<(), SnsError> {
    if is_reserved_universal_selector(selector) {
        return Err(SnsError::Conflict(
            "reserved dataspace alias `universal` is immutable".to_owned(),
        ));
    }
    Ok(())
}

fn canonicalize_request_selector(
    selector: NameSelectorV1,
    catalog: &DataSpaceCatalog,
) -> Result<(SnsNamespace, NameSelectorV1), SnsError> {
    let namespace = SnsNamespace::from_suffix_id(selector.suffix_id)?;
    let canonical = match namespace {
        SnsNamespace::AccountAlias => selector_for_account_alias_literal(&selector.label, catalog)?,
        SnsNamespace::Domain => {
            let domain = DomainId::parse_fully_qualified(selector.label.trim())
                .map_err(|err| SnsError::BadRequest(err.reason().to_owned()))?;
            selector_for_domain(&domain).map_err(|err| SnsError::BadRequest(err.to_string()))?
        }
        SnsNamespace::Dataspace => NameSelectorV1::new(selector.suffix_id, selector.label)
            .map_err(|err| SnsError::BadRequest(err.to_string()))?,
    };
    Ok((namespace, canonical))
}

fn canonicalize_resolved_selector(
    selector: NameSelectorV1,
) -> Result<(SnsNamespace, NameSelectorV1), SnsError> {
    if selector.version != NameSelectorV1::VERSION {
        return Err(SnsError::BadRequest(format!(
            "unsupported SNS selector version `{}`",
            selector.version
        )));
    }
    let namespace = SnsNamespace::from_suffix_id(selector.suffix_id)?;
    let canonical = match namespace {
        SnsNamespace::AccountAlias => {
            let alias = selector
                .label
                .parse::<AccountAliasName>()
                .map_err(|err| SnsError::BadRequest(err.to_string()))?;
            NameSelectorV1 {
                version: NameSelectorV1::VERSION,
                suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
                label: alias.canonical_text(),
            }
        }
        SnsNamespace::Domain => {
            let domain = DomainId::parse_fully_qualified(selector.label.trim())
                .map_err(|err| SnsError::BadRequest(err.reason().to_owned()))?;
            selector_for_domain(&domain).map_err(|err| SnsError::BadRequest(err.to_string()))?
        }
        SnsNamespace::Dataspace => selector_for_dataspace_alias(&selector.label)
            .map_err(|err| SnsError::BadRequest(err.to_string()))?,
    };
    Ok((namespace, canonical))
}

fn record_or_not_found(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
) -> Result<NameRecordV1, SnsError> {
    let key = record_storage_key(selector);
    let bytes = world.smart_contract_state().get(&key).ok_or_else(|| {
        SnsError::NotFound(format!(
            "registration `{}` not found",
            selector.normalized_label()
        ))
    })?;
    decode_record_for_selector(bytes, selector)
}

fn policy_or_not_found(
    world: &impl WorldReadOnly,
    suffix_id: SuffixId,
) -> Result<SuffixPolicyV1, SnsError> {
    let key = policy_storage_key(suffix_id);
    let bytes = world.smart_contract_state().get(&key).ok_or_else(|| {
        SnsError::NotFound(format!("suffix policy {suffix_id} is not registered"))
    })?;
    decode_policy_for_suffix(bytes, suffix_id)
}

/// Fetch a SNS record by namespace/literal and apply the current lifecycle view.
///
/// # Errors
///
/// Returns [`SnsError`] when the namespace or literal is invalid or the record
/// is missing from authoritative state.
pub fn get_name_record(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    namespace: SnsNamespace,
    literal: &str,
    now_ms: u64,
) -> Result<NameRecordV1, SnsError> {
    let selector = selector_for_namespace_literal(namespace, literal, catalog)?;
    get_name_record_by_selector(world, &selector, now_ms)
}

/// Fetch a SNS record by pre-canonicalized selector and apply the current lifecycle view.
///
/// # Errors
///
/// Returns [`SnsError`] when the record is missing from authoritative state.
pub fn get_name_record_by_selector(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
    now_ms: u64,
) -> Result<NameRecordV1, SnsError> {
    let mut record = record_or_not_found(world, selector)?;
    refresh_lifecycle(&mut record, now_ms);
    Ok(record)
}

/// Build the internal payment record after Core debits the exact quote.
#[must_use]
pub(crate) fn native_payment_for_quote(quote: &LeaseQuote) -> LeasePayment {
    LeasePayment {
        asset_id: quote.payment_asset_id.clone(),
        gross_amount: quote.charge_amount.clone(),
        net_amount: quote.charge_amount.clone(),
    }
}

/// Quote the cost and resulting lifecycle for acquiring an account-alias lease.
///
/// # Errors
///
/// Returns [`SnsError`] when the alias is invalid, already registered, or does
/// not satisfy the active suffix policy.
pub fn quote_account_alias_registration(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    owner: &AccountId,
    term_years: u8,
    pricing_class_hint: Option<u8>,
    now_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    let selector = selector_for_account_alias(alias, catalog)
        .map_err(|err| SnsError::BadRequest(err.to_string()))?;
    ensure_selector_is_mutable(&selector)?;
    let policy = policy_or_not_found(world, selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    enforce_reserved_label_assignment(
        SnsNamespace::AccountAlias,
        &policy,
        &selector,
        owner,
        now_ms,
    )?;
    if record_by_selector(world, &selector).is_some() {
        return Err(SnsError::Conflict(format!(
            "selector `{}` is already registered",
            selector.normalized_label()
        )));
    }
    let tier = pick_pricing_tier(&policy, &selector, pricing_class_hint)?;
    lease_quote(selector, &policy, &tier, term_years, now_ms)
}

/// Quote account-alias registration only when policy and configured fee asset agree exactly.
pub fn quote_account_alias_registration_with_configured_fee_asset(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    owner: &AccountId,
    term_years: u8,
    pricing_class_hint: Option<u8>,
    now_ms: u64,
    configured_fee_asset_selector: &str,
) -> Result<LeaseQuote, SnsError> {
    let selector = selector_for_account_alias(alias, catalog)
        .map_err(|err| SnsError::BadRequest(err.to_string()))?;
    ensure_selector_is_mutable(&selector)?;
    let policy = policy_or_not_found(world, selector.suffix_id)?;
    ensure_policy_matches_configured_payment_asset(world, &policy, configured_fee_asset_selector)?;
    enforce_policy_active(&policy)?;
    enforce_reserved_label_assignment(
        SnsNamespace::AccountAlias,
        &policy,
        &selector,
        owner,
        now_ms,
    )?;
    if record_by_selector(world, &selector).is_some() {
        return Err(SnsError::Conflict(format!(
            "selector `{}` is already registered",
            selector.normalized_label()
        )));
    }
    let tier = pick_pricing_tier(&policy, &selector, pricing_class_hint)?;
    lease_quote(selector, &policy, &tier, term_years, now_ms)
}

/// Quote the cost and resulting lifecycle for renewing an account-alias lease.
///
/// # Errors
///
/// Returns [`SnsError`] when the alias is missing, immutable, or no longer
/// eligible for renewal.
pub fn quote_account_alias_renewal(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    term_years: u8,
    now_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    let selector = selector_for_account_alias(alias, catalog)
        .map_err(|err| SnsError::BadRequest(err.to_string()))?;
    ensure_selector_is_mutable(&selector)?;
    let policy = policy_or_not_found(world, selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    let mut record = record_or_not_found(world, &selector)?;
    refresh_lifecycle(&mut record, now_ms);
    match record.status {
        NameStatus::Tombstoned(_) => {
            return Err(SnsError::Conflict(format!(
                "registration `{}` is tombstoned",
                selector.normalized_label()
            )));
        }
        NameStatus::Frozen(_) => {
            return Err(SnsError::Conflict(format!(
                "registration `{}` is frozen",
                selector.normalized_label()
            )));
        }
        _ => {}
    }
    let tier = tier_by_pricing_class(&policy, &record.selector, record.pricing_class)?;
    lease_quote(selector, &policy, &tier, term_years, record.expires_at_ms)
}

/// Quote account-alias renewal only when policy and configured fee asset agree exactly.
pub fn quote_account_alias_renewal_with_configured_fee_asset(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    term_years: u8,
    now_ms: u64,
    configured_fee_asset_selector: &str,
) -> Result<LeaseQuote, SnsError> {
    let selector = selector_for_account_alias(alias, catalog)
        .map_err(|err| SnsError::BadRequest(err.to_string()))?;
    ensure_selector_is_mutable(&selector)?;
    let policy = policy_or_not_found(world, selector.suffix_id)?;
    ensure_policy_matches_configured_payment_asset(world, &policy, configured_fee_asset_selector)?;
    enforce_policy_active(&policy)?;
    let mut record = record_or_not_found(world, &selector)?;
    refresh_lifecycle(&mut record, now_ms);
    match record.status {
        NameStatus::Tombstoned(_) => {
            return Err(SnsError::Conflict(format!(
                "registration `{}` is tombstoned",
                selector.normalized_label()
            )));
        }
        NameStatus::Frozen(_) => {
            return Err(SnsError::Conflict(format!(
                "registration `{}` is frozen",
                selector.normalized_label()
            )));
        }
        _ => {}
    }
    let tier = tier_by_pricing_class(&policy, &record.selector, record.pricing_class)?;
    lease_quote(selector, &policy, &tier, term_years, record.expires_at_ms)
}

/// Quote the cost and resulting lifecycle for registering a SNS name.
///
/// # Errors
///
/// Returns [`SnsError`] when the selector is invalid, the suffix policy is missing or inactive,
/// the label is reserved for another owner, or the name is already registered.
pub fn quote_name_registration(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    selector: NameSelectorV1,
    owner: &AccountId,
    term_years: u8,
    pricing_class_hint: Option<u8>,
    now_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    let (namespace, canonical_selector) = canonicalize_request_selector(selector, catalog)?;
    ensure_selector_is_mutable(&canonical_selector)?;
    let policy = policy_or_not_found(world, canonical_selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    enforce_reserved_label_assignment(namespace, &policy, &canonical_selector, owner, now_ms)?;
    if record_by_selector(world, &canonical_selector).is_some() {
        return Err(SnsError::Conflict(format!(
            "selector `{}` is already registered",
            canonical_selector.normalized_label()
        )));
    }
    let tier = pick_pricing_tier(&policy, &canonical_selector, pricing_class_hint)?;
    lease_quote(canonical_selector, &policy, &tier, term_years, now_ms)
}

/// Quote registration for a catalog-free, pre-resolved canonical selector.
///
/// Unlike [`quote_name_registration`], account aliases are parsed from their
/// complete textual form and do not require a static dataspace catalog. Callers
/// must separately validate the textual dataspace against its pinned numeric ID.
///
/// # Errors
///
/// Returns [`SnsError`] when the selector or lease terms are invalid, the
/// suffix policy is unavailable, the label is reserved, or the name exists.
pub fn quote_resolved_name_registration(
    world: &impl WorldReadOnly,
    selector: NameSelectorV1,
    owner: &AccountId,
    term_years: u8,
    pricing_class_hint: Option<u8>,
    now_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    let (namespace, canonical_selector) = canonicalize_resolved_selector(selector)?;
    ensure_selector_is_mutable(&canonical_selector)?;
    let policy = policy_or_not_found(world, canonical_selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    enforce_reserved_label_assignment(namespace, &policy, &canonical_selector, owner, now_ms)?;
    if record_by_selector(world, &canonical_selector).is_some() {
        return Err(SnsError::Conflict(format!(
            "selector `{}` is already registered",
            canonical_selector.normalized_label()
        )));
    }
    let tier = pick_pricing_tier(&policy, &canonical_selector, pricing_class_hint)?;
    lease_quote(canonical_selector, &policy, &tier, term_years, now_ms)
}

fn resolved_renewal_term_years(
    current_expiry_ms: u64,
    target_expiry_ms: u64,
) -> Result<u8, SnsError> {
    let extension_ms = target_expiry_ms
        .checked_sub(current_expiry_ms)
        .ok_or_else(|| {
            SnsError::BadRequest(
                "target lease expiry must be later than the current expiry".to_owned(),
            )
        })?;
    if extension_ms == 0 || extension_ms % MS_PER_YEAR != 0 {
        return Err(SnsError::BadRequest(format!(
            "target lease expiry must extend the current expiry by a whole number of {MS_PER_YEAR}ms years"
        )));
    }
    u8::try_from(extension_ms / MS_PER_YEAR)
        .map_err(|_| SnsError::BadRequest("target lease extension exceeds 255 years".to_owned()))
}

fn ensure_record_renewable(record: &NameRecordV1) -> Result<(), SnsError> {
    match &record.status {
        NameStatus::Tombstoned(_) => Err(SnsError::Conflict(format!(
            "registration `{}` is tombstoned",
            record.selector.normalized_label()
        ))),
        NameStatus::Frozen(_) => Err(SnsError::Conflict(format!(
            "registration `{}` is frozen",
            record.selector.normalized_label()
        ))),
        NameStatus::Active | NameStatus::GracePeriod | NameStatus::Redemption => Ok(()),
    }
}

/// Quote a catalog-free lease renewal using expiry compare-and-set and an absolute target.
///
/// # Errors
///
/// Returns [`SnsError`] when the selector or policy is invalid, the current
/// expiry differs, the record is frozen/tombstoned, or the target does not add
/// an allowed whole-year term.
pub fn quote_resolved_name_renewal(
    world: &impl WorldReadOnly,
    selector: NameSelectorV1,
    expected_current_expiry_ms: u64,
    target_expiry_ms: u64,
    now_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    let (_, canonical_selector) = canonicalize_resolved_selector(selector)?;
    ensure_selector_is_mutable(&canonical_selector)?;
    let policy = policy_or_not_found(world, canonical_selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    let mut record = record_or_not_found(world, &canonical_selector)?;
    refresh_lifecycle(&mut record, now_ms);
    ensure_record_renewable(&record)?;
    if record.expires_at_ms != expected_current_expiry_ms {
        return Err(SnsError::Conflict(format!(
            "alias.lease.expiry_conflict: expected current expiry {expected_current_expiry_ms}, actual expiry is {}",
            record.expires_at_ms
        )));
    }
    let term_years = resolved_renewal_term_years(expected_current_expiry_ms, target_expiry_ms)?;
    let tier = tier_by_pricing_class(&policy, &record.selector, record.pricing_class)?;
    let quote = lease_quote(
        canonical_selector,
        &policy,
        &tier,
        term_years,
        expected_current_expiry_ms,
    )?;
    if quote.expires_at_ms != target_expiry_ms {
        return Err(SnsError::Conflict(
            "renewal quote did not reproduce the absolute target expiry".to_owned(),
        ));
    }
    Ok(quote)
}

/// Quote the cost and resulting lifecycle for renewing a SNS name.
///
/// # Errors
///
/// Returns [`SnsError`] when the name or policy is missing, immutable, tombstoned, or no longer
/// satisfies the pricing class used for the original registration.
pub fn quote_name_renewal(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    namespace: SnsNamespace,
    literal: &str,
    term_years: u8,
    now_ms: u64,
) -> Result<LeaseQuote, SnsError> {
    let selector = selector_for_namespace_literal(namespace, literal, catalog)?;
    ensure_selector_is_mutable(&selector)?;
    let policy = policy_or_not_found(world, selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    let mut record = record_or_not_found(world, &selector)?;
    refresh_lifecycle(&mut record, now_ms);
    if matches!(record.status, NameStatus::Tombstoned(_)) {
        return Err(SnsError::Conflict(format!(
            "registration `{}` is tombstoned",
            selector.normalized_label()
        )));
    }
    let tier = tier_by_pricing_class(&policy, &record.selector, record.pricing_class)?;
    lease_quote(selector, &policy, &tier, term_years, record.expires_at_ms)
}

fn persist_record(state_transaction: &mut StateTransaction<'_, '_>, record: &NameRecordV1) {
    state_transaction
        .world
        .smart_contract_state
        .insert(record_storage_key(&record.selector), record.encode());
}

/// Register a new SNS name in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the selector is invalid, the policy is missing or
/// inactive, or a record already exists for the same canonical selector.
#[cfg(test)]
fn register_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    request: RegisterNameInput,
) -> Result<NameRecordV1, SnsError> {
    register_name_with_selector(state_transaction, request, canonicalize_request_selector)
}

/// Register a catalog-free, pre-resolved SNS selector in authoritative state.
///
/// The caller must first revalidate the selector's textual dataspace against
/// the numeric ID carried by the resolved setup intent.
pub(crate) fn register_resolved_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    request: RegisterNameInput,
) -> Result<NameRecordV1, SnsError> {
    register_name_with_selector(state_transaction, request, |selector, _catalog| {
        canonicalize_resolved_selector(selector)
    })
}

/// Apply a catalog-free absolute-expiry renewal after exact payment was charged.
pub(crate) fn renew_resolved_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    selector: NameSelectorV1,
    expected_current_expiry_ms: u64,
    target_expiry_ms: u64,
    payment: LeasePayment,
) -> Result<NameRecordV1, SnsError> {
    let (_, canonical_selector) = canonicalize_resolved_selector(selector)?;
    ensure_selector_is_mutable(&canonical_selector)?;
    let policy = policy_or_not_found(state_transaction.world(), canonical_selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    let now_ms = state_transaction.block_unix_timestamp_ms();
    let mut record = record_or_not_found(state_transaction.world(), &canonical_selector)?;
    refresh_lifecycle(&mut record, now_ms);
    ensure_record_renewable(&record)?;
    if record.expires_at_ms != expected_current_expiry_ms {
        return Err(SnsError::Conflict(format!(
            "alias.lease.expiry_conflict: expected current expiry {expected_current_expiry_ms}, actual expiry is {}",
            record.expires_at_ms
        )));
    }
    let term_years = resolved_renewal_term_years(expected_current_expiry_ms, target_expiry_ms)?;
    let tier = tier_by_pricing_class(&policy, &record.selector, record.pricing_class)?;
    validate_term_bounds(&policy, &tier, term_years)?;
    validate_payment_for_term(&policy, &tier, term_years, &payment)?;
    record.expires_at_ms = target_expiry_ms;
    record.grace_expires_at_ms =
        target_expiry_ms.saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
    record.redemption_expires_at_ms = record
        .grace_expires_at_ms
        .saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);
    refresh_lifecycle(&mut record, now_ms);
    persist_record(state_transaction, &record);
    Ok(record)
}

fn register_name_with_selector(
    state_transaction: &mut StateTransaction<'_, '_>,
    request: RegisterNameInput,
    canonicalize: impl FnOnce(
        NameSelectorV1,
        &DataSpaceCatalog,
    ) -> Result<(SnsNamespace, NameSelectorV1), SnsError>,
) -> Result<NameRecordV1, SnsError> {
    let RegisterNameInput {
        selector,
        owner,
        controllers,
        term_years,
        pricing_class_hint,
        payment,
        metadata,
    } = request;

    let (namespace, canonical_selector) =
        canonicalize(selector, &state_transaction.nexus.dataspace_catalog)?;
    ensure_selector_is_mutable(&canonical_selector)?;
    let policy = policy_or_not_found(state_transaction.world(), canonical_selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    let now_ms = state_transaction.block_unix_timestamp_ms();
    enforce_reserved_label_assignment(namespace, &policy, &canonical_selector, &owner, now_ms)?;
    let key = record_storage_key(&canonical_selector);
    if state_transaction
        .world
        .smart_contract_state
        .get(&key)
        .is_some()
    {
        return Err(SnsError::Conflict(format!(
            "selector `{}` is already registered",
            canonical_selector.normalized_label()
        )));
    }
    let tier = pick_pricing_tier(&policy, &canonical_selector, pricing_class_hint)?;
    let record = registration_record(
        canonical_selector,
        owner,
        controllers,
        term_years,
        &payment,
        metadata,
        &policy,
        &tier,
        now_ms,
    )?;
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Set the absolute lease expiry for an existing SNS name in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the record or policy is missing, the selector is immutable, the
/// record is tombstoned, or the requested expiry is not in the future.
#[cfg(test)]
fn set_name_lease_expiry(
    state_transaction: &mut StateTransaction<'_, '_>,
    namespace: SnsNamespace,
    literal: &str,
    expires_at_ms: u64,
) -> Result<NameRecordV1, SnsError> {
    let selector = selector_for_namespace_literal(
        namespace,
        literal,
        &state_transaction.nexus.dataspace_catalog,
    )?;
    ensure_selector_is_mutable(&selector)?;
    let policy = policy_or_not_found(state_transaction.world(), selector.suffix_id)?;
    enforce_policy_active(&policy)?;
    let mut record = record_or_not_found(state_transaction.world(), &selector)?;
    let now_ms = state_transaction.block_unix_timestamp_ms();
    refresh_lifecycle(&mut record, now_ms);
    if matches!(record.status, NameStatus::Tombstoned(_)) {
        return Err(SnsError::Conflict(format!(
            "registration `{}` is tombstoned",
            selector.normalized_label()
        )));
    }
    if expires_at_ms <= now_ms {
        return Err(SnsError::BadRequest(
            "lease_expiry_ms must be greater than the current block timestamp".to_owned(),
        ));
    }

    record.expires_at_ms = expires_at_ms;
    record.grace_expires_at_ms =
        expires_at_ms.saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
    record.redemption_expires_at_ms = record
        .grace_expires_at_ms
        .saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);
    refresh_lifecycle(&mut record, now_ms);
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Apply a ledger-backed SNS mutation in a dedicated state block for unit tests.
///
/// # Errors
///
/// Returns [`SnsError`] when the mutation fails or the state block cannot be
/// committed.
#[cfg(test)]
pub fn apply_with_state_block<T>(
    state: &State,
    mutation: impl FnOnce(&mut StateTransaction<'_, '_>) -> Result<T, SnsError>,
) -> Result<T, SnsError> {
    let latest_block = state.view().latest_block();
    let next_height = latest_block
        .as_ref()
        .map(|block| block.header().height().get().saturating_add(1))
        .unwrap_or(1);
    let prev_hash = latest_block.as_ref().map(|block| block.as_ref().hash());
    let ledger_time_ms = latest_block
        .as_ref()
        .map(|block| u64::try_from(block.header().creation_time().as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let wall_clock_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(ledger_time_ms);
    let header = BlockHeader::new(
        next_height
            .try_into()
            .expect("block height must always fit into NonZeroU64"),
        prev_hash,
        None,
        None,
        wall_clock_ms.max(ledger_time_ms),
        0,
    );
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    let out = mutation(&mut transaction)?;
    transaction.apply();
    block
        .commit()
        .map_err(|err| SnsError::Internal(format!("failed to commit SNS state block: {err}")))?;
    Ok(out)
}

/// Compute the effective lifecycle for `record` using deterministic ledger time.
#[must_use]
pub fn effective_status(record: &NameRecordV1, now_ms: u64) -> NameStatus {
    if matches!(record.status, NameStatus::Tombstoned(_)) {
        return record.status.clone();
    }
    if let NameStatus::Frozen(frozen) = &record.status
        && now_ms < frozen.until_ms
    {
        return record.status.clone();
    }

    if now_ms >= record.redemption_expires_at_ms {
        NameStatus::Tombstoned(NameTombstoneStateV1 {
            reason: EXPIRED_TOMBSTONE_REASON.to_owned(),
        })
    } else if now_ms >= record.grace_expires_at_ms {
        NameStatus::Redemption
    } else if now_ms >= record.expires_at_ms {
        NameStatus::GracePeriod
    } else {
        NameStatus::Active
    }
}

/// Return the active owner for a SNS selector when the record lifecycle is `Active`.
#[must_use]
pub fn active_owner_by_selector(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
    now_ms: u64,
) -> Option<AccountId> {
    let record = record_by_selector(world, selector)?;
    matches!(effective_status(&record, now_ms), NameStatus::Active).then_some(record.owner)
}

/// Return the active owner for a full account-alias lease record.
#[must_use]
pub fn active_account_alias_owner(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    now_ms: u64,
) -> Option<AccountId> {
    let selector = active_account_alias_selector(world, catalog, alias, now_ms).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
}

/// Resolve an account alias only when its authoritative lease and both canonical binding indexes
/// agree on an existing account at deterministic ledger time.
///
/// Missing, corrupt, expired, frozen, grace-period, redemption, tombstoned, or split-brain state
/// deliberately resolves to `None`.
#[must_use]
pub fn resolve_active_account_alias(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    now_ms: u64,
) -> Option<AccountId> {
    let lease_owner = active_account_alias_owner(world, catalog, alias, now_ms)?;
    let indexed_owner = world.account_aliases().get(alias)?;
    let rekey_owner = &world.account_rekey_records().get(alias)?.active_account_id;
    if indexed_owner != rekey_owner || indexed_owner != &lease_owner {
        return None;
    }
    world.account(indexed_owner).ok()?;
    Some(indexed_owner.clone())
}

fn active_account_id_rekey_suffix_for_alias<'world>(
    world: &'world impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    now_ms: u64,
) -> Result<Option<(AccountId, &'world [AccountId])>, ()> {
    let Some(active_account_id) = resolve_active_account_alias(world, catalog, alias, now_ms)
    else {
        return Ok(None);
    };
    let record = world.account_rekey_records().get(alias).ok_or(())?;
    if &record.label != alias || record.active_account_id != active_account_id {
        return Err(());
    }
    let predecessors = record
        .active_account_id_rekey_predecessors()
        .map_err(|_| ())?;
    let mut unique_predecessors = BTreeSet::new();
    for predecessor in predecessors {
        if predecessor == &active_account_id
            || !unique_predecessors.insert(predecessor.clone())
            || world.account(predecessor).is_ok()
        {
            return Err(());
        }
    }
    Ok(Some((active_account_id, predecessors)))
}

/// Resolve an account id through one exact alias's active, explicitly proven account-id rekey
/// suffix.
///
/// Legacy history and ordinary alias reassignment are permanently non-authorizing. The alias
/// lease, forward index, continuity record, and active account must agree at `now_ms`; every
/// predecessor in the active suffix must be unique and retired.
#[must_use]
pub fn resolve_active_account_id_rekey_lineage_for_alias(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    account_id: &AccountId,
    now_ms: u64,
) -> Option<AccountId> {
    let (active_account_id, predecessors) =
        active_account_id_rekey_suffix_for_alias(world, catalog, alias, now_ms)
            .ok()
            .flatten()?;
    (account_id == &active_account_id || predecessors.contains(account_id))
        .then_some(active_account_id)
}

/// Resolve an account id to its unique active account-id rekey target across all live aliases.
///
/// A currently registered account resolves to itself. Any malformed live suffix, reused retired
/// predecessor, cycle, or conflicting active target fails closed.
#[must_use]
pub fn resolve_active_account_id_rekey_lineage(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    account_id: &AccountId,
    now_ms: u64,
) -> Option<AccountId> {
    let mut resolved = world.account(account_id).ok().map(|_| account_id.clone());
    for (alias, _) in world.account_rekey_records().iter() {
        let Some((active_account_id, predecessors)) =
            active_account_id_rekey_suffix_for_alias(world, catalog, alias, now_ms).ok()?
        else {
            continue;
        };
        if account_id != &active_account_id && !predecessors.contains(account_id) {
            continue;
        }
        if resolved
            .as_ref()
            .is_some_and(|existing| existing != &active_account_id)
        {
            return None;
        }
        resolved = Some(active_account_id);
    }
    resolved
}

/// Return the active owner for a domain-name lease record.
#[must_use]
pub fn active_domain_owner(
    world: &impl WorldReadOnly,
    domain: &DomainId,
    now_ms: u64,
) -> Option<AccountId> {
    let selector = selector_for_domain(domain).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
}

/// Return the active owner for a canonical dataspace alias.
#[must_use]
pub fn active_dataspace_owner_by_alias(
    world: &impl WorldReadOnly,
    alias: &str,
    now_ms: u64,
) -> Option<AccountId> {
    active_dataspace_owner_and_generation_by_alias(world, alias, now_ms).map(|(owner, _)| owner)
}

/// Return the active owner and monotonic ownership generation for a dataspace alias.
///
/// A malformed zero generation fails closed so a signed namespace delegation can never bind to
/// an inert or legacy-reset ownership epoch.
#[must_use]
pub fn active_dataspace_owner_and_generation_by_alias(
    world: &impl WorldReadOnly,
    alias: &str,
    now_ms: u64,
) -> Option<(AccountId, u64)> {
    let selector = selector_for_dataspace_alias(alias).ok()?;
    let record = record_by_selector(world, &selector)?;
    (matches!(effective_status(&record, now_ms), NameStatus::Active)
        && record.ownership_generation != 0)
        .then_some((record.owner, record.ownership_generation))
}

fn active_dataspace_record_id(record: &NameRecordV1) -> Result<DataSpaceId, SnsError> {
    if let Some(encoded_id) = record.metadata.get(SNS_DATASPACE_ID_METADATA_KEY) {
        let raw = norito::json::from_str::<u64>(encoded_id.get()).map_err(|err| {
            SnsError::Conflict(format!(
                "{ALIAS_CATALOG_MAPPING_CONFLICT_CODE}: dataspace alias `{}` stores an invalid numeric id: {err}",
                record.selector.normalized_label()
            ))
        })?;
        return Ok(DataSpaceId::new(raw));
    }
    dataspace_id_for_sns_alias(record.selector.normalized_label()).ok_or_else(|| {
        SnsError::Internal(format!(
            "failed to derive dataspace id for canonical alias `{}`",
            record.selector.normalized_label()
        ))
    })
}

struct ActiveDataspaceResolution {
    alias: String,
}

fn resolve_active_dataspace_by_id(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    dataspace_id: DataSpaceId,
    now_ms: u64,
) -> Result<ActiveDataspaceResolution, SnsError> {
    let prefix = StatePath::from_str(&format!("sns/records/{DATASPACE_ALIAS_SUFFIX_ID}/"))
        .expect("static dataspace SNS record prefix is valid");
    let prefix_literal = prefix.as_ref().to_owned();
    let mut resolution = catalog
        .by_id(dataspace_id)
        .map(|entry| {
            if entry.alias.len() > iroha_data_model::name::MAX_NAME_BYTES {
                return Err(SnsError::Conflict(format!(
                    "{ALIAS_CATALOG_MAPPING_CONFLICT_CODE}: configured dataspace alias exceeds the canonical name limit"
                )));
            }
            Ok(ActiveDataspaceResolution {
                alias: entry.alias.clone(),
            })
        })
        .transpose()?;

    for (storage_key, bytes) in world.smart_contract_state().range(prefix..) {
        if !storage_key.as_ref().starts_with(&prefix_literal) {
            break;
        }
        let decode_candidate = || {
            let mut slice = bytes.as_slice();
            let record = NameRecordV1::decode(&mut slice).map_err(|_| {
                SnsError::Internal("failed to decode a dataspace SNS record".to_owned())
            })?;
            if !slice.is_empty() {
                return Err(SnsError::Internal(
                    "dataspace SNS record contains trailing bytes".to_owned(),
                ));
            }
            if record.selector.label.len() > iroha_data_model::name::MAX_NAME_BYTES {
                return Err(SnsError::Internal(
                    "dataspace SNS record label exceeds the canonical name limit".to_owned(),
                ));
            }
            if record.selector.suffix_id != DATASPACE_ALIAS_SUFFIX_ID
                || record.name_hash != record.selector.name_hash()
                || record_storage_key(&record.selector).as_ref() != storage_key.as_ref()
            {
                return Err(SnsError::Internal(
                    "dataspace SNS record identity mismatch".to_owned(),
                ));
            }
            if !matches!(effective_status(&record, now_ms), NameStatus::Active) {
                return Ok(None);
            }

            let dynamic_id = active_dataspace_record_id(&record)?;
            let alias = record.selector.normalized_label();
            if let Some(static_entry) = catalog.by_alias(alias)
                && static_entry.id != dynamic_id
                && (static_entry.id == dataspace_id || dynamic_id == dataspace_id)
            {
                return Err(SnsError::Conflict(format!(
                    "{ALIAS_CATALOG_MAPPING_CONFLICT_CODE}: active SNS and configured dataspace mappings disagree"
                )));
            }
            if dynamic_id != dataspace_id {
                return Ok(None);
            }
            Ok(Some(record.selector.label))
        };
        let candidate = if crate::smartcontracts::isi::query::singular_query_limits_active() {
            let elements = bytes.len().checked_mul(8).ok_or_else(|| {
                SnsError::Internal("dataspace SNS record exceeds query memory limits".to_owned())
            })?;
            let limits = crate::smartcontracts::isi::query::singular_query_decode_limits(
                bytes.len(),
                norito::DecodeLimits::new(elements, bytes.len(), elements, usize::MAX, 64),
            )
            .map_err(|_| {
                SnsError::Internal("dataspace SNS record exceeds query memory limits".to_owned())
            })?;
            norito::with_decode_limits_scope(limits, decode_candidate)
        } else {
            decode_candidate()
        }?;

        if let Some(alias) = candidate {
            match &mut resolution {
                None => {
                    resolution = Some(ActiveDataspaceResolution { alias });
                }
                Some(existing) if existing.alias == alias => {}
                Some(_) => {
                    return Err(SnsError::Conflict(format!(
                        "{ALIAS_CATALOG_MAPPING_CONFLICT_CODE}: dataspace id maps to multiple active aliases"
                    )));
                }
            }
        }
    }

    resolution.ok_or_else(|| SnsError::NotFound(format!("unknown dataspace id `{dataspace_id}`")))
}

/// Resolve a dataspace alias against both the static catalog and active SNS state.
///
/// Static and dynamic mappings are independent evidence for the same canonical
/// text-to-id pair. If both are present they must agree exactly; a caller must
/// never silently prefer one directory over the other.
///
/// # Errors
///
/// Returns [`SnsError::NotFound`] when neither directory knows the alias,
/// [`SnsError::BadRequest`] when the alias is not canonical, and
/// [`SnsError::Conflict`] with [`ALIAS_CATALOG_MAPPING_CONFLICT_CODE`] when the
/// two directories disagree.
pub fn resolve_active_dataspace_id_by_alias(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &str,
    now_ms: u64,
) -> Result<DataSpaceId, SnsError> {
    let alias = alias.trim();
    let selector =
        selector_for_dataspace_alias(alias).map_err(|err| SnsError::BadRequest(err.to_string()))?;
    let static_id = catalog
        .by_alias(selector.normalized_label())
        .map(|entry| entry.id);
    let dynamic_id = record_by_selector(world, &selector)
        .filter(|record| matches!(effective_status(record, now_ms), NameStatus::Active))
        .map(|record| active_dataspace_record_id(&record))
        .transpose()?;

    let resolved_id = match (static_id, dynamic_id) {
        (Some(static_id), Some(dynamic_id)) if static_id != dynamic_id => {
            return Err(SnsError::Conflict(format!(
                "{ALIAS_CATALOG_MAPPING_CONFLICT_CODE}: dataspace alias `{}` maps to static id {static_id} and active SNS id {dynamic_id}",
                selector.normalized_label()
            )));
        }
        (Some(id), _) | (None, Some(id)) => id,
        (None, None) => {
            return Err(SnsError::NotFound(format!(
                "unknown dataspace alias `{}`",
                selector.normalized_label()
            )));
        }
    };
    let reverse_alias = resolve_active_dataspace_alias_by_id(world, catalog, resolved_id, now_ms)?;
    if reverse_alias != selector.normalized_label() {
        return Err(SnsError::Conflict(format!(
            "{ALIAS_CATALOG_MAPPING_CONFLICT_CODE}: dataspace alias `{}` maps to id {resolved_id}, whose canonical active name is `{reverse_alias}`",
            selector.normalized_label()
        )));
    }
    Ok(resolved_id)
}

/// Resolve a dataspace id to its unique canonical alias across the static catalog and active SNS.
///
/// Static and dynamic mappings must describe one exact text/id pair. Multiple active names for
/// the same numeric id, or a disagreement between the directories, fail closed instead of
/// selecting an arbitrary spelling.
///
/// # Errors
///
/// Returns [`SnsError::NotFound`] when neither directory knows the id,
/// [`SnsError::Conflict`] with [`ALIAS_CATALOG_MAPPING_CONFLICT_CODE`] when the id has multiple
/// canonical names or a static/dynamic mapping disagrees, and [`SnsError::Internal`] for malformed
/// authoritative SNS state.
pub fn resolve_active_dataspace_alias_by_id(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    dataspace_id: DataSpaceId,
    now_ms: u64,
) -> Result<String, SnsError> {
    resolve_active_dataspace_by_id(world, catalog, dataspace_id, now_ms)
        .map(|resolution| resolution.alias)
}

/// Render an account alias with the unique active dataspace name for its numeric id.
///
/// # Errors
///
/// Returns [`SnsError`] when the dataspace mapping is unknown, conflicting, or the resulting
/// account-alias literal is invalid.
pub fn active_account_alias_literal(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    now_ms: u64,
) -> Result<String, SnsError> {
    let dataspace = resolve_active_dataspace_alias_by_id(world, catalog, alias.dataspace, now_ms)?;
    AccountAliasName::try_new(
        alias.label.as_ref(),
        alias.domain.as_ref().map(|domain| domain.name().as_ref()),
        dataspace,
    )
    .map(|name| name.to_string())
    .map_err(|error| SnsError::BadRequest(error.to_string()))
}

/// Build the authoritative selector for an account alias using live/static dataspace resolution.
///
/// # Errors
///
/// Returns [`SnsError`] when the dataspace mapping or account-alias literal is invalid.
pub fn active_account_alias_selector(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &AccountAlias,
    now_ms: u64,
) -> Result<NameSelectorV1, SnsError> {
    let literal = active_account_alias_literal(world, catalog, alias, now_ms)?;
    Ok(NameSelectorV1 {
        version: NameSelectorV1::VERSION,
        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
        label: literal,
    })
}

/// Resolve an active dataspace alias to its canonical id.
///
/// This convenience projection fails closed for unknown or conflicting mappings.
#[must_use]
pub fn active_dataspace_id_by_alias(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &str,
    now_ms: u64,
) -> Option<DataSpaceId> {
    resolve_active_dataspace_id_by_alias(world, catalog, alias, now_ms).ok()
}

/// Resolve active dataspace metadata from the bootstrap catalog or SNS.
#[must_use]
pub fn active_dataspace_metadata_by_alias(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    alias: &str,
    now_ms: u64,
) -> Option<DataSpaceMetadata> {
    let alias = alias.trim();
    let resolved_id = resolve_active_dataspace_id_by_alias(world, catalog, alias, now_ms).ok()?;
    if let Some(entry) = catalog.by_alias(alias) {
        return Some(entry.clone());
    }
    let selector = selector_for_dataspace_alias(alias).ok()?;
    active_owner_by_selector(world, &selector, now_ms)?;
    Some(DataSpaceMetadata {
        id: resolved_id,
        alias: selector.label,
        description: Some("ledger-backed SNS dataspace".to_owned()),
        fault_tolerance: SNS_DYNAMIC_DATASPACE_FAULT_TOLERANCE,
    })
}

/// Resolve the active owner for the dataspace id using the current catalog alias.
#[must_use]
pub fn active_dataspace_owner_by_id(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    dataspace_id: DataSpaceId,
    now_ms: u64,
) -> Option<AccountId> {
    let resolution = resolve_active_dataspace_by_id(world, catalog, dataspace_id, now_ms).ok()?;
    active_dataspace_owner_by_alias(world, &resolution.alias, now_ms)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        Registrable,
        account::{
            Account, AccountAddress, AccountId,
            rekey::{AccountAlias, AccountAliasDomain},
        },
        alias_setup::{
            AccountAliasName, AccountAliasRoleV1, AccountProvisionV1, AliasAccountIntentV1,
            AliasAutoRenewStateV1, AliasDataSpaceIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1,
            AliasQuoteGuardV1, AliasTargetV1, ResolvedAccountAliasV1, ResolvedDataSpaceV1,
        },
        asset::{AssetDefinition, AssetDefinitionId},
        block::SignedBlock,
        domain::Domain,
        isi::{InstructionBox, Register, alias_setup::EnsureAlias},
        metadata::Metadata,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
        sns::{
            NameControllerV1, NameFrozenStateV1, NameRecordV1, NameSelectorV1, NameStatus,
            NameTombstoneStateV1,
        },
        transaction::TransactionBuilder,
    };

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    include!("sns_core_tests.rs");

    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }

    fn another_owner() -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("derive alternate SNS fixture owner");
        AccountId::new(keypair.public_key().clone())
    }

    fn dataspace_catalog() -> DataSpaceCatalog {
        DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "banking".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(10),
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("catalog")
    }

    fn world_with_payment_asset(definition_id: AssetDefinitionId) -> World {
        let authority = owner();
        let domain_id = DomainId::try_new("issuer", "universal").expect("domain");
        let domain = Domain::new(domain_id).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let definition = AssetDefinition::numeric(
            definition_id,
            "xor".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        )
        .build(&authority);
        World::with([domain], [account], [definition])
    }

    fn controller(owner: &AccountId) -> NameControllerV1 {
        let address =
            AccountAddress::from_account_id(owner).expect("should encode account address");
        NameControllerV1::account(&address)
    }

    fn default_payment(_owner: &AccountId) -> LeasePayment {
        LeasePayment {
            asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
            gross_amount: default_namespace_lease_price(),
            net_amount: default_namespace_lease_price(),
        }
    }

    fn dataspace_record(
        alias: &str,
        owner: &AccountId,
        expires_at_ms: u64,
        grace_expires_at_ms: u64,
        redemption_expires_at_ms: u64,
    ) -> (NameSelectorV1, NameRecordV1) {
        let selector = selector_for_dataspace_alias(alias).expect("selector");
        let address = AccountAddress::from_account_id(owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            10,
            expires_at_ms,
            grace_expires_at_ms,
            redemption_expires_at_ms,
            Metadata::default(),
        );
        (selector, record)
    }

    fn world_with_dataspace_record(selector: &NameSelectorV1, record: &NameRecordV1) -> World {
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(selector), record.encode());
        world
    }

    #[test]
    fn account_alias_selector_uses_canonical_literal() {
        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("treasury".parse().expect("label"), DataSpaceId::new(7));

        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");

        assert_eq!(selector.suffix_id, ACCOUNT_ALIAS_SUFFIX_ID);
        assert_eq!(selector.label, "treasury@banking");
    }

    #[test]
    fn active_account_alias_selector_resolves_canonical_domainful_literal() {
        let catalog = dataspace_catalog();
        let alias = AccountAlias::new(
            "treasury".parse().expect("label"),
            Some(AccountAliasDomain::new("banka".parse().expect("domain"))),
            DataSpaceId::new(7),
        );
        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");
        let owner = owner();
        let account = Account::new(owner.clone()).build(&owner);
        let mut world = World::with([], [account], []);
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            0,
            100,
            200,
            300,
            Metadata::default(),
        );
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
        world.account_aliases.insert(alias.clone(), owner.clone());
        world.account_rekey_records.insert(
            alias.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(alias.clone(), owner.clone()),
        );

        assert_eq!(selector.label, "treasury@banka.banking");
        assert_eq!(
            active_account_alias_selector(&world.view(), &catalog, &alias, 50)
                .expect("active selector"),
            selector,
        );
        assert_eq!(
            resolve_active_account_alias(&world.view(), &catalog, &alias, 50),
            Some(owner),
        );
    }

    #[test]
    fn active_account_alias_selector_resolves_dynamic_only_dataspace() {
        let catalog = DataSpaceCatalog::default();
        let dataspace = DataSpaceId::new(42);
        let alias = AccountAlias::domainless("treasury".parse().expect("label"), dataspace);
        assert!(
            selector_for_account_alias(&alias, &catalog).is_err(),
            "the bootstrap catalog must not know the dynamic-only dataspace"
        );

        let owner = owner();
        let dataspace_selector =
            selector_for_dataspace_alias("paynet").expect("dynamic dataspace selector");
        let mut metadata = Metadata::default();
        metadata.insert(
            SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            iroha_primitives::json::Json::new(dataspace.as_u64()),
        );
        let dataspace_record = NameRecordV1::new(
            dataspace_selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            0,
            100,
            200,
            300,
            metadata,
        );
        let mut world = World::default();
        world.smart_contract_state_mut_for_testing().insert(
            record_storage_key(&dataspace_selector),
            dataspace_record.encode(),
        );

        let selector = active_account_alias_selector(&world.view(), &catalog, &alias, 50)
            .expect("live dynamic dataspace mapping should build the selector");
        assert_eq!(selector.suffix_id, ACCOUNT_ALIAS_SUFFIX_ID);
        assert_eq!(selector.label, "treasury@paynet");
    }

    #[test]
    fn account_alias_selector_rejects_malformed_reserved_separator_literals() {
        let catalog = dataspace_catalog();
        for literal in [
            "treasury#banka.banking",
            "treas$ury@banka.banking",
            "treasury@@banka.banking",
            "treasury@banka@banking",
            "treasury@banka.banking.extra",
        ] {
            assert!(
                selector_for_account_alias_literal(literal, &catalog).is_err(),
                "malformed account-alias selector must be rejected: {literal}",
            );
        }
    }

    #[test]
    fn account_alias_resolution_requires_active_lease_and_consistent_indexes() {
        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("resolver".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");
        let owner = owner();
        let other = another_owner();
        let account = Account::new(owner.clone()).build(&owner);
        let other_account = Account::new(other.clone()).build(&owner);
        let mut world = World::with([], [account, other_account], []);
        let mut record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            0,
            100,
            200,
            300,
            Metadata::default(),
        );
        let key = record_storage_key(&selector);
        world
            .smart_contract_state_mut_for_testing()
            .insert(key.clone(), record.encode());
        world.account_aliases.insert(alias.clone(), owner.clone());
        world.account_rekey_records.insert(
            alias.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(alias.clone(), owner.clone()),
        );

        assert_eq!(
            resolve_active_account_alias(&world.view(), &catalog, &alias, 50),
            Some(owner.clone())
        );

        record.status = NameStatus::Frozen(NameFrozenStateV1 {
            reason: "hold".to_owned(),
            until_ms: 90,
        });
        world
            .smart_contract_state_mut_for_testing()
            .insert(key.clone(), record.encode());
        assert_eq!(
            resolve_active_account_alias(&world.view(), &catalog, &alias, 50),
            None
        );

        record.status = NameStatus::Active;
        record.expires_at_ms = 40;
        record.grace_expires_at_ms = 45;
        record.redemption_expires_at_ms = 50;
        world
            .smart_contract_state_mut_for_testing()
            .insert(key.clone(), record.encode());
        assert_eq!(
            resolve_active_account_alias(&world.view(), &catalog, &alias, 50),
            None
        );

        record.status = NameStatus::Tombstoned(NameTombstoneStateV1 {
            reason: "revoked".to_owned(),
        });
        record.expires_at_ms = 100;
        record.grace_expires_at_ms = 200;
        record.redemption_expires_at_ms = 300;
        world
            .smart_contract_state_mut_for_testing()
            .insert(key.clone(), record.encode());
        assert_eq!(
            resolve_active_account_alias(&world.view(), &catalog, &alias, 50),
            None
        );

        record.status = NameStatus::Active;
        world
            .smart_contract_state_mut_for_testing()
            .insert(key, record.encode());
        world.account_rekey_records.insert(
            alias.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(alias.clone(), other),
        );
        assert_eq!(
            resolve_active_account_alias(&world.view(), &catalog, &alias, 50),
            None,
            "split binding indexes must fail closed"
        );
    }

    #[test]
    fn account_id_rekey_lineage_requires_typed_live_unambiguous_retired_history() {
        use iroha_data_model::account::rekey::{
            AccountRekeyRecord, AccountRekeyTransitionProvenance,
        };

        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("lineage".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");
        let retired = checked_account_id();
        let active = checked_account_id();
        let unrelated = checked_account_id();
        let mut world = World::with(
            [],
            [
                Account::new(active.clone()).build(&active),
                Account::new(unrelated.clone()).build(&active),
            ],
            [],
        );
        let mut lease = NameRecordV1::new(
            selector.clone(),
            active.clone(),
            vec![controller(&active)],
            0,
            0,
            100,
            200,
            300,
            Metadata::default(),
        );
        let storage_key = record_storage_key(&selector);
        world
            .smart_contract_state_mut_for_testing()
            .insert(storage_key.clone(), lease.encode());
        world.account_aliases.insert(alias.clone(), active.clone());
        let canonical = AccountRekeyRecord::new(alias.clone(), retired.clone())
            .repoint_for_account_id_rekey(active.clone())
            .expect("canonical account-id rekey fixture");
        world
            .account_rekey_records
            .insert(alias.clone(), canonical.clone());

        assert_eq!(
            resolve_active_account_id_rekey_lineage_for_alias(
                &world.view(),
                &catalog,
                &alias,
                &retired,
                50,
            ),
            Some(active.clone())
        );
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            Some(active.clone())
        );
        assert_eq!(
            resolve_active_account_id_rekey_lineage_for_alias(
                &world.view(),
                &catalog,
                &alias,
                &unrelated,
                50,
            ),
            None,
            "an unrelated account must not join the lineage"
        );

        lease.expires_at_ms = 40;
        lease.grace_expires_at_ms = 45;
        lease.redemption_expires_at_ms = 50;
        world
            .smart_contract_state_mut_for_testing()
            .insert(storage_key.clone(), lease.encode());
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            None,
            "expired lineage lease must fail closed"
        );

        lease.status = NameStatus::Tombstoned(NameTombstoneStateV1 {
            reason: "revoked".to_owned(),
        });
        lease.expires_at_ms = 100;
        lease.grace_expires_at_ms = 200;
        lease.redemption_expires_at_ms = 300;
        world
            .smart_contract_state_mut_for_testing()
            .insert(storage_key.clone(), lease.encode());
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            None,
            "revoked lineage lease must fail closed"
        );

        lease.status = NameStatus::Active;
        world
            .smart_contract_state_mut_for_testing()
            .insert(storage_key.clone(), lease.encode());
        world.account_rekey_records.insert(
            alias.clone(),
            AccountRekeyRecord::new(alias.clone(), retired.clone())
                .reassign_alias_to_account(active.clone())
                .expect("alias reassignment fixture"),
        );
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            None,
            "ordinary alias reassignment must break controller lineage"
        );

        let mut malformed = canonical.clone();
        malformed.previous_account_ids.push(retired.clone());
        malformed
            .transition_provenance
            .push(AccountRekeyTransitionProvenance::AccountIdRekey);
        world.account_rekey_records.insert(alias.clone(), malformed);
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            None,
            "duplicate predecessor history must fail closed"
        );

        let mut cyclic = canonical.clone();
        cyclic.previous_account_ids.push(active.clone());
        cyclic
            .transition_provenance
            .push(AccountRekeyTransitionProvenance::AccountIdRekey);
        world.account_rekey_records.insert(alias.clone(), cyclic);
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            None,
            "active-id cycles must fail closed"
        );

        world.account_rekey_records.insert(alias.clone(), canonical);
        let second_alias =
            AccountAlias::domainless("ambiguous".parse().expect("label"), DataSpaceId::UNIVERSAL);
        let second_selector =
            selector_for_account_alias(&second_alias, &catalog).expect("selector");
        let second_lease = NameRecordV1::new(
            second_selector.clone(),
            unrelated.clone(),
            vec![controller(&unrelated)],
            0,
            0,
            100,
            200,
            300,
            Metadata::default(),
        );
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&second_selector), second_lease.encode());
        world
            .account_aliases
            .insert(second_alias.clone(), unrelated.clone());
        world.account_rekey_records.insert(
            second_alias.clone(),
            AccountRekeyRecord::new(second_alias, retired.clone())
                .repoint_for_account_id_rekey(unrelated)
                .expect("ambiguous fixture transition"),
        );
        assert_eq!(
            resolve_active_account_id_rekey_lineage(&world.view(), &catalog, &retired, 50),
            None,
            "one retired predecessor cannot resolve to two active targets"
        );
    }

    #[test]
    fn sns_namespace_from_path_accepts_account_alias_spelling_variants() {
        assert_eq!(
            SnsNamespace::from_path("account-alias").expect("hyphenated namespace"),
            SnsNamespace::AccountAlias
        );
        assert_eq!(
            SnsNamespace::from_path("account_alias").expect("underscored namespace"),
            SnsNamespace::AccountAlias
        );
    }

    #[test]
    fn sns_namespace_from_path_rejects_unknown_value() {
        let err = SnsNamespace::from_path("mystery").expect_err("unknown path must fail");
        assert!(
            err.to_string().contains("unknown SNS namespace"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn sns_namespace_from_suffix_id_rejects_unknown_value() {
        let err = SnsNamespace::from_suffix_id(0xFFFF).expect_err("unknown suffix id must fail");
        assert!(
            err.to_string().contains("unsupported SNS suffix id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn active_dataspace_owner_reads_from_world_storage() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let owner = another_owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let mut record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        record.metadata.insert(
            SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            IrohaJson::new(7_u64),
        );

        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
        let view = world.view();

        assert_eq!(
            active_dataspace_owner_by_id(&view, &catalog, DataSpaceId::new(7), 50),
            Some(owner)
        );
    }

    #[test]
    fn active_dataspace_id_rejects_conflicting_static_and_dynamic_mappings() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let owner = another_owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner,
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());

        let error = resolve_active_dataspace_id_by_alias(&world.view(), &catalog, "banking", 50);
        let error = error.expect_err("conflicting directories must fail closed");
        assert!(
            error
                .to_string()
                .contains(ALIAS_CATALOG_MAPPING_CONFLICT_CODE),
            "unexpected error: {error}"
        );
        assert_eq!(
            active_dataspace_id_by_alias(&world.view(), &catalog, "banking", 50),
            None
        );
    }

    #[test]
    fn active_dataspace_id_accepts_matching_static_and_dynamic_mappings() {
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let expected_id = DataSpaceId::from_hash(&selector.name_hash());
        let catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: expected_id,
            alias: "banking".to_owned(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("matching dataspace catalog");
        let owner = another_owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner,
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());

        assert_eq!(
            resolve_active_dataspace_id_by_alias(&world.view(), &catalog, "banking", 50)
                .expect("matching directories"),
            expected_id
        );
    }

    #[test]
    fn active_dataspace_id_accepts_explicit_dynamic_id_matching_static_catalog() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let owner = another_owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let mut record = NameRecordV1::new(
            selector.clone(),
            owner,
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        record.metadata.insert(
            SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            IrohaJson::new(7_u64),
        );
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());

        assert_eq!(
            resolve_active_dataspace_id_by_alias(&world.view(), &catalog, "banking", 50)
                .expect("matching explicit mapping"),
            DataSpaceId::new(7)
        );
    }

    #[test]
    fn active_dataspace_id_derives_from_dynamic_sns_alias() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("alpha").expect("selector");
        let expected_id = DataSpaceId::from_hash(&selector.name_hash());
        let owner = another_owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let record = NameRecordV1::new(
            selector.clone(),
            owner,
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());

        let view = world.view();
        assert_eq!(
            active_dataspace_id_by_alias(&view, &catalog, "alpha", 50),
            Some(expected_id)
        );
        let metadata =
            active_dataspace_metadata_by_alias(&view, &catalog, "alpha", 50).expect("metadata");
        assert_eq!(metadata.id, expected_id);
        assert_eq!(metadata.alias, "alpha");
        assert_eq!(
            resolve_active_dataspace_alias_by_id(&view, &catalog, expected_id, 50)
                .expect("dynamic reverse mapping"),
            "alpha"
        );
    }

    mod active_dataspace_alias_tests {
        include!("sns/active_dataspace_alias_tests.rs");
    }

    #[test]
    fn active_dataspace_alias_by_id_rejects_static_dynamic_name_collision() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let owner = another_owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let conflicting_id = DataSpaceId::new(8);
        let mut record = NameRecordV1::new(
            selector.clone(),
            owner,
            vec![NameControllerV1::account(&address)],
            0,
            10,
            110,
            210,
            310,
            Metadata::default(),
        );
        record.metadata.insert(
            SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            IrohaJson::new(conflicting_id.as_u64()),
        );
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());

        for id in [DataSpaceId::new(7), conflicting_id] {
            let error = resolve_active_dataspace_alias_by_id(&world.view(), &catalog, id, 50)
                .expect_err("both sides of a mapping collision must fail closed");
            assert!(
                error
                    .to_string()
                    .contains(ALIAS_CATALOG_MAPPING_CONFLICT_CODE),
                "unexpected error for {id}: {error}"
            );
        }
    }

    #[test]
    fn dataspace_id_for_sns_alias_treats_universal_as_reserved_case_insensitively() {
        assert_eq!(
            dataspace_id_for_sns_alias("universal"),
            Some(DataSpaceId::UNIVERSAL)
        );
        assert_eq!(
            dataspace_id_for_sns_alias(" Universal "),
            Some(DataSpaceId::UNIVERSAL)
        );
    }

    #[test]
    fn dataspace_id_for_sns_alias_rejects_empty_or_invalid_aliases() {
        assert_eq!(dataspace_id_for_sns_alias(""), None);
        assert_eq!(dataspace_id_for_sns_alias("   "), None);
        assert_eq!(dataspace_id_for_sns_alias("not valid"), None);
    }

    #[test]
    fn active_dataspace_id_returns_none_for_unregistered_dynamic_alias() {
        let catalog = dataspace_catalog();
        let world = World::default();

        assert_eq!(
            active_dataspace_id_by_alias(&world.view(), &catalog, "alpha", 50),
            None
        );
        assert_eq!(
            active_dataspace_metadata_by_alias(&world.view(), &catalog, "alpha", 50),
            None
        );
        let error = resolve_active_dataspace_id_by_alias(&world.view(), &catalog, "alpha", 50)
            .expect_err("unknown mapping must fail");
        assert!(error.to_string().contains("unknown dataspace alias"));
    }

    #[test]
    fn active_dataspace_id_ignores_expired_dynamic_alias() {
        let catalog = dataspace_catalog();
        let owner = another_owner();
        let (selector, record) = dataspace_record("alpha", &owner, 10, 20, 30);
        let world = world_with_dataspace_record(&selector, &record);

        assert_eq!(
            active_dataspace_id_by_alias(&world.view(), &catalog, "alpha", 10),
            None
        );
        assert_eq!(
            active_dataspace_owner_by_alias(&world.view(), "alpha", 25),
            None
        );
    }

    #[test]
    fn active_dataspace_id_ignores_frozen_or_tombstoned_dynamic_alias() {
        let catalog = dataspace_catalog();
        let owner = another_owner();
        let (frozen_selector, mut frozen_record) =
            dataspace_record("frozen-alpha", &owner, 100, 200, 300);
        frozen_record.status = NameStatus::Frozen(NameFrozenStateV1 {
            reason: "governance hold".to_owned(),
            until_ms: 90,
        });
        let frozen_world = world_with_dataspace_record(&frozen_selector, &frozen_record);
        assert_eq!(
            active_dataspace_id_by_alias(&frozen_world.view(), &catalog, "frozen-alpha", 50),
            None
        );

        let (tombstoned_selector, mut tombstoned_record) =
            dataspace_record("retired-alpha", &owner, 100, 200, 300);
        tombstoned_record.status = NameStatus::Tombstoned(NameTombstoneStateV1 {
            reason: "retired".to_owned(),
        });
        let tombstoned_world =
            world_with_dataspace_record(&tombstoned_selector, &tombstoned_record);
        assert_eq!(
            active_dataspace_id_by_alias(&tombstoned_world.view(), &catalog, "retired-alpha", 50),
            None
        );
    }

    #[test]
    fn quote_account_alias_registration_uses_default_policy_price_and_term() {
        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("treasury".parse().expect("label"), DataSpaceId::new(7));
        let owner = owner();
        let mut world = World::default();
        seed_default_namespace_policies(&mut world);
        let view = world.view();

        let quote = quote_account_alias_registration(&view, &catalog, &alias, &owner, 2, None, 100)
            .expect("registration quote");

        assert_eq!(quote.selector.label, "treasury@banking");
        assert_eq!(quote.payment_asset_id, "61CtjvNd9T3THAR65GsMVHr82Bjc");
        assert_eq!(quote.charge_amount, Quantity::one());
        assert_eq!(quote.expires_at_ms, 100 + years_to_ms(2));
    }

    #[test]
    fn sync_default_namespace_policy_payment_asset_pins_canonical_asset_before_registration() {
        let payment_asset_definition_id =
            AssetDefinitionId::parse_address_literal("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
                .expect("deployment XOR asset id");
        let payment_asset_literal = payment_asset_definition_id.to_string();
        let mut world = World::default();
        seed_default_namespace_policies(&mut world);

        assert!(sync_default_namespace_policy_payment_asset(
            &mut world,
            &payment_asset_literal
        ));

        for namespace in [
            SnsNamespace::AccountAlias,
            SnsNamespace::Domain,
            SnsNamespace::Dataspace,
        ] {
            let key = policy_storage_key(namespace.suffix_id());
            let policy = world
                .smart_contract_state
                .view()
                .get(&key)
                .and_then(|bytes| SuffixPolicyV1::decode(&mut bytes.as_slice()).ok())
                .expect("namespace policy");
            assert_eq!(policy.payment_asset_id, payment_asset_literal);
            assert!(
                policy
                    .pricing
                    .iter()
                    .all(|tier| tier.base_price.asset_id == payment_asset_literal)
            );
            assert_eq!(policy.policy_version, 2);
        }
    }

    #[test]
    fn configured_fee_asset_quote_rejects_stale_policy_until_explicit_state_convergence() {
        let payment_asset_definition_id =
            AssetDefinitionId::parse_address_literal("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
                .expect("deployment XOR asset id");
        let payment_asset_literal = payment_asset_definition_id.to_string();
        let mut world = world_with_payment_asset(payment_asset_definition_id.clone());
        seed_default_namespace_policies(&mut world);

        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("treasury".parse().expect("label"), DataSpaceId::new(7));
        let err = quote_account_alias_registration_with_configured_fee_asset(
            &world.view(),
            &catalog,
            &alias,
            &owner(),
            2,
            None,
            100,
            &payment_asset_literal,
        )
        .expect_err("read-only quote must not virtually retarget a stale policy");
        assert!(
            err.to_string()
                .contains("does not match configured Nexus fee asset"),
            "unexpected error: {err}"
        );

        let stored_policy =
            policy_by_id(&world.view(), ACCOUNT_ALIAS_SUFFIX_ID).expect("stored policy");
        assert_eq!(
            stored_policy.payment_asset_id, LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
            "rejected read-only quotes must not mutate policy state"
        );

        assert!(sync_default_namespace_policy_payment_asset(
            &mut world,
            &payment_asset_literal
        ));
        let quote = quote_account_alias_registration_with_configured_fee_asset(
            &world.view(),
            &catalog,
            &alias,
            &owner(),
            2,
            None,
            100,
            &payment_asset_literal,
        )
        .expect("quote after explicit state convergence");
        assert_eq!(quote.payment_asset_id, payment_asset_literal);
        assert_eq!(
            quote.payment_asset_definition_id,
            payment_asset_definition_id
        );
    }

    #[test]
    fn quote_account_alias_registration_rejects_existing_record() {
        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("treasury".parse().expect("label"), DataSpaceId::new(7));
        let owner = owner();
        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            1,
            5_000,
            5_000 + (30 * MS_PER_DAY),
            5_000 + (90 * MS_PER_DAY),
            Metadata::default(),
        );
        let mut world = World::default();
        seed_default_namespace_policies(&mut world);
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
        let view = world.view();

        let err = quote_account_alias_registration(&view, &catalog, &alias, &owner, 1, None, 100)
            .expect_err("existing registration must be rejected");

        assert!(
            err.to_string().contains("already registered"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn quote_account_alias_renewal_extends_from_existing_expiry() {
        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("merchant".parse().expect("label"), DataSpaceId::new(7));
        let owner = owner();
        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            1,
            5_000,
            5_000 + (30 * MS_PER_DAY),
            5_000 + (90 * MS_PER_DAY),
            Metadata::default(),
        );
        let mut world = World::default();
        seed_default_namespace_policies(&mut world);
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
        let view = world.view();

        let quote =
            quote_account_alias_renewal(&view, &catalog, &alias, 3, 4_000).expect("renewal quote");

        assert_eq!(quote.selector, selector);
        assert_eq!(
            quote.charge_amount,
            "1.5".parse::<Quantity>().expect("canonical quantity")
        );
        assert_eq!(quote.expires_at_ms, 5_000 + years_to_ms(3));
    }

    #[test]
    fn quote_account_alias_renewal_rejects_tombstoned_record() {
        let catalog = dataspace_catalog();
        let alias =
            AccountAlias::domainless("merchant".parse().expect("label"), DataSpaceId::new(7));
        let owner = owner();
        let selector = selector_for_account_alias(&alias, &catalog).expect("selector");
        let mut record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            1,
            5_000,
            5_000 + (30 * MS_PER_DAY),
            5_000 + (90 * MS_PER_DAY),
            Metadata::default(),
        );
        record.status = NameStatus::Tombstoned(NameTombstoneStateV1 {
            reason: "retired".to_owned(),
        });
        let mut world = World::default();
        seed_default_namespace_policies(&mut world);
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
        let view = world.view();

        let err = quote_account_alias_renewal(&view, &catalog, &alias, 1, 4_000)
            .expect_err("tombstoned registration must not renew");

        assert!(
            err.to_string().contains("tombstoned"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn seed_default_namespace_policies_populates_fixed_suffixes() {
        let mut world = World::default();

        seed_default_namespace_policies(&mut world);
        let view = world.view();

        assert!(policy_by_id(&view, ACCOUNT_ALIAS_SUFFIX_ID).is_some());
        let domain_policy =
            policy_by_id(&view, DOMAIN_NAME_SUFFIX_ID).expect("domain policy should be seeded");
        assert!(
            domain_policy
                .reserved_labels
                .iter()
                .any(|entry| entry.normalized_label == "treasury"),
            "default domain policy should keep the reserved treasury label"
        );
        assert!(policy_by_id(&view, DATASPACE_ALIAS_SUFFIX_ID).is_some());
    }

    #[test]
    fn sns_decoders_reject_trailing_bytes_and_embedded_identity_mismatches() {
        let owner = owner();
        let selector = selector_for_namespace_literal(
            SnsNamespace::Domain,
            "strict.universal",
            &dataspace_catalog(),
        )
        .expect("selector");
        let record = NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![controller(&owner)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        let record_key = record_storage_key(&selector);
        let mut world = World::default();

        let mut trailing_record = record.encode();
        trailing_record.push(0xA5);
        world
            .smart_contract_state
            .insert(record_key.clone(), trailing_record);
        assert!(record_by_selector(&world.view(), &selector).is_none());
        let err = record_or_not_found(&world.view(), &selector)
            .expect_err("trailing record bytes must fail closed");
        let message = err.to_string();
        assert!(
            message.contains("trailing bytes") || message.contains("length mismatch"),
            "{message}"
        );

        let other_selector = selector_for_namespace_literal(
            SnsNamespace::Domain,
            "other.universal",
            &dataspace_catalog(),
        )
        .expect("other selector");
        let mut mismatched_record = record.clone();
        mismatched_record.selector = other_selector.clone();
        mismatched_record.name_hash = other_selector.name_hash();
        world
            .smart_contract_state
            .insert(record_key, mismatched_record.encode());
        assert!(record_by_selector(&world.view(), &selector).is_none());
        let err = record_or_not_found(&world.view(), &selector)
            .expect_err("embedded record identity must match its lookup selector");
        assert!(err.to_string().contains("identity mismatch"), "{err}");

        seed_default_namespace_policies(&mut world);
        let policy_key = policy_storage_key(DOMAIN_NAME_SUFFIX_ID);
        let policy = policy_by_id(&world.view(), DOMAIN_NAME_SUFFIX_ID).expect("domain policy");
        let mut trailing_policy = policy.encode();
        trailing_policy.push(0x5A);
        world
            .smart_contract_state
            .insert(policy_key.clone(), trailing_policy.clone());
        assert!(policy_by_id(&world.view(), DOMAIN_NAME_SUFFIX_ID).is_none());
        seed_default_namespace_policies(&mut world);
        assert_eq!(
            world.smart_contract_state.view().get(&policy_key),
            Some(&trailing_policy),
            "seeding must not overwrite corrupt policy evidence"
        );

        let mut mismatched_policy = policy;
        mismatched_policy.suffix_id = DATASPACE_ALIAS_SUFFIX_ID;
        mismatched_policy.suffix = ".dataspace".to_owned();
        world
            .smart_contract_state
            .insert(policy_key, mismatched_policy.encode());
        assert!(policy_by_id(&world.view(), DOMAIN_NAME_SUFFIX_ID).is_none());
        let err = policy_or_not_found(&world.view(), DOMAIN_NAME_SUFFIX_ID)
            .expect_err("embedded policy identity must match its storage suffix");
        assert!(err.to_string().contains("identity mismatch"), "{err}");
    }

    #[test]
    fn seed_default_namespace_policies_upgrades_legacy_account_alias_regex() {
        let steward = owner();
        let mut policy = default_namespace_policy(
            SnsNamespace::AccountAlias,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        policy.pricing[0].label_regex = LEGACY_ACCOUNT_ALIAS_LABEL_REGEX.to_owned();
        let mut world = World::default();
        world
            .smart_contract_state_mut_for_testing()
            .insert(policy_storage_key(ACCOUNT_ALIAS_SUFFIX_ID), policy.encode());

        seed_default_namespace_policies(&mut world);

        let updated = policy_by_id(&world.view(), ACCOUNT_ALIAS_SUFFIX_ID).expect("policy");
        assert_eq!(updated.pricing[0].label_regex, r"^[a-z0-9_@.-]{3,255}$");
        assert_eq!(updated.policy_version, 2);
    }

    #[test]
    fn seed_genesis_alias_bootstrap_covers_domains_and_account_labels() {
        let genesis_key = checked_keypair();
        let genesis_account = AccountId::new(genesis_key.public_key().clone());
        let domain_id: DomainId = DomainId::try_new("cbuae", "universal").expect("domain");
        let account_id = checked_account_id();
        let label = AccountAlias::new(
            "gas".parse().expect("label"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::UNIVERSAL,
        );
        let bound_alias = AccountAlias::new(
            "settlement".parse().expect("label"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::UNIVERSAL,
        );
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(4),
                alias: "cbuae".to_owned(),
                fault_tolerance: 1,
                ..DataSpaceMetadata::default()
            },
        ])
        .expect("dataspace catalog");
        let primary_alias = AccountAlias::new(
            "ops".parse().expect("label"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::UNIVERSAL,
        );
        let payment_asset = AssetDefinitionId::derive_from_components(
            DomainId::try_new("assets", "universal").expect("asset domain"),
            "xor".parse().expect("asset name"),
        );
        let ensure_alias = |literal: &str, target_account: AccountId, role| {
            EnsureAlias::new(
                AliasIntentV1::AccountAlias(AliasAccountIntentV1 {
                    alias: ResolvedAccountAliasV1::new(
                        literal.parse().expect("resolved account alias"),
                        DataSpaceId::UNIVERSAL,
                    ),
                    target_account,
                    provision: AccountProvisionV1::Existing,
                    role,
                }),
                AliasLeaseAcquisitionV1::new(1, None),
                AliasQuoteGuardV1 {
                    expected_policy_version: 1,
                    expected_payment_asset: payment_asset.clone(),
                    max_amount: Quantity::zero(),
                    valid_until_ms: u64::MAX,
                },
            )
        };
        let ensure_dataspace = EnsureAlias::new(
            AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
                dataspace: ResolvedDataSpaceV1::new(
                    "cbuae".parse().expect("dataspace name"),
                    DataSpaceId::new(4),
                ),
                owner: genesis_account.clone(),
            }),
            AliasLeaseAcquisitionV1::new(1, None),
            AliasQuoteGuardV1 {
                expected_policy_version: 1,
                expected_payment_asset: payment_asset.clone(),
                max_amount: Quantity::zero(),
                valid_until_ms: u64::MAX,
            },
        );
        let tx = TransactionBuilder::new_genesis(
            genesis_account.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Batch(
            [
                InstructionBox::from(ensure_dataspace.clone()),
                InstructionBox::from(Register::domain(Domain::new(domain_id.clone()))),
                InstructionBox::from(Register::account(
                    Account::new(account_id.clone()).with_label(Some(label.clone())),
                )),
                InstructionBox::from(ensure_alias(
                    "settlement@cbuae.universal",
                    account_id.clone(),
                    AccountAliasRoleV1::Additional,
                )),
                InstructionBox::from(ensure_alias(
                    "ops@cbuae.universal",
                    genesis_account.clone(),
                    AccountAliasRoleV1::Primary,
                )),
            ]
            .into_iter()
            .map(iroha_data_model::transaction::ExecutableBatchItem::Instruction)
            .collect::<Vec<_>>()
            .into(),
        ))
        .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let bootstrap_authority = block
            .external_transactions()
            .next()
            .expect("genesis transaction")
            .authority()
            .clone();
        let mut world = World::default();

        seed_genesis_alias_bootstrap(&mut world, &block, &dataspace_catalog);

        let view = world.view();
        let domain_selector = selector_for_domain(&domain_id).expect("selector");
        let dataspace_selector = selector_for_dataspace_alias("cbuae").expect("selector");
        let label_selector =
            selector_for_account_alias(&label, &dataspace_catalog).expect("selector");
        let bound_selector =
            selector_for_account_alias(&bound_alias, &dataspace_catalog).expect("selector");
        let relabel_selector =
            selector_for_account_alias(&primary_alias, &dataspace_catalog).expect("selector");

        assert!(
            record_by_selector(&view, &domain_selector).is_some(),
            "genesis domain names must be leased before validation"
        );
        assert_eq!(
            record_by_selector(&view, &dataspace_selector)
                .expect("declarative dataspace aliases must seed leases")
                .metadata,
            crate::alias_setup::alias_registration_metadata(&ensure_dataspace.intent.target())
                .expect("dataspace setup metadata"),
            "genesis dataspace leases must retain their immutable text-to-ID metadata"
        );
        assert_eq!(
            record_by_selector(&view, &label_selector)
                .expect("genesis account labels must be leased before validation")
                .owner,
            account_id,
            "genesis account-label leases must be owned by the registered target account"
        );
        assert_eq!(
            record_by_selector(&view, &bound_selector)
                .expect("declarative account aliases must seed leases")
                .owner,
            account_id,
            "genesis bound-alias leases must be owned by the exact target account"
        );
        assert_eq!(
            record_by_selector(&view, &relabel_selector)
                .expect("declarative primary aliases must also seed leases")
                .owner,
            genesis_account,
            "genesis primary-alias leases must be owned by the exact target account"
        );
        let permissions = world
            .account_permissions
            .view()
            .get(&bootstrap_authority)
            .cloned()
            .expect("genesis authority permissions");
        assert!(
            permissions.contains(&Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Dataspace(label.dataspace),
            })),
            "genesis authority must be able to manage the alias dataspace used at genesis"
        );
        assert!(
            permissions.contains(&Permission::from(CanManageAccountAlias {
                scope: AccountAliasPermissionScope::Domain(domain_id.clone()),
            })),
            "genesis authority must be able to manage the alias domain used at genesis"
        );
    }

    #[test]
    fn register_name_persists_account_alias_record_in_state() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = another_owner();

        let record = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
                        label: "treasury@banking".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("register name");

        let view = state.view();
        let fetched = record_by_selector(view.world(), &record.selector).expect("stored record");

        assert_eq!(fetched.owner, owner);
        assert_eq!(fetched.selector.label, "treasury@banking");
    }

    #[test]
    fn register_name_rejects_duplicate_domain_registration() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();

        apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "duplicate.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("first registration");

        let err = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "duplicate.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect_err("duplicate registration must fail");

        assert!(
            err.to_string().contains("already registered"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_name_accepts_underscore_account_alias_labels() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();

        let record = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
                        label: "pk_gov_pharmacy@paynet".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("register underscore account alias name");

        assert_eq!(record.selector.label, "pk_gov_pharmacy@paynet");
        assert_eq!(record.pricing_class, 0);
    }

    #[test]
    fn sns_state_block_does_not_advance_transaction_height() {
        use std::collections::HashSet;

        use iroha_data_model::block::BlockHeader;
        use nonzero_ext::nonzero;

        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();

        assert_eq!(state.transactions_latest_height_for_testing(), 0);

        apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
                        label: "ops@banking".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("register name");

        assert_eq!(
            state.transactions_latest_height_for_testing(),
            0,
            "SNS state-only mutations must not advance committed transaction height"
        );

        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        {
            let tx = block.transaction();
            tx.apply();
        }
        block
            .transactions
            .insert_block(HashSet::new(), nonzero!(1_usize));
        block
            .commit()
            .expect("real block commit after SNS mutation should succeed");

        assert_eq!(state.transactions_latest_height_for_testing(), 1);
    }

    #[test]
    fn sns_state_block_uses_wall_clock_lifecycle_time() {
        use std::time::SystemTime;

        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();
        let before_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_millis() as u64;

        let record = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "soraswap.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("register name");

        assert!(
            record.registered_at_ms >= before_ms.saturating_sub(1_000),
            "SNS lifecycle timestamps should track wall clock time"
        );
        assert!(
            record.expires_at_ms > before_ms + MS_PER_DAY,
            "one-year registration should not appear expired immediately"
        );
    }

    #[test]
    fn register_domain_name_rejects_bare_domain_literal() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();

        let err = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "soraswap".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect_err("bare domain labels must be rejected");

        assert!(
            err.to_string().contains("domain.dataspace"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn register_domain_name_reserved_label_requires_steward() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = another_owner();
        let steward = fixtures::steward_account();

        let err = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "treasury.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect_err("non-steward should not claim reserved domain label");
        assert!(
            err.to_string().contains("reserved"),
            "unexpected error: {err}"
        );

        let record = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "treasury.universal".to_owned(),
                    },
                    owner: steward.clone(),
                    controllers: vec![controller(&steward)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&steward),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("steward should keep reserved domain label");
        assert_eq!(record.owner, steward);
    }

    #[test]
    fn find_active_reserved_domain_label_matches_label_key() {
        let steward = fixtures::steward_account();
        let policy = default_namespace_policy(
            SnsNamespace::Domain,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        let selector =
            NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, "treasury.universal").expect("selector");

        let reserved = find_active_reserved_label(SnsNamespace::Domain, &policy, &selector, 0)
            .expect("domain label reservation should match the label key");

        assert_eq!(reserved.normalized_label, "treasury");
    }

    #[test]
    fn find_active_reserved_domain_label_matches_fully_qualified_literal() {
        let steward = fixtures::steward_account();
        let mut policy = default_namespace_policy(
            SnsNamespace::Domain,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        policy.reserved_labels = vec![ReservedNameV1 {
            normalized_label: "ops.universal".to_owned(),
            assigned_to: Some(steward),
            release_at_ms: None,
            note: "Explicit fully qualified reservation".to_owned(),
        }];
        let selector =
            NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, "ops.universal").expect("selector");

        let reserved = find_active_reserved_label(SnsNamespace::Domain, &policy, &selector, 0)
            .expect("fully qualified domain literal should match directly");

        assert_eq!(reserved.normalized_label, "ops.universal");
    }

    #[test]
    fn find_active_reserved_domain_label_honors_release_boundary() {
        let steward = fixtures::steward_account();
        let mut policy = default_namespace_policy(
            SnsNamespace::Domain,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        policy.reserved_labels = vec![ReservedNameV1 {
            normalized_label: "ops".to_owned(),
            assigned_to: Some(steward),
            release_at_ms: Some(10),
            note: "Scheduled release".to_owned(),
        }];
        let selector =
            NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, "ops.universal").expect("selector");

        assert!(
            find_active_reserved_label(SnsNamespace::Domain, &policy, &selector, 9).is_some(),
            "reservation should still be active before the release timestamp"
        );
        assert!(
            find_active_reserved_label(SnsNamespace::Domain, &policy, &selector, 10).is_none(),
            "reservation should stop matching at the release timestamp"
        );
    }

    #[test]
    fn enforce_reserved_label_assignment_rejects_unassigned_domain_label() {
        let steward = fixtures::steward_account();
        let mut policy = default_namespace_policy(
            SnsNamespace::Domain,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        policy.reserved_labels = vec![ReservedNameV1 {
            normalized_label: "custody".to_owned(),
            assigned_to: None,
            release_at_ms: None,
            note: "Unassigned reserved label".to_owned(),
        }];
        let owner = another_owner();
        let selector =
            NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, "custody.universal").expect("selector");

        let err =
            enforce_reserved_label_assignment(SnsNamespace::Domain, &policy, &selector, &owner, 0)
                .expect_err("unassigned reserved labels must reject registration");

        assert!(
            err.to_string().contains("label `custody` is reserved"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn enforce_reserved_label_assignment_allows_matching_assignee() {
        let steward = fixtures::steward_account();
        let policy = default_namespace_policy(
            SnsNamespace::Domain,
            &steward,
            LEGACY_DEFAULT_NAMESPACE_PAYMENT_ASSET_ID,
        );
        let selector =
            NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, "treasury.universal").expect("selector");

        enforce_reserved_label_assignment(SnsNamespace::Domain, &policy, &selector, &steward, 0)
            .expect("matching assignee should be allowed");
    }

    #[test]
    fn register_domain_name_allows_released_reserved_label() {
        let mut state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let steward = fixtures::steward_account();
        let mut policy = {
            let view = state.view();
            policy_by_id(view.world(), DOMAIN_NAME_SUFFIX_ID).expect("seeded domain policy")
        };
        policy.reserved_labels = vec![ReservedNameV1 {
            normalized_label: "treasury".to_owned(),
            assigned_to: Some(steward),
            release_at_ms: Some(0),
            note: "Released reservation".to_owned(),
        }];
        state
            .world
            .smart_contract_state
            .insert(policy_storage_key(DOMAIN_NAME_SUFFIX_ID), policy.encode());

        let owner = another_owner();
        let record = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "treasury.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("released reserved labels should allow registration");

        assert_eq!(record.owner, owner);
    }

    #[test]
    fn selector_for_namespace_literal_canonicalizes_domain_literal() {
        let selector = selector_for_namespace_literal(
            SnsNamespace::Domain,
            "TreAsury.Universal",
            &dataspace_catalog(),
        )
        .expect("domain selector");

        assert_eq!(selector.normalized_label(), "treasury.universal");
    }

    #[test]
    fn selector_for_namespace_literal_canonicalizes_account_alias_literal() {
        let selector = selector_for_namespace_literal(
            SnsNamespace::AccountAlias,
            "Treasury@Banking",
            &dataspace_catalog(),
        )
        .expect("account alias selector");

        assert_eq!(selector.normalized_label(), "treasury@banking");
    }

    #[test]
    fn selector_for_namespace_literal_canonicalizes_dataspace_literal() {
        let selector = selector_for_namespace_literal(
            SnsNamespace::Dataspace,
            "Banking",
            &dataspace_catalog(),
        )
        .expect("dataspace selector");

        assert_eq!(selector.normalized_label(), "banking");
    }

    #[test]
    fn selector_for_namespace_literal_rejects_bare_domain_literal() {
        let err =
            selector_for_namespace_literal(SnsNamespace::Domain, "treasury", &dataspace_catalog())
                .expect_err("bare domain literal must fail");

        assert!(
            err.to_string().contains("domain.dataspace"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn reserved_label_key_extracts_account_alias_local_label() {
        let selector = NameSelectorV1 {
            version: NameSelectorV1::VERSION,
            suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
            label: "treasury@banking".to_owned(),
        };

        assert_eq!(
            reserved_label_key(SnsNamespace::AccountAlias, &selector),
            "treasury"
        );
    }

    #[test]
    fn reserved_label_key_keeps_dataspace_literal() {
        let selector = NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, "banking").expect("selector");

        assert_eq!(
            reserved_label_key(SnsNamespace::Dataspace, &selector),
            "banking"
        );
    }

    #[test]
    fn register_name_rejects_unknown_suffix_id() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let owner = owner();

        let err = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: 0xFFFF,
                        label: "mystery".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect_err("unknown suffix ids must be rejected");

        assert!(
            err.to_string().contains("unsupported SNS suffix id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_name_lease_expiry_rejects_past_timestamp() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();

        apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "leasepast.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("register name");

        let err = apply_with_state_block(&state, |tx| {
            set_name_lease_expiry(tx, SnsNamespace::Domain, "leasepast.universal", 0)
        })
        .expect_err("past expiry must fail");

        assert!(
            err.to_string().contains("lease_expiry_ms must be greater"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn set_name_lease_expiry_updates_lifecycle_windows() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        state.nexus.write().dataspace_catalog = dataspace_catalog();
        let owner = owner();

        apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameInput {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "leasefuture.universal".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: default_payment(&owner),
                    metadata: Metadata::default(),
                },
            )
        })
        .expect("register name");

        let future_expiry_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system clock after unix epoch")
            .as_millis() as u64
            + 60_000;
        let record = apply_with_state_block(&state, |tx| {
            set_name_lease_expiry(
                tx,
                SnsNamespace::Domain,
                "leasefuture.universal",
                future_expiry_ms,
            )
        })
        .expect("lease expiry update");

        assert_eq!(record.expires_at_ms, future_expiry_ms);
        assert_eq!(
            record.grace_expires_at_ms,
            future_expiry_ms + 30 * MS_PER_DAY
        );
        assert_eq!(
            record.redemption_expires_at_ms,
            future_expiry_ms + 90 * MS_PER_DAY
        );
    }

    #[test]
    fn reserved_universal_dataspace_selector_is_immutable() {
        let selector =
            selector_for_dataspace_alias(RESERVED_UNIVERSAL_DATASPACE_ALIAS).expect("selector");
        let err = ensure_selector_is_mutable(&selector)
            .expect_err("reserved universal selector must reject every mutation path");
        assert!(
            err.to_string().contains("immutable"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn get_name_record_refreshes_expired_lifecycle() {
        let mut world = World::default();
        let selector =
            selector_for_domain(&DomainId::try_new("trade", "universal").expect("domain id"))
                .expect("selector");
        let owner = owner();
        let record = NameRecordV1::new(
            selector.clone(),
            owner,
            vec![controller(&another_owner())],
            0,
            0,
            5,
            10,
            15,
            Metadata::default(),
        );
        world
            .smart_contract_state_mut_for_testing()
            .insert(record_storage_key(&selector), record.encode());
        let view = world.view();

        let fetched = get_name_record(
            &view,
            &DataSpaceCatalog::default(),
            SnsNamespace::Domain,
            "trade.universal",
            11,
        )
        .expect("fetch record");

        assert!(matches!(fetched.status, NameStatus::Redemption));
    }
}
