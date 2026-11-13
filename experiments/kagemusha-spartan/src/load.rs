//! Complete ordinary Load transition for the future research backend.
//!
//! Native must verify the genesis-rooted finality original before requesting
//! the hardware-backed Advance signature. This preserves the explicit native
//! finality carve-out in the canonical protocol: this relation proves the exact
//! ordinary receipt, value/ordinal/recovery effects and that signed Advance;
//! it neither accepts a host finality boolean nor claims to prove BLS finality.

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};
use num_bigint::BigUint;

use crate::{
    bootstrap::Receipt,
    bootstrap_context as context,
    certificate::{self, Cells, field_modulus, fp},
    credential::Credential,
    fs::SpikeRng,
    full::{VerifiedPredecessor, Wiring},
    indexed_map::{self, InsertWitness},
    monetary_state::{self, StateCells},
    operation_auth::{self, CurrentCredential},
    receipt::{self, AuthenticatedReceipt, PaymentDigest},
    send_arithmetic::Source,
    statement::{self, Kind, StatementCells},
    uniform::{constant, own, plus, scaled},
};

pub const ORDINARY_BYTES: usize = 282;
pub const ORDINARY_DOMAIN: u64 = u64::from_le_bytes(*b"kgwolod1");
pub const RECOVERY_DOMAIN: u64 = u64::from_le_bytes(*b"kgwload1");

#[derive(Clone)]
pub struct Witness {
    pub before_core: [Fp; 30],
    pub before_rest: [Fp; 6],
    pub after_core: [Fp; 30],
    pub after_rest: [Fp; 6],
    pub credential: Credential,
    pub ordinary: [u8; ORDINARY_BYTES],
    pub statement: [Fp; 26],
    pub receipt: Receipt,
    pub recovery: InsertWitness<32>,
}

pub struct LoadOperation<R> {
    before: StateCells,
    after: StateCells,
    statement: StatementCells,
    credential: CurrentCredential,
    receipt: R,
    lineage: [Source; context::LINEAGE_FIELDS],
    context: [Source; context::FIELDS],
    table: [Source; 4],
}
pub type CompleteLoad = LoadOperation<AuthenticatedReceipt>;
pub type PreparedLoad = LoadOperation<receipt::PreparedBody>;
impl<R> LoadOperation<R> {
    pub fn before(&self) -> &StateCells {
        &self.before
    }
    pub fn after(&self) -> &StateCells {
        &self.after
    }
    pub fn statement(&self) -> &StatementCells {
        &self.statement
    }
    pub fn credential(&self) -> &CurrentCredential {
        &self.credential
    }
    pub fn receipt(&self) -> &R {
        &self.receipt
    }
    pub fn head(&self) -> Source {
        source(self.after.local, self.after.commitment)
    }
    pub fn lineage(&self) -> [Source; context::LINEAGE_FIELDS] {
        self.lineage
    }
    pub fn context(&self) -> [Source; context::FIELDS] {
        self.context
    }
    pub fn table(&self) -> [Source; 4] {
        self.table
    }
    fn with_receipt<T>(self, receipt: T) -> LoadOperation<T> {
        LoadOperation {
            before: self.before,
            after: self.after,
            statement: self.statement,
            credential: self.credential,
            receipt,
            lineage: self.lineage,
            context: self.context,
            table: self.table,
        }
    }
}
impl PreparedLoad {
    pub fn body(&self) -> &receipt::PreparedBody {
        &self.receipt
    }
}
fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}
fn bounded(c: &mut Cells, value: Fp, bytes: usize) -> usize {
    let k = c.cell(value, 255);
    let indices: Vec<_> = value.to_repr()[..bytes]
        .iter()
        .map(|b| c.byte(*b))
        .collect();
    c.eq(own(k, 1), c.bytes(&indices));
    k
}
fn nonzero(c: &mut Cells, cell: usize) {
    let inverse = c.cell(
        Option::<Fp>::from(c.a.values[cell][0].invert()).unwrap_or(Fp::ZERO),
        255,
    );
    c.a.rows
        .push([own(cell, 1), own(inverse, 1), constant(vec![Fp::ONE])]);
}
fn zero(c: &mut Cells, cell: usize) -> usize {
    let value = c.a.values[cell][0];
    let bit = c.bit(value == Fp::ZERO);
    let inverse = c.cell(Option::<Fp>::from(value.invert()).unwrap_or(Fp::ZERO), 255);
    c.a.rows.push([own(cell, 1), own(bit, 1), vec![]]);
    c.a.rows.push([
        own(cell, 1),
        own(inverse, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(bit, 1), -Fp::ONE)),
    ]);
    bit
}

/// Exact ordinary receipt digest used by Native and the constrained relation.
pub fn ordinary_digest(bytes: &[u8; ORDINARY_BYTES]) -> Fp {
    let words: Vec<_> = std::iter::once(Fp::from(ORDINARY_BYTES as u64))
        .chain(
            bytes
                .chunks(31)
                .map(|chunk| fp(&BigUint::from_bytes_le(chunk))),
        )
        .collect();
    poseidon::hash_with_domain(ORDINARY_DOMAIN, &words)
}

struct Ordinary {
    local: usize,
    words: [usize; 18],
    digest: usize,
}
fn append_ordinary(w: &mut Wiring<Fp>, family: usize, bytes: &[u8; ORDINARY_BYTES]) -> Ordinary {
    let mut c = Cells::new(family);
    c.a.name = "exact ordinary Load receipt";
    for b in bytes {
        c.byte(*b);
    }
    let offsets = [
        (0, 2),
        (2, 16),
        (18, 16),
        (34, 16),
        (50, 16),
        (66, 16),
        (82, 16),
        (98, 16),
        (114, 16),
        (130, 16),
        (146, 16),
        (162, 16),
        (178, 32),
        (210, 16),
        (226, 16),
        (242, 8),
        (250, 16),
        (266, 16),
    ];
    let words = offsets.map(|(start, len)| {
        let value = BigUint::from_bytes_le(&bytes[start..start + len]);
        let indices: Vec<_> = (start..start + len).collect();
        if len == 32 {
            c.at_most(&indices, &value, &(field_modulus() - 1u32));
        }
        let cell = c.cell(fp(&value), 255);
        c.eq(own(cell, 1), c.bytes(&indices));
        cell
    });
    c.eq(own(words[0], 1), constant(vec![Fp::ONE]));
    for first in [1, 3, 5, 7, 13, 16] {
        let value = c.a.values[words[first]][0] + c.a.values[words[first + 1]][0];
        let sum = c.cell(value, 255);
        c.eq(
            own(sum, 1),
            plus(&own(words[first], 1), &own(words[first + 1], 1)),
        );
        nonzero(&mut c, sum);
    }
    nonzero(&mut c, words[10]);
    let next_ordinal_value = c.a.values[words[9]][0] + Fp::ONE;
    let next_ordinal = bounded(&mut c, next_ordinal_value, 16);
    c.eq(
        own(next_ordinal, 1),
        plus(&own(words[9], 1), &constant(vec![Fp::ONE])),
    );
    let debit_value = c.a.values[words[10]][0] + c.a.values[words[11]][0];
    let debit = bounded(&mut c, debit_value, 16);
    c.eq(own(debit, 1), plus(&own(words[10], 1), &own(words[11], 1)));
    let charge_zero = zero(&mut c, words[11]);
    let quote_zero = zero(&mut c, words[12]);
    c.eq(own(charge_zero, 1), own(quote_zero, 1));
    let after_first_value = c.a.values[words[15]][0] - Fp::from(2);
    let after_first = bounded(&mut c, after_first_value, 8);
    c.eq(
        own(words[15], 1),
        plus(&own(after_first, 1), &constant(vec![Fp::from(2)])),
    );
    let mut inputs = vec![(
        Fp::from(ORDINARY_BYTES as u64),
        constant(vec![Fp::from(ORDINARY_BYTES as u64)]),
    )];
    for (index, chunk) in bytes.chunks(31).enumerate() {
        let value = fp(&BigUint::from_bytes_le(chunk));
        let bridge = c.cell(value, 255);
        c.eq(
            own(bridge, 1),
            c.bytes(&(31 * index..31 * index + chunk.len()).collect::<Vec<_>>()),
        );
        inputs.push((value, own(bridge, 1)));
    }
    let digest = ordinary_digest(bytes);
    let outputs = [
        c.cell(Fp::ZERO, 255),
        c.cell(digest, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let local = w.push(c.a);
    certificate::hash_relation(
        w,
        family,
        local,
        ORDINARY_DOMAIN,
        &inputs,
        outputs,
        &mut SpikeRng::new(b"exact ordinary Load receipt"),
    );
    Ordinary {
        local,
        words,
        digest: outputs[1],
    }
}

pub fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<CompleteLoad, &'static str> {
    let operation = append_relation(w, family, input, predecessor)?;
    let receipt = receipt::append_own(
        w,
        family,
        &input.receipt,
        &operation.credential,
        &operation.statement,
        Kind::Load,
        PaymentDigest::Absent,
    )?;
    Ok(operation.with_receipt(receipt))
}

/// Exact same monetary/map/authentication relation before hardware Advance.
/// Native must also perform the required genesis-rooted finality verification.
pub fn append_preparation(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<PreparedLoad, &'static str> {
    let operation = append_relation(w, family, input, predecessor)?;
    let receipt = receipt::append_preparation(
        w,
        family,
        &input.receipt.body,
        &operation.credential,
        &operation.statement,
        Kind::Load,
        PaymentDigest::Absent,
    )?;
    Ok(operation.with_receipt(receipt))
}

fn append_relation(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<LoadOperation<()>, &'static str> {
    let before = monetary_state::append_state(w, family, &input.before_core, &input.before_rest);
    let after = monetary_state::append_state(w, family, &input.after_core, &input.after_rest);
    let credential =
        operation_auth::append_current(w, family, &input.credential, &before, predecessor)?;
    let statement = statement::append_statement(w, family, Kind::Load, &input.statement);
    let ordinary = append_ordinary(w, family, &input.ordinary);
    let prior = predecessor.lineage();
    let bf = |i| source(before.local, before.core[i]);
    let af = |i| source(after.local, after.core[i]);
    let sf = |i| source(statement.local, statement.fields[i]);
    let of = |i| source(ordinary.local, ordinary.words[i]);
    let digest = source(ordinary.local, ordinary.digest);
    let mut c = Cells::new(family);
    c.a.name = "complete Load monetary and original joins";
    for (a, b) in [
        (source(before.local, before.commitment), predecessor.head()),
        (bf(0), prior[context::LIFECYCLE]),
        (bf(27), prior[context::POLICY_EPOCH]),
        (bf(22), prior[context::ENABLED_CONTROLS]),
        (sf(1), predecessor.context()[context::RELATION]),
        (sf(2), predecessor.context()[context::RELATION + 1]),
        (sf(3), af(1)),
        (sf(4), af(2)),
        (sf(5), af(3)),
        (sf(6), af(4)),
        (sf(7), af(7)),
        (sf(8), af(0)),
        (sf(9), af(11)),
        (sf(10), af(13)),
        (sf(11), bf(22)),
        (sf(14), source(before.local, before.commitment)),
        (sf(15), source(after.local, after.commitment)),
        (sf(17), digest),
        (sf(18), of(9)),
        (sf(19), of(10)),
        (sf(20), of(11)),
        (of(1), bf(1)),
        (of(2), bf(2)),
        (of(3), bf(3)),
        (of(4), bf(4)),
        (of(5), bf(5)),
        (of(6), bf(6)),
        (bf(13), of(9)),
    ] {
        c.eq(a.linear(), b.linear());
    }
    c.eq(af(9).linear(), plus(&bf(9).linear(), &of(10).linear()));
    for i in [11, 13] {
        c.eq(
            af(i).linear(),
            plus(&bf(i).linear(), &constant(vec![Fp::ONE])),
        );
    }
    for i in 0..30 {
        if ![9, 11, 13, 19, 29].contains(&i) {
            c.eq(af(i).linear(), bf(i).linear());
        }
    }
    for i in 0..6 {
        c.eq(
            source(before.local, before.rest[i]).linear(),
            source(after.local, after.rest[i]).linear(),
        );
    }
    let two128 = Fp::from_u128(1u128 << 127).double();
    let recovery_key = c.cell(two128 + of(9).value(w), 255);
    c.eq(
        own(recovery_key, 1),
        plus(&constant(vec![two128]), &of(9).linear()),
    );
    let recovery_sources = [of(9), digest, of(10)];
    let recovery_value =
        poseidon::hash_with_domain(RECOVERY_DOMAIN, &recovery_sources.map(|p| p.value(w)));
    let outputs = [
        c.cell(Fp::ZERO, 255),
        c.cell(recovery_value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let local = w.push(c.a);
    let items: Vec<_> = recovery_sources
        .iter()
        .map(|p| (p.value(w), p.linear()))
        .collect();
    certificate::hash_relation(
        w,
        family,
        local,
        RECOVERY_DOMAIN,
        &items,
        outputs,
        &mut SpikeRng::new(b"Load ordinal recovery entry"),
    );
    let insertion = indexed_map::append_insert(w, family, &input.recovery);
    let mut joins = Cells::new(family);
    joins.a.name = "complete Load recovery joins";
    for (a, b) in [
        (source(insertion.local, insertion.old_root), bf(19)),
        (
            source(insertion.local, insertion.key),
            source(local, recovery_key),
        ),
        (
            source(insertion.local, insertion.value),
            source(local, outputs[1]),
        ),
        (source(insertion.local, insertion.new_root), af(19)),
    ] {
        joins.eq(a.linear(), b.linear());
    }
    w.push(joins.a);
    let lineage = [
        af(0),
        af(27),
        af(22),
        prior[context::BURNED_TOTAL],
        prior[context::PENDING_ROOT],
        prior[context::CREDIT_ROOT],
    ];
    Ok(LoadOperation {
        before,
        after,
        statement,
        credential,
        receipt: (),
        lineage,
        context: predecessor.context(),
        table: predecessor.table(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bootstrap,
        full::{self, WalletKey},
        ipa::Gens,
        operation_auth::tests as harness,
        spartan,
    };
    use iroha_pasta::Eq as Vesta;

    struct Fixture {
        input: Witness,
        profile: [Fp; 64],
        key: WalletKey,
    }
    fn fixture(charge: u128) -> Fixture {
        let key = WalletKey::labelled("complete ordinary Load fixture");
        let (context, boot) = bootstrap::fixture(&key, &Gens::<Vesta>::new(1 << 16));
        let profile = bootstrap::public_profile(&context, &boot);
        let mut ordinary = [0; ORDINARY_BYTES];
        ordinary[0] = 1;
        ordinary[2..98].copy_from_slice(&boot.credential.body[2..98]);
        ordinary[98..130].fill(0x45);
        ordinary[146..162].copy_from_slice(&2000u128.to_le_bytes());
        ordinary[162..178].copy_from_slice(&charge.to_le_bytes());
        ordinary[178] = u8::from(charge != 0);
        ordinary[210..242].fill(0x56);
        ordinary[242] = 2;
        ordinary[250..282].copy_from_slice(&boot.credential.body[98..130]);
        let digest = ordinary_digest(&ordinary);
        let value =
            poseidon::hash_with_domain(RECOVERY_DOMAIN, &[Fp::ZERO, digest, Fp::from(2000)]);
        let (recovery, root) =
            indexed_map::empty_insert(Fp::from_u128(1u128 << 127).double(), value);
        let mut after = boot.core;
        after[9] = Fp::from(2000);
        after[11] = Fp::ONE;
        after[13] = Fp::ONE;
        after[19] = root;
        after[29] += Fp::ONE;
        let mut statement = boot.statement;
        statement[9] = Fp::ONE;
        statement[10] = Fp::ONE;
        statement[14] = profile[full::HEAD_IO];
        statement[15] = monetary_state::commitment(&after, &boot.rest);
        statement[16] = Fp::from(2);
        statement[17] = digest;
        statement[18] = Fp::ZERO;
        statement[19] = Fp::from(2000);
        statement[20] = Fp::from_u128(charge);
        statement[21..].fill(Fp::ZERO);
        let mut input = Witness {
            before_core: boot.core,
            before_rest: boot.rest,
            after_core: after,
            after_rest: boot.rest,
            credential: boot.credential,
            ordinary,
            statement,
            receipt: boot.receipt,
            recovery,
        };
        sign(&mut input, &key);
        Fixture {
            input,
            profile,
            key,
        }
    }
    fn sign(input: &mut Witness, key: &WalletKey) {
        let operation = poseidon::hash_with_domain(
            u64::from_le_bytes(*b"kgwopid1"),
            &[
                input.before_core[5],
                input.before_core[6],
                Fp::from(2),
                input.statement[17],
            ],
        );
        input.receipt.body[98..114].copy_from_slice(&input.statement[9].to_repr()[..16]);
        for (at, value) in [
            (114, operation),
            (146, input.statement[14]),
            (178, input.statement[15]),
            (210, statement::digest(&input.statement)),
        ] {
            input.receipt.body[at..at + 32].copy_from_slice(value.to_repr().as_ref());
        }
        input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&input.receipt),
            &key.d,
            "complete Load receipt fixture",
        );
    }
    fn build(f: &Fixture) -> Result<harness::Built, &'static str> {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, f.profile);
        let completed = append(&mut w, family, &f.input, &predecessor)?;
        harness::expose(
            &mut w,
            family,
            4,
            &[
                completed.head(),
                source(completed.statement.local, completed.statement.digest),
                completed.receipt.digest(),
            ],
        );
        harness::expose(&mut w, family, 16, &completed.context());
        harness::expose(&mut w, family, 32, &completed.lineage());
        Ok(harness::finish(w, 16))
    }
    fn parser(bytes: &[u8; 282]) -> harness::Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        operation_auth::ensure_range(&mut w, family);
        let receipt = append_ordinary(&mut w, family, bytes);
        harness::expose(
            &mut w,
            family,
            4,
            &[
                source(receipt.local, receipt.digest),
                source(receipt.local, receipt.words[9]),
                source(receipt.local, receipt.words[10]),
                source(receipt.local, receipt.words[11]),
            ],
        );
        harness::finish(w, 12)
    }
    #[test]
    fn ordinary_receipt_matches_native_golden_and_rejects_malformed_terms() {
        let text = concat!(
            "01009692c8c83bef03c3991f61516a22cdb560ba5a980bfd4a9c3f27339c9d2dd4456",
            "e85849d458ab1a848d48e030c816121e9b08998cc5b2aff7481f2b933875101",
            "54b2461056dc535bf12f3fe2676c94b51077cc1c37770f454250395c678e5f54",
            "9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b9b",
            "0000000000000000000000000000000088130000000000000000000000000000",
            "190000000000000000000000000000009756504f25a62ff49f27f980828bb62f11c2b8678514cd5906aaa274bee15b3a",
            "9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a9a",
            "2a00000000000000e090740d52cb3d9a4969d253e0ada8b815c87385ca103787ac4c72b2b0c38204"
        );
        assert_eq!(text.len(), 564);
        let bytes =
            std::array::from_fn(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap());
        let expected = "bd76885c1791108bf4d8627fdf6759f8ae0aac2c6d5be737c2a8bd114c8ea504";
        let expected: [u8; 32] =
            std::array::from_fn(|i| u8::from_str_radix(&expected[2 * i..2 * i + 2], 16).unwrap());
        assert_eq!(ordinary_digest(&bytes).to_repr(), expected);
        assert!(harness::holds(&parser(&bytes)));
        for (name, at, raw) in [
            ("version", 0, 0u16.to_le_bytes().to_vec()),
            ("wallet", 66, vec![0; 32]),
            ("request", 98, vec![0; 32]),
            ("ordinal overflow", 130, vec![255; 16]),
            ("zero amount", 146, vec![0; 16]),
            ("debit overflow", 146, vec![255; 16]),
            ("missing quote", 178, vec![0; 32]),
            ("noncanonical quote", 178, field_modulus().to_bytes_le()),
            ("transaction", 210, vec![0; 32]),
            ("height", 242, 1u64.to_le_bytes().to_vec()),
            ("payer", 250, vec![0; 32]),
        ] {
            let mut changed = bytes;
            changed[at..at + raw.len()].copy_from_slice(&raw);
            assert!(!harness::holds(&parser(&changed)), "{name}");
        }
    }
    #[test]
    fn complete_load_funds_zero_bootstrap_without_crediting_online_charge() {
        let a = build(&fixture(0)).unwrap();
        let b = build(&fixture(3)).unwrap();
        assert_eq!(
            a.0.entries(&[Fp::from(31), Fp::from(47)]),
            b.0.entries(&[Fp::from(31), Fp::from(47)])
        );
        let gens = Gens::<Vesta>::new(a.0.shape.cols());
        for candidate in [&a, &b] {
            assert!(harness::holds(candidate));
            let (_, i, w) = candidate;
            let (proof, _) = spartan::prove_circuit(
                &a.0.shape,
                &gens,
                i,
                w,
                &mut SpikeRng::new(b"complete Load proof"),
                &a.0,
            );
            assert!(spartan::verify_circuit(&a.0.shape, &gens, i, &proof.to_bytes(), &a.0).is_ok());
            for at in [4, 5, 6, 24, 32, 35, 36, 37, 56, 128 + 4, 128 + 24] {
                let mut changed = i.clone();
                changed.io[at] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(&a.0.shape, &gens, &changed, &proof.to_bytes(), &a.0)
                        .is_err(),
                    "public port {at}"
                );
            }
        }
    }
    #[test]
    fn complete_load_replay_and_original_state_substitutions_fail() {
        for index in [9, 10, 12, 13, 17, 18, 19, 20, 21, 22, 28] {
            let mut f = fixture(3);
            f.input.after_core[index] += Fp::ONE;
            f.input.statement[15] =
                monetary_state::commitment(&f.input.after_core, &f.input.after_rest);
            sign(&mut f.input, &f.key);
            assert!(
                !harness::holds(&build(&f).unwrap()),
                "signed bad successor core {index}"
            );
        }
        let mut replay = fixture(3);
        replay.input.before_core = replay.input.after_core;
        replay.profile[full::HEAD_IO] = replay.input.statement[15];
        replay.input.statement[14] = replay.input.statement[15];
        sign(&mut replay.input, &replay.key);
        assert!(
            !harness::holds(&build(&replay).unwrap()),
            "same ordinary ordinal cannot fund twice"
        );
        let mut changed = fixture(3);
        changed.input.ordinary[250] ^= 1;
        assert!(
            !harness::holds(&build(&changed).unwrap()),
            "original payer byte bound even outside monetary projection"
        );
    }
}
