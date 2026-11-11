//! Recursive census: R1CS cost of the two-curve verifier circuits for the
//! ZK Spartan pair, from this prototype's verifier structure and measured
//! gadget costs (`gadgets.rs`).
//!
//! Design counted (RECURSION.md):
//! - P-proofs (Fp scalars, Vesta points): transcript + group checks in the
//!   secondary circuit C_S (Fq), scalar checks in the primary C_P (Fp).
//!   S-proofs the reverse.
//! - Inherited generator claims fold into the proof's own opening (Halo
//!   accumulation; no restructuring IPA): wallet ids and heads are public, so
//!   claims need no re-randomization (wallet spec line 937; recursion
//!   soundness spec "no stronger privacy or linkability conclusion").
//! - kappa depends on the predecessor's public inputs, so deferred checks stay
//!   in-circuit; the values both circuits need are bound either with no bytes
//!   (masked random linear combination, non-native on the C_P side: "B2") or
//!   by transmitting the public-safe challenges and committing kappa
//!   ("B1": a Pallas commitment recomputed in C_P equals S's dedicated row).

use crate::gadgets::GadgetCosts;
use crate::r1cs::Shape;

/// Non-native multiply-add step in C_P for the B2 random linear combination:
/// 6 limb products + range checks of quotient (128), result (255), carries
/// (3 x 90) + carry equations. An estimate, not a measured gadget.
pub const NONNATIVE_STEP: usize = 700;
/// Uniform-CCS matrix evaluation budget per verified proof (the toy's is ~60
/// multiplications; real circuits add bounded sparse exceptions). The gate-1b
/// census used 2,000; the operation census (OPERATION.md) uses 8,000: about
/// ten uniform block types (non-native check, point operations, RP57, SHA-256
/// round, tree level, lookup rows) whose local matrices are evaluated at the
/// challenge, with wiring in preprocessed index columns opened in the batch.
pub static MATRIX_MLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(2_000);

fn matrix_mle() -> usize {
    MATRIX_MLE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Cost of one cross-circuit binding step in C_P (non-native Horner over Fq),
/// as constraints (including one LogUp row per lookup) and witness entries.
#[derive(Clone, Copy, Debug)]
pub struct StepCost {
    pub rows: usize,
    pub full: usize,
    pub narrow: usize,
}

impl StepCost {
    /// Gate 1b's estimate: 700 constraints, all charged full width.
    pub const ESTIMATE: StepCost = StepCost {
        rows: NONNATIVE_STEP,
        full: NONNATIVE_STEP,
        narrow: 0,
    };
}
/// Public-input field elements of each verified proof besides inherited
/// claims (statement digests, heads, roots, amounts, shared-vector RLC).
pub const STATEMENT_IO: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct ProofShape {
    pub shape: Shape,
    /// Inherited claims folded into this proof's own opening.
    pub inherited: usize,
    /// Claims exposed in this proof's public inputs (folded or restructured).
    pub exported: usize,
}

impl ProofShape {
    fn rounds(&self) -> (usize, usize) {
        (
            self.shape.outer_blocks().len(),
            self.shape.inner_blocks().len(),
        )
    }
    pub fn ipa_rounds(&self) -> usize {
        self.shape.col_bits
    }
    fn points(&self) -> usize {
        let (mo, mi) = self.rounds();
        self.shape.rows() + mo + mi + 2 * self.ipa_rounds() + 2
    }
    /// Challenges: tau, one per round (Kronecker powers), rA/rB/rC (powers),
    /// row-bit endoscalars, alpha, gamma, zeta, xi, IPA u_j, Schnorr c.
    pub fn challenges(&self) -> usize {
        let (mo, mi) = self.rounds();
        1 + mo + 1 + mi + self.shape.row_bits + 4 + self.ipa_rounds() + 1
    }
    fn endoscalar_challenges(&self) -> usize {
        self.shape.row_bits + 3 + self.ipa_rounds() + 1
    }
    pub fn io_elements(&self) -> usize {
        STATEMENT_IO + self.exported * (2 + self.ipa_rounds())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Split {
    /// In the circuit where the proof's points are native.
    pub transcript: usize,
    pub group: usize,
    /// In the circuit where the proof's scalars are native.
    pub scalar: usize,
    pub shared: usize,
    /// One-bit witness entries among the above (challenge, endoscale and
    /// scalar-multiplication bits): points-native circuit, scalar circuit.
    pub bits_points: usize,
    pub bits_scalar: usize,
}

/// Verifier work for one proof, split by circuit (`g` = gadgets of the
/// circuit where its points are native).
pub fn verify_cost(p: &ProofShape, g: &GadgetCosts) -> Split {
    let (mo, mi) = p.rounds();
    let rounds = mo + mi;
    let l = p.ipa_rounds();
    let r = p.shape.rows();
    let k = p.shape.k;

    // Transcript in the group circuit: points as (x, y), proof scalars
    // embedded, public io; rate-2 RP57; two 127-bit challenges per squeeze.
    // The public inputs (sensitive: the predecessor's statement) are hashed
    // to one digest in the scalar circuit; the transcript absorbs the digest
    // and the inherited claims' points (needed natively for the fold).
    let absorbed = 2 * p.points() + 5 + 1 + 2 * p.inherited;
    let squeezes = p.challenges().div_ceil(2);
    let transcript = absorbed.div_ceil(2) * g.poseidon
        + squeezes * (g.poseidon + g.squeeze_bits)
        + (p.points() + p.inherited) * g.on_curve;

    // Group checks: row tensor fold (R-1 endoscales, 2 adds each), Horner
    // over round commitments with gamma, inherited claims with xi, kappa' U,
    // IPA fold (2 endoscales + 2 adds per round), Schnorr
    // z1 G_f + (z1 b_f zeta) U + z2 H = A + c P_final.
    let endoscales = (r - 1) + rounds + p.inherited + 2 * l + 1;
    let adds = 2 * (r - 1) + rounds + p.inherited + 2 * l + 5;
    let group =
        endoscales * g.endoscale_128 + adds * g.point_add + g.var_mul_255 + 3 * g.fixed_mul_255;

    // Scalar checks: endoscalar values from bits, Kronecker powers, the
    // per-slot round weights and kappa (relation_rows), the closed form of
    // b_final and of each inherited claim's <s(u_old), b> (closed_form_rows),
    // the matrix MLE and the io evaluation.
    let scalar = p.endoscalar_challenges() * 130
        + rounds * (k - 1)
        + relation_rows(&p.shape, true)
        + (1 + p.inherited) * closed_form_rows(&p.shape)
        + matrix_mle()
        + 3 * p.io_elements()
        + 8
        + p.io_elements().div_ceil(2) * g.poseidon;

    // Shared values: challenges, kappa' and the Schnorr fixed-base scalar,
    // the five proof scalars, the io digest, and each inherited claim point
    // as four 128-bit limbs (absorbed in the scalar circuit's digest, used
    // natively in the group circuit's fold).
    let bits_points = endoscales * 128 + 255 + 3 * 256 + squeezes * 254;
    let bits_scalar = p.endoscalar_challenges() * 128;
    Split {
        transcript,
        group,
        scalar,
        shared: p.challenges() + 2 + 5 + 1 + 4 * p.exported,
        bits_points,
        bits_scalar,
    }
}

/// Rows of the batched relation's weights and kappa (verifier.rs): alpha
/// powers (the first is free), gamma^-1 and its powers, each slot's two
/// weight factors (slot 0's first is free), one square per round variable the
/// challenge-power chain leaves (the last power of a lean round; every
/// variable of a per-variable round), and kappa's ten products.
pub fn relation_rows(sh: &Shape, lean: bool) -> usize {
    let (ob, ib) = (sh.outer_blocks(), sh.inner_blocks());
    let slots = ob.len() + ib.len();
    let mut squares: usize = 0;
    for &k in &ob {
        squares += if lean { 1 } else { k };
    }
    let mut g = 0;
    for &k in &ib {
        squares += if lean && g > sh.row_bits { 1 } else { k };
        g += k;
    }
    (slots + 1) + 1 + (slots - 1) + (2 * slots - 1) + squares + 10
}

/// Rows of one closed-form opening `<s(u), b>` (verifier.rs closed_form_c):
/// the head's pass over the column bits (r u, the running product, and the
/// tail sum: one product where active's bit is set, two where not; the first
/// bit's are free), the head product, s(u) over each slot's high bits (one
/// product per new prefix of two or more bits), and per slot: the Boolean
/// factors (u_lo tau per outer digit and their chain; the inner chain once
/// per slot size), the Lagrange factors (two products per digit and their
/// chain), both weight products and the high-bit product.
pub fn closed_form_rows(sh: &Shape) -> usize {
    let l = sh.col_bits;
    let active = sh.active_cols();
    let mut rows = 1;
    for (n, j) in (0..l).rev().enumerate() {
        rows += 1;
        if n > 0 {
            rows += 1 + if (active >> (l - 1 - j)) & 1 == 1 {
                1
            } else {
                2
            };
        }
    }
    let mut prefixes = std::collections::HashSet::new();
    let mut inner_sizes = std::collections::HashSet::new();
    for slot in crate::spartan::slots(sh) {
        let k = slot.k;
        let high = l - 2 * sh.k;
        let set: Vec<usize> = (0..high)
            .filter(|j| (slot.offset >> (l - 1 - j)) & 1 == 1)
            .collect();
        for n in 2..=set.len() {
            if prefixes.insert(set[..n].to_vec()) {
                rows += 1;
            }
        }
        if slot.outer {
            rows += k + (k - 1);
        } else if inner_sizes.insert(k) {
            rows += k - 1;
        }
        rows += 2 * k + (k - 1) + 2 + usize::from(!set.is_empty());
    }
    rows
}

/// Verifier cost model: gate 1b's formula, or the structure synthesized in
/// gate 1d (`verifier.rs`, real proofs, every constraint checked).
pub static SYNTHESIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn synthesized() -> bool {
    SYNTHESIZED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Gate 1d's synthesized verifier, as a formula that walks the same schedule:
/// - transcript: the duplex sponge's permutations (absorbing at rate 2, one per
///   squeeze) and one canonical decomposition per squeeze; challenges are one
///   per sumcheck variable, two per squeeze unless an absorption intervenes;
/// - group: endoscaling with existing bits (448), additions with both slope
///   equations (6), the signed-digit variable-base multiplication (1,549),
///   three fixed-base multiplications (520) and their scalars' bits (512 each);
/// - scalar side: endoscalar values (190 each), the io digest, the round
///   weights and the tail, one closed form per opening, the matrix budget;
/// - shared: every challenge, nine full scalars as two limbs, four limbs per
///   inherited claim point.
/// `embed`: the proof's scalars embed in the base field (Vesta proofs).
/// Estimate only (not synthesized): a leaner challenge schedule. tau as powers
/// t^(2^i) of one full-width squeeze (error 2^sx / q); each sumcheck round's
/// challenges as powers of one 126-bit challenge (degree 2 (2^k - 1), error
/// about 2^-121), except the inner rounds holding the row bits, which stay
/// per-variable endoscalars; rA, rB, rC as powers; squeezes that yield only raw
/// challenges range-checked by lookups (about 50 rows) instead of 255 bits.
pub fn lean_schedule() -> bool {
    std::env::var("ZKS_SCHEDULE").as_deref() == Ok("lean")
}

pub fn verify_cost_synth(p: &ProofShape, embed: bool) -> Split {
    if lean_schedule() {
        return verify_cost_lean(p, embed);
    }
    let sh = &p.shape;
    let (ob, ib) = (sh.outer_blocks(), sh.inner_blocks());
    let (mo, mi) = (ob.len(), ib.len());
    let l = p.ipa_rounds();
    let r = sh.rows();
    let w = if embed { 1 } else { 2 };
    let io = p.io_elements();
    // Sponge schedule: a segment absorbing n words from `pending` costs
    // floor((pending + n - 1) / 2) permutations before its squeeze.
    let seg =
        |pending: usize, n: usize| -> usize { if n == 0 { 0 } else { (pending + n - 1) / 2 } };
    let half = |n: usize| n.div_ceil(2);
    let mut perms = seg(1, w + 2 * r);
    let mut squeezes = half(sh.sx);
    for &k in &ob {
        perms += seg(0, 2);
        squeezes += half(k);
    }
    perms += seg(0, 3 * w);
    squeezes += 2;
    for &k in &ib {
        perms += seg(0, 2);
        squeezes += half(k);
    }
    squeezes += 1; // alpha, gamma
    if p.inherited > 0 {
        perms += seg(0, 2 * p.inherited);
        squeezes += 1;
    }
    perms += seg(0, 2 + w);
    squeezes += 1; // zeta
    perms += l * seg(0, 4);
    squeezes += l;
    perms += seg(0, 4);
    squeezes += 1; // c
    let points = r + mo + mi + p.inherited + 2 * l + 2;
    let transcript = (perms + squeezes) * 243 + squeezes * 511 + points * 3;
    let endoscales = (r - 1) + (mo + mi) + p.inherited + 2 * l + 1;
    let adds = 2 * (r - 1) + (mo + mi) + p.inherited + 1 + 2 * l + 3;
    let group = endoscales * 448 + adds * 6 + 1_549 + 3 * 520 + 4 * 512;
    // Scalar side.
    let endo = sh.row_bits + 1 + usize::from(p.inherited > 0) + 1 + l + 1;
    let relation = relation_rows(sh, false);
    let closed = (1 + p.inherited) * closed_form_rows(sh);
    let io_bits = io.next_power_of_two().trailing_zeros() as usize;
    let io_eval = (sh.sw() - io_bits) + ((1 << io_bits) - 2) + 2 * io;
    let scalar = endo * 190 + io.div_ceil(2) * 243 + relation + closed + io_eval + matrix_mle() + 4;
    let challenges = 2 * sh.sx + 3 + sh.sz() + 2 + usize::from(p.inherited > 0) + 1 + l + 1;
    // Range checks in C_P, where the binding is non-native (13-bit lookups):
    // for a P-proof (scalar side in C_P), raw challenges have no bits there,
    // and the nine full scalars need limbs plus canonicity; for an S-proof
    // (points side in C_P), the five absorbed-only scalars need limbs.
    let raw = challenges - endo;
    let (transcript, scalar) = if embed {
        (transcript, scalar + raw * 10 + 9 * 40)
    } else {
        (transcript + 5 * 20, scalar)
    };
    Split {
        transcript: transcript + PAIR_LINK_ROWS,
        group,
        scalar,
        shared: challenges + 2 * 9 + 4 * p.inherited + PAIR_LINK_SHARED,
        bits_points: squeezes * 255 + 4 * 255,
        bits_scalar: endo * 126,
    }
}

/// The pairing link's replay per verified proof (gate 1f, synthesized): one
/// full-width squeeze and four absorbed words (+527 rows on the points side),
/// and six shared limbs per pair (d, rho, R), three per proof on average.
pub const PAIR_LINK_ROWS: usize = 527;
pub const PAIR_LINK_SHARED: usize = 3;

fn verify_cost_lean(p: &ProofShape, embed: bool) -> Split {
    // Gate 1e: the lean schedule as synthesized (verifier.rs, real proofs).
    // Squeezes cost a permutation plus: 511 when decomposed into bits (some
    // half is an endoscalar), 44 when every challenge from it is raw (lookup
    // range checks, counted), 41 for the full-width tau word.
    let sh = &p.shape;
    let (ob, ib) = (sh.outer_blocks(), sh.inner_blocks());
    let (mo, mi) = (ob.len(), ib.len());
    let l = p.ipa_rounds();
    let r = sh.rows();
    let w = if embed { 1 } else { 2 };
    let io = p.io_elements();
    let seg =
        |pending: usize, n: usize| -> usize { if n == 0 { 0 } else { (pending + n - 1) / 2 } };
    let (mut perms, mut bits_sq, mut raw_sq, full_sq) = (seg(1, w + 2 * r), 0usize, 0usize, 1usize);
    let mut challenges = 1; // tau word (two limbs cross)
    for _ in &ob {
        perms += seg(0, 2);
        raw_sq += 1;
        challenges += 1;
    }
    perms += seg(0, 3 * w);
    raw_sq += 1;
    challenges += 1;
    let mut g = 0;
    let mut lean_inner_powers = 0;
    for &k in &ib {
        perms += seg(0, 2);
        if g > sh.row_bits {
            raw_sq += 1;
            challenges += 1;
            lean_inner_powers += 2 * (k - 1);
        } else {
            // Per-variable pairs within the block: bits if either is a row bit.
            for pair in (0..k).collect::<Vec<_>>().chunks(2) {
                if pair.iter().any(|j| (1..=sh.row_bits).contains(&(g + j))) {
                    bits_sq += 1;
                } else {
                    raw_sq += 1;
                }
            }
            challenges += k;
        }
        g += k;
    }
    bits_sq += 1; // alpha, gamma
    challenges += 2;
    if p.inherited > 0 {
        perms += seg(0, 2 * p.inherited);
        bits_sq += 1;
        challenges += 1;
    }
    perms += seg(0, 2 + w);
    bits_sq += 1;
    perms += l * seg(0, 4);
    bits_sq += l;
    perms += seg(0, 4);
    bits_sq += 1;
    challenges += 1 + l + 1;
    let squeezes = bits_sq + raw_sq + full_sq;
    let points = r + mo + mi + p.inherited + 2 * l + 2;
    // The first permutation's constant lanes are free (6 rows).
    let transcript =
        (perms + squeezes) * 243 - 6 + bits_sq * 511 + raw_sq * 44 + full_sq * 41 + points * 3;
    let endoscales = (r - 1) + (mo + mi) + p.inherited + 2 * l + 1;
    let adds = 2 * (r - 1) + (mo + mi) + p.inherited + 1 + 2 * l + 3;
    let group = endoscales * 448 + adds * 6 + 1_549 + 3 * 520 + 4 * 512;
    let endo = sh.row_bits + 1 + usize::from(p.inherited > 0) + 1 + l + 1;
    let powers =
        (sh.sx - 1) + ob.iter().map(|k| 2 * (k - 1)).sum::<usize>() + 2 + lean_inner_powers;
    let relation = relation_rows(sh, true);
    let closed = (1 + p.inherited) * closed_form_rows(sh);
    let io_bits = io.next_power_of_two().trailing_zeros() as usize;
    let io_eval = (sh.sw() - io_bits) + ((1 << io_bits) - 2) + 2 * io;
    let scalar =
        endo * 190 + io.div_ceil(2) * 243 + relation + powers + closed + io_eval + matrix_mle() + 4;
    let shared = challenges + 1 + 2 * 9 + 4 * p.inherited;
    // Range checks in C_P where the binding is non-native (not in the
    // synthesis): raw challenges and full scalars for a P-proof, the absorbed-
    // only limbs for an S-proof.
    let raw = challenges - endo;
    let (transcript, scalar) = if embed {
        (transcript, scalar + raw * 10 + 9 * 40)
    } else {
        (transcript + 5 * 20, scalar)
    };
    Split {
        transcript: transcript + PAIR_LINK_ROWS,
        group,
        scalar,
        shared: shared + PAIR_LINK_SHARED,
        bits_points: bits_sq * 255 + 4 * 255,
        bits_scalar: endo * 126,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inheritance {
    /// Fold inherited claims into the own opening in clear (links histories).
    Joint,
    /// Private masked restructuring per curve, exporting one fresh claim
    /// (today's create_restructure behaviour; histories stay unlinkable).
    Restructure,
}

/// In-circuit verification of one private masked restructuring IPA over a
/// generator vector of 2^`log_cols`, combining `sources` claims: source
/// combination, mask fold, IPA fold, a non-ZK final opening check, and its
/// transcript (L/R and the mask commitment) in the points-native circuit;
/// the claimed value sum_j mu^j prod_a (1 - r_a + r_a u_ja) in the other.
pub fn restructure_cost(log_cols: usize, sources: usize, g: &GadgetCosts) -> Split {
    let l = log_cols;
    let endoscales = 2 * l + (sources - 1) + 1;
    let group = endoscales * g.endoscale_128
        + (2 * l + sources + 2) * g.point_add
        + g.var_mul_255
        + 2 * g.fixed_mul_255;
    let points = 2 * l + 2;
    let challenges = 2 + l + l; // mu, delta binding, opening point (MLE), u_j
    let transcript = points * g.poseidon
        + challenges.div_ceil(2) * (g.poseidon + g.squeeze_bits)
        + points * g.on_curve;
    let scalar = sources * 2 * l + (l + 2) * 130 + 16;
    let bits_points = endoscales * 128 + 255 + 2 * 256 + challenges.div_ceil(2) * 254;
    Split {
        transcript,
        group,
        scalar,
        shared: challenges + 4,
        bits_points,
        bits_scalar: (l + 2) * 128,
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Binding {
    /// Masked random linear combination, non-native in C_P: no bytes.
    Rlc,
    /// Challenges and proof scalars transmitted; kappa committed (Pallas
    /// commitment recomputed in C_P, equal to S's dedicated row).
    Transmit,
}

pub struct OperationCost {
    pub primary: usize,
    pub secondary: usize,
    pub transmitted_bytes: usize,
    /// Witness entries of the recursion overhead by class: one-bit, narrow
    /// range chunks (C_P binding), full width (everything else).
    pub primary_bits: usize,
    pub primary_narrow: usize,
    pub secondary_bits: usize,
}

/// Recursion overhead of one operation verifying `pairs` (P, S) proof pairs.
pub fn operation(
    pairs: &[(ProofShape, ProofShape)],
    fp: &GadgetCosts,
    fq: &GadgetCosts,
    binding: Binding,
    inheritance: Inheritance,
) -> OperationCost {
    operation_with(pairs, fp, fq, binding, inheritance, StepCost::ESTIMATE)
}

pub fn operation_with(
    pairs: &[(ProofShape, ProofShape)],
    fp: &GadgetCosts,
    fq: &GadgetCosts,
    binding: Binding,
    inheritance: Inheritance,
    step: StepCost,
) -> OperationCost {
    let (mut primary, mut secondary, mut bytes) = (0, 0, 0);
    let (mut primary_bits, mut primary_narrow, mut secondary_bits) = (0, 0, 0);
    if inheritance == Inheritance::Restructure {
        // Each source pair contributes its own claim and its exported claim
        // on each curve. Vesta claims: group work in C_S; Pallas in C_P.
        let sources = 2 * pairs.len();
        let (p0, s0) = pairs[0];
        let vesta = restructure_cost(p0.shape.col_bits, sources, fq);
        let pallas = restructure_cost(s0.shape.col_bits, sources, fp);
        secondary += vesta.transcript + vesta.group + pallas.scalar;
        primary += pallas.transcript + pallas.group + vesta.scalar;
        secondary_bits += vesta.bits_points + pallas.bits_scalar;
        primary_bits += pallas.bits_points + vesta.bits_scalar;
        if let Binding::Rlc = binding {
            primary += (vesta.shared + pallas.shared) * step.rows;
            primary_narrow += (vesta.shared + pallas.shared) * step.narrow;
            secondary += vesta.shared + pallas.shared;
        }
    }
    for (p, s) in pairs {
        // P-proof: points (Vesta) native in C_S (Fq gadgets).
        let vp = if synthesized() {
            verify_cost_synth(p, true)
        } else {
            verify_cost(p, fq)
        };
        // S-proof: points (Pallas) native in C_P (Fp gadgets).
        let vs = if synthesized() {
            verify_cost_synth(s, false)
        } else {
            verify_cost(s, fp)
        };
        secondary += vp.transcript + vp.group + vs.scalar;
        primary += vs.transcript + vs.group + vp.scalar;
        secondary_bits += vp.bits_points + vs.bits_scalar;
        primary_bits += vs.bits_points + vp.bits_scalar;
        match binding {
            Binding::Rlc => {
                let shared = vp.shared + vs.shared + 1;
                primary += shared * step.rows;
                primary_narrow += shared * step.narrow;
                secondary += shared;
            }
            Binding::Transmit => {
                // kappa'_P and (z1 b_f zeta)_P committed in C_P: two fixed-base
                // multiplications each plus the blind; S's side is native.
                primary += 3 * fp.fixed_mul_255 + 2 * fp.point_add;
                bytes += 16 * (p.challenges() + s.challenges()) + 32 * (5 + 5 + 2);
            }
        }
    }
    OperationCost {
        primary,
        secondary,
        transmitted_bytes: bytes,
        primary_bits,
        primary_narrow,
        secondary_bits,
    }
}
