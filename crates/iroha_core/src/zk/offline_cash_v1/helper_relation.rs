//! Private field-neutral relation for Offline Cash V1 hardware helpers.
//!
//! This module fixes the common `GuardUse`, `PlatformBind`, normalized Android
//! `KeyCert`, and `GuardBundle` statement before recursive proof verification is
//! available. It performs exact host-side P-256 validation only to reject bad
//! inputs before circuit construction. The resulting owner is deliberately
//! private, move-only, and non-authorizing: no production verifier consumes it,
//! and the binding circuits do not claim to prove the P-256 checks.

use core::fmt;

use iroha_data_model::offline::{KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2};
use p256::ecdsa::{
    signature::hazmat::PrehashVerifier as _, Signature as P256Signature,
    VerifyingKey as P256VerifyingKey,
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize;

use super::{
    helper_abi::{
        OfflineCashHelperAbiErrorV1, OfflineCashHelperOperationV1,
        OfflineCashHelperPublicInstancesV1, OfflineCashHelperStatementV1,
    },
    protocol::OfflineCashHalo2CircuitRoleV1,
    OfflineCashHalo2ParityV1,
};

const CURRENT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:current-guard";
const NEXT_GUARD_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:next-guard";
const PLATFORM_MESSAGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-message";
const GUARD_USE_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-use-claim";
const PLATFORM_BIND_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:platform-bind-claim";
const ANDROID_KEY_CERT_CLAIM_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:android-key-cert-claim";
const GUARD_BUNDLE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:helper:guard-bundle";
const P256_ALGORITHM: &[u8] = b"ecdsa-p256-sha256";
const ANDROID_KEY_ORIGIN: &[u8] = b"generated-in-keymint-hardware";
const ANDROID_KEY_PURPOSE: &[u8] = b"sign";
const ANDROID_DIGEST_MODE: &[u8] = b"sha-256";
const ANDROID_USAGE_LIMIT_ONE: [u8; 4] = 1_u32.to_le_bytes();
const FRAMED_MESSAGE_MAX_BYTES: usize = 1_024;

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
/// governed-root membership constraints remain a recursive helper blocker.
#[must_use]
pub(super) struct OfflineCashAndroidKeyCertWitnessV1 {
    issuer_public_key_sec1: [u8; 65],
    certificate_signature_raw: [u8; 64],
    certificate_digest: [u8; 32],
    tbs_digest: [u8; 32],
    attestation_digest: [u8; 32],
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
    }
}

impl OfflineCashAndroidKeyCertWitnessV1 {
    /// Construct normalized KeyCert inputs from already canonical typed values.
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
        })
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

/// Move-only, non-authorizing owner of one fully checked field-neutral relation.
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
