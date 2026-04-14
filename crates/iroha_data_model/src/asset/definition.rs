//! Asset definitions and builders.

use core::fmt;

use derive_more::Display;
use getset::{CopyGetters, Getters};
use iroha_crypto::Hash;
use iroha_data_model_derive::{IdEqOrdHash, RegistrableBuilder, model};
use iroha_primitives::numeric::{Numeric, NumericSpec};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::Value;

pub use self::model::*;
use super::{alias::AssetDefinitionAlias, id::AssetDefinitionId};
use crate::{
    HasMetadata, Identifiable, Registered, Registrable, account::prelude::*,
    isi::error::MintabilityError, metadata::Metadata, sorafs_uri::SorafsUri,
};

/// Maximum accepted asset human-name length.
pub const MAX_ASSET_NAME_LEN: usize = 128;
/// Maximum accepted asset description length.
pub const MAX_ASSET_DESCRIPTION_LEN: usize = 2048;

/// Validate human-facing asset name.
///
/// # Errors
/// Returns [`crate::error::ParseError`] when `name` is blank, too long, or contains
/// reserved alias separators (`#`/`@`).
pub fn validate_asset_name(name: &str) -> Result<(), crate::error::ParseError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(crate::error::ParseError::new(
            "asset name must not be blank",
        ));
    }
    if trimmed.len() > MAX_ASSET_NAME_LEN {
        return Err(crate::error::ParseError::new(
            "asset name exceeds maximum length",
        ));
    }
    if name.contains('#') || name.contains('@') {
        return Err(crate::error::ParseError::new(
            "asset name must not contain `#` or `@`",
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(crate::error::ParseError::new(
            "asset name must not contain control characters",
        ));
    }
    Ok(())
}

/// Validate optional human-facing description.
///
/// # Errors
/// Returns [`crate::error::ParseError`] when provided description is blank, too long,
/// or contains control characters.
pub fn validate_asset_description(
    description: Option<&str>,
) -> Result<(), crate::error::ParseError> {
    let Some(description) = description else {
        return Ok(());
    };
    if description.trim().is_empty() {
        return Err(crate::error::ParseError::new(
            "asset description must not be blank when provided",
        ));
    }
    if description.len() > MAX_ASSET_DESCRIPTION_LEN {
        return Err(crate::error::ParseError::new(
            "asset description exceeds maximum length",
        ));
    }
    if description.chars().any(char::is_control) {
        return Err(crate::error::ParseError::new(
            "asset description must not contain control characters",
        ));
    }
    Ok(())
}

/// Validate optional alias literal for an asset definition.
///
/// # Errors
/// Returns [`crate::error::ParseError`] when the alias does not match the asset name.
pub fn validate_asset_alias(
    alias: Option<&AssetDefinitionAlias>,
    expected_name: &str,
) -> Result<(), crate::error::ParseError> {
    let Some(alias) = alias else {
        return Ok(());
    };
    if alias.name_segment() != expected_name {
        return Err(crate::error::ParseError::new(
            "asset alias name segment must match asset name exactly",
        ));
    }
    Ok(())
}

#[model]
mod model {
    use super::*;

    /// Balance partition policy for transparent asset ownership buckets.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        Default,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[repr(u8)]
    pub enum AssetBalancePolicy {
        /// Keep balances in a global bucket shared across dataspaces.
        #[default]
        #[display("Global")]
        Global,
        /// Partition balances by the transaction dataspace context.
        #[display("DataspaceRestricted")]
        DataspaceRestricted,
    }

    /// Asset definition defines the type of that asset.
    #[derive(
        Debug,
        Display,
        Clone,
        IdEqOrdHash,
        CopyGetters,
        Getters,
        Decode,
        Encode,
        IntoSchema,
        RegistrableBuilder,
    )]
    #[display("{id} {spec}{mintable}")]
    #[allow(clippy::multiple_inherent_impl)]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize,
            crate::DeriveFastJson
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetDefinition {
        /// An Identification of the [`AssetDefinition`].
        pub id: AssetDefinitionId,
        /// Human-readable asset name shown in UX surfaces.
        #[getset(get = "pub")]
        #[registrable_builder(default = String::new())]
        pub name: String,
        /// Optional human-readable description.
        #[getset(get = "pub")]
        #[registrable_builder(default = None)]
        pub description: Option<String>,
        /// Optional alias literal (`<name>#<domain>.<dataspace>` or `<name>#<dataspace>`).
        #[getset(get = "pub")]
        #[registrable_builder(default = None)]
        pub alias: Option<AssetDefinitionAlias>,
        /// Numeric spec of this asset.
        #[getset(get_copy = "pub")]
        pub spec: NumericSpec,
        /// Is the asset mintable
        #[getset(get_copy = "pub")]
        #[registrable_builder(default = Mintable::default())]
        pub mintable: Mintable,
        /// `SoraFS` URI to the [`AssetDefinition`] logo.
        #[getset(get = "pub")]
        #[registrable_builder(default = None)]
        pub logo: Option<SorafsUri>,
        /// Metadata of this asset definition as a key-value store.
        #[registrable_builder(default = Metadata::default())]
        pub metadata: Metadata,
        /// Balance partition policy for concrete ownership buckets.
        #[getset(get_copy = "pub")]
        #[registrable_builder(default = AssetBalancePolicy::default())]
        #[norito(default)]
        pub balance_scope_policy: AssetBalancePolicy,
        /// The account that owns this asset. Usually the [`Account`] that registered it.
        #[getset(get = "pub")]
        #[registrable_builder(skip, init = authority.clone())]
        pub owned_by: AccountId,
        /// The total amount of this asset in existence (sum of all asset values).
        #[getset(get = "pub")]
        #[registrable_builder(skip, init = Numeric::zero())]
        pub total_quantity: Numeric,
        /// Confidential asset policy controlling shielded operations.
        #[getset(get = "pub")]
        #[registrable_builder(default = AssetConfidentialPolicy::default())]
        pub confidential_policy: AssetConfidentialPolicy,
    }

    /// An assets mintability scheme. `Infinitely` means elastic
    /// supply. `Once` is what you want to use. Don't use `Not` explicitly
    /// outside of smartcontracts.
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        Default,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[repr(u8)]
    pub enum Mintable {
        /// Regular asset with elastic supply. Can be minted and burned.
        #[default]
        #[display("+")]
        Infinitely,
        /// Non-mintable asset (token), with a fixed supply. Can be burned, and minted **once**.
        #[display("=")]
        Once,
        /// Non-mintable asset (token), with a fixed supply. Can be burned, but not minted.
        #[display("-")]
        Not,
        /// Asset may be minted a limited number of additional times.
        #[display("Limited({_0})")]
        Limited(MintabilityTokens),
    }

    /// Remaining mintability budget for limited assets.
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema, CopyGetters,
    )]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize,
            crate::DeriveFastJson
        ),
        norito(no_fast_from_json)
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[repr(transparent)]
    pub struct MintabilityTokens {
        #[getset(get_copy = "pub")]
        value: u32,
    }

    impl MintabilityTokens {
        /// Construct a new token budget if the value is non-zero.
        #[must_use]
        pub const fn new(value: u32) -> Option<Self> {
            if value == 0 {
                None
            } else {
                Some(Self { value })
            }
        }

        /// Attempt to construct a token budget, returning an error when the provided value is zero.
        ///
        /// # Errors
        /// Returns [`MintabilityError::InvalidMintabilityTokens`] when `value` is zero.
        pub fn try_new(value: u32) -> Result<Self, MintabilityError> {
            Self::new(value).ok_or(MintabilityError::InvalidMintabilityTokens(value))
        }

        /// Decrement the budget by one, returning the remaining value or `None` when it reaches zero.
        #[must_use]
        pub const fn decrement(self) -> Option<Self> {
            if self.value <= 1 {
                None
            } else {
                Some(Self {
                    value: self.value - 1,
                })
            }
        }
    }

    impl From<MintabilityTokens> for u32 {
        fn from(tokens: MintabilityTokens) -> Self {
            tokens.value
        }
    }

    impl fmt::Display for MintabilityTokens {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.value)
        }
    }

    impl Mintable {
        /// Create a limited mintability variant from a pre-validated token budget.
        #[must_use]
        pub const fn limited(tokens: MintabilityTokens) -> Self {
            Self::Limited(tokens)
        }

        /// Attempt to create a limited mintability variant from a raw token value.
        ///
        /// # Errors
        /// Returns [`MintabilityError::InvalidMintabilityTokens`] when `tokens` is zero.
        pub fn limited_from_u32(tokens: u32) -> Result<Self, MintabilityError> {
            MintabilityTokens::try_new(tokens).map(Self::Limited)
        }

        /// Remaining limited token budget, if applicable.
        #[must_use]
        pub const fn remaining_tokens(self) -> Option<MintabilityTokens> {
            match self {
                Self::Limited(tokens) => Some(tokens),
                _ => None,
            }
        }

        /// Consume one unit of mintability budget.
        ///
        /// # Errors
        /// Returns [`MintabilityError::MintUnmintable`] when minting is forbidden.
        pub fn consume_one(&mut self) -> Result<bool, MintabilityError> {
            match *self {
                Self::Infinitely => Ok(false),
                Self::Not => Err(MintabilityError::MintUnmintable),
                Self::Once => {
                    *self = Self::Not;
                    Ok(true)
                }
                Self::Limited(tokens) => {
                    if let Some(next) = tokens.decrement() {
                        *self = Self::Limited(next);
                        Ok(false)
                    } else {
                        *self = Self::Not;
                        Ok(true)
                    }
                }
            }
        }
    }

    /// Operating mode for confidential asset flows.
    #[derive(
        Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[repr(u8)]
    pub enum ConfidentialPolicyMode {
        /// Asset behaves transparently; shielded instructions are rejected.
        #[display("TransparentOnly")]
        TransparentOnly,
        /// All issuance and movement must occur through confidential instructions.
        #[display("ShieldedOnly")]
        ShieldedOnly,
        /// Asset may move between transparent and confidential representations.
        #[display("Convertible")]
        Convertible,
    }

    /// Pending transition to a new confidential policy mode.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        CopyGetters,
        Getters,
    )]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize,
            crate::DeriveFastJson
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct ConfidentialPolicyTransition {
        /// Identifier of the new mode to transition into.
        #[getset(get_copy = "pub")]
        pub new_mode: ConfidentialPolicyMode,
        /// Block height at which the transition becomes effective.
        #[getset(get_copy = "pub")]
        pub effective_height: u64,
        /// Policy mode active before this transition was scheduled.
        #[getset(get_copy = "pub")]
        pub previous_mode: ConfidentialPolicyMode,
        /// Transition identifier for replay protection and auditability.
        #[getset(get = "pub")]
        pub transition_id: Hash,
        /// Optional conversion window length (in blocks) prior to finalizing the transition.
        #[getset(get = "pub")]
        pub conversion_window: Option<u64>,
    }

    /// Configuration governing whether and how an asset uses confidential flows.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        IntoSchema,
        CopyGetters,
        Getters,
    )]
    #[cfg_attr(
        feature = "json",
        derive(
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize,
            crate::DeriveFastJson
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct AssetConfidentialPolicy {
        /// Current mode for shielded versus transparent handling.
        #[getset(get_copy = "pub")]
        pub mode: ConfidentialPolicyMode,
        /// Digest of the confidential feature set this asset expects.
        #[getset(get = "pub")]
        pub vk_set_hash: Option<Hash>,
        /// Poseidon parameter set identifier expected for proofs referencing this asset.
        #[getset(get_copy = "pub")]
        pub poseidon_params_id: Option<u32>,
        /// Pedersen parameter set identifier expected for commitments associated with this asset.
        #[getset(get_copy = "pub")]
        pub pedersen_params_id: Option<u32>,
        /// Pending transition to a new policy mode, if scheduled.
        #[getset(get = "pub")]
        pub pending_transition: Option<ConfidentialPolicyTransition>,
    }
}

impl AssetDefinition {
    /// Construct builder for [`AssetDefinition`] identifiable by [`AssetDefinitionId`].
    ///
    /// Callers must provide a human-facing name with [`NewAssetDefinition::with_name`]
    /// before registration.
    #[must_use]
    #[inline]
    pub fn new(id: AssetDefinitionId, spec: NumericSpec) -> <Self as Registered>::With {
        <Self as Registered>::With::new(id, spec)
    }

    /// Construct builder for [`AssetDefinition`] identifiable by [`AssetDefinitionId`].
    ///
    /// Callers must provide a human-facing name with [`NewAssetDefinition::with_name`]
    /// before registration.
    #[must_use]
    #[inline]
    pub fn numeric(id: AssetDefinitionId) -> <Self as Registered>::With {
        <Self as Registered>::With::new(id, NumericSpec::default())
    }

    /// Mutable access to asset definition metadata for in-place updates.
    pub fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }

    /// Consume one unit of the limited mintability budget.
    ///
    /// # Errors
    /// Returns [`MintabilityError::MintUnmintable`] when the definition cannot be minted.
    pub fn consume_mintability(&mut self) -> Result<bool, MintabilityError> {
        self.mintable.consume_one()
    }

    /// Set mintability mode.
    pub fn set_mintable(&mut self, mintable: Mintable) {
        self.mintable = mintable;
    }

    /// Set the owner of this asset definition.
    pub fn set_owned_by(&mut self, owner: AccountId) {
        self.owned_by = owner;
    }

    /// Set the confidential policy configuration.
    pub fn set_confidential_policy(&mut self, policy: AssetConfidentialPolicy) {
        self.confidential_policy = policy;
    }
}

impl NewAssetDefinition {
    /// Set mintability to [`Mintable::Once`]
    #[inline]
    #[must_use]
    pub fn mintable_once(mut self) -> Self {
        self.mintable = Mintable::Once;
        self
    }

    /// Set mintability to [`Mintable::Limited`] with a pre-validated token budget.
    #[inline]
    #[must_use]
    pub fn mintable_limited(mut self, tokens: MintabilityTokens) -> Self {
        self.mintable = Mintable::limited(tokens);
        self
    }

    /// Try to set mintability to [`Mintable::Limited`] using a raw token value.
    ///
    /// Returns an error when the provided value is zero.
    ///
    /// # Errors
    /// Returns [`MintabilityError::InvalidMintabilityTokens`] when `tokens` is zero.
    pub fn try_mintable_limited(mut self, tokens: u32) -> Result<Self, MintabilityError> {
        self.mintable = Mintable::limited_from_u32(tokens)?;
        Ok(self)
    }

    /// Set a custom confidential policy.
    #[inline]
    #[must_use]
    pub fn confidential_policy(mut self, policy: AssetConfidentialPolicy) -> Self {
        self.confidential_policy = policy;
        self
    }
}

impl Default for AssetConfidentialPolicy {
    fn default() -> Self {
        Self {
            mode: ConfidentialPolicyMode::TransparentOnly,
            vk_set_hash: None,
            poseidon_params_id: None,
            pedersen_params_id: None,
            pending_transition: None,
        }
    }
}

impl AssetConfidentialPolicy {
    /// Create a transparent-only policy.
    #[must_use]
    pub fn transparent() -> Self {
        Self::default()
    }

    /// Create a shielded-only policy without pending transitions.
    #[must_use]
    pub fn shielded_only() -> Self {
        Self {
            mode: ConfidentialPolicyMode::ShieldedOnly,
            ..Self::default()
        }
    }

    /// Create a convertible policy without pending transitions.
    #[must_use]
    pub fn convertible() -> Self {
        Self {
            mode: ConfidentialPolicyMode::Convertible,
            ..Self::default()
        }
    }

    /// Compute a digest summarizing the confidential feature expectations.
    #[must_use]
    pub fn features_digest(&self) -> Hash {
        let mut buf = Vec::with_capacity(Hash::LENGTH + 8 + 8);
        if let Some(hash) = &self.vk_set_hash {
            buf.extend_from_slice(hash.as_ref());
        } else {
            buf.extend_from_slice(&[0u8; Hash::LENGTH]);
        }
        buf.extend_from_slice(&self.poseidon_params_id.unwrap_or_default().to_le_bytes());
        buf.extend_from_slice(&self.pedersen_params_id.unwrap_or_default().to_le_bytes());
        Hash::new(&buf)
    }

    /// Determine the policy mode that should be in effect at `block_height`.
    /// Returns the pending transition's mode once the effective height is reached.
    #[must_use]
    pub fn effective_mode(&self, block_height: u64) -> ConfidentialPolicyMode {
        if let Some(transition) = self.pending_transition.as_ref() {
            if transition.new_mode == ConfidentialPolicyMode::ShieldedOnly
                && let Some(window) = transition.conversion_window()
            {
                let window_open = transition.effective_height().saturating_sub(*window);
                if block_height >= window_open && block_height < transition.effective_height() {
                    return ConfidentialPolicyMode::Convertible;
                }
            }
            if block_height >= transition.effective_height() {
                return transition.new_mode();
            }
        }
        self.mode
    }

    /// Apply the pending transition when it is due, returning the updated policy and
    /// whether a change occurred.
    #[must_use]
    pub fn apply_if_due(mut self, block_height: u64) -> (Self, bool) {
        if let Some(transition) = self.pending_transition {
            if transition.new_mode == ConfidentialPolicyMode::ShieldedOnly
                && let Some(window) = transition.conversion_window
            {
                let window_open = transition.effective_height.saturating_sub(window);
                if block_height >= window_open
                    && block_height < transition.effective_height
                    && self.mode != ConfidentialPolicyMode::Convertible
                {
                    self.mode = ConfidentialPolicyMode::Convertible;
                    return (self, true);
                }
            }
            if block_height >= transition.effective_height {
                self.mode = transition.new_mode;
                self.pending_transition = None;
                return (self, true);
            }
        }
        (self, false)
    }
}

impl HasMetadata for AssetDefinition {
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Mintable {
    fn write_json(&self, out: &mut String) {
        match self {
            Mintable::Infinitely => norito::json::write_json_string("Infinitely", out),
            Mintable::Once => norito::json::write_json_string("Once", out),
            Mintable::Not => norito::json::write_json_string("Not", out),
            Mintable::Limited(tokens) => {
                let label = format!("Limited({})", tokens.value());
                norito::json::write_json_string(&label, out);
            }
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ConfidentialPolicyMode {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            ConfidentialPolicyMode::TransparentOnly => "TransparentOnly",
            ConfidentialPolicyMode::ShieldedOnly => "ShieldedOnly",
            ConfidentialPolicyMode::Convertible => "Convertible",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for AssetBalancePolicy {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            AssetBalancePolicy::Global => "Global",
            AssetBalancePolicy::DataspaceRestricted => "DataspaceRestricted",
        };
        norito::json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ConfidentialPolicyMode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let label = parser.parse_string()?;
        match label.as_str() {
            "TransparentOnly" => Ok(ConfidentialPolicyMode::TransparentOnly),
            "ShieldedOnly" => Ok(ConfidentialPolicyMode::ShieldedOnly),
            "Convertible" => Ok(ConfidentialPolicyMode::Convertible),
            other => Err(norito::json::Error::InvalidField {
                field: String::from("ConfidentialPolicyMode"),
                message: format!("unknown variant '{other}'"),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for AssetBalancePolicy {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let label = parser.parse_string()?;
        match label.as_str() {
            "Global" => Ok(AssetBalancePolicy::Global),
            "DataspaceRestricted" => Ok(AssetBalancePolicy::DataspaceRestricted),
            other => Err(norito::json::Error::InvalidField {
                field: String::from("AssetBalancePolicy"),
                message: format!("unknown variant '{other}'"),
            }),
        }
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Mintable {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        parser.skip_ws();
        let next = match parser.peek() {
            Some(byte) => byte,
            None => {
                return Err(norito::json::Error::InvalidField {
                    field: String::from("Mintable"),
                    message: String::from("unexpected end of input"),
                });
            }
        };

        if next == b'"' {
            let label = parser.parse_string()?;
            return parse_mintable_label(label.as_str());
        }

        if next != b'{' {
            return Err(norito::json::Error::InvalidField {
                field: String::from("Mintable"),
                message: String::from("expected string variant or object"),
            });
        }

        let mut visitor = MapVisitor::new(parser)?;
        let mut kind: Option<String> = None;
        let mut tokens: Option<MintabilityTokens> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "kind" => {
                    if kind.is_some() {
                        return Err(norito::json::Error::duplicate_field("kind"));
                    }
                    kind = Some(visitor.parse_value::<String>()?);
                }
                "tokens" | "value" => {
                    if tokens.is_some() {
                        return Err(norito::json::Error::duplicate_field(key.as_str()));
                    }
                    let raw = visitor.parse_value::<Value>()?;
                    tokens = Some(parse_limited_tokens_value(raw)?);
                }
                _ => {
                    visitor.skip_value()?;
                }
            }
        }
        visitor.finish()?;

        let kind = kind.ok_or_else(|| norito::json::Error::missing_field("kind"))?;

        match kind.as_str() {
            "Infinitely" => Ok(Mintable::Infinitely),
            "Once" => Ok(Mintable::Once),
            "Not" => Ok(Mintable::Not),
            "Limited" => {
                let tokens = tokens.ok_or_else(|| {
                    norito::json::Error::missing_field("tokens or value for Limited mintable")
                })?;
                Ok(Mintable::Limited(tokens))
            }
            other => Err(norito::json::Error::InvalidField {
                field: String::from("Mintable"),
                message: format!("unknown variant '{other}'"),
            }),
        }
    }
}

#[cfg(feature = "json")]
fn parse_mintable_label(label: &str) -> Result<Mintable, norito::json::Error> {
    match label {
        "Infinitely" => Ok(Mintable::Infinitely),
        "Once" => Ok(Mintable::Once),
        "Not" => Ok(Mintable::Not),
        other if other.starts_with("Limited(") && other.ends_with(')') => {
            let inner = &other["Limited(".len()..other.len() - 1];
            let count = inner
                .parse::<u32>()
                .map_err(|_| norito::json::Error::InvalidField {
                    field: String::from("Mintable"),
                    message: format!("invalid Limited token count '{inner}'"),
                })?;
            Mintable::limited_from_u32(count).map_err(|err| norito::json::Error::InvalidField {
                field: String::from("Mintable"),
                message: err.to_string(),
            })
        }
        other => Err(norito::json::Error::InvalidField {
            field: String::from("Mintable"),
            message: format!("unknown variant '{other}'"),
        }),
    }
}

#[cfg(feature = "json")]
fn parse_limited_tokens_value(value: Value) -> Result<MintabilityTokens, norito::json::Error> {
    let raw = match value {
        Value::Number(number) => {
            number
                .as_u64()
                .ok_or_else(|| norito::json::Error::InvalidField {
                    field: String::from("Mintable"),
                    message: String::from("tokens number must be non-negative"),
                })?
        }
        Value::String(raw) => {
            raw.parse::<u64>()
                .map_err(|_| norito::json::Error::InvalidField {
                    field: String::from("Mintable"),
                    message: String::from("tokens string must parse as unsigned integer"),
                })?
        }
        other => {
            return Err(norito::json::Error::InvalidField {
                field: String::from("Mintable"),
                message: format!("tokens must be string or number, got {other:?}"),
            });
        }
    };

    let raw = u32::try_from(raw).map_err(|_| norito::json::Error::InvalidField {
        field: String::from("Mintable"),
        message: String::from("tokens exceed u32 range"),
    })?;

    MintabilityTokens::try_new(raw).map_err(|err| norito::json::Error::InvalidField {
        field: String::from("Mintable"),
        message: err.to_string(),
    })
}

impl HasMetadata for NewAssetDefinition {
    fn metadata(&self) -> &Metadata {
        &self.metadata
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    use crate::domain::DomainId;

    #[test]
    fn constructors_leave_name_empty_without_explicit_with_name() {
        let id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "rose".parse().expect("name"),
        );

        let numeric = AssetDefinition::numeric(id.clone());
        assert!(
            numeric.name.is_empty(),
            "numeric constructor must not auto-fill name"
        );

        let custom = AssetDefinition::new(id, NumericSpec::fractional(2));
        assert!(
            custom.name.is_empty(),
            "new constructor must not auto-fill name"
        );
    }

    #[test]
    fn asset_name_validation_rejects_blank_and_alias_separators() {
        assert!(validate_asset_name("   ").is_err());
        assert!(validate_asset_name("usd#x").is_err());
        assert!(validate_asset_name("usd@x").is_err());
    }

    #[test]
    fn asset_name_validation_accepts_simple_label() {
        validate_asset_name("USD Coin").expect("valid name");
    }

    #[test]
    fn asset_description_validation_rejects_blank_when_present() {
        assert!(validate_asset_description(Some("  ")).is_err());
        validate_asset_description(None).expect("none is valid");
    }

    #[test]
    fn asset_alias_validation_requires_name_segment_match() {
        let alias: AssetDefinitionAlias = "usd#issuer.main".parse().expect("alias");
        validate_asset_alias(Some(&alias), "usd").expect("matching name segment");
        assert!(validate_asset_alias(Some(&alias), "US Dollar").is_err());
    }
}

#[cfg(all(test, feature = "json"))]
mod json_tests {
    use std::str::FromStr;

    use iroha_crypto::Hash;
    use norito::json::{Arena, FastFromJson, TapeWalker};

    use super::*;
    use crate::{Name, domain::DomainId, metadata::Metadata};

    #[test]
    fn new_asset_definition_json_roundtrip_preserves_policy() {
        let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let id = AssetDefinitionId::new(domain, Name::from_str("rose").expect("asset name"));
        let mut metadata = Metadata::default();
        metadata.insert("unit".parse().expect("metadata key"), "bloom");
        let transition = ConfidentialPolicyTransition {
            new_mode: ConfidentialPolicyMode::ShieldedOnly,
            effective_height: 64,
            previous_mode: ConfidentialPolicyMode::Convertible,
            transition_id: Hash::prehashed([0x55; 32]),
            conversion_window: Some(5),
        };
        let policy = AssetConfidentialPolicy {
            mode: ConfidentialPolicyMode::Convertible,
            vk_set_hash: Some(Hash::prehashed([0x11; 32])),
            poseidon_params_id: Some(7),
            pedersen_params_id: Some(3),
            pending_transition: Some(transition),
        };
        let new_definition = NewAssetDefinition {
            id,
            name: "Rose".to_owned(),
            description: Some("Flower-backed settlement unit".to_owned()),
            alias: Some("Rose#issuer.main".parse().expect("asset alias")),
            spec: NumericSpec::fractional(4),
            mintable: Mintable::Limited(MintabilityTokens::try_new(5).expect("tokens")),
            logo: Some(
                "sorafs://bafybeigdyrztk/logo/rose.png"
                    .parse()
                    .expect("sorafs uri"),
            ),
            metadata: metadata.clone(),
            balance_scope_policy: AssetBalancePolicy::DataspaceRestricted,
            confidential_policy: policy,
        };

        let json =
            norito::json::to_json(&new_definition).expect("serialize asset definition builder");
        let decoded: NewAssetDefinition =
            norito::json::from_json(&json).expect("deserialize asset definition builder");

        assert_eq!(decoded.id, new_definition.id);
        assert_eq!(decoded.name, new_definition.name);
        assert_eq!(decoded.description, new_definition.description);
        assert_eq!(decoded.alias, new_definition.alias);
        assert_eq!(decoded.spec.scale(), new_definition.spec.scale());
        assert_eq!(decoded.mintable, new_definition.mintable);
        assert_eq!(decoded.logo, new_definition.logo);
        assert_eq!(decoded.metadata, metadata);
        assert_eq!(
            decoded.balance_scope_policy,
            new_definition.balance_scope_policy
        );
        assert_eq!(decoded.confidential_policy, policy);
    }

    #[test]
    fn new_asset_definition_fast_from_json_matches_value() {
        let domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let id = AssetDefinitionId::new(domain, Name::from_str("rose").expect("asset name"));
        let new_definition = NewAssetDefinition {
            id,
            name: "Rose".to_owned(),
            description: None,
            alias: None,
            spec: NumericSpec::fractional(2),
            mintable: Mintable::Once,
            logo: None,
            metadata: Metadata::default(),
            balance_scope_policy: AssetBalancePolicy::Global,
            confidential_policy: AssetConfidentialPolicy::transparent(),
        };

        let json = norito::json::to_json(&new_definition).expect("serialize asset definition");
        let mut walker = TapeWalker::new(&json);
        let mut arena = Arena::new();
        let parsed =
            <NewAssetDefinition as FastFromJson>::parse(&mut walker, &mut arena).expect("parse");
        assert_eq!(parsed, new_definition);
    }
}
