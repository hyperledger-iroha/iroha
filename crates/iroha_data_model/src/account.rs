//! Structures, traits and impls related to `Account`s.
use core::fmt;
use std::{format, io::Write, str::FromStr, string::String, vec::Vec};

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
    common::{Owned, Ref},
    domain::prelude::*,
    error::ParseError,
    metadata::Metadata,
    name::Name,
    nexus::UniversalAccountId,
};

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
    /// let literal = id.to_string();
    /// let parsed = AccountId::parse_encoded(&literal)
    ///     .map(|parsed| parsed.into_account_id())
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

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AccountId {
    fn write_json(&self, out: &mut String) {
        let literal = self.canonical_ih58().map_or_else(
            |_| {
                let payload = self.encode();
                let encoded = hex::encode(payload);
                format!("norito:{encoded}")
            },
            std::convert::identity,
        );
        norito::json::JsonSerialize::json_serialize(&literal, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AccountId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        let norito_prefix = "norito:";
        let norito_rest = value
            .get(..norito_prefix.len())
            .filter(|head| head.eq_ignore_ascii_case(norito_prefix))
            .map(|_| &value[norito_prefix.len()..]);
        if let Some(rest) = norito_rest {
            if rest.contains('@') {
                return Err(norito::json::Error::Message(
                    "invalid norito AccountId hex payload: domain suffix is not supported"
                        .to_string(),
                ));
            }
            let payload = hex::decode(rest).map_err(|err| {
                norito::json::Error::Message(format!("invalid norito AccountId hex payload: {err}"))
            })?;
            let mut cursor = std::io::Cursor::new(payload);
            let decoded = <AccountId as Decode>::decode(&mut cursor)
                .map_err(|err| norito::json::Error::Message(err.to_string()))?;
            return Ok(decoded);
        }

        AccountId::parse_encoded(&value)
            .map(ParsedAccountId::into_account_id)
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

/// Source that produced an [`AccountId`] when parsing textual account identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountAddressSource {
    /// The identifier was supplied using one of the encoded address formats.
    Encoded(AccountAddressFormat),
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

/// Domain-agnostic account identity keyed only by authorization controller.
///
/// This subject identity can be linked to one or more domains by materializing
/// scoped [`AccountId`] values with [`Self::to_account_id`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct AccountSubjectId {
    controller: AccountController,
}

impl AccountSubjectId {
    /// Construct a subject identifier from a controller.
    #[must_use]
    pub fn new(controller: AccountController) -> Self {
        Self { controller }
    }

    /// Derive a subject identifier from a scoped [`AccountId`].
    #[must_use]
    pub fn from_account_id(account_id: &AccountId) -> Self {
        Self::new(account_id.controller().clone())
    }

    /// Borrow the controller backing this subject identity.
    #[must_use]
    pub fn controller(&self) -> &AccountController {
        &self.controller
    }

    /// Materialize a scoped [`AccountId`] for this subject within `domain`.
    #[must_use]
    pub fn to_account_id(&self, domain: DomainId) -> AccountId {
        match &self.controller {
            AccountController::Single(key) => AccountId::new(domain, key.clone()),
            AccountController::Multisig(policy) => AccountId::new_multisig(domain, policy.clone()),
        }
    }
}

impl From<&AccountId> for AccountSubjectId {
    fn from(account_id: &AccountId) -> Self {
        Self::from_account_id(account_id)
    }
}

impl From<AccountId> for AccountSubjectId {
    fn from(account_id: AccountId) -> Self {
        Self::new(account_id.controller().clone())
    }
}

impl From<&AccountSubjectId> for AccountSubjectId {
    fn from(subject: &AccountSubjectId) -> Self {
        subject.clone()
    }
}

impl From<AccountController> for AccountSubjectId {
    fn from(controller: AccountController) -> Self {
        Self::new(controller)
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
        norito::core::NoritoDeserialize::try_deserialize(archived_tuple)
            .map(|(domain, controller)| Self { domain, controller })
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AccountId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((domain, controller), used) =
            norito::core::decode_field_canonical::<(DomainId, AccountController)>(bytes)?;
        Ok((Self { domain, controller }, used))
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

const ERR_ACCOUNT_LITERAL_FORMAT: &str =
    "AccountId must be an IH58 (preferred) or sora compressed literal";

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

    /// Return the domainless subject identity for this scoped account.
    #[inline]
    #[must_use]
    pub fn subject_id(&self) -> AccountSubjectId {
        AccountSubjectId::from_account_id(self)
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

    /// Construct the address payload used for IH58 (preferred)/sora (second-best) encoders.
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
    /// Only IH58 and sora compressed literals are accepted.
    /// Legacy forms such as `<identifier>@<domain>`, canonical hex, aliases, UAID,
    /// and opaque account literals are rejected.
    /// The returned canonical string always matches the canonical IH58 representation.
    ///
    /// # Errors
    ///
    /// Propagates [`ParseError`] when the textual representation is invalid.
    pub fn parse_encoded(input: &str) -> Result<ParsedAccountId, ParseError> {
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
        match AccountAddress::parse_encoded(input, Some(expected_prefix)) {
            Ok((
                address,
                format @ (AccountAddressFormat::IH58 { .. } | AccountAddressFormat::Compressed),
            )) => {
                let default_domain = address::default_domain_name();
                let domain = default_domain
                    .as_ref()
                    .parse()
                    .map_err(|_| ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT))?;
                let controller = address
                    .to_account_controller()
                    .map_err(|err| ParseError::new(err.code_str()))?;
                Ok((
                    Self { domain, controller },
                    AccountAddressSource::Encoded(format),
                ))
            }
            Ok((_, AccountAddressFormat::CanonicalHex)) => {
                Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT))
            }
            Err(AccountAddressError::UnsupportedAddressFormat) => {
                Err(ParseError::new(ERR_ACCOUNT_LITERAL_FORMAT))
            }
            Err(err) => Err(ParseError::new(err.code_str())),
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

#[cfg(test)]
mod account_id_parsing_tests {
    use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::{core::decode_from_bytes, to_bytes};

    use super::*;
    use crate::domain::DomainId;

    fn guard_chain_discriminant() -> MutexGuard<'static, ()> {
        static CHAIN_DISCRIMINANT_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        CHAIN_DISCRIMINANT_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn guard_default_domain_label() -> address::DefaultDomainGuard {
        address::default_domain_guard(None)
    }

    fn configured_default_domain_id() -> DomainId {
        address::default_domain_name()
            .as_ref()
            .parse()
            .expect("configured default domain must parse")
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
    fn from_str_rejects_public_key_addresses() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("parse public key literal");
        let raw = format!("{public_key}@{domain}");

        let err = AccountId::parse_encoded(&raw)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("public_key@domain literals must be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn from_str_rejects_canonical_hex_addresses_without_domain() {
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let canonical = account.to_canonical_hex().expect("canonical hex encoding");
        let err = AccountId::parse_encoded(&canonical)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("canonical hex account literals must be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn encoded_literals_with_domain_are_rejected() {
        let _chain_guard = guard_chain_discriminant();
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
            let err = AccountId::parse_encoded(&literal)
                .expect_err("encoded literals with @domain suffix must be rejected");
            assert!(
                err.reason().contains("IH58") || err.reason().contains("compressed"),
                "unexpected error: {}",
                err.reason()
            );
        }
    }

    #[test]
    fn from_str_rejects_alias_literals() {
        let err = AccountId::parse_encoded("blue-alias@wonderland")
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("aliases must be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn from_str_rejects_base58_like_alias() {
        let alias_label = "primary";
        let err = AccountAddress::parse_any(alias_label, Some(address::chain_discriminant()))
            .expect_err("alias label should not parse as a valid address");
        assert_eq!(err.code_str(), "ERR_CHECKSUM_MISMATCH");

        let err = AccountId::parse_encoded("primary@wonderland")
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("aliases must be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
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
            err.reason().contains("IH58") || err.reason().contains("compressed"),
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
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("ih58 encode");
        let parsed = AccountId::parse_encoded(&ih58).expect("ih58 account id must parse");
        assert_eq!(
            parsed.source(),
            AccountAddressSource::Encoded(AccountAddressFormat::IH58 {
                network_prefix: address::chain_discriminant(),
            })
        );
        assert_eq!(parsed.canonical(), parsed.account_id().to_string());

        let compressed = address.to_compressed_sora().expect("compressed encode");
        let parsed =
            AccountId::parse_encoded(&compressed).expect("compressed account id must parse");
        assert_eq!(
            parsed.source(),
            AccountAddressSource::Encoded(AccountAddressFormat::Compressed)
        );
        assert_eq!(parsed.canonical(), parsed.account_id().to_string());
    }

    #[test]
    fn parse_rejects_public_key_source() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
        let raw = format!("{public_key}@{domain}");

        let err = AccountId::parse_encoded(&raw).expect_err("public key source must be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
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
            let parsed = AccountId::parse_encoded(&literal)
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
    fn implicit_literals_resolve_with_configured_default_domain() {
        let _guard_chain = guard_chain_discriminant();
        let _guard_label = guard_default_domain_label();
        let _reset_label = reset_default_domain_to_default();
        address::set_default_domain_name("implicit-default-domain-test")
            .expect("configure test default domain label");

        let source_domain: DomainId = "treasury".parse().expect("valid domain");
        let expected_domain: DomainId = "implicit-default-domain-test"
            .parse()
            .expect("configured default domain parses");
        let key_pair = KeyPair::from_seed(vec![0xEE; 32], Algorithm::Ed25519);
        let account = AccountId::new(source_domain, key_pair.public_key().clone());
        let expected = AccountId::new(expected_domain, key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");

        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("IH58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");

        for literal in [ih58, compressed] {
            let parsed = AccountId::parse_encoded(&literal).expect("implicit literal should parse");
            assert_eq!(parsed.account_id(), &expected);
            assert_eq!(parsed.canonical(), expected.to_string());
        }
    }

    #[test]
    fn implicit_literals_survive_default_domain_changes() {
        let _guard_chain = guard_chain_discriminant();
        let _guard_label = guard_default_domain_label();
        let _reset_label = reset_default_domain_to_default();

        let source_domain: DomainId = "treasury".parse().expect("domain");
        let key_pair = KeyPair::from_seed(vec![0xED; 32], Algorithm::Ed25519);
        let source_account = AccountId::new(source_domain, key_pair.public_key().clone());
        let literal = AccountAddress::from_account_id(&source_account)
            .expect("account encodes")
            .to_compressed_sora()
            .expect("compressed encode");

        address::set_default_domain_name("wonderland")
            .expect("configure test default domain label");
        let parsed_wonderland = AccountId::parse_encoded(&literal)
            .expect("literal should parse under wonderland default");
        assert_eq!(
            parsed_wonderland.account_id(),
            &AccountId::new(
                "wonderland".parse().expect("domain"),
                key_pair.public_key().clone()
            )
        );

        address::set_default_domain_name("acme").expect("configure default domain label");
        let parsed_acme =
            AccountId::parse_encoded(&literal).expect("literal should parse under acme default");
        assert_eq!(
            parsed_acme.account_id(),
            &AccountId::new(
                "acme".parse().expect("domain"),
                key_pair.public_key().clone()
            )
        );
    }

    #[test]
    fn implicit_literals_bind_to_default_domain_without_selector_resolver() {
        let _guard_chain = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("valid domain");
        let key_pair = KeyPair::from_seed(vec![0xEF; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());
        let address = AccountAddress::from_account_id(&account).expect("account encodes");
        let ih58 = address
            .to_ih58(address::chain_discriminant())
            .expect("IH58 encode");
        let compressed = address.to_compressed_sora().expect("compressed encode");

        for literal in [ih58, compressed] {
            let parsed =
                AccountId::parse_encoded(&literal).expect("default-domain fallback should resolve");
            assert_eq!(parsed.account_id().signatory(), account.signatory());
            assert_eq!(
                parsed.account_id().domain(),
                &configured_default_domain_id()
            );
        }
    }

    #[test]
    fn norito_roundtrip_non_default_domain_without_resolver() {
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
        let err =
            AccountId::parse_encoded("blue-alias@wonderland").expect_err("alias must be rejected");

        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
    }

    #[test]
    fn canonicalize_rejects_canonical_hex_input() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("domain");
        let key_pair = KeyPair::from_seed(vec![0xBC; 32], Algorithm::Ed25519);
        let account = AccountId::new(domain, key_pair.public_key().clone());
        let literal = account
            .to_canonical_hex()
            .expect("canonical hex literal must be available");
        let err =
            AccountId::canonicalize(&literal).expect_err("canonical hex input must be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
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

        let parsed = AccountId::parse_encoded(&literal)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("matching prefix should parse");
        assert_eq!(parsed.signatory(), account.signatory());
        let default_domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("configured default domain");
        assert_eq!(parsed.domain(), &default_domain);

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

        let err = AccountId::parse_encoded(&literal)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect_err("encoded address with domain should be rejected");
        assert!(
            err.reason().contains("IH58") || err.reason().contains("compressed"),
            "unexpected error: {}",
            err.reason()
        );
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
        AccountAddressSource, AccountAdmissionMode, AccountAdmissionPolicy, AccountController,
        AccountDomainSelector, AccountEntry, AccountId, AccountLabel, AccountRekeyRecord,
        AccountSubjectId, AccountValue, MultisigMember, MultisigPolicy, NewAccount,
        OpaqueAccountId, ParsedAccountId,
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
        let _default_domain_guard = address::default_domain_guard(None);
        let key_pair = KeyPair::random();
        let domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("valid domain");
        let account_id = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let literal = account_id.to_string();
        let parsed = AccountId::parse_encoded(&literal)
            .map(ParsedAccountId::into_account_id)
            .expect("should be valid");
        assert_eq!(parsed.domain(), account_id.domain());
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
        let parsed = AccountId::parse_encoded(&rendered)
            .map(ParsedAccountId::into_account_id)
            .expect("rendered IH58 must parse");
        assert_eq!(parsed.signatory(), account_id.signatory());
        let default_domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("configured default domain");
        assert_eq!(parsed.domain(), &default_domain);
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
        let _default_domain_guard = address::default_domain_guard(None);
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
        let literal = account_id
            .canonical_ih58()
            .expect("ih58 encoding should succeed");
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
    fn canonical_string_uses_configured_default_domain() {
        let _default_domain_guard = address::default_domain_guard(None);
        address::set_default_domain_name("acme").expect("configure default domain label");
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let key_pair = KeyPair::random();
        let id = AccountId::new(domain, key_pair.public_key().clone());
        let encoded = id.canonical_ih58().expect("canonical ih58 encoding");
        let parsed = AccountId::parse_encoded(&encoded)
            .map(ParsedAccountId::into_account_id)
            .expect("canonical encoded parse");
        let expected = AccountId::new(
            "acme".parse().expect("default domain parses"),
            key_pair.public_key().clone(),
        );
        assert_eq!(parsed, expected);
    }

    #[test]
    fn account_subject_id_maps_one_subject_to_many_domains() {
        let key_pair = KeyPair::random();
        let wonderland: DomainId = "wonderland".parse().expect("domain id");
        let acme: DomainId = "acme".parse().expect("domain id");
        let scoped = AccountId::new(wonderland.clone(), key_pair.public_key().clone());
        let subject = scoped.subject_id();

        let wonderland_scoped = subject.to_account_id(wonderland.clone());
        let acme_scoped = subject.to_account_id(acme.clone());

        assert_eq!(wonderland_scoped.controller(), scoped.controller());
        assert_eq!(acme_scoped.controller(), scoped.controller());
        assert_eq!(wonderland_scoped.domain(), &wonderland);
        assert_eq!(acme_scoped.domain(), &acme);
        assert_eq!(AccountSubjectId::from(&scoped), subject);
    }

    #[test]
    fn ih58_checksum_failure_reports_error_code() {
        // Negative vector from fixtures/account/address_vectors.json (`ih58-checksum-mismatch`).
        let literal = "RnuaJGGDL8HNkN8bwHwBTU32fTWQmbRoM3QZBJintx5RqTU7GgPJmNiz";
        let err = AccountId::parse_encoded(literal).expect_err("invalid IH58 payload must fail");
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
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    use super::*;
    use crate::{
        account::address, domain::DomainId, metadata::Metadata, name::Name,
        nexus::UniversalAccountId,
    };

    fn guard_chain_discriminant() -> address::ChainDiscriminantGuard {
        address::ChainDiscriminantGuard::enter(address::chain_discriminant())
    }

    #[test]
    fn account_json_roundtrip() {
        let _guard = guard_chain_discriminant();
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
    fn account_id_json_uses_canonical_ih58_literal() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "acme".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain, keypair.public_key().clone());

        let json = norito::json::to_json(&id).expect("serialize account id");
        let ih58 = id.canonical_ih58().expect("ih58 encoding");
        let expected = format!("\"{ih58}\"");
        assert_eq!(json, expected);

        let decoded: AccountId = norito::json::from_json(&json).expect("deserialize account id");
        let default_domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("default domain");
        assert_eq!(decoded.controller(), id.controller());
        assert_eq!(decoded.domain(), &default_domain);
    }

    #[test]
    fn account_id_json_roundtrip_large_multisig_uses_norito_literal() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "weights".parse().expect("domain id");
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
        let id = AccountId::new_multisig(domain.clone(), policy);

        let json = norito::json::to_json(&id).expect("serialize account id");
        assert!(
            json.contains("norito:"),
            "large multisig account should use norito literal"
        );
        let decoded: AccountId = norito::json::from_json(&json).expect("deserialize account id");
        assert_eq!(decoded, id);
    }

    #[test]
    fn account_id_json_rejects_norito_literal_with_domain_suffix() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "weights".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain, keypair.public_key().clone());
        let payload_hex = hex::encode(id.encode());
        let legacy = format!("\"norito:{payload_hex}@wonderland\"");

        let err = norito::json::from_json::<AccountId>(&legacy)
            .expect_err("norito account literal with domain suffix must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("domain suffix is not supported"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn new_account_json_roundtrip_defaults() {
        let _guard = guard_chain_discriminant();
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
        let _guard = guard_chain_discriminant();
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
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain.clone(), keypair.public_key().clone());
        let ih58 = id.canonical_ih58().expect("ih58 encoding");
        let payload = format!("{{\"id\":\"{ih58}\"}}");

        let decoded: NewAccount =
            norito::json::from_json(&payload).expect("deserialize with defaults");

        assert_eq!(decoded.id.signatory(), id.signatory());
        let default_domain: DomainId = address::default_domain_name()
            .as_ref()
            .parse()
            .expect("configured default domain");
        assert_eq!(decoded.id.domain(), &default_domain);
        assert_eq!(decoded.metadata, Metadata::default());
        assert!(decoded.label.is_none());
        assert!(decoded.uaid.is_none());
    }

    #[test]
    fn new_account_json_rejects_unknown_fields() {
        let _guard = guard_chain_discriminant();
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let id = AccountId::new(domain, keypair.public_key().clone());
        let ih58 = id.canonical_ih58().expect("ih58 encoding");
        let payload = format!("{{\"id\":\"{ih58}\",\"metadata\":{{}},\"extra\":true}}");

        let err = norito::json::from_json::<NewAccount>(&payload).expect_err("unknown field");
        match err {
            norito::json::Error::UnknownField { field } => assert_eq!(field, "extra"),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
