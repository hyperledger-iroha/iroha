//! Review finding 4: a verified circuit proof's io fold leaves.
//!
//! The native verifier reads a circuit proof's io as its ordinary io, the
//! pair link's io and its late io, and zero at every other slot below the
//! io's length. In-circuit, the io digests cover the ordinary and late slots
//! and the link ties cover the link slots; every other leaf must be pinned to
//! zero. A free leaf lets the receiver change the io evaluation and so kappa:
//! a valid incoming pair's soft verdict turns 0 (a forced burn), and a hard
//! slot accepts a proof that native verification refuses.
use super::*;
use crate::r1cs::{Circuit, Instance, Witness};
use crate::spartan::{Link, Proof, prove_linked_circuit, verify_linked_circuit};
use crate::uniform::UniformCircuit;
use crate::verifier::{Trace, trace_circuit, trace_circuit_with_io};
use iroha_pasta::{Ep as Pallas, Eq as Vesta};

type Built<S> = (UniformCircuit<S>, Instance<S>, Witness<S>);

/// P: the binding chain (link slots 124-127) and a late io (the empty
/// generator claim at 160-167) over a four-word ordinary io. `extra` adds a
/// link io slot whose value is 1: the same relation and transcript, but a
/// proof over an io the native verifier of the plain program reads as 0.
fn p_program(values: &[u128], mu: &BigUint, extra: Option<usize>) -> Built<Fp> {
    let mut w = Wiring::<Fp>::default();
    let gfam = w.family(0, 1);
    p_binding(&mut w, gfam, mu, values);
    generator_integration::expose_empty::<Vesta>(&mut w, gfam, &crate::ipa::Gens::new(16));
    if let Some(slot) = extra {
        let a = w.push(Array {
            name: "test extra link io",
            family: gfam,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vec![Fp::ZERO]],
        });
        w.link.io.push((a, slot));
        let inner = w.link.io_values.take().expect("binding link io");
        w.link.io_values = Some(Box::new(move |rho: &[u8; 32], r: &[u8; 32]| {
            let mut v = inner(rho, r);
            v.push(Fp::ONE);
            v
        }));
    }
    let (arrays, pi, counts, tables, copies) = w.finish();
    UniformCircuit::build_with_copies(
        "io-binding-p",
        arrays,
        pi,
        counts,
        tables,
        copies,
        12,
        4,
        b"io binding p",
    )
}

/// S: the native binding chain (link slots 124 and 126) and a late io.
fn s_program(values: &[u128], mu: Fq) -> Built<Fq> {
    let mut w = Wiring::<Fq>::default();
    let gfam = w.family(0, 1);
    native_binding(&mut w, gfam, mu, values);
    generator_integration::expose_empty::<Pallas>(&mut w, gfam, &crate::ipa::Gens::new(16));
    let (arrays, pi, counts, tables, copies) = w.finish();
    UniformCircuit::build_with_copies(
        "io-binding-s",
        arrays,
        pi,
        counts,
        tables,
        copies,
        10,
        4,
        b"io binding s",
    )
}

struct Pair {
    pp: Proof<Vesta>,
    ps: Proof<Pallas>,
    d: [u8; 32],
    rho: [u8; 32],
    r: [u8; 32],
}

/// Proves a linked pair as the runner links them (P's first phase gives d, S
/// squeezes rho and ends at R, P proves again with rho and R).
fn prove_pair(
    p: &Built<Fp>,
    s: &Built<Fq>,
    gp: &crate::ipa::Gens<Vesta>,
    gs: &crate::ipa::Gens<Pallas>,
) -> Pair {
    let (cp, ip, wp) = p;
    let (cs, is, ws) = s;
    let rlc = |rho: &[u8; 32]| crate::verifier::rlc_value(rho, Fq::ZERO, &[]).to_repr();
    let rng_p = SpikeRng::new(b"io binding pair p");
    let first = Link::First {
        rho: [0; 32],
        r: [0; 32],
    };
    let (_, d, _) = prove_linked_circuit(&cp.shape, gp, ip, wp, &mut rng_p.clone(), &first, cp);
    let (ps, rho, r) = prove_linked_circuit(
        &cs.shape,
        gs,
        is,
        ws,
        &mut SpikeRng::new(b"io binding pair s"),
        &Link::Second { d, rlc: &rlc },
        cs,
    );
    let (pp, d2, _) = prove_linked_circuit(
        &cp.shape,
        gp,
        ip,
        wp,
        &mut rng_p.clone(),
        &Link::First { rho, r },
        cp,
    );
    assert_eq!(d, d2, "same first phase, same d");
    Pair { pp, ps, d, rho, r }
}

fn trace_p(
    p: &Built<Fp>,
    gp: &crate::ipa::Gens<Vesta>,
    pair: &Pair,
    extra: &[(usize, Fp)],
) -> Trace<Vesta> {
    let (cp, ip, _) = p;
    let fixed = crate::spartan::fixed_for(&cp.shape, gp, cp, &ip.io);
    let link = Link::First {
        rho: pair.rho,
        r: pair.r,
    };
    if extra.is_empty() {
        trace_circuit(&cp.shape, gp, ip, &pair.pp, &[], &link, cp, &fixed)
    } else {
        trace_circuit_with_io(&cp.shape, gp, ip, &pair.pp, &[], &link, cp, &fixed, extra)
    }
}

fn trace_s(
    s: &Built<Fq>,
    gs: &crate::ipa::Gens<Pallas>,
    pair: &Pair,
    extra: &[(usize, Fq)],
) -> Trace<Pallas> {
    let (cs, is, _) = s;
    let fixed = crate::spartan::fixed_for(&cs.shape, gs, cs, &is.io);
    let r = pair.r;
    let rlc = move |_: &[u8; 32]| r;
    let link = Link::Second {
        d: pair.d,
        rlc: &rlc,
    };
    if extra.is_empty() {
        trace_circuit(&cs.shape, gs, is, &pair.ps, &[], &link, cs, &fixed)
    } else {
        trace_circuit_with_io(&cs.shape, gs, is, &pair.ps, &[], &link, cs, &fixed, extra)
    }
}

/// One verified proof's scalar side as the complete builders place it
/// (`scalar_into`, `io_digests`, `link_ties`), with the pair's shared values
/// as a free array: in the full circuits they are bound only to the other
/// half and to the pair link, so the receiver chooses them.
#[allow(clippy::too_many_arguments)]
fn scalar_half<C: iroha_pasta::PastaCurve>(
    program: &Built<crate::ipa::F<C>>,
    proof: &Proof<C>,
    tr: &Trace<C>,
    values: &[u128],
    base: usize,
    offsets: (usize, usize, usize),
    split: bool,
) -> Built<crate::ipa::F<C>>
where
    crate::ipa::F<C>:
        Scalar + PoseidonField + ff::WithSmallOrderMulGroup<3> + zeroize::Zeroize,
{
    type S<C> = crate::ipa::F<C>;
    let (circuit, inst, _) = program;
    let mut w = Wiring::<S<C>>::default();
    let gfam = w.family(0, 1);
    let e_out = w.reserve();
    let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![S::<C>::ZERO]]);
    let mut pool = Pool::new(S::<C>::from(NODE_DOMAIN));
    let n = values.len().next_power_of_two();
    let mut sv: Vec<S<C>> = values.iter().map(|v| S::<C>::from_u128(*v)).collect();
    sv.resize(n, S::<C>::ZERO);
    let shared = w.push(Array {
        name: "test shared values",
        family: gfam,
        rho: n.trailing_zeros() as usize,
        r_count: values.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![128],
        rows: vec![],
        values: vec![sv],
    });
    let _ = endo_table(&mut w, gfam);
    let ct = tr.circuit.as_ref().expect("a circuit proof");
    let rel = circuit.instantiate(&circuit.shape, &ct.phase2, None);
    let evals = |rx: &[S<C>], ry: &[S<C>]| rel.matrix_evals(&circuit.shape, rx, ry);
    let sc = scalar_into::<C>(
        &mut w,
        gfam,
        shared,
        base,
        &circuit.shape,
        inst,
        proof,
        &[],
        tr,
        &evals,
    );
    let mut items = vec![IoDigest {
        io: &inst.io,
        limb: io_limb(tr, base),
        fold: sc.io,
        slots: (0..inst.io.len()).collect(),
    }];
    if ct.late_digest.is_some() {
        items.push(IoDigest {
            io: &proof.late,
            limb: late_limb(tr, &[], base),
            fold: sc.io,
            slots: ct.late_slots.clone(),
        });
    }
    assert_io_bound::<C>(inst, tr, &items, sc.io, split);
    io_digests(&mut w, &mut pool, gfam, e_out, e_zero, shared, &items);
    let host = w.push(Array {
        name: "test link tie host",
        family: gfam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: vec![],
        widths: vec![1],
        rows: vec![],
        values: vec![vec![S::<C>::ZERO]],
    });
    link_ties(&mut w, host, &[(true, sc.io)], &[offsets], shared, split);
    pool.build(&mut w, e_out, &mut SpikeRng::new(b"io binding pool"));
    let (arrays, pi, counts, tables, copies) = w.finish();
    UniformCircuit::build_with_copies(
        "io-binding-verifier",
        arrays,
        pi,
        counts,
        tables,
        copies,
        13,
        4,
        b"io binding verifier",
    )
}

/// The P proof's total points side (in S's field), as the Receive circuit's
/// incoming slot places it, over the pair's shared values: (holds, verdict).
fn total_points_p(
    p: &Built<Fp>,
    gp: &crate::ipa::Gens<Vesta>,
    proof: &Proof<Vesta>,
    tr: &Trace<Vesta>,
    values: &[u128],
    len_p: usize,
) -> (bool, Fq) {
    use crate::points::{PointsBuilder, SharedMap};
    let mut w = Wiring::<Fq>::default();
    let gfam = w.family(0, 1);
    let e_out = w.reserve();
    let e_v = w.reserve();
    let e_zero = constant_entries(&mut w, gfam, "zero word", &[vec![Fq::ZERO]]);
    let mut pool = Pool::new(Fq::from(NODE_DOMAIN));
    let mut pb = PointsBuilder::<Vesta>::new(&mut w, e_out, e_v, e_zero, OPERATION_IDENTITY);
    let token = pb.verify_keyed_total(
        &mut pool,
        &p.0.shape,
        gp,
        proof,
        &[],
        tr,
        SharedMap {
            base: 0,
            other: Some([len_p, len_p + 1]),
            r: Some([len_p - 2, len_p - 1]),
        },
        None,
        None,
    );
    let n = values.len().next_power_of_two();
    let mut sv: Vec<Fq> = values.iter().map(|v| Fq::from_u128(*v)).collect();
    sv.resize(n, Fq::ZERO);
    let shared = w.push(Array {
        name: "test shared values",
        family: gfam,
        rho: n.trailing_zeros() as usize,
        r_count: values.len(),
        fixed: false,
        lookups: vec![],
        widths: vec![128],
        rows: vec![],
        values: vec![sv],
    });
    w.set_export(e_v, shared, vec![own(0, n)]);
    pool.build(&mut w, e_out, &mut SpikeRng::new(b"io binding points pool"));
    let idx = pb.build(&mut w, gfam, shared, true);
    let (va, vc, vi) = token.validity(&idx);
    let verdict = w.arrays[va].values[vc][vi];
    let (arrays, pi, counts, tables, copies) = w.finish();
    let built = UniformCircuit::build_with_copies(
        "io-binding-points",
        arrays,
        pi,
        counts,
        tables,
        copies,
        13,
        4,
        b"io binding points",
    );
    (failing(&built).is_empty(), verdict)
}

/// The arrays owning the rows a witness leaves unsatisfied (empty: it holds).
fn failing<S: Scalar>((c, i, w): &Built<S>) -> Vec<String> {
    let mut values = w.w.clone();
    let rel = c.instantiate(&c.shape, &[S::from(31), S::from(47)], Some(&mut values));
    let z = crate::r1cs::z_vector(&c.shape, i, &Witness { w: values });
    let (a, b, cz) = rel.abc(&c.shape, &z);
    let mut owners: Vec<String> = (0..1usize << c.shape.sx)
        .filter(|x| a[*x] * b[*x] != cz[*x])
        .map(|x| {
            c.row_owner(x)
                .map_or("?".into(), |(a, _, _)| c.arrays[a].name.to_string())
        })
        .collect();
    owners.dedup();
    owners
}

/// Finding 4. The pair verifies natively; its scalar halves hold. For each
/// io slot that neither digest nor link tie covers, the verification of the
/// same valid proof with that leaf set to 1 (every later value recomputed, as
/// a receiver choosing the leaf would) is a different verification: native
/// verification of the valid pair accepts, the altered one refuses (a soft
/// slot's verdict 0, a forced burn). Its witness must not hold. A hard slot:
/// a proof made over an io with 1 at an unbound slot is refused natively,
/// accepted by the altered verification, and its witness must not hold.
#[test]
fn unbound_io_fold_leaves_are_pinned_in_both_halves() {
    let (_, _, values) = nonnative::sample_binding(20);
    let mu = binding_mu(&values);
    let fq = |x: &BigUint| Option::<Fq>::from(Fq::from_repr(nonnative::fq_bytes(x))).unwrap();
    let p = p_program(&values, &mu, None);
    let s = s_program(&values, fq(&mu));
    let gp = crate::ipa::Gens::<Vesta>::new(p.0.shape.cols());
    let gs = crate::ipa::Gens::<Pallas>::new(s.0.shape.cols());
    let pair = prove_pair(&p, &s, &gp, &gs);
    assert!(
        verify_linked_circuit(
            &p.0.shape,
            &gp,
            &p.1,
            &pair.pp.to_bytes(),
            &Link::First {
                rho: pair.rho,
                r: pair.r
            },
            &p.0,
        )
        .is_ok(),
        "the valid P proof verifies natively"
    );
    let tr_p = trace_p(&p, &gp, &pair, &[]);
    let tr_s = trace_s(&s, &gs, &pair, &[]);
    assert!(tr_p.accepts && tr_s.accepts, "the pair verifies natively");
    let len_p = crate::verifier::shared_values(&tr_p, &pair.pp, &[]).len();
    let offsets = (0, len_p, len_p);
    let both = |tp: &Trace<Vesta>, ts: &Trace<Pallas>, pr: &Pair| -> Vec<u128> {
        let mut v = crate::verifier::shared_values(tp, &pr.pp, &[]);
        assert_eq!(v.len(), len_p);
        v.extend(crate::verifier::shared_values(ts, &pr.ps, &[]));
        v
    };
    let half_p = |tp: &Trace<Vesta>, ts: &Trace<Pallas>, pr: &Pair| {
        scalar_half::<Vesta>(&p, &pr.pp, tp, &both(tp, ts, pr), 0, offsets, true)
    };
    let half_s = |tp: &Trace<Vesta>, ts: &Trace<Pallas>, pr: &Pair| {
        scalar_half::<Pallas>(&s, &pr.ps, ts, &both(tp, ts, pr), len_p, offsets, false)
    };
    // The layouts: P binds 0-3, 124-127, 160-167; S binds 0-3, 124, 126,
    // 160-167.
    let ct_p = tr_p.circuit.as_ref().unwrap();
    let ct_s = tr_s.circuit.as_ref().unwrap();
    assert_eq!((ct_p.io_full.len(), ct_s.io_full.len()), (168, 168));
    let late: Vec<usize> = (160..168).collect();
    let bound_p: Vec<usize> = (0..4).chain(124..128).chain(late.clone()).collect();
    let bound_s: Vec<usize> = (0..4).chain([124, 126]).chain(late).collect();
    assert_eq!(crate::scalar::bound_io_slots(&p.1, &tr_p), bound_p);
    assert_eq!(crate::scalar::bound_io_slots(&s.1, &tr_s), bound_s);
    let honest_p = half_p(&tr_p, &tr_s, &pair);
    assert_eq!(failing(&honest_p), Vec::<String>::new(), "honest P half");
    let layout_p = honest_p.0.layout();
    drop(honest_p);
    let honest_s = half_s(&tr_p, &tr_s, &pair);
    assert_eq!(failing(&honest_s), Vec::<String>::new(), "honest S half");
    let layout_s = honest_s.0.layout();
    drop(honest_s);
    // The incoming slot's other half, the P proof's total points side: the
    // valid pair's verdict is 1; with leaf 64 set to 1 it holds with verdict
    // 0. Only the io fold's pinning tells the two apart.
    assert_eq!(
        total_points_p(&p, &gp, &pair.pp, &tr_p, &both(&tr_p, &tr_s, &pair), len_p),
        (true, Fq::ONE),
        "the valid pair's total verdict"
    );
    let altered = trace_p(&p, &gp, &pair, &[(64, Fp::ONE)]);
    assert_eq!(
        total_points_p(&p, &gp, &pair.pp, &altered, &both(&altered, &tr_s, &pair), len_p),
        (true, Fq::ZERO),
        "the altered verification's points half holds with verdict 0"
    );
    // A soft slot: the valid pair with one unbound leaf set to 1.
    let mut refused: Vec<(String, Vec<String>)> = Vec::new();
    for slot in [4, 64, 123, 128, 159] {
        let altered = trace_p(&p, &gp, &pair, &[(slot, Fp::ONE)]);
        assert!(!altered.accepts, "slot {slot}: the altered verification refuses");
        let half = half_p(&altered, &tr_s, &pair);
        assert_eq!(half.0.layout(), layout_p, "the same circuit");
        let rows = failing(&half);
        eprintln!("P slot {slot}: unsatisfied rows in {rows:?}");
        refused.push((format!("P slot {slot}"), rows));
    }
    for slot in [4, 123, 125, 127, 159] {
        let altered = trace_s(&s, &gs, &pair, &[(slot, Fq::ONE)]);
        assert!(!altered.accepts, "slot {slot}: the altered verification refuses");
        let half = half_s(&tr_p, &altered, &pair);
        assert_eq!(half.0.layout(), layout_s, "the same circuit");
        let rows = failing(&half);
        eprintln!("S slot {slot}: unsatisfied rows in {rows:?}");
        refused.push((format!("S slot {slot}"), rows));
    }
    // A hard slot: a proof over an io with 1 at unbound slot 100.
    let p_extra = p_program(&values, &mu, Some(100));
    assert_eq!(
        format!("{:?}", p_extra.0.shape),
        format!("{:?}", p.0.shape),
        "the same shape and transcript"
    );
    assert_eq!(p_extra.0.layout(), p.0.layout(), "the same layout");
    let forged = prove_pair(&p_extra, &s, &gp, &gs);
    assert!(
        verify_linked_circuit(
            &p.0.shape,
            &gp,
            &p.1,
            &forged.pp.to_bytes(),
            &Link::First {
                rho: forged.rho,
                r: forged.r
            },
            &p.0,
        )
        .is_err(),
        "native verification of the program refuses the proof"
    );
    let natively_read = trace_p(&p, &gp, &forged, &[]);
    assert!(!natively_read.accepts);
    let altered = trace_p(&p, &gp, &forged, &[(100, Fp::ONE)]);
    assert!(
        altered.accepts,
        "the verification with leaf 100 set to 1 accepts it"
    );
    let forged_s = trace_s(&s, &gs, &forged, &[]);
    assert!(forged_s.accepts);
    let half = half_p(&altered, &forged_s, &forged);
    assert_eq!(half.0.layout(), layout_p, "the same circuit");
    let rows = failing(&half);
    eprintln!("hard P slot 100: unsatisfied rows in {rows:?}");
    refused.push(("hard P slot 100".into(), rows));
    for (case, rows) in &refused {
        assert!(
            rows.iter().any(|a| a == "scalar io fold"),
            "{case}: the unbound leaf is refused by the io fold, not left free ({rows:?})"
        );
    }
}
