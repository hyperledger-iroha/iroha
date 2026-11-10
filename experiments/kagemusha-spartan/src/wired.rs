//! Stage A2, step 6: the remaining flows, wired. Copies (uniform.rs: LogUp
//! over address and values, the addresses in preprocessed index columns)
//! carry values between families; links carry them inside a family or from
//! a global array.
//!
//! - `own_chain`: the own relation's signed messages. Each message is a
//!   Poseidon digest. The RP57 family exports each permutation's output lane
//!   0 from its input array, through links into the last round, so the export
//!   adds no cells; SHA-256's message array reads it as m by a copy. SHA-256
//!   and the comb share the signature family: the output words carry z's three
//!   88-bit limbs as running sums whose last step is the comb's z limb (a
//!   link).
//! - `scalar_wired`: one proof's scalar side with its inputs bound. The
//!   binding chain holds every shared value (range-checked below 2^128). Raw
//!   challenges are read from it as they are; endo challenges through the
//!   endo value array (the raw value as thirteen 10-bit chunks, each looked
//!   up with its share of the endoscalar in one fixed table); full-width
//!   values through limb splits (value = lo + 2^128 hi). Every array is
//!   global, so the flows are links: an input cell equals its source.

use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use iroha_pasta::Eq as Vesta;
use iroha_pasta::poseidon::PoseidonField;
use num_bigint::BigUint;

use crate::fs::endo_scalar;
use crate::ipa::Claim;
use crate::nonnative::{self, Binding, Comb, Z_INSTANCE};
use crate::p256;
use crate::r1cs::{Instance, Shape};
use crate::scalar::{self, ScalarSide};
use crate::sha::{self, Sha};
use crate::spartan::{self, Proof};
use crate::uniform::{
    self, Array, Copies, Export, Import, Lin, RP_FULL, RP_INPUT, Src, Term, constant, link, own,
    plus, scaled,
};
use crate::verifier::{Kind, Trace, shared_scalars, shared_values};

pub type Fp = iroha_pasta::Fp;

/// A wired circuit's parts, ready for `UniformCircuit::build_with_copies`.
pub struct Wired {
    pub arrays: Vec<Array<Fp>>,
    pub pi: Vec<usize>,
    pub counts: Vec<usize>,
    pub tables: Vec<(usize, u32)>,
    pub copies: Copies<Fp>,
}

/// Array offsets of `own_chain`: the RP57 arrays, SHA-256's, the comb's.
pub const W_PERM: usize = 0;
pub const W_SHA: usize = 5;
pub const W_COMB: usize = 10;

/// Families of `own_chain`: permutations (0) and their mask gates (1) as
/// `rp57` makes them, the signatures, the global tables.
const F_SIG: usize = 2;
const F_GLOBAL: usize = 3;

/// Cells appended to SHA-256's output array: lo16, lo24, the word, then the
/// three running limb sums.
pub const Z_LO16: usize = 33;
pub const Z_ACC: usize = 36;

/// The permutation whose output lane 0 signature `p` signs.
pub fn digest_perm(p: usize) -> usize {
    2 * p + 1
}

fn fp_big(x: &BigUint) -> Fp {
    let mut b = x.to_bytes_le();
    b.resize(32, 0);
    Option::from(Fp::from_repr(b.try_into().unwrap())).expect("below p")
}

fn p2(e: i64) -> Fp {
    let x = Fp::from(2u64).pow_vartime([e.unsigned_abs()]);
    if e >= 0 { x } else { x.invert().unwrap() }
}

/// The own relation's signed messages: `2^log` RP57 permutations (on random
/// inputs), `sigs` of whose outputs are signed: SHA-256 of the digest's
/// encoding, then the comb verifies the wallet's ECDSA signature on it.
/// Without `wire`, the same blocks with no flows between them (to price the
/// wiring).
pub fn own_chain(log: usize, sigs: usize, wire: bool) -> Wired {
    assert!(
        (1..=2).contains(&sigs) && digest_perm(sigs - 1) < 1 << log,
        "digest permutations fit"
    );
    let (mut arrays, pi_perm, outputs) = uniform::rp57::<Fp>(log, b"wired perms");
    assert_eq!(arrays.len(), W_SHA);
    let messages: Vec<Fp> = (0..sigs).map(|p| outputs[digest_perm(p)][0]).collect();
    let pi_sig = 1;
    let Sha {
        arrays: sha_arrays,
        tables: sha_tables,
        digests,
    } = sha::sha(W_SHA, F_SIG, pi_sig, F_GLOBAL, &messages);
    arrays.extend(sha_arrays);
    assert_eq!(arrays.len(), W_COMB);
    // The wallet's signatures on the digests.
    let n = p256::n();
    let d = p256::scalar_below("wired-wallet-key", &n);
    let q = p256::mul(&p256::generator(), &d).unwrap();
    let inputs: Vec<(p256::Point, BigUint, BigUint, BigUint)> = digests
        .iter()
        .enumerate()
        .map(|(k, dg)| {
            let z = BigUint::from_bytes_be(dg);
            let sig = p256::sign(&d, &z, &p256::scalar_below(&format!("wired-nonce-{k}"), &n));
            (q.clone(), z, sig.r, sig.s)
        })
        .collect();
    let Comb {
        arrays: comb_arrays,
        tables: comb_tables,
        ..
    } = nonnative::comb(W_COMB, F_SIG, pi_sig, F_GLOBAL, &inputs);
    arrays.extend(comb_arrays);
    let mut tables = sha_tables;
    tables.extend(comb_tables);
    let (pi, counts) = (
        vec![pi_perm[0], pi_perm[1], pi_sig, 0],
        vec![1 << log, 2, sigs, 1],
    );
    if !wire {
        return Wired {
            arrays,
            pi,
            counts,
            tables,
            copies: Copies::default(),
        };
    }
    let zs: Vec<BigUint> = inputs.iter().map(|x| x.1.clone()).collect();
    digest_limbs(
        &mut arrays,
        W_SHA + sha::A_OUT,
        W_COMB + nonnative::A_VALUE,
        &zs,
    );
    // m <- the digest permutation's output lane 0 (the last full round's
    // boxes after the MDS row).
    let mds = <Fp as PoseidonField>::rp57().mds();
    let lane0: Lin<Fp> = (0..3).fold(Vec::new(), |acc, j| {
        plus(
            &acc,
            &link(W_PERM + RP_FULL, 3 * j + 2, vec![Some(7)], mds[0][j]),
        )
    });
    let msg_a = W_SHA + sha::A_MSG;
    let (rho_m, msg_acc) = (arrays[msg_a].rho, arrays[msg_a].cells() - 1);
    let mut from = vec![None; arrays[msg_a].values[0].len()];
    for p in 0..sigs {
        from[(p << rho_m) | 7] = Some((0, digest_perm(p)));
    }
    let copies = Copies {
        exports: vec![Export {
            array: W_PERM + RP_INPUT,
            tuple: vec![lane0],
        }],
        imports: vec![Import {
            array: msg_a,
            tuple: vec![own(msg_acc, 1 << rho_m)],
            from,
        }],
        ..Default::default()
    };
    Wired {
        arrays,
        pi,
        counts,
        tables,
        copies,
    }
}

/// Appends z's limbs to SHA-256's output array (8 word instances per block):
/// lo16, lo24 and the word from the bits, and three running sums over the
/// words. Word k sits at bit 32 (7 - k) of z (big-endian), so limb 0 takes
/// words 7, 6 and word 5's low 24 bits; limb 1 word 5's high 8 bits, words 4,
/// 3 and word 2's low 16 bits; limb 2 word 2's high 16 bits and words 1, 0.
/// A sum's last step (word 7) is the comb's z limb, by a link.
pub(crate) fn digest_limbs(arrays: &mut [Array<Fp>], out_a: usize, value_a: usize, zs: &[BigUint]) {
    let out = &mut arrays[out_a];
    let n = 1usize << out.rho;
    assert_eq!(
        (n, out.cells()),
        (8, Z_LO16),
        "SHA-256 output words: 32 bits and a carry"
    );
    // Coefficients per limb and word of the word, lo24 and lo16.
    let mut cw = [[Fp::ZERO; 8]; 3];
    let mut c24 = [[Fp::ZERO; 8]; 3];
    let mut c16 = [[Fp::ZERO; 8]; 3];
    cw[0][7] = p2(0);
    cw[0][6] = p2(32);
    c24[0][5] = p2(64);
    cw[1][5] = p2(-24);
    c24[1][5] = -p2(-24);
    cw[1][4] = p2(8);
    cw[1][3] = p2(40);
    c16[1][2] = p2(72);
    cw[2][2] = p2(-16);
    c16[2][2] = -p2(-16);
    cw[2][1] = p2(16);
    cw[2][0] = p2(48);
    let (lo16, lo24, word) = (Z_LO16, Z_LO16 + 1, Z_LO16 + 2);
    let slots = out.values[0].len();
    let mut vals = vec![vec![Fp::ZERO; slots]; 6];
    for i in 0..slots {
        let w: u64 = (0..32)
            .map(|b| u64::from(out.values[b][i] == Fp::ONE) << b)
            .sum();
        vals[0][i] = Fp::from(w & 0xffff);
        vals[1][i] = Fp::from(w & 0xff_ffff);
        vals[2][i] = Fp::from(w);
    }
    for (p, z) in zs.iter().enumerate() {
        let mut acc = [Fp::ZERO; 3];
        for k in 0..8 {
            let i = (p << 3) | k;
            for l in 0..3 {
                acc[l] += cw[l][k] * vals[2][i] + c24[l][k] * vals[1][i] + c16[l][k] * vals[0][i];
                if k < 7 {
                    vals[3 + l][i] = acc[l];
                }
            }
        }
        for (l, a) in acc.iter().enumerate() {
            let limb = (z >> (88 * l)) & ((BigUint::from(1u32) << 88) - 1u32);
            assert_eq!(*a, fp_big(&limb), "z limb {l} from the digest words");
        }
    }
    let ones = vec![Fp::ONE; 8];
    let bits_sum = |lo: usize, hi: usize| -> Lin<Fp> {
        (lo..hi).fold(Vec::new(), |acc, b| {
            plus(&acc, &scaled(&own(b, 8), p2(b as i64)))
        })
    };
    let neg = |l: &Lin<Fp>| scaled(l, -Fp::ONE);
    let lin_row = |l: Lin<Fp>| -> [Lin<Fp>; 3] { [l, constant(ones.clone()), Vec::new()] };
    let per = |cell: usize, c: &[Fp; 8]| -> Lin<Fp> {
        vec![Term {
            src: Src::Own(cell),
            coef: c.iter().map(|x| -*x).collect(),
        }]
    };
    out.rows
        .push(lin_row(plus(&own(lo16, 8), &neg(&bits_sum(0, 16)))));
    out.rows.push(lin_row(plus(
        &plus(&own(lo24, 8), &neg(&own(lo16, 8))),
        &neg(&bits_sum(16, 24)),
    )));
    out.rows.push(lin_row(plus(
        &plus(&own(word, 8), &neg(&own(lo24, 8))),
        &neg(&bits_sum(24, 32)),
    )));
    for l in 0..3 {
        let mut row = vec![Term {
            src: Src::Own(Z_ACC + l),
            coef: (0..8)
                .map(|k| if k < 7 { Fp::ONE } else { Fp::ZERO })
                .collect(),
        }];
        row = plus(
            &row,
            &link(
                value_a,
                l,
                (0..8).map(|k| (k == 7).then_some(Z_INSTANCE)).collect(),
                Fp::ONE,
            ),
        );
        row = plus(
            &row,
            &link(
                out_a,
                Z_ACC + l,
                (0..8).map(|k: usize| k.checked_sub(1)).collect(),
                -Fp::ONE,
            ),
        );
        row = plus(&row, &per(word, &cw[l]));
        row = plus(&row, &per(lo24, &c24[l]));
        row = plus(&row, &per(lo16, &c16[l]));
        out.rows.push(lin_row(row));
    }
    out.widths.extend([16, 24, 32, 88, 88, 80]);
    out.values.extend(vals);
}

/// Array offsets of `scalar_wired`: the scalar side's seven arrays, the
/// binding chain's four (value, check, range table, shared), the endo values
/// and their table, the limb splits.
pub const S_SIDE: usize = 0;
pub const S_BIND: usize = 7;
pub const S_ENDO: usize = 11;
pub const S_ENDO_TABLE: usize = 12;
pub const S_LIMB: usize = 13;

pub const ENDO_TABLE: u32 = 3;
const ENDO_CHUNK: usize = 10;
const ENDO_CHUNKS: usize = 13;
/// Endo value cells: the raw value's chunks (they recompose the shared value
/// itself), their table values, the endoscalar.
pub const E_CH: usize = 0;
pub const E_EV: usize = ENDO_CHUNKS;
pub const E_VAL: usize = 2 * ENDO_CHUNKS;
/// The limb split's one cell: value = lo + 2^128 hi of two shared values.
pub const L_VAL: usize = 0;

/// What feeds a scalar-side input: a raw challenge's shared value, an endo
/// value instance, or a limb split instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Feed {
    Raw(usize),
    Endo(usize),
    Full(usize),
}

impl Feed {
    /// The source (array, cell, instance).
    fn source(self) -> (usize, usize, usize) {
        match self {
            Feed::Raw(i) => (S_BIND + nonnative::B_SHARED, 0, i),
            Feed::Endo(e) => (S_ENDO, E_VAL, e),
            Feed::Full(k) => (S_LIMB, L_VAL, k),
        }
    }
}

/// One chunk's share of an endoscalar: pairs (s, e) = bits (2i, 2i + 1) add
/// `2^i (2 s - 1 + 2 (zeta - 1) s e + (1 - zeta) e)` to `a zeta + b`.
fn endo_share(c: usize) -> Fp {
    let zeta = <Fp as WithSmallOrderMulGroup<3>>::ZETA;
    (0..ENDO_CHUNK / 2).fold(Fp::ZERO, |acc, i| {
        let (s, e) = (
            Fp::from(((c >> (2 * i)) & 1) as u64),
            Fp::from(((c >> (2 * i + 1)) & 1) as u64),
        );
        acc + p2(i as i64)
            * (s.double() - Fp::ONE + (zeta - Fp::ONE).double() * s * e + (Fp::ONE - zeta) * e)
    })
}

/// The endoscalar's constant: `2^64 (zeta + 1)`, plus the last chunk's two
/// missing pairs (each read as `-2^i`, i = 3, 4, weighted 2^60).
fn endo_constant() -> Fp {
    let zeta = <Fp as WithSmallOrderMulGroup<3>>::ZETA;
    p2(64) * (zeta + Fp::ONE) + p2(60) * Fp::from(24u64)
}

pub struct ScalarWired {
    pub wired: Wired,
    pub endo: usize,
    pub limbs: usize,
    pub shared: usize,
    /// Scalar-side inputs bound to a source (challenges and full values).
    pub inputs: usize,
    pub expected: Vec<(&'static str, usize, usize, usize, Fp)>,
    pub q_pad: usize,
    pub slot_instances: usize,
}

/// The scalar side of one unlinked proof with its inputs
/// bound: challenges and the full-width scalars come from the binding
/// chain's shared values; the scalar side's outputs that are shared values
/// (kappa', the three Schnorr scalars) are bound the same way. The io
/// values, the inherited claims' u and the matrix MLE's values stay inputs
/// (the statement and the in-circuit matrix evaluation wire them). Without
/// `wire`: the scalar side and the binding chain only, no flows.
pub fn scalar_wired(
    shape: &Shape,
    inst: &Instance<Fp>,
    proof: &Proof<Vesta>,
    claims: &[Claim<Vesta>],
    tr: &Trace<Vesta>,
    wire: bool,
) -> ScalarWired {
    let lean = crate::fs::lean_schedule();
    assert!(tr.link.is_none(), "an unlinked proof");
    let ScalarSide {
        mut arrays,
        q_pad,
        slot_instances: n_s,
        expected,
        ..
    } = scalar::scalar_side::<Vesta>(S_SIDE, 0, shape, lean, inst, proof, claims, tr);
    assert_eq!(arrays.len(), S_BIND);
    let values = shared_values(tr, proof, claims);
    let ch = &tr.challenges;
    let slots = spartan::slots(shape);
    let (mo, ns, l) = (shape.outer_blocks().len(), slots.len(), shape.col_bits);
    let x_of = |s: usize| n_s - ns + s;
    let mut shared_at = Vec::with_capacity(ch.len());
    let mut at = 0;
    for (_, kind, _) in ch {
        shared_at.push(at);
        at += if *kind == Kind::Full { 2 } else { 1 };
    }
    let full_base = at;
    // Walk the transcript (as `verifier::trace`) and record what feeds each input.
    let mut next = 0;
    let mut take = |want: Kind| -> usize {
        assert_eq!(ch[next].1, want, "challenge {next}'s kind");
        next += 1;
        next - 1
    };
    let mut endo_ch: Vec<usize> = Vec::new();
    let mut endo_of = |i: usize| -> Feed {
        endo_ch.push(i);
        Feed::Endo(endo_ch.len() - 1)
    };
    let p128 = p2(128);
    let full = |lo: usize| -> (usize, usize, Fp) {
        (
            lo,
            lo + 1,
            Fp::from_u128(values[lo]) + p128 * Fp::from_u128(values[lo + 1]),
        )
    };
    let mut limb_src: Vec<(usize, usize, Fp)> = Vec::new();
    let mut slot_feed: Vec<[Option<Feed>; 3]> = vec![[None; 3]; n_s];
    let mut tau_feed: Vec<[Option<Feed>; 3]> = vec![[None; 3]; n_s];
    let mut glue_feed: Vec<(usize, Feed)> = Vec::new();
    let mut u_feed: Vec<Feed> = Vec::new();
    if lean {
        // tau: one full-width challenge; slot 0's first digit holds it.
        let i = take(Kind::Full);
        limb_src.push(full(shared_at[i]));
        assert!(limb_src[0].2 == tr.tau[0], "tau from its limbs");
        tau_feed[x_of(0)][3 - slots[0].k] = Some(Feed::Full(0));
    } else {
        for s in 0..mo {
            for v in 0..slots[s].k {
                tau_feed[x_of(s)][3 - slots[s].k + v] = Some(Feed::Raw(shared_at[take(Kind::Raw)]));
            }
        }
    }
    for s in 0..mo {
        let n_round = if lean { 1 } else { slots[s].k };
        for v in 0..n_round {
            slot_feed[x_of(s)][3 - slots[s].k + v] = Some(Feed::Raw(shared_at[take(Kind::Raw)]));
        }
    }
    for k in 0..if lean { 1 } else { 3 } {
        glue_feed.push((scalar::R1 + k, Feed::Raw(shared_at[take(Kind::Raw)])));
    }
    let mut g = 0;
    for (s, sl) in slots.iter().enumerate().skip(mo) {
        assert_eq!(sl.first, g, "inner slot order");
        let d0 = 3 - sl.k;
        if lean && g > shape.row_bits {
            let i = take(Kind::Raw);
            slot_feed[x_of(s)][d0] = Some(Feed::Raw(shared_at[i]));
            g += sl.k;
            continue;
        }
        for v in 0..sl.k {
            let feed = if (1..=shape.row_bits).contains(&g) {
                endo_of(take(Kind::Endo))
            } else {
                Feed::Raw(shared_at[take(Kind::Raw)])
            };
            slot_feed[x_of(s)][d0 + v] = Some(feed);
            g += 1;
        }
    }
    glue_feed.push((scalar::ALPHA, Feed::Raw(shared_at[take(Kind::Raw)])));
    glue_feed.push((scalar::GAMMA, endo_of(take(Kind::Endo))));
    if !claims.is_empty() {
        glue_feed.push((scalar::XI, endo_of(take(Kind::Endo))));
    }
    glue_feed.push((scalar::ZETA, endo_of(take(Kind::Endo))));
    for _ in 0..l {
        u_feed.push(endo_of(take(Kind::Endo)));
    }
    let _c = take(Kind::Endo);
    assert_eq!(next, ch.len(), "every challenge placed");
    // Full-width scalars: shared as [io digest, v_A, v_B, v_C, kappa',
    // kappa' zeta, c(z1), z1 b_final zeta, z2], two limbs each.
    let sh = shared_scalars(tr, proof);
    let fulls = [
        (1, scalar::V0, proof.v[0]),
        (2, scalar::V0 + 1, proof.v[1]),
        (3, scalar::V0 + 2, proof.v[2]),
        (4, scalar::KAPPA2, tr.kappa_claims),
        (5, scalar::KZ, sh.kappa_zeta),
        (6, scalar::CZ, sh.c_z1),
        (7, scalar::ZBZ, sh.z1_bf_zeta),
    ];
    for (pos, cell, want) in fulls {
        let src = full(full_base + 2 * pos);
        assert!(src.2 == want, "full scalar {pos} from its limbs");
        limb_src.push(src);
        glue_feed.push((cell, Feed::Full(limb_src.len() - 1)));
    }
    // The binding chain over every shared value.
    let (rho, mu, _) = nonnative::sample_binding(0);
    let Binding {
        arrays: bind,
        tables: bind_tables,
        ..
    } = nonnative::binding_chain(S_BIND, 0, 0, &rho, &mu, &values);
    arrays.extend(bind);
    assert_eq!(arrays.len(), S_ENDO);
    if !wire {
        return ScalarWired {
            wired: Wired {
                arrays,
                pi: vec![0],
                counts: vec![1],
                tables: bind_tables,
                copies: Copies::default(),
            },
            endo: 0,
            limbs: 0,
            shared: values.len(),
            inputs: 0,
            expected,
            q_pad,
            slot_instances: n_s,
        };
    }
    // Endo values: raw = sum 2^(10 j) chunk_j, value = constant + sum_j
    // 2^(5 j) share(chunk_j); (chunk, share) looked up in one fixed table.
    let endo_bits: Vec<&Vec<bool>> = endo_ch.iter().map(|i| &ch[*i].0).collect();
    let ne = endo_bits.len();
    let rho_e = ne.next_power_of_two().trailing_zeros() as usize;
    let n_e = 1usize << rho_e;
    let mut ev = vec![vec![Fp::ZERO; n_e]; E_VAL + 1];
    for (e, bits) in endo_bits.iter().enumerate() {
        let raw: u128 = bits
            .iter()
            .enumerate()
            .fold(0, |a, (i, b)| a | ((*b as u128) << i));
        let mut val = endo_constant();
        for j in 0..ENDO_CHUNKS {
            let c = ((raw >> (ENDO_CHUNK * j)) & ((1 << ENDO_CHUNK) - 1)) as usize;
            ev[E_CH + j][e] = Fp::from(c as u64);
            ev[E_EV + j][e] = endo_share(c);
            val += p2((5 * j) as i64) * endo_share(c);
        }
        ev[E_VAL][e] = val;
        assert!(val == endo_scalar::<Fp>(bits), "endoscalar from its chunks");
    }
    let endo_shared: Vec<Option<usize>> = (0..n_e)
        .map(|e| endo_ch.get(e).map(|i| shared_at[*i]))
        .collect();
    let ones_e = vec![Fp::ONE; n_e];
    let mut erows: Vec<[Lin<Fp>; 3]> = Vec::new();
    let raw_sum = (0..ENDO_CHUNKS).fold(Vec::new(), |acc, j| {
        plus(
            &acc,
            &scaled(&own(E_CH + j, n_e), p2((ENDO_CHUNK * j) as i64)),
        )
    });
    erows.push([
        plus(
            &link(S_BIND + nonnative::B_SHARED, 0, endo_shared, Fp::ONE),
            &scaled(&raw_sum, -Fp::ONE),
        ),
        constant(ones_e.clone()),
        Vec::new(),
    ]);
    let val_sum = (0..ENDO_CHUNKS).fold(Vec::new(), |acc, j| {
        plus(&acc, &scaled(&own(E_EV + j, n_e), p2((5 * j) as i64)))
    });
    erows.push([
        plus(
            &plus(&own(E_VAL, n_e), &scaled(&val_sum, -Fp::ONE)),
            &constant(vec![-endo_constant(); n_e]),
        ),
        constant(ones_e),
        Vec::new(),
    ]);
    let elookups = (0..ENDO_CHUNKS)
        .map(|j| (ENDO_TABLE, vec![own(E_CH + j, n_e), own(E_EV + j, n_e)]))
        .collect();
    let mut ewidths = vec![ENDO_CHUNK as u16; ENDO_CHUNKS];
    ewidths.extend(vec![255u16; ENDO_CHUNKS]);
    ewidths.push(255);
    arrays.push(Array {
        name: "endo value",
        family: 0,
        rho: rho_e,
        r_count: ne,
        fixed: false,
        lookups: elookups,
        widths: ewidths,
        rows: erows,
        values: ev,
    });
    arrays.push(Array {
        name: "endo table",
        family: 0,
        rho: ENDO_CHUNK,
        r_count: 1 << ENDO_CHUNK,
        fixed: true,
        lookups: Vec::new(),
        widths: vec![ENDO_CHUNK as u16, 255],
        rows: Vec::new(),
        values: vec![
            (0..1u64 << ENDO_CHUNK).map(Fp::from).collect(),
            (0..1usize << ENDO_CHUNK).map(endo_share).collect(),
        ],
    });
    // Limb splits: value = lo + 2^128 hi, lo and hi read from the shared values.
    let nl = limb_src.len();
    let rho_l = nl.next_power_of_two().trailing_zeros() as usize;
    let n_l = 1usize << rho_l;
    let mut lv = vec![vec![Fp::ZERO; n_l]];
    for (k, (_, _, val)) in limb_src.iter().enumerate() {
        lv[L_VAL][k] = *val;
    }
    let to_shared = |f: &dyn Fn(&(usize, usize, Fp)) -> usize| -> Vec<Option<usize>> {
        (0..n_l).map(|k| limb_src.get(k).map(f)).collect()
    };
    let split = plus(
        &plus(
            &own(L_VAL, n_l),
            &link(
                S_BIND + nonnative::B_SHARED,
                0,
                to_shared(&|s| s.0),
                -Fp::ONE,
            ),
        ),
        &link(S_BIND + nonnative::B_SHARED, 0, to_shared(&|s| s.1), -p128),
    );
    let lrows = vec![[split, constant(vec![Fp::ONE; n_l]), Vec::new()]];
    arrays.push(Array {
        name: "limb split",
        family: 0,
        rho: rho_l,
        r_count: nl,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: lrows,
        values: lv,
    });
    assert_eq!(arrays.len(), S_LIMB + 1);
    // Wiring rows: an input cell (where fed) minus its source.
    let wire = |n: usize, cell: usize, feeds: &[Option<Feed>]| -> [Lin<Fp>; 3] {
        let mask: Vec<Fp> = feeds
            .iter()
            .map(|f| if f.is_some() { Fp::ONE } else { Fp::ZERO })
            .collect();
        let mut row: Lin<Fp> = vec![Term {
            src: Src::Own(cell),
            coef: mask,
        }];
        for kind in 0..3 {
            let mut src = None;
            let map: Vec<Option<usize>> = feeds
                .iter()
                .map(|f| {
                    f.filter(|f| {
                        matches!(
                            (kind, f),
                            (0, Feed::Raw(_)) | (1, Feed::Endo(_)) | (2, Feed::Full(_))
                        )
                    })
                    .map(|f| {
                        let (a, k, i) = f.source();
                        src = Some((a, k));
                        i
                    })
                })
                .collect();
            if let Some((a, k)) = src {
                row = plus(&row, &link(a, k, map, -Fp::ONE));
            }
        }
        [row, constant(vec![Fp::ONE; n]), Vec::new()]
    };
    let mut inputs = 0;
    {
        let sd = &mut arrays[S_SIDE + scalar::S_SLOT];
        for d in 0..3 {
            let feeds: Vec<Option<Feed>> = (0..n_s).map(|x| slot_feed[x][d]).collect();
            if feeds.iter().any(|f| f.is_some()) {
                inputs += feeds.iter().filter(|f| f.is_some()).count();
                sd.rows.push(wire(n_s, scalar::T[d], &feeds));
            }
        }
        for d in 0..3 {
            let feeds: Vec<Option<Feed>> = (0..n_s).map(|x| tau_feed[x][d]).collect();
            if feeds.iter().any(|f| f.is_some()) {
                inputs += feeds.iter().filter(|f| f.is_some()).count();
                sd.rows.push(wire(n_s, scalar::TAU[d], &feeds));
            }
        }
    }
    {
        let bits = &mut arrays[S_SIDE + scalar::S_BITS];
        let n_b = 1usize << bits.rho;
        let feeds: Vec<Option<Feed>> = (0..n_b)
            .map(|r| (r % q_pad == 0 && r / q_pad < l).then(|| u_feed[r / q_pad]))
            .collect();
        bits.rows.push(wire(n_b, scalar::U, &feeds));
        inputs += l;
    }
    {
        let glue = &mut arrays[S_SIDE + scalar::S_GLUE];
        for (cell, feed) in &glue_feed {
            glue.rows.push(wire(1, *cell, &[Some(*feed)]));
        }
        inputs += glue_feed.len();
    }
    let mut tables = bind_tables;
    tables.push((S_ENDO_TABLE, ENDO_TABLE));
    ScalarWired {
        wired: Wired {
            arrays,
            pi: vec![0],
            counts: vec![1],
            tables,
            copies: Copies::default(),
        },
        endo: ne,
        limbs: nl,
        shared: values.len(),
        inputs,
        expected,
        q_pad,
        slot_instances: n_s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::SpikeRng;
    use crate::grid::eq_table;
    use crate::r1cs::{Circuit, Instance, Witness};
    use crate::uniform::UniformCircuit;
    use iroha_pasta::Eq as Vesta;

    fn build(
        w: Wired,
        label: &'static str,
        col_bits: usize,
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>) {
        UniformCircuit::build_with_copies(
            label,
            w.arrays,
            w.pi,
            w.counts,
            w.tables,
            w.copies,
            col_bits,
            4,
            label.as_bytes(),
        )
    }

    /// Every row holds; the compiled evaluation and M(r_x, .) equal the
    /// expansion; the honest proof verifies.
    fn holds_and_proves(
        c: &UniformCircuit<Fp>,
        inst: &Instance<Fp>,
        wit: &Witness<Fp>,
        gens: &crate::ipa::Gens<Vesta>,
    ) {
        let shape = c.shape;
        let mut rng = SpikeRng::new(b"wired points");
        let ch: Vec<Fp> = vec![rng.scalar(), rng.scalar()];
        let mut w = wit.w.clone();
        let rel = c.instantiate(&shape, &ch, Some(&mut w));
        let z = crate::r1cs::z_vector(&shape, inst, &Witness { w });
        let (az, bz, cz) = rel.abc(&shape, &z);
        let bad: Vec<usize> = (0..1 << shape.sx)
            .filter(|x| az[*x] * bz[*x] != cz[*x])
            .collect();
        assert!(
            bad.is_empty(),
            "{} rows fail, first {:?}",
            bad.len(),
            &bad[..bad.len().min(5)]
        );
        let rx: Vec<Fp> = (0..shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<Fp> = (0..shape.sz()).map(|_| rng.scalar()).collect();
        let (ex, ey) = (eq_table(&rx), eq_table(&ry));
        let rm: [Fp; 3] = [rng.scalar(), rng.scalar(), rng.scalar()];
        let mut evals = [Fp::ZERO; 3];
        let mut m = vec![Fp::ZERO; 1 << shape.sz()];
        for (mat, x, y, v) in c.entries(&ch) {
            evals[mat] += v * ex[x] * ey[y];
            m[y] += ex[x] * v * rm[mat];
        }
        assert_eq!(
            rel.matrix_evals(&shape, &rx, &ry),
            (evals[0], evals[1], evals[2]),
            "compiled evaluation"
        );
        assert!(
            rel.m_table(&shape, &ex, rm[0], rm[1], rm[2]) == m,
            "M(r_x, .)"
        );
        let mut prng = SpikeRng::new(b"wired prove");
        let (proof, _) = crate::spartan::prove_circuit(&shape, gens, inst, wit, &mut prng, c);
        assert!(
            crate::spartan::verify_circuit(&shape, gens, inst, &proof.to_bytes(), c).is_ok(),
            "honest"
        );
    }

    fn accepted(
        c: &UniformCircuit<Fp>,
        inst: &Instance<Fp>,
        gens: &crate::ipa::Gens<Vesta>,
        bad: Vec<Fp>,
    ) -> bool {
        let mut prng = SpikeRng::new(b"wired tamper");
        let (proof, _) =
            crate::spartan::prove_circuit(&c.shape, gens, inst, &Witness { w: bad }, &mut prng, c);
        crate::spartan::verify_circuit(&c.shape, gens, inst, &proof.to_bytes(), c).is_ok()
    }

    #[test]
    fn own_chain_wires_digest_sha_and_comb() {
        let (c, inst, wit) = build(own_chain(2, 2, true), "wired-own-test", 14);
        let gens = crate::ipa::Gens::<Vesta>::new(c.shape.cols());
        holds_and_proves(&c, &inst, &wit, &gens);
        // A permutation recomputed from another input: every round row holds
        // and its output changes. Signed: refused (SHA-256 hashed the old
        // digest); unsigned: accepted (nobody reads its entry).
        let (other, _, _) = uniform::rp57::<Fp>(2, b"other perms");
        let splice = |p: usize| -> Vec<Fp> {
            let mut bad = wit.w.clone();
            for a in 0..4 {
                let rho = other[a].rho;
                for (k, col) in other[a].values.iter().enumerate() {
                    for (i, v) in col.iter().enumerate().filter(|(i, _)| i >> rho == p) {
                        bad[c.cell_index(W_PERM + a, k, i)] = *v;
                    }
                }
            }
            bad
        };
        assert!(
            !accepted(&c, &inst, &gens, splice(digest_perm(0))),
            "a signed digest that SHA-256 did not hash is refused"
        );
        assert!(
            accepted(&c, &inst, &gens, splice(0)),
            "an unread permutation may change"
        );
        // A running limb sum, and the m read's index column.
        let mut bad = wit.w.clone();
        bad[c.cell_index(W_SHA + sha::A_OUT, Z_ACC + 1, (1 << 3) | 4)] += Fp::ONE;
        assert!(!accepted(&c, &inst, &gens, bad), "changed limb sum refused");
        let idx = c
            .index_array(W_SHA + sha::A_MSG)
            .expect("the message array reads");
        let mut bad = wit.w.clone();
        bad[c.cell_index(idx, 0, (1 << 3) | 7)] += Fp::ONE;
        assert!(
            !accepted(&c, &inst, &gens, bad),
            "changed index column refused"
        );
    }

    #[test]
    fn scalar_side_wired_to_the_binding_chain() {
        let shape = Shape {
            label: "scalar-wired-test",
            sx: 13,
            row_bits: 2,
            col_bits: 11,
            k: 3,
        };
        let gens = crate::ipa::Gens::<Vesta>::new(shape.cols());
        let io_len = crate::census::STATEMENT_IO + 2 * (2 + shape.col_bits) - 6;
        let (inst, wit, claims) =
            crate::verifier::instance::<Vesta>(shape, &gens, 2, io_len, b"scalar wired");
        let proof = crate::spartan::prove_with_claims(
            &shape,
            &gens,
            &inst,
            &wit,
            &mut SpikeRng::new(b"scalar wired prove"),
            &claims,
        );
        let tr = crate::verifier::trace(
            &shape,
            &gens,
            &inst,
            &proof,
            &claims,
            &crate::spartan::Link::None,
        );
        let sw = scalar_wired(&shape, &inst, &proof, &claims, &tr, true);
        let lean = crate::fs::lean_schedule();
        assert_eq!(
            (sw.endo, sw.limbs),
            (2 + 3 + shape.col_bits, 7 + usize::from(lean)),
            "endo values: row bits, gamma, xi, zeta, u; limb splits: (lean) tau and seven scalars"
        );
        let expected = sw.expected.clone();
        let (c, uinst, uwit) = build(sw.wired, "scalar-wired-test", 12);
        let ugens = crate::ipa::Gens::<Vesta>::new(c.shape.cols());
        for (name, a, k, i, v) in &expected {
            assert!(
                uwit.w[c.cell_index(*a, *k, *i)] == *v,
                "{name} equals the native verifier's"
            );
        }
        holds_and_proves(&c, &uinst, &uwit, &ugens);
        // An endo value re-derived from other bits (its rows and lookups hold):
        // its raw value no longer equals the shared value.
        let e = 0;
        let chunk = |j: usize| -> u128 {
            let r = uwit.w[c.cell_index(S_ENDO, E_CH + j, e)].to_repr();
            u128::from(u16::from_le_bytes([r[0], r[1]]))
        };
        let raw0: u128 = (0..ENDO_CHUNKS).fold(0, |acc, j| acc | (chunk(j) << (ENDO_CHUNK * j)));
        let mut bits: Vec<bool> = (0..126).map(|i| (raw0 >> i) & 1 == 1).collect();
        bits[0] = !bits[0];
        let raw: u128 = bits
            .iter()
            .enumerate()
            .fold(0, |a, (i, b)| a | ((*b as u128) << i));
        let mut bad = uwit.w.clone();
        for j in 0..ENDO_CHUNKS {
            let ch = ((raw >> (ENDO_CHUNK * j)) & ((1 << ENDO_CHUNK) - 1)) as usize;
            bad[c.cell_index(S_ENDO, E_CH + j, e)] = Fp::from(ch as u64);
            bad[c.cell_index(S_ENDO, E_EV + j, e)] = endo_share(ch);
        }
        bad[c.cell_index(S_ENDO, E_VAL, e)] = endo_scalar::<Fp>(&bits);
        assert!(
            !accepted(&c, &uinst, &ugens, bad),
            "a forged endo challenge is refused"
        );
        // A limb split's value off its shared limbs (the glue's v_A follows it).
        let mut bad = uwit.w.clone();
        bad[c.cell_index(S_LIMB, L_VAL, 1)] += Fp::ONE;
        bad[c.cell_index(S_SIDE + scalar::S_GLUE, scalar::V0, 0)] += Fp::ONE;
        assert!(
            !accepted(&c, &uinst, &ugens, bad),
            "a limb split off its shared limbs is refused"
        );
        // A glue input.
        let mut bad = uwit.w.clone();
        bad[c.cell_index(S_SIDE + scalar::S_GLUE, scalar::R1, 0)] += Fp::ONE;
        assert!(
            !accepted(&c, &uinst, &ugens, bad),
            "a changed challenge input is refused"
        );
    }
}
