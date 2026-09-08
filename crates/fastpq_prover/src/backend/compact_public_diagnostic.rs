//! Explicit complete candidate AXT and multi-delta public verification diagnostics.
//!
//! Fixtures retain public facts only after proving. Each carrier uses a distinct
//! schema and all children must verify under original complete caller context.
//! This local diagnostic does not qualify cryptography or production admission.

use super::*;
use crate::{
    backend::{
        compact_axt_batch::AxtTransferBatch,
        compact_axt_context::tests::Fixture,
        compact_bundle::{self as bundle, AxtBundleWire, BundleLimits, BundleWire},
        compact_protocol::shared_openings::prove_shared,
        compact_public_batch::{BatchContextLimits, PublicTransferBatch},
    },
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
        compact_trace_columns::smt_row_cells,
        public_transfer_statement::{PublicTransferLimits, public_claims_from_transcripts},
        transfer::attach_transfer_smt_witnesses,
    },
};
use iroha_data_model::fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript};
use sha2::{Digest as _, Sha256};

type Columns = Vec<Vec<u64>>;

fn context(fixture: &Fixture) -> AxtVerificationContext<'_> {
    AxtVerificationContext {
        binding: &fixture.binding,
        metadata: fixture.metadata(),
        mirrors: fixture.outer,
        remote_spend_claims: fixture.remote.as_deref(),
    }
}

fn policy(count: usize) -> BundleLimits {
    BundleLimits {
        max_segments: count,
        max_wire_bytes: 16 * 1024 * 1024,
        max_total_segment_bytes: 16 * 1024 * 1024,
        max_total_statement_bytes: 512 * 1024,
        max_total_queries: 375 * count,
        max_total_decode_allocation_charges: 128 * 1024 * 1024,
        segment: VerifyLimits {
            max_proof_bytes: 4_326_227,
            max_queries: 375,
            ..VerifyLimits::default()
        },
    }
}

fn public_fixture_and_columns(
    count: usize,
    remote: bool,
) -> (Fixture, Vec<[u8; 32]>, Vec<Columns>) {
    let mut fixture = Fixture::multiple(count, remote);
    let mut transcripts: Vec<_> = {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        prepared
            .claims()
            .iter()
            .map(|claim| TransferTranscript {
                batch_hash: claim.batch_hash,
                authority_digest: claim.authority_digest,
                poseidon_preimage_digest: claim.poseidon_preimage_digest,
                deltas: claim
                    .deltas
                    .iter()
                    .map(|delta| TransferDeltaTranscript {
                        from_account: delta.from_account.clone(),
                        to_account: delta.to_account.clone(),
                        asset_definition: delta.asset_definition.clone(),
                        amount: delta.amount.clone(),
                        from_balance_before: delta.from_balance_before.clone(),
                        from_balance_after: delta.from_balance_after.clone(),
                        to_balance_before: delta.to_balance_before.clone(),
                        to_balance_after: delta.to_balance_after.clone(),
                        from_smt_witness: TransferSmtWitness::default(),
                        to_smt_witness: TransferSmtWitness::default(),
                    })
                    .collect(),
            })
            .collect()
    };
    let (old_root, new_root) = attach_transfer_smt_witnesses(&mut transcripts).unwrap();
    fixture.set_touched_roots(old_root, new_root);
    let deltas: Vec<_> = transcripts.iter().flat_map(|t| &t.deltas).collect();
    assert_eq!(deltas.len(), count);
    for pair in deltas.windows(2) {
        assert_eq!(
            pair[0].to_smt_witness.root_after,
            pair[1].from_smt_witness.root_before
        );
    }
    let intermediate: Vec<_> = deltas[..count - 1]
        .iter()
        .map(|d| d.to_smt_witness.root_after)
        .collect();
    let columns = {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        assert_eq!(
            public_claims_from_transcripts(&transcripts, PublicTransferLimits::default())
                .unwrap()
                .as_slice(),
            prepared.claims()
        );
        let statements = prepared.compact_statements(&intermediate).unwrap();
        assert_eq!(statements.len(), count);
        deltas
            .iter()
            .zip(&statements)
            .map(|(delta, statement)| {
                let paths = [&delta.from_smt_witness, &delta.to_smt_witness];
                for (path, update) in paths.iter().zip(statement.updates) {
                    assert_eq!(path.path_bits.as_slice(), update.path.to_le_bytes());
                    assert_eq!(path.siblings.len(), PATH_LEVELS);
                }
                let siblings = core::array::from_fn(|update| {
                    core::array::from_fn(|level| {
                        let bytes = paths[update].siblings[level];
                        core::array::from_fn(|limb| {
                            u32::from_le_bytes(bytes[4 * limb..4 * limb + 4].try_into().unwrap())
                        })
                    })
                });
                let witness = SmtWitness::from_inputs(statement, &siblings)
                    .unwrap()
                    .into_physical();
                let mut columns: Columns = (0..COLUMN_COUNT)
                    .map(|_| Vec::with_capacity(PHYSICAL_ROW_COUNT))
                    .collect();
                for row in witness.rows() {
                    for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
                        column.push(value);
                    }
                }
                columns
            })
            .collect()
    };
    // No private path, witness or transcript leaves this helper.
    (fixture, intermediate, columns)
}

fn retain_public_frame(label: &str, bytes: &[u8]) -> String {
    let sha = format!("{:x}", Sha256::digest(bytes));
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("compact-v1-{label}-{sha}.bin"));
    std::fs::write(&path, bytes).unwrap();
    path.display().to_string()
}

#[test]
#[ignore = "explicit complete candidate AXT proof and typed raw facade verification"]
fn complete_candidate_axt_facade_drops_private_data_before_verification() {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let started = std::time::Instant::now();
    let (fixture, roots, mut columns) = public_fixture_and_columns(1, true);
    assert!(roots.is_empty());
    let limits = policy(1).segment;
    let proving_started = std::time::Instant::now();
    let bytes = {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected(&prepared);
        let air = AxtTransferAir::new(
            &prepared,
            &expected,
            &fixture.binding,
            fixture.metadata(),
            fixture.outer,
            fixture.remote.as_deref(),
        )
        .unwrap();
        let columns = columns.pop().unwrap();
        let proof = prove_shared(
            &air,
            &columns,
            VerifyLimits {
                max_proof_bytes: 16 * 1024 * 1024,
                ..limits
            },
        )
        .unwrap();
        norito::encode_canonical(&proof).unwrap()
    };
    drop(columns);
    let proving = proving_started.elapsed();
    eprintln!(
        "candidate AXT proving complete: {proving:?}; {} bytes",
        bytes.len()
    );
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let expected = fixture.expected(&prepared);
    let verifying_started = std::time::Instant::now();
    let budget =
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 64 * 1024 * 1024, 16);
    let (result, usage) = norito::core::with_decode_limits_measured(budget, || {
        verify_axt_transfer_with_allocation(
            &prepared,
            &expected,
            context(&fixture),
            &bytes,
            limits,
            64 * 1024 * 1024,
        )
    });
    let result = result.unwrap();
    let verifying = verifying_started.elapsed();
    assert_eq!(result.public_io(), expected);
    assert_eq!(result.work().air_evaluations, 375);
    assert_eq!(result.work().transcripts, 1);
    assert_eq!(result.work().terminal_degree_checks, 1);
    assert_eq!(result.work().proof_bytes, bytes.len());
    let charges = usage.total_allocated_bytes();
    assert!(charges < 64 * 1024 * 1024);
    // The outer scope counts AXT metadata validation as well as child decoding.
    let exact = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, charges, 16);
    assert_eq!(
        norito::core::with_decode_limits_scope(exact, || verify_axt_transfer_with_allocation(
            &prepared,
            &expected,
            context(&fixture),
            &bytes,
            limits,
            64 * 1024 * 1024
        ))
        .unwrap(),
        result
    );
    let low = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, charges - 1, 16);
    assert!(
        norito::core::with_decode_limits_scope(low, || verify_axt_transfer_with_allocation(
            &prepared,
            &expected,
            context(&fixture),
            &bytes,
            limits,
            64 * 1024 * 1024
        ))
        .is_err()
    );
    assert!(verify_axt_transfer(&prepared, &expected, context(&fixture), &bytes, limits).is_err());
    let ordinary = fixture.prepare(ProofSemantics::StateTransition);
    assert!(
        verify_transfer_with_allocation(
            &ordinary,
            &fixture.expected(&ordinary),
            &bytes,
            limits,
            64 * 1024 * 1024
        )
        .is_err()
    );
    let mut changed = context(&fixture);
    changed.remote_spend_claims = None;
    assert!(
        verify_axt_transfer_with_allocation(
            &prepared,
            &expected,
            changed,
            &bytes,
            limits,
            64 * 1024 * 1024
        )
        .is_err()
    );
    let mut corrupt = bytes.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 1;
    assert!(
        verify_axt_transfer_with_allocation(
            &prepared,
            &expected,
            context(&fixture),
            &corrupt,
            limits,
            64 * 1024 * 1024
        )
        .is_err()
    );
    eprintln!(
        "candidate_axt_proving={proving:?}; raw_facade_verifying={verifying:?}; work={:?}; decode_allocation_charges={charges}; total={:?}; public_fixture_artifact={}; production_security_qualified=false",
        result.work(),
        started.elapsed(),
        retain_public_frame("axt-single", &bytes)
    );
}

fn complete_candidate_bundle(axt: bool) {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let started = std::time::Instant::now();
    let (fixture, roots, columns) = public_fixture_and_columns(2, axt);
    let semantics = if axt {
        ProofSemantics::AxtTransferClaim
    } else {
        ProofSemantics::StateTransition
    };
    let limits = policy(2);
    let mut proving_seconds = Vec::new();
    let bytes = {
        let prepared = fixture.prepare(semantics);
        let expected = fixture.expected(&prepared);
        let context_limits = BatchContextLimits {
            max_segments: 2,
            max_total_statement_bytes: limits.max_total_statement_bytes,
        };
        let mut frames = Vec::new();
        if axt {
            let batch = AxtTransferBatch::new(
                &prepared,
                &expected,
                &roots,
                context(&fixture),
                context_limits,
            )
            .unwrap();
            for (ordinal, columns) in columns.into_iter().enumerate() {
                let relation = batch.segment(ordinal).unwrap();
                let start = std::time::Instant::now();
                let proof = prove_shared(
                    &relation,
                    &columns,
                    VerifyLimits {
                        max_proof_bytes: 16 * 1024 * 1024,
                        ..limits.segment
                    },
                )
                .unwrap();
                proving_seconds.push(start.elapsed().as_secs_f64());
                drop(columns);
                frames.push(norito::encode_canonical(&proof).unwrap());
                eprintln!(
                    "candidate AXT segment {ordinal} proved in {}s",
                    proving_seconds[ordinal]
                );
            }
            bundle::encode_axt_wire(
                &AxtBundleWire {
                    version: 1,
                    intermediate_roots: roots,
                    segments: frames,
                },
                2,
                limits,
            )
            .unwrap()
        } else {
            let batch =
                PublicTransferBatch::new(&prepared, &expected, &roots, context_limits).unwrap();
            for (ordinal, columns) in columns.into_iter().enumerate() {
                let relation = batch.segment(ordinal).unwrap();
                let start = std::time::Instant::now();
                let proof = prove_shared(
                    &relation,
                    &columns,
                    VerifyLimits {
                        max_proof_bytes: 16 * 1024 * 1024,
                        ..limits.segment
                    },
                )
                .unwrap();
                proving_seconds.push(start.elapsed().as_secs_f64());
                drop(columns);
                frames.push(norito::encode_canonical(&proof).unwrap());
                eprintln!(
                    "candidate ordinary segment {ordinal} proved in {}s",
                    proving_seconds[ordinal]
                );
            }
            bundle::encode_wire(
                &BundleWire {
                    version: 1,
                    intermediate_roots: roots,
                    segments: frames,
                },
                2,
                limits,
            )
            .unwrap()
        }
        // Prover relations, columns, trees, opening DTOs and carrier DTOs all drop.
    };
    let prepared = fixture.prepare(semantics);
    let expected = fixture.expected(&prepared);
    let verify = |bytes: &[u8], limits: BundleLimits| {
        if axt {
            bundle::verify_axt_transfer_bundle_with_allocation(
                &prepared,
                &expected,
                context(&fixture),
                bytes,
                limits,
                64 * 1024 * 1024,
            )
        } else {
            bundle::verify_transfer_bundle_with_allocation(
                &prepared,
                &expected,
                bytes,
                limits,
                64 * 1024 * 1024,
            )
        }
    };
    let start = std::time::Instant::now();
    let (result, usage) = norito::core::with_decode_limits_measured(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 16),
        || verify(&bytes, limits),
    );
    let result = result.unwrap();
    let verifying = start.elapsed();
    let charges = usage.total_allocated_bytes();
    assert_eq!(result.public_io(), expected);
    assert_eq!(result.segments(), 2);
    assert_eq!(result.work().transcripts, 2);
    assert_eq!(result.work().air_evaluations, 750);
    assert_eq!(result.work().terminal_degree_checks, 2);
    assert_eq!(result.wire_bytes(), bytes.len());
    assert!(charges < limits.max_total_decode_allocation_charges);
    assert_eq!(
        verify(
            &bytes,
            BundleLimits {
                max_total_decode_allocation_charges: charges,
                ..limits
            }
        )
        .unwrap(),
        result
    );
    assert!(
        verify(
            &bytes,
            BundleLimits {
                max_total_decode_allocation_charges: charges - 1,
                ..limits
            }
        )
        .is_err()
    );
    assert!(matches!(
        verify(
            &bytes,
            BundleLimits {
                max_total_queries: 749,
                ..limits
            }
        ),
        Err(Error::VerifierLimitExceeded {
            limit: "max_bundle_queries",
            ..
        })
    ));
    assert!(verify(&bytes, BundleLimits::default()).is_err());
    // Preserve the nominal carrier while substituting order, root linkage and
    // an invalid final child after a valid prefix. Every case rejects completely.
    let (roots, frames) = if axt {
        let w: AxtBundleWire = norito::decode_canonical(&bytes).unwrap();
        (w.intermediate_roots, w.segments)
    } else {
        let w: BundleWire = norito::decode_canonical(&bytes).unwrap();
        (w.intermediate_roots, w.segments)
    };
    for kind in 0..5 {
        let mut roots = roots.clone();
        let mut frames = frames.clone();
        match kind {
            0 => frames.swap(0, 1),
            1 => {
                let last = frames[1].len() - 1;
                frames[1][last] ^= 1;
            }
            2 => frames[1] = frames[0].clone(),
            3 => roots[0][0] ^= 1,
            4 => {
                frames.pop();
            }
            _ => unreachable!(),
        }
        let corrupt = if axt {
            norito::encode_canonical(&AxtBundleWire {
                version: 1,
                intermediate_roots: roots,
                segments: frames,
            })
            .unwrap()
        } else {
            norito::encode_canonical(&BundleWire {
                version: 1,
                intermediate_roots: roots,
                segments: frames,
            })
            .unwrap()
        };
        assert!(
            verify(&corrupt, limits).is_err(),
            "candidate bundle tamper {kind}"
        );
    }
    if axt {
        assert!(
            bundle::verify_axt_transfer_bundle(
                &prepared,
                &expected,
                context(&fixture),
                &bytes,
                limits
            )
            .is_err()
        );
        let ordinary = fixture.prepare(ProofSemantics::StateTransition);
        let retagged = bundle::encode_wire(
            &BundleWire {
                version: 1,
                intermediate_roots: roots,
                segments: frames,
            },
            2,
            limits,
        )
        .unwrap();
        assert!(
            bundle::verify_transfer_bundle_with_allocation(
                &ordinary,
                &fixture.expected(&ordinary),
                &retagged,
                limits,
                64 * 1024 * 1024
            )
            .is_err()
        );
    } else {
        assert!(bundle::verify_transfer_bundle(&prepared, &expected, &bytes, limits).is_err());
        let axt_prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let retagged = bundle::encode_axt_wire(
            &AxtBundleWire {
                version: 1,
                intermediate_roots: roots,
                segments: frames,
            },
            2,
            limits,
        )
        .unwrap();
        assert!(
            bundle::verify_axt_transfer_bundle_with_allocation(
                &axt_prepared,
                &fixture.expected(&axt_prepared),
                context(&fixture),
                &retagged,
                limits,
                64 * 1024 * 1024
            )
            .is_err()
        );
    }
    let label = if axt {
        "axt-two-delta"
    } else {
        "ordinary-two-delta"
    };
    eprintln!(
        "candidate_bundle={label}; proving_seconds={proving_seconds:?}; raw_verifying={verifying:?}; outer_bytes={}; public_statement_bytes={}; work={:?}; decode_allocation_charges={charges}; total={:?}; public_fixture_artifact={}; production_security_qualified=false",
        bytes.len(),
        result.statement_bytes(),
        result.work(),
        started.elapsed(),
        retain_public_frame(label, &bytes)
    );
}

#[test]
#[ignore = "explicit two complete candidate ordinary proofs and cumulative raw bundle verification"]
fn complete_candidate_ordinary_bundle_drops_private_data_before_verification() {
    complete_candidate_bundle(false);
}

#[test]
#[ignore = "explicit two complete candidate AXT proofs and cumulative raw bundle verification"]
fn complete_candidate_axt_bundle_drops_private_data_before_verification() {
    complete_candidate_bundle(true);
}
