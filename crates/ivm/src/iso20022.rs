//! ISO 20022 message handling opcodes and bridge parser support.
//!
//! The module keeps a compact in-memory message stack for IVM opcodes while providing deterministic
//! XML and key-value parsing for the ISO bridge. XML payloads are bound to their declared ISO
//! message definition through `MsgDefIdr` and `Document` XSD namespaces before schema-table
//! validation is applied. Network transport is deliberately outside this module and outside
//! consensus execution.
use crate::signature::{SignatureScheme, verify_signature};
use core::fmt;
use ed25519_dalek::{Signer as _, SigningKey};
use iroha_crypto::{Algorithm, EcdsaSecp256k1Sha256};
use sha2::{Digest as _, Sha256};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    io::Write,
    ops::Range,
};
#[derive(Clone, Copy, Debug)]
struct XmlFieldSource {
    start: usize,
    end: usize,
}
/// Extremely small ISO 20022 message representation used for testing.
#[derive(Clone, Default)]
struct IsoMessage {
    /// Identifier such as `pacs.008`.
    message_type: String,
    /// Flat map of field name to encoded value.
    fields: HashMap<String, Vec<u8>>,
    /// Counters for `MSG_ADD` to emulate repeating fields.
    repeats: HashMap<String, usize>,
    /// Digest of the real XML source from which the fields were materialised.
    xml_source_sha256: Option<[u8; 32]>,
    /// Byte range in the original source which owns each materialised field.
    xml_field_sources: HashMap<String, XmlFieldSource>,
}
thread_local! {
    /// Thread-local stack of ISO 20022 messages.  The most recently created
    /// or parsed message lives at the top of the stack.
    static MESSAGE_STACK: RefCell<Vec<IsoMessage>> = const { RefCell::new(Vec::new()) };
    /// Last validation failure recorded by [`msg_validate`].
    static LAST_VALIDATION_FAILURE: RefCell<Option<ValidationFailure>> =
        const { RefCell::new(None) };
}
/// Return the list of fields that must be present for the given message type.
///
/// This is a tiny stand-in for schema driven validation. Only a handful of message types are
/// recognised and each lists a couple of representative mandatory fields. The map can be extended
/// over time as more messages are supported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Requirement {
    Required,
    Optional,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentifierKind {
    /// International Securities Identification Number (ISO 6166)
    Isin,
    /// Committee on Uniform Securities Identification Procedures
    Cusip,
    /// Legal Entity Identifier (ISO 17442)
    Lei,
    /// Business Identifier Code (ISO 9362)
    Bic,
    /// Market Identifier Code (ISO 10383)
    Mic,
    /// International Bank Account Number (ISO 13616 / 7064 checksum)
    Iban,
    /// ISO 4217 currency code
    Currency,
}
impl IdentifierKind {
    fn label(self) -> &'static str {
        match self {
            IdentifierKind::Isin => "ISIN",
            IdentifierKind::Cusip => "CUSIP",
            IdentifierKind::Lei => "LEI",
            IdentifierKind::Bic => "BIC",
            IdentifierKind::Mic => "MIC",
            IdentifierKind::Iban => "IBAN",
            IdentifierKind::Currency => "ISO 4217 currency",
        }
    }
}
impl fmt::Display for IdentifierKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldKind {
    Text,
    Numeric,
    Amount,
    Identifier(IdentifierKind),
    Instrument,
    Date,
    DateTime,
    Enum(&'static [&'static str]),
}
#[derive(Clone, Debug)]
enum InvalidReason {
    Empty,
    Numeric,
    Amount,
    Identifier(IdentifierKind),
    Instrument,
    Date,
    DateTime,
    Enum,
    Utf8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidValueKind {
    Empty,
    Numeric,
    Amount,
    Date,
    DateTime,
    Enum,
    Utf8,
}
impl InvalidValueKind {
    fn label(self) -> &'static str {
        match self {
            InvalidValueKind::Empty => "empty",
            InvalidValueKind::Numeric => "numeric",
            InvalidValueKind::Amount => "amount",
            InvalidValueKind::Date => "date",
            InvalidValueKind::DateTime => "date-time",
            InvalidValueKind::Enum => "enumerated",
            InvalidValueKind::Utf8 => "UTF-8",
        }
    }
}
impl fmt::Display for InvalidValueKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
#[derive(Clone, Debug)]
enum ValidationFailure {
    MissingField(&'static str),
    TooManyOccurrences {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    InvalidField {
        field: String,
        reason: InvalidReason,
    },
}
#[derive(Clone, Copy, Debug)]
struct AliasSpec {
    alias: &'static str,
    canonical: &'static str,
}
#[derive(Clone, Copy, Debug)]
struct FieldSpec {
    pattern: &'static str,
    requirement: Requirement,
    max_occurs: Option<usize>,
    kind: FieldKind,
}
impl FieldSpec {
    const fn required(pattern: &'static str, kind: FieldKind) -> Self {
        Self {
            pattern,
            requirement: Requirement::Required,
            max_occurs: None,
            kind,
        }
    }
    const fn optional(pattern: &'static str, kind: FieldKind) -> Self {
        Self {
            pattern,
            requirement: Requirement::Optional,
            max_occurs: None,
            kind,
        }
    }
    const fn limited(
        pattern: &'static str,
        min_required: bool,
        max: usize,
        kind: FieldKind,
    ) -> Self {
        Self {
            pattern,
            requirement: if min_required {
                Requirement::Required
            } else {
                Requirement::Optional
            },
            max_occurs: Some(max),
            kind,
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct MessageSchema {
    fields: &'static [FieldSpec],
    aliases: &'static [AliasSpec],
}
impl MessageSchema {
    fn field_specs(&self) -> &'static [FieldSpec] {
        self.fields
    }
    fn aliases(&self) -> &'static [AliasSpec] {
        self.aliases
    }
}
fn canonical_message_type(message_type: &str) -> Cow<'_, str> {
    let parts: Vec<&str> = message_type.split('.').collect();
    if parts.len() >= 4
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && parts[2].chars().all(|c| c.is_ascii_digit())
    {
        Cow::Owned(format!("{}.{}", parts[0], parts[1]))
    } else {
        Cow::Borrowed(message_type)
    }
}
fn record_validation_failure(failure: ValidationFailure) {
    LAST_VALIDATION_FAILURE.with(|cell| {
        *cell.borrow_mut() = Some(failure);
    });
}
fn take_validation_failure() -> Option<ValidationFailure> {
    LAST_VALIDATION_FAILURE.with(|cell| cell.borrow_mut().take())
}
fn clear_validation_failure() {
    LAST_VALIDATION_FAILURE.with(|cell| {
        cell.borrow_mut().take();
    });
}
include!(concat!(env!("OUT_DIR"), "/iso20022_schema_v1.rs"));
/// Errors that can occur when parsing, validating, or serializing ISO 20022 messages.
#[derive(Debug)]
pub enum MsgError {
    NoActiveMessage,
    UnknownMessageType,
    ValidationFailed,
    MissingField(&'static str),
    TooManyOccurrences {
        field: &'static str,
        max: usize,
        actual: usize,
    },
    InvalidIdentifier {
        field: String,
        kind: IdentifierKind,
    },
    InvalidInstrument {
        field: String,
    },
    InvalidValue {
        field: String,
        kind: InvalidValueKind,
    },
    InvalidFormat,
}
impl fmt::Display for MsgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MsgError::NoActiveMessage => f.write_str("no active ISO 20022 message"),
            MsgError::UnknownMessageType => f.write_str("unsupported ISO 20022 message type"),
            MsgError::ValidationFailed => f.write_str("ISO 20022 validation failed"),
            MsgError::MissingField(field) => {
                write!(f, "missing ISO 20022 field `{field}`")
            }
            MsgError::TooManyOccurrences { field, max, actual } => write!(
                f,
                "field `{field}` exceeds max occurrences ({actual} > {max})"
            ),
            MsgError::InvalidIdentifier { field, kind } => {
                write!(f, "invalid {} value for field `{field}`", kind.label())
            }
            MsgError::InvalidInstrument { field } => write!(
                f,
                "field `{field}` must contain a valid ISIN or CUSIP identifier"
            ),
            MsgError::InvalidValue { field, kind } => {
                write!(f, "invalid {} value for field `{field}`", kind.label())
            }
            MsgError::InvalidFormat => f.write_str("ISO 20022 message format is invalid"),
        }
    }
}
impl From<ValidationFailure> for MsgError {
    fn from(failure: ValidationFailure) -> Self {
        match failure {
            ValidationFailure::MissingField(field) => MsgError::MissingField(field),
            ValidationFailure::TooManyOccurrences { field, max, actual } => {
                MsgError::TooManyOccurrences { field, max, actual }
            }
            ValidationFailure::InvalidField { field, reason } => match reason {
                InvalidReason::Identifier(kind) => MsgError::InvalidIdentifier { field, kind },
                InvalidReason::Instrument => MsgError::InvalidInstrument { field },
                InvalidReason::Empty => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Empty,
                },
                InvalidReason::Numeric => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Numeric,
                },
                InvalidReason::Amount => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Amount,
                },
                InvalidReason::Date => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Date,
                },
                InvalidReason::DateTime => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::DateTime,
                },
                InvalidReason::Enum => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Enum,
                },
                InvalidReason::Utf8 => MsgError::InvalidValue {
                    field,
                    kind: InvalidValueKind::Utf8,
                },
            },
        }
    }
}
/// Consume and return the most recent validation error recorded by [`msg_validate`].
///
/// The helper yields [`MsgError`] variants mirroring the validation failure and clears the stored
/// state so subsequent calls return `None` until [`msg_validate`] runs again.
#[must_use]
pub fn take_validation_error() -> Option<MsgError> {
    take_validation_failure().map(MsgError::from)
}
/// Materialised ISO 20022 message extracted from the VM stack.
#[derive(Clone, Debug)]
pub struct ParsedMessage {
    message_type: String,
    fields: BTreeMap<String, Vec<u8>>,
    xml_source_sha256: Option<[u8; 32]>,
    xml_field_sources: BTreeMap<String, XmlFieldSource>,
}
impl ParsedMessage {
    /// Return the ISO 20022 message code (e.g. `"pacs.008"`).
    pub fn message_type(&self) -> &str {
        &self.message_type
    }
    /// Retrieve the raw bytes stored under the canonical field path.
    pub fn field_bytes(&self, field: &str) -> Option<&[u8]> {
        self.fields.get(field).map(|v| v.as_slice())
    }
    /// Retrieve the UTF-8 string stored under the canonical field path.
    pub fn field_text(&self, field: &str) -> Option<&str> {
        self.field_bytes(field)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
    }
    /// Iterate over canonical field paths and their values.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.fields.iter()
    }
    /// Return whether every materialised field came from `source` inside `range`.
    ///
    /// Messages parsed from the developer-only key/value or internal XML formats,
    /// messages mutated after XML parsing, and incomplete provenance fail closed.
    #[must_use]
    pub fn fields_are_covered_by_xml_range(&self, source: &[u8], range: Range<usize>) -> bool {
        if range.start > range.end || range.end > source.len() {
            return false;
        }
        let Some(expected_digest) = self.xml_source_sha256 else {
            return false;
        };
        let actual_digest: [u8; 32] = Sha256::digest(source).into();
        expected_digest == actual_digest
            && self.xml_field_sources.len() == self.fields.len()
            && self.fields.keys().all(|field| {
                self.xml_field_sources.get(field).is_some_and(|source| {
                    source.start >= range.start
                        && source.end <= range.end
                        && source.start <= source.end
                })
            })
    }
}
fn materialise_current_message(valid: bool) -> Result<ParsedMessage, MsgError> {
    MESSAGE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let maybe_msg = stack.pop();
        drop(stack);
        match (valid, maybe_msg) {
            (true, Some(msg)) => Ok(ParsedMessage {
                message_type: msg.message_type,
                fields: msg.fields.into_iter().collect(),
                xml_source_sha256: msg.xml_source_sha256,
                xml_field_sources: msg.xml_field_sources.into_iter().collect(),
            }),
            (false, _) => {
                let err = take_validation_failure()
                    .map(MsgError::from)
                    .unwrap_or(MsgError::ValidationFailed);
                Err(err)
            }
            (true, None) => Err(MsgError::NoActiveMessage),
        }
    })
}
/// Parse, validate, and materialise an ISO 20022 message.
///
/// This helper wraps `msg_parse`/`msg_validate` and drains the temporary VM
/// stack entry, returning an owned [`ParsedMessage`] on success.
pub fn parse_message(message_type: &str, data: &[u8]) -> Result<ParsedMessage, MsgError> {
    msg_parse(message_type, data)?;
    materialise_current_message(msg_validate())
}

/// Parse, validate, and materialise a production ISO 20022 XML message.
///
/// Unlike [`parse_message`], this entry point rejects the developer-only
/// key/value and internal `<ISO20022>` representations.
pub fn parse_xml_message(message_type: &str, data: &[u8]) -> Result<ParsedMessage, MsgError> {
    if !looks_like_xml(data) {
        return Err(MsgError::InvalidFormat);
    }
    let text = core::str::from_utf8(data).map_err(|_| MsgError::InvalidFormat)?;
    if text.trim_start().starts_with("<ISO20022") {
        return Err(MsgError::InvalidFormat);
    }
    msg_create(message_type);
    if let Err(error) = parse_real_iso20022(message_type, text) {
        MESSAGE_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
        return Err(error);
    }
    materialise_current_message(msg_validate())
}
/// Norito-friendly projections of the ISO 20022 settlement messages covered by
/// this helper. These structs make it easy to encode/decode settlement payloads
/// alongside the VM message stack without reimplementing field mapping.
pub mod norito_schemas {
    use super::{InvalidValueKind, MsgError, ParsedMessage, msg_add, msg_create, msg_set};
    use norito::codec::{Decode, Encode};
    fn required_text(parsed: &ParsedMessage, field: &'static str) -> Result<String, MsgError> {
        parsed
            .field_text(field)
            .map(str::to_owned)
            .ok_or(MsgError::MissingField(field))
    }
    fn optional_text(parsed: &ParsedMessage, field: &str) -> Option<String> {
        parsed.field_text(field).map(str::to_owned)
    }
    fn optional_bool(
        parsed: &ParsedMessage,
        field: &'static str,
    ) -> Result<Option<bool>, MsgError> {
        let Some(text) = parsed.field_text(field) else {
            return Ok(None);
        };
        match text {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Err(MsgError::InvalidValue {
                field: field.to_owned(),
                kind: InvalidValueKind::Enum,
            }),
        }
    }
    fn bool_bytes(value: bool) -> &'static [u8] {
        if value { b"true" } else { b"false" }
    }
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Linkage {
        pub relation: String,
        pub reference: String,
    }
    /// Norito schema for `sese.023` DvP instructions.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Sese023 {
        pub tx_id: String,
        pub settlement_date: String,
        pub movement_type: String,
        pub payment_type: String,
        pub fin_instr_id: String,
        pub quantity: String,
        pub cash_amount: String,
        pub cash_currency: String,
        pub delivering_party_bic: String,
        pub delivering_account: String,
        pub receiving_party_bic: String,
        pub receiving_account: String,
        pub execution_order: String,
        pub atomicity: String,
        pub settlement_condition: Option<String>,
        pub partial_settlement_indicator: Option<String>,
        pub hold_indicator: Option<bool>,
        pub venue_mic: Option<String>,
        pub linkages: Vec<Linkage>,
        pub securities_metadata: Option<String>,
        pub cash_metadata: Option<String>,
    }
    impl Sese023 {
        /// Populate the VM message stack with this instruction.
        pub fn apply_to_stack(&self) {
            msg_create("sese.023");
            msg_set("TxId", self.tx_id.as_bytes());
            msg_set("SttlmDt", self.settlement_date.as_bytes());
            msg_set(
                "SttlmTpAndAddtlParams/SctiesMvmntTp",
                self.movement_type.as_bytes(),
            );
            msg_set("SttlmTpAndAddtlParams/Pmt", self.payment_type.as_bytes());
            if let Some(condition) = &self.settlement_condition {
                msg_set("SttlmParams/SttlmTxCond/Cd", condition.as_bytes());
            }
            if let Some(indicator) = &self.partial_settlement_indicator {
                msg_set("SttlmParams/PrtlSttlmInd", indicator.as_bytes());
            }
            if let Some(hold) = self.hold_indicator {
                msg_set("SttlmParams/HldInd", bool_bytes(hold));
            }
            if let Some(mic) = &self.venue_mic {
                msg_set("PlcOfSttlm/MktId", mic.as_bytes());
            }
            msg_set("SctiesLeg/FinInstrmId", self.fin_instr_id.as_bytes());
            msg_set("SctiesLeg/Qty", self.quantity.as_bytes());
            msg_set("CashLeg/Amt", self.cash_amount.as_bytes());
            msg_set("CashLeg/Ccy", self.cash_currency.as_bytes());
            msg_set(
                "DlvrgSttlmPties/Pty/Bic",
                self.delivering_party_bic.as_bytes(),
            );
            msg_set("DlvrgSttlmPties/Acct", self.delivering_account.as_bytes());
            msg_set(
                "RcvgSttlmPties/Pty/Bic",
                self.receiving_party_bic.as_bytes(),
            );
            msg_set("RcvgSttlmPties/Acct", self.receiving_account.as_bytes());
            msg_set("Plan/ExecutionOrder", self.execution_order.as_bytes());
            msg_set("Plan/Atomicity", self.atomicity.as_bytes());
            for (idx, linkage) in self.linkages.iter().enumerate() {
                msg_add("Lnkgs/Lnkg");
                let prefix = format!("Lnkgs/Lnkg[{idx}]");
                msg_set(
                    format!("{prefix}/Tp/Cd").as_str(),
                    linkage.relation.as_bytes(),
                );
                msg_set(
                    format!("{prefix}/Ref/Prtry").as_str(),
                    linkage.reference.as_bytes(),
                );
            }
            if let Some(meta) = &self.securities_metadata {
                msg_set("SctiesLeg/Metadata", meta.as_bytes());
            }
            if let Some(meta) = &self.cash_metadata {
                msg_set("CashLeg/Metadata", meta.as_bytes());
            }
        }
        /// Build the Norito view from a parsed and validated message.
        pub fn from_parsed(parsed: &ParsedMessage) -> Result<Self, MsgError> {
            Ok(Self {
                tx_id: required_text(parsed, "TxId")?,
                settlement_date: required_text(parsed, "SttlmDt")?,
                movement_type: required_text(parsed, "SttlmTpAndAddtlParams/SctiesMvmntTp")?,
                payment_type: required_text(parsed, "SttlmTpAndAddtlParams/Pmt")?,
                fin_instr_id: required_text(parsed, "SctiesLeg/FinInstrmId")?,
                quantity: required_text(parsed, "SctiesLeg/Qty")?,
                cash_amount: required_text(parsed, "CashLeg/Amt")?,
                cash_currency: required_text(parsed, "CashLeg/Ccy")?,
                delivering_party_bic: required_text(parsed, "DlvrgSttlmPties/Pty/Bic")?,
                delivering_account: required_text(parsed, "DlvrgSttlmPties/Acct")?,
                receiving_party_bic: required_text(parsed, "RcvgSttlmPties/Pty/Bic")?,
                receiving_account: required_text(parsed, "RcvgSttlmPties/Acct")?,
                execution_order: required_text(parsed, "Plan/ExecutionOrder")?,
                atomicity: required_text(parsed, "Plan/Atomicity")?,
                settlement_condition: optional_text(parsed, "SttlmParams/SttlmTxCond/Cd"),
                partial_settlement_indicator: optional_text(parsed, "SttlmParams/PrtlSttlmInd"),
                hold_indicator: optional_bool(parsed, "SttlmParams/HldInd")?,
                venue_mic: optional_text(parsed, "PlcOfSttlm/MktId"),
                linkages: collect_linkages(parsed),
                securities_metadata: optional_text(parsed, "SctiesLeg/Metadata"),
                cash_metadata: optional_text(parsed, "CashLeg/Metadata"),
            })
        }
    }
    fn collect_linkages(parsed: &ParsedMessage) -> Vec<Linkage> {
        let mut linkages = Vec::new();
        let mut idx = 0usize;
        loop {
            let tp_field = format!("Lnkgs/Lnkg[{idx}]/Tp/Cd");
            let ref_field = format!("Lnkgs/Lnkg[{idx}]/Ref/Prtry");
            let Some(tp) = parsed.field_text(&tp_field) else {
                break;
            };
            let reference = parsed.field_text(&ref_field).unwrap_or_default().to_owned();
            linkages.push(Linkage {
                relation: tp.to_owned(),
                reference,
            });
            idx += 1;
        }
        linkages
    }
    /// Norito schema for `sese.025` PvP confirmations.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Sese025 {
        pub tx_id: String,
        pub settlement_date: String,
        pub movement_type: String,
        pub payment_type: String,
        pub confirmation_status: String,
        pub settlement_quantity: String,
        pub settlement_amount: String,
        pub settlement_currency: String,
        pub security_id: Option<String>,
        pub security_quantity: Option<String>,
        pub delivering_party_bic: Option<String>,
        pub delivering_account: Option<String>,
        pub receiving_party_bic: Option<String>,
        pub receiving_account: Option<String>,
        pub execution_order: String,
        pub atomicity: String,
        pub hold_indicator: Option<bool>,
        pub partial_settlement_indicator: Option<String>,
        pub settlement_condition: Option<String>,
        pub venue_mic: Option<String>,
        pub reason_code: Option<String>,
        pub additional_info: Option<String>,
    }
    impl Sese025 {
        /// Populate the VM stack with a confirmation message.
        pub fn apply_to_stack(&self) {
            msg_create("sese.025");
            msg_set("TxId", self.tx_id.as_bytes());
            msg_set("SttlmDt", self.settlement_date.as_bytes());
            msg_set(
                "SttlmTpAndAddtlParams/SctiesMvmntTp",
                self.movement_type.as_bytes(),
            );
            msg_set("SttlmTpAndAddtlParams/Pmt", self.payment_type.as_bytes());
            msg_set("ConfSts", self.confirmation_status.as_bytes());
            msg_set("SttlmQty", self.settlement_quantity.as_bytes());
            msg_set("SttlmAmt", self.settlement_amount.as_bytes());
            msg_set("SttlmCcy", self.settlement_currency.as_bytes());
            if let Some(id) = &self.security_id {
                msg_set("SctiesLeg/FinInstrmId", id.as_bytes());
            }
            if let Some(qty) = &self.security_quantity {
                msg_set("SctiesLeg/Qty", qty.as_bytes());
            }
            if let Some(bic) = &self.delivering_party_bic {
                msg_set("DlvrgSttlmPties/Pty/Bic", bic.as_bytes());
            }
            if let Some(acct) = &self.delivering_account {
                msg_set("DlvrgSttlmPties/Acct", acct.as_bytes());
            }
            if let Some(bic) = &self.receiving_party_bic {
                msg_set("RcvgSttlmPties/Pty/Bic", bic.as_bytes());
            }
            if let Some(acct) = &self.receiving_account {
                msg_set("RcvgSttlmPties/Acct", acct.as_bytes());
            }
            msg_set("Plan/ExecutionOrder", self.execution_order.as_bytes());
            msg_set("Plan/Atomicity", self.atomicity.as_bytes());
            if let Some(hold) = self.hold_indicator {
                msg_set("SttlmParams/HldInd", bool_bytes(hold));
            }
            if let Some(indicator) = &self.partial_settlement_indicator {
                msg_set("SttlmParams/PrtlSttlmInd", indicator.as_bytes());
            }
            if let Some(condition) = &self.settlement_condition {
                msg_set("SttlmParams/SttlmTxCond/Cd", condition.as_bytes());
            }
            if let Some(mic) = &self.venue_mic {
                msg_set("PlcOfSttlm/MktId", mic.as_bytes());
            }
            if let Some(reason) = &self.reason_code {
                msg_set("RsnCd", reason.as_bytes());
            }
            if let Some(info) = &self.additional_info {
                msg_set("AddtlInf", info.as_bytes());
            }
        }
        /// Convert a parsed message into the Norito struct.
        pub fn from_parsed(parsed: &ParsedMessage) -> Result<Self, MsgError> {
            Ok(Self {
                tx_id: required_text(parsed, "TxId")?,
                settlement_date: required_text(parsed, "SttlmDt")?,
                movement_type: required_text(parsed, "SttlmTpAndAddtlParams/SctiesMvmntTp")?,
                payment_type: required_text(parsed, "SttlmTpAndAddtlParams/Pmt")?,
                confirmation_status: required_text(parsed, "ConfSts")?,
                settlement_quantity: required_text(parsed, "SttlmQty")?,
                settlement_amount: required_text(parsed, "SttlmAmt")?,
                settlement_currency: required_text(parsed, "SttlmCcy")?,
                security_id: optional_text(parsed, "SctiesLeg/FinInstrmId"),
                security_quantity: optional_text(parsed, "SctiesLeg/Qty"),
                delivering_party_bic: optional_text(parsed, "DlvrgSttlmPties/Pty/Bic"),
                delivering_account: optional_text(parsed, "DlvrgSttlmPties/Acct"),
                receiving_party_bic: optional_text(parsed, "RcvgSttlmPties/Pty/Bic"),
                receiving_account: optional_text(parsed, "RcvgSttlmPties/Acct"),
                execution_order: required_text(parsed, "Plan/ExecutionOrder")?,
                atomicity: required_text(parsed, "Plan/Atomicity")?,
                hold_indicator: optional_bool(parsed, "SttlmParams/HldInd")?,
                partial_settlement_indicator: optional_text(parsed, "SttlmParams/PrtlSttlmInd"),
                settlement_condition: optional_text(parsed, "SttlmParams/SttlmTxCond/Cd"),
                venue_mic: optional_text(parsed, "PlcOfSttlm/MktId"),
                reason_code: optional_text(parsed, "RsnCd"),
                additional_info: optional_text(parsed, "AddtlInf"),
            })
        }
    }
    /// Norito schema for collateral substitution confirmations.
    #[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
    pub struct Colr012 {
        pub tx_id: String,
        pub obligation_id: String,
        pub original_amount: String,
        pub original_currency: String,
        pub substitute_amount: String,
        pub substitute_currency: String,
        pub haircut: Option<String>,
        pub effective_date: String,
        pub substitution_type: String,
        pub original_fin_instr_id: Option<String>,
        pub substitute_fin_instr_id: Option<String>,
        pub reason_code: Option<String>,
    }
    impl Colr012 {
        /// Populate the VM stack with a substitution confirmation.
        pub fn apply_to_stack(&self) {
            msg_create("colr.012");
            msg_set("TxId", self.tx_id.as_bytes());
            msg_set("OblgtnId", self.obligation_id.as_bytes());
            msg_set("Substitution/OriginalAmt", self.original_amount.as_bytes());
            msg_set(
                "Substitution/OriginalCcy",
                self.original_currency.as_bytes(),
            );
            msg_set(
                "Substitution/SubstituteAmt",
                self.substitute_amount.as_bytes(),
            );
            msg_set(
                "Substitution/SubstituteCcy",
                self.substitute_currency.as_bytes(),
            );
            if let Some(haircut) = &self.haircut {
                msg_set("Substitution/Haircut", haircut.as_bytes());
            }
            msg_set("Substitution/EffectiveDt", self.effective_date.as_bytes());
            msg_set("Substitution/Type", self.substitution_type.as_bytes());
            if let Some(id) = &self.original_fin_instr_id {
                msg_set("Substitution/OriginalFinInstrmId", id.as_bytes());
            }
            if let Some(id) = &self.substitute_fin_instr_id {
                msg_set("Substitution/SubstituteFinInstrmId", id.as_bytes());
            }
            if let Some(reason) = &self.reason_code {
                msg_set("Substitution/ReasonCd", reason.as_bytes());
            }
        }
        /// Convert a parsed message into the Norito struct.
        pub fn from_parsed(parsed: &ParsedMessage) -> Result<Self, MsgError> {
            Ok(Self {
                tx_id: required_text(parsed, "TxId")?,
                obligation_id: required_text(parsed, "OblgtnId")?,
                original_amount: required_text(parsed, "Substitution/OriginalAmt")?,
                original_currency: required_text(parsed, "Substitution/OriginalCcy")?,
                substitute_amount: required_text(parsed, "Substitution/SubstituteAmt")?,
                substitute_currency: required_text(parsed, "Substitution/SubstituteCcy")?,
                haircut: optional_text(parsed, "Substitution/Haircut"),
                effective_date: required_text(parsed, "Substitution/EffectiveDt")?,
                substitution_type: required_text(parsed, "Substitution/Type")?,
                original_fin_instr_id: optional_text(parsed, "Substitution/OriginalFinInstrmId"),
                substitute_fin_instr_id: optional_text(
                    parsed,
                    "Substitution/SubstituteFinInstrmId",
                ),
                reason_code: optional_text(parsed, "Substitution/ReasonCd"),
            })
        }
    }
}
fn parse_index(segment: &str) -> Option<(&str, usize)> {
    let (name, rest) = segment.split_once('[')?;
    let idx_str = rest.strip_suffix(']')?;
    if idx_str.is_empty() {
        return None;
    }
    Some((name, idx_str.parse().ok()?))
}
fn pattern_matches(pattern: &str, field: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.split('/').collect();
    let field_parts: Vec<&str> = field.split('/').collect();
    if pattern_parts.len() != field_parts.len() {
        return false;
    }
    pattern_parts
        .iter()
        .zip(field_parts.iter())
        .all(|(pat, actual)| {
            if let Some(base) = pat.strip_suffix("[*]") {
                parse_index(actual).is_some_and(|(name, _)| name == base)
            } else {
                pat == actual
            }
        })
}
fn repeating_base(pattern: &'static str) -> Option<&'static str> {
    let idx = pattern.find("[*]")?;
    let base = &pattern[..idx];
    Some(base.strip_suffix('/').unwrap_or(base))
}
fn resolve_alias(schema: &MessageSchema, field: &str) -> Option<String> {
    if schema
        .field_specs()
        .iter()
        .any(|spec| pattern_matches(spec.pattern, field))
    {
        return Some(field.to_owned());
    }
    for alias in schema.aliases() {
        if alias.alias == field {
            return Some(alias.canonical.to_owned());
        }
        if pattern_matches(alias.alias, field) {
            let field_parts: Vec<&str> = field.split('/').collect();
            let alias_parts: Vec<&str> = alias.alias.split('/').collect();
            let filtered_parts: Vec<&str> = field_parts
                .iter()
                .copied()
                .filter(|part| !part.starts_with('@'))
                .collect();
            let canonical_parts: Vec<&str> = alias.canonical.split('/').collect();
            if canonical_parts.len() <= filtered_parts.len() {
                let offset = filtered_parts.len().saturating_sub(canonical_parts.len());
                let mut out = Vec::with_capacity(canonical_parts.len());
                for (i, canon) in canonical_parts.iter().enumerate() {
                    if let Some(base_for_index) = canon.strip_suffix("[*]") {
                        let field_part = alias_parts
                            .iter()
                            .position(|part| part.trim_end_matches("[*]") == base_for_index)
                            .and_then(|idx| filtered_parts.get(idx))
                            .or_else(|| filtered_parts.get(offset + i));
                        if let Some(field_part) = field_part
                            && let Some((_, idx)) = parse_index(field_part)
                        {
                            out.push(format!("{base_for_index}[{idx}]"));
                        } else {
                            out.push(base_for_index.to_owned());
                        }
                    } else {
                        out.push((*canon).to_owned());
                    }
                }
                return Some(out.join("/"));
            }
        }
        if alias.alias.ends_with("[*]") {
            let base = alias.alias.trim_end_matches("[*]");
            if let Some(rest) = field.strip_prefix(base) {
                let canonical_base = alias.canonical.trim_end_matches("[*]");
                return Some(format!("{canonical_base}{rest}"));
            }
        }
    }
    None
}
fn canonical_field_name(message_type: &str, field: &str) -> String {
    schema_for(message_type)
        .and_then(|schema| resolve_alias(schema, field))
        .unwrap_or_else(|| field.to_owned())
}
fn canonical_repeating_base(message_type: &str, base: &str) -> String {
    if let Some(schema) = schema_for(message_type) {
        if let Some(resolved) = resolve_alias(schema, base) {
            return resolved;
        }
        if schema
            .field_specs()
            .iter()
            .filter_map(|spec| repeating_base(spec.pattern))
            .any(|candidate| candidate == base)
        {
            return base.to_owned();
        }
    }
    base.to_owned()
}
#[derive(Clone, Copy)]
struct IbanSpec {
    code: [u8; 2],
    length: u8,
}
/// Source: IBAN Registry (June 2024). Keep alphabetically sorted so
/// `iban_length_for_country` can binary-search deterministically.
const IBAN_SPEC_BYTES: &[u8; 234] = include_bytes!("assets/iso20022_iban_specs_v1.bin");
const fn decode_iban_specs(bytes: &[u8; 234]) -> [IbanSpec; 78] {
    let mut specs = [IbanSpec {
        code: [0_u8; 2],
        length: 0,
    }; 78];
    let mut index = 0;
    while index < 78 {
        let offset = index * 3;
        specs[index] = IbanSpec {
            code: [bytes[offset], bytes[offset + 1]],
            length: bytes[offset + 2],
        };
        index += 1;
    }
    specs
}
const IBAN_SPEC_VALUES: [IbanSpec; 78] = decode_iban_specs(IBAN_SPEC_BYTES);
const IBAN_SPECS: &[IbanSpec] = &IBAN_SPEC_VALUES;
fn iban_length_for_country(code: [u8; 2]) -> Option<usize> {
    IBAN_SPECS
        .binary_search_by(|spec| spec.code.cmp(&code))
        .ok()
        .map(|idx| IBAN_SPECS[idx].length as usize)
}
/// Validate an IBAN using the ISO 7064 mod 97-10 algorithm with
/// country-specific length checks and digit validation for the check byte pair.
fn validate_iban(value: &[u8]) -> bool {
    if value.len() < 4 {
        return false;
    }
    let mut normalized = Vec::with_capacity(value.len());
    for &byte in value {
        match byte {
            b'0'..=b'9' => normalized.push(byte),
            b'A'..=b'Z' => normalized.push(byte),
            b'a'..=b'z' => normalized.push(byte.to_ascii_uppercase()),
            _ => return false,
        }
    }
    if normalized.len() < 4 {
        return false;
    }
    let country = [normalized[0], normalized[1]];
    let expected_len = match iban_length_for_country(country) {
        Some(len) => len,
        None => return false,
    };
    if normalized.len() != expected_len {
        return false;
    }
    if !normalized[2].is_ascii_digit() || !normalized[3].is_ascii_digit() {
        return false;
    }
    // Rotate the country code and check digits to the end before running mod 97.
    normalized.rotate_left(4);
    let mut acc: u32 = 0;
    for byte in normalized {
        match byte {
            b'0'..=b'9' => acc = (acc * 10 + u32::from(byte - b'0')) % 97,
            b'A'..=b'Z' => acc = (acc * 100 + u32::from(byte - b'A' + 10)) % 97,
            _ => return false,
        }
    }
    acc == 1
}
/// Validate a BIC. The check is deliberately lightweight: it enforces
/// an uppercase alphanumeric string of length 8 or 11 as per ISO 9362 but
/// does not verify country codes or institution existence.
fn validate_bic(value: &[u8]) -> bool {
    core::str::from_utf8(value)
        .map(validate_bic_str)
        .unwrap_or(false)
}
fn validate_bic_str(value: &str) -> bool {
    let bytes = value.as_bytes();
    let len = bytes.len();
    if !(len == 8 || len == 11) {
        return false;
    }
    for &b in bytes {
        if !b.is_ascii_alphanumeric() {
            return false;
        }
    }
    if !bytes[..4].iter().all(|&b| b.is_ascii_uppercase()) {
        return false;
    }
    if !bytes[4..6].iter().all(|&b| b.is_ascii_uppercase()) {
        return false;
    }
    if !bytes[6..8]
        .iter()
        .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    if len == 11
        && !bytes[8..11]
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    true
}
fn validate_amount(value: &[u8]) -> bool {
    if value.is_empty() {
        return false;
    }
    let mut digits = 0;
    let mut dot = 0;
    for b in value {
        match b {
            b'0'..=b'9' => digits += 1,
            b'.' => {
                dot += 1;
                if dot > 1 {
                    return false;
                }
            }
            _ => return false,
        }
    }
    if digits == 0 {
        return false;
    }
    if dot == 1 {
        let s = match core::str::from_utf8(value) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let mut parts = s.split('.');
        let whole = parts.next().unwrap_or("");
        let frac = parts.next().unwrap_or("");
        if parts.next().is_some() {
            return false;
        }
        if whole.is_empty() {
            return false;
        }
        frac.len() <= 5
    } else {
        true
    }
}
const VALID_CURRENCY_CODES: &[&str] = &[
    "AED", "AFN", "ALL", "AMD", "ANG", "AOA", "ARS", "AUD", "AWG", "AZN", "BAM", "BBD", "BDT",
    "BGN", "BHD", "BIF", "BMD", "BND", "BOB", "BOV", "BRL", "BSD", "BTN", "BWP", "BYN", "BZD",
    "CAD", "CDF", "CHE", "CHF", "CHW", "CLF", "CLP", "CNY", "COP", "COU", "CRC", "CUC", "CUP",
    "CVE", "CZK", "DJF", "DKK", "DOP", "DZD", "EGP", "ERN", "ETB", "EUR", "FJD", "FKP", "GBP",
    "GEL", "GHS", "GIP", "GMD", "GNF", "GTQ", "GYD", "HKD", "HNL", "HRK", "HTG", "HUF", "IDR",
    "ILS", "INR", "IQD", "IRR", "ISK", "JMD", "JOD", "JPY", "KES", "KGS", "KHR", "KMF", "KPW",
    "KRW", "KWD", "KYD", "KZT", "LAK", "LBP", "LKR", "LRD", "LSL", "LYD", "MAD", "MDL", "MGA",
    "MKD", "MMK", "MNT", "MOP", "MRU", "MUR", "MVR", "MWK", "MXN", "MXV", "MYR", "MZN", "NAD",
    "NGN", "NIO", "NOK", "NPR", "NZD", "OMR", "PAB", "PEN", "PGK", "PHP", "PKR", "PLN", "PYG",
    "QAR", "RON", "RSD", "RUB", "RWF", "SAR", "SBD", "SCR", "SDG", "SEK", "SGD", "SHP", "SLL",
    "SOS", "SRD", "SSP", "STN", "SVC", "SYP", "SZL", "THB", "TJS", "TMT", "TND", "TOP", "TRY",
    "TTD", "TWD", "TZS", "UAH", "UGX", "USD", "USN", "UYI", "UYU", "UYW", "UZS", "VED", "VES",
    "VND", "VUV", "WST", "XAF", "XAG", "XAU", "XBA", "XBB", "XBC", "XBD", "XCD", "XDR", "XOF",
    "XPD", "XPF", "XPT", "XSU", "XTS", "XUA", "XXX", "YER", "ZAR", "ZMW", "ZWL",
];
fn validate_currency_str(value: &str) -> bool {
    if value.len() != 3 {
        return false;
    }
    if !value.chars().all(|c| c.is_ascii_uppercase()) {
        return false;
    }
    VALID_CURRENCY_CODES.binary_search(&value).is_ok()
}
fn validate_mic_str(value: &str) -> bool {
    if value.len() != 4 {
        return false;
    }
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() || !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}
fn luhn_sum_from_digits(digits: impl DoubleEndedIterator<Item = u32>) -> u32 {
    let mut sum = 0;
    let mut double = true;
    for mut value in digits {
        if double {
            value *= 2;
            if value >= 10 {
                sum += value / 10 + value % 10;
            } else {
                sum += value;
            }
        } else {
            sum += value;
        }
        double = !double;
    }
    sum
}
fn validate_isin_str(value: &str) -> bool {
    if value.len() != 12 {
        return false;
    }
    if !value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if value.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let mut digits = Vec::with_capacity(24);
    for ch in value.chars() {
        if let Some(d) = ch.to_digit(10) {
            digits.push(d);
        } else if ch.is_ascii_uppercase() {
            let mapped = 10 + (ch as u32 - 'A' as u32);
            if (10..36).contains(&mapped) {
                if mapped >= 20 {
                    digits.push(mapped / 10);
                } else {
                    digits.push(1);
                }
                digits.push(mapped % 10);
            } else {
                return false;
            }
        } else {
            return false;
        }
    }
    let check = digits.pop().unwrap_or(0);
    let sum = luhn_sum_from_digits(digits.into_iter().rev());
    (sum + check) % 10 == 0
}
fn cusip_char_value(ch: char) -> Option<u32> {
    match ch {
        '0'..='9' => Some(ch as u32 - '0' as u32),
        'A'..='Z' => Some(ch as u32 - 'A' as u32 + 10),
        '*' => Some(36),
        '@' => Some(37),
        '#' => Some(38),
        _ => None,
    }
}
fn validate_cusip_str(value: &str) -> bool {
    if value.len() != 9 {
        return false;
    }
    if value.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let value = value.to_ascii_uppercase();
    let mut sum = 0u32;
    for (idx, ch) in value.chars().take(8).enumerate() {
        let mut val = match cusip_char_value(ch) {
            Some(v) => v,
            None => return false,
        };
        if idx % 2 == 1 {
            val *= 2;
        }
        sum += val / 10 + val % 10;
    }
    let check_char = value.chars().nth(8).unwrap_or('0');
    let check_digit = match check_char.to_digit(10) {
        Some(d) => d,
        None => return false,
    };
    (sum + check_digit).is_multiple_of(10)
}
fn validate_lei_str(value: &str) -> bool {
    if value.len() != 20 {
        return false;
    }
    if !value.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if value.chars().any(|c| c.is_ascii_lowercase()) {
        return false;
    }
    let upper = value.to_ascii_uppercase();
    let mut remainder: u32 = 0;
    for ch in upper.chars() {
        if let Some(d) = ch.to_digit(10) {
            remainder = (remainder * 10 + d) % 97;
        } else if ch.is_ascii_uppercase() {
            let mapped = 10 + (ch as u32 - 'A' as u32);
            remainder = (remainder * 100 + mapped) % 97;
        } else {
            return false;
        }
    }
    remainder == 1
}
pub fn validate_identifier(kind: IdentifierKind, value: &str) -> bool {
    match kind {
        IdentifierKind::Isin => validate_isin_str(value),
        IdentifierKind::Cusip => validate_cusip_str(value),
        IdentifierKind::Lei => validate_lei_str(value),
        IdentifierKind::Bic => validate_bic_str(value),
        IdentifierKind::Mic => validate_mic_str(value),
        IdentifierKind::Iban => validate_iban(value.as_bytes()),
        IdentifierKind::Currency => validate_currency_str(value),
    }
}
pub fn validate_instrument_identifier(value: &str) -> bool {
    validate_identifier(IdentifierKind::Isin, value)
        || validate_identifier(IdentifierKind::Cusip, value)
}
fn validate_numeric(value: &[u8]) -> bool {
    !value.is_empty() && value.iter().all(|b| b.is_ascii_digit())
}
fn parse_ascii_u32(slice: &[u8]) -> Option<u32> {
    if slice.is_empty() {
        return None;
    }
    let mut acc = 0u32;
    for &b in slice {
        if !b.is_ascii_digit() {
            return None;
        }
        acc = acc * 10 + u32::from(b - b'0');
    }
    Some(acc)
}
fn validate_date(value: &[u8]) -> bool {
    if value.len() != 10 {
        return false;
    }
    if value[4] != b'-' || value[7] != b'-' {
        return false;
    }
    let year = match parse_ascii_u32(&value[0..4]) {
        Some(v) => v,
        None => return false,
    };
    let month = match parse_ascii_u32(&value[5..7]) {
        Some(v) => v,
        None => return false,
    };
    let day = match parse_ascii_u32(&value[8..10]) {
        Some(v) => v,
        None => return false,
    };
    if !(1..=12).contains(&month) || day == 0 {
        return false;
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
            if leap { 29 } else { 28 }
        }
        _ => return false,
    };
    day <= max_day
}
fn validate_offset(offset: &str) -> bool {
    if offset.len() != 6 {
        return false;
    }
    let mut chars = offset.chars();
    let sign = chars.next().unwrap_or('+');
    if sign != '+' && sign != '-' {
        return false;
    }
    let hour_tens = chars.next().and_then(|c| c.to_digit(10));
    let hour_ones = chars.next().and_then(|c| c.to_digit(10));
    if chars.next() != Some(':') {
        return false;
    }
    let min_tens = chars.next().and_then(|c| c.to_digit(10));
    let min_ones = chars.next().and_then(|c| c.to_digit(10));
    if chars.next().is_some() {
        return false;
    }
    let hour = match (hour_tens, hour_ones) {
        (Some(t), Some(o)) => t * 10 + o,
        _ => return false,
    };
    let minute = match (min_tens, min_ones) {
        (Some(t), Some(o)) => t * 10 + o,
        _ => return false,
    };
    hour <= 23 && minute <= 59
}
fn validate_datetime(value: &[u8]) -> bool {
    let s = match core::str::from_utf8(value) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let (date_part, rest) = match s.split_once('T') {
        Some(parts) => parts,
        None => return false,
    };
    if !validate_date(date_part.as_bytes()) {
        return false;
    }
    let (time_part, tz_part) = if let Some(v) = rest.strip_suffix('Z') {
        (v, None)
    } else if let Some(pos) = rest.rfind(['+', '-']) {
        (rest[..pos].trim_end_matches('Z'), Some(&rest[pos..]))
    } else {
        (rest, None)
    };
    let mut pieces = time_part.split(':');
    let hour = match pieces.next() {
        Some(v) if v.len() == 2 => match v.parse::<u32>() {
            Ok(v) => v,
            Err(_) => return false,
        },
        _ => return false,
    };
    let minute = match pieces.next() {
        Some(v) if v.len() == 2 => match v.parse::<u32>() {
            Ok(v) => v,
            Err(_) => return false,
        },
        _ => return false,
    };
    let sec_fragment = match pieces.next() {
        Some(v) => v,
        None => return false,
    };
    if pieces.next().is_some() {
        return false;
    }
    let (second_str, fraction_ok) = if let Some((sec, frac)) = sec_fragment.split_once('.') {
        (
            sec,
            !frac.is_empty() && frac.len() <= 6 && frac.chars().all(|c| c.is_ascii_digit()),
        )
    } else {
        (sec_fragment, true)
    };
    if !fraction_ok || second_str.len() != 2 {
        return false;
    }
    let second = match second_str.parse::<u32>() {
        Ok(v) => v,
        Err(_) => return false,
    };
    let tz_ok = if let Some(offset) = tz_part {
        validate_offset(offset)
    } else {
        true
    };
    tz_ok && hour <= 23 && minute <= 59 && second <= 60
}
fn validate_identifier_bytes(kind: IdentifierKind, value: &[u8]) -> Result<(), InvalidReason> {
    let text = core::str::from_utf8(value).map_err(|_| InvalidReason::Utf8)?;
    if validate_identifier(kind, text) {
        Ok(())
    } else {
        Err(InvalidReason::Identifier(kind))
    }
}
fn validate_instrument_bytes(value: &[u8]) -> Result<(), InvalidReason> {
    let text = core::str::from_utf8(value).map_err(|_| InvalidReason::Utf8)?;
    if validate_identifier(IdentifierKind::Isin, text)
        || validate_identifier(IdentifierKind::Cusip, text)
    {
        Ok(())
    } else {
        Err(InvalidReason::Instrument)
    }
}
fn validate_field(kind: FieldKind, value: &[u8]) -> Result<(), InvalidReason> {
    match kind {
        FieldKind::Text => {
            if value.is_empty() {
                Err(InvalidReason::Empty)
            } else {
                Ok(())
            }
        }
        FieldKind::Numeric => {
            if validate_numeric(value) {
                Ok(())
            } else {
                Err(InvalidReason::Numeric)
            }
        }
        FieldKind::Amount => {
            if validate_amount(value) {
                Ok(())
            } else {
                Err(InvalidReason::Amount)
            }
        }
        FieldKind::Identifier(kind) => validate_identifier_bytes(kind, value),
        FieldKind::Instrument => validate_instrument_bytes(value),
        FieldKind::Date => {
            if validate_date(value) {
                Ok(())
            } else {
                Err(InvalidReason::Date)
            }
        }
        FieldKind::DateTime => {
            if validate_datetime(value) {
                Ok(())
            } else {
                Err(InvalidReason::DateTime)
            }
        }
        FieldKind::Enum(options) => {
            if options.iter().any(|opt| opt.as_bytes() == value) {
                Ok(())
            } else {
                Err(InvalidReason::Enum)
            }
        }
    }
}
fn proxy_fallback_match<'a>(
    pattern: &str,
    message: &'a IsoMessage,
) -> Option<(&'a String, &'a Vec<u8>, FieldKind)> {
    match pattern {
        "DbtrAcct" => message
            .fields
            .get_key_value("DbtrAcct/Prxy/Id")
            .map(|(field, value)| (field, value, FieldKind::Text)),
        "CdtrAcct" => message
            .fields
            .get_key_value("CdtrAcct/Prxy/Id")
            .map(|(field, value)| (field, value, FieldKind::Text)),
        "CreDtTm" if canonical_message_type(&message.message_type).as_ref() == "pacs.009" => {
            message
                .fields
                .get_key_value("AppHdr/CreDt")
                .map(|(field, value)| (field, value, FieldKind::DateTime))
        }
        _ => None,
    }
}
fn validate_message_against_schema(
    message: &IsoMessage,
    schema: &MessageSchema,
) -> Result<(), ValidationFailure> {
    for spec in schema.field_specs() {
        let mut matches: Vec<(&String, &Vec<u8>, FieldKind)> = message
            .fields
            .iter()
            .filter(|(field, _)| pattern_matches(spec.pattern, field))
            .map(|(field, value)| (field, value, spec.kind))
            .collect();
        if matches.is_empty()
            && let Some(fallback) = proxy_fallback_match(spec.pattern, message)
        {
            matches.push(fallback);
        }
        if matches.is_empty() && matches!(spec.requirement, Requirement::Required) {
            return Err(ValidationFailure::MissingField(spec.pattern));
        }
        if let Some(max) = spec.max_occurs
            && matches.len() > max
        {
            return Err(ValidationFailure::TooManyOccurrences {
                field: spec.pattern,
                max,
                actual: matches.len(),
            });
        }
        for (field, value, kind) in matches {
            if let Err(reason) = validate_field(kind, value) {
                return Err(ValidationFailure::InvalidField {
                    field: field.clone(),
                    reason,
                });
            }
        }
    }
    Ok(())
}
fn collect_fields_in_order<'a>(
    message: &'a IsoMessage,
    schema: Option<&'static MessageSchema>,
) -> Vec<(&'a String, &'a Vec<u8>, usize)> {
    let mut pairs: Vec<(&String, &Vec<u8>, usize)> = message
        .fields
        .iter()
        .map(|(key, value)| {
            let order = schema
                .and_then(|s| {
                    s.field_specs()
                        .iter()
                        .position(|spec| pattern_matches(spec.pattern, key))
                })
                .unwrap_or(usize::MAX);
            (key, value, order)
        })
        .collect();
    pairs.sort_by(|a, b| a.2.cmp(&b.2).then_with(|| a.0.cmp(b.0)));
    pairs
}
fn escape_xml_text(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
fn escape_xml_attr(input: &str) -> String {
    escape_xml_text(input)
}
fn serialize_key_value(message: &IsoMessage, schema: Option<&'static MessageSchema>) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, (key, value, _)) in collect_fields_in_order(message, schema)
        .into_iter()
        .enumerate()
    {
        if i != 0 {
            out.push(b'\n');
        }
        out.extend_from_slice(key.as_bytes());
        out.push(b'=');
        out.extend_from_slice(value);
    }
    out
}
fn serialize_xml(message: &IsoMessage, schema: Option<&'static MessageSchema>) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = write!(out, "<ISO20022 message=\"{}\">", message.message_type);
    for (key, value, _) in collect_fields_in_order(message, schema) {
        let path = escape_xml_attr(key);
        if let Ok(text) = core::str::from_utf8(value)
            && contains_only_xml_characters(text)
        {
            let escaped = escape_xml_text(text);
            let _ = write!(out, "<Field path=\"{path}\">{escaped}</Field>");
            continue;
        }
        let encoded = encode_base64(value);
        let encoded_str = String::from_utf8(encoded).unwrap_or_default();
        let _ = write!(
            out,
            "<Field path=\"{path}\" encoding=\"base64\">{encoded_str}</Field>"
        );
    }
    out.extend_from_slice(b"</ISO20022>");
    out
}
fn looks_like_xml(data: &[u8]) -> bool {
    data.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'<')
}
fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}
const ISO_20022_XSD_NAMESPACE_PREFIX: &str = "urn:iso:std:iso:20022:tech:xsd:";
fn message_type_from_namespace(ns: &str) -> Option<String> {
    ns.strip_prefix(ISO_20022_XSD_NAMESPACE_PREFIX)
        .filter(|message_type| !message_type.is_empty())
        .map(str::to_owned)
}
fn namespace_bindings(attrs: &[(String, String)]) -> Vec<(String, String)> {
    attrs
        .iter()
        .filter_map(|(name, value)| {
            if name == "xmlns" {
                Some(("".to_owned(), value.to_owned()))
            } else {
                name.strip_prefix("xmlns:")
                    .map(|prefix| (prefix.to_owned(), value.to_owned()))
            }
        })
        .collect()
}
fn namespace_uri_for_prefix<'a>(
    prefix: &str,
    attrs: &'a [(String, String)],
    namespace_scopes: &'a [Vec<(String, String)>],
) -> Option<&'a str> {
    if prefix == "xml" {
        return Some("http://www.w3.org/XML/1998/namespace");
    }
    attrs
        .iter()
        .rev()
        .find_map(|(name, value)| {
            if prefix.is_empty() && name == "xmlns" {
                Some(value.as_str())
            } else if let Some(bound_prefix) = name.strip_prefix("xmlns:") {
                (bound_prefix == prefix).then_some(value.as_str())
            } else {
                None
            }
        })
        .or_else(|| {
            namespace_scopes.iter().rev().find_map(|scope| {
                scope.iter().rev().find_map(|(bound_prefix, value)| {
                    (bound_prefix == prefix).then_some(value.as_str())
                })
            })
        })
}
fn element_namespace_uri<'a>(
    name: &str,
    attrs: &'a [(String, String)],
    namespace_scopes: &'a [Vec<(String, String)>],
) -> Result<Option<&'a str>, MsgError> {
    if let Some((prefix, local)) = name.split_once(':') {
        if prefix.is_empty() || local.is_empty() {
            return Err(MsgError::InvalidFormat);
        }
        return namespace_uri_for_prefix(prefix, attrs, namespace_scopes)
            .map(Some)
            .ok_or(MsgError::InvalidFormat);
    }
    Ok(namespace_uri_for_prefix("", attrs, namespace_scopes))
}
fn is_versioned_message_definition_id(message_type: &str) -> bool {
    let mut parts = message_type.split('.');
    let (Some(_business_area), Some(number), Some(variant), Some(version)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    parts.next().is_none()
        && number.len() == 3
        && variant.len() == 3
        && version.len() == 2
        && number.chars().all(|c| c.is_ascii_digit())
        && variant.chars().all(|c| c.is_ascii_digit())
        && version.chars().all(|c| c.is_ascii_digit())
}
fn declared_message_definitions_match(first: &str, second: &str) -> bool {
    if is_versioned_message_definition_id(first) || is_versioned_message_definition_id(second) {
        first.eq_ignore_ascii_case(second)
    } else {
        canonical_message_type(first)
            .as_ref()
            .eq_ignore_ascii_case(canonical_message_type(second).as_ref())
    }
}
fn requested_message_matches_declaration(requested: &str, declared: &str) -> bool {
    if is_versioned_message_definition_id(requested) {
        requested.eq_ignore_ascii_case(declared)
    } else {
        canonical_message_type(requested)
            .as_ref()
            .eq_ignore_ascii_case(canonical_message_type(declared).as_ref())
    }
}
fn observe_declared_message_type(
    declared_message_type: &mut Option<String>,
    candidate: &str,
) -> Result<(), MsgError> {
    let candidate = candidate.trim();
    if candidate.is_empty() {
        return Err(MsgError::InvalidFormat);
    }
    if let Some(declared) = declared_message_type.as_deref() {
        if !declared_message_definitions_match(declared, candidate) {
            return Err(MsgError::UnknownMessageType);
        }
    } else {
        *declared_message_type = Some(candidate.to_owned());
    }
    Ok(())
}
fn document_root_matches_message(message_type: &str, root: &str) -> Option<bool> {
    Some(match canonical_message_type(message_type).as_ref() {
        "colr.012" => root == "CollSbstitnConf",
        "pacs.002" => root == "FIToFIPmtStsRpt",
        "pacs.004" => root == "PmtRtr",
        "pacs.007" => root == "FIToFIPmtRvsl",
        "pacs.008" => root == "FIToFICstmrCdtTrf",
        "pacs.009" => root == "FICdtTrf",
        "pacs.028" => root == "FIToFIPmtStsReq",
        "pacs.029" => root == "RsltnOfInvstgtn",
        "pain.001" => root == "CstmrCdtTrfInitn",
        "pain.002" => root == "CstmrPmtStsRpt",
        "camt.029" => root == "RsltnOfInvstgtn",
        "camt.052" => root == "BkToCstmrAcctRpt",
        "camt.053" => root == "BkToCstmrStmt",
        "camt.054" => root == "BkToCstmrDbtCdtNtfctn",
        "camt.056" => root == "FIToFIPmtCxlReq",
        "sese.023" => root == "SctiesSttlmTxInstr",
        "sese.024" => root == "SctiesSttlmTxStsAdvc",
        "sese.025" => root == "SctiesSttlmTxConf",
        _ => return None,
    })
}
fn message_type_requires_document_root(message_type: &str) -> bool {
    document_root_matches_message(message_type, "").is_some()
}
fn repeating_bases_for(message_type: &str) -> Vec<String> {
    schema_for(message_type)
        .map(|schema| {
            schema
                .field_specs()
                .iter()
                .filter_map(|spec| repeating_base(spec.pattern))
                .map(|base| base.to_owned())
                .collect()
        })
        .unwrap_or_default()
}
fn should_index(path: &str, repeating_bases: &[String]) -> bool {
    repeating_bases.iter().any(|base| path.ends_with(base))
}
const SIGNATURE_IGNORED_VALUE: &[u8] = b"signature-block-ignored";
const XMLDSIG_NAMESPACE: &str = "http://www.w3.org/2000/09/xmldsig#";
const REAL_XML_MAX_DEPTH: usize = 64;
const REAL_XML_MAX_ELEMENTS: usize = 16_384;
const REAL_XML_MAX_ATTRIBUTES_PER_ELEMENT: usize = 64;
const REAL_XML_MAX_ATTRIBUTES: usize = 65_536;
const REAL_XML_MAX_PATH_BYTES: usize = 4_096;
const REAL_XML_MAX_FIELDS: usize = 16_384;
fn normalised_parts(stack: &[String]) -> Vec<String> {
    stack
        .iter()
        .map(|s| s.as_str())
        .filter(|s| {
            let name = local_name(s);
            !matches!(name, "DataPDU" | "DataEnvelope" | "Body")
        })
        .map(|s| local_name(s).to_owned())
        .collect()
}
fn current_path(stack: &[String]) -> Option<String> {
    let parts = normalised_parts(stack);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}
fn find_tag_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut in_quote = None;
    while i < bytes.len() {
        let b = bytes[i];
        match in_quote {
            Some(q) if b == q => in_quote = None,
            None if b == b'"' || b == b'\'' => in_quote = Some(b),
            None if b == b'>' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}
fn supported_xml_comment_end(text: &str, start: usize) -> Result<usize, MsgError> {
    let Some(body_start) = text[start..].strip_prefix("<!--").map(|_| start + 4) else {
        return Err(MsgError::InvalidFormat);
    };
    let Some(comment_end) = text[body_start..]
        .find("-->")
        .map(|offset| body_start + offset)
    else {
        return Err(MsgError::InvalidFormat);
    };
    let body = &text[body_start..comment_end];
    if body.contains("--") || body.ends_with('-') {
        return Err(MsgError::InvalidFormat);
    }
    Ok(comment_end + 3)
}
fn supported_processing_instruction_end(text: &str, start: usize) -> Result<usize, MsgError> {
    let Some(body_start) = text[start..].strip_prefix("<?").map(|_| start + 2) else {
        return Err(MsgError::InvalidFormat);
    };
    let Some(pi_end) = text[body_start..]
        .find("?>")
        .map(|offset| body_start + offset)
    else {
        return Err(MsgError::InvalidFormat);
    };
    let body = &text[body_start..pi_end];
    if body.is_empty() || body.chars().next().is_some_and(char::is_whitespace) {
        return Err(MsgError::InvalidFormat);
    }
    let target = body
        .split(char::is_whitespace)
        .next()
        .ok_or(MsgError::InvalidFormat)?;
    if !is_supported_xml_name(target) {
        return Err(MsgError::InvalidFormat);
    }
    Ok(pi_end + 2)
}
fn supported_special_xml_markup_end(text: &str, start: usize) -> Result<Option<usize>, MsgError> {
    if text[start..].starts_with("<!--") {
        return supported_xml_comment_end(text, start).map(Some);
    }
    if text[start..].starts_with("<?") {
        return supported_processing_instruction_end(text, start).map(Some);
    }
    if text[start..].starts_with("<!") {
        return Err(MsgError::InvalidFormat);
    }
    Ok(None)
}
fn parse_attributes_limited(
    tag_body: &str,
    max_attributes: usize,
) -> Result<Vec<(String, String)>, MsgError> {
    let mut attrs = Vec::new();
    let mut cursor = tag_body.trim();
    if let Some((_, rest)) = cursor.split_once(char::is_whitespace) {
        cursor = rest.trim();
    } else {
        return Ok(attrs);
    }
    if cursor.ends_with('/') {
        cursor = cursor.trim_end_matches('/').trim_end();
    }
    while !cursor.is_empty() {
        if attrs.len() >= max_attributes {
            return Err(MsgError::InvalidFormat);
        }
        let name_end = cursor
            .find(|c: char| c.is_whitespace() || c == '=')
            .unwrap_or(cursor.len());
        if name_end == 0 {
            return Err(MsgError::InvalidFormat);
        }
        let name = &cursor[..name_end];
        if !is_supported_xml_attribute_name(name)
            || attrs.iter().any(|(attr_name, _)| attr_name == name)
        {
            return Err(MsgError::InvalidFormat);
        }
        let mut remainder = cursor[name_end..].trim_start();
        if !remainder.starts_with('=') {
            return Err(MsgError::InvalidFormat);
        }
        remainder = remainder[1..].trim_start();
        let Some(quote) = remainder.chars().next() else {
            return Err(MsgError::InvalidFormat);
        };
        if quote != '"' && quote != '\'' {
            return Err(MsgError::InvalidFormat);
        }
        let value_start = quote.len_utf8();
        let value_remainder = &remainder[value_start..];
        let Some(value_end) = value_remainder.find(quote) else {
            return Err(MsgError::InvalidFormat);
        };
        let value = &value_remainder[..value_end];
        if value.contains('<') {
            return Err(MsgError::InvalidFormat);
        }
        attrs.push((name.to_owned(), unescape_xml_text(value)?));
        let consumed = value_start + value_end + quote.len_utf8();
        cursor = remainder[consumed..].trim_start();
    }
    Ok(attrs)
}
fn parse_attributes(tag_body: &str) -> Result<Vec<(String, String)>, MsgError> {
    parse_attributes_limited(tag_body, usize::MAX)
}
fn is_supported_xml_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
fn is_supported_xml_qname(name: &str) -> bool {
    if name.matches(':').count() > 1 {
        return false;
    }
    if let Some((prefix, local)) = name.split_once(':') {
        is_supported_xml_name(prefix) && is_supported_xml_name(local) && prefix != "xmlns"
    } else {
        is_supported_xml_name(name)
    }
}
fn is_supported_xml_attribute_name(name: &str) -> bool {
    if name == "xmlns" {
        return true;
    }
    if let Some(prefix) = name.strip_prefix("xmlns:") {
        return is_supported_xml_name(prefix) && !matches!(prefix, "xml" | "xmlns");
    }
    is_supported_xml_qname(name)
}
fn unescape_xml_text(input: &str) -> Result<String, MsgError> {
    if !input.contains('&') {
        ensure_xml_characters(input)?;
        return Ok(input.to_owned());
    }
    let mut out = String::with_capacity(input.len());
    let mut remainder = input;
    while let Some(idx) = remainder.find('&') {
        out.push_str(&remainder[..idx]);
        remainder = &remainder[idx + 1..];
        let Some(end) = remainder.find(';') else {
            return Err(MsgError::InvalidFormat);
        };
        let entity = &remainder[..end];
        remainder = &remainder[end + 1..];
        match entity {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => out.push(decode_xml_character_reference(entity)?),
        }
    }
    out.push_str(remainder);
    ensure_xml_characters(&out)?;
    Ok(out)
}
fn decode_xml_character_reference(entity: &str) -> Result<char, MsgError> {
    let (digits, radix) = if let Some(digits) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        (digits, 16_u32)
    } else if let Some(digits) = entity.strip_prefix('#') {
        (digits, 10_u32)
    } else {
        return Err(MsgError::InvalidFormat);
    };
    if digits.is_empty() {
        return Err(MsgError::InvalidFormat);
    }
    let mut value = 0_u32;
    for byte in digits.bytes() {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' if radix == 16 => u32::from(byte - b'a' + 10),
            b'A'..=b'F' if radix == 16 => u32::from(byte - b'A' + 10),
            _ => return Err(MsgError::InvalidFormat),
        };
        if digit >= radix {
            return Err(MsgError::InvalidFormat);
        }
        value = value
            .checked_mul(radix)
            .and_then(|acc| acc.checked_add(digit))
            .ok_or(MsgError::InvalidFormat)?;
    }
    let ch = char::from_u32(value).ok_or(MsgError::InvalidFormat)?;
    if is_xml_character(ch) {
        Ok(ch)
    } else {
        Err(MsgError::InvalidFormat)
    }
}
fn is_xml_character(ch: char) -> bool {
    matches!(
        ch as u32,
        0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
    )
}
fn contains_only_xml_characters(input: &str) -> bool {
    input.chars().all(is_xml_character)
}
fn ensure_xml_characters(input: &str) -> Result<(), MsgError> {
    if contains_only_xml_characters(input) {
        Ok(())
    } else {
        Err(MsgError::InvalidFormat)
    }
}
fn buffer_real_iso20022_text(
    stack: &[String],
    path: &str,
    raw_text: &str,
    element_child_counts: &[usize],
    text_buffers: &mut HashMap<String, String>,
) -> Result<(), MsgError> {
    if raw_text.trim().is_empty() {
        return Ok(());
    }
    if element_child_counts.last().copied().unwrap_or_default() != 0 {
        return Err(MsgError::InvalidFormat);
    }
    let value = unescape_xml_text(raw_text)?;
    text_buffers
        .entry(path.to_owned())
        .or_default()
        .push_str(&value);
    let _ = stack;
    Ok(())
}
fn begin_real_xml_provenance(source: &[u8]) {
    MESSAGE_STACK.with(|stack| {
        if let Some(message) = stack.borrow_mut().last_mut() {
            message.xml_source_sha256 = Some(Sha256::digest(source).into());
            message.xml_field_sources.clear();
        }
    });
}
fn canonical_real_xml_field_name(message_type: &str, field: &str) -> String {
    if canonical_message_type(message_type).as_ref() == "pacs.009" {
        match field {
            // Preserve the application-header timestamp independently from the
            // group-header creation time used by the pacs.009 schema.
            "AppHdr/CreDt" => return field.to_owned(),
            // Payment status and return messages correlate through GrpHdr/MsgId;
            // it must not collapse into the distinct BAH BizMsgIdr identity.
            "Document/FICdtTrf/GrpHdr/MsgId" => return "MsgId".to_owned(),
            _ => {}
        }
    }
    canonical_field_name(message_type, field)
}
fn msg_set_xml(
    field: &str,
    value: &[u8],
    source: Range<usize>,
    fields_materialised: &mut usize,
) -> Result<(), MsgError> {
    MESSAGE_STACK.with(|stack| {
        let mut stack = stack.borrow_mut();
        let message = stack.last_mut().ok_or(MsgError::NoActiveMessage)?;
        let key = canonical_real_xml_field_name(&message.message_type, field);
        if let Some(existing) = message.fields.get(&key) {
            if existing.as_slice() != value {
                return Err(MsgError::InvalidFormat);
            }
            let existing_source = message
                .xml_field_sources
                .get_mut(&key)
                .ok_or(MsgError::InvalidFormat)?;
            existing_source.start = existing_source.start.min(source.start);
            existing_source.end = existing_source.end.max(source.end);
            return Ok(());
        }
        if *fields_materialised >= REAL_XML_MAX_FIELDS {
            return Err(MsgError::InvalidFormat);
        }
        *fields_materialised += 1;
        message.fields.insert(key.clone(), value.to_vec());
        message.xml_field_sources.insert(
            key,
            XmlFieldSource {
                start: source.start,
                end: source.end,
            },
        );
        Ok(())
    })
}
fn flush_real_iso20022_text(
    stack: &[String],
    declared_message_type: &mut Option<String>,
    path: &str,
    text_buffers: &mut HashMap<String, String>,
    source: Range<usize>,
    fields_materialised: &mut usize,
) -> Result<(), MsgError> {
    let Some(value) = text_buffers.remove(path) else {
        return Ok(());
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if stack
        .last()
        .is_some_and(|last| local_name(last) == "MsgDefIdr")
    {
        observe_declared_message_type(declared_message_type, trimmed)?;
    }
    msg_set_xml(path, trimmed.as_bytes(), source, fields_materialised)
}
fn parsed_attr_value<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find_map(|(attr_name, value)| (attr_name == name).then_some(value.as_str()))
}
fn parse_named_opening_attributes(
    raw_tag: &str,
    expected_name: &str,
) -> Result<Vec<(String, String)>, MsgError> {
    let tag_body = raw_tag.trim();
    if tag_body.ends_with('/') {
        return Err(MsgError::InvalidFormat);
    }
    let (name_part, _) = tag_body
        .split_once(char::is_whitespace)
        .unwrap_or((tag_body, ""));
    if name_part != expected_name || !is_supported_xml_qname(name_part) {
        return Err(MsgError::InvalidFormat);
    }
    parse_attributes(tag_body)
}
fn reject_unexpected_attrs(
    attrs: &[(String, String)],
    allowed: &[&str],
    label: &'static str,
) -> Result<(), MsgError> {
    if attrs
        .iter()
        .any(|(name, _)| !allowed.iter().any(|allowed_name| allowed_name == name))
    {
        return Err(MsgError::InvalidFormat);
    }
    if allowed.iter().any(|name| {
        attrs
            .iter()
            .filter(|(attr_name, _)| attr_name == name)
            .count()
            > 1
    }) {
        return Err(MsgError::InvalidFormat);
    }
    let _ = label;
    Ok(())
}
fn is_supported_internal_field_path(path: &str) -> bool {
    !path.is_empty()
        && path.split('/').all(|segment| {
            if segment.is_empty() {
                return false;
            }
            if let Some(attribute_name) = segment.strip_prefix('@') {
                return is_supported_xml_name(attribute_name);
            }
            let (name, index) = if let Some(index_start) = segment.find('[') {
                if !segment.ends_with(']') {
                    return false;
                }
                (
                    &segment[..index_start],
                    Some(&segment[index_start + 1..segment.len() - 1]),
                )
            } else {
                (segment, None)
            };
            is_supported_xml_name(name)
                && index.is_none_or(|idx| {
                    idx == "*" || !idx.is_empty() && idx.bytes().all(|b| b.is_ascii_digit())
                })
        })
}
fn parse_key_values(message_type: &str, text: &str) {
    for (key, value) in text.lines().filter_map(|line| line.split_once('=')) {
        msg_set(key.trim(), value.trim().as_bytes());
    }
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            // Ensure the message type was set correctly for empty inputs.
            if m.message_type != message_type {
                m.message_type = message_type.to_owned();
            }
        }
    });
}
fn parse_real_iso20022(message_type: &str, text: &str) -> Result<(), MsgError> {
    begin_real_xml_provenance(text.as_bytes());
    let mut stack: Vec<String> = Vec::new();
    let mut qname_stack: Vec<String> = Vec::new();
    let mut skip_stack: Vec<bool> = Vec::new();
    let mut element_child_counts: Vec<usize> = Vec::new();
    let mut element_starts: Vec<usize> = Vec::new();
    let mut semantic_namespace_stack: Vec<Option<String>> = Vec::new();
    let mut skip_depth = 0usize;
    let repeating_bases = repeating_bases_for(message_type);
    let mut repeat_counters: HashMap<String, usize> = HashMap::new();
    let mut declared_message_type: Option<String> = None;
    let mut document_root_seen = false;
    let mut top_level_root_seen = false;
    let mut namespace_scopes: Vec<Vec<(String, String)>> = Vec::new();
    let mut text_buffers: HashMap<String, String> = HashMap::new();
    let mut element_count = 0usize;
    let mut attribute_count = 0usize;
    let mut fields_materialised = 0usize;
    let mut idx = 0usize;
    let bytes = text.as_bytes();
    let len = bytes.len();
    while idx < len {
        let next_lt = match text[idx..].find('<') {
            Some(offset) => idx + offset,
            None => {
                let tail = &text[idx..];
                if skip_depth == 0
                    && semantic_namespace_stack.last().is_some_and(Option::is_some)
                    && let Some(path) = current_path(&stack)
                {
                    buffer_real_iso20022_text(
                        &stack,
                        &path,
                        tail,
                        &element_child_counts,
                        &mut text_buffers,
                    )?;
                } else if skip_depth == 0 && stack.is_empty() && !tail.trim().is_empty() {
                    return Err(MsgError::InvalidFormat);
                }
                break;
            }
        };
        if next_lt > idx && skip_depth == 0 {
            let body = &text[idx..next_lt];
            if semantic_namespace_stack.last().is_some_and(Option::is_some)
                && let Some(path) = current_path(&stack)
            {
                buffer_real_iso20022_text(
                    &stack,
                    &path,
                    body,
                    &element_child_counts,
                    &mut text_buffers,
                )?;
            } else if stack.is_empty() && !body.trim().is_empty() {
                return Err(MsgError::InvalidFormat);
            }
        }
        if let Some(special_end) = supported_special_xml_markup_end(text, next_lt)? {
            idx = special_end;
            continue;
        }
        let Some(tag_end) = find_tag_end(bytes, next_lt + 1) else {
            return Err(MsgError::InvalidFormat);
        };
        let raw_tag = &text[next_lt + 1..tag_end];
        idx = tag_end + 1;
        let tag = raw_tag.trim();
        if tag.starts_with('?') || tag.starts_with('!') {
            return Err(MsgError::InvalidFormat);
        }
        let closing = tag.starts_with('/');
        let tag_body = if closing {
            let closing_body = tag.strip_prefix('/').ok_or(MsgError::InvalidFormat)?;
            if closing_body.chars().next().is_some_and(char::is_whitespace) {
                return Err(MsgError::InvalidFormat);
            }
            let closing_body = closing_body.trim();
            if closing_body.is_empty() || closing_body.chars().any(char::is_whitespace) {
                return Err(MsgError::InvalidFormat);
            }
            closing_body
        } else {
            tag
        };
        let self_closing = !closing && tag_body.ends_with('/');
        let tag_body = if self_closing {
            tag_body.trim_end_matches('/').trim_end()
        } else {
            tag_body
        };
        let (name_part, _) = tag_body
            .split_once(char::is_whitespace)
            .unwrap_or((tag_body, ""));
        if !is_supported_xml_qname(name_part) {
            return Err(MsgError::InvalidFormat);
        }
        let lname = local_name(name_part);
        if closing {
            let Some(opened) = qname_stack.pop() else {
                return Err(MsgError::InvalidFormat);
            };
            if opened != name_part {
                return Err(MsgError::InvalidFormat);
            }
            let source_start = element_starts
                .last()
                .copied()
                .ok_or(MsgError::InvalidFormat)?;
            if let Some(skipped) = skip_stack.pop()
                && skipped
                && skip_depth > 0
            {
                skip_depth -= 1;
            }
            if skip_depth == 0
                && semantic_namespace_stack.last().is_some_and(Option::is_some)
                && let Some(path) = current_path(&stack)
            {
                flush_real_iso20022_text(
                    &stack,
                    &mut declared_message_type,
                    &path,
                    &mut text_buffers,
                    source_start..idx,
                    &mut fields_materialised,
                )?;
            }
            stack.pop();
            element_child_counts.pop();
            element_starts.pop();
            semantic_namespace_stack.pop();
            namespace_scopes.pop();
            continue;
        }
        element_count = element_count
            .checked_add(1)
            .ok_or(MsgError::InvalidFormat)?;
        if element_count > REAL_XML_MAX_ELEMENTS || qname_stack.len() >= REAL_XML_MAX_DEPTH {
            return Err(MsgError::InvalidFormat);
        }
        let remaining_attributes = REAL_XML_MAX_ATTRIBUTES
            .checked_sub(attribute_count)
            .ok_or(MsgError::InvalidFormat)?;
        let attrs = parse_attributes_limited(
            tag_body,
            remaining_attributes.min(REAL_XML_MAX_ATTRIBUTES_PER_ELEMENT),
        )?;
        attribute_count = attribute_count
            .checked_add(attrs.len())
            .ok_or(MsgError::InvalidFormat)?;
        if skip_depth == 0 && stack.is_empty() {
            if top_level_root_seen {
                return Err(MsgError::InvalidFormat);
            }
            top_level_root_seen = true;
        }
        let current_namespace_bindings = namespace_bindings(&attrs);
        let parent_namespace = semantic_namespace_stack.last().cloned().flatten();
        let parent_is_document = skip_depth == 0
            && stack
                .last()
                .is_some_and(|parent| local_name(parent) == "Document");
        let element_namespace = if skip_depth == 0 {
            element_namespace_uri(name_part, &attrs, &namespace_scopes)?.map(ToOwned::to_owned)
        } else {
            None
        };
        let is_dsig_signature = skip_depth == 0
            && lname == "Signature"
            && element_namespace.as_deref() == Some(XMLDSIG_NAMESPACE);
        if skip_depth == 0 && lname == "Signature" && !is_dsig_signature {
            return Err(MsgError::InvalidFormat);
        }
        if skip_depth == 0
            && parent_namespace.is_some()
            && matches!(lname, "DataPDU" | "DataEnvelope" | "Body")
        {
            return Err(MsgError::InvalidFormat);
        }
        let semantic_namespace = if skip_depth > 0 {
            parent_namespace.clone()
        } else if lname == "AppHdr" {
            if parent_namespace.is_some() {
                return Err(MsgError::InvalidFormat);
            }
            let namespace = element_namespace
                .as_deref()
                .ok_or(MsgError::UnknownMessageType)?;
            let definition =
                message_type_from_namespace(namespace).ok_or(MsgError::UnknownMessageType)?;
            if canonical_message_type(&definition).as_ref() != "head.001"
                || !is_versioned_message_definition_id(&definition)
            {
                return Err(MsgError::UnknownMessageType);
            }
            Some(namespace.to_owned())
        } else if lname == "Document" {
            if parent_namespace.is_some() {
                return Err(MsgError::InvalidFormat);
            }
            let namespace = element_namespace
                .as_deref()
                .ok_or(MsgError::UnknownMessageType)?;
            let definition =
                message_type_from_namespace(namespace).ok_or(MsgError::UnknownMessageType)?;
            observe_declared_message_type(&mut declared_message_type, &definition)?;
            Some(namespace.to_owned())
        } else if let Some(owner) = parent_namespace.as_deref() {
            if !is_dsig_signature && element_namespace.as_deref() != Some(owner) {
                return Err(MsgError::InvalidFormat);
            }
            Some(owner.to_owned())
        } else {
            None
        };
        let materialise_semantic = semantic_namespace.is_some();
        let is_skipped = skip_depth == 0
            && (is_dsig_signature
                || (lname == "Sgntr"
                    && parent_namespace.is_some()
                    && element_namespace == parent_namespace));
        if parent_is_document
            && !is_skipped
            && let Some(matches) = document_root_matches_message(message_type, lname)
        {
            let namespace = element_namespace
                .as_deref()
                .ok_or(MsgError::UnknownMessageType)?;
            let definition =
                message_type_from_namespace(namespace).ok_or(MsgError::UnknownMessageType)?;
            observe_declared_message_type(&mut declared_message_type, &definition)?;
            if !matches || document_root_seen {
                return Err(if matches {
                    MsgError::InvalidFormat
                } else {
                    MsgError::UnknownMessageType
                });
            }
            document_root_seen = true;
        }
        if lname.len() > REAL_XML_MAX_PATH_BYTES {
            return Err(MsgError::InvalidFormat);
        }
        let mut parts = normalised_parts(&stack);
        parts.push(lname.to_owned());
        let base_path = parts.join("/");
        if base_path.len() > REAL_XML_MAX_PATH_BYTES {
            return Err(MsgError::InvalidFormat);
        }
        let mut element_name = lname.to_owned();
        if should_index(&base_path, &repeating_bases) {
            let counter = repeat_counters.entry(base_path.clone()).or_insert(0);
            element_name = format!("{lname}[{counter}]");
            *counter = counter.checked_add(1).ok_or(MsgError::InvalidFormat)?;
        }
        if skip_depth == 0
            && let Some(parent_path) = current_path(&stack)
        {
            if text_buffers
                .get(&parent_path)
                .is_some_and(|text| !text.trim().is_empty())
            {
                return Err(MsgError::InvalidFormat);
            }
            if let Some(child_count) = element_child_counts.last_mut() {
                *child_count = child_count.checked_add(1).ok_or(MsgError::InvalidFormat)?;
            }
        }
        stack.push(element_name);
        let path = current_path(&stack);
        if path
            .as_ref()
            .is_some_and(|path| path.len() > REAL_XML_MAX_PATH_BYTES)
        {
            return Err(MsgError::InvalidFormat);
        }
        qname_stack.push(name_part.to_owned());
        element_child_counts.push(0);
        element_starts.push(next_lt);
        semantic_namespace_stack.push(semantic_namespace);
        namespace_scopes.push(current_namespace_bindings);
        if is_skipped && skip_depth == 0 && materialise_semantic {
            let path = path.as_deref().ok_or(MsgError::InvalidFormat)?;
            let marker_path = format!("{path}/@ignored");
            if marker_path.len() > REAL_XML_MAX_PATH_BYTES {
                return Err(MsgError::InvalidFormat);
            }
            msg_set_xml(
                &marker_path,
                SIGNATURE_IGNORED_VALUE,
                next_lt..idx,
                &mut fields_materialised,
            )?;
        }
        skip_stack.push(is_skipped);
        if is_skipped {
            skip_depth += 1;
        }
        if skip_depth == 0
            && materialise_semantic
            && let Some(path) = path.as_deref()
        {
            for (attr_name, value) in &attrs {
                if attr_name == "xmlns" || attr_name.starts_with("xmlns:") {
                    continue;
                }
                if let Some((prefix, _)) = attr_name.split_once(':') {
                    namespace_uri_for_prefix(prefix, &[], &namespace_scopes)
                        .ok_or(MsgError::InvalidFormat)?;
                    continue;
                }
                let attr_path = format!("{path}/@{attr_name}");
                if attr_path.len() > REAL_XML_MAX_PATH_BYTES {
                    return Err(MsgError::InvalidFormat);
                }
                msg_set_xml(
                    &attr_path,
                    value.as_bytes(),
                    next_lt..idx,
                    &mut fields_materialised,
                )?;
            }
        }
        if self_closing {
            if let Some(skipped) = skip_stack.pop()
                && skipped
                && skip_depth > 0
            {
                skip_depth -= 1;
            }
            stack.pop();
            qname_stack.pop();
            element_child_counts.pop();
            element_starts.pop();
            semantic_namespace_stack.pop();
            namespace_scopes.pop();
        }
    }
    if !qname_stack.is_empty() || !text_buffers.is_empty() || !top_level_root_seen {
        return Err(MsgError::InvalidFormat);
    }
    if let Some(declared) = declared_message_type {
        let requested_head = canonical_message_type(message_type) == "head.001";
        if !requested_head && !requested_message_matches_declaration(message_type, &declared) {
            return Err(MsgError::UnknownMessageType);
        }
    }
    if message_type_requires_document_root(message_type) && !document_root_seen {
        return Err(MsgError::InvalidFormat);
    }
    Ok(())
}
fn parse_xml_into_current(message_type: &str, text: &str) -> Result<(), MsgError> {
    let trimmed = text.trim();
    if !trimmed.starts_with("<ISO20022") {
        return Err(MsgError::InvalidFormat);
    }
    let tag_end = find_tag_end(trimmed.as_bytes(), 1).ok_or(MsgError::InvalidFormat)?;
    let root_attrs = parse_named_opening_attributes(&trimmed[1..tag_end], "ISO20022")?;
    reject_unexpected_attrs(&root_attrs, &["message"], "ISO20022")?;
    let declared = parsed_attr_value(&root_attrs, "message").ok_or(MsgError::InvalidFormat)?;
    if declared != message_type {
        return Err(MsgError::UnknownMessageType);
    }
    let cursor = &trimmed[tag_end + 1..];
    let close_idx = cursor.find("</ISO20022>").ok_or(MsgError::InvalidFormat)?;
    if !cursor[close_idx + "</ISO20022>".len()..].trim().is_empty() {
        return Err(MsgError::InvalidFormat);
    }
    let mut fields = &cursor[..close_idx];
    loop {
        fields = fields.trim_start();
        if fields.is_empty() {
            break;
        }
        if !fields.starts_with("<Field") {
            return Err(MsgError::InvalidFormat);
        }
        let field_tag_end = find_tag_end(fields.as_bytes(), 1).ok_or(MsgError::InvalidFormat)?;
        let field_attrs = parse_named_opening_attributes(&fields[1..field_tag_end], "Field")?;
        reject_unexpected_attrs(&field_attrs, &["path", "encoding"], "Field")?;
        let path = parsed_attr_value(&field_attrs, "path").ok_or(MsgError::InvalidFormat)?;
        if !is_supported_internal_field_path(path) {
            return Err(MsgError::InvalidFormat);
        }
        let encoding = parsed_attr_value(&field_attrs, "encoding");
        if let Some(encoding) = encoding
            && encoding != "base64"
        {
            return Err(MsgError::InvalidFormat);
        }
        fields = &fields[field_tag_end + 1..];
        let end_idx = fields.find("</Field>").ok_or(MsgError::InvalidFormat)?;
        let value_text = fields[..end_idx].trim();
        fields = &fields[end_idx + "</Field>".len()..];
        let value = if encoding == Some("base64") {
            decode_base64(value_text.as_bytes()).ok_or(MsgError::InvalidFormat)?
        } else {
            if value_text.contains('<') || value_text.contains("]]>") {
                return Err(MsgError::InvalidFormat);
            }
            unescape_xml_text(value_text)?.into_bytes()
        };
        msg_set(path, &value);
    }
    Ok(())
}
/// Encode a numeric amount as an ASCII string.
pub fn encode_amount(value: u64) -> Vec<u8> {
    value.to_string().into_bytes()
}
/// Decode a numeric amount from an ASCII string.
///
/// Returns `None` if the input contains non-digit characters or does not fit into a `u64`.
pub fn decode_amount(value: &[u8]) -> Option<u64> {
    if value.iter().all(|b| b.is_ascii_digit()) {
        core::str::from_utf8(value).ok()?.parse().ok()
    } else {
        None
    }
}
/// Base64 alphabet used by [`encode_base64`] and [`decode_base64`].
const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
/// Precomputed table mapping ASCII bytes to their 6-bit Base64 value. Invalid bytes map to `0xFF`.
const fn build_b64_decode_table() -> [u8; 256] {
    let mut table = [0xFFu8; 256];
    let mut i = 0;
    while i < 64 {
        table[BASE64_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    // Padding character is treated as zero during decoding.
    table[b'=' as usize] = 0;
    table
}
const BASE64_DECODE_TABLE: [u8; 256] = build_b64_decode_table();
/// Encode binary data as a Base64 ASCII string.
///
/// This lightweight helper is sufficient for tests and prototypes and uses
/// constant-time table lookups.
pub fn encode_base64(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        out.push(BASE64_TABLE[(b0 >> 2) as usize]);
        out.push(BASE64_TABLE[((b0 & 0x03) << 4 | (b1 >> 4)) as usize]);
        if chunk.len() > 1 {
            out.push(BASE64_TABLE[((b1 & 0x0F) << 2 | (b2 >> 6)) as usize]);
        } else {
            out.push(b'=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_TABLE[(b2 & 0x3F) as usize]);
        } else {
            out.push(b'=');
        }
    }
    out
}
/// Decode a Base64 ASCII string into the provided output buffer.
///
/// The caller supplies the destination [`Vec`] which is extended with the decoded bytes. This
/// allows large payloads to be processed without allocating a fresh buffer for every call.
///
/// Returns `None` if the input contains invalid characters or has the wrong padding.
pub fn decode_base64_into(data: &[u8], out: &mut Vec<u8>) -> Option<()> {
    if !data.len().is_multiple_of(4) {
        return None;
    }
    out.reserve(data.len().div_ceil(4) * 3);
    let mut i = 0;
    while i < data.len() {
        let n0 = BASE64_DECODE_TABLE[data[i] as usize];
        let n1 = BASE64_DECODE_TABLE[data[i + 1] as usize];
        let n2 = BASE64_DECODE_TABLE[data[i + 2] as usize];
        let n3 = BASE64_DECODE_TABLE[data[i + 3] as usize];
        if (n0 | n1 | n2 | n3) == 0xFF {
            return None;
        }
        out.push((n0 << 2) | (n1 >> 4));
        if data[i + 2] != b'=' {
            out.push((n1 << 4) | (n2 >> 2));
            if data[i + 3] != b'=' {
                out.push((n2 << 6) | n3);
            } else if n3 != 0 {
                return None;
            }
        } else if n2 != 0 || n3 != 0 {
            return None;
        }
        i += 4;
    }
    Some(())
}
/// Decode a Base64 ASCII string back into binary data.
///
/// Returns `None` if the input contains invalid characters or has the wrong padding.
pub fn decode_base64(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(data.len().div_ceil(4) * 3);
    decode_base64_into(data, &mut out)?;
    Some(out)
}
/// Encode a byte slice according to a named format.
///
/// Currently only `"BASE64"` is supported.
pub fn encode_str(format: &str, value: &[u8]) -> Vec<u8> {
    match format {
        "BASE64" => encode_base64(value),
        _ => value.to_vec(),
    }
}
/// Decode a string according to a named format.
///
/// Currently only `"BASE64"` is supported.
pub fn decode_str(format: &str, value: &[u8]) -> Option<Vec<u8>> {
    match format {
        "BASE64" => decode_base64(value),
        _ => Some(value.to_vec()),
    }
}
/// Validate a value against a named pattern.
///
/// Supported patterns are `"IBAN"`, `"BIC"`, and `"NUMERIC"`.
pub fn validate_format(pattern: &str, value: &[u8]) -> bool {
    match pattern {
        "IBAN" => validate_iban(value),
        "BIC" => validate_bic(value),
        "NUMERIC" => value.iter().all(|b| b.is_ascii_digit()),
        _ => false,
    }
}
/// Create a new ISO 20022 message of the given type.
///
/// The message is represented as a deterministic in-memory [`IsoMessage`] slot. Schema helpers and
/// validators can inspect or update the fields before the encoded XML is emitted.
pub fn msg_create(message_type: &str) {
    MESSAGE_STACK.with(|stack| {
        stack.borrow_mut().push(IsoMessage {
            message_type: message_type.to_owned(),
            ..IsoMessage::default()
        });
    });
}
/// Clone the current ISO 20022 message object.
///
/// The thread-local message stack stores deterministic in-memory structures, so
/// cloning duplicates the active message fields without reparsing XML.
pub fn msg_clone() {
    MESSAGE_STACK.with(|stack| {
        let cloned = { stack.borrow().last().cloned() };
        if let Some(m) = cloned {
            stack.borrow_mut().push(m);
        }
    });
}
/// Set the value of an ISO 20022 field.
///
/// Values are stored verbatim; type checking is intentionally omitted but the
/// call site can at least observe storage behaviour.
pub fn msg_set(field: &str, value: &[u8]) {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            let key = stored_field_key(m, field);
            m.fields.insert(key, value.to_vec());
            m.xml_source_sha256 = None;
            m.xml_field_sources.clear();
        }
    });
}
fn stored_field_key(message: &IsoMessage, field: &str) -> String {
    if message.fields.contains_key(field) {
        return field.to_owned();
    }
    let real_xml_key = canonical_real_xml_field_name(&message.message_type, field);
    if message.fields.contains_key(&real_xml_key) {
        return real_xml_key;
    }
    let canonical_key = canonical_field_name(&message.message_type, field);
    if real_xml_key != canonical_key {
        // The sealed v1 table historically aliases two distinct pacs.009
        // identities. Keep the corrected runtime key even when it has not yet
        // been materialised so a missing group MsgId cannot read, overwrite, or
        // remove the BAH BizMsgIdr (and likewise for the two creation times).
        return real_xml_key;
    }
    canonical_key
}
/// Retrieve the value of an ISO 20022 field.
pub fn msg_get(field: &str) -> Option<Vec<u8>> {
    MESSAGE_STACK.with(|stack| {
        let borrow = stack.borrow();
        let message = borrow.last()?;
        let key = stored_field_key(message, field);
        message.fields.get(&key).cloned()
    })
}
/// Append a repeating ISO 20022 sub-structure.
///
/// Each call creates an empty entry with an incremented index.  The entry can
/// later be populated using [`msg_set`] with the generated key.
pub fn msg_add(field: &str) {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            let base = canonical_repeating_base(&m.message_type, field);
            let count = m.repeats.entry(base.clone()).or_insert(0);
            let key = format!("{}[{}]", base, *count);
            m.fields.entry(key).or_default();
            *count += 1;
            m.xml_source_sha256 = None;
            m.xml_field_sources.clear();
        }
    });
}
/// Remove a field or sub-structure from the current message.
pub fn msg_remove(field: &str) {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            let key = stored_field_key(m, field);
            m.fields.remove(&key);
            m.xml_source_sha256 = None;
            m.xml_field_sources.clear();
        }
    });
}
/// Clear all fields of the current ISO 20022 message.
pub fn msg_clear() {
    MESSAGE_STACK.with(|stack| {
        if let Some(m) = stack.borrow_mut().last_mut() {
            m.fields.clear();
            m.repeats.clear();
            m.xml_source_sha256 = None;
            m.xml_field_sources.clear();
        }
    });
}
/// Parse raw data into an ISO 20022 message.
///
/// The parser accepts deterministic internal `<ISO20022>` XML wrappers, real
/// ISO 20022 XML payloads, and a compact `key=value` line-oriented format for
/// tests. XML inputs fail closed on malformed structure before fields are stored.
pub fn msg_parse(message_type: &str, data: &[u8]) -> Result<(), MsgError> {
    msg_create(message_type);
    let result = if looks_like_xml(data) {
        let text = core::str::from_utf8(data).map_err(|_| MsgError::InvalidFormat)?;
        if text.contains("<ISO20022") {
            parse_xml_into_current(message_type, text)
        } else {
            parse_real_iso20022(message_type, text)
        }
    } else {
        let text = core::str::from_utf8(data).map_err(|_| MsgError::InvalidFormat)?;
        parse_key_values(message_type, text);
        Ok(())
    };
    if result.is_err() {
        MESSAGE_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
    result
}
/// Serialize the current ISO 20022 message into a simple key=value format.
///
/// Fields are written one per line in lexicographic order of their keys.
pub fn msg_serialize(format: &str) -> Result<Vec<u8>, MsgError> {
    MESSAGE_STACK.with(|stack| {
        let borrow = stack.borrow();
        let message = borrow.last().ok_or(MsgError::NoActiveMessage)?;
        let schema = schema_for(&message.message_type);
        let normalized = if format.is_empty() {
            "KV".to_owned()
        } else {
            format.to_ascii_uppercase()
        };
        match normalized.as_str() {
            "XML" => Ok(serialize_xml(message, schema)),
            "KV" | "KEYVALUE" => Ok(serialize_key_value(message, schema)),
            _ => Err(MsgError::InvalidFormat),
        }
    })
}
/// Validate the current ISO 20022 message against schema rules.
///
/// Rather than a full schema engine we keep a tiny table of mandatory fields
/// for a handful of message types. Validation succeeds when the current message
/// exists **and** all required fields for its type are present.
pub fn msg_validate() -> bool {
    clear_validation_failure();
    MESSAGE_STACK.with(|stack| {
        stack.borrow().last().is_some_and(|m| {
            if let Some(schema) = schema_for(&m.message_type) {
                match validate_message_against_schema(m, schema) {
                    Ok(()) => true,
                    Err(err) => {
                        record_validation_failure(err);
                        false
                    }
                }
            } else {
                false
            }
        })
    })
}
/// Sign the current ISO 20022 message.
///
/// Uses Ed25519, secp256k1, or ML-DSA (Dilithium3) depending on the key length. Secret keys may be
/// prefixed with an `iroha_crypto::Algorithm` tag; secp256k1 signing requires the
/// `Algorithm::Secp256k1` tag to disambiguate 32-byte secret keys. The function signs the
/// serialized message bytes and returns the signature or an empty vector if signing fails.
#[allow(unused_variables)]
pub fn msg_sign(key: &[u8]) -> Vec<u8> {
    let msg = match msg_serialize("XML") {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    use pqcrypto_mldsa::mldsa65 as dilithium;
    use pqcrypto_traits::sign::{DetachedSignature as _, SecretKey as _};
    if let Some((tag, rest)) = key.split_first() {
        if *tag == Algorithm::Ed25519 as u8 && rest.len() == 32 {
            let Ok(sk_bytes) = <[u8; 32]>::try_from(rest) else {
                return Vec::new();
            };
            let sk = SigningKey::from_bytes(&sk_bytes);
            return sk.sign(&msg).to_bytes().to_vec();
        }
        if *tag == Algorithm::Secp256k1 as u8 && rest.len() == 32 {
            let Ok(sk_bytes) = <[u8; 32]>::try_from(rest) else {
                return Vec::new();
            };
            let Ok(sk) = EcdsaSecp256k1Sha256::parse_private_key(&sk_bytes) else {
                return Vec::new();
            };
            return EcdsaSecp256k1Sha256::sign(&msg, &sk);
        }
        if *tag == Algorithm::MlDsa as u8 && rest.len() == dilithium::secret_key_bytes() {
            let Ok(sk) = dilithium::SecretKey::from_bytes(rest) else {
                return Vec::new();
            };
            let sig = dilithium::detached_sign(&msg, &sk);
            return sig.as_bytes().to_vec();
        }
    }
    if let Ok(sk_bytes) = <[u8; 32]>::try_from(key) {
        let sk = SigningKey::from_bytes(&sk_bytes);
        return sk.sign(&msg).to_bytes().to_vec();
    }
    if key.len() == dilithium::secret_key_bytes()
        && let Ok(sk) = dilithium::SecretKey::from_bytes(key)
    {
        let sig = dilithium::detached_sign(&msg, &sk);
        return sig.as_bytes().to_vec();
    }
    Vec::new()
}
/// Verify the signature on an ISO 20022 message.
///
/// Uses the [`verify_signature`] helper with Ed25519, secp256k1, or ML-DSA
/// depending on the key length. The serialized message bytes are used as the
/// signing payload. Secp256k1 expects a 33-byte compressed SEC1 public key.
#[allow(unused_variables)]
pub fn msg_verify_sig(sig: &[u8], key: &[u8]) -> bool {
    let msg = match msg_serialize("XML") {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    {
        if key.len() == 32 {
            return verify_signature(SignatureScheme::Ed25519, &msg, sig, key);
        }
    }
    {
        use pqcrypto_mldsa::mldsa65 as dilithium;
        if key.len() == dilithium::public_key_bytes() && sig.len() == dilithium::signature_bytes() {
            return verify_signature(SignatureScheme::MlDsa, &msg, sig, key);
        }
    }
    {
        if key.len() == 33 && sig.len() == 64 {
            return verify_signature(SignatureScheme::Secp256k1, &msg, sig, key);
        }
    }
    false
}
#[cfg(test)]
mod tests {
    use super::{
        norito_schemas::{Colr012, Linkage, Sese023, Sese025},
        *,
    };
    use ed25519_dalek::SigningKey;
    use norito::codec::{Decode, Encode};
    // Helper to reset the thread-local between tests.
    fn reset() {
        MESSAGE_STACK.with(|m| m.borrow_mut().clear());
    }
    fn populate_pacs008_minimal() {
        msg_set("MsgId", b"1");
        msg_set("IntrBkSttlmCcy", b"USD");
        msg_set("IntrBkSttlmAmt", b"100");
        msg_set("IntrBkSttlmDt", b"2024-01-01");
        msg_set("DbtrAcct", b"GB82WEST12345698765432");
        msg_set("CdtrAcct", b"GB33BUKB20201555555555");
        msg_set("DbtrAgt", b"DEUTDEFF");
        msg_set("CdtrAgt", b"DEUTDEFF");
    }
    fn populate_camt053_minimal() {
        msg_set("Stmt/Id", b"1");
        msg_set("Stmt/Acct/Id", b"GB82WEST12345698765432");
        msg_set("Stmt/Acct/Ccy", b"USD");
        msg_set("Stmt/Bal[0]/Amt", b"100");
        msg_set("Stmt/Bal[0]/Ccy", b"USD");
        msg_set("Stmt/Bal[0]/Cd", b"CRDT");
    }
    fn populate_camt052_minimal() {
        msg_set("Rpt/Id", b"RPT1");
        msg_set("Rpt/CreDtTm", b"2024-01-01T00:00:00Z");
        msg_set("Rpt/Acct/Id", b"GB82WEST12345698765432");
        msg_set("Rpt/Acct/Ccy", b"USD");
        msg_add("Rpt/Ntry");
        msg_set("Rpt/Ntry[0]/Amt", b"100.00");
        msg_set("Rpt/Ntry[0]/CdtDbtInd", b"CRDT");
        msg_set("Rpt/Ntry[0]/BookgDt", b"2024-01-01");
    }
    fn populate_pain001_minimal() {
        msg_set("GrpHdr/MsgId", b"MSG-1");
        msg_set("GrpHdr/CreDtTm", b"2024-01-01T10:00:00Z");
        msg_set("GrpHdr/NbOfTxs", b"1");
        msg_set("GrpHdr/InitgPty/Nm", b"Initiator");
        msg_add("PmtInf");
        msg_set("PmtInf[0]/PmtInfId", b"PMT1");
        msg_set("PmtInf[0]/ReqdExctnDt", b"2024-01-02");
        msg_set("PmtInf[0]/DbtrAcct/Id", b"GB82WEST12345698765432");
        msg_add("PmtInf[0]/CdtTrfTxInf");
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/Amt", b"100");
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/Ccy", b"USD");
        msg_set(
            "PmtInf[0]/CdtTrfTxInf[0]/CdtrAcct/Id",
            b"GB33BUKB20201555555555",
        );
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/CdtrAgt", b"DEUTDEFF");
        msg_set("PmtInf[0]/CdtTrfTxInf[0]/EndToEndId", b"E2E1");
    }
    fn populate_pacs009_minimal() {
        msg_set("BizMsgIdr", b"BMSG1");
        msg_set("MsgDefIdr", b"pacs.009.001.10");
        msg_set("CreDtTm", b"2024-01-01T12:00:00Z");
        msg_set("IntrBkSttlmAmt", b"5000");
        msg_set("IntrBkSttlmCcy", b"USD");
        msg_set("IntrBkSttlmDt", b"2024-01-03");
        msg_set("InstgAgt", b"DEUTDEFF");
        msg_set("InstdAgt", b"MARKDEFF");
        msg_set("DbtrAcct", b"GB82WEST12345698765432");
        msg_set("CdtrAcct", b"GB33BUKB20201555555555");
    }
    fn populate_head001_minimal() {
        msg_set("AppHdr/BizMsgIdr", b"HDR-123");
        msg_set("AppHdr/MsgDefIdr", b"pacs.008.001.08");
        msg_set("AppHdr/CreDt", b"2025-01-01T12:00:00Z");
        msg_set("AppHdr/Fr/FIId/FinInstnId/BICFI", b"DEUTDEFF");
        msg_set("AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId", b"123456");
    }
    fn populate_pacs004_minimal() {
        msg_set("MsgId", b"RTRN1");
        msg_set("CreDtTm", b"2024-01-05T10:00:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
        msg_add("TxInf");
        msg_set("TxInf[0]/OrgnlInstrId", b"INST1");
        msg_set("TxInf[0]/RtrdInstdAmt", b"100.00");
        msg_set("TxInf[0]/RtrdInstdAmtCcy", b"USD");
    }
    fn populate_pacs028_minimal() {
        msg_set("MsgId", b"REQ1");
        msg_set("CreDtTm", b"2024-01-06T09:30:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
    }
    fn populate_pacs029_minimal() {
        msg_set("MsgId", b"STAT1");
        msg_set("CreDtTm", b"2024-01-06T09:45:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
        msg_add("TxInfAndSts");
        msg_set("TxInfAndSts[0]/TxSts", b"ACSP");
    }
    fn populate_pain002_minimal() {
        msg_set("GrpHdr/MsgId", b"PAINSTAT1");
        msg_set("GrpHdr/CreDtTm", b"2024-01-07T08:00:00Z");
        msg_set("OrgnlGrpInfAndSts/OrgnlMsgId", b"PAIN1");
        msg_set("OrgnlGrpInfAndSts/GrpSts", b"ACSP");
    }
    fn populate_pacs007_minimal() {
        msg_set("MsgId", b"CXL1");
        msg_set("CreDtTm", b"2024-01-02T09:30:00Z");
        msg_set("OrgnlGrpInf/OrgnlMsgId", b"ORIG1");
        msg_add("TxInf");
        msg_set("TxInf[0]/OrgnlInstrId", b"INST1");
        msg_set("TxInf[0]/OrgnlEndToEndId", b"E2E1");
        msg_set("TxInf[0]/OrgnlTxId", b"TX1");
        msg_set("TxInf[0]/CxlRsnInf/Rsn/Cd", b"RR01");
    }
    fn populate_camt056_minimal() {
        msg_set("Assgnmt/Id", b"CXL2");
        msg_set("Assgnmt/CreDtTm", b"2024-01-03T11:15:00Z");
        msg_set("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId", b"ORIG2");
        msg_set("Undrlyg/TxInf/OrgnlInstrId", b"INST2");
        msg_set("Undrlyg/TxInf/OrgnlEndToEndId", b"E2E2");
        msg_set("Undrlyg/TxInf/OrgnlTxId", b"TX2");
        msg_set("Undrlyg/TxInf/CxlRsnInf/Rsn/Cd", b"RC01");
    }
    fn populate_sese023_minimal() {
        msg_set("TxId", b"DVP-SETTLEMENT-1");
        msg_set("SttlmDt", b"2024-01-02");
        msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
        msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
        msg_set("SctiesLeg/FinInstrmId", b"US0378331005");
        msg_set("SctiesLeg/Qty", b"1000");
        msg_set("CashLeg/Amt", b"1050000");
        msg_set("CashLeg/Ccy", b"USD");
        msg_set("DlvrgSttlmPties/Pty/Bic", b"DEUTDEFF");
        msg_set("DlvrgSttlmPties/Acct", b"DLVRY-ACC");
        msg_set("RcvgSttlmPties/Pty/Bic", b"MARKDEFF");
        msg_set("RcvgSttlmPties/Acct", b"RCVG-ACC");
        msg_set("Plan/ExecutionOrder", b"DELIVERY_THEN_PAYMENT");
        msg_set("Plan/Atomicity", b"ALL_OR_NOTHING");
    }
    fn populate_sese025_minimal() {
        msg_set("TxId", b"DVP-SETTLEMENT-1");
        msg_set("SttlmDt", b"2024-01-02");
        msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
        msg_set("SttlmTpAndAddtlParams/Pmt", b"APMT");
        msg_set("ConfSts", b"ACCP");
        msg_set("SttlmQty", b"1000");
        msg_set("SttlmAmt", b"1050000");
        msg_set("SttlmCcy", b"USD");
        msg_set("Plan/ExecutionOrder", b"DELIVERY_THEN_PAYMENT");
        msg_set("Plan/Atomicity", b"ALL_OR_NOTHING");
        msg_set("RsnCd", b"SETTLED");
    }
    fn populate_colr012_minimal() {
        msg_set("TxId", b"COLLATERAL-EXCHANGE-1");
        msg_set("OblgtnId", b"REPO-DAILY-1");
        msg_set("Substitution/OriginalAmt", b"1000000");
        msg_set("Substitution/OriginalCcy", b"USD");
        msg_set("Substitution/SubstituteAmt", b"1005000");
        msg_set("Substitution/SubstituteCcy", b"USD");
        msg_set("Substitution/EffectiveDt", b"2024-01-05");
        msg_set("Substitution/Type", b"FULL");
        msg_set("Substitution/ReasonCd", b"HAIRCUT");
    }
    const PACS002_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/pacs002_fixture.xml");
    const PACS004_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/pacs004_fixture.xml");
    const CAMT056_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/camt056_fixture.xml");
    const CAMT056_001_09_FIXTURE: &str =
        include_str!(r"../../../fixtures/iso20022/camt056_001_09_fixture.xml");
    const SESE023_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/sese023_fixture.xml");
    const SESE024_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/sese024_fixture.xml");
    const SESE025_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/sese025_fixture.xml");
    const COLR007_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/colr007_fixture.xml");
    const COLR012_FIXTURE: &str = include_str!(r"../../../fixtures/iso20022/colr012_fixture.xml");
    fn expected_sese023_schema() -> Sese023 {
        Sese023 {
            tx_id: "DVP-FIXTURE-1".to_owned(),
            settlement_date: "2024-02-02".to_owned(),
            movement_type: "DELI".to_owned(),
            payment_type: "APMT".to_owned(),
            fin_instr_id: "US0378331005".to_owned(),
            quantity: "500".to_owned(),
            cash_amount: "1050000".to_owned(),
            cash_currency: "USD".to_owned(),
            delivering_party_bic: "DEUTDEFF".to_owned(),
            delivering_account: "DLVRY-ACC".to_owned(),
            receiving_party_bic: "MARKDEFF".to_owned(),
            receiving_account: "RCVG-ACC".to_owned(),
            execution_order: "DELIVERY_THEN_PAYMENT".to_owned(),
            atomicity: "ALL_OR_NOTHING".to_owned(),
            settlement_condition: Some("NOMC".to_owned()),
            partial_settlement_indicator: Some("NPAR".to_owned()),
            hold_indicator: Some(true),
            venue_mic: Some("XNAS".to_owned()),
            linkages: vec![
                Linkage {
                    relation: "WITH".to_owned(),
                    reference: "SUBST-PAIR-B".to_owned(),
                },
                Linkage {
                    relation: "BEFO".to_owned(),
                    reference: "PACS009-CLS".to_owned(),
                },
            ],
            securities_metadata: Some(r#"{"note":"delivery"}"#.to_owned()),
            cash_metadata: Some(r#"{"note":"cash"}"#.to_owned()),
        }
    }
    fn expected_sese025_schema() -> Sese025 {
        Sese025 {
            tx_id: "PVP-FIXTURE-1".to_owned(),
            settlement_date: "2024-03-01".to_owned(),
            movement_type: "RECE".to_owned(),
            payment_type: "APMT".to_owned(),
            confirmation_status: "ACCP".to_owned(),
            settlement_quantity: "250000".to_owned(),
            settlement_amount: "100000".to_owned(),
            settlement_currency: "USD".to_owned(),
            security_id: Some("US0378331005".to_owned()),
            security_quantity: Some("500".to_owned()),
            delivering_party_bic: Some("DEUTDEFF".to_owned()),
            delivering_account: Some("DLVRY-ACC".to_owned()),
            receiving_party_bic: Some("MARKDEFF".to_owned()),
            receiving_account: Some("RCVG-ACC".to_owned()),
            execution_order: "PAYMENT_THEN_DELIVERY".to_owned(),
            atomicity: "COMMIT_SECOND_LEG".to_owned(),
            hold_indicator: Some(false),
            partial_settlement_indicator: Some("NPAR".to_owned()),
            settlement_condition: Some("NOMC".to_owned()),
            venue_mic: None,
            reason_code: Some("MATCHED".to_owned()),
            additional_info: Some(r#"{"counter_ccy":"EUR"}"#.to_owned()),
        }
    }
    fn expected_colr012_schema() -> Colr012 {
        Colr012 {
            tx_id: "COLR-FIXTURE-1".to_owned(),
            obligation_id: "REPO-123".to_owned(),
            original_amount: "1000000".to_owned(),
            original_currency: "USD".to_owned(),
            substitute_amount: "1002000".to_owned(),
            substitute_currency: "USD".to_owned(),
            haircut: Some("50".to_owned()),
            effective_date: "2024-04-05".to_owned(),
            substitution_type: "PARTIAL".to_owned(),
            original_fin_instr_id: Some("US0378331005".to_owned()),
            substitute_fin_instr_id: Some("US5949181045".to_owned()),
            reason_code: Some("MARGIN".to_owned()),
        }
    }
    const GENERATED_MD: &str = include_str!(r"../../../generatediso20022.md");
    const SAMPLE_PACS008_XML: &str = include_str!("assets/text_v1/pacs008_sample.xml");
    const SAMPLE_PACS004_XML: &str = include_str!("assets/text_v1/pacs004_sample.xml");
    const SAMPLE_PACS009_XML: &str = include_str!("assets/text_v1/pacs009_sample.xml");
    const SAMPLE_PACS009_ENVELOPE_XML: &str = include_str!("assets/text_v1/pacs009_envelope.xml");
    const SAMPLE_PACS002_STATUS_XML: &str = include_str!("assets/text_v1/pacs002_status.xml");
    const SAMPLE_PACS002_AUTH_XML: &str = include_str!("assets/text_v1/pacs002_auth.xml");
    const SAMPLE_CAMT052_XML: &str = include_str!("assets/text_v1/camt052_sample.xml");
    const SAMPLE_CAMT056_XML: &str = include_str!("assets/text_v1/camt056_sample.xml");
    fn assert_validated(message_type: &str, xml: &str) {
        reset();
        msg_parse(message_type, xml.as_bytes())
            .unwrap_or_else(|err| panic!("parse {message_type} sample: {err:?}"));
        let valid = msg_validate();
        let failure = take_validation_failure();
        assert!(valid, "validation failed: {failure:?}");
    }
    fn generated_sample(message_marker: &str) -> String {
        let needle = format!("<!-- {message_marker} -->");
        let all = GENERATED_MD;
        let marker_pos = all
            .find(&needle)
            .unwrap_or_else(|| panic!("marker not found: {message_marker}"));
        let before = &all[..marker_pos];
        let start_fence = before
            .rfind("```xml")
            .unwrap_or_else(|| panic!("xml fence missing for {message_marker}"));
        let after_fence = &all[start_fence + "```xml".len()..];
        let end = after_fence
            .find("```")
            .unwrap_or_else(|| panic!("closing fence missing for {message_marker}"));
        after_fence[..end].trim().to_owned()
    }
    #[test]
    fn msg_create_and_validate() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs008_requires_creditor_agent() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_remove("CdtrAgt");
        assert!(!msg_validate());
    }
    #[test]
    fn pacs008_requires_debtor_account() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_remove("DbtrAcct");
        assert!(!msg_validate());
    }
    #[test]
    fn msg_validate_rejects_non_numeric_amount() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmAmt", b"not-a-number");
        assert!(!msg_validate());
    }
    #[test]
    fn msg_validate_rejects_invalid_iban() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAcct", b"GB82WEST12345698765433");
        assert!(!msg_validate());
    }
    #[test]
    fn take_validation_error_reports_identifier_failure() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("IntrBkSttlmCcy", b"ZZZ");
        assert!(!msg_validate());
        let err = take_validation_error().expect("validation error captured");
        match err {
            MsgError::InvalidIdentifier { field, kind } => {
                assert_eq!(field, "IntrBkSttlmCcy");
                assert_eq!(kind, IdentifierKind::Currency);
            }
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(take_validation_error().is_none(), "error should be drained");
    }
    #[test]
    fn msg_validate_accepts_valid_iban() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs008_validates_proxy_identifiers() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAcct/Prxy/Id", b"1233214568521");
        msg_set("DbtrAcct/Prxy/Tp/Cd", b"2100");
        msg_set("CdtrAcct/Prxy/Id", b"4210118604441");
        assert!(
            msg_validate(),
            "proxy identifiers should validate when populated alongside IBANs"
        );
        assert_eq!(
            msg_get("DbtrAcct/Prxy/Id").as_deref(),
            Some(&b"1233214568521"[..])
        );
    }
    #[test]
    fn pacs008_rejects_empty_proxy_identifier() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAcct/Prxy/Id", b"");
        assert!(!msg_validate(), "empty proxy ids must fail validation");
        let failure = take_validation_failure().expect("validation failure captured");
        match failure {
            ValidationFailure::InvalidField { field, reason } => {
                assert_eq!(field, "DbtrAcct/Prxy/Id");
                assert!(matches!(reason, InvalidReason::Empty));
            }
            other => panic!("unexpected validation failure: {other:?}"),
        }
    }
    #[test]
    fn head001_requires_core_fields() {
        reset();
        msg_create("head.001.001.03");
        populate_head001_minimal();
        assert!(msg_validate(), "baseline header should validate");
        msg_remove("AppHdr/CreDt");
        assert!(!msg_validate(), "missing CreDt must fail validation");
        let err = take_validation_error().expect("validation error captured");
        match err {
            MsgError::MissingField(field) => assert_eq!(field, "AppHdr/CreDt"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn msg_validate_rejects_invalid_bic() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        msg_set("DbtrAgt", b"deutdeff");
        assert!(!msg_validate());
    }
    #[test]
    fn msg_validate_accepts_valid_bic() {
        reset();
        msg_create("pacs.008");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn msg_validate_rejects_invalid_isin() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_set("SctiesLeg/FinInstrmId", b"INVALID123456");
        assert!(!msg_validate());
    }
    #[test]
    fn msg_validate_accepts_cusip_instrument() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_set("SctiesLeg/FinInstrmId", b"037833100");
        assert!(msg_validate());
    }
    #[test]
    fn parse_message_reports_invalid_instrument() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_set("SctiesLeg/FinInstrmId", b"BADSIGN");
        let xml = msg_serialize("XML").expect("serialize");
        let err = parse_message("sese.023", &xml).expect_err("validation should fail");
        assert!(matches!(
            err,
            MsgError::InvalidInstrument { field } if field == "SctiesLeg/FinInstrmId"
        ));
    }
    #[test]
    fn validate_identifier_helpers() {
        assert!(validate_identifier(IdentifierKind::Isin, "US0378331005"));
        assert!(!validate_identifier(IdentifierKind::Isin, "US0378331004"));
        assert!(validate_identifier(IdentifierKind::Cusip, "037833100"));
        assert!(!validate_identifier(IdentifierKind::Cusip, "03783310X"));
        assert!(validate_identifier(
            IdentifierKind::Lei,
            "5493001KJTIIGC8Y1R12"
        ));
        assert!(!validate_identifier(
            IdentifierKind::Lei,
            "5493001KJTIIGC8Y1R13"
        ));
        assert!(validate_identifier(IdentifierKind::Bic, "DEUTDEFF"));
        assert!(!validate_identifier(IdentifierKind::Bic, "deutDEFF"));
        assert!(validate_identifier(IdentifierKind::Mic, "XNAS"));
        assert!(!validate_identifier(IdentifierKind::Mic, "1NAS"));
        assert!(validate_identifier(
            IdentifierKind::Iban,
            "GB82WEST12345698765432"
        ));
        assert!(!validate_identifier(
            IdentifierKind::Iban,
            "GB82WEST12345698765433"
        ));
        assert!(validate_identifier(
            IdentifierKind::Iban,
            "de89370400440532013000"
        ));
        assert!(validate_identifier(IdentifierKind::Iban, "NO9386011117947"));
        assert!(!validate_identifier(
            IdentifierKind::Iban,
            "GB82WEST1234569876543"
        ));
        assert!(!validate_identifier(
            IdentifierKind::Iban,
            "ZZ82WEST12345698765432"
        ));
        assert!(validate_identifier(IdentifierKind::Currency, "USD"));
        assert!(!validate_identifier(IdentifierKind::Currency, "ZZZ"));
    }
    #[test]
    fn validate_instrument_identifier_helper() {
        assert!(validate_instrument_identifier("US0378331005"));
        assert!(validate_instrument_identifier("037833100"));
        assert!(!validate_instrument_identifier("INVALID"));
    }
    #[test]
    fn camt053_requires_account_id() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        msg_remove("Stmt/Acct/Id");
        assert!(!msg_validate());
    }
    #[test]
    fn camt053_rejects_invalid_iban() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        msg_set("Stmt/Acct/Id", b"GB82WEST12345698765433");
        assert!(!msg_validate());
    }
    #[test]
    fn camt053_rejects_non_numeric_balance() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        msg_set("Stmt/Bal[0]/Amt", b"not-number");
        assert!(!msg_validate());
    }
    #[test]
    fn camt053_accepts_valid_message() {
        reset();
        msg_create("camt.053");
        populate_camt053_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn camt052_accepts_valid_message() {
        reset();
        msg_create("camt.052");
        populate_camt052_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pain001_accepts_valid_message() {
        reset();
        msg_create("pain.001");
        populate_pain001_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pain001_rejects_missing_credit_transfer() {
        reset();
        msg_create("pain.001");
        populate_pain001_minimal();
        msg_remove("PmtInf[0]/CdtTrfTxInf[0]/Amt");
        assert!(!msg_validate());
    }
    #[test]
    fn pacs009_accepts_valid_message() {
        reset();
        msg_create("pacs.009");
        populate_pacs009_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs009_rejects_missing_agents() {
        reset();
        msg_create("pacs.009");
        populate_pacs009_minimal();
        msg_remove("InstgAgt");
        assert!(!msg_validate());
    }
    #[test]
    fn pacs004_accepts_valid_message() {
        reset();
        msg_create("pacs.004");
        populate_pacs004_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs028_accepts_valid_message() {
        reset();
        msg_create("pacs.028");
        populate_pacs028_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs029_accepts_valid_message() {
        reset();
        msg_create("pacs.029");
        populate_pacs029_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pain002_accepts_valid_message() {
        reset();
        msg_create("pain.002");
        populate_pain002_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs007_accepts_valid_message() {
        reset();
        msg_create("pacs.007");
        populate_pacs007_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn camt056_accepts_valid_message() {
        reset();
        msg_create("camt.056");
        populate_camt056_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn pacs002_accepts_valid_message() {
        reset();
        msg_create("pacs.002");
        msg_set("OrgnlMsgId", b"1");
        msg_set("TxSts", b"ACTC");
        assert!(msg_validate());
    }
    #[test]
    fn pacs002_accepts_missing_tx_status() {
        reset();
        msg_create("pacs.002");
        msg_set("OrgnlMsgId", b"1");
        assert!(msg_validate());
    }
    #[test]
    fn pacs002_rejects_unknown_status() {
        reset();
        msg_create("pacs.002");
        msg_set("OrgnlMsgId", b"1");
        msg_set("TxSts", b"XXXX");
        assert!(!msg_validate());
    }
    #[test]
    fn msg_clone_copies_fields() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        msg_clone();
        assert_eq!(msg_get("field").as_deref(), Some(&b"value"[..]));
    }
    #[test]
    fn msg_set_and_get() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        assert_eq!(msg_get("field").as_deref(), Some(&b"value"[..]));
    }
    #[test]
    fn msg_clear_removes_all() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        msg_clear();
        assert!(msg_get("field").is_none());
    }
    #[test]
    fn msg_add_creates_incrementing_keys() {
        reset();
        msg_create("pacs.008");
        msg_add("Entry");
        msg_add("Entry");
        assert!(msg_get("Entry[0]").is_some());
        assert!(msg_get("Entry[1]").is_some());
        assert!(msg_get("Entry[2]").is_none());
    }
    #[test]
    fn msg_remove_deletes_field() {
        reset();
        msg_create("pacs.008");
        msg_set("field", b"value");
        msg_remove("field");
        assert!(msg_get("field").is_none());
    }
    #[test]
    fn msg_clone_pushes_copy_on_stack() {
        reset();
        msg_create("pacs.008");
        msg_set("MsgId", b"1");
        msg_clone();
        msg_set("MsgId", b"2");
        super::MESSAGE_STACK.with(|s| {
            let stack = s.borrow();
            assert_eq!(stack.len(), 2);
            assert_eq!(stack[0].fields.get("MsgId").unwrap(), b"1");
            assert_eq!(stack[1].fields.get("MsgId").unwrap(), b"2");
        });
    }
    #[test]
    fn msg_parse_and_serialize_roundtrip() {
        reset();
        msg_parse("pacs.008", b"field=value\nfoo=bar").unwrap();
        assert_eq!(msg_get("foo").as_deref(), Some(&b"bar"[..]));
        assert_eq!(
            msg_serialize("KV").unwrap(),
            b"field=value\nfoo=bar".to_vec()
        );
    }
    #[test]
    fn parse_message_materialises_fields() {
        reset();
        let parsed = parse_message(
            "pacs.008",
            b"MsgId=abc\nIntrBkSttlmCcy=USD\nIntrBkSttlmAmt=10\nIntrBkSttlmDt=2024-01-01\nDbtrAcct=GB82WEST12345698765432\nCdtrAcct=GB33BUKB20201555555555\nDbtrAgt=DEUTDEFF\nCdtrAgt=DEUTDEFF",
        )
        .expect("message parses");
        assert_eq!(parsed.message_type(), "pacs.008");
        assert_eq!(parsed.field_text("MsgId"), Some("abc"));
        assert_eq!(parsed.field_text("IntrBkSttlmCcy"), Some("USD"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_xml_message_rejects_developer_formats() {
        reset();
        assert!(matches!(
            parse_xml_message("pacs.008", b"MsgId=abc"),
            Err(MsgError::InvalidFormat)
        ));
        assert!(matches!(
            parse_xml_message(
                "pacs.008",
                br#"<ISO20022 message="pacs.008"><Field path="MsgId">abc</Field></ISO20022>"#,
            ),
            Err(MsgError::InvalidFormat)
        ));
        let parsed = parse_xml_message("pacs.008", SAMPLE_PACS008_XML.as_bytes())
            .expect("real ISO XML remains accepted");
        assert_eq!(parsed.field_text("MsgId"), Some("ISO-008-GRP"));
    }
    #[test]
    fn real_xml_provenance_covers_only_the_signed_range() {
        reset();
        let parsed = parse_xml_message("pacs.008", SAMPLE_PACS008_XML.as_bytes())
            .expect("sample XML parses");
        assert!(parsed.fields_are_covered_by_xml_range(
            SAMPLE_PACS008_XML.as_bytes(),
            0..SAMPLE_PACS008_XML.len()
        ));
        let document_start = SAMPLE_PACS008_XML
            .find("<Document")
            .expect("Document start");
        let document_end = SAMPLE_PACS008_XML
            .find("</Document>")
            .map(|offset| offset + "</Document>".len())
            .expect("Document end");
        assert!(!parsed.fields_are_covered_by_xml_range(
            SAMPLE_PACS008_XML.as_bytes(),
            document_start..document_end
        ));
        let changed = SAMPLE_PACS008_XML.replace("ISO-008-GRP", "ISO-008-ALT");
        assert!(!parsed.fields_are_covered_by_xml_range(changed.as_bytes(), 0..changed.len()));
    }
    #[test]
    fn parse_message_validation_failure() {
        reset();
        let err = parse_message("pacs.008", b"MsgId=abc").unwrap_err();
        assert!(matches!(err, MsgError::MissingField("IntrBkSttlmCcy")));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_head001_envelope_preserves_apphdr_fields() {
        reset();
        let xml = include_str!("assets/text_v1/head001_envelope.xml");
        let parsed =
            parse_message("head.001.001.03", xml.as_bytes()).expect("header envelope parses");
        assert_eq!(parsed.message_type(), "head.001.001.03");
        assert_eq!(parsed.field_text("AppHdr/BizMsgIdr"), Some("HDR-123"));
        assert_eq!(
            parsed.field_text("AppHdr/MsgDefIdr"),
            Some("pacs.008.001.08")
        );
        assert_eq!(
            parsed.field_text("AppHdr/Fr/FIId/FinInstnId/BICFI"),
            Some("DEUTDEFF")
        );
        assert_eq!(
            parsed.field_text("AppHdr/To/FIId/FinInstnId/ClrSysMmbId/MmbId"),
            Some("654321")
        );
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn pacs008_accepts_proxy_accounts_without_iban() {
        reset();
        let xml = include_str!("assets/text_v1/pacs008_proxy.xml");
        let parsed =
            parse_message("pacs.008.001.08", xml.as_bytes()).expect("proxy-only pacs.008 parses");
        assert_eq!(parsed.field_text("DbtrAcct/Prxy/Id"), Some("proxy-debtor"));
        assert_eq!(
            parsed.field_text("CdtrAcct/Prxy/Id"),
            Some("proxy-creditor")
        );
        assert!(parsed.field_text("DbtrAcct").is_none());
        assert!(parsed.field_text("CdtrAcct").is_none());
    }
    #[test]
    fn msg_validate_none() {
        reset();
        assert!(!msg_validate());
    }
    #[test]
    fn versioned_pacs008_supported() {
        reset();
        msg_create("pacs.008.001.10");
        populate_pacs008_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_pacs002_supported() {
        reset();
        msg_create("pacs.002.001.12");
        msg_set("OrgnlMsgId", b"ABC");
        msg_set("TxSts", b"ACSP");
        assert!(msg_validate());
    }
    #[test]
    fn versioned_camt053_supported() {
        reset();
        msg_create("camt.053.001.08");
        populate_camt053_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_camt054_supported() {
        reset();
        msg_create("camt.054.001.08");
        msg_set("Ntfctn/Id", b"NTF1");
        msg_set("Ntfctn/Acct/Id", b"GB82WEST12345698765432");
        msg_add("Ntfctn/Ntry");
        msg_set("Ntfctn/Ntry[0]/Amt", b"15.00");
        msg_set("Ntfctn/Ntry[0]/Ccy", b"USD");
        msg_set("Ntfctn/Ntry[0]/CdtDbtInd", b"CRDT");
        assert!(msg_validate());
    }
    #[test]
    fn versioned_camt052_supported() {
        reset();
        msg_create("camt.052.001.09");
        populate_camt052_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn parse_sample_pacs008() {
        assert_validated("pacs.008.001.08", SAMPLE_PACS008_XML);
        let msg_id = msg_get("MsgId");
        assert_eq!(msg_id.as_deref(), Some(b"ISO-008-GRP".as_ref()));
        assert_eq!(msg_get("IntrBkSttlmCcy").as_deref(), Some(b"USD".as_ref()));
        assert_eq!(
            msg_get("IntrBkSttlmAmt").as_deref(),
            Some(b"1400.00".as_ref())
        );
    }
    #[test]
    fn iso_xsd_document_roots_cover_supported_xml_families() {
        for (message_type, root) in [
            ("colr.012.001.05", "CollSbstitnConf"),
            ("pacs.002.001.10", "FIToFIPmtStsRpt"),
            ("pacs.004.001.09", "PmtRtr"),
            ("pacs.007.001.09", "FIToFIPmtRvsl"),
            ("pacs.008.001.08", "FIToFICstmrCdtTrf"),
            ("pacs.009.001.10", "FICdtTrf"),
            ("pacs.028.001.09", "FIToFIPmtStsReq"),
            ("pacs.029.001.09", "RsltnOfInvstgtn"),
            ("pain.001.001.11", "CstmrCdtTrfInitn"),
            ("pain.002.001.12", "CstmrPmtStsRpt"),
            ("camt.029.001.09", "RsltnOfInvstgtn"),
            ("camt.052.001.08", "BkToCstmrAcctRpt"),
            ("camt.053.001.08", "BkToCstmrStmt"),
            ("camt.054.001.08", "BkToCstmrDbtCdtNtfctn"),
            ("camt.056.001.08", "FIToFIPmtCxlReq"),
            ("sese.023.001.11", "SctiesSttlmTxInstr"),
            ("sese.024.001.10", "SctiesSttlmTxStsAdvc"),
            ("sese.025.001.10", "SctiesSttlmTxConf"),
        ] {
            assert_eq!(
                document_root_matches_message(message_type, root),
                Some(true)
            );
            assert_eq!(
                document_root_matches_message(message_type, "WrongDocumentRoot"),
                Some(false)
            );
        }
    }
    #[test]
    fn parse_real_iso20022_rejects_mismatched_xsd_document_root() {
        reset();
        let xml = include_str!("assets/text_v1/pacs002_wrong_root.xml");
        let err = parse_message("pacs.002.001.10", xml.as_bytes())
            .expect_err("pacs.002 must not accept pacs.008 document root");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_missing_xsd_document_root() {
        reset();
        let xml = include_str!("assets/text_v1/pacs002_missing_root.xml");
        let err = parse_message("pacs.002.001.10", xml.as_bytes())
            .expect_err("real pacs.002 XML must carry its XSD document root");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_requested_version_drift() {
        reset();
        let err = parse_message("pacs.008.001.10", SAMPLE_PACS008_XML.as_bytes())
            .expect_err("exact requested MDR version must match payload declarations");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_header_document_definition_drift() {
        reset();
        let xml = SAMPLE_PACS008_XML.replace(
            "urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08",
            "urn:iso:std:iso:20022:tech:xsd:pacs.008.001.10",
        );
        let err = parse_message("pacs.008", xml.as_bytes())
            .expect_err("BAH MsgDefIdr must match Document XSD namespace exactly");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_spoofed_document_namespace_suffix() {
        reset();
        let xml = SAMPLE_PACS008_XML.replace(
            "urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08",
            "https://attacker.invalid/schema:pacs.008.001.08",
        );
        let err = parse_message("pacs.008", xml.as_bytes())
            .expect_err("Document namespace must use the exact ISO 20022 XSD prefix");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_empty_document_namespace_definition() {
        reset();
        let xml = SAMPLE_PACS008_XML.replace(
            "urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08",
            "urn:iso:std:iso:20022:tech:xsd:",
        );
        let err = parse_message("pacs.008", xml.as_bytes())
            .expect_err("Document namespace must include a concrete message definition");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unqualified_document_namespace() {
        reset();
        let xml = SAMPLE_PACS008_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">"#,
            "<Document>",
        );
        let err = parse_message("pacs.008", xml.as_bytes())
            .expect_err("Document must be bound to the ISO 20022 XSD namespace");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_accepts_prefixed_iso_document_namespace() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace(
                r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
                r#"<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10" xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            )
            .replace("<FIToFIPmtStsRpt>", "<pacs:FIToFIPmtStsRpt>")
            .replace("</FIToFIPmtStsRpt>", "</pacs:FIToFIPmtStsRpt>")
            .replace("</Document>", "</pacs:Document>");
        let parsed = parse_message("pacs.002", xml.as_bytes())
            .expect("prefixed Document and payload root should resolve to ISO namespace");
        assert_eq!(parsed.message_type(), "pacs.002");
        assert_eq!(parsed.field_text("MsgId"), Some("ISO-PACS002-STATUS"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_prefixed_document_namespace_spoofing() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace(
                r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
                r#"<pacs:Document xmlns:pacs="https://attacker.invalid/schema:pacs.002.001.10" xmlns="https://attacker.invalid/schema:pacs.002.001.10">"#,
            )
            .replace("<FIToFIPmtStsRpt>", "<pacs:FIToFIPmtStsRpt>")
            .replace("</FIToFIPmtStsRpt>", "</pacs:FIToFIPmtStsRpt>")
            .replace("</Document>", "</pacs:Document>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("prefixed Document namespace must use the ISO XSD URI");
        assert!(matches!(err, MsgError::UnknownMessageType));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_payload_root_namespace_spoofing() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace(
                "<FIToFIPmtStsRpt>",
                r#"<evil:FIToFIPmtStsRpt xmlns:evil="https://attacker.invalid/schema:pacs.002.001.10">"#,
            )
            .replace("</FIToFIPmtStsRpt>", "</evil:FIToFIPmtStsRpt>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("Document payload root must resolve to the ISO XSD namespace");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_conflicting_canonical_aliases() {
        reset();
        let xml =
            SAMPLE_PACS002_STATUS_XML.replace("<GrpSts>ACSP</GrpSts>", "<GrpSts>RJCT</GrpSts>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("canonical aliases may repeat only an identical value");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_descendant_namespace_spoofing() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace(
                "<MsgId>",
                r#"<evil:MsgId xmlns:evil="https://attacker.invalid/iso">"#,
            )
            .replace("</MsgId>", "</evil:MsgId>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("every semantic descendant must retain the Document namespace");
        assert!(matches!(err, MsgError::InvalidFormat));
    }
    #[test]
    fn parse_real_iso20022_ignores_fields_outside_semantic_namespaces() {
        reset();
        let xml = r#"<DataPDU xmlns:evil="https://attacker.invalid/iso">
  <evil:Body>
    <evil:MsgId>EVIL-MSG</evil:MsgId>
    <evil:IntrBkSttlmAmt>10.00</evil:IntrBkSttlmAmt>
    <evil:IntrBkSttlmCcy>USD</evil:IntrBkSttlmCcy>
    <evil:IntrBkSttlmDt>2024-01-01</evil:IntrBkSttlmDt>
    <evil:DbtrAcct>GB82WEST12345698765432</evil:DbtrAcct>
    <evil:CdtrAcct>GB33BUKB20201555555555</evil:CdtrAcct>
    <evil:DbtrAgt>DEUTDEFF</evil:DbtrAgt>
    <evil:CdtrAgt>MARKDEFF</evil:CdtrAgt>
  </evil:Body>
  <Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.08">
    <FIToFICstmrCdtTrf/>
  </Document>
</DataPDU>"#;
        let err = parse_xml_message("pacs.008", xml.as_bytes())
            .expect_err("transport-wrapper fields must not satisfy the ISO schema");
        assert!(matches!(err, MsgError::MissingField("MsgId")));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_transparent_wrappers_inside_document() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace("<GrpHdr>", "<Body><GrpHdr>")
            .replace("</GrpHdr>", "</GrpHdr></Body>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("transport wrappers must not erase semantic path components");
        assert!(matches!(err, MsgError::InvalidFormat));
    }
    #[test]
    fn parse_real_iso20022_enforces_depth_and_attribute_budgets() {
        reset();
        let nested = format!(
            "{}{}",
            "<X>".repeat(REAL_XML_MAX_DEPTH),
            "</X>".repeat(REAL_XML_MAX_DEPTH)
        );
        let deep = SAMPLE_PACS002_STATUS_XML.replace("<GrpHdr>", &format!("<GrpHdr>{nested}"));
        assert!(matches!(
            parse_message("pacs.002", deep.as_bytes()),
            Err(MsgError::InvalidFormat)
        ));

        let attributes = (0..=REAL_XML_MAX_ATTRIBUTES_PER_ELEMENT)
            .map(|index| format!(" a{index}=\"x\""))
            .collect::<String>();
        let wide = SAMPLE_PACS002_STATUS_XML.replace("<MsgId>", &format!("<MsgId{attributes}>"));
        assert!(matches!(
            parse_message("pacs.002", wide.as_bytes()),
            Err(MsgError::InvalidFormat)
        ));
    }
    #[test]
    fn parse_real_iso20022_rejects_mismatched_closing_tag() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace("</GrpHdr>", "</WrongGrpHdr>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("mismatched closing tags must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_extra_closing_tag() {
        reset();
        let xml = format!("{SAMPLE_PACS002_STATUS_XML}</Document>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("extra closing tags must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_attributed_closing_tag() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace("</GrpHdr>", r#"</GrpHdr attr="x">"#);
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("closing tags with attributes must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unclosed_document_tag() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace("</Document>", "");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unclosed Document tags must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_accepts_single_quoted_namespace_attribute() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<Document xmlns='urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10'>"#,
        );
        let parsed = parse_message("pacs.002", xml.as_bytes())
            .expect("single-quoted XML attributes are well-formed");
        assert_eq!(parsed.message_type(), "pacs.002");
        assert_eq!(parsed.field_text("MsgId"), Some("ISO-PACS002-STATUS"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unquoted_namespace_attribute() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<Document xmlns=urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10>"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unquoted XML namespace attributes must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unterminated_attribute_value() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10>"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unterminated XML attributes must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_duplicate_namespace_attribute() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10" xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("duplicate XML attributes must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_malformed_trailing_attribute() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10" malformed>"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("malformed trailing XML attributes must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_decodes_xml_entities_in_text_and_attributes() {
        reset();
        let xml = SAMPLE_PACS008_XML
            .replace(
                "<BizMsgIdr>ISO-SAMPLE-008</BizMsgIdr>",
                "<BizMsgIdr>ISO&#45;SAMPLE&amp;008</BizMsgIdr>",
            )
            .replace(
                r#"<IntrBkSttlmAmt Ccy="USD">1400.00</IntrBkSttlmAmt>"#,
                r#"<IntrBkSttlmAmt Ccy="US&#68;">1400.00</IntrBkSttlmAmt>"#,
            );
        let parsed = parse_message("pacs.008", xml.as_bytes())
            .expect("valid XML character references should parse");
        assert_eq!(
            parsed.field_text("AppHdr/BizMsgIdr"),
            Some("ISO-SAMPLE&008")
        );
        assert_eq!(parsed.field_text("IntrBkSttlmCcy"), Some("USD"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unknown_xml_entity_reference() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            "<MsgId>ISO-PACS002-STATUS</MsgId>",
            "<MsgId>ISO-&xxe;-STATUS</MsgId>",
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unknown XML entities must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unterminated_xml_entity_reference() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            "<MsgId>ISO-PACS002-STATUS</MsgId>",
            "<MsgId>ISO&amp-PACS002-STATUS</MsgId>",
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unterminated XML entities must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_invalid_numeric_xml_character_reference() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<IntrBkSttlmAmt Ccy="USD">1400</IntrBkSttlmAmt>"#,
            r#"<IntrBkSttlmAmt Ccy="US&#x0;">1400</IntrBkSttlmAmt>"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("invalid XML numeric character references must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_raw_invalid_xml_character_in_text() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            "<MsgId>ISO-PACS002-STATUS</MsgId>",
            "<MsgId>ISO-\u{1}-STATUS</MsgId>",
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("raw invalid XML characters in text must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_raw_invalid_xml_character_in_attribute() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<IntrBkSttlmAmt Ccy="USD">1400</IntrBkSttlmAmt>"#,
            "<IntrBkSttlmAmt Ccy=\"US\u{1}\">1400</IntrBkSttlmAmt>",
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("raw invalid XML characters in attributes must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_raw_less_than_in_attribute_value() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<IntrBkSttlmAmt Ccy="USD">1400</IntrBkSttlmAmt>"#,
            r#"<IntrBkSttlmAmt Ccy="US<D">1400</IntrBkSttlmAmt>"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("raw less-than characters in XML attributes must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_accepts_xml_declaration_and_well_formed_comment() {
        reset();
        let xml =
            format!("<?xml version=\"1.0\"?>\n<!--valid comment-->\n{SAMPLE_PACS002_STATUS_XML}");
        let parsed = parse_message("pacs.002", xml.as_bytes())
            .expect("well-formed XML declaration and comments should parse");
        assert_eq!(parsed.message_type(), "pacs.002");
        assert_eq!(parsed.field_text("MsgId"), Some("ISO-PACS002-STATUS"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_concatenates_text_split_by_comment() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            "<MsgId>ISO-PACS002-STATUS</MsgId>",
            "<MsgId>ISO-<!--valid-->PACS002-STATUS</MsgId>",
        );
        let parsed = parse_message("pacs.002", xml.as_bytes())
            .expect("comments inside simple content should not overwrite text chunks");
        assert_eq!(parsed.field_text("MsgId"), Some("ISO-PACS002-STATUS"));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_text_before_child_element() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace("<GrpHdr>", "<GrpHdr>mixed");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("mixed content before child elements must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_text_after_child_element() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace("</MsgId>", "</MsgId>mixed");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("mixed content after child elements must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_content_outside_single_root() {
        for (label, xml) in [
            (
                "extra leading root",
                format!("<Ignored/>{SAMPLE_PACS002_STATUS_XML}"),
            ),
            (
                "trailing text",
                format!("{SAMPLE_PACS002_STATUS_XML}outside"),
            ),
        ] {
            reset();
            let err = match parse_message("pacs.002", xml.as_bytes()) {
                Ok(_) => panic!("{label} outside the ISO XML root must fail parsing"),
                Err(err) => err,
            };
            assert!(matches!(err, MsgError::InvalidFormat), "{label}: {err:?}");
            assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
        }
    }
    #[test]
    fn parse_real_iso20022_rejects_unterminated_comment() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace("<GrpHdr>", "<!--unterminated><GrpHdr>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unterminated comments must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_malformed_comment_body() {
        reset();
        let xml = format!("<!--bad--comment-->\n{SAMPLE_PACS002_STATUS_XML}");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("comments containing double hyphen must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_malformed_processing_instruction() {
        reset();
        let xml = format!("<?bad processing>\n{SAMPLE_PACS002_STATUS_XML}");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unterminated processing instructions must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unsupported_doctype_declaration() {
        reset();
        let xml =
            format!("<!DOCTYPE Document [<!ENTITY x \"boom\">]>\n{SAMPLE_PACS002_STATUS_XML}");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("DOCTYPE declarations must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_unsupported_cdata_section() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            "<MsgId>ISO-PACS002-STATUS</MsgId>",
            "<MsgId><![CDATA[ISO-PACS002-STATUS]]></MsgId>",
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("CDATA must fail real ISO XML parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_malformed_document_qname() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace(
                r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
                r#"<pacs::Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            )
            .replace("</Document>", "</pacs::Document>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("malformed Document QNames must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_malformed_payload_root_qname() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML
            .replace(
                "<FIToFIPmtStsRpt>",
                r#"<pacs::FIToFIPmtStsRpt xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            )
            .replace("</FIToFIPmtStsRpt>", "</pacs::FIToFIPmtStsRpt>");
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("malformed payload root QNames must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_malformed_namespace_declaration_name() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<pacs:Document xmlns::pacs="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("malformed namespace declaration names must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_rejects_invalid_element_name_start() {
        reset();
        let xml = SAMPLE_PACS002_STATUS_XML.replace(
            r#"<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
            r#"<1Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.002.001.10">"#,
        );
        let err = parse_message("pacs.002", xml.as_bytes())
            .expect_err("unsupported XML element names must fail parsing");
        assert!(matches!(err, MsgError::InvalidFormat));
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_real_iso20022_allows_canonical_family_request_with_consistent_version() {
        reset();
        let parsed = parse_message("pacs.008", SAMPLE_PACS008_XML.as_bytes())
            .expect("canonical family requests defer exact MDR allowlists to profiles");
        assert_eq!(parsed.message_type(), "pacs.008");
        assert_eq!(
            parsed.field_text("AppHdr/MsgDefIdr"),
            Some("pacs.008.001.08")
        );
        assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
    }
    #[test]
    fn parse_sample_pacs009() {
        assert_validated("pacs.009.001.10", SAMPLE_PACS009_XML);
        assert_eq!(
            msg_get("BizMsgIdr").as_deref(),
            Some(b"PACS009-BIZ".as_ref())
        );
        assert_eq!(msg_get("MsgId").as_deref(), Some(b"PACS009-GRP".as_ref()));
        assert_eq!(
            msg_get("MsgDefIdr").as_deref(),
            Some(b"pacs.009.001.10".as_ref())
        );
        assert_eq!(msg_get("IntrBkSttlmAmt").as_deref(), Some(b"2500".as_ref()));
        assert_eq!(msg_get("IntrBkSttlmCcy").as_deref(), Some(b"USD".as_ref()));
        assert_eq!(
            msg_get("DbtrAcct").as_deref(),
            Some(b"GB82WEST12345698765432".as_ref())
        );
        assert_eq!(
            msg_get("CdtrAcct").as_deref(),
            Some(b"GB33BUKB20201555555555".as_ref())
        );
        assert_eq!(msg_get("Purp").as_deref(), Some(b"SECU".as_ref()));
    }
    #[test]
    fn parse_sample_pacs009_envelope() {
        assert_validated("pacs.009.001.10", SAMPLE_PACS009_ENVELOPE_XML);
        assert_eq!(
            msg_get("BizMsgIdr").as_deref(),
            Some(b"BAH-PACS009-1".as_ref())
        );
        assert_eq!(
            msg_get("MsgDefIdr").as_deref(),
            Some(b"pacs.009.001.10".as_ref())
        );
        assert_eq!(
            msg_get("AppHdr/CreDt").as_deref(),
            Some(b"2025-11-12T09:34:09Z".as_ref())
        );
        assert_eq!(msg_get("InstgAgt").as_deref(), Some(b"DEUTDEFF".as_ref()));
        assert_eq!(msg_get("InstdAgt").as_deref(), Some(b"MARKDEFF".as_ref()));
        assert_eq!(
            msg_get("DbtrAcct").as_deref(),
            Some(b"GB82WEST12345698765432".as_ref())
        );
        assert_eq!(
            msg_get("CdtrAcct").as_deref(),
            Some(b"GB33BUKB20201555555555".as_ref())
        );
    }
    #[test]
    fn parsed_pacs009_distinct_identity_fields_remain_mutable() {
        assert_validated("pacs.009.001.10", SAMPLE_PACS009_ENVELOPE_XML);

        msg_set("AppHdr/CreDt", b"2025-11-12T09:35:00Z");
        assert_eq!(
            msg_get("AppHdr/CreDt").as_deref(),
            Some(b"2025-11-12T09:35:00Z".as_ref())
        );

        msg_set("Document/FICdtTrf/GrpHdr/MsgId", b"PACS009-GRP-NEW");
        assert_eq!(
            msg_get("MsgId").as_deref(),
            Some(b"PACS009-GRP-NEW".as_ref())
        );
        msg_remove("Document/FICdtTrf/GrpHdr/MsgId");
        assert!(msg_get("MsgId").is_none());
        assert_eq!(
            msg_get("BizMsgIdr").as_deref(),
            Some(b"BAH-PACS009-1".as_ref())
        );
    }
    #[test]
    fn parse_sample_pacs002_auth_allows_missing_txsts() {
        assert_validated("pacs.002.001.10", SAMPLE_PACS002_AUTH_XML);
        assert_eq!(
            msg_get("MsgId").as_deref(),
            Some(b"ISO-PACS002-AUTH".as_ref())
        );
        assert_eq!(
            msg_get("OrgnlMsgId").as_deref(),
            Some(b"ISO-SAMPLE-008".as_ref())
        );
    }
    #[test]
    fn parse_sese024_status_advice_lifecycle_fields() {
        let xml = include_str!("assets/text_v1/sese024_status.xml");
        assert_validated("sese.024.001.10", xml);
        assert_eq!(msg_get("TxId").as_deref(), Some(b"SETTLEMENT-123".as_ref()));
        assert_eq!(msg_get("SttlmSts").as_deref(), Some(b"SETT".as_ref()));
        assert_eq!(msg_get("RsnCd").as_deref(), Some(b"NARR".as_ref()));
    }
    #[test]
    fn parse_sample_camt052_allows_other_account_id() {
        assert_validated("camt.052.001.08", SAMPLE_CAMT052_XML);
        assert_eq!(
            msg_get("Rpt/Acct/Id").as_deref(),
            Some(b"ALTACCOUNT".as_ref())
        );
    }
    #[test]
    fn parse_sample_camt056_tracks_assignment() {
        assert_validated("camt.056.001.08", SAMPLE_CAMT056_XML);
        assert_eq!(
            msg_get("Assgnmt/Id").as_deref(),
            Some(b"ISO-CAMT056-ASSIGN".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId").as_deref(),
            Some(b"ISO-SAMPLE-008".as_ref())
        );
    }
    #[test]
    fn camt056_fixture_parses_cancellation_fields() {
        assert_validated("camt.056.001.08", CAMT056_FIXTURE);
        assert_eq!(
            msg_get("Assgnmt/Id").as_deref(),
            Some(b"CANCEL-FIXTURE-1".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId").as_deref(),
            Some(b"CANCEL-ORIG-1".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/CxlRsnInf/Rsn/Cd").as_deref(),
            Some(b"CUST".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/CxlRsnInf/AddtlInf").as_deref(),
            Some(b"customer requested recall".as_ref())
        );
    }
    #[test]
    fn camt056_001_09_fixture_parses_cancellation_fields() {
        assert_validated("camt.056.001.09", CAMT056_001_09_FIXTURE);
        assert_eq!(
            msg_get("Assgnmt/Id").as_deref(),
            Some(b"CANCEL-FIXTURE-9".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/OrgnlGrpInf/OrgnlMsgId").as_deref(),
            Some(b"CANCEL-ORIG-9".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/CxlRsnInf/Rsn/Cd").as_deref(),
            Some(b"CUST".as_ref())
        );
        assert_eq!(
            msg_get("Undrlyg/TxInf/CxlRsnInf/AddtlInf").as_deref(),
            Some(b"customer requested recall".as_ref())
        );
    }
    #[test]
    fn parse_camt029_resolution_of_investigation() {
        let xml = include_str!("assets/text_v1/camt029_resolution.xml");
        assert_validated("camt.029.001.09", xml);
        assert_eq!(
            msg_get("Assgnmt/Id").as_deref(),
            Some(b"IROHA-CAMT029-ORIGINAL-1".as_ref())
        );
        assert_eq!(msg_get("Sts").as_deref(), Some(b"CNCL".as_ref()));
        assert_eq!(
            msg_get("CxlDtls/OrgnlGrpInf/OrgnlMsgId").as_deref(),
            Some(b"ORIGINAL-1".as_ref())
        );
    }
    #[test]
    fn parse_sample_pacs004_return() {
        assert_validated("pacs.004.001.09", SAMPLE_PACS004_XML);
        assert_eq!(
            msg_get("MsgId").as_deref(),
            Some(b"ISO-PACS004-MSG".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdInstdAmt").as_deref(),
            Some(b"10.00".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdInstdAmtCcy").as_deref(),
            Some(b"USD".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/ChrgBr").as_deref(),
            Some(b"SLEV".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdRsn/Prtry").as_deref(),
            Some(b"TechnicalProblem".as_ref())
        );
    }
    #[test]
    fn pacs004_fixture_parses_return_fields() {
        assert_validated("pacs.004.001.09", PACS004_FIXTURE);
        assert_eq!(
            msg_get("MsgId").as_deref(),
            Some(b"RETURN-FIXTURE-1".as_ref())
        );
        assert_eq!(
            msg_get("OrgnlGrpInf/OrgnlMsgId").as_deref(),
            Some(b"ORIGINAL-008".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdInstdAmt").as_deref(),
            Some(b"10.00".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdInstdAmtCcy").as_deref(),
            Some(b"USD".as_ref())
        );
        assert_eq!(
            msg_get("TxInf[0]/RtrdRsn/Cd").as_deref(),
            Some(b"AC01".as_ref())
        );
    }
    #[test]
    fn parse_sample_pacs002_status() {
        assert_validated("pacs.002.001.10", SAMPLE_PACS002_STATUS_XML);
        assert_eq!(
            msg_get("OrgnlMsgId").as_deref(),
            Some(b"ISO-SAMPLE-008".as_ref())
        );
        assert_eq!(msg_get("TxSts").as_deref(), Some(b"ACSP".as_ref()));
    }
    #[test]
    fn pacs002_fixture_parses_status_fields() {
        assert_validated("pacs.002.001.10", PACS002_FIXTURE);
        assert_eq!(
            msg_get("MsgId").as_deref(),
            Some(b"STATUS-FIXTURE-1".as_ref())
        );
        assert_eq!(msg_get("StsId").as_deref(), Some(b"STATUS-TX-1".as_ref()));
        assert_eq!(
            msg_get("OrgnlMsgId").as_deref(),
            Some(b"STATUS-ORIG-1".as_ref())
        );
        assert_eq!(msg_get("TxSts").as_deref(), Some(b"ACSC".as_ref()));
        assert_eq!(
            msg_get("AddtlInf[0]").as_deref(),
            Some(b"settled by fixture report".as_ref())
        );
    }
    #[test]
    fn parse_generated_md_pacs008() {
        let xml = generated_sample("pacs.008.001.08");
        let parsed = parse_message("pacs.008", xml.as_bytes()).expect("parse pacs.008 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(
            parsed.field_text("MsgId"),
            Some("ISO-SAMPLE-001"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("Document/FIToFICstmrCdtTrf/CdtTrfTxInf/PmtId/UETR"),
            Some("123e4567-e89b-12d3-a456-426614174000"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("Document/FIToFICstmrCdtTrf/CdtTrfTxInf/ChrgBr"),
            Some("SHAR"),
            "keys={keys:?}"
        );
    }
    #[test]
    fn parse_generated_md_pacs004() {
        let xml = generated_sample("pacs.004.001.09");
        let parsed = parse_message("pacs.004", xml.as_bytes()).expect("parse pacs.004 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(
            parsed.field_text("MsgId"),
            Some("ISO-SAMPLE-004"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("OrgnlGrpInf/OrgnlMsgId"),
            Some("ISO-SAMPLE-001"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("TxInf[0]/ChrgBr"),
            Some("SHAR"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("TxInf[0]/RtrdRsn/Prtry"),
            Some("PR01"),
            "keys={keys:?}"
        );
    }
    #[test]
    fn parse_generated_md_pacs002() {
        let xml = generated_sample("pacs.002.001.10");
        let parsed = parse_message("pacs.002", xml.as_bytes()).expect("parse pacs.002 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(parsed.field_text("TxSts"), Some("ACSP"), "keys={keys:?}");
        assert_eq!(
            parsed.field_text("Document/FIToFIPmtStsRpt/OrgnlGrpInfAndSts/OrgnlMsgNmId"),
            Some("pacs.008.001.08"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("OrgnlMsgId"),
            Some("ISO-SAMPLE-001"),
            "keys={keys:?}"
        );
    }
    #[test]
    fn parse_generated_md_camt052() {
        let xml = generated_sample("camt.052.001.08");
        reset();
        msg_parse("camt.052", xml.as_bytes()).expect("parse camt.052 sample");
        let valid = msg_validate();
        let failure = take_validation_failure();
        assert!(
            !valid,
            "camt.052 sample should miss Rpt/CreDtTm: {failure:?}"
        );
        assert_eq!(
            msg_get("Rpt/Acct/Id").as_deref(),
            Some(b"ALPHBANK-USD-ACCOUNT-001".as_ref())
        );
        assert_eq!(
            msg_get("Rpt/Ntry[0]/CdtDbtInd").as_deref(),
            Some(b"DBIT".as_ref())
        );
    }
    #[test]
    fn parse_generated_md_camt056() {
        let xml = generated_sample("camt.056.001.08");
        let parsed = parse_message("camt.056", xml.as_bytes()).expect("parse camt.056 sample");
        let keys: Vec<String> = parsed.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(
            parsed.field_text("Document/FIToFIPmtCxlReq/Undrlyg/TxInf/OrgnlUETR"),
            Some("123e4567-e89b-12d3-a456-426614174000"),
            "keys={keys:?}"
        );
        assert_eq!(
            parsed.field_text("Assgnmt/Id"),
            Some("ISO-SAMPLE-056-ASGMT-001"),
            "keys={keys:?}"
        );
    }
    #[test]
    fn versioned_pacs007_supported() {
        reset();
        msg_create("pacs.007.001.09");
        populate_pacs007_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_camt056_supported() {
        reset();
        msg_create("camt.056.001.09");
        populate_camt056_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_pacs004_supported() {
        reset();
        msg_create("pacs.004.001.10");
        populate_pacs004_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_pacs028_supported() {
        reset();
        msg_create("pacs.028.001.09");
        populate_pacs028_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_pacs029_supported() {
        reset();
        msg_create("pacs.029.001.09");
        populate_pacs029_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn sese023_roundtrip_and_norito_snapshot() {
        reset();
        let schema = expected_sese023_schema();
        schema.apply_to_stack();
        assert!(msg_validate());
        let xml = msg_serialize("XML").expect("serialize sese.023");
        let xml_str = String::from_utf8(xml.clone()).expect("utf8");
        assert!(xml_str.contains("ISO20022 message=\"sese.023\""));
        assert!(xml_str.contains("Field path=\"SttlmTpAndAddtlParams/SctiesMvmntTp\""));
        assert!(xml_str.contains("Field path=\"Plan/Atomicity\""));
        let parsed = parse_message("sese.023", &xml).expect("parse sese.023");
        assert_eq!(parsed.field_text("SttlmParams/PrtlSttlmInd"), Some("NPAR"));
        assert_eq!(parsed.field_text("SttlmParams/HldInd"), Some("true"));
        let materialized =
            Sese023::from_parsed(&parsed).expect("materialize sese.023 into Norito schema");
        assert_eq!(schema, materialized);
        let encoded = schema.encode();
        let mut cursor = encoded.as_slice();
        let decoded = Sese023::decode(&mut cursor).expect("decode");
        assert_eq!(schema, decoded);
    }
    #[test]
    fn sese023_requires_movement_and_payment_qualifiers() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_remove("SttlmTpAndAddtlParams/SctiesMvmntTp");
        assert!(!msg_validate());
        msg_set("SttlmTpAndAddtlParams/SctiesMvmntTp", b"DELI");
        msg_set("SttlmTpAndAddtlParams/Pmt", b"INVALID");
        assert!(!msg_validate());
    }
    #[test]
    fn sese023_missing_execution_order_fails() {
        reset();
        msg_create("sese.023");
        populate_sese023_minimal();
        msg_remove("Plan/ExecutionOrder");
        assert!(!msg_validate());
        msg_set("Plan/ExecutionOrder", b"INVALID");
        assert!(!msg_validate());
    }
    #[test]
    fn sese023_fixture_parses_into_schema() {
        reset();
        let parsed =
            parse_message("sese.023", SESE023_FIXTURE.as_bytes()).expect("parse sese.023 fixture");
        let schema = Sese023::from_parsed(&parsed).expect("materialize sese.023 from fixture");
        assert_eq!(schema, expected_sese023_schema());
    }
    #[test]
    fn sese024_fixture_parses_status_advice_fields() {
        reset();
        let parsed =
            parse_message("sese.024", SESE024_FIXTURE.as_bytes()).expect("parse sese.024 fixture");
        assert_eq!(parsed.field_text("TxId"), Some("DVP-FIXTURE-1"));
        assert_eq!(parsed.field_text("SttlmDt"), Some("2024-02-02"));
        assert_eq!(parsed.field_text("SttlmSts"), Some("PEND"));
        assert_eq!(parsed.field_text("RsnCd"), Some("NORE"));
        assert_eq!(
            parsed.field_text("AddtlInf"),
            Some("awaiting CSD matching confirmation")
        );
    }
    #[test]
    fn sese025_validation_and_serialization() {
        reset();
        let schema = expected_sese025_schema();
        schema.apply_to_stack();
        assert!(msg_validate());
        let xml = msg_serialize("XML").expect("serialize sese.025");
        let parsed = parse_message("sese.025", &xml).expect("parse sese.025");
        let materialized = Sese025::from_parsed(&parsed).expect("materialize sese.025 into schema");
        assert_eq!(materialized, schema);
        assert_eq!(parsed.field_text("ConfSts"), Some("ACCP"));
        assert_eq!(
            parsed.field_text("Plan/Atomicity"),
            Some("COMMIT_SECOND_LEG")
        );
        assert_eq!(
            parsed.field_text("SttlmTpAndAddtlParams/SctiesMvmntTp"),
            Some("RECE")
        );
        assert_eq!(parsed.field_text("SttlmParams/HldInd"), Some("false"));
    }
    #[test]
    fn sese025_requires_plan_fields() {
        reset();
        msg_create("sese.025");
        populate_sese025_minimal();
        msg_remove("Plan/ExecutionOrder");
        assert!(!msg_validate());
        msg_set("Plan/ExecutionOrder", b"PAYMENT_THEN_DELIVERY");
        msg_set("Plan/Atomicity", b"INVALID");
        assert!(!msg_validate());
    }
    #[test]
    fn sese025_fixture_parses_into_schema() {
        reset();
        let parsed =
            parse_message("sese.025", SESE025_FIXTURE.as_bytes()).expect("parse sese.025 fixture");
        let schema = Sese025::from_parsed(&parsed).expect("materialize sese.025 from fixture");
        assert_eq!(schema, expected_sese025_schema());
    }
    #[test]
    fn colr012_roundtrip_and_norito_snapshot() {
        reset();
        let schema = expected_colr012_schema();
        schema.apply_to_stack();
        assert!(msg_validate());
        let xml = msg_serialize("XML").expect("serialize colr.012");
        let parsed = parse_message("colr.012", &xml).expect("parse colr.012");
        assert_eq!(parsed.field_text("Substitution/Haircut"), Some("50"));
        let materialized = Colr012::from_parsed(&parsed).expect("materialize colr.012 into schema");
        assert_eq!(materialized, schema);
        let encoded = schema.encode();
        let mut cursor = encoded.as_slice();
        let decoded = Colr012::decode(&mut cursor).expect("decode");
        assert_eq!(schema, decoded);
    }
    #[test]
    fn colr007_fixture_is_not_supported() {
        reset();
        assert!(parse_message("colr.007", COLR007_FIXTURE.as_bytes()).is_err());
    }
    #[test]
    fn colr012_fixture_parses_into_schema() {
        reset();
        let parsed =
            parse_message("colr.012", COLR012_FIXTURE.as_bytes()).expect("parse colr.012 fixture");
        let schema = Colr012::from_parsed(&parsed).expect("materialize colr.012 from fixture");
        assert_eq!(schema, expected_colr012_schema());
    }
    #[test]
    fn colr012_rejects_unknown_type() {
        reset();
        msg_create("colr.012");
        populate_colr012_minimal();
        msg_set("Substitution/Type", b"UNEXPECTED");
        assert!(!msg_validate());
    }
    #[test]
    fn versioned_sese023_supported() {
        reset();
        msg_create("sese.023.001.09");
        populate_sese023_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_sese025_supported() {
        reset();
        msg_create("sese.025.001.08");
        populate_sese025_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_colr007_is_not_supported() {
        reset();
        msg_create("colr.007.001.08");
        populate_colr012_minimal();
        assert!(!msg_validate());
    }
    #[test]
    fn versioned_colr012_supported() {
        reset();
        msg_create("colr.012.001.05");
        populate_colr012_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn versioned_pain002_supported() {
        reset();
        msg_create("pain.002.001.12");
        populate_pain002_minimal();
        assert!(msg_validate());
    }
    #[test]
    fn msg_sign_and_verify_roundtrip() {
        reset();
        msg_parse("pacs.008", b"field=value").unwrap();
        let sk_bytes = [7u8; 32];
        let sig = msg_sign(&sk_bytes);
        let pk = SigningKey::from_bytes(&sk_bytes).verifying_key();
        assert!(msg_verify_sig(&sig, pk.as_bytes()));
    }
    #[test]
    fn msg_sign_and_verify_roundtrip_dilithium() {
        use pqcrypto_mldsa::mldsa65 as dilithium;
        use pqcrypto_traits::sign::{PublicKey, SecretKey};
        reset();
        msg_parse("pacs.008", b"field=value").unwrap();
        let (pk, sk) = dilithium::keypair();
        let mut tagged = Vec::with_capacity(1 + sk.as_bytes().len());
        tagged.push(Algorithm::MlDsa as u8);
        tagged.extend_from_slice(sk.as_bytes());
        let sig = msg_sign(&tagged);
        assert!(msg_verify_sig(&sig, pk.as_bytes()));
    }
    #[test]
    fn msg_sign_and_verify_roundtrip_secp256k1() {
        use k256::ecdsa::{SigningKey, VerifyingKey};
        reset();
        msg_parse("pacs.008", b"field=value").unwrap();
        let sk = SigningKey::from_bytes(&[9u8; 32].into()).expect("sk");
        let sk_bytes = sk.to_bytes();
        let mut tagged = Vec::with_capacity(1 + sk_bytes.len());
        tagged.push(Algorithm::Secp256k1 as u8);
        tagged.extend_from_slice(sk_bytes.as_slice());
        let sig = msg_sign(&tagged);
        let pk = VerifyingKey::from(&sk);
        let pk_bytes = pk.to_encoded_point(true);
        assert!(msg_verify_sig(&sig, pk_bytes.as_bytes()));
    }
    #[test]
    fn msg_parse_xml_roundtrip() {
        reset();
        msg_parse(
            "pacs.008",
            b"<ISO20022 message=\"pacs.008\"><Field path=\"MsgId\">1</Field><Field path=\"IntrBkSttlmCcy\">USD</Field><Field path=\"IntrBkSttlmAmt\">10</Field><Field path=\"IntrBkSttlmDt\">2024-01-01</Field><Field path=\"DbtrAcct\">GB82WEST12345698765432</Field><Field path=\"CdtrAcct\">GB33BUKB20201555555555</Field><Field path=\"DbtrAgt\">DEUTDEFF</Field><Field path=\"CdtrAgt\">DEUTDEFF</Field></ISO20022>"
        ).unwrap();
        assert!(msg_validate());
        let xml = msg_serialize("XML").unwrap();
        let xml_str = String::from_utf8(xml).unwrap();
        assert!(xml_str.contains("<ISO20022"));
        assert!(xml_str.contains("CdtrAgt"));
    }
    #[test]
    fn msg_parse_xml_wrapper_accepts_single_quoted_attributes() {
        reset();
        msg_parse(
            "pacs.008",
            b"<ISO20022 message='pacs.008'><Field path='MsgId'>1</Field></ISO20022>",
        )
        .expect("single-quoted internal XML wrapper attributes are well-formed");
        assert_eq!(msg_get("MsgId").as_deref(), Some(b"1".as_slice()));
    }
    #[test]
    fn msg_parse_xml_wrapper_rejects_malformed_attribute_and_tag_shapes() {
        let cases: &[(&str, &[u8])] = &[
            (
                "message attribute substring",
                br#"<ISO20022 xmessage="pacs.008"><Field path="MsgId">1</Field></ISO20022>"#,
            ),
            (
                "path attribute substring",
                br#"<ISO20022 message="pacs.008"><Field xpath="MsgId">1</Field></ISO20022>"#,
            ),
            (
                "unknown root attribute",
                br#"<ISO20022 message="pacs.008" extra="x"><Field path="MsgId">1</Field></ISO20022>"#,
            ),
            (
                "unknown field attribute",
                br#"<ISO20022 message="pacs.008"><Field path="MsgId" extra="x">1</Field></ISO20022>"#,
            ),
            (
                "unsupported field encoding",
                br#"<ISO20022 message="pacs.008"><Field path="MsgId" encoding="hex">31</Field></ISO20022>"#,
            ),
            (
                "field element prefix match",
                br#"<ISO20022 message="pacs.008"><Fieldx path="MsgId">1</Field></ISO20022>"#,
            ),
            (
                "leading non-root markup",
                br#"<Ignored/><ISO20022 message="pacs.008"></ISO20022>"#,
            ),
            (
                "trailing non-root markup",
                br#"<ISO20022 message="pacs.008"></ISO20022><Ignored/>"#,
            ),
            (
                "entity-decoded invalid field path",
                br#"<ISO20022 message="pacs.008"><Field path="Msg&lt;Id">1</Field></ISO20022>"#,
            ),
            (
                "empty field index",
                br#"<ISO20022 message="pacs.008"><Field path="TxInf[]">1</Field></ISO20022>"#,
            ),
            (
                "self-closing field",
                br#"<ISO20022 message="pacs.008"><Field path="MsgId"/></ISO20022>"#,
            ),
            (
                "raw nested field markup",
                br#"<ISO20022 message="pacs.008"><Field path="MsgId"><b>1</b></Field></ISO20022>"#,
            ),
        ];
        for (label, xml) in cases {
            reset();
            let err = match msg_parse("pacs.008", xml) {
                Ok(()) => panic!("{label} must fail internal XML wrapper parsing"),
                Err(err) => err,
            };
            assert!(
                matches!(err, MsgError::InvalidFormat),
                "{label} returned {err:?}"
            );
            assert!(super::MESSAGE_STACK.with(|stack| stack.borrow().is_empty()));
        }
    }
    #[test]
    fn msg_serialize_xml_base64_encodes_utf8_that_is_not_xml_text() {
        reset();
        let msg_id = "bad-\u{1}-xml";
        msg_create("pacs.008");
        msg_set("MsgId", msg_id.as_bytes());
        let xml = msg_serialize("XML").expect("XML serializes");
        let xml_str = String::from_utf8(xml.clone()).expect("internal XML is UTF-8");
        assert!(xml_str.contains(r#"<Field path="MsgId" encoding="base64">"#));
        assert!(!xml_str.contains(msg_id));
        reset();
        msg_parse("pacs.008", &xml).expect("base64 internal XML parses");
        assert_eq!(msg_get("MsgId").as_deref(), Some(msg_id.as_bytes()));
    }
    #[test]
    fn signature_blocks_marked_as_ignored() {
        reset();
        let xml = include_str!("assets/text_v1/pacs008_signature.xml");
        msg_parse("pacs.008", xml.as_bytes()).expect("parse pacs.008 with signature");
        assert_eq!(
            msg_get("Document/FIToFICstmrCdtTrf/Sgntr/@ignored").as_deref(),
            Some(SIGNATURE_IGNORED_VALUE),
        );
        assert!(msg_get("Document/FIToFICstmrCdtTrf/Sgntr/SignatureValue").is_none());
        assert!(msg_validate());
    }
    #[test]
    fn amount_encode_decode_roundtrip() {
        let enc = encode_amount(42);
        assert_eq!(enc, b"42".to_vec());
        assert_eq!(decode_amount(&enc), Some(42));
        assert!(decode_amount(b"12a").is_none());
    }
    #[test]
    fn validate_format_dispatches() {
        assert!(validate_format("IBAN", b"GB82WEST12345698765432"));
        assert!(!validate_format("IBAN", b"GB82WEST12345698765433"));
        assert!(!validate_format("IBAN", b"NO938601111794"));
        assert!(validate_format("BIC", b"DEUTDEFF"));
        assert!(validate_format("NUMERIC", b"12345"));
        assert!(!validate_format("NUMERIC", b"12a"));
    }
    #[test]
    fn base64_encode_decode_roundtrip() {
        let data = b"hello world";
        let enc = encode_base64(data);
        assert_eq!(enc, b"aGVsbG8gd29ybGQ=".to_vec());
        assert_eq!(decode_base64(&enc), Some(data.to_vec()));
    }
    #[test]
    fn decode_base64_rejects_invalid() {
        assert!(decode_base64(b"@@@=").is_none());
    }
    #[test]
    fn decode_base64_into_reuses_buffer() {
        let mut out = Vec::new();
        decode_base64_into(b"SGVsbG8=", &mut out).unwrap();
        assert_eq!(out, b"Hello");
    }
    #[test]
    fn decode_base64_into_rejects_invalid() {
        let mut out = Vec::new();
        assert!(decode_base64_into(b"@@@=", &mut out).is_none());
    }
}
