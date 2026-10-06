//! Canonical certificate/bridge boundary, shape, and every-cell regressions.

use super::*;
use crate::{
    GlueConfig, Word,
    ff::FfConfig,
    range::{LimbBits, RunningSumChip, RunningSumConfig},
    tamper::undetected_tampers,
};
use iroha_pasta::{Fp, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};

#[derive(Clone)]
struct Bridge<F: PastaField> {
    modulus: ForeignModulus,
    target: Option<ForeignModulus>,
    integer: Nat,
    reverse: bool,
    known: bool,
    marker: core::marker::PhantomData<F>,
}
#[derive(Clone, Debug)]
struct Config {
    ff: FfConfig,
    glue: GlueConfig,
    range: RunningSumConfig,
    public: Column<Instance>,
}
impl<F: PastaField> Circuit<F> for Bridge<F> {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let ff_cols = core::array::from_fn(|_| meta.advice_column());
        let ff = FfConfig::configure(
            meta,
            ff_cols,
            &[ForeignModulus::PASTA_FP, ForeignModulus::PASTA_FQ],
        );
        let glue_cols = core::array::from_fn(|_| meta.advice_column());
        let constants = meta.fixed_column();
        let glue = GlueConfig::configure(meta, glue_cols, constants);
        let range_col = meta.advice_column();
        let range = RunningSumConfig::configure(meta, range_col, LimbBits::new(15).unwrap());
        let public = meta.instance_column(5);
        meta.enable_equality(public);
        Config {
            ff,
            glue,
            range,
            public,
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let mut ff = FfChip::new(config.ff);
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        ff.load_table(&mut layouter)?;
        range.load_table(&mut layouter)?;
        let public = layouter.assign_region(
            || "canonical certificate round trip",
            |mut region| {
                let known = |value| {
                    if self.known {
                        Value::known(value)
                    } else {
                        Value::unknown()
                    }
                };
                let mut uint = UintChip::new(&mut glue, &mut range);
                let (scalar, encoded) = if self.reverse {
                    let scalar = ff.witness_canonical(
                        &mut region,
                        self.modulus,
                        known(self.integer).map(|n| n.low_words()),
                    )?;
                    let encoded = ff.export_s6(&mut uint, &mut region, &scalar)?;
                    let imported = ff.import_s6(&mut uint, &mut region, &encoded)?;
                    FfChip::assert_equal(&mut region, &scalar, &imported)?;
                    (imported, encoded)
                } else {
                    let lo =
                        uint.assign::<128>(&mut region, known(self.integer).map(|n| n.low_u128()))?;
                    let hi = uint.assign::<127>(
                        &mut region,
                        known(self.integer).map(|n| n.shr(128).low_u128()),
                    )?;
                    let encoded =
                        CanonicalS6::from_limbs(&mut uint, &mut region, self.modulus, &lo, &hi)?;
                    let encoded = if let Some(target) = self.target {
                        encoded.with_modulus(&mut uint, &mut region, target)?
                    } else {
                        encoded
                    };
                    let scalar = ff.import_s6(&mut uint, &mut region, &encoded)?;
                    let exported = ff.export_s6(&mut uint, &mut region, &scalar)?;
                    GlueChip::assert_equal(&mut region, encoded.lo.word(), exported.lo.word())?;
                    GlueChip::assert_equal(&mut region, encoded.hi.word(), exported.hi.word())?;
                    (scalar, exported)
                };
                let mut out: Vec<Word<F>> = scalar.limbs().to_vec();
                out.extend([encoded.lo.word().clone(), encoded.hi.word().clone()]);
                Ok(out)
            },
        )?;
        for (row, word) in public.into_iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
impl<F: PastaField> Bridge<F> {
    fn public(&self) -> Vec<F> {
        let mut out = super::super::limb_fields::<F>(&self.integer).to_vec();
        out.extend([
            F::from_u128(self.integer.low_u128()),
            F::from_u128(self.integer.shr(128).low_u128()),
        ]);
        out
    }
}
fn boundaries<F: PastaField>() {
    for modulus in [ForeignModulus::PASTA_FP, ForeignModulus::PASTA_FQ] {
        for reverse in [false, true] {
            for integer in [
                Nat::ZERO,
                Nat::ONE,
                modulus.nat().wrapping_sub(&Nat::ONE),
                modulus.nat(),
                modulus.nat().wrapping_add(&Nat::ONE),
                Nat::pow2(255),
            ] {
                let circuit = Bridge::<F> {
                    modulus,
                    target: None,
                    integer,
                    reverse,
                    known: true,
                    marker: core::marker::PhantomData,
                };
                let report =
                    check_circuit(&circuit, 16, &[circuit.public()], CheckMode::Strict).unwrap();
                assert_eq!(
                    report.is_satisfied(),
                    integer.cmp_vartime(&modulus.nat()).is_lt(),
                    "{modulus:?} {reverse} {integer:?}"
                );
            }
        }
    }
}
#[test]
fn canonical_s6_bridges_reject_modulus_and_top_bit_aliases_both_fields() {
    boundaries::<Fp>();
    boundaries::<Fq>();
}

fn tamper<F: PastaField>() {
    let modulus = ForeignModulus::PASTA_FQ;
    for reverse in [false, true] {
        let circuit = Bridge::<F> {
            modulus,
            target: None,
            integer: modulus.nat().wrapping_sub(&Nat::ONE),
            reverse,
            known: true,
            marker: core::marker::PhantomData,
        };
        let public = [circuit.public()];
        let assigned = synthesize(&circuit, 16, Some(&public)).unwrap();
        let unknown = synthesize(&circuit.without_witnesses(), 16, None).unwrap();
        assert_eq!(assigned.tables.fixed(), unknown.tables.fixed());
        assert_eq!(assigned.tables.selectors(), unknown.tables.selectors());
        assert_eq!(assigned.tables.permutation(), unknown.tables.permutation());
        assert_eq!(
            assigned.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
        assert!(
            undetected_tampers(&circuit, 16, &public)
                .unwrap()
                .is_empty()
        );
    }
}
#[test]
fn canonical_s6_bridge_every_cell_is_bound_and_unknown_shape_matches() {
    tamper::<Fp>();
    tamper::<Fq>();
}

#[test]
fn canonical_s6_modulus_change_proves_narrowing_and_retains_source_checks() {
    fn run<F: PastaField>() {
        let p = ForeignModulus::PASTA_FP;
        let q = ForeignModulus::PASTA_FQ;
        for (source, target, integer, accepted) in [
            (p, q, Nat::ONE, true),
            (q, p, p.nat().wrapping_sub(&Nat::ONE), true),
            (q, p, p.nat(), false),
            (p, q, p.nat(), false),
        ] {
            let circuit = Bridge::<F> {
                modulus: source,
                target: Some(target),
                integer,
                reverse: false,
                known: true,
                marker: core::marker::PhantomData,
            };
            let report =
                check_circuit(&circuit, 16, &[circuit.public()], CheckMode::Strict).unwrap();
            assert_eq!(report.is_satisfied(), accepted);
        }
        for target in [
            ForeignModulus::P256_BASE,
            ForeignModulus::new([5, 0, 0, 0x4000_0000_0000_0000]).unwrap(),
        ] {
            let circuit = Bridge::<F> {
                modulus: p,
                target: Some(target),
                integer: Nat::ONE,
                reverse: false,
                known: true,
                marker: core::marker::PhantomData,
            };
            assert!(check_circuit(&circuit, 16, &[circuit.public()], CheckMode::Strict).is_err());
        }
    }
    run::<Fp>();
    run::<Fq>();
}
