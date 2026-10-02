//! Joint CA/MAIN ownership inside the unchanged MAIN arithmetic envelope.
//!
//! This module requires the retained-cut MAIN replay candidate. Both original
//! cut owners are already charged by its ordinary plan. Every registration's
//! coefficient cache loses the complete additional CA/metadata reservation.
//! The24 MAIN originals are constructed only after registration scratch drops.
//! These are allocation admissions, never measured process RSS/address-space
//! guarantees. The staged caller still owns the fixed source/runtime reserve.

use super::super::super::accumulator_stark::{self as ca, stages::CaAwaitingMainAuxiliaryV1};
use super::main_ca_links::{MainCaOriginalAuxiliaryV1, MainCaPrivatePlanV1};
use super::main_resources::MainProverBufferPlanV1;
use super::*;

/// Exact phase classes determine which original owners must coexist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MainCaBufferPhaseV1 {
    /// Sampling and original joined base/auxiliary commitments.
    OriginalCommitments,
    /// Each ordinary registration; MAIN24 has not yet been created.
    Registration,
    /// Late MAIN24 construction, private quotient pass and chunk contribution.
    PrivateLinks,
    /// Both compositions, original supplemental DEEP and separate local FRIs.
    Finalization,
}

/// Public forecasts are derived once from the same canonical layout. No field
/// is caller-selectable, and no phase is permitted to enlarge the MAIN ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MainCaJointBufferPlanV1 {
    common: MainProverBufferPlanV1,
    ca_originals: usize,
    main_originals: usize,
    metadata: usize,
    ca_work: usize,
}

fn sum_v1(values: &[usize]) -> Result<usize, ZkX509StarkErrorV1> {
    values.iter().try_fold(0usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)
    })
}

impl MainCaJointBufferPlanV1 {
    // Closed caps inside, not in addition to, the unchanged arithmetic ceiling.
    const METADATA_CAP_V1: usize = 1 << 20;
    const CA_WORK_CAP_V1: usize = ca::stages::CA_JOINT_WORKING_CAP_V1;
    const SELECTED_REPLAY_CAP_V1: usize = 64 << 20;

    /// Include full typed carriers for all31 existing MAIN extras as well as
    /// the24 new MAIN and108 CA extras. Arrays include values and mixes in both
    /// transcripts, and the simultaneous public source-plan clone is charged.
    fn metadata_required_v1() -> Result<usize, ZkX509StarkErrorV1> {
        const EXTRAS: usize = super::main_key_joins::OPENINGS_V1 + 24 + 108;
        sum_v1(&[
            2 * core::mem::size_of::<MainCaPrivatePlanV1>(),
            EXTRAS * core::mem::size_of::<aggregate::AggregateSupplementalDeepOpeningV1>(),
            4 * EXTRAS * core::mem::size_of::<E>(),
            4 * core::mem::size_of::<Vec<aggregate::AggregateSupplementalDeepOpeningV1>>(),
            core::mem::size_of::<Self>(),
            // Bounded public plan/transcript framing and exact-capacity lists.
            64 << 10,
        ])
    }

    pub(super) fn new_v1(layout: &AggregateProofLayoutV1) -> Result<Self, ZkX509StarkErrorV1> {
        let common = MainProverBufferPlanV1::new_v1(layout)?;
        let request = ca::ca_accumulator_resource_request_v1(
            ca::ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1,
            1,
            136,
        )
        .map_err(|_| ZkX509StarkErrorV1::ProofTooLarge)?;
        let ca_envelope = ca::checked_ca_accumulator_resource_envelope_v1(request)
            .map_err(|_| ZkX509StarkErrorV1::ProofTooLarge)?;
        // Conservative simultaneous CA native/LDE/chunk work plus all base,
        // auxiliary, four composition and independent FRI-mask Merkle trees,
        // four FRI layer allocations, DEEP coefficients and bounded metadata.
        let ca_rows = 1usize << super::super::super::profile::ZK_X509_CA_FRI_LDE_LOG2_V1;
        // The envelope records an inclusive polynomial degree. Allocation and
        // the closed FRI capacity count coefficients, including the last one.
        let ca_fri_coefficients = ca_envelope
            .fri_degree_cap
            .checked_add(1)
            .ok_or(ZkX509StarkErrorV1::ProofTooLarge)?;
        let ca_work_required = sum_v1(&[
            ca_envelope.adapter_resident_payload_bytes,
            7 * 2 * ca_rows * core::mem::size_of::<PrivacyOuterDigestV1>(),
            4 * ca_rows * core::mem::size_of::<E>(),
            4 * ca_fri_coefficients * core::mem::size_of::<E>(),
            Self::METADATA_CAP_V1,
            2 * ca::ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1,
        ])?;
        if Self::metadata_required_v1()? > Self::METADATA_CAP_V1
            || ca_work_required > Self::CA_WORK_CAP_V1
            || common.selected_replay > Self::SELECTED_REPLAY_CAP_V1
            || ca_envelope.mask_coefficients != 2100
            || ca_envelope.maximum_masked_trace_degree != 6195
            || ca_fri_coefficients != 9216
        {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        let plan = Self {
            common,
            ca_originals: CaAwaitingMainAuxiliaryV1::payload_bound_v1()
                .map_err(|_| ZkX509StarkErrorV1::ProofTooLarge)?,
            main_originals: MainCaOriginalAuxiliaryV1::payload_bound_v1(),
            metadata: Self::METADATA_CAP_V1,
            ca_work: Self::CA_WORK_CAP_V1,
        };
        for phase in [
            MainCaBufferPhaseV1::OriginalCommitments,
            MainCaBufferPhaseV1::Registration,
            MainCaBufferPhaseV1::PrivateLinks,
            MainCaBufferPhaseV1::Finalization,
        ] {
            plan.required_v1(phase)?;
        }
        Ok(plan)
    }

    /// The MAIN24 cache is absent during every ordinary registration. Charging
    /// it there would exceed the largest registration's remaining headroom.
    pub(super) fn required_v1(
        &self,
        phase: MainCaBufferPhaseV1,
    ) -> Result<usize, ZkX509StarkErrorV1> {
        let common = &self.common;
        let extra = sum_v1(&[self.ca_originals, self.metadata])?;
        let transient = match phase {
            MainCaBufferPhaseV1::OriginalCommitments => {
                sum_v1(&[common.joined_streams, common.replay_batch, self.ca_work])?
            }
            MainCaBufferPhaseV1::Registration => {
                sum_v1(&[common.quotient_stage, common.replay_batch])?
            }
            MainCaBufferPhaseV1::PrivateLinks => sum_v1(&[
                self.main_originals,
                common.replay_batch,
                common.composition,
                MainCaPrivatePlanV1::quotient_private_bytes_v1(),
            ])?,
            MainCaBufferPhaseV1::Finalization => sum_v1(&[
                self.main_originals,
                self.ca_work,
                common.selected_replay,
                common.replay_batch,
                common.composition,
                common.fri_stage,
                common.openings,
            ])?,
        };
        let total = sum_v1(&[common.masks, common.retained_cuts, extra, transient])?;
        if total > common.maximum_live_buffers {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        Ok(total)
    }

    /// Narrow the canonical registration cache instead of spending the same
    /// slack on CA originals and cached MAIN columns. Ordinary cut charges are
    /// retained by the underlying common plan without any ceiling adjustment.
    pub(super) fn quotient_cache_plan_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        registration: RegisteredSegmentLayoutV1,
    ) -> Result<main_quotient_cache::MainQuotientCachePlanV1, ZkX509StarkErrorV1> {
        if self.common != MainProverBufferPlanV1::new_v1(layout)? {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        self.common
            .quotient_cache_plan_v1(layout, registration)?
            .reserve_additional_v1(sum_v1(&[self.ca_originals, self.metadata])?)?
            .prioritize_registration_v1(registration)
    }

    /// Verify actual retained capacities against the forecasts used before
    /// allocation. The caller supplies MAIN24 only after ordinary quotient
    /// production returns and its registration-local owners have been dropped.
    pub(super) fn check_original_owners_v1(
        &self,
        phase: MainCaBufferPhaseV1,
        ca: &CaAwaitingMainAuxiliaryV1,
        main: Option<&MainCaOriginalAuxiliaryV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.check_original_payloads_v1(
            phase,
            ca.allocated_payload_bytes_v1()
                .map_err(|_| ZkX509StarkErrorV1::ProofTooLarge)?,
            main.map(MainCaOriginalAuxiliaryV1::allocated_payload_bytes_v1)
                .transpose()?,
        )
    }

    fn check_original_payloads_v1(
        &self,
        phase: MainCaBufferPhaseV1,
        ca_payload: usize,
        main_payload: Option<usize>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        self.required_v1(phase)?;
        if ca_payload != self.ca_originals {
            return Err(ZkX509StarkErrorV1::ProofTooLarge);
        }
        match (phase, main_payload) {
            (
                MainCaBufferPhaseV1::OriginalCommitments | MainCaBufferPhaseV1::Registration,
                None,
            ) => Ok(()),
            (MainCaBufferPhaseV1::PrivateLinks | MainCaBufferPhaseV1::Finalization, Some(main))
                if main == self.main_originals =>
            {
                Ok(())
            }
            _ => Err(ZkX509StarkErrorV1::ProofTooLarge),
        }
    }
}

// TODO: validate the wired phase forecasts and actual-capacity gates in the
// complete native proof; allocation arithmetic does not establish process limits.

#[cfg(test)]
#[path = "main_ca_resources_tests.rs"]
mod tests;
