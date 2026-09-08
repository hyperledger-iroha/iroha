//! Fixed provider-policy configuration and its native-reconstructed Guard constraint.

use halo2_base::{AssignedValue, gates::circuit::BaseCircuitParams};
use halo2_proofs::{
    circuit::{Layouter, Value},
    plonk::{Advice, Column, ConstraintSystem, Error, Expression, Selector},
    poly::Rotation,
};

use super::DigestV1;
use crate::zk::kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs};

/// Exact key configuration for a provider-root-bound credential or Guard relation.
///
/// Setup inputs are non-authorizing. Runtime obtains the root from an authenticated release.
/// The default value exists solely for Halo2's trait and cannot configure an accepting circuit.
#[derive(Clone, Debug, Default)]
pub struct KagemushaProviderRootCircuitParamsV1 {
    /// Native Base layout, independently covered by the authenticated native profile.
    pub base: BaseCircuitParams,
    /// Governed provider-policy tree root fixed for the complete key lifetime.
    pub provider_policy_root: DigestV1,
}

impl KagemushaProviderRootCircuitParamsV1 {
    /// Build a non-authorizing setup configuration with an explicit nonzero root.
    ///
    /// # Errors
    /// Rejects a missing root; the release loader separately authenticates the selected value.
    pub fn new(base: BaseCircuitParams, provider_policy_root: DigestV1) -> Result<Self, String> {
        if provider_policy_root == [0; 32] {
            return Err("Kagemusha provider-policy root is absent".to_owned());
        }
        Ok(Self {
            base,
            provider_policy_root,
        })
    }
}

/// The root is part of the constraint-system expressions reconstructed by the native VK reader.
#[derive(Clone, Debug)]
pub(super) struct ProviderPolicyRootConfigV1 {
    limbs: [Column<Advice>; 2],
    enabled: Selector,
}

impl ProviderPolicyRootConfigV1 {
    pub(super) fn configure<F: KagemushaPoseidonFieldV1>(
        meta: &mut ConstraintSystem<F>,
        root: DigestV1,
    ) -> Self {
        assert_ne!(root, [0; 32], "Kagemusha provider-policy root is absent");
        let limbs = std::array::from_fn(|_| meta.advice_column());
        for column in limbs {
            meta.enable_equality(column);
        }
        let enabled = meta.selector();
        let expected = digest_limbs::<F>(root);
        meta.create_gate("Kagemusha governed provider-policy root", |meta| {
            let active = meta.query_selector(enabled);
            limbs
                .into_iter()
                .zip(expected)
                .map(|(column, expected)| {
                    active.clone()
                        * (meta.query_advice(column, Rotation::cur())
                            - Expression::Constant(expected))
                })
                .collect::<Vec<_>>()
        });
        Self { limbs, enabled }
    }

    pub(super) fn synthesize<F: KagemushaPoseidonFieldV1>(
        &self,
        layouter: &mut impl Layouter<F>,
        actual: [AssignedValue<F>; 2],
        copy_manager: &halo2_base::virtual_region::copy_constraints::SharedCopyConstraintManager<F>,
        witness_gen_only: bool,
    ) -> Result<(), Error> {
        let physical = if witness_gen_only {
            None
        } else {
            Some(copy_manager.lock().map_err(|_| Error::Synthesis)?)
        };
        layouter.assign_region(
            || "Kagemusha governed provider-policy binding",
            |mut region| {
                self.enabled.enable(&mut region, 0)?;
                for (column, value) in self.limbs.into_iter().zip(actual) {
                    let assigned = region.assign_advice(column, 0, Value::known(*value.value()));
                    if let Some(physical) = &physical {
                        let virtual_cell = value.cell.ok_or(Error::Synthesis)?;
                        let original = physical
                            .assigned_advices
                            .resolve(&virtual_cell)
                            .ok_or(Error::Synthesis)?;
                        region.constrain_equal(assigned.cell(), original);
                    }
                }
                Ok(())
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::{BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        SerdeFormat,
        circuit::V1,
        dev::MockProver,
        halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
        plonk::{Circuit, VerifyingKey, keygen_vk_custom},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };

    #[derive(Clone)]
    struct RootHarness<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        root: DigestV1,
        actual: [AssignedValue<F>; 2],
    }

    impl<F: KagemushaPoseidonFieldV1> RootHarness<F> {
        fn new(root: DigestV1, actual: DigestV1) -> Self {
            let mut builder = BaseCircuitBuilder::new(false)
                .use_k(6)
                .use_instance_columns(0);
            let actual = digest_limbs::<F>(actual).map(|value| builder.main(0).load_witness(value));
            builder.calculate_params(Some(9));
            Self {
                builder,
                root,
                actual,
            }
        }
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for RootHarness<F> {
        type Config = (BaseConfig<F>, ProviderPolicyRootConfigV1);
        type FloorPlanner = V1;
        type Params = KagemushaProviderRootCircuitParamsV1;

        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                root: self.root,
                actual: self.actual,
            }
        }

        fn params(&self) -> Self::Params {
            KagemushaProviderRootCircuitParamsV1::new(self.builder.config_params.clone(), self.root)
                .expect("explicit test root")
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("root-bound configuration is mandatory")
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params.base);
            base.set_usable_rows(55);
            (
                base,
                ProviderPolicyRootConfigV1::configure(meta, params.provider_policy_root),
            )
        }

        fn synthesize(
            &self,
            (base, root): Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                base,
                layouter.namespace(|| "test Base root"),
            )?;
            root.synthesize(
                &mut layouter,
                self.actual,
                &self.builder.core().copy_manager,
                self.builder.witness_gen_only(),
            )
        }
    }

    #[test]
    fn provider_root_gate_binds_both_pasta_parities_and_the_original_base_cells() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            let root = [0x31; 32];
            let valid = RootHarness::<F>::new(root, root);
            MockProver::run(6, &valid, vec![])
                .expect("valid root synthesis")
                .assert_satisfied();
            let wrong = RootHarness::<F>::new(root, [0x32; 32]);
            assert!(
                MockProver::run(6, &wrong, vec![])
                    .expect("wrong root synthesis")
                    .verify()
                    .is_err()
            );
            // The custom gate still receives the expected root value, but its original Base
            // cell was changed. Only the actual cross-region copy binding rejects this case.
            let mut detached = RootHarness::<F>::new(root, root);
            detached.actual[0].debug_prank(
                detached.builder.main(0),
                *detached.actual[0].value() + F::ONE,
            );
            assert!(
                MockProver::run(6, &detached, vec![])
                    .expect("detached root synthesis")
                    .verify()
                    .is_err()
            );
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn provider_root_changes_native_reconstructed_vk_identity_in_both_parities() {
        // Tiny real keys qualify the CS-expression binding, not the full K16 monetary circuit.
        macro_rules! check {
            ($curve:ty, $field:ty) => {{
                let params = ParamsIPA::<$curve>::new(6);
                let circuit = RootHarness::<$field>::new([0x31; 32], [0x31; 32]);
                let key = keygen_vk_custom(&params, &circuit, true).expect("root-bound tiny key");
                let bytes = key.to_bytes(SerdeFormat::Processed);
                let mut changed = circuit.params();
                changed.provider_policy_root = [0x32; 32];
                let recovered = VerifyingKey::<$curve>::read_checked::<_, RootHarness<$field>>(
                    &mut std::io::Cursor::new(&bytes),
                    SerdeFormat::Processed,
                    6,
                    changed,
                )
                .expect("same bounded key bytes under different root configuration");
                assert_ne!(
                    recovered.transcript_repr(),
                    key.transcript_repr(),
                    "native protocol identity must include the configured root"
                );
                let original = VerifyingKey::<$curve>::read_checked::<_, RootHarness<$field>>(
                    &mut std::io::Cursor::new(&bytes),
                    SerdeFormat::Processed,
                    6,
                    circuit.params(),
                )
                .expect("exact root configuration");
                assert_eq!(original.transcript_repr(), key.transcript_repr());
            }};
        }
        check!(EqAffine, Fp);
        check!(EpAffine, Fq);
    }

    #[test]
    fn provider_root_setup_rejects_a_missing_root() {
        assert!(
            KagemushaProviderRootCircuitParamsV1::new(BaseCircuitParams::default(), [0; 32])
                .is_err()
        );
        assert!(
            KagemushaProviderRootCircuitParamsV1::new(BaseCircuitParams::default(), [1; 32])
                .is_ok()
        );
    }
}
