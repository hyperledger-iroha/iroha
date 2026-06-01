use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write as FmtWrite,
    fs,
    path::PathBuf,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use dashmap::DashMap;
use eyre::WrapErr as _;
use iroha_config::parameters::actual;
use iroha_core::iso_bridge::{
    profiles::{
        self, EmbeddedSignaturePolicy, MessageDirection, MessageProfile,
        ReferenceDatasetRequirement, StructuredAddressMode, TradfiRail, TradfiRailProfile,
    },
    reference_data::{ReferenceDataError, ReferenceDataSnapshots, ValidationOutcome},
};
use iroha_core::state::WorldReadOnly;
use iroha_crypto::PrivateKey;
use iroha_data_model::{
    ValidationFail,
    account::address::{AccountAddress, AccountAddressError, AddressDomainKind},
    alias::AliasIndex,
    asset::AssetDefinitionAlias,
    prelude::{
        AccountId, AssetDefinitionId, AssetId, ChainId, DomainId, InstructionBox, Metadata, Name,
        TransactionBuilder, Transfer,
    },
    transaction::error::TransactionRejectionReason,
};
use iroha_primitives::{json::Json, numeric::Numeric};
use ivm::iso20022::{IdentifierKind, InvalidValueKind, MsgError, ParsedMessage, parse_message};
use norito::json::Value as JsonValue;
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey, signature::Verifier as _,
};
use sha2::{Digest, Sha256};
use x509_parser::prelude::{FromDer as _, X509Certificate};

use crate::routing::{self, MaybeTelemetry};

/// Runtime bridge configuration derived from Torii settings.
#[derive(Clone)]
pub struct Iso20022BridgeRuntime {
    signer_account: AccountId,
    signer_private_key: PrivateKey,
    account_aliases: Arc<HashMap<String, AccountId>>,
    alias_indices: Arc<HashMap<String, AliasIndex>>,
    index_aliases: Arc<BTreeMap<AliasIndex, (String, AccountId)>>,
    currency_assets: Arc<HashMap<String, String>>,
    reference_data: Arc<ReferenceDataSnapshots>,
    default_profile_id: String,
    profiles: Arc<HashMap<String, TradfiRailProfile>>,
    store_dir: Option<PathBuf>,
    dedupe_ttl: Duration,
    records: DashMap<String, IsoMessageRecord>,
    tx_hash_index: DashMap<String, String>,
    payload_hash_index: DashMap<String, String>,
    uetr_index: DashMap<String, String>,
}

#[derive(Clone, Debug, Default)]
/// Metadata captured while parsing an ISO 20022 payment message.
pub struct IsoMessageContext {
    ledger_id: Option<String>,
    source_account_id: Option<String>,
    source_account_address: Option<String>,
    target_account_id: Option<String>,
    target_account_address: Option<String>,
    asset_definition_id: Option<String>,
    asset_id: Option<String>,
    settlement_amount: Option<String>,
    settlement_currency: Option<String>,
    settlement_date: Option<String>,
    settlement_quantity: Option<String>,
    settlement_movement_type: Option<String>,
    settlement_payment_type: Option<String>,
    security_instrument_id: Option<String>,
    plan_execution_order: Option<String>,
    plan_atomicity: Option<String>,
    source_address_observation: AddressParseObservation,
    target_address_observation: AddressParseObservation,
}

impl IsoMessageContext {
    /// Ledger identifier supplied in the message, if any.
    pub fn ledger_id(&self) -> Option<&str> {
        self.ledger_id.as_deref()
    }

    /// Account ID of the sender once resolved from hints or aliases.
    pub fn source_account_id(&self) -> Option<&str> {
        self.source_account_id.as_deref()
    }

    /// Optional textual address associated with the sender.
    pub fn source_account_address(&self) -> Option<&str> {
        self.source_account_address.as_deref()
    }

    /// Account ID of the recipient once resolved from hints or aliases.
    pub fn target_account_id(&self) -> Option<&str> {
        self.target_account_id.as_deref()
    }

    /// Optional textual address associated with the recipient.
    pub fn target_account_address(&self) -> Option<&str> {
        self.target_account_address.as_deref()
    }

    /// Asset definition inferred for the transfer.
    pub fn asset_definition_id(&self) -> Option<&str> {
        self.asset_definition_id.as_deref()
    }

    /// Specific asset instance referenced by the message.
    pub fn asset_id(&self) -> Option<&str> {
        self.asset_id.as_deref()
    }

    /// Settlement amount carried by the ISO payment or securities instruction.
    pub fn settlement_amount(&self) -> Option<&str> {
        self.settlement_amount.as_deref()
    }

    /// Settlement currency carried by the ISO payment or securities instruction.
    pub fn settlement_currency(&self) -> Option<&str> {
        self.settlement_currency.as_deref()
    }

    /// Requested or confirmed settlement date.
    pub fn settlement_date(&self) -> Option<&str> {
        self.settlement_date.as_deref()
    }

    /// Securities quantity carried by a securities settlement instruction.
    pub fn settlement_quantity(&self) -> Option<&str> {
        self.settlement_quantity.as_deref()
    }

    /// Securities movement type carried by a securities settlement instruction.
    pub fn settlement_movement_type(&self) -> Option<&str> {
        self.settlement_movement_type.as_deref()
    }

    /// Payment type carried by a securities settlement instruction.
    pub fn settlement_payment_type(&self) -> Option<&str> {
        self.settlement_payment_type.as_deref()
    }

    /// Financial instrument identifier carried by a securities settlement instruction.
    pub fn security_instrument_id(&self) -> Option<&str> {
        self.security_instrument_id.as_deref()
    }

    /// Durable execution-order plan captured from supplementary settlement data.
    pub fn plan_execution_order(&self) -> Option<&str> {
        self.plan_execution_order.as_deref()
    }

    /// Durable atomicity plan captured from supplementary settlement data.
    pub fn plan_atomicity(&self) -> Option<&str> {
        self.plan_atomicity.as_deref()
    }

    /// Parsed metadata captured when handling `SourceAccountAddress`.
    pub fn source_address_observation(&self) -> &AddressParseObservation {
        &self.source_address_observation
    }

    /// Parsed metadata captured when handling `TargetAccountAddress`.
    pub fn target_address_observation(&self) -> &AddressParseObservation {
        &self.target_address_observation
    }
}

#[derive(Clone, Debug, Default)]
pub struct AddressParseObservation {
    literal: Option<String>,
    domain_kind: Option<AddressDomainKind>,
    error_code: Option<&'static str>,
}

impl AddressParseObservation {
    fn from_success(literal: &str, address: &AccountAddress) -> Self {
        Self {
            literal: Some(literal.to_owned()),
            domain_kind: Some(address.domain_kind()),
            error_code: None,
        }
    }

    fn from_error(literal: &str, code: &'static str) -> Self {
        Self {
            literal: Some(literal.to_owned()),
            domain_kind: None,
            error_code: Some(code),
        }
    }

    pub fn error_code(&self) -> Option<&'static str> {
        self.error_code
    }

    pub fn domain_kind(&self) -> Option<AddressDomainKind> {
        self.domain_kind
    }

    pub fn domain_label(&self) -> Option<&'static str> {
        self.domain_kind.map(AddressDomainKind::as_str)
    }
}

/// Profile and idempotency metadata captured for an inbound ISO message.
#[derive(Clone, Debug, Default)]
pub struct IsoMessageMetadata {
    profile_id: Option<String>,
    message_type: Option<String>,
    business_service: Option<String>,
    business_message_id: Option<String>,
    uetr: Option<String>,
    payload_hash: Option<String>,
    reference_snapshot_id: Option<String>,
    embedded_signature_detected: bool,
}

impl IsoMessageMetadata {
    fn inbound(
        profile_id: &str,
        message_type: &str,
        business_service: Option<String>,
        business_message_id: Option<String>,
        uetr: Option<String>,
        payload_hash: String,
        reference_snapshot_id: String,
        embedded_signature_detected: bool,
    ) -> Self {
        Self {
            profile_id: Some(profile_id.to_owned()),
            message_type: Some(message_type.to_owned()),
            business_service,
            business_message_id,
            uetr,
            payload_hash: Some(payload_hash),
            reference_snapshot_id: Some(reference_snapshot_id),
            embedded_signature_detected,
        }
    }

    /// Selected rail profile identifier.
    pub fn profile_id(&self) -> Option<&str> {
        self.profile_id.as_deref()
    }

    /// ISO message family.
    pub fn message_type(&self) -> Option<&str> {
        self.message_type.as_deref()
    }

    /// Business service identifier from the Business Application Header.
    pub fn business_service(&self) -> Option<&str> {
        self.business_service.as_deref()
    }

    /// Business message identifier used for rail idempotency.
    pub fn business_message_id(&self) -> Option<&str> {
        self.business_message_id.as_deref()
    }

    /// UETR when present.
    pub fn uetr(&self) -> Option<&str> {
        self.uetr.as_deref()
    }

    /// SHA-256 hash of the submitted payload.
    pub fn payload_hash(&self) -> Option<&str> {
        self.payload_hash.as_deref()
    }

    /// Deterministic checksum of the reference-data snapshot used for validation.
    pub fn reference_snapshot_id(&self) -> Option<&str> {
        self.reference_snapshot_id.as_deref()
    }

    /// Whether an embedded XML signature subtree was observed.
    pub fn embedded_signature_detected(&self) -> bool {
        self.embedded_signature_detected
    }
}

/// Historical ISO bridge status transition.
#[derive(Clone, Debug)]
pub struct IsoStatusHistoryEntry {
    status: IsoMessageState,
    pacs002_code: Pacs002Status,
    updated_at: SystemTime,
    detail: Option<String>,
    reason_code: Option<String>,
}

impl IsoStatusHistoryEntry {
    fn new(record: &IsoMessageRecord) -> Self {
        Self {
            status: record.state,
            pacs002_code: record.derived_status(),
            updated_at: record.updated_at,
            detail: record.detail.clone(),
            reason_code: record.rejection_reason_code.clone(),
        }
    }

    /// Human-readable bridge state label.
    pub fn status_label(&self) -> &'static str {
        self.status.label()
    }

    /// ISO pacs.002 status code derived for this transition.
    pub fn pacs002_code(&self) -> &'static str {
        self.pacs002_code.code()
    }

    /// Transition timestamp.
    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }

    /// Optional transition detail.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// Optional ISO or proprietary reason code.
    pub fn reason_code(&self) -> Option<&str> {
        self.reason_code.as_deref()
    }
}

fn parse_account_address_literal(input: &str) -> (Option<String>, AddressParseObservation) {
    if input.is_empty() {
        return (None, AddressParseObservation::default());
    }
    match AccountAddress::parse_encoded(input, None) {
        Ok(address) => {
            let canonical = address.canonical_hex().unwrap_or_else(|_| input.to_owned());
            (
                Some(canonical),
                AddressParseObservation::from_success(input, &address),
            )
        }
        Err(err) => {
            let code = err.code_str();
            (
                Some(input.to_owned()),
                AddressParseObservation::from_error(input, code),
            )
        }
    }
}

fn parse_iso_account_hint(
    literal: &str,
    telemetry: &MaybeTelemetry,
    context: &'static str,
) -> Result<(AccountId, String), MsgError> {
    let parsed = routing::parse_account_literal(literal, telemetry, context)
        .map_err(|_| MsgError::ValidationFailed)?;
    let canonical = parsed.canonical().to_owned();
    Ok((parsed.into_account_id(), canonical))
}

#[derive(Clone, Debug)]
pub struct IsoMessageStatus {
    message_id: String,
    state: IsoMessageState,
    transaction_hash: Option<String>,
    detail: Option<String>,
    updated_at: SystemTime,
    settled_at: Option<SystemTime>,
    context: IsoMessageContext,
    metadata: IsoMessageMetadata,
    derived_status: Pacs002Status,
    hold_reason_code: Option<String>,
    change_reason_codes: Vec<String>,
    rejection_reason_code: Option<String>,
    status_history: Vec<IsoStatusHistoryEntry>,
}

impl IsoMessageStatus {
    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    pub fn status_label(&self) -> &'static str {
        self.state.label()
    }

    pub fn pacs002_code(&self) -> &'static str {
        self.derived_status.code()
    }

    pub fn transaction_hash(&self) -> Option<&str> {
        self.transaction_hash.as_deref()
    }

    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub fn updated_at(&self) -> SystemTime {
        self.updated_at
    }

    pub fn settled_at(&self) -> Option<SystemTime> {
        self.settled_at
    }

    pub fn ledger_id(&self) -> Option<&str> {
        self.context.ledger_id.as_deref()
    }

    pub fn source_account_id(&self) -> Option<&str> {
        self.context.source_account_id.as_deref()
    }

    pub fn source_account_address(&self) -> Option<&str> {
        self.context.source_account_address.as_deref()
    }

    pub fn target_account_id(&self) -> Option<&str> {
        self.context.target_account_id.as_deref()
    }

    pub fn target_account_address(&self) -> Option<&str> {
        self.context.target_account_address.as_deref()
    }

    pub fn asset_definition_id(&self) -> Option<&str> {
        self.context.asset_definition_id.as_deref()
    }

    pub fn asset_id(&self) -> Option<&str> {
        self.context.asset_id.as_deref()
    }

    pub fn settlement_amount(&self) -> Option<&str> {
        self.context.settlement_amount.as_deref()
    }

    pub fn settlement_currency(&self) -> Option<&str> {
        self.context.settlement_currency.as_deref()
    }

    pub fn settlement_date(&self) -> Option<&str> {
        self.context.settlement_date.as_deref()
    }

    pub fn settlement_quantity(&self) -> Option<&str> {
        self.context.settlement_quantity.as_deref()
    }

    pub fn settlement_movement_type(&self) -> Option<&str> {
        self.context.settlement_movement_type.as_deref()
    }

    pub fn settlement_payment_type(&self) -> Option<&str> {
        self.context.settlement_payment_type.as_deref()
    }

    pub fn security_instrument_id(&self) -> Option<&str> {
        self.context.security_instrument_id.as_deref()
    }

    pub fn plan_execution_order(&self) -> Option<&str> {
        self.context.plan_execution_order.as_deref()
    }

    pub fn plan_atomicity(&self) -> Option<&str> {
        self.context.plan_atomicity.as_deref()
    }

    pub fn derived_status(&self) -> Pacs002Status {
        self.derived_status
    }

    /// Profile/idempotency metadata captured for the message.
    pub fn metadata(&self) -> &IsoMessageMetadata {
        &self.metadata
    }

    pub fn hold_reason_code(&self) -> Option<&str> {
        self.hold_reason_code.as_deref()
    }

    pub fn change_reason_codes(&self) -> &[String] {
        &self.change_reason_codes
    }

    pub fn rejection_reason_code(&self) -> Option<&str> {
        self.rejection_reason_code.as_deref()
    }

    /// Status transition history for the message.
    pub fn status_history(&self) -> &[IsoStatusHistoryEntry] {
        &self.status_history
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IsoLifecycleOutcome {
    referenced_message_id: Option<String>,
    referenced_message_known: bool,
    lifecycle_status_code: Option<String>,
    lifecycle_reason_code: Option<String>,
    action: &'static str,
}

impl IsoLifecycleOutcome {
    pub(crate) fn referenced_message_id(&self) -> Option<&str> {
        self.referenced_message_id.as_deref()
    }

    pub(crate) fn referenced_message_known(&self) -> bool {
        self.referenced_message_known
    }

    pub(crate) fn lifecycle_status_code(&self) -> Option<&str> {
        self.lifecycle_status_code.as_deref()
    }

    pub(crate) fn lifecycle_reason_code(&self) -> Option<&str> {
        self.lifecycle_reason_code.as_deref()
    }

    pub(crate) fn action(&self) -> &'static str {
        self.action
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsoMessageState {
    Pending,
    Accepted,
    Rejected,
}

impl IsoMessageState {
    pub fn label(self) -> &'static str {
        match self {
            IsoMessageState::Pending => "Pending",
            IsoMessageState::Accepted => "Accepted",
            IsoMessageState::Rejected => "Rejected",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pacs002Status {
    Actc,
    Acsp,
    Acsc,
    Acwc,
    Pdng,
    Rjct,
}

impl Pacs002Status {
    pub fn code(self) -> &'static str {
        match self {
            Pacs002Status::Actc => "ACTC",
            Pacs002Status::Acsp => "ACSP",
            Pacs002Status::Acsc => "ACSC",
            Pacs002Status::Acwc => "ACWC",
            Pacs002Status::Pdng => "PDNG",
            Pacs002Status::Rjct => "RJCT",
        }
    }
}

#[derive(Clone, Debug)]
struct IsoMessageRecord {
    last_seen: Instant,
    updated_at: SystemTime,
    state: IsoMessageState,
    transaction_hash: Option<String>,
    detail: Option<String>,
    context: IsoMessageContext,
    metadata: IsoMessageMetadata,
    ledger_tx_queued: bool,
    settled_at: Option<SystemTime>,
    hold_reason_code: Option<String>,
    change_reason_codes: Vec<String>,
    rejection_reason_code: Option<String>,
    status_history: Vec<IsoStatusHistoryEntry>,
}

impl IsoMessageRecord {
    fn pending(now: Instant) -> Self {
        let mut record = Self {
            last_seen: now,
            updated_at: SystemTime::now(),
            state: IsoMessageState::Pending,
            transaction_hash: None,
            detail: None,
            context: IsoMessageContext::default(),
            metadata: IsoMessageMetadata::default(),
            ledger_tx_queued: false,
            settled_at: None,
            hold_reason_code: None,
            change_reason_codes: Vec::new(),
            rejection_reason_code: None,
            status_history: Vec::new(),
        };
        record.push_history();
        record
    }

    fn accepted(now: Instant, tx_hash: String) -> Self {
        let mut record = Self {
            last_seen: now,
            updated_at: SystemTime::now(),
            state: IsoMessageState::Accepted,
            transaction_hash: Some(tx_hash),
            detail: None,
            context: IsoMessageContext::default(),
            metadata: IsoMessageMetadata::default(),
            ledger_tx_queued: true,
            settled_at: None,
            hold_reason_code: None,
            change_reason_codes: Vec::new(),
            rejection_reason_code: None,
            status_history: Vec::new(),
        };
        record.push_history();
        record
    }

    fn rejected(now: Instant, detail: Option<String>) -> Self {
        let mut record = Self {
            last_seen: now,
            updated_at: SystemTime::now(),
            state: IsoMessageState::Rejected,
            transaction_hash: None,
            detail,
            context: IsoMessageContext::default(),
            metadata: IsoMessageMetadata::default(),
            ledger_tx_queued: false,
            settled_at: None,
            hold_reason_code: None,
            change_reason_codes: Vec::new(),
            rejection_reason_code: None,
            status_history: Vec::new(),
        };
        record.push_history();
        record
    }

    fn derived_status(&self) -> Pacs002Status {
        match self.state {
            IsoMessageState::Rejected => Pacs002Status::Rjct,
            IsoMessageState::Accepted => {
                if self.settled_at.is_some() {
                    Pacs002Status::Acsc
                } else {
                    Pacs002Status::Acsp
                }
            }
            IsoMessageState::Pending => {
                if self.hold_reason_code.is_some() {
                    Pacs002Status::Pdng
                } else if !self.change_reason_codes.is_empty() {
                    Pacs002Status::Acwc
                } else if self.ledger_tx_queued {
                    Pacs002Status::Acsp
                } else {
                    Pacs002Status::Actc
                }
            }
        }
    }

    fn set_hold_reason(&mut self, reason: Option<String>) {
        self.hold_reason_code = reason;
    }

    fn clear_hold(&mut self) {
        self.hold_reason_code = None;
    }

    fn replace_change_reason_codes(&mut self, mut codes: Vec<String>) {
        dedup_codes(&mut codes);
        self.change_reason_codes = codes;
    }

    fn add_change_reason_code(&mut self, code: String) {
        if !self
            .change_reason_codes
            .iter()
            .any(|existing| existing == &code)
        {
            self.change_reason_codes.push(code);
        }
    }

    fn set_queued(&mut self) {
        self.ledger_tx_queued = true;
    }

    fn mark_settled(&mut self, when: SystemTime) {
        self.settled_at = Some(when);
    }

    fn push_history(&mut self) {
        let entry = IsoStatusHistoryEntry::new(self);
        let should_push = self.status_history.last().is_none_or(|last| {
            last.status != entry.status
                || last.pacs002_code != entry.pacs002_code
                || last.detail != entry.detail
                || last.reason_code != entry.reason_code
        });
        if should_push {
            self.status_history.push(entry);
        }
    }
}

const ISO_PACS008_CONTEXT: &str = "/v1/iso20022/pacs008";
const ISO_PACS009_CONTEXT: &str = "/v1/iso20022/pacs009";

fn parse_config_account_id(literal: &str, field: &str) -> eyre::Result<AccountId> {
    AccountId::parse_encoded(literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .wrap_err_with(|| format!("{field} must parse as an account identifier"))
}

fn load_profile_catalog(
    config: &actual::IsoBridge,
) -> eyre::Result<HashMap<String, TradfiRailProfile>> {
    let mut catalog = profiles::default_profile_catalog();
    let global_policy = config
        .embedded_signature_policy
        .as_deref()
        .map(parse_signature_policy)
        .transpose()?;
    if let Some(policy) = global_policy {
        for profile in catalog.values_mut() {
            profile.embedded_signature_policy = policy;
        }
    }

    for override_profile in &config.profiles {
        let profile = convert_config_profile(override_profile, global_policy)?;
        catalog.insert(profile.id.clone(), profile);
    }

    let default_id = config.default_profile.trim();
    if default_id.is_empty() {
        eyre::bail!("iso_bridge default_profile must not be empty");
    }
    if !catalog.contains_key(default_id) {
        eyre::bail!("iso_bridge default_profile `{default_id}` is not configured");
    }

    Ok(catalog.into_iter().collect())
}

fn convert_config_profile(
    config: &actual::IsoBridgeProfile,
    global_policy: Option<EmbeddedSignaturePolicy>,
) -> eyre::Result<TradfiRailProfile> {
    let id = config.id.trim();
    if id.is_empty() {
        eyre::bail!("iso_bridge profile id must not be empty");
    }
    let rail = TradfiRail::parse(&config.rail).ok_or_else(|| {
        eyre::eyre!(
            "iso_bridge profile `{id}` has unknown rail `{}`",
            config.rail
        )
    })?;
    let embedded_signature_policy = config
        .embedded_signature_policy
        .as_deref()
        .map(parse_signature_policy)
        .transpose()?
        .or(global_policy)
        .unwrap_or(EmbeddedSignaturePolicy::RecordOnly);
    let trusted_public_key_sha256 = normalise_sha256_pins(
        &config.trusted_public_key_sha256,
        &format!("iso_bridge profile `{id}` trusted_public_key_sha256"),
    )?;
    let trusted_certificate_sha256 = normalise_sha256_pins(
        &config.trusted_certificate_sha256,
        &format!("iso_bridge profile `{id}` trusted_certificate_sha256"),
    )?;
    let required_reference_datasets = config
        .required_reference_datasets
        .iter()
        .map(|dataset| {
            ReferenceDatasetRequirement::parse(dataset).ok_or_else(|| {
                eyre::eyre!("iso_bridge profile `{id}` has unknown reference dataset `{dataset}`")
            })
        })
        .collect::<eyre::Result<Vec<_>>>()?;
    let message_profiles = config
        .message_profiles
        .iter()
        .map(|message| convert_config_message_profile(id, message))
        .collect::<eyre::Result<Vec<_>>>()?;
    if message_profiles.is_empty() {
        eyre::bail!("iso_bridge profile `{id}` must define at least one message profile");
    }
    Ok(TradfiRailProfile {
        id: id.to_owned(),
        rail,
        embedded_signature_policy,
        trusted_public_key_sha256,
        trusted_certificate_sha256,
        required_reference_datasets,
        message_profiles,
    })
}

fn normalise_sha256_pins(values: &[String], field: &str) -> eyre::Result<Vec<String>> {
    values
        .iter()
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.len() != 64 || !trimmed.chars().all(|ch| ch.is_ascii_hexdigit()) {
                eyre::bail!("{field} entry `{value}` must be a SHA-256 hex string");
            }
            let canonical = trimmed.to_ascii_lowercase();
            if trimmed != canonical {
                eyre::bail!("{field} entry `{value}` must use lowercase canonical hex");
            }
            if canonical.chars().all(|ch| ch == '0') {
                eyre::bail!("{field} entries must not be all zero");
            }
            Ok(canonical)
        })
        .collect()
}

fn convert_config_message_profile(
    profile_id: &str,
    config: &actual::IsoMessageProfile,
) -> eyre::Result<MessageProfile> {
    let message_type = config.message_type.trim();
    if message_type.is_empty() {
        eyre::bail!("iso_bridge profile `{profile_id}` has an empty message_type");
    }
    let direction = MessageDirection::parse(&config.direction).ok_or_else(|| {
        eyre::eyre!(
            "iso_bridge profile `{profile_id}` message `{message_type}` has unknown direction `{}`",
            config.direction
        )
    })?;
    let structured_address_mode =
        StructuredAddressMode::parse(&config.structured_address_mode).ok_or_else(|| {
            eyre::eyre!(
                "iso_bridge profile `{profile_id}` message `{message_type}` has unknown structured_address_mode `{}`",
                config.structured_address_mode
            )
        })?;
    let amount_minor_units = config
        .amount_minor_units
        .iter()
        .map(|entry| {
            let currency = normalise_currency(&entry.currency);
            if !ivm::iso20022::validate_identifier(IdentifierKind::Currency, &currency) {
                eyre::bail!(
                    "iso_bridge profile `{profile_id}` message `{message_type}` has invalid currency `{}`",
                    entry.currency
                );
            }
            Ok((currency, entry.minor_units))
        })
        .collect::<eyre::Result<BTreeMap<_, _>>>()?;
    Ok(MessageProfile {
        message_type: message_type.to_owned(),
        direction,
        versions: config.versions.clone(),
        business_services: config.business_services.clone(),
        require_app_header: config.require_app_header,
        require_business_service: config.require_business_service,
        require_uetr: config.require_uetr,
        structured_address_mode,
        supplementary_data_max_bytes: config.supplementary_data_max_bytes,
        amount_minor_units,
    })
}

fn parse_signature_policy(value: &str) -> eyre::Result<EmbeddedSignaturePolicy> {
    EmbeddedSignaturePolicy::parse(value)
        .ok_or_else(|| eyre::eyre!("unknown ISO embedded signature policy `{value}`"))
}

impl Iso20022BridgeRuntime {
    /// Construct runtime helper from user configuration.
    pub fn from_config(config: &actual::IsoBridge) -> eyre::Result<Option<Self>> {
        if !config.enabled {
            return Ok(None);
        }

        let signer = config
            .signer
            .as_ref()
            .ok_or_else(|| eyre::eyre!("iso_bridge signer must be configured when enabled"))?;
        let signer_account =
            parse_config_account_id(&signer.account_id, "iso_bridge signer account_id")?;
        let signer_private_key = signer.private_key.clone();

        let mut aliases = HashMap::new();
        let mut alias_indices = HashMap::new();
        let mut index_aliases = BTreeMap::new();
        for (position, alias) in config.account_aliases.iter().enumerate() {
            let iban = normalise_iban(&alias.iban);
            if !ivm::iso20022::validate_identifier(IdentifierKind::Iban, &iban) {
                eyre::bail!(
                    "iso_bridge account alias `{}` is not a valid IBAN",
                    alias.iban
                );
            }
            let account_id = parse_config_account_id(
                &alias.account_id,
                &format!("iso_bridge account alias `{iban}` account_id"),
            )?;
            let index = AliasIndex(position as u64);
            alias_indices.insert(iban.clone(), index);
            index_aliases.insert(index, (iban.clone(), account_id.clone()));
            aliases.insert(iban, account_id);
        }

        let mut currencies = HashMap::new();
        for binding in &config.currency_assets {
            let currency = normalise_currency(&binding.currency);
            if !ivm::iso20022::validate_identifier(IdentifierKind::Currency, &currency) {
                eyre::bail!(
                    "iso_bridge currency binding `{}` is not a valid ISO 4217 code",
                    binding.currency
                );
            }
            let asset_selector = validate_asset_definition_selector(&binding.asset_definition)
                .wrap_err_with(|| format!("invalid asset definition for currency {currency}"))?;
            currencies.insert(currency, asset_selector);
        }

        let reference_data = Arc::new(ReferenceDataSnapshots::from_config(&config.reference_data));
        let profiles = load_profile_catalog(config)?;

        let runtime = Iso20022BridgeRuntime {
            signer_account,
            signer_private_key,
            account_aliases: Arc::new(aliases),
            alias_indices: Arc::new(alias_indices),
            index_aliases: Arc::new(index_aliases),
            currency_assets: Arc::new(currencies),
            reference_data,
            default_profile_id: config.default_profile.trim().to_owned(),
            profiles: Arc::new(profiles),
            store_dir: config.store_dir.clone(),
            dedupe_ttl: Duration::from_secs(config.dedupe_ttl_secs),
            records: DashMap::new(),
            tx_hash_index: DashMap::new(),
            payload_hash_index: DashMap::new(),
            uetr_index: DashMap::new(),
        };

        runtime.load_persisted_records();

        Ok(Some(runtime))
    }

    /// Resolve an IBAN into an on-ledger account identifier.
    pub fn resolve_account(&self, iban: &str) -> Option<AccountId> {
        let iban = normalise_iban(iban);
        self.account_aliases.get(&iban).cloned()
    }

    /// Look up the canonical index assigned to an alias (IBAN) if present.
    pub fn resolve_alias_index(&self, alias: &str) -> Option<AliasIndex> {
        let alias = normalise_iban(alias);
        self.alias_indices.get(&alias).copied()
    }

    /// Resolve an alias index back into the normalized alias and account identifier.
    pub fn resolve_account_by_index(&self, index: AliasIndex) -> Option<(String, AccountId)> {
        self.index_aliases.get(&index).cloned()
    }

    /// Resolve an ISO 4217 currency code into an asset definition identifier.
    pub fn resolve_asset(
        &self,
        world: &impl WorldReadOnly,
        now_ms: u64,
        currency: &str,
    ) -> Option<AssetDefinitionId> {
        let currency = normalise_currency(currency);
        let selector = self.currency_assets.get(&currency)?;
        resolve_asset_definition_selector(world, selector, now_ms)
    }

    /// Access the cached ISO reference datasets.
    pub fn reference_data(&self) -> &ReferenceDataSnapshots {
        &self.reference_data
    }

    /// Return the configured default rail profile.
    pub fn default_profile(&self) -> &TradfiRailProfile {
        self.profiles
            .get(&self.default_profile_id)
            .expect("default ISO profile validated during runtime construction")
    }

    /// Resolve a profile identifier, falling back to the configured default.
    pub fn resolve_profile(&self, profile_id: Option<&str>) -> Option<&TradfiRailProfile> {
        let selected = profile_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(&self.default_profile_id);
        self.profiles.get(selected)
    }

    /// Validate profile policy for a parsed inbound message and produce audit metadata.
    pub fn validate_profile_submission(
        &self,
        profile: &TradfiRailProfile,
        message_type: &str,
        parsed: &ParsedMessage,
        payload: &[u8],
    ) -> Result<IsoMessageMetadata, MsgError> {
        let message_profile = profile
            .message_profile(message_type, MessageDirection::Inbound)
            .ok_or(MsgError::ValidationFailed)?;
        self.require_profile_reference_data(profile)?;
        let definition_id =
            message_definition_id(parsed, message_type).ok_or(MsgError::ValidationFailed)?;
        if !message_profile.allows_version(definition_id) {
            return Err(MsgError::UnknownMessageType);
        }
        let business_message_id = business_message_id(parsed).map(ToOwned::to_owned);
        if message_profile.require_app_header
            && (app_header_business_message_id(parsed).is_none()
                || app_header_message_definition_id(parsed).is_none()
                || app_header_creation_date(parsed).is_none())
        {
            return Err(MsgError::MissingField("AppHdr"));
        }
        let business_service = business_service(parsed).map(ToOwned::to_owned);
        if message_profile.require_business_service {
            let service = business_service
                .as_deref()
                .ok_or_else(|| MsgError::MissingField("AppHdr/BizSvc"))?;
            if !message_profile.allows_business_service(service) {
                return Err(MsgError::InvalidValue {
                    field: "AppHdr/BizSvc".to_owned(),
                    kind: InvalidValueKind::Enum,
                });
            }
        } else if let Some(service) = business_service.as_deref()
            && !message_profile.allows_business_service(service)
        {
            return Err(MsgError::InvalidValue {
                field: "AppHdr/BizSvc".to_owned(),
                kind: InvalidValueKind::Enum,
            });
        }
        let uetr = uetr(parsed).map(ToOwned::to_owned);
        if message_profile.require_uetr && uetr.is_none() {
            return Err(MsgError::MissingField("UETR"));
        }
        self.validate_amount_minor_units(message_profile, parsed)?;
        self.validate_supplementary_data_limit(message_profile, parsed)?;
        self.validate_structured_address_mode(message_profile, parsed)?;
        let embedded_signature_detected =
            has_embedded_signature_marker(parsed) || payload_has_embedded_signature(payload);
        match profile.embedded_signature_policy {
            EmbeddedSignaturePolicy::RecordOnly => {}
            EmbeddedSignaturePolicy::RejectUnsupported if embedded_signature_detected => {
                return Err(MsgError::ValidationFailed);
            }
            EmbeddedSignaturePolicy::RejectUnsupported => {}
            EmbeddedSignaturePolicy::RequireVerified => {
                if !embedded_signature_detected || !verify_embedded_xml_signature(payload, profile)?
                {
                    return Err(MsgError::ValidationFailed);
                }
            }
        }
        Ok(IsoMessageMetadata::inbound(
            &profile.id,
            message_type,
            business_service,
            business_message_id,
            uetr,
            sha256_hex(payload),
            self.reference_data.snapshot_id(),
            embedded_signature_detected,
        ))
    }

    /// Perform a deduplication check for the provided message identifier.
    /// Returns `true` when the identifier is new (and records it), or `false`
    /// when a still-active entry already exists.
    pub fn check_and_record_message(&self, message_id: &str) -> bool {
        self.check_and_record_inbound(message_id, IsoMessageMetadata::default())
    }

    /// Perform idempotency checks and record a new inbound message.
    pub fn check_and_record_inbound(&self, message_id: &str, metadata: IsoMessageMetadata) -> bool {
        let now = Instant::now();
        self.prune_expired(now);
        if let Some(mut existing) = self.records.get_mut(message_id) {
            let expired = now.saturating_duration_since(existing.last_seen) > self.dedupe_ttl;
            if expired || existing.state == IsoMessageState::Rejected {
                if self.metadata_conflicts(message_id, &metadata) {
                    return false;
                }
                self.remove_record_indexes(message_id, &existing);
                *existing = IsoMessageRecord::pending(now);
                existing.metadata = metadata.clone();
                existing.push_history();
                drop(existing);
                self.insert_metadata_indexes(message_id, &metadata);
                self.persist_message(message_id);
                true
            } else {
                false
            }
        } else if self.metadata_conflicts(message_id, &metadata) {
            false
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.metadata = metadata.clone();
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
            self.insert_metadata_indexes(message_id, &metadata);
            self.persist_message(message_id);
            true
        }
    }

    /// Remove a tracked message identifier from the dedupe cache (e.g. after a failed submission).
    pub fn remove_message(&self, message_id: &str) {
        if let Some((_, record)) = self.records.remove(message_id) {
            self.remove_record_indexes(message_id, &record);
        }
        self.remove_persisted_message(message_id);
    }

    /// Record supplementary ledger/account context attached to the message.
    pub fn update_message_context(&self, message_id: &str, context: IsoMessageContext) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.context = context;
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.context = context;
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Mark the provided message as queued for ledger execution.
    pub fn mark_queued(&self, message_id: &str) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.set_queued();
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.set_queued();
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Flag a message as pending due to screening/manual hold with an optional ISO reason code.
    pub fn mark_hold(&self, message_id: &str, reason_code: Option<&str>) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.state = IsoMessageState::Pending;
            existing.settled_at = None;
            existing.rejection_reason_code = None;
            existing.set_hold_reason(reason_code.map(std::borrow::ToOwned::to_owned));
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.set_hold_reason(reason_code.map(std::borrow::ToOwned::to_owned));
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Clear any previously-set hold indicator for the message.
    pub fn clear_hold(&self, message_id: &str) {
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = Instant::now();
            existing.updated_at = SystemTime::now();
            existing.clear_hold();
            existing.push_history();
        }
        self.persist_message(message_id);
    }

    /// Replace the change-reason codes recorded for the message.
    pub fn replace_change_reason_codes<I, S>(&self, message_id: &str, codes: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let now = Instant::now();
        let codes_vec = codes.into_iter().map(Into::into).collect::<Vec<_>>();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.replace_change_reason_codes(codes_vec);
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.replace_change_reason_codes(codes_vec);
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Append a change-reason code for the message (deduplicated).
    pub fn add_change_reason_code(&self, message_id: &str, code: &str) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.add_change_reason_code(code.to_owned());
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.add_change_reason_code(code.to_owned());
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Mark the message as fully settled on-ledger.
    pub fn mark_settled(&self, message_id: &str, settled_at: SystemTime) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.state = IsoMessageState::Accepted;
            existing.set_queued();
            existing.mark_settled(settled_at);
            existing.clear_hold();
            existing.rejection_reason_code = None;
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.state = IsoMessageState::Accepted;
            record.set_queued();
            record.mark_settled(settled_at);
            record.clear_hold();
            record.rejection_reason_code = None;
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Mark the transaction identified by `tx_hash` as applied and fully settled.
    pub fn mark_transaction_applied(&self, tx_hash: &str, settled_at: SystemTime) {
        if let Some((_, message_id)) = self.tx_hash_index.remove(tx_hash) {
            self.mark_settled(&message_id, settled_at);
        }
    }

    /// Mark the transaction identified by `tx_hash` as rejected.
    pub fn mark_transaction_rejected(
        &self,
        tx_hash: &str,
        reason: Option<&TransactionRejectionReason>,
    ) {
        if let Some((_, message_id)) = self.tx_hash_index.remove(tx_hash) {
            let (detail, reason_code) = reason
                .map(Self::rejection_reason_metadata)
                .map(|(code, detail)| (Some(detail), Some(code)))
                .unwrap_or_else(|| {
                    (
                        Some("transaction rejected".to_owned()),
                        Some("PRTRY:TX_REJECTED".to_owned()),
                    )
                });
            self.mark_rejected(&message_id, detail, reason_code.as_deref());
        }
    }

    /// Mark the transaction identified by `tx_hash` as expired in the queue.
    pub fn mark_transaction_expired(&self, tx_hash: &str) {
        if let Some((_, message_id)) = self.tx_hash_index.remove(tx_hash) {
            self.mark_rejected(
                &message_id,
                Some("transaction expired before admission".to_owned()),
                Some("ED07"),
            );
        }
    }

    fn rejection_reason_metadata(reason: &TransactionRejectionReason) -> (String, String) {
        match reason {
            TransactionRejectionReason::AccountDoesNotExist(_) => {
                ("AC01".to_owned(), "Account does not exist".to_owned())
            }
            TransactionRejectionReason::LimitCheck(err) => (
                "BE01".to_owned(),
                format!("Transaction limit check failed: {}", err.reason),
            ),
            TransactionRejectionReason::Validation(fail) => match fail {
                ValidationFail::AxtReject(ctx) => {
                    let mut detail =
                        format!("AXT rejection ({}): {}", ctx.reason.label(), ctx.detail);
                    if ctx.snapshot_version > 0 {
                        let _ = FmtWrite::write_fmt(
                            &mut detail,
                            format_args!(" snapshot_version={}", ctx.snapshot_version),
                        );
                    }
                    if let Some(dsid) = ctx.dataspace {
                        let _ = FmtWrite::write_fmt(
                            &mut detail,
                            format_args!(" dsid={}", dsid.as_u64()),
                        );
                    }
                    if let Some(lane) = ctx.lane {
                        let _ = FmtWrite::write_fmt(
                            &mut detail,
                            format_args!(" lane={}", lane.as_u32()),
                        );
                    }
                    if let Some(era) = ctx.next_min_handle_era {
                        let _ = FmtWrite::write_fmt(
                            &mut detail,
                            format_args!(" next_min_handle_era={era}"),
                        );
                    }
                    if let Some(sub) = ctx.next_min_sub_nonce {
                        let _ = FmtWrite::write_fmt(
                            &mut detail,
                            format_args!(" next_min_sub_nonce={sub}"),
                        );
                    }
                    (format!("PRTRY:{}", ctx.reason.code()), detail)
                }
                other => ("BE01".to_owned(), format!("Validation failed: {other}")),
            },
            TransactionRejectionReason::InstructionExecution(fail) => (
                "PRTRY:INSTRUCTION_EXEC".to_owned(),
                format!("Instruction execution failed: {}", fail.reason),
            ),
            TransactionRejectionReason::IvmExecution(fail) => {
                ("PRTRY:IVM_EXEC".to_owned(), fail.reason.clone())
            }
            TransactionRejectionReason::TriggerExecution(fail) => (
                "PRTRY:TRIGGER_EXEC".to_owned(),
                format!("Trigger execution failed: {fail}"),
            ),
        }
    }

    /// Mark the provided message as successfully submitted on-chain.
    pub fn mark_accepted(&self, message_id: &str, transaction_hash: &str) {
        let now = Instant::now();
        let tx_hash = transaction_hash.to_owned();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            if let Some(old_hash) = existing.transaction_hash.replace(tx_hash.clone()) {
                if old_hash != tx_hash {
                    self.tx_hash_index.remove(&old_hash);
                }
            }
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.state = IsoMessageState::Accepted;
            existing.detail = None;
            existing.set_queued();
            existing.settled_at = None;
            existing.hold_reason_code = None;
            existing.change_reason_codes.clear();
            existing.rejection_reason_code = None;
            existing.push_history();
        } else {
            self.records.insert(
                message_id.to_owned(),
                IsoMessageRecord::accepted(now, tx_hash.clone()),
            );
        }
        self.tx_hash_index.insert(tx_hash, message_id.to_owned());
        self.persist_message(message_id);
    }

    /// Mark an inbound lifecycle message as durably accepted without creating a ledger transfer.
    pub(crate) fn mark_lifecycle_accepted(&self, message_id: &str, detail: Option<String>) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            if let Some(old_hash) = existing.transaction_hash.take() {
                self.tx_hash_index.remove(&old_hash);
            }
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.state = IsoMessageState::Accepted;
            existing.detail = detail;
            existing.ledger_tx_queued = false;
            existing.settled_at = None;
            existing.hold_reason_code = None;
            existing.change_reason_codes.clear();
            existing.rejection_reason_code = None;
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.state = IsoMessageState::Accepted;
            record.detail = detail;
            record.status_history.clear();
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Mark the provided message as rejected and record the reason.
    pub fn mark_rejected(
        &self,
        message_id: &str,
        reason: Option<String>,
        reason_code: Option<&str>,
    ) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            if let Some(old_hash) = existing.transaction_hash.take() {
                self.tx_hash_index.remove(&old_hash);
            }
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.state = IsoMessageState::Rejected;
            existing.detail = reason;
            existing.ledger_tx_queued = false;
            existing.settled_at = None;
            existing.hold_reason_code = None;
            existing.change_reason_codes.clear();
            existing.rejection_reason_code = reason_code.map(std::borrow::ToOwned::to_owned);
            existing.push_history();
        } else {
            let mut record = IsoMessageRecord::rejected(now, reason);
            record.rejection_reason_code = reason_code.map(std::borrow::ToOwned::to_owned);
            record.status_history.clear();
            record.push_history();
            self.records.insert(message_id.to_owned(), record);
        }
        self.persist_message(message_id);
    }

    /// Retrieve the current status of a processed ISO 20022 message.
    pub fn message_status(&self, message_id: &str) -> Option<IsoMessageStatus> {
        self.records.get(message_id).map(|record| {
            let record = record.clone();
            let derived_status = record.derived_status();
            let hold_reason_code = record.hold_reason_code.clone();
            let change_reason_codes = record.change_reason_codes.clone();
            let context = record.context;
            IsoMessageStatus {
                message_id: message_id.to_owned(),
                state: record.state,
                transaction_hash: record.transaction_hash.clone(),
                detail: record.detail.clone(),
                updated_at: record.updated_at,
                settled_at: record.settled_at,
                context,
                metadata: record.metadata.clone(),
                derived_status,
                hold_reason_code,
                change_reason_codes,
                rejection_reason_code: record.rejection_reason_code.clone(),
                status_history: record.status_history.clone(),
            }
        })
    }

    /// Determine the durable identifier for an inbound lifecycle message.
    pub(crate) fn lifecycle_message_id(
        message_type: &str,
        parsed: &ParsedMessage,
    ) -> Result<String, MsgError> {
        let id = business_message_id(parsed)
            .or_else(|| parsed.field_text("Assgnmt/Id"))
            .or_else(|| parsed.field_text("TxId"))
            .or_else(|| lifecycle_referenced_message_id(message_type, parsed))
            .ok_or(MsgError::MissingField("MsgId"))?;
        let id = id.trim();
        if id.is_empty() {
            return Err(MsgError::MissingField("MsgId"));
        }
        if matches!(message_type, "sese.023" | "sese.024" | "sese.025")
            && business_message_id(parsed).is_none()
        {
            Ok(format!("{message_type}:{id}"))
        } else {
            Ok(id.to_owned())
        }
    }

    /// Apply an inbound lifecycle message to the referenced durable record when present.
    pub(crate) fn apply_inbound_lifecycle_message(
        &self,
        message_id: &str,
        message_type: &str,
        parsed: &ParsedMessage,
    ) -> Result<IsoLifecycleOutcome, MsgError> {
        let referenced_message_id = lifecycle_referenced_message_id(message_type, parsed)
            .map(ToOwned::to_owned)
            .map(|id| {
                if matches!(message_type, "sese.024" | "sese.025") {
                    format!("sese.023:{id}")
                } else {
                    id
                }
            });
        let status_code = lifecycle_status_code(message_type, parsed).map(ToOwned::to_owned);
        let reason_code = lifecycle_reason_code(parsed).map(ToOwned::to_owned);
        let detail = lifecycle_detail(message_type, parsed, status_code.as_deref());
        let referenced_message_known = referenced_message_id
            .as_deref()
            .is_some_and(|id| self.records.contains_key(id));
        let mut action = "recorded";

        if let Some(original_id) = referenced_message_id.as_deref()
            && referenced_message_known
        {
            action = self.apply_lifecycle_update(
                original_id,
                message_type,
                status_code.as_deref(),
                reason_code.as_deref(),
                detail.as_deref(),
            );
        }

        if let Some(context) = lifecycle_context(message_type, parsed) {
            self.update_message_context(message_id, context);
        }

        self.mark_lifecycle_accepted(
            message_id,
            Some(format!(
                "recorded inbound ISO 20022 {message_type} lifecycle message"
            )),
        );

        Ok(IsoLifecycleOutcome {
            referenced_message_id,
            referenced_message_known,
            lifecycle_status_code: status_code,
            lifecycle_reason_code: reason_code,
            action,
        })
    }

    /// Create a signed transfer transaction from a validated pacs.008 message.
    pub fn build_pacs008_transaction(
        &self,
        parsed: &ParsedMessage,
        world: &impl WorldReadOnly,
        now_ms: u64,
        chain_id: &ChainId,
        telemetry: &MaybeTelemetry,
    ) -> Result<
        (
            iroha_data_model::transaction::SignedTransaction,
            IsoMessageContext,
        ),
        MsgError,
    > {
        let debtor_iban = require_identifier(
            "DbtrAcct",
            IdentifierKind::Iban,
            parsed
                .field_text("DbtrAcct")
                .ok_or(MsgError::ValidationFailed)?,
        )?;
        let creditor_iban = require_identifier(
            "CdtrAcct",
            IdentifierKind::Iban,
            parsed
                .field_text("CdtrAcct")
                .ok_or(MsgError::ValidationFailed)?,
        )?;
        let currency = require_identifier(
            "IntrBkSttlmCcy",
            IdentifierKind::Currency,
            parsed
                .field_text("IntrBkSttlmCcy")
                .ok_or(MsgError::ValidationFailed)?,
        )?;
        let amount_raw = parsed
            .field_text("IntrBkSttlmAmt")
            .ok_or(MsgError::ValidationFailed)?;
        let debtor_agent = parsed
            .field_text("DbtrAgt")
            .ok_or(MsgError::ValidationFailed)?;
        self.require_bic("DbtrAgt", debtor_agent)?;
        let creditor_agent = parsed
            .field_text("CdtrAgt")
            .ok_or(MsgError::ValidationFailed)?;
        self.require_bic("CdtrAgt", creditor_agent)?;

        let mut context = IsoMessageContext {
            ledger_id: Some(chain_id.as_str().to_owned()),
            settlement_amount: Some(amount_raw.trim().to_owned()),
            settlement_currency: Some(currency.clone()),
            ..IsoMessageContext::default()
        };

        if let Some(ledger_hint) = parsed.field_text("SplmtryData/LedgerId") {
            if ledger_hint != chain_id.as_str() {
                return Err(MsgError::ValidationFailed);
            }
            context.ledger_id = Some(ledger_hint.to_owned());
        }

        let debtor = if let Some(hint) = parsed.field_text("SplmtryData/SourceAccountId") {
            let (account, canonical) =
                parse_iso_account_hint(hint, telemetry, ISO_PACS008_CONTEXT)?;
            context.source_account_id = Some(canonical);
            account
        } else {
            let account =
                self.resolve_account(&debtor_iban)
                    .ok_or_else(|| MsgError::InvalidIdentifier {
                        field: "DbtrAcct".to_owned(),
                        kind: IdentifierKind::Iban,
                    })?;
            context.source_account_id = Some(account.to_string());
            account
        };
        if let Some(address) = parsed.field_text("SplmtryData/SourceAccountAddress") {
            let trimmed = address.trim();
            if !trimmed.is_empty() {
                let (value, observation) = parse_account_address_literal(trimmed);
                if let Some(value) = value {
                    context.source_account_address = Some(value);
                }
                context.source_address_observation = observation;
            }
        }

        let creditor = if let Some(hint) = parsed.field_text("SplmtryData/TargetAccountId") {
            let (account, canonical) =
                parse_iso_account_hint(hint, telemetry, ISO_PACS008_CONTEXT)?;
            context.target_account_id = Some(canonical);
            account
        } else {
            let account = self.resolve_account(&creditor_iban).ok_or_else(|| {
                MsgError::InvalidIdentifier {
                    field: "CdtrAcct".to_owned(),
                    kind: IdentifierKind::Iban,
                }
            })?;
            context.target_account_id = Some(account.to_string());
            account
        };
        if let Some(address) = parsed.field_text("SplmtryData/TargetAccountAddress") {
            let trimmed = address.trim();
            if !trimmed.is_empty() {
                let (value, observation) = parse_account_address_literal(trimmed);
                if let Some(value) = value {
                    context.target_account_address = Some(value);
                }
                context.target_address_observation = observation;
            }
        }

        let asset_definition =
            if let Some(hint) = parsed.field_text("SplmtryData/AssetDefinitionId") {
                let definition = resolve_asset_definition_selector(world, hint, now_ms)
                    .ok_or(MsgError::ValidationFailed)?;
                context.asset_definition_id = Some(definition.to_string());
                definition
            } else {
                let definition = self
                    .resolve_asset(world, now_ms, &currency)
                    .ok_or_else(|| MsgError::InvalidIdentifier {
                        field: "IntrBkSttlmCcy".to_owned(),
                        kind: IdentifierKind::Currency,
                    })?;
                context.asset_definition_id = Some(definition.to_string());
                definition
            };
        let amount = Numeric::from_str(amount_raw).map_err(|_| MsgError::ValidationFailed)?;
        let asset = AssetId::new(asset_definition.clone(), debtor.clone());
        let asset_id_str = asset.to_string();
        context.asset_id = Some(asset_id_str);
        let transfer = Transfer::asset_numeric(asset, amount, creditor);

        let mut metadata = Metadata::default();
        for (key, value) in [
            ("iso20022_ledger_id", context.ledger_id.as_deref()),
            (
                "iso20022_source_account_id",
                context.source_account_id.as_deref(),
            ),
            (
                "iso20022_source_account_address",
                context.source_account_address.as_deref(),
            ),
            (
                "iso20022_target_account_id",
                context.target_account_id.as_deref(),
            ),
            (
                "iso20022_target_account_address",
                context.target_account_address.as_deref(),
            ),
            (
                "iso20022_asset_definition_id",
                context.asset_definition_id.as_deref(),
            ),
            ("iso20022_asset_id", context.asset_id.as_deref()),
            (
                "iso20022_settlement_amount",
                context.settlement_amount.as_deref(),
            ),
            (
                "iso20022_settlement_currency",
                context.settlement_currency.as_deref(),
            ),
        ] {
            if let Some(value) = value {
                insert_metadata_value(&mut metadata, key, value)?;
            }
        }

        let mut builder = TransactionBuilder::new(chain_id.clone(), self.signer_account.clone())
            .with_instructions(core::iter::once(InstructionBox::from(transfer)));
        if metadata.iter().len() > 0 {
            builder = builder.with_metadata(metadata);
        }

        let transaction = builder.sign(&self.signer_private_key);
        Ok((transaction, context))
    }

    /// Create a signed transfer transaction from a validated pacs.009 message.
    pub fn build_pacs009_transaction(
        &self,
        parsed: &ParsedMessage,
        world: &impl WorldReadOnly,
        now_ms: u64,
        chain_id: &ChainId,
        telemetry: &MaybeTelemetry,
    ) -> Result<
        (
            iroha_data_model::transaction::SignedTransaction,
            IsoMessageContext,
        ),
        MsgError,
    > {
        let debtor_iban = require_identifier(
            "DbtrAcct",
            IdentifierKind::Iban,
            parsed
                .field_text("DbtrAcct")
                .ok_or(MsgError::ValidationFailed)?,
        )?;
        let creditor_iban = require_identifier(
            "CdtrAcct",
            IdentifierKind::Iban,
            parsed
                .field_text("CdtrAcct")
                .ok_or(MsgError::ValidationFailed)?,
        )?;
        let currency = require_identifier(
            "IntrBkSttlmCcy",
            IdentifierKind::Currency,
            parsed
                .field_text("IntrBkSttlmCcy")
                .ok_or(MsgError::ValidationFailed)?,
        )?;
        let amount_raw = parsed
            .field_text("IntrBkSttlmAmt")
            .ok_or(MsgError::ValidationFailed)?;
        let instructing_agent_bic = parsed
            .field_text("InstgAgt")
            .ok_or(MsgError::ValidationFailed)?;
        self.require_bic("InstgAgt", instructing_agent_bic)?;
        let instructed_agent_bic = parsed
            .field_text("InstdAgt")
            .ok_or(MsgError::ValidationFailed)?;
        self.require_bic("InstdAgt", instructed_agent_bic)?;

        if let Some(purpose) = parsed.field_text("Purp") {
            if !purpose.trim().eq_ignore_ascii_case("SECU") {
                return Err(MsgError::InvalidValue {
                    field: "Purp".to_owned(),
                    kind: InvalidValueKind::Enum,
                });
            }
        }

        let mut context = IsoMessageContext {
            ledger_id: Some(chain_id.as_str().to_owned()),
            settlement_amount: Some(amount_raw.trim().to_owned()),
            settlement_currency: Some(currency.clone()),
            ..IsoMessageContext::default()
        };

        if let Some(ledger_hint) = parsed.field_text("SplmtryData/LedgerId") {
            if ledger_hint != chain_id.as_str() {
                return Err(MsgError::ValidationFailed);
            }
            context.ledger_id = Some(ledger_hint.to_owned());
        }

        let debtor = if let Some(hint) = parsed.field_text("SplmtryData/SourceAccountId") {
            let (account, canonical) =
                parse_iso_account_hint(hint, telemetry, ISO_PACS009_CONTEXT)?;
            context.source_account_id = Some(canonical);
            account
        } else {
            let account =
                self.resolve_account(&debtor_iban)
                    .ok_or_else(|| MsgError::InvalidIdentifier {
                        field: "DbtrAcct".to_owned(),
                        kind: IdentifierKind::Iban,
                    })?;
            context.source_account_id = Some(account.to_string());
            account
        };
        if let Some(address) = parsed.field_text("SplmtryData/SourceAccountAddress") {
            let trimmed = address.trim();
            if !trimmed.is_empty() {
                let (value, observation) = parse_account_address_literal(trimmed);
                if let Some(value) = value {
                    context.source_account_address = Some(value);
                }
                context.source_address_observation = observation;
            }
        }

        let creditor = if let Some(hint) = parsed.field_text("SplmtryData/TargetAccountId") {
            let (account, canonical) =
                parse_iso_account_hint(hint, telemetry, ISO_PACS009_CONTEXT)?;
            context.target_account_id = Some(canonical);
            account
        } else {
            let account = self.resolve_account(&creditor_iban).ok_or_else(|| {
                MsgError::InvalidIdentifier {
                    field: "CdtrAcct".to_owned(),
                    kind: IdentifierKind::Iban,
                }
            })?;
            context.target_account_id = Some(account.to_string());
            account
        };
        if let Some(address) = parsed.field_text("SplmtryData/TargetAccountAddress") {
            let trimmed = address.trim();
            if !trimmed.is_empty() {
                let (value, observation) = parse_account_address_literal(trimmed);
                if let Some(value) = value {
                    context.target_account_address = Some(value);
                }
                context.target_address_observation = observation;
            }
        }

        let asset_definition =
            if let Some(hint) = parsed.field_text("SplmtryData/AssetDefinitionId") {
                let definition = resolve_asset_definition_selector(world, hint, now_ms)
                    .ok_or(MsgError::ValidationFailed)?;
                context.asset_definition_id = Some(definition.to_string());
                definition
            } else {
                let definition = self
                    .resolve_asset(world, now_ms, &currency)
                    .ok_or_else(|| MsgError::InvalidIdentifier {
                        field: "IntrBkSttlmCcy".to_owned(),
                        kind: IdentifierKind::Currency,
                    })?;
                context.asset_definition_id = Some(definition.to_string());
                definition
            };
        let amount = Numeric::from_str(amount_raw).map_err(|_| MsgError::ValidationFailed)?;
        let asset = AssetId::new(asset_definition.clone(), debtor.clone());
        let asset_id_str = asset.to_string();
        context.asset_id = Some(asset_id_str);
        let transfer = Transfer::asset_numeric(asset, amount, creditor);

        let mut metadata = Metadata::default();
        for (key, value) in [
            ("iso20022_ledger_id", context.ledger_id.as_deref()),
            (
                "iso20022_source_account_id",
                context.source_account_id.as_deref(),
            ),
            (
                "iso20022_source_account_address",
                context.source_account_address.as_deref(),
            ),
            (
                "iso20022_target_account_id",
                context.target_account_id.as_deref(),
            ),
            (
                "iso20022_target_account_address",
                context.target_account_address.as_deref(),
            ),
            (
                "iso20022_asset_definition_id",
                context.asset_definition_id.as_deref(),
            ),
            ("iso20022_asset_id", context.asset_id.as_deref()),
            (
                "iso20022_settlement_amount",
                context.settlement_amount.as_deref(),
            ),
            (
                "iso20022_settlement_currency",
                context.settlement_currency.as_deref(),
            ),
            (
                "iso20022_business_message_id",
                parsed.field_text("BizMsgIdr"),
            ),
            ("iso20022_definition_id", parsed.field_text("MsgDefIdr")),
            ("iso20022_category_purpose", parsed.field_text("Purp")),
        ] {
            if let Some(value) = value {
                insert_metadata_value(&mut metadata, key, value)?;
            }
        }

        let mut builder = TransactionBuilder::new(chain_id.clone(), self.signer_account.clone())
            .with_instructions(core::iter::once(InstructionBox::from(transfer)));
        if metadata.iter().len() > 0 {
            builder = builder.with_metadata(metadata);
        }

        let transaction = builder.sign(&self.signer_private_key);
        Ok((transaction, context))
    }

    /// Access signer account identifier.
    pub fn signer_account(&self) -> &AccountId {
        &self.signer_account
    }
}

impl Iso20022BridgeRuntime {
    fn apply_lifecycle_update(
        &self,
        original_id: &str,
        message_type: &str,
        status_code: Option<&str>,
        reason_code: Option<&str>,
        detail: Option<&str>,
    ) -> &'static str {
        if message_type == "pacs.004" {
            self.mark_rejected(
                original_id,
                Some(
                    detail
                        .unwrap_or("payment returned by inbound pacs.004")
                        .to_owned(),
                ),
                reason_code.or(Some("PRTRY:PAYMENT_RETURN")),
            );
            return "marked_returned";
        }
        if message_type == "camt.056" {
            self.mark_hold(original_id, reason_code.or(Some("CANC")));
            self.add_change_reason_code(original_id, "CANCELLATION_REQUESTED");
            return "marked_cancellation_requested";
        }

        match status_code
            .map(str::trim)
            .filter(|code| !code.is_empty())
            .map(|code| code.to_ascii_uppercase())
            .as_deref()
        {
            Some("ACSC" | "ACCP" | "SETT" | "SETTLED") => {
                self.mark_settled(original_id, SystemTime::now());
                "marked_settled"
            }
            Some("RJCT" | "REJT" | "CANC" | "CAND") => {
                self.mark_rejected(
                    original_id,
                    Some(
                        detail
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| "ISO 20022 lifecycle rejection".to_owned()),
                    ),
                    reason_code.or(Some("RJCT")),
                );
                "marked_rejected"
            }
            Some("PDNG" | "PEND" | "PENF") => {
                self.mark_hold(original_id, reason_code.or(status_code));
                "marked_pending"
            }
            Some("PART") => {
                self.add_change_reason_code(original_id, "PARTIAL_SETTLEMENT");
                "marked_partial"
            }
            Some("ACSP" | "ACTC") => {
                self.mark_queued(original_id);
                "marked_processing"
            }
            Some(other) => {
                self.add_change_reason_code(original_id, other);
                "recorded_status_code"
            }
            None => {
                self.add_change_reason_code(original_id, message_type);
                "recorded_lifecycle_reference"
            }
        }
    }

    fn prune_expired(&self, now: Instant) {
        let ttl = self.dedupe_ttl;
        let expired = self
            .records
            .iter()
            .filter_map(|entry| {
                (now.saturating_duration_since(entry.last_seen) > ttl).then(|| entry.key().clone())
            })
            .collect::<Vec<_>>();
        for message_id in expired {
            self.remove_message(&message_id);
        }
    }

    fn metadata_conflicts(&self, message_id: &str, metadata: &IsoMessageMetadata) -> bool {
        metadata
            .payload_hash()
            .and_then(|hash| self.payload_hash_index.get(hash))
            .is_some_and(|existing| existing.as_str() != message_id)
            || metadata
                .uetr()
                .and_then(|uetr| self.uetr_index.get(&normalise_uetr(uetr)))
                .is_some_and(|existing| existing.as_str() != message_id)
    }

    fn insert_metadata_indexes(&self, message_id: &str, metadata: &IsoMessageMetadata) {
        if let Some(payload_hash) = metadata.payload_hash() {
            self.payload_hash_index
                .insert(payload_hash.to_owned(), message_id.to_owned());
        }
        if let Some(uetr) = metadata.uetr() {
            self.uetr_index
                .insert(normalise_uetr(uetr), message_id.to_owned());
        }
    }

    fn remove_record_indexes(&self, message_id: &str, record: &IsoMessageRecord) {
        if let Some(hash) = record.transaction_hash.as_deref() {
            self.tx_hash_index.remove(hash);
        }
        if let Some(payload_hash) = record.metadata.payload_hash() {
            self.payload_hash_index.remove(payload_hash);
        }
        if let Some(uetr) = record.metadata.uetr() {
            self.uetr_index.remove(&normalise_uetr(uetr));
        }
        self.payload_hash_index
            .retain(|_, existing_message| existing_message != message_id);
        self.uetr_index
            .retain(|_, existing_message| existing_message != message_id);
    }

    fn load_persisted_records(&self) {
        let Some(store_dir) = self.store_dir.as_deref() else {
            return;
        };
        let messages_dir = store_dir.join("messages");
        let Ok(entries) = fs::read_dir(&messages_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = norito::json::from_json::<JsonValue>(&text) else {
                continue;
            };
            if let Some((message_id, record)) = persisted_record_from_value(&value) {
                self.insert_metadata_indexes(&message_id, &record.metadata);
                if let Some(tx_hash) = record.transaction_hash.as_deref() {
                    self.tx_hash_index
                        .insert(tx_hash.to_owned(), message_id.clone());
                }
                self.records.insert(message_id, record);
            }
        }
    }

    fn persist_message(&self, message_id: &str) {
        let Some(store_dir) = self.store_dir.as_deref() else {
            return;
        };
        let Some(record) = self.records.get(message_id).map(|entry| entry.clone()) else {
            return;
        };
        let messages_dir = store_dir.join("messages");
        if fs::create_dir_all(&messages_dir).is_err() {
            return;
        }
        let payload = persisted_record_value(message_id, &record);
        let Ok(json) = norito::json::to_string_pretty(&payload) else {
            return;
        };
        let path = messages_dir.join(message_filename(message_id));
        let _ = fs::write(path, json);
    }

    fn remove_persisted_message(&self, message_id: &str) {
        let Some(store_dir) = self.store_dir.as_deref() else {
            return;
        };
        let path = store_dir
            .join("messages")
            .join(message_filename(message_id));
        let _ = fs::remove_file(path);
    }

    fn require_profile_reference_data(&self, profile: &TradfiRailProfile) -> Result<(), MsgError> {
        for requirement in &profile.required_reference_datasets {
            if !self.reference_data.has_required_dataset(*requirement) {
                return Err(MsgError::ValidationFailed);
            }
        }
        Ok(())
    }

    fn validate_amount_minor_units(
        &self,
        profile: &MessageProfile,
        parsed: &ParsedMessage,
    ) -> Result<(), MsgError> {
        let Some(currency) = parsed.field_text("IntrBkSttlmCcy") else {
            return Ok(());
        };
        let Some(amount) = parsed.field_text("IntrBkSttlmAmt") else {
            return Ok(());
        };
        let currency = normalise_currency(currency);
        let minor_units = profile.minor_units_for(&currency);
        if amount_fraction_digits(amount) > usize::from(minor_units) {
            return Err(MsgError::InvalidValue {
                field: "IntrBkSttlmAmt".to_owned(),
                kind: InvalidValueKind::Amount,
            });
        }
        Ok(())
    }

    fn validate_supplementary_data_limit(
        &self,
        profile: &MessageProfile,
        parsed: &ParsedMessage,
    ) -> Result<(), MsgError> {
        let total = parsed
            .iter()
            .filter(|(field, _)| field.contains("SplmtryData"))
            .map(|(field, value)| field.len() + value.len())
            .sum::<usize>();
        if total > profile.supplementary_data_max_bytes {
            return Err(MsgError::TooManyOccurrences {
                field: "SplmtryData",
                max: profile.supplementary_data_max_bytes,
                actual: total,
            });
        }
        Ok(())
    }

    fn validate_structured_address_mode(
        &self,
        profile: &MessageProfile,
        parsed: &ParsedMessage,
    ) -> Result<(), MsgError> {
        if profile.structured_address_mode == StructuredAddressMode::Permissive {
            return Ok(());
        }
        let has_unstructured_address = parsed
            .iter()
            .any(|(field, _)| field.ends_with("/PstlAdr/AdrLine") || field.contains("/AdrLine["));
        if has_unstructured_address {
            return Err(MsgError::InvalidValue {
                field: "PstlAdr/AdrLine".to_owned(),
                kind: InvalidValueKind::Enum,
            });
        }
        Ok(())
    }

    fn require_bic(&self, field: &str, value: &str) -> Result<(), MsgError> {
        let bic = require_identifier(field, IdentifierKind::Bic, value)?;
        match self.reference_data.validate_bic(&bic) {
            Ok(ValidationOutcome::Enforced | ValidationOutcome::Skipped) => Ok(()),
            Err(err) => Err(Self::map_reference_error(field, IdentifierKind::Bic, err)),
        }
    }

    fn map_reference_error(field: &str, kind: IdentifierKind, err: ReferenceDataError) -> MsgError {
        match err {
            ReferenceDataError::DatasetFailed { .. } => MsgError::ValidationFailed,
            ReferenceDataError::NotFound { .. } => MsgError::InvalidIdentifier {
                field: field.to_owned(),
                kind,
            },
            ReferenceDataError::MicInactive { .. } => MsgError::InvalidIdentifier {
                field: field.to_owned(),
                kind: IdentifierKind::Mic,
            },
        }
    }
}

fn persisted_record_value(message_id: &str, record: &IsoMessageRecord) -> JsonValue {
    let mut root = norito::json::Map::new();
    root.insert("message_id".to_owned(), JsonValue::from(message_id));
    root.insert("state".to_owned(), JsonValue::from(record.state.label()));
    root.insert(
        "updated_at_ms".to_owned(),
        JsonValue::from(system_time_to_ms(record.updated_at)),
    );
    root.insert(
        "transaction_hash".to_owned(),
        string_or_null(record.transaction_hash.as_deref()),
    );
    root.insert(
        "detail".to_owned(),
        string_or_null(record.detail.as_deref()),
    );
    root.insert(
        "ledger_tx_queued".to_owned(),
        JsonValue::from(record.ledger_tx_queued),
    );
    root.insert(
        "settled_at_ms".to_owned(),
        record
            .settled_at
            .map(system_time_to_ms)
            .map_or(JsonValue::Null, JsonValue::from),
    );
    root.insert(
        "hold_reason_code".to_owned(),
        string_or_null(record.hold_reason_code.as_deref()),
    );
    root.insert(
        "change_reason_codes".to_owned(),
        JsonValue::Array(
            record
                .change_reason_codes
                .iter()
                .map(|code| JsonValue::from(code.as_str()))
                .collect(),
        ),
    );
    root.insert(
        "rejection_reason_code".to_owned(),
        string_or_null(record.rejection_reason_code.as_deref()),
    );
    root.insert("context".to_owned(), context_value(&record.context));
    root.insert("metadata".to_owned(), metadata_value(&record.metadata));
    root.insert(
        "status_history".to_owned(),
        JsonValue::Array(
            record
                .status_history
                .iter()
                .map(history_value)
                .collect::<Vec<_>>(),
        ),
    );
    JsonValue::Object(root)
}

fn persisted_record_from_value(value: &JsonValue) -> Option<(String, IsoMessageRecord)> {
    let obj = value.as_object()?;
    let message_id = obj.get("message_id")?.as_str()?.to_owned();
    let state = state_from_label(obj.get("state")?.as_str()?)?;
    let updated_at = obj
        .get("updated_at_ms")
        .and_then(JsonValue::as_u64)
        .map(system_time_from_ms)
        .unwrap_or_else(SystemTime::now);
    let transaction_hash = obj
        .get("transaction_hash")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned);
    let detail = obj
        .get("detail")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned);
    let ledger_tx_queued = obj
        .get("ledger_tx_queued")
        .and_then(JsonValue::as_bool)
        .unwrap_or(false);
    let settled_at = obj
        .get("settled_at_ms")
        .and_then(JsonValue::as_u64)
        .map(system_time_from_ms);
    let hold_reason_code = obj
        .get("hold_reason_code")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned);
    let change_reason_codes = obj
        .get("change_reason_codes")
        .and_then(JsonValue::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(JsonValue::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let rejection_reason_code = obj
        .get("rejection_reason_code")
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned);
    let context = obj
        .get("context")
        .and_then(context_from_value)
        .unwrap_or_default();
    let metadata = obj
        .get("metadata")
        .and_then(metadata_from_value)
        .unwrap_or_default();
    let mut record = IsoMessageRecord {
        last_seen: Instant::now(),
        updated_at,
        state,
        transaction_hash,
        detail,
        context,
        metadata,
        ledger_tx_queued,
        settled_at,
        hold_reason_code,
        change_reason_codes,
        rejection_reason_code,
        status_history: obj
            .get("status_history")
            .and_then(JsonValue::as_array)
            .map(|items| items.iter().filter_map(history_from_value).collect())
            .unwrap_or_default(),
    };
    if record.status_history.is_empty() {
        record.push_history();
    }
    Some((message_id, record))
}

fn context_value(context: &IsoMessageContext) -> JsonValue {
    let mut map = norito::json::Map::new();
    map.insert(
        "ledger_id".to_owned(),
        string_or_null(context.ledger_id.as_deref()),
    );
    map.insert(
        "source_account_id".to_owned(),
        string_or_null(context.source_account_id.as_deref()),
    );
    map.insert(
        "source_account_address".to_owned(),
        string_or_null(context.source_account_address.as_deref()),
    );
    map.insert(
        "target_account_id".to_owned(),
        string_or_null(context.target_account_id.as_deref()),
    );
    map.insert(
        "target_account_address".to_owned(),
        string_or_null(context.target_account_address.as_deref()),
    );
    map.insert(
        "asset_definition_id".to_owned(),
        string_or_null(context.asset_definition_id.as_deref()),
    );
    map.insert(
        "asset_id".to_owned(),
        string_or_null(context.asset_id.as_deref()),
    );
    map.insert(
        "settlement_amount".to_owned(),
        string_or_null(context.settlement_amount.as_deref()),
    );
    map.insert(
        "settlement_currency".to_owned(),
        string_or_null(context.settlement_currency.as_deref()),
    );
    map.insert(
        "settlement_date".to_owned(),
        string_or_null(context.settlement_date.as_deref()),
    );
    map.insert(
        "settlement_quantity".to_owned(),
        string_or_null(context.settlement_quantity.as_deref()),
    );
    map.insert(
        "settlement_movement_type".to_owned(),
        string_or_null(context.settlement_movement_type.as_deref()),
    );
    map.insert(
        "settlement_payment_type".to_owned(),
        string_or_null(context.settlement_payment_type.as_deref()),
    );
    map.insert(
        "security_instrument_id".to_owned(),
        string_or_null(context.security_instrument_id.as_deref()),
    );
    map.insert(
        "plan_execution_order".to_owned(),
        string_or_null(context.plan_execution_order.as_deref()),
    );
    map.insert(
        "plan_atomicity".to_owned(),
        string_or_null(context.plan_atomicity.as_deref()),
    );
    JsonValue::Object(map)
}

fn context_from_value(value: &JsonValue) -> Option<IsoMessageContext> {
    let obj = value.as_object()?;
    Some(IsoMessageContext {
        ledger_id: optional_string(obj, "ledger_id"),
        source_account_id: optional_string(obj, "source_account_id"),
        source_account_address: optional_string(obj, "source_account_address"),
        target_account_id: optional_string(obj, "target_account_id"),
        target_account_address: optional_string(obj, "target_account_address"),
        asset_definition_id: optional_string(obj, "asset_definition_id"),
        asset_id: optional_string(obj, "asset_id"),
        settlement_amount: optional_string(obj, "settlement_amount"),
        settlement_currency: optional_string(obj, "settlement_currency"),
        settlement_date: optional_string(obj, "settlement_date"),
        settlement_quantity: optional_string(obj, "settlement_quantity"),
        settlement_movement_type: optional_string(obj, "settlement_movement_type"),
        settlement_payment_type: optional_string(obj, "settlement_payment_type"),
        security_instrument_id: optional_string(obj, "security_instrument_id"),
        plan_execution_order: optional_string(obj, "plan_execution_order"),
        plan_atomicity: optional_string(obj, "plan_atomicity"),
        source_address_observation: AddressParseObservation::default(),
        target_address_observation: AddressParseObservation::default(),
    })
}

fn metadata_value(metadata: &IsoMessageMetadata) -> JsonValue {
    let mut map = norito::json::Map::new();
    map.insert(
        "profile_id".to_owned(),
        string_or_null(metadata.profile_id.as_deref()),
    );
    map.insert(
        "message_type".to_owned(),
        string_or_null(metadata.message_type.as_deref()),
    );
    map.insert(
        "business_service".to_owned(),
        string_or_null(metadata.business_service.as_deref()),
    );
    map.insert(
        "business_message_id".to_owned(),
        string_or_null(metadata.business_message_id.as_deref()),
    );
    map.insert("uetr".to_owned(), string_or_null(metadata.uetr.as_deref()));
    map.insert(
        "payload_hash".to_owned(),
        string_or_null(metadata.payload_hash.as_deref()),
    );
    map.insert(
        "reference_snapshot_id".to_owned(),
        string_or_null(metadata.reference_snapshot_id.as_deref()),
    );
    map.insert(
        "embedded_signature_detected".to_owned(),
        JsonValue::from(metadata.embedded_signature_detected),
    );
    JsonValue::Object(map)
}

fn metadata_from_value(value: &JsonValue) -> Option<IsoMessageMetadata> {
    let obj = value.as_object()?;
    Some(IsoMessageMetadata {
        profile_id: optional_string(obj, "profile_id"),
        message_type: optional_string(obj, "message_type"),
        business_service: optional_string(obj, "business_service"),
        business_message_id: optional_string(obj, "business_message_id"),
        uetr: optional_string(obj, "uetr"),
        payload_hash: optional_string(obj, "payload_hash"),
        reference_snapshot_id: optional_string(obj, "reference_snapshot_id"),
        embedded_signature_detected: obj
            .get("embedded_signature_detected")
            .and_then(JsonValue::as_bool)
            .unwrap_or(false),
    })
}

fn history_value(entry: &IsoStatusHistoryEntry) -> JsonValue {
    let mut map = norito::json::Map::new();
    map.insert("status".to_owned(), JsonValue::from(entry.status_label()));
    map.insert(
        "pacs002_code".to_owned(),
        JsonValue::from(entry.pacs002_code()),
    );
    map.insert(
        "updated_at_ms".to_owned(),
        JsonValue::from(system_time_to_ms(entry.updated_at())),
    );
    map.insert("detail".to_owned(), string_or_null(entry.detail()));
    map.insert(
        "reason_code".to_owned(),
        string_or_null(entry.reason_code()),
    );
    JsonValue::Object(map)
}

fn history_from_value(value: &JsonValue) -> Option<IsoStatusHistoryEntry> {
    let obj = value.as_object()?;
    Some(IsoStatusHistoryEntry {
        status: state_from_label(obj.get("status")?.as_str()?)?,
        pacs002_code: pacs002_from_code(obj.get("pacs002_code")?.as_str()?)?,
        updated_at: obj
            .get("updated_at_ms")
            .and_then(JsonValue::as_u64)
            .map(system_time_from_ms)
            .unwrap_or_else(SystemTime::now),
        detail: optional_string(obj, "detail"),
        reason_code: optional_string(obj, "reason_code"),
    })
}

fn optional_string(obj: &norito::json::Map, key: &str) -> Option<String> {
    obj.get(key)
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
}

fn string_or_null(value: Option<&str>) -> JsonValue {
    value.map_or(JsonValue::Null, JsonValue::from)
}

fn message_filename(message_id: &str) -> String {
    format!("{}.json", sha256_hex(message_id.as_bytes()))
}

fn system_time_to_ms(time: SystemTime) -> u64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn system_time_from_ms(ms: u64) -> SystemTime {
    std::time::UNIX_EPOCH + Duration::from_millis(ms)
}

fn state_from_label(value: &str) -> Option<IsoMessageState> {
    match value {
        "Pending" => Some(IsoMessageState::Pending),
        "Accepted" => Some(IsoMessageState::Accepted),
        "Rejected" => Some(IsoMessageState::Rejected),
        _ => None,
    }
}

fn pacs002_from_code(value: &str) -> Option<Pacs002Status> {
    match value {
        "ACTC" => Some(Pacs002Status::Actc),
        "ACSP" => Some(Pacs002Status::Acsp),
        "ACSC" => Some(Pacs002Status::Acsc),
        "ACWC" => Some(Pacs002Status::Acwc),
        "PDNG" => Some(Pacs002Status::Pdng),
        "RJCT" => Some(Pacs002Status::Rjct),
        _ => None,
    }
}

fn message_definition_id<'a>(parsed: &'a ParsedMessage, message_type: &'a str) -> Option<&'a str> {
    field_text_by_suffix(parsed, &["AppHdr/MsgDefIdr", "MsgDefIdr"]).or(Some(message_type))
}

fn app_header_business_message_id(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(parsed, &["AppHdr/BizMsgIdr", "BizMsgIdr"])
}

fn app_header_message_definition_id(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(parsed, &["AppHdr/MsgDefIdr", "MsgDefIdr"])
}

fn app_header_creation_date(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(
        parsed,
        &["AppHdr/CreDt", "CreDt", "AppHdr/CreDtTm", "CreDtTm"],
    )
}

fn business_service(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(parsed, &["AppHdr/BizSvc", "BizSvc"])
}

fn business_message_id(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(parsed, &["AppHdr/BizMsgIdr", "BizMsgIdr"])
        .or_else(|| parsed.field_text("MsgId"))
}

fn uetr(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(parsed, &["UETR"]).filter(|value| !value.trim().is_empty())
}

fn lifecycle_referenced_message_id<'a>(
    message_type: &str,
    parsed: &'a ParsedMessage,
) -> Option<&'a str> {
    match message_type {
        "pacs.002" => parsed.field_text("OrgnlMsgId"),
        "pacs.004" => parsed.field_text("OrgnlGrpInf/OrgnlMsgId"),
        "camt.056" => parsed.field_text("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId"),
        "sese.024" | "sese.025" => parsed.field_text("TxId"),
        "sese.023" => None,
        _ => None,
    }
    .filter(|value| !value.trim().is_empty())
}

fn lifecycle_status_code<'a>(message_type: &str, parsed: &'a ParsedMessage) -> Option<&'a str> {
    match message_type {
        "pacs.002" => parsed.field_text("TxSts"),
        "pacs.004" => Some("RJCT"),
        "camt.056" => Some("PDNG"),
        "sese.024" => field_text_by_suffix(
            parsed,
            &[
                "SttlmSts",
                "TxSts",
                "SttlmTxSts/Sts/Cd",
                "SttlmTxSts/Sts/Prtry",
            ],
        ),
        "sese.025" => parsed.field_text("ConfSts"),
        "sese.023" => Some("ACTC"),
        _ => None,
    }
    .filter(|value| !value.trim().is_empty())
}

fn lifecycle_reason_code(parsed: &ParsedMessage) -> Option<&str> {
    field_text_by_suffix(
        parsed,
        &[
            "RsnCd",
            "RtrdRsn/Cd",
            "RtrdRsn/Prtry",
            "CxlRsnInf/Rsn/Cd",
            "CxlRsnInf/Rsn/Prtry",
            "StsRsnInf/Rsn/Cd",
            "StsRsnInf/Rsn/Prtry",
        ],
    )
    .filter(|value| !value.trim().is_empty())
}

fn lifecycle_detail(
    message_type: &str,
    parsed: &ParsedMessage,
    status_code: Option<&str>,
) -> Option<String> {
    field_text_by_suffix(parsed, &["AddtlInf", "AddtlInf[*]", "CxlRsnInf/AddtlInf"])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| match (message_type, status_code) {
            ("pacs.004", _) => Some("payment returned by inbound pacs.004".to_owned()),
            (_, Some(code)) => Some(format!("ISO 20022 lifecycle status {code}")),
            _ => None,
        })
}

fn lifecycle_context(message_type: &str, parsed: &ParsedMessage) -> Option<IsoMessageContext> {
    let mut context = IsoMessageContext::default();
    match message_type {
        "sese.023" => {
            context.settlement_date = parsed_text(parsed, "SttlmDt");
            context.settlement_movement_type =
                parsed_text(parsed, "SttlmTpAndAddtlParams/SctiesMvmntTp");
            context.settlement_payment_type = parsed_text(parsed, "SttlmTpAndAddtlParams/Pmt");
            context.settlement_quantity = parsed_text(parsed, "SctiesLeg/Qty");
            context.security_instrument_id = parsed_text(parsed, "SctiesLeg/FinInstrmId");
            context.settlement_amount = parsed_text(parsed, "CashLeg/Amt");
            context.settlement_currency = parsed_text(parsed, "CashLeg/Ccy");
            context.plan_execution_order = parsed_text(parsed, "Plan/ExecutionOrder");
            context.plan_atomicity = parsed_text(parsed, "Plan/Atomicity");
        }
        "sese.025" => {
            context.settlement_date = parsed_text(parsed, "SttlmDt");
            context.settlement_movement_type =
                parsed_text(parsed, "SttlmTpAndAddtlParams/SctiesMvmntTp");
            context.settlement_payment_type = parsed_text(parsed, "SttlmTpAndAddtlParams/Pmt");
            context.settlement_quantity = parsed_text(parsed, "SttlmQty");
            context.security_instrument_id = parsed_text(parsed, "SctiesLeg/FinInstrmId");
            context.settlement_amount = parsed_text(parsed, "SttlmAmt");
            context.settlement_currency = parsed_text(parsed, "SttlmCcy");
            context.plan_execution_order = parsed_text(parsed, "Plan/ExecutionOrder");
            context.plan_atomicity = parsed_text(parsed, "Plan/Atomicity");
        }
        _ => return None,
    }

    [
        context.settlement_date.as_deref(),
        context.settlement_movement_type.as_deref(),
        context.settlement_payment_type.as_deref(),
        context.settlement_quantity.as_deref(),
        context.security_instrument_id.as_deref(),
        context.settlement_amount.as_deref(),
        context.settlement_currency.as_deref(),
        context.plan_execution_order.as_deref(),
        context.plan_atomicity.as_deref(),
    ]
    .iter()
    .any(|value| value.is_some())
    .then_some(context)
}

fn parsed_text(parsed: &ParsedMessage, field: &str) -> Option<String> {
    parsed
        .field_text(field)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn field_text_by_suffix<'a>(parsed: &'a ParsedMessage, suffixes: &[&str]) -> Option<&'a str> {
    suffixes.iter().find_map(|suffix| {
        parsed.field_text(suffix).or_else(|| {
            parsed.iter().find_map(|(field, value)| {
                field_matches_suffix(field, suffix)
                    .then(|| core::str::from_utf8(value).ok())
                    .flatten()
            })
        })
    })
}

fn field_matches_suffix(field: &str, suffix: &str) -> bool {
    field == suffix
        || field
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('/'))
}

fn has_embedded_signature_marker(parsed: &ParsedMessage) -> bool {
    parsed.iter().any(|(field, _)| {
        field.contains("Sgntr")
            || field.contains("Signature")
            || field.ends_with("/@ignored")
            || field.contains("SignedInfo")
            || field.contains("QualifyingProperties")
    })
}

#[derive(Debug, Clone, Copy)]
struct XmlElementSpan {
    start: usize,
    opening_end: usize,
    content_start: usize,
    content_end: usize,
    end: usize,
}

const XMLDSIG_ECDSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256";
const XMLDSIG_SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const XMLDSIG_ENVELOPED_SIGNATURE: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
const XML_C14N_1_0: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const XML_C14N_1_1: &str = "http://www.w3.org/2006/12/xml-c14n11";
const XML_EXCLUSIVE_C14N_1_0: &str = "http://www.w3.org/2001/10/xml-exc-c14n#";

fn payload_has_embedded_signature(payload: &[u8]) -> bool {
    std::str::from_utf8(payload).is_ok_and(|text| {
        find_first_xml_element(text, "Signature").is_some()
            || find_first_xml_element(text, "Sgntr").is_some()
    })
}

fn verify_embedded_xml_signature(
    payload: &[u8],
    profile: &TradfiRailProfile,
) -> Result<bool, MsgError> {
    let text = std::str::from_utf8(payload).map_err(|_| MsgError::InvalidFormat)?;
    let Some(signature_span) =
        find_first_xml_element(text, "Signature").or_else(|| find_first_xml_element(text, "Sgntr"))
    else {
        return Ok(false);
    };
    if find_first_xml_element(&text[signature_span.end..], "Signature").is_some() {
        return Err(MsgError::ValidationFailed);
    }
    let signature_xml = &text[signature_span.start..signature_span.end];
    let signed_info_span =
        find_first_xml_element(signature_xml, "SignedInfo").ok_or(MsgError::ValidationFailed)?;
    let signed_info_xml = &signature_xml[signed_info_span.start..signed_info_span.end];
    let c14n_algorithm = child_attr(signed_info_xml, "CanonicalizationMethod", "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if !matches!(
        c14n_algorithm.as_str(),
        XML_C14N_1_0 | XML_C14N_1_1 | XML_EXCLUSIVE_C14N_1_0
    ) {
        return Err(MsgError::ValidationFailed);
    }
    let signature_algorithm = child_attr(signed_info_xml, "SignatureMethod", "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if signature_algorithm != XMLDSIG_ECDSA_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    verify_xml_signature_references(text, signature_span, signed_info_xml)?;

    let signature_value = decode_required_child_base64(signature_xml, "SignatureValue")?;
    let key_material = xml_signature_key_material(signature_xml)?;
    if !profile.has_xml_signature_trust_anchors()
        || !profile.accepts_xml_signature_key(
            &sha256_hex(&key_material.public_key),
            &key_material.certificate_sha256,
        )
    {
        return Err(MsgError::ValidationFailed);
    }
    let verifying_key = P256VerifyingKey::from_sec1_bytes(&key_material.public_key)
        .map_err(|_| MsgError::ValidationFailed)?;
    let signature =
        P256Signature::from_der(&signature_value).map_err(|_| MsgError::ValidationFailed)?;
    verifying_key
        .verify(signed_info_xml.as_bytes(), &signature)
        .map_err(|_| MsgError::ValidationFailed)?;
    Ok(true)
}

fn verify_xml_signature_references(
    full_xml: &str,
    signature_span: XmlElementSpan,
    signed_info_xml: &str,
) -> Result<(), MsgError> {
    let reference_span =
        find_first_xml_element(signed_info_xml, "Reference").ok_or(MsgError::ValidationFailed)?;
    if find_first_xml_element(&signed_info_xml[reference_span.end..], "Reference").is_some() {
        return Err(MsgError::ValidationFailed);
    }
    let reference_xml = &signed_info_xml[reference_span.start..reference_span.end];
    let uri = element_attr(signed_info_xml, reference_span, "URI").unwrap_or_default();
    if !uri.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    if !contains_child_attr(
        reference_xml,
        "Transform",
        "Algorithm",
        XMLDSIG_ENVELOPED_SIGNATURE,
    ) {
        return Err(MsgError::ValidationFailed);
    }
    let digest_algorithm =
        child_attr(reference_xml, "DigestMethod", "Algorithm").ok_or(MsgError::ValidationFailed)?;
    if digest_algorithm != XMLDSIG_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    let expected_digest = decode_required_child_base64(reference_xml, "DigestValue")?;
    let mut unsigned = String::with_capacity(full_xml.len());
    unsigned.push_str(&full_xml[..signature_span.start]);
    unsigned.push_str(&full_xml[signature_span.end..]);
    let digest = Sha256::digest(unsigned.trim().as_bytes());
    if expected_digest.as_slice() != &digest[..] {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

struct XmlSignatureKeyMaterial {
    public_key: Vec<u8>,
    certificate_sha256: Vec<String>,
}

fn xml_signature_key_material(signature_xml: &str) -> Result<XmlSignatureKeyMaterial, MsgError> {
    if let Some(public_key) = child_text_compact(signature_xml, "PublicKey") {
        let public_key = BASE64_STANDARD
            .decode(public_key)
            .map_err(|_| MsgError::ValidationFailed)?;
        return Ok(XmlSignatureKeyMaterial {
            public_key,
            certificate_sha256: Vec::new(),
        });
    }
    let certificates = decode_child_base64_values(signature_xml, "X509Certificate")?;
    if certificates.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    let parsed_certificates = certificates
        .iter()
        .map(|certificate| {
            X509Certificate::from_der(certificate)
                .map(|(_, cert)| cert)
                .map_err(|_| MsgError::ValidationFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let public_key = parsed_certificates[0]
        .public_key()
        .subject_public_key
        .data
        .to_vec();
    let mut certificate_sha256 = vec![sha256_hex(&certificates[0])];
    for (certificates_der, chain_pair) in
        certificates[1..].iter().zip(parsed_certificates.windows(2))
    {
        chain_pair[0]
            .verify_signature(Some(chain_pair[1].public_key()))
            .map_err(|_| MsgError::ValidationFailed)?;
        certificate_sha256.push(sha256_hex(certificates_der));
    }
    Ok(XmlSignatureKeyMaterial {
        public_key,
        certificate_sha256,
    })
}

fn decode_required_child_base64(container: &str, child: &str) -> Result<Vec<u8>, MsgError> {
    let value = child_text_compact(container, child).ok_or(MsgError::ValidationFailed)?;
    BASE64_STANDARD
        .decode(value)
        .map_err(|_| MsgError::ValidationFailed)
}

fn decode_child_base64_values(container: &str, child: &str) -> Result<Vec<Vec<u8>>, MsgError> {
    let mut values = Vec::new();
    let mut cursor = 0usize;
    while cursor < container.len() {
        let Some(span) = find_first_xml_element(&container[cursor..], child) else {
            break;
        };
        let absolute = XmlElementSpan {
            start: cursor + span.start,
            opening_end: cursor + span.opening_end,
            content_start: cursor + span.content_start,
            content_end: cursor + span.content_end,
            end: cursor + span.end,
        };
        let value: String = container[absolute.content_start..absolute.content_end]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect();
        values.push(
            BASE64_STANDARD
                .decode(value)
                .map_err(|_| MsgError::ValidationFailed)?,
        );
        cursor = absolute.end;
    }
    Ok(values)
}

fn child_attr(container: &str, child: &str, attr: &str) -> Option<String> {
    let span = find_first_xml_element(container, child)?;
    element_attr(container, span, attr)
}

fn contains_child_attr(container: &str, child: &str, attr: &str, expected: &str) -> bool {
    let mut cursor = 0usize;
    while cursor < container.len() {
        let Some(span) = find_first_xml_element(&container[cursor..], child) else {
            return false;
        };
        let absolute = XmlElementSpan {
            start: cursor + span.start,
            opening_end: cursor + span.opening_end,
            content_start: cursor + span.content_start,
            content_end: cursor + span.content_end,
            end: cursor + span.end,
        };
        if element_attr(container, absolute, attr).as_deref() == Some(expected) {
            return true;
        }
        cursor = absolute.end;
    }
    false
}

fn child_text_compact(container: &str, child: &str) -> Option<String> {
    let span = find_first_xml_element(container, child)?;
    Some(
        container[span.content_start..span.content_end]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect(),
    )
}

fn element_attr(container: &str, span: XmlElementSpan, attr: &str) -> Option<String> {
    let opening = &container[span.start + 1..span.opening_end];
    attr_value(opening, attr)
}

fn attr_value(opening: &str, attr: &str) -> Option<String> {
    let mut cursor = opening.trim();
    if let Some((_, rest)) = cursor.split_once(char::is_whitespace) {
        cursor = rest.trim();
    } else {
        return None;
    }
    if cursor.ends_with('/') {
        cursor = cursor.trim_end_matches('/').trim_end();
    }
    while !cursor.is_empty() {
        let eq_idx = cursor.find('=')?;
        let name = cursor[..eq_idx].trim();
        let mut remainder = cursor[eq_idx + 1..].trim_start();
        if remainder.is_empty() {
            return None;
        }
        let quote = remainder.as_bytes()[0];
        if quote != b'"' && quote != b'\'' {
            return None;
        }
        remainder = &remainder[1..];
        let end_idx = remainder.find(quote as char)?;
        if name == attr || xml_local_name(name) == attr {
            return Some(remainder[..end_idx].to_owned());
        }
        remainder = remainder[end_idx + 1..].trim_start();
        cursor = remainder;
    }
    None
}

fn find_first_xml_element(text: &str, local: &str) -> Option<XmlElementSpan> {
    let mut cursor = 0usize;
    while cursor < text.len() {
        let start = cursor + text[cursor..].find('<')?;
        let tag_start = start + 1;
        let opening_end = find_xml_tag_end(text.as_bytes(), tag_start)?;
        let raw_tag = text[tag_start..opening_end].trim();
        if raw_tag.starts_with('/')
            || raw_tag.starts_with('?')
            || raw_tag.starts_with("!--")
            || raw_tag.starts_with("![CDATA[")
        {
            cursor = opening_end + 1;
            continue;
        }
        let self_closing = raw_tag.ends_with('/');
        let tag_body = raw_tag.trim_end_matches('/').trim_end();
        let (name, _) = tag_body
            .split_once(char::is_whitespace)
            .unwrap_or((tag_body, ""));
        if xml_local_name(name) == local {
            if self_closing {
                return Some(XmlElementSpan {
                    start,
                    opening_end,
                    content_start: opening_end + 1,
                    content_end: opening_end + 1,
                    end: opening_end + 1,
                });
            }
            let (content_end, end) = find_xml_element_end(text, opening_end + 1, local)?;
            return Some(XmlElementSpan {
                start,
                opening_end,
                content_start: opening_end + 1,
                content_end,
                end,
            });
        }
        cursor = opening_end + 1;
    }
    None
}

fn find_xml_element_end(text: &str, mut cursor: usize, local: &str) -> Option<(usize, usize)> {
    let mut depth = 1usize;
    while cursor < text.len() {
        let start = cursor + text[cursor..].find('<')?;
        let tag_start = start + 1;
        let tag_end = find_xml_tag_end(text.as_bytes(), tag_start)?;
        let raw_tag = text[tag_start..tag_end].trim();
        if raw_tag.starts_with('?') || raw_tag.starts_with("!--") || raw_tag.starts_with("![CDATA[")
        {
            cursor = tag_end + 1;
            continue;
        }
        let closing = raw_tag.starts_with('/');
        let tag_body = if closing {
            raw_tag.trim_start_matches('/').trim()
        } else {
            raw_tag
        };
        let self_closing = !closing && tag_body.ends_with('/');
        let tag_body = tag_body.trim_end_matches('/').trim_end();
        let (name, _) = tag_body
            .split_once(char::is_whitespace)
            .unwrap_or((tag_body, ""));
        if xml_local_name(name) == local {
            if closing {
                depth -= 1;
                if depth == 0 {
                    return Some((start, tag_end + 1));
                }
            } else if !self_closing {
                depth += 1;
            }
        }
        cursor = tag_end + 1;
    }
    None
}

fn find_xml_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut quote = None;
    while i < bytes.len() {
        match quote {
            Some(q) if bytes[i] == q => quote = None,
            None if bytes[i] == b'"' || bytes[i] == b'\'' => quote = Some(bytes[i]),
            None if bytes[i] == b'>' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn xml_local_name(name: &str) -> &str {
    name.rsplit_once(':').map_or(name, |(_, local)| local)
}

fn amount_fraction_digits(amount: &str) -> usize {
    amount
        .trim()
        .split_once('.')
        .map(|(_, fraction)| fraction.len())
        .unwrap_or(0)
}

fn normalise_uetr(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn normalise_identifier(kind: IdentifierKind, value: &str) -> String {
    match kind {
        IdentifierKind::Iban => normalise_iban(value),
        IdentifierKind::Currency => normalise_currency(value),
        IdentifierKind::Isin
        | IdentifierKind::Cusip
        | IdentifierKind::Lei
        | IdentifierKind::Bic
        | IdentifierKind::Mic => value.trim().to_ascii_uppercase(),
    }
}

fn require_identifier(field: &str, kind: IdentifierKind, value: &str) -> Result<String, MsgError> {
    let normalised = normalise_identifier(kind, value);
    if normalised.is_empty() || !ivm::iso20022::validate_identifier(kind, &normalised) {
        return Err(MsgError::InvalidIdentifier {
            field: field.to_owned(),
            kind,
        });
    }
    Ok(normalised)
}

fn normalise_iban(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

fn normalise_currency(input: &str) -> String {
    input.trim().to_ascii_uppercase()
}

fn validate_asset_definition_selector(input: &str) -> eyre::Result<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        eyre::bail!("asset definition selector must not be empty");
    }
    if AssetDefinitionId::parse_address_literal(trimmed).is_ok()
        || AssetDefinitionAlias::from_str(trimmed).is_ok()
    {
        return Ok(trimmed.to_owned());
    }
    eyre::bail!(
        "invalid asset definition selector `{trimmed}`; expected canonical Base58 asset definition id or on-chain asset alias literal"
    );
}

fn resolve_asset_definition_selector(
    world: &impl WorldReadOnly,
    selector: &str,
    now_ms: u64,
) -> Option<AssetDefinitionId> {
    let literal = selector.trim();
    if literal.is_empty() {
        return None;
    }

    AssetDefinitionId::parse_address_literal(literal)
        .ok()
        .or_else(|| {
            AssetDefinitionAlias::from_str(literal)
                .ok()
                .and_then(|alias| world.asset_definition_id_by_alias_at(&alias, now_ms))
        })
}

fn insert_metadata_value(metadata: &mut Metadata, key: &str, value: &str) -> Result<(), MsgError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    let name = Name::from_str(key).map_err(|_| MsgError::ValidationFailed)?;
    let json = Json::try_new(JsonValue::String(trimmed.to_owned()))
        .map_err(|_| MsgError::ValidationFailed)?;
    metadata.insert(name, json);
    Ok(())
}

fn dedup_codes(codes: &mut Vec<String>) {
    let mut seen = HashSet::new();
    codes.retain(|code| seen.insert(code.clone()));
}

#[cfg(test)]
mod tests {
    use std::{io::Write as _, str::FromStr, time::SystemTime};

    use iroha_core::iso_bridge::reference_data::SnapshotState;
    use iroha_core::state::World;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        Registrable, ValidationFail,
        account::Account,
        asset::{AssetDefinition, AssetDefinitionAlias},
        domain::Domain,
        nexus::{AxtRejectContext, AxtRejectReason, DataSpaceId, LaneId},
        transaction::error::{TransactionLimitError, TransactionRejectionReason},
    };
    use p256::ecdsa::{SigningKey as P256SigningKey, signature::Signer as _};
    use rcgen::{
        BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair as RcgenKeyPair,
        KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, SigningKey as _,
    };
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    const LEGACY_PUBLIC_KEY_LITERAL: &str =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@test";

    fn assert_outbox_error_contains(err: crate::Error, expected: &str) {
        match err {
            crate::Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => assert!(
                message.contains(expected),
                "expected `{message}` to contain `{expected}`"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn sample_account_bundle() -> (AccountId, String, iroha_crypto::PrivateKey) {
        let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
        let (public_key, private_key) = key_pair.into_parts();
        let account = AccountId::new(public_key);
        let literal = account.to_string();
        (account, literal, private_key)
    }

    fn sample_asset_definition_literal() -> String {
        AssetDefinitionId::new(
            DomainId::try_new("test", "universal").expect("domain"),
            "usd".parse().expect("name"),
        )
        .to_string()
    }

    fn sample_config() -> actual::IsoBridge {
        let (_account_id, account_literal, private_key) = sample_account_bundle();
        let asset_definition = sample_asset_definition_literal();
        actual::IsoBridge {
            enabled: true,
            dedupe_ttl_secs: 60,
            default_profile: "generic-iso20022".to_owned(),
            profiles: Vec::new(),
            store_dir: None,
            embedded_signature_policy: None,
            signer: Some(actual::IsoBridgeSigner {
                account_id: account_literal.clone(),
                private_key,
            }),
            account_aliases: vec![actual::IsoAccountAlias {
                iban: "GB82 WEST 1234 5698 7654 32".to_string(),
                account_id: account_literal,
            }],
            currency_assets: vec![actual::IsoCurrencyAsset {
                currency: "USD".to_string(),
                asset_definition,
            }],
            reference_data: actual::IsoReferenceData::default(),
        }
    }

    fn sample_pacs008() -> ParsedMessage {
        parse_message(
            "pacs.008",
            b"MsgId=m-profile\nIntrBkSttlmAmt=10.00\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed")
    }

    fn live_message_profile(message_type: &str, version: &str) -> actual::IsoBridgeProfile {
        actual::IsoBridgeProfile {
            id: format!("{message_type}-live-test"),
            rail: "generic-iso20022".to_owned(),
            embedded_signature_policy: None,
            trusted_public_key_sha256: Vec::new(),
            trusted_certificate_sha256: Vec::new(),
            required_reference_datasets: Vec::new(),
            message_profiles: vec![actual::IsoMessageProfile {
                message_type: message_type.to_owned(),
                direction: "inbound".to_owned(),
                versions: vec![version.to_owned()],
                business_services: vec!["swift.cbprplus.02".to_owned()],
                require_app_header: true,
                require_business_service: true,
                require_uetr: false,
                structured_address_mode: "permissive".to_owned(),
                supplementary_data_max_bytes: 4096,
                amount_minor_units: Vec::new(),
            }],
        }
    }

    fn signed_message_profile(policy: &str) -> actual::IsoBridgeProfile {
        actual::IsoBridgeProfile {
            id: "signed-pacs008-test".to_owned(),
            rail: "generic-iso20022".to_owned(),
            embedded_signature_policy: Some(policy.to_owned()),
            trusted_public_key_sha256: vec![signed_profile_public_key_sha256()],
            trusted_certificate_sha256: Vec::new(),
            required_reference_datasets: Vec::new(),
            message_profiles: vec![actual::IsoMessageProfile {
                message_type: "pacs.008".to_owned(),
                direction: "inbound".to_owned(),
                versions: vec!["pacs.008".to_owned(), "pacs.008.001.08".to_owned()],
                business_services: Vec::new(),
                require_app_header: false,
                require_business_service: false,
                require_uetr: false,
                structured_address_mode: "permissive".to_owned(),
                supplementary_data_max_bytes: 4096,
                amount_minor_units: Vec::new(),
            }],
        }
    }

    fn xml_signature_test_signing_key() -> P256SigningKey {
        P256SigningKey::from_bytes(&[0x31; 32].into()).expect("deterministic P-256 key")
    }

    fn signed_profile_public_key_sha256() -> String {
        let signing_key = xml_signature_test_signing_key();
        sha256_hex(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
    }

    fn unsigned_pacs008_xml() -> String {
        concat!(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">"#,
            "<FIToFICstmrCdtTrf><GrpHdr><MsgId>sig-001</MsgId></GrpHdr>",
            r#"<CdtTrfTxInf><IntrBkSttlmAmt Ccy="USD">10.00</IntrBkSttlmAmt>"#,
            "<IntrBkSttlmDt>2024-01-01</IntrBkSttlmDt>",
            "<DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>",
            "<CdtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></CdtrAcct>",
            "<DbtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></DbtrAgt>",
            "<CdtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></CdtrAgt>",
            "</CdtTrfTxInf></FIToFICstmrCdtTrf></Document>",
        )
        .to_owned()
    }

    fn signed_pacs008_xml() -> String {
        signed_pacs008_xml_with_c14n_algorithm(XML_C14N_1_0)
    }

    fn signed_pacs008_xml_with_c14n_algorithm(c14n_algorithm: &str) -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let digest = BASE64_STANDARD.encode(Sha256::digest(unsigned.as_bytes()));
        let signed_info = format!(
            r#"<SignedInfo><CanonicalizationMethod Algorithm="{c14n_algorithm}"/><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"/><Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"/></Transforms><DigestMethod Algorithm="{XMLDSIG_SHA256}"/><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
        );
        let signing_key = xml_signature_test_signing_key();
        let signature: P256Signature = signing_key.sign(signed_info.as_bytes());
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                "<Signature>{signed_info}<SignatureValue>{signature_value}</SignatureValue>",
                "<KeyInfo><KeyValue><ECKeyValue>",
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"/>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"/></Object>"##,
                "</Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            public_key = public_key
        );
        format!(
            "{}{}{}",
            &unsigned[..insertion],
            signature_xml,
            &unsigned[insertion..]
        )
    }

    struct CertificateChainSignedPayload {
        payload: String,
        issuer_sha256: String,
    }

    fn signed_pacs008_xml_with_certificate_chain() -> CertificateChainSignedPayload {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let digest = BASE64_STANDARD.encode(Sha256::digest(unsigned.as_bytes()));
        let signed_info = format!(
            r#"<SignedInfo><CanonicalizationMethod Algorithm="{XML_C14N_1_0}"/><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"/><Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"/></Transforms><DigestMethod Algorithm="{XMLDSIG_SHA256}"/><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
        );

        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer_sha256 = sha256_hex(issuer_cert.der().as_ref());
        let issuer = Issuer::from_params(&issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        let signature_value = BASE64_STANDARD.encode(
            leaf_key
                .sign(signed_info.as_bytes())
                .expect("leaf XMLDSig signature"),
        );
        let leaf_certificate = BASE64_STANDARD.encode(leaf_cert.der().as_ref());
        let issuer_certificate = BASE64_STANDARD.encode(issuer_cert.der().as_ref());
        let signature_xml = format!(
            concat!(
                "<Signature>{signed_info}<SignatureValue>{signature_value}</SignatureValue>",
                "<KeyInfo><X509Data>",
                "<X509Certificate>{leaf_certificate}</X509Certificate>",
                "<X509Certificate>{issuer_certificate}</X509Certificate>",
                "</X509Data></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"/></Object>"##,
                "</Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            leaf_certificate = leaf_certificate,
            issuer_certificate = issuer_certificate
        );
        CertificateChainSignedPayload {
            payload: format!(
                "{}{}{}",
                &unsigned[..insertion],
                signature_xml,
                &unsigned[insertion..]
            ),
            issuer_sha256,
        }
    }

    fn sample_world(asset_alias: Option<&str>) -> World {
        let (authority, _, _) = sample_account_bundle();
        let domain_id: DomainId = DomainId::try_new("test", "universal").expect("domain");
        let asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "usd".parse().expect("name"));
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("USD".to_owned())
            .build(&authority);
        let world = World::with([domain], [account], [asset_definition]);
        if let Some(alias_literal) = asset_alias {
            let alias: AssetDefinitionAlias = alias_literal.parse().expect("asset alias");
            let mut block = world.block();
            let mut tx = block.transaction_without_telemetry(
                iroha_config::parameters::actual::LaneConfig::default(),
                0,
            );
            tx.bind_asset_definition_alias(&asset_definition_id, alias, None, None, 10_000)
                .expect("bind alias");
            tx.apply();
            block.commit();
        }
        world
    }

    fn write_snapshot(contents: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("snapshot file");
        file.write_all(contents.as_bytes()).expect("write snapshot");
        file
    }

    #[test]
    fn parse_account_address_literal_records_error_code() {
        let (value, observation) = super::parse_account_address_literal("not-an-address");
        assert_eq!(value.as_deref(), Some("not-an-address"));
        assert!(observation.error_code().is_some());
    }

    #[test]
    fn parse_account_address_literal_captures_domain_kind() {
        let key_pair =
            iroha_crypto::KeyPair::from_seed(vec![0xAB; 32], iroha_crypto::Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("address");
        let i105 = address
            .to_i105_for_discriminant(iroha_data_model::account::address::chain_discriminant())
            .expect("i105 encoding");
        let (value, observation) = super::parse_account_address_literal(&i105);
        assert!(value.is_some());
        assert_eq!(observation.error_code(), None);
        assert_eq!(observation.domain_label(), Some("default"));
    }

    #[test]
    fn parse_account_address_literal_rejects_canonical_hex() {
        let key_pair =
            iroha_crypto::KeyPair::from_seed(vec![0xAC; 32], iroha_crypto::Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("address");
        let canonical = address.canonical_hex().expect("canonical hex");
        let (value, observation) = super::parse_account_address_literal(&canonical);
        assert_eq!(value.as_deref(), Some(canonical.as_str()));
        assert_eq!(
            observation.error_code(),
            Some(AccountAddressError::UnsupportedAddressFormat.code_str())
        );
        assert_eq!(observation.domain_label(), None);
    }

    #[test]
    fn runtime_from_config_normalises_aliases() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let resolved = runtime
            .resolve_account("gb82west12345698765432")
            .expect("alias");
        let (expected_account, _, _) = sample_account_bundle();
        assert_eq!(resolved, expected_account);
    }

    #[test]
    fn runtime_maps_alias_indices() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let index = runtime
            .resolve_alias_index("GB82 WEST 1234 5698 7654 32")
            .expect("alias index");
        assert_eq!(index, AliasIndex(0));
        let (alias, account) = runtime
            .resolve_account_by_index(index)
            .expect("alias by index");
        assert_eq!(alias, "GB82WEST12345698765432");
        let (expected_account, _, _) = sample_account_bundle();
        assert_eq!(account, expected_account);
    }

    #[test]
    fn runtime_exposes_reference_data_snapshots() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert_eq!(
            runtime.reference_data().isin_cusip().state(),
            SnapshotState::Missing
        );
    }

    #[test]
    fn runtime_resolves_default_profile() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert_eq!(runtime.default_profile().id, "generic-iso20022");
        assert!(runtime.resolve_profile(Some("swift-cbpr-plus")).is_some());
        assert!(runtime.resolve_profile(Some("unknown-profile")).is_none());
    }

    #[test]
    fn runtime_from_config_rejects_noncanonical_xml_signature_pin() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256 = vec!["AB".repeat(32)];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("uppercase SHA-256 pins must fail configuration"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("trusted_public_key_sha256"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn profile_validation_records_metadata_for_generic_messages() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let parsed = sample_pacs008();
        let metadata = runtime
            .validate_profile_submission(
                runtime.default_profile(),
                "pacs.008",
                &parsed,
                b"profile payload",
            )
            .expect("generic profile accepts message");
        assert_eq!(metadata.profile_id(), Some("generic-iso20022"));
        assert_eq!(metadata.message_type(), Some("pacs.008"));
        assert!(metadata.payload_hash().is_some());
        assert!(metadata.reference_snapshot_id().is_some());
        assert!(!metadata.embedded_signature_detected());
    }

    #[test]
    fn live_profile_accepts_bah_fields_with_data_pdu_prefixes() {
        let mut config = sample_config();
        config
            .profiles
            .push(live_message_profile("pacs.008", "pacs.008.001.08"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let profile = runtime
            .resolve_profile(Some("pacs.008-live-test"))
            .expect("custom profile");
        let parsed = parse_message(
            "pacs.008",
            b"DataPDU/AppHdr/BizMsgIdr=HDR-123\nDataPDU/AppHdr/MsgDefIdr=pacs.008.001.08\nDataPDU/AppHdr/CreDt=2025-01-01T12:00:00Z\nDataPDU/AppHdr/BizSvc=swift.cbprplus.02\nMsgId=m-profile\nIntrBkSttlmAmt=10.00\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, b"profile payload")
            .expect("BAH suffix fields should satisfy live profile");

        assert_eq!(metadata.business_service(), Some("swift.cbprplus.02"));
        assert_eq!(metadata.business_message_id(), Some("HDR-123"));
    }

    #[test]
    fn live_profile_accepts_pacs009_canonical_app_header_aliases() {
        let mut config = sample_config();
        config
            .profiles
            .push(live_message_profile("pacs.009", "pacs.009.001.10"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let profile = runtime
            .resolve_profile(Some("pacs.009-live-test"))
            .expect("custom profile");
        let parsed = parse_message(
            "pacs.009",
            b"AppHdr/BizMsgIdr=HDR-009\nAppHdr/MsgDefIdr=pacs.009.001.10\nAppHdr/CreDt=2025-01-01T12:00:00Z\nAppHdr/BizSvc=swift.cbprplus.02\nIntrBkSttlmAmt=2500\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-03\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nInstgAgt=DEUTDEFF\nInstdAgt=DEUTDEFF",
        )
        .expect("parsed");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.009", &parsed, b"profile payload")
            .expect("canonicalized BAH aliases should satisfy live profile");

        assert_eq!(metadata.business_service(), Some("swift.cbprplus.02"));
        assert_eq!(metadata.business_message_id(), Some("HDR-009"));
    }

    #[test]
    fn require_verified_profile_accepts_valid_p256_xmldsig_xades() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("valid XMLDSig/XAdES payload should pass require-verified profile");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_supported_canonicalization_algorithms() {
        for c14n_algorithm in [XML_C14N_1_1, XML_EXCLUSIVE_C14N_1_0] {
            let mut config = sample_config();
            config
                .profiles
                .push(signed_message_profile("require-verified"));
            let runtime = Iso20022BridgeRuntime::from_config(&config)
                .expect("cfg")
                .expect("enabled");
            let payload = signed_pacs008_xml_with_c14n_algorithm(c14n_algorithm);
            let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
            let profile = runtime
                .resolve_profile(Some("signed-pacs008-test"))
                .expect("signed profile");

            runtime
                .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
                .expect("supported canonicalization URI should pass");
        }
    }

    #[test]
    fn require_verified_profile_rejects_unpinned_public_key() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256 = vec!["11".repeat(32)];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("require-verified must fail closed for unpinned XMLDSig keys");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_accepts_certificate_chain_with_pinned_issuer() {
        let CertificateChainSignedPayload {
            payload,
            issuer_sha256,
        } = signed_pacs008_xml_with_certificate_chain();
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![issuer_sha256];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("certificate chain ending at a pinned issuer must pass");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_with_unpinned_issuer() {
        let CertificateChainSignedPayload { payload, .. } =
            signed_pacs008_xml_with_certificate_chain();
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec!["11".repeat(32)];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("certificate chains must still require a configured trust pin");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_missing_signature() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = unsigned_pacs008_xml();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse unsigned XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("require-verified must fail closed when the signature is absent");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_unsupported_canonicalization_method() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml().replace(XML_C14N_1_0, "urn:unsupported:c14n");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("unsupported canonicalization methods must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_unsupported_signature_method() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload =
            signed_pacs008_xml().replace(XMLDSIG_ECDSA_SHA256, "urn:unsupported:rsa-sha1");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("unsupported signature methods must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_payload_digest_tampering() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml().replace(">10.00<", ">10.01<");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("reference digest mismatch must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_signature_value_tampering() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml().replacen("<SignatureValue>", "<SignatureValue>A", 1);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("invalid signature bytes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn reject_unsupported_profile_still_rejects_valid_embedded_signature() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("reject-unsupported"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("reject-unsupported live profiles must keep rejecting signatures");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn live_profile_requires_reference_datasets() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let parsed = sample_pacs008();
        let profile = runtime
            .resolve_profile(Some("swift-cbpr-plus"))
            .expect("swift profile");
        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, b"profile payload")
            .expect_err("missing reference data must reject live profile");
        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn profile_validation_rejects_amount_minor_unit_mismatch() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message(
            "pacs.008",
            b"MsgId=m-minor\nIntrBkSttlmAmt=10.001\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed");
        let err = runtime
            .validate_profile_submission(runtime.default_profile(), "pacs.008", &parsed, b"minor")
            .expect_err("USD has two minor units");
        assert!(matches!(
            err,
            MsgError::InvalidValue {
                field,
                kind: InvalidValueKind::Amount
            } if field == "IntrBkSttlmAmt"
        ));
    }

    #[test]
    fn profile_idempotency_rejects_replayed_uetr() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let first = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-1".to_owned()),
            Some("123e4567-e89b-12d3-a456-426614174000".to_owned()),
            "hash-1".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let replay = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-2".to_owned()),
            Some("123E4567-E89B-12D3-A456-426614174000".to_owned()),
            "hash-2".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        assert!(runtime.check_and_record_inbound("msg-1", first));
        assert!(!runtime.check_and_record_inbound("msg-2", replay));
    }

    #[test]
    fn retry_replacement_rejects_conflicting_uetr() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let first = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-1".to_owned()),
            Some("123e4567-e89b-12d3-a456-426614174000".to_owned()),
            "hash-1".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let second = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-2".to_owned()),
            Some("123e4567-e89b-12d3-a456-426614174001".to_owned()),
            "hash-2".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let conflicting_retry = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-1-retry".to_owned()),
            Some("123E4567-E89B-12D3-A456-426614174001".to_owned()),
            "hash-3".to_owned(),
            "snapshot".to_owned(),
            false,
        );

        assert!(runtime.check_and_record_inbound("msg-1", first));
        assert!(runtime.check_and_record_inbound("msg-2", second));
        runtime.mark_rejected("msg-1", Some("retry allowed".to_owned()), Some("ED05"));

        assert!(!runtime.check_and_record_inbound("msg-1", conflicting_retry));
        assert_eq!(
            runtime
                .uetr_index
                .get(&normalise_uetr("123e4567-e89b-12d3-a456-426614174001"))
                .map(|entry| entry.clone()),
            Some("msg-2".to_owned())
        );
    }

    #[test]
    fn durable_store_reloads_message_status() {
        let store = TempDir::new().expect("tempdir");
        let mut config = sample_config();
        config.store_dir = Some(store.path().to_path_buf());
        {
            let runtime = Iso20022BridgeRuntime::from_config(&config)
                .expect("cfg")
                .expect("enabled");
            assert!(runtime.check_and_record_message("persisted-msg"));
            runtime.update_message_context(
                "persisted-msg",
                IsoMessageContext {
                    settlement_amount: Some("99.50".to_owned()),
                    settlement_currency: Some("USD".to_owned()),
                    ..IsoMessageContext::default()
                },
            );
            runtime.mark_accepted("persisted-msg", "tx-persisted");
        }
        let reloaded = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let status = reloaded
            .message_status("persisted-msg")
            .expect("reloaded status");
        assert_eq!(status.status_label(), "Accepted");
        assert_eq!(status.transaction_hash(), Some("tx-persisted"));
        assert_eq!(status.settlement_amount(), Some("99.50"));
        assert_eq!(status.settlement_currency(), Some("USD"));
        assert!(!status.status_history().is_empty());
    }

    #[test]
    fn runtime_rejects_invalid_alias_iban() {
        let mut config = sample_config();
        config.account_aliases[0].iban = "INVALID".to_string();
        let err = Iso20022BridgeRuntime::from_config(&config)
            .err()
            .expect("invalid IBAN");
        assert!(
            err.to_string()
                .contains("iso_bridge account alias `INVALID` is not a valid IBAN")
        );
    }

    #[test]
    fn runtime_rejects_invalid_currency_code() {
        let mut config = sample_config();
        config.currency_assets[0].currency = "ZZZ".to_string();
        let err = Iso20022BridgeRuntime::from_config(&config)
            .err()
            .expect("invalid currency binding");
        assert!(
            err.to_string()
                .contains("iso_bridge currency binding `ZZZ` is not a valid ISO 4217 code")
        );
    }

    #[test]
    fn runtime_accepts_asset_alias_currency_binding() {
        let mut config = sample_config();
        config.currency_assets[0].asset_definition = "usd#test".to_string();
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let world = sample_world(Some("usd#test"));
        let world_view = world.view();

        let resolved = runtime
            .resolve_asset(&world_view, 10_000, "USD")
            .expect("currency binding should resolve");

        assert_eq!(resolved.to_string(), sample_asset_definition_literal());
    }

    #[test]
    fn runtime_rejects_legacy_signer_account_literal() {
        let mut config = sample_config();
        if let Some(ref mut signer) = config.signer {
            signer.account_id = LEGACY_PUBLIC_KEY_LITERAL.to_string();
        }
        let err = Iso20022BridgeRuntime::from_config(&config)
            .err()
            .expect("legacy signer literal must be rejected");
        assert!(err.to_string().contains("signer account_id"));
    }

    #[test]
    fn runtime_rejects_legacy_alias_account_literal() {
        let mut config = sample_config();
        config.account_aliases[0].account_id = LEGACY_PUBLIC_KEY_LITERAL.to_string();
        let err = Iso20022BridgeRuntime::from_config(&config)
            .err()
            .expect("legacy alias literal must be rejected");
        assert!(err.to_string().contains("account alias"));
    }

    #[test]
    fn dedupe_prevents_duplicates_within_ttl() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("abc"));
        assert!(!runtime.check_and_record_message("abc"));
    }

    #[test]
    fn build_transaction_extracts_transfer() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.008",
            b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF\nSplmtryData/SourceAccountAddress=0xdebtor\nSplmtryData/TargetAccountAddress=0xcreditor",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let (tx, context) = runtime
            .build_pacs008_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect("build");
        assert_eq!(context.ledger_id.as_deref(), Some(chain_id.as_str()));
        let (_expected_account, canonical_account, _) = sample_account_bundle();
        assert_eq!(
            context.source_account_id.as_deref(),
            Some(canonical_account.as_str())
        );
        assert_eq!(
            context.target_account_id.as_deref(),
            Some(canonical_account.as_str())
        );
        let asset_definition = sample_asset_definition_literal();
        assert_eq!(
            context.asset_definition_id.as_deref(),
            Some(asset_definition.as_str())
        );
        assert_eq!(context.source_account_address.as_deref(), Some("0xdebtor"));
        assert_eq!(
            context.target_account_address.as_deref(),
            Some("0xcreditor")
        );
        assert!(context.asset_id.as_ref().is_some());

        assert_eq!(tx.chain(), &chain_id);
        let metadata = tx.metadata();

        let ledger_key = Name::from_str("iso20022_ledger_id").unwrap();
        let stored_ledger = metadata
            .get(&ledger_key)
            .and_then(|json| json.try_into_any_norito::<String>().ok())
            .expect("ledger metadata");
        assert_eq!(stored_ledger, chain_id.as_str());

        let source_key = Name::from_str("iso20022_source_account_id").unwrap();
        let stored_source = metadata
            .get(&source_key)
            .and_then(|json| json.try_into_any_norito::<String>().ok())
            .expect("source metadata");
        assert_eq!(stored_source, canonical_account);

        let asset_key = Name::from_str("iso20022_asset_definition_id").unwrap();
        let stored_asset = metadata
            .get(&asset_key)
            .and_then(|json| json.try_into_any_norito::<String>().ok())
            .expect("asset metadata");
        assert_eq!(stored_asset, asset_definition);

        let asset_id_key = Name::from_str("iso20022_asset_id").unwrap();
        let stored_asset_id = metadata
            .get(&asset_id_key)
            .and_then(|json| json.try_into_any_norito::<String>().ok())
            .expect("asset id metadata");
        assert_eq!(context.asset_id.as_deref(), Some(stored_asset_id.as_str()));
    }

    #[test]
    fn build_transaction_accepts_asset_alias_hint() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.008",
            b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF\nSplmtryData/AssetDefinitionId=usd#test",
        )
        .expect("parsed");
        let world = sample_world(Some("usd#test"));
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let (_tx, context) = runtime
            .build_pacs008_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect("build");
        let expected_asset_definition = sample_asset_definition_literal();

        assert_eq!(
            context.asset_definition_id.as_deref(),
            Some(expected_asset_definition.as_str())
        );
    }

    #[test]
    fn pacs008_rejects_unknown_bic() {
        let snapshot = r#"{
            "version":"2024-05-01",
            "source":"GLEIF sample",
            "entries":[
                {"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}
            ]
        }"#;
        let file = write_snapshot(snapshot);

        let mut config = sample_config();
        config.reference_data.bic_lei_path = Some(file.path().to_path_buf());

        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");

        let msg = parse_message(
            "pacs.008",
            b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=TESTUS33\nCdtrAgt=TESTUS33",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs008_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("unknown BIC must fail");
        match err {
            MsgError::InvalidIdentifier { ref field, kind } => {
                assert_eq!(field, "DbtrAgt");
                assert_eq!(kind, IdentifierKind::Bic);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn pacs008_rejects_unmapped_iban() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.008",
            b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB29NWBK60161331926819\nCdtrAcct=GB29NWBK60161331926819\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs008_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("unmapped IBAN must fail");
        match err {
            MsgError::InvalidIdentifier { ref field, kind } => {
                assert_eq!(field, "DbtrAcct");
                assert_eq!(kind, IdentifierKind::Iban);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn pacs008_rejects_unbound_currency() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.008",
            b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=EUR\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs008_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("unbound currency must fail");
        match err {
            MsgError::InvalidIdentifier { ref field, kind } => {
                assert_eq!(field, "IntrBkSttlmCcy");
                assert_eq!(kind, IdentifierKind::Currency);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn build_pacs009_transaction_extracts_transfer() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.009",
            b"BizMsgIdr=b1\nMsgDefIdr=pacs.009.001.10\nCreDtTm=2024-01-01T12:00:00Z\nIntrBkSttlmAmt=2500\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-03\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nInstgAgt=DEUTDEFF\nInstdAgt=DEUTDEFF\nPurp=SECU\nSplmtryData/SourceAccountAddress=0xdebtor\nSplmtryData/TargetAccountAddress=0xcreditor",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let (tx, context) = runtime
            .build_pacs009_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect("build");
        assert_eq!(context.ledger_id.as_deref(), Some(chain_id.as_str()));
        let (_expected_account, canonical_account, _) = sample_account_bundle();
        assert_eq!(
            context.source_account_id.as_deref(),
            Some(canonical_account.as_str())
        );
        assert_eq!(
            context.target_account_id.as_deref(),
            Some(canonical_account.as_str())
        );
        let asset_definition = sample_asset_definition_literal();
        assert_eq!(
            context.asset_definition_id.as_deref(),
            Some(asset_definition.as_str())
        );

        let metadata = tx.metadata();
        let purpose_key = Name::from_str("iso20022_category_purpose").unwrap();
        let stored_purpose = metadata
            .get(&purpose_key)
            .and_then(|json| json.try_into_any_norito::<String>().ok())
            .expect("purpose metadata");
        assert_eq!(stored_purpose, "SECU");
    }

    #[test]
    fn pacs009_requires_securities_purpose() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.009",
            b"BizMsgIdr=b1\nMsgDefIdr=pacs.009.001.10\nCreDtTm=2024-01-01T12:00:00Z\nIntrBkSttlmAmt=2500\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-03\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nInstgAgt=DEUTDEFF\nInstdAgt=DEUTDEFF\nPurp=OTHR",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs009_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("non-SECU purpose must fail");
        match err {
            MsgError::InvalidValue { field, kind } => {
                assert_eq!(field, "Purp");
                assert_eq!(kind, InvalidValueKind::Enum);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn pacs009_rejects_unmapped_iban() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.009",
            b"BizMsgIdr=b1\nMsgDefIdr=pacs.009.001.10\nCreDtTm=2024-01-01T12:00:00Z\nIntrBkSttlmAmt=2500\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-03\nDbtrAcct=GB29NWBK60161331926819\nCdtrAcct=GB29NWBK60161331926819\nInstgAgt=DEUTDEFF\nInstdAgt=DEUTDEFF\nPurp=SECU",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs009_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("unmapped IBAN must fail");
        match err {
            MsgError::InvalidIdentifier { ref field, kind } => {
                assert_eq!(field, "DbtrAcct");
                assert_eq!(kind, IdentifierKind::Iban);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn pacs009_rejects_unbound_currency() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.009",
            b"BizMsgIdr=b1\nMsgDefIdr=pacs.009.001.10\nCreDtTm=2024-01-01T12:00:00Z\nIntrBkSttlmAmt=2500\nIntrBkSttlmCcy=EUR\nIntrBkSttlmDt=2024-01-03\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nInstgAgt=DEUTDEFF\nInstdAgt=DEUTDEFF\nPurp=SECU",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs009_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("unbound currency must fail");
        match err {
            MsgError::InvalidIdentifier { ref field, kind } => {
                assert_eq!(field, "IntrBkSttlmCcy");
                assert_eq!(kind, IdentifierKind::Currency);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn pacs009_rejects_unknown_bic() {
        let snapshot = r#"{
            "version":"2024-05-01",
            "source":"GLEIF sample",
            "entries":[
                {"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}
            ]
        }"#;
        let file = write_snapshot(snapshot);

        let mut config = sample_config();
        config.reference_data.bic_lei_path = Some(file.path().to_path_buf());

        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");

        let msg = parse_message(
            "pacs.009",
            b"BizMsgIdr=b1\nMsgDefIdr=pacs.009.001.10\nCreDtTm=2024-01-01T12:00:00Z\nIntrBkSttlmAmt=2500\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-03\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nInstgAgt=TESTUS33\nInstdAgt=TESTUS33\nPurp=SECU",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let err = runtime
            .build_pacs009_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect_err("unknown BIC must fail");
        match err {
            MsgError::InvalidIdentifier { ref field, kind } => {
                assert_eq!(field, "InstgAgt");
                assert_eq!(kind, IdentifierKind::Bic);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejected_message_can_be_retried() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("m1"));
        runtime.mark_rejected("m1", Some("missing mapping".to_string()), None);
        assert!(runtime.check_and_record_message("m1"));
    }

    #[test]
    fn status_transitions_are_recorded() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("m2"));
        let pending = runtime.message_status("m2").expect("status");
        assert_eq!(pending.status_label(), "Pending");
        assert!(pending.ledger_id().is_none());
        assert_eq!(pending.pacs002_code(), "ACTC");
        runtime.mark_accepted("m2", "hash-1");
        let accepted = runtime.message_status("m2").expect("status");
        assert_eq!(accepted.status_label(), "Accepted");
        assert_eq!(accepted.pacs002_code(), "ACSP");
        assert_eq!(accepted.transaction_hash(), Some("hash-1"));
        runtime.mark_transaction_applied("hash-1", SystemTime::now());
        let settled = runtime.message_status("m2").expect("status");
        assert_eq!(settled.pacs002_code(), "ACSC");
    }

    #[test]
    fn rejected_status_carries_reason() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("m3"));
        runtime.mark_rejected("m3", Some("validation failed".to_string()), None);
        let status = runtime.message_status("m3").expect("status");
        assert_eq!(status.status_label(), "Rejected");
        assert_eq!(status.pacs002_code(), "RJCT");
        assert_eq!(status.detail(), Some("validation failed"));
        assert!(status.transaction_hash().is_none());
    }

    #[test]
    fn message_context_is_preserved_in_status() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_ctx";
        assert!(runtime.check_and_record_message(message_id));
        let context = IsoMessageContext {
            ledger_id: Some("ledger-A".to_string()),
            source_account_id: Some(
                "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".to_string(),
            ),
            ..IsoMessageContext::default()
        };
        runtime.update_message_context(message_id, context.clone());
        runtime.mark_accepted(message_id, "hash-ctx");

        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.ledger_id(), Some("ledger-A"));
        assert_eq!(
            status.source_account_id(),
            Some("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76")
        );
        assert_eq!(status.transaction_hash(), Some("hash-ctx"));
    }

    #[test]
    fn payment_outbox_xml_uses_durable_amount_currency_and_escapes_reasons() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let msg = parse_message(
            "pacs.008",
            b"MsgId=m-outbox\nIntrBkSttlmAmt=10.25\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed");
        let world = sample_world(None);
        let world_view = world.view();
        let chain_id: ChainId = "test-chain".parse().unwrap();
        let telemetry = MaybeTelemetry::for_tests();
        let (_tx, context) = runtime
            .build_pacs008_transaction(&msg, &world_view, 10_000, &chain_id, &telemetry)
            .expect("build");
        assert_eq!(context.settlement_amount(), Some("10.25"));
        assert_eq!(context.settlement_currency(), Some("USD"));

        assert!(runtime.check_and_record_message("m-outbox"));
        runtime.update_message_context("m-outbox", context);
        runtime.mark_rejected(
            "m-outbox",
            Some("return <detail> & audit".to_owned()),
            Some("PRTRY:RETURN<&>"),
        );
        let status = runtime.message_status("m-outbox").expect("status");

        let pacs004 = crate::iso_pacs004_xml(&status).expect("pacs.004 xml");
        assert!(pacs004.contains("<RtrdIntrBkSttlmAmt Ccy=\"USD\">10.25</RtrdIntrBkSttlmAmt>"));
        assert!(pacs004.contains("return &lt;detail&gt; &amp; audit"));
        assert!(!pacs004.contains("return <detail> & audit"));
        parse_message("pacs.004", pacs004.as_bytes()).expect("generated pacs.004 parses");

        let camt029 = crate::iso_camt029_xml(&status);
        assert!(camt029.contains("<Conf>CNCL</Conf>"));
        assert!(camt029.contains("RETURN&lt;&amp;&gt;"));
        parse_message("camt.029", camt029.as_bytes()).expect("generated camt.029 parses");
    }

    #[test]
    fn pacs004_outbox_refuses_missing_settlement_amount_or_currency() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("m-missing-amount"));
        runtime.update_message_context(
            "m-missing-amount",
            IsoMessageContext {
                settlement_currency: Some("USD".to_owned()),
                ..IsoMessageContext::default()
            },
        );
        runtime.mark_rejected("m-missing-amount", Some("return".to_owned()), Some("BE01"));
        let status = runtime.message_status("m-missing-amount").expect("status");
        let err = crate::iso_pacs004_xml(&status).expect_err("missing amount must fail");
        assert_outbox_error_contains(err, "settlement_amount");

        assert!(runtime.check_and_record_message("m-missing-currency"));
        runtime.update_message_context(
            "m-missing-currency",
            IsoMessageContext {
                settlement_amount: Some("10".to_owned()),
                ..IsoMessageContext::default()
            },
        );
        runtime.mark_rejected(
            "m-missing-currency",
            Some("return".to_owned()),
            Some("BE01"),
        );
        let status = runtime
            .message_status("m-missing-currency")
            .expect("status");
        let err = crate::iso_pacs004_xml(&status).expect_err("missing currency must fail");
        assert_outbox_error_contains(err, "settlement_currency");
    }

    #[test]
    fn securities_outbox_xml_uses_sese023_context_and_settlement_state() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message(
            "sese.023",
            b"TxId=SETT-123\nSttlmDt=2025-11-12\nSttlmTpAndAddtlParams/SctiesMvmntTp=DELI\nSttlmTpAndAddtlParams/Pmt=APMT\nSctiesLeg/FinInstrmId=US0378331005\nSctiesLeg/Qty=42\nCashLeg/Amt=1234.56\nCashLeg/Ccy=USD\nDlvrgSttlmPties/Pty/Bic=DEUTDEFF\nDlvrgSttlmPties/Acct=DELIVER-1\nRcvgSttlmPties/Pty/Bic=MARKDEFF\nRcvgSttlmPties/Acct=RECEIVE-1\nPlan/ExecutionOrder=DELIVERY_THEN_PAYMENT\nPlan/Atomicity=ALL_OR_NOTHING",
        )
        .expect("parsed sese.023");
        let message_id =
            Iso20022BridgeRuntime::lifecycle_message_id("sese.023", &parsed).expect("message id");
        assert_eq!(message_id, "sese.023:SETT-123");
        assert!(runtime.check_and_record_message(&message_id));
        runtime
            .apply_inbound_lifecycle_message(&message_id, "sese.023", &parsed)
            .expect("record sese.023");

        let accepted = runtime
            .message_status(&message_id)
            .expect("accepted status");
        assert_eq!(accepted.settlement_amount(), Some("1234.56"));
        assert_eq!(accepted.settlement_quantity(), Some("42"));
        let accepted_status = crate::iso_sese024_xml(&accepted);
        assert!(accepted_status.contains("<TxId>SETT-123</TxId>"));
        assert!(accepted_status.contains("<Sts><Cd>ACCP</Cd></Sts>"));
        parse_message("sese.024", accepted_status.as_bytes())
            .expect("generated accepted sese.024 parses");

        runtime.mark_hold(&message_id, Some("NORE"));
        let pending = runtime.message_status(&message_id).expect("pending status");
        let pending_status = crate::iso_sese024_xml(&pending);
        assert!(pending_status.contains("<Sts><Cd>PEND</Cd></Sts>"));
        assert!(pending_status.contains("<Rsn><Cd>NORE</Cd></Rsn>"));
        parse_message("sese.024", pending_status.as_bytes())
            .expect("generated pending sese.024 parses");

        runtime.mark_settled(&message_id, SystemTime::now());
        let settled = runtime.message_status(&message_id).expect("settled status");
        let confirmation = crate::iso_sese025_xml(&settled).expect("sese.025 xml");
        assert!(confirmation.contains("<TxId>SETT-123</TxId>"));
        assert!(confirmation.contains("<SttlmAmt Ccy=\"USD\">1234.56</SttlmAmt>"));
        assert!(confirmation.contains("<Unit>42</Unit>"));
        assert!(confirmation.contains("<ExecutionOrder>DELIVERY_THEN_PAYMENT</ExecutionOrder>"));
        parse_message("sese.025", confirmation.as_bytes()).expect("generated sese.025 parses");
    }

    #[test]
    fn sese025_outbox_refuses_unsettled_or_incomplete_records() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("sese.023:INCOMPLETE"));
        runtime.update_message_context(
            "sese.023:INCOMPLETE",
            IsoMessageContext {
                settlement_amount: Some("15".to_owned()),
                settlement_currency: Some("USD".to_owned()),
                settlement_quantity: Some("1".to_owned()),
                settlement_movement_type: Some("DELI".to_owned()),
                settlement_payment_type: Some("APMT".to_owned()),
                plan_execution_order: Some("DELIVERY_THEN_PAYMENT".to_owned()),
                plan_atomicity: Some("ALL_OR_NOTHING".to_owned()),
                ..IsoMessageContext::default()
            },
        );
        let pending = runtime
            .message_status("sese.023:INCOMPLETE")
            .expect("pending");
        let err = crate::iso_sese025_xml(&pending).expect_err("unsettled must fail");
        assert_outbox_error_contains(err, "requires a settled");

        runtime.mark_settled("sese.023:INCOMPLETE", SystemTime::now());
        runtime.update_message_context(
            "sese.023:INCOMPLETE",
            IsoMessageContext {
                settlement_amount: Some("15".to_owned()),
                settlement_currency: Some("USD".to_owned()),
                settlement_quantity: Some("1".to_owned()),
                settlement_movement_type: Some("DELI".to_owned()),
                settlement_payment_type: Some("APMT".to_owned()),
                plan_execution_order: Some("DELIVERY_THEN_PAYMENT".to_owned()),
                ..IsoMessageContext::default()
            },
        );
        runtime.mark_settled("sese.023:INCOMPLETE", SystemTime::now());
        let incomplete = runtime
            .message_status("sese.023:INCOMPLETE")
            .expect("settled");
        let err = crate::iso_sese025_xml(&incomplete).expect_err("missing atomicity must fail");
        assert_outbox_error_contains(err, "plan_atomicity");
    }

    #[test]
    fn queued_message_reports_acsp() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_queue";
        assert!(runtime.check_and_record_message(message_id));
        runtime.mark_queued(message_id);
        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.pacs002_code(), "ACSP");
        assert_eq!(status.derived_status(), Pacs002Status::Acsp);
    }

    #[test]
    fn hold_message_reports_pdng() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_hold";
        assert!(runtime.check_and_record_message(message_id));
        runtime.mark_hold(message_id, Some("PDNG"));
        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.pacs002_code(), "PDNG");
        assert_eq!(status.hold_reason_code(), Some("PDNG"));
    }

    #[test]
    fn change_message_reports_acwc() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_acwc";
        assert!(runtime.check_and_record_message(message_id));
        runtime.add_change_reason_code(message_id, "VAL_DATE_SHIFT");
        runtime.add_change_reason_code(message_id, "VAL_DATE_SHIFT");
        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.pacs002_code(), "ACWC");
        assert_eq!(
            status.change_reason_codes(),
            &["VAL_DATE_SHIFT".to_owned()][..]
        );
    }

    #[test]
    fn transaction_rejection_marks_message() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_reject";
        assert!(runtime.check_and_record_message(message_id));
        runtime.mark_accepted(message_id, "tx-reject");
        let reason = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "too many instructions".to_owned(),
        });
        runtime.mark_transaction_rejected("tx-reject", Some(&reason));
        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.pacs002_code(), "RJCT");
        assert_eq!(status.rejection_reason_code(), Some("BE01"));
        assert_eq!(
            status.detail(),
            Some("Transaction limit check failed: too many instructions"),
        );
    }

    #[test]
    fn axt_rejection_produces_prtry_code_and_detail() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_axt_reject";
        assert!(runtime.check_and_record_message(message_id));
        runtime.mark_accepted(message_id, "tx-axt");
        let ctx = AxtRejectContext {
            reason: AxtRejectReason::HandleEra,
            dataspace: Some(DataSpaceId::new(11)),
            lane: Some(LaneId::new(2)),
            snapshot_version: 99,
            detail: "handle era below policy minimum".to_owned(),
            next_min_handle_era: Some(7),
            next_min_sub_nonce: Some(4),
        };
        let reason = TransactionRejectionReason::Validation(ValidationFail::AxtReject(ctx));
        runtime.mark_transaction_rejected("tx-axt", Some(&reason));
        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.pacs002_code(), "RJCT");
        assert_eq!(status.rejection_reason_code(), Some("PRTRY:AXT_HANDLE_ERA"));
        let detail = status.detail().expect("detail");
        assert!(
            detail.contains("AXT rejection"),
            "detail missing AXT label: {detail}"
        );
        assert!(
            detail.contains("snapshot_version=99"),
            "detail missing snapshot version: {detail}"
        );
        assert!(
            detail.contains("dsid=11") && detail.contains("lane=2"),
            "detail missing ids: {detail}"
        );
        assert!(
            detail.contains("next_min_handle_era=7") && detail.contains("next_min_sub_nonce=4"),
            "detail missing hints: {detail}"
        );
    }

    #[test]
    fn transaction_expiry_marks_message() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let message_id = "m_expired";
        assert!(runtime.check_and_record_message(message_id));
        runtime.mark_accepted(message_id, "tx-expired");
        runtime.mark_transaction_expired("tx-expired");
        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.pacs002_code(), "RJCT");
        assert_eq!(status.rejection_reason_code(), Some("ED07"));
        assert_eq!(
            status.detail(),
            Some("transaction expired before admission")
        );
    }

    #[test]
    fn lifecycle_rejects_replayed_payload_hash() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let first = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.002",
            None,
            Some("status-1".to_owned()),
            None,
            "same-payload-hash".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let replay = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.002",
            None,
            Some("status-2".to_owned()),
            None,
            "same-payload-hash".to_owned(),
            "snapshot".to_owned(),
            false,
        );

        assert!(runtime.check_and_record_inbound("status-1", first));
        assert!(!runtime.check_and_record_inbound("status-2", replay));
    }

    #[test]
    fn lifecycle_pacs002_settles_known_original() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("orig-1"));
        runtime.mark_accepted("orig-1", "tx-orig-1");
        let parsed = parse_message("pacs.002", b"MsgId=status-1\nOrgnlMsgId=orig-1\nTxSts=ACSC")
            .expect("pacs.002 parsed");
        let metadata = runtime
            .validate_profile_submission(runtime.default_profile(), "pacs.002", &parsed, b"pacs2")
            .expect("profile accepts pacs.002");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("pacs.002", &parsed).expect("lifecycle id");

        assert_eq!(lifecycle_id, "status-1");
        assert!(runtime.check_and_record_inbound(&lifecycle_id, metadata));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "pacs.002", &parsed)
            .expect("lifecycle applied");

        assert_eq!(outcome.referenced_message_id(), Some("orig-1"));
        assert!(outcome.referenced_message_known());
        assert_eq!(outcome.lifecycle_status_code(), Some("ACSC"));
        assert_eq!(outcome.action(), "marked_settled");
        assert_eq!(
            runtime
                .message_status("status-1")
                .expect("lifecycle status")
                .status_label(),
            "Accepted"
        );
        assert_eq!(
            runtime
                .message_status("orig-1")
                .expect("original status")
                .pacs002_code(),
            "ACSC"
        );
    }

    #[test]
    fn lifecycle_pacs004_marks_original_returned() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("orig-return"));
        runtime.mark_accepted("orig-return", "tx-return");
        let parsed = parse_message(
            "pacs.004",
            b"MsgId=return-1\nCreDtTm=2025-01-01T00:00:00Z\nOrgnlGrpInf/OrgnlMsgId=orig-return\nTxInf[0]/OrgnlInstrId=instr-1\nTxInf[0]/RtrdInstdAmt=10.00\nTxInf[0]/RtrdInstdAmtCcy=USD\nTxInf[0]/RtrdRsn/Cd=AC01",
        )
        .expect("pacs.004 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("pacs.004", &parsed).expect("lifecycle id");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "pacs.004", &parsed)
            .expect("lifecycle applied");

        assert_eq!(outcome.referenced_message_id(), Some("orig-return"));
        assert_eq!(outcome.lifecycle_reason_code(), Some("AC01"));
        assert_eq!(outcome.action(), "marked_returned");
        let original = runtime
            .message_status("orig-return")
            .expect("original status");
        assert_eq!(original.status_label(), "Rejected");
        assert_eq!(original.rejection_reason_code(), Some("AC01"));
    }

    #[test]
    fn lifecycle_camt056_records_unknown_original_without_creating_it() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message(
            "camt.056",
            b"Assgnmt/Id=cancel-1\nAssgnmt/CreDtTm=2025-01-01T00:00:00Z\nUndrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId=missing-original\nUndrlyg/TxInf/CxlRsnInf/Rsn/Cd=CUST",
        )
        .expect("camt.056 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("camt.056", &parsed).expect("lifecycle id");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "camt.056", &parsed)
            .expect("lifecycle applied");

        assert_eq!(outcome.referenced_message_id(), Some("missing-original"));
        assert!(!outcome.referenced_message_known());
        assert_eq!(outcome.action(), "recorded");
        assert!(runtime.message_status("missing-original").is_none());
        assert_eq!(
            runtime
                .message_status("cancel-1")
                .expect("lifecycle status")
                .status_label(),
            "Accepted"
        );
    }

    #[test]
    fn lifecycle_sese025_confirms_prefixed_settlement_instruction() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("sese.023:settle-1"));
        let parsed = parse_message(
            "sese.025",
            b"TxId=settle-1\nSttlmDt=2025-01-02\nSttlmTpAndAddtlParams/SctiesMvmntTp=DELI\nSttlmTpAndAddtlParams/Pmt=APMT\nConfSts=ACCP\nSttlmQty=100\nSttlmAmt=25.00\nSttlmCcy=USD\nPlan/ExecutionOrder=DELIVERY_THEN_PAYMENT\nPlan/Atomicity=ALL_OR_NOTHING",
        )
        .expect("sese.025 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("sese.025", &parsed).expect("lifecycle id");
        assert_eq!(lifecycle_id, "sese.025:settle-1");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "sese.025", &parsed)
            .expect("lifecycle applied");

        assert_eq!(outcome.referenced_message_id(), Some("sese.023:settle-1"));
        assert_eq!(outcome.action(), "marked_settled");
        assert_eq!(
            runtime
                .message_status("sese.023:settle-1")
                .expect("settlement instruction status")
                .pacs002_code(),
            "ACSC"
        );
    }

    #[test]
    fn lifecycle_wrong_message_family_fails_parser_validation() {
        let err = parse_message(
            "pacs.004",
            b"MsgId=m1\nIntrBkSttlmAmt=10\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect_err("pacs.008 fields must not satisfy pacs.004 endpoint");
        assert!(matches!(err, MsgError::MissingField(_)));
    }
}
