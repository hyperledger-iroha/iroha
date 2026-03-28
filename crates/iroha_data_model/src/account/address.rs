//! Address encoding utilities for accounts.
use core::{
    convert::{TryFrom, TryInto},
    fmt,
    str::FromStr,
};
use std::{
    cell::Cell,
    io::Write,
    sync::{
        Arc, Condvar, LazyLock, Mutex, RwLock,
        atomic::{AtomicU16, Ordering},
    },
    thread::ThreadId,
};

use blake2::{
    Blake2sMac,
    digest::{Mac, typenum::U32},
};
use hex;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_schema::{Ident, IntoSchema, MetaMap, Metadata, TypeId, VecMeta};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
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

/// Built-in implicit domain label used when configuration does not override it.
pub const DEFAULT_DOMAIN_NAME: &str = "default";

static DEFAULT_DOMAIN_LABEL: LazyLock<RwLock<Arc<str>>> =
    LazyLock::new(|| RwLock::new(Arc::<str>::from(DEFAULT_DOMAIN_NAME)));

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

/// Obtain the currently configured chain discriminant for i105 literal encoding,
/// honoring any thread-local override.
#[must_use]
pub fn chain_discriminant() -> u16 {
    CHAIN_DISCRIMINANT_OVERRIDE.with(|cell| {
        cell.get()
            .unwrap_or_else(|| CHAIN_DISCRIMINANT.load(Ordering::Relaxed))
    })
}

/// Set the global chain discriminant used by i105 addresses, returning the previous value.
pub fn set_chain_discriminant(discriminant: u16) -> u16 {
    CHAIN_DISCRIMINANT.swap(discriminant, Ordering::Relaxed)
}

const LOCAL_DOMAIN_KEY: &[u8] = b"SORA-LOCAL-K:v1";
const HEADER_VERSION_V1: u8 = 0;
const HEADER_NORM_VERSION_V1: u8 = 1;
const I105_SENTINEL_SORA: &str = "sora";
const I105_SENTINEL_TEST: &str = "test";
const I105_SENTINEL_DEV: &str = "dev";
const I105_SENTINEL_FALLBACK_PREFIX: &str = "n";
const I105_CHECKSUM_LEN: usize = 6;
const BECH32M_CONST: u32 = 0x2bc8_30a3;

const I105_BASE_U8: u8 = 105;
const I105_BASE: u32 = I105_BASE_U8 as u32;
const CHAIN_DISCRIMINANT_SORA: u16 = 0x02F1;
const CHAIN_DISCRIMINANT_TEST: u16 = 0x0171;
const CHAIN_DISCRIMINANT_DEV: u16 = 0x0000;
const DEFAULT_CHAIN_DISCRIMINANT: u16 = CHAIN_DISCRIMINANT_SORA;

static CHAIN_DISCRIMINANT: AtomicU16 = AtomicU16::new(DEFAULT_CHAIN_DISCRIMINANT);
thread_local! {
    static CHAIN_DISCRIMINANT_OVERRIDE: Cell<Option<u16>> = const { Cell::new(None) };
}

/// Scoped chain discriminant override for the current thread.
#[derive(Debug)]
pub struct ChainDiscriminantGuard(Option<u16>);

impl ChainDiscriminantGuard {
    /// Override the chain discriminant for the current thread.
    pub fn enter(discriminant: u16) -> Self {
        let previous = CHAIN_DISCRIMINANT_OVERRIDE.with(|cell| {
            let prev = cell.get();
            cell.set(Some(discriminant));
            prev
        });
        Self(previous)
    }
}

impl Drop for ChainDiscriminantGuard {
    fn drop(&mut self) {
        CHAIN_DISCRIMINANT_OVERRIDE.with(|cell| cell.set(self.0));
    }
}

/// Canonical representation of an account address payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAddress {
    header: AddressHeader,
    /// Domain selector metadata carried alongside the canonical controller payload.
    /// Canonical wire bytes always decode into [`DomainSelector::Default`].
    domain: DomainSelector,
    controller: ControllerPayload,
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

/// Stable selector key embedded into account addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum AccountDomainSelector {
    /// Selector referencing the configured default domain.
    Default,
    /// Selector carrying the 12-byte local digest derived from a domain label.
    LocalDigest12([u8; 12]),
    /// Selector pointing at a global registry entry.
    GlobalRegistry(u32),
}

impl AccountDomainSelector {
    /// Derive the selector key for a canonical domain identifier.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] when the domain label cannot be normalized.
    pub fn from_domain(domain: &DomainId) -> Result<Self, AccountAddressError> {
        DomainSelector::from_domain(domain).map(Self::from_selector)
    }

    /// Classify the selector into its coarse-grained kind.
    #[must_use]
    pub const fn kind(self) -> AddressDomainKind {
        match self {
            Self::Default => AddressDomainKind::Default,
            Self::LocalDigest12(_) => AddressDomainKind::LocalDigest12,
            Self::GlobalRegistry(_) => AddressDomainKind::GlobalRegistry,
        }
    }

    /// Borrow the local digest when present.
    #[must_use]
    pub const fn local12(self) -> Option<[u8; 12]> {
        match self {
            Self::LocalDigest12(bytes) => Some(bytes),
            _ => None,
        }
    }

    fn from_selector(selector: DomainSelector) -> Self {
        match selector {
            DomainSelector::Default => Self::Default,
            DomainSelector::Local12(bytes) => Self::LocalDigest12(bytes),
            DomainSelector::Global { registry_id } => Self::GlobalRegistry(registry_id),
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
        // Hard cut: payloads are globally scoped and no longer embed domain affinity.
        let domain = DomainSelector::Default;
        Ok(Self {
            header,
            domain,
            controller,
        })
    }

    /// Encode the payload as a canonical I105 literal using the active
    /// chain discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical payload construction or i105
    /// encoding fails.
    pub fn to_i105(&self) -> Result<String, AccountAddressError> {
        self.to_i105_for_discriminant(chain_discriminant())
    }

    /// Encode the payload as a canonical I105 literal with a specific
    /// chain discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if canonical payload construction or i105
    /// encoding fails.
    pub fn to_i105_for_discriminant(
        &self,
        discriminant: u16,
    ) -> Result<String, AccountAddressError> {
        let canonical = self.canonical_bytes()?;
        encode_i105_literal(discriminant, &canonical)
    }

    /// Classify the embedded domain selector.
    ///
    /// Canonical payloads do not encode domain selectors and always report
    /// [`AddressDomainKind::Default`].
    #[must_use]
    pub const fn domain_kind(&self) -> AddressDomainKind {
        match &self.domain {
            DomainSelector::Default => AddressDomainKind::Default,
            DomainSelector::Local12(_) => AddressDomainKind::LocalDigest12,
            DomainSelector::Global { .. } => AddressDomainKind::GlobalRegistry,
        }
    }

    /// Return the canonical selector key embedded in this address.
    ///
    /// New canonical payloads do not encode selector bytes and therefore always
    /// return [`AccountDomainSelector::Default`].
    #[must_use]
    pub fn domain_selector(&self) -> AccountDomainSelector {
        AccountDomainSelector::from_selector(self.domain)
    }

    /// Return the raw Local-12 selector digest when the address targets a non-default domain.
    ///
    /// Canonical payloads never include Local-12 data.
    #[must_use]
    pub fn local12_digest(&self) -> Option<[u8; 12]> {
        match &self.domain {
            DomainSelector::Local12(bytes) => Some(*bytes),
            _ => None,
        }
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
        let mut controller_cursor = 1usize;
        let controller = ControllerPayload::decode(bytes, &mut controller_cursor)?;
        if controller_cursor != bytes.len() {
            return Err(AccountAddressError::UnexpectedTrailingBytes);
        }
        Ok(Self {
            header,
            domain: DomainSelector::Default,
            controller,
        })
    }

    /// Decode the canonical I105 representation.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the string contains symbols outside
    /// the canonical I105 alphabet, carries a mismatching chain discriminant,
    /// or fails checksum validation.
    pub fn from_i105(encoded: &str) -> Result<Self, AccountAddressError> {
        let expected = chain_discriminant();
        let address = Self::from_i105_for_discriminant(encoded, Some(expected))?;
        address.ensure_canonical_i105_literal(encoded, expected)?;
        Ok(address)
    }

    /// Decode the i105 representation against an explicit expected chain
    /// discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the string carries a mismatching
    /// discriminant, has symbols outside the canonical I105 alphabet, or fails
    /// checksum validation.
    pub fn from_i105_for_discriminant(
        encoded: &str,
        expected_discriminant: Option<u16>,
    ) -> Result<Self, AccountAddressError> {
        let expected = expected_discriminant.unwrap_or_else(chain_discriminant);
        let (found, canonical) = decode_i105_literal(encoded)?;
        if found != expected {
            return Err(AccountAddressError::UnexpectedNetworkPrefix { expected, found });
        }
        Self::from_canonical_bytes(&canonical)
    }

    fn ensure_canonical_i105_literal(
        &self,
        encoded: &str,
        discriminant: u16,
    ) -> Result<(), AccountAddressError> {
        let canonical = self.to_i105_for_discriminant(discriminant)?;
        if canonical == encoded {
            Ok(())
        } else {
            Err(AccountAddressError::UnsupportedAddressFormat)
        }
    }

    /// Parse an address string in strict encoded i105 form.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError::UnsupportedAddressFormat`] for unsupported
    /// non-i105 literals (including canonical-hex parser input) and malformed
    /// i105 lexical forms.
    ///
    /// Preserves semantic decode failures such as checksum and discriminant
    /// mismatches.
    pub fn parse_encoded(
        input: &str,
        expected_discriminant: Option<u16>,
    ) -> Result<Self, AccountAddressError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(AccountAddressError::InvalidLength);
        }
        if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
            return Err(AccountAddressError::UnsupportedAddressFormat);
        }
        let expected = expected_discriminant.unwrap_or_else(chain_discriminant);
        let address = Self::from_i105_for_discriminant(trimmed, Some(expected))?;
        address.ensure_canonical_i105_literal(trimmed, expected)?;
        Ok(address)
    }

    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if encoding the controller payload fails.
    ///
    /// Canonical payloads are domain-agnostic and therefore do not include a
    /// serialized domain selector segment.
    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>, AccountAddressError> {
        let mut bytes = Vec::with_capacity(1 + CONTROLLER_MAX_LEN);
        bytes.push(self.header.encode());
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

    /// Convert this address into a domainless [`AccountId`].
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] when the controller payload cannot be decoded.
    pub fn to_account_id(&self) -> Result<AccountId, AccountAddressError> {
        let controller = self.to_account_controller()?;
        Ok(AccountId { controller })
    }

    /// Convert this address into a [`ScopedAccountId`] using an explicit domain.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] when the provided domain does not match the selector
    /// embedded in the address or when the controller payload cannot be decoded.
    pub fn to_scoped_account_id(
        &self,
        domain: &DomainId,
    ) -> Result<super::ScopedAccountId, AccountAddressError> {
        self.ensure_domain_matches(domain)?;
        Ok(self.to_account_id()?.to_account_id(domain.clone()))
    }

    /// Check that the provided domain matches the selector embedded in this address.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError::DomainMismatch`] when domains do not match.
    pub fn ensure_domain_matches(&self, domain: &DomainId) -> Result<(), AccountAddressError> {
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
        Self::parse_encoded(s, None)
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
        match AccountAddress::from_str(&literal) {
            Ok(address) => Ok(address),
            Err(AccountAddressError::UnsupportedAddressFormat) => {
                let canonical_hex = literal
                    .strip_prefix("0x")
                    .or_else(|| literal.strip_prefix("0X"))
                    .unwrap_or(&literal);
                let canonical_bytes = hex::decode(canonical_hex).map_err(|_| {
                    json::Error::Message(AccountAddressError::InvalidHexAddress.to_string())
                })?;
                AccountAddress::from_canonical_bytes(&canonical_bytes)
                    .map_err(|err| json::Error::Message(err.to_string()))
            }
            Err(err) => Err(json::Error::Message(err.to_string())),
        }
    }
}

const CONTROLLER_MAX_LEN: usize = 1024;
const CONTROLLER_SINGLE_KEY_TAG: u8 = 0x00;
const CONTROLLER_MULTISIG_TAG: u8 = 0x01;
const CONTROLLER_MULTISIG_MEMBER_MAX: usize = u16::MAX as usize;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    fn matches_domain(&self, domain: &DomainId) -> bool {
        let Ok(canonical) = Self::canonical_domain(domain) else {
            return false;
        };
        match self {
            // The `Default` selector intentionally does not encode a concrete domain label.
            // In multi-tenant deployments, callers may attach an explicit `@<domain>` suffix
            // externally to disambiguate. Treat `Default` as matching any provided domain and
            // let higher-level code (or on-chain existence checks) validate the final AccountId.
            Self::Local12(expected) => compute_local_digest(&canonical) == *expected,
            Self::Default | Self::Global { .. } => true,
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
                let member_count = u16::try_from(payload.members.len()).map_err(|_| {
                    AccountAddressError::MultisigMemberOverflow(payload.members.len())
                })?;
                out.extend_from_slice(&member_count.to_be_bytes());
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
                let member_count_bytes = bytes
                    .get(*cursor..*cursor + 2)
                    .ok_or(AccountAddressError::InvalidLength)?;
                *cursor += 2;
                let member_count = u16::from_be_bytes(member_count_bytes.try_into().unwrap());
                let member_count = usize::from(member_count);
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

fn i105_sentinel_for_discriminant(discriminant: u16) -> String {
    match discriminant {
        CHAIN_DISCRIMINANT_SORA => I105_SENTINEL_SORA.to_owned(),
        CHAIN_DISCRIMINANT_TEST => I105_SENTINEL_TEST.to_owned(),
        CHAIN_DISCRIMINANT_DEV => I105_SENTINEL_DEV.to_owned(),
        _ => format!("{I105_SENTINEL_FALLBACK_PREFIX}{discriminant}"),
    }
}

fn i105_discriminant_from_sentinel(input: &str) -> Option<u16> {
    for (discriminant, sentinel) in [
        (CHAIN_DISCRIMINANT_SORA, I105_SENTINEL_SORA),
        (CHAIN_DISCRIMINANT_TEST, I105_SENTINEL_TEST),
        (CHAIN_DISCRIMINANT_DEV, I105_SENTINEL_DEV),
    ] {
        if input.starts_with(sentinel) {
            return Some(discriminant);
        }
        if let Some(fullwidth) = ascii_str_to_fullwidth(sentinel)
            && input.starts_with(&fullwidth)
        {
            return Some(discriminant);
        }
    }
    i105_discriminant_from_numeric_sentinel(input)
}

fn i105_discriminant_from_numeric_sentinel(input: &str) -> Option<u16> {
    if let Some(rest) = input.strip_prefix(I105_SENTINEL_FALLBACK_PREFIX) {
        return parse_i105_numeric_sentinel(rest, false);
    }
    if let Some(fullwidth_n) = ascii_char_to_fullwidth('n')
        && let Some(rest) = input.strip_prefix(fullwidth_n)
    {
        return parse_i105_numeric_sentinel(rest, true);
    }
    None
}

fn parse_i105_numeric_sentinel(rest: &str, fullwidth: bool) -> Option<u16> {
    let mut digits = String::new();
    for ch in rest.chars().take(5) {
        let ascii_digit = if fullwidth {
            match fullwidth_digit_to_ascii(ch) {
                Some(digit) => digit,
                None => break,
            }
        } else if ch.is_ascii_digit() {
            ch
        } else {
            break;
        };
        digits.push(ascii_digit);
    }
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok()
}

fn ascii_char_to_fullwidth(ch: char) -> Option<char> {
    match ch {
        '0'..='9' => char::from_u32(ch as u32 - '0' as u32 + 0xFF10),
        'A'..='Z' => char::from_u32(ch as u32 - 'A' as u32 + 0xFF21),
        'a'..='z' => char::from_u32(ch as u32 - 'a' as u32 + 0xFF41),
        _ => None,
    }
}

fn ascii_str_to_fullwidth(input: &str) -> Option<String> {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        output.push(ascii_char_to_fullwidth(ch)?);
    }
    Some(output)
}

fn fullwidth_digit_to_ascii(ch: char) -> Option<char> {
    match ch {
        '０'..='９' => char::from_u32(ch as u32 - '０' as u32 + '0' as u32),
        _ => None,
    }
}

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
    /// Payload length invalid for the requested operation.
    InvalidLength,
    /// i105 checksum validation failed.
    ChecksumMismatch,
    /// Canonical hexadecimal payload failed to decode.
    InvalidHexAddress,
    /// Domain selector did not match expectation.
    DomainMismatch,
    /// Domain label failed normalisation.
    InvalidDomainLabel,
    /// Chain discriminant prefix did not match expectation.
    UnexpectedNetworkPrefix,
    /// Unknown address class encountered.
    UnknownAddressClass,
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
    /// i105 form missing the expected chain-discriminant sentinel.
    MissingI105Sentinel,
    /// i105 form shorter than minimal payload.
    I105TooShort,
    /// Invalid character in i105 alphabet.
    InvalidI105Char,
    /// Invalid i105 alphabet base requested.
    InvalidI105Base,
    /// Digit outside i105 alphabet bounds.
    InvalidI105Digit,
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
            Self::InvalidLength => "ERR_INVALID_LENGTH",
            Self::ChecksumMismatch => "ERR_CHECKSUM_MISMATCH",
            Self::InvalidHexAddress => "ERR_INVALID_HEX_ADDRESS",
            Self::DomainMismatch => "ERR_DOMAIN_MISMATCH",
            Self::InvalidDomainLabel => "ERR_INVALID_DOMAIN_LABEL",
            Self::UnexpectedNetworkPrefix => "ERR_UNEXPECTED_NETWORK_PREFIX",
            Self::UnknownAddressClass => "ERR_UNKNOWN_ADDRESS_CLASS",
            Self::UnexpectedExtensionFlag => "ERR_UNEXPECTED_EXTENSION_FLAG",
            Self::UnknownControllerTag => "ERR_UNKNOWN_CONTROLLER_TAG",
            Self::InvalidPublicKey => "ERR_INVALID_PUBLIC_KEY",
            Self::UnknownCurve => "ERR_UNKNOWN_CURVE",
            Self::UnexpectedTrailingBytes => "ERR_UNEXPECTED_TRAILING_BYTES",
            Self::MissingI105Sentinel => "ERR_MISSING_I105_SENTINEL",
            Self::I105TooShort => "ERR_I105_TOO_SHORT",
            Self::InvalidI105Char => "ERR_INVALID_I105_CHAR",
            Self::InvalidI105Base => "ERR_INVALID_I105_BASE",
            Self::InvalidI105Digit => "ERR_INVALID_I105_DIGIT",
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
    /// Data length is invalid for the requested operation.
    #[error("invalid length for address payload")]
    InvalidLength,
    /// i105 checksum validation failed.
    #[error("i105 checksum mismatch")]
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
    /// Chain discriminant prefix did not match expectations.
    #[error("unexpected i105 chain discriminant: expected {expected}, found {found}")]
    UnexpectedNetworkPrefix {
        /// Chain discriminant we expected to decode.
        expected: u16,
        /// Chain discriminant decoded from the prefix bytes.
        found: u16,
    },
    /// Encountered an unknown address class.
    #[error("unknown address class: {0}")]
    UnknownAddressClass(u8),
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
    /// i105 form is missing the expected chain-discriminant sentinel.
    #[error("i105 address is missing the expected chain-discriminant sentinel")]
    MissingI105Sentinel,
    /// i105 form is too short to contain payload and checksum.
    #[error("i105 address too short")]
    I105TooShort,
    /// Encountered a character outside of the i105 alphabet.
    #[error("invalid character `{0}` in i105 address")]
    InvalidI105Char(char),
    /// The i105 alphabet base is invalid or unsupported.
    #[error("invalid i105 alphabet base")]
    InvalidI105Base,
    /// Encountered a digit value outside of the i105 alphabet size.
    #[error("invalid i105 digit value: {0}")]
    InvalidI105Digit(u8),
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
            Self::InvalidLength => AccountAddressErrorCode::InvalidLength,
            Self::ChecksumMismatch => AccountAddressErrorCode::ChecksumMismatch,
            Self::InvalidHexAddress => AccountAddressErrorCode::InvalidHexAddress,
            Self::DomainMismatch => AccountAddressErrorCode::DomainMismatch,
            Self::InvalidDomainLabel(_) => AccountAddressErrorCode::InvalidDomainLabel,
            Self::UnexpectedNetworkPrefix { .. } => {
                AccountAddressErrorCode::UnexpectedNetworkPrefix
            }
            Self::UnknownAddressClass(_) => AccountAddressErrorCode::UnknownAddressClass,
            Self::UnexpectedExtensionFlag => AccountAddressErrorCode::UnexpectedExtensionFlag,
            Self::UnknownControllerTag(_) => AccountAddressErrorCode::UnknownControllerTag,
            Self::InvalidPublicKey => AccountAddressErrorCode::InvalidPublicKey,
            Self::UnknownCurve(_) => AccountAddressErrorCode::UnknownCurve,
            Self::UnexpectedTrailingBytes => AccountAddressErrorCode::UnexpectedTrailingBytes,
            Self::MissingI105Sentinel => AccountAddressErrorCode::MissingI105Sentinel,
            Self::I105TooShort => AccountAddressErrorCode::I105TooShort,
            Self::InvalidI105Char(_) => AccountAddressErrorCode::InvalidI105Char,
            Self::InvalidI105Base => AccountAddressErrorCode::InvalidI105Base,
            Self::InvalidI105Digit(_) => AccountAddressErrorCode::InvalidI105Digit,
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

fn encode_i105_literal(prefix: u16, canonical: &[u8]) -> Result<String, AccountAddressError> {
    let digits = encode_base_n(canonical, I105_BASE)?;
    let checksum = i105_checksum_digits(canonical);
    let mut output = i105_sentinel_for_discriminant(prefix);
    output.reserve(output.len() + digits.len() + checksum.len());
    for digit in digits {
        output.push_str(i105_digit_symbol(digit, false)?);
    }
    for digit in checksum {
        output.push_str(i105_digit_symbol(digit, false)?);
    }
    Ok(output)
}

fn decode_i105_literal(input: &str) -> Result<(u16, Vec<u8>), AccountAddressError> {
    let Some(discriminant) = i105_discriminant_from_sentinel(input) else {
        return Err(AccountAddressError::MissingI105Sentinel);
    };
    let payload = input
        .strip_prefix(&i105_sentinel_for_discriminant(discriminant))
        .or_else(|| {
            ascii_str_to_fullwidth(&i105_sentinel_for_discriminant(discriminant))
                .and_then(|fullwidth| input.strip_prefix(&fullwidth))
        })
        .ok_or(AccountAddressError::MissingI105Sentinel)?;
    let canonical = decode_i105_payload(payload)?;
    Ok((discriminant, canonical))
}

fn encode_base_n(bytes: &[u8], base: u32) -> Result<Vec<u8>, AccountAddressError> {
    if base < 2 {
        return Err(AccountAddressError::InvalidI105Base);
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
        return Err(AccountAddressError::InvalidI105Base);
    }
    if digits.is_empty() {
        return Err(AccountAddressError::InvalidLength);
    }
    let leading_zeros = digits.iter().take_while(|&&d| d == 0).count();
    let mut value = digits.to_vec();
    let mut bytes = Vec::new();
    let mut start = leading_zeros;
    while start < value.len() {
        let mut remainder = 0u32;
        for digit in &mut value[start..] {
            if u32::from(*digit) >= base {
                return Err(AccountAddressError::InvalidI105Digit(*digit));
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

fn i105_checksum_digits(canonical: &[u8]) -> [u8; I105_CHECKSUM_LEN] {
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

fn bech32m_checksum(data: &[u8]) -> [u8; I105_CHECKSUM_LEN] {
    let mut values = expand_hrp("snx");
    values.extend_from_slice(data);
    values.extend([0u8; I105_CHECKSUM_LEN]);
    let polymod = bech32_polymod(values.iter().copied()) ^ BECH32M_CONST;
    let mut result = [0u8; I105_CHECKSUM_LEN];
    for (index, slot) in result.iter_mut().enumerate() {
        let shift = 5 * (I105_CHECKSUM_LEN - 1 - index);
        let value = (polymod >> shift) & 0x1f;
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
        for (index, generator) in GEN.iter().enumerate() {
            if (top >> index) & 1 == 1 {
                chk ^= generator;
            }
        }
    }
    chk
}

fn expand_hrp(hrp: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(hrp.len() * 2 + 1);
    for byte in hrp.bytes() {
        out.push(byte >> 5);
    }
    out.push(0);
    out.extend(hrp.bytes().map(|byte| byte & 0x1f));
    out
}

fn decode_i105_payload(payload: &str) -> Result<Vec<u8>, AccountAddressError> {
    let digits = i105_payload_digits(payload)?;
    if digits.len() <= I105_CHECKSUM_LEN {
        return Err(AccountAddressError::I105TooShort);
    }
    let split_at = digits.len() - I105_CHECKSUM_LEN;
    let canonical = decode_base_n(&digits[..split_at], I105_BASE)?;
    let expected = i105_checksum_digits(&canonical);
    if digits[split_at..] != expected {
        return Err(AccountAddressError::ChecksumMismatch);
    }
    Ok(canonical)
}

fn i105_payload_digits(payload: &str) -> Result<Vec<u8>, AccountAddressError> {
    let mut digits = Vec::with_capacity(payload.chars().count());
    for ch in payload.chars() {
        let mut symbol = [0_u8; 4];
        let encoded = ch.encode_utf8(&mut symbol);
        let Some(digit) = lookup_i105_digit(encoded) else {
            return Err(AccountAddressError::InvalidI105Char(ch));
        };
        digits.push(digit);
    }
    Ok(digits)
}

fn lookup_i105_digit(symbol: &str) -> Option<u8> {
    I105_DIGIT_TABLE
        .iter()
        .find_map(|(candidate, value)| (*candidate == symbol).then_some(*value))
}

fn i105_digit_symbol(digit: u8, _fullwidth: bool) -> Result<&'static str, AccountAddressError> {
    I105_ALPHABET
        .get(usize::from(digit))
        .copied()
        .ok_or(AccountAddressError::InvalidI105Digit(digit))
}

const BASE58_ALPHABET: [&str; 58] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H", "J", "K",
    "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b", "c", "d", "e",
    "f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y",
    "z",
];

const IROHA_POEM_KANA_FULLWIDTH: [&str; 47] = [
    "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ", "ヨ", "タ",
    "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク", "ヤ", "マ", "ケ", "フ",
    "コ", "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ", "ヱ", "ヒ", "モ", "セ", "ス",
];

const IROHA_POEM_KANA_HALFWIDTH: [&str; 47] = [
    "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ", "ﾂ",
    "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ",
    "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
];

static I105_ALPHABET: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut table = Vec::with_capacity(BASE58_ALPHABET.len() + IROHA_POEM_KANA_FULLWIDTH.len());
    table.extend_from_slice(&BASE58_ALPHABET);
    table.extend_from_slice(&IROHA_POEM_KANA_FULLWIDTH);
    table
});

static I105_DIGIT_TABLE: LazyLock<Vec<(&'static str, u8)>> = LazyLock::new(|| {
    let mut table = Vec::with_capacity(usize::from(I105_BASE_U8) * 2);
    for (index, symbol) in I105_ALPHABET.iter().enumerate() {
        table.push((
            *symbol,
            u8::try_from(index).expect("I105 alphabet length fits within u8"),
        ));
    }
    for (index, symbol) in IROHA_POEM_KANA_HALFWIDTH.iter().enumerate() {
        table.push((
            *symbol,
            u8::try_from(BASE58_ALPHABET.len() + index)
                .expect("I105 alphabet length fits within u8"),
        ));
    }
    table
});

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
        let account = AccountId::new(ed25519_pk_with(seed));
        AccountAddress::from_account_id(&account).expect("account id encodes into an address")
    }

    fn fullwidth_sentinel_literal(canonical: &str) -> String {
        for sentinel in [I105_SENTINEL_SORA, I105_SENTINEL_TEST, I105_SENTINEL_DEV] {
            if let Some(rest) = canonical.strip_prefix(sentinel) {
                return format!(
                    "{}{}",
                    ascii_str_to_fullwidth(sentinel).expect("ASCII sentinel must widen"),
                    rest
                );
            }
        }
        if let Some(rest) = canonical.strip_prefix(I105_SENTINEL_FALLBACK_PREFIX) {
            return format!(
                "{}{}",
                ascii_str_to_fullwidth(I105_SENTINEL_FALLBACK_PREFIX)
                    .expect("ASCII sentinel must widen"),
                rest
            );
        }
        canonical.to_owned()
    }

    #[cfg(feature = "json")]
    #[test]
    fn account_address_json_roundtrip_supports_canonical_hex_literals() {
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("account address");
        let json_literal = norito::json::to_json(&address).expect("serialize account address");
        let decoded: AccountAddress =
            norito::json::from_str(&json_literal).expect("deserialize account address");
        assert_eq!(decoded, address);
    }

    #[test]
    fn chain_discriminant_guard_scopes_override() {
        let _outer = ChainDiscriminantGuard::enter(42);
        let original = chain_discriminant();
        {
            let _guard = ChainDiscriminantGuard::enter(original.wrapping_add(1));
            assert_eq!(chain_discriminant(), original.wrapping_add(1));
        }
        assert_eq!(chain_discriminant(), original);
    }

    #[test]
    fn dotted_local8_payloads_are_rejected() {
        let mut canonical =
            hex::decode("0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c")
                .expect("legacy local-12 fixture");
        let digest_start = 2; // header (0) + tag (1) + digest payload
        canonical.drain(digest_start + 8..digest_start + 12);

        let err =
            AccountAddress::from_canonical_bytes(&canonical).expect_err("legacy payload rejected");
        let literal = format!("0x{}", hex::encode(&canonical));
        let parse_err = AccountId::parse_encoded(&literal).expect_err("account parsing fails");
        assert_eq!(
            parse_err.reason(),
            "AccountId must use a canonical I105 literal"
        );
        assert_eq!(
            err.code_str(),
            AccountAddressErrorCode::UnknownCurve.as_str()
        );
    }

    #[test]
    fn dotted_local8_payloads_without_controller_tag_are_rejected() {
        let mut canonical =
            hex::decode("0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c")
                .expect("legacy local-12 fixture");
        canonical.drain(10..15);

        let err =
            AccountAddress::from_canonical_bytes(&canonical).expect_err("legacy payload rejected");
        let literal = format!("0x{}", hex::encode(&canonical));
        let parse_err = AccountId::parse_encoded(&literal).expect_err("account parsing fails");
        assert_eq!(
            parse_err.reason(),
            "AccountId must use a canonical I105 literal"
        );
        assert_eq!(
            err.code_str(),
            AccountAddressErrorCode::UnknownCurve.as_str()
        );
    }

    #[test]
    fn selector_prefixed_payloads_are_rejected() {
        let canonical = hex::decode(
            "0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
        )
        .expect("legacy local-12 fixture");
        AccountAddress::from_canonical_bytes(&canonical)
            .expect_err("selector-prefixed legacy payload must be rejected");
    }

    #[test]
    fn local12_digest_absent_for_default_domain() {
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        assert!(
            address.local12_digest().is_none(),
            "default domain selectors should not report Local-12 digests"
        );
    }

    #[test]
    fn iroha_poem_kana_matches_expected_order() {
        const EXPECTED: [&str; 47] = [
            "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ",
            "ヨ", "タ", "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク",
            "ヤ", "マ", "ケ", "フ", "コ", "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ",
            "ヱ", "ヒ", "モ", "セ", "ス",
        ];
        assert_eq!(IROHA_POEM_KANA_FULLWIDTH, EXPECTED);
    }

    #[test]
    fn i105_alphabet_has_unique_canonical_symbols() {
        let mut symbols = BTreeSet::new();
        for symbol in BASE58_ALPHABET {
            assert!(symbols.insert(symbol), "duplicate Base58 symbol {symbol}");
        }
        for symbol in IROHA_POEM_KANA_FULLWIDTH {
            assert!(
                symbols.insert(symbol),
                "duplicate Iroha poem symbol {symbol}"
            );
        }
        assert_eq!(
            symbols.len(),
            I105_ALPHABET.len(),
            "unexpected I105 alphabet length"
        );
    }

    #[test]
    fn i105_golden_vectors_roundtrip() {
        let _guard_default = guard_default_label();
        let original = default_domain_name();
        let _reset = Reset(original.clone());
        set_default_domain_name("default").expect("restore default label");

        let vectors = [
            ("default", 0_u8),
            ("treasury", 1_u8),
            ("wonderland", 2_u8),
            ("iroha", 3_u8),
            ("alpha", 4_u8),
            ("omega", 5_u8),
            ("governance", 6_u8),
            ("validators", 7_u8),
            ("explorer", 8_u8),
            ("soranet", 9_u8),
            ("kitsune", 10_u8),
            ("da", 11_u8),
        ];

        for (label, seed_byte) in vectors {
            let account = AccountId::new(ed25519_pk_with(seed_byte));
            let address = AccountAddress::from_account_id(&account).expect("address encoding");
            let canonical = address
                .canonical_hex()
                .expect("canonical encoding must succeed");
            let literal = address
                .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
                .expect("i105 encoding must succeed");
            assert!(
                canonical.starts_with("0x020001"),
                "canonical payloads must not include a domain selector byte: label={label} seed={seed_byte} canonical={canonical}"
            );
            let decoded =
                AccountAddress::from_i105_for_discriminant(&literal, Some(CHAIN_DISCRIMINANT_SORA))
                    .expect("i105 decode");
            assert_eq!(
                decoded.canonical_hex().expect("canonical"),
                canonical,
                "label={label} seed={seed_byte}"
            );
        }
    }

    #[test]
    fn canonical_payload_omits_domain_selector_bytes() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let canonical = address.canonical_bytes().expect("bytes");
        assert_eq!(canonical[0] >> 5, HEADER_VERSION_V1);
        assert_eq!(canonical[1], CONTROLLER_SINGLE_KEY_TAG);
        let mut legacy = canonical.clone();
        legacy.insert(1, 0x00);
        AccountAddress::from_canonical_bytes(&legacy)
            .expect_err("legacy selector-prefixed payloads are rejected");
    }

    #[test]
    fn non_default_domain_address_bytes_match_default_domain_bytes() {
        let _guard = guard_default_label();
        let key = ed25519_pk();
        let default_account = AccountId::new(key.clone());
        let local_account = AccountId::new(key);
        let default_address = AccountAddress::from_account_id(&default_account).expect("encode");
        let local_address = AccountAddress::from_account_id(&local_account).expect("encode");
        assert_eq!(
            default_address.canonical_bytes().expect("default bytes"),
            local_address.canonical_bytes().expect("local bytes"),
            "domain must not influence canonical address bytes"
        );
        assert!(local_address.local12_digest().is_none());
    }

    #[test]
    fn domain_kind_distinguishes_default_and_local_selectors() {
        let _guard = guard_default_label();
        let default_account = AccountId::new(ed25519_pk_with(7));
        let default_address =
            AccountAddress::from_account_id(&default_account).expect("encode default domain");
        assert_eq!(default_address.domain_kind(), AddressDomainKind::Default);

        let local_account = AccountId::new(ed25519_pk_with(9));
        let local_address =
            AccountAddress::from_account_id(&local_account).expect("encode local domain");
        assert_eq!(local_address.domain_kind(), AddressDomainKind::Default);
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
        let _canonical = set_default_domain_name("ledger").expect("set default label");
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let canonical_bytes = address.canonical_bytes().expect("bytes");
        assert_eq!(canonical_bytes[1], CONTROLLER_SINGLE_KEY_TAG);
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
    fn i105_encoding_respects_chain_discriminant() {
        let _guard = guard_default_label();
        let _chain = ChainDiscriminantGuard::enter(42);
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let encoded = address.to_i105().expect("i105");
        assert_eq!(
            encoded,
            address
                .to_i105_for_discriminant(42)
                .expect("explicit discriminant")
        );
        assert_ne!(
            encoded,
            address
                .to_i105_for_discriminant(DEFAULT_CHAIN_DISCRIMINANT)
                .expect("default discriminant"),
            "changing the chain discriminant must change the encoded prefix bytes"
        );
    }

    #[test]
    fn i105_known_discriminants_roundtrip() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");

        let sora = address
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
            .expect("sora");
        let taira = address
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_TEST)
            .expect("test");
        let dev = address
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_DEV)
            .expect("dev");

        assert_ne!(sora, taira);
        assert_ne!(sora, dev);
        assert_ne!(taira, dev);
        assert!(!sora.contains(':'));
        assert!(!taira.contains(':'));
        assert!(!dev.contains(':'));

        for (literal, discriminant) in [
            (sora, CHAIN_DISCRIMINANT_SORA),
            (taira, CHAIN_DISCRIMINANT_TEST),
            (dev, CHAIN_DISCRIMINANT_DEV),
        ] {
            let decoded = AccountAddress::from_i105_for_discriminant(&literal, Some(discriminant))
                .expect("decode succeeds with discriminant");
            assert_eq!(
                decoded.canonical_bytes().unwrap(),
                address.canonical_bytes().unwrap()
            );
        }
    }

    #[test]
    fn parse_encoded_rejects_fullwidth_sentinel_literals() {
        let account = AccountId::new(ed25519_pk());
        let canonical = AccountAddress::from_account_id(&account)
            .expect("encode")
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
            .expect("canonical I105");
        let noncanonical = fullwidth_sentinel_literal(&canonical);

        let err = AccountAddress::parse_encoded(&noncanonical, Some(CHAIN_DISCRIMINANT_SORA))
            .expect_err("fullwidth sentinel must be rejected");
        assert_eq!(
            err.code_str(),
            AccountAddressErrorCode::UnsupportedAddressFormat.as_str()
        );
    }

    #[test]
    fn i105_canonical_payload_uses_base58_and_iroha_kana() {
        let account = AccountId::new(ed25519_pk());
        let literal = AccountAddress::from_account_id(&account)
            .expect("encode")
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
            .expect("i105");
        let payload = literal
            .strip_prefix(I105_SENTINEL_SORA)
            .expect("sora sentinel must prefix canonical literal");

        assert!(
            !payload.is_empty(),
            "canonical literal must contain payload symbols"
        );
        assert!(
            payload.chars().any(|ch| ch.is_ascii_alphanumeric()),
            "canonical I105 payload must expose Base58 symbols: {literal}"
        );
        assert!(
            payload.chars().any(|ch| {
                let mut symbol = [0_u8; 4];
                let encoded = ch.encode_utf8(&mut symbol);
                IROHA_POEM_KANA_FULLWIDTH
                    .iter()
                    .any(|candidate| *candidate == encoded)
            }),
            "canonical I105 payload must expose Iroha-poem katakana: {literal}"
        );
    }

    #[test]
    fn account_address_encodes_secp256k1_controller() {
        let (public_key, _) = KeyPair::random_with_algorithm(Algorithm::Secp256k1).into_parts();
        let account = AccountId::new(public_key.clone());
        let address = AccountAddress::from_account_id(&account).expect("encode secp256k1");
        let controller = address
            .to_account_controller()
            .expect("decode secp256k1 controller");
        assert_eq!(controller.single_signatory(), Some(&public_key));
    }

    #[test]
    fn account_address_to_account_id_roundtrip() {
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode account id");
        let roundtrip = address.to_account_id().expect("decode account id");
        assert_eq!(roundtrip, account);

        let other_domain = domain("garden");
        let projected = address
            .to_scoped_account_id(&other_domain)
            .expect("global selector should project to arbitrary domain");
        assert_eq!(projected.domain(), &other_domain);
        assert_eq!(projected.signatory(), account.signatory());
    }

    #[test]
    fn i105_round_trip_recovers_canonical_payload() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
        let original = AccountAddress::from_account_id(&account).expect("encode");
        let encoded = original.to_i105_for_discriminant(73).expect("i105");
        let decoded = AccountAddress::from_i105_for_discriminant(&encoded, Some(73))
            .expect("decode succeeds with discriminant");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn i105_discriminant_mismatch_fails() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let encoded = address.to_i105_for_discriminant(10).expect("i105");
        let err = AccountAddress::from_i105_for_discriminant(&encoded, Some(11))
            .expect_err("discriminant mismatch");
        assert!(matches!(
            err,
            AccountAddressError::UnexpectedNetworkPrefix {
                expected: 11,
                found: 10
            }
        ));
    }

    #[test]
    fn i105_round_trip() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
        let original = AccountAddress::from_account_id(&account).expect("encode");
        let literal = original.to_i105().expect("i105 encode");
        let decoded = AccountAddress::from_i105(&literal).expect("i105 decode");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn i105_round_trip_accepts_halfwidth_iroha_kana_inputs() {
        let account = AccountId::new(ed25519_pk());
        let original = AccountAddress::from_account_id(&account).expect("encode");
        let mut halfwidth = original
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
            .expect("i105 encode");
        for (fullwidth, halfwidth_kana) in IROHA_POEM_KANA_FULLWIDTH
            .iter()
            .zip(IROHA_POEM_KANA_HALFWIDTH.iter())
        {
            halfwidth = halfwidth.replace(fullwidth, halfwidth_kana);
        }
        let decoded =
            AccountAddress::from_i105_for_discriminant(&halfwidth, Some(CHAIN_DISCRIMINANT_SORA))
                .expect("i105 decode");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            original.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn i105_invalid_char_rejected() {
        let account = AccountId::new(ed25519_pk());
        let mut chars = AccountAddress::from_account_id(&account)
            .expect("encode")
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
            .expect("i105 encode")
            .chars()
            .collect::<Vec<_>>();
        let last = chars.len() - 1;
        chars[last] = '!';
        let literal = chars.into_iter().collect::<String>();
        let err =
            AccountAddress::from_i105_for_discriminant(&literal, Some(CHAIN_DISCRIMINANT_SORA))
                .expect_err("invalid char should fail");
        assert!(matches!(err, AccountAddressError::InvalidI105Char('!')));
    }

    #[test]
    fn i105_checksum_mismatch_detected() {
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let mut tampered = address
            .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
            .expect("i105 encode")
            .chars()
            .collect::<Vec<_>>();
        let last = tampered.len() - 1;
        tampered[last] = if tampered[last] == '1' { '2' } else { '1' };
        let tampered = tampered.into_iter().collect::<String>();
        let err =
            AccountAddress::from_i105_for_discriminant(&tampered, Some(CHAIN_DISCRIMINANT_SORA))
                .expect_err("checksum mismatch");
        assert!(matches!(err, AccountAddressError::ChecksumMismatch));
    }

    proptest! {
        #[test]
        fn i105_checksum_corruption_detected(seed in any::<u8>(), offset in any::<u8>()) {
            let _guard = guard_default_label();
            let address = account_address_for_seed(seed);
            let literal = address
                .to_i105_for_discriminant(CHAIN_DISCRIMINANT_SORA)
                .expect("i105 encode");
            let sentinel_len = i105_sentinel_for_discriminant(CHAIN_DISCRIMINANT_SORA)
                .chars()
                .count();
            let mut chars = literal.chars().collect::<Vec<_>>();
            let payload_len = chars.len() - sentinel_len;
            let index = sentinel_len + (usize::from(offset) % payload_len);
            chars[index] = match chars[index] {
                '1' => '2',
                _ => '1',
            };
            let tampered = chars.into_iter().collect::<String>();
            let err = AccountAddress::from_i105_for_discriminant(
                &tampered,
                Some(CHAIN_DISCRIMINANT_SORA),
            )
                .expect_err("checksum mismatch");
            prop_assert!(matches!(err, AccountAddressError::ChecksumMismatch));
        }
    }

    #[test]
    fn canonical_decode_rejects_small_order_public_key() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
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
        let account = AccountId::new(ed25519_pk());
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
    fn parse_encoded_accepts_i105_format() {
        let _guard = guard_default_label();
        let original = default_domain_name();
        let _reset = Reset(original.clone());
        set_default_domain_name(DEFAULT_DOMAIN_NAME).expect("reset default domain label");
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let literal = address
            .to_i105_for_discriminant(42)
            .expect("i105 with explicit discriminant");
        let decoded = AccountAddress::parse_encoded(&literal, Some(42)).expect("parse i105");
        assert_eq!(
            decoded.canonical_bytes().unwrap(),
            address.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn parse_encoded_rejects_unknown_format() {
        let err = AccountAddress::parse_encoded("alice@hbl.dataspace", None)
            .expect_err("alias literal rejected");
        assert!(matches!(err, AccountAddressError::UnsupportedAddressFormat));
    }

    #[test]
    fn parse_encoded_accepts_only_i105() {
        let _guard = guard_default_label();
        let account = AccountId::new(ed25519_pk());
        let address = AccountAddress::from_account_id(&account).expect("encode");
        let i105 = address.to_i105_for_discriminant(42).expect("i105");
        let canonical = address.canonical_hex().expect("canonical");

        AccountAddress::parse_encoded(&i105, Some(42)).expect("parse i105");

        let err = AccountAddress::parse_encoded(&canonical, None)
            .expect_err("canonical must be rejected");
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
        let account = AccountId::new_multisig(policy.clone());
        let address = AccountAddress::from_account_id(&account).expect("encode");

        address
            .ensure_domain_matches(&domain)
            .expect("domain digest must match");
        let controller = address.to_account_controller().expect("controller");
        assert_eq!(controller.multisig_policy().expect("multisig"), &policy);

        let canonical = address.canonical_bytes().expect("bytes");
        assert_eq!((canonical[0] >> 3) & 0b11, 1, "multisig class tag");

        let i105 = address.to_i105_for_discriminant(42).expect("i105");
        let parsed = AccountAddress::from_str(&i105).expect("parse i105");
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
            AccountAddressError::I105TooShort,
            I105TooShort,
            "ERR_I105_TOO_SHORT"
        );
        assert_code!(
            AccountAddressError::InvalidI105Char('!'),
            InvalidI105Char,
            "ERR_INVALID_I105_CHAR"
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
