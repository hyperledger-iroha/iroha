//! Public and restricted Torii DTOs for atomic private cross-dataspace settlement.
//!
//! Public status objects contain only protocol-allowlisted identifiers, routes,
//! timing, commitments, roots, nullifiers, ciphertexts, quorum certificates,
//! sponsor/fee terms, and terminal state.  Proof bytes and encrypted audit
//! capsules appear only in authenticated restricted requests or auditor
//! responses.  Auditor plaintext is never an HTTP DTO.

use core::fmt;

use iroha_crypto::Hash;
use iroha_data_model::nexus::{
    AtomicPrivateSettlementV1, PrivateSettlementAbortReceiptV1, PrivateSettlementAuditApprovalV1,
    PrivateSettlementAuditCapsuleV1, PrivateSettlementAuditPolicyV1,
    PrivateSettlementAvailabilityShareV1, PrivateSettlementCommitteeAuthorityV1,
    PrivateSettlementDeltaV1, PrivateSettlementLegPayloadV1, PrivateSettlementPhaseCertificateV1,
    PrivateSettlementPhaseV1, PrivateSettlementPhaseVoteV1, PrivateSettlementPrepareBarrierV1,
    PrivateSettlementProofStatementV1, PrivateSettlementProvisionalLegMaterialV1,
    PrivateSettlementReceiptV1, PrivateSettlementRouteV1, PrivateSettlementSidecarAvailabilityV1,
};
use iroha_data_model::transaction::SignedTransaction;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Durable first-release settlement lifecycle projected by Torii.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum PrivateSettlementLifecycleDtoV1 {
    /// Encrypted sidecar is durable and auditor approvals are being collected.
    Collecting,
    /// The governed local auditor threshold is durable.
    Audited,
    /// The committee durably staged the verified private-state delta.
    Prepared,
    /// The local Commit QC is durable.
    CommitCertified,
    /// Every leg was atomically applied and a public receipt is durable.
    Finalized,
    /// An authoritative abort released the staged lock.
    Aborted,
    /// Height expiry released the staged lock.
    Expired,
}

/// Account-authenticated upload of one complete encrypted settlement leg.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementLegUploadRequestV1 {
    /// Exact public atomic bundle manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Governed local auditor policy active at the authority context.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact four-validator participant authority.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Proof, fixed opaque delta, encrypted capsule, and availability certificate.
    pub payload: PrivateSettlementLegPayloadV1,
}

/// Sponsor-authenticated request for one node's provisional availability share.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAvailabilityShareRequestV1 {
    /// Exact encrypted material that the node must persist before signing.
    pub material: PrivateSettlementProvisionalLegMaterialV1,
}

/// Redacted response carrying one node-authenticated availability share.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAvailabilityShareResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact encrypted leg material.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Whether this request stored new bytes or reused an exact durable record.
    pub disposition: PrivateSettlementLegUploadDispositionV1,
    /// Exact BLS-normal share issued by this node after persistence.
    pub share: PrivateSettlementAvailabilityShareV1,
}

/// Sponsor-authenticated request for one independently verified Prepare vote.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPrepareVoteRequestV1 {
    /// Exact finalized public manifest retained with the local encrypted leg.
    pub manifest: AtomicPrivateSettlementV1,
    /// Content address selecting the exact local participant leg.
    pub payload_digest: Hash,
}

/// Sponsor-authenticated request for one exact complete-barrier Commit vote.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementCommitVoteRequestV1 {
    /// Content address selecting the exact locally staged participant leg.
    pub payload_digest: Hash,
    /// Exact complete all-Prepare barrier to bind into Commit.
    pub barrier: PrivateSettlementPrepareBarrierV1,
}

/// Redacted response carrying one node-authenticated participant phase vote.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseVoteResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact local encrypted leg.
    pub payload_digest: Hash,
    /// Canonical participant leg ordinal.
    pub leg_ordinal: u8,
    /// Exact BLS-normal phase vote issued by this node.
    pub vote: PrivateSettlementPhaseVoteV1,
}

/// Sponsor-authenticated handoff of one aggregate phase certificate to signers.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseCertificateRequestV1 {
    /// Exact finalized public manifest retained with the local encrypted leg.
    pub manifest: AtomicPrivateSettlementV1,
    /// Content address selecting the exact local participant leg.
    pub payload_digest: Hash,
    /// Exact three-of-four aggregate Prepare or Commit certificate.
    pub certificate: PrivateSettlementPhaseCertificateV1,
}

/// Redacted acknowledgement that one aggregate phase certificate is durable.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseCertificateResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact local encrypted leg.
    pub payload_digest: Hash,
    /// Canonical participant leg ordinal.
    pub leg_ordinal: u8,
    /// Durable participant phase.
    pub phase: PrivateSettlementPhaseV1,
    /// Current durable local lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Sponsor-only recovery view for exact locally durable phase certificates.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementPhaseCertificatesResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact local encrypted leg.
    pub payload_digest: Hash,
    /// Canonical participant leg ordinal.
    pub leg_ordinal: u8,
    /// Current durable local lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
    /// Exact locally durable Prepare QC, or explicit absence.
    #[norito(required)]
    pub prepare_certificate: Option<PrivateSettlementPhaseCertificateV1>,
    /// Exact locally durable Commit QC, or explicit absence.
    #[norito(required)]
    pub commit_certificate: Option<PrivateSettlementPhaseCertificateV1>,
}

/// Idempotent encrypted-leg upload disposition.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(tag = "result", content = "value", rename_all = "snake_case")]
pub enum PrivateSettlementLegUploadDispositionV1 {
    /// New encrypted bytes became durable.
    Stored,
    /// The exact canonical encrypted bytes were already durable.
    AlreadyStored,
}

/// Redacted response for an encrypted-leg upload.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementLegUploadResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the exact encrypted upload.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Idempotent storage outcome.
    pub disposition: PrivateSettlementLegUploadDispositionV1,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Redacted lifecycle response for one encrypted settlement leg.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementLegStatusResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the encrypted upload.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Public participant route.
    pub route: PrivateSettlementRouteV1,
    /// Height at which encrypted bytes first became durable.
    pub stored_at_height: u64,
    /// Latest durable lifecycle height.
    pub lifecycle_height: u64,
    /// Final valid global height.
    pub expiry_height: u64,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Restricted proof view returned only to an exact participant validator.
///
/// This DTO deliberately omits encrypted capsule bytes and every plaintext
/// audit opening. Validators receive only the proof, fixed opaque delta,
/// governed approval material, and durable-availability evidence needed for
/// Prepare.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementCommitteeProofResponseV1 {
    /// Exact public bundle manifest.
    pub manifest: AtomicPrivateSettlementV1,
    /// Governed auditor policy used to verify signatures and threshold.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact four-validator participant authority.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Restricted public proof statement.
    pub statement: PrivateSettlementProofStatementV1,
    /// Native fixed-profile zero-knowledge proof bytes.
    pub proof: Vec<u8>,
    /// Opaque fixed-shape private-state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Canonical governed auditor approvals collected so far.
    pub audit_approvals: Vec<PrivateSettlementAuditApprovalV1>,
    /// Digest of the encrypted capsule; the capsule itself is absent.
    pub audit_capsule_digest: Hash,
    /// Durable restricted-DA certificate.
    pub availability: PrivateSettlementSidecarAvailabilityV1,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

impl fmt::Debug for PrivateSettlementCommitteeProofResponseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementCommitteeProofResponseV1")
            .field("bundle_id", &self.manifest.bundle_id)
            .field("leg_ordinal", &self.statement.leg_ordinal)
            .field("route", &self.statement.route)
            .field("audit_approvals", &self.audit_approvals.len())
            .field("lifecycle", &self.lifecycle)
            .finish_non_exhaustive()
    }
}

/// Restricted capsule response returned only to a governed local auditor.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditorCapsuleResponseV1 {
    /// Exact public manifest used to recompute bindings.
    pub manifest: AtomicPrivateSettlementV1,
    /// Exact governed local policy.
    pub audit_policy: PrivateSettlementAuditPolicyV1,
    /// Exact consensus authority used to enforce key separation.
    pub committee_authority: PrivateSettlementCommitteeAuthorityV1,
    /// Restricted proof statement; proof bytes are deliberately absent.
    pub statement: PrivateSettlementProofStatementV1,
    /// Opaque fixed-shape private-state delta.
    pub delta: PrivateSettlementDeltaV1,
    /// Padded hybrid-encrypted capsule.
    pub audit_capsule: PrivateSettlementAuditCapsuleV1,
    /// Durable restricted-DA certificate.
    pub availability: PrivateSettlementSidecarAvailabilityV1,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Governed-auditor submission of one purpose-separated approval.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditApprovalRequestV1 {
    /// Signed approval whose auditor identity must match the request key.
    pub approval: PrivateSettlementAuditApprovalV1,
}

/// Redacted result of durably collecting one approval.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementAuditApprovalResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Content address of the encrypted leg.
    pub payload_digest: Hash,
    /// Canonical leg ordinal.
    pub leg_ordinal: u8,
    /// Number of distinct governed approvals now durable.
    pub collected: u8,
    /// Governed approval threshold.
    pub required: u8,
    /// Whether this request inserted new durable approval material.
    pub newly_recorded: bool,
    /// Current durable lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
}

/// Sponsor-authenticated complete global finalization or abort carrier submission.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementBundleSubmitRequestV1 {
    /// Exact sponsor-signed transaction carrying one finalization or abort instruction.
    pub transaction: SignedTransaction,
}

/// Redacted admission response for one globally submitted carrier.
#[derive(
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementBundleSubmitResponseV1 {
    /// Public bundle identifier.
    pub bundle_id: Hash,
    /// Height observed when the carrier entered transaction admission.
    pub accepted_at_height: u64,
    /// Public carrier hash or transaction identifier assigned by admission.
    pub carrier_id: Hash,
}

/// Public allowlisted bundle status.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
pub struct PrivateSettlementBundleStatusResponseV1 {
    /// Exact public bundle manifest when locally retained.
    ///
    /// An opaque global abort marker can outlive restricted sidecars, so
    /// terminal abort status remains queryable without manufacturing fields.
    #[norito(required)]
    pub manifest: Option<AtomicPrivateSettlementV1>,
    /// Current public lifecycle.
    pub lifecycle: PrivateSettlementLifecycleDtoV1,
    /// Finalized height when terminal success is durable.
    #[norito(required)]
    pub finalized_height: Option<u64>,
}

/// Public receipt query result.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(tag = "status", content = "value", rename_all = "snake_case")]
pub enum PrivateSettlementBundleReceiptResponseV1 {
    /// The bundle is known but has no public terminal receipt yet.
    Pending {
        /// Public bundle identifier.
        bundle_id: Hash,
        /// Current public lifecycle.
        lifecycle: PrivateSettlementLifecycleDtoV1,
    },
    /// Every private delta finalized atomically.
    Finalized(PrivateSettlementReceiptV1),
    /// The bundle terminated without applying any private delta.
    Aborted(PrivateSettlementAbortReceiptV1),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_status_json_requires_explicit_manifest() {
        let response = PrivateSettlementBundleStatusResponseV1 {
            manifest: None,
            lifecycle: PrivateSettlementLifecycleDtoV1::Aborted,
            finalized_height: None,
        };
        let value = norito::json::to_value(&response).expect("encode bundle status JSON");
        assert_eq!(value.get("manifest"), Some(&norito::json::Value::Null));
        let decoded =
            norito::json::from_value::<PrivateSettlementBundleStatusResponseV1>(value.clone())
                .expect("explicit null manifest decodes");
        assert_eq!(decoded, response);

        let mut omitted = value;
        omitted
            .as_object_mut()
            .expect("bundle status is a JSON object")
            .remove("manifest");
        let error = norito::json::from_value::<PrivateSettlementBundleStatusResponseV1>(omitted)
            .expect_err("omitted manifest must reject");
        assert!(error.to_string().contains("missing field `manifest`"));
    }

    #[test]
    fn lifecycle_and_pending_receipt_roundtrip_canonically() {
        let response = PrivateSettlementBundleReceiptResponseV1::Pending {
            bundle_id: Hash::new(b"private-settlement-dto-roundtrip"),
            lifecycle: PrivateSettlementLifecycleDtoV1::Prepared,
        };
        let bytes = norito::encode_canonical(&response).expect("DTO encodes canonically");
        let decoded: PrivateSettlementBundleReceiptResponseV1 =
            norito::decode_canonical(&bytes).expect("DTO decodes canonically");
        assert_eq!(decoded, response);
        let json = norito::json::to_json(&response).expect("DTO JSON encodes");
        let decoded_json: PrivateSettlementBundleReceiptResponseV1 =
            norito::json::from_json(&json).expect("DTO JSON decodes");
        assert_eq!(decoded_json, response);
    }

    #[test]
    fn phase_certificate_recovery_requires_explicit_optional_fields() {
        let response = PrivateSettlementPhaseCertificatesResponseV1 {
            bundle_id: Hash::new(b"private-settlement-recovery-bundle"),
            payload_digest: Hash::new(b"private-settlement-recovery-payload"),
            leg_ordinal: 1,
            lifecycle: PrivateSettlementLifecycleDtoV1::Prepared,
            prepare_certificate: None,
            commit_certificate: None,
        };
        let value = norito::json::to_value(&response).expect("encode recovery JSON");
        assert_eq!(
            value.get("prepare_certificate"),
            Some(&norito::json::Value::Null)
        );
        assert_eq!(
            value.get("commit_certificate"),
            Some(&norito::json::Value::Null)
        );
        let decoded =
            norito::json::from_value::<PrivateSettlementPhaseCertificatesResponseV1>(value.clone())
                .expect("explicit null recovery fields decode");
        assert_eq!(decoded, response);

        let mut omitted = value;
        omitted
            .as_object_mut()
            .expect("recovery response is an object")
            .remove("prepare_certificate");
        let error =
            norito::json::from_value::<PrivateSettlementPhaseCertificatesResponseV1>(omitted)
                .expect_err("omitted recovery field must reject");
        assert!(
            error
                .to_string()
                .contains("missing field `prepare_certificate`")
        );
    }
}
