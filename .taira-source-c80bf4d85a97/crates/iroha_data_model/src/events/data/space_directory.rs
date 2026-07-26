//! Space Directory manifest lifecycle events.

use iroha_crypto::Hash;
use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Events emitted when capability manifests change state.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        iroha_data_model_derive::EventSet,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum SpaceDirectoryEvent {
        /// A manifest was activated for a UAID/dataspace pair.
        ManifestActivated(SpaceDirectoryManifestActivated),
        /// A manifest naturally expired and is no longer enforced.
        ManifestExpired(SpaceDirectoryManifestExpired),
        /// A manifest was revoked ahead of expiry (deny-wins audit).
        ManifestRevoked(SpaceDirectoryManifestRevoked),
    }

    /// Payload describing an activation event.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SpaceDirectoryManifestActivated {
        /// Dataspace hosting the manifest.
        pub dataspace: crate::nexus::DataSpaceId,
        /// UAID that owns the manifest.
        pub uaid: crate::nexus::UniversalAccountId,
        /// Canonical hash of the manifest payload.
        pub manifest_hash: Hash,
        /// Epoch (inclusive) when the manifest becomes active.
        pub activation_epoch: u64,
        /// Optional expiry epoch (inclusive).
        #[norito(default)]
        pub expiry_epoch: Option<u64>,
    }

    /// Payload describing a natural expiry event.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SpaceDirectoryManifestExpired {
        /// Dataspace hosting the manifest.
        pub dataspace: crate::nexus::DataSpaceId,
        /// UAID associated with the manifest.
        pub uaid: crate::nexus::UniversalAccountId,
        /// Canonical hash for the expired manifest.
        pub manifest_hash: Hash,
        /// Epoch when the manifest expired.
        pub expired_epoch: u64,
    }

    /// Payload describing a manifest revocation.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub struct SpaceDirectoryManifestRevoked {
        /// Dataspace hosting the manifest.
        pub dataspace: crate::nexus::DataSpaceId,
        /// UAID associated with the manifest.
        pub uaid: crate::nexus::UniversalAccountId,
        /// Canonical hash for the revoked manifest.
        pub manifest_hash: Hash,
        /// Epoch when the revocation took effect.
        pub revoked_epoch: u64,
        /// Optional audit note describing the revocation reason.
        #[norito(default)]
        pub reason: Option<String>,
    }
}

/// Common re-exports for the Space Directory event module.
pub mod prelude {
    pub use super::{
        SpaceDirectoryEvent, SpaceDirectoryManifestActivated, SpaceDirectoryManifestExpired,
        SpaceDirectoryManifestRevoked,
    };
}
