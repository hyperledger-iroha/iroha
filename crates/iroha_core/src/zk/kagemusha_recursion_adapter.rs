//! Fail-closed boundary for Kagemusha Pasta-cycle recursion.
//!
//! The reviewed Axiom `PoseidonTranscript` hashes in `C::Scalar` and explicitly
//! assumes that field is native to the verifier circuit.  A generic
//! `Halo2Loader` adapter across the Pasta cycle therefore emulates every
//! transcript scalar.  The measured Ep-to-Fp prototype required 39,275,522
//! advice cells and 7,436,318 lookup cells (about 4.1 GiB live RSS); bounded
//! CRT batching and native curve coordinates still required 18,040,862 advice
//! cells, 2,669,809 lookup cells, 100.35 seconds to construct, and
//! 2,414,559,232 bytes peak RSS.  Proof parsing consumed 8,287,023 advice cells
//! and fold-transcript parsing another 5,835,004.  That construction is
//! structurally outside the wallet's 128 MiB preparation gate and is not kept
//! as a production fallback.
//! The supported same-scalar-field `Eq/Fp` tuple avoids that trait boundary but
//! not the resource bound: the fixed verifier still measured 4,659,490 advice
//! cells at degree 12, while a degree-18 outer proof measured 7,296 bytes
//! ordinary and 7,328 bytes with its folded generator (about 4 GiB live RSS).
//! Both exceed the fixed 1,600-byte step-proof contract by construction.
//!
//! The production wire carries the current Eq/Fp and Ep/Fq proofs together,
//! with one exact 890-`u32` predecessor state and one exact resulting state.
//! The fixed verifier derives every transcript challenge, residual coefficient,
//! and IPA accumulator from proof bytes; none is caller-selected wire data.
//! The production build retains the native terminal Eq/Vesta and Ep/Pallas
//! decisions over authenticated parameters and verifier keys. Tests retain the
//! fixed-key Poseidon proof wires, canonical BGH19 IPA folding, and exact
//! bounded proof bytes. Both recursive fixed-VK verifier halves constrain those
//! same operations. Production availability remains false pending the
//! authenticated complete archive, independent review, and physical-device gates.

#[cfg(test)]
use iroha_data_model::offline::KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4;
use iroha_data_model::offline::{
    KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
    KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4, KagemushaPastaCycleParityV1,
    KagemushaPastaCycleProvingKeyHeaderV4, KagemushaRecursiveSpendPublicStatementV4,
};
pub use iroha_data_model::offline::{KagemushaPastaPublicLayoutV4, KagemushaStepCircuitParamsV4};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

use ff::{Field as _, PrimeField};
use halo2_proofs::halo2curves::{
    CurveAffine,
    pasta::{Fp, Fq},
};
use snark_verifier::verifier::plonk::PlonkProtocol;

use super::kagemusha_accumulation::{
    KagemushaIpaAccumulationProofV4, KagemushaIpaAccumulatorWireV4,
};
use super::kagemusha_step_transition::{
    KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4, KAGEMUSHA_STEP_OPERATION_LIMBS_V4,
    KagemushaStepOperationVectorV4,
};

/// Maximum exact parent states consumed by one recursive transition.
pub const KAGEMUSHA_PASTA_PARENT_SLOTS_V1: usize = 2;

/// Exact public-column layout for the operation/protocol-identity Step wire.
pub const KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4: usize = 0;
/// First exact operation limb.
pub const KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4 + 8;
/// Parent-count cell.
pub const KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4 + KAGEMUSHA_STEP_OPERATION_LIMBS_V4;
/// First parent-state limb; each slot has the fixed state-vector stride.
pub const KAGEMUSHA_PASTA_PARENT_STATES_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4 + 1;
/// Exact stride of one parent/result state.
pub const KAGEMUSHA_PASTA_STATE_STRIDE_V4: usize =
    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2;
/// First result-state limb.
pub const KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4: usize = KAGEMUSHA_PASTA_PARENT_STATES_OFFSET_V4
    + KAGEMUSHA_PASTA_PARENT_SLOTS_V1 * KAGEMUSHA_PASTA_STATE_STRIDE_V4;
/// First manifest SHA-256 word.
pub const KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4 + KAGEMUSHA_PASTA_STATE_STRIDE_V4;
/// First Eq compiled-protocol identity word.
pub const KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + 8;
/// First Ep compiled-protocol identity word.
pub const KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4: usize =
    KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4 + 8;
fn validate_kagemusha_circuit_params_v4(
    params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaPastaPublicLayoutV4, String> {
    params
        .validate()
        .map_err(|error| format!("invalid authenticated Kagemusha V4 circuit parameters: {error}"))
}

/// Convert an authenticated data-model V4 configuration to Halo2's runtime
/// representation.  Callers must obtain `params` from a verified V4 profile;
/// bridge/local configuration inputs are never accepted here.
pub(crate) fn kagemusha_base_circuit_params_v4(
    params: &KagemushaStepCircuitParamsV4,
) -> Result<halo2_base::gates::circuit::BaseCircuitParams, String> {
    validate_kagemusha_circuit_params_v4(params)?;
    let convert = |values: &[u32], role: &str| {
        values
            .iter()
            .map(|value| {
                usize::try_from(*value)
                    .map_err(|_| format!("Kagemusha V4 {role} count does not fit usize"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(halo2_base::gates::circuit::BaseCircuitParams {
        k: usize::try_from(params.k)
            .map_err(|_| "Kagemusha V4 degree does not fit usize".to_owned())?,
        num_advice_per_phase: convert(&params.num_advice_per_phase, "advice")?,
        num_fixed: usize::try_from(params.num_fixed)
            .map_err(|_| "Kagemusha V4 fixed-column count does not fit usize".to_owned())?,
        num_lookup_advice_per_phase: convert(&params.num_lookup_advice_per_phase, "lookup advice")?,
        lookup_bits: Some(
            usize::try_from(params.lookup_bits)
                .map_err(|_| "Kagemusha V4 lookup width does not fit usize".to_owned())?,
        ),
        num_instance_columns: usize::try_from(params.num_instance_columns)
            .map_err(|_| "Kagemusha V4 instance-column count does not fit usize".to_owned())?,
    })
}

fn kagemusha_usable_rows_v4(params: &KagemushaStepCircuitParamsV4) -> Result<usize, String> {
    validate_kagemusha_circuit_params_v4(params)?;
    let domain_rows = 1_usize
        .checked_shl(params.k)
        .ok_or_else(|| "Kagemusha V4 domain row count does not fit usize".to_owned())?;
    let minimum_unusable_rows = usize::try_from(params.minimum_unusable_rows)
        .map_err(|_| "Kagemusha V4 unusable-row count does not fit usize".to_owned())?;
    domain_rows
        .checked_sub(minimum_unusable_rows)
        .ok_or_else(|| "Kagemusha V4 unusable-row count exceeds its domain".to_owned())
}

type KagemushaBreakPointsV4 = Vec<Vec<usize>>;

fn kagemusha_break_points_to_wire_v4(break_points: &[Vec<usize>]) -> Result<Vec<Vec<u32>>, String> {
    break_points
        .iter()
        .enumerate()
        .map(|(phase, phase_break_points)| {
            phase_break_points
                .iter()
                .map(|row| {
                    u32::try_from(*row).map_err(|_| {
                        format!("Kagemusha V4 phase {phase} breakpoint row does not fit u32")
                    })
                })
                .collect()
        })
        .collect()
}

fn kagemusha_break_points_from_wire_v4(
    break_points: &[Vec<u32>],
) -> Result<KagemushaBreakPointsV4, String> {
    break_points
        .iter()
        .enumerate()
        .map(|(phase, phase_break_points)| {
            phase_break_points
                .iter()
                .map(|row| {
                    usize::try_from(*row).map_err(|_| {
                        format!("Kagemusha V4 phase {phase} breakpoint row does not fit usize")
                    })
                })
                .collect()
        })
        .collect()
}

/// Reproduce halo2-base's witness-only advice placement without touching a
/// Halo2 region. This turns malformed or stale authenticated breakpoints into
/// ordinary loader errors instead of assignment-time indexing panics.
fn simulate_kagemusha_phase_assignment_v4(
    advice_lengths: &[usize],
    break_points: &[usize],
    advice_columns: usize,
    usable_rows: usize,
    label: &str,
    phase: usize,
) -> Result<(), String> {
    let total_advice = advice_lengths.iter().try_fold(0_usize, |total, len| {
        total
            .checked_add(*len)
            .ok_or_else(|| format!("Kagemusha V4 {label} phase {phase} advice length overflow"))
    })?;
    if advice_columns == 0 {
        if total_advice == 0 && break_points.is_empty() {
            return Ok(());
        }
        return Err(format!(
            "Kagemusha V4 {label} phase {phase} has advice or breakpoints but no advice columns"
        ));
    }
    if usable_rows == 0 {
        return Err(format!(
            "Kagemusha V4 {label} phase {phase} has no usable rows"
        ));
    }

    let mut next_breakpoint = 0_usize;
    let mut column = 0_usize;
    let mut row = 0_usize;
    for advice_length in advice_lengths {
        for _ in 0..*advice_length {
            if column >= advice_columns {
                return Err(format!(
                    "Kagemusha V4 {label} phase {phase} advice-column overflow"
                ));
            }
            if row >= usable_rows {
                return Err(format!(
                    "Kagemusha V4 {label} phase {phase} usable-row overflow"
                ));
            }
            if break_points
                .get(next_breakpoint)
                .is_some_and(|breakpoint| *breakpoint < row)
            {
                return Err(format!(
                    "Kagemusha V4 {label} phase {phase} missed breakpoint {} at row {row}",
                    break_points[next_breakpoint]
                ));
            }
            if break_points.get(next_breakpoint) == Some(&row) {
                next_breakpoint += 1;
                row = 0;
                column = column.checked_add(1).ok_or_else(|| {
                    format!("Kagemusha V4 {label} phase {phase} advice-column overflow")
                })?;
                if column >= advice_columns {
                    return Err(format!(
                        "Kagemusha V4 {label} phase {phase} advice-column overflow"
                    ));
                }
            }
            row = row
                .checked_add(1)
                .ok_or_else(|| format!("Kagemusha V4 {label} phase {phase} row-offset overflow"))?;
        }
    }
    if let Some(unconsumed) = break_points.get(next_breakpoint) {
        return Err(format!(
            "Kagemusha V4 {label} phase {phase} has unconsumed breakpoint {unconsumed}"
        ));
    }
    Ok(())
}

fn validate_kagemusha_builder_layout_v4<F>(
    builder: &halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    params: &KagemushaStepCircuitParamsV4,
    break_points: &[Vec<usize>],
    expected_witness_gen_only: bool,
    label: &str,
) -> Result<(), String>
where
    F: halo2_base::utils::ScalarField,
{
    let base_params = kagemusha_base_circuit_params_v4(params)?;
    if builder.witness_gen_only() != expected_witness_gen_only {
        return Err(format!(
            "Kagemusha V4 {label} builder is in the wrong synthesis stage"
        ));
    }
    let configured = &builder.config_params;
    if configured.k != base_params.k
        || configured.num_advice_per_phase != base_params.num_advice_per_phase
        || configured.num_fixed != base_params.num_fixed
        || configured.num_lookup_advice_per_phase != base_params.num_lookup_advice_per_phase
        || configured.lookup_bits != base_params.lookup_bits
        || configured.num_instance_columns != base_params.num_instance_columns
    {
        return Err(format!(
            "Kagemusha V4 {label} builder configuration does not match authenticated parameters"
        ));
    }
    if break_points.len() != base_params.num_advice_per_phase.len()
        || builder.core().phase_manager.len() != break_points.len()
    {
        return Err(format!(
            "Kagemusha V4 {label} breakpoint phase count does not match authenticated parameters"
        ));
    }
    let usable_rows = kagemusha_usable_rows_v4(params)?;
    for (phase, ((manager, phase_break_points), advice_columns)) in builder
        .core()
        .phase_manager
        .iter()
        .zip(break_points)
        .zip(&base_params.num_advice_per_phase)
        .enumerate()
    {
        let advice_lengths = manager
            .threads
            .iter()
            .map(|thread| thread.advice.len())
            .collect::<Vec<_>>();
        simulate_kagemusha_phase_assignment_v4(
            &advice_lengths,
            phase_break_points,
            *advice_columns,
            usable_rows,
            label,
            phase,
        )?;
    }

    let statistics = builder.statistics();
    if statistics.total_lookup_advice_per_phase.len()
        < base_params.num_lookup_advice_per_phase.len()
    {
        return Err(format!(
            "Kagemusha V4 {label} lookup phase count is incomplete"
        ));
    }
    for (phase, lookup_columns) in base_params
        .num_lookup_advice_per_phase
        .iter()
        .copied()
        .enumerate()
    {
        let lookup_cells = statistics.total_lookup_advice_per_phase[phase];
        if lookup_cells == 0 {
            continue;
        }
        if base_params.num_advice_per_phase[phase] == 1 {
            // RangeConfig's single-gate-column mode enables lookups on the
            // already simulated gate cells; it does not copy them into the
            // nominal lookup-advice columns, and repeated references consume
            // no additional physical rows.
            continue;
        }
        let lookup_capacity = lookup_columns.checked_mul(usable_rows).ok_or_else(|| {
            format!("Kagemusha V4 {label} phase {phase} lookup capacity overflow")
        })?;
        if lookup_columns == 0 || lookup_cells > lookup_capacity {
            return Err(format!(
                "Kagemusha V4 {label} phase {phase} lookup row/column overflow"
            ));
        }
    }
    Ok(())
}

fn capture_kagemusha_keygen_break_points_v4<F>(
    builder: &halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    params: &KagemushaStepCircuitParamsV4,
    label: &str,
) -> Result<KagemushaBreakPointsV4, String>
where
    F: halo2_base::utils::ScalarField,
{
    let break_points =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| builder.break_points())).map_err(
            |_| format!("Kagemusha V4 {label} keygen did not capture every breakpoint phase"),
        )?;
    validate_kagemusha_builder_layout_v4(builder, params, &break_points, false, label)?;
    // Exercise both portable conversions at the point of capture so an
    // architecture-width mismatch cannot survive into an artifact header.
    let wire = kagemusha_break_points_to_wire_v4(&break_points)?;
    if kagemusha_break_points_from_wire_v4(&wire)? != break_points {
        return Err(format!(
            "Kagemusha V4 {label} keygen breakpoint conversion is not lossless"
        ));
    }
    Ok(break_points)
}

/// Exact version of the canonical per-parity bootstrap payload.
pub const KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4: u16 = 4;
/// One fully parseable parent slot in a canonical V4 bootstrap artifact.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStepBootstrapParentSlotV4 {
    /// Exact one-column public instances, represented as unreduced `u32`
    /// values before conversion into either Pasta scalar field.
    pub instances: Vec<Vec<u32>>,
    /// Ordinary augmented Step proof transcript.
    pub ordinary_proof_bytes: Vec<u8>,
    /// Non-identity carried accumulator used by the always-executed fold.
    pub carried_lineage: KagemushaIpaAccumulatorWireV4,
    /// Complete post-proof fold transcript, present even though the bootstrap
    /// parent's public parent count is zero.
    pub post_proof_fold: KagemushaIpaAccumulationProofV4,
}

/// Canonical, independently authenticated bootstrap artifact for one parity.
///
/// It contains both fixed parent slots and the all-bootstrap branch fold needed
/// by genesis. Any synthesis with a real parent supplies a per-step mixed/real
/// branch fold, which is parsed and verified by the recursive circuit.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaStepBootstrapV4 {
    /// Exact bootstrap payload version.
    pub version: u16,
    /// Step parity for which the proof and curve encodings are valid.
    pub parity: KagemushaPastaCycleParityV1,
    /// Domain-separated SHA-256 of the exact CircuitParamsV4 payload.
    pub circuit_params_sha256: [u8; 32],
    /// Authenticated value-free compiled-protocol structure identity.
    pub compiled_protocol_structure_sha256: [u8; 32],
    /// Identity of the independently reproducible bootstrap protocol values.
    pub bootstrap_compiled_protocol_sha256: [u8; 32],
    /// One manifest-independent all-zero public slot. Both disabled circuit
    /// slots use this exact authenticated payload.
    pub parent_slot: KagemushaStepBootstrapParentSlotV4,
    /// Complete fold transcript for the two canonical bootstrap lineages.
    pub branch_merge_fold: KagemushaIpaAccumulationProofV4,
}

impl KagemushaStepBootstrapV4 {
    /// Validate every host-checkable bootstrap invariant against authenticated
    /// circuit parameters. Ordinary/fold equation validity is checked by the
    /// recursive circuit; shape validation never creates substitute bytes.
    pub fn validate(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let layout = validate_kagemusha_circuit_params_v4(params)?;
        let params_sha256 = params.sha256().map_err(|error| {
            format!("failed to identify authenticated Kagemusha V4 parameters: {error}")
        })?;
        if self.version != KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4
            || self.parity != expected_parity
            || self.circuit_params_sha256 != params_sha256
            || expected_structure_sha256 == [0; 32]
            || self.compiled_protocol_structure_sha256 != expected_structure_sha256
            || self.bootstrap_compiled_protocol_sha256 == [0; 32]
        {
            return Err("Kagemusha V4 bootstrap header mismatch".to_owned());
        }
        self.branch_merge_fold.validate_fixed_transcript(params.k)?;
        let instance_len = usize::try_from(layout.instance_column_limbs)
            .map_err(|_| "Kagemusha V4 bootstrap public length does not fit usize".to_owned())?;
        let accumulator_len = usize::try_from(layout.accumulator_limbs).map_err(|_| {
            "Kagemusha V4 bootstrap accumulator length does not fit usize".to_owned()
        })?;
        let maximum_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 parent-proof bound does not fit usize".to_owned())?;
        let eq_accumulator_offset = usize::try_from(layout.parent_eq_accumulator_offset)
            .map_err(|_| "Kagemusha V4 Eq accumulator offset does not fit usize".to_owned())?;
        let ep_accumulator_offset = usize::try_from(layout.parent_ep_accumulator_offset)
            .map_err(|_| "Kagemusha V4 Ep accumulator offset does not fit usize".to_owned())?;
        let slot = &self.parent_slot;
        if slot.instances.len() != 1
            || slot.instances[0].len() != instance_len
            || slot.instances[0].iter().any(|limb| *limb != 0)
            || slot.ordinary_proof_bytes.len() != maximum_proof_bytes
            || slot.instances[0][eq_accumulator_offset..eq_accumulator_offset + accumulator_len]
                .iter()
                .chain(
                    &slot.instances[0]
                        [ep_accumulator_offset..ep_accumulator_offset + accumulator_len],
                )
                .any(|limb| *limb != 0)
        {
            return Err("Kagemusha V4 bootstrap parent shape mismatch".to_owned());
        }
        slot.post_proof_fold.validate_fixed_transcript(params.k)?;
        match expected_parity {
            KagemushaPastaCycleParityV1::StepEq => {
                slot.carried_lineage.to_eq(params.k)?;
            }
            KagemushaPastaCycleParityV1::StepEp => {
                slot.carried_lineage.to_ep(params.k)?;
            }
        }
        Ok(layout)
    }

    /// Require this payload's bootstrap protocol identities to match a locally
    /// reconstructed protocol under the authenticated V4 profile.
    pub(crate) fn validate_bootstrap_protocol<C>(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
        bootstrap_protocol: &PlonkProtocol<C>,
    ) -> Result<KagemushaPastaPublicLayoutV4, String>
    where
        C: CurveAffine,
        C::ScalarExt: PrimeField,
    {
        let layout = self.validate(params, expected_parity, expected_structure_sha256)?;
        let actual_structure =
            kagemusha_compiled_protocol_structure_sha256(bootstrap_protocol, expected_parity)?;
        let actual_identity =
            kagemusha_compiled_protocol_identity_sha256(bootstrap_protocol, expected_parity)?;
        if actual_structure != expected_structure_sha256
            || actual_identity != self.bootstrap_compiled_protocol_sha256
        {
            return Err("Kagemusha V4 bootstrap protocol identity mismatch".to_owned());
        }
        Ok(layout)
    }

    /// Decode and canonically re-encode one bounded bootstrap payload before
    /// exposing any of its recursion witnesses.
    pub(crate) fn decode_authenticated(
        bytes: &[u8],
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
    ) -> Result<Self, String> {
        let maximum = usize::try_from(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4,
        )
        .map_err(|_| "Kagemusha V4 artifact bound does not fit usize".to_owned())?;
        if bytes.is_empty() || bytes.len() > maximum {
            return Err("Kagemusha V4 bootstrap payload length is invalid".to_owned());
        }
        let decoded: Self = norito::decode_from_bytes(bytes)
            .map_err(|error| format!("failed to decode Kagemusha V4 bootstrap: {error}"))?;
        decoded.validate(params, expected_parity, expected_structure_sha256)?;
        let canonical = norito::to_bytes(&decoded)
            .map_err(|error| format!("failed to re-encode Kagemusha V4 bootstrap: {error}"))?;
        if canonical != bytes {
            return Err("Kagemusha V4 bootstrap payload is not canonical Norito".to_owned());
        }
        Ok(decoded)
    }

    /// Encode one validated bootstrap payload for content-addressed framing.
    pub(crate) fn encode_authenticated(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_parity: KagemushaPastaCycleParityV1,
        expected_structure_sha256: [u8; 32],
    ) -> Result<Vec<u8>, String> {
        self.validate(params, expected_parity, expected_structure_sha256)?;
        norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha V4 bootstrap: {error}"))
    }

    /// Decode one authenticated Eq bootstrap parent for the runtime circuit.
    pub(crate) fn step_eq_parent(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_structure_sha256: [u8; 32],
        slot: usize,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String>
    {
        self.validate(
            params,
            KagemushaPastaCycleParityV1::StepEq,
            expected_structure_sha256,
        )?;
        if slot >= KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            return Err("Kagemusha V4 Eq bootstrap slot is out of range".to_owned());
        }
        let parent = &self.parent_slot;
        Ok(KagemushaStepParentProofV4 {
            instances: parent
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|limb| Fp::from(u64::from(*limb)))
                        .collect()
                })
                .collect(),
            proof_bytes: parent.ordinary_proof_bytes.clone(),
            carried_lineage: parent.carried_lineage.to_eq(params.k)?,
            external_accumulation_proof: parent.post_proof_fold.clone(),
        })
    }

    /// Decode one authenticated Ep bootstrap parent for the runtime circuit.
    pub(crate) fn step_ep_parent(
        &self,
        params: &KagemushaStepCircuitParamsV4,
        expected_structure_sha256: [u8; 32],
        slot: usize,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String>
    {
        self.validate(
            params,
            KagemushaPastaCycleParityV1::StepEp,
            expected_structure_sha256,
        )?;
        if slot >= KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            return Err("Kagemusha V4 Ep bootstrap slot is out of range".to_owned());
        }
        let parent = &self.parent_slot;
        Ok(KagemushaStepParentProofV4 {
            instances: parent
                .instances
                .iter()
                .map(|column| {
                    column
                        .iter()
                        .map(|limb| Fq::from(u64::from(*limb)))
                        .collect()
                })
                .collect(),
            proof_bytes: parent.ordinary_proof_bytes.clone(),
            carried_lineage: parent.carried_lineage.to_ep(params.k)?,
            external_accumulation_proof: parent.post_proof_fold.clone(),
        })
    }
}

/// Validate one canonical V4 bootstrap payload for release tooling.
///
/// This public helper exposes no recursion witness. It returns only the exact
/// authenticated ordinary-proof byte count so bundle generation can bind its
/// measured profile without duplicating the private bootstrap wire schema.
pub fn validate_kagemusha_step_bootstrap_payload_v4(
    bytes: &[u8],
    params: &KagemushaStepCircuitParamsV4,
    parity: KagemushaPastaCycleParityV1,
    expected_structure_sha256: [u8; 32],
) -> Result<usize, String> {
    let bootstrap = KagemushaStepBootstrapV4::decode_authenticated(
        bytes,
        params,
        parity,
        expected_structure_sha256,
    )?;
    Ok(bootstrap.parent_slot.ordinary_proof_bytes.len())
}

/// Return the first public limb of one fixed parent-state slot.
#[must_use]
pub const fn kagemusha_pasta_parent_state_offset_v4(parent_slot: usize) -> usize {
    assert!(parent_slot < KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
    KAGEMUSHA_PASTA_PARENT_STATES_OFFSET_V4 + parent_slot * KAGEMUSHA_PASTA_STATE_STRIDE_V4
}

/// Version of the compiled parent-protocol identity bound inside both halves.
pub const KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1: u32 = 1;
/// Pinned `snark-verifier` revision whose `PlonkProtocol` layout and private
/// enum encodings define the explicit V1 structural descriptor.
pub const KAGEMUSHA_SNARK_VERIFIER_PROTOCOL_REVISION_V1: &str =
    "bbfcc721d714bea0d44a27c8fc6c4736e73ca853";
/// Domain separator for the fixed, value-free compiled-protocol descriptor.
pub const KAGEMUSHA_COMPILED_PROTOCOL_STRUCTURE_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:compiled-protocol-structure:v1";
/// Domain separator for the authenticated compiled-protocol identity.
pub const KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:compiled-protocol-identity:v1";

fn protocol_parity_tag(parity: KagemushaPastaCycleParityV1) -> u32 {
    match parity {
        KagemushaPastaCycleParityV1::StepEq => 1,
        KagemushaPastaCycleParityV1::StepEp => 2,
    }
}

fn append_len(output: &mut Vec<u8>, len: usize, label: &str) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(len)
            .map_err(|_| format!("Kagemusha {label} length does not fit u32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_index(output: &mut Vec<u8>, value: usize, label: &str) -> Result<(), String> {
    output.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| format!("Kagemusha {label} does not fit u32"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn append_scalar_repr<F: PrimeField>(output: &mut Vec<u8>, scalar: F) -> Result<(), String> {
    let repr = scalar.to_repr();
    if repr.as_ref().len() != 32 {
        return Err("Kagemusha compiled protocol scalar is not 32 bytes".to_owned());
    }
    output.extend_from_slice(repr.as_ref());
    Ok(())
}

fn expression_unary_node(tag: u8, child: Result<Vec<u8>, String>) -> Result<Vec<u8>, String> {
    let child = child?;
    let mut encoded = vec![tag];
    append_len(&mut encoded, child.len(), "expression child")?;
    encoded.extend_from_slice(&child);
    Ok(encoded)
}

fn expression_binary_node(
    tag: u8,
    left: Result<Vec<u8>, String>,
    right: Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let left = left?;
    let right = right?;
    let mut encoded = vec![tag];
    append_len(&mut encoded, left.len(), "left expression child")?;
    encoded.extend_from_slice(&left);
    append_len(&mut encoded, right.len(), "right expression child")?;
    encoded.extend_from_slice(&right);
    Ok(encoded)
}

fn encode_common_polynomial_value(value: ciborium::value::Value) -> Result<Vec<u8>, String> {
    match value {
        ciborium::value::Value::Text(variant) if variant == "Identity" => Ok(vec![1, 0]),
        ciborium::value::Value::Map(mut fields) if fields.len() == 1 => {
            let (variant, rotation) = fields.pop().expect("one checked enum field");
            let ciborium::value::Value::Text(variant) = variant else {
                return Err("Kagemusha common-polynomial variant is not text".to_owned());
            };
            if variant != "Lagrange" {
                return Err(format!(
                    "unsupported Kagemusha common-polynomial variant `{variant}`"
                ));
            }
            let ciborium::value::Value::Integer(rotation) = rotation else {
                return Err("Kagemusha Lagrange rotation is not an integer".to_owned());
            };
            let rotation = i32::try_from(rotation)
                .map_err(|_| "Kagemusha Lagrange rotation does not fit i32".to_owned())?;
            let mut encoded = vec![1, 1];
            encoded.extend_from_slice(&rotation.to_le_bytes());
            Ok(encoded)
        }
        _ => Err("unsupported Kagemusha common-polynomial encoding".to_owned()),
    }
}

fn encode_linearization_value(value: ciborium::value::Value) -> Result<u8, String> {
    match value {
        ciborium::value::Value::Null => Ok(0),
        ciborium::value::Value::Text(variant) if variant == "WithoutConstant" => Ok(1),
        ciborium::value::Value::Text(variant) if variant == "MinusVanishingTimesQuotient" => Ok(2),
        _ => Err("unsupported Kagemusha linearization encoding".to_owned()),
    }
}

fn append_compressed_point<C: CurveAffine>(output: &mut Vec<u8>, point: C) -> Result<(), String> {
    let encoding = point.to_bytes();
    if encoding.as_ref().len() != 32 {
        return Err("Kagemusha compiled protocol point is not 32 bytes".to_owned());
    }
    output.extend_from_slice(encoding.as_ref());
    Ok(())
}

/// Return the exact fixed descriptor of a compiled parent protocol.
///
/// The descriptor deliberately excludes only the self-referential verifier-key
/// commitments and transcript initial state.  It includes every verifier
/// control-flow field, quotient expression, and instance-commitment key.  Its
/// digest can therefore be fixed before the final self key is known, while the
/// excluded values are witness-loaded and constrained by the identity below.
pub fn kagemusha_compiled_protocol_structure_sha256<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    if protocol.domain_as_witness.is_some() {
        return Err("native Kagemusha protocol unexpectedly has a witness domain".to_owned());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(KAGEMUSHA_COMPILED_PROTOCOL_STRUCTURE_DOMAIN_V1);
    bytes.push(0);
    bytes.extend_from_slice(&KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(KAGEMUSHA_SNARK_VERIFIER_PROTOCOL_REVISION_V1.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&protocol_parity_tag(parity).to_le_bytes());

    append_index(&mut bytes, protocol.domain.k, "domain k")?;
    append_index(&mut bytes, protocol.domain.n, "domain n")?;
    append_scalar_repr(&mut bytes, protocol.domain.n_inv)?;
    append_scalar_repr(&mut bytes, protocol.domain.r#gen)?;
    append_scalar_repr(&mut bytes, protocol.domain.gen_inv)?;

    // Only the count belongs to the fixed structure. The self-referential
    // preprocessed values are authenticated separately by the identity below.
    append_len(
        &mut bytes,
        protocol.preprocessed.len(),
        "preprocessed point count",
    )?;
    for (label, values) in [
        ("instance column count", &protocol.num_instance),
        ("witness phase count", &protocol.num_witness),
        ("challenge phase count", &protocol.num_challenge),
    ] {
        append_len(&mut bytes, values.len(), label)?;
        for value in values {
            append_index(&mut bytes, *value, label)?;
        }
    }

    for (label, queries) in [
        ("evaluation query count", &protocol.evaluations),
        ("PCS query count", &protocol.queries),
    ] {
        append_len(&mut bytes, queries.len(), label)?;
        for query in queries {
            append_index(&mut bytes, query.poly, "query polynomial index")?;
            bytes.extend_from_slice(&query.rotation.0.to_le_bytes());
        }
    }

    append_index(
        &mut bytes,
        protocol.quotient.chunk_degree,
        "quotient chunk degree",
    )?;
    // `Expression::evaluate` is the pinned verifier's own exhaustive recursive
    // visitor. It canonicalizes `DistributePowers` to the same sum/product
    // operations used during verification, while retaining every scalar,
    // polynomial, common-polynomial, challenge, unary, binary, and scale node.
    let numerator = protocol.quotient.numerator.evaluate(
        &|scalar| {
            let mut encoded = vec![0];
            append_scalar_repr(&mut encoded, scalar)?;
            Ok(encoded)
        },
        &|common_polynomial| {
            let value =
                ciborium::value::Value::serialized(&common_polynomial).map_err(|error| {
                    format!("failed to inspect Kagemusha common polynomial: {error}")
                })?;
            encode_common_polynomial_value(value)
        },
        &|query| {
            let mut encoded = vec![2];
            append_index(&mut encoded, query.poly, "expression polynomial index")?;
            encoded.extend_from_slice(&query.rotation.0.to_le_bytes());
            Ok(encoded)
        },
        &|challenge| {
            let mut encoded = vec![3];
            append_index(&mut encoded, challenge, "expression challenge index")?;
            Ok(encoded)
        },
        &|child| expression_unary_node(4, child),
        &|left, right| expression_binary_node(5, left, right),
        &|left, right| expression_binary_node(6, left, right),
        &|child, scalar| {
            let mut encoded = expression_unary_node(7, child)?;
            append_scalar_repr(&mut encoded, scalar)?;
            Ok(encoded)
        },
    )?;
    append_len(&mut bytes, numerator.len(), "quotient numerator")?;
    bytes.extend_from_slice(&numerator);

    // Presence, not the self-referential value, is part of the fixed shape.
    bytes.push(u8::from(protocol.transcript_initial_state.is_some()));
    match &protocol.instance_committing_key {
        Some(key) => {
            bytes.push(1);
            append_len(
                &mut bytes,
                key.bases.len(),
                "instance committing-key base count",
            )?;
            for base in &key.bases {
                append_compressed_point(&mut bytes, *base)?;
            }
            match key.constant {
                Some(constant) => {
                    bytes.push(1);
                    append_compressed_point(&mut bytes, constant)?;
                }
                None => bytes.push(0),
            }
        }
        None => bytes.push(0),
    }

    let linearization = ciborium::value::Value::serialized(&protocol.linearization)
        .map_err(|error| format!("failed to inspect Kagemusha linearization: {error}"))?;
    bytes.push(encode_linearization_value(linearization)?);

    append_len(
        &mut bytes,
        protocol.accumulator_indices.len(),
        "accumulator column count",
    )?;
    for column in &protocol.accumulator_indices {
        append_len(&mut bytes, column.len(), "accumulator index count")?;
        for (column, row) in column {
            append_index(&mut bytes, *column, "accumulator column index")?;
            append_index(&mut bytes, *row, "accumulator row index")?;
        }
    }
    Ok(Sha256::digest(bytes).into())
}

fn kagemusha_compiled_protocol_identity_preimage<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<Vec<u8>, String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    let structure = kagemusha_compiled_protocol_structure_sha256(protocol, parity)?;
    let transcript_initial_state = protocol
        .transcript_initial_state
        .ok_or_else(|| "Kagemusha compiled protocol has no transcript initial state".to_owned())?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1);
    bytes.push(0);
    bytes.extend_from_slice(&KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&protocol_parity_tag(parity).to_le_bytes());
    bytes.extend_from_slice(&structure);
    bytes.extend_from_slice(
        &u32::try_from(protocol.preprocessed.len())
            .map_err(|_| "Kagemusha preprocessed point count does not fit u32".to_owned())?
            .to_le_bytes(),
    );
    for point in &protocol.preprocessed {
        append_compressed_point(&mut bytes, *point)?;
    }
    bytes.extend_from_slice(transcript_initial_state.to_repr().as_ref());
    Ok(bytes)
}

/// Derive the release-authenticated identity of one exact compiled protocol.
///
/// Terminal verification computes this value from the authenticated Params/VK
/// artifacts.  Recursive circuits independently hash the same preimage from
/// witness-loaded preprocessed points and transcript state.
pub fn kagemusha_compiled_protocol_identity_sha256<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    Ok(
        Sha256::digest(kagemusha_compiled_protocol_identity_preimage(
            protocol, parity,
        )?)
        .into(),
    )
}

/// Convert a standard SHA-256 digest to the eight public big-endian words used
/// by the constrained SHA gadget.
#[must_use]
pub fn kagemusha_sha256_public_words(digest: [u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        u32::from_be_bytes(
            digest[index * 4..index * 4 + 4]
                .try_into()
                .expect("SHA-256 word has four bytes"),
        )
    })
}

/// Preserve one exact 32-byte wire value as eight little-endian `u32` limbs.
///
/// This is deliberately distinct from [`kagemusha_sha256_public_words`]: the
/// constrained SHA gadget exposes big-endian digest words, while manifest and
/// state-vector bindings carry their original bytes without reinterpreting the
/// wire encoding.
#[must_use]
fn kagemusha_exact_u32_public_limbs(bytes: [u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("exact 32-byte value has eight four-byte limbs"),
        )
    })
}

/// Canonical V4 recursive-verifier compilation profile.
///
/// Querying the public instance polynomial through an IPA commitment expands
/// every public limb into a fixed-base MSM inside the recursive verifier. The
/// V4 public column contains thousands of limbs, so the split scalar/point
/// audit would otherwise serialize thousands of fixed bases. The pinned
/// verifier supports the equivalent direct Lagrange-evaluation path when
/// queried instances are disabled.
fn kagemusha_ipa_compile_config_v4(public_len: usize) -> snark_verifier::system::halo2::Config {
    snark_verifier::system::halo2::Config::ipa()
        .set_query_instance(false)
        .with_num_instance(vec![public_len])
}

/// IPA multi-open prover matching [`kagemusha_ipa_compile_config_v4`].
///
/// The pinned Halo2 `ProverIPA` implementation hard-codes queried instances.
/// Delegating the opening proof while overriding this associated constant keeps
/// Halo2's proof transcript aligned with snark-verifier's direct-instance
/// protocol without forking the cryptographic implementation.
#[derive(Debug)]
struct KagemushaDirectInstanceProverIpa<'params, C: halo2_proofs::halo2curves::CurveAffine>(
    halo2_proofs::poly::ipa::multiopen::ProverIPA<'params, C>,
);

impl<'params, C>
    halo2_proofs::poly::commitment::Prover<
        'params,
        halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
    > for KagemushaDirectInstanceProverIpa<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    const QUERY_INSTANCE: bool = false;

    fn new(params: &'params halo2_proofs::poly::ipa::commitment::ParamsIPA<C>) -> Self {
        Self(<
            halo2_proofs::poly::ipa::multiopen::ProverIPA<'params, C>
            as halo2_proofs::poly::commitment::Prover<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::new(params))
    }

    fn create_proof<'com, E, T, R, I>(
        &self,
        rng: R,
        transcript: &mut T,
        queries: I,
    ) -> std::io::Result<()>
    where
        E: halo2_proofs::transcript::EncodedChallenge<C>,
        T: halo2_proofs::transcript::TranscriptWrite<C, E>,
        R: rand_core_06::RngCore,
        I: IntoIterator<Item = halo2_proofs::poly::ProverQuery<'com, C>> + Clone,
    {
        <
            halo2_proofs::poly::ipa::multiopen::ProverIPA<'params, C>
            as halo2_proofs::poly::commitment::Prover<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::create_proof(&self.0, rng, transcript, queries)
    }
}

/// IPA multi-open verifier matching [`KagemushaDirectInstanceProverIpa`].
#[derive(Debug)]
struct KagemushaDirectInstanceVerifierIpa<'params, C: halo2_proofs::halo2curves::CurveAffine>(
    halo2_proofs::poly::ipa::multiopen::VerifierIPA<'params, C>,
);

impl<'params, C>
    halo2_proofs::poly::commitment::Verifier<
        'params,
        halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
    > for KagemushaDirectInstanceVerifierIpa<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    type Guard = halo2_proofs::poly::ipa::strategy::GuardIPA<'params, C>;
    type MSMAccumulator = halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>;

    const QUERY_INSTANCE: bool = false;

    fn new(params: &'params halo2_proofs::poly::ipa::commitment::ParamsVerifierIPA<C>) -> Self {
        Self(<
            halo2_proofs::poly::ipa::multiopen::VerifierIPA<'params, C>
            as halo2_proofs::poly::commitment::Verifier<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::new(params))
    }

    fn verify_proof<'com, E, T, I>(
        &self,
        transcript: &mut T,
        queries: I,
        msm: Self::MSMAccumulator,
    ) -> Result<Self::Guard, halo2_proofs::poly::Error>
    where
        'params: 'com,
        E: halo2_proofs::transcript::EncodedChallenge<C>,
        T: halo2_proofs::transcript::TranscriptRead<C, E>,
        I: IntoIterator<
                Item = halo2_proofs::poly::VerifierQuery<
                    'com,
                    C,
                    halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>,
                >,
            > + Clone,
    {
        <
            halo2_proofs::poly::ipa::multiopen::VerifierIPA<'params, C>
            as halo2_proofs::poly::commitment::Verifier<
                'params,
                halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
            >
        >::verify_proof(&self.0, transcript, queries, msm)
    }
}

/// Single-proof strategy for the direct-instance IPA verifier.
///
/// snark-verifier's otherwise-equivalent strategy is implemented specifically
/// for Halo2's queried-instance `VerifierIPA`, so the local verifier needs the
/// same final MSM decision under its own type.
#[derive(Debug)]
struct KagemushaDirectInstanceSingleStrategy<'params, C: halo2_proofs::halo2curves::CurveAffine> {
    msm: halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>,
}

impl<'params, C> KagemushaDirectInstanceSingleStrategy<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    fn from_params(params: &'params halo2_proofs::poly::ipa::commitment::ParamsIPA<C>) -> Self {
        Self {
            msm: halo2_proofs::poly::ipa::msm::MSMIPA::new(params),
        }
    }
}

impl<'params, C>
    halo2_proofs::poly::VerificationStrategy<
        'params,
        halo2_proofs::poly::ipa::commitment::IPACommitmentScheme<C>,
        KagemushaDirectInstanceVerifierIpa<'params, C>,
    > for KagemushaDirectInstanceSingleStrategy<'params, C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    type Output = C;

    fn new(params: &'params halo2_proofs::poly::ipa::commitment::ParamsIPA<C>) -> Self {
        Self::from_params(params)
    }

    fn process(
        self,
        verify: impl FnOnce(
            halo2_proofs::poly::ipa::msm::MSMIPA<'params, C>,
        ) -> Result<
            halo2_proofs::poly::ipa::strategy::GuardIPA<'params, C>,
            halo2_proofs::plonk::Error,
        >,
    ) -> Result<Self::Output, halo2_proofs::plonk::Error> {
        use halo2_proofs::poly::commitment::MSM as _;

        let guard = verify(self.msm)?;
        let folded_generator = guard.compute_g();
        let (msm, _) = guard.use_g(folded_generator);
        if msm.check() {
            Ok(folded_generator)
        } else {
            Err(halo2_proofs::plonk::Error::ConstraintSystemFailure)
        }
    }

    fn finalize(self) -> bool {
        unreachable!("Kagemusha single-proof strategy decides in process")
    }
}

/// Deterministic universal target used to break the remaining self-protocol
/// shape cycle during artifact generation.
///
/// `BaseConfig` fixes the complete Halo2 constraint-system structure from
/// `BaseCircuitParams`; virtual arithmetic performed during synthesis changes
/// fixed/preprocessed *values* but not the PLONK query graph.  Artifact
/// generation therefore compiles this empty bootstrap circuit first, preserves
/// that protocol structure in `without_witnesses`, generates the real Step key,
/// recompiles it, and requires the two structure digests to match exactly.
#[derive(Clone, Debug)]
pub struct KagemushaUniversalProtocolTargetV1 {
    /// Exact release `BaseConfig`, shared by bootstrap and final Step circuit.
    pub base_circuit_params: halo2_base::gates::circuit::BaseCircuitParams,
    /// Exact instance-column lengths supplied to `snark-verifier::compile`.
    pub instance_column_lengths: Vec<usize>,
}

impl KagemushaUniversalProtocolTargetV1 {
    /// Reject a target that cannot describe the one-column Kagemusha Step ABI.
    pub fn validate(&self) -> Result<(), String> {
        if self.base_circuit_params.k == 0
            || self.base_circuit_params.num_instance_columns != 1
            || self.instance_column_lengths.len() != 1
            || self.instance_column_lengths[0] == 0
        {
            return Err("Kagemusha universal protocol target shape mismatch".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct KagemushaProtocolBootstrapCircuit<F>
where
    F: halo2_base::utils::ScalarField,
{
    params: halo2_base::gates::circuit::BaseCircuitParams,
    marker: std::marker::PhantomData<F>,
}

impl<F> halo2_proofs::plonk::Circuit<F> for KagemushaProtocolBootstrapCircuit<F>
where
    F: halo2_base::utils::ScalarField,
{
    type Config = halo2_base::gates::circuit::BaseConfig<F>;
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
    type Params = halo2_base::gates::circuit::BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        self.clone()
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<F>,
        params: Self::Params,
    ) -> Self::Config {
        halo2_base::gates::circuit::BaseConfig::configure(meta, params)
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<F>) -> Self::Config {
        unreachable!("Kagemusha bootstrap requires circuit params")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl halo2_proofs::circuit::Layouter<F>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::<F>::new(false)
            .use_params(self.params.clone());
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<F> as halo2_proofs::plonk::Circuit<
            F,
        >>::synthesize(&builder, config, layouter)
    }
}

fn kagemusha_bootstrap_verifying_key_v1<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    target: &KagemushaUniversalProtocolTargetV1,
) -> Result<halo2_proofs::plonk::VerifyingKey<C>, String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField,
{
    use halo2_proofs::poly::commitment::Params as _;

    target.validate()?;
    if usize::try_from(params.k()).ok() != Some(target.base_circuit_params.k) {
        return Err("Kagemusha bootstrap Params degree does not match BaseConfig".to_owned());
    }
    let circuit = KagemushaProtocolBootstrapCircuit::<C::ScalarExt> {
        params: target.base_circuit_params.clone(),
        marker: std::marker::PhantomData,
    };
    halo2_proofs::plonk::keygen_vk(params, &circuit)
        .map_err(|error| format!("failed to generate Kagemusha bootstrap VK: {error}"))
}

/// Compile the deterministic bootstrap protocol whose structure is retained
/// by a self-recursive Step circuit during key generation.
///
/// The protocol values belong only to the canonical all-zero bootstrap proof;
/// they are never substituted for the final Step protocol. After the final
/// Step VK exists, callers compare structure hashes with
/// [`kagemusha_require_protocol_structure_v1`] and authenticate both protocol
/// identities independently.
#[cfg(test)]
pub fn kagemusha_bootstrap_compiled_protocol_v1<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    target: &KagemushaUniversalProtocolTargetV1,
) -> Result<PlonkProtocol<C>, String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField,
{
    let verifying_key = kagemusha_bootstrap_verifying_key_v1(params, target)?;
    Ok(snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(target.instance_column_lengths[0]),
    ))
}

/// Require a final self protocol to converge to the deterministic bootstrap
/// structure.  A mismatch is an artifact-generation failure, never a reason to
/// alter the target at runtime.
pub fn kagemusha_require_protocol_structure_v1<C>(
    bootstrap: &PlonkProtocol<C>,
    final_protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<[u8; 32], String>
where
    C: CurveAffine,
    C::ScalarExt: PrimeField,
{
    let expected = kagemusha_compiled_protocol_structure_sha256(bootstrap, parity)?;
    let actual = kagemusha_compiled_protocol_structure_sha256(final_protocol, parity)?;
    if actual != expected {
        return Err("Kagemusha final protocol did not converge to bootstrap structure".to_owned());
    }
    Ok(actual)
}

/// Internal semantic boundary shared by both authoritative V4 parities.
struct KagemushaSemanticBoundaryV4 {
    /// Canonical public-statement digest as eight unreduced little-endian limbs.
    pub public_statement_digest: [u32; 8],
    /// Number of consumed parent proof pairs.
    pub parent_count: u32,
    /// Complete ordered parent result states with exact zero padding.
    pub parent_states: [Vec<u32>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Complete state resulting from the current transition.
    pub result_state: Vec<u32>,
    /// Authenticated artifact-manifest SHA-256 as eight unreduced limbs.
    pub manifest_sha256: [u32; 8],
    /// SHA-256 joins for the Eq parent's scalar and point verifier halves.
    pub parent_eq_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// SHA-256 joins for the Ep parent's scalar and point verifier halves.
    pub parent_ep_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
}

impl KagemushaSemanticBoundaryV4 {
    fn validate_with_parent_state_order(
        &self,
        proof_step_count: u32,
        require_lexicographic_parent_state_order: bool,
        require_deferred_audit_joins: bool,
    ) -> Result<(), String> {
        use super::kagemusha_v2::KagemushaRecursiveSpendStateVectorV2;
        use iroha_data_model::offline::{
            KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2,
            KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2,
        };

        let parent_count = usize::try_from(self.parent_count)
            .map_err(|_| "Kagemusha parent count does not fit usize".to_owned())?;
        let initializing = proof_step_count == 1;
        if proof_step_count == 0
            || self.public_statement_digest == [0; 8]
            || self.manifest_sha256 == [0; 8]
            || parent_count > KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            || initializing != (parent_count == 0)
            || (!initializing && parent_count == 0)
            || self
                .parent_states
                .iter()
                .any(|state| state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2)
            || self.result_state.len() != KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2
            || self.result_state.first().copied()
                != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2)
        {
            return Err("Kagemusha exact-state public-instance shape mismatch".to_owned());
        }
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            let present = slot < parent_count;
            let state = &self.parent_states[slot];
            let eq_digest = self.parent_eq_deferred_sha256[slot];
            let ep_digest = self.parent_ep_deferred_sha256[slot];
            if present {
                if state.first().copied()
                    != Some(KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2)
                    || state == &self.result_state
                    || if require_deferred_audit_joins {
                        eq_digest == [0; 8] || ep_digest == [0; 8] || eq_digest == ep_digest
                    } else {
                        eq_digest != [0; 8] || ep_digest != [0; 8]
                    }
                {
                    return Err("Kagemusha present parent slot is invalid".to_owned());
                }
            } else if state.iter().any(|limb| *limb != 0)
                || eq_digest != [0; 8]
                || ep_digest != [0; 8]
            {
                return Err("Kagemusha absent parent slot has non-zero padding".to_owned());
            }
        }
        if require_lexicographic_parent_state_order
            && parent_count == KAGEMUSHA_PASTA_PARENT_SLOTS_V1
            && self.parent_states[0] >= self.parent_states[1]
        {
            return Err("Kagemusha parent states are not in canonical order".to_owned());
        }
        let result_vector = KagemushaRecursiveSpendStateVectorV2 {
            limbs: self
                .result_state
                .clone()
                .try_into()
                .map_err(|_| "Kagemusha result state has the wrong length".to_owned())?,
        };
        if result_vector.proof_step_count() != proof_step_count
            || result_vector.peer_hop_count() > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
            || result_vector.manifest_sha256_limbs() != self.manifest_sha256
        {
            return Err("Kagemusha result-state counters or manifest mismatch".to_owned());
        }
        let mut maximum_parent_step = 0_u32;
        let mut maximum_parent_hop = 0_u32;
        for state in self.parent_states.iter().take(parent_count) {
            let vector = KagemushaRecursiveSpendStateVectorV2 {
                limbs: state
                    .clone()
                    .try_into()
                    .map_err(|_| "Kagemusha parent state has the wrong length".to_owned())?,
            };
            let parent_step = vector.proof_step_count();
            let parent_hop = vector.peer_hop_count();
            if parent_step == 0
                || parent_step >= proof_step_count
                || parent_hop > KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2
                || vector.manifest_sha256_limbs() != self.manifest_sha256
            {
                return Err("Kagemusha parent-state counters or manifest mismatch".to_owned());
            }
            maximum_parent_step = maximum_parent_step.max(parent_step);
            maximum_parent_hop = maximum_parent_hop.max(parent_hop);
        }
        if initializing {
            if result_vector.peer_hop_count() != 0 {
                return Err("Kagemusha initialization state has a peer hop".to_owned());
            }
        } else if maximum_parent_step.checked_add(1) != Some(proof_step_count)
            || !matches!(
                result_vector
                    .peer_hop_count()
                    .checked_sub(maximum_parent_hop),
                Some(0 | 1)
            )
        {
            return Err("Kagemusha parent/result step or hop relation mismatch".to_owned());
        }
        Ok(())
    }
}

/// Degree-parameterized V4 public inputs used by both concrete Step circuits.
///
/// The semantic prefix is fixed for ABI-20. Only the two IPA accumulator
/// slices are dynamic, and their exact offsets are derived from the separately
/// authenticated [`KagemushaStepCircuitParamsV4`].
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct KagemushaPastaCyclePublicInputsV4 {
    /// Canonical public-statement digest as eight unreduced limbs.
    pub public_statement_digest: [u32; 8],
    /// Exact canonical operation row shared by both Step parities.
    pub operation: KagemushaStepOperationVectorV4,
    /// Number of consumed parent proof pairs.
    pub parent_count: u32,
    /// Complete ordered parent result states with exact zero padding.
    pub parent_states: [Vec<u32>; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Complete state resulting from this transition.
    pub result_state: Vec<u32>,
    /// Authenticated artifact-manifest SHA-256.
    pub manifest_sha256: [u32; 8],
    /// SHA-256 identity of the exact compiled Eq parent protocol.
    pub step_eq_compiled_protocol_sha256: [u32; 8],
    /// SHA-256 identity of the exact compiled Ep parent protocol.
    pub step_ep_compiled_protocol_sha256: [u32; 8],
    /// Complete Eq parent lineage, absent only at initialization.
    pub parent_eq_lineage_accumulator: Option<KagemushaIpaAccumulatorWireV4>,
    /// Complete Ep parent lineage, absent only at initialization.
    pub parent_ep_lineage_accumulator: Option<KagemushaIpaAccumulatorWireV4>,
    /// Eq scalar/point audit joins for the two fixed parent slots.
    pub parent_eq_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Ep scalar/point audit joins for the two fixed parent slots.
    pub parent_ep_deferred_sha256: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    /// Explicit circuit mode. Public proof pairs accept only `1` (live); the
    /// adapter alone uses `0` for its authenticated all-zero bootstrap proof.
    pub live_selector: u32,
}

impl KagemushaPastaCyclePublicInputsV4 {
    /// Validate the semantic state boundary and its degree-specific lineage.
    pub fn validate(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        self.validate_with_deferred_audit_joins(proof_step_count, params, true)
    }

    fn validate_for_audit_derivation_prepass(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        self.validate_with_deferred_audit_joins(proof_step_count, params, false)
    }

    fn validate_with_deferred_audit_joins(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        require_deferred_audit_joins: bool,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let layout = validate_kagemusha_circuit_params_v4(params)?;
        KagemushaSemanticBoundaryV4 {
            public_statement_digest: self.public_statement_digest,
            parent_count: self.parent_count,
            parent_states: self.parent_states.clone(),
            result_state: self.result_state.clone(),
            manifest_sha256: self.manifest_sha256,
            parent_eq_deferred_sha256: self.parent_eq_deferred_sha256,
            parent_ep_deferred_sha256: self.parent_ep_deferred_sha256,
        }
        // ABI-20 parent slots preserve split.inputs order, which is already
        // canonical by bundle digest. State-vector lexicographic order is an
        // unrelated historical V1 wire rule and cannot be imposed here.
        .validate_with_parent_state_order(
            proof_step_count,
            false,
            require_deferred_audit_joins,
        )?;
        if self.live_selector != KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4
            || self.operation.to_fields().is_err()
            || self.step_eq_compiled_protocol_sha256 == [0; 8]
            || self.step_ep_compiled_protocol_sha256 == [0; 8]
            || self.step_eq_compiled_protocol_sha256 == self.step_ep_compiled_protocol_sha256
        {
            return Err("Kagemusha V4 operation/protocol public shape mismatch".to_owned());
        }
        let initializing = proof_step_count == 1;
        match (
            initializing,
            &self.parent_eq_lineage_accumulator,
            &self.parent_ep_lineage_accumulator,
        ) {
            (true, None, None) => {}
            (false, Some(eq), Some(ep)) => {
                eq.to_eq(params.k)?;
                ep.to_ep(params.k)?;
            }
            _ => {
                return Err("Kagemusha V4 parent-lineage accumulator presence mismatch".to_owned());
            }
        }
        Ok(layout)
    }

    /// Convert the complete field-neutral V4 vector to one Halo2 column.
    pub fn instance_column<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        self.instance_column_with_deferred_audit_joins(proof_step_count, params, true)
    }

    fn instance_column_for_audit_derivation_prepass<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        self.instance_column_with_deferred_audit_joins(proof_step_count, params, false)
    }

    fn instance_column_with_deferred_audit_joins<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        require_deferred_audit_joins: bool,
    ) -> Result<Vec<F>, String>
    where
        F: PrimeField + From<u64>,
    {
        let layout = self.validate_with_deferred_audit_joins(
            proof_step_count,
            params,
            require_deferred_audit_joins,
        )?;
        let mut limbs = self
            .public_statement_digest
            .iter()
            .chain(&self.operation.limbs)
            .chain(std::iter::once(&self.parent_count))
            .chain(self.parent_states.iter().flatten())
            .chain(&self.result_state)
            .chain(&self.manifest_sha256)
            .chain(&self.step_eq_compiled_protocol_sha256)
            .chain(&self.step_ep_compiled_protocol_sha256)
            .copied()
            .collect::<Vec<_>>();
        let accumulator_limbs = usize::try_from(layout.accumulator_limbs)
            .map_err(|_| "Kagemusha V4 accumulator length does not fit usize".to_owned())?;
        for accumulator in [
            &self.parent_eq_lineage_accumulator,
            &self.parent_ep_lineage_accumulator,
        ] {
            match accumulator {
                Some(accumulator) => limbs.extend(accumulator.instance_limbs(params.k)?),
                None => limbs.resize(limbs.len() + accumulator_limbs, 0),
            }
        }
        limbs.extend(self.parent_eq_deferred_sha256.iter().flatten().copied());
        limbs.extend(self.parent_ep_deferred_sha256.iter().flatten().copied());
        limbs.push(self.live_selector);
        let expected = usize::try_from(layout.instance_column_limbs)
            .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
        if limbs.len() != expected {
            return Err("Kagemusha V4 instance-column length mismatch".to_owned());
        }
        Ok(limbs
            .into_iter()
            .map(|limb| F::from(u64::from(limb)))
            .collect())
    }
}

/// Backend-native V4 Eq/Ep pair encoded inside the public opaque proof box.
///
/// This is deliberately not a data-model envelope.  ABI 20 carries the
/// canonical Norito bytes of this value as an opaque proof payload, while the
/// core alone constructs, decodes, and verifies its recursion-specific fields.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct KagemushaPastaCycleProofPairV4 {
    /// Exact backend-native pair layout version.
    pub(crate) version: u16,
    /// Logical recursive transition count proved by both halves.
    pub(crate) proof_step_count: u32,
    /// Exact common public instances used by both proofs.
    pub(crate) public_inputs: KagemushaPastaCyclePublicInputsV4,
    /// Current Eq/Fp augmented proof bytes.
    pub(crate) step_eq_proof_bytes: Vec<u8>,
    /// Current Ep/Fq augmented proof bytes.
    pub(crate) step_ep_proof_bytes: Vec<u8>,
    /// BGH19 proof folding the current Eq opening with its parent lineage.
    pub(crate) step_eq_accumulation_proof: KagemushaIpaAccumulationProofV4,
    /// BGH19 proof folding the current Ep opening with its parent lineage.
    pub(crate) step_ep_accumulation_proof: KagemushaIpaAccumulationProofV4,
}

/// Exact backend-native layout version of [`KagemushaPastaCycleProofPairV4`].
pub(crate) const KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4: u16 = 4;

impl KagemushaPastaCycleProofPairV4 {
    /// Validate the complete pair against authenticated release parameters and
    /// the release's measured opaque-proof payload cap.
    pub(crate) fn validate(
        &self,
        step_eq_params: &KagemushaStepCircuitParamsV4,
        step_ep_params: &KagemushaStepCircuitParamsV4,
        max_pair_bytes: u32,
    ) -> Result<KagemushaPastaPublicLayoutV4, String> {
        let eq_layout = self
            .public_inputs
            .validate(self.proof_step_count, step_eq_params)?;
        let ep_layout = self
            .public_inputs
            .validate(self.proof_step_count, step_ep_params)?;
        let eq_proof_bytes = usize::try_from(step_eq_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof size does not fit usize".to_owned())?;
        let ep_proof_bytes = usize::try_from(step_ep_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof size does not fit usize".to_owned())?;
        if self.version != KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4
            || eq_layout != ep_layout
            || step_eq_params.k != step_ep_params.k
            || self.step_eq_proof_bytes.len() != eq_proof_bytes
            || self.step_ep_proof_bytes.len() != ep_proof_bytes
            || self.step_eq_proof_bytes == self.step_ep_proof_bytes
        {
            return Err("Kagemusha V4 Eq/Ep proof-pair shape mismatch".to_owned());
        }
        let has_parent = self.public_inputs.parent_count != 0;
        self.step_eq_accumulation_proof
            .validate(step_eq_params.k, has_parent)?;
        self.step_ep_accumulation_proof
            .validate(step_ep_params.k, has_parent)?;

        let maximum = usize::try_from(max_pair_bytes)
            .map_err(|_| "Kagemusha V4 pair bound does not fit usize".to_owned())?;
        let absolute_maximum = usize::try_from(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4,
        )
        .map_err(|_| "Kagemusha V4 absolute pair bound does not fit usize".to_owned())?;
        if maximum == 0 || maximum > absolute_maximum {
            return Err("Kagemusha V4 authenticated pair bound is invalid".to_owned());
        }
        let encoded = norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha V4 proof pair: {error}"))?;
        if encoded.len() > maximum {
            return Err(format!(
                "Kagemusha V4 proof pair is {} bytes; authenticated maximum is {maximum}",
                encoded.len()
            ));
        }
        Ok(eq_layout)
    }

    /// Decode one opaque ABI-20 proof payload, reject non-canonical bytes, and
    /// validate it against the pinned authenticated release profile.
    pub(crate) fn decode_authenticated(
        bytes: &[u8],
        step_eq_params: &KagemushaStepCircuitParamsV4,
        step_ep_params: &KagemushaStepCircuitParamsV4,
        max_pair_bytes: u32,
    ) -> Result<Self, String> {
        let maximum = usize::try_from(max_pair_bytes)
            .map_err(|_| "Kagemusha V4 pair bound does not fit usize".to_owned())?;
        if bytes.is_empty() || bytes.len() > maximum {
            return Err("Kagemusha V4 opaque proof payload length is invalid".to_owned());
        }
        let pair: Self = norito::decode_from_bytes(bytes)
            .map_err(|error| format!("failed to decode Kagemusha V4 proof pair: {error}"))?;
        pair.validate(step_eq_params, step_ep_params, max_pair_bytes)?;
        let canonical = norito::to_bytes(&pair)
            .map_err(|error| format!("failed to re-encode Kagemusha V4 proof pair: {error}"))?;
        if canonical != bytes {
            return Err("Kagemusha V4 proof pair is not canonical Norito".to_owned());
        }
        Ok(pair)
    }

    /// Encode one fully validated native pair for the public opaque proof box.
    pub(crate) fn encode_authenticated(
        &self,
        step_eq_params: &KagemushaStepCircuitParamsV4,
        step_ep_params: &KagemushaStepCircuitParamsV4,
        max_pair_bytes: u32,
    ) -> Result<Vec<u8>, String> {
        self.validate(step_eq_params, step_ep_params, max_pair_bytes)?;
        norito::to_bytes(self)
            .map_err(|error| format!("failed to encode Kagemusha V4 proof pair: {error}"))
    }
}

/// Validate a canonical V4 proof-pair measurement without exposing its wire.
///
/// Artifact tooling uses this only after producing a real pair with the
/// authenticated keys. Runtime verification additionally performs both
/// terminal cryptographic decisions through the installed verifier.
pub fn validate_kagemusha_proof_pair_measurement_v4(
    bytes: &[u8],
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
) -> Result<usize, String> {
    KagemushaPastaCycleProofPairV4::decode_authenticated(
        bytes,
        step_eq_params,
        step_ep_params,
        max_pair_bytes,
    )?;
    Ok(bytes.len())
}

const KAGEMUSHA_POSEIDON_WIDTH: usize = 3;
const KAGEMUSHA_POSEIDON_RATE: usize = 2;
const KAGEMUSHA_POSEIDON_FULL_ROUNDS: usize = 8;
const KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS: usize = 57;
const KAGEMUSHA_POSEIDON_SECURE_MDS: usize = 0;

fn catch_kagemusha_native_verifier_panic<T>(
    label: &str,
    verify: impl FnOnce() -> T,
) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(verify))
        .map_err(|_| format!("Kagemusha V4 {label} rejected an invalid native verifier relation"))
}

/// Fully verify and terminally decide a degree-parameterized V4 Eq proof.
pub(crate) fn terminal_verify_step_eq_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<(), String> {
    use halo2_proofs::poly::commitment::Params as _;

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k {
        return Err("Kagemusha V4 Eq ParamsIPA/circuit degree mismatch".to_owned());
    }
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?;
    let instances = vec![public_inputs.instance_column::<Fp>(proof_step_count, circuit_params)?];
    let current = succinct_verify_step_eq_instances(
        params,
        verifying_key,
        proof,
        &instances,
        max_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current,
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    Ok(())
}

/// Fully verify and terminally decide a degree-parameterized V4 Ep proof.
pub(crate) fn terminal_verify_step_ep_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<(), String> {
    use halo2_proofs::poly::commitment::Params as _;

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k {
        return Err("Kagemusha V4 Ep ParamsIPA/circuit degree mismatch".to_owned());
    }
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?;
    let instances = vec![public_inputs.instance_column::<Fq>(proof_step_count, circuit_params)?];
    let current = succinct_verify_step_ep_instances(
        params,
        verifying_key,
        proof,
        &instances,
        max_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current,
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    Ok(())
}

fn succinct_verify_step_eq_instances(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
    max_proof_bytes: usize,
) -> Result<
    snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EqAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
    String,
> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Eq, EqAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
        system::halo2::{compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
    };

    if max_proof_bytes == 0 || proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha Eq proof length is invalid".to_owned());
    }
    type Scheme = IpaAs<EqAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let w = hash_to_curve(&[1]).to_affine();
    let u = hash_to_curve(&[2]).to_affine();
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "Eq parameter degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k())
                    .map_err(|_| "Eq parameter degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        u,
        Some(w),
    );
    let protocol = compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(instances[0].len()),
    );
    let mut cursor = std::io::Cursor::new(proof);
    {
        let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
        let parsed = catch_kagemusha_native_verifier_panic("Eq proof parse", || {
            PlonkSuccinctVerifier::<Scheme>::read_proof(&svk, &protocol, instances, &mut transcript)
        })?
        .map_err(|error| format!("failed to parse Kagemusha Eq proof: {error:?}"))?;
        let accumulators = catch_kagemusha_native_verifier_panic("Eq proof verification", || {
            PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, instances, &parsed)
        })?
        .map_err(|error| format!("Kagemusha Eq succinct verification failed: {error:?}"))?;
        let [accumulator]: [_; 1] = accumulators.try_into().map_err(|accumulators: Vec<_>| {
            format!(
                "Kagemusha Eq proof emitted {} opening accumulators instead of one",
                accumulators.len()
            )
        })?;
        if cursor.position()
            != u64::try_from(proof.len()).map_err(|_| "Eq proof length does not fit u64")?
        {
            return Err("Kagemusha Eq proof has trailing bytes".to_owned());
        }
        return Ok(accumulator);
    }
}

fn succinct_verify_step_ep_instances(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
    max_proof_bytes: usize,
) -> Result<
    snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EpAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
    String,
> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Ep, EpAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
        system::halo2::{compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
    };

    if max_proof_bytes == 0 || proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha Ep proof length is invalid".to_owned());
    }
    type Scheme = IpaAs<EpAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let w = hash_to_curve(&[1]).to_affine();
    let u = hash_to_curve(&[2]).to_affine();
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "Ep parameter degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k())
                    .map_err(|_| "Ep parameter degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        u,
        Some(w),
    );
    let protocol = compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(instances[0].len()),
    );
    let mut cursor = std::io::Cursor::new(proof);
    {
        let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
        let parsed = catch_kagemusha_native_verifier_panic("Ep proof parse", || {
            PlonkSuccinctVerifier::<Scheme>::read_proof(&svk, &protocol, instances, &mut transcript)
        })?
        .map_err(|error| format!("failed to parse Kagemusha Ep proof: {error:?}"))?;
        let accumulators = catch_kagemusha_native_verifier_panic("Ep proof verification", || {
            PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, instances, &parsed)
        })?
        .map_err(|error| format!("Kagemusha Ep succinct verification failed: {error:?}"))?;
        let [accumulator]: [_; 1] = accumulators.try_into().map_err(|accumulators: Vec<_>| {
            format!(
                "Kagemusha Ep proof emitted {} opening accumulators instead of one",
                accumulators.len()
            )
        })?;
        if cursor.position()
            != u64::try_from(proof.len()).map_err(|_| "Ep proof length does not fit u64")?
        {
            return Err("Kagemusha Ep proof has trailing bytes".to_owned());
        }
        return Ok(accumulator);
    }
}

/// Recompile the exact V4 self protocols from authenticated Params/VKs and
/// require the pair's public identities to match both compiled protocols.
fn terminal_validate_compiled_protocol_identities_v4(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<(), String> {
    use halo2_proofs::poly::commitment::Params as _;
    use snark_verifier::system::halo2::compile;

    let eq_layout = public_inputs.validate(proof_step_count, step_eq_circuit_params)?;
    let ep_layout = public_inputs.validate(proof_step_count, step_ep_circuit_params)?;
    if eq_layout != ep_layout
        || step_eq_circuit_params.k != step_ep_circuit_params.k
        || step_eq_params.k() != step_eq_circuit_params.k
        || step_ep_params.k() != step_ep_circuit_params.k
    {
        return Err("Kagemusha V4 terminal parameter/layout mismatch".to_owned());
    }
    let instance_len = usize::try_from(eq_layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 terminal public length does not fit usize".to_owned())?;
    let compile_config = || kagemusha_ipa_compile_config_v4(instance_len);
    let eq_protocol = compile(step_eq_params, step_eq_verifying_key, compile_config());
    let ep_protocol = compile(step_ep_params, step_ep_verifying_key, compile_config());
    let expected_eq = kagemusha_sha256_public_words(kagemusha_compiled_protocol_identity_sha256(
        &eq_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?);
    let expected_ep = kagemusha_sha256_public_words(kagemusha_compiled_protocol_identity_sha256(
        &ep_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?);
    if public_inputs.step_eq_compiled_protocol_sha256 != expected_eq
        || public_inputs.step_ep_compiled_protocol_sha256 != expected_ep
    {
        return Err(
            "Kagemusha V4 compiled-protocol identity does not match authenticated artifacts"
                .to_owned(),
        );
    }
    Ok(())
}

/// Fully verify and terminally decide both halves of one backend-native V4
/// pair under its authenticated release parameters.
pub(crate) fn terminal_verify_proof_pair_v4(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    pair: &KagemushaPastaCycleProofPairV4,
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
) -> Result<(), String> {
    terminal_verify_proof_pair_lineage_v4(
        step_eq_params,
        step_eq_verifying_key,
        step_ep_params,
        step_ep_verifying_key,
        pair,
        step_eq_circuit_params,
        step_ep_circuit_params,
        max_pair_bytes,
    )?;
    Ok(())
}

/// Verify a V4 pair, terminally decide both folds, and return the complete
/// lineages needed to construct an authenticated child operation.
pub(crate) fn terminal_verify_proof_pair_lineage_v4(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    pair: &KagemushaPastaCycleProofPairV4,
    step_eq_circuit_params: &KagemushaStepCircuitParamsV4,
    step_ep_circuit_params: &KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
) -> Result<(KagemushaIpaAccumulatorWireV4, KagemushaIpaAccumulatorWireV4), String> {
    pair.validate(
        step_eq_circuit_params,
        step_ep_circuit_params,
        max_pair_bytes,
    )?;
    terminal_validate_compiled_protocol_identities_v4(
        step_eq_params,
        step_eq_verifying_key,
        step_ep_params,
        step_ep_verifying_key,
        &pair.public_inputs,
        pair.proof_step_count,
        step_eq_circuit_params,
        step_ep_circuit_params,
    )?;

    let eq_instances = vec![
        pair.public_inputs
            .instance_column::<Fp>(pair.proof_step_count, step_eq_circuit_params)?,
    ];
    let eq_current = succinct_verify_step_eq_instances(
        step_eq_params,
        step_eq_verifying_key,
        &pair.step_eq_proof_bytes,
        &eq_instances,
        usize::try_from(step_eq_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?,
    )?;
    let eq_parent = pair
        .public_inputs
        .parent_eq_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_eq(step_eq_circuit_params.k))
        .transpose()?;
    let eq_lineage = super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        step_eq_params,
        step_eq_circuit_params.k,
        eq_current,
        eq_parent,
        &pair.step_eq_accumulation_proof,
    )?;

    let ep_instances = vec![
        pair.public_inputs
            .instance_column::<Fq>(pair.proof_step_count, step_ep_circuit_params)?,
    ];
    let ep_current = succinct_verify_step_ep_instances(
        step_ep_params,
        step_ep_verifying_key,
        &pair.step_ep_proof_bytes,
        &ep_instances,
        usize::try_from(step_ep_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?,
    )?;
    let ep_parent = pair
        .public_inputs
        .parent_ep_lineage_accumulator
        .as_ref()
        .map(|wire| wire.to_ep(step_ep_circuit_params.k))
        .transpose()?;
    let ep_lineage = super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        step_ep_params,
        step_ep_circuit_params.k,
        ep_current,
        ep_parent,
        &pair.step_ep_accumulation_proof,
    )?;

    Ok((
        KagemushaIpaAccumulatorWireV4::from_eq(&eq_lineage, step_eq_circuit_params.k)?,
        KagemushaIpaAccumulatorWireV4::from_ep(&ep_lineage, step_ep_circuit_params.k)?,
    ))
}

/// Parsed terminal-verifier material for one authenticated V4 release.
///
/// As with the prover, fields are private and are populated only by the V4
/// framed-artifact loader after profile, digest, key, and bootstrap checks.
fn parse_kagemusha_params_v4<C>(
    bytes: &[u8],
    expected_k: u32,
    role: &str,
) -> Result<halo2_proofs::poly::ipa::commitment::ParamsIPA<C>, String>
where
    C: CurveAffine,
{
    use halo2_proofs::poly::commitment::Params as _;

    let mut cursor = std::io::Cursor::new(bytes);
    let params = halo2_proofs::poly::ipa::commitment::ParamsIPA::<C>::read(&mut cursor)
        .map_err(|error| format!("failed to parse Kagemusha V4 {role} parameters: {error}"))?;
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| format!("Kagemusha V4 {role} parameter length does not fit u64"))?
        || params.k() != expected_k
    {
        return Err(format!(
            "Kagemusha V4 {role} parameters have a trailing byte or degree mismatch"
        ));
    }
    Ok(params)
}

fn parse_kagemusha_eq_vk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::VerifyingKey};

    let mut cursor = std::io::Cursor::new(bytes);
    #[cfg(feature = "circuit-params")]
    let key = VerifyingKey::read::<_, KagemushaStepEqCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Eq verifier key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        VerifyingKey::read::<_, KagemushaStepEqCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq verifier key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| "Kagemusha V4 Eq verifier-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Eq verifier key has trailing bytes".to_owned());
    }
    Ok(key)
}

fn parse_kagemusha_ep_vk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    use halo2_proofs::{SerdeFormat, plonk::VerifyingKey};

    let mut cursor = std::io::Cursor::new(bytes);
    #[cfg(feature = "circuit-params")]
    let key = VerifyingKey::read::<_, KagemushaStepEpCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Ep verifier key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        VerifyingKey::read::<_, KagemushaStepEpCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep verifier key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(bytes.len())
            .map_err(|_| "Kagemusha V4 Ep verifier-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Ep verifier key has trailing bytes".to_owned());
    }
    Ok(key)
}

fn validate_kagemusha_profile_protocol_v4<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<C>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    parity: KagemushaPastaCycleParityV1,
    expected_structure_sha256: [u8; 32],
    bootstrap_bytes: &[u8],
) -> Result<(KagemushaStepBootstrapV4, [u8; 32], PlonkProtocol<C>), String>
where
    C: CurveAffine,
    C::ScalarExt: halo2_base::utils::ScalarField + PrimeField,
{
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public layout does not fit usize".to_owned())?;
    let final_protocol = snark_verifier::system::halo2::compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(circuit_params)?,
        instance_column_lengths: vec![public_len],
    };
    let bootstrap_verifying_key = kagemusha_bootstrap_verifying_key_v1(params, &target)?;
    let bootstrap_protocol = snark_verifier::system::halo2::compile(
        params,
        &bootstrap_verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    let actual_structure =
        kagemusha_require_protocol_structure_v1(&bootstrap_protocol, &final_protocol, parity)?;
    if expected_structure_sha256 == [0; 32] || actual_structure != expected_structure_sha256 {
        return Err("Kagemusha V4 compiled protocol structure mismatch".to_owned());
    }
    let bootstrap = KagemushaStepBootstrapV4::decode_authenticated(
        bootstrap_bytes,
        circuit_params,
        parity,
        expected_structure_sha256,
    )?;
    bootstrap.validate_bootstrap_protocol(
        circuit_params,
        parity,
        expected_structure_sha256,
        &bootstrap_protocol,
    )?;
    let final_identity = kagemusha_compiled_protocol_identity_sha256(&final_protocol, parity)?;
    Ok((bootstrap, final_identity, final_protocol))
}

/// Terminally verify every Eq bootstrap equation before the payload can enter
/// a recursive witness. The ordinary selector-zero proof is generated by the
/// final Step proving key and is therefore verified by the final Step VK. The
/// separately reconstructed bootstrap protocol above authenticates only the
/// key-generation structure and identity recorded in the payload. The all-zero
/// parent has no carried public lineage, so the circuit selects `current`;
/// nevertheless both fixed-shape fold stages execute and must be valid for
/// `(current, current)`.
fn terminal_validate_kagemusha_eq_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    bootstrap: &KagemushaStepBootstrapV4,
) -> Result<(), String> {
    let instances = bootstrap
        .parent_slot
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|limb| Fp::from(u64::from(*limb)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let exact_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Eq bootstrap proof length does not fit usize".to_owned())?;
    let current = succinct_verify_step_eq_instances(
        params,
        step_verifying_key,
        &bootstrap.parent_slot.ordinary_proof_bytes,
        &instances,
        exact_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    let current_wire = KagemushaIpaAccumulatorWireV4::from_eq(&current, circuit_params.k)?;
    if bootstrap.parent_slot.carried_lineage != current_wire {
        return Err(
            "Kagemusha V4 Eq bootstrap carried lineage is not its proof lineage".to_owned(),
        );
    }
    let carried = bootstrap
        .parent_slot
        .carried_lineage
        .to_eq(circuit_params.k)?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(carried),
        &bootstrap.parent_slot.post_proof_fold,
    )?;
    super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(current),
        &bootstrap.branch_merge_fold,
    )?;
    Ok(())
}

/// Ep/Pallas analogue of [`terminal_validate_kagemusha_eq_bootstrap_v4`].
fn terminal_validate_kagemusha_ep_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    bootstrap: &KagemushaStepBootstrapV4,
) -> Result<(), String> {
    let instances = bootstrap
        .parent_slot
        .instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|limb| Fq::from(u64::from(*limb)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let exact_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Ep bootstrap proof length does not fit usize".to_owned())?;
    let current = succinct_verify_step_ep_instances(
        params,
        step_verifying_key,
        &bootstrap.parent_slot.ordinary_proof_bytes,
        &instances,
        exact_proof_bytes,
    )?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        None,
        &KagemushaIpaAccumulationProofV4::initialization(circuit_params.k)?,
    )?;
    let current_wire = KagemushaIpaAccumulatorWireV4::from_ep(&current, circuit_params.k)?;
    if bootstrap.parent_slot.carried_lineage != current_wire {
        return Err(
            "Kagemusha V4 Ep bootstrap carried lineage is not its proof lineage".to_owned(),
        );
    }
    let carried = bootstrap
        .parent_slot
        .carried_lineage
        .to_ep(circuit_params.k)?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(carried),
        &bootstrap.parent_slot.post_proof_fold,
    )?;
    super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        params,
        circuit_params.k,
        current.clone(),
        Some(current),
        &bootstrap.branch_merge_fold,
    )?;
    Ok(())
}

pub(crate) struct KagemushaPastaCycleTerminalVerifierV4 {
    step_eq_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    step_ep_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    max_pair_bytes: u32,
}

impl KagemushaPastaCycleTerminalVerifierV4 {
    /// Parse and cross-check all verifier roles from one authenticated release.
    pub(crate) fn from_authenticated_artifacts(
        artifacts: &super::kagemusha_artifact_v4::KagemushaPastaCycleVerifierArtifactsV4,
    ) -> Result<Self, String> {
        let step_eq = artifacts.step_eq_profile();
        let step_ep = artifacts.step_ep_profile();
        if step_eq.parity != KagemushaPastaCycleParityV1::StepEq
            || step_ep.parity != KagemushaPastaCycleParityV1::StepEp
        {
            return Err("Kagemusha V4 release profile order mismatch".to_owned());
        }
        let step_eq_params = parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
            artifacts.step_eq_parameters(),
            step_eq.ipa_k,
            "Eq",
        )?;
        let step_ep_params = parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
            artifacts.step_ep_parameters(),
            step_ep.ipa_k,
            "Ep",
        )?;
        let step_eq_verifying_key = parse_kagemusha_eq_vk_v4(
            artifacts.step_eq_verifying_key(),
            step_eq.circuit_params.clone(),
        )?;
        let step_ep_verifying_key = parse_kagemusha_ep_vk_v4(
            artifacts.step_ep_verifying_key(),
            step_ep.circuit_params.clone(),
        )?;
        let (step_eq_bootstrap, _, _) = validate_kagemusha_profile_protocol_v4(
            &step_eq_params,
            &step_eq_verifying_key,
            &step_eq.circuit_params,
            KagemushaPastaCycleParityV1::StepEq,
            step_eq.compiled_protocol_structure_sha256,
            artifacts.step_eq_bootstrap_witness(),
        )?;
        terminal_validate_kagemusha_eq_bootstrap_v4(
            &step_eq_params,
            &step_eq_verifying_key,
            &step_eq.circuit_params,
            &step_eq_bootstrap,
        )?;
        let (step_ep_bootstrap, _, _) = validate_kagemusha_profile_protocol_v4(
            &step_ep_params,
            &step_ep_verifying_key,
            &step_ep.circuit_params,
            KagemushaPastaCycleParityV1::StepEp,
            step_ep.compiled_protocol_structure_sha256,
            artifacts.step_ep_bootstrap_witness(),
        )?;
        terminal_validate_kagemusha_ep_bootstrap_v4(
            &step_ep_params,
            &step_ep_verifying_key,
            &step_ep.circuit_params,
            &step_ep_bootstrap,
        )?;
        Ok(Self {
            step_eq_params,
            step_eq_verifying_key,
            step_eq_circuit_params: step_eq.circuit_params.clone(),
            step_ep_params,
            step_ep_verifying_key,
            step_ep_circuit_params: step_ep.circuit_params.clone(),
            max_pair_bytes: artifacts.max_proof_bytes(),
        })
    }

    /// Decode and terminally decide one opaque ABI-20 pair only after its
    /// complete public state is matched to the caller's canonical statement.
    ///
    /// This keeps fold transcripts and accumulator wires private to the
    /// recursion adapter while giving the public facade a fail-closed binding
    /// check over every value needed by the lifecycle.
    pub(crate) fn verify_encoded_pair_binding(
        &self,
        bytes: &[u8],
        expected_statement: &KagemushaRecursiveSpendPublicStatementV4,
        expected_operation: &KagemushaStepOperationVectorV4,
        expected_statement_digest: [u32; 8],
        expected_state: &[u32],
        expected_proof_step_count: u32,
        expected_manifest_sha256: [u32; 8],
    ) -> Result<(), String> {
        let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
            bytes,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        if pair.proof_step_count != expected_proof_step_count
            || pair.public_inputs.public_statement_digest != expected_statement_digest
            || pair.public_inputs.result_state != expected_state
            || pair.public_inputs.manifest_sha256 != expected_manifest_sha256
        {
            return Err(
                "Kagemusha V4 proof pair does not match the canonical public statement".to_owned(),
            );
        }
        pair.public_inputs
            .operation
            .validate_terminal_statement_v4(expected_statement)?;
        if &pair.public_inputs.operation != expected_operation {
            return Err(
                "Kagemusha V4 proof pair does not match the expected semantic operation".to_owned(),
            );
        }
        self.verify_pair(&pair)
    }

    /// Decode and terminally decide the generator's unbound live-pair
    /// calibration vector. This is used only to qualify an authenticated
    /// release; lifecycle acceptance must use `verify_encoded_pair_binding`.
    pub(crate) fn verify_encoded_pair_qualification(&self, bytes: &[u8]) -> Result<(), String> {
        let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
            bytes,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        self.verify_pair(&pair)
    }

    /// Fully verify and terminally decide one decoded backend-native V4 pair.
    pub(crate) fn verify_pair(&self, pair: &KagemushaPastaCycleProofPairV4) -> Result<(), String> {
        terminal_verify_proof_pair_v4(
            &self.step_eq_params,
            &self.step_eq_verifying_key,
            &self.step_ep_params,
            &self.step_ep_verifying_key,
            pair,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )
    }
}

/// Parsed proving material for one authenticated V4 release.
///
/// Fields are private and no raw-parts constructor is exposed.  The V4
/// artifact loader is the only production constructor, preventing callers
/// from mixing local BaseConfig values, keys, or proof-size limits.
struct KagemushaExactBytesWriterV4<'a> {
    expected: &'a [u8],
    position: usize,
    mismatch: bool,
}

impl<'a> KagemushaExactBytesWriterV4<'a> {
    fn new(expected: &'a [u8]) -> Self {
        Self {
            expected,
            position: 0,
            mismatch: false,
        }
    }

    fn finish(self, label: &str) -> Result<(), String> {
        if self.mismatch || self.position != self.expected.len() {
            return Err(format!(
                "Kagemusha V4 {label} serialization does not match its canonical bytes"
            ));
        }
        Ok(())
    }
}

impl std::io::Write for KagemushaExactBytesWriterV4<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.position.checked_add(bytes.len());
        match end.and_then(|end| self.expected.get(self.position..end)) {
            Some(expected) if expected == bytes => {}
            _ => self.mismatch = true,
        }
        self.position = end.unwrap_or(usize::MAX);
        // Halo2's nested polynomial writers assume an infallible sink. Record
        // mismatches out of band and always accept the complete chunk.
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn parse_kagemusha_eq_pk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<
    (
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
        KagemushaBreakPointsV4,
    ),
    String,
> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let payload =
        super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
            bytes,
            KagemushaPastaCycleParityV1::StepEq,
            &circuit_params,
        )?;
    let break_points = kagemusha_break_points_from_wire_v4(&payload.header().break_points)?;
    let processed_proving_key = payload.processed_proving_key();
    let mut cursor = std::io::Cursor::new(processed_proving_key);
    #[cfg(feature = "circuit-params")]
    let key = ProvingKey::read::<_, KagemushaStepEqCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        ProvingKey::read::<_, KagemushaStepEqCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Eq proving key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(processed_proving_key.len())
            .map_err(|_| "Kagemusha V4 Eq proving-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Eq proving key has trailing bytes".to_owned());
    }
    Ok((key, break_points))
}

fn parse_kagemusha_ep_pk_v4(
    bytes: &[u8],
    circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<
    (
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
        KagemushaBreakPointsV4,
    ),
    String,
> {
    use halo2_proofs::{SerdeFormat, plonk::ProvingKey};

    let payload =
        super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
            bytes,
            KagemushaPastaCycleParityV1::StepEp,
            &circuit_params,
        )?;
    let break_points = kagemusha_break_points_from_wire_v4(&payload.header().break_points)?;
    let processed_proving_key = payload.processed_proving_key();
    let mut cursor = std::io::Cursor::new(processed_proving_key);
    #[cfg(feature = "circuit-params")]
    let key = ProvingKey::read::<_, KagemushaStepEpCircuitV4>(
        &mut cursor,
        SerdeFormat::Processed,
        circuit_params,
    )
    .map_err(|error| format!("failed to parse Kagemusha V4 Ep proving key: {error}"))?;
    #[cfg(not(feature = "circuit-params"))]
    let key = {
        let _ = circuit_params;
        ProvingKey::read::<_, KagemushaStepEpCircuitV4>(&mut cursor, SerdeFormat::Processed)
            .map_err(|error| format!("failed to parse Kagemusha V4 Ep proving key: {error}"))?
    };
    if cursor.position()
        != u64::try_from(processed_proving_key.len())
            .map_err(|_| "Kagemusha V4 Ep proving-key length does not fit u64".to_owned())?
    {
        return Err("Kagemusha V4 Ep proving key has trailing bytes".to_owned());
    }
    Ok((key, break_points))
}

fn kagemusha_eq_succinct_vk_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
) -> Result<
    snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    String,
> {
    use halo2_proofs::{
        halo2curves::{CurveExt as _, group::Curve as _, pasta::Eq},
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        pcs::ipa::IpaSuccinctVerifyingKey,
        util::arithmetic::{Domain, root_of_unity},
    };

    let k = usize::try_from(params.k())
        .map_err(|_| "Kagemusha V4 Eq parameter degree does not fit usize".to_owned())?;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    Ok(IpaSuccinctVerifyingKey::new(
        Domain::new(k, root_of_unity(k)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    ))
}

fn kagemusha_ep_succinct_vk_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
) -> Result<
    snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    String,
> {
    use halo2_proofs::{
        halo2curves::{CurveExt as _, group::Curve as _, pasta::Ep},
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        pcs::ipa::IpaSuccinctVerifyingKey,
        util::arithmetic::{Domain, root_of_unity},
    };

    let k = usize::try_from(params.k())
        .map_err(|_| "Kagemusha V4 Ep parameter degree does not fit usize".to_owned())?;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    Ok(IpaSuccinctVerifyingKey::new(
        Domain::new(k, root_of_unity(k)),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    ))
}

pub(crate) struct KagemushaPastaCycleProverV4 {
    manifest_sha256: [u8; 32],
    step_eq_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_proving_key:
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_break_points: KagemushaBreakPointsV4,
    step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    step_eq_bootstrap: KagemushaStepBootstrapV4,
    step_eq_compiled_protocol_sha256: [u8; 32],
    step_eq_compiled_parent_protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_eq_succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params:
        halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_proving_key:
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_break_points: KagemushaBreakPointsV4,
    step_ep_circuit_params: KagemushaStepCircuitParamsV4,
    step_ep_bootstrap: KagemushaStepBootstrapV4,
    step_ep_compiled_protocol_sha256: [u8; 32],
    step_ep_compiled_parent_protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    step_ep_succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    max_pair_bytes: u32,
}

impl KagemushaPastaCycleProverV4 {
    /// Parse all eight authenticated roles and reject cross-key/profile material.
    pub(crate) fn from_authenticated_artifacts(
        artifacts: &super::kagemusha_artifact_v4::KagemushaPastaCycleProverArtifactsV4,
    ) -> Result<Self, String> {
        use halo2_proofs::SerdeFormat;

        let verifier = artifacts.verifier();
        let step_eq = verifier.step_eq_profile();
        let step_ep = verifier.step_ep_profile();
        if step_eq.parity != KagemushaPastaCycleParityV1::StepEq
            || step_ep.parity != KagemushaPastaCycleParityV1::StepEp
        {
            return Err("Kagemusha V4 release profile order mismatch".to_owned());
        }
        let step_eq_params = parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
            verifier.step_eq_parameters(),
            step_eq.ipa_k,
            "Eq",
        )?;
        let step_ep_params = parse_kagemusha_params_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
            verifier.step_ep_parameters(),
            step_ep.ipa_k,
            "Ep",
        )?;
        let (step_eq_proving_key, step_eq_break_points) = parse_kagemusha_eq_pk_v4(
            artifacts.step_eq_proving_key(),
            step_eq.circuit_params.clone(),
        )?;
        let (step_ep_proving_key, step_ep_break_points) = parse_kagemusha_ep_pk_v4(
            artifacts.step_ep_proving_key(),
            step_ep.circuit_params.clone(),
        )?;
        if step_eq_proving_key
            .get_vk()
            .to_bytes(SerdeFormat::Processed)
            != verifier.step_eq_verifying_key()
            || step_ep_proving_key
                .get_vk()
                .to_bytes(SerdeFormat::Processed)
                != verifier.step_ep_verifying_key()
        {
            return Err("Kagemusha V4 proving key embeds a different verifier key".to_owned());
        }
        let (step_eq_bootstrap, step_eq_compiled_protocol_sha256, step_eq_compiled_parent_protocol) =
            validate_kagemusha_profile_protocol_v4(
                &step_eq_params,
                step_eq_proving_key.get_vk(),
                &step_eq.circuit_params,
                KagemushaPastaCycleParityV1::StepEq,
                step_eq.compiled_protocol_structure_sha256,
                verifier.step_eq_bootstrap_witness(),
            )?;
        terminal_validate_kagemusha_eq_bootstrap_v4(
            &step_eq_params,
            step_eq_proving_key.get_vk(),
            &step_eq.circuit_params,
            &step_eq_bootstrap,
        )?;
        let (step_ep_bootstrap, step_ep_compiled_protocol_sha256, step_ep_compiled_parent_protocol) =
            validate_kagemusha_profile_protocol_v4(
                &step_ep_params,
                step_ep_proving_key.get_vk(),
                &step_ep.circuit_params,
                KagemushaPastaCycleParityV1::StepEp,
                step_ep.compiled_protocol_structure_sha256,
                verifier.step_ep_bootstrap_witness(),
            )?;
        terminal_validate_kagemusha_ep_bootstrap_v4(
            &step_ep_params,
            step_ep_proving_key.get_vk(),
            &step_ep.circuit_params,
            &step_ep_bootstrap,
        )?;
        let step_eq_succinct_vk = kagemusha_eq_succinct_vk_v4(&step_eq_params)?;
        let step_ep_succinct_vk = kagemusha_ep_succinct_vk_v4(&step_ep_params)?;
        Ok(Self {
            manifest_sha256: artifacts.manifest_sha256(),
            step_eq_params,
            step_eq_proving_key,
            step_eq_break_points,
            step_eq_circuit_params: step_eq.circuit_params.clone(),
            step_eq_bootstrap,
            step_eq_compiled_protocol_sha256,
            step_eq_compiled_parent_protocol,
            step_eq_succinct_vk,
            step_ep_params,
            step_ep_proving_key,
            step_ep_break_points,
            step_ep_circuit_params: step_ep.circuit_params.clone(),
            step_ep_bootstrap,
            step_ep_compiled_protocol_sha256,
            step_ep_compiled_parent_protocol,
            step_ep_succinct_vk,
            max_pair_bytes: artifacts.max_proof_bytes(),
        })
    }

    pub(crate) fn step_eq_compiled_protocol_sha256(&self) -> [u8; 32] {
        self.step_eq_compiled_protocol_sha256
    }

    pub(crate) fn step_ep_compiled_protocol_sha256(&self) -> [u8; 32] {
        self.step_ep_compiled_protocol_sha256
    }

    fn step_eq_parent_from_pair_v4(
        &self,
        pair: &KagemushaPastaCycleProofPairV4,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EqAffine>, String>
    {
        let instances = vec![
            pair.public_inputs
                .instance_column::<Fp>(pair.proof_step_count, &self.step_eq_circuit_params)?,
        ];
        let (carried_lineage, external_accumulation_proof) = if pair.public_inputs.parent_count == 0
        {
            (
                self.step_eq_bootstrap
                    .parent_slot
                    .carried_lineage
                    .to_eq(self.step_eq_circuit_params.k)?,
                self.step_eq_bootstrap.parent_slot.post_proof_fold.clone(),
            )
        } else {
            (
                pair.public_inputs
                    .parent_eq_lineage_accumulator
                    .as_ref()
                    .ok_or_else(|| "Kagemusha V4 Eq parent omitted its carried lineage".to_owned())?
                    .to_eq(self.step_eq_circuit_params.k)?,
                pair.step_eq_accumulation_proof.clone(),
            )
        };
        Ok(KagemushaStepParentProofV4 {
            instances,
            proof_bytes: pair.step_eq_proof_bytes.clone(),
            carried_lineage,
            external_accumulation_proof,
        })
    }

    fn step_ep_parent_from_pair_v4(
        &self,
        pair: &KagemushaPastaCycleProofPairV4,
    ) -> Result<KagemushaStepParentProofV4<halo2_proofs::halo2curves::pasta::EpAffine>, String>
    {
        let instances = vec![
            pair.public_inputs
                .instance_column::<Fq>(pair.proof_step_count, &self.step_ep_circuit_params)?,
        ];
        let (carried_lineage, external_accumulation_proof) = if pair.public_inputs.parent_count == 0
        {
            (
                self.step_ep_bootstrap
                    .parent_slot
                    .carried_lineage
                    .to_ep(self.step_ep_circuit_params.k)?,
                self.step_ep_bootstrap.parent_slot.post_proof_fold.clone(),
            )
        } else {
            (
                pair.public_inputs
                    .parent_ep_lineage_accumulator
                    .as_ref()
                    .ok_or_else(|| "Kagemusha V4 Ep parent omitted its carried lineage".to_owned())?
                    .to_ep(self.step_ep_circuit_params.k)?,
                pair.step_ep_accumulation_proof.clone(),
            )
        };
        Ok(KagemushaStepParentProofV4 {
            instances,
            proof_bytes: pair.step_ep_proof_bytes.clone(),
            carried_lineage,
            external_accumulation_proof,
        })
    }

    fn prepare_step_recursions_v4(
        &self,
        public_inputs: &mut KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
    ) -> Result<
        (
            KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>,
            KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>,
        ),
        String,
    > {
        if parent_pair_bytes.len() > KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            return Err("Kagemusha V4 operation consumes more than two parents".to_owned());
        }
        let manifest_words = kagemusha_exact_u32_public_limbs(self.manifest_sha256);
        let eq_protocol_words =
            kagemusha_sha256_public_words(self.step_eq_compiled_protocol_sha256);
        let ep_protocol_words =
            kagemusha_sha256_public_words(self.step_ep_compiled_protocol_sha256);

        let mut parents = Vec::with_capacity(parent_pair_bytes.len());
        for bytes in parent_pair_bytes {
            let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
                bytes,
                &self.step_eq_circuit_params,
                &self.step_ep_circuit_params,
                self.max_pair_bytes,
            )?;
            if pair.public_inputs.manifest_sha256 != manifest_words
                || pair.public_inputs.step_eq_compiled_protocol_sha256 != eq_protocol_words
                || pair.public_inputs.step_ep_compiled_protocol_sha256 != ep_protocol_words
            {
                return Err(
                    "Kagemusha V4 parent pair belongs to a different authenticated release"
                        .to_owned(),
                );
            }
            let (eq_lineage, ep_lineage) = terminal_verify_proof_pair_lineage_v4(
                &self.step_eq_params,
                self.step_eq_proving_key.get_vk(),
                &self.step_ep_params,
                self.step_ep_proving_key.get_vk(),
                &pair,
                &self.step_eq_circuit_params,
                &self.step_ep_circuit_params,
                self.max_pair_bytes,
            )?;
            parents.push((pair, eq_lineage, ep_lineage));
        }

        public_inputs.parent_count = u32::try_from(parents.len())
            .map_err(|_| "Kagemusha V4 parent count does not fit u32".to_owned())?;
        public_inputs.manifest_sha256 = manifest_words;
        public_inputs.step_eq_compiled_protocol_sha256 = eq_protocol_words;
        public_inputs.step_ep_compiled_protocol_sha256 = ep_protocol_words;
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            public_inputs.parent_states[slot] = parents.get(slot).map_or_else(
                || {
                    vec![
                        0;
                        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2
                    ]
                },
                |(pair, _, _)| pair.public_inputs.result_state.clone(),
            );
            // The native audit-derivation prepass accepts only blank derived-join
            // slots. It derives both real digests below before either
            // proof circuit is built, so no placeholder can enter a proof.
            public_inputs.parent_eq_deferred_sha256[slot] = [0; 8];
            public_inputs.parent_ep_deferred_sha256[slot] = [0; 8];
        }

        let (parent_eq_lineage_accumulator, eq_branch_merge_fold) = match parents.as_slice() {
            [] => (None, self.step_eq_bootstrap.branch_merge_fold.clone()),
            [(_, lineage, _)] => (
                Some(lineage.clone()),
                self.step_eq_bootstrap.branch_merge_fold.clone(),
            ),
            [(_, first, _), (_, second, _)] => {
                let (fold, accumulated) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
                    &self.step_eq_params,
                    self.step_eq_circuit_params.k,
                    first.to_eq(self.step_eq_circuit_params.k)?,
                    Some(second.to_eq(self.step_eq_circuit_params.k)?),
                )?;
                (
                    Some(KagemushaIpaAccumulatorWireV4::from_eq(
                        &accumulated,
                        self.step_eq_circuit_params.k,
                    )?),
                    fold,
                )
            }
            _ => unreachable!("parent count was bounded above"),
        };
        let (parent_ep_lineage_accumulator, ep_branch_merge_fold) = match parents.as_slice() {
            [] => (None, self.step_ep_bootstrap.branch_merge_fold.clone()),
            [(_, _, lineage)] => (
                Some(lineage.clone()),
                self.step_ep_bootstrap.branch_merge_fold.clone(),
            ),
            [(_, _, first), (_, _, second)] => {
                let (fold, accumulated) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
                    &self.step_ep_params,
                    self.step_ep_circuit_params.k,
                    first.to_ep(self.step_ep_circuit_params.k)?,
                    Some(second.to_ep(self.step_ep_circuit_params.k)?),
                )?;
                (
                    Some(KagemushaIpaAccumulatorWireV4::from_ep(
                        &accumulated,
                        self.step_ep_circuit_params.k,
                    )?),
                    fold,
                )
            }
            _ => unreachable!("parent count was bounded above"),
        };
        public_inputs.parent_eq_lineage_accumulator = parent_eq_lineage_accumulator;
        public_inputs.parent_ep_lineage_accumulator = parent_ep_lineage_accumulator;

        let mut eq_parent_witnesses = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
        let mut ep_parent_witnesses = Vec::with_capacity(KAGEMUSHA_PASTA_PARENT_SLOTS_V1);
        for slot in 0..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
            if let Some((pair, _, _)) = parents.get(slot) {
                eq_parent_witnesses.push(self.step_eq_parent_from_pair_v4(pair)?);
                ep_parent_witnesses.push(self.step_ep_parent_from_pair_v4(pair)?);
            } else {
                eq_parent_witnesses.push(self.step_eq_bootstrap.step_eq_parent(
                    &self.step_eq_circuit_params,
                    self.step_eq_bootstrap.compiled_protocol_structure_sha256,
                    slot,
                )?);
                ep_parent_witnesses.push(self.step_ep_bootstrap.step_ep_parent(
                    &self.step_ep_circuit_params,
                    self.step_ep_bootstrap.compiled_protocol_structure_sha256,
                    slot,
                )?);
            }
        }
        let step_eq_recursion = KagemushaStepParityRecursionV4 {
            succinct_vk: self.step_eq_succinct_vk.clone(),
            compiled_parent_protocol: self.step_eq_compiled_parent_protocol.clone(),
            fixed_structure_sha256: self.step_eq_bootstrap.compiled_protocol_structure_sha256,
            parents: eq_parent_witnesses.try_into().map_err(|parents: Vec<_>| {
                format!(
                    "Kagemusha V4 Eq recursion has {} parents instead of two",
                    parents.len()
                )
            })?,
            branch_merge_fold: eq_branch_merge_fold,
        };
        let step_ep_recursion = KagemushaStepParityRecursionV4 {
            succinct_vk: self.step_ep_succinct_vk.clone(),
            compiled_parent_protocol: self.step_ep_compiled_parent_protocol.clone(),
            fixed_structure_sha256: self.step_ep_bootstrap.compiled_protocol_structure_sha256,
            parents: ep_parent_witnesses.try_into().map_err(|parents: Vec<_>| {
                format!(
                    "Kagemusha V4 Ep recursion has {} parents instead of two",
                    parents.len()
                )
            })?,
            branch_merge_fold: ep_branch_merge_fold,
        };

        if public_inputs.parent_count == 0 {
            // Initialization has no public deferred join. The fixed Step
            // circuits still execute both verifier halves and constrain both
            // zero slots; only the native derivation passes are unnecessary.
            public_inputs.parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
            public_inputs.parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        } else {
            // Keep the public inputs blank until both independent derivations
            // have finished: the derivation boundary deliberately rejects a
            // caller-preselected join. Each large witness-only builder drops
            // inside its scope before the opposite parity starts.
            let eq_public_words = {
                let audits = collect_kagemusha_scalar_audits_v4::<
                    halo2_proofs::halo2curves::pasta::EqAffine,
                >(
                    public_inputs,
                    proof_step_count,
                    &self.step_eq_circuit_params,
                    &step_eq_recursion,
                    KagemushaPastaCycleParityV1::StepEq,
                )?;
                kagemusha_deferred_audit_public_words_v4(
                    &audits.audit,
                    &audits.stages,
                    public_inputs.parent_count,
                    audits.inner_parent_counts,
                )?
            };
            let ep_public_words = {
                let audits = collect_kagemusha_scalar_audits_v4::<
                    halo2_proofs::halo2curves::pasta::EpAffine,
                >(
                    public_inputs,
                    proof_step_count,
                    &self.step_ep_circuit_params,
                    &step_ep_recursion,
                    KagemushaPastaCycleParityV1::StepEp,
                )?;
                kagemusha_deferred_audit_public_words_v4(
                    &audits.audit,
                    &audits.stages,
                    public_inputs.parent_count,
                    audits.inner_parent_counts,
                )?
            };
            public_inputs.parent_eq_deferred_sha256 = eq_public_words;
            public_inputs.parent_ep_deferred_sha256 = ep_public_words;
        }
        let eq_layout = public_inputs.validate(proof_step_count, &self.step_eq_circuit_params)?;
        let ep_layout = public_inputs.validate(proof_step_count, &self.step_ep_circuit_params)?;
        if eq_layout != ep_layout {
            return Err("Kagemusha V4 prepared Eq/Ep public layouts differ".to_owned());
        }
        Ok((step_eq_recursion, step_ep_recursion))
    }

    /// Prepare canonical real-or-bootstrap parent slots, derive both deferred
    /// audit joins, build both concrete circuits, and return a terminally
    /// verified backend-native V4 proof pair.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_operation_v4(
        &self,
        mut public_inputs: KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
        output_membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    ) -> Result<KagemushaPastaCycleProofPairV4, String> {
        let (step_eq_recursion, step_ep_recursion) = self.prepare_step_recursions_v4(
            &mut public_inputs,
            proof_step_count,
            parent_pair_bytes,
        )?;
        let result_frontier = public_inputs
            .result_state
            .get(super::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX)
            .copied()
            .ok_or_else(|| "Kagemusha V4 result state omits its frontier".to_owned())?;
        if result_frontier != output_membership.dummy_leaf_index {
            return Err("Kagemusha V4 result state/frontier witness mismatch".to_owned());
        }
        let expected_parent_frontier = match output_membership.operation {
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::Init => None,
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::Split => output_membership
                .recipient
                .as_ref()
                .map(|leaf| leaf.leaf_index),
            super::kagemusha_v2::KagemushaOutputMembershipOperationV4::RedemptionChange => {
                output_membership
                    .change
                    .as_ref()
                    .map(|leaf| leaf.leaf_index)
            }
        };
        match expected_parent_frontier {
            None if public_inputs.parent_count == 0 => {}
            Some(expected) if public_inputs.parent_count > 0 => {
                for parent in public_inputs
                    .parent_states
                    .iter()
                    .take(public_inputs.parent_count as usize)
                {
                    if parent
                        .get(super::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX)
                        .copied()
                        != Some(expected)
                    {
                        return Err(
                            "Kagemusha V4 output insertion does not start at the parent frontier"
                                .to_owned(),
                        );
                    }
                }
            }
            _ => return Err("Kagemusha V4 membership/parent profile mismatch".to_owned()),
        }
        let witness = KagemushaStepWitnessV4 {
            public_inputs: &public_inputs,
            proof_step_count,
            secure,
            output_membership,
            step_eq_recursion: &step_eq_recursion,
            step_ep_recursion: &step_ep_recursion,
            step_eq_bootstrap: Some(&self.step_eq_bootstrap),
            step_ep_bootstrap: Some(&self.step_ep_bootstrap),
        };
        let circuits = build_kagemusha_step_circuits_v4(
            &witness,
            self.step_eq_circuit_params.clone(),
            self.step_ep_circuit_params.clone(),
            &self.step_eq_break_points,
            &self.step_ep_break_points,
        )?;
        self.prove_step_v4(circuits, public_inputs, proof_step_count)
    }

    /// Prove and terminally decide one operation, then expose only canonical
    /// opaque ABI-20 bytes to the public lifecycle facade.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_operation_encoded_v4(
        &self,
        public_inputs: KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
        parent_pair_bytes: &[&[u8]],
        secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
        output_membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    ) -> Result<Vec<u8>, String> {
        let pair = self.prove_operation_v4(
            public_inputs,
            proof_step_count,
            parent_pair_bytes,
            secure,
            output_membership,
        )?;
        pair.encode_authenticated(
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )
    }

    /// Prove both concrete V4 halves, fold each current opening with its parent
    /// lineage, and terminally decide the resulting pair before returning it.
    pub(crate) fn prove_step_v4(
        &self,
        circuits: KagemushaStepCircuitsV4,
        public_inputs: KagemushaPastaCyclePublicInputsV4,
        proof_step_count: u32,
    ) -> Result<KagemushaPastaCycleProofPairV4, String> {
        let eq_layout = public_inputs.validate(proof_step_count, &self.step_eq_circuit_params)?;
        let ep_layout = public_inputs.validate(proof_step_count, &self.step_ep_circuit_params)?;
        if eq_layout != ep_layout || self.step_eq_circuit_params.k != self.step_ep_circuit_params.k
        {
            return Err("Kagemusha V4 prover Eq/Ep profile mismatch".to_owned());
        }

        let step_eq_proof_bytes = prove_step_eq_v4(
            &self.step_eq_params,
            &self.step_eq_proving_key,
            circuits.step_eq,
            &public_inputs,
            proof_step_count,
            &self.step_eq_circuit_params,
        )?;
        let step_ep_proof_bytes = prove_step_ep_v4(
            &self.step_ep_params,
            &self.step_ep_proving_key,
            circuits.step_ep,
            &public_inputs,
            proof_step_count,
            &self.step_ep_circuit_params,
        )?;

        let eq_instances = vec![
            public_inputs.instance_column::<Fp>(proof_step_count, &self.step_eq_circuit_params)?,
        ];
        let eq_current = succinct_verify_step_eq_instances(
            &self.step_eq_params,
            self.step_eq_proving_key.get_vk(),
            &step_eq_proof_bytes,
            &eq_instances,
            usize::try_from(self.step_eq_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?,
        )?;
        let eq_parent = public_inputs
            .parent_eq_lineage_accumulator
            .as_ref()
            .map(|wire| wire.to_eq(self.step_eq_circuit_params.k))
            .transpose()?;
        let (step_eq_accumulation_proof, _) =
            super::kagemusha_accumulation::fold_eq_accumulators_v4(
                &self.step_eq_params,
                self.step_eq_circuit_params.k,
                eq_current,
                eq_parent,
            )?;

        let ep_instances = vec![
            public_inputs.instance_column::<Fq>(proof_step_count, &self.step_ep_circuit_params)?,
        ];
        let ep_current = succinct_verify_step_ep_instances(
            &self.step_ep_params,
            self.step_ep_proving_key.get_vk(),
            &step_ep_proof_bytes,
            &ep_instances,
            usize::try_from(self.step_ep_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?,
        )?;
        let ep_parent = public_inputs
            .parent_ep_lineage_accumulator
            .as_ref()
            .map(|wire| wire.to_ep(self.step_ep_circuit_params.k))
            .transpose()?;
        let (step_ep_accumulation_proof, _) =
            super::kagemusha_accumulation::fold_ep_accumulators_v4(
                &self.step_ep_params,
                self.step_ep_circuit_params.k,
                ep_current,
                ep_parent,
            )?;

        let pair = KagemushaPastaCycleProofPairV4 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
            proof_step_count,
            public_inputs,
            step_eq_proof_bytes,
            step_ep_proof_bytes,
            step_eq_accumulation_proof,
            step_ep_accumulation_proof,
        };
        pair.validate(
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        terminal_verify_proof_pair_v4(
            &self.step_eq_params,
            self.step_eq_proving_key.get_vk(),
            &self.step_ep_params,
            self.step_ep_proving_key.get_vk(),
            &pair,
            &self.step_eq_circuit_params,
            &self.step_ep_circuit_params,
            self.max_pair_bytes,
        )?;
        Ok(pair)
    }
}

/// Circuit-side parent-proof and lineage-accumulation primitives shared by
/// the fixed StepEq and StepEp builders.
mod scalar_lineage_v1 {
    use std::{
        cell::Cell,
        io::{self, Read},
        ops::Range,
        rc::Rc,
    };

    use halo2_base::{
        AssignedValue,
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions, RangeInstructions},
        utils::{BigPrimeField, CurveAffineExt},
    };
    use halo2_proofs::halo2curves::ff::Field as _;
    use snark_verifier::{
        Error,
        loader::{halo2::Halo2Loader, native::NativeLoader},
        pcs::{
            AccumulationScheme,
            ipa::{Bgh19, IpaAccumulator, IpaAs, IpaSuccinctVerifyingKey},
        },
        system::halo2::transcript::halo2::PoseidonTranscript,
        verifier::{
            SnarkVerifier,
            plonk::{PlonkProtocol, PlonkSuccinctVerifier},
        },
    };

    use super::{
        KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1,
        KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1, KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS, KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS, KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_SECURE_MDS, KAGEMUSHA_POSEIDON_WIDTH, KagemushaPastaCycleParityV1,
        kagemusha_compiled_protocol_structure_sha256, protocol_parity_tag,
    };
    use crate::zk::{
        kagemusha_accumulation::{
            KagemushaIpaAccumulationProofV4, kagemusha_ipa_accumulation_proof_bytes_v4,
        },
        kagemusha_cycle_loader::DeferredScalarEccChip,
    };

    type DeferredLoader<'chip, C> = Rc<Halo2Loader<C, DeferredScalarEccChip<'chip, C>>>;
    type DeferredLoadedScalar<'chip, C> =
        snark_verifier::loader::halo2::Scalar<C, DeferredScalarEccChip<'chip, C>>;
    pub(super) type DeferredAccumulator<'chip, C> = IpaAccumulator<C, DeferredLoader<'chip, C>>;
    type DeferredTranscript<'chip, C, R> = PoseidonTranscript<
        C,
        DeferredLoader<'chip, C>,
        R,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;

    /// Native values transported to the reciprocal point half after the
    /// scalar half has witness-loaded and identity-bound one parent protocol.
    #[derive(Clone, Debug)]
    pub(super) struct DeferredProtocolIdentityWitness<C>
    where
        C: CurveAffineExt,
    {
        /// Exact fixed protocol-structure digest embedded by the circuit.
        pub(super) structure_sha256: [u8; 32],
        /// Protocol parity/domain tag.
        pub(super) parity: KagemushaPastaCycleParityV1,
        /// Self-referential VK commitments, in compiled-protocol order.
        pub(super) preprocessed: Vec<C>,
        /// Exact verifier-key transcript initial state.
        pub(super) transcript_initial_state: C::ScalarExt,
    }

    /// One witness-loaded compiled protocol whose dynamic values have already
    /// been constrained to the release identity public input.
    pub(super) struct LoadedParentProtocolV1<'chip, C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        pub(super) protocol: PlonkProtocol<C, DeferredLoader<'chip, C>>,
        pub(super) identity_witness: DeferredProtocolIdentityWitness<C>,
    }

    /// One parent-instance copy binding used by the fixed-shape V4 verifier.
    ///
    /// The binding itself carries no host presence flag.  Its equality is
    /// gated exclusively by the already-constrained current-Step slot bit
    /// passed to [`constrain_parent_scalar_lineage_v4`].
    pub(super) struct ParentInstanceCopyBindingV4<'a, F>
    where
        F: ff::Field,
    {
        /// Parent proof instance column.
        pub(super) column: usize,
        /// Exact source range in that parent column.
        pub(super) source: Range<usize>,
        /// Current Step cells receiving the conditional copy constraint.
        pub(super) expected: &'a [AssignedValue<F>],
    }

    /// One parent ordinary proof together with the external fold that completed
    /// that parent's lineage after its outer proof was created.
    pub(super) struct ParentScalarLineageWitnessV4<'a, C>
    where
        C: CurveAffineExt,
    {
        /// Exact parent public instances, real or authenticated bootstrap.
        pub(super) instances: &'a [Vec<C::ScalarExt>],
        /// Exact ordinary parent transcript.
        pub(super) proof_bytes: &'a [u8],
        /// Always-present, non-identity carried accumulator.
        pub(super) carried_lineage: &'a IpaAccumulator<C, NativeLoader>,
        /// Instance column containing the dynamic accumulator vector.
        pub(super) carried_lineage_instance_column: usize,
        /// Exact degree-derived carried-accumulator range.
        pub(super) carried_lineage_instance_range: Range<usize>,
        /// Parent slices rebound to the current Step public boundary.
        pub(super) instance_copy_bindings: &'a [ParentInstanceCopyBindingV4<'a, C::ScalarExt>],
        /// Always-present degree-specific post-proof BGH19 transcript.
        pub(super) external_accumulation_proof: &'a KagemushaIpaAccumulationProofV4,
    }

    /// Semantic reason an exact range of deferred curve equations is enabled.
    ///
    /// The enum, rather than a caller-provided Boolean vector, is retained in
    /// the fixed audit shape.  The scalar half derives its assigned selector
    /// from verified parent instances; the reciprocal half derives the same
    /// selector from the cross-bound parent-count witnesses and public slot
    /// bits.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum DeferredEquationGateV4 {
        /// Ordinary succinct verification for one parent slot.
        ParentCurrent { slot: usize },
        /// Parent-current plus carried-lineage BGH19 fold.
        ParentCarriedFold { slot: usize },
        /// Selection of the parent's current or folded lineage.
        ParentLineageSelect { slot: usize },
        /// Two-parent branch BGH19 fold.
        BranchFold,
        /// Selection of parent zero or the two-parent branch fold.
        BranchSelect,
    }

    impl DeferredEquationGateV4 {
        /// Stable tag committed in the recursive deferred-audit preimage.
        pub(super) fn audit_tag(self) -> u32 {
            match self {
                Self::ParentCurrent { slot: 0 } => 1,
                Self::ParentCurrent { slot: 1 } => 2,
                Self::ParentCarriedFold { slot: 0 } => 3,
                Self::ParentCarriedFold { slot: 1 } => 4,
                Self::ParentLineageSelect { slot: 0 } => 5,
                Self::ParentLineageSelect { slot: 1 } => 6,
                Self::BranchFold => 7,
                Self::BranchSelect => 8,
                Self::ParentCurrent { .. }
                | Self::ParentCarriedFold { .. }
                | Self::ParentLineageSelect { .. } => {
                    unreachable!("validated V4 parent slot is zero or one")
                }
            }
        }
    }

    /// One contiguous, fixed-shape range of deferred equations and its
    /// in-circuit scalar selector.
    #[derive(Clone, Debug)]
    pub(super) struct AssignedDeferredEquationStageV4<F>
    where
        F: ff::Field,
    {
        pub(super) range: Range<usize>,
        pub(super) gate: DeferredEquationGateV4,
        pub(super) enabled: AssignedValue<F>,
    }

    /// Field-independent compiled shape of one deferred-equation stage.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct DeferredEquationStageShapeV4 {
        pub(super) range: Range<usize>,
        pub(super) gate: DeferredEquationGateV4,
    }

    impl<F> AssignedDeferredEquationStageV4<F>
    where
        F: ff::Field,
    {
        pub(super) fn shape(&self) -> DeferredEquationStageShapeV4 {
            DeferredEquationStageShapeV4 {
                range: self.range.clone(),
                gate: self.gate,
            }
        }
    }

    /// Complete selected lineage for one parent slot.
    pub(super) struct ParentScalarLineageV4<'chip, C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        pub(super) accumulator: DeferredAccumulator<'chip, C>,
        pub(super) stages: Vec<AssignedDeferredEquationStageV4<C::ScalarExt>>,
    }

    /// Unconditionally-computed two-parent branch candidate and its fixed
    /// deferred-equation stages.
    pub(super) struct ExposedParentLineageV4<C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        pub(super) stages: Vec<AssignedDeferredEquationStageV4<C::ScalarExt>>,
    }

    /// Require the complete post-branch V4 stage order for the shared public
    /// deferred audit.
    ///
    /// Both V4 slots bind the same complete audit. Slot presence
    /// only controls public exposure of that digest.  This is required for a
    /// one-parent step: its enabled `BranchSelect` equation is created after
    /// parent zero and therefore must be covered by slot zero's non-zero join.
    pub(super) fn validate_stage_shapes_v4(
        stages: &[DeferredEquationStageShapeV4],
        equation_count: usize,
    ) -> Result<(), Error> {
        const COMPLETE: [DeferredEquationGateV4; 8] = [
            DeferredEquationGateV4::ParentCurrent { slot: 0 },
            DeferredEquationGateV4::ParentCarriedFold { slot: 0 },
            DeferredEquationGateV4::ParentLineageSelect { slot: 0 },
            DeferredEquationGateV4::ParentCurrent { slot: 1 },
            DeferredEquationGateV4::ParentCarriedFold { slot: 1 },
            DeferredEquationGateV4::ParentLineageSelect { slot: 1 },
            DeferredEquationGateV4::BranchFold,
            DeferredEquationGateV4::BranchSelect,
        ];
        if stages.len() != COMPLETE.len()
            || stages
                .iter()
                .zip(COMPLETE)
                .any(|(stage, expected)| stage.gate != expected)
        {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 deferred stages do not have the complete post-branch order"
                    .to_owned(),
            ));
        }
        let mut cursor = 0;
        for stage in stages {
            if stage.range.start != cursor
                || stage.range.start >= stage.range.end
                || stage.range.end > equation_count
            {
                return Err(Error::AssertionFailure(
                    "Kagemusha V4 deferred stages are not a contiguous audit partition".to_owned(),
                ));
            }
            cursor = stage.range.end;
        }
        if cursor != equation_count {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 deferred stages do not cover the complete post-branch audit"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    fn expand_stage_plan_v4<F>(
        stages: &[AssignedDeferredEquationStageV4<F>],
        equation_count: usize,
    ) -> Result<(Vec<u32>, Vec<AssignedValue<F>>), Error>
    where
        F: ff::Field,
    {
        let shapes = stages
            .iter()
            .map(AssignedDeferredEquationStageV4::shape)
            .collect::<Vec<_>>();
        validate_stage_shapes_v4(&shapes, equation_count)?;
        let mut gate_tags = Vec::with_capacity(equation_count);
        let mut selectors = Vec::with_capacity(equation_count);
        for stage in stages {
            gate_tags.extend(std::iter::repeat_n(
                stage.gate.audit_tag(),
                stage.range.len(),
            ));
            selectors.extend(std::iter::repeat_n(stage.enabled, stage.range.len()));
        }
        Ok((gate_tags, selectors))
    }

    /// A `Read` implementation whose position remains observable after the
    /// transcript borrows it, allowing every parser to reject trailing bytes.
    #[derive(Clone, Debug)]
    struct ExactReader<'a> {
        bytes: &'a [u8],
        position: Rc<Cell<usize>>,
    }

    impl<'a> ExactReader<'a> {
        fn new(bytes: &'a [u8]) -> (Self, Rc<Cell<usize>>) {
            let position = Rc::new(Cell::new(0));
            (
                Self {
                    bytes,
                    position: Rc::clone(&position),
                },
                position,
            )
        }
    }

    impl Read for ExactReader<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            let start = self.position.get();
            let available = &self.bytes[start..];
            let len = available.len().min(output.len());
            output[..len].copy_from_slice(&available[..len]);
            self.position.set(start + len);
            Ok(len)
        }
    }

    fn transcript_error(message: impl Into<String>) -> Error {
        Error::Transcript(io::ErrorKind::InvalidData, message.into())
    }

    fn push_constant_bytes<F: BigPrimeField>(
        ctx: &mut halo2_base::Context<F>,
        output: &mut Vec<AssignedValue<F>>,
        bytes: &[u8],
    ) {
        output.extend(
            bytes
                .iter()
                .map(|byte| ctx.load_constant(F::from(u64::from(*byte)))),
        );
    }

    /// Witness-load the only self-referential protocol values and bind their
    /// exact canonical identity to the release-authenticated public words.
    ///
    /// `fixed_structure_sha256` is part of the outer circuit relation.  It is
    /// checked against the native compiled protocol before assignment and then
    /// loaded as constants.  The final VK may therefore be compiled only after
    /// key generation without ever becoming a constant of its own circuit.
    pub(super) fn load_and_constrain_parent_protocol<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        protocol: &PlonkProtocol<C>,
        parity: KagemushaPastaCycleParityV1,
        fixed_structure_sha256: [u8; 32],
        expected_words: &[AssignedValue<C::ScalarExt>],
    ) -> Result<LoadedParentProtocolV1<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if expected_words.len() != 8
            || protocol.preprocessed.is_empty()
            || protocol
                .preprocessed
                .iter()
                .any(|point| bool::from(point.is_identity()))
        {
            return Err(Error::InvalidInstances);
        }
        let actual_structure = kagemusha_compiled_protocol_structure_sha256(protocol, parity)
            .map_err(transcript_error)?;
        if actual_structure != fixed_structure_sha256 {
            return Err(transcript_error(
                "Kagemusha compiled parent protocol structure mismatch",
            ));
        }
        let transcript_initial_state = protocol.transcript_initial_state.ok_or_else(|| {
            transcript_error("Kagemusha compiled parent protocol has no transcript state")
        })?;
        let loaded = protocol.loaded_preprocessed_as_witness(loader, false);

        let chip = loader.ecc_chip();
        let mut ctx = loader.ctx_mut();
        let mut bytes = Vec::new();
        push_constant_bytes(
            ctx.main(),
            &mut bytes,
            KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1,
        );
        push_constant_bytes(ctx.main(), &mut bytes, &[0]);
        push_constant_bytes(
            ctx.main(),
            &mut bytes,
            &KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes(),
        );
        push_constant_bytes(
            ctx.main(),
            &mut bytes,
            &protocol_parity_tag(parity).to_le_bytes(),
        );
        push_constant_bytes(ctx.main(), &mut bytes, &fixed_structure_sha256);
        push_constant_bytes(
            ctx.main(),
            &mut bytes,
            &u32::try_from(loaded.preprocessed.len())
                .map_err(|_| transcript_error("Kagemusha preprocessed count does not fit u32"))?
                .to_le_bytes(),
        );
        for point in &loaded.preprocessed {
            bytes.extend(chip.assigned_point_bytes(&mut ctx, &point.assigned())?);
        }
        let loaded_transcript_state =
            loaded.transcript_initial_state.as_ref().ok_or_else(|| {
                transcript_error("loaded Kagemusha parent protocol has no transcript state")
            })?;
        bytes.extend(chip.assigned_scalar_bytes(&mut ctx, *loaded_transcript_state.assigned()));
        let digest = super::KagemushaSha256Chip::digest(ctx.main(), chip.range(), &bytes);
        for (assigned, expected) in digest.iter().zip(expected_words) {
            ctx.main().constrain_equal(assigned, expected);
        }
        drop(ctx);

        Ok(LoadedParentProtocolV1 {
            protocol: loaded,
            identity_witness: DeferredProtocolIdentityWitness {
                structure_sha256: fixed_structure_sha256,
                parity,
                preprocessed: protocol.preprocessed.clone(),
                transcript_initial_state,
            },
        })
    }

    fn load_native_accumulator<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        accumulator: &IpaAccumulator<C, NativeLoader>,
    ) -> DeferredAccumulator<'chip, C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        IpaAccumulator::new(
            accumulator
                .xi
                .iter()
                .map(|challenge| loader.assign_scalar(*challenge))
                .collect(),
            loader.assign_ec_point(accumulator.u),
        )
    }

    fn assigned_instance_cells_v4<C>(
        column: &[DeferredLoadedScalar<'_, C>],
        range: Range<usize>,
        expected_len: usize,
    ) -> Result<Vec<AssignedValue<C::ScalarExt>>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if expected_len == 0 || range.len() != expected_len || range.end > column.len() {
            return Err(Error::InvalidInstances);
        }
        Ok(column[range]
            .iter()
            .map(|scalar| *scalar.assigned())
            .collect())
    }

    fn constrain_equal_when<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        enabled: AssignedValue<F>,
        lhs: AssignedValue<F>,
        rhs: AssignedValue<F>,
    ) where
        F: BigPrimeField,
    {
        range.gate().assert_bit(ctx, enabled);
        let difference = range.gate().sub(ctx, Existing(lhs), Existing(rhs));
        let selected = range
            .gate()
            .mul(ctx, Existing(enabled), Existing(difference));
        range.gate().assert_is_const(ctx, &selected, &F::ZERO);
    }

    fn selector_and<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        lhs: AssignedValue<F>,
        rhs: AssignedValue<F>,
    ) -> AssignedValue<F>
    where
        F: BigPrimeField,
    {
        range.gate().assert_bit(ctx, lhs);
        range.gate().assert_bit(ctx, rhs);
        let output = range.gate().mul(ctx, Existing(lhs), Existing(rhs));
        range.gate().assert_bit(ctx, output);
        output
    }

    fn derive_parent_count_and_presence<C>(
        loader: &DeferredLoader<'_, C>,
        loaded_instances: &[Vec<DeferredLoadedScalar<'_, C>>],
    ) -> Result<(AssignedValue<C::ScalarExt>, AssignedValue<C::ScalarExt>), Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let parent_count = loaded_instances
            .first()
            .and_then(|column| column.get(KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4))
            .map(|value| *value.assigned())
            .ok_or(Error::InvalidInstances)?;
        let chip = loader.ecc_chip();
        let range = chip.range();
        let mut ctx = loader.ctx_mut();
        range.range_check(ctx.main(), parent_count, 2);
        let is_three =
            range
                .gate()
                .is_equal(ctx.main(), parent_count, Constant(C::ScalarExt::from(3)));
        range
            .gate()
            .assert_is_const(ctx.main(), &is_three, &C::ScalarExt::ZERO);
        let is_zero = range.gate().is_zero(ctx.main(), parent_count);
        let has_parent = range.gate().not(ctx.main(), is_zero);
        range.gate().assert_bit(ctx.main(), has_parent);
        Ok((parent_count, has_parent))
    }

    /// Derive the exact two public slot-presence bits from the current Step's
    /// constrained parent-count cell.
    pub(super) fn constrain_parent_slot_selectors_v4<C>(
        loader: &DeferredLoader<'_, C>,
        parent_count: AssignedValue<C::ScalarExt>,
    ) -> [AssignedValue<C::ScalarExt>; 2]
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let chip = loader.ecc_chip();
        let range = chip.range();
        let mut ctx = loader.ctx_mut();
        range.range_check(ctx.main(), parent_count, 2);
        let is_three =
            range
                .gate()
                .is_equal(ctx.main(), parent_count, Constant(C::ScalarExt::from(3)));
        range
            .gate()
            .assert_is_const(ctx.main(), &is_three, &C::ScalarExt::ZERO);
        let is_zero = range.gate().is_zero(ctx.main(), parent_count);
        let present_zero = range.gate().not(ctx.main(), is_zero);
        let present_one =
            range
                .gate()
                .is_equal(ctx.main(), parent_count, Constant(C::ScalarExt::from(2)));
        range.gate().assert_bit(ctx.main(), present_zero);
        range.gate().assert_bit(ctx.main(), present_one);
        [present_zero, present_one]
    }

    fn select_accumulator<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        when_true: &DeferredAccumulator<'chip, C>,
        when_false: &DeferredAccumulator<'chip, C>,
        selector: AssignedValue<C::ScalarExt>,
    ) -> Result<DeferredAccumulator<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if when_true.xi.len() != when_false.xi.len() {
            return Err(Error::AssertionFailure(
                "Kagemusha accumulator selector received different round counts".to_owned(),
            ));
        }
        let selected_xi = when_true
            .xi
            .iter()
            .zip(&when_false.xi)
            .map(|(when_true, when_false)| {
                let when_true = *when_true.assigned();
                let when_false = *when_false.assigned();
                let selected = {
                    let chip = loader.ecc_chip();
                    let range = chip.range();
                    range
                        .gate()
                        .select(loader.ctx_mut().main(), when_true, when_false, selector)
                };
                loader.scalar_from_assigned(selected)
            })
            .collect();
        let when_true = when_true.u.assigned().clone();
        let when_false = when_false.u.assigned().clone();
        let selected_u = {
            let chip = loader.ecc_chip();
            chip.select_point(&mut loader.ctx_mut(), &when_true, &when_false, selector)
        };
        Ok(IpaAccumulator::new(
            selected_xi,
            loader.ec_point_from_assigned(selected_u),
        ))
    }

    fn record_stage<C>(
        loader: &DeferredLoader<'_, C>,
        start: usize,
        gate: DeferredEquationGateV4,
        enabled: AssignedValue<C::ScalarExt>,
        stages: &mut Vec<AssignedDeferredEquationStageV4<C::ScalarExt>>,
    ) -> Result<(), Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        let end = loader.ecc_chip().equation_count();
        if start >= end {
            return Err(Error::AssertionFailure(
                "Kagemusha fixed deferred-equation stage is empty".to_owned(),
            ));
        }
        stages.push(AssignedDeferredEquationStageV4 {
            range: start..end,
            gate,
            enabled,
        });
        Ok(())
    }

    fn verify_ordinary_parent<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        protocol: &PlonkProtocol<C, DeferredLoader<'chip, C>>,
        instances: &[Vec<DeferredLoadedScalar<'chip, C>>],
        proof_bytes: &[u8],
        max_proof_bytes: usize,
    ) -> Result<DeferredAccumulator<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if max_proof_bytes == 0 || proof_bytes.is_empty() || proof_bytes.len() > max_proof_bytes {
            return Err(transcript_error(
                "Kagemusha parent proof violates the fixed proof slot",
            ));
        }
        let (reader, position) = ExactReader::new(proof_bytes);
        let mut transcript =
            DeferredTranscript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(loader, reader);
        let parsed = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::read_proof(
            succinct_vk,
            protocol,
            instances,
            &mut transcript,
        )?;
        let mut accumulators = PlonkSuccinctVerifier::<IpaAs<C, Bgh19>>::verify(
            succinct_vk,
            protocol,
            instances,
            &parsed,
        )?;
        if position.get() != proof_bytes.len() {
            return Err(transcript_error(
                "Kagemusha parent proof has trailing bytes",
            ));
        }
        if accumulators.len() != 1 {
            return Err(Error::AssertionFailure(
                "Kagemusha fixed parent verifier did not emit one IPA accumulator".to_owned(),
            ));
        }
        Ok(accumulators.remove(0))
    }

    fn verify_fold<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        inputs: &[DeferredAccumulator<'chip, C>],
        proof_bytes: &[u8],
        expected_proof_bytes: usize,
    ) -> Result<DeferredAccumulator<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if inputs.len() < 2
            || expected_proof_bytes == 0
            || proof_bytes.len() != expected_proof_bytes
        {
            return Err(transcript_error(
                "Kagemusha BGH19 fold has the wrong input or byte count",
            ));
        }
        let (reader, position) = ExactReader::new(proof_bytes);
        let mut transcript =
            DeferredTranscript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(loader, reader);
        let parsed =
            <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::read_proof(
                succinct_vk,
                inputs,
                &mut transcript,
            )?;
        let accumulated =
            <IpaAs<C, Bgh19> as AccumulationScheme<C, DeferredLoader<'chip, C>>>::verify(
                succinct_vk,
                inputs,
                &parsed,
            )?;
        if position.get() != proof_bytes.len() {
            return Err(transcript_error("Kagemusha BGH19 fold has trailing bytes"));
        }
        Ok(accumulated)
    }

    /// Verify one V4 parent slot with degree-derived accumulator and transcript
    /// lengths.  All three stages execute even when the public slot selector is
    /// zero; authenticated bootstrap material must therefore be fully parseable.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn constrain_parent_scalar_lineage_v4<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        protocol: &LoadedParentProtocolV1<'chip, C>,
        parent_slot: usize,
        slot_enabled: AssignedValue<C::ScalarExt>,
        authenticated_round_count: u32,
        max_parent_proof_bytes: usize,
        accumulator_instance_limbs: usize,
        witness: ParentScalarLineageWitnessV4<'_, C>,
    ) -> Result<ParentScalarLineageV4<'chip, C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if parent_slot >= 2 || max_parent_proof_bytes == 0 || accumulator_instance_limbs == 0 {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 parent slot/configuration is invalid".to_owned(),
            ));
        }
        witness
            .external_accumulation_proof
            .validate_fixed_transcript(authenticated_round_count)
            .map_err(|error| transcript_error(error))?;
        let expected_fold_bytes =
            kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)
                .map_err(|error| transcript_error(error))?;
        {
            let chip = loader.ecc_chip();
            chip.range()
                .gate()
                .assert_bit(loader.ctx_mut().main(), slot_enabled);
        }
        let loaded_instances = witness
            .instances
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| loader.assign_scalar(*value))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let parent_live_selector = loaded_instances
            .first()
            .and_then(|column| column.last())
            .ok_or(Error::InvalidInstances)?;
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&parent_live_selector.assigned(), &slot_enabled);
        let (_parent_count, has_carried_lineage) =
            derive_parent_count_and_presence(loader, &loaded_instances)?;

        let carried_column = loaded_instances
            .get(witness.carried_lineage_instance_column)
            .ok_or(Error::InvalidInstances)?;
        let expected_carried = assigned_instance_cells_v4(
            carried_column,
            witness.carried_lineage_instance_range,
            accumulator_instance_limbs,
        )?;
        for binding in witness.instance_copy_bindings {
            let column = loaded_instances
                .get(binding.column)
                .ok_or(Error::InvalidInstances)?;
            if binding.source.len() != binding.expected.len() || binding.source.end > column.len() {
                return Err(Error::InvalidInstances);
            }
            let parent_cells = column[binding.source.clone()]
                .iter()
                .map(|scalar| *scalar.assigned())
                .collect::<Vec<_>>();
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            for (parent, expected) in parent_cells.iter().zip(binding.expected) {
                constrain_equal_when(ctx.main(), range, slot_enabled, *parent, *expected);
            }
        }

        let carried = load_native_accumulator(loader, witness.carried_lineage);
        let carried_challenges = carried
            .xi
            .iter()
            .map(|challenge| *challenge.assigned())
            .collect::<Vec<_>>();
        let carried_point = carried.u.assigned().clone();
        let assigned_carried = {
            let chip = loader.ecc_chip();
            chip.assigned_accumulator_instance_limbs_v4(
                &mut loader.ctx_mut(),
                authenticated_round_count,
                &carried_challenges,
                &carried_point,
            )?
        };
        if assigned_carried.len() != accumulator_instance_limbs {
            return Err(Error::InvalidInstances);
        }
        {
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            let zero = ctx.main().load_zero();
            for (actual, expected) in assigned_carried.iter().zip(&expected_carried) {
                let selected = range
                    .gate()
                    .select(ctx.main(), *actual, zero, has_carried_lineage);
                constrain_equal_when(ctx.main(), range, slot_enabled, selected, *expected);
            }
        }

        let mut stages = Vec::with_capacity(3);
        let current_start = loader.ecc_chip().equation_count();
        let current = verify_ordinary_parent(
            loader,
            succinct_vk,
            &protocol.protocol,
            &loaded_instances,
            witness.proof_bytes,
            max_parent_proof_bytes,
        )?;
        if usize::try_from(authenticated_round_count).ok() != Some(current.xi.len()) {
            return Err(Error::AssertionFailure(
                "Kagemusha V4 ordinary proof emitted the wrong IPA round count".to_owned(),
            ));
        }
        record_stage(
            loader,
            current_start,
            DeferredEquationGateV4::ParentCurrent { slot: parent_slot },
            slot_enabled,
            &mut stages,
        )?;

        let fold_enabled = {
            let chip = loader.ecc_chip();
            selector_and(
                loader.ctx_mut().main(),
                chip.range(),
                slot_enabled,
                has_carried_lineage,
            )
        };
        let fold_start = loader.ecc_chip().equation_count();
        let folded = verify_fold(
            loader,
            succinct_vk,
            &[current.clone(), carried],
            &witness.external_accumulation_proof.bytes,
            expected_fold_bytes,
        )?;
        record_stage(
            loader,
            fold_start,
            DeferredEquationGateV4::ParentCarriedFold { slot: parent_slot },
            fold_enabled,
            &mut stages,
        )?;

        let select_start = loader.ecc_chip().equation_count();
        let accumulator = select_accumulator(loader, &folded, &current, has_carried_lineage)?;
        record_stage(
            loader,
            select_start,
            DeferredEquationGateV4::ParentLineageSelect { slot: parent_slot },
            slot_enabled,
            &mut stages,
        )?;
        Ok(ParentScalarLineageV4 {
            accumulator,
            stages,
        })
    }

    /// Degree-parameterized V4 branch fold and public-lineage selection.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn constrain_exposed_parent_lineage_v4<'chip, C>(
        loader: &DeferredLoader<'chip, C>,
        succinct_vk: &IpaSuccinctVerifyingKey<C>,
        authenticated_round_count: u32,
        accumulator_instance_limbs: usize,
        parent_zero: &DeferredAccumulator<'chip, C>,
        parent_one: &DeferredAccumulator<'chip, C>,
        slot_present: [AssignedValue<C::ScalarExt>; 2],
        branch_merge_proof: &KagemushaIpaAccumulationProofV4,
        exposed_instance_limbs: &[AssignedValue<C::ScalarExt>],
    ) -> Result<ExposedParentLineageV4<C>, Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        branch_merge_proof
            .validate_fixed_transcript(authenticated_round_count)
            .map_err(|error| transcript_error(error))?;
        let expected_fold_bytes =
            kagemusha_ipa_accumulation_proof_bytes_v4(authenticated_round_count)
                .map_err(|error| transcript_error(error))?;
        if accumulator_instance_limbs == 0
            || exposed_instance_limbs.len() != accumulator_instance_limbs
        {
            return Err(Error::InvalidInstances);
        }
        {
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            range.gate().assert_bit(ctx.main(), slot_present[0]);
            range.gate().assert_bit(ctx.main(), slot_present[1]);
            let absent_zero = range.gate().not(ctx.main(), slot_present[0]);
            let invalid_second =
                range
                    .gate()
                    .mul(ctx.main(), Existing(slot_present[1]), Existing(absent_zero));
            range
                .gate()
                .assert_is_const(ctx.main(), &invalid_second, &C::ScalarExt::ZERO);
        }

        let branch_start = loader.ecc_chip().equation_count();
        let branch = verify_fold(
            loader,
            succinct_vk,
            &[parent_zero.clone(), parent_one.clone()],
            &branch_merge_proof.bytes,
            expected_fold_bytes,
        )?;
        let mut stages = Vec::with_capacity(2);
        record_stage(
            loader,
            branch_start,
            DeferredEquationGateV4::BranchFold,
            slot_present[1],
            &mut stages,
        )?;

        let select_start = loader.ecc_chip().equation_count();
        let selected = select_accumulator(loader, &branch, parent_zero, slot_present[1])?;
        record_stage(
            loader,
            select_start,
            DeferredEquationGateV4::BranchSelect,
            slot_present[0],
            &mut stages,
        )?;

        let selected_challenges = selected
            .xi
            .iter()
            .map(|challenge| *challenge.assigned())
            .collect::<Vec<_>>();
        let selected_point = selected.u.assigned().clone();
        let selected_limbs = {
            let chip = loader.ecc_chip();
            chip.assigned_accumulator_instance_limbs_v4(
                &mut loader.ctx_mut(),
                authenticated_round_count,
                &selected_challenges,
                &selected_point,
            )?
        };
        if selected_limbs.len() != accumulator_instance_limbs {
            return Err(Error::InvalidInstances);
        }
        {
            let chip = loader.ecc_chip();
            let range = chip.range();
            let mut ctx = loader.ctx_mut();
            let zero = ctx.main().load_zero();
            for (actual, expected) in selected_limbs.iter().zip(exposed_instance_limbs) {
                let exposed = range
                    .gate()
                    .select(ctx.main(), *actual, zero, slot_present[0]);
                ctx.main().constrain_equal(&exposed, expected);
            }
        }
        Ok(ExposedParentLineageV4 { stages })
    }

    /// Hash the complete selector-bound V4 audit once and expose it through
    /// both independently presence-gated public join slots.
    ///
    /// Both public slots receive the same complete post-branch preimage.  For
    /// a one-parent step slot zero is present and therefore binds every
    /// enabled equation, including `BranchSelect`; slot one remains canonical
    /// zero.  A two-parent step exposes the same complete digest in both slots.
    pub(super) fn constrain_scalar_audit_identity_v4<C>(
        loader: &DeferredLoader<'_, C>,
        range: &halo2_base::gates::RangeChip<C::ScalarExt>,
        stages: &[AssignedDeferredEquationStageV4<C::ScalarExt>],
        slot_present: [AssignedValue<C::ScalarExt>; 2],
        expected_words: [&[AssignedValue<C::ScalarExt>]; 2],
    ) -> Result<[AssignedValue<C::ScalarExt>; 8], Error>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: BigPrimeField,
    {
        if expected_words.iter().any(|words| words.len() != 8) {
            return Err(Error::InvalidInstances);
        }
        let chip = loader.ecc_chip();
        let (gate_tags, selectors) = expand_stage_plan_v4(stages, chip.equation_count())?;
        let mut ctx = loader.ctx_mut();
        for selector in slot_present.iter().copied() {
            range.gate().assert_bit(ctx.main(), selector);
        }
        let bytes = chip.assigned_equation_bytes_v4(&mut ctx, &gate_tags, &selectors)?;
        let digest = super::KagemushaSha256Chip::digest(ctx.main(), range, &bytes);
        for (slot_present, expected_words) in slot_present.into_iter().zip(expected_words) {
            for (assigned, expected) in digest.iter().zip(expected_words) {
                let exposed =
                    range
                        .gate()
                        .mul(ctx.main(), Existing(slot_present), Existing(*assigned));
                ctx.main().constrain_equal(&exposed, expected);
            }
        }
        Ok(digest)
    }
}

/// One real-or-bootstrap fixed parent slot consumed by a V4 parity circuit.
#[derive(Clone)]
pub(crate) struct KagemushaStepParentProofV4<C>
where
    C: halo2_proofs::halo2curves::CurveAffine,
{
    /// Exact one-column parent instances.
    pub(crate) instances: Vec<Vec<C::ScalarExt>>,
    /// Ordinary augmented parent proof transcript.
    pub(crate) proof_bytes: Vec<u8>,
    /// Always-present non-identity carried accumulator.
    pub(crate) carried_lineage:
        snark_verifier::pcs::ipa::IpaAccumulator<C, snark_verifier::loader::native::NativeLoader>,
    /// Always-present post-proof fold transcript.
    pub(crate) external_accumulation_proof: KagemushaIpaAccumulationProofV4,
}

/// Complete fixed two-parent/three-fold recursive witness for one V4 parity.
pub(crate) struct KagemushaStepParityRecursionV4<C>
where
    C: halo2_base::utils::CurveAffineExt,
{
    /// Canonical IPA succinct key derived from the authenticated ParamsIPA.
    pub(crate) succinct_vk: snark_verifier::pcs::ipa::IpaSuccinctVerifyingKey<C>,
    /// Final compiled self protocol derived from authenticated ParamsIPA/VK.
    pub(crate) compiled_parent_protocol: PlonkProtocol<C>,
    /// Authenticated value-free self-protocol structure digest.
    pub(crate) fixed_structure_sha256: [u8; 32],
    /// Exactly two real-or-bootstrap ordinary proofs and post-proof folds.
    pub(crate) parents: [KagemushaStepParentProofV4<C>; 2],
    /// Per-step branch fold.  This is distinct from the all-bootstrap genesis
    /// artifact whenever either parent slot is real.
    pub(crate) branch_merge_fold: KagemushaIpaAccumulationProofV4,
}

/// Complete concrete witness needed to build both V4 Step parities.
pub(crate) struct KagemushaStepWitnessV4<'a> {
    /// Common field-neutral public boundary.
    pub(crate) public_inputs: &'a KagemushaPastaCyclePublicInputsV4,
    /// Exact logical recursive step counter.
    pub(crate) proof_step_count: u32,
    /// All fixed Eq-only secure relation openings.
    pub(crate) secure: &'a super::confidential_v2::KagemushaStepSecureWitnessV3,
    /// Eq-only output insertion/membership witness.
    pub(crate) output_membership: &'a super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
    /// Same-scalar Eq recursion witness.
    pub(crate) step_eq_recursion:
        &'a KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    /// Same-scalar Ep recursion witness.
    pub(crate) step_ep_recursion:
        &'a KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    /// Authenticated canonical Eq bootstrap payload; absence is an error.
    pub(crate) step_eq_bootstrap: Option<&'a KagemushaStepBootstrapV4>,
    /// Authenticated canonical Ep bootstrap payload; absence is an error.
    pub(crate) step_ep_bootstrap: Option<&'a KagemushaStepBootstrapV4>,
}

struct KagemushaScalarAuditOutputV4<C>
where
    C: halo2_base::utils::CurveAffineExt,
{
    identity: scalar_lineage_v1::DeferredProtocolIdentityWitness<C>,
    audit: super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: Vec<scalar_lineage_v1::DeferredEquationStageShapeV4>,
    inner_parent_counts: [u32; 2],
}

/// Serialize one scalar-verifier audit exactly as both constrained halves do
/// and derive its selector-gated public SHA-256 words.
fn kagemusha_deferred_audit_public_words_v4<C>(
    witness: &super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
    current_parent_count: u32,
    inner_parent_counts: [u32; 2],
) -> Result<[[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1], String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: PrimeField,
    C::ScalarExt: PrimeField,
{
    use super::kagemusha_cycle_loader::{
        KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4, KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4,
    };

    if current_parent_count > 2 || inner_parent_counts.into_iter().any(|count| count > 2) {
        return Err("Kagemusha V4 deferred-audit parent count is invalid".to_owned());
    }
    scalar_lineage_v1::validate_stage_shapes_v4(stages, witness.equations.len())
        .map_err(|error| format!("invalid Kagemusha V4 deferred-audit stage plan: {error:?}"))?;

    let slot_present = [current_parent_count >= 1, current_parent_count == 2];
    let parent_has_carried = inner_parent_counts.map(|count| count != 0);
    let mut gate_tags = vec![0_u32; witness.equations.len()];
    let mut selectors = vec![false; witness.equations.len()];
    for stage in stages {
        let enabled = match stage.gate {
            scalar_lineage_v1::DeferredEquationGateV4::ParentCurrent { slot }
            | scalar_lineage_v1::DeferredEquationGateV4::ParentLineageSelect { slot } => {
                slot_present[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::ParentCarriedFold { slot } => {
                slot_present[slot] && parent_has_carried[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::BranchFold => slot_present[1],
            scalar_lineage_v1::DeferredEquationGateV4::BranchSelect => slot_present[0],
        };
        for equation in stage.range.clone() {
            gate_tags[equation] = stage.gate.audit_tag();
            selectors[equation] = enabled;
        }
    }

    fn push_len(output: &mut Sha256, value: usize, label: &str) -> Result<(), String> {
        let value = u32::try_from(value)
            .map_err(|_| format!("Kagemusha V4 deferred-audit {label} does not fit u32"))?;
        output.update(value.to_le_bytes());
        Ok(())
    }

    fn push_field<F: PrimeField>(
        output: &mut Sha256,
        value: &F,
        label: &str,
    ) -> Result<(), String> {
        let repr = value.to_repr();
        if repr.as_ref().len() != 32 {
            return Err(format!(
                "Kagemusha V4 deferred-audit {label} is not a 32-byte Pasta scalar"
            ));
        }
        output.update(repr.as_ref());
        Ok(())
    }

    let mut digest = Sha256::new();
    digest.update(KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4);
    digest.update([0]);
    digest.update(KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4.to_le_bytes());
    push_len(&mut digest, witness.sources.len(), "source count")?;
    push_len(&mut digest, witness.equations.len(), "equation count")?;
    for source in &witness.sources {
        let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<C>> =
            source.coordinates().into();
        let coordinates = coordinates
            .ok_or_else(|| "Kagemusha V4 deferred-audit source is the identity point".to_owned())?;
        push_field(&mut digest, coordinates.x(), "source x-coordinate")?;
        push_field(&mut digest, coordinates.y(), "source y-coordinate")?;
    }
    for (index, equation) in witness.equations.iter().enumerate() {
        digest.update(gate_tags[index].to_le_bytes());
        digest.update([u8::from(selectors[index])]);
        push_len(&mut digest, equation.len(), "term count")?;
        for (source_index, coefficient) in equation {
            push_len(&mut digest, *source_index, "source index")?;
            push_field(&mut digest, coefficient, "coefficient")?;
        }
    }
    let public_words = kagemusha_sha256_public_words(digest.finalize().into());
    Ok(slot_present.map(|present| if present { public_words } else { [0; 8] }))
}

/// Execute the scalar-verifier witness pass with blank derived-audit words.
/// The resulting native audit is then serialized above and installed as
/// the public join before the proving pass builds both complete circuits.
fn collect_kagemusha_scalar_audits_v4<C>(
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    params: &KagemushaStepCircuitParamsV4,
    recursion: &KagemushaStepParityRecursionV4<C>,
    parity: KagemushaPastaCycleParityV1,
) -> Result<KagemushaScalarAuditOutputV4<C>, String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt:
        halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField + PrimeField + From<u64>,
{
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

    let layout = public_inputs.validate_for_audit_derivation_prepass(proof_step_count, params)?;
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(true)
        .use_params(kagemusha_base_circuit_params_v4(params)?);
    let values = public_inputs
        .instance_column_for_audit_derivation_prepass::<C::ScalarExt>(proof_step_count, params)?;
    let public_cells = builder.main(0).assign_witnesses(values);
    builder.assigned_instances = vec![public_cells.clone()];
    constrain_kagemusha_parity_scalar_v4(
        &mut builder,
        &public_cells,
        parity,
        params,
        &layout,
        recursion,
        false,
    )
}

fn scalar_field_parent_count_v4<F: ff::Field>(value: F) -> Result<u32, String> {
    if value == F::ZERO {
        Ok(0)
    } else if value == F::ONE {
        Ok(1)
    } else if value == F::ONE + F::ONE {
        Ok(2)
    } else {
        Err("Kagemusha parent proof exposes an invalid parent count".to_owned())
    }
}

fn parent_matches_bootstrap_v4<C>(
    parent: &KagemushaStepParentProofV4<C>,
    bootstrap: &KagemushaStepBootstrapParentSlotV4,
    expected_accumulator: &snark_verifier::pcs::ipa::IpaAccumulator<
        C,
        snark_verifier::loader::native::NativeLoader,
    >,
) -> bool
where
    C: halo2_proofs::halo2curves::CurveAffine,
    C::ScalarExt: From<u64> + PartialEq,
{
    parent.proof_bytes == bootstrap.ordinary_proof_bytes
        && parent.external_accumulation_proof == bootstrap.post_proof_fold
        && parent.carried_lineage.xi == expected_accumulator.xi
        && parent.carried_lineage.u == expected_accumulator.u
        && parent.instances.len() == bootstrap.instances.len()
        && parent
            .instances
            .iter()
            .zip(&bootstrap.instances)
            .all(|(actual, expected)| {
                actual.len() == expected.len()
                    && actual.iter().zip(expected).all(|(actual, expected)| {
                        *actual == C::ScalarExt::from(u64::from(*expected))
                    })
            })
}

fn validate_runtime_parity_v4<C>(
    recursion: &KagemushaStepParityRecursionV4<C>,
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
) -> Result<(), String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::ScalarExt: PrimeField + From<u64>,
{
    let expected_instances = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 instance length does not fit usize".to_owned())?;
    let max_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 proof bound does not fit usize".to_owned())?;
    let expected_rounds = usize::try_from(params.k)
        .map_err(|_| "Kagemusha V4 IPA degree does not fit usize".to_owned())?;
    let live_offset = usize::try_from(layout.live_selector_offset)
        .map_err(|_| "Kagemusha V4 live-selector offset does not fit usize".to_owned())?;
    recursion
        .branch_merge_fold
        .validate_fixed_transcript(params.k)?;
    for parent in &recursion.parents {
        if parent.instances.len() != 1
            || parent.instances[0].len() != expected_instances
            || parent.proof_bytes.is_empty()
            || parent.proof_bytes.len() > max_proof_bytes
            || parent.carried_lineage.xi.len() != expected_rounds
            || bool::from(parent.carried_lineage.u.is_identity())
            || !matches!(
                parent.instances[0].get(live_offset),
                Some(value) if *value == C::ScalarExt::ZERO || *value == C::ScalarExt::ONE
            )
        {
            return Err("Kagemusha V4 runtime parent shape mismatch".to_owned());
        }
        parent
            .external_accumulation_proof
            .validate_fixed_transcript(params.k)?;
        scalar_field_parent_count_v4(parent.instances[0][KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4])?;
    }
    Ok(())
}

fn require_kagemusha_step_bootstrap_v4<'a>(
    bootstrap: Option<&'a KagemushaStepBootstrapV4>,
    role: &str,
) -> Result<&'a KagemushaStepBootstrapV4, String> {
    bootstrap.ok_or_else(|| format!("Kagemusha V4 {role} bootstrap artifact is missing"))
}

fn validate_kagemusha_step_witness_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaPastaPublicLayoutV4, String> {
    let eq_layout = witness
        .public_inputs
        .validate(witness.proof_step_count, step_eq_params)?;
    let ep_layout = witness
        .public_inputs
        .validate(witness.proof_step_count, step_ep_params)?;
    if eq_layout != ep_layout || step_eq_params.k != step_ep_params.k {
        return Err("Kagemusha V4 Eq/Ep public layouts differ".to_owned());
    }
    let step_eq_bootstrap = require_kagemusha_step_bootstrap_v4(witness.step_eq_bootstrap, "Eq")?;
    let step_ep_bootstrap = require_kagemusha_step_bootstrap_v4(witness.step_ep_bootstrap, "Ep")?;
    step_eq_bootstrap.validate(
        step_eq_params,
        KagemushaPastaCycleParityV1::StepEq,
        witness.step_eq_recursion.fixed_structure_sha256,
    )?;
    step_ep_bootstrap.validate(
        step_ep_params,
        KagemushaPastaCycleParityV1::StepEp,
        witness.step_ep_recursion.fixed_structure_sha256,
    )?;
    validate_runtime_parity_v4(witness.step_eq_recursion, step_eq_params, &eq_layout)?;
    validate_runtime_parity_v4(witness.step_ep_recursion, step_ep_params, &ep_layout)?;

    let parent_count = usize::try_from(witness.public_inputs.parent_count)
        .map_err(|_| "Kagemusha V4 parent count does not fit usize".to_owned())?;
    let live_offset = usize::try_from(eq_layout.live_selector_offset)
        .map_err(|_| "Kagemusha V4 live-selector offset does not fit usize".to_owned())?;
    for slot in 0..parent_count {
        if witness.step_eq_recursion.parents[slot].instances[0][live_offset] != Fp::ONE
            || witness.step_ep_recursion.parents[slot].instances[0][live_offset] != Fq::ONE
        {
            return Err(format!(
                "Kagemusha V4 real parent slot {slot} is not a live proof"
            ));
        }
    }
    for slot in parent_count..KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
        let expected = step_eq_bootstrap
            .parent_slot
            .carried_lineage
            .to_eq(step_eq_params.k)?;
        if !parent_matches_bootstrap_v4(
            &witness.step_eq_recursion.parents[slot],
            &step_eq_bootstrap.parent_slot,
            &expected,
        ) {
            return Err(format!(
                "Kagemusha V4 Eq absent parent slot {slot} is not authenticated bootstrap"
            ));
        }
        let expected = step_ep_bootstrap
            .parent_slot
            .carried_lineage
            .to_ep(step_ep_params.k)?;
        if !parent_matches_bootstrap_v4(
            &witness.step_ep_recursion.parents[slot],
            &step_ep_bootstrap.parent_slot,
            &expected,
        ) {
            return Err(format!(
                "Kagemusha V4 Ep absent parent slot {slot} is not authenticated bootstrap"
            ));
        }
    }
    if parent_count < KAGEMUSHA_PASTA_PARENT_SLOTS_V1 {
        if witness.step_eq_recursion.branch_merge_fold != step_eq_bootstrap.branch_merge_fold
            || witness.step_ep_recursion.branch_merge_fold != step_ep_bootstrap.branch_merge_fold
        {
            return Err(
                "Kagemusha V4 disabled branch fold is not authenticated bootstrap".to_owned(),
            );
        }
    }
    Ok(eq_layout)
}

fn constrain_kagemusha_parity_scalar_v4<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::ScalarExt>,
    public_cells: &[halo2_base::AssignedValue<C::ScalarExt>],
    parity: KagemushaPastaCycleParityV1,
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
    recursion: &KagemushaStepParityRecursionV4<C>,
    bind_public_audits: bool,
) -> Result<KagemushaScalarAuditOutputV4<C>, String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
{
    use std::mem;

    use halo2_ecc::fields::fp::FpChip;
    use snark_verifier::loader::halo2::Halo2Loader;

    use super::kagemusha_cycle_loader::{DeferredScalarEccChip, LIMB_BITS, LIMBS};

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
    let accumulator_limbs = usize::try_from(layout.accumulator_limbs)
        .map_err(|_| "Kagemusha V4 accumulator length does not fit usize".to_owned())?;
    if public_cells.len() != public_len
        || recursion
            .parents
            .iter()
            .any(|parent| parent.instances.len() != 1 || parent.instances[0].len() != public_len)
    {
        return Err("Kagemusha V4 fixed parent-instance shape mismatch".to_owned());
    }
    let own_protocol_offset = match parity {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4,
    };
    let carried_offset = usize::try_from(match parity {
        KagemushaPastaCycleParityV1::StepEq => layout.parent_eq_accumulator_offset,
        KagemushaPastaCycleParityV1::StepEp => layout.parent_ep_accumulator_offset,
    })
    .map_err(|_| "Kagemusha V4 carried offset does not fit usize".to_owned())?;
    let deferred_offset = usize::try_from(match parity {
        KagemushaPastaCycleParityV1::StepEq => layout.parent_eq_deferred_offset,
        KagemushaPastaCycleParityV1::StepEp => layout.parent_ep_deferred_offset,
    })
    .map_err(|_| "Kagemusha V4 deferred offset does not fit usize".to_owned())?;
    let max_parent_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 proof bound does not fit usize".to_owned())?;

    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let chip = DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer);
    let loader = Halo2Loader::new(chip, mem::take(builder.pool(0)));
    let loaded_protocol = scalar_lineage_v1::load_and_constrain_parent_protocol(
        &loader,
        &recursion.compiled_parent_protocol,
        parity,
        recursion.fixed_structure_sha256,
        &public_cells[own_protocol_offset..own_protocol_offset + 8],
    )
    .map_err(|error| format!("failed to bind Kagemusha V4 parent protocol: {error:?}"))?;
    let parent_count = public_cells[KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4];
    let slot_present = scalar_lineage_v1::constrain_parent_slot_selectors_v4(&loader, parent_count);

    let mut lineages = Vec::with_capacity(2);
    let mut inner_parent_counts = [0_u32; 2];
    for slot in 0..2 {
        let parent = &recursion.parents[slot];
        inner_parent_counts[slot] = scalar_field_parent_count_v4(
            parent.instances[0][KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4],
        )?;
        let bindings = [
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4
                    ..KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4 + KAGEMUSHA_PASTA_STATE_STRIDE_V4,
                expected: &public_cells[kagemusha_pasta_parent_state_offset_v4(slot)
                    ..kagemusha_pasta_parent_state_offset_v4(slot)
                        + KAGEMUSHA_PASTA_STATE_STRIDE_V4],
            },
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4
                    ..KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + 8,
                expected: &public_cells[KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4
                    ..KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + 8],
            },
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4
                    ..KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4 + 16,
                expected: &public_cells[KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4
                    ..KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4 + 16],
            },
        ];
        let lineage = scalar_lineage_v1::constrain_parent_scalar_lineage_v4(
            &loader,
            &recursion.succinct_vk,
            &loaded_protocol,
            slot,
            slot_present[slot],
            params.k,
            max_parent_proof_bytes,
            accumulator_limbs,
            scalar_lineage_v1::ParentScalarLineageWitnessV4 {
                instances: &parent.instances,
                proof_bytes: &parent.proof_bytes,
                carried_lineage: &parent.carried_lineage,
                carried_lineage_instance_column: 0,
                carried_lineage_instance_range: carried_offset..carried_offset + accumulator_limbs,
                instance_copy_bindings: &bindings,
                external_accumulation_proof: &parent.external_accumulation_proof,
            },
        )
        .map_err(|error| {
            format!("failed to constrain Kagemusha V4 parent slot {slot}: {error:?}")
        })?;
        lineages.push(lineage);
    }

    let branch = scalar_lineage_v1::constrain_exposed_parent_lineage_v4(
        &loader,
        &recursion.succinct_vk,
        params.k,
        accumulator_limbs,
        &lineages[0].accumulator,
        &lineages[1].accumulator,
        slot_present,
        &recursion.branch_merge_fold,
        &public_cells[carried_offset..carried_offset + accumulator_limbs],
    )
    .map_err(|error| format!("failed to constrain Kagemusha V4 branch lineage: {error:?}"))?;
    let mut all_stages = lineages
        .iter()
        .flat_map(|lineage| lineage.stages.iter().cloned())
        .collect::<Vec<_>>();
    all_stages.extend(branch.stages.iter().cloned());
    let complete_audit = loader.ecc_chip().witness();
    let complete_shapes = all_stages
        .iter()
        .map(|stage| stage.shape())
        .collect::<Vec<_>>();
    if bind_public_audits {
        scalar_lineage_v1::constrain_scalar_audit_identity_v4(
            &loader,
            &range,
            &all_stages,
            slot_present,
            [
                &public_cells[deferred_offset..deferred_offset + 8],
                &public_cells[deferred_offset + 8..deferred_offset + 16],
            ],
        )
        .map_err(|error| format!("failed to bind Kagemusha V4 complete audits: {error:?}"))?;
    }
    let identity = loaded_protocol.identity_witness.clone();
    *builder.pool(0) = loader.take_ctx();

    Ok(KagemushaScalarAuditOutputV4 {
        identity,
        audit: complete_audit,
        stages: complete_shapes,
        inner_parent_counts,
    })
}

fn constrain_equal_if_v4<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    enabled: halo2_base::AssignedValue<F>,
    lhs: halo2_base::AssignedValue<F>,
    rhs: halo2_base::AssignedValue<F>,
) where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.gate().assert_bit(ctx, enabled);
    let difference = range.gate().sub(ctx, Existing(lhs), Existing(rhs));
    let selected = range
        .gate()
        .mul(ctx, Existing(enabled), Existing(difference));
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn constrain_kagemusha_output_frontier_v4<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    bindings: &super::kagemusha_step_transition::NamedTransitionBindings<F>,
    output: &[halo2_base::AssignedValue<F>;
         super::kagemusha_v2::KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4],
    topup_leaf_index: halo2_base::AssignedValue<F>,
) where
    F: halo2_base::utils::BigPrimeField,
{
    constrain_equal_if_v4(ctx, range, bindings.is_init, topup_leaf_index, output[7]);
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_append,
        output[7],
        bindings.input_next_zero_leaf_index,
    );
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_redemption,
        output[9],
        bindings.input_next_zero_leaf_index,
    );
    ctx.constrain_equal(&output[10], &bindings.output_next_zero_leaf_index);
}

fn constrain_kagemusha_common_transition<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    public_cells: &[halo2_base::AssignedValue<F>],
    expected_public_len: usize,
) -> Result<super::kagemusha_step_transition::NamedTransitionBindings<F>, String>
where
    F: halo2_base::utils::BigPrimeField,
{
    if public_cells.len() != expected_public_len {
        return Err("Kagemusha Step public column has the wrong length".to_owned());
    }
    let operation: &[halo2_base::AssignedValue<F>; KAGEMUSHA_STEP_OPERATION_LIMBS_V4] =
        public_cells[KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4
            ..KAGEMUSHA_PASTA_STEP_OPERATION_OFFSET_V4 + KAGEMUSHA_STEP_OPERATION_LIMBS_V4]
            .try_into()
            .expect("validated fixed operation range");
    let parent_states: [&[halo2_base::AssignedValue<F>]; 2] = std::array::from_fn(|slot| {
        &public_cells[kagemusha_pasta_parent_state_offset_v4(slot)
            ..kagemusha_pasta_parent_state_offset_v4(slot) + KAGEMUSHA_PASTA_STATE_STRIDE_V4]
    });
    let result_state = &public_cells[KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4
        ..KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4 + KAGEMUSHA_PASTA_STATE_STRIDE_V4];
    let bindings = super::kagemusha_step_transition::constrain_two_input_step_transition_v4(
        ctx,
        range,
        public_cells[KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4],
        parent_states,
        result_state,
        operation,
    )?;
    for (operation_limb, public_limb) in bindings.statement_digest_limbs.iter().zip(
        &public_cells[KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4
            ..KAGEMUSHA_PASTA_PUBLIC_STATEMENT_DIGEST_OFFSET_V4 + 8],
    ) {
        ctx.constrain_equal(operation_limb, public_limb);
    }
    for index in 0..8 {
        let operation_limb = bindings.operation.limbs
            [(super::kagemusha_v2::I_ARTIFACT_MANIFEST_SHA256 + index / 2) * 8 + index % 2];
        ctx.constrain_equal(
            &operation_limb,
            &public_cells[KAGEMUSHA_PASTA_MANIFEST_SHA256_OFFSET_V4 + index],
        );
    }
    Ok(bindings)
}

fn operation_u128_v4<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    operation: &super::kagemusha_step_transition::AssignedKagemushaStepOperationV4<F>,
    low_index: usize,
) -> halo2_base::AssignedValue<F>
where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Constant,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.gate().mul_add(
        ctx,
        operation.fields[low_index + 1],
        Constant(F::from_u128(1_u128 << 64)),
        operation.fields[low_index],
    )
}

fn constrain_kagemusha_eq_secure_relations_v4(
    ctx: &mut halo2_base::Context<Fp>,
    range: &halo2_base::gates::RangeChip<Fp>,
    bindings: &super::kagemusha_step_transition::NamedTransitionBindings<Fp>,
    secure: &super::confidential_v2::KagemushaStepSecureWitnessV3,
    membership: &super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
) -> Result<(), String> {
    use ff::Field as _;
    use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};

    use super::{confidential_v2, kagemusha_v2};

    const DEPTH: usize = confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2;
    let topup = confidential_v2::secure_relation_v3::assign_kagemusha_topup_shield_v3::<DEPTH>(
        ctx,
        range,
        Some(&secure.topup),
    )?;
    let transfer = confidential_v2::secure_relation_v3::assign_confidential_transfer_step_v4::<
        DEPTH,
    >(ctx, range, Some(&secure.transfer))?;
    let unshield =
        confidential_v2::secure_relation_v3::assign_confidential_unshield_change_step_v4::<DEPTH>(
            ctx,
            range,
            Some(&secure.unshield_change),
        )?;
    let output = kagemusha_v2::output_membership_v4::assign_kagemusha_output_membership_v4(
        ctx,
        range,
        Some(membership),
    )?;

    for (lhs, rhs) in [
        (output[0], bindings.is_init),
        (output[1], bindings.is_append),
        (output[2], bindings.is_redemption),
        (output[3], bindings.has_change),
        (output[4], bindings.input_root),
        (output[5], bindings.output_root),
        (output[6], bindings.recipient_commitment),
        (output[8], bindings.change_commitment),
    ] {
        ctx.constrain_equal(&lhs, &rhs);
    }
    // The secure membership relation exposes the exact output leaf positions.
    // Copy-bind those positions to the append-only public-state frontier so a
    // recursive proof cannot skip an empty leaf or seed an arbitrary frontier.
    constrain_kagemusha_output_frontier_v4(ctx, range, bindings, &output, topup[6]);

    let operation = &bindings.operation;
    let init_amount = operation_u128_v4(ctx, range, operation, kagemusha_v2::I_CURRENT_AMOUNT_LO);
    for (lhs, rhs) in [
        (topup[0], bindings.recipient_commitment),
        (
            topup[1],
            operation.fields[kagemusha_v2::I_CURRENT_NULLIFIER],
        ),
        (topup[2], bindings.input_root),
        (topup[3], bindings.output_root),
        (topup[4], init_amount),
        (topup[5], operation.fields[kagemusha_v2::I_ASSET_SCALE]),
        (topup[7], operation.fields[kagemusha_v2::I_ASSET_TAG]),
        (topup[8], operation.fields[kagemusha_v2::I_CHAIN_TAG]),
    ] {
        constrain_equal_if_v4(ctx, range, bindings.is_init, lhs, rhs);
    }
    super::kagemusha_step_transition::constrain_kagemusha_step_init_topup_tags_v4(
        ctx, range, bindings, topup[9], topup[10],
    );

    let input_amount = operation_u128_v4(ctx, range, operation, kagemusha_v2::I_INPUT_AMOUNT_LO);
    let recipient_amount =
        operation_u128_v4(ctx, range, operation, kagemusha_v2::I_RECIPIENT_AMOUNT_LO);
    let change_amount = operation_u128_v4(ctx, range, operation, kagemusha_v2::I_CHANGE_AMOUNT_LO);
    for (lhs, rhs) in [
        (transfer.input_amount, input_amount),
        (transfer.recipient_amount, recipient_amount),
        (transfer.change_amount, change_amount),
        (transfer.has_change, bindings.has_change),
    ] {
        constrain_equal_if_v4(ctx, range, bindings.is_append, lhs, rhs);
    }
    let one = ctx.load_constant(Fp::ONE);
    let append_second_input = range.gate().sub(
        ctx,
        operation.fields[kagemusha_v2::I_TRANSFER_INPUT_COUNT],
        one,
    );
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_append,
        transfer.has_second_input,
        append_second_input,
    );
    for (lhs, rhs) in transfer.public.into_iter().zip([
        bindings.input_commitments[0],
        bindings.input_commitments[1],
        bindings.input_nullifiers[0],
        bindings.input_nullifiers[1],
        bindings.recipient_commitment,
        bindings.change_commitment,
        bindings.input_root,
        operation.fields[kagemusha_v2::I_ASSET_TAG],
        operation.fields[kagemusha_v2::I_CHAIN_TAG],
    ]) {
        constrain_equal_if_v4(ctx, range, bindings.is_append, lhs, rhs);
    }

    let public_amount = operation.fields[kagemusha_v2::I_UNSHIELD_PUBLIC_AMOUNT];
    for (lhs, rhs) in [
        (unshield.input_amount, input_amount),
        (unshield.change_amount, change_amount),
    ] {
        constrain_equal_if_v4(ctx, range, bindings.is_redemption, lhs, rhs);
    }
    let zero = ctx.load_constant(Fp::ZERO);
    constrain_equal_if_v4(
        ctx,
        range,
        bindings.is_redemption,
        unshield.has_second_input,
        zero,
    );
    for (lhs, rhs) in unshield.public.into_iter().zip([
        bindings.input_commitments[0],
        bindings.input_commitments[1],
        bindings.input_nullifiers[0],
        bindings.input_nullifiers[1],
        bindings.change_commitment,
        bindings.input_root,
        public_amount,
        operation.fields[kagemusha_v2::I_ASSET_TAG],
        operation.fields[kagemusha_v2::I_CHAIN_TAG],
    ]) {
        constrain_equal_if_v4(ctx, range, bindings.is_redemption, lhs, rhs);
    }
    Ok(())
}

fn constrain_kagemusha_reciprocal_output_v4<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::Base>,
    public_cells: &[halo2_base::AssignedValue<C::Base>],
    layout: &KagemushaPastaPublicLayoutV4,
    output: &KagemushaScalarAuditOutputV4<C>,
) -> Result<(), String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use std::mem;

    use halo2_ecc::fields::fp::FpChip;

    use super::kagemusha_cycle_loader::{LIMB_BITS, LIMBS};

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 reciprocal public length does not fit usize".to_owned())?;
    if public_cells.len() != public_len {
        return Err("Kagemusha V4 reciprocal public column has the wrong length".to_owned());
    }
    let (protocol_offset, deferred_offset) = match output.identity.parity {
        KagemushaPastaCycleParityV1::StepEq => (
            KAGEMUSHA_PASTA_STEP_EQ_PROTOCOL_SHA256_OFFSET_V4,
            usize::try_from(layout.parent_eq_deferred_offset)
                .map_err(|_| "Kagemusha V4 Eq audit offset does not fit usize".to_owned())?,
        ),
        KagemushaPastaCycleParityV1::StepEp => (
            KAGEMUSHA_PASTA_STEP_EP_PROTOCOL_SHA256_OFFSET_V4,
            usize::try_from(layout.parent_ep_deferred_offset)
                .map_err(|_| "Kagemusha V4 Ep audit offset does not fit usize".to_owned())?,
        ),
    };
    let range = builder.range_chip();
    let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let mut ctx = mem::take(builder.pool(0));
    let parent_public_parent_counts = output
        .inner_parent_counts
        .map(|count| ctx.main().load_witness(C::Base::from(u64::from(count))));
    constrain_reciprocal_point_audit_identity_v4::<C>(
        &mut ctx,
        &base,
        &scalar,
        &output.audit,
        &output.stages,
        public_cells[KAGEMUSHA_PASTA_PARENT_COUNT_OFFSET_V4],
        parent_public_parent_counts,
        [
            &public_cells[deferred_offset..deferred_offset + 8],
            &public_cells[deferred_offset + 8..deferred_offset + 16],
        ],
    )?;
    constrain_reciprocal_protocol_identity::<C>(
        &mut ctx,
        &base,
        &scalar,
        &output.identity,
        output.identity.structure_sha256,
        &public_cells[protocol_offset..protocol_offset + 8],
    )?;
    *builder.pool(0) = ctx;
    Ok(())
}

fn kagemusha_builder_without_witnesses_v4<F>(
    builder: &halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>
where
    F: halo2_base::utils::ScalarField,
{
    builder.deep_clone().unknown(true)
}

#[derive(Clone)]
pub(crate) struct KagemushaStepEqCircuitV4 {
    params: KagemushaStepCircuitParamsV4,
    builder: halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp>,
}

impl halo2_proofs::plonk::Circuit<Fp> for KagemushaStepEqCircuitV4 {
    type Config = halo2_base::gates::circuit::BaseConfig<Fp>;
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
    type Params = KagemushaStepCircuitParamsV4;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            params: self.params.clone(),
            builder: kagemusha_builder_without_witnesses_v4(&self.builder),
        }
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fp>,
        params: Self::Params,
    ) -> Self::Config {
        let base = kagemusha_base_circuit_params_v4(&params)
            .expect("authenticated Kagemusha StepEq V4 circuit parameters");
        let usable_rows = kagemusha_usable_rows_v4(&params)
            .expect("authenticated Kagemusha StepEq V4 unusable-row bound");
        let mut config = halo2_base::gates::circuit::BaseConfig::configure(meta, base);
        config.set_usable_rows(usable_rows);
        config
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fp>) -> Self::Config {
        unreachable!("Kagemusha StepEq V4 requires authenticated circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl halo2_proofs::circuit::Layouter<Fp>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> as halo2_proofs::plonk::Circuit<
            Fp,
        >>::synthesize(&self.builder, config, layouter)
    }
}

/// Production StepEp circuit type with explicit authenticated V4 parameters.
#[derive(Clone)]
pub(crate) struct KagemushaStepEpCircuitV4 {
    params: KagemushaStepCircuitParamsV4,
    builder: halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq>,
}

impl halo2_proofs::plonk::Circuit<Fq> for KagemushaStepEpCircuitV4 {
    type Config = halo2_base::gates::circuit::BaseConfig<Fq>;
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;
    type Params = KagemushaStepCircuitParamsV4;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            params: self.params.clone(),
            builder: kagemusha_builder_without_witnesses_v4(&self.builder),
        }
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fq>,
        params: Self::Params,
    ) -> Self::Config {
        let base = kagemusha_base_circuit_params_v4(&params)
            .expect("authenticated Kagemusha StepEp V4 circuit parameters");
        let usable_rows = kagemusha_usable_rows_v4(&params)
            .expect("authenticated Kagemusha StepEp V4 unusable-row bound");
        let mut config = halo2_base::gates::circuit::BaseConfig::configure(meta, base);
        config.set_usable_rows(usable_rows);
        config
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fq>) -> Self::Config {
        unreachable!("Kagemusha StepEp V4 requires authenticated circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        layouter: impl halo2_proofs::circuit::Layouter<Fq>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq> as halo2_proofs::plonk::Circuit<
            Fq,
        >>::synthesize(&self.builder, config, layouter)
    }
}

/// Complete concrete V4 Eq/Ep circuit pair.
pub(crate) struct KagemushaStepCircuitsV4 {
    /// Eq/Vesta proof circuit over `Fp`.
    pub(crate) step_eq: KagemushaStepEqCircuitV4,
    /// Ep/Pallas proof circuit over `Fq`.
    pub(crate) step_ep: KagemushaStepEpCircuitV4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KagemushaStepPublicModeV4 {
    Live,
    Bootstrap,
}

/// Assign the exposed V4 column and gate its complete semantic interpretation
/// behind the appended live selector.
///
/// Both modes assign and constrain the same two columns of advice values. In
/// live mode every semantic limb is copy-equivalent to its exposed limb. In
/// bootstrap mode every exposed limb (including the selector) is constrained
/// to zero, while the same fixed-shape semantic relation is populated with the
/// adapter's private calibration witness. Consequently a bootstrap proof has
/// no public spend/state meaning and cannot be replayed as a live proof.
fn assign_kagemusha_public_mode_v4<F>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    semantic_values: Vec<F>,
    layout: &KagemushaPastaPublicLayoutV4,
    mode: KagemushaStepPublicModeV4,
) -> Result<Vec<halo2_base::AssignedValue<F>>, String>
where
    F: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
    let live_offset = usize::try_from(layout.live_selector_offset)
        .map_err(|_| "Kagemusha V4 live-selector offset does not fit usize".to_owned())?;
    if semantic_values.len() != public_len || live_offset >= public_len {
        return Err("Kagemusha V4 semantic public column has the wrong length".to_owned());
    }
    let exposed_values = match mode {
        KagemushaStepPublicModeV4::Live => semantic_values.clone(),
        KagemushaStepPublicModeV4::Bootstrap => vec![F::ZERO; public_len],
    };
    let exposed = builder.main(0).assign_witnesses(exposed_values);
    let semantic = builder.main(0).assign_witnesses(semantic_values);
    builder.assigned_instances = vec![exposed.clone()];

    let range = builder.range_chip();
    let ctx = builder.main(0);
    let live = exposed[live_offset];
    range.gate().assert_bit(ctx, live);
    range
        .gate()
        .assert_is_const(ctx, &semantic[live_offset], &F::ONE);
    let not_live = range.gate().not(ctx, live);
    for (exposed, semantic) in exposed.iter().zip(&semantic) {
        let bootstrap_value = range
            .gate()
            .mul(ctx, Existing(not_live), Existing(*exposed));
        range
            .gate()
            .assert_is_const(ctx, &bootstrap_value, &F::ZERO);
        constrain_equal_if_v4(ctx, &range, live, *exposed, *semantic);
    }
    Ok(semantic)
}

/// Build the complete degree-parameterized StepEq and StepEp pair.
///
/// This constructor requires both canonical bootstrap artifacts even for a
/// two-real-parent step. It rejects stale public layouts, missing padding,
/// undersized k, and reuse of the all-bootstrap branch transcript with a real
/// parent before creating either circuit.
pub(crate) fn build_kagemusha_step_circuits_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: KagemushaStepCircuitParamsV4,
    step_ep_params: KagemushaStepCircuitParamsV4,
    step_eq_break_points: &[Vec<usize>],
    step_ep_break_points: &[Vec<usize>],
) -> Result<KagemushaStepCircuitsV4, String> {
    build_kagemusha_step_circuits_with_mode_v4(
        witness,
        step_eq_params,
        step_ep_params,
        KagemushaStepPublicModeV4::Live,
        KagemushaStepBuilderStageV4 {
            step_eq_break_points,
            step_ep_break_points,
        },
    )
}

#[derive(Clone, Copy)]
struct KagemushaStepBuilderStageV4<'a> {
    step_eq_break_points: &'a [Vec<usize>],
    step_ep_break_points: &'a [Vec<usize>],
}

fn kagemusha_step_builder_v4<F>(
    params: &KagemushaStepCircuitParamsV4,
    break_points: Option<&[Vec<usize>]>,
) -> Result<halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>, String>
where
    F: halo2_base::utils::ScalarField,
{
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

    let base_params = kagemusha_base_circuit_params_v4(params)?;
    Ok(match break_points {
        Some(break_points) => BaseCircuitBuilder::prover(base_params, break_points.to_vec()),
        None => BaseCircuitBuilder::new(false).use_params(base_params),
    })
}

fn build_kagemusha_step_eq_half_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
    mode: KagemushaStepPublicModeV4,
    break_points: Option<&[Vec<usize>]>,
) -> Result<
    (
        halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp>,
        Vec<halo2_base::AssignedValue<Fp>>,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    ),
    String,
> {
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
    let mut builder = kagemusha_step_builder_v4::<Fp>(params, break_points)?;
    let values = witness
        .public_inputs
        .instance_column::<Fp>(witness.proof_step_count, params)?;
    let public = assign_kagemusha_public_mode_v4(&mut builder, values, layout, mode)?;
    let range = builder.range_chip();
    let bindings =
        constrain_kagemusha_common_transition(builder.main(0), &range, &public, public_len)?;
    constrain_kagemusha_eq_secure_relations_v4(
        builder.main(0),
        &range,
        &bindings,
        witness.secure,
        witness.output_membership,
    )?;
    let output = constrain_kagemusha_parity_scalar_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &mut builder,
        &public,
        KagemushaPastaCycleParityV1::StepEq,
        params,
        layout,
        witness.step_eq_recursion,
        true,
    )?;
    Ok((builder, public, output))
}

fn build_kagemusha_step_ep_half_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
    mode: KagemushaStepPublicModeV4,
    break_points: Option<&[Vec<usize>]>,
) -> Result<
    (
        halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq>,
        Vec<halo2_base::AssignedValue<Fq>>,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
> {
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 public length does not fit usize".to_owned())?;
    let mut builder = kagemusha_step_builder_v4::<Fq>(params, break_points)?;
    let values = witness
        .public_inputs
        .instance_column::<Fq>(witness.proof_step_count, params)?;
    let public = assign_kagemusha_public_mode_v4(&mut builder, values, layout, mode)?;
    let range = builder.range_chip();
    constrain_kagemusha_common_transition(builder.main(0), &range, &public, public_len)?;
    let output = constrain_kagemusha_parity_scalar_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &mut builder,
        &public,
        KagemushaPastaCycleParityV1::StepEp,
        params,
        layout,
        witness.step_ep_recursion,
        true,
    )?;
    Ok((builder, public, output))
}

fn build_kagemusha_step_circuits_with_mode_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: KagemushaStepCircuitParamsV4,
    step_ep_params: KagemushaStepCircuitParamsV4,
    mode: KagemushaStepPublicModeV4,
    stage: KagemushaStepBuilderStageV4<'_>,
) -> Result<KagemushaStepCircuitsV4, String> {
    let layout = validate_kagemusha_step_witness_v4(witness, &step_eq_params, &step_ep_params)?;

    let step_eq_break_points = Some(stage.step_eq_break_points);
    let (mut step_eq, eq_public, eq_output) = build_kagemusha_step_eq_half_v4(
        witness,
        &step_eq_params,
        &layout,
        mode,
        step_eq_break_points,
    )?;

    let step_ep_break_points = Some(stage.step_ep_break_points);
    let (mut step_ep, ep_public, ep_output) = build_kagemusha_step_ep_half_v4(
        witness,
        &step_ep_params,
        &layout,
        mode,
        step_ep_break_points,
    )?;
    constrain_kagemusha_reciprocal_output_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &mut step_eq,
        &eq_public,
        &layout,
        &ep_output,
    )?;
    constrain_kagemusha_reciprocal_output_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &mut step_ep,
        &ep_public,
        &layout,
        &eq_output,
    )?;

    validate_kagemusha_builder_layout_v4(
        &step_eq,
        &step_eq_params,
        stage.step_eq_break_points,
        true,
        "StepEq prover",
    )?;
    validate_kagemusha_builder_layout_v4(
        &step_ep,
        &step_ep_params,
        stage.step_ep_break_points,
        true,
        "StepEp prover",
    )?;

    Ok(KagemushaStepCircuitsV4 {
        step_eq: KagemushaStepEqCircuitV4 {
            params: step_eq_params,
            builder: step_eq,
        },
        step_ep: KagemushaStepEpCircuitV4 {
            params: step_ep_params,
            builder: step_ep,
        },
    })
}

fn build_kagemusha_step_eq_circuit_sequential_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: KagemushaStepCircuitParamsV4,
    step_ep_params: &KagemushaStepCircuitParamsV4,
    mode: KagemushaStepPublicModeV4,
    break_points: Option<&[Vec<usize>]>,
    reciprocal_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
) -> Result<
    (
        KagemushaStepEqCircuitV4,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    ),
    String,
> {
    let layout = validate_kagemusha_step_witness_v4(witness, &step_eq_params, step_ep_params)?;
    let (mut builder, public, output) =
        build_kagemusha_step_eq_half_v4(witness, &step_eq_params, &layout, mode, break_points)?;
    constrain_kagemusha_reciprocal_output_v4::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &mut builder,
        &public,
        &layout,
        reciprocal_output,
    )?;
    if let Some(break_points) = break_points {
        validate_kagemusha_builder_layout_v4(
            &builder,
            &step_eq_params,
            break_points,
            true,
            "StepEq sequential prover",
        )?;
    }
    Ok((
        KagemushaStepEqCircuitV4 {
            params: step_eq_params,
            builder,
        },
        output,
    ))
}

fn build_kagemusha_step_ep_circuit_sequential_v4(
    witness: &KagemushaStepWitnessV4<'_>,
    step_eq_params: &KagemushaStepCircuitParamsV4,
    step_ep_params: KagemushaStepCircuitParamsV4,
    mode: KagemushaStepPublicModeV4,
    break_points: Option<&[Vec<usize>]>,
    reciprocal_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
) -> Result<
    (
        KagemushaStepEpCircuitV4,
        KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
> {
    let layout = validate_kagemusha_step_witness_v4(witness, step_eq_params, &step_ep_params)?;
    let (mut builder, public, output) =
        build_kagemusha_step_ep_half_v4(witness, &step_ep_params, &layout, mode, break_points)?;
    constrain_kagemusha_reciprocal_output_v4::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &mut builder,
        &public,
        &layout,
        reciprocal_output,
    )?;
    if let Some(break_points) = break_points {
        validate_kagemusha_builder_layout_v4(
            &builder,
            &step_ep_params,
            break_points,
            true,
            "StepEp sequential prover",
        )?;
    }
    Ok((
        KagemushaStepEpCircuitV4 {
            params: step_ep_params,
            builder,
        },
        output,
    ))
}

fn create_augmented_eq_proof_v4<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: &halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit: C,
    instances: &[Vec<Fp>],
) -> Result<Vec<u8>, String>
where
    C: halo2_proofs::plonk::Circuit<Fp>,
{
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EqAffine},
        plonk::{create_proof, verify_proof},
        poly::ipa::commitment::IPACommitmentScheme,
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    if instances.is_empty() || instances.iter().any(Vec::is_empty) {
        return Err("Kagemusha V4 Eq proof instances are empty".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let columns = instances.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let proofs_instances: [&[&[Fp]]; 1] = [columns.as_slice()];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceProverIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Eq proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
    >(
        params,
        proving_key.get_vk(),
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Eq generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    Ok(proof)
}

fn create_augmented_ep_proof_v4<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: &halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit: C,
    instances: &[Vec<Fq>],
) -> Result<Vec<u8>, String>
where
    C: halo2_proofs::plonk::Circuit<Fq>,
{
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EpAffine},
        plonk::{create_proof, verify_proof},
        poly::ipa::commitment::IPACommitmentScheme,
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    if instances.is_empty() || instances.iter().any(Vec::is_empty) {
        return Err("Kagemusha V4 Ep proof instances are empty".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let columns = instances.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let proofs_instances: [&[&[Fq]]; 1] = [columns.as_slice()];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceProverIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Ep proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
    >(
        params,
        proving_key.get_vk(),
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Ep generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    Ok(proof)
}

/// Raw, manifest-independent payloads emitted by the V4 artifact generator for
/// one Pasta parity.  The framing/export layer owns release identity and file
/// publication; this type contains only material derived by the circuit/key
/// generation process itself.
pub struct KagemushaGeneratedParityArtifactsV4 {
    /// Calibrated, inline circuit profile used to create every other payload.
    pub circuit_params: KagemushaStepCircuitParamsV4,
    /// Value-free compiled-protocol structure digest shared by bootstrap and
    /// final self protocols.
    pub compiled_protocol_structure_sha256: [u8; 32],
    /// Exact augmented Step-proof size measured during generation.
    pub step_proof_size_bytes: u32,
    /// Canonical `ParamsIPA::write` bytes.
    pub parameters: Vec<u8>,
    /// Processed proving-key bytes whose embedded VK is checked below.
    pub proving_key: Vec<u8>,
    /// Processed verifier-key bytes.
    pub verifying_key: Vec<u8>,
    /// Canonical Norito bootstrap payload containing a genuine selector-zero
    /// proof under `verifying_key`.
    pub bootstrap_witness: Vec<u8>,
}

/// Complete raw Eq/Ep output of one V4 generation run.
pub struct KagemushaGeneratedPastaCycleArtifactsV4 {
    /// StepEq/Vesta material.
    pub step_eq: KagemushaGeneratedParityArtifactsV4,
    /// StepEp/Pallas material.
    pub step_ep: KagemushaGeneratedParityArtifactsV4,
    /// Canonical live selector-one pair used solely to measure the opaque ABI
    /// payload.  It is terminally verified before being returned.
    pub measured_live_pair_bytes: Vec<u8>,
}

struct KagemushaGenerationCalibrationV4 {
    public_inputs: KagemushaPastaCyclePublicInputsV4,
    secure: super::confidential_v2::KagemushaStepSecureWitnessV3,
    output_membership: super::kagemusha_v2::KagemushaOutputMembershipWitnessV4,
}

fn kagemusha_calibration_exact_limbs_v4(bytes: [u8; 32]) -> [u32; 8] {
    std::array::from_fn(|index| {
        u32::from_le_bytes(
            bytes[index * 4..index * 4 + 4]
                .try_into()
                .expect("32-byte calibration value has exact limbs"),
        )
    })
}

fn kagemusha_calibration_scalar_v4(bytes: [u8; 32], role: &str) -> Result<Fp, String> {
    Option::<Fp>::from(Fp::from_repr(bytes.into()))
        .ok_or_else(|| format!("Kagemusha V4 calibration {role} is not canonical Fp"))
}

fn kagemusha_calibration_put_digest_v4(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    start: usize,
    bytes: [u8; 32],
) -> Result<(), String> {
    let target = fields
        .get_mut(start..start + 4)
        .ok_or_else(|| "Kagemusha V4 calibration digest range is invalid".to_owned())?;
    for (field, chunk) in target.iter_mut().zip(bytes.chunks_exact(8)) {
        *field = Fp::from(u64::from_le_bytes(
            chunk
                .try_into()
                .expect("32-byte calibration digest has exact chunks"),
        ));
    }
    Ok(())
}

fn kagemusha_calibration_put_field_v4(
    fields: &mut [Fp; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4],
    index: usize,
    bytes: [u8; 32],
    role: &str,
) -> Result<(), String> {
    *fields
        .get_mut(index)
        .ok_or_else(|| format!("Kagemusha V4 calibration {role} index is invalid"))? =
        kagemusha_calibration_scalar_v4(bytes, role)?;
    Ok(())
}

fn kagemusha_calibration_membership_path_v4(
    path: super::confidential_v2::ConfidentialMerklePathV2,
) -> iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
    let (siblings, directions, _, root) = path.into_parts();
    iroha_data_model::offline::KagemushaConfidentialMerklePathV2 {
        siblings,
        directions,
        root,
    }
}

/// Build one deterministic, satisfying initialization relation for key
/// calibration and the measured live pair.  None of these values is an
/// authenticated release identity: the exporter supplies that layer after the
/// generated payloads and sizes are known.
fn kagemusha_generation_calibration_v4(
    step_eq_compiled_protocol_sha256: [u8; 32],
    step_ep_compiled_protocol_sha256: [u8; 32],
) -> Result<KagemushaGenerationCalibrationV4, String> {
    use halo2_proofs::halo2curves::pasta::Fp;
    use iroha_data_model::ChainId;

    use super::{confidential_v2, kagemusha_v2};

    const ASSET_DEFINITION: &str = "kagemusha-fixed-padding#internal";
    const CHAIN: &str = "kagemusha-fixed-padding-chain";
    const PAYER: &str = "kagemusha-fixed-padding-payer";
    const AMOUNT: u128 = 1;
    const ASSET_SCALE: u32 = 0;
    const LEAF_INDEX: u32 = 0;

    let chain_id = ChainId::from(CHAIN);
    let spend_key = [0x46_u8; 32];
    let rho = [0x47_u8; 32];
    let operation_id = [0x48_u8; 32];
    let diversifier = {
        let repr = Fp::from(4).to_repr();
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(repr.as_ref());
        bytes
    };

    let empty_path = confidential_v2::compute_confidential_merkle_path_v3(&[], 0)?;
    let secure = confidential_v2::prepare_kagemusha_step_topup_witness_v3(
        &chain_id,
        ASSET_DEFINITION,
        PAYER,
        operation_id,
        AMOUNT,
        ASSET_SCALE,
        &spend_key,
        rho,
        diversifier,
        LEAF_INDEX,
        &empty_path,
    )?;

    let asset_tag = confidential_v2::derive_confidential_asset_tag_v3(ASSET_DEFINITION)?;
    let chain_tag = confidential_v2::derive_confidential_chain_tag_v3(CHAIN)?;
    let payer_tag = confidential_v2::derive_kagemusha_topup_payer_tag_v3(PAYER)?;
    let operation_tag = confidential_v2::derive_kagemusha_topup_operation_tag_v3(&operation_id)?;
    let owner_tag = confidential_v2::derive_confidential_owner_tag_v3_with_diversifier(
        &spend_key,
        diversifier,
    )?;
    let output_commitment =
        confidential_v2::derive_confidential_note_v3(asset_tag, AMOUNT, rho, owner_tag)?;
    let spend_nullifier =
        confidential_v2::derive_confidential_nullifier_v3(&spend_key, rho, asset_tag, chain_tag)?;
    let initial_root = confidential_v2::compute_confidential_root_v3(&[])?;
    let final_commitments = [output_commitment];
    let final_root = confidential_v2::compute_confidential_root_v3(&final_commitments)?;
    if empty_path.root != initial_root {
        return Err("Kagemusha V4 calibration empty path/root mismatch".to_owned());
    }

    let recipient_update_path = kagemusha_calibration_membership_path_v4(empty_path.clone());
    let recipient_membership_path = kagemusha_calibration_membership_path_v4(
        confidential_v2::compute_confidential_merkle_path_v3(&final_commitments, 0)?,
    );
    let dummy_leaf_index = 1_u32;
    let dummy_path = kagemusha_calibration_membership_path_v4(
        confidential_v2::compute_confidential_merkle_path_v3(
            &final_commitments,
            usize::try_from(dummy_leaf_index)
                .map_err(|_| "Kagemusha V4 calibration dummy index does not fit usize")?,
        )?,
    );
    let output_membership = kagemusha_v2::KagemushaOutputMembershipWitnessV4 {
        operation: kagemusha_v2::KagemushaOutputMembershipOperationV4::Init,
        initial_root,
        final_root,
        recipient: Some(kagemusha_v2::KagemushaOutputMembershipLeafV4 {
            commitment: output_commitment,
            leaf_index: LEAF_INDEX,
            update_path: recipient_update_path,
            membership_path: recipient_membership_path,
        }),
        change: None,
        dummy_leaf_index,
        dummy_path,
    };
    kagemusha_v2::KagemushaOutputMembershipCircuitV4::new(output_membership.clone())?;

    let statement_digest = [0x11_u8; 32];
    let topup_anchor_digest = [0x31_u8; 32];
    let manifest_sha256 = [0x41_u8; 32];
    let verifier_key_id_digest = [0x51_u8; 32];
    let mut fields = [Fp::ZERO; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4];
    fields[kagemusha_v2::I_LAYOUT_VERSION] = Fp::ONE;
    fields[kagemusha_v2::I_PROOF_STEP_COUNT] = Fp::ONE;
    fields[kagemusha_v2::I_ASSET_SCALE] = Fp::from(u64::from(ASSET_SCALE));
    for index in [
        kagemusha_v2::I_INPUT_SCALE,
        kagemusha_v2::I_TRANSFER_SCALE,
        kagemusha_v2::I_RECIPIENT_SCALE,
        kagemusha_v2::I_CURRENT_SCALE,
    ] {
        fields[index] = Fp::from(u64::from(ASSET_SCALE));
    }
    fields[kagemusha_v2::I_RECORD_OUTPUT_COUNT] = Fp::ONE;
    fields[kagemusha_v2::I_TRANSFER_OUTPUT_COUNT] = Fp::ONE;
    for index in [
        kagemusha_v2::I_CURRENT_AMOUNT_LO,
        kagemusha_v2::I_INPUT_AMOUNT_LO,
        kagemusha_v2::I_TRANSFER_AMOUNT_LO,
        kagemusha_v2::I_RECIPIENT_AMOUNT_LO,
    ] {
        fields[index] = Fp::from_u128(AMOUNT);
    }
    for (index, bytes, role) in [
        (kagemusha_v2::I_INITIAL_ROOT, initial_root, "initial root"),
        (kagemusha_v2::I_FINAL_ROOT, final_root, "final root"),
        (
            kagemusha_v2::I_RECORD_ROOT_BEFORE,
            initial_root,
            "record root before",
        ),
        (
            kagemusha_v2::I_RECORD_ROOT_AFTER,
            final_root,
            "record root after",
        ),
        (kagemusha_v2::I_TRANSFER_ROOT, final_root, "transfer root"),
        (
            kagemusha_v2::I_CURRENT_COMMITMENT,
            output_commitment,
            "current commitment",
        ),
        (
            kagemusha_v2::I_CURRENT_NULLIFIER,
            spend_nullifier,
            "current nullifier",
        ),
        (
            kagemusha_v2::I_RECIPIENT_COMMITMENT,
            output_commitment,
            "recipient commitment",
        ),
        (
            kagemusha_v2::I_RECIPIENT_NULLIFIER,
            spend_nullifier,
            "recipient nullifier",
        ),
        (
            kagemusha_v2::I_RECORD_OUTPUT_0,
            output_commitment,
            "record output",
        ),
        (
            kagemusha_v2::I_TRANSFER_OUTPUT_0,
            output_commitment,
            "transfer output",
        ),
        (kagemusha_v2::I_ASSET_TAG, asset_tag, "asset tag"),
        (kagemusha_v2::I_CHAIN_TAG, chain_tag, "chain tag"),
    ] {
        kagemusha_calibration_put_field_v4(&mut fields, index, bytes, role)?;
    }
    for (index, bytes) in [
        (kagemusha_v2::I_STATEMENT_DIGEST, statement_digest),
        (kagemusha_v2::I_RECIPIENT_REQUEST_DIGEST, payer_tag),
        (kagemusha_v2::I_OPERATION_ID, operation_tag),
        (kagemusha_v2::I_BRANCH_LINEAGE_ROOT, topup_anchor_digest),
        (kagemusha_v2::I_TOPUP_OPERATION_ID, operation_id),
        (kagemusha_v2::I_ARTIFACT_MANIFEST_SHA256, manifest_sha256),
        (kagemusha_v2::I_TOPUP_RECEIPT_DIGEST, topup_anchor_digest),
        (kagemusha_v2::I_TOPUP_ANCHOR_DIGEST, topup_anchor_digest),
        (
            kagemusha_v2::I_VERIFIER_KEY_ID_DIGEST,
            verifier_key_id_digest,
        ),
    ] {
        kagemusha_calibration_put_digest_v4(&mut fields, index, bytes)?;
    }
    fields[kagemusha_v2::I_TOPUP_ANCHOR_COUNT] = Fp::ONE;
    let operation = KagemushaStepOperationVectorV4::from_fields(fields);

    let mut result_state =
        vec![0_u32; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
    result_state[kagemusha_v2::S_VERSION] =
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
    result_state[kagemusha_v2::S_CHAIN_TAG..kagemusha_v2::S_CHAIN_TAG + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(chain_tag));
    result_state[kagemusha_v2::S_ASSET_TAG..kagemusha_v2::S_ASSET_TAG + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(asset_tag));
    result_state[kagemusha_v2::S_ASSET_SCALE] = ASSET_SCALE;
    result_state[kagemusha_v2::S_FINAL_ROOT..kagemusha_v2::S_FINAL_ROOT + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(final_root));
    result_state[kagemusha_v2::S_TOPUP_ANCHOR_COUNT] = 1;
    result_state[kagemusha_v2::S_TOPUP_ANCHORS..kagemusha_v2::S_TOPUP_ANCHORS + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(operation_id));
    result_state[kagemusha_v2::S_TOPUP_ANCHORS + 8..kagemusha_v2::S_TOPUP_ANCHORS + 16]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(topup_anchor_digest));
    result_state[kagemusha_v2::S_PROOF_STEP_COUNT] = 1;
    result_state[kagemusha_v2::S_CURRENT_COMMITMENT..kagemusha_v2::S_CURRENT_COMMITMENT + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(output_commitment));
    result_state[kagemusha_v2::S_CURRENT_NULLIFIER..kagemusha_v2::S_CURRENT_NULLIFIER + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(spend_nullifier));
    for (target, limb) in result_state
        [kagemusha_v2::S_CURRENT_AMOUNT..kagemusha_v2::S_CURRENT_AMOUNT + 4]
        .iter_mut()
        .zip(AMOUNT.to_le_bytes().chunks_exact(4))
    {
        *target = u32::from_le_bytes(
            limb.try_into()
                .expect("u128 calibration amount has exact limbs"),
        );
    }
    result_state[kagemusha_v2::S_CURRENT_SCALE] = ASSET_SCALE;
    result_state[kagemusha_v2::S_BRANCH_CLAIM_COUNT] = 1;
    result_state[kagemusha_v2::S_BRANCH_CLAIMS..kagemusha_v2::S_BRANCH_CLAIMS + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(topup_anchor_digest));
    result_state
        [kagemusha_v2::S_ARTIFACT_MANIFEST_SHA256..kagemusha_v2::S_ARTIFACT_MANIFEST_SHA256 + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(manifest_sha256));
    result_state[kagemusha_v2::S_VERIFIER_KEY_ID..kagemusha_v2::S_VERIFIER_KEY_ID + 8]
        .copy_from_slice(&kagemusha_calibration_exact_limbs_v4(
            verifier_key_id_digest,
        ));

    let public_inputs = KagemushaPastaCyclePublicInputsV4 {
        public_statement_digest: kagemusha_calibration_exact_limbs_v4(statement_digest),
        operation,
        parent_count: 0,
        parent_states: std::array::from_fn(|_| {
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2]
        }),
        result_state,
        manifest_sha256: kagemusha_calibration_exact_limbs_v4(manifest_sha256),
        step_eq_compiled_protocol_sha256: kagemusha_sha256_public_words(
            step_eq_compiled_protocol_sha256,
        ),
        step_ep_compiled_protocol_sha256: kagemusha_sha256_public_words(
            step_ep_compiled_protocol_sha256,
        ),
        parent_eq_lineage_accumulator: None,
        parent_ep_lineage_accumulator: None,
        parent_eq_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        parent_ep_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
        live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
    };

    Ok(KagemushaGenerationCalibrationV4 {
        public_inputs,
        secure,
        output_membership,
    })
}

struct KagemushaEqBootstrapSeedV4 {
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    structure_sha256: [u8; 32],
    protocol_sha256: [u8; 32],
    proof: Vec<u8>,
    current: snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EqAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
}

struct KagemushaEpBootstrapSeedV4 {
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    structure_sha256: [u8; 32],
    protocol_sha256: [u8; 32],
    proof: Vec<u8>,
    current: snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EpAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
}

fn kagemusha_eq_bootstrap_seed_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaEqBootstrapSeedV4, String> {
    use halo2_proofs::plonk::keygen_pk;

    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 Eq bootstrap public length does not fit usize".to_owned())?;
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(circuit_params)?,
        instance_column_lengths: vec![public_len],
    };
    let circuit = KagemushaProtocolBootstrapCircuit::<Fp> {
        params: target.base_circuit_params.clone(),
        marker: std::marker::PhantomData,
    };
    let verifying_key = kagemusha_bootstrap_verifying_key_v1(params, &target)?;
    let proving_key = keygen_pk(params, verifying_key.clone(), &circuit)
        .map_err(|error| format!("failed to generate Kagemusha V4 Eq bootstrap PK: {error}"))?;
    let instances = vec![vec![Fp::ZERO; public_len]];
    let proof = create_augmented_eq_proof_v4(params, &proving_key, circuit, &instances)?;
    let current =
        succinct_verify_step_eq_instances(params, &verifying_key, &proof, &instances, proof.len())?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    let structure_sha256 = kagemusha_compiled_protocol_structure_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    Ok(KagemushaEqBootstrapSeedV4 {
        protocol,
        structure_sha256,
        protocol_sha256,
        proof,
        current,
    })
}

fn kagemusha_ep_bootstrap_seed_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaEpBootstrapSeedV4, String> {
    use halo2_proofs::plonk::keygen_pk;

    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 Ep bootstrap public length does not fit usize".to_owned())?;
    let target = KagemushaUniversalProtocolTargetV1 {
        base_circuit_params: kagemusha_base_circuit_params_v4(circuit_params)?,
        instance_column_lengths: vec![public_len],
    };
    let circuit = KagemushaProtocolBootstrapCircuit::<Fq> {
        params: target.base_circuit_params.clone(),
        marker: std::marker::PhantomData,
    };
    let verifying_key = kagemusha_bootstrap_verifying_key_v1(params, &target)?;
    let proving_key = keygen_pk(params, verifying_key.clone(), &circuit)
        .map_err(|error| format!("failed to generate Kagemusha V4 Ep bootstrap PK: {error}"))?;
    let instances = vec![vec![Fq::ZERO; public_len]];
    let proof = create_augmented_ep_proof_v4(params, &proving_key, circuit, &instances)?;
    let current =
        succinct_verify_step_ep_instances(params, &verifying_key, &proof, &instances, proof.len())?;
    let protocol = snark_verifier::system::halo2::compile(
        params,
        &verifying_key,
        kagemusha_ipa_compile_config_v4(public_len),
    );
    let structure_sha256 = kagemusha_compiled_protocol_structure_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    Ok(KagemushaEpBootstrapSeedV4 {
        protocol,
        structure_sha256,
        protocol_sha256,
        proof,
        current,
    })
}

fn kagemusha_eq_seed_bootstrap_payload_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    seed: &KagemushaEqBootstrapSeedV4,
) -> Result<KagemushaStepBootstrapV4, String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    if seed.proof.len()
        != usize::try_from(circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 Eq calibrated proof size changed".to_owned());
    }
    let (post_proof_fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let (branch_merge_fold, _) = super::kagemusha_accumulation::fold_eq_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let bootstrap = KagemushaStepBootstrapV4 {
        version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        circuit_params_sha256: circuit_params
            .sha256()
            .map_err(|error| format!("failed to identify Kagemusha V4 Eq params: {error}"))?,
        compiled_protocol_structure_sha256: seed.structure_sha256,
        bootstrap_compiled_protocol_sha256: seed.protocol_sha256,
        parent_slot: KagemushaStepBootstrapParentSlotV4 {
            instances: vec![vec![
                0;
                usize::try_from(layout.instance_column_limbs).map_err(
                    |_| { "Kagemusha V4 Eq bootstrap public length does not fit usize".to_owned() }
                )?
            ]],
            ordinary_proof_bytes: seed.proof.clone(),
            carried_lineage: KagemushaIpaAccumulatorWireV4::from_eq(
                &seed.current,
                circuit_params.k,
            )?,
            post_proof_fold,
        },
        branch_merge_fold,
    };
    bootstrap.validate_bootstrap_protocol(
        circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        seed.structure_sha256,
        &seed.protocol,
    )?;
    Ok(bootstrap)
}

fn kagemusha_ep_seed_bootstrap_payload_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    seed: &KagemushaEpBootstrapSeedV4,
) -> Result<KagemushaStepBootstrapV4, String> {
    let layout = validate_kagemusha_circuit_params_v4(circuit_params)?;
    if seed.proof.len()
        != usize::try_from(circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Ep proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 Ep calibrated proof size changed".to_owned());
    }
    let (post_proof_fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let (branch_merge_fold, _) = super::kagemusha_accumulation::fold_ep_accumulators_v4(
        params,
        circuit_params.k,
        seed.current.clone(),
        Some(seed.current.clone()),
    )?;
    let bootstrap = KagemushaStepBootstrapV4 {
        version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        circuit_params_sha256: circuit_params
            .sha256()
            .map_err(|error| format!("failed to identify Kagemusha V4 Ep params: {error}"))?,
        compiled_protocol_structure_sha256: seed.structure_sha256,
        bootstrap_compiled_protocol_sha256: seed.protocol_sha256,
        parent_slot: KagemushaStepBootstrapParentSlotV4 {
            instances: vec![vec![
                0;
                usize::try_from(layout.instance_column_limbs).map_err(
                    |_| { "Kagemusha V4 Ep bootstrap public length does not fit usize".to_owned() }
                )?
            ]],
            ordinary_proof_bytes: seed.proof.clone(),
            carried_lineage: KagemushaIpaAccumulatorWireV4::from_ep(
                &seed.current,
                circuit_params.k,
            )?,
            post_proof_fold,
        },
        branch_merge_fold,
    };
    bootstrap.validate_bootstrap_protocol(
        circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        seed.structure_sha256,
        &seed.protocol,
    )?;
    Ok(bootstrap)
}

fn kagemusha_eq_recursion_from_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EqAffine>,
    structure_sha256: [u8; 32],
    bootstrap: &KagemushaStepBootstrapV4,
) -> Result<KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EqAffine>, String> {
    Ok(KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_eq_succinct_vk_v4(params)?,
        compiled_parent_protocol: protocol,
        fixed_structure_sha256: structure_sha256,
        parents: [
            bootstrap.step_eq_parent(circuit_params, structure_sha256, 0)?,
            bootstrap.step_eq_parent(circuit_params, structure_sha256, 1)?,
        ],
        branch_merge_fold: bootstrap.branch_merge_fold.clone(),
    })
}

fn kagemusha_ep_recursion_from_bootstrap_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    circuit_params: &KagemushaStepCircuitParamsV4,
    protocol: PlonkProtocol<halo2_proofs::halo2curves::pasta::EpAffine>,
    structure_sha256: [u8; 32],
    bootstrap: &KagemushaStepBootstrapV4,
) -> Result<KagemushaStepParityRecursionV4<halo2_proofs::halo2curves::pasta::EpAffine>, String> {
    Ok(KagemushaStepParityRecursionV4 {
        succinct_vk: kagemusha_ep_succinct_vk_v4(params)?,
        compiled_parent_protocol: protocol,
        fixed_structure_sha256: structure_sha256,
        parents: [
            bootstrap.step_ep_parent(circuit_params, structure_sha256, 0)?,
            bootstrap.step_ep_parent(circuit_params, structure_sha256, 1)?,
        ],
        branch_merge_fold: bootstrap.branch_merge_fold.clone(),
    })
}

fn kagemusha_eq_parameters_bytes_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::poly::commitment::Params as _;

    let mut bytes = Vec::new();
    params
        .write(&mut bytes)
        .map_err(|error| format!("failed to encode Kagemusha V4 Eq parameters: {error}"))?;
    Ok(bytes)
}

fn kagemusha_ep_parameters_bytes_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::poly::commitment::Params as _;

    let mut bytes = Vec::new();
    params
        .write(&mut bytes)
        .map_err(|error| format!("failed to encode Kagemusha V4 Ep parameters: {error}"))?;
    Ok(bytes)
}

struct KagemushaGeneratedKeyBytesV4 {
    parameters: Vec<u8>,
    verifying_key: Vec<u8>,
    proving_key: Vec<u8>,
}

fn frame_kagemusha_consumed_proving_key_v4(
    header: &KagemushaPastaCycleProvingKeyHeaderV4,
    expected_parity: KagemushaPastaCycleParityV1,
    circuit_params: &KagemushaStepCircuitParamsV4,
    mut processed_proving_key: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if processed_proving_key.is_empty() {
        return Err("Kagemusha V4 processed proving key is empty".to_owned());
    }

    // Ask the canonical encoder for the validated prefix, using one sentinel
    // byte as the required non-empty key suffix.  Moving that small prefix
    // into the consumed Halo2 buffer avoids retaining separate full raw and
    // framed proving-key vectors.
    let mut prefix =
        super::kagemusha_artifact_v4::encode_kagemusha_pasta_cycle_proving_key_payload_v4(
            header,
            expected_parity,
            circuit_params,
            &[0],
        )?;
    if prefix.pop() != Some(0) {
        return Err("Kagemusha V4 proving-key prefix sentinel mismatch".to_owned());
    }
    let prefix_len = prefix.len();
    let processed_len = processed_proving_key.len();
    let payload_len = prefix_len
        .checked_add(processed_len)
        .ok_or_else(|| "Kagemusha V4 proving-key payload length overflow".to_owned())?;
    if u64::try_from(payload_len).ok().is_none_or(|len| {
        len > iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4
    }) {
        return Err("Kagemusha V4 proving-key payload exceeds its bound".to_owned());
    }
    processed_proving_key
        .try_reserve_exact(prefix_len)
        .map_err(|_| "failed to reserve Kagemusha V4 proving-key payload".to_owned())?;
    processed_proving_key.resize(payload_len, 0);
    processed_proving_key.copy_within(0..processed_len, prefix_len);
    processed_proving_key[..prefix_len].copy_from_slice(&prefix);
    Ok(processed_proving_key)
}

fn finalize_kagemusha_eq_generated_keys_v4(
    params: halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    break_points: &[Vec<usize>],
) -> Result<KagemushaGeneratedKeyBytesV4, String> {
    use halo2_proofs::{SerdeFormat, halo2curves::pasta::EqAffine};

    let parameters = kagemusha_eq_parameters_bytes_v4(&params)?;
    let verifying_key = proving_key.get_vk().to_bytes(SerdeFormat::Processed);
    let header = KagemushaPastaCycleProvingKeyHeaderV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
        parity: KagemushaPastaCycleParityV1::StepEq,
        circuit_params_sha256: circuit_params
            .sha256()
            .map_err(|error| format!("failed to hash Kagemusha V4 Eq parameters: {error}"))?,
        break_points: kagemusha_break_points_to_wire_v4(break_points)?,
    };
    let proving_key_bytes = frame_kagemusha_consumed_proving_key_v4(
        &header,
        KagemushaPastaCycleParityV1::StepEq,
        circuit_params,
        proving_key.into_bytes(SerdeFormat::Processed),
    )?;
    drop(params);

    {
        use halo2_proofs::poly::commitment::Params as _;

        let parsed =
            parse_kagemusha_params_v4::<EqAffine>(&parameters, circuit_params.k, "generated Eq")?;
        let mut comparison = KagemushaExactBytesWriterV4::new(&parameters);
        parsed
            .write(&mut comparison)
            .map_err(|error| format!("failed to compare Kagemusha V4 Eq parameters: {error}"))?;
        comparison.finish("Eq parameters")?;
    }
    {
        let parsed = parse_kagemusha_eq_vk_v4(&verifying_key, circuit_params.clone())?;
        let mut comparison = KagemushaExactBytesWriterV4::new(&verifying_key);
        parsed
            .write(&mut comparison, SerdeFormat::Processed)
            .map_err(|error| format!("failed to compare Kagemusha V4 Eq verifier key: {error}"))?;
        comparison.finish("Eq verifier key")?;
    }
    {
        let processed =
            super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &proving_key_bytes,
                KagemushaPastaCycleParityV1::StepEq,
                circuit_params,
            )?;
        let (parsed, parsed_break_points) =
            parse_kagemusha_eq_pk_v4(&proving_key_bytes, circuit_params.clone())?;
        if parsed_break_points != break_points {
            return Err("Kagemusha V4 generated Eq breakpoint round-trip mismatch".to_owned());
        }
        let mut comparison = KagemushaExactBytesWriterV4::new(processed.processed_proving_key());
        parsed
            .write(&mut comparison, SerdeFormat::Processed)
            .map_err(|error| format!("failed to compare Kagemusha V4 Eq proving key: {error}"))?;
        comparison.finish("Eq proving key")?;
        let mut comparison = KagemushaExactBytesWriterV4::new(&verifying_key);
        parsed
            .get_vk()
            .write(&mut comparison, SerdeFormat::Processed)
            .map_err(|error| {
                format!("failed to compare Kagemusha V4 Eq embedded verifier key: {error}")
            })?;
        comparison.finish("Eq embedded verifier key")?;
    }
    Ok(KagemushaGeneratedKeyBytesV4 {
        parameters,
        verifying_key,
        proving_key: proving_key_bytes,
    })
}

fn finalize_kagemusha_ep_generated_keys_v4(
    params: halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit_params: &KagemushaStepCircuitParamsV4,
    break_points: &[Vec<usize>],
) -> Result<KagemushaGeneratedKeyBytesV4, String> {
    use halo2_proofs::{SerdeFormat, halo2curves::pasta::EpAffine};

    let parameters = kagemusha_ep_parameters_bytes_v4(&params)?;
    let verifying_key = proving_key.get_vk().to_bytes(SerdeFormat::Processed);
    let header = KagemushaPastaCycleProvingKeyHeaderV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
        parity: KagemushaPastaCycleParityV1::StepEp,
        circuit_params_sha256: circuit_params
            .sha256()
            .map_err(|error| format!("failed to hash Kagemusha V4 Ep parameters: {error}"))?,
        break_points: kagemusha_break_points_to_wire_v4(break_points)?,
    };
    let proving_key_bytes = frame_kagemusha_consumed_proving_key_v4(
        &header,
        KagemushaPastaCycleParityV1::StepEp,
        circuit_params,
        proving_key.into_bytes(SerdeFormat::Processed),
    )?;
    drop(params);

    {
        use halo2_proofs::poly::commitment::Params as _;

        let parsed =
            parse_kagemusha_params_v4::<EpAffine>(&parameters, circuit_params.k, "generated Ep")?;
        let mut comparison = KagemushaExactBytesWriterV4::new(&parameters);
        parsed
            .write(&mut comparison)
            .map_err(|error| format!("failed to compare Kagemusha V4 Ep parameters: {error}"))?;
        comparison.finish("Ep parameters")?;
    }
    {
        let parsed = parse_kagemusha_ep_vk_v4(&verifying_key, circuit_params.clone())?;
        let mut comparison = KagemushaExactBytesWriterV4::new(&verifying_key);
        parsed
            .write(&mut comparison, SerdeFormat::Processed)
            .map_err(|error| format!("failed to compare Kagemusha V4 Ep verifier key: {error}"))?;
        comparison.finish("Ep verifier key")?;
    }
    {
        let processed =
            super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &proving_key_bytes,
                KagemushaPastaCycleParityV1::StepEp,
                circuit_params,
            )?;
        let (parsed, parsed_break_points) =
            parse_kagemusha_ep_pk_v4(&proving_key_bytes, circuit_params.clone())?;
        if parsed_break_points != break_points {
            return Err("Kagemusha V4 generated Ep breakpoint round-trip mismatch".to_owned());
        }
        let mut comparison = KagemushaExactBytesWriterV4::new(processed.processed_proving_key());
        parsed
            .write(&mut comparison, SerdeFormat::Processed)
            .map_err(|error| format!("failed to compare Kagemusha V4 Ep proving key: {error}"))?;
        comparison.finish("Ep proving key")?;
        let mut comparison = KagemushaExactBytesWriterV4::new(&verifying_key);
        parsed
            .get_vk()
            .write(&mut comparison, SerdeFormat::Processed)
            .map_err(|error| {
                format!("failed to compare Kagemusha V4 Ep embedded verifier key: {error}")
            })?;
        comparison.finish("Ep embedded verifier key")?;
    }
    Ok(KagemushaGeneratedKeyBytesV4 {
        parameters,
        verifying_key,
        proving_key: proving_key_bytes,
    })
}

/// Generate the complete Eq/Ep V4 artifact payload set from current source.
///
/// This is deliberately a two-stage fixed-point construction. A deterministic
/// universal BaseConfig proof supplies parseable disabled-parent transcripts
/// while the final self-recursive PK/VK are generated. The final PK then
/// creates a genuine selector-zero proof over the all-zero public column; its
/// current accumulator and both independent folds become the authenticated
/// bootstrap payload. Finally a selector-one initialization is proved and
/// terminally decided to measure the public opaque pair.
pub fn generate_kagemusha_pasta_cycle_artifacts_v4(
    mut step_eq_circuit_params: KagemushaStepCircuitParamsV4,
    mut step_ep_circuit_params: KagemushaStepCircuitParamsV4,
) -> Result<KagemushaGeneratedPastaCycleArtifactsV4, String> {
    use halo2_proofs::{
        halo2curves::pasta::{EpAffine, EqAffine},
        plonk::{keygen_pk, keygen_vk},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };

    // Full release generation is deliberately narrower than artifact decoding.
    // The degree floor alone does not prove that a virtual trace fits: an
    // uncalibrated one-column profile previously consumed the host before its
    // late Halo2 layout failure. Keep this admission before ParamsIPA or any
    // witness/circuit allocation.
    let eq_layout = step_eq_circuit_params
        .validate_release_generation_profile()
        .map_err(|error| format!("Kagemusha V4 Eq release profile is not reviewed: {error}"))?;
    let ep_layout = step_ep_circuit_params
        .validate_release_generation_profile()
        .map_err(|error| format!("Kagemusha V4 Ep release profile is not reviewed: {error}"))?;
    if eq_layout != ep_layout || step_eq_circuit_params.k != step_ep_circuit_params.k {
        return Err("Kagemusha V4 generator Eq/Ep profile mismatch".to_owned());
    }
    let public_len = usize::try_from(eq_layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V4 generator public length does not fit usize".to_owned())?;

    // `ParamsIPA::new` is a transparent, public-coin derivation: the vendored
    // Halo2 implementation hashes the public domain `Halo2-Parameters` and
    // indexed messages directly to curve points (with `[1]`/`[2]` for w/u).
    // It accepts no RNG or secret seed, so reproducibility exposes no known
    // discrete-log relation or toxic setup material.
    let step_eq_params = ParamsIPA::<EqAffine>::new(step_eq_circuit_params.k);
    let step_ep_params = ParamsIPA::<EpAffine>::new(step_ep_circuit_params.k);
    let step_eq_seed = kagemusha_eq_bootstrap_seed_v4(&step_eq_params, &step_eq_circuit_params)?;
    let step_ep_seed = kagemusha_ep_bootstrap_seed_v4(&step_ep_params, &step_ep_circuit_params)?;
    step_eq_circuit_params.max_parent_proof_bytes = u32::try_from(step_eq_seed.proof.len())
        .map_err(|_| "Kagemusha V4 Eq proof size does not fit u32".to_owned())?;
    step_ep_circuit_params.max_parent_proof_bytes = u32::try_from(step_ep_seed.proof.len())
        .map_err(|_| "Kagemusha V4 Ep proof size does not fit u32".to_owned())?;
    validate_kagemusha_circuit_params_v4(&step_eq_circuit_params)?;
    validate_kagemusha_circuit_params_v4(&step_ep_circuit_params)?;

    let step_eq_seed_bootstrap = kagemusha_eq_seed_bootstrap_payload_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        &step_eq_seed,
    )?;
    let step_ep_seed_bootstrap = kagemusha_ep_seed_bootstrap_payload_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        &step_ep_seed,
    )?;

    let keygen_calibration = kagemusha_generation_calibration_v4(
        step_eq_seed.protocol_sha256,
        step_ep_seed.protocol_sha256,
    )?;
    let step_eq_seed_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_seed.protocol.clone(),
        step_eq_seed.structure_sha256,
        &step_eq_seed_bootstrap,
    )?;
    let step_ep_seed_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_seed.protocol.clone(),
        step_ep_seed.structure_sha256,
        &step_ep_seed_bootstrap,
    )?;
    let keygen_witness = KagemushaStepWitnessV4 {
        public_inputs: &keygen_calibration.public_inputs,
        proof_step_count: 1,
        secure: &keygen_calibration.secure,
        output_membership: &keygen_calibration.output_membership,
        step_eq_recursion: &step_eq_seed_recursion,
        step_ep_recursion: &step_ep_seed_recursion,
        step_eq_bootstrap: Some(&step_eq_seed_bootstrap),
        step_ep_bootstrap: Some(&step_ep_seed_bootstrap),
    };
    let step_ep_keygen_audit = collect_kagemusha_scalar_audits_v4::<EpAffine>(
        keygen_witness.public_inputs,
        keygen_witness.proof_step_count,
        &step_ep_circuit_params,
        keygen_witness.step_ep_recursion,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let (keygen_step_eq, step_eq_keygen_output) = build_kagemusha_step_eq_circuit_sequential_v4(
        &keygen_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        KagemushaStepPublicModeV4::Bootstrap,
        None,
        &step_ep_keygen_audit,
    )?;
    drop(step_ep_keygen_audit);
    let step_eq_verifying_key = keygen_vk(&step_eq_params, &keygen_step_eq)
        .map_err(|error| format!("failed to generate Kagemusha V4 Eq VK: {error}"))?;
    let step_eq_break_points = capture_kagemusha_keygen_break_points_v4(
        &keygen_step_eq.builder,
        &step_eq_circuit_params,
        "StepEq",
    )?;
    let step_eq_proving_key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        keygen_pk(&step_eq_params, step_eq_verifying_key, &keygen_step_eq)
    }))
    .map_err(|_| "Kagemusha V4 Eq PK synthesis did not match captured breakpoints".to_owned())?
    .map_err(|error| format!("failed to generate Kagemusha V4 Eq PK: {error}"))?;
    if capture_kagemusha_keygen_break_points_v4(
        &keygen_step_eq.builder,
        &step_eq_circuit_params,
        "StepEq PK",
    )? != step_eq_break_points
    {
        return Err("Kagemusha V4 Eq PK synthesis changed captured breakpoints".to_owned());
    }
    drop(keygen_step_eq);

    let (keygen_step_ep, _) = build_kagemusha_step_ep_circuit_sequential_v4(
        &keygen_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        KagemushaStepPublicModeV4::Bootstrap,
        None,
        &step_eq_keygen_output,
    )?;
    drop(step_eq_keygen_output);
    let step_ep_verifying_key = keygen_vk(&step_ep_params, &keygen_step_ep)
        .map_err(|error| format!("failed to generate Kagemusha V4 Ep VK: {error}"))?;
    let step_ep_break_points = capture_kagemusha_keygen_break_points_v4(
        &keygen_step_ep.builder,
        &step_ep_circuit_params,
        "StepEp",
    )?;
    let step_ep_proving_key = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        keygen_pk(&step_ep_params, step_ep_verifying_key, &keygen_step_ep)
    }))
    .map_err(|_| "Kagemusha V4 Ep PK synthesis did not match captured breakpoints".to_owned())?
    .map_err(|error| format!("failed to generate Kagemusha V4 Ep PK: {error}"))?;
    if capture_kagemusha_keygen_break_points_v4(
        &keygen_step_ep.builder,
        &step_ep_circuit_params,
        "StepEp PK",
    )? != step_ep_break_points
    {
        return Err("Kagemusha V4 Ep PK synthesis changed captured breakpoints".to_owned());
    }
    drop(keygen_step_ep);

    let compile_config = || kagemusha_ipa_compile_config_v4(public_len);
    let step_eq_final_protocol = snark_verifier::system::halo2::compile(
        &step_eq_params,
        step_eq_proving_key.get_vk(),
        compile_config(),
    );
    let step_ep_final_protocol = snark_verifier::system::halo2::compile(
        &step_ep_params,
        step_ep_proving_key.get_vk(),
        compile_config(),
    );
    let step_eq_structure_sha256 = kagemusha_require_protocol_structure_v1(
        &step_eq_seed.protocol,
        &step_eq_final_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let step_ep_structure_sha256 = kagemusha_require_protocol_structure_v1(
        &step_ep_seed.protocol,
        &step_ep_final_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let step_eq_final_protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &step_eq_final_protocol,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let step_ep_final_protocol_sha256 = kagemusha_compiled_protocol_identity_sha256(
        &step_ep_final_protocol,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    if step_eq_final_protocol_sha256 == step_ep_final_protocol_sha256 {
        return Err("Kagemusha V4 Eq/Ep final protocol identities collide".to_owned());
    }

    let final_calibration = kagemusha_generation_calibration_v4(
        step_eq_final_protocol_sha256,
        step_ep_final_protocol_sha256,
    )?;
    let step_eq_final_seed_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_final_protocol.clone(),
        step_eq_structure_sha256,
        &step_eq_seed_bootstrap,
    )?;
    let step_ep_final_seed_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_final_protocol.clone(),
        step_ep_structure_sha256,
        &step_ep_seed_bootstrap,
    )?;
    let final_bootstrap_witness = KagemushaStepWitnessV4 {
        public_inputs: &final_calibration.public_inputs,
        proof_step_count: 1,
        secure: &final_calibration.secure,
        output_membership: &final_calibration.output_membership,
        step_eq_recursion: &step_eq_final_seed_recursion,
        step_ep_recursion: &step_ep_final_seed_recursion,
        step_eq_bootstrap: Some(&step_eq_seed_bootstrap),
        step_ep_bootstrap: Some(&step_ep_seed_bootstrap),
    };
    let step_eq_zero_instances = vec![vec![Fp::ZERO; public_len]];
    let step_ep_zero_instances = vec![vec![Fq::ZERO; public_len]];
    let step_ep_final_bootstrap_audit = collect_kagemusha_scalar_audits_v4::<EpAffine>(
        final_bootstrap_witness.public_inputs,
        final_bootstrap_witness.proof_step_count,
        &step_ep_circuit_params,
        final_bootstrap_witness.step_ep_recursion,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let (final_bootstrap_step_eq, step_eq_final_bootstrap_output) =
        build_kagemusha_step_eq_circuit_sequential_v4(
            &final_bootstrap_witness,
            step_eq_circuit_params.clone(),
            &step_ep_circuit_params,
            KagemushaStepPublicModeV4::Bootstrap,
            Some(&step_eq_break_points),
            &step_ep_final_bootstrap_audit,
        )?;
    drop(step_ep_final_bootstrap_audit);
    let step_eq_bootstrap_proof = create_augmented_eq_proof_v4(
        &step_eq_params,
        &step_eq_proving_key,
        final_bootstrap_step_eq,
        &step_eq_zero_instances,
    )?;
    let (final_bootstrap_step_ep, _) = build_kagemusha_step_ep_circuit_sequential_v4(
        &final_bootstrap_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        KagemushaStepPublicModeV4::Bootstrap,
        Some(&step_ep_break_points),
        &step_eq_final_bootstrap_output,
    )?;
    drop(step_eq_final_bootstrap_output);
    let step_ep_bootstrap_proof = create_augmented_ep_proof_v4(
        &step_ep_params,
        &step_ep_proving_key,
        final_bootstrap_step_ep,
        &step_ep_zero_instances,
    )?;
    if step_eq_bootstrap_proof.len()
        != usize::try_from(step_eq_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq proof size does not fit usize".to_owned())?
        || step_ep_bootstrap_proof.len()
            != usize::try_from(step_ep_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Ep proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 final/bootstrap proof size did not converge".to_owned());
    }
    let step_eq_bootstrap_current = succinct_verify_step_eq_instances(
        &step_eq_params,
        step_eq_proving_key.get_vk(),
        &step_eq_bootstrap_proof,
        &step_eq_zero_instances,
        step_eq_bootstrap_proof.len(),
    )?;
    let step_ep_bootstrap_current = succinct_verify_step_ep_instances(
        &step_ep_params,
        step_ep_proving_key.get_vk(),
        &step_ep_bootstrap_proof,
        &step_ep_zero_instances,
        step_ep_bootstrap_proof.len(),
    )?;

    let step_eq_final_bootstrap = kagemusha_eq_seed_bootstrap_payload_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        &KagemushaEqBootstrapSeedV4 {
            protocol: step_eq_seed.protocol.clone(),
            structure_sha256: step_eq_structure_sha256,
            protocol_sha256: step_eq_seed.protocol_sha256,
            proof: step_eq_bootstrap_proof,
            current: step_eq_bootstrap_current,
        },
    )?;
    let step_ep_final_bootstrap = kagemusha_ep_seed_bootstrap_payload_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        &KagemushaEpBootstrapSeedV4 {
            protocol: step_ep_seed.protocol.clone(),
            structure_sha256: step_ep_structure_sha256,
            protocol_sha256: step_ep_seed.protocol_sha256,
            proof: step_ep_bootstrap_proof,
            current: step_ep_bootstrap_current,
        },
    )?;
    terminal_validate_kagemusha_eq_bootstrap_v4(
        &step_eq_params,
        step_eq_proving_key.get_vk(),
        &step_eq_circuit_params,
        &step_eq_final_bootstrap,
    )?;
    terminal_validate_kagemusha_ep_bootstrap_v4(
        &step_ep_params,
        step_ep_proving_key.get_vk(),
        &step_ep_circuit_params,
        &step_ep_final_bootstrap,
    )?;
    let step_eq_bootstrap_witness = step_eq_final_bootstrap.encode_authenticated(
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_structure_sha256,
    )?;
    let step_ep_bootstrap_witness = step_ep_final_bootstrap.encode_authenticated(
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_structure_sha256,
    )?;
    KagemushaStepBootstrapV4::decode_authenticated(
        &step_eq_bootstrap_witness,
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
        step_eq_structure_sha256,
    )?;
    KagemushaStepBootstrapV4::decode_authenticated(
        &step_ep_bootstrap_witness,
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
        step_ep_structure_sha256,
    )?;

    let live_calibration = kagemusha_generation_calibration_v4(
        step_eq_final_protocol_sha256,
        step_ep_final_protocol_sha256,
    )?;
    let step_eq_live_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_final_protocol.clone(),
        step_eq_structure_sha256,
        &step_eq_final_bootstrap,
    )?;
    let step_ep_live_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_final_protocol.clone(),
        step_ep_structure_sha256,
        &step_ep_final_bootstrap,
    )?;
    let live_witness = KagemushaStepWitnessV4 {
        public_inputs: &live_calibration.public_inputs,
        proof_step_count: 1,
        secure: &live_calibration.secure,
        output_membership: &live_calibration.output_membership,
        step_eq_recursion: &step_eq_live_recursion,
        step_ep_recursion: &step_ep_live_recursion,
        step_eq_bootstrap: Some(&step_eq_final_bootstrap),
        step_ep_bootstrap: Some(&step_ep_final_bootstrap),
    };
    let step_ep_live_audit = collect_kagemusha_scalar_audits_v4::<EpAffine>(
        live_witness.public_inputs,
        live_witness.proof_step_count,
        &step_ep_circuit_params,
        live_witness.step_ep_recursion,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let (live_step_eq, step_eq_live_output) = build_kagemusha_step_eq_circuit_sequential_v4(
        &live_witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        KagemushaStepPublicModeV4::Live,
        Some(&step_eq_break_points),
        &step_ep_live_audit,
    )?;
    drop(step_ep_live_audit);
    let step_eq_live_proof = prove_step_eq_v4(
        &step_eq_params,
        &step_eq_proving_key,
        live_step_eq,
        &live_calibration.public_inputs,
        1,
        &step_eq_circuit_params,
    )?;
    let (live_step_ep, _) = build_kagemusha_step_ep_circuit_sequential_v4(
        &live_witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        KagemushaStepPublicModeV4::Live,
        Some(&step_ep_break_points),
        &step_eq_live_output,
    )?;
    drop(step_eq_live_output);
    let step_ep_live_proof = prove_step_ep_v4(
        &step_ep_params,
        &step_ep_proving_key,
        live_step_ep,
        &live_calibration.public_inputs,
        1,
        &step_ep_circuit_params,
    )?;
    if step_eq_live_proof.len()
        != usize::try_from(step_eq_circuit_params.max_parent_proof_bytes)
            .map_err(|_| "Kagemusha V4 Eq live proof size does not fit usize".to_owned())?
        || step_ep_live_proof.len()
            != usize::try_from(step_ep_circuit_params.max_parent_proof_bytes)
                .map_err(|_| "Kagemusha V4 Ep live proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V4 live proof size differs from bootstrap calibration".to_owned());
    }
    let measured_pair = KagemushaPastaCycleProofPairV4 {
        version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
        proof_step_count: 1,
        public_inputs: live_calibration.public_inputs,
        step_eq_proof_bytes: step_eq_live_proof,
        step_ep_proof_bytes: step_ep_live_proof,
        step_eq_accumulation_proof: KagemushaIpaAccumulationProofV4::initialization(
            step_eq_circuit_params.k,
        )?,
        step_ep_accumulation_proof: KagemushaIpaAccumulationProofV4::initialization(
            step_ep_circuit_params.k,
        )?,
    };
    let absolute_pair_max =
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4;
    terminal_verify_proof_pair_v4(
        &step_eq_params,
        step_eq_proving_key.get_vk(),
        &step_ep_params,
        step_ep_proving_key.get_vk(),
        &measured_pair,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        absolute_pair_max,
    )?;
    let measured_live_pair_bytes = measured_pair.encode_authenticated(
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        absolute_pair_max,
    )?;
    KagemushaPastaCycleProofPairV4::decode_authenticated(
        &measured_live_pair_bytes,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        absolute_pair_max,
    )?;

    let step_eq_keys = finalize_kagemusha_eq_generated_keys_v4(
        step_eq_params,
        step_eq_proving_key,
        &step_eq_circuit_params,
        &step_eq_break_points,
    )?;
    let step_ep_keys = finalize_kagemusha_ep_generated_keys_v4(
        step_ep_params,
        step_ep_proving_key,
        &step_ep_circuit_params,
        &step_ep_break_points,
    )?;

    Ok(KagemushaGeneratedPastaCycleArtifactsV4 {
        step_eq: KagemushaGeneratedParityArtifactsV4 {
            circuit_params: step_eq_circuit_params.clone(),
            compiled_protocol_structure_sha256: step_eq_structure_sha256,
            step_proof_size_bytes: step_eq_circuit_params.max_parent_proof_bytes,
            parameters: step_eq_keys.parameters,
            proving_key: step_eq_keys.proving_key,
            verifying_key: step_eq_keys.verifying_key,
            bootstrap_witness: step_eq_bootstrap_witness,
        },
        step_ep: KagemushaGeneratedParityArtifactsV4 {
            circuit_params: step_ep_circuit_params.clone(),
            compiled_protocol_structure_sha256: step_ep_structure_sha256,
            step_proof_size_bytes: step_ep_circuit_params.max_parent_proof_bytes,
            parameters: step_ep_keys.parameters,
            proving_key: step_ep_keys.proving_key,
            verifying_key: step_ep_keys.verifying_key,
            bootstrap_witness: step_ep_bootstrap_witness,
        },
        measured_live_pair_bytes,
    })
}

/// Produce and immediately self-verify one concrete V4 StepEq proof.
pub(crate) fn prove_step_eq_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: &halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit: KagemushaStepEqCircuitV4,
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EqAffine},
        plonk::{create_proof, verify_proof},
        poly::{commitment::Params as _, ipa::commitment::IPACommitmentScheme},
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k || circuit.params != *circuit_params {
        return Err("Kagemusha V4 StepEq proving configuration mismatch".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let column = public_inputs.instance_column::<Fp>(proof_step_count, circuit_params)?;
    let columns: [&[Fp]; 1] = [&column];
    let proofs_instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceProverIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Eq proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
    >(
        params,
        proving_key.get_vk(),
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Eq generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Eq proof bound does not fit usize".to_owned())?;
    if proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha V4 Eq proof exceeds its authenticated bound".to_owned());
    }
    terminal_verify_step_eq_v4(
        params,
        proving_key.get_vk(),
        &proof,
        public_inputs,
        proof_step_count,
        circuit_params,
    )?;
    Ok(proof)
}

/// Produce and immediately self-verify one concrete V4 StepEp proof.
pub(crate) fn prove_step_ep_v4(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: &halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit: KagemushaStepEpCircuitV4,
    public_inputs: &KagemushaPastaCyclePublicInputsV4,
    proof_step_count: u32,
    circuit_params: &KagemushaStepCircuitParamsV4,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EpAffine},
        plonk::{create_proof, verify_proof},
        poly::{commitment::Params as _, ipa::commitment::IPACommitmentScheme},
    };
    use rand_core_06::OsRng;
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    public_inputs.validate(proof_step_count, circuit_params)?;
    if params.k() != circuit_params.k || circuit.params != *circuit_params {
        return Err("Kagemusha V4 StepEp proving configuration mismatch".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let column = public_inputs.instance_column::<Fq>(proof_step_count, circuit_params)?;
    let columns: [&[Fq]; 1] = [&column];
    let proofs_instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    create_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceProverIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        &[circuit],
        &proofs_instances,
        OsRng,
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V4 Ep proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
    >(
        params,
        proving_key.get_vk(),
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proofs_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V4 Ep generator: {error}"))?;
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    let max_proof_bytes = usize::try_from(circuit_params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V4 Ep proof bound does not fit usize".to_owned())?;
    if proof.is_empty() || proof.len() > max_proof_bytes {
        return Err("Kagemusha V4 Ep proof exceeds its authenticated bound".to_owned());
    }
    terminal_verify_step_ep_v4(
        params,
        proving_key.get_vk(),
        &proof,
        public_inputs,
        proof_step_count,
        circuit_params,
    )?;
    Ok(proof)
}

/// Bit-exact SHA-256 gadget used to join the two Pasta verifier halves.
///
/// The input length is part of the fixed circuit shape. Every input byte is
/// range constrained, SHA padding is inserted as circuit constants, and every
/// Boolean and modular-addition relation is constrained. The returned words
/// are the standard big-endian SHA-256 digest words.
pub struct KagemushaSha256Chip;

impl KagemushaSha256Chip {
    /// Constrain SHA-256 over one fixed-length byte slice.
    pub fn digest<F>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        message: &[halo2_base::AssignedValue<F>],
    ) -> [halo2_base::AssignedValue<F>; 8]
    where
        F: halo2_base::utils::BigPrimeField,
    {
        use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};

        #[derive(Clone)]
        struct Word<F: halo2_base::utils::BigPrimeField> {
            value: halo2_base::AssignedValue<F>,
            bits: Vec<halo2_base::AssignedValue<F>>,
        }

        fn word_from_bits<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            bits: Vec<halo2_base::AssignedValue<F>>,
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            assert_eq!(bits.len(), 32, "SHA-256 words contain 32 bits");
            let gate = range.gate();
            let value = gate.inner_product(
                ctx,
                bits.iter().copied().map(Existing),
                gate.pow_of_two()[..32]
                    .iter()
                    .copied()
                    .map(halo2_base::QuantumCell::Constant),
            );
            Word { value, bits }
        }

        fn constant_word<F>(ctx: &mut halo2_base::Context<F>, value: u32) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            let bits = (0..32)
                .map(|bit| ctx.load_constant(F::from(u64::from((value >> bit) & 1))))
                .collect();
            let value = ctx.load_constant(F::from(u64::from(value)));
            Word { value, bits }
        }

        fn xor_bit<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            lhs: halo2_base::AssignedValue<F>,
            rhs: halo2_base::AssignedValue<F>,
        ) -> halo2_base::AssignedValue<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            let gate = range.gate();
            let product = gate.mul(ctx, Existing(lhs), Existing(rhs));
            let sum = gate.add(ctx, Existing(lhs), Existing(rhs));
            let twice = gate.mul(
                ctx,
                Existing(product),
                halo2_base::QuantumCell::Constant(F::from(2)),
            );
            gate.sub(ctx, Existing(sum), Existing(twice))
        }

        fn rotate_right<F>(word: &Word<F>, amount: usize) -> Vec<halo2_base::AssignedValue<F>>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            (0..32).map(|bit| word.bits[(bit + amount) % 32]).collect()
        }

        fn shift_right<F>(
            ctx: &mut halo2_base::Context<F>,
            word: &Word<F>,
            amount: usize,
        ) -> Vec<halo2_base::AssignedValue<F>>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            let zero = ctx.load_constant(F::ZERO);
            (0..32)
                .map(|bit| word.bits.get(bit + amount).copied().unwrap_or(zero))
                .collect()
        }

        fn xor_bit_vectors<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            vectors: &[Vec<halo2_base::AssignedValue<F>>],
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            assert!(vectors.len() >= 2);
            let bits = (0..32)
                .map(|bit| {
                    vectors[1..].iter().fold(vectors[0][bit], |acc, vector| {
                        xor_bit(ctx, range, acc, vector[bit])
                    })
                })
                .collect();
            word_from_bits(ctx, range, bits)
        }

        fn choice<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            e: &Word<F>,
            f: &Word<F>,
            g: &Word<F>,
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            let gate = range.gate();
            let bits = (0..32)
                .map(|bit| {
                    let selected = gate.and(ctx, Existing(e.bits[bit]), Existing(f.bits[bit]));
                    let not_e = gate.not(ctx, Existing(e.bits[bit]));
                    let fallback = gate.and(ctx, Existing(not_e), Existing(g.bits[bit]));
                    gate.add(ctx, Existing(selected), Existing(fallback))
                })
                .collect();
            word_from_bits(ctx, range, bits)
        }

        fn majority<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            a: &Word<F>,
            b: &Word<F>,
            c: &Word<F>,
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};
            let gate = range.gate();
            let bits = (0..32)
                .map(|bit| {
                    let ab = gate.and(ctx, Existing(a.bits[bit]), Existing(b.bits[bit]));
                    let ac = gate.and(ctx, Existing(a.bits[bit]), Existing(c.bits[bit]));
                    let bc = gate.and(ctx, Existing(b.bits[bit]), Existing(c.bits[bit]));
                    let partial = xor_bit(ctx, range, ab, ac);
                    xor_bit(ctx, range, partial, bc)
                })
                .collect();
            word_from_bits(ctx, range, bits)
        }

        fn add_words<F>(
            ctx: &mut halo2_base::Context<F>,
            range: &halo2_base::gates::RangeChip<F>,
            words: &[&Word<F>],
        ) -> Word<F>
        where
            F: halo2_base::utils::BigPrimeField,
        {
            use halo2_base::{
                QuantumCell::{Constant, Existing},
                gates::{GateInstructions as _, RangeInstructions as _},
            };
            assert!(!words.is_empty());
            let host_sum = words
                .iter()
                .fold(0_u64, |sum, word| sum + word.value.value().get_lower_64());
            let result = ctx.load_witness(F::from(host_sum & 0xffff_ffff));
            let quotient = ctx.load_witness(F::from(host_sum >> 32));
            let gate = range.gate();
            let total = gate.sum(ctx, words.iter().map(|word| Existing(word.value)));
            let reconstructed = gate.mul_add(
                ctx,
                Existing(quotient),
                Constant(F::from(1_u64 << 32)),
                Existing(result),
            );
            ctx.constrain_equal(&total, &reconstructed);
            range.range_check(ctx, quotient, 3);
            let bits = gate.num_to_bits(ctx, result, 32);
            Word {
                value: result,
                bits,
            }
        }

        const INITIAL: [u32; 8] = [
            0x6a09_e667,
            0xbb67_ae85,
            0x3c6e_f372,
            0xa54f_f53a,
            0x510e_527f,
            0x9b05_688c,
            0x1f83_d9ab,
            0x5be0_cd19,
        ];
        const ROUND: [u32; 64] = [
            0x428a_2f98,
            0x7137_4491,
            0xb5c0_fbcf,
            0xe9b5_dba5,
            0x3956_c25b,
            0x59f1_11f1,
            0x923f_82a4,
            0xab1c_5ed5,
            0xd807_aa98,
            0x1283_5b01,
            0x2431_85be,
            0x550c_7dc3,
            0x72be_5d74,
            0x80de_b1fe,
            0x9bdc_06a7,
            0xc19b_f174,
            0xe49b_69c1,
            0xefbe_4786,
            0x0fc1_9dc6,
            0x240c_a1cc,
            0x2de9_2c6f,
            0x4a74_84aa,
            0x5cb0_a9dc,
            0x76f9_88da,
            0x983e_5152,
            0xa831_c66d,
            0xb003_27c8,
            0xbf59_7fc7,
            0xc6e0_0bf3,
            0xd5a7_9147,
            0x06ca_6351,
            0x1429_2967,
            0x27b7_0a85,
            0x2e1b_2138,
            0x4d2c_6dfc,
            0x5338_0d13,
            0x650a_7354,
            0x766a_0abb,
            0x81c2_c92e,
            0x9272_2c85,
            0xa2bf_e8a1,
            0xa81a_664b,
            0xc24b_8b70,
            0xc76c_51a3,
            0xd192_e819,
            0xd699_0624,
            0xf40e_3585,
            0x106a_a070,
            0x19a4_c116,
            0x1e37_6c08,
            0x2748_774c,
            0x34b0_bcb5,
            0x391c_0cb3,
            0x4ed8_aa4a,
            0x5b9c_ca4f,
            0x682e_6ff3,
            0x748f_82ee,
            0x78a5_636f,
            0x84c8_7814,
            0x8cc7_0208,
            0x90be_fffa,
            0xa450_6ceb,
            0xbef9_a3f7,
            0xc671_78f2,
        ];

        let mut byte_bits = Vec::with_capacity(message.len() + 72);
        for byte in message {
            range.range_check(ctx, *byte, 8);
            byte_bits.push(range.gate().num_to_bits(ctx, *byte, 8));
        }
        let bit_length = u64::try_from(message.len())
            .expect("fixed SHA-256 message length fits u64")
            .checked_mul(8)
            .expect("fixed SHA-256 bit length fits u64");
        let mut padding = vec![0x80_u8];
        while (message.len() + padding.len()) % 64 != 56 {
            padding.push(0);
        }
        padding.extend_from_slice(&bit_length.to_be_bytes());
        for byte in padding {
            byte_bits.push(
                (0..8)
                    .map(|bit| ctx.load_constant(F::from(u64::from((byte >> bit) & 1))))
                    .collect(),
            );
        }
        assert_eq!(byte_bits.len() % 64, 0);

        let mut state = INITIAL.map(|value| constant_word(ctx, value));
        for block in byte_bits.chunks_exact(64) {
            let mut schedule = Vec::with_capacity(64);
            for bytes in block.chunks_exact(4) {
                let bits = bytes
                    .iter()
                    .rev()
                    .flat_map(|byte| byte.iter().copied())
                    .collect();
                schedule.push(word_from_bits(ctx, range, bits));
            }
            for index in 16..64 {
                let shifted_15 = shift_right(ctx, &schedule[index - 15], 3);
                let s0 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&schedule[index - 15], 7),
                        rotate_right(&schedule[index - 15], 18),
                        shifted_15,
                    ],
                );
                let shifted_2 = shift_right(ctx, &schedule[index - 2], 10);
                let s1 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&schedule[index - 2], 17),
                        rotate_right(&schedule[index - 2], 19),
                        shifted_2,
                    ],
                );
                let next = add_words(
                    ctx,
                    range,
                    &[&schedule[index - 16], &s0, &schedule[index - 7], &s1],
                );
                schedule.push(next);
            }

            let mut working = state.clone();
            for round in 0..64 {
                let sigma1 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&working[4], 6),
                        rotate_right(&working[4], 11),
                        rotate_right(&working[4], 25),
                    ],
                );
                let choose = choice(ctx, range, &working[4], &working[5], &working[6]);
                let round_constant = constant_word(ctx, ROUND[round]);
                let t1 = add_words(
                    ctx,
                    range,
                    &[
                        &working[7],
                        &sigma1,
                        &choose,
                        &round_constant,
                        &schedule[round],
                    ],
                );
                let sigma0 = xor_bit_vectors(
                    ctx,
                    range,
                    &[
                        rotate_right(&working[0], 2),
                        rotate_right(&working[0], 13),
                        rotate_right(&working[0], 22),
                    ],
                );
                let majority = majority(ctx, range, &working[0], &working[1], &working[2]);
                let t2 = add_words(ctx, range, &[&sigma0, &majority]);
                let next_a = add_words(ctx, range, &[&t1, &t2]);
                let next_e = add_words(ctx, range, &[&working[3], &t1]);
                working = [
                    next_a,
                    working[0].clone(),
                    working[1].clone(),
                    working[2].clone(),
                    next_e,
                    working[4].clone(),
                    working[5].clone(),
                    working[6].clone(),
                ];
            }
            state = std::array::from_fn(|index| {
                add_words(ctx, range, &[&state[index], &working[index]])
            });
        }

        state.map(|word| word.value)
    }
}

fn constrain_two_parent_presence_bits<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    parent_count: halo2_base::AssignedValue<F>,
) -> [halo2_base::AssignedValue<F>; 2]
where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Constant,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.range_check(ctx, parent_count, 2);
    let is_three = range
        .gate()
        .is_equal(ctx, parent_count, Constant(F::from(3)));
    range.gate().assert_is_const(ctx, &is_three, &F::ZERO);
    let is_zero = range.gate().is_zero(ctx, parent_count);
    let slot_zero = range.gate().not(ctx, is_zero);
    let slot_one = range
        .gate()
        .is_equal(ctx, parent_count, Constant(F::from(2)));
    range.gate().assert_bit(ctx, slot_zero);
    range.gate().assert_bit(ctx, slot_one);
    [slot_zero, slot_one]
}

/// Constrain one complete selector-bound V4 reciprocal audit once.
///
/// The complete post-branch stage plan is required for both public slots.  As
/// on the scalar side, each public exposure is multiplied by its slot-presence
/// bit; every deferred MSM, selector schedule, serialization, and hash is
/// evaluated only once.
fn constrain_reciprocal_point_audit_identity_v4<'chip, C>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<C::Base>,
    base: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::Base>,
    scalar: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::ScalarExt>,
    witness: &super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
    current_public_parent_count: halo2_base::AssignedValue<C::Base>,
    parent_public_parent_counts: [halo2_base::AssignedValue<C::Base>; 2],
    expected_words: [&[halo2_base::AssignedValue<C::Base>]; 2],
) -> Result<[halo2_base::AssignedValue<C::Base>; 8], String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    use super::kagemusha_cycle_loader::PastaCycleEccChip;

    if expected_words.iter().any(|words| words.len() != 8) {
        return Err("Kagemusha reciprocal V4 audit words have the wrong shape".to_owned());
    }
    scalar_lineage_v1::validate_stage_shapes_v4(stages, witness.equations.len())
        .map_err(|error| format!("invalid Kagemusha reciprocal V4 stage plan: {error:?}"))?;

    let slot_present =
        constrain_two_parent_presence_bits(ctx.main(), base.range, current_public_parent_count);
    let parent_has_carried = parent_public_parent_counts.map(|parent_count| {
        constrain_two_parent_presence_bits(ctx.main(), base.range, parent_count)[0]
    });

    let mut gate_tags = Vec::with_capacity(witness.equations.len());
    let mut selectors = Vec::with_capacity(witness.equations.len());
    for stage in stages {
        let enabled = match stage.gate {
            scalar_lineage_v1::DeferredEquationGateV4::ParentCurrent { slot }
            | scalar_lineage_v1::DeferredEquationGateV4::ParentLineageSelect { slot } => {
                slot_present[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::ParentCarriedFold { slot } => {
                let enabled = base.range.gate().mul(
                    ctx.main(),
                    Existing(slot_present[slot]),
                    Existing(parent_has_carried[slot]),
                );
                base.range.gate().assert_bit(ctx.main(), enabled);
                enabled
            }
            scalar_lineage_v1::DeferredEquationGateV4::BranchFold => slot_present[1],
            scalar_lineage_v1::DeferredEquationGateV4::BranchSelect => slot_present[0],
        };
        gate_tags.extend(std::iter::repeat_n(
            stage.gate.audit_tag(),
            stage.range.len(),
        ));
        selectors.extend(std::iter::repeat_n(enabled, stage.range.len()));
    }

    let mut chip = PastaCycleEccChip::<C>::new(base, scalar);
    let audit = chip.constrain_deferred_equations_with_selectors(ctx, witness, &selectors)?;
    let bytes = chip.assigned_equation_bytes_v4(ctx, &audit, &gate_tags, &selectors)?;
    let digest = KagemushaSha256Chip::digest(ctx.main(), base.range, &bytes);
    for (slot_present, expected_words) in slot_present.into_iter().zip(expected_words) {
        for (assigned, expected) in digest.iter().zip(expected_words) {
            let exposed =
                base.range
                    .gate()
                    .mul(ctx.main(), Existing(slot_present), Existing(*assigned));
            ctx.main().constrain_equal(&exposed, expected);
        }
    }
    Ok(digest)
}

/// Reconstruct the exact compiled-protocol identity in the reciprocal
/// native-point circuit and bind it to the same public release words.
///
/// The protocol points are assigned and canonicalized independently here.
/// Their equality with the values used by the scalar verifier follows from the
/// scalar/point deferred-equation SHA join; this additional digest anchors that
/// common point namespace and transcript state to the authenticated release.
fn constrain_reciprocal_protocol_identity<'chip, C>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<C::Base>,
    base: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::Base>,
    scalar: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::ScalarExt>,
    identity: &scalar_lineage_v1::DeferredProtocolIdentityWitness<C>,
    fixed_structure_sha256: [u8; 32],
    expected_words: &[halo2_base::AssignedValue<C::Base>],
) -> Result<[halo2_base::AssignedValue<C::Base>; 8], String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use snark_verifier::loader::halo2::{EccInstructions as _, IntegerInstructions as _};

    use super::kagemusha_cycle_loader::PastaCycleEccChip;

    if expected_words.len() != 8
        || identity.structure_sha256 != fixed_structure_sha256
        || identity.preprocessed.is_empty()
        || identity
            .preprocessed
            .iter()
            .any(|point| bool::from(point.is_identity()))
    {
        return Err("Kagemusha reciprocal protocol identity shape mismatch".to_owned());
    }
    let mut push_constants = |output: &mut Vec<halo2_base::AssignedValue<C::Base>>,
                              bytes: &[u8]| {
        output.extend(
            bytes
                .iter()
                .map(|byte| ctx.main().load_constant(C::Base::from(u64::from(*byte)))),
        );
    };
    let mut bytes = Vec::new();
    push_constants(&mut bytes, KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_DOMAIN_V1);
    push_constants(&mut bytes, &[0]);
    push_constants(
        &mut bytes,
        &KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_VERSION_V1.to_le_bytes(),
    );
    push_constants(
        &mut bytes,
        &protocol_parity_tag(identity.parity).to_le_bytes(),
    );
    push_constants(&mut bytes, &fixed_structure_sha256);
    push_constants(
        &mut bytes,
        &u32::try_from(identity.preprocessed.len())
            .map_err(|_| "Kagemusha reciprocal protocol point count does not fit u32".to_owned())?
            .to_le_bytes(),
    );
    drop(push_constants);

    let chip = PastaCycleEccChip::<C>::new(base, scalar);
    for point in &identity.preprocessed {
        let point = chip.assign_point(ctx, *point);
        bytes.extend(chip.assigned_point_bytes(ctx, &point));
    }
    let transcript_initial_state = chip
        .scalar_chip()
        .assign_integer(ctx, identity.transcript_initial_state);
    bytes.extend(chip.assigned_scalar_bytes(ctx, &transcript_initial_state));
    let digest = KagemushaSha256Chip::digest(ctx.main(), base.range, &bytes);
    for (assigned, expected) in digest.iter().zip(expected_words) {
        ctx.main().constrain_equal(assigned, expected);
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use std::{mem, rc::Rc};

    use super::*;
    use iroha_data_model::offline::{
        KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4, KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
        KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4,
        KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4,
    };
    use norito::to_bytes;

    use halo2_proofs::arithmetic::Field;
    use snark_verifier::util::arithmetic::PrimeCurveAffine as _;

    fn valid_step_circuit_params_v4() -> KagemushaStepCircuitParamsV4 {
        valid_step_circuit_params_for_k_v4(20)
    }

    fn valid_step_circuit_params_for_k_v4(k: u32) -> KagemushaStepCircuitParamsV4 {
        let layout = KagemushaPastaPublicLayoutV4::for_ipa_round_count(k)
            .expect("supported V4 public layout");
        KagemushaStepCircuitParamsV4 {
            version: KAGEMUSHA_STEP_CIRCUIT_PARAMS_VERSION_V4,
            k,
            num_advice_per_phase: vec![1],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: k - 1,
            num_instance_columns: 1,
            public_input_limbs: layout.instance_column_limbs,
            minimum_unusable_rows: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_UNUSABLE_ROWS_V4,
            max_parent_proof_bytes: 16_384,
        }
    }

    fn release_step_circuit_params_v4() -> KagemushaStepCircuitParamsV4 {
        let mut params = valid_step_circuit_params_v4();
        params.num_advice_per_phase = KAGEMUSHA_STEP_CIRCUIT_RELEASE_ADVICE_COLUMNS_V4.to_vec();
        params.num_lookup_advice_per_phase =
            KAGEMUSHA_STEP_CIRCUIT_RELEASE_LOOKUP_COLUMNS_V4.to_vec();
        params
            .validate_release_generation_profile()
            .expect("reviewed degree-20 generation profile");
        params
    }

    fn require_release_test_resource_guard_v4(phase: &str) {
        #[cfg(unix)]
        {
            use std::{fs::OpenOptions, io::Write as _, os::unix::fs::FileTypeExt as _};

            let descriptor = std::env::var("IROHA_KAGEMUSHA_V4_GUARD_FD")
                .expect("full Kagemusha V4 generation must use the staged resource supervisor")
                .parse::<i32>()
                .expect("resource-supervisor descriptor must be an integer");
            assert!(descriptor >= 3, "guard descriptor must not alias stdio");
            let path = [
                format!("/proc/self/fd/{descriptor}"),
                format!("/dev/fd/{descriptor}"),
            ]
            .into_iter()
            .find(|path| std::path::Path::new(path).exists())
            .expect("resource-supervisor descriptor is not open");
            let mut channel = OpenOptions::new()
                .write(true)
                .open(path)
                .expect("open resource-supervisor descriptor");
            assert!(
                channel
                    .metadata()
                    .expect("guard metadata")
                    .file_type()
                    .is_fifo(),
                "resource-supervisor descriptor must be a live pipe"
            );
            writeln!(channel, "stage={phase}").expect("write resource-supervisor phase");
            channel.flush().expect("flush resource-supervisor phase");
        }
        #[cfg(not(unix))]
        panic!("full Kagemusha V4 generation requires a POSIX resource supervisor");
    }

    #[test]
    fn v4_config_applies_authenticated_unusable_rows_to_both_parities() {
        use halo2_proofs::plonk::{Circuit as _, ConstraintSystem};

        let params = valid_step_circuit_params_v4();
        let authenticated_minimum =
            usize::try_from(params.minimum_unusable_rows).expect("unusable rows fit usize");
        let expected = (1_usize << params.k) - authenticated_minimum;

        let mut eq_meta = ConstraintSystem::<Fp>::default();
        let eq_config =
            KagemushaStepEqCircuitV4::configure_with_params(&mut eq_meta, params.clone());
        assert!(
            authenticated_minimum >= eq_meta.minimum_rows(),
            "authenticated unusable rows must cover the configured Eq minimum"
        );
        assert_eq!(eq_config.gate().max_rows, expected);

        let mut ep_meta = ConstraintSystem::<Fq>::default();
        let ep_config = KagemushaStepEpCircuitV4::configure_with_params(&mut ep_meta, params);
        assert!(
            authenticated_minimum >= ep_meta.minimum_rows(),
            "authenticated unusable rows must cover the configured Ep minimum"
        );
        assert_eq!(ep_config.gate().max_rows, expected);
    }

    #[test]
    fn v4_witness_layout_simulation_rejects_every_breakpoint_failure_mode() {
        simulate_kagemusha_phase_assignment_v4(&[7], &[3, 3], 3, 4, "test", 0)
            .expect("repeated per-column offsets are valid");

        let unconsumed = simulate_kagemusha_phase_assignment_v4(&[1], &[3], 2, 4, "test", 0)
            .expect_err("an extra breakpoint must fail");
        assert!(unconsumed.contains("unconsumed breakpoint"));

        let missed = simulate_kagemusha_phase_assignment_v4(&[2], &[0, 0], 3, 4, "test", 0)
            .expect_err("a breakpoint behind the current row must fail");
        assert!(missed.contains("missed breakpoint"));

        let column = simulate_kagemusha_phase_assignment_v4(&[1], &[0], 1, 4, "test", 0)
            .expect_err("a breakpoint without another column must fail");
        assert!(column.contains("advice-column overflow"));

        let row = simulate_kagemusha_phase_assignment_v4(&[3], &[], 1, 2, "test", 0)
            .expect_err("a missing breakpoint must overflow usable rows");
        assert!(row.contains("usable-row overflow"));
    }

    #[test]
    fn v4_proving_key_payload_binds_breakpoints_parity_and_params() {
        let params = valid_step_circuit_params_v4();
        let header = KagemushaPastaCycleProvingKeyHeaderV4 {
            version: KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_params_sha256: params.sha256().expect("identify params"),
            break_points: vec![vec![]],
        };
        let raw_key = [0xa5, 0x5a];
        let encoded =
            super::super::kagemusha_artifact_v4::encode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &header,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
                &raw_key,
            )
            .expect("encode canonical proving-key payload");
        let framed_consumed = frame_kagemusha_consumed_proving_key_v4(
            &header,
            KagemushaPastaCycleParityV1::StepEq,
            &params,
            raw_key.to_vec(),
        )
        .expect("frame consumed proving-key bytes");
        assert_eq!(framed_consumed, encoded);
        assert!(
            frame_kagemusha_consumed_proving_key_v4(
                &header,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
                Vec::new(),
            )
            .is_err(),
            "an empty consumed proving key must fail"
        );
        let decoded =
            super::super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &encoded,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .expect("decode canonical proving-key payload");
        assert_eq!(decoded.header(), &header);
        assert_eq!(decoded.processed_proving_key(), raw_key);
        assert!(
            super::super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &encoded,
                KagemushaPastaCycleParityV1::StepEp,
                &params,
            )
            .is_err(),
            "wrong parity must fail"
        );
        let wrong_params = valid_step_circuit_params_for_k_v4(21);
        assert!(
            super::super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &encoded,
                KagemushaPastaCycleParityV1::StepEq,
                &wrong_params,
            )
            .is_err(),
            "wrong authenticated parameters must fail"
        );
        assert!(
            super::super::kagemusha_artifact_v4::decode_kagemusha_pasta_cycle_proving_key_payload_v4(
                &raw_key,
                KagemushaPastaCycleParityV1::StepEq,
                &params,
            )
            .is_err(),
            "legacy raw processed proving keys must fail"
        );
    }

    #[test]
    fn v4_prover_builder_installs_authenticated_stage() {
        let params = valid_step_circuit_params_v4();
        let break_points = vec![vec![]];
        let builder = kagemusha_step_builder_v4::<Fp>(&params, Some(&break_points))
            .expect("construct authenticated prover builder");
        assert!(builder.witness_gen_only());
        assert_eq!(builder.break_points(), break_points);
        validate_kagemusha_builder_layout_v4(&builder, &params, &break_points, true, "test prover")
            .expect("empty prover trace respects the authenticated stage");
    }

    #[test]
    fn v4_authenticated_minimum_framed_keys_prove_bootstrap_and_live_for_both_parities() {
        use halo2_base::{
            QuantumCell::Existing,
            gates::{GateInstructions as _, RangeInstructions as _},
        };
        use halo2_proofs::{
            SerdeFormat,
            halo2curves::pasta::{EpAffine, EqAffine},
            plonk::{keygen_pk, keygen_vk},
            poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
        };

        fn builder<F>(
            params: &KagemushaStepCircuitParamsV4,
            break_points: Option<&[Vec<usize>]>,
            live: bool,
        ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>
        where
            F: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField + From<u64>,
        {
            let mut builder = kagemusha_step_builder_v4::<F>(params, break_points)
                .expect("small-degree authenticated builder");
            let range = builder.range_chip();
            let ctx = builder.main(0);
            let semantic = ctx.load_witness(F::from(42));
            let selector = ctx.load_witness(F::from(u64::from(live)));
            range.gate().assert_bit(ctx, selector);
            let exposed = range
                .gate()
                .mul(ctx, Existing(selector), Existing(semantic));
            builder.assigned_instances = vec![vec![exposed]];
            builder
        }

        // Use the smallest authenticated V4 domain. The circuit is intentionally
        // tiny, while still using the exact production wrapper, captured
        // breakpoint header, parser, and witness-only proof stage.
        let circuit_params = valid_step_circuit_params_for_k_v4(
            iroha_data_model::offline::KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
        );
        {
            let params = ParamsIPA::<EqAffine>::new(circuit_params.k);
            let keygen_circuit = KagemushaStepEqCircuitV4 {
                params: circuit_params.clone(),
                builder: builder::<Fp>(&circuit_params, None, false),
            };
            let verifying_key = keygen_vk(&params, &keygen_circuit).expect("small-degree Eq VK");
            let break_points = capture_kagemusha_keygen_break_points_v4(
                &keygen_circuit.builder,
                &circuit_params,
                "small-degree Eq",
            )
            .expect("capture small-degree Eq breakpoints");
            let proving_key =
                keygen_pk(&params, verifying_key, &keygen_circuit).expect("small-degree Eq PK");
            assert_eq!(keygen_circuit.builder.break_points(), break_points);
            drop(keygen_circuit);

            let header = KagemushaPastaCycleProvingKeyHeaderV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
                parity: KagemushaPastaCycleParityV1::StepEq,
                circuit_params_sha256: circuit_params.sha256().expect("identify Eq params"),
                break_points: kagemusha_break_points_to_wire_v4(&break_points)
                    .expect("encode Eq breakpoints"),
            };
            let payload = frame_kagemusha_consumed_proving_key_v4(
                &header,
                KagemushaPastaCycleParityV1::StepEq,
                &circuit_params,
                proving_key.into_bytes(SerdeFormat::Processed),
            )
            .expect("frame consumed Eq key");
            let (proving_key, parsed_break_points) =
                parse_kagemusha_eq_pk_v4(&payload, circuit_params.clone())
                    .expect("parse framed Eq key");
            assert_eq!(parsed_break_points, break_points);

            for live in [false, true] {
                let prover_builder =
                    builder::<Fp>(&circuit_params, Some(&parsed_break_points), live);
                validate_kagemusha_builder_layout_v4(
                    &prover_builder,
                    &circuit_params,
                    &parsed_break_points,
                    true,
                    if live {
                        "small Eq live"
                    } else {
                        "small Eq bootstrap"
                    },
                )
                .expect("validate witness-only Eq layout");
                let circuit = KagemushaStepEqCircuitV4 {
                    params: circuit_params.clone(),
                    builder: prover_builder,
                };
                let instances = vec![vec![if live { Fp::from(42) } else { Fp::ZERO }]];
                create_augmented_eq_proof_v4(&params, &proving_key, circuit, &instances)
                    .expect("prove small-degree framed Eq circuit");
            }
        }
        {
            let params = ParamsIPA::<EpAffine>::new(circuit_params.k);
            let keygen_circuit = KagemushaStepEpCircuitV4 {
                params: circuit_params.clone(),
                builder: builder::<Fq>(&circuit_params, None, false),
            };
            let verifying_key = keygen_vk(&params, &keygen_circuit).expect("small-degree Ep VK");
            let break_points = capture_kagemusha_keygen_break_points_v4(
                &keygen_circuit.builder,
                &circuit_params,
                "small-degree Ep",
            )
            .expect("capture small-degree Ep breakpoints");
            let proving_key =
                keygen_pk(&params, verifying_key, &keygen_circuit).expect("small-degree Ep PK");
            assert_eq!(keygen_circuit.builder.break_points(), break_points);
            drop(keygen_circuit);

            let header = KagemushaPastaCycleProvingKeyHeaderV4 {
                version: KAGEMUSHA_RECURSIVE_SPEND_PROVING_KEY_PAYLOAD_VERSION_V4,
                parity: KagemushaPastaCycleParityV1::StepEp,
                circuit_params_sha256: circuit_params.sha256().expect("identify Ep params"),
                break_points: kagemusha_break_points_to_wire_v4(&break_points)
                    .expect("encode Ep breakpoints"),
            };
            let payload = frame_kagemusha_consumed_proving_key_v4(
                &header,
                KagemushaPastaCycleParityV1::StepEp,
                &circuit_params,
                proving_key.into_bytes(SerdeFormat::Processed),
            )
            .expect("frame consumed Ep key");
            let (proving_key, parsed_break_points) =
                parse_kagemusha_ep_pk_v4(&payload, circuit_params.clone())
                    .expect("parse framed Ep key");
            assert_eq!(parsed_break_points, break_points);

            for live in [false, true] {
                let prover_builder =
                    builder::<Fq>(&circuit_params, Some(&parsed_break_points), live);
                validate_kagemusha_builder_layout_v4(
                    &prover_builder,
                    &circuit_params,
                    &parsed_break_points,
                    true,
                    if live {
                        "small Ep live"
                    } else {
                        "small Ep bootstrap"
                    },
                )
                .expect("validate witness-only Ep layout");
                let circuit = KagemushaStepEpCircuitV4 {
                    params: circuit_params.clone(),
                    builder: prover_builder,
                };
                let instances = vec![vec![if live { Fq::from(42) } else { Fq::ZERO }]];
                create_augmented_ep_proof_v4(&params, &proving_key, circuit, &instances)
                    .expect("prove small-degree framed Ep circuit");
            }
        }
    }

    fn output_frontier_binding_builder(
        profile: [u64; 3],
        input_frontier: u64,
        result_frontier: u64,
        recipient_index: u64,
        change_index: u64,
        dummy_index: u64,
        topup_leaf_index: u64,
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;

        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(8)
            .use_lookup_bits(7);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let zero = ctx.load_witness(Fp::ZERO);
        let [is_init, is_append, is_redemption] =
            profile.map(|value| ctx.load_witness(Fp::from(value)));
        let input_frontier = ctx.load_witness(Fp::from(input_frontier));
        let result_frontier = ctx.load_witness(Fp::from(result_frontier));
        let mut output =
            [zero; crate::zk::kagemusha_v2::KAGEMUSHA_OUTPUT_MEMBERSHIP_INSTANCE_COLUMNS_V4];
        output[7] = ctx.load_witness(Fp::from(recipient_index));
        output[9] = ctx.load_witness(Fp::from(change_index));
        output[10] = ctx.load_witness(Fp::from(dummy_index));
        let bindings = crate::zk::kagemusha_step_transition::NamedTransitionBindings {
            operation: crate::zk::kagemusha_step_transition::AssignedKagemushaStepOperationV4 {
                limbs: vec![zero; KAGEMUSHA_STEP_OPERATION_LIMBS_V4]
                    .into_boxed_slice()
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("exact Kagemusha operation limb count")),
                fields: vec![zero; KAGEMUSHA_STEP_OPERATION_FIELD_ELEMENTS_V4]
                    .into_boxed_slice()
                    .try_into()
                    .unwrap_or_else(|_| unreachable!("exact Kagemusha operation field count")),
            },
            is_init,
            is_append,
            is_redemption,
            has_change: zero,
            input_root: zero,
            output_root: zero,
            input_next_zero_leaf_index: input_frontier,
            output_next_zero_leaf_index: result_frontier,
            input_commitments: [zero; 2],
            input_nullifiers: [zero; 2],
            recipient_commitment: zero,
            change_commitment: zero,
            statement_digest_limbs: [zero; 8],
            init_payer_tag_limbs: [zero; 8],
            init_operation_tag_limbs: [zero; 8],
        };
        let topup_leaf_index = ctx.load_witness(Fp::from(topup_leaf_index));
        constrain_kagemusha_output_frontier_v4(ctx, &range, &bindings, &output, topup_leaf_index);
        builder.calculate_params(Some(9));
        builder
    }

    fn assert_frontier_binding(
        expected_satisfied: bool,
        profile: [u64; 3],
        input: u64,
        result: u64,
        recipient: u64,
        change: u64,
        dummy: u64,
        topup: u64,
    ) {
        let builder = output_frontier_binding_builder(
            profile, input, result, recipient, change, dummy, topup,
        );
        let verification =
            halo2_proofs::dev::MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("frontier binding mock prover")
                .verify();
        assert_eq!(verification.is_ok(), expected_satisfied);
    }

    #[test]
    fn v4_eq_frontier_copy_constraints_reject_every_index_substitution() {
        assert_frontier_binding(true, [1, 0, 0], 0, 8, 7, 0, 8, 7);
        assert_frontier_binding(false, [1, 0, 0], 0, 8, 7, 0, 8, 6);

        assert_frontier_binding(true, [0, 1, 0], 7, 8, 7, 0, 8, 0);
        assert_frontier_binding(false, [0, 1, 0], 7, 8, 6, 0, 8, 0);

        assert_frontier_binding(true, [0, 0, 1], 7, 8, 0, 7, 8, 0);
        assert_frontier_binding(false, [0, 0, 1], 7, 8, 0, 6, 8, 0);

        assert_frontier_binding(false, [0, 1, 0], 7, 9, 7, 0, 8, 0);
    }

    #[test]
    fn v4_params_reject_default_k12_and_stale_public_layout() {
        assert!(KagemushaStepCircuitParamsV4::default().validate().is_err());

        let valid = valid_step_circuit_params_v4();
        let layout = valid.validate().expect("valid V4 lower-bound layout");
        assert_eq!(layout.accumulator_limbs, 170);
        assert_eq!(layout.instance_column_limbs, 4_156);
        assert_eq!(layout.live_selector_offset, 4_155);

        let mut k12 = valid.clone();
        k12.k = 12;
        assert!(k12.validate().is_err());

        let mut legacy_fixed_degree_layout = valid;
        legacy_fixed_degree_layout.public_input_limbs = 4_024;
        assert!(legacy_fixed_degree_layout.validate().is_err());
    }

    #[test]
    fn v4_release_generator_rejects_uncalibrated_profile_before_allocation() {
        let uncalibrated = valid_step_circuit_params_v4();
        assert!(uncalibrated.validate().is_ok());
        let error =
            match generate_kagemusha_pasta_cycle_artifacts_v4(uncalibrated.clone(), uncalibrated) {
                Ok(_) => panic!("single-column release generation must fail admission"),
                Err(error) => error,
            };
        assert!(error.contains("release profile is not reviewed"));
    }

    #[test]
    #[ignore = "release-evidence smoke test performs full degree-20 Eq/Ep key generation"]
    fn v4_release_generator_proves_verifies_and_rejects_substitution() {
        use std::collections::BTreeSet;

        use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine};
        use sha2::{Digest as _, Sha256};

        require_release_test_resource_guard_v4("core.release-generator.admitted");
        let requested = release_step_circuit_params_v4();
        let generated = generate_kagemusha_pasta_cycle_artifacts_v4(requested.clone(), requested)
            .expect("generate and self-verify the complete V4 Eq/Ep artifact set");
        let maximum = u32::try_from(generated.measured_live_pair_bytes.len())
            .expect("measured V4 pair length fits u32");
        let pair = KagemushaPastaCycleProofPairV4::decode_authenticated(
            &generated.measured_live_pair_bytes,
            &generated.step_eq.circuit_params,
            &generated.step_ep.circuit_params,
            maximum,
        )
        .expect("decode generated live V4 pair");

        let step_eq_params = parse_kagemusha_params_v4::<EqAffine>(
            &generated.step_eq.parameters,
            generated.step_eq.circuit_params.k,
            "generated smoke-test Eq",
        )
        .expect("parse generated Eq parameters");
        let step_ep_params = parse_kagemusha_params_v4::<EpAffine>(
            &generated.step_ep.parameters,
            generated.step_ep.circuit_params.k,
            "generated smoke-test Ep",
        )
        .expect("parse generated Ep parameters");
        let step_eq_vk = parse_kagemusha_eq_vk_v4(
            &generated.step_eq.verifying_key,
            generated.step_eq.circuit_params.clone(),
        )
        .expect("parse generated Eq verifying key");
        let step_ep_vk = parse_kagemusha_ep_vk_v4(
            &generated.step_ep.verifying_key,
            generated.step_ep.circuit_params.clone(),
        )
        .expect("parse generated Ep verifying key");
        terminal_verify_proof_pair_v4(
            &step_eq_params,
            &step_eq_vk,
            &step_ep_params,
            &step_ep_vk,
            &pair,
            &generated.step_eq.circuit_params,
            &generated.step_ep.circuit_params,
            maximum,
        )
        .expect("terminally verify both generated V4 proof halves");

        let payload_digests = [
            &generated.step_eq.parameters,
            &generated.step_eq.proving_key,
            &generated.step_eq.verifying_key,
            &generated.step_eq.bootstrap_witness,
            &generated.step_ep.parameters,
            &generated.step_ep.proving_key,
            &generated.step_ep.verifying_key,
            &generated.step_ep.bootstrap_witness,
        ]
        .map(|payload| <[u8; 32]>::from(Sha256::digest(payload)));
        assert_eq!(
            payload_digests.into_iter().collect::<BTreeSet<_>>().len(),
            8,
            "the generated external cryptographic inventory has eight distinct payloads"
        );

        let mut proof_substitution = pair.clone();
        proof_substitution.step_eq_proof_bytes[0] ^= 0x80;
        assert!(
            terminal_verify_proof_pair_v4(
                &step_eq_params,
                &step_eq_vk,
                &step_ep_params,
                &step_ep_vk,
                &proof_substitution,
                &generated.step_eq.circuit_params,
                &generated.step_ep.circuit_params,
                maximum,
            )
            .is_err(),
            "a substituted Eq transcript must fail terminal verification"
        );

        let mut public_input_substitution = pair;
        public_input_substitution.public_inputs.result_state[1] ^= 1;
        assert!(
            terminal_verify_proof_pair_v4(
                &step_eq_params,
                &step_eq_vk,
                &step_ep_params,
                &step_ep_vk,
                &public_input_substitution,
                &generated.step_eq.circuit_params,
                &generated.step_ep.circuit_params,
                maximum,
            )
            .is_err(),
            "a substituted continuing-state limb must fail both proof bindings"
        );
    }

    fn v4_complete_stage_plan() -> Vec<scalar_lineage_v1::DeferredEquationStageShapeV4> {
        use scalar_lineage_v1::{DeferredEquationGateV4 as Gate, DeferredEquationStageShapeV4};

        [
            Gate::ParentCurrent { slot: 0 },
            Gate::ParentCarriedFold { slot: 0 },
            Gate::ParentLineageSelect { slot: 0 },
            Gate::ParentCurrent { slot: 1 },
            Gate::ParentCarriedFold { slot: 1 },
            Gate::ParentLineageSelect { slot: 1 },
            Gate::BranchFold,
            Gate::BranchSelect,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, gate)| DeferredEquationStageShapeV4 {
            range: index..index + 1,
            gate,
        })
        .collect()
    }

    #[test]
    fn v4_complete_stage_validator_rejects_omission_reorder_and_duplicate() {
        let stages = v4_complete_stage_plan();
        scalar_lineage_v1::validate_stage_shapes_v4(&stages, 8).expect("complete V4 stage plan");

        for omitted in 0..stages.len() {
            let mut candidate = stages.clone();
            candidate.remove(omitted);
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted omission {omitted}"
            );
        }

        for swapped in 0..stages.len() - 1 {
            let mut candidate = stages.clone();
            candidate.swap(swapped, swapped + 1);
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted reorder at {swapped}"
            );
        }

        for duplicated in 0..stages.len() - 1 {
            let mut candidate = stages.clone();
            candidate[duplicated + 1].gate = candidate[duplicated].gate;
            assert!(
                scalar_lineage_v1::validate_stage_shapes_v4(&candidate, 8).is_err(),
                "accepted duplicate at {duplicated}"
            );
        }
    }

    #[test]
    fn v4_every_enabled_stage_is_covered_by_a_present_complete_join() {
        use scalar_lineage_v1::DeferredEquationGateV4 as Gate;

        let stages = v4_complete_stage_plan();
        for parent_count in 0..=2 {
            let slot_present = [parent_count >= 1, parent_count == 2];
            let parent_has_carried = [true, false];
            for stage in &stages {
                let enabled = match stage.gate {
                    Gate::ParentCurrent { slot } | Gate::ParentLineageSelect { slot } => {
                        slot_present[slot]
                    }
                    Gate::ParentCarriedFold { slot } => {
                        slot_present[slot] && parent_has_carried[slot]
                    }
                    Gate::BranchFold => slot_present[1],
                    Gate::BranchSelect => slot_present[0],
                };
                if enabled {
                    assert!(
                        slot_present[0]
                            && scalar_lineage_v1::validate_stage_shapes_v4(&stages, 8).is_ok()
                            && stages.iter().any(|candidate| candidate == stage),
                        "enabled {:?} is not covered for parent count {parent_count}",
                        stage.gate
                    );
                }
            }
        }
    }

    #[test]
    fn v4_host_deferred_audit_bytes_bind_complete_one_parent_branch_select() {
        use halo2_proofs::halo2curves::{group::prime::PrimeCurveAffine as _, pasta::EqAffine};

        use crate::zk::kagemusha_cycle_loader::{
            DeferredEquationWitness, KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4,
            KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4,
        };

        let source = EqAffine::generator();
        let coefficients = [3_u64, 5, 7, 11, 13, 17, 19, 23];
        let witness = DeferredEquationWitness::<EqAffine> {
            sources: vec![source],
            equations: coefficients
                .map(|coefficient| vec![(0, Fp::from(coefficient))])
                .to_vec(),
        };
        let stages = v4_complete_stage_plan();

        let expected_bytes = |selectors: [u8; 8], coefficients: [u64; 8]| {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4);
            bytes.push(0);
            bytes.extend_from_slice(&KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4.to_le_bytes());
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&8_u32.to_le_bytes());
            let coordinates: Option<snark_verifier::util::arithmetic::Coordinates<EqAffine>> =
                source.coordinates().into();
            let coordinates = coordinates.expect("generator has affine coordinates");
            bytes.extend_from_slice(coordinates.x().to_repr().as_ref());
            bytes.extend_from_slice(coordinates.y().to_repr().as_ref());
            for ((gate_tag, coefficient), selector) in [1_u32, 3, 5, 2, 4, 6, 7, 8]
                .into_iter()
                .zip(coefficients)
                .zip(selectors)
            {
                bytes.extend_from_slice(&gate_tag.to_le_bytes());
                bytes.push(selector);
                bytes.extend_from_slice(&1_u32.to_le_bytes());
                bytes.extend_from_slice(&0_u32.to_le_bytes());
                bytes.extend_from_slice(Fp::from(coefficient).to_repr().as_ref());
            }
            bytes
        };

        let one_parent = kagemusha_deferred_audit_public_words_v4(&witness, &stages, 1, [1, 0])
            .expect("serialize complete one-parent V4 audit");
        assert_eq!(
            one_parent[0],
            kagemusha_sha256_public_words(
                Sha256::digest(expected_bytes([1, 1, 1, 0, 0, 0, 0, 1], coefficients)).into()
            )
        );
        assert_ne!(one_parent[0], [0; 8]);
        assert_eq!(one_parent[1], [0; 8]);

        let mut tampered = witness.clone();
        tampered.equations[7] = vec![(0, Fp::from(29))];
        let tampered_one_parent =
            kagemusha_deferred_audit_public_words_v4(&tampered, &stages, 1, [1, 0])
                .expect("serialize BranchSelect-tampered V4 audit");
        assert_ne!(one_parent[0], tampered_one_parent[0]);
        assert_eq!(tampered_one_parent[1], [0; 8]);

        let two_parent = kagemusha_deferred_audit_public_words_v4(&witness, &stages, 2, [1, 1])
            .expect("serialize complete two-parent V4 audit");
        assert_eq!(two_parent[0], two_parent[1]);
        assert_eq!(
            two_parent[0],
            kagemusha_sha256_public_words(
                Sha256::digest(expected_bytes([1; 8], coefficients)).into()
            )
        );

        assert_eq!(
            kagemusha_deferred_audit_public_words_v4(&witness, &stages, 0, [0, 0])
                .expect("serialize absent V4 slots"),
            [[0; 8]; 2]
        );
    }

    fn v4_reciprocal_audit_builder<C>(
        witness: &crate::zk::kagemusha_cycle_loader::DeferredEquationWitness<C>,
        stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
        current_parent_count: u32,
        expected_words: [[u32; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::Base>
    where
        C: halo2_base::utils::CurveAffineExt,
        C::Base: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
        C::ScalarExt: halo2_base::utils::BigPrimeField,
    {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_ecc::fields::fp::FpChip;

        use crate::zk::kagemusha_cycle_loader::{LIMB_BITS, LIMBS};

        let mut builder = BaseCircuitBuilder::<C::Base>::new(false)
            .use_k(17)
            .use_lookup_bits(16);
        let range = builder.range_chip();
        let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
        let mut ctx = mem::take(builder.pool(0));
        let current_parent_count = ctx
            .main()
            .load_witness(C::Base::from(u64::from(current_parent_count)));
        let parent_counts = [
            ctx.main().load_witness(C::Base::ZERO),
            ctx.main().load_witness(C::Base::ZERO),
        ];
        let expected_words = expected_words
            .map(|words| words.map(|word| ctx.main().load_witness(C::Base::from(u64::from(word)))));
        constrain_reciprocal_point_audit_identity_v4::<C>(
            &mut ctx,
            &base,
            &scalar,
            witness,
            stages,
            current_parent_count,
            parent_counts,
            [&expected_words[0], &expected_words[1]],
        )
        .expect("complete V4 reciprocal audit shape");
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
    }

    #[test]
    fn v4_one_parent_branch_select_reciprocal_substitution_fails_for_both_parities() {
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::{
                group::prime::PrimeCurveAffine as _,
                pasta::{EpAffine, EqAffine},
            },
        };

        use crate::zk::kagemusha_cycle_loader::DeferredEquationWitness;

        fn assert_join<C>(source: C)
        where
            C: halo2_base::utils::CurveAffineExt,
            C::Base: halo2_base::utils::BigPrimeField + halo2_base::utils::ScalarField,
            C::ScalarExt: halo2_base::utils::BigPrimeField,
        {
            let stages = v4_complete_stage_plan();
            let original = DeferredEquationWitness::<C> {
                sources: vec![source],
                equations: vec![vec![(0, C::ScalarExt::ZERO)]; 8],
            };
            let expected = kagemusha_deferred_audit_public_words_v4(&original, &stages, 1, [0, 0])
                .expect("serialize original one-parent audit");
            assert_ne!(expected[0], [0; 8]);
            assert_eq!(expected[1], [0; 8]);

            let valid = v4_reciprocal_audit_builder(&original, &stages, 1, expected);
            MockProver::run(valid.config_params.k as u32, &valid, vec![])
                .expect("valid complete reciprocal audit prover")
                .assert_satisfied();

            let mut wrong_absent_slot = expected;
            wrong_absent_slot[1] = expected[0];
            let wrong_absent_slot =
                v4_reciprocal_audit_builder(&original, &stages, 1, wrong_absent_slot);
            assert!(
                MockProver::run(
                    wrong_absent_slot.config_params.k as u32,
                    &wrong_absent_slot,
                    vec![],
                )
                .expect("non-canonical one-parent reciprocal audit prover")
                .verify()
                .is_err(),
                "a one-parent step must expose canonical zero in slot one"
            );

            let two_parent =
                kagemusha_deferred_audit_public_words_v4(&original, &stages, 2, [0, 0])
                    .expect("serialize original two-parent audit");
            assert_ne!(two_parent[0], [0; 8]);
            assert_eq!(two_parent[0], two_parent[1]);
            let valid_two_parent = v4_reciprocal_audit_builder(&original, &stages, 2, two_parent);
            MockProver::run(
                valid_two_parent.config_params.k as u32,
                &valid_two_parent,
                vec![],
            )
            .expect("valid two-parent reciprocal audit prover")
            .assert_satisfied();

            let mut wrong_second_digest = two_parent;
            wrong_second_digest[1] = [0; 8];
            let wrong_second_digest =
                v4_reciprocal_audit_builder(&original, &stages, 2, wrong_second_digest);
            assert!(
                MockProver::run(
                    wrong_second_digest.config_params.k as u32,
                    &wrong_second_digest,
                    vec![],
                )
                .expect("mismatched two-parent reciprocal audit prover")
                .verify()
                .is_err(),
                "both present parent slots must expose the same complete digest"
            );

            let mut substituted = original;
            substituted.sources.push(-source);
            substituted.equations[7] = vec![(0, C::ScalarExt::ONE), (1, C::ScalarExt::ONE)];
            let adversarial = v4_reciprocal_audit_builder(&substituted, &stages, 1, expected);
            assert!(
                MockProver::run(adversarial.config_params.k as u32, &adversarial, vec![])
                    .expect("adversarial complete reciprocal audit prover")
                    .verify()
                    .is_err(),
                "a satisfiable BranchSelect substitution must fail the scalar-audit join"
            );
        }

        assert_join(EqAffine::generator());
        assert_join(EpAffine::generator());
    }

    fn v4_accumulator(
        parity: KagemushaPastaCycleParityV1,
        k: u32,
    ) -> KagemushaIpaAccumulatorWireV4 {
        use halo2_proofs::halo2curves::{
            group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
            pasta::{EpAffine, EqAffine},
        };

        let folded_generator = match parity {
            KagemushaPastaCycleParityV1::StepEq => {
                let mut bytes = [0; 32];
                bytes.copy_from_slice(EqAffine::generator().to_bytes().as_ref());
                bytes
            }
            KagemushaPastaCycleParityV1::StepEp => {
                let mut bytes = [0; 32];
                bytes.copy_from_slice(EpAffine::generator().to_bytes().as_ref());
                bytes
            }
        };
        KagemushaIpaAccumulatorWireV4 {
            version: crate::zk::kagemusha_accumulation::KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
            round_count: k,
            round_challenges: vec![[0; 32]; usize::try_from(k).expect("test degree fits")],
            folded_generator,
        }
    }

    fn v4_fold(k: u32, tag: u8, has_parent: bool) -> KagemushaIpaAccumulationProofV4 {
        if !has_parent {
            return KagemushaIpaAccumulationProofV4::initialization(k)
                .expect("supported initialization degree");
        }
        let len = crate::zk::kagemusha_accumulation::kagemusha_ipa_accumulation_proof_bytes_v4(k)
            .expect("supported fold degree");
        KagemushaIpaAccumulationProofV4::from_fold_bytes(k, vec![tag; len])
            .expect("fixed-size fold fixture")
    }

    fn v4_public_inputs(step: u32, parent_count: u32) -> KagemushaPastaCyclePublicInputsV4 {
        assert!((1..=3).contains(&step));
        assert!(parent_count <= 2);
        let mut parent_states = std::array::from_fn(|_| {
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2]
        });
        let mut parent_eq_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let mut parent_ep_deferred_sha256 = [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1];
        let eq_deferred_sha256 = std::array::from_fn(|index| 0xE410_0000 | index as u32 + 1);
        let ep_deferred_sha256 = std::array::from_fn(|index| 0xE420_0000 | index as u32 + 1);
        for slot in 0..usize::try_from(parent_count).expect("parent count fits") {
            parent_states[slot] =
                exact_state(step - parent_count + u32::try_from(slot).expect("slot fits"));
            parent_eq_deferred_sha256[slot] = eq_deferred_sha256;
            parent_ep_deferred_sha256[slot] = ep_deferred_sha256;
        }
        let has_parent = parent_count != 0;
        KagemushaPastaCyclePublicInputsV4 {
            public_statement_digest: std::array::from_fn(|index| {
                0xA410_0000 | step << 8 | index as u32 + 1
            }),
            operation: KagemushaStepOperationVectorV4::default(),
            parent_count,
            parent_states,
            result_state: exact_state(step),
            manifest_sha256: std::array::from_fn(|index| 0xA500_0000 | index as u32 + 1),
            step_eq_compiled_protocol_sha256: [0xC1C1_C1C1; 8],
            step_ep_compiled_protocol_sha256: [0xC2C2_C2C2; 8],
            parent_eq_lineage_accumulator: has_parent
                .then(|| v4_accumulator(KagemushaPastaCycleParityV1::StepEq, 20)),
            parent_ep_lineage_accumulator: has_parent
                .then(|| v4_accumulator(KagemushaPastaCycleParityV1::StepEp, 20)),
            parent_eq_deferred_sha256,
            parent_ep_deferred_sha256,
            live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
        }
    }

    fn v4_pair(step: u32, parent_count: u32) -> KagemushaPastaCycleProofPairV4 {
        let params = valid_step_circuit_params_v4();
        let has_parent = parent_count != 0;
        KagemushaPastaCycleProofPairV4 {
            version: KAGEMUSHA_PASTA_PROOF_PAIR_VERSION_V4,
            proof_step_count: step,
            public_inputs: v4_public_inputs(step, parent_count),
            step_eq_proof_bytes: vec![0x41; params.max_parent_proof_bytes as usize],
            step_ep_proof_bytes: vec![0x42; params.max_parent_proof_bytes as usize],
            step_eq_accumulation_proof: v4_fold(params.k, 0xE1, has_parent),
            step_ep_accumulation_proof: v4_fold(params.k, 0xE2, has_parent),
        }
    }

    #[test]
    fn v4_manifest_preserves_exact_little_endian_state_limbs() {
        let params = valid_step_circuit_params_v4();
        let expected = std::array::from_fn(|index| 0xA500_0000 | index as u32 + 1);
        let mut manifest_bytes = [0_u8; 32];
        for (chunk, limb) in manifest_bytes.chunks_exact_mut(4).zip(expected) {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }

        let exact = kagemusha_exact_u32_public_limbs(manifest_bytes);
        assert_eq!(exact, expected);
        assert_ne!(exact, kagemusha_sha256_public_words(manifest_bytes));

        let mut public_inputs = v4_public_inputs(1, 0);
        public_inputs.manifest_sha256 = exact;
        public_inputs
            .validate(1, &params)
            .expect("exact manifest limbs match the result-state binding");

        public_inputs.manifest_sha256 = kagemusha_sha256_public_words(manifest_bytes);
        assert!(public_inputs.validate(1, &params).is_err());
    }

    #[test]
    fn v4_eq_and_ep_public_columns_share_the_v2_result_frontier_limb() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let params = valid_step_circuit_params_v4();
        let mut public_inputs = v4_public_inputs(1, 0);
        public_inputs.result_state[crate::zk::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX] = 37;
        let eq = public_inputs
            .instance_column::<Fp>(1, &params)
            .expect("Eq public column");
        let ep = public_inputs
            .instance_column::<Fq>(1, &params)
            .expect("Ep public column");
        let offset = KAGEMUSHA_PASTA_RESULT_STATE_OFFSET_V4
            + crate::zk::kagemusha_v2::S_NEXT_ZERO_LEAF_INDEX;
        assert_eq!(eq[offset], Fp::from(37));
        assert_eq!(ep[offset], Fq::from(37));
    }

    #[test]
    fn v4_public_boundary_rejects_non_live_and_bootstrap_pairs() {
        let params = valid_step_circuit_params_v4();
        let mut selector_two = v4_public_inputs(1, 0);
        selector_two.live_selector = 2;
        assert!(selector_two.validate(1, &params).is_err());

        let mut bootstrap = v4_public_inputs(1, 0);
        bootstrap.live_selector = KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4;
        assert!(bootstrap.validate(1, &params).is_err());

        for selector in [KAGEMUSHA_PASTA_PUBLIC_BOOTSTRAP_SELECTOR_V4, 2] {
            let mut pair = v4_pair(1, 0);
            pair.public_inputs.live_selector = selector;
            assert!(pair.validate(&params, &params, 64_000).is_err());
            let encoded = to_bytes(&pair).expect("encode adversarial V4 pair");
            assert!(
                validate_kagemusha_proof_pair_measurement_v4(&encoded, &params, &params, 64_000,)
                    .is_err(),
                "the public opaque-pair parser must reject selector {selector}"
            );
        }
    }

    #[test]
    fn v4_audit_derivation_prepass_accepts_only_blank_derived_join_slots() {
        let params = valid_step_circuit_params_v4();
        let mut public_inputs = v4_public_inputs(2, 1);
        public_inputs
            .validate(2, &params)
            .expect("proof inputs require authenticated deferred-audit joins");
        assert!(
            public_inputs
                .validate_for_audit_derivation_prepass(2, &params)
                .is_err(),
            "audit derivation prepass must reject a preselected join digest"
        );

        public_inputs.parent_eq_deferred_sha256[0] = [0; 8];
        public_inputs.parent_ep_deferred_sha256[0] = [0; 8];
        public_inputs
            .validate_for_audit_derivation_prepass(2, &params)
            .expect("audit derivation prepass accepts a blank derived-join parent slot");
        assert!(
            public_inputs.validate(2, &params).is_err(),
            "a live proof must require every derived parent audit join"
        );
    }

    #[test]
    fn v4_circuit_mode_rejects_selector_two_nonzero_bootstrap_and_live_all_zero() {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_proofs::dev::MockProver;

        fn builder(
            mode: KagemushaStepPublicModeV4,
        ) -> (BaseCircuitBuilder<Fp>, Vec<Fp>, u32, usize) {
            let layout =
                KagemushaPastaPublicLayoutV4::for_ipa_round_count(20).expect("test public layout");
            let public_len = usize::try_from(layout.instance_column_limbs)
                .expect("test public length fits usize");
            let live_offset =
                usize::try_from(layout.live_selector_offset).expect("test live offset fits usize");
            let mut semantic = vec![Fp::ZERO; public_len];
            semantic[0] = Fp::from(7);
            semantic[live_offset] = Fp::ONE;
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(17)
                .use_lookup_bits(8)
                .use_instance_columns(1);
            assign_kagemusha_public_mode_v4(&mut builder, semantic.clone(), &layout, mode)
                .expect("assign test V4 public mode");
            let params = builder.calculate_params(Some(8));
            (
                builder,
                semantic,
                u32::try_from(params.k).expect("small k"),
                live_offset,
            )
        }

        let (bootstrap, _, bootstrap_k, live_offset) =
            builder(KagemushaStepPublicModeV4::Bootstrap);
        let mut zero = vec![Fp::ZERO; live_offset + 1];
        MockProver::run(bootstrap_k, &bootstrap, vec![zero.clone()])
            .expect("bootstrap public-mode prover")
            .assert_satisfied();

        zero[live_offset] = Fp::from(2);
        assert!(
            MockProver::run(bootstrap_k, &bootstrap, vec![zero.clone()])
                .expect("selector-two public-mode prover")
                .verify()
                .is_err()
        );
        zero[live_offset] = Fp::ZERO;
        zero[0] = Fp::ONE;
        assert!(
            MockProver::run(bootstrap_k, &bootstrap, vec![zero])
                .expect("nonzero-bootstrap public-mode prover")
                .verify()
                .is_err()
        );

        let (live, live_instance, live_k, _) = builder(KagemushaStepPublicModeV4::Live);
        MockProver::run(live_k, &live, vec![live_instance])
            .expect("live public-mode prover")
            .assert_satisfied();
        assert!(
            MockProver::run(live_k, &live, vec![vec![Fp::ZERO; live_offset + 1]])
                .expect("live-all-zero public-mode prover")
                .verify()
                .is_err()
        );
    }

    fn v4_bootstrap() -> KagemushaStepBootstrapV4 {
        let params = valid_step_circuit_params_v4();
        let layout = params.validate().expect("valid V4 params");
        KagemushaStepBootstrapV4 {
            version: KAGEMUSHA_STEP_BOOTSTRAP_VERSION_V4,
            parity: KagemushaPastaCycleParityV1::StepEq,
            circuit_params_sha256: params.sha256().expect("identify V4 params"),
            compiled_protocol_structure_sha256: [0x51; 32],
            bootstrap_compiled_protocol_sha256: [0x52; 32],
            parent_slot: KagemushaStepBootstrapParentSlotV4 {
                instances: vec![vec![
                    0;
                    usize::try_from(layout.instance_column_limbs)
                        .expect("public length fits")
                ]],
                ordinary_proof_bytes: vec![0x53; params.max_parent_proof_bytes as usize],
                carried_lineage: v4_accumulator(KagemushaPastaCycleParityV1::StepEq, params.k),
                post_proof_fold: v4_fold(params.k, 0x54, true),
            },
            branch_merge_fold: v4_fold(params.k, 0x55, true),
        }
    }

    #[test]
    fn v4_bootstrap_is_canonical_manifest_independent_and_profile_bound() {
        let params = valid_step_circuit_params_v4();
        let structure = [0x51; 32];
        let bootstrap = v4_bootstrap();
        bootstrap
            .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure)
            .expect("valid manifest-independent bootstrap");
        let encoded = bootstrap
            .encode_authenticated(&params, KagemushaPastaCycleParityV1::StepEq, structure)
            .expect("encode bootstrap");
        assert_eq!(
            KagemushaStepBootstrapV4::decode_authenticated(
                &encoded,
                &params,
                KagemushaPastaCycleParityV1::StepEq,
                structure,
            )
            .expect("decode canonical bootstrap"),
            bootstrap
        );

        for mutation in [
            "version",
            "parity",
            "params_hash",
            "structure",
            "bootstrap_identity",
            "nonzero_instance",
            "short_proof",
            "long_proof",
            "parent_fold",
            "branch_fold",
        ] {
            let mut candidate = bootstrap.clone();
            match mutation {
                "version" => candidate.version ^= 1,
                "parity" => candidate.parity = KagemushaPastaCycleParityV1::StepEp,
                "params_hash" => candidate.circuit_params_sha256[0] ^= 1,
                "structure" => candidate.compiled_protocol_structure_sha256[0] ^= 1,
                "bootstrap_identity" => candidate.bootstrap_compiled_protocol_sha256 = [0; 32],
                "nonzero_instance" => candidate.parent_slot.instances[0][0] = 1,
                "short_proof" => {
                    candidate.parent_slot.ordinary_proof_bytes.pop();
                }
                "long_proof" => candidate.parent_slot.ordinary_proof_bytes.push(0),
                "parent_fold" => {
                    candidate.parent_slot.post_proof_fold.bytes.pop();
                }
                "branch_fold" => {
                    candidate.branch_merge_fold.bytes.pop();
                }
                _ => unreachable!(),
            }
            assert!(
                candidate
                    .validate(&params, KagemushaPastaCycleParityV1::StepEq, structure)
                    .is_err(),
                "bootstrap mutation {mutation} must fail"
            );
        }

        let wrong_profile = valid_step_circuit_params_for_k_v4(21);
        assert!(
            bootstrap
                .validate(
                    &wrong_profile,
                    KagemushaPastaCycleParityV1::StepEq,
                    structure,
                )
                .is_err()
        );
    }

    #[test]
    fn v4_pair_enforces_zero_one_two_parent_shapes_and_exact_bounds() {
        let params = valid_step_circuit_params_v4();
        let maximum = 1_000_000;
        for (step, parent_count) in [(1, 0), (2, 1), (3, 2)] {
            let pair = v4_pair(step, parent_count);
            let layout = pair
                .validate(&params, &params, maximum)
                .expect("valid V4 selector shape");
            assert_eq!(
                pair.public_inputs
                    .instance_column::<Fp>(step, &params)
                    .expect("V4 instance column")
                    .len(),
                usize::try_from(layout.instance_column_limbs).expect("public length fits")
            );
            let bytes = pair
                .encode_authenticated(&params, &params, maximum)
                .expect("encode bounded pair");
            assert_eq!(
                KagemushaPastaCycleProofPairV4::decode_authenticated(
                    &bytes, &params, &params, maximum,
                )
                .expect("decode canonical pair"),
                pair
            );
            assert!(
                pair.validate(
                    &params,
                    &params,
                    u32::try_from(bytes.len() - 1).expect("fixture size fits"),
                )
                .is_err(),
                "pair cap below the canonical payload must fail"
            );
        }

        let mut invalid_count = v4_pair(3, 2);
        invalid_count.public_inputs.parent_count = 3;
        assert!(invalid_count.validate(&params, &params, maximum).is_err());

        let mut bundle_ordered = v4_pair(3, 2);
        assert!(
            bundle_ordered.public_inputs.parent_states[0]
                < bundle_ordered.public_inputs.parent_states[1]
        );
        bundle_ordered.public_inputs.parent_states.swap(0, 1);
        bundle_ordered
            .public_inputs
            .parent_eq_deferred_sha256
            .swap(0, 1);
        bundle_ordered
            .public_inputs
            .parent_ep_deferred_sha256
            .swap(0, 1);
        assert!(
            bundle_ordered.public_inputs.parent_states[0]
                > bundle_ordered.public_inputs.parent_states[1]
        );
        bundle_ordered
            .validate(&params, &params, maximum)
            .expect("V4 parent slots follow bundle-digest order, not state-vector order");

        let mut short = v4_pair(2, 1);
        short.step_eq_proof_bytes.pop();
        assert!(short.validate(&params, &params, maximum).is_err());
        let mut long = v4_pair(2, 1);
        long.step_ep_proof_bytes.push(0);
        assert!(long.validate(&params, &params, maximum).is_err());

        let wrong_layout = valid_step_circuit_params_for_k_v4(21);
        assert!(
            v4_pair(1, 0)
                .validate(&params, &wrong_layout, maximum)
                .is_err()
        );
        let mut wrong_manifest = v4_pair(2, 1);
        wrong_manifest.public_inputs.manifest_sha256[0] ^= 1;
        assert!(wrong_manifest.validate(&params, &params, maximum).is_err());
    }

    #[test]
    fn v4_missing_bootstrap_rejects_without_generating_padding() {
        assert!(require_kagemusha_step_bootstrap_v4(None, "Eq").is_err());
        assert!(require_kagemusha_step_bootstrap_v4(None, "Ep").is_err());
    }

    /// Keep the exact same-field Pasta recursion tuples executable.
    ///
    /// An Eq IPA proof uses `ParamsIPA<EqAffine>` and has scalar field `Fp`, so
    /// its direct Axiom circuit verifier must also be an `Fp` circuit with a
    /// `Halo2Loader<EqAffine, BaseFieldEccChip<EqAffine>>`. The reciprocal Ep
    /// tuple is `ParamsIPA<EpAffine>` / `Fq` /
    /// `Halo2Loader<EpAffine, BaseFieldEccChip<EpAffine>>`. This test is a
    /// compile-time guard against accidentally diagnosing that supported path
    /// as a Pasta trait mismatch.
    #[test]
    fn same_field_pasta_loader_type_tuples_compile() {
        use halo2_base::gates::circuit::{BaseCircuitParams, builder::BaseCircuitBuilder};
        use halo2_ecc::{ecc::BaseFieldEccChip, fields::fp::FpChip};
        use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine};
        use snark_verifier::loader::halo2::Halo2Loader;

        const LIMB_BITS: usize = 86;
        const LIMBS: usize = 3;
        let seed = BaseCircuitParams {
            k: 12,
            num_advice_per_phase: vec![1],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(11),
            num_instance_columns: 1,
        };

        let mut eq_outer = BaseCircuitBuilder::<Fp>::new(false).use_params(seed.clone());
        let eq_range = eq_outer.range_chip();
        let eq_base = FpChip::<Fp, Fq>::new(&eq_range, LIMB_BITS, LIMBS);
        let eq_loader = Halo2Loader::new(
            BaseFieldEccChip::<EqAffine>::new(&eq_base),
            mem::take(eq_outer.pool(0)),
        );
        fn require_eq_tuple(_: &Rc<Halo2Loader<EqAffine, BaseFieldEccChip<'_, EqAffine>>>) {}
        require_eq_tuple(&eq_loader);
        *eq_outer.pool(0) = eq_loader.take_ctx();

        let mut ep_outer = BaseCircuitBuilder::<Fq>::new(false).use_params(seed);
        let ep_range = ep_outer.range_chip();
        let ep_base = FpChip::<Fq, Fp>::new(&ep_range, LIMB_BITS, LIMBS);
        let ep_loader = Halo2Loader::new(
            BaseFieldEccChip::<EpAffine>::new(&ep_base),
            mem::take(ep_outer.pool(0)),
        );
        fn require_ep_tuple(_: &Rc<Halo2Loader<EpAffine, BaseFieldEccChip<'_, EpAffine>>>) {}
        require_ep_tuple(&ep_loader);
        *ep_outer.pool(0) = ep_loader.take_ctx();
    }

    #[test]
    fn protocol_private_enum_projection_is_explicit_and_fail_closed() {
        use ciborium::value::Value;

        assert_eq!(
            encode_common_polynomial_value(Value::Text("Identity".to_owned()))
                .expect("identity common polynomial"),
            vec![1, 0]
        );
        let mut expected_lagrange = vec![1, 1];
        expected_lagrange.extend_from_slice(&(-7_i32).to_le_bytes());
        assert_eq!(
            encode_common_polynomial_value(Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Integer((-7_i64).into()),
            )]))
            .expect("Lagrange common polynomial"),
            expected_lagrange
        );
        for malformed in [
            Value::Text("Unknown".to_owned()),
            Value::Map(Vec::new()),
            Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Text("zero".to_owned()),
            )]),
            Value::Map(vec![(
                Value::Text("Unknown".to_owned()),
                Value::Integer(0.into()),
            )]),
            Value::Map(vec![(
                Value::Text("Lagrange".to_owned()),
                Value::Integer(i64::MAX.into()),
            )]),
        ] {
            assert!(encode_common_polynomial_value(malformed).is_err());
        }

        assert_eq!(encode_linearization_value(Value::Null), Ok(0));
        assert_eq!(
            encode_linearization_value(Value::Text("WithoutConstant".to_owned())),
            Ok(1)
        );
        assert_eq!(
            encode_linearization_value(Value::Text("MinusVanishingTimesQuotient".to_owned())),
            Ok(2)
        );
        assert!(
            encode_linearization_value(Value::Text("Unknown".to_owned())).is_err(),
            "an upstream enum extension requires an identity-version review"
        );
    }

    #[test]
    fn universal_protocol_bootstrap_converges_for_the_same_base_config() {
        use halo2_base::gates::{GateInstructions as _, RangeInstructions as _};
        use halo2_proofs::{
            halo2curves::pasta::EqAffine,
            plonk::{keygen_pk, keygen_vk},
            poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
        };
        use snark_verifier::system::halo2::{Config, compile};

        let base_circuit_params = halo2_base::gates::circuit::BaseCircuitParams {
            k: 8,
            num_advice_per_phase: vec![2],
            num_lookup_advice_per_phase: vec![1],
            num_fixed: 1,
            lookup_bits: Some(7),
            num_instance_columns: 1,
        };
        let target = KagemushaUniversalProtocolTargetV1 {
            base_circuit_params: base_circuit_params.clone(),
            instance_column_lengths: vec![1],
        };
        let params = ParamsIPA::<EqAffine>::new(8);
        let bootstrap = kagemusha_bootstrap_compiled_protocol_v1(&params, &target)
            .expect("deterministic bootstrap protocol");
        assert_eq!(bootstrap.num_instance, vec![1]);
        assert!(
            bootstrap.instance_committing_key.is_none(),
            "canonical V4 compilation must evaluate public instances directly"
        );
        let bootstrap_structure = kagemusha_compiled_protocol_structure_sha256(
            &bootstrap,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("canonical bootstrap structure");
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("repeat canonical bootstrap structure"),
            "the explicit protocol descriptor must be stable"
        );
        assert_ne!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEp,
            )
            .expect("opposite-parity protocol descriptor"),
            "the same protocol bytes must remain parity-domain-separated"
        );

        let assert_structure_changes = |label: &str, protocol: &PlonkProtocol<EqAffine>| {
            assert_ne!(
                bootstrap_structure,
                kagemusha_compiled_protocol_structure_sha256(
                    protocol,
                    KagemushaPastaCycleParityV1::StepEq,
                )
                .expect("mutated protocol structure"),
                "the {label} verifier-control-flow category must affect the descriptor"
            );
        };

        let mut changed_domain = bootstrap.clone();
        changed_domain.domain.k += 1;
        assert_structure_changes("domain", &changed_domain);

        let mut changed_instance_count = bootstrap.clone();
        changed_instance_count.num_instance.push(0);
        assert_structure_changes("instance count", &changed_instance_count);
        let mut changed_witness_count = bootstrap.clone();
        changed_witness_count.num_witness.push(1);
        assert_structure_changes("witness count", &changed_witness_count);
        let mut changed_challenge_count = bootstrap.clone();
        changed_challenge_count.num_challenge.push(1);
        assert_structure_changes("challenge count", &changed_challenge_count);

        let mut changed_evaluations = bootstrap.clone();
        changed_evaluations
            .evaluations
            .first_mut()
            .expect("compiled protocol has an evaluation")
            .poly += 1;
        assert_structure_changes("evaluation", &changed_evaluations);
        let mut changed_queries = bootstrap.clone();
        changed_queries
            .queries
            .first_mut()
            .expect("compiled protocol has an opening query")
            .rotation
            .0 += 1;
        assert_structure_changes("opening query", &changed_queries);

        let mut changed_quotient = bootstrap.clone();
        changed_quotient.quotient.chunk_degree += 1;
        assert_structure_changes("quotient", &changed_quotient);

        let bootstrap_vk = kagemusha_bootstrap_verifying_key_v1(&params, &target)
            .expect("deterministic bootstrap verifying key");
        let queried_instance_protocol = compile(
            &params,
            &bootstrap_vk,
            Config::ipa().with_num_instance(vec![1]),
        );
        assert!(
            queried_instance_protocol.instance_committing_key.is_some(),
            "the upstream IPA default remains queried-instance mode"
        );
        assert_structure_changes("queried-instance presence", &queried_instance_protocol);

        // `LinearizationStrategy` is intentionally not re-exported by the
        // pinned dependency. Its derived Ciborium representation still lets
        // this regression exercise the public protocol field without copying
        // the dependency's private enum into Iroha.
        let mut changed_linearization = bootstrap.clone();
        changed_linearization.linearization =
            ciborium::value::Value::Text("WithoutConstant".to_owned())
                .deserialized()
                .expect("deserialize explicit linearization variant");
        assert_structure_changes("linearization", &changed_linearization);

        let mut changed_accumulator_indices = bootstrap.clone();
        changed_accumulator_indices
            .accumulator_indices
            .push(vec![(0, 0)]);
        assert_structure_changes("accumulator indices", &changed_accumulator_indices);

        let mut changed_transcript_presence = bootstrap.clone();
        changed_transcript_presence.transcript_initial_state = None;
        assert_structure_changes("transcript presence", &changed_transcript_presence);

        let mut changed_preprocessed_length = bootstrap.clone();
        changed_preprocessed_length.preprocessed.pop();
        assert_structure_changes("preprocessed length", &changed_preprocessed_length);

        let bootstrap_identity = kagemusha_compiled_protocol_identity_sha256(
            &bootstrap,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("bootstrap identity");
        let mut changed_preprocessed_value = bootstrap.clone();
        changed_preprocessed_value.preprocessed[0] = EqAffine::identity();
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &changed_preprocessed_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("structure with changed preprocessed value"),
            "only preprocessed point values are scrubbed from the fixed descriptor"
        );

        let mut changed_transcript_value = bootstrap.clone();
        changed_transcript_value.transcript_initial_state = changed_transcript_value
            .transcript_initial_state
            .map(|state| state + Fp::ONE);
        assert_eq!(
            bootstrap_structure,
            kagemusha_compiled_protocol_structure_sha256(
                &changed_transcript_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("structure with changed transcript value"),
            "only the transcript-state value is scrubbed from the fixed descriptor"
        );
        assert_ne!(
            bootstrap_identity,
            kagemusha_compiled_protocol_identity_sha256(
                &changed_preprocessed_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("identity with changed preprocessed value"),
            "the complete identity must authenticate preprocessed point values"
        );
        assert_ne!(
            bootstrap_identity,
            kagemusha_compiled_protocol_identity_sha256(
                &changed_transcript_value,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("identity with changed transcript value"),
            "the complete identity must authenticate the transcript-state value"
        );

        let mut missing_transcript_state = bootstrap.clone();
        missing_transcript_state.transcript_initial_state = None;
        assert!(
            kagemusha_compiled_protocol_identity_sha256(
                &missing_transcript_state,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .is_err(),
            "a protocol without its authenticated transcript state must fail closed"
        );

        let mut final_builder =
            halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::new(false)
                .use_params(base_circuit_params.clone());
        let range = final_builder.range_chip();
        let public = {
            let ctx = final_builder.main(0);
            let lhs = ctx.load_witness(Fp::from(17));
            let rhs = ctx.load_witness(Fp::from(25));
            range.range_check(ctx, lhs, 8);
            range.range_check(ctx, rhs, 8);
            range.gate().add(ctx, lhs, rhs)
        };
        final_builder.assigned_instances = vec![vec![public]];
        let final_vk = keygen_vk(&params, &final_builder).expect("final universal BaseConfig VK");
        let captured_break_points = final_builder.break_points();
        assert_eq!(
            kagemusha_break_points_from_wire_v4(
                &kagemusha_break_points_to_wire_v4(&captured_break_points)
                    .expect("encode captured breakpoints")
            )
            .expect("decode captured breakpoints"),
            captured_break_points,
            "captured breakpoints must round-trip through the portable header width"
        );
        let final_protocol = compile(&params, &final_vk, kagemusha_ipa_compile_config_v4(1));
        assert!(
            final_protocol.instance_committing_key.is_none(),
            "final V4 compilation must evaluate public instances directly"
        );
        kagemusha_require_protocol_structure_v1(
            &bootstrap,
            &final_protocol,
            KagemushaPastaCycleParityV1::StepEq,
        )
        .expect("the universal target must converge in one pass");
        assert_ne!(
            kagemusha_compiled_protocol_identity_sha256(
                &bootstrap,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("bootstrap identity"),
            kagemusha_compiled_protocol_identity_sha256(
                &final_protocol,
                KagemushaPastaCycleParityV1::StepEq,
            )
            .expect("final identity"),
            "the static shape converges while dynamic VK values remain distinct"
        );

        let final_pk = keygen_pk(&params, final_vk.clone(), &final_builder)
            .expect("direct-instance test proving key");
        assert_eq!(
            final_builder.break_points(),
            captured_break_points,
            "PK synthesis must reproduce the VK layout"
        );
        let mut prover_builder =
            halo2_base::gates::circuit::builder::BaseCircuitBuilder::<Fp>::prover(
                base_circuit_params,
                captured_break_points,
            );
        let range = prover_builder.range_chip();
        let public = {
            let ctx = prover_builder.main(0);
            let lhs = ctx.load_witness(Fp::from(17));
            let rhs = ctx.load_witness(Fp::from(25));
            range.range_check(ctx, lhs, 8);
            range.range_check(ctx, rhs, 8);
            range.gate().add(ctx, lhs, rhs)
        };
        prover_builder.assigned_instances = vec![vec![public]];
        assert!(
            prover_builder.witness_gen_only(),
            "the proof circuit must use the witness-only prover stage"
        );
        let instances = vec![vec![Fp::from(42)]];
        let proof = create_augmented_eq_proof_v4(&params, &final_pk, prover_builder, &instances)
            .expect("direct-instance augmented proof");
        let decide = |candidate: &[Vec<Fp>]| -> Result<(), String> {
            let current = succinct_verify_step_eq_instances(
                &params,
                &final_vk,
                &proof,
                candidate,
                proof.len(),
            )?;
            let initialization = KagemushaIpaAccumulationProofV4::initialization(8)?;
            crate::zk::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
                &params,
                8,
                current,
                None,
                &initialization,
            )
            .map(|_| ())
        };
        decide(&instances).expect("direct-instance IPA proof round-trip");
        assert!(
            decide(&[vec![Fp::from(43)]]).is_err(),
            "substituting a non-zero public instance must fail"
        );
    }

    fn exact_state(step: u32) -> Vec<u32> {
        let mut state =
            vec![0; iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
        state[0] =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
        state[1] = step;
        for (index, limb) in state.iter_mut().enumerate().skip(2) {
            *limb = step
                .wrapping_mul(1_003)
                .wrapping_add(u32::try_from(index).expect("state-vector index fits u32"));
        }
        let offset = |field: &str| {
            crate::zk::kagemusha_v2::KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_V2
                .iter()
                .find_map(|(name, start, _)| (*name == field).then_some(*start))
                .expect("state fixture field exists")
        };
        state[offset("proof_step_count")] = step;
        state[offset("peer_hop_count")] = step
            .saturating_sub(1)
            .min(iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_MAX_PEER_HOPS_V2);
        let manifest = offset("artifact_manifest_sha256");
        for (index, limb) in state[manifest..manifest + 8].iter_mut().enumerate() {
            *limb = 0xA500_0000 | u32::try_from(index + 1).expect("digest index fits u32");
        }
        state
    }

    fn constrained_sha_builder<F>(
        message: &[u8],
        k: usize,
    ) -> halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>
    where
        F: halo2_base::utils::BigPrimeField,
    {
        let mut builder = halo2_base::gates::circuit::builder::BaseCircuitBuilder::new(false)
            .use_k(k)
            .use_lookup_bits(k - 1)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let digest = {
            let ctx = builder.main(0);
            let bytes =
                ctx.assign_witnesses(message.iter().copied().map(|byte| F::from(u64::from(byte))));
            KagemushaSha256Chip::digest(ctx, &range, &bytes)
        };
        builder.assigned_instances = vec![digest.to_vec()];
        builder.calculate_params(Some(9));
        builder
    }

    fn sha256_words(message: &[u8]) -> [u32; 8] {
        let digest: [u8; 32] = Sha256::digest(message).into();
        std::array::from_fn(|index| {
            u32::from_be_bytes(
                digest[index * 4..index * 4 + 4]
                    .try_into()
                    .expect("SHA-256 word"),
            )
        })
    }

    #[test]
    fn constrained_sha256_matches_fips_and_padding_boundaries_in_both_pasta_fields() {
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::pasta::{Fp, Fq},
        };

        const K: usize = 20;
        fn check<F>()
        where
            F: halo2_base::utils::BigPrimeField,
        {
            for message in [
                Vec::new(),
                b"abc".to_vec(),
                vec![0x5A; 55],
                vec![0xA5; 56],
                vec![0x11; 63],
                vec![0x22; 64],
                vec![0x33; 65],
            ] {
                let expected = sha256_words(&message)
                    .into_iter()
                    .map(|word| F::from(u64::from(word)))
                    .collect::<Vec<_>>();
                let builder = constrained_sha_builder::<F>(&message, K);
                MockProver::run(K as u32, &builder, vec![expected])
                    .expect("constrained SHA-256 mock prover")
                    .assert_satisfied();
            }
        }

        assert_eq!(
            sha256_words(b""),
            [
                0xe3b0_c442,
                0x98fc_1c14,
                0x9afb_f4c8,
                0x996f_b924,
                0x27ae_41e4,
                0x649b_934c,
                0xa495_991b,
                0x7852_b855,
            ]
        );
        assert_eq!(
            sha256_words(b"abc"),
            [
                0xba78_16bf,
                0x8f01_cfea,
                0x4141_40de,
                0x5dae_2223,
                0xb003_61a3,
                0x9617_7a9c,
                0xb410_ff61,
                0xf200_15ad,
            ]
        );
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn constrained_sha256_rejects_message_and_digest_substitution() {
        use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp};

        const K: usize = 20;
        let expected = sha256_words(b"abc")
            .into_iter()
            .map(|word| Fp::from(u64::from(word)))
            .collect::<Vec<_>>();
        let substituted_message = constrained_sha_builder::<Fp>(b"abd", K);
        assert!(
            MockProver::run(K as u32, &substituted_message, vec![expected.clone()])
                .expect("message-substitution prover")
                .verify()
                .is_err()
        );

        let original = constrained_sha_builder::<Fp>(b"abc", K);
        let mut substituted_digest = expected;
        substituted_digest[7] += Fp::ONE;
        assert!(
            MockProver::run(K as u32, &original, vec![substituted_digest])
                .expect("digest-substitution prover")
                .verify()
                .is_err()
        );
    }

    #[test]
    fn split_deferred_equation_constrains_scalar_join_and_reciprocal_msm() {
        use std::mem;

        use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::ScalarField as _};
        use halo2_ecc::fields::fp::FpChip;
        use halo2_proofs::{
            dev::MockProver,
            halo2curves::{
                group::Curve as _,
                pasta::{EqAffine, Fp, Fq},
            },
        };
        use snark_verifier::loader::halo2::{EccInstructions, IntegerInstructions};
        use snark_verifier::util::arithmetic::PrimeCurveAffine as _;

        use crate::zk::kagemusha_cycle_loader::{
            DeferredScalarEccChip, LIMB_BITS, LIMBS, PastaCycleEccChip,
        };

        const K: usize = 20;
        let generator = EqAffine::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();

        let mut scalar_builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(1);
        let scalar_range = scalar_builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&scalar_range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&scalar_range, LIMB_BITS, LIMBS);
        let mut scalar_chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let mut scalar_ctx = mem::take(scalar_builder.pool(0));
        let assigned_generator = scalar_chip.assign_point(&mut scalar_ctx, generator);
        let assigned_doubled = scalar_chip.assign_point(&mut scalar_ctx, doubled);
        let two = scalar_chip
            .scalar_chip()
            .assign_integer(&mut scalar_ctx, Fp::from(2));
        let minus_one = scalar_chip
            .scalar_chip()
            .assign_integer(&mut scalar_ctx, -Fp::ONE);
        let result = scalar_chip.variable_base_msm(
            &mut scalar_ctx,
            &[(&two, &assigned_generator), (&minus_one, &assigned_doubled)],
        );
        let identity = scalar_chip.assign_constant(&mut scalar_ctx, EqAffine::identity());
        scalar_chip.assert_equal(&mut scalar_ctx, &result, &identity);
        let equation_count = scalar_chip.equation_count();
        let gate_tags = vec![0x5644_0001; equation_count];
        let scalar_selectors = (0..equation_count)
            .map(|_| scalar_ctx.main().load_constant(Fp::ONE))
            .collect::<Vec<_>>();
        let scalar_join = scalar_chip
            .assigned_equation_bytes_v4(&mut scalar_ctx, &gate_tags, &scalar_selectors)
            .expect("selector-bound scalar audit bytes");
        let scalar_digest =
            KagemushaSha256Chip::digest(scalar_ctx.main(), &scalar_range, &scalar_join);
        let expected_words = sha256_words(
            &scalar_join
                .iter()
                .map(|byte| u8::try_from(byte.value().get_lower_64()).expect("assigned byte"))
                .collect::<Vec<_>>(),
        );
        let equation_witness = scalar_chip.witness();
        *scalar_builder.pool(0) = scalar_ctx;
        scalar_builder.assigned_instances = vec![scalar_digest.to_vec()];
        scalar_builder.calculate_params(Some(9));

        let scalar_instances = expected_words
            .into_iter()
            .map(|word| Fp::from(u64::from(word)))
            .collect::<Vec<_>>();
        MockProver::run(K as u32, &scalar_builder, vec![scalar_instances])
            .expect("deferred scalar-half mock prover")
            .assert_satisfied();

        let mut point_builder = BaseCircuitBuilder::<Fq>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(1);
        let point_range = point_builder.range_chip();
        let base = FpChip::<Fq, Fq>::new(&point_range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Fq, Fp>::new(&point_range, LIMB_BITS, LIMBS);
        let mut point_chip = PastaCycleEccChip::<EqAffine>::new(&base, &scalar);
        let mut point_ctx = mem::take(point_builder.pool(0));
        let point_selectors = (0..equation_count)
            .map(|_| point_ctx.main().load_constant(Fq::ONE))
            .collect::<Vec<_>>();
        let point_audit = point_chip
            .constrain_deferred_equations_with_selectors(
                &mut point_ctx,
                &equation_witness,
                &point_selectors,
            )
            .expect("canonical reciprocal point witness");
        let point_join = point_chip
            .assigned_equation_bytes_v4(&mut point_ctx, &point_audit, &gate_tags, &point_selectors)
            .expect("selector-bound point audit bytes");
        let point_digest = KagemushaSha256Chip::digest(point_ctx.main(), &point_range, &point_join);
        assert_eq!(
            scalar_join
                .iter()
                .map(|byte| byte.value().get_lower_64())
                .collect::<Vec<_>>(),
            point_join
                .iter()
                .map(|byte| byte.value().get_lower_64())
                .collect::<Vec<_>>(),
            "both constrained halves must hash the exact same bytes"
        );
        *point_builder.pool(0) = point_ctx;
        point_builder.assigned_instances = vec![point_digest.to_vec()];
        point_builder.calculate_params(Some(9));
        let point_instances = expected_words
            .into_iter()
            .map(|word| Fq::from(u64::from(word)))
            .collect::<Vec<_>>();
        MockProver::run(K as u32, &point_builder, vec![point_instances])
            .expect("deferred point-half mock prover")
            .assert_satisfied();

        let mut substituted = equation_witness;
        substituted.equations[0][0].1 += Fp::ONE;
        let mut rejected_builder = BaseCircuitBuilder::<Fq>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(1);
        let rejected_range = rejected_builder.range_chip();
        let rejected_base = FpChip::<Fq, Fq>::new(&rejected_range, LIMB_BITS, LIMBS);
        let rejected_scalar = FpChip::<Fq, Fp>::new(&rejected_range, LIMB_BITS, LIMBS);
        let mut rejected_chip =
            PastaCycleEccChip::<EqAffine>::new(&rejected_base, &rejected_scalar);
        let mut rejected_ctx = mem::take(rejected_builder.pool(0));
        let rejected_selectors = (0..equation_count)
            .map(|_| rejected_ctx.main().load_constant(Fq::ONE))
            .collect::<Vec<_>>();
        let rejected_audit = rejected_chip
            .constrain_deferred_equations_with_selectors(
                &mut rejected_ctx,
                &substituted,
                &rejected_selectors,
            )
            .expect("shape-preserving substituted witness");
        let rejected_join = rejected_chip
            .assigned_equation_bytes_v4(
                &mut rejected_ctx,
                &rejected_audit,
                &gate_tags,
                &rejected_selectors,
            )
            .expect("selector-bound substituted audit bytes");
        let rejected_digest =
            KagemushaSha256Chip::digest(rejected_ctx.main(), &rejected_range, &rejected_join);
        *rejected_builder.pool(0) = rejected_ctx;
        rejected_builder.assigned_instances = vec![rejected_digest.to_vec()];
        rejected_builder.calculate_params(Some(9));
        let expected_digest = expected_words
            .into_iter()
            .map(|word| Fq::from(u64::from(word)))
            .collect::<Vec<_>>();
        assert!(
            MockProver::run(K as u32, &rejected_builder, vec![expected_digest])
                .expect("substituted deferred point-half mock prover")
                .verify()
                .is_err(),
            "a coefficient substitution must fail both the MSM and the shared join"
        );
    }
}
