//! A later generation's fifth kind, ArchiveSent, on a third selector bit.
//! These component programs exercise the selector geometry, the any-kind
//! slot's selector-word rows (`reserved_corner_rows`) and the native matrix
//! decision. They are not monetary parameter manifests: no generation is
//! built, signed or selected here.

use super::*;
use crate::{
    fs::SpikeRng,
    native::engine::matrix_accumulator,
    r1cs::{Circuit, Instance, Witness},
    uniform::{UniformCircuit, constant, link, plus, scaled},
};
use ff::Field;
use iroha_pasta::{Eq as Vesta, Fp};

/// Toy kinds of one shape (an any-kind slot needs the kinds' padding to
/// agree) with distinct matrices and LogUp-challenge terms.
fn kind(index: usize) -> Kind<Fp> {
    Kind {
        sx: 5,
        sz: 4,
        deg: (1, 1),
        eval: Box::new(move |ch: &[Fp], rx: &[Fp], ry: &[Fp]| {
            let x = rx.iter().copied().sum::<Fp>();
            let y = ry.iter().copied().sum::<Fp>();
            let tag = Fp::from((index + 1) as u64);
            (
                tag + x + ch[0] * y,
                tag.double() + x * y + rx[0],
                tag * tag + ch[1] * x + y * ry[0],
            )
        }),
        eval_many: None,
    }
}

/// The four-kind profile and its five-kind extension over the same kinds.
fn polys() -> (MatPoly<Fp>, MatPoly<Fp>) {
    (
        MatPoly::with_min_vec((0..4).map(kind).collect(), 5, 4),
        MatPoly::with_min_vec((0..MAX_KINDS).map(kind).collect(), 5, 4),
    )
}

/// A verified proof's fresh claim of kind `k`: its point and its own matrix
/// value `r A + r^2 B + r^3 C`.
fn fresh(poly: &MatPoly<Fp>, k: usize, rng: &mut SpikeRng) -> Claim<Fp> {
    let kk = &poly.kinds[k];
    let rx: Vec<Fp> = (0..kk.sx).map(|_| rng.scalar()).collect();
    let ry: Vec<Fp> = (0..kk.sz).map(|_| rng.scalar()).collect();
    let ch = [rng.scalar(), rng.scalar()];
    let r: Fp = rng.scalar();
    let (a, b, c) = (kk.eval)(&ch, &rx, &ry);
    Claim {
        point: poly.fresh_point(k, r, &ch, &rx, &ry),
        value: r * a + r.square() * b + r.square() * r * c,
    }
}

fn random(poly: &MatPoly<Fp>, rng: &mut SpikeRng) -> Claim<Fp> {
    let point: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
    Claim {
        value: poly.eval(&point),
        point,
    }
}

/// The honest q, bent through every claim false on `poly` (as a prover must
/// to satisfy the fold rows): the fold then holds and only the native
/// decision of the folded claim can refuse it.
fn bent(poly: &MatPoly<Fp>, claims: &[Claim<Fp>; 4]) -> Vec<Fp> {
    let mut q = restrict(poly, claims);
    let l = lagrange4::<Fp>();
    for (t, cl) in claims.iter().enumerate() {
        let off = cl.value - horner(&q, Fp::from(t as u64));
        for (e, c) in l[t].iter().enumerate() {
            q[e] += off * c;
        }
    }
    q
}

#[test]
fn five_kinds_select_installed_corners_and_keep_the_four_kind_profile() {
    let (four, five) = polys();
    let mut rng = SpikeRng::new(b"five kind selector geometry");
    assert_eq!((five.selector_bits(), five.selector_degree()), (3, 2));
    assert_eq!((four.selector_bits(), four.selector_degree()), (2, 2));
    assert_eq!(five.n(), four.n() + 1, "c2 is one more coordinate");
    assert_eq!(five.c1_at(), four.c1_at(), "c1 stays after y");
    assert_eq!((four.c2_at(), five.c2_at()), (None, Some(five.n() - 1)));
    assert_eq!(five.selector_coords(), vec![0, four.n() - 1, five.n() - 1]);
    assert_eq!(
        five.degree(),
        four.degree(),
        "the third selector bit adds a coordinate, not fold degree"
    );
    let mut claims = Vec::new();
    for k in 0..MAX_KINDS {
        let mut r2 = SpikeRng::new(format!("fresh {k}").as_bytes());
        let cl = fresh(&five, k, &mut r2);
        assert_eq!(five.eval(&cl.point), cl.value, "kind {k}'s corner selects it alone");
        let corner = kind_corner(MAX_KINDS, k).unwrap();
        for (j, bit) in five.selector_coords().into_iter().zip(corner) {
            assert_eq!(cl.point[j], Fp::from(bit), "kind {k} selector coordinate {j}");
        }
        assert_eq!(corner_kind(MAX_KINDS, corner.map(Fp::from)), Ok(k));
        for (j, f) in five.fixed_coords(k).into_iter().enumerate() {
            if let Some(v) = f {
                assert_eq!(cl.point[j], v, "kind {k} fixed coordinate {j}");
            }
        }
        if k < 4 {
            // Fixed four-kind control: the same point with c2 = 0 appended,
            // the same value, the same constants, the same two bits.
            let mut r4 = SpikeRng::new(format!("fresh {k}").as_bytes());
            let c4 = fresh(&four, k, &mut r4);
            assert_eq!(&cl.point[..four.n()], &c4.point[..]);
            assert_eq!((cl.point[four.n()], cl.value), (Fp::ZERO, c4.value));
            assert_eq!(four.eval(&c4.point), c4.value);
            let mut f4 = four.fixed_coords(k);
            f4.push(Some(Fp::ZERO));
            assert_eq!(five.fixed_coords(k), f4);
            let (b0, b1) = kind_bits(k);
            assert_eq!(kind_corner(4, k), Ok([b0, b1, 0]));
            assert_eq!(corner_kind(4, corner.map(Fp::from)), Ok(k));
        }
        claims.push(cl);
    }
    // At c2 = 0 the five-kind polynomial is exactly the four-kind one.
    for _ in 0..6 {
        let p4: Vec<Fp> = (0..four.n()).map(|_| rng.scalar()).collect();
        let mut p5 = p4.clone();
        p5.push(Fp::ZERO);
        assert_eq!(five.eval(&p5), four.eval(&p4));
    }
    let (e4, e5) = (four.empty_claim(), five.empty_claim());
    let mut embedded = e4.point.clone();
    embedded.push(Fp::ZERO);
    assert_eq!(e5.point, embedded, "the empty claim keeps its four-kind corner");
    assert_eq!((five.eval(&e5.point), e5.value), (Fp::ZERO, Fp::ZERO));
    // The reserved corners are not empty points of P (ArchiveSent's and a
    // two-bit kind's terms alias there), so fresh claims must be kept off
    // them: the any-kind slot's rows and the native decode refuse them.
    for bits in [[1u64, 0, 1], [0, 1, 1], [1, 1, 1]] {
        let mut reserved = claims[ARCHIVE_SENT_KIND].point.clone();
        for (j, b) in five.selector_coords().into_iter().zip(bits) {
            reserved[j] = Fp::from(b);
        }
        assert_ne!(five.eval(&reserved), claims[ARCHIVE_SENT_KIND].value);
        assert_eq!(
            corner_kind(MAX_KINDS, bits.map(Fp::from)),
            Err(SelectorError::Uninstalled)
        );
    }
    assert_eq!(
        corner_kind(MAX_KINDS, [Fp::ZERO, Fp::ZERO, Fp::from(2)]),
        Err(SelectorError::OutOfRange)
    );
    assert_eq!(
        corner_kind(4, [Fp::ZERO, Fp::ZERO, Fp::ONE]),
        Err(SelectorError::Uninstalled),
        "the four-kind profile has no ArchiveSent corner"
    );
    assert_eq!(kind_corner(4, ARCHIVE_SENT_KIND), Err(SelectorError::Uninstalled));
    assert_eq!(kind_corner(MAX_KINDS, 5), Err(SelectorError::Uninstalled));
    assert_eq!(kind_corner(MAX_KINDS, 8), Err(SelectorError::OutOfRange));
    assert_eq!(kind_corner(6, 0), Err(SelectorError::Profile));
    assert_eq!(corner_kind(1, [Fp::ZERO; 3]), Err(SelectorError::Profile));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            four.fresh_point(ARCHIVE_SENT_KIND, Fp::ONE, &[], &[Fp::ZERO; 5], &[Fp::ZERO; 4])
        }))
        .is_err(),
        "a four-kind polynomial cannot place an ArchiveSent claim"
    );
    // A fold through ArchiveSent's fresh claim, the empty claim, a random
    // accumulator and Control's fresh claim: q at the four-kind degree is P
    // along the curve, and the folded claim decides true natively.
    let cl = [
        claims[ARCHIVE_SENT_KIND].clone(),
        e5,
        random(&five, &mut rng),
        claims[3].clone(),
    ];
    let q = restrict(&five, &cl);
    assert_eq!(q.len(), five.degree() + 1);
    assert_eq!(q, restrict_pointwise(&five, &cl), "batched = pointwise");
    for (t, c) in cl.iter().enumerate() {
        assert_eq!(horner(&q, Fp::from(t as u64)), c.value, "q passes claim {t}");
    }
    let tau: Fp = rng.scalar();
    assert_eq!(
        horner(&q, tau),
        five.eval(&curve(&cl, tau)),
        "the declared degree bounds P along the curve"
    );
    let out = fold_output(&cl, &q, rng.scalar());
    let mut late = out.point.clone();
    late.push(out.value);
    assert!(matrix_accumulator(&five, &late));
    assert!(
        !matrix_accumulator(&four, &late),
        "the signed four-kind decision refuses a five-kind accumulator"
    );
}

/// The kind words' public io offset in the component fold program.
const WORDS_IO: usize = 8;

/// A fold whose claim 1 is an any-kind slot's fresh claim: its selector
/// coordinates are public kind words (as an any-kind slot reads them from the
/// verified proof's io), held Boolean and restricted to installed corners by
/// `reserved_corner_rows`; its other constant coordinates are the kinds'
/// common ones. Claim 3 is Control's constant-kind fresh claim.
fn slot_fold(
    poly: &MatPoly<Fp>,
    claims: &[Claim<Fp>; 4],
    q: &[Fp],
    words: [u64; 3],
) -> (UniformCircuit<Fp>, Instance<Fp>, Witness<Fp>, FoldArrays) {
    let one = Fp::ONE;
    let mut w: Wiring<Fp> = Wiring::default();
    let fam = w.family(0, 1);
    let fold = fold_arrays(&mut w, fam, claims, q);
    fix_coords(&mut w, &fold, 3, &poly.fixed_coords(3));
    let io = w.push(Array {
        name: "verified kind words",
        family: fam,
        rho: 2,
        r_count: 3,
        fixed: false,
        lookups: Vec::new(),
        widths: vec![255],
        rows: Vec::new(),
        values: vec![vec![Fp::from(words[0]), Fp::from(words[1]), Fp::from(words[2]), Fp::ZERO]],
    });
    w.io_arrays.push((io, WORDS_IO));
    let sel = poly.selector_coords();
    let word = |b: usize| link(io, 0, vec![Some(b)], one);
    let lins: Vec<_> = (0..sel.len()).map(word).collect();
    let mut rows: Vec<[crate::uniform::Lin<Fp>; 3]> = lins
        .iter()
        .map(|l| [l.clone(), plus(&constant(vec![one]), &scaled(l, -one)), Vec::new()])
        .collect();
    rows.extend(reserved_corner_rows(poly.kinds.len(), &lins));
    w.push(Array {
        name: "selector word rows",
        family: fam,
        rho: 0,
        r_count: 1,
        fixed: false,
        lookups: Vec::new(),
        widths: Vec::new(),
        rows,
        values: Vec::new(),
    });
    let mut fixed = poly.fixed_coords(0);
    for k in 1..poly.kinds.len() {
        let f = poly.fixed_coords(k);
        assert!(
            (0..poly.n())
                .filter(|j| !sel.contains(j))
                .all(|j| f[j] == fixed[j]),
            "a slot of any kind: the kinds' padding agrees"
        );
    }
    for j in &sel {
        fixed[*j] = None;
    }
    fix_coords(&mut w, &fold, 1, &fixed);
    let nc = 1usize << w.arrays[fold.coords].rho;
    let on: Vec<Fp> = (0..nc)
        .map(|j| if sel.contains(&j) { one } else { Fp::ZERO })
        .collect();
    let row = plus(
        &vec![crate::uniform::Term {
            src: crate::uniform::Src::Own(C_P[1]),
            coef: on.into(),
        }],
        &link(
            io,
            0,
            (0..nc).map(|j| sel.iter().position(|s| *s == j)).collect(),
            -one,
        ),
    );
    w.arrays[fold.coords]
        .rows
        .push([row, constant(vec![one; nc]), Vec::new()]);
    expose(&mut w, fam, &fold, 32);
    let (arrays, pi, counts, tables, copies) = w.finish();
    let (c, inst, wit) = UniformCircuit::build_with_copies(
        "archive-sent-slot-fold",
        arrays,
        pi,
        counts,
        tables,
        copies,
        10,
        16,
        b"archive-sent-slot-fold",
    );
    (c, inst, wit, fold)
}

fn holds(c: &UniformCircuit<Fp>, inst: &Instance<Fp>, wit: &Witness<Fp>, ch: &[Fp]) -> bool {
    let mut w = wit.w.clone();
    let rel = c.instantiate(&c.shape, ch, Some(&mut w));
    let io = crate::spartan::io_with_late(&inst.io, &c.late_slots(), &c.late_values(&w));
    let z = crate::r1cs::z_vector(&c.shape, &Instance { io }, &Witness { w });
    let (az, bz, cz) = rel.abc(&c.shape, &z);
    (0..1usize << c.shape.sx).all(|x| az[x] * bz[x] == cz[x])
}

/// Proves and verifies the fold program, then decides its exported matrix
/// claim natively: (verified, native decision on `poly`, the late io).
fn prove_and_decide(
    c: &UniformCircuit<Fp>,
    inst: &Instance<Fp>,
    wit: &Witness<Fp>,
    poly: &MatPoly<Fp>,
) -> (bool, bool, Vec<Fp>) {
    let gens = crate::ipa::Gens::<Vesta>::new(c.shape.cols());
    let mut rng = SpikeRng::new(b"archive sent slot proof");
    let (proof, _) = crate::spartan::prove_circuit(&c.shape, &gens, inst, wit, &mut rng, c);
    let verified = crate::spartan::verify_circuit(&c.shape, &gens, inst, &proof.to_bytes(), c).is_ok();
    (verified, matrix_accumulator(poly, &proof.late), proof.late.clone())
}

#[test]
fn archive_sent_slot_words_fold_and_native_decision_select_only_installed_corners() {
    let (four, five) = polys();
    let mut rng = SpikeRng::new(b"archive sent slot fold");
    let ch = [rng.scalar(), rng.scalar()];
    let archive = fresh(&five, ARCHIVE_SENT_KIND, &mut rng);
    let control = fresh(&five, 3, &mut rng);
    let (acc0, acc1) = (random(&five, &mut rng), random(&five, &mut rng));
    let with_slot = |slot: Claim<Fp>| [acc0.clone(), slot, acc1.clone(), control.clone()];
    let place = |bits: [u64; 3]| {
        let mut point = archive.point.clone();
        for (j, b) in five.selector_coords().into_iter().zip(bits) {
            point[j] = Fp::from(b);
        }
        point
    };

    // Genuine: ArchiveSent's proof with its own corner (0, 0, 1).
    let genuine = with_slot(archive.clone());
    let q = restrict(&five, &genuine);
    let (c, inst, wit, fold) = slot_fold(&five, &genuine, &q, [0, 0, 1]);
    assert!(holds(&c, &inst, &wit, &ch), "ArchiveSent's corner holds");
    let mut filled = wit.w.clone();
    let _ = c.instantiate(&c.shape, &ch, Some(&mut filled));
    let output = read_output(&c, &fold, &filled);
    assert_eq!(output, fold_output(&genuine, &q, ch[1]));
    assert_eq!(five.eval(&output.point), output.value);
    let (verified, decided, late) = prove_and_decide(&c, &inst, &wit, &five);
    assert!(verified && decided, "genuine ArchiveSent: proof verifies, native accepts");
    // Uninstalled: the signed four-kind decision has no ArchiveSent term.
    assert!(!matrix_accumulator(&four, &late));
    let program = c.program_digest();

    // Wrong kind: ArchiveSent's own matrix value under Receive's words. The
    // fold rows hold (q bent through the false claim) and the same program
    // proves it; the native matrix decision refuses the folded claim.
    let wrong = with_slot(Claim {
        point: place([0, 0, 0]),
        value: archive.value,
    });
    assert_ne!(five.eval(&wrong[1].point), wrong[1].value);
    let q = bent(&five, &wrong);
    let (c, inst, wit, _) = slot_fold(&five, &wrong, &q, [0, 0, 0]);
    assert_eq!(c.program_digest(), program, "one program for every kind word");
    assert!(holds(&c, &inst, &wit, &ch), "the fold itself holds");
    let (verified, decided, _) = prove_and_decide(&c, &inst, &wit, &five);
    assert!(verified && !decided, "wrong kind: native decision refuses");
    // Wrong kind the other way: Receive's corner value under ArchiveSent's word.
    let receive = fresh(&five, 0, &mut rng);
    let mut swapped = receive.clone();
    for (j, b) in five.selector_coords().into_iter().zip([0u64, 0, 1]) {
        swapped.point[j] = Fp::from(b);
    }
    let wrong = with_slot(swapped);
    let q = bent(&five, &wrong);
    let (c, inst, wit, _) = slot_fold(&five, &wrong, &q, [0, 0, 1]);
    assert!(holds(&c, &inst, &wit, &ch));
    let (verified, decided, _) = prove_and_decide(&c, &inst, &wit, &five);
    assert!(verified && !decided, "Receive's value at ArchiveSent's corner is refused");

    // Reserved corners: refused by the slot rows even when the claim is true
    // on P there (no prover witness exists).
    for bits in [[1u64, 0, 1], [0, 1, 1], [1, 1, 1]] {
        let point = place(bits);
        let claims = with_slot(Claim {
            value: five.eval(&point),
            point,
        });
        let q = restrict(&five, &claims);
        let (c, inst, wit, _) = slot_fold(&five, &claims, &q, bits);
        assert_eq!(c.program_digest(), program);
        assert!(!holds(&c, &inst, &wit, &ch), "reserved corner {bits:?} refused");
    }
    // Out of range: a non-Boolean c2 word.
    let point = place([0, 0, 2]);
    let claims = with_slot(Claim {
        value: five.eval(&point),
        point,
    });
    let q = restrict(&five, &claims);
    let (c, inst, wit, _) = slot_fold(&five, &claims, &q, [0, 0, 2]);
    assert!(!holds(&c, &inst, &wit, &ch), "non-Boolean c2 refused");
    // Every installed corner holds in the same program.
    for k in 0..MAX_KINDS {
        let mut r2 = SpikeRng::new(format!("installed slot {k}").as_bytes());
        let claims = with_slot(fresh(&five, k, &mut r2));
        let q = restrict(&five, &claims);
        let (c, inst, wit, _) =
            slot_fold(&five, &claims, &q, kind_corner(MAX_KINDS, k).unwrap());
        assert_eq!(c.program_digest(), program);
        assert!(holds(&c, &inst, &wit, &ch), "installed kind {k} holds");
    }
}
