//! Explicit full-domain proof diagnostics, excluded from routine unit test runs.
//!
//! TODO: Qualify these test-only candidates and integrate authenticated source
//! expectations before production use. Mathematical verification is not finality.

use super::{
    compact_axt_batch::AxtTransferBatch,
    compact_bundle::BundleLimits,
    compact_protocol::{FixedAir, FixedAirSchema, PreparedAir},
    compact_public_api::{
        DeepVerifier, verify_axt_transfer_with_allocation, verify_transfer_with_allocation,
    },
    compact_public_batch::{BatchContextLimits, PublicTransferBatch},
    compact_quantity_tests::{QuantityCase, QuantityFixture},
    compact_value_domain::CompactTransferValue,
    deep_relation::DeepRelation,
    deep_trace_source::OwnedTraceSource,
};
use crate::{
    Error, ProofSemantics, VerifyLimits,
    gadgets::{
        compact_smt_air::{PATH_LEVELS, SmtWitness},
        public_transfer_statement::DerivedTransferSmtWitnesses,
    },
};
use iroha_crypto::Hash;
use sha2::{Digest as _, Sha256};

type Columns = OwnedTraceSource;

fn policy(count: usize) -> BundleLimits {
    BundleLimits {
        max_segments: count,
        max_wire_bytes: 16 * 1024 * 1024,
        max_total_segment_bytes: 16 * 1024 * 1024,
        max_total_statement_bytes: 512 * 1024,
        max_total_queries: count * super::deep_geometry::QUERY_COUNT,
        max_total_decode_allocation_charges: 192 * 1024 * 1024,
        segment: VerifyLimits {
            max_proof_bytes: super::deep_proof::MAX_FRAME_BYTES,
            max_queries: super::deep_geometry::QUERY_COUNT,
            ..VerifyLimits::default()
        },
    }
}

fn prove(relation: &impl DeepRelation, source: OwnedTraceSource, seed: u64) -> Vec<u8> {
    super::deep_fixture::prove(relation, source, seed).unwrap()
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
            OwnedTraceSource::from_rows(witness.rows()).unwrap()
        })
        .collect();
    drop(private);
    (roots, columns)
}

/// Public native output awaiting an independently reviewed, immutable fixture pin.
#[derive(norito::derive::JsonSerialize, norito::derive::JsonDeserialize)]
struct FixturePin {
    bytes: usize,
    sha256: String,
    schema: String,
    row_root: String,
    // Every public work counter is pinned, including raw unused tape bytes.
    work: Vec<usize>,
}

fn counters(w: super::deep_engine::VerificationWork) -> Vec<usize> {
    vec![
        w.proof_bytes,
        w.air_evaluations,
        w.leaf_hashes,
        w.parent_hashes,
        w.h_calls,
        w.verifier_messages,
        w.g_tape_bytes,
        w.fold_checks,
        w.terminal_values,
    ]
}

fn retain(label: &str, bytes: &[u8], work: super::deep_engine::VerificationWork) {
    let proof = super::deep_proof::decode(bytes, super::deep_proof::MAX_FRAME_BYTES).unwrap();
    let pin = FixturePin {
        bytes: bytes.len(),
        sha256: hex::encode(Sha256::digest(bytes)),
        schema: hex::encode(&bytes[6..22]),
        row_root: hex::encode(proof.row_root.as_bytes()),
        work: counters(work),
    };
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fastpq-production-validation");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("quantity-q77-{label}-{}.bin", pin.sha256));
    std::fs::write(&path, bytes).unwrap();
    // This is an observed output, never automatically promoted to the canonical pin.
    std::fs::write(
        directory.join(format!("quantity-q77-{label}-{}.observed.json", pin.sha256)),
        norito::json::to_vec(&pin).unwrap(),
    )
    .unwrap();
    eprintln!(
        "retained {} bytes sha256={} at {}; canonical fixture pin review pending",
        bytes.len(),
        pin.sha256,
        path.display()
    );
}

/// Keep the exact statement and equations, changing only the claimed identity.
struct Retagged<'a, R> {
    relation: &'a R,
    identity: &'static str,
}

impl<R: DeepRelation> FixedAir for Retagged<'_, R> {
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

impl<R: DeepRelation> super::deep_relation::sealed::Sealed for Retagged<'_, R> {}
impl<R: DeepRelation> DeepRelation for Retagged<'_, R> {
    fn deep_relation(&self) -> &super::compact_transfer_air::CompactTransferAir {
        self.relation.deep_relation()
    }
}

fn reject_retag(
    relation: &impl DeepRelation,
    identity: &'static str,
    bytes: &[u8],
    limits: VerifyLimits,
) {
    let retagged = Retagged { relation, identity };
    assert!(
        DeepVerifier {
            max_decode_allocation_charges: 80 * 1024 * 1024
        }
        .verify_frame(&retagged, bytes, limits)
        .is_err()
    );
}

/// Matching equations and statement bytes cannot substitute a narrow identity.
fn reject_single_retag(fixture: &QuantityFixture, axt: bool, bytes: &[u8], limits: VerifyLimits) {
    if axt {
        let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
        let expected = fixture.expected();
        let batch = AxtTransferBatch::new(
            &prepared,
            &expected,
            &[],
            fixture.context(),
            BatchContextLimits::default(),
        )
        .unwrap();
        let relation = batch.segment(0).unwrap();
        reject_retag(&relation, u64::AXT_IDENTITY, bytes, limits);
    } else {
        let prepared = fixture.prepare(ProofSemantics::StateTransition);
        let expected = fixture.expected();
        let batch =
            PublicTransferBatch::new(&prepared, &expected, &[], BatchContextLimits::default())
                .unwrap();
        let relation = batch.segment(0).unwrap();
        reject_retag(&relation, u64::TRANSFER_IDENTITY, bytes, limits);
    }
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
            let batch = AxtTransferBatch::new(
                &prepared,
                &expected,
                &[],
                fixture.context(),
                BatchContextLimits::default(),
            )
            .unwrap();
            prove(
                &batch.segment(0).unwrap(),
                private_columns.pop().unwrap(),
                0x077_511,
            )
        } else {
            let prepared = fixture.prepare(ProofSemantics::StateTransition);
            let batch =
                PublicTransferBatch::new(&prepared, &expected, &[], BatchContextLimits::default())
                    .unwrap();
            prove(
                &batch.segment(0).unwrap(),
                private_columns.pop().unwrap(),
                0x077_605,
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
    assert_eq!(verified.work().air_evaluations, 1);
    assert_eq!(verified.work().terminal_values, 128);
    assert_eq!(verified.work().verifier_messages, 10);
    assert_eq!(verified.work().proof_bytes, bytes.len());
    eprintln!(
        "quantity single axt={axt} bounded verification {:?}; decode charges {}; work {:?}",
        started.elapsed(),
        usage.total_allocated_bytes(),
        verified.work()
    );
    reject_single_retag(&fixture, axt, &bytes, limits);
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
    retain(
        if axt { "axt-single" } else { "ordinary-single" },
        &bytes,
        verified.work(),
    );
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

// Native generation must precede a separately reviewed canonical pin. The
// producer writes observations only; these consumers never bless their own output.
// Pins come from reviewed source/binary-bound native generation. A separately
// rebuilt consumer must pass every retained-wire check before qualification.
fn read_retained_quantity_wire(label: &str) -> (Vec<u8>, FixturePin) {
    use std::io::Read as _;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let pin: FixturePin = norito::json::from_slice(
        &std::fs::read(root.join(format!("fixtures/fastpq/q77-quantity-{label}.json")))
            .expect("generate and independently review the current native fixture pin first"),
    )
    .unwrap();
    assert!(pin.bytes > 40 && pin.bytes <= super::deep_proof::MAX_FRAME_BYTES);
    assert_eq!(pin.sha256.len(), 64);
    assert_eq!(pin.row_root.len(), 64);
    assert_eq!(pin.schema.len(), 32);
    assert_eq!(pin.work.len(), 9);
    let file = std::fs::File::open(
        root.join("target/fastpq-production-validation")
            .join(format!("quantity-q77-{label}-{}.bin", pin.sha256)),
    )
    .expect("retain the exact current native proof named by the reviewed pin");
    let mut bytes = Vec::with_capacity(pin.bytes + 1);
    file.take((pin.bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), pin.bytes);
    assert_eq!(hex::encode(Sha256::digest(&bytes)), pin.sha256);
    assert_eq!(&bytes[..6], b"NRT0\0\0");
    assert_eq!(bytes[22], 0);
    assert_eq!(bytes[39], norito::core::header_flags::COMPACT_LEN);
    assert_eq!(hex::encode(&bytes[6..22]), pin.schema);
    assert_eq!(
        u64::from_le_bytes(bytes[23..31].try_into().unwrap()),
        (pin.bytes - 40) as u64
    );
    let proof = super::deep_proof::decode(&bytes, super::deep_proof::MAX_FRAME_BYTES).unwrap();
    assert_eq!(norito::encode_canonical(&proof).unwrap(), bytes);
    (bytes, pin)
}

fn assert_retained_single(relation: &impl DeepRelation, bytes: &[u8], pin: &FixturePin) {
    let limits = policy(1).segment;
    let verifier = DeepVerifier {
        max_decode_allocation_charges: 32 * 1024 * 1024,
    };
    let (result, usage) = norito::core::with_decode_limits_measured(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 32 * 1024 * 1024, 16),
        || verifier.verify_frame_committed(relation, bytes, limits),
    );
    let result = result.unwrap();
    assert_eq!(hex::encode(result.row_root().as_bytes()), pin.row_root);
    assert_eq!(counters(result.work()), pin.work);
    assert_eq!(result.work().proof_bytes, bytes.len());
    assert_eq!(result.work().verifier_messages, 10);
    assert_eq!(result.work().air_evaluations, 1);
    assert_eq!(result.work().terminal_values, 128);
    let charges = usage.total_allocated_bytes();
    assert!(charges > 0 && charges <= 32 * 1024 * 1024);
    for (cap, accepted) in [(charges, true), (charges - 1, false)] {
        let exact = DeepVerifier {
            max_decode_allocation_charges: cap,
        };
        assert_eq!(
            exact
                .verify_frame_committed(relation, bytes, limits)
                .is_ok(),
            accepted
        );
    }
    // Current compact proofs meet the unchanged ordinary and AXT byte ceilings.
    for max in [
        VerifyLimits::default().max_proof_bytes,
        512 * 1024,
        1024 * 1024,
    ] {
        assert_eq!(
            verifier
                .verify_frame_committed(
                    relation,
                    bytes,
                    VerifyLimits {
                        max_proof_bytes: max,
                        ..limits
                    }
                )
                .unwrap(),
            result
        );
    }
    assert!(matches!(verifier.verify_frame_committed(relation, bytes,
        VerifyLimits { max_proof_bytes: bytes.len() - 1, ..limits }),
        Err(Error::VerifierLimitExceeded { limit: "max_proof_bytes", actual, max })
            if actual == bytes.len() && max == bytes.len() - 1));
}

#[test]
#[ignore = "requires the independently pinned current q77 ordinary quantity proof"]
fn retained_ordinary_single_preserves_full_wire_root_and_quantity_context() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (bytes, pin) = read_retained_quantity_wire("ordinary-single");
    let (fixture, private) = QuantityFixture::new(QuantityCase::MixedScale, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::StateTransition);
    let batch =
        PublicTransferBatch::new(&prepared, &expected, &[], BatchContextLimits::default()).unwrap();
    assert_retained_single(&batch.segment(0).unwrap(), &bytes, &pin);
    reject_single_retag(&fixture, false, &bytes, policy(1).segment);
}

#[test]
#[ignore = "requires the independently pinned current q77 AXT quantity proof"]
fn retained_axt_single_preserves_full_wire_root_and_quantity_context() {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (bytes, pin) = read_retained_quantity_wire("axt-single");
    let (fixture, private) = QuantityFixture::new(QuantityCase::Maximum, 1);
    drop(private);
    let expected = fixture.expected();
    let prepared = fixture.prepare(ProofSemantics::AxtTransferClaim);
    let batch = AxtTransferBatch::new(
        &prepared,
        &expected,
        &[],
        fixture.context(),
        BatchContextLimits::default(),
    )
    .unwrap();
    assert_retained_single(&batch.segment(0).unwrap(), &bytes, &pin);
    reject_single_retag(&fixture, true, &bytes, policy(1).segment);
}
