//! Small-key regression for the actual witness-loaded recursive protocol identity.
//!
//! These tests exercise real VK generation and the maintained in-circuit protocol identity.
//! They do not qualify the monetary State, Terminal or Wrapper circuits. The release graph
//! must additionally prove its final Wrapper descriptor matches the frozen State descriptor.
use super::*;
use crate::kagemusha_v1_poseidon::digest_limbs;
use halo2_proofs::{
    SerdeFormat,
    circuit::{Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    halo2curves::pasta::{EpAffine, EqAffine},
    plonk::{
        Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Fixed, Instance, keygen_vk,
    },
    poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
};
use snark_verifier::system::halo2::{Config, compile};

/// Both key values are fixed cells, so changing the actual predecessor produces a new VK
/// while the verifier topology, query rotations and quotient expression stay unchanged.
#[derive(Clone)]
struct FixedPredecessorIdentity<F: ff::PrimeField> {
    predecessor: [F; 2],
}
impl<F: ff::PrimeField> Circuit<F> for FixedPredecessorIdentity<F> {
    type Config = (Column<Advice>, Column<Fixed>, Column<Instance>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        self.clone()
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let fixed = meta.fixed_column();
        let instance = meta.instance_column();
        meta.enable_equality(advice);
        meta.enable_equality(fixed);
        meta.enable_equality(instance);
        (advice, fixed, instance)
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let cells = layouter.assign_region(
            || "exact fixed predecessor identity",
            |mut region| {
                let mut cells = Vec::new();
                for (row, value) in self.predecessor.iter().copied().enumerate() {
                    let fixed = region.assign_fixed(config.1, row, value);
                    let advice = region.assign_advice(config.0, row, Value::known(value));
                    region.constrain_equal(fixed, advice.cell());
                    cells.push(advice.cell());
                }
                Ok(cells)
            },
        )?;
        for (row, cell) in cells.into_iter().enumerate() {
            layouter.constrain_instance(cell, config.2, row);
        }
        Ok(())
    }
}

fn fixed_protocol<C>(params: &ParamsIPA<C>, predecessor: [C::ScalarExt; 2]) -> PlonkProtocol<C>
where
    C: CurveAffineExt,
    C::ScalarExt: ff::PrimeField,
    C::Scalar: ff::FromUniformBytes<64>,
{
    let circuit = FixedPredecessorIdentity { predecessor };
    let key = keygen_vk(params, &circuit).expect("actual small fixed-parent VK");
    compile(params, &key, Config::ipa().with_num_instance(vec![2]))
}

/// Construct precisely the maintained dynamic identity relation. All preprocessed points
/// and transcript state are witnesses; only the complete value-free descriptor is fixed.
fn dynamic_identity<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaParityV1,
) -> BaseCircuitBuilder<C::ScalarExt>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    const K: usize = 13;
    let descriptor = kagemusha_protocol_structure_digest_v1(protocol, parity).unwrap();
    let identity = native_parent_protocol_digest_v1(protocol, parity).unwrap();
    let mut base = BaseCircuitBuilder::new(false)
        .use_k(K)
        .use_lookup_bits(K - 1)
        .use_instance_columns(1);
    let expected =
        digest_limbs::<C::ScalarExt>(identity).map(|value| base.main(0).load_witness(value));
    base.assigned_instances = vec![expected.to_vec()];
    let range = base.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut base, &coordinate, &scalar_integer);
    let loaded =
        load_and_constrain_parent_protocol_v1(&loader, protocol, parity, descriptor, &expected)
            .expect("actual maintained dynamic parent identity");
    assert_eq!(
        loaded.protocol.preprocessed.len(),
        protocol.preprocessed.len()
    );
    drop(loaded);
    *base.pool(0) = loader.take_ctx();
    drop(loader);
    base.calculate_params(Some(9));
    base
}

fn vk_original<C>(params: &ParamsIPA<C>, circuit: &BaseCircuitBuilder<C::ScalarExt>) -> Vec<u8>
where
    C: CurveAffineExt + halo2_proofs::SerdeCurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let key = keygen_vk(params, circuit).expect("actual small witness-parent VK");
    let mut bytes = Vec::new();
    key.write(&mut bytes, SerdeFormat::Processed).unwrap();
    bytes
}

fn check_planner<C>(parity: KagemushaPastaParityV1)
where
    C: CurveAffineExt + halo2_proofs::SerdeCurveAffine,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
    C::Scalar: halo2_proofs::SerdePrimeField + ff::FromUniformBytes<64>,
{
    let parent_parameters = ParamsIPA::<C>::new(8);
    let identity_parameters = ParamsIPA::<C>::new(13);
    // These are genuine small keys with the same shape but different fixed-parent identities.
    let seed_wrapper = fixed_protocol::<C>(
        &parent_parameters,
        [C::ScalarExt::from(11), C::ScalarExt::from(12)],
    );
    let first_state = fixed_protocol::<C>(
        &parent_parameters,
        [C::ScalarExt::from(21), C::ScalarExt::from(22)],
    );
    let state_digest = native_parent_protocol_digest_v1(&first_state, parity).unwrap();
    let final_terminal = fixed_protocol::<C>(&parent_parameters, digest_limbs(state_digest));
    let terminal_digest = native_parent_protocol_digest_v1(&final_terminal, parity).unwrap();
    let final_wrapper = fixed_protocol::<C>(&parent_parameters, digest_limbs(terminal_digest));
    let seed_descriptor = kagemusha_protocol_structure_digest_v1(&seed_wrapper, parity).unwrap();
    let final_descriptor = kagemusha_protocol_structure_digest_v1(&final_wrapper, parity).unwrap();
    assert_eq!(
        seed_descriptor, final_descriptor,
        "final held Wrapper layout must match the frozen State descriptor"
    );
    let seed_identity = native_parent_protocol_digest_v1(&seed_wrapper, parity).unwrap();
    let final_identity = native_parent_protocol_digest_v1(&final_wrapper, parity).unwrap();
    assert_ne!(
        seed_identity, final_identity,
        "descriptor convergence never asks key identities to converge"
    );
    let first = dynamic_identity::<C>(&seed_wrapper, parity);
    let finalized = dynamic_identity::<C>(&final_wrapper, parity);
    let left = first.params();
    let right = finalized.params();
    assert_eq!(
        (
            left.k,
            left.num_advice_per_phase,
            left.num_fixed,
            left.num_lookup_advice_per_phase,
            left.lookup_bits,
            left.num_instance_columns
        ),
        (
            right.k,
            right.num_advice_per_phase,
            right.num_fixed,
            right.num_lookup_advice_per_phase,
            right.lookup_bits,
            right.num_instance_columns
        ),
        "same final descriptor must retain identical physical layout"
    );
    assert_eq!(
        vk_original(&identity_parameters, &first),
        vk_original(&identity_parameters, &finalized),
        "final VK commitments are witness values, not a key-hash fixed point"
    );
    let public = vec![digest_limbs::<C::ScalarExt>(final_identity).to_vec()];
    MockProver::run(13, &finalized, public.clone())
        .unwrap()
        .assert_satisfied();
    let mut substituted = public;
    substituted[0][0] += C::ScalarExt::ONE;
    assert!(
        MockProver::run(13, &finalized, substituted)
            .unwrap()
            .verify()
            .is_err(),
        "full protocol identity remains constrained to exact public limbs"
    );
    let mut wrong_layout = final_wrapper.clone();
    wrong_layout.num_witness[0] += 1;
    assert_ne!(
        seed_descriptor,
        kagemusha_protocol_structure_digest_v1(&wrong_layout, parity).unwrap(),
        "a different verifier shape cannot pass final graph sealing"
    );
}

#[test]
fn ordinary_recursive_key_plan_converges_only_layout_and_binds_final_protocol_identity() {
    check_planner::<EqAffine>(KagemushaPastaParityV1::Eq);
    check_planner::<EpAffine>(KagemushaPastaParityV1::Ep);
}
