//! Producer admission boundaries and explicit complete public-API proof coverage.

use std::cell::Cell;

use super::*;
use crate::{
    VerifyLimits,
    backend::{
        compact_prover_resources::segment_charge,
        compact_quantity_tests::{QuantityCase, QuantityFixture},
    },
    gadgets::public_transfer_statement::{PublicTransferLimits, TransferSmtBuildLimits},
    offline_compact::{
        BundleVerificationLimits, prove_quantity_axt_artifact, prove_quantity_ordinary_artifact,
        verify_quantity_axt_artifact, verify_quantity_ordinary_artifact,
    },
};
use iroha_data_model::fastpq::{
    FastpqArtifactIdentityDescriptionV1, FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1,
    FastpqCommitmentDescriptionV1, FastpqCompactArtifactDecodeLimits, FastpqProofKindV1,
};
use norito::core::DecodeLimits;
use sha2::{Digest as _, Sha256};

fn policy() -> VerificationLimits {
    VerificationLimits {
        transport: FastpqCompactArtifactDecodeLimits {
            max_wire_bytes: 1024 * 1024,
            max_bundle_frame_bytes: 1024 * 1024,
            norito: DecodeLimits::new(
                20 * 1024 * 1024,
                20 * 1024 * 1024,
                25 * 1024 * 1024,
                96 * 1024 * 1024,
                32,
            ),
        },
        public_statement: PublicTransferLimits::default(),
        bundle: BundleVerificationLimits {
            max_segments: 2,
            max_wire_bytes: 1024 * 1024,
            max_total_segment_bytes: 1024 * 1024,
            max_total_statement_bytes: 512 * 1024,
            max_total_queries: 2 * QUERY_COUNT,
            max_total_decode_allocation_charges: 128 * 1024 * 1024,
            segment: VerifyLimits {
                max_proof_bytes: 512 * 1024,
                max_queries: QUERY_COUNT,
                ..VerifyLimits::default()
            },
        },
        max_segment_decode_allocation_charges: 64 * 1024 * 1024,
        total_decode: DecodeLimits::new(
            20 * 1024 * 1024,
            20 * 1024 * 1024,
            30 * 1024 * 1024,
            192 * 1024 * 1024,
            32,
        ),
    }
}

fn proving() -> ProvingLimits {
    ProvingLimits {
        digest_execution: crate::DigestExecutionV1::Cpu,
        private_smt: TransferSmtBuildLimits::for_update_limit(4).unwrap(),
        max_total_trace_cells: 2 * COLUMN_COUNT * PHYSICAL_ROW_COUNT,
        max_segment_work_units: usize::try_from(1_u64 << 42).unwrap(),
        max_segment_charge_bytes: segment_charge(
            policy().bundle.max_total_statement_bytes,
            SHARED_FRAME_BOUND,
        )
        .unwrap(),
    }
}

fn fixture() -> QuantityFixture {
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 2);
    drop(private);
    fixture
}

fn expected(statement: &FastpqPublicTransferStatementV1) -> ExpectedStatement {
    ExpectedStatement {
        inputs: statement.public_inputs,
        ordering_hash: statement.ordering_hash,
        public_statement_digest: Hash::new(norito::encode_canonical(statement).unwrap()).into(),
    }
}

fn axt_fields(
    fixture: &QuantityFixture,
) -> (FastpqAxtPublicMetadataV1, FastpqAxtPreProofMirrorsV1) {
    let context = fixture.context();
    let metadata = context.metadata;
    let mirrors = context.mirrors;
    (
        FastpqAxtPublicMetadataV1 {
            parameter: metadata.parameter.to_owned(),
            entry_hash: metadata.entry_hash.try_into().unwrap(),
            committed_amount: metadata.committed_amount.map(|b| b.try_into().unwrap()),
            expiry_slot: metadata.expiry_slot.try_into().unwrap(),
            manifest_root: metadata.manifest_root.try_into().unwrap(),
            da_commitment: metadata.da_commitment.try_into().unwrap(),
        },
        FastpqAxtPreProofMirrorsV1 {
            dsid: mirrors.dsid,
            manifest_root: mirrors.manifest_root,
            da_commitment: mirrors.da_commitment,
            committed_amount: mirrors.committed_amount,
            expiry_slot: mirrors.expiry_slot,
        },
    )
}

fn assert_limit(result: Result<impl std::fmt::Debug>, name: &str, actual: usize, max: usize) {
    assert!(
        matches!(result, Err(Error::VerifierLimitExceeded { limit, actual: got, max: cap })
        if limit == name && got == actual && cap == max)
    );
}

#[test]
fn exclusive_admission_reports_busy_releases_and_recovers_poison_locally() {
    // This mutex is private to the test, so neither the global producer nor a
    // parallel full-proof diagnostic can interfere with these assertions.
    let mutex = Mutex::new(());
    let first = acquire(&mutex).unwrap();
    assert!(matches!(acquire(&mutex), Err(ProvingError::Busy)));
    drop(first);
    drop(acquire(&mutex).unwrap());
    std::thread::scope(|scope| {
        let result = scope
            .spawn(|| {
                let _held = mutex.lock().unwrap();
                panic!("deliberately poison only this test's admission mutex");
            })
            .join();
        assert!(result.is_err());
    });
    assert!(mutex.is_poisoned());
    let recovered = acquire(&mutex).unwrap();
    assert!(matches!(acquire(&mutex), Err(ProvingError::Busy)));
    drop(recovered);
    drop(acquire(&mutex).unwrap());
}

#[test]
fn byte_and_work_arithmetic_rejects_overflow_without_saturating() {
    assert_eq!(add(usize::MAX, 0).unwrap(), usize::MAX);
    assert_eq!(mul(usize::MAX, 1).unwrap(), usize::MAX);
    assert_eq!(mul(usize::MAX, 0).unwrap(), 0);
    assert!(
        matches!(add(usize::MAX, 1), Err(Error::TransferInvariant { details })
        if details == "producer byte count overflows")
    );
    assert!(
        matches!(mul(usize::MAX, 2), Err(Error::TransferInvariant { details })
        if details == "producer work count overflows")
    );
}

#[test]
fn complete_statement_caps_are_inclusive_and_fail_before_public_preparation() {
    let statement = fixture().model();
    let expected = expected(&statement);
    let mut exact = policy();
    exact.public_statement.max_rows = statement.transitions.len();
    exact.public_statement.max_transcripts = statement.transcripts.len();
    exact.public_statement.max_deltas = 2;
    exact.public_statement.max_public_bytes = norito::core::encoded_frame_len(&statement).unwrap();
    exact.bundle.max_segments = 2;
    exact.bundle.max_total_queries = 2 * QUERY_COUNT;
    exact.bundle.segment.max_queries = QUERY_COUNT;
    exact.bundle.segment.max_proof_bytes = SHARED_FRAME_BOUND;
    exact.bundle.max_total_segment_bytes = 2 * SHARED_FRAME_BOUND;
    let mut work = proving();
    work.max_segment_charge_bytes = segment_charge(0, SHARED_FRAME_BOUND).unwrap();
    assert_eq!(
        check_statement(&statement, expected, work, exact).unwrap(),
        2
    );

    for (name, actual) in [
        ("max_public_transfer_rows", statement.transitions.len()),
        (
            "max_public_transfer_transcripts",
            statement.transcripts.len(),
        ),
        ("max_public_transfer_deltas", 2),
        ("max_bundle_segments", 2),
        ("max_queries", QUERY_COUNT),
        ("max_bundle_queries", 2 * QUERY_COUNT),
        ("max_proof_bytes", SHARED_FRAME_BOUND),
        ("max_bundle_segment_bytes", 2 * SHARED_FRAME_BOUND),
        ("max_compact_prover_trace_cells", work.max_total_trace_cells),
        (
            "max_compact_prover_segment_charge_bytes",
            work.max_segment_charge_bytes,
        ),
        (
            "max_compact_producer_statement_bytes",
            exact.public_statement.max_public_bytes,
        ),
    ] {
        let mut limited = exact;
        let mut limited_work = work;
        let cap = actual - 1;
        match name {
            "max_public_transfer_rows" => limited.public_statement.max_rows = cap,
            "max_public_transfer_transcripts" => limited.public_statement.max_transcripts = cap,
            "max_public_transfer_deltas" => limited.public_statement.max_deltas = cap,
            "max_bundle_segments" => limited.bundle.max_segments = cap,
            "max_queries" => limited.bundle.segment.max_queries = cap,
            "max_bundle_queries" => limited.bundle.max_total_queries = cap,
            "max_proof_bytes" => limited.bundle.segment.max_proof_bytes = cap,
            "max_bundle_segment_bytes" => limited.bundle.max_total_segment_bytes = cap,
            "max_compact_prover_trace_cells" => limited_work.max_total_trace_cells = cap,
            "max_compact_prover_segment_charge_bytes" => {
                limited_work.max_segment_charge_bytes = cap
            }
            "max_compact_producer_statement_bytes" => {
                limited.public_statement.max_public_bytes = cap
            }
            _ => unreachable!(),
        }
        assert_limit(
            check_statement(&statement, expected, limited_work, limited),
            name,
            actual,
            cap,
        );
    }
}

#[test]
fn coefficient_work_budget_rejects_before_statement_digest_or_private_rows() {
    let statement = fixture().model();
    let mut expected = expected(&statement);
    expected.public_statement_digest[0] ^= 1;
    let minimum = DeepTraceCoefficients::required_resources()
        .unwrap()
        .work_units;
    for cap in [0, minimum - 1] {
        let mut limits = proving();
        limits.max_segment_work_units = cap;
        assert_limit(
            check_statement(&statement, expected, limits, policy()),
            "max_compact_prover_segment_work_units",
            minimum,
            cap,
        );
    }
    let mut limits = proving();
    limits.max_segment_work_units = minimum;
    assert!(matches!(
        check_statement(&statement, expected, limits, policy()),
        Err(Error::PublicIoMismatch {
            field: "compact_public_statement_digest"
        })
    ));
}

#[test]
fn producer_binds_original_headers_and_rejects_empty_bundles() {
    let statement = fixture().model();
    let expected = expected(&statement);
    for authority in [true, false] {
        let mut changed = statement.clone();
        if authority {
            changed.transcripts[0].authority_digest = Hash::new(b"different original authority");
        } else {
            changed.transcripts[0].batch_hash = Hash::new(b"different original batch");
        }
        assert!(matches!(
            check_statement(&changed, expected, proving(), policy()),
            Err(Error::PublicIoMismatch {
                field: "compact_public_statement_digest"
            })
        ));
    }
    let mut empty = statement;
    empty.transcripts.clear();
    assert!(matches!(
        check_statement(&empty, expected, proving(), policy()),
        Err(Error::TransferInvariant { details }) if details.contains("nonempty complete bundle")
    ));
}

#[test]
fn impossible_decoder_budget_rejects_before_statement_digest_work() {
    let statement = fixture().model();
    let mut expected = expected(&statement);
    expected.public_statement_digest[0] ^= 1;
    let mut limited = policy();
    limited.max_segment_decode_allocation_charges = 0;
    assert_limit(
        check_statement(&statement, expected, proving(), limited),
        "max_compact_producer_segment_decode_allocation_charges",
        QUERY_COUNT
            * crate::backend::compact_public_columns::COMMITTED_COLUMN_COUNT
            * size_of::<u64>(),
        0,
    );
    assert!(matches!(
        check_statement(&statement, expected, proving(), policy()),
        Err(Error::PublicIoMismatch {
            field: "compact_public_statement_digest"
        })
    ));
}

#[test]
fn artifact_byte_preflight_and_final_encoding_keep_inclusive_bounds() {
    let f = fixture();
    let statement = f.model();
    let (metadata, mirrors) = axt_fields(&f);
    let context = ExpectedAxtContext {
        binding: &f.axt.binding,
        metadata: &metadata,
        mirrors,
        remote_spend_claims: f.axt.remote.as_deref(),
    };
    for axt in [None, Some(context)] {
        let artifact = Artifact::new(&statement, axt);
        assert!(matches!(
            artifact.preflight(0, policy()),
            Err(Error::TransferInvariant { .. })
        ));
        let carrier = 1024 + 64 + 2 * (SHARED_FRAME_BOUND + 32);
        let empty = match &artifact {
            Artifact::Ordinary(value) => norito::core::encoded_frame_len(value).unwrap(),
            Artifact::Axt(value) => norito::core::encoded_frame_len(value).unwrap(),
        };
        let total = empty + carrier + 32;
        let mut exact = policy();
        exact.bundle.max_wire_bytes = carrier;
        exact.transport.max_bundle_frame_bytes = carrier;
        exact.transport.max_wire_bytes = total;
        artifact.preflight(2, exact).unwrap();
        for (name, actual) in [
            ("max_bundle_wire_bytes", carrier),
            ("max_compact_producer_bundle_bytes", carrier),
            ("max_compact_producer_artifact_bytes", total),
        ] {
            let mut limited = exact;
            match name {
                "max_bundle_wire_bytes" => limited.bundle.max_wire_bytes -= 1,
                "max_compact_producer_bundle_bytes" => {
                    limited.transport.max_bundle_frame_bytes -= 1
                }
                _ => limited.transport.max_wire_bytes -= 1,
            }
            assert_limit(artifact.preflight(2, limited), name, actual, actual - 1);
        }
        let bytes = Artifact::new(&statement, axt)
            .finish(vec![1, 2, 3], policy())
            .unwrap();
        let mut exact = policy();
        exact.transport.max_bundle_frame_bytes = 3;
        exact.transport.max_wire_bytes = bytes.len();
        assert_eq!(
            Artifact::new(&statement, axt)
                .finish(vec![1, 2, 3], exact)
                .unwrap(),
            bytes
        );
        exact.transport.max_wire_bytes -= 1;
        assert_limit(
            Artifact::new(&statement, axt).finish(vec![1, 2, 3], exact),
            "max_compact_producer_artifact_bytes",
            bytes.len(),
            bytes.len() - 1,
        );
    }
}

#[test]
fn supplied_root_mismatch_is_not_replaced_with_locally_derived_roots() {
    let f = fixture();
    for old in [true, false] {
        let mut statement = f.model();
        if old {
            statement.public_inputs.old_root = Hash::prehashed([17; 32]).into();
        } else {
            statement.public_inputs.new_root = Hash::prehashed([19; 32]).into();
        }
        let expected = expected(&statement);
        let result = with_prepared_quantity_statement(
            &statement,
            &expected.internal(),
            ProofSemantics::StateTransition,
            policy().public_statement,
            |prepared| prepare_and_prove(prepared, &statement, expected, None, proving(), policy()),
        );
        assert!(
            matches!(result, Err(Error::TransferInvariant { details })
            if details.contains("root")),
            "producer must reject the supplied endpoint"
        );
    }
}

#[test]
fn axt_context_mismatch_precedes_even_private_tree_work() {
    let f = fixture();
    let statement = f.model();
    let expected = expected(&statement);
    let (metadata, mut mirrors) = axt_fields(&f);
    mirrors.manifest_root[0] ^= 1;
    let context = ExpectedAxtContext {
        binding: &f.axt.binding,
        metadata: &metadata,
        mirrors,
        remote_spend_claims: f.axt.remote.as_deref(),
    };
    let mut work = proving();
    work.private_smt = TransferSmtBuildLimits::for_update_limit(0).unwrap();
    let result = prepare_and_prove(
        &f.prepare(ProofSemantics::AxtTransferClaim),
        &statement,
        expected,
        Some(context),
        work,
        policy(),
    );
    assert!(matches!(result, Err(Error::InvalidAxtBinding { details })
        if details.contains("manifest_root")));
}

#[test]
fn all_segment_contexts_are_checked_before_the_first_physical_witness() {
    let (f, private) = QuantityFixture::new(QuantityCase::MixedScale, 2);
    let prepared = f.prepare(ProofSemantics::StateTransition);
    let roots = [private.pairs()[0][1].root_after];
    let batch = PublicTransferBatch::new(
        &prepared,
        &f.expected(),
        &roots,
        BatchContextLimits {
            max_segments: 2,
            max_total_statement_bytes: 512 * 1024,
        },
    )
    .unwrap();
    let mut bad_private = private.pairs().to_vec();
    bad_private[0][0].path_bits[0] ^= 1;
    let calls = Cell::new(0);
    let result = segments(
        batch.statements(),
        &bad_private,
        |ordinal| {
            calls.set(calls.get() + 1);
            if ordinal == 1 {
                return Err(invalid("second segment rejected before any witness"));
            }
            batch.segment(ordinal)
        },
        proving(),
        policy(),
    );
    assert!(matches!(result, Err(Error::TransferInvariant { details })
        if details == "second segment rejected before any witness"));
    assert_eq!(calls.get(), 2);

    calls.set(0);
    let result = segments(
        batch.statements(),
        &bad_private[..1],
        |ordinal| {
            calls.set(calls.get() + 1);
            batch.segment(ordinal)
        },
        proving(),
        policy(),
    );
    assert!(matches!(result, Err(Error::TransferInvariant { details })
        if details.contains("pair count differs")));
    assert_eq!(calls.get(), 0);
}

#[test]
fn malformed_private_paths_fail_before_physical_column_allocation() {
    let (f, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    let prepared = f.prepare(ProofSemantics::StateTransition);
    let statement = prepared.compact_statements(&[]).unwrap().remove(0);
    for wrong_path in [true, false] {
        let mut pair = private.pairs()[0].clone();
        if wrong_path {
            pair[0].path_bits[0] ^= 1;
        } else {
            pair[1].siblings.pop();
        }
        assert!(
            matches!(columns(&statement, &pair), Err(Error::TransferInvariant { details })
            if details.contains("private path differs"))
        );
    }
}

#[test]
#[ignore = "explicit fresh ordinary and AXT public producer proofs over two full-domain segments"]
fn public_producer_generates_complete_ordinary_and_axt_artifacts() {
    // A single test keeps both requests sequential even when the harness uses
    // parallel test threads; the public producer intentionally rejects overlap.
    let f = fixture();
    let statement = f.model();
    let expected = expected(&statement);
    let (metadata, mirrors) = axt_fields(&f);
    let context = ExpectedAxtContext {
        binding: &f.axt.binding,
        metadata: &metadata,
        mirrors,
        remote_spend_claims: f.axt.remote.as_deref(),
    };
    for is_axt in [false, true] {
        let started = std::time::Instant::now();
        let bytes = if is_axt {
            prove_quantity_axt_artifact(&statement, expected, context, proving(), policy())
        } else {
            prove_quantity_ordinary_artifact(&statement, expected, proving(), policy())
        }
        .unwrap();
        let prove_elapsed = started.elapsed();
        let started = std::time::Instant::now();
        let verified = if is_axt {
            verify_quantity_axt_artifact(&bytes, expected, context, policy())
        } else {
            verify_quantity_ordinary_artifact(&bytes, expected, policy())
        }
        .unwrap();
        let verify_elapsed = started.elapsed();
        assert_eq!(verified.expected_statement(), expected);
        assert_eq!(verified.segments(), 2);
        assert_eq!(verified.work().air_evaluations, 2);
        assert_eq!(verified.work().terminal_degree_checks, 2);
        assert_eq!(
            verified.identity().artifact_bytes,
            u64::try_from(bytes.len()).unwrap()
        );
        let label = if is_axt { "axt" } else { "ordinary" };
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let path = retain_public_artifact(label, &bytes);
        eprintln!(
            "quantity_public_producer={label}; bytes={}; sha256={sha}; proving_including_self_verification={prove_elapsed:?}; independent_verification={verify_elapsed:?}; work={:?}; retained={}",
            bytes.len(),
            verified.work(),
            path.display()
        );
    }
}

fn assert_internal_artifact_matches_public(
    bytes: &[u8],
    is_axt: bool,
    expected: ExpectedStatement,
    context: ExpectedAxtContext<'_>,
    public: &crate::offline_compact::VerifiedArtifact,
    raw_bundle: &crate::backend::compact_bundle::VerifiedBundle,
) {
    use crate::backend::compact_model_statement::candidate_artifact::{
        ArtifactLimits, verify_bound_quantity_axt_artifact, verify_bound_quantity_ordinary_artifact,
    };
    let limits = policy();
    let internal_limits = ArtifactLimits {
        transport: limits.transport,
        public_statement: limits.public_statement,
        bundle: limits.bundle.internal(),
        max_segment_decode_allocation_charges: limits.max_segment_decode_allocation_charges,
        total_decode: limits.total_decode,
    };
    let internal = if is_axt {
        verify_bound_quantity_axt_artifact(
            bytes,
            &expected.internal(),
            expected.public_statement_digest,
            context.internal(),
            internal_limits,
        )
    } else {
        verify_bound_quantity_ordinary_artifact(
            bytes,
            &expected.internal(),
            expected.public_statement_digest,
            internal_limits,
        )
    }
    .unwrap();
    // Preserve artifact-level equality independently of the existing direct
    // bundle/public work checks, including the complete canonical identity.
    assert_eq!(internal.identity(), public.identity());
    assert_eq!(internal.bundle(), raw_bundle);
}

fn retained_directory() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation")
}

fn retain_public_artifact(label: &str, bytes: &[u8]) -> std::path::PathBuf {
    use std::io::Write;
    assert!(bytes.len() <= policy().transport.max_wire_bytes);
    let directory = retained_directory();
    std::fs::create_dir_all(&directory).unwrap();
    let sha = format!("{:x}", Sha256::digest(bytes));
    let path = directory.join(format!("quantity-public-producer-deep-{label}-{sha}.bin"));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut output) => {
            output.write_all(bytes).unwrap();
            output.sync_all().unwrap();
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            assert_eq!(
                std::fs::metadata(&path).unwrap().len(),
                u64::try_from(bytes.len()).unwrap()
            );
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
        Err(error) => panic!("retain public artifact: {error}"),
    }
    path
}

fn read_public_artifact(variable: &str, label: &str) -> Vec<u8> {
    use std::io::Read;
    let supplied = std::env::var_os(variable).unwrap_or_else(|| panic!("set {variable}"));
    let path = std::path::Path::new(&supplied).canonicalize().unwrap();
    assert_eq!(
        path.parent().unwrap(),
        retained_directory().canonicalize().unwrap()
    );
    let name = path.file_name().unwrap().to_str().unwrap();
    let sha = name
        .strip_prefix(&format!("quantity-public-producer-deep-{label}-"))
        .and_then(|suffix| suffix.strip_suffix(".bin"))
        .expect("use the SHA-addressed output of the DEEP public producer");
    assert!(sha.len() == 64 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let input = std::fs::File::open(path.as_path()).unwrap();
    let cap = policy().transport.max_wire_bytes;
    assert!(input.metadata().unwrap().len() <= u64::try_from(cap).unwrap());
    let mut bytes = Vec::new();
    input
        .take(u64::try_from(cap).unwrap() + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= cap);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), sha);
    bytes
}

fn raw_bundle_frame(wire: &BundleWire, is_axt: bool) -> Vec<u8> {
    if is_axt {
        norito::encode_canonical(&AxtBundleWire {
            version: wire.version,
            intermediate_roots: wire.intermediate_roots.clone(),
            segments: wire.segments.clone(),
        })
        .unwrap()
    } else {
        norito::encode_canonical(wire).unwrap()
    }
}

fn assert_deep_context_rejected(error: crate::offline_compact::VerificationError) {
    assert!(
        matches!(&error,
            crate::offline_compact::VerificationError::Verify(Error::InvalidTraceShape { details })
            if details == "DEEP out-of-domain AIR quotient identity does not hold"
                || details == "DEEP opening positions differ from the exact derived query set"
        ),
        "changed public context must reach its DEEP binding check: {error:?}"
    );
}

fn assert_valid_public_context(
    statement: &FastpqPublicTransferStatementV1,
    expected: ExpectedStatement,
    context: Option<ExpectedAxtContext<'_>>,
    roots: &[[u8; 32]],
) {
    let semantics = if context.is_some() {
        ProofSemantics::AxtTransferClaim
    } else {
        ProofSemantics::StateTransition
    };
    with_prepared_quantity_statement(
        statement,
        &expected.internal(),
        semantics,
        policy().public_statement,
        |prepared| {
            let limits = BatchContextLimits {
                max_segments: 2,
                max_total_statement_bytes: policy().bundle.max_total_statement_bytes,
            };
            if let Some(context) = context {
                AxtTransferBatch::new(
                    prepared,
                    &expected.internal(),
                    roots,
                    context.internal(),
                    limits,
                )?;
            } else {
                PublicTransferBatch::new(prepared, &expected.internal(), roots, limits)?;
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
#[ignore = "read-only complete artifacts supplied by FASTPQ_TEST_ORDINARY_ARTIFACT and FASTPQ_TEST_AXT_ARTIFACT"]
fn captured_public_producer_artifacts_verify_against_independent_fixture() {
    // The paths are test-only inputs. Expectations come from the same independent
    // fixture as the producer regression, never from the supplied artifact bytes.
    let f = fixture();
    let statement = f.model();
    let expected = expected(&statement);
    let (metadata, mirrors) = axt_fields(&f);
    let context = ExpectedAxtContext {
        binding: &f.axt.binding,
        metadata: &metadata,
        mirrors,
        remote_spend_claims: f.axt.remote.as_deref(),
    };
    let profile = crate::offline_compact::quantity_profile_id();
    for (is_axt, variable) in [
        (false, "FASTPQ_TEST_ORDINARY_ARTIFACT"),
        (true, "FASTPQ_TEST_AXT_ARTIFACT"),
    ] {
        let bytes = read_public_artifact(variable, if is_axt { "axt" } else { "ordinary" });
        let verify = |bytes: &[u8], expected| {
            if is_axt {
                verify_quantity_axt_artifact(bytes, expected, context, policy())
            } else {
                verify_quantity_ordinary_artifact(bytes, expected, policy())
            }
        };
        let started = std::time::Instant::now();
        let verified = {
            let flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
            let verified = verify(&bytes, expected).unwrap();
            assert_eq!(norito::core::get_decode_flags(), flags);
            verified
        };
        let elapsed = started.elapsed();
        let frame = if is_axt {
            let artifact = FastpqAxtCompactArtifactV1::decode_canonical_with_limits(
                &bytes,
                profile,
                policy().transport,
            )
            .unwrap();
            assert_eq!(artifact.statement, statement);
            artifact.bundle_frame
        } else {
            let artifact = FastpqOrdinaryCompactArtifactV1::decode_canonical_with_limits(
                &bytes,
                profile,
                policy().transport,
            )
            .unwrap();
            assert_eq!(artifact.statement, statement);
            artifact.bundle_frame
        };
        let wire: BundleWire = if is_axt {
            let wire: AxtBundleWire =
                norito::decode_canonical_with_limits(&frame, policy().total_decode).unwrap();
            BundleWire {
                version: wire.version,
                intermediate_roots: wire.intermediate_roots,
                segments: wire.segments,
            }
        } else {
            norito::decode_canonical_with_limits(&frame, policy().total_decode).unwrap()
        };
        assert_eq!(verified.expected_statement(), expected);
        assert_eq!(verified.segments(), 2);
        assert_eq!(wire.segments.len(), 2);
        assert_eq!(verified.work().air_evaluations, 2);
        assert_eq!(verified.work().terminal_degree_checks, 2);
        assert_eq!(verified.work().transcripts, 2);
        assert_eq!(verified.bundle_frame_bytes(), frame.len());
        assert_eq!(
            verified.work().proof_bytes,
            wire.segments.iter().map(Vec::len).sum::<usize>()
        );
        let identity = verified.identity();
        assert_eq!(identity.profile_id, profile);
        assert_eq!(
            identity.proof_kind,
            if is_axt {
                FastpqProofKindV1::AxtCompact
            } else {
                FastpqProofKindV1::OrdinaryCompact
            }
        );
        assert_eq!(
            identity.public_statement_digest,
            expected.public_statement_digest
        );
        assert_eq!(
            identity.artifact_digest,
            <[u8; 32]>::from(Hash::new(&bytes))
        );
        assert_eq!(
            identity.inner_bundle_digest,
            <[u8; 32]>::from(Hash::new(&frame))
        );
        assert_eq!(identity.artifact_bytes, u64::try_from(bytes.len()).unwrap());
        assert_ne!(identity.artifact_digest, identity.inner_bundle_digest);
        let FastpqCommitmentDescriptionV1::OrderedCompactAir(roots) = &identity.commitments else {
            panic!("quantity artifact must retain complete ordered AIR commitments")
        };
        assert_eq!(roots.segment_count, 2);
        assert_eq!(roots.segment_air_row_roots.len(), 2);
        assert_eq!(roots.segment_air_row_roots, verified.air_row_roots());
        assert_eq!(
            norito::decode_canonical::<FastpqArtifactIdentityDescriptionV1>(
                &norito::encode_canonical(identity).unwrap()
            )
            .unwrap(),
            *identity
        );

        let mut wrong = expected;
        wrong.public_statement_digest[0] ^= 1;
        assert!(matches!(
            verify(&bytes, wrong),
            Err(crate::offline_compact::VerificationError::Verify(
                Error::PublicIoMismatch {
                    field: "compact_artifact_public_statement_digest"
                }
            ))
        ));
        wrong = expected;
        wrong.inputs.old_root[0] ^= 1;
        assert!(matches!(
            verify(&bytes, wrong),
            Err(crate::offline_compact::VerificationError::Verify(
                Error::PublicIoMismatch {
                    field: "compact_model_public_io"
                }
            ))
        ));

        // Preserve the complete raw-bundle acceptance, ordering, root-chain,
        // context and resource assertions through the normal public entry point.
        assert_eq!(verified.work().row_leaves, 2 * QUERY_COUNT);
        assert_eq!(verified.work().oracle_leaves, 2 * QUERY_COUNT);
        assert!(frame.len() <= 1024 * 1024);
        assert!(bytes.len() <= 1024 * 1024);
        let mut observed_roots = Vec::new();
        let mut fri_leaves = 0;
        let mut parent_hashes = 0;
        for child in &wire.segments {
            assert!(child.len() <= 512 * 1024);
            let proof = crate::backend::deep_proof::decode_with_allocation(
                child,
                512 * 1024,
                policy().max_segment_decode_allocation_charges,
            )
            .unwrap();
            observed_roots.push(proof.row_root);
            fri_leaves += proof
                .rounds
                .iter()
                .map(|round| round.groups.len())
                .sum::<usize>()
                + 1;
            // Each binary multiproof reconstructs leaves + frontier - 1 parents;
            // the sole terminal leaf has one required duplicate-child parent.
            parent_hashes += proof.rows.len() + proof.row_siblings.len() - 1
                + proof.quotients.len()
                + proof.quotient_siblings.len()
                - 1
                + proof
                    .rounds
                    .iter()
                    .map(|round| round.groups.len() + round.siblings.len() - 1)
                    .sum::<usize>()
                + 1;
        }
        assert_eq!(observed_roots, verified.air_row_roots());
        assert_eq!(verified.work().fri_leaves, fri_leaves);
        assert_eq!(verified.work().parent_hashes, parent_hashes);
        assert_ne!(wire.intermediate_roots[0], expected.inputs.old_root);
        assert_ne!(wire.intermediate_roots[0], expected.inputs.new_root);
        assert_valid_public_context(
            &statement,
            expected,
            is_axt.then_some(context),
            &wire.intermediate_roots,
        );

        let verify_limits = |raw: &[u8], limits| {
            if is_axt {
                verify_quantity_axt_artifact(raw, expected, context, limits)
            } else {
                verify_quantity_ordinary_artifact(raw, expected, limits)
            }
        };
        // Inclusive cumulative outer/child charges and elements remain in force
        // even inside a stricter caller scope. No child can reset that scope.
        let (measured, usage) =
            norito::core::with_decode_limits_measured(policy().total_decode, || {
                verify_limits(&bytes, policy())
            });
        assert_eq!(measured.unwrap(), verified);
        assert!(usage.total_allocated_bytes() > frame.len());
        let mut exact = policy();
        exact.total_decode = DecodeLimits::new(
            20 * 1024 * 1024,
            20 * 1024 * 1024,
            usage.total_elements(),
            usage.total_allocated_bytes(),
            32,
        );
        assert_eq!(verify_limits(&bytes, exact).unwrap(), verified);
        for (elements, allocation) in [
            (usage.total_elements() - 1, usage.total_allocated_bytes()),
            (usage.total_elements(), usage.total_allocated_bytes() - 1),
        ] {
            let mut low = exact;
            low.total_decode =
                DecodeLimits::new(20 * 1024 * 1024, 20 * 1024 * 1024, elements, allocation, 32);
            assert!(verify_limits(&bytes, low).is_err());
            assert!(
                norito::core::with_decode_limits_scope(low.total_decode, || verify_limits(
                    &bytes,
                    policy()
                ))
                .is_err()
            );
        }
        let (raw_result, bundle_usage) = with_prepared_quantity_statement(
            &statement,
            &expected.internal(),
            if is_axt {
                ProofSemantics::AxtTransferClaim
            } else {
                ProofSemantics::StateTransition
            },
            policy().public_statement,
            |prepared| {
                Ok(norito::core::with_decode_limits_measured(
                    policy().total_decode,
                    || {
                        if is_axt {
                            compact_bundle::verify_axt_transfer_bundle_with_allocation(
                                prepared,
                                &expected.internal(),
                                context.internal(),
                                &frame,
                                policy().bundle.internal(),
                                policy().max_segment_decode_allocation_charges,
                            )
                        } else {
                            compact_bundle::verify_transfer_bundle_with_allocation(
                                prepared,
                                &expected.internal(),
                                &frame,
                                policy().bundle.internal(),
                                policy().max_segment_decode_allocation_charges,
                            )
                        }
                    },
                ))
            },
        )
        .unwrap();
        let raw_result = raw_result.unwrap();
        assert_internal_artifact_matches_public(
            &bytes,
            is_axt,
            expected,
            context,
            &verified,
            &raw_result,
        );
        assert_eq!(raw_result.public_io(), expected.internal());
        assert_eq!(raw_result.row_roots(), verified.air_row_roots());
        assert_eq!(raw_result.statement_bytes(), verified.statement_bytes());
        assert_eq!(raw_result.wire_bytes(), verified.bundle_frame_bytes());
        let raw_work = raw_result.work();
        assert_eq!(
            verified.work(),
            crate::offline_compact::VerificationWork {
                proof_bytes: raw_work.proof_bytes,
                transcripts: raw_work.transcripts,
                row_leaves: raw_work.row_leaves,
                oracle_leaves: raw_work.oracle_leaves,
                fri_leaves: raw_work.fri_leaves,
                parent_hashes: raw_work.parent_hashes,
                air_evaluations: raw_work.air_evaluations,
                terminal_degree_checks: raw_work.terminal_degree_checks,
            }
        );
        let mut exact_bundle = policy();
        exact_bundle.bundle.max_total_decode_allocation_charges =
            bundle_usage.total_allocated_bytes();
        assert_eq!(verify_limits(&bytes, exact_bundle).unwrap(), verified);
        exact_bundle.bundle.max_total_decode_allocation_charges -= 1;
        assert!(verify_limits(&bytes, exact_bundle).is_err());

        for boundary in 0..5 {
            let mut low = policy();
            match boundary {
                0 => low.bundle.max_segments = 1,
                1 => low.bundle.max_total_queries = 2 * QUERY_COUNT - 1,
                2 => low.bundle.max_total_statement_bytes = verified.statement_bytes() - 1,
                3 => low.bundle.max_wire_bytes = frame.len() - 1,
                4 => {
                    low.bundle.segment.max_proof_bytes =
                        wire.segments.iter().map(Vec::len).max().unwrap() - 1
                }
                _ => unreachable!(),
            }
            assert!(
                verify_limits(&bytes, low).is_err(),
                "inclusive policy boundary {boundary}"
            );
        }
        let mut no_child_allocation = policy();
        no_child_allocation.max_segment_decode_allocation_charges = 0;
        assert!(verify_limits(&bytes, no_child_allocation).is_err());
        let mut one_child = policy();
        one_child.bundle.max_segments = 1;
        assert!(verify_limits(&bytes, one_child).is_err());

        // Re-encode valid transports so failures exercise the child/context
        // checks, not a damaged outer checksum. Exact-count errors still reject
        // the complete bundle; no successfully checked prefix is returned.
        for mutation in 0..7 {
            let mut changed = wire.clone();
            match mutation {
                0 => changed.segments.swap(0, 1),
                1 => changed.segments[1] = changed.segments[0].clone(),
                2 => {
                    changed.segments.pop();
                }
                // A short extra carrier keeps this malformed count within the
                // enclosing byte cap; count rejection precedes child decoding.
                3 => changed.segments.push(vec![0]),
                4 => changed.intermediate_roots[0][0] ^= 1,
                5 => {
                    let last = changed.segments[1].len() - 1;
                    changed.segments[1][last] ^= 1;
                }
                6 => {
                    assert!(changed.segments[0].pop().is_some());
                }
                _ => unreachable!(),
            }
            if mutation == 4 {
                assert_valid_public_context(
                    &statement,
                    expected,
                    is_axt.then_some(context),
                    &changed.intermediate_roots,
                );
            }
            let changed = Artifact::new(&statement, is_axt.then_some(context))
                .finish(raw_bundle_frame(&changed, is_axt), policy())
                .unwrap();
            let error = verify(&changed, expected).unwrap_err();
            if matches!(mutation, 0 | 1 | 4) {
                assert_deep_context_rejected(error);
            } else if matches!(mutation, 2 | 3) {
                assert!(matches!(
                    error,
                    crate::offline_compact::VerificationError::Verify(Error::InvalidTraceShape { details })
                        if details == "compact bundle segment/root count mismatch"
                ));
            } else if mutation == 6 {
                assert!(matches!(
                    error,
                    crate::offline_compact::VerificationError::Verify(Error::Encode(_))
                ));
            }
        }
        let mut corrupted_artifact = bytes.clone();
        *corrupted_artifact.last_mut().unwrap() ^= 1;
        assert!(verify(&corrupted_artifact, expected).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(verify(&trailing, expected).is_err());
        let mut trailing_bundle = frame.clone();
        trailing_bundle.push(0);
        let trailing = Artifact::new(&statement, is_axt.then_some(context))
            .finish(trailing_bundle, policy())
            .unwrap();
        assert!(verify(&trailing, expected).is_err());

        // Change both the advertisement and independent expectation together:
        // these remain valid public statements and must fail proof binding.
        for change_authority in [false, true] {
            let mut changed_statement = statement.clone();
            if change_authority {
                changed_statement.transcripts[1].authority_digest =
                    Hash::new(b"different second occurrence authority");
            } else {
                changed_statement.public_inputs.perm_root[0] ^= 1;
            }
            let changed_expected = self::expected(&changed_statement);
            assert_valid_public_context(
                &changed_statement,
                changed_expected,
                is_axt.then_some(context),
                &wire.intermediate_roots,
            );
            let changed = Artifact::new(&changed_statement, is_axt.then_some(context))
                .finish(frame.clone(), policy())
                .unwrap();
            assert_deep_context_rejected(verify(&changed, changed_expected).unwrap_err());
        }

        if is_axt {
            use iroha_data_model::nexus::compute_remote_spend_claim_commitment_v1;
            let remote = context.remote_spend_claims.unwrap();
            assert_eq!(remote.len(), 2);
            assert_ne!(remote[0].handle_replay_key, remote[1].handle_replay_key);
            assert_eq!(remote[0].effective_amount, remote[1].effective_amount);
            assert_eq!(remote[0].from, remote[1].from);
            assert_eq!(remote[0].to, remote[1].to);
            let mut changed_metadata = metadata.clone();
            changed_metadata.manifest_root[0] ^= 1;
            let changed_metadata_context = ExpectedAxtContext {
                metadata: &changed_metadata,
                mirrors: FastpqAxtPreProofMirrorsV1 {
                    manifest_root: changed_metadata.manifest_root,
                    ..mirrors
                },
                ..context
            };
            let mut changed_remote = remote.to_vec();
            changed_remote[0].handle_replay_key.handle_era = 17;
            changed_remote.sort_by_key(compute_remote_spend_claim_commitment_v1);
            let mut changed_binding = (*context.binding).clone();
            changed_binding.remote_spend_intent_commitments = changed_remote
                .iter()
                .map(compute_remote_spend_claim_commitment_v1)
                .collect();
            let changed_remote_context = ExpectedAxtContext {
                binding: &changed_binding,
                remote_spend_claims: Some(&changed_remote),
                ..context
            };
            for changed_context in [changed_metadata_context, changed_remote_context] {
                assert_valid_public_context(
                    &statement,
                    expected,
                    Some(changed_context),
                    &wire.intermediate_roots,
                );
                let changed = Artifact::new(&statement, Some(changed_context))
                    .finish(frame.clone(), policy())
                    .unwrap();
                assert_deep_context_rejected(
                    verify_quantity_axt_artifact(&changed, expected, changed_context, policy())
                        .unwrap_err(),
                );
            }
            let mut omitted_remote = remote.to_vec();
            omitted_remote.pop();
            let mut omitted_binding = (*context.binding).clone();
            omitted_binding.remote_spend_intent_commitments = omitted_remote
                .iter()
                .map(compute_remote_spend_claim_commitment_v1)
                .collect();
            let omitted = ExpectedAxtContext {
                binding: &omitted_binding,
                remote_spend_claims: Some(&omitted_remote),
                ..context
            };
            let changed = Artifact::new(&statement, Some(omitted))
                .finish(frame.clone(), policy())
                .unwrap();
            assert!(
                matches!(verify_quantity_axt_artifact(&changed, expected, omitted, policy()),
                Err(crate::offline_compact::VerificationError::Verify(Error::InvalidAxtBinding { details }))
                if details.contains("one-for-one"))
            );
            let missing = ExpectedAxtContext {
                remote_spend_claims: None,
                ..context
            };
            let changed = Artifact::new(&statement, Some(missing))
                .finish(frame.clone(), policy())
                .unwrap();
            assert!(matches!(
                verify_quantity_axt_artifact(&changed, expected, missing, policy()),
                Err(crate::offline_compact::VerificationError::Verify(
                    Error::MissingMetadata { .. }
                ))
            ));
            let mut wrong_mirrors = mirrors;
            wrong_mirrors.manifest_root[0] ^= 1;
            let wrong = ExpectedAxtContext {
                mirrors: wrong_mirrors,
                ..context
            };
            let changed = Artifact::new(&statement, Some(wrong))
                .finish(frame.clone(), policy())
                .unwrap();
            assert!(matches!(
                verify_quantity_axt_artifact(&changed, expected, wrong, policy()),
                Err(crate::offline_compact::VerificationError::Verify(
                    Error::InvalidAxtBinding { .. }
                ))
            ));
        }
        // Retag both enclosing transports while leaving authenticated children
        // intact: the distinct ordinary/AXT relation identity still rejects.
        let opposite = !is_axt;
        assert_valid_public_context(
            &statement,
            expected,
            opposite.then_some(context),
            &wire.intermediate_roots,
        );
        let retagged = Artifact::new(&statement, opposite.then_some(context))
            .finish(raw_bundle_frame(&wire, opposite), policy())
            .unwrap();
        let error = if opposite {
            verify_quantity_axt_artifact(&retagged, expected, context, policy())
        } else {
            verify_quantity_ordinary_artifact(&retagged, expected, policy())
        }
        .unwrap_err();
        assert_deep_context_rejected(error);
        if is_axt {
            assert!(verify_quantity_ordinary_artifact(&bytes, expected, policy()).is_err());
        } else {
            assert!(verify_quantity_axt_artifact(&bytes, expected, context, policy()).is_err());
        }

        eprintln!(
            "captured_quantity_artifact={variable}; bytes={}; sha256={:x}; bundle_frame_bytes={}; segment_bytes={:?}; independent_verification={elapsed:?}; work={:?}",
            bytes.len(),
            Sha256::digest(&bytes),
            frame.len(),
            wire.segments.iter().map(Vec::len).collect::<Vec<_>>(),
            verified.work()
        );
    }
}
