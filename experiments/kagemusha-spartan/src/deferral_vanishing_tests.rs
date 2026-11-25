//! A kind whose term vanishes is not evaluated, and nothing else changes.
//! A five-kind fold whose claims all keep `c2 = 0` (a history without
//! ArchiveSent) gives the same `q` without running ArchiveSent's matrices,
//! and an evaluation at a fresh claim's corner runs only that claim's kind.
//! Every result equals a reference that evaluates every kind everywhere.

use super::*;
use crate::fs::SpikeRng;
use ff::Field;
use iroha_pasta::Fp;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Five kinds of distinct dimensions (so padding coordinates exist) and
/// matrices, each counting its evaluations.
fn counted_five_kinds() -> (MatPoly<Fp>, Vec<Arc<AtomicUsize>>) {
    let calls: Vec<Arc<AtomicUsize>> = (0..MAX_KINDS).map(|_| Arc::default()).collect();
    let kinds = (0..MAX_KINDS)
        .map(|index| {
            let count = calls[index].clone();
            Kind {
                sx: 4 + index % 3,
                sz: 3 + index % 2,
                deg: (1, 1),
                eval: Box::new(move |ch: &[Fp], rx: &[Fp], ry: &[Fp]| {
                    count.fetch_add(1, Ordering::Relaxed);
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
        })
        .collect();
    (MatPoly::with_min_vec(kinds, 0, 0), calls)
}

fn counts(calls: &[Arc<AtomicUsize>]) -> Vec<usize> {
    calls.iter().map(|c| c.swap(0, Ordering::Relaxed)).collect()
}

/// Every kind's term at `point`, each kind evaluated.
fn reference_eval(poly: &MatPoly<Fp>, point: &[Fp]) -> Fp {
    (0..poly.kinds.len())
        .map(|k| {
            let (ch, rx, ry) = poly.kind_inputs(k, point);
            poly.kind_term(k, point, (poly.kinds[k].eval)(ch, rx, ry))
        })
        .sum()
}

/// `restrict` from every kind evaluated at every point of the curve.
fn reference_q(poly: &MatPoly<Fp>, claims: &[Claim<Fp>; 4]) -> Vec<Fp> {
    let values: Vec<Fp> = (0..=poly.degree())
        .map(|t| reference_eval(poly, &curve(claims, Fp::from(t as u64))))
        .collect();
    interpolate(&values)
}

fn fresh(poly: &MatPoly<Fp>, rng: &mut SpikeRng, kind: usize) -> Claim<Fp> {
    let k = &poly.kinds[kind];
    let ch: Vec<Fp> = (0..2).map(|_| rng.scalar()).collect();
    let rx: Vec<Fp> = (0..k.sx).map(|_| rng.scalar()).collect();
    let ry: Vec<Fp> = (0..k.sz).map(|_| rng.scalar()).collect();
    Claim {
        point: poly.fresh_point(kind, rng.scalar(), &ch, &rx, &ry),
        value: Fp::ZERO,
    }
}

/// A folded accumulator: every coordinate random, `c2` zero unless the
/// history it folds had an ArchiveSent.
fn inherited(poly: &MatPoly<Fp>, rng: &mut SpikeRng, archive: bool) -> Claim<Fp> {
    let mut point: Vec<Fp> = (0..poly.n()).map(|_| rng.scalar()).collect();
    if !archive {
        point[poly.c2_at().expect("five kinds")] = Fp::ZERO;
    }
    Claim {
        point,
        value: Fp::ZERO,
    }
}

#[test]
fn a_fold_without_archive_sent_skips_its_matrices_and_keeps_q() {
    let (poly, calls) = counted_five_kinds();
    let mut rng = SpikeRng::new(b"vanishing kinds: fold");
    for (predecessor, incoming) in [(1, 0), (3, 1), (2, 1), (0, 1)] {
        let claims = [
            inherited(&poly, &mut rng, false),
            fresh(&poly, &mut rng, predecessor),
            inherited(&poly, &mut rng, false),
            fresh(&poly, &mut rng, incoming),
        ];
        let q = restrict(&poly, &claims);
        let skipped = counts(&calls);
        assert_eq!(q, reference_q(&poly, &claims), "the same q");
        assert_eq!(
            skipped[ARCHIVE_SENT_KIND], 0,
            "ArchiveSent's matrices are not run"
        );
        assert!(
            skipped[..ARCHIVE_SENT_KIND].iter().all(|c| *c > 0),
            "the other kinds are: {skipped:?}"
        );
        assert_eq!(restrict_pointwise(&poly, &claims), q, "pointwise agrees");
        counts(&calls);
    }
}

#[test]
fn a_fold_over_an_archive_sent_history_evaluates_every_kind() {
    let (poly, calls) = counted_five_kinds();
    let mut rng = SpikeRng::new(b"vanishing kinds: archive history");
    // An accumulator from a history with ArchiveSent in any slot, or a
    // fresh ArchiveSent claim, puts c2 off zero somewhere on the curve.
    for slot in 0..4 {
        let mut claims = [
            inherited(&poly, &mut rng, false),
            fresh(&poly, &mut rng, 1),
            inherited(&poly, &mut rng, false),
            fresh(&poly, &mut rng, 0),
        ];
        claims[slot] = if slot % 2 == 0 {
            inherited(&poly, &mut rng, true)
        } else {
            fresh(&poly, &mut rng, ARCHIVE_SENT_KIND)
        };
        let q = restrict(&poly, &claims);
        let evaluated = counts(&calls);
        assert_eq!(q, reference_q(&poly, &claims), "the same q");
        assert!(
            evaluated.iter().all(|c| *c > 0),
            "every kind: {evaluated:?}"
        );
        counts(&calls);
    }
}

#[test]
fn a_fresh_corner_evaluates_only_its_kind() {
    let (poly, calls) = counted_five_kinds();
    let mut rng = SpikeRng::new(b"vanishing kinds: corners");
    for kind in 0..MAX_KINDS {
        let claim = fresh(&poly, &mut rng, kind);
        let value = poly.eval(&claim.point);
        let evaluated = counts(&calls);
        assert_eq!(
            value,
            reference_eval(&poly, &claim.point),
            "kind {kind}: the same value"
        );
        let mut expected = vec![0; MAX_KINDS];
        expected[kind] = 1;
        assert_eq!(evaluated, expected, "kind {kind}: only its own matrices");
        counts(&calls);
    }
    // A random point selects every kind, and the batched form agrees.
    let points: Vec<Vec<Fp>> = (0..6)
        .map(|i| inherited(&poly, &mut rng, i % 2 == 0).point)
        .collect();
    let many = poly.eval_many(&points);
    for (point, value) in points.iter().zip(many) {
        assert_eq!(value, reference_eval(&poly, point));
        assert_eq!(value, poly.eval(point));
    }
}
