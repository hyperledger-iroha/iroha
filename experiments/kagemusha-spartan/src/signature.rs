//! Total P-256 ECDSA verdict over exact original Request bytes.
//!
//! Arithmetic and lookups remain hard on both verdicts. Every field result is
//! canonical, every exceptional point case is selected by an iff zero test,
//! and the only soft output is equality of the computed ECDSA point and r.
use crate::{
    certificate::{self, Cells, fp, pow2},
    credential_policy as pred,
    full::Wiring,
    nonnative::{self, CheckShape, Port},
    operation_auth::CurrentCredential,
    p256::{self, Point},
    receive_originals::OriginalRequest,
    request,
    send_arithmetic::Source,
    sha,
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};
use ff::{Field, PrimeField};
use iroha_pasta::Fp;
use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_traits::{One, Zero};

/// Only this hard functional verifier can construct a signature verdict.
pub struct SignatureValidity {
    valid: Source,
}
impl SignatureValidity {
    pub fn valid(&self) -> Source {
        self.valid
    }
}
#[derive(Clone)]
struct N {
    value: BigUint,
    limbs: [usize; 3],
}
#[derive(Clone)]
struct P {
    x: N,
    y: N,
    infinity: usize,
}
struct Check {
    a: N,
    b: N,
    lin: Vec<(i64, N)>,
    constant: i64,
    modulus: usize,
}
struct Arithmetic {
    c: Cells,
    checks: Vec<Check>,
    zero: N,
    one: N,
    p: BigUint,
    n: BigUint,
}
fn limbs(v: &BigUint) -> [BigInt; 3] {
    std::array::from_fn(|i| BigInt::from((v >> (88 * i)) % pow2(if i == 2 { 80 } else { 88 })))
}
impl Arithmetic {
    fn new(family: usize) -> Self {
        let mut c = Cells::new(family);
        c.a.name = "total Request ECDSA canonical functional arithmetic";
        let zero = Self::constant_value(&mut c, BigUint::zero());
        let one = Self::constant_value(&mut c, BigUint::one());
        Self {
            c,
            checks: vec![],
            zero,
            one,
            p: p256::p(),
            n: p256::n(),
        }
    }
    fn constant_value(c: &mut Cells, v: BigUint) -> N {
        let l = limbs(&v).map(|x| {
            let f = fp(x.magnitude());
            let k = c.cell(f, 88);
            c.eq(own(k, 1), constant(vec![f]));
            k
        });
        N { value: v, limbs: l }
    }
    fn constant(&mut self, v: BigUint) -> N {
        Self::constant_value(&mut self.c, v)
    }
    fn allocate(&mut self, v: BigUint, bound: Option<BigUint>) -> (N, Vec<usize>) {
        assert!(v.bits() <= 256);
        let bytes: Vec<_> = certificate::scalar_bytes(&v)
            .iter()
            .rev()
            .map(|b| self.c.byte(*b))
            .collect();
        if let Some(b) = bound {
            self.c.at_most(&bytes, &v, &b);
        }
        let l = self.c.value(&v, &bytes);
        (N { value: v, limbs: l }, bytes)
    }
    fn residue(&mut self, v: BigUint, modulus: usize) -> N {
        let m = if modulus == 0 { &self.p } else { &self.n };
        assert!(v < *m);
        self.allocate(v, Some(m - 1u32)).0
    }
    fn check(&mut self, a: &N, b: &N, lin: Vec<(i64, N)>, constant: i64, modulus: usize) {
        self.checks.push(Check {
            a: a.clone(),
            b: b.clone(),
            lin,
            constant,
            modulus,
        });
    }
    fn linear(&mut self, terms: &[(i64, &N)], constant: i64, modulus: usize) -> N {
        let m = if modulus == 0 { &self.p } else { &self.n };
        let v = terms.iter().fold(BigInt::from(constant), |a, (coef, n)| {
            a + BigInt::from(*coef) * BigInt::from(n.value.clone())
        });
        let out = self.residue(
            v.mod_floor(&BigInt::from(m.clone())).to_biguint().unwrap(),
            modulus,
        );
        let mut lin: Vec<_> = terms
            .iter()
            .map(|(coef, n)| (*coef, (*n).clone()))
            .collect();
        lin.push((-1, out.clone()));
        self.check(
            &self.zero.clone(),
            &self.zero.clone(),
            lin,
            constant,
            modulus,
        );
        out
    }
    fn mul(&mut self, a: &N, b: &N, modulus: usize) -> N {
        let m = if modulus == 0 { &self.p } else { &self.n };
        let out = self.residue(&a.value * &b.value % m, modulus);
        self.check(a, b, vec![(-1, out.clone())], 0, modulus);
        out
    }
    fn is_zero(&mut self, a: &N) -> usize {
        let e = a
            .limbs
            .into_iter()
            .fold(vec![], |acc, k| plus(&acc, &own(k, 1)));
        let value = a.limbs.into_iter().map(|k| self.c.a.values[k][0]).sum();
        pred::zero_predicate(&mut self.c, e, value)
    }
    fn equal(&mut self, a: &N, b: &N) -> usize {
        let checks: Vec<_> = (0..3)
            .map(|i| {
                let e = plus(&own(a.limbs[i], 1), &scaled(&own(b.limbs[i], 1), -Fp::ONE));
                let v = self.c.a.values[a.limbs[i]][0] - self.c.a.values[b.limbs[i]][0];
                pred::zero_predicate(&mut self.c, e, v)
            })
            .collect();
        pred::all(&mut self.c, &checks)
    }
    fn not(&mut self, z: usize) -> usize {
        pred::negate(&mut self.c, z)
    }
    fn bitselect(&mut self, bit: usize, yes: usize, no: usize) -> usize {
        let bv = self.c.a.values[bit][0];
        let y = self.c.a.values[yes][0];
        let n = self.c.a.values[no][0];
        let out = self.c.cell(bv * (y - n) + n, 1);
        self.c.a.rows.push([
            own(bit, 1),
            plus(&own(yes, 1), &scaled(&own(no, 1), -Fp::ONE)),
            plus(&own(out, 1), &scaled(&own(no, 1), -Fp::ONE)),
        ]);
        out
    }
    fn select(&mut self, bit: usize, yes: &N, no: &N) -> N {
        let choose = self.c.a.values[bit][0] == Fp::ONE;
        let value = if choose {
            yes.value.clone()
        } else {
            no.value.clone()
        };
        let l = std::array::from_fn(|i| {
            let y = self.c.a.values[yes.limbs[i]][0];
            let n = self.c.a.values[no.limbs[i]][0];
            let k = self
                .c
                .cell(if choose { y } else { n }, if i == 2 { 80 } else { 88 });
            self.c.a.rows.push([
                own(bit, 1),
                plus(
                    &own(yes.limbs[i], 1),
                    &scaled(&own(no.limbs[i], 1), -Fp::ONE),
                ),
                plus(&own(k, 1), &scaled(&own(no.limbs[i], 1), -Fp::ONE)),
            ]);
            k
        });
        N { value, limbs: l }
    }
    fn inverse(&mut self, a: &N, modulus: usize) -> (N, usize) {
        let z = self.is_zero(a);
        let m = if modulus == 0 { &self.p } else { &self.n };
        let v = if a.value.is_zero() {
            BigUint::zero()
        } else {
            p256::inv(&a.value, m)
        };
        let inv = self.residue(v, modulus);
        let one = self.one.clone();
        let zero = self.zero.clone();
        let result = self.select(z, &zero, &one);
        self.check(a, &inv, vec![(-1, result)], 0, modulus);
        for k in inv.limbs {
            self.c.a.rows.push([own(z, 1), own(k, 1), vec![]]);
        }
        (inv, z)
    }
    fn fixed_bit(&mut self, value: bool) -> usize {
        let k = self.c.bit(value);
        self.c.eq(own(k, 1), constant(vec![Fp::from(value as u64)]));
        k
    }
    fn identity(&mut self) -> P {
        P {
            x: self.zero.clone(),
            y: self.zero.clone(),
            infinity: self.fixed_bit(true),
        }
    }
    fn point_select(&mut self, b: usize, yes: &P, no: &P) -> P {
        P {
            x: self.select(b, &yes.x, &no.x),
            y: self.select(b, &yes.y, &no.y),
            infinity: self.bitselect(b, yes.infinity, no.infinity),
        }
    }
    /// Complete addition: canonical coordinates for infinity are (0,0).
    fn add(&mut self, a: &P, b: &P) -> P {
        let dx = self.linear(&[(1, &b.x), (-1, &a.x)], 0, 0);
        let dy = self.linear(&[(1, &b.y), (-1, &a.y)], 0, 0);
        let same_x = self.is_zero(&dx);
        let xx = self.mul(&a.x, &a.x, 0);
        let tangent = self.linear(&[(3, &xx)], -3, 0);
        let twice_y = self.linear(&[(2, &a.y)], 0, 0);
        let numerator = self.select(same_x, &tangent, &dy);
        let denominator = self.select(same_x, &twice_y, &dx);
        let (inverse, zero_den) = self.inverse(&denominator, 0);
        let slope = self.mul(&numerator, &inverse, 0);
        let square = self.mul(&slope, &slope, 0);
        let x = self.linear(&[(1, &square), (-1, &a.x), (-1, &b.x)], 0, 0);
        let xdiff = self.linear(&[(1, &a.x), (-1, &x)], 0, 0);
        let product = self.mul(&slope, &xdiff, 0);
        let y = self.linear(&[(1, &product), (-1, &a.y)], 0, 0);
        let same_y = self.is_zero(&dy);
        let different_y = self.not(same_y);
        let opposite = pred::all(&mut self.c, &[same_x, different_y]);
        let finite = self.fixed_bit(false);
        let raw = P {
            x,
            y,
            infinity: finite,
        };
        let identity = self.identity();
        let raw = self.point_select(zero_den, &identity, &raw);
        let raw = self.point_select(opposite, &identity, &raw);
        let raw = self.point_select(b.infinity, a, &raw);
        self.point_select(a.infinity, b, &raw)
    }
    fn digits(&mut self, u: &N) -> Vec<usize> {
        let bits = nonnative::comb_bits(&u.value);
        let v: BigUint = bits
            .iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| pow2(i))
            .sum();
        let (b, bytes) = self.allocate(v, Some(&self.n - 1u32));
        let digits: Vec<_> = bits.into_iter().map(|x| self.c.bit(x)).collect();
        for i in 0..32 {
            let expr = (0..8).fold(vec![], |a, k| {
                plus(&a, &scaled(&own(digits[8 * i + k], 1), Fp::from(1u64 << k)))
            });
            self.c.eq(own(bytes[i], 1), expr);
        }
        let offset = self.constant(pow2(256) - 1u32);
        let zero = self.zero.clone();
        self.check(
            &zero,
            &zero,
            vec![(2, b), (-1, offset), (-1, u.clone())],
            0,
            1,
        );
        digits
    }
    fn selected(
        &mut self,
        table: u32,
        entries: &[Vec<Point>],
        digits: &[usize],
        half: usize,
        step: usize,
    ) -> P {
        let mut index = half * 256;
        let mut expr = constant(vec![Fp::from((half * 256) as u64)]);
        for k in 0..8 {
            let bit = digits[16 * (8 * half + k) + step];
            if self.c.a.values[bit][0] == Fp::ONE {
                index += 1 << k;
            }
            expr = plus(&expr, &scaled(&own(bit, 1), Fp::from(1u64 << k)));
        }
        let point = &entries[half][index % 256];
        let x = self.residue(point.x.clone(), 0);
        let y = self.residue(point.y.clone(), 0);
        let mut tuple = vec![expr];
        tuple.extend(x.limbs.into_iter().chain(y.limbs).map(|k| own(k, 1)));
        self.c.a.lookups.push((table, tuple));
        P {
            x,
            y,
            infinity: self.fixed_bit(false),
        }
    }
    fn finish(self, w: &mut Wiring<Fp>) -> usize {
        let family = self.c.a.family;
        let local = w.push(self.c.a);
        let bound = [pow2(88) - 1u32, pow2(88) - 1u32, pow2(80) - 1u32];
        let shape = CheckShape::new(
            &[(bound.clone(), bound.clone())],
            &bound.map(|b| b * 8u32),
            4,
            &[self.p, self.n],
        );
        let port = |n: &N, coef: i64| Port {
            links: vec![(
                local,
                n.limbs[0],
                vec![Some(0)],
                vec![if coef < 0 {
                    -Fp::from(coef.unsigned_abs())
                } else {
                    Fp::from(coef as u64)
                }],
            )],
            consts: [vec![Fp::ZERO], vec![Fp::ZERO], vec![Fp::ZERO]],
        };
        for check in self.checks {
            let lin = Port {
                links: check
                    .lin
                    .iter()
                    .flat_map(|(coef, n)| port(n, *coef).links)
                    .collect(),
                consts: [vec![Fp::ZERO], vec![Fp::ZERO], vec![Fp::ZERO]],
            };
            let cst = if check.constant < 0 {
                -Fp::from(check.constant.unsigned_abs())
            } else {
                Fp::from(check.constant as u64)
            };
            let (rows, lookups) = shape.template(
                &[port(&check.a, 1)],
                &[port(&check.b, 1)],
                &[vec![Fp::ONE]],
                &lin,
                &[cst],
                &[check.modulus],
                1,
            );
            let lv = check.lin.iter().fold(
                [BigInt::zero(), BigInt::zero(), BigInt::zero()],
                |mut a, (coef, n)| {
                    for (a, v) in a.iter_mut().zip(limbs(&n.value)) {
                        *a += BigInt::from(*coef) * v;
                    }
                    a
                },
            );
            let values = shape.witness(
                &[limbs(&check.a.value)],
                &[limbs(&check.b.value)],
                &[1],
                &lv,
                check.constant,
                check.modulus,
            );
            w.push(Array {
                name: "total ECDSA modular functional check",
                family,
                rho: 0,
                r_count: 1,
                fixed: false,
                rows,
                lookups,
                widths: shape.widths(),
                values: values.into_iter().map(|v| vec![v]).collect(),
            });
        }
        local
    }
}

/// The own receipt must be appended first: its mandatory omitted wallet row
/// authenticates every Q lookup here to the current credential's commitment.
pub fn append_request_signature_total(
    w: &mut Wiring<Fp>,
    family: usize,
    request: &OriginalRequest,
    credential: &CurrentCredential,
) -> SignatureValidity {
    let (wallet, io) = w
        .wallet
        .expect("own receipt installs authenticated wallet table first");
    assert_eq!(io, crate::full::TABLE_IO);
    assert!(w.tables.contains(&(wallet, nonnative::WALLET_TABLE)));
    assert!(w.tables.iter().any(|(_, id)| *id == nonnative::POINT_TABLE));
    let q = credential.payment_point(w);
    let mut a = Arithmetic::new(family);
    // Bind the same certified table commitment as the hard own receipt.
    let table = w
        .io_arrays
        .iter()
        .find(|(_, offset)| *offset == io)
        .expect("own table public input")
        .0;
    for (i, s) in credential.table_commitment().into_iter().enumerate() {
        a.c.eq(s.linear(), link(table, 0, vec![Some(i)], Fp::ONE));
    }
    let req = request.request();
    let message = Source {
        array: req.local,
        cell: req.message,
        instance: 0,
    };
    let m = message.value(w);
    let message_bytes: Vec<_> = m.to_repr().iter().map(|b| a.c.byte(*b)).collect();
    a.c.at_most(
        &message_bytes,
        &BigUint::from_bytes_le(m.to_repr().as_ref()),
        &(certificate::field_modulus() - 1u32),
    );
    a.c.eq(a.c.bytes(&message_bytes), message.linear());
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(m.to_repr().as_ref()));
    let (zn, zbytes) = a.allocate(z.clone(), None);
    let mut signature = vec![];
    let mut valid = vec![];
    for (half, bound) in [(0, p256::n() - 1u32), (1, p256::n() >> 1)] {
        let original: Vec<_> = (0..32)
            .map(|i| request.byte(request::BODY_BYTES + 32 * half + 31 - i))
            .collect();
        let raw: Vec<u8> = original.iter().map(|s| s.value(w).to_repr()[0]).collect();
        let value = BigUint::from_bytes_le(&raw);
        let (n, bytes) = a.allocate(value.clone(), None);
        for (byte, source) in bytes.iter().zip(original) {
            a.c.eq(own(*byte, 1), source.linear());
        }
        let canonical = a.c.at_most_bit(&bytes, &value, &bound);
        let zero = a.is_zero(&n);
        let nonzero = a.not(zero);
        let accepted = pred::all(&mut a.c, &[canonical, nonzero]);
        valid.push(accepted);
        let one = a.one.clone();
        signature.push(a.select(accepted, &n, &one));
    }
    let (inverse, _) = a.inverse(&signature[1], 1);
    let u1 = a.mul(&zn, &inverse, 1);
    let u2 = a.mul(&signature[0], &inverse, 1);
    let b1 = a.digits(&u1);
    let b2 = a.digits(&u2);
    let gt = crate::operation::comb_tables(&p256::generator(), 16);
    let qt = crate::operation::comb_tables(&q, 16);
    let mut acc = a.identity();
    for step in (0..16).rev() {
        acc = a.add(&acc, &acc);
        for (table, entries, digits) in [
            (nonnative::WALLET_TABLE, &qt, &b2),
            (nonnative::POINT_TABLE, &gt, &b1),
        ] {
            for half in 0..2 {
                let selected = a.selected(table, entries, digits, half, step);
                acc = a.add(&acc, &selected);
            }
        }
    }
    let xr = a.linear(&[(1, &acc.x)], 0, 1);
    let equal = a.equal(&xr, &signature[0]);
    let finite = a.not(acc.infinity);
    valid.extend([finite, equal]);
    let verdict = pred::all(&mut a.c, &valid);
    let local = a.finish(w);
    // Exact canonical signed field -> SHA-256. Output bits bind all raw digest bytes.
    let sfam = w.family(1, 1);
    let at = w.arrays.len();
    let sh = sha::sha(at, sfam, 1, family, &[m]);
    w.arrays.extend(sh.arrays);
    w.tables.extend(sh.tables);
    let mut input = vec![];
    for byte in 0..4 {
        for j in 0..8 {
            let mut map = vec![None; 8];
            map[j] = Some(0);
            input = plus(
                &input,
                &link(
                    local,
                    message_bytes[4 * j + byte],
                    map,
                    Fp::from(1u64 << (8 * (3 - byte))),
                ),
            );
        }
    }
    w.arrays[at + sha::A_MSG].rows.push([
        plus(&own(0, 8), &scaled(&input, -Fp::ONE)),
        constant(vec![Fp::ONE; 8]),
        vec![],
    ]);
    // SHA owns a per-signature family; its rows may read the global scalar
    // tape, whereas a global row cannot directly read a child family.
    for byte in 0..4 {
        let bits = (0..8).fold(vec![], |a, bit| {
            plus(&a, &scaled(&own(8 * byte + bit, 8), Fp::from(1u64 << bit)))
        });
        let exact = (0..8).fold(vec![], |a, word| {
            let mut map = vec![None; 8];
            map[word] = Some(0);
            plus(
                &a,
                &link(local, zbytes[4 * (7 - word) + byte], map, Fp::ONE),
            )
        });
        w.arrays[at + sha::A_OUT].rows.push([
            plus(&bits, &scaled(&exact, -Fp::ONE)),
            constant(vec![Fp::ONE; 8]),
            vec![],
        ]);
    }
    let mut seen = std::collections::HashSet::new();
    w.tables.retain(|(_, id)| seen.insert(*id));
    SignatureValidity {
        valid: Source {
            array: local,
            cell: verdict,
            instance: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bootstrap,
        full::WalletKey,
        ipa::Gens,
        monetary_state,
        operation_auth::{
            self,
            tests::{Built, expose, finish, holds, predecessor, proof},
        },
        r1cs::Circuit,
        receipt,
        statement::{self, Kind},
    };
    use iroha_pasta::Eq as Vesta;
    fn build(key: &WalletKey, input: &request::CompletedRequest) -> Built {
        let (context, fixture) = bootstrap::fixture(key, &Gens::<Vesta>::new(1 << 16));
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let prior = predecessor(
            &mut w,
            family,
            bootstrap::public_profile(&context, &fixture),
        );
        let before = monetary_state::append_state(&mut w, family, &fixture.core, &fixture.rest);
        let credential =
            operation_auth::append_current(&mut w, family, &fixture.credential, &before, &prior)
                .unwrap();
        let fields = receipt::tests::fields(&fixture, Kind::Send);
        let statement = statement::append_statement(&mut w, family, Kind::Send, &fields);
        let own = receipt::tests::signed(key, &fixture.credential, &fields, Fp::ZERO);
        let _ = receipt::append_own(
            &mut w,
            family,
            &own,
            &credential,
            &statement,
            Kind::Send,
            receipt::PaymentDigest::Absent,
        )
        .unwrap();
        let original = crate::receive_originals::append_request(&mut w, family, input);
        let verdict = append_request_signature_total(&mut w, family, &original, &credential);
        expose(&mut w, family, 4, &[verdict.valid()]);
        // The exact unsigned message and all signature halves are public test inputs.
        let request = original.request();
        let ports: Vec<_> = std::iter::once(request.message)
            .chain(request.signature)
            .map(|cell| Source {
                array: request.local,
                cell,
                instance: 0,
            })
            .collect();
        expose(&mut w, family, 8, &ports);
        let raw_cells: usize = w
            .arrays
            .iter()
            .map(|a| a.cells() * a.values.first().map_or(0, Vec::len))
            .sum();
        eprintln!(
            "total signature wiring: arrays={} stored_field_values={raw_cells}",
            w.arrays.len()
        );
        let built = finish(w, 16);
        eprintln!(
            "total signature layout: sx={} row_bits={} col_bits={} constraints={} arrays={}",
            built.0.shape.sx,
            built.0.shape.row_bits,
            built.0.shape.col_bits,
            built.0.constraints,
            built.0.arrays.len()
        );
        built
    }
    fn matrix_hash(b: &Built) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        for a in 0..b.0.arrays.len() {
            for (matrix, row, col, value) in b.0.entries_of(&[Fp::from(31), Fp::from(47)], Some(a))
            {
                for v in [matrix, row, col] {
                    h.update(&(v as u64).to_le_bytes());
                }
                h.update(value.to_repr().as_ref());
            }
            if b.0.arrays[a].fixed {
                for column in &b.0.arrays[a].values {
                    for value in column {
                        h.update(value.to_repr().as_ref());
                    }
                }
            }
        }
        *h.finalize().as_bytes()
    }
    fn prove_verdict(b: &Built) {
        let (c, i, w) = b;
        let start = std::time::Instant::now();
        eprintln!("total signature proof starts: verdict={:?}, memory={:?}", i.io[4], c.proving_memory_accounting());
        let g = Gens::<Vesta>::new(c.shape.cols());
        let (p, _) = crate::spartan::prove_circuit(
            &c.shape,
            &g,
            i,
            w,
            &mut crate::fs::SpikeRng::new(b"total signature proof"),
            c,
        );
        assert!(crate::spartan::verify_circuit(&c.shape, &g, i, &p.to_bytes(), c).is_ok());
        let mut forged = i.clone();
        forged.io[4] = Fp::ONE - forged.io[4];
        assert!(crate::spartan::verify_circuit(&c.shape, &g, &forged, &p.to_bytes(), c).is_err());
        eprintln!("total signature proof and forged-verdict rejection completed in {:?}", start.elapsed());
    }
    #[test]
    fn total_request_signature_layout_and_rows() {
        let key = WalletKey::stand_in();
        let mut input = request::reference_request();
        let message = request::commitments(&input).unwrap().message;
        input.signature = bootstrap::sign(message, &key.d, "total Request valid signature");
        let b = build(&key, &input);
        assert_eq!(b.1.io[4], Fp::ONE);
        assert!(holds(&b));
        eprintln!("total signature retained memory breakdown {:?}", b.0.proving_memory_accounting());
    }
    #[test]
    fn total_request_signatures_have_same_vk_and_unforgeable_verdicts() {
        let key = WalletKey::stand_in();
        let mut input = request::reference_request();
        let m = request::commitments(&input).unwrap().message;
        input.signature = bootstrap::sign(m, &key.d, "total Request valid signature");
        let expected = {
            let good = build(&key, &input);
            assert!(holds(&good));
            assert_eq!(good.1.io[4], Fp::ONE);
            prove_verdict(&good);
            let fingerprint = matrix_hash(&good);
            let mut forged = good.1.clone();
            forged.io[4] = Fp::ZERO;
            let bad = (good.0, forged, good.2);
            assert!(!holds(&bad));
            fingerprint
        };
        let mut cases = vec![];
        for half in [0, 1] {
            let mut c = input.clone();
            c.signature[half * 32..half * 32 + 32].fill(0);
            cases.push(c);
            let mut c = input.clone();
            c.signature[half * 32..half * 32 + 32]
                .copy_from_slice(&certificate::scalar_bytes(&p256::n()));
            cases.push(c);
        }
        let mut c = input.clone();
        c.signature[0] ^= 1;
        cases.push(c);
        let mut c = input.clone();
        let s = p256::n() - BigUint::from_bytes_be(&c.signature[32..]);
        c.signature[32..].copy_from_slice(&certificate::scalar_bytes(&s));
        cases.push(c);
        let mut c = input.clone();
        c.signature = bootstrap::sign(
            m,
            &WalletKey::labelled("wrong Request signer").d,
            "wrong Request signature",
        );
        cases.push(c);
        // An adversary can choose canonical r,s whose computed point is infinity.
        let z = BigUint::from_bytes_be(&p256::sha256_one_block(m.to_repr().as_ref()));
        let r = (&p256::n() - (&z % p256::n())) * p256::inv(&key.d, &p256::n()) % p256::n();
        let mut infinity = input.clone();
        infinity.signature[..32].copy_from_slice(&certificate::scalar_bytes(&r));
        infinity.signature[32..].copy_from_slice(&certificate::scalar_bytes(&BigUint::one()));
        cases.push(infinity);
        for (i, input) in cases.iter().enumerate() {
            let mut b = build(&key, input);
            assert!(
                holds(&b),
                "false signature must remain a satisfiable total computation {i}"
            );
            assert_eq!(b.1.io[4], Fp::ZERO, "invalid signature {i}");
            assert_eq!(expected, matrix_hash(&b), "same VK {i}");
            if i == 0 || i == cases.len() - 1 {
                prove_verdict(&b);
            }
            b.1.io[4] = Fp::ONE;
            assert!(!holds(&b), "cannot claim true {i}");
        }
    }
}
