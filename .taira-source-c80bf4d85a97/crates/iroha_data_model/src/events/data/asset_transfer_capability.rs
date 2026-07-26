//! Events emitted by exact bounded asset-transfer capabilities.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::asset_transfer_capability::AssetTransferCapabilityV1;

/// Native asset-transfer capability lifecycle events.
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
    IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "event", content = "payload"))]
pub enum AssetTransferCapabilityEventV1 {
    /// A source owner registered an exact authorization.
    Registered(AssetTransferCapabilityV1),
    /// The source owner revoked the remaining execution budget.
    Revoked(AssetTransferCapabilityV1),
    /// The fixed delegate atomically transferred funds and consumed one use.
    Executed(AssetTransferCapabilityV1),
}

impl AssetTransferCapabilityEventV1 {
    /// Return the post-transition capability record carried by this event.
    #[must_use]
    pub const fn capability(&self) -> &AssetTransferCapabilityV1 {
        match self {
            Self::Registered(capability)
            | Self::Revoked(capability)
            | Self::Executed(capability) => capability,
        }
    }
}

/// Prelude exports for native asset-transfer capability events.
pub mod prelude {
    pub use super::{AssetTransferCapabilityEventV1, AssetTransferCapabilityEventV1Set};
}
