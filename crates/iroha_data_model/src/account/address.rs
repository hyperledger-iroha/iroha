//! Address encoding utilities for accounts.
use core::{
    convert::{TryFrom, TryInto},
    fmt,
    str::FromStr,
};
use std::{
    io::Write,
    sync::{
        Arc, Condvar, LazyLock, Mutex, OnceLock, RwLock,
        atomic::{AtomicU16, Ordering},
    },
    thread::ThreadId,
};

use blake2::{
    Blake2bVar, Blake2sMac,
    digest::{Mac, Update, VariableOutput, typenum::U32},
};
use bs58;
use hex;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_schema::{Ident, IntoSchema, MetaMap, Metadata, TypeId, VecMeta};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self as ncore, Archived},
};
use thiserror::Error;

use super::{
    AccountController, AccountId, MultisigMember, MultisigPolicy, MultisigPolicyError,
    curve::{CurveId, CurveRegistryError},
};
use crate::{domain::DomainId, error::ParseError, name};

#[cfg(feature = "json")]
pub mod compliance_vectors;
#[cfg(feature = "json")]
pub mod vectors;

/// Fallback default domain label used before configuration overrides.
/// Built-in implicit domain label used when configuration does not override it.
pub const DEFAULT_DOMAIN_NAME: &str = "default";
/// Alias for [`DEFAULT_DOMAIN_NAME`] retained for transition while configuration wiring lands.
pub const DEFAULT_DOMAIN_NAME_FALLBACK: &str = DEFAULT_DOMAIN_NAME;

static DEFAULT_DOMAIN_LABEL: LazyLock<RwLock<Arc<str>>> =
    LazyLock::new(|| RwLock::new(Arc::<str>::from(DEFAULT_DOMAIN_NAME_FALLBACK)));

#[derive(Default)]
struct DefaultDomainLockState {
    owner: Option<ThreadId>,
    depth: usize,
}

static DEFAULT_DOMAIN_LOCK: LazyLock<(Mutex<DefaultDomainLockState>, Condvar)> =
    LazyLock::new(|| {
        (
            Mutex::new(DefaultDomainLockState::default()),
            Condvar::new(),
        )
    });

/// Guard that serializes mutations to the default domain label and restores the previous value when requested.
#[derive(Debug)]
pub(crate) struct DefaultDomainGuard {
    release_on_drop: bool,
    original: Option<Arc<str>>,
}

impl DefaultDomainGuard {
    fn enter(label: Option<&str>) -> Self {
        let current = std::thread::current().id();
        let (lock, cvar) = &*DEFAULT_DOMAIN_LOCK;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        while state.owner.is_some() && state.owner != Some(current) {
            state = cvar
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }

        if state.owner == Some(current) {
            state.depth = state
                .depth
                .checked_add(1)
                .expect("default domain guard recursion overflow");
            return Self {
                release_on_drop: false,
                original: None,
            };
        }

        state.owner = Some(current);
        state.depth = 1;
        drop(state);

        let original = label.map(|_| default_domain_name());
        if let Some(label) = label {
            let _ = set_default_domain_name(label.to_owned());
        }

        Self {
            release_on_drop: label.is_some(),
            original,
        }
    }
}

impl Drop for DefaultDomainGuard {
    fn drop(&mut self) {
        let current = std::thread::current().id();
        let (lock, cvar) = &*DEFAULT_DOMAIN_LOCK;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if state.owner != Some(current) {
            return;
        }

        if state.depth > 1 {
            state.depth -= 1;
            return;
        }

        state.owner = None;
        state.depth = 0;
        drop(state);

        if self.release_on_drop
            && let Some(original) = self.original.take()
        {
            let _ = set_default_domain_name(original.as_ref().to_owned());
        }

        cvar.notify_one();
    }
}

/// Error returned when configuring the default domain label fails.
#[derive(Debug, Clone, Copy, Error)]
pub enum DefaultDomainLabelError {
    /// Supplied label is not a valid domain name according to Name normalization rules.
    #[error("default domain label must be a valid domain name: {0}")]
    InvalidName(#[from] ParseError),
}

/// Obtain the configured default domain label (or the fallback when unset).
#[must_use]
pub fn default_domain_name() -> Arc<str> {
    DEFAULT_DOMAIN_LABEL
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Configure the default domain label used when encoding implicit-domain addresses.
///
/// The provided value is normalised via [`Name`] to match on-chain domain identifiers.
///
/// # Errors
///
/// Returns [`DefaultDomainLabelError`] when the supplied label is not a valid domain name.
pub fn set_default_domain_name(
    label: impl Into<String>,
) -> Result<Arc<str>, DefaultDomainLabelError> {
    let label = label.into();
    let canonical_label = name::canonicalize_domain_label(&label)?;
    let canonical = Arc::<str>::from(canonical_label);
    {
        let mut guard = DEFAULT_DOMAIN_LABEL
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = canonical.clone();
    }
    Ok(canonical)
}

pub(crate) fn default_domain_guard(label: Option<&str>) -> DefaultDomainGuard {
    DefaultDomainGuard::enter(label)
}

/// Obtain the currently configured chain discriminant (IH58 network prefix).
#[must_use]
pub fn chain_discriminant() -> u16 {
    CHAIN_DISCRIMINANT.load(Ordering::Relaxed)
}

/// Set the global chain discriminant / IH58 prefix, returning the previous value.
pub fn set_chain_discriminant(discriminant: u16) -> u16 {
    CHAIN_DISCRIMINANT.swap(discriminant, Ordering::Relaxed)
}

const LOCAL_DOMAIN_KEY: &[u8] = b"SORA-LOCAL-K:v1";
const IH58_CHECKSUM_PREFIX: &[u8] = b"IH58PRE";
const HEADER_VERSION_V1: u8 = 0;
const HEADER_NORM_VERSION_V1: u8 = 1;
const COMPRESSED_SENTINEL: &str = "snx1";
const COMPRESSED_CHECKSUM_LEN: usize = 6;
const BECH32M_CONST: u32 = 0x2bc8_30a3;

const COMPRESSED_BASE_U8: u8 = 105;
const COMPRESSED_BASE: u32 = COMPRESSED_BASE_U8 as u32;
const DEFAULT_CHAIN_DISCRIMINANT: u16 = 0x02F1;

static CHAIN_DISCRIMINANT: AtomicU16 = AtomicU16::new(DEFAULT_CHAIN_DISCRIMINANT);

/// Canonical representation of an account address payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAddress {
    header: AddressHeader,
    domain: DomainSelector,
    controller: ControllerPayload,
}

/// Supported textual encodings for an account address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountAddressFormat {
    /// IH58 string representation containing the network prefix.
    IH58 {
        #[doc = "Network prefix encoded in the IH58 string."]
        network_prefix: u16,
    },
    /// Sora-only compressed alphabet using the `snx1` sentinel.
    Compressed,
    /// Canonical hexadecimal encoding prefixed with `0x`.
    CanonicalHex,
}

/// Classification of the domain selector embedded in an [`AccountAddress`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressDomainKind {
    /// Selector references the configured default domain.
    Default,
    /// Selector contains the 12-byte local digest derived from a domain label.
    LocalDigest12,
    /// Selector references a global registry record.
    GlobalRegistry,
}

impl AddressDomainKind {
    /// Stable textual label for logs, telemetry, and CLI output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::LocalDigest12 => "local12",
            Self::GlobalRegistry => "global",
        }
    }
}

impl AccountAddress {
    /// Construct from an [`AccountId`] assuming a single-key controller.
    ///
    /// Construct the canonical byte representation of the address payload.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the account cannot be represented with the
    /// supported address header or controller payload variants.
    pub fn from_account_id(account: &AccountId) -> Result<Self, AccountAddressError> {
        let (class, controller) = ControllerPayload::from_account_controller(account.controller())?;
        let header = AddressHeader::new(HEADER_VERSION_V1, class, HEADER_NORM_VERSION_V1)?;
        let domain = DomainSelector::from_domain(account.domain())?;
        Ok(Self {
            header,
            domain,
            controller,
        })
    }

    /// Encode the payload as IH58 using the provided network prefix.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical payload construction or IH58
    /// encoding fails.
    pub fn to_ih58(&self, network_prefix: u16) -> Result<String, AccountAddressError> {
        let canonical = self.canonical_bytes()?;
        encode_ih58(network_prefix, &canonical)
    }

    /// Encode the payload using the Sora-only compressed alphabet.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical payload construction or compressed
    /// encoding fails.
    pub fn to_compressed_sora(&self) -> Result<String, AccountAddressError> {
        let canonical = self.canonical_bytes()?;
        let digits = encode_base_n(&canonical, COMPRESSED_BASE)?;
        let checksum_digits = compressed_checksum_digits(&canonical);
        let alphabet = compressed_alphabet();

        let mut out =
            String::with_capacity(COMPRESSED_SENTINEL.len() + digits.len() + checksum_digits.len());
        out.push_str(COMPRESSED_SENTINEL);
        for digit in digits.iter().copied() {
            out.push_str(alphabet[usize::from(digit)]);
        }
        for digit in checksum_digits.iter().copied() {
            out.push_str(alphabet[usize::from(digit)]);
        }
        Ok(out)
    }

    /// Encode the payload using the Sora-only compressed alphabet with full-width kana.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical payload construction or compressed
    /// encoding fails.
    pub fn to_compressed_sora_fullwidth(&self) -> Result<String, AccountAddressError> {
        let canonical = self.canonical_bytes()?;
        let digits = encode_base_n(&canonical, COMPRESSED_BASE)?;
        let checksum_digits = compressed_checksum_digits(&canonical);
        let alphabet = compressed_alphabet_fullwidth();

        let mut out =
            String::with_capacity(COMPRESSED_SENTINEL.len() + digits.len() + checksum_digits.len());
        out.push_str(COMPRESSED_SENTINEL);
        for digit in digits.iter().copied() {
            out.push_str(alphabet[usize::from(digit)]);
        }
        for digit in checksum_digits.iter().copied() {
            out.push_str(alphabet[usize::from(digit)]);
        }
        Ok(out)
    }

    /// Classify the embedded domain selector.
    #[must_use]
    pub const fn domain_kind(&self) -> AddressDomainKind {
        match &self.domain {
            DomainSelector::Default => AddressDomainKind::Default,
            DomainSelector::Local12(_) => AddressDomainKind::LocalDigest12,
            DomainSelector::Global { .. } => AddressDomainKind::GlobalRegistry,
        }
    }

    /// Return the raw Local-12 selector digest when the address targets a non-default domain.
    #[must_use]
    pub fn local12_digest(&self) -> Option<[u8; 12]> {
        match &self.domain {
            DomainSelector::Local12(bytes) => Some(*bytes),
            _ => None,
        }
    }

    /// Decode an IH58 representation back into an address payload.
    ///
    /// If `expected_prefix` is provided, the decoded network prefix must match.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if decoding fails or the network prefix does not
    /// match the expected value (when provided).
    pub fn from_ih58(
        encoded: &str,
        expected_prefix: Option<u16>,
    ) -> Result<Self, AccountAddressError> {
        let (prefix, canonical) = decode_ih58(encoded)?;
        if let Some(expect) = expected_prefix
            && prefix != expect
        {
            return Err(AccountAddressError::UnexpectedNetworkPrefix {
                expected: expect,
                found: prefix,
            });
        }
        Self::from_canonical_bytes(&canonical)
    }

    /// Parse an address payload from its canonical byte representation.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the byte slice does not contain a valid
    /// canonical representation.
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Self, AccountAddressError> {
        if bytes.is_empty() {
            return Err(AccountAddressError::InvalidLength);
        }
        let header = AddressHeader::decode(bytes[0])?;
        let mut cursor = 1;
        let domain = DomainSelector::decode(bytes, &mut cursor)?;
        if matches!(domain, DomainSelector::Local12(_)) {
            ensure_local_selector_boundary(bytes, cursor)?;
        }
        let controller = ControllerPayload::decode(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(AccountAddressError::UnexpectedTrailingBytes);
        }
        Ok(Self {
            header,
            domain,
            controller,
        })
    }

    /// Decode the compressed Sora-only representation.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the string lacks the compressed sentinel,
    /// has an invalid alphabet symbol, or the checksum does not validate.
    pub fn from_compressed_sora(encoded: &str) -> Result<Self, AccountAddressError> {
        if !encoded.starts_with(COMPRESSED_SENTINEL) {
            return Err(AccountAddressError::MissingCompressedSentinel);
        }
        let payload = &encoded[COMPRESSED_SENTINEL.len()..];
        let digits = compressed_to_digits(payload)?;
        if digits.len() <= COMPRESSED_CHECKSUM_LEN {
            return Err(AccountAddressError::CompressedTooShort);
        }
        let digits_len = digits.len() - COMPRESSED_CHECKSUM_LEN;
        let data_digits = &digits[..digits_len];
        let checksum_digits = &digits[digits_len..];

        let canonical = decode_base_n(data_digits, COMPRESSED_BASE)?;
        let expected = compressed_checksum_digits(&canonical);
        if expected.as_ref() != checksum_digits {
            return Err(AccountAddressError::ChecksumMismatch);
        }

        Self::from_canonical_bytes(&canonical)
    }

    /// Parse an address string in any supported format.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the input cannot be decoded as IH58,
    /// compressed, or canonical hex (or if the IH58 prefix mismatches expectations).
    pub fn parse_any(
        input: &str,
        expected_prefix: Option<u16>,
    ) -> Result<(Self, AccountAddressFormat), AccountAddressError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(AccountAddressError::InvalidLength);
        }
        if trimmed.starts_with(COMPRESSED_SENTINEL) {
            let address = Self::from_compressed_sora(trimmed)?;
            return Ok((address, AccountAddressFormat::Compressed));
        }
        if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            let body = &trimmed[2..];
            let bytes = hex::decode(body).map_err(|_| AccountAddressError::InvalidHexAddress)?;
            let address = Self::from_canonical_bytes(&bytes)?;
            return Ok((address, AccountAddressFormat::CanonicalHex));
        }
        if trimmed.len() > 1 {
            match decode_ih58(trimmed) {
                Ok((prefix, canonical)) => {
                    if let Some(expect) = expected_prefix
                        && prefix != expect
                    {
                        return Err(AccountAddressError::UnexpectedNetworkPrefix {
                            expected: expect,
                            found: prefix,
                        });
                    }
                    let address = Self::from_canonical_bytes(&canonical)?;
                    return Ok((
                        address,
                        AccountAddressFormat::IH58 {
                            network_prefix: prefix,
                        },
                    ));
                }
                Err(AccountAddressError::ChecksumMismatch) => {
                    return Err(AccountAddressError::ChecksumMismatch);
                }
                Err(_) => {}
            }
        }
        Err(AccountAddressError::UnsupportedAddressFormat)
    }

    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if encoding the controller payload fails.
    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, AccountAddressError> {
        let mut bytes = Vec::with_capacity(1 + DOMAIN_SELECTOR_MAX_LEN + CONTROLLER_MAX_LEN);
        bytes.push(self.header.encode());
        self.domain.encode_into(&mut bytes);
        self.controller.encode_into(&mut bytes)?;
        Ok(bytes)
    }

    /// Return the canonical bytes encoded as a lowercase hex string with `0x` prefix.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical byte construction fails.
    pub fn canonical_hex(&self) -> Result<String, AccountAddressError> {
        let canonical = self.canonical_bytes()?;
        Ok(format!("0x{}", hex::encode(canonical)))
    }

    pub(crate) fn ensure_domain_matches(
        &self,
        domain: &DomainId,
    ) -> Result<(), AccountAddressError> {
        if self.domain.matches_domain(domain) {
            Ok(())
        } else {
            Err(AccountAddressError::DomainMismatch)
        }
    }

    pub(crate) fn to_account_controller(&self) -> Result<AccountController, AccountAddressError> {
        self.controller.to_account_controller()
    }
}

impl FromStr for AccountAddress {
    type Err = AccountAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_any(s, None).map(|(address, _)| address)
    }
}

impl TypeId for AccountAddress {
    fn id() -> Ident {
        std::any::type_name::<Self>().to_owned()
    }
}

impl IntoSchema for AccountAddress {
    fn type_name() -> Ident {
        "AccountAddress".to_owned()
    }

    fn update_schema_map(map: &mut MetaMap) {
        if !map.insert::<Self>(Metadata::Vec(VecMeta {
            ty: core::any::TypeId::of::<u8>(),
        })) {
            return;
        }
        <u8 as IntoSchema>::update_schema_map(map);
    }
}

fn account_address_norito_error(err: AccountAddressError) -> ncore::Error {
    ncore::Error::Message(err.to_string())
}

impl NoritoSerialize for AccountAddress {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), ncore::Error> {
        let canonical = self
            .canonical_bytes()
            .map_err(account_address_norito_error)?;
        <Vec<u8> as NoritoSerialize>::serialize(&canonical, writer)
    }
}

impl<'de> NoritoDeserialize<'de> for AccountAddress {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("archived AccountAddress must contain canonical bytes")
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = <Vec<u8> as NoritoDeserialize>::deserialize(archived.cast());
        AccountAddress::from_canonical_bytes(&bytes).map_err(account_address_norito_error)
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for AccountAddress {
    fn json_serialize(&self, out: &mut String) {
        let canonical = self
            .canonical_hex()
            .expect("AccountAddress must produce canonical hex");
        json::write_json_string(&canonical, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for AccountAddress {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let literal = parser.parse_string()?;
        AccountAddress::from_str(&literal).map_err(|err| json::Error::Message(err.to_string()))
    }
}

const LOCAL_SELECTOR_DIGEST_LEN: usize = 12;
const DOMAIN_SELECTOR_MAX_LEN: usize = 1 + LOCAL_SELECTOR_DIGEST_LEN;
const CONTROLLER_MAX_LEN: usize = 1024;
const CONTROLLER_SINGLE_KEY_TAG: u8 = 0x00;
const CONTROLLER_MULTISIG_TAG: u8 = 0x01;
const CONTROLLER_MULTISIG_MEMBER_MAX: usize = 255;

const fn is_valid_controller_tag(byte: u8) -> bool {
    matches!(byte, CONTROLLER_SINGLE_KEY_TAG | CONTROLLER_MULTISIG_TAG)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AddressHeader {
    version: u8,
    class: AddressClass,
    norm_version: u8,
    ext_flag: bool,
}

impl AddressHeader {
    fn new(
        version: u8,
        class: AddressClass,
        norm_version: u8,
    ) -> Result<Self, AccountAddressError> {
        if version > 0b111 {
            return Err(AccountAddressError::InvalidHeaderVersion(version));
        }
        if norm_version > 0b11 {
            return Err(AccountAddressError::InvalidNormVersion(norm_version));
        }
        Ok(Self {
            version,
            class,
            norm_version,
            ext_flag: false,
        })
    }

    fn encode(self) -> u8 {
        let class_bits = (self.class as u8) & 0b11;
        (self.version << 5)
            | (class_bits << 3)
            | ((self.norm_version & 0b11) << 1)
            | u8::from(self.ext_flag)
    }

    fn decode(byte: u8) -> Result<Self, AccountAddressError> {
        let version = byte >> 5;
        let class_bits = (byte >> 3) & 0b11;
        let norm_version = (byte >> 1) & 0b11;
        let ext_flag = (byte & 1) == 1;
        if ext_flag {
            return Err(AccountAddressError::UnexpectedExtensionFlag);
        }
        let class = match class_bits {
            0 => AddressClass::SingleKey,
            1 => AddressClass::MultiSig,
            other => return Err(AccountAddressError::UnknownAddressClass(other)),
        };
        Self::new(version, class, norm_version)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddressClass {
    SingleKey = 0,
    #[allow(dead_code)]
    MultiSig = 1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DomainSelector {
    Default,
    Local12([u8; 12]),
    #[allow(dead_code)]
    Global {
        registry_id: u32,
    },
}

impl DomainSelector {
    fn canonical_domain(domain: &DomainId) -> Result<String, AccountAddressError> {
        name::canonicalize_domain_label(domain.name().as_ref())
            .map_err(|err| AccountAddressError::InvalidDomainLabel(err.reason()))
    }

    fn from_domain(domain: &DomainId) -> Result<Self, AccountAddressError> {
        let canonical = Self::canonical_domain(domain)?;
        let default_label = default_domain_name();
        if canonical == default_label.as_ref() {
            Ok(Self::Default)
        } else {
            Ok(Self::Local12(compute_local_digest(&canonical)))
        }
    }

    fn encode_into(&self, out: &mut Vec<u8>) {
        match self {
            Self::Default => out.push(0x00),
            Self::Local12(bytes) => {
                out.push(0x01);
                out.extend_from_slice(bytes);
            }
            Self::Global { registry_id } => {
                out.push(0x02);
                out.extend_from_slice(&registry_id.to_be_bytes());
            }
        }
    }

    fn decode(bytes: &[u8], cursor: &mut usize) -> Result<Self, AccountAddressError> {
        let tag = *bytes
            .get(*cursor)
            .ok_or(AccountAddressError::InvalidLength)?;
        *cursor += 1;
        match tag {
            0x00 => Ok(Self::Default),
            0x01 => {
                let remaining = bytes.len().saturating_sub(*cursor);
                if remaining < 12 {
                    return Err(AccountAddressError::LocalDigestTooShort {
                        expected: 12,
                        found: remaining,
                    });
                }
                let digest = bytes
                    .get(*cursor..*cursor + 12)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += 12;
                let mut buf = [0u8; 12];
                buf.copy_from_slice(digest);
                Ok(Self::Local12(buf))
            }
            0x02 => {
                let raw = bytes
                    .get(*cursor..*cursor + 4)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += 4;
                let registry_id = u32::from_be_bytes(raw.try_into().unwrap());
                Ok(Self::Global { registry_id })
            }
            other => Err(AccountAddressError::UnknownDomainTag(other)),
        }
    }

    fn matches_domain(&self, domain: &DomainId) -> bool {
        let Ok(canonical) = Self::canonical_domain(domain) else {
            return false;
        };
        match self {
            Self::Default => canonical == default_domain_name().as_ref(),
            Self::Local12(expected) => compute_local_digest(&canonical) == *expected,
            Self::Global { .. } => true,
        }
    }
}

fn ensure_local_selector_boundary(
    bytes: &[u8],
    controller_cursor: usize,
) -> Result<(), AccountAddressError> {
    match bytes.get(controller_cursor) {
        Some(&tag) if is_valid_controller_tag(tag) => Ok(()),
        Some(_) => Ok(()),
        None => {
            let digest_start = controller_cursor.saturating_sub(LOCAL_SELECTOR_DIGEST_LEN);
            let found = bytes
                .len()
                .saturating_sub(digest_start)
                .min(LOCAL_SELECTOR_DIGEST_LEN);
            Err(AccountAddressError::LocalDigestTooShort {
                expected: LOCAL_SELECTOR_DIGEST_LEN,
                found,
            })
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ControllerPayload {
    SingleKey {
        curve: CurveId,
        public_key: PublicKey,
    },
    MultiSig(MultisigPayload),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MultisigPayload {
    version: u8,
    threshold: u16,
    members: Vec<MultisigMemberPayload>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MultisigMemberPayload {
    curve: CurveId,
    weight: u16,
    public_key: PublicKey,
}

impl ControllerPayload {
    fn from_account_controller(
        controller: &AccountController,
    ) -> Result<(AddressClass, Self), AccountAddressError> {
        match controller {
            AccountController::Single(key) => {
                let (algorithm, _) = key.to_bytes();
                let curve =
                    CurveId::try_from_algorithm(algorithm).map_err(AccountAddressError::from)?;
                Ok((
                    AddressClass::SingleKey,
                    Self::SingleKey {
                        curve,
                        public_key: key.clone(),
                    },
                ))
            }
            AccountController::Multisig(policy) => {
                if policy.members().len() > CONTROLLER_MULTISIG_MEMBER_MAX {
                    return Err(AccountAddressError::MultisigMemberOverflow(
                        policy.members().len(),
                    ));
                }
                let mut members = Vec::with_capacity(policy.members().len());
                for member in policy.members() {
                    let (algorithm, _) = member.public_key().to_bytes();
                    let curve = CurveId::try_from_algorithm(algorithm)
                        .map_err(AccountAddressError::from)?;
                    members.push(MultisigMemberPayload {
                        curve,
                        weight: member.weight(),
                        public_key: member.public_key().clone(),
                    });
                }
                Ok((
                    AddressClass::MultiSig,
                    Self::MultiSig(MultisigPayload {
                        version: policy.version(),
                        threshold: policy.threshold(),
                        members,
                    }),
                ))
            }
        }
    }

    fn encode_into(&self, out: &mut Vec<u8>) -> Result<(), AccountAddressError> {
        match self {
            Self::SingleKey { curve, public_key } => {
                out.push(CONTROLLER_SINGLE_KEY_TAG);
                out.push(curve.as_u8());
                let (_alg, payload) = public_key.to_bytes();
                if payload.len() > u8::MAX as usize {
                    let reported = u16::try_from(payload.len()).unwrap_or(u16::MAX);
                    return Err(AccountAddressError::KeyPayloadTooLong(reported));
                }
                let length =
                    u8::try_from(payload.len()).expect("payload length bounded by prior check");
                out.push(length);
                out.extend_from_slice(payload);
                Ok(())
            }
            Self::MultiSig(payload) => {
                out.push(CONTROLLER_MULTISIG_TAG);
                out.push(payload.version);
                out.extend_from_slice(&payload.threshold.to_be_bytes());
                let member_count = u8::try_from(payload.members.len()).map_err(|_| {
                    AccountAddressError::MultisigMemberOverflow(payload.members.len())
                })?;
                out.push(member_count);
                for member in &payload.members {
                    out.push(member.curve.as_u8());
                    out.extend_from_slice(&member.weight.to_be_bytes());
                    let (_alg, key_bytes) = member.public_key.to_bytes();
                    let length = u16::try_from(key_bytes.len())
                        .map_err(|_| AccountAddressError::KeyPayloadTooLong(u16::MAX))?;
                    out.extend_from_slice(&length.to_be_bytes());
                    out.extend_from_slice(key_bytes);
                }
                Ok(())
            }
        }
    }

    fn decode(bytes: &[u8], cursor: &mut usize) -> Result<Self, AccountAddressError> {
        let tag = *bytes
            .get(*cursor)
            .ok_or(AccountAddressError::InvalidLength)?;
        *cursor += 1;
        match tag {
            CONTROLLER_SINGLE_KEY_TAG => {
                let curve_raw = *bytes
                    .get(*cursor)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += 1;
                let curve = CurveId::try_from(curve_raw).map_err(AccountAddressError::from)?;
                let len = *bytes
                    .get(*cursor)
                    .ok_or(AccountAddressError::InvalidLength)? as usize;
                *cursor += 1;
                let payload = bytes
                    .get(*cursor..*cursor + len)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += len;
                let public_key = PublicKey::from_bytes(curve.algorithm(), payload)
                    .map_err(|_| AccountAddressError::InvalidPublicKey)?;
                Ok(Self::SingleKey { curve, public_key })
            }
            CONTROLLER_MULTISIG_TAG => {
                let version = *bytes
                    .get(*cursor)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += 1;
                let threshold_bytes = bytes
                    .get(*cursor..*cursor + 2)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += 2;
                let threshold = u16::from_be_bytes(threshold_bytes.try_into().unwrap());
                let member_count = *bytes
                    .get(*cursor)
                    .ok_or(AccountAddressError::InvalidLength)?
                    as usize;
                *cursor += 1;
                if member_count > CONTROLLER_MULTISIG_MEMBER_MAX {
                    return Err(AccountAddressError::MultisigMemberOverflow(member_count));
                }
                let mut members = Vec::with_capacity(member_count);
                for _ in 0..member_count {
                    let curve_raw = *bytes
                        .get(*cursor)
                        .ok_or(AccountAddressError::InvalidLength)?;
                    *cursor += 1;
                    let curve = CurveId::try_from(curve_raw).map_err(AccountAddressError::from)?;
                    let weight_bytes = bytes
                        .get(*cursor..*cursor + 2)
                        .ok_or(AccountAddressError::InvalidLength)?;
                    *cursor += 2;
                    let weight = u16::from_be_bytes(weight_bytes.try_into().unwrap());
                    let key_len_bytes = bytes
                        .get(*cursor..*cursor + 2)
                        .ok_or(AccountAddressError::InvalidLength)?;
                    *cursor += 2;
                    let key_len = u16::from_be_bytes(key_len_bytes.try_into().unwrap()) as usize;
                    let payload = bytes
                        .get(*cursor..*cursor + key_len)
                        .ok_or(AccountAddressError::InvalidLength)?;
                    *cursor += key_len;
                    let public_key = PublicKey::from_bytes(curve.algorithm(), payload)
                        .map_err(|_| AccountAddressError::InvalidPublicKey)?;
                    members.push(MultisigMemberPayload {
                        curve,
                        weight,
                        public_key,
                    });
                }
                Ok(Self::MultiSig(MultisigPayload {
                    version,
                    threshold,
                    members,
                }))
            }
            other => Err(AccountAddressError::UnknownControllerTag(other)),
        }
    }

    fn to_account_controller(&self) -> Result<AccountController, AccountAddressError> {
        match self {
            Self::SingleKey { public_key, .. } => Ok(AccountController::single(public_key.clone())),
            Self::MultiSig(payload) => {
                let mut members = Vec::with_capacity(payload.members.len());
                for member in &payload.members {
                    members.push(
                        MultisigMember::new(member.public_key.clone(), member.weight)
                            .map_err(AccountAddressError::InvalidMultisigPolicy)?,
                    );
                }
                let policy =
                    MultisigPolicy::from_serialized(payload.version, payload.threshold, members)
                        .map_err(AccountAddressError::InvalidMultisigPolicy)?;
                Ok(AccountController::multisig(policy))
            }
        }
    }
}

fn compute_local_digest(label: &str) -> [u8; 12] {
    let mut mac =
        Blake2sMac::<U32>::new_from_slice(LOCAL_DOMAIN_KEY).expect("static key with valid length");
    Mac::update(&mut mac, label.as_bytes());
    let mac_bytes = mac.finalize().into_bytes();
    let mut digest = [0u8; 12];
    digest.copy_from_slice(&mac_bytes[..12]);
    digest
}

fn encode_ih58(prefix: u16, payload: &[u8]) -> Result<String, AccountAddressError> {
    let mut body = encode_ih58_prefix(prefix)?;
    body.extend_from_slice(payload);

    let mut checksum_input = Vec::with_capacity(IH58_CHECKSUM_PREFIX.len() + body.len());
    checksum_input.extend_from_slice(IH58_CHECKSUM_PREFIX);
    checksum_input.extend_from_slice(&body);

    let mut hasher = Blake2bVar::new(64).map_err(|_| AccountAddressError::HashError)?;
    hasher.update(&checksum_input);
    let mut checksum = [0u8; 64];
    hasher
        .finalize_variable(&mut checksum)
        .map_err(|_| AccountAddressError::HashError)?;

    body.extend_from_slice(&checksum[..2]);

    Ok(bs58::encode(body).into_string())
}

fn decode_ih58(encoded: &str) -> Result<(u16, Vec<u8>), AccountAddressError> {
    let decoded = bs58::decode(encoded)
        .into_vec()
        .map_err(|_| AccountAddressError::InvalidIh58Encoding)?;
    if decoded.len() < 1 + IH58_CHECKSUM_BYTES {
        return Err(AccountAddressError::InvalidLength);
    }
    let (body, checksum_bytes) = decoded.split_at(decoded.len() - IH58_CHECKSUM_BYTES);
    let (prefix, prefix_len) = decode_ih58_prefix(body)?;
    let mut checksum_input = Vec::with_capacity(IH58_CHECKSUM_PREFIX.len() + body.len());
    checksum_input.extend_from_slice(IH58_CHECKSUM_PREFIX);
    checksum_input.extend_from_slice(body);

    let mut hasher = Blake2bVar::new(64).map_err(|_| AccountAddressError::HashError)?;
    hasher.update(&checksum_input);
    let mut expected = [0u8; 64];
    hasher
        .finalize_variable(&mut expected)
        .map_err(|_| AccountAddressError::HashError)?;
    if checksum_bytes != &expected[..IH58_CHECKSUM_BYTES] {
        return Err(AccountAddressError::ChecksumMismatch);
    }

    let payload = body
        .get(prefix_len..)
        .ok_or(AccountAddressError::InvalidLength)?
        .to_vec();
    Ok((prefix, payload))
}

fn encode_ih58_prefix(prefix: u16) -> Result<Vec<u8>, AccountAddressError> {
    if prefix <= 63 {
        let byte =
            u8::try_from(prefix).map_err(|_| AccountAddressError::InvalidIh58Prefix(prefix))?;
        Ok(vec![byte])
    } else if prefix <= 16_383 {
        let lower_bits = (prefix & 0b0011_1111) | 0b0100_0000;
        let lower =
            u8::try_from(lower_bits).map_err(|_| AccountAddressError::InvalidIh58Prefix(prefix))?;
        let upper = u8::try_from(prefix >> 6)
            .map_err(|_| AccountAddressError::InvalidIh58Prefix(prefix))?;
        Ok(vec![lower, upper])
    } else {
        Err(AccountAddressError::InvalidIh58Prefix(prefix))
    }
}

fn decode_ih58_prefix(body: &[u8]) -> Result<(u16, usize), AccountAddressError> {
    let first = *body.first().ok_or(AccountAddressError::InvalidLength)?;
    if first <= 63 {
        Ok((u16::from(first), 1))
    } else if first & 0b0100_0000 != 0 {
        let second = *body.get(1).ok_or(AccountAddressError::InvalidLength)?;
        let value = (u16::from(second) << 6) | (u16::from(first) & 0b0011_1111);
        Ok((value, 2))
    } else {
        Err(AccountAddressError::InvalidIh58PrefixEncoding(first))
    }
}

const IH58_CHECKSUM_BYTES: usize = 2;

/// Stable error codes surfaced by address encoders/decoders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountAddressErrorCode {
    /// Unsupported signing algorithm requested.
    UnsupportedAlgorithm,
    /// Key payload exceeds supported length.
    KeyPayloadTooLong,
    /// Address header version outside supported range.
    InvalidHeaderVersion,
    /// Normalisation version outside supported range.
    InvalidNormVersion,
    /// Invalid IH58 prefix encountered.
    InvalidIh58Prefix,
    /// Failure while hashing canonical payload bytes.
    HashFailure,
    /// IH58 encoding failed to decode.
    InvalidIh58Encoding,
    /// Payload length invalid for the requested operation.
    InvalidLength,
    /// IH58 checksum validation failed.
    ChecksumMismatch,
    /// Canonical hexadecimal payload failed to decode.
    InvalidHexAddress,
    /// Domain selector did not match expectation.
    DomainMismatch,
    /// Domain label failed normalisation.
    InvalidDomainLabel,
    /// Network prefix did not match expectation.
    UnexpectedNetworkPrefix,
    /// Unknown address class encountered.
    UnknownAddressClass,
    /// Unknown domain selector tag encountered.
    UnknownDomainTag,
    /// Reserved extension flag set unexpectedly.
    UnexpectedExtensionFlag,
    /// Unknown controller payload tag encountered.
    UnknownControllerTag,
    /// Public key payload failed validation.
    InvalidPublicKey,
    /// Unknown curve identifier encountered.
    UnknownCurve,
    /// Canonical payload contained trailing bytes.
    UnexpectedTrailingBytes,
    /// IH58 prefix encoding was malformed.
    InvalidIh58PrefixEncoding,
    /// Compressed form missing `snx1` sentinel.
    MissingCompressedSentinel,
    /// Compressed form shorter than minimal payload.
    CompressedTooShort,
    /// Invalid character in compressed alphabet.
    InvalidCompressedChar,
    /// Invalid compressed alphabet base requested.
    InvalidCompressedBase,
    /// Digit outside compressed alphabet bounds.
    InvalidCompressedDigit,
    /// Local-domain digest shorter than the required 12-byte selector.
    LocalDigestTooShort,
    /// Address string format unsupported.
    UnsupportedAddressFormat,
    /// Multisig controller declares too many members.
    MultisigMemberOverflow,
    /// Multisig policy payload failed validation.
    InvalidMultisigPolicy,
}

impl AccountAddressErrorCode {
    /// Stable string representation suitable for telemetry/logging.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedAlgorithm => "ERR_UNSUPPORTED_ALGORITHM",
            Self::KeyPayloadTooLong => "ERR_KEY_PAYLOAD_TOO_LONG",
            Self::InvalidHeaderVersion => "ERR_INVALID_HEADER_VERSION",
            Self::InvalidNormVersion => "ERR_INVALID_NORM_VERSION",
            Self::InvalidIh58Prefix => "ERR_INVALID_IH58_PREFIX",
            Self::HashFailure => "ERR_CANONICAL_HASH_FAILURE",
            Self::InvalidIh58Encoding => "ERR_INVALID_IH58_ENCODING",
            Self::InvalidLength => "ERR_INVALID_LENGTH",
            Self::ChecksumMismatch => "ERR_CHECKSUM_MISMATCH",
            Self::InvalidHexAddress => "ERR_INVALID_HEX_ADDRESS",
            Self::DomainMismatch => "ERR_DOMAIN_MISMATCH",
            Self::InvalidDomainLabel => "ERR_INVALID_DOMAIN_LABEL",
            Self::UnexpectedNetworkPrefix => "ERR_UNEXPECTED_NETWORK_PREFIX",
            Self::UnknownAddressClass => "ERR_UNKNOWN_ADDRESS_CLASS",
            Self::UnknownDomainTag => "ERR_UNKNOWN_DOMAIN_TAG",
            Self::UnexpectedExtensionFlag => "ERR_UNEXPECTED_EXTENSION_FLAG",
            Self::UnknownControllerTag => "ERR_UNKNOWN_CONTROLLER_TAG",
            Self::InvalidPublicKey => "ERR_INVALID_PUBLIC_KEY",
            Self::UnknownCurve => "ERR_UNKNOWN_CURVE",
            Self::UnexpectedTrailingBytes => "ERR_UNEXPECTED_TRAILING_BYTES",
            Self::InvalidIh58PrefixEncoding => "ERR_INVALID_IH58_PREFIX_ENCODING",
            Self::MissingCompressedSentinel => "ERR_MISSING_COMPRESSED_SENTINEL",
            Self::CompressedTooShort => "ERR_COMPRESSED_TOO_SHORT",
            Self::InvalidCompressedChar => "ERR_INVALID_COMPRESSED_CHAR",
            Self::InvalidCompressedBase => "ERR_INVALID_COMPRESSED_BASE",
            Self::InvalidCompressedDigit => "ERR_INVALID_COMPRESSED_DIGIT",
            Self::LocalDigestTooShort => "ERR_LOCAL8_DEPRECATED",
            Self::UnsupportedAddressFormat => "ERR_UNSUPPORTED_ADDRESS_FORMAT",
            Self::MultisigMemberOverflow => "ERR_MULTISIG_MEMBER_OVERFLOW",
            Self::InvalidMultisigPolicy => "ERR_INVALID_MULTISIG_POLICY",
        }
    }
}
/// Errors raised during address construction or encoding.
#[derive(Clone, Copy, Debug, Error)]
pub enum AccountAddressError {
    /// Requested signing algorithm is not supported by the encoder.
    #[error("unsupported signing algorithm: {0}")]
    UnsupportedAlgorithm(Algorithm),
    /// The signing key payload exceeds the supported length.
    #[error("key payload too long: {0} bytes")]
    KeyPayloadTooLong(u16),
    /// Address header version is outside the supported range.
    #[error("invalid address header version: {0}")]
    InvalidHeaderVersion(u8),
    /// Normalisation version flag is outside the supported range.
    #[error("invalid normalization version: {0}")]
    InvalidNormVersion(u8),
    /// The provided Iroha Base58 network prefix cannot be encoded.
    #[error("invalid IH58 prefix: {0}")]
    InvalidIh58Prefix(u16),
    /// Failure while hashing canonical payload bytes.
    #[error("hashing error during canonicalisation")]
    HashError,
    /// The IH58 string could not be decoded from Base58.
    #[error("invalid Iroha Base58 encoding")]
    InvalidIh58Encoding,
    /// Data length is invalid for the requested operation.
    #[error("invalid length for address payload")]
    InvalidLength,
    /// IH58 checksum validation failed.
    #[error("Iroha Base58 checksum mismatch")]
    ChecksumMismatch,
    /// Canonical hexadecimal payload could not be decoded.
    #[error("invalid canonical hex account address")]
    InvalidHexAddress,
    /// Domain selector does not match the expected domain.
    #[error("account address domain does not match provided domain")]
    DomainMismatch,
    /// Domain label failed normalization.
    #[error("domain label failed normalization: {0}")]
    InvalidDomainLabel(&'static str),
    /// Network prefix did not match expectations.
    #[error("unexpected Iroha Base58 network prefix: expected {expected}, found {found}")]
    UnexpectedNetworkPrefix {
        /// Network prefix we expected to decode.
        expected: u16,
        /// Network prefix actually decoded from the string.
        found: u16,
    },
    /// Encountered an unknown address class.
    #[error("unknown address class: {0}")]
    UnknownAddressClass(u8),
    /// Encountered an unknown domain selector tag.
    #[error("unknown domain selector tag: {0}")]
    UnknownDomainTag(u8),
    /// Address header sets the reserved extension flag.
    #[error("address header reserves extension flag but it was set")]
    UnexpectedExtensionFlag,
    /// Encountered an unknown controller tag.
    #[error("unknown controller payload tag: {0}")]
    UnknownControllerTag(u8),
    /// Public key payload could not be parsed for the declared curve.
    #[error("invalid public key payload for declared curve")]
    InvalidPublicKey,
    /// Curve identifier is not recognised.
    #[error("unknown curve identifier: {0}")]
    UnknownCurve(u8),
    /// Address contains trailing bytes beyond the expected payload.
    #[error("unexpected trailing bytes in canonical payload")]
    UnexpectedTrailingBytes,
    /// IH58 prefix encoding was malformed.
    #[error("invalid IH58 prefix encoding: {0}")]
    InvalidIh58PrefixEncoding(u8),
    /// Compressed form is missing the required `snx1` sentinel prefix.
    #[error("compressed Sora address must start with \"{COMPRESSED_SENTINEL}\"")]
    MissingCompressedSentinel,
    /// Compressed form is too short to contain payload and checksum.
    #[error("compressed Sora address too short")]
    CompressedTooShort,
    /// Encountered a character outside of the compressed alphabet.
    #[error("invalid character `{0}` in compressed Sora address")]
    InvalidCompressedChar(char),
    /// The compressed alphabet base is invalid or unsupported.
    #[error("invalid compressed alphabet base")]
    InvalidCompressedBase,
    /// Encountered a digit value outside of the compressed alphabet size.
    #[error("invalid compressed digit value: {0}")]
    InvalidCompressedDigit(u8),
    /// Local-domain selector digest shorter than the 12-byte requirement.
    #[error("local domain digest too short: expected {expected} bytes, found {found}")]
    LocalDigestTooShort {
        /// Required digest length (always 12 bytes for Norm v1).
        expected: usize,
        /// Actual digest length recovered from the payload.
        found: usize,
    },
    /// Address string is not in a recognised format.
    #[error("unsupported account address format")]
    UnsupportedAddressFormat,
    /// Multisignature controller declares more members than supported.
    #[error("multisig controller has too many members: {0}")]
    MultisigMemberOverflow(usize),
    /// Multisignature controller payload failed validation.
    #[error("invalid multisig policy: {0}")]
    InvalidMultisigPolicy(#[from] MultisigPolicyError),
}

impl From<CurveRegistryError> for AccountAddressError {
    fn from(error: CurveRegistryError) -> Self {
        match error {
            CurveRegistryError::UnsupportedAlgorithm(algorithm) => {
                Self::UnsupportedAlgorithm(algorithm)
            }
            CurveRegistryError::UnknownCurveId(curve) => Self::UnknownCurve(curve),
        }
    }
}

impl AccountAddressError {
    /// Stable error code attached to this failure.
    #[must_use]
    pub const fn code(&self) -> AccountAddressErrorCode {
        match self {
            Self::UnsupportedAlgorithm(_) => AccountAddressErrorCode::UnsupportedAlgorithm,
            Self::KeyPayloadTooLong(_) => AccountAddressErrorCode::KeyPayloadTooLong,
            Self::InvalidHeaderVersion(_) => AccountAddressErrorCode::InvalidHeaderVersion,
            Self::InvalidNormVersion(_) => AccountAddressErrorCode::InvalidNormVersion,
            Self::InvalidIh58Prefix(_) => AccountAddressErrorCode::InvalidIh58Prefix,
            Self::HashError => AccountAddressErrorCode::HashFailure,
            Self::InvalidIh58Encoding => AccountAddressErrorCode::InvalidIh58Encoding,
            Self::InvalidLength => AccountAddressErrorCode::InvalidLength,
            Self::ChecksumMismatch => AccountAddressErrorCode::ChecksumMismatch,
            Self::InvalidHexAddress => AccountAddressErrorCode::InvalidHexAddress,
            Self::DomainMismatch => AccountAddressErrorCode::DomainMismatch,
            Self::InvalidDomainLabel(_) => AccountAddressErrorCode::InvalidDomainLabel,
            Self::UnexpectedNetworkPrefix { .. } => {
                AccountAddressErrorCode::UnexpectedNetworkPrefix
            }
            Self::UnknownAddressClass(_) => AccountAddressErrorCode::UnknownAddressClass,
            Self::UnknownDomainTag(_) => AccountAddressErrorCode::UnknownDomainTag,
            Self::UnexpectedExtensionFlag => AccountAddressErrorCode::UnexpectedExtensionFlag,
            Self::UnknownControllerTag(_) => AccountAddressErrorCode::UnknownControllerTag,
            Self::InvalidPublicKey => AccountAddressErrorCode::InvalidPublicKey,
            Self::UnknownCurve(_) => AccountAddressErrorCode::UnknownCurve,
            Self::UnexpectedTrailingBytes => AccountAddressErrorCode::UnexpectedTrailingBytes,
            Self::InvalidIh58PrefixEncoding(_) => {
                AccountAddressErrorCode::InvalidIh58PrefixEncoding
            }
            Self::MissingCompressedSentinel => AccountAddressErrorCode::MissingCompressedSentinel,
            Self::CompressedTooShort => AccountAddressErrorCode::CompressedTooShort,
            Self::InvalidCompressedChar(_) => AccountAddressErrorCode::InvalidCompressedChar,
            Self::InvalidCompressedBase => AccountAddressErrorCode::InvalidCompressedBase,
            Self::InvalidCompressedDigit(_) => AccountAddressErrorCode::InvalidCompressedDigit,
            Self::LocalDigestTooShort { .. } => AccountAddressErrorCode::LocalDigestTooShort,
            Self::UnsupportedAddressFormat => AccountAddressErrorCode::UnsupportedAddressFormat,
            Self::MultisigMemberOverflow(_) => AccountAddressErrorCode::MultisigMemberOverflow,
            Self::InvalidMultisigPolicy(_) => AccountAddressErrorCode::InvalidMultisigPolicy,
        }
    }

    /// Stable string identifier for this failure.
    #[must_use]
    pub const fn code_str(&self) -> &'static str {
        self.code().as_str()
    }
}

impl fmt::Display for AccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let canonical = self.canonical_bytes().map_err(|_| fmt::Error)?;
        write!(f, "0x{}", hex::encode(canonical))
    }
}

fn encode_base_n(bytes: &[u8], base: u32) -> Result<Vec<u8>, AccountAddressError> {
    if base < 2 {
        return Err(AccountAddressError::InvalidCompressedBase);
    }
    if bytes.is_empty() {
        return Ok(vec![0]);
    }
    let leading_zeros = bytes.iter().take_while(|&&b| b == 0).count();
    let mut value = bytes.to_vec();
    let mut digits = Vec::new();
    let mut start = leading_zeros;
    while start < value.len() {
        let mut remainder = 0u32;
        for byte in &mut value[start..] {
            let accumulator = (remainder << 8) | u32::from(*byte);
            let quotient = u8::try_from(accumulator / base)
                .expect("radix division quotient always fits in a byte");
            *byte = quotient;
            remainder = accumulator % base;
        }
        digits.push(
            u8::try_from(remainder).expect("remainder of division by base < 256 always fits in u8"),
        );
        while start < value.len() && value[start] == 0 {
            start += 1;
        }
    }
    digits.resize(digits.len() + leading_zeros, 0);
    if digits.is_empty() {
        digits.push(0);
    }
    digits.reverse();
    Ok(digits)
}

fn decode_base_n(digits: &[u8], base: u32) -> Result<Vec<u8>, AccountAddressError> {
    if base < 2 {
        return Err(AccountAddressError::InvalidCompressedBase);
    }
    if digits.is_empty() {
        return Err(AccountAddressError::InvalidLength);
    }
    let leading_zeros = digits.iter().take_while(|&&d| d == 0).count();
    let mut value: Vec<u8> = digits.to_vec();
    let mut bytes = Vec::new();
    let mut start = leading_zeros;
    while start < value.len() {
        let mut remainder = 0u32;
        for digit in &mut value[start..] {
            if u32::from(*digit) >= base {
                return Err(AccountAddressError::InvalidCompressedDigit(*digit));
            }
            let accumulator = remainder * base + u32::from(*digit);
            let quotient = u8::try_from(accumulator / 256)
                .expect("division by 256 produces quotient that fits in u8");
            *digit = quotient;
            remainder = accumulator % 256;
        }
        bytes.push(u8::try_from(remainder).expect("remainder modulo 256 must fit in u8"));
        while start < value.len() && value[start] == 0 {
            start += 1;
        }
    }
    bytes.resize(bytes.len() + leading_zeros, 0);
    bytes.reverse();
    Ok(bytes)
}

fn compressed_checksum_digits(canonical: &[u8]) -> [u8; COMPRESSED_CHECKSUM_LEN] {
    let data = convert_to_base32(canonical);
    bech32m_checksum(&data)
}

fn convert_to_base32(data: &[u8]) -> Vec<u8> {
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::with_capacity((data.len() * 8).div_ceil(5));
    for &byte in data {
        acc = (acc << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let value = u8::try_from((acc >> bits) & 0x1f).expect("base32 limb fits in u8");
            out.push(value);
        }
    }
    if bits > 0 {
        let value = u8::try_from((acc << (5 - bits)) & 0x1f).expect("base32 limb fits in u8");
        out.push(value);
    }
    out
}

fn bech32m_checksum(data: &[u8]) -> [u8; COMPRESSED_CHECKSUM_LEN] {
    let mut values = expand_hrp("snx");
    values.extend_from_slice(data);
    values.extend([0u8; COMPRESSED_CHECKSUM_LEN]);
    let polymod = bech32_polymod(values.iter().copied()) ^ BECH32M_CONST;
    let mut result = [0u8; COMPRESSED_CHECKSUM_LEN];
    for (index, slot) in result.iter_mut().enumerate() {
        let shift = 5 * (COMPRESSED_CHECKSUM_LEN - 1 - index);
        let value = ((polymod >> shift) & 0x1f) as u32;
        *slot = u8::try_from(value).expect("bech32 checksum limb fits in u8");
    }
    result
}

fn bech32_polymod<I>(values: I) -> u32
where
    I: Iterator<Item = u8>,
{
    const GEN: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let mut chk = 1u32;
    for value in values {
        let top = chk >> 25;
        chk = ((chk & 0x1ff_ffff) << 5) ^ u32::from(value);
        for (i, generator) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= generator;
            }
        }
    }
    chk
}

fn expand_hrp(hrp: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(hrp.len() * 2 + 1);
    for b in hrp.bytes() {
        out.push(b >> 5);
    }
    out.push(0);
    out.extend(hrp.bytes().map(|b| b & 0x1f));
    out
}

fn compressed_to_digits(payload: &str) -> Result<Vec<u8>, AccountAddressError> {
    let mut digits = Vec::new();
    let char_indices: Vec<(usize, char)> = payload.char_indices().collect();
    let mut i = 0;
    while i < char_indices.len() {
        let (start, ch) = char_indices[i];
        let next_start = if i + 1 < char_indices.len() {
            char_indices[i + 1].0
        } else {
            payload.len()
        };
        if i + 1 < char_indices.len() {
            let end = if i + 2 < char_indices.len() {
                char_indices[i + 2].0
            } else {
                payload.len()
            };
            let candidate = &payload[start..end];
            if let Some(digit) = lookup_compressed_digit(candidate) {
                digits.push(digit);
                i += 2;
                continue;
            }
        }
        let slice = &payload[start..next_start];
        if let Some(digit) = lookup_compressed_digit(slice) {
            digits.push(digit);
            i += 1;
        } else {
            return Err(AccountAddressError::InvalidCompressedChar(ch));
        }
    }
    Ok(digits)
}

fn compressed_alphabet() -> &'static [&'static str] {
    static ALPHABET: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALPHABET.get_or_init(|| {
        let mut table = Vec::with_capacity(IH58_ALPHABET.len() + SORA_KANA.len());
        table.extend_from_slice(&IH58_ALPHABET);
        table.extend_from_slice(&SORA_KANA);
        debug_assert_eq!(table.len(), COMPRESSED_BASE as usize);
        table
    })
}

fn compressed_alphabet_fullwidth() -> &'static [&'static str] {
    static ALPHABET: OnceLock<Vec<&'static str>> = OnceLock::new();
    ALPHABET.get_or_init(|| {
        let mut table = Vec::with_capacity(IH58_ALPHABET.len() + SORA_KANA_FULLWIDTH.len());
        table.extend_from_slice(&IH58_ALPHABET);
        table.extend_from_slice(&SORA_KANA_FULLWIDTH);
        debug_assert_eq!(table.len(), COMPRESSED_BASE as usize);
        table
    })
}

fn compressed_digit_table() -> &'static [(&'static str, u8)] {
    static MAP: OnceLock<Vec<(&'static str, u8)>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut table = Vec::with_capacity((COMPRESSED_BASE as usize) * 2);
        for (idx, symbol) in compressed_alphabet().iter().enumerate() {
            table.push((
                *symbol,
                u8::try_from(idx).expect("compressed alphabet length fits within u8"),
            ));
        }
        for (idx, symbol) in compressed_alphabet_fullwidth().iter().enumerate() {
            table.push((
                *symbol,
                u8::try_from(idx).expect("compressed alphabet length fits within u8"),
            ));
        }
        table.shrink_to_fit();
        table
    })
}

fn lookup_compressed_digit(symbol: &str) -> Option<u8> {
    compressed_digit_table()
        .iter()
        .find_map(|(s, value)| (*s == symbol).then_some(*value))
}

const IH58_ALPHABET: [&str; 58] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H", "J", "K",
    "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b", "c", "d", "e",
    "f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y",
    "z",
];

const SORA_KANA: [&str; 47] = [
    "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ", "ﾂ",
    "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ",
    "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
];

const SORA_KANA_FULLWIDTH: [&str; 47] = [
    "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ", "ヨ", "タ",
    "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク", "ヤ", "マ", "ケ", "フ",
    "コ", "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ", "ヱ", "ヒ", "モ", "セ", "ス",
];

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, str::FromStr, sync::Arc};

    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use proptest::prelude::*;

    use super::*;
    use crate::{domain::DomainId, name::Name};

    fn ed25519_pk() -> PublicKey {
        PublicKey::from_hex(
            Algorithm::Ed25519,
            "27c96646f2d4632d4fc241f84cbc427fbc3ecaa95becba55088d6c7b81fc5bbf",
        )
        .expect("valid ed25519 payload")
    }

    fn ed25519_pk_with(byte: u8) -> PublicKey {
        let seed = vec![byte; 32];
        let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
        public_key
    }

    const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const ED25519_NON_CANONICAL_IDENTITY: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn domain(name: &str) -> DomainId {
        DomainId::new(Name::from_str(name).expect("valid domain name"))
    }

    fn default_domain_id() -> DomainId {
        let label = default_domain_name();
        domain(label.as_ref())
    }

    fn guard_default_label() -> DefaultDomainGuard {
        default_domain_guard(None)
    }

    struct Reset(Arc<str>);

    impl Drop for Reset {
        fn drop(&mut self) {
            let _ = set_default_domain_name(self.0.as_ref().to_owned());
        }
    }

    fn account_address_for_seed(seed: u8) -> AccountAddress {
        let account = AccountId::new(default_domain_id(), ed25519_pk_with(seed));
        AccountAddress::from_account_id(&account).expect("account id encodes into an address")
    }

    fn nfkc_like_payload(compressed: &str) -> String {
        assert!(compressed.starts_with(COMPRESSED_SENTINEL));
        let payload = &compressed[COMPRESSED_SENTINEL.len()..];
        let digits =
            compressed_to_digits(payload).expect("compressed payload must decode into digits");
        let alphabet = compressed_alphabet_fullwidth();
        let mut output = String::with_capacity(compressed.len());
        output.push_str(COMPRESSED_SENTINEL);
        for digit in digits {
            output.push_str(alphabet[usize::from(digit)]);
        }
        output
    }

    fn ascii_payload_to_fullwidth(compressed: &str) -> Option<String> {
        assert!(compressed.starts_with(COMPRESSED_SENTINEL));
        let payload = &compressed[COMPRESSED_SENTINEL.len()..];
        let mut output = String::with_capacity(compressed.len());
        output.push_str(COMPRESSED_SENTINEL);
        let mut changed = false;
        for ch in payload.chars() {
            if let Some(fullwidth) = ascii_to_fullwidth_char(ch) {
                output.push(fullwidth);
                changed = true;
            } else {
                output.push(ch);
            }
        }
        changed.then_some(output)
    }

    fn ascii_str_to_fullwidth(input: &str) -> Option<String> {
        let mut output = String::with_capacity(input.len());
        for ch in input.chars() {
            let converted = ascii_to_fullwidth_char(ch)?;
            output.push(converted);
        }
        Some(output)
    }

    fn ascii_to_fullwidth_char(ch: char) -> Option<char> {
        match ch {
            '0'..='9' => char::from_u32(ch as u32 - '0' as u32 + 0xFF10),
            'A'..='Z' => char::from_u32(ch as u32 - 'A' as u32 + 0xFF21),
            'a'..='z' => char::from_u32(ch as u32 - 'a' as u32 + 0xFF41),
            _ => None,
        }
    }

    fn digits_to_payload_string(digits: &[u8], alphabet: &[&str]) -> String {
        let mut output = String::with_capacity(digits.len() * 3);
        for digit in digits {
            output.push_str(alphabet[usize::from(*digit)]);
        }
        output
    }

    #[test]
    fn local8_payloads_emit_dedicated_error_code() {
        let account = AccountId::new(domain("wonderland"), ed25519_pk());
        let mut canonical = AccountAddress::from_account_id(&account)
            .expect("account encodes")
            .canonical_bytes()
            .expect("canonical bytes");
        let digest_start = 2; // header (0) + tag (1) + digest payload
        canonical.drain(digest_start + 8..digest_start + 12);

        let err = AccountAddress::from_canonical_bytes(&canonical)
            .expect_err("short digest payload must be rejected");
        assert!(matches!(
            err,
            AccountAddressError::LocalDigestTooShort {
                expected: 12,
                found: 8
            }
        ));

        let literal = format!("0x{}@wonderland", hex::encode(&canonical));
        let parse_err = AccountId::parse(&literal).expect_err("account parsing fails");
        assert_eq!(
            parse_err.reason(),
            AccountAddressErrorCode::LocalDigestTooShort.as_str()
        );
    }

    #[test]
    fn local12_digest_is_exposed_for_non_default_domains() {
        let account = AccountId::new(domain("wonderland"), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        let digest = address
            .local12_digest()
            .expect("non-default domain must surface a digest");
        assert_eq!(digest, compute_local_digest("wonderland"));
    }

    #[test]
    fn local12_digest_absent_for_default_domain() {
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        assert!(
            address.local12_digest().is_none(),
            "default domain selectors should not report Local-12 digests"
        );
    }

    #[test]
    fn sora_kana_matches_iroha_poem_order() {
        const EXPECTED: [&str; 47] = [
            "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ",
            "ｿ", "ﾂ", "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ",
            "ﾃ", "ｱ", "ｻ", "ｷ", "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
        ];
        assert_eq!(SORA_KANA, EXPECTED);
    }

    #[test]
    fn compressed_alphabet_has_unique_symbols() {
        let mut symbols = BTreeSet::new();
        for symbol in IH58_ALPHABET {
            assert!(symbols.insert(symbol), "duplicate IH58 symbol {symbol}");
        }
        for symbol in SORA_KANA {
            assert!(
                symbols.insert(symbol),
                "duplicate compressed symbol {symbol}"
            );
        }
        assert_eq!(
            symbols.len(),
            (IH58_ALPHABET.len() + SORA_KANA.len()),
            "unexpected compressed alphabet length"
        );
    }

    #[test]
    fn compressed_golden_vectors_match_expected_strings() {
        let _guard_default = guard_default_label();
        let original = default_domain_name();
        let _reset = Reset(original.clone());
        set_default_domain_name("default").expect("restore default label");

        let vectors: [(&str, u8, &str, &str); 12] = [
            (
                "default",
                0,
                "0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201",
                "snx12QGﾈkﾀｱﾚiﾉﾘuﾛWRヱﾏxﾁSuﾁepnhｽvｶrﾓｶ9Tｹｿp3ﾇVWｳｲｾU4N5E5",
            ),
            (
                "treasury",
                1,
                "0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8",
                "snx15ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾘｻYﾃhﾓMQ9CBEﾅﾊﾈｷﾉVRｺnKRwTﾋｼqﾅWrﾎU7ｼiﾍQt1TPGNJ",
            ),
            (
                "wonderland",
                2,
                "0x0201b8ae571b79c5a80f5834da2b000120ad29ac2c12d4daaa4a2415235f2b01730bff1193dd4a6eaee29e945b01a4a212",
                "snx15ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓRKSｷﾗﾕneﾀM3Rabvﾂ1JﾚﾉｺｲﾕｹｺﾀFﾔﾇﾖFSXsﾜCHmB59S5KS",
            ),
            (
                "iroha",
                3,
                "0x0201de8b36819700c807083608e2000120ce6d4f240893505e112cdc1b83585d8efc271ea6f934c5f6a49217e27e61b9e7",
                "snx15ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓsYヰxﾎﾍﾚﾇｺﾊehjyzXGヰaｿSﾔ1kWｺｾJeﾒAWkwﾋﾐRRQQKXYE",
            ),
            (
                "alpha",
                4,
                "0x020146be2154ae86826a3fef0ec000012077143459c5b54808313cd57ded18322fc02c4616de930e0e3af578bb509bb5dc",
                "snx15ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRSﾗgPﾏrHﾔGﾀqﾂｹfoﾂHwﾉoﾊ4ﾎﾇ74ｼﾕﾎUw8JaU3ﾙJFYHVLUS",
            ),
            (
                "omega",
                5,
                "0x0201390d946885bc8416b3d30c9d000120e18cbb31e5249ff9205b72fe50e50dcc78fb80e28028bdc4c47bcf63ee61c6b8",
                "snx15ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8qfBﾌﾒaTjQpTxPﾊｦNﾚnヱvorHｷﾎkﾈEヱFﾎｻTUﾗhiVqURKRVM",
            ),
            (
                "governance",
                6,
                "0x0201989eb45a80940d187e2c908f0001208a5bd65d39ba61bde2a87ee10d242bd5575cd02bf687c4b5960d4141698dd76a",
                "snx15ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾌbﾜｸeGｵzﾙzｽﾐﾌﾉQﾃw2ﾕLDﾔｽﾙFﾙﾇﾋBTdUXﾎﾙsｽRDJCHS",
            ),
            (
                "validators",
                7,
                "0x0201e4ffa58704c69afaeb7cc2d7000120f0f80d8a09aa1276d2e605bc904137f7a52b9c4847b9b5366d4002ca4049daeb",
                "snx15ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊWTﾂfKｳmU7fWﾍXｱ2JnyﾋE4ﾕghZｱVｶｦﾇaFﾕbr8qﾑR4VEFR",
            ),
            (
                "explorer",
                8,
                "0x02013b35422c65c2a83c99c523ad00012033af4073c5815cbe5d0fec37cffe02e542302b60e24d8a7c0819f772ca6886f9",
                "snx15ｻ4nmｻaﾚﾚPvNLgｿｱv6MHdPﾓWﾍヱpeﾕFﾕmFﾌﾀKhﾉWｴeﾋbｷXMﾎ2ﾃnQﾐﾗﾑﾎBBｻC8P548",
            ),
            (
                "soranet",
                9,
                "0x0201047d9ea7f5d5dbec3f7bfc58000120cd3c119f6c81e28a2747c531f5cbe8dbc44ed8e16751bc4a467445b272db4097",
                "snx15ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvwS8JｳEnQaｿHTdﾒXZﾍvﾆazｿgﾔﾙhF9hcsﾘvNﾌヱJ9MGDNBW",
            ),
            (
                "kitsune",
                10,
                "0x0201e91933de397fd7723dc9a76c0001206e4c4188e1b8455ff3369dc163240a5d653f13a6f420fd0edbb23303bad239e7",
                "snx15ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpzﾘKmC6ｱSﾇhqｦJB1gﾙwCwﾁﾍeﾉﾔABﾆpqYﾍEﾌｼﾆﾃFNCAT97",
            ),
            (
                "da",
                11,
                "0x02016838cf5bb0ce0f3d4f380e1c00012056f02721c153689b09efafd07d8ef7bed2c4a581dd00faa118aed1d51f7a1ad6",
                "snx15ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKmgﾁﾃｻ3ｸGLpﾄﾆHnXGﾘﾑDﾃJ6ｸXﾐﾂﾊwhｶtｵｾｴﾃ9PﾖDFC3YQ",
            ),
        ];

        for (label, seed_byte, expected_canonical, expected_compressed) in vectors {
            let account = AccountId::new(domain(label), ed25519_pk_with(seed_byte));
            let address = AccountAddress::from_account_id(&account).expect("address encoding");
            let canonical = address
                .canonical_hex()
                .expect("canonical encoding must succeed");
            let compressed = address
                .to_compressed_sora()
                .expect("compressed encoding must succeed");
            assert_eq!(
                canonical, expected_canonical,
                "label={label} seed={seed_byte}"
            );
            assert_eq!(
                compressed, expected_compressed,
                "label={label} seed={seed_byte}"
            );
        }
    }

    #[test]
    fn default_domain_selector_is_zero() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let canonical = address.canonical_bytes().expect("bytes");
        assert_eq!(canonical[0] >> 5, HEADER_VERSION_V1);
        assert_eq!(canonical[1], 0x00);
    }

    #[test]
    fn non_default_domain_uses_local_digest() {
        let _guard = guard_default_label();
        let account = AccountId::new(domain("treasury"), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let canonical = address.canonical_bytes().expect("bytes");
        assert_eq!(canonical[1], 0x01);
        let mut expected = [0u8; 12];
        expected.copy_from_slice(&canonical[2..14]);
        assert_eq!(expected, compute_local_digest("treasury"));
    }

    #[test]
    fn domain_kind_distinguishes_default_and_local_selectors() {
        let _guard = guard_default_label();
        let default_account = AccountId::new(default_domain_id(), ed25519_pk_with(7));
        let default_address =
            AccountAddress::from_account_id(&default_account).expect("encode default domain");
        assert_eq!(default_address.domain_kind(), AddressDomainKind::Default);

        let local_account = AccountId::new(domain("treasury"), ed25519_pk_with(9));
        let local_address =
            AccountAddress::from_account_id(&local_account).expect("encode local domain");
        assert_eq!(
            local_address.domain_kind(),
            AddressDomainKind::LocalDigest12
        );
    }

    #[test]
    fn domain_kind_reports_global_registry_variant() {
        let _guard = guard_default_label();
        let mut address = account_address_for_seed(11);
        address.domain = DomainSelector::Global { registry_id: 42 };
        assert_eq!(address.domain_kind(), AddressDomainKind::GlobalRegistry);
    }

    #[test]
    fn domain_selector_canonicalises_before_digest() {
        let _guard = guard_default_label();
        let selectors = (
            DomainSelector::from_domain(&domain("Treasury")).expect("upper-case normalizes"),
            DomainSelector::from_domain(&domain("treasury")).expect("lower-case normalizes"),
        );
        match selectors {
            (DomainSelector::Local12(a), DomainSelector::Local12(b)) => assert_eq!(a, b),
            _ => panic!("expected Local12 selectors for non-default domains"),
        }
    }

    #[test]
    fn configurable_default_domain_label_updates_selector() {
        let _guard = guard_default_label();
        let original = default_domain_name();
        let _reset = Reset(original.clone());
        let canonical = set_default_domain_name("ledger").expect("set default label");
        let account = AccountId::new(domain(canonical.as_ref()), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let canonical_bytes = address.canonical_bytes().expect("bytes");
        assert_eq!(canonical_bytes[1], 0x00);
    }

    #[test]
    fn configurable_default_domain_label_is_canonicalised() {
        let _guard = guard_default_label();
        let original = default_domain_name();
        let _reset = Reset(original.clone());
        let canonical =
            set_default_domain_name("Ledger").expect("default domain canonicalization succeeds");
        assert_eq!(canonical.as_ref(), "ledger");
        assert_eq!(default_domain_name().as_ref(), "ledger");
    }

    #[test]
    fn ih58_encoding_has_expected_structure() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let encoded = address.to_ih58(42).expect("ih58");
        let raw = bs58::decode(encoded).into_vec().expect("base58");
        assert_eq!(raw[0], 42);
        assert_eq!(
            raw.len(),
            1 + address.canonical_bytes().unwrap().len() + IH58_CHECKSUM_BYTES
        );
    }

    #[test]
    fn account_address_encodes_secp256k1_controller() {
        let (public_key, _) = KeyPair::random_with_algorithm(Algorithm::Secp256k1).into_parts();
        let account = AccountId::new(domain("wonderland"), public_key.clone());
        let address = AccountAddress::from_account_id(&account).expect("encode secp256k1");
        let controller = address
            .to_account_controller()
            .expect("decode secp256k1 controller");
        assert_eq!(controller.single_signatory(), Some(&public_key));
    }

    #[test]
    fn ih58_round_trip_recovers_canonical_payload() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let original = AccountAddress::from_account_id(&account).expect("encode");
        let encoded = original.to_ih58(73).expect("ih58");
        let decoded =
            AccountAddress::from_ih58(&encoded, Some(73)).expect("decode succeeds with prefix");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn ih58_prefix_mismatch_fails() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let encoded = address.to_ih58(10).expect("ih58");
        let err = AccountAddress::from_ih58(&encoded, Some(11)).expect_err("prefix mismatch");
        assert!(matches!(
            err,
            AccountAddressError::UnexpectedNetworkPrefix {
                expected: 11,
                found: 10
            }
        ));
    }

    #[test]
    fn ih58_invalid_checksum_rejected() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let mut bytes = bs58::decode(address.to_ih58(0).unwrap())
            .into_vec()
            .expect("base58");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        let tampered = bs58::encode(bytes).into_string();
        let err = AccountAddress::from_ih58(&tampered, Some(0)).expect_err("checksum mismatch");
        assert!(matches!(err, AccountAddressError::ChecksumMismatch));
    }

    #[test]
    fn compressed_round_trip() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let original = AccountAddress::from_account_id(&account).expect("encode");
        let compressed = original.to_compressed_sora().expect("compressed encode");
        assert!(compressed.starts_with(COMPRESSED_SENTINEL));
        let decoded = AccountAddress::from_compressed_sora(&compressed).expect("compressed decode");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn compressed_fullwidth_round_trip() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let original = AccountAddress::from_account_id(&account).expect("encode");
        let compressed = original
            .to_compressed_sora_fullwidth()
            .expect("compressed encode");
        assert!(compressed.starts_with(COMPRESSED_SENTINEL));
        let decoded = AccountAddress::from_compressed_sora(&compressed).expect("compressed decode");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }

    proptest! {
        #[test]
        fn compressed_nfkc_payload_round_trip(seed in any::<u8>()) {
            let _guard = guard_default_label();
            let address = account_address_for_seed(seed);
            let canonical = address.canonical_bytes().expect("canonical bytes");
            let compressed = address.to_compressed_sora().expect("compressed encode");
            let nfkc_variant = nfkc_like_payload(&compressed);
            let (decoded, format) = AccountAddress::parse_any(&nfkc_variant, None).expect("nfkc payload decodes");
            prop_assert_eq!(format, AccountAddressFormat::Compressed);
            prop_assert_eq!(decoded.canonical_bytes().unwrap(), canonical);
        }
    }

    #[test]
    fn compressed_invalid_char_rejected() {
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let mut compressed = AccountAddress::from_account_id(&account)
            .expect("encode")
            .to_compressed_sora()
            .expect("compressed encode");
        let idx = COMPRESSED_SENTINEL.len();
        compressed.replace_range(idx..=idx, "!");
        let err = AccountAddress::from_compressed_sora(&compressed)
            .expect_err("invalid char should fail");
        assert!(matches!(
            err,
            AccountAddressError::InvalidCompressedChar('!')
        ));
    }

    proptest! {
        #[test]
        fn compressed_ascii_payload_fullwidth_is_rejected(seed in any::<u8>()) {
            let _guard = guard_default_label();
            let address = account_address_for_seed(seed);
            let compressed = address.to_compressed_sora().expect("compressed encode");
            let mutated = ascii_payload_to_fullwidth(&compressed);
            prop_assume!(mutated.is_some());
            let mutated = mutated.expect("payload contains ascii characters");
            let err = AccountAddress::from_compressed_sora(&mutated)
                .expect_err("IME full-width ascii must raise an error");
            prop_assert!(matches!(err, AccountAddressError::InvalidCompressedChar(_)));
        }
    }

    #[test]
    fn compressed_uppercase_sentinel_rejected() {
        let _guard = guard_default_label();
        let address = account_address_for_seed(0);
        let compressed = address.to_compressed_sora().expect("compressed encode");
        let payload = &compressed[COMPRESSED_SENTINEL.len()..];
        let tampered = format!("SNX1{payload}");
        let err = AccountAddress::from_compressed_sora(&tampered)
            .expect_err("uppercase sentinel rejected");
        assert!(matches!(
            err,
            AccountAddressError::MissingCompressedSentinel
        ));
    }

    #[test]
    fn compressed_fullwidth_sentinel_rejected() {
        let _guard = guard_default_label();
        let address = account_address_for_seed(1);
        let compressed = address.to_compressed_sora().expect("compressed encode");
        let payload = &compressed[COMPRESSED_SENTINEL.len()..];
        let sentinel = ascii_str_to_fullwidth(COMPRESSED_SENTINEL).expect("sentinel converts");
        let tampered = format!("{sentinel}{payload}");
        let err = AccountAddress::from_compressed_sora(&tampered)
            .expect_err("full-width sentinel rejected");
        assert!(matches!(
            err,
            AccountAddressError::MissingCompressedSentinel
        ));
    }

    #[test]
    fn compressed_checksum_mismatch_detected() {
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");
        let payload = &compressed[COMPRESSED_SENTINEL.len()..];
        let mut digits = compressed_to_digits(payload).expect("digits");
        let last_data_index = digits.len() - COMPRESSED_CHECKSUM_LEN - 1;
        digits[last_data_index] = (digits[last_data_index] + 1) % COMPRESSED_BASE_U8;
        let mut tampered = String::from(COMPRESSED_SENTINEL);
        for &digit in &digits {
            tampered.push_str(compressed_alphabet()[usize::from(digit)]);
        }
        let err = AccountAddress::from_compressed_sora(&tampered).expect_err("checksum mismatch");
        assert!(matches!(err, AccountAddressError::ChecksumMismatch));
    }

    proptest! {
        #[test]
        fn compressed_checksum_corruption_detected(seed in any::<u8>(), offset in any::<u8>()) {
            let _guard = guard_default_label();
            let address = account_address_for_seed(seed);
            let compressed = address.to_compressed_sora().expect("compressed encode");
            let payload = &compressed[COMPRESSED_SENTINEL.len()..];
            let mut digits = compressed_to_digits(payload).expect("digits");
            let data_len = digits.len().saturating_sub(COMPRESSED_CHECKSUM_LEN);
            prop_assume!(data_len > 0);
            let index = usize::from(offset) % data_len;
            let current = digits[index];
            digits[index] = (current + 1) % COMPRESSED_BASE_U8;
            let alphabet = compressed_alphabet();
            let tampered_payload = digits_to_payload_string(&digits, alphabet);
            let tampered = format!("{COMPRESSED_SENTINEL}{tampered_payload}");
            let err = AccountAddress::from_compressed_sora(&tampered)
                .expect_err("checksum mismatch");
            prop_assert!(matches!(err, AccountAddressError::ChecksumMismatch));
        }
    }

    #[test]
    fn canonical_decode_rejects_small_order_public_key() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let mut canonical = AccountAddress::from_account_id(&account)
            .expect("encode")
            .canonical_bytes()
            .expect("bytes");
        let offset = canonical.len() - ED25519_SMALL_ORDER_POINT.len();
        canonical[offset..].copy_from_slice(&ED25519_SMALL_ORDER_POINT);

        let err = AccountAddress::from_canonical_bytes(&canonical).unwrap_err();
        assert!(matches!(err, AccountAddressError::InvalidPublicKey));
    }

    #[test]
    fn canonical_decode_rejects_non_canonical_public_key() {
        let _guard = guard_default_label();
        let account = AccountId::new(default_domain_id(), ed25519_pk());
        let mut canonical = AccountAddress::from_account_id(&account)
            .expect("encode")
            .canonical_bytes()
            .expect("bytes");
        let offset = canonical.len() - ED25519_NON_CANONICAL_IDENTITY.len();
        canonical[offset..].copy_from_slice(&ED25519_NON_CANONICAL_IDENTITY);

        let err = AccountAddress::from_canonical_bytes(&canonical).unwrap_err();
        assert!(matches!(err, AccountAddressError::InvalidPublicKey));
    }

    #[test]
    fn parse_any_accepts_all_formats() {
        let _guard = guard_default_label();
        let original = default_domain_name();
        let _reset = Reset(original.clone());
        set_default_domain_name(DEFAULT_DOMAIN_NAME).expect("reset default domain label");
        let account = AccountId::new(domain(DEFAULT_DOMAIN_NAME), ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let canonical = address.canonical_hex().expect("canonical hex");
        let ih58 = address.to_ih58(42).expect("ih58");
        let compressed = address.to_compressed_sora().expect("compressed");

        assert!(canonical.starts_with("0x"));
        assert!(compressed.starts_with(COMPRESSED_SENTINEL));
        assert!(ih58.len() >= 10);

        assert_eq!(
            canonical,
            "0x020000012027c96646f2d4632d4fc241f84cbc427fbc3ecaa95becba55088d6c7b81fc5bbf"
        );
        assert_eq!(
            ih58,
            "364YxF5WtB9F5aThpqnyEsc9xijgc965zbfRvY4vL7VFo2Hzxb37Gat"
        );
        assert_eq!(
            compressed,
            "snx12QGﾈkﾀqｴeｺﾏQD31PZｾgｷﾁzﾗﾔｿ4ｺQbｳﾍｽkogﾙﾏmpGZEhﾊ3BMK8R"
        );

        let (decoded_ih58, fmt_ih58) =
            AccountAddress::parse_any(&ih58, Some(42)).expect("parse ih58");
        assert_eq!(fmt_ih58, AccountAddressFormat::IH58 { network_prefix: 42 });
        assert_eq!(
            decoded_ih58.canonical_bytes().unwrap(),
            address.canonical_bytes().unwrap()
        );

        let (decoded_compressed, fmt_compressed) =
            AccountAddress::parse_any(&compressed, None).expect("parse compressed");
        assert_eq!(fmt_compressed, AccountAddressFormat::Compressed);
        assert_eq!(
            decoded_compressed.canonical_bytes().unwrap(),
            address.canonical_bytes().unwrap()
        );

        let (decoded_hex, fmt_hex) =
            AccountAddress::parse_any(&canonical, None).expect("parse hex");
        assert_eq!(fmt_hex, AccountAddressFormat::CanonicalHex);
        assert_eq!(
            decoded_hex.canonical_bytes().unwrap(),
            address.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn parse_any_rejects_unknown_format() {
        let err = AccountAddress::parse_any("alice@wonderland", None)
            .expect_err("alias literal rejected");
        assert!(matches!(err, AccountAddressError::UnsupportedAddressFormat));
    }

    #[test]
    fn multisig_address_round_trip_preserves_policy() {
        let domain = domain("wonderland");
        let members = vec![
            MultisigMember::new(ed25519_pk_with(1), 1).expect("member"),
            MultisigMember::new(ed25519_pk_with(2), 2).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let account = AccountId::new_multisig(domain.clone(), policy.clone());
        let address = AccountAddress::from_account_id(&account).expect("encode");

        address
            .ensure_domain_matches(&domain)
            .expect("domain digest must match");
        let controller = address.to_account_controller().expect("controller");
        assert_eq!(controller.multisig_policy().expect("multisig"), &policy);

        let canonical = address.canonical_bytes().expect("bytes");
        assert_eq!((canonical[0] >> 3) & 0b11, 1, "multisig class tag");

        let canonical_hex = address.canonical_hex().expect("canonical hex");
        let parsed = AccountAddress::from_str(&canonical_hex).expect("parse hex");
        let decoded = parsed.to_account_controller().expect("controller");
        assert_eq!(decoded.multisig_policy().expect("multisig"), &policy);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn account_address_error_codes_are_stable() {
        macro_rules! assert_code {
            ($error:expr, $code_variant:ident, $code_str:literal) => {{
                let err: AccountAddressError = $error;
                assert_eq!(err.code(), AccountAddressErrorCode::$code_variant);
                assert_eq!(err.code_str(), $code_str);
            }};
        }

        assert_code!(
            AccountAddressError::UnsupportedAlgorithm(Algorithm::Secp256k1),
            UnsupportedAlgorithm,
            "ERR_UNSUPPORTED_ALGORITHM"
        );
        assert_code!(
            AccountAddressError::KeyPayloadTooLong(1024),
            KeyPayloadTooLong,
            "ERR_KEY_PAYLOAD_TOO_LONG"
        );
        assert_code!(
            AccountAddressError::InvalidHeaderVersion(5),
            InvalidHeaderVersion,
            "ERR_INVALID_HEADER_VERSION"
        );
        assert_code!(
            AccountAddressError::InvalidNormVersion(3),
            InvalidNormVersion,
            "ERR_INVALID_NORM_VERSION"
        );
        assert_code!(
            AccountAddressError::InvalidIh58Prefix(128),
            InvalidIh58Prefix,
            "ERR_INVALID_IH58_PREFIX"
        );
        assert_code!(
            AccountAddressError::HashError,
            HashFailure,
            "ERR_CANONICAL_HASH_FAILURE"
        );
        assert_code!(
            AccountAddressError::InvalidIh58Encoding,
            InvalidIh58Encoding,
            "ERR_INVALID_IH58_ENCODING"
        );
        assert_code!(
            AccountAddressError::InvalidLength,
            InvalidLength,
            "ERR_INVALID_LENGTH"
        );
        assert_code!(
            AccountAddressError::ChecksumMismatch,
            ChecksumMismatch,
            "ERR_CHECKSUM_MISMATCH"
        );
        assert_code!(
            AccountAddressError::InvalidHexAddress,
            InvalidHexAddress,
            "ERR_INVALID_HEX_ADDRESS"
        );
        assert_code!(
            AccountAddressError::DomainMismatch,
            DomainMismatch,
            "ERR_DOMAIN_MISMATCH"
        );
        assert_code!(
            AccountAddressError::InvalidDomainLabel("bad"),
            InvalidDomainLabel,
            "ERR_INVALID_DOMAIN_LABEL"
        );
        assert_code!(
            AccountAddressError::UnexpectedNetworkPrefix {
                expected: 0x1200,
                found: 0x3400
            },
            UnexpectedNetworkPrefix,
            "ERR_UNEXPECTED_NETWORK_PREFIX"
        );
        assert_code!(
            AccountAddressError::UnknownAddressClass(7),
            UnknownAddressClass,
            "ERR_UNKNOWN_ADDRESS_CLASS"
        );
        assert_code!(
            AccountAddressError::UnknownDomainTag(9),
            UnknownDomainTag,
            "ERR_UNKNOWN_DOMAIN_TAG"
        );
        assert_code!(
            AccountAddressError::UnexpectedExtensionFlag,
            UnexpectedExtensionFlag,
            "ERR_UNEXPECTED_EXTENSION_FLAG"
        );
        assert_code!(
            AccountAddressError::UnknownControllerTag(11),
            UnknownControllerTag,
            "ERR_UNKNOWN_CONTROLLER_TAG"
        );
        assert_code!(
            AccountAddressError::InvalidPublicKey,
            InvalidPublicKey,
            "ERR_INVALID_PUBLIC_KEY"
        );
        assert_code!(
            AccountAddressError::UnknownCurve(4),
            UnknownCurve,
            "ERR_UNKNOWN_CURVE"
        );
        assert_code!(
            AccountAddressError::UnexpectedTrailingBytes,
            UnexpectedTrailingBytes,
            "ERR_UNEXPECTED_TRAILING_BYTES"
        );
        assert_code!(
            AccountAddressError::InvalidIh58PrefixEncoding(0x80),
            InvalidIh58PrefixEncoding,
            "ERR_INVALID_IH58_PREFIX_ENCODING"
        );
        assert_code!(
            AccountAddressError::MissingCompressedSentinel,
            MissingCompressedSentinel,
            "ERR_MISSING_COMPRESSED_SENTINEL"
        );
        assert_code!(
            AccountAddressError::CompressedTooShort,
            CompressedTooShort,
            "ERR_COMPRESSED_TOO_SHORT"
        );
        assert_code!(
            AccountAddressError::InvalidCompressedChar('!'),
            InvalidCompressedChar,
            "ERR_INVALID_COMPRESSED_CHAR"
        );
        assert_code!(
            AccountAddressError::InvalidCompressedBase,
            InvalidCompressedBase,
            "ERR_INVALID_COMPRESSED_BASE"
        );
        assert_code!(
            AccountAddressError::InvalidCompressedDigit(123),
            InvalidCompressedDigit,
            "ERR_INVALID_COMPRESSED_DIGIT"
        );
        assert_code!(
            AccountAddressError::UnsupportedAddressFormat,
            UnsupportedAddressFormat,
            "ERR_UNSUPPORTED_ADDRESS_FORMAT"
        );
        assert_code!(
            AccountAddressError::MultisigMemberOverflow(32),
            MultisigMemberOverflow,
            "ERR_MULTISIG_MEMBER_OVERFLOW"
        );
        assert_code!(
            AccountAddressError::InvalidMultisigPolicy(MultisigPolicyError::EmptyMembers),
            InvalidMultisigPolicy,
            "ERR_INVALID_MULTISIG_POLICY"
        );
    }
}
