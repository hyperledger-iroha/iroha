//! Complete original enrollment mathematics at an immutable financial proof interval.
//!
//! This scoped result authenticates signatures and selected Integrity originals only. It
//! supplies no current issuer state, FI decision, native clock, pending enrollment or monetary
//! grant. Its Node consumer must independently admit both interval endpoints from the genuine
//! signed-clock original and separately retain current World policy before a financial effect.

use super::*;
use crate::kagemusha::{
    KagemushaAuthenticatedReleaseV1, KagemushaDevicePublicKeyV1,
    KagemushaPlayIntegrityRefreshLeaseV1, KagemushaSignedPlayIntegrityRefreshChallengeV1,
    KagemushaVerifiedOrdinaryAppCredentialV1, KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
};

/// Borrowed complete data originals; decoding or choosing these bytes establishes no owner.
#[derive(Clone, Copy)]
pub struct KagemushaOrdinaryEnrollmentProofOriginalsV1<'a> {
    /// Full signed C transport original.
    pub preparation_original: &'a [u8],
    /// Exact expected original subject, additionally joined by the financial proof consumer.
    pub expected_preparation: &'a KagemushaOrdinaryAppEnrollmentChallengeV1,
    /// Full signed raw admission original.
    pub raw_admission_original: &'a [u8],
    /// Complete untouched platform evidence container original.
    pub platform_original: &'a [u8],
    /// Complete canonical enrollment-possession original.
    pub possession_original: &'a [u8],
    /// Complete signed app credential original.
    pub credential_original: &'a [u8],
    /// Exact original app key, separately joined to the admitted financial subject.
    pub selected_key: &'a KagemushaDevicePublicKeyV1,
}

/// Complete selected refresh data. The baseline lease is retained as enrollment history,
/// while this distinct lease must cover the actual immutable proof interval.
pub struct KagemushaOrdinaryIntegrityProofOriginalsV1<'a> {
    /// Complete signed Core refresh challenge original.
    pub challenge_original: &'a [u8],
    /// Complete issuer/platform refresh lease original.
    pub lease_original: &'a [u8],
}

/// Closed signature/proof-context result; no decoder, Clone, current or pending owner API.
pub struct KagemushaVerifiedOrdinaryEnrollmentProofContextV1 {
    history: KagemushaVerifiedHistoricalOrdinaryEnrollmentV1,
    selected_integrity: Option<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    interval: [u64; 2],
}
impl KagemushaVerifiedOrdinaryEnrollmentProofContextV1 {
    /// Borrow the actual issuer-authenticated original credential for proof mathematics.
    /// Native custody, current FI and current PI remain separate obligations.
    #[must_use]
    pub const fn credential(&self) -> &KagemushaVerifiedOrdinaryAppCredentialV1 {
        &self.history.credential
    }
    /// Borrow the exact separately authenticated lease selected in this immutable proof.
    #[must_use]
    pub fn selected_integrity_lease(
        &self,
    ) -> Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1> {
        self.selected_integrity.as_ref()
    }
    /// Both original mathematical endpoints; this is data and establishes no elapsed clock.
    #[must_use]
    pub const fn proof_interval_ms(&self) -> [u64; 2] {
        self.interval
    }
    /// Recheck only the exact retained originals and original mathematical interval.
    /// This does not extend the interval or grant a live/current financial effect.
    /// # Errors
    /// Refuses another policy/root, original interval, credential or exact selected lease.
    pub fn recheck_originals(
        &self,
        policy: &KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1,
    ) -> Result<(), String> {
        let c = self.credential();
        let [lower, upper] = self.interval;
        let profile = &policy.policy.profile;
        if lower > upper
            || lower < profile.valid_from_ms
            || upper >= profile.expires_at_ms
            || lower < c.subject().issued_at_ms
            || c.identity_policy_id() != policy.policy_id()
            || c.identity_policy_original() != policy.original()
            || c.identity_authority_original() != policy.authority_original()
        {
            return Err("ordinary proof enrollment policy/original interval differs".into());
        }
        for endpoint in [lower, upper] {
            match &self.selected_integrity {
                Some(lease) => c.recheck_with_integrity_lease(lease, endpoint)?,
                None => c.recheck_at_trusted_time(endpoint)?,
            }
        }
        Ok(())
    }
}

impl KagemushaAuthenticatedOrdinaryAppIdentityPolicyV1 {
    /// Authenticate full enrollment history and the exact selected proof-interval lease.
    ///
    /// All C/raw/E/credential originals are authenticated at the credential's signed issue
    /// instant by the existing archived verifier. The selected refresh is independently
    /// authenticated under the SAME threshold policy/Core/app keys and complete governed
    /// release. An expired short enrollment ceremony or baseline PI is never renewed.
    /// `lower_ms`/`upper_ms` are mathematical data here; only the caller's concrete verified
    /// signed-clock capability can establish their authority. This result cannot issue or
    /// reopen enrollment, produce current FI status or authorize a reserve/debit.
    /// # Errors
    /// Refuses complete-original/signature substitutions, a malformed original interval,
    /// missing selected Integrity, wrong issuer/release or expiration at either endpoint.
    pub fn authenticate_proof_enrollment_originals(
        &self,
        originals: KagemushaOrdinaryEnrollmentProofOriginalsV1<'_>,
        release: &KagemushaAuthenticatedReleaseV1,
        selected_integrity: Option<KagemushaOrdinaryIntegrityProofOriginalsV1<'_>>,
        lower_ms: u64,
        upper_ms: u64,
    ) -> Result<KagemushaVerifiedOrdinaryEnrollmentProofContextV1, String> {
        if lower_ms > upper_ms {
            return Err("ordinary proof enrollment interval is reversed".into());
        }
        let archive = self.authenticate_archived_enrollment_original_data(
            originals.preparation_original,
            originals.expected_preparation,
            originals.raw_admission_original,
            originals.platform_original,
            originals.possession_original,
            originals.credential_original,
            originals.selected_key,
        )?;
        let history = archive.history;
        let selected_integrity = if let Some(originals) = selected_integrity {
            let raw = originals.lease_original;
            if raw.is_empty() || raw.len() > 4096 {
                return Err(
                    "ordinary proof selected Integrity original is outside its bound".into(),
                );
            }
            let lease: KagemushaPlayIntegrityRefreshLeaseV1 = norito::decode_canonical_with_limits(
                raw,
                norito::canonical_decode_limits(raw.len()),
            )
            .map_err(|e| e.to_string())?;
            if lease.canonical_bytes()? != raw {
                return Err("ordinary proof selected Integrity original is not canonical".into());
            }
            let challenge = KagemushaSignedPlayIntegrityRefreshChallengeV1::from_transport_bytes(
                originals.challenge_original,
            )?;
            Some(lease.authenticate(
                &history.credential,
                release,
                &self.policy.trust,
                &self.policy.app_authority(),
                &challenge,
                &self.policy.enrollment_issuer_key,
                lower_ms,
            )?)
        } else {
            None
        };
        let result = KagemushaVerifiedOrdinaryEnrollmentProofContextV1 {
            history,
            selected_integrity,
            interval: [lower_ms, upper_ms],
        };
        result.recheck_originals(self)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha::*;
    use crate::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1;
    use iroha_crypto::KeyPair;

    // Genuine signatures over known-public synthetic originals only. No physical evidence,
    // installed Native clock/owner, FI current admission or monetary grant is constructed.
    struct Originals {
        c: Vec<u8>,
        raw: Vec<u8>,
        e: Vec<u8>,
        credential: Vec<u8>,
    }
    impl Originals {
        fn new(f: &KagemushaOrdinaryRetailEnrollmentFixtureV1) -> Self {
            let subject = &f.selection.issuance.credential.subject;
            let c = &f.selection.preparation.challenge;
            let platform_sha = Sha256::digest(&f.proof.raw_attestation).into();
            let raw_subject = KagemushaRawAppAttestationAdmissionSubjectV1 {
                version: 1,
                enrollment_challenge_digest: c.attestation_challenge().unwrap(),
                authority_policy_digest: c.app_authority_policy_digest,
                platform_class: subject.platform_class,
                security_level: subject.security_level,
                app_public_key: subject.app_public_key,
                attested_key_id: subject.attested_key_id,
                raw_platform_evidence_digest: platform_sha,
                app_signing_identity_digest: subject.app_signing_identity_digest,
                original_app_attest_counter: 0,
                issued_at_ms: c.issued_at_ms,
                expires_at_ms: c.expires_at_ms,
            };
            let app_issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
            let raw = KagemushaRawAppAttestationAdmissionV1 {
                signature: Signature::new(
                    app_issuer.private_key(),
                    &raw_subject.canonical_signing_bytes().unwrap(),
                ),
                subject: raw_subject,
            };
            let e = KagemushaAppEnrollmentPossessionV1 {
                challenge: KagemushaAppEnrollmentPossessionChallengeV1::from_original_enrollment(
                    c,
                    &subject.app_public_key,
                    platform_sha,
                )
                .unwrap(),
                evidence: f.proof.app_possession.clone(),
            };
            Self {
                c: f.selection.preparation.to_transport_bytes().unwrap(),
                raw: raw.to_transport_bytes().unwrap(),
                e: norito::encode_canonical(&e).unwrap(),
                credential: f.selection.issuance.credential.canonical_bytes().unwrap(),
            }
        }
        fn borrowed<'a>(
            &'a self,
            f: &'a KagemushaOrdinaryRetailEnrollmentFixtureV1,
        ) -> KagemushaOrdinaryEnrollmentProofOriginalsV1<'a> {
            KagemushaOrdinaryEnrollmentProofOriginalsV1 {
                preparation_original: &self.c,
                expected_preparation: &f.selection.preparation.challenge,
                raw_admission_original: &self.raw,
                platform_original: &f.proof.raw_attestation,
                possession_original: &self.e,
                credential_original: &self.credential,
                selected_key: &f.selection.issuance.credential.subject.app_public_key,
            }
        }
    }

    #[test]
    fn exact_refreshed_integrity_covers_proof_after_baseline_and_short_ceremony_expire() {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::android_with_integrity();
        let originals = Originals::new(&f);
        let (challenge, lease) = f.integrity_refresh_originals();
        let challenge_raw = challenge.to_transport_bytes().unwrap();
        let lease_raw = lease.canonical_bytes().unwrap();
        let policy = f.ordinary_policy.identity_policy();
        let selected = || KagemushaOrdinaryIntegrityProofOriginalsV1 {
            challenge_original: &challenge_raw,
            lease_original: &lease_raw,
        };
        let proof = policy
            .authenticate_proof_enrollment_originals(
                originals.borrowed(&f),
                &f.release,
                Some(selected()),
                2100,
                2200,
            )
            .unwrap();
        assert_eq!(proof.proof_interval_ms(), [2100, 2200]);
        assert_eq!(proof.credential().original(), originals.credential);
        assert_eq!(
            proof.selected_integrity_lease().unwrap().original(),
            lease_raw
        );
        proof.recheck_originals(policy).unwrap();
        assert!(
            policy
                .authenticate_proof_enrollment_originals(
                    originals.borrowed(&f),
                    &f.release,
                    None,
                    2100,
                    2200,
                )
                .is_err()
        );
        assert!(
            policy
                .authenticate_proof_enrollment_originals(
                    originals.borrowed(&f),
                    &f.release,
                    Some(selected()),
                    2100,
                    2400,
                )
                .is_err()
        );
        // The established current restart API retains its own stricter current/baseline gate.
        assert!(
            policy
                .authenticate_historical_enrollment_originals(
                    &originals.c,
                    &f.selection.preparation.challenge,
                    &originals.raw,
                    &f.proof.raw_attestation,
                    &originals.e,
                    &originals.credential,
                    &f.selection.issuance.credential.subject.app_public_key,
                    2100,
                )
                .is_err()
        );
    }

    #[test]
    fn historical_proof_context_rejects_reversed_future_and_changed_originals() {
        let f = KagemushaOrdinaryRetailEnrollmentFixtureV1::new(false);
        let mut originals = Originals::new(&f);
        let policy = f.ordinary_policy.identity_policy();
        assert!(
            policy
                .authenticate_proof_enrollment_originals(
                    originals.borrowed(&f),
                    &f.release,
                    None,
                    3000,
                    3100,
                )
                .is_ok()
        );
        for bounds in [[3100, 3000], [199, 200], [3000, 10200]] {
            assert!(
                policy
                    .authenticate_proof_enrollment_originals(
                        originals.borrowed(&f),
                        &f.release,
                        None,
                        bounds[0],
                        bounds[1],
                    )
                    .is_err()
            );
        }
        originals.credential[60] ^= 1;
        assert!(
            policy
                .authenticate_proof_enrollment_originals(
                    originals.borrowed(&f),
                    &f.release,
                    None,
                    3000,
                    3100,
                )
                .is_err()
        );
        originals = Originals::new(&f);
        originals.raw[100] ^= 1;
        assert!(
            policy
                .authenticate_proof_enrollment_originals(
                    originals.borrowed(&f),
                    &f.release,
                    None,
                    3000,
                    3100,
                )
                .is_err()
        );
    }
}
