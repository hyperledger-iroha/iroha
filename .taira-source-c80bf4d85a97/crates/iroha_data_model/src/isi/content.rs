use super::*;
use crate::content::{ContentBundleId, ContentBundleManifest};

isi! {
    /// Publish a content bundle (hashed tar archive) into the on-chain content lane.
    pub struct PublishContentBundle {
        /// Expected bundle identifier (BLAKE3 of the tar archive).
        pub bundle_id: ContentBundleId,
        /// Raw tar archive bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        pub tarball: Vec<u8>,
        /// Optional block height after which the bundle expires.
        pub expires_at_height: Option<u64>,
        /// Optional manifest describing cache/auth/placement metadata.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(default))]
        #[cfg_attr(feature = "json", norito(skip_serializing_if = "Option::is_none"))]
        pub manifest: Option<ContentBundleManifest>,
    }
}

impl crate::seal::Instruction for PublishContentBundle {}

isi! {
    /// Retire a previously published content bundle.
    pub struct RetireContentBundle {
        /// Identifier of the bundle to retire.
        pub bundle_id: ContentBundleId,
    }
}

impl crate::seal::Instruction for RetireContentBundle {}
