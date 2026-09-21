//! Sole candidate-bound topology configuration subject; no detached-envelope decoder.

use super::SignerTopologyReceiptErrorV1 as Error;
use crate::signer::{
    custody::SignerCustodyBindingV1,
    protocol::{SignerKeyAlgorithmV1, SignerPurposeBindingV1, SignerRoleV1, digest_canonical},
};
use norito::codec::{Decode, Encode};

/// Maximum exact domain-prefixed topology configuration approval message.
pub const TOPOLOGY_APPROVAL_MAX_BYTES_V1: usize = 4096;
/// Maximum independently reviewed configuration approval lifetime: fourteen days.
pub const TOPOLOGY_APPROVAL_MAX_VALIDITY_MS_V1: u64 = 14 * 24 * 60 * 60 * 1000;
/// Domain of the sole canonical topology configuration approval, before its Norito frame.
pub const TOPOLOGY_APPROVAL_DOMAIN_V1: &[u8] = b"iroha:sorafs:topology-configuration-approval:v1\0";

/// Independently reviewed configuration evidence for one exact release candidate.
///
/// This approves configuration only. It cannot assert a live deployment, resilience, throughput,
/// custody, native execution, or promotion. Digests are SHA-256 over the named exact inputs;
/// the ordered-validator digest retains the qualification summary's canonical calculation.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_manifest::signer::topology::subject::TopologyApprovalSubjectV1")]
pub struct TopologyApprovalSubjectV1 {
    /// Canonical deployment identity; must equal the independently governed signer purpose.
    pub deployment_id: String,
    /// Exact genesis-derived network identity, never an operator label alone.
    pub network_id: [u8; 32],
    /// Exact governed chain label.
    pub chain_id: String,
    /// Reviewed chain address discriminator; equality is checked against independent inputs.
    pub chain_discriminant: u16,
    /// Exact reviewed release manifest, binding the configuration to one candidate.
    pub release_manifest_sha256: [u8; 32],
    /// Exact configuration-only qualification summary bytes.
    pub qualification_summary_sha256: [u8; 32],
    /// Exact source topology manifest bytes.
    pub manifest_sha256: [u8; 32],
    /// Exact canonical topology manifest bytes.
    pub canonical_manifest_sha256: [u8; 32],
    /// Canonical ordered four-validator identity inventory digest.
    pub validator_ids_sha256: [u8; 32],
    /// Independently approved review time, in Unix milliseconds.
    pub reviewed_at_unix_ms: u64,
    /// Exclusive reviewed expiry, bounded by fourteen days.
    pub expires_at_unix_ms: u64,
}

/// Prepared canonical bytes retaining their entire independently reviewed subject and binding.
///
/// Preparation proves grammar and input equality only; it grants no signer or native authority.
#[derive(Debug)]
pub struct PreparedTopologyApprovalV1 {
    subject: TopologyApprovalSubjectV1,
    message: Vec<u8>,
    binding_digest: [u8; 32],
}
impl PreparedTopologyApprovalV1 {
    /// Borrow the exact canonical role payload.
    #[must_use]
    pub fn message(&self) -> &[u8] {
        &self.message
    }
    /// Borrow the retained reviewed input; no finality or authority is implied.
    #[must_use]
    pub const fn subject(&self) -> &TopologyApprovalSubjectV1 {
        &self.subject
    }
    /// Commit the entire original signer binding.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }
}

/// Prepare exact canonical bytes for a separately reviewed configuration subject.
///
/// # Errors
/// Rejects wrong roles, another deployment/network/chain, invalid custody grammar, zero digests,
/// unbounded review periods or encoding bounds. No alternate JSON/envelope layout is accepted.
pub fn prepare_topology_approval_v1(
    subject: &TopologyApprovalSubjectV1,
    binding: &SignerCustodyBindingV1,
) -> Result<PreparedTopologyApprovalV1, Error> {
    validate_topology_binding(binding)?;
    let SignerPurposeBindingV1::TopologyApproval { deployment_id } = &binding.purpose else {
        return Err(Error::WrongPurpose);
    };
    if subject.deployment_id != *deployment_id
        || subject.network_id != binding.network_id
        || subject.chain_id != binding.chain_id
    {
        return Err(Error::SubjectMismatch);
    }
    if [
        subject.release_manifest_sha256,
        subject.qualification_summary_sha256,
        subject.manifest_sha256,
        subject.canonical_manifest_sha256,
        subject.validator_ids_sha256,
    ]
    .contains(&[0; 32])
        || subject.reviewed_at_unix_ms == 0
        || subject.expires_at_unix_ms <= subject.reviewed_at_unix_ms
        || subject.expires_at_unix_ms - subject.reviewed_at_unix_ms
            > TOPOLOGY_APPROVAL_MAX_VALIDITY_MS_V1
    {
        return Err(Error::InvalidSubject);
    }
    let mut message = TOPOLOGY_APPROVAL_DOMAIN_V1.to_vec();
    message.extend(norito::encode_canonical(subject).map_err(|_| Error::InvalidSubject)?);
    if message.len() > TOPOLOGY_APPROVAL_MAX_BYTES_V1 {
        return Err(Error::InvalidSubject);
    }
    Ok(PreparedTopologyApprovalV1 {
        subject: subject.clone(),
        message,
        binding_digest: digest_canonical(b"iroha.sorafs.signer.custody-binding.v1", binding)
            .map_err(|_| Error::InvalidSubject)?,
    })
}

pub(super) fn validate_topology_binding(binding: &SignerCustodyBindingV1) -> Result<(), Error> {
    if binding.role != SignerRoleV1::TopologyApproval
        || !matches!(
            binding.purpose,
            SignerPurposeBindingV1::TopologyApproval { .. }
        )
        || binding.algorithm != SignerKeyAlgorithmV1::Ed25519
    {
        return Err(Error::WrongPurpose);
    }
    binding.validate().map_err(Error::Custody)
}
