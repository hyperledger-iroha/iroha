//! Private quartic SHA/RFC endpoint joins with an exact public source plan.
//!
//! Four physical SHA segments each contribute four byte streams per lane.
//! Sixteen constant-native RFC columns connect those segment products to the
//! four actual RFC consumer roles. Every join uses the original masked column.

use super::super::super::rfc5280_stark::zk_x509_rfc_sha_union_columns_v1;
use super::super::super::sha_call_bus_stark::{
    ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1, ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1,
};
use super::*;

pub(super) const UNION_QUOTIENTS_V1: usize = 20;
const DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:main-private-sha-union:v1";
const DESCRIPTOR_V1: &[u8] = b"four-segments-four-byte-streams-four-compression-lanes:16-constant-native-rfc-bridges:16-sha-quartic-endpoint-links+4-rfc-quartic-union-links:original-independent-masks:private-centers:no-extra-openings:no-private-product-inversion";
const NATIVE_LOG_V1: u8 = 19;
const NATIVE_ROWS_V1: usize = 1 << NATIVE_LOG_V1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ColumnV1 {
    group: usize,
    column: usize,
}

/// Only public column coordinates and native endpoints are retained here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MainShaUnionPlanV1 {
    bridges: [[ColumnV1; 4]; 4],
    consumers: [[ColumnV1; 4]; 4],
    streams: [[[ColumnV1; 4]; 4]; 4],
    points: [F; 5],
}

impl MainShaUnionPlanV1 {
    /// Conservative simultaneous public plan, alpha and temporary-copy charge.
    pub(super) const fn public_owner_charge_v1() -> usize {
        3 * core::mem::size_of::<Self>()
            + core::mem::size_of::<Vec<E>>()
            + UNION_QUOTIENTS_V1 * core::mem::size_of::<E>()
            + 1024
    }

    pub(super) fn new_v1(layout: &AggregateProofLayoutV1) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let rfc = layout.registered_segment(SegmentAdapterIdV1::Rfc5280, 0)?;
        let column = |registration: RegisteredSegmentLayoutV1, local: usize| {
            if registration.segment.trace_log2 != NATIVE_LOG_V1
                || local >= registration.segment.aux_width
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            Ok(ColumnV1 {
                group: registration.trace_group,
                column: registration
                    .aux_start
                    .checked_add(local)
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            })
        };
        let (bridge_columns, consumer_columns) = zk_x509_rfc_sha_union_columns_v1();
        let placeholder = column(rfc, 0)?;
        let mut bridges = [[placeholder; 4]; 4];
        let mut consumers = bridges;
        let mut streams = [bridges; 4];
        for index in 0..4 {
            for lane in 0..4 {
                bridges[index][lane] = column(rfc, bridge_columns[index][lane])?;
                consumers[index][lane] = column(rfc, consumer_columns[index][lane])?;
            }
            let sha = layout.registered_segment(SegmentAdapterIdV1::Sha256CallBus, index as u16)?;
            for stream in 0..4 {
                for lane in 0..4 {
                    streams[index][stream][lane] = column(
                        sha,
                        ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1 + stream * 4 + lane,
                    )?;
                }
            }
        }
        let root = goldilocks_primitive_root_v1(NATIVE_LOG_V1).map_err(map_transparent_error_v1)?;
        let mut points = [F::ONE; 5];
        for segment in 0..4 {
            let row = ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment]
                .checked_sub(1)
                .filter(|row| *row < NATIVE_ROWS_V1)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            points[segment] = root.pow(row as u128);
        }
        points[4] = root.pow((NATIVE_ROWS_V1 - 1) as u128);
        Ok(Self {
            bridges,
            consumers,
            streams,
            points,
        })
    }

    /// Bind the complete public source plan after all auxiliary commitments.
    pub(super) fn derive_alphas_v1(
        &self,
        transcript: &mut TransparentTranscriptV1,
    ) -> Result<Vec<E>, ZkX509StarkErrorV1> {
        transcript
            .absorb(DOMAIN_V1, &[DESCRIPTOR_V1])
            .map_err(map_transparent_error_v1)?;
        for (index, point) in self.points.iter().enumerate() {
            let mut record = [0_u8; 10];
            record[0] = index as u8;
            record[1] = NATIVE_LOG_V1;
            record[2..].copy_from_slice(&point.0.to_be_bytes());
            transcript
                .absorb(DOMAIN_V1, &[&record])
                .map_err(map_transparent_error_v1)?;
        }
        // Fixed bridge, role and segment/stream/lane ordering is unambiguous.
        for source in self
            .bridges
            .iter()
            .flatten()
            .chain(self.consumers.iter().flatten())
            .chain(self.streams.iter().flatten().flatten())
        {
            let mut record = [0_u8; 16];
            record[..8].copy_from_slice(&(source.group as u64).to_be_bytes());
            record[8..].copy_from_slice(&(source.column as u64).to_be_bytes());
            transcript
                .absorb(DOMAIN_V1, &[&record])
                .map_err(map_transparent_error_v1)?;
        }
        let mut alphas = Vec::new();
        alphas
            .try_reserve_exact(UNION_QUOTIENTS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if alphas.capacity() != UNION_QUOTIENTS_V1 {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        for _ in 0..UNION_QUOTIENTS_V1 {
            alphas.push(
                transcript
                    .challenge_fp4(DOMAIN_V1)
                    .map_err(map_transparent_error_v1)?,
            );
        }
        Ok(alphas)
    }

    #[cfg(test)]
    pub(super) fn source_coordinates_for_test_v1(&self) -> Vec<(usize, usize)> {
        self.bridges
            .iter()
            .flatten()
            .chain(self.consumers.iter().flatten())
            .chain(self.streams.iter().flatten().flatten())
            .map(|column| (column.group, column.column))
            .collect()
    }

    /// No new proof values: all factors are existing authenticated aux(z) cells.
    pub(super) fn evaluate_v1(
        &self,
        groups: &[aggregate::AggregateOpenedDeepTraceGroupV1],
        point: E,
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        self.evaluate_with_v1(point, alphas, |column| {
            groups
                .get(column.group)
                .and_then(|group| group.aux_current.get(column.column))
                .copied()
                .ok_or(ZkX509StarkErrorV1::ConstraintOpening)
        })
    }

    fn evaluate_with_v1(
        &self,
        point: E,
        alphas: &[E],
        value: impl Fn(ColumnV1) -> Result<E, ZkX509StarkErrorV1>,
    ) -> Result<E, ZkX509StarkErrorV1> {
        if !point.is_canonical()
            || alphas.len() != UNION_QUOTIENTS_V1
            || alphas.iter().any(|alpha| !alpha.is_canonical())
        {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        let checked = |column| {
            let result = value(column)?;
            if !result.is_canonical() {
                return Err(ZkX509StarkErrorV1::ConstraintOpening);
            }
            Ok(result)
        };
        let mut sum = E::ZERO;
        for segment in 0..4 {
            let inverse = point
                .sub(E::from_base(self.points[segment]))
                .inv()
                .ok_or(ZkX509StarkErrorV1::ConstraintOpening)?;
            for lane in 0..4 {
                let mut product = E::ONE;
                for stream in 0..4 {
                    product = product.mul(checked(self.streams[segment][stream][lane])?);
                }
                sum = sum.add(
                    alphas[segment * 4 + lane]
                        .mul(checked(self.bridges[segment][lane])?.sub(product))
                        .mul(inverse),
                );
            }
        }
        let inverse = point
            .sub(E::from_base(self.points[4]))
            .inv()
            .ok_or(ZkX509StarkErrorV1::ConstraintOpening)?;
        for lane in 0..4 {
            let mut consumer = E::ONE;
            let mut bridge = E::ONE;
            for index in 0..4 {
                consumer = consumer.mul(checked(self.consumers[index][lane])?);
                bridge = bridge.mul(checked(self.bridges[index][lane])?);
            }
            sum = sum.add(alphas[16 + lane].mul(consumer.sub(bridge)).mul(inverse));
        }
        Ok(sum)
    }
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl MainShaUnionPlanV1 {
    const QUOTIENT_LOG_V1: u8 = 22;
    const QUOTIENT_ROWS_V1: usize = 1 << Self::QUOTIENT_LOG_V1;
    const COEFFICIENT_COUNT_V1: usize = NATIVE_ROWS_V1 + MASK_DEGREE + 1;
    pub(super) const UNION_DEGREE_V1: usize = 4 * (Self::COEFFICIENT_COUNT_V1 - 1) - 1;

    /// The maximum of disjoint explicitly scoped phases, including an eight-
    /// column native replay scratch reserve. Common accumulators, source owners
    /// and runtime are already retained in the parent ledger. Device staging
    /// consumes only the policy allowance left after this reservation.
    pub(super) const fn private_owner_charge_v1() -> usize {
        let extension = Self::QUOTIENT_ROWS_V1 * core::mem::size_of::<E>();
        let replay = extension
            + 8 * Self::COEFFICIENT_COUNT_V1 * core::mem::size_of::<F>()
            + (2 + 8 + 8) * NATIVE_ROWS_V1 * core::mem::size_of::<F>()
            + ShaUnionEndpointDenominatorsV1::payload_charge_v1();
        let inverse = 2 * extension;
        let split = extension
            + COMPOSITION_DEGREE_CHUNKS
                * (589_824 * core::mem::size_of::<E>() + core::mem::size_of::<Vec<E>>());
        let peak = if replay > inverse { replay } else { inverse };
        let peak = if split > peak { split } else { peak };
        peak + 128 * core::mem::size_of::<Vec<F>>()
            + 64 * core::mem::size_of::<ZeroizingMainTraceColumnV1>()
            + 4096
    }

    /// Centered contributions reuse already resident registration columns.
    /// Only these full polynomial halves enter each registration's degree gate.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn local_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        row: usize,
        aux: &[Vec<F>],
        centers: &[[F; 4]; 4],
        alphas: &[E],
        denominators: &ShaUnionEndpointDenominatorsV1,
    ) -> Result<E, ZkX509StarkErrorV1> {
        let value = |column: ColumnV1| {
            if column.group != registration.trace_group {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            column
                .column
                .checked_sub(registration.aux_start)
                .and_then(|column| aux.get(column))
                .and_then(|column| column.get(row))
                .copied()
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
        };
        if alphas.len() != UNION_QUOTIENTS_V1 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut result = E::ZERO;
        match registration.segment.adapter {
            SegmentAdapterIdV1::Rfc5280 if registration.segment.instance == 0 => {
                for segment in 0..4 {
                    let inverse = denominators.at_v1(segment, row)?;
                    for lane in 0..4 {
                        result = result.add(
                            alphas[4 * segment + lane].mul_base(
                                value(self.bridges[segment][lane])?
                                    .sub(centers[segment][lane])
                                    .mul(inverse),
                            ),
                        );
                    }
                }
            }
            SegmentAdapterIdV1::Sha256CallBus if registration.segment.instance < 4 => {
                let segment = usize::from(registration.segment.instance);
                let inverse = denominators.at_v1(segment, row)?;
                for lane in 0..4 {
                    let mut product = F::ONE;
                    for stream in 0..4 {
                        product = product.mul(value(self.streams[segment][stream][lane])?);
                    }
                    result = result.add(
                        alphas[4 * segment + lane]
                            .mul_base(centers[segment][lane].sub(product).mul(inverse)),
                    );
                }
            }
            _ => return Err(ZkX509StarkErrorV1::ProfileMismatch),
        }
        Ok(result)
    }

    /// Four quartic unions use one log22 accumulator. Each lane caches its eight
    /// original masked RFC columns once. All 32 original generations and native
    /// IFFTs are charged; eight stripes give 256 native-size forward FFTs.
    /// Stripe folding retains every coefficient above native degree.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn accumulate_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        polynomials: &MainTracePolynomialSetV1,
        sources: &MainTraceReplaySourcesV1<'_, '_>,
        alphas: &[E],
        policy: main_bounded_transform::MainBoundedTransformPolicyV1,
        accumulator: &mut [Vec<Vec<E>>],
    ) -> Result<(), ZkX509StarkErrorV1> {
        use super::super::super::private_table::{PrivateTableV1, zeroize_field_rows_v1};
        use fastpq_prover::goldilocks_transform::{
            goldilocks_transform_completion_uncertain_v1, transform_goldilocks_columns_v1,
        };
        if *self != Self::new_v1(layout)?
            || alphas.len() != UNION_QUOTIENTS_V1
            || alphas.iter().any(|alpha| !alpha.is_canonical())
            || SECURITY_LANES != 1
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        main_bounded_transform::check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        let policy = policy.reserve_additional_v1(Self::private_owner_charge_v1())?;
        let shared = layout.as_shared()?;
        let cap = shared
            .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
            .map_err(map_aggregate_error_v1)?;
        if cap != 589_824 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let matrix = |count: usize| {
            let mut owner = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1::<Vec<F>>);
            owner
                .try_reserve_exact(count)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            if owner.capacity() != count {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            for _ in 0..count {
                let column = zeroed_main_trace_column_v1(NATIVE_ROWS_V1)?;
                if column.0.capacity() != NATIVE_ROWS_V1 {
                    return Err(ZkX509StarkErrorV1::ProofTooLarge);
                }
                owner.push(column.into_vec_v1());
            }
            Ok::<_, ZkX509StarkErrorV1>(owner)
        };
        let mut products = matrix(2)?;
        let mut batch = matrix(8)?;
        let mut quotient = ZeroizingExtensionColumnV1(Vec::new());
        quotient
            .0
            .try_reserve_exact(Self::QUOTIENT_ROWS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if quotient.0.capacity() != Self::QUOTIENT_ROWS_V1 {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        quotient.0.resize(Self::QUOTIENT_ROWS_V1, E::ZERO);
        for lane in 0..4 {
            let mut coefficients = Vec::new();
            coefficients
                .try_reserve_exact(8)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            if coefficients.capacity() != 8 {
                return Err(ZkX509StarkErrorV1::ProofTooLarge);
            }
            for column in self
                .bridges
                .iter()
                .chain(&self.consumers)
                .map(|columns| columns[lane])
            {
                let generated = polynomials.replay_columns_coefficients_v1(
                    layout,
                    MainTraceColumnKindV1::Aux,
                    column.group,
                    column.column..column.column + 1,
                    sources,
                    policy,
                )?;
                if generated.len() != 1
                    || generated.capacity() != 1
                    || generated.iter().any(|column| {
                        column.len() != Self::COEFFICIENT_COUNT_V1
                            || column.0.capacity() != column.len()
                    })
                {
                    return Err(ZkX509StarkErrorV1::ProofTooLarge);
                }
                coefficients.extend(generated);
            }
            for ordinal in 0..8 {
                let stripe = main_quotient_stripes::MainQuotientStripeV1::new_v1(
                    NATIVE_LOG_V1,
                    Self::QUOTIENT_LOG_V1,
                    ordinal,
                )?;
                let denominators = ShaUnionEndpointDenominatorsV1::new_v1(stripe)?;
                for product in products.iter_mut() {
                    product.fill(F::ONE);
                }
                main_bounded_transform::check_completion_v1(
                    goldilocks_transform_completion_uncertain_v1(),
                )?;
                batch
                    .par_iter_mut()
                    .zip(coefficients.par_iter())
                    .try_for_each(|(output, source)| stripe.fold_into_v1(source, output))?;
                policy.forward_with_v1(
                    &mut batch,
                    stripe.root,
                    &mut |words: &mut [Vec<u64>], root, direction| {
                        transform_goldilocks_columns_v1(
                            words,
                            root,
                            direction,
                            fastpq_prover::ExecutionMode::Auto,
                        )
                    },
                    &mut goldilocks_transform_completion_uncertain_v1,
                )?;
                for (index, source) in batch.iter().enumerate() {
                    products[index / 4]
                        .par_iter_mut()
                        .zip(source.par_iter())
                        .for_each(|(target, factor)| *target = target.mul(*factor));
                }
                quotient
                    .0
                    .par_chunks_mut(stripe.count)
                    .enumerate()
                    .try_for_each(|(row, entries)| {
                        let inverse = denominators.at_v1(4, row)?;
                        entries[ordinal] = entries[ordinal].add(
                            alphas[16 + lane]
                                .mul_base(products[1][row].sub(products[0][row]).mul(inverse)),
                        );
                        Ok::<_, ZkX509StarkErrorV1>(())
                    })?;
            }
            // The next compression lane cannot coexist with this private cache.
            drop(coefficients);
        }
        // Release every original coefficient/source/product owner before the
        // Fp4 inverse and chunk allocation. Unknown device completion is fatal.
        drop(products);
        drop(batch);
        main_bounded_transform::check_completion_v1(goldilocks_transform_completion_uncertain_v1())?;
        let inverse = fp4_coset_coefficients_v1(&quotient, Self::QUOTIENT_LOG_V1)?;
        drop(quotient);
        if inverse.0.capacity() != Self::QUOTIENT_ROWS_V1 {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let contribution = ZeroizingExtensionLanesV1::new(
            vec![composition_coefficient_chunks_v1(
                &inverse,
                Self::UNION_DEGREE_V1,
                &shared,
            )?],
            zeroize_extension_lanes_v1,
        );
        // The existing sink validates every destination and degree first;
        // nothing is published on a replay, transform, degree or capacity error.
        add_main_composition_coefficient_chunks_v1(accumulator, &contribution, cap)
    }
}

/// One public N-row table serves all five endpoint denominators on a stripe.
/// Prefix multiplication and reverse reconstruction require no second vector.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) struct ShaUnionEndpointDenominatorsV1 {
    inverses: Vec<F>,
    endpoint_rows: [usize; 5],
    endpoint_inverses: [F; 5],
}

#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ShaUnionEndpointDenominatorsV1 {
    pub(super) const fn payload_charge_v1() -> usize {
        core::mem::size_of::<Self>() + NATIVE_ROWS_V1 * core::mem::size_of::<F>()
    }

    pub(super) fn new_v1(
        stripe: main_quotient_stripes::MainQuotientStripeV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        let full_rows = stripe
            .rows
            .checked_mul(stripe.count)
            .filter(|rows| rows.is_power_of_two())
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let expected = main_quotient_stripes::MainQuotientStripeV1::new_v1(
            NATIVE_LOG_V1,
            full_rows.ilog2() as u8,
            stripe.ordinal,
        )?;
        if stripe.rows != NATIVE_ROWS_V1
            || stripe.rows != expected.rows
            || stripe.count != expected.count
            || stripe.next_stride != expected.next_stride
            || stripe.root != expected.root
            || stripe.shift != expected.shift
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Self::from_public_coset_v1(
            stripe.rows,
            stripe.root,
            stripe.shift,
            [
                ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0] - 1,
                ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[1] - 1,
                ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[2] - 1,
                ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[3] - 1,
                NATIVE_ROWS_V1 - 1,
            ],
        )
    }

    fn from_public_coset_v1(
        rows: usize,
        root: F,
        shift: F,
        endpoint_rows: [usize; 5],
    ) -> Result<Self, ZkX509StarkErrorV1> {
        if rows < 2
            || !rows.is_power_of_two()
            || rows > NATIVE_ROWS_V1
            || root
                != goldilocks_primitive_root_v1(rows.ilog2() as u8)
                    .map_err(map_transparent_error_v1)?
            || !shift.is_canonical()
            || shift == F::ZERO
            || shift.pow(rows as u128) == F::ONE
            || endpoint_rows.iter().any(|row| *row >= rows)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut inverses = Vec::new();
        inverses
            .try_reserve_exact(rows)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        if inverses.capacity() != rows {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let mut product = F::ONE;
        let mut x = shift;
        for _ in 0..rows {
            product = product.mul(x.sub(F::ONE));
            inverses.push(product);
            x = x.mul(root);
        }
        let mut inverse = product.inv().ok_or(ZkX509StarkErrorV1::ConstraintOpening)?;
        let root_inverse = root.inv().ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        x = shift.mul(root_inverse);
        for row in (0..rows).rev() {
            let prefix = if row == 0 { F::ONE } else { inverses[row - 1] };
            inverses[row] = inverse.mul(prefix);
            inverse = inverse.mul(x.sub(F::ONE));
            x = x.mul(root_inverse);
        }
        if inverse != F::ONE {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(Self {
            inverses,
            endpoint_rows,
            endpoint_inverses: endpoint_rows.map(|row| root_inverse.pow(row as u128)),
        })
    }

    pub(super) fn at_v1(&self, endpoint: usize, row: usize) -> Result<F, ZkX509StarkErrorV1> {
        let endpoint_row = *self
            .endpoint_rows
            .get(endpoint)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let rows = self.inverses.len();
        if row >= rows {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let shifted = (row + rows - endpoint_row) % rows;
        Ok(self.endpoint_inverses[endpoint].mul(self.inverses[shifted]))
    }
}

#[cfg(test)]
#[path = "main_sha_union_tests.rs"]
mod tests;
