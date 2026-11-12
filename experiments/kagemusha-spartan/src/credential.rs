//! Approved future-generation 532-byte credential authentication research.
//!
//! Composes a root-signed Enrollment certificate, exact credential bytes,
//! and a derived issuer comb table. The public payment-table commitment is
//! authenticated as an issuer claim, not recomputed here. Signed evidence,
//! regulatory and self-contained renewal rules are enforced; original admission,
//! predecessor continuity and identity derivation remain separate obligations.
//! The production codec and its existing fixture still use 468 bytes.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    certificate::{self, Cells, Certificate, RootContext, field_modulus, fp, pow2, scalar_bytes},
    fs::SpikeRng,
    nonnative::{self, KeyLimbSource, KeyTable},
    p256::{self, Point},
    r1cs::{Instance, Witness},
    uniform::{Array, UniformCircuit, constant, link, own, plus, scaled},
};

pub const BODY_BYTES: usize = 532;
const SIGN_DOMAIN: u64 = u64::from_le_bytes(*b"kgwcred1");
const OBJECT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwocrd1");
const PAYMENT_KEY: usize = 130;
const TABLE: usize = 195;
const PROVIDER: usize = 259;
const ISSUER_CERTIFICATE: usize = 500;
const SIGNATURE: usize = BODY_BYTES;
const MESSAGE: usize = BODY_BYTES + 64;
pub const KEY_IO: usize = 256;
pub const TABLE_IO: usize = KEY_IO + 65;
pub const DIGEST_IO: usize = TABLE_IO + 64;
const ISSUER_TABLE: u32 = 9;
const PROVIDER_BYTES: [u8; 32] = [
    0xac, 0x0c, 0x9b, 0xdc, 0xa3, 0x9a, 0x5a, 0x00, 0x38, 0x52, 0x14, 0x93, 0x4f, 0x02, 0x99, 0xda,
    0x60, 0xd5, 0xff, 0x14, 0x25, 0xaa, 0x27, 0x0c, 0x53, 0x5a, 0x46, 0x22, 0x47, 0xa9, 0xfa, 0x1e,
];

#[derive(Clone)]
pub struct Credential {
    pub body: [u8; BODY_BYTES],
    pub signature: [u8; 64],
}

pub fn message(credential: &Credential) -> Fp {
    let inputs: Vec<_> = std::iter::once(Fp::from(BODY_BYTES as u64))
        .chain(
            credential
                .body
                .chunks(31)
                .map(|b| fp(&BigUint::from_bytes_le(b))),
        )
        .collect();
    poseidon::hash_with_domain(SIGN_DOMAIN, &inputs)
}
pub fn digest(credential: &Credential) -> Fp {
    let r = BigUint::from_bytes_be(&credential.signature[..32]);
    let s = BigUint::from_bytes_be(&credential.signature[32..]);
    poseidon::hash_with_domain(
        OBJECT_DOMAIN,
        &[
            message(credential),
            fp(&(&r % pow2(128))),
            fp(&(&r >> 128)),
            fp(&(&s % pow2(128))),
            fp(&(&s >> 128)),
        ],
    )
}

struct CurveInput {
    cells: [[usize; 3]; 4],
    values: [BigUint; 4],
    modulus: BigUint,
    coefficient: u64,
    b: BigUint,
}
fn curve_input(
    c: &mut Cells,
    bytes: &[u8],
    offset: usize,
    little_endian: bool,
    modulus: BigUint,
    coefficient: u64,
    b: BigUint,
) -> CurveInput {
    let indices = |off| {
        if little_endian {
            (off..off + 32).collect::<Vec<_>>()
        } else {
            (off..off + 32).rev().collect()
        }
    };
    let integer = |off| {
        if little_endian {
            BigUint::from_bytes_le(&bytes[off..off + 32])
        } else {
            BigUint::from_bytes_be(&bytes[off..off + 32])
        }
    };
    let (x, y) = (integer(offset), integer(offset + 32));
    let (xi, yi) = (indices(offset), indices(offset + 32));
    c.at_most(&xi, &x, &(&modulus - 1u32));
    c.at_most(&yi, &y, &(&modulus - 1u32));
    let xc = c.value(&x, &xi);
    let yc = c.value(&y, &yi);
    let x2 = &x * &x % &modulus;
    let residue =
        (&y * &y + &x * coefficient + &modulus * 2u32 - ((&x * &x2 + &b) % &modulus)) % &modulus;
    let mut auxiliary = |v: &BigUint| {
        let indices: Vec<_> = scalar_bytes(v).iter().rev().map(|b| c.byte(*b)).collect();
        c.value(v, &indices)
    };
    let x2c = auxiliary(&x2);
    let rc = auxiliary(&residue);
    for cell in rc {
        c.zero(own(cell, 1));
    }
    CurveInput {
        cells: [xc, yc, x2c, rc],
        values: [x, y, x2, residue],
        modulus,
        coefficient,
        b,
    }
}

pub struct Built {
    pub circuit: UniformCircuit<Fp>,
    pub instance: Instance<Fp>,
    pub witness: Witness<Fp>,
    local: usize,
    certificate_local: usize,
    issuer_table: usize,
}

/// Authenticate the exact future-generation issuer claims and their
/// self-contained evidence/regulatory rules. This does not prove admission,
/// predecessor renewal continuity, identity hashes or payment-table derivation.
pub fn build(
    context: &RootContext,
    cert: &Certificate,
    credential: &Credential,
) -> Result<Built, &'static str> {
    let issuer = Point {
        x: BigUint::from_bytes_be(&cert.body[36..68]),
        y: BigUint::from_bytes_be(&cert.body[68..100]),
    };
    if issuer.x >= p256::p() || issuer.y >= p256::p() || !p256::on_curve(&issuer) {
        return Err("invalid issuer key");
    }
    let m = message(credential);
    let mb = m.to_repr();
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(mb.as_ref()));
    let r = BigUint::from_bytes_be(&credential.signature[..32]);
    let s = BigUint::from_bytes_be(&credential.signature[32..]);
    if !p256::verify(
        &issuer,
        &z,
        &p256::Signature {
            r: r.clone(),
            s: s.clone(),
        },
    ) {
        return Err("credential signature has no ECDSA witness");
    }
    let prepared = certificate::prepare(context, cert)?;
    let gfam = prepared.gfam;
    let certificate_local = prepared.local;
    let source = KeyLimbSource {
        array: prepared.local,
        x: prepared.issuer_x,
        y: prepared.issuer_y,
        instance: 0,
    };
    let mut w = prepared.wiring;
    let mut c = Cells::new(gfam);
    for byte in credential
        .body
        .iter()
        .chain(&credential.signature)
        .chain(mb.as_ref())
    {
        c.byte(*byte);
    }
    crate::credential_policy::constrain(&mut c, &credential.body);
    for (k, v) in [(0, 1), (1, 0), (PAYMENT_KEY, 4)].into_iter().chain(
        PROVIDER_BYTES
            .iter()
            .enumerate()
            .map(|(i, b)| (PROVIDER + i, *b)),
    ) {
        c.eq(own(k, 1), constant(vec![Fp::from(v as u64)]));
    }
    // The scheme is carried from the certificate's trusted public input,
    // rather than fixed in the VK that participates in scheme identity.
    for i in 0..32 {
        c.eq(
            own(2 + i, 1),
            link(prepared.local, 2 + i, vec![Some(0)], Fp::ONE),
        );
    }
    // Required identifier bindings are nonzero. Their identity preimages are
    // issuer claims; the self-contained signed policy rules are checked above.
    for start in [34, 66, 98, 404, 448] {
        let sum = (start..start + 32).fold(vec![], |a, k| plus(&a, &own(k, 1)));
        let value = Fp::from(
            credential.body[start..start + 32]
                .iter()
                .map(|b| *b as u64)
                .sum::<u64>(),
        );
        let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
        c.a.rows
            .push([sum, own(inverse, 1), constant(vec![Fp::ONE])]);
    }
    let key = curve_input(
        &mut c,
        &credential.body,
        PAYMENT_KEY + 1,
        false,
        p256::p(),
        3,
        p256::b(),
    );
    let table = curve_input(
        &mut c,
        &credential.body,
        TABLE,
        true,
        nonnative::fq_modulus(),
        0,
        BigUint::from(5u32),
    );
    let cert_bytes: Vec<_> = (ISSUER_CERTIFICATE..ISSUER_CERTIFICATE + 32).collect();
    let cert_integer = BigUint::from_bytes_le(&credential.body[ISSUER_CERTIFICATE..]);
    c.at_most(&cert_bytes, &cert_integer, &(field_modulus() - 1u32));
    let certcell = c.cell(fp(&cert_integer), 255);
    c.eq(own(certcell, 1), c.bytes(&cert_bytes));
    c.eq(
        own(certcell, 1),
        link(prepared.local, prepared.digest_cell, vec![Some(0)], Fp::ONE),
    );
    let cert_inverse = c.cell(
        Option::<Fp>::from(fp(&cert_integer).invert()).unwrap_or(Fp::ZERO),
        255,
    );
    c.a.rows.push([
        own(certcell, 1),
        own(cert_inverse, 1),
        constant(vec![Fp::ONE]),
    ]);
    let rle: Vec<_> = (SIGNATURE..SIGNATURE + 32).rev().collect();
    let sle: Vec<_> = (SIGNATURE + 32..SIGNATURE + 64).rev().collect();
    c.at_most(&sle, &s, &(p256::n() >> 1));
    let rlimbs = c.value(&r, &rle);
    let slimbs = c.value(&s, &sle);
    let mle: Vec<_> = (MESSAGE..MESSAGE + 32).collect();
    c.at_most(
        &mle,
        &BigUint::from_bytes_le(mb.as_ref()),
        &(field_modulus() - 1u32),
    );
    let mcell = c.cell(m, 255);
    c.eq(own(mcell, 1), c.bytes(&mle));
    let mstate = [c.cell(Fp::ZERO, 255), mcell, c.cell(Fp::ZERO, 255)];
    let dg = digest(credential);
    let dcell = c.cell(dg, 255);
    let dstate = [c.cell(Fp::ZERO, 255), dcell, c.cell(Fp::ZERO, 255)];
    let body_items: Vec<_> = std::iter::once((
        Fp::from(BODY_BYTES as u64),
        constant(vec![Fp::from(BODY_BYTES as u64)]),
    ))
    .chain(credential.body.chunks(31).enumerate().map(|(i, b)| {
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
    let mut output = vec![Fp::ZERO; 256];
    for (i, b) in credential.body[PAYMENT_KEY..TABLE + 64].iter().enumerate() {
        output[i] = Fp::from(*b as u64);
    }
    output[129] = dg;
    let io = w.push(Array {
        name: "authenticated credential claims",
        family: gfam,
        rho: 8,
        r_count: 130,
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![output],
    });
    w.io_arrays.push((io, KEY_IO));
    for (k, i) in (PAYMENT_KEY..TABLE + 64)
        .enumerate()
        .map(|(i, k)| (k, i))
        .chain(std::iter::once((dcell, 129)))
    {
        w.arrays[local].rows.push([
            plus(
                &own(k, 1),
                &scaled(&link(io, 0, vec![Some(i)], Fp::ONE), -Fp::ONE),
            ),
            constant(vec![Fp::ONE]),
            vec![],
        ]);
    }
    for p in [key, table] {
        certificate::curve_relation(
            &mut w,
            gfam,
            local,
            p.cells,
            p.values.each_ref(),
            &p.modulus,
            p.coefficient,
            &p.b,
        );
    }
    let mut rng = SpikeRng::new(b"credential exact native sponge");
    certificate::hash_relation(
        &mut w,
        gfam,
        local,
        SIGN_DOMAIN,
        &body_items,
        mstate,
        &mut rng,
    );
    certificate::hash_relation(
        &mut w,
        gfam,
        local,
        OBJECT_DOMAIN,
        &obj_items,
        dstate,
        &mut rng,
    );
    assert_eq!(w.arrays[local].values[mcell][0], m);
    assert_eq!(w.arrays[local].values[dcell][0], dg);
    let (_, comb) = certificate::add_signature(
        &mut w,
        gfam,
        local,
        MESSAGE,
        m,
        (issuer.clone(), z, r, s),
        [rlimbs, slimbs],
        KeyTable::Witness { id: ISSUER_TABLE },
    );
    let derivation = w.arrays.len();
    let kt = nonnative::key_tables(derivation, gfam, &issuer);
    w.arrays.extend(kt.arrays);
    w.tables.extend(kt.tables);
    let issuer_table = comb + nonnative::A_WTABLE;
    nonnative::bind_derived_key_table(&mut w.arrays, derivation, issuer_table, source);
    let mut ids = std::collections::HashSet::new();
    w.tables.retain(|(_, id)| ids.insert(*id));
    let (arrays, pi, counts, tables, copies) = w.finish();
    let (circuit, instance, witness) = UniformCircuit::build_with_copies(
        "credential-authentication",
        arrays,
        pi,
        counts,
        tables,
        copies,
        14,
        512,
        b"credential-authentication",
    );
    Ok(Built {
        circuit,
        instance,
        witness,
        local,
        certificate_local,
        issuer_table,
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
    fn reference(key: &str) -> Vec<u8> {
        let value = include_str!("../fixtures/credential-source-vectors.txt")
            .lines()
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| *k == key)
            .unwrap()
            .1;
        hex(value)
    }
    fn root() -> RootContext {
        RootContext {
            scheme: reference("certificate_body_108")[2..34].try_into().unwrap(),
            root: p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[0x11; 32])).unwrap(),
        }
    }
    fn sign(m: Fp, d: &BigUint, label: &str) -> [u8; 64] {
        let z = BigUint::from_bytes_be(&p256::sha256_one_block(m.to_repr().as_ref()));
        let mut sig = p256::sign(d, &z, &p256::scalar_below(label, &p256::n()));
        if sig.s > p256::n() >> 1 {
            sig.s = p256::n() - sig.s;
        }
        let mut raw = [0; 64];
        raw[..32].copy_from_slice(&scalar_bytes(&sig.r));
        raw[32..].copy_from_slice(&scalar_bytes(&sig.s));
        raw
    }
    fn resign(c: &mut Credential, issuer: u8) {
        c.signature = sign(
            message(c),
            &BigUint::from_bytes_be(&[issuer; 32]),
            "credential 532 fixture signature",
        );
    }
    fn commitment(q: &Point) -> [u8; 64] {
        let gens = Gens::<Vesta>::new(4096);
        let point = crate::full::wallet_table_commitment(q, &gens);
        let limbs = spartan::limbs_of_point::<Vesta>(&point);
        let mut out = [0; 64];
        for (i, limb) in limbs.iter().enumerate() {
            out[16 * i..16 * i + 16].copy_from_slice(&limb.to_repr().as_ref()[..16]);
        }
        out
    }
    fn fixture(issuer: u8) -> (Certificate, Credential) {
        let mut cert = Certificate {
            body: reference("certificate_body_108").try_into().unwrap(),
            signature: reference("certificate_signature").try_into().unwrap(),
        };
        if issuer != 0x26 {
            let key =
                p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[issuer; 32])).unwrap();
            cert.body[36..68].copy_from_slice(&scalar_bytes(&key.x));
            cert.body[68..100].copy_from_slice(&scalar_bytes(&key.y));
            cert.signature = sign(
                certificate::message(&cert),
                &BigUint::from_bytes_be(&[0x11; 32]),
                "credential rotation certificate",
            );
        }
        let old = reference("credential_body_468");
        assert_eq!(old.len(), 468);
        let payment = Point {
            x: BigUint::from_bytes_be(&old[131..163]),
            y: BigUint::from_bytes_be(&old[163..195]),
        };
        let table = commitment(&payment);
        let mut c = Credential {
            body: [0; 532],
            signature: [0; 64],
        };
        c.body[..195].copy_from_slice(&old[..195]);
        c.body[195..259].copy_from_slice(&table);
        c.body[259..].copy_from_slice(&old[195..]);
        c.body[500..].copy_from_slice(certificate::certificate_digest(&cert).to_repr().as_ref());
        resign(&mut c, issuer);
        (cert, c)
    }
    fn holds(b: &Built) -> bool {
        let mut w = b.witness.w.clone();
        let rel = b.circuit.instantiate(
            &b.circuit.shape,
            &[Fp::from(31), Fp::from(47)],
            Some(&mut w),
        );
        let z = crate::r1cs::z_vector(&b.circuit.shape, &b.instance, &Witness { w });
        let (a, b, c) = rel.abc(&b.circuit.shape, &z);
        a.iter().zip(b).zip(c).all(|((a, b), c)| *a * b == c)
    }
    fn other_scheme_fixture() -> (RootContext, Certificate, Credential) {
        let (mut cert, mut credential) = fixture(0x22);
        let mut context = root();
        context.scheme = [0x77; 32];
        cert.body[2..34].copy_from_slice(&context.scheme);
        cert.signature = sign(
            certificate::message(&cert),
            &BigUint::from_bytes_be(&[0x11; 32]),
            "second scheme certificate",
        );
        credential.body[2..34].copy_from_slice(&context.scheme);
        credential.body[500..]
            .copy_from_slice(certificate::certificate_digest(&cert).to_repr().as_ref());
        resign(&mut credential, 0x22);
        (context, cert, credential)
    }
    #[test]
    fn approved_532_fixture_has_exact_offsets_and_separate_signature() {
        let old = reference("credential_body_468");
        let old_items: Vec<_> = std::iter::once(Fp::from(468))
            .chain(old.chunks(31).map(|b| fp(&BigUint::from_bytes_le(b))))
            .collect();
        assert_eq!(
            poseidon::hash_with_domain(SIGN_DOMAIN, &old_items)
                .to_repr()
                .as_ref(),
            reference("credential_message_468")
        );
        let (cert, c) = fixture(0x26);
        assert_eq!(&c.body[..195], &old[..195]);
        assert_eq!(&c.body[259..], &old[195..]);
        assert_eq!(&c.body[PROVIDER..PROVIDER + 32], &PROVIDER_BYTES);
        assert_eq!(
            &c.body[500..],
            certificate::certificate_digest(&cert).to_repr().as_ref()
        );
        assert_eq!(1 + c.body.chunks(31).count(), 19);
        assert_eq!(c.body.chunks(31).last().unwrap().len(), 5);
        assert_ne!(
            message(&c).to_repr().as_ref(),
            reference("credential_message_468")
        );
        assert_ne!(
            c.signature.as_slice(),
            reference("credential_signature_468")
        );
        for (key, actual) in [
            ("credential_532_body", c.body.to_vec()),
            ("credential_532_table", c.body[TABLE..TABLE + 64].to_vec()),
            (
                "credential_532_message",
                message(&c).to_repr().as_ref().to_vec(),
            ),
            ("credential_532_signature", c.signature.to_vec()),
            (
                "credential_532_digest",
                digest(&c).to_repr().as_ref().to_vec(),
            ),
        ] {
            let expected = include_str!("../fixtures/credential-532.txt")
                .lines()
                .filter_map(|line| line.split_once('='))
                .find(|(name, _)| *name == key)
                .unwrap()
                .1;
            assert_eq!(actual, hex(expected), "pinned research vector: {key}");
        }
    }
    #[test]
    fn credential_chain_and_issuer_substitutions_are_constrained() {
        let ctx = root();
        let (cert, honest) = fixture(0x26);
        let mut b = build(&ctx, &cert, &honest).unwrap();
        assert!(
            holds(&b),
            "certificate, derived issuer table and exact credential"
        );
        for index in [
            KEY_IO + 1,
            TABLE_IO,
            DIGEST_IO,
            certificate::KEY_IO + 1,
            certificate::DIGEST_IO,
            certificate::SCHEME_IO,
        ] {
            b.instance.io[index] += Fp::ONE;
            assert!(!holds(&b), "authenticated output {index}");
            b.instance.io[index] -= Fp::ONE;
        }
        for (a, k, i) in [
            (b.local, 500, 0),
            (b.local, 195, 0),
            (b.certificate_local, 36, 0),
            (b.issuer_table, 1, 13),
        ] {
            let pos = b.circuit.cell_index(a, k, i);
            b.witness.w[pos] += Fp::ONE;
            assert!(!holds(&b), "certificate or issuer table substitution");
            b.witness.w[pos] -= Fp::ONE;
        }
        for (label, index) in [
            ("certificate digest", 500),
            ("scheme", 2),
            ("provider", 259),
            ("payment point", 143),
            ("commitment point", TABLE + 14),
        ] {
            let mut wrong = honest.clone();
            wrong.body[index] ^= 1;
            resign(&mut wrong, 0x26);
            assert!(
                !holds(&build(&ctx, &cert, &wrong).unwrap()),
                "correct issuer signature cannot waive {label}"
            );
        }
        let (rotated, mut wrong) = fixture(0x22);
        assert!(
            holds(&build(&ctx, &rotated, &wrong).unwrap()),
            "rotated issuer is valid"
        );
        // Both certificates and the signature are valid; this exact join is not.
        wrong.body[500..].copy_from_slice(&honest.body[500..]);
        resign(&mut wrong, 0x22);
        assert!(
            !holds(&build(&ctx, &rotated, &wrong).unwrap()),
            "another valid issuer cannot substitute its certificate"
        );
        let mut foreign = honest.clone();
        resign(&mut foreign, 0x33);
        assert!(
            build(&ctx, &cert, &foreign).is_err(),
            "credential must use certificate's exact issuer"
        );
        let mut altered = honest.clone();
        altered.body[TABLE] ^= 1;
        assert!(
            build(&ctx, &cert, &altered).is_err(),
            "table bytes are signed"
        );
        let mut high = honest.clone();
        let s = BigUint::from_bytes_be(&high.signature[32..]);
        high.signature[32..].copy_from_slice(&scalar_bytes(&(p256::n() - s)));
        assert!(
            !holds(&build(&ctx, &cert, &high).unwrap()),
            "high-S credential twin is rejected"
        );
        let (other_context, other_cert, other_credential) = other_scheme_fixture();
        assert!(holds(
            &build(&other_context, &other_cert, &other_credential).unwrap()
        ));
        assert!(
            !holds(&build(&ctx, &other_cert, &other_credential).unwrap()),
            "wrong expected public scheme"
        );
    }
    #[test]
    fn signed_table_commitment_still_requires_payment_table_derivation() {
        let (cert, mut c) = fixture(0x26);
        // A valid commitment to a different key remains an authenticated issuer
        // claim. Issuer/native intake recomputation from the payment key is
        // still required; this relation does not recompute it.
        c.body[TABLE..TABLE + 64].copy_from_slice(&commitment(&root().root));
        resign(&mut c, 0x26);
        assert!(holds(&build(&root(), &cert, &c).unwrap()));
    }
    #[test]
    fn signed_evidence_renewal_and_regulatory_violations_are_refused() {
        let (cert, honest) = fixture(0x26);
        let mut cases = Vec::new();
        let mut bad = honest.clone();
        bad.body[291] = 0;
        cases.push(("evidence kind", bad));
        let mut bad = honest.clone();
        bad.body[333] |= 2;
        bad.body[389] |= 2;
        cases.push(("reserved fact bit 9", bad));
        let mut bad = honest.clone();
        bad.body[292..324].fill(0);
        bad.body[348..380].fill(0);
        cases.push(("zero evidence digest", bad));
        let mut bad = honest.clone();
        bad.body[332] &= !1;
        bad.body[388] &= !1;
        cases.push(("missing hardware key fact", bad));
        let mut bad = honest.clone();
        bad.body[488] = 1;
        let enrollment = u64::from_le_bytes(bad.body[324..332].try_into().unwrap());
        bad.body[380..388].copy_from_slice(&(enrollment - 1).to_le_bytes());
        cases.push(("renewal evidence goes backwards", bad));
        let mut bad = honest.clone();
        bad.body[436] = 4;
        cases.push(("undefined regulatory control", bad));
        let mut bad = honest.clone();
        bad.body[436] = 1;
        cases.push(("quota response bound is absent", bad));
        let mut bad = honest.clone();
        bad.body[436] = 2;
        bad.body[440] = 1;
        cases.push(("permitted lease has zero expiry", bad));
        for (name, mut bad) in cases {
            resign(&mut bad, 0x26);
            assert!(
                !holds(&build(&root(), &cert, &bad).unwrap()),
                "correctly signed {name}"
            );
        }
    }
    #[test]
    #[ignore = "bounded full authentication proofs; schedule after concurrent platform gates"]
    fn rotated_credential_issuers_prove_under_one_root_vk() {
        let (cert1, cred1) = fixture(0x26);
        let (context2, cert2, cred2) = other_scheme_fixture();
        let a = build(&root(), &cert1, &cred1).unwrap();
        let b = build(&context2, &cert2, &cred2).unwrap();
        assert!(holds(&a) && holds(&b));
        let ch = [Fp::from(31), Fp::from(47)];
        assert_eq!(
            a.circuit.entries(&ch),
            b.circuit.entries(&ch),
            "rotated issuers and public schemes keep the same complete matrix"
        );
        let gens = Gens::<Vesta>::new(a.circuit.shape.cols());
        let mut rng = SpikeRng::new(b"credential issuer rotation proof");
        for source in [&a, &b] {
            let (proof, _) = spartan::prove_circuit(
                &a.circuit.shape,
                &gens,
                &source.instance,
                &source.witness,
                &mut rng,
                &a.circuit,
            );
            println!(
                "standalone credential authentication: {} proof bytes, 2^{} constraints, row_bits={}, col_bits={}",
                proof.to_bytes().len(),
                a.circuit.shape.sx,
                a.circuit.shape.row_bits,
                a.circuit.shape.col_bits
            );
            assert!(
                spartan::verify_circuit(
                    &a.circuit.shape,
                    &gens,
                    &source.instance,
                    &proof.to_bytes(),
                    &a.circuit
                )
                .is_ok()
            );
            for index in [
                TABLE_IO,
                DIGEST_IO,
                certificate::KEY_IO + 1,
                certificate::DIGEST_IO,
                certificate::SCHEME_IO,
            ] {
                let mut wrong = Instance {
                    io: source.instance.io.clone(),
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
    }
}
