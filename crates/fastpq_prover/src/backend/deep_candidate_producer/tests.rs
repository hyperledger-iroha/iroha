//! Whole-attempt budget/entropy controls and an explicit full-size native diagnostic.

use super::*;
use crate::gadgets::compact_smt_air::{PublicStatement, PublicUpdate};
use rand::{SeedableRng, TryRngCore, rngs::StdRng};

fn limits() -> ConstructionLimits {
    ConstructionLimits {
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
    assert!(plan.hash_calls > 4 * (2 * LDE_ROWS - 1));
    let exact = ConstructionLimits {
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
    assert!(plan.build(&[], &mut rng).is_err());
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
#[ignore = "explicit full 8M-row DEEP producer: over 69M typed hashes; run with measured resource budget"]
fn complete_native_masked_deep_producer_roundtrip_and_statement_rejection() {
    use crate::gadgets::{
        compact_smt_air::PhysicalSmtWitness, compact_trace_columns::smt_row_cells,
    };
    let siblings: [_; 32] = core::array::from_fn(|level| digest((level + 17) as u8));
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
    let statement = PublicStatement {
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
    };
    let witness = PhysicalSmtWitness::from_inputs(&statement, &[siblings, siblings]).unwrap();
    let mut columns: Vec<_> = (0..342)
        .map(|_| zeroize::Zeroizing::new(Vec::with_capacity(TRACE_ROWS)))
        .collect();
    for row in witness.rows() {
        for (column, value) in columns.iter_mut().zip(smt_row_cells(row)) {
            column.push(value);
        }
    }
    drop(witness);
    let air = CompactTransferAir::new(&statement, Some(b"native producer diagnostic")).unwrap();
    let mut rng = StdRng::from_seed([83; 32]);
    let borrowed = columns
        .iter()
        .map(|column| column.as_slice())
        .collect::<Vec<_>>();
    let plan = ProducerPlan::new(&air, limits()).unwrap();
    eprintln!(
        "complete DEEP attempt bound: payload={}, work={}, hashes={}",
        plan.payload_bytes, plan.work_units, plan.hash_calls
    );
    let proof = plan.build(&borrowed, &mut rng).unwrap();
    assert!(proof.len() <= deep_proof::MAX_FRAME_BYTES);
    assert_eq!(
        deep_engine::verify(&air, &proof, deep_proof::PROOF_BYTE_TARGET)
            .unwrap()
            .air_evaluations,
        1
    );
    assert!(deep_engine::verify(&air, &proof, proof.len() - 1).is_err());
    let other = CompactTransferAir::new(
        &statement,
        Some(b"different authoritative statement context"),
    )
    .unwrap();
    assert!(deep_engine::verify(&other, &proof, deep_proof::PROOF_BYTE_TARGET).is_err());
    let mut altered = proof;
    let last = altered.len() - 1;
    altered[last] ^= 1;
    assert!(deep_engine::verify(&air, &altered, deep_proof::PROOF_BYTE_TARGET).is_err());
}
