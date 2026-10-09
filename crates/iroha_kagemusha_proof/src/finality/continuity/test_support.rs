//! Test-only original source-key qualification for descriptor-sized layouts.
//!
//! This imports actual serialized proving tables against the sealed compiled
//! source and its exact wrapper. It creates no live proof, authenticates no
//! interval and grants no finality authority to placeholder witness values.

use ff::Field;
use iroha_pasta::{Ep, Eq, Fp, msm::MemoryBudget};
use iroha_plonk::{
    cs::InstanceType,
    frontend::Circuit,
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2, pk::artifact::ReadConfig},
    pcs::ipa::PinnedParams,
};
use iroha_plonk_recursion::FOLD_WITNESS_BYTES;

use super::{
    SourceVerifier,
    producer::{self, OriginalArtifact, SourceCircuit},
};
use crate::omega::{OmegaCircuit, OmegaPlan, OmegaWitness};

/// Import original serialized proving tables for the actual source and its exact
/// wrapper. No hand-constructed source capability or live proof is used.
pub fn qualified<C: SourceCircuit>(
    circuit: &C,
    pallas: &PinnedParams<Ep>,
    vesta: &PinnedParams<Eq>,
) -> SourceVerifier {
    let mut config = KeygenConfigV2::pipa_r(vec![InstanceType::Bounded]);
    config.compress_selectors = false;
    let source = keygen_pk_v2(vesta, &circuit.without_witnesses(), &config).unwrap();
    let wrapper_plan = OmegaPlan::new(
        source.binding().clone(),
        vesta.clone(),
        vec![source.vk().kagemusha_digest(source.binding()).unwrap()],
    )
    .unwrap()
    .with_key_catalog(vec![source.vk().clone()])
    .unwrap();
    let length = wrapper_plan.verifier().proof_length();
    let wrapper = OmegaCircuit::new(
        wrapper_plan,
        OmegaWitness {
            key: source.vk().clone(),
            instances: vec![Fp::ZERO; 69],
            proof: vec![0; length],
            length: u32::try_from(length).unwrap(),
            fold: [0; FOLD_WITNESS_BYTES],
        },
    )
    .unwrap()
    .without_witnesses();
    let outer = keygen_pk_v2(
        pallas,
        &wrapper,
        &KeygenConfigV2::pipa_r(OmegaPlan::instance_types().to_vec()),
    )
    .unwrap();
    let source_descriptor = source.binding().encoded().to_vec();
    let source_verifying = source.vk().to_bytes().to_vec();
    let source_proving = source.artifact_bytes_v2().unwrap();
    let outer_descriptor = outer.binding().encoded().to_vec();
    let outer_verifying = outer.vk().to_bytes().to_vec();
    let outer_proving = outer.artifact_bytes_v2().unwrap();
    drop((source, outer));
    let imported = producer::Prover::from_original_artifacts(
        circuit,
        OriginalArtifact {
            descriptor: &source_descriptor,
            verifying_key: &source_verifying,
            proving_key: &source_proving,
        },
        OriginalArtifact {
            descriptor: &outer_descriptor,
            verifying_key: &outer_verifying,
            proving_key: &outer_proving,
        },
        pallas.clone(),
        vesta.clone(),
        ReadConfig {
            maximum_bytes: source_proving.len().max(outer_proving.len()),
            maximum_rows: 1 << 16,
            coset_cache: CosetCachePolicy::OnDemand,
            msm_budget: MemoryBudget::DEFAULT,
        },
    )
    .unwrap();
    imported.qualified_source().unwrap()
}
