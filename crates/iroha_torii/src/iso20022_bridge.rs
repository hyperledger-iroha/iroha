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
use sha1::Sha1;
use sha2::{Digest, Sha256};
use x509_parser::{
    extensions::{GeneralName, NameConstraints, ParsedExtension},
    oid_registry::OID_SIG_ECDSA_WITH_SHA256,
    prelude::{FromDer as _, X509Certificate},
    revocation_list::CertificateRevocationList,
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
    let signature_public_key_sha256_pins = normalise_profile_sha256_pins(
        id,
        "signature_public_key_sha256_pins",
        &config.signature_public_key_sha256_pins,
    )?;
    let x509_trust_anchor_sha256_pins = normalise_profile_sha256_pins(
        id,
        "x509_trust_anchor_sha256_pins",
        &config.x509_trust_anchor_sha256_pins,
    )?;
    let x509_required_certificate_policy_oids = normalise_profile_oid_literals(
        id,
        "x509_required_certificate_policy_oids",
        &config.x509_required_certificate_policy_oids,
    )?;
    let x509_crl_der_base64 =
        normalise_profile_crl_der_base64(id, "x509_crl_der_base64", &config.x509_crl_der_base64)?;
    let x509_ocsp_response_der_base64 = normalise_profile_ocsp_response_der_base64(
        id,
        "x509_ocsp_response_der_base64",
        &config.x509_ocsp_response_der_base64,
    )?;
    if message_profiles.is_empty() {
        eyre::bail!("iso_bridge profile `{id}` must define at least one message profile");
    }
    Ok(TradfiRailProfile {
        id: id.to_owned(),
        rail,
        embedded_signature_policy,
        signature_public_key_sha256_pins,
        x509_trust_anchor_sha256_pins,
        x509_required_certificate_policy_oids,
        x509_require_crl_revocation_check: config.x509_require_crl_revocation_check,
        x509_crl_der_base64,
        x509_require_ocsp_revocation_check: config.x509_require_ocsp_revocation_check,
        x509_ocsp_response_der_base64,
        required_reference_datasets,
        message_profiles,
    })
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

fn normalise_profile_sha256_pins(
    profile_id: &str,
    field: &str,
    pins: &[String],
) -> eyre::Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for pin in pins {
        let candidate = pin.trim().to_ascii_lowercase();
        if candidate.len() != 64 || !candidate.chars().all(|ch| ch.is_ascii_hexdigit()) {
            eyre::bail!(
                "iso_bridge profile `{profile_id}` {field} entries must be 64-character SHA-256 hex strings"
            );
        }
        if candidate.chars().all(|ch| ch == '0') {
            eyre::bail!(
                "iso_bridge profile `{profile_id}` {field} must not contain the all-zero placeholder"
            );
        }
        if seen.insert(candidate.clone()) {
            normalized.push(candidate);
        }
    }
    Ok(normalized)
}

fn normalise_profile_oid_literals(
    profile_id: &str,
    field: &str,
    values: &[String],
) -> eyre::Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let candidate = value.trim().to_owned();
        if !is_valid_oid_literal(&candidate) {
            eyre::bail!(
                "iso_bridge profile `{profile_id}` {field} entries must be dotted numeric OIDs"
            );
        }
        if seen.insert(candidate.clone()) {
            normalized.push(candidate);
        }
    }
    Ok(normalized)
}

fn normalise_profile_crl_der_base64(
    profile_id: &str,
    field: &str,
    values: &[String],
) -> eyre::Result<Vec<String>> {
    if values.len() > XMLDSIG_MAX_X509_CRLS {
        eyre::bail!(
            "iso_bridge profile `{profile_id}` {field} must not contain more than {XMLDSIG_MAX_X509_CRLS} CRLs"
        );
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let candidate = value.trim();
        let der = BASE64_STANDARD.decode(candidate).map_err(|_| {
            eyre::eyre!("iso_bridge profile `{profile_id}` {field} entries must be base64 DER CRLs")
        })?;
        if der.is_empty() || der.len() > XMLDSIG_MAX_X509_CRL_BYTES {
            eyre::bail!(
                "iso_bridge profile `{profile_id}` {field} entries must be non-empty CRLs no larger than {XMLDSIG_MAX_X509_CRL_BYTES} bytes"
            );
        }
        parse_x509_crl_der(&der).map_err(|_| {
            eyre::eyre!("iso_bridge profile `{profile_id}` {field} entries must parse as DER CRLs")
        })?;
        if seen.insert(sha256_hex(&der)) {
            normalized.push(BASE64_STANDARD.encode(&der));
        }
    }
    Ok(normalized)
}

fn normalise_profile_ocsp_response_der_base64(
    profile_id: &str,
    field: &str,
    values: &[String],
) -> eyre::Result<Vec<String>> {
    if values.len() > XMLDSIG_MAX_X509_OCSP_RESPONSES {
        eyre::bail!(
            "iso_bridge profile `{profile_id}` {field} must not contain more than {XMLDSIG_MAX_X509_OCSP_RESPONSES} OCSP responses"
        );
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let candidate = value.trim();
        let der = BASE64_STANDARD.decode(candidate).map_err(|_| {
            eyre::eyre!(
                "iso_bridge profile `{profile_id}` {field} entries must be base64 DER OCSP responses"
            )
        })?;
        if der.is_empty() || der.len() > XMLDSIG_MAX_X509_OCSP_RESPONSE_BYTES {
            eyre::bail!(
                "iso_bridge profile `{profile_id}` {field} entries must be non-empty OCSP responses no larger than {XMLDSIG_MAX_X509_OCSP_RESPONSE_BYTES} bytes"
            );
        }
        parse_ocsp_response_der(&der).map_err(|_| {
            eyre::eyre!(
                "iso_bridge profile `{profile_id}` {field} entries must parse as DER OCSP responses"
            )
        })?;
        if seen.insert(sha256_hex(&der)) {
            normalized.push(BASE64_STANDARD.encode(&der));
        }
    }
    Ok(normalized)
}

fn is_valid_oid_literal(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    let Some(second) = parts.next() else {
        return false;
    };
    if first.is_empty()
        || second.is_empty()
        || !first.chars().all(|ch| ch.is_ascii_digit())
        || !second.chars().all(|ch| ch.is_ascii_digit())
    {
        return false;
    }
    if !parts.all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit())) {
        return false;
    }
    let Ok(first_arc) = first.parse::<u32>() else {
        return false;
    };
    let Ok(second_arc) = second.parse::<u32>() else {
        return false;
    };
    first_arc <= 2 && (first_arc == 2 || second_arc <= 39)
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
                if !embedded_signature_detected
                    || !verify_embedded_xml_signature(
                        payload,
                        &profile.signature_public_key_sha256_pins,
                        &profile.x509_trust_anchor_sha256_pins,
                        &profile.x509_required_certificate_policy_oids,
                        profile.x509_require_crl_revocation_check,
                        &profile.x509_crl_der_base64,
                        profile.x509_require_ocsp_revocation_check,
                        &profile.x509_ocsp_response_der_base64,
                    )?
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
        let original_message_type = self
            .records
            .get(original_id)
            .and_then(|record| record.metadata.message_type().map(ToOwned::to_owned));
        if !lifecycle_update_matches_original(message_type, original_message_type.as_deref()) {
            return "ignored_profile_mismatch";
        }

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
                .and_then(normalise_business_message_id)
                .and_then(|id| {
                    self.business_message_id_index
                        .get(&id)
                        .map(|existing| existing.value().clone())
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
        if let Some(business_message_id) = metadata
            .business_message_id()
            .and_then(normalise_business_message_id)
        {
            self.business_message_id_index
                .insert(business_message_id, message_id.to_owned());
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
        if let Some(business_message_id) = record
            .metadata
            .business_message_id()
            .and_then(normalise_business_message_id)
        {
            self.business_message_id_index.remove(&business_message_id);
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

fn lifecycle_update_matches_original(lifecycle_type: &str, original_type: Option<&str>) -> bool {
    let Some(original_type) = original_type
        .map(str::trim)
        .filter(|message_type| !message_type.is_empty())
    else {
        return false;
    };
    match lifecycle_type {
        "pacs.002" | "pacs.004" | "camt.056" => {
            is_iso_family(original_type, "pacs.008") || is_iso_family(original_type, "pacs.009")
        }
        "sese.024" | "sese.025" => is_iso_family(original_type, "sese.023"),
        _ => false,
    }
}

fn is_iso_family(message_type: &str, family: &str) -> bool {
    let message_type = message_type.trim();
    message_type == family
        || message_type
            .strip_prefix(family)
            .is_some_and(|suffix| suffix.starts_with('.'))
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
const XMLDSIG_MAX_X509_CERTIFICATES: usize = 8;
const XMLDSIG_MAX_X509_CERTIFICATE_BYTES: usize = 16 * 1024;
const XMLDSIG_MAX_X509_CRLS: usize = 8;
const XMLDSIG_MAX_X509_CRL_BYTES: usize = 1024 * 1024;
const XMLDSIG_MAX_X509_OCSP_RESPONSES: usize = 8;
const XMLDSIG_MAX_X509_OCSP_RESPONSE_BYTES: usize = 1024 * 1024;
const OID_ECDSA_WITH_SHA256_DER: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x04, 0x03, 0x02];
const OID_OCSP_BASIC_RESPONSE_DER: &[u8] = &[0x2B, 0x06, 0x01, 0x05, 0x05, 0x07, 0x30, 0x01, 0x01];
const OID_SHA256_DER: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];

fn payload_has_embedded_signature(payload: &[u8]) -> bool {
    std::str::from_utf8(payload).is_ok_and(|text| {
        find_first_xml_element(text, "Signature").is_some()
            || find_first_xml_element(text, "Sgntr").is_some()
    })
}

fn verify_embedded_xml_signature(
    payload: &[u8],
    signature_public_key_sha256_pins: &[String],
    x509_trust_anchor_sha256_pins: &[String],
    x509_required_certificate_policy_oids: &[String],
    x509_require_crl_revocation_check: bool,
    x509_crl_der_base64: &[String],
    x509_require_ocsp_revocation_check: bool,
    x509_ocsp_response_der_base64: &[String],
) -> Result<bool, MsgError> {
    let text = std::str::from_utf8(payload).map_err(|_| MsgError::InvalidFormat)?;
    let Some(signature_span) =
        find_first_xml_element(text, "Signature").or_else(|| find_first_xml_element(text, "Sgntr"))
    else {
        return Ok(false);
    };
    if signature_public_key_sha256_pins.is_empty() && x509_trust_anchor_sha256_pins.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    if find_first_xml_element(&text[signature_span.end..], "Signature").is_some()
        || find_first_xml_element(&text[signature_span.end..], "Sgntr").is_some()
    {
        return Err(MsgError::ValidationFailed);
    }
    let signature_xml = &text[signature_span.start..signature_span.end];
    let signed_info_span =
        find_first_xml_element(signature_xml, "SignedInfo").ok_or(MsgError::ValidationFailed)?;
    let signed_info_xml = &signature_xml[signed_info_span.start..signed_info_span.end];
    let c14n_algorithm = child_attr(signed_info_xml, "CanonicalizationMethod", "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if c14n_algorithm != XML_C14N_1_0 {
        return Err(MsgError::ValidationFailed);
    }
    let signature_algorithm = child_attr(signed_info_xml, "SignatureMethod", "Algorithm")
        .ok_or(MsgError::ValidationFailed)?;
    if signature_algorithm != XMLDSIG_ECDSA_SHA256 {
        return Err(MsgError::ValidationFailed);
    }
    verify_xml_signature_references(text, signature_span, signed_info_xml)?;

    let signature_value = decode_required_child_base64(signature_xml, "SignatureValue")?;
    let public_key = xml_signature_public_key(
        signature_xml,
        signature_public_key_sha256_pins,
        x509_trust_anchor_sha256_pins,
        x509_required_certificate_policy_oids,
        x509_require_crl_revocation_check,
        x509_crl_der_base64,
        x509_require_ocsp_revocation_check,
        x509_ocsp_response_der_base64,
    )?;
    let verifying_key =
        P256VerifyingKey::from_sec1_bytes(&public_key).map_err(|_| MsgError::ValidationFailed)?;
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
    let transform_span =
        find_first_xml_element(reference_xml, "Transform").ok_or(MsgError::ValidationFailed)?;
    if find_first_xml_element(&reference_xml[transform_span.end..], "Transform").is_some() {
        return Err(MsgError::ValidationFailed);
    }
    if element_attr(reference_xml, transform_span, "Algorithm").as_deref()
        != Some(XMLDSIG_ENVELOPED_SIGNATURE)
    {
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

fn xml_signature_public_key(
    signature_xml: &str,
    signature_public_key_sha256_pins: &[String],
    x509_trust_anchor_sha256_pins: &[String],
    x509_required_certificate_policy_oids: &[String],
    x509_require_crl_revocation_check: bool,
    x509_crl_der_base64: &[String],
    x509_require_ocsp_revocation_check: bool,
    x509_ocsp_response_der_base64: &[String],
) -> Result<Vec<u8>, MsgError> {
    let public_key = single_child_text_compact(signature_xml, "PublicKey")?;
    let certificates = child_texts_compact(signature_xml, "X509Certificate");
    let embedded_crls = child_texts_compact(signature_xml, "X509CRL");
    let mut embedded_ocsp_responses = child_texts_compact(signature_xml, "OCSPResponse");
    embedded_ocsp_responses.extend(child_texts_compact(signature_xml, "EncapsulatedOCSPValue"));
    match (public_key, certificates.is_empty()) {
        (Some(public_key), true) => {
            if x509_require_crl_revocation_check
                || !x509_crl_der_base64.is_empty()
                || !embedded_crls.is_empty()
                || x509_require_ocsp_revocation_check
                || !x509_ocsp_response_der_base64.is_empty()
                || !embedded_ocsp_responses.is_empty()
            {
                return Err(MsgError::ValidationFailed);
            }
            let public_key = BASE64_STANDARD
                .decode(public_key)
                .map_err(|_| MsgError::ValidationFailed)?;
            if !public_key_pin_matches(&public_key, signature_public_key_sha256_pins) {
                return Err(MsgError::ValidationFailed);
            }
            Ok(public_key)
        }
        (None, false) => x509_signature_public_key(
            &certificates,
            &embedded_crls,
            &embedded_ocsp_responses,
            signature_public_key_sha256_pins,
            x509_trust_anchor_sha256_pins,
            x509_required_certificate_policy_oids,
            x509_require_crl_revocation_check,
            x509_crl_der_base64,
            x509_require_ocsp_revocation_check,
            x509_ocsp_response_der_base64,
        ),
        (Some(_), false) | (None, true) => Err(MsgError::ValidationFailed),
    }
}

fn x509_signature_public_key(
    certificate_values: &[String],
    embedded_crl_values: &[String],
    embedded_ocsp_response_values: &[String],
    signature_public_key_sha256_pins: &[String],
    x509_trust_anchor_sha256_pins: &[String],
    x509_required_certificate_policy_oids: &[String],
    x509_require_crl_revocation_check: bool,
    x509_crl_der_base64: &[String],
    x509_require_ocsp_revocation_check: bool,
    x509_ocsp_response_der_base64: &[String],
) -> Result<Vec<u8>, MsgError> {
    let certificate_chain = decode_x509_certificate_chain(certificate_values)?;
    ensure_x509_certificates_parse(&certificate_chain)?;
    let leaf = parse_x509_certificate_der(&certificate_chain[0])?;
    if !leaf.validity().is_valid() {
        return Err(MsgError::ValidationFailed);
    }
    if !x509_certificate_is_end_entity_signer(&leaf)? {
        return Err(MsgError::ValidationFailed);
    }
    if !x509_certificate_allows_digital_signature(&leaf)? {
        return Err(MsgError::ValidationFailed);
    }
    if !x509_certificate_satisfies_policy_oids(&leaf, x509_required_certificate_policy_oids)? {
        return Err(MsgError::ValidationFailed);
    }
    let public_key = leaf.public_key().subject_public_key.data.to_vec();
    let authorized_by_pin = public_key_pin_matches(&public_key, signature_public_key_sha256_pins);
    if !authorized_by_pin {
        validate_x509_certificate_chain(&certificate_chain, x509_trust_anchor_sha256_pins)?;
        validate_x509_name_constraints(&certificate_chain)?;
    }
    validate_x509_leaf_revocation(
        &certificate_chain,
        embedded_crl_values,
        x509_require_crl_revocation_check,
        x509_crl_der_base64,
        embedded_ocsp_response_values,
        x509_require_ocsp_revocation_check,
        x509_ocsp_response_der_base64,
    )?;
    if authorized_by_pin {
        return Ok(public_key);
    }
    Ok(public_key)
}

fn decode_x509_certificate_chain(certificate_values: &[String]) -> Result<Vec<Vec<u8>>, MsgError> {
    if certificate_values.is_empty() || certificate_values.len() > XMLDSIG_MAX_X509_CERTIFICATES {
        return Err(MsgError::ValidationFailed);
    }
    certificate_values
        .iter()
        .map(|certificate| {
            let certificate = BASE64_STANDARD
                .decode(certificate)
                .map_err(|_| MsgError::ValidationFailed)?;
            if certificate.is_empty() || certificate.len() > XMLDSIG_MAX_X509_CERTIFICATE_BYTES {
                return Err(MsgError::ValidationFailed);
            }
            Ok(certificate)
        })
        .collect()
}

fn ensure_x509_certificates_parse(certificate_chain: &[Vec<u8>]) -> Result<(), MsgError> {
    for certificate in certificate_chain {
        let certificate = parse_x509_certificate_der(certificate)?;
        validate_x509_certificate_critical_extensions(&certificate)?;
    }
    Ok(())
}

fn validate_x509_certificate_chain(
    certificate_chain: &[Vec<u8>],
    x509_trust_anchor_sha256_pins: &[String],
) -> Result<(), MsgError> {
    if certificate_chain.is_empty() || x509_trust_anchor_sha256_pins.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    let mut seen = HashSet::new();
    for certificate_der in certificate_chain {
        if !seen.insert(sha256_hex(certificate_der)) {
            return Err(MsgError::ValidationFailed);
        }
        let certificate = parse_x509_certificate_der(certificate_der)?;
        validate_x509_certificate_critical_extensions(&certificate)?;
        if !certificate.validity().is_valid() {
            return Err(MsgError::ValidationFailed);
        }
    }

    for index in 0..certificate_chain.len() {
        let certificate = parse_x509_certificate_der(&certificate_chain[index])?;
        if let Some(issuer_der) = certificate_chain.get(index + 1) {
            let issuer = parse_x509_certificate_der(issuer_der)?;
            if certificate.issuer() != issuer.subject() || !x509_certificate_is_ca(&issuer)? {
                return Err(MsgError::ValidationFailed);
            }
            verify_x509_certificate_signature(&certificate, &issuer)?;
        } else {
            if !certificate_der_pin_matches(
                &certificate_chain[index],
                x509_trust_anchor_sha256_pins,
            ) || !x509_certificate_is_ca(&certificate)?
            {
                return Err(MsgError::ValidationFailed);
            }
            if certificate.issuer() == certificate.subject() {
                verify_x509_certificate_signature(&certificate, &certificate)?;
            }
        }
    }
    validate_x509_path_length_constraints(certificate_chain)?;
    Ok(())
}

fn validate_x509_path_length_constraints(certificate_chain: &[Vec<u8>]) -> Result<(), MsgError> {
    let parsed_chain = certificate_chain
        .iter()
        .map(|certificate| parse_x509_certificate_der(certificate))
        .collect::<Result<Vec<_>, _>>()?;
    for issuer_index in 1..parsed_chain.len() {
        let Some(basic_constraints) = parsed_chain[issuer_index]
            .basic_constraints()
            .map_err(|_| MsgError::ValidationFailed)?
        else {
            return Err(MsgError::ValidationFailed);
        };
        if !basic_constraints.value.ca {
            return Err(MsgError::ValidationFailed);
        }
        let Some(path_len_constraint) = basic_constraints.value.path_len_constraint else {
            continue;
        };
        let mut subordinate_ca_count = 0usize;
        for subordinate in &parsed_chain[1..issuer_index] {
            if x509_certificate_is_ca(subordinate)? && subordinate.subject() != subordinate.issuer()
            {
                subordinate_ca_count += 1;
            }
        }
        if subordinate_ca_count > path_len_constraint as usize {
            return Err(MsgError::ValidationFailed);
        }
    }
    Ok(())
}

fn validate_x509_name_constraints(certificate_chain: &[Vec<u8>]) -> Result<(), MsgError> {
    let parsed_chain = certificate_chain
        .iter()
        .map(|certificate| parse_x509_certificate_der(certificate))
        .collect::<Result<Vec<_>, _>>()?;
    for issuer_index in 1..parsed_chain.len() {
        let Some(name_constraints) = parsed_chain[issuer_index]
            .name_constraints()
            .map_err(|_| MsgError::ValidationFailed)?
        else {
            continue;
        };
        validate_x509_name_constraint_bases(name_constraints.value)?;
        for certificate in &parsed_chain[..issuer_index] {
            validate_x509_certificate_against_name_constraints(
                certificate,
                name_constraints.value,
            )?;
        }
    }
    Ok(())
}

fn validate_x509_name_constraint_bases(constraints: &NameConstraints<'_>) -> Result<(), MsgError> {
    if let Some(permitted_subtrees) = &constraints.permitted_subtrees {
        for subtree in permitted_subtrees {
            x509_constraint_kind(&subtree.base)?;
        }
    }
    if let Some(excluded_subtrees) = &constraints.excluded_subtrees {
        for subtree in excluded_subtrees {
            x509_constraint_kind(&subtree.base)?;
        }
    }
    Ok(())
}

fn validate_x509_certificate_against_name_constraints(
    certificate: &X509Certificate<'_>,
    constraints: &NameConstraints<'_>,
) -> Result<(), MsgError> {
    let names = x509_certificate_presented_names(certificate)?;
    if let Some(excluded_subtrees) = &constraints.excluded_subtrees {
        for subtree in excluded_subtrees {
            for name in &names {
                if x509_presented_name_kind(name) == x509_constraint_kind(&subtree.base)?
                    && x509_presented_name_matches_constraint(name, &subtree.base)?
                {
                    return Err(MsgError::ValidationFailed);
                }
            }
        }
    }
    if let Some(permitted_subtrees) = &constraints.permitted_subtrees {
        let permitted_kinds = permitted_subtrees
            .iter()
            .map(|subtree| x509_constraint_kind(&subtree.base))
            .collect::<Result<HashSet<_>, _>>()?;
        for kind in permitted_kinds {
            for name in names
                .iter()
                .filter(|name| x509_presented_name_kind(name) == kind)
            {
                let permitted = permitted_subtrees.iter().any(|subtree| {
                    x509_constraint_kind(&subtree.base).is_ok_and(|constraint_kind| {
                        constraint_kind == kind
                            && x509_presented_name_matches_constraint(name, &subtree.base)
                                .unwrap_or(false)
                    })
                });
                if !permitted {
                    return Err(MsgError::ValidationFailed);
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum X509GeneralNameKind {
    DirectoryName,
    Dns,
    Ip,
    Rfc822,
    Uri,
}

enum X509PresentedName {
    DirectoryName(String),
    Dns(String),
    Ip(Vec<u8>),
    Rfc822(String),
    Uri(String),
}

fn x509_certificate_presented_names(
    certificate: &X509Certificate<'_>,
) -> Result<Vec<X509PresentedName>, MsgError> {
    let mut names = vec![X509PresentedName::DirectoryName(
        certificate.subject().to_string(),
    )];
    if let Some(subject_alternative_name) = certificate
        .subject_alternative_name()
        .map_err(|_| MsgError::ValidationFailed)?
    {
        for name in &subject_alternative_name.value.general_names {
            names.push(match name {
                GeneralName::DirectoryName(name) => {
                    X509PresentedName::DirectoryName(name.to_string())
                }
                GeneralName::DNSName(name) => X509PresentedName::Dns((*name).to_owned()),
                GeneralName::IPAddress(name) => X509PresentedName::Ip((*name).to_vec()),
                GeneralName::RFC822Name(name) => X509PresentedName::Rfc822((*name).to_owned()),
                GeneralName::URI(name) => X509PresentedName::Uri((*name).to_owned()),
                GeneralName::Invalid(_, _) => return Err(MsgError::ValidationFailed),
                GeneralName::EDIPartyName(_)
                | GeneralName::OtherName(_, _)
                | GeneralName::RegisteredID(_)
                | GeneralName::X400Address(_) => return Err(MsgError::ValidationFailed),
            });
        }
    }
    Ok(names)
}

fn x509_presented_name_kind(name: &X509PresentedName) -> X509GeneralNameKind {
    match name {
        X509PresentedName::DirectoryName(_) => X509GeneralNameKind::DirectoryName,
        X509PresentedName::Dns(_) => X509GeneralNameKind::Dns,
        X509PresentedName::Ip(_) => X509GeneralNameKind::Ip,
        X509PresentedName::Rfc822(_) => X509GeneralNameKind::Rfc822,
        X509PresentedName::Uri(_) => X509GeneralNameKind::Uri,
    }
}

fn x509_constraint_kind(base: &GeneralName<'_>) -> Result<X509GeneralNameKind, MsgError> {
    match base {
        GeneralName::DirectoryName(_) => Ok(X509GeneralNameKind::DirectoryName),
        GeneralName::DNSName(_) => Ok(X509GeneralNameKind::Dns),
        GeneralName::IPAddress(_) => Ok(X509GeneralNameKind::Ip),
        GeneralName::RFC822Name(_) => Ok(X509GeneralNameKind::Rfc822),
        GeneralName::URI(_) => Ok(X509GeneralNameKind::Uri),
        GeneralName::Invalid(_, _)
        | GeneralName::EDIPartyName(_)
        | GeneralName::OtherName(_, _)
        | GeneralName::RegisteredID(_)
        | GeneralName::X400Address(_) => Err(MsgError::ValidationFailed),
    }
}

fn x509_presented_name_matches_constraint(
    name: &X509PresentedName,
    base: &GeneralName<'_>,
) -> Result<bool, MsgError> {
    match (name, base) {
        (X509PresentedName::DirectoryName(name), GeneralName::DirectoryName(base)) => {
            Ok(name == &base.to_string())
        }
        (X509PresentedName::Dns(name), GeneralName::DNSName(base)) => {
            dns_name_matches_constraint(name, base)
        }
        (X509PresentedName::Ip(name), GeneralName::IPAddress(base)) => {
            ip_address_matches_constraint(name, base)
        }
        (X509PresentedName::Rfc822(name), GeneralName::RFC822Name(base)) => {
            rfc822_name_matches_constraint(name, base)
        }
        (X509PresentedName::Uri(name), GeneralName::URI(base)) => {
            uri_name_matches_constraint(name, base)
        }
        _ => Ok(false),
    }
}

fn dns_name_matches_constraint(name: &str, base: &str) -> Result<bool, MsgError> {
    let name = normalise_dns_constraint_name(name, false)?;
    let base = normalise_dns_constraint_name(base, true)?;
    if let Some(suffix) = base.strip_prefix('.') {
        return Ok(name.len() > suffix.len() && name.ends_with(&base));
    }
    Ok(name == base || name.ends_with(&format!(".{base}")))
}

fn normalise_dns_constraint_name(value: &str, allow_leading_dot: bool) -> Result<String, MsgError> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if value.is_empty() || value.contains('*') {
        return Err(MsgError::ValidationFailed);
    }
    let labels = if allow_leading_dot {
        value.strip_prefix('.').unwrap_or(&value)
    } else {
        value.as_str()
    };
    if labels.is_empty()
        || labels.split('.').any(str::is_empty)
        || !labels
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.')
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok(value)
}

fn ip_address_matches_constraint(name: &[u8], base: &[u8]) -> Result<bool, MsgError> {
    let (address, mask) = match (name.len(), base.len()) {
        (4, 8) => (&base[..4], &base[4..]),
        (16, 32) => (&base[..16], &base[16..]),
        _ => return Err(MsgError::ValidationFailed),
    };
    Ok(name
        .iter()
        .zip(address.iter().zip(mask.iter()))
        .all(|(value, (base, mask))| value & mask == base & mask))
}

fn rfc822_name_matches_constraint(name: &str, base: &str) -> Result<bool, MsgError> {
    let name = name.trim().to_ascii_lowercase();
    let base = base.trim().to_ascii_lowercase();
    if name.is_empty() || base.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    if base.contains('@') {
        return Ok(name == base);
    }
    let Some((_, domain)) = name.rsplit_once('@') else {
        return Err(MsgError::ValidationFailed);
    };
    dns_name_matches_constraint(domain, &base)
}

fn uri_name_matches_constraint(name: &str, base: &str) -> Result<bool, MsgError> {
    let Some(host) = uri_host(name) else {
        return Err(MsgError::ValidationFailed);
    };
    dns_name_matches_constraint(host, base)
}

fn uri_host(uri: &str) -> Option<&str> {
    let (_, authority_and_path) = uri.split_once("://")?;
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(authority_and_path);
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    if host_port.starts_with('[') {
        return host_port
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .map(|(host, _)| host);
    }
    host_port.split(':').next().filter(|host| !host.is_empty())
}

fn validate_x509_leaf_revocation(
    certificate_chain: &[Vec<u8>],
    embedded_crl_values: &[String],
    x509_require_crl_revocation_check: bool,
    x509_crl_der_base64: &[String],
    embedded_ocsp_response_values: &[String],
    x509_require_ocsp_revocation_check: bool,
    x509_ocsp_response_der_base64: &[String],
) -> Result<(), MsgError> {
    let has_crl_material = !embedded_crl_values.is_empty() || !x509_crl_der_base64.is_empty();
    let has_ocsp_material =
        !embedded_ocsp_response_values.is_empty() || !x509_ocsp_response_der_base64.is_empty();
    if !has_crl_material && !has_ocsp_material {
        return if x509_require_crl_revocation_check || x509_require_ocsp_revocation_check {
            Err(MsgError::ValidationFailed)
        } else {
            Ok(())
        };
    }
    let leaf = parse_x509_certificate_der(&certificate_chain[0])?;
    let issuer = x509_leaf_issuer_certificate(certificate_chain)?;
    if has_crl_material || x509_require_crl_revocation_check {
        validate_x509_leaf_crl_revocation(
            &leaf,
            &issuer,
            embedded_crl_values,
            x509_require_crl_revocation_check,
            x509_crl_der_base64,
        )?;
    }
    if has_ocsp_material || x509_require_ocsp_revocation_check {
        validate_x509_leaf_ocsp_revocation(
            &leaf,
            &issuer,
            embedded_ocsp_response_values,
            x509_require_ocsp_revocation_check,
            x509_ocsp_response_der_base64,
        )?;
    }
    Ok(())
}

fn validate_x509_leaf_crl_revocation(
    leaf: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
    embedded_crl_values: &[String],
    x509_require_crl_revocation_check: bool,
    x509_crl_der_base64: &[String],
) -> Result<(), MsgError> {
    if embedded_crl_values.is_empty() && x509_crl_der_base64.is_empty() {
        return if x509_require_crl_revocation_check {
            Err(MsgError::ValidationFailed)
        } else {
            Ok(())
        };
    }
    if !x509_certificate_allows_crl_sign(issuer)? {
        return Err(MsgError::ValidationFailed);
    }
    let crl_der_values = decode_x509_crls(embedded_crl_values, x509_crl_der_base64)?;
    let mut matching_crl_seen = false;
    for crl_der in &crl_der_values {
        let crl = parse_x509_crl_der(crl_der)?;
        validate_x509_crl_freshness(&crl)?;
        if crl.issuer() != issuer.subject() {
            continue;
        }
        matching_crl_seen = true;
        verify_x509_crl_signature(&crl, &issuer)?;
        if crl
            .iter_revoked_certificates()
            .any(|revoked| revoked.raw_serial() == leaf.raw_serial())
        {
            return Err(MsgError::ValidationFailed);
        }
    }
    if !matching_crl_seen {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn validate_x509_leaf_ocsp_revocation(
    leaf: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
    embedded_ocsp_response_values: &[String],
    x509_require_ocsp_revocation_check: bool,
    x509_ocsp_response_der_base64: &[String],
) -> Result<(), MsgError> {
    if embedded_ocsp_response_values.is_empty() && x509_ocsp_response_der_base64.is_empty() {
        return if x509_require_ocsp_revocation_check {
            Err(MsgError::ValidationFailed)
        } else {
            Ok(())
        };
    }
    let ocsp_response_der_values =
        decode_x509_ocsp_responses(embedded_ocsp_response_values, x509_ocsp_response_der_base64)?;
    let mut matching_good_response_seen = false;
    for ocsp_response_der in &ocsp_response_der_values {
        match validate_ocsp_response_for_leaf(ocsp_response_der, leaf, issuer)? {
            OcspLeafStatus::Good => matching_good_response_seen = true,
            OcspLeafStatus::Revoked | OcspLeafStatus::Unknown => {
                return Err(MsgError::ValidationFailed);
            }
            OcspLeafStatus::NotForLeaf => {}
        }
    }
    if !matching_good_response_seen {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn decode_x509_crls(
    embedded_crl_values: &[String],
    x509_crl_der_base64: &[String],
) -> Result<Vec<Vec<u8>>, MsgError> {
    let crl_count = embedded_crl_values.len() + x509_crl_der_base64.len();
    if crl_count == 0 || crl_count > XMLDSIG_MAX_X509_CRLS {
        return Err(MsgError::ValidationFailed);
    }
    let mut seen = HashSet::new();
    let mut crls = Vec::with_capacity(crl_count);
    for value in embedded_crl_values.iter().chain(x509_crl_der_base64.iter()) {
        let crl = BASE64_STANDARD
            .decode(value)
            .map_err(|_| MsgError::ValidationFailed)?;
        if crl.is_empty() || crl.len() > XMLDSIG_MAX_X509_CRL_BYTES {
            return Err(MsgError::ValidationFailed);
        }
        if !seen.insert(sha256_hex(&crl)) {
            return Err(MsgError::ValidationFailed);
        }
        crls.push(crl);
    }
    Ok(crls)
}

fn decode_x509_ocsp_responses(
    embedded_ocsp_response_values: &[String],
    x509_ocsp_response_der_base64: &[String],
) -> Result<Vec<Vec<u8>>, MsgError> {
    let response_count = embedded_ocsp_response_values.len() + x509_ocsp_response_der_base64.len();
    if response_count == 0 || response_count > XMLDSIG_MAX_X509_OCSP_RESPONSES {
        return Err(MsgError::ValidationFailed);
    }
    let mut seen = HashSet::new();
    let mut responses = Vec::with_capacity(response_count);
    for value in embedded_ocsp_response_values
        .iter()
        .chain(x509_ocsp_response_der_base64.iter())
    {
        let response = BASE64_STANDARD
            .decode(value)
            .map_err(|_| MsgError::ValidationFailed)?;
        if response.is_empty() || response.len() > XMLDSIG_MAX_X509_OCSP_RESPONSE_BYTES {
            return Err(MsgError::ValidationFailed);
        }
        if !seen.insert(sha256_hex(&response)) {
            return Err(MsgError::ValidationFailed);
        }
        responses.push(response);
    }
    Ok(responses)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OcspCertStatus {
    Good,
    Revoked,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OcspLeafStatus {
    Good,
    Revoked,
    Unknown,
    NotForLeaf,
}

struct ParsedOcspResponse<'a> {
    responder_id: OcspResponderId<'a>,
    produced_at: ASN1Time,
    responses: Vec<ParsedOcspSingleResponse<'a>>,
    tbs_response_data: &'a [u8],
    signature_algorithm_oid: &'a [u8],
    signature_value: &'a [u8],
    responder_certificates: Vec<&'a [u8]>,
}

struct ParsedOcspSingleResponse<'a> {
    issuer_name_hash: &'a [u8],
    issuer_key_hash: &'a [u8],
    serial: &'a [u8],
    status: OcspCertStatus,
    this_update: ASN1Time,
    next_update: ASN1Time,
}

enum OcspResponderId<'a> {
    ByName(&'a [u8]),
    ByKey(&'a [u8]),
}

fn validate_ocsp_response_for_leaf(
    ocsp_response_der: &[u8],
    leaf: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<OcspLeafStatus, MsgError> {
    let response = parse_ocsp_response_der(ocsp_response_der)?;
    validate_ocsp_response_signature(&response, issuer)?;
    let mut matched_status = None;
    for single_response in &response.responses {
        if !ocsp_cert_id_matches_leaf(single_response, leaf, issuer) {
            continue;
        }
        validate_ocsp_response_freshness(response.produced_at, single_response)?;
        if matched_status.replace(single_response.status).is_some() {
            return Err(MsgError::ValidationFailed);
        }
    }
    Ok(match matched_status {
        Some(OcspCertStatus::Good) => OcspLeafStatus::Good,
        Some(OcspCertStatus::Revoked) => OcspLeafStatus::Revoked,
        Some(OcspCertStatus::Unknown) => OcspLeafStatus::Unknown,
        None => OcspLeafStatus::NotForLeaf,
    })
}

fn validate_ocsp_response_signature(
    response: &ParsedOcspResponse<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    if response.signature_algorithm_oid != OID_ECDSA_WITH_SHA256_DER {
        return Err(MsgError::ValidationFailed);
    }
    if ocsp_responder_id_matches_certificate(&response.responder_id, issuer)? {
        return verify_ocsp_signature_with_certificate(response, issuer);
    }
    for certificate_der in &response.responder_certificates {
        let responder = parse_x509_certificate_der(certificate_der)?;
        validate_x509_certificate_critical_extensions(&responder)?;
        if !responder.validity().is_valid()
            || responder.issuer() != issuer.subject()
            || !x509_certificate_allows_ocsp_signing(&responder)?
            || !ocsp_responder_id_matches_certificate(&response.responder_id, &responder)?
        {
            continue;
        }
        verify_x509_certificate_signature(&responder, issuer)?;
        verify_ocsp_signature_with_certificate(response, &responder)?;
        return Ok(());
    }
    Err(MsgError::ValidationFailed)
}

fn verify_ocsp_signature_with_certificate(
    response: &ParsedOcspResponse<'_>,
    signer: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    let public_key = signer.public_key().subject_public_key.data.to_vec();
    let verifying_key =
        P256VerifyingKey::from_sec1_bytes(&public_key).map_err(|_| MsgError::ValidationFailed)?;
    let signature = P256Signature::from_der(response.signature_value)
        .map_err(|_| MsgError::ValidationFailed)?;
    verifying_key
        .verify(response.tbs_response_data, &signature)
        .map_err(|_| MsgError::ValidationFailed)
}

fn x509_certificate_allows_ocsp_signing(
    certificate: &X509Certificate<'_>,
) -> Result<bool, MsgError> {
    let Some(eku) = certificate
        .extended_key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    Ok(eku.value.ocsp_signing && key_usage.value.digital_signature())
}

fn ocsp_responder_id_matches_certificate(
    responder_id: &OcspResponderId<'_>,
    certificate: &X509Certificate<'_>,
) -> Result<bool, MsgError> {
    match responder_id {
        OcspResponderId::ByName(name) => Ok(*name == certificate.subject().as_raw()),
        OcspResponderId::ByKey(key_hash) => {
            let public_key = certificate.public_key().subject_public_key.data.as_ref();
            let digest = Sha1::digest(public_key);
            Ok(*key_hash == &digest[..])
        }
    }
}

fn ocsp_cert_id_matches_leaf(
    response: &ParsedOcspSingleResponse<'_>,
    leaf: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> bool {
    let issuer_name_hash = Sha256::digest(issuer.subject().as_raw());
    let issuer_key_hash = Sha256::digest(issuer.public_key().subject_public_key.data.as_ref());
    response.serial == leaf.raw_serial()
        && response.issuer_name_hash == &issuer_name_hash[..]
        && response.issuer_key_hash == &issuer_key_hash[..]
}

fn validate_ocsp_response_freshness(
    produced_at: ASN1Time,
    response: &ParsedOcspSingleResponse<'_>,
) -> Result<(), MsgError> {
    let now = ASN1Time::now();
    if produced_at > now || response.this_update > now || response.next_update < now {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn parse_ocsp_response_der(ocsp_response_der: &[u8]) -> Result<ParsedOcspResponse<'_>, MsgError> {
    let response = der_expect_single(ocsp_response_der, 0x30)?;
    let mut response_content = response.value;
    let response_status = der_read_required_integer(&mut response_content, 0x0A)?;
    if response_status != 0 {
        return Err(MsgError::ValidationFailed);
    }
    let response_bytes = der_read_required(&mut response_content, 0xA0)?;
    if !response_content.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    let response_bytes = der_expect_single(response_bytes.value, 0x30)?;
    let mut response_bytes_content = response_bytes.value;
    let response_type = der_read_required(&mut response_bytes_content, 0x06)?;
    if response_type.value != OID_OCSP_BASIC_RESPONSE_DER {
        return Err(MsgError::ValidationFailed);
    }
    let basic_response = der_read_required(&mut response_bytes_content, 0x04)?;
    if !response_bytes_content.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    parse_ocsp_basic_response_der(basic_response.value)
}

fn parse_ocsp_basic_response_der(
    basic_response_der: &[u8],
) -> Result<ParsedOcspResponse<'_>, MsgError> {
    let basic_response = der_expect_single(basic_response_der, 0x30)?;
    let mut content = basic_response.value;
    let tbs_response_data = der_read_required(&mut content, 0x30)?;
    let (responder_id, produced_at, responses) = parse_ocsp_response_data(tbs_response_data.value)?;
    let signature_algorithm_oid = der_read_algorithm_identifier(&mut content)?;
    let signature_value = der_read_bit_string(&mut content)?;
    let responder_certificates = if content.first() == Some(&0xA0) {
        parse_ocsp_responder_certificates(der_read_required(&mut content, 0xA0)?.value)?
    } else {
        Vec::new()
    };
    if !content.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(ParsedOcspResponse {
        responder_id,
        produced_at,
        responses,
        tbs_response_data: tbs_response_data.full,
        signature_algorithm_oid,
        signature_value,
        responder_certificates,
    })
}

fn parse_ocsp_response_data<'a>(
    response_data: &'a [u8],
) -> Result<
    (
        OcspResponderId<'a>,
        ASN1Time,
        Vec<ParsedOcspSingleResponse<'a>>,
    ),
    MsgError,
> {
    let mut content = response_data;
    if content.first() == Some(&0xA0) {
        let _version = der_read_required(&mut content, 0xA0)?;
    }
    let responder_id = parse_ocsp_responder_id(&mut content)?;
    let produced_at = der_read_asn1_time(&mut content, 0x18)?;
    let responses = der_read_required(&mut content, 0x30)?;
    let responses = parse_ocsp_single_responses(responses.value)?;
    if content.first() == Some(&0xA1) {
        let _extensions = der_read_required(&mut content, 0xA1)?;
    }
    if !content.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok((responder_id, produced_at, responses))
}

fn parse_ocsp_responder_id<'a>(content: &mut &'a [u8]) -> Result<OcspResponderId<'a>, MsgError> {
    let element = der_read_element(content)?;
    match element.tag {
        0xA1 => {
            let name = der_expect_single(element.value, 0x30)?;
            Ok(OcspResponderId::ByName(name.full))
        }
        0x82 => Ok(OcspResponderId::ByKey(element.value)),
        0xA2 => {
            let key_hash = der_expect_single(element.value, 0x04)?;
            Ok(OcspResponderId::ByKey(key_hash.value))
        }
        _ => Err(MsgError::ValidationFailed),
    }
}

fn parse_ocsp_single_responses(
    responses_der: &[u8],
) -> Result<Vec<ParsedOcspSingleResponse<'_>>, MsgError> {
    let mut responses_content = responses_der;
    let mut responses = Vec::new();
    while !responses_content.is_empty() {
        let response = der_read_required(&mut responses_content, 0x30)?;
        responses.push(parse_ocsp_single_response(response.value)?);
    }
    if responses.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(responses)
}

fn parse_ocsp_single_response(
    response_der: &[u8],
) -> Result<ParsedOcspSingleResponse<'_>, MsgError> {
    let mut content = response_der;
    let cert_id = der_read_required(&mut content, 0x30)?;
    let (issuer_name_hash, issuer_key_hash, serial) = parse_ocsp_cert_id(cert_id.value)?;
    let status = parse_ocsp_cert_status(&mut content)?;
    let this_update = der_read_asn1_time(&mut content, 0x18)?;
    let next_update = if content.first() == Some(&0xA0) {
        let next_update = der_read_required(&mut content, 0xA0)?;
        der_read_asn1_time_single(next_update.value, 0x18)?
    } else {
        return Err(MsgError::ValidationFailed);
    };
    if content.first() == Some(&0xA1) {
        let _extensions = der_read_required(&mut content, 0xA1)?;
    }
    if !content.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(ParsedOcspSingleResponse {
        issuer_name_hash,
        issuer_key_hash,
        serial,
        status,
        this_update,
        next_update,
    })
}

fn parse_ocsp_cert_id(cert_id_der: &[u8]) -> Result<(&[u8], &[u8], &[u8]), MsgError> {
    let mut content = cert_id_der;
    let hash_algorithm_oid = der_read_algorithm_identifier(&mut content)?;
    if hash_algorithm_oid != OID_SHA256_DER {
        return Err(MsgError::ValidationFailed);
    }
    let issuer_name_hash = der_read_required(&mut content, 0x04)?;
    let issuer_key_hash = der_read_required(&mut content, 0x04)?;
    let serial = der_read_required(&mut content, 0x02)?;
    if issuer_name_hash.value.is_empty()
        || issuer_key_hash.value.is_empty()
        || serial.value.is_empty()
        || !content.is_empty()
    {
        return Err(MsgError::ValidationFailed);
    }
    Ok((issuer_name_hash.value, issuer_key_hash.value, serial.value))
}

fn parse_ocsp_cert_status(content: &mut &[u8]) -> Result<OcspCertStatus, MsgError> {
    let status = der_read_element(content)?;
    match status.tag {
        0x80 => {
            if !status.value.is_empty() {
                return Err(MsgError::ValidationFailed);
            }
            Ok(OcspCertStatus::Good)
        }
        0xA1 | 0x81 => Ok(OcspCertStatus::Revoked),
        0x82 => {
            if !status.value.is_empty() {
                return Err(MsgError::ValidationFailed);
            }
            Ok(OcspCertStatus::Unknown)
        }
        _ => Err(MsgError::ValidationFailed),
    }
}

fn parse_ocsp_responder_certificates(certs_der: &[u8]) -> Result<Vec<&[u8]>, MsgError> {
    let certs = der_expect_single(certs_der, 0x30)?;
    let mut content = certs.value;
    let mut certificates = Vec::new();
    while !content.is_empty() {
        let certificate = der_read_required(&mut content, 0x30)?;
        parse_x509_certificate_der(certificate.full)?;
        certificates.push(certificate.full);
    }
    Ok(certificates)
}

#[derive(Clone, Copy)]
struct DerElement<'a> {
    tag: u8,
    value: &'a [u8],
    full: &'a [u8],
}

fn der_expect_single(input: &[u8], tag: u8) -> Result<DerElement<'_>, MsgError> {
    let mut content = input;
    let element = der_read_required(&mut content, tag)?;
    if !content.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(element)
}

fn der_read_required<'a>(input: &mut &'a [u8], tag: u8) -> Result<DerElement<'a>, MsgError> {
    let element = der_read_element(input)?;
    if element.tag != tag {
        return Err(MsgError::ValidationFailed);
    }
    Ok(element)
}

fn der_read_element<'a>(input: &mut &'a [u8]) -> Result<DerElement<'a>, MsgError> {
    if input.len() < 2 {
        return Err(MsgError::ValidationFailed);
    }
    let start = *input;
    let tag = start[0];
    let length_byte = start[1];
    let (length, header_len) = if length_byte & 0x80 == 0 {
        (usize::from(length_byte), 2)
    } else {
        let length_len = usize::from(length_byte & 0x7F);
        if length_len == 0 || length_len > 4 || start.len() < 2 + length_len {
            return Err(MsgError::ValidationFailed);
        }
        let mut length = 0usize;
        for byte in &start[2..2 + length_len] {
            length = length
                .checked_mul(256)
                .and_then(|value| value.checked_add(usize::from(*byte)))
                .ok_or(MsgError::ValidationFailed)?;
        }
        if length < 128 {
            return Err(MsgError::ValidationFailed);
        }
        (length, 2 + length_len)
    };
    let end = header_len
        .checked_add(length)
        .ok_or(MsgError::ValidationFailed)?;
    if start.len() < end {
        return Err(MsgError::ValidationFailed);
    }
    let full = &start[..end];
    *input = &start[end..];
    Ok(DerElement {
        tag,
        value: &start[header_len..end],
        full,
    })
}

fn der_read_required_integer(input: &mut &[u8], tag: u8) -> Result<u64, MsgError> {
    let element = der_read_required(input, tag)?;
    der_integer_value(element.value)
}

fn der_integer_value(value: &[u8]) -> Result<u64, MsgError> {
    if value.is_empty() || value.len() > 8 || value[0] & 0x80 != 0 {
        return Err(MsgError::ValidationFailed);
    }
    let mut result = 0u64;
    for byte in value {
        result = result
            .checked_mul(256)
            .and_then(|current| current.checked_add(u64::from(*byte)))
            .ok_or(MsgError::ValidationFailed)?;
    }
    Ok(result)
}

fn der_read_algorithm_identifier<'a>(content: &mut &'a [u8]) -> Result<&'a [u8], MsgError> {
    let algorithm = der_read_required(content, 0x30)?;
    let mut algorithm_content = algorithm.value;
    let oid = der_read_required(&mut algorithm_content, 0x06)?;
    if !algorithm_content.is_empty() {
        let null = der_read_required(&mut algorithm_content, 0x05)?;
        if !null.value.is_empty() || !algorithm_content.is_empty() {
            return Err(MsgError::ValidationFailed);
        }
    }
    Ok(oid.value)
}

fn der_read_bit_string<'a>(content: &mut &'a [u8]) -> Result<&'a [u8], MsgError> {
    let bit_string = der_read_required(content, 0x03)?;
    if bit_string.value.first() != Some(&0) {
        return Err(MsgError::ValidationFailed);
    }
    Ok(&bit_string.value[1..])
}

fn der_read_asn1_time(content: &mut &[u8], tag: u8) -> Result<ASN1Time, MsgError> {
    let time = der_read_required(content, tag)?;
    der_asn1_time(time)
}

fn der_read_asn1_time_single(content: &[u8], tag: u8) -> Result<ASN1Time, MsgError> {
    let time = der_expect_single(content, tag)?;
    der_asn1_time(time)
}

fn der_asn1_time(time: DerElement<'_>) -> Result<ASN1Time, MsgError> {
    let (remaining, time) =
        ASN1Time::from_der(time.full).map_err(|_| MsgError::ValidationFailed)?;
    if !remaining.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(time)
}

fn x509_leaf_issuer_certificate<'a>(
    certificate_chain: &'a [Vec<u8>],
) -> Result<X509Certificate<'a>, MsgError> {
    let leaf = parse_x509_certificate_der(&certificate_chain[0])?;
    if let Some(issuer_der) = certificate_chain.get(1) {
        let issuer = parse_x509_certificate_der(issuer_der)?;
        if leaf.issuer() != issuer.subject() || !x509_certificate_is_ca(&issuer)? {
            return Err(MsgError::ValidationFailed);
        }
        verify_x509_certificate_signature(&leaf, &issuer)?;
        return Ok(issuer);
    }
    if leaf.issuer() == leaf.subject() {
        verify_x509_certificate_signature(&leaf, &leaf)?;
        return Ok(leaf);
    }
    Err(MsgError::ValidationFailed)
}

fn validate_x509_crl_freshness(crl: &CertificateRevocationList<'_>) -> Result<(), MsgError> {
    let now = ASN1Time::now();
    if crl.last_update() > now {
        return Err(MsgError::ValidationFailed);
    }
    let Some(next_update) = crl.next_update() else {
        return Err(MsgError::ValidationFailed);
    };
    if next_update < now {
        return Err(MsgError::ValidationFailed);
    }
    Ok(())
}

fn parse_x509_certificate_der(certificate_der: &[u8]) -> Result<X509Certificate<'_>, MsgError> {
    let (remaining, certificate) =
        X509Certificate::from_der(certificate_der).map_err(|_| MsgError::ValidationFailed)?;
    if !remaining.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(certificate)
}

fn parse_x509_crl_der(crl_der: &[u8]) -> Result<CertificateRevocationList<'_>, MsgError> {
    let (remaining, crl) =
        CertificateRevocationList::from_der(crl_der).map_err(|_| MsgError::ValidationFailed)?;
    if !remaining.is_empty() {
        return Err(MsgError::ValidationFailed);
    }
    Ok(crl)
}

fn x509_certificate_is_ca(certificate: &X509Certificate<'_>) -> Result<bool, MsgError> {
    let Some(basic_constraints) = certificate
        .basic_constraints()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    if !basic_constraints.value.ca {
        return Ok(false);
    }
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    Ok(key_usage.value.key_cert_sign())
}

fn x509_certificate_is_end_entity_signer(
    certificate: &X509Certificate<'_>,
) -> Result<bool, MsgError> {
    let Some(basic_constraints) = certificate
        .basic_constraints()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    Ok(!basic_constraints.value.ca)
}

fn x509_certificate_allows_crl_sign(certificate: &X509Certificate<'_>) -> Result<bool, MsgError> {
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    Ok(key_usage.value.crl_sign())
}

fn x509_certificate_allows_digital_signature(
    certificate: &X509Certificate<'_>,
) -> Result<bool, MsgError> {
    let Some(key_usage) = certificate
        .key_usage()
        .map_err(|_| MsgError::ValidationFailed)?
    else {
        return Ok(false);
    };
    Ok(key_usage.value.digital_signature())
}

fn x509_certificate_satisfies_policy_oids(
    certificate: &X509Certificate<'_>,
    required_policy_oids: &[String],
) -> Result<bool, MsgError> {
    if required_policy_oids.is_empty() {
        return Ok(true);
    }
    let mut policy_extension_count = 0usize;
    let mut present = HashSet::new();
    for extension in certificate.extensions() {
        if let ParsedExtension::CertificatePolicies(policies) = extension.parsed_extension() {
            policy_extension_count += 1;
            for policy in policies {
                present.insert(policy.policy_id.to_string());
            }
        }
    }
    if policy_extension_count != 1 {
        return Ok(false);
    }
    Ok(required_policy_oids
        .iter()
        .all(|required| present.contains(required)))
}

fn validate_x509_certificate_critical_extensions(
    certificate: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    for extension in certificate.extensions() {
        if !extension.critical {
            continue;
        }
        match extension.parsed_extension() {
            ParsedExtension::UnsupportedExtension { .. }
            | ParsedExtension::ParseError { .. }
            | ParsedExtension::Unparsed => return Err(MsgError::ValidationFailed),
            _ => {}
        }
    }
    Ok(())
}

fn verify_x509_certificate_signature(
    certificate: &X509Certificate<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    if certificate.signature_algorithm.algorithm != OID_SIG_ECDSA_WITH_SHA256
        || certificate.tbs_certificate.signature.algorithm != OID_SIG_ECDSA_WITH_SHA256
    {
        return Err(MsgError::ValidationFailed);
    }
    let issuer_public_key = issuer.public_key().subject_public_key.data.to_vec();
    let verifying_key = P256VerifyingKey::from_sec1_bytes(&issuer_public_key)
        .map_err(|_| MsgError::ValidationFailed)?;
    let signature = P256Signature::from_der(&certificate.signature_value.data)
        .map_err(|_| MsgError::ValidationFailed)?;
    verifying_key
        .verify(certificate.tbs_certificate.as_ref(), &signature)
        .map_err(|_| MsgError::ValidationFailed)
}

fn verify_x509_crl_signature(
    crl: &CertificateRevocationList<'_>,
    issuer: &X509Certificate<'_>,
) -> Result<(), MsgError> {
    if crl.signature_algorithm.algorithm != OID_SIG_ECDSA_WITH_SHA256
        || crl.tbs_cert_list.signature.algorithm != OID_SIG_ECDSA_WITH_SHA256
    {
        return Err(MsgError::ValidationFailed);
    }
    let issuer_public_key = issuer.public_key().subject_public_key.data.to_vec();
    let verifying_key = P256VerifyingKey::from_sec1_bytes(&issuer_public_key)
        .map_err(|_| MsgError::ValidationFailed)?;
    let signature = P256Signature::from_der(&crl.signature_value.data)
        .map_err(|_| MsgError::ValidationFailed)?;
    verifying_key
        .verify(crl.tbs_cert_list.as_ref(), &signature)
        .map_err(|_| MsgError::ValidationFailed)
}

fn public_key_pin_matches(public_key: &[u8], pins: &[String]) -> bool {
    let public_key_pin = sha256_hex(public_key);
    pins.iter()
        .any(|pin| pin.eq_ignore_ascii_case(&public_key_pin))
}

fn certificate_der_pin_matches(certificate_der: &[u8], pins: &[String]) -> bool {
    let certificate_pin = sha256_hex(certificate_der);
    pins.iter()
        .any(|pin| pin.eq_ignore_ascii_case(&certificate_pin))
}

fn decode_required_child_base64(container: &str, child: &str) -> Result<Vec<u8>, MsgError> {
    let value = child_text_compact(container, child).ok_or(MsgError::ValidationFailed)?;
    BASE64_STANDARD
        .decode(value)
        .map_err(|_| MsgError::ValidationFailed)
}

fn child_attr(container: &str, child: &str, attr: &str) -> Option<String> {
    let span = find_first_xml_element(container, child)?;
    element_attr(container, span, attr)
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

fn child_texts_compact(container: &str, child: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = container;
    while let Some(span) = find_first_xml_element(rest, child) {
        values.push(
            rest[span.content_start..span.content_end]
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect(),
        );
        rest = &rest[span.end..];
    }
    values
}

fn single_child_text_compact(container: &str, child: &str) -> Result<Option<String>, MsgError> {
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

fn normalise_business_message_id(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
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
    use p256::{
        ecdsa::{SigningKey as P256SigningKey, signature::Signer as _},
        pkcs8::DecodePrivateKey as _,
    };
    use tempfile::{NamedTempFile, TempDir};

    use super::*;

    const LEGACY_PUBLIC_KEY_LITERAL: &str =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@test";
    const OFFICIAL_XSD_PACS008_001_08: &str =
        include_str!(r"../../../fixtures/iso20022/xsd/iso/pacs.008.001.08.xsd");
    const OFFICIAL_XSD_PACS009_001_08: &str =
        include_str!(r"../../../fixtures/iso20022/xsd/iso/pacs.009.001.08.xsd");
    const TEST_X509_CERTIFICATE_DER_B64: &str = "MIIBlTCCATugAwIBAgIUXiaSrYGsJgKt3u4x6BKKksfNLiAwCgYIKoZIzj0EAwIwIDEeMBwGA1UEAwwVSXJvaGEgSVNPIFRlc3QgU2lnbmVyMB4XDTI2MDYwMTExMjgyOVoXDTM2MDUyOTExMjgyOVowIDEeMBwGA1UEAwwVSXJvaGEgSVNPIFRlc3QgU2lnbmVyMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEAWqzmlOSaGzSgxorUv+ewSw3Fy3Sde/6hMOgKLkgt21a791Jequ1zTyts6rrpZoBLozZBqHl0A7E8vTW587o9KNTMFEwHQYDVR0OBBYEFBL9Vo3Y+LvYlfKs0v5haxjUm1rtMB8GA1UdIwQYMBaAFBL9Vo3Y+LvYlfKs0v5haxjUm1rtMA8GA1UdEwEB/wQFMAMBAf8wCgYIKoZIzj0EAwIDSAAwRQIgZQdqU2vQe1kA3tnVW3/Md+A0CvjHC4VwaRGw1GTVsQwCIQD7mrXnQAnnVmnriWX35eVtmDSz2uGA5Xztav0D1Gd0PA==";
    const TEST_X509_CHAIN_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgUryIDQvn/6N9fG2pQEVTQEP9UqzRspBrkNdwndN+s+WhRANCAARTR4/Y7uTA2FBawlygx0Q7obN3BTEoh3AKjHTqZl+nMYfoiY+9bGJ7YHVDy6Ca/xWf2Yd0y64/7P1Ti9rJqPM6";
    const TEST_X509_CHAIN_LEAF_CERTIFICATE_DER_B64: &str = "MIIBnzCCAUagAwIBAgIUUOi8MD0vRAq9AAbDmuxRfRDZdeQwCgYIKoZIzj0EAwIwHjEcMBoGA1UEAwwTSXJvaGEgSVNPIFRlc3QgUm9vdDAgFw0yNjA2MDExMjAwMzhaGA8yMTI2MDUwODEyMDAzOFowHjEcMBoGA1UEAwwTSXJvaGEgSVNPIFRlc3QgTGVhZjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABFNHj9ju5MDYUFrCXKDHRDuhs3cFMSiHcAqMdOpmX6cxh+iJj71sYntgdUPLoJr/FZ/Zh3TLrj/s/VOL2smo8zqjYDBeMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMB0GA1UdDgQWBBS7L2CAjWtk6fBcYscT8f7Cjpv4vDAfBgNVHSMEGDAWgBRD+yqEVN85+okIsrT2tOc86jK3HzAKBggqhkjOPQQDAgNHADBEAiAled9C2Mpk2BdR84/evD5DyQ+Kt9TZuNrZMkkrjx6tiwIgLHYNDEXdEZMgKj838ELQ/9vtz5f2WSNaUN5ehURQBjY=";
    const TEST_X509_CHAIN_ROOT_CERTIFICATE_DER_B64: &str = "MIIBpzCCAUygAwIBAgIUN59zZbCa78pNzUJAco7cB7HX9S0wCgYIKoZIzj0EAwIwHjEcMBoGA1UEAwwTSXJvaGEgSVNPIFRlc3QgUm9vdDAgFw0yNjA2MDExMjAwMzhaGA8yMTI2MDUwODEyMDAzOFowHjEcMBoGA1UEAwwTSXJvaGEgSVNPIFRlc3QgUm9vdDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABJtrF+Op4oQRaLLDdmComaR9cvtsrLpACh9gm1HLvCGC0HB9r50w6P1cgrEZ2j2dokVXpdx3axIUM9+BjrhmI4SjZjBkMB8GA1UdIwQYMBaAFEP7KoRU3zn6iQiytPa05zzqMrcfMBIGA1UdEwEB/wQIMAYBAf8CAQEwDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBRD+yqEVN85+okIsrT2tOc86jK3HzAKBggqhkjOPQQDAgNJADBGAiEA4Y9l0Zhe+cgWss7+jyOXJ/bsqLJ+MLGtD54H9dKRA/UCIQDRkR+vyF4COJO3ZGMeLGxZDf/e4bEYwEu72okoPu9/RA==";
    const TEST_X509_UNKNOWN_CRITICAL_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgwU8aNjPS3jld2m1xIeGpj9006lOpLzF66Jc3OrrvzsWhRANCAAQ7OfFyeTB68ox7nX66Br7Az7o1dG4lEePTIgQDxOWnQNrDJDMRDAC2SrB9qT02pqXShqN+i/zSnf/ULeuaZOmq";
    const TEST_X509_UNKNOWN_CRITICAL_LEAF_CERTIFICATE_DER_B64: &str = "MIIBxTCCAWqgAwIBAgICUQEwCgYIKoZIzj0EAwIwLzEtMCsGA1UEAwwkSXJvaGEgSVNPIFVua25vd24gQ3JpdGljYWwgVGVzdCBSb290MCAXDTI2MDYwMTAwMDAwMFoYDzIxMjYwNTA4MDAwMDAwWjAvMS0wKwYDVQQDDCRJcm9oYSBJU08gVW5rbm93biBDcml0aWNhbCBUZXN0IExlYWYwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQ7OfFyeTB68ox7nX66Br7Az7o1dG4lEePTIgQDxOWnQNrDJDMRDAC2SrB9qT02pqXShqN+i/zSnf/ULeuaZOmqo3QwcjAMBgNVHRMBAf8EAjAAMA4GA1UdDwEB/wQEAwIHgDAdBgNVHQ4EFgQU0PAOpCGj0CuPwuWetUFFPNgJVSYwHwYDVR0jBBgwFoAUFILzl7ccbofpo9P/TlWVjtux/uYwEgYFKgMEBQYBAf8EBgQEZGVueTAKBggqhkjOPQQDAgNJADBGAiEA1Xcu7wyj9eRGRCQQQhI75D+lRVootpWB2fw0I+ZJMxACIQCYhjqnSvkfMQDYbXtgVhjlzGrGmrMZW7OCkPzLH4TMPg==";
    const TEST_X509_UNKNOWN_CRITICAL_ROOT_CERTIFICATE_DER_B64: &str = "MIIBtzCCAVygAwIBAgICUQAwCgYIKoZIzj0EAwIwLzEtMCsGA1UEAwwkSXJvaGEgSVNPIFVua25vd24gQ3JpdGljYWwgVGVzdCBSb290MCAXDTI2MDYwMTAwMDAwMFoYDzIxMjYwNTA4MDAwMDAwWjAvMS0wKwYDVQQDDCRJcm9oYSBJU08gVW5rbm93biBDcml0aWNhbCBUZXN0IFJvb3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQ+qc16/Tw0+Jwm0K4Ubdse6Ucaf+GEqaX2QcM2DkakIJ0DLH+A8W6ZFlDHYH8Dswx3oNhwgkaGBG5XTykdmLVRo2YwZDAfBgNVHSMEGDAWgBQUgvOXtxxuh+mj0/9OVZWO27H+5jASBgNVHRMBAf8ECDAGAQH/AgEBMA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUFILzl7ccbofpo9P/TlWVjtux/uYwCgYIKoZIzj0EAwIDSQAwRgIhAJeaEZvvddOVWj1hCHp6D8UzU/iiHRPzHfKHmYMYfG0XAiEA2gtAnkmepG3hxrb3n03iZZ5pfI6CsZjA3m7XUoWGyto=";
    const TEST_X509_UNKNOWN_CRITICAL_ROOT_LEAF_CERTIFICATE_DER_B64: &str = "MIIBtDCCAVqgAwIBAgICUgEwCgYIKoZIzj0EAwIwMzExMC8GA1UEAwwoSXJvaGEgSVNPIFVua25vd24gQ3JpdGljYWwgVGVzdCBCYWQgUm9vdDAgFw0yNjA2MDEwMDAwMDBaGA8yMTI2MDUwODAwMDAwMFowLzEtMCsGA1UEAwwkSXJvaGEgSVNPIFVua25vd24gQ3JpdGljYWwgVGVzdCBMZWFmMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEOznxcnkwevKMe51+uga+wM+6NXRuJRHj0yIEA8Tlp0DawyQzEQwAtkqwfak9Nqal0oajfov80p3/1C3rmmTpqqNgMF4wDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwHQYDVR0OBBYEFNDwDqQho9Arj8LlnrVBRTzYCVUmMB8GA1UdIwQYMBaAFCf5sYdzYU0M0fzUbICr/ouYALzHMAoGCCqGSM49BAMCA0gAMEUCIDMhhqN38SBoNdDr3BlTjDWJmkzrxBttCUcNEQZi/+gFAiEAy2n+QinRV9M+glNq6vuwAd/dMkfDY71LizMlVFcf0AY=";
    const TEST_X509_UNKNOWN_CRITICAL_BAD_ROOT_CERTIFICATE_DER_B64: &str = "MIIB0zCCAXigAwIBAgICUgAwCgYIKoZIzj0EAwIwMzExMC8GA1UEAwwoSXJvaGEgSVNPIFVua25vd24gQ3JpdGljYWwgVGVzdCBCYWQgUm9vdDAgFw0yNjA2MDEwMDAwMDBaGA8yMTI2MDUwODAwMDAwMFowMzExMC8GA1UEAwwoSXJvaGEgSVNPIFVua25vd24gQ3JpdGljYWwgVGVzdCBCYWQgUm9vdDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABIhI+nhfc40RMDjRIT88GenkZGd+6QRVvBg3RGIDvSi07ARZ6nIuFDaTQfX70kvHmj4oSnZ+jxXVH/6AvqwT6wSjejB4MB8GA1UdIwQYMBaAFCf5sYdzYU0M0fzUbICr/ouYALzHMBIGA1UdEwEB/wQIMAYBAf8CAQEwDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBQn+bGHc2FNDNH81GyAq/6LmAC8xzASBgUqAwQFBgEB/wQGBARkZW55MAoGCCqGSM49BAMCA0kAMEYCIQDxIen+s/EZE5Aq6NKUN1Dtq6Q2J974ceGImv5N1wvKUwIhAJ1znLxRlHxiRlUzCHNNjsb8zev7depz/giUUzFRqxRt";
    const TEST_X509_EXPIRED_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgu7FcG13Y1bLf6M5wsHpylL9IBEbqcbNv7LubaQU1SU2hRANCAATzDPtRqdSbvfazWua4HD9f9eOvZhZbalrQBdeZKuWbrZu7UKwDcOiPKD5bSU2TwbJ617zEhuLBVqA0aXBPnQKF";
    const TEST_X509_EXPIRED_LEAF_CERTIFICATE_DER_B64: &str = "MIIBqDCCAU6gAwIBAgICUwEwCgYIKoZIzj0EAwIwKzEpMCcGA1UEAwwgSXJvaGEgSVNPIEV4cGlyZWQgTGVhZiBUZXN0IFJvb3QwHhcNMjAwMTAxMDAwMDAwWhcNMjAwMTAyMDAwMDAwWjAtMSswKQYDVQQDDCJJcm9oYSBJU08gRXhwaXJlZCBMZWFmIFRlc3QgU2lnbmVyMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE8wz7UanUm732s1rmuBw/X/Xjr2YWW2pa0AXXmSrlm62bu1CsA3Dojyg+W0lNk8Gyete8xIbiwVagNGlwT50ChaNgMF4wDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwHQYDVR0OBBYEFKx0rYXMy3zJXZogrHDrrTvs6iWUMB8GA1UdIwQYMBaAFJxKYQ0QVEQKSuGb2txaz6IkttNxMAoGCCqGSM49BAMCA0gAMEUCIQD8iTy25LrcubWcZMZFE98B46zvqP+G8UoVIFprf2KohQIgceHQlTJdmI5EdbLgc2E7w8Bgy0YHFSz0MdfyauUmKow=";
    const TEST_X509_EXPIRED_ROOT_CERTIFICATE_DER_B64: &str = "MIIBrjCCAVSgAwIBAgICUwAwCgYIKoZIzj0EAwIwKzEpMCcGA1UEAwwgSXJvaGEgSVNPIEV4cGlyZWQgTGVhZiBUZXN0IFJvb3QwIBcNMjAwMTAxMDAwMDAwWhgPMjEyNjA1MDgwMDAwMDBaMCsxKTAnBgNVBAMMIElyb2hhIElTTyBFeHBpcmVkIExlYWYgVGVzdCBSb290MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE3Ch3zNLwM5sPUEkqV8XKfWk6kJo3WBoj7hvbu2SpfLIQ2NeWwh7p+1kPZgGyTZYU4igN81H9A/qQnPDdka5lJaNmMGQwHwYDVR0jBBgwFoAUnEphDRBURApK4Zva3FrPoiS203EwEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFJxKYQ0QVEQKSuGb2txaz6IkttNxMAoGCCqGSM49BAMCA0gAMEUCIQCga2/Sf9eBHaZyALOuo3qBaYi85U1aRQ1aSuLkSZQY6QIgD6aiw2YfEImma3f+a7Mi/TCGTJNLcihkIZTRIZxW5EU=";
    const TEST_X509_PATHLEN_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg3nuXdJt4HKvYmPHNQhHghYBH5m2rJhLM6X2mb3ZEAiWhRANCAAQSY1wJcCoKdL3Jofs44Th6YP3PrDlitaq8jMZY10IlqxoRWJCt4cQNbFs9MOjiDirdTGH/1LtNso+pt4/7A9nW";
    const TEST_X509_PATHLEN_LEAF_CERTIFICATE_DER_B64: &str = "MIIBuDCCAV6gAwIBAgIUAroD5KgdplQBUFMVASJFffMavp8wCgYIKoZIzj0EAwIwLjEsMCoGA1UEAwwjSXJvaGEgSVNPIFBhdGhMZW4gVGVzdCBJbnRlcm1lZGlhdGUwIBcNMjYwNjAxMTUxOTMxWhgPMjEyNjA1MDgxNTE5MzFaMCYxJDAiBgNVBAMMG0lyb2hhIElTTyBQYXRoTGVuIFRlc3QgTGVhZjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABBJjXAlwKgp0vcmh+zjhOHpg/c+sOWK1qryMxljXQiWrGhFYkK3hxA1sWz0w6OIOKt1MYf/Uu02yj6m3j/sD2dajYDBeMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMB0GA1UdDgQWBBRd71CtjCd8i1OrveZVj7PhbftdozAfBgNVHSMEGDAWgBQ+9z6697KLGLz8FsX7pXkDmF0y0jAKBggqhkjOPQQDAgNIADBFAiEAqH2FNcRnBYe2euURS2b4HiWwDsDBLaYKvJUqcaGSF8kCIAr+D9sGxQyezdmBSLqdtK6Vf05V3CoDSQHAGcnhJgDU";
    const TEST_X509_PATHLEN_ROOT0_CERTIFICATE_DER_B64: &str = "MIIBtzCCAVygAwIBAgIUQdOUC+LffkVAc60QoHHaziSBgJMwCgYIKoZIzj0EAwIwJjEkMCIGA1UEAwwbSXJvaGEgSVNPIFBhdGhMZW4gVGVzdCBSb290MCAXDTI2MDYwMTE1MTkzMVoYDzIxMjYwNTA4MTUxOTMxWjAmMSQwIgYDVQQDDBtJcm9oYSBJU08gUGF0aExlbiBUZXN0IFJvb3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAATHfMRmNRjSg2Q0EEqdYcGAJVEKCCTOnpjDZKgeUb/AwXgu42NfeTHsq7t3wFPd/pbCIqhBp/vJqw4btetClty+o2YwZDASBgNVHRMBAf8ECDAGAQH/AgEAMA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQULFjuLrmZd/w+EKb109b4f6d0C7gwHwYDVR0jBBgwFoAULFjuLrmZd/w+EKb109b4f6d0C7gwCgYIKoZIzj0EAwIDSQAwRgIhAIBI5KM6i2Abfp7Qn2AKIht2AMV1LAdLDDOK3+sLNPsKAiEA6eEOPnEEFLK47Xw8h+Gho0BzPRwtUBPYrTDadszwWRI=";
    const TEST_X509_PATHLEN_ROOT1_CERTIFICATE_DER_B64: &str = "MIIBtjCCAVygAwIBAgIUVYUM67urrA2F95Tj31aPkK6AugAwCgYIKoZIzj0EAwIwJjEkMCIGA1UEAwwbSXJvaGEgSVNPIFBhdGhMZW4gVGVzdCBSb290MCAXDTI2MDYwMTE1MTkzMVoYDzIxMjYwNTA4MTUxOTMxWjAmMSQwIgYDVQQDDBtJcm9oYSBJU08gUGF0aExlbiBUZXN0IFJvb3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQvJ/kW/Ql7XZrRKiwuSP76VNYewdBChK8sFJZD60oQMWgSOgWmoknQuB0xandKtoROoTTCMfgSo+EcMB4tFsono2YwZDASBgNVHRMBAf8ECDAGAQH/AgEBMA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUI79hYad5xLdxwDxxW9GIRDD18p8wHwYDVR0jBBgwFoAUI79hYad5xLdxwDxxW9GIRDD18p8wCgYIKoZIzj0EAwIDSAAwRQIgYo4bO/nU5nq3XUWFeydIRT8E0p1sOAEjfRHLFXFZ/wECIQCzoHmyP9UXMAipsQfWAwgaxuhdJKzcnN3ImSwngmm7xA==";
    const TEST_X509_PATHLEN_INTERMEDIATE_BY_ROOT0_CERTIFICATE_DER_B64: &str = "MIIBvzCCAWSgAwIBAgIUGeFrg7nJWaNu59GA5EOLYc9FC20wCgYIKoZIzj0EAwIwJjEkMCIGA1UEAwwbSXJvaGEgSVNPIFBhdGhMZW4gVGVzdCBSb290MCAXDTI2MDYwMTE1MTkzMVoYDzIxMjYwNTA4MTUxOTMxWjAuMSwwKgYDVQQDDCNJcm9oYSBJU08gUGF0aExlbiBUZXN0IEludGVybWVkaWF0ZTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABICoJU5zZO+jzgkEKvJyFJVQ4oJTyNwovh9S0lGDJ/zNoQ+Mg/tK7YdYsx7j2YYpVQZTydSvosMXJtwEmXTWX/6jZjBkMBIGA1UdEwEB/wQIMAYBAf8CAQAwDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBQ+9z6697KLGLz8FsX7pXkDmF0y0jAfBgNVHSMEGDAWgBQsWO4uuZl3/D4QpvXT1vh/p3QLuDAKBggqhkjOPQQDAgNJADBGAiEAvkSjmAyWMrrTO5uAfBOw0MEqRUqED1BVry8JEl+FARQCIQDbK10MV3v2txjHoryr79TRMxdC7uLgCXuzpukRFNkvfQ==";
    const TEST_X509_PATHLEN_INTERMEDIATE_BY_ROOT1_CERTIFICATE_DER_B64: &str = "MIIBvjCCAWSgAwIBAgIUQvlda1/AptwyFoBDZw8XKMW9fi4wCgYIKoZIzj0EAwIwJjEkMCIGA1UEAwwbSXJvaGEgSVNPIFBhdGhMZW4gVGVzdCBSb290MCAXDTI2MDYwMTE1MTkzMVoYDzIxMjYwNTA4MTUxOTMxWjAuMSwwKgYDVQQDDCNJcm9oYSBJU08gUGF0aExlbiBUZXN0IEludGVybWVkaWF0ZTBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABICoJU5zZO+jzgkEKvJyFJVQ4oJTyNwovh9S0lGDJ/zNoQ+Mg/tK7YdYsx7j2YYpVQZTydSvosMXJtwEmXTWX/6jZjBkMBIGA1UdEwEB/wQIMAYBAf8CAQAwDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQWBBQ+9z6697KLGLz8FsX7pXkDmF0y0jAfBgNVHSMEGDAWgBQjv2Fhp3nEt3HAPHFb0YhEMPXynzAKBggqhkjOPQQDAgNIADBFAiBB8NTKueclNUvxh7z9/rsE04Ct3TyarUtIroAuRF52tgIhAM3SGfSqiKTzNiFmM6jUxIAxV2zQ3heg6dHMiVQlD1+A";
    const TEST_X509_CA_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgg2pYEgqawGgRejSavdNh0i6zsVq9SoQVJRPpsdu3R7mhRANCAASxJvg1WnWA9S5TNmoxBb0/wI1J6SzCA3NP7QzjejecqvID6K5lLSgxjrG+wjBR5VXozh7bnLaPTLI/Uv2qERHA";
    const TEST_X509_CA_LEAF_CERTIFICATE_DER_B64: &str = "MIIBuTCCAV6gAwIBAgIUYGDPiGTotSHSfWM8XYe/GEbgxMAwCgYIKoZIzj0EAwIwJjEkMCIGA1UEAwwbSXJvaGEgSVNPIENBIExlYWYgVGVzdCBSb290MCAXDTI2MDYwMTE1Mzk0OFoYDzIxMjYwNTA4MTUzOTQ4WjAoMSYwJAYDVQQDDB1Jcm9oYSBJU08gQ0EgTGVhZiBUZXN0IFNpZ25lcjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABLEm+DVadYD1LlM2ajEFvT/AjUnpLMIDc0/tDON6N5yq8gPormUtKDGOsb7CMFHlVejOHtucto9Msj9S/aoREcCjZjBkMBIGA1UdEwEB/wQIMAYBAf8CAQAwDgYDVR0PAQH/BAQDAgGGMB0GA1UdDgQWBBTcnncTx0zw/Hfrbw/23gK54Jcx+DAfBgNVHSMEGDAWgBSU0NnWDyVoLCSbSLjWVa39Qo7RzjAKBggqhkjOPQQDAgNJADBGAiEAwgSjxDFs4mVOJytH5JxTIaTBWMp1MlMXpRunfQ3Gj8MCIQD8pkrPjeAjm8Rj+A234oR5BALB2rJnHNRjChyXXxYoJA==";
    const TEST_X509_CA_LEAF_ROOT_CERTIFICATE_DER_B64: &str = "MIIBtTCCAVygAwIBAgIUWUVYri4LhKX478p+THSVJUaPOo0wCgYIKoZIzj0EAwIwJjEkMCIGA1UEAwwbSXJvaGEgSVNPIENBIExlYWYgVGVzdCBSb290MCAXDTI2MDYwMTE1Mzk0OFoYDzIxMjYwNTA4MTUzOTQ4WjAmMSQwIgYDVQQDDBtJcm9oYSBJU08gQ0EgTGVhZiBUZXN0IFJvb3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAARA+2dbKqgiqg/E83r4aUwHObSZ+F3vOpucoKVw8/t0DiK1Ip4ajtLDFBUBB9KRW9AZn3VqX3pigu6N8VXkRRJ+o2YwZDASBgNVHRMBAf8ECDAGAQH/AgEBMA4GA1UdDwEB/wQEAwIBBjAdBgNVHQ4EFgQUlNDZ1g8laCwkm0i41lWt/UKO0c4wHwYDVR0jBBgwFoAUlNDZ1g8laCwkm0i41lWt/UKO0c4wCgYIKoZIzj0EAwIDRwAwRAIgVnt/5K51wxExk4D7ndU8ZwyehZnQ7a3ZXUjSRHZeOAsCIEtJiNOCtiTilnOPOTLd44TpK/xJJ5en8VThPpTzvbQN";
    const TEST_X509_POLICY_OID: &str = "1.3.6.1.4.1.55555.1.1";
    const TEST_X509_POLICY_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgQf3eEFqA7jglOKGQ5WsPZaDme/HcKmlcWRlmlqR4bHGhRANCAASVooKXzWhB5zksBPxZWOT58wiH+r0Ibqwf2Ru5Rh7zw4nw8H7/j2vY8zbRJ3E3r/Rh3pOdK3z1Qw/w0bT3gB5A";
    const TEST_X509_POLICY_LEAF_CERTIFICATE_DER_B64: &str = "MIIBxzCCAW2gAwIBAgIUPuFsDaDMpHF3NgeTGnIEhCOq3NEwCgYIKoZIzj0EAwIwJTEjMCEGA1UEAwwaSXJvaGEgSVNPIFBvbGljeSBUZXN0IFJvb3QwIBcNMjYwNjAxMTIxNzAwWhgPMjEyNjA1MDgxMjE3MDBaMCUxIzAhBgNVBAMMGklyb2hhIElTTyBQb2xpY3kgVGVzdCBMZWFmMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAElaKCl81oQec5LAT8WVjk+fMIh/q9CG6sH9kbuUYe88OJ8PB+/49r2PM20SdxN6/0Yd6TnSt89UMP8NG094AeQKN5MHcwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwHQYDVR0OBBYEFER1X6dAjJEwRyZR0U9snoEqP1m3MB8GA1UdIwQYMBaAFDdfynzPJ3kOExwR74uNUYowe1IRMBcGA1UdIAQQMA4wDAYKKwYBBAGDsgMBATAKBggqhkjOPQQDAgNIADBFAiA6Q8LV3dY5W+t3TkfF4CG5cSPnp0Ck1Y8syE121dXjbgIhAP0h9a2FZzfQP04iu/e8T8iE/VK1EJ+cUQJaGs09Wa9R";
    const TEST_X509_POLICY_ROOT_CERTIFICATE_DER_B64: &str = "MIIBszCCAVqgAwIBAgIUL5B/imcHZ4KDLP8LZnFRqYSmTqEwCgYIKoZIzj0EAwIwJTEjMCEGA1UEAwwaSXJvaGEgSVNPIFBvbGljeSBUZXN0IFJvb3QwIBcNMjYwNjAxMTIxNzAwWhgPMjEyNjA1MDgxMjE3MDBaMCUxIzAhBgNVBAMMGklyb2hhIElTTyBQb2xpY3kgVGVzdCBSb290MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEbpV/eWHEwxV+no2+2M6BIGiPho0U+Ky+iUZ/J/4KXdEk0y81+/MC72LiD0rbSxLL+nxRE9VwhzILejQG8KVg8qNmMGQwHwYDVR0jBBgwFoAUN1/KfM8neQ4THBHvi41RijB7UhEwEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFDdfynzPJ3kOExwR74uNUYowe1IRMAoGCCqGSM49BAMCA0cAMEQCIFDwLtr3jD6nngIPGkIb5a5VU3vI8CEp1TLTFkZDEi1EAiANb9nOlWWaTbgaVU6efVlbrwNmMVgH3aVRcGszRdg4BA==";
    const TEST_X509_CRL_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgB7VSSxZw4i1maCz0tjs+Ig0seslvt33h+wfolIqxj1WhRANCAAS9vJu7tpKTX3O6vPLlaZNh+xMjHSJKNGmIXYYATkVrcb5E/S0TEDC7t2ZEm1erCRqjbOnoF9n2u6UmlaZCRoJ0";
    const TEST_X509_CRL_LEAF_CERTIFICATE_DER_B64: &str = "MIIBljCCATygAwIBAgICEAAwCgYIKoZIzj0EAwIwIjEgMB4GA1UEAwwXSXJvaGEgSVNPIENSTCBUZXN0IFJvb3QwIBcNMjYwNjAxMDAwMDAwWhgPMjEyNjA1MDgwMDAwMDBaMCIxIDAeBgNVBAMMF0lyb2hhIElTTyBDUkwgVGVzdCBMZWFmMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEvbybu7aSk19zurzy5WmTYfsTIx0iSjRpiF2GAE5Fa3G+RP0tExAwu7dmRJtXqwkao2zp6BfZ9rulJpWmQkaCdKNgMF4wDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwHQYDVR0OBBYEFA3XcZGfO52bKHnCO+4ym+iUaRQeMB8GA1UdIwQYMBaAFAfSITIYh7s/HFXEuWgTWUbY7FPYMAoGCCqGSM49BAMCA0gAMEUCIQDWg/T42cqMyz1Tv0BnJ3RIX7FXTMwm6qQD6wdMrSaqBQIgXb5ibnHAYMbe+lEwXDLY8sc9r2LlkWIC+hDuyzXB6cU=";
    const TEST_X509_CRL_ROOT_CERTIFICATE_DER_B64: &str = "MIIBrzCCAVSgAwIBAgIUJE41eX9WgYGLzBP/qB9/8FA/CxEwCgYIKoZIzj0EAwIwIjEgMB4GA1UEAwwXSXJvaGEgSVNPIENSTCBUZXN0IFJvb3QwIBcNMjYwNjAxMTMxNjU4WhgPMjEyNjA1MDgxMzE2NThaMCIxIDAeBgNVBAMMF0lyb2hhIElTTyBDUkwgVGVzdCBSb290MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEtgOtGpXNN/v42vMmR8TYH2rdZi+q/medyz6OmLvSsI68PD1170XuJV1C9qPC3rdWztfYo2RuBndGfLc0IoCfzqNmMGQwEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFAfSITIYh7s/HFXEuWgTWUbY7FPYMB8GA1UdIwQYMBaAFAfSITIYh7s/HFXEuWgTWUbY7FPYMAoGCCqGSM49BAMCA0kAMEYCIQC6Yim3bERFkQSXw2dO06K31ojFQw5Jf+SomCPXQjcu4QIhAIW+jPETlQXYt8lDbOC6nb/0m3gSbm8R3vn3bZTqX9Ir";
    const TEST_X509_CRL_EMPTY_DER_B64: &str = "MIHdMIGFAgEBMAoGCCqGSM49BAMCMCIxIDAeBgNVBAMMF0lyb2hhIElTTyBDUkwgVGVzdCBSb290Fw0yNjA2MDEwMDAwMDBaGA8yMTI2MDUwODAwMDAwMFqgMDAuMB8GA1UdIwQYMBaAFAfSITIYh7s/HFXEuWgTWUbY7FPYMAsGA1UdFAQEAgIQADAKBggqhkjOPQQDAgNHADBEAiA/m/8+c7k15xNLpd2sfai08GTXDRVzbGJiRGvBeRxACQIgBOyF2PNzol2If3Vg4UgVkf/5COVXoDvkLlUelTaj62Y=";
    const TEST_X509_CRL_REVOKED_DER_B64: &str = "MIIBBDCBqgIBATAKBggqhkjOPQQDAjAiMSAwHgYDVQQDDBdJcm9oYSBJU08gQ1JMIFRlc3QgUm9vdBcNMjYwNjAxMDAwMDAwWhgPMjEyNjA1MDgwMDAwMDBaMCMwIQICEAAXDTI2MDYwMTEzMTY1OFowDDAKBgNVHRUEAwoBAaAwMC4wHwYDVR0jBBgwFoAUB9IhMhiHuz8cVcS5aBNZRtjsU9gwCwYDVR0UBAQCAhACMAoGCCqGSM49BAMCA0kAMEYCIQC7LiVi/w7PlmRroPXn4kBMQ2Ep7nTFJ6t35H8wb2abGAIhAOKFvALdoshlevpu4FPYtj/uTdDolRjW/J8ZknJhQEcg";
    const TEST_X509_CRL_EXPIRED_DER_B64: &str = "MIHbMIGDAgEBMAoGCCqGSM49BAMCMCIxIDAeBgNVBAMMF0lyb2hhIElTTyBDUkwgVGVzdCBSb290Fw0yMDAxMDEwMDAwMDBaFw0yMDAxMDIwMDAwMDBaoDAwLjAfBgNVHSMEGDAWgBQH0iEyGIe7PxxVxLloE1lG2OxT2DALBgNVHRQEBAICEAEwCgYIKoZIzj0EAwIDRwAwRAIgOgcF6ZCKDyVWfidxODibs3k6iUMctmh2OEGdUPZ0B6ECIEqgqdL/2jFlr6Z9em1M8QMxr03lpE9UdO+viTFWf/3Z";
    const TEST_X509_CRL_OTHER_ISSUER_DER_B64: &str = "MIHgMIGGAgEBMAoGCCqGSM49BAMCMCMxITAfBgNVBAMMGElyb2hhIElTTyBDUkwgT3RoZXIgUm9vdBcNMjYwNjAxMDAwMDAwWhgPMjEyNjA1MDgwMDAwMDBaoDAwLjAfBgNVHSMEGDAWgBTl4dhk5NXuYpKTUaMkMpK/Z4BA2TALBgNVHRQEBAICIAAwCgYIKoZIzj0EAwIDSQAwRgIhAMTqzMxhaAlCO/l+dcrm+UCIQ1JiPqoOBugTIVaCM/pRAiEA8evRaUSJ6ixL+lyNSDt/4dnii8NZOKFTzDe8hYxDQBM=";
    const TEST_X509_NAME_CONSTRAINTS_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgbOLWcsasQZL6fbjY5AJAEf4V54T/tVnjaA21ui71Q1+hRANCAAQFuA9elo/hc8MHsTJv/iLY/uJQ23YVh0wQM4s++iXnmrEfkHpNV3w2tGUoDbztQC6AxpM0V6ZNGgdDII+cFl88";
    const TEST_X509_NAME_CONSTRAINTS_ALLOWED_LEAF_CERTIFICATE_DER_B64: &str = "MIIBwzCCAWmgAwIBAgIULYzUNb+BDIiGxVl4bJ1EEd3bDfwwCgYIKoZIzj0EAwIwITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgUm9vdDAgFw0yNjA2MDExMzUwNTZaGA8yMTI2MDUwODEzNTA1NlowITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgTGVhZjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABAW4D16Wj+FzwwexMm/+Itj+4lDbdhWHTBAziz76JeeasR+Qek1XfDa0ZSgNvO1ALoDGkzRXpk0aB0Mgj5wWXzyjfTB7MAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMB0GA1UdDgQWBBTdZE7Rq40abeZVmD8GVvGmUSyeEzAfBgNVHSMEGDAWgBT9eipz/v6UBltMJtYpHCUGdBPJrjAbBgNVHREEFDASghBzaWduZXIuYmFuay50ZXN0MAoGCCqGSM49BAMCA0gAMEUCIQCKJidOlBwkTttdoh4Xl7Q2Xf3Av6dSTpn2VTcxEFc/QAIgShrDnNKILYLcMSLtjNjLci6jWDzcYGqNWiiM6THC59U=";
    const TEST_X509_NAME_CONSTRAINTS_ROOT_CERTIFICATE_DER_B64: &str = "MIIB5TCCAYqgAwIBAgIUPqN14bWcN646xRoOnH66kHDQXTMwCgYIKoZIzj0EAwIwITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgUm9vdDAgFw0yNjA2MDExMzUwNTZaGA8yMTI2MDUwODEzNTA1NlowITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgUm9vdDBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABMy3R6Uv13TYux3PLQDyOyNbTSwCt4q2ti20XJELV5v+y9Pu3clkQM9YyuYJYdtubTIx5tYde8tuQp6gL2ygSECjgZ0wgZowEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFP16KnP+/pQGW0wm1ikcJQZ0E8muMB8GA1UdIwQYMBaAFP16KnP+/pQGW0wm1ikcJQZ0E8muMDQGA1UdHgEB/wQqMCigDjAMggouYmFuay50ZXN0oRYwFIISLmJsb2NrZWQuYmFuay50ZXN0MAoGCCqGSM49BAMCA0kAMEYCIQD6JqC+JihQ7z7DK3Xv5rj++4ZKO2NTDMPfac0ITWR36QIhAOVHAJPEcxcsl4aiMEIr7x65CwEi74PIPHRZskv23hiO";
    const TEST_X509_NAME_CONSTRAINTS_OUTSIDE_LEAF_CERTIFICATE_DER_B64: &str = "MIIBxjCCAW2gAwIBAgIULYzUNb+BDIiGxVl4bJ1EEd3bDf0wCgYIKoZIzj0EAwIwITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgUm9vdDAgFw0yNjA2MDExMzUwNTZaGA8yMTI2MDUwODEzNTA1NlowITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgTGVhZjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABAW4D16Wj+FzwwexMm/+Itj+4lDbdhWHTBAziz76JeeasR+Qek1XfDa0ZSgNvO1ALoDGkzRXpk0aB0Mgj5wWXzyjgYAwfjAMBgNVHRMBAf8EAjAAMA4GA1UdDwEB/wQEAwIHgDAdBgNVHQ4EFgQU3WRO0auNGm3mVZg/BlbxplEsnhMwHwYDVR0jBBgwFoAU/Xoqc/7+lAZbTCbWKRwlBnQTya4wHgYDVR0RBBcwFYITc2lnbmVyLmV4YW1wbGUudGVzdDAKBggqhkjOPQQDAgNHADBEAiAjNtnk4lfeqsqjExCVTfrtqlu0RlU/Dn0LDqqoOSh/ogIgcPprmjBwegQXpjksGPVCP/TLkNi3esvozSmkH/pAzvo=";
    const TEST_X509_NAME_CONSTRAINTS_EXCLUDED_LEAF_CERTIFICATE_DER_B64: &str = "MIIBzDCCAXKgAwIBAgIULYzUNb+BDIiGxVl4bJ1EEd3bDf4wCgYIKoZIzj0EAwIwITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgUm9vdDAgFw0yNjA2MDExMzUwNTZaGA8yMTI2MDUwODEzNTA1NlowITEfMB0GA1UEAwwWSXJvaGEgSVNPIE5DIFRlc3QgTGVhZjBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABAW4D16Wj+FzwwexMm/+Itj+4lDbdhWHTBAziz76JeeasR+Qek1XfDa0ZSgNvO1ALoDGkzRXpk0aB0Mgj5wWXzyjgYUwgYIwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwHQYDVR0OBBYEFN1kTtGrjRpt5lWYPwZW8aZRLJ4TMB8GA1UdIwQYMBaAFP16KnP+/pQGW0wm1ikcJQZ0E8muMCIGA1UdEQQbMBmCF3BheWVlLmJsb2NrZWQuYmFuay50ZXN0MAoGCCqGSM49BAMCA0gAMEUCIQCk5NPOJkXsvdwWCQ9NIYaS37JlivB/8dTHjv/HUA4fUQIgb65DjzIAyQdveqDve8AT0AW+iZwY8dcqyExZVfZx5bQ=";
    const TEST_X509_OCSP_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgOzTGZkOgS1T9pB1fWVC+0w+Prc/yFCID0klpj9gC5wyhRANCAAShay14G5YTdNi3gcQ1HU57Tqq/djk9NjZUUB188pQKBl28Re1rPtOGcqnp/cQ/fXJ4wCZL+VZIZLBXPnWI+ycM";
    const TEST_X509_OCSP_LEAF_CERTIFICATE_DER_B64: &str = "MIIB3zCCAYWgAwIBAgIUVzuYJYNzXMyTPXW0KohPO/1jeMAwCgYIKoZIzj0EAwIwIzEhMB8GA1UEAwwYSXJvaGEgSVNPIE9DU1AgVGVzdCBSb290MCAXDTI2MDYwMTE0MTYwNloYDzIxMjYwNTA4MTQxNjA2WjAjMSEwHwYDVQQDDBhJcm9oYSBJU08gT0NTUCBUZXN0IExlYWYwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAShay14G5YTdNi3gcQ1HU57Tqq/djk9NjZUUB188pQKBl28Re1rPtOGcqnp/cQ/fXJ4wCZL+VZIZLBXPnWI+ycMo4GUMIGRMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgeAMB0GA1UdDgQWBBRP6VxpI7GgZ0OiFNO9GbKev40fszAfBgNVHSMEGDAWgBTGJreH2IBwn1DyPNeDcsR5YP9ifTAxBggrBgEFBQcBAQQlMCMwIQYIKwYBBQUHMAGGFWh0dHA6Ly9vY3NwLmJhbmsudGVzdDAKBggqhkjOPQQDAgNIADBFAiAWRjZFwlw986e0nBGrWO646+QR1+kYlnWG4pJLXC5i7QIhAJ52kUkChDHZgwU8+y9YQ+8nn3qHbFn7elYJ0FlC9dyD";
    const TEST_X509_OCSP_ROOT_CERTIFICATE_DER_B64: &str = "MIIBsDCCAVagAwIBAgIUAJAyggtpTfF3RPeQ3TydTPhQeiMwCgYIKoZIzj0EAwIwIzEhMB8GA1UEAwwYSXJvaGEgSVNPIE9DU1AgVGVzdCBSb290MCAXDTI2MDYwMTE0MTYwNloYDzIxMjYwNTA4MTQxNjA2WjAjMSEwHwYDVQQDDBhJcm9oYSBJU08gT0NTUCBUZXN0IFJvb3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAASGfoQ2ey3oZKXuPvsT+e+lrzJHfvlYp5zyTd1Nvdwd0TzLIziFEdnVB22ADa6GobI1NvlbGp9q51qezbMImvx6o2YwZDASBgNVHRMBAf8ECDAGAQH/AgEBMA4GA1UdDwEB/wQEAwIBhjAdBgNVHQ4EFgQUxia3h9iAcJ9Q8jzXg3LEeWD/Yn0wHwYDVR0jBBgwFoAUxia3h9iAcJ9Q8jzXg3LEeWD/Yn0wCgYIKoZIzj0EAwIDSAAwRQIhAP0oYY1kvA31qfhBVo97pTyV3nRW+UKK/pLOg79IvOYJAiAYlhB9BkARGqNxO60RfPOCwd3Z5YLMMUY/qIhu/VK1Aw==";
    const TEST_X509_OCSP_GOOD_RESPONSE_DER_B64: &str = "MIIBbAoBAKCCAWUwggFhBgkrBgEFBQcwAQEEggFSMIIBTjCB9KElMCMxITAfBgNVBAMMGElyb2hhIElTTyBPQ1NQIFRlc3QgUm9vdBgPMjAyNjA2MDExNDE2MDZaMIGUMIGRMGkwDQYJYIZIAWUDBAIBBQAEIOKw7yAGJupJX6XKZEMFbrvYLi40IxU/Id/RvO6sDO5aBCAqmLjm6DUnDfjI4vbf7mfWINfEFWta/zrWYVHyyQn4jQIUVzuYJYNzXMyTPXW0KohPO/1jeMCAABgPMjAyNjA2MDExNDE2MDZaoBEYDzIxMjYwNTA4MTQxNjA2WqEjMCEwHwYJKwYBBQUHMAECBBIEECHLWkORHnikxmVlHHc7YawwCgYIKoZIzj0EAwIDSQAwRgIhAKzvPh2bC3v/udrO0riVUkmc0OLpbJpl/UYdWY1wOeSyAiEAyj2Zl7T15Y5wZsUSIcBoGPCCVhJDol1a2YIbSF5d2HI=";
    const TEST_X509_OCSP_REVOKED_RESPONSE_DER_B64: &str = "MIIBfAoBAKCCAXUwggFxBgkrBgEFBQcwAQEEggFiMIIBXjCCAQWhJTAjMSEwHwYDVQQDDBhJcm9oYSBJU08gT0NTUCBUZXN0IFJvb3QYDzIwMjYwNjAxMTQxNjA2WjCBpTCBojBpMA0GCWCGSAFlAwQCAQUABCDisO8gBibqSV+lymRDBW672C4uNCMVPyHf0bzurAzuWgQgKpi45ug1Jw34yOL23+5n1iDXxBVrWv861mFR8skJ+I0CFFc7mCWDc1zMkz11tCqITzv9Y3jAoREYDzIwMjYwNjAxMDAwMDAwWhgPMjAyNjA2MDExNDE2MDZaoBEYDzIxMjYwNTA4MTQxNjA2WqEjMCEwHwYJKwYBBQUHMAECBBIEECHLWkORHnikxmVlHHc7YawwCgYIKoZIzj0EAwIDRwAwRAIgSyzZpk3IKV0xaXhD738PO3pV3sBVQPT23d7q2Pseb6gCIDoSy7PNbPsEuptSWFozu2/mU2BQF/R+YtOowEYZ/b1A";
    const TEST_X509_OCSP_DELEGATED_LEAF_SIGNING_KEY_PKCS8_DER_B64: &str = "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgWktoWLMVwgzz3ln+T2mhunKn5/W+8T4dd8FesU1J11ChRANCAATlaMh389EpRfZ7tG+Q/z5X5jik4Oxn8Lup6dqkdiX6arE4AhtqinWmUOx5ra4+oHtoBF8VH58fh3utTwzrSZ8N";
    const TEST_X509_OCSP_DELEGATED_LEAF_CERTIFICATE_DER_B64: &str = "MIIB6TCCAY+gAwIBAgIULsNyLg2GZvJsuxu0Z4E1+rjg/wUwCgYIKoZIzj0EAwIwKDEmMCQGA1UEAwwdSXJvaGEgSVNPIE9DU1AgRGVsZWdhdGVkIFJvb3QwIBcNMjYwNjAxMTQzNjI3WhgPMjEyNjA1MDgxNDM2MjdaMCgxJjAkBgNVBAMMHUlyb2hhIElTTyBPQ1NQIERlbGVnYXRlZCBMZWFmMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE5WjId/PRKUX2e7RvkP8+V+Y4pODsZ/C7qenapHYl+mqxOAIbaop1plDsea2uPqB7aARfFR+fH4d7rU8M60mfDaOBlDCBkTAMBgNVHRMBAf8EAjAAMA4GA1UdDwEB/wQEAwIHgDAdBgNVHQ4EFgQUN9imSEFE/c1q4u3hrErIroqYwxAwHwYDVR0jBBgwFoAURNE5Qhb1JX7BNYubJowFAJ0h7l4wMQYIKwYBBQUHAQEEJTAjMCEGCCsGAQUFBzABhhVodHRwOi8vb2NzcC5iYW5rLnRlc3QwCgYIKoZIzj0EAwIDSAAwRQIgUXkd20bjVSgi5GkYWI3Ykem25+IC7cQ1LvATVO9i06ACIQD3QpgfocKpDqnoc9H58RDLSMaqjnW9MAsKbcHNs4ZrNw==";
    const TEST_X509_OCSP_DELEGATED_ROOT_CERTIFICATE_DER_B64: &str = "MIIBuzCCAWCgAwIBAgIUUMvPwuhsVdnJyn9ENoMZSLHctPEwCgYIKoZIzj0EAwIwKDEmMCQGA1UEAwwdSXJvaGEgSVNPIE9DU1AgRGVsZWdhdGVkIFJvb3QwIBcNMjYwNjAxMTQzNjI3WhgPMjEyNjA1MDgxNDM2MjdaMCgxJjAkBgNVBAMMHUlyb2hhIElTTyBPQ1NQIERlbGVnYXRlZCBSb290MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEvftG/vk1C8ioQjTeL0Nr5fDqrQs4DyBB7JVVEf7RoTjgEJojUCOT1TXV42KDPF4Ou6t/m3GHJb7FnbqE8LGxwqNmMGQwEgYDVR0TAQH/BAgwBgEB/wIBATAOBgNVHQ8BAf8EBAMCAYYwHQYDVR0OBBYEFETROUIW9SV+wTWLmyaMBQCdIe5eMB8GA1UdIwQYMBaAFETROUIW9SV+wTWLmyaMBQCdIe5eMAoGCCqGSM49BAMCA0kAMEYCIQDIkyHlFpZC6XaESe0K+84GJb0k+I4LRpWxfmCV0cjfdAIhALPdEDSnKxJOXugAqWzj7LXBm63ISwzFQkZm3Olpc+zN";
    const TEST_X509_OCSP_DELEGATED_GOOD_RESPONSE_DER_B64: &str = "MIIDUwoBAKCCA0wwggNIBgkrBgEFBQcwAQEEggM5MIIDNTCB/qEvMC0xKzApBgNVBAMMIklyb2hhIElTTyBPQ1NQIERlbGVnYXRlZCBSZXNwb25kZXIYDzIwMjYwNjAxMTQzNjI3WjCBlDCBkTBpMA0GCWCGSAFlAwQCAQUABCCdvQl6x4ExlidXQVvJaOO18NCxQamAPKLpdT3RYW2aZwQgOR36gtHUOgMjPcOprp5f1+pmvdtIaCE5GBV0FmFzSU8CFC7Dci4NhmbybLsbtGeBNfq44P8FgAAYDzIwMjYwNjAxMTQzNjI3WqARGA8yMTI2MDUwODE0MzYyN1qhIzAhMB8GCSsGAQUFBzABAgQSBBC7tDgAD3HvEmvGnxZumAhBMAoGCCqGSM49BAMCA0gAMEUCIFUNO3cufftkA53FMOw9jL55Pn9DYhk2sY/3ZqHxByVwAiEA7ppZRWYFtpqWGsd26/alBWQrvJNNXhIiL2TDb+Z5C7CgggHaMIIB1jCCAdIwggF3oAMCAQICFC7Dci4NhmbybLsbtGeBNfq44P8GMAoGCCqGSM49BAMCMCgxJjAkBgNVBAMMHUlyb2hhIElTTyBPQ1NQIERlbGVnYXRlZCBSb290MCAXDTI2MDYwMTE0MzYyN1oYDzIxMjYwNTA4MTQzNjI3WjAtMSswKQYDVQQDDCJJcm9oYSBJU08gT0NTUCBEZWxlZ2F0ZWQgUmVzcG9uZGVyMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEGTD2ooQHAbEE/MDCFGMkyP49kdn13dUG9OqfuQJT5WGUaElh+XRiebSgzrsJT4UZJ5CROd4RjJ5L35cspDr5KqN4MHYwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCB4AwFgYDVR0lAQH/BAwwCgYIKwYBBQUHAwkwHQYDVR0OBBYEFCV3N7muRz5ulCe3ltUOfKcWwsRDMB8GA1UdIwQYMBaAFETROUIW9SV+wTWLmyaMBQCdIe5eMAoGCCqGSM49BAMCA0kAMEYCIQCzL+5oyJ5K2V2JvwqqOzT0OLFM5bpcfQzcE+4IMb+DZwIhAMmwczkcst9vLrwTjqZFAoNu0+GSnwnRWwpGJRSENSFq";
    const TEST_X509_OCSP_DELEGATED_GOOD_NO_CERTS_RESPONSE_DER_B64: &str = "MIIBdQoBAKCCAW4wggFqBgkrBgEFBQcwAQEEggFbMIIBVzCB/qEvMC0xKzApBgNVBAMMIklyb2hhIElTTyBPQ1NQIERlbGVnYXRlZCBSZXNwb25kZXIYDzIwMjYwNjAxMTQzNjI3WjCBlDCBkTBpMA0GCWCGSAFlAwQCAQUABCCdvQl6x4ExlidXQVvJaOO18NCxQamAPKLpdT3RYW2aZwQgOR36gtHUOgMjPcOprp5f1+pmvdtIaCE5GBV0FmFzSU8CFC7Dci4NhmbybLsbtGeBNfq44P8FgAAYDzIwMjYwNjAxMTQzNjI3WqARGA8yMTI2MDUwODE0MzYyN1qhIzAhMB8GCSsGAQUFBzABAgQSBBC7tDgAD3HvEmvGnxZumAhBMAoGCCqGSM49BAMCA0gAMEUCIC3xUMoiVdy4ENrVP6PHecQfp4PsrBxPercEvmEcpV0rAiEAjIe5JEkGyBNjY+VhKHCAdPvnKeLGbt6KKnLNJ3wChuQ=";

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
            signature_public_key_sha256_pins: Vec::new(),
            x509_trust_anchor_sha256_pins: Vec::new(),
            x509_required_certificate_policy_oids: Vec::new(),
            x509_require_crl_revocation_check: false,
            x509_crl_der_base64: Vec::new(),
            x509_require_ocsp_revocation_check: false,
            x509_ocsp_response_der_base64: Vec::new(),
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
            signature_public_key_sha256_pins: vec![test_p256_public_key_pin()],
            x509_trust_anchor_sha256_pins: Vec::new(),
            x509_required_certificate_policy_oids: Vec::new(),
            x509_require_crl_revocation_check: false,
            x509_crl_der_base64: Vec::new(),
            x509_require_ocsp_revocation_check: false,
            x509_ocsp_response_der_base64: Vec::new(),
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

    fn test_p256_signing_key() -> P256SigningKey {
        P256SigningKey::from_bytes(&[0x31; 32].into()).expect("deterministic P-256 key")
    }

    fn test_p256_public_key_pin() -> String {
        let public_key = test_p256_signing_key()
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .to_vec();
        sha256_hex(&public_key)
    }

    fn test_x509_public_key_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_CHAIN_LEAF_CERTIFICATE_DER_B64)
            .expect("X.509 fixture must decode");
        let (_, certificate) =
            X509Certificate::from_der(&certificate).expect("X.509 fixture must parse");
        let public_key = certificate.public_key().subject_public_key.data.to_vec();
        sha256_hex(&public_key)
    }

    fn test_x509_chain_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_CHAIN_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("chain leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("chain leaf PKCS#8 fixture must parse")
    }

    fn test_x509_chain_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_CHAIN_ROOT_CERTIFICATE_DER_B64)
            .expect("chain root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_unknown_critical_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_UNKNOWN_CRITICAL_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("unknown-critical leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8)
            .expect("unknown-critical leaf PKCS#8 fixture must parse")
    }

    fn test_x509_unknown_critical_leaf_public_key_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_UNKNOWN_CRITICAL_LEAF_CERTIFICATE_DER_B64)
            .expect("unknown-critical leaf X.509 fixture must decode");
        let (_, certificate) =
            X509Certificate::from_der(&certificate).expect("unknown-critical leaf must parse");
        let public_key = certificate.public_key().subject_public_key.data.to_vec();
        sha256_hex(&public_key)
    }

    fn test_x509_unknown_critical_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_UNKNOWN_CRITICAL_ROOT_CERTIFICATE_DER_B64)
            .expect("unknown-critical root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_unknown_critical_bad_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_UNKNOWN_CRITICAL_BAD_ROOT_CERTIFICATE_DER_B64)
            .expect("unknown-critical bad root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_expired_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_EXPIRED_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("expired leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("expired leaf PKCS#8 fixture must parse")
    }

    fn test_x509_expired_leaf_public_key_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_EXPIRED_LEAF_CERTIFICATE_DER_B64)
            .expect("expired leaf X.509 fixture must decode");
        let (_, certificate) =
            X509Certificate::from_der(&certificate).expect("expired leaf X.509 fixture must parse");
        let public_key = certificate.public_key().subject_public_key.data.to_vec();
        sha256_hex(&public_key)
    }

    fn test_x509_expired_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_EXPIRED_ROOT_CERTIFICATE_DER_B64)
            .expect("expired-leaf root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_pathlen_root0_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_PATHLEN_ROOT0_CERTIFICATE_DER_B64)
            .expect("pathLen root0 X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_pathlen_root1_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_PATHLEN_ROOT1_CERTIFICATE_DER_B64)
            .expect("pathLen root1 X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_ca_leaf_public_key_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_CA_LEAF_CERTIFICATE_DER_B64)
            .expect("CA leaf X.509 fixture must decode");
        let (_, certificate) =
            X509Certificate::from_der(&certificate).expect("CA leaf X.509 fixture must parse");
        let public_key = certificate.public_key().subject_public_key.data.to_vec();
        sha256_hex(&public_key)
    }

    fn test_x509_ca_leaf_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_CA_LEAF_ROOT_CERTIFICATE_DER_B64)
            .expect("CA leaf root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_leaf_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_CHAIN_LEAF_CERTIFICATE_DER_B64)
            .expect("chain leaf X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_policy_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_POLICY_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("policy leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("policy leaf PKCS#8 fixture must parse")
    }

    fn test_x509_pathlen_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_PATHLEN_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("pathLen leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("pathLen leaf PKCS#8 fixture must parse")
    }

    fn test_x509_ca_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_CA_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("CA leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("CA leaf PKCS#8 fixture must parse")
    }

    fn test_x509_policy_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_POLICY_ROOT_CERTIFICATE_DER_B64)
            .expect("policy root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_crl_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_CRL_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("CRL leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("CRL leaf PKCS#8 fixture must parse")
    }

    fn test_x509_crl_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_CRL_ROOT_CERTIFICATE_DER_B64)
            .expect("CRL root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_name_constraints_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_NAME_CONSTRAINTS_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("name-constrained leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8)
            .expect("name-constrained leaf PKCS#8 fixture must parse")
    }

    fn test_x509_name_constraints_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_NAME_CONSTRAINTS_ROOT_CERTIFICATE_DER_B64)
            .expect("name-constrained root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_ocsp_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_OCSP_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("OCSP leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8).expect("OCSP leaf PKCS#8 fixture must parse")
    }

    fn test_x509_ocsp_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_OCSP_ROOT_CERTIFICATE_DER_B64)
            .expect("OCSP root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn test_x509_ocsp_delegated_leaf_signing_key() -> P256SigningKey {
        let pkcs8 = BASE64_STANDARD
            .decode(TEST_X509_OCSP_DELEGATED_LEAF_SIGNING_KEY_PKCS8_DER_B64)
            .expect("delegated OCSP leaf PKCS#8 fixture must decode");
        P256SigningKey::from_pkcs8_der(&pkcs8)
            .expect("delegated OCSP leaf PKCS#8 fixture must parse")
    }

    fn test_x509_ocsp_delegated_root_certificate_pin() -> String {
        let certificate = BASE64_STANDARD
            .decode(TEST_X509_OCSP_DELEGATED_ROOT_CERTIFICATE_DER_B64)
            .expect("delegated OCSP root X.509 fixture must decode");
        sha256_hex(&certificate)
    }

    fn signed_pacs008_xml() -> String {
        let signing_key = test_p256_signing_key();
        let public_key = BASE64_STANDARD.encode(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><KeyValue><ECKeyValue>",
                    r#"<NamedCurve URI="urn:oid:1.2.840.10045.3.1.7"/>"#,
                    "<PublicKey>{public_key}</PublicKey>",
                    "</ECKeyValue></KeyValue></KeyInfo>"
                ),
                public_key = public_key
            ),
        )
    }

    fn signed_pacs008_xml_with_x509_certificate() -> String {
        let signing_key = test_x509_chain_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                "<KeyInfo><X509Data><X509Certificate>{TEST_X509_CHAIN_LEAF_CERTIFICATE_DER_B64}</X509Certificate></X509Data></KeyInfo>"
            ),
        )
    }

    fn signed_pacs008_xml_with_x509_certificate_chain() -> String {
        let signing_key = test_x509_chain_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_CHAIN_LEAF_CERTIFICATE_DER_B64,
                root = TEST_X509_CHAIN_ROOT_CERTIFICATE_DER_B64
            ),
        )
    }

    fn signed_pacs008_xml_with_unknown_critical_leaf_x509_certificate_chain(
        include_root: bool,
    ) -> String {
        let signing_key = test_x509_unknown_critical_leaf_signing_key();
        let root_xml = if include_root {
            format!(
                "<X509Certificate>{TEST_X509_UNKNOWN_CRITICAL_ROOT_CERTIFICATE_DER_B64}</X509Certificate>"
            )
        } else {
            String::new()
        };
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "{root_xml}",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_UNKNOWN_CRITICAL_LEAF_CERTIFICATE_DER_B64,
                root_xml = root_xml
            ),
        )
    }

    fn signed_pacs008_xml_with_unknown_critical_root_x509_certificate_chain() -> String {
        let signing_key = test_x509_unknown_critical_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_UNKNOWN_CRITICAL_ROOT_LEAF_CERTIFICATE_DER_B64,
                root = TEST_X509_UNKNOWN_CRITICAL_BAD_ROOT_CERTIFICATE_DER_B64
            ),
        )
    }

    fn signed_pacs008_xml_with_expired_x509_certificate_chain(include_root: bool) -> String {
        let signing_key = test_x509_expired_leaf_signing_key();
        let root_xml = if include_root {
            format!(
                "<X509Certificate>{TEST_X509_EXPIRED_ROOT_CERTIFICATE_DER_B64}</X509Certificate>"
            )
        } else {
            String::new()
        };
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "{root_xml}",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_EXPIRED_LEAF_CERTIFICATE_DER_B64,
                root_xml = root_xml
            ),
        )
    }

    fn signed_pacs008_xml_with_pathlen_x509_certificate_chain(
        intermediate: &str,
        root: &str,
    ) -> String {
        let signing_key = test_x509_pathlen_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{intermediate}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_PATHLEN_LEAF_CERTIFICATE_DER_B64,
                intermediate = intermediate,
                root = root
            ),
        )
    }

    fn signed_pacs008_xml_with_ca_leaf_x509_certificate_chain(include_root: bool) -> String {
        let signing_key = test_x509_ca_leaf_signing_key();
        let root_xml = if include_root {
            format!(
                "<X509Certificate>{TEST_X509_CA_LEAF_ROOT_CERTIFICATE_DER_B64}</X509Certificate>"
            )
        } else {
            String::new()
        };
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "{root_xml}",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_CA_LEAF_CERTIFICATE_DER_B64,
                root_xml = root_xml
            ),
        )
    }

    fn signed_pacs008_xml_with_leaf_x509_certificate() -> String {
        let signing_key = test_x509_chain_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                "<KeyInfo><X509Data><X509Certificate>{TEST_X509_CHAIN_LEAF_CERTIFICATE_DER_B64}</X509Certificate></X509Data></KeyInfo>"
            ),
        )
    }

    fn signed_pacs008_xml_with_policy_x509_certificate_chain() -> String {
        let signing_key = test_x509_policy_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_POLICY_LEAF_CERTIFICATE_DER_B64,
                root = TEST_X509_POLICY_ROOT_CERTIFICATE_DER_B64
            ),
        )
    }

    fn signed_pacs008_xml_with_crl_x509_certificate_chain(embedded_crl: Option<&str>) -> String {
        let signing_key = test_x509_crl_leaf_signing_key();
        let crl_xml = embedded_crl
            .map(|crl| format!("<X509CRL>{crl}</X509CRL>"))
            .unwrap_or_default();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "{crl_xml}",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_CRL_LEAF_CERTIFICATE_DER_B64,
                root = TEST_X509_CRL_ROOT_CERTIFICATE_DER_B64,
                crl_xml = crl_xml
            ),
        )
    }

    fn signed_pacs008_xml_with_name_constrained_x509_certificate_chain(leaf: &str) -> String {
        let signing_key = test_x509_name_constraints_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "</X509Data></KeyInfo>"
                ),
                leaf = leaf,
                root = TEST_X509_NAME_CONSTRAINTS_ROOT_CERTIFICATE_DER_B64
            ),
        )
    }

    fn signed_pacs008_xml_with_ocsp_x509_certificate_chain(embedded_ocsp: Option<&str>) -> String {
        let signing_key = test_x509_ocsp_leaf_signing_key();
        let ocsp_xml = embedded_ocsp
            .map(|ocsp| format!("<EncapsulatedOCSPValue>{ocsp}</EncapsulatedOCSPValue>"))
            .unwrap_or_default();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "{ocsp_xml}",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_OCSP_LEAF_CERTIFICATE_DER_B64,
                root = TEST_X509_OCSP_ROOT_CERTIFICATE_DER_B64,
                ocsp_xml = ocsp_xml
            ),
        )
    }

    fn signed_pacs008_xml_with_delegated_ocsp_x509_certificate_chain() -> String {
        let signing_key = test_x509_ocsp_delegated_leaf_signing_key();
        signed_pacs008_xml_with_key_info(
            &signing_key,
            &format!(
                concat!(
                    "<KeyInfo><X509Data>",
                    "<X509Certificate>{leaf}</X509Certificate>",
                    "<X509Certificate>{root}</X509Certificate>",
                    "</X509Data></KeyInfo>"
                ),
                leaf = TEST_X509_OCSP_DELEGATED_LEAF_CERTIFICATE_DER_B64,
                root = TEST_X509_OCSP_DELEGATED_ROOT_CERTIFICATE_DER_B64
            ),
        )
    }

    fn signed_pacs008_xml_with_key_info(signing_key: &P256SigningKey, key_info: &str) -> String {
        let unsigned = unsigned_pacs008_xml();
        let insertion = unsigned
            .find("</FIToFICstmrCdtTrf>")
            .expect("signature insertion point");
        let digest = BASE64_STANDARD.encode(Sha256::digest(unsigned.as_bytes()));
        let signed_info = format!(
            r#"<SignedInfo><CanonicalizationMethod Algorithm="{XML_C14N_1_0}"/><SignatureMethod Algorithm="{XMLDSIG_ECDSA_SHA256}"/><Reference URI=""><Transforms><Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"/></Transforms><DigestMethod Algorithm="{XMLDSIG_SHA256}"/><DigestValue>{digest}</DigestValue></Reference></SignedInfo>"#
        );
        let signature: P256Signature = signing_key.sign(signed_info.as_bytes());
        let signature_value = BASE64_STANDARD.encode(signature.to_der().as_bytes());
        let signature_xml = format!(
            concat!(
                "<Signature>{signed_info}<SignatureValue>{signature_value}</SignatureValue>",
                "{key_info}",
                r##"<Object><QualifyingProperties Target="#sig-001"/></Object>"##,
                "</Signature>"
            ),
            signed_info = signed_info,
            signature_value = signature_value,
            key_info = key_info
        );
        format!(
            "{}{}{}",
            &unsigned[..insertion],
            signature_xml,
            &unsigned[insertion..]
        )
    }

    fn swift_pacs008_xml(business_message_id: &str, uetr: &str) -> String {
        format!(
            r#"<DataPDU>
  <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
    <BizMsgIdr>{business_message_id}</BizMsgIdr>
    <MsgDefIdr>pacs.008.001.08</MsgDefIdr>
    <BizSvc>swift.cbprplus.02</BizSvc>
    <CreDt>2025-01-01T12:00:00Z</CreDt>
  </AppHdr>
  <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
    <FIToFICstmrCdtTrf>
      <GrpHdr><MsgId>{business_message_id}-grp</MsgId></GrpHdr>
      <CdtTrfTxInf>
        <PmtId><UETR>{uetr}</UETR></PmtId>
        <IntrBkSttlmAmt Ccy="USD">10.00</IntrBkSttlmAmt>
        <IntrBkSttlmDt>2024-01-01</IntrBkSttlmDt>
        <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
        <CdtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></CdtrAcct>
        <DbtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></DbtrAgt>
        <CdtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></CdtrAgt>
      </CdtTrfTxInf>
    </FIToFICstmrCdtTrf>
  </Document>
</DataPDU>"#
        )
    }

    fn live_pacs008_xml(
        business_message_id: &str,
        msg_def_id: &str,
        business_service: &str,
        currency: &str,
        amount: &str,
        uetr: &str,
    ) -> String {
        format!(
            r#"<DataPDU>
  <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
    <BizMsgIdr>{business_message_id}</BizMsgIdr>
    <MsgDefIdr>{msg_def_id}</MsgDefIdr>
    <BizSvc>{business_service}</BizSvc>
    <CreDt>2025-01-01T12:00:00Z</CreDt>
  </AppHdr>
  <Document xmlns="urn:iso:std:iso:20022:tech:xsd:{msg_def_id}">
    <FIToFICstmrCdtTrf>
      <GrpHdr><MsgId>{business_message_id}-grp</MsgId></GrpHdr>
      <CdtTrfTxInf>
        <PmtId><UETR>{uetr}</UETR></PmtId>
        <IntrBkSttlmAmt Ccy="{currency}">{amount}</IntrBkSttlmAmt>
        <IntrBkSttlmDt>2024-01-01</IntrBkSttlmDt>
        <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
        <CdtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></CdtrAcct>
        <DbtrAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></DbtrAgt>
        <CdtrAgt><FinInstnId><BICFI>MARKDEFF</BICFI></FinInstnId></CdtrAgt>
      </CdtTrfTxInf>
    </FIToFICstmrCdtTrf>
  </Document>
</DataPDU>"#
        )
    }

    fn xsd_tag_with_attr<'a>(
        xsd: &'a str,
        tag_name: &str,
        attr_name: &str,
        expected_value: &str,
    ) -> Option<&'a str> {
        let pattern = format!("<{tag_name}");
        let mut rest = xsd;
        while let Some(offset) = rest.find(&pattern) {
            let candidate = &rest[offset..];
            let end = candidate.find('>')?;
            let tag = &candidate[..=end];
            if attr_value(tag, attr_name).as_deref() == Some(expected_value) {
                return Some(tag);
            }
            rest = candidate.get(end + 1..)?;
        }
        None
    }

    fn xsd_schema_target_namespace(xsd: &str) -> Option<String> {
        let schema_start = xsd.find("<xs:schema")?;
        let schema = &xsd[schema_start..];
        let schema_end = schema.find('>')?;
        attr_value(&schema[..=schema_end], "targetNamespace")
    }

    fn xsd_document_payload_root(xsd: &str) -> Option<String> {
        let document_tag = xsd_tag_with_attr(xsd, "xs:element", "name", "Document")?;
        let document_type = attr_value(document_tag, "type")?;
        let document_type_tag = xsd_tag_with_attr(xsd, "xs:complexType", "name", &document_type)?;
        let document_type_start = xsd.find(document_type_tag)?;
        let document_type_body = &xsd[document_type_start + document_type_tag.len()..];
        let sequence_start = document_type_body.find("<xs:sequence")?;
        let sequence = &document_type_body[sequence_start..];
        let element_start = sequence.find("<xs:element")?;
        let element = &sequence[element_start..];
        let element_end = element.find('>')?;
        attr_value(&element[..=element_end], "name")
    }

    fn live_pacs009_xml(
        business_message_id: &str,
        msg_def_id: &str,
        business_service: &str,
    ) -> String {
        format!(
            r#"<DataPDU>
  <AppHdr xmlns="urn:iso:std:iso:20022:tech:xsd:head.001.001.01">
    <BizMsgIdr>{business_message_id}</BizMsgIdr>
    <MsgDefIdr>{msg_def_id}</MsgDefIdr>
    <BizSvc>{business_service}</BizSvc>
    <CreDt>2025-01-01T12:00:00Z</CreDt>
  </AppHdr>
  <Document xmlns="urn:iso:std:iso:20022:tech:xsd:{msg_def_id}">
    <FICdtTrf>
      <GrpHdr>
        <MsgId>{business_message_id}-grp</MsgId>
        <InstgAgt><FinInstnId><BICFI>DEUTDEFF</BICFI></FinInstnId></InstgAgt>
        <InstdAgt><FinInstnId><BICFI>MARKDEFF</BICFI></FinInstnId></InstdAgt>
      </GrpHdr>
      <CdtTrfTxInf>
        <IntrBkSttlmAmt Ccy="USD">2500.00</IntrBkSttlmAmt>
        <IntrBkSttlmDt>2024-01-03</IntrBkSttlmDt>
        <DbtrAcct><Id><IBAN>GB82WEST12345698765432</IBAN></Id></DbtrAcct>
        <CdtrAcct><Id><IBAN>GB33BUKB20201555555555</IBAN></Id></CdtrAcct>
        <Purp><Cd>SECU</Cd></Purp>
      </CdtTrfTxInf>
    </FICdtTrf>
  </Document>
</DataPDU>"#
        )
    }

    fn sample_config_with_live_reference_data() -> (actual::IsoBridge, Vec<NamedTempFile>) {
        let bic_lei = write_snapshot(
            r#"{
                "version":"2024-05-01",
                "source":"GLEIF sample",
                "entries":[
                    {"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"},
                    {"bic":"MARKDEFF","lei":"5493001KJTIIGC8Y1R12"}
                ]
            }"#,
        );
        let isin_crosswalk = write_snapshot(
            r#"{
                "version":"2024-05-01",
                "source":"ANNA DSB sample",
                "entries":[{"isin":"US0378331005","cusip":"037833100"}]
            }"#,
        );
        let mic_directory = write_snapshot(
            r#"{
                "version":"2024-05-01",
                "source":"ISO 10383 sample",
                "entries":[{"mic":"XNAS","status":"ACTIVE"}]
            }"#,
        );
        let mut config = sample_config();
        config.reference_data.bic_lei_path = Some(bic_lei.path().to_path_buf());
        config.reference_data.isin_crosswalk_path = Some(isin_crosswalk.path().to_path_buf());
        config.reference_data.mic_directory_path = Some(mic_directory.path().to_path_buf());
        (config, vec![bic_lei, isin_crosswalk, mic_directory])
    }

    fn inbound_metadata(message_id: &str, message_type: &str) -> IsoMessageMetadata {
        IsoMessageMetadata::inbound(
            "generic-iso20022",
            message_type,
            None,
            Some(format!("{message_id}-biz")),
            None,
            format!("{message_id}-payload-hash"),
            "snapshot".to_owned(),
            false,
        )
    }

    fn record_original(runtime: &Iso20022BridgeRuntime, message_id: &str, message_type: &str) {
        assert!(
            runtime
                .check_and_record_inbound(message_id, inbound_metadata(message_id, message_type)),
            "record original {message_id}"
        );
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
    fn require_verified_profile_accepts_pinned_x509_certificate_key_info() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins = vec![test_x509_public_key_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_x509_certificate();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("pinned X.509 XMLDSig payload should pass require-verified profile");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_x509_certificate_chain_trust_anchor() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_chain_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("X.509 chain signed by a pinned trust anchor should pass");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_rejects_directly_pinned_x509_unknown_critical_leaf() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins =
            vec![test_x509_unknown_critical_leaf_public_key_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_unknown_critical_leaf_x509_certificate_chain(false);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("direct public-key pins must not bypass unknown critical X.509 extensions");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_trust_anchor_x509_unknown_critical_leaf() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_unknown_critical_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_unknown_critical_leaf_x509_certificate_chain(true);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err(
                "trust-anchor chains must reject signer certs with unknown critical extensions",
            );

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_unknown_critical_trust_anchor() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_unknown_critical_bad_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_unknown_critical_root_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("pinned trust anchors with unknown critical extensions must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_directly_pinned_x509_expired_leaf() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins = vec![test_x509_expired_leaf_public_key_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_expired_x509_certificate_chain(false);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("direct public-key pins must not authorize expired X.509 signer leaves");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_trust_anchor_x509_expired_leaf() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_expired_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_expired_x509_certificate_chain(true);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("trust-anchor chains must not authorize expired X.509 signer leaves");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_accepts_x509_path_length_allowed_intermediate() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_pathlen_root1_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_pathlen_x509_certificate_chain(
            TEST_X509_PATHLEN_INTERMEDIATE_BY_ROOT1_CERTIFICATE_DER_B64,
            TEST_X509_PATHLEN_ROOT1_CERTIFICATE_DER_B64,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("pinned root with pathLen 1 should authorize one intermediate CA");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_rejects_x509_path_length_violation() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_pathlen_root0_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_pathlen_x509_certificate_chain(
            TEST_X509_PATHLEN_INTERMEDIATE_BY_ROOT0_CERTIFICATE_DER_B64,
            TEST_X509_PATHLEN_ROOT0_CERTIFICATE_DER_B64,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("pinned root with pathLen 0 must not authorize an intermediate CA");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_directly_pinned_x509_ca_signer_leaf() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins = vec![test_x509_ca_leaf_public_key_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_ca_leaf_x509_certificate_chain(false);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("direct public-key pins must not authorize CA certificates as signers");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_trust_anchor_x509_ca_signer_leaf() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_ca_leaf_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_ca_leaf_x509_certificate_chain(true);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("trust-anchor chains must not authorize CA certificates as signers");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_accepts_x509_certificate_policy_oid() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_policy_root_certificate_pin()];
        profile.x509_required_certificate_policy_oids = vec![TEST_X509_POLICY_OID.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_policy_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("X.509 signer certificate with the required policy OID should pass");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_x509_name_constraints_permitted_subtree() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_name_constraints_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_name_constrained_x509_certificate_chain(
            TEST_X509_NAME_CONSTRAINTS_ALLOWED_LEAF_CERTIFICATE_DER_B64,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("X.509 signer inside permitted name subtree should pass");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_rejects_x509_name_constraints_outside_permitted_subtree() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_name_constraints_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_name_constrained_x509_certificate_chain(
            TEST_X509_NAME_CONSTRAINTS_OUTSIDE_LEAF_CERTIFICATE_DER_B64,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("X.509 signer outside permitted name subtree must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_name_constraints_excluded_subtree() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_name_constraints_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_name_constrained_x509_certificate_chain(
            TEST_X509_NAME_CONSTRAINTS_EXCLUDED_LEAF_CERTIFICATE_DER_B64,
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("X.509 signer inside excluded name subtree must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn x509_name_constraint_matchers_cover_rfc822_uri_and_ip_forms() {
        assert!(rfc822_name_matches_constraint("ops@bank.test", "bank.test").expect("email"));
        assert!(
            !rfc822_name_matches_constraint("ops@example.test", "bank.test").expect("email reject")
        );
        assert!(
            uri_name_matches_constraint("https://signer.bank.test/path", ".bank.test")
                .expect("uri host")
        );
        assert!(
            !uri_name_matches_constraint("https://bank.test/path", ".bank.test")
                .expect("uri host excludes root")
        );
        assert!(
            ip_address_matches_constraint(&[192, 0, 2, 17], &[192, 0, 2, 0, 255, 255, 255, 0],)
                .expect("IPv4 subnet")
        );
        assert!(
            !ip_address_matches_constraint(&[192, 0, 3, 17], &[192, 0, 2, 0, 255, 255, 255, 0],)
                .expect("IPv4 subnet reject")
        );
    }

    #[test]
    fn require_verified_profile_accepts_x509_chain_with_configured_ocsp_response() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_ocsp_root_certificate_pin()];
        profile.x509_require_ocsp_revocation_check = true;
        profile.x509_ocsp_response_der_base64 =
            vec![TEST_X509_OCSP_GOOD_RESPONSE_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_ocsp_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("fresh good OCSP response should satisfy revocation checking");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_x509_chain_with_embedded_ocsp_response() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_ocsp_root_certificate_pin()];
        profile.x509_require_ocsp_revocation_check = true;
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_ocsp_x509_certificate_chain(Some(
            TEST_X509_OCSP_GOOD_RESPONSE_DER_B64,
        ));
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("fresh embedded good OCSP response should satisfy revocation checking");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_x509_chain_with_delegated_ocsp_responder() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_ocsp_delegated_root_certificate_pin()];
        profile.x509_require_ocsp_revocation_check = true;
        profile.x509_ocsp_response_der_base64 =
            vec![TEST_X509_OCSP_DELEGATED_GOOD_RESPONSE_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_delegated_ocsp_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect(
                "delegated OCSP signer with OCSPSigning EKU should satisfy revocation checking",
            );

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn profile_x509_ocsp_response_der_base64_rejects_invalid_material() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.x509_ocsp_response_der_base64 = vec!["not-an-ocsp-response".to_owned()];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("invalid OCSP must fail config"),
            Err(err) => err,
        };

        assert!(
            format!("{err:?}").contains("x509_ocsp_response_der_base64"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn require_verified_profile_rejects_missing_x509_ocsp_when_required() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_ocsp_root_certificate_pin()];
        profile.x509_require_ocsp_revocation_check = true;
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_ocsp_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("profiles requiring OCSP revocation checking must fail without OCSP");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_certificate_revoked_by_ocsp_response() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_ocsp_root_certificate_pin()];
        profile.x509_ocsp_response_der_base64 =
            vec![TEST_X509_OCSP_REVOKED_RESPONSE_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_ocsp_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("OCSP revoked signer certificate must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_delegated_ocsp_without_responder_certificate() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins =
            vec![test_x509_ocsp_delegated_root_certificate_pin()];
        profile.x509_ocsp_response_der_base64 =
            vec![TEST_X509_OCSP_DELEGATED_GOOD_NO_CERTS_RESPONSE_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_delegated_ocsp_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("delegated OCSP signer without responder certificate must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_accepts_x509_chain_with_configured_crl() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_crl_root_certificate_pin()];
        profile.x509_require_crl_revocation_check = true;
        profile.x509_crl_der_base64 = vec![TEST_X509_CRL_EMPTY_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_crl_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("fresh CRL with no leaf revocation should satisfy revocation checking");

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn require_verified_profile_accepts_x509_chain_with_embedded_crl() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_crl_root_certificate_pin()];
        profile.x509_require_crl_revocation_check = true;
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload =
            signed_pacs008_xml_with_crl_x509_certificate_chain(Some(TEST_X509_CRL_EMPTY_DER_B64));
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let metadata = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect(
                "fresh embedded CRL with no leaf revocation should satisfy revocation checking",
            );

        assert!(metadata.embedded_signature_detected());
    }

    #[test]
    fn profile_signature_public_key_pins_are_normalised_and_deduped() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        let pin = test_p256_public_key_pin();
        profile.signature_public_key_sha256_pins =
            vec![format!("  {}  ", pin.to_ascii_uppercase()), pin.clone()];
        config.profiles.push(profile);

        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        assert_eq!(profile.signature_public_key_sha256_pins, vec![pin]);
    }

    #[test]
    fn profile_signature_public_key_pins_reject_invalid_hex() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins = vec!["not-a-sha256-pin".to_owned()];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("invalid signer public-key pins must reject config"),
            Err(err) => err,
        };

        assert!(
            format!("{err:?}").contains("signature_public_key_sha256_pins"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn profile_signature_public_key_pins_reject_zero_placeholder() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins = vec!["00".repeat(32)];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("all-zero signer public-key pins must reject config"),
            Err(err) => err,
        };

        assert!(
            format!("{err:?}").contains("all-zero"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn profile_x509_trust_anchor_pins_reject_invalid_hex() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.x509_trust_anchor_sha256_pins = vec!["not-a-sha256-pin".to_owned()];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("invalid X.509 trust-anchor pins must reject config"),
            Err(err) => err,
        };

        assert!(
            format!("{err:?}").contains("x509_trust_anchor_sha256_pins"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn profile_x509_required_certificate_policy_oids_reject_invalid_oid() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.x509_required_certificate_policy_oids = vec!["1.40.bad".to_owned()];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("invalid X.509 certificate policy OIDs must reject config"),
            Err(err) => err,
        };

        assert!(
            format!("{err:?}").contains("x509_required_certificate_policy_oids"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn profile_x509_crl_der_base64_rejects_invalid_material() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.x509_crl_der_base64 = vec!["not-a-crl".to_owned()];
        config.profiles.push(profile);

        let err = match Iso20022BridgeRuntime::from_config(&config) {
            Ok(_) => panic!("invalid X.509 CRL material must reject config"),
            Err(err) => err,
        };

        assert!(
            format!("{err:?}").contains("x509_crl_der_base64"),
            "unexpected error: {err:?}"
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
    fn require_verified_profile_rejects_wrong_x509_trust_anchor_pin() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec!["22".repeat(32)];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("untrusted X.509 chains must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_missing_x509_certificate_policy_oid() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_chain_root_certificate_pin()];
        profile.x509_required_certificate_policy_oids = vec![TEST_X509_POLICY_OID.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("X.509 leaf certificates missing a required policy OID must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_wrong_x509_certificate_policy_oid() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_policy_root_certificate_pin()];
        profile.x509_required_certificate_policy_oids = vec!["1.3.6.1.4.1.55555.1.2".to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_policy_x509_certificate_chain();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("X.509 leaf certificates with the wrong policy OID must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_missing_x509_crl_when_required() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_crl_root_certificate_pin()];
        profile.x509_require_crl_revocation_check = true;
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_crl_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err(
                "profiles requiring CRL revocation checking must fail without CRL material",
            );

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_certificate_revoked_by_configured_crl() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_crl_root_certificate_pin()];
        profile.x509_crl_der_base64 = vec![TEST_X509_CRL_REVOKED_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_crl_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("configured CRL material must reject revoked signer certificates");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_expired_x509_crl() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_crl_root_certificate_pin()];
        profile.x509_require_crl_revocation_check = true;
        profile.x509_crl_der_base64 = vec![TEST_X509_CRL_EXPIRED_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_crl_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("expired CRLs must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_crl_from_wrong_issuer() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_crl_root_certificate_pin()];
        profile.x509_require_crl_revocation_check = true;
        profile.x509_crl_der_base64 = vec![TEST_X509_CRL_OTHER_ISSUER_DER_B64.to_owned()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_crl_x509_certificate_chain(None);
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("CRLs from unrelated issuers must not satisfy signer revocation checking");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_chain_missing_trust_anchor_certificate() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_chain_root_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_leaf_x509_certificate();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("leaf-only X.509 key info must not satisfy a root-anchor profile");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_non_ca_x509_trust_anchor() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![test_x509_leaf_certificate_pin()];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_leaf_x509_certificate();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("non-CA certificates must not act as X.509 trust anchors");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_x509_chain_issuer_mismatch() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        profile.x509_trust_anchor_sha256_pins = vec![sha256_hex(
            &BASE64_STANDARD
                .decode(TEST_X509_CERTIFICATE_DER_B64)
                .expect("X.509 fixture must decode"),
        )];
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_leaf_x509_certificate().replace(
            "</X509Data>",
            &format!(
                "<X509Certificate>{TEST_X509_CERTIFICATE_DER_B64}</X509Certificate></X509Data>"
            ),
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("issuer/subject mismatches in X.509 chains must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_unpinned_public_key() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
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
            .expect_err("require-verified must fail closed without a configured key pin");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_wrong_public_key_pin() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins = vec!["11".repeat(32)];
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
            .expect_err("mismatched signer public-key pins must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_unpinned_x509_certificate() {
        let mut config = sample_config();
        let mut profile = signed_message_profile("require-verified");
        profile.signature_public_key_sha256_pins.clear();
        config.profiles.push(profile);
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let payload = signed_pacs008_xml_with_x509_certificate();
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("X.509 XMLDSig payloads must fail closed without a configured key pin");

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
    fn require_verified_profile_rejects_unimplemented_canonicalization_methods() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        for unsupported_c14n in [XML_C14N_1_1, XML_EXCLUSIVE_C14N_1_0] {
            let payload = signed_pacs008_xml().replace(XML_C14N_1_0, unsupported_c14n);
            let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
            let err = runtime
                .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
                .expect_err("unsupported canonicalization methods must fail closed");

            assert!(matches!(err, MsgError::ValidationFailed));
        }
    }

    #[test]
    fn require_verified_profile_rejects_extra_reference_transforms() {
        let mut config = sample_config();
        config
            .profiles
            .push(signed_message_profile("require-verified"));
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let transform = format!(r#"<Transform Algorithm="{XMLDSIG_ENVELOPED_SIGNATURE}"/>"#);
        let payload = signed_pacs008_xml().replace(
            &transform,
            &format!(r#"<Transform Algorithm="urn:unsupported:base64"/>{transform}"#),
        );
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse signed XML");
        let profile = runtime
            .resolve_profile(Some("signed-pacs008-test"))
            .expect("signed profile");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("extra reference transforms must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_sgntr_blocks() {
        let payload = signed_pacs008_xml()
            .replace("<Signature>", "<Sgntr>")
            .replace("</Signature>", "</Sgntr>")
            .replace("</FIToFICstmrCdtTrf>", "<Sgntr/></FIToFICstmrCdtTrf>");

        let err = verify_embedded_xml_signature(
            payload.as_bytes(),
            &[test_p256_public_key_pin()],
            &[],
            &[],
            false,
            &[],
            false,
            &[],
        )
        .expect_err("duplicate Sgntr blocks must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_ambiguous_key_info_material() {
        let payload = signed_pacs008_xml().replace(
            "</KeyInfo>",
            "<X509Certificate>AA==</X509Certificate></KeyInfo>",
        );

        let err = verify_embedded_xml_signature(
            payload.as_bytes(),
            &[test_p256_public_key_pin()],
            &[],
            &[],
            false,
            &[],
            false,
            &[],
        )
        .expect_err("ambiguous raw public-key and X.509 key material must fail closed");

        assert!(matches!(err, MsgError::ValidationFailed));
    }

    #[test]
    fn require_verified_profile_rejects_duplicate_public_key_material() {
        let duplicate_public_key = BASE64_STANDARD.encode(
            test_p256_signing_key()
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        );
        let payload = signed_pacs008_xml().replace(
            "</KeyInfo>",
            &format!("<PublicKey>{duplicate_public_key}</PublicKey></KeyInfo>"),
        );

        let err = verify_embedded_xml_signature(
            payload.as_bytes(),
            &[test_p256_public_key_pin()],
            &[],
            &[],
            false,
            &[],
            false,
            &[],
        )
        .expect_err("duplicate public-key elements must fail closed");

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
    fn live_profile_records_exact_reference_snapshot_checksum() {
        let first_snapshot = r#"{
            "version":"2024-05-01",
            "source":"GLEIF sample A",
            "entries":[{"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}]
        }"#;
        let second_snapshot = r#"{
            "version":"2024-05-02",
            "source":"GLEIF sample B",
            "entries":[{"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}]
        }"#;
        let first_file = write_snapshot(first_snapshot);
        let second_file = write_snapshot(second_snapshot);
        let payload = swift_pacs008_xml("HDR-SNAPSHOT-1", "123e4567-e89b-12d3-a456-426614174000");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse live XML");

        let mut first_config = sample_config();
        first_config.reference_data.bic_lei_path = Some(first_file.path().to_path_buf());
        let first_runtime = Iso20022BridgeRuntime::from_config(&first_config)
            .expect("cfg")
            .expect("enabled");
        let first_profile = first_runtime
            .resolve_profile(Some("swift-cbpr-plus"))
            .expect("swift profile");
        let first_snapshot_id = first_runtime.reference_data().snapshot_id();
        let first_metadata = first_runtime
            .validate_profile_submission(first_profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("first reference snapshot validates");

        assert_eq!(
            first_metadata.reference_snapshot_id(),
            Some(first_snapshot_id.as_str())
        );

        let mut second_config = sample_config();
        second_config.reference_data.bic_lei_path = Some(second_file.path().to_path_buf());
        let second_runtime = Iso20022BridgeRuntime::from_config(&second_config)
            .expect("cfg")
            .expect("enabled");
        let second_profile = second_runtime
            .resolve_profile(Some("swift-cbpr-plus"))
            .expect("swift profile");
        let second_snapshot_id = second_runtime.reference_data().snapshot_id();
        let second_metadata = second_runtime
            .validate_profile_submission(second_profile, "pacs.008", &parsed, payload.as_bytes())
            .expect("second reference snapshot validates");

        assert_eq!(
            second_metadata.reference_snapshot_id(),
            Some(second_snapshot_id.as_str())
        );
        assert_ne!(first_snapshot_id, second_snapshot_id);
    }

    #[test]
    fn official_mdr_xsd_fixtures_cover_live_rail_profiles() {
        let (config, _reference_files) = sample_config_with_live_reference_data();
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let cases = vec![
            (
                "swift-cbpr-plus",
                "pacs.008",
                "pacs.008.001.08",
                "swift.cbprplus.02",
                OFFICIAL_XSD_PACS008_001_08,
                "FIToFICstmrCdtTrf",
                live_pacs008_xml(
                    "SWIFT-MDR-XSD-1",
                    "pacs.008.001.08",
                    "swift.cbprplus.02",
                    "USD",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174400",
                ),
            ),
            (
                "fedwire-funds",
                "pacs.008",
                "pacs.008.001.08",
                "fedwire.funds.01",
                OFFICIAL_XSD_PACS008_001_08,
                "FIToFICstmrCdtTrf",
                live_pacs008_xml(
                    "FEDWIRE-MDR-XSD-1",
                    "pacs.008.001.08",
                    "fedwire.funds.01",
                    "USD",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174401",
                ),
            ),
            (
                "sepa-sct-inst",
                "pacs.008",
                "pacs.008.001.08",
                "sepa.sct.inst",
                OFFICIAL_XSD_PACS008_001_08,
                "FIToFICstmrCdtTrf",
                live_pacs008_xml(
                    "SEPA-MDR-XSD-1",
                    "pacs.008.001.08",
                    "sepa.sct.inst",
                    "EUR",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174402",
                ),
            ),
            (
                "securities-csd",
                "pacs.009",
                "pacs.009.001.08",
                "securities.csd.cash",
                OFFICIAL_XSD_PACS009_001_08,
                "FICdtTrf",
                live_pacs009_xml(
                    "SECURITIES-MDR-XSD-1",
                    "pacs.009.001.08",
                    "securities.csd.cash",
                ),
            ),
        ];

        for (profile_id, message_type, msg_def_id, expected_service, xsd, expected_root, payload) in
            cases
        {
            let expected_namespace = format!("urn:iso:std:iso:20022:tech:xsd:{msg_def_id}");
            assert_eq!(xsd_schema_target_namespace(xsd), Some(expected_namespace));
            assert_eq!(
                xsd_document_payload_root(xsd).as_deref(),
                Some(expected_root)
            );

            let parsed = parse_message(message_type, payload.as_bytes())
                .unwrap_or_else(|err| panic!("{profile_id} MDR/XSD fixture must parse: {err:?}"));
            let profile = runtime
                .resolve_profile(Some(profile_id))
                .unwrap_or_else(|| panic!("{profile_id} profile"));
            let metadata = runtime
                .validate_profile_submission(profile, message_type, &parsed, payload.as_bytes())
                .unwrap_or_else(|err| {
                    panic!("{profile_id} MDR/XSD fixture must validate: {err:?}")
                });

            assert_eq!(metadata.profile_id(), Some(profile_id));
            assert_eq!(metadata.business_service(), Some(expected_service));
            assert_eq!(metadata.message_type(), Some(message_type));
        }
    }

    #[test]
    fn official_mdr_xsd_fixture_rejects_document_root_drift() {
        let pacs009_root = xsd_document_payload_root(OFFICIAL_XSD_PACS009_001_08)
            .expect("pacs.009 XSD root fixture");
        assert_eq!(pacs009_root, "FICdtTrf");
        let payload = live_pacs008_xml(
            "SWIFT-MDR-XSD-DRIFT",
            "pacs.008.001.08",
            "swift.cbprplus.02",
            "USD",
            "10.00",
            "123e4567-e89b-12d3-a456-426614174450",
        )
        .replace("FIToFICstmrCdtTrf", &pacs009_root);

        let err = parse_message("pacs.008", payload.as_bytes())
            .expect_err("pacs.008 must reject a pacs.009 XSD document root");

        assert!(matches!(err, MsgError::UnknownMessageType));
    }

    #[test]
    fn live_rail_profile_xsd_fixtures_accept_supported_messages() {
        let (config, _reference_files) = sample_config_with_live_reference_data();
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let cases = vec![
            (
                "swift-cbpr-plus",
                "pacs.008",
                "swift.cbprplus.02",
                live_pacs008_xml(
                    "SWIFT-XSD-1",
                    "pacs.008.001.08",
                    "swift.cbprplus.02",
                    "USD",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174100",
                ),
            ),
            (
                "fedwire-funds",
                "pacs.008",
                "fedwire.funds.01",
                live_pacs008_xml(
                    "FEDWIRE-XSD-1",
                    "pacs.008.001.08",
                    "fedwire.funds.01",
                    "USD",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174101",
                ),
            ),
            (
                "sepa-sct-inst",
                "pacs.008",
                "sepa.sct.inst",
                live_pacs008_xml(
                    "SEPA-XSD-1",
                    "pacs.008.001.10",
                    "sepa.sct.inst",
                    "EUR",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174102",
                ),
            ),
            (
                "securities-csd",
                "pacs.009",
                "securities.csd.cash",
                live_pacs009_xml("SECURITIES-XSD-1", "pacs.009.001.10", "securities.csd.cash"),
            ),
        ];

        for (profile_id, message_type, expected_service, payload) in cases {
            let parsed = parse_message(message_type, payload.as_bytes())
                .unwrap_or_else(|err| panic!("{profile_id} fixture must parse: {err:?}"));
            let profile = runtime
                .resolve_profile(Some(profile_id))
                .unwrap_or_else(|| panic!("{profile_id} profile"));
            let metadata = runtime
                .validate_profile_submission(profile, message_type, &parsed, payload.as_bytes())
                .unwrap_or_else(|err| panic!("{profile_id} fixture must validate: {err:?}"));

            assert_eq!(metadata.profile_id(), Some(profile_id));
            assert_eq!(metadata.business_service(), Some(expected_service));
            assert_eq!(
                metadata.reference_snapshot_id(),
                Some(runtime.reference_data().snapshot_id().as_str())
            );
            assert_eq!(metadata.message_type(), Some(message_type));
        }
    }

    #[test]
    fn live_rail_profile_xsd_fixtures_reject_wrong_business_services() {
        let (config, _reference_files) = sample_config_with_live_reference_data();
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let cases = vec![
            (
                "fedwire-funds",
                "pacs.008",
                live_pacs008_xml(
                    "FEDWIRE-BAD-SVC",
                    "pacs.008.001.08",
                    "swift.cbprplus.02",
                    "USD",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174201",
                ),
            ),
            (
                "sepa-sct-inst",
                "pacs.008",
                live_pacs008_xml(
                    "SEPA-BAD-SVC",
                    "pacs.008.001.10",
                    "fedwire.funds.01",
                    "EUR",
                    "10.00",
                    "123e4567-e89b-12d3-a456-426614174202",
                ),
            ),
            (
                "securities-csd",
                "pacs.009",
                live_pacs009_xml("SECURITIES-BAD-SVC", "pacs.009.001.10", "sepa.sct.inst"),
            ),
        ];

        for (profile_id, message_type, payload) in cases {
            let parsed = parse_message(message_type, payload.as_bytes())
                .unwrap_or_else(|err| panic!("{profile_id} wrong-service fixture parses: {err:?}"));
            let profile = runtime
                .resolve_profile(Some(profile_id))
                .unwrap_or_else(|| panic!("{profile_id} profile"));
            let err = runtime
                .validate_profile_submission(profile, message_type, &parsed, payload.as_bytes())
                .unwrap_err();

            assert!(matches!(
                err,
                MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Enum
                } if field == "AppHdr/BizSvc"
            ));
        }
    }

    #[test]
    fn live_rail_profile_xsd_fixtures_reject_version_and_amount_drift() {
        let (config, _reference_files) = sample_config_with_live_reference_data();
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let bad_version = live_pacs008_xml(
            "FEDWIRE-BAD-VERSION",
            "pacs.008.999.99",
            "fedwire.funds.01",
            "USD",
            "10.00",
            "123e4567-e89b-12d3-a456-426614174301",
        );
        let parsed_bad_version =
            parse_message("pacs.008", bad_version.as_bytes()).expect("bad-version XML parses");
        let fedwire = runtime
            .resolve_profile(Some("fedwire-funds"))
            .expect("fedwire profile");
        let err = runtime
            .validate_profile_submission(
                fedwire,
                "pacs.008",
                &parsed_bad_version,
                bad_version.as_bytes(),
            )
            .expect_err("unsupported MDR version must fail profile validation");
        assert!(matches!(err, MsgError::UnknownMessageType));

        let bad_minor_units = live_pacs008_xml(
            "SEPA-BAD-AMOUNT",
            "pacs.008.001.10",
            "sepa.sct.inst",
            "EUR",
            "10.001",
            "123e4567-e89b-12d3-a456-426614174302",
        );
        let parsed_bad_minor_units =
            parse_message("pacs.008", bad_minor_units.as_bytes()).expect("bad-amount XML parses");
        let sepa = runtime
            .resolve_profile(Some("sepa-sct-inst"))
            .expect("sepa profile");
        let err = runtime
            .validate_profile_submission(
                sepa,
                "pacs.008",
                &parsed_bad_minor_units,
                bad_minor_units.as_bytes(),
            )
            .expect_err("minor-unit drift must fail profile validation");
        assert!(matches!(
            err,
            MsgError::InvalidValue {
                field,
                kind: InvalidValueKind::Amount
            } if field == "IntrBkSttlmAmt"
        ));
    }

    #[test]
    fn live_profile_rejects_mismatched_message_version() {
        let snapshot = r#"{
            "version":"2024-05-01",
            "source":"GLEIF sample",
            "entries":[{"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}]
        }"#;
        let file = write_snapshot(snapshot);
        let mut config = sample_config();
        config.reference_data.bic_lei_path = Some(file.path().to_path_buf());
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let profile = runtime
            .resolve_profile(Some("swift-cbpr-plus"))
            .expect("swift profile");
        let payload = swift_pacs008_xml("HDR-BAD-VERSION", "123e4567-e89b-12d3-a456-426614174000")
            .replace("pacs.008.001.08", "pacs.008.999.99");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse bad version XML");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("unsupported live profile version must fail");

        assert!(matches!(err, MsgError::UnknownMessageType));
    }

    #[test]
    fn live_profile_rejects_mismatched_business_service() {
        let snapshot = r#"{
            "version":"2024-05-01",
            "source":"GLEIF sample",
            "entries":[{"bic":"DEUTDEFF","lei":"5493001KJTIIGC8Y1R12"}]
        }"#;
        let file = write_snapshot(snapshot);
        let mut config = sample_config();
        config.reference_data.bic_lei_path = Some(file.path().to_path_buf());
        let runtime = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let profile = runtime
            .resolve_profile(Some("swift-cbpr-plus"))
            .expect("swift profile");
        let payload = swift_pacs008_xml("HDR-BAD-SERVICE", "123e4567-e89b-12d3-a456-426614174000")
            .replace("swift.cbprplus.02", "fedwire.funds.01");
        let parsed = parse_message("pacs.008", payload.as_bytes()).expect("parse bad service XML");

        let err = runtime
            .validate_profile_submission(profile, "pacs.008", &parsed, payload.as_bytes())
            .expect_err("unsupported business service must fail");

        assert!(matches!(
            err,
            MsgError::InvalidValue {
                field,
                kind: InvalidValueKind::Enum
            } if field == "AppHdr/BizSvc"
        ));
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
            Some(" biz-duplicate ".to_owned()),
            Some("123e4567-e89b-12d3-a456-426614174000".to_owned()),
            "hash-1".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        let replay = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-duplicate".to_owned()),
            Some("123e4567-e89b-12d3-a456-426614174001".to_owned()),
            "hash-2".to_owned(),
            "snapshot".to_owned(),
            false,
        );

        assert!(runtime.check_and_record_inbound("msg-1", first));
        assert!(!runtime.check_and_record_inbound("msg-2", replay));
        assert_eq!(
            runtime
                .business_message_id_index
                .get("biz-duplicate")
                .map(|entry| entry.value().clone()),
            Some("msg-1".to_owned())
        );
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
    fn durable_store_reloads_business_message_id_index() {
        let store = TempDir::new().expect("tempdir");
        let mut config = sample_config();
        config.store_dir = Some(store.path().to_path_buf());

        {
            let runtime = Iso20022BridgeRuntime::from_config(&config)
                .expect("cfg")
                .expect("enabled");
            let first = IsoMessageMetadata::inbound(
                "generic-iso20022",
                "pacs.008",
                None,
                Some("biz-persisted".to_owned()),
                Some("123e4567-e89b-12d3-a456-426614174000".to_owned()),
                "hash-1".to_owned(),
                "snapshot".to_owned(),
                false,
            );
            assert!(runtime.check_and_record_inbound("msg-1", first));
        }

        let reloaded = Iso20022BridgeRuntime::from_config(&config)
            .expect("cfg")
            .expect("enabled");
        let replay = IsoMessageMetadata::inbound(
            "generic-iso20022",
            "pacs.008",
            None,
            Some("biz-persisted".to_owned()),
            Some("123e4567-e89b-12d3-a456-426614174001".to_owned()),
            "hash-2".to_owned(),
            "snapshot".to_owned(),
            false,
        );
        assert!(!reloaded.check_and_record_inbound("msg-2", replay));
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
        record_original(&runtime, "orig-1", "pacs.008");
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
    fn lifecycle_pacs002_ignores_non_payment_original() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        record_original(&runtime, "orig-status-securities", "sese.023");
        runtime.mark_accepted("orig-status-securities", "tx-status-securities");
        let parsed = parse_message(
            "pacs.002",
            b"MsgId=status-securities\nOrgnlMsgId=orig-status-securities\nTxSts=ACSC",
        )
        .expect("pacs.002 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("pacs.002", &parsed).expect("lifecycle id");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "pacs.002", &parsed)
            .expect("lifecycle applied");

        assert_eq!(
            outcome.referenced_message_id(),
            Some("orig-status-securities")
        );
        assert!(outcome.referenced_message_known());
        assert_eq!(outcome.action(), "ignored_profile_mismatch");
        assert_eq!(
            runtime
                .message_status("orig-status-securities")
                .expect("original status")
                .pacs002_code(),
            "ACSP"
        );
    }

    #[test]
    fn lifecycle_pacs004_marks_original_returned() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        record_original(&runtime, "orig-return", "pacs.008");
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
    fn lifecycle_pacs004_ignores_non_payment_original() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        record_original(&runtime, "orig-return-securities", "sese.023");
        runtime.mark_accepted("orig-return-securities", "tx-return-securities");
        let parsed = parse_message(
            "pacs.004",
            b"MsgId=return-securities\nCreDtTm=2025-01-01T00:00:00Z\nOrgnlGrpInf/OrgnlMsgId=orig-return-securities\nTxInf[0]/OrgnlInstrId=instr-1\nTxInf[0]/RtrdInstdAmt=10.00\nTxInf[0]/RtrdInstdAmtCcy=USD\nTxInf[0]/RtrdRsn/Cd=AC01",
        )
        .expect("pacs.004 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("pacs.004", &parsed).expect("lifecycle id");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "pacs.004", &parsed)
            .expect("lifecycle applied");

        assert_eq!(
            outcome.referenced_message_id(),
            Some("orig-return-securities")
        );
        assert!(outcome.referenced_message_known());
        assert_eq!(outcome.action(), "ignored_profile_mismatch");
        let original = runtime
            .message_status("orig-return-securities")
            .expect("original status");
        assert_eq!(original.status_label(), "Accepted");
        assert_eq!(original.pacs002_code(), "ACSP");
        assert_eq!(original.rejection_reason_code(), None);
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
    fn lifecycle_camt056_ignores_non_payment_original() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        record_original(&runtime, "orig-cancel-securities", "sese.023");
        runtime.mark_accepted("orig-cancel-securities", "tx-cancel-securities");
        let parsed = parse_message(
            "camt.056",
            b"Assgnmt/Id=cancel-securities\nAssgnmt/CreDtTm=2025-01-01T00:00:00Z\nUndrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId=orig-cancel-securities\nUndrlyg/TxInf/CxlRsnInf/Rsn/Cd=CUST",
        )
        .expect("camt.056 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("camt.056", &parsed).expect("lifecycle id");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "camt.056", &parsed)
            .expect("lifecycle applied");

        assert_eq!(
            outcome.referenced_message_id(),
            Some("orig-cancel-securities")
        );
        assert!(outcome.referenced_message_known());
        assert_eq!(outcome.action(), "ignored_profile_mismatch");
        let original = runtime
            .message_status("orig-cancel-securities")
            .expect("original status");
        assert_eq!(original.pacs002_code(), "ACSP");
        assert_eq!(original.hold_reason_code(), None);
        assert!(original.change_reason_codes().is_empty());
    }

    #[test]
    fn lifecycle_sese025_confirms_prefixed_settlement_instruction() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        record_original(&runtime, "sese.023:settle-1", "sese.023");
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
    fn lifecycle_sese025_ignores_non_settlement_original() {
        let runtime = Iso20022BridgeRuntime::from_config(&sample_config())
            .expect("cfg")
            .expect("enabled");
        record_original(&runtime, "sese.023:settle-wrong-family", "pacs.008");
        runtime.mark_accepted("sese.023:settle-wrong-family", "tx-wrong-family");
        let parsed = parse_message(
            "sese.025",
            b"TxId=settle-wrong-family\nSttlmDt=2025-01-02\nSttlmTpAndAddtlParams/SctiesMvmntTp=DELI\nSttlmTpAndAddtlParams/Pmt=APMT\nConfSts=ACCP\nSttlmQty=100\nSttlmAmt=25.00\nSttlmCcy=USD\nPlan/ExecutionOrder=DELIVERY_THEN_PAYMENT\nPlan/Atomicity=ALL_OR_NOTHING",
        )
        .expect("sese.025 parsed");
        let lifecycle_id =
            Iso20022BridgeRuntime::lifecycle_message_id("sese.025", &parsed).expect("lifecycle id");
        assert!(runtime.check_and_record_message(&lifecycle_id));
        let outcome = runtime
            .apply_inbound_lifecycle_message(&lifecycle_id, "sese.025", &parsed)
            .expect("lifecycle applied");

        assert_eq!(
            outcome.referenced_message_id(),
            Some("sese.023:settle-wrong-family")
        );
        assert!(outcome.referenced_message_known());
        assert_eq!(outcome.action(), "ignored_profile_mismatch");
        assert_eq!(
            runtime
                .message_status("sese.023:settle-wrong-family")
                .expect("original status")
                .pacs002_code(),
            "ACSP"
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
