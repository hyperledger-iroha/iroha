//! Route-specific profile binding for the complete-effect ordinary artifact.
//!
//! The current AXT descriptor and ID remain byte-identical. This new nominal
//! descriptor commits that fixed proof-kernel descriptor plus every replacement
//! ordinary source/statement/context/carrier identity. An AXT profile never
//! selects the complete-effect relation and an effect profile never selects AXT.

use super::*;
use crate::backend::{
    compact_bundle::execution_effect::EffectBundleWire, compact_execution_effect_batch,
};
use iroha_data_model::fastpq::{
    FastpqExecutionEffectKindV1, FastpqExecutionEffectStatementV1,
    FastpqOrdinarySourceStatementLeafV1,
};

#[derive(Clone, NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "fastpq_prover::backend::compact_model_statement::candidate_artifact::DeepExecutionEffectArtifactProfile",
    frame = "fastpq_prover::deep_compact::ExecutionEffectArtifactProfileV1"
)]
struct DeepExecutionEffectArtifactProfile {
    version: u16,
    // This exact descriptor includes all fixed field/FRI/masking/challenge
    // geometry. Its existing AXT identity is never rewritten as part of this cutover.
    proof_kernel_descriptor: DeepQuantityArtifactProfile,
    ordinary_relation: &'static str,
    ordinary_artifact_schema: String,
    ordinary_artifact_frame_hash: [u8; 16],
    statement_schema: String,
    statement_frame_hash: [u8; 16],
    source_schema: String,
    source_frame_hash: [u8; 16],
    effect_kind_frame_hash: [u8; 16],
    batch_context_frame_hash: [u8; 16],
    segment_context_frame_hash: [u8; 16],
    bundle_schema: String,
    bundle_frame_hash: [u8; 16],
    typed_key_domain: Vec<u8>,
    statement_digest_domain: Vec<u8>,
    key_hash_domain: Vec<u8>,
    ordering_hash_domain: Vec<u8>,
    quantity_value_format: u16,
    effect_discriminants: [u8; 4],
}
impl DeepExecutionEffectArtifactProfile {
    fn fixed() -> Self {
        use norito::schema::identity::{NoritoSchema as _, frame_hash};
        let [batch_context_frame_hash, segment_context_frame_hash] =
            compact_execution_effect_batch::context_frame_hashes();
        Self {
            version: 1,
            proof_kernel_descriptor: DeepQuantityArtifactProfile::fixed(),
            ordinary_relation: compact_execution_effect_batch::IDENTITY,
            ordinary_artifact_schema: FastpqOrdinaryCompactArtifactV1::frame_name(),
            ordinary_artifact_frame_hash: frame_hash::<FastpqOrdinaryCompactArtifactV1>(),
            statement_schema: FastpqExecutionEffectStatementV1::frame_name(),
            statement_frame_hash: frame_hash::<FastpqExecutionEffectStatementV1>(),
            source_schema: FastpqOrdinarySourceStatementLeafV1::frame_name(),
            source_frame_hash: frame_hash::<FastpqOrdinarySourceStatementLeafV1>(),
            effect_kind_frame_hash: frame_hash::<FastpqExecutionEffectKindV1>(),
            batch_context_frame_hash,
            segment_context_frame_hash,
            bundle_schema: EffectBundleWire::frame_name(),
            bundle_frame_hash: frame_hash::<EffectBundleWire>(),
            typed_key_domain: b"iroha:fastpq:execution-quantity-key:v1\0".to_vec(),
            statement_digest_domain: b"fastpq:execution-effects:v1:statement|".to_vec(),
            key_hash_domain: b"fastpq:execution-effects:v1:key|".to_vec(),
            ordering_hash_domain: b"fastpq:execution-effects:v1:ordering|".to_vec(),
            quantity_value_format: 1,
            effect_discriminants: [0, 1, 2, 3],
        }
    }
    fn profile_id(&self) -> FastpqCompactProfileIdV1 {
        FastpqCompactProfileIdV1(
            Sha256::digest(
                norito::encode_canonical(self).expect("bounded fixed execution-effect descriptor"),
            )
            .into(),
        )
    }
}
/// Canonical complete-effect ordinary profile, distinct from the unchanged AXT profile.
pub(in crate::backend) fn execution_effect_profile_id() -> FastpqCompactProfileIdV1 {
    DeepExecutionEffectArtifactProfile::fixed().profile_id()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effect_profile_is_distinct_and_retains_exact_axt_kernel_descriptor() {
        let original = DeepQuantityArtifactProfile::fixed();
        let original_bytes = norito::encode_canonical(&original).unwrap();
        let effect = DeepExecutionEffectArtifactProfile::fixed();
        assert_eq!(
            norito::encode_canonical(&effect.proof_kernel_descriptor).unwrap(),
            original_bytes
        );
        assert_eq!(
            effect.proof_kernel_descriptor.profile_id(),
            quantity_diagnostic_profile_id()
        );
        assert_eq!(effect.profile_id(), execution_effect_profile_id());
        assert_ne!(effect.profile_id(), quantity_diagnostic_profile_id());
        for flags in [0, 1, 2, 3] {
            let _flags = norito::core::DecodeFlagsGuard::enter(flags);
            assert_eq!(execution_effect_profile_id(), effect.profile_id());
            assert_eq!(
                norito::encode_canonical(&DeepQuantityArtifactProfile::fixed()).unwrap(),
                original_bytes
            );
        }
    }
    #[test]
    fn every_effect_profile_field_is_bound() {
        let baseline = DeepExecutionEffectArtifactProfile::fixed();
        let expected = baseline.profile_id();
        for mutation in 0..21 {
            let mut changed = baseline.clone();
            match mutation {
                0 => changed.version += 1,
                1 => changed.proof_kernel_descriptor.query_count += 1,
                2 => changed.ordinary_relation = "axt must not select ordinary effects",
                3 => changed.ordinary_artifact_schema.push('x'),
                4 => changed.ordinary_artifact_frame_hash[0] ^= 1,
                5 => changed.statement_schema.push('x'),
                6 => changed.statement_frame_hash[0] ^= 1,
                7 => changed.source_schema.push('x'),
                8 => changed.source_frame_hash[0] ^= 1,
                9 => changed.effect_kind_frame_hash[0] ^= 1,
                10 => changed.batch_context_frame_hash[0] ^= 1,
                11 => changed.segment_context_frame_hash[0] ^= 1,
                12 => changed.bundle_schema.push('x'),
                13 => changed.bundle_frame_hash[0] ^= 1,
                14 => changed.typed_key_domain.push(1),
                15 => changed.statement_digest_domain.push(1),
                16 => changed.quantity_value_format += 1,
                17 => changed.effect_discriminants.swap(0, 1),
                18 => changed.proof_kernel_descriptor.trace_mask_coefficients += 1,
                19 => changed.key_hash_domain.push(1),
                20 => changed.ordering_hash_domain.push(1),
                _ => unreachable!(),
            }
            assert_ne!(
                changed.profile_id(),
                expected,
                "unbound field mutation {mutation}"
            );
        }
    }
}
