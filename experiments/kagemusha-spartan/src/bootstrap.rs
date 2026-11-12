//! Exact future-generation Bootstrap: authenticated 532-byte credential,
//! initial 30-field Core, 26-field statement, and the payment-key receipt.
//! Native custody/admission and the other monetary transitions remain separate.
use crate::{
    certificate::{self, Cells, Certificate, RootContext, field_modulus, fp, pow2, scalar_bytes},
    credential::{self, Credential},
    fs::SpikeRng,
    full::{self, WalletKey, Wiring},
    nonnative::{self, KeyTable},
    p256,
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};
use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

pub const CORE_FIELDS: usize = 30;
pub const RECEIPT_BYTES: usize = 338;
pub const LINEAGE_IO: usize = 32;
const SIGNATURE: usize = RECEIPT_BYTES;
const MESSAGE: usize = SIGNATURE + 64;
const CORE: usize = MESSAGE + 32;
const REST: usize = CORE + CORE_FIELDS;
const STATEMENT: usize = REST + 6;

pub struct Context {
    pub root: RootContext,
    pub relation: [u8; 32],
}
#[derive(Clone)]
pub struct Receipt {
    pub body: [u8; RECEIPT_BYTES],
    pub signature: [u8; 64],
}
#[derive(Clone)]
pub struct Inputs {
    pub certificate: Certificate,
    pub credential: Credential,
    pub core: [Fp; CORE_FIELDS],
    pub rest: [Fp; 6],
    pub statement: [Fp; 26],
    pub receipt: Receipt,
}
pub struct Prepared {
    pub wiring: Wiring<Fp>,
    pub gfam: usize,
    pub local: usize,
    pub credential_local: usize,
    pub state_cell: usize,
    pub statement_cell: usize,
    pub receipt_cell: usize,
    pub receipt_comb: usize,
}
fn hash(domain: &[u8; 8], values: &[Fp]) -> Fp {
    poseidon::hash_with_domain(u64::from_le_bytes(*domain), values)
}
fn byte_hash(domain: &[u8; 8], bytes: &[u8]) -> Fp {
    let values: Vec<_> = std::iter::once(Fp::from(bytes.len() as u64))
        .chain(bytes.chunks(31).map(|b| fp(&BigUint::from_bytes_le(b))))
        .collect();
    hash(domain, &values)
}
pub fn receipt_message(receipt: &Receipt) -> Fp {
    byte_hash(b"kgwrcpt1", &receipt.body)
}
pub fn receipt_digest(receipt: &Receipt) -> Fp {
    let r = BigUint::from_bytes_be(&receipt.signature[..32]);
    let s = BigUint::from_bytes_be(&receipt.signature[32..]);
    hash(
        b"kgworcp1",
        &[
            receipt_message(receipt),
            fp(&(&r % pow2(128))),
            fp(&(&r >> 128)),
            fp(&(&s % pow2(128))),
            fp(&(&s >> 128)),
        ],
    )
}
pub fn empty_indexed_root() -> Fp {
    let mut root = hash(b"kgwimlf1", &[Fp::ZERO; 3]);
    let mut empty = Fp::ZERO;
    for _ in 0..32 {
        root = hash(b"kgwimnd1", &[root, empty]);
        empty = hash(b"kgwimnd1", &[empty, empty]);
    }
    root
}
pub fn padding_root(leaf: &[u8; 8], node: &[u8; 8], fields: usize, depth: usize) -> Fp {
    let mut root = hash(leaf, &vec![Fp::ZERO; fields]);
    for _ in 0..depth {
        root = hash(node, &[root, root]);
    }
    root
}
fn word(bytes: &[u8]) -> Fp {
    fp(&BigUint::from_bytes_le(bytes))
}
fn id(bytes: &[u8]) -> [Fp; 2] {
    [word(&bytes[..16]), word(&bytes[16..32])]
}
fn nonzero(c: &mut Cells, value: Fp, expr: Lin<Fp>) {
    let inv = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([expr, own(inv, 1), constant(vec![Fp::ONE])]);
}
fn packed(c: &Cells, start: usize, len: usize) -> Lin<Fp> {
    c.bytes(&(start..start + len).collect::<Vec<_>>())
}
fn other_bytes(array: usize, start: usize, len: usize, reverse: bool) -> Lin<Fp> {
    (0..len).fold(vec![], |acc, i| {
        plus(
            &acc,
            &link(
                array,
                if reverse {
                    start + len - 1 - i
                } else {
                    start + i
                },
                vec![Some(0)],
                Fp::from(256).pow_vartime([i as u64]),
            ),
        )
    })
}
fn hash_cells(c: &mut Cells, value: Fp) -> [usize; 3] {
    [
        c.cell(Fp::ZERO, 255),
        c.cell(value, 255),
        c.cell(Fp::ZERO, 255),
    ]
}
fn public(
    w: &mut Wiring<Fp>,
    gfam: usize,
    local: usize,
    offset: usize,
    values: &[Fp],
    sources: &[Lin<Fp>],
) {
    let rho = values.len().next_power_of_two().trailing_zeros() as usize;
    let mut out = values.to_vec();
    out.resize(1 << rho, Fp::ZERO);
    let io = w.push(Array {
        name: "Bootstrap public fields",
        family: gfam,
        rho,
        r_count: values.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![out],
    });
    w.io_arrays.push((io, offset));
    for (i, source) in sources.iter().enumerate() {
        w.arrays[local].rows.push([
            plus(source, &link(io, 0, vec![Some(i)], -Fp::ONE)),
            constant(vec![Fp::ONE]),
            vec![],
        ]);
    }
}

/// Compose private authentication ports and bind the externally supplied scheme
/// and relation to compact public context. No fixed issuer or scheme VK exists.
pub fn prepare(context: &Context, input: &Inputs) -> Result<Prepared, &'static str> {
    if context.relation == [0; 32] {
        return Err("zero trusted relation");
    }
    let cred = &input.credential;
    let q = p256::Point {
        x: BigUint::from_bytes_be(&cred.body[131..163]),
        y: BigUint::from_bytes_be(&cred.body[163..195]),
    };
    if q.x >= p256::p() || q.y >= p256::p() || !p256::on_curve(&q) {
        return Err("invalid payment point");
    }
    let m = receipt_message(&input.receipt);
    let mb = m.to_repr();
    let z = BigUint::from_bytes_be(&p256::sha256_one_block(mb.as_ref()));
    let r = BigUint::from_bytes_be(&input.receipt.signature[..32]);
    let s = BigUint::from_bytes_be(&input.receipt.signature[32..]);
    if !p256::verify(
        &q,
        &z,
        &p256::Signature {
            r: r.clone(),
            s: s.clone(),
        },
    ) {
        return Err("receipt signature has no ECDSA witness");
    }
    let prepared = credential::prepare(&context.root, &input.certificate, cred)?;
    let (mut w, gfam, cl, cd) = (
        prepared.wiring,
        prepared.gfam,
        prepared.local,
        prepared.digest_cell,
    );
    let mut c = Cells::new(gfam);
    for b in input
        .receipt
        .body
        .iter()
        .chain(&input.receipt.signature)
        .chain(mb.as_ref())
    {
        c.byte(*b);
    }
    for v in input.core.iter().chain(&input.rest).chain(&input.statement) {
        c.cell(*v, 255);
    }
    assert_eq!(c.a.values.len(), STATEMENT + 26);
    let empty = empty_indexed_root();
    let prior = padding_root(b"kgwpcrd1", b"kgwpcrn1", 1, 16);
    let quota = padding_root(b"kgwquse1", b"kgwqusn1", 4, 6);
    let one = constant(vec![Fp::ONE]);
    let zero = vec![];
    let credbytes = |start, len| other_bytes(cl, start, len, false);
    let credword = |start, len| word(&cred.body[start..start + len]);
    let mut expected = vec![zero.clone(); 30];
    expected[0] = one.clone();
    for (core, body) in [(1, 2), (3, 34), (5, 66)] {
        for i in 0..2 {
            expected[core + i] = credbytes(body + 16 * i, 16);
        }
    }
    expected[7] = link(cl, cd, vec![Some(0)], Fp::ONE);
    expected[8] = constant(vec![prior]);
    for i in 17..21 {
        expected[i] = constant(vec![empty]);
    }
    expected[21] = constant(vec![quota]);
    expected[25] = credbytes(440, 8);
    expected[26] = credbytes(492, 8);
    expected[29] = own(CORE + 29, 1);
    for (i, e) in expected.into_iter().enumerate() {
        c.eq(own(CORE + i, 1), e);
    }
    nonzero(&mut c, input.core[7], own(CORE + 7, 1));
    nonzero(&mut c, input.core[29], own(CORE + 29, 1));
    c.eq(own(REST, 1), credbytes(436, 4));
    for i in 1..6 {
        c.zero(own(REST + i, 1));
    }
    let rest = hash(b"kgwrest1", &input.rest);
    let rest_state = hash_cells(&mut c, rest);
    let mut core_values = input.core.to_vec();
    core_values.push(rest);
    let state = hash(b"kgwcore1", &core_values);
    let state_cells = hash_cells(&mut c, state);
    nonzero(&mut c, state, own(state_cells[1], 1));
    let mut expected = vec![zero; 26];
    expected[0] = one;
    for i in 1..3 {
        expected[i] = own(STATEMENT + i, 1);
    }
    for (st, co) in [(3, 1), (4, 2), (5, 3), (6, 4), (7, 7), (8, 0)] {
        expected[st] = own(CORE + co, 1);
    }
    expected[15] = own(state_cells[1], 1);
    expected[16] = constant(vec![Fp::ONE]);
    expected[17] = credbytes(448, 16);
    expected[18] = credbytes(464, 16);
    expected[19] = own(STATEMENT + 19, 1);
    expected[20] = own(STATEMENT + 20, 1);
    for (i, e) in expected.into_iter().enumerate() {
        c.eq(own(STATEMENT + i, 1), e);
    }
    // Identifier limbs are unsigned128. The marker is opaque but nonzero.
    for i in [1, 2, 19, 20] {
        let b = input.statement[i].to_repr();
        let cells: Vec<_> = b.as_ref()[..16].iter().map(|b| c.byte(*b)).collect();
        c.eq(own(STATEMENT + i, 1), c.bytes(&cells));
    }
    for start in [1, 19] {
        nonzero(
            &mut c,
            input.statement[start] + input.statement[start + 1],
            plus(&own(STATEMENT + start, 1), &own(STATEMENT + start + 1, 1)),
        );
    }
    let sd = hash(b"kgwstmt1", &input.statement);
    let statement_cells = hash_cells(&mut c, sd);
    let op_values = [
        input.core[5],
        input.core[6],
        Fp::ONE,
        credword(448, 16),
        credword(464, 16),
    ];
    let op = hash(b"kgwopid1", &op_values);
    let op_cells = hash_cells(&mut c, op);
    nonzero(&mut c, op, own(op_cells[1], 1));
    // All receipt bytes are authenticated, and every derived field is joined.
    c.eq(own(0, 1), constant(vec![Fp::ONE]));
    c.zero(own(1, 1));
    for (receipt, body) in [(2, 2), (34, 66), (66, 259)] {
        for i in 0..32 {
            c.eq(
                own(receipt + i, 1),
                link(cl, body + i, vec![Some(0)], Fp::ONE),
            );
        }
    }
    c.eq(packed(&c, 98, 16), own(STATEMENT + 9, 1));
    for (start, cell) in [
        (114, op_cells[1]),
        (146, STATEMENT + 14),
        (178, STATEMENT + 15),
        (210, statement_cells[1]),
    ] {
        let integer = BigUint::from_bytes_le(&input.receipt.body[start..start + 32]);
        let indices: Vec<_> = (start..start + 32).collect();
        c.at_most(&indices, &integer, &(field_modulus() - 1u32));
        c.eq(c.bytes(&indices), own(cell, 1));
    }
    let proof_indices: Vec<_> = (242..274).collect();
    let proof = BigUint::from_bytes_le(&input.receipt.body[242..274]);
    c.at_most(&proof_indices, &proof, &(field_modulus() - 1u32));
    let proof_expr = c.bytes(&proof_indices);
    nonzero(&mut c, fp(&proof), proof_expr);
    // Capsule is an opaque32-byte identifier, not a reduced field element.
    let capsule_sum = (274..306).fold(vec![], |a, k| plus(&a, &own(k, 1)));
    nonzero(
        &mut c,
        Fp::from(
            input.receipt.body[274..306]
                .iter()
                .map(|b| *b as u64)
                .sum::<u64>(),
        ),
        capsule_sum,
    );
    for i in 306..338 {
        c.zero(own(i, 1));
    }
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
    let mstate = hash_cells(&mut c, m);
    c.eq(own(mstate[1], 1), c.bytes(&mle));
    let rd = receipt_digest(&input.receipt);
    let rdstate = hash_cells(&mut c, rd);
    let rest_items: Vec<_> = input
        .rest
        .iter()
        .enumerate()
        .map(|(i, v)| (*v, own(REST + i, 1)))
        .collect();
    let core_items: Vec<_> = input
        .core
        .iter()
        .enumerate()
        .map(|(i, v)| (*v, own(CORE + i, 1)))
        .chain(std::iter::once((rest, own(rest_state[1], 1))))
        .collect();
    let statement_items: Vec<_> = input
        .statement
        .iter()
        .enumerate()
        .map(|(i, v)| (*v, own(STATEMENT + i, 1)))
        .collect();
    let op_items = vec![
        (op_values[0], own(CORE + 5, 1)),
        (op_values[1], own(CORE + 6, 1)),
        (Fp::ONE, constant(vec![Fp::ONE])),
        (op_values[3], credbytes(448, 16)),
        (op_values[4], credbytes(464, 16)),
    ];
    let receipt_items: Vec<_> = std::iter::once((Fp::from(338), constant(vec![Fp::from(338)])))
        .chain(
            input
                .receipt
                .body
                .chunks(31)
                .enumerate()
                .map(|(i, b)| (word(b), packed(&c, i * 31, b.len()))),
        )
        .collect();
    let object_items = vec![
        (m, own(mstate[1], 1)),
        (fp(&(&r % pow2(128))), c.bytes(&rle[..16])),
        (fp(&(&r >> 128)), c.bytes(&rle[16..])),
        (fp(&(&s % pow2(128))), c.bytes(&sle[..16])),
        (fp(&(&s >> 128)), c.bytes(&sle[16..])),
    ];
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"exact future Bootstrap hashes");
    for (domain, items, state) in [
        (b"kgwrest1", rest_items, rest_state),
        (b"kgwcore1", core_items, state_cells),
        (b"kgwstmt1", statement_items, statement_cells),
        (b"kgwopid1", op_items, op_cells),
        (b"kgwrcpt1", receipt_items, mstate),
        (b"kgworcp1", object_items, rdstate),
    ] {
        certificate::hash_relation(
            &mut w,
            gfam,
            local,
            u64::from_le_bytes(*domain),
            &items,
            state,
            &mut rng,
        );
    }
    let (_, comb) = certificate::add_signature(
        &mut w,
        gfam,
        local,
        MESSAGE,
        m,
        (q, z, r, s),
        [rlimbs, slimbs],
        KeyTable::Witness {
            id: nonnative::WALLET_TABLE,
        },
    );
    w.wallet = Some((comb + nonnative::A_WTABLE, full::TABLE_IO));
    let table_values: Vec<_> = cred.body[195..259].chunks_exact(16).map(word).collect();
    let table_sources: Vec<_> = (0..4).map(|i| credbytes(195 + 16 * i, 16)).collect();
    public(
        &mut w,
        gfam,
        local,
        full::TABLE_IO,
        &table_values,
        &table_sources,
    );
    public(
        &mut w,
        gfam,
        local,
        full::HEAD_IO,
        &[state],
        &[own(state_cells[1], 1)],
    );
    let mut context_values = [Fp::ZERO; 13];
    context_values[..2].copy_from_slice(&id(&context.root.scheme));
    context_values[2..4].copy_from_slice(&id(&context.relation));
    context_values[4..6].copy_from_slice(&id(&cred.body[34..66]));
    context_values[6..8].copy_from_slice(&id(&cred.body[66..98]));
    context_values[8] = credential::digest(cred);
    for (j, start) in [147, 131, 179, 163].into_iter().enumerate() {
        context_values[9 + j] = fp(&BigUint::from_bytes_be(&cred.body[start..start + 16]));
    }
    let sources: [Lin<Fp>; 13] = std::array::from_fn(|i| match i {
        0..=1 => credbytes(2 + 16 * i, 16),
        2..=3 => link(local, STATEMENT + i - 1, vec![Some(0)], Fp::ONE),
        4..=5 => credbytes(34 + 16 * (i - 4), 16),
        6..=7 => credbytes(66 + 16 * (i - 6), 16),
        8 => link(cl, cd, vec![Some(0)], Fp::ONE),
        _ => other_bytes(cl, [147, 131, 179, 163][i - 9], 16, true),
    });
    crate::bootstrap_context::expose(&mut w, gfam, &context_values, &sources);
    public(
        &mut w,
        gfam,
        local,
        LINEAGE_IO,
        &[Fp::ONE, Fp::ZERO, Fp::ZERO, Fp::ZERO, empty, empty],
        &[
            own(CORE, 1),
            own(CORE + 27, 1),
            own(CORE + 22, 1),
            own(CORE + 10, 1),
            own(CORE + 18, 1),
            constant(vec![empty]),
        ],
    );
    let mut ids = std::collections::HashSet::new();
    w.tables.retain(|(_, id)| ids.insert(*id));
    Ok(Prepared {
        wiring: w,
        gfam,
        local,
        credential_local: cl,
        state_cell: state_cells[1],
        statement_cell: statement_cells[1],
        receipt_cell: rdstate[1],
        receipt_comb: comb,
    })
}

/// Reconstruct the compact public inputs from the original expected context,
/// credential and state. This does not authenticate them. The caller must first
/// accept the Bootstrap proof against those expectations; no proof IO is read.
pub(crate) fn projection(context: &Context, input: &Inputs) -> crate::carrier::ProjectedInputs {
    let body = &input.credential.body;
    let mut words = [Fp::ZERO; 13];
    words[..2].copy_from_slice(&id(&context.root.scheme));
    words[2..4].copy_from_slice(&id(&context.relation));
    words[4..6].copy_from_slice(&id(&body[34..66]));
    words[6..8].copy_from_slice(&id(&body[66..98]));
    words[8] = credential::digest(&input.credential);
    for (i, start) in [147, 131, 179, 163].into_iter().enumerate() {
        words[9 + i] = fp(&BigUint::from_bytes_be(&body[start..start + 16]));
    }
    let table = std::array::from_fn(|i| word(&body[195 + 16 * i..211 + 16 * i]));
    let head = hash(
        b"kgwcore1",
        &[input.core.as_slice(), &[hash(b"kgwrest1", &input.rest)]].concat(),
    );
    crate::carrier::ProjectedInputs::from_authenticated_fields(
        head,
        words,
        [
            input.core[0],
            input.core[27],
            input.core[22],
            input.core[10],
            input.core[18],
            empty_indexed_root(),
        ],
        table,
        full::BOOTSTRAP,
    )
    .expect("valid expected Bootstrap output encodings")
}

/// Public deterministic research fixture keys only. Actual deployments supply
/// root/issuer-authorized credential bytes and hardware-produced receipt bytes.
pub(crate) fn fixture(
    key: &WalletKey,
    gens: &crate::ipa::Gens<iroha_pasta::Eq>,
) -> (Context, Inputs) {
    let bytes = |name: &str| -> Vec<u8> {
        let h = include_str!("../fixtures/credential-source-vectors.txt")
            .lines()
            .filter_map(|l| l.split_once('='))
            .find(|(k, _)| *k == name)
            .unwrap()
            .1;
        (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
            .collect()
    };
    let cert = Certificate {
        body: bytes("certificate_body_108").try_into().unwrap(),
        signature: bytes("certificate_signature").try_into().unwrap(),
    };
    let context = Context {
        root: RootContext {
            scheme: cert.body[2..34].try_into().unwrap(),
            root: p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[0x11; 32])).unwrap(),
        },
        relation: [0x33; 32],
    };
    let old = bytes("credential_body_468");
    let mut cred = Credential {
        body: [0; 532],
        signature: [0; 64],
    };
    cred.body[..195].copy_from_slice(&old[..195]);
    cred.body[259..].copy_from_slice(&old[195..]);
    cred.body[131..163].copy_from_slice(&scalar_bytes(&key.q.x));
    cred.body[163..195].copy_from_slice(&scalar_bytes(&key.q.y));
    // Distinct wallets keep distinct issuer-authenticated identities.
    let wid = identity(
        b"wallet-id",
        &[
            &cred.body[2..66],
            &cred.body[130..195],
            &cred.body[448..480],
        ]
        .concat(),
    );
    cred.body[66..98].copy_from_slice(&wid);
    let table = full::wallet_table_commitment(&key.q, gens);
    for (i, v) in crate::spartan::limbs_of_point::<iroha_pasta::Eq>(&table)
        .iter()
        .enumerate()
    {
        cred.body[195 + 16 * i..211 + 16 * i].copy_from_slice(&v.to_repr().as_ref()[..16]);
    }
    cred.body[500..].copy_from_slice(certificate::certificate_digest(&cert).to_repr().as_ref());
    cred.signature = sign(
        credential::message(&cred),
        &BigUint::from_bytes_be(&[0x26; 32]),
        "Bootstrap issuer fixture",
    );
    let mut core = [Fp::ZERO; 30];
    core[0] = Fp::ONE;
    for (co, bo) in [(1, 2), (3, 34), (5, 66)] {
        core[co..co + 2].copy_from_slice(&id(&cred.body[bo..bo + 32]));
    }
    core[7] = credential::digest(&cred);
    core[8] = padding_root(b"kgwpcrd1", b"kgwpcrn1", 1, 16);
    core[17..21].fill(empty_indexed_root());
    core[21] = padding_root(b"kgwquse1", b"kgwqusn1", 4, 6);
    core[25] = word(&cred.body[440..448]);
    core[26] = word(&cred.body[492..500]);
    core[29] = Fp::from(12345);
    let mut rest = [Fp::ZERO; 6];
    rest[0] = word(&cred.body[436..440]);
    let head = hash(
        b"kgwcore1",
        &[core.as_slice(), &[hash(b"kgwrest1", &rest)]].concat(),
    );
    let mut st = [Fp::ZERO; 26];
    st[0] = Fp::ONE;
    st[1..3].copy_from_slice(&id(&context.relation));
    st[3..7].copy_from_slice(&core[1..5]);
    st[7] = core[7];
    st[8] = Fp::ONE;
    st[15] = head;
    st[16] = Fp::ONE;
    st[17..19].copy_from_slice(&id(&cred.body[448..480]));
    st[19..21].copy_from_slice(&id(&[0x44; 32]));
    let op = hash(b"kgwopid1", &[core[5], core[6], Fp::ONE, st[17], st[18]]);
    let mut receipt = Receipt {
        body: [0; 338],
        signature: [0; 64],
    };
    receipt.body[0] = 1;
    for (to, from) in [(2, 2), (34, 66), (66, 259)] {
        receipt.body[to..to + 32].copy_from_slice(&cred.body[from..from + 32]);
    }
    for (to, value) in [
        (114, op),
        (178, head),
        (210, hash(b"kgwstmt1", &st)),
        (242, Fp::from(987)),
    ] {
        receipt.body[to..to + 32].copy_from_slice(value.to_repr().as_ref());
    }
    receipt.body[274..306].fill(0x55);
    receipt.signature = sign(
        receipt_message(&receipt),
        &key.d,
        "Bootstrap receipt fixture",
    );
    (
        context,
        Inputs {
            certificate: cert,
            credential: cred,
            core,
            rest,
            statement: st,
            receipt,
        },
    )
}
fn identity(role: &[u8], body: &[u8]) -> [u8; 32] {
    let mut bytes = [
        b"iroha:kagemusha:wallet:v1:".as_slice(),
        role,
        &[0],
        &(body.len() as u64).to_le_bytes(),
        body,
    ]
    .concat();
    let length = (bytes.len() as u64) * 8;
    bytes.push(0x80);
    while bytes.len() % 64 != 56 {
        bytes.push(0);
    }
    bytes.extend_from_slice(&length.to_be_bytes());
    let mut state = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    for block in bytes.chunks_exact(64) {
        let words: [u32; 16] = std::array::from_fn(|i| {
            u32::from_be_bytes(block[4 * i..4 * i + 4].try_into().unwrap())
        });
        p256::sha256_compress(&mut state, &words);
    }
    let mut out = [0; 32];
    for (i, word) in state.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}
pub(crate) fn sign(m: Fp, d: &BigUint, label: &str) -> [u8; 64] {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ipa::Gens,
        r1cs::{Circuit, Instance, Witness},
        uniform::UniformCircuit,
    };
    use iroha_pasta::Eq as Vesta;
    fn build(
        context: &Context,
        input: &Inputs,
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, usize) {
        build_at(context, input, 14)
    }
    fn build_at(
        context: &Context,
        input: &Inputs,
        col_bits: usize,
    ) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, usize) {
        let p = prepare(context, input).unwrap();
        let local = p.local;
        let (a, pi, counts, tables, copies) = p.wiring.finish();
        let (c, i, w) = UniformCircuit::build_with_copies(
            "exact Bootstrap",
            a,
            pi,
            counts,
            tables,
            copies,
            col_bits,
            64,
            b"exact Bootstrap",
        );
        (c, i, w, local)
    }
    fn holds(c: &UniformCircuit<Fp>, i: &Instance<Fp>, w: &Witness<Fp>) -> bool {
        let mut witness = w.w.clone();
        let rel = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut witness));
        let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: witness });
        let (a, b, values) = rel.abc(&c.shape, &z);
        let bad: Vec<_> = a
            .iter()
            .zip(&b)
            .zip(&values)
            .enumerate()
            .filter_map(|(row, ((a, b), value))| (*a * b != *value).then_some(row))
            .collect();
        if std::env::var("ZKS_BOOT_DEBUG").is_ok() {
            for row in bad.iter().take(8) {
                eprintln!(
                    "bad row {row}: {:?}",
                    c.row_owner(*row)
                        .map(|(a, j, i)| (c.arrays[a].name, a, j, i))
                );
            }
        }
        bad.is_empty()
    }
    #[test]
    fn recursive_natural_layout_profiles() {
        let (gens, pairs) = full::receive_pairs();
        let key = WalletKey::stand_in();
        let p = |w: Wiring<Fp>| {
            let (a, pi, count, tables, copies) = w.finish();
            UniformCircuit::build_with_copies(
                "P-uniform",
                a,
                pi,
                count,
                tables,
                copies,
                16,
                64,
                b"profile P",
            )
            .0
        };
        let pr = p(full::p_recv(&gens, &pairs, &key).0);
        let pt = p(full::p_send(&gens, &pairs[0], &key).0);
        let pb = p(full::p_boot(&gens, &key).0);
        let pl = crate::bootstrap_layout::common(&[&pr, &pt, &pb]);
        println!(
            "recursive initial P16 common={pl:?}; recv={:?}; send={:?}; boot={:?}",
            pr.regions(),
            pt.regions(),
            pb.regions()
        );
        for cols in [14, 15] {
            let build = |w: Wiring<crate::verifier::Fq>| {
                let (a, pi, count, tables, copies) = w.finish();
                crate::uniform::UniformCircuit::build_with_copies(
                    "S-uniform",
                    a,
                    pi,
                    count,
                    tables,
                    copies,
                    cols,
                    62,
                    b"profile S",
                )
                .0
            };
            let sr = build(full::s_recv(&gens, &pairs).0);
            let st = build(full::s_send(&gens, &pairs[0]).0);
            let sb = build(full::s_boot(&gens).0);
            let sl = crate::bootstrap_layout::common(&[&sr, &st, &sb]);
            println!(
                "recursive initial S{cols} common={sl:?}; recv={:?}; send={:?}; boot={:?}",
                sr.regions(),
                st.regions(),
                sb.regions()
            );
        }
    }
    #[test]
    fn natural_layout_column_sweep() {
        let key = WalletKey::stand_in();
        let (context, input) = fixture(&key, &Gens::<Vesta>::new(1 << 14));
        for cols in [14, 15, 16] {
            let (c, i, w, _) = build_at(&context, &input, cols);
            assert!(holds(&c, &i, &w));
            println!(
                "Bootstrap column sweep col_bits={cols}: sx={}, row_bits={}, regions={:?}, link={}, cells={}",
                c.shape.sx,
                c.shape.row_bits,
                c.regions(),
                c.link_phase_rows(),
                c.sizes().0
            );
        }
    }
    #[test]
    fn exact_bootstrap_native_roots_and_relation() {
        let check: String = identity(b"scheme", b"")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            check,
            "90882608a8e8892521a2661be1e81c3fbfca0f8773e621000daed362a37485a5"
        );
        let h: String = empty_indexed_root()
            .to_repr()
            .as_ref()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(
            h,
            "e1844c5683af83c12383e719e37a29a9b1867d1c171e77e1d060bafc401d5118"
        );
        let key = WalletKey::stand_in();
        let (context, input) = fixture(&key, &Gens::<Vesta>::new(1 << 14));
        let (c, i, w, local) = build(&context, &input);
        println!(
            "Bootstrap natural: sx={}, rows={}, phases={:?}, link={}, cells={}",
            c.shape.sx,
            c.shape.rows(),
            c.regions(),
            c.link_phase_rows(),
            c.sizes().0
        );
        assert!(holds(&c, &i, &w));
        // Every initial state field and statement field is constrained, including
        // nonce (by the hash) and the opaque, signed enrollment marker.
        for cell in [
            CORE + 1,
            CORE + 7,
            CORE + 8,
            CORE + 9,
            CORE + 17,
            CORE + 21,
            CORE + 25,
            CORE + 26,
            CORE + 29,
            REST,
            REST + 1,
            STATEMENT + 1,
            STATEMENT + 15,
            STATEMENT + 17,
            STATEMENT + 19,
            STATEMENT + 25,
        ] {
            let mut bad = Witness { w: w.w.clone() };
            bad.w[c.cell_index(local, cell, 0)] += Fp::ONE;
            assert!(!holds(&c, &i, &bad), "unbound local cell {cell}");
        }
        for slot in [
            full::HEAD_IO,
            16,
            18,
            20,
            22,
            24,
            25,
            26,
            27,
            28,
            32,
            33,
            34,
            35,
            36,
            37,
            56,
            57,
            58,
            59,
        ] {
            let mut bad = i.clone();
            bad.io[slot] += Fp::ONE;
            assert!(!holds(&c, &bad, &w), "unbound public slot {slot}");
        }
    }
    #[test]
    fn correctly_signed_invalid_bootstrap_receipts_and_statements_are_refused() {
        let key = WalletKey::stand_in();
        let (context, input) = fixture(&key, &Gens::<Vesta>::new(1 << 14));
        // These are genuine valid payment-key signatures; the relation must
        // reject the inconsistent object semantics after signature validation.
        for offset in [0, 2, 34, 66, 98, 114, 146, 178, 210, 306] {
            let mut bad = input.clone();
            bad.receipt.body[offset] ^= 1;
            bad.receipt.signature = sign(
                receipt_message(&bad.receipt),
                &key.d,
                "resigned invalid Bootstrap receipt",
            );
            let (c, i, w, _) = build(&context, &bad);
            assert!(!holds(&c, &i, &w), "accepted receipt offset {offset}");
        }
        for range in [242..274, 274..306] {
            let mut bad = input.clone();
            bad.receipt.body[range].fill(0);
            bad.receipt.signature = sign(
                receipt_message(&bad.receipt),
                &key.d,
                "resigned zero Bootstrap field",
            );
            let (c, i, w, _) = build(&context, &bad);
            assert!(!holds(&c, &i, &w));
        }
        let mut high = input.clone();
        let s = BigUint::from_bytes_be(&high.receipt.signature[32..]);
        high.receipt.signature[32..].copy_from_slice(&scalar_bytes(&(p256::n() - s)));
        let (c, i, w, _) = build(&context, &high);
        assert!(!holds(&c, &i, &w), "high S receipt");
        let mut wrong = context;
        wrong.relation[0] ^= 1;
        let (c, i, w, _) = build(&wrong, &input);
        assert!(!holds(&c, &i, &w), "wrong trusted relation");
        wrong.root.scheme[0] ^= 1;
        let (c, i, w, _) = build(&wrong, &input);
        assert!(!holds(&c, &i, &w), "wrong trusted scheme");
    }
    #[test]
    #[ignore = "full Bootstrap proof generation"]
    fn bootstrap_proves_two_wallets_with_one_root_relation_key() {
        let gens = Gens::<Vesta>::new(1 << 14);
        let mut first = None;
        for label in ["receiver-d", "Bootstrap second wallet"] {
            let key = WalletKey::labelled(label);
            let (context, input) = fixture(&key, &gens);
            let (c, i, w, _) = build(&context, &input);
            if let Some(c0) = &first {
                let c0: &UniformCircuit<Fp> = c0;
                assert_eq!(
                    (c0.shape.sx, c0.shape.row_bits, c0.shape.col_bits),
                    (c.shape.sx, c.shape.row_bits, c.shape.col_bits)
                );
                assert_eq!(c0.template_size(), c.template_size());
                assert_eq!(c0.fixed_values(&c0.shape), c.fixed_values(&c.shape));
                assert_eq!(
                    c0.entries(&[Fp::from(31), Fp::from(47)]),
                    c.entries(&[Fp::from(31), Fp::from(47)])
                );
            }
            let mut rng = SpikeRng::new(label.as_bytes());
            let verifier = first.as_ref().unwrap_or(&c);
            let (proof, _) =
                crate::spartan::prove_circuit(&verifier.shape, &gens, &i, &w, &mut rng, verifier);
            assert!(
                crate::spartan::verify_circuit(
                    &verifier.shape,
                    &gens,
                    &i,
                    &proof.to_bytes(),
                    verifier
                )
                .is_ok()
            );
            first = Some(c);
        }
    }
    #[test]
    #[ignore = "full invalid certified table proof"]
    fn signed_foreign_payment_table_is_refused_by_commitment_opening() {
        let gens = Gens::<Vesta>::new(1 << 14);
        let key = WalletKey::stand_in();
        let (context, mut input) = fixture(&key, &gens);
        let (_, foreign) = fixture(
            &WalletKey::labelled("foreign certified payment table"),
            &gens,
        );
        input.credential.body[195..259].copy_from_slice(&foreign.credential.body[195..259]);
        input.credential.signature = sign(
            credential::message(&input.credential),
            &BigUint::from_bytes_be(&[0x26; 32]),
            "signed foreign table",
        );
        input.core[7] = credential::digest(&input.credential);
        let head = hash(
            b"kgwcore1",
            &[input.core.as_slice(), &[hash(b"kgwrest1", &input.rest)]].concat(),
        );
        input.statement[7] = input.core[7];
        input.statement[15] = head;
        input.receipt.body[178..210].copy_from_slice(head.to_repr().as_ref());
        input.receipt.body[210..242]
            .copy_from_slice(hash(b"kgwstmt1", &input.statement).to_repr().as_ref());
        input.receipt.signature = sign(
            receipt_message(&input.receipt),
            &key.d,
            "signed wrong table receipt",
        );
        let (c, i, w, _) = build(&context, &input);
        assert!(
            holds(&c, &i, &w),
            "the omitted-row opening is the intended refusal boundary"
        );
        let mut rng = SpikeRng::new(b"signed foreign table proof");
        let (proof, _) = crate::spartan::prove_circuit(&c.shape, &gens, &i, &w, &mut rng, &c);
        assert!(
            crate::spartan::verify_circuit(&c.shape, &gens, &i, &proof.to_bytes(), &c).is_err()
        );
    }
}
