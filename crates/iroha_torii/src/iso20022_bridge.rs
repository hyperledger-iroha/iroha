use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write as FmtWrite,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use dashmap::DashMap;
use eyre::WrapErr as _;
use iroha_config::parameters::actual;
use iroha_core::iso_bridge::reference_data::{
    ReferenceDataError, ReferenceDataSnapshots, ValidationOutcome,
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
    dedupe_ttl: Duration,
    records: DashMap<String, IsoMessageRecord>,
    hash_index: DashMap<String, String>,
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
    context: IsoMessageContext,
    derived_status: Pacs002Status,
    hold_reason_code: Option<String>,
    change_reason_codes: Vec<String>,
    rejection_reason_code: Option<String>,
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

    pub fn derived_status(&self) -> Pacs002Status {
        self.derived_status
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
    ledger_tx_queued: bool,
    settled_at: Option<SystemTime>,
    hold_reason_code: Option<String>,
    change_reason_codes: Vec<String>,
    rejection_reason_code: Option<String>,
}

impl IsoMessageRecord {
    fn pending(now: Instant) -> Self {
        Self {
            last_seen: now,
            updated_at: SystemTime::now(),
            state: IsoMessageState::Pending,
            transaction_hash: None,
            detail: None,
            context: IsoMessageContext::default(),
            ledger_tx_queued: false,
            settled_at: None,
            hold_reason_code: None,
            change_reason_codes: Vec::new(),
            rejection_reason_code: None,
        }
    }

    fn accepted(now: Instant, tx_hash: String) -> Self {
        Self {
            last_seen: now,
            updated_at: SystemTime::now(),
            state: IsoMessageState::Accepted,
            transaction_hash: Some(tx_hash),
            detail: None,
            context: IsoMessageContext::default(),
            ledger_tx_queued: true,
            settled_at: None,
            hold_reason_code: None,
            change_reason_codes: Vec::new(),
            rejection_reason_code: None,
        }
    }

    fn rejected(now: Instant, detail: Option<String>) -> Self {
        Self {
            last_seen: now,
            updated_at: SystemTime::now(),
            state: IsoMessageState::Rejected,
            transaction_hash: None,
            detail,
            context: IsoMessageContext::default(),
            ledger_tx_queued: false,
            settled_at: None,
            hold_reason_code: None,
            change_reason_codes: Vec::new(),
            rejection_reason_code: None,
        }
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
}

const ISO_PACS008_CONTEXT: &str = "/v1/iso20022/pacs008";
const ISO_PACS009_CONTEXT: &str = "/v1/iso20022/pacs009";

fn parse_config_account_id(literal: &str, field: &str) -> eyre::Result<AccountId> {
    AccountId::parse_encoded(literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .wrap_err_with(|| format!("{field} must parse as an account identifier"))
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

        let runtime = Iso20022BridgeRuntime {
            signer_account,
            signer_private_key,
            account_aliases: Arc::new(aliases),
            alias_indices: Arc::new(alias_indices),
            index_aliases: Arc::new(index_aliases),
            currency_assets: Arc::new(currencies),
            reference_data,
            dedupe_ttl: Duration::from_secs(config.dedupe_ttl_secs),
            records: DashMap::new(),
            hash_index: DashMap::new(),
        };

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

    /// Perform a deduplication check for the provided message identifier.
    /// Returns `true` when the identifier is new (and records it), or `false`
    /// when a still-active entry already exists.
    pub fn check_and_record_message(&self, message_id: &str) -> bool {
        let now = Instant::now();
        self.prune_expired(now);
        if let Some(mut existing) = self.records.get_mut(message_id) {
            let expired = now.saturating_duration_since(existing.last_seen) > self.dedupe_ttl;
            if expired || existing.state == IsoMessageState::Rejected {
                if let Some(old_hash) = existing.transaction_hash.take() {
                    self.hash_index.remove(&old_hash);
                }
                *existing = IsoMessageRecord::pending(now);
                true
            } else {
                false
            }
        } else {
            self.records
                .insert(message_id.to_owned(), IsoMessageRecord::pending(now));
            true
        }
    }

    /// Remove a tracked message identifier from the dedupe cache (e.g. after a failed submission).
    pub fn remove_message(&self, message_id: &str) {
        if let Some((_, record)) = self.records.remove(message_id) {
            if let Some(hash) = record.transaction_hash {
                self.hash_index.remove(&hash);
            }
        }
    }

    /// Record supplementary ledger/account context attached to the message.
    pub fn update_message_context(&self, message_id: &str, context: IsoMessageContext) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.context = context;
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.context = context;
            self.records.insert(message_id.to_owned(), record);
        }
    }

    /// Mark the provided message as queued for ledger execution.
    pub fn mark_queued(&self, message_id: &str) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.set_queued();
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.set_queued();
            self.records.insert(message_id.to_owned(), record);
        }
    }

    /// Flag a message as pending due to screening/manual hold with an optional ISO reason code.
    pub fn mark_hold(&self, message_id: &str, reason_code: Option<&str>) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.set_hold_reason(reason_code.map(std::borrow::ToOwned::to_owned));
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.set_hold_reason(reason_code.map(std::borrow::ToOwned::to_owned));
            self.records.insert(message_id.to_owned(), record);
        }
    }

    /// Clear any previously-set hold indicator for the message.
    pub fn clear_hold(&self, message_id: &str) {
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = Instant::now();
            existing.updated_at = SystemTime::now();
            existing.clear_hold();
        }
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
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.replace_change_reason_codes(codes_vec);
            self.records.insert(message_id.to_owned(), record);
        }
    }

    /// Append a change-reason code for the message (deduplicated).
    pub fn add_change_reason_code(&self, message_id: &str, code: &str) {
        let now = Instant::now();
        if let Some(mut existing) = self.records.get_mut(message_id) {
            existing.last_seen = now;
            existing.updated_at = SystemTime::now();
            existing.add_change_reason_code(code.to_owned());
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.add_change_reason_code(code.to_owned());
            self.records.insert(message_id.to_owned(), record);
        }
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
        } else {
            let mut record = IsoMessageRecord::pending(now);
            record.state = IsoMessageState::Accepted;
            record.set_queued();
            record.mark_settled(settled_at);
            record.clear_hold();
            record.rejection_reason_code = None;
            self.records.insert(message_id.to_owned(), record);
        }
    }

    /// Mark the transaction identified by `tx_hash` as applied and fully settled.
    pub fn mark_transaction_applied(&self, tx_hash: &str, settled_at: SystemTime) {
        if let Some((_, message_id)) = self.hash_index.remove(tx_hash) {
            self.mark_settled(&message_id, settled_at);
        }
    }

    /// Mark the transaction identified by `tx_hash` as rejected.
    pub fn mark_transaction_rejected(
        &self,
        tx_hash: &str,
        reason: Option<&TransactionRejectionReason>,
    ) {
        if let Some((_, message_id)) = self.hash_index.remove(tx_hash) {
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
        if let Some((_, message_id)) = self.hash_index.remove(tx_hash) {
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
                    self.hash_index.remove(&old_hash);
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
        } else {
            self.records.insert(
                message_id.to_owned(),
                IsoMessageRecord::accepted(now, tx_hash.clone()),
            );
        }
        self.hash_index.insert(tx_hash, message_id.to_owned());
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
                self.hash_index.remove(&old_hash);
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
        } else {
            let mut record = IsoMessageRecord::rejected(now, reason);
            record.rejection_reason_code = reason_code.map(std::borrow::ToOwned::to_owned);
            self.records.insert(message_id.to_owned(), record);
        }
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
                context,
                derived_status,
                hold_reason_code,
                change_reason_codes,
                rejection_reason_code: record.rejection_reason_code.clone(),
            }
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
    fn prune_expired(&self, now: Instant) {
        let ttl = self.dedupe_ttl;
        self.records
            .retain(|_, record| now.saturating_duration_since(record.last_seen) <= ttl);
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
    use tempfile::NamedTempFile;

    use super::*;

    const LEGACY_PUBLIC_KEY_LITERAL: &str =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@test";

    fn sample_account_bundle() -> (AccountId, String, iroha_crypto::PrivateKey) {
        let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
        let (public_key, private_key) = key_pair.into_parts();
        let account = AccountId::new(public_key);
        let literal = account.to_string();
        (account, literal, private_key)
    }

    fn sample_asset_definition_literal() -> String {
        AssetDefinitionId::new(
            "test".parse().expect("domain"),
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

    fn sample_world(asset_alias: Option<&str>) -> World {
        let (authority, _, _) = sample_account_bundle();
        let domain_id: DomainId = "test".parse().expect("domain");
        let asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "usd".parse().expect("name"));
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let account = Account::new_in_domain(authority.clone(), domain_id).build(&authority);
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
                "sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"
                    .to_string(),
            ),
            ..IsoMessageContext::default()
        };
        runtime.update_message_context(message_id, context.clone());
        runtime.mark_accepted(message_id, "hash-ctx");

        let status = runtime.message_status(message_id).expect("status");
        assert_eq!(status.ledger_id(), Some("ledger-A"));
        assert_eq!(
            status.source_account_id(),
            Some("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76")
        );
        assert_eq!(status.transaction_hash(), Some("hash-ctx"));
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
}
