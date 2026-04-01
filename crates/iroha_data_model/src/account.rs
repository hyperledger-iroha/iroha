//! Structures, traits and impls related to `Account`s.
use core::fmt;
use std::{format, io::Write, str::FromStr, string::String, vec::Vec};

pub use admission::{
    ACCOUNT_ADMISSION_POLICY_METADATA_KEY, AccountAdmissionMode, AccountAdmissionPolicy,
    DEFAULT_MAX_IMPLICIT_ACCOUNT_CREATIONS_PER_TX,
};
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model_derive::{IdEqOrdHash, model};
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::{
    model::*,
    rekey::{AccountAlias, AccountAliasDomain, AccountRekeyRecord},
};
pub mod address;
pub mod admission;
pub mod controller;
pub mod curve;
pub mod rekey;
pub use address::{
    AccountAddress, AccountAddressError, AccountAddressErrorCode, AccountDomainSelector,
};
pub use controller::{AccountController, MultisigMember, MultisigPolicy, MultisigPolicyError};

use crate::{
    HasMetadata, Identifiable, IntoKeyValue, Registered, Registrable,
    common::{Owned, Ref},
    error::ParseError,
    metadata::Metadata,
    name::Name,
    nexus::UniversalAccountId,
};

#[model]
mod model {
    use super::*;
    use crate::account::rekey::AccountAlias;

    /// Canonical domainless account identity keyed only by the authorization controller.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use iroha_crypto::{Algorithm, KeyPair};
    /// use iroha_data_model::account::AccountId;
    ///
    /// let keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
    /// let id = AccountId::new(keypair.public_key().clone());
    /// ```
    #[derive(Clone, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AccountId {
        /// Controller responsible for authorising account actions.
        pub controller: AccountController,
    }

    /// Account entity is an authority which is used to execute `Iroha Special Instructions`.
    #[derive(derive_more::Debug, Clone, IdEqOrdHash, Decode, Encode, IntoSchema)]
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
        pub metadata: Metadata,
        /// Stable alias under which the account is addressed (if provided).
        #[norito(default)]
        pub label: Option<AccountAlias>,
        /// Universal account identifier bound to this account (if registered in Nexus).
        #[norito(default)]
        pub uaid: Option<crate::nexus::UniversalAccountId>,
        /// Opaque identifiers bound to this account's UAID.
        #[norito(default)]
        pub opaque_ids: Vec<OpaqueAccountId>,
    }

    /// Builder submitted in a transaction to register a canonical domainless account.
    #[derive(derive_more::Debug, Clone, IdEqOrdHash, Decode, Encode, IntoSchema)]
    #[allow(clippy::multiple_inherent_impl)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct NewAccount {
        /// Canonical domainless account identity.
        pub id: AccountId,
        /// Metadata supplied during registration.
        pub metadata: Metadata,
        /// Stable alias under which the account is addressed (if provided).
        #[norito(default)]
        pub label: Option<AccountAlias>,
        /// Universal account identifier bound to this account (if registered in Nexus).
        #[norito(default)]
        pub uaid: Option<crate::nexus::UniversalAccountId>,
        /// Opaque identifiers bound to this account's UAID.
        #[norito(default)]
        pub opaque_ids: Vec<OpaqueAccountId>,
    }
}

impl PartialEq for AccountId {
    fn eq(&self, other: &Self) -> bool {
        self.controller == other.controller
    }
}

impl Eq for AccountId {}

impl PartialOrd for AccountId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AccountId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.controller.cmp(&other.controller)
    }
}

impl core::hash::Hash for AccountId {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.controller.hash(state);
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AccountId {
    fn write_json(&self, out: &mut String) {
        let literal = self
            .canonical_i105()
            .expect("AccountId JSON serialization requires canonical I105 encoding");
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AccountId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        AccountId::parse_encoded(&value)
            .map(ParsedAccountId::into_account_id)
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

/// Source that produced an [`AccountId`] when parsing textual account identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountAddressSource {
    /// The identifier was supplied using one of the encoded address formats.
    Encoded,
}

/// Result returned by [`AccountId::parse_encoded`] providing both the identifier and its canonical layout.
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

    /// Borrow the canonical textual representation (i105).
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
        norito::core::NoritoSerialize::serialize(&self.controller, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_hint(&self.controller)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_exact(&self.controller)
    }
}

impl<'de> norito::NoritoDeserialize<'de> for AccountId {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("AccountId deserialization must succeed for valid archives")
    }

    fn try_deserialize(archived: &'de norito::core::Archived<Self>) -> Result<Self, norito::Error> {
        let archived_controller = archived.cast::<AccountController>();
        norito::core::NoritoDeserialize::try_deserialize(archived_controller)
            .map(|controller| Self { controller })
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AccountId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (controller, used) = norito::core::decode_field_canonical::<AccountController>(bytes)?;
        Ok((Self { controller }, used))
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
    /// Stable alias referenced by rekey records.
    #[norito(default)]
    pub label: Option<rekey::AccountAlias>,
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
        label: Option<rekey::AccountAlias>,
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

    /// Borrow the stable account alias, if assigned.
    #[must_use]
    pub fn label(&self) -> Option<&rekey::AccountAlias> {
        self.label.as_ref()
    }

    /// Set or clear the stable account alias.
    pub fn set_label(&mut self, label: Option<rekey::AccountAlias>) {
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

const ERR_ACCOUNT_LITERAL_FORMAT: &str = "AccountId must use a canonical I105 literal";

impl AccountId {
    /// Construct a single-signature account identifier.
    #[inline]
    #[must_use]
    pub fn new(signatory: PublicKey) -> Self {
        Self {
            controller: AccountController::single(signatory),
        }
    }

    /// Construct a multisignature account identifier.
    #[inline]
    #[must_use]
    pub fn new_multisig(policy: MultisigPolicy) -> Self {
        Self {
            controller: AccountController::multisig(policy),
        }
    }

    /// Convenience alias for [`Self::new`].
    #[inline]
    #[must_use]
    pub fn of(signatory: PublicKey) -> Self {
        Self::new(signatory)
    }

    /// Borrow the controller governing this account.
    #[inline]
    #[must_use]
    pub fn controller(&self) -> &AccountController {
        &self.controller
    }

    /// Borrow this account identifier.
    #[inline]
    #[must_use]
    pub fn account(&self) -> &Self {
        self
    }

    /// Return the canonical subject identity for this account.
    #[inline]
    #[must_use]
    pub fn subject_id(&self) -> Self {
        self.clone()
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

    /// Construct the address payload used for canonical I105 encoding.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the account identifier cannot be encoded
    /// into an [`AccountAddress`] (for example, when the controller configuration lacks support).
    #[inline]
    pub fn to_account_address(&self) -> Result<AccountAddress, AccountAddressError> {
        AccountAddress::from_account_id(self)
    }

    /// Encode the account as an i105 string for the provided network prefix.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] if the account cannot be encoded or if i105
    /// conversion fails for the provided network prefix.
    #[inline]
    pub fn to_i105_for_discriminant(
        &self,
        network_prefix: u16,
    ) -> Result<String, AccountAddressError> {
        self.to_account_address()?
            .to_i105_for_discriminant(network_prefix)
    }

    /// Encode the account as canonical I105 using the configured chain discriminant.
    ///
    /// # Errors
    ///
    /// Returns [`AccountAddressError`] when address encoding fails.
    #[inline]
    pub fn canonical_i105(&self) -> Result<String, AccountAddressError> {
        let prefix = address::chain_discriminant();
        self.to_account_address()?.to_i105_for_discriminant(prefix)
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
    /// Canonical I105 literals are accepted.
    /// Legacy forms such as `<identifier>@<domain>`, canonical hex, dotted/non-canonical
    /// i105 literals, aliases, UAID, opaque account literals, and historical
    /// non-i105 envelopes are rejected.
    /// The returned canonical string always matches the canonical I105 representation.
    ///
    /// # Errors
    ///
    /// Propagates [`ParseError`] when the textual representation is invalid.
    pub fn parse_encoded(input: &str) -> Result<ParsedAccountId, ParseError> {
        let (account_id, source) = Self::parse_internal(input)?;
        let canonical = account_id
            .canonical_i105()
            .map_err(|err| ParseError::new(err.code_str()))?;
        Ok(ParsedAccountId {
            account_id,
            canonical,
            source,
        })
    }

    /// Canonicalise a textual identifier into the i105 form.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the provided input is invalid.
    pub fn canonicalize(input: &str) -> Result<String, ParseError> {
        Self::parse_encoded(input).map(|parsed| parsed.canonical)
    }

    fn parse_internal(input: &str) -> Result<(Self, AccountAddressSource), ParseError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT));
        }
        if trimmed.contains('@') {
            return Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT));
        }

        Self::parse_address_literal(trimmed)
    }

    fn parse_address_literal(input: &str) -> Result<(Self, AccountAddressSource), ParseError> {
        let expected_prefix = address::chain_discriminant();
        match AccountAddress::from_i105_for_discriminant(input, Some(expected_prefix)) {
            Ok(address) => {
                let canonical = address
                    .to_i105_for_discriminant(expected_prefix)
                    .map_err(|err| ParseError::new(err.code_str()))?;
                if canonical != input {
                    return Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT));
                }
                let controller = address
                    .to_account_controller()
                    .map_err(|err| ParseError::new(err.code_str()))?;
                Ok((Self { controller }, AccountAddressSource::Encoded))
            }
            Err(
                AccountAddressError::MissingI105Sentinel
                | AccountAddressError::I105TooShort
                | AccountAddressError::InvalidI105Char(_)
                | AccountAddressError::InvalidI105Base
                | AccountAddressError::InvalidI105Digit(_)
                | AccountAddressError::UnsupportedAddressFormat
                | AccountAddressError::InvalidLength
                | AccountAddressError::ChecksumMismatch,
            ) => {
                if matches!(
                    AccountAddress::from_i105_for_discriminant(input, Some(expected_prefix)),
                    Err(AccountAddressError::ChecksumMismatch)
                ) {
                    Err(ParseError::new(
                        AccountAddressErrorCode::ChecksumMismatch.as_str(),
                    ))
                } else {
                    Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT))
                }
            }
            Err(err) => Err(ParseError::new(err.code_str())),
        }
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let i105 = self.canonical_i105().map_err(|_| fmt::Error)?;
        f.write_str(&i105)
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
    /// Construct a registration builder for a canonical domainless account.
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

    /// Borrow the canonical account alias, if one is assigned.
    #[inline]
    #[must_use]
    pub fn label(&self) -> Option<&rekey::AccountAlias> {
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
    /// Create a registration builder for a canonical domainless account.
    #[must_use]
    pub fn new(id: AccountId) -> Self {
        Self {
            id,
            metadata: Metadata::default(),
            label: None,
            uaid: None,
            opaque_ids: Vec::new(),
        }
    }

    /// Replace metadata on this builder.
    #[must_use]
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Assign or replace the stable alias on this builder.
    #[must_use]
    pub fn with_label(mut self, label: Option<rekey::AccountAlias>) -> Self {
        self.label = label;
        self
    }

    /// Assign or clear the bound UAID on this builder.
    #[must_use]
    pub fn with_uaid(mut self, uaid: Option<UniversalAccountId>) -> Self {
        self.uaid = uaid;
        self
    }

    /// Replace the opaque identifier set on this builder.
    #[must_use]
    pub fn with_opaque_ids(mut self, opaque_ids: Vec<OpaqueAccountId>) -> Self {
        self.opaque_ids = opaque_ids;
        self
    }

    /// Remove the label assigned to this builder, if any.
    #[must_use]
    pub fn without_label(mut self) -> Self {
        self.label = None;
        self
    }

    /// Borrow the currently assigned alias on the builder.
    #[must_use]
    pub fn label(&self) -> Option<&rekey::AccountAlias> {
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

impl Registered for Account {
    type With = NewAccount;
}

impl Registrable for NewAccount {
    type Target = Account;

    fn build(self, _authority: &AccountId) -> Self::Target {
        Self::Target {
            id: self.id,
            metadata: self.metadata,
            label: self.label,
            uaid: self.uaid,
            opaque_ids: self.opaque_ids,
        }
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
            "Account{{id: {}, controller: {controller_desc}}}",
            self.id
        )
    }
}

impl fmt::Display for NewAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

#[cfg(test)]
mod account_id_parsing_tests {
    use std::sync::{LazyLock, Mutex, MutexGuard};

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::{core::decode_from_bytes, to_bytes};

    use super::*;
    use crate::DomainId;

    fn guard_chain_discriminant() -> MutexGuard<'static, ()> {
        static CHAIN_DISCRIMINANT_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        CHAIN_DISCRIMINANT_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn from_str_rejects_public_key_addresses() {
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("parse public key literal");
        let raw = format!("{public_key}@hbl.dataspace");

        let err = AccountId::parse_encoded(&raw)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("public_key@domain literals must be rejected");
        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn from_str_rejects_canonical_hex_addresses_without_domain() {
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let canonical = account.to_canonical_hex().expect("canonical hex encoding");
        let err = AccountId::parse_encoded(&canonical)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("canonical hex account literals must be rejected");
        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn encoded_literals_with_domain_are_rejected() {
        let _chain_guard = guard_chain_discriminant();
        let domain: DomainId = "fallback-domain".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0x5A; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("address encodes");
        let i105 = address
            .to_i105_for_discriminant(address::chain_discriminant())
            .expect("i105 encode");
        let canonical_hex = address.canonical_hex().expect("canonical hex encode");
        let domain_suffix = domain.to_string();

        for literal in [
            format!("{i105}@{domain_suffix}"),
            format!("{canonical_hex}@{domain_suffix}"),
        ] {
            let err = AccountId::parse_encoded(&literal)
                .expect_err("encoded literals with @domain suffix must be rejected");
            assert!(
                err.reason().contains("i105"),
                "unexpected error: {}",
                err.reason()
            );
        }
    }

    #[test]
    fn from_str_rejects_alias_literals() {
        let err = AccountId::parse_encoded("blue-alias@hbl.dataspace")
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("aliases must be rejected");
        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn from_str_rejects_i105_alphabet_alias() {
        let alias_label = "primary";
        let err = AccountAddress::parse_encoded(alias_label, Some(address::chain_discriminant()))
            .expect_err("alias label should not parse as a valid address");
        assert_eq!(err.code_str(), "ERR_MISSING_I105_SENTINEL");

        let err = AccountId::parse_encoded("primary@hbl.dataspace")
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("aliases must be rejected");
        assert!(
            err.reason().contains("canonical I105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn from_str_rejects_alias_domain_mismatch() {
        let err = AccountId::parse_encoded("blue-alias@otherland")
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("mismatched alias domain must fail");
        assert!(
            err.reason().contains("i105"),
            "unexpected error message: {}",
            err.reason()
        );
    }

    #[test]
    fn parse_reports_encoded_source() {
        let _guard = guard_chain_discriminant();
        let key_pair = KeyPair::from_seed(vec![0xCD; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        let i105 = address
            .to_i105_for_discriminant(address::chain_discriminant())
            .expect("i105 encode");
        let parsed = AccountId::parse_encoded(&i105).expect("i105 account id must parse");
        assert_eq!(parsed.source(), AccountAddressSource::Encoded);
        assert_eq!(parsed.canonical(), parsed.account_id().to_string());
    }

    #[test]
    fn parse_rejects_fullwidth_sentinel_i105_literal() {
        let _guard = guard_chain_discriminant();
        let key_pair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let canonical = account.to_string();
        let noncanonical = canonical.replacen("sora", "ｓｏｒａ", 1);

        let err = AccountId::parse_encoded(&noncanonical)
            .expect_err("fullwidth sentinel literal must be rejected");
        assert_eq!(err.reason(), ERR_ACCOUNT_LITERAL_FORMAT);
    }

    #[test]
    fn parse_rejects_public_key_source() {
        let _guard = guard_chain_discriminant();
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
        let raw = format!("{public_key}@hbl.dataspace");

        let err = AccountId::parse_encoded(&raw).expect_err("public key source must be rejected");
        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn encoded_literals_roundtrip_without_domain_context() {
        let _guard_chain = guard_chain_discriminant();
        let key_pair = KeyPair::from_seed(vec![0xEF; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        let i105 = address
            .to_i105_for_discriminant(address::chain_discriminant())
            .expect("i105 encode");

        let literal = i105;
        let parsed = AccountId::parse_encoded(&literal).expect("encoded literal should parse");
        assert_eq!(parsed.account_id(), &account);
        assert_eq!(parsed.canonical(), account.to_string());
    }

    #[test]
    fn norito_roundtrip_account_id() {
        let key_pair = KeyPair::from_seed(vec![0xEF; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());

        let framed = to_bytes(&account).expect("encode account id");
        let decoded = decode_from_bytes::<AccountId>(&framed).expect("decode account id");

        assert_eq!(decoded, account);
    }

    #[test]
    fn parse_rejects_alias_source() {
        struct Reset(u16);

        impl Drop for Reset {
            fn drop(&mut self) {
                address::set_chain_discriminant(self.0);
            }
        }

        let _chain_guard = guard_chain_discriminant();

        let previous_chain_discriminant = address::set_chain_discriminant(42);
        let _reset = Reset(previous_chain_discriminant);
        let err = AccountId::parse_encoded("blue-alias@hbl.dataspace")
            .expect_err("alias must be rejected");

        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn canonicalize_rejects_canonical_hex_input() {
        let _guard = guard_chain_discriminant();
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let literal = account
            .to_canonical_hex()
            .expect("canonical hex literal must be available");
        let err =
            AccountId::canonicalize(&literal).expect_err("canonical hex input must be rejected");
        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn from_str_rejects_mismatched_i105_discriminant() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(42);
        let key_pair = KeyPair::from_seed(vec![0xAA; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let payload =
            address::AccountAddress::from_account_id(&account).expect("address encoding succeeds");
        let literal = payload
            .to_i105_for_discriminant(41)
            .expect("encode i105 with foreign prefix");

        let err = AccountId::parse_encoded(&literal)
            .map(crate::account::ParsedAccountId::into_account_id)
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
    fn from_str_accepts_configured_i105_discriminant() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(7);
        let key_pair = KeyPair::from_seed(vec![0xBB; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let payload =
            address::AccountAddress::from_account_id(&account).expect("address encoding succeeds");
        let literal = payload
            .to_i105_for_discriminant(7)
            .expect("encode i105 with configured prefix");

        let parsed = AccountId::parse_encoded(&literal)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("matching prefix should parse");
        assert_eq!(parsed.signatory(), account.signatory());

        address::set_chain_discriminant(previous);
    }

    #[test]
    fn from_str_rejects_encoded_address_with_domain_suffix() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let payload =
            address::AccountAddress::from_account_id(&account).expect("address encoding succeeds");
        let literal = format!(
            "{}@{}",
            payload
                .to_i105_for_discriminant(address::chain_discriminant())
                .expect("encode i105"),
            domain
        );

        let err = AccountId::parse_encoded(&literal)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("encoded address with domain should be rejected");
        assert!(
            err.reason().contains("i105"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn display_uses_chain_discriminant_sentinel() {
        let _guard = guard_chain_discriminant();
        let previous = address::set_chain_discriminant(73);
        let key_pair = KeyPair::from_seed(vec![0xCC; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let rendered = account.to_string();
        let parsed =
            AccountAddress::parse_encoded(&rendered, None).expect("display should parse as i105");
        assert_eq!(
            parsed.to_account_id().expect("decode account id"),
            account,
            "rendered address should roundtrip to the same account"
        );
        address::set_chain_discriminant(previous);
    }
}

impl IntoKeyValue for Account {
    type Key = AccountId;
    type Value = AccountValue;
    fn into_key_value(self) -> (Self::Key, Self::Value) {
        let details = AccountDetails::new(self.metadata, self.label, self.uaid, self.opaque_ids);
        (self.id, Owned::new(details))
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{
        ACCOUNT_ADMISSION_POLICY_METADATA_KEY, Account, AccountAddress, AccountAddressSource,
        AccountAdmissionMode, AccountAdmissionPolicy, AccountAlias, AccountAliasDomain,
        AccountController, AccountDomainSelector, AccountEntry, AccountId, AccountRekeyRecord,
        AccountValue, MultisigMember, MultisigPolicy, NewAccount, OpaqueAccountId, ParsedAccountId,
    };
}

#[cfg(test)]
#[cfg(feature = "transparent_api")]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    use super::*;
    use crate::{name::Name, nexus::DataSpaceId};

    #[test]
    fn parse_account_id() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
        let literal = account_id.to_string();
        let parsed = AccountId::parse_encoded(&literal)
            .map(ParsedAccountId::into_account_id)
            .expect("should be valid");
        assert_eq!(parsed.controller(), account_id.controller());
        assert_eq!(parsed.signatory(), key_pair.public_key());

        let _err_empty_address =
            AccountId::parse_encoded("@domain").expect_err("@domain should not be valid");
        let _err_empty_domain = AccountId::parse_encoded(&format!("{literal}@"))
            .expect_err("address@ should not be valid");
        let _err_violates_format = AccountId::parse_encoded(&format!("{literal}#domain"))
            .expect_err("address#domain should not be valid");
    }

    #[test]
    fn account_signatory_exposed() {
        let key_pair = KeyPair::random();
        let public_key = key_pair.public_key().clone();
        let account_id = AccountId::new(public_key.clone());
        let account = Account::new(account_id.clone()).build(&account_id);
        assert_eq!(account.signatory(), &public_key);
    }

    #[test]
    fn display_renders_i105_for_secp256k1() {
        let kp = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let account_id = AccountId::new(kp.public_key().clone());

        let rendered = account_id.to_string();
        let parsed = AccountId::parse_encoded(&rendered)
            .map(ParsedAccountId::into_account_id)
            .expect("rendered i105 must parse");
        assert_eq!(parsed.signatory(), account_id.signatory());
    }

    #[test]
    fn rekey_record_uses_account_alias() {
        let key_pair = KeyPair::random();
        let signatory = key_pair.public_key().clone();
        let account_id = AccountId::new(signatory.clone());
        let label = rekey::AccountAlias::new(
            "alice".parse::<Name>().expect("valid label"),
            Some(rekey::AccountAliasDomain::new(
                "wonderland".parse::<Name>().expect("alias domain"),
            )),
            DataSpaceId::GLOBAL,
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
        assert_eq!(record.active_account_id, account_id);
        assert_eq!(record.active_signatory, Some(signatory));
        assert!(record.previous_account_ids.is_empty());
        assert!(record.previous_signatories.is_empty());
    }

    #[test]
    fn rekey_record_absent_without_label() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
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
        let members = vec![
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let account_id = AccountId::new_multisig(policy);
        assert!(account_id.try_signatory().is_none());

        let account = Account {
            id: account_id.clone(),
            metadata: Metadata::default(),
            label: Some(rekey::AccountAlias::new(
                "vault".parse::<Name>().expect("label"),
                Some(rekey::AccountAliasDomain::new(
                    "wonderland".parse::<Name>().expect("alias domain"),
                )),
                DataSpaceId::GLOBAL,
            )),
            uaid: None,
            opaque_ids: Vec::new(),
        };
        assert!(account.try_signatory().is_none());
        let record = rekey::AccountRekeyRecord::from_account(&account).expect("record");
        assert_eq!(record.active_account_id, account_id);
        assert!(record.active_signatory.is_none());
        assert!(record.previous_account_ids.is_empty());
        assert!(record.previous_signatories.is_empty());
    }

    #[test]
    fn multisig_account_id_roundtrip() {
        let members = vec![
            MultisigMember::new(KeyPair::random().public_key().clone(), 1).expect("member"),
            MultisigMember::new(KeyPair::random().public_key().clone(), 2).expect("member"),
        ];
        let policy = MultisigPolicy::new(2, members).expect("policy");
        let account_id = AccountId::new_multisig(policy.clone());
        let literal = account_id
            .canonical_i105()
            .expect("i105 encoding should succeed");
        let parsed = AccountId::parse_encoded(&literal)
            .map(ParsedAccountId::into_account_id)
            .expect("should parse multisig");
        let parsed_policy = parsed
            .multisig_policy()
            .expect("multisig policy should be present");
        assert_eq!(parsed_policy, &policy);
        assert!(parsed.try_signatory().is_none());
    }

    #[test]
    fn account_subject_id_is_domainless() {
        let key_pair = KeyPair::random();
        let account = AccountId::new(key_pair.public_key().clone());

        assert_eq!(account.subject_id(), account);
    }

    #[test]
    fn account_accessor_returns_self() {
        let key_pair = KeyPair::random();
        let account = AccountId::new(key_pair.public_key().clone());

        assert_eq!(account.account(), &account);
    }

    #[test]
    fn i105_checksum_failure_reports_error_code() {
        // Negative vector from fixtures/account/address_vectors.json (`i105-checksum-mismatch`).
        let literal = "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSア";
        let err = AccountId::parse_encoded(literal).expect_err("invalid i105 payload must fail");
        assert_eq!(
            err.reason(),
            AccountAddressErrorCode::ChecksumMismatch.as_str()
        );
    }

    #[test]
    fn account_builder_carries_uaid_into_details() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());
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

    #[test]
    fn domainless_account_builder_roundtrips_without_domain_state() {
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(key_pair.public_key().clone());

        let account = Account::new(account_id.clone()).build(&account_id);
        assert_eq!(account.id, account_id);

        let new_account = NewAccount::new(account_id.clone());
        assert_eq!(new_account.to_string(), account_id.to_string());
        assert_eq!(new_account.build(&account_id).id, account_id);
    }
}

#[cfg(all(test, feature = "json"))]
mod json_tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use norito::codec::{decode_adaptive, encode_adaptive};

    use super::*;
    use crate::{
        account::address,
        metadata::Metadata,
        name::Name,
        nexus::{DataSpaceId, UniversalAccountId},
        prelude::Register,
    };

    fn guard_chain_discriminant() -> address::ChainDiscriminantGuard {
        address::ChainDiscriminantGuard::enter(address::chain_discriminant())
    }

    #[test]
    fn account_json_roundtrip() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
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
    fn account_id_json_uses_canonical_i105_literal() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());

        let json = norito::json::to_json(&id).expect("serialize account id");
        let i105 = id.canonical_i105().expect("i105 encoding");
        let expected = format!("\"{i105}\"");
        assert_eq!(json, expected);

        let decoded: AccountId = norito::json::from_json(&json).expect("deserialize account id");
        assert_eq!(decoded.controller(), id.controller());
    }

    #[test]
    fn account_id_json_roundtrips_large_multisig_as_canonical_i105() {
        let _guard = guard_chain_discriminant();
        let member_count = (u8::MAX as usize) + 1;
        let mut members = Vec::with_capacity(member_count);
        for idx in 0..member_count {
            let mut seed = vec![0_u8; 32];
            seed[..8].copy_from_slice(&(idx as u64).to_le_bytes());
            let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
            let member = MultisigMember::new(keypair.public_key().clone(), 1).expect("member");
            members.push(member);
        }
        let policy = MultisigPolicy::new(1, members).expect("policy");
        let id = AccountId::new_multisig(policy);

        let json = norito::json::to_json(&id).expect("serialize large multisig account id");
        let i105 = id.canonical_i105().expect("i105 encoding");
        assert_eq!(json, format!("\"{i105}\""));

        let decoded: AccountId =
            norito::json::from_json(&json).expect("deserialize large multisig account id");
        assert_eq!(decoded, id);
    }

    #[test]
    fn account_id_json_rejects_legacy_norito_literal() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let payload_hex = hex::encode(id.encode());
        let legacy = format!("\"norito:{payload_hex}\"");

        let err = norito::json::from_json::<AccountId>(&legacy)
            .expect_err("legacy norito account literal must fail");
        let msg = err.to_string();
        assert!(msg.contains("i105"), "unexpected error: {msg}");
    }

    #[test]
    fn new_account_json_roundtrip_defaults() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let new_account = NewAccount::new(id.clone());

        let json = norito::json::to_json(&new_account).expect("serialize new account");
        let decoded: NewAccount = norito::json::from_json(&json).expect("deserialize new account");

        assert_eq!(decoded, new_account);
        assert!(decoded.label.is_none());
        assert!(decoded.uaid.is_none());
        assert_eq!(decoded.metadata, Metadata::default());
        let removed_field = concat!("linked", "_domains");
        assert!(
            !json.contains(removed_field),
            "domainless registration payloads must not serialize the removed domain-link field"
        );
    }

    #[test]
    fn new_domainless_account_json_roundtrip_defaults() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let new_account = NewAccount::new(id.clone());

        let json = norito::json::to_json(&new_account).expect("serialize new account");
        let decoded: NewAccount = norito::json::from_json(&json).expect("deserialize new account");

        assert_eq!(decoded, new_account);
        assert!(decoded.label.is_none());
        assert!(decoded.uaid.is_none());
        assert_eq!(decoded.metadata, Metadata::default());
    }

    #[test]
    fn new_account_json_roundtrip_with_alias_and_uaid() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert("title".parse().expect("key"), "queen");
        let label = rekey::AccountAlias::new(
            "alice".parse::<Name>().expect("label name"),
            Some(rekey::AccountAliasDomain::new(
                "wonderland".parse::<Name>().expect("alias domain"),
            )),
            DataSpaceId::GLOBAL,
        );
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
    fn new_account_json_allows_domainless_payload() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let i105 = id.canonical_i105().expect("i105 encoding");
        let payload = format!("{{\"id\":\"{i105}\"}}");

        let decoded =
            norito::json::from_json::<NewAccount>(&payload).expect("domainless account payload");
        assert_eq!(decoded.id, id);
        assert!(decoded.label.is_none());
        assert!(decoded.uaid.is_none());
        assert_eq!(decoded.metadata, Metadata::default());
    }

    #[test]
    fn new_account_norito_roundtrip_preserves_packed_self_delimiting_fields() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let mut metadata = Metadata::default();
        metadata.insert("title".parse().expect("metadata key"), "queen");
        let label = rekey::AccountAlias::new(
            "alice".parse::<Name>().expect("label"),
            Some(rekey::AccountAliasDomain::new(
                "wonderland".parse::<Name>().expect("alias domain"),
            )),
            DataSpaceId::GLOBAL,
        );
        let uaid = UniversalAccountId::from_hash(Hash::prehashed([0xAB; 32]));
        let opaque_id = OpaqueAccountId::from_hash(Hash::prehashed([0xCD; 32]));

        let new_account = NewAccount::new(id)
            .with_metadata(metadata)
            .with_label(Some(label))
            .with_uaid(Some(uaid))
            .with_opaque_ids(vec![opaque_id]);

        let bytes = encode_adaptive(&new_account);
        let decoded: NewAccount = decode_adaptive(&bytes).expect("decode new account");

        assert_eq!(decoded, new_account);
    }

    #[test]
    fn register_account_norito_roundtrip_matches_kagami_genesis_shape() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let register = Register::account(NewAccount::new(id));

        let bytes = encode_adaptive(&register);
        let decoded: Register<Account> = decode_adaptive(&bytes).expect("decode register account");

        assert_eq!(decoded, register);
    }

    #[test]
    fn new_account_json_rejects_unknown_fields() {
        let _guard = guard_chain_discriminant();
        let keypair = KeyPair::random();
        let id = AccountId::new(keypair.public_key().clone());
        let i105 = id.canonical_i105().expect("i105 encoding");
        let payload = format!("{{\"id\":\"{i105}\",\"metadata\":{{}},\"extra\":true}}");

        let err = norito::json::from_json::<NewAccount>(&payload).expect_err("unknown field");
        match err {
            norito::json::Error::UnknownField { field } => assert_eq!(field, "extra"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
