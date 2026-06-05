//! Kagemusha recursive spend D2D payload-size benchmarks.

#![allow(clippy::option_if_let_else, clippy::too_many_lines)]

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use iroha_crypto::Hash;
use iroha_data_model::{
    ChainId,
    asset::AssetDefinitionId,
    domain::DomainId,
    offline::{
        KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS, KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1, KagemushaFoldStep,
        KagemushaRecursiveAggregationEvidence, KagemushaRecursiveAggregationProof,
        KagemushaRecursiveSpendAccumulatorV1, KagemushaRecursiveSpendBundleV1,
        KagemushaRecursiveSpendLineageAppendOpeningPreflightV1,
        KagemushaRecursiveSpendTransitionProfileV1, KagemushaRecursiveVerifierPreflightV1,
        KagemushaSpendableNoteDescriptorV1, kagemusha_recursive_aggregation_evidence_from_steps,
        kagemusha_recursive_previous_proof_open_envelope_metadata,
        kagemusha_recursive_previous_proof_open_envelopes_archive_digest,
        kagemusha_recursive_spend_accumulator_append_evidence,
        kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract,
        kagemusha_recursive_spend_accumulator_digest,
        kagemusha_recursive_spend_accumulator_from_initial_evidence,
        kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile,
        kagemusha_recursive_spend_proof_artifact_digest,
        kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight,
        kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract,
        kagemusha_recursive_spend_transition_profile_from_initial_evidence,
        kagemusha_verifier_key_poseidon_digest,
    },
    proof::{ProofBox, VerifyingKeyId},
    zk::{BackendTag, OpenVerifyEnvelope},
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

fn spend_proof_with_open_envelope(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> KagemushaRecursiveAggregationProof {
    let mut proof = spend_proof(accumulator);
    attach_open_envelope(
        &mut proof,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        format!("recursive-spend-bench-proof-vk-{}", accumulator.hop_count).as_bytes(),
    );
    proof
}

fn spend_lineage_proof_with_open_envelope(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    scalar_projection_label: &[u8],
    vk_label: &[u8],
) -> KagemushaRecursiveAggregationProof {
    let mut proof = spend_proof(accumulator);
    let max_hops = u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS).expect("max hops fits u32");
    let lineage_circuit_id = match accumulator.hop_count {
        1 => KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        hop_count if (2..=max_hops).contains(&hop_count) => {
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        }
        hop_count => panic!("unsupported reserved-lineage benchmark hop count {hop_count}"),
    };
    lineage_circuit_id.clone_into(&mut proof.verifier_key_id.name);
    proof
        .public_inputs
        .recursive_verifier_scalar_projection_digest = fixed_hash(scalar_projection_label);
    proof.public_inputs_hash = proof
        .public_inputs
        .public_inputs_hash()
        .expect("lineage public-input hash");
    attach_open_envelope(&mut proof, lineage_circuit_id, vk_label);
    proof
}

fn attach_open_envelope(
    proof: &mut KagemushaRecursiveAggregationProof,
    circuit_id: &str,
    vk_label: &[u8],
) {
    proof.proof.bytes = norito::to_bytes(&OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: fixed_hash(vk_label),
        public_inputs: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA.to_vec(),
        proof_bytes: vec![0xA5; 64],
        aux: Vec::new(),
    })
    .expect("encode recursive spend proof envelope");
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

fn previous_proof_open_envelope_archive(
    previous_bundle: &KagemushaRecursiveSpendBundleV1,
    label: u8,
) -> Vec<u8> {
    let metadata = kagemusha_recursive_previous_proof_open_envelope_metadata(previous_bundle)
        .expect("previous proof opening metadata");
    norito::to_bytes(&vec![iroha_zkp_halo2::OpenVerifyEnvelope {
        params: iroha_zkp_halo2::IpaParams {
            version: 1,
            curve_id: 1,
            n: 2,
            g: vec![[label; 32], [label.wrapping_add(1); 32]],
            h: vec![[label.wrapping_add(2); 32], [label.wrapping_add(3); 32]],
            u: [label.wrapping_add(4); 32],
        },
        public: iroha_zkp_halo2::PolyOpenPublic {
            version: 1,
            curve_id: 1,
            n: 2,
            z: [label.wrapping_add(5); 32],
            t: [label.wrapping_add(6); 32],
            p_g: [label.wrapping_add(7); 32],
        },
        proof: iroha_zkp_halo2::IpaProofData {
            version: 1,
            l: vec![[label.wrapping_add(8); 32]],
            r: vec![[label.wrapping_add(9); 32]],
            a_final: [label.wrapping_add(10); 32],
            b_final: [label.wrapping_add(11); 32],
        },
        transcript_label: format!("recursive-spend-bench-previous-proof-{label}"),
        vk_commitment: metadata.vk_commitment,
        public_inputs_schema_hash: metadata.public_inputs_schema_hash,
        domain_tag: metadata.domain_tag,
    }])
    .expect("encode previous proof opening archive")
}

fn recursive_verifier_preflight_for_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
    aggregate_digest: [u8; 32],
) -> KagemushaRecursiveVerifierPreflightV1 {
    KagemushaRecursiveVerifierPreflightV1 {
        proof_count: 1,
        verifier_witness_profile: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1.to_owned(),
        opening_len: evidence.verifier_opening_len,
        params_fingerprint: evidence.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: evidence.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: evidence
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest: evidence.fixed_window_table_base_digest,
        aggregate_digest,
    }
}

fn append_opening_preflight_contract(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    previous_recursive_proof_open_envelopes_archive: &[u8],
    evidence: &KagemushaRecursiveAggregationEvidence,
    hop_index: usize,
) -> KagemushaRecursiveSpendLineageAppendOpeningPreflightV1 {
    KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
        recursive_verifier_preflight_for_evidence(
            evidence,
            fixed_hash(format!("recursive-spend-bench-previous-opening-{hop_index}").as_bytes()),
        ),
        recursive_verifier_preflight_for_evidence(evidence, evidence.verifier_witness_batch_digest),
        kagemusha_recursive_spend_accumulator_digest(previous)
            .expect("previous accumulator digest"),
        kagemusha_recursive_spend_proof_artifact_digest(previous_recursive_proof)
            .expect("previous proof artifact digest"),
        kagemusha_recursive_previous_proof_open_envelopes_archive_digest(
            previous_recursive_proof_open_envelopes_archive,
        )
        .expect("previous proof opening archive digest"),
        evidence.aggregation_statement.steps[0].proof_hash,
    )
    .expect("append opening preflight contract")
}

struct RecursiveSpendArchiveSample {
    hop_count: usize,
    bundle_archive: Vec<u8>,
    transition_profile_archive: Vec<u8>,
}

struct ReservedLineageArchiveSample {
    hop_count: usize,
    bundle_archive: Vec<u8>,
    transition_profile_archive: Vec<u8>,
}

fn recursive_spend_archives() -> Vec<RecursiveSpendArchiveSample> {
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
        let output_seed =
            u8::try_from(0x40 + hop_index * 2).expect("output seed fits without reuse");
        let mut step = kagemusha_step(
            previous.as_ref().map_or(root_before, |acc| acc.final_root),
            root_after,
            u8::try_from(0x20 + hop_index).expect("input seed fits"),
            output_seed,
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
        let accumulator = match previous.as_ref() {
            Some(previous) => kagemusha_recursive_spend_accumulator_append_evidence(
                previous,
                previous_proof
                    .as_ref()
                    .expect("previous recursive spend proof"),
                &evidence,
                &note,
            )
            .expect("append recursive spend accumulator"),
            None => kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                .expect("initial recursive spend accumulator"),
        };
        let transition_profile = match previous.as_ref() {
            Some(previous) => {
                let profile_previous_proof = spend_proof_with_open_envelope(previous);
                let previous_bundle = KagemushaRecursiveSpendBundleV1 {
                    accumulator: previous.clone(),
                    recursive_proof: profile_previous_proof.clone(),
                };
                let previous_openings_archive = previous_proof_open_envelope_archive(
                    &previous_bundle,
                    u8::try_from(hop_index).expect("benchmark hop label fits"),
                );
                let append_opening_preflight_digest = fixed_hash(
                    format!("recursive-spend-bench-append-opening-preflight-{hop_index}")
                        .as_bytes(),
                );
                kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight(
                    previous,
                    &profile_previous_proof,
                    &previous_openings_archive,
                    append_opening_preflight_digest,
                    &evidence,
                    &note,
                )
                .expect("append recursive spend transition profile")
            }
            None => {
                kagemusha_recursive_spend_transition_profile_from_initial_evidence(&evidence, &note)
                    .expect("initial recursive spend transition profile")
            }
        };
        let hop_count = usize::try_from(accumulator.hop_count).expect("hop count fits");
        if target_hops.contains(&hop_count) {
            let bundle = spend_bundle(accumulator.clone());
            bundle
                .validate_public_input_binding()
                .expect("recursive spend bundle binding");
            transition_profile
                .validate_context()
                .expect("recursive spend transition profile binding");
            if hop_count > 1 {
                assert!(
                    transition_profile
                        .previous_recursive_proof_open_envelopes_archive_digest
                        .is_some(),
                    "append transition profile benchmark must bind previous proof openings"
                );
                assert!(
                    transition_profile.append_opening_preflight_digest.is_some(),
                    "append transition profile benchmark must bind append opening preflight"
                );
            }
            observed.push(RecursiveSpendArchiveSample {
                hop_count,
                bundle_archive: norito::to_bytes(&bundle).expect("encode recursive spend bundle"),
                transition_profile_archive: norito::to_bytes(&transition_profile)
                    .expect("encode recursive spend transition profile"),
            });
        }
        previous_proof = Some(spend_proof(&accumulator));
        previous = Some(accumulator);
    }

    let first_len = observed
        .first()
        .map(|sample| sample.bundle_archive.len())
        .expect("at least one benchmark archive");
    for sample in &observed {
        assert_eq!(
            sample.bundle_archive.len(),
            first_len,
            "recursive Kagemusha payload grew at hop {}",
            sample.hop_count
        );
    }
    let append_profile_len = observed
        .iter()
        .find(|sample| sample.hop_count > 1)
        .map(|sample| sample.transition_profile_archive.len())
        .expect("at least one append transition profile archive");
    for sample in observed.iter().filter(|sample| sample.hop_count > 1) {
        assert_eq!(
            sample.transition_profile_archive.len(),
            append_profile_len,
            "recursive Kagemusha append transition profile grew at hop {}",
            sample.hop_count
        );
    }
    observed
}

fn recursive_spend_reserved_lineage_archives() -> Vec<ReservedLineageArchiveSample> {
    let chain_id: ChainId = "kagemusha-recursive-spend-reserved-bench"
        .parse()
        .expect("chain id");
    let asset = kagemusha_asset("kgm-recursive-spend-reserved-bench");
    let target_hops = [1usize, 2, 3, 5, 8, 13, 21, 34, 55, 64];
    let mut observed = Vec::new();
    let mut previous = None::<KagemushaRecursiveSpendAccumulatorV1>;
    let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;

    for hop_index in 0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        let root_before =
            fixed_hash(format!("reserved-lineage-spend-bench-root-{hop_index}").as_bytes());
        let root_after =
            fixed_hash(format!("reserved-lineage-spend-bench-root-{}", hop_index + 1).as_bytes());
        let proof_label = format!("reserved-lineage-spend-bench-hop-{hop_index}");
        let output_seed =
            u8::try_from(0x40 + hop_index * 2).expect("output seed fits without reuse");
        let mut step = kagemusha_step(
            previous.as_ref().map_or(root_before, |acc| acc.final_root),
            root_after,
            u8::try_from(0x20 + hop_index).expect("input seed fits"),
            output_seed,
            proof_label.as_bytes(),
        );
        if let Some(previous) = previous.as_ref() {
            step.input_nullifiers = vec![previous.current_note.spend_nullifier];
        }
        let note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step.output_commitments[0],
            spend_nullifier: fixed_hash(
                format!("reserved-lineage-spend-bench-nullifier-{hop_index}").as_bytes(),
            ),
            amount: Numeric::new(42, 0),
        };
        let evidence = kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &[step],
            4,
            fixed_hash(b"reserved-lineage-spend-bench-pallas-params"),
            fixed_hash(b"reserved-lineage-spend-bench-fixed-window-schedule"),
            fixed_hash(b"reserved-lineage-spend-bench-fixed-window-shared-manifest"),
            fixed_hash(b"reserved-lineage-spend-bench-fixed-window-bases"),
            fixed_hash(format!("reserved-lineage-spend-bench-witness-{hop_index}").as_bytes()),
        )
        .expect("reserved-lineage recursive spend evidence");

        let (accumulator, transition_profile) = match previous.as_ref() {
            Some(previous) => {
                let previous_recursive_proof = previous_proof
                    .as_ref()
                    .expect("previous reserved-lineage recursive spend proof");
                let previous_bundle = KagemushaRecursiveSpendBundleV1 {
                    accumulator: previous.clone(),
                    recursive_proof: previous_recursive_proof.clone(),
                };
                let previous_openings_archive = previous_proof_open_envelope_archive(
                    &previous_bundle,
                    u8::try_from(hop_index).expect("benchmark hop label fits"),
                );
                let append_opening_preflight = append_opening_preflight_contract(
                    previous,
                    previous_recursive_proof,
                    &previous_openings_archive,
                    &evidence,
                    hop_index,
                );
                let accumulator =
                    kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract(
                        previous,
                        previous_recursive_proof,
                        &previous_openings_archive,
                        append_opening_preflight.clone(),
                        &evidence,
                        &note,
                    )
                    .expect("append reserved-lineage accumulator with compact boundary");
                let transition_profile =
                    kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                        previous,
                        previous_recursive_proof,
                        &previous_openings_archive,
                        append_opening_preflight,
                        &evidence,
                        &note,
                    )
                    .expect("append reserved-lineage transition profile with compact boundary");
                let append_boundary =
                    kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
                        &transition_profile,
                    )
                    .expect("reserved-lineage append boundary");
                assert_eq!(
                    accumulator.append_boundary_digest, append_boundary.append_boundary_digest,
                    "reserved-lineage benchmark accumulator must carry compact append boundary"
                );
                (accumulator, transition_profile)
            }
            None => (
                kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                    .expect("initial reserved-lineage recursive spend accumulator"),
                kagemusha_recursive_spend_transition_profile_from_initial_evidence(
                    &evidence, &note,
                )
                .expect("initial reserved-lineage recursive spend transition profile"),
            ),
        };

        let proof = spend_lineage_proof_with_open_envelope(
            &accumulator,
            format!("reserved-lineage-spend-bench-scalar-{hop_index}").as_bytes(),
            format!("reserved-lineage-spend-bench-vk-{hop_index}").as_bytes(),
        );
        let hop_count = usize::try_from(accumulator.hop_count).expect("hop count fits");
        if target_hops.contains(&hop_count) {
            let bundle = KagemushaRecursiveSpendBundleV1 {
                accumulator: accumulator.clone(),
                recursive_proof: proof.clone(),
            };
            bundle
                .validate_public_input_binding()
                .expect("reserved-lineage recursive spend bundle binding");
            transition_profile
                .validate_context()
                .expect("reserved-lineage transition profile binding");
            if hop_count > 1 {
                assert_ne!(
                    bundle.accumulator.append_boundary_digest, [0u8; 32],
                    "reserved-lineage append payload must carry compact boundary digest"
                );
            }
            observed.push(ReservedLineageArchiveSample {
                hop_count,
                bundle_archive: norito::to_bytes(&bundle)
                    .expect("encode reserved-lineage recursive spend bundle"),
                transition_profile_archive: norito::to_bytes(&transition_profile)
                    .expect("encode reserved-lineage recursive spend transition profile"),
            });
        }
        previous_proof = Some(proof);
        previous = Some(accumulator);
    }

    let first_len = observed
        .first()
        .map(|sample| sample.bundle_archive.len())
        .expect("at least one reserved-lineage benchmark archive");
    for sample in &observed {
        assert_eq!(
            sample.bundle_archive.len(),
            first_len,
            "reserved-lineage recursive Kagemusha payload grew at hop {}",
            sample.hop_count
        );
    }
    let append_profile_len = observed
        .iter()
        .find(|sample| sample.hop_count > 1)
        .map(|sample| sample.transition_profile_archive.len())
        .expect("at least one reserved-lineage append transition profile archive");
    for sample in observed.iter().filter(|sample| sample.hop_count > 1) {
        assert_eq!(
            sample.transition_profile_archive.len(),
            append_profile_len,
            "reserved-lineage recursive Kagemusha append transition profile grew at hop {}",
            sample.hop_count
        );
    }
    observed
}

fn recursive_spend_payload_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kagemusha_recursive_spend_payload_bytes");
    for sample in recursive_spend_archives() {
        let parameter = format!(
            "{}_hops_{}_bytes",
            sample.hop_count,
            sample.bundle_archive.len()
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(parameter),
            &sample.bundle_archive,
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

fn recursive_spend_transition_profile_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kagemusha_recursive_spend_transition_profile_bytes");
    for sample in recursive_spend_archives() {
        let parameter = format!(
            "{}_hops_{}_bytes",
            sample.hop_count,
            sample.transition_profile_archive.len()
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(parameter),
            &sample.transition_profile_archive,
            |b, archive| {
                b.iter(|| {
                    let decoded: KagemushaRecursiveSpendTransitionProfileV1 =
                        norito::decode_from_bytes(black_box(archive)).expect("decode archive");
                    black_box(decoded);
                });
            },
        );
    }
    group.finish();
}

fn recursive_spend_reserved_lineage_payload_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kagemusha_recursive_spend_reserved_lineage_payload_bytes");
    for sample in recursive_spend_reserved_lineage_archives() {
        let parameter = format!(
            "{}_hops_{}_bytes",
            sample.hop_count,
            sample.bundle_archive.len()
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(parameter),
            &sample.bundle_archive,
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

fn recursive_spend_reserved_lineage_transition_profile_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("kagemusha_reserved_lineage_transition_profile_bytes");
    for sample in recursive_spend_reserved_lineage_archives() {
        let parameter = format!(
            "{}_hops_{}_bytes",
            sample.hop_count,
            sample.transition_profile_archive.len()
        );
        group.bench_with_input(
            BenchmarkId::from_parameter(parameter),
            &sample.transition_profile_archive,
            |b, archive| {
                b.iter(|| {
                    let decoded: KagemushaRecursiveSpendTransitionProfileV1 =
                        norito::decode_from_bytes(black_box(archive)).expect("decode archive");
                    black_box(decoded);
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    recursive_spend_payload_bytes,
    recursive_spend_transition_profile_bytes,
    recursive_spend_reserved_lineage_payload_bytes,
    recursive_spend_reserved_lineage_transition_profile_bytes
);
criterion_main!(benches);
