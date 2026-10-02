//! Complete ordinary Node Mint113 submission data; no decoded field grants authority.
use super::*;
use iroha_crypto::Signature;
use norito::{Decode, Encode, JsonDeserialize, JsonSerialize};
use sha2::{Digest as _, Sha256};

/// Absolute sole canonical submission ceiling. Actual native transaction/block bounds still apply. Individual
/// complete originals have independent limits; a maximal Cartesian product is deliberately
/// refused before submission/debit instead of omitting any original or truncating proof parents.
pub const KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1: usize = 60 * 1024 * 1024;

/// Full selected refresh originals. Baseline PI is immutable enrollment history.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::OrdinaryNodeMintIntegrityOriginalsV1")]
pub struct KagemushaOrdinaryNodeMintIntegrityOriginalsV1 {
    /// Full Core-signed refresh challenge original, without renewal.
    pub challenge_original: Vec<u8>,
    /// Full issuer/platform refresh lease original.
    pub lease_original: Vec<u8>,
}
impl KagemushaOrdinaryNodeMintIntegrityOriginalsV1 {
    fn validate_shape(&self) -> Result<(), String> {
        if self.challenge_original.len() != KAGEMUSHA_PLAY_INTEGRITY_REFRESH_TRANSPORT_BYTES_V1
            || self.lease_original.is_empty()
            || self.lease_original.len() > 4096
        {
            return Err("ordinary Node Mint Integrity originals exceed their exact bound".into());
        }
        Ok(())
    }
}

/// Complete first-release public Node input. The actual current World selects the full
/// purpose token BEFORE any offered identity root, signature, C or clock is authenticated.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::kagemusha::OrdinaryNodeMintSubmissionV1")]
pub struct KagemushaOrdinaryNodeMintSubmissionV1 {
    /// Exactly one; no old field fallback.
    pub version: u16,
    /// Complete unsigned Mint113 request original, including approval/proofs/ciphertext.
    pub topup_request_original: Vec<u8>,
    /// Separate exact account consent over the unsigned full request; not part of its ID.
    pub account_consent: Signature,
    /// Sole complete canonical Norito `Permission` archive, including its canonical JSON token
    /// payload/name; accepted only if its exact value is in actual current World.
    pub issuer_purpose_original: Vec<u8>,
    /// Full threshold-signed ordinary identity policy. Roots come from the actual World token.
    pub identity_policy_original: Vec<u8>,
    /// Full distinct Core enrollment issuer policy selected by that signed identity policy.
    pub core_enrollment_issuer_policy_original: Vec<u8>,
    /// Complete original signed enrollment challenge.
    pub enrollment_challenge_original: Vec<u8>,
    /// Complete original signed raw app-key admission.
    pub raw_admission_original: Vec<u8>,
    /// Full untouched original platform attestation container.
    pub platform_attestation_original: Vec<u8>,
    /// Full original enrollment possession statement and signature evidence.
    pub enrollment_possession_original: Vec<u8>,
    /// Full original signed app credential.
    pub credential_original: Vec<u8>,
    /// Complete acknowledged FI enrollment original, independently authenticated by Node.
    pub financial_enrollment_original: Vec<u8>,
    /// Original PI selected by the immutable Mint proof, independently authenticated at its bounds.
    pub preparation_integrity: Option<KagemushaOrdinaryNodeMintIntegrityOriginalsV1>,
    /// Separately fresh PI selected by the current financial effect decision.
    pub decision_integrity: Option<KagemushaOrdinaryNodeMintIntegrityOriginalsV1>,
    /// Full clock checkpoint/node/policy original whose SHA is in the actual World token.
    pub clock_selection_original: Vec<u8>,
    /// Full four signed nonce-bound preparation observations, retained for the immutable proof.
    pub preparation_clock_original: Vec<u8>,
    /// Separate four signed current decision observations; old proof time is never renewed.
    pub decision_clock_original: Vec<u8>,
    /// Up to two complete consecutive preparation proof parents, oldest first. These are
    /// untrusted data until actual Node committed execution independently admits each decision.
    pub preparation_clock_parent_originals: Vec<Vec<u8>>,
    /// Separate complete consecutive decision proof parents, oldest first.
    pub decision_clock_parent_originals: Vec<Vec<u8>>,
    /// Exact separately acknowledged FI decision from the immutable preparation context.
    pub preparation_control_original: Vec<u8>,
    /// Full independently purpose-authorized signed Core exclusive-head debit decision.
    pub debit_decision_original: Vec<u8>,
    /// Separate fresh signed FI control original captured for this exact effect decision.
    pub current_control_original: Vec<u8>,
}
impl KagemushaOrdinaryNodeMintSubmissionV1 {
    /// Bound every original BEFORE decoding and enforce exact immutable data joins.
    /// No root, proof, FI decision, reserve debit or funding owner is admitted here.
    /// # Errors
    /// Refuses unsupported, oversized, noncanonical or internally substituted data originals.
    pub fn validate_shape(&self) -> Result<(), String> {
        if self.version != 1 || self.account_consent.payload().len() != 64 {
            return Err("ordinary Node Mint version or account consent width differs".into());
        }
        for (raw, maximum) in [
            (
                &self.topup_request_original,
                KAGEMUSHA_ORDINARY_MINT_ORIGINAL_MAX_BYTES_V1,
            ),
            (&self.issuer_purpose_original, 32 * 1024),
            (&self.identity_policy_original, 16 * 1024),
            (&self.core_enrollment_issuer_policy_original, 16 * 1024),
            (&self.enrollment_challenge_original, 4096),
            (&self.raw_admission_original, 4096),
            (&self.platform_attestation_original, 192 * 1024),
            (
                &self.enrollment_possession_original,
                KAGEMUSHA_APP_ENROLLMENT_POSSESSION_MAX_BYTES_V1 + 1024,
            ),
            (&self.credential_original, 16 * 1024),
            (
                &self.financial_enrollment_original,
                KAGEMUSHA_ORDINARY_RETAIL_ENROLLMENT_MAX_BYTES_V1,
            ),
            (&self.clock_selection_original, 16 * 1024 * 1024 + 4096),
            (&self.preparation_clock_original, 16 * 1024 * 1024 + 4096),
            (&self.decision_clock_original, 16 * 1024 * 1024 + 4096),
            (&self.debit_decision_original, 16 * 1024),
            (
                &self.current_control_original,
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
            ),
            (
                &self.preparation_control_original,
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
            ),
        ] {
            if raw.is_empty() || raw.len() > maximum {
                return Err("ordinary Node Mint complete original is outside its bound".into());
            }
        }
        for parents in [
            &self.preparation_clock_parent_originals,
            &self.decision_clock_parent_originals,
        ] {
            if parents.len() > 2
                || parents
                    .iter()
                    .any(|raw| raw.is_empty() || raw.len() > 4 * 1024 * 1024)
            {
                return Err(
                    "ordinary Node Mint proof parents exceed their independent bound".into(),
                );
            }
        }
        for selected in [&self.preparation_integrity, &self.decision_integrity]
            .into_iter()
            .flatten()
        {
            selected.validate_shape()?;
        }
        // Charge every complete input before the sole canonical encoder allocates its frame.
        // 64KiB reserves finite Norito headers/lengths for this fixed field inventory.
        let mut payload_bytes = 0_usize;
        for raw in [
            &self.topup_request_original,
            &self.issuer_purpose_original,
            &self.identity_policy_original,
            &self.core_enrollment_issuer_policy_original,
            &self.enrollment_challenge_original,
            &self.raw_admission_original,
            &self.platform_attestation_original,
            &self.enrollment_possession_original,
            &self.credential_original,
            &self.financial_enrollment_original,
            &self.clock_selection_original,
            &self.preparation_clock_original,
            &self.decision_clock_original,
            &self.preparation_control_original,
            &self.debit_decision_original,
            &self.current_control_original,
        ]
        .into_iter()
        .chain(self.preparation_clock_parent_originals.iter())
        .chain(self.decision_clock_parent_originals.iter())
        {
            payload_bytes = payload_bytes
                .checked_add(raw.len())
                .ok_or("ordinary Node Mint payload arithmetic overflow")?;
        }
        for selected in [&self.preparation_integrity, &self.decision_integrity]
            .into_iter()
            .flatten()
        {
            payload_bytes = payload_bytes
                .checked_add(selected.challenge_original.len())
                .and_then(|n| n.checked_add(selected.lease_original.len()))
                .ok_or("ordinary Node Mint PI payload arithmetic overflow")?;
        }
        if payload_bytes > KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1 - 64 * 1024 {
            return Err("ordinary Node Mint complete originals exceed whole payload budget".into());
        }
        let request =
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&self.topup_request_original)?;
        let decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &self.debit_decision_original,
        )?;
        decision
            .subject
            .selection
            .validate_against_topup(&request)?;
        if request
            .authorization
            .statement
            .context
            .financial_control_original_sha256
            != <[u8; 32]>::from(Sha256::digest(&self.preparation_control_original))
            || decision.subject.current_financial_control_original_sha256
                != <[u8; 32]>::from(Sha256::digest(&self.current_control_original))
        {
            return Err(
                "ordinary Node Mint full request or current control original differs".into(),
            );
        }
        Ok(())
    }
    /// Decode only the bounded request selector for actual installed runtime dispatch.
    /// # Errors
    /// Refuses invalid complete data or unsupported original request.
    pub fn selected_release_id(&self) -> Result<[u8; 32], String> {
        self.validate_shape()?;
        Ok(
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&self.topup_request_original)?
                .authorization
                .statement
                .context
                .release_id,
        )
    }
    /// Sole complete bounded canonical data; no implicit role or authority conversion.
    /// # Errors
    /// Refuses invalid data, encoding errors or whole-submission overflow.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate_shape()?;
        let raw = norito::encode_canonical(self).map_err(|e| e.to_string())?;
        if raw.len() > KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1 {
            return Err("ordinary Node Mint full submission exceeds its finite budget".into());
        }
        Ok(raw)
    }
    /// Decode bounded exact public data only, without admitting offered policies or clocks.
    /// # Errors
    /// Refuses oversized, noncanonical, trailing or invalid full originals.
    pub fn decode_canonical_exact(raw: &[u8]) -> Result<Self, String> {
        if raw.is_empty() || raw.len() > KAGEMUSHA_ORDINARY_NODE_MINT_SUBMISSION_MAX_BYTES_V1 {
            return Err("ordinary Node Mint submission outside its finite budget".into());
        }
        let value: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|e| e.to_string())?;
        if value.canonical_bytes()? != raw {
            return Err("ordinary Node Mint submission is not canonical".into());
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_submission_roundtrip_preserves_both_control_originals_and_large_sequence() {
        let fixture =
            crate::testing::ordinary_node_mint::kagemusha_ordinary_node_mint_codec_fixture_v1();
        let value = fixture.submission;
        let raw = value.canonical_bytes().unwrap();
        let decoded = KagemushaOrdinaryNodeMintSubmissionV1::decode_canonical_exact(&raw).unwrap();
        assert_eq!(decoded, value);
        let decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &decoded.debit_decision_original,
        )
        .unwrap();
        assert!(decision.subject.selection.predecessor.logical_sequence > u128::from(u64::MAX));
        let mut trailing = raw;
        trailing.push(0);
        assert!(KagemushaOrdinaryNodeMintSubmissionV1::decode_canonical_exact(&trailing).is_err());
        let mut substitution = value.clone();
        substitution.preparation_control_original.push(0);
        assert!(substitution.validate_shape().is_err());
        let mut substitution = value.clone();
        substitution.current_control_original.push(0);
        assert!(substitution.validate_shape().is_err());
        let mut substitution = value;
        substitution.preparation_clock_parent_originals = vec![vec![1]; 3];
        assert!(substitution.validate_shape().is_err());
    }
    #[test]
    fn full_submission_limits_charge_all_complete_clock_and_parent_inputs() {
        let mut value =
            crate::testing::ordinary_node_mint::kagemusha_ordinary_node_mint_codec_fixture_v1()
                .submission;
        value.clock_selection_original = vec![1; 16 * 1024 * 1024];
        value.preparation_clock_original = vec![2; 16 * 1024 * 1024];
        value.decision_clock_original = vec![3; 16 * 1024 * 1024];
        value.preparation_clock_parent_originals = vec![vec![4; 4 * 1024 * 1024]; 2];
        value.decision_clock_parent_originals = vec![vec![5; 4 * 1024 * 1024]; 2];
        assert!(
            value
                .validate_shape()
                .unwrap_err()
                .contains("whole payload budget")
        );
    }
    #[test]
    fn distinct_ordinary_instruction_uses_actual_registered_codec_and_preserves_full_input() {
        use crate::isi::{InstructionRegistry, TopUpKagemushaOrdinaryV1};
        let value = TopUpKagemushaOrdinaryV1::new(
            crate::testing::ordinary_node_mint::kagemusha_ordinary_node_mint_codec_fixture_v1()
                .submission,
        )
        .unwrap();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let raw =
            norito::core::frame_bare_with_header_flags::<TopUpKagemushaOrdinaryV1>(&payload, flags)
                .unwrap();
        let registry = crate::isi::registry::default();
        let decoded =
            InstructionRegistry::decode(&registry, TopUpKagemushaOrdinaryV1::WIRE_ID, &raw)
                .unwrap()
                .unwrap();
        assert_eq!(
            decoded.as_any().downcast_ref::<TopUpKagemushaOrdinaryV1>(),
            Some(&value)
        );
        assert!(
            InstructionRegistry::decode(&registry, "iroha.kagemusha.v1.top_up", &raw)
                .unwrap()
                .is_err()
        );
    }
}
