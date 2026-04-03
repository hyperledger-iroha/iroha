//! This module contains [`Domain`](`crate::domain::Domain`) structure
//! and related implementations and trait implementations.
use std::{format, str::FromStr, string::String, vec::Vec};

use derive_more::Display;
use iroha_data_model_derive::{IdEqOrdHash, model};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::{
    HasMetadata, Identifiable, Name, Registered, Registrable, error::ParseError,
    metadata::Metadata, name, prelude::*, sorafs_uri::SorafsUri,
};

#[model]
mod model {
    use getset::Getters;

    use super::*;

    /// Identification of a [`Domain`].
    #[derive(
        Debug,
        Display,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{name}.{dataspace}")]
    #[getset(get = "pub")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct DomainId {
        /// Domain label unique only within its parent dataspace.
        pub name: Name,
        /// Dataspace alias that owns the domain namespace.
        pub dataspace: Name,
    }

    /// Named group of [`Account`] and [`Asset`](`crate::asset::value::Asset`) entities.
    #[derive(Debug, Display, Clone, IdEqOrdHash, Getters, Decode, Encode, IntoSchema)]
    #[allow(clippy::multiple_inherent_impl)]
    #[display("[{id}]")]
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
    pub struct Domain {
        /// Identification of this [`Domain`].
        pub id: DomainId,
        /// `SoraFS` URI to the [`Domain`] logo.
        #[getset(get = "pub")]
        pub logo: Option<SorafsUri>,
        /// [`Metadata`] of this `Domain` as a key-value store.
        pub metadata: Metadata,
        /// The account that owns this domain. Usually the [`Account`] that registered it.
        #[getset(get = "pub")]
        pub owned_by: AccountId,
    }

    /// Builder which can be submitted in a transaction to create a new [`Domain`]
    #[derive(Debug, Display, Clone, IdEqOrdHash, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        norito(rename = "Domain"),
        derive(
            crate::DeriveJsonSerialize,
            crate::DeriveJsonDeserialize,
            crate::DeriveFastJson
        )
    )]
    #[cfg_attr(feature = "json", norito(no_fast_from_json))]
    #[display("[{id}]")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct NewDomain {
        /// The identification associated with the domain builder.
        pub id: DomainId,
        /// The (`SoraFS`) link to the logo of this domain.
        pub logo: Option<SorafsUri>,
        /// Metadata associated with the domain builder.
        pub metadata: Metadata,
    }
}

impl DomainId {
    fn parse_label(raw: &str) -> Result<Name, ParseError> {
        let canonical = name::canonicalize_domain_label(raw)?;
        Name::from_str(&canonical)
    }

    fn from_canonical_parts(name: Name, dataspace: Name) -> Self {
        Self { name, dataspace }
    }

    /// Build a dataspace-qualified domain identifier from explicit parts.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when either segment is invalid under the domain-label
    /// canonicalisation rules.
    pub fn try_new(name: impl AsRef<str>, dataspace: impl AsRef<str>) -> Result<Self, ParseError> {
        Ok(Self::from_canonical_parts(
            Self::parse_label(name.as_ref())?,
            Self::parse_label(dataspace.as_ref())?,
        ))
    }

    /// Parse a fully qualified `domain.dataspace` literal.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the literal is empty, contains surrounding whitespace,
    /// or does not use the exact `domain.dataspace` format.
    pub fn parse_fully_qualified(candidate: &str) -> Result<Self, ParseError> {
        if candidate.trim().is_empty() {
            return Err(ParseError::new("domain id must not be empty"));
        }
        if candidate.trim() != candidate {
            return Err(ParseError::new(
                "domain id must not contain leading or trailing whitespace",
            ));
        }

        let dot_count = candidate.bytes().filter(|byte| *byte == b'.').count();
        if dot_count != 1 {
            return Err(ParseError::new(
                "domain id must use `domain.dataspace` format",
            ));
        }

        let (name, dataspace) = candidate
            .split_once('.')
            .expect("validated domain literal must contain exactly one dot");
        if name.is_empty() || dataspace.is_empty() {
            return Err(ParseError::new("domain id segments must not be empty"));
        }

        Self::try_new(name, dataspace)
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for DomainId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.to_string(), out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for DomainId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        Self::parse_fully_qualified(&value)
            .map_err(|err| norito::json::Error::Message(err.reason().into()))
    }
}

impl HasMetadata for NewDomain {
    #[inline]
    fn metadata(&self) -> &crate::metadata::Metadata {
        &self.metadata
    }
}

impl NewDomain {
    /// Create a [`NewDomain`], reserved for internal use.
    #[must_use]
    fn new(id: DomainId) -> Self {
        Self {
            id,
            logo: None,
            metadata: Metadata::default(),
        }
    }

    /// Add [`logo`](SorafsUri) to the domain replacing previously defined value.
    #[must_use]
    pub fn with_logo(mut self, logo: SorafsUri) -> Self {
        self.logo = Some(logo);
        self
    }

    /// Add [`Metadata`] to the domain replacing previously defined value
    #[must_use]
    pub fn with_metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = metadata;
        self
    }
}

impl HasMetadata for Domain {
    #[inline]
    fn metadata(&self) -> &crate::metadata::Metadata {
        &self.metadata
    }
}

impl Registered for Domain {
    type With = NewDomain;
}

impl Registrable for NewDomain {
    type Target = Domain;

    #[inline]
    fn build(self, authority: &AccountId) -> Self::Target {
        Self::Target {
            id: self.id,
            metadata: self.metadata,
            logo: self.logo,
            owned_by: authority.clone(),
        }
    }
}

impl Domain {
    /// Construct builder for [`Domain`] identifiable by [`DomainId`].
    #[inline]
    pub fn new(id: DomainId) -> <Self as Registered>::With {
        <Self as Registered>::With::new(id)
    }

    /// Mutable access to domain metadata for in-place updates.
    pub fn metadata_mut(&mut self) -> &mut Metadata {
        &mut self.metadata
    }

    /// Set the domain owner.
    pub fn set_owned_by(&mut self, owner: AccountId) {
        self.owned_by = owner;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::dsl::{HasProjection, PredicateMarker, SelectorMarker};

    fn assert_predicate<T: HasProjection<PredicateMarker>>() {}
    fn assert_selector<T: HasProjection<SelectorMarker>>() {}

    #[test]
    fn domain_has_projection_impls() {
        assert_predicate::<Domain>();
        assert_selector::<Domain>();
    }

    #[test]
    fn domain_id_try_new_canonicalizes_both_segments() {
        let domain_id = DomainId::try_new("Treasury", "CentralBank").expect("domain id");
        assert_eq!(domain_id.to_string(), "treasury.centralbank");
    }

    #[test]
    fn domain_id_parse_fully_qualified_requires_both_segments() {
        let domain_id = DomainId::parse_fully_qualified("treasury.centralbank").expect("domain id");
        assert_eq!(domain_id.to_string(), "treasury.centralbank");
        assert!(DomainId::parse_fully_qualified("treasury").is_err());
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{Domain, DomainId};
}
