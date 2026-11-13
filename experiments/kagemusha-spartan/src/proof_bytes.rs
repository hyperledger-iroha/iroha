//! Exact compressed proof bytes joined to the two recursive verifier halves.
//!
//! Both circuits bind the same sixteen-byte words through the pair binding.
//! Each half authenticates its native points and the other proof's native
//! scalars. The primary hashes the entire fixed-layout pair, including final
//! Schnorr responses. No transcript digest substitutes for those bytes.

use ff::{Field, PrimeField};
use iroha_pasta::Fp;
use num_bigint::BigUint;

use crate::{
    fs::Scalar,
    full::{VerifierCell, Wiring},
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};

pub struct SharedBytes {
    pub array: usize,
    pub len: usize,
}
impl SharedBytes {
    pub fn byte(&self, i: usize) -> VerifierCell {
        assert!(i < self.len);
        VerifierCell {
            array: self.array,
            cell: 0,
            instance: i,
        }
    }
    pub(crate) fn packed<S: Scalar>(&self, start: usize, len: usize) -> Lin<S> {
        (0..len).fold(Vec::new(), |acc, i| {
            plus(
                &acc,
                &link(
                    self.array,
                    0,
                    vec![Some(start + i)],
                    S::from(256).pow_vartime([i as u64]),
                ),
            )
        })
    }
}

/// Append the common exact byte sequence to the cross-field binding vector.
pub fn shared_words(bytes: &[u8]) -> Vec<u128> {
    bytes
        .chunks(16)
        .map(|chunk| {
            let mut b = [0; 16];
            b[..chunk.len()].copy_from_slice(chunk);
            u128::from_le_bytes(b)
        })
        .collect()
}

fn ensure_range<S: Scalar>(w: &mut Wiring<S>, family: usize) {
    if w.tables
        .iter()
        .any(|(_, id)| *id == crate::nonnative::RANGE_TABLE)
    {
        return;
    }
    let array = w.push(Array {
        name: "proof encoding byte range",
        family,
        rho: 11,
        r_count: 2048,
        fixed: true,
        lookups: Vec::new(),
        widths: vec![11],
        rows: Vec::new(),
        values: vec![(0..2048).map(|i| S::from(i as u64)).collect()],
    });
    w.tables.push((array, crate::nonnative::RANGE_TABLE));
}
fn range<S: Scalar>(cell: usize, width: usize, n: usize) -> Vec<(u32, Vec<Lin<S>>)> {
    assert!((1..=11).contains(&width));
    let table = crate::nonnative::RANGE_TABLE;
    vec![
        (table, vec![own(cell, n)]),
        (
            table,
            vec![scaled(&own(cell, n), S::from(1u64 << (11 - width)))],
        ),
    ]
}
fn byte_array<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    bytes: &[u8],
    name: &'static str,
) -> SharedBytes {
    ensure_range(w, family);
    let n = bytes.len().max(1).next_power_of_two();
    let mut values = vec![S::ZERO; n];
    for (i, b) in bytes.iter().enumerate() {
        values[i] = S::from(*b as u64);
    }
    let array = w.push(Array {
        name,
        family,
        rho: n.trailing_zeros() as usize,
        r_count: bytes.len(),
        fixed: false,
        lookups: range(0, 8, n),
        widths: vec![8],
        rows: Vec::new(),
        values: vec![values],
    });
    SharedBytes {
        array,
        len: bytes.len(),
    }
}

pub fn append_shared<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    bytes: &[u8],
    shared: usize,
    start: usize,
) -> SharedBytes {
    let raw = byte_array(w, family, bytes, "exact pair bytes");
    let words = shared_words(bytes);
    let m = words.len().max(1).next_power_of_two();
    let mut row = link(
        shared,
        0,
        (0..m)
            .map(|i| (i < words.len()).then_some(start + i))
            .collect(),
        -S::ONE,
    );
    for b in 0..16 {
        row = plus(
            &row,
            &link(
                raw.array,
                0,
                (0..m)
                    .map(|i| (16 * i + b < bytes.len()).then_some(16 * i + b))
                    .collect(),
                S::from(256).pow_vartime([b as u64]),
            ),
        );
    }
    w.push(Array {
        name: "exact pair shared word bindings",
        family,
        rho: m.trailing_zeros() as usize,
        r_count: words.len(),
        fixed: false,
        lookups: Vec::new(),
        widths: vec![1],
        rows: vec![[row, constant(vec![S::ONE; m]), Vec::new()]],
        values: vec![vec![S::ZERO; m]],
    });
    raw
}

fn source<S: Scalar>(s: VerifierCell) -> Lin<S> {
    link(s.array, s.cell, vec![Some(s.instance)], S::ONE)
}
fn value<S: Scalar>(w: &Wiring<S>, s: VerifierCell) -> S {
    w.arrays[s.array].values[s.cell][s.instance]
}

/// Bind an exact canonical scalar representation. The integer comparison is
/// essential: a modular field equality alone would also admit x + modulus.
pub fn bind_scalar<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    bytes: &SharedBytes,
    start: usize,
    scalar: VerifierCell,
) {
    assert!(start.checked_add(32).is_some_and(|end| end <= bytes.len));
    ensure_range(w, family);
    let repr = value(w, scalar).to_repr();
    let pminusone = (-S::ONE).to_repr();
    let difference = BigUint::from_bytes_le(&pminusone) - BigUint::from_bytes_le(&repr);
    let mut diff = difference.to_bytes_le();
    diff.resize(32, 0);
    let array = w.arrays.len();
    let mut values = vec![vec![S::ZERO; 32]; 2];
    let mut carry = false;
    for i in 0..32 {
        values[0][i] = S::from(diff[i] as u64);
        carry = (repr[i] as u16 + diff[i] as u16 + u16::from(carry)) > 255;
        values[1][i] = S::from(carry as u64);
    }
    let raw = link(
        bytes.array,
        0,
        (0..32).map(|i| Some(start + i)).collect(),
        S::ONE,
    );
    let prev = link(
        array,
        1,
        (0usize..32).map(|i| i.checked_sub(1)).collect(),
        S::ONE,
    );
    let difference = plus(&plus(&raw, &own(0, 32)), &prev);
    let expected = plus(
        &constant(pminusone.iter().map(|b| S::from(*b as u64)).collect()),
        &scaled(&own(1, 32), S::from(256)),
    );
    w.push(Array {
        name: "canonical proof scalar byte carries",
        family,
        rho: 5,
        r_count: 32,
        fixed: false,
        lookups: range(0, 8, 32),
        widths: vec![8, 1],
        rows: vec![
            [
                plus(&difference, &scaled(&expected, -S::ONE)),
                constant(vec![S::ONE; 32]),
                Vec::new(),
            ],
            [
                own(1, 32),
                plus(&own(1, 32), &constant(vec![-S::ONE; 32])),
                Vec::new(),
            ],
            [
                vec![crate::uniform::Term {
                    src: crate::uniform::Src::Own(1),
                    coef: (0..32)
                        .map(|i| if i == 31 { S::ONE } else { S::ZERO })
                        .collect(),
                }],
                constant(vec![S::ONE; 32]),
                Vec::new(),
            ],
        ],
        values,
    });
    w.push(Array {
        name: "canonical proof scalar field join",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![1],
        values: vec![vec![S::ZERO]],
        rows: vec![[
            plus(&bytes.packed(start, 32), &scaled(&source(scalar), -S::ONE)),
            constant(vec![S::ONE]),
            Vec::new(),
        ]],
    });
}

/// Exact compressed Pasta encoding: canonical x with the canonical y parity
/// in bit255. The identity's constrained coordinates (0,0) encode as zero.
pub fn bind_point<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    bytes: &SharedBytes,
    start: usize,
    coordinates: [VerifierCell; 2],
) {
    assert!(start.checked_add(32).is_some_and(|end| end <= bytes.len));
    let x = value(w, coordinates[0]).to_repr();
    let y = value(w, coordinates[1]).to_repr();
    let xraw = byte_array(w, family, &x, "point x canonical bytes");
    let yraw = byte_array(w, family, &y, "point y canonical bytes");
    bind_scalar(w, family, &xraw, 0, coordinates[0]);
    bind_scalar(w, family, &yraw, 0, coordinates[1]);
    let parity = w.push(Array {
        name: "canonical point y parity",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: range(1, 7, 1),
        widths: vec![1, 7],
        values: vec![
            vec![S::from((y[0] & 1) as u64)],
            vec![S::from((y[0] >> 1) as u64)],
        ],
        rows: vec![
            [
                own(0, 1),
                plus(&own(0, 1), &constant(vec![-S::ONE])),
                Vec::new(),
            ],
            [
                plus(
                    &source(yraw.byte(0)),
                    &scaled(&plus(&own(0, 1), &scaled(&own(1, 1), S::from(2))), -S::ONE),
                ),
                constant(vec![S::ONE]),
                Vec::new(),
            ],
        ],
    });
    let encoded = link(
        bytes.array,
        0,
        (0..32).map(|i| Some(start + i)).collect(),
        S::ONE,
    );
    let xbytes = link(xraw.array, 0, (0..32).map(Some).collect(), -S::ONE);
    let sign = link(
        parity,
        0,
        (0..32).map(|i| (i == 31).then_some(0)).collect(),
        -S::from(128),
    );
    w.push(Array {
        name: "compressed proof point exact bytes",
        family,
        rho: 5,
        r_count: 32,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![1],
        values: vec![vec![S::ZERO; 32]],
        rows: vec![[
            plus(&plus(&encoded, &xbytes), &sign),
            constant(vec![S::ONE; 32]),
            Vec::new(),
        ]],
    });
}

/// A total byte decoder's field value and its uniquely determined canonical
/// encoding predicate. An invalid encoding still has a constrained value;
/// callers select a canonical dummy before passing it to a hard verifier.
pub struct ScalarEncoding {
    pub canonical: VerifierCell,
    pub value: VerifierCell,
}

pub struct PointEncoding {
    pub canonical: VerifierCell,
    pub coordinates: [VerifierCell; 2],
}

/// Derive `raw < modulus` by byte subtraction. Unlike `bind_scalar`, this
/// accepts every 32-byte input and exposes the final borrow as a verdict.
pub fn total_scalar<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    bytes: &SharedBytes,
    start: usize,
) -> ScalarEncoding {
    assert!(start.checked_add(32).is_some_and(|end| end <= bytes.len));
    ensure_range(w, family);
    let raw: Vec<u8> = (0..32)
        .map(|i| {
            let repr = value(w, bytes.byte(start + i)).to_repr();
            repr[0]
        })
        .collect();
    let mut modulus = (BigUint::from_bytes_le(&(-S::ONE).to_repr()) + 1u32).to_bytes_le();
    modulus.resize(32, 0);
    let mut values = vec![vec![S::ZERO; 32]; 2];
    let mut borrow = 0i16;
    for i in 0..32 {
        let d = raw[i] as i16 - modulus[i] as i16 - borrow;
        borrow = i16::from(d < 0);
        values[0][i] = S::from((d + 256 * borrow) as u64);
        values[1][i] = S::from(borrow as u64);
    }
    let array = w.arrays.len();
    let row = plus(
        &plus(
            &link(
                bytes.array,
                0,
                (0..32).map(|i| Some(start + i)).collect(),
                S::ONE,
            ),
            &constant(modulus.iter().map(|v| -S::from(*v as u64)).collect()),
        ),
        &plus(
            &link(
                array,
                1,
                (0usize..32).map(|i| i.checked_sub(1)).collect(),
                -S::ONE,
            ),
            &plus(
                &scaled(&own(1, 32), S::from(256)),
                &scaled(&own(0, 32), -S::ONE),
            ),
        ),
    );
    w.push(Array {
        name: "total scalar encoding subtraction",
        family,
        rho: 5,
        r_count: 32,
        fixed: false,
        lookups: range(0, 8, 32),
        widths: vec![8, 1],
        values,
        rows: vec![
            [row, constant(vec![S::ONE; 32]), vec![]],
            [
                own(1, 32),
                plus(&own(1, 32), &constant(vec![-S::ONE; 32])),
                vec![],
            ],
        ],
    });
    let mut wide = [0u8; 64];
    wide[..32].copy_from_slice(&raw);
    let reduced = S::from_uniform_bytes(&wide);
    let field = w.push(Array {
        name: "total scalar encoding field value",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        values: vec![vec![reduced]],
        rows: vec![[
            plus(&own(0, 1), &scaled(&bytes.packed(start, 32), -S::ONE)),
            constant(vec![S::ONE]),
            vec![],
        ]],
    });
    ScalarEncoding {
        canonical: VerifierCell {
            array,
            cell: 1,
            instance: 31,
        },
        value: VerifierCell {
            array: field,
            cell: 0,
            instance: 0,
        },
    }
}

struct Local<S> {
    a: Array<S>,
}
impl<S: Scalar> Local<S> {
    fn new(family: usize, name: &'static str) -> Self {
        Self {
            a: Array {
                name,
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
    fn cell(&mut self, value: S, width: usize) -> usize {
        let i = self.a.values.len();
        self.a.values.push(vec![value]);
        self.a
            .widths
            .push(u16::try_from(width).expect("field cell width"));
        i
    }
    fn eq(&mut self, a: Lin<S>, b: Lin<S>) {
        self.a.rows.push([
            plus(&a, &scaled(&b, -S::ONE)),
            constant(vec![S::ONE]),
            vec![],
        ]);
    }
    fn product(&mut self, a: Lin<S>, b: Lin<S>, c: Lin<S>) {
        self.a.rows.push([a, b, c]);
    }
    fn bit(&mut self, value: bool) -> usize {
        let i = self.cell(S::from(value as u64), 1);
        self.product(
            own(i, 1),
            plus(&own(i, 1), &constant(vec![-S::ONE])),
            vec![],
        );
        i
    }
    fn zero(&mut self, expression: Lin<S>, v: S) -> usize {
        let z = self.bit(v == S::ZERO);
        let inverse = self.cell(Option::<S>::from(v.invert()).unwrap_or(S::ZERO), 255);
        self.product(expression.clone(), own(z, 1), vec![]);
        self.product(
            expression,
            own(inverse, 1),
            plus(&constant(vec![S::ONE]), &scaled(&own(z, 1), -S::ONE)),
        );
        z
    }
}

/// Total compressed Pasta parsing. The square/nonsquare witness proves the
/// residue predicate in both directions; a prover cannot declare a valid
/// point invalid by supplying an arbitrary off-curve y coordinate.
pub fn total_point<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    bytes: &SharedBytes,
    start: usize,
) -> PointEncoding {
    assert!(start.checked_add(32).is_some_and(|end| end <= bytes.len));
    let mut raw: Vec<u8> = (0..32)
        .map(|i| value(w, bytes.byte(start + i)).to_repr()[0])
        .collect();
    let sign = raw[31] >> 7;
    raw[31] &= 127;
    let xbytes = byte_array(w, family, &raw, "total point canonical x candidate");
    let x = total_scalar(w, family, &xbytes, 0);
    let xv = value(w, x.value);
    let a = xv.square() * xv + S::from(5);
    let nonsquare = S::MULTIPLICATIVE_GENERATOR;
    // A primitive multiplicative generator is necessarily a nonsquare.
    let square = bool::from(a.sqrt().is_some());
    let mut y = Option::<S>::from(if square {
        a.sqrt()
    } else {
        (a * nonsquare).sqrt()
    })
    .expect("a nonzero element is square or becomes square after multiplying by a nonsquare");
    let wanted = if a == S::ZERO { 0 } else { sign };
    if (y.to_repr()[0] & 1) != wanted {
        y = -y;
    }
    let mut c = Local::new(family, "total compressed point residue and sign");
    // Without this seven-bit bound, a set sign bit could instead be treated
    // as a noncanonical high x byte, falsely declaring a valid point invalid.
    let low_last = c.cell(S::from(raw[31] as u64), 7);
    c.a.lookups.extend(range(low_last, 7, 1));
    c.eq(own(low_last, 1), source(xbytes.byte(31)));
    let sign_cell = c.bit(sign != 0);
    let x2 = c.cell(xv.square(), 255);
    let radicand = c.cell(a, 255);
    c.product(source(x.value), source(x.value), own(x2, 1));
    c.product(
        own(x2, 1),
        source(x.value),
        plus(&own(radicand, 1), &constant(vec![-S::from(5)])),
    );
    let square_cell = c.bit(square);
    let factor = c.cell(if square { S::ONE } else { nonsquare }, 255);
    c.eq(
        own(factor, 1),
        plus(
            &constant(vec![nonsquare]),
            &scaled(&own(square_cell, 1), S::ONE - nonsquare),
        ),
    );
    let rhs = c.cell(a * if square { S::ONE } else { nonsquare }, 255);
    c.product(own(radicand, 1), own(factor, 1), own(rhs, 1));
    let y_cell = c.cell(y, 255);
    c.product(own(y_cell, 1), own(y_cell, 1), own(rhs, 1));
    let a_zero = c.zero(own(radicand, 1), a);
    c.product(
        own(a_zero, 1),
        plus(&own(square_cell, 1), &constant(vec![-S::ONE])),
        vec![],
    );
    let x_zero = c.zero(source(x.value), xv);
    let identity = xv == S::ZERO && sign == 0;
    let identity_cell = c.cell(S::from(identity as u64), 1);
    c.product(
        own(x_zero, 1),
        plus(
            &constant(vec![S::ONE]),
            &scaled(&own(sign_cell, 1), -S::ONE),
        ),
        own(identity_cell, 1),
    );
    let zero_sign_ok = c.cell(S::from((a != S::ZERO || sign == 0) as u64), 1);
    c.product(
        own(a_zero, 1),
        own(sign_cell, 1),
        plus(
            &constant(vec![S::ONE]),
            &scaled(&own(zero_sign_ok, 1), -S::ONE),
        ),
    );
    let ordinary = square && (a != S::ZERO || sign == 0);
    let ordinary_cell = c.cell(S::from(ordinary as u64), 1);
    c.product(
        own(square_cell, 1),
        own(zero_sign_ok, 1),
        own(ordinary_cell, 1),
    );
    let either = c.cell(S::from((identity || ordinary) as u64), 1);
    c.product(
        plus(
            &constant(vec![S::ONE]),
            &scaled(&own(identity_cell, 1), -S::ONE),
        ),
        own(ordinary_cell, 1),
        plus(&own(either, 1), &scaled(&own(identity_cell, 1), -S::ONE)),
    );
    let canonical = c.cell(
        value(w, x.canonical) * S::from((identity || ordinary) as u64),
        1,
    );
    c.product(source(x.canonical), own(either, 1), own(canonical, 1));
    let output_y = c.cell(if identity { S::ZERO } else { y }, 255);
    c.product(
        own(y_cell, 1),
        plus(
            &constant(vec![S::ONE]),
            &scaled(&own(identity_cell, 1), -S::ONE),
        ),
        own(output_y, 1),
    );
    let local = w.push(c.a);
    let vc = |cell| VerifierCell {
        array: local,
        cell,
        instance: 0,
    };
    let mut join = link(
        bytes.array,
        0,
        (0..32).map(|i| Some(start + i)).collect(),
        S::ONE,
    );
    join = plus(
        &join,
        &link(xbytes.array, 0, (0..32).map(Some).collect(), -S::ONE),
    );
    join = plus(
        &join,
        &link(
            local,
            sign_cell,
            (0..32).map(|i| (i == 31).then_some(0)).collect(),
            -S::from(128),
        ),
    );
    w.push(Array {
        name: "total point exact sign split",
        family,
        rho: 5,
        r_count: 32,
        fixed: false,
        lookups: vec![],
        widths: vec![1],
        values: vec![vec![S::ZERO; 32]],
        rows: vec![[join, constant(vec![S::ONE; 32]), vec![]]],
    });
    let yraw = byte_array(w, family, &y.to_repr(), "total point chosen root bytes");
    bind_scalar(w, family, &yraw, 0, vc(y_cell));
    let mut parity = Local::new(family, "total point root sign normalization");
    let bit = parity.bit((y.to_repr()[0] & 1) != 0);
    let half = parity.cell(S::from((y.to_repr()[0] >> 1) as u64), 7);
    parity.a.lookups.extend(range(half, 7, 1));
    parity.eq(
        source(yraw.byte(0)),
        plus(&own(bit, 1), &scaled(&own(half, 1), S::from(2))),
    );
    parity.product(
        source(vc(sign_cell)),
        plus(
            &constant(vec![S::ONE]),
            &scaled(&source(vc(a_zero)), -S::ONE),
        ),
        own(bit, 1),
    );
    w.push(parity.a);
    PointEncoding {
        canonical: vc(canonical),
        coordinates: [x.value, vc(output_y)],
    }
}

/// Select canonical verifier bytes using a circuit-derived parse predicate.
/// The caller must supply that same predicate to both sides' binding chain.
/// Hash `raw`, never this selected sequence, for the Payment identity.
pub fn select_canonical_bytes<S: Scalar>(
    w: &mut Wiring<S>,
    family: usize,
    raw: &SharedBytes,
    valid: VerifierCell,
    dummy: &[u8],
) -> SharedBytes {
    assert_eq!(raw.len, dummy.len(), "one fixed-profile proof length");
    let v = value(w, valid);
    let selected: Vec<u8> = dummy
        .iter()
        .enumerate()
        .map(|(i, d)| {
            if v == S::ONE {
                value(w, raw.byte(i)).to_repr()[0]
            } else {
                *d
            }
        })
        .collect();
    let bytes = byte_array(w, family, &selected, "canonical selected verifier bytes");
    let n = raw.len.max(1).next_power_of_two();
    let indices: Vec<_> = (0..n).map(|i| (i < raw.len).then_some(i)).collect();
    let mut constants = vec![S::ZERO; n];
    for (i, b) in dummy.iter().enumerate() {
        constants[i] = S::from(*b as u64);
    }
    let one_minus_dummy = scaled(&constant(constants), -S::ONE);
    let selected_minus_dummy = plus(
        &link(bytes.array, 0, indices.clone(), S::ONE),
        &one_minus_dummy,
    );
    let raw_minus_dummy = plus(&link(raw.array, 0, indices, S::ONE), &one_minus_dummy);
    let selector = link(
        valid.array,
        valid.cell,
        vec![Some(valid.instance); n],
        S::ONE,
    );
    w.push(Array {
        name: "derived canonical proof selection",
        family,
        rho: n.trailing_zeros() as usize,
        r_count: raw.len,
        fixed: false,
        lookups: vec![],
        widths: vec![1],
        values: vec![vec![S::ZERO; n]],
        rows: vec![
            [raw_minus_dummy, selector.clone(), selected_minus_dummy],
            [
                selector.clone(),
                plus(&selector, &constant(vec![-S::ONE; n])),
                vec![],
            ],
        ],
    });
    bytes
}

/// Hash the complete pair after all native points/scalars/R and fixed header
/// bytes have been joined on their respective sides of the binding argument.
pub fn append_digest(
    w: &mut Wiring<Fp>,
    family: usize,
    bytes: &SharedBytes,
    raw: &[u8],
) -> crate::send_arithmetic::Source {
    use crate::{
        certificate::{self, Cells},
        fs::SpikeRng,
    };
    assert_eq!(bytes.len, raw.len());
    let mut preimage = (raw.len() as u32).to_le_bytes().to_vec();
    preimage.extend_from_slice(raw);
    let digest = crate::payment::proof_digest(raw).expect("bounded pair proof");
    let mut c = Cells::new(family);
    c.a.name = "complete raw pair proof digest";
    let mut words = vec![constant(vec![Fp::from(preimage.len() as u64)])];
    let mut values = vec![Fp::from(preimage.len() as u64)];
    for (chunk_index, chunk) in preimage.chunks(31).enumerate() {
        let mut expression = Vec::new();
        for (j, b) in chunk.iter().enumerate() {
            let i = 31 * chunk_index + j;
            let coefficient = Fp::from(256).pow_vartime([j as u64]);
            let term = if i < 4 {
                constant(vec![Fp::from(*b as u64)])
            } else {
                source(bytes.byte(i - 4))
            };
            expression = plus(&expression, &scaled(&term, coefficient));
        }
        words.push(expression);
        values.push(crate::certificate::fp(&BigUint::from_bytes_le(chunk)));
    }
    let output = [
        c.cell(Fp::ZERO, 255),
        c.cell(digest, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let out = output[1];
    let local = w.push(c.a);
    let inputs = values.into_iter().zip(words).collect::<Vec<_>>();
    certificate::hash_relation(
        w,
        family,
        local,
        crate::payment::PROOF_DOMAIN,
        &inputs,
        output,
        &mut SpikeRng::new(b"exact proof digest wiring"),
    );
    crate::send_arithmetic::Source {
        array: local,
        cell: out,
        instance: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fs::SpikeRng,
        ipa::Gens,
        r1cs::{Circuit, Instance, Witness},
        spartan,
        uniform::UniformCircuit,
    };
    use group::{Curve, Group, GroupEncoding};
    use iroha_pasta::{Ep, Eq, Fq, PastaAffine, PastaCurve};

    type Built<S> = (UniformCircuit<S>, Instance<S>, Witness<S>);
    fn wiring<S: Scalar>(
        coordinates: [S; 2],
        scalar: S,
        raw: &[u8],
    ) -> (Wiring<S>, SharedBytes, usize) {
        assert_eq!(raw.len(), 64);
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let state = w.push(Array {
            name: "proof encoding public coordinates",
            family,
            rho: 2,
            r_count: 3,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![255],
            rows: Vec::new(),
            values: vec![vec![coordinates[0], coordinates[1], scalar, S::ZERO]],
        });
        w.io_arrays.push((state, 16));
        let shared = w.push(Array {
            name: "proof encoding public exact words",
            family,
            rho: 2,
            r_count: 4,
            fixed: false,
            lookups: Vec::new(),
            widths: vec![128],
            rows: Vec::new(),
            values: vec![shared_words(raw).into_iter().map(S::from_u128).collect()],
        });
        w.io_arrays.push((shared, 128));
        let bytes = append_shared(&mut w, family, raw, shared, 0);
        let source = |instance| VerifierCell {
            array: state,
            cell: 0,
            instance,
        };
        bind_point(&mut w, family, &bytes, 0, [source(0), source(1)]);
        bind_scalar(&mut w, family, &bytes, 32, source(2));
        (w, bytes, family)
    }
    fn finish<S: Scalar>(w: Wiring<S>) -> Built<S> {
        let (a, p, n, t, c) = w.finish();
        UniformCircuit::build_with_copies(
            "exact proof encoding",
            a,
            p,
            n,
            t,
            c,
            12,
            192,
            b"exact proof encoding",
        )
    }
    fn holds<S: Scalar>((c, i, w): &Built<S>) -> bool {
        let mut values = w.w.clone();
        let rel = c.instantiate(&c.shape, &[S::from(31), S::from(47)], Some(&mut values));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
        let (a, b, c) = rel.abc(&c.shape, &z);
        a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
    }
    fn check<S: Scalar>(coordinates: [S; 2], compressed: [u8; 32]) {
        let scalar = S::from(37);
        let raw = [compressed.as_slice(), scalar.to_repr().as_slice()].concat();
        let (w, _, _) = wiring(coordinates, scalar, &raw);
        let good = finish(w);
        assert!(holds(&good));
        let layout = good.0.layout();
        drop(good);
        // Mutations also update the exact public byte words. The native value
        // sources stay fixed, so these exercise encoding joins, not just a
        // stale transport commitment.
        for which in [0, 31, 32, 63] {
            let mut bad = raw.clone();
            bad[which] ^= if which == 31 { 128 } else { 1 };
            let (w, _, _) = wiring(coordinates, scalar, &bad);
            let built = finish(w);
            assert_eq!(built.0.layout(), layout);
            assert!(!holds(&built), "altered canonical byte {which}");
        }
        let zeros = vec![0; 64];
        let (w, _, _) = wiring([S::ZERO; 2], S::ZERO, &zeros);
        assert!(holds(&finish(w)));
        let maximum = [compressed.as_slice(), (-S::ONE).to_repr().as_slice()].concat();
        let (w, _, _) = wiring(coordinates, -S::ONE, &maximum);
        assert!(holds(&finish(w)), "largest canonical field element");
        let modulus = BigUint::from_bytes_le(&(-S::ONE).to_repr()) + 1u32;
        let mut modulus_bytes = modulus.to_bytes_le();
        modulus_bytes.resize(32, 0);
        for offset in [0, 32] {
            let mut aliased = zeros.clone();
            aliased[offset..offset + 32].copy_from_slice(&modulus_bytes);
            let (w, _, _) = wiring([S::ZERO; 2], S::ZERO, &aliased);
            assert!(!holds(&finish(w)), "value plus field modulus at {offset}");
        }
    }
    #[test]
    fn canonical_point_and_scalar_bytes_bind_both_pasta_fields() {
        let p = Ep::generator().to_affine();
        let (x, y) = Option::<(Fp, Fp)>::from(p.coordinates()).unwrap();
        check([x, y], p.to_bytes());
        let p = Eq::generator().to_affine();
        let (x, y) = Option::<(Fq, Fq)>::from(p.coordinates()).unwrap();
        check([x, y], p.to_bytes());
    }

    fn parsed<S: Scalar>(raw: &[u8; 32], point: bool) -> (Built<S>, Vec<S>) {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let shared = w.push(Array {
            name: "total parser exact public bytes",
            family,
            rho: 1,
            r_count: 2,
            fixed: false,
            lookups: vec![],
            widths: vec![128],
            rows: vec![],
            values: vec![shared_words(raw).into_iter().map(S::from_u128).collect()],
        });
        w.io_arrays.push((shared, 128));
        let bytes = append_shared(&mut w, family, raw, shared, 0);
        let sources = if point {
            let p = total_point(&mut w, family, &bytes, 0);
            vec![p.canonical, p.coordinates[0], p.coordinates[1]]
        } else {
            let p = total_scalar(&mut w, family, &bytes, 0);
            vec![p.canonical, p.value]
        };
        let values: Vec<S> = sources.iter().map(|s| value(&w, *s)).collect();
        let selected = select_canonical_bytes(&mut w, family, &bytes, sources[0], &[0; 32]);
        for i in 0..32 {
            assert_eq!(
                value(&w, selected.byte(i)),
                if values[0] == S::ONE {
                    S::from(raw[i] as u64)
                } else {
                    S::ZERO
                }
            );
        }
        let mut padded = values.clone();
        padded.resize(4, S::ZERO);
        let output = w.push(Array {
            name: "total parser public derived outputs",
            family,
            rho: 2,
            r_count: values.len(),
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![padded],
        });
        w.io_arrays.push((output, 4));
        let mut c = Local::new(family, "total parser output joins");
        for (i, s) in sources.iter().enumerate() {
            c.eq(source(*s), link(output, 0, vec![Some(i)], S::ONE));
        }
        w.push(c.a);
        (finish(w), values)
    }

    fn total_scalar_cases<S: Scalar>() {
        let p = BigUint::from_bytes_le(&(-S::ONE).to_repr()) + 1u32;
        for number in [
            BigUint::from(0u32),
            BigUint::from(1u32),
            &p - 1u32,
            p.clone(),
            &p + 1u32,
            (BigUint::from(1u32) << 256) - 1u32,
        ] {
            let mut raw = number.to_bytes_le();
            raw.resize(32, 0);
            let raw: [u8; 32] = raw.try_into().unwrap();
            let expected = bool::from(S::from_repr(raw).is_some());
            let (mut built, values) = parsed::<S>(&raw, false);
            assert_eq!(values[0], S::from(expected as u64));
            assert!(holds(&built));
            built.1.io[4] = S::ONE - values[0];
            assert!(
                !holds(&built),
                "scalar canonical predicate cannot be supplied by the host"
            );
        }
    }

    fn total_point_cases<C: PastaCurve>()
    where
        C::Base: Scalar,
    {
        let generator = C::generator().to_affine();
        let mut candidates = vec![
            [0u8; 32],
            [255u8; 32],
            generator.to_bytes(),
            (-C::generator()).to_affine().to_bytes(),
        ];
        let p = BigUint::from_bytes_le(&(-C::Base::ONE).to_repr()) + 1u32;
        let mut modulus = p.to_bytes_le();
        modulus.resize(32, 0);
        candidates.push(modulus.try_into().unwrap());
        for first in [0, 1, 2, 3, 7, 255] {
            for sign in [0, 128] {
                let mut raw = [0u8; 32];
                raw[0] = first;
                raw[31] = sign;
                candidates.push(raw);
            }
        }
        for raw in candidates {
            let native = Option::<C::AffineExt>::from(C::AffineExt::from_bytes(&raw));
            let expected = native.as_ref().is_some_and(|p| p.to_bytes() == raw);
            let (mut built, values) = parsed::<C::Base>(&raw, true);
            assert_eq!(
                values[0],
                C::Base::from(expected as u64),
                "native compressed parser parity for {raw:?}"
            );
            if expected {
                let (x, y) = Option::<(C::Base, C::Base)>::from(native.unwrap().coordinates())
                    .unwrap_or((C::Base::ZERO, C::Base::ZERO));
                assert_eq!(&values[1..], &[x, y]);
            }
            assert!(
                holds(&built),
                "total parsing must accept invalid candidates with a derived false result"
            );
            built.1.io[4] = C::Base::ONE - values[0];
            assert!(
                !holds(&built),
                "point canonical predicate cannot be supplied by the host"
            );
        }
    }

    #[test]
    fn total_scalar_parser_matches_both_native_fields_and_binds_verdict() {
        total_scalar_cases::<Fp>();
        total_scalar_cases::<Fq>();
    }

    #[test]
    fn total_point_parser_matches_both_native_curves_and_binds_verdict() {
        total_point_cases::<Ep>();
        total_point_cases::<Eq>();
    }

    #[test]
    fn raw_digest_matches_native_at_word_and_sponge_boundaries() {
        for len in [0usize, 1, 16, 27, 28, 31, 32, 33, 65] {
            let raw: Vec<u8> = (0..len).map(|i| ((i * 73 + 191) & 255) as u8).collect();
            let mut w = Wiring::<Fp>::default();
            let family = w.family(0, 1);
            let words = shared_words(&raw);
            let n = words.len().max(1).next_power_of_two();
            let mut values = vec![Fp::ZERO; n];
            for (to, value) in values.iter_mut().zip(words) {
                *to = Fp::from_u128(value);
            }
            let shared = w.push(Array {
                name: "raw digest boundary public words",
                family,
                rho: n.trailing_zeros() as usize,
                r_count: len.div_ceil(16),
                fixed: false,
                lookups: vec![],
                widths: vec![128],
                rows: vec![],
                values: vec![values],
            });
            w.io_arrays.push((shared, 128));
            let bytes = append_shared(&mut w, family, &raw, shared, 0);
            let digest = append_digest(&mut w, family, &bytes, &raw);
            assert_eq!(
                digest.value(&w),
                crate::payment::proof_digest(&raw).unwrap()
            );
            crate::operation_auth::tests::expose(&mut w, family, 4, &[digest]);
            assert!(
                holds(&finish(w)),
                "exact length/chunk framing at {len} bytes"
            );
        }
    }
    #[test]
    fn complete_raw_digest_proves_and_rejects_final_response_substitution() {
        let p = Ep::generator().to_affine();
        let (x, y) = Option::<(Fp, Fp)>::from(p.coordinates()).unwrap();
        let raw = [p.to_bytes().as_slice(), Fp::from(37).to_repr().as_slice()].concat();
        let (mut w, bytes, family) = wiring([x, y], Fp::from(37), &raw);
        let digest = append_digest(&mut w, family, &bytes, &raw);
        assert_eq!(
            digest.value(&w),
            crate::payment::proof_digest(&raw).unwrap()
        );
        crate::operation_auth::tests::expose(&mut w, family, 4, &[digest]);
        let (c, i, wit) = finish(w);
        let built = (c, i, wit);
        assert!(holds(&built));
        let (c, i, wit) = built;
        eprintln!(
            "exact proof bytes: constraints={} rows={} cols={} witness_cells={}",
            c.constraints,
            c.shape.rows(),
            c.shape.cols(),
            wit.w.len()
        );
        let g = Gens::<Eq>::new(c.shape.cols());
        let (p, _) = spartan::prove_circuit(
            &c.shape,
            &g,
            &i,
            &wit,
            &mut SpikeRng::new(b"exact proof bytes proof"),
            &c,
        );
        spartan::verify_circuit(&c.shape, &g, &i, &p.to_bytes(), &c).unwrap();
        let mut changed = i.clone();
        changed.io[4] += Fp::ONE;
        assert!(spartan::verify_circuit(&c.shape, &g, &changed, &p.to_bytes(), &c).is_err());
        let mut changed = wit.w.clone();
        changed[c.cell_index(bytes.array, 0, 63)] += Fp::ONE;
        assert!(
            !holds(&(c, i, Witness { w: changed })),
            "final scalar byte substitution must fail the exact digest relation"
        );
    }
}
