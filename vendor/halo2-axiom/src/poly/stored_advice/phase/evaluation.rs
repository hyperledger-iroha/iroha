//! Whole-tile transactions over completed advice receipts, without mutable snapshot escape.
//!
//! Only the concrete expression and graph entry points are exposed. Their private transaction
//! directly returns the actual evaluator result, so preflight, allocation, arithmetic and final
//! consumer errors or unwinding all drop the complete session. Every live advice identity is
//! rechecked after a successful consumer before restoration. Catching a panic outside an
//! entry point leaves the original owner poisoned. A per-read latch additionally prevents a
//! future internal caller from restoring a session after swallowing a chunk failure.
//!
//! Receipts are still in their original Lagrange basis. Coset conversion, stored arguments and
//! quotient/output/opening owners remain separate work. Fixed/instance banks, the expression or
//! finalized graph, and beta/gamma/theta/y must come from the same admitted key/transcript owner;
//! this module checks retained advice/challenge bindings and dimensions, not key provenance.
//! TODO: connect these tiles to the admitted key, owned basis conversions and complete
//! argument/quotient/opening pipeline before claiming complete-prover resource qualification.

use super::{CompleteStoredAdviceV1, Session};
use crate::{
    arithmetic::CurveAffine,
    plonk::stored::{
        StoredAuxiliarySourceV1, StoredExpressionContextV1, StoredExpressionErrorV1,
        StoredExpressionPlanV1, StoredRowTileV1,
        graph::{StoredGraphPlanV1, with_stored_graph_reader_v1},
        with_stored_expression_reader_v1, with_stored_expression_sources_v1,
    },
    poly::{
        LagrangeCoeff, Polynomial,
        commitment::Params,
        stored_advice::{
            StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
            StoredPolynomialSnapshotV1, assignment::StoredAssignmentFieldV1,
            reader::StoredAdviceChunkSourceV1,
        },
    },
};

// This view cannot escape the private transaction, expose S or copy a commitment blind.
struct CompleteReader<'read, 'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    session: &'read mut Session<'params, C, S>,
    poisoned: bool,
}

impl<C, S> StoredAdviceChunkSourceV1 for CompleteReader<'_, '_, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    fn column_count(&self) -> usize {
        self.session.columns.len()
    }

    fn validate_layout(
        &mut self,
        expected: StoredPolynomialLayoutV1,
    ) -> Result<(), StoredPolynomialErrorV1> {
        if self.poisoned {
            return Err(StoredPolynomialErrorV1::Poisoned);
        }
        self.poisoned = true;
        let column = self
            .session
            .columns
            .get(expected.advice_coordinates()?.0 as usize)
            .ok_or(StoredPolynomialErrorV1::Context)?;
        if column.layout != expected || column.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        self.poisoned = false;
        Ok(())
    }

    fn with_chunk<R>(
        &mut self,
        expected: StoredPolynomialLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]]) -> Result<R, StoredPolynomialErrorV1>,
    ) -> Result<R, StoredPolynomialErrorV1> {
        self.validate_layout(expected)?;
        // Set before geometry, backend and decoder calls. Only their genuine success clears it.
        self.poisoned = true;
        let count = expected.chunk_scalar_count(chunk)?;
        let column = self
            .session
            .columns
            .get_mut(expected.advice_coordinates()?.0 as usize)
            .ok_or(StoredPolynomialErrorV1::Context)?;
        let result = column.snapshot.with_chunk(expected, chunk, |encoded| {
            if encoded.len() != count
                || encoded
                    .iter()
                    .any(|value| !expected.field().is_canonical(value))
            {
                return Err(StoredPolynomialErrorV1::Encoding);
            }
            consume(encoded)
        })?;
        if column.snapshot.layout() != expected {
            return Err(StoredPolynomialErrorV1::Context);
        }
        self.poisoned = false;
        Ok(result)
    }
}

fn validate_session<C, S>(
    session: &Session<'_, C, S>,
    context: StoredExpressionContextV1<'_>,
    challenges: &[C::Scalar],
) -> Result<(), StoredExpressionErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
{
    let plan = &session.plan;
    for expected in context.advice {
        expected.advice_coordinates()?;
    }
    if context.domain.field() != C::Scalar::STORED_FIELD
        || context.domain.field() != plan.field
        || context.domain.k() != plan.k
        || context.domain.k() != plan.params.k()
        || context.domain.basis() != StoredPolynomialBasisV1::Lagrange
        || context.advice.len() != plan.columns
        || context.advice.len() != session.columns.len()
        || context.instance_columns != plan.instance_columns
        || context.challenge_phases.len() != plan.challenges
        || challenges.len() != plan.challenges
        || session.challenges.len() != plan.challenges
        || session
            .proof_context
            .is_some_and(|proof_context| context.domain.proof_context != proof_context)
        || context
            .advice
            .iter()
            .zip(&session.columns)
            .any(|(expected, actual)| *expected != actual.layout)
        || challenges
            .iter()
            .zip(&session.challenges)
            .any(|(expected, actual)| Some(*expected) != *actual)
    {
        return Err(StoredExpressionErrorV1::Context);
    }
    for (phase, scheduled) in plan.phases.iter().enumerate() {
        for challenge in &scheduled.challenges {
            if context.challenge_phases.get(*challenge).copied() != Some(phase as u8) {
                return Err(StoredExpressionErrorV1::Context);
            }
        }
    }
    // Zero-advice completion has no stored proof context. Its public domain label cannot
    // establish key provenance; field/k/base-basis/instance dimensions still match the plan.
    Ok(())
}

impl<'params, C, S> CompleteStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    // Deliberately private: an arbitrary exported closure could swallow an evaluator failure
    // and return Ok. The concrete wrappers below propagate its real Result directly.
    fn with_evaluation<R>(
        &mut self,
        context: StoredExpressionContextV1<'_>,
        challenges: &[C::Scalar],
        evaluate: impl FnOnce(
            &mut CompleteReader<'_, 'params, C, S>,
        ) -> Result<R, StoredExpressionErrorV1>,
    ) -> Result<R, StoredExpressionErrorV1> {
        let mut session = self
            .session
            .take()
            .ok_or(StoredPolynomialErrorV1::Poisoned)?;
        validate_session(&session, context, challenges)?;
        let mut reader = CompleteReader {
            session: &mut session,
            poisoned: false,
        };
        let result = evaluate(&mut reader)?;
        if reader.poisoned {
            return Err(StoredPolynomialErrorV1::Poisoned.into());
        }
        // A consumer can share a backend with any receipt, including one absent from this
        // expression. Its success alone cannot authorize restoration of changed identities.
        for expected in context.advice {
            reader.validate_layout(*expected)?;
        }
        self.session = Some(session);
        Ok(result)
    }

    /// Evaluate one complete expression tile while owning every receipt until success.
    ///
    /// Exact retained advice identities, base domain, challenge values and phase schedule are
    /// checked before plaintext access. The caller owns matching fixed/instance/key provenance.
    /// Any failure or unwind, including from the final consumer, destroys the complete owner;
    /// downstream output/transcript side effects must also be discarded by their owners.
    pub(crate) fn with_expression_tile<R>(
        &mut self,
        plan: &StoredExpressionPlanV1<'_, C::Scalar>,
        tile: StoredRowTileV1,
        fixed: &[Polynomial<C::Scalar, LagrangeCoeff>],
        instance: &[Polynomial<C::Scalar, LagrangeCoeff>],
        challenges: &[C::Scalar],
        consume: impl FnOnce(&[C::Scalar]) -> Result<R, StoredExpressionErrorV1>,
    ) -> Result<R, StoredExpressionErrorV1> {
        self.with_evaluation(plan.context(), challenges, |reader| {
            with_stored_expression_reader_v1(
                plan, tile, reader, fixed, instance, challenges, consume,
            )
        })
    }

    /// Evaluate one expression tile with the proof owner's fixed/instance value source.
    ///
    /// The actual fallible evaluator result controls restoration of this session. Auxiliary
    /// source failures, consumer rejection and unwinding therefore destroy every original
    /// receipt just like an advice-read failure. No source or snapshot handle escapes here.
    pub(crate) fn with_expression_sources<R, X>(
        &mut self,
        plan: &StoredExpressionPlanV1<'_, C::Scalar>,
        tile: StoredRowTileV1,
        auxiliary: &mut X,
        challenges: &[C::Scalar],
        consume: impl FnOnce(&[C::Scalar]) -> Result<R, StoredExpressionErrorV1>,
    ) -> Result<R, StoredExpressionErrorV1>
    where
        X: StoredAuxiliarySourceV1<C::Scalar>,
    {
        self.with_evaluation(plan.context(), challenges, |reader| {
            with_stored_expression_sources_v1(plan, tile, reader, auxiliary, challenges, consume)
        })
    }

    /// Evaluate one retained-graph tile under the same complete-session transaction.
    ///
    /// This is a base-domain consumer; it does not create a quotient or coset snapshot. The
    /// caller supplies the matching key's graph and transcript's beta/gamma/theta/y and previous
    /// outer-fold tile. Their provenance is separate from the retained advice/challenge checks.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_graph_tile<R>(
        &mut self,
        plan: &StoredGraphPlanV1<'_, '_, C>,
        tile: StoredRowTileV1,
        fixed: &[Polynomial<C::Scalar, LagrangeCoeff>],
        instance: &[Polynomial<C::Scalar, LagrangeCoeff>],
        challenges: &[C::Scalar],
        beta: C::Scalar,
        gamma: C::Scalar,
        theta: C::Scalar,
        y: C::Scalar,
        previous: &[C::Scalar],
        consume: impl FnOnce(&[C::Scalar]) -> Result<R, StoredExpressionErrorV1>,
    ) -> Result<R, StoredExpressionErrorV1> {
        self.with_evaluation(plan.context(), challenges, |reader| {
            with_stored_graph_reader_v1(
                plan, tile, reader, fixed, instance, challenges, beta, gamma, theta, y, previous,
                consume,
            )
        })
    }
}

#[cfg(test)]
mod unit_tests {
    use super::super::{SecretBlind, StoredColumn, admit_stored_phase_plan_v1};
    use super::*;
    use crate::{
        plonk::ConstraintSystem,
        poly::{
            EvaluationDomain,
            commitment::{Blind, ParamsProver},
            ipa::commitment::ParamsIPA,
            stored_advice::{StoredLookupSideV1, StoredPolynomialRoleV1},
        },
    };
    use ff::Field;
    use halo2curves::pasta::{EqAffine, Fp};

    #[test]
    fn self_consistent_lookup_receipts_fail_completed_evaluation_role_preflight() {
        let params = ParamsIPA::<EqAffine>::new(4);
        let domain = EvaluationDomain::<Fp>::new(3, 4);
        for side in [StoredLookupSideV1::Input, StoredLookupSideV1::Table] {
            let mut meta = ConstraintSystem::default();
            meta.advice_column();
            let plan = admit_stored_phase_plan_v1(&params, &domain, &meta).unwrap();
            let label = StoredPolynomialLayoutV1::new(
                [37; 32],
                0,
                Fp::STORED_FIELD,
                StoredPolynomialBasisV1::Lagrange,
                4,
                StoredPolynomialRoleV1::LookupCompressed { lookup: 0, side },
            )
            .unwrap();
            let layouts = [label];
            // All dimensions, contexts, cached identities and phase challenges agree.
            // An inert snapshot makes clear that this preflight requires no backend access.
            let session = Session::<EqAffine, ()> {
                plan,
                next_phase: 1,
                proof_context: Some([37; 32]),
                greatest_ordinal: Some(0),
                columns: vec![StoredColumn {
                    layout: label,
                    snapshot: (),
                    blind: SecretBlind(Blind(Fp::ONE)),
                }],
                challenges: vec![],
            };
            let context = StoredExpressionContextV1 {
                domain: label,
                advice: &layouts,
                fixed_columns: 0,
                instance_columns: 0,
                challenge_phases: &[],
            };
            assert_eq!(
                validate_session(&session, context, &[]),
                Err(StoredPolynomialErrorV1::Context.into())
            );
        }
    }
}
