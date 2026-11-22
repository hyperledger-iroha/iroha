//! An any-kind slot's key selection with a fifth (ArchiveSent) key: the
//! verified proof's fixed rows must be exactly the key its kind bits select.
//! Component programs only: five toy circuits of one layout stand in for the
//! five programs; nothing here is a monetary generation.

use super::*;
use crate::fs::SpikeRng;
use crate::full::{NODE_DOMAIN, constant_entries};
use crate::r1cs::{Circuit, Instance, Witness};
use crate::uniform::UniformCircuit;
use iroha_pasta::{Ep as Pallas, Fp, Fq};

/// One kind's toy program: the same structure for every kind, its own fixed
/// constants (so its own key).
fn program(kind: u64) -> (UniformCircuit<Fq>, Instance<Fq>, Witness<Fq>) {
    let mut w = Wiring::<Fq>::default();
    let f = w.family(0, 1);
    let values = vec![Fq::from(kind + 1), Fq::from(3 * kind + 5)];
    let key = w.push(Array {
        name: "kind key constants",
        family: f,
        rho: 1,
        r_count: 2,
        fixed: true,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![values.clone()],
    });
    w.push(Array {
        name: "kind key copy",
        family: f,
        rho: 1,
        r_count: 2,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: vec![[
            plus(&own(0, 2), &link(key, 0, vec![Some(0), Some(1)], -Fq::ONE)),
            constant(vec![Fq::ONE, Fq::ONE]),
            Vec::new(),
        ]],
        values: vec![values],
    });
    let (a, p, n, t, c) = w.finish();
    UniformCircuit::build_with_copies("kind key program", a, p, n, t, c, 10, 4, b"kind key program")
}

struct Kinds {
    gens: Gens<Pallas>,
    programs: Vec<(UniformCircuit<Fq>, Instance<Fq>, Witness<Fq>)>,
    keys: Vec<Vec<Aff<Pallas>>>,
}

fn kinds() -> Kinds {
    let programs: Vec<_> = (0..crate::deferral::MAX_KINDS as u64).map(program).collect();
    let gens = Gens::<Pallas>::new(programs[0].0.shape.cols());
    for (c, _, _) in &programs {
        assert_eq!(c.layout(), programs[0].0.layout(), "one layout");
        assert_eq!(c.shape.cols(), programs[0].0.shape.cols());
    }
    let keys: Vec<_> = programs
        .iter()
        .map(|(c, _, _)| crate::spartan::fixed_commitments(&c.shape, &gens, c))
        .collect();
    assert!(!keys[0].is_empty(), "the programs have fixed rows");
    for i in 0..keys.len() {
        for j in 0..i {
            assert_ne!(keys[i], keys[j], "distinct keys");
        }
    }
    Kinds {
        gens,
        programs,
        keys,
    }
}

/// Kind `kind`'s proof and its native trace under its own key.
fn proved(k: &Kinds, kind: usize) -> (crate::spartan::Proof<Pallas>, Trace<Pallas>) {
    let (c, inst, wit) = &k.programs[kind];
    let mut rng = SpikeRng::new(b"kind key proof");
    let (proof, _) = crate::spartan::prove_circuit(&c.shape, &k.gens, inst, wit, &mut rng, c);
    let trace = crate::verifier::trace_circuit(
        &c.shape,
        &k.gens,
        inst,
        &proof,
        &[],
        &crate::spartan::Link::None,
        c,
        &k.keys[kind],
    );
    assert!(trace.accepts, "kind {kind}'s proof verifies natively");
    (proof, trace)
}

/// The proof's points side in an Fp circuit whose slot takes any of `keys`,
/// with `bits` as the slot's shared kind words; whether every row holds.
fn slot_holds(
    k: &Kinds,
    keys: &[Vec<Aff<Pallas>>],
    proof: &crate::spartan::Proof<Pallas>,
    tr: &Trace<Pallas>,
    bits: &[u64],
) -> bool {
    let shape = k.programs[0].0.shape;
    let mut values = crate::verifier::shared_values(tr, proof, &[]);
    let kind_at = values.len();
    values.extend(bits.iter().map(|b| u128::from(*b)));
    let mut w = Wiring::<Fp>::default();
    let gfam = w.family(0, 1);
    let e_out = w.reserve();
    let e_v = w.reserve();
    let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![Fp::ZERO]]);
    let mut pool = Pool::new(Fp::from(NODE_DOMAIN));
    let mut pb = PointsBuilder::<Pallas>::new(&mut w, e_out, e_v, e_zero, IdentityPoints::Refused);
    let select = KeySelect {
        fixed: keys.iter().map(Vec::as_slice).collect(),
        kind_at,
    };
    pb.verify_keyed(
        &mut pool,
        &shape,
        &k.gens,
        proof,
        &[],
        tr,
        SharedMap {
            base: 0,
            other: None,
            r: None,
        },
        Some(&select),
        None,
    );
    let (rho, mu, _) = crate::nonnative::sample_binding(0);
    let first = w.arrays.len();
    let b = crate::nonnative::binding_chain(first, gfam, gfam, &rho, &mu, &values);
    w.arrays.extend(b.arrays);
    w.tables.extend(b.tables);
    let shared_a = first + crate::nonnative::B_SHARED;
    let n_sh = 1usize << w.arrays[shared_a].rho;
    w.set_export(e_v, shared_a, vec![own(0, n_sh)]);
    let _ = pool.build(&mut w, e_out, &mut SpikeRng::new(b"key select pool"));
    let _ = pb.build(&mut w, gfam, shared_a, false);
    let (arrays, pi, counts, tables, copies) = w.finish();
    let (c, inst, wit) = UniformCircuit::build_with_copies(
        "key-select", arrays, pi, counts, tables, copies, 14, 4, b"key select",
    );
    let mut prng = SpikeRng::new(b"key select challenges");
    let ch: Vec<Fp> = vec![prng.scalar(), prng.scalar()];
    let mut filled = wit.w.clone();
    let rel = c.instantiate(&c.shape, &ch, Some(&mut filled));
    let z = crate::r1cs::z_vector(&c.shape, &inst, &Witness { w: filled });
    let (az, bz, cz) = rel.abc(&c.shape, &z);
    (0..1usize << c.shape.sx).all(|x| az[x] * bz[x] == cz[x])
}

#[test]
fn five_key_slot_authenticates_archive_sent_exactly() {
    let k = kinds();
    let five = &k.keys[..];
    let four = &k.keys[..4];
    let corner = |kind: usize| crate::deferral::kind_corner(crate::deferral::MAX_KINDS, kind).unwrap();
    let archive = proved(&k, crate::full::ARCHIVE_SENT);
    let control = proved(&k, crate::full::CONTROL);
    let receive = proved(&k, crate::full::RECEIVE);

    // Installed corners select their own keys (Control's c0 c1 product with
    // the c2 term present, Receive's base key, ArchiveSent's fifth key).
    assert!(slot_holds(&k, five, &archive.0, &archive.1, &corner(4)));
    assert!(slot_holds(&k, five, &control.0, &control.1, &corner(3)));
    assert!(slot_holds(&k, five, &receive.0, &receive.1, &corner(0)));
    // Wrong kind, both ways.
    assert!(!slot_holds(&k, five, &archive.0, &archive.1, &corner(0)));
    assert!(!slot_holds(&k, five, &receive.0, &receive.1, &corner(4)));
    // Reserved corners and a non-Boolean c2.
    for bits in [[1, 0, 1], [0, 1, 1], [1, 1, 1], [0, 0, 2]] {
        assert!(
            !slot_holds(&k, five, &archive.0, &archive.1, &bits),
            "reserved or out-of-range kind words {bits:?}"
        );
    }
    // Fixed four-kind control: a four-key slot (two kind words) still selects
    // Control at (1, 1), and has no selection for the fifth key at all.
    assert!(slot_holds(&k, four, &control.0, &control.1, &[1, 1]));
    assert!(!slot_holds(&k, four, &control.0, &control.1, &[0, 1]));
    let uninstalled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        slot_holds(&k, four, &archive.0, &archive.1, &[0, 0])
    }));
    let why = uninstalled.expect_err("no four-key slot verifies the fifth key");
    let why = why
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| why.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default();
    assert!(why.contains("the proof's key is one of the slot's"), "{why}");
}
