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
use time::{Date, Month, PrimitiveDateTime, Time, UtcOffset};
use x509_parser::{
    extensions::ParsedExtension,
    oid_registry::{OID_EC_P256, OID_KEY_TYPE_EC_PUBLIC_KEY, OID_SIG_ECDSA_WITH_SHA256},
    prelude::{FromDer as _, X509Certificate},
    time::ASN1Time,
};

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
    business_message_id_index: DashMap<String, String>,
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
    let revoked_certificate_sha256 = normalise_sha256_pins(
        &config.revoked_certificate_sha256,
        &format!("iso_bridge profile `{id}` revoked_certificate_sha256"),
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
        revoked_certificate_sha256,
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
            business_message_id_index: DashMap::new(),
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
            .ok_or(MsgError::UnknownMessageType)?;
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
                if !embedded_signature_detected
                    || !verify_embedded_xml_signature(payload, profile, parsed)?
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
                .business_message_id()
                .and_then(|id| {
                    self.business_message_id_index
                        .get(&normalise_business_message_id(id))
                })
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
        if let Some(id) = metadata.business_message_id() {
            self.business_message_id_index
                .insert(normalise_business_message_id(id), message_id.to_owned());
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
        if let Some(id) = record.metadata.business_message_id() {
            self.business_message_id_index
                .remove(&normalise_business_message_id(id));
        }
        if let Some(uetr) = record.metadata.uetr() {
            self.uetr_index.remove(&normalise_uetr(uetr));
        }
        self.payload_hash_index
            .retain(|_, existing_message| existing_message != message_id);
        self.business_message_id_index
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

const XMLDSIG_NS: &str = "http://www.w3.org/2000/09/xmldsig#";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";
const XMLDSIG_ECDSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256";
const XMLDSIG_SHA256: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const XMLDSIG_ENVELOPED_SIGNATURE: &str = "http://www.w3.org/2000/09/xmldsig#enveloped-signature";
const XADES_NS: &str = "http://uri.etsi.org/01903/v1.3.2#";
const XADES_SIGNED_PROPERTIES_TYPE: &str = "http://uri.etsi.org/01903#SignedProperties";
const XML_C14N_1_0: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const XML_C14N_1_1: &str = "http://www.w3.org/2006/12/xml-c14n11";
const XML_EXCLUSIVE_C14N_1_0: &str = "http://www.w3.org/2001/10/xml-exc-c14n#";
const XMLDSIG_P256_NAMED_CURVE: &str = "urn:oid:1.2.840.10045.3.1.7";
const P256_XMLDSIG_SIGNATURE_LEN: usize = 64;
const P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN: usize = 65;
const XML_SIGNATURE_MAX_X509_CERTIFICATES: usize = 8;

fn payload_has_embedded_signature(payload: &[u8]) -> bool {
    std::str::from_utf8(payload).is_ok_and(|text| {
        find_first_xml_element(text, "Signature").is_some()
            || find_first_xml_element(text, "Sgntr").is_some()
    })
}

fn verify_embedded_xml_signature(
    payload: &[u8],
    profile: &TradfiRailProfile,
    parsed: &ParsedMessage,
) -> Result<bool, MsgError> {
    let text = std::str::from_utf8(payload).map_err(|_| MsgError::InvalidFormat)?;
    let Some(signature_carrier) = xml_signature_carrier(text)? else {
        return Ok(false);
    };
    let signature_span = signature_carrier.signature_span;
    let signature_xml = &text[signature_span.start..signature_span.end];
    let signature_children = xml_signature_direct_child_spans_with_namespaces(
        signature_xml,
        &signature_carrier.inherited_namespaces,
    )?;
    let signature_root_span = required_single_xml_element(signature_xml, "Signature")?;
    let signature_namespaces = xml_element_namespace_scope(
        signature_xml,
        signature_root_span,
        &signature_carrier.inherited_namespaces,
    )?;
    let signed_info_xml =
        &signature_xml[signature_children.signed_info.start..signature_children.signed_info.end];
    let signed_info_root_span = required_single_xml_element(signed_info_xml, "SignedInfo")?;
    let signed_info_namespaces = xml_element_namespace_scope(
        signed_info_xml,
        signed_info_root_span,
        &signature_namespaces,
    )?;
    let signed_info_children =
        xml_signed_info_direct_child_spans_with_namespaces(signed_info_xml, &signature_namespaces)?;
    let c14n_method_span = signed_info_children.canonicalization_method;
    ensure_xml_element_attributes_allowed(signed_info_xml, c14n_method_span, &["Algorithm"])?;
    ensure_xml_element_content_empty(signed_info_xml, c14n_method_span)?;
    let c14n_algorithm = element_attr(signed_info_xml, c14n_method_span, "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    let c14n_mode = xml_canonicalization_mode(&c14n_algorithm)?;
    let signature_method_span = signed_info_children.signature_method;
    ensure_xml_element_attributes_allowed(signed_info_xml, signature_method_span, &["Algorithm"])?;
    ensure_xml_element_content_empty(signed_info_xml, signature_method_span)?;
    let signature_algorithm = element_attr(signed_info_xml, signature_method_span, "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if signature_algorithm != XMLDSIG_ECDSA_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    let inherited_namespaces = inherited_namespace_attributes_from_scope(&signature_namespaces);
    let canonical_signed_info =
        canonicalize_supported_xml_with_mode(signed_info_xml, &inherited_namespaces, c14n_mode)?;
    let reference_verification = verify_xml_signature_references(
        text,
        signature_span,
        signature_carrier.carrier_span,
        signature_xml,
        signed_info_xml,
        &signed_info_namespaces,
    )?;

    let signature_value =
        decode_direct_child_base64(signature_xml, signature_children.signature_value)?;
    let evaluation_time = xml_signature_evaluation_time_for_verified_properties(
        parsed,
        reference_verification.signed_properties.as_ref(),
    )?;
    let key_info_xml =
        &signature_xml[signature_children.key_info.start..signature_children.key_info.end];
    let key_material = xml_signature_key_material_with_namespaces(
        key_info_xml,
        evaluation_time,
        &signature_namespaces,
    )?;
    if let Some(signed_properties) = reference_verification.signed_properties.as_ref() {
        verify_xades_signing_certificate_v2_binding_with_namespaces(
            signed_properties.xml,
            &signed_properties.inherited_namespaces,
            &key_material,
        )?;
    }
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
    let signature = decode_p256_xmldsig_signature(&signature_value)?;
    if verifying_key
        .verify(canonical_signed_info.as_bytes(), &signature)
        .is_err()
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(true)
}

#[derive(Debug)]
struct XmlSignatureCarrier {
    carrier_span: XmlElementSpan,
    signature_span: XmlElementSpan,
    inherited_namespaces: Vec<CanonicalXmlNamespaceBinding>,
}

fn xml_signature_carrier(text: &str) -> Result<Option<XmlSignatureCarrier>, MsgError> {
    let signature = find_first_xml_element(text, "Signature");
    let sgntr = find_first_xml_element(text, "Sgntr");
    match (signature, sgntr) {
        (None, None) => Ok(None),
        (Some(signature_span), None) => {
            ensure_no_xml_signature_carrier_outside_span(text, signature_span)?;
            Ok(Some(XmlSignatureCarrier {
                carrier_span: signature_span,
                signature_span,
                inherited_namespaces: Vec::new(),
            }))
        }
        (_, Some(sgntr_span)) => {
            ensure_xml_element_attributes_allowed(text, sgntr_span, &[])?;
            ensure_direct_xml_child_elements(text, sgntr_span, &["Signature"])?;
            let signature_span = required_direct_xml_child_element(text, sgntr_span, "Signature")?;
            ensure_no_xml_signature_carrier_outside_span(text, sgntr_span)?;
            if find_xml_element_outside_span(text, signature_span, "Signature") {
                return Err(MsgError::ValidationFailed);
            }
            let inherited_namespaces = xml_element_namespace_scope(text, sgntr_span, &[])?;
            Ok(Some(XmlSignatureCarrier {
                carrier_span: sgntr_span,
                signature_span,
                inherited_namespaces,
            }))
        }
    }
}

fn ensure_no_xml_signature_carrier_outside_span(
    text: &str,
    allowed_span: XmlElementSpan,
) -> Result<(), MsgError> {
    for carrier in ["Signature", "Sgntr"] {
        if find_xml_element_outside_span(text, allowed_span, carrier) {
            return Err(MsgError::ValidationFailed);
        }
    }
    Ok(())
}

fn decode_p256_xmldsig_signature(signature_value: &[u8]) -> Result<P256Signature, MsgError> {
    let signature = if signature_value.len() == P256_XMLDSIG_SIGNATURE_LEN {
        P256Signature::from_slice(signature_value).map_err(|_| MsgError::ValidationFailed)
    } else {
        P256Signature::from_der(signature_value).map_err(|_| MsgError::ValidationFailed)
    }?;
    if signature.normalize_s().is_some() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(signature)
}

#[derive(Debug)]
struct XmlSignatureDirectChildSpans {
    signed_info: XmlElementSpan,
    signature_value: XmlElementSpan,
    key_info: XmlElementSpan,
}

fn xml_signature_direct_child_spans(
    signature_xml: &str,
) -> Result<XmlSignatureDirectChildSpans, MsgError> {
    xml_signature_direct_child_spans_with_namespaces(signature_xml, &[])
}

fn xml_signature_direct_child_spans_with_namespaces(
    signature_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<XmlSignatureDirectChildSpans, MsgError> {
    let signature_span = required_single_xml_element(signature_xml, "Signature")?;
    if signature_span.start != 0 || signature_span.end != signature_xml.len() {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xml_element_prefixed_namespace(
        signature_xml,
        signature_span,
        inherited_namespaces,
        XMLDSIG_NS,
    )?;
    let signature_namespaces =
        xml_element_namespace_scope(signature_xml, signature_span, inherited_namespaces)?;
    ensure_xml_element_attributes_allowed(signature_xml, signature_span, &["Id"])?;
    ensure_direct_xml_child_elements(
        signature_xml,
        signature_span,
        &["SignedInfo", "SignatureValue", "KeyInfo", "Object"],
    )?;
    let signed_info =
        required_direct_xml_child_element(signature_xml, signature_span, "SignedInfo")?;
    let signature_value =
        required_direct_xml_child_element(signature_xml, signature_span, "SignatureValue")?;
    let key_info = required_direct_xml_child_element(signature_xml, signature_span, "KeyInfo")?;
    let object = optional_direct_xml_child_element(signature_xml, signature_span, "Object")?;
    for child_span in [signed_info, signature_value, key_info] {
        ensure_xml_element_prefixed_namespace(
            signature_xml,
            child_span,
            &signature_namespaces,
            XMLDSIG_NS,
        )?;
    }
    if let Some(object) = object {
        ensure_xml_element_prefixed_namespace(
            signature_xml,
            object,
            &signature_namespaces,
            XMLDSIG_NS,
        )?;
    }
    if !(signed_info.start < signature_value.start && signature_value.start < key_info.start)
        || object.is_some_and(|object| key_info.start > object.start)
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(XmlSignatureDirectChildSpans {
        signed_info,
        signature_value,
        key_info,
    })
}

#[derive(Debug)]
struct XmlSignedInfoDirectChildSpans {
    canonicalization_method: XmlElementSpan,
    signature_method: XmlElementSpan,
}

fn xml_signed_info_direct_child_spans(
    signed_info_xml: &str,
) -> Result<XmlSignedInfoDirectChildSpans, MsgError> {
    xml_signed_info_direct_child_spans_with_namespaces(signed_info_xml, &[])
}

fn xml_signed_info_direct_child_spans_with_namespaces(
    signed_info_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<XmlSignedInfoDirectChildSpans, MsgError> {
    let signed_info_span = required_single_xml_element(signed_info_xml, "SignedInfo")?;
    if signed_info_span.start != 0 || signed_info_span.end != signed_info_xml.len() {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xml_element_prefixed_namespace(
        signed_info_xml,
        signed_info_span,
        inherited_namespaces,
        XMLDSIG_NS,
    )?;
    let signed_info_namespaces =
        xml_element_namespace_scope(signed_info_xml, signed_info_span, inherited_namespaces)?;
    ensure_direct_xml_child_elements_allowing_comments(
        signed_info_xml,
        signed_info_span,
        &["CanonicalizationMethod", "SignatureMethod", "Reference"],
    )?;
    let canonicalization_method = required_direct_xml_child_element_allowing_comments(
        signed_info_xml,
        signed_info_span,
        "CanonicalizationMethod",
    )?;
    let signature_method = required_direct_xml_child_element_allowing_comments(
        signed_info_xml,
        signed_info_span,
        "SignatureMethod",
    )?;
    ensure_xml_element_prefixed_namespace(
        signed_info_xml,
        canonicalization_method,
        &signed_info_namespaces,
        XMLDSIG_NS,
    )?;
    ensure_xml_element_prefixed_namespace(
        signed_info_xml,
        signature_method,
        &signed_info_namespaces,
        XMLDSIG_NS,
    )?;
    let first_reference =
        find_first_xml_element(signed_info_xml, "Reference").ok_or(MsgError::ValidationFailed)?;
    ensure_xml_element_prefixed_namespace(
        signed_info_xml,
        first_reference,
        &signed_info_namespaces,
        XMLDSIG_NS,
    )?;
    if !(canonicalization_method.start < signature_method.start
        && signature_method.start < first_reference.start)
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(XmlSignedInfoDirectChildSpans {
        canonicalization_method,
        signature_method,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanonicalXmlMode {
    Inclusive,
    Exclusive,
}

fn xml_canonicalization_mode(algorithm: &str) -> Result<CanonicalXmlMode, MsgError> {
    match algorithm {
        XML_C14N_1_0 | XML_C14N_1_1 => Ok(CanonicalXmlMode::Inclusive),
        XML_EXCLUSIVE_C14N_1_0 => Ok(CanonicalXmlMode::Exclusive),
        _ => Err(MsgError::ValidationFailed),
    }
}

struct XmlSignatureReferenceVerification<'a> {
    signed_properties: Option<VerifiedSignedProperties<'a>>,
}

struct VerifiedSignedProperties<'a> {
    xml: &'a str,
    inherited_namespaces: Vec<CanonicalXmlNamespaceBinding>,
}

fn verify_xml_signature_references<'a>(
    full_xml: &str,
    signature_span: XmlElementSpan,
    carrier_span: XmlElementSpan,
    signature_xml: &'a str,
    signed_info_xml: &str,
    signed_info_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<XmlSignatureReferenceVerification<'a>, MsgError> {
    let mut unsigned = String::with_capacity(full_xml.len());
    unsigned.push_str(&full_xml[..signature_span.start]);
    unsigned.push_str(&full_xml[signature_span.end..]);
    if unsigned.as_str() != unsigned.trim() {
        return Err(MsgError::ValidationFailed);
    }

    let mut cursor = 0usize;
    let mut payload_reference_seen = false;
    let mut signed_properties = None;
    let mut reference_count = 0usize;
    while cursor < signed_info_xml.len() {
        let Some(span) = find_first_xml_element(&signed_info_xml[cursor..], "Reference") else {
            break;
        };
        let reference_span = XmlElementSpan {
            start: cursor + span.start,
            opening_end: cursor + span.opening_end,
            content_start: cursor + span.content_start,
            content_end: cursor + span.content_end,
            end: cursor + span.end,
        };
        reference_count += 1;
        ensure_xml_element_prefixed_namespace(
            signed_info_xml,
            reference_span,
            signed_info_namespaces,
            XMLDSIG_NS,
        )?;
        let reference_xml = &signed_info_xml[reference_span.start..reference_span.end];
        let uri = element_attr(signed_info_xml, reference_span, "URI").unwrap_or_default();
        match element_attr(signed_info_xml, reference_span, "Type").as_deref() {
            None => {
                if payload_reference_seen {
                    return Err(MsgError::ValidationFailed);
                }
                payload_reference_seen = true;
                verify_xml_signature_payload_reference(
                    full_xml,
                    &unsigned,
                    carrier_span,
                    reference_xml,
                    &uri,
                    signed_info_namespaces,
                )?;
            }
            Some(XADES_SIGNED_PROPERTIES_TYPE) => {
                if signed_properties.is_some() {
                    return Err(MsgError::ValidationFailed);
                }
                signed_properties = Some(verify_xml_signature_signed_properties_reference(
                    signature_xml,
                    reference_xml,
                    &uri,
                    signed_info_namespaces,
                )?);
            }
            Some(_) => return Err(MsgError::ValidationFailed),
        }
        cursor = reference_span.end;
    }

    if reference_count == 0 || !payload_reference_seen {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xades_signed_properties_reference_coverage(
        signature_xml,
        signed_properties.as_ref().map(|properties| properties.xml),
    )?;
    if signed_properties.is_some() {
        ensure_xades_qualifying_properties_target(signature_xml)?;
    }
    Ok(XmlSignatureReferenceVerification { signed_properties })
}

fn verify_xml_signature_payload_reference(
    full_xml: &str,
    unsigned_xml: &str,
    carrier_span: XmlElementSpan,
    reference_xml: &str,
    uri: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    let c14n_mode = supported_xml_signature_reference_c14n_mode_with_namespaces(
        reference_xml,
        inherited_namespaces,
    )?;
    ensure_xml_signature_payload_reference_covers_carrier(full_xml, uri, carrier_span)?;
    let referenced_xml = xml_signature_reference_target(unsigned_xml, uri)?;
    let canonical_referenced_xml = canonicalize_supported_xml_with_mode(
        referenced_xml.xml,
        &referenced_xml.inherited_namespaces,
        c14n_mode,
    )?;
    verify_xml_signature_reference_digest_with_namespaces(
        reference_xml,
        &canonical_referenced_xml,
        inherited_namespaces,
    )
}

fn ensure_xml_signature_payload_reference_covers_carrier(
    full_xml: &str,
    uri: &str,
    carrier_span: XmlElementSpan,
) -> Result<(), MsgError> {
    if uri.is_empty() {
        return Ok(());
    }
    let reference_id = uri
        .strip_prefix('#')
        .filter(|reference_id| !reference_id.is_empty())
        .ok_or(MsgError::ValidationFailed)?;
    ensure_supported_same_document_reference_id(reference_id)?;
    let target = find_xml_element_by_reference_id(full_xml, reference_id)?;
    if target.span.start < carrier_span.start && carrier_span.end < target.span.end {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn verify_xml_signature_signed_properties_reference<'a>(
    signature_xml: &'a str,
    reference_xml: &str,
    uri: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<VerifiedSignedProperties<'a>, MsgError> {
    let c14n_mode =
        signed_properties_reference_c14n_mode_with_namespaces(reference_xml, inherited_namespaces)?;
    let referenced_xml = xml_signature_reference_target(signature_xml, uri)?;
    let signed_properties_namespaces =
        namespace_bindings_from_attributes(&referenced_xml.inherited_namespaces);
    ensure_signed_properties_reference_target_with_namespaces(
        referenced_xml.xml,
        &signed_properties_namespaces,
    )?;
    let canonical_referenced_xml = canonicalize_supported_xml_with_mode(
        referenced_xml.xml,
        &referenced_xml.inherited_namespaces,
        c14n_mode,
    )?;
    verify_xml_signature_reference_digest_with_namespaces(
        reference_xml,
        &canonical_referenced_xml,
        inherited_namespaces,
    )?;
    Ok(VerifiedSignedProperties {
        xml: referenced_xml.xml,
        inherited_namespaces: signed_properties_namespaces,
    })
}

fn verify_xml_signature_reference_digest(
    reference_xml: &str,
    canonical_referenced_xml: &str,
) -> Result<(), MsgError> {
    verify_xml_signature_reference_digest_with_namespaces(
        reference_xml,
        canonical_referenced_xml,
        &[],
    )
}

fn verify_xml_signature_reference_digest_with_namespaces(
    reference_xml: &str,
    canonical_referenced_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    let reference_span = required_single_xml_element(reference_xml, "Reference")?;
    let reference_namespaces =
        xml_element_namespace_scope(reference_xml, reference_span, inherited_namespaces)?;
    let digest_method_span = required_single_xml_element(reference_xml, "DigestMethod")?;
    ensure_xml_element_prefixed_namespace(
        reference_xml,
        digest_method_span,
        &reference_namespaces,
        XMLDSIG_NS,
    )?;
    ensure_xml_element_attributes_allowed(reference_xml, digest_method_span, &["Algorithm"])?;
    ensure_xml_element_content_empty(reference_xml, digest_method_span)?;
    let digest_algorithm = element_attr(reference_xml, digest_method_span, "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if digest_algorithm != XMLDSIG_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    let digest_value_span = required_single_xml_element(reference_xml, "DigestValue")?;
    ensure_xml_element_prefixed_namespace(
        reference_xml,
        digest_value_span,
        &reference_namespaces,
        XMLDSIG_NS,
    )?;
    let expected_digest = decode_direct_child_base64(reference_xml, digest_value_span)?;
    let digest = Sha256::digest(canonical_referenced_xml.as_bytes());
    if expected_digest.as_slice() != &digest[..] {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn supported_xml_signature_reference_c14n_mode(
    reference_xml: &str,
) -> Result<CanonicalXmlMode, MsgError> {
    supported_xml_signature_reference_c14n_mode_with_namespaces(reference_xml, &[])
}

fn supported_xml_signature_reference_c14n_mode_with_namespaces(
    reference_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<CanonicalXmlMode, MsgError> {
    ensure_xml_signature_reference_shape_with_namespaces(
        reference_xml,
        &["URI"],
        inherited_namespaces,
    )?;
    let transform_algorithms = xml_signature_reference_transform_algorithms_with_namespaces(
        reference_xml,
        inherited_namespaces,
    )?;

    match transform_algorithms.as_slice() {
        [transform] if transform == XMLDSIG_ENVELOPED_SIGNATURE => Ok(CanonicalXmlMode::Exclusive),
        [transform, c14n_algorithm] if transform == XMLDSIG_ENVELOPED_SIGNATURE => {
            xml_canonicalization_mode(c14n_algorithm)
        }
        _ => Err(MsgError::ValidationFailed),
    }
}

fn signed_properties_reference_c14n_mode(
    reference_xml: &str,
) -> Result<CanonicalXmlMode, MsgError> {
    signed_properties_reference_c14n_mode_with_namespaces(reference_xml, &[])
}

fn signed_properties_reference_c14n_mode_with_namespaces(
    reference_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<CanonicalXmlMode, MsgError> {
    ensure_xml_signature_reference_shape_with_namespaces(
        reference_xml,
        &["URI", "Type"],
        inherited_namespaces,
    )?;
    let transform_algorithms = xml_signature_reference_transform_algorithms_with_namespaces(
        reference_xml,
        inherited_namespaces,
    )?;
    let mut c14n_mode = None;
    for algorithm in transform_algorithms {
        let mode = xml_canonicalization_mode(&algorithm)?;
        if c14n_mode.replace(mode).is_some() {
            return Err(MsgError::ValidationFailed);
        }
    }
    c14n_mode.ok_or(MsgError::ValidationFailed)
}

fn ensure_xml_signature_reference_shape(
    reference_xml: &str,
    allowed_attributes: &[&str],
) -> Result<(), MsgError> {
    ensure_xml_signature_reference_shape_with_namespaces(reference_xml, allowed_attributes, &[])
}

fn ensure_xml_signature_reference_shape_with_namespaces(
    reference_xml: &str,
    allowed_attributes: &[&str],
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    let reference_span = required_single_xml_element(reference_xml, "Reference")?;
    if reference_span.start != 0 || reference_span.end != reference_xml.len() {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xml_element_prefixed_namespace(
        reference_xml,
        reference_span,
        inherited_namespaces,
        XMLDSIG_NS,
    )?;
    let reference_namespaces =
        xml_element_namespace_scope(reference_xml, reference_span, inherited_namespaces)?;
    ensure_xml_element_attributes_allowed(reference_xml, reference_span, allowed_attributes)?;
    ensure_direct_xml_child_elements_allowing_comments(
        reference_xml,
        reference_span,
        &["Transforms", "DigestMethod", "DigestValue"],
    )?;
    let transforms = required_direct_xml_child_element_allowing_comments(
        reference_xml,
        reference_span,
        "Transforms",
    )?;
    let digest_method = optional_direct_xml_child_element_allowing_comments(
        reference_xml,
        reference_span,
        "DigestMethod",
    )?;
    let digest_value = optional_direct_xml_child_element_allowing_comments(
        reference_xml,
        reference_span,
        "DigestValue",
    )?;
    ensure_xml_element_prefixed_namespace(
        reference_xml,
        transforms,
        &reference_namespaces,
        XMLDSIG_NS,
    )?;
    if let Some(digest_method) = digest_method {
        ensure_xml_element_prefixed_namespace(
            reference_xml,
            digest_method,
            &reference_namespaces,
            XMLDSIG_NS,
        )?;
    }
    if let Some(digest_value) = digest_value {
        ensure_xml_element_prefixed_namespace(
            reference_xml,
            digest_value,
            &reference_namespaces,
            XMLDSIG_NS,
        )?;
    }
    if digest_method.is_some_and(|digest_method| transforms.start > digest_method.start)
        || digest_value.is_some_and(|digest_value| transforms.start > digest_value.start)
        || digest_method
            .zip(digest_value)
            .is_some_and(|(digest_method, digest_value)| digest_method.start > digest_value.start)
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn xml_signature_reference_transform_algorithms(
    reference_xml: &str,
) -> Result<Vec<String>, MsgError> {
    xml_signature_reference_transform_algorithms_with_namespaces(reference_xml, &[])
}

fn xml_signature_reference_transform_algorithms_with_namespaces(
    reference_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<Vec<String>, MsgError> {
    let reference_span = required_single_xml_element(reference_xml, "Reference")?;
    let reference_namespaces =
        xml_element_namespace_scope(reference_xml, reference_span, inherited_namespaces)?;
    let transforms_span = required_single_xml_element(reference_xml, "Transforms")?;
    ensure_xml_element_prefixed_namespace(
        reference_xml,
        transforms_span,
        &reference_namespaces,
        XMLDSIG_NS,
    )?;
    let transforms_namespaces =
        xml_element_namespace_scope(reference_xml, transforms_span, &reference_namespaces)?;
    ensure_xml_element_attributes_allowed(reference_xml, transforms_span, &[])?;
    ensure_direct_xml_child_elements_allowing_comments(
        reference_xml,
        transforms_span,
        &["Transform"],
    )?;
    if find_first_xml_element(&reference_xml[..transforms_span.start], "Transform").is_some()
        || find_first_xml_element(&reference_xml[transforms_span.end..], "Transform").is_some()
    {
        return Err(MsgError::ValidationFailed);
    }

    let transforms_xml = &reference_xml[transforms_span.content_start..transforms_span.content_end];
    let mut cursor = 0usize;
    let mut transform_algorithms = Vec::new();
    while cursor < transforms_xml.len() {
        let Some(span) = find_first_xml_element(&transforms_xml[cursor..], "Transform") else {
            break;
        };
        let absolute = XmlElementSpan {
            start: cursor + span.start,
            opening_end: cursor + span.opening_end,
            content_start: cursor + span.content_start,
            content_end: cursor + span.content_end,
            end: cursor + span.end,
        };
        ensure_xml_element_prefixed_namespace(
            transforms_xml,
            absolute,
            &transforms_namespaces,
            XMLDSIG_NS,
        )?;
        ensure_xml_element_attributes_allowed(transforms_xml, absolute, &["Algorithm"])?;
        ensure_xml_element_content_empty(transforms_xml, absolute)?;
        transform_algorithms.push(
            element_attr(transforms_xml, absolute, "Algorithm")
                .ok_or(MsgError::ValidationFailed)?,
        );
        cursor = absolute.end;
    }
    Ok(transform_algorithms)
}

fn ensure_signed_properties_reference_target(target_xml: &str) -> Result<(), MsgError> {
    ensure_signed_properties_reference_target_with_namespaces(target_xml, &[])
}

fn ensure_signed_properties_reference_target_with_namespaces(
    target_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    let span =
        find_first_xml_element(target_xml, "SignedProperties").ok_or(MsgError::ValidationFailed)?;
    if span.start == 0 && span.end == target_xml.len() {
        ensure_xades_element_prefixed_namespace(target_xml, span, inherited_namespaces)?;
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn ensure_xades_signed_properties_reference_coverage(
    signature_xml: &str,
    referenced_signed_properties_xml: Option<&str>,
) -> Result<(), MsgError> {
    let Some(signed_properties_span) = find_first_xml_element(signature_xml, "SignedProperties")
    else {
        return if referenced_signed_properties_xml.is_none() {
            Ok(())
        } else {
            Err(MsgError::ValidationFailed)
        };
    };
    let signed_properties_xml =
        &signature_xml[signed_properties_span.start..signed_properties_span.end];
    if Some(signed_properties_xml) != referenced_signed_properties_xml {
        return Err(MsgError::ValidationFailed);
    }
    if find_first_xml_element(
        &signature_xml[signed_properties_span.end..],
        "SignedProperties",
    )
    .is_some()
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn ensure_xades_qualifying_properties_target(signature_xml: &str) -> Result<(), MsgError> {
    let signature_span =
        find_first_xml_element(signature_xml, "Signature").ok_or(MsgError::ValidationFailed)?;
    if signature_span.start != 0 || signature_span.end != signature_xml.len() {
        return Err(MsgError::ValidationFailed);
    }
    let signature_namespaces = xml_element_namespace_scope(signature_xml, signature_span, &[])?;
    let signature_id = element_attr(signature_xml, signature_span, "Id")
        .filter(|id| !id.is_empty())
        .ok_or(MsgError::ValidationFailed)?;
    let object_span = required_direct_xml_child_element(signature_xml, signature_span, "Object")?;
    let object_namespaces =
        xml_element_namespace_scope(signature_xml, object_span, &signature_namespaces)?;
    ensure_xml_element_attributes_allowed(signature_xml, object_span, &[])?;
    ensure_direct_xml_child_elements(signature_xml, object_span, &["QualifyingProperties"])?;
    let qualifying_properties_span =
        required_direct_xml_child_element(signature_xml, object_span, "QualifyingProperties")?;
    ensure_xades_element_prefixed_namespace(
        signature_xml,
        qualifying_properties_span,
        &object_namespaces,
    )?;
    let qualifying_properties_namespaces = xml_element_namespace_scope(
        signature_xml,
        qualifying_properties_span,
        &object_namespaces,
    )?;
    if find_xml_element_outside_span(signature_xml, object_span, "QualifyingProperties") {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xml_element_attributes_allowed(signature_xml, qualifying_properties_span, &["Target"])?;
    ensure_direct_xml_child_elements(
        signature_xml,
        qualifying_properties_span,
        &["SignedProperties"],
    )?;
    required_direct_xml_child_element(
        signature_xml,
        qualifying_properties_span,
        "SignedProperties",
    )
    .and_then(|signed_properties_span| {
        ensure_xades_element_prefixed_namespace(
            signature_xml,
            signed_properties_span,
            &qualifying_properties_namespaces,
        )
    })?;
    let target = element_attr(signature_xml, qualifying_properties_span, "Target")
        .ok_or(MsgError::ValidationFailed)?;
    if target == format!("#{signature_id}") {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn verify_xades_signing_certificate_v2_binding(
    signed_properties_xml: &str,
    key_material: &XmlSignatureKeyMaterial,
) -> Result<(), MsgError> {
    verify_xades_signing_certificate_v2_binding_with_namespaces(
        signed_properties_xml,
        &[],
        key_material,
    )
}

fn verify_xades_signing_certificate_v2_binding_with_namespaces(
    signed_properties_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
    key_material: &XmlSignatureKeyMaterial,
) -> Result<(), MsgError> {
    let signed_signature_properties = xades_signed_signature_properties_with_namespaces(
        signed_properties_xml,
        inherited_namespaces,
    )?;
    let signed_signature_properties_xml = signed_signature_properties.xml;
    let signed_signature_properties_span =
        required_single_xml_element(signed_signature_properties_xml, "SignedSignatureProperties")?;
    let Some(signing_certificate_span) = optional_direct_xml_child_element(
        signed_signature_properties_xml,
        signed_signature_properties_span,
        "SigningCertificateV2",
    )?
    else {
        if key_material.certificate_sha256.is_empty() {
            return Ok(());
        }
        return Err(MsgError::ValidationFailed);
    };
    if key_material.certificate_sha256.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xades_element_prefixed_namespace(
        signed_signature_properties_xml,
        signing_certificate_span,
        &signed_signature_properties.namespaces,
    )?;
    let signing_certificate_xml = &signed_signature_properties_xml
        [signing_certificate_span.start..signing_certificate_span.end];
    let certificate_digests = xades_signing_certificate_v2_digests_with_namespaces(
        signing_certificate_xml,
        &signed_signature_properties.namespaces,
    )?;
    ensure_xades_signing_certificate_v2_chain_prefix(
        &certificate_digests,
        &key_material.certificate_sha256,
    )
}

fn ensure_xades_signing_certificate_v2_chain_prefix(
    certificate_digests: &[String],
    verified_certificate_sha256: &[String],
) -> Result<(), MsgError> {
    if certificate_digests.is_empty()
        || certificate_digests.len() > verified_certificate_sha256.len()
    {
        return Err(MsgError::ValidationFailed);
    }
    let mut seen = HashSet::new();
    for (index, digest) in certificate_digests.iter().enumerate() {
        if !seen.insert(digest.as_str()) {
            return Err(MsgError::ValidationFailed);
        }
        match verified_certificate_sha256.get(index) {
            Some(expected) if expected == digest => {}
            _ => return Err(MsgError::ValidationFailed),
        }
    }
    Ok(())
}

fn xades_signed_signature_properties_xml(signed_properties_xml: &str) -> Result<&str, MsgError> {
    Ok(xades_signed_signature_properties_with_namespaces(signed_properties_xml, &[])?.xml)
}

struct XadesSignedSignatureProperties<'a> {
    xml: &'a str,
    namespaces: Vec<CanonicalXmlNamespaceBinding>,
}

fn xades_signed_signature_properties_with_namespaces<'a>(
    signed_properties_xml: &'a str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<XadesSignedSignatureProperties<'a>, MsgError> {
    let signed_properties_span =
        required_single_xml_element(signed_properties_xml, "SignedProperties")?;
    if signed_properties_span.start != 0
        || signed_properties_span.end != signed_properties_xml.len()
    {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xades_element_prefixed_namespace(
        signed_properties_xml,
        signed_properties_span,
        inherited_namespaces,
    )?;
    let signed_properties_namespaces = xml_element_namespace_scope(
        signed_properties_xml,
        signed_properties_span,
        inherited_namespaces,
    )?;
    ensure_xml_element_attributes_allowed(signed_properties_xml, signed_properties_span, &["Id"])?;
    ensure_direct_xml_child_elements(
        signed_properties_xml,
        signed_properties_span,
        &["SignedSignatureProperties"],
    )?;
    let signed_signature_properties_span = optional_direct_xml_child_element(
        signed_properties_xml,
        signed_properties_span,
        "SignedSignatureProperties",
    )?
    .ok_or(MsgError::ValidationFailed)?;
    ensure_xades_element_prefixed_namespace(
        signed_properties_xml,
        signed_signature_properties_span,
        &signed_properties_namespaces,
    )?;
    let signed_signature_properties_namespaces = xml_element_namespace_scope(
        signed_properties_xml,
        signed_signature_properties_span,
        &signed_properties_namespaces,
    )?;
    ensure_xml_element_attributes_allowed(
        signed_properties_xml,
        signed_signature_properties_span,
        &[],
    )?;
    ensure_direct_xml_child_elements(
        signed_properties_xml,
        signed_signature_properties_span,
        &["SigningTime", "SigningCertificateV2"],
    )?;
    for xades_child in ["SigningTime", "SigningCertificateV2"] {
        if let Some(child_span) = optional_direct_xml_child_element(
            signed_properties_xml,
            signed_signature_properties_span,
            xades_child,
        )? {
            ensure_xades_element_prefixed_namespace(
                signed_properties_xml,
                child_span,
                &signed_signature_properties_namespaces,
            )?;
        }
    }
    Ok(XadesSignedSignatureProperties {
        xml: &signed_properties_xml
            [signed_signature_properties_span.start..signed_signature_properties_span.end],
        namespaces: signed_signature_properties_namespaces,
    })
}

fn xades_signing_certificate_v2_digests(
    signing_certificate_xml: &str,
) -> Result<Vec<String>, MsgError> {
    xades_signing_certificate_v2_digests_with_namespaces(signing_certificate_xml, &[])
}

fn xades_signing_certificate_v2_digests_with_namespaces(
    signing_certificate_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<Vec<String>, MsgError> {
    let signing_certificate_span =
        required_single_xml_element(signing_certificate_xml, "SigningCertificateV2")?;
    if signing_certificate_span.start != 0
        || signing_certificate_span.end != signing_certificate_xml.len()
    {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xades_element_prefixed_namespace(
        signing_certificate_xml,
        signing_certificate_span,
        inherited_namespaces,
    )?;
    let signing_certificate_namespaces = xml_element_namespace_scope(
        signing_certificate_xml,
        signing_certificate_span,
        inherited_namespaces,
    )?;
    ensure_xml_element_attributes_allowed(signing_certificate_xml, signing_certificate_span, &[])?;
    ensure_direct_xml_child_elements(signing_certificate_xml, signing_certificate_span, &["Cert"])?;

    let mut cursor = 0usize;
    let mut digests = Vec::new();
    while cursor < signing_certificate_xml.len() {
        let Some(span) = find_first_xml_element(&signing_certificate_xml[cursor..], "Cert") else {
            break;
        };
        let cert_span = XmlElementSpan {
            start: cursor + span.start,
            opening_end: cursor + span.opening_end,
            content_start: cursor + span.content_start,
            content_end: cursor + span.content_end,
            end: cursor + span.end,
        };
        ensure_xades_element_prefixed_namespace(
            signing_certificate_xml,
            cert_span,
            &signing_certificate_namespaces,
        )?;
        let cert_xml = &signing_certificate_xml[cert_span.start..cert_span.end];
        digests.push(xades_cert_digest_sha256_with_namespaces(
            cert_xml,
            &signing_certificate_namespaces,
        )?);
        cursor = cert_span.end;
    }
    Ok(digests)
}

fn xades_cert_digest_sha256(cert_xml: &str) -> Result<String, MsgError> {
    xades_cert_digest_sha256_with_namespaces(cert_xml, &[])
}

fn xades_cert_digest_sha256_with_namespaces(
    cert_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<String, MsgError> {
    let cert_span = required_single_xml_element(cert_xml, "Cert")?;
    if cert_span.start != 0 || cert_span.end != cert_xml.len() {
        return Err(MsgError::ValidationFailed);
    }
    ensure_xades_element_prefixed_namespace(cert_xml, cert_span, inherited_namespaces)?;
    let cert_namespaces = xml_element_namespace_scope(cert_xml, cert_span, inherited_namespaces)?;
    ensure_xml_element_attributes_allowed(cert_xml, cert_span, &[])?;
    ensure_direct_xml_child_elements(cert_xml, cert_span, &["CertDigest"])?;

    let cert_digest_span = required_single_xml_element(cert_xml, "CertDigest")?;
    ensure_xades_element_prefixed_namespace(cert_xml, cert_digest_span, &cert_namespaces)?;
    let cert_digest_namespaces =
        xml_element_namespace_scope(cert_xml, cert_digest_span, &cert_namespaces)?;
    ensure_xml_element_attributes_allowed(cert_xml, cert_digest_span, &[])?;
    ensure_direct_xml_child_elements(cert_xml, cert_digest_span, &["DigestMethod", "DigestValue"])?;
    let cert_digest_xml = &cert_xml[cert_digest_span.start..cert_digest_span.end];
    let digest_method_span = required_single_xml_element(cert_digest_xml, "DigestMethod")?;
    ensure_xml_element_prefixed_namespace(
        cert_digest_xml,
        digest_method_span,
        &cert_digest_namespaces,
        XMLDSIG_NS,
    )?;
    ensure_xml_element_attributes_allowed(cert_digest_xml, digest_method_span, &["Algorithm"])?;
    ensure_xml_element_content_empty(cert_digest_xml, digest_method_span)?;
    let digest_algorithm = element_attr(cert_digest_xml, digest_method_span, "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if digest_algorithm != XMLDSIG_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    let digest_value_span = required_single_xml_element(cert_digest_xml, "DigestValue")?;
    ensure_xml_element_prefixed_namespace(
        cert_digest_xml,
        digest_value_span,
        &cert_digest_namespaces,
        XMLDSIG_NS,
    )?;
    let digest = decode_direct_child_base64(cert_digest_xml, digest_value_span)?;
    if digest.len() != 32 {
        return Err(MsgError::ValidationFailed);
    }
    Ok(lower_hex(&digest))
}

#[derive(Debug)]
struct XmlSignatureReferenceTarget<'a> {
    xml: &'a str,
    inherited_namespaces: Vec<CanonicalXmlAttribute>,
}

fn xml_signature_reference_target<'a>(
    unsigned_xml: &'a str,
    uri: &str,
) -> Result<XmlSignatureReferenceTarget<'a>, MsgError> {
    if uri.is_empty() {
        return Ok(XmlSignatureReferenceTarget {
            xml: unsigned_xml,
            inherited_namespaces: Vec::new(),
        });
    }
    let reference_id = uri
        .strip_prefix('#')
        .filter(|reference_id| !reference_id.is_empty())
        .ok_or(MsgError::ValidationFailed)?;
    ensure_supported_same_document_reference_id(reference_id)?;
    let target = find_xml_element_by_reference_id(unsigned_xml, reference_id)?;
    Ok(XmlSignatureReferenceTarget {
        xml: &unsigned_xml[target.span.start..target.span.end],
        inherited_namespaces: target.inherited_namespaces,
    })
}

fn ensure_supported_same_document_reference_id(reference_id: &str) -> Result<(), MsgError> {
    ensure_supported_xml_name(reference_id)?;
    if reference_id.contains(':') {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

#[derive(Debug)]
struct XmlReferenceTarget {
    span: XmlElementSpan,
    inherited_namespaces: Vec<CanonicalXmlAttribute>,
}

fn find_xml_element_by_reference_id(
    text: &str,
    reference_id: &str,
) -> Result<XmlReferenceTarget, MsgError> {
    let mut cursor = 0usize;
    let mut found = None;
    let mut namespace_scope_lengths = Vec::new();
    let mut in_scope_namespaces = Vec::new();
    while cursor < text.len() {
        let Some(start_offset) = text[cursor..].find('<') else {
            break;
        };
        let start = cursor + start_offset;
        let tag_start = start + 1;
        let opening_end =
            find_xml_tag_end(text.as_bytes(), tag_start).ok_or(MsgError::ValidationFailed)?;
        let raw_tag = text[tag_start..opening_end].trim();
        if raw_tag.starts_with('/')
            || raw_tag.starts_with('?')
            || raw_tag.starts_with("!--")
            || raw_tag.starts_with("![CDATA[")
        {
            if raw_tag.starts_with('/') {
                let namespace_scope_len = namespace_scope_lengths
                    .pop()
                    .ok_or(MsgError::ValidationFailed)?;
                in_scope_namespaces.truncate(namespace_scope_len);
            }
            cursor = opening_end + 1;
            continue;
        }
        let self_closing = raw_tag.ends_with('/');
        let tag_body = raw_tag.trim_end_matches('/').trim_end();
        let (name, attributes) = split_supported_xml_tag(tag_body)?;
        ensure_supported_xml_name(name)?;
        let mut attributes = parse_supported_xml_attributes(attributes)?;
        sort_and_validate_canonical_xml_attributes(&mut attributes, &in_scope_namespaces)?;
        if same_document_reference_id_matches(raw_tag, reference_id) {
            if found.is_some() {
                return Err(MsgError::ValidationFailed);
            }
            let inherited_namespaces =
                inherited_namespace_attributes_from_scope(&in_scope_namespaces);
            let span = if self_closing {
                XmlElementSpan {
                    start,
                    opening_end,
                    content_start: opening_end + 1,
                    content_end: opening_end + 1,
                    end: opening_end + 1,
                }
            } else {
                let (content_end, end) = find_xml_element_end(text, opening_end + 1, name)
                    .ok_or(MsgError::ValidationFailed)?;
                XmlElementSpan {
                    start,
                    opening_end,
                    content_start: opening_end + 1,
                    content_end,
                    end,
                }
            };
            found = Some(XmlReferenceTarget {
                span,
                inherited_namespaces,
            });
        }
        if !self_closing {
            namespace_scope_lengths.push(in_scope_namespaces.len());
            in_scope_namespaces.extend(
                attributes
                    .iter()
                    .filter_map(namespace_binding_from_attribute),
            );
        }
        cursor = opening_end + 1;
    }
    found.ok_or(MsgError::ValidationFailed)
}

fn same_document_reference_id_matches(opening: &str, reference_id: &str) -> bool {
    ["Id", "ID", "id", "xml:id"]
        .iter()
        .any(|attr| attr_value_exact(opening, attr).as_deref() == Some(reference_id))
}

#[derive(Debug)]
struct XmlSignatureKeyMaterial {
    public_key: Vec<u8>,
    certificate_sha256: Vec<String>,
}

fn xml_signature_key_material(
    signature_xml: &str,
    evaluation_time: Option<ASN1Time>,
) -> Result<XmlSignatureKeyMaterial, MsgError> {
    xml_signature_key_material_with_namespaces(signature_xml, evaluation_time, &[])
}

fn xml_signature_key_material_with_namespaces(
    signature_xml: &str,
    evaluation_time: Option<ASN1Time>,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<XmlSignatureKeyMaterial, MsgError> {
    let key_info_span = required_single_xml_element(signature_xml, "KeyInfo")?;
    ensure_xml_element_prefixed_namespace(
        signature_xml,
        key_info_span,
        inherited_namespaces,
        XMLDSIG_NS,
    )?;
    let key_info_namespaces =
        xml_element_namespace_scope(signature_xml, key_info_span, inherited_namespaces)?;
    for key_material_element in ["PublicKey", "X509Certificate"] {
        if find_xml_element_outside_span(signature_xml, key_info_span, key_material_element) {
            return Err(MsgError::ValidationFailed);
        }
    }
    let key_info_xml = &signature_xml[key_info_span.start..key_info_span.end];
    let public_key = optional_single_child_text_compact(key_info_xml, "PublicKey")?;
    let has_certificate = find_first_xml_element(key_info_xml, "X509Certificate").is_some();
    if public_key.is_some() && has_certificate {
        return Err(MsgError::ValidationFailed);
    }
    if let Some(public_key) = public_key {
        ensure_xml_signature_public_key_info_shape_with_namespaces(
            key_info_xml,
            &key_info_namespaces,
        )?;
        let public_key = BASE64_STANDARD
            .decode(public_key)
            .map_err(|_| MsgError::ValidationFailed)?;
        ensure_xml_signature_p256_public_key(&public_key)?;
        return Ok(XmlSignatureKeyMaterial {
            public_key,
            certificate_sha256: Vec::new(),
        });
    }
    let certificates =
        xml_signature_x509_certificates_with_namespaces(key_info_xml, &key_info_namespaces)?;
    ensure_xml_signature_certificate_chain_bounds(&certificates)?;
    let evaluation_time = evaluation_time.ok_or(MsgError::ValidationFailed)?;
    let parsed_certificates = certificates
        .iter()
        .map(|certificate| {
            X509Certificate::from_der(certificate)
                .map(|(_, cert)| cert)
                .map_err(|_| MsgError::ValidationFailed)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for certificate in &parsed_certificates {
        ensure_xml_signature_supported_certificate_algorithm(certificate)?;
        ensure_xml_signature_supported_critical_extensions(certificate)?;
    }
    let public_key = parsed_certificates[0]
        .public_key()
        .subject_public_key
        .data
        .to_vec();
    ensure_xml_signature_certificate_valid_at(&parsed_certificates[0], evaluation_time)?;
    ensure_xml_signature_leaf_certificate_policy(&parsed_certificates[0])?;
    let mut certificate_sha256 = vec![sha256_hex(&certificates[0])];
    for (subordinate_ca_count, (certificates_der, chain_pair)) in certificates[1..]
        .iter()
        .zip(parsed_certificates.windows(2))
        .enumerate()
    {
        ensure_xml_signature_certificate_valid_at(&chain_pair[1], evaluation_time)?;
        ensure_xml_signature_issuer_certificate_policy(&chain_pair[1])?;
        ensure_xml_signature_issuer_path_len(&chain_pair[1], subordinate_ca_count)?;
        ensure_xml_signature_certificate_issued_by(&chain_pair[0], &chain_pair[1])?;
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

fn ensure_xml_signature_certificate_chain_bounds(certificates: &[Vec<u8>]) -> Result<(), MsgError> {
    if certificates.is_empty() || certificates.len() > XML_SIGNATURE_MAX_X509_CERTIFICATES {
        return Err(MsgError::ValidationFailed);
    }
    let mut seen = HashSet::new();
    for certificate in certificates {
        if !seen.insert(sha256_hex(certificate)) {
            return Err(MsgError::ValidationFailed);
        }
    }
    Ok(())
}

fn ensure_xml_signature_public_key_info_shape(key_info_xml: &str) -> Result<(), MsgError> {
    ensure_xml_signature_public_key_info_shape_with_namespaces(key_info_xml, &[])
}

fn ensure_xml_signature_p256_public_key(public_key: &[u8]) -> Result<(), MsgError> {
    if public_key.len() != P256_UNCOMPRESSED_SEC1_PUBLIC_KEY_LEN
        || public_key.first().copied() != Some(0x04)
    {
        return Err(MsgError::ValidationFailed);
    }
    P256VerifyingKey::from_sec1_bytes(public_key)
        .map(|_| ())
        .map_err(|_| MsgError::ValidationFailed)
}

fn ensure_xml_signature_public_key_info_shape_with_namespaces(
    key_info_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    if find_first_xml_element(key_info_xml, "X509Data").is_some() {
        return Err(MsgError::ValidationFailed);
    }
    let key_info_span = required_single_xml_element(key_info_xml, "KeyInfo")?;
    ensure_xml_element_prefixed_namespace(
        key_info_xml,
        key_info_span,
        inherited_namespaces,
        XMLDSIG_NS,
    )?;
    let key_info_namespaces =
        xml_element_namespace_scope(key_info_xml, key_info_span, inherited_namespaces)?;
    ensure_xml_element_attributes_allowed(key_info_xml, key_info_span, &[])?;
    ensure_direct_xml_child_elements(key_info_xml, key_info_span, &["KeyValue"])?;
    let key_value_span = required_single_xml_element(key_info_xml, "KeyValue")?;
    ensure_xml_element_prefixed_namespace(
        key_info_xml,
        key_value_span,
        &key_info_namespaces,
        XMLDSIG_NS,
    )?;
    let key_value_namespaces =
        xml_element_namespace_scope(key_info_xml, key_value_span, &key_info_namespaces)?;
    ensure_xml_element_attributes_allowed(key_info_xml, key_value_span, &[])?;
    ensure_direct_xml_child_elements(key_info_xml, key_value_span, &["ECKeyValue"])?;
    for key_value_element in ["ECKeyValue", "NamedCurve", "PublicKey"] {
        if find_xml_element_outside_span(key_info_xml, key_value_span, key_value_element) {
            return Err(MsgError::ValidationFailed);
        }
    }
    let key_value_xml = &key_info_xml[key_value_span.start..key_value_span.end];
    let ec_key_value_span = required_single_xml_element(key_value_xml, "ECKeyValue")?;
    ensure_xml_element_prefixed_namespace(
        key_value_xml,
        ec_key_value_span,
        &key_value_namespaces,
        XMLDSIG_NS,
    )?;
    let ec_key_value_namespaces =
        xml_element_namespace_scope(key_value_xml, ec_key_value_span, &key_value_namespaces)?;
    ensure_xml_element_attributes_allowed(key_value_xml, ec_key_value_span, &[])?;
    ensure_direct_xml_child_elements(
        key_value_xml,
        ec_key_value_span,
        &["NamedCurve", "PublicKey"],
    )?;
    for ec_key_value_element in ["NamedCurve", "PublicKey"] {
        if find_xml_element_outside_span(key_value_xml, ec_key_value_span, ec_key_value_element) {
            return Err(MsgError::ValidationFailed);
        }
    }
    let ec_key_value_xml = &key_value_xml[ec_key_value_span.start..ec_key_value_span.end];
    let named_curve_span = required_single_xml_element(ec_key_value_xml, "NamedCurve")?;
    ensure_xml_element_prefixed_namespace(
        ec_key_value_xml,
        named_curve_span,
        &ec_key_value_namespaces,
        XMLDSIG_NS,
    )?;
    ensure_xml_element_attributes_allowed(ec_key_value_xml, named_curve_span, &["URI"])?;
    ensure_xml_element_content_empty(ec_key_value_xml, named_curve_span)?;
    if element_attr(ec_key_value_xml, named_curve_span, "URI").as_deref()
        != Some(XMLDSIG_P256_NAMED_CURVE)
    {
        return Err(MsgError::ValidationFailed);
    }
    let public_key_span = required_single_xml_element(ec_key_value_xml, "PublicKey")?;
    ensure_xml_element_prefixed_namespace(
        ec_key_value_xml,
        public_key_span,
        &ec_key_value_namespaces,
        XMLDSIG_NS,
    )?;
    ensure_xml_element_attributes_allowed(ec_key_value_xml, public_key_span, &[])?;
    ensure_xml_element_text_only(ec_key_value_xml, public_key_span)?;
    required_single_child_text_compact(ec_key_value_xml, "PublicKey")?;
    Ok(())
}

fn xml_signature_x509_certificates(key_info_xml: &str) -> Result<Vec<Vec<u8>>, MsgError> {
    xml_signature_x509_certificates_with_namespaces(key_info_xml, &[])
}

fn xml_signature_x509_certificates_with_namespaces(
    key_info_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<Vec<Vec<u8>>, MsgError> {
    if find_first_xml_element(key_info_xml, "KeyValue").is_some()
        || find_first_xml_element(key_info_xml, "PublicKey").is_some()
    {
        return Err(MsgError::ValidationFailed);
    }
    let key_info_span = required_single_xml_element(key_info_xml, "KeyInfo")?;
    ensure_xml_element_prefixed_namespace(
        key_info_xml,
        key_info_span,
        inherited_namespaces,
        XMLDSIG_NS,
    )?;
    let key_info_namespaces =
        xml_element_namespace_scope(key_info_xml, key_info_span, inherited_namespaces)?;
    ensure_xml_element_attributes_allowed(key_info_xml, key_info_span, &[])?;
    ensure_direct_xml_child_elements(key_info_xml, key_info_span, &["X509Data"])?;
    let x509_data_span = required_single_xml_element(key_info_xml, "X509Data")?;
    ensure_xml_element_prefixed_namespace(
        key_info_xml,
        x509_data_span,
        &key_info_namespaces,
        XMLDSIG_NS,
    )?;
    let x509_data_namespaces =
        xml_element_namespace_scope(key_info_xml, x509_data_span, &key_info_namespaces)?;
    ensure_xml_element_attributes_allowed(key_info_xml, x509_data_span, &[])?;
    ensure_direct_xml_child_elements(key_info_xml, x509_data_span, &["X509Certificate"])?;
    if find_xml_element_outside_span(key_info_xml, x509_data_span, "X509Certificate") {
        return Err(MsgError::ValidationFailed);
    }
    let x509_data_xml = &key_info_xml[x509_data_span.start..x509_data_span.end];
    decode_child_base64_values_with_namespaces(
        x509_data_xml,
        "X509Certificate",
        &x509_data_namespaces,
    )
}

fn find_xml_element_outside_span(container: &str, allowed: XmlElementSpan, local: &str) -> bool {
    find_first_xml_element(&container[..allowed.start], local).is_some()
        || find_first_xml_element(&container[allowed.end..], local).is_some()
}

fn decode_required_child_base64(container: &str, child: &str) -> Result<Vec<u8>, MsgError> {
    let span = required_single_xml_element(container, child)?;
    ensure_xml_element_attributes_allowed(container, span, &[])?;
    ensure_xml_element_text_only(container, span)?;
    let value: String = container[span.content_start..span.content_end]
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    BASE64_STANDARD
        .decode(value)
        .map_err(|_| MsgError::ValidationFailed)
}

fn decode_child_base64_values(container: &str, child: &str) -> Result<Vec<Vec<u8>>, MsgError> {
    decode_child_base64_values_with_namespaces(container, child, &[])
}

fn decode_child_base64_values_with_namespaces(
    container: &str,
    child: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<Vec<Vec<u8>>, MsgError> {
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
        ensure_xml_element_prefixed_namespace(
            container,
            absolute,
            inherited_namespaces,
            XMLDSIG_NS,
        )?;
        ensure_xml_element_attributes_allowed(container, absolute, &[])?;
        ensure_xml_element_text_only(container, absolute)?;
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

fn xml_signature_evaluation_time(
    parsed: &ParsedMessage,
    signed_properties_xml: Option<&str>,
) -> Result<Option<ASN1Time>, MsgError> {
    let signed_properties = signed_properties_xml.map(|xml| VerifiedSignedProperties {
        xml,
        inherited_namespaces: Vec::new(),
    });
    xml_signature_evaluation_time_for_verified_properties(parsed, signed_properties.as_ref())
}

fn xml_signature_evaluation_time_for_verified_properties(
    parsed: &ParsedMessage,
    signed_properties: Option<&VerifiedSignedProperties<'_>>,
) -> Result<Option<ASN1Time>, MsgError> {
    let signed_properties_time = signed_properties
        .map(|properties| {
            xades_signed_properties_signing_time_with_namespaces(
                properties.xml,
                &properties.inherited_namespaces,
            )
        })
        .transpose()?
        .flatten();
    signed_properties_time
        .or_else(|| app_header_creation_date(parsed).map(ToOwned::to_owned))
        .map(|value| parse_iso_datetime_as_asn1_time(&value))
        .transpose()
}

fn xades_signed_properties_signing_time(
    signed_properties_xml: &str,
) -> Result<Option<String>, MsgError> {
    xades_signed_properties_signing_time_with_namespaces(signed_properties_xml, &[])
}

fn xades_signed_properties_signing_time_with_namespaces(
    signed_properties_xml: &str,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<Option<String>, MsgError> {
    let signed_signature_properties = xades_signed_signature_properties_with_namespaces(
        signed_properties_xml,
        inherited_namespaces,
    )?;
    let signed_signature_properties_xml = signed_signature_properties.xml;
    let signed_signature_properties_span =
        required_single_xml_element(signed_signature_properties_xml, "SignedSignatureProperties")?;
    let Some(signing_time) = optional_direct_xml_child_element(
        signed_signature_properties_xml,
        signed_signature_properties_span,
        "SigningTime",
    )?
    else {
        return Ok(None);
    };
    ensure_xades_element_prefixed_namespace(
        signed_signature_properties_xml,
        signing_time,
        &signed_signature_properties.namespaces,
    )?;
    ensure_xml_element_attributes_allowed(signed_signature_properties_xml, signing_time, &[])?;
    ensure_xml_element_text_only(signed_signature_properties_xml, signing_time)?;
    Ok(Some(
        signed_signature_properties_xml[signing_time.content_start..signing_time.content_end]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect(),
    ))
}

fn parse_iso_datetime_as_asn1_time(value: &str) -> Result<ASN1Time, MsgError> {
    let value = value.trim();
    let (date_part, rest) = value.split_once('T').ok_or(MsgError::ValidationFailed)?;
    let date_bytes = date_part.as_bytes();
    if date_bytes.len() != 10 || date_bytes[4] != b'-' || date_bytes[7] != b'-' {
        return Err(MsgError::ValidationFailed);
    }
    let year: i32 = parse_ascii_decimal(&date_bytes[0..4])?;
    let month: u8 = parse_ascii_decimal(&date_bytes[5..7])?;
    let day: u8 = parse_ascii_decimal(&date_bytes[8..10])?;

    let (time_part, offset) = if let Some(time_part) = rest.strip_suffix('Z') {
        (time_part, UtcOffset::UTC)
    } else if let Some(offset_idx) = rest.rfind(['+', '-']) {
        let offset_literal = &rest[offset_idx..];
        let offset_bytes = offset_literal.as_bytes();
        if offset_bytes.len() != 6
            || !matches!(offset_bytes[0], b'+' | b'-')
            || offset_bytes[3] != b':'
        {
            return Err(MsgError::ValidationFailed);
        }
        let sign = if offset_bytes[0] == b'-' { -1 } else { 1 };
        let hours: i8 = parse_ascii_decimal(&offset_bytes[1..3])?;
        let minutes: i8 = parse_ascii_decimal(&offset_bytes[4..6])?;
        let offset = UtcOffset::from_hms(sign * hours, sign * minutes, 0)
            .map_err(|_| MsgError::ValidationFailed)?;
        (&rest[..offset_idx], offset)
    } else {
        return Err(MsgError::ValidationFailed);
    };

    let mut pieces = time_part.split(':');
    let hour_literal = pieces
        .next()
        .filter(|value| value.len() == 2)
        .ok_or(MsgError::ValidationFailed)?;
    let hour: u8 = parse_ascii_decimal(hour_literal.as_bytes())?;
    let minute_literal = pieces
        .next()
        .filter(|value| value.len() == 2)
        .ok_or(MsgError::ValidationFailed)?;
    let minute: u8 = parse_ascii_decimal(minute_literal.as_bytes())?;
    let second_fragment = pieces.next().ok_or(MsgError::ValidationFailed)?;
    if pieces.next().is_some() {
        return Err(MsgError::ValidationFailed);
    }
    let second_literal = if let Some((second, fraction)) = second_fragment.split_once('.') {
        if fraction.is_empty() || !fraction.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(MsgError::ValidationFailed);
        }
        second
    } else {
        second_fragment
    };
    if second_literal.len() != 2 {
        return Err(MsgError::ValidationFailed);
    }
    let second: u8 = parse_ascii_decimal(second_literal.as_bytes())?;

    let date = Date::from_calendar_date(
        year,
        Month::try_from(month).map_err(|_| MsgError::ValidationFailed)?,
        day,
    )
    .map_err(|_| MsgError::ValidationFailed)?;
    let time = Time::from_hms(hour, minute, second).map_err(|_| MsgError::ValidationFailed)?;
    let timestamp = PrimitiveDateTime::new(date, time)
        .assume_offset(offset)
        .to_offset(UtcOffset::UTC)
        .unix_timestamp();
    ASN1Time::from_timestamp(timestamp).map_err(|_| MsgError::ValidationFailed)
}

fn parse_ascii_decimal<T: std::str::FromStr>(value: &[u8]) -> Result<T, MsgError> {
    if value.is_empty() || !value.iter().all(|byte| byte.is_ascii_digit()) {
        return Err(MsgError::ValidationFailed);
    }
    std::str::from_utf8(value)
        .map_err(|_| MsgError::ValidationFailed)?
        .parse()
        .map_err(|_| MsgError::ValidationFailed)
}

fn ensure_xml_signature_leaf_certificate_policy(
    certificate: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    if matches!(
        certificate
            .basic_constraints()
            .map_err(|_| MsgError::ValidationFailed)?,
        Some(basic_constraints) if basic_constraints.value.ca
    ) {
        return Err(MsgError::ValidationFailed);
    }
    match certificate
        .key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    {
        Some(key_usage) if key_usage.critical && key_usage.value.digital_signature() => Ok(()),
        _ => Err(MsgError::ValidationFailed),
    }
}

fn ensure_xml_signature_certificate_valid_at(
    certificate: &X509Certificate<'_>,
    evaluation_time: ASN1Time,
) -> Result<(), MsgError> {
    certificate
        .validity()
        .is_valid_at(evaluation_time)
        .then_some(())
        .ok_or(MsgError::ValidationFailed)
}

fn ensure_xml_signature_certificate_issued_by(
    certificate: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    (certificate.issuer().as_raw() == issuer.subject().as_raw())
        .then_some(())
        .ok_or(MsgError::ValidationFailed)
}

fn ensure_xml_signature_supported_certificate_algorithm(
    certificate: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    if certificate.signature_algorithm.algorithm != OID_SIG_ECDSA_WITH_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    let public_key = certificate.public_key();
    let public_key_algorithm = &public_key.algorithm;
    if public_key_algorithm.algorithm != OID_KEY_TYPE_EC_PUBLIC_KEY {
        return Err(MsgError::ValidationFailed);
    }
    match public_key_algorithm
        .parameters
        .as_ref()
        .and_then(|parameters| parameters.as_oid().ok())
    {
        Some(curve_oid) if curve_oid == OID_EC_P256 => {
            ensure_xml_signature_p256_public_key(public_key.subject_public_key.data.as_ref())
        }
        _ => Err(MsgError::ValidationFailed),
    }
}

fn ensure_xml_signature_issuer_certificate_policy(
    certificate: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    match certificate
        .basic_constraints()
        .map_err(|_| MsgError::ValidationFailed)?
    {
        Some(basic_constraints) if basic_constraints.critical && basic_constraints.value.ca => {}
        _ => return Err(MsgError::ValidationFailed),
    }
    match certificate
        .key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    {
        Some(key_usage) if key_usage.critical && key_usage.value.key_cert_sign() => Ok(()),
        _ => Err(MsgError::ValidationFailed),
    }
}

fn ensure_xml_signature_issuer_path_len(
    certificate: &X509Certificate<'_>,
    subordinate_ca_count: usize,
) -> Result<(), MsgError> {
    let Some(basic_constraints) = certificate
        .basic_constraints()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Err(MsgError::ValidationFailed);
    };
    match basic_constraints.value.path_len_constraint {
        Some(path_len_constraint) if subordinate_ca_count > path_len_constraint as usize => {
            Err(MsgError::ValidationFailed)
        }
        _ => Ok(()),
    }
}

fn ensure_xml_signature_supported_critical_extensions(
    certificate: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    for extension in certificate.extensions() {
        if !extension.critical {
            continue;
        }
        match extension.parsed_extension() {
            ParsedExtension::BasicConstraints(_) | ParsedExtension::KeyUsage(_) => {}
            ParsedExtension::UnsupportedExtension { .. }
            | ParsedExtension::ParseError { .. }
            | ParsedExtension::Unparsed => return Err(MsgError::ValidationFailed),
            _ => return Err(MsgError::ValidationFailed),
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum CanonicalXmlAttributeKind {
    Namespace,
    Attribute,
}

#[derive(Debug, Clone)]
struct CanonicalXmlAttribute {
    name: String,
    value: String,
    kind: CanonicalXmlAttributeKind,
}

#[derive(Debug, Clone)]
struct CanonicalXmlNamespaceBinding {
    prefix: String,
    uri: String,
}

fn canonicalize_supported_xml(xml: &str) -> Result<String, MsgError> {
    canonicalize_supported_xml_with_inherited_namespaces(xml, &[])
}

fn canonicalize_supported_xml_with_inherited_namespaces(
    xml: &str,
    inherited_namespaces: &[CanonicalXmlAttribute],
) -> Result<String, MsgError> {
    canonicalize_supported_xml_with_mode(xml, inherited_namespaces, CanonicalXmlMode::Exclusive)
}

fn canonicalize_supported_xml_with_mode(
    xml: &str,
    inherited_namespaces: &[CanonicalXmlAttribute],
    mode: CanonicalXmlMode,
) -> Result<String, MsgError> {
    if xml.is_empty()
        || xml.trim() != xml
        || xml.contains("<?")
        || xml.contains("<![CDATA[")
        || xml.contains('\r')
    {
        return Err(MsgError::ValidationFailed);
    }

    let mut cursor = 0usize;
    let mut output = String::with_capacity(xml.len());
    let mut stack = Vec::new();
    let mut namespace_scope_lengths = Vec::new();
    let mut in_scope_namespaces = Vec::new();
    let mut root_seen = false;
    while cursor < xml.len() {
        let Some(tag_start) = xml[cursor..].find('<').map(|offset| cursor + offset) else {
            let text = &xml[cursor..];
            if stack.is_empty() && !text.is_empty() {
                return Err(MsgError::ValidationFailed);
            }
            push_canonical_xml_text(text, &mut output)?;
            break;
        };
        let text = &xml[cursor..tag_start];
        if stack.is_empty() && !text.is_empty() {
            return Err(MsgError::ValidationFailed);
        }
        push_canonical_xml_text(text, &mut output)?;
        if xml[tag_start..].starts_with("<!--") {
            cursor = find_supported_xml_comment_end(xml, tag_start)? + 3;
            continue;
        }
        let raw_start = tag_start + 1;
        let tag_end =
            find_xml_tag_end(xml.as_bytes(), raw_start).ok_or(MsgError::ValidationFailed)?;
        let raw_tag_literal = &xml[raw_start..tag_end];
        if raw_tag_literal
            .chars()
            .next()
            .is_some_and(char::is_whitespace)
        {
            return Err(MsgError::ValidationFailed);
        }
        let raw_tag = raw_tag_literal.trim();
        if raw_tag.is_empty() || raw_tag.starts_with('?') || raw_tag.starts_with('!') {
            return Err(MsgError::ValidationFailed);
        }
        if let Some(closing_tag) = raw_tag.strip_prefix('/') {
            let name = closing_tag.trim();
            if name.is_empty() || name.split_once(char::is_whitespace).is_some() {
                return Err(MsgError::ValidationFailed);
            }
            ensure_supported_xml_name(name)?;
            match stack.pop() {
                Some(open_name) if open_name == name => {}
                _ => return Err(MsgError::ValidationFailed),
            }
            let namespace_scope_len = namespace_scope_lengths
                .pop()
                .ok_or(MsgError::ValidationFailed)?;
            in_scope_namespaces.truncate(namespace_scope_len);
            output.push_str("</");
            output.push_str(name);
            output.push('>');
        } else {
            let self_closing = raw_tag.ends_with('/');
            let tag_body = if self_closing {
                raw_tag.trim_end_matches('/').trim_end()
            } else {
                raw_tag
            };
            let (name, attributes) = split_supported_xml_tag(tag_body)?;
            ensure_supported_xml_name(name)?;
            let mut attributes = parse_supported_xml_attributes(attributes)?;
            if stack.is_empty() {
                apply_inherited_root_namespaces(name, &mut attributes, inherited_namespaces, mode)?;
            }
            ensure_supported_xml_element_namespace(name, &attributes, &in_scope_namespaces)?;
            sort_and_validate_canonical_xml_attributes(&mut attributes, &in_scope_namespaces)?;
            if stack.is_empty() {
                if root_seen {
                    return Err(MsgError::ValidationFailed);
                }
                root_seen = true;
            }
            push_canonical_xml_start_tag(name, &attributes, &mut output)?;
            if self_closing {
                output.push_str("</");
                output.push_str(name);
                output.push('>');
            } else {
                namespace_scope_lengths.push(in_scope_namespaces.len());
                in_scope_namespaces.extend(
                    attributes
                        .iter()
                        .filter_map(namespace_binding_from_attribute),
                );
                stack.push(name);
            }
        }
        cursor = tag_end + 1;
    }
    if !root_seen || !stack.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(output)
}

fn find_supported_xml_comment_end(xml: &str, start: usize) -> Result<usize, MsgError> {
    let Some(comment_body_start) = xml[start..].strip_prefix("<!--").map(|_| start + 4) else {
        return Err(MsgError::ValidationFailed);
    };
    let comment_end = xml[comment_body_start..]
        .find("-->")
        .map(|offset| comment_body_start + offset)
        .ok_or(MsgError::ValidationFailed)?;
    let comment_body = &xml[comment_body_start..comment_end];
    if comment_body.contains("--") || comment_body.ends_with('-') {
        return Err(MsgError::ValidationFailed);
    }
    Ok(comment_end)
}

fn xml_root_namespace_declarations(xml: &str) -> Result<Vec<CanonicalXmlAttribute>, MsgError> {
    if !xml.starts_with('<') {
        return Err(MsgError::ValidationFailed);
    }
    let tag_end = find_xml_tag_end(xml.as_bytes(), 1).ok_or(MsgError::ValidationFailed)?;
    let raw_tag = xml[1..tag_end].trim();
    if raw_tag.is_empty()
        || raw_tag.starts_with('/')
        || raw_tag.starts_with('?')
        || raw_tag.starts_with('!')
    {
        return Err(MsgError::ValidationFailed);
    }
    let tag_body = raw_tag.trim_end_matches('/').trim_end();
    let (_, attributes) = split_supported_xml_tag(tag_body)?;
    let mut attributes = parse_supported_xml_attributes(attributes)?;
    sort_and_validate_canonical_xml_attributes(&mut attributes, &[])?;
    Ok(attributes
        .into_iter()
        .filter(|attribute| attribute.kind == CanonicalXmlAttributeKind::Namespace)
        .collect())
}

fn split_supported_xml_tag(tag_body: &str) -> Result<(&str, &str), MsgError> {
    if let Some((name, attributes)) = tag_body.split_once(char::is_whitespace) {
        if name.is_empty() {
            return Err(MsgError::ValidationFailed);
        }
        Ok((name, attributes))
    } else if tag_body.is_empty() {
        Err(MsgError::ValidationFailed)
    } else {
        Ok((tag_body, ""))
    }
}

fn parse_supported_xml_attributes(
    attributes: &str,
) -> Result<Vec<CanonicalXmlAttribute>, MsgError> {
    let mut parsed = Vec::new();
    let mut cursor = attributes.trim();
    while !cursor.is_empty() {
        let name_end = cursor
            .find(|ch: char| ch.is_whitespace() || ch == '=')
            .ok_or(MsgError::ValidationFailed)?;
        let name = &cursor[..name_end];
        ensure_supported_xml_name(name)?;
        let kind = if name == "xmlns" || name.starts_with("xmlns:") {
            CanonicalXmlAttributeKind::Namespace
        } else {
            CanonicalXmlAttributeKind::Attribute
        };
        cursor = cursor[name_end..].trim_start();
        cursor = cursor
            .strip_prefix('=')
            .ok_or(MsgError::ValidationFailed)?
            .trim_start();
        let quote = cursor
            .chars()
            .next()
            .filter(|quote| matches!(quote, '"' | '\''))
            .ok_or(MsgError::ValidationFailed)?;
        cursor = &cursor[quote.len_utf8()..];
        let value_end = cursor.find(quote).ok_or(MsgError::ValidationFailed)?;
        let value = &cursor[..value_end];
        ensure_supported_xml_attribute_value(value)?;
        if kind == CanonicalXmlAttributeKind::Namespace {
            ensure_supported_xml_namespace_declaration(name, value)?;
        }
        parsed.push(CanonicalXmlAttribute {
            name: name.to_owned(),
            value: value.to_owned(),
            kind,
        });
        cursor = cursor[value_end + quote.len_utf8()..].trim_start();
    }
    Ok(parsed)
}

fn apply_inherited_root_namespaces(
    name: &str,
    attributes: &mut Vec<CanonicalXmlAttribute>,
    inherited_namespaces: &[CanonicalXmlAttribute],
    mode: CanonicalXmlMode,
) -> Result<(), MsgError> {
    match mode {
        CanonicalXmlMode::Inclusive => {
            for namespace in inherited_namespaces {
                if !attributes
                    .iter()
                    .any(|attribute| attribute.name == namespace.name)
                {
                    attributes.push(namespace.clone());
                }
            }
        }
        CanonicalXmlMode::Exclusive => {
            if let Some((prefix, _)) = name.split_once(':') {
                let namespace_name = format!("xmlns:{prefix}");
                apply_inherited_namespace(attributes, inherited_namespaces, &namespace_name);
            } else if !attributes.iter().any(|attribute| attribute.name == "xmlns") {
                if let Some(namespace) = inherited_namespaces
                    .iter()
                    .find(|attribute| attribute.name == "xmlns" && !attribute.value.is_empty())
                {
                    attributes.push(namespace.clone());
                }
            }
            let visibly_used_attribute_namespaces = attributes
                .iter()
                .filter(|attribute| attribute.kind == CanonicalXmlAttributeKind::Attribute)
                .filter_map(|attribute| attribute.name.split_once(':').map(|(prefix, _)| prefix))
                .filter(|prefix| *prefix != "xml")
                .map(|prefix| format!("xmlns:{prefix}"))
                .collect::<Vec<_>>();
            for namespace_name in visibly_used_attribute_namespaces {
                apply_inherited_namespace(attributes, inherited_namespaces, &namespace_name);
            }
        }
    }
    Ok(())
}

fn apply_inherited_namespace(
    attributes: &mut Vec<CanonicalXmlAttribute>,
    inherited_namespaces: &[CanonicalXmlAttribute],
    namespace_name: &str,
) {
    if attributes
        .iter()
        .any(|attribute| attribute.name == namespace_name)
    {
        return;
    }
    if let Some(namespace) = inherited_namespaces
        .iter()
        .find(|attribute| attribute.name == namespace_name)
    {
        attributes.push(namespace.clone());
    }
}

fn sort_and_validate_canonical_xml_attributes(
    parsed: &mut Vec<CanonicalXmlAttribute>,
    in_scope_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    ensure_supported_xml_attribute_namespaces(parsed, in_scope_namespaces)?;
    ensure_unique_canonical_xml_attribute_names(parsed, in_scope_namespaces)?;
    let lookup = parsed.clone();
    parsed.sort_by(|left, right| match (left.kind, right.kind) {
        (CanonicalXmlAttributeKind::Namespace, CanonicalXmlAttributeKind::Namespace) => {
            namespace_attribute_sort_key(&left.name).cmp(namespace_attribute_sort_key(&right.name))
        }
        (CanonicalXmlAttributeKind::Namespace, CanonicalXmlAttributeKind::Attribute) => {
            std::cmp::Ordering::Less
        }
        (CanonicalXmlAttributeKind::Attribute, CanonicalXmlAttributeKind::Namespace) => {
            std::cmp::Ordering::Greater
        }
        (CanonicalXmlAttributeKind::Attribute, CanonicalXmlAttributeKind::Attribute) => {
            canonical_xml_attribute_sort_key(&left.name, &lookup, in_scope_namespaces)
                .expect("attribute namespaces are validated before sorting")
                .cmp(
                    &canonical_xml_attribute_sort_key(&right.name, &lookup, in_scope_namespaces)
                        .expect("attribute namespaces are validated before sorting"),
                )
        }
    });
    if parsed
        .windows(2)
        .any(|attrs| attrs[0].name.as_str() == attrs[1].name.as_str())
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn namespace_attribute_sort_key(name: &str) -> &str {
    name.strip_prefix("xmlns:").unwrap_or_default()
}

fn canonical_xml_attribute_sort_key(
    name: &str,
    attributes: &[CanonicalXmlAttribute],
    in_scope_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(String, String), MsgError> {
    if let Some((prefix, local)) = name.split_once(':') {
        let namespace_uri = namespace_uri_for_prefix(prefix, attributes, in_scope_namespaces)
            .ok_or(MsgError::ValidationFailed)?;
        Ok((namespace_uri.to_owned(), local.to_owned()))
    } else {
        Ok((String::new(), name.to_owned()))
    }
}

fn namespace_binding_from_attribute(
    attribute: &CanonicalXmlAttribute,
) -> Option<CanonicalXmlNamespaceBinding> {
    if attribute.kind != CanonicalXmlAttributeKind::Namespace {
        return None;
    }
    let prefix = if attribute.name == "xmlns" {
        ""
    } else {
        attribute.name.strip_prefix("xmlns:")?
    };
    if prefix == "xml" {
        return None;
    }
    Some(CanonicalXmlNamespaceBinding {
        prefix: prefix.to_owned(),
        uri: attribute.value.clone(),
    })
}

fn namespace_bindings_from_attributes(
    attributes: &[CanonicalXmlAttribute],
) -> Vec<CanonicalXmlNamespaceBinding> {
    attributes
        .iter()
        .filter_map(namespace_binding_from_attribute)
        .collect()
}

fn inherited_namespace_attributes_from_scope(
    in_scope_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Vec<CanonicalXmlAttribute> {
    let mut inherited = Vec::new();
    for binding in in_scope_namespaces.iter().rev() {
        let name = if binding.prefix.is_empty() {
            "xmlns".to_owned()
        } else {
            format!("xmlns:{}", binding.prefix)
        };
        if inherited
            .iter()
            .any(|attribute: &CanonicalXmlAttribute| attribute.name == name)
        {
            continue;
        }
        inherited.push(CanonicalXmlAttribute {
            name,
            value: binding.uri.clone(),
            kind: CanonicalXmlAttributeKind::Namespace,
        });
    }
    inherited.reverse();
    inherited
}

fn namespace_uri_for_prefix<'a>(
    prefix: &str,
    attributes: &'a [CanonicalXmlAttribute],
    in_scope_namespaces: &'a [CanonicalXmlNamespaceBinding],
) -> Option<&'a str> {
    if prefix == "xml" {
        return Some(XML_NS);
    }
    if prefix == "xmlns" {
        return None;
    }
    attributes
        .iter()
        .rev()
        .find(|attribute| namespace_prefix_from_attribute(attribute) == Some(prefix))
        .map(|attribute| attribute.value.as_str())
        .or_else(|| {
            in_scope_namespaces
                .iter()
                .rev()
                .find(|binding| binding.prefix == prefix)
                .map(|binding| binding.uri.as_str())
        })
}

fn namespace_prefix_from_attribute(attribute: &CanonicalXmlAttribute) -> Option<&str> {
    if attribute.kind != CanonicalXmlAttributeKind::Namespace {
        return None;
    }
    if attribute.name == "xmlns" {
        Some("")
    } else {
        attribute.name.strip_prefix("xmlns:")
    }
}

fn ensure_supported_xml_attribute_namespaces(
    attributes: &[CanonicalXmlAttribute],
    in_scope_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.kind == CanonicalXmlAttributeKind::Attribute)
    {
        let Some((prefix, _)) = attribute.name.split_once(':') else {
            continue;
        };
        namespace_uri_for_prefix(prefix, attributes, in_scope_namespaces)
            .ok_or(MsgError::ValidationFailed)?;
    }
    Ok(())
}

fn ensure_unique_canonical_xml_attribute_names(
    attributes: &[CanonicalXmlAttribute],
    in_scope_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    let mut expanded_names = Vec::new();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.kind == CanonicalXmlAttributeKind::Attribute)
    {
        let expanded_name =
            canonical_xml_attribute_sort_key(&attribute.name, attributes, in_scope_namespaces)?;
        if expanded_names
            .iter()
            .any(|existing| existing == &expanded_name)
        {
            return Err(MsgError::ValidationFailed);
        }
        expanded_names.push(expanded_name);
    }
    Ok(())
}

fn ensure_supported_xml_namespace_declaration(name: &str, value: &str) -> Result<(), MsgError> {
    if name == "xmlns:xml" {
        return if value == XML_NS {
            Ok(())
        } else {
            Err(MsgError::ValidationFailed)
        };
    }
    if name.starts_with("xmlns:") && value.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    let reserved_prefix = name
        .strip_prefix("xmlns:")
        .is_some_and(|prefix| matches!(prefix, "xml" | "xmlns"));
    let reserved_namespace = matches!(value, XML_NS | XMLNS_NS);
    if reserved_prefix || reserved_namespace {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn ensure_supported_xml_name(name: &str) -> Result<(), MsgError> {
    if name.starts_with(':') || name.ends_with(':') || name.matches(':').count() > 1 {
        return Err(MsgError::ValidationFailed);
    }
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return Err(MsgError::ValidationFailed);
    };
    if !matches!(first, b'A'..=b'Z' | b'a'..=b'z' | b'_' | b':') {
        return Err(MsgError::ValidationFailed);
    }
    if bytes.all(|byte| {
        matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b':'
        )
    }) {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn ensure_supported_xml_element_namespace(
    name: &str,
    attributes: &[CanonicalXmlAttribute],
    in_scope_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    let Some((prefix, _)) = name.split_once(':') else {
        return Ok(());
    };
    if namespace_uri_for_prefix(prefix, attributes, in_scope_namespaces).is_some() {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn ensure_supported_xml_attribute_value(value: &str) -> Result<(), MsgError> {
    if value.contains('<') || value.contains('\r') || value.contains('\n') || value.contains('\t') {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn push_canonical_xml_start_tag(
    name: &str,
    attributes: &[CanonicalXmlAttribute],
    output: &mut String,
) -> Result<(), MsgError> {
    output.push('<');
    output.push_str(name);
    for attribute in attributes {
        if attribute.name == "xmlns:xml" && attribute.value == XML_NS {
            continue;
        }
        output.push(' ');
        output.push_str(&attribute.name);
        output.push_str("=\"");
        push_canonical_xml_attribute_value(&attribute.value, output)?;
        output.push('"');
    }
    output.push('>');
    Ok(())
}

fn push_canonical_xml_text(value: &str, output: &mut String) -> Result<(), MsgError> {
    push_canonical_xml_value(value, output, push_canonical_xml_text_char)
}

fn push_canonical_xml_attribute_value(value: &str, output: &mut String) -> Result<(), MsgError> {
    push_canonical_xml_value(value, output, push_canonical_xml_attribute_char)
}

fn push_canonical_xml_value(
    value: &str,
    output: &mut String,
    push_char: fn(char, &mut String) -> Result<(), MsgError>,
) -> Result<(), MsgError> {
    let mut cursor = 0usize;
    while cursor < value.len() {
        let Some(entity_start) = value[cursor..].find('&').map(|offset| cursor + offset) else {
            push_raw_canonical_xml_chars(&value[cursor..], output, push_char)?;
            return Ok(());
        };
        push_raw_canonical_xml_chars(&value[cursor..entity_start], output, push_char)?;
        let reference_start = entity_start + 1;
        let reference_end = value[reference_start..]
            .find(';')
            .map(|offset| reference_start + offset)
            .ok_or(MsgError::ValidationFailed)?;
        let ch = decode_supported_xml_character_reference(&value[reference_start..reference_end])?;
        push_char(ch, output)?;
        cursor = reference_end + 1;
    }
    Ok(())
}

fn push_raw_canonical_xml_chars(
    value: &str,
    output: &mut String,
    push_char: fn(char, &mut String) -> Result<(), MsgError>,
) -> Result<(), MsgError> {
    for ch in value.chars() {
        push_char(ch, output)?;
    }
    Ok(())
}

fn push_canonical_xml_text_char(ch: char, output: &mut String) -> Result<(), MsgError> {
    ensure_supported_xml_char(ch)?;
    match ch {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '>' => output.push_str("&gt;"),
        '\r' => output.push_str("&#xD;"),
        _ => output.push(ch),
    }
    Ok(())
}

fn push_canonical_xml_attribute_char(ch: char, output: &mut String) -> Result<(), MsgError> {
    ensure_supported_xml_char(ch)?;
    match ch {
        '&' => output.push_str("&amp;"),
        '<' => output.push_str("&lt;"),
        '"' => output.push_str("&quot;"),
        '\t' => output.push_str("&#x9;"),
        '\n' => output.push_str("&#xA;"),
        '\r' => output.push_str("&#xD;"),
        _ => output.push(ch),
    }
    Ok(())
}

fn decode_supported_xml_character_reference(reference: &str) -> Result<char, MsgError> {
    match reference {
        "amp" => return Ok('&'),
        "lt" => return Ok('<'),
        "gt" => return Ok('>'),
        "quot" => return Ok('"'),
        "apos" => return Ok('\''),
        _ => {}
    }
    let codepoint = if let Some(hex) = reference
        .strip_prefix("#x")
        .or_else(|| reference.strip_prefix("#X"))
    {
        if hex.is_empty() || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(MsgError::ValidationFailed);
        }
        u32::from_str_radix(hex, 16).map_err(|_| MsgError::ValidationFailed)?
    } else if let Some(decimal) = reference.strip_prefix('#') {
        if decimal.is_empty() || !decimal.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(MsgError::ValidationFailed);
        }
        decimal
            .parse::<u32>()
            .map_err(|_| MsgError::ValidationFailed)?
    } else {
        return Err(MsgError::ValidationFailed);
    };
    let ch = char::from_u32(codepoint).ok_or(MsgError::ValidationFailed)?;
    ensure_supported_xml_char(ch)?;
    Ok(ch)
}

fn ensure_supported_xml_char(ch: char) -> Result<(), MsgError> {
    if matches!(ch, '\u{9}' | '\u{a}' | '\u{d}')
        || ('\u{20}'..='\u{d7ff}').contains(&ch)
        || ('\u{e000}'..='\u{fffd}').contains(&ch)
        || ('\u{10000}'..='\u{10ffff}').contains(&ch)
    {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn required_single_xml_element(container: &str, local: &str) -> Result<XmlElementSpan, MsgError> {
    let span = find_first_xml_element(container, local).ok_or(MsgError::ValidationFailed)?;
    ensure_supported_xml_element_span_name(container, span)?;
    if find_first_xml_element(&container[span.end..], local).is_some() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(span)
}

fn ensure_xml_element_content_empty(container: &str, span: XmlElementSpan) -> Result<(), MsgError> {
    if container[span.content_start..span.content_end]
        .trim()
        .is_empty()
    {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn ensure_xml_element_text_only(container: &str, span: XmlElementSpan) -> Result<(), MsgError> {
    if container[span.content_start..span.content_end].contains('<') {
        Err(MsgError::ValidationFailed)
    } else {
        Ok(())
    }
}

fn ensure_direct_xml_child_elements(
    container: &str,
    parent_span: XmlElementSpan,
    allowed_children: &[&str],
) -> Result<(), MsgError> {
    ensure_direct_xml_child_elements_with_comment_policy(
        container,
        parent_span,
        allowed_children,
        false,
    )
}

fn ensure_direct_xml_child_elements_allowing_comments(
    container: &str,
    parent_span: XmlElementSpan,
    allowed_children: &[&str],
) -> Result<(), MsgError> {
    ensure_direct_xml_child_elements_with_comment_policy(
        container,
        parent_span,
        allowed_children,
        true,
    )
}

fn ensure_direct_xml_child_elements_with_comment_policy(
    container: &str,
    parent_span: XmlElementSpan,
    allowed_children: &[&str],
    allow_comments: bool,
) -> Result<(), MsgError> {
    let mut cursor = parent_span.content_start;
    while cursor < parent_span.content_end {
        let Some(offset) = container[cursor..parent_span.content_end].find('<') else {
            return if container[cursor..parent_span.content_end].trim().is_empty() {
                Ok(())
            } else {
                Err(MsgError::ValidationFailed)
            };
        };
        let start = cursor + offset;
        if !container[cursor..start].trim().is_empty() {
            return Err(MsgError::ValidationFailed);
        }

        let tag_start = start + 1;
        let tag_end =
            find_xml_tag_end(container.as_bytes(), tag_start).ok_or(MsgError::ValidationFailed)?;
        if tag_end > parent_span.content_end {
            return Err(MsgError::ValidationFailed);
        }
        if allow_comments && container[start..].starts_with("<!--") {
            let comment_end = find_supported_xml_comment_end(container, start)? + 3;
            if comment_end > parent_span.content_end {
                return Err(MsgError::ValidationFailed);
            }
            cursor = comment_end;
            continue;
        }
        let raw_tag = container[tag_start..tag_end].trim();
        if raw_tag.starts_with('/')
            || raw_tag.starts_with('?')
            || raw_tag.starts_with("!--")
            || raw_tag.starts_with("![CDATA[")
        {
            return Err(MsgError::ValidationFailed);
        }

        let self_closing = raw_tag.ends_with('/');
        let tag_body = raw_tag.trim_end_matches('/').trim_end();
        let (name, _) = tag_body
            .split_once(char::is_whitespace)
            .unwrap_or((tag_body, ""));
        ensure_supported_xml_name(name)?;
        let local = xml_local_name(name);
        if !allowed_children
            .iter()
            .any(|allowed_child| local == *allowed_child)
        {
            return Err(MsgError::ValidationFailed);
        }
        cursor = if self_closing {
            tag_end + 1
        } else {
            let (_, end) = find_xml_element_end(container, tag_end + 1, name)
                .ok_or(MsgError::ValidationFailed)?;
            if end > parent_span.content_end {
                return Err(MsgError::ValidationFailed);
            }
            end
        };
    }
    Ok(())
}

fn optional_direct_xml_child_element(
    container: &str,
    parent_span: XmlElementSpan,
    child: &str,
) -> Result<Option<XmlElementSpan>, MsgError> {
    optional_direct_xml_child_element_with_comment_policy(container, parent_span, child, false)
}

fn optional_direct_xml_child_element_allowing_comments(
    container: &str,
    parent_span: XmlElementSpan,
    child: &str,
) -> Result<Option<XmlElementSpan>, MsgError> {
    optional_direct_xml_child_element_with_comment_policy(container, parent_span, child, true)
}

fn optional_direct_xml_child_element_with_comment_policy(
    container: &str,
    parent_span: XmlElementSpan,
    child: &str,
    allow_comments: bool,
) -> Result<Option<XmlElementSpan>, MsgError> {
    let mut cursor = parent_span.content_start;
    let mut found = None;
    while cursor < parent_span.content_end {
        let Some(offset) = container[cursor..parent_span.content_end].find('<') else {
            return if container[cursor..parent_span.content_end].trim().is_empty() {
                Ok(found)
            } else {
                Err(MsgError::ValidationFailed)
            };
        };
        let start = cursor + offset;
        if !container[cursor..start].trim().is_empty() {
            return Err(MsgError::ValidationFailed);
        }

        let tag_start = start + 1;
        let tag_end =
            find_xml_tag_end(container.as_bytes(), tag_start).ok_or(MsgError::ValidationFailed)?;
        if tag_end > parent_span.content_end {
            return Err(MsgError::ValidationFailed);
        }
        if allow_comments && container[start..].starts_with("<!--") {
            let comment_end = find_supported_xml_comment_end(container, start)? + 3;
            if comment_end > parent_span.content_end {
                return Err(MsgError::ValidationFailed);
            }
            cursor = comment_end;
            continue;
        }
        let raw_tag = container[tag_start..tag_end].trim();
        if raw_tag.starts_with('/')
            || raw_tag.starts_with('?')
            || raw_tag.starts_with("!--")
            || raw_tag.starts_with("![CDATA[")
        {
            return Err(MsgError::ValidationFailed);
        }

        let self_closing = raw_tag.ends_with('/');
        let tag_body = raw_tag.trim_end_matches('/').trim_end();
        let (name, _) = tag_body
            .split_once(char::is_whitespace)
            .unwrap_or((tag_body, ""));
        ensure_supported_xml_name(name)?;
        let local = xml_local_name(name);
        let child_span = if self_closing {
            XmlElementSpan {
                start,
                opening_end: tag_end,
                content_start: tag_end + 1,
                content_end: tag_end + 1,
                end: tag_end + 1,
            }
        } else {
            let (content_end, end) = find_xml_element_end(container, tag_end + 1, name)
                .ok_or(MsgError::ValidationFailed)?;
            if end > parent_span.content_end {
                return Err(MsgError::ValidationFailed);
            }
            XmlElementSpan {
                start,
                opening_end: tag_end,
                content_start: tag_end + 1,
                content_end,
                end,
            }
        };
        if local == child && found.replace(child_span).is_some() {
            return Err(MsgError::ValidationFailed);
        }
        cursor = child_span.end;
    }
    Ok(found)
}

fn required_direct_xml_child_element(
    container: &str,
    parent_span: XmlElementSpan,
    child: &str,
) -> Result<XmlElementSpan, MsgError> {
    optional_direct_xml_child_element(container, parent_span, child)?
        .ok_or(MsgError::ValidationFailed)
}

fn required_direct_xml_child_element_allowing_comments(
    container: &str,
    parent_span: XmlElementSpan,
    child: &str,
) -> Result<XmlElementSpan, MsgError> {
    optional_direct_xml_child_element_allowing_comments(container, parent_span, child)?
        .ok_or(MsgError::ValidationFailed)
}

fn decode_direct_child_base64(
    container: &str,
    child_span: XmlElementSpan,
) -> Result<Vec<u8>, MsgError> {
    ensure_xml_element_attributes_allowed(container, child_span, &[])?;
    ensure_xml_element_text_only(container, child_span)?;
    let value: String = container[child_span.content_start..child_span.content_end]
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    BASE64_STANDARD
        .decode(value)
        .map_err(|_| MsgError::ValidationFailed)
}

fn optional_direct_child_text_compact(
    container: &str,
    parent_span: XmlElementSpan,
    child: &str,
) -> Result<Option<String>, MsgError> {
    let Some(span) = optional_direct_xml_child_element(container, parent_span, child)? else {
        return Ok(None);
    };
    ensure_xml_element_attributes_allowed(container, span, &[])?;
    ensure_xml_element_text_only(container, span)?;
    Ok(Some(
        container[span.content_start..span.content_end]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect(),
    ))
}

fn ensure_xml_element_attributes_allowed(
    container: &str,
    span: XmlElementSpan,
    allowed_attributes: &[&str],
) -> Result<(), MsgError> {
    let (_, attributes) = validated_xml_element_name_and_attributes(container, span, &[])?;
    for attribute in attributes {
        if attribute.kind == CanonicalXmlAttributeKind::Namespace {
            continue;
        }
        if !allowed_attributes
            .iter()
            .any(|allowed_attribute| attribute.name == *allowed_attribute)
        {
            return Err(MsgError::ValidationFailed);
        }
    }
    Ok(())
}

fn ensure_xml_element_prefixed_namespace(
    container: &str,
    span: XmlElementSpan,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
    expected_namespace: &str,
) -> Result<(), MsgError> {
    let (name, attributes) =
        validated_xml_element_name_and_attributes(container, span, inherited_namespaces)?;
    let Some((prefix, _)) = name.split_once(':') else {
        return match namespace_uri_for_prefix("", &attributes, inherited_namespaces) {
            Some(namespace) if !namespace.is_empty() && namespace != expected_namespace => {
                Err(MsgError::ValidationFailed)
            }
            _ => Ok(()),
        };
    };
    if namespace_uri_for_prefix(prefix, &attributes, inherited_namespaces)
        == Some(expected_namespace)
    {
        Ok(())
    } else {
        Err(MsgError::ValidationFailed)
    }
}

fn ensure_xades_element_prefixed_namespace(
    container: &str,
    span: XmlElementSpan,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(), MsgError> {
    ensure_xml_element_prefixed_namespace(container, span, inherited_namespaces, XADES_NS)
}

fn xml_element_namespace_scope(
    container: &str,
    span: XmlElementSpan,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<Vec<CanonicalXmlNamespaceBinding>, MsgError> {
    let (_, attributes) =
        validated_xml_element_name_and_attributes(container, span, inherited_namespaces)?;
    let mut scope = inherited_namespaces.to_vec();
    scope.extend(
        attributes
            .iter()
            .filter_map(namespace_binding_from_attribute),
    );
    Ok(scope)
}

fn validated_xml_element_name_and_attributes<'a>(
    container: &'a str,
    span: XmlElementSpan,
    inherited_namespaces: &[CanonicalXmlNamespaceBinding],
) -> Result<(&'a str, Vec<CanonicalXmlAttribute>), MsgError> {
    let opening = container[span.start + 1..span.opening_end].trim();
    let tag_body = opening.trim_end_matches('/').trim_end();
    let (name, attributes) = split_supported_xml_tag(tag_body)?;
    ensure_supported_xml_name(name)?;
    let mut attributes = parse_supported_xml_attributes(attributes)?;
    sort_and_validate_canonical_xml_attributes(&mut attributes, inherited_namespaces)?;
    Ok((name, attributes))
}

fn ensure_supported_xml_element_span_name(
    container: &str,
    span: XmlElementSpan,
) -> Result<(), MsgError> {
    let opening = container[span.start + 1..span.opening_end].trim();
    let tag_body = opening.trim_end_matches('/').trim_end();
    let (name, _) = split_supported_xml_tag(tag_body)?;
    ensure_supported_xml_name(name)
}

fn required_single_child_text_compact(container: &str, child: &str) -> Result<String, MsgError> {
    optional_single_child_text_compact(container, child)?.ok_or(MsgError::ValidationFailed)
}

fn optional_single_child_text_compact(
    container: &str,
    child: &str,
) -> Result<Option<String>, MsgError> {
    let Some(span) = find_first_xml_element(container, child) else {
        return Ok(None);
    };
    if find_first_xml_element(&container[span.end..], child).is_some() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(Some(
        container[span.content_start..span.content_end]
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect(),
    ))
}

fn element_attr(container: &str, span: XmlElementSpan, attr: &str) -> Option<String> {
    let opening = &container[span.start + 1..span.opening_end];
    attr_value_exact(opening, attr)
}

fn attr_value_exact(opening: &str, attr: &str) -> Option<String> {
    attr_value_matching(opening, |name| name == attr)
}

fn attr_value_matching(
    opening: &str,
    mut name_matches: impl FnMut(&str) -> bool,
) -> Option<String> {
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
        if name_matches(name) {
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
            let (content_end, end) = find_xml_element_end(text, opening_end + 1, name)?;
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

fn find_xml_element_end(
    text: &str,
    mut cursor: usize,
    opening_name: &str,
) -> Option<(usize, usize)> {
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
        if name == opening_name {
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

fn normalise_business_message_id(value: &str) -> String {
    value.trim().to_owned()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    lower_hex(&digest)
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
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
    use p256::ecdsa::{
        SigningKey as P256SigningKey,
        signature::{Signer as _, Verifier as _},
    };
    use rcgen::{
        BasicConstraints, CertificateParams, CustomExtension, DnType, IsCa, Issuer,
        KeyPair as RcgenKeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, PKCS_ECDSA_P384_SHA384,
        PublicKeyData, SignatureAlgorithm, SigningKey as _, date_time_ymd,
    };
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    const LEGACY_PUBLIC_KEY_LITERAL: &str =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@test";
    const XML_SIGNATURE_TEST_SIGNING_TIME: &str = "2025-01-01T12:00:00Z";

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
            revoked_certificate_sha256: Vec::new(),
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
            revoked_certificate_sha256: Vec::new(),
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

    fn low_s_p256_signature(signature: P256Signature) -> P256Signature {
        signature.normalize_s().unwrap_or(signature)
    }

    fn low_s_p256_signature_der_from_bytes(signature: &[u8]) -> Vec<u8> {
        let signature = if signature.len() == P256_XMLDSIG_SIGNATURE_LEN {
            P256Signature::from_slice(signature)
        } else {
            P256Signature::from_der(signature)
        }
        .expect("rcgen P-256 signature");
        low_s_p256_signature(signature).to_der().as_bytes().to_vec()
    }

    fn high_s_p256_signature(signature: P256Signature) -> P256Signature {
        const P256_ORDER: [u8; 32] = [
            0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2,
            0xfc, 0x63, 0x25, 0x51,
        ];
        let low_s = low_s_p256_signature(signature);
        let low_s_bytes = low_s.to_bytes();
        let mut high_s_bytes = [0_u8; P256_XMLDSIG_SIGNATURE_LEN];
        high_s_bytes[..32].copy_from_slice(&low_s_bytes[..32]);

        let mut borrow = 0_u16;
        for i in (0..32).rev() {
            let minuend = i16::from(P256_ORDER[i]) - i16::from(borrow as u8);
            let subtrahend = i16::from(low_s_bytes[32 + i]);
            if minuend >= subtrahend {
                high_s_bytes[32 + i] = (minuend - subtrahend) as u8;
                borrow = 0;
            } else {
                high_s_bytes[32 + i] = (minuend + 256 - subtrahend) as u8;
                borrow = 1;
            }
        }
        assert_eq!(
            borrow, 0,
            "low-S fixture scalar must be below the curve order"
        );
        let high_s = P256Signature::from_slice(&high_s_bytes).expect("high-S signature");
        assert!(
            high_s.normalize_s().is_some(),
            "test helper must produce a non-canonical high-S signature"
        );
        high_s
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
        unsigned_pacs008_xml_with_message_id("sig-001")
    }

    fn unsigned_pacs008_xml_with_message_id(message_id: &str) -> String {
        format!(
            concat!(
                r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">"#,
                "<FIToFICstmrCdtTrf><GrpHdr><MsgId>{}</MsgId></GrpHdr>",
                r#"<CdtTrfTxInf><IntrBkSttlmAmt Ccy="USD">10.00</IntrBkSttlmAmt>"#,
                "<IntrBkSttlmDt>2024-01-01</IntrBkSttlmDt>",
                "<DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>",
                "<CdtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></CdtrAcct>",
                "<DbtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></DbtrAgt>",
                "<CdtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></CdtrAgt>",
                "</CdtTrfTxInf></FIToFICstmrCdtTrf></Document>",
            ),
            message_id
        )
    }

    fn unsigned_pacs008_xml_with_document_id(reference_id: &str) -> String {
        unsigned_pacs008_xml().replacen(
            "<Document ",
            &format!(r#"<Document Id="{reference_id}" "#),
            1,
        )
    }

    fn unsigned_pacs008_xml_with_inherited_default_namespace_reference_id(
        reference_id: &str,
    ) -> String {
        unsigned_pacs008_xml().replacen(
            "<FIToFICstmrCdtTrf>",
            &format!(r#"<FIToFICstmrCdtTrf Id="{reference_id}">"#),
            1,
        )
    }

    fn unsigned_pacs008_xml_with_inherited_unused_namespace_reference_id(
        reference_id: &str,
    ) -> String {
        unsigned_pacs008_xml_with_inherited_default_namespace_reference_id(reference_id).replacen(
            r#"<Document xmlns=""#,
            r#"<Document xmlns:unused="urn:unused" xmlns=""#,
            1,
        )
    }

    fn signed_pacs008_xml() -> String {
        signed_pacs008_xml_with_c14n_algorithm(XML_C14N_1_0)
    }

    fn signed_pacs008_xml_with_public_key(public_key_base64: &str) -> String {
        let payload = signed_pacs008_xml();
        let public_key_start =
            payload.find("<PublicKey>").expect("fixture PublicKey") + "<PublicKey>".len();
        let public_key_end = payload[public_key_start..]
            .find("</PublicKey>")
            .expect("fixture PublicKey end")
            + public_key_start;
        format!(
            "{}{}{}",
            &payload[..public_key_start],
            public_key_base64,
            &payload[public_key_end..]
        )
    }

    fn signed_pacs008_xml_with_c14n_algorithm(c14n_algorithm: &str) -> String {
        signed_pacs008_xml_with_c14n_algorithm_and_signed_info_style(c14n_algorithm, false)
    }

    fn signed_pacs008_xml_with_self_closing_signed_info() -> String {
        signed_pacs008_xml_with_c14n_algorithm_and_signed_info_style(XML_C14N_1_0, true)
    }

    fn signed_pacs008_xml_with_raw_self_closing_signed_info_signature() -> String {
        signed_pacs008_xml_with_c14n_algorithm_and_signed_info_options(XML_C14N_1_0, true, false)
    }

    fn signed_pacs008_xml_with_fixed_width_signature_value() -> String {
        signed_pacs008_xml_from_unsigned_with_signature_encoding(
            unsigned_pacs008_xml(),
            XML_C14N_1_0,
            false,
            true,
            XmlSignatureValueEncoding::FixedWidth,
        )
    }

    fn signed_pacs008_xml_with_sgntr_signature_carrier() -> String {
        let unsigned = unsigned_pacs008_xml().replacen(
            "</FIToFICstmrCdtTrf>",
            &format!(r#"<Sgntr xmlns:ds="{XMLDSIG_NS}"></Sgntr></FIToFICstmrCdtTrf>"#),
            1,
        );
        let insertion = unsigned.find("</Sgntr>").expect("Sgntr insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = signed_info_xml_prefixed(XML_C14N_1_0, &digest);
        let inherited_namespaces = [CanonicalXmlAttribute {
            name: "xmlns:ds".to_owned(),
            value: XMLDSIG_NS.to_owned(),
            kind: CanonicalXmlAttributeKind::Namespace,
        }];
        let canonical_signed_info = canonicalize_supported_xml_with_mode(
            &signed_info,
            &inherited_namespaces,
            CanonicalXmlMode::Inclusive,
        )
        .expect("canonical Sgntr-wrapped SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<ds:Signature Id="sig-001">{signed_info}"#,
                "<ds:SignatureValue>{signature_value}</ds:SignatureValue>",
                "<ds:KeyInfo><ds:KeyValue><ds:ECKeyValue>",
                r#"<ds:NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></ds:NamedCurve>"#,
                "<ds:PublicKey>{public_key}</ds:PublicKey>",
                "</ds:ECKeyValue></ds:KeyValue></ds:KeyInfo>",
                r##"<ds:Object><xades:QualifyingProperties xmlns:xades="http://uri.etsi.org/01903/v1.3.2#" Target="#sig-001"></xades:QualifyingProperties></ds:Object>"##,
                "</ds:Signature>"
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

    fn signed_pacs008_xml_with_extra_sgntr_carrier() -> String {
        let unsigned = unsigned_pacs008_xml().replacen(
            "</FIToFICstmrCdtTrf>",
            "<Sgntr><SignedInfo><SignatureMethod>ignored</SignatureMethod></SignedInfo><SignatureValue>ignored</SignatureValue></Sgntr></FIToFICstmrCdtTrf>",
            1,
        );
        signed_pacs008_xml_from_unsigned(unsigned, XML_C14N_1_0, false, true)
    }

    fn signed_pacs008_xml_with_same_document_reference() -> String {
        let reference_id = "doc-001";
        let unsigned = unsigned_pacs008_xml_with_document_id(reference_id);
        signed_pacs008_xml_from_unsigned_with_reference_uri_and_signature_encoding(
            unsigned,
            XML_C14N_1_0,
            &format!("#{reference_id}"),
            false,
            true,
            XmlSignatureValueEncoding::Der,
        )
    }

    fn signed_pacs008_xml_with_inherited_namespace_same_document_reference() -> String {
        let reference_id = "doc-001";
        let unsigned =
            unsigned_pacs008_xml_with_inherited_default_namespace_reference_id(reference_id);
        signed_pacs008_xml_from_unsigned_with_reference_uri_and_signature_encoding(
            unsigned,
            XML_C14N_1_0,
            &format!("#{reference_id}"),
            false,
            true,
            XmlSignatureValueEncoding::Der,
        )
    }

    fn signed_pacs008_xml_with_header_only_same_document_reference() -> String {
        let reference_id = "grp-header-001";
        let unsigned = unsigned_pacs008_xml().replacen(
            "<GrpHdr>",
            &format!(r#"<GrpHdr Id="{reference_id}">"#),
            1,
        );
        signed_pacs008_xml_from_unsigned_with_reference_uri_and_signature_encoding(
            unsigned,
            XML_C14N_1_0,
            &format!("#{reference_id}"),
            false,
            true,
            XmlSignatureValueEncoding::Der,
        )
    }

    fn signed_pacs008_xml_with_reference_c14n_transform() -> String {
        let reference_id = "doc-001";
        let unsigned =
            unsigned_pacs008_xml_with_inherited_unused_namespace_reference_id(reference_id);
        signed_pacs008_xml_from_unsigned_with_reference_options(
            unsigned,
            XML_C14N_1_0,
            &format!("#{reference_id}"),
            false,
            true,
            XmlSignatureValueEncoding::Der,
            Some(XML_C14N_1_0),
        )
    }

    fn signed_pacs008_xml_with_extra_reference_transform() -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let supported_transform =
            format!(r#"<Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform>"#);
        let signed_info = signed_info_xml(XML_C14N_1_0, &digest, false).replace(
            &supported_transform,
            &format!(
                r#"{supported_transform}<Transform Algorithm="urn:unsupported-transform"></Transform>"#
            ),
        );
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical transformed SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<Signature Id="sig-001">{signed_info}<SignatureValue>{signature_value}</SignatureValue>"#,
                "<KeyInfo><KeyValue><ECKeyValue>",
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"></QualifyingProperties></Object>"##,
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

    fn signed_pacs008_xml_with_signed_info_rewrite(
        rewrite_signed_info: impl FnOnce(String) -> String,
    ) -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = rewrite_signed_info(signed_info_xml(XML_C14N_1_0, &digest, false));
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical rewritten SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<Signature Id="sig-001">{signed_info}<SignatureValue>{signature_value}</SignatureValue>"#,
                "<KeyInfo><KeyValue><ECKeyValue>",
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"></QualifyingProperties></Object>"##,
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

    fn signed_pacs008_xml_with_signed_properties_reference() -> String {
        let signed_properties_id = "signed-props-001";
        let signed_properties = signed_properties_xml(signed_properties_id);
        signed_pacs008_xml_with_signed_properties(signed_properties_id, &signed_properties)
    }

    fn signed_pacs008_xml_with_prefixed_xades_signed_properties() -> String {
        let signed_properties_id = "signed-props-001";
        let signed_properties = signed_properties_xml_prefixed(signed_properties_id, XADES_NS);
        signed_pacs008_xml_with_signed_properties(signed_properties_id, &signed_properties)
    }

    fn signed_pacs008_xml_with_wrong_prefixed_xades_namespace() -> String {
        let signed_properties_id = "signed-props-001";
        let signed_properties =
            signed_properties_xml_prefixed(signed_properties_id, "urn:not-xades");
        signed_pacs008_xml_with_signed_properties(signed_properties_id, &signed_properties)
    }

    fn signed_pacs008_xml_with_wrong_default_xades_namespace() -> String {
        let signed_properties_id = "signed-props-001";
        let signed_properties =
            signed_properties_xml_with_default_namespace(signed_properties_id, "urn:not-xades");
        signed_pacs008_xml_with_signed_properties(signed_properties_id, &signed_properties)
            .replacen(
                r##"<QualifyingProperties Target="#sig-001">"##,
                r##"<QualifyingProperties xmlns="urn:not-xades" Target="#sig-001">"##,
                1,
            )
    }

    fn signed_pacs008_xml_with_signed_properties(
        signed_properties_id: &str,
        signed_properties: &str,
    ) -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let payload_digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let canonical_signed_properties =
            canonicalize_supported_xml(signed_properties).expect("canonical SignedProperties");
        let signed_properties_digest =
            BASE64_STANDARD.encode(Sha256::digest(canonical_signed_properties.as_bytes()));
        let signed_info = signed_info_xml_with_signed_properties_reference(
            XML_C14N_1_0,
            &payload_digest,
            signed_properties_id,
            &signed_properties_digest,
        );
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<Signature Id="sig-001">{signed_info}<SignatureValue>{signature_value}</SignatureValue>"#,
                "<KeyInfo><KeyValue><ECKeyValue>",
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001">"##,
                "{signed_properties}",
                "</QualifyingProperties></Object>",
                "</Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            public_key = public_key,
            signed_properties = signed_properties
        );
        format!(
            "{}{}{}",
            &unsigned[..insertion],
            signature_xml,
            &unsigned[insertion..]
        )
    }

    fn signed_properties_xml(id: &str) -> String {
        format!(
            concat!(
                r#"<SignedProperties Id="{id}"><SignedSignatureProperties>"#,
                "<SigningTime>{signing_time}</SigningTime>",
                "</SignedSignatureProperties></SignedProperties>"
            ),
            id = id,
            signing_time = XML_SIGNATURE_TEST_SIGNING_TIME
        )
    }

    fn signed_properties_xml_prefixed(id: &str, xades_namespace: &str) -> String {
        format!(
            concat!(
                r#"<xades:SignedProperties xmlns:xades="{xades_namespace}" Id="{id}">"#,
                "<xades:SignedSignatureProperties>",
                "<xades:SigningTime>{signing_time}</xades:SigningTime>",
                "</xades:SignedSignatureProperties>",
                "</xades:SignedProperties>"
            ),
            id = id,
            signing_time = XML_SIGNATURE_TEST_SIGNING_TIME,
            xades_namespace = xades_namespace
        )
    }

    fn signed_properties_xml_with_default_namespace(id: &str, xades_namespace: &str) -> String {
        format!(
            concat!(
                r#"<SignedProperties xmlns="{xades_namespace}" Id="{id}">"#,
                "<SignedSignatureProperties>",
                "<SigningTime>{signing_time}</SigningTime>",
                "</SignedSignatureProperties>",
                "</SignedProperties>"
            ),
            id = id,
            signing_time = XML_SIGNATURE_TEST_SIGNING_TIME,
            xades_namespace = xades_namespace
        )
    }

    fn signed_properties_xml_with_signing_certificate_v2(
        id: &str,
        leaf_der: &[u8],
    ) -> (String, String) {
        let leaf_digest = BASE64_STANDARD.encode(Sha256::digest(leaf_der));
        (
            format!(
                concat!(
                    r#"<SignedProperties Id="{id}"><SignedSignatureProperties>"#,
                    "<SigningTime>{signing_time}</SigningTime>",
                    "<SigningCertificateV2><Cert><CertDigest>",
                    r#"<DigestMethod Algorithm="{sha256}"></DigestMethod>"#,
                    "<DigestValue>{leaf_digest}</DigestValue>",
                    "</CertDigest></Cert></SigningCertificateV2>",
                    "</SignedSignatureProperties></SignedProperties>"
                ),
                id = id,
                signing_time = XML_SIGNATURE_TEST_SIGNING_TIME,
                sha256 = XMLDSIG_SHA256,
                leaf_digest = leaf_digest
            ),
            leaf_digest,
        )
    }

    fn signed_properties_xml_with_signing_certificate_v2_digest_bytes(
        id: &str,
        digests: &[[u8; 32]],
    ) -> String {
        let certs = digests
            .iter()
            .map(|digest| {
                let digest = BASE64_STANDARD.encode(digest);
                format!(
                    concat!(
                        "<Cert><CertDigest>",
                        r#"<DigestMethod Algorithm="{sha256}"></DigestMethod>"#,
                        "<DigestValue>{digest}</DigestValue>",
                        "</CertDigest></Cert>"
                    ),
                    sha256 = XMLDSIG_SHA256,
                    digest = digest
                )
            })
            .collect::<String>();
        format!(
            concat!(
                r#"<SignedProperties Id="{id}"><SignedSignatureProperties>"#,
                "<SigningTime>{signing_time}</SigningTime>",
                "<SigningCertificateV2>{certs}</SigningCertificateV2>",
                "</SignedSignatureProperties></SignedProperties>"
            ),
            id = id,
            signing_time = XML_SIGNATURE_TEST_SIGNING_TIME,
            certs = certs
        )
    }

    struct CertificateChainSignedPropertiesPayload {
        payload: String,
        issuer_sha256: String,
        signing_certificate_digest: String,
    }

    fn signed_pacs008_xml_with_certificate_chain_signed_properties_reference()
    -> CertificateChainSignedPropertiesPayload {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let payload_digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));

        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
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
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");
        let signed_properties_id = "signed-props-001";
        let (signed_properties, signing_certificate_digest) =
            signed_properties_xml_with_signing_certificate_v2(
                signed_properties_id,
                leaf_cert.der().as_ref(),
            );
        let canonical_signed_properties =
            canonicalize_supported_xml(&signed_properties).expect("canonical SignedProperties");
        let signed_properties_digest =
            BASE64_STANDARD.encode(Sha256::digest(canonical_signed_properties.as_bytes()));
        let signed_info = signed_info_xml_with_signed_properties_reference(
            XML_C14N_1_0,
            &payload_digest,
            signed_properties_id,
            &signed_properties_digest,
        );
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical SignedInfo");
        let signature = leaf_key
            .sign(canonical_signed_info.as_bytes())
            .expect("leaf XMLDSig signature");
        let signature_value =
            BASE64_STANDARD.encode(low_s_p256_signature_der_from_bytes(&signature));
        let leaf_certificate = BASE64_STANDARD.encode(leaf_cert.der().as_ref());
        let issuer_certificate = BASE64_STANDARD.encode(issuer_cert.der().as_ref());
        let signature_xml = format!(
            concat!(
                r#"<Signature Id="sig-001">{signed_info}<SignatureValue>{signature_value}</SignatureValue>"#,
                "<KeyInfo><X509Data>",
                "<X509Certificate>{leaf_certificate}</X509Certificate>",
                "<X509Certificate>{issuer_certificate}</X509Certificate>",
                "</X509Data></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001">"##,
                "{signed_properties}",
                "</QualifyingProperties></Object>",
                "</Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            leaf_certificate = leaf_certificate,
            issuer_certificate = issuer_certificate,
            signed_properties = signed_properties
        );
        CertificateChainSignedPropertiesPayload {
            payload: format!(
                "{}{}{}",
                &unsigned[..insertion],
                signature_xml,
                &unsigned[insertion..]
            ),
            issuer_sha256,
            signing_certificate_digest,
        }
    }

    fn signed_pacs008_xml_with_comments() -> String {
        let unsigned = unsigned_pacs008_xml_with_comments();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned =
            canonicalize_supported_xml(&unsigned).expect("canonical commented payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = signed_info_xml(XML_C14N_1_0, &digest, false)
            .replace("<Reference", "<!--signed-info comment--><Reference")
            .replace("</SignedInfo>", "<!--signed-info tail--></SignedInfo>");
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical commented SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
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
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"></QualifyingProperties></Object>"##,
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

    fn unsigned_pacs008_xml_with_comments() -> String {
        unsigned_pacs008_xml()
            .replace(
                "<FIToFICstmrCdtTrf>",
                "<!--document comment--><FIToFICstmrCdtTrf><!--payment comment-->",
            )
            .replace(
                "<MsgId>sig-001</MsgId>",
                "<!--group header comment--><MsgId>sig-001</MsgId>",
            )
            .replace(
                "</CdtTrfTxInf>",
                "<!--transaction tail comment--></CdtTrfTxInf>",
            )
    }

    fn signed_pacs008_xml_with_character_reference_message_id() -> String {
        signed_pacs008_xml_from_unsigned(
            unsigned_pacs008_xml_with_message_id("sig&amp;001"),
            XML_C14N_1_0,
            false,
            true,
        )
    }

    fn unsigned_pacs008_xml_with_xml_namespace_attribute() -> String {
        unsigned_pacs008_xml().replace("<GrpHdr>", r#"<GrpHdr xml:lang="en">"#)
    }

    fn unsigned_pacs008_xml_with_prefixed_attribute() -> String {
        unsigned_pacs008_xml().replace(
            "<GrpHdr>",
            r#"<GrpHdr xmlns:pay="urn:iso:test:pay" pay:scope="retail">"#,
        )
    }

    fn signed_pacs008_xml_with_xml_namespace_attribute() -> String {
        signed_pacs008_xml_from_unsigned(
            unsigned_pacs008_xml_with_xml_namespace_attribute(),
            XML_C14N_1_0,
            false,
            true,
        )
    }

    fn signed_pacs008_xml_with_prefixed_attribute() -> String {
        signed_pacs008_xml_from_unsigned(
            unsigned_pacs008_xml_with_prefixed_attribute(),
            XML_C14N_1_0,
            false,
            true,
        )
    }

    fn signed_pacs008_xml_with_signed_info_xml_namespace_attribute() -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = signed_info_xml(XML_C14N_1_0, &digest, false).replacen(
            "<SignedInfo>",
            r#"<SignedInfo xml:lang="en">"#,
            1,
        );
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical xml-attribute SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
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
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"></QualifyingProperties></Object>"##,
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

    fn signed_pacs008_xml_with_signed_info_inherited_prefixed_attribute() -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = signed_info_xml(XML_EXCLUSIVE_C14N_1_0, &digest, false).replacen(
            "<SignedInfo>",
            r#"<SignedInfo ext:role="authorization">"#,
            1,
        );
        let inherited_namespaces = vec![CanonicalXmlAttribute {
            name: "xmlns:ext".to_owned(),
            value: "urn:iso:test:ext".to_owned(),
            kind: CanonicalXmlAttributeKind::Namespace,
        }];
        let canonical_signed_info = canonicalize_supported_xml_with_mode(
            &signed_info,
            &inherited_namespaces,
            CanonicalXmlMode::Exclusive,
        )
        .expect("canonical inherited prefixed-attribute SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<Signature xmlns:ext="urn:iso:test:ext">{signed_info}"#,
                "<SignatureValue>{signature_value}</SignatureValue>",
                "<KeyInfo><KeyValue><ECKeyValue>",
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"></QualifyingProperties></Object>"##,
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

    fn signed_pacs008_xml_with_prefixed_signature_namespace() -> String {
        signed_pacs008_xml_with_prefixed_signature_namespace_options(XML_C14N_1_0, false, true)
    }

    fn signed_pacs008_xml_with_wrong_prefixed_signature_namespace() -> String {
        signed_pacs008_xml_with_prefixed_signature_namespace_options_and_namespace(
            XML_C14N_1_0,
            false,
            true,
            "urn:not-xmldsig",
        )
    }

    fn signed_pacs008_xml_with_wrong_default_signature_namespace() -> String {
        signed_pacs008_xml_with_default_signature_namespace("urn:not-xmldsig")
    }

    fn signed_pacs008_xml_with_default_signature_namespace(xmldsig_namespace: &str) -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = signed_info_xml(XML_C14N_1_0, &digest, false);
        let inherited_namespaces = [CanonicalXmlAttribute {
            name: "xmlns".to_owned(),
            value: xmldsig_namespace.to_owned(),
            kind: CanonicalXmlAttributeKind::Namespace,
        }];
        let canonical_signed_info = canonicalize_supported_xml_with_mode(
            &signed_info,
            &inherited_namespaces,
            CanonicalXmlMode::Inclusive,
        )
        .expect("canonical default-namespaced SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<Signature xmlns="{xmldsig_ns}" Id="sig-001">{signed_info}"#,
                "<SignatureValue>{signature_value}</SignatureValue>",
                "<KeyInfo><KeyValue><ECKeyValue>",
                r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></NamedCurve>"#,
                "<PublicKey>{public_key}</PublicKey>",
                "</ECKeyValue></KeyValue></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001"></QualifyingProperties></Object>"##,
                "</Signature>"
            ),
            xmldsig_ns = xmldsig_namespace,
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

    fn signed_pacs008_xml_with_inclusive_unused_signature_namespace() -> String {
        signed_pacs008_xml_with_prefixed_signature_namespace_options(XML_C14N_1_0, true, true)
    }

    fn signed_pacs008_xml_with_exclusive_bytes_declared_as_inclusive() -> String {
        signed_pacs008_xml_with_prefixed_signature_namespace_options(XML_C14N_1_0, true, false)
    }

    fn signed_pacs008_xml_with_prefixed_signature_namespace_options(
        c14n_algorithm: &str,
        include_unused_namespace: bool,
        sign_with_declared_c14n_mode: bool,
    ) -> String {
        signed_pacs008_xml_with_prefixed_signature_namespace_options_and_namespace(
            c14n_algorithm,
            include_unused_namespace,
            sign_with_declared_c14n_mode,
            XMLDSIG_NS,
        )
    }

    fn signed_pacs008_xml_with_prefixed_signature_namespace_options_and_namespace(
        c14n_algorithm: &str,
        include_unused_namespace: bool,
        sign_with_declared_c14n_mode: bool,
        xmldsig_namespace: &str,
    ) -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));
        let signed_info = signed_info_xml_prefixed(c14n_algorithm, &digest);
        let mut inherited_namespaces = vec![CanonicalXmlAttribute {
            name: "xmlns:ds".to_owned(),
            value: xmldsig_namespace.to_owned(),
            kind: CanonicalXmlAttributeKind::Namespace,
        }];
        if include_unused_namespace {
            inherited_namespaces.push(CanonicalXmlAttribute {
                name: "xmlns:unused".to_owned(),
                value: "urn:unused".to_owned(),
                kind: CanonicalXmlAttributeKind::Namespace,
            });
        }
        let signing_mode = if sign_with_declared_c14n_mode {
            xml_canonicalization_mode(c14n_algorithm).expect("test canonicalization algorithm")
        } else {
            CanonicalXmlMode::Exclusive
        };
        let canonical_signed_info =
            canonicalize_supported_xml_with_mode(&signed_info, &inherited_namespaces, signing_mode)
                .expect("canonical prefixed SignedInfo");
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let signature_xml = format!(
            concat!(
                r#"<ds:Signature xmlns:ds="{xmldsig_ns}"{unused_namespace}>{signed_info}"#,
                "<ds:SignatureValue>{signature_value}</ds:SignatureValue>",
                "<ds:KeyInfo><ds:KeyValue><ds:ECKeyValue>",
                r#"<ds:NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"></ds:NamedCurve>"#,
                "<ds:PublicKey>{public_key}</ds:PublicKey>",
                "</ds:ECKeyValue></ds:KeyValue></ds:KeyInfo>",
                r##"<ds:Object><xades:QualifyingProperties xmlns:xades="http://uri.etsi.org/01903/v1.3.2#" Target="#sig-001"></xades:QualifyingProperties></ds:Object>"##,
                "</ds:Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            public_key = public_key,
            xmldsig_ns = xmldsig_namespace,
            unused_namespace = if include_unused_namespace {
                r#" xmlns:unused="urn:unused""#
            } else {
                ""
            }
        );
        format!(
            "{}{}{}",
            &unsigned[..insertion],
            signature_xml,
            &unsigned[insertion..]
        )
    }

    fn signed_pacs008_xml_with_c14n_algorithm_and_signed_info_style(
        c14n_algorithm: &str,
        self_closing_signed_info_methods: bool,
    ) -> String {
        signed_pacs008_xml_with_c14n_algorithm_and_signed_info_options(
            c14n_algorithm,
            self_closing_signed_info_methods,
            true,
        )
    }

    fn signed_pacs008_xml_with_c14n_algorithm_and_signed_info_options(
        c14n_algorithm: &str,
        self_closing_signed_info_methods: bool,
        sign_canonical_signed_info: bool,
    ) -> String {
        signed_pacs008_xml_from_unsigned(
            unsigned_pacs008_xml(),
            c14n_algorithm,
            self_closing_signed_info_methods,
            sign_canonical_signed_info,
        )
    }

    fn signed_pacs008_xml_from_unsigned(
        unsigned: String,
        c14n_algorithm: &str,
        self_closing_signed_info_methods: bool,
        sign_canonical_signed_info: bool,
    ) -> String {
        signed_pacs008_xml_from_unsigned_with_signature_encoding(
            unsigned,
            c14n_algorithm,
            self_closing_signed_info_methods,
            sign_canonical_signed_info,
            XmlSignatureValueEncoding::Der,
        )
    }

    fn signed_pacs008_xml_from_unsigned_with_signature_encoding(
        unsigned: String,
        c14n_algorithm: &str,
        self_closing_signed_info_methods: bool,
        sign_canonical_signed_info: bool,
        signature_encoding: XmlSignatureValueEncoding,
    ) -> String {
        signed_pacs008_xml_from_unsigned_with_reference_uri_and_signature_encoding(
            unsigned,
            c14n_algorithm,
            "",
            self_closing_signed_info_methods,
            sign_canonical_signed_info,
            signature_encoding,
        )
    }

    fn signed_pacs008_xml_from_unsigned_with_reference_uri_and_signature_encoding(
        unsigned: String,
        c14n_algorithm: &str,
        reference_uri: &str,
        self_closing_signed_info_methods: bool,
        sign_canonical_signed_info: bool,
        signature_encoding: XmlSignatureValueEncoding,
    ) -> String {
        signed_pacs008_xml_from_unsigned_with_reference_options(
            unsigned,
            c14n_algorithm,
            reference_uri,
            self_closing_signed_info_methods,
            sign_canonical_signed_info,
            signature_encoding,
            None,
        )
    }

    fn signed_pacs008_xml_from_unsigned_with_reference_options(
        unsigned: String,
        c14n_algorithm: &str,
        reference_uri: &str,
        self_closing_signed_info_methods: bool,
        sign_canonical_signed_info: bool,
        signature_encoding: XmlSignatureValueEncoding,
        reference_c14n_algorithm: Option<&str>,
    ) -> String {
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let reference_target =
            xml_signature_reference_target(&unsigned, reference_uri).expect("reference target");
        let reference_c14n_mode = reference_c14n_algorithm
            .map(xml_canonicalization_mode)
            .transpose()
            .expect("reference canonicalization algorithm")
            .unwrap_or(CanonicalXmlMode::Exclusive);
        let canonical_reference = canonicalize_supported_xml_with_mode(
            reference_target.xml,
            &reference_target.inherited_namespaces,
            reference_c14n_mode,
        )
        .expect("canonical payload");
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical_reference.as_bytes()));
        let signed_info = signed_info_xml_with_reference_options(
            c14n_algorithm,
            &digest,
            reference_uri,
            self_closing_signed_info_methods,
            reference_c14n_algorithm,
        );
        let canonical_signed_info = if sign_canonical_signed_info {
            canonicalize_supported_xml(&signed_info).expect("canonical SignedInfo")
        } else {
            signed_info.clone()
        };
        let signing_key = xml_signature_test_signing_key();
        let signature = low_s_p256_signature(signing_key.sign(canonical_signed_info.as_bytes()));
        let signature_value = encode_p256_signature_value(&signature, signature_encoding);
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

    fn encode_p256_signature_value(
        signature: &P256Signature,
        encoding: XmlSignatureValueEncoding,
    ) -> String {
        match encoding {
            XmlSignatureValueEncoding::Der => BASE64_STANDARD.encode(signature.to_der().as_bytes()),
            XmlSignatureValueEncoding::FixedWidth => {
                BASE64_STANDARD.encode(signature.to_bytes().as_slice())
            }
        }
    }

    fn signed_info_xml(c14n_algorithm: &str, digest: &str, self_closing_methods: bool) -> String {
        signed_info_xml_with_reference_uri(c14n_algorithm, digest, "", self_closing_methods)
    }

    fn signed_info_xml_with_reference_uri(
        c14n_algorithm: &str,
        digest: &str,
        reference_uri: &str,
        self_closing_methods: bool,
    ) -> String {
        signed_info_xml_with_reference_options(
            c14n_algorithm,
            digest,
            reference_uri,
            self_closing_methods,
            None,
        )
    }

    fn signed_info_xml_with_reference_options(
        c14n_algorithm: &str,
        digest: &str,
        reference_uri: &str,
        self_closing_methods: bool,
        reference_c14n_algorithm: Option<&str>,
    ) -> String {
        if self_closing_methods {
            let c14n_transform = reference_c14n_algorithm
                .map(|algorithm| format!(r#"<Transform Algorithm="{algorithm}"/>"#))
                .unwrap_or_default();
            return format!(
                r#"<SignedInfo><CanonicalizationMethod Algorithm="{c14n_algorithm}"/><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"/><Reference URI="{reference_uri}"><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"/>{c14n_transform}</Transforms><DigestMethod Algorithm="{XMLDSIG_SHA256}"/><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
            );
        }
        let c14n_transform = reference_c14n_algorithm
            .map(|algorithm| format!(r#"<Transform Algorithm="{algorithm}"></Transform>"#))
            .unwrap_or_default();
        format!(
            r#"<SignedInfo><CanonicalizationMethod Algorithm="{c14n_algorithm}"></CanonicalizationMethod><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod><Reference URI="{reference_uri}"><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform>{c14n_transform}</Transforms><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
        )
    }

    fn signed_info_xml_with_signed_properties_reference(
        c14n_algorithm: &str,
        payload_digest: &str,
        signed_properties_id: &str,
        signed_properties_digest: &str,
    ) -> String {
        format!(
            concat!(
                r#"<SignedInfo><CanonicalizationMethod Algorithm="{c14n_algorithm}"></CanonicalizationMethod>"#,
                r#"<SignatureMethod Algorithm="{ecdsa_sha256}"></SignatureMethod>"#,
                r#"<Reference URI=""><Transforms><Transform Algorithm="{enveloped_signature}"></Transform></Transforms>"#,
                r#"<DigestMethod Algorithm="{sha256}"></DigestMethod><DigestValue>{payload_digest}</DigestValue></Reference>"#,
                r##"<Reference URI="#{signed_properties_id}" Type="{signed_properties_type}">"##,
                r#"<Transforms><Transform Algorithm="{exclusive_c14n}"></Transform></Transforms>"#,
                r#"<DigestMethod Algorithm="{sha256}"></DigestMethod><DigestValue>{signed_properties_digest}</DigestValue></Reference>"#,
                "</SignedInfo>"
            ),
            c14n_algorithm = c14n_algorithm,
            ecdsa_sha256 = XMLDSIG_ECDSA_SHA256,
            enveloped_signature = XMLDSIG_ENVELOPED_SIGNATURE,
            sha256 = XMLDSIG_SHA256,
            payload_digest = payload_digest,
            signed_properties_id = signed_properties_id,
            signed_properties_type = XADES_SIGNED_PROPERTIES_TYPE,
            exclusive_c14n = XML_EXCLUSIVE_C14N_1_0,
            signed_properties_digest = signed_properties_digest
        )
    }

    fn signed_info_xml_prefixed(c14n_algorithm: &str, digest: &str) -> String {
        format!(
            r#"<ds:SignedInfo><ds:CanonicalizationMethod Algorithm="{c14n_algorithm}"></ds:CanonicalizationMethod><ds:SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></ds:SignatureMethod><ds:Reference URI=""><ds:Transforms><ds:Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></ds:Transform></ds:Transforms><ds:DigestMethod Algorithm="{XMLDSIG_SHA256}"></ds:DigestMethod><ds:DigestValue>{digest}</ds:DigestValue></ds:Reference></ds:SignedInfo>"#
        )
    }

    struct CertificateChainSignedPayload {
        payload: String,
        leaf_sha256: String,
        issuer_sha256: String,
    }

    #[derive(Clone, Copy)]
    enum CertificateChainValidity {
        Valid,
        ExpiredLeaf,
        FutureIssuer,
    }

    #[derive(Clone, Copy)]
    enum CertificateChainIssuerName {
        Matching,
        MismatchedLeafIssuer,
    }

    #[derive(Clone, Copy)]
    enum CertificateChainCriticalUnknownExtension {
        Leaf,
        Issuer,
    }

    #[derive(Clone, Copy)]
    enum CertificateChainCriticalUnsupportedParsedExtension {
        Leaf,
        Issuer,
    }

    #[derive(Clone, Copy)]
    enum XmlSignatureValueEncoding {
        Der,
        FixedWidth,
    }

    fn signed_pacs008_xml_with_certificate_chain() -> CertificateChainSignedPayload {
        signed_pacs008_xml_with_certificate_chain_policy(true, true, true)
    }

    fn signed_pacs008_xml_with_certificate_chain_policy(
        issuer_is_ca: bool,
        issuer_key_cert_sign: bool,
        leaf_digital_signature: bool,
    ) -> CertificateChainSignedPayload {
        signed_pacs008_xml_with_certificate_chain_options(
            issuer_is_ca,
            issuer_key_cert_sign,
            leaf_digital_signature,
            CertificateChainValidity::Valid,
            CertificateChainIssuerName::Matching,
            false,
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_options(
        issuer_is_ca: bool,
        issuer_key_cert_sign: bool,
        leaf_digital_signature: bool,
        validity: CertificateChainValidity,
        issuer_name: CertificateChainIssuerName,
        leaf_is_ca: bool,
    ) -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = if issuer_is_ca {
            IsCa::Ca(BasicConstraints::Unconstrained)
        } else {
            IsCa::NoCa
        };
        issuer_params.not_before = match validity {
            CertificateChainValidity::FutureIssuer => date_time_ymd(2026, 1, 1),
            CertificateChainValidity::Valid | CertificateChainValidity::ExpiredLeaf => {
                date_time_ymd(2020, 1, 1)
            }
        };
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = if issuer_key_cert_sign {
            vec![
                KeyUsagePurpose::DigitalSignature,
                KeyUsagePurpose::KeyCertSign,
            ]
        } else {
            vec![KeyUsagePurpose::DigitalSignature]
        };
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let signing_issuer_params = match issuer_name {
            CertificateChainIssuerName::Matching => issuer_params.clone(),
            CertificateChainIssuerName::MismatchedLeafIssuer => {
                let mut params = issuer_params.clone();
                params.distinguished_name = rcgen::DistinguishedName::new();
                params
                    .distinguished_name
                    .push(DnType::CommonName, "ISO Bridge Mismatched Leaf Issuer");
                params
            }
        };
        let issuer = Issuer::new(signing_issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.is_ca = if leaf_is_ca {
            IsCa::Ca(BasicConstraints::Unconstrained)
        } else {
            IsCa::NoCa
        };
        leaf_params.key_usages = if leaf_digital_signature {
            vec![KeyUsagePurpose::DigitalSignature]
        } else {
            vec![KeyUsagePurpose::KeyEncipherment]
        };
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = match validity {
            CertificateChainValidity::ExpiredLeaf => date_time_ymd(2024, 1, 1),
            CertificateChainValidity::Valid | CertificateChainValidity::FutureIssuer => {
                date_time_ymd(2030, 1, 1)
            }
        };
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_compressed_leaf_certificate_spki() -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let compressed_leaf_spki = compressed_p256_subject_public_key_info(&leaf_key);
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&compressed_leaf_spki, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_compressed_issuer_certificate_spki() -> CertificateChainSignedPayload
    {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let compressed_issuer_spki = compressed_p256_subject_public_key_info(&issuer_key);
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let issuer = Issuer::new(issuer_params.clone(), issuer_key);
        let issuer_cert = issuer_params
            .signed_by(&compressed_issuer_spki, &issuer)
            .expect("issuer certificate");

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    struct CompressedP256PublicKey {
        bytes: Vec<u8>,
    }

    impl PublicKeyData for CompressedP256PublicKey {
        fn der_bytes(&self) -> &[u8] {
            &self.bytes
        }

        fn algorithm(&self) -> &'static SignatureAlgorithm {
            &PKCS_ECDSA_P256_SHA256
        }
    }

    fn compressed_p256_subject_public_key_info(key_pair: &RcgenKeyPair) -> CompressedP256PublicKey {
        let compressed_point = P256VerifyingKey::from_sec1_bytes(key_pair.public_key_raw())
            .expect("rcgen P-256 public key")
            .to_encoded_point(true);
        CompressedP256PublicKey {
            bytes: compressed_point.as_bytes().to_vec(),
        }
    }

    fn signed_pacs008_xml_with_three_certificate_chain(
        root_path_len: u8,
    ) -> CertificateChainSignedPayload {
        let root_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("root key");
        let mut root_params =
            CertificateParams::new(vec!["iso-root.example".to_owned()]).expect("root params");
        root_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Root");
        root_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(root_path_len));
        root_params.not_before = date_time_ymd(2020, 1, 1);
        root_params.not_after = date_time_ymd(2030, 1, 1);
        root_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let root_cert = root_params
            .self_signed(&root_key)
            .expect("root certificate");
        let root_issuer = Issuer::new(root_params, root_key);

        let intermediate_key =
            RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("intermediate key");
        let mut intermediate_params =
            CertificateParams::new(vec!["iso-intermediate.example".to_owned()])
                .expect("intermediate params");
        intermediate_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Intermediate");
        intermediate_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        intermediate_params.not_before = date_time_ymd(2020, 1, 1);
        intermediate_params.not_after = date_time_ymd(2030, 1, 1);
        intermediate_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let intermediate_cert = intermediate_params
            .signed_by(&intermediate_key, &root_issuer)
            .expect("intermediate certificate");
        let intermediate_issuer = Issuer::new(intermediate_params, intermediate_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &intermediate_issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[
                leaf_cert.der().as_ref(),
                intermediate_cert.der().as_ref(),
                root_cert.der().as_ref(),
            ],
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_material(
        leaf_key: &RcgenKeyPair,
        certificates_der: &[&[u8]],
    ) -> CertificateChainSignedPayload {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let canonical_unsigned = canonicalize_supported_xml(&unsigned).expect("canonical payload");
        let payload_digest = BASE64_STANDARD.encode(Sha256::digest(canonical_unsigned.as_bytes()));

        let signed_properties_id = "signed-props-001";
        let (signed_properties, _) = signed_properties_xml_with_signing_certificate_v2(
            signed_properties_id,
            certificates_der[0],
        );
        let canonical_signed_properties =
            canonicalize_supported_xml(&signed_properties).expect("canonical SignedProperties");
        let signed_properties_digest =
            BASE64_STANDARD.encode(Sha256::digest(canonical_signed_properties.as_bytes()));
        let signed_info = signed_info_xml_with_signed_properties_reference(
            XML_C14N_1_0,
            &payload_digest,
            signed_properties_id,
            &signed_properties_digest,
        );
        let canonical_signed_info =
            canonicalize_supported_xml(&signed_info).expect("canonical SignedInfo");
        let signature = leaf_key
            .sign(canonical_signed_info.as_bytes())
            .expect("leaf XMLDSig signature");
        let signature_value =
            BASE64_STANDARD.encode(low_s_p256_signature_der_from_bytes(&signature));
        let certificates_xml = certificates_der
            .iter()
            .map(|certificate_der| {
                format!(
                    "<X509Certificate>{}</X509Certificate>",
                    BASE64_STANDARD.encode(certificate_der)
                )
            })
            .collect::<String>();
        let signature_xml = format!(
            concat!(
                r#"<Signature Id="sig-001">{signed_info}<SignatureValue>{signature_value}</SignatureValue>"#,
                "<KeyInfo><X509Data>",
                "{certificates_xml}",
                "</X509Data></KeyInfo>",
                r##"<Object><QualifyingProperties Target="#sig-001">"##,
                "{signed_properties}",
                "</QualifyingProperties></Object>",
                "</Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            certificates_xml = certificates_xml,
            signed_properties = signed_properties
        );
        CertificateChainSignedPayload {
            payload: format!(
                "{}{}{}",
                &unsigned[..insertion],
                signature_xml,
                &unsigned[insertion..]
            ),
            leaf_sha256: sha256_hex(certificates_der[0]),
            issuer_sha256: sha256_hex(certificates_der[certificates_der.len() - 1]),
        }
    }

    fn signed_pacs008_xml_with_certificate_chain_issuer_name_mismatch()
    -> CertificateChainSignedPayload {
        signed_pacs008_xml_with_certificate_chain_options(
            true,
            true,
            true,
            CertificateChainValidity::Valid,
            CertificateChainIssuerName::MismatchedLeafIssuer,
            false,
        )
    }

    fn signed_pacs008_xml_with_leaf_ca_certificate_chain() -> CertificateChainSignedPayload {
        signed_pacs008_xml_with_certificate_chain_options(
            true,
            true,
            true,
            CertificateChainValidity::Valid,
            CertificateChainIssuerName::Matching,
            true,
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_noncritical_issuer_basic_constraints()
    -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        issuer_params
            .custom_extensions
            .push(noncritical_basic_constraints_ca_extension());
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_noncritical_issuer_key_usage()
    -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params
            .custom_extensions
            .push(noncritical_key_usage_digital_signature_key_cert_sign_extension());
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_p384_issuer_certificate_chain() -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P384_SHA384).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_noncritical_leaf_key_usage()
    -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params
            .custom_extensions
            .push(noncritical_key_usage_digital_signature_extension());
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_critical_unknown_extension(
        target: CertificateChainCriticalUnknownExtension,
    ) -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        if matches!(target, CertificateChainCriticalUnknownExtension::Issuer) {
            issuer_params
                .custom_extensions
                .push(critical_unknown_x509_extension());
        }
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        if matches!(target, CertificateChainCriticalUnknownExtension::Leaf) {
            leaf_params
                .custom_extensions
                .push(critical_unknown_x509_extension());
        }
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn signed_pacs008_xml_with_certificate_chain_critical_unsupported_parsed_extension(
        target: CertificateChainCriticalUnsupportedParsedExtension,
    ) -> CertificateChainSignedPayload {
        let issuer_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("issuer key");
        let mut issuer_params =
            CertificateParams::new(vec!["iso-issuer.example".to_owned()]).expect("issuer params");
        issuer_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Issuer");
        issuer_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        issuer_params.not_before = date_time_ymd(2020, 1, 1);
        issuer_params.not_after = date_time_ymd(2030, 1, 1);
        issuer_params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
        ];
        if matches!(
            target,
            CertificateChainCriticalUnsupportedParsedExtension::Issuer
        ) {
            issuer_params
                .custom_extensions
                .push(critical_extended_key_usage_x509_extension());
        }
        let issuer_cert = issuer_params
            .self_signed(&issuer_key)
            .expect("issuer certificate");
        let issuer = Issuer::new(issuer_params, issuer_key);

        let leaf_key = RcgenKeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).expect("leaf key");
        let mut leaf_params =
            CertificateParams::new(vec!["iso-leaf.example".to_owned()]).expect("leaf params");
        leaf_params
            .distinguished_name
            .push(DnType::CommonName, "ISO Bridge Test Leaf");
        leaf_params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        leaf_params.not_before = date_time_ymd(2020, 1, 1);
        leaf_params.not_after = date_time_ymd(2030, 1, 1);
        if matches!(
            target,
            CertificateChainCriticalUnsupportedParsedExtension::Leaf
        ) {
            leaf_params
                .custom_extensions
                .push(critical_extended_key_usage_x509_extension());
        }
        let leaf_cert = leaf_params
            .signed_by(&leaf_key, &issuer)
            .expect("leaf certificate");

        signed_pacs008_xml_with_certificate_chain_material(
            &leaf_key,
            &[leaf_cert.der().as_ref(), issuer_cert.der().as_ref()],
        )
    }

    fn critical_unknown_x509_extension() -> CustomExtension {
        let mut extension =
            CustomExtension::from_oid_content(&[1, 3, 6, 1, 4, 1, 55555, 42], vec![0x05, 0x00]);
        extension.set_criticality(true);
        extension
    }

    fn critical_extended_key_usage_x509_extension() -> CustomExtension {
        let mut extension = CustomExtension::from_oid_content(
            &[2, 5, 29, 37],
            vec![
                0x30, 0x0a, 0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x01,
            ],
        );
        extension.set_criticality(true);
        extension
    }

    fn noncritical_basic_constraints_ca_extension() -> CustomExtension {
        CustomExtension::from_oid_content(&[2, 5, 29, 19], vec![0x30, 0x03, 0x01, 0x01, 0xff])
    }

    fn noncritical_key_usage_digital_signature_key_cert_sign_extension() -> CustomExtension {
        CustomExtension::from_oid_content(&[2, 5, 29, 15], vec![0x03, 0x02, 0x02, 0x84])
    }

    fn noncritical_key_usage_digital_signature_extension() -> CustomExtension {
        CustomExtension::from_oid_content(&[2, 5, 29, 15], vec![0x03, 0x02, 0x07, 0x80])
    }

    fn assert_pinned_certificate_chain_rejected(
        fixture: CertificateChainSignedPayload,
        expected: &str,
    ) {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![fixture.issuer_sha256];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed =
            parse_message("pacs.008", fixture.payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, fixture.payload.as_bytes())
            .expect_err(expected);

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    fn assert_pinned_certificate_chain_accepted(
        fixture: CertificateChainSignedPayload,
        expected: &str,
    ) {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![fixture.issuer_sha256];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed =
            parse_message("pacs.008", fixture.payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, fixture.payload.as_bytes())
            .expect(expected);

        assert!(metadata.embedded_signature_detected());
    }

    fn assert_require_verified_signed_payload_accepted(payload: String, expected: &str) {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect(expected);

        assert!(metadata.embedded_signature_detected());
    }

    fn assert_require_verified_signed_payload_rejected(payload: String, expected: &str) {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err(expected);

        assert!(matches!(err, MsgError::ValidationFailed));
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
    fn runtime_from_config_rejects_noncanonical_xml_signature_revocation_pin() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.revoked_certificate_sha256 = vec!["AB".repeat(32)];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("uppercase SHA-256 revocation pins must fail configuration"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("revoked_certificate_sha256"),
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
    fn profile_validation_records_reference_snapshot_checksum() {
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
        let parsed = sample_pacs008();
        let expected_snapshot_id = runtime.reference_data().snapshot_id();

        let metadata = runtime
            .validate_profile_submission(
                runtime.default_profile(),
                "pacs.008",
                &parsed,
                b"profile payload",
            )
            .expect("generic profile accepts message");

        assert_eq!(
            metadata.reference_snapshot_id(),
            Some(expected_snapshot_id.as_str())
        );
        assert_ne!(
            expected_snapshot_id,
            ReferenceDataSnapshots::from_config(&actual::IsoReferenceData::default()).snapshot_id()
        );
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
    fn profile_validation_rejects_selected_profile_message_type_mismatch() {
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
        let parsed = sample_pacs008();

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, b"profile payload")
            .expect_err("pacs.009 profile must not admit pacs.008 endpoint submissions");

        assert!(matches!(err, MsgError::UnknownMessageType));
    }

    #[test]
    fn live_profile_rejects_app_header_message_definition_mismatch() {
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
            b"DataPDU/AppHdr/BizMsgIdr=HDR-MISMATCH\nDataPDU/AppHdr/MsgDefIdr=pacs.009.001.10\nDataPDU/AppHdr/CreDt=2025-01-01T12:00:00Z\nDataPDU/AppHdr/BizSvc=swift.cbprplus.02\nMsgId=m-profile\nIntrBkSttlmAmt=10.00\nIntrBkSttlmCcy=USD\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB82WEST12345698765432\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("parsed");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, b"profile payload")
            .expect_err("BAH MsgDefIdr must match the selected profile version set");

        assert!(matches!(err, MsgError::UnknownMessageType));
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
    fn require_verified_profile_accepts_xades_signed_properties_reference() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_signed_properties_reference();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("XAdES SignedProperties Reference digest should verify");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_prefixed_xades_signed_properties_reference() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_prefixed_xades_signed_properties();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("prefixed XAdES SignedProperties should bind to the supported namespace");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_rejects_prefixed_xades_with_wrong_namespace() {
        assert_require_verified_signed_payload_rejected(
            signed_pacs008_xml_with_wrong_prefixed_xades_namespace(),
            "prefixed XAdES elements must bind to the supported XAdES namespace",
        );
    }

    #[test]
    fn require_verified_profile_rejects_unprefixed_xades_with_wrong_default_namespace() {
        assert_require_verified_signed_payload_rejected(
            signed_pacs008_xml_with_wrong_default_xades_namespace(),
            "unprefixed XAdES elements must reject wrong default namespaces",
        );
    }

    #[test]
    fn require_verified_profile_rejects_unreferenced_signed_properties() {
        let payload = signed_pacs008_xml().replacen(
            r##"<Object><QualifyingProperties Target="#sig-001"/></Object>"##,
            &format!(
                r##"<Object><QualifyingProperties Target="#sig-001">{}</QualifyingProperties></Object>"##,
                signed_properties_xml("unsigned-props-001")
            ),
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "unreferenced SignedProperties must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_signed_properties() {
        let payload = signed_pacs008_xml_with_signed_properties_reference().replacen(
            "</SignedProperties>",
            &format!(
                "</SignedProperties>{}",
                signed_properties_xml("unsigned-props-002")
            ),
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "duplicate SignedProperties must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_signed_signing_time_unsupported_attribute() {
        let signed_properties_id = "signed-props-001";
        let signed_properties = format!(
            concat!(
                r#"<SignedProperties Id="{id}"><SignedSignatureProperties>"#,
                r#"<SigningTime Source="unsupported">{signing_time}</SigningTime>"#,
                "</SignedSignatureProperties></SignedProperties>"
            ),
            id = signed_properties_id,
            signing_time = XML_SIGNATURE_TEST_SIGNING_TIME
        );
        let payload =
            signed_pacs008_xml_with_signed_properties(signed_properties_id, &signed_properties);

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported signed SigningTime attributes must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_accepts_xades_signing_certificate_v2_binding() {
        let fixture = signed_pacs008_xml_with_certificate_chain_signed_properties_reference();
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![fixture.issuer_sha256];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let parsed =
            parse_message("pacs.008", fixture.payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, fixture.payload.as_bytes())
            .expect("SigningCertificateV2 leaf digest should bind to the XMLDSig leaf certificate");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
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
    fn supported_xml_canonicalizer_expands_empty_elements_and_sorts_attributes() {
        let canonical =
            canonicalize_supported_xml(r#"<A z='last' xmlns="urn:test" a="first"><B /></A>"#)
                .expect("supported XML subset should canonicalize");

        assert_eq!(
            canonical,
            r#"<A xmlns="urn:test" a="first" z="last"><B></B></A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_decodes_character_references() {
        let canonical = canonicalize_supported_xml(
            r#"<A attr='&quot;&amp;&lt;&apos;&#x9;&#10;&#xD;'>one&amp;&lt;&gt;&#13;</A>"#,
        )
        .expect("predefined and numeric XML character references should canonicalize");

        assert_eq!(
            canonical,
            r#"<A attr="&quot;&amp;&lt;'&#x9;&#xA;&#xD;">one&amp;&lt;&gt;&#xD;</A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_accepts_xml_namespace_attributes() {
        let canonical =
            canonicalize_supported_xml(r#"<A z='last' xml:lang="en" a="first" xml:id='id-1'></A>"#)
                .expect("implicit xml namespace attributes are inside the supported subset");

        assert_eq!(
            canonical,
            r#"<A a="first" z="last" xml:id="id-1" xml:lang="en"></A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_omits_legal_xml_namespace_declaration() {
        let canonical = canonicalize_supported_xml(
            r#"<A xmlns:xml="http://www.w3.org/XML/1998/namespace" xml:lang="en"><B xml:id="b-1"></B></A>"#,
        )
        .expect("the fixed XML namespace declaration should be accepted and omitted");

        assert_eq!(canonical, r#"<A xml:lang="en"><B xml:id="b-1"></B></A>"#);

        let inherited_namespaces = vec![CanonicalXmlAttribute {
            name: "xmlns:xml".to_owned(),
            value: XML_NS.to_owned(),
            kind: CanonicalXmlAttributeKind::Namespace,
        }];
        let canonical = canonicalize_supported_xml_with_mode(
            r#"<A xml:space="preserve"></A>"#,
            &inherited_namespaces,
            CanonicalXmlMode::Inclusive,
        )
        .expect("inherited XML namespace declaration should not serialize");

        assert_eq!(canonical, r#"<A xml:space="preserve"></A>"#);
    }

    #[test]
    fn supported_xml_reference_target_resolves_same_document_id() {
        let xml = r#"<A><B xml:id="target"><C Id="other"></C></B></A>"#;
        let target = xml_signature_reference_target(xml, "#target")
            .expect("same-document xml:id reference should resolve");

        assert_eq!(target.xml, r#"<B xml:id="target"><C Id="other"></C></B>"#);
    }

    #[test]
    fn supported_xml_reference_target_carries_inherited_namespaces() {
        let xml = r#"<Root xmlns:p="urn:p"><p:Payload Id="target"><p:Child></p:Child></p:Payload></Root>"#;
        let target = xml_signature_reference_target(xml, "#target")
            .expect("same-document reference target should inherit ancestor namespaces");
        let canonical = canonicalize_supported_xml_with_inherited_namespaces(
            target.xml,
            &target.inherited_namespaces,
        )
        .expect("inherited namespace should canonicalize on referenced root");

        assert_eq!(
            canonical,
            r#"<p:Payload xmlns:p="urn:p" Id="target"><p:Child></p:Child></p:Payload>"#
        );
    }

    #[test]
    fn supported_xml_reference_target_rejects_invalid_same_document_uris() {
        for uri in [
            "target",
            "https://example.invalid/#target",
            "#",
            "#bad:prefix",
        ] {
            let err = xml_signature_reference_target(r#"<A Id="target"></A>"#, uri)
                .expect_err("unsupported Reference URIs must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }

        let err = xml_signature_reference_target(r#"<A Id="dup"><B Id="dup"></B></A>"#, "#dup")
            .expect_err("duplicate same-document IDs must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let err = xml_signature_reference_target(
            r#"<A xmlns:p="urn:test" p:Id="target"></A>"#,
            "#target",
        )
        .expect_err("namespace-qualified non-xml ID attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_reference_transform_accepts_optional_c14n_transform() {
        let valid = format!(
            r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
        );
        assert_eq!(
            supported_xml_signature_reference_c14n_mode(&valid)
                .expect("the supported Reference transform set should pass"),
            CanonicalXmlMode::Exclusive
        );

        let valid_with_c14n = format!(
            r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform><Transform Algorithm="{XML_C14N_1_0}"></Transform></Transforms></Reference>"#
        );
        assert_eq!(
            supported_xml_signature_reference_c14n_mode(&valid_with_c14n)
                .expect("a final supported C14N transform should pass"),
            CanonicalXmlMode::Inclusive
        );

        let valid_with_transform_comments = format!(
            r#"<Reference URI=""><Transforms><!--before--><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform><!--after--></Transforms></Reference>"#
        );
        assert_eq!(
            supported_xml_signature_reference_c14n_mode(&valid_with_transform_comments)
                .expect("comments in no-comments SignedInfo transform wrappers should pass"),
            CanonicalXmlMode::Exclusive
        );

        for reference in [
            format!(
                r#"<Reference URI="" Id="ref-001"><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference ds:URI="" xmlns:ds="{XMLDSIG_NS}"><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            r#"<Reference URI=""><Transforms></Transforms></Reference>"#.to_owned(),
            format!(
                r#"<Reference URI=""><Transforms Id="transforms-001"><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Id="transform-001" Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform ds:Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}" xmlns:ds="{XMLDSIG_NS}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Ignored></Ignored><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform><Ignored></Ignored></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms><DigestValue>AQ==</DigestValue></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms><DigestValue>AQ==</DigestValue><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Object></Object><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XML_C14N_1_0}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XML_C14N_1_0}"></Transform><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></Reference>"#
            ),
            r#"<Reference URI=""><Transforms><Transform Algorithm="urn:unsupported"></Transform></Transforms></Reference>"#
                .to_owned(),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform><Transform Algorithm="urn:unsupported"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"><XPath>not-supported</XPath></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform><Transform Algorithm="{XML_C14N_1_0}"><InclusiveNamespaces PrefixList="unused"></InclusiveNamespaces></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms><Transforms><Transform Algorithm="{XML_C14N_1_0}"></Transform></Transforms></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms><Transform Algorithm="{XML_C14N_1_0}"></Transform></Reference>"#
            ),
            format!(
                r#"<Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform><Transform Algorithm="{XML_C14N_1_0}"></Transform><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"#
            ),
        ] {
            let err = supported_xml_signature_reference_c14n_mode(&reference)
                .expect_err("missing, reordered, extra, or unsupported transforms must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_signature_reference_rejects_wrong_prefixed_namespace() {
        let inherited_namespaces = vec![CanonicalXmlNamespaceBinding {
            prefix: "ds".to_owned(),
            uri: XMLDSIG_NS.to_owned(),
        }];
        let valid = format!(
            r#"<ds:Reference URI=""><ds:Transforms><ds:Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></ds:Transform></ds:Transforms></ds:Reference>"#
        );
        assert_eq!(
            supported_xml_signature_reference_c14n_mode_with_namespaces(
                &valid,
                &inherited_namespaces
            )
            .expect("XMLDSig-prefixed Reference should resolve through inherited namespace"),
            CanonicalXmlMode::Exclusive
        );

        let wrong_transforms = format!(
            r#"<ds:Reference URI=""><bad:Transforms xmlns:bad="urn:not-xmldsig"><ds:Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></ds:Transform></bad:Transforms></ds:Reference>"#
        );
        let err = supported_xml_signature_reference_c14n_mode_with_namespaces(
            &wrong_transforms,
            &inherited_namespaces,
        )
        .expect_err("Transforms bound outside XMLDSig must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let canonical = "<Document></Document>";
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical.as_bytes()));
        let wrong_digest_method = format!(
            r#"<ds:Reference URI=""><ds:DigestMethod xmlns:bad="urn:not-xmldsig" Algorithm="{XMLDSIG_SHA256}"></ds:DigestMethod><bad:DigestValue xmlns:bad="urn:not-xmldsig">{digest}</bad:DigestValue></ds:Reference>"#
        );
        let err = verify_xml_signature_reference_digest_with_namespaces(
            &wrong_digest_method,
            canonical,
            &inherited_namespaces,
        )
        .expect_err("DigestValue bound outside XMLDSig must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_signature_reference_rejects_malformed_qnames() {
        let malformed_reference = format!(
            r#"<ds::Reference xmlns:ds="{XMLDSIG_NS}" URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms></ds::Reference>"#
        );
        let err = supported_xml_signature_reference_c14n_mode(&malformed_reference)
            .expect_err("malformed XMLDSig Reference QName must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let malformed_transform = format!(
            r#"<Reference URI=""><Transforms><ds::Transform xmlns:ds="{XMLDSIG_NS}" Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></ds::Transform></Transforms></Reference>"#
        );
        let err = supported_xml_signature_reference_c14n_mode(&malformed_transform)
            .expect_err("malformed direct XMLDSig Transform QName must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let canonical = "<Document></Document>";
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical.as_bytes()));
        let malformed_digest_method = format!(
            r#"<Reference URI=""><ds::DigestMethod xmlns:ds="{XMLDSIG_NS}" Algorithm="{XMLDSIG_SHA256}"></ds::DigestMethod><DigestValue>{digest}</DigestValue></Reference>"#
        );
        let err = verify_xml_signature_reference_digest(&malformed_digest_method, canonical)
            .expect_err("malformed namespace-bound DigestMethod QName must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_signed_properties_reference_requires_supported_c14n_transform() {
        let valid = format!(
            r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"##
        );
        assert_eq!(
            signed_properties_reference_c14n_mode(&valid)
                .expect("SignedProperties Reference with supported C14N should pass"),
            CanonicalXmlMode::Exclusive
        );

        for reference in [
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}" Id="ref-001"><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"##
            ),
            r##"<Reference URI="#signed-props-001" Type="http://uri.etsi.org/01903#SignedProperties"><Transforms></Transforms></Reference>"##
                .to_owned(),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms Id="transforms-001"><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Transform Id="transform-001" Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Ignored></Ignored><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms><DigestValue>AQ==</DigestValue></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms><DigestValue>AQ==</DigestValue><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Object></Object><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Reference>"##
            ),
            r##"<Reference URI="#signed-props-001" Type="http://uri.etsi.org/01903#SignedProperties"><Transforms><Transform Algorithm="urn:unsupported"></Transform></Transforms></Reference>"##
                .to_owned(),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"><InclusiveNamespaces PrefixList="ds"></InclusiveNamespaces></Transform></Transforms></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Transform Algorithm="{XML_EXCLUSIVE_C14N_1_0}"></Transform></Transforms><Transforms><Transform Algorithm="{XML_C14N_1_0}"></Transform></Transforms></Reference>"##
            ),
            format!(
                r##"<Reference URI="#signed-props-001" Type="{XADES_SIGNED_PROPERTIES_TYPE}"><Transforms><Transform Algorithm="{XML_C14N_1_0}"></Transform><Transform Algorithm="{XML_C14N_1_1}"></Transform></Transforms></Reference>"##
            ),
        ] {
            let err = signed_properties_reference_c14n_mode(&reference)
                .expect_err("SignedProperties references require one supported C14N transform");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_reference_digest_rejects_digest_method_unsupported_attributes() {
        let canonical = "<Document></Document>";
        let digest = BASE64_STANDARD.encode(Sha256::digest(canonical.as_bytes()));
        let valid = format!(
            r#"<Reference URI=""><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue></Reference>"#
        );
        verify_xml_signature_reference_digest(&valid, canonical)
            .expect("SHA-256 Reference digest should pass");

        let invalid = format!(
            r#"<Reference URI=""><DigestMethod Algorithm="{XMLDSIG_SHA256}" Id="digest-method-001"></DigestMethod><DigestValue>{digest}</DigestValue></Reference>"#
        );
        let err = verify_xml_signature_reference_digest(&invalid, canonical)
            .expect_err("unsupported DigestMethod attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let namespace_qualified_algorithm = format!(
            r#"<Reference URI=""><DigestMethod ds:Algorithm="{XMLDSIG_SHA256}" xmlns:ds="{XMLDSIG_NS}"></DigestMethod><DigestValue>{digest}</DigestValue></Reference>"#
        );
        let err = verify_xml_signature_reference_digest(&namespace_qualified_algorithm, canonical)
            .expect_err("namespace-qualified DigestMethod Algorithm must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_signed_properties_reference_verifies_generated_fixture() {
        let payload = signed_pacs008_xml_with_signed_properties_reference();
        let signature_span = find_first_xml_element(&payload, "Signature").expect("signature");
        let signature_xml = &payload[signature_span.start..signature_span.end];
        ensure_xades_qualifying_properties_target(signature_xml)
            .expect("QualifyingProperties Target should bind to Signature Id");
        let signed_info_span =
            find_first_xml_element(signature_xml, "SignedInfo").expect("SignedInfo");
        let signed_info_xml = &signature_xml[signed_info_span.start..signed_info_span.end];

        verify_xml_signature_references(
            &payload,
            signature_span,
            signature_span,
            signature_xml,
            signed_info_xml,
            &[],
        )
        .expect("generated XAdES fixture references should verify");
    }

    #[test]
    fn supported_xml_xades_qualifying_properties_requires_direct_shape() {
        let payload = signed_pacs008_xml_with_signed_properties_reference();
        let signature_span = find_first_xml_element(&payload, "Signature").expect("signature");
        let signature_xml = &payload[signature_span.start..signature_span.end];
        ensure_xades_qualifying_properties_target(signature_xml)
            .expect("direct XAdES QualifyingProperties should be accepted");

        let prefixed_signature = format!(
            r##"<Signature Id="sig-001"><Object><xades:QualifyingProperties xmlns:xades="{XADES_NS}" Target="#sig-001"><xades:SignedProperties Id="signed-props-001"></xades:SignedProperties></xades:QualifyingProperties></Object></Signature>"##
        );
        ensure_xades_qualifying_properties_target(&prefixed_signature)
            .expect("prefixed XAdES QualifyingProperties should bind to the supported namespace");

        for invalid_payload in [
            format!(
                r##"<Signature Id="sig-001"><Object><xades:QualifyingProperties xmlns:xades="urn:not-xades" Target="#sig-001"><xades:SignedProperties Id="signed-props-001"></xades:SignedProperties></xades:QualifyingProperties></Object></Signature>"##
            ),
            r##"<Signature Id="sig-001"><Object><QualifyingProperties xmlns="urn:not-xades" Target="#sig-001"><SignedProperties Id="signed-props-001"></SignedProperties></QualifyingProperties></Object></Signature>"##.to_owned(),
            signed_pacs008_xml_with_signed_properties_reference().replacen(
                r##"<QualifyingProperties Target="#sig-001">"##,
                r##"<QualifyingProperties Target="#sig-001" Id="qp-001">"##,
                1,
            ),
            signed_pacs008_xml_with_signed_properties_reference()
                .replacen(
                    r##"<Object><QualifyingProperties Target="#sig-001">"##,
                    r##"<Object><Wrapper><QualifyingProperties Target="#sig-001">"##,
                    1,
                )
                .replacen(
                    "</QualifyingProperties></Object>",
                    "</QualifyingProperties></Wrapper></Object>",
                    1,
                ),
            signed_pacs008_xml_with_signed_properties_reference()
                .replacen("<SignedProperties", "<Wrapper><SignedProperties", 1)
                .replacen("</SignedProperties>", "</SignedProperties></Wrapper>", 1),
            signed_pacs008_xml_with_signed_properties_reference().replacen(
                "</SignedProperties>",
                "</SignedProperties><UnsignedProperties></UnsignedProperties>",
                1,
            ),
        ] {
            let signature_span =
                find_first_xml_element(&invalid_payload, "Signature").expect("signature");
            let signature_xml = &invalid_payload[signature_span.start..signature_span.end];
            let err = ensure_xades_qualifying_properties_target(signature_xml)
                .expect_err("unsupported XAdES wrapper shape must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_xades_signing_certificate_v2_requires_direct_cert_children() {
        let digest = BASE64_STANDARD.encode([7_u8; 32]);
        let cert = format!(
            r#"<Cert><CertDigest><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue></CertDigest></Cert>"#
        );
        let valid = format!(r#"<SigningCertificateV2>{cert}</SigningCertificateV2>"#);
        assert_eq!(
            xades_signing_certificate_v2_digests(&valid)
                .expect("direct Cert children should be accepted"),
            vec![lower_hex(&[7_u8; 32])]
        );

        let prefixed_cert = format!(
            r#"<xades:Cert><xades:CertDigest xmlns:ds="{XMLDSIG_NS}"><ds:DigestMethod Algorithm="{XMLDSIG_SHA256}"></ds:DigestMethod><ds:DigestValue>{digest}</ds:DigestValue></xades:CertDigest></xades:Cert>"#
        );
        let prefixed = format!(
            r#"<xades:SigningCertificateV2 xmlns:xades="{XADES_NS}">{prefixed_cert}</xades:SigningCertificateV2>"#
        );
        assert_eq!(
            xades_signing_certificate_v2_digests(&prefixed)
                .expect("prefixed XAdES SigningCertificateV2 should decode"),
            vec![lower_hex(&[7_u8; 32])]
        );

        for invalid in [
            format!(
                r#"<xades:SigningCertificateV2 xmlns:xades="urn:not-xades">{prefixed_cert}</xades:SigningCertificateV2>"#
            ),
            format!(r#"<SigningCertificateV2 Id="certs-001">{cert}</SigningCertificateV2>"#),
            format!(r#"<SigningCertificateV2><Wrapper>{cert}</Wrapper></SigningCertificateV2>"#),
            format!(
                r#"<SigningCertificateV2>{cert}<Wrapper>{cert}</Wrapper></SigningCertificateV2>"#
            ),
        ] {
            let err = xades_signing_certificate_v2_digests(&invalid)
                .expect_err("wrapped Cert children must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_xades_signing_certificate_v2_binds_ordered_chain_prefix() {
        let leaf = [1_u8; 32];
        let issuer = [2_u8; 32];
        let root = [3_u8; 32];
        let extra = [4_u8; 32];
        let key_material = XmlSignatureKeyMaterial {
            public_key: Vec::new(),
            certificate_sha256: vec![lower_hex(&leaf), lower_hex(&issuer), lower_hex(&root)],
        };

        for valid in [
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf],
            ),
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf, issuer],
            ),
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf, issuer, root],
            ),
        ] {
            verify_xades_signing_certificate_v2_binding(&valid, &key_material)
                .expect("ordered certificate chain prefix should be accepted");
        }

        for invalid in [
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[issuer],
            ),
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf, root],
            ),
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf, leaf],
            ),
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf, issuer, leaf],
            ),
            signed_properties_xml_with_signing_certificate_v2_digest_bytes(
                "signed-props-001",
                &[leaf, issuer, root, extra],
            ),
        ] {
            let err = verify_xades_signing_certificate_v2_binding(&invalid, &key_material)
                .expect_err("unordered, duplicate, or unknown chain digest must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_xades_cert_digest_requires_direct_child() {
        let digest = BASE64_STANDARD.encode([9_u8; 32]);
        let cert_digest = format!(
            r#"<CertDigest><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue></CertDigest>"#
        );
        let valid = format!(r#"<Cert>{cert_digest}</Cert>"#);
        assert_eq!(
            xades_cert_digest_sha256(&valid).expect("direct CertDigest should be accepted"),
            lower_hex(&[9_u8; 32])
        );

        for invalid in [
            format!(r#"<Cert Id="cert-001">{cert_digest}</Cert>"#),
            format!(
                r#"<Cert><CertDigest Id="digest-001"><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue></CertDigest></Cert>"#
            ),
            format!(
                r#"<Cert><CertDigest><DigestMethod Algorithm="{XMLDSIG_SHA256}" Id="method-001"></DigestMethod><DigestValue>{digest}</DigestValue></CertDigest></Cert>"#
            ),
            format!(r#"<Cert><Wrapper>{cert_digest}</Wrapper></Cert>"#),
            format!(r#"<Cert>{cert_digest}<Wrapper>{cert_digest}</Wrapper></Cert>"#),
            format!(
                r#"<Cert><CertDigest><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue><Other></Other></CertDigest></Cert>"#
            ),
        ] {
            let err = xades_cert_digest_sha256(&invalid)
                .expect_err("wrapped CertDigest children must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_xades_signed_signature_properties_requires_direct_shape() {
        let valid = signed_properties_xml("signed-props-001");
        xades_signed_signature_properties_xml(&valid)
            .expect("direct SignedSignatureProperties should be accepted");
        assert_eq!(
            xades_signed_properties_signing_time(&valid)
                .expect("direct SigningTime should be accepted"),
            Some(XML_SIGNATURE_TEST_SIGNING_TIME.to_owned())
        );

        let prefixed = signed_properties_xml_prefixed("signed-props-001", XADES_NS);
        xades_signed_signature_properties_xml(&prefixed)
            .expect("prefixed SignedSignatureProperties should bind to the supported namespace");
        assert_eq!(
            xades_signed_properties_signing_time(&prefixed)
                .expect("prefixed SigningTime should be accepted"),
            Some(XML_SIGNATURE_TEST_SIGNING_TIME.to_owned())
        );

        for invalid in [
            signed_properties_xml_prefixed("signed-props-001", "urn:not-xades"),
            format!(
                r#"<SignedProperties Id="signed-props-001"><Wrapper>{}</Wrapper></SignedProperties>"#,
                signed_properties_xml("inner")
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><Wrapper><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime></Wrapper></SignedSignatureProperties></SignedProperties>"#
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime></SignedSignatureProperties><SignedDataObjectProperties></SignedDataObjectProperties></SignedProperties>"#
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties Profile="unsupported"><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime></SignedSignatureProperties></SignedProperties>"#
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><SigningCertificate></SigningCertificate></SignedSignatureProperties></SignedProperties>"#
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime></SignedSignatureProperties><SignedSignatureProperties><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime></SignedSignatureProperties></SignedProperties>"#
            ),
        ] {
            let err = xades_signed_signature_properties_xml(&invalid)
                .expect_err("unsupported SignedProperties shape must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }

        for invalid in [
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><SigningTime Source="unsupported">{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime></SignedSignatureProperties></SignedProperties>"#
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}<Chunk></Chunk></SigningTime></SignedSignatureProperties></SignedProperties>"#
            ),
            format!(
                r#"<SignedProperties Id="signed-props-001"><SignedSignatureProperties><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}<!--comment--></SigningTime></SignedSignatureProperties></SignedProperties>"#
            ),
        ] {
            let err = xades_signed_properties_signing_time(&invalid)
                .expect_err("unsupported SigningTime leaf shape must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_certificate_evaluation_time_uses_verified_signed_properties_time() {
        let parsed =
            parse_message("pacs.008", unsigned_pacs008_xml().as_bytes()).expect("parse pacs.008");
        assert!(
            xml_signature_evaluation_time(&parsed, None)
                .expect("missing signed time and BAH creation date should be valid")
                .is_none(),
            "certificate evaluation time must not come from unsigned signature-local data"
        );

        let signed_properties = signed_properties_xml("signed-props-001");
        assert!(
            xml_signature_evaluation_time(&parsed, Some(&signed_properties))
                .expect("verified SignedProperties SigningTime should parse")
                .is_some()
        );

        let duplicate_signing_time = signed_properties.replacen(
            "</SigningTime>",
            &format!("</SigningTime><SigningTime>{XML_SIGNATURE_TEST_SIGNING_TIME}</SigningTime>"),
            1,
        );
        let err = xml_signature_evaluation_time(&parsed, Some(&duplicate_signing_time))
            .expect_err("duplicate signed SigningTime values must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_signature_shape_requires_direct_children() {
        assert!(
            find_first_xml_element(
                &format!(r#"<ds:Signature xmlns:ds="{XMLDSIG_NS}"></bad:Signature>"#),
                "Signature",
            )
            .is_none(),
            "opening and closing XML element QNames must match exactly"
        );

        let payload = signed_pacs008_xml();
        let signature_span =
            find_first_xml_element(&payload, "Signature").expect("fixture Signature element");
        let signature_xml = &payload[signature_span.start..signature_span.end];
        let signature_children = xml_signature_direct_child_spans(signature_xml)
            .expect("direct XMLDSig Signature children should be accepted");
        let signed_info_xml = &signature_xml
            [signature_children.signed_info.start..signature_children.signed_info.end];
        xml_signed_info_direct_child_spans(signed_info_xml)
            .expect("direct XMLDSig SignedInfo children should be accepted");

        for invalid_payload in [
            signed_pacs008_xml()
                .replacen("<SignedInfo>", "<Wrapper><SignedInfo>", 1)
                .replacen("</SignedInfo>", "</SignedInfo></Wrapper>", 1),
            "<Signature><SignatureValue>AQ==</SignatureValue><SignedInfo></SignedInfo><KeyInfo></KeyInfo></Signature>"
                .to_owned(),
            "<Signature><SignedInfo></SignedInfo><KeyInfo></KeyInfo><SignatureValue>AQ==</SignatureValue></Signature>"
                .to_owned(),
            "<Signature><SignedInfo></SignedInfo><SignatureValue>AQ==</SignatureValue><Object></Object><KeyInfo></KeyInfo></Signature>"
                .to_owned(),
            signed_pacs008_xml().replacen(
                "</SignatureValue>",
                "</SignatureValue><SignatureValue>AQ==</SignatureValue>",
                1,
            ),
            signed_pacs008_xml().replacen("<KeyInfo>", "<Manifest></Manifest><KeyInfo>", 1),
            format!(
                r#"<ds:Signature xmlns:ds="{XMLDSIG_NS}"><ds:SignedInfo></bad:SignedInfo><ds:SignatureValue>AQ==</ds:SignatureValue><ds:KeyInfo></ds:KeyInfo></ds:Signature>"#
            ),
        ] {
            let signature_span = find_first_xml_element(&invalid_payload, "Signature")
                .expect("invalid fixture Signature element");
            let signature_xml = &invalid_payload[signature_span.start..signature_span.end];
            let err = xml_signature_direct_child_spans(signature_xml)
                .expect_err("unsupported Signature shape must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }

        for invalid_signed_info in [
            format!(
                r#"<SignedInfo><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod><CanonicalizationMethod Algorithm="{XML_C14N_1_0}"></CanonicalizationMethod><Reference URI=""></Reference></SignedInfo>"#
            ),
            format!(
                r#"<SignedInfo><CanonicalizationMethod Algorithm="{XML_C14N_1_0}"></CanonicalizationMethod><Reference URI=""></Reference><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod></SignedInfo>"#
            ),
            signed_info_xml
                .replacen(
                    "<CanonicalizationMethod",
                    "<Wrapper><CanonicalizationMethod",
                    1,
                )
                .replacen("</CanonicalizationMethod>", "</CanonicalizationMethod></Wrapper>", 1),
            signed_info_xml.replacen("</SignatureMethod>", "</SignatureMethod><Object></Object>", 1),
            signed_info_xml.replacen(
                "</SignatureMethod>",
                &format!(
                    r#"</SignatureMethod><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod>"#
                ),
                1,
            ),
        ] {
            let err = xml_signed_info_direct_child_spans(&invalid_signed_info)
                .expect_err("unsupported SignedInfo shape must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_p256_signature_rejects_high_s() {
        let signing_key = xml_signature_test_signing_key();
        let signature: P256Signature = signing_key.sign(b"high-s regression");
        let low_s = low_s_p256_signature(signature);
        decode_p256_xmldsig_signature(low_s.to_bytes().as_slice())
            .expect("low-S fixed-width signatures should be accepted");
        decode_p256_xmldsig_signature(low_s.to_der().as_bytes())
            .expect("low-S DER signatures should be accepted");

        let high_s = high_s_p256_signature(low_s);
        signing_key
            .verifying_key()
            .verify(b"high-s regression", &high_s)
            .expect("high-S counterpart remains mathematically valid ECDSA");

        for signature_value in [
            high_s.to_bytes().to_vec(),
            high_s.to_der().as_bytes().to_vec(),
        ] {
            let err = decode_p256_xmldsig_signature(&signature_value)
                .expect_err("high-S P-256 signatures must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_required_base64_rejects_duplicate_children() {
        let err = decode_required_child_base64(
            r#"<Reference><DigestValue>AQ==</DigestValue><DigestValue>Ag==</DigestValue></Reference>"#,
            "DigestValue",
        )
        .expect_err("duplicate required base64 children must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let err = decode_required_child_base64(
            r#"<Reference><DigestValue Id="digest-001">AQ==</DigestValue></Reference>"#,
            "DigestValue",
        )
        .expect_err("required base64 attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let err = decode_required_child_base64(
            r#"<Reference><DigestValue><Chunk>AQ==</Chunk></DigestValue></Reference>"#,
            "DigestValue",
        )
        .expect_err("nested required base64 markup must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let err = decode_required_child_base64(
            r#"<Reference><DigestValue>A<!--comment-->Q==</DigestValue></Reference>"#,
            "DigestValue",
        )
        .expect_err("required base64 comments must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_key_material_enforces_key_info_shape() {
        let signing_key = xml_signature_test_signing_key();
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let valid = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        xml_signature_key_material(&valid, None)
            .expect("structured P-256 public-key KeyInfo should decode");

        let inherited_namespaces = vec![CanonicalXmlNamespaceBinding {
            prefix: "ds".to_owned(),
            uri: XMLDSIG_NS.to_owned(),
        }];
        let prefixed = format!(
            r#"<ds:KeyInfo><ds:KeyValue><ds:ECKeyValue><ds:NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></ds:NamedCurve><ds:PublicKey>{public_key}</ds:PublicKey></ds:ECKeyValue></ds:KeyValue></ds:KeyInfo>"#
        );
        xml_signature_key_material_with_namespaces(&prefixed, None, &inherited_namespaces)
            .expect("prefixed XMLDSig KeyInfo should resolve inherited namespaces");

        let wrong_key_value_namespace = format!(
            r#"<ds:KeyInfo><bad:KeyValue xmlns:bad="urn:not-xmldsig"><ds:ECKeyValue><ds:NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></ds:NamedCurve><ds:PublicKey>{public_key}</ds:PublicKey></ds:ECKeyValue></bad:KeyValue></ds:KeyInfo>"#
        );
        let err = xml_signature_key_material_with_namespaces(
            &wrong_key_value_namespace,
            None,
            &inherited_namespaces,
        )
        .expect_err("KeyValue bound outside XMLDSig must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let malformed_public_key = BASE64_STANDARD.encode([1_u8, 2, 3]);
        let malformed_public_key_xml = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{malformed_public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&malformed_public_key_xml, None)
            .expect_err("malformed P-256 public-key bytes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let compressed_public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(true)
                .as_bytes(),
        );
        let compressed_public_key_xml = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{compressed_public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&compressed_public_key_xml, None)
            .expect_err("compressed P-256 public-key bytes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let duplicate = format!(
            "<Signature><KeyInfo><PublicKey>{public_key}</PublicKey><PublicKey>{public_key}</PublicKey></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(&duplicate, None)
            .expect_err("duplicate PublicKey elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let mixed = format!(
            "<Signature><KeyInfo><PublicKey>{public_key}</PublicKey><X509Data><X509Certificate>AQ==</X509Certificate></X509Data></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(&mixed, None)
            .expect_err("mixed PublicKey and X509Certificate material must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let wrong_curve = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="urn:oid:1.3.132.0.34"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&wrong_curve, None)
            .expect_err("unsupported public-key curves must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let missing_key_value = format!(
            r#"<Signature><KeyInfo><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&missing_key_value, None)
            .expect_err("PublicKey must be wrapped in KeyValue/ECKeyValue");

        assert!(matches!(err, MsgError::ValidationFailed));

        let public_key_outside_ec_key_value = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve></ECKeyValue><PublicKey>{public_key}</PublicKey></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&public_key_outside_ec_key_value, None)
            .expect_err("PublicKey outside ECKeyValue must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let key_info_extra_child = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue><RetrievalMethod></RetrievalMethod></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&key_info_extra_child, None)
            .expect_err("unsupported KeyInfo child elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let key_value_extra_child = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue><ECParameters></ECParameters></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&key_value_extra_child, None)
            .expect_err("unsupported KeyValue child elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let ec_key_value_extra_child = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey><ECParameters></ECParameters></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&ec_key_value_extra_child, None)
            .expect_err("unsupported ECKeyValue child elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let key_info_text = format!(
            r#"<Signature><KeyInfo>text<KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&key_info_text, None)
            .expect_err("non-whitespace KeyInfo text must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let key_info_attribute = format!(
            r#"<Signature><KeyInfo Id="key-001"><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&key_info_attribute, None)
            .expect_err("unsupported KeyInfo attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let key_value_attribute = format!(
            r#"<Signature><KeyInfo><KeyValue Id="key-value-001"><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&key_value_attribute, None)
            .expect_err("unsupported KeyValue attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let ec_key_value_attribute = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue Type="named"><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&ec_key_value_attribute, None)
            .expect_err("unsupported ECKeyValue attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let named_curve_attribute = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}" Type="named"></NamedCurve><PublicKey>{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&named_curve_attribute, None)
            .expect_err("unsupported NamedCurve attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let public_key_attribute = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey Encoding="base64">{public_key}</PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&public_key_attribute, None)
            .expect_err("unsupported PublicKey attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let public_key_nested_markup = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}<Chunk></Chunk></PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&public_key_nested_markup, None)
            .expect_err("nested PublicKey markup must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let public_key_comment = format!(
            r#"<Signature><KeyInfo><KeyValue><ECKeyValue><NamedCurve URI="{XMLDSIG_P256_NAMED_CURVE}"></NamedCurve><PublicKey>{public_key}<!--comment--></PublicKey></ECKeyValue></KeyValue></KeyInfo></Signature>"#
        );
        let err = xml_signature_key_material(&public_key_comment, None)
            .expect_err("PublicKey comments must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let certificate_without_x509_data =
            "<Signature><KeyInfo><X509Certificate>AQ==</X509Certificate></KeyInfo></Signature>";
        let err = xml_signature_key_material(certificate_without_x509_data, None)
            .expect_err("X509Certificate outside X509Data must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let duplicate_x509_data = concat!(
            "<Signature><KeyInfo><X509Data><X509Certificate>AQ==</X509Certificate></X509Data>",
            "<X509Data><X509Certificate>Ag==</X509Certificate></X509Data></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(duplicate_x509_data, None)
            .expect_err("duplicate X509Data wrappers must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let x509_data_extra_child = concat!(
            "<Signature><KeyInfo><X509Data><X509Certificate>AQ==</X509Certificate>",
            "<X509IssuerSerial></X509IssuerSerial></X509Data></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(x509_data_extra_child, None)
            .expect_err("unsupported X509Data child elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let x509_data_attribute = concat!(
            "<Signature><KeyInfo><X509Data Id=\"certs-001\">",
            "<X509Certificate>AQ==</X509Certificate></X509Data></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(x509_data_attribute, None)
            .expect_err("unsupported X509Data attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let x509_certificate_attribute = concat!(
            "<Signature><KeyInfo><X509Data>",
            "<X509Certificate Encoding=\"base64\">AQ==</X509Certificate>",
            "</X509Data></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(x509_certificate_attribute, None)
            .expect_err("unsupported X509Certificate attributes must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let x509_certificate_nested_markup = concat!(
            "<KeyInfo><X509Data><X509Certificate>AQ==<Chunk></Chunk></X509Certificate>",
            "</X509Data></KeyInfo>"
        );
        let err = xml_signature_x509_certificates(x509_certificate_nested_markup)
            .expect_err("nested X509Certificate markup must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let x509_certificate_comment = concat!(
            "<KeyInfo><X509Data><X509Certificate>AQ==<!--comment--></X509Certificate>",
            "</X509Data></KeyInfo>"
        );
        let err = xml_signature_x509_certificates(x509_certificate_comment)
            .expect_err("X509Certificate comments must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let missing_key_info =
            format!("<Signature><PublicKey>{public_key}</PublicKey></Signature>");
        let err = xml_signature_key_material(&missing_key_info, None)
            .expect_err("PublicKey outside KeyInfo must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let out_of_scope = format!(
            "<Signature><PublicKey>{public_key}</PublicKey><KeyInfo><PublicKey>{public_key}</PublicKey></KeyInfo></Signature>"
        );
        let err = xml_signature_key_material(&out_of_scope, None)
            .expect_err("key material outside KeyInfo must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_x509_certificate_chain_bounds_reject_duplicates_and_overlong_chains() {
        let valid = (0..XML_SIGNATURE_MAX_X509_CERTIFICATES)
            .map(|index| vec![index as u8])
            .collect::<Vec<_>>();
        ensure_xml_signature_certificate_chain_bounds(&valid)
            .expect("maximum supported unique X509 chain should be accepted");

        let empty = Vec::new();
        let err = ensure_xml_signature_certificate_chain_bounds(&empty)
            .expect_err("empty X509 chain must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let duplicate = vec![vec![1_u8], vec![1_u8]];
        let err = ensure_xml_signature_certificate_chain_bounds(&duplicate)
            .expect_err("duplicate X509 DER entries must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));

        let overlong = (0..=XML_SIGNATURE_MAX_X509_CERTIFICATES)
            .map(|index| vec![index as u8])
            .collect::<Vec<_>>();
        let err = ensure_xml_signature_certificate_chain_bounds(&overlong)
            .expect_err("overlong X509 chain must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_xades_cert_digest_rejects_duplicate_methods() {
        let digest = BASE64_STANDARD.encode([0_u8; 32]);
        let cert_xml = format!(
            r#"<Cert><CertDigest><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod><DigestValue>{digest}</DigestValue></CertDigest></Cert>"#
        );
        let err = xades_cert_digest_sha256(&cert_xml)
            .expect_err("duplicate XAdES CertDigest methods must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_canonicalizer_accepts_prefixed_attributes() {
        let canonical = canonicalize_supported_xml(
            r#"<A xmlns:z="urn:z" z:flag="yes" b="2" xmlns:a="urn:a" a:flag="no"></A>"#,
        )
        .expect("declared prefixed attributes should canonicalize by expanded name");

        assert_eq!(
            canonical,
            r#"<A xmlns:a="urn:a" xmlns:z="urn:z" b="2" a:flag="no" z:flag="yes"></A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_accepts_inherited_prefixed_attributes() {
        let canonical =
            canonicalize_supported_xml(r#"<A xmlns:p="urn:p"><B p:flag="true"></B></A>"#)
                .expect("child attributes should resolve namespace bindings from ancestors");

        assert_eq!(canonical, r#"<A xmlns:p="urn:p"><B p:flag="true"></B></A>"#);
    }

    #[test]
    fn supported_xml_canonicalizer_strips_comments_for_no_comments_c14n() {
        let canonical = canonicalize_supported_xml(
            "<!--before--><A><!--inside--><B>value<!--text comment--></B><!--tail--></A><!--after-->",
        )
        .expect("no-comments XML canonicalization should omit comments");

        assert_eq!(canonical, "<A><B>value</B></A>");
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_malformed_comments() {
        for xml in ["<A><!--unterminated</A>", "<A><!--bad--comment--></A>"] {
            let err = canonicalize_supported_xml(xml)
                .expect_err("malformed XML comments must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_unsupported_entities() {
        for xml in [
            "<A>&custom;</A>",
            "<A>&amp</A>",
            "<A>&#0;</A>",
            "<A>&#xD800;</A>",
            "<A>&#x110000;</A>",
        ] {
            let err = canonicalize_supported_xml(xml)
                .expect_err("unsupported or invalid XML character references must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_reserved_namespace_declarations() {
        for xml in [
            r#"<A xmlns:xml="urn:not-xml"></A>"#,
            r#"<A xmlns="http://www.w3.org/XML/1998/namespace"></A>"#,
            r#"<A xmlns:xmlns="http://www.w3.org/2000/xmlns/"></A>"#,
            r#"<A xmlns:bad="http://www.w3.org/XML/1998/namespace"></A>"#,
            r#"<A xmlns:bad="http://www.w3.org/2000/xmlns/"></A>"#,
        ] {
            let err = canonicalize_supported_xml(xml)
                .expect_err("reserved namespace declarations must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_unbound_prefixed_attributes() {
        let err = canonicalize_supported_xml(r#"<A p:id="id-1"></A>"#)
            .expect_err("prefixed attributes without in-scope namespace bindings must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_duplicate_expanded_attributes() {
        let err = canonicalize_supported_xml(
            r#"<A xmlns:p="urn:same" xmlns:q="urn:same" p:id="one" q:id="two"></A>"#,
        )
        .expect_err("attributes with duplicate expanded names must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_canonicalizer_accepts_locally_declared_prefixes() {
        let canonical = canonicalize_supported_xml(r#"<ds:A xmlns:ds="urn:test"><ds:B /></ds:A>"#)
            .expect("locally declared namespace prefixes are inside the supported subset");

        assert_eq!(
            canonical,
            r#"<ds:A xmlns:ds="urn:test"><ds:B></ds:B></ds:A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_applies_inherited_root_namespace() {
        let inherited_namespaces = vec![CanonicalXmlAttribute {
            name: "xmlns:ds".to_owned(),
            value: XMLDSIG_NS.to_owned(),
            kind: CanonicalXmlAttributeKind::Namespace,
        }];
        let canonical = canonicalize_supported_xml_with_inherited_namespaces(
            "<ds:A><ds:B /></ds:A>",
            &inherited_namespaces,
        )
        .expect("root namespace inherited from the enclosing XMLDSig element should canonicalize");

        assert_eq!(
            canonical,
            r#"<ds:A xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:B></ds:B></ds:A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_distinguishes_inclusive_and_exclusive_namespaces() {
        let inherited_namespaces = vec![
            CanonicalXmlAttribute {
                name: "xmlns:ds".to_owned(),
                value: XMLDSIG_NS.to_owned(),
                kind: CanonicalXmlAttributeKind::Namespace,
            },
            CanonicalXmlAttribute {
                name: "xmlns:unused".to_owned(),
                value: "urn:unused".to_owned(),
                kind: CanonicalXmlAttributeKind::Namespace,
            },
        ];
        let inclusive = canonicalize_supported_xml_with_mode(
            "<ds:A><ds:B /></ds:A>",
            &inherited_namespaces,
            CanonicalXmlMode::Inclusive,
        )
        .expect("inclusive canonicalization should carry inherited namespaces");
        let exclusive = canonicalize_supported_xml_with_mode(
            "<ds:A><ds:B /></ds:A>",
            &inherited_namespaces,
            CanonicalXmlMode::Exclusive,
        )
        .expect("exclusive canonicalization should carry only visibly used namespaces");

        assert_eq!(
            inclusive,
            r#"<ds:A xmlns:ds="http://www.w3.org/2000/09/xmldsig#" xmlns:unused="urn:unused"><ds:B></ds:B></ds:A>"#
        );
        assert_eq!(
            exclusive,
            r#"<ds:A xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:B></ds:B></ds:A>"#
        );
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_inherited_prefix_context() {
        let err = canonicalize_supported_xml("<ds:A></ds:A>")
            .expect_err("inherited namespace context is outside this canonicalizer");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn supported_xml_canonicalizer_rejects_attribute_whitespace_rewrites() {
        let err = canonicalize_supported_xml("<A value='line\nbreak'></A>")
            .expect_err("attribute whitespace normalization is outside this canonicalizer");

        assert!(matches!(err, MsgError::ValidationFailed));
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
    fn require_verified_profile_accepts_comments_in_no_comments_c14n() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_comments();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("no-comments C14N should omit XML comments before digest and signature checks");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_character_references_in_payload_digest() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_character_reference_message_id();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("canonicalized character references should satisfy the XMLDSig digest");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig&amp;001"));
    }

    #[test]
    fn require_verified_profile_accepts_xml_namespace_attribute_in_payload_digest() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_xml_namespace_attribute();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("implicit xml namespace attributes should satisfy the XMLDSig digest");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_prefixed_attribute_in_payload_digest() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_prefixed_attribute();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("prefixed payload attributes should satisfy the XMLDSig digest");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_xml_namespace_attribute_in_signed_info() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_signed_info_xml_namespace_attribute();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("implicit xml namespace attributes should canonicalize inside SignedInfo");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_same_document_id_reference() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_same_document_reference();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("same-document #id XMLDSig references should verify");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_same_document_reference_with_inherited_namespace() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_inherited_namespace_same_document_reference();
        let parsed = sample_pacs008();
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("same-document references should inherit ancestor namespace context");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("m-profile"));
    }

    #[test]
    fn require_verified_profile_rejects_same_document_reference_outside_signature_carrier() {
        assert_require_verified_signed_payload_rejected(
            signed_pacs008_xml_with_header_only_same_document_reference(),
            "same-document payload Reference must cover the XMLDSig carrier",
        );
    }

    #[test]
    fn require_verified_profile_accepts_reference_c14n_transform() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_reference_c14n_transform();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("payload Reference C14N transform should drive digest canonicalization");

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_inherited_prefixed_attribute_in_signed_info() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_signed_info_inherited_prefixed_attribute();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect(
                "exclusive C14N should inherit namespaces visibly used by SignedInfo attributes",
            );

        assert!(metadata.embedded_signature_detected());
        assert_eq!(metadata.business_message_id(), Some("sig-001"));
    }

    #[test]
    fn require_verified_profile_accepts_prefixed_signed_info_with_inherited_namespace() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_prefixed_signature_namespace();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("prefixed XMLDSig SignedInfo should inherit the Signature namespace");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_sgntr_wrapped_signature_carrier() {
        assert_require_verified_signed_payload_accepted(
            signed_pacs008_xml_with_sgntr_signature_carrier(),
            "Sgntr wrappers with one direct XMLDSig Signature must verify",
        );
    }

    #[test]
    fn require_verified_profile_rejects_prefixed_signature_with_wrong_namespace() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_wrong_prefixed_signature_namespace();
        let parsed = parse_message("pacs.008", payload.as_bytes())
            .expect("parse wrong-namespace signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("prefixed XMLDSig elements must bind to the XMLDSig namespace");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_unprefixed_signature_with_wrong_default_namespace() {
        assert_require_verified_signed_payload_rejected(
            signed_pacs008_xml_with_wrong_default_signature_namespace(),
            "unprefixed XMLDSig elements must reject wrong default namespaces",
        );
    }

    #[test]
    fn require_verified_profile_accepts_inclusive_c14n_with_unused_inherited_namespace() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_inclusive_unused_signature_namespace();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("inclusive XML canonicalization should carry inherited root namespaces");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_rejects_exclusive_bytes_declared_as_inclusive_c14n() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_exclusive_bytes_declared_as_inclusive();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("declared inclusive C14N must not verify exclusive canonical bytes");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_accepts_fixed_width_ecdsa_signature_value() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_fixed_width_signature_value();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("XMLDSig ECDSA SignatureValue should accept fixed-width r||s bytes");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_self_closing_signed_info_methods() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_self_closing_signed_info();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("supported canonicalizer must expand self-closing SignedInfo methods");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_rejects_raw_self_closing_signed_info_signature() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_raw_self_closing_signed_info_signature();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("signature must be checked against canonical SignedInfo bytes");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_extra_sgntr_signature_carrier() {
        assert_require_verified_signed_payload_rejected(
            signed_pacs008_xml_with_extra_sgntr_carrier(),
            "extra XMLDSig/Sgntr signature carriers must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_accepts_certificate_chain_with_pinned_issuer() {
        assert_pinned_certificate_chain_accepted(
            signed_pacs008_xml_with_certificate_chain(),
            "certificate chain ending at a pinned issuer must pass",
        );
    }

    #[test]
    fn require_verified_profile_accepts_certificate_chain_with_path_len_permitting_intermediate() {
        assert_pinned_certificate_chain_accepted(
            signed_pacs008_xml_with_three_certificate_chain(1),
            "pathLenConstraint=1 should allow one subordinate intermediate CA",
        );
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
    fn require_verified_profile_rejects_certificate_chain_with_revoked_issuer() {
        let CertificateChainSignedPayload {
            payload,
            issuer_sha256,
            ..
        } = signed_pacs008_xml_with_certificate_chain();
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![issuer_sha256.clone()];
        profile.revoked_certificate_sha256 = vec![issuer_sha256];
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
            .expect_err("revoked certificate pins must override matching trust pins");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_with_revoked_leaf() {
        let CertificateChainSignedPayload {
            payload,
            leaf_sha256,
            issuer_sha256,
        } = signed_pacs008_xml_with_certificate_chain();
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![issuer_sha256];
        profile.revoked_certificate_sha256 = vec![leaf_sha256];
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
            .expect_err("revoked leaf certificate pins must override matching issuer trust pins");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_with_non_ca_issuer() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_policy(false, true, true),
            "certificate-chain issuers must be CA certificates",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_without_issuer_key_cert_sign() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_policy(true, false, true),
            "certificate-chain issuers must carry keyCertSign usage",
        );
    }

    #[test]
    fn certificate_chain_rejects_noncritical_issuer_basic_constraints() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_noncritical_issuer_basic_constraints(),
            "certificate-chain issuers must carry critical CA basicConstraints",
        );
    }

    #[test]
    fn certificate_chain_rejects_noncritical_issuer_key_usage() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_noncritical_issuer_key_usage(),
            "certificate-chain issuers must carry critical keyCertSign usage",
        );
    }

    #[test]
    fn certificate_chain_rejects_unsupported_certificate_algorithm() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_p384_issuer_certificate_chain(),
            "certificate chains must stay in the supported P-256/SHA-256 corridor",
        );
    }

    #[test]
    fn certificate_chain_rejects_compressed_leaf_spki() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_compressed_leaf_certificate_spki(),
            "XMLDSig leaf certificate SPKI must be an uncompressed P-256 point",
        );
    }

    #[test]
    fn certificate_chain_rejects_compressed_issuer_spki() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_compressed_issuer_certificate_spki(),
            "XMLDSig issuer certificate SPKI must be an uncompressed P-256 point",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_without_leaf_digital_signature() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_policy(true, true, false),
            "XMLDSig leaf certificates must carry digitalSignature usage",
        );
    }

    #[test]
    fn certificate_chain_rejects_noncritical_leaf_key_usage() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_noncritical_leaf_key_usage(),
            "XMLDSig leaf certificates must carry critical digitalSignature usage",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_with_leaf_ca_basic_constraints() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_leaf_ca_certificate_chain(),
            "XMLDSig leaf certificates must not be CA certificates",
        );
    }

    #[test]
    fn require_verified_profile_rejects_expired_certificate_chain_leaf() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_options(
                true,
                true,
                true,
                CertificateChainValidity::ExpiredLeaf,
                CertificateChainIssuerName::Matching,
                false,
            ),
            "XMLDSig leaf certificate must be valid at signing time",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_with_future_issuer() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_options(
                true,
                true,
                true,
                CertificateChainValidity::FutureIssuer,
                CertificateChainIssuerName::Matching,
                false,
            ),
            "XMLDSig issuer certificate must be valid at signing time",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_with_issuer_name_mismatch() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_issuer_name_mismatch(),
            "certificate-chain issuers must match child issuer distinguished names",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_chain_path_len_violation() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_three_certificate_chain(0),
            "pathLenConstraint=0 must reject a subordinate intermediate CA",
        );
    }

    #[test]
    fn require_verified_profile_rejects_leaf_with_critical_unknown_extension() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_critical_unknown_extension(
                CertificateChainCriticalUnknownExtension::Leaf,
            ),
            "critical unknown leaf certificate extensions must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_issuer_with_critical_unknown_extension() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_critical_unknown_extension(
                CertificateChainCriticalUnknownExtension::Issuer,
            ),
            "critical unknown issuer certificate extensions must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_leaf_with_critical_unsupported_parsed_extension() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_critical_unsupported_parsed_extension(
                CertificateChainCriticalUnsupportedParsedExtension::Leaf,
            ),
            "critical parsed leaf certificate extensions outside the enforced subset must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_issuer_with_critical_unsupported_parsed_extension() {
        assert_pinned_certificate_chain_rejected(
            signed_pacs008_xml_with_certificate_chain_critical_unsupported_parsed_extension(
                CertificateChainCriticalUnsupportedParsedExtension::Issuer,
            ),
            "critical parsed issuer certificate extensions outside the enforced subset must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_non_ascii_certificate_signing_time() {
        let mut fixture = signed_pacs008_xml_with_certificate_chain();
        fixture.payload = fixture.payload.replace(
            XML_SIGNATURE_TEST_SIGNING_TIME,
            "\u{ff12}\u{ff10}\u{ff12}\u{ff15}-01-01T12:00:00Z",
        );
        assert_pinned_certificate_chain_rejected(
            fixture,
            "XMLDSig certificate signing time parser must reject non-ASCII dates without panicking",
        );
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
    fn require_verified_profile_rejects_wrapped_signed_info() {
        let payload = signed_pacs008_xml()
            .replacen("<SignedInfo>", "<Wrapper><SignedInfo>", 1)
            .replacen("</SignedInfo>", "</SignedInfo></Wrapper>", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "wrapped SignedInfo must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_signature_extra_child() {
        let payload =
            signed_pacs008_xml().replacen("<KeyInfo>", "<Manifest></Manifest><KeyInfo>", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported direct Signature children must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_reordered_signature_children() {
        let payload = signed_pacs008_xml();
        let signature_value_start = payload.find("<SignatureValue>").expect("SignatureValue");
        let signature_value_end = payload
            .find("</SignatureValue>")
            .expect("SignatureValue end")
            + "</SignatureValue>".len();
        let key_info_start = payload.find("<KeyInfo>").expect("KeyInfo");
        let key_info_end = payload.find("</KeyInfo>").expect("KeyInfo end") + "</KeyInfo>".len();
        let payload = format!(
            "{}{}{}{}{}",
            &payload[..signature_value_start],
            &payload[key_info_start..key_info_end],
            &payload[signature_value_start..signature_value_end],
            &payload[signature_value_end..key_info_start],
            &payload[key_info_end..]
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "reordered Signature children must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_signed_info_extra_child() {
        let payload = signed_pacs008_xml().replacen(
            "</SignatureMethod>",
            "</SignatureMethod><Object></Object>",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported direct SignedInfo children must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_reordered_signed_info_children() {
        let payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            let canonicalization_method = format!(
                r#"<CanonicalizationMethod Algorithm="{XML_C14N_1_0}"></CanonicalizationMethod>"#
            );
            let signature_method = format!(
                r#"<SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod>"#
            );
            signed_info.replacen(
                &format!("{canonicalization_method}{signature_method}"),
                &format!("{signature_method}{canonicalization_method}"),
                1,
            )
        });

        assert_require_verified_signed_payload_rejected(
            payload,
            "reordered SignedInfo children must fail closed",
        );
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
    fn require_verified_profile_rejects_canonicalization_method_parameters() {
        let payload = signed_pacs008_xml().replace(
            &format!(r#"<CanonicalizationMethod Algorithm="{XML_C14N_1_0}"></CanonicalizationMethod>"#),
            &format!(
                r#"<CanonicalizationMethod Algorithm="{XML_C14N_1_0}"><InclusiveNamespaces PrefixList="unused"></InclusiveNamespaces></CanonicalizationMethod>"#
            ),
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "parameterized SignedInfo canonicalization must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_signed_info_method_unsupported_attributes() {
        let canonicalization_payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            let method = format!(
                r#"<CanonicalizationMethod Algorithm="{XML_C14N_1_0}"></CanonicalizationMethod>"#
            );
            let unsupported = format!(
                r#"<CanonicalizationMethod Algorithm="{XML_C14N_1_0}" Id="c14n-001"></CanonicalizationMethod>"#
            );
            signed_info.replacen(&method, &unsupported, 1)
        });
        let signature_payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            let method = format!(
                r#"<SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod>"#
            );
            let unsupported = format!(
                r#"<SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}" Id="signature-method-001"></SignatureMethod>"#
            );
            signed_info.replacen(&method, &unsupported, 1)
        });

        for payload in [canonicalization_payload, signature_payload] {
            assert_require_verified_signed_payload_rejected(
                payload,
                "unsupported SignedInfo method attributes must fail closed",
            );
        }
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_canonicalization_method() {
        let method = format!(
            r#"<CanonicalizationMethod Algorithm="{XML_C14N_1_0}"></CanonicalizationMethod>"#
        );
        let payload = signed_pacs008_xml().replace(&method, &format!("{method}{method}"));

        assert_require_verified_signed_payload_rejected(
            payload,
            "duplicate SignedInfo canonicalization methods must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_signature_method_parameters() {
        let payload = signed_pacs008_xml().replace(
            &format!(r#"<SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod>"#),
            &format!(
                r#"<SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"><HMACOutputLength>256</HMACOutputLength></SignatureMethod>"#
            ),
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "parameterized SignatureMethod must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_signature_method() {
        let method =
            format!(r#"<SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"></SignatureMethod>"#);
        let payload = signed_pacs008_xml().replace(&method, &format!("{method}{method}"));

        assert_require_verified_signed_payload_rejected(
            payload,
            "duplicate SignatureMethod elements must fail closed",
        );
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
    fn require_verified_profile_rejects_digest_method_parameters() {
        let payload = signed_pacs008_xml().replace(
            &format!(r#"<DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod>"#),
            &format!(
                r#"<DigestMethod Algorithm="{XMLDSIG_SHA256}"><DigestParams>not-supported</DigestParams></DigestMethod>"#
            ),
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "parameterized DigestMethod must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_digest_method() {
        let method = format!(r#"<DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod>"#);
        let payload = signed_pacs008_xml().replace(&method, &format!("{method}{method}"));

        assert_require_verified_signed_payload_rejected(
            payload,
            "duplicate DigestMethod elements must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_reference_unsupported_attributes() {
        let reference_payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            signed_info.replacen(
                r#"<Reference URI="">"#,
                r#"<Reference URI="" Id="payload-ref-001">"#,
                1,
            )
        });
        let transforms_payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            signed_info.replacen("<Transforms>", r#"<Transforms Id="transforms-001">"#, 1)
        });
        let transform_payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            let transform =
                format!(r#"<Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform>"#);
            let unsupported = format!(
                r#"<Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}" Id="transform-001"></Transform>"#
            );
            signed_info.replacen(&transform, &unsupported, 1)
        });
        let digest_method_payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            let method = format!(r#"<DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod>"#);
            let unsupported = format!(
                r#"<DigestMethod Algorithm="{XMLDSIG_SHA256}" Id="digest-method-001"></DigestMethod>"#
            );
            signed_info.replacen(&method, &unsupported, 1)
        });

        for payload in [
            reference_payload,
            transforms_payload,
            transform_payload,
            digest_method_payload,
        ] {
            assert_require_verified_signed_payload_rejected(
                payload,
                "unsupported Reference attributes must fail closed",
            );
        }
    }

    #[test]
    fn require_verified_profile_rejects_reordered_reference_children() {
        let payload = signed_pacs008_xml_with_signed_info_rewrite(|signed_info| {
            let transforms = format!(
                r#"<Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms>"#
            );
            let digest_method =
                format!(r#"<DigestMethod Algorithm="{XMLDSIG_SHA256}"></DigestMethod>"#);
            signed_info.replacen(
                &format!("{transforms}{digest_method}"),
                &format!("{digest_method}{transforms}"),
                1,
            )
        });

        assert_require_verified_signed_payload_rejected(
            payload,
            "reordered Reference children must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_reference_c14n_transform_parameters() {
        let payload = signed_pacs008_xml_with_reference_c14n_transform().replace(
            &format!(r#"<Transform Algorithm="{XML_C14N_1_0}"></Transform>"#),
            &format!(
                r#"<Transform Algorithm="{XML_C14N_1_0}"><InclusiveNamespaces PrefixList="unused"></InclusiveNamespaces></Transform>"#
            ),
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "parameterized Reference C14N transforms must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_reference_transform_without_wrapper() {
        let wrapped = format!(
            r#"<Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform></Transforms>"#
        );
        let unwrapped =
            format!(r#"<Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"></Transform>"#);
        let payload = signed_pacs008_xml().replace(&wrapped, &unwrapped);

        assert_require_verified_signed_payload_rejected(
            payload,
            "Reference transforms outside Transforms wrapper must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_extra_reference_transform() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_extra_reference_transform();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("Reference transforms outside the supported set must fail closed");

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
    fn require_verified_profile_rejects_signed_properties_digest_tampering() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_signed_properties_reference()
            .replace(XML_SIGNATURE_TEST_SIGNING_TIME, "2025-01-01T12:00:01Z");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("SignedProperties digest tampering must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_signed_properties_target_drift() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_signed_properties_reference()
            .replace(r##"Target="#sig-001""##, r##"Target="#other-sig""##);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("QualifyingProperties Target must bind to the enclosing Signature Id");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_wrapped_qualifying_properties() {
        let payload = signed_pacs008_xml_with_signed_properties_reference()
            .replacen(
                r##"<Object><QualifyingProperties Target="#sig-001">"##,
                r##"<Object><Wrapper><QualifyingProperties Target="#sig-001">"##,
                1,
            )
            .replacen(
                "</QualifyingProperties></Object>",
                "</QualifyingProperties></Wrapper></Object>",
                1,
            );

        assert_require_verified_signed_payload_rejected(
            payload,
            "wrapped QualifyingProperties must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_wrapped_xades_signed_properties() {
        let payload = signed_pacs008_xml_with_signed_properties_reference()
            .replacen("<SignedProperties", "<Wrapper><SignedProperties", 1)
            .replacen("</SignedProperties>", "</SignedProperties></Wrapper>", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "wrapped XAdES SignedProperties must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_signing_certificate_v2_digest_tampering() {
        let fixture = signed_pacs008_xml_with_certificate_chain_signed_properties_reference();
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256.clear();
        profile.trusted_certificate_sha256 = vec![fixture.issuer_sha256];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let tampered_digest = BASE64_STANDARD.encode([0u8; 32]);
        let payload = fixture
            .payload
            .replace(&fixture.signing_certificate_digest, &tampered_digest);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("SigningCertificateV2 digest tampering must fail closed");

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
    fn require_verified_profile_rejects_high_s_signature_value() {
        let payload = signed_pacs008_xml();
        let signature_value_start =
            payload.find("<SignatureValue>").expect("SignatureValue") + "<SignatureValue>".len();
        let signature_value_end = payload
            .find("</SignatureValue>")
            .expect("SignatureValue end");
        let signature_value = BASE64_STANDARD
            .decode(&payload[signature_value_start..signature_value_end])
            .expect("fixture SignatureValue base64");
        let signature =
            P256Signature::from_der(&signature_value).expect("fixture DER SignatureValue");
        let high_s = high_s_p256_signature(signature);
        let high_s_signature_value = BASE64_STANDARD.encode(high_s.to_der().as_bytes());
        let payload = format!(
            "{}{}{}",
            &payload[..signature_value_start],
            high_s_signature_value,
            &payload[signature_value_end..]
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "high-S SignatureValue must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_signature_value() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml().replacen(
            "</SignatureValue>",
            "</SignatureValue><SignatureValue>AA==</SignatureValue>",
            1,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("duplicate SignatureValue elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_signature_value_unsupported_attribute() {
        let payload = signed_pacs008_xml().replacen(
            "<SignatureValue>",
            "<SignatureValue Id=\"signature-value-001\">",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported SignatureValue attributes must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_digest_value() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml().replacen(
            "</DigestValue>",
            "</DigestValue><DigestValue>AA==</DigestValue>",
            1,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("duplicate DigestValue elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_public_key() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml().replacen(
            "</PublicKey>",
            "</PublicKey><PublicKey>AA==</PublicKey>",
            1,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("duplicate PublicKey elements must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_public_key_nested_markup() {
        let payload =
            signed_pacs008_xml().replacen("</PublicKey>", "<Chunk></Chunk></PublicKey>", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "nested PublicKey markup must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_key_info() {
        let payload = signed_pacs008_xml().replacen(
            "</KeyInfo>",
            "</KeyInfo><KeyInfo><PublicKey>AQ==</PublicKey></KeyInfo>",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "duplicate KeyInfo elements must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_public_key_outside_key_info() {
        let payload =
            signed_pacs008_xml().replacen("<KeyInfo>", "<PublicKey>AQ==</PublicKey><KeyInfo>", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "PublicKey outside KeyInfo must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_public_key_wrong_named_curve() {
        let payload =
            signed_pacs008_xml().replace(XMLDSIG_P256_NAMED_CURVE, "urn:oid:1.3.132.0.34");

        assert_require_verified_signed_payload_rejected(
            payload,
            "PublicKey must declare the supported P-256 NamedCurve",
        );
    }

    #[test]
    fn require_verified_profile_rejects_compressed_public_key() {
        let signing_key = xml_signature_test_signing_key();
        let compressed_public_key = signing_key
            .verifying_key()
            .to_encoded_point(true)
            .as_bytes()
            .to_vec();
        let payload =
            signed_pacs008_xml_with_public_key(&BASE64_STANDARD.encode(&compressed_public_key));
        let mut profile = signed_message_profile("require-verified");
        profile.trusted_public_key_sha256 = vec![sha256_hex(&compressed_public_key)];
        let mut config = sample_config();
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
            .expect_err("compressed SEC1 PublicKey material must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_public_key_without_key_value_wrapper() {
        let payload = signed_pacs008_xml()
            .replace("<KeyInfo><KeyValue><ECKeyValue>", "<KeyInfo><ECKeyValue>")
            .replace(
                "</ECKeyValue></KeyValue></KeyInfo>",
                "</ECKeyValue></KeyInfo>",
            );

        assert_require_verified_signed_payload_rejected(
            payload,
            "PublicKey must be wrapped in KeyValue/ECKeyValue",
        );
    }

    #[test]
    fn require_verified_profile_rejects_key_info_extra_child() {
        let payload = signed_pacs008_xml().replacen(
            "</KeyInfo>",
            "<RetrievalMethod></RetrievalMethod></KeyInfo>",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported KeyInfo child elements must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_key_value_extra_child() {
        let payload = signed_pacs008_xml().replacen(
            "</KeyValue>",
            "<ECParameters></ECParameters></KeyValue>",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported KeyValue child elements must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_ec_key_value_extra_child() {
        let payload = signed_pacs008_xml().replacen(
            "</ECKeyValue>",
            "<ECParameters></ECParameters></ECKeyValue>",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported ECKeyValue child elements must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_key_info_unsupported_attribute() {
        let payload = signed_pacs008_xml().replacen("<KeyInfo>", "<KeyInfo Id=\"key-001\">", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported KeyInfo attributes must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_named_curve_unsupported_attribute() {
        let payload =
            signed_pacs008_xml().replacen("<NamedCurve URI=", "<NamedCurve Type=\"named\" URI=", 1);

        assert_require_verified_signed_payload_rejected(
            payload,
            "unsupported NamedCurve attributes must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_certificate_without_x509_data_wrapper() {
        let mut fixture = signed_pacs008_xml_with_certificate_chain();
        fixture.payload = fixture
            .payload
            .replace("<X509Data>", "")
            .replace("</X509Data>", "");
        assert_pinned_certificate_chain_rejected(
            fixture,
            "X509Certificate elements must be wrapped in X509Data",
        );
    }

    #[test]
    fn require_verified_profile_rejects_x509_data_extra_child() {
        let mut fixture = signed_pacs008_xml_with_certificate_chain();
        fixture.payload = fixture.payload.replacen(
            "</X509Data>",
            "<X509IssuerSerial></X509IssuerSerial></X509Data>",
            1,
        );

        assert_pinned_certificate_chain_rejected(
            fixture,
            "unsupported X509Data child elements must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_x509_certificate_unsupported_attribute() {
        let mut fixture = signed_pacs008_xml_with_certificate_chain();
        fixture.payload = fixture.payload.replacen(
            "<X509Certificate>",
            "<X509Certificate Encoding=\"base64\">",
            1,
        );

        assert_pinned_certificate_chain_rejected(
            fixture,
            "unsupported X509Certificate attributes must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_x509_certificate_nested_markup() {
        let mut fixture = signed_pacs008_xml_with_certificate_chain();
        fixture.payload =
            fixture
                .payload
                .replacen("</X509Certificate>", "<Chunk></Chunk></X509Certificate>", 1);

        assert_pinned_certificate_chain_rejected(
            fixture,
            "nested X509Certificate markup must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_x509_certificate_der() {
        let mut fixture = signed_pacs008_xml_with_certificate_chain();
        let certificate_start = fixture
            .payload
            .find("<X509Certificate>")
            .expect("fixture X509Certificate start");
        let certificate_end = fixture
            .payload
            .find("</X509Certificate>")
            .map(|end| end + "</X509Certificate>".len())
            .expect("fixture X509Certificate end");
        let duplicate_certificate = fixture.payload[certificate_start..certificate_end].to_owned();
        fixture
            .payload
            .insert_str(certificate_end, &duplicate_certificate);

        assert_pinned_certificate_chain_rejected(
            fixture,
            "duplicate X509Certificate DER entries must fail closed",
        );
    }

    #[test]
    fn require_verified_profile_rejects_mixed_public_key_and_certificate_material() {
        let payload = signed_pacs008_xml().replacen(
            "</KeyInfo>",
            "<X509Data><X509Certificate>AQ==</X509Certificate></X509Data></KeyInfo>",
            1,
        );

        assert_require_verified_signed_payload_rejected(
            payload,
            "mixed PublicKey and X509Certificate key material must fail closed",
        );
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
    fn profile_idempotency_rejects_replayed_business_message_id() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let first = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-1".to_owned()),
            None,
            "hash-1".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let replay = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some(" biz-1 ".to_owned()),
            None,
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
    fn retry_replacement_rejects_conflicting_business_message_id() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        let first = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-1".to_owned()),
            None,
            "hash-1".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let second = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-2".to_owned()),
            None,
            "hash-2".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let conflicting_retry = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some(" biz-2 ".to_owned()),
            None,
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
                .business_message_id_index
                .get(&normalise_business_message_id("biz-2"))
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
            let metadata = IsoMessageMetadata::inbound(
                "generic-iso20022",
                "pacs.008",
                None,
                Some("persisted-biz".to_owned()),
                None,
                "persisted-hash".to_owned(),
                "snapshot".to_owned(),
                false,
            );
            assert!(runtime.check_and_record_inbound("persisted-msg", metadata));
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
        let replay = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("persisted-biz".to_owned()),
            None,
            "replay-hash".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        assert!(!reloaded.check_and_record_inbound("persisted-replay", replay));
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
        assert_eq!(
            original.detail(),
            Some("payment returned by inbound pacs.004")
        );
        let lifecycle = runtime
            .message_status("return-1")
            .expect("lifecycle status");
        assert_eq!(lifecycle.status_label(), "Accepted");
        assert_eq!(lifecycle.pacs002_code(), "ACSP");
        assert_eq!(
            lifecycle.detail(),
            Some("recorded inbound ISO 20022 pacs.004 lifecycle message")
        );
    }

    #[test]
    fn lifecycle_camt056_marks_known_original_pending_cancellation() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        assert!(runtime.check_and_record_message("orig-cancel"));
        runtime.mark_accepted("orig-cancel", "tx-cancel");
        let parsed = parse_message(
            "camt.056",
            b"Assgnmt/Id=cancel-2\nAssgnmt/CreDtTm=2025-01-01T00:00:00Z\nUndrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId=orig-cancel\nUndrlyg/TxInf/CxlRsnInf/Rsn/Cd=CUST\nUndrlyg/TxInf/CxlRsnInf/AddtlInf=customer requested recall",
        )
        .expect("camt.056 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("camt.056", &parsed).expect("lifecycle id");
        assert_eq!(lifecycle_id, "cancel-2");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "camt.056", &parsed)
            .expect("lifecycle applied");

        assert_eq!(outcome.referenced_message_id(), Some("orig-cancel"));
        assert!(outcome.referenced_message_known());
        assert_eq!(outcome.lifecycle_status_code(), Some("PDNG"));
        assert_eq!(outcome.lifecycle_reason_code(), Some("CUST"));
        assert_eq!(outcome.action(), "marked_cancellation_requested");
        let original = runtime
            .message_status("orig-cancel")
            .expect("original status");
        assert_eq!(original.status_label(), "Pending");
        assert_eq!(original.pacs002_code(), "PDNG");
        assert_eq!(original.hold_reason_code(), Some("CUST"));
        assert!(
            original
                .change_reason_codes()
                .iter()
                .any(|code| code == "CANCELLATION_REQUESTED"),
            "expected cancellation reason to be recorded: {:?}",
            original.change_reason_codes()
        );
        let lifecycle = runtime
            .message_status("cancel-2")
            .expect("lifecycle status");
        assert_eq!(lifecycle.status_label(), "Accepted");
        assert_eq!(
            lifecycle.detail(),
            Some("recorded inbound ISO 20022 camt.056 lifecycle message")
        );
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
