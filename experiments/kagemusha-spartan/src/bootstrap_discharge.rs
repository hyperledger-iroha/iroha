//! E4 step 1 (research/E4-F1-F2-DESIGN.md §3.4-§3.5): Bootstrap's exact
//! matrix polynomial evaluated inside a circuit, so that a successor can
//! discharge a Bootstrap proof's fresh matrix claim instead of folding it into
//! the recurring polynomial.
//!
//! **Normalization.** The authenticated Bootstrap program's matrix tape
//! (`UniformCircuit::matrix_tape_words`: input slots `r_x`, `r_y`, then the
//! LogUp challenges alpha and beta, then one slot per instruction) is checked
//! again from its public words and split, in tape order, into five instruction
//! families: Add, Sub, Mul, Scale and Const. The normal form is a function of
//! the exact tape and gives the tape back word for word
//! ([`ColdTape::words`]): no reordering, no operand commutation, no constant
//! renumbering. It refuses forward or self references, invalid tags,
//! constant indices or Const padding words, output slots past the tape, an
//! input count other than the program layout's, and instructions whose result
//! nothing reads (the compiler drops those, so a tape with one is not its
//! output).
//!
//! **Evaluator.** One uniform array per family, one instance per instruction:
//!
//! | Family | Cells | Constraint |
//! |---|---|---|
//! | Add | left, right | exports `left + right` |
//! | Sub | left, right | exports `left - right` |
//! | Mul | left, right, out | `left · right = out` |
//! | Scale | left, out; fixed `k` | `left · k = out` |
//! | Const | fixed `k` | exports `k` |
//!
//! Add and Sub carry their linear constraint in the exported combination (the
//! copy entry's value is `left ± right`), so they need no result cell or row.
//! Every input word and every instruction result is exported once to the copy
//! table, and every operand is a copy read (`Wiring::import`), so its address
//! sits in an index column that the copy argument turns into fixed rows. The
//! constants are fixed cells too. Instruction counts, operand addresses,
//! multiplicity widths and padding follow from the tape alone. The program
//! that contains the evaluator therefore fixes the whole tape in its key and
//! fixed values; the prover chooses only values, and the evaluator's own
//! matrix program is a few template rows whatever the tape's length.
//!
//! **Binding and discharge.** A port array holds the verified predecessor's
//! transcript inputs `t_i` (its `r_x`, `r_y`, alpha, beta) and the evaluator
//! inputs `u_i = b · t_i`, `b` the predecessor's Bootstrap discriminator
//! (io62). A check array holds `b`, the predecessor's matrix-combination
//! challenge `r` and the value `v` its scalar verifier checks, reads `A, B, C`
//! from the tape's authenticated output slots, and constrains
//! `b · (v - r A - r^2 B - r^3 C) = 0` and `b` Boolean. With `b = 1` the fresh
//! Bootstrap matrix claim is decided exactly; with `b = 0` the evaluator still
//! runs (on zero inputs) and imposes nothing. Which cells the caller ties to
//! the predecessor verifier, and how the claims are then routed (§3.3, §3.6),
//! is the integration's part: [`DischargeIdx`] names the cells.
// Selected by the recurring P/S circuits in the E4 integration step (full.rs
// predecessor slots, points.rs key selection, deferral.rs hot polynomial).
#![cfg_attr(not(test), allow(dead_code))]

use crate::fs::Scalar;
use crate::full::Wiring;
use crate::uniform::{Array, UniformCircuit, link, own, plus, scaled};

/// The normalization version an authenticated manifest binds with the cold
/// program's identity (§2.3). A change to the family split, the export or
/// import order, or any evaluator row changes it.
pub const NORMALIZATION_VERSION: u32 = 1;

/// Slots (inputs plus instructions) and pool entries a cold tape may have.
pub const MAX_SLOTS: usize = 1 << 24;

const DIGEST_DOMAIN: &[u8] = b"kagemusha-spartan/cold-evaluator/v1\0";

/// A matrix tape in its public-program form. `layout` is the program's
/// `(sx, sz, challenges)`; `words` are the instruction records of
/// `UniformCircuit::export_program` (tag 0 Add, 1 Sub, 2 Mul, 3 Scale with a
/// pool index, 4 Const with a pool index and a zero word).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TapeWords<F> {
    pub layout: (usize, usize, usize),
    pub inputs: usize,
    pub words: Vec<[u32; 3]>,
    pub pool: Vec<F>,
    pub outputs: [u32; 3],
}

/// The five instruction families, in their canonical order.
pub const ADD: usize = 0;
pub const SUB: usize = 1;
pub const MUL: usize = 2;
pub const SCALE: usize = 3;
pub const CONST: usize = 4;
pub const FAMILY_NAMES: [&str; 5] = ["Add", "Sub", "Mul", "Scale", "Const"];

/// One checked instruction over tape slots (Scale and Const name a pool entry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColdOp {
    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Scale(u32, u32),
    Const(u32),
}

impl ColdOp {
    pub fn family(self) -> usize {
        match self {
            ColdOp::Add(..) => ADD,
            ColdOp::Sub(..) => SUB,
            ColdOp::Mul(..) => MUL,
            ColdOp::Scale(..) => SCALE,
            ColdOp::Const(_) => CONST,
        }
    }
    /// The slots it reads, in operand order (a repeated operand twice).
    fn reads(self) -> ([u32; 2], usize) {
        match self {
            ColdOp::Add(a, b) | ColdOp::Sub(a, b) | ColdOp::Mul(a, b) => ([a, b], 2),
            ColdOp::Scale(a, _) => ([a, a], 1),
            ColdOp::Const(_) => ([0, 0], 0),
        }
    }
    fn words(self) -> [u32; 3] {
        match self {
            ColdOp::Add(a, b) => [0, a, b],
            ColdOp::Sub(a, b) => [1, a, b],
            ColdOp::Mul(a, b) => [2, a, b],
            ColdOp::Scale(a, k) => [3, a, k],
            ColdOp::Const(k) => [4, k, 0],
        }
    }
}

/// Where a tape slot's value comes from in the evaluator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Input word `i` (the port's instance `i`).
    Input(usize),
    /// Instance `index` of a family.
    Result { family: usize, index: usize },
}

/// A normalized cold tape: the exact checked instructions and, per family,
/// its instructions' slots in tape order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColdTape<F> {
    layout: (usize, usize, usize),
    inputs: usize,
    ops: Vec<ColdOp>,
    pool: Vec<F>,
    outputs: [u32; 3],
    members: [Vec<u32>; 5],
    /// Each instruction's index within its family.
    place: Vec<u32>,
}

/// What a tape asks of the evaluator.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TapeCensus {
    pub inputs: usize,
    pub instructions: usize,
    pub family: [usize; 5],
    /// Operand reads: two per Add, Sub and Mul, one per Scale.
    pub reads: usize,
    pub pool: usize,
    /// Pool entries no instruction names (left behind by dead-code removal).
    pub unused_pool: usize,
    /// The most reads of one slot (outputs included).
    pub max_fanout: usize,
    /// Input words no instruction or output reads.
    pub unread_inputs: usize,
}

impl<F: Scalar> ColdTape<F> {
    /// Check a tape and normalize it. Nothing in `t` is trusted.
    pub(crate) fn normalize(t: TapeWords<F>) -> Result<Self, String> {
        let TapeWords {
            layout,
            inputs,
            words,
            pool,
            outputs,
        } = t;
        let (sx, sz, challenges) = layout;
        if sx == 0 || sz == 0 || !matches!(challenges, 0 | 2) || inputs != sx + sz + challenges {
            return Err("cold tape input layout".into());
        }
        let slots = inputs
            .checked_add(words.len())
            .filter(|&n| n <= MAX_SLOTS)
            .ok_or("cold tape slot bound")?;
        if pool.len() > MAX_SLOTS {
            return Err("cold tape constant pool bound".into());
        }
        let mut ops = Vec::with_capacity(words.len());
        for (i, &[tag, a, b]) in words.iter().enumerate() {
            let before = inputs + i;
            let slot = |v: u32| (v as usize) < before;
            let constant = |v: u32| (v as usize) < pool.len();
            ops.push(match tag {
                0 if slot(a) && slot(b) => ColdOp::Add(a, b),
                1 if slot(a) && slot(b) => ColdOp::Sub(a, b),
                2 if slot(a) && slot(b) => ColdOp::Mul(a, b),
                3 if slot(a) && constant(b) => ColdOp::Scale(a, b),
                4 if constant(a) && b == 0 => ColdOp::Const(a),
                _ => {
                    return Err(format!(
                        "cold tape instruction {i}: forward, invalid or noncanonical record {:?}",
                        [tag, a, b]
                    ));
                }
            });
        }
        if outputs.iter().any(|&o| o as usize >= slots) {
            return Err("cold tape output bound".into());
        }
        // Every instruction's result is read: walking down, a slot nothing
        // above it has read is dead.
        let mut live = vec![false; slots];
        for o in outputs {
            live[o as usize] = true;
        }
        for s in (inputs..slots).rev() {
            if !live[s] {
                return Err(format!(
                    "cold tape instruction {}: result never read",
                    s - inputs
                ));
            }
            let (src, k) = ops[s - inputs].reads();
            for a in &src[..k] {
                live[*a as usize] = true;
            }
        }
        let mut members: [Vec<u32>; 5] = Default::default();
        let mut place = Vec::with_capacity(ops.len());
        for (i, op) in ops.iter().enumerate() {
            let f = op.family();
            place.push(members[f].len() as u32);
            members[f].push((inputs + i) as u32);
        }
        Ok(ColdTape {
            layout,
            inputs,
            ops,
            pool,
            outputs,
            members,
            place,
        })
    }

    /// The normalized tape of a program's compiled matrix evaluation (for
    /// E4, the authenticated Bootstrap program of one side).
    pub fn from_program(c: &UniformCircuit<F>) -> Result<Self, String> {
        Self::normalize(c.matrix_tape_words())
    }

    /// The exact tape back, word for word.
    pub(crate) fn words(&self) -> TapeWords<F> {
        TapeWords {
            layout: self.layout,
            inputs: self.inputs,
            words: self.ops.iter().map(|op| op.words()).collect(),
            pool: self.pool.clone(),
            outputs: self.outputs,
        }
    }

    /// `(sx, sz, challenges)`.
    pub fn layout(&self) -> (usize, usize, usize) {
        self.layout
    }
    /// Input words: `r_x`, `r_y`, then alpha and beta when the program has them.
    pub fn inputs(&self) -> usize {
        self.inputs
    }
    pub fn ops(&self) -> &[ColdOp] {
        &self.ops
    }
    pub fn outputs(&self) -> [u32; 3] {
        self.outputs
    }
    /// The slots of family `f`'s instructions, in tape order.
    pub fn members(&self, f: usize) -> &[u32] {
        &self.members[f]
    }

    /// Where slot `s`'s value is in the evaluator.
    pub fn source(&self, s: u32) -> Source {
        let s = s as usize;
        if s < self.inputs {
            Source::Input(s)
        } else {
            Source::Result {
                family: self.ops[s - self.inputs].family(),
                index: self.place[s - self.inputs] as usize,
            }
        }
    }

    /// The identity of the evaluator this tape defines: the normalization
    /// version, the layout and every word of the tape.
    pub fn digest(&self) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(DIGEST_DOMAIN);
        let word = |h: &mut blake3::Hasher, v: u64| {
            h.update(&v.to_le_bytes());
        };
        word(&mut h, NORMALIZATION_VERSION as u64);
        for v in [
            self.layout.0,
            self.layout.1,
            self.layout.2,
            self.inputs,
            self.ops.len(),
            self.pool.len(),
        ] {
            word(&mut h, v as u64);
        }
        for op in &self.ops {
            for v in op.words() {
                h.update(&v.to_le_bytes());
            }
        }
        for c in &self.pool {
            h.update(&c.to_repr());
        }
        for o in self.outputs {
            h.update(&o.to_le_bytes());
        }
        *h.finalize().as_bytes()
    }

    /// Every slot's value at the input words `u`.
    pub fn slot_values(&self, u: &[F]) -> Vec<F> {
        assert_eq!(u.len(), self.inputs, "one value per input word");
        let mut v = Vec::with_capacity(self.inputs + self.ops.len());
        v.extend_from_slice(u);
        for op in &self.ops {
            let x = match *op {
                ColdOp::Add(a, b) => v[a as usize] + v[b as usize],
                ColdOp::Sub(a, b) => v[a as usize] - v[b as usize],
                ColdOp::Mul(a, b) => v[a as usize] * v[b as usize],
                ColdOp::Scale(a, k) => v[a as usize] * self.pool[k as usize],
                ColdOp::Const(k) => self.pool[k as usize],
            };
            v.push(x);
        }
        v
    }

    /// `(A, B, C)` at the input words `u` (native reference).
    pub fn eval(&self, u: &[F]) -> (F, F, F) {
        let v = self.slot_values(u);
        let o = self.outputs.map(|o| v[o as usize]);
        (o[0], o[1], o[2])
    }

    pub fn census(&self) -> TapeCensus {
        let mut fanout = vec![0usize; self.inputs + self.ops.len()];
        let mut used = vec![false; self.pool.len()];
        let mut reads = 0;
        for op in &self.ops {
            let (src, k) = op.reads();
            reads += k;
            for a in &src[..k] {
                fanout[*a as usize] += 1;
            }
            if let ColdOp::Scale(_, c) | ColdOp::Const(c) = *op {
                used[c as usize] = true;
            }
        }
        for o in self.outputs {
            fanout[o as usize] += 1;
        }
        TapeCensus {
            inputs: self.inputs,
            instructions: self.ops.len(),
            family: std::array::from_fn(|f| self.members[f].len()),
            reads,
            pool: self.pool.len(),
            unused_pool: used.iter().filter(|u| !**u).count(),
            max_fanout: fanout.iter().copied().max().unwrap_or(0),
            unread_inputs: fanout[..self.inputs].iter().filter(|n| **n == 0).count(),
        }
    }
}

/// The values of one discharge: whether the verified predecessor is
/// Bootstrap, its transcript inputs in tape order (`r_x`, `r_y`, alpha,
/// beta), its matrix-combination challenge `r` and the value `v` its scalar
/// verifier checks (`r A + r^2 B + r^3 C` for an honest proof).
#[derive(Clone, Debug)]
pub struct ColdWitness<F> {
    pub base: bool,
    pub transcript: Vec<F>,
    pub r: F,
    pub value: F,
}

/// Port cells, instance `i` per input word.
pub const PORT_T: usize = 0;
pub const PORT_U: usize = 1;
/// Check cells (one instance).
pub const CK_B: usize = 0;
pub const CK_R: usize = 1;
pub const CK_V: usize = 2;
pub const CK_R2: usize = 3;
pub const CK_R3: usize = 4;
pub const CK_A: usize = 5;
pub const CK_BB: usize = 6;
pub const CK_C: usize = 7;
pub const CK_PA: usize = 8;
pub const CK_PB: usize = 9;
pub const CK_PC: usize = 10;
/// Family cells: the operands, then Mul's and Scale's result.
pub const OP_L: usize = 0;
pub const OP_R: usize = 1;
pub const MUL_O: usize = 2;
pub const SCALE_O: usize = 1;

/// Where a discharge's parts sit (array indices before the build).
#[derive(Clone, Debug)]
pub struct DischargeIdx {
    /// Cells `PORT_T`, `PORT_U`; the caller ties `PORT_T` to the
    /// predecessor's transcript inputs.
    pub port: usize,
    /// Cells `CK_*`; the caller ties `CK_B`, `CK_R` and `CK_V` to the
    /// predecessor verifier's io62, `r` and value.
    pub check: usize,
    /// Each family's instruction array (`None` for an empty family). Const's
    /// is its cell-less export carrier.
    pub families: [Option<usize>; 5],
    /// The fixed constant arrays of Scale and Const.
    pub constants: [Option<usize>; 2],
    /// Export sites: the port's, then each family's.
    pub port_export: usize,
    pub exports: [Option<usize>; 5],
}

fn bits(n: usize) -> usize {
    n.next_power_of_two().trailing_zeros() as usize
}

/// Append the evaluator of `tape` and its discharge equality to `w`. `gfam`
/// is a global family (no p range) holding the port and check arrays.
pub fn append<F: Scalar>(
    w: &mut Wiring<F>,
    gfam: usize,
    tape: &ColdTape<F>,
    wit: &ColdWitness<F>,
) -> DischargeIdx {
    let n_in = tape.inputs;
    assert_eq!(
        wit.transcript.len(),
        n_in,
        "one transcript word per tape input"
    );
    assert!(
        w.pi[gfam] == 0 && w.counts[gfam] == 1,
        "the port and check arrays are global"
    );
    let one = F::ONE;
    let b = if wit.base { F::ONE } else { F::ZERO };
    let u: Vec<F> = wit.transcript.iter().map(|t| b * t).collect();
    let vals = tape.slot_values(&u);
    let [oa, ob, oc] = tape.outputs.map(|o| vals[o as usize]);
    let (r, r2) = (wit.r, wit.r.square());
    let r3 = r2 * r;
    let cell = |k: usize| own::<F>(k, 1);
    let check = w.push(Array {
        name: "cold discharge",
        family: gfam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![1, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255],
        rows: vec![
            [cell(CK_B), cell(CK_B), cell(CK_B)],
            [cell(CK_R), cell(CK_R), cell(CK_R2)],
            [cell(CK_R2), cell(CK_R), cell(CK_R3)],
            [cell(CK_R), cell(CK_A), cell(CK_PA)],
            [cell(CK_R2), cell(CK_BB), cell(CK_PB)],
            [cell(CK_R3), cell(CK_C), cell(CK_PC)],
            [
                cell(CK_B),
                plus(
                    &cell(CK_V),
                    &scaled(&plus(&plus(&cell(CK_PA), &cell(CK_PB)), &cell(CK_PC)), -one),
                ),
                Vec::new(),
            ],
        ],
        values: [
            b,
            r,
            wit.value,
            r2,
            r3,
            oa,
            ob,
            oc,
            r * oa,
            r2 * ob,
            r3 * oc,
        ]
        .into_iter()
        .map(|v| vec![v])
        .collect(),
    });
    let rp = bits(n_in);
    let np = 1usize << rp;
    let pad = |v: &[F]| {
        let mut v = v.to_vec();
        v.resize(np, F::ZERO);
        v
    };
    let port = w.push(Array {
        name: "cold port",
        family: gfam,
        rho: rp,
        r_count: n_in,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255, 255],
        rows: vec![[
            link(check, CK_B, vec![Some(0); np], one),
            own(PORT_T, np),
            own(PORT_U, np),
        ]],
        values: vec![pad(&wit.transcript), pad(&u)],
    });
    let port_export = w.export(port, vec![own(PORT_U, np)]);
    // The families and their exports first: an operand may name any family.
    let mut families = [None; 5];
    let mut constants = [None; 2];
    let mut exports = [None; 5];
    for f in 0..5 {
        let slots_of = &tape.members[f];
        let n = slots_of.len();
        if n == 0 {
            continue;
        }
        let fam = w.family(bits(n), n);
        let size = 1usize << bits(n);
        let column = |value: &dyn Fn(ColdOp, u32) -> F| -> Vec<F> {
            let mut c: Vec<F> = slots_of
                .iter()
                .map(|&s| value(tape.ops[s as usize - n_in], s))
                .collect();
            c.resize(size, F::ZERO);
            c
        };
        let left = |op: ColdOp, _: u32| vals[op.reads().0[0] as usize];
        let right = |op: ColdOp, _: u32| vals[op.reads().0[1] as usize];
        let result = |_: ColdOp, s: u32| vals[s as usize];
        let pooled = |op: ColdOp, _: u32| match op {
            ColdOp::Scale(_, k) | ColdOp::Const(k) => tape.pool[k as usize],
            _ => unreachable!("a pooled family"),
        };
        let fixed = |w: &mut Wiring<F>, name: &'static str| {
            w.push(Array {
                name,
                family: fam,
                rho: 0,
                r_count: 1,
                fixed: true,
                lookups: Vec::new(),
                widths: vec![255],
                rows: Vec::new(),
                values: vec![column(&pooled)],
            })
        };
        let (array, tuple) = match f {
            ADD | SUB => {
                let a = w.push(Array {
                    name: if f == ADD { "cold add" } else { "cold sub" },
                    family: fam,
                    rho: 0,
                    r_count: 1,
                    fixed: false,
                    lookups: Vec::new(),
                    widths: vec![255, 255],
                    rows: Vec::new(),
                    values: vec![column(&left), column(&right)],
                });
                let sign = if f == ADD { one } else { -one };
                (a, plus(&own(OP_L, 1), &scaled(&own(OP_R, 1), sign)))
            }
            MUL => {
                let a = w.push(Array {
                    name: "cold mul",
                    family: fam,
                    rho: 0,
                    r_count: 1,
                    fixed: false,
                    lookups: Vec::new(),
                    widths: vec![255, 255, 255],
                    rows: vec![[own(OP_L, 1), own(OP_R, 1), own(MUL_O, 1)]],
                    values: vec![column(&left), column(&right), column(&result)],
                });
                (a, own(MUL_O, 1))
            }
            SCALE => {
                let k = fixed(w, "cold scale constants");
                constants[0] = Some(k);
                let a = w.push(Array {
                    name: "cold scale",
                    family: fam,
                    rho: 0,
                    r_count: 1,
                    fixed: false,
                    lookups: Vec::new(),
                    widths: vec![255, 255],
                    rows: vec![[
                        own(OP_L, 1),
                        link(k, 0, vec![Some(0)], one),
                        own(SCALE_O, 1),
                    ]],
                    values: vec![column(&left), column(&result)],
                });
                (a, own(SCALE_O, 1))
            }
            _ => {
                let k = fixed(w, "cold constants");
                constants[1] = Some(k);
                let a = w.push(Array {
                    name: "cold const",
                    family: fam,
                    rho: 0,
                    r_count: 1,
                    fixed: false,
                    lookups: Vec::new(),
                    widths: Vec::new(),
                    rows: Vec::new(),
                    values: Vec::new(),
                });
                (a, link(k, 0, vec![Some(0)], one))
            }
        };
        families[f] = Some(array);
        exports[f] = Some(w.export(array, vec![tuple]));
    }
    let entry = |s: u32| -> (usize, usize) {
        match tape.source(s) {
            Source::Input(i) => (port_export, i),
            Source::Result { family, index } => (exports[family].expect("a present family"), index),
        }
    };
    for f in [ADD, SUB, MUL, SCALE] {
        let Some(array) = families[f] else { continue };
        let size = w.slots(array);
        for operand in 0..if f == SCALE { 1 } else { 2 } {
            let from = (0..size)
                .map(|j| {
                    tape.members[f]
                        .get(j)
                        .map(|&s| entry(tape.ops[s as usize - n_in].reads().0[operand]))
                })
                .collect();
            w.import(array, vec![own(operand, 1)], from);
        }
    }
    for (k, o) in [CK_A, CK_BB, CK_C].into_iter().zip(tape.outputs) {
        w.import(check, vec![cell(k)], vec![Some(entry(o))]);
    }
    DischargeIdx {
        port,
        check,
        families,
        constants,
        port_export,
        exports,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::r1cs::{Circuit, Instance, Witness};
    use crate::uniform::{Lin, constant};
    use ff::Field;
    use iroha_pasta::{Fp, Fq};

    type Built<F> = (UniformCircuit<F>, Instance<F>, Witness<F>, DischargeIdx, Io);

    /// Row width of the small synthetic evaluators.
    const SMALL_COLS: usize = 10;

    /// Where the test's public claim sits: io index of `t_0`, then `b`, `r`, `v`.
    #[derive(Clone, Copy, Debug)]
    struct Io {
        at: usize,
        inputs: usize,
    }
    impl Io {
        fn t(&self, i: usize) -> usize {
            self.at + i
        }
        fn b(&self) -> usize {
            self.at + self.inputs
        }
        fn v(&self) -> usize {
            self.at + self.inputs + 2
        }
    }

    fn zero_row<F: Field>(lin: Lin<F>, n: usize) -> [Lin<F>; 3] {
        [lin, constant(vec![F::ONE; n]), Vec::new()]
    }

    /// The claim as public io (the stand-in for the predecessor verifier's
    /// cells): rows tie the port's `t` and the check's `b`, `r`, `v` to it.
    fn public_claim<F: Scalar>(
        w: &mut Wiring<F>,
        gfam: usize,
        idx: &DischargeIdx,
        wit: &ColdWitness<F>,
    ) -> Io {
        let n = wit.transcript.len();
        let rho = bits(n + 3);
        let size = 1usize << rho;
        let mut values = wit.transcript.clone();
        values.extend([if wit.base { F::ONE } else { F::ZERO }, wit.r, wit.value]);
        values.resize(size, F::ZERO);
        let io = w.push(Array {
            name: "cold claim io",
            family: gfam,
            rho,
            r_count: n + 3,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255],
            rows: Vec::new(),
            values: vec![values],
        });
        let at = size;
        w.io_arrays.push((io, at));
        let np = 1usize << w.arrays[idx.port].rho;
        let map = (0..np).map(|r| (r < n).then_some(r)).collect();
        w.arrays[idx.port].rows.push(zero_row(
            plus(&own(PORT_T, np), &link(io, 0, map, -F::ONE)),
            np,
        ));
        for (k, slot) in [(CK_B, n), (CK_R, n + 1), (CK_V, n + 2)] {
            w.arrays[idx.check].rows.push(zero_row(
                plus(&own(k, 1), &link(io, 0, vec![Some(slot)], -F::ONE)),
                1,
            ));
        }
        Io { at, inputs: n }
    }

    fn build<F: Scalar>(tape: &ColdTape<F>, wit: &ColdWitness<F>, col_bits: usize) -> Built<F> {
        let mut w = Wiring::<F>::default();
        let gfam = w.family(0, 1);
        let idx = append(&mut w, gfam, tape, wit);
        let io = public_claim(&mut w, gfam, &idx, wit);
        let (arrays, pi, counts, tables, copies) = w.finish();
        let (c, inst, wt) = UniformCircuit::build_with_copies(
            "cold-evaluator",
            arrays,
            pi,
            counts,
            tables,
            copies,
            col_bits,
            2 * io.at,
            b"cold evaluator",
        );
        (c, inst, wt, idx, io)
    }

    /// Whether the witness satisfies every row at the challenges `ch`:
    /// `before` changes it before the second phase is filled (an adversary
    /// that recomputes every helper), `after` once it is.
    fn holds_with<F: Scalar>(
        c: &UniformCircuit<F>,
        inst: &Instance<F>,
        w: &[F],
        ch: &[F],
        before: impl FnOnce(&mut [F]),
        after: impl FnOnce(&mut [F]),
    ) -> bool {
        let mut v = w.to_vec();
        before(&mut v);
        let rel = c.instantiate(&c.shape, ch, Some(&mut v));
        after(&mut v);
        let z = crate::r1cs::z_vector(&c.shape, inst, &Witness { w: v });
        let (a, b, cc) = rel.abc(&c.shape, &z);
        a.iter().zip(&b).zip(&cc).all(|((a, b), c)| *a * *b == *c)
    }

    fn holds<F: Scalar>(b: &Built<F>, ch: &[F]) -> bool {
        holds_with(&b.0, &b.1, &b.2.w, ch, |_| {}, |_| {})
    }

    /// The circuit's cell value.
    fn at<F: Scalar>(b: &Built<F>, a: usize, k: usize, i: usize) -> F {
        b.2.w[b.0.cell_index(a, k, i)]
    }

    /// A random valid tape over `layout` with about `n` instructions: every
    /// family (the first six instructions cycle through them), repeated
    /// operands, operands at both ends of the range, and every result read
    /// (unread ones are summed into the outputs).
    fn random_tape<F: Scalar>(
        rng: &mut SpikeRng,
        layout: (usize, usize, usize),
        n: usize,
    ) -> TapeWords<F> {
        let inputs = layout.0 + layout.1 + layout.2;
        let mut pool: Vec<F> = vec![F::ZERO, F::ONE, -F::ONE];
        pool.extend((0..5).map(|_| rng.scalar::<F>()));
        let mut words: Vec<[u32; 3]> = Vec::new();
        let mut read = vec![false; inputs];
        let pick = |rng: &mut SpikeRng, slots: usize, read: &mut Vec<bool>| -> u32 {
            let byte = rng.scalar::<F>().to_repr()[0] as usize;
            let s = match byte % 4 {
                0 => 0,
                1 => slots - 1,
                _ => (byte.wrapping_mul(2654435761) >> 3) % slots,
            };
            read[s] = true;
            s as u32
        };
        for i in 0..n {
            let byte = rng.scalar::<F>().to_repr()[1] as usize;
            let choice = if i < 6 { i } else { byte % 6 };
            let slots = inputs + words.len();
            let word = match choice {
                0 => [4, (byte % pool.len()) as u32, 0],
                1 => {
                    let a = pick(rng, slots, &mut read);
                    [0, a, a]
                }
                2 => [0, pick(rng, slots, &mut read), pick(rng, slots, &mut read)],
                3 => [1, pick(rng, slots, &mut read), pick(rng, slots, &mut read)],
                4 => {
                    let a = pick(rng, slots, &mut read);
                    let b = if byte % 3 == 0 {
                        a
                    } else {
                        pick(rng, slots, &mut read)
                    };
                    [2, a, b]
                }
                _ => [
                    3,
                    pick(rng, slots, &mut read),
                    ((byte / 6) % pool.len()) as u32,
                ],
            };
            words.push(word);
            read.push(false);
        }
        // Fold every unread result into one chain: its end is A and B.
        let unread: Vec<u32> = (inputs..inputs + words.len())
            .filter(|&s| !read[s])
            .map(|s| s as u32)
            .collect();
        let mut acc = unread[0];
        for &s in &unread[1..] {
            words.push([0, acc, s]);
            acc = (inputs + words.len() - 1) as u32;
        }
        TapeWords {
            layout,
            inputs,
            words,
            pool,
            outputs: [acc, acc, 0],
        }
    }

    /// Every family with repeated operands, an input as an output, a Sub
    /// of equal slots and a Const read twice.
    fn all_families<F: Scalar>() -> TapeWords<F> {
        // Inputs 0..7: r_x (2), r_y (3), alpha, beta.
        TapeWords {
            layout: (2, 3, 2),
            inputs: 7,
            words: vec![
                [4, 1, 0],   // 7: pool[1]
                [2, 0, 5],   // 8: r_x0 alpha
                [0, 8, 8],   // 9: 2 r_x0 alpha
                [1, 9, 7],   // 10
                [3, 10, 0],  // 11: 10 * pool[0]
                [2, 6, 6],   // 12: beta^2
                [1, 12, 12], // 13: 0
                [0, 11, 13], // 14
                [2, 14, 7],  // 15
                [0, 15, 7],  // 16
                [2, 3, 1],   // 17: r_y0 r_x1
                [1, 17, 2],  // 18
                [3, 18, 2],  // 19
                [0, 16, 19], // 20
                [2, 20, 4],  // 21
            ],
            pool: vec![F::from(5), F::from(7) - F::from(19), F::from(3)],
            outputs: [21, 4, 16],
        }
    }

    fn challenges<F: Scalar>(rng: &mut SpikeRng) -> Vec<F> {
        vec![rng.scalar(), rng.scalar()]
    }

    fn honest<F: Scalar>(tape: &ColdTape<F>, t: Vec<F>, r: F) -> ColdWitness<F> {
        let (a, b, c) = tape.eval(&t);
        ColdWitness {
            base: true,
            transcript: t,
            r,
            value: r * a + r.square() * b + r.square() * r * c,
        }
    }

    fn parity<F: Scalar>(seed: &[u8]) {
        let mut rng = SpikeRng::new(seed);
        let mut tapes = vec![ColdTape::<F>::normalize(all_families()).unwrap()];
        for (layout, n) in [((2, 3, 2), 20), ((3, 4, 0), 40), ((5, 6, 2), 300)] {
            tapes.push(ColdTape::normalize(random_tape(&mut rng, layout, n)).unwrap());
        }
        for tape in &tapes {
            let census = tape.census();
            assert!(census.family.iter().all(|n| *n > 0), "{census:?}");
            let mut points: Vec<Vec<F>> =
                vec![vec![F::ZERO; tape.inputs()], vec![F::ONE; tape.inputs()]];
            points.extend((0..3).map(|_| (0..tape.inputs()).map(|_| rng.scalar()).collect()));
            for t in points {
                let wit = honest(tape, t.clone(), rng.scalar());
                let b = build(tape, &wit, SMALL_COLS);
                let ch = challenges(&mut rng);
                assert!(holds(&b, &ch), "the honest evaluator holds");
                let (a, bb, c) = tape.eval(&t);
                let check = b.3.check;
                assert_eq!(
                    (
                        at(&b, check, CK_A, 0),
                        at(&b, check, CK_BB, 0),
                        at(&b, check, CK_C, 0)
                    ),
                    (a, bb, c),
                    "the evaluator's outputs are the native evaluation"
                );
                for (i, ti) in t.iter().enumerate() {
                    assert_eq!(at(&b, b.3.port, PORT_U, i), *ti);
                }
            }
        }
    }

    /// Evaluator outputs equal native evaluation in both fields: every
    /// opcode, zero and one inputs, repeated operands, boundary operands.
    #[test]
    fn evaluator_matches_native_tape_in_both_fields() {
        parity::<Fp>(b"cold parity fp");
        parity::<Fq>(b"cold parity fq");
    }

    /// The normal form is the exact tape (words back unchanged, one digest),
    /// any single-word change gives another digest, the evaluator's public
    /// program does not depend on the witness (active, inactive, any point),
    /// and the evaluator's own compiled tape normalizes and agrees with it.
    #[test]
    fn normalization_is_canonical_and_exact() {
        fn run<F: Scalar>(seed: &[u8]) {
            let mut rng = SpikeRng::new(seed);
            for words in [all_families::<F>(), random_tape(&mut rng, (3, 4, 2), 120)] {
                let tape = ColdTape::normalize(words.clone()).unwrap();
                assert_eq!(tape.words(), words, "the normal form gives the tape back");
                assert_eq!(tape.outputs(), words.outputs);
                assert_eq!(ColdTape::normalize(tape.words()).unwrap(), tape);
                let digest = tape.digest();
                assert_eq!(ColdTape::normalize(words.clone()).unwrap().digest(), digest);
                let mut digests = std::collections::HashSet::from([digest]);
                for i in 0..words.words.len() {
                    for j in 1..3 {
                        let mut m = words.clone();
                        m.words[i][j] ^= 1;
                        if let Ok(t) = ColdTape::normalize(m) {
                            assert!(
                                digests.insert(t.digest()),
                                "word {i}.{j} is in the identity"
                            );
                        }
                    }
                }
                let mut m = words.clone();
                m.pool[0] += F::ONE;
                assert!(digests.insert(ColdTape::normalize(m).unwrap().digest()));
                let mut m = words.clone();
                m.outputs.swap(0, 2);
                assert!(digests.insert(ColdTape::normalize(m).unwrap().digest()));
                // Family members are in tape order; every result has one source.
                let mut seen = vec![false; tape.ops().len()];
                for f in 0..5 {
                    let m = tape.members(f);
                    assert!(m.windows(2).all(|p| p[0] < p[1]));
                    for (j, &s) in m.iter().enumerate() {
                        assert_eq!(
                            tape.source(s),
                            Source::Result {
                                family: f,
                                index: j
                            }
                        );
                        seen[s as usize - tape.inputs()] = true;
                    }
                }
                assert!(seen.iter().all(|s| *s));
                // One public program whatever the witness.
                let n = tape.inputs();
                let wits = [
                    honest(&tape, (0..n).map(|_| rng.scalar()).collect(), rng.scalar()),
                    honest(&tape, vec![F::ZERO; n], F::ZERO),
                    ColdWitness {
                        base: false,
                        transcript: (0..n).map(|_| rng.scalar()).collect(),
                        r: rng.scalar(),
                        value: rng.scalar(),
                    },
                ];
                let built: Vec<_> = wits
                    .iter()
                    .map(|wit| build(&tape, wit, SMALL_COLS))
                    .collect();
                let program = built[0].0.program_digest();
                for b in &built[1..] {
                    assert_eq!(
                        b.0.program_digest(),
                        program,
                        "active and inactive witnesses: one program"
                    );
                }
                // A changed tape changes the containing program's identity
                // (its fixed constants and address columns).
                let mut changed = words.clone();
                let i = changed.words.iter().rposition(|w| w[0] <= 2).unwrap();
                changed.words[i][0] = if changed.words[i][0] == 0 { 1 } else { 0 };
                let changed = ColdTape::normalize(changed).unwrap();
                assert_ne!(changed.digest(), digest);
                assert_ne!(
                    build(&changed, &wits[1], SMALL_COLS).0.program_digest(),
                    program
                );
                let mut constant = words.clone();
                let k = match words.words.iter().find(|w| w[0] >= 3).unwrap() {
                    [3, _, k] => *k,
                    [_, k, _] => *k,
                };
                constant.pool[k as usize] += F::ONE;
                let constant = ColdTape::normalize(constant).unwrap();
                assert_ne!(
                    build(&constant, &wits[1], SMALL_COLS).0.program_digest(),
                    program
                );
                // The evaluator's own matrix tape is a valid uniform tape.
                let c = &built[0].0;
                let own_tape = ColdTape::from_program(c).unwrap();
                for _ in 0..2 {
                    let rx: Vec<F> = (0..c.shape.sx).map(|_| rng.scalar()).collect();
                    let ry: Vec<F> = (0..c.shape.sz()).map(|_| rng.scalar()).collect();
                    let ch = challenges::<F>(&mut rng);
                    let u: Vec<F> = rx.iter().chain(&ry).chain(&ch).copied().collect();
                    assert_eq!(own_tape.eval(&u), c.evals(&rx, &ry, &ch));
                }
            }
        }
        run::<Fp>(b"cold canonical fp");
        run::<Fq>(b"cold canonical fq");
    }

    /// Structural refusals: layout, forward and self references, tags,
    /// constant indices, Const padding, outputs and unread results.
    #[test]
    fn malformed_tapes_are_refused() {
        let good = all_families::<Fp>();
        assert!(ColdTape::normalize(good.clone()).is_ok());
        let refused = |m: TapeWords<Fp>, what: &str| {
            let e = ColdTape::normalize(m).expect_err(what);
            eprintln!("refused, {what}: {e}");
        };
        let edit = |f: &dyn Fn(&mut TapeWords<Fp>)| {
            let mut m = good.clone();
            f(&mut m);
            m
        };
        refused(edit(&|m| m.inputs = 6), "inputs below the layout");
        refused(edit(&|m| m.layout = (2, 3, 1)), "one challenge");
        refused(
            edit(&|m| {
                m.layout = (0, 5, 2);
            }),
            "no r_x",
        );
        refused(edit(&|m| m.words[1] = [2, 8, 5]), "self reference");
        refused(edit(&|m| m.words[1] = [2, 0, 9]), "forward reference");
        refused(edit(&|m| m.words[1] = [5, 0, 1]), "unknown tag");
        refused(
            edit(&|m| m.words[4] = [3, 10, 3]),
            "Scale constant past the pool",
        );
        refused(edit(&|m| m.words[0] = [4, 3, 0]), "Const past the pool");
        refused(edit(&|m| m.words[0] = [4, 1, 1]), "Const padding word");
        refused(
            edit(&|m| m.words[0] = [4, u32::MAX, 0]),
            "Const index overflow",
        );
        refused(edit(&|m| m.words[2] = [0, u32::MAX, 8]), "operand overflow");
        refused(edit(&|m| m.outputs[1] = 22), "output past the tape");
        refused(edit(&|m| m.outputs[0] = 20), "an unread instruction");
        refused(
            edit(&|m| m.words.push([0, 0, 1])),
            "an unread last instruction",
        );
        refused(edit(&|m| m.words.clear()), "outputs past an empty tape");
        // A self-consistent empty tape (outputs read inputs) is a tape.
        assert!(
            ColdTape::normalize(edit(&|m| {
                m.words.clear();
                m.outputs = [0, 6, 3];
            }))
            .is_ok()
        );
    }

    /// False claims and forged witnesses are refused; `b = 0` imposes no
    /// equality; `b` must be Boolean.
    #[test]
    fn false_claims_and_forgeries_are_unsatisfiable() {
        fn run<F: Scalar>(seed: &[u8]) {
            let mut rng = SpikeRng::new(seed);
            let tape = ColdTape::<F>::normalize(random_tape(&mut rng, (3, 4, 2), 60)).unwrap();
            let n = tape.inputs();
            let t: Vec<F> = (0..n).map(|_| rng.scalar()).collect();
            let wit = honest(&tape, t.clone(), rng.scalar());
            let ch = challenges::<F>(&mut rng);
            let good = build(&tape, &wit, SMALL_COLS);
            assert!(holds(&good, &ch));
            let (c, inst, w, idx, io) = (&good.0, &good.1, &good.2.w, &good.3, good.4);
            let none = |_: &mut [F]| {};
            let with_io = |slot: usize, delta: F| {
                let mut i = Instance {
                    io: inst.io.clone(),
                };
                i.io[slot] += delta;
                holds_with(c, &i, w, &ch, none, none)
            };
            // A false value or point in the public claim.
            assert!(!with_io(io.v(), F::ONE), "false claim value");
            for i in [0, n / 2, n - 1] {
                assert!(!with_io(io.t(i), F::ONE), "false point coordinate {i}");
            }
            // An honest evaluation with a false value: built, never satisfied.
            let mut lie = wit.clone();
            lie.value += F::ONE;
            let lying = build(&tape, &lie, SMALL_COLS);
            assert!(!holds(&lying, &ch), "built with a false value");
            // A witness for another point fails the claim's point.
            let mut moved_t = t.clone();
            moved_t[1] += F::ONE;
            let other = build(&tape, &honest(&tape, moved_t, wit.r), SMALL_COLS);
            assert!(holds(&other, &ch));
            assert!(
                !holds_with(&other.0, inst, &other.2.w, &ch, none, none),
                "the claim's point binds the evaluator's inputs"
            );
            // Forge A so that the false value passes the equality (helpers
            // recomputed): the copy read of A refuses it.
            let forge = {
                let cell = |k| lying.0.cell_index(lying.3.check, k, 0);
                let (ca, cpa, cr, cv, cpb, cpc) = (
                    cell(CK_A),
                    cell(CK_PA),
                    cell(CK_R),
                    cell(CK_V),
                    cell(CK_PB),
                    cell(CK_PC),
                );
                move |v: &mut [F]| {
                    let a = (v[cv] - v[cpb] - v[cpc]) * v[cr].invert().unwrap();
                    v[ca] = a;
                    v[cpa] = v[cr] * a;
                }
            };
            assert!(!holds_with(
                &lying.0, &lying.1, &lying.2.w, &ch, forge, none
            ));
            // Mutated operand, result, constant, address and copy cells.
            let mutate = |cell: usize| !holds_with(c, inst, w, &ch, |v| v[cell] += F::ONE, none);
            let fam = |f: usize| idx.families[f].unwrap();
            assert!(mutate(c.cell_index(fam(ADD), OP_L, 0)), "Add operand");
            assert!(mutate(c.cell_index(fam(SUB), OP_R, 0)), "Sub operand");
            assert!(mutate(c.cell_index(fam(MUL), OP_L, 0)), "Mul operand");
            assert!(mutate(c.cell_index(fam(MUL), MUL_O, 0)), "Mul result");
            assert!(mutate(c.cell_index(fam(SCALE), SCALE_O, 0)), "Scale result");
            assert!(
                mutate(c.cell_index(idx.constants[0].unwrap(), 0, 0)),
                "Scale constant"
            );
            assert!(
                mutate(c.cell_index(idx.constants[1].unwrap(), 0, 0)),
                "Const constant"
            );
            assert!(mutate(c.cell_index(idx.port, PORT_U, 1)), "input word");
            for site in [
                fam(ADD),
                fam(SUB),
                fam(MUL),
                fam(SCALE),
                fam(CONST),
                idx.port,
            ] {
                let x = c.index_array(site).expect("an index column array");
                assert!(mutate(c.cell_index(x, 0, 0)), "entry address of {site}");
            }
            for site in [fam(ADD), fam(MUL), fam(SCALE), idx.check] {
                let x = c.index_array(site).unwrap();
                let first_read = usize::from(site != idx.check);
                assert!(
                    mutate(c.cell_index(x, first_read, 0)),
                    "read address of {site}"
                );
            }
            let entries = c.export_array(idx.exports[MUL].unwrap());
            assert!(mutate(c.cell_index(entries, 0, 0)), "entry multiplicity");
            let inputs = c.export_array(idx.port_export);
            assert!(
                mutate(c.cell_index(inputs, 0, 2)),
                "input entry multiplicity"
            );
            let helper = c.cell_index(entries, 1, 0);
            assert!(
                !holds_with(c, inst, w, &ch, none, |v| v[helper] += F::ONE),
                "entry helper"
            );
            // b = 0: the evaluator runs on zeros and imposes nothing.
            let off = ColdWitness {
                base: false,
                value: rng.scalar(),
                ..wit.clone()
            };
            let idle = build(&tape, &off, SMALL_COLS);
            assert!(holds(&idle, &ch), "b = 0 decides no Bootstrap equality");
            assert_eq!(idle.0.program_digest(), c.program_digest());
            assert_eq!(
                at(&idle, idle.3.check, CK_A, 0),
                tape.eval(&vec![F::ZERO; n]).0
            );
            // u = b t: an inactive witness cannot feed the transcript in.
            let feed = {
                let cells: Vec<usize> = (0..n)
                    .map(|i| idle.0.cell_index(idle.3.port, PORT_U, i))
                    .collect();
                let t = t.clone();
                move |v: &mut [F]| {
                    for (c, x) in cells.iter().zip(&t) {
                        v[*c] = *x;
                    }
                }
            };
            assert!(!holds_with(&idle.0, &idle.1, &idle.2.w, &ch, feed, none));
            // b = 2 is refused (it would scale the equality).
            let mut two = Instance {
                io: inst.io.clone(),
            };
            two.io[io.b()] = F::from(2);
            let b_cell = c.cell_index(idx.check, CK_B, 0);
            assert!(!holds_with(
                c,
                &two,
                w,
                &ch,
                |v| v[b_cell] = F::from(2),
                none
            ));
        }
        run::<Fp>(b"cold refusals fp");
        run::<Fq>(b"cold refusals fq");
    }

    fn saved_program<F: Scalar>(kind: usize, primary: bool) -> UniformCircuit<F> {
        let directory = std::env::var_os("KAGEMUSHA_SPARTAN_PROGRAMS")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("output/full-backend-run-20261117T051751Z")
            });
        let path = directory.join(format!(
            "research-kind-{kind}-{}.norito",
            if primary { "primary" } else { "secondary" }
        ));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let program = UniformCircuit::import_program(&bytes, primary).unwrap();
        assert!(
            program.export_program().unwrap() == bytes,
            "{}: the same canonical bytes back",
            path.display()
        );
        program
    }

    fn hex(d: &[u8; 32]) -> String {
        d.iter().take(8).map(|b| format!("{b:02x}")).collect()
    }

    /// The real P- and S-Bootstrap tapes of the 11-17 chain: native parity
    /// with the imported program's own evaluation, the evaluator at a random
    /// point, a false value and a false point refused, and the census.
    #[test]
    #[ignore = "requires the saved full-backend-run Bootstrap programs"]
    fn real_bootstrap_tapes_discharge_exactly() {
        fn run<F: Scalar>(primary: bool, col_bits: usize) {
            let side = if primary { "P" } else { "S" };
            let program = saved_program::<F>(2, primary);
            let started = std::time::Instant::now();
            let tape = ColdTape::from_program(&program).unwrap();
            let census = tape.census();
            eprintln!(
                "{side}-Bootstrap tape: layout {:?}, normalized in {:?}, evaluator digest {}; {census:?}",
                tape.layout(),
                started.elapsed(),
                hex(&tape.digest())
            );
            let (pm, ps, pops) = program.plan_stats();
            assert_eq!(
                (
                    census.instructions,
                    census.family[MUL],
                    census.family[SCALE]
                ),
                (pops, pm, ps)
            );
            let (sx, sz, k) = tape.layout();
            let mut rng = SpikeRng::new(b"cold real tapes");
            let mut points: Vec<Vec<F>> = (0..4)
                .map(|_| (0..tape.inputs()).map(|_| rng.scalar()).collect())
                .collect();
            // Boolean r_x, r_y: actual matrix entries (alpha, beta random).
            for corner in 0..2u64 {
                let mut p: Vec<F> = (0..sx + sz)
                    .map(|j| F::from((corner >> (j % 2)) & 1))
                    .collect();
                p.extend((0..k).map(|_| rng.scalar::<F>()));
                points.push(p);
            }
            points.push(vec![F::ZERO; tape.inputs()]);
            for p in &points {
                let (rx, rest) = p.split_at(sx);
                let (ry, ch) = rest.split_at(sz);
                assert_eq!(tape.eval(p), program.evals(rx, ry, ch), "native parity");
            }
            let wit = honest(&tape, points[0].clone(), rng.scalar());
            let started = std::time::Instant::now();
            let built = build(&tape, &wit, col_bits);
            let build_time = started.elapsed();
            let ch = challenges::<F>(&mut rng);
            let started = std::time::Instant::now();
            assert!(holds(&built, &ch), "{side}: the real evaluator holds");
            eprintln!("{side}: satisfiability check in {:?}", started.elapsed());
            let (a, b, c) = tape.eval(&points[0]);
            let check = built.3.check;
            assert_eq!(
                (
                    at(&built, check, CK_A, 0),
                    at(&built, check, CK_BB, 0),
                    at(&built, check, CK_C, 0)
                ),
                (a, b, c)
            );
            let none = |_: &mut [F]| {};
            for slot in [built.4.v(), built.4.t(sx)] {
                let mut false_claim = Instance {
                    io: built.1.io.clone(),
                };
                false_claim.io[slot] += F::ONE;
                assert!(!holds_with(
                    &built.0,
                    &false_claim,
                    &built.2.w,
                    &ch,
                    none,
                    none
                ));
            }
            census_report(side, &program, &tape, &built, build_time);
        }
        run::<Fq>(false, 17);
        run::<Fp>(true, 18);
    }

    /// Rows, cells, lookups and verifier products of a built evaluator.
    fn census_report<F: Scalar>(
        side: &str,
        program: &UniformCircuit<F>,
        tape: &ColdTape<F>,
        b: &Built<F>,
        build_time: std::time::Duration,
    ) {
        let c = &b.0;
        let idx = &b.3;
        let (p1, p2, fixed) = c.regions();
        let (entries, reads) = c.copy_counts();
        let (terms, _) = c.template_size();
        let (pm, ps, pops) = c.plan_stats();
        let mut by_region = [0usize; 3];
        for a in 0..c.arrays.len() {
            for k in 0..c.arrays[a].cells() {
                if c.cell_placed(a, k) {
                    by_region[c.cell_region(a, k)] += c.instances(a);
                }
            }
        }
        eprintln!(
            "{side} evaluator: built in {build_time:?}; shape sx {} row_bits {} col_bits {}; constraint extent {}; rows p1 {p1} pl {} p2 {p2} fixed {fixed} of {}; present cells: first phase {}, second phase {}, fixed {}; copy entries {entries}, copy reads (lookups) {reads}; challenge degrees {:?}; template terms {terms}; verifier program {pops} instructions ({pm} products, {ps} constant scales)",
            c.shape.sx,
            c.shape.row_bits,
            c.shape.col_bits,
            c.constraints,
            c.layout().pl,
            c.shape.rows(),
            by_region[0],
            by_region[1],
            by_region[2],
            c.challenge_degrees(),
        );
        for (f, a) in idx.families.iter().enumerate() {
            if let Some(a) = a {
                let (t, m) = c.array_size(*a);
                eprintln!(
                    "{side} evaluator family {}: {} instances, {t} template terms, {m} products of its own program",
                    FAMILY_NAMES[f],
                    tape.members(f).len()
                );
            }
        }
        let (bm, bs, bops) = program.plan_stats();
        let (bsx, bsz, bdeg) = (
            program.shape.sx,
            program.shape.sz(),
            program.challenge_degrees(),
        );
        eprintln!(
            "{side}-Bootstrap program: sx {bsx} sz {bsz} degrees {bdeg:?}; {bops} instructions ({bm} products, {bs} constant scales); grid {} points",
            3 * (bsx + bsz + bdeg.0 + bdeg.1) + 1
        );
    }

    /// The saved 11-17 programs: each recurring program's room for the
    /// evaluator at its layout, and native q per fold with Bootstrap in the
    /// polynomial against the evaluator in every recurring program instead
    /// (counted instructions and timed program runs at each kind's grid).
    #[test]
    #[ignore = "requires the saved full-backend-run programs"]
    fn saved_programs_fit_and_native_q() {
        fn median(mut v: Vec<f64>) -> f64 {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        }
        fn run<F: Scalar>(primary: bool, col_bits: usize, dims: (usize, usize)) {
            let side = if primary { "P" } else { "S" };
            let names = ["Receive", "Send", "Bootstrap", "Load"];
            let programs: Vec<UniformCircuit<F>> =
                (0..4).map(|k| saved_program::<F>(k, primary)).collect();
            let tape = ColdTape::from_program(&programs[2]).unwrap();
            let mut rng = SpikeRng::new(b"cold fit");
            let wit = honest(
                &tape,
                (0..tape.inputs()).map(|_| rng.scalar()).collect(),
                rng.scalar(),
            );
            let built = build(&tape, &wit, col_bits);
            let ev = &built.0;
            let (p1e, p2e, fe) = ev.regions();
            let mut cells = [0usize; 3];
            for a in 0..ev.arrays.len() {
                for k in 0..ev.arrays[a].cells() {
                    if ev.cell_placed(a, k) {
                        cells[ev.cell_region(a, k)] += ev.instances(a);
                    }
                }
            }
            let active = programs[0].shape.active_cols();
            eprintln!(
                "{side} evaluator needs: constraint extent {}, first-phase cells {} ({:.2} rows of {active} active columns), second-phase {} ({:.2} rows), fixed {} ({:.2} rows); standalone rows p1 {p1e} p2 {p2e} fixed {fe}",
                ev.constraints,
                cells[0],
                cells[0] as f64 / active as f64,
                cells[1],
                cells[1] as f64 / active as f64,
                cells[2],
                cells[2] as f64 / active as f64,
            );
            for (k, c) in programs.iter().enumerate() {
                let (p1, p2, fixed) = c.regions();
                let used = p1 + c.layout().pl + p2 + fixed;
                eprintln!(
                    "{side}-{}: sx {} row_bits {} col_bits {}; constraints {} of {} ({} free); rows p1 {p1} pl {} p2 {p2} fixed {fixed} = {used} of {} ({} free); degrees {:?}; program {:?} (products, scales, instructions)",
                    names[k],
                    c.shape.sx,
                    c.shape.row_bits,
                    c.shape.col_bits,
                    c.constraints,
                    1usize << c.shape.sx,
                    (1usize << c.shape.sx) - c.constraints,
                    c.layout().pl,
                    c.shape.rows(),
                    c.shape.rows() - used,
                    c.challenge_degrees(),
                    c.plan_stats(),
                );
            }
            // Native q: every installed kind runs its program at
            // min(3 (sx + sz + da + db) + 1, D + 1) points of the curve.
            let degree = |kinds: &[(usize, usize, (usize, usize))]| {
                let da = kinds.iter().map(|k| k.2.0).max().unwrap();
                let db = kinds.iter().map(|k| k.2.1).max().unwrap();
                3 * (2 + dims.0 + dims.1) + 9 + 3 * da + 3 * db
            };
            let grid = |k: &(usize, usize, (usize, usize)), d: usize| {
                (3 * (k.0 + k.1 + k.2.0 + k.2.1) + 1).min(d + 1)
            };
            let shape_of =
                |c: &UniformCircuit<F>| (c.shape.sx, c.shape.sz(), c.challenge_degrees());
            let before: Vec<_> = programs.iter().map(shape_of).collect();
            let ed = ev.challenge_degrees();
            let after: Vec<_> = [0, 1, 3]
                .iter()
                .map(|&k| {
                    let (sx, sz, d) = before[k];
                    (sx, sz, (d.0.max(ed.0), d.1.max(ed.1)))
                })
                .collect();
            let (d_before, d_after) = (degree(&before), degree(&after));
            let points = |c: &UniformCircuit<F>,
                          m: usize,
                          rng: &mut SpikeRng|
             -> Vec<(Vec<F>, Vec<F>, Vec<F>)> {
                (0..m)
                    .map(|_| {
                        (
                            vec![rng.scalar(), rng.scalar()],
                            (0..c.shape.sx).map(|_| rng.scalar()).collect(),
                            (0..c.shape.sz()).map(|_| rng.scalar()).collect(),
                        )
                    })
                    .collect()
            };
            let time = |c: &UniformCircuit<F>, m: usize, rng: &mut SpikeRng| -> f64 {
                let pts = points(c, m, rng);
                let refs: Vec<(&[F], &[F], &[F])> = pts
                    .iter()
                    .map(|(a, b, d)| (&a[..], &b[..], &d[..]))
                    .collect();
                let _ = c.evals_many(&refs);
                median(
                    (0..7)
                        .map(|_| {
                            let t = std::time::Instant::now();
                            std::hint::black_box(c.evals_many(&refs));
                            t.elapsed().as_secs_f64() * 1e3
                        })
                        .collect(),
                )
            };
            let (mut work_before, mut ms_before) = (0usize, 0.0);
            for (k, c) in programs.iter().enumerate() {
                let m = grid(&before[k], d_before);
                let ms = time(c, m, &mut rng);
                work_before += c.plan_stats().2 * m;
                ms_before += ms;
                eprintln!(
                    "{side} native q before, {}: {} points x {} instructions, {ms:.2} ms",
                    names[k],
                    m,
                    c.plan_stats().2
                );
            }
            let (mut work_after, mut ms_after) = (0usize, 0.0);
            for (j, &k) in [0usize, 1, 3].iter().enumerate() {
                let m = grid(&after[j], d_after);
                let (own, extra) = (time(&programs[k], m, &mut rng), time(ev, m, &mut rng));
                work_after += (programs[k].plan_stats().2 + ev.plan_stats().2) * m;
                ms_after += own + extra;
                eprintln!(
                    "{side} native q after, {}: {} points x ({} + {} evaluator) instructions, {own:.2} + {extra:.2} ms",
                    names[k],
                    m,
                    programs[k].plan_stats().2,
                    ev.plan_stats().2
                );
            }
            eprintln!(
                "{side} native q per fold: deg q {d_before} -> {d_after}; instruction evaluations {work_before} -> {work_after} ({:.1}%); timed program runs {ms_before:.1} -> {ms_after:.1} ms",
                100.0 * work_after as f64 / work_before as f64
            );
        }
        let layout = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("output/full-backend-run-20261117T051751Z/layout.txt"),
        )
        .unwrap();
        eprintln!("saved chain {}", layout.replace('\n', "; "));
        run::<Fq>(false, 17, (24, 24));
        run::<Fp>(true, 18, (24, 25));
    }
}
