//! Points/transcript half of private exact generator restructuring.
use super::*;
use crate::generator_claim::{Fold, SOURCE_COUNT, Trace as FoldTrace};

pub struct GeneratorPoints {
    sources: Vec<usize>,
    output: usize,
}
/// Iff comparison of an exact corrected claim with its authenticated original.
pub struct GeneratorEquality {
    verdict: usize,
}
impl GeneratorEquality {
    pub fn validity(&self, built: &PointsIdx) -> (usize, usize, usize) {
        (
            built.validity.expect("generator equality verdict array"),
            0,
            self.verdict,
        )
    }
}
impl GeneratorPoints {
    pub fn sources(&self, built: &PointsIdx) -> Vec<[(usize, usize, usize); 2]> {
        self.sources
            .iter()
            .map(|i| [(built.points, 0, *i), (built.points, 1, *i)])
            .collect()
    }
    pub fn output(&self, built: &PointsIdx) -> [(usize, usize, usize); 2] {
        [
            (built.points, 0, self.output),
            (built.points, 1, self.output),
        ]
    }
}
impl<C: PastaCurve> PointsBuilder<C>
where
    C::Base: Scalar + PoseidonField,
{
    /// Compare a corrected source with the proof's exact original g_final.
    pub fn generator_source_equals_proof(
        &mut self,
        fold: &GeneratorPoints,
        source: usize,
        original: &TotalPoints,
    ) -> GeneratorEquality {
        self.generator_source_equals_input(
            fold.sources[source],
            original.wire_nodes[original.wire_nodes.len() - 2],
        )
    }
    /// Compare a corrected source with an inherited point authenticated by the
    /// preceding proof's late-IO coordinate limbs. These shared indices must be
    /// linked to that proof's exact IO cells by the enclosing circuit.
    pub fn generator_source_equals_inherited(
        &mut self,
        fold: &GeneratorPoints,
        source: usize,
        original: Aff<C>,
        limbs: [usize; 4],
    ) -> GeneratorEquality {
        let i = self.inputs.len();
        let (x, y) = coords(&C::from(original));
        self.inputs.push((x, y, None));
        self.claim_limbs.push((i, limbs));
        self.generator_source_equals_input(fold.sources[source], i)
    }
    fn generator_source_equals_input(&mut self, left: usize, right: usize) -> GeneratorEquality {
        self.total = true;
        let zero_i = self.inputs.len();
        self.inputs.push((C::Base::ZERO, C::Base::ZERO, None));
        self.consts.push((zero_i, None));
        let zero = (PtRef::Input(zero_i), C::identity());
        let point = |me: &Self, i: usize| {
            let (x, y, _) = me.inputs[i];
            let p = if x == C::Base::ZERO && y == C::Base::ZERO {
                C::identity()
            } else {
                C::from(
                    Option::<Aff<C>>::from(<Aff<C> as iroha_pasta::PastaAffine>::from_xy(x, y))
                        .expect("source point"),
                )
            };
            (PtRef::Input(i), p)
        };
        let l = self.add(point(self, left), zero, false);
        let r = self.add(point(self, right), zero, false);
        let (PtRef::Add(l), PtRef::Add(r)) = (l.0, r.0) else {
            unreachable!()
        };
        let verdict = self.soft_equal.len();
        self.soft_equal.push((l, r, None));
        GeneratorEquality { verdict }
    }
    fn fold_sp(&mut self, header: ([C::Base; 3], usize)) -> Sp<C::Base> {
        let i = self.headers.len();
        self.headers.push(header.0);
        Sp {
            src: Src3::Entry(self.e.header, i),
            val: header.0,
            pending: header.1,
            lanes: [None, None],
            spare: None,
        }
    }
    fn fold_words(
        &mut self,
        pool: &mut Pool<C::Base>,
        sp: &mut Sp<C::Base>,
        base: usize,
        tr: &FoldTrace<C>,
        at: usize,
        n: usize,
    ) {
        for i in at..at + n {
            self.absorb(
                pool,
                sp,
                Word::V(base + i),
                C::Base::from_u128(tr.shared[i]),
            );
        }
    }
    fn fold_challenge(
        &mut self,
        pool: &mut Pool<C::Base>,
        sp: &mut Sp<C::Base>,
        base: usize,
        tr: &FoldTrace<C>,
        j: usize,
    ) -> (ChRef, Vec<bool>) {
        let (bits, fresh) = &tr.challenges[j];
        let ch = if *fresh {
            let s = self.squeeze(pool, sp, false);
            sp.spare = Some(ChRef::Half(s, true));
            ChRef::Half(s, false)
        } else {
            sp.spare.take().expect("private fold spare challenge")
        };
        self.v_from.push((base + tr.layout.challenges + j, ch));
        (ch, bits.clone())
    }
    fn fold_readback(
        &mut self,
        pool: &mut Pool<C::Base>,
        sp: &mut Sp<C::Base>,
        p: (PtRef, C),
    ) -> (PtRef, C) {
        let i = self.inputs.len();
        let (x, y) = coords(&p.1);
        self.inputs.push((x, y, Some(p.0)));
        self.absorb(pool, sp, Word::X(i), x);
        self.absorb(pool, sp, Word::Y(i), y);
        (PtRef::Input(i), p.1)
    }

    /// Verifies the private fold, including its source/mask commitment and exact
    /// gamma correction. Source coordinates and source u values must be connected
    /// to their predecessor proof ports by the enclosing recursive circuit.
    pub fn generator_fold(
        &mut self,
        pool: &mut Pool<C::Base>,
        gens: &Gens<C>,
        fold: &Fold<C>,
        tr: &FoldTrace<C>,
        base: usize,
    ) -> GeneratorPoints {
        self.total = true;
        let l = &tr.layout;
        let mut input_sp = self.fold_sp(tr.input_header);
        self.fold_words(pool, &mut input_sp, base, tr, 0, 2);
        let mut sources = vec![];
        let mut source_points = vec![];
        for j in 0..SOURCE_COUNT {
            let p = self.input(pool, &mut input_sp, &fold.sources[j].g);
            let PtRef::Input(i) = p.0 else { unreachable!() };
            sources.push(i);
            source_points.push(p);
            self.claim_limbs
                .push((i, std::array::from_fn(|n| base + l.source(j) + n)));
            self.fold_words(pool, &mut input_sp, base, tr, l.source(j) + 4, 2 * l.k);
        }
        self.fold_words(pool, &mut input_sp, base, tr, l.masks, 2 * (1 + 2 * l.k));
        let digest = self.squeeze(pool, &mut input_sp, true);
        self.v_from
            .push((base + l.digest, ChRef::Full(digest, false)));
        self.v_from
            .push((base + l.digest + 1, ChRef::Full(digest, true)));
        let d0 = self.multiply(
            None,
            Some(C::from(gens.g[0])),
            &F::<C>::from_u128(tr.shared[l.digest]),
            [base + l.digest, base + l.zero],
        );
        let d1 = self.multiply(
            None,
            Some(C::from(gens.g[1])),
            &F::<C>::from_u128(tr.shared[l.digest + 1]),
            [base + l.digest + 1, base + l.zero],
        );
        let dr = self.multiply(
            None,
            Some(C::from(gens.h)),
            &fold.masks[0],
            [base + l.masks, base + l.masks + 1],
        );
        let ds = self.add(d0, d1, false);
        let d = self.add(ds, dr, false);
        assert_eq!(d.1, C::from(fold.d));
        let mut sp = self.fold_sp(tr.fold_header);
        self.fold_words(pool, &mut sp, base, tr, 0, 2);
        self.fold_readback(pool, &mut sp, d);
        let mu = self.fold_challenge(pool, &mut sp, base, tr, 0);
        let mut commitment = *source_points.last().unwrap();
        for j in (0..SOURCE_COUNT - 1).rev() {
            let m = self.endoscale(commitment, mu.0, &mu.1);
            commitment = self.add(source_points[j], m, false);
        }
        assert_eq!(commitment.1, tr.commitment);
        self.fold_readback(pool, &mut sp, commitment);
        for j in 0..l.k {
            let _ = self.fold_challenge(pool, &mut sp, base, tr, 1 + j);
        }
        let _ = self.fold_challenge(pool, &mut sp, base, tr, 1 + l.k);
        let kz = self.multiply(
            None,
            Some(C::from(gens.u)),
            &(tr.value * tr.zeta),
            [base + l.kz, base + l.kz + 1],
        );
        let mut folded = self.add(commitment, kz, false);
        for (j, (left, right)) in fold.lr.iter().enumerate() {
            let lp = self.input(pool, &mut sp, left);
            let rp = self.input(pool, &mut sp, right);
            let u = self.fold_challenge(pool, &mut sp, base, tr, 2 + l.k + j);
            let ul = self.endoscale(lp, u.0, &u.1);
            let sum = self.add(folded, ul, false);
            let scaled = self.endoscale(sum, u.0, &u.1);
            folded = self.add(scaled, rp, false);
        }
        let output = self.input(pool, &mut sp, &fold.output.g);
        let PtRef::Input(output_i) = output.0 else {
            unreachable!()
        };
        self.claim_limbs
            .push((output_i, std::array::from_fn(|n| base + l.output + n)));
        let offset = F::<C>::from(2).pow_vartime([255]) + F::<C>::ONE;
        let adjusted = (fold.c - offset) * F::<C>::from(2).invert().unwrap();
        let cg = self.multiply(
            Some(output),
            None,
            &adjusted,
            [base + l.adjusted_c, base + l.adjusted_c + 1],
        );
        let cbz = self.multiply(
            None,
            Some(C::from(gens.u)),
            &(fold.c * tr.b_final * tr.zeta),
            [base + l.cbz, base + l.cbz + 1],
        );
        let gamma = self.multiply(
            None,
            Some(C::from(gens.h)),
            &tr.gamma,
            [base + l.gamma, base + l.gamma + 1],
        );
        let sum = self.add(cg, cbz, false);
        let rhs = self.add(sum, gamma, false);
        assert_eq!(rhs.1, folded.1, "exact private generator restructuring");
        let (PtRef::Add(left), PtRef::Add(right)) = (rhs.0, folded.0) else {
            unreachable!()
        };
        self.equal.push((left, right));
        GeneratorPoints {
            sources,
            output: output_i,
        }
    }
}
