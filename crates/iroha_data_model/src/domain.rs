//! This module contains [`Domain`](`crate::domain::Domain`) structure
//! and related implementations and trait implementations.
pub use self::model::*;

use crate::{
    DeriveFastJson as DeriveFast, DeriveJsonDeserialize as DeriveJsonDe,
    DeriveJsonSerialize as DeriveJsonSer,
};
use crate::{
    HasMetadata, Identifiable, Registered, Registrable, prelude::*, sorafs_uri::SorafsUri,
};
use derive_more::Display;
use iroha_data_model_derive::{IdEqOrdHash, model};
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[model]
mod model {
    use super::*;
    use getset::Getters;
    /// Named group of [`Account`] and [`Asset`](`crate::asset::value::Asset`) entities.
    #[derive(Debug, Display, Clone, IdEqOrdHash, Getters, Decode, Encode, IntoSchema)]
    #[allow(clippy::multiple_inherent_impl)]
    #[display("[{id}]")]
    #[derive(DeriveJsonSer, DeriveJsonDe, DeriveFast)]
    #[norito(no_fast_from_json)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::domain::model::Domain")]
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
    #[derive(
        Debug,
        Display,
        Clone,
        IdEqOrdHash,
        Decode,
        Encode,
        IntoSchema,
        DeriveJsonSer,
        DeriveJsonDe,
        DeriveFast,
    )]
    #[norito(no_fast_from_json)]
    #[display("[{id}]")]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::domain::model::NewDomain")]
    pub struct NewDomain {
        /// The identification associated with the domain builder.
        pub id: DomainId,
        /// The (`SoraFS`) link to the logo of this domain.
        pub logo: Option<SorafsUri>,
        /// Metadata associated with the domain builder.
        pub metadata: Metadata,
    }
}
impl HasMetadata for NewDomain {
    #[inline]
    fn metadata(&self) -> &iroha_model_base::metadata::Metadata {
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
    fn metadata(&self) -> &iroha_model_base::metadata::Metadata {
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
}
/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::Domain;
}
