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
use iroha_crypto::PublicKey;
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
    /// use iroha_data_model::account::AccountId;
    ///
    /// let id: AccountId =
    ///     "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
    ///         .parse()
    ///         .expect("canonical account address must parse");
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

    /// Borrow the canonical textual representation (`IH58@domain`).
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

impl norito::NoritoSerialize for AccountId {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), norito::Error> {
        let normalized = self.to_string();
        norito::core::NoritoSerialize::serialize(&normalized, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_hint(&self.to_string())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_exact(&self.to_string())
    }
}

impl<'de> norito::NoritoDeserialize<'de> for AccountId {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("AccountId deserialization must succeed for valid archives")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
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
}

impl AccountDetails {
    /// Construct a new account details record.
    #[must_use]
    pub fn new(
        metadata: Metadata,
        label: Option<rekey::AccountLabel>,
        uaid: Option<UniversalAccountId>,
    ) -> Self {
        Self {
            metadata,
            label,
            uaid,
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
}

impl Default for AccountDetails {
    fn default() -> Self {
        Self::new(Metadata::default(), None, None)
    }
}

/// [`Account`] without `id`.
/// Needed only for the world-state account map to reduce memory usage.
/// In other places use [`Account`] directly.
pub type AccountValue = Owned<AccountDetails>;

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

    /// Construct the address payload used for IH58/compressed encoders.
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
    /// distinguish between raw public keys, encoded addresses, or aliases. The returned canonical
    /// string always matches [`AccountId::to_string`].
    ///
    /// # Errors
    ///
    /// Propagates [`ParseError`] when the textual representation is invalid.
    pub fn parse(input: &str) -> Result<ParsedAccountId, ParseError> {
        let (account_id, source) = Self::parse_internal(input)?;
        let canonical = account_id.to_string();
        Ok(ParsedAccountId {
            account_id,
            canonical,
            source,
        })
    }

    /// Canonicalise a textual identifier into the `IH58@domain` form.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the provided input is invalid.
    pub fn canonicalize(input: &str) -> Result<String, ParseError> {
        Self::parse(input).map(|parsed| parsed.canonical)
    }

    fn parse_internal(input: &str) -> Result<(Self, AccountAddressSource), ParseError> {
        let split_err = match split_nonempty(
            input,
            '@',
            "AccountId should have format `address@domain`",
            "Empty `address` part in `address@domain`",
            "Empty `domain` part in `address@domain`",
        ) {
            Ok((address_part, domain_part)) => {
                let domain = domain_part.parse().map_err(|_| ParseError {
                    reason: "Failed to parse `domain` part in `address@domain`",
                })?;

                let expected_prefix = address::chain_discriminant();
                let mut encoded_err: Option<AccountAddressError> = None;
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
                    Err(AccountAddressError::UnsupportedAddressFormat) => {}
                    Err(err @ AccountAddressError::ChecksumMismatch) => {
                        encoded_err = Some(err);
                    }
                    Err(err) => {
                        return Err(ParseError::new(err.code_str()));
                    }
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
                    encoded_err.map_or_else(
                        || ParseError::new("Failed to parse `address` part in `address@domain`"),
                        |err| ParseError::new(err.code_str()),
                    )
                })?;
                return Ok((
                    Self::new(domain, signatory),
                    AccountAddressSource::RawPublicKey,
                ));
            }
            Err(err) => err,
        };

        Self::parse_implicit_default(input, split_err)
    }

    fn parse_implicit_default(
        input: &str,
        fallback_err: ParseError,
    ) -> Result<(Self, AccountAddressSource), ParseError> {
        const DEFAULT_DOMAIN_ONLY_REASON: &str = "ERR_DEFAULT_DOMAIN_IMPLICIT_ONLY";

        let expected_prefix = address::chain_discriminant();
        match AccountAddress::parse_any(input, Some(expected_prefix)) {
            Ok((address, format)) => {
                if address.domain_kind() != address::AddressDomainKind::Default {
                    return Err(ParseError::new(DEFAULT_DOMAIN_ONLY_REASON));
                }
                let default_domain = address::default_domain_name();
                let domain: DomainId = default_domain.as_ref().parse()?;
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
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_account_address() {
            Ok(address) => {
                let prefix = address::chain_discriminant();
                let ih58 = address.to_ih58(prefix).map_err(|_| fmt::Error)?;
                write!(f, "{ih58}@{}", self.domain())
            }
            Err(_) => {
                // Fallback to the raw `public_key@domain` representation for controllers
                // whose curve is not yet encodable via AccountAddress. This keeps
                // serialization working for preview algorithms such as secp256k1.
                self.try_signatory().map_or(Err(fmt::Error), |signatory| {
                    write!(f, "{}@{}", signatory, self.domain())
                })
            }
        }
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

    use super::*;
    use crate::domain::DomainId;

    fn guard_alias_resolver() -> MutexGuard<'static, ()> {
        static ALIAS_RESOLVER_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        ALIAS_RESOLVER_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
    fn from_str_accepts_hex_prefixed_public_keys() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let hex_prefixed = "0x0201b8ae571b79c5a80f5834da2b000120ce7fa46c9dce7ea4b125e2e36bdb63ea33073e7590ac92816ae1e861b7048b03";
        let raw = format!("{hex_prefixed}@{domain}");
        let parsed: AccountId = raw.parse().expect("0x-prefixed account id must parse");
        assert_eq!(parsed.domain(), &domain);
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
        set_account_alias_resolver(Arc::new(move |label, alias_domain| {
            if label == "bob" && alias_domain == &domain {
                Some(account_for_resolver.clone())
            } else {
                None
            }
        }));

        let parsed: AccountId = "bob@wonderland"
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
        let parsed = AccountId::parse(
            "0x0201b8ae571b79c5a80f5834da2b000120ce7fa46c9dce7ea4b125e2e36bdb63ea33073e7590ac92816ae1e861b7048b03@wonderland",
        )
        .expect("encoded account id must parse");
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
    fn canonicalize_adds_default_domain_suffix_for_implicit_addresses() {
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
                "canonical form should append the default domain suffix and normalize encoding"
            );
        }
    }

    #[test]
    fn implicit_literals_for_non_default_domains_are_rejected() {
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
                "non-default implicit literal should be rejected without an explicit domain",
            );
            assert_eq!(
                err.reason(),
                "ERR_DEFAULT_DOMAIN_IMPLICIT_ONLY",
                "unexpected parse error for literal without domain suffix"
            );
        }
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
        let literal = "0x0201B8AE571B79C5A80F5834DA2B000120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland";
        let canonical = AccountId::canonicalize(literal).expect("canonicalize succeeds");
        assert!(
            canonical.ends_with("@wonderland"),
            "canonical form should preserve the domain, got {canonical}"
        );
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
        let literal = format!(
            "{}@{}",
            payload
                .to_ih58(41)
                .expect("encode ih58 with foreign prefix"),
            domain
        );

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
        let literal = format!(
            "{}@{}",
            payload
                .to_ih58(7)
                .expect("encode ih58 with configured prefix"),
            domain
        );

        let parsed = literal
            .parse::<AccountId>()
            .expect("matching prefix should parse");
        assert_eq!(parsed, account);

        address::set_chain_discriminant(previous);
    }

    #[test]
    fn display_uses_chain_discriminant_prefix() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(73);
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let rendered = account.to_string();
        let (address_part, domain_part) = rendered
            .split_once('@')
            .expect("account display contains domain delimiter");
        assert_eq!(domain_part, "wonderland");
        let (parsed, format) =
            AccountAddress::parse_any(address_part, None).expect("display should parse as IH58");
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
            Owned::new(AccountDetails::new(self.metadata, self.label, self.uaid)),
        )
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{
        ACCOUNT_ADMISSION_POLICY_METADATA_KEY, Account, AccountAddress, AccountAddressFormat,
        AccountAddressSource, AccountAdmissionMode, AccountAdmissionPolicy, AccountAliasResolver,
        AccountController, AccountEntry, AccountId, AccountLabel, AccountRekeyRecord, AccountValue,
        MultisigMember, MultisigPolicy, NewAccount, ParsedAccountId, clear_account_alias_resolver,
        set_account_alias_resolver,
    };
}

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    #[test]
    fn parse_account_id() {
        let key_pair = KeyPair::random();
        let domain: DomainId = "domain".parse().expect("valid domain");
        let account_id = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let canonical = account_id
            .to_canonical_hex()
            .expect("canonical encoding should succeed");
        let encoded = format!("{canonical}@{domain}");

        let parsed: AccountId = encoded.parse().expect("should be valid");
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
        };
        assert!(account.try_signatory().is_none());
    }

    #[test]
    fn multisig_account_id_roundtrip() {
        let domain: DomainId = "nexus".parse().expect("domain id");
        let members = vec![
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
            MultisigMember::new(KeyPair::random().public_key().clone(), 2).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let account_id = AccountId::new_multisig(domain.clone(), policy.clone());
        let canonical = account_id
            .to_canonical_hex()
            .expect("canonical encoding should succeed");
        let encoded = format!("{canonical}@{domain}");

        let parsed: AccountId = encoded.parse().expect("should parse multisig");
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
        let mut encoded = id
            .to_canonical_hex()
            .expect("canonical encoding should succeed");
        encoded.push_str("@another_domain");
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
        let err = format!("{literal}@wonderland")
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
