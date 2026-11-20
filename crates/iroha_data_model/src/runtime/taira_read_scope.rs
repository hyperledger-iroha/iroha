//! Four-validator certificate for the one scoped-read permission activation on Taira.
//! Core authenticates the deployment, current selection and exact change set.

use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_model_base::chain::ChainId;

use super::{
    definition::{DefinitionDescriptorV1, DefinitionId, SelectionId},
    lifecycle::{ChangeSetV1, RootScopeV1, RuntimeNoticeV1, SelectionTransitionV1},
};
use crate::NetworkId;

/// Selection produced by release R's certified Taira profile cutover.
pub const KEYED_PREDECESSOR: SelectionId = SelectionId([
    0xfd, 0xda, 0x49, 0xf6, 0x1f, 0x74, 0xf9, 0xfe, 0x50, 0x34, 0x4a, 0x95, 0x2c, 0xcf, 0xe7, 0xc9,
    0x65, 0xbe, 0xaa, 0x44, 0x6c, 0x9c, 0x72, 0x7a, 0xb2, 0xb, 0x34, 0x9a, 0x5, 0xca, 0x6b, 0x29,
]);
/// Exact compiled permission published by this certificate.
pub const SCOPED_PERMISSION: DefinitionId = DefinitionId([
    0x58, 0x92, 0x2c, 0x8b, 0xa9, 0x77, 0xae, 0x78, 0x6c, 0x44, 0x0b, 0xb3, 0xdf, 0x20, 0xdf, 0x04,
    0xc6, 0x6f, 0xed, 0x1e, 0x94, 0xf8, 0xbb, 0x97, 0x09, 0x31, 0x56, 0x00, 0x64, 0xcb, 0x50, 0x6d,
]);
/// Reachable selection after publishing and activating only the scoped-read permission on R.
pub const SCOPED_SUCCESSOR: SelectionId = SelectionId([
    0x10, 0x8d, 0x5c, 0xfd, 0x34, 0x15, 0x0b, 0xd0, 0x50, 0x29, 0x11, 0x56, 0x44, 0x29, 0xfb, 0xd9,
    0x42, 0xf8, 0x04, 0x8d, 0xb6, 0x30, 0x59, 0xfe, 0x61, 0xa2, 0xe2, 0x4f, 0x89, 0xe2, 0xb1, 0xb5,
]);

/// Sole supported certificate version.
pub const TAIRA_READ_SCOPE_VERSION_V1: u16 = 1;
/// Exact number of scheduled validators that must independently authorize each step.
pub const TAIRA_READ_SCOPE_COMMITTEE_SIZE_V1: usize = 4;
/// Maximum canonical plan or certificate bytes, below the signed transaction size limit.
pub const TAIRA_READ_SCOPE_MAX_CERTIFICATE_BYTES_V1: usize = 8 * 1024 * 1024;
/// Domain separator for all four signatures over the canonical plan.
pub const TAIRA_READ_SCOPE_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:runtime:taira-read-scope:signature:v1\0";
/// Domain separator for the content-addressed plan identity.
pub const TAIRA_READ_SCOPE_ID_DOMAIN_V1: &[u8] = b"iroha:runtime:taira-read-scope:plan:v1\0";

/// Exact, independently reviewable authorization for the scoped-read change set.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::runtime::taira_read_scope::TairaReadScopePlanV1")]
#[norito(deny_unknown_fields)]
pub struct TairaReadScopePlanV1 {
    /// Fixed protocol version, [`TAIRA_READ_SCOPE_VERSION_V1`].
    pub version: u16,
    /// Exact deployment chain label.
    pub chain_id: ChainId,
    /// Signed-genesis-derived network identity.
    pub network_id: NetworkId,
    /// Nonzero, operator-chosen identity of this single approved operation.
    pub operation_id: [u8; 32],
    /// Exact publication and activation of the scoped-read permission, including qualification evidence.
    pub change_set: ChangeSetV1,
    /// Notice frozen into the scheduled record, in place of a committed upgrade policy.
    pub notice: RuntimeNoticeV1,
    /// All four scheduled validator keys, in the exact frozen roster order.
    #[norito(json = "crate::runtime::sorafs_repair::four_items_json")]
    pub committee: [PublicKey; TAIRA_READ_SCOPE_COMMITTEE_SIZE_V1],
    /// Last block timestamp, in Unix milliseconds, that may schedule this plan.
    pub expires_at_unix_ms: u64,
    /// Digest of the explicit, bounded owner authorization being exercised.
    pub authorization_digest: [u8; 32],
    /// Digest of the reviewed release artifact evidence.
    pub release_digest: [u8; 32],
    /// Digest of the reviewed immutable definition source closure evidence.
    pub source_closure_digest: [u8; 32],
}

/// Four independent signatures over the complete plan, one per ordered roster slot.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    norito::codec::Encode,
    norito::codec::Decode,
    iroha_schema::IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::runtime::taira_read_scope::TairaReadScopeCertificateV1")]
#[norito(deny_unknown_fields)]
pub struct TairaReadScopeCertificateV1 {
    /// Every context, authority and evidence field authorized by the signers.
    pub plan: TairaReadScopePlanV1,
    /// Signature at position `i` must verify with `plan.committee[i]`; no quorum subset suffices.
    #[norito(json = "crate::runtime::sorafs_repair::four_items_json")]
    pub signatures: [Signature; TAIRA_READ_SCOPE_COMMITTEE_SIZE_V1],
}

/// Stable State-independent reasons to reject a read-scope plan or certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TairaReadScopeErrorV1 {
    /// Required fields, root, network, notice or roster invariants are invalid.
    #[error("invalid Taira read-scope shape: {0}")]
    Shape(&'static str),
    /// The canonical frame exceeds the fixed certificate bound.
    #[error("Taira read-scope plan exceeds its canonical byte bound")]
    TooLarge,
    /// A frame cannot be encoded or is not an exact canonical encoding.
    #[error("invalid Taira read-scope canonical encoding")]
    Encoding,
    /// One of the four exact roster signatures does not verify.
    #[error("invalid Taira read-scope signature at roster index {0}")]
    BadSignature(usize),
}

impl TairaReadScopePlanV1 {
    /// Validate all State-independent invariants and the encoded size bound.
    ///
    /// # Errors
    /// Rejects missing commitments, an inconsistent change set, a zero notice or repeated keys.
    /// The roster, exact catalog equality, expiry and once-only scheduling belong to Core.
    pub fn validate(&self) -> Result<(), TairaReadScopeErrorV1> {
        if self.version != TAIRA_READ_SCOPE_VERSION_V1 {
            return Err(TairaReadScopeErrorV1::Shape("unsupported version"));
        }
        if self.change_set.root != RootScopeV1::Universal
            || self.change_set.network != self.network_id
            || self.change_set.successor != SCOPED_SUCCESSOR
        {
            return Err(TairaReadScopeErrorV1::Shape(
                "inconsistent successor selection",
            ));
        }
        let [definition @ DefinitionDescriptorV1::Permission(permission)] =
            self.change_set.publish.as_slice()
        else {
            return Err(TairaReadScopeErrorV1::Shape(
                "the change set must publish one permission",
            ));
        };
        let [SelectionTransitionV1::Activate(activated)] = self.change_set.transitions.as_slice()
        else {
            return Err(TairaReadScopeErrorV1::Shape(
                "the change set must only activate that permission",
            ));
        };
        if self.change_set.predecessor != KEYED_PREDECESSOR
            || *activated != SCOPED_PERMISSION
            || permission.name != "CanReadStateScope"
            || definition.id().ok().as_ref() != Some(activated)
        {
            return Err(TairaReadScopeErrorV1::Shape(
                "incorrect predecessor or permission",
            ));
        }
        // Scheduling must land before the block whose start activates the change set.
        if self.notice.blocks == 0 || self.change_set.effective_height < 2 {
            return Err(TairaReadScopeErrorV1::Shape(
                "activation requires a later block start",
            ));
        }
        if self.expires_at_unix_ms == 0
            || [
                self.operation_id,
                self.change_set.qualification,
                self.authorization_digest,
                self.release_digest,
                self.source_closure_digest,
            ]
            .contains(&[0; 32])
        {
            return Err(TairaReadScopeErrorV1::Shape("missing commitment or expiry"));
        }
        for (index, key) in self.committee.iter().enumerate() {
            if self.committee[..index].contains(key) {
                return Err(TairaReadScopeErrorV1::Shape("repeated committee key"));
            }
        }
        check_size(self)
    }

    /// Canonical unsigned plan, bounded and validated before allocation of its frame.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, TairaReadScopeErrorV1> {
        self.validate()?;
        norito::encode_canonical(self).map_err(|_| TairaReadScopeErrorV1::Encoding)
    }

    /// Decode an exact canonical, bounded and structurally valid unsigned plan.
    ///
    /// # Errors
    /// Rejects excess size, malformed, noncanonical, trailing or structurally invalid data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TairaReadScopeErrorV1> {
        check_input_size(bytes)?;
        let plan: Self =
            norito::decode_canonical(bytes).map_err(|_| TairaReadScopeErrorV1::Encoding)?;
        if plan.to_bytes()? != bytes {
            return Err(TairaReadScopeErrorV1::Encoding);
        }
        Ok(plan)
    }

    /// Domain-separated canonical bytes every roster member must sign verbatim.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, TairaReadScopeErrorV1> {
        let bytes = self.to_bytes()?;
        let mut payload =
            Vec::with_capacity(TAIRA_READ_SCOPE_SIGNATURE_DOMAIN_V1.len() + bytes.len());
        payload.extend_from_slice(TAIRA_READ_SCOPE_SIGNATURE_DOMAIN_V1);
        payload.extend_from_slice(&bytes);
        Ok(payload)
    }

    /// Domain-separated identity of every field of the exact canonical plan.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn id(&self) -> Result<[u8; 32], TairaReadScopeErrorV1> {
        Ok(Hash::new_from_chunks(&[TAIRA_READ_SCOPE_ID_DOMAIN_V1, &self.to_bytes()?]).into())
    }
}

impl TairaReadScopeCertificateV1 {
    /// Verify the bounded plan and every signature in the exact four-seat order.
    ///
    /// # Errors
    /// Rejects invalid shape, excess size or any missing, swapped, repeated or invalid signature.
    /// This does not replace Core's authentication of the roster and selection.
    pub fn verify(&self) -> Result<(), TairaReadScopeErrorV1> {
        check_size(self)?;
        let bytes = self.plan.signing_bytes()?;
        for (index, (signature, key)) in
            self.signatures.iter().zip(&self.plan.committee).enumerate()
        {
            signature
                .verify(key, &bytes)
                .map_err(|_| TairaReadScopeErrorV1::BadSignature(index))?;
        }
        Ok(())
    }

    /// Canonical bounded certificate after all four signatures have been verified.
    ///
    /// # Errors
    /// Rejects an invalid certificate or an encoding failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, TairaReadScopeErrorV1> {
        self.verify()?;
        norito::encode_canonical(self).map_err(|_| TairaReadScopeErrorV1::Encoding)
    }

    /// Decode an exact canonical, bounded and fully signed certificate.
    ///
    /// # Errors
    /// Rejects excess size, malformed, noncanonical, trailing or incorrectly signed data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TairaReadScopeErrorV1> {
        check_input_size(bytes)?;
        let certificate: Self =
            norito::decode_canonical(bytes).map_err(|_| TairaReadScopeErrorV1::Encoding)?;
        if certificate.to_bytes()? != bytes {
            return Err(TairaReadScopeErrorV1::Encoding);
        }
        Ok(certificate)
    }
}

fn check_size(value: &impl norito::core::NoritoSerialize) -> Result<(), TairaReadScopeErrorV1> {
    if norito::canonical_frame_len(value).map_err(|_| TairaReadScopeErrorV1::Encoding)?
        > TAIRA_READ_SCOPE_MAX_CERTIFICATE_BYTES_V1
    {
        return Err(TairaReadScopeErrorV1::TooLarge);
    }
    Ok(())
}

fn check_input_size(bytes: &[u8]) -> Result<(), TairaReadScopeErrorV1> {
    if bytes.len() > TAIRA_READ_SCOPE_MAX_CERTIFICATE_BYTES_V1 {
        return Err(TairaReadScopeErrorV1::TooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
