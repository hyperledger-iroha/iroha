//! Native 26-field statement rules and kgwstmt1, composable with Core30 ports.
//! This validates one fixed operation kind; it does not authorize a transition.
use crate::{
    certificate::{self, Cells, fp},
    fs::SpikeRng,
    full::Wiring,
    uniform::{Lin, constant, own, plus, scaled},
};
use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

pub const FIELDS: usize = 26;
pub const DOMAIN: u64 = u64::from_le_bytes(*b"kgwstmt1");
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bootstrap,
    Load,
    Send,
    Receive,
    Archive,
    Unload,
    RefreshCredential,
    RefreshSchemePolicy,
    RefreshQuotaShare,
    RefreshTimeAnchor,
    Retiring,
}
impl Kind {
    fn tag(self) -> u64 {
        match self {
            Self::Bootstrap => 1,
            Self::Load => 2,
            Self::Send => 3,
            Self::Receive => 4,
            Self::Archive => 5,
            Self::Unload => 6,
            Self::Retiring => 8,
            _ => 7,
        }
    }
    fn consumes_lineage(self) -> bool {
        matches!(self, Self::Send | Self::Unload | Self::Retiring)
    }
}
pub struct StatementCells {
    pub local: usize,
    pub fields: [usize; FIELDS],
    pub digest: usize,
}
pub fn digest(fields: &[Fp; FIELDS]) -> Fp {
    poseidon::hash_with_domain(DOMAIN, fields)
}
fn nonzero(c: &mut Cells, e: Lin<Fp>, v: Fp) {
    let k = c.cell(Option::<Fp>::from(v.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([e, own(k, 1), constant(vec![Fp::ONE])]);
}
fn bound(c: &mut Cells, k: usize, v: Fp, n: usize) {
    let bytes = v.to_repr();
    let indices: Vec<_> = bytes.as_ref()[..n].iter().map(|b| c.byte(*b)).collect();
    c.eq(own(k, 1), c.bytes(&indices));
}
fn bounded_value(c: &mut Cells, v: Fp, n: usize) -> usize {
    let k = c.cell(v, 255);
    bound(c, k, v, n);
    k
}
fn is_zero(c: &mut Cells, k: usize, v: Fp) -> usize {
    let z = c.bit(v == Fp::ZERO);
    let inv = c.cell(Option::<Fp>::from(v.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([own(k, 1), own(z, 1), vec![]]);
    c.a.rows.push([
        own(k, 1),
        own(inv, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(z, 1), -Fp::ONE)),
    ]);
    z
}
fn le(c: &mut Cells, a: usize, b: usize, va: Fp, vb: Fp, bytes: usize) {
    let ai = BigUint::from_bytes_le(va.to_repr().as_ref());
    let bi = BigUint::from_bytes_le(vb.to_repr().as_ref());
    let difference = if bi >= ai { fp(&(bi - ai)) } else { Fp::ZERO };
    let d = bounded_value(c, difference, bytes);
    c.eq(own(b, 1), plus(&own(a, 1), &own(d, 1)));
}
fn identity(c: &mut Cells, f: &[usize; 26], v: &[Fp; 26], start: usize) {
    for i in start..start + 2 {
        bound(c, f[i], v[i], 16);
    }
    nonzero(
        c,
        plus(&own(f[start], 1), &own(f[start + 1], 1)),
        v[start] + v[start + 1],
    );
}

/// The caller supplies a byte-range table and joins all state/effect/authority
/// ports. Invalid fields keep the same template and fail the relation.
pub fn append_statement(
    w: &mut Wiring<Fp>,
    family: usize,
    kind: Kind,
    values: &[Fp; FIELDS],
) -> StatementCells {
    let mut c = Cells::new(family);
    c.a.name = "canonical26 field statement";
    let f = values.map(|v| c.cell(v, 255));
    c.eq(own(f[0], 1), constant(vec![Fp::ONE]));
    for start in [1, 3, 5] {
        identity(&mut c, &f, values, start);
    }
    for i in [7, 15] {
        nonzero(&mut c, own(f[i], 1), values[i]);
    }
    c.a.rows.push([
        plus(&own(f[8], 1), &constant(vec![-Fp::ONE])),
        plus(&own(f[8], 1), &constant(vec![-Fp::from(2)])),
        vec![],
    ]);
    for i in [9, 10, 12] {
        bound(&mut c, f[i], values[i], 16);
    }
    let bits = [
        c.bit(values[11].to_repr()[0] & 1 != 0),
        c.bit(values[11].to_repr()[0] & 2 != 0),
    ];
    c.eq(
        own(f[11], 1),
        plus(&own(bits[0], 1), &scaled(&own(bits[1], 1), Fp::from(2))),
    );
    if kind.consumes_lineage() {
        nonzero(&mut c, own(f[13], 1), values[13]);
    } else {
        for i in [12, 13] {
            c.zero(own(f[i], 1));
        }
    }
    if kind == Kind::Bootstrap {
        for i in [9, 10, 11, 14] {
            c.zero(own(f[i], 1));
        }
        c.eq(own(f[8], 1), constant(vec![Fp::ONE]));
    } else {
        for i in [9, 14] {
            nonzero(&mut c, own(f[i], 1), values[i]);
        }
    }
    c.eq(own(f[16], 1), constant(vec![Fp::from(kind.tag())]));
    let count = match kind {
        Kind::Bootstrap => {
            identity(&mut c, &f, values, 17);
            identity(&mut c, &f, values, 19);
            4
        }
        Kind::Load => {
            nonzero(&mut c, own(f[17], 1), values[17]);
            for i in 18..21 {
                bound(&mut c, f[i], values[i], 16);
            }
            c.eq(
                own(f[10], 1),
                plus(&own(f[18], 1), &constant(vec![Fp::ONE])),
            );
            4
        }
        Kind::Send => {
            for i in [17, 23] {
                nonzero(&mut c, own(f[i], 1), values[i]);
            }
            identity(&mut c, &f, values, 18);
            for i in 20..23 {
                bound(&mut c, f[i], values[i], 16);
            }
            nonzero(&mut c, own(f[21], 1), values[21]);
            let gross = bounded_value(&mut c, values[21] + values[22], 16);
            c.eq(own(gross, 1), plus(&own(f[21], 1), &own(f[22], 1)));
            for i in [24, 25] {
                bound(&mut c, f[i], values[i], 8);
            }
            le(&mut c, f[24], f[25], values[24], values[25], 8);
            9
        }
        Kind::Receive => {
            nonzero(&mut c, own(f[17], 1), values[17]);
            identity(&mut c, &f, values, 18);
            bound(&mut c, f[20], values[20], 16);
            nonzero(&mut c, own(f[20], 1), values[20]);
            4
        }
        Kind::Archive => {
            for i in [17, 18] {
                nonzero(&mut c, own(f[i], 1), values[i]);
            }
            2
        }
        Kind::Unload => {
            nonzero(&mut c, own(f[17], 1), values[17]);
            for i in 18..21 {
                bound(&mut c, f[i], values[i], 16);
            }
            nonzero(&mut c, own(f[19], 1), values[19]);
            le(&mut c, f[20], f[19], values[20], values[19], 16);
            let no_charge = is_zero(&mut c, f[20], values[20]);
            let no_quote = is_zero(&mut c, f[21], values[21]);
            c.eq(own(no_charge, 1), own(no_quote, 1));
            5
        }
        Kind::Retiring => {
            c.eq(own(f[8], 1), constant(vec![Fp::from(2)]));
            0
        }
        k => {
            let tag = match k {
                Kind::RefreshCredential => 1,
                Kind::RefreshSchemePolicy => 2,
                Kind::RefreshQuotaShare => 3,
                Kind::RefreshTimeAnchor => 4,
                _ => unreachable!(),
            };
            c.eq(own(f[17], 1), constant(vec![Fp::from(tag)]));
            nonzero(&mut c, own(f[18], 1), values[18]);
            bound(&mut c, f[19], values[19], 8);
            if k == Kind::RefreshCredential {
                c.eq(own(f[18], 1), own(f[7], 1));
            }
            3
        }
    };
    for i in 17 + count..26 {
        c.zero(own(f[i], 1));
    }
    let dg = digest(values);
    let ds = [
        c.cell(Fp::ZERO, 255),
        c.cell(dg, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let items: Vec<_> = values.iter().zip(f).map(|(v, k)| (*v, own(k, 1))).collect();
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"native statement digest");
    certificate::hash_relation(w, family, local, DOMAIN, &items, ds, &mut rng);
    StatementCells {
        local,
        fields: f,
        digest: ds[1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        nonnative,
        r1cs::{Circuit, Instance, Witness},
        uniform::{Array, UniformCircuit, link},
    };
    fn field(s: &str) -> Fp {
        let b: Vec<_> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect();
        Option::from(Fp::from_repr(b.try_into().unwrap())).unwrap()
    }
    fn reference(name: &str) -> &'static str {
        include_str!("../fixtures/statement-source-vectors.txt")
            .lines()
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| *k == name)
            .unwrap()
            .1
    }
    fn fixture(name: &str) -> [Fp; 26] {
        reference(&format!("{name}_statement_items"))
            .split(',')
            .map(field)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }
    fn build(kind: Kind, v: &[Fp; 26]) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>) {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let ports = append_statement(&mut w, family, kind, v);
        let range = w.push(Array {
            name: "statement range",
            family,
            rho: 11,
            r_count: 2048,
            fixed: true,
            lookups: vec![],
            widths: vec![11],
            rows: vec![],
            values: vec![(0..2048).map(|v| Fp::from(v as u64)).collect()],
        });
        w.tables.push((range, nonnative::RANGE_TABLE));
        let mut out = v.to_vec();
        out.push(digest(v));
        out.resize(32, Fp::ZERO);
        let io = w.push(Array {
            name: "statement ports",
            family,
            rho: 5,
            r_count: 27,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![out],
        });
        w.io_arrays.push((io, 32));
        for (i, k) in ports.fields.into_iter().chain([ports.digest]).enumerate() {
            w.arrays[ports.local].rows.push([
                plus(&own(k, 1), &link(io, 0, vec![Some(i)], -Fp::ONE)),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        let (a, pi, n, t, copies) = w.finish();
        UniformCircuit::build_with_copies(
            "native statement",
            a,
            pi,
            n,
            t,
            copies,
            12,
            64,
            b"native statement",
        )
    }
    fn holds((c, i, wt): &(UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>)) -> bool {
        let mut w = wt.w.clone();
        let rel = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut w));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w });
        let (a, b, c) = rel.abc(&c.shape, &z);
        a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
    }
    #[test]
    fn native_send_receive_vectors_and_every_exposed_field() {
        for (name, kind) in [("send", Kind::Send), ("receive", Kind::Receive)] {
            let v = fixture(name);
            assert_eq!(
                digest(&v),
                field(reference(&format!("{name}_statement_digest")))
            );
            let mut built = build(kind, &v);
            assert!(holds(&built));
            for i in 32..59 {
                built.1.io[i] += Fp::ONE;
                assert!(!holds(&built), "unbound statement port {i}");
                built.1.io[i] -= Fp::ONE;
            }
        }
    }
    #[test]
    fn send_width_order_nonzero_mask_and_overflow_fail_closed() {
        let v = fixture("send");
        let base = build(Kind::Send, &v);
        let mut cases = vec![];
        for (i, x) in [
            (0, Fp::from(2)),
            (7, Fp::ZERO),
            (8, Fp::from(3)),
            (9, Fp::ZERO),
            (11, Fp::from(4)),
            (13, Fp::ZERO),
            (14, Fp::ZERO),
            (15, Fp::ZERO),
            (16, Fp::from(4)),
            (17, Fp::ZERO),
            (21, Fp::ZERO),
            (23, Fp::ZERO),
            (24, v[25] + Fp::ONE),
        ] {
            let mut bad = v;
            bad[i] = x;
            cases.push(bad);
        }
        for i in [1, 3, 5, 9, 10, 12, 18, 20, 21, 22] {
            let mut bad = v;
            bad[i] = Fp::from(2).pow_vartime([128]);
            cases.push(bad);
        }
        for i in [24, 25] {
            let mut bad = v;
            bad[i] = Fp::from(2).pow_vartime([64]);
            cases.push(bad);
        }
        let mut bad = v;
        bad[21] = Fp::from(2).pow_vartime([128]) - Fp::ONE;
        bad[22] = Fp::ONE;
        cases.push(bad);
        for bad in cases {
            let b = build(Kind::Send, &bad);
            assert!(!holds(&b));
            assert_eq!(
                base.0.entries(&[Fp::from(31), Fp::from(47)]),
                b.0.entries(&[Fp::from(31), Fp::from(47)])
            );
        }
    }
    #[test]
    fn bootstrap_load_unload_refresh_and_retirement_rules() {
        let mut v = fixture("receive");
        v[8] = Fp::ONE;
        v[9] = Fp::ZERO;
        v[14] = Fp::ZERO;
        v[16] = Fp::ONE;
        v[17..21].fill(Fp::ONE);
        assert!(holds(&build(Kind::Bootstrap, &v)));
        for i in [9, 10, 11, 14, 21] {
            let mut bad = v;
            bad[i] = Fp::ONE;
            assert!(!holds(&build(Kind::Bootstrap, &bad)));
        }
        v = fixture("receive");
        v[16] = Fp::from(2);
        v[18] = Fp::from(7);
        v[10] = Fp::from(8);
        assert!(holds(&build(Kind::Load, &v)));
        v[10] = Fp::from(7);
        assert!(!holds(&build(Kind::Load, &v)));
        v = fixture("send");
        v[16] = Fp::from(6);
        v[18] = Fp::from(2);
        v[19] = Fp::from(10);
        v[20] = Fp::from(3);
        v[21] = Fp::from(4);
        v[22..].fill(Fp::ZERO);
        assert!(holds(&build(Kind::Unload, &v)));
        for (amount, charge, quote) in [(0, 0, 0), (10, 11, 1), (10, 1, 0), (10, 0, 1)] {
            let mut bad = v;
            bad[19] = Fp::from(amount);
            bad[20] = Fp::from(charge);
            bad[21] = Fp::from(quote);
            assert!(!holds(&build(Kind::Unload, &bad)));
        }
        v = fixture("receive");
        v[16] = Fp::from(7);
        v[17] = Fp::ONE;
        v[18] = v[7];
        v[19] = Fp::from(1);
        v[20..].fill(Fp::ZERO);
        assert!(holds(&build(Kind::RefreshCredential, &v)));
        v[18] += Fp::ONE;
        assert!(!holds(&build(Kind::RefreshCredential, &v)));
        v = fixture("send");
        v[16] = Fp::from(8);
        v[8] = Fp::from(2);
        v[17..].fill(Fp::ZERO);
        assert!(holds(&build(Kind::Retiring, &v)));
        v[8] = Fp::ONE;
        assert!(!holds(&build(Kind::Retiring, &v)));
    }
}
