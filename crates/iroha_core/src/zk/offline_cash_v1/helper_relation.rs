//! Private field-neutral relation for Offline Cash V1 hardware helpers.
//!
//! This module fixes the common `GuardUse`, `PlatformBind`, normalized Android
//! `KeyCert`, and `GuardBundle` statement for authenticated proof composition.
//! It performs exact P-256 preflight to reject bad inputs before circuit
//! construction, retains fixed-size zeroizing evidence for
//! constrained SHA-256 synthesis, and emits one-shot exact P-256 V3 child
//! statements. The resulting private, move-only owner feeds the authenticated
//! helper proof and exact child-composition boundary.

use core::fmt;

use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::offline::{
    KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, OfflineCashAuthenticatedReleaseV1,
    OfflineCashInternalValidationReceiptV1, OfflineDeviceAttestationPolicyViewV1,
    OfflineDeviceAttestationRegistration, OfflineDeviceEligibilityCredentialV1,
};
use p256::ecdsa::{
    Signature as P256Signature, VerifyingKey as P256VerifyingKey,
    signature::hazmat::PrehashVerifier as _,
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize;

use crate::smartcontracts::isi::offline::isi::offline_cash_android_keymint_fixed_source_v1;

use super::{
    OfflineCashHalo2ParityV1, P256PackedStatementSourceV3,
    helper_abi::{
        OfflineCashHelperAbiErrorV1, OfflineCashHelperOperationV1,
        OfflineCashHelperPublicInstancesV1, OfflineCashHelperStatementV1,
    },
    protocol::OfflineCashHalo2CircuitRoleV1,
};

const CURRENT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:current-guard";
const NEXT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:next-guard";
const PLATFORM_MESSAGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-message";
const GUARD_USE_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-use-claim";
const PLATFORM_BIND_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-bind-claim";
const ANDROID_KEY_CERT_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:android-key-cert-claim";
const ANDROID_KEYMINT_FIXED_SOURCE_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:helper:android-keymint-fixed-source";
const GUARD_BUNDLE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-bundle";
const P256_ALGORITHM: &[u8] = b"ecdsa-p256-sha256";
const ANDROID_KEY_ORIGIN: &[u8] = b"generated-in-keymint-hardware";
const ANDROID_KEY_PURPOSE: &[u8] = b"sign";
const ANDROID_DIGEST_MODE: &[u8] = b"sha-256";
const ANDROID_USAGE_LIMIT_ONE: [u8; 4] = 1_u32.to_le_bytes();
const FRAMED_MESSAGE_MAX_BYTES: usize = 1_024;
const P256_CHILD_STATEMENT_BYTES_V3: usize = 65 + 32 + 64;

/// Public values from one prepared monetary transition before helper claims are derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct OfflineCashHelperRelationInputV1 {
    pub(super) operation: OfflineCashHelperOperationV1,
    pub(super) release_id: [u8; 32],
    pub(super) context_digest: [u8; 32],
    pub(super) current_head: [u8; 32],
    pub(super) current_lineage_digest: [u8; 32],
    pub(super) transition_digest: [u8; 32],
    pub(super) wallet_binding: [u8; 32],
    pub(super) hardware_policy_id: [u8; 32],
    pub(super) guard_device_id: [u8; 32],
    pub(super) from_sequence: u64,
    pub(super) to_sequence: u64,
}

impl OfflineCashHelperRelationInputV1 {
    fn validate(self) -> Result<(), OfflineCashHelperAbiErrorV1> {
        if [
            self.release_id,
            self.context_digest,
            self.current_head,
            self.current_lineage_digest,
            self.transition_digest,
            self.wallet_binding,
            self.hardware_policy_id,
            self.guard_device_id,
        ]
        .into_iter()
        .any(|digest| digest == [0; 32])
            || self.current_head == self.transition_digest
            || self.from_sequence.checked_add(1) != Some(self.to_sequence)
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        Ok(())
    }
}

/// Normalized Android certificate facts retained by the private helper relation.
///
/// The signature is the canonical low-S raw P-256 signature over the exact
/// SHA-256 digest of DER `TBSCertificate`. The certificate and attestation
/// digests are separately bound into the claim. Full DER/KeyMint-extension and
/// governed-root membership are established by `from_governed_keymint` before
/// these fixed fields can enter a production build.
#[must_use]
pub(super) struct OfflineCashAndroidKeyCertWitnessV1 {
    issuer_public_key_sec1: [u8; 65],
    certificate_signature_raw: [u8; 64],
    certificate_digest: [u8; 32],
    tbs_digest: [u8; 32],
    attestation_digest: [u8; 32],
    governance: OfflineCashAndroidKeyCertGovernanceV1,
}

enum OfflineCashAndroidKeyCertGovernanceV1 {
    Governed {
        release_id: [u8; 32],
        hardware_policy_digest: [u8; 32],
    },
    #[cfg(test)]
    Synthetic,
}

impl fmt::Debug for OfflineCashAndroidKeyCertWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashAndroidKeyCertWitnessV1")
            .field("certificate_digest", &self.certificate_digest)
            .field("tbs_digest", &self.tbs_digest)
            .field("key_and_signature", &"[REDACTED]")
            .finish()
    }
}

impl Drop for OfflineCashAndroidKeyCertWitnessV1 {
    fn drop(&mut self) {
        self.issuer_public_key_sec1.zeroize();
        self.certificate_signature_raw.zeroize();
        self.certificate_digest.zeroize();
        self.tbs_digest.zeroize();
        self.attestation_digest.zeroize();
        match &mut self.governance {
            OfflineCashAndroidKeyCertGovernanceV1::Governed {
                release_id,
                hardware_policy_digest,
            } => {
                release_id.zeroize();
                hardware_policy_digest.zeroize();
            }
            #[cfg(test)]
            OfflineCashAndroidKeyCertGovernanceV1::Synthetic => {}
        }
    }
}

impl OfflineCashAndroidKeyCertWitnessV1 {
    /// Construct normalized KeyCert inputs from already canonical typed values.
    #[cfg(test)]
    pub(super) fn new(
        issuer_public_key: KagemushaDevicePublicKeyV2,
        certificate_signature: KagemushaDeviceSignatureV2,
        certificate_digest: [u8; 32],
        tbs_digest: [u8; 32],
        attestation_digest: [u8; 32],
    ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
        issuer_public_key
            .validate()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        certificate_signature
            .validate()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        if [certificate_digest, tbs_digest, attestation_digest]
            .into_iter()
            .any(|digest| digest == [0; 32])
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        Ok(Self {
            issuer_public_key_sec1: *issuer_public_key.as_sec1_bytes(),
            certificate_signature_raw: *certificate_signature.as_raw_bytes(),
            certificate_digest,
            tbs_digest,
            attestation_digest,
            governance: OfflineCashAndroidKeyCertGovernanceV1::Synthetic,
        })
    }

    #[cfg(test)]
    pub(super) fn bind_governance_for_test(
        mut self,
        release_id: [u8; 32],
        hardware_policy_digest: [u8; 32],
    ) -> Self {
        self.governance = OfflineCashAndroidKeyCertGovernanceV1::Governed {
            release_id,
            hardware_policy_digest,
        };
        self
    }

    /// Map one consensus-admitted KeyMint registration through the finalized
    /// hardware policy and threshold-authenticated offline-cash release.
    ///
    /// The native mapper performs bounded CBOR/DER parsing, full KeyMint and
    /// X.509 validation, revocation/status/root checks, and canonical low-S
    /// P-256 projection. The signed eligibility credential binds the exact
    /// canonical registration to the same finalized policy. Missing release
    /// receipt or review identities fail before any fixed source is emitted.
    pub(super) fn from_governed_keymint(
        registration: &OfflineDeviceAttestationRegistration,
        eligibility_credential: &OfflineDeviceEligibilityCredentialV1,
        expected_credential_issuer: &PublicKey,
        policy_view: &OfflineDeviceAttestationPolicyViewV1,
        authenticated_release: &OfflineCashAuthenticatedReleaseV1,
        validation_receipt: &OfflineCashInternalValidationReceiptV1,
        evaluation_time_ms: u64,
    ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
        let receipt_digest = validation_receipt
            .canonical_digest()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        if receipt_digest != authenticated_release.receipt_digest()
            || validation_receipt.profile_digest != authenticated_release.profile_digest()
            || policy_view.policy_hash != validation_receipt.hardware_policy_digest
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        let policy = policy_view
            .validated_policy_v1(evaluation_time_ms)
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        eligibility_credential
            .verify_against_policy_view_v1(
                expected_credential_issuer,
                policy_view,
                evaluation_time_ms,
            )
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        let registration_bytes = norito::encode_canonical(registration)
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        let registration_hash: [u8; 32] = Hash::new(&registration_bytes).into();
        let claims = &eligibility_credential.payload;
        if claims.registration_hash != registration_hash
            || claims.device_id != registration.device_id
            || claims.attestation_key_id != registration.key_id
            || claims.account_id != registration.account_id
            || claims.device_public_key != registration.public_key
            || claims.assertion_public_key != registration.assertion_public_key
        {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        let fixed_source =
            offline_cash_android_keymint_fixed_source_v1(registration, &policy, evaluation_time_ms)
                .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        let issuer_public_key =
            KagemushaDevicePublicKeyV2::from_sec1_bytes(&fixed_source.issuer_public_key_sec1)
                .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        let certificate_signature =
            KagemushaDeviceSignatureV2::from_raw_bytes(&fixed_source.certificate_signature_raw)
                .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        let attestation_digest = framed_digest(
            ANDROID_KEYMINT_FIXED_SOURCE_DOMAIN,
            &[
                &authenticated_release.release_id(),
                &policy_view.policy_hash,
                &registration_hash,
                &fixed_source.trusted_root_der_sha256,
                &fixed_source.attestation_report_sha256,
            ],
        );
        let witness = Self {
            issuer_public_key_sec1: *issuer_public_key.as_sec1_bytes(),
            certificate_signature_raw: *certificate_signature.as_raw_bytes(),
            certificate_digest: fixed_source.certificate_sha256,
            tbs_digest: fixed_source.tbs_certificate_sha256,
            attestation_digest,
            governance: OfflineCashAndroidKeyCertGovernanceV1::Governed {
                release_id: authenticated_release.release_id(),
                hardware_policy_digest: policy_view.policy_hash,
            },
        };
        witness.verify_signature()?;
        Ok(witness)
    }

    fn validate_governance_binding(
        &self,
        input: &OfflineCashHelperRelationInputV1,
    ) -> Result<(), OfflineCashHelperAbiErrorV1> {
        match &self.governance {
            OfflineCashAndroidKeyCertGovernanceV1::Governed {
                release_id,
                hardware_policy_digest,
            } if release_id == &input.release_id
                && hardware_policy_digest == &input.hardware_policy_id =>
            {
                Ok(())
            }
            #[cfg(test)]
            OfflineCashAndroidKeyCertGovernanceV1::Synthetic => Ok(()),
            _ => Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness),
        }
    }

    fn verify_signature(&self) -> Result<(), OfflineCashHelperAbiErrorV1> {
        let key = P256VerifyingKey::from_sec1_bytes(&self.issuer_public_key_sec1)
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        if key.to_encoded_point(false).as_bytes() != self.issuer_public_key_sec1 {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        let signature = P256Signature::from_slice(&self.certificate_signature_raw)
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        if signature.normalize_s().is_some() {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        key.verify_prehash(&self.tbs_digest, &signature)
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)
    }
}

#[must_use]
struct OfflineCashHelperPrivateWitnessV1 {
    platform_public_key_sec1: [u8; 65],
    platform_signature_raw: [u8; 64],
    android_key_cert: Option<OfflineCashAndroidKeyCertWitnessV1>,
}

impl fmt::Debug for OfflineCashHelperPrivateWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashHelperPrivateWitnessV1")
            .field("platform_key_and_signature", &"[REDACTED]")
            .field("has_android_key_cert", &self.android_key_cert.is_some())
            .finish()
    }
}

impl Drop for OfflineCashHelperPrivateWitnessV1 {
    fn drop(&mut self) {
        self.platform_public_key_sec1.zeroize();
        self.platform_signature_raw.zeroize();
    }
}

/// Fixed-size, circuit-owned copy of the private bytes owned by one
/// role-specialized helper leaf.
///
/// There are no variable-size fields and every retained byte is zeroized on
/// drop. Canonical signatures remain in the relation's separate one-shot V3
/// statement source and are never introduced as unconstrained circuit advice.
/// Recursive composition binds the exact SEC1 key and prehash (97 bytes) but
/// intentionally treats any canonical low-S P-256 signature by that key over
/// that prehash as the same authorization statement; raw signature identity is
/// not part of the helper semantic ABI.
#[must_use]
pub(super) struct OfflineCashHelperCircuitWitnessV1 {
    pub(super) platform_public_key_sec1: Option<[u8; 65]>,
    pub(super) android_issuer_public_key_sec1: Option<[u8; 65]>,
}

impl fmt::Debug for OfflineCashHelperCircuitWitnessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashHelperCircuitWitnessV1")
            .field("platform_evidence", &"[REDACTED]")
            .field(
                "has_android_key_cert_evidence",
                &self.android_issuer_public_key_sec1.is_some(),
            )
            .finish()
    }
}

impl Drop for OfflineCashHelperCircuitWitnessV1 {
    fn drop(&mut self) {
        if let Some(key) = self.platform_public_key_sec1.as_mut() {
            key.zeroize();
        }
        if let Some(key) = self.android_issuer_public_key_sec1.as_mut() {
            key.zeroize();
        }
    }
}

/// One-shot, exact `[SEC1 | SHA-256 prehash | P1363 r || s]` source for the
/// private packed-affine V3 child circuit.
///
/// The source is closed at 161 bytes, cannot be cloned, rejects a second read,
/// and zeroizes its retained frame after the first successful transfer and on
/// drop. It deliberately exposes no proof or acceptance method.
#[must_use]
pub(super) struct OfflineCashP256ChildStatementSourceV3 {
    frame: [u8; P256_CHILD_STATEMENT_BYTES_V3],
    consumed: bool,
}

impl fmt::Debug for OfflineCashP256ChildStatementSourceV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashP256ChildStatementSourceV3")
            .field("frame", &"[REDACTED]")
            .field("consumed", &self.consumed)
            .finish()
    }
}

impl Drop for OfflineCashP256ChildStatementSourceV3 {
    fn drop(&mut self) {
        self.frame.zeroize();
    }
}

impl OfflineCashP256ChildStatementSourceV3 {
    fn new(key: &[u8; 65], digest: &[u8; 32], signature: &[u8; 64]) -> Self {
        let mut frame = [0_u8; P256_CHILD_STATEMENT_BYTES_V3];
        frame[..65].copy_from_slice(key);
        frame[65..97].copy_from_slice(digest);
        frame[97..].copy_from_slice(signature);
        Self {
            frame,
            consumed: false,
        }
    }
}

impl P256PackedStatementSourceV3 for OfflineCashP256ChildStatementSourceV3 {
    fn read_exact_statement(
        &mut self,
        destination: &mut [u8; P256_CHILD_STATEMENT_BYTES_V3],
    ) -> Result<(), &'static str> {
        if self.consumed {
            return Err("Offline Cash P-256 child statement source was already consumed");
        }
        destination.copy_from_slice(&self.frame);
        self.frame.zeroize();
        self.consumed = true;
        Ok(())
    }
}

/// Move-only owner of one fully checked field-neutral relation.
#[must_use]
pub(super) struct OfflineCashValidatedHelperRelationV1 {
    statement: OfflineCashHelperStatementV1,
    private_witness: OfflineCashHelperPrivateWitnessV1,
}

impl fmt::Debug for OfflineCashValidatedHelperRelationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OfflineCashValidatedHelperRelationV1")
            .field("operation", &self.statement.operation)
            .field("from_sequence", &self.statement.from_sequence)
            .field("to_sequence", &self.statement.to_sequence)
            .field("private_witness", &"[REDACTED]")
            .finish()
    }
}

impl OfflineCashValidatedHelperRelationV1 {
    /// Validate and bind the exact helper statement without granting acceptance authority.
    pub(super) fn new(
        input: OfflineCashHelperRelationInputV1,
        platform_public_key: KagemushaDevicePublicKeyV2,
        platform_signature: KagemushaDeviceSignatureV2,
        android_key_cert: Option<OfflineCashAndroidKeyCertWitnessV1>,
    ) -> Result<Self, OfflineCashHelperAbiErrorV1> {
        input.validate()?;
        platform_public_key
            .validate()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        platform_signature
            .validate()
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;

        let (current_guard_binding, next_guard_binding) = guard_bindings_v1(&input);
        let platform_message =
            platform_message_v1(&input, &current_guard_binding, &next_guard_binding)?;
        platform_signature
            .verify(&platform_public_key, &platform_message)
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
        let platform_key_digest: [u8; 32] =
            Sha256::digest(platform_public_key.as_sec1_bytes()).into();
        let platform_message_digest: [u8; 32] = Sha256::digest(&platform_message).into();
        let guard_use_claim_digest = guard_use_claim_v1(
            &input,
            &current_guard_binding,
            &next_guard_binding,
            &platform_message_digest,
        );
        let platform_bind_claim_digest = platform_bind_claim_v1(
            &input,
            &current_guard_binding,
            &next_guard_binding,
            &platform_key_digest,
            &platform_message_digest,
        );

        let (
            android_certificate_digest,
            android_tbs_digest,
            android_issuer_key_digest,
            android_attestation_digest,
            android_key_cert_claim_digest,
        ) = if let Some(android) = android_key_cert.as_ref() {
            android.validate_governance_binding(&input)?;
            android.verify_signature()?;
            let issuer_key_digest: [u8; 32] = Sha256::digest(android.issuer_public_key_sec1).into();
            let claim = android_key_cert_claim_v1(
                &input,
                &platform_key_digest,
                &android.certificate_digest,
                &android.tbs_digest,
                &issuer_key_digest,
                &android.attestation_digest,
            );
            (
                android.certificate_digest,
                android.tbs_digest,
                issuer_key_digest,
                android.attestation_digest,
                claim,
            )
        } else {
            ([0; 32], [0; 32], [0; 32], [0; 32], [0; 32])
        };
        let android_key_cert_present = android_key_cert.is_some();
        let guard_bundle_digest = guard_bundle_v1(
            &input,
            &current_guard_binding,
            &next_guard_binding,
            &guard_use_claim_digest,
            &platform_bind_claim_digest,
            android_key_cert_present,
            &android_key_cert_claim_digest,
        );
        let statement = OfflineCashHelperStatementV1 {
            operation: input.operation,
            android_key_cert_present,
            from_sequence: input.from_sequence,
            to_sequence: input.to_sequence,
            release_id: input.release_id,
            context_digest: input.context_digest,
            current_head: input.current_head,
            current_lineage_digest: input.current_lineage_digest,
            transition_digest: input.transition_digest,
            wallet_binding: input.wallet_binding,
            hardware_policy_id: input.hardware_policy_id,
            guard_device_id: input.guard_device_id,
            current_guard_binding,
            next_guard_binding,
            platform_key_digest,
            platform_message_digest,
            guard_use_claim_digest,
            platform_bind_claim_digest,
            android_certificate_digest,
            android_tbs_digest,
            android_issuer_key_digest,
            android_attestation_digest,
            android_key_cert_claim_digest,
            guard_bundle_digest,
        };
        // Route through the ABI constructor once so its independently maintained
        // structural checks cannot drift behind the private relation.
        let _ = OfflineCashHelperPublicInstancesV1::new(
            statement,
            OfflineCashHalo2ParityV1::Eq,
            OfflineCashHalo2CircuitRoleV1::GuardUse,
        )?;
        Ok(Self {
            statement,
            private_witness: OfflineCashHelperPrivateWitnessV1 {
                platform_public_key_sec1: *platform_public_key.as_sec1_bytes(),
                platform_signature_raw: *platform_signature.as_raw_bytes(),
                android_key_cert,
            },
        })
    }

    /// Encode one exact role/parity ABI from this checked owner.
    pub(super) fn public_instances(
        &self,
        parity: OfflineCashHalo2ParityV1,
        role: OfflineCashHalo2CircuitRoleV1,
    ) -> Result<OfflineCashHelperPublicInstancesV1, OfflineCashHelperAbiErrorV1> {
        OfflineCashHelperPublicInstancesV1::new(self.statement, parity, role)
    }

    /// Copy the fixed private evidence into one circuit-owned, zeroizing owner.
    ///
    /// `AndroidKeyCert` retains fixed recursion geometry when Android is absent:
    /// it owns a zero issuer witness and its SHA bindings are gated by the
    /// in-circuit common `present` bit.
    pub(super) fn circuit_witness(
        &self,
        role: OfflineCashHalo2CircuitRoleV1,
    ) -> Result<OfflineCashHelperCircuitWitnessV1, OfflineCashHelperAbiErrorV1> {
        if matches!(
            role,
            OfflineCashHalo2CircuitRoleV1::State
                | OfflineCashHalo2CircuitRoleV1::StateLeaf
                | OfflineCashHalo2CircuitRoleV1::GuardBundle
                | OfflineCashHalo2CircuitRoleV1::P256V3
        ) {
            return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
        }
        Ok(OfflineCashHelperCircuitWitnessV1 {
            platform_public_key_sec1: (role == OfflineCashHalo2CircuitRoleV1::PlatformBind)
                .then_some(self.private_witness.platform_public_key_sec1),
            android_issuer_public_key_sec1: if role == OfflineCashHalo2CircuitRoleV1::AndroidKeyCert
            {
                self.private_witness
                    .android_key_cert
                    .as_ref()
                    .map(|android| android.issuer_public_key_sec1)
            } else {
                None
            },
        })
    }

    /// Exact canonical low-S platform-signature statement for the private V3
    /// child circuit. The separate authenticated proof boundary owns verifier
    /// selection and proof acceptance.
    pub(super) fn platform_p256_child_statement_v3(&self) -> OfflineCashP256ChildStatementSourceV3 {
        OfflineCashP256ChildStatementSourceV3::new(
            &self.private_witness.platform_public_key_sec1,
            &self.statement.platform_message_digest,
            &self.private_witness.platform_signature_raw,
        )
    }

    /// Exact canonical low-S Android certificate-signature statement for the
    /// private V3 child circuit, when normalized certificate evidence exists.
    pub(super) fn android_p256_child_statement_v3(
        &self,
    ) -> Option<OfflineCashP256ChildStatementSourceV3> {
        self.private_witness
            .android_key_cert
            .as_ref()
            .map(|android| {
                OfflineCashP256ChildStatementSourceV3::new(
                    &android.issuer_public_key_sec1,
                    &android.tbs_digest,
                    &android.certificate_signature_raw,
                )
            })
    }

    #[cfg(test)]
    pub(super) const fn statement_for_test(&self) -> OfflineCashHelperStatementV1 {
        self.statement
    }

    #[cfg(test)]
    pub(super) fn private_witness_is_retained_for_test(&self) -> bool {
        self.private_witness.platform_public_key_sec1 != [0; 65]
            && self.private_witness.platform_signature_raw != [0; 64]
    }
}

fn framed_bytes(domain: &[u8], fields: &[&[u8]]) -> Result<Vec<u8>, OfflineCashHelperAbiErrorV1> {
    let payload_len = 8_usize
        .checked_add(domain.len())
        .and_then(|length| {
            fields.iter().try_fold(length, |length, field| {
                length.checked_add(8)?.checked_add(field.len())
            })
        })
        .ok_or(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
    if payload_len > FRAMED_MESSAGE_MAX_BYTES {
        return Err(OfflineCashHelperAbiErrorV1::InvalidPrivateWitness);
    }
    let mut message = Vec::new();
    message
        .try_reserve_exact(payload_len)
        .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?;
    message.extend_from_slice(
        &u64::try_from(domain.len())
            .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?
            .to_le_bytes(),
    );
    message.extend_from_slice(domain);
    for field in fields {
        message.extend_from_slice(
            &u64::try_from(field.len())
                .map_err(|_| OfflineCashHelperAbiErrorV1::InvalidPrivateWitness)?
                .to_le_bytes(),
        );
        message.extend_from_slice(field);
    }
    debug_assert_eq!(message.len(), payload_len);
    Ok(message)
}

fn framed_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(domain.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(domain);
    for field in fields {
        hasher.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(field);
    }
    hasher.finalize().into()
}

pub(super) fn guard_bindings_v1(input: &OfflineCashHelperRelationInputV1) -> ([u8; 32], [u8; 32]) {
    let operation = [input.operation as u8];
    let from_sequence = input.from_sequence.to_le_bytes();
    let to_sequence = input.to_sequence.to_le_bytes();
    let current = framed_digest(
        CURRENT_GUARD_DOMAIN,
        &[
            &operation,
            &input.release_id,
            &input.context_digest,
            &input.current_head,
            &input.current_lineage_digest,
            &input.wallet_binding,
            &input.hardware_policy_id,
            &input.guard_device_id,
            &from_sequence,
        ],
    );
    let next = framed_digest(
        NEXT_GUARD_DOMAIN,
        &[
            &operation,
            &input.release_id,
            &input.context_digest,
            &input.current_head,
            &input.current_lineage_digest,
            &input.transition_digest,
            &input.wallet_binding,
            &input.hardware_policy_id,
            &input.guard_device_id,
            &current,
            &to_sequence,
        ],
    );
    (current, next)
}

pub(super) fn platform_message_v1(
    input: &OfflineCashHelperRelationInputV1,
    current_guard_binding: &[u8; 32],
    next_guard_binding: &[u8; 32],
) -> Result<Vec<u8>, OfflineCashHelperAbiErrorV1> {
    let operation = [input.operation as u8];
    let from_sequence = input.from_sequence.to_le_bytes();
    let to_sequence = input.to_sequence.to_le_bytes();
    framed_bytes(
        PLATFORM_MESSAGE_DOMAIN,
        &[
            &operation,
            &input.release_id,
            &input.context_digest,
            &input.current_head,
            &input.current_lineage_digest,
            &input.transition_digest,
            &input.wallet_binding,
            &input.hardware_policy_id,
            &input.guard_device_id,
            current_guard_binding,
            next_guard_binding,
            &from_sequence,
            &to_sequence,
        ],
    )
}

fn guard_use_claim_v1(
    input: &OfflineCashHelperRelationInputV1,
    current_guard_binding: &[u8; 32],
    next_guard_binding: &[u8; 32],
    platform_message_digest: &[u8; 32],
) -> [u8; 32] {
    let operation = [input.operation as u8];
    let from_sequence = input.from_sequence.to_le_bytes();
    let to_sequence = input.to_sequence.to_le_bytes();
    framed_digest(
        GUARD_USE_CLAIM_DOMAIN,
        &[
            &operation,
            &input.release_id,
            &input.context_digest,
            &input.current_head,
            &input.current_lineage_digest,
            &input.transition_digest,
            &input.wallet_binding,
            &input.hardware_policy_id,
            &input.guard_device_id,
            current_guard_binding,
            next_guard_binding,
            &from_sequence,
            &to_sequence,
            platform_message_digest,
        ],
    )
}

fn platform_bind_claim_v1(
    input: &OfflineCashHelperRelationInputV1,
    current_guard_binding: &[u8; 32],
    next_guard_binding: &[u8; 32],
    platform_key_digest: &[u8; 32],
    platform_message_digest: &[u8; 32],
) -> [u8; 32] {
    framed_digest(
        PLATFORM_BIND_CLAIM_DOMAIN,
        &[
            &input.release_id,
            &input.hardware_policy_id,
            &input.wallet_binding,
            &input.guard_device_id,
            platform_key_digest,
            platform_message_digest,
            current_guard_binding,
            next_guard_binding,
        ],
    )
}

fn android_key_cert_claim_v1(
    input: &OfflineCashHelperRelationInputV1,
    platform_key_digest: &[u8; 32],
    certificate_digest: &[u8; 32],
    tbs_digest: &[u8; 32],
    issuer_key_digest: &[u8; 32],
    attestation_digest: &[u8; 32],
) -> [u8; 32] {
    framed_digest(
        ANDROID_KEY_CERT_CLAIM_DOMAIN,
        &[
            &input.release_id,
            &input.hardware_policy_id,
            &input.guard_device_id,
            platform_key_digest,
            certificate_digest,
            tbs_digest,
            issuer_key_digest,
            attestation_digest,
            P256_ALGORITHM,
            ANDROID_KEY_ORIGIN,
            ANDROID_KEY_PURPOSE,
            ANDROID_DIGEST_MODE,
            &ANDROID_USAGE_LIMIT_ONE,
        ],
    )
}

fn guard_bundle_v1(
    input: &OfflineCashHelperRelationInputV1,
    current_guard_binding: &[u8; 32],
    next_guard_binding: &[u8; 32],
    guard_use_claim_digest: &[u8; 32],
    platform_bind_claim_digest: &[u8; 32],
    android_present: bool,
    android_key_cert_claim_digest: &[u8; 32],
) -> [u8; 32] {
    let operation = [input.operation as u8];
    let android_present = [u8::from(android_present)];
    let from_sequence = input.from_sequence.to_le_bytes();
    let to_sequence = input.to_sequence.to_le_bytes();
    framed_digest(
        GUARD_BUNDLE_DOMAIN,
        &[
            &operation,
            &android_present,
            &input.release_id,
            &input.context_digest,
            &input.current_head,
            &input.current_lineage_digest,
            &input.transition_digest,
            &input.wallet_binding,
            &input.hardware_policy_id,
            &input.guard_device_id,
            current_guard_binding,
            next_guard_binding,
            &from_sequence,
            &to_sequence,
            guard_use_claim_digest,
            platform_bind_claim_digest,
            android_key_cert_claim_digest,
        ],
    )
}
