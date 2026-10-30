//! Exact public digest endpoints for the retained authoritative CRL revision.
//!
//! Each block divides the original masked SHA word polynomial minus the unique
//! degree-seven interpolation of the authoritative digest by its eight actual
//! native digest-row roots. No witness-side hash check or new opening is used.

use super::*;
use crate::privacy_engines::zk_x509::sha_call_bus_stark::{
    ZK_X509_SHA_SEGMENT_ROWS_V1, ZkX509ShaCallActivationV1, ZkX509ShaCallPublicShapeV1,
    ZkX509ShaCallRoleV1, ZkX509ShaCallScheduleV1,
};

pub(super) const PUBLIC_DIGEST_BLOCKS_V1: usize = 2;
const PUBLIC_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:authoritative-crl-sha-endpoints:v1";
const PUBLIC_DIGEST_DESCRIPTOR_V1: &[u8] = b"calls4,12:required-complete-crl-and-issuer-spki:authoritative-retained-crl-revision-digests:8-big-endian-u32-each:sha-original-base-column0:actual-fixed-final-eight-rows:degree7-public-interpolation:eight-distinct-native-roots:two-unconditional-original-polynomial-quotients:no-extra-DEEP-opening:no-private-value-publication";
const CALLS_V1: [usize; PUBLIC_DIGEST_BLOCKS_V1] = [4, 12];

/// A small public interpolation plan, independent of private candidate arrays.
pub(super) struct MainPublicShaDigestPlanV1 {
    blocks: [BlockV1; PUBLIC_DIGEST_BLOCKS_V1],
    public_coefficients: [[F; 8]; PUBLIC_DIGEST_BLOCKS_V1],
}
impl MainPublicShaDigestPlanV1 {
    pub(super) fn new_v1(
        layout: &AggregateProofLayoutV1,
        disclosures: usize,
        public: ZkX509CrlPublicInputsV1,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosures,
        })
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
        let root = goldilocks_primitive_root_v1(19).map_err(map_transparent_error_v1)?;
        let dummy = ColumnV1 {
            group: 0,
            column: 0,
            log: 19,
        };
        let mut blocks = [BlockV1 {
            source: dummy,
            target: dummy,
            start: F::ONE,
            step: root,
            count: 8,
            power: 1,
            scale: F::ONE,
        }; PUBLIC_DIGEST_BLOCKS_V1];
        let mut public_coefficients = [[F::ZERO; 8]; PUBLIC_DIGEST_BLOCKS_V1];
        for (index, call) in CALLS_V1.into_iter().enumerate() {
            let manifest = schedule
                .calls()
                .get(call)
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            let expected_role = if index == 0 {
                ZkX509ShaCallRoleV1::CrlCommitment
            } else {
                ZkX509ShaCallRoleV1::CrlIssuerSpki
            };
            if usize::from(manifest.call) != call
                || manifest.role != expected_role
                || manifest.activation != ZkX509ShaCallActivationV1::Required
            {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let segment = manifest.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1;
            let first = manifest.first_logical_row % ZK_X509_SHA_SEGMENT_ROWS_V1;
            let digest_row = manifest
                .maximum_local_rows
                .checked_sub(8)
                .and_then(|row| first.checked_add(row))
                .filter(|row| {
                    row.checked_add(8)
                        .is_some_and(|end| end <= ZK_X509_SHA_SEGMENT_ROWS_V1)
                })
                .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
            let registration = layout.registered_segment(
                SegmentAdapterIdV1::Sha256CallBus,
                u16::try_from(segment).map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
            )?;
            if registration.segment.trace_log2 != 19 || registration.segment.base_width == 0 {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let source = ColumnV1 {
                group: registration.trace_group,
                column: registration.base_start,
                log: 19,
            };
            blocks[index].source = source;
            // The target is public coefficients, so this coordinate is never opened.
            blocks[index].target = source;
            blocks[index].start = root.pow(digest_row as u128);
            let digest = if index == 0 {
                public.crl_der
            } else {
                public.issuer_spki
            };
            public_coefficients[index] = Self::interpolate_v1(blocks[index], digest)?;
        }
        let shared = layout.as_shared()?;
        let geometry =
            crate::privacy_engines::zk_x509::composition_masking::QuotientChunkGeometryV1::new_v1(
                &shared,
                AGGREGATE_PARAMETERS_V1,
            )
            .map_err(map_aggregate_error_v1)?;
        let degree = (1_usize << 19) + MASK_DEGREE - 8;
        if geometry
            .stride_v1()
            .checked_mul(AGGREGATE_PARAMETERS_V1.composition_degree_chunks)
            .is_none_or(|coverage| degree >= coverage)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(Self {
            blocks,
            public_coefficients,
        })
    }

    /// Public Lagrange interpolation uses only eight fixed stack coefficients.
    fn interpolate_v1(block: BlockV1, digest: [u8; 32]) -> Result<[F; 8], ZkX509StarkErrorV1> {
        let points =
            core::array::from_fn::<_, 8, _>(|i| block.start.mul(block.step.pow(i as u128)));
        let mut coefficients = [F::ZERO; 8];
        for (i, point) in points.iter().enumerate() {
            let mut basis = [F::ZERO; 8];
            basis[0] = F::ONE;
            let mut degree = 0;
            let mut denominator = F::ONE;
            for (j, other) in points.iter().enumerate() {
                if i == j {
                    continue;
                }
                denominator = denominator.mul(point.sub(*other));
                for k in (0..=degree).rev() {
                    basis[k + 1] = basis[k + 1].add(basis[k]);
                    basis[k] = basis[k].mul(F::ZERO.sub(*other));
                }
                degree += 1;
            }
            let word = u32::from_be_bytes(
                digest[4 * i..4 * i + 4]
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
            );
            let scale = F(u64::from(word)).mul(
                denominator
                    .inv()
                    .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?,
            );
            for (out, value) in coefficients.iter_mut().zip(basis) {
                *out = out.add(value.mul(scale));
            }
        }
        Ok(coefficients)
    }

    pub(super) fn source_v1(&self, index: usize) -> Result<ColumnV1, ZkX509StarkErrorV1> {
        self.blocks
            .get(index)
            .map(|b| b.source)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }

    pub(super) fn absorb_plan_v1(
        &self,
        transcript: &mut TransparentTranscriptV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        transcript
            .absorb(PUBLIC_DIGEST_DOMAIN_V1, &[PUBLIC_DIGEST_DESCRIPTOR_V1])
            .map_err(map_transparent_error_v1)?;
        for (index, block) in self.blocks.iter().enumerate() {
            // All integers are public, bounded layout coordinates; digest words
            // enter through their uniquely determined interpolation coefficients.
            let fields = [
                CALLS_V1[index] as u64,
                block.source.group as u64,
                block.source.column as u64,
                u64::from(block.source.log),
                block.start.0,
                block.step.0,
                block.count as u64,
            ];
            let mut bytes = [0_u8; 15 * 8];
            for (i, field) in fields
                .into_iter()
                .chain(self.public_coefficients[index].map(|v| v.0))
                .enumerate()
            {
                bytes[8 * i..8 * i + 8].copy_from_slice(&field.to_be_bytes());
            }
            transcript
                .absorb(PUBLIC_DIGEST_DOMAIN_V1, &[&bytes])
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
        if alphas.len() != PUBLIC_DIGEST_BLOCKS_V1
            || !z.is_canonical()
            || alphas.iter().any(|v| !v.is_canonical())
        {
            return Err(ZkX509StarkErrorV1::ConstraintOpening);
        }
        let mut result = E::ZERO;
        for (i, block) in self.blocks.iter().enumerate() {
            let actual = groups
                .get(block.source.group)
                .and_then(|g| g.base_current.get(block.source.column))
                .copied()
                .ok_or(ZkX509StarkErrorV1::ConstraintOpening)?;
            if !actual.is_canonical() {
                return Err(ZkX509StarkErrorV1::NonCanonicalField);
            }
            let expected =
                MainKeyJoinPlanV1::evaluate_coefficients_v1(&self.public_coefficients[i], z);
            let mut point = block.start;
            let mut divisor = E::ONE;
            for _ in 0..8 {
                divisor = divisor.mul(z.sub(E::from_base(point)));
                point = point.mul(block.step);
            }
            result = result.add(
                alphas[i]
                    .mul(actual.sub(expected))
                    .mul(divisor.inv().ok_or(ZkX509StarkErrorV1::ConstraintOpening)?),
            );
        }
        Ok(result)
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
        MainKeyJoinPlanV1::quotient_v1(block, source, &self.public_coefficients[index])
    }
}

#[cfg(test)]
#[path = "main_public_sha_digests_tests.rs"]
mod tests;
