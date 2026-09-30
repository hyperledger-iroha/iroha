//! Bounded test observers of the existing bound P-256 row providers.

use super::*;

fn visit_selected_base<const WIDTH: usize>(
    indices: &[usize],
    mut row: impl FnMut(usize) -> Result<[F; WIDTH], P256AggregateAdapterErrorV1>,
    visitor: &mut impl FnMut(usize, &[F]),
) -> Result<(), P256AggregateAdapterErrorV1> {
    for &index in indices {
        let values = zeroize::Zeroizing::new(row(index)?);
        visitor(index, &values[..]);
    }
    Ok(())
}

fn visit_selected_aux<const WIDTH: usize>(
    size: usize,
    indices: &[usize],
    mut next: impl FnMut() -> Result<Option<[F; WIDTH]>, P256AggregateAdapterErrorV1>,
    visitor: &mut impl FnMut(usize, &[F]),
) -> Result<(), P256AggregateAdapterErrorV1> {
    let mut selected = 0;
    for index in 0..size {
        let values = zeroize::Zeroizing::new(next()?.ok_or(P256AggregateAdapterErrorV1::Topology)?);
        if indices.get(selected) == Some(&index) {
            visitor(index, &values[..]);
            selected += 1;
        }
    }
    let extra = next()?.map(zeroize::Zeroizing::new);
    if extra.is_some() || selected != indices.len() {
        return Err(P256AggregateAdapterErrorV1::Topology);
    }
    Ok(())
}

impl P256MainBoundSourceV1 {
    /// Observe selected native rows using the same provider/stream objects as
    /// column replay, without an eager matrix or per-column replay multiplier.
    pub(crate) fn visit_native_boundary_rows_for_test_v1(
        &self,
        registration: P256MainRegistrationV1,
        indices: &[usize],
        mut base_visitor: impl FnMut(usize, &[F]),
        mut aux_visitor: impl FnMut(usize, &[F]),
    ) -> Result<(), P256AggregateAdapterErrorV1> {
        let shape = registration.shape_v1()?;
        if indices.is_empty()
            || indices.windows(2).any(|pair| pair[0] >= pair[1])
            || indices.last().copied().unwrap() >= shape.trace_size
        {
            return Err(P256AggregateAdapterErrorV1::Topology);
        }
        let signature = self.signature_v1(registration)?;
        let post_base = self.post_base_v1()?;
        match (registration.adapter_v1(), registration.local_instance_v1()) {
            (P256MainAdapterV1::ValueBus, local @ 0..=1) => {
                let value = signature
                    .value
                    .as_ref()
                    .ok_or(P256AggregateAdapterErrorV1::Phase)?;
                let rows = if local == 0 {
                    value.execution_base_rows_v1()
                } else {
                    value.sorted_base_rows_v1()
                }?;
                visit_selected_base(
                    indices,
                    |index| Ok(rows.base_row_v1(index)?),
                    &mut base_visitor,
                )?;
                if local == 0 {
                    let mut stream = P256ValueExecutionAggregateStreamV1::new_v1(value)?;
                    visit_selected_aux(
                        shape.trace_size,
                        indices,
                        || stream.next_aux_row_v1(),
                        &mut aux_visitor,
                    )
                } else {
                    let mut stream = value.sorted_aux_source_v1()?;
                    visit_selected_aux(
                        shape.trace_size,
                        indices,
                        || Ok(stream.next_aux_row_v1()?),
                        &mut aux_visitor,
                    )
                }
            }
            (P256MainAdapterV1::Arithmetic, 0) => {
                let rows = signature
                    .arithmetic
                    .as_ref()
                    .ok_or(P256AggregateAdapterErrorV1::Phase)?
                    .rows_v1(signature.role)?;
                visit_selected_base(indices, |index| rows.base_row_v1(index), &mut base_visitor)?;
                let mut stream = self.arithmetic_aux_stream_v1(registration)?;
                visit_selected_aux(
                    shape.trace_size,
                    indices,
                    || stream.next_aux_row_v1(),
                    &mut aux_visitor,
                )
            }
            (P256MainAdapterV1::WindowBatch, 0) => {
                let trace = signature
                    .window
                    .as_ref()
                    .ok_or(P256AggregateAdapterErrorV1::Phase)?;
                let rows = P256WindowAggregateRowsV1::new_v1(trace)?;
                visit_selected_base(indices, |index| rows.base_row_v1(index), &mut base_visitor)?;
                let start = self
                    .cross_claim_v1(registration, P256CrossTraceTerminalRoleV1::WindowBatch)?
                    .start;
                let mut stream = P256WindowAggregateAuxStreamV1::new_v1(
                    trace,
                    start,
                    post_base.p256_cross(),
                    post_base.p256_scalar(),
                )?;
                visit_selected_aux(
                    shape.trace_size,
                    indices,
                    || stream.next_aux_row_v1(),
                    &mut aux_visitor,
                )
            }
            (P256MainAdapterV1::Reduction, local @ 0..=1) => {
                let (role, claim_role, trace) = if local == 0 {
                    (
                        P256ReductionAggregateRoleV1::Digest,
                        P256CrossTraceTerminalRoleV1::DigestReduction,
                        signature.digest_reduction.as_ref(),
                    )
                } else {
                    (
                        P256ReductionAggregateRoleV1::ResultX,
                        P256CrossTraceTerminalRoleV1::ResultXReduction,
                        signature.result_x_reduction.as_ref(),
                    )
                };
                let trace = trace.ok_or(P256AggregateAdapterErrorV1::Phase)?;
                let rows = P256ReductionAggregateRowsV1::new_v1(role, trace)?;
                visit_selected_base(indices, |index| rows.base_row_v1(index), &mut base_visitor)?;
                let start = self.cross_claim_v1(registration, claim_role)?.start;
                let mut stream = P256ReductionAggregateAuxStreamV1::new_v1(
                    role,
                    trace,
                    start,
                    post_base.p256_cross(),
                )?;
                visit_selected_aux(
                    shape.trace_size,
                    indices,
                    || stream.next_aux_row_v1(),
                    &mut aux_visitor,
                )
            }
            (P256MainAdapterV1::WalletLowS, 0) => {
                let trace = signature
                    .low_s
                    .as_ref()
                    .ok_or(P256AggregateAdapterErrorV1::Phase)?;
                let rows = P256LowSAggregateRowsV1::new_v1(signature.role, trace)?;
                visit_selected_base(indices, |index| rows.base_row_v1(index), &mut base_visitor)?;
                let start = self
                    .cross_claim_v1(registration, P256CrossTraceTerminalRoleV1::WalletLowS)?
                    .start;
                let mut stream = P256LowSAggregateAuxStreamV1::new_v1(
                    signature.role,
                    trace,
                    start,
                    post_base.p256_cross(),
                )?;
                visit_selected_aux(
                    shape.trace_size,
                    indices,
                    || stream.next_aux_row_v1(),
                    &mut aux_visitor,
                )
            }
            (P256MainAdapterV1::BindingSink, 0) => {
                let trace = signature
                    .sink
                    .as_ref()
                    .ok_or(P256AggregateAdapterErrorV1::Phase)?;
                let rows = P256BindingSinkRowsV1::new_v1(trace)?;
                visit_selected_base(indices, |index| rows.base_row_v1(index), &mut base_visitor)?;
                let mut stream =
                    P256BindingSinkAggregateStreamV1::new_with_optional_certificate_v1(
                        trace,
                        post_base.p256_cross(),
                        registration.signature_v1() == 2,
                    )?;
                visit_selected_aux(
                    shape.trace_size,
                    indices,
                    || stream.next_aux_row_v1(),
                    &mut aux_visitor,
                )
            }
            (P256MainAdapterV1::ScalarBitBus, 0) => {
                let scalar = signature
                    .scalar
                    .as_ref()
                    .ok_or(P256AggregateAdapterErrorV1::Phase)?;
                let rows = scalar.base_rows_v1()?;
                visit_selected_base(
                    indices,
                    |index| Ok(rows.base_row_v1(index)?),
                    &mut base_visitor,
                )?;
                let mut stream = scalar.aux_source_v1()?;
                visit_selected_aux(
                    shape.trace_size,
                    indices,
                    || Ok(stream.next_aux_row_v1()?),
                    &mut aux_visitor,
                )
            }
            _ => Err(P256AggregateAdapterErrorV1::Topology),
        }
    }
}
