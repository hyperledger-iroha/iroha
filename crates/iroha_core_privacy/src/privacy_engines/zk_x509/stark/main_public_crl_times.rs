//! Exact retained CRL timestamps joined to their original RFC calendar column.
use super::*;
use crate::privacy_engines::zk_x509::rfc5280_stark::zk_x509_rfc_crl_time_join_rows_v1;

pub(super) const PUBLIC_TIME_BLOCKS_V1: usize = 2;
const DOMAIN: &[u8] = b"iroha:privacy:zk-x509:authoritative-crl-time-endpoints:v1";
const DESCRIPTOR: &[u8] = b"retained-crl-thisUpdate,nextUpdate:rfc-original-calendar-slots6,7-final-phase6:required-activity:original-timestamp-column:two-single-native-root-quotients:no-new-DEEP-opening:expected-values-from-authoritative-record";

pub(super) struct MainPublicCrlTimePlanV1 {
    blocks: [BlockV1; PUBLIC_TIME_BLOCKS_V1],
    values: [F; PUBLIC_TIME_BLOCKS_V1],
}
impl MainPublicCrlTimePlanV1 {
    pub(super) fn new_v1(
        layout: &AggregateProofLayoutV1,
        shape: ZkX509Rfc5280StarkShapeV1,
        public: ZkX509CrlPublicInputsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        if public.this_update >= public.next_update
            || public.next_update > iroha_data_model::privacy::ZK_X509_MAX_UNIX_SECONDS_V1
        {
            return Err(ZkX509StarkErrorV1::InvalidStatement);
        }
        let (rows, column) = zk_x509_rfc_crl_time_join_rows_v1(shape)
            .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let registration = layout.registered_segment(SegmentAdapterIdV1::Rfc5280, 0)?;
        if registration.segment.trace_log2 != 19 || column >= registration.segment.base_width {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let source = ColumnV1 {
            group: registration.trace_group,
            column: registration
                .base_start
                .checked_add(column)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            log: 19,
        };
        let root = goldilocks_primitive_root_v1(19).map_err(map_transparent_error_v1)?;
        let blocks = rows.map(|row| BlockV1 {
            source,
            target: source,
            start: root.pow(row as u128),
            step: root,
            count: 1,
            power: 1,
            scale: F::ONE,
        });
        let shared = layout.as_shared()?;
        let geometry =
            crate::privacy_engines::zk_x509::composition_masking::QuotientChunkGeometryV1::new_v1(
                &shared,
                AGGREGATE_PARAMETERS_V1,
            )
            .map_err(map_aggregate_error_v1)?;
        if geometry
            .stride_v1()
            .checked_mul(AGGREGATE_PARAMETERS_V1.composition_degree_chunks)
            .is_none_or(|coverage| (1usize << 19) + MASK_DEGREE > coverage)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            blocks,
            values: [F(public.this_update), F(public.next_update)],
        })
    }
    pub(super) fn source_v1(&self) -> ColumnV1 {
        self.blocks[0].source
    }
    pub(super) fn absorb_plan_v1(
        &self,
        transcript: &mut TransparentTranscriptV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        transcript
            .absorb(DOMAIN, &[DESCRIPTOR])
            .map_err(map_transparent_error_v1)?;
        for (i, b) in self.blocks.iter().enumerate() {
            let mut bytes = [0u8; 6 * 8];
            for (j, value) in [
                i as u64,
                b.source.group as u64,
                b.source.column as u64,
                u64::from(b.source.log),
                b.start.0,
                self.values[i].0,
            ]
            .into_iter()
            .enumerate()
            {
                bytes[8 * j..8 * j + 8].copy_from_slice(&value.to_be_bytes());
            }
            transcript
                .absorb(DOMAIN, &[&bytes])
                .map_err(map_transparent_error_v1)?;
        }
        Ok(())
    }
    pub(super) fn evaluate_v1(
        &self,
        groups: &[aggregate::AggregateOpenedDeepTraceGroupV1],
        z: E,
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if alphas.len() != PUBLIC_TIME_BLOCKS_V1
            || !z.is_canonical()
            || alphas.iter().any(|v| !v.is_canonical())
        {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        let mut sum = E::ZERO;
        for (i, b) in self.blocks.iter().enumerate() {
            let actual = groups
                .get(b.source.group)
                .and_then(|g| g.base_current.get(b.source.column))
                .copied()
                .ok_or(ZkX509StarkErrorV1::ConstraintOpening)?;
            if !actual.is_canonical() {
                return Err(ZkX509StarkErrorV1::NonCanonicalField);
            }
            sum = sum.add(
                alphas[i].mul(actual.sub(E::from_base(self.values[i]))).mul(
                    z.sub(E::from_base(b.start))
                        .inv()
                        .ok_or(ZkX509StarkErrorV1::ConstraintOpening)?,
                ),
            );
        }
        Ok(sum)
    }
    pub(super) fn quotient_v1(
        &self,
        index: usize,
        source: &[F],
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let block = *self
            .blocks
            .get(index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        MainKeyJoinPlanV1::quotient_v1(block, source, &[self.values[index]])
    }
}
#[cfg(test)]
#[path = "main_public_crl_times_tests.rs"]
mod tests;
