//! Resolver directory events exposed via the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;

    /// Events emitted by the resolver attestation directory governance lane.
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
    pub enum SoradnsDirectoryEvent {
        /// A resolver directory draft was submitted.
        DraftSubmitted(crate::soradns::DirectoryDraftSubmittedEventV1),
        /// A resolver directory record was published.
        Published(crate::soradns::DirectoryPublishedEventV1),
        /// A resolver was revoked via hotfix.
        Revoked(crate::soradns::DirectoryRevokedEventV1),
        /// A resolver revocation was lifted.
        Unrevoked(crate::soradns::DirectoryUnrevokedEventV1),
        /// A release signer was added to the allowlist.
        ReleaseSignerAdded(crate::soradns::DirectoryReleaseSignerEventV1),
        /// A release signer was removed from the allowlist.
        ReleaseSignerRemoved(crate::soradns::DirectoryReleaseSignerEventV1),
        /// The directory rotation policy changed.
        PolicyUpdated(crate::soradns::DirectoryPolicyUpdatedEventV1),
    }
}
