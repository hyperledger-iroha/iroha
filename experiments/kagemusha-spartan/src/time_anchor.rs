//! RefreshPolicy(TimeAnchor): authenticate one issuer-signed TimeAnchor,
//! replace the held anchor digest and raise the accepted-time floor exactly.
//!
//! The 138-byte body (`kgwtanc1`, wire §3.3) is signed by a TimeAnchor-role
//! key (role 4) whose 108-byte signer certificate is signed by the installed
//! scheme root (wire §3.1). The relation checks both signatures in circuit:
//! the root's against its fixed comb table (a circuit constant, as in
//! Bootstrap), the anchor's against a comb table derived in circuit from the
//! authenticated certificate key, never the wallet table or a supplied one.
//!
//! The transition (E4-F1-F2-DESIGN §5.3-§5.5) changes exactly the sequence
//! (+1, checked u128), the accepted-time floor (`max(old floor, T)`, exact),
//! the nonce (native entropy) and the held `time_anchor` digest, which must
//! differ from the held one. It renews no credential or lease, changes no key,
//! table, prior-credential root, quota or replay map, and materializes no
//! lineage adjustment. An older authenticated anchor is valid and keeps the
//! floor. Nothing here proves genuine OS time, boot identity or response
//! latency: that is native direct-time custody (§5.8), not this relation.
//!
//! All authority predicates are total, so this branch shares one fixed
//! Control program with Load (`control.rs`): with `t = control_mode`, every
//! Refresh-only rule is `t·X = 0` and an inactive branch proves
//! `Authority::dummy()`.
// Selected by the Control program in the F2 integration step (full.rs kinds
// and programs, engine and native witness owners).
#![cfg_attr(not(test), allow(dead_code))]

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    certificate::{self, Cells, Certificate, Role, field_modulus, fp, pow2},
    credential_policy::{all, negate, zero_predicate},
    fs::SpikeRng,
    full::Wiring,
    monetary_state::StateCells,
    nonnative,
    operation_auth::CurrentCredential,
    p256::Point,
    send_arithmetic::Source,
    signature::{self, TotalKey},
    statement::StatementCells,
    uniform::{Array, constant, own, plus, scaled},
};

pub const BODY_BYTES: usize = 138;
pub const SIGNATURE_BYTES: usize = 64;
pub const SIGN_DOMAIN: u64 = u64::from_le_bytes(*b"kgwtanc1");
pub const OBJECT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwotim1");
/// RefreshPolicy statement tag and the TimeAnchor update kind (wire §3.2).
pub const REFRESH_TAG: u64 = 7;
pub const UPDATE_KIND: u64 = 4;
/// Core30 fields a RefreshTimeAnchor changes: sequence, accepted-time floor
/// and nonce. Rest6 changes only `time_anchor` (index 5).
pub const CHANGED_CORE: [usize; 3] = [11, 28, 29];
/// Lookup id of the comb table derived from the TimeAnchor signer key.
const SIGNER_TABLE: u32 = 10;
const SIGNATURE: usize = BODY_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeAnchor {
    pub body: [u8; BODY_BYTES],
    pub signature: [u8; SIGNATURE_BYTES],
}

/// The exact originals a RefreshTimeAnchor consumes.
#[derive(Clone)]
pub struct Authority {
    pub certificate: Certificate,
    pub anchor: TimeAnchor,
}
impl Authority {
    /// Canonical inactive-branch authority: all-zero bytes. Every total
    /// predicate is false on it and every helper row stays satisfiable.
    pub fn dummy() -> Self {
        Self {
            certificate: Certificate {
                body: [0; 108],
                signature: [0; 64],
            },
            anchor: TimeAnchor {
                body: [0; BODY_BYTES],
                signature: [0; SIGNATURE_BYTES],
            },
        }
    }
}

fn items(body: &[u8; BODY_BYTES]) -> Vec<Fp> {
    std::iter::once(Fp::from(BODY_BYTES as u64))
        .chain(
            body.chunks(31)
                .map(|chunk| fp(&BigUint::from_bytes_le(chunk))),
        )
        .collect()
}
/// Signing message `P_bytes(kgwtanc1, body)`.
pub fn message(anchor: &TimeAnchor) -> Fp {
    poseidon::hash_with_domain(SIGN_DOMAIN, &items(&anchor.body))
}
fn halves(bytes: &[u8]) -> [Fp; 2] {
    let x = BigUint::from_bytes_be(bytes);
    [fp(&(&x % pow2(128))), fp(&(&x >> 128))]
}
/// Object digest `P(kgwotim1, [m, r_lo, r_hi, s_lo, s_hi])`: the `update`
/// a RefreshPolicy(TimeAnchor) applies and the held Rest6 `time_anchor`.
pub fn digest(anchor: &TimeAnchor) -> Fp {
    let [r_lo, r_hi] = halves(&anchor.signature[..32]);
    let [s_lo, s_hi] = halves(&anchor.signature[32..]);
    poseidon::hash_with_domain(OBJECT_DOMAIN, &[message(anchor), r_lo, r_hi, s_lo, s_hi])
}
/// The signed `issuer_time_ms` `T`.
pub fn issuer_time(anchor: &TimeAnchor) -> u64 {
    u64::from_le_bytes(anchor.body[98..106].try_into().unwrap())
}

/// A natively authenticated anchor: its object digest and signed time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verified {
    pub digest: Fp,
    pub issuer_time: u64,
}

/// Native authentication with the same rules as the relation: a
/// root-certified TimeAnchor-role signer of the trusted scheme, version 1,
/// the wallet's scheme and wallet, a nonzero nonce, the exact certificate
/// digest in the canonical nonzero signer field and a canonical low-S
/// signature over `m`. It does not decide whether this is a direct exchange
/// (§5.8): a cached or peer-carried anchor authenticates the same way.
pub fn authenticate(
    authority: &Authority,
    scheme: &[u8; 32],
    wallet: &[u8; 32],
    root: &Point,
) -> Result<Verified, &'static str> {
    let key = certificate::authenticate_signer(
        &authority.certificate,
        scheme,
        root,
        Role::TimeAnchor,
    )?;
    let body = &authority.anchor.body;
    if body[..2] != 1u16.to_le_bytes() {
        return Err("time anchor version");
    }
    if body[2..34] != scheme[..] {
        return Err("time anchor scheme");
    }
    if body[34..66] != wallet[..] {
        return Err("time anchor wallet");
    }
    if body[66..98] == [0; 32] {
        return Err("zero time anchor nonce");
    }
    let signer = BigUint::from_bytes_le(&body[106..138]);
    if signer == BigUint::ZERO || signer >= field_modulus() {
        return Err("noncanonical time anchor signer field");
    }
    if fp(&signer) != certificate::certificate_digest(&authority.certificate) {
        return Err("time anchor signer certificate");
    }
    certificate::verify_native(&key, message(&authority.anchor), &authority.anchor.signature)
        .map_err(|_| "time anchor signature")?;
    Ok(Verified {
        digest: digest(&authority.anchor),
        issuer_time: issuer_time(&authority.anchor),
    })
}

fn integer(x: Fp) -> BigUint {
    BigUint::from_bytes_le(x.to_repr().as_ref())
}

/// Native exact transition: `sequence + 1`, `floor = max(floor, T)`, the
/// fresh nonzero nonce and the new held anchor; every other field unchanged.
pub fn refresh(
    core: &[Fp; 30],
    rest: &[Fp; 6],
    anchor: &Verified,
    nonce: Fp,
) -> Result<([Fp; 30], [Fp; 6]), &'static str> {
    if anchor.digest == Fp::ZERO || anchor.digest == rest[5] {
        return Err("time anchor must differ from the held one");
    }
    if nonce == Fp::ZERO {
        return Err("zero successor nonce");
    }
    let sequence = integer(core[11]);
    if sequence >= pow2(128) - 1u32 {
        return Err("sequence overflow");
    }
    let floor = integer(core[28]);
    if floor >= pow2(64) {
        return Err("noncanonical accepted-time floor");
    }
    let mut after = *core;
    after[11] = core[11] + Fp::ONE;
    after[28] = fp(&floor.max(BigUint::from(anchor.issuer_time)));
    after[29] = nonce;
    let mut rest_after = *rest;
    rest_after[5] = anchor.digest;
    Ok((after, rest_after))
}

/// The exact 26-field RefreshPolicy(TimeAnchor) statement (§5.5): common
/// bindings, zero lineage fields, tag 7, update kind 4, update `d`, the
/// successor floor, zero tail.
pub fn statement(
    relation: [Fp; 2],
    before: (&[Fp; 30], &[Fp; 6]),
    after: (&[Fp; 30], &[Fp; 6]),
    update: Fp,
) -> [Fp; 26] {
    let (core, rest) = after;
    let mut st = [Fp::ZERO; 26];
    st[0] = Fp::ONE;
    st[1..3].copy_from_slice(&relation);
    st[3..7].copy_from_slice(&core[1..5]);
    st[7] = core[7];
    st[8] = core[0];
    st[9] = core[11];
    st[10] = core[13];
    st[11] = before.0[22];
    st[14] = crate::monetary_state::commitment(before.0, before.1);
    st[15] = crate::monetary_state::commitment(core, rest);
    st[16] = Fp::from(REFRESH_TAG);
    st[17] = Fp::from(UPDATE_KIND);
    st[18] = update;
    st[19] = core[28];
    st
}

/// Constrained ports of the Refresh branch.
pub struct RefreshBranch {
    valid: Source,
    digest: Source,
    time: Source,
    floor: Source,
}
impl RefreshBranch {
    /// Total authority verdict: certificate (role 4, root) and anchor.
    pub fn valid(&self) -> Source {
        self.valid
    }
    /// The exact anchor object digest `d`.
    pub fn digest(&self) -> Source {
        self.digest
    }
    /// The signed issuer time `T` (u64).
    pub fn time(&self) -> Source {
        self.time
    }
    /// `max(C[28], T)`, exact for every u64 pair.
    pub fn floor(&self) -> Source {
        self.floor
    }
}

fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

/// The RefreshTimeAnchor branch over already-opened states, statement and
/// verified current credential, enabled by the constrained Boolean `enable`
/// (`control_mode`). Authority opening, signer-table derivation and the
/// maximum are hard total computations in both modes; each Refresh-only rule
/// is `enable·X = 0`: authority valid, `C'[28] = max`, `R'[5] = d`,
/// `d ≠ 0`, `d ≠ R[5]`, statement update/floor, and preservation of the
/// fields only the Load branch changes (`C[9]`, `C[13]`, `C[19]`). Fields
/// both branches preserve, and the sequence step, are the common rows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn append_branch(
    w: &mut Wiring<Fp>,
    family: usize,
    enable: Source,
    authority: &Authority,
    root: &Point,
    [before, after]: [&StateCells; 2],
    statement: &StatementCells,
    credential: &CurrentCredential,
) -> RefreshBranch {
    for s in [enable, credential.scheme()[0], credential.wallet()[0]] {
        assert_eq!(w.arrays[s.array].family, family, "same-family branch port");
    }
    let signer = certificate::append_signer_total(
        w,
        family,
        root,
        &authority.certificate,
        Role::TimeAnchor,
        credential.scheme(),
    );
    // The signer's comb tables, derived in circuit from the (selected)
    // certificate key and bound row by row to the lookup table.
    let q = signer.point().clone();
    let table = w.push(Array {
        name: "time-anchor signer derived comb table",
        family,
        rho: 9,
        r_count: nonnative::WALLET_ROWS,
        fixed: false,
        lookups: vec![],
        widths: vec![9, 88, 88, 80, 88, 88, 80],
        rows: vec![],
        values: nonnative::wallet_table_values(&q),
    });
    w.tables.push((table, SIGNER_TABLE));
    let derivation = w.arrays.len();
    let derived = nonnative::key_tables(derivation, family, &q);
    w.arrays.extend(derived.arrays);
    w.tables.extend(derived.tables);
    nonnative::bind_derived_key_table(&mut w.arrays, derivation, table, signer.key());
    let opening = append_opening(w, family, &authority.anchor, signer.digest(), credential);
    let signature = std::array::from_fn(|k| source(opening.local, SIGNATURE + k));
    let anchor_signature = signature::append_signature_total(
        w,
        family,
        "total time-anchor ECDSA arithmetic",
        source(opening.local, opening.message),
        &signature,
        TotalKey {
            table: SIGNER_TABLE,
            q: &q,
        },
        vec![],
    );
    let mut ids = std::collections::HashSet::new();
    w.tables.retain(|(_, id)| ids.insert(*id));

    let t = enable.linear();
    let tv = enable.value(w);
    let mut c = Cells::new(family);
    c.a.name = "RefreshTimeAnchor verdict, maximum and enabled joins";
    let parts = [
        signer.valid(),
        source(opening.local, opening.valid),
        anchor_signature.valid(),
    ]
    .map(|s| {
        let k = c.bit(s.value(w) == Fp::ONE);
        c.eq(own(k, 1), s.linear());
        k
    });
    let valid = all(&mut c, &parts);
    c.a.rows.push([
        t.clone(),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(valid, 1), -Fp::ONE)),
        vec![],
    ]);
    // Exact maximum (§5.4): h = 1 exactly when T > f. With f, T, d0, d1 u64
    // the differences cannot wrap, so the wrong h has no witness.
    let bf = |i| source(before.local, before.core[i]);
    let af = |i| source(after.local, after.core[i]);
    let sf = |i| source(statement.local, statement.fields[i]);
    let f = bf(28);
    let time = source(opening.local, opening.time);
    let (fv, timev) = (integer(f.value(w)), integer(time.value(w)));
    let newer = timev > fv;
    let h = c.bit(newer);
    let (d0v, d1v) = if newer {
        (BigUint::ZERO, &timev - &fv - 1u32)
    } else {
        // An off-range f (a malformed opening) leaves d0 unsatisfiable.
        ((&fv + pow2(256) - &timev) % pow2(256), BigUint::ZERO)
    };
    let u64_cell = |c: &mut Cells, v: &BigUint| {
        let bytes = v.to_bytes_le();
        let indices: Vec<_> = (0..8)
            .map(|i| c.byte(bytes.get(i).copied().unwrap_or(0)))
            .collect();
        let k = c.cell(fp(v), 64);
        let e = c.bytes(&indices);
        c.eq(own(k, 1), e);
        k
    };
    let d0 = u64_cell(&mut c, &d0v);
    let d1 = u64_cell(&mut c, &d1v);
    let older = plus(&constant(vec![Fp::ONE]), &scaled(&own(h, 1), -Fp::ONE));
    let difference = plus(&time.linear(), &scaled(&f.linear(), -Fp::ONE));
    c.a.rows.push([
        older.clone(),
        plus(&scaled(&difference, -Fp::ONE), &scaled(&own(d0, 1), -Fp::ONE)),
        vec![],
    ]);
    c.a.rows.push([
        own(h, 1),
        plus(
            &plus(&difference, &constant(vec![-Fp::ONE])),
            &scaled(&own(d1, 1), -Fp::ONE),
        ),
        vec![],
    ]);
    c.a.rows.push([own(h, 1), own(d0, 1), vec![]]);
    c.a.rows.push([older, own(d1, 1), vec![]]);
    let floor_value = if newer { time.value(w) } else { f.value(w) };
    let floor = c.cell(floor_value, 255);
    c.a.rows.push([
        own(h, 1),
        difference,
        plus(&own(floor, 1), &scaled(&f.linear(), -Fp::ONE)),
    ]);
    // Refresh-only joins, each `t·(a - b) = 0`.
    let d = source(opening.local, opening.digest);
    let held = source(before.local, before.rest[5]);
    let successor_anchor = source(after.local, after.rest[5]);
    for (a, b) in [
        (af(28).linear(), own(floor, 1)),
        (successor_anchor.linear(), d.linear()),
        (sf(18).linear(), d.linear()),
        (sf(19).linear(), af(28).linear()),
        (af(9).linear(), bf(9).linear()),
        (af(13).linear(), bf(13).linear()),
        (af(19).linear(), bf(19).linear()),
    ] {
        c.a.rows
            .push([t.clone(), plus(&a, &scaled(&b, -Fp::ONE)), vec![]]);
    }
    // d ≠ 0 and d ≠ R[5]: `x·inverse = t`.
    let dv = d.value(w);
    for (expression, value) in [
        (d.linear(), dv),
        (
            plus(&d.linear(), &scaled(&held.linear(), -Fp::ONE)),
            dv - held.value(w),
        ),
    ] {
        let inverse = c.cell(
            Option::<Fp>::from((tv * value).invert()).unwrap_or(Fp::ZERO),
            255,
        );
        c.a.rows.push([expression, own(inverse, 1), t.clone()]);
    }
    let verdict = w.push(c.a);
    RefreshBranch {
        valid: source(verdict, valid),
        digest: d,
        time,
        floor: source(verdict, floor),
    }
}

struct Opening {
    local: usize,
    message: usize,
    digest: usize,
    time: usize,
    valid: usize,
}

/// Exact anchor bytes, `m`, `d` and `T`, with a total encoding verdict:
/// version 1, the wallet's scheme and wallet, a nonzero nonce, and a
/// canonical nonzero signer field equal to the opened certificate's digest.
fn append_opening(
    w: &mut Wiring<Fp>,
    family: usize,
    anchor: &TimeAnchor,
    certificate_digest: Source,
    credential: &CurrentCredential,
) -> Opening {
    let mut c = Cells::new(family);
    c.a.name = "total time-anchor opening";
    for byte in anchor.body.iter().chain(&anchor.signature) {
        c.byte(*byte);
    }
    let value = |c: &Cells, start: usize, len: usize| {
        let indices: Vec<_> = (start..start + len).collect();
        (
            c.bytes(&indices),
            fp(&BigUint::from_bytes_le(&anchor.body[start..start + len])),
        )
    };
    let mut checks = vec![];
    let (version, v) = value(&c, 0, 2);
    checks.push(zero_predicate(
        &mut c,
        plus(&version, &constant(vec![-Fp::ONE])),
        v - Fp::ONE,
    ));
    for (start, expected) in [
        (2, credential.scheme()[0]),
        (18, credential.scheme()[1]),
        (34, credential.wallet()[0]),
        (50, credential.wallet()[1]),
    ] {
        let (packed, v) = value(&c, start, 16);
        checks.push(zero_predicate(
            &mut c,
            plus(&packed, &scaled(&expected.linear(), -Fp::ONE)),
            v - expected.value(w),
        ));
    }
    let nonce = (66..98).fold(vec![], |a, k| plus(&a, &own(k, 1)));
    let nonce_value = Fp::from(anchor.body[66..98].iter().map(|b| *b as u64).sum::<u64>());
    let zero_nonce = zero_predicate(&mut c, nonce, nonce_value);
    checks.push(negate(&mut c, zero_nonce));
    let (packed_time, time_value) = value(&c, 98, 8);
    let time = c.cell(time_value, 64);
    c.eq(own(time, 1), packed_time);
    let signer: Vec<_> = (106..138).collect();
    let signer_integer = BigUint::from_bytes_le(&anchor.body[106..138]);
    let canonical = c.at_most_bit(&signer, &signer_integer, &(field_modulus() - 1u32));
    checks.push(canonical);
    let signer_value = if signer_integer < field_modulus() {
        fp(&signer_integer)
    } else {
        Fp::ZERO
    };
    let signer_cell = c.cell(signer_value, 255);
    let packed_signer = c.bytes(&signer);
    c.a.rows
        .push([own(canonical, 1), packed_signer, own(signer_cell, 1)]);
    let zero_signer = zero_predicate(&mut c, own(signer_cell, 1), signer_value);
    checks.push(negate(&mut c, zero_signer));
    checks.push(zero_predicate(
        &mut c,
        plus(
            &own(signer_cell, 1),
            &scaled(&certificate_digest.linear(), -Fp::ONE),
        ),
        signer_value - certificate_digest.value(w),
    ));
    let valid = all(&mut c, &checks);
    let m = message(anchor);
    let ms = [c.cell(Fp::ZERO, 255), c.cell(m, 255), c.cell(Fp::ZERO, 255)];
    let d = digest(anchor);
    let ds = [c.cell(Fp::ZERO, 255), c.cell(d, 255), c.cell(Fp::ZERO, 255)];
    let body_items: Vec<_> = std::iter::once((
        Fp::from(BODY_BYTES as u64),
        constant(vec![Fp::from(BODY_BYTES as u64)]),
    ))
    .chain(anchor.body.chunks(31).enumerate().map(|(i, b)| {
        (
            fp(&BigUint::from_bytes_le(b)),
            c.bytes(&(31 * i..31 * i + b.len()).collect::<Vec<_>>()),
        )
    }))
    .collect();
    let rle: Vec<_> = (SIGNATURE..SIGNATURE + 32).rev().collect();
    let sle: Vec<_> = (SIGNATURE + 32..SIGNATURE + 64).rev().collect();
    let [r_lo, r_hi] = halves(&anchor.signature[..32]);
    let [s_lo, s_hi] = halves(&anchor.signature[32..]);
    let object_items = vec![
        (m, own(ms[1], 1)),
        (r_lo, c.bytes(&rle[..16])),
        (r_hi, c.bytes(&rle[16..])),
        (s_lo, c.bytes(&sle[..16])),
        (s_hi, c.bytes(&sle[16..])),
    ];
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"exact time-anchor hashes");
    certificate::hash_relation(w, family, local, SIGN_DOMAIN, &body_items, ms, &mut rng);
    certificate::hash_relation(w, family, local, OBJECT_DOMAIN, &object_items, ds, &mut rng);
    Opening {
        local,
        message: ms[1],
        digest: ds[1],
        time,
        valid,
    }
}
