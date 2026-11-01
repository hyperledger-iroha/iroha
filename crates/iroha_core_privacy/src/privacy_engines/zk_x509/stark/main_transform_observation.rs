//! Canonical public relation attribution for each unchanged joined transform batch.

use super::super::prover_observation::transform::{
    ExpectedV1, FamilyV1, InvalidAttributionV1, KindV1,
};
use super::*;
use crate::privacy_engines::aggregate_stark::joined_trace::JoinedTraceBatchCoordinatesV1;

/// Map the already validated MAIN challenge phase to its diagnostic identity.
pub(super) const fn kind_v1(kind: main_aggregate::MainTraceColumnKindV1) -> KindV1 {
    match kind {
        main_aggregate::MainTraceColumnKindV1::Base => KindV1::Base,
        main_aggregate::MainTraceColumnKindV1::Aux => KindV1::Aux,
    }
}

fn registration_family_v1(
    registration: RegisteredSegmentLayoutV1,
) -> Result<FamilyV1, InvalidAttributionV1> {
    let family = match registration.segment.adapter {
        SegmentAdapterIdV1::ByteMemory => FamilyV1::ByteMemory,
        SegmentAdapterIdV1::StrictDer => FamilyV1::StrictDer,
        SegmentAdapterIdV1::Rfc5280 => FamilyV1::Rfc5280,
        SegmentAdapterIdV1::Sha256CallBus => FamilyV1::Sha256CallBus,
        SegmentAdapterIdV1::Projection => FamilyV1::Projection,
        SegmentAdapterIdV1::CaAccumulator => return Err(InvalidAttributionV1),
        _ => match p256_main_registration_from_main_layout_v1(registration)
            .map_err(|_| InvalidAttributionV1)?
            .adapter_v1()
        {
            P256MainAdapterV1::Arithmetic => FamilyV1::P256Arithmetic,
            P256MainAdapterV1::Reduction => FamilyV1::P256Reduction,
            P256MainAdapterV1::WalletLowS => FamilyV1::P256WalletLowS,
            P256MainAdapterV1::WindowBatch => FamilyV1::P256WindowBatch,
            P256MainAdapterV1::BindingSink => FamilyV1::P256BindingSink,
            P256MainAdapterV1::ValueBus => FamilyV1::P256ValueBus,
            P256MainAdapterV1::ScalarBitBus => FamilyV1::P256ScalarBitBus,
        },
    };
    Ok(family)
}

/// Classify only public coordinates from the original batch.
///
/// The caller's evaluator constructor has already validated the exact immutable
/// MAIN layout. Repeating registered_main_group_column_v1 here would reconstruct
/// that entire layout once per column. This bounded scan checks local coverage
/// without allocating, reading coefficients, or changing any evaluator refusal.
pub(super) fn classify_v1(
    layout: &AggregateProofLayoutV1,
    kind: KindV1,
    coordinates: JoinedTraceBatchCoordinatesV1,
    native: u8,
    common: u8,
    count: usize,
) -> Result<FamilyV1, InvalidAttributionV1> {
    let group = layout
        .trace_groups
        .get(coordinates.group)
        .ok_or(InvalidAttributionV1)?;
    let width = match kind {
        KindV1::Base => group.base_width,
        KindV1::Aux => group.aux_width,
    };
    if native != group.native_trace_log2
        || common != layout.common_lde_log2
        || count == 0
        || count > aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        || coordinates.start >= width
        || !coordinates
            .start
            .is_multiple_of(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
        || coordinates.end.checked_sub(coordinates.start) != Some(count)
        || coordinates
            .start
            .checked_add(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1)
            .map(|end| end.min(width))
            != Some(coordinates.end)
    {
        return Err(InvalidAttributionV1);
    }
    let mut family = None;
    let mut mixed = false;
    for column in coordinates.start..coordinates.end {
        let mut matched = None;
        for registration in layout
            .registered_segments
            .iter()
            .copied()
            .filter(|registration| registration.trace_group == coordinates.group)
        {
            let (start, end) = match kind {
                KindV1::Base => (registration.base_start, registration.base_end()),
                KindV1::Aux => (registration.aux_start, registration.aux_end()),
            };
            let end = end.map_err(|_| InvalidAttributionV1)?;
            if (start..end).contains(&column) {
                if registration.segment.trace_log2 != native
                    || matched.replace(registration).is_some()
                {
                    return Err(InvalidAttributionV1);
                }
            }
        }
        let current = registration_family_v1(matched.ok_or(InvalidAttributionV1)?)?;
        if let Some(first) = family {
            mixed |= first != current;
        } else {
            family = Some(current);
        }
    }
    if mixed {
        Ok(FamilyV1::MixedRelations)
    } else {
        family.ok_or(InvalidAttributionV1)
    }
}

/// Derive expected calls and columns from the verifier-owned complete registry.
/// This runs outside proving when the diagnostic checks its retained record.
pub(in super::super) fn expected_v1() -> Result<ExpectedV1, InvalidAttributionV1> {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().map_err(|_| InvalidAttributionV1)?;
    let mut expected = ExpectedV1::default();
    for kind in [KindV1::Base, KindV1::Aux] {
        for (group_index, group) in layout.trace_groups.iter().enumerate() {
            let width = match kind {
                KindV1::Base => group.base_width,
                KindV1::Aux => group.aux_width,
            };
            for start in (0..width).step_by(aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
                let end = (start + aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1).min(width);
                let family = classify_v1(
                    &layout,
                    kind,
                    JoinedTraceBatchCoordinatesV1 {
                        group: group_index,
                        start,
                        end,
                    },
                    group.native_trace_log2,
                    layout.common_lde_log2,
                    end - start,
                )?;
                let tally = &mut expected.0[kind as usize][family as usize];
                tally.calls = tally.calls.checked_add(1).ok_or(InvalidAttributionV1)?;
                tally.columns = tally
                    .columns
                    .checked_add(u64::try_from(end - start).map_err(|_| InvalidAttributionV1)?)
                    .ok_or(InvalidAttributionV1)?;
            }
        }
    }
    Ok(expected)
}

#[test]
fn canonical_transform_family_census_matches_all_original_batches() {
    use super::super::prover_observation::transform::{FAMILIES, TallyV1};
    // Independent literal pins in the closed family order, not classifier output.
    let base = [
        (4, 28),
        (5, 40),
        (131, 1048),
        (15, 118),
        (70, 560),
        (4, 30),
        (42, 336),
        (5, 36),
        (38, 304),
        (3, 17),
        (35, 280),
        (43, 344),
        (9, 72),
    ];
    let aux = [
        (5, 39),
        (6, 48),
        (44, 352),
        (23, 183),
        (23, 184),
        (20, 160),
        (80, 636),
        (2, 12),
        (23, 184),
        (5, 36),
        (34, 272),
        (40, 320),
        (24, 192),
    ];
    assert_eq!(
        kind_v1(main_aggregate::MainTraceColumnKindV1::Base),
        KindV1::Base
    );
    assert_eq!(
        kind_v1(main_aggregate::MainTraceColumnKindV1::Aux),
        KindV1::Aux
    );
    let expected = expected_v1().unwrap();
    for (kind, pins) in [(KindV1::Base, base), (KindV1::Aux, aux)] {
        for (family, (calls, columns)) in FAMILIES.into_iter().zip(pins) {
            assert_eq!(
                expected.0[kind as usize][family as usize],
                TallyV1 { calls, columns },
                "{kind:?}/{family:?}"
            );
        }
    }
    assert_eq!(
        expected.total(KindV1::Base),
        Some(TallyV1 {
            calls: 404,
            columns: 3213
        })
    );
    assert_eq!(
        expected.total(KindV1::Aux),
        Some(TallyV1 {
            calls: 329,
            columns: 2618
        })
    );
    let totals = [KindV1::Base, KindV1::Aux].map(|kind| expected.total(kind).unwrap());
    assert_eq!(2 * (totals[0].calls + totals[1].calls), 1466);
    assert_eq!(2 * (totals[0].columns + totals[1].columns), 11662);
    assert_eq!(main_diagnostic_transform_columns_v1().unwrap(), 11662);
}

#[test]
fn every_mixed_range_and_same_family_registration_crossing_is_honest() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let mut mixed = Vec::new();
    for kind in [KindV1::Base, KindV1::Aux] {
        for (group_index, group) in layout.trace_groups.iter().enumerate() {
            let width = match kind {
                KindV1::Base => group.base_width,
                KindV1::Aux => group.aux_width,
            };
            for start in (0..width).step_by(8) {
                let end = (start + 8).min(width);
                let family = classify_v1(
                    &layout,
                    kind,
                    JoinedTraceBatchCoordinatesV1 {
                        group: group_index,
                        start,
                        end,
                    },
                    group.native_trace_log2,
                    layout.common_lde_log2,
                    end - start,
                )
                .unwrap();
                if family == FamilyV1::MixedRelations {
                    mixed.push((kind, group.native_trace_log2, start, end));
                }
            }
        }
    }
    assert_eq!(
        mixed,
        vec![
            (KindV1::Base, 16, 304, 312),
            (KindV1::Base, 19, 72, 80),
            (KindV1::Base, 19, 360, 368),
            (KindV1::Base, 19, 712, 720),
            (KindV1::Base, 19, 1768, 1776),
            (KindV1::Aux, 5, 184, 192),
            (KindV1::Aux, 16, 184, 192),
            (KindV1::Aux, 19, 192, 200),
            (KindV1::Aux, 19, 472, 480),
            (KindV1::Aux, 19, 800, 808),
            (KindV1::Aux, 19, 1160, 1168),
        ]
    );
    let group = layout
        .trace_groups
        .iter()
        .position(|group| group.native_trace_log2 == 16)
        .unwrap();
    for (start, family) in [
        (56, FamilyV1::P256WindowBatch),
        (296, FamilyV1::P256WindowBatch),
        (304, FamilyV1::MixedRelations),
        (312, FamilyV1::P256BindingSink),
    ] {
        assert_eq!(
            classify_v1(
                &layout,
                KindV1::Base,
                JoinedTraceBatchCoordinatesV1 {
                    group,
                    start,
                    end: start + 8
                },
                16,
                22,
                8
            ),
            Ok(family)
        );
    }
    // Independently resolve every member of the important original mixed batch.
    // The source validates the entire layout; the hot classifier does not repeat it.
    for column in 304..312 {
        let registration = layout
            .registered_segments
            .iter()
            .copied()
            .find(|registration| {
                registration.trace_group == group
                    && (registration.base_start..registration.base_end().unwrap()).contains(&column)
            })
            .unwrap();
        let adapter = p256_main_registration_from_main_layout_v1(registration)
            .unwrap()
            .adapter_v1();
        assert_eq!(
            adapter,
            if column == 304 {
                P256MainAdapterV1::WindowBatch
            } else {
                P256MainAdapterV1::BindingSink
            }
        );
    }
}

#[test]
fn invalid_public_batch_metadata_and_coverage_never_become_mixed() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let coordinates = JoinedTraceBatchCoordinatesV1 {
        group: 3,
        start: 304,
        end: 312,
    };
    assert_eq!(
        classify_v1(&layout, KindV1::Base, coordinates, 16, 22, 8),
        Ok(FamilyV1::MixedRelations)
    );
    for (group, start, end, native, common, count) in [
        (usize::MAX, 304, 312, 16, 22, 8),
        (3, 304, 304, 16, 22, 0),
        (3, 312, 304, 16, 22, 8),
        (3, 305, 313, 16, 22, 8),
        (3, 304, 313, 16, 22, 9),
        (3, 304, 311, 16, 22, 7),
        (3, 304, 312, 15, 22, 8),
        (3, 304, 312, 16, 21, 8),
        (3, 304, 312, 16, 22, 7),
        (3, usize::MAX - 7, usize::MAX, 16, 22, 7),
    ] {
        assert_eq!(
            classify_v1(
                &layout,
                KindV1::Base,
                JoinedTraceBatchCoordinatesV1 { group, start, end },
                native,
                common,
                count
            ),
            Err(InvalidAttributionV1)
        );
    }
    let sink = layout
        .registered_segments
        .iter()
        .position(|registration| registration.trace_group == 3 && registration.base_start == 305)
        .unwrap();
    for mutation in 0..4 {
        let mut malformed = layout.clone();
        match mutation {
            0 => {
                malformed.registered_segments.remove(sink);
            }
            1 => {
                malformed
                    .registered_segments
                    .push(malformed.registered_segments[sink]);
            }
            2 => {
                malformed.registered_segments[sink].segment.instance = u16::MAX;
            }
            3 => {
                malformed.registered_segments[sink].segment.adapter =
                    SegmentAdapterIdV1::CaAccumulator;
            }
            _ => unreachable!(),
        }
        assert!(
            malformed
                .validate_exact_full_profile_registration_v1()
                .is_err()
        );
        assert_eq!(
            classify_v1(&malformed, KindV1::Base, coordinates, 16, 22, 8),
            Err(InvalidAttributionV1)
        );
    }
}
