//! Injective typed Q framing for the fixed Receive context representation.

use super::*;
use iroha_pasta::Fq;
use iroha_plonk::{
    check::{CheckMode, check},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, synthesize},
};
use iroha_plonk_gadgets::statement::foreign_limbs;
use iroha_plonk_recursion::verifier::VerifierConfig;

#[derive(Clone, Debug)]
struct Config {
    verifier: VerifierConfig<Ep>,
    public: Column<Instance>,
}

#[derive(Clone)]
struct Encoding {
    value: Fq,
    ty: InstanceType,
    receive: bool,
    known: bool,
}

impl Circuit<Fp> for Encoding {
    type Config = Config;
    type Params = ();
    type FloorPlanner = SimpleFloorPlanner;
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let verifier = VerifierConfig::configure(meta);
        let public = meta.instance_column(2);
        meta.enable_equality(public);
        Config { verifier, public }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut chip = VerifierChip::new(config.verifier);
        chip.load_tables(&mut layouter)?;
        let output = layouter.assign_region(
            || "typed Receive Q context",
            |mut region| {
                let witness = |value| {
                    if self.known {
                        Value::known(value)
                    } else {
                        Value::unknown()
                    }
                };
                let [lo, hi] = foreign_limbs(&self.value);
                let lo = chip.uint().assign::<128>(&mut region, witness(lo))?;
                let hi = chip.uint().assign::<127>(&mut region, witness(hi))?;
                let value = ScalarCells::from_limbs(&mut chip.uint(), &mut region, &lo, &hi)?;
                let mut words = Vec::new();
                push_q_context(
                    &mut words,
                    &mut chip,
                    &mut region,
                    &value,
                    self.ty,
                    self.receive,
                )?;
                assert_eq!(words.len(), if self.packed() { 1 } else { 2 });
                // Padding belongs only to this two-column test surface; the real
                // context appends exactly one or two words according to its schema.
                if words.len() == 1 {
                    words.push(chip.uint().glue().constant(&mut region, Fp::ZERO)?);
                }
                Ok(words)
            },
        )?;
        for (i, value) in output.iter().enumerate() {
            layouter.constrain_instance(value.cell(), config.public, i)?;
        }
        Ok(())
    }
}

impl Encoding {
    fn packed(&self) -> bool {
        self.receive && matches!(self.ty, InstanceType::Bounded | InstanceType::Bits(0..=253))
    }
    fn public(&self) -> Vec<Vec<Fp>> {
        let [lo, hi] = foreign_limbs(&self.value).map(Fp::from_u128);
        vec![if self.packed() {
            vec![lo + hi * Fp::from(2).pow_vartime([128]), Fp::ZERO]
        } else {
            vec![lo, hi]
        }]
    }
    fn accepts(&self, public: &[Vec<Fp>]) -> bool {
        synthesize(self, 16, Some(public)).is_ok_and(|a| {
            check(&a.cs, &a.tables, CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        })
    }
}

#[test]
fn receive_typed_q_encoding_rejects_modular_aliases_and_exact_bit_overflow() {
    let p_minus_one = Fq::from_repr((-Fp::ONE).to_repr()).unwrap();
    for value in [Fq::ZERO, Fq::ONE, p_minus_one] {
        let c = Encoding {
            value,
            ty: InstanceType::Bounded,
            receive: true,
            known: true,
        };
        assert!(c.accepts(&c.public()));
        let mut wrong = c.public();
        wrong[0][0] += Fp::ONE;
        assert!(!c.accepts(&wrong));
    }
    for value in [p_minus_one + Fq::ONE, p_minus_one + Fq::from(2), -Fq::ONE] {
        let c = Encoding {
            value,
            ty: InstanceType::Bounded,
            receive: true,
            known: true,
        };
        assert!(
            !c.accepts(&c.public()),
            "a canonical Fq scalar is not an injective Fp encoding"
        );
        for ty in [InstanceType::Field, InstanceType::Bits(254)] {
            let full = Encoding { ty, ..c.clone() };
            assert!(!full.packed());
            assert!(full.accepts(&full.public()));
        }
    }
    for (value, expected) in [(7, true), (8, false)] {
        let c = Encoding {
            value: Fq::from(value),
            ty: InstanceType::Bits(3),
            receive: true,
            known: true,
        };
        assert_eq!(c.accepts(&c.public()), expected);
    }
    let explicit = Encoding {
        value: p_minus_one + Fq::ONE,
        ty: InstanceType::Bounded,
        receive: false,
        known: true,
    };
    assert!(!explicit.packed());
    assert!(
        explicit.accepts(&explicit.public()),
        "non-Receive retains the unchanged two-limb context format"
    );
}

#[test]
fn receive_typed_q_encoding_has_witness_independent_layout() {
    for ty in [
        InstanceType::Bounded,
        InstanceType::Bits(5),
        InstanceType::Field,
    ] {
        let c = Encoding {
            value: Fq::from(17),
            ty,
            receive: true,
            known: true,
        };
        let known = synthesize(&c, 16, Some(&c.public())).unwrap();
        let unknown = synthesize(&c.without_witnesses(), 16, None).unwrap();
        assert_eq!(known.cs, unknown.cs);
        assert_eq!(known.tables.fixed(), unknown.tables.fixed());
        assert_eq!(
            known.tables.advice_assigned(),
            unknown.tables.advice_assigned()
        );
        assert_eq!(known.tables.permutation(), unknown.tables.permutation());
    }
}
