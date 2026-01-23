//! Structures, traits and impls related to `Account`s.
use core::fmt;
use std::{
    format,
    io::Write,
    str::FromStr,
    string::String,
    sync::{Arc, RwLock},
    vec::Vec,
};

pub use admission::{
    ACCOUNT_ADMISSION_POLICY_METADATA_KEY, AccountAdmissionMode, AccountAdmissionPolicy,
    DEFAULT_MAX_IMPLICIT_ACCOUNT_CREATIONS_PER_TX,
};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model_derive::{IdEqOrdHash, RegistrableBuilder, model};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::{
    model::*,
    rekey::{AccountLabel, AccountRekeyRecord},
};
pub mod address;
pub mod admission;
pub mod controller;
pub mod curve;
pub mod rekey;
pub use address::{
    AccountAddress, AccountAddressError, AccountAddressErrorCode, AccountAddressFormat,
    AccountDomainSelector,
};
pub use controller::{AccountController, MultisigMember, MultisigPolicy, MultisigPolicyError};

use crate::{
    HasMetadata, Identifiable, IntoKeyValue, Registered, Registrable,
    common::{Owned, Ref, split_nonempty},
    domain::prelude::*,
    error::ParseError,
    metadata::Metadata,
    name::Name,
    nexus::UniversalAccountId,
};

/// Function type used to resolve account aliases into canonical [`AccountId`] values.
pub type AccountAliasResolver =
    dyn Fn(&str, &DomainId) -> Option<AccountId> + Send + Sync + 'static;

static ACCOUNT_ALIAS_RESOLVER: std::sync::LazyLock<RwLock<Option<Arc<AccountAliasResolver>>>> =
    std::sync::LazyLock::new(|| RwLock::new(None));

/// Install a global account-alias resolver consulted by [`AccountId::from_str`].
pub fn set_account_alias_resolver(resolver: Arc<AccountAliasResolver>) {
    let mut guard = ACCOUNT_ALIAS_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(resolver);
}

/// Clear the globally installed account-alias resolver.
pub fn clear_account_alias_resolver() {
    let mut guard = ACCOUNT_ALIAS_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.take();
}

fn resolve_account_alias(label: &str, domain: &DomainId) -> Option<AccountId> {
    let guard = ACCOUNT_ALIAS_RESOLVER
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.as_ref().and_then(|resolver| resolver(label, domain))
}

/// Function type used to resolve domain selectors into canonical [`DomainId`] values.
pub type AccountDomainSelectorResolver =
    dyn Fn(&AccountDomainSelector) -> Option<DomainId> + Send + Sync + 'static;

static ACCOUNT_DOMAIN_SELECTOR_RESOLVER: std::sync::LazyLock<
    RwLock<Option<Arc<AccountDomainSelectorResolver>>>,
> = std::sync::LazyLock::new(|| RwLock::new(None));

/// Install a global domain-selector resolver consulted by [`AccountId::from_str`].
pub fn set_account_domain_selector_resolver(resolver: Arc<AccountDomainSelectorResolver>) {
    let mut guard = ACCOUNT_DOMAIN_SELECTOR_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(resolver);
}

/// Clear the globally installed domain-selector resolver.
pub fn clear_account_domain_selector_resolver() {
    let mut guard = ACCOUNT_DOMAIN_SELECTOR_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.take();
}

fn resolve_account_domain_selector(selector: &AccountDomainSelector) -> Option<DomainId> {
    let guard = ACCOUNT_DOMAIN_SELECTOR_RESOLVER
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.as_ref().and_then(|resolver| resolver(selector))
}

/// Function type used to resolve UAID values into canonical [`AccountId`] values.
pub type AccountUaidResolver =
    dyn Fn(&UniversalAccountId) -> Option<AccountId> + Send + Sync + 'static;

static ACCOUNT_UAID_RESOLVER: std::sync::LazyLock<RwLock<Option<Arc<AccountUaidResolver>>>> =
    std::sync::LazyLock::new(|| RwLock::new(None));

/// Install a global UAID resolver consulted by [`AccountId::from_str`].
pub fn set_account_uaid_resolver(resolver: Arc<AccountUaidResolver>) {
    let mut guard = ACCOUNT_UAID_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(resolver);
}

/// Clear the globally installed UAID resolver.
pub fn clear_account_uaid_resolver() {
    let mut guard = ACCOUNT_UAID_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.take();
}

fn resolve_account_uaid(uaid: &UniversalAccountId) -> Option<AccountId> {
    let guard = ACCOUNT_UAID_RESOLVER
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.as_ref().and_then(|resolver| resolver(uaid))
}

/// Function type used to resolve opaque account identifiers into UAIDs.
pub type AccountOpaqueResolver =
    dyn Fn(&OpaqueAccountId) -> Option<UniversalAccountId> + Send + Sync + 'static;

static ACCOUNT_OPAQUE_RESOLVER: std::sync::LazyLock<RwLock<Option<Arc<AccountOpaqueResolver>>>> =
    std::sync::LazyLock::new(|| RwLock::new(None));

/// Install a global opaque-id resolver consulted by [`AccountId::from_str`].
pub fn set_account_opaque_resolver(resolver: Arc<AccountOpaqueResolver>) {
    let mut guard = ACCOUNT_OPAQUE_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(resolver);
}

/// Clear the globally installed opaque-id resolver.
pub fn clear_account_opaque_resolver() {
    let mut guard = ACCOUNT_OPAQUE_RESOLVER
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.take();
}

fn resolve_account_opaque_id(opaque: &OpaqueAccountId) -> Option<UniversalAccountId> {
    let guard = ACCOUNT_OPAQUE_RESOLVER
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.as_ref().and_then(|resolver| resolver(opaque))
}

#[model]
mod model {
    use super::*;
    use crate::account::rekey::AccountLabel;

    /// Identification of [`Account`] by the [`Domain`](crate::domain::Domain) it belongs to
    /// and the controller governing its authorisation policy.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use iroha_crypto::{Algorithm, KeyPair};
    /// use iroha_data_model::{account::AccountId, domain::DomainId};
    ///
    /// let domain: DomainId = "wonderland".parse().expect("valid domain");
    /// let keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
    /// let id = AccountId::new(domain, keypair.public_key().clone());
    /// let parsed: AccountId = id
    ///     .to_string()
    ///     .parse()
    ///     .expect("IH58 account identifier must parse");
    /// assert_eq!(parsed, id);
    /// ```
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AccountId {
        /// [`Domain`](crate::domain::Domain) that the [`Account`] belongs to.
        pub domain: DomainId,
        /// Controller responsible for authorising account actions.
        pub controller: AccountController,
    }

    /// Account entity is an authority which is used to execute `Iroha Special Instructions`.
    #[derive(
        derive_more::Debug, Clone, IdEqOrdHash, Decode, Encode, IntoSchema, RegistrableBuilder,
    )]
    #[allow(clippy::multiple_inherent_impl)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct Account {
        /// Identification of the [`Account`].
        pub id: AccountId,
        /// Metadata of this account as a key-value store.
        #[registrable_builder(default = Metadata::default())]
        pub metadata: Metadata,
        /// Stable label under which the account is addressed (if provided).
        #[registrable_builder(default = None)]
        #[norito(default)]
        pub label: Option<AccountLabel>,
        /// Universal account identifier bound to this account (if registered in Nexus).
        #[registrable_builder(default = None)]
        #[norito(default)]
        pub uaid: Option<crate::nexus::UniversalAccountId>,
        /// Opaque identifiers bound to this account's UAID.
        #[registrable_builder(default = Vec::new())]
        #[norito(default)]
        pub opaque_ids: Vec<OpaqueAccountId>,
    }
}

string_id!(AccountId);

/// Source that produced an [`AccountId`] when parsing textual account identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountAddressSource {
    /// The identifier was supplied using one of the encoded address formats.
    Encoded(AccountAddressFormat),
    /// The identifier was resolved through the global alias resolver.
    Alias,
    /// The identifier was expressed directly as a public key.
    RawPublicKey,
    /// The identifier was resolved through a UAID lookup.
    Uaid,
    /// The identifier was resolved through an opaque-id lookup.
    Opaque,
}

/// Result returned by [`AccountId::parse`] providing both the identifier and its canonical layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedAccountId {
    account_id: AccountId,
    canonical: String,
    source: AccountAddressSource,
}

impl ParsedAccountId {
    /// Borrow the parsed [`AccountId`].
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    /// Borrow the canonical textual representation (IH58).
    #[must_use]
    pub fn canonical(&self) -> &str {
        &self.canonical
    }

    /// Inspect how the identifier was supplied.
    #[must_use]
    pub fn source(&self) -> AccountAddressSource {
        self.source
    }

    /// Consume the result, yielding the parsed [`AccountId`].
    pub fn into_account_id(self) -> AccountId {
        self.account_id
    }

    /// Consume the result into all captured components.
    #[must_use]
    pub fn into_parts(self) -> (AccountId, String, AccountAddressSource) {
        (self.account_id, self.canonical, self.source)
    }
}

/// Opaque identifier that maps to a UAID without disclosing raw PII.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct OpaqueAccountId(Hash);

impl OpaqueAccountId {
    /// Construct an opaque identifier from a pre-hashed value.
    #[must_use]
    pub fn from_hash(hash: Hash) -> Self {
        Self(hash)
    }

    /// Borrow the underlying hash.
    #[must_use]
    pub fn as_hash(&self) -> &Hash {
        &self.0
    }
}

impl fmt::Display for OpaqueAccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "opaque:{}", self.0)
    }
}

impl From<Hash> for OpaqueAccountId {
    fn from(value: Hash) -> Self {
        Self::from_hash(value)
    }
}

impl From<OpaqueAccountId> for Hash {
    fn from(value: OpaqueAccountId) -> Self {
        value.0
    }
}

impl FromStr for OpaqueAccountId {
    type Err = iroha_crypto::error::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let hex_literal = match trimmed.get(..7) {
            Some(prefix) if prefix.eq_ignore_ascii_case("opaque:") => trimmed[7..].trim(),
            _ => trimmed,
        };
        Hash::from_str(hex_literal).map(Self::from_hash)
    }
}

impl norito::NoritoSerialize for AccountId {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), norito::Error> {
        let normalized = (self.domain.clone(), self.controller.clone());
        norito::core::NoritoSerialize::serialize(&normalized, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let normalized = (self.domain.clone(), self.controller.clone());
        norito::core::NoritoSerialize::encoded_len_hint(&normalized)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let normalized = (self.domain.clone(), self.controller.clone());
        norito::core::NoritoSerialize::encoded_len_exact(&normalized)
    }
}

impl<'de> norito::NoritoDeserialize<'de> for AccountId {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("AccountId deserialization must succeed for valid archives")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let archived_tuple = archived.cast::<(DomainId, AccountController)>();
        if let Ok((domain, controller)) =
            norito::core::NoritoDeserialize::try_deserialize(archived_tuple)
        {
            return Ok(Self { domain, controller });
        }

        let archived_str = archived.cast::<String>();
        if let Ok(value) = norito::core::NoritoDeserialize::try_deserialize(archived_str)
            && let Ok(id) = value.parse::<AccountId>()
        {
            return Ok(id);
        }

        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let payload = norito::core::payload_slice_from_ptr(ptr)?;
        match norito::core::read_len_from_slice(payload) {
            Ok((len, header)) => {
                let end = header
                    .checked_add(len)
                    .ok_or(norito::core::Error::LengthMismatch)?;
                if end > payload.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                let raw = std::str::from_utf8(&payload[header..end])
                    .map_err(|_| norito::core::Error::InvalidUtf8)?;
                raw.parse::<AccountId>()
                    .map_err(|err| norito::core::Error::Message(err.to_string()))
            }
            Err(norito::core::Error::LengthMismatch) => {
                let raw =
                    std::str::from_utf8(payload).map_err(|_| norito::core::Error::InvalidUtf8)?;
                raw.parse::<AccountId>()
                    .map_err(|err| norito::core::Error::Message(err.to_string()))
            }
            Err(err) => Err(err),
        }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AccountId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        if let Ok(((domain, controller), used)) =
            norito::core::decode_field_canonical::<(DomainId, AccountController)>(bytes)
        {
            return Ok((Self { domain, controller }, used));
        }

        match norito::core::read_len_from_slice(bytes) {
            Ok((len, header)) => {
                let end = header
                    .checked_add(len)
                    .ok_or(norito::core::Error::LengthMismatch)?;
                if end <= bytes.len()
                    && let Ok(raw) = core::str::from_utf8(&bytes[header..end])
                    && let Ok(account) = raw.parse::<AccountId>()
                {
                    return Ok((account, end));
                }
            }
            Err(norito::core::Error::LengthMismatch) => {
                if let Ok(raw) = core::str::from_utf8(bytes)
                    && let Ok(account) = raw.parse::<AccountId>()
                {
                    return Ok((account, raw.len()));
                }
            }
            Err(err) => return Err(err),
        }

        match <String as norito::core::DecodeFromSlice>::decode_from_slice(bytes) {
            Ok((raw, used)) => {
                if let Ok(account) = raw.parse::<AccountId>() {
                    return Ok((account, used));
                }
            }
            Err(norito::core::Error::LengthMismatch) => {}
            Err(err) => return Err(err),
        }

        let ((domain, signatory), used) =
            norito::core::decode_field_canonical::<(DomainId, PublicKey)>(bytes)?;
        Ok((AccountId::new(domain, signatory), used))
    }
}

/// Read-only reference to [`Account`].
/// Used in query filters to avoid copying.
pub type AccountEntry<'world> = Ref<'world, AccountId, AccountValue>;

/// Canonical account data stored in the world state without duplicating the identifier.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AccountDetails {
    /// Arbitrary metadata attached to the account.
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    pub metadata: Metadata,
    /// Stable label referenced by rekey records.
    #[norito(default)]
    pub label: Option<rekey::AccountLabel>,
    /// Universal account identifier bound to this account, when applicable.
    #[norito(default)]
    pub uaid: Option<UniversalAccountId>,
    /// Opaque identifiers mapped to this account's UAID.
    #[norito(default)]
    pub opaque_ids: Vec<OpaqueAccountId>,
}

impl AccountDetails {
    /// Construct a new account details record.
    #[must_use]
    pub fn new(
        metadata: Metadata,
        label: Option<rekey::AccountLabel>,
        uaid: Option<UniversalAccountId>,
        opaque_ids: Vec<OpaqueAccountId>,
    ) -> Self {
        Self {
            metadata,
            label,
            uaid,
            opaque_ids,
        }
    }

    /// Get a reference to the attached metadata.
    #[must_use]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Get a mutable reference to the attached metadata.
    #[must_use]
    pub fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }

    /// Insert a metadata key/value pair, returning the previous value if present.
    pub fn insert(&mut self, key: Name, value: Json) -> Option<Json> {
        self.metadata.insert(key, value)
    }

    /// Remove a metadata entry by key, returning the removed value if present.
    #[cfg(feature = "transparent_api")]
    pub fn remove(&mut self, key: &Name) -> Option<Json> {
        self.metadata.remove(key)
    }

    /// Borrow the stable account label, if assigned.
    #[must_use]
    pub fn label(&self) -> Option<&rekey::AccountLabel> {
        self.label.as_ref()
    }

    /// Set or clear the stable account label.
    pub fn set_label(&mut self, label: Option<rekey::AccountLabel>) {
        self.label = label;
    }

    /// Borrow the universal account identifier attached to this account.
    #[must_use]
    pub fn uaid(&self) -> Option<&UniversalAccountId> {
        self.uaid.as_ref()
    }

    /// Assign a universal account identifier to this record.
    pub fn set_uaid(&mut self, uaid: Option<UniversalAccountId>) {
        self.uaid = uaid;
    }

    /// Borrow the opaque identifiers bound to this account.
    #[must_use]
    pub fn opaque_ids(&self) -> &[OpaqueAccountId] {
        &self.opaque_ids
    }

    /// Replace the opaque identifiers bound to this account.
    pub fn set_opaque_ids(&mut self, opaque_ids: Vec<OpaqueAccountId>) {
        self.opaque_ids = opaque_ids;
    }
}

impl Default for AccountDetails {
    fn default() -> Self {
        Self::new(Metadata::default(), None, None, Vec::new())
    }
}

/// [`Account`] without `id`.
/// Needed only for the world-state account map to reduce memory usage.
/// In other places use [`Account`] directly.
pub type AccountValue = Owned<AccountDetails>;

const ERR_ACCOUNT_LITERAL_FORMAT: &str = "AccountId must be IH58 (preferred)/snx1 (second-best)/0x, uaid:, opaque:, or `<alias|public_key>@<domain>`";
const ERR_DOMAIN_SELECTOR_UNRESOLVED: &str = "ERR_DOMAIN_SELECTOR_UNRESOLVED";
const ERR_UAID_UNRESOLVED: &str = "ERR_UAID_UNRESOLVED";
const ERR_OPAQUE_ID_UNRESOLVED: &str = "ERR_OPAQUE_ID_UNRESOLVED";
const ERR_INVALID_UAID_LITERAL: &str = "ERR_INVALID_UAID_LITERAL";
const ERR_INVALID_OPAQUE_LITERAL: &str = "ERR_INVALID_OPAQUE_ID_LITERAL";

impl AccountId {
    /// Construct a single-signature account identifier.
    #[inline]
    #[must_use]
    pub fn new(domain: DomainId, signatory: PublicKey) -> Self {
        Self {
            domain,
            controller: AccountController::single(signatory),
        }
    }

    /// Construct a multisignature account identifier.
    #[inline]
    #[must_use]
    pub fn new_multisig(domain: DomainId, policy: MultisigPolicy) -> Self {
        Self {
            domain,
            controller: AccountController::multisig(policy),
        }
    }

    /// Convenience alias for [`Self::new`].
    #[inline]
    #[must_use]
    pub fn of(domain: DomainId, signatory: PublicKey) -> Self {
        Self::new(domain, signatory)
    }

    /// Borrow the account domain.
    #[inline]
    #[must_use]
    pub fn domain(&self) -> &DomainId {
        &self.domain
    }

    /// Borrow the controller governing this account.
    #[inline]
    #[must_use]
    pub fn controller(&self) -> &AccountController {
        &self.controller
    }

    /// Borrow the single-signature public key.
    #[inline]
    #[must_use]
    pub fn signatory(&self) -> &PublicKey {
        self.expect_single_signatory()
    }

    /// Borrow the single-signature public key when present.
    #[inline]
    #[must_use]
    pub fn try_signatory(&self) -> Option<&PublicKey> {
        self.controller.single_signatory()
    }

    /// Replace the account signatory, converting the controller to single-key.
    #[inline]
    pub fn set_signatory(&mut self, signatory: PublicKey) {
        self.controller = AccountController::single(signatory);
    }

    /// Borrow the single-signature public key, panicking if the controller is not single-key.
    #[inline]
    #[must_use]
    pub fn expect_single_signatory(&self) -> &PublicKey {
        self.controller
            .single_signatory()
            .expect("account controller is not single-key")
    }

    /// Borrow the multisignature policy when configured.
    #[inline]
    #[must_use]
    pub fn multisig_policy(&self) -> Option<&MultisigPolicy> {
        self.controller.multisig_policy()
    }

    /// Return `true` if the account signatory matches the given `public_key`.
    #[inline]
    #[cfg(feature = "transparent_api")]
    pub fn signatory_matches(&self, public_key: &PublicKey) -> bool {
        self.try_signatory().is_some_and(|pk| pk == public_key)
    }

    /// Construct the address payload used for IH58 (preferred)/snx1 (second-best) encoders.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the account identifier cannot be encoded
    /// into an [`AccountAddress`] (for example, when the controller configuration lacks support).
    #[inline]
    pub fn to_account_address(&self) -> Result<AccountAddress, AccountAddressError> {
        AccountAddress::from_account_id(self)
    }

    /// Encode the account as an IH58 string for the provided network prefix.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the account cannot be encoded or if IH58
    /// conversion fails for the provided network prefix.
    #[inline]
    pub fn to_ih58(&self, network_prefix: u16) -> Result<String, AccountAddressError> {
        self.to_account_address()?.to_ih58(network_prefix)
    }

    /// Encode the account as canonical IH58 using the configured chain discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] when address encoding fails.
    #[inline]
    pub fn canonical_ih58(&self) -> Result<String, AccountAddressError> {
        let prefix = address::chain_discriminant();
        self.to_account_address()?.to_ih58(prefix)
    }

    /// Encode the account as canonical lowercase hexadecimal.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] when canonical payload construction fails.
    #[inline]
    pub fn to_canonical_hex(&self) -> Result<String, AccountAddressError> {
        self.to_account_address()?.canonical_hex()
    }

    /// Parse an account identifier from text, returning the canonical representation and source.
    ///
    /// This helper funnels all parsing through one code path so callers no longer need to
    /// distinguish between raw public keys, encoded addresses, aliases, or UAID/opaque lookups.
    /// The returned canonical string always matches the canonical IH58 representation.
    ///
    /// # Errors
    ///
    /// Propagates [`ParseError`] when the textual representation is invalid.
    pub fn parse(input: &str) -> Result<ParsedAccountId, ParseError> {
        let (account_id, source) = Self::parse_internal(input)?;
        let canonical = account_id
            .canonical_ih58()
            .map_err(|err| ParseError::new(err.code_str()))?;
        Ok(ParsedAccountId {
            account_id,
            canonical,
            source,
        })
    }

    /// Canonicalise a textual identifier into the IH58 form.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the provided input is invalid.
    pub fn canonicalize(input: &str) -> Result<String, ParseError> {
        Self::parse(input).map(|parsed| parsed.canonical)
    }

    fn parse_internal(input: &str) -> Result<(Self, AccountAddressSource), ParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT));
        }

        if let Some(rest) = Self::strip_prefix_case_insensitive(trimmed, "uaid:") {
            let uaid = UniversalAccountId::from_str(rest)
                .map_err(|_| ParseError::new(ERR_INVALID_UAID_LITERAL))?;
            let account =
                resolve_account_uaid(&uaid).ok_or_else(|| ParseError::new(ERR_UAID_UNRESOLVED))?;
            return Ok((account, AccountAddressSource::Uaid));
        }

        if let Some(rest) = Self::strip_prefix_case_insensitive(trimmed, "opaque:") {
            let opaque = OpaqueAccountId::from_str(rest)
                .map_err(|_| ParseError::new(ERR_INVALID_OPAQUE_LITERAL))?;
            let uaid = resolve_account_opaque_id(&opaque)
                .ok_or_else(|| ParseError::new(ERR_OPAQUE_ID_UNRESOLVED))?;
            let account =
                resolve_account_uaid(&uaid).ok_or_else(|| ParseError::new(ERR_UAID_UNRESOLVED))?;
            return Ok((account, AccountAddressSource::Opaque));
        }

        let split_err = match split_nonempty(
            trimmed,
            '@',
            ERR_ACCOUNT_LITERAL_FORMAT,
            "Empty `identifier` part in `<identifier>@<domain>`",
            "Empty `domain` part in `<identifier>@<domain>`",
        ) {
            Ok((address_part, domain_part)) => {
                let domain = domain_part.parse().map_err(|_| ParseError {
                    reason: "Failed to parse `domain` part in `<identifier>@<domain>`",
                })?;
                let expected_prefix = address::chain_discriminant();
                match AccountAddress::parse_any(address_part, Some(expected_prefix)) {
                    Ok((address, format)) => {
                        address
                            .ensure_domain_matches(&domain)
                            .map_err(|err| ParseError::new(err.code_str()))?;
                        let controller = address
                            .to_account_controller()
                            .map_err(|err| ParseError::new(err.code_str()))?;
                        return Ok((
                            Self { domain, controller },
                            AccountAddressSource::Encoded(format),
                        ));
                    }
                    Err(
                        AccountAddressError::UnsupportedAddressFormat
                        | AccountAddressError::ChecksumMismatch,
                    ) => {}
                    Err(err) => return Err(ParseError::new(err.code_str())),
                }

                if let Some(alias_account) = resolve_account_alias(address_part, &domain) {
                    if alias_account.domain() != &domain {
                        return Err(ParseError::new(
                            "Alias resolved to an account in a different domain",
                        ));
                    }
                    return Ok((alias_account, AccountAddressSource::Alias));
                }

                let signatory: PublicKey = address_part.parse().map_err(|_| {
                    ParseError::new("Failed to parse `identifier` part in `<identifier>@<domain>`")
                })?;
                return Ok((
                    Self::new(domain, signatory),
                    AccountAddressSource::RawPublicKey,
                ));
            }
            Err(err) => err,
        };

        Self::parse_address_literal(trimmed, split_err)
    }

    fn parse_address_literal(
        input: &str,
        fallback_err: ParseError,
    ) -> Result<(Self, AccountAddressSource), ParseError> {
        let expected_prefix = address::chain_discriminant();
        match AccountAddress::parse_any(input, Some(expected_prefix)) {
            Ok((address, format)) => {
                let selector = address.domain_selector();
                let resolved = resolve_account_domain_selector(&selector).or_else(|| {
                    if matches!(selector, AccountDomainSelector::Default) {
                        let default_domain = address::default_domain_name();
                        default_domain.as_ref().parse().ok()
                    } else {
                        None
                    }
                });
                let domain =
                    resolved.ok_or_else(|| ParseError::new(ERR_DOMAIN_SELECTOR_UNRESOLVED))?;
                address
                    .ensure_domain_matches(&domain)
                    .map_err(|err| ParseError::new(err.code_str()))?;
                let controller = address
                    .to_account_controller()
                    .map_err(|err| ParseError::new(err.code_str()))?;
                Ok((
                    Self { domain, controller },
                    AccountAddressSource::Encoded(format),
                ))
            }
            Err(AccountAddressError::UnsupportedAddressFormat) => Err(fallback_err),
            Err(err) => Err(ParseError::new(err.code_str())),
        }
    }

    fn strip_prefix_case_insensitive<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
        let head = input.get(..prefix.len())?;
        if head.eq_ignore_ascii_case(prefix) {
            Some(&input[prefix.len()..])
        } else {
            None
        }
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ih58 = self.canonical_ih58().map_err(|_| fmt::Error)?;
        f.write_str(&ih58)
    }
}

impl fmt::Debug for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

const _: fn() = || {
    fn assert_decode<'a, T: norito::core::DecodeFromSlice<'a>>() {}
    fn assert_account() {
        assert_decode::<AccountId>();
    }
    let _ = assert_account;
};

impl Account {
    /// Construct builder for [`Account`] identifiable by [`AccountId`] containing the given signatory.
    #[inline]
    #[must_use]
    pub fn new(id: AccountId) -> <Self as Registered>::With {
        <Self as Registered>::With::new(id)
    }

    /// Return a reference to the account signatory, panicking if the controller is not single-key.
    #[inline]
    #[must_use]
    pub fn signatory(&self) -> &PublicKey {
        self.id.signatory()
    }

    /// Borrow the account signatory when present.
    #[inline]
    #[must_use]
    pub fn try_signatory(&self) -> Option<&PublicKey> {
        self.id.try_signatory()
    }

    /// Return the controller governing this account.
    #[inline]
    #[must_use]
    pub fn controller(&self) -> &AccountController {
        self.id.controller()
    }

    /// Borrow the canonical account label, if one is assigned.
    #[inline]
    #[must_use]
    pub fn label(&self) -> Option<&rekey::AccountLabel> {
        self.label.as_ref()
    }

    /// Borrow the universal account identifier, if assigned.
    #[inline]
    #[must_use]
    pub fn uaid(&self) -> Option<&UniversalAccountId> {
        self.uaid.as_ref()
    }

    /// Borrow the opaque identifiers bound to this account.
    #[inline]
    #[must_use]
    pub fn opaque_ids(&self) -> &[OpaqueAccountId] {
        &self.opaque_ids
    }
}

#[cfg(feature = "transparent_api")]
impl NewAccount {
    /// Convert into [`Account`].
    pub fn into_account(self) -> Account {
        Account {
            id: self.id,
            metadata: self.metadata,
            label: self.label,
            uaid: self.uaid,
            opaque_ids: self.opaque_ids,
        }
    }
}

impl NewAccount {
    /// Remove the label assigned to this builder, if any.
    #[must_use]
    pub fn without_label(mut self) -> Self {
        self.label = None;
        self
    }

    /// Borrow the currently assigned label on the builder.
    #[must_use]
    pub fn label(&self) -> Option<&rekey::AccountLabel> {
        self.label.as_ref()
    }
}

impl HasMetadata for NewAccount {
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

impl HasMetadata for Account {
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let controller_desc = match self.controller() {
            AccountController::Single(signatory) => signatory.to_string(),
            AccountController::Multisig(policy) => format!(
                "multisig(threshold={}, members={})",
                policy.threshold(),
                policy.members().len()
            ),
        };
        write!(
            f,
            "Account{{id: {}, domain: {}, controller: {controller_desc}}}",
            self.id,
            self.id.domain()
        )
    }
}

impl FromStr for AccountId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_internal(s).map(|(account_id, _)| account_id)
    }
}

#[cfg(test)]
mod account_id_parsing_tests {
    use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::{core::decode_from_bytes, to_bytes};

    use super::*;
    use crate::domain::DomainId;

    static DOMAIN_SELECTOR_RESOLVER_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    fn guard_alias_resolver() -> MutexGuard<'static, ()> {
        static ALIAS_RESOLVER_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        ALIAS_RESOLVER_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    struct DomainSelectorResolverGuard {
        _lock: MutexGuard<'static, ()>,
    }

    impl Drop for DomainSelectorResolverGuard {
        fn drop(&mut self) {
            clear_account_domain_selector_resolver();
        }
    }

    fn guard_domain_selector_resolver(
        resolver: Arc<AccountDomainSelectorResolver>,
    ) -> DomainSelectorResolverGuard {
        let lock = DOMAIN_SELECTOR_RESOLVER_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_account_domain_selector_resolver(resolver);
        DomainSelectorResolverGuard { _lock: lock }
    }

    fn guard_domain_selector_resolver_clear() -> DomainSelectorResolverGuard {
        let lock = DOMAIN_SELECTOR_RESOLVER_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        clear_account_domain_selector_resolver();
        DomainSelectorResolverGuard { _lock: lock }
    }

    fn guard_chain_discriminant() -> MutexGuard<'static, ()> {
        static CHAIN_DISCRIMINANT_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        CHAIN_DISCRIMINANT_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn guard_default_domain_label() -> MutexGuard<'static, ()> {
        static DEFAULT_DOMAIN_LABEL_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        DEFAULT_DOMAIN_LABEL_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    struct DefaultDomainReset(Arc<str>);

    impl Drop for DefaultDomainReset {
        fn drop(&mut self) {
            let _ = address::set_default_domain_name(self.0.as_ref().to_owned());
        }
    }

    fn reset_default_domain_to_default() -> DefaultDomainReset {
        let previous = address::default_domain_name();
        address::set_default_domain_name(address::DEFAULT_DOMAIN_NAME)
            .expect("reset default domain label");
        DefaultDomainReset(previous)
    }

    #[test]
    fn from_str_accepts_public_key_addresses() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("parse public key literal");
        let raw = format!("{public_key}@{domain}");

        let parsed: AccountId = raw.parse().expect("canonical account id must parse");
        assert_eq!(parsed.domain(), &domain);
        assert_eq!(parsed.signatory(), &public_key);
    }

    #[test]
    fn from_str_accepts_canonical_hex_addresses_without_domain() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let canonical = account.to_canonical_hex().expect("canonical hex encoding");
        let parsed: AccountId = canonical
            .parse()
            .expect("0x-prefixed account id must parse");
        assert_eq!(parsed, account);
    }

    #[test]
    fn encoded_literals_with_domain_parse_without_resolver() {
        let _guard = guard_domain_selector_resolver_clear();
        let domain: DomainId = "fallback-domain".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0x5A; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("address encodes");
        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("IH58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");
        let canonical_hex = address.canonical_hex().expect("canonical hex encode");
        let domain_suffix = domain.to_string();

        for literal in [
            format!("{ih58}@{domain_suffix}"),
            format!("{compressed}@{domain_suffix}"),
            format!("{canonical_hex}@{domain_suffix}"),
        ] {
            let parsed = AccountId::parse(&literal)
                .expect("encoded literal should parse with explicit domain")
                .into_account_id();
            assert_eq!(parsed, account);
        }
    }

    #[test]
    fn from_str_resolves_alias_via_global_resolver() {
        let _guard = guard_alias_resolver();
        clear_account_alias_resolver();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let domain_for_resolver = domain.clone();
        let account_for_resolver = account.clone();
        set_account_alias_resolver(Arc::new(move |label, alias_domain| {
            if label.eq_ignore_ascii_case("blue-alias") && alias_domain == &domain_for_resolver {
                Some(account_for_resolver.clone())
            } else {
                None
            }
        }));

        let parsed: AccountId = "blue-alias@wonderland"
            .parse()
            .expect("alias should resolve");
        assert_eq!(parsed, account);
        clear_account_alias_resolver();
    }

    #[test]
    fn from_str_resolves_base58_like_alias() {
        let _guard = guard_alias_resolver();
        clear_account_alias_resolver();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let account_for_resolver = account.clone();
        let alias_label = "primary";
        let err = AccountAddress::parse_any(alias_label, Some(address::chain_discriminant()))
            .expect_err("alias label should not parse as a valid address");
        assert_eq!(err.code_str(), "ERR_CHECKSUM_MISMATCH");
        set_account_alias_resolver(Arc::new(move |label, alias_domain| {
            if label == alias_label && alias_domain == &domain {
                Some(account_for_resolver.clone())
            } else {
                None
            }
        }));

        let parsed: AccountId = "primary@wonderland"
            .parse()
            .expect("alias matching the IH58 alphabet should still resolve");
        assert_eq!(parsed, account);
        clear_account_alias_resolver();
    }

    #[test]
    fn from_str_rejects_alias_domain_mismatch() {
        let _guard = guard_alias_resolver();
        clear_account_alias_resolver();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xBA; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let account_for_resolver = account.clone();
        set_account_alias_resolver(Arc::new(move |label, _| {
            if label == "blue-alias" {
                Some(account_for_resolver.clone())
            } else {
                None
            }
        }));

        let err = "blue-alias@otherland"
            .parse::<AccountId>()
            .expect_err("mismatched alias domain must fail");
        clear_account_alias_resolver();
        assert!(
            err.reason()
                .contains("Alias resolved to an account in a different domain"),
            "unexpected error message: {}",
            err.reason()
        );
    }

    #[test]
    fn parse_reports_encoded_source() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("default domain parses");
        let key_pair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());
        let canonical_hex = account.to_canonical_hex().expect("canonical hex encoding");
        let parsed = AccountId::parse(&canonical_hex).expect("encoded account id must parse");
        assert_eq!(
            parsed.source(),
            AccountAddressSource::Encoded(AccountAddressFormat::CanonicalHex)
        );
        assert_eq!(parsed.canonical(), parsed.account_id().to_string());
    }

    #[test]
    fn parse_reports_public_key_source() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
        let raw = format!("{public_key}@{domain}");

        let parsed = AccountId::parse(&raw).expect("public key account id must parse");
        assert_eq!(parsed.source(), AccountAddressSource::RawPublicKey);
        assert_eq!(parsed.canonical(), parsed.account_id().to_string());
    }

    #[test]
    fn default_domain_addresses_can_omit_domain_suffix() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("default domain parses");
        let key_pair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let address =
            AccountAddress::from_account_id(&account).expect("default-domain address encodes");

        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("ih58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");

        for (literal, expected_format) in [
            (
                ih58,
                AccountAddressSource::Encoded(AccountAddressFormat::IH58 {
                    network_prefix: address::chain_discriminant(),
                }),
            ),
            (
                compressed,
                AccountAddressSource::Encoded(AccountAddressFormat::Compressed),
            ),
        ] {
            let parsed = AccountId::parse(&literal)
                .unwrap_or_else(|err| panic!("default-domain literal should parse: {err}"));
            assert_eq!(parsed.account_id(), &account);
            assert_eq!(parsed.canonical(), account.to_string());
            assert_eq!(parsed.source(), expected_format);
        }
    }

    #[test]
    fn canonicalize_normalizes_default_domain_literals() {
        let _guard = guard_chain_discriminant();
        let _guard_label = guard_default_domain_label();
        let _reset_label = reset_default_domain_to_default();
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("default domain parses");
        let key_pair = KeyPair::from_seed(vec![0xDA; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let address =
            AccountAddress::from_account_id(&account).expect("default-domain address encodes");

        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("IH58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");
        let expected = account.to_string();

        for literal in [ih58, compressed] {
            let canonical = AccountId::canonicalize(&literal)
                .unwrap_or_else(|err| panic!("implicit literal should canonicalize: {err}"));
            assert_eq!(
                canonical, expected,
                "canonical form should normalize to IH58"
            );
        }
    }

    #[test]
    fn implicit_literals_require_domain_selector_resolver() {
        let _guard_chain = guard_chain_discriminant();
        let _guard_label = guard_default_domain_label();
        let _reset_label = reset_default_domain_to_default();
        address::set_default_domain_name("implicit-default-domain-test")
            .expect("configure test default domain label");

        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");

        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("IH58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");

        for literal in [ih58, compressed] {
            let err = AccountId::canonicalize(&literal).expect_err(
                "non-default implicit literal should be rejected without a selector resolver",
            );
            assert_eq!(
                err.reason(),
                "ERR_DOMAIN_SELECTOR_UNRESOLVED",
                "unexpected parse error for literal without domain suffix"
            );
        }
    }

    #[test]
    fn implicit_literals_resolve_with_domain_selector_resolver() {
        let _guard_chain = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("IH58 encode");
        let selector = AccountDomainSelector::from_domain(&domain).expect("selector");
        let resolver_domain = domain.clone();
        let _resolver_guard = guard_domain_selector_resolver(Arc::new(move |candidate| {
            if candidate == &selector {
                Some(resolver_domain.clone())
            } else {
                None
            }
        }));

        let parsed = AccountId::parse(&ih58).expect("selector-resolved literal should parse");
        assert_eq!(parsed.account_id(), &account);
        assert_eq!(parsed.canonical(), account.to_string());
    }

    #[test]
    fn norito_roundtrip_non_default_domain_without_resolver() {
        let _guard = guard_domain_selector_resolver(Arc::new(|_| None));
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xEF; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());

        let framed = to_bytes(&account).expect("encode account id");
        let decoded = decode_from_bytes::<AccountId>(&framed).expect("decode account id");

        assert_eq!(decoded, account);
    }

    #[test]
    fn norito_roundtrip_default_domain_without_resolver() {
        let _guard_chain = guard_chain_discriminant();
        let _guard_label = guard_default_domain_label();
        let _reset_label = reset_default_domain_to_default();
        let _guard = guard_domain_selector_resolver(Arc::new(|_| None));
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("default domain parses");
        let key_pair = KeyPair::from_seed(vec![0xEF; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());

        let framed = to_bytes(&account).expect("encode account id");
        let decoded = decode_from_bytes::<AccountId>(&framed).expect("decode account id");

        assert_eq!(decoded, account);
    }

    #[test]
    fn parse_reports_alias_source() {
        struct Reset(u16);

        impl Drop for Reset {
            fn drop(&mut self) {
                address::set_chain_discriminant(self.0);
            }
        }

        let _alias_guard = guard_alias_resolver();
        let _chain_guard = guard_chain_discriminant();

        let previous_chain_discriminant = address::set_chain_discriminant(42);
        let _reset = Reset(previous_chain_discriminant);
        clear_account_alias_resolver();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let account_for_resolver = account.clone();
        set_account_alias_resolver(Arc::new(move |label, alias_domain| {
            if label == "blue-alias" && alias_domain == &domain {
                Some(account_for_resolver.clone())
            } else {
                None
            }
        }));

        let parsed = AccountId::parse("blue-alias@wonderland").expect("alias must parse");
        clear_account_alias_resolver();

        assert_eq!(parsed.account_id(), &account);
        assert_eq!(parsed.source(), AccountAddressSource::Alias);
        assert_eq!(parsed.canonical(), account.to_string());
    }

    #[test]
    fn canonicalize_normalizes_input() {
        let _guard = guard_chain_discriminant();
        let literal = "0x0201B8AE571B79C5A80F5834DA2B000120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
        let canonical = AccountId::canonicalize(literal).expect("canonicalize succeeds");
        assert!(
            !canonical.starts_with("0x"),
            "canonical form should emit IH58 instead of canonical hex, got {canonical}"
        );
        let parsed = AccountId::parse(literal).expect("literal parses");
        assert_eq!(canonical, parsed.account_id().to_string());
    }

    #[test]
    fn from_str_rejects_mismatched_ih58_prefix() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(42);
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let payload =
            address::AccountAddress::from_account_id(&account).expect("address encoding succeeds");
        let literal = payload
            .to_ih58(41)
            .expect("encode ih58 with foreign prefix");

        let err = literal
            .parse::<AccountId>()
            .expect_err("prefix mismatch must fail");
        assert!(
            err.reason()
                .contains(AccountAddressErrorCode::UnexpectedNetworkPrefix.as_str()),
            "expected ERR_UNEXPECTED_NETWORK_PREFIX, got {}",
            err.reason()
        );

        address::set_chain_discriminant(previous);
    }

    #[test]
    fn from_str_accepts_configured_ih58_prefix() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(7);
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xBB; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let payload =
            address::AccountAddress::from_account_id(&account).expect("address encoding succeeds");
        let literal = payload
            .to_ih58(7)
            .expect("encode ih58 with configured prefix");

        let parsed = literal
            .parse::<AccountId>()
            .expect("matching prefix should parse");
        assert_eq!(parsed, account);

        address::set_chain_discriminant(previous);
    }

    #[test]
    fn from_str_rejects_encoded_address_with_domain_suffix() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let payload =
            address::AccountAddress::from_account_id(&account).expect("address encoding succeeds");
        let literal = format!(
            "{}@{}",
            payload
                .to_ih58(address::chain_discriminant())
                .expect("encode ih58"),
            domain
        );

        let err = literal
            .parse::<AccountId>()
            .expect_err("encoded address with domain should be rejected");
        assert_eq!(err.reason(), ERR_ACCOUNT_LITERAL_FORMAT);
    }

    #[test]
    fn display_uses_chain_discriminant_prefix() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(73);
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let rendered = account.to_string();
        let (parsed, format) =
            AccountAddress::parse_any(&rendered, None).expect("display should parse as IH58");
        match format {
            AccountAddressFormat::IH58 { network_prefix } => assert_eq!(network_prefix, 73),
            other => panic!("expected IH58 display, got {other:?}"),
        }
        parsed
            .ensure_domain_matches(&domain)
            .expect("rendered address should encode the correct domain");
        address::set_chain_discriminant(previous);
    }
}

impl IntoKeyValue for Account {
    type Key = AccountId;
    type Value = AccountValue;
    fn into_key_value(self) -> (Self::Key, Self::Value) {
        (
            self.id,
            Owned::new(AccountDetails::new(
                self.metadata,
                self.label,
                self.uaid,
                self.opaque_ids,
            )),
        )
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{
        ACCOUNT_ADMISSION_POLICY_METADATA_KEY, Account, AccountAddress, AccountAddressFormat,
        AccountAddressSource, AccountAdmissionMode, AccountAdmissionPolicy, AccountAliasResolver,
        AccountController, AccountDomainSelector, AccountEntry, AccountId, AccountLabel,
        AccountOpaqueResolver, AccountRekeyRecord, AccountUaidResolver, AccountValue,
        MultisigMember, MultisigPolicy, NewAccount, OpaqueAccountId, ParsedAccountId,
        clear_account_alias_resolver, clear_account_domain_selector_resolver,
        clear_account_opaque_resolver, clear_account_uaid_resolver, set_account_alias_resolver,
        set_account_domain_selector_resolver, set_account_opaque_resolver,
        set_account_uaid_resolver,
    };
}

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

    use iroha_crypto::{Algorithm, Hash, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    struct DomainSelectorResolverGuard {
        _lock: MutexGuard<'static, ()>,
    }

    impl Drop for DomainSelectorResolverGuard {
        fn drop(&mut self) {
            clear_account_domain_selector_resolver();
        }
    }

    fn guard_domain_selector_resolver(
        resolver: Arc<AccountDomainSelectorResolver>,
    ) -> DomainSelectorResolverGuard {
        static DOMAIN_SELECTOR_RESOLVER_GUARD: LazyLock<Mutex<()>> =
            LazyLock::new(|| Mutex::new(()));
        let lock = DOMAIN_SELECTOR_RESOLVER_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set_account_domain_selector_resolver(resolver);
        DomainSelectorResolverGuard { _lock: lock }
    }

    #[test]
    fn parse_account_id() {
        let key_pair = KeyPair::random();
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("valid domain");
        let account_id = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let canonical = account_id
            .to_canonical_hex()
            .expect("canonical encoding should succeed");
        let parsed: AccountId = canonical.parse().expect("should be valid");
        assert_eq!(parsed.domain(), account_id.domain());
        assert_eq!(parsed.signatory(), key_pair.public_key());

        let _err_empty_address = "@domain"
            .parse::<AccountId>()
            .expect_err("@domain should not be valid");
        let _err_empty_domain = format!("{canonical}@")
            .parse::<AccountId>()
            .expect_err("address@ should not be valid");
        let _err_violates_format = format!("{canonical}#domain")
            .parse::<AccountId>()
            .expect_err("address#domain should not be valid");
    }

    #[test]
    fn account_signatory_exposed() {
        let key_pair = KeyPair::random();
        let public_key = key_pair.public_key().clone();
        let domain_id: DomainId = "wonderland".parse().expect("valid domain name");
        let account_id = AccountId::new(domain_id, public_key.clone());
        let account = Account::new(account_id.clone()).build(&account_id);
        assert_eq!(account.signatory(), &public_key);
    }

    #[test]
    fn display_renders_ih58_for_secp256k1() {
        let kp = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let account_id = AccountId::new(domain.clone(), kp.public_key().clone());

        let rendered = account_id.to_string();
        let parsed: AccountId = rendered.parse().expect("rendered IH58 must parse");
        assert_eq!(parsed, account_id);
    }

    #[test]
    fn rekey_record_uses_account_label() {
        let key_pair = KeyPair::random();
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let signatory = key_pair.public_key().clone();
        let account_id = AccountId::new(domain.clone(), signatory.clone());
        let label = rekey::AccountLabel::new(
            domain.clone(),
            "alice".parse::<Name>().expect("valid label"),
        );
        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: Some(label.clone()),
            uaid: None,
            opaque_ids: Vec::new(),
        };

        let record =
            rekey::AccountRekeyRecord::from_account(&account).expect("label must be present");
        assert_eq!(record.label, label);
        assert_eq!(record.active_signatory, signatory);
        assert!(record.previous_signatories.is_empty());
    }

    #[test]
    fn rekey_record_absent_without_label() {
        let key_pair = KeyPair::random();
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let account_id = AccountId::new(domain, key_pair.public_key().clone());
        let account = Account {
            id: account_id,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };

        assert!(rekey::AccountRekeyRecord::from_account(&account).is_none());
    }

    #[test]
    fn multisig_account_exposes_no_primary_signatory() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let members = vec![
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let account_id = AccountId::new_multisig(domain, policy);
        assert!(account_id.try_signatory().is_none());

        let account = Account {
            id: account_id,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };
        assert!(account.try_signatory().is_none());
    }

    #[test]
    fn multisig_account_id_roundtrip() {
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("domain id");
        let members = vec![
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
            MultisigMember::new(KeyPair::random().public_key().clone(), 2).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let account_id = AccountId::new_multisig(domain.clone(), policy.clone());
        let canonical = account_id
            .to_canonical_hex()
            .expect("canonical encoding should succeed");
        let parsed: AccountId = canonical.parse().expect("should parse multisig");
        let parsed_policy = parsed
            .multisig_policy()
            .expect("multisig policy should be present");
        assert_eq!(parsed_policy, &policy);
        assert!(parsed.try_signatory().is_none());
    }

    #[test]
    fn canonical_string_rejects_domain_mismatch() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let key_pair = KeyPair::random();
        let id = AccountId::new(domain, key_pair.public_key().clone());
        let encoded = id
            .to_canonical_hex()
            .expect("canonical encoding should succeed");
        let wrong_domain: DomainId = "another_domain".parse().expect("domain id");
        let wrong_domain_for_resolver = wrong_domain.clone();
        let selector = AccountDomainSelector::from_domain(id.domain()).expect("domain selector");
        let _guard = guard_domain_selector_resolver(Arc::new(move |candidate| {
            if candidate == &selector {
                Some(wrong_domain_for_resolver.clone())
            } else {
                None
            }
        }));
        let err = encoded
            .parse::<AccountId>()
            .expect_err("mismatched domain digest must not parse");
        assert_eq!(
            err.reason(),
            AccountAddressErrorCode::DomainMismatch.as_str()
        );
    }

    #[test]
    fn ih58_checksum_failure_reports_error_code() {
        // Negative vector from fixtures/account/address_vectors.json (`ih58-checksum-mismatch`).
        let literal = "RnuaJGGDL9CghX9U4iqYRMghp31xkGuCvqQTzXu9AF8kzt7etZdZeGqz";
        let err = literal
            .parse::<AccountId>()
            .expect_err("invalid IH58 payload must fail");
        assert_eq!(
            err.reason(),
            AccountAddressErrorCode::ChecksumMismatch.as_str()
        );
    }

    #[test]
    fn account_builder_carries_uaid_into_details() {
        let key_pair = KeyPair::random();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let account_id = AccountId::new(domain, key_pair.public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"uaid::builder"));

        let account = Account::new(account_id.clone())
            .with_uaid(Some(uaid))
            .build(&account_id);
        assert_eq!(account.uaid(), Some(&uaid));

        let (stored_id, stored_value) = account.clone().into_key_value();
        assert_eq!(stored_id, account_id);

        let mut details = stored_value.into_inner();
        assert_eq!(details.uaid(), Some(&uaid));

        details.set_uaid(None);
        assert!(details.uaid().is_none());
    }
}

#[cfg(all(test, feature = "json"))]
mod json_tests {
    use iroha_crypto::{Hash, KeyPair};

    use super::*;
    use crate::{domain::DomainId, metadata::Metadata, name::Name, nexus::UniversalAccountId};

    #[test]
    fn account_json_roundtrip() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain, keypair.public_key().clone());
        let account = Account {
            id,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        };

        let json = norito::json::to_json(&account).expect("serialize account");
        let decoded: Account = norito::json::from_json(&json).expect("deserialize account");

        assert_eq!(decoded, account);
    }

    #[test]
    fn new_account_json_roundtrip_defaults() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain, keypair.public_key().clone());
        let new_account = NewAccount::new(id);

        let json = norito::json::to_json(&new_account).expect("serialize new account");
        let decoded: NewAccount = norito::json::from_json(&json).expect("deserialize new account");

        assert_eq!(decoded, new_account);
        assert!(decoded.label.is_none());
        assert!(decoded.uaid.is_none());
        assert_eq!(decoded.metadata, Metadata::default());
    }

    #[test]
    fn new_account_json_roundtrip_with_label_and_uaid() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain.clone(), keypair.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert("title".parse().expect("key"), "queen");
        let label =
            rekey::AccountLabel::new(domain.clone(), "alice".parse::<Name>().expect("label name"));
        let uaid = UniversalAccountId::from_hash(Hash::prehashed([0xAB; 32]));

        let new_account = NewAccount {
            id: id.clone(),
            metadata: metadata.clone(),
            label: Some(label.clone()),
            uaid: Some(uaid),
            opaque_ids: Vec::new(),
        };

        let json = norito::json::to_json(&new_account).expect("serialize new account");
        let decoded: NewAccount = norito::json::from_json(&json).expect("deserialize new account");

        assert_eq!(decoded, new_account);
        assert_eq!(decoded.label, Some(label));
        assert_eq!(decoded.uaid, Some(uaid));
        assert_eq!(decoded.metadata, metadata);
    }

    #[test]
    fn new_account_json_missing_fields_apply_defaults() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain.clone(), keypair.public_key().clone());
        let payload = format!("{{\"id\":\"{id}\"}}");

        let decoded: NewAccount =
            norito::json::from_json(&payload).expect("deserialize with defaults");

        assert_eq!(decoded.id, id);
        assert_eq!(decoded.metadata, Metadata::default());
        assert!(decoded.label.is_none());
        assert!(decoded.uaid.is_none());
    }

    #[test]
    fn new_account_json_rejects_unknown_fields() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain, keypair.public_key().clone());
        let payload = format!("{{\"id\":\"{id}\",\"metadata\":{{}},\"extra\":true}}");

        let err = norito::json::from_json::<NewAccount>(&payload).expect_err("unknown field");
        match err {
            norito::json::Error::UnknownField { field } => assert_eq!(field, "extra"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
