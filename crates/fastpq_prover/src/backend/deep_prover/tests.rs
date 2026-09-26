//! Producer preflight, actual typed trees and an explicit full proof diagnostic.

use super::*;
use crate::backend::{
    GOLDILOCKS_MODULUS,
    compact_transfer_air::CompactTransferAir,
    deep_engine,
    deep_geometry::QUERY_COUNT,
    deep_quotient::tests::{actual_smt_fixture, actual_smt_relation},
    merkle_multiproof::{MultiproofLimits, MultiproofWork},
};

fn limits() -> ProverLimits {
    ProverLimits {
        digest_execution: DigestExecutionV1::Cpu,
        max_payload_bytes: usize::MAX,
        quotient: DeepQuotientLimits {
            max_payload_bytes: usize::MAX,
            max_work_units: usize::MAX,
        },
        max_proof_bytes: deep_proof::PROOF_BYTE_TARGET,
    }
}

fn dense(seed: u64) -> F {
    F::new([seed + 1, 2 * seed + 3, 3 * seed + 5, 5 * seed + 7]).unwrap()
}

fn multiproof(leaves: usize, indices: &[usize]) -> MultiproofPlan {
    MultiproofPlan::new(
        leaves,
        indices,
        MultiproofLimits {
            max_depth: 23,
            max_queried_leaves: 128,
            max_siblings: 128 * 23,
            max_parent_hashes: 128 * 23,
        },
    )
    .unwrap()
}

fn verify_frontier(
    context: &Context,
    oracle: Oracle,
    plan: &MultiproofPlan,
    root: Digest,
    leaves: &[Digest],
    frontier: &[WireDigest],
) -> Result<MultiproofWork> {
    let siblings = frontier
        .iter()
        .map(|value| value.as_fastpq())
        .collect::<Vec<_>>();
    plan.verify_parallel_with(root, leaves, &siblings, |level, index, left, right| {
        context
            .hash_parent(oracle, level as u32, index as u32, left, right)
            .map_err(binding_error)
    })
}

fn changed_digest(digest: Digest) -> Digest {
    let mut words = digest.words();
    words[0] = if words[0] == GOLDILOCKS_MODULUS - 1 {
        0
    } else {
        words[0] + 1
    };
    Digest::new(words).unwrap()
}

#[test]
fn preflight_checks_complete_source_shape_and_canonicality_before_large_allocations() {
    let relation = actual_smt_relation(Some(b"preflight"));
    let empty = [&[][..]; COMMITTED_COLUMN_COUNT];
    for width in [0, 300, 302, 342] {
        assert!(matches!(
            preflight(&relation, &vec![&[][..]; width], limits()),
            Err(Error::InvalidTraceShape { .. })
        ));
    }
    let oversized = vec![0; TRACE_ROWS + 1];
    let mut columns = empty;
    columns[COMMITTED_COLUMN_COUNT - 1] = &oversized;
    assert!(matches!(
        preflight(&relation, &columns, limits()),
        Err(Error::InvalidTraceShape { .. })
    ));
    for column in 0..COMMITTED_COLUMN_COUNT {
        let invalid = [0, GOLDILOCKS_MODULUS];
        let mut columns = empty;
        columns[column] = &invalid;
        assert!(matches!(preflight(&relation, &columns, limits()),
            Err(Error::NonCanonicalGoldilocksElement { context: "deep_prover_coefficients", indices }) if indices == [column, 1]));
    }
    let mut boundary = vec![0; TRACE_ROWS];
    boundary[TRACE_ROWS - 1] = GOLDILOCKS_MODULUS;
    columns[COMMITTED_COLUMN_COUNT - 1] = &boundary;
    assert!(matches!(preflight(&relation, &columns, limits()),
        Err(Error::NonCanonicalGoldilocksElement { context: "deep_prover_coefficients", indices }) if indices == [COMMITTED_COLUMN_COUNT - 1, TRACE_ROWS - 1]));
    boundary[TRACE_ROWS - 1] = GOLDILOCKS_MODULUS - 1;
    let mut accepted = empty;
    accepted[COMMITTED_COLUMN_COUNT - 1] = &boundary;
    preflight(&relation, &accepted, limits()).unwrap();
    preflight(&relation, &empty, limits()).unwrap();
    let geometry = DeepGeometry::new().unwrap();
    assert!(base_lde(&geometry, &oversized).is_err());
    assert!(matches!(base_lde(&geometry, &[GOLDILOCKS_MODULUS]),
        Err(Error::NonCanonicalGoldilocksElement { context: "deep_prover_base_lde", indices }) if indices == [0]));
}

#[test]
fn proof_and_payload_policies_reject_before_domain_sized_buffers() {
    let relation = actual_smt_relation(Some(b"policy preflight"));
    let columns = [&[][..]; COMMITTED_COLUMN_COUNT];
    let charge = payload_charge(0).unwrap();
    assert!(charge > COMMITTED_COLUMN_COUNT * LDE_ROWS * 8);
    assert_eq!(payload_charge(123_457).unwrap(), charge + 123_457);
    assert!(payload_charge(usize::MAX).is_err());
    assert!(matches!(
        prove(
            &relation,
            &columns,
            ProverLimits {
                max_proof_bytes: deep_proof::MAX_FRAME_BYTES - 1,
                ..limits()
            }
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_deep_proof_bytes",
            actual: deep_proof::MAX_FRAME_BYTES,
            ..
        })
    ));
    assert!(matches!(prove(&relation, &columns, ProverLimits {
        max_payload_bytes: charge - 1, ..limits()
    }), Err(Error::VerifierLimitExceeded { limit: "max_deep_prover_payload_bytes", actual, .. }) if actual == charge));
    preflight(
        &relation,
        &columns,
        ProverLimits {
            max_payload_bytes: charge,
            max_proof_bytes: deep_proof::MAX_FRAME_BYTES,
            ..limits()
        },
    )
    .unwrap();
    // The only calls to prove in ordinary tests fail before allocating a public
    // coefficient matrix, base LDE, FRI layer or full commitment tree.
    assert!(matches!(
        prove(
            &relation,
            &columns,
            ProverLimits {
                quotient: DeepQuotientLimits {
                    max_payload_bytes: 0,
                    max_work_units: 0
                },
                ..limits()
            }
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_deep_quotient_preparation_bytes",
            ..
        })
    ));
}

#[test]
fn actual_final_fri_tree_has_exact_frontier_and_rejects_tampering() {
    let binding = Context::new(b"small actual final FRI tree").unwrap();
    let oracle = Oracle::Fri(4);
    let mut tree = Tree::build(&binding, oracle, 128, DigestExecutionV1::Cpu, |index| {
        let bytes: Vec<_> = (0..4)
            .flat_map(|coordinate| dense((4 * index + coordinate) as u64).to_le_bytes())
            .collect();
        binding.prepare_leaf(oracle, index as u32, &bytes)
    })
    .unwrap();
    assert_eq!(
        tree.levels.iter().map(Vec::len).collect::<Vec<_>>(),
        [128, 64, 32, 16, 8, 4, 2, 1]
    );
    let indices = [0, 1, 31, 64, 127];
    let plan = multiproof(128, &indices);
    let leaves = indices.map(|index| tree.levels[0][index]);
    let frontier = tree.frontier(&plan).unwrap();
    assert_eq!(frontier.len(), plan.work().siblings);
    for (value, position) in frontier.iter().zip(plan.sibling_positions()) {
        assert_eq!(
            value.as_fastpq(),
            tree.levels[position.level][position.index]
        );
    }
    assert_eq!(
        verify_frontier(&binding, oracle, &plan, tree.root(), &leaves, &frontier).unwrap(),
        plan.work()
    );
    let mut changed_leaves = leaves;
    changed_leaves[2] = changed_digest(changed_leaves[2]);
    assert!(
        verify_frontier(
            &binding,
            oracle,
            &plan,
            tree.root(),
            &changed_leaves,
            &frontier
        )
        .is_err()
    );
    let mut changed_frontier = frontier.clone();
    changed_frontier[0] = WireDigest::from(changed_digest(changed_frontier[0].as_fastpq()));
    assert!(
        verify_frontier(
            &binding,
            oracle,
            &plan,
            tree.root(),
            &leaves,
            &changed_frontier
        )
        .is_err()
    );
    assert!(
        verify_frontier(
            &binding,
            oracle,
            &plan,
            tree.root(),
            &leaves,
            &frontier[..frontier.len() - 1]
        )
        .is_err()
    );
    let changed_context = Context::new(b"different final FRI tree context").unwrap();
    assert!(
        verify_frontier(
            &changed_context,
            oracle,
            &plan,
            tree.root(),
            &leaves,
            &frontier
        )
        .is_err()
    );
    assert!(
        verify_frontier(
            &binding,
            Oracle::Fri(3),
            &plan,
            tree.root(),
            &leaves,
            &frontier
        )
        .is_err()
    );
    assert!(tree.frontier(&multiproof(256, &[0])).is_err());
    tree.levels[0][0] = changed_digest(tree.levels[0][0]);
    assert!(tree.frontier(&plan).is_err());
    // Invalid oracle geometry fails before the leaf callback can run.
    for (oracle, leaves) in [
        (Oracle::Fri(4), 64),
        (Oracle::Fri(5), 128),
        (Oracle::Terminal, 2),
    ] {
        assert!(
            Tree::build(
                &binding,
                oracle,
                leaves,
                DigestExecutionV1::Cpu,
                |_| panic!("invalid geometry reached leaf allocation")
            )
            .is_err()
        );
    }
}

#[test]
fn complete_terminal_is_one_leaf_with_its_required_duplicate_parent() {
    let binding = Context::new(b"complete terminal").unwrap();
    let terminal: Vec<_> = [dense(19); 128]
        .into_iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let leaf = binding.hash_leaf(Oracle::Terminal, 0, &terminal).unwrap();
    let mut tree = Tree::build(
        &binding,
        Oracle::Terminal,
        1,
        DigestExecutionV1::Cpu,
        |_| binding.prepare_leaf(Oracle::Terminal, 0, &terminal),
    )
    .unwrap();
    assert_eq!(tree.levels.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(tree.levels[0], [leaf, leaf]);
    let expected = binding
        .hash_parent(Oracle::Terminal, 1, 0, leaf, leaf)
        .unwrap();
    assert_eq!(tree.root(), expected);
    assert_ne!(tree.root(), leaf);
    let plan = multiproof(1, &[0]);
    let frontier = tree.frontier(&plan).unwrap();
    assert!(frontier.is_empty());
    let work = verify_frontier(
        &binding,
        Oracle::Terminal,
        &plan,
        tree.root(),
        &[leaf],
        &frontier,
    )
    .unwrap();
    assert_eq!(work.parent_hashes, 1);
    assert_eq!(work.queried_leaves, 1);
    assert!(verify_frontier(&binding, Oracle::Terminal, &plan, leaf, &[leaf], &frontier).is_err());
    assert!(
        verify_frontier(
            &binding,
            Oracle::Terminal,
            &plan,
            tree.root(),
            &[changed_digest(leaf)],
            &frontier
        )
        .is_err()
    );
    tree.levels[0][1] = changed_digest(leaf);
    assert!(tree.frontier(&plan).is_err());
}

// Only the explicitly ignored qualification probes use this test-only setting.
// Artifact paths remain under ignored validation storage. Existing proof bytes
// can be reverified without reconstructing a witness or running the producer.
fn proof_artifact_path() -> Option<std::path::PathBuf> {
    use std::path::{Component, PathBuf};
    let requested = std::env::var_os("FASTPQ_DEEP_PROOF_ARTIFACT")?;
    let base =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&base).unwrap();
    let base = base.canonicalize().unwrap();
    let requested = PathBuf::from(requested);
    assert!(
        !requested
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    );
    let path = if requested.is_absolute() {
        requested
    } else {
        base.join(requested)
    };
    assert!(
        path.starts_with(&base),
        "proof artifact must remain in ignored validation storage"
    );
    Some(path)
}

fn proof_artifact_base() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation")
        .canonicalize()
        .unwrap()
}

// Preserve bytes before verification, so a failed full probe remains reproducible.
fn preserve_proof_if_requested(bytes: &[u8]) {
    use std::io::Write;
    let Some(path) = proof_artifact_path() else {
        return;
    };
    let parent = path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();
    assert!(
        parent
            .canonicalize()
            .unwrap()
            .starts_with(proof_artifact_base())
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    eprintln!(
        "deep_proof_artifact={} bytes={}",
        path.display(),
        bytes.len()
    );
}

fn read_captured_proof() -> Vec<u8> {
    use std::io::Read;
    let path = proof_artifact_path()
        .expect("set FASTPQ_DEEP_PROOF_ARTIFACT to an existing captured proof");
    assert!(
        path.canonicalize()
            .unwrap()
            .starts_with(proof_artifact_base())
    );
    let mut file = std::fs::File::open(&path).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    let length = usize::try_from(metadata.len()).unwrap();
    assert!(
        length <= deep_proof::MAX_FRAME_BYTES,
        "captured proof exceeds frame ceiling"
    );
    // Allocate only after inspecting the opened file's bounded length. Reading
    // exactly that many bytes and checking EOF also rejects a concurrent append.
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(
        file.read(&mut [0]).unwrap(),
        0,
        "captured proof changed while reading"
    );
    eprintln!(
        "deep_proof_artifact_read={} bytes={}",
        path.display(),
        bytes.len()
    );
    bytes
}

#[test]
#[ignore = "actual 8M-domain DEEP proof: tens of GiB and costly hashes/FFTs; explicit local qualification only"]
fn actual_smt_proof_roundtrips_through_bounded_verifier_and_rejects_context_and_tampering() {
    let (relation, coefficients) = actual_smt_fixture();
    let borrowed = coefficients.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let bytes = prove(
        &relation,
        &borrowed,
        ProverLimits {
            digest_execution: DigestExecutionV1::Cpu,
            // Conservative array payload ceilings, not limits on process RSS.
            // These allow the ~30 GiB retained producer plus its 4N quotient.
            max_payload_bytes: usize::try_from(64_u64 << 30).unwrap(),
            quotient: DeepQuotientLimits {
                max_payload_bytes: usize::try_from(8_u64 << 30).unwrap(),
                // Finite structural field/inspection work, checked by the exact
                // 4N quotient plan before any 8M LDE or commitment allocation.
                // Even the checked ledger maxima bound this below 167e9 units.
                max_work_units: usize::try_from(1_u64 << 42).unwrap(),
            },
            max_proof_bytes: deep_proof::PROOF_BYTE_TARGET,
        },
    )
    .unwrap();
    drop(borrowed);
    drop(coefficients);
    preserve_proof_if_requested(&bytes);
    assert_valid_and_tamper(&relation, &bytes);
}

#[test]
#[ignore = "explicit retained-artifact qualification; requires FASTPQ_DEEP_PROOF_ARTIFACT"]
fn verify_captured_actual_smt_proof() {
    let bytes = read_captured_proof();
    let relation = actual_smt_relation(Some(b"actual unmasked quotient control"));
    assert_valid_and_tamper(&relation, &bytes);
}

fn assert_valid_and_tamper(relation: &CompactTransferAir, bytes: &[u8]) {
    assert!(bytes.len() <= deep_proof::MAX_FRAME_BYTES);
    assert!(bytes.len() <= deep_proof::PROOF_BYTE_TARGET);
    let proof = deep_proof::decode(bytes, deep_proof::PROOF_BYTE_TARGET).unwrap();
    assert_eq!(norito::encode_canonical(&proof).unwrap().as_slice(), bytes);
    let policy = crate::VerifyLimits::default();
    let committed =
        deep_engine::verify_committed(relation, bytes, policy, deep_proof::MAX_ALLOCATION_CHARGES)
            .unwrap();
    assert_eq!(committed.row_root(), proof.row_root);
    let work = committed.work();
    assert_eq!(work.proof_bytes, bytes.len());
    assert_eq!(work.air_evaluations, 1);
    assert_eq!(work.verifier_messages, 10);
    assert_eq!(work.g_blocks, 637);
    assert_eq!(work.fold_checks, QUERY_COUNT * FRI_ARITIES.len());
    assert_eq!(work.terminal_values, 128);
    assert!(work.h_calls <= 5125);
    eprintln!("deep_full_proof_verified={work:?}");
    let changed_context =
        actual_smt_relation(Some(b"changed caller context with identical SMT statement"));
    assert!(
        deep_engine::verify_committed(
            &changed_context,
            bytes,
            policy,
            deep_proof::MAX_ALLOCATION_CHARGES,
        )
        .is_err()
    );
    assert!(
        deep_engine::verify_committed(
            relation,
            bytes,
            crate::VerifyLimits {
                max_proof_bytes: bytes.len() - 1,
                ..policy
            },
            deep_proof::MAX_ALLOCATION_CHARGES,
        )
        .is_err()
    );
    let mut changed = proof.clone();
    changed.ood.quotient[0] = changed.ood.quotient[0].add(F::ONE);
    assert!(
        deep_engine::verify_committed(
            relation,
            &norito::encode_canonical(&changed).unwrap(),
            policy,
            deep_proof::MAX_ALLOCATION_CHARGES,
        )
        .is_err()
    );
    changed = proof.clone();
    let mut values = changed.rows[0].values.to_vec();
    values[0] = if values[0] == GOLDILOCKS_MODULUS - 1 {
        0
    } else {
        values[0] + 1
    };
    changed.rows[0].values = RowValues::new(values).unwrap();
    assert!(
        deep_engine::verify_committed(
            relation,
            &norito::encode_canonical(&changed).unwrap(),
            policy,
            deep_proof::MAX_ALLOCATION_CHARGES,
        )
        .is_err()
    );
    changed = proof;
    changed.terminal[127] = changed.terminal[127].add(F::ONE);
    assert!(
        deep_engine::verify_committed(
            relation,
            &norito::encode_canonical(&changed).unwrap(),
            policy,
            deep_proof::MAX_ALLOCATION_CHARGES,
        )
        .is_err()
    );
}

#[test]
fn prepared_leaves_and_parents_match_independent_canonical_hashes() {
    let binding = Context::new(&vec![73; 200 * 1024]).unwrap();
    for oracle in [
        Oracle::Row,
        Oracle::QuotientPair,
        Oracle::Fri(0),
        Oracle::Fri(4),
        Oracle::Terminal,
    ] {
        let (_, _, leaves, width) = oracle.shape().unwrap();
        for index in [0, leaves - 1] {
            let payload: Vec<_> = (0..width / 8)
                .flat_map(|i| (i as u64 + 1).to_le_bytes())
                .collect();
            let prepared = binding
                .prepare_leaf(oracle, index as u32, &payload)
                .unwrap();
            assert_eq!(prepared.job().unwrap().prefix().received_len(), 0);
            assert!(prepared.job().unwrap().final_field().len() <= MAX_PREPARED_HASH_FRAME_BYTES);
            assert_eq!(
                execute_prepared_frames(&[prepared], DigestExecutionV1::Cpu).unwrap(),
                [binding.hash_leaf(oracle, index as u32, &payload).unwrap()]
            );
            assert!(
                binding
                    .prepare_leaf(oracle, leaves as u32, &payload)
                    .is_err()
            );
            assert!(
                binding
                    .prepare_leaf(oracle, index as u32, &payload[..width - 1])
                    .is_err()
            );
            let mut malformed = payload;
            malformed[..8].copy_from_slice(&GOLDILOCKS_MODULUS.to_le_bytes());
            assert!(
                binding
                    .prepare_leaf(oracle, index as u32, &malformed)
                    .is_err()
            );
        }
    }
    let left = Digest::new([1, 2, 3, 4, 5, 6]).unwrap();
    let right = Digest::new([6, 5, 4, 3, 2, 1]).unwrap();
    for (oracle, level, index) in [
        (Oracle::Row, 1, 257),
        (Oracle::Row, 23, 0),
        (Oracle::Fri(0), 2, 513),
    ] {
        let frame = binding
            .prepare_parent(oracle, level, index, left, right)
            .unwrap();
        assert_eq!(
            execute_prepared_frames(&[frame], DigestExecutionV1::Cpu).unwrap(),
            [binding
                .hash_parent(oracle, level, index, left, right)
                .unwrap()]
        );
        assert!(
            binding
                .prepare_parent(oracle, 0, index, left, right)
                .is_err()
        );
    }
    assert!(
        binding
            .prepare_parent(Oracle::Terminal, 1, 0, left, right)
            .is_err()
    );
    assert!(
        binding
            .prepare_parent(Oracle::Row, 24, 0, left, right)
            .is_err()
    );
}

#[test]
fn bounded_tree_preserves_all_levels_across_leaf_preparation_boundaries() {
    let binding = Context::new(b"bounded tree preparation boundary").unwrap();
    let oracle = Oracle::Fri(3);
    let (_, _, count, width) = oracle.shape().unwrap();
    assert_eq!(count, 2 * HASH_BATCH_FRAMES);
    let payload = |index: usize| {
        (0..width / 8)
            .flat_map(|column| ((index * width + column) as u64).to_le_bytes())
            .collect::<Vec<_>>()
    };
    let tree = Tree::build(&binding, oracle, count, DigestExecutionV1::Cpu, |index| {
        binding.prepare_leaf(oracle, index as u32, &payload(index))
    })
    .unwrap();
    let mut expected = (0..count)
        .map(|index| {
            binding
                .hash_leaf(oracle, index as u32, &payload(index))
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(tree.levels[0], expected);
    for (level, actual) in tree.levels.iter().enumerate().skip(1) {
        expected = expected
            .chunks_exact(2)
            .enumerate()
            .map(|(index, pair)| {
                binding
                    .hash_parent(oracle, level as u32, index as u32, pair[0], pair[1])
                    .unwrap()
            })
            .collect();
        assert_eq!(actual, &expected);
    }
    assert_eq!(tree.root(), expected[0]);
}

#[test]
fn fixed_hash_preparation_charge_includes_guarded_bodies_jobs_rows_and_backend_pages() {
    let bodies = HASH_BATCH_FRAMES * MAX_PREPARED_HASH_FRAME_BYTES;
    let descriptors_and_rows = HASH_BATCH_FRAMES
        * (core::mem::size_of::<Result<PreparedHashFrame>>()
            + core::mem::size_of::<PreparedHashFrame>()
            + core::mem::size_of::<Digest384LastFieldJob<'_>>()
            + 128 * F::BYTES);
    assert_eq!(
        hash_batch_payload_charge().unwrap(),
        bodies
            + descriptors_and_rows
            + last_fields_payload_charge(HASH_BATCH_FRAMES, bodies).unwrap()
    );
    assert!(payload_charge(0).unwrap() > hash_batch_payload_charge().unwrap());
    let binding = Context::new(b"oversized prepared batch").unwrap();
    let payload = vec![0; COMMITTED_COLUMN_COUNT * 8];
    let frames = (0..=HASH_BATCH_FRAMES)
        .map(|index| {
            binding
                .prepare_leaf(Oracle::Row, index as u32, &payload)
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(execute_prepared_frames(&frames, DigestExecutionV1::Cpu).is_err());
}

#[cfg(all(feature = "fastpq-gpu", target_os = "macos"))]
#[test]
#[ignore = "requires actual Metal DEEP body/parent execution; no CPU substitution"]
fn bounded_deep_tree_metal_matches_cpu_at_every_level_and_opening() {
    let _lane = crate::backend::acquire_gpu_lane();
    let binding = Context::new(b"actual Metal canonical DEEP tree").unwrap();
    let oracle = Oracle::Fri(3);
    let (_, _, count, width) = oracle.shape().unwrap();
    let make_leaf = |index: usize| {
        let payload = (0..width / 8)
            .flat_map(|column| ((index * width + column) as u64).to_le_bytes())
            .collect::<Vec<_>>();
        binding.prepare_leaf(oracle, index as u32, &payload)
    };
    let cpu = Tree::build(&binding, oracle, count, DigestExecutionV1::Cpu, &make_leaf).unwrap();
    let metal = Tree::build(
        &binding,
        oracle,
        count,
        DigestExecutionV1::Device(crate::Digest384GpuBackendV1::Metal),
        &make_leaf,
    )
    .unwrap();
    assert_eq!(metal.levels, cpu.levels);
    let plan = multiproof(count, &[0, 255, 256, count - 1]);
    assert_eq!(metal.frontier(&plan).unwrap(), cpu.frontier(&plan).unwrap());
}
