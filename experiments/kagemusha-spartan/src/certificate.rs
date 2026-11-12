//! Research-only, current-wire Enrollment certificate authentication.
//!
//! A fixed, trusted root verification key accepts rotated issuer keys. The
//! trusted scheme is a public input, avoiding a scheme/relation identity cycle.
//! The witness contains the exact 108-byte certificate transcript and raw
//! signature; the public output is the canonical SEC1 issuer key and certificate
//! object digest. This does not authenticate an issuer comb table, implement a
//! Credential, or complete Bootstrap. See CERTIFICATE.md for the boundary.

use ff::{Field, PrimeField};
use iroha_pasta::{
    Fp,
    poseidon::{self, PoseidonField},
};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Zero};

use crate::{
    fs::SpikeRng,
    full::{Wiring, constant_entries},
    nonnative::{self, CheckShape, Port},
    p256::{self, Point},
    r1cs::{Instance, Witness},
    sha,
    uniform::{self, Array, Lin, UniformCircuit, constant, link, own, plus, scaled},
};

const SIGN_DOMAIN: u64 = u64::from_le_bytes(*b"kgwcert1");
const OBJECT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwocrt1");
const BODY_BYTES: usize = 108;
const SIGNATURE_BYTES: usize = 64;
const KEY_START: usize = 35;
const SIGNATURE_START: usize = BODY_BYTES;
const MESSAGE_START: usize = BODY_BYTES + SIGNATURE_BYTES;
pub const KEY_IO: usize = 128;
pub const DIGEST_IO: usize = KEY_IO + 65;
pub const SCHEME_IO: usize = DIGEST_IO + 1;
const ROOT_TABLE: u32 = 8;

/// Trusted context: the root is fixed in the VK. The verifier must supply
/// `scheme` from its trusted state/context as public input, not trust a scheme
/// selected by an untrusted proof or credential.
pub struct RootContext {
    pub scheme: [u8; 32],
    pub root: Point,
}

#[derive(Clone)]
pub struct Certificate {
    pub body: [u8; BODY_BYTES],
    pub signature: [u8; SIGNATURE_BYTES],
}

pub(crate) fn fp(x: &BigUint) -> Fp {
    nonnative::fp_u(x)
}
pub(crate) fn pow2(n: usize) -> BigUint {
    BigUint::one() << n
}
pub(crate) fn scalar_bytes(x: &BigUint) -> [u8; 32] {
    let bytes = x.to_bytes_be();
    assert!(bytes.len() <= 32);
    let mut out = [0; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    out
}
pub(crate) fn field_modulus() -> BigUint {
    BigUint::from_bytes_le((-Fp::ONE).to_repr().as_ref()) + 1u32
}
fn items(body: &[u8; BODY_BYTES]) -> Vec<Fp> {
    std::iter::once(Fp::from(BODY_BYTES as u64))
        .chain(
            body.chunks(31)
                .map(|chunk| fp(&BigUint::from_bytes_le(chunk))),
        )
        .collect()
}
pub fn message(cert: &Certificate) -> Fp {
    poseidon::hash_with_domain(SIGN_DOMAIN, &items(&cert.body))
}
pub fn certificate_digest(cert: &Certificate) -> Fp {
    let r = BigUint::from_bytes_be(&cert.signature[..32]);
    let s = BigUint::from_bytes_be(&cert.signature[32..]);
    poseidon::hash_with_domain(
        OBJECT_DOMAIN,
        &[
            message(cert),
            fp(&(&r % pow2(128))),
            fp(&(&r >> 128)),
            fp(&(&s % pow2(128))),
            fp(&(&s >> 128)),
        ],
    )
}

/// A single global row. Range lookups constrain bytes over the integers;
/// widths alone are not range checks.
pub(crate) struct Cells {
    pub(crate) a: Array<Fp>,
}
impl Cells {
    pub(crate) fn new(family: usize) -> Self {
        Self {
            a: Array {
                name: "certificate bytes and integers",
                family,
                rho: 0,
                r_count: 1,
                fixed: false,
                lookups: vec![],
                widths: vec![],
                rows: vec![],
                values: vec![],
            },
        }
    }
    pub(crate) fn cell(&mut self, v: Fp, width: u16) -> usize {
        let k = self.a.values.len();
        self.a.values.push(vec![v]);
        self.a.widths.push(width);
        k
    }
    pub(crate) fn byte(&mut self, b: u8) -> usize {
        let k = self.cell(Fp::from(b as u64), 8);
        nonnative::range_lookups(&mut self.a.lookups, k, 8, 1);
        k
    }
    pub(crate) fn bit(&mut self, b: bool) -> usize {
        let k = self.cell(Fp::from(b as u64), 1);
        self.a.rows.push([own(k, 1), own(k, 1), own(k, 1)]);
        k
    }
    pub(crate) fn zero(&mut self, l: Lin<Fp>) {
        self.a.rows.push([l, constant(vec![Fp::ONE]), vec![]]);
    }
    pub(crate) fn eq(&mut self, a: Lin<Fp>, b: Lin<Fp>) {
        self.zero(plus(&a, &scaled(&b, -Fp::ONE)));
    }
    pub(crate) fn bytes(&self, indices_le: &[usize]) -> Lin<Fp> {
        indices_le.iter().enumerate().fold(vec![], |acc, (i, k)| {
            plus(&acc, &scaled(&own(*k, 1), fp(&pow2(8 * i))))
        })
    }
    pub(crate) fn value(&mut self, v: &BigUint, indices_le: &[usize]) -> [usize; 3] {
        std::array::from_fn(|i| {
            let lo = 11 * i;
            let hi = (lo + 11).min(indices_le.len());
            let width = 8 * (hi - lo);
            let k = self.cell(fp(&((v >> (88 * i)) % pow2(width))), width as u16);
            self.eq(own(k, 1), self.bytes(&indices_le[lo..hi]));
            k
        })
    }
    /// Bytewise addition X + D = bound, with constrained carries and final
    /// carry zero. Invalid X still gets a witness, which fails the final row.
    pub(crate) fn at_most(&mut self, indices_le: &[usize], value: &BigUint, bound: &BigUint) {
        assert_eq!(indices_le.len(), 32);
        let d = (bound + pow2(256) - value) % pow2(256);
        let mut db = d.to_bytes_le();
        db.resize(32, 0);
        let mut bb = bound.to_bytes_le();
        bb.resize(32, 0);
        let mut xb = value.to_bytes_le();
        xb.resize(32, 0);
        let mut carry = false;
        let mut previous = None;
        for i in 0..32 {
            let dk = self.byte(db[i]);
            let next = (xb[i] as u16 + db[i] as u16 + u16::from(carry)) > 255;
            let ck = self.bit(next);
            let mut l = plus(&own(indices_le[i], 1), &own(dk, 1));
            if let Some(k) = previous {
                l = plus(&l, &own(k, 1));
            }
            l = plus(&l, &constant(vec![-Fp::from(bb[i] as u64)]));
            l = plus(&l, &scaled(&own(ck, 1), -Fp::from(256)));
            self.zero(l);
            previous = Some(ck);
            carry = next;
        }
        self.zero(own(previous.unwrap(), 1));
    }
}

fn port(array: usize, first: usize, coefficient: i64) -> Port {
    Port {
        links: vec![(
            array,
            first,
            vec![Some(0)],
            vec![if coefficient < 0 {
                -Fp::from(coefficient.unsigned_abs())
            } else {
                Fp::from(coefficient as u64)
            }],
        )],
        consts: [vec![Fp::ZERO], vec![Fp::ZERO], vec![Fp::ZERO]],
    }
}
fn limbs(x: &BigUint) -> [BigInt; 3] {
    std::array::from_fn(|i| BigInt::from((x >> (88 * i)) % pow2(if i == 2 { 80 } else { 88 })))
}

/// Proves x² = x2 and y² - x*x2 + coefficient*x - b = residue (mod p), then
/// residue = 0. The residual witness lets an off-curve input reach the R1CS
/// rejection without relying on a host-side point parser.
pub(crate) fn curve_relation(
    w: &mut Wiring<Fp>,
    gfam: usize,
    a: usize,
    cells: [[usize; 3]; 4],
    values: [&BigUint; 4],
    modulus: &BigUint,
    linear_coefficient: u64,
    curve_b: &BigUint,
) {
    let [xc, yc, x2c, rc] = cells;
    let [x, y, x2, residue] = values;
    let bound = [pow2(88) - 1u32, pow2(88) - 1u32, pow2(80) - 1u32];
    let shape1 = CheckShape::new(
        &[(bound.clone(), bound.clone())],
        &bound,
        0,
        &[modulus.clone()],
    );
    let (rows, lookups) = shape1.template(
        &[port(a, xc[0], 1)],
        &[port(a, xc[0], 1)],
        &[vec![Fp::ONE]],
        &port(a, x2c[0], -1),
        &[Fp::ZERO],
        &[0],
        1,
    );
    let vals = shape1.witness(&[limbs(x)], &[limbs(x)], &[1], &limbs(x2).map(|v| -v), 0, 0);
    w.push(Array {
        name: "certificate issuer square",
        family: gfam,
        rho: 0,
        r_count: 1,
        fixed: false,
        rows,
        lookups,
        widths: shape1.widths(),
        values: vals.into_iter().map(|v| vec![v]).collect(),
    });
    let lb = bound.clone().map(|v| v * (linear_coefficient + 2));
    let shape2 = CheckShape::new(
        &[(bound.clone(), bound.clone()), (bound.clone(), bound)],
        &lb,
        0,
        &[modulus.clone()],
    );
    let b = limbs(curve_b);
    let mut linear = port(a, xc[0], linear_coefficient as i64);
    linear.links.extend(port(a, rc[0], -1).links);
    linear.consts = std::array::from_fn(|i| vec![-fp(b[i].magnitude())]);
    let (rows, lookups) = shape2.template(
        &[port(a, yc[0], 1), port(a, xc[0], 1)],
        &[port(a, yc[0], 1), port(a, x2c[0], 1)],
        &[vec![Fp::ONE], vec![-Fp::ONE]],
        &linear,
        &[Fp::ZERO],
        &[0],
        1,
    );
    let xl = limbs(x);
    let rl = limbs(residue);
    let lin = std::array::from_fn(|i| &xl[i] * linear_coefficient - &b[i] - &rl[i]);
    let vals = shape2.witness(
        &[limbs(y), limbs(x)],
        &[limbs(y), limbs(x2)],
        &[1, -1],
        &lin,
        0,
        0,
    );
    w.push(Array {
        name: "certificate issuer curve",
        family: gfam,
        rho: 0,
        r_count: 1,
        fixed: false,
        rows,
        lookups,
        widths: shape2.widths(),
        values: vals.into_iter().map(|v| vec![v]).collect(),
    });
}

/// Exact native sponge: state [2^64,0,0], rate lanes 1/2, one padding word,
/// output lane 1. The synthetic operation pool has different lane conventions
/// and must not be substituted for this transcript hash.
pub(crate) fn hash_relation(
    w: &mut Wiring<Fp>,
    gfam: usize,
    local: usize,
    domain: u64,
    elements: &[(Fp, Lin<Fp>)],
    output_cells: [usize; 3],
    rng: &mut SpikeRng,
) {
    let mut terms = vec![
        (Fp::from(domain), constant(vec![Fp::from(domain)])),
        (
            Fp::from(elements.len() as u64),
            constant(vec![Fp::from(elements.len() as u64)]),
        ),
    ];
    terms.extend_from_slice(elements);
    terms.push((Fp::ONE, constant(vec![Fp::ONE])));
    if terms.len() % 2 != 0 {
        terms.push((Fp::ZERO, constant(vec![Fp::ZERO])));
    }
    let n = terms.len() / 2;
    let log = n.next_power_of_two().trailing_zeros() as usize;
    let initial = [Fp::from_u128(1u128 << 64), Fp::ZERO, Fp::ZERO];
    let mut state = initial;
    let mut inputs = vec![];
    for pair in terms.chunks(2) {
        state[1] += pair[0].0;
        state[2] += pair[1].0;
        inputs.push(state);
        poseidon::permute(&mut state);
    }
    // The compiler packs byte range checks into chunk arrays. Copy exports
    // must read stable field cells, so materialize each packed integer and
    // constrain its equality to the byte expression before exporting it.
    let sources: Vec<usize> = terms
        .iter()
        .map(|(value, l)| {
            let k = w.arrays[local].values.len();
            w.arrays[local].values.push(vec![*value]);
            w.arrays[local].widths.push(255);
            w.arrays[local].rows.push([
                plus(&own(k, 1), &scaled(l, -Fp::ONE)),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
            w.export(local, vec![own(k, 1)])
        })
        .collect();
    let start = constant_entries(
        w,
        gfam,
        "certificate sponge initial state",
        &[initial.to_vec()],
    );
    let at = w.arrays.len();
    let fam = w.pi.len();
    let (mut arrays, pis, outputs) = uniform::rp57_with(log, &inputs, rng);
    uniform::relocate(&mut arrays, at, fam);
    w.pi.extend(pis);
    w.counts.extend([n, 2]);
    let slots = 1usize << log;
    let input = &mut arrays[uniform::RP_INPUT];
    input.values.extend((0..2).map(|j| {
        (0..slots)
            .map(|i| terms.get(2 * i + j).map_or(Fp::ZERO, |t| t.0))
            .collect::<Vec<_>>()
    }));
    input.widths.extend([255, 255]);
    w.arrays.extend(arrays);
    let mds = <Fp as PoseidonField>::rp57().mds();
    let lane = |k: usize| {
        (0..3).fold(vec![], |acc, j| {
            plus(
                &acc,
                &link(at + uniform::RP_FULL, 3 * j + 2, vec![Some(7)], mds[k][j]),
            )
        })
    };
    let ip = at + uniform::RP_INPUT;
    let out = w.export(ip, vec![lane(0), lane(1), lane(2)]);
    w.import(
        ip,
        vec![
            own(0, 1),
            plus(&own(1, 1), &scaled(&own(3, 1), -Fp::ONE)),
            plus(&own(2, 1), &scaled(&own(4, 1), -Fp::ONE)),
        ],
        (0..slots)
            .map(|i| {
                if i == 0 {
                    Some((start, 0))
                } else if i < n {
                    Some((out, i - 1))
                } else {
                    None
                }
            })
            .collect(),
    );
    for j in 0..2 {
        w.import(
            ip,
            vec![own(3 + j, 1)],
            (0..slots)
                .map(|i| sources.get(2 * i + j).map(|e| (*e, 0)))
                .collect(),
        );
    }
    w.import(
        local,
        output_cells.iter().map(|k| own(*k, 1)).collect(),
        vec![Some((out, n - 1))],
    );
    for (k, v) in output_cells.into_iter().zip(outputs[n - 1]) {
        w.arrays[local].values[k][0] = v;
    }
}

pub struct Built {
    pub circuit: UniformCircuit<Fp>,
    pub instance: Instance<Fp>,
    pub witness: Witness<Fp>,
    local: usize,
    message_cell: usize,
    digest_cell: usize,
    sha: usize,
}

/// Builds a witness for a root-verifiable signature. Semantic invalidity
/// (wrong role/scheme/version, bad delegated point, high S) is rejected by
/// constraints, not this builder. Invalid ECDSA scalars/signatures have no
/// comb witness and return an error before proving.
pub(crate) struct Prepared {
    pub wiring: Wiring<Fp>,
    pub gfam: usize,
    pub local: usize,
    pub issuer_x: [usize; 3],
    pub issuer_y: [usize; 3],
    pub message_cell: usize,
    pub digest_cell: usize,
    pub sha: usize,
}

pub(crate) fn prepare(context: &RootContext, cert: &Certificate) -> Result<Prepared, &'static str> {
    if context.scheme == [0; 32]
        || context.root.x >= p256::p()
        || context.root.y >= p256::p()
        || !p256::on_curve(&context.root)
    {
        return Err("invalid trusted root context");
    }
    let m = message(cert);
    let mb = m.to_repr();
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(mb.as_ref()));
    let r = BigUint::from_bytes_be(&cert.signature[..32]);
    let s = BigUint::from_bytes_be(&cert.signature[32..]);
    if !p256::verify(
        &context.root,
        &z,
        &p256::Signature {
            r: r.clone(),
            s: s.clone(),
        },
    ) {
        return Err("signature has no valid ECDSA witness");
    }
    let digest = certificate_digest(cert);
    let mut w = Wiring::default();
    let gfam = w.family(0, 1);
    let mut c = Cells::new(gfam);
    for byte in cert.body.iter().chain(&cert.signature).chain(mb.as_ref()) {
        c.byte(*byte);
    }
    // Fixed wire version, Enrollment role and SEC1 tag.
    for (k, v) in [(0, 1), (1, 0), (34, 1), (KEY_START, 4)].into_iter() {
        c.eq(own(k, 1), constant(vec![Fp::from(v as u64)]));
    }
    let scheme_sum = (2..34).fold(vec![], |acc, k| plus(&acc, &own(k, 1)));
    let scheme_value = Fp::from(cert.body[2..34].iter().map(|b| *b as u64).sum::<u64>());
    let inverse = c.cell(
        Option::<Fp>::from(scheme_value.invert()).unwrap_or(Fp::ZERO),
        255,
    );
    c.a.rows
        .push([scheme_sum, own(inverse, 1), constant(vec![Fp::ONE])]);
    let x = BigUint::from_bytes_be(&cert.body[36..68]);
    let y = BigUint::from_bytes_be(&cert.body[68..100]);
    let xle: Vec<_> = (36..68).rev().collect();
    let yle: Vec<_> = (68..100).rev().collect();
    c.at_most(&xle, &x, &(p256::p() - 1u32));
    c.at_most(&yle, &y, &(p256::p() - 1u32));
    let xc = c.value(&x, &xle);
    let yc = c.value(&y, &yle);
    let x2 = &x * &x % p256::p();
    let residue =
        (&y * &y + &x * 3u32 + p256::p() * 2u32 - ((&x * &x2 + p256::b()) % p256::p())) % p256::p();
    let mut aux = |v: &BigUint| {
        let indices: Vec<_> = scalar_bytes(v).iter().rev().map(|b| c.byte(*b)).collect();
        c.value(v, &indices)
    };
    let x2c = aux(&x2);
    let rc = aux(&residue);
    for k in rc {
        c.zero(own(k, 1));
    }
    let rle: Vec<_> = (SIGNATURE_START..SIGNATURE_START + 32).rev().collect();
    let sle: Vec<_> = (SIGNATURE_START + 32..SIGNATURE_START + 64).rev().collect();
    c.at_most(&sle, &s, &(p256::n() >> 1));
    let rlimbs = c.value(&r, &rle);
    let slimbs = c.value(&s, &sle);
    let mle: Vec<_> = (MESSAGE_START..MESSAGE_START + 32).collect();
    c.at_most(
        &mle,
        &BigUint::from_bytes_le(mb.as_ref()),
        &(field_modulus() - 1u32),
    );
    let mcell = c.cell(m, 255);
    c.eq(own(mcell, 1), c.bytes(&mle));
    let mstate = [c.cell(Fp::ZERO, 255), mcell, c.cell(Fp::ZERO, 255)];
    let dcell = c.cell(digest, 255);
    let dstate = [c.cell(Fp::ZERO, 255), dcell, c.cell(Fp::ZERO, 255)];
    let body_items: Vec<_> = std::iter::once((Fp::from(108), constant(vec![Fp::from(108)])))
        .chain(cert.body.chunks(31).enumerate().map(|(i, b)| {
            (
                fp(&BigUint::from_bytes_le(b)),
                c.bytes(&(31 * i..31 * i + b.len()).collect::<Vec<_>>()),
            )
        }))
        .collect();
    let obj_items = vec![
        (m, own(mcell, 1)),
        (fp(&(&r % pow2(128))), c.bytes(&rle[..16])),
        (fp(&(&r >> 128)), c.bytes(&rle[16..])),
        (fp(&(&s % pow2(128))), c.bytes(&sle[..16])),
        (fp(&(&s >> 128)), c.bytes(&sle[16..])),
    ];
    let local = w.push(c.a);
    curve_relation(
        &mut w,
        gfam,
        local,
        [xc, yc, x2c, rc],
        [&x, &y, &x2, &residue],
        &p256::p(),
        3,
        &p256::b(),
    );
    let mut rng = SpikeRng::new(b"certificate exact native sponge");
    hash_relation(
        &mut w,
        gfam,
        local,
        SIGN_DOMAIN,
        &body_items,
        mstate,
        &mut rng,
    );
    hash_relation(
        &mut w,
        gfam,
        local,
        OBJECT_DOMAIN,
        &obj_items,
        dstate,
        &mut rng,
    );
    assert_eq!(w.arrays[local].values[mcell][0], m);
    assert_eq!(w.arrays[local].values[dcell][0], digest);
    let (sha_at, _) = add_signature(
        &mut w,
        gfam,
        local,
        MESSAGE_START,
        m,
        (context.root.clone(), z, r, s),
        [rlimbs, slimbs],
        nonnative::KeyTable::Fixed { id: ROOT_TABLE },
    );
    Ok(Prepared {
        wiring: w,
        gfam,
        local,
        issuer_x: xc,
        issuer_y: yc,
        message_cell: mcell,
        digest_cell: dcell,
        sha: sha_at,
    })
}

/// Standalone public certificate outputs. Compositions use the private ports
/// and bind their own public context rather than duplicating these IO bytes.
pub(crate) fn expose(
    w: &mut Wiring<Fp>,
    gfam: usize,
    local: usize,
    dcell: usize,
    context: &RootContext,
    cert: &Certificate,
) {
    let digest = certificate_digest(cert);
    // Public output cells are linked explicitly to private cells before any
    // copy exports. Copy helpers do not read public io cells as witnesses.
    let mut out = vec![Fp::ZERO; 128];
    for i in 0..65 {
        out[i] = Fp::from(cert.body[KEY_START + i] as u64);
    }
    out[65] = digest;
    for (i, byte) in context.scheme.iter().enumerate() {
        out[66 + i] = Fp::from(*byte as u64);
    }
    let io = w.push(Array {
        name: "authenticated issuer and certificate digest",
        family: gfam,
        rho: 7,
        r_count: 98,
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![out],
    });
    w.io_arrays.push((io, KEY_IO));
    for (k, index) in (0..65)
        .map(|i| (KEY_START + i, i))
        .chain(std::iter::once((dcell, 65)))
        .chain((0..32).map(|i| (2 + i, 66 + i)))
    {
        w.arrays[local].rows.push([
            plus(
                &own(k, 1),
                &scaled(&link(io, 0, vec![Some(index)], Fp::ONE), -Fp::ONE),
            ),
            constant(vec![Fp::ONE]),
            vec![],
        ]);
    }
}

/// Exact canonical-message bytes -> SHA-256 -> ECDSA, with signature scalars
/// linked to their raw-byte limbs. The caller supplies low-S/encoding constraints
/// and authenticates the chosen key table.
pub(crate) fn add_signature(
    w: &mut Wiring<Fp>,
    gfam: usize,
    local: usize,
    message_start: usize,
    m: Fp,
    input: (Point, BigUint, BigUint, BigUint),
    scalar_cells: [[usize; 3]; 2],
    key_table: nonnative::KeyTable,
) -> (usize, usize) {
    let (q, z, r, s) = input;
    let [rlimbs, slimbs] = scalar_cells;
    let sfam = w.family(1, 1);
    let sha_at = w.arrays.len();
    let sh = sha::sha(sha_at, sfam, 1, gfam, &[m]);
    w.arrays.extend(sh.arrays);
    w.tables.extend(sh.tables);
    // Bind all SHA message words to the same canonical 32 little-endian
    // bytes that recompose the signed Poseidon field element.
    let mut word: Lin<Fp> = vec![];
    for byte in 0..4 {
        for j in 0..8 {
            let mut map = vec![None; 8];
            map[j] = Some(0);
            word = plus(
                &word,
                &link(
                    local,
                    message_start + 4 * j + byte,
                    map,
                    Fp::from(1u64 << (8 * (3 - byte))),
                ),
            );
        }
    }
    w.arrays[sha_at + sha::A_MSG].rows.push([
        plus(&own(0, 8), &scaled(&word, -Fp::ONE)),
        constant(vec![Fp::ONE; 8]),
        vec![],
    ]);
    let comb_at = w.arrays.len();
    let comb =
        nonnative::comb_with_key_table(comb_at, sfam, 1, gfam, &[(q, z.clone(), r, s)], key_table);
    w.arrays.extend(comb.arrays);
    w.tables.extend(comb.tables);
    crate::wired::digest_limbs(
        &mut w.arrays,
        sha_at + sha::A_OUT,
        comb_at + nonnative::A_VALUE,
        &[z],
    );
    let av = comb_at + nonnative::A_VALUE;
    let nr = 1 << w.arrays[av].rho;
    for (row, limbs) in [
        (nonnative::Z_INSTANCE - 2, rlimbs),
        (nonnative::Z_INSTANCE - 1, slimbs),
    ] {
        let mut select = vec![Fp::ZERO; nr];
        select[row] = Fp::ONE;
        let mut map = vec![None; nr];
        map[row] = Some(0);
        for (k, source) in limbs.into_iter().enumerate() {
            w.arrays[av].rows.push([
                constant(select.clone()),
                plus(
                    &own(k, nr),
                    &scaled(&link(local, source, map.clone(), Fp::ONE), -Fp::ONE),
                ),
                vec![],
            ]);
        }
    }
    (sha_at, comb_at)
}

pub fn build(context: &RootContext, cert: &Certificate) -> Result<Built, &'static str> {
    let Prepared {
        mut wiring,
        gfam,
        local,
        message_cell,
        digest_cell,
        sha,
        ..
    } = prepare(context, cert)?;
    expose(&mut wiring, gfam, local, digest_cell, context, cert);
    let (arrays, pi, counts, tables, copies) = wiring.finish();
    let (circuit, instance, witness) = UniformCircuit::build_with_copies(
        "enrollment-certificate",
        arrays,
        pi,
        counts,
        tables,
        copies,
        14,
        256,
        b"enrollment-certificate",
    );
    Ok(Built {
        circuit,
        instance,
        witness,
        local,
        message_cell,
        digest_cell,
        sha,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ipa::Gens, r1cs::Circuit, spartan};
    use iroha_pasta::Eq as Vesta;
    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
    fn root() -> RootContext {
        RootContext {
            scheme: hex("9692c8c83bef03c3991f61516a22cdb560ba5a980bfd4a9c3f27339c9d2dd445")
                .try_into()
                .unwrap(),
            root: p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[0x11; 32])).unwrap(),
        }
    }
    fn resign(cert: &mut Certificate, d: &BigUint) {
        let z = BigUint::from_bytes_be(&p256::sha256_one_block(message(cert).to_repr().as_ref()));
        let nonce = p256::scalar_below("research exact certificate nonce", &p256::n());
        let mut s = p256::sign(d, &z, &nonce);
        if s.s > p256::n() >> 1 {
            s.s = p256::n() - s.s;
        }
        cert.signature[..32].copy_from_slice(&scalar_bytes(&s.r));
        cert.signature[32..].copy_from_slice(&scalar_bytes(&s.s));
    }
    fn cert(which: u8) -> Certificate {
        let q = p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[which; 32])).unwrap();
        let mut c = Certificate {
            body: [0; 108],
            signature: [0; 64],
        };
        c.body[..2].copy_from_slice(&1u16.to_le_bytes());
        c.body[2..34].copy_from_slice(&root().scheme);
        c.body[34] = 1;
        c.body[35] = 4;
        c.body[36..68].copy_from_slice(&scalar_bytes(&q.x));
        c.body[68..100].copy_from_slice(&scalar_bytes(&q.y));
        c.body[100..].copy_from_slice(&9u64.to_le_bytes());
        resign(&mut c, &BigUint::from_bytes_be(&[0x11; 32]));
        c
    }
    fn holds(b: &Built) -> bool {
        let mut witness = b.witness.w.clone();
        let rel = b.circuit.instantiate(
            &b.circuit.shape,
            &[Fp::from(31), Fp::from(47)],
            Some(&mut witness),
        );
        let z = crate::r1cs::z_vector(&b.circuit.shape, &b.instance, &Witness { w: witness });
        let (a, b, c) = rel.abc(&b.circuit.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    #[test]
    fn canonical_message_rejects_the_field_modulus_alias() {
        for alias in [false, true] {
            let m = BigUint::from(13u32);
            let encoded = if alias {
                &m + field_modulus()
            } else {
                m.clone()
            };
            let mut cells = Cells::new(0);
            let bytes: Vec<_> = scalar_bytes(&encoded)
                .iter()
                .rev()
                .map(|b| cells.byte(*b))
                .collect();
            // Both encodings recompose to the same field value. Only the
            // integer bound distinguishes them, as required before SHA-256.
            cells.eq(cells.bytes(&bytes), constant(vec![fp(&m)]));
            cells.at_most(&bytes, &encoded, &(field_modulus() - 1u32));
            let range = Array {
                name: "canonical byte test range",
                family: 0,
                rho: 11,
                r_count: 2048,
                fixed: true,
                lookups: vec![],
                widths: vec![11],
                rows: vec![],
                values: vec![(0..2048).map(|i| Fp::from(i as u64)).collect()],
            };
            let (circuit, instance, witness) = UniformCircuit::build(
                "canonical certificate message",
                vec![cells.a, range],
                vec![0],
                vec![1],
                vec![(1, nonnative::RANGE_TABLE)],
                12,
                4,
                b"canonical certificate message",
            );
            let built = Built {
                circuit,
                instance,
                witness,
                local: 0,
                message_cell: 0,
                digest_cell: 0,
                sha: 0,
            };
            assert_eq!(
                holds(&built),
                !alias,
                "m + field modulus is a forbidden SHA input"
            );
        }
    }
    #[test]
    fn exact_current_wire_fixture() {
        let mut c = cert(0x26);
        assert_eq!(
            message(&c).to_repr().as_ref(),
            hex("10d3e2b33cdd065f3087ad6f19a47523e14d68210a5394c88ed323d613abc81b")
        );
        c.signature=hex("233c5d3368f8d5f665d43cccc6bc1fbd5df9c1139d9f544f5c39ee041a85ad45734b8dfaa48dedae20d1d3ce4b3e924739a83618b7e62c0ec33a42c71f065c93").try_into().unwrap();
        assert_eq!(
            certificate_digest(&c).to_repr().as_ref(),
            hex("e528d6121f48dedcbc8dad69eb4a8f1f9c3ef2b6886b4e89f7d078d4e0ebdc21")
        );
        assert!(holds(&build(&root(), &c).unwrap()));
    }
    #[test]
    fn certificate_semantics_and_tampering_are_constrained() {
        let ctx = root();
        let honest = cert(0x26);
        assert!(holds(&build(&ctx, &honest).unwrap()));
        // Correct root signatures do not waive role, scheme, version or SEC1 validity.
        for (label, index, value) in [
            ("role", 34, 3),
            ("scheme", 2, 0),
            ("version", 0, 2),
            ("SEC1 tag", 35, 3),
            ("off curve", 70, 0),
        ] {
            let mut c = honest.clone();
            c.body[index] = value;
            resign(&mut c, &BigUint::from_bytes_be(&[0x11; 32]));
            assert!(!holds(&build(&ctx, &c).unwrap()), "{label}");
        }
        let mut high = honest.clone();
        let s = BigUint::from_bytes_be(&high.signature[32..]);
        high.signature[32..].copy_from_slice(&scalar_bytes(&(p256::n() - s)));
        assert!(
            !holds(&build(&ctx, &high).unwrap()),
            "high S is rejected in circuit"
        );
        for (label, index) in [("key", 36), ("serial", 100)] {
            let mut c = honest.clone();
            c.body[index] ^= 1;
            assert!(build(&ctx, &c).is_err(), "{label}");
        }
        for value in [BigUint::zero(), p256::n()] {
            for offset in [0, 32] {
                let mut c = honest.clone();
                c.signature[offset..offset + 32].copy_from_slice(&scalar_bytes(&value));
                assert!(build(&ctx, &c).is_err(), "zero or out-of-range scalar");
            }
        }
        let mut nc = honest.clone();
        nc.body[36..68].copy_from_slice(&scalar_bytes(&p256::p()));
        resign(&mut nc, &BigUint::from_bytes_be(&[0x11; 32]));
        assert!(!holds(&build(&ctx, &nc).unwrap()), "coordinate equal to p");
        let mut bad_signature = honest.clone();
        bad_signature.signature[13] ^= 1;
        assert!(build(&ctx, &bad_signature).is_err());
        let mut b = build(&ctx, &honest).unwrap();
        for k in [
            36,
            100,
            SIGNATURE_START,
            MESSAGE_START,
            b.message_cell,
            b.digest_cell,
        ] {
            let pos = b.circuit.cell_index(b.local, k, 0);
            b.witness.w[pos] += Fp::ONE;
            assert!(!holds(&b), "private byte or digest cell {k}");
            b.witness.w[pos] -= Fp::ONE;
        }
        let pos = b.circuit.cell_index(b.sha + sha::A_MSG, 0, 0);
        b.witness.w[pos] += Fp::ONE;
        assert!(!holds(&b), "SHA word must read the canonical message bytes");
        b.witness.w[pos] -= Fp::ONE;
        for i in [KEY_IO + 1, DIGEST_IO, SCHEME_IO] {
            b.instance.io[i] += Fp::ONE;
            assert!(!holds(&b), "public authenticated output {i}");
            b.instance.io[i] -= Fp::ONE;
        }
    }
    #[test]
    fn two_rotated_issuers_prove_under_the_same_root_vk() {
        let a = build(&root(), &cert(0x26)).unwrap();
        let b = build(&root(), &cert(0x22)).unwrap();
        let mut other_context = root();
        other_context.scheme = [0x77; 32];
        let mut other_certificate = cert(0x26);
        other_certificate.body[2..34].copy_from_slice(&other_context.scheme);
        resign(&mut other_certificate, &BigUint::from_bytes_be(&[0x11; 32]));
        let c = build(&other_context, &other_certificate).unwrap();
        let challenge = [Fp::from(31), Fp::from(47)];
        assert_eq!(
            a.circuit.entries(&challenge),
            b.circuit.entries(&challenge),
            "same constraint matrix"
        );
        assert_eq!(
            a.circuit.entries(&challenge),
            c.circuit.entries(&challenge),
            "scheme is a carried public input under the same root VK"
        );
        assert_ne!(
            a.instance.io[KEY_IO + 1..KEY_IO + 65],
            b.instance.io[KEY_IO + 1..KEY_IO + 65]
        );
        let gens = Gens::<Vesta>::new(a.circuit.shape.cols());
        let mut rng = SpikeRng::new(b"certificate rotation proof");
        for witness in [&a, &b, &c] {
            let (proof, _) = spartan::prove_circuit(
                &a.circuit.shape,
                &gens,
                &witness.instance,
                &witness.witness,
                &mut rng,
                &a.circuit,
            );
            assert!(
                spartan::verify_circuit(
                    &a.circuit.shape,
                    &gens,
                    &witness.instance,
                    &proof.to_bytes(),
                    &a.circuit
                )
                .is_ok()
            );
            for index in [KEY_IO + 1, DIGEST_IO, SCHEME_IO] {
                let mut wrong = Instance {
                    io: witness.instance.io.clone(),
                };
                wrong.io[index] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(
                        &a.circuit.shape,
                        &gens,
                        &wrong,
                        &proof.to_bytes(),
                        &a.circuit
                    )
                    .is_err()
                );
            }
        }
        let mut high = cert(0x26);
        let s = BigUint::from_bytes_be(&high.signature[32..]);
        high.signature[32..].copy_from_slice(&scalar_bytes(&(p256::n() - s)));
        let high = build(&root(), &high).unwrap();
        let (proof, _) = spartan::prove_circuit(
            &a.circuit.shape,
            &gens,
            &high.instance,
            &high.witness,
            &mut rng,
            &a.circuit,
        );
        assert!(
            spartan::verify_circuit(
                &a.circuit.shape,
                &gens,
                &high.instance,
                &proof.to_bytes(),
                &a.circuit,
            )
            .is_err(),
            "valid ECDSA high-S twin is rejected under the same VK"
        );
        let d = BigUint::from_bytes_be(&[0x33; 32]);
        let mut different = root();
        different.root = p256::mul(&p256::generator(), &d).unwrap();
        let mut wrong = cert(0x26);
        resign(&mut wrong, &d);
        assert!(
            build(&root(), &wrong).is_err(),
            "wrong root cannot build a valid witness"
        );
        let foreign = build(&different, &wrong).unwrap();
        let (proof, _) = spartan::prove_circuit(
            &foreign.circuit.shape,
            &gens,
            &foreign.instance,
            &foreign.witness,
            &mut rng,
            &foreign.circuit,
        );
        assert!(
            spartan::verify_circuit(
                &a.circuit.shape,
                &gens,
                &foreign.instance,
                &proof.to_bytes(),
                &a.circuit
            )
            .is_err(),
            "different root fixed table changes VK"
        );
    }
}
