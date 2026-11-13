use super::*;
use crate::{
    bootstrap, credential,
    full::WalletKey,
    ipa::Gens,
    monetary_state,
    operation_auth::{
        self,
        tests::{Built, expose, finish, holds, predecessor, proof},
    },
    r1cs::Circuit,
};
use iroha_pasta::Eq as Vesta;

pub(crate) fn fields(input: &bootstrap::Inputs, kind: Kind) -> [Fp; 26] {
    let mut f = input.statement;
    f[9] = Fp::ONE;
    f[14] = monetary_state::commitment(&input.core, &input.rest);
    f[15] = Fp::from(999);
    f[16] = Fp::from(kind.tag());
    f[17..].fill(Fp::ZERO);
    f[17] = Fp::from(123);
    f[18] = Fp::from(71);
    f[19] = Fp::from(72);
    if kind == Kind::Send {
        f[13] = Fp::from(99);
        f[21] = Fp::from(5);
        f[23] = Fp::from(555);
        f[24] = Fp::from(100);
        f[25] = Fp::from(100);
    } else {
        f[13] = Fp::ZERO;
        f[20] = Fp::from(5);
    }
    f
}
pub(crate) fn signed(
    key: &WalletKey,
    credential: &credential::Credential,
    f: &[Fp; 26],
    payment: Fp,
) -> Receipt {
    let mut r = Receipt {
        body: [0; 338],
        signature: [0; 64],
    };
    r.body[..2].copy_from_slice(&1u16.to_le_bytes());
    for (to, from) in [(2, 2), (34, 66), (66, 259)] {
        r.body[to..to + 32].copy_from_slice(&credential.body[from..from + 32]);
    }
    r.body[98..114].copy_from_slice(&f[9].to_repr()[..16]);
    let op = poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[
            word(&credential.body[66..82]),
            word(&credential.body[82..98]),
            f[16],
            f[17],
        ],
    );
    for (at, v) in [
        (114, op),
        (146, f[14]),
        (178, f[15]),
        (210, crate::statement::digest(f)),
        (242, Fp::from(7)),
        (306, payment),
    ] {
        r.body[at..at + 32].copy_from_slice(v.to_repr().as_ref());
    }
    r.body[274..306].fill(77);
    r.signature = bootstrap::sign(message(&r), &key.d, "shared receipt fixture");
    r
}
fn own(key: &WalletKey, kind: Kind, mutate: impl FnOnce(&mut Receipt)) -> Built {
    own_steps(key, kind, mutate, |_| {})
}
fn own_steps(
    key: &WalletKey,
    kind: Kind,
    mutate: impl FnOnce(&mut Receipt),
    after_preparation: impl FnOnce(&mut Receipt),
) -> Built {
    let (ctx, input) = bootstrap::fixture(key, &Gens::<Vesta>::new(1 << 14));
    let f = fields(&input, kind);
    let payment = if kind == Kind::Receive {
        Fp::from(987)
    } else {
        Fp::ZERO
    };
    let mut r = signed(key, &input.credential, &f, payment);
    mutate(&mut r);
    r.signature = bootstrap::sign(message(&r), &key.d, "shared receipt fixture");
    let mut w = Wiring::default();
    let family = w.family(0, 1);
    let prior = predecessor(&mut w, family, bootstrap::public_profile(&ctx, &input));
    let state = monetary_state::append_state(&mut w, family, &input.core, &input.rest);
    let cred =
        operation_auth::append_current(&mut w, family, &input.credential, &state, &prior).unwrap();
    let stmt = crate::statement::append_statement(&mut w, family, kind, &f);
    let payment = if kind == Kind::Receive {
        let mut c = Cells::new(family);
        let cell = c.cell(payment, 255);
        let array = w.push(c.a);
        let s = Source {
            array,
            cell,
            instance: 0,
        };
        expose(&mut w, family, 8, &[s]);
        PaymentDigest::Present(s)
    } else {
        PaymentDigest::Absent
    };
    let prepared =
        append_preparation(&mut w, family, &r.body, &cred, &stmt, kind, payment).unwrap();
    after_preparation(&mut r);
    r.signature = bootstrap::sign(message(&r), &key.d, "shared receipt final signature");
    let auth =
        authenticate_prepared(&mut w, family, &prepared, &r, &cred, &stmt, kind, payment).unwrap();
    expose(&mut w, family, 4, &[auth.digest()]);
    finish(w, 14)
}
fn raw(r: &Receipt) -> Built {
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    operation_auth::ensure_range(&mut w, f);
    let o = append_opening_total(&mut w, f, r);
    expose(
        &mut w,
        f,
        4,
        &[
            o.valid(),
            o.digest(),
            o.statement_digest(),
            o.payment_digest(),
        ],
    );
    finish(w, 12)
}
#[test]
fn own_send_receive_receipts_prove_and_bind_every_exact_field() {
    let key = WalletKey::stand_in();
    let good = own(&key, Kind::Send, |_| {});
    assert!(holds(&good));
    assert!(proof(&good));
    let recv = own(&key, Kind::Receive, |_| {});
    assert!(holds(&recv));
    assert!(proof(&recv));
    let entries = good.0.entries(&[Fp::from(31), Fp::from(47)]);
    for at in [0, 2, 34, 66, 98, 114, 146, 178, 210, 306] {
        let b = own(&key, Kind::Send, |r| r.body[at] ^= 1);
        assert!(!holds(&b), "wrong receipt field {at}");
        assert_eq!(
            entries,
            b.0.entries(&[Fp::from(31), Fp::from(47)]),
            "same VK {at}"
        );
    }
    for (a, b) in [(242, 274), (274, 306)] {
        let b = own(&key, Kind::Send, |r| r.body[a..b].fill(0));
        assert!(!holds(&b));
    }
    let b = own(&key, Kind::Receive, |r| r.body[306] ^= 1);
    assert!(!holds(&b));
    let other = own(
        &WalletKey::labelled("receipt second wallet"),
        Kind::Send,
        |_| {},
    );
    assert!(holds(&other));
    assert!(proof(&other));
    assert_eq!(entries, other.0.entries(&[Fp::from(31), Fp::from(47)]));
    let mut bad = good;
    bad.1.io[4] += Fp::ONE;
    assert!(!proof(&bad));
}
#[test]
fn incoming_receipt_parsing_is_total_and_cannot_invent_failure() {
    let key = WalletKey::stand_in();
    let (_, input) = bootstrap::fixture(&key, &Gens::<Vesta>::new(1 << 14));
    let f = fields(&input, Kind::Send);
    let r = signed(&key, &input.credential, &f, Fp::ZERO);
    let good = raw(&r);
    assert!(holds(&good));
    assert_eq!(good.1.io[4], Fp::ONE);
    assert_eq!(good.1.io[5], digest(&r));
    assert!(proof(&good));
    let mut cases = vec![
        Receipt {
            body: [0; 338],
            signature: [0; 64],
        },
        Receipt {
            body: [255; 338],
            signature: [255; 64],
        },
    ];
    for (a, b) in [
        (114, 146),
        (146, 178),
        (178, 210),
        (210, 242),
        (242, 274),
        (306, 338),
    ] {
        let mut c = r.clone();
        c.body[a..b].fill(255);
        cases.push(c);
    }
    for (a, b) in [(114, 146), (242, 274), (274, 306)] {
        let mut c = r.clone();
        c.body[a..b].fill(0);
        cases.push(c);
    }
    let mut c = r.clone();
    c.signature[..32].fill(0);
    cases.push(c);
    let mut c = r.clone();
    let high = p256::n() - BigUint::from_bytes_be(&c.signature[32..]);
    c.signature[32..].copy_from_slice(&certificate::scalar_bytes(&high));
    cases.push(c);
    let entries = good.0.entries(&[Fp::from(31), Fp::from(47)]);
    for (i, c) in cases.iter().enumerate() {
        let mut b = raw(c);
        assert!(holds(&b), "total malformed {i}");
        assert_eq!(b.1.io[4], Fp::ZERO, "verdict {i}");
        assert_eq!(b.1.io[5], digest(c));
        assert_eq!(entries, b.0.entries(&[Fp::from(31), Fp::from(47)]));
        if i == 0 {
            assert!(proof(&b));
        }
        b.1.io[4] = Fp::ONE;
        assert!(!holds(&b));
    }
    let mut bad = good;
    bad.1.io[4] = Fp::ZERO;
    assert!(!holds(&bad));
    assert!(!proof(&bad));
}

#[test]
fn prepared_body_cannot_be_replaced_even_with_a_valid_hardware_signature() {
    let key = WalletKey::stand_in();
    for offset in [242, 274] {
        let changed = own_steps(&key, Kind::Send, |_| {}, |r| r.body[offset] ^= 1);
        assert!(!holds(&changed), "prepared intent byte {offset}");
    }
}
