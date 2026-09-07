//! Cached-key proof equivalence against the frozen original borrowed prover.

use super::*;
use crate::{
    circuit::{Layouter, SimpleFloorPlanner},
    plonk::{FirstPhase, SecondPhase, ThirdPhase, keygen_pk, keygen_vk, verifier::verify_proof},
    poly::{
        Rotation, VerificationStrategy as _,
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
use halo2curves::pasta::{EpAffine, EqAffine};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

#[derive(Clone)]
struct AdviceCircuit<F: Field> {
    value: Assigned<F>,
    expose_reference: bool,
    overwrite: bool,
    commit_final: bool,
}

#[derive(Clone, Copy, Debug)]
struct AdviceConfig {
    first: Column<Advice>,
    second: Column<Advice>,
    third: Column<Advice>,
    after_first: Challenge,
    after_second: Challenge,
    selector: Selector,
}

impl<F: Field> Circuit<F> for AdviceCircuit<F> {
    type Config = AdviceConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self {
            value: Assigned::Zero,
            ..self.clone()
        }
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let first = meta.advice_column_in(FirstPhase);
        let second = meta.advice_column_in(SecondPhase);
        let third = meta.advice_column_in(ThirdPhase);
        let after_first = meta.challenge_usable_after(FirstPhase);
        let after_second = meta.challenge_usable_after(SecondPhase);
        let selector = meta.selector();
        meta.create_gate("cached-key advice values survive all phases", |meta| {
            let first = meta.query_advice(first, Rotation::cur());
            let second = meta.query_advice(second, Rotation::cur());
            let third = meta.query_advice(third, Rotation::cur());
            let after_first = meta.query_challenge(after_first);
            let after_second = meta.query_challenge(after_second);
            let selector = meta.query_selector(selector);
            vec![
                selector.clone() * (second.clone() - first.clone() * after_first),
                selector * (third - first - second * after_second),
            ]
        });
        AdviceConfig {
            first,
            second,
            third,
            after_first,
            after_second,
            selector,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let first = layouter.assign_region(
            || "first phase may expose a retained reference",
            |mut region| {
                config.selector.enable(&mut region, 0)?;
                if self.overwrite {
                    region.assign_advice_discarding_value(config.first, 0, Value::known(F::ONE));
                }
                if self.expose_reference {
                    Ok(Some(region.assign_advice(
                        config.first,
                        0,
                        Value::known(self.value),
                    )))
                } else {
                    region.assign_advice_discarding_value(
                        config.first,
                        0,
                        Value::known(self.value),
                    );
                    Ok(None)
                }
            },
        )?;
        layouter.next_phase();
        let first_value = || {
            first
                .as_ref()
                .map_or(Value::known(self.value), |cell| cell.value().map(|v| **v))
        };
        let second_value = first_value() * layouter.get_challenge(config.after_first);
        let second = layouter.assign_region(
            || "second phase exposes a reference",
            |mut region| Ok(region.assign_advice(config.second, 0, second_value)),
        )?;
        layouter.next_phase();
        // Read the first reference again after two phase commits. Reclaiming exposed backing
        // storage early would violate this supported assignment API even if transcript bytes
        // happened to match in an exclusively discarding-value circuit.
        let third_value = first_value()
            + second.value().map(|v| **v) * layouter.get_challenge(config.after_second);
        layouter.assign_region(
            || "third phase discards its assignment value",
            |mut region| {
                region.assign_advice_discarding_value(config.third, 0, third_value);
                Ok(())
            },
        )?;
        if self.commit_final {
            layouter.next_phase();
        }
        Ok(())
    }
}

fn check_cached_key<C: CurveAffine + crate::SerdeCurveAffine>()
where
    C::ScalarExt: WithSmallOrderMulGroup<3> + crate::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let params = ParamsIPA::<C>::new(5);
    let template = AdviceCircuit {
        value: Assigned::Zero,
        expose_reference: false,
        overwrite: false,
        commit_final: false,
    };
    let vk = keygen_vk(&params, &template).expect("cached-key test VK");
    let pk = keygen_pk(&params, vk, &template).expect("cached-key test PK");
    let original_pk = pk.to_bytes(crate::SerdeFormat::Processed);
    let proof_instances: [&[&[C::ScalarExt]]; 1] = [&[]];
    let seed = [83_u8; 32];
    for value in [
        Assigned::Zero,
        Assigned::Trivial(C::ScalarExt::from(7)),
        Assigned::Rational(C::ScalarExt::from(10), C::ScalarExt::from(2)),
        Assigned::Rational(C::ScalarExt::ONE, C::ScalarExt::ZERO),
    ] {
        for expose_reference in [false, true] {
            for overwrite in [false, true] {
                for commit_final in [false, true] {
                    let circuit = AdviceCircuit {
                        value,
                        expose_reference,
                        overwrite,
                        commit_final,
                    };
                    let circuits = [circuit];
                    let mut reference = Blake2bWrite::<_, _, Challenge255<_>>::init(Vec::new());
                    super::borrowed_advice_reference::create_proof_reference::<
                        IPACommitmentScheme<C>,
                        ProverIPA<C>,
                        _,
                        _,
                        _,
                        _,
                    >(
                        &params,
                        &pk,
                        &circuits,
                        &proof_instances,
                        ChaCha20Rng::from_seed(seed),
                        &mut reference,
                    )
                    .expect("original borrowed cached-key proof");
                    let mut actual = Blake2bWrite::<_, _, Challenge255<_>>::init(Vec::new());
                    create_proof::<IPACommitmentScheme<C>, ProverIPA<C>, _, _, _, _>(
                        &params,
                        &pk,
                        &circuits,
                        &proof_instances,
                        ChaCha20Rng::from_seed(seed),
                        &mut actual,
                    )
                    .expect("transferred advice cached-key proof");
                    let proof = actual.finalize();
                    assert_eq!(proof, reference.finalize());
                    let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&proof[..]);
                    verify_proof::<IPACommitmentScheme<C>, VerifierIPA<C>, _, _, _>(
                        &params,
                        pk.get_vk(),
                        SingleStrategy::new(&params),
                        &proof_instances,
                        &mut transcript,
                    )
                    .expect("actual IPA verification after borrowed advice transfer");
                    assert_eq!(pk.to_bytes(crate::SerdeFormat::Processed), original_pk);
                }
            }
        }
    }
}

#[test]
fn eq_cached_key_advice_matches_original_proofs() {
    check_cached_key::<EqAffine>();
}

#[test]
fn ep_cached_key_advice_matches_original_proofs() {
    check_cached_key::<EpAffine>();
}
