//! Complete phase-specific state openings. Unused registers do not exist.
use iroha_pasta::Fp;
use iroha_plonk::frontend::{Error, Region, Value};
use iroha_plonk_gadgets::{
    Word,
    bls12_381::{
        curve::{G1AffineWitness, G1Value, G2AffineWitness, G2Value},
        extension::{Fp2, Fp2Value, Fp12, Fp12Value},
        field::Bls381Chip,
        hash_to_curve::SwuG2Value,
        pairing::{MillerG2Witness, miller_program::MillerPairState},
    },
};

/// Untrusted opening of every live register at a fixed program boundary.
/// It is never a verification result or a signature-admission capability.
#[derive(Clone, Debug)]
pub enum BlsStateWitness {
    /// Unique initial boundary, with no caller-chosen registers.
    Empty,
    /// Three G1 subgroup registers.
    Key([G1AffineWitness; 3]),
    /// Six signature subgroup registers.
    Signature([G2AffineWitness; 6]),
    /// Both hash-to-field outputs.
    Fields([Fp2; 2]),
    /// First SWU point and second hash-to-field output.
    SwuFirst {
        /// First SWU point's x coordinate.
        x: Fp2,
        /// First SWU point's y coordinate.
        y: Fp2,
        /// Second hash-to-field output awaiting its map.
        second: Fp2,
    },
    /// First mapped point and second hash-to-field output.
    First {
        /// First point after the isogeny map.
        point: G2AffineWitness,
        /// Second hash-to-field output awaiting its map.
        second: Fp2,
    },
    /// First mapped point and second SWU point.
    SwuSecond {
        /// Retained first mapped point.
        first: G2AffineWitness,
        /// Second SWU point's x coordinate.
        x: Fp2,
        /// Second SWU point's y coordinate.
        y: Fp2,
    },
    /// Both mapped points before cofactor clearing.
    Points([G2AffineWitness; 2]),
    /// Six exact cofactor-clearing registers.
    Cofactor([G2AffineWitness; 6]),
    /// Complete two-pair Miller continuation, including its original message point.
    Miller {
        /// Original cofactor-cleared message point.
        message_point: G2AffineWitness,
        /// Two running points used by the Miller loop.
        points: Box<[MillerG2Witness; 2]>,
        /// Three line coefficients for each running point.
        lines: Box<[[Fp2; 3]; 2]>,
        /// Product accumulated by the Miller loop.
        accumulator: Box<Fp12>,
    },
    /// All five final-exponent registers.
    Final(Box<[Fp12; 5]>),
    /// Unique terminal boundary after the final result equals one.
    Done,
}
impl BlsStateWitness {
    /// Allocate only the fixed phase shape for witnessless source installation.
    /// No arithmetic, point decoding or signature witness generation occurs.
    pub(super) fn for_tag(tag: u64) -> Result<Self, Error> {
        let g1 = G1AffineWitness {
            x: [0; 6],
            y: [0; 6],
            infinity: true,
        };
        let zero = [[0; 6]; 2];
        let g2 = G2AffineWitness {
            x: zero,
            y: zero,
            infinity: true,
        };
        let fp12 = [[zero; 3]; 2];
        Ok(match tag {
            0 => Self::Empty,
            1 => Self::Key([g1; 3]),
            2 => Self::Signature([g2; 6]),
            3 => Self::Fields([zero; 2]),
            4 => Self::SwuFirst {
                x: zero,
                y: zero,
                second: zero,
            },
            5 => Self::First {
                point: g2,
                second: zero,
            },
            6 => Self::SwuSecond {
                first: g2,
                x: zero,
                y: zero,
            },
            7 => Self::Points([g2; 2]),
            8 => Self::Cofactor([g2; 6]),
            9 => Self::Miller {
                message_point: g2,
                points: Box::new(
                    [MillerG2Witness {
                        x: zero,
                        y: zero,
                        z: zero,
                    }; 2],
                ),
                lines: Box::new([[zero; 3]; 2]),
                accumulator: Box::new(fp12),
            },
            10 => Self::Final(Box::new([fp12; 5])),
            11 => Self::Done,
            _ => return Err(Error::Synthesis),
        })
    }
    pub(super) const fn tag(&self) -> u64 {
        match self {
            Self::Empty => 0,
            Self::Key(_) => 1,
            Self::Signature(_) => 2,
            Self::Fields(_) => 3,
            Self::SwuFirst { .. } => 4,
            Self::First { .. } => 5,
            Self::SwuSecond { .. } => 6,
            Self::Points(_) => 7,
            Self::Cofactor(_) => 8,
            Self::Miller { .. } => 9,
            Self::Final(_) => 10,
            Self::Done => 11,
        }
    }
    pub(super) fn words(&self) -> Vec<Fp> {
        let mut out = Vec::new();
        match self {
            Self::Empty | Self::Done => {}
            Self::Key(values) => {
                for p in values {
                    native_g1(&mut out, p);
                }
            }
            Self::Signature(values) | Self::Cofactor(values) => {
                for p in values {
                    native_g2(&mut out, p);
                }
            }
            Self::Fields(values) => {
                for v in values {
                    native_fp2(&mut out, v);
                }
            }
            Self::SwuFirst { x, y, second } => {
                for v in [x, y, second] {
                    native_fp2(&mut out, v);
                }
            }
            Self::First { point, second } => {
                native_g2(&mut out, point);
                native_fp2(&mut out, second);
            }
            Self::SwuSecond { first, x, y } => {
                native_g2(&mut out, first);
                native_fp2(&mut out, x);
                native_fp2(&mut out, y);
            }
            Self::Points(values) => {
                for p in values {
                    native_g2(&mut out, p);
                }
            }
            Self::Miller {
                message_point,
                points,
                lines,
                accumulator,
            } => {
                native_g2(&mut out, message_point);
                for p in points.iter() {
                    for v in [&p.x, &p.y, &p.z] {
                        native_fp2(&mut out, v);
                    }
                }
                for line in lines.iter() {
                    for v in line {
                        native_fp2(&mut out, v);
                    }
                }
                native_fp12(&mut out, accumulator);
            }
            Self::Final(values) => {
                for v in values.iter() {
                    native_fp12(&mut out, v);
                }
            }
        }
        out
    }
    pub(super) fn assign(
        &self,
        chip: &mut Bls381Chip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        known: bool,
    ) -> Result<StateCells, Error> {
        fn v<T: Copy>(value: T, known: bool) -> Value<T> {
            if known {
                Value::known(value)
            } else {
                Value::unknown()
            }
        }
        Ok(match self {
            Self::Empty => StateCells::Empty,
            Self::Done => StateCells::Done,
            Self::Key(a) => StateCells::Key([
                chip.assign_g1(region, v(a[0], known))?,
                chip.assign_g1(region, v(a[1], known))?,
                chip.assign_g1(region, v(a[2], known))?,
            ]),
            Self::Signature(a) | Self::Cofactor(a) => {
                let mut points = Vec::with_capacity(6);
                for p in a {
                    points.push(chip.assign_g2(region, v(*p, known))?);
                }
                let points = points.try_into().map_err(|_| Error::Synthesis)?;
                if matches!(self, Self::Signature(_)) {
                    StateCells::Signature(points)
                } else {
                    StateCells::Cofactor(points)
                }
            }
            Self::Fields(a) => StateCells::Fields([
                chip.assign_fp2(region, v(a[0], known))?,
                chip.assign_fp2(region, v(a[1], known))?,
            ]),
            Self::SwuFirst { x, y, second } => StateCells::SwuFirst {
                point: chip.assign_swu_g2(region, v([*x, *y], known))?,
                second: chip.assign_fp2(region, v(*second, known))?,
            },
            Self::First { point, second } => StateCells::First {
                point: chip.assign_g2(region, v(*point, known))?,
                second: chip.assign_fp2(region, v(*second, known))?,
            },
            Self::SwuSecond { first, x, y } => StateCells::SwuSecond {
                first: chip.assign_g2(region, v(*first, known))?,
                point: chip.assign_swu_g2(region, v([*x, *y], known))?,
            },
            Self::Points(a) => StateCells::Points([
                chip.assign_g2(region, v(a[0], known))?,
                chip.assign_g2(region, v(a[1], known))?,
            ]),
            Self::Miller {
                message_point,
                points,
                lines,
                accumulator,
            } => StateCells::Miller {
                message_point: chip.assign_g2(region, v(*message_point, known))?,
                state: Box::new(MillerPairState::from_parts(
                    [
                        chip.assign_miller_g2(region, v(points[0], known))?,
                        chip.assign_miller_g2(region, v(points[1], known))?,
                    ],
                    [
                        chip.assign_miller_line(region, v(lines[0], known))?,
                        chip.assign_miller_line(region, v(lines[1], known))?,
                    ],
                    chip.assign_fp12(region, v(**accumulator, known))?,
                )),
            },
            Self::Final(a) => {
                let mut values = Vec::with_capacity(5);
                for x in a.iter() {
                    values.push(chip.assign_fp12(region, v(*x, known))?);
                }
                StateCells::Final(Box::new(values.try_into().map_err(|_| Error::Synthesis)?))
            }
        })
    }
}
pub(super) fn native_g1(out: &mut Vec<Fp>, p: &G1AffineWitness) {
    out.extend(p.x.into_iter().chain(p.y).map(Fp::from));
    out.push(Fp::from(u64::from(p.infinity)));
}
pub(super) fn native_g2(out: &mut Vec<Fp>, p: &G2AffineWitness) {
    native_fp2(out, &p.x);
    native_fp2(out, &p.y);
    out.push(Fp::from(u64::from(p.infinity)));
}
fn native_fp2(out: &mut Vec<Fp>, v: &Fp2) {
    out.extend(v.iter().flatten().copied().map(Fp::from));
}
fn native_fp12(out: &mut Vec<Fp>, v: &Fp12) {
    for c6 in v {
        for c2 in c6 {
            native_fp2(out, c2);
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum StateCells {
    Empty,
    Key([G1Value<Fp>; 3]),
    Signature([G2Value<Fp>; 6]),
    Fields([Fp2Value<Fp>; 2]),
    SwuFirst {
        point: SwuG2Value<Fp>,
        second: Fp2Value<Fp>,
    },
    First {
        point: G2Value<Fp>,
        second: Fp2Value<Fp>,
    },
    SwuSecond {
        first: G2Value<Fp>,
        point: SwuG2Value<Fp>,
    },
    Points([G2Value<Fp>; 2]),
    Cofactor([G2Value<Fp>; 6]),
    Miller {
        message_point: G2Value<Fp>,
        state: Box<MillerPairState<Fp>>,
    },
    Final(Box<[Fp12Value<Fp>; 5]>),
    Done,
}
impl StateCells {
    pub(super) fn words(&self) -> Vec<Word<Fp>> {
        let mut out = Vec::new();
        match self {
            Self::Empty | Self::Done => {}
            Self::Key(a) => {
                for p in a {
                    cells_g1(&mut out, p);
                }
            }
            Self::Signature(a) | Self::Cofactor(a) => {
                for p in a {
                    cells_g2(&mut out, p);
                }
            }
            Self::Fields(a) => {
                for v in a {
                    cells_fp2(&mut out, v);
                }
            }
            Self::SwuFirst { point, second } => {
                cells_fp2(&mut out, point.x());
                cells_fp2(&mut out, point.y());
                cells_fp2(&mut out, second);
            }
            Self::First { point, second } => {
                cells_g2(&mut out, point);
                cells_fp2(&mut out, second);
            }
            Self::SwuSecond { first, point } => {
                cells_g2(&mut out, first);
                cells_fp2(&mut out, point.x());
                cells_fp2(&mut out, point.y());
            }
            Self::Points(a) => {
                for p in a {
                    cells_g2(&mut out, p);
                }
            }
            Self::Miller {
                message_point,
                state,
            } => {
                cells_g2(&mut out, message_point);
                for p in state.points() {
                    for v in [p.x(), p.y(), p.z()] {
                        cells_fp2(&mut out, v);
                    }
                }
                for line in state.lines() {
                    for v in line.coefficients() {
                        cells_fp2(&mut out, v);
                    }
                }
                cells_fp12(&mut out, state.accumulator());
            }
            Self::Final(a) => {
                for v in a.iter() {
                    cells_fp12(&mut out, v);
                }
            }
        }
        out
    }
}
pub(super) fn cells_g1(out: &mut Vec<Word<Fp>>, p: &G1Value<Fp>) {
    out.extend(p.x().limbs().iter().chain(p.y().limbs()).cloned());
    out.push(p.infinity().word().clone());
}
pub(super) fn cells_g2(out: &mut Vec<Word<Fp>>, p: &G2Value<Fp>) {
    cells_fp2(out, p.x());
    cells_fp2(out, p.y());
    out.push(p.infinity().word().clone());
}
fn cells_fp2(out: &mut Vec<Word<Fp>>, v: &Fp2Value<Fp>) {
    for c in v.coefficients() {
        out.extend(c.limbs().iter().cloned());
    }
}
fn cells_fp12(out: &mut Vec<Word<Fp>>, v: &Fp12Value<Fp>) {
    for c6 in v.coefficients() {
        for c2 in c6.coefficients() {
            cells_fp2(out, c2);
        }
    }
}
