//! Explicit full-domain proof diagnostics, excluded from routine unit test runs.
//!
//! TODO: Qualify these test-only candidates and integrate authenticated source
//! expectations before production use. Mathematical verification is not finality.

use super::{
    compact_axt_air::AxtTransferAir,
    compact_axt_batch::AxtTransferBatch,
    compact_bundle::{self, BundleLimits, ShakeAxtBundleWire, ShakeBundleWire},
    compact_protocol::{
        FixedAir, FixedAirSchema, PreparedAir, shared_openings::prove_shake_shared,
    },
    compact_public_api::{SharedVerifier, verify_shake_axt_transfer, verify_shake_transfer},
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
    compact_public_transfer::PublicTransferAir,
    compact_quantity_tests::{QuantityCase, QuantityFixture},
    compact_value_domain::CompactTransferValue,
};
use crate::{
    ProofSemantics, VerifyLimits,
    gadgets::{
        compact_smt_air::{COLUMN_COUNT, PATH_LEVELS, PHYSICAL_ROW_COUNT, SmtWitness},
        compact_trace_columns::smt_row_cells,
        public_transfer_statement::DerivedTransferSmtWitnesses,
    },
};
use iroha_crypto::Hash;
use sha2::{Digest as _, Sha256};

type Columns = Vec<Vec<u64>>;

fn policy(count: usize) -> BundleLimits {
    BundleLimits {
        max_segments: count,
        max_wire_bytes: 16 * 1024 * 1024,
        max_total_segment_bytes: 16 * 1024 * 1024,
        max_total_statement_bytes: 512 * 1024,
        max_total_queries: count * 375,
        max_total_decode_allocation_charges: 192 * 1024 * 1024,
        segment: VerifyLimits {
            max_proof_bytes: 5 * 1024 * 1024,
            max_queries: 375,
            ..VerifyLimits::default()
        },
    }
}

fn prove(relation: &impl FixedAir, columns: Columns, limits: VerifyLimits) -> Vec<u8> {
    let proof = prove_shake_shared(
        relation,
        &columns,
        VerifyLimits {
            max_proof_bytes: 16 * 1024 * 1024,
            ..limits
        },
    )
    .unwrap();
    norito::encode_canonical(&proof).unwrap()
}

// Consumes every private path. Only public endpoints and proof-ready columns leave.
fn columns(
    fixture: &QuantityFixture,
    private: DerivedTransferSmtWitnesses,
) -> (Vec<[u8; 32]>, Vec<Columns>) {
    let count = private.pairs().len();
    let roots: Vec<_> = private.pairs()[..count - 1]
        .iter()
        .map(|p| p[1].root_after)
        .collect();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let statements = prepared.compact_statements(&roots).unwrap();
    let columns = statements
        .iter()
        .zip(private.pairs())
        .map(|(statement, pair)| {
            for (update, witness) in statement.updates.iter().zip(pair) {
                assert_eq!(witness.path_bits, update.path.to_le_bytes());
                assert_eq!(witness.siblings.len(), PATH_LEVELS);
            }
            let siblings = core::array::from_fn(|update| {
                core::array::from_fn(|level| {
                    let bytes = pair[update].siblings[level];
                    core::array::from_fn(|limb| {
                        u32::from_le_bytes(bytes[limb * 4..limb * 4 + 4].try_into().unwrap())
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
        .collect();
    (roots, columns)
}

fn retain(label: &str, bytes: &[u8]) {
    let sha = format!("{:x}", Sha256::digest(bytes));
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("quantity-compact-{label}-{sha}.bin"));
    std::fs::write(&path, bytes).unwrap();
    eprintln!(
        "retained {} bytes sha256={sha} at {}",
        bytes.len(),
        path.display()
    );
}

/// Keep the exact statement and equations, changing only the claimed identity.
struct Retagged<'a, R> {
    relation: &'a R,
    identity: &'static str,
}

impl<R: FixedAir> FixedAir for Retagged<'_, R> {
    fn schema(&self) -> FixedAirSchema {
        FixedAirSchema {
            identity: self.identity,
            ..self.relation.schema()
        }
    }
    fn statement_bytes(&self) -> &[u8] {
        self.relation.statement_bytes()
    }
    fn evaluate(&self, point: u64, current: &[u64], next: &[u64]) -> crate::Result<Vec<u64>> {
        self.relation.evaluate(point, current, next)
    }
    fn prepare_prover(&self) -> crate::Result<Box<dyn PreparedAir + '_>> {
        panic!("bounded quantity verifier attempted prover preparation")
    }
}

fn reject_retag(
    relation: &impl FixedAir,
    identity: &'static str,
    bytes: &[u8],
    limits: VerifyLimits,
) {
    let retagged = Retagged { relation, identity };
    assert!(
        SharedVerifier::ShakeCandidate {
            max_decode_allocation_charges: 80 * 1024 * 1024
        }
        .verify_frame(&retagged, bytes, limits)
        .is_err()
    );
}

fn single(axt: bool) {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (mut fixture, private) = QuantityFixture::new(
        if axt {
            QuantityCase::Maximum
        } else {
            QuantityCase::MixedScale
        },
        1,
    );
    let (roots, mut private_columns) = columns(&fixture, private);
    assert!(roots.is_empty());
    let limits = policy(1).segment;
    let started = std::time::Instant::now();
    let bytes = {
        let expected = fixture.expected();
        if axt {
            let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
            let relation = AxtTransferAir::new(
                &prepared,
                &expected,
                &fixture.axt.binding,
                fixture.axt.metadata(),
                fixture.axt.outer,
                fixture.axt.remote.as_deref(),
            )
            .unwrap();
            prove(&relation, private_columns.pop().unwrap(), limits)
        } else {
            let prepared = fixture.prepare(ProofSemantics::StateTransition);
            prove(
                &PublicTransferAir::new(&prepared, &expected).unwrap(),
                private_columns.pop().unwrap(),
                limits,
            )
        }
    };
    drop(private_columns);
    eprintln!("quantity single axt={axt} proving {:?}", started.elapsed());
    let verify = |f: &QuantityFixture, bytes: &[u8], cap: usize| {
        let expected = f.expected();
        if axt {
            verify_shake_axt_transfer(
                &f.prepare(ProofSemantics::AxtTransferClaim),
                &expected,
                f.context(),
                bytes,
                limits,
                cap,
            )
        } else {
            verify_shake_transfer(
                &f.prepare(ProofSemantics::StateTransition),
                &expected,
                bytes,
                limits,
                cap,
            )
        }
    };
    let started = std::time::Instant::now();
    let (result, usage) = norito::core::with_decode_limits_measured(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 80 * 1024 * 1024, 16),
        || verify(&fixture, &bytes, 80 * 1024 * 1024),
    );
    let verified = result.unwrap();
    assert_eq!(verified.public_io(), fixture.expected());
    assert_eq!(verified.work().air_evaluations, 375);
    assert_eq!(verified.work().terminal_degree_checks, 1);
    assert_eq!(verified.work().transcripts, 1);
    assert_eq!(verified.work().proof_bytes, bytes.len());
    eprintln!(
        "quantity single axt={axt} bounded verification {:?}; decode charges {}; work {:?}",
        started.elapsed(),
        usage.total_allocated_bytes(),
        verified.work()
    );
    // Matching equations and statement bytes cannot substitute a narrow identity.
    if axt {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let relation = AxtTransferAir::new(
            &prepared,
            &fixture.expected(),
            &fixture.axt.binding,
            fixture.axt.metadata(),
            fixture.axt.outer,
            fixture.axt.remote.as_deref(),
        )
        .unwrap();
        reject_retag(&relation, u64::AXT_IDENTITY, &bytes, limits);
    } else {
        let prepared = fixture.prepare(ProofSemantics::StateTransition);
        let relation = PublicTransferAir::new(&prepared, &fixture.expected()).unwrap();
        reject_retag(&relation, u64::TRANSFER_IDENTITY, &bytes, limits);
    }
    // Coherent alternate full-domain statements, including changed high limbs
    // and decimal scales, must not reuse this proof even with recomputed roots.
    for case in [QuantityCase::U128, QuantityCase::Tiny] {
        let (changed, private) = QuantityFixture::new(case, 1);
        drop(private);
        assert!(verify(&changed, &bytes, 80 * 1024 * 1024).is_err());
    }
    assert!(verify(&fixture, &bytes, 1).is_err());
    assert!(verify(&fixture, &bytes[..bytes.len() - 1], 80 * 1024 * 1024).is_err());
    fixture.claims[0].authority_digest = Hash::new(b"different proof-bound quantity authority");
    assert!(verify(&fixture, &bytes, 80 * 1024 * 1024).is_err());
    retain(if axt { "axt-single" } else { "ordinary-single" }, &bytes);
}

#[test]
#[ignore = "explicit full-domain ordinary candidate proof with 605-bit common-scale values"]
fn full_domain_ordinary_single_verifies_after_private_data_is_dropped() {
    single(false);
}

#[test]
#[ignore = "explicit full-domain AXT candidate proof with maximum 511-bit remote spend"]
fn full_domain_axt_single_verifies_after_private_data_is_dropped() {
    single(true);
}

fn bundle(axt: bool) {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (mut fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 2);
    let (roots, private_columns) = columns(&fixture, private);
    let limits = policy(2);
    let started = std::time::Instant::now();
    let bytes = {
        let expected = fixture.expected();
        let contexts = BatchContextLimits {
            max_segments: 2,
            max_total_statement_bytes: limits.max_total_statement_bytes,
        };
        if axt {
            let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
            let batch =
                AxtTransferBatch::new(&prepared, &expected, &roots, fixture.context(), contexts)
                    .unwrap();
            let segments = private_columns
                .into_iter()
                .enumerate()
                .map(|(i, columns)| prove(&batch.segment(i).unwrap(), columns, limits.segment))
                .collect();
            compact_bundle::encode_shake_axt_wire(
                &ShakeAxtBundleWire {
                    version: 1,
                    intermediate_roots: roots,
                    segments,
                },
                2,
                limits,
            )
            .unwrap()
        } else {
            let prepared = fixture.prepare(ProofSemantics::StateTransition);
            let batch = PublicTransferBatch::new(&prepared, &expected, &roots, contexts).unwrap();
            let segments = private_columns
                .into_iter()
                .enumerate()
                .map(|(i, columns)| prove(&batch.segment(i).unwrap(), columns, limits.segment))
                .collect();
            compact_bundle::encode_shake_wire(
                &ShakeBundleWire {
                    version: 1,
                    intermediate_roots: roots,
                    segments,
                },
                2,
                limits,
            )
            .unwrap()
        }
    };
    eprintln!("quantity bundle axt={axt} proving {:?}", started.elapsed());
    let verify = |f: &QuantityFixture, bytes: &[u8]| {
        if axt {
            compact_bundle::verify_shake_axt_transfer_bundle(
                &f.prepare(ProofSemantics::AxtTransferClaim),
                &f.expected(),
                f.context(),
                bytes,
                limits,
                80 * 1024 * 1024,
            )
        } else {
            compact_bundle::verify_shake_transfer_bundle(
                &f.prepare(ProofSemantics::StateTransition),
                &f.expected(),
                bytes,
                limits,
                80 * 1024 * 1024,
            )
        }
    };
    let started = std::time::Instant::now();
    let result = verify(&fixture, &bytes).unwrap();
    assert_eq!(result.public_io(), fixture.expected());
    assert_eq!(result.work().air_evaluations, 750);
    assert_eq!(result.work().terminal_degree_checks, 2);
    assert_eq!(result.segments(), 2);
    eprintln!(
        "quantity bundle axt={axt} bounded verification {:?}; work {:?}",
        started.elapsed(),
        result.work()
    );
    fixture.claims[1].authority_digest = Hash::new(b"different second occurrence authority");
    assert!(verify(&fixture, &bytes).is_err());
    retain(if axt { "axt-bundle" } else { "ordinary-bundle" }, &bytes);
}

#[test]
#[ignore = "explicit two-segment full-domain ordinary candidate bundle"]
fn full_domain_ordinary_bundle_verifies_every_segment_without_private_data() {
    bundle(false);
}

#[test]
#[ignore = "explicit two-segment full-domain AXT candidate bundle"]
fn full_domain_axt_bundle_verifies_every_segment_without_private_data() {
    bundle(true);
}
