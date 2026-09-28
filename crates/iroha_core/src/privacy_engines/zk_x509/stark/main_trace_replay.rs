//! Original-mask replay from closed immutable MAIN witness owners.

use super::*;
use crate::privacy_engines::transparent_stark::{
    ReplayableTraceMaskV1, masked_trace_coefficients_with_mask_v1, sample_trace_mask_v1,
};

/// The only long-lived per-column randomness; coefficients clear on drop.
pub(in super::super) struct MainTraceMaskGroupV1 {
    native_log: u8,
    common_log: u8,
    masks: Vec<ReplayableTraceMaskV1>,
}

impl MainTraceMaskGroupV1 {
    pub(in super::super) fn sample_v1<R: TryRngCore>(
        native_log: u8,
        common_log: u8,
        width: usize,
        rng: &mut R,
        mut source: impl FnMut(usize) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let native_rows = 1_usize
            .checked_shl(u32::from(native_log))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let common_rows = 1_usize
            .checked_shl(u32::from(common_log))
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if native_log >= common_log
            || width == 0
            || width > usize::from(u16::MAX)
            || native_rows
                .checked_add(MASK_DEGREE)
                .is_none_or(|highest| highest >= common_rows)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut masks = Vec::new();
        masks
            .try_reserve_exact(width)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for column in 0..width {
            // Preserve the original source-before-entropy order and reject bad
            // native columns before sampling their masks. No native column survives.
            let native = source(column)?;
            if native.len() != native_rows
                || native.iter().any(|value| F::canonical(value.0).is_none())
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            masks.push(sample_trace_mask_v1(MASK_DEGREE, rng).map_err(map_transparent_error_v1)?);
        }
        Ok(Self {
            native_log,
            common_log,
            masks,
        })
    }

    fn replay_v1(
        &self,
        column: usize,
        native: &[F],
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let mask = self
            .masks
            .get(column)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        masked_trace_coefficients_with_mask_v1(native, self.native_log, mask.coefficients())
            .map(ZeroizingMainTraceColumnV1)
            .map_err(map_transparent_error_v1)
    }
}

/// Six exact groups retain explicit original masks, never all native coefficients.
/// Witnesses remain borrowed from the closed phase owners below. Each replay
/// reconstructs the same polynomial and final commitment replay checks its root.
pub(in super::super) struct MainTracePolynomialSetV1 {
    groups: [MainTraceMaskGroupV1; FULL_PROFILE_TRACE_GROUPS_V1],
}

impl MainTracePolynomialSetV1 {
    pub(in super::super) fn from_ordered_v1(
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        groups: Vec<MainTraceMaskGroupV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let set = Self {
            groups: groups
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?,
        };
        set.validate_v1(layout, kind)?;
        Ok(set)
    }

    pub(super) fn validate_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        for (masks, group) in self.groups.iter().zip(&layout.trace_groups) {
            let width = match kind {
                MainTraceColumnKindV1::Base => group.base_width,
                MainTraceColumnKindV1::Aux => group.aux_width,
            };
            if masks.masks.len() != width
                || masks.native_log != group.native_trace_log2
                || masks.common_log != layout.common_lde_log2
                || masks
                    .masks
                    .iter()
                    .any(|mask| mask.coefficients().len() != MASK_DEGREE + 1)
            {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn joined_plan_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<aggregate::joined_trace::JoinedTraceCommitmentPlanV1, ZkX509StarkErrorV1> {
        self.validate_v1(layout, kind)?;
        aggregate::joined_trace::JoinedTraceCommitmentPlanV1::new_v1(
            layout.parameters_v1(),
            &layout.as_shared()?,
            match kind {
                MainTraceColumnKindV1::Base => {
                    aggregate::joined_trace::JoinedTraceColumnKindV1::Base
                }
                MainTraceColumnKindV1::Aux => aggregate::joined_trace::JoinedTraceColumnKindV1::Aux,
            },
        )
        .map_err(map_aggregate_error_v1)
    }

    pub(super) fn replay_column_coefficients_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        group_index: usize,
        column: usize,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let (registration, local_column) =
            registered_main_group_column_v1(layout, group_index, kind, column)?;
        let masks = self
            .groups
            .get(group_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let native = sources.native_column_v1(layout, kind, registration, local_column)?;
        masks.replay_v1(column, &native)
    }

    pub(super) fn commit_joined_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        indices: &[usize],
        sources: &MainTraceReplaySourcesV1<'_, '_>,
    ) -> Result<aggregate::StreamingRowCommitmentResultV1, ZkX509StarkErrorV1> {
        let plan = self.joined_plan_v1(layout, kind)?;
        // The same joined plan creates the leaf framing for retained and replayed
        // columns. At most eight coefficient/evaluation columns coexist.
        let mut source_error = None;
        let result = plan.commit_replayed_v1(AGGREGATE_DOMAINS_V1, indices, |group, column| {
            self.replay_column_coefficients_v1(layout, kind, group, column, sources)
                .map(|coefficients| coefficients.into_vec_v1())
                .map_err(|error| {
                    source_error = Some(error);
                    AggregateStarkErrorV1::InvalidLayout
                })
        });
        match (result, source_error) {
            (Ok(result), None) => Ok(result),
            (Err(_), Some(error)) => Err(error),
            (Err(error), None) => Err(map_aggregate_error_v1(error)),
            (Ok(_), Some(_)) => Err(ZkX509StarkErrorV1::InternalInvariant),
        }
    }

    pub(super) fn deep_group_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        group: usize,
        point: E,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
    ) -> Result<(Vec<E>, Vec<E>), ZkX509StarkErrorV1> {
        self.validate_v1(layout, kind)?;
        let masks = self
            .groups
            .get(group)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let next = point.mul_base(
            goldilocks_primitive_root_v1(masks.native_log).map_err(map_transparent_error_v1)?,
        );
        let mut current_values = ZeroizingExtensionColumnV1(Vec::new());
        let mut next_values = ZeroizingExtensionColumnV1(Vec::new());
        current_values
            .0
            .try_reserve_exact(masks.masks.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        next_values
            .0
            .try_reserve_exact(masks.masks.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for column in 0..masks.masks.len() {
            let coefficients =
                self.replay_column_coefficients_v1(layout, kind, group, column, sources)?;
            let evaluate = |x: E| {
                coefficients
                    .iter()
                    .rev()
                    .fold(E::ZERO, |value, &coefficient| {
                        value.mul(x).add(E::from_base(coefficient))
                    })
            };
            current_values.0.push(evaluate(point));
            next_values.0.push(evaluate(next));
        }
        Ok((
            core::mem::take(&mut current_values.0),
            core::mem::take(&mut next_values.0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    fn native(log: u8, column: usize) -> ZeroizingMainTraceColumnV1 {
        ZeroizingMainTraceColumnV1(
            (0..1_usize << log)
                .map(|row| F((row * row + 17 * column + 3) as u64))
                .collect(),
        )
    }

    #[test]
    fn mask_only_replay_matches_retained_coefficients_entropy_and_independent_native_evaluation() {
        for native_log in [3, 8, 11] {
            let common_log = 13;
            let mut replay_rng = StdRng::from_seed([native_log; 32]);
            let mut retained_rng = StdRng::from_seed([native_log; 32]);
            let group = MainTraceMaskGroupV1::sample_v1(
                native_log,
                common_log,
                3,
                &mut replay_rng,
                |column| Ok(native(native_log, column)),
            )
            .unwrap();
            let retained = aggregate::MaskedTracePolynomialSetV1::sample_columns_v1(
                native_log,
                common_log,
                3,
                MASK_DEGREE,
                &mut retained_rng,
                |column| Ok(native(native_log, column).into_vec_v1()),
            )
            .unwrap();
            assert_eq!(replay_rng.next_u64(), retained_rng.next_u64());
            let root = goldilocks_primitive_root_v1(native_log).unwrap();
            for column in 0..3 {
                let values = native(native_log, column);
                let coefficients = group.replay_v1(column, &values).unwrap();
                assert_eq!(
                    &*coefficients,
                    retained.column_coefficients_v1(column).unwrap()
                );
                assert_eq!(&*group.replay_v1(column, &values).unwrap(), &*coefficients);
                // The mask vanishes at each independently selected native point,
                // including when its degree exceeds the smallest native subgroup.
                for row in [0, 1, (1_usize << native_log) - 1] {
                    let point = root.pow(row as u128);
                    let actual = coefficients
                        .iter()
                        .rev()
                        .fold(F::ZERO, |value, &coefficient| {
                            value.mul(point).add(coefficient)
                        });
                    assert_eq!(actual, values[row]);
                }
            }
        }
    }

    #[test]
    fn mask_replay_rejects_invalid_shapes_and_preserves_entropy_on_source_failure() {
        let mut rng = StdRng::from_seed([71; 32]);
        let mut unchanged = rng.clone();
        let mut calls = 0;
        assert!(
            MainTraceMaskGroupV1::sample_v1(3, 3, 1, &mut rng, |_| {
                calls += 1;
                Ok(native(3, 0))
            })
            .is_err()
        );
        assert_eq!(calls, 0);
        assert_eq!(rng.next_u64(), unchanged.next_u64());
        for invalid in [
            vec![F::ONE; 7],
            vec![F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1); 8],
        ] {
            let mut rng = StdRng::from_seed([72; 32]);
            let mut unchanged = rng.clone();
            assert!(
                MainTraceMaskGroupV1::sample_v1(3, 11, 1, &mut rng, |_| Ok(
                    ZeroizingMainTraceColumnV1(invalid.clone())
                ))
                .is_err()
            );
            assert_eq!(rng.next_u64(), unchanged.next_u64());
        }
        let mut rng = StdRng::from_seed([73; 32]);
        let group =
            MainTraceMaskGroupV1::sample_v1(3, 11, 1, &mut rng, |_| Ok(native(3, 0))).unwrap();
        assert!(group.replay_v1(1, &native(3, 0)).is_err());
        assert!(group.replay_v1(0, &[F::ONE; 7]).is_err());
        let mut coefficients = group.replay_v1(0, &native(3, 0)).unwrap();
        coefficients.zeroize_private_v1();
        assert!(coefficients.is_empty());
        assert!(core::mem::needs_drop::<MainTraceMaskGroupV1>());
        assert!(core::mem::needs_drop::<ReplayableTraceMaskV1>());
    }
}

/// Immutable closed source roles for deterministic replay of original columns.
pub(super) enum MainTraceReplaySourcesV1<'phase, 'assembly> {
    Base {
        assembly: &'assembly ZkX509MainTraceAssemblyV1,
        sha: &'phase [ZkX509ShaBatchSegmentBaseSourceV1<'assembly>; ZK_X509_SHA_SEGMENT_COUNT_V1],
        p256: &'phase P256MainBaseSourceV1,
        projection: &'phase MainProjectionTraceGroupSourceV1<'assembly>,
        io: &'phase MainIoTraceGroupSourceV1<'assembly>,
    },
    Bound {
        log19: &'phase MainLog19BoundTraceGroupSourceV1<'assembly>,
        projection: &'phase MainProjectionTraceGroupSourceV1<'assembly>,
        io: &'phase MainIoTraceGroupSourceV1<'assembly>,
    },
}

impl MainTraceReplaySourcesV1<'_, '_> {
    fn native_column_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        registration: RegisteredSegmentLayoutV1,
        column: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let get = |source: &dyn MainTraceGroupSourceV1| match kind {
            MainTraceColumnKindV1::Base => source.native_base_column_v1(registration, column),
            MainTraceColumnKindV1::Aux => source.native_aux_column_v1(registration, column),
        };
        match self {
            Self::Base {
                assembly,
                sha,
                p256,
                projection,
                io,
            } => {
                if matches!(kind, MainTraceColumnKindV1::Aux) {
                    return Err(ZkX509StarkErrorV1::TranscriptMismatch);
                }
                match registration.trace_group {
                    0 => get(&MainP256Log5TraceGroupSourceV1::for_base_v1(layout, p256)?),
                    1 => get(&MainP256ScalarTraceGroupSourceV1::for_base_v1(
                        layout, p256,
                    )?),
                    2 => get(*projection),
                    3 => get(&MainP256Log16TraceGroupSourceV1::for_base_v1(layout, p256)?),
                    4 => get(*io),
                    5 => get(&MainLog19BaseTraceGroupSourceV1::for_main_v1(
                        layout, assembly, sha, p256,
                    )?),
                    _ => Err(ZkX509StarkErrorV1::ProfileMismatch),
                }
            }
            Self::Bound {
                log19,
                projection,
                io,
            } => match registration.trace_group {
                0 => get(&MainP256Log5TraceGroupSourceV1::for_bound_v1(
                    layout,
                    &log19.p256,
                )?),
                1 => get(&MainP256ScalarTraceGroupSourceV1::for_bound_v1(
                    layout,
                    &log19.p256,
                )?),
                2 => get(*projection),
                3 => get(&MainP256Log16TraceGroupSourceV1::for_bound_v1(
                    layout,
                    &log19.p256,
                )?),
                4 => get(*io),
                5 => get(*log19),
                _ => Err(ZkX509StarkErrorV1::ProfileMismatch),
            },
        }
    }
}
