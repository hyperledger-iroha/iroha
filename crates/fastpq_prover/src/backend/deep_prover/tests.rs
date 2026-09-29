//! Whole-attempt budget/entropy controls and an explicit full-size native diagnostic.

use super::*;
use crate::{
    backend::compact_transfer_air::CompactTransferAir,
    gadgets::compact_smt_air::{PublicStatement, PublicUpdate},
};
use rand::{SeedableRng, TryRngCore, rngs::StdRng};

#[path = "diagnostic_artifact.rs"]
mod diagnostic_artifact;

const COMPLETE_CONTEXT: &[u8] = b"native producer diagnostic";
const COMPLETE_BYTES: usize = 482_978;
const COMPLETE_HASH: &str = "7d16efc5143e19aa9fe7d1c9d37605741fb338953ff282f91b3ab72af5509507";

fn limits() -> ConstructionLimits {
    ConstructionLimits {
        digest_execution: DigestExecutionV1::Cpu,
        max_payload_bytes: usize::MAX,
        max_work_units: usize::MAX,
        max_hash_calls: usize::MAX,
        max_proof_bytes: deep_proof::PROOF_BYTE_TARGET,
    }
}
fn digest(seed: u8) -> [u32; 8] {
    limbs(iroha_crypto::Hash::new([seed; 33]).as_ref())
}
fn limbs(bytes: &[u8; 32]) -> [u32; 8] {
    core::array::from_fn(|i| u32::from_le_bytes(bytes[4 * i..4 * i + 4].try_into().unwrap()))
}
fn statement() -> PublicStatement {
    PublicStatement {
        updates: [
            PublicUpdate {
                old_leaf: digest(1),
                new_leaf: digest(2),
                path: 7,
            },
            PublicUpdate {
                old_leaf: digest(3),
                new_leaf: digest(4),
                path: 11,
            },
        ],
        old_root: digest(5),
        new_root: digest(6),
    }
}
struct NoEntropy(usize);
impl TryRngCore for NoEntropy {
    type Error = &'static str;
    fn try_next_u32(&mut self) -> std::result::Result<u32, Self::Error> {
        self.0 += 1;
        Err("unexpected entropy")
    }
    fn try_next_u64(&mut self) -> std::result::Result<u64, Self::Error> {
        self.0 += 1;
        Err("unexpected entropy")
    }
    fn try_fill_bytes(&mut self, _: &mut [u8]) -> std::result::Result<(), Self::Error> {
        self.0 += 1;
        Err("unexpected entropy")
    }
}
impl TryCryptoRng for NoEntropy {}

#[test]
#[cfg(feature = "fastpq-gpu")]
fn required_device_failure_precedes_source_reading_and_entropy() {
    let air = CompactTransferAir::new(&statement(), None).unwrap();
    let plan = ProducerPlan::new(
        &air,
        ConstructionLimits {
            digest_execution: DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Cuda),
            ..limits()
        },
    )
    .unwrap();
    let mut rng = NoEntropy(0);
    assert!(matches!(
        plan.build_from_borrowed_for_test(&[], &mut rng),
        Err(Error::NativeDigestExecution { .. })
    ));
    assert_eq!(rng.0, 0);
}

#[test]
fn whole_attempt_preflight_binds_every_budget_before_entropy_or_private_allocation() {
    let air = CompactTransferAir::new(&statement(), None).unwrap();
    let plan = ProducerPlan::new(&air, limits()).unwrap();
    eprintln!(
        "deep_complete_producer_payload_bytes={}; structural_work_units={}; hash_calls={}",
        plan.payload_bytes, plan.work_units, plan.hash_calls
    );
    assert!(plan.payload_bytes < 2 * 1024 * 1024 * 1024);
    assert!(plan.payload_bytes > plan.quotient.payload_bytes);
    assert!(plan.work_units > plan.quotient.work_units);
    assert!(plan.hash_calls > 2 * (2 * LDE_ROWS - 1));
    assert!(plan.hash_calls < 3 * (2 * LDE_ROWS - 1));
    assert!(plan.payload_bytes > 1_800_000_000);
    let exact = ConstructionLimits {
        digest_execution: DigestExecutionV1::Cpu,
        max_payload_bytes: plan.payload_bytes,
        max_work_units: plan.work_units,
        max_hash_calls: plan.hash_calls,
        max_proof_bytes: deep_proof::MAX_FRAME_BYTES,
    };
    assert!(ProducerPlan::new(&air, exact).is_ok());
    for budget in [
        ConstructionLimits {
            max_payload_bytes: exact.max_payload_bytes - 1,
            ..exact
        },
        ConstructionLimits {
            max_work_units: exact.max_work_units - 1,
            ..exact
        },
        ConstructionLimits {
            max_hash_calls: exact.max_hash_calls - 1,
            ..exact
        },
        ConstructionLimits {
            max_proof_bytes: exact.max_proof_bytes - 1,
            ..exact
        },
    ] {
        assert!(ProducerPlan::new(&air, budget).is_err());
    }
    let mut rng = NoEntropy(0);
    // A valid plan cannot turn an absent source into entropy consumption or a tree.
    assert!(plan.build_from_borrowed_for_test(&[], &mut rng).is_err());
    assert_eq!(rng.0, 0);
    assert!(
        ProducerPlan::new(
            &air,
            ConstructionLimits {
                max_payload_bytes: 0,
                ..limits()
            }
        )
        .is_err()
    );
}

#[test]
fn retained_device_pool_remains_charged_during_the_larger_cpu_quotient_phase() {
    let pool = crate::gpu_memory::METAL_POOL_MAX_CACHED_BYTES;
    assert_eq!(
        active_phase_payload(2 * pool, pool + 8, pool + 16).unwrap(),
        3 * pool
    );
    // A larger digest stage includes this allowance already; do not add it twice.
    assert_eq!(
        active_phase_payload(8, 3 * pool, 2 * pool).unwrap(),
        3 * pool
    );
    assert_eq!(
        active_phase_payload(8, 2 * pool, 3 * pool).unwrap(),
        3 * pool
    );
    assert!(active_phase_payload(usize::MAX, 0, 0).is_err());
}

#[test]
fn frontier_envelope_and_replayed_root_equality_are_explicit() {
    let queries = maximal_queries();
    let plans = OpeningPlans::new(&queries).unwrap();
    assert_eq!(plans.initial.work().siblings, 1088);
    assert_eq!(
        plans.rounds.each_ref().map(|p| p.work().siblings),
        [832, 576, 384, 192, 64]
    );
    assert!(
        plans
            .round_indices
            .iter()
            .all(|positions| positions.len() == QUERY_COUNT)
    );
    let first = Digest::new([1, 2, 3, 5, 7, 11]).unwrap();
    let second = Digest::new([1, 2, 3, 5, 7, 13]).unwrap();
    same_root(first, first).unwrap();
    assert!(same_root(first, second).is_err());
    let context = Context::new(b"whole producer scheduling regression").unwrap();
    let mut transcript = Transcript::new(context);
    assert!(fields(&mut transcript, CONSTRAINTS).is_err());
    assert!(fields(&mut transcript, 1).is_err());
}

#[test]
fn canonical_output_writer_is_byte_exact_and_refuses_short_cap_before_output() {
    // Serializer-only fixture: empty vectors deliberately do not satisfy the
    // proof relation. Production construction preflights and verifies separately.
    let proof = DeepProof {
        row_root: Digest::default().into(),
        quotient_root: Digest::default().into(),
        fri_roots: Vec::new(),
        ood: OodAnswers {
            current: Vec::new(),
            next: Vec::new(),
            quotient: Vec::new(),
        },
        rows: Vec::new(),
        quotients: Vec::new(),
        row_siblings: Vec::new(),
        quotient_siblings: Vec::new(),
        rounds: Vec::new(),
        terminal: Vec::new(),
    };
    let expected = norito::encode_canonical(&proof).unwrap();
    assert_eq!(encode_bounded(&proof, expected.len()).unwrap(), expected);
    assert!(encode_bounded(&proof, expected.len() - 1).is_err());
    assert!(deep_proof::decode(&expected, deep_proof::PROOF_BYTE_TARGET).is_err());
}

#[test]
#[ignore = "explicit full 8M-row DEEP producer with retained public artifact; measure separately"]
fn complete_native_masked_deep_producer_roundtrip_and_statement_rejection() {
    complete_masked_producer(DigestExecutionV1::Cpu);
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "explicit complete 8M-row proof with required Metal leaves/lower parents; measure separately"]
fn complete_required_metal_masked_deep_producer_roundtrip_and_statement_rejection() {
    let _lane = crate::backend::acquire_gpu_lane();
    complete_masked_producer(DigestExecutionV1::Device(
        crate::Digest384GpuBackendV1::Metal,
    ));
}

fn complete_siblings() -> [[u32; 8]; 32] {
    core::array::from_fn(|level| digest((level + 17) as u8))
}

// Independent public fixture facts are derived before reading any artifact.
fn complete_statement() -> PublicStatement {
    let siblings = complete_siblings();
    let path = 0xa59c_71e3;
    let first = digest(1);
    let second = digest(2);
    let mut root = first;
    for (level, sibling) in siblings.iter().enumerate() {
        let (left, right) = if (path >> level) & 1 == 0 {
            (root, *sibling)
        } else {
            (*sibling, root)
        };
        let mut message = b"fastpq:v1:smt:node|".to_vec();
        for limb in left.into_iter().chain(right) {
            message.extend_from_slice(&limb.to_le_bytes());
        }
        root = limbs(iroha_crypto::Hash::new(message).as_ref());
    }
    PublicStatement {
        updates: [
            PublicUpdate {
                old_leaf: first,
                new_leaf: second,
                path,
            },
            PublicUpdate {
                old_leaf: second,
                new_leaf: first,
                path,
            },
        ],
        old_root: root,
        new_root: root,
    }
}

fn complete_masked_producer(execution: DigestExecutionV1) {
    use crate::gadgets::compact_smt_air::PhysicalSmtWitness;
    // Required-device failure must precede the diagnostic's private witness too.
    crate::digest384_batch::preflight_last_fields_execution(execution).unwrap();
    let statement = complete_statement();
    let siblings = complete_siblings();
    let air = CompactTransferAir::new(&statement, Some(COMPLETE_CONTEXT)).unwrap();
    let defaults = crate::backend::offline_compact::ProvingLimits::default();
    let plan = ProducerPlan::new(
        &air,
        ConstructionLimits {
            digest_execution: execution,
            max_payload_bytes: defaults.max_segment_charge_bytes,
            max_work_units: defaults.max_segment_work_units,
            max_hash_calls: defaults.max_segment_work_units,
            max_proof_bytes: deep_proof::PROOF_BYTE_TARGET,
        },
    )
    .unwrap();
    eprintln!(
        "complete DEEP attempt bound: payload={}, work={}, hashes={}",
        plan.payload_bytes, plan.work_units, plan.hash_calls
    );
    let witness = PhysicalSmtWitness::from_inputs(&statement, &[siblings, siblings]).unwrap();
    let columns = OwnedTraceSource::from_rows(witness.rows()).unwrap();
    drop(witness);
    let mut rng = StdRng::from_seed([83; 32]);
    let charges = (plan.payload_bytes, plan.work_units, plan.hash_calls);
    let started = std::time::Instant::now();
    let proof = plan.build(columns, &mut rng).unwrap();
    let elapsed = started.elapsed().as_secs_f64();
    eprintln!(
        "complete fixed-SMT DEEP proof: bytes={}; build_and_self_check_seconds={:.3}; proof_hash={}",
        proof.len(),
        elapsed,
        iroha_crypto::Hash::new(&proof),
    );
    // Retain only public outputs before later resource/golden/verifier assertions.
    use crate::backend::compact_protocol::FixedAir as _;
    let receipt = diagnostic_artifact::retain(
        &proof,
        air.statement_bytes(),
        COMPLETE_CONTEXT,
        elapsed,
        charges,
    )
    .unwrap();
    assert_complete_proof_controls(&statement, &air, &proof);
    diagnostic_artifact::mark_controls_passed(&receipt).unwrap();
}

fn assert_complete_proof_controls(
    statement: &PublicStatement,
    air: &CompactTransferAir,
    proof: &[u8],
) {
    assert!(proof.len() <= deep_proof::MAX_FRAME_BYTES);
    assert_eq!(
        proof.len(),
        COMPLETE_BYTES,
        "pre-cache complete seeded proof byte length"
    );
    assert_eq!(
        iroha_crypto::Hash::new(proof).to_string(),
        COMPLETE_HASH,
        "pre-cache complete seeded proof bytes"
    );
    assert_eq!(
        deep_engine::verify(air, proof, deep_proof::PROOF_BYTE_TARGET)
            .unwrap()
            .air_evaluations,
        1
    );
    assert!(deep_engine::verify(air, proof, proof.len() - 1).is_err());
    let other = CompactTransferAir::new(
        statement,
        Some(b"different authoritative statement context"),
    )
    .unwrap();
    assert!(deep_engine::verify(&other, proof, deep_proof::PROOF_BYTE_TARGET).is_err());
    let mut altered = proof.to_vec();
    let last = altered.len() - 1;
    altered[last] ^= 1;
    assert!(deep_engine::verify(air, &altered, deep_proof::PROOF_BYTE_TARGET).is_err());
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "bounded actual Metal leaf and CPU parent timing before a complete masked proof"]
fn measured_required_metal_leaf_and_cpu_parent_costs() {
    use std::time::Instant;

    use crate::backend::deep_leaf_batch;

    let _lane = crate::backend::acquire_gpu_lane();
    let execution = DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Metal);
    crate::digest384_batch::preflight_last_fields_execution(execution).unwrap();
    let air = CompactTransferAir::new(&statement(), Some(b"native producer diagnostic")).unwrap();
    let binding = Context::for_relation(&air).unwrap();
    let mut estimated_hash_seconds = 0.0;
    for oracle in [
        Oracle::Row,
        Oracle::QuotientAndMask,
        Oracle::Fri(0),
        Oracle::Fri(1),
        Oracle::Fri(2),
        Oracle::Fri(3),
        Oracle::Fri(4),
    ] {
        let (_, _, leaves, width) = oracle.shape().unwrap();
        let count = leaves.min(deep_leaf_batch::CAPACITY);
        let batches = 4096 / count;
        let payload = (0..count * width / 8)
            .flat_map(|value| (value as u64 + 7).to_le_bytes())
            .collect::<Vec<_>>();
        let indices = (0..count).collect::<Vec<_>>();
        let mut output = SecretPolynomial::<[u64; 6]>::zeroed(indices.len()).unwrap();
        // Warm this oracle's public prefix cache before measuring its fixed batch.
        deep_leaf_batch::hash(
            &binding,
            oracle,
            &indices,
            &payload,
            width,
            &mut output,
            execution,
        )
        .unwrap();
        let started = Instant::now();
        for _ in 0..batches {
            deep_leaf_batch::hash(
                &binding,
                oracle,
                &indices,
                &payload,
                width,
                &mut output,
                execution,
            )
            .unwrap();
        }
        let leaf_seconds = started.elapsed().as_secs_f64();
        for index in [0, indices.len() - 1] {
            assert_eq!(
                output[index],
                binding
                    .hash_leaf(
                        oracle,
                        index as u32,
                        &payload[index * width..(index + 1) * width]
                    )
                    .unwrap()
                    .words()
            );
        }
        let mut left = Digest::new([1, 2, 3, 5, 7, 11]).unwrap();
        let right = Digest::new([13, 17, 19, 23, 29, 31]).unwrap();
        left = binding.hash_parent(oracle, 1, 0, left, right).unwrap();
        let started = Instant::now();
        for index in 0..4096 {
            left = binding
                .hash_parent(oracle, 1, (index % (leaves / 2)) as u32, left, right)
                .unwrap();
        }
        std::hint::black_box(left);
        let parent_seconds = started.elapsed().as_secs_f64();
        let estimate = 2.0
            * (leaf_seconds * leaves as f64 / (batches * count) as f64
                + parent_seconds * (leaves - 1) as f64 / 4096.0);
        estimated_hash_seconds += estimate;
        eprintln!(
            "oracle={oracle:?}; leaf_samples={}; leaf_seconds={leaf_seconds:.6}; parent_samples=4096; parent_seconds={parent_seconds:.6}; two_tree_hash_seconds_estimate={estimate:.3}",
            batches * count
        );
    }
    eprintln!(
        "total_hash_seconds_estimate={estimated_hash_seconds:.3}; excludes transforms, AIR, coefficient work, terminal, verifier and allocation variance; no complete-proof measurement"
    );
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "bounded batch-size comparison of real Metal continuations"]
fn measured_required_metal_batch_sizes_separate_preparation_and_dispatch() {
    measure_required_metal_batch_sizes(&[32, 256, 1024], 4096);
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "bounded larger typed Metal dispatch measurement; production remains at its fixed capacity"]
fn measured_required_metal_larger_typed_batches_without_changing_production_capacity() {
    measure_required_metal_batch_sizes(&[4096, 8192], 8192);
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
fn measure_required_metal_batch_sizes(counts: &[usize], samples: usize) {
    use std::time::Instant;

    use rayon::prelude::*;

    use crate::digest384_batch::execute_last_fields_with_cpu;

    let _lane = crate::backend::acquire_gpu_lane();
    let execution = DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Metal);
    crate::digest384_batch::preflight_last_fields_execution(execution).unwrap();
    let air = CompactTransferAir::new(&statement(), Some(b"native producer diagnostic")).unwrap();
    let binding = Context::for_relation(&air).unwrap();
    let left = Digest::new([1, 2, 3, 5, 7, 11]).unwrap();
    let right = Digest::new([13, 17, 19, 23, 29, 31]).unwrap();
    for (oracle, parent) in [
        (Oracle::Row, false),
        (Oracle::QuotientAndMask, false),
        (Oracle::Fri(0), false),
        (Oracle::Row, true),
    ] {
        let (_, _, _, width) = oracle.shape().unwrap();
        for &count in counts {
            assert!(count > 0 && count <= 8192 && samples % count == 0);
            let payloads = (0..count * width / 8)
                .flat_map(|value| (value as u64 + 7).to_le_bytes())
                .collect::<Vec<_>>();
            let mut preparation = 0.0;
            let mut dispatch = 0.0;
            let mut owner_cleanup = 0.0;
            let mut charged = 0;
            // One warm iteration, then the same sample count at each batch size.
            for iteration in 0..=samples / count {
                let started = Instant::now();
                let frames = (0..count)
                    .into_par_iter()
                    .map(|index| {
                        if parent {
                            binding.prepare_parent(oracle, 1, index as u32, left, right)
                        } else {
                            binding.prepare_leaf(
                                oracle,
                                index as u32,
                                &payloads[index * width..(index + 1) * width],
                            )
                        }
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .unwrap();
                let bytes = frames.iter().map(|frame| frame.payload_len()).sum();
                // The larger diagnostic does not enter or change the production
                // batch helper. Its exact actual frame bytes must independently
                // fit the same typed executor limit before dispatch.
                let jobs = if count <= super::super::deep_leaf_batch::CAPACITY {
                    super::super::deep_leaf_batch::prepare_jobs(&frames).unwrap()
                } else {
                    frames
                        .par_iter()
                        .map(|frame| frame.job())
                        .collect::<Vec<_>>()
                        .into_iter()
                        .collect::<Result<Vec<_>>>()
                        .unwrap()
                };
                let preparation_seconds = started.elapsed().as_secs_f64();
                charged = crate::digest384_batch::last_fields_payload_charge(count, bytes).unwrap();
                let started = Instant::now();
                let output = zeroize::Zeroizing::new(
                    execute_last_fields_with_cpu(
                        count,
                        bytes,
                        execution,
                        |_| panic!("required-Metal diagnostic cannot use CPU substitution"),
                        || Ok(jobs),
                    )
                    .unwrap(),
                );
                let dispatch_seconds = started.elapsed().as_secs_f64();
                assert_eq!(output.len(), count);
                for index in [0, count - 1] {
                    assert_eq!(output[index], frames[index].hash_cpu().unwrap());
                }
                let cleanup_started = Instant::now();
                drop(output);
                drop(frames);
                let cleanup_seconds = cleanup_started.elapsed().as_secs_f64();
                if iteration != 0 {
                    preparation += preparation_seconds;
                    dispatch += dispatch_seconds;
                    owner_cleanup += cleanup_seconds;
                }
            }
            eprintln!(
                "oracle={oracle:?}; parent={parent}; batch={count}; samples={samples}; frame_and_job_preparation_seconds={preparation:.6}; executor_seconds={dispatch:.6}; returned_owner_cleanup_seconds={owner_cleanup:.6}; executor_payload_charge={charged}; executor includes host packing, GPU execution, readback and internal clearing, not isolated kernel time"
            );
        }
    }
}

/// Default policies admit one exact plan whose every budget is also a lower bound.
fn check_default_policy_plan(relation: &impl DeepRelation) {
    use crate::backend::{
        compact_protocol::FixedAir,
        offline_compact::{ProvingLimits, VerificationLimits},
    };
    let proving = ProvingLimits::default();
    let verification = VerificationLimits::default();
    let limits = ConstructionLimits {
        digest_execution: proving.digest_execution,
        max_payload_bytes: proving.max_segment_charge_bytes,
        max_work_units: proving.max_segment_work_units,
        max_hash_calls: proving.max_segment_work_units,
        max_proof_bytes: verification.bundle.segment.max_proof_bytes,
    };
    let plan = ProducerPlan::new(relation, limits).unwrap();
    assert!(plan.payload_bytes <= proving.max_segment_charge_bytes);
    // Preserve the existing resource boundary. Admission must cover the
    // actual replay and additional quotient/commitment work within it.
    assert_eq!(
        proving.max_segment_work_units,
        usize::try_from(1_u64 << 42).unwrap()
    );
    assert!(plan.work_units > plan.replay.work_units);
    assert!(plan.work_units <= proving.max_segment_work_units);
    assert!(plan.hash_calls <= proving.max_segment_work_units);
    assert_eq!(
        plan.binding
            .hash_parent(Oracle::Row, 1, 0, Digest::default(), Digest::default())
            .unwrap(),
        Context::for_relation(relation)
            .unwrap()
            .hash_parent(Oracle::Row, 1, 0, Digest::default(), Digest::default())
            .unwrap()
    );
    assert_ne!(
        relation.schema().identity,
        relation.deep_relation().schema().identity
    );
    assert_ne!(
        plan.binding
            .hash_parent(Oracle::Row, 1, 0, Digest::default(), Digest::default())
            .unwrap(),
        Context::for_relation(relation.deep_relation())
            .unwrap()
            .hash_parent(Oracle::Row, 1, 0, Digest::default(), Digest::default())
            .unwrap()
    );
    for limited in [
        ConstructionLimits {
            max_payload_bytes: plan.payload_bytes - 1,
            ..limits
        },
        ConstructionLimits {
            max_work_units: plan.work_units - 1,
            ..limits
        },
        ConstructionLimits {
            max_hash_calls: plan.hash_calls - 1,
            ..limits
        },
    ] {
        assert!(ProducerPlan::new(relation, limited).is_err());
    }
    let mut rng = NoEntropy(0);
    assert!(plan.build_from_borrowed_for_test(&[], &mut rng).is_err());
    assert_eq!(rng.0, 0);
}

#[test]
fn default_policies_preflight_quantity_relations_without_private_columns_or_entropy() {
    use crate::{
        ProofSemantics,
        backend::{
            compact_axt_batch::AxtTransferBatch,
            compact_axt_context::tests::Fixture,
            compact_public_api::AxtVerificationContext,
            compact_public_batch::{BatchContextLimits, PublicTransferBatch},
            deep_relation::tests as relation_fixture,
        },
        gadgets::public_transfer_statement::{
            PublicTransferLimits, prepare_quantity_public_transfers,
        },
    };

    // This fixture constructs only public transfer facts. It never materializes
    // touched-tree paths, a physical witness, coefficient matrices or an LDE.
    let fixture = Fixture::multiple(2, true);
    for semantics in [
        ProofSemantics::StateTransition,
        ProofSemantics::AxtTransferClaim,
    ] {
        let narrow = fixture.prepare(semantics);
        let (rows, claims, inputs) = relation_fixture::quantity_copy(&narrow);
        let prepared = prepare_quantity_public_transfers(
            &rows,
            &claims,
            inputs,
            semantics,
            PublicTransferLimits::default(),
        )
        .unwrap();
        let expected = relation_fixture::expected(&prepared);
        let roots = [inputs.old_root];
        if semantics == ProofSemantics::StateTransition {
            let batch = PublicTransferBatch::new(
                &prepared,
                &expected,
                &roots,
                BatchContextLimits::default(),
            )
            .unwrap();
            for index in 0..batch.segment_count() {
                check_default_policy_plan(&batch.segment(index).unwrap());
            }
        } else {
            let batch = AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots,
                AxtVerificationContext {
                    binding: &fixture.binding,
                    metadata: fixture.metadata(),
                    mirrors: fixture.outer,
                    remote_spend_claims: fixture.remote.as_deref(),
                },
                BatchContextLimits::default(),
            )
            .unwrap();
            for index in 0..batch.segment_count() {
                check_default_policy_plan(&batch.segment(index).unwrap());
            }
        }
    }
}
