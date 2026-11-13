//! Current-credential authority joins and total original-byte verdict tests.
use super::*;
use crate::{
    bootstrap,
    fs::SpikeRng,
    full::{self, WalletKey},
    ipa::Gens,
    monetary_state,
    r1cs::{Circuit, Instance, Witness},
    spartan,
    uniform::UniformCircuit,
};
use iroha_pasta::Eq as Vesta;
pub(crate) type Built = (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>);

pub(crate) fn predecessor(
    w: &mut Wiring<Fp>,
    family: usize,
    profile: [Fp; 64],
) -> VerifiedPredecessor {
    let verified = VerifiedPredecessor::fixture(w, family, profile);
    let io = w.push(Array {
        name: "public verified predecessor fixture",
        family,
        rho: 6,
        r_count: 64,
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![profile.to_vec()],
    });
    w.io_arrays.push((io, 128));
    let source = verified.head();
    let leaves = source.instance - full::HEAD_IO;
    w.push(Array {
        name: "fixture predecessor public binding",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: (0..64)
            .map(|i| {
                [
                    plus(
                        &link(source.array, 0, vec![Some(leaves + i)], Fp::ONE),
                        &link(io, 0, vec![Some(i)], -Fp::ONE),
                    ),
                    constant(vec![Fp::ONE]),
                    vec![],
                ]
            })
            .collect(),
    });
    verified
}
pub(crate) fn expose(w: &mut Wiring<Fp>, family: usize, offset: usize, values: &[Source]) {
    let n = values.len().next_power_of_two();
    let mut v: Vec<_> = values.iter().map(|s| s.value(w)).collect();
    v.resize(n, Fp::ZERO);
    let io = w.push(Array {
        name: "authentication test outputs",
        family,
        rho: n.trailing_zeros() as usize,
        r_count: values.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![255],
        rows: vec![],
        values: vec![v],
    });
    w.io_arrays.push((io, offset));
    w.push(Array {
        name: "authentication test output binding",
        family,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![],
        values: vec![],
        rows: values
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
}
pub(crate) fn finish(w: Wiring<Fp>, cols: usize) -> Built {
    let (a, p, n, t, c) = w.finish();
    UniformCircuit::build_with_copies(
        "operation auth",
        a,
        p,
        n,
        t,
        c,
        cols,
        192,
        b"operation auth fixture",
    )
}
pub(crate) fn holds((c, i, w): &Built) -> bool {
    let mut w = w.w.clone();
    let r = c.instantiate(&c.shape, &[Fp::from(31), Fp::from(47)], Some(&mut w));
    let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w });
    let (a, b, c) = r.abc(&c.shape, &z);
    a.into_iter().zip(b).zip(c).all(|((a, b), c)| a * b == c)
}
pub(crate) fn proof(b: &Built) -> bool {
    let (c, i, w) = b;
    let g = Gens::<Vesta>::new(c.shape.cols());
    let (p, _) = spartan::prove_circuit(&c.shape, &g, i, w, &mut SpikeRng::new(b"auth proof"), c);
    spartan::verify_circuit(&c.shape, &g, i, &p.to_bytes(), c).is_ok()
}
fn raw(credential: &Credential) -> Built {
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    let o = append_opening_total(&mut w, f, credential);
    expose(
        &mut w,
        f,
        4,
        &[o.valid(), o.digest(), o.account()[0], o.account()[1]],
    );
    finish(w, 12)
}
fn current(input: &bootstrap::Inputs, profile: [Fp; 64]) -> Built {
    let mut w = Wiring::default();
    let f = w.family(0, 1);
    let prior = predecessor(&mut w, f, profile);
    let state = monetary_state::append_state(&mut w, f, &input.core, &input.rest);
    let c = append_current(&mut w, f, &input.credential, &state, &prior).unwrap();
    expose(
        &mut w,
        f,
        4,
        &[c.valid(), c.digest(), c.account()[0], c.account()[1]],
    );
    finish(w, 12)
}
#[test]
fn total_credential_has_one_verdict_for_valid_and_malformed_originals() {
    let key = WalletKey::stand_in();
    let (_, input) = bootstrap::fixture(&key, &Gens::<Vesta>::new(1 << 14));
    let good = raw(&input.credential);
    assert!(holds(&good));
    assert_eq!(good.1.io[4], Fp::ONE);
    assert!(proof(&good));
    let mut cases = vec![];
    for (offset, value) in [
        (0, 2),
        (291, 0),
        (291, 4),
        (292, 0),
        (348, 0),
        (436, 4),
        (131, 0),
        (195, 0),
    ] {
        let mut c = input.credential.clone();
        c.body[offset] = value;
        if c.body == input.credential.body {
            c.body[offset] ^= 1;
        }
        cases.push(c);
    }
    let mut c = input.credential.clone();
    c.body[500..532].fill(255);
    cases.push(c);
    let mut c = input.credential.clone();
    c.signature[..32].fill(0);
    cases.push(c);
    let mut c = input.credential.clone();
    let high_s = p256::n() - BigUint::from_bytes_be(&c.signature[32..]);
    c.signature[32..].copy_from_slice(&certificate::scalar_bytes(&high_s));
    cases.push(c);
    cases.push(Credential {
        body: [0; 532],
        signature: [0; 64],
    });
    cases.push(Credential {
        body: [255; 532],
        signature: [255; 64],
    });
    let entries = good.0.entries(&[Fp::from(31), Fp::from(47)]);
    for (i, c) in cases.iter().enumerate() {
        let mut b = raw(c);
        assert!(
            holds(&b),
            "invalid original still has a valid total opening {i}"
        );
        assert_eq!(b.1.io[4], Fp::ZERO, "malformed original {i}");
        assert_eq!(
            entries,
            b.0.entries(&[Fp::from(31), Fp::from(47)]),
            "same VK for malformed {i}"
        );
        if i == 0 {
            assert!(proof(&b));
        }
        b.1.io[4] = Fp::ONE;
        assert!(!holds(&b), "cannot claim malformed original valid {i}");
    }
    let mut bad = good;
    bad.1.io[4] = Fp::ZERO;
    assert!(!holds(&bad));
    assert!(!proof(&bad));
}
#[test]
fn verified_current_digest_identity_key_and_state_are_mandatory() {
    let key = WalletKey::stand_in();
    let (ctx, input) = bootstrap::fixture(&key, &Gens::<Vesta>::new(1 << 14));
    let profile = bootstrap::public_profile(&ctx, &input);
    let good = current(&input, profile);
    assert!(holds(&good));
    assert!(proof(&good));
    for at in [
        full::HEAD_IO,
        16,
        17,
        20,
        21,
        22,
        23,
        24,
        25,
        26,
        27,
        28,
        56,
        57,
        58,
        59,
    ] {
        let mut altered = profile;
        altered[at] += Fp::ONE;
        let b = current(&input, altered);
        assert!(!holds(&b), "unbound verified port {at}");
        assert_eq!(
            good.0.entries(&[Fp::from(31), Fp::from(47)]),
            b.0.entries(&[Fp::from(31), Fp::from(47)])
        );
    }
    let mut bad = input.clone();
    bad.credential.body[98] ^= 1;
    let b = current(&bad, profile);
    assert!(!holds(&b), "account is in authenticated exact digest");
    let other = WalletKey::labelled("auth second wallet");
    let (ctx, input) = bootstrap::fixture(&other, &Gens::<Vesta>::new(1 << 14));
    let b = current(&input, bootstrap::public_profile(&ctx, &input));
    assert!(holds(&b));
    assert!(proof(&b));
    assert_eq!(
        good.0.entries(&[Fp::from(31), Fp::from(47)]),
        b.0.entries(&[Fp::from(31), Fp::from(47)])
    );
}
