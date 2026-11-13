//! In-memory public verifier projection after a completed proof. Transport
//! validation remains the responsibility of the bounded public artifact codec.
use super::*;

impl<F: Scalar> UniformCircuit<F> {
    /// Counts of matrix instructions, field constants and fixed-row fields.
    /// Computing these counts compiles the exact public matrix program.
    pub fn public_program_counts(&self) -> (usize, usize, usize) {
        let plan = self.plan();
        (plan.ops.len(), plan.pool.len(), self.fixed_values.len())
    }

    /// Locate exact public program differences while diagnosing convergence.
    /// These components are public verifier data, never retained witnesses.
    pub fn public_program_differences(&self, other: &Self) -> String {
        let a = self.plan();
        let b = other.plan();
        let first = |a: &[F], b: &[F]| a.iter().zip(b).position(|(a, b)| a != b);
        let op = |op: &Op| match *op {
            Op::Add(a, b) => (0, a, b),
            Op::Sub(a, b) => (1, a, b),
            Op::Mul(a, b) => (2, a, b),
            Op::Scale(a, b) => (3, a, b),
            Op::Const(a) => (4, a, 0),
        };
        let ops = a.ops.iter().zip(&b.ops).position(|(a, b)| op(a) != op(b));
        let pool = first(&a.pool, &b.pool);
        let fixed = first(&self.fixed_values, &other.fixed_values);
        let values = |a: &[F], b: &[F], at: Option<usize>| {
            at.map(|i| format!("{i}: {:?} -> {:?}", a[i].to_repr(), b[i].to_repr()))
        };
        format!(
            "geometry_same={} challenges_same={} io_same={} tape_inputs={}=>{} ops={}=>{} first_op={ops:?} outputs_same={} pool={}=>{} first_pool={:?} fixed={}=>{} first_fixed={:?} late_same={} link_same={}",
            self.layout() == other.layout()
                && self.shape.col_bits == other.shape.col_bits
                && self.constraints == other.constraints,
            (self.phase2_challenges(), self.challenge_degrees())
                == (other.phase2_challenges(), other.challenge_degrees()),
            self.io_len == other.io_len,
            a.inputs,
            b.inputs,
            a.ops.len(),
            b.ops.len(),
            a.out == b.out,
            a.pool.len(),
            b.pool.len(),
            values(&a.pool, &b.pool, pool),
            self.fixed_values.len(),
            other.fixed_values.len(),
            values(&self.fixed_values, &other.fixed_values, fixed),
            self.late_io == other.late_io,
            self.link_io == other.link_io,
        )
    }

    /// Consume a completed compiler/prover and retain only its public verifier.
    /// No wire-size limit applies to temporary in-memory research templates;
    /// installed final programs must still pass the strict artifact codec.
    pub fn into_verifier_program(mut self) -> Self
    where
        F: zeroize::Zeroize,
    {
        let metadata = (self.phase2_challenges(), self.challenge_degrees());
        let _ = self.plan();
        self.zeroize_witness_values();
        Self {
            verifier_only: Some(metadata),
            label: self.label,
            arrays: vec![],
            pi: vec![],
            counts: vec![],
            shape: self.shape,
            io_len: self.io_len,
            constraints: self.constraints,
            product_masks: vec![],
            row_base: vec![],
            blocks: vec![],
            bounds: vec![],
            fixed_rows: self.fixed_rows,
            fixed_values: self.fixed_values,
            wallet_rows: self.wallet_rows,
            wallet_io: self.wallet_io,
            phase2: vec![],
            lookup_arrays: vec![],
            tables: vec![],
            ch_rows: vec![],
            ch_fill: None,
            io_arrays: vec![],
            late_io: self.late_io,
            exports: vec![],
            lookup_mask: HashMap::new(),
            lookup_mask_col: HashMap::new(),
            copy_exports: vec![],
            copy_imports: vec![],
            index_arrays: vec![],
            p1_rows: self.p1_rows,
            pl_rows: self.pl_rows,
            p2_rows: self.p2_rows,
            link_tables: vec![],
            link_fill: None,
            link_io: self.link_io,
            // This public-input mapping is distinct from the discarded private
            // link witness fill. Monetary P/S mappings are capture-free.
            link_io_values: self.link_io_values,
            link_binding: None,
            rfield: vec![],
            moved: vec![],
            split_of: vec![],
            gather: OnceLock::new(),
            symbolic: OnceLock::new(),
            readers: OnceLock::new(),
            m_plans: OnceLock::new(),
            plan: self.plan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ipa, spartan};

    fn check<C: iroha_pasta::PastaCurve>()
    where
        ipa::F<C>: Scalar + zeroize::Zeroize,
    {
        type F<C> = ipa::F<C>;
        let mut w = crate::full::Wiring::<F<C>>::default();
        let family = w.family(0, 1);
        let fixed = w.push(Array {
            name: "retention public constant",
            family,
            rho: 0,
            r_count: 1,
            fixed: true,
            lookups: vec![],
            widths: vec![1],
            rows: vec![],
            values: vec![vec![F::<C>::ONE]],
        });
        let output = w.push(Array {
            name: "retention public square",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![],
            values: vec![vec![F::<C>::from(49)]],
        });
        w.io_arrays.push((output, 4));
        w.push(Array {
            name: "retention private root",
            family,
            rho: 0,
            r_count: 1,
            fixed: false,
            lookups: vec![],
            widths: vec![255],
            rows: vec![
                [
                    own(0, 1),
                    own(0, 1),
                    link(output, 0, vec![Some(0)], F::<C>::ONE),
                ],
                [
                    own(0, 1),
                    link(fixed, 0, vec![Some(0)], F::<C>::ONE),
                    own(0, 1),
                ],
            ],
            values: vec![vec![F::<C>::from(7)]],
        });
        let (arrays, pi, counts, tables, copies) = w.finish();
        let (c, instance, mut witness) = UniformCircuit::build_with_copies(
            "retention fixture",
            arrays,
            pi,
            counts,
            tables,
            copies,
            10,
            64,
            b"public retention fixture",
        );
        let mut rng = SpikeRng::new(b"public retention proof");
        let gens = ipa::Gens::<C>::new(c.shape.cols());
        let (proof, _) = spartan::prove_circuit(&c.shape, &gens, &instance, &witness, &mut rng, &c);
        let rx: Vec<_> = (0..c.shape.sx).map(|_| rng.scalar()).collect();
        let ry: Vec<_> = (0..c.shape.sz()).map(|_| rng.scalar()).collect();
        let ch = [rng.scalar(), rng.scalar()];
        let expected = (
            c.program_digest(),
            c.layout(),
            c.evals(&rx, &ry, &ch),
            spartan::fixed_commitments(&c.shape, &gens, &c),
            c.public_program_counts(),
        );
        zeroize::Zeroize::zeroize(&mut witness);
        let public = c.into_verifier_program();
        assert!(public.arrays.is_empty());
        assert!(public.gather.get().is_none() && public.symbolic.get().is_none());
        assert!(public.readers.get().is_none() && public.m_plans.get().is_none());
        assert_eq!(
            (
                public.program_digest(),
                public.layout(),
                public.evals(&rx, &ry, &ch),
                spartan::fixed_commitments(&public.shape, &gens, &public),
                public.public_program_counts(),
            ),
            expected,
        );
        assert!(
            spartan::verify_circuit(&public.shape, &gens, &instance, &proof.to_bytes(), &public)
                .is_ok()
        );
        let mut forged = instance.clone();
        forged.io[4] += F::<C>::ONE;
        assert!(
            spartan::verify_circuit(&public.shape, &gens, &forged, &proof.to_bytes(), &public)
                .is_err()
        );
    }

    #[test]
    fn retained_public_program_preserves_both_curve_verifiers() {
        check::<iroha_pasta::Eq>();
        check::<iroha_pasta::Ep>();
    }
}
