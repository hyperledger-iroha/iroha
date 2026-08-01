//! Canonical native Falcon-512 blind issuance for Bootle/Lantern credentials.
//!
//! The first-release lifecycle is deliberately closed: generate a concrete
//! Falcon/NTRU issuer key, derive its governed policy, prove a blinded holder
//! request, verify and sign that request, then let the holder independently
//! finalize and validate the credential before any presentation proof is
//! produced. There is no direct or trusted-issuance shortcut.

use iroha_data_model::privacy::{
    BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1, BootleLanternAllowedAttributeValuesV1,
    BootleLanternAttributeValueV1, BootleLanternDisclosedAttributeV1,
    BootleLanternIssuerPolicyLifecycleV1, BootleLanternIssuerPolicyV1,
    BootleLanternIssuerPublicMatrixV1, BootleLanternPolynomialV1,
    IrohaBootleLanternAnoncredStatementV1, PrivacyBootleLanternIssuerPolicyDigestV1,
    PrivacyIssuerIdV1, PrivacyParameterDigestV1, PrivacyParameterIdV1, PrivacyPolicyIdV1,
    PrivacyStatementContextV1,
};
use rand_core_06::{CryptoRng, RngCore};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use super::{
    credential_sampling::{CredentialRandomnessErrorV1, sample_credential_randomness_v1},
    falcon512::{self, DEGREE as FALCON_DEGREE_V1, Trapdoor},
    params::{APPLICATION_MODULUS_V1, APPLICATION_RING_DEGREE_V1, APPLICATION_ROWS_V1},
    proof::{
        PresentationProofErrorV1, prove_blind_issuance_request_v1, verify_blind_issuance_request_v1,
    },
    relation::{
        BootleLanternPresentationWitnessV1, RelationErrorV1, compile_application_relation_v1,
        compile_blind_issuance_request_relation_v1, validate_presentation_witness_v1,
    },
    ring::ApplicationPolynomialV1,
    scope::{BootleLanternCredentialScopeV1, CredentialScopeErrorV1},
    transcript::{
        BlindIssuanceRequestChallengeBindingV1, BlindIssuanceRequestTranscriptV1, MatrixRoleV1,
        TranscriptErrorV1, expand_application_matrix_v1, matrix_seed_v1,
    },
};
use crate::privacy_engines::prover_randomness::{
    HealthCheckedCryptoRngV1, ProverRandomnessErrorV1,
};

/// Exact concrete issuer profile committed by compiled privacy metadata.
pub const BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1: &[u8] = b"falcon512-ntru-r512-as-r64-rank8-interleaved|source:rust-fn-dsa-workspace-0.3-daf14859b5aa3f8d75c42966ba7de83e6eb59997-Unlicense|specialization:BLNS-specialization-no-main-construction-reduction|public-key:H_i[j]=h[8*j+i]|equation:s1+h*s2=t+A_tau*tau+credential-scope|keygen-candidates:4096|request-nonce-draws:4|preimage-attempts:64|tau:one-64-byte-draw-eight-R64-MSB-first|preimage-rng:56-byte-Falcon-ChaCha20-word-major|canonical-flow:keygen-request-P1-verify-issue-finalize-P2|no-direct-issuance";
/// Maximum complete Falcon/NTRU key candidates derived from one seed.
pub const MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1: u32 = 4_096;
/// Maximum non-zero request-nonce draws before failing closed.
pub const MAX_BOOTLE_LANTERN_REQUEST_NONCE_ATTEMPTS_V1: u32 = 4;
/// Maximum independent Falcon sampler-coin draws for one fixed target.
pub const MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1: u32 = 64;

const ISSUER_PROFILE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.issuer-profile-digest.v1";
const MASKED_TARGET_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.bootle-lantern.masked-target-digest.v1";
const REQUEST_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.bootle-lantern.blind-request-digest.v1";

/// Public governance metadata used to derive one active issuer policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootleLanternIssuerPolicyMetadataV1 {
    /// Credential issuer governed by the new record.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Stable policy identity in the issuer namespace.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact non-zero policy/key epoch.
    pub epoch: u64,
    /// Attributes every presentation must disclose.
    pub required_disclosure_bitmap: u8,
    /// Fixed-order allowed public values for all eight attributes.
    pub allowed_values: Vec<BootleLanternAllowedAttributeValuesV1>,
}

/// One native Falcon-512 issuer trapdoor and its exact public matrix.
pub struct BootleLanternIssuerKeyPairV1 {
    issuer_parameter_id: PrivacyParameterIdV1,
    public_matrix: BootleLanternIssuerPublicMatrixV1,
    trapdoor: Trapdoor,
}

impl core::fmt::Debug for BootleLanternIssuerKeyPairV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BootleLanternIssuerKeyPairV1(<redacted>)")
    }
}

impl BootleLanternIssuerKeyPairV1 {
    /// Generate one genuine bounded Falcon-512/NTRU issuer key.
    ///
    /// # Errors
    ///
    /// Rejects a zero parameter identity, unavailable or unhealthy
    /// cryptographic randomness, key-generation exhaustion, or a public-key
    /// structural invariant failure.
    pub fn generate_with_rng_v1<R: CryptoRng + RngCore>(
        issuer_parameter_id: PrivacyParameterIdV1,
        rng: &mut R,
    ) -> Result<Self, BootleLanternIssuanceErrorV1> {
        if issuer_parameter_id.is_zero() {
            return Err(BootleLanternIssuanceErrorV1::InvalidIssuerParameterId);
        }
        let mut checked = HealthCheckedCryptoRngV1::new(rng).map_err(map_randomness_error_v1)?;
        let mut seed = Zeroizing::new([0_u8; 32]);
        checked
            .try_fill_bytes(seed.as_mut())
            .map_err(|_| BootleLanternIssuanceErrorV1::RandomnessUnavailable)?;
        let trapdoor =
            falcon512::generate_from_seed(&*seed, MAX_BOOTLE_LANTERN_ISSUER_KEYGEN_CANDIDATES_V1)
                .ok_or(BootleLanternIssuanceErrorV1::IssuerKeyGenerationExhausted)?;
        let public_matrix = public_matrix_from_falcon_h_v1(&**trapdoor.h)?;
        public_matrix
            .validate_r512_multiplication_structure_v1()
            .map_err(|_| BootleLanternIssuanceErrorV1::InvalidIssuerPublicMatrix)?;
        Ok(Self {
            issuer_parameter_id,
            public_matrix,
            trapdoor,
        })
    }

    /// Build the self-digested active governed policy for this exact key.
    ///
    /// # Errors
    ///
    /// Rejects malformed metadata or any matrix, parameter-digest, record,
    /// lifecycle, or initial-policy invariant failure.
    pub fn active_policy_v1(
        &self,
        metadata: BootleLanternIssuerPolicyMetadataV1,
    ) -> Result<BootleLanternIssuerPolicyV1, BootleLanternIssuanceErrorV1> {
        let mut policy = BootleLanternIssuerPolicyV1 {
            issuer_id: metadata.issuer_id,
            policy_id: metadata.policy_id,
            epoch: metadata.epoch,
            lifecycle: BootleLanternIssuerPolicyLifecycleV1::Active,
            issuer_parameter_id: self.issuer_parameter_id,
            issuer_parameter_digest: PrivacyParameterDigestV1::new([0; 32]),
            issuer_public_matrix: self.public_matrix.clone(),
            required_disclosure_bitmap: metadata.required_disclosure_bitmap,
            allowed_values: metadata.allowed_values,
            record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
        };
        policy.issuer_parameter_digest = policy
            .computed_issuer_parameter_digest()
            .map_err(|_| BootleLanternIssuanceErrorV1::PolicyEncodingFailed)?;
        policy.record_digest = policy
            .computed_record_digest()
            .map_err(|_| BootleLanternIssuanceErrorV1::PolicyEncodingFailed)?;
        policy
            .validate()
            .map_err(|_| BootleLanternIssuanceErrorV1::InvalidIssuerPolicy)?;
        Ok(policy)
    }

    fn matches_policy(&self, policy: &BootleLanternIssuerPolicyV1) -> bool {
        policy.lifecycle == BootleLanternIssuerPolicyLifecycleV1::Active
            && policy.issuer_parameter_id == self.issuer_parameter_id
            && policy.issuer_public_matrix == self.public_matrix
            && policy.validate().is_ok()
    }
}

/// Holder-to-issuer P1 request. All fields are exact and field-private.
pub struct BootleLanternBlindIssuanceRequestV1 {
    target: [ApplicationPolynomialV1; APPLICATION_ROWS_V1],
    target_digest: [u8; 32],
    request_nonce: [u8; 32],
    scope_digest: [u8; 32],
    issuer_profile_digest: [u8; 32],
    policy_record_digest: PrivacyBootleLanternIssuerPolicyDigestV1,
    proof: super::codec::BootleLanternBlindIssuanceRequestProofV1,
    request_digest: [u8; 32],
}

impl core::fmt::Debug for BootleLanternBlindIssuanceRequestV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("BootleLanternBlindIssuanceRequestV1")
            .field("request_digest", &self.request_digest)
            .finish_non_exhaustive()
    }
}

impl BootleLanternBlindIssuanceRequestV1 {
    /// Digest of the complete canonical request, including its P1 wire.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
}

/// Secret holder state consumed while finalizing exactly one request.
pub struct BootleLanternBlindIssuanceStateV1 {
    randomness: [ApplicationPolynomialV1; 16],
    attributes: [[u8; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
    request_digest: [u8; 32],
    scope: BootleLanternCredentialScopeV1,
}

impl core::fmt::Debug for BootleLanternBlindIssuanceStateV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BootleLanternBlindIssuanceStateV1(<redacted>)")
    }
}

impl Zeroize for BootleLanternBlindIssuanceStateV1 {
    fn zeroize(&mut self) {
        self.randomness.zeroize();
        self.attributes.zeroize();
        self.request_digest.zeroize();
    }
}

impl Drop for BootleLanternBlindIssuanceStateV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Exact issuer response to one verified P1 request.
pub struct BootleLanternBlindIssuanceResponseV1 {
    tag: [ApplicationPolynomialV1; 8],
    signature_one: [ApplicationPolynomialV1; 8],
    signature_two: [ApplicationPolynomialV1; 8],
    request_digest: [u8; 32],
    scope_digest: [u8; 32],
    policy_record_digest: PrivacyBootleLanternIssuerPolicyDigestV1,
}

impl core::fmt::Debug for BootleLanternBlindIssuanceResponseV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BootleLanternBlindIssuanceResponseV1(<redacted>)")
    }
}

impl Zeroize for BootleLanternBlindIssuanceResponseV1 {
    fn zeroize(&mut self) {
        self.tag.zeroize();
        self.signature_one.zeroize();
        self.signature_two.zeroize();
        self.request_digest.zeroize();
        self.scope_digest.zeroize();
    }
}

impl Drop for BootleLanternBlindIssuanceResponseV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// Reusable holder credential produced only by canonical finalization.
pub struct BootleLanternCredentialV1 {
    randomness: [ApplicationPolynomialV1; 16],
    tag: [ApplicationPolynomialV1; 8],
    signature_one: [ApplicationPolynomialV1; 8],
    signature_two: [ApplicationPolynomialV1; 8],
    attributes: [[u8; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
    scope: BootleLanternCredentialScopeV1,
}

impl core::fmt::Debug for BootleLanternCredentialV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("BootleLanternCredentialV1(<redacted>)")
    }
}

impl Zeroize for BootleLanternCredentialV1 {
    fn zeroize(&mut self) {
        self.randomness.zeroize();
        self.tag.zeroize();
        self.signature_one.zeroize();
        self.signature_two.zeroize();
        self.attributes.zeroize();
    }
}

impl Drop for BootleLanternCredentialV1 {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl BootleLanternCredentialV1 {
    /// Build and independently validate the witness for one later statement.
    ///
    /// Action index and transaction intent may change between presentations;
    /// every reusable scope field and the exact active policy must remain the
    /// same.
    ///
    /// # Errors
    ///
    /// Rejects a policy/statement/scope substitution, disclosure violation,
    /// norm violation, or failed credential equation.
    pub fn presentation_witness_v1(
        &self,
        statement: &IrohaBootleLanternAnoncredStatementV1,
        policy: &BootleLanternIssuerPolicyV1,
        canonical_genesis_hash: [u8; 32],
    ) -> Result<BootleLanternPresentationWitnessV1, BootleLanternIssuanceErrorV1> {
        if !self
            .scope
            .matches(&statement.context, canonical_genesis_hash, policy)
        {
            return Err(BootleLanternIssuanceErrorV1::CredentialScopeMismatch);
        }
        let matrix_seed = matrix_seed_v1(*statement.context.parameter_digest.as_bytes())
            .map_err(|_| BootleLanternIssuanceErrorV1::TranscriptFailed)?;
        let relation =
            compile_application_relation_v1(statement, policy, matrix_seed, canonical_genesis_hash)
                .map_err(|_| BootleLanternIssuanceErrorV1::RelationFailed)?;
        let witness = BootleLanternPresentationWitnessV1 {
            randomness: self.randomness,
            tag: self.tag,
            signature_one: self.signature_one,
            signature_two: self.signature_two,
            attributes: self.attributes,
        };
        validate_presentation_witness_v1(&relation, &witness)
            .map_err(|_| BootleLanternIssuanceErrorV1::CredentialValidationFailed)?;
        Ok(witness)
    }
}

/// Prepare the only canonical holder P1 request.
///
/// # Errors
///
/// Rejects an invalid/non-active policy or scope, unavailable/unhealthy
/// randomness, holder-sampler exhaustion, matrix expansion, invalid request
/// nonce, transcript construction, or P1 proof failure.
pub fn holder_prepare_blind_issuance_with_rng_v1<
    RMask: CryptoRng + RngCore,
    RProof: CryptoRng + RngCore,
>(
    context: &PrivacyStatementContextV1,
    canonical_genesis_hash: [u8; 32],
    policy: &BootleLanternIssuerPolicyV1,
    attributes: [[u8; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
    mask_rng: &mut RMask,
    proof_rng: &mut RProof,
) -> Result<
    (
        BootleLanternBlindIssuanceRequestV1,
        BootleLanternBlindIssuanceStateV1,
    ),
    BootleLanternIssuanceErrorV1,
> {
    require_active_policy_v1(policy)?;
    let scope = BootleLanternCredentialScopeV1::new(context, canonical_genesis_hash, policy)
        .map_err(|_| BootleLanternIssuanceErrorV1::CredentialScopeFailed)?;
    let scope_digest = scope
        .digest()
        .map_err(|_| BootleLanternIssuanceErrorV1::CredentialScopeFailed)?;
    let issuer_profile_digest = bootle_lantern_issuer_profile_digest_v1();
    let matrix_seed = matrix_seed_v1(*context.parameter_digest.as_bytes())
        .map_err(|_| BootleLanternIssuanceErrorV1::TranscriptFailed)?;

    let mut checked_mask_rng =
        HealthCheckedCryptoRngV1::new(mask_rng).map_err(map_randomness_error_v1)?;
    let randomness = sample_credential_randomness_v1(&mut checked_mask_rng)
        .map_err(map_credential_randomness_error_v1)?;
    let randomness = randomness.into_polynomials();
    let target = masked_target_v1(matrix_seed, &randomness, &attributes)?;
    let target_digest = digest_application_vector_v1(MASKED_TARGET_DIGEST_DOMAIN_V1, &target);
    let request_nonce = sample_nonzero_request_nonce_v1(&mut checked_mask_rng)?;
    let relation = compile_blind_issuance_request_relation_v1(matrix_seed, &target)
        .map_err(|_| BootleLanternIssuanceErrorV1::RelationFailed)?;
    let transcript = BlindIssuanceRequestTranscriptV1::new(
        BlindIssuanceRequestChallengeBindingV1 {
            parameter_digest: *context.parameter_digest.as_bytes(),
            genesis_hash: canonical_genesis_hash,
            issuer_profile_digest,
            credential_scope_digest: scope_digest,
            issuer_policy_record_digest: *policy.record_digest.as_bytes(),
            masked_target_digest: target_digest,
            request_nonce,
        },
        matrix_seed,
        super::application_relation_digest_v1(&relation),
    )
    .map_err(|_| BootleLanternIssuanceErrorV1::TranscriptFailed)?;
    let p1_witness = BootleLanternPresentationWitnessV1 {
        randomness,
        tag: [ApplicationPolynomialV1::ZERO; 8],
        signature_one: [ApplicationPolynomialV1::ZERO; 8],
        signature_two: [ApplicationPolynomialV1::ZERO; 8],
        attributes,
    };
    let mut checked_proof_rng =
        HealthCheckedCryptoRngV1::new(proof_rng).map_err(map_randomness_error_v1)?;
    let proof =
        prove_blind_issuance_request_v1(&relation, &p1_witness, transcript, &mut checked_proof_rng)
            .map_err(|_| BootleLanternIssuanceErrorV1::BlindRequestProofFailed)?;
    let request_digest = blind_request_digest_v1(
        &target_digest,
        &request_nonce,
        &scope_digest,
        policy.record_digest.as_bytes(),
        &proof.encode(),
    );
    let request = BootleLanternBlindIssuanceRequestV1 {
        target,
        target_digest,
        request_nonce,
        scope_digest,
        issuer_profile_digest,
        policy_record_digest: policy.record_digest,
        proof,
        request_digest,
    };
    let state = BootleLanternBlindIssuanceStateV1 {
        randomness,
        attributes,
        request_digest,
        scope,
    };
    Ok((request, state))
}

/// Verify P1 and issue one blinded credential response.
///
/// # Errors
///
/// Rejects key/policy/context/request substitution before randomness, any P1
/// failure, unavailable/unhealthy randomness, or bounded Falcon preimage
/// exhaustion. The tag and target are fixed across all preimage attempts.
pub fn issuer_blind_issue_with_rng_v1<RTag: CryptoRng + RngCore, RPreimage: CryptoRng + RngCore>(
    issuer: &BootleLanternIssuerKeyPairV1,
    context: &PrivacyStatementContextV1,
    canonical_genesis_hash: [u8; 32],
    policy: &BootleLanternIssuerPolicyV1,
    request: &BootleLanternBlindIssuanceRequestV1,
    tag_rng: &mut RTag,
    preimage_rng: &mut RPreimage,
) -> Result<BootleLanternBlindIssuanceResponseV1, BootleLanternIssuanceErrorV1> {
    require_active_policy_v1(policy)?;
    if !issuer.matches_policy(policy) {
        return Err(BootleLanternIssuanceErrorV1::IssuerKeyPolicyMismatch);
    }
    let scope = BootleLanternCredentialScopeV1::new(context, canonical_genesis_hash, policy)
        .map_err(|_| BootleLanternIssuanceErrorV1::CredentialScopeFailed)?;
    let scope_digest = scope
        .digest()
        .map_err(|_| BootleLanternIssuanceErrorV1::CredentialScopeFailed)?;
    if request.scope_digest != scope_digest
        || request.issuer_profile_digest != bootle_lantern_issuer_profile_digest_v1()
        || request.policy_record_digest != policy.record_digest
        || request.target_digest
            != digest_application_vector_v1(MASKED_TARGET_DIGEST_DOMAIN_V1, &request.target)
        || request.request_digest
            != blind_request_digest_v1(
                &request.target_digest,
                &request.request_nonce,
                &request.scope_digest,
                policy.record_digest.as_bytes(),
                &request.proof.encode(),
            )
    {
        return Err(BootleLanternIssuanceErrorV1::BlindRequestBindingMismatch);
    }
    let matrix_seed = matrix_seed_v1(*context.parameter_digest.as_bytes())
        .map_err(|_| BootleLanternIssuanceErrorV1::TranscriptFailed)?;
    let relation = compile_blind_issuance_request_relation_v1(matrix_seed, &request.target)
        .map_err(|_| BootleLanternIssuanceErrorV1::RelationFailed)?;
    let transcript = BlindIssuanceRequestTranscriptV1::new(
        BlindIssuanceRequestChallengeBindingV1 {
            parameter_digest: *context.parameter_digest.as_bytes(),
            genesis_hash: canonical_genesis_hash,
            issuer_profile_digest: request.issuer_profile_digest,
            credential_scope_digest: request.scope_digest,
            issuer_policy_record_digest: *policy.record_digest.as_bytes(),
            masked_target_digest: request.target_digest,
            request_nonce: request.request_nonce,
        },
        matrix_seed,
        super::application_relation_digest_v1(&relation),
    )
    .map_err(|_| BootleLanternIssuanceErrorV1::TranscriptFailed)?;
    verify_blind_issuance_request_v1(&relation, transcript, &request.proof)
        .map_err(|_| BootleLanternIssuanceErrorV1::BlindRequestProofFailed)?;

    let mut checked_tag_rng =
        HealthCheckedCryptoRngV1::new(tag_rng).map_err(map_randomness_error_v1)?;
    let tag = sample_tag_v1(&mut checked_tag_rng)?;
    let a_tau = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationTag)
        .map_err(|_| BootleLanternIssuanceErrorV1::MatrixExpansionFailed)?;
    let scope_term = scope
        .application_term()
        .map_err(|_| BootleLanternIssuanceErrorV1::CredentialScopeFailed)?;
    let mut signing_target = request.target;
    for row in 0..APPLICATION_ROWS_V1 {
        signing_target[row] = signing_target[row].add(scope_term[row]);
        for column in 0..8 {
            let coefficient = a_tau
                .get(
                    u16::try_from(row).expect("fixed row fits u16"),
                    u16::try_from(column).expect("fixed column fits u16"),
                )
                .ok_or(BootleLanternIssuanceErrorV1::MatrixExpansionFailed)?;
            signing_target[row] = signing_target[row].add(coefficient.multiply(tag[column]));
        }
    }
    let falcon_target = r64_rank8_to_r512_v1(&signing_target);
    let mut checked_preimage_rng =
        HealthCheckedCryptoRngV1::new(preimage_rng).map_err(map_randomness_error_v1)?;
    let mut preimage = None;
    for _ in 0..MAX_BOOTLE_LANTERN_PREIMAGE_ATTEMPTS_V1 {
        let mut seed = Zeroizing::new([0_u8; 56]);
        checked_preimage_rng
            .try_fill_bytes(seed.as_mut())
            .map_err(|_| BootleLanternIssuanceErrorV1::RandomnessUnavailable)?;
        if let Some(candidate) =
            falcon512::sample_preimage_from_seed(&issuer.trapdoor, &falcon_target, &*seed)
        {
            preimage = Some(candidate);
            break;
        }
    }
    let preimage = preimage.ok_or(BootleLanternIssuanceErrorV1::PreimageSamplingExhausted)?;
    let signature_one = centered_r512_to_r64_rank8_v1(&**preimage.first);
    let signature_two = centered_r512_to_r64_rank8_v1(&**preimage.second);
    Ok(BootleLanternBlindIssuanceResponseV1 {
        tag,
        signature_one,
        signature_two,
        request_digest: request.request_digest,
        scope_digest,
        policy_record_digest: policy.record_digest,
    })
}

/// Consume holder state, validate the complete credential equation, and
/// finalize one reusable credential.
///
/// # Errors
///
/// Rejects response, policy, scope, disclosure, norm, or application-equation
/// mismatch. Holder state is consumed and wiped on every path.
pub fn holder_finalize_blind_issuance_v1(
    state: BootleLanternBlindIssuanceStateV1,
    context: &PrivacyStatementContextV1,
    canonical_genesis_hash: [u8; 32],
    policy: &BootleLanternIssuerPolicyV1,
    response: BootleLanternBlindIssuanceResponseV1,
) -> Result<BootleLanternCredentialV1, BootleLanternIssuanceErrorV1> {
    require_active_policy_v1(policy)?;
    if response.request_digest != state.request_digest
        || response.policy_record_digest != policy.record_digest
        || response.scope_digest
            != state
                .scope
                .digest()
                .map_err(|_| BootleLanternIssuanceErrorV1::CredentialScopeFailed)?
        || !state.scope.matches(context, canonical_genesis_hash, policy)
    {
        return Err(BootleLanternIssuanceErrorV1::IssuanceResponseMismatch);
    }
    let witness = BootleLanternPresentationWitnessV1 {
        randomness: state.randomness,
        tag: response.tag,
        signature_one: response.signature_one,
        signature_two: response.signature_two,
        attributes: state.attributes,
    };
    let statement = validation_statement_v1(context.clone(), policy, &state.attributes);
    let matrix_seed = matrix_seed_v1(*context.parameter_digest.as_bytes())
        .map_err(|_| BootleLanternIssuanceErrorV1::TranscriptFailed)?;
    let relation =
        compile_application_relation_v1(&statement, policy, matrix_seed, canonical_genesis_hash)
            .map_err(|_| BootleLanternIssuanceErrorV1::RelationFailed)?;
    validate_presentation_witness_v1(&relation, &witness)
        .map_err(|_| BootleLanternIssuanceErrorV1::CredentialValidationFailed)?;
    Ok(BootleLanternCredentialV1 {
        randomness: witness.randomness,
        tag: witness.tag,
        signature_one: witness.signature_one,
        signature_two: witness.signature_two,
        attributes: witness.attributes,
        scope: state.scope.clone(),
    })
}

/// Digest of the exact native issuer implementation profile.
#[must_use]
pub fn bootle_lantern_issuer_profile_digest_v1() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ISSUER_PROFILE_DIGEST_DOMAIN_V1);
    hash.update(
        u64::try_from(BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1.len())
            .expect("fixed issuer descriptor length fits u64")
            .to_be_bytes(),
    );
    hash.update(BOOTLE_LANTERN_ISSUER_PROFILE_DESCRIPTOR_V1);
    hash.finalize().into()
}

fn public_matrix_from_falcon_h_v1(
    h: &[u16; FALCON_DEGREE_V1],
) -> Result<BootleLanternIssuerPublicMatrixV1, BootleLanternIssuanceErrorV1> {
    let first_column = core::array::from_fn(|row| BootleLanternPolynomialV1 {
        coefficients: (0..APPLICATION_RING_DEGREE_V1)
            .map(|coefficient| h[8 * coefficient + row])
            .collect(),
    });
    BootleLanternIssuerPublicMatrixV1::from_r512_first_column_blocks_v1(first_column)
        .map_err(|_| BootleLanternIssuanceErrorV1::InvalidIssuerPublicMatrix)
}

fn masked_target_v1(
    matrix_seed: super::transcript::MatrixSeedV1,
    randomness: &[ApplicationPolynomialV1; 16],
    attributes: &[[u8; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
) -> Result<[ApplicationPolynomialV1; APPLICATION_ROWS_V1], BootleLanternIssuanceErrorV1> {
    let a_r = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationRandomness)
        .map_err(|_| BootleLanternIssuanceErrorV1::MatrixExpansionFailed)?;
    let a_m = expand_application_matrix_v1(matrix_seed, MatrixRoleV1::ApplicationAttributes)
        .map_err(|_| BootleLanternIssuanceErrorV1::MatrixExpansionFailed)?;
    let attribute_polynomials = attributes.map(ApplicationPolynomialV1::from_direct_attribute);
    let mut target = [ApplicationPolynomialV1::ZERO; APPLICATION_ROWS_V1];
    for row in 0..APPLICATION_ROWS_V1 {
        for column in 0..randomness.len() {
            let coefficient = a_r
                .get(
                    u16::try_from(row).expect("fixed row fits u16"),
                    u16::try_from(column).expect("fixed column fits u16"),
                )
                .ok_or(BootleLanternIssuanceErrorV1::MatrixExpansionFailed)?;
            target[row] = target[row].add(coefficient.multiply(randomness[column]));
        }
        for column in 0..attribute_polynomials.len() {
            let coefficient = a_m
                .get(
                    u16::try_from(row).expect("fixed row fits u16"),
                    u16::try_from(column).expect("fixed column fits u16"),
                )
                .ok_or(BootleLanternIssuanceErrorV1::MatrixExpansionFailed)?;
            target[row] = target[row].add(coefficient.multiply(attribute_polynomials[column]));
        }
    }
    Ok(target)
}

fn sample_nonzero_request_nonce_v1<R: CryptoRng + RngCore>(
    rng: &mut R,
) -> Result<[u8; 32], BootleLanternIssuanceErrorV1> {
    for _ in 0..MAX_BOOTLE_LANTERN_REQUEST_NONCE_ATTEMPTS_V1 {
        let mut nonce = [0_u8; 32];
        rng.try_fill_bytes(&mut nonce)
            .map_err(|_| BootleLanternIssuanceErrorV1::RandomnessUnavailable)?;
        if nonce != [0; 32] {
            return Ok(nonce);
        }
        nonce.zeroize();
    }
    Err(BootleLanternIssuanceErrorV1::RequestNonceExhausted)
}

fn sample_tag_v1<R: CryptoRng + RngCore>(
    rng: &mut R,
) -> Result<[ApplicationPolynomialV1; 8], BootleLanternIssuanceErrorV1> {
    let mut bytes = Zeroizing::new([0_u8; 64]);
    rng.try_fill_bytes(bytes.as_mut())
        .map_err(|_| BootleLanternIssuanceErrorV1::RandomnessUnavailable)?;
    Ok(core::array::from_fn(|row| {
        let mut coefficients = [0_u16; APPLICATION_RING_DEGREE_V1];
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            *coefficient = u16::from((bytes[row * 8 + index / 8] >> (7 - index % 8)) & 1);
        }
        ApplicationPolynomialV1::new(coefficients).expect("binary tag coefficients are canonical")
    }))
}

fn r64_rank8_to_r512_v1(
    input: &[ApplicationPolynomialV1; APPLICATION_ROWS_V1],
) -> [u16; FALCON_DEGREE_V1] {
    let mut output = [0_u16; FALCON_DEGREE_V1];
    for row in 0..APPLICATION_ROWS_V1 {
        for coefficient in 0..APPLICATION_RING_DEGREE_V1 {
            output[8 * coefficient + row] = input[row].coefficients()[coefficient];
        }
    }
    output
}

fn centered_r512_to_r64_rank8_v1(
    input: &[i16; FALCON_DEGREE_V1],
) -> [ApplicationPolynomialV1; APPLICATION_ROWS_V1] {
    core::array::from_fn(|row| {
        let coefficients =
            core::array::from_fn(|coefficient| i64::from(input[8 * coefficient + row]));
        ApplicationPolynomialV1::from_centered_coefficients(coefficients)
    })
}

fn validation_statement_v1(
    context: PrivacyStatementContextV1,
    policy: &BootleLanternIssuerPolicyV1,
    attributes: &[[u8; 8]; BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1],
) -> IrohaBootleLanternAnoncredStatementV1 {
    let disclosures = attributes
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| policy.required_disclosure_bitmap & (1_u8 << index) != 0)
        .map(|(index, attribute)| BootleLanternDisclosedAttributeV1 {
            index: u8::try_from(index).expect("fixed attribute index fits u8"),
            value: BootleLanternAttributeValueV1::new(attribute),
        })
        .collect();
    IrohaBootleLanternAnoncredStatementV1 {
        context,
        issuer_id: policy.issuer_id,
        policy_id: policy.policy_id,
        issuer_policy_epoch: policy.epoch,
        issuer_policy_record_digest: policy.record_digest,
        issuer_parameter_id: policy.issuer_parameter_id,
        issuer_parameter_digest: policy.issuer_parameter_digest,
        disclosures,
    }
}

fn digest_application_vector_v1(
    domain: &[u8],
    vector: &[ApplicationPolynomialV1; APPLICATION_ROWS_V1],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    for polynomial in vector {
        for coefficient in polynomial.coefficients() {
            hash.update(coefficient.to_be_bytes());
        }
    }
    hash.finalize().into()
}

fn blind_request_digest_v1(
    target_digest: &[u8; 32],
    request_nonce: &[u8; 32],
    scope_digest: &[u8; 32],
    policy_record_digest: &[u8; 32],
    proof_wire: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(REQUEST_DIGEST_DOMAIN_V1);
    for field in [
        target_digest.as_slice(),
        request_nonce.as_slice(),
        scope_digest.as_slice(),
        policy_record_digest.as_slice(),
        proof_wire,
    ] {
        hash.update(
            u64::try_from(field.len())
                .expect("bounded request field length fits u64")
                .to_be_bytes(),
        );
        hash.update(field);
    }
    hash.finalize().into()
}

fn require_active_policy_v1(
    policy: &BootleLanternIssuerPolicyV1,
) -> Result<(), BootleLanternIssuanceErrorV1> {
    policy
        .validate()
        .map_err(|_| BootleLanternIssuanceErrorV1::InvalidIssuerPolicy)?;
    if policy.lifecycle != BootleLanternIssuerPolicyLifecycleV1::Active {
        return Err(BootleLanternIssuanceErrorV1::IssuerPolicyNotActive);
    }
    Ok(())
}

fn map_randomness_error_v1(error: ProverRandomnessErrorV1) -> BootleLanternIssuanceErrorV1 {
    match error {
        ProverRandomnessErrorV1::Unavailable => BootleLanternIssuanceErrorV1::RandomnessUnavailable,
        ProverRandomnessErrorV1::Unhealthy => BootleLanternIssuanceErrorV1::RandomnessUnhealthy,
    }
}

fn map_credential_randomness_error_v1(
    error: CredentialRandomnessErrorV1,
) -> BootleLanternIssuanceErrorV1 {
    match error {
        CredentialRandomnessErrorV1::RandomnessUnavailable => {
            BootleLanternIssuanceErrorV1::RandomnessUnavailable
        }
        CredentialRandomnessErrorV1::CoefficientSamplingExhausted
        | CredentialRandomnessErrorV1::SamplingExhausted => {
            BootleLanternIssuanceErrorV1::HolderRandomnessSamplingExhausted
        }
        CredentialRandomnessErrorV1::InternalInvariant => {
            BootleLanternIssuanceErrorV1::InternalInvariant
        }
    }
}

/// Failure in the closed native issuance lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum BootleLanternIssuanceErrorV1 {
    /// Issuer parameter identity was zero.
    #[error("Bootle/Lantern issuer parameter id is invalid")]
    InvalidIssuerParameterId,
    /// Cryptographic randomness became unavailable.
    #[error("Bootle/Lantern issuance randomness is unavailable")]
    RandomnessUnavailable,
    /// Cryptographic randomness failed the canonical health check.
    #[error("Bootle/Lantern issuance randomness failed its health check")]
    RandomnessUnhealthy,
    /// Bounded Falcon/NTRU key generation exhausted its candidates.
    #[error("Bootle/Lantern Falcon issuer key generation exhausted")]
    IssuerKeyGenerationExhausted,
    /// Generated issuer public matrix failed its exact structure.
    #[error("Bootle/Lantern issuer public matrix is invalid")]
    InvalidIssuerPublicMatrix,
    /// Policy canonical encoding failed.
    #[error("Bootle/Lantern issuer policy encoding failed")]
    PolicyEncodingFailed,
    /// Governed issuer policy was malformed.
    #[error("Bootle/Lantern issuer policy is invalid")]
    InvalidIssuerPolicy,
    /// A revoked policy was selected for issuance or presentation.
    #[error("Bootle/Lantern issuer policy is not active")]
    IssuerPolicyNotActive,
    /// Issuer trapdoor and governed public policy differ.
    #[error("Bootle/Lantern issuer key does not match policy")]
    IssuerKeyPolicyMismatch,
    /// Reusable credential scope construction failed.
    #[error("Bootle/Lantern credential scope construction failed")]
    CredentialScopeFailed,
    /// Later presentation selected another reusable scope.
    #[error("Bootle/Lantern credential scope mismatch")]
    CredentialScopeMismatch,
    /// Exact holder randomness sampling exhausted its bound.
    #[error("Bootle/Lantern holder randomness sampling exhausted")]
    HolderRandomnessSamplingExhausted,
    /// Matrix expansion failed.
    #[error("Bootle/Lantern application matrix expansion failed")]
    MatrixExpansionFailed,
    /// Non-zero request nonce sampling exhausted its bound.
    #[error("Bootle/Lantern request nonce sampling exhausted")]
    RequestNonceExhausted,
    /// Public relation construction failed.
    #[error("Bootle/Lantern issuance relation construction failed")]
    RelationFailed,
    /// Transcript construction failed.
    #[error("Bootle/Lantern issuance transcript construction failed")]
    TranscriptFailed,
    /// P1 construction or verification failed.
    #[error("Bootle/Lantern blind-request proof failed")]
    BlindRequestProofFailed,
    /// Request fields, target, policy, or proof digest were substituted.
    #[error("Bootle/Lantern blind-request binding mismatch")]
    BlindRequestBindingMismatch,
    /// Bounded Falcon preimage sampling exhausted for the fixed target.
    #[error("Bootle/Lantern Falcon preimage sampling exhausted")]
    PreimageSamplingExhausted,
    /// Holder state and issuer response differ.
    #[error("Bootle/Lantern issuance response mismatch")]
    IssuanceResponseMismatch,
    /// Final credential equation or norm check failed.
    #[error("Bootle/Lantern finalized credential validation failed")]
    CredentialValidationFailed,
    /// A closed implementation invariant failed.
    #[error("Bootle/Lantern issuance internal invariant failed")]
    InternalInvariant,
}

impl From<CredentialScopeErrorV1> for BootleLanternIssuanceErrorV1 {
    fn from(_: CredentialScopeErrorV1) -> Self {
        Self::CredentialScopeFailed
    }
}

impl From<RelationErrorV1> for BootleLanternIssuanceErrorV1 {
    fn from(_: RelationErrorV1) -> Self {
        Self::RelationFailed
    }
}

impl From<TranscriptErrorV1> for BootleLanternIssuanceErrorV1 {
    fn from(_: TranscriptErrorV1) -> Self {
        Self::TranscriptFailed
    }
}

impl From<PresentationProofErrorV1> for BootleLanternIssuanceErrorV1 {
    fn from(_: PresentationProofErrorV1) -> Self {
        Self::BlindRequestProofFailed
    }
}

const _: () = {
    assert!(APPLICATION_ROWS_V1 == 8);
    assert!(APPLICATION_RING_DEGREE_V1 * APPLICATION_ROWS_V1 == FALCON_DEGREE_V1);
    assert!(APPLICATION_MODULUS_V1 == 12_289);
};
