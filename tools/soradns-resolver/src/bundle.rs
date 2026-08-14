use norito_derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use crate::limits::{MAX_CHILD_STRINGS, MAX_FIELD_BYTES, MAX_IDENTIFIER_BYTES};
/// Expected length of a Blake3 digest used for namehash and manifest hashes.
pub const BLAKE3_HASH_LEN: usize = 32;
/// Proof bundle describing the linkage between registry entries, manifests, and CAR archives.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct ProofBundleV1 {
    pub namehash: [u8; BLAKE3_HASH_LEN],
    pub zone_version: u64,
    pub manifest_hash: [u8; BLAKE3_HASH_LEN],
    pub car_root_cid: String,
    pub ksk_set: Vec<KskEntryV1>,
    pub zsk_signatures: Vec<ZskSignatureV1>,
    pub delegation_chain: Vec<DelegationProofV1>,
    pub freshness: FreshnessProofV1,
    pub policy_hash: [u8; BLAKE3_HASH_LEN],
}
impl ProofBundleV1 {
    /// Validate bundle invariants according to DG-1 specification.
    pub fn validate(&self) -> Result<(), ProofBundleValidationError> {
        self.validate_resource_bounds()?;
        if self.zone_version == 0 {
            return Err(ProofBundleValidationError::InvalidZoneVersion);
        }
        if is_zero_hash(&self.manifest_hash) {
            return Err(ProofBundleValidationError::InvalidManifestHash);
        }
        if is_zero_hash(&self.namehash) {
            return Err(ProofBundleValidationError::InvalidNamehash);
        }
        if is_zero_hash(&self.policy_hash) {
            return Err(ProofBundleValidationError::InvalidPolicyHash);
        }
        if self.car_root_cid.trim().is_empty() {
            return Err(ProofBundleValidationError::MissingCarRootCid);
        }
        if self.ksk_set.is_empty() {
            return Err(ProofBundleValidationError::MissingKskEntries);
        }
        for entry in &self.ksk_set {
            entry.validate()?;
        }
        if self.zsk_signatures.is_empty() {
            return Err(ProofBundleValidationError::MissingZskSignatures);
        }
        for signature in &self.zsk_signatures {
            signature.validate()?;
        }
        for proof in &self.delegation_chain {
            proof.validate()?;
        }
        self.freshness.validate()?;
        Ok(())
    }
    /// Validate source-coupled collection and field limits before retention.
    pub(crate) fn validate_resource_bounds(&self) -> Result<(), ProofBundleValidationError> {
        check_count("ksk_set", self.ksk_set.len(), MAX_CHILD_STRINGS)?;
        check_count(
            "zsk_signatures",
            self.zsk_signatures.len(),
            MAX_CHILD_STRINGS,
        )?;
        check_count(
            "delegation_chain",
            self.delegation_chain.len(),
            MAX_CHILD_STRINGS,
        )?;
        check_bytes(
            "car_root_cid",
            self.car_root_cid.len(),
            MAX_IDENTIFIER_BYTES,
        )?;
        for entry in &self.ksk_set {
            check_bytes("ksk public_key", entry.public_key.len(), MAX_FIELD_BYTES)?;
            check_bytes("ksk signature", entry.signature.len(), MAX_FIELD_BYTES)?;
        }
        for signature in &self.zsk_signatures {
            check_bytes("zsk id", signature.zsk_id.len(), MAX_FIELD_BYTES)?;
            check_bytes("zsk signature", signature.signature.len(), MAX_FIELD_BYTES)?;
        }
        for proof in &self.delegation_chain {
            check_bytes(
                "delegation signature",
                proof.signature.len(),
                MAX_FIELD_BYTES,
            )?;
        }
        check_bytes(
            "freshness signer",
            self.freshness.signer.len(),
            MAX_IDENTIFIER_BYTES,
        )?;
        check_bytes(
            "freshness signature",
            self.freshness.signature.len(),
            MAX_FIELD_BYTES,
        )
    }
    /// Account the heap retained by this decoded bundle, including spare capacities.
    pub(crate) fn retained_bytes(&self) -> Result<usize, ProofBundleValidationError> {
        self.validate_resource_bounds()?;
        let mut bytes = std::mem::size_of::<Self>();
        charge(&mut bytes, self.car_root_cid.capacity())?;
        charge_vec::<KskEntryV1>(&mut bytes, self.ksk_set.capacity())?;
        for entry in &self.ksk_set {
            charge(&mut bytes, entry.public_key.capacity())?;
            charge(&mut bytes, entry.signature.capacity())?;
        }
        charge_vec::<ZskSignatureV1>(&mut bytes, self.zsk_signatures.capacity())?;
        for signature in &self.zsk_signatures {
            charge(&mut bytes, signature.zsk_id.capacity())?;
            charge(&mut bytes, signature.signature.capacity())?;
        }
        charge_vec::<DelegationProofV1>(&mut bytes, self.delegation_chain.capacity())?;
        for proof in &self.delegation_chain {
            charge(&mut bytes, proof.signature.capacity())?;
        }
        charge(&mut bytes, self.freshness.signer.capacity())?;
        charge(&mut bytes, self.freshness.signature.capacity())?;
        Ok(bytes)
    }
    /// Returns the hexadecimal representation of the bundle namehash.
    #[must_use]
    pub fn namehash_hex(&self) -> String {
        hex::encode(self.namehash)
    }
}
fn check_count(
    field: &'static str,
    found: usize,
    maximum: usize,
) -> Result<(), ProofBundleValidationError> {
    if found > maximum {
        return Err(ProofBundleValidationError::ResourceLimit {
            field,
            found,
            maximum,
        });
    }
    Ok(())
}
fn check_bytes(
    field: &'static str,
    found: usize,
    maximum: usize,
) -> Result<(), ProofBundleValidationError> {
    check_count(field, found, maximum)
}
fn charge(total: &mut usize, additional: usize) -> Result<(), ProofBundleValidationError> {
    *total = (*total)
        .checked_add(additional)
        .ok_or(ProofBundleValidationError::RetainedSizeOverflow)?;
    Ok(())
}
fn charge_vec<T>(total: &mut usize, capacity: usize) -> Result<(), ProofBundleValidationError> {
    let bytes = capacity
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(ProofBundleValidationError::RetainedSizeOverflow)?;
    charge(total, bytes)
}
fn is_zero_hash(hash: &[u8; BLAKE3_HASH_LEN]) -> bool {
    hash.iter().all(|byte| *byte == 0)
}
/// Key signing key (KSK) entry.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct KskEntryV1 {
    pub public_key: Vec<u8>,
    pub valid_from: u64,
    pub valid_until: u64,
    pub signature: Vec<u8>,
}
impl KskEntryV1 {
    fn validate(&self) -> Result<(), ProofBundleValidationError> {
        if self.public_key.is_empty() {
            return Err(ProofBundleValidationError::EmptyKskPublicKey);
        }
        if self.signature.is_empty() {
            return Err(ProofBundleValidationError::EmptyKskSignature);
        }
        if self.valid_until <= self.valid_from {
            return Err(ProofBundleValidationError::KskValidityWindow);
        }
        Ok(())
    }
}
/// Zone signing key (ZSK) signature entry.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct ZskSignatureV1 {
    pub zsk_id: Vec<u8>,
    pub signature: Vec<u8>,
}
impl ZskSignatureV1 {
    fn validate(&self) -> Result<(), ProofBundleValidationError> {
        if self.zsk_id.is_empty() {
            return Err(ProofBundleValidationError::EmptyZskId);
        }
        if self.signature.is_empty() {
            return Err(ProofBundleValidationError::EmptyZskSignature);
        }
        Ok(())
    }
}
/// Delegation proof linking parent and child zones.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct DelegationProofV1 {
    pub parent_namehash: [u8; BLAKE3_HASH_LEN],
    pub child_namehash: [u8; BLAKE3_HASH_LEN],
    pub valid_from: u64,
    pub valid_until: u64,
    pub signature: Vec<u8>,
}
impl DelegationProofV1 {
    fn validate(&self) -> Result<(), ProofBundleValidationError> {
        if is_zero_hash(&self.parent_namehash) {
            return Err(ProofBundleValidationError::InvalidDelegationParent);
        }
        if is_zero_hash(&self.child_namehash) {
            return Err(ProofBundleValidationError::InvalidDelegationChild);
        }
        if self.valid_until <= self.valid_from {
            return Err(ProofBundleValidationError::DelegationValidityWindow);
        }
        if self.signature.is_empty() {
            return Err(ProofBundleValidationError::EmptyDelegationSignature);
        }
        Ok(())
    }
}
/// Freshness proof describing the attestation of bundle freshness.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
pub struct FreshnessProofV1 {
    pub issued_at: u64,
    pub expires_at: u64,
    pub signer: String,
    pub signature: Vec<u8>,
}
impl FreshnessProofV1 {
    fn validate(&self) -> Result<(), ProofBundleValidationError> {
        if self.signature.is_empty() {
            return Err(ProofBundleValidationError::EmptyFreshnessSignature);
        }
        if self.signer.trim().is_empty() {
            return Err(ProofBundleValidationError::EmptyFreshnessSigner);
        }
        if self.expires_at <= self.issued_at {
            return Err(ProofBundleValidationError::FreshnessWindow);
        }
        Ok(())
    }
}
/// Errors raised when validating proof bundles.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProofBundleValidationError {
    /// A variable-width field or child collection exceeded its first-release limit.
    #[error("{field} contains {found} entries/bytes; the limit is {maximum}")]
    ResourceLimit {
        /// Stable field label used in diagnostics.
        field: &'static str,
        /// Observed element or byte count.
        found: usize,
        /// Maximum admitted element or byte count.
        maximum: usize,
    },
    /// Retained-memory accounting overflowed `usize`.
    #[error("proof bundle retained-byte accounting overflowed")]
    RetainedSizeOverflow,
    #[error("namehash must not be zero")]
    InvalidNamehash,
    #[error("zone_version must be non-zero")]
    InvalidZoneVersion,
    #[error("manifest hash must not be zero")]
    InvalidManifestHash,
    #[error("policy hash must not be zero")]
    InvalidPolicyHash,
    #[error("car_root_cid must not be empty")]
    MissingCarRootCid,
    #[error("bundle must contain at least one KSK entry")]
    MissingKskEntries,
    #[error("bundle must contain at least one ZSK signature")]
    MissingZskSignatures,
    #[error("ksk public key must not be empty")]
    EmptyKskPublicKey,
    #[error("ksk signature must not be empty")]
    EmptyKskSignature,
    #[error("ksk validity window must be positive")]
    KskValidityWindow,
    #[error("zsk id must not be empty")]
    EmptyZskId,
    #[error("zsk signature must not be empty")]
    EmptyZskSignature,
    #[error("delegation parent hash must not be zero")]
    InvalidDelegationParent,
    #[error("delegation child hash must not be zero")]
    InvalidDelegationChild,
    #[error("delegation validity window must be positive")]
    DelegationValidityWindow,
    #[error("delegation signature must not be empty")]
    EmptyDelegationSignature,
    #[error("freshness signature must not be empty")]
    EmptyFreshnessSignature,
    #[error("freshness signer must not be empty")]
    EmptyFreshnessSigner,
    #[error("freshness validity window must be positive")]
    FreshnessWindow,
}
#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use expect_test::expect;
    use norito::{Compression, deserialize_from, serialize_into};
    use super::*;
    fn make_hash(seed: u8) -> [u8; BLAKE3_HASH_LEN] {
        let mut hash = [0_u8; BLAKE3_HASH_LEN];
        for (idx, byte) in hash.iter_mut().enumerate() {
            *byte = seed.wrapping_add(idx as u8);
        }
        hash
    }
    fn sample_bundle() -> ProofBundleV1 {
        ProofBundleV1 {
            namehash: make_hash(0x01),
            zone_version: 1,
            manifest_hash: make_hash(0x02),
            car_root_cid: "bafybeigdyrzt...".to_string(),
            ksk_set: vec![KskEntryV1 {
                public_key: vec![0x01, 0x02],
                valid_from: 100,
                valid_until: 200,
                signature: vec![0xAA],
            }],
            zsk_signatures: vec![ZskSignatureV1 {
                zsk_id: vec![0x10],
                signature: vec![0xBB],
            }],
            delegation_chain: vec![DelegationProofV1 {
                parent_namehash: make_hash(0x03),
                child_namehash: make_hash(0x04),
                valid_from: 100,
                valid_until: 300,
                signature: vec![0xCC],
            }],
            freshness: FreshnessProofV1 {
                issued_at: 1_000,
                expires_at: 2_000,
                signer: "governance.sora".to_string(),
                signature: vec![0xDD],
            },
            policy_hash: make_hash(0x05),
        }
    }
    #[test]
    fn bundle_validates() {
        let bundle = sample_bundle();
        assert!(bundle.validate().is_ok());
    }
    #[test]
    fn bundle_roundtrip() {
        let bundle = sample_bundle();
        let mut buf = Vec::new();
        serialize_into(&mut buf, &bundle, Compression::None).expect("serialize proof bundle");
        let decoded: ProofBundleV1 =
            deserialize_from(&mut Cursor::new(&buf)).expect("decode proof bundle");
        assert_eq!(decoded, bundle);
    }
    #[test]
    fn detects_missing_ksk() {
        let mut bundle = sample_bundle();
        bundle.ksk_set.clear();
        let error = bundle.validate().expect_err("validation failure expected");
        expect!["bundle must contain at least one KSK entry"].assert_eq(&error.to_string());
    }
    #[test]
    fn detects_invalid_freshness_window() {
        let mut bundle = sample_bundle();
        bundle.freshness.expires_at = bundle.freshness.issued_at;
        let error = bundle.validate().expect_err("validation failure expected");
        expect!["freshness validity window must be positive"].assert_eq(&error.to_string());
    }
    #[test]
    fn bundle_collection_limit_accepts_exact_and_rejects_plus_one() {
        let mut bundle = sample_bundle();
        bundle.ksk_set = vec![bundle.ksk_set[0].clone(); MAX_CHILD_STRINGS];
        bundle
            .validate_resource_bounds()
            .expect("exact KSK count is admitted");
        bundle.ksk_set.push(bundle.ksk_set[0].clone());
        assert!(matches!(
            bundle.validate_resource_bounds(),
            Err(ProofBundleValidationError::ResourceLimit {
                field: "ksk_set",
                found,
                maximum: MAX_CHILD_STRINGS,
            }) if found == MAX_CHILD_STRINGS + 1
        ));
    }
}
