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
    FastpqAxtPreProofMirrorsV1, FastpqAxtPublicMetadataV1, FastpqCompactArtifactDecodeLimits,
};
use norito::core::DecodeLimits;
use sha2::{Digest as _, Sha256};

fn policy() -> VerificationLimits {
    VerificationLimits {
        transport: FastpqCompactArtifactDecodeLimits {
            max_wire_bytes: 20 * 1024 * 1024,
            max_bundle_frame_bytes: 16 * 1024 * 1024,
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
            max_wire_bytes: 16 * 1024 * 1024,
            max_total_segment_bytes: 16 * 1024 * 1024,
            max_total_statement_bytes: 512 * 1024,
            max_total_queries: 750,
            max_total_decode_allocation_charges: 128 * 1024 * 1024,
            segment: VerifyLimits {
                max_proof_bytes: 5 * 1024 * 1024,
                max_queries: 375,
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
        private_smt: TransferSmtBuildLimits::for_update_limit(4).unwrap(),
        max_total_trace_cells: 2 * COLUMN_COUNT * PHYSICAL_ROW_COUNT,
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
    exact.bundle.max_total_queries = 750;
    exact.bundle.segment.max_queries = 375;
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
        ("max_queries", 375),
        ("max_bundle_queries", 750),
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
        375 * COLUMN_COUNT * size_of::<u64>(),
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
        assert_eq!(verified.work().air_evaluations, 750);
        assert_eq!(verified.work().terminal_degree_checks, 2);
        assert_eq!(
            verified.identity().artifact_bytes,
            u64::try_from(bytes.len()).unwrap()
        );
        let label = if is_axt { "axt" } else { "ordinary" };
        let sha = format!("{:x}", Sha256::digest(&bytes));
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/fastpq-production-validation");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("quantity-public-producer-{label}-{sha}.bin"));
        std::fs::write(&path, &bytes).unwrap();
        eprintln!(
            "quantity_public_producer={label}; bytes={}; sha256={sha}; proving_including_self_verification={prove_elapsed:?}; independent_verification={verify_elapsed:?}; work={:?}; retained={}",
            bytes.len(),
            verified.work(),
            path.display()
        );
    }
}
