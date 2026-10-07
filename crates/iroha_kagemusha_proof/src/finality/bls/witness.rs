//! Bounded native witness preparation for the fixed constrained BLS program.
//!
//! Arkworks only supplies untrusted intermediate values. In particular, this
//! builder deliberately does not return a signature-verification capability:
//! an algebraically invalid signature can produce a trace whose final leaf is
//! unsatisfiable. Every transition and the terminal pairing equation must still
//! be proved by the source-qualified circuits.
use super::{BlsContextWitness, BlsLeafCircuit, BlsLeafPlan, BlsStateWitness, Step};
use ark_bls12_381::{Fq, Fq2, Fq6, Fq12, G1Affine, G2Affine, g1, g2};
use ark_ec::{
    AffineRepr, CurveGroup,
    hashing::{
        curve_maps::{swu::SWUMap, wb::WBConfig},
        map_to_curve_hasher::MapToCurve,
    },
};
use ark_ff::{
    BigInteger, Field, PrimeField,
    fields::field_hashers::{DefaultFieldHasher, HashToField},
};
use ark_serialize::CanonicalDeserialize;
use iroha_plonk_gadgets::bls12_381::{
    curve::{
        G1AffineWitness, G2AffineWitness,
        g1_program::{G1_SUBGROUP_STEPS, G1Step},
        programs::{G2_COFACTOR_STEPS, G2_SUBGROUP_STEPS, G2Step},
    },
    extension::{Fp2, Fp12},
    hash_to_field::W3F_SIGNING_PREFIX,
    native,
    pairing::{
        MillerG2Witness,
        final_exponent::{FINAL_EXPONENT_STEPS, FinalExponentStep},
        miller_program::{MILLER_STEPS, MillerStep},
    },
};
use sha2::Sha256;
type Iso = <g2::Config as WBConfig>::IsogenousCurve;

/// Failure to prepare a bounded witness, never a verification verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlsWitnessError {
    /// Public-key bytes are not a canonical nonidentity native G1 key.
    PublicKey,
    /// Signature bytes are not a canonical nonidentity native G2 signature.
    Signature,
    /// A native map, inverse or nonidentity continuation could not be prepared.
    Arithmetic,
    /// A fixed program ordinal or phase does not match the compiled program.
    Program,
}
impl core::fmt::Display for BlsWitnessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::PublicKey => "invalid native BLS public key",
            Self::Signature => "invalid native BLS signature encoding",
            Self::Arithmetic => "BLS witness arithmetic failed",
            Self::Program => "BLS witness program mismatch",
        })
    }
}
impl std::error::Error for BlsWitnessError {}

/// Prepare exactly [`BlsLeafPlan::LENGTH`] untrusted leaf witnesses from the
/// fixed-size native signing message and compressed key/signature bytes.
///
/// No input controls allocation counts or program selection. A returned trace
/// is **not** evidence that its signature is valid: only the fully composed,
/// source-qualified circuit proof and both generator decisions establish that.
/// # Errors
/// Malformed/identity point encodings, failed native arithmetic, or an internal
/// mismatch between the fixed program and its phase-specific witness shape.
pub fn prepare_bls_witness(
    message: [u8; 165],
    public_key: [u8; 48],
    signature: [u8; 96],
) -> Result<Vec<BlsLeafCircuit>, BlsWitnessError> {
    let key = G1Affine::deserialize_compressed(public_key.as_slice())
        .map_err(|_| BlsWitnessError::PublicKey)?;
    let sig = G2Affine::deserialize_compressed(signature.as_slice())
        .map_err(|_| BlsWitnessError::Signature)?;
    if key.infinity {
        return Err(BlsWitnessError::PublicKey);
    }
    if sig.infinity {
        return Err(BlsWitnessError::Signature);
    }
    let context = BlsContextWitness {
        message,
        public_key,
        signature,
        key_point: w1(key),
        signature_point: w2(sig),
    };
    let mut state = State::Empty;
    let mut leaves = Vec::with_capacity(BlsLeafPlan::LENGTH as usize);
    for cursor in 0..BlsLeafPlan::LENGTH {
        let plan = BlsLeafPlan::at(cursor).ok_or(BlsWitnessError::Program)?;
        let before = state.witness();
        state = transition(plan.step(), &context.message, key, sig, state)?;
        leaves.push(
            BlsLeafCircuit::new(plan, context.clone(), before, state.witness())
                .map_err(|_| BlsWitnessError::Program)?,
        );
    }
    Ok(leaves)
}
fn fp2(value: Fq2) -> Fp2 {
    [value.c0.into_bigint().0, value.c1.into_bigint().0]
}
fn fp6(value: &Fq6) -> [Fp2; 3] {
    [fp2(value.c0), fp2(value.c1), fp2(value.c2)]
}
fn fp12(value: &Fq12) -> Fp12 {
    [fp6(&value.c0), fp6(&value.c1)]
}
fn w1(point: G1Affine) -> G1AffineWitness {
    if point.infinity {
        G1AffineWitness {
            x: native::ZERO,
            y: native::ZERO,
            infinity: true,
        }
    } else {
        G1AffineWitness {
            x: point.x.into_bigint().0,
            y: point.y.into_bigint().0,
            infinity: false,
        }
    }
}
fn w2(point: G2Affine) -> G2AffineWitness {
    if point.infinity {
        G2AffineWitness {
            x: [native::ZERO; 2],
            y: [native::ZERO; 2],
            infinity: true,
        }
    } else {
        G2AffineWitness {
            x: fp2(point.x),
            y: fp2(point.y),
            infinity: false,
        }
    }
}
#[derive(Clone, Copy)]
struct Homogeneous {
    x: Fq2,
    y: Fq2,
    z: Fq2,
}
impl Homogeneous {
    fn from_affine(p: G2Affine) -> Result<Self, BlsWitnessError> {
        if p.infinity {
            return Err(BlsWitnessError::Arithmetic);
        }
        Ok(Self {
            x: p.x,
            y: p.y,
            z: Fq2::ONE,
        })
    }
    fn witness(self) -> MillerG2Witness {
        MillerG2Witness {
            x: fp2(self.x),
            y: fp2(self.y),
            z: fp2(self.z),
        }
    }
}
#[derive(Clone)]
enum State {
    Empty,
    Key([G1Affine; 3]),
    Signature([G2Affine; 6]),
    Fields([Fq2; 2]),
    SwuFirst {
        x: Fq2,
        y: Fq2,
        second: Fq2,
    },
    First {
        point: G2Affine,
        second: Fq2,
    },
    SwuSecond {
        first: G2Affine,
        x: Fq2,
        y: Fq2,
    },
    Points([G2Affine; 2]),
    Cofactor([G2Affine; 6]),
    Miller {
        message_point: G2Affine,
        points: Box<[Homogeneous; 2]>,
        lines: Box<[[Fq2; 3]; 2]>,
        accumulator: Box<Fq12>,
    },
    Final(Box<[Fq12; 5]>),
    Done,
}
impl State {
    fn witness(&self) -> BlsStateWitness {
        use BlsStateWitness as W;
        match self {
            Self::Empty => W::Empty,
            Self::Done => W::Done,
            Self::Key(a) => W::Key(a.map(w1)),
            Self::Signature(a) => W::Signature(a.map(w2)),
            Self::Fields(a) => W::Fields(a.map(fp2)),
            Self::SwuFirst { x, y, second } => W::SwuFirst {
                x: fp2(*x),
                y: fp2(*y),
                second: fp2(*second),
            },
            Self::First { point, second } => W::First {
                point: w2(*point),
                second: fp2(*second),
            },
            Self::SwuSecond { first, x, y } => W::SwuSecond {
                first: w2(*first),
                x: fp2(*x),
                y: fp2(*y),
            },
            Self::Points(a) => W::Points(a.map(w2)),
            Self::Cofactor(a) => W::Cofactor(a.map(w2)),
            Self::Miller {
                message_point,
                points,
                lines,
                accumulator,
            } => W::Miller {
                message_point: w2(*message_point),
                points: Box::new(points.map(Homogeneous::witness)),
                lines: Box::new(lines.map(|line| line.map(fp2))),
                accumulator: Box::new(fp12(accumulator)),
            },
            Self::Final(a) => W::Final(Box::new(a.map(|value| fp12(&value)))),
        }
    }
}
fn psi(point: G2Affine) -> Result<G2Affine, BlsWitnessError> {
    if point.infinity {
        return Ok(point);
    }
    let mut exponent = Fq::MODULUS;
    exponent.sub_with_borrow(&1_u64.into());
    let mut third = [0; 6];
    let mut carry = 0_u128;
    for index in (0..6).rev() {
        let n = (carry << 64) + u128::from(exponent.0[index]);
        third[index] = u64::try_from(n / 3).map_err(|_| BlsWitnessError::Arithmetic)?;
        carry = n % 3;
    }
    exponent.div2();
    let xi = Fq2::new(Fq::ONE, Fq::ONE);
    let x = xi.pow(third).inverse().ok_or(BlsWitnessError::Arithmetic)?;
    let y = xi
        .pow(exponent)
        .inverse()
        .ok_or(BlsWitnessError::Arithmetic)?;
    Ok(G2Affine::new_unchecked(
        point.x.frobenius_map(1) * x,
        point.y.frobenius_map(1) * y,
    ))
}
fn g1_step(step: G1Step, r: &mut [G1Affine; 3]) -> Result<(), BlsWitnessError> {
    match step {
        G1Step::Copy {
            destination,
            source,
        } => r[destination] = r[source],
        G1Step::Double {
            destination,
            source,
        } => r[destination] = (r[source] + r[source]).into_affine(),
        G1Step::Add {
            destination,
            left,
            right,
        } => r[destination] = (r[left] + r[right]).into_affine(),
        G1Step::Negate {
            destination,
            source,
        } => r[destination] = -r[source],
        G1Step::Phi {
            destination,
            source,
        } => r[destination] = g1::endomorphism(&r[source]),
        G1Step::RejectNonidentityFixedPoint => {
            if r[0] == r[1] && !r[0].infinity {
                return Err(BlsWitnessError::Arithmetic);
            }
        }
    }
    Ok(())
}
fn g2_step(step: G2Step, r: &mut [G2Affine; 6]) -> Result<(), BlsWitnessError> {
    match step {
        G2Step::Copy {
            destination,
            source,
        } => r[destination] = r[source],
        G2Step::Double {
            destination,
            source,
        } => r[destination] = (r[source] + r[source]).into_affine(),
        G2Step::Add {
            destination,
            left,
            right,
        } => r[destination] = (r[left] + r[right]).into_affine(),
        G2Step::Negate {
            destination,
            source,
        } => r[destination] = -r[source],
        G2Step::Psi {
            destination,
            source,
        } => r[destination] = psi(r[source])?,
        G2Step::Psi2 {
            destination,
            source,
        } => r[destination] = psi(psi(r[source])?)?,
    }
    Ok(())
}
fn isogeny(x: Fq2, y: Fq2) -> Result<G2Affine, BlsWitnessError> {
    fn polynomial(coefficients: &[Fq2], x: Fq2) -> Fq2 {
        coefficients.iter().rev().fold(Fq2::ZERO, |a, c| a * x + c)
    }
    let map = g2::Config::ISOGENY_MAP;
    let xd = polynomial(map.x_map_denominator, x)
        .inverse()
        .ok_or(BlsWitnessError::Arithmetic)?;
    let yd = polynomial(map.y_map_denominator, x)
        .inverse()
        .ok_or(BlsWitnessError::Arithmetic)?;
    Ok(G2Affine::new_unchecked(
        polynomial(map.x_map_numerator, x) * xd,
        y * polynomial(map.y_map_numerator, x) * yd,
    ))
}
fn double(point: &Homogeneous) -> Result<(Homogeneous, [Fq2; 3]), BlsWitnessError> {
    let Homogeneous { x, y, z } = *point;
    let half = Fq2::new(
        Fq::from(2_u64)
            .inverse()
            .ok_or(BlsWitnessError::Arithmetic)?,
        Fq::ZERO,
    );
    let half_xy = x * y * half;
    let y_squared = y.square();
    let z_squared = z.square();
    let curve_term = Fq2::new(Fq::from(4), Fq::from(4)) * (z_squared + z_squared + z_squared);
    let triple_curve = curve_term + curve_term + curve_term;
    let half_sum = (y_squared + triple_curve) * half;
    let cross_yz = (y + z).square() - y_squared - z_squared;
    let line_constant = curve_term - y_squared;
    let x_squared = x.square();
    let out = Homogeneous {
        x: half_xy * (y_squared - triple_curve),
        y: half_sum.square() - (curve_term.square() + curve_term.square() + curve_term.square()),
        z: y_squared * cross_yz,
    };
    if out.z == Fq2::ZERO {
        return Err(BlsWitnessError::Arithmetic);
    }
    Ok((
        out,
        [line_constant, x_squared + x_squared + x_squared, -cross_yz],
    ))
}
fn add(point: &Homogeneous, addend: G2Affine) -> Result<(Homogeneous, [Fq2; 3]), BlsWitnessError> {
    if addend.infinity {
        return Err(BlsWitnessError::Arithmetic);
    }
    let Homogeneous { x, y, z } = *point;
    let theta = y - addend.y * z;
    let lambda = x - addend.x * z;
    let theta_squared = theta.square();
    let lambda_squared = lambda.square();
    let lambda_cubed = lambda * lambda_squared;
    let z_theta_squared = z * theta_squared;
    let x_lambda_squared = x * lambda_squared;
    let difference = lambda_cubed + z_theta_squared - x_lambda_squared - x_lambda_squared;
    let out = Homogeneous {
        x: lambda * difference,
        y: theta * (x_lambda_squared - difference) - lambda_cubed * y,
        z: z * lambda_cubed,
    };
    if out.z == Fq2::ZERO {
        return Err(BlsWitnessError::Arithmetic);
    }
    Ok((out, [theta * addend.x - lambda * addend.y, -theta, lambda]))
}
fn transition(
    step: Step,
    message: &[u8; 165],
    key: G1Affine,
    signature: G2Affine,
    state: State,
) -> Result<State, BlsWitnessError> {
    use State as S;
    Ok(match (step, state) {
        (Step::Start, S::Empty) => S::Key([key, G1Affine::identity(), G1Affine::identity()]),
        (Step::G1(index), S::Key(mut registers)) => {
            g1_step(
                *G1_SUBGROUP_STEPS
                    .get(index)
                    .ok_or(BlsWitnessError::Program)?,
                &mut registers,
            )?;
            S::Key(registers)
        }
        (Step::StartSignature, S::Key(_)) => {
            let mut r = [G2Affine::identity(); 6];
            r[0] = signature;
            S::Signature(r)
        }
        (Step::G2(index), S::Signature(mut registers)) => {
            g2_step(
                *G2_SUBGROUP_STEPS
                    .get(index)
                    .ok_or(BlsWitnessError::Program)?,
                &mut registers,
            )?;
            S::Signature(registers)
        }
        (Step::HashFields, S::Signature(_)) => {
            let mut exact = W3F_SIGNING_PREFIX.to_vec();
            exact.extend_from_slice(message);
            let hasher = <DefaultFieldHasher<Sha256> as HashToField<Fq2>>::new(&[1]);
            let fields: Vec<Fq2> = hasher.hash_to_field(&exact, 2);
            S::Fields(fields.try_into().map_err(|_| BlsWitnessError::Arithmetic)?)
        }
        (Step::Swu0, S::Fields(fields)) => {
            let map = SWUMap::<Iso>::new().map_err(|_| BlsWitnessError::Arithmetic)?;
            let p = map
                .map_to_curve(fields[0])
                .map_err(|_| BlsWitnessError::Arithmetic)?;
            S::SwuFirst {
                x: p.x,
                y: p.y,
                second: fields[1],
            }
        }
        (Step::Iso0, S::SwuFirst { x, y, second }) => S::First {
            point: isogeny(x, y)?,
            second,
        },
        (Step::Swu1, S::First { point, second }) => {
            let map = SWUMap::<Iso>::new().map_err(|_| BlsWitnessError::Arithmetic)?;
            let p = map
                .map_to_curve(second)
                .map_err(|_| BlsWitnessError::Arithmetic)?;
            S::SwuSecond {
                first: point,
                x: p.x,
                y: p.y,
            }
        }
        (Step::Iso1, S::SwuSecond { first, x, y }) => S::Points([first, isogeny(x, y)?]),
        (Step::StartCofactor, S::Points(points)) => {
            let mut r = [G2Affine::identity(); 6];
            r[0] = (points[0] + points[1]).into_affine();
            S::Cofactor(r)
        }
        (Step::Cofactor(index), S::Cofactor(mut registers)) => {
            g2_step(
                *G2_COFACTOR_STEPS
                    .get(index)
                    .ok_or(BlsWitnessError::Program)?,
                &mut registers,
            )?;
            S::Cofactor(registers)
        }
        (Step::StartMiller, S::Cofactor(registers)) => {
            let h = registers[4];
            S::Miller {
                message_point: h,
                points: Box::new([
                    Homogeneous::from_affine(h)?,
                    Homogeneous::from_affine(signature)?,
                ]),
                lines: Box::new([[Fq2::ZERO; 3]; 2]),
                accumulator: Box::new(Fq12::ONE),
            }
        }
        (
            Step::Miller(index),
            S::Miller {
                message_point,
                mut points,
                mut lines,
                mut accumulator,
            },
        ) => {
            match *MILLER_STEPS.get(index).ok_or(BlsWitnessError::Program)? {
                MillerStep::Square => accumulator.square_in_place(),
                MillerStep::Double { pair } => {
                    (points[pair], lines[pair]) = double(&points[pair])?;
                    accumulator.as_mut()
                }
                MillerStep::Add { pair } => {
                    (points[pair], lines[pair]) =
                        add(&points[pair], [message_point, signature][pair])?;
                    accumulator.as_mut()
                }
                MillerStep::Evaluate { pair } => {
                    let line = lines[pair];
                    let p = [key, -G1Affine::generator()][pair];
                    accumulator.mul_by_014(
                        &line[0],
                        &(line[1] * Fq2::new(p.x, Fq::ZERO)),
                        &(line[2] * Fq2::new(p.y, Fq::ZERO)),
                    );
                    accumulator.as_mut()
                }
                MillerStep::Conjugate => {
                    *accumulator = Fq12::new(accumulator.c0, -accumulator.c1);
                    accumulator.as_mut()
                }
            };
            S::Miller {
                message_point,
                points,
                lines,
                accumulator,
            }
        }
        (Step::StartFinal, S::Miller { accumulator, .. }) => {
            let mut r = [Fq12::ZERO; 5];
            r[0] = *accumulator;
            S::Final(Box::new(r))
        }
        (Step::Final(index), S::Final(mut r)) => {
            match *FINAL_EXPONENT_STEPS
                .get(index)
                .ok_or(BlsWitnessError::Program)?
            {
                FinalExponentStep::Copy {
                    destination,
                    source,
                } => r[destination] = r[source],
                FinalExponentStep::Multiply {
                    destination,
                    left,
                    right,
                } => r[destination] = r[left] * r[right],
                FinalExponentStep::Square {
                    destination,
                    source,
                } => r[destination] = r[source].square(),
                FinalExponentStep::Inverse {
                    destination,
                    source,
                } => r[destination] = r[source].inverse().ok_or(BlsWitnessError::Arithmetic)?,
                FinalExponentStep::Conjugate {
                    destination,
                    source,
                } => r[destination] = Fq12::new(r[source].c0, -r[source].c1),
                FinalExponentStep::Frobenius {
                    destination,
                    source,
                    power,
                } => r[destination] = r[source].frobenius_map(power),
            }
            S::Final(r)
        }
        // A witness is not acceptance. The Finish circuit independently enforces
        // register zero == ONE; a wrong-signature trace remains unsatisfiable.
        (Step::Finish, S::Final(_)) => S::Done,
        _ => return Err(BlsWitnessError::Program),
    })
}
