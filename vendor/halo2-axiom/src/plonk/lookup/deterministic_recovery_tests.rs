//! Lookup grouping and seeded proof recovery must not depend on hash-table randomness.

use super::*;
use crate::{
    SerdeCurveAffine, SerdeFormat, SerdePrimeField,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Instance, Selector, TableColumn, create_proof,
        create_proof_consuming, keygen_pk2, verify_proof,
    },
    poly::{
        VerificationStrategy as _,
        commitment::ParamsProver,
        ipa::{
            commitment::{IPACommitmentScheme, ParamsIPA},
            multiopen::{ProverIPA, VerifierIPA},
            strategy::SingleStrategy,
        },
    },
    transcript::{
        Blake2bRead, Blake2bWrite, Challenge255, TranscriptReadBuffer, TranscriptWriterBuffer,
    },
};
use ff::FromUniformBytes;
use halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

#[test]
fn lookup_groups_are_canonical_across_hash_seeds_and_insertion_orders() {
    fn check<F: PrimeField + Ord>() {
        let entries = [(11, 4), (2, 9), (7, 3), (1, 2), (13, 7), (5, 1)];
        let mut expected = entries.map(|(value, count)| (F::from(value), count));
        expected.sort_by_key(|(value, _)| *value);
        let mut next = 0;
        let expected = expected
            .map(|(value, count)| {
                let range = next..next + count;
                next += count;
                (value, range)
            })
            .to_vec();
        for iteration in 0..32 {
            // Each new map has independent RandomState entropy. Vary allocation,
            // insertion and replacement history while preserving the final counts.
            let mut counts = HashMap::with_capacity(iteration * 3 + 1);
            for index in 0..entries.len() {
                let (value, count) = entries[(index + iteration) % entries.len()];
                counts.insert(ScalarKey(F::from(value)), count + 1);
            }
            for &(value, count) in entries.iter().rev() {
                counts.insert(ScalarKey(F::from(value)), count);
            }
            assert_eq!(canonical_input_unique_ranges(&counts), expected);
        }
    }
    check::<Fp>();
    check::<Fq>();
}

#[test]
fn lookup_group_ranges_cover_empty_and_single_value_inputs() {
    let empty = HashMap::<ScalarKey<Fp>, usize>::new();
    assert!(canonical_input_unique_ranges(&empty).is_empty());
    let single = HashMap::from([(ScalarKey(Fp::from(7)), 53)]);
    assert_eq!(
        canonical_input_unique_ranges(&single),
        vec![(Fp::from(7), 0..53)]
    );
}

#[derive(Clone)]
struct RecoveryLookupCircuit<F: PrimeField> {
    offset: Value<F>,
}

#[derive(Clone, Copy)]
struct RecoveryLookupConfig {
    advice: Column<Advice>,
    copied: Column<Advice>,
    instance: Column<Instance>,
    selector: Selector,
    table: TableColumn,
}

impl<F: PrimeField> Circuit<F> for RecoveryLookupCircuit<F> {
    type Config = RecoveryLookupConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            offset: Value::unknown(),
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let copied = meta.advice_column();
        let instance = meta.instance_column();
        let selector = meta.complex_selector();
        let table = meta.lookup_table_column();
        meta.enable_equality(advice);
        meta.enable_equality(copied);
        meta.enable_equality(instance);
        meta.set_minimum_degree(7);
        meta.create_gate("copied lookup witness", |meta| {
            vec![
                meta.query_selector(selector)
                    * (meta.query_advice(advice, Rotation::cur())
                        - meta.query_advice(copied, Rotation::cur())),
            ]
        });
        meta.lookup("varied seeded recovery lookup", |meta| {
            vec![(
                meta.query_selector(selector) * meta.query_advice(advice, Rotation::cur()),
                table,
            )]
        });
        RecoveryLookupConfig {
            advice,
            copied,
            instance,
            selector,
            table,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_table(
            || "recovery lookup range",
            |mut table| {
                for row in 0..16 {
                    table.assign_cell(
                        || "range entry",
                        config.table,
                        row,
                        || Value::known(F::from(row as u64)),
                    )?;
                }
                Ok(())
            },
        )?;
        let first = layouter.assign_region(
            || "repeated and varied lookup groups",
            |mut region| {
                let mut first = None;
                for (row, addend) in [0, 7, 2, 7, 1, 9, 4, 2, 11, 5, 0, 11, 3, 4, 9, 1]
                    .into_iter()
                    .enumerate()
                {
                    config.selector.enable(&mut region, row)?;
                    let value = self.offset.map(|offset| offset + F::from(addend));
                    let assigned = region.assign_advice(config.advice, row, value);
                    if row == 0 {
                        first = Some(assigned.cell());
                    }
                    assigned.copy_advice(&mut region, config.copied, row);
                }
                Ok(first.expect("sixteen fixture rows"))
            },
        )?;
        layouter.constrain_instance(first, config.instance, 0);
        Ok(())
    }
}

fn check_seeded_lookup_proofs<C: SerdeCurveAffine>(parity: &str)
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(6);
    for witness in [1, 2] {
        let public_value = C::Scalar::from(witness);
        let circuit = RecoveryLookupCircuit {
            offset: Value::known(public_value),
        };
        let pk = keygen_pk2(&params, &circuit, true).expect("lookup recovery key");
        let vk = pk.get_vk().clone();
        assert_eq!(vk.get_domain().extended_len() / 64, 8);
        assert_eq!(vk.cs().lookups().len(), 1);
        let public = [public_value];
        let columns = [public.as_slice()];
        let instances = [columns.as_slice()];
        let verify = |proof: &[u8], public_value: C::Scalar| {
            let public = [public_value];
            let columns = [public.as_slice()];
            let instances = [columns.as_slice()];
            let mut transcript = Blake2bRead::<_, C, Challenge255<C>>::init(proof);
            verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C>, _, _, _>(
                &params,
                &vk,
                SingleStrategy::new(&params),
                &instances,
                &mut transcript,
            )
        };
        let mut expected = None;
        for _ in 0..3 {
            let mut borrowed = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
            create_proof::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
                &params,
                &pk,
                &[circuit.clone()],
                &instances,
                ChaCha20Rng::from_seed([101; 32]),
                &mut borrowed,
            )
            .expect("seeded borrowed lookup proof");
            let borrowed = borrowed.finalize();
            if let Some(expected) = &expected {
                assert_eq!(
                    &borrowed, expected,
                    "fresh HashMap seeds must not alter recovery bytes"
                );
            } else {
                expected = Some(borrowed.clone());
            }
            let mut consuming = Blake2bWrite::<_, C, Challenge255<C>>::init(Vec::new());
            let returned_vk =
                create_proof_consuming::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
                    &params,
                    pk.clone(),
                    circuit.clone(),
                    &instances,
                    ChaCha20Rng::from_seed([101; 32]),
                    &mut consuming,
                )
                .expect("seeded consuming lookup proof");
            let consuming = consuming.finalize();
            assert_eq!(
                consuming, borrowed,
                "both prover owners recover exactly the same lookup proof"
            );
            assert_eq!(
                returned_vk.to_bytes(SerdeFormat::Processed),
                vk.to_bytes(SerdeFormat::Processed)
            );
            for proof in [borrowed, consuming] {
                verify(&proof, public_value).expect("full IPA verification");
                assert!(verify(&proof, public_value + C::Scalar::ONE).is_err());
                let mut corrupt = proof;
                let last = corrupt.len() - 1;
                corrupt[last] ^= 1;
                assert!(verify(&corrupt, public_value).is_err());
            }
        }
        // A process runner can compare these public-fixture fingerprints across
        // fresh processes and Rayon thread counts without storing proof payloads.
        eprintln!(
            "lookup_recovery_digest parity={parity} witness={witness} digest={}",
            blake2b_simd::blake2b(&expected.expect("three proof rounds")).to_hex()
        );
    }
}

#[test]
fn eq_seeded_lookup_recovery_is_exact_for_borrowed_and_consuming_provers() {
    check_seeded_lookup_proofs::<EqAffine>("eq");
}

#[test]
fn ep_seeded_lookup_recovery_is_exact_for_borrowed_and_consuming_provers() {
    check_seeded_lookup_proofs::<EpAffine>("ep");
}
