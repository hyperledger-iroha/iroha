use super::*;
use crate::nexus::{AssetPermissionManifest, DataSpaceId, UniversalAccountId};

isi! {
    /// Publish or replace a capability manifest in the Space Directory.
    pub struct PublishSpaceDirectoryManifest {
        /// Canonical manifest payload (UAID, dataspace, rules, lifecycle schedule).
        pub manifest: AssetPermissionManifest,
    }
}

impl crate::seal::Instruction for PublishSpaceDirectoryManifest {}

isi! {
    /// Revoke an existing Space Directory manifest for a UAID/dataspace pair.
    pub struct RevokeSpaceDirectoryManifest {
        /// UAID that owns the manifest.
        pub uaid: UniversalAccountId,
        /// Dataspace hosting the manifest.
        pub dataspace: DataSpaceId,
        /// Epoch (inclusive) when the revocation takes effect.
        pub revoked_epoch: u64,
        /// Optional audit reason recorded with the revocation.
        #[norito(default)]
        pub reason: Option<String>,
    }
}

impl crate::seal::Instruction for RevokeSpaceDirectoryManifest {}

isi! {
    /// Expire an existing Space Directory manifest for a UAID/dataspace pair.
    pub struct ExpireSpaceDirectoryManifest {
        /// UAID that owns the manifest.
        pub uaid: UniversalAccountId,
        /// Dataspace hosting the manifest.
        pub dataspace: DataSpaceId,
        /// Epoch (inclusive) when the manifest expired.
        pub expired_epoch: u64,
    }
}

impl crate::seal::Instruction for ExpireSpaceDirectoryManifest {}
