//! Authenticated provisional restricted-DA availability shares.
//!
//! A node retains its BLS private key behind
//! [`PrivateSettlementAvailabilitySignerV1`]. The only signing operation first
//! commits exact encrypted leg material through the owner-only sidecar store,
//! then signs the one body derived from those immutable bytes.

use super::{
    protocol::validate_authority_cryptography_v1,
    sidecar_store::{
        PrivateSettlementFileSidecarStoreV1, PrivateSettlementSidecarStoreErrorV1,
        PrivateSettlementSidecarStoreOutcomeV1,
        verify_private_settlement_availability_certificate_v1,
    },
};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_data_model::{
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, PRIVATE_SETTLEMENT_BLS_BYTES_V1,
        PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1,
        PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1,
        PrivateSettlementAuditApprovalAcknowledgementAttestationV1,
        PrivateSettlementAuditorViewAttestationBodyV1, PrivateSettlementAuditorViewAttestationV1,
        PrivateSettlementAvailabilityShareV1, PrivateSettlementCommitteeAuthorityV1,
        PrivateSettlementProvisionalLegMaterialV1, PrivateSettlementSidecarAvailabilityBodyV1,
        PrivateSettlementSidecarAvailabilityV1,
    },
    peer::PeerId,
};
use std::{collections::BTreeMap, fmt};
use thiserror::Error;

/// Redacted failure from provisional availability signing or aggregation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivateSettlementAvailabilityErrorV1 {
    /// The configured runtime key is not a BLS-normal validator key.
    #[error("private-settlement availability signer is invalid")]
    InvalidSigner,
    /// The signer, authority, body, or signature differs from the exact request.
    #[error("private-settlement availability share is invalid")]
    InvalidShare,
    /// The share set is not exactly three distinct members of the four-validator roster.
    #[error("private-settlement availability quorum is invalid")]
    InvalidQuorum,
    /// Exact provisional material could not be made durable.
    #[error("private-settlement availability storage failed")]
    Storage,
}

impl From<PrivateSettlementSidecarStoreErrorV1> for PrivateSettlementAvailabilityErrorV1 {
    fn from(_: PrivateSettlementSidecarStoreErrorV1) -> Self {
        Self::Storage
    }
}

/// Runtime-only bounded signer for provisional restricted-DA material.
#[derive(Clone)]
pub struct PrivateSettlementAvailabilitySignerV1 {
    key_pair: KeyPair,
    peer_id: PeerId,
}

impl fmt::Debug for PrivateSettlementAvailabilitySignerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateSettlementAvailabilitySignerV1")
            .field("peer_id", &self.peer_id)
            .finish_non_exhaustive()
    }
}

impl PrivateSettlementAvailabilitySignerV1 {
    /// Retain one node-owned BLS-normal key behind the bounded capability.
    ///
    /// # Errors
    ///
    /// Rejects any non-BLS-normal key.
    pub fn new(key_pair: KeyPair) -> Result<Self, PrivateSettlementAvailabilityErrorV1> {
        if key_pair.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal) {
            return Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner);
        }
        let peer_id = PeerId::from(key_pair.public_key().clone());
        Ok(Self { key_pair, peer_id })
    }

    /// Public identity of the retained node key.
    #[must_use]
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Persist exact material and only then issue its exact availability share.
    ///
    /// The signer must be a member of the supplied four-validator authority.
    /// The private key is never returned or accepted through a request object.
    ///
    /// # Errors
    ///
    /// Rejects invalid/substituted material, a signer outside the roster, or a
    /// durable-store failure.
    pub fn persist_and_sign(
        &self,
        store: &PrivateSettlementFileSidecarStoreV1,
        material: PrivateSettlementProvisionalLegMaterialV1,
        authoritative_height: u64,
    ) -> Result<
        (
            PrivateSettlementSidecarStoreOutcomeV1,
            PrivateSettlementAvailabilityShareV1,
        ),
        PrivateSettlementAvailabilityErrorV1,
    > {
        material
            .validate()
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        validate_authority_cryptography_v1(&material.committee_authority)
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        if !material
            .committee_authority
            .validators
            .iter()
            .any(|validator| validator == &self.peer_id)
        {
            return Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner);
        }

        let body = material.availability_body;
        let authority = material.committee_authority.clone();
        let outcome = store.store_provisional(material, authoritative_height)?;
        // This line is deliberately after the fsync-backed store boundary.
        let signature = Signature::try_new(
            self.key_pair.private_key(),
            &body
                .signature_preimage()
                .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?,
        )
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        let share = PrivateSettlementAvailabilityShareV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            body,
            signer: self.peer_id.clone(),
            signature: signature.payload().to_vec(),
        };
        verify_private_settlement_availability_share_v1(&share, &share.body, &authority)?;
        Ok((outcome, share))
    }

    /// Authenticate one exact restricted auditor response with this node's key.
    ///
    /// Unlike availability signing, this operation does not claim a new
    /// persistence boundary. It can sign only a fully typed, purpose-separated
    /// auditor-view body that names this roster member and the exact authority.
    ///
    /// # Errors
    ///
    /// Rejects a malformed body, an authority mismatch, a signer outside the
    /// four-validator roster, or a body naming another responder.
    pub fn sign_auditor_view(
        &self,
        body: PrivateSettlementAuditorViewAttestationBodyV1,
        authority: &PrivateSettlementCommitteeAuthorityV1,
    ) -> Result<PrivateSettlementAuditorViewAttestationV1, PrivateSettlementAvailabilityErrorV1>
    {
        body.validate_shape()
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        validate_authority_cryptography_v1(authority)
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        let authority_digest = authority
            .digest()
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        if body.responder != self.peer_id
            || body.authority_digest != authority_digest
            || !authority
                .validators
                .iter()
                .any(|validator| validator == &self.peer_id)
        {
            return Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner);
        }
        let signature = Signature::try_new(
            self.key_pair.private_key(),
            &body
                .signature_preimage()
                .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?,
        )
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        let attestation = PrivateSettlementAuditorViewAttestationV1 {
            body,
            signature: signature.payload().to_vec(),
        };
        verify_private_settlement_auditor_view_attestation_v1(
            &attestation,
            &attestation.body,
            authority,
        )?;
        Ok(attestation)
    }

    /// Authenticate one exact durable approval acknowledgement with this node's key.
    ///
    /// # Errors
    ///
    /// Rejects a malformed body, an authority mismatch, a signer outside the
    /// four-validator roster, or a body naming another responder.
    pub fn sign_audit_approval_acknowledgement(
        &self,
        body: PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1,
        authority: &PrivateSettlementCommitteeAuthorityV1,
    ) -> Result<
        PrivateSettlementAuditApprovalAcknowledgementAttestationV1,
        PrivateSettlementAvailabilityErrorV1,
    > {
        body.validate_shape()
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        validate_authority_cryptography_v1(authority)
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        let authority_digest = authority
            .digest()
            .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        if body.responder != self.peer_id
            || body.authority_digest != authority_digest
            || !authority
                .validators
                .iter()
                .any(|validator| validator == &self.peer_id)
        {
            return Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner);
        }
        let signature = Signature::try_new(
            self.key_pair.private_key(),
            &body
                .signature_preimage()
                .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?,
        )
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        let attestation = PrivateSettlementAuditApprovalAcknowledgementAttestationV1 {
            body,
            signature: signature.payload().to_vec(),
        };
        verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
            &attestation,
            &attestation.body,
            authority,
        )?;
        Ok(attestation)
    }
}

/// Verify one exact approval-acknowledgement attestation against a roster.
///
/// # Errors
///
/// Rejects body substitution, a responder outside the exact authority, an
/// invalid proof of possession, or a malformed BLS-normal signature.
pub fn verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
    attestation: &PrivateSettlementAuditApprovalAcknowledgementAttestationV1,
    expected_body: &PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<(), PrivateSettlementAvailabilityErrorV1> {
    attestation
        .validate_shape()
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    validate_authority_cryptography_v1(authority)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    let authority_digest = authority
        .digest()
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    if &attestation.body != expected_body
        || attestation.body.authority_digest != authority_digest
        || !authority
            .validators
            .iter()
            .any(|validator| validator == &attestation.body.responder)
    {
        return Err(PrivateSettlementAvailabilityErrorV1::InvalidShare);
    }
    let signature = Signature::try_from_bytes(&attestation.signature)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    signature
        .verify(
            attestation.body.responder.public_key(),
            &attestation
                .body
                .signature_preimage()
                .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?,
        )
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)
}

/// Verify one exact auditor-view attestation against a four-validator roster.
///
/// # Errors
///
/// Rejects body substitution, a responder outside the exact authority, an
/// invalid proof of possession, or a malformed BLS-normal signature.
pub fn verify_private_settlement_auditor_view_attestation_v1(
    attestation: &PrivateSettlementAuditorViewAttestationV1,
    expected_body: &PrivateSettlementAuditorViewAttestationBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<(), PrivateSettlementAvailabilityErrorV1> {
    attestation
        .validate_shape()
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    validate_authority_cryptography_v1(authority)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    let authority_digest = authority
        .digest()
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    if &attestation.body != expected_body
        || attestation.body.authority_digest != authority_digest
        || !authority
            .validators
            .iter()
            .any(|validator| validator == &attestation.body.responder)
    {
        return Err(PrivateSettlementAvailabilityErrorV1::InvalidShare);
    }
    let signature = Signature::try_from_bytes(&attestation.signature)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    signature
        .verify(
            attestation.body.responder.public_key(),
            &attestation
                .body
                .signature_preimage()
                .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?,
        )
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)
}

/// Verify one exact-body share against an exact four-validator authority.
///
/// # Errors
///
/// Rejects a signer outside the roster, body substitution, malformed PoP, or
/// invalid BLS signature.
pub fn verify_private_settlement_availability_share_v1(
    share: &PrivateSettlementAvailabilityShareV1,
    expected_body: &PrivateSettlementSidecarAvailabilityBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
) -> Result<(), PrivateSettlementAvailabilityErrorV1> {
    share
        .validate_shape()
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    validate_authority_cryptography_v1(authority)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    let authority_digest = authority
        .digest()
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    if &share.body != expected_body
        || share.body.route != authority.route
        || share.body.authority_digest != authority_digest
        || share.signature.len() != PRIVATE_SETTLEMENT_BLS_BYTES_V1
        || !authority
            .validators
            .iter()
            .any(|validator| validator == &share.signer)
    {
        return Err(PrivateSettlementAvailabilityErrorV1::InvalidShare);
    }
    let signature = Signature::try_from_bytes(&share.signature)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    signature
        .verify(
            share.signer.public_key(),
            &share
                .body
                .signature_preimage()
                .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?,
        )
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)
}

/// Aggregate exactly three distinct valid shares in canonical roster order.
///
/// Input order never affects the signer bitmap or aggregate signature.
/// Supplying two shares, four shares, duplicates, or mixed bodies is rejected.
///
/// # Errors
///
/// Returns a redacted quorum/share error for any malformed set.
pub fn aggregate_private_settlement_availability_shares_v1(
    body: PrivateSettlementSidecarAvailabilityBodyV1,
    authority: &PrivateSettlementCommitteeAuthorityV1,
    shares: &[PrivateSettlementAvailabilityShareV1],
) -> Result<PrivateSettlementSidecarAvailabilityV1, PrivateSettlementAvailabilityErrorV1> {
    if shares.len() != usize::from(PRIVATE_SETTLEMENT_COMMITTEE_QUORUM_V1) {
        return Err(PrivateSettlementAvailabilityErrorV1::InvalidQuorum);
    }
    validate_authority_cryptography_v1(authority)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    let mut indexed = BTreeMap::<usize, Vec<u8>>::new();
    for share in shares {
        verify_private_settlement_availability_share_v1(share, &body, authority)?;
        let index = authority
            .validators
            .iter()
            .position(|validator| validator == &share.signer)
            .ok_or(PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
        if indexed.insert(index, share.signature.clone()).is_some() {
            return Err(PrivateSettlementAvailabilityErrorV1::InvalidShare);
        }
    }
    let mut signers_bitmap = 0_u8;
    let signatures = indexed
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap |= 1_u8 << index;
            signature
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    let certificate = PrivateSettlementSidecarAvailabilityV1 {
        body,
        signers_bitmap,
        aggregate_signature,
    };
    verify_private_settlement_availability_certificate_v1(&certificate, authority)
        .map_err(|_| PrivateSettlementAvailabilityErrorV1::InvalidShare)?;
    Ok(certificate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_settlement::sidecar_store::{
        PRIVATE_SETTLEMENT_SIDECAR_DEFAULT_MAX_TOTAL_BYTES_V1,
        PrivateSettlementRestrictedSidecarV1, PrivateSettlementSidecarStoreConfigV1,
        tests::{provisional_material_fixture, sidecar_fixture},
    };
    use iroha_data_model::nexus::PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1;
    use std::{fs, sync::Arc};

    fn stores(
        count: usize,
    ) -> (
        tempfile::TempDir,
        Vec<Arc<PrivateSettlementFileSidecarStoreV1>>,
    ) {
        let temp = tempfile::tempdir().expect("temporary parent");
        let stores = (0..count)
            .map(|index| {
                Arc::new(
                    PrivateSettlementFileSidecarStoreV1::open(
                        temp.path().join(format!("validator-{index}")),
                        PrivateSettlementSidecarStoreConfigV1::default(),
                    )
                    .expect("validator store"),
                )
            })
            .collect();
        (temp, stores)
    }

    #[test]
    fn exact_three_of_four_is_canonical_and_two_four_or_duplicates_are_rejected() {
        let fixture = sidecar_fixture();
        let material = provisional_material_fixture(&fixture);
        let material_bytes = norito::encode_canonical(&material).expect("canonical material");
        assert_eq!(
            norito::decode_canonical::<PrivateSettlementProvisionalLegMaterialV1>(&material_bytes,)
                .expect("material roundtrip"),
            material
        );
        let (_temp, stores) = stores(4);
        let shares = fixture
            .validator_keys
            .iter()
            .zip(&stores)
            .map(|(key, store)| {
                PrivateSettlementAvailabilitySignerV1::new(key.clone())
                    .expect("BLS signer")
                    .persist_and_sign(store, material.clone(), 11)
                    .expect("durable share")
                    .1
            })
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_private_settlement_availability_shares_v1(
                material.availability_body,
                &material.committee_authority,
                &shares[..2],
            ),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidQuorum)
        );
        assert_eq!(
            aggregate_private_settlement_availability_shares_v1(
                material.availability_body,
                &material.committee_authority,
                &shares,
            ),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidQuorum)
        );
        assert_eq!(
            aggregate_private_settlement_availability_shares_v1(
                material.availability_body,
                &material.committee_authority,
                &[shares[0].clone(), shares[0].clone(), shares[1].clone()],
            ),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
        );

        let certificate = aggregate_private_settlement_availability_shares_v1(
            material.availability_body,
            &material.committee_authority,
            &[shares[2].clone(), shares[0].clone(), shares[1].clone()],
        )
        .expect("three canonical shares");
        assert_eq!(certificate.signers_bitmap, 0b0111);
        assert_eq!(certificate, fixture.sidecar.payload.availability);
        let share_bytes = norito::encode_canonical(&shares[0]).expect("canonical share");
        assert_eq!(
            norito::decode_canonical::<PrivateSettlementAvailabilityShareV1>(&share_bytes)
                .expect("share roundtrip"),
            shares[0]
        );
    }

    #[test]
    fn signer_is_roster_bounded_and_share_follows_owner_only_fsync_record() {
        let fixture = sidecar_fixture();
        let material = provisional_material_fixture(&fixture);
        let (temp, stores) = stores(1);
        let outsider = KeyPair::from_seed(vec![0xEE; 32], Algorithm::BlsNormal);
        assert_eq!(
            PrivateSettlementAvailabilitySignerV1::new(outsider)
                .expect("BLS signer")
                .persist_and_sign(&stores[0], material.clone(), 11),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner)
        );
        let (outcome, share) =
            PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[0].clone())
                .expect("roster signer")
                .persist_and_sign(&stores[0], material.clone(), 11)
                .expect("share after durable store");
        assert_eq!(outcome, PrivateSettlementSidecarStoreOutcomeV1::Stored);
        verify_private_settlement_availability_share_v1(
            &share,
            &material.availability_body,
            &material.committee_authority,
        )
        .expect("valid share");
        let records = fs::read_dir(temp.path().join("validator-0"))
            .expect("store directory")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".apv1"))
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1, "share requires one committed APV1 file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                records[0]
                    .metadata()
                    .expect("record metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assert_eq!(stores[0].prune(120).expect("retain through ticket"), 0);
        assert_eq!(stores[0].prune(121).expect("prune after ticket"), 1);
    }

    #[test]
    fn auditor_view_attestation_is_exact_body_and_roster_bound() {
        let fixture = sidecar_fixture();
        let material = provisional_material_fixture(&fixture);
        let signer = PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[0].clone())
            .expect("roster signer");
        let body = PrivateSettlementAuditorViewAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: material.manifest.network_id,
            payload_digest: material.availability_body.payload_digest,
            view_digest: iroha_crypto::Hash::new(b"exact restricted auditor view"),
            authority_digest: material
                .committee_authority
                .digest()
                .expect("authority digest"),
            lifecycle_code: PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
            authoritative_height: 11,
            responder: signer.peer_id().clone(),
        };
        let attestation = signer
            .sign_auditor_view(body.clone(), &material.committee_authority)
            .expect("attested auditor view");
        verify_private_settlement_auditor_view_attestation_v1(
            &attestation,
            &body,
            &material.committee_authority,
        )
        .expect("attestation verifies");

        let mut substituted = body.clone();
        substituted.authoritative_height += 1;
        assert_eq!(
            verify_private_settlement_auditor_view_attestation_v1(
                &attestation,
                &substituted,
                &material.committee_authority,
            ),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
        );

        let other_signer =
            PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[1].clone())
                .expect("second roster signer");
        assert_eq!(
            other_signer.sign_auditor_view(body, &material.committee_authority),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner)
        );
    }

    #[test]
    fn audit_approval_acknowledgement_attestation_is_exact_and_roster_bound() {
        let fixture = sidecar_fixture();
        let material = provisional_material_fixture(&fixture);
        let signer = PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[0].clone())
            .expect("roster signer");
        let body = PrivateSettlementAuditApprovalAcknowledgementAttestationBodyV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            network_id: material.manifest.network_id,
            payload_digest: material.availability_body.payload_digest,
            approval_digest: iroha_crypto::Hash::new(b"exact auditor approval"),
            acknowledgement_digest: iroha_crypto::Hash::new(
                b"exact durable approval acknowledgement",
            ),
            authority_digest: material
                .committee_authority
                .digest()
                .expect("authority digest"),
            lifecycle_code: PRIVATE_SETTLEMENT_LIFECYCLE_COLLECTING_V1,
            authoritative_height: 11,
            responder: signer.peer_id().clone(),
        };
        let attestation = signer
            .sign_audit_approval_acknowledgement(body.clone(), &material.committee_authority)
            .expect("attested acknowledgement");
        verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
            &attestation,
            &body,
            &material.committee_authority,
        )
        .expect("acknowledgement attestation verifies");

        let mut substituted = body.clone();
        substituted.acknowledgement_digest =
            iroha_crypto::Hash::new(b"substituted approval acknowledgement");
        assert_eq!(
            verify_private_settlement_audit_approval_acknowledgement_attestation_v1(
                &attestation,
                &substituted,
                &material.committee_authority,
            ),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidShare)
        );

        let other_signer =
            PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[1].clone())
                .expect("second roster signer");
        assert_eq!(
            other_signer.sign_audit_approval_acknowledgement(body, &material.committee_authority),
            Err(PrivateSettlementAvailabilityErrorV1::InvalidSigner)
        );
    }

    #[test]
    fn provisional_retry_restart_substitution_and_promotion_are_fail_closed() {
        let fixture = sidecar_fixture();
        let material = provisional_material_fixture(&fixture);
        let temp = tempfile::tempdir().expect("temporary parent");
        let root = temp.path().join("restart-store");
        let config = PrivateSettlementSidecarStoreConfigV1::new(
            1,
            PRIVATE_SETTLEMENT_SIDECAR_DEFAULT_MAX_TOTAL_BYTES_V1,
        )
        .expect("single-record capacity");
        let signer = PrivateSettlementAvailabilitySignerV1::new(fixture.validator_keys[0].clone())
            .expect("signer");
        let store = PrivateSettlementFileSidecarStoreV1::open(&root, config).expect("store");
        let first = signer
            .persist_and_sign(&store, material.clone(), 11)
            .expect("first share");
        let duplicate = signer
            .persist_and_sign(&store, material.clone(), 12)
            .expect("idempotent share");
        assert_eq!(
            duplicate.0,
            PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored
        );
        assert_eq!(duplicate.1, first.1);
        let provisional_path = fs::read_dir(&root)
            .expect("provisional files")
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().ends_with(".apv1"))
            .expect("provisional record")
            .path();
        let provisional_backup = temp.path().join("provisional-backup");
        fs::copy(&provisional_path, &provisional_backup).expect("backup provisional record");

        let mut substituted = material.clone();
        substituted.availability_body.retention_until_height += 1;
        assert_eq!(
            signer.persist_and_sign(&store, substituted, 12),
            Err(PrivateSettlementAvailabilityErrorV1::Storage)
        );
        drop(store);

        let reopened = PrivateSettlementFileSidecarStoreV1::open(&root, config)
            .expect("restart reconciliation");
        assert_eq!(
            signer
                .persist_and_sign(&reopened, material.clone(), 13)
                .expect("restart idempotency")
                .0,
            PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored
        );
        assert_eq!(
            reopened
                .promote(fixture.sidecar.clone())
                .expect("exact certificate promotion"),
            PrivateSettlementSidecarStoreOutcomeV1::Stored
        );
        assert_eq!(
            reopened
                .promote(fixture.sidecar.clone())
                .expect("promotion retry"),
            PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored
        );
        drop(reopened);
        fs::copy(&provisional_backup, &provisional_path)
            .expect("simulate crash after final rename before provisional removal");
        let reconciled = PrivateSettlementFileSidecarStoreV1::open(&root, config)
            .expect("matching provisional/final crash pair reconciles");
        assert!(
            fs::read_dir(&root)
                .expect("store files")
                .filter_map(Result::ok)
                .all(|entry| !entry.file_name().to_string_lossy().ends_with(".apv1"))
        );
        drop(reconciled);
    }

    #[test]
    fn malformed_aggregate_and_final_material_substitution_are_rejected() {
        let fixture = sidecar_fixture();
        let material = provisional_material_fixture(&fixture);
        let (_temp, stores) = stores(3);
        let shares = fixture.validator_keys[..3]
            .iter()
            .zip(&stores)
            .map(|(key, store)| {
                PrivateSettlementAvailabilitySignerV1::new(key.clone())
                    .expect("signer")
                    .persist_and_sign(store, material.clone(), 11)
                    .expect("share")
                    .1
            })
            .collect::<Vec<_>>();
        let mut malformed = aggregate_private_settlement_availability_shares_v1(
            material.availability_body,
            &material.committee_authority,
            &shares,
        )
        .expect("certificate");
        malformed.aggregate_signature[0] ^= 1;
        assert!(
            verify_private_settlement_availability_certificate_v1(
                &malformed,
                &material.committee_authority,
            )
            .is_err()
        );

        let mut substituted = fixture.sidecar.clone();
        substituted.payload.availability.body.retention_until_height += 1;
        assert_eq!(
            stores[0].promote(substituted),
            Err(PrivateSettlementSidecarStoreErrorV1::InvalidSidecar)
        );
        stores[0]
            .promote(PrivateSettlementRestrictedSidecarV1 {
                ..fixture.sidecar.clone()
            })
            .expect("exact final material remains promotable");
    }
}
