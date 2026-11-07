//! Advice columns committed elsewhere (relaxed-relation deciders): the proof
//! carries exactly the caller's commitment over all `n` given rows and
//! verifies unchanged, the given rows are used verbatim, the shape is checked,
//! and a witness without such columns proves byte for byte as before.

use ff::Field;
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaCurve, poseidon::PoseidonField};

use super::*;
use crate::{
    pcs::ipa::commit::{Secrecy, commit_lagrange},
    protocol::Shape,
    test_circuits::{Arithmetic, BUDGET, CHOICES, setup},
    transcript::{MESSAGE_BYTES, encode_point},
};

const ARITHMETIC: Arithmetic = Arithmetic {
    start: 3,
    rows: 12,
    tamper: None,
};

/// The `index`-th 32-byte message of a proof.
fn message(proof: &[u8], index: usize) -> &[u8] {
    &proof[index * MESSAGE_BYTES..(index + 1) * MESSAGE_BYTES]
}

/// The protocol shape of `pk`.
fn shape_of<C: PastaCurve>(pk: &ProvingKey<C>) -> Shape {
    *Protocol::new(pk.binding().descriptor())
        .expect("protocol")
        .shape()
}

/// Without committed columns, and with an all-absent committed list, every
/// proof byte equals the ordinary owned prover's under every option.
fn ordinary_bytes<C>(instances: &[Vec<C::ScalarExt>])
where
    C: PastaCurve,
    C::ScalarExt: PoseidonField,
    C::Base: PoseidonField,
{
    for choice in CHOICES {
        let setup = setup::<C, _>(&ARITHMETIC, choice);
        let randomness = || ProverRandomness::fixed_seed_for_tests([5; 32]);
        let witness = Witness::from_circuit(&setup.pk, &ARITHMETIC, instances).expect("witness");
        let advice = witness.advice().to_vec();
        let reference = create_proof_owned_with_claim(
            &setup.params,
            &setup.pk,
            witness,
            randomness(),
            ProverConfig::default(),
        )
        .expect("reference proof");
        let absent = vec![None; advice.len()];
        let output = prove_with_committed_advice(
            &setup.params,
            &setup.pk,
            advice,
            absent,
            instances.to_vec(),
            randomness(),
            ProverConfig::default(),
        )
        .expect("proof");
        assert_eq!(output.proof, reference.proof, "{choice:?}");
        assert_eq!(output.opening, reference.opening, "{choice:?}");
    }
}

#[test]
fn absent_committed_columns_keep_every_proof_byte() {
    ordinary_bytes::<Ep>(&ARITHMETIC.instances::<Fq>());
    ordinary_bytes::<Eq>(&ARITHMETIC.instances::<Fp>());
}

/// Commits the first and last advice columns elsewhere, with given values on
/// every blinding row: the proof's commitments to them are exactly
/// `commit_lagrange(values, blind)`, the proof verifies, and changing one
/// given blinding row changes that commitment and still verifies.
fn committed_columns<C>(instances: &[Vec<C::ScalarExt>])
where
    C: PastaCurve,
    C::ScalarExt: PoseidonField,
    C::Base: PoseidonField,
{
    for choice in CHOICES {
        let setup = setup::<C, _>(&ARITHMETIC, choice);
        let shape = shape_of(&setup.pk);
        let witness = Witness::from_circuit(&setup.pk, &ARITHMETIC, instances).expect("witness");
        let mut advice = witness.advice().to_vec();
        drop(witness);
        let last = advice.len() - 1;
        assert!(last > 0, "the circuit has several advice columns");
        for column in [0, last] {
            for (row, value) in advice[column]
                .iter_mut()
                .enumerate()
                .skip(shape.usable_rows)
            {
                *value = C::ScalarExt::from(1_000 + (column * 100 + row) as u64);
            }
        }
        let mut committed = vec![None; advice.len()];
        committed[0] = Some(C::ScalarExt::from(7));
        committed[last] = Some(C::ScalarExt::from(11));

        let expected = |advice: &[Vec<C::ScalarExt>], column: usize, blind: u64| {
            let point = commit_lagrange(
                setup.params.params(),
                &advice[column],
                &C::ScalarExt::from(blind),
                Secrecy::Secret,
                BUDGET,
            )
            .expect("commit")
            .to_affine();
            encode_point::<C>(&point)
        };
        let prove = |advice: Vec<Vec<C::ScalarExt>>, seed: u8| {
            prove_with_committed_advice(
                &setup.params,
                &setup.pk,
                advice,
                committed.clone(),
                instances.to_vec(),
                ProverRandomness::fixed_seed_for_tests([seed; 32]),
                ProverConfig::default(),
            )
            .expect("proof")
            .proof
        };

        let proof = prove(advice.clone(), 9);
        assert_eq!(setup.verify(instances, &proof), Ok(()), "{choice:?}");
        assert_eq!(message(&proof, 0), expected(&advice, 0, 7), "{choice:?}");
        assert_eq!(
            message(&proof, last),
            expected(&advice, last, 11),
            "{choice:?}"
        );

        // Another seed changes the ordinary columns' commitments only.
        let reseeded = prove(advice.clone(), 10);
        assert_eq!(setup.verify(instances, &reseeded), Ok(()), "{choice:?}");
        assert_eq!(message(&reseeded, 0), message(&proof, 0), "{choice:?}");
        assert_eq!(
            message(&reseeded, last),
            message(&proof, last),
            "{choice:?}"
        );
        for column in 1..last {
            assert_ne!(
                message(&reseeded, column),
                message(&proof, column),
                "{choice:?}"
            );
        }

        // The given blinding rows are used verbatim.
        let mut changed = advice.clone();
        changed[0][shape.n - 1] += C::ScalarExt::from(1);
        let proof = prove(changed.clone(), 9);
        assert_eq!(setup.verify(instances, &proof), Ok(()), "{choice:?}");
        assert_eq!(message(&proof, 0), expected(&changed, 0, 7), "{choice:?}");
        assert_ne!(message(&proof, 0), expected(&advice, 0, 7), "{choice:?}");
    }
}

#[test]
fn committed_columns_carry_the_callers_commitment_and_verify() {
    committed_columns::<Ep>(&ARITHMETIC.instances::<Fq>());
    committed_columns::<Eq>(&ARITHMETIC.instances::<Fp>());
}

#[test]
fn committed_blinds_need_one_entry_per_advice_column() {
    let setup = setup::<Ep, _>(&ARITHMETIC, CHOICES[0]);
    let instances = ARITHMETIC.instances::<Fq>();
    let witness = Witness::from_circuit(&setup.pk, &ARITHMETIC, &instances).expect("witness");
    let advice = witness.advice().to_vec();
    let columns = advice.len();
    for committed in [vec![Some(Fq::ONE)], vec![None; columns + 1]] {
        let found = committed.len();
        assert_eq!(
            Witness::from_columns_with_committed_advice(
                &setup.pk,
                advice.clone(),
                instances.clone(),
                committed,
            )
            .err(),
            Some(ProverError::WitnessShape {
                expected: columns,
                found,
            })
        );
    }
}
