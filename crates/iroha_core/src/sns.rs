//! Ledger-backed SNS storage and mutation helpers.
//!
//! This module is the authoritative SNS read/write path used by account aliases,
//! domain-name lease checks, dataspace-name ownership checks, and the Torii SNS
//! HTTP API. SNS records and policies are stored in `World.smart_contract_state`
//! so the ledger-backed lifecycle model remains deterministic across peers.

use std::{str::FromStr, time::SystemTime};

use iroha_data_model::{
    account::{AccountAddress, AccountId, rekey::AccountAlias},
    block::BlockHeader,
    domain::DomainId,
    isi::{
        domain_link::{SetAccountAliasBinding, SetPrimaryAccountAlias},
        register::RegisterBox,
    },
    metadata::Metadata,
    name::Name,
    nexus::{DataSpaceCatalog, DataSpaceId},
    permission::Permission,
    sns::{
        AuctionKind, FreezeNameRequestV1, GovernanceHookV1, NameAuctionStateV1, NameControllerV1,
        NameFrozenStateV1, NameRecordV1, NameSelectorError, NameSelectorV1, NameStatus,
        NameTombstoneStateV1, PaymentProofV1, PriceTierV1, RegisterNameRequestV1,
        RenewNameRequestV1, SuffixFeeSplitV1, SuffixId, SuffixPolicyV1, SuffixStatus, TokenValue,
        TransferNameRequestV1, UpdateControllersRequestV1, fixtures,
    },
    transaction::Executable,
};
use iroha_executor_data_model::permission::account::{
    AccountAliasPermissionScope, CanManageAccountAlias,
};
use mv::storage::StorageReadOnly;
use norito::codec::{Decode as _, Encode as _};
use regex::Regex;
use thiserror::Error;

use crate::state::{State, StateReadOnly, StateTransaction, World, WorldReadOnly};

pub use iroha_data_model::sns::{
    ACCOUNT_ALIAS_SUFFIX_ID, DATASPACE_ALIAS_SUFFIX_ID, DOMAIN_NAME_SUFFIX_ID,
};

const MS_PER_DAY: u64 = 86_400_000;
const MS_PER_YEAR: u64 = MS_PER_DAY * 365;
const EXPIRED_TOMBSTONE_REASON: &str = "expired";

/// Reserved dataspace alias that must stay permanently defined.
pub const RESERVED_UNIVERSAL_DATASPACE_ALIAS: &str = "universal";

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
    /// Canonical domain literal.
    Domain,
    /// Canonical dataspace alias.
    Dataspace,
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
            Self::AccountAlias => r"^[a-z0-9@.-]{3,255}$",
            Self::Domain | Self::Dataspace => r"^[a-z0-9-]{1,63}$",
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
pub fn record_storage_key(selector: &NameSelectorV1) -> Name {
    Name::from_str(&format!(
        "sns/records/{}/{}",
        selector.suffix_id,
        hex::encode(selector.name_hash())
    ))
    .expect("static SNS storage key format is a valid Name")
}

/// Compute the durable smart-contract-state key for a SNS suffix policy.
#[must_use]
pub fn policy_storage_key(suffix_id: SuffixId) -> Name {
    Name::from_str(&format!("sns/policies/{suffix_id}"))
        .expect("static SNS policy storage key format is a valid Name")
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
    NameSelectorV1::new(DOMAIN_NAME_SUFFIX_ID, domain.name().as_ref())
}

/// Build the selector used for a canonical dataspace-alias lease record.
pub fn selector_for_dataspace_alias(alias: &str) -> Result<NameSelectorV1, NameSelectorError> {
    NameSelectorV1::new(DATASPACE_ALIAS_SUFFIX_ID, alias)
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
            let name = Name::from_str(literal.trim())
                .map_err(|err| SnsError::BadRequest(err.reason().to_owned()))?;
            selector_for_domain(&DomainId::new(name))
                .map_err(|err| SnsError::BadRequest(err.to_string()))
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
    let mut slice = bytes.as_slice();
    NameRecordV1::decode(&mut slice).ok()
}

/// Decode a SNS policy from world state for the supplied suffix id.
#[must_use]
pub fn policy_by_id(world: &impl WorldReadOnly, suffix_id: SuffixId) -> Option<SuffixPolicyV1> {
    let key = policy_storage_key(suffix_id);
    let bytes = world.smart_contract_state().get(&key)?;
    let mut slice = bytes.as_slice();
    SuffixPolicyV1::decode(&mut slice).ok()
}

fn bootstrap_steward_for_world(world: &World) -> AccountId {
    world
        .domains
        .view()
        .get(&*iroha_genesis::GENESIS_DOMAIN_ID)
        .map(|domain| domain.owned_by().clone())
        .unwrap_or_else(fixtures::steward_account)
}

fn seed_name_record_if_missing(world: &mut World, owner: &AccountId, selector: NameSelectorV1) {
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
        Metadata::default(),
    );
    world
        .smart_contract_state
        .insert(storage_key, record.encode());
}

fn seed_alias_manage_permissions_if_missing(
    world: &mut World,
    authority: &AccountId,
    label: &AccountAlias,
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
    if let Some(domain) = &label.domain {
        let domain_permission = Permission::from(CanManageAccountAlias {
            scope: AccountAliasPermissionScope::Domain(domain.clone()),
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
        let Executable::Instructions(instructions) = transaction.instructions() else {
            continue;
        };

        for instruction in instructions {
            if let Some(register) = instruction.as_any().downcast_ref::<RegisterBox>() {
                match register {
                    RegisterBox::Domain(register) => {
                        let selector = selector_for_domain(&register.object().id)
                            .expect("genesis domain ids should be canonical");
                        seed_name_record_if_missing(world, authority, selector);
                    }
                    RegisterBox::Account(register) => {
                        if let Some(label) = register.object().label() {
                            seed_alias_manage_permissions_if_missing(world, authority, label);
                            if let Ok(selector) =
                                selector_for_account_alias(label, dataspace_catalog)
                            {
                                seed_name_record_if_missing(world, authority, selector);
                            }
                        }
                    }
                    _ => {}
                }
            }

            if let Some(set_alias) = instruction
                .as_any()
                .downcast_ref::<SetPrimaryAccountAlias>()
            {
                if let Some(alias) = set_alias.alias.as_ref() {
                    seed_alias_manage_permissions_if_missing(world, authority, alias);
                    if let Ok(selector) = selector_for_account_alias(alias, dataspace_catalog) {
                        seed_name_record_if_missing(world, authority, selector);
                    }
                }
            }

            if let Some(bind_alias) = instruction
                .as_any()
                .downcast_ref::<SetAccountAliasBinding>()
            {
                if let Some(alias) = bind_alias.alias.as_ref() {
                    seed_alias_manage_permissions_if_missing(world, authority, alias);
                    if let Ok(selector) = selector_for_account_alias(alias, dataspace_catalog) {
                        seed_name_record_if_missing(world, authority, selector);
                    }
                }
            }
        }
    }
}

fn default_namespace_policy(namespace: SnsNamespace, steward: &AccountId) -> SuffixPolicyV1 {
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
            SnsNamespace::Dataspace => vec![iroha_data_model::sns::ReservedNameV1 {
                normalized_label: RESERVED_UNIVERSAL_DATASPACE_ALIAS.to_owned(),
                assigned_to: Some(steward.clone()),
                release_at_ms: None,
                note: "Protocol reserved dataspace alias".to_owned(),
            }],
            _ => Vec::new(),
        },
        payment_asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned(),
        pricing: vec![PriceTierV1 {
            tier_id: 0,
            label_regex: namespace.label_regex().to_owned(),
            base_price: TokenValue::new("61CtjvNd9T3THAR65GsMVHr82Bjc", 120),
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

/// Seed the fixed namespace policies required by the on-chain SNS model.
pub fn seed_default_namespace_policies(world: &mut World) {
    let steward = bootstrap_steward_for_world(world);
    for namespace in [
        SnsNamespace::AccountAlias,
        SnsNamespace::Domain,
        SnsNamespace::Dataspace,
    ] {
        let policy = default_namespace_policy(namespace, &steward);
        let key = policy_storage_key(policy.suffix_id);
        if world.smart_contract_state.view().get(&key).is_none() {
            world.smart_contract_state.insert(key, policy.encode());
        }
    }
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
    payment: &PaymentProofV1,
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
        .checked_mul(u128::from(term_years))
        .ok_or_else(|| {
            SnsError::Conflict(format!(
                "required payment overflowed for pricing class {}",
                tier.tier_id
            ))
        })?;
    let paid_gross = u128::from(payment.gross_amount);
    let paid_net = u128::from(payment.net_amount);
    if paid_gross < required || paid_net < required {
        return Err(SnsError::BadRequest(format!(
            "payment ({}/{} {}) does not meet required amount {} for term {term_years}",
            payment.net_amount, payment.gross_amount, payment.asset_id, required
        )));
    }
    Ok(())
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

fn registration_record(
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,
    payment: &PaymentProofV1,
    metadata: Metadata,
    policy: &SuffixPolicyV1,
    tier: &PriceTierV1,
    now_ms: u64,
) -> Result<NameRecordV1, SnsError> {
    enforce_policy_active(policy)?;
    if controllers.is_empty() {
        return Err(SnsError::BadRequest(
            "at least one controller must be provided".to_owned(),
        ));
    }
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
        SnsNamespace::Domain | SnsNamespace::Dataspace => {
            NameSelectorV1::new(selector.suffix_id, selector.label)
                .map_err(|err| SnsError::BadRequest(err.to_string()))?
        }
    };
    Ok((namespace, canonical))
}

fn record_or_not_found(
    world: &impl WorldReadOnly,
    selector: &NameSelectorV1,
) -> Result<NameRecordV1, SnsError> {
    record_by_selector(world, selector).ok_or_else(|| {
        SnsError::NotFound(format!(
            "registration `{}` not found",
            selector.normalized_label()
        ))
    })
}

fn policy_or_not_found(
    world: &impl WorldReadOnly,
    suffix_id: SuffixId,
) -> Result<SuffixPolicyV1, SnsError> {
    policy_by_id(world, suffix_id)
        .ok_or_else(|| SnsError::NotFound(format!("suffix policy {suffix_id} is not registered")))
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
    let mut record = record_or_not_found(world, &selector)?;
    refresh_lifecycle(&mut record, now_ms);
    Ok(record)
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
pub fn register_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    request: RegisterNameRequestV1,
) -> Result<NameRecordV1, SnsError> {
    let RegisterNameRequestV1 {
        selector,
        owner,
        controllers,
        term_years,
        pricing_class_hint,
        payment,
        governance: _,
        metadata,
    } = request;

    let (_namespace, canonical_selector) =
        canonicalize_request_selector(selector, &state_transaction.nexus.dataspace_catalog)?;
    ensure_selector_is_mutable(&canonical_selector)?;
    let policy = policy_or_not_found(state_transaction.world(), canonical_selector.suffix_id)?;
    enforce_policy_active(&policy)?;
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
    let now_ms = state_transaction.block_unix_timestamp_ms();
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

/// Renew an existing SNS name in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the name or policy is missing, the record is not
/// mutable, or the payment/term fails policy validation.
pub fn renew_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    namespace: SnsNamespace,
    literal: &str,
    payload: RenewNameRequestV1,
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
    let tier = tier_by_pricing_class(&policy, &record.selector, record.pricing_class)?;
    validate_term_bounds(&policy, &tier, payload.term_years)?;
    validate_payment_for_term(&policy, &tier, payload.term_years, &payload.payment)?;
    record.expires_at_ms = record
        .expires_at_ms
        .saturating_add(years_to_ms(payload.term_years));
    record.grace_expires_at_ms = record
        .expires_at_ms
        .saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
    record.redemption_expires_at_ms = record
        .grace_expires_at_ms
        .saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);
    refresh_lifecycle(&mut record, now_ms);
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Set the absolute lease expiry for an existing SNS name in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the record or policy is missing, the selector is immutable, the
/// record is tombstoned, or the requested expiry is not in the future.
pub fn set_name_lease_expiry(
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

/// Transfer SNS name ownership in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the name or policy is missing, the record is
/// immutable, or the current lifecycle does not permit transfer.
pub fn transfer_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    namespace: SnsNamespace,
    literal: &str,
    payload: TransferNameRequestV1,
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
    record.owner = payload.new_owner;
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Update SNS controllers in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the name or policy is missing, the record is
/// immutable, or the new controller set is invalid.
pub fn update_name_controllers(
    state_transaction: &mut StateTransaction<'_, '_>,
    namespace: SnsNamespace,
    literal: &str,
    payload: UpdateControllersRequestV1,
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
    if payload.controllers.is_empty() {
        return Err(SnsError::BadRequest(
            "at least one controller must be provided".to_owned(),
        ));
    }
    record.controllers = payload.controllers;
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Freeze a SNS name in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the name or policy is missing, the record is
/// immutable, or the current lifecycle does not permit freezing.
pub fn freeze_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    namespace: SnsNamespace,
    literal: &str,
    payload: FreezeNameRequestV1,
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
        return Err(SnsError::Conflict(
            "cannot freeze a tombstoned name".to_owned(),
        ));
    }
    record.status = NameStatus::Frozen(NameFrozenStateV1 {
        reason: payload.reason,
        until_ms: payload.until_ms,
    });
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Clear a freeze and reactivate a SNS name in authoritative state.
///
/// # Errors
///
/// Returns [`SnsError`] when the name or policy is missing, the record is
/// immutable, or the current lifecycle does not permit unfreezing.
pub fn unfreeze_name(
    state_transaction: &mut StateTransaction<'_, '_>,
    namespace: SnsNamespace,
    literal: &str,
    _governance: GovernanceHookV1,
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
    match record.status {
        NameStatus::Tombstoned(_) => {
            return Err(SnsError::Conflict(
                "cannot unfreeze a tombstoned name".to_owned(),
            ));
        }
        NameStatus::Frozen(_) => {}
        _ => {
            return Err(SnsError::Conflict(
                "registration is not currently frozen".to_owned(),
            ));
        }
    }
    record.status = NameStatus::Active;
    refresh_lifecycle(&mut record, now_ms);
    persist_record(state_transaction, &record);
    Ok(record)
}

/// Apply a ledger-backed SNS mutation in a dedicated state block.
///
/// This helper is used by Torii's HTTP adapter to keep SNS mutations in core
/// state instead of a separate in-memory registry.
///
/// SNS HTTP mutations are world-state side effects, not canonical chain
/// blocks. The helper therefore commits the `StateBlock` without registering a
/// synthetic entry in the transactions index; doing so would advance the
/// in-memory committed transaction height without a matching Kura block and
/// poison the next real block commit.
///
/// # Errors
///
/// Returns [`SnsError`] when the mutation fails or the state block cannot be
/// committed.
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
    let selector = selector_for_account_alias(alias, catalog).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
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
    let selector = selector_for_dataspace_alias(alias).ok()?;
    active_owner_by_selector(world, &selector, now_ms)
}

/// Resolve the active owner for the dataspace id using the current catalog alias.
#[must_use]
pub fn active_dataspace_owner_by_id(
    world: &impl WorldReadOnly,
    catalog: &DataSpaceCatalog,
    dataspace_id: DataSpaceId,
    now_ms: u64,
) -> Option<AccountId> {
    let alias = catalog.by_id(dataspace_id)?.alias.as_str();
    active_dataspace_owner_by_alias(world, alias, now_ms)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        account::{
            Account, AccountAddress, AccountId,
            rekey::{AccountAlias, AccountAliasDomain},
        },
        block::SignedBlock,
        domain::Domain,
        isi::{InstructionBox, Register, domain_link::SetPrimaryAccountAlias},
        metadata::Metadata,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata},
        sns::{
            FreezeNameRequestV1, GovernanceHookV1, NameControllerV1, NameRecordV1, NameSelectorV1,
            NameStatus, PaymentProofV1, RegisterNameRequestV1, TransferNameRequestV1,
        },
        transaction::TransactionBuilder,
    };
    use iroha_primitives::json::Json;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    fn owner() -> AccountId {
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
    }

    fn another_owner() -> AccountId {
        let public_key = "ed0120C70416DC2D60D9AB2F0C6CED829837F1006DDED2DE794E9D5091A60663FA8C11"
            .parse()
            .expect("public key");
        AccountId::new(public_key)
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
        ])
        .expect("catalog")
    }

    fn controller(owner: &AccountId) -> NameControllerV1 {
        let address =
            AccountAddress::from_account_id(owner).expect("should encode account address");
        NameControllerV1::account(&address)
    }

    fn payment(owner: &AccountId, amount: u64) -> PaymentProofV1 {
        PaymentProofV1 {
            asset_id: "61CtjvNd9T3THAR65GsMVHr82Bjc".to_string(),
            gross_amount: amount,
            net_amount: amount,
            settlement_tx: Json::from("tx"),
            payer: owner.clone(),
            signature: Json::from("sig"),
        }
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
    fn active_dataspace_owner_reads_from_world_storage() {
        let catalog = dataspace_catalog();
        let selector = selector_for_dataspace_alias("banking").expect("selector");
        let owner = owner();
        let address = AccountAddress::from_account_id(&owner).expect("account address");
        let record = NameRecordV1::new(
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
    fn seed_default_namespace_policies_populates_fixed_suffixes() {
        let mut world = World::default();

        seed_default_namespace_policies(&mut world);
        let view = world.view();

        assert!(policy_by_id(&view, ACCOUNT_ALIAS_SUFFIX_ID).is_some());
        assert!(policy_by_id(&view, DOMAIN_NAME_SUFFIX_ID).is_some());
        assert!(policy_by_id(&view, DATASPACE_ALIAS_SUFFIX_ID).is_some());
    }

    #[test]
    fn seed_genesis_alias_bootstrap_covers_domains_and_account_labels() {
        let chain_id = iroha_data_model::ChainId::from("sns-genesis-alias-bootstrap");
        let genesis_key = KeyPair::random();
        let genesis_account = AccountId::new(genesis_key.public_key().clone());
        let domain_id: DomainId = "cbuae".parse().expect("domain");
        let account_id = AccountId::new(KeyPair::random().public_key().clone());
        let label = AccountAlias::new(
            "gas".parse().expect("label"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::GLOBAL,
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
        let tx = TransactionBuilder::new(chain_id, genesis_account.clone())
            .with_instructions([
                InstructionBox::from(Register::domain(Domain::new(domain_id.clone()))),
                InstructionBox::from(Register::account(
                    Account::new(account_id.clone()).with_label(Some(label.clone())),
                )),
                InstructionBox::from(SetPrimaryAccountAlias {
                    account: genesis_account.clone(),
                    alias: Some(AccountAlias::new(
                        "ops".parse().expect("label"),
                        Some(AccountAliasDomain::new(domain_id.name().clone())),
                        DataSpaceId::GLOBAL,
                    )),
                    lease_expiry_ms: None,
                }),
            ])
            .sign(genesis_key.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key.private_key(), None, None);
        let bootstrap_authority = block
            .transactions_vec()
            .first()
            .expect("genesis transaction")
            .authority()
            .clone();
        let mut world = World::default();

        seed_genesis_alias_bootstrap(&mut world, &block, &dataspace_catalog);

        let view = world.view();
        let domain_selector = selector_for_domain(&domain_id).expect("selector");
        let label_selector =
            selector_for_account_alias(&label, &dataspace_catalog).expect("selector");
        let relabel_selector = selector_for_account_alias(
            &AccountAlias::new(
                "ops".parse().expect("label"),
                Some(AccountAliasDomain::new(domain_id.name().clone())),
                DataSpaceId::GLOBAL,
            ),
            &dataspace_catalog,
        )
        .expect("selector");

        assert!(
            record_by_selector(&view, &domain_selector).is_some(),
            "genesis domain names must be leased before validation"
        );
        assert!(
            record_by_selector(&view, &label_selector).is_some(),
            "genesis account labels must be leased before validation"
        );
        assert!(
            record_by_selector(&view, &relabel_selector).is_some(),
            "explicit label-setting instructions must also seed leases"
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
                scope: AccountAliasPermissionScope::Domain(AccountAliasDomain::new(
                    domain_id.name().clone(),
                )),
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
        let owner = owner();

        let record = apply_with_state_block(&state, |tx| {
            register_name(
                tx,
                RegisterNameRequestV1 {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
                        label: "treasury@banking".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: payment(&owner, 120),
                    governance: None,
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
                RegisterNameRequestV1 {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: ACCOUNT_ALIAS_SUFFIX_ID,
                        label: "ops@banking".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: payment(&owner, 120),
                    governance: None,
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
                RegisterNameRequestV1 {
                    selector: NameSelectorV1 {
                        version: NameSelectorV1::VERSION,
                        suffix_id: DOMAIN_NAME_SUFFIX_ID,
                        label: "soraswap".to_owned(),
                    },
                    owner: owner.clone(),
                    controllers: vec![controller(&owner)],
                    term_years: 1,
                    pricing_class_hint: None,
                    payment: payment(&owner, 120),
                    governance: None,
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
    fn reserved_universal_dataspace_record_is_immutable() {
        let mut state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let selector =
            selector_for_dataspace_alias(RESERVED_UNIVERSAL_DATASPACE_ALIAS).expect("selector");
        let owner = owner();
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
        state
            .world
            .smart_contract_state
            .insert(record_storage_key(&selector), record.encode());

        let transfer_err = apply_with_state_block(&state, |tx| {
            transfer_name(
                tx,
                SnsNamespace::Dataspace,
                RESERVED_UNIVERSAL_DATASPACE_ALIAS,
                TransferNameRequestV1 {
                    new_owner: another_owner(),
                    governance: GovernanceHookV1 {
                        proposal_id: "proposal-1".into(),
                        council_vote_hash: Json::from("council"),
                        dao_vote_hash: Json::from("dao"),
                        steward_ack: Json::from("steward"),
                        guardian_clearance: None,
                    },
                },
            )
        })
        .expect_err("universal transfer must fail");
        assert!(
            transfer_err.to_string().contains("immutable"),
            "unexpected error: {transfer_err}"
        );

        let freeze_err = apply_with_state_block(&state, |tx| {
            freeze_name(
                tx,
                SnsNamespace::Dataspace,
                RESERVED_UNIVERSAL_DATASPACE_ALIAS,
                FreezeNameRequestV1 {
                    reason: "maintenance".to_owned(),
                    until_ms: 10,
                    guardian_ticket: Json::from("ticket"),
                },
            )
        })
        .expect_err("universal freeze must fail");
        assert!(
            freeze_err.to_string().contains("immutable"),
            "unexpected error: {freeze_err}"
        );
    }

    #[test]
    fn get_name_record_refreshes_expired_lifecycle() {
        let mut world = World::default();
        let selector = selector_for_domain(&DomainId::new("trade".parse().expect("domain")))
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
            "trade",
            11,
        )
        .expect("fetch record");

        assert!(matches!(fetched.status, NameStatus::Redemption));
    }
}
