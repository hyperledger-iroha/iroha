use super::*;
use crate::{
    ipfs::IpfsPath,
    soradns::{
        DirectoryId, DirectoryRotationPolicyV1, RadRevokeReason, ResolverDirectoryRecordV1,
        ResolverId,
    },
};

isi! {
    /// Submit a resolver directory draft signed by an approved release engineer.
    pub struct SubmitDirectoryDraft {
        /// Resolver directory record that will be anchored on-chain upon publish.
        pub record: ResolverDirectoryRecordV1,
        /// CID of the CAR bundle containing directory.json, RADs, and Merkle proofs.
        pub car_cid: IpfsPath,
        /// SHA-256 digest of the canonical directory.json artifact.
        pub directory_json_sha256: [u8; 32],
        /// Builder (release engineer) public key used to sign this draft.
        pub builder_public_key: iroha_crypto::PublicKey,
        /// Builder signature over the canonical record payload + directory digest.
        pub builder_signature: iroha_crypto::Signature,
    }
}

impl crate::seal::Instruction for SubmitDirectoryDraft {}

isi! {
    /// Publish a resolver directory draft after council approval.
    pub struct PublishDirectory {
        /// Directory identifier (Merkle root) that identifies the draft.
        pub directory_id: DirectoryId,
        /// Optional guard ensuring the previous directory matches caller expectations.
        pub expected_prev: Option<DirectoryId>,
    }
}

impl crate::seal::Instruction for PublishDirectory {}

isi! {
    /// Revoke a resolver immediately without publishing a new directory record.
    pub struct RevokeResolver {
        /// Resolver identifier targeted by the revocation.
        pub resolver_id: ResolverId,
        /// Governance-stamped revocation reason.
        pub reason: RadRevokeReason,
    }
}

impl crate::seal::Instruction for RevokeResolver {}

isi! {
    /// Remove an entry from the resolver revocation set.
    pub struct UnrevokeResolver {
        /// Resolver identifier that will be restored.
        pub resolver_id: ResolverId,
    }
}

impl crate::seal::Instruction for UnrevokeResolver {}

isi! {
    /// Add a release engineer key authorized to submit directory drafts.
    pub struct AddReleaseSigner {
        /// Public key that will be permitted to submit drafts.
        pub public_key: iroha_crypto::PublicKey,
    }
}

impl crate::seal::Instruction for AddReleaseSigner {}

isi! {
    /// Remove a release engineer key from the draft submission allowlist.
    pub struct RemoveReleaseSigner {
        /// Public key that will be removed from the allowlist.
        pub public_key: iroha_crypto::PublicKey,
    }
}

impl crate::seal::Instruction for RemoveReleaseSigner {}

isi! {
    /// Update the resolver directory rotation policy enforced during publish.
    pub struct SetDirectoryRotationPolicy {
        /// Updated rotation policy.
        pub policy: DirectoryRotationPolicyV1,
    }
}

impl crate::seal::Instruction for SetDirectoryRotationPolicy {}
