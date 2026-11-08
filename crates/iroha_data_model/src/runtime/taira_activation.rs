//! Explicit, all-four-validator Time and Ironwood activation for one Taira selection.
//!
//! This certificate takes the place of Parliament's approval and of nothing else. Core compares
//! its whole change set with the exact compiled composition and records it `Scheduled`; every
//! validator still attests, and the change set still activates at the start of its effective
//! height (`specs/runtime/taira_time_ironwood_v1.md`). The Ironwood pool in the plan is bootstrapped
//! once, by a second transaction, after activation. Verifying the signatures here establishes only
//! the signed plan.

use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_model_base::chain::ChainId;

use super::lifecycle::{ChangeSetV1, RootScopeV1, RuntimeNoticeV1};
use crate::{NetworkId, privacy::PrivacyIronwoodPoolBootstrapV1};

/// Sole supported certificate version.
pub const TAIRA_ACTIVATION_VERSION_V1: u16 = 1;
/// Exact number of scheduled validators that must independently authorize the activation.
pub const TAIRA_ACTIVATION_COMMITTEE_SIZE_V1: usize = 4;
/// Maximum canonical plan or certificate bytes, below the signed transaction size limit.
pub const TAIRA_ACTIVATION_MAX_CERTIFICATE_BYTES_V1: usize = 8 * 1024 * 1024;
/// Domain separator for all four signatures over the canonical plan.
pub const TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1: &[u8] =
    b"iroha:runtime:taira-time-ironwood:signature:v1\0";
/// Domain separator for the content-addressed plan identity.
pub const TAIRA_ACTIVATION_ID_DOMAIN_V1: &[u8] = b"iroha:runtime:taira-time-ironwood:plan:v1\0";

/// Exact, independently reviewable authorization for one Time and Ironwood selection change.
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
#[norito_schema(name = "iroha_data_model::runtime::taira_activation::TairaActivationPlanV1")]
#[norito(deny_unknown_fields)]
pub struct TairaActivationPlanV1 {
    /// Fixed protocol version, [`TAIRA_ACTIVATION_VERSION_V1`].
    pub version: u16,
    /// Exact deployment chain label.
    pub chain_id: ChainId,
    /// Signed-genesis-derived network identity.
    pub network_id: NetworkId,
    /// Nonzero, operator-chosen identity of this single approved operation.
    pub operation_id: [u8; 32],
    /// Exact successor publication and selection, including qualification evidence.
    pub change_set: ChangeSetV1,
    /// Notice frozen into the scheduled record, in place of a committed upgrade policy.
    pub notice: RuntimeNoticeV1,
    /// All four scheduled validator keys, in the exact frozen roster order.
    #[norito(json = "crate::runtime::sorafs_repair::four_items_json")]
    pub committee: [PublicKey; TAIRA_ACTIVATION_COMMITTEE_SIZE_V1],
    /// Last block timestamp, in Unix milliseconds, that may schedule this plan.
    pub expires_at_unix_ms: u64,
    /// Digest of the explicit, bounded owner authorization being exercised.
    pub authorization_digest: [u8; 32],
    /// Digest of the reviewed release artifact evidence.
    pub release_digest: [u8; 32],
    /// Digest of the reviewed immutable definition source closure evidence.
    pub source_closure_digest: [u8; 32],
    /// Exact Ironwood pool bootstrapped once after the successor activates.
    pub pool: PrivacyIronwoodPoolBootstrapV1,
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
#[norito_schema(name = "iroha_data_model::runtime::taira_activation::TairaActivationCertificateV1")]
#[norito(deny_unknown_fields)]
pub struct TairaActivationCertificateV1 {
    /// Every context, authority, evidence and pool field authorized by the signers.
    pub plan: TairaActivationPlanV1,
    /// Signature at position `i` must verify with `plan.committee[i]`; no quorum subset suffices.
    #[norito(json = "crate::runtime::sorafs_repair::four_items_json")]
    pub signatures: [Signature; TAIRA_ACTIVATION_COMMITTEE_SIZE_V1],
}

/// Stable State-independent reasons to reject an activation plan or certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TairaActivationErrorV1 {
    /// Required fields, root, network, notice or roster invariants are invalid.
    #[error("invalid Taira activation shape: {0}")]
    Shape(&'static str),
    /// The canonical frame exceeds the fixed certificate bound.
    #[error("Taira activation exceeds its canonical byte bound")]
    TooLarge,
    /// A frame cannot be encoded or is not an exact canonical encoding.
    #[error("invalid Taira activation canonical encoding")]
    Encoding,
    /// One of the four exact roster signatures does not verify.
    #[error("invalid Taira activation signature at roster index {0}")]
    BadSignature(usize),
}

impl TairaActivationPlanV1 {
    /// Validate all State-independent invariants and the encoded size bound.
    ///
    /// # Errors
    /// Rejects missing commitments, an inconsistent change set, a zero notice, repeated keys or an
    /// invalid pool. The roster, exact catalog equality, expiry and once-only scheduling belong to
    /// Core.
    pub fn validate(&self) -> Result<(), TairaActivationErrorV1> {
        if self.version != TAIRA_ACTIVATION_VERSION_V1 {
            return Err(TairaActivationErrorV1::Shape("unsupported version"));
        }
        if self.change_set.root != RootScopeV1::Universal
            || self.change_set.network != self.network_id
            || self.change_set.predecessor == self.change_set.successor
        {
            return Err(TairaActivationErrorV1::Shape(
                "inconsistent successor selection",
            ));
        }
        // Scheduling must land before the block whose start activates the change set.
        if self.notice.blocks == 0 || self.change_set.effective_height < 2 {
            return Err(TairaActivationErrorV1::Shape(
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
            return Err(TairaActivationErrorV1::Shape(
                "missing commitment or expiry",
            ));
        }
        for (index, key) in self.committee.iter().enumerate() {
            if self.committee[..index].contains(key) {
                return Err(TairaActivationErrorV1::Shape("repeated committee key"));
            }
        }
        self.pool
            .validate()
            .map_err(|_| TairaActivationErrorV1::Shape("invalid Ironwood pool"))?;
        check_size(self)
    }

    /// Canonical unsigned plan, bounded and validated before allocation of its frame.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, TairaActivationErrorV1> {
        self.validate()?;
        norito::encode_canonical(self).map_err(|_| TairaActivationErrorV1::Encoding)
    }

    /// Decode an exact canonical, bounded and structurally valid unsigned plan.
    ///
    /// # Errors
    /// Rejects excess size, malformed, noncanonical, trailing or structurally invalid data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TairaActivationErrorV1> {
        check_input_size(bytes)?;
        let plan: Self =
            norito::decode_canonical(bytes).map_err(|_| TairaActivationErrorV1::Encoding)?;
        if plan.to_bytes()? != bytes {
            return Err(TairaActivationErrorV1::Encoding);
        }
        Ok(plan)
    }

    /// Domain-separated canonical bytes every roster member must sign verbatim.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, TairaActivationErrorV1> {
        let bytes = self.to_bytes()?;
        let mut payload =
            Vec::with_capacity(TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1.len() + bytes.len());
        payload.extend_from_slice(TAIRA_ACTIVATION_SIGNATURE_DOMAIN_V1);
        payload.extend_from_slice(&bytes);
        Ok(payload)
    }

    /// Domain-separated identity of every field of the exact canonical plan.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn id(&self) -> Result<[u8; 32], TairaActivationErrorV1> {
        Ok(Hash::new_from_chunks(&[TAIRA_ACTIVATION_ID_DOMAIN_V1, &self.to_bytes()?]).into())
    }
}

impl TairaActivationCertificateV1 {
    /// Verify the bounded plan and every signature in the exact four-seat order.
    ///
    /// # Errors
    /// Rejects invalid shape, excess size or any missing, swapped, repeated or invalid signature.
    /// This does not replace Core's authentication of the roster and selection.
    pub fn verify(&self) -> Result<(), TairaActivationErrorV1> {
        check_size(self)?;
        let bytes = self.plan.signing_bytes()?;
        for (index, (signature, key)) in
            self.signatures.iter().zip(&self.plan.committee).enumerate()
        {
            signature
                .verify(key, &bytes)
                .map_err(|_| TairaActivationErrorV1::BadSignature(index))?;
        }
        Ok(())
    }

    /// Canonical bounded certificate after all four signatures have been verified.
    ///
    /// # Errors
    /// Rejects an invalid certificate or an encoding failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, TairaActivationErrorV1> {
        self.verify()?;
        norito::encode_canonical(self).map_err(|_| TairaActivationErrorV1::Encoding)
    }

    /// Decode an exact canonical, bounded and fully signed certificate.
    ///
    /// # Errors
    /// Rejects excess size, malformed, noncanonical, trailing or incorrectly signed data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TairaActivationErrorV1> {
        check_input_size(bytes)?;
        let certificate: Self =
            norito::decode_canonical(bytes).map_err(|_| TairaActivationErrorV1::Encoding)?;
        if certificate.to_bytes()? != bytes {
            return Err(TairaActivationErrorV1::Encoding);
        }
        Ok(certificate)
    }
}

fn check_size(value: &impl norito::core::NoritoSerialize) -> Result<(), TairaActivationErrorV1> {
    if norito::canonical_frame_len(value).map_err(|_| TairaActivationErrorV1::Encoding)?
        > TAIRA_ACTIVATION_MAX_CERTIFICATE_BYTES_V1
    {
        return Err(TairaActivationErrorV1::TooLarge);
    }
    Ok(())
}

fn check_input_size(bytes: &[u8]) -> Result<(), TairaActivationErrorV1> {
    if bytes.len() > TAIRA_ACTIVATION_MAX_CERTIFICATE_BYTES_V1 {
        return Err(TairaActivationErrorV1::TooLarge);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
