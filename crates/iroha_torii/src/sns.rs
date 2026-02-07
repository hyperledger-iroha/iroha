//! App-facing Sora Name Service (SNS) registrar shim.
//!
//! This module hosts a lightweight, in-memory registrar surface so SDK/CLI
//! consumers can exercise the SNS API while the ledger-backed contracts are
//! wired in. The data model mirrors `iroha_data_model::sns` and will be swapped
//! for deterministic state transitions in a follow-up once the smart contracts
//! land.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Path, RawQuery, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use iroha_data_model::{
    account::AccountId,
    metadata::Metadata,
    sns::{
        AuctionKind, FreezeNameRequestV1, GovernanceHookV1, NameAuctionStateV1, NameControllerV1,
        NameFrozenStateV1, NameRecordV1, NameSelectorV1, NameStatus, NameTombstoneStateV1,
        PaymentProofV1, PriceTierV1, RegisterNameRequestV1, RegisterNameResponseV1,
        RenewNameRequestV1, SuffixId, SuffixPolicyV1, SuffixStatus, TransferNameRequestV1,
        UpdateControllersRequestV1,
    },
};
use norito::json::{Map as JsonMap, Value};
use regex::Regex;
use url::form_urlencoded;

use crate::{JsonBody, SharedAppState};

const MS_PER_DAY: u64 = 86_400_000;
const MS_PER_YEAR: u64 = MS_PER_DAY * 365;
const EXPIRED_TOMBSTONE_REASON: &str = "expired";

/// In-memory registrar state backing the SNS routes.
#[derive(Debug)]
pub struct Registry {
    policies: DashMap<SuffixId, SuffixPolicyV1>,
    suffix_lookup: DashMap<String, SuffixId>,
    registrations: DashMap<String, NameRecordV1>,
    cases: DashMap<String, Value>,
    case_counter: AtomicU64,
}

impl Registry {
    /// Construct an empty registry.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            policies: DashMap::new(),
            suffix_lookup: DashMap::new(),
            registrations: DashMap::new(),
            cases: DashMap::new(),
            case_counter: AtomicU64::new(1),
        }
    }

    /// Seed the registry with the default `.sora` policy.
    #[must_use]
    pub fn bootstrap_default() -> Self {
        let registry = Self::empty();
        registry.insert_policy(iroha_data_model::sns::fixtures::default_policy());
        registry
    }

    fn insert_policy(&self, policy: SuffixPolicyV1) {
        let suffix_key = policy.suffix_key();
        self.suffix_lookup.insert(suffix_key, policy.suffix_id);
        self.policies.insert(policy.suffix_id, policy);
    }

    fn policy_by_id(&self, suffix_id: SuffixId) -> Result<SuffixPolicyV1, SnsError> {
        self.policies
            .get(&suffix_id)
            .map(|entry| entry.clone())
            .ok_or_else(|| {
                SnsError::NotFound(format!("suffix policy {suffix_id} is not registered"))
            })
    }

    fn policy_by_suffix(&self, suffix: &str) -> Result<SuffixPolicyV1, SnsError> {
        let suffix_key = suffix.to_ascii_lowercase();
        let suffix_id = self
            .suffix_lookup
            .get(&suffix_key)
            .map(|entry| *entry.value())
            .ok_or_else(|| SnsError::NotFound(format!("suffix `{suffix}` is not registered")))?;
        self.policy_by_id(suffix_id)
    }

    fn selector_from_literal(
        &self,
        literal: &str,
    ) -> Result<(NameSelectorV1, SuffixPolicyV1), SnsError> {
        let trimmed = literal.trim();
        let (label, suffix) = trimmed.rsplit_once('.').ok_or_else(|| {
            SnsError::BadRequest("expected <label>.<suffix> selector literal".into())
        })?;
        let policy = self.policy_by_suffix(suffix)?;
        let selector = NameSelectorV1::new(policy.suffix_id, label).map_err(|err| {
            SnsError::BadRequest(format!("invalid selector label `{label}`: {err}"))
        })?;
        Ok((selector, policy))
    }

    fn registry_key(selector: &NameSelectorV1, policy: &SuffixPolicyV1) -> String {
        format!("{}.{}", selector.normalized_label(), policy.suffix_key())
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64
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
        Ok(Self::tier_regex(tier)?.is_match(label))
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
            if !Self::label_matches_tier(tier, label)? {
                return Err(SnsError::BadRequest(format!(
                    "label `{label}` does not satisfy pricing class {hint}"
                )));
            }
            return Ok(tier.clone());
        }

        for tier in &policy.pricing {
            if Self::label_matches_tier(tier, label)? {
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
        if !Self::label_matches_tier(tier, label)? {
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

    #[allow(clippy::unused_self)]
    fn validate_payment_for_term(
        &self,
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
                "net_amount must not exceed gross_amount".into(),
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

    #[allow(clippy::unused_self)]
    fn enforce_reserved_labels(
        &self,
        policy: &SuffixPolicyV1,
        selector: &NameSelectorV1,
        owner: &AccountId,
        now_ms: u64,
    ) -> Result<(), SnsError> {
        if let Some(reserved) = policy
            .reserved_labels
            .iter()
            .find(|entry| entry.normalized_label == selector.normalized_label())
        {
            if reserved
                .release_at_ms
                .is_some_and(|release_at| now_ms < release_at)
            {
                return Err(SnsError::Conflict(format!(
                    "label `{}` is reserved until {}",
                    reserved.normalized_label,
                    reserved.release_at_ms.unwrap_or_default()
                )));
            }

            if let Some(assignee) = &reserved.assigned_to {
                if assignee != owner {
                    return Err(SnsError::Conflict(format!(
                        "label `{}` is reserved for a different owner",
                        reserved.normalized_label
                    )));
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn registration_record(
        &self,
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
        Self::enforce_policy_active(policy)?;
        if controllers.is_empty() {
            return Err(SnsError::BadRequest(
                "at least one controller must be provided".into(),
            ));
        }
        Self::validate_term_bounds(policy, tier, term_years)?;
        self.validate_payment_for_term(policy, tier, term_years, payment)?;
        let effective_term = term_years;
        let expires_at_ms = now_ms.saturating_add(Self::years_to_ms(effective_term));
        let grace_expires_at_ms =
            expires_at_ms.saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
        let redemption_expires_at_ms = grace_expires_at_ms
            .saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);
        let pricing_class = tier.tier_id;
        let auction = Self::maybe_auction_state(tier, now_ms);
        let name_hash = selector.name_hash();

        Ok(NameRecordV1 {
            selector,
            name_hash,
            owner,
            controllers,
            status: NameStatus::Active,
            pricing_class,
            registered_at_ms: now_ms,
            expires_at_ms,
            grace_expires_at_ms,
            redemption_expires_at_ms,
            metadata,
            auction,
        })
    }

    fn refresh_lifecycle(record: &mut NameRecordV1, now_ms: u64) {
        if matches!(record.status, NameStatus::Tombstoned(_)) {
            return;
        }
        if let NameStatus::Frozen(frozen) = &record.status {
            if now_ms < frozen.until_ms {
                return;
            }
        }
        record.status = if now_ms >= record.redemption_expires_at_ms {
            NameStatus::Tombstoned(NameTombstoneStateV1 {
                reason: EXPIRED_TOMBSTONE_REASON.to_string(),
            })
        } else if now_ms >= record.grace_expires_at_ms {
            NameStatus::Redemption
        } else if now_ms >= record.expires_at_ms {
            NameStatus::GracePeriod
        } else {
            NameStatus::Active
        };
    }

    /// Register a new name and persist it in the in-memory registry.
    pub fn register(&self, request: RegisterNameRequestV1) -> Result<NameRecordV1, SnsError> {
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
        let normalized_selector = NameSelectorV1::new(selector.suffix_id, selector.label)
            .map_err(|err| SnsError::BadRequest(err.to_string()))?;
        let policy = self.policy_by_id(selector.suffix_id)?;
        Self::enforce_policy_active(&policy)?;
        let now_ms = Self::now_ms();
        self.enforce_reserved_labels(&policy, &normalized_selector, &owner, now_ms)?;
        let key = Self::registry_key(&normalized_selector, &policy);
        if self.registrations.contains_key(&key) {
            return Err(SnsError::Conflict(format!(
                "selector `{key}` is already registered"
            )));
        }
        let tier = Self::pick_pricing_tier(&policy, &normalized_selector, pricing_class_hint)?;
        let record = self.registration_record(
            normalized_selector,
            owner,
            controllers,
            term_years,
            &payment,
            metadata,
            &policy,
            &tier,
            now_ms,
        )?;
        self.registrations.insert(key, record.clone());
        Ok(record)
    }

    /// Fetch a registration by literal selector.
    pub fn get(&self, literal: &str) -> Result<NameRecordV1, SnsError> {
        let (selector, policy) = self.selector_from_literal(literal)?;
        let key = Self::registry_key(&selector, &policy);
        let mut guard = self
            .registrations
            .get_mut(&key)
            .ok_or_else(|| SnsError::NotFound(format!("registration `{key}` not found")))?;
        let now_ms = Self::now_ms();
        Self::refresh_lifecycle(&mut guard, now_ms);
        Ok(guard.clone())
    }

    /// Renew an existing registration.
    pub fn renew(
        &self,
        literal: &str,
        payload: RenewNameRequestV1,
    ) -> Result<NameRecordV1, SnsError> {
        let (selector, policy) = self.selector_from_literal(literal)?;
        let key = Self::registry_key(&selector, &policy);
        let mut guard = self
            .registrations
            .get_mut(&key)
            .ok_or_else(|| SnsError::NotFound(format!("registration `{key}` not found")))?;
        Self::enforce_policy_active(&policy)?;
        let now_ms = Self::now_ms();
        Self::refresh_lifecycle(&mut guard, now_ms);
        if matches!(guard.status, NameStatus::Tombstoned(_)) {
            return Err(SnsError::Conflict(format!(
                "registration `{key}` is tombstoned"
            )));
        }
        let tier = Self::tier_by_pricing_class(&policy, &guard.selector, guard.pricing_class)?;
        Self::validate_term_bounds(&policy, &tier, payload.term_years)?;
        self.validate_payment_for_term(&policy, &tier, payload.term_years, &payload.payment)?;
        guard.expires_at_ms = guard
            .expires_at_ms
            .saturating_add(Self::years_to_ms(payload.term_years));
        guard.grace_expires_at_ms = guard
            .expires_at_ms
            .saturating_add(u64::from(policy.grace_period_days) * MS_PER_DAY);
        guard.redemption_expires_at_ms = guard
            .grace_expires_at_ms
            .saturating_add(u64::from(policy.redemption_period_days) * MS_PER_DAY);
        Self::refresh_lifecycle(&mut guard, now_ms);
        Ok(guard.clone())
    }

    /// Transfer ownership to a new account.
    pub fn transfer(
        &self,
        literal: &str,
        payload: TransferNameRequestV1,
    ) -> Result<NameRecordV1, SnsError> {
        let (selector, policy) = self.selector_from_literal(literal)?;
        let key = Self::registry_key(&selector, &policy);
        let mut guard = self
            .registrations
            .get_mut(&key)
            .ok_or_else(|| SnsError::NotFound(format!("registration `{key}` not found")))?;
        Self::enforce_policy_active(&policy)?;
        let now_ms = Self::now_ms();
        Self::refresh_lifecycle(&mut guard, now_ms);
        match guard.status {
            NameStatus::Tombstoned(_) => {
                return Err(SnsError::Conflict(format!(
                    "registration `{key}` is tombstoned"
                )));
            }
            NameStatus::Frozen(_) => {
                return Err(SnsError::Conflict(format!(
                    "registration `{key}` is frozen"
                )));
            }
            _ => {}
        }
        guard.owner = payload.new_owner;
        Ok(guard.clone())
    }

    /// Update controllers attached to a registration.
    pub fn update_controllers(
        &self,
        literal: &str,
        payload: UpdateControllersRequestV1,
    ) -> Result<NameRecordV1, SnsError> {
        let (selector, policy) = self.selector_from_literal(literal)?;
        let key = Self::registry_key(&selector, &policy);
        let mut guard = self
            .registrations
            .get_mut(&key)
            .ok_or_else(|| SnsError::NotFound(format!("registration `{key}` not found")))?;
        Self::enforce_policy_active(&policy)?;
        let now_ms = Self::now_ms();
        Self::refresh_lifecycle(&mut guard, now_ms);
        if matches!(guard.status, NameStatus::Tombstoned(_)) {
            return Err(SnsError::Conflict(format!(
                "registration `{key}` is tombstoned"
            )));
        }
        if payload.controllers.is_empty() {
            return Err(SnsError::BadRequest(
                "at least one controller must be provided".into(),
            ));
        }
        guard.controllers = payload.controllers;
        Ok(guard.clone())
    }

    /// Freeze a registration.
    pub fn freeze(
        &self,
        literal: &str,
        payload: FreezeNameRequestV1,
    ) -> Result<NameRecordV1, SnsError> {
        let (selector, policy) = self.selector_from_literal(literal)?;
        let key = Self::registry_key(&selector, &policy);
        Self::enforce_policy_active(&policy)?;
        let mut guard = self
            .registrations
            .get_mut(&key)
            .ok_or_else(|| SnsError::NotFound(format!("registration `{key}` not found")))?;
        let now_ms = Self::now_ms();
        Self::refresh_lifecycle(&mut guard, now_ms);
        if matches!(guard.status, NameStatus::Tombstoned(_)) {
            return Err(SnsError::Conflict("cannot freeze a tombstoned name".into()));
        }
        guard.status = NameStatus::Frozen(NameFrozenStateV1 {
            reason: payload.reason,
            until_ms: payload.until_ms,
        });
        Ok(guard.clone())
    }

    /// Clear a freeze and reactivate the name.
    pub fn unfreeze(
        &self,
        literal: &str,
        _governance: GovernanceHookV1,
    ) -> Result<NameRecordV1, SnsError> {
        let (selector, policy) = self.selector_from_literal(literal)?;
        let key = Self::registry_key(&selector, &policy);
        Self::enforce_policy_active(&policy)?;
        let mut guard = self
            .registrations
            .get_mut(&key)
            .ok_or_else(|| SnsError::NotFound(format!("registration `{key}` not found")))?;
        let now_ms = Self::now_ms();
        Self::refresh_lifecycle(&mut guard, now_ms);
        match guard.status {
            NameStatus::Tombstoned(_) => {
                return Err(SnsError::Conflict(
                    "cannot unfreeze a tombstoned name".into(),
                ));
            }
            NameStatus::Frozen(_) => {}
            _ => {
                return Err(SnsError::Conflict(
                    "registration is not currently frozen".into(),
                ));
            }
        }
        guard.status = NameStatus::Active;
        Self::refresh_lifecycle(&mut guard, now_ms);
        Ok(guard.clone())
    }

    /// Persist an arbitrary dispute or governance case envelope.
    pub fn record_case(&self, payload: Value) -> Value {
        let id = self.case_counter.fetch_add(1, Ordering::Relaxed);
        let case_id = format!("case-{id:06}");
        let mut envelope = match payload {
            Value::Object(mut map) => {
                map.entry("case_id".into())
                    .or_insert_with(|| Value::String(case_id.clone()));
                Value::Object(map)
            }
            other => {
                let mut map = JsonMap::new();
                map.insert("case_id".into(), Value::String(case_id.clone()));
                map.insert("payload".into(), other);
                Value::Object(map)
            }
        };
        if let Value::Object(map) = &mut envelope {
            map.entry("submitted_at_ms".into())
                .or_insert_with(|| Value::Number(Self::now_ms().into()));
        }
        self.cases.insert(case_id, envelope.clone());
        envelope
    }

    /// Export recorded cases (best-effort filters).
    pub fn export_cases(
        &self,
        since: Option<&str>,
        status: Option<&str>,
        limit: Option<usize>,
    ) -> Value {
        let mut all: Vec<Value> = self
            .cases
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        if let Some(status_filter) = status {
            all.retain(|value| match value {
                Value::Object(map) => map
                    .get("status")
                    .and_then(Value::as_str)
                    .map(|entry| entry.eq_ignore_ascii_case(status_filter))
                    .unwrap_or(false),
                _ => false,
            });
        }
        if let Some(since_filter) = since {
            if let Ok(parsed) = since_filter.parse::<u64>() {
                all.retain(|value| match value {
                    Value::Object(map) => map
                        .get("submitted_at_ms")
                        .and_then(Value::as_u64)
                        .is_some_and(|ts| ts >= parsed),
                    _ => true,
                });
            }
        }
        if let Some(max) = limit {
            all.truncate(max);
        }
        Value::Array(all)
    }

    /// Return a registered policy.
    pub fn policy(&self, suffix_id: SuffixId) -> Result<SuffixPolicyV1, SnsError> {
        self.policy_by_id(suffix_id)
    }
}

/// HTTP-friendly error wrapper for SNS routes.
#[derive(Debug)]
pub enum SnsError {
    /// Entity was not found.
    NotFound(String),
    /// Request failed validation.
    BadRequest(String),
    /// Request conflicts with existing state.
    Conflict(String),
}

impl IntoResponse for SnsError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            Self::Conflict(msg) => (StatusCode::CONFLICT, msg),
        };
        (status, message).into_response()
    }
}

fn metric_suffix_from_literal(selector_literal: &str) -> String {
    selector_literal
        .trim()
        .rsplit_once('.')
        .map(|(_, suffix)| suffix.trim().to_ascii_lowercase())
        .filter(|suffix| !suffix.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn metric_suffix_from_suffix_id(registry: &Registry, suffix_id: SuffixId) -> String {
    registry
        .policy(suffix_id)
        .map(|policy| policy.suffix_key())
        .unwrap_or_else(|_| suffix_id.to_string())
}

fn record_registrar_status<T>(app: &SharedAppState, suffix: &str, outcome: &Result<T, SnsError>) {
    let result = if outcome.is_ok() { "ok" } else { "error" };
    app.telemetry.with_metrics(|telemetry| {
        telemetry.inc_sns_registrar_status(result, suffix);
    });
}

/// Handle `POST /v1/sns/registrations`.
#[axum::debug_handler(state = SharedAppState)]
pub async fn handle_register(
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<RegisterNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let suffix = metric_suffix_from_suffix_id(registry, request.selector.suffix_id);
    let outcome = registry.register(request);
    record_registrar_status(&app, &suffix, &outcome);
    let record = outcome?;
    Ok((
        StatusCode::CREATED,
        JsonBody(RegisterNameResponseV1 {
            name_record: record,
        }),
    ))
}

/// Handle `GET /v1/sns/registrations/{selector}`.
pub async fn handle_get_registration(
    Path(selector): Path<String>,
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let record = registry.get(&selector)?;
    Ok(JsonBody(record))
}

/// Handle `POST /v1/sns/registrations/{selector}/renew`.
pub async fn handle_renew_registration(
    Path(selector): Path<String>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<RenewNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let suffix = metric_suffix_from_literal(&selector);
    let outcome = registry.renew(&selector, request);
    record_registrar_status(&app, &suffix, &outcome);
    let record = outcome?;
    Ok(JsonBody(record))
}

/// Handle `POST /v1/sns/registrations/{selector}/transfer`.
pub async fn handle_transfer_registration(
    Path(selector): Path<String>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<TransferNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let suffix = metric_suffix_from_literal(&selector);
    let outcome = registry.transfer(&selector, request);
    record_registrar_status(&app, &suffix, &outcome);
    let record = outcome?;
    Ok(JsonBody(record))
}

/// Handle `POST /v1/sns/registrations/{selector}/controllers`.
pub async fn handle_update_controllers(
    Path(selector): Path<String>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<UpdateControllersRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let suffix = metric_suffix_from_literal(&selector);
    let outcome = registry.update_controllers(&selector, request);
    record_registrar_status(&app, &suffix, &outcome);
    let record = outcome?;
    Ok(JsonBody(record))
}

/// Handle `POST /v1/sns/registrations/{selector}/freeze`.
pub async fn handle_freeze_registration(
    Path(selector): Path<String>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(request): crate::JsonOnly<FreezeNameRequestV1>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let suffix = metric_suffix_from_literal(&selector);
    let outcome = registry.freeze(&selector, request);
    record_registrar_status(&app, &suffix, &outcome);
    let record = outcome?;
    Ok(JsonBody(record))
}

/// Handle `DELETE /v1/sns/registrations/{selector}/freeze`.
pub async fn handle_unfreeze_registration(
    Path(selector): Path<String>,
    State(app): State<SharedAppState>,
    crate::JsonOnly(governance): crate::JsonOnly<GovernanceHookV1>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let suffix = metric_suffix_from_literal(&selector);
    let outcome = registry.unfreeze(&selector, governance);
    record_registrar_status(&app, &suffix, &outcome);
    let record = outcome?;
    Ok(JsonBody(record))
}

/// Handle `GET /v1/sns/policies/{suffix_id}`.
pub async fn handle_get_policy(
    Path(suffix_id): Path<SuffixId>,
    State(app): State<SharedAppState>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let policy = registry.policy(suffix_id)?;
    Ok(JsonBody(policy))
}

/// Handle `POST /v1/sns/governance/cases`.
pub async fn handle_post_case(
    State(app): State<SharedAppState>,
    crate::JsonOnly(payload): crate::JsonOnly<Value>,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let envelope = registry.record_case(payload);
    Ok(JsonBody(envelope))
}

/// Handle `GET /v1/sns/governance/cases`.
pub async fn handle_get_cases(
    State(app): State<SharedAppState>,
    RawQuery(raw): RawQuery,
) -> Result<impl IntoResponse, SnsError> {
    let registry = app.sns_registry.as_ref();
    let (mut since, mut status, mut limit) = (None, None, None);
    if let Some(query) = raw {
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            match key.as_ref() {
                "since" => since = Some(value.into_owned()),
                "status" => status = Some(value.into_owned()),
                "limit" => limit = value.parse::<usize>().ok(),
                _ => {}
            }
        }
    }
    let payload = registry.export_cases(since.as_deref(), status.as_deref(), limit);
    Ok(JsonBody(payload))
}

#[cfg(test)]
mod tests {
    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::AccountAddress,
        sns::{self, SuffixStatus},
    };
    use iroha_primitives::json::Json;

    use super::*;

    fn payment(owner: &AccountId, amount: u64) -> PaymentProofV1 {
        PaymentProofV1 {
            asset_id: "xor#sora".to_string(),
            gross_amount: amount,
            net_amount: amount,
            settlement_tx: Json::from("tx"),
            payer: owner.clone(),
            signature: Json::from("sig"),
        }
    }

    fn controller(owner: &AccountId) -> NameControllerV1 {
        let address =
            AccountAddress::from_account_id(owner).expect("should encode account address");
        NameControllerV1::account(&address)
    }

    #[test]
    fn registration_requires_matching_pricing_regex() {
        let registry = Registry::bootstrap_default();
        let owner = sns::fixtures::steward_account();
        let request = RegisterNameRequestV1 {
            selector: NameSelectorV1::new(0x0001, "ab").expect("selector"),
            owner: owner.clone(),
            controllers: vec![controller(&owner)],
            term_years: 1,
            pricing_class_hint: None,
            payment: payment(&owner, 120),
            governance: None,
            metadata: Metadata::default(),
        };

        let err = registry
            .register(request)
            .expect_err("registration must fail");
        match err {
            SnsError::BadRequest(msg) => {
                assert!(msg.contains("pricing"), "unexpected message: {msg}")
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn paused_policy_blocks_registration() {
        let registry = Registry::bootstrap_default();
        let mut policy = registry.policy_by_id(0x0001).expect("policy");
        policy.status = SuffixStatus::Paused;
        registry.insert_policy(policy);

        let owner = sns::fixtures::steward_account();
        let request = RegisterNameRequestV1 {
            selector: NameSelectorV1::new(0x0001, "blocked").expect("selector"),
            owner: owner.clone(),
            controllers: vec![controller(&owner)],
            term_years: 1,
            pricing_class_hint: None,
            payment: payment(&owner, 120),
            governance: None,
            metadata: Metadata::default(),
        };

        let err = registry
            .register(request)
            .expect_err("registration must fail");
        match err {
            SnsError::Conflict(msg) => {
                assert!(msg.contains("paused"), "unexpected message: {msg}")
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn lifecycle_transitions_follow_timestamps() {
        let registry = Registry::bootstrap_default();
        let owner = sns::fixtures::steward_account();
        let selector = NameSelectorV1::new(0x0001, "timed").expect("selector");
        let registered = registry
            .register(RegisterNameRequestV1 {
                selector,
                owner: owner.clone(),
                controllers: vec![controller(&owner)],
                term_years: 1,
                pricing_class_hint: None,
                payment: payment(&owner, 120),
                governance: None,
                metadata: Metadata::default(),
            })
            .expect("must register");
        let policy = registry
            .policy_by_id(registered.selector.suffix_id)
            .expect("policy");
        let key = Registry::registry_key(&registered.selector, &policy);

        let now_ms = Registry::now_ms();
        {
            let mut guard = registry
                .registrations
                .get_mut(&key)
                .expect("registration present");
            guard.expires_at_ms = now_ms.saturating_sub(10);
            guard.grace_expires_at_ms = now_ms + 50;
            guard.redemption_expires_at_ms = now_ms + 100;
        }
        let grace = registry
            .get("timed.sora")
            .expect("grace lookup should succeed");
        assert!(matches!(grace.status, NameStatus::GracePeriod));

        {
            let mut guard = registry
                .registrations
                .get_mut(&key)
                .expect("registration present");
            let past = Registry::now_ms().saturating_sub(1);
            guard.expires_at_ms = past;
            guard.grace_expires_at_ms = past;
            guard.redemption_expires_at_ms = past;
        }
        let tombstoned = registry
            .get("timed.sora")
            .expect("tombstone lookup should succeed");
        match tombstoned.status {
            NameStatus::Tombstoned(state) => {
                assert_eq!(state.reason, EXPIRED_TOMBSTONE_REASON.to_string());
            }
            other => panic!("expected tombstone, got {other:?}"),
        }
    }

    #[test]
    fn tombstoned_registration_rejects_mutations() {
        let registry = Registry::bootstrap_default();
        let owner = sns::fixtures::steward_account();
        let selector = NameSelectorV1::new(0x0001, "expired").expect("selector");
        let registered = registry
            .register(RegisterNameRequestV1 {
                selector,
                owner: owner.clone(),
                controllers: vec![controller(&owner)],
                term_years: 1,
                pricing_class_hint: None,
                payment: payment(&owner, 120),
                governance: None,
                metadata: Metadata::default(),
            })
            .expect("must register");
        let policy = registry
            .policy_by_id(registered.selector.suffix_id)
            .expect("policy");
        let key = Registry::registry_key(&registered.selector, &policy);

        {
            let mut guard = registry
                .registrations
                .get_mut(&key)
                .expect("registration present");
            let past = Registry::now_ms().saturating_sub(1);
            guard.expires_at_ms = past;
            guard.grace_expires_at_ms = past;
            guard.redemption_expires_at_ms = past;
        }

        let renew_err = registry
            .renew(
                "expired.sora",
                RenewNameRequestV1 {
                    term_years: 1,
                    payment: payment(&owner, 120),
                },
            )
            .expect_err("tombstoned renewal must fail");
        assert_conflict_contains(renew_err, "tombstoned");

        let new_owner = {
            let domain = "sns".parse().expect("domain parses");
            let public_key: PublicKey =
                "ed0120C70416DC2D60D9AB2F0C6CED829837F1006DDED2DE794E9D5091A60663FA8C11"
                    .parse()
                    .expect("key parses");
            AccountId::new(domain, public_key)
        };
        let transfer_err = registry
            .transfer(
                "expired.sora",
                TransferNameRequestV1 {
                    new_owner,
                    governance: GovernanceHookV1 {
                        proposal_id: "proposal-1".into(),
                        council_vote_hash: Json::from("council"),
                        dao_vote_hash: Json::from("dao"),
                        steward_ack: Json::from("steward"),
                        guardian_clearance: None,
                    },
                },
            )
            .expect_err("tombstoned transfer must fail");
        assert_conflict_contains(transfer_err, "tombstoned");

        let controllers_err = registry
            .update_controllers(
                "expired.sora",
                UpdateControllersRequestV1 {
                    controllers: vec![controller(&owner)],
                },
            )
            .expect_err("tombstoned controller update must fail");
        assert_conflict_contains(controllers_err, "tombstoned");
    }

    fn assert_conflict_contains(err: SnsError, needle: &str) {
        match err {
            SnsError::Conflict(msg) => {
                assert!(
                    msg.contains(needle),
                    "expected conflict to mention `{needle}`, got `{msg}`"
                );
            }
            other => panic!("expected conflict error, got {other:?}"),
        }
    }
}
