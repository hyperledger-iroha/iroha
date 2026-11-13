//! Exact own-operation receipt, signed under the verified current payment key.
//!
//! The certified wallet-table commitment is a mandatory public input and the
//! comb table is its omitted commitment row. Incoming malformed receipts use
//! the exact digest committed by their independently verified Send relation;
//! this hard own-receipt constructor does not model an invalid-input branch.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

pub use crate::bootstrap::Receipt;
use crate::{
    certificate::{self, Cells, field_modulus, fp, pow2},
    fs::SpikeRng,
    full::{self, Wiring},
    nonnative::{self, KeyTable},
    operation_auth::CurrentCredential,
    p256,
    send_arithmetic::Source,
    statement::{Kind, StatementCells},
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};

pub const BODY_BYTES: usize = 338;
const SIGNATURE: usize = BODY_BYTES;
const MESSAGE: usize = SIGNATURE + 64;

/// Receive binds the digest of the complete canonical incoming Payment.
/// Every other own receipt must carry the canonical zero field.
#[derive(Clone, Copy)]
pub enum PaymentDigest {
    Absent,
    Present(Source),
}

/// An exact hard-valid receipt authenticated under verified credential/table
/// continuity. Private fields prevent treating an arbitrary signature as one.
pub struct AuthenticatedReceipt {
    local: usize,
    digest: Source,
    proof_digest: Source,
    capsule: [Source; 2],
    comb: usize,
    wallet_row: usize,
}
impl AuthenticatedReceipt {
    pub fn digest(&self) -> Source {
        self.digest
    }
    pub fn proof_digest(&self) -> Source {
        self.proof_digest
    }
    pub fn capsule(&self) -> [Source; 2] {
        self.capsule
    }
    pub fn comb(&self) -> usize {
        self.comb
    }
    pub fn wallet_row(&self) -> usize {
        self.wallet_row
    }
    pub fn byte(&self, index: usize) -> Source {
        assert!(index < BODY_BYTES + 64);
        Source {
            array: self.local,
            cell: index,
            instance: 0,
        }
    }
}
pub fn message(receipt: &Receipt) -> Fp {
    crate::bootstrap::receipt_message(receipt)
}
pub fn digest(receipt: &Receipt) -> Fp {
    crate::bootstrap::receipt_digest(receipt)
}
fn word(bytes: &[u8]) -> Fp {
    fp(&BigUint::from_bytes_le(bytes))
}
fn packed(c: &Cells, start: usize, len: usize) -> Lin<Fp> {
    c.bytes(&(start..start + len).collect::<Vec<_>>())
}
fn nonzero(c: &mut Cells, v: Fp, expression: Lin<Fp>) {
    let inv = c.cell(Option::<Fp>::from(v.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows
        .push([expression, own(inv, 1), constant(vec![Fp::ONE])]);
}
fn hash_cells(c: &mut Cells, v: Fp) -> [usize; 3] {
    [c.cell(Fp::ZERO, 255), c.cell(v, 255), c.cell(Fp::ZERO, 255)]
}

struct BodyFields {
    op_items: Vec<(Fp, Lin<Fp>)>,
    op_cells: [usize; 3],
    proof_cell: usize,
    capsule_indices: [usize; 2],
}
fn append_body_fields(
    w: &Wiring<Fp>,
    family: usize,
    receipt: &Receipt,
    credential: &CurrentCredential,
    statement: &StatementCells,
    kind: Kind,
    payment: PaymentDigest,
    c: &mut Cells,
) -> BodyFields {
    c.eq(own(0, 1), constant(vec![Fp::ONE]));
    c.zero(own(1, 1));
    let sf = |i| Source {
        array: statement.local,
        cell: statement.fields[i],
        instance: 0,
    };
    let sd = Source {
        array: statement.local,
        cell: statement.digest,
        instance: 0,
    };
    // Scheme comes from both the exact statement and verified credential.
    for (offset, sources) in [
        (2, credential.scheme()),
        (34, credential.wallet()),
        (66, credential.provider()),
    ] {
        for j in 0..2 {
            c.eq(packed(&c, offset + 16 * j, 16), sources[j].linear());
        }
    }
    for (a, b) in credential.scheme().into_iter().zip([sf(3), sf(4)]) {
        c.eq(a.linear(), b.linear());
    }
    c.eq(packed(&c, 98, 16), sf(9).linear());
    c.eq(sf(16).linear(), constant(vec![Fp::from(kind.tag())]));
    let mut op_sources = credential.wallet().to_vec();
    op_sources.push(sf(16));
    match kind {
        Kind::Bootstrap => op_sources.extend([sf(17), sf(18)]),
        Kind::Archive
        | Kind::RefreshCredential
        | Kind::RefreshSchemePolicy
        | Kind::RefreshQuotaShare
        | Kind::RefreshTimeAnchor => op_sources.push(sf(18)),
        Kind::Retiring => {}
        _ => op_sources.push(sf(17)),
    }
    let mut op_items: Vec<_> = op_sources
        .iter()
        .map(|s| (s.value(w), s.linear()))
        .collect();
    if kind == Kind::Retiring {
        op_items.push((Fp::ZERO, vec![]));
    }
    let op = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &op_items.iter().map(|x| x.0).collect::<Vec<_>>(),
    );
    let op_cells = hash_cells(c, op);
    nonzero(c, op, own(op_cells[1], 1));
    for (offset, source) in [(146, sf(14)), (178, sf(15)), (210, sd)] {
        let indices: Vec<_> = (offset..offset + 32).collect();
        let integer = BigUint::from_bytes_le(&receipt.body[offset..offset + 32]);
        c.at_most(&indices, &integer, &(field_modulus() - 1u32));
        c.eq(c.bytes(&indices), source.linear());
    }
    let op_indices: Vec<_> = (114..146).collect();
    c.at_most(
        &op_indices,
        &BigUint::from_bytes_le(&receipt.body[114..146]),
        &(field_modulus() - 1u32),
    );
    c.eq(c.bytes(&op_indices), own(op_cells[1], 1));
    let proof_indices: Vec<_> = (242..274).collect();
    let proof = BigUint::from_bytes_le(&receipt.body[242..274]);
    c.at_most(&proof_indices, &proof, &(field_modulus() - 1u32));
    let proof_cell = c.cell(fp(&proof), 255);
    c.eq(own(proof_cell, 1), c.bytes(&proof_indices));
    nonzero(c, fp(&proof), own(proof_cell, 1));
    let capsule_indices = std::array::from_fn::<_, 2, _>(|j| {
        let k = c.cell(word(&receipt.body[274 + 16 * j..290 + 16 * j]), 128);
        c.eq(own(k, 1), packed(&c, 274 + 16 * j, 16));
        k
    });
    let capsule_sum = (274..306).fold(vec![], |a, k| plus(&a, &own(k, 1)));
    let capsule_value = Fp::from(
        receipt.body[274..306]
            .iter()
            .map(|b| *b as u64)
            .sum::<u64>(),
    );
    nonzero(c, capsule_value, capsule_sum);
    let payment_indices: Vec<_> = (306..338).collect();
    c.at_most(
        &payment_indices,
        &BigUint::from_bytes_le(&receipt.body[306..338]),
        &(field_modulus() - 1u32),
    );
    match payment {
        PaymentDigest::Absent => c.zero(c.bytes(&payment_indices)),
        PaymentDigest::Present(source) => {
            assert_eq!(w.arrays[source.array].family, family);
            c.eq(c.bytes(&payment_indices), source.linear());
            nonzero(c, source.value(w), source.linear());
        }
    }
    BodyFields {
        op_items,
        op_cells,
        proof_cell,
        capsule_indices,
    }
}

/// Exact unsigned intent, validated against current credential and statement.
/// This is not an authenticated receipt or a proof-admission capability.
pub struct PreparedBody {
    body: [u8; BODY_BYTES],
    local: usize,
    message: Source,
    operation: Source,
    proof: Source,
    capsule: [Source; 2],
}
impl PreparedBody {
    pub fn body(&self) -> &[u8; BODY_BYTES] {
        &self.body
    }
    pub fn message(&self) -> Source {
        self.message
    }
    pub fn operation_id(&self) -> Source {
        self.operation
    }
    pub fn proof_digest(&self) -> Source {
        self.proof
    }
    pub fn capsule(&self) -> [Source; 2] {
        self.capsule
    }
    pub fn byte(&self, i: usize) -> Source {
        assert!(i < BODY_BYTES);
        Source {
            array: self.local,
            cell: i,
            instance: 0,
        }
    }
}

/// Install the one certified current wallet table without requiring a new
/// hardware signature. Its row commitment is checked against the authenticated
/// predecessor's credential C by every complete proof using this wiring.
pub fn install_wallet_table(
    w: &mut Wiring<Fp>,
    family: usize,
    credential: &CurrentCredential,
) -> usize {
    if let Some((row, offset)) = w.wallet {
        assert_eq!(offset, full::TABLE_IO);
        let io = w
            .io_arrays
            .iter()
            .find(|(_, at)| *at == offset)
            .expect("installed wallet public C")
            .0;
        w.push(Array {
            name: "reused wallet credential binding",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![],
            values: vec![],
            rows: credential
                .table_commitment()
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    [
                        plus(&s.linear(), &link(io, 0, vec![Some(i)], -Fp::ONE)),
                        constant(vec![Fp::ONE]),
                        vec![],
                    ]
                })
                .collect(),
        });
        return row;
    }
    crate::operation_auth::ensure_range(w, family);
    let q = credential.payment_point(w);
    let row = w.push(Array {
        name: "prepared certified wallet table",
        family,
        rho: 9,
        r_count: 512,
        fixed: false,
        lookups: vec![],
        widths: vec![9, 88, 88, 80, 88, 88, 80],
        rows: vec![],
        values: nonnative::wallet_table_values(&q),
    });
    w.tables.push((row, nonnative::WALLET_TABLE));
    w.wallet = Some((row, full::TABLE_IO));
    if !w.tables.iter().any(|(_, id)| *id == nonnative::POINT_TABLE) {
        let generator = w.push(Array {
            name: "prepared fixed generator table",
            family,
            rho: 9,
            r_count: 512,
            fixed: true,
            lookups: vec![],
            widths: vec![9, 88, 88, 80, 88, 88, 80],
            rows: vec![],
            values: nonnative::wallet_table_values(&p256::generator()),
        });
        w.tables.push((generator, nonnative::POINT_TABLE));
    }
    let sources = credential.table_commitment();
    let io = w.push(Array {
        name: "prepared certified table commitment",
        family,
        rho: 2,
        r_count: 4,
        fixed: false,
        lookups: vec![],
        widths: vec![128],
        rows: vec![],
        values: vec![sources.iter().map(|s| s.value(w)).collect()],
    });
    w.io_arrays.push((io, full::TABLE_IO));
    w.push(Array {
        name: "prepared table authority binding",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: sources
            .iter()
            .enumerate()
            .map(|(i, s)| {
                [
                    plus(&s.linear(), &link(io, 0, vec![Some(i)], -Fp::ONE)),
                    constant(vec![Fp::ONE]),
                    vec![],
                ]
            })
            .collect(),
    });
    row
}

pub fn append_preparation(
    w: &mut Wiring<Fp>,
    family: usize,
    body: &[u8; BODY_BYTES],
    credential: &CurrentCredential,
    statement: &StatementCells,
    kind: Kind,
    payment: PaymentDigest,
) -> Result<PreparedBody, &'static str> {
    if (kind == Kind::Receive) != matches!(payment, PaymentDigest::Present(_)) {
        return Err("Receive alone requires full Payment digest");
    }
    install_wallet_table(w, family, credential);
    let receipt = Receipt {
        body: *body,
        signature: [0; 64],
    };
    let m = message(&receipt);
    let mb = m.to_repr();
    let mut c = Cells::new(family);
    c.a.name = "exact unsigned prepared receipt";
    for byte in body.iter().chain(mb.as_ref()) {
        c.byte(*byte);
    }
    let BodyFields {
        op_items,
        op_cells,
        proof_cell,
        capsule_indices,
    } = append_body_fields(
        w, family, &receipt, credential, statement, kind, payment, &mut c,
    );
    let indices: Vec<_> = (BODY_BYTES..BODY_BYTES + 32).collect();
    c.at_most(
        &indices,
        &BigUint::from_bytes_le(mb.as_ref()),
        &(field_modulus() - 1u32),
    );
    let message_cells = hash_cells(&mut c, m);
    c.eq(own(message_cells[1], 1), c.bytes(&indices));
    let body_items: Vec<_> = std::iter::once((
        Fp::from(BODY_BYTES as u64),
        constant(vec![Fp::from(BODY_BYTES as u64)]),
    ))
    .chain(
        body.chunks(31)
            .enumerate()
            .map(|(i, b)| (word(b), packed(&c, 31 * i, b.len()))),
    )
    .collect();
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"prepared own receipt hashes");
    for (domain, items, state) in [
        (b"kgwopid1", op_items, op_cells),
        (b"kgwrcpt1", body_items, message_cells),
    ] {
        certificate::hash_relation(
            w,
            family,
            local,
            u64::from_le_bytes(*domain),
            &items,
            state,
            &mut rng,
        );
    }
    let source = |cell| Source {
        array: local,
        cell,
        instance: 0,
    };
    Ok(PreparedBody {
        body: *body,
        local,
        message: source(message_cells[1]),
        operation: source(op_cells[1]),
        proof: source(proof_cell),
        capsule: capsule_indices.map(source),
    })
}

/// Constrain every receipt byte, native operation-id derivation and exact
/// statement/head bindings; verify low-S P-256 through the certified wallet row.
/// The signed proof digest is the nonzero lean own-receipt field, not a circular
/// hash of the recursive proof being generated.
pub fn append_own(
    w: &mut Wiring<Fp>,
    family: usize,
    receipt: &Receipt,
    credential: &CurrentCredential,
    statement: &StatementCells,
    kind: Kind,
    payment: PaymentDigest,
) -> Result<AuthenticatedReceipt, &'static str> {
    if (kind == Kind::Receive) != matches!(payment, PaymentDigest::Present(_)) {
        return Err("Receive alone requires the full Payment digest");
    }
    let installed_wallet = w
        .wallet
        .map(|_| install_wallet_table(w, family, credential));
    assert_eq!(w.arrays[statement.local].family, family);
    let q = credential.payment_point(w);
    if q.x >= p256::p() || q.y >= p256::p() || !p256::on_curve(&q) {
        return Err("invalid verified payment point witness");
    }
    let m = message(receipt);
    let mb = m.to_repr();
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(mb.as_ref()));
    let r = BigUint::from_bytes_be(&receipt.signature[..32]);
    let s = BigUint::from_bytes_be(&receipt.signature[32..]);
    if !p256::verify(
        &q,
        &z,
        &p256::Signature {
            r: r.clone(),
            s: s.clone(),
        },
    ) {
        return Err("own receipt signature has no ECDSA witness");
    }
    crate::operation_auth::ensure_range(w, family);
    let mut c = Cells::new(family);
    c.a.name = "exact authenticated own receipt";
    for b in receipt
        .body
        .iter()
        .chain(&receipt.signature)
        .chain(mb.as_ref())
    {
        c.byte(*b);
    }
    let BodyFields {
        op_items,
        op_cells,
        proof_cell,
        capsule_indices,
    } = append_body_fields(
        w, family, receipt, credential, statement, kind, payment, &mut c,
    );
    let rle: Vec<_> = (SIGNATURE..SIGNATURE + 32).rev().collect();
    let sle: Vec<_> = (SIGNATURE + 32..SIGNATURE + 64).rev().collect();
    c.at_most(&rle, &r, &(p256::n() - 1u32));
    c.at_most(&sle, &s, &(p256::n() >> 1));
    for indices in [&rle, &sle] {
        let expression = indices.iter().fold(vec![], |a, k| plus(&a, &own(*k, 1)));
        let value = indices.iter().map(|k| c.a.values[*k][0]).sum::<Fp>();
        nonzero(&mut c, value, expression);
    }
    let limbs = [c.value(&r, &rle), c.value(&s, &sle)];
    let mle: Vec<_> = (MESSAGE..MESSAGE + 32).collect();
    c.at_most(
        &mle,
        &BigUint::from_bytes_le(mb.as_ref()),
        &(field_modulus() - 1u32),
    );
    let message_cells = hash_cells(&mut c, m);
    c.eq(own(message_cells[1], 1), c.bytes(&mle));
    let object_cells = hash_cells(&mut c, digest(receipt));
    let body_items: Vec<_> = std::iter::once((Fp::from(338), constant(vec![Fp::from(338)])))
        .chain(
            receipt
                .body
                .chunks(31)
                .enumerate()
                .map(|(i, b)| (word(b), packed(&c, 31 * i, b.len()))),
        )
        .collect();
    let object_items = vec![
        (m, own(message_cells[1], 1)),
        (fp(&(&r % pow2(128))), c.bytes(&rle[..16])),
        (fp(&(&r >> 128)), c.bytes(&rle[16..])),
        (fp(&(&s % pow2(128))), c.bytes(&sle[..16])),
        (fp(&(&s >> 128)), c.bytes(&sle[16..])),
    ];
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"exact own receipt hashes");
    for (domain, items, state) in [
        (b"kgwopid1", op_items, op_cells),
        (b"kgwrcpt1", body_items, message_cells),
        (b"kgworcp1", object_items, object_cells),
    ] {
        certificate::hash_relation(
            w,
            family,
            local,
            u64::from_le_bytes(*domain),
            &items,
            state,
            &mut rng,
        );
    }
    let (_, comb) = certificate::add_signature(
        w,
        family,
        local,
        MESSAGE,
        m,
        (q, z, r, s),
        limbs,
        KeyTable::Witness {
            id: nonnative::WALLET_TABLE,
        },
    );
    let wallet_row = if let Some(row) = installed_wallet {
        let duplicate = comb + nonnative::A_WTABLE;
        assert_eq!(w.arrays.len(), duplicate + 1, "comb wallet table is last");
        w.tables.retain(|(array, _)| *array != duplicate);
        w.arrays.pop();
        row
    } else {
        // Mandatory external authentication of the omitted variable payment table.
        // The complete proof verifies its commitment against this exact signed C.
        w.wallet = Some((comb + nonnative::A_WTABLE, full::TABLE_IO));
        let table = credential.table_commitment();
        let io = w.push(Array {
            name: "receipt certified wallet commitment",
            family,
            rho: 2,
            r_count: 4,
            fixed: false,
            lookups: vec![],
            widths: vec![128],
            rows: vec![],
            values: vec![table.iter().map(|s| s.value(w)).collect()],
        });
        w.io_arrays.push((io, full::TABLE_IO));
        for (i, source) in table.into_iter().enumerate() {
            w.arrays[local].rows.push([
                plus(&source.linear(), &link(io, 0, vec![Some(i)], -Fp::ONE)),
                constant(vec![Fp::ONE]),
                vec![],
            ]);
        }
        comb + nonnative::A_WTABLE
    };
    let mut seen = std::collections::HashSet::new();
    w.tables.retain(|(_, id)| seen.insert(*id));
    let source = |cell| Source {
        array: local,
        cell,
        instance: 0,
    };
    Ok(AuthenticatedReceipt {
        local,
        digest: source(object_cells[1]),
        proof_digest: source(proof_cell),
        capsule: capsule_indices.map(source),
        comb,
        wallet_row,
    })
}

/// Finish a prepared unsigned body using the actual hardware signature. Every
/// original body byte is joined; signing another capsule or intent cannot reuse
/// the prepared relation. Only this returns the authenticated receipt capability.
pub fn authenticate_prepared(
    w: &mut Wiring<Fp>,
    family: usize,
    prepared: &PreparedBody,
    receipt: &Receipt,
    credential: &CurrentCredential,
    statement: &StatementCells,
    kind: Kind,
    payment: PaymentDigest,
) -> Result<AuthenticatedReceipt, &'static str> {
    let authenticated = append_own(w, family, receipt, credential, statement, kind, payment)?;
    w.push(Array {
        name: "prepared receipt exact body finalization",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: (0..BODY_BYTES)
            .map(|i| {
                [
                    plus(
                        &prepared.byte(i).linear(),
                        &scaled(&authenticated.byte(i).linear(), -Fp::ONE),
                    ),
                    constant(vec![Fp::ONE]),
                    vec![],
                ]
            })
            .collect(),
    });
    Ok(authenticated)
}

/// Exact original receipt opening for an incoming predicate. This object is not
/// authenticated until its digest is joined to a verified operation's receipt
/// digest (or a separately constrained signature verdict).
pub struct ReceiptOpening {
    local: usize,
    digest: Source,
    message: Source,
    valid: Source,
    scheme: [Source; 2],
    wallet: [Source; 2],
    provider: [Source; 2],
    sequence: Source,
    operation: Source,
    predecessor: Source,
    successor: Source,
    statement: Source,
    proof: Source,
    capsule: [Source; 2],
    payment: Source,
}
impl ReceiptOpening {
    pub fn digest(&self) -> Source {
        self.digest
    }
    pub fn message(&self) -> Source {
        self.message
    }
    pub fn valid(&self) -> Source {
        self.valid
    }
    pub fn scheme(&self) -> [Source; 2] {
        self.scheme
    }
    pub fn wallet(&self) -> [Source; 2] {
        self.wallet
    }
    pub fn provider(&self) -> [Source; 2] {
        self.provider
    }
    pub fn sequence(&self) -> Source {
        self.sequence
    }
    pub fn operation_id(&self) -> Source {
        self.operation
    }
    pub fn predecessor(&self) -> Source {
        self.predecessor
    }
    pub fn successor(&self) -> Source {
        self.successor
    }
    pub fn statement_digest(&self) -> Source {
        self.statement
    }
    pub fn proof_digest(&self) -> Source {
        self.proof
    }
    pub fn capsule(&self) -> [Source; 2] {
        self.capsule
    }
    pub fn payment_digest(&self) -> Source {
        self.payment
    }
    pub fn byte(&self, index: usize) -> Source {
        assert!(index < BODY_BYTES + 64);
        Source {
            array: self.local,
            cell: index,
            instance: 0,
        }
    }
}

/// Arbitrary fixed-length original receipt -> exact raw byte/object hashes,
/// canonical field projections (zero for a noncanonical field), and a total
/// structural/self-contained verdict. False never disables helper constraints.
pub fn append_opening_total(
    w: &mut Wiring<Fp>,
    family: usize,
    receipt: &Receipt,
) -> ReceiptOpening {
    use crate::credential_policy::{all, negate, zero_predicate};
    crate::operation_auth::ensure_range(w, family);
    let mut c = Cells::new(family);
    c.a.name = "total incoming receipt original";
    for b in receipt.body.iter().chain(&receipt.signature) {
        c.byte(*b);
    }
    let mut checks = vec![
        zero_predicate(
            &mut c,
            plus(&own(0, 1), &constant(vec![-Fp::ONE])),
            Fp::from(receipt.body[0] as u64) - Fp::ONE,
        ),
        zero_predicate(&mut c, own(1, 1), Fp::from(receipt.body[1] as u64)),
    ];
    let mut pack = |start: usize, len: usize| {
        let k = c.cell(word(&receipt.body[start..start + len]), 255);
        c.eq(own(k, 1), packed(&c, start, len));
        k
    };
    let scheme = [pack(2, 16), pack(18, 16)];
    let wallet = [pack(34, 16), pack(50, 16)];
    let provider = [pack(66, 16), pack(82, 16)];
    let sequence = pack(98, 16);
    let capsule = [pack(274, 16), pack(290, 16)];
    let mut canonical = |offset: usize| {
        let indices: Vec<_> = (offset..offset + 32).collect();
        let integer = BigUint::from_bytes_le(&receipt.body[offset..offset + 32]);
        let good = c.at_most_bit(&indices, &integer, &(field_modulus() - 1u32));
        checks.push(good);
        let value = if integer < field_modulus() {
            fp(&integer)
        } else {
            Fp::ZERO
        };
        let k = c.cell(value, 255);
        c.a.rows.push([own(good, 1), c.bytes(&indices), own(k, 1)]);
        k
    };
    let operation = canonical(114);
    let predecessor = canonical(146);
    let successor = canonical(178);
    let statement = canonical(210);
    let proof = canonical(242);
    let payment = canonical(306);
    for cell in [operation, proof] {
        let value = c.a.values[cell][0];
        let zero = zero_predicate(&mut c, own(cell, 1), value);
        checks.push(negate(&mut c, zero));
    }
    let capsule_sum = (274..306).fold(vec![], |a, k| plus(&a, &own(k, 1)));
    let v = Fp::from(
        receipt.body[274..306]
            .iter()
            .map(|b| *b as u64)
            .sum::<u64>(),
    );
    let zero = zero_predicate(&mut c, capsule_sum, v);
    checks.push(negate(&mut c, zero));
    let r = BigUint::from_bytes_be(&receipt.signature[..32]);
    let s = BigUint::from_bytes_be(&receipt.signature[32..]);
    let rle: Vec<_> = (SIGNATURE..SIGNATURE + 32).rev().collect();
    let sle: Vec<_> = (SIGNATURE + 32..SIGNATURE + 64).rev().collect();
    checks.push(c.at_most_bit(&rle, &r, &(p256::n() - 1u32)));
    checks.push(c.at_most_bit(&sle, &s, &(p256::n() >> 1)));
    for indices in [&rle, &sle] {
        let expression = indices.iter().fold(vec![], |a, k| plus(&a, &own(*k, 1)));
        let value = indices.iter().map(|k| c.a.values[*k][0]).sum::<Fp>();
        let zero = zero_predicate(&mut c, expression, value);
        checks.push(negate(&mut c, zero));
    }
    let valid = all(&mut c, &checks);
    let m = message(receipt);
    let ms = hash_cells(&mut c, m);
    let ds = hash_cells(&mut c, digest(receipt));
    let body_items: Vec<_> = std::iter::once((Fp::from(338), constant(vec![Fp::from(338)])))
        .chain(
            receipt
                .body
                .chunks(31)
                .enumerate()
                .map(|(i, b)| (word(b), packed(&c, 31 * i, b.len()))),
        )
        .collect();
    let object_items = vec![
        (m, own(ms[1], 1)),
        (fp(&(&r % pow2(128))), c.bytes(&rle[..16])),
        (fp(&(&r >> 128)), c.bytes(&rle[16..])),
        (fp(&(&s % pow2(128))), c.bytes(&sle[..16])),
        (fp(&(&s >> 128)), c.bytes(&sle[16..])),
    ];
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"total original receipt hashes");
    certificate::hash_relation(
        w,
        family,
        local,
        u64::from_le_bytes(*b"kgwrcpt1"),
        &body_items,
        ms,
        &mut rng,
    );
    certificate::hash_relation(
        w,
        family,
        local,
        u64::from_le_bytes(*b"kgworcp1"),
        &object_items,
        ds,
        &mut rng,
    );
    let source = |cell| Source {
        array: local,
        cell,
        instance: 0,
    };
    ReceiptOpening {
        local,
        digest: source(ds[1]),
        message: source(ms[1]),
        valid: source(valid),
        scheme: scheme.map(source),
        wallet: wallet.map(source),
        provider: provider.map(source),
        sequence: source(sequence),
        operation: source(operation),
        predecessor: source(predecessor),
        successor: source(successor),
        statement: source(statement),
        proof: source(proof),
        capsule: capsule.map(source),
        payment: source(payment),
    }
}

#[cfg(test)]
#[path = "receipt_tests.rs"]
pub(crate) mod tests;

pub use crate::signature::{SignatureValidity, append_request_signature_total};
