//! Exact unsigned batch bounds and adversarial CRT-root checks.

use super::*;
use crate::{
    GlueConfig,
    cells::{RowCursor, SharedRows},
    ff::serialized::SerializedFf,
    ff::{FfChip, rotated::RotatedFfConfig},
    phase::PhaseColumns,
    range::{LimbBits, RunningSumConfig},
    tamper::undetected_tampers,
};
use iroha_pasta::{Fp, Fq};
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::ConstraintSystem,
    frontend::{Circuit, Layouter, SimpleFloorPlanner, synthesize},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Attack {
    None,
    NativeOnly,
    Carry,
    Quotient,
    Lazy,
    Mixed,
}
#[derive(Clone)]
struct Dot<F: PastaField> {
    modulus: ForeignModulus,
    count: usize,
    attack: Attack,
    maximum: bool,
    staged: bool,
    rows: Option<(usize, usize)>,
    known: bool,
    marker: core::marker::PhantomData<F>,
}
impl<F: PastaField> Circuit<F> for Dot<F> {
    type Config = (GlueConfig, RunningSumConfig, Option<RotatedFfConfig>);
    type FloorPlanner = SimpleFloorPlanner;
    type Params = Option<(ForeignModulus, bool)>;
    fn params(&self) -> Self::Params {
        Some((self.modulus, self.staged))
    }
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        Self::configure_with_params(meta, None)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let (modulus, staged) = params.unwrap_or((ForeignModulus::PASTA_FP, false));
        let ports = core::array::from_fn(|_| meta.advice_column());
        let fixed = meta.fixed_column();
        let phases = PhaseColumns::allocate(meta);
        let glue = GlueConfig::configure_phased(meta, ports, fixed, phases);
        let kernel = staged.then(|| {
            let sums = core::array::from_fn(|_| meta.advice_column());
            RotatedFfConfig::configure_staged_phased(meta, ports, sums, modulus, phases)
        });
        let column = meta.advice_column();
        let range = RunningSumConfig::configure_compact(meta, column, LimbBits::new(7).unwrap());
        (glue, range, kernel)
    }
    fn synthesize(
        &self,
        (glue, range, kernel): Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let mut glue = GlueChip::with_shared_cursor(
            glue,
            &SharedRows::new(self.rows.map_or_else(
                || RowCursor::starting_at(0),
                |(start, end)| RowCursor::bounded(start, end),
            )),
        );
        let mut range =
            RunningSumChip::with_shared_cursor(range, &SharedRows::new(RowCursor::starting_at(0)));
        range.load_table(&mut layouter)?;
        layouter.assign_region(
            || "unsigned dot",
            |mut region| {
                let value = if self.attack == Attack::NativeOnly {
                    Nat::pow2(174)
                } else if self.maximum {
                    Nat::pow2(256).wrapping_sub(&Nat::ONE)
                } else {
                    Nat::ZERO
                };
                let input = if self.known {
                    Value::known(value.low_words())
                } else {
                    Value::unknown()
                };
                let mut values = Vec::new();
                for _ in 0..2 * self.count {
                    values.push(SerializedFf::witness(
                        &mut range,
                        &mut region,
                        self.modulus,
                        input,
                    )?);
                }
                if self.attack == Attack::Lazy {
                    values[0].form = Form::Bounded;
                }
                if self.attack == Attack::Mixed {
                    values[0].modulus = if self.modulus == ForeignModulus::PASTA_FP {
                        ForeignModulus::PASTA_FQ
                    } else {
                        ForeignModulus::PASTA_FP
                    };
                }
                let pairs: Vec<_> = values.chunks_exact(2).map(|v| (&v[0], &v[1])).collect();
                let start = range.next_row();
                let arithmetic = glue.next_row();
                let result = if matches!(
                    self.attack,
                    Attack::NativeOnly | Attack::Carry | Attack::Quotient
                ) {
                    let limbs = nat_limbs(&value);
                    let mut w = witness::<F>(self.modulus, &vec![(limbs, limbs); self.count]);
                    match self.attack {
                        Attack::NativeOnly => {
                            w = FusedWitness {
                                c: [F::ZERO; LIMBS],
                                q: [F::ZERO; LIMBS],
                                u: [F::from_u128(1 << CARRY_OFFSET_BITS); CARRIES],
                            }
                        }
                        Attack::Carry => w.u[3] = F::from_u128(1 << (CARRY_OFFSET_BITS + 1)),
                        Attack::Quotient => w.q[2] = F::from_u128(1 << LIMB_BITS),
                        _ => unreachable!(),
                    }
                    if let Some(kernel) = kernel {
                        kernel
                            .constrain_dot(
                                &mut glue,
                                &mut range,
                                &mut region,
                                &pairs,
                                Value::known(w),
                            )?
                            .ok_or(Error::Synthesis)?
                    } else {
                        constrain(&mut glue, &mut range, &mut region, &pairs, Value::known(w))?
                    }
                } else {
                    let ff = FfChip::serialized(glue.clone(), range.clone(), &[self.modulus]);
                    let mut ff = if let Some(kernel) = kernel {
                        ff.with_rotated_kernel(&kernel)?
                    } else {
                        ff
                    };
                    ff.dot_proper(&mut region, &pairs)?
                };
                assert_eq!(result.form, Form::Proper);
                assert_eq!(range.next_row() - start, 143);
                if self.attack == Attack::None {
                    // Each additional pair costs eleven Glue rows while sharing
                    // all ten result/quotient/carry range certificates.
                    assert_eq!(
                        glue.next_row() - arithmetic,
                        if self.staged {
                            2 * self.count + 3
                        } else {
                            20 + 11 * (self.count - 1)
                        }
                    );
                    let expected = (0..self.count)
                        .fold(Nat::ZERO, |sum, _| {
                            sum.wrapping_add(&value.wrapping_mul(&value))
                        })
                        .div_rem(&self.modulus.nat())
                        .unwrap()
                        .1;
                    for (actual, expected) in result
                        .limbs
                        .iter()
                        .zip(super::super::limb_fields::<F>(&expected))
                    {
                        GlueChip::assert_constant(&mut region, actual, expected)?;
                    }
                }
                Ok(())
            },
        )
    }
}
fn cases<F: PastaField>(staged: bool) {
    for modulus in [
        ForeignModulus::PASTA_FP,
        ForeignModulus::PASTA_FQ,
        ForeignModulus::P256_BASE,
        ForeignModulus::P256_ORDER,
    ] {
        for count in 1..=8 {
            for maximum in [false, true] {
                let circuit = Dot::<F> {
                    modulus,
                    count,
                    attack: Attack::None,
                    maximum,
                    staged,
                    rows: None,
                    known: true,
                    marker: core::marker::PhantomData,
                };
                let report = check_circuit(&circuit, 11, &[], CheckMode::Strict).unwrap();
                assert!(
                    report.is_satisfied(),
                    "{modulus:?} count{count}: {:?}",
                    report.failures().first()
                );
                if maximum && [1, 8].contains(&count) {
                    let assigned = synthesize(&circuit, 11, Some(&[])).unwrap();
                    let unknown = synthesize(&circuit.without_witnesses(), 11, None).unwrap();
                    assert_eq!(assigned.tables.fixed(), unknown.tables.fixed());
                    assert_eq!(assigned.tables.selectors(), unknown.tables.selectors());
                    assert_eq!(assigned.tables.permutation(), unknown.tables.permutation());
                    assert_eq!(
                        assigned.tables.advice_assigned(),
                        unknown.tables.advice_assigned()
                    );
                    assert!(undetected_tampers(&circuit, 11, &[]).unwrap().is_empty());
                }
            }
        }
        let fixture = Dot::<F> {
            modulus,
            count: 8,
            attack: Attack::None,
            maximum: true,
            staged,
            rows: None,
            known: true,
            marker: core::marker::PhantomData,
        };
        for count in [0, 9] {
            assert!(
                synthesize(
                    &Dot {
                        count,
                        ..fixture.clone()
                    },
                    11,
                    Some(&[])
                )
                .is_err()
            );
        }
        for attack in [Attack::Lazy, Attack::Mixed] {
            assert!(
                synthesize(
                    &Dot {
                        attack,
                        ..fixture.clone()
                    },
                    11,
                    Some(&[])
                )
                .is_err()
            );
        }
        for attack in [Attack::NativeOnly, Attack::Carry, Attack::Quotient] {
            assert!(
                !check_circuit(
                    &Dot {
                        attack,
                        ..fixture.clone()
                    },
                    11,
                    &[],
                    CheckMode::Strict
                )
                .unwrap()
                .is_satisfied()
            );
        }
    }
}
#[test]
fn unsigned_dot_boundaries_and_all_cells_fp() {
    cases::<Fp>(false);
}
#[test]
fn unsigned_dot_boundaries_and_all_cells_fq() {
    cases::<Fq>(false);
}
#[test]
fn unsigned_dot_exact_integer_bound_obligations() {
    let radix = Nat::pow2(LIMB_BITS);
    let coefficient = radix.wrapping_mul(&radix).wrapping_mul(&Nat::from_u64(28));
    let carry = radix.wrapping_mul(&Nat::from_u64(29));
    assert!(carry.cmp_vartime(&Nat::pow2(92)).is_lt());
    assert!(
        coefficient
            .wrapping_add(&carry)
            .cmp_vartime(&carry.shl(LIMB_BITS))
            .is_lt()
    );
    for modulus in [
        ForeignModulus::PASTA_FP,
        ForeignModulus::PASTA_FQ,
        ForeignModulus::P256_BASE,
        ForeignModulus::P256_ORDER,
    ] {
        assert!(modulus.nat().cmp_vartime(&Nat::pow2(254)).is_gt());
        assert!(Nat::pow2(515).cmp_vartime(&modulus.nat().shl(261)).is_lt());
        let input = Nat::pow2(256).wrapping_sub(&Nat::ONE);
        let sum = input.wrapping_mul(&input).wrapping_mul(&Nat::from_u64(8));
        let (q, c) = sum.div_rem(&modulus.nat()).unwrap();
        assert!(q.cmp_vartime(&Nat::pow2(261)).is_lt());
        assert!(c.cmp_vartime(&modulus.nat()).is_lt());
        let witness = witness::<Fp>(modulus, &vec![(nat_limbs(&input), nat_limbs(&input)); 8]);
        for offset in witness.u {
            assert!(
                Nat::from_words(offset.to_canonical_limbs())
                    .cmp_vartime(&Nat::pow2(105))
                    .is_lt()
            );
        }
    }
    // Both native primes exceed2^254: local residuals below2^194 lift,
    // and global residuals below2^518 cannot reach B^4*p (>2^602).
    assert!(
        Nat::pow2(194)
            .cmp_vartime(&ForeignModulus::PASTA_FP.nat())
            .is_lt()
    );
    assert!(
        Nat::pow2(194)
            .cmp_vartime(&ForeignModulus::PASTA_FQ.nat())
            .is_lt()
    );
    assert!(
        Nat::pow2(518)
            .shr(348)
            .cmp_vartime(&ForeignModulus::PASTA_FP.nat())
            .is_lt()
    );
    assert!(
        Nat::pow2(518)
            .shr(348)
            .cmp_vartime(&ForeignModulus::PASTA_FQ.nat())
            .is_lt()
    );
}

#[test]
fn staged_unsigned_dot_all_fields_moduli_batches_and_cells() {
    cases::<Fp>(true);
    cases::<Fq>(true);
}

fn staged_boundary<F: PastaField>() {
    let mut circuit = Dot::<F> {
        modulus: ForeignModulus::PASTA_FP,
        count: 8,
        attack: Attack::NativeOnly,
        maximum: true,
        staged: true,
        known: true,
        rows: None,
        marker: core::marker::PhantomData,
    };
    let (meta, _) = iroha_plonk::frontend::configure(&circuit).unwrap();
    let end = meta.usable_rows(11).unwrap();
    // NativeOnly deliberately uses inconsistent result/quotient roots; the
    // low convolution is zero, so only the native residue rejects it.
    circuit.rows = Some((end - 19, end));
    assert!(
        !check_circuit(&circuit, 11, &[], CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    circuit.attack = Attack::None;
    // The final three reference constants do not reserve arithmetic rows in
    // this explicit test profile, so the block ends at the final usable row.
    assert!(
        check_circuit(&circuit, 11, &[], CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    for bounds in [(end - 18, end), (end - 18, usize::MAX)] {
        circuit.rows = Some(bounds);
        assert!(synthesize(&circuit, 11, Some(&[])).is_err());
    }
}
#[test]
fn staged_unsigned_dot_final_usable_boundary() {
    staged_boundary::<Fp>();
    staged_boundary::<Fq>();
}
