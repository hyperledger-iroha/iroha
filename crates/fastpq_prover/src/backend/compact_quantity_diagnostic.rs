//! Explicit full-domain proof diagnostics, excluded from routine unit test runs.
//!
//! TODO: Qualify these test-only candidates and integrate authenticated source
//! expectations before production use. Mathematical verification is not finality.

use super::{
    compact_axt_air::AxtTransferAir,
    compact_axt_batch::AxtTransferBatch,
    compact_bundle::{self, AxtBundleWire, BundleLimits, BundleWire},
    compact_protocol::{FixedAir, FixedAirSchema, PreparedAir, shared_openings::prove_shared},
    compact_public_api::{
        SharedVerifier, verify_axt_transfer_with_allocation, verify_transfer_with_allocation,
    },
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
    compact_public_transfer::PublicTransferAir,
    compact_quantity_tests::{QuantityCase, QuantityFixture},
    compact_value_domain::CompactTransferValue,
};
use crate::{
    Error, ProofSemantics, VerifyLimits,
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
    let proof = prove_shared(
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
        SharedVerifier {
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
            verify_axt_transfer_with_allocation(
                &f.prepare(ProofSemantics::AxtTransferClaim),
                &expected,
                f.context(),
                bytes,
                limits,
                cap,
            )
        } else {
            verify_transfer_with_allocation(
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
            compact_bundle::encode_axt_wire(
                &AxtBundleWire {
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
            compact_bundle::encode_wire(
                &BundleWire {
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
            compact_bundle::verify_axt_transfer_bundle_with_allocation(
                &f.prepare(ProofSemantics::AxtTransferClaim),
                &f.expected(),
                f.context(),
                bytes,
                limits,
                80 * 1024 * 1024,
            )
        } else {
            compact_bundle::verify_transfer_bundle_with_allocation(
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

// These ignored regressions consume the exact content-addressed output of the
// diagnostics above. Their explicit larger test budgets never change admission.
fn read_retained_quantity_wire(
    label: &str,
    expected_bytes: usize,
    expected_sha256: &str,
    expected_schema: &str,
) -> Vec<u8> {
    use std::io::Read as _;

    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    let file = std::fs::File::open(
        directory.join(format!("quantity-compact-{label}-{expected_sha256}.bin")),
    )
    .expect("run the matching full-domain diagnostic and retain its exact proof");
    let mut bytes = Vec::with_capacity(expected_bytes + 1);
    file.take((expected_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), expected_bytes);
    assert_eq!(hex::encode(Sha256::digest(&bytes)), expected_sha256);
    assert_eq!(&bytes[..6], b"NRT0\0\0");
    assert_eq!(bytes[22], 0);
    assert_eq!(bytes[39], norito::core::header_flags::COMPACT_LEN);
    assert_eq!(hex::encode(&bytes[6..22]), expected_schema);
    assert_eq!(
        u64::from_le_bytes(bytes[23..31].try_into().unwrap()),
        (expected_bytes - 40) as u64
    );
    bytes
}

fn assert_retained_single(
    relation: &impl FixedAir,
    bytes: &[u8],
    expected_root: &str,
    expected_counts: [usize; 4],
) {
    let limits = policy(1).segment;
    let result = SharedVerifier {
        max_decode_allocation_charges: 80 * 1024 * 1024,
    }
    .verify_frame_committed(relation, bytes, limits)
    .unwrap();
    assert_eq!(hex::encode(result.row_root().to_le_bytes()), expected_root);
    let work = result.work();
    assert_eq!(work.proof_bytes, bytes.len());
    assert_eq!(work.transcripts, 1);
    assert_eq!(work.air_evaluations, 375);
    assert_eq!(
        [
            work.row_leaves,
            work.oracle_leaves,
            work.fri_leaves,
            work.parent_hashes
        ],
        expected_counts
    );
    assert_eq!(work.terminal_degree_checks, 1);
    assert!(matches!(
        SharedVerifier {
            max_decode_allocation_charges: 32 * 1024 * 1024,
        }
        .verify_frame_committed(relation, bytes, limits),
        Err(Error::Encode(norito::Error::TotalAllocationExceeded { attempted, limit }))
            if limit == 32 * 1024 * 1024 && attempted > limit
    ));
    // Check the replay, compact target and AXT byte policies separately from allocation.
    for max in [
        VerifyLimits::default().max_proof_bytes,
        512 * 1024,
        1024 * 1024,
    ] {
        assert!(matches!(
            SharedVerifier {
                max_decode_allocation_charges: 80 * 1024 * 1024,
            }
            .verify_frame_committed(relation, bytes, VerifyLimits {
                max_proof_bytes: max,
                max_queries: 375,
                ..VerifyLimits::default()
            }),
            Err(Error::VerifierLimitExceeded { limit: "max_proof_bytes", actual, max: observed })
                if actual == bytes.len() && observed == max
        ));
    }
}

#[test]
#[ignore = "requires the exact retained six-lane ordinary-single quantity proof"]
fn retained_ordinary_single_preserves_full_wire_root_and_quantity_context() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = read_retained_quantity_wire(
        "ordinary-single",
        3994619,
        "8ed0b0db090e7342ae7b7dc1fb9a4e5f2ad265c5f5f4cfe2808cb066385f9091",
        "626ab2f2e794c043f1d57dec4d05650a",
    );
    // The complete single relation differs from even a count-one bundle.
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let relation = PublicTransferAir::new(&prepared, &expected).unwrap();
    assert_retained_single(
        &relation,
        &bytes,
        "aed561b2f726e3a9776d9855e18503c3c8965ecba1746c9c424a5a3396dd2a77f9fc8d797855096915fd679aa0002e1b",
        [749, 750, 3960, 33169],
    );
}

#[test]
#[ignore = "requires the exact retained six-lane axt-single quantity proof"]
fn retained_axt_single_preserves_full_wire_root_and_quantity_context() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = read_retained_quantity_wire(
        "axt-single",
        4015551,
        "bba32fd6bdf97bd5b349a789a60cc24645f4594c2bde79ea1b42138b21cc0189",
        "626ab2f2e794c043f1d57dec4d05650a",
    );
    // The complete single relation differs from even a count-one bundle.
    let (fixture, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let expected = fixture.expected();
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
    assert_retained_single(
        &relation,
        &bytes,
        "dbabc4745d74f63ac4a8a03236b68e909ef26a4db1d91279af33cfb3f690fed41d9fa5e1ec1d6dd9d89be9c3b5edef37",
        [750, 750, 3956, 33536],
    );
}

#[test]
#[ignore = "requires the exact retained six-lane ordinary-bundle quantity proof"]
fn retained_ordinary_bundle_preserves_full_wire_root_and_quantity_context() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = read_retained_quantity_wire(
        "ordinary-bundle",
        7986384,
        "e22530aabc8b71c7a76e97f70e74fa34e8eb8ea992b2ad323d07e57148edba51",
        "b503258578379edf3e8fe8779ed1dc01",
    );
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 2);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let verify = |limits, allocation| {
        compact_bundle::verify_transfer_bundle_with_allocation(
            &prepared, &expected, &bytes, limits, allocation,
        )
    };
    let result = verify(policy(2), 80 * 1024 * 1024).unwrap();
    assert_eq!(result.public_io(), expected);
    assert_eq!(result.segments(), 2);
    assert_eq!(result.wire_bytes(), 7986384);
    assert_eq!(
        result
            .row_roots()
            .iter()
            .map(|root| hex::encode(root.to_le_bytes()))
            .collect::<Vec<_>>(),
        [
            "1bf048409596047f4a3e8502113c05c677fbede1c7a4ed91ab5e1257f3959b0c1596cd6b2d90509bf30b7a67ba33512e",
            "25395796735933aad6091e2882da3345bb95aad2a008e4a041576d524a81376396ec861f73e71064eee5c58bf06c7435",
        ]
    );
    let work = result.work();
    assert_eq!(work.proof_bytes, 7986231);
    assert_eq!(work.transcripts, 2);
    assert_eq!(work.row_leaves, 1499);
    assert_eq!(work.oracle_leaves, 1500);
    assert_eq!(work.fri_leaves, 7923);
    assert_eq!(work.parent_hashes, 66213);
    assert_eq!(work.air_evaluations, 750);
    assert_eq!(work.terminal_degree_checks, 2);
    assert!(matches!(verify(policy(2), 32 * 1024 * 1024),
        Err(Error::Encode(norito::Error::TotalAllocationExceeded { attempted, limit }))
            if limit == 32 * 1024 * 1024 && attempted > limit
    ));
    for max in [512 * 1024, 1024 * 1024] {
        assert!(
            matches!(verify(BundleLimits { max_wire_bytes: max, ..policy(2) }, 80 * 1024 * 1024),
                Err(Error::VerifierLimitExceeded { limit: "max_bundle_wire_bytes", actual, max: observed })
                    if actual == bytes.len() && observed == max
            )
        );
    }
}

#[test]
#[ignore = "requires the exact retained six-lane axt-bundle quantity proof"]
fn retained_axt_bundle_preserves_full_wire_root_and_quantity_context() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let bytes = read_retained_quantity_wire(
        "axt-bundle",
        8011999,
        "66f9676041c729250cd1d1f0226ab14536e0f2f1615293b67bde634a126386aa",
        "2248a1a6445362f5b2336e17b0462752",
    );
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 2);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let verify = |limits, allocation| {
        compact_bundle::verify_axt_transfer_bundle_with_allocation(
            &prepared,
            &expected,
            fixture.context(),
            &bytes,
            limits,
            allocation,
        )
    };
    let result = verify(policy(2), 80 * 1024 * 1024).unwrap();
    assert_eq!(result.public_io(), expected);
    assert_eq!(result.segments(), 2);
    assert_eq!(result.wire_bytes(), 8011999);
    assert_eq!(
        result
            .row_roots()
            .iter()
            .map(|root| hex::encode(root.to_le_bytes()))
            .collect::<Vec<_>>(),
        [
            "4373fb588622baf1b9874f57f41499cf61fa527b9f6ee42a7129a4d895c6cb6644da52f9b83f8ccfcd8eb9d912563a90",
            "c208d9c934f3e3f085939343e42bf7e8913584cf3b76e06364925af5fcd19e57c0b798a63d54124cba4cdcd06fc755b5",
        ]
    );
    let work = result.work();
    assert_eq!(work.proof_bytes, 8011846);
    assert_eq!(work.transcripts, 2);
    assert_eq!(work.row_leaves, 1499);
    assert_eq!(work.oracle_leaves, 1500);
    assert_eq!(work.fri_leaves, 7947);
    assert_eq!(work.parent_hashes, 66724);
    assert_eq!(work.air_evaluations, 750);
    assert_eq!(work.terminal_degree_checks, 2);
    assert!(matches!(verify(policy(2), 32 * 1024 * 1024),
        Err(Error::Encode(norito::Error::TotalAllocationExceeded { attempted, limit }))
            if limit == 32 * 1024 * 1024 && attempted > limit
    ));
    for max in [512 * 1024, 1024 * 1024] {
        assert!(
            matches!(verify(BundleLimits { max_wire_bytes: max, ..policy(2) }, 80 * 1024 * 1024),
                Err(Error::VerifierLimitExceeded { limit: "max_bundle_wire_bytes", actual, max: observed })
                    if actual == bytes.len() && observed == max
            )
        );
    }
}
