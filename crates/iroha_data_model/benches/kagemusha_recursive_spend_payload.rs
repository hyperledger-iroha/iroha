//! Kagemusha recursive spend D2D payload-size benchmarks.

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId,
    asset::AssetDefinitionId,
    domain::DomainId,
    offline::{
        KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS, KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KagemushaFoldStep, KagemushaRecursiveAggregationProof,
        KagemushaRecursiveSpendAccumulatorV1, KagemushaRecursiveSpendBundleV1,
        KagemushaSpendableNoteDescriptorV1, kagemusha_recursive_aggregation_evidence_from_steps,
        kagemusha_recursive_spend_accumulator_append_evidence,
        kagemusha_recursive_spend_accumulator_from_initial_evidence,
        kagemusha_verifier_key_poseidon_digest,
    },
    proof::{ProofBox, VerifyingKeyId},
};
use iroha_primitives::numeric::Numeric;

fn fixed_hash(label: &[u8]) -> [u8; 32] {
    Hash::new(label).into()
}

fn kagemusha_asset(name: &str) -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("offline", "universal").expect("domain id"),
        name.parse().expect("asset definition name"),
    )
}

fn kagemusha_step(
    root_before: [u8; 32],
    root_after: [u8; 32],
    input_seed: u8,
    output_seed: u8,
    proof_label: &[u8],
) -> KagemushaFoldStep {
    let mut proof_inputs_label = proof_label.to_vec();
    proof_inputs_label.extend_from_slice(b":public-inputs");
    KagemushaFoldStep {
        root_before,
        input_nullifiers: vec![[input_seed.wrapping_add(1); 32], [input_seed; 32]],
        output_commitments: vec![[output_seed.wrapping_add(1); 32], [output_seed; 32]],
        root_after,
        proof_hash: Hash::new(proof_label),
        proof_public_inputs_digest: fixed_hash(&proof_inputs_label),
        verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture"),
        verifier_key_commitment: fixed_hash(proof_label),
        verifier_key_poseidon_digest: kagemusha_verifier_key_poseidon_digest(
            "halo2/ipa",
            proof_label,
        )
        .expect("verifier-key digest"),
    }
}

fn spend_proof(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> KagemushaRecursiveAggregationProof {
    let public_inputs = accumulator
        .recursive_public_inputs()
        .expect("recursive spend public inputs");
    let public_inputs_hash = public_inputs
        .public_inputs_hash()
        .expect("recursive spend public-input hash");
    KagemushaRecursiveAggregationProof {
        verifier_key_id: VerifyingKeyId::new(
            "halo2/ipa",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        ),
        public_inputs,
        public_inputs_hash,
        proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 256]),
    }
}

fn spend_bundle(
    accumulator: KagemushaRecursiveSpendAccumulatorV1,
) -> KagemushaRecursiveSpendBundleV1 {
    let recursive_proof = spend_proof(&accumulator);
    KagemushaRecursiveSpendBundleV1 {
        accumulator,
        recursive_proof,
    }
}

fn recursive_spend_archives() -> Vec<(usize, Vec<u8>)> {
    let chain_id: ChainId = "kagemusha-recursive-spend-bench".parse().expect("chain id");
    let asset = kagemusha_asset("kgm-recursive-spend-bench");
    let target_hops = [1usize, 2, 3, 5, 8, 13, 21, 34, 55, 64];
    let mut observed = Vec::new();
    let mut previous = None::<KagemushaRecursiveSpendAccumulatorV1>;
    let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;

    for hop_index in 0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        let root_before = fixed_hash(format!("recursive-spend-bench-root-{hop_index}").as_bytes());
        let root_after =
            fixed_hash(format!("recursive-spend-bench-root-{}", hop_index + 1).as_bytes());
        let proof_label = format!("recursive-spend-bench-hop-{hop_index}");
        let mut step = kagemusha_step(
            previous.as_ref().map_or(root_before, |acc| acc.final_root),
            root_after,
            u8::try_from(0x20 + hop_index).expect("input seed fits"),
            u8::try_from(0x80 + hop_index).expect("output seed fits"),
            proof_label.as_bytes(),
        );
        if let Some(previous) = previous.as_ref() {
            step.input_nullifiers = vec![previous.current_note.spend_nullifier];
        }
        let note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step.output_commitments[0],
            spend_nullifier: fixed_hash(
                format!("recursive-spend-bench-nullifier-{hop_index}").as_bytes(),
            ),
            amount: Numeric::new(42, 0),
        };
        let evidence = kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &[step],
            4,
            fixed_hash(b"recursive-spend-bench-pallas-params"),
            fixed_hash(b"recursive-spend-bench-fixed-window-schedule"),
            fixed_hash(b"recursive-spend-bench-fixed-window-shared-manifest"),
            fixed_hash(b"recursive-spend-bench-fixed-window-bases"),
            fixed_hash(format!("recursive-spend-bench-witness-{hop_index}").as_bytes()),
        )
        .expect("recursive spend evidence");
        let accumulator = previous.as_ref().map_or_else(
            || {
                kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                    .expect("initial recursive spend accumulator")
            },
            |previous| {
                kagemusha_recursive_spend_accumulator_append_evidence(
                    previous,
                    previous_proof
                        .as_ref()
                        .expect("previous recursive spend proof"),
                    &evidence,
                    &note,
                )
                .expect("append recursive spend accumulator")
            },
        );
        let hop_count = usize::try_from(accumulator.hop_count).expect("hop count fits");
        if target_hops.contains(&hop_count) {
            let bundle = spend_bundle(accumulator.clone());
            bundle
                .validate_public_input_binding()
                .expect("recursive spend bundle binding");
            observed.push((
                hop_count,
                norito::to_bytes(&bundle).expect("encode recursive spend bundle"),
            ));
        }
        previous_proof = Some(spend_proof(&accumulator));
        previous = Some(accumulator);
    }

    let first_len = observed
        .first()
        .map(|(_, bytes)| bytes.len())
        .expect("at least one benchmark archive");
    for (hop_count, bytes) in &observed {
        assert_eq!(
            bytes.len(),
            first_len,
            "recursive Kagemusha payload grew at hop {hop_count}"
        );
    }
    observed
}

fn recursive_spend_payload_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kagemusha_recursive_spend_payload_bytes");
    for (hop_count, bytes) in recursive_spend_archives() {
        let parameter = format!("{hop_count}_hops_{}_bytes", bytes.len());
        group.bench_with_input(
            BenchmarkId::from_parameter(parameter),
            &bytes,
            |b, archive| {
                b.iter(|| {
                    let decoded: KagemushaRecursiveSpendBundleV1 =
                        norito::decode_from_bytes(black_box(archive)).expect("decode archive");
                    black_box(decoded);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, recursive_spend_payload_bytes);
criterion_main!(benches);
