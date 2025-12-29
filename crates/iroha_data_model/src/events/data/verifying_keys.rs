//! Verifying key registry lifecycle events (ZK) for the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Verifying key registry lifecycle events.
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
    pub enum VerifyingKeyEvent {
        /// A verifying key record was registered.
        Registered(VerifyingKeyRegistered),
        /// A verifying key record was updated (new version).
        Updated(VerifyingKeyUpdated),
        /// A verifying key record was deprecated (status flipped).
        Deprecated(VerifyingKeyDeprecated),
    }

    /// Payload for a verifying key registration event.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct VerifyingKeyRegistered {
        /// Identifier of the verifying key (backend + name).
        pub id: crate::proof::VerifyingKeyId,
        /// Full record stored in the registry.
        pub record: crate::proof::VerifyingKeyRecord,
    }

    /// Payload for a verifying key update event.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct VerifyingKeyUpdated {
        /// Identifier of the verifying key (backend + name).
        pub id: crate::proof::VerifyingKeyId,
        /// New record stored in the registry.
        pub record: crate::proof::VerifyingKeyRecord,
    }

    /// Payload for a verifying key deprecation event.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct VerifyingKeyDeprecated {
        /// Identifier of the verifying key (backend + name).
        pub id: crate::proof::VerifyingKeyId,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    VerifyingKeyEvent,
    VerifyingKeyRegistered,
    VerifyingKeyUpdated,
    VerifyingKeyDeprecated,
);

/// Prelude exports for verifying key events
pub mod prelude {
    pub use super::{
        VerifyingKeyDeprecated, VerifyingKeyEvent, VerifyingKeyRegistered, VerifyingKeyUpdated,
    };
}

// Convenience constructors for common event-set presets
impl VerifyingKeyEventSet {
    /// A set that matches only `Registered` events.
    pub const fn only_registered() -> Self {
        Self::Registered
    }
    /// A set that matches only `Updated` events.
    pub const fn only_updated() -> Self {
        Self::Updated
    }
    /// A set that matches only `Deprecated` events.
    pub const fn only_deprecated() -> Self {
        Self::Deprecated
    }
    /// A set that matches `Updated` and `Deprecated` events.
    pub const fn updates_and_deprecations() -> Self {
        Self::Updated.or(Self::Deprecated)
    }
}
