//! Exact-key lookup-expression tiles with borrowed fixed values and virtual instance padding.
//!
//! This consumes the whole pending proof owner for each concrete tile operation. Expressions,
//! fixed preprocessing, instance prefixes and phase metadata come only from its retained key
//! and original protocol state. The value source owns no polynomial bank or witness buffer.
//! The sibling lookup stage consumes this metadata for compression. Permutation, commitments
//! and products remain TODO; successful tiles are not an argument-complete proof transition.

use ff::{Field, WithSmallOrderMulGroup};

use super::PendingStoredIpaProverV1;
use crate::{
    arithmetic::CurveAffine,
    plonk::{
        ProvingKey,
        stored::{
            StoredAuxiliarySourceV1, StoredExpressionContextV1, StoredExpressionErrorV1,
            StoredRowTileV1, prepare_stored_expression_v1,
        },
    },
    poly::{
        commitment::Params,
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, StoredLookupSideV1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
            StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialRoleV1,
            assignment::StoredAssignmentFieldV1, phase::StoredPhaseErrorV1,
        },
    },
};

pub(super) fn phase_error(error: StoredPhaseErrorV1) -> StoredExpressionErrorV1 {
    match error {
        StoredPhaseErrorV1::Store(error) => error.into(),
        StoredPhaseErrorV1::Poisoned => StoredPolynomialErrorV1::Poisoned.into(),
        _ => StoredExpressionErrorV1::Context,
    }
}

fn metadata<T>(count: usize) -> Result<Vec<T>, StoredExpressionErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredExpressionErrorV1::Allocation)?;
    Ok(values)
}

// Constructed only after destructuring the original pending owner below. Neither detached
// keys nor alternative fixed/instance arrays can enter through its consuming tile method.
pub(super) struct KeyBaseInputs<'key, 'instances, C: CurveAffine> {
    pk: &'key ProvingKey<C>,
    params: &'key ParamsIPA<C>,
    instances: &'instances [&'instances [C::Scalar]],
    domain: StoredPolynomialLayoutV1,
    rows: usize,
}

impl<'key, 'instances, C: CurveAffine> KeyBaseInputs<'key, 'instances, C> {
    pub(super) fn new(
        pk: &'key ProvingKey<C>,
        params: &'key ParamsIPA<C>,
        instances: &'instances [&'instances [C::Scalar]],
        domain: StoredPolynomialLayoutV1,
    ) -> Self {
        Self {
            pk,
            params,
            instances,
            domain,
            rows: domain.scalar_count(),
        }
    }
}

/// Public planning metadata copied from the concrete owner; no witness bank is retained.
pub(super) struct KeyExpressionMetadata<F> {
    pub(super) domain: StoredPolynomialLayoutV1,
    layouts: Vec<StoredPolynomialLayoutV1>,
    challenge_phases: Vec<u8>,
    pub(super) challenges: Vec<F>,
    fixed_columns: usize,
    instance_columns: usize,
}
impl<F> KeyExpressionMetadata<F> {
    pub(super) fn context(&self) -> StoredExpressionContextV1<'_> {
        StoredExpressionContextV1 {
            domain: self.domain,
            advice: &self.layouts,
            fixed_columns: self.fixed_columns,
            instance_columns: self.instance_columns,
            challenge_phases: &self.challenge_phases,
        }
    }
}

/// Use the retained key and original protocol inputs for both tiles and actual compression.
/// The empty-advice geometry label creates no authenticated receipt or provider context.
pub(super) fn key_expression_metadata<C>(
    pk: &ProvingKey<C>,
    params: &ParamsIPA<C>,
    instances: &[&[C::Scalar]],
    receipts: impl ExactSizeIterator<Item = StoredPolynomialLayoutV1>,
    challenges: impl Iterator<Item = C::Scalar>,
) -> Result<KeyExpressionMetadata<C::Scalar>, StoredExpressionErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    if receipts.len() != pk.vk.cs.num_advice_columns
        || pk.vk.cs.challenge_phase.len() != pk.vk.cs.num_challenges
    {
        return Err(StoredExpressionErrorV1::Context);
    }
    let mut layouts = metadata(receipts.len())?;
    layouts.extend(receipts);
    let domain = match layouts.first() {
        Some(layout) => *layout,
        None => StoredPolynomialLayoutV1::new(
            [1; 32],
            0,
            C::Scalar::STORED_FIELD,
            StoredPolynomialBasisV1::Lagrange,
            pk.vk.domain.k(),
            StoredPolynomialRoleV1::Advice {
                column: 0,
                phase: 0,
            },
        )?,
    };
    let mut challenge_phases = metadata(pk.vk.cs.challenge_phase.len())?;
    challenge_phases.extend(pk.vk.cs.challenge_phase.iter().map(|phase| phase.to_u8()));
    let mut values = metadata(pk.vk.cs.num_challenges)?;
    for challenge in challenges {
        if values.len() == pk.vk.cs.num_challenges {
            return Err(StoredExpressionErrorV1::Context);
        }
        values.push(challenge);
    }
    if values.len() != pk.vk.cs.num_challenges {
        return Err(StoredExpressionErrorV1::Context);
    }
    let result = KeyExpressionMetadata {
        domain,
        layouts,
        challenge_phases,
        challenges: values,
        fixed_columns: pk.vk.cs.num_fixed_columns,
        instance_columns: pk.vk.cs.num_instance_columns,
    };
    KeyBaseInputs::new(pk, params, instances, domain).validate(result.context())?;
    Ok(result)
}

impl<C> StoredAuxiliarySourceV1<C::Scalar> for KeyBaseInputs<'_, '_, C>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    fn validate(
        &mut self,
        expected: StoredExpressionContextV1<'_>,
    ) -> Result<(), StoredExpressionErrorV1> {
        let cs = &self.pk.vk.cs;
        let k = self.pk.vk.domain.k();
        if k > STORED_MAX_K_V1
            || !self.domain.same_proof_context(expected.domain)
            || expected.domain.field() != C::Scalar::STORED_FIELD
            || expected.domain.k() != k
            || expected.domain.basis() != StoredPolynomialBasisV1::Lagrange
            || self.params.k() != k
            || self.params.n() != 1_u64 << k
            || self.rows != 1_usize << k
            || self.params.get_g_lagrange().len() != self.rows
            || expected.fixed_columns != cs.num_fixed_columns
            || self.pk.fixed_values.len() != cs.num_fixed_columns
            || self
                .pk
                .fixed_values
                .iter()
                .any(|column| column.len() != self.rows)
            || expected.instance_columns != cs.num_instance_columns
            || self.instances.len() != cs.num_instance_columns
            || expected.advice.len() != cs.num_advice_columns
            || !expected
                .challenge_phases
                .iter()
                .copied()
                .eq(cs.challenge_phase.iter().map(|phase| phase.to_u8()))
        {
            return Err(StoredExpressionErrorV1::Context);
        }
        let unusable = cs
            .blinding_factors()
            .checked_add(1)
            .ok_or(StoredExpressionErrorV1::Context)?;
        let usable = self
            .rows
            .checked_sub(unusable)
            .ok_or(StoredExpressionErrorV1::Context)?;
        if self.instances.iter().any(|column| column.len() > usable) {
            return Err(StoredExpressionErrorV1::Context);
        }
        Ok(())
    }

    fn fixed_value(
        &mut self,
        column: usize,
        row: usize,
    ) -> Result<C::Scalar, StoredExpressionErrorV1> {
        if row >= self.rows {
            return Err(StoredExpressionErrorV1::Context);
        }
        self.pk
            .fixed_values
            .get(column)
            .and_then(|values| values.get(row))
            .copied()
            .ok_or(StoredExpressionErrorV1::Context)
    }

    fn instance_value(
        &mut self,
        column: usize,
        row: usize,
    ) -> Result<C::Scalar, StoredExpressionErrorV1> {
        if row >= self.rows {
            return Err(StoredExpressionErrorV1::Context);
        }
        let values = self
            .instances
            .get(column)
            .ok_or(StoredExpressionErrorV1::Context)?;
        // This is post-synthesis polynomial evaluation. Every valid domain row beyond the
        // supplied prefix is ZERO, including inactive rows; query_instance rules do not apply.
        Ok(values.get(row).copied().unwrap_or(C::Scalar::ZERO))
    }
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
    PendingStoredIpaProverV1<'params, 'instances, C, P, R, T, E, QUERY_INSTANCE, INSTANCE_MASK>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    /// Evaluate one exact retained lookup expression while retaining the whole proof owner.
    ///
    /// Only public lookup/side/expression/tile indices and a scratch budget are accepted. The
    /// key supplies the expression and fixed values; original instance prefixes are padded on
    /// access, after the evaluator wraps rotations. No theta squeeze or proof-RNG/transcript
    /// operation occurs. No full instance polynomial is built. Consumer/backend errors and
    /// unwinding destroy the original key, advice, provider and protocol state together.
    /// TODO: integrate real compression and argument commitments before exposing a proof path.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_lookup_expression_tile<V>(
        self,
        lookup_index: usize,
        side: StoredLookupSideV1,
        expression_index: usize,
        tile: StoredRowTileV1,
        scratch_limit_bytes: usize,
        consume: impl FnOnce(&[C::Scalar]) -> Result<V, StoredExpressionErrorV1>,
    ) -> Result<(Self, V), StoredExpressionErrorV1> {
        let Self {
            params,
            pk,
            mut advice,
            provider,
            rng,
            transcript,
            instances,
            _challenge,
        } = self;
        let result = {
            let lookup = pk
                .vk
                .cs
                .lookups
                .get(lookup_index)
                .ok_or(StoredExpressionErrorV1::Context)?;
            let expressions = match side {
                StoredLookupSideV1::Input => &lookup.input_expressions,
                StoredLookupSideV1::Table => &lookup.table_expressions,
            };
            let expression = expressions
                .get(expression_index)
                .ok_or(StoredExpressionErrorV1::Context)?;
            let metadata = key_expression_metadata(
                &pk,
                params,
                instances,
                advice.layouts().map_err(phase_error)?,
                advice.challenges().map_err(phase_error)?,
            )?;
            let plan =
                prepare_stored_expression_v1(expression, metadata.context(), scratch_limit_bytes)?;
            let mut auxiliary = KeyBaseInputs::new(&pk, params, instances, metadata.domain);
            advice.with_expression_sources(
                &plan,
                tile,
                &mut auxiliary,
                &metadata.challenges,
                consume,
            )?
        };
        Ok((
            Self {
                params,
                pk,
                advice,
                provider,
                rng,
                transcript,
                instances,
                _challenge,
            },
            result,
        ))
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn metadata_capacity_overflow_and_phase_errors_remain_fallible() {
        assert!(matches!(
            metadata::<u8>(usize::MAX),
            Err(StoredExpressionErrorV1::Allocation)
        ));
        assert!(metadata::<u8>(0).unwrap().is_empty());
        assert_eq!(
            phase_error(StoredPhaseErrorV1::Poisoned),
            StoredPolynomialErrorV1::Poisoned.into()
        );
        assert_eq!(
            phase_error(StoredPhaseErrorV1::Store(
                StoredPolynomialErrorV1::Authentication
            )),
            StoredPolynomialErrorV1::Authentication.into()
        );
        assert_eq!(
            phase_error(StoredPhaseErrorV1::Admission),
            StoredExpressionErrorV1::Context
        );
    }
}
