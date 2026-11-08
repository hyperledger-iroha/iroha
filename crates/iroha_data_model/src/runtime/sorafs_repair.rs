//! Explicit, all-four-validator SoraFS repair at one authenticated parent boundary.
//!
//! This certificate is not Parliament authority and does not extend any selected lifecycle
//! definition. Core compares the whole change set with its dedicated immutable repair catalog,
//! authenticates the parent and scheduled roster, and atomically selects the successor and applies
//! the bounded payload once. Signature verification here establishes only the signed plan.

use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_model_base::chain::ChainId;

use super::lifecycle::{ChangeSetV1, RootScopeV1};
use crate::{NetworkId, sorafs::bootstrap::SorafsBootstrapPayloadV1};

/// Sole supported repair certificate version.
pub const SORAFS_REPAIR_VERSION_V1: u16 = 1;
/// Exact number of scheduled validators that must independently authorize this repair.
pub const SORAFS_REPAIR_COMMITTEE_SIZE_V1: usize = 4;
/// Maximum canonical plan or certificate bytes, below the signed transaction size limit.
pub const SORAFS_REPAIR_MAX_CERTIFICATE_BYTES_V1: usize = 8 * 1024 * 1024;
/// Domain separator for all four signatures over the canonical plan.
pub const SORAFS_REPAIR_SIGNATURE_DOMAIN_V1: &[u8] = b"iroha:sorafs:repair:signature:v1\0";
/// Domain separator for the content-addressed plan identity.
pub const SORAFS_REPAIR_ID_DOMAIN_V1: &[u8] = b"iroha:sorafs:repair:plan:v1\0";

/// Exact, independently reviewable authorization for one SoraFS successor selection.
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
#[norito_schema(name = "iroha_data_model::runtime::sorafs_repair::SorafsRepairPlanV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsRepairPlanV1 {
    /// Fixed protocol version, [`SORAFS_REPAIR_VERSION_V1`].
    pub version: u16,
    /// Exact deployment chain label.
    pub chain_id: ChainId,
    /// Signed-genesis-derived network identity.
    pub network_id: NetworkId,
    /// Nonzero, operator-chosen identity of this single approved operation.
    pub operation_id: [u8; 32],
    /// Authenticated parent height; execution is exactly the following height.
    pub parent_height: u64,
    /// Native Sumeragi committed core hash, not the Iroha block header hash.
    pub parent_block_hash: [u8; 32],
    /// Native authenticated execution result at the parent.
    pub parent_result_hash: [u8; 32],
    /// Exact successor definition publication and selection, including qualification evidence.
    pub change_set: ChangeSetV1,
    /// All four scheduled validator keys, in the exact frozen roster order.
    #[norito(json = "crate::runtime::sorafs_repair::four_items_json")]
    pub committee: [PublicKey; SORAFS_REPAIR_COMMITTEE_SIZE_V1],
    /// Last permitted executing block timestamp in Unix milliseconds.
    pub expires_at_unix_ms: u64,
    /// Digest of the explicit, bounded operator authorization being exercised.
    pub authorization_digest: [u8; 32],
    /// Digest of the reviewed release artifact evidence.
    pub release_digest: [u8; 32],
    /// Digest of the reviewed immutable definition source closure evidence.
    pub source_closure_digest: [u8; 32],
    /// Exact first council, admissions and allowlisted SoraFS grants.
    pub payload: SorafsBootstrapPayloadV1,
}

/// Four independent signatures over the complete repair plan, one per ordered roster slot.
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
#[norito_schema(name = "iroha_data_model::runtime::sorafs_repair::SorafsRepairCertificateV1")]
#[norito(deny_unknown_fields)]
pub struct SorafsRepairCertificateV1 {
    /// Every context, authority, evidence and payload field authorized by the signers.
    pub plan: SorafsRepairPlanV1,
    /// Signature at position `i` must verify with `plan.committee[i]`; no quorum subset suffices.
    #[norito(json = "crate::runtime::sorafs_repair::four_items_json")]
    pub signatures: [Signature; SORAFS_REPAIR_COMMITTEE_SIZE_V1],
}

/// Stable State-independent reasons to reject a repair plan or certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SorafsRepairErrorV1 {
    /// Required fields, root, network, height or roster invariants are invalid.
    #[error("invalid SoraFS repair shape: {0}")]
    Shape(&'static str),
    /// The closed bootstrap payload violates its own shape or permission restrictions.
    #[error("invalid SoraFS bootstrap payload")]
    Payload,
    /// The canonical frame exceeds the fixed repair certificate bound.
    #[error("SoraFS repair exceeds its canonical byte bound")]
    TooLarge,
    /// A frame cannot be encoded or is not an exact canonical encoding.
    #[error("invalid SoraFS repair canonical encoding")]
    Encoding,
    /// One of the four exact roster signatures does not verify.
    #[error("invalid SoraFS repair signature at roster index {0}")]
    BadSignature(usize),
}

impl SorafsRepairPlanV1 {
    /// Validate all State-independent invariants and the encoded size bound.
    ///
    /// # Errors
    /// Rejects missing commitments, an inconsistent change set, repeated keys or an invalid payload.
    /// Parent authentication, exact catalog equality, expiry and once-only execution belong to Core.
    pub fn validate(&self) -> Result<(), SorafsRepairErrorV1> {
        if self.version != SORAFS_REPAIR_VERSION_V1 {
            return Err(SorafsRepairErrorV1::Shape("unsupported version"));
        }
        if self.parent_height == 0
            || self.parent_height.checked_add(1) != Some(self.change_set.effective_height)
        {
            return Err(SorafsRepairErrorV1::Shape(
                "invalid parent or effective height",
            ));
        }
        if self.change_set.root != RootScopeV1::Universal
            || self.change_set.network != self.network_id
            || self.change_set.predecessor == self.change_set.successor
        {
            return Err(SorafsRepairErrorV1::Shape(
                "inconsistent successor selection",
            ));
        }
        if self.expires_at_unix_ms == 0
            || [
                self.operation_id,
                self.parent_block_hash,
                self.parent_result_hash,
                self.change_set.qualification,
                self.authorization_digest,
                self.release_digest,
                self.source_closure_digest,
            ]
            .contains(&[0; 32])
        {
            return Err(SorafsRepairErrorV1::Shape("missing commitment or expiry"));
        }
        for (index, key) in self.committee.iter().enumerate() {
            if self.committee[..index].contains(key) {
                return Err(SorafsRepairErrorV1::Shape("repeated committee key"));
            }
        }
        self.payload
            .validate()
            .map_err(|_| SorafsRepairErrorV1::Payload)?;
        check_size(self)
    }

    /// Canonical unsigned plan, bounded and validated before allocation of its frame.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SorafsRepairErrorV1> {
        self.validate()?;
        norito::encode_canonical(self).map_err(|_| SorafsRepairErrorV1::Encoding)
    }

    /// Decode an exact canonical, bounded and structurally valid unsigned plan.
    ///
    /// # Errors
    /// Rejects excess size, malformed, noncanonical, trailing or structurally invalid data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SorafsRepairErrorV1> {
        check_input_size(bytes)?;
        let plan: Self =
            norito::decode_canonical(bytes).map_err(|_| SorafsRepairErrorV1::Encoding)?;
        if plan.to_bytes()? != bytes {
            return Err(SorafsRepairErrorV1::Encoding);
        }
        Ok(plan)
    }

    /// Domain-separated canonical bytes every roster member must sign verbatim.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, SorafsRepairErrorV1> {
        let bytes = self.to_bytes()?;
        let mut payload = Vec::with_capacity(SORAFS_REPAIR_SIGNATURE_DOMAIN_V1.len() + bytes.len());
        payload.extend_from_slice(SORAFS_REPAIR_SIGNATURE_DOMAIN_V1);
        payload.extend_from_slice(&bytes);
        Ok(payload)
    }

    /// Domain-separated identity of every field of the exact canonical plan.
    ///
    /// # Errors
    /// Rejects an invalid plan or an encoding failure.
    pub fn id(&self) -> Result<[u8; 32], SorafsRepairErrorV1> {
        Ok(Hash::new_from_chunks(&[SORAFS_REPAIR_ID_DOMAIN_V1, &self.to_bytes()?]).into())
    }
}

impl SorafsRepairCertificateV1 {
    /// Verify the bounded plan and every signature in the exact four-seat order.
    ///
    /// # Errors
    /// Rejects invalid shape, excess size or any missing, swapped, repeated or invalid signature.
    /// This does not replace Core's authentication of the actual parent, roster and selection.
    pub fn verify(&self) -> Result<(), SorafsRepairErrorV1> {
        check_size(self)?;
        let bytes = self.plan.signing_bytes()?;
        for (index, (signature, key)) in
            self.signatures.iter().zip(&self.plan.committee).enumerate()
        {
            signature
                .verify(key, &bytes)
                .map_err(|_| SorafsRepairErrorV1::BadSignature(index))?;
        }
        Ok(())
    }

    /// Canonical bounded certificate after all four signatures have been verified.
    ///
    /// # Errors
    /// Rejects an invalid certificate or an encoding failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SorafsRepairErrorV1> {
        self.verify()?;
        norito::encode_canonical(self).map_err(|_| SorafsRepairErrorV1::Encoding)
    }

    /// Decode an exact canonical, bounded and fully signed certificate.
    ///
    /// # Errors
    /// Rejects excess size, malformed, noncanonical, trailing or incorrectly signed data.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SorafsRepairErrorV1> {
        check_input_size(bytes)?;
        let certificate: Self =
            norito::decode_canonical(bytes).map_err(|_| SorafsRepairErrorV1::Encoding)?;
        if certificate.to_bytes()? != bytes {
            return Err(SorafsRepairErrorV1::Encoding);
        }
        Ok(certificate)
    }
}

fn check_size(value: &impl norito::core::NoritoSerialize) -> Result<(), SorafsRepairErrorV1> {
    if norito::canonical_frame_len(value).map_err(|_| SorafsRepairErrorV1::Encoding)?
        > SORAFS_REPAIR_MAX_CERTIFICATE_BYTES_V1
    {
        return Err(SorafsRepairErrorV1::TooLarge);
    }
    Ok(())
}

fn check_input_size(bytes: &[u8]) -> Result<(), SorafsRepairErrorV1> {
    if bytes.len() > SORAFS_REPAIR_MAX_CERTIFICATE_BYTES_V1 {
        return Err(SorafsRepairErrorV1::TooLarge);
    }
    Ok(())
}

// Norito's general JSON array implementation is byte-specific. Keep the binary and Rust
// representations fixed at four, and reject a different JSON arity before parsing any item.
pub(crate) mod four_items_json {
    use norito::json::{
        BoundedJsonError, Error, JsonDeserialize, JsonSerialize, JsonWriteSink, Parser,
    };

    pub(crate) fn serialize<T: JsonSerialize>(values: &[T; 4], out: &mut String) {
        out.push('[');
        for (index, value) in values.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            value.json_serialize(out);
        }
        out.push(']');
    }

    pub(crate) fn serialize_bounded<T: JsonSerialize>(
        values: &[T; 4],
        out: &mut dyn JsonWriteSink,
    ) -> Result<(), BoundedJsonError> {
        out.begin_container()?;
        let result = (|| {
            out.push('[')?;
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    out.push(',')?;
                }
                value.json_serialize_to(out)?;
            }
            out.push(']')
        })();
        out.end_container();
        result
    }

    pub(crate) fn deserialize<T: JsonDeserialize>(
        parser: &mut Parser<'_>,
    ) -> Result<[T; 4], Error> {
        let (items, _) = parser.parse_array_with(
            |count| {
                if count != 4 {
                    return Err(Error::Message(
                        "a certified plan requires exactly four roster items".into(),
                    ));
                }
                Ok(([None, None, None, None], 0usize))
            },
            T::json_deserialize,
            |(items, index), item| {
                items[*index] = Some(item);
                *index += 1;
            },
        )?;
        let [first, second, third, fourth] = items;
        let missing = || Error::Message("a certified plan roster item is missing".into());
        Ok([
            first.ok_or_else(missing)?,
            second.ok_or_else(missing)?,
            third.ok_or_else(missing)?,
            fourth.ok_or_else(missing)?,
        ])
    }
}

#[cfg(test)]
mod tests;
