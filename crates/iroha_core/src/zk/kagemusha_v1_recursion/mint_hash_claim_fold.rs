//! Ordered recursive claim fold for KAGEMUSHA mint-hash shards.
//!
//! A one-block Table8 shard proves only one SHA-256 compression. It is never monetary authority
//! on its own. This module gives those leaves an order- and completeness-preserving state machine
//! and a sound bridge into the sole `k = 16` monetary history. The bridge prepends four constant
//! zero IPA challenges to a `k = 12` leaf accumulator. With the release-authenticated generator
//! prefix check, its 4,096 coefficients are therefore the first 4,096 coefficients of the
//! 65,536-generator monetary basis and every remaining coefficient is zero.
//!
//! Each recursive step folds both the predecessor claim proof and one lifted leaf proof into one
//! fixed 544-byte accumulator. Public claim state is constant-size regardless of leaf count. A
//! terminal claim is valid only after the exact typed-plan stage and job totals have been reached
//! and the ordered terminal-digest root equals the plan commitment.

use ff::{Field as _, PrimeField};
use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{
        GateInstructions as _, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::{
        CurveAffine,
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::{
        commitment::{Params as _, ParamsProver as _},
        ipa::commitment::ParamsIPA,
    },
};
use snark_verifier::{
    loader::{ScalarLoader as _, native::NativeLoader},
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

use super::{
    DigestV1, KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaPastaParityV1,
    deferred_parent::{
        DeferredAccumulator, DeferredLoader, DeferredScalar, KagemushaDeferredParentOutputV1,
        bind_accumulator_limbs, constrain_reciprocal_output_with_u128_binding_serialized_v1,
        deferred_field_chips_v1, deferred_loader_v1,
        finalize_deferred_audit_plan_with_u128_binding_v1, kagemusha_protocol_structure_digest_v1,
        load_and_constrain_parent_protocol_v1, load_native_accumulator, select_accumulator_v1,
        verify_fold, verify_ordinary_proof_with_canonical_bytes_at_k_v1,
        verify_ordinary_proof_with_canonical_bytes_v1,
    },
    mint_hash_shard::{
        KAGEMUSHA_MINT_HASH_SHARD_K_V1, KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1,
        KagemushaMintHashShardStatementV1, public_instance as shard_public,
    },
};
use crate::zk::{
    kagemusha_v1_poseidon::{
        KagemushaPoseidonChipV1, KagemushaPoseidonFieldV1, decode, digest_limbs, encode, from_u128,
        hash,
    },
    pasta_sha256_table8::{BLOCK_SIZE, DIGEST_SIZE, IV},
};

const PLAN_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhpln1");
const MESSAGE_SEED_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhmsg0");
const MESSAGE_STEP_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhmsg1");
const TERMINAL_SEED_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhseed");
const TERMINAL_STEP_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhjob1");
const PROOF_CHAIN_SEED_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhprf0");
const PROOF_CHAIN_STEP_DOMAIN_V1: u64 = u64::from_le_bytes(*b"kgmhprf1");
const CLAIM_PARENT_EQUATION_TAG_V1: u32 = 11;
const CLAIM_SHARD_EQUATION_TAG_V1: u32 = 12;
const MINIMUM_UNUSABLE_ROWS: usize = 9;
const SHARD_TO_HISTORY_ZERO_ROUNDS_V1: usize =
    (KAGEMUSHA_RECURSION_IPA_K_V1 - KAGEMUSHA_MINT_HASH_SHARD_K_V1) as usize;

/// Typed, parity-specific plan commitment consumed by every shard and claim step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimPlanV1 {
    pub(crate) parity: KagemushaPastaParityV1,
    pub(crate) release_id: DigestV1,
    pub(crate) total_stages: u64,
    pub(crate) total_jobs: u32,
    /// Expected ordered commitment to every canonically padded message block.
    pub(crate) expected_message_root: DigestV1,
    pub(crate) expected_terminal_root: DigestV1,
    pub(crate) plan_binding: DigestV1,
}

impl KagemushaMintHashClaimPlanV1 {
    /// Derive the complete plan commitment from every exact ordered compression leaf.
    ///
    /// `plan_binding` is deliberately ignored while deriving the commitment: callers first build
    /// provisional leaves, derive this plan, then rebuild the same leaves with the returned
    /// binding. Every position, padded block word and terminal state is committed here.
    pub(crate) fn from_leaves<F: KagemushaPoseidonFieldV1>(
        release_id: DigestV1,
        leaves: &[KagemushaMintHashShardStatementV1],
    ) -> Result<Self, String> {
        if release_id == [0; 32] || leaves.is_empty() {
            return Err("mint hash claim plan is empty or release-unbound".to_owned());
        }
        let total_stages = u64::try_from(leaves.len())
            .map_err(|_| "mint hash claim stage count exceeds u64".to_owned())?;
        let parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        let mut jobs = Vec::new();
        // The seed commits the final job count. Derive it first from exact terminal positions.
        let total_jobs = leaves
            .last()
            .and_then(|leaf| leaf.job_index.checked_add(1))
            .ok_or_else(|| "mint hash claim terminal job index overflowed".to_owned())?;
        let mut expected_message_root =
            message_seed_native::<F>(release_id, total_stages, total_jobs);
        let mut expected_stage = 0_u64;
        let mut expected_job = 0_u32;
        let mut expected_block = 0_u32;
        let mut active_blocks = 0_u32;
        let mut chaining_state = IV;
        for leaf in leaves {
            leaf.validate_shape()?;
            if leaf.parity != parity
                || leaf.release_id != release_id
                || leaf.stage_index != expected_stage
                || leaf.job_index != expected_job
                || leaf.block_index != expected_block
                || leaf.initial_state != chaining_state
                || (expected_block == 0 && active_blocks != 0)
                || (expected_block != 0 && active_blocks != leaf.job_block_count)
            {
                return Err("mint hash plan leaves are not one exact ordered job stream".to_owned());
            }
            expected_message_root = message_step_native::<F>(expected_message_root, leaf);
            expected_stage = expected_stage
                .checked_add(1)
                .ok_or_else(|| "mint hash claim stage count overflowed u64".to_owned())?;
            if leaf.is_final_block() {
                jobs.push((leaf.job_block_count, leaf.output_state));
                expected_job = expected_job
                    .checked_add(1)
                    .ok_or_else(|| "mint hash claim job count overflowed u32".to_owned())?;
                expected_block = 0;
                active_blocks = 0;
                chaining_state = IV;
            } else {
                expected_block = expected_block
                    .checked_add(1)
                    .ok_or_else(|| "mint hash claim block index overflowed u32".to_owned())?;
                active_blocks = leaf.job_block_count;
                chaining_state = leaf.output_state;
            }
        }
        if expected_stage != total_stages
            || expected_job != total_jobs
            || expected_block != 0
            || active_blocks != 0
            || chaining_state != IV
        {
            return Err("mint hash claim leaves end inside a job".to_owned());
        }
        let total_jobs = u32::try_from(jobs.len())
            .map_err(|_| "mint hash claim job count exceeds u32".to_owned())?;
        let counted_stages = jobs.iter().try_fold(0_u64, |total, (blocks, _)| {
            if *blocks == 0 {
                return Err("mint hash claim contains a zero-block job".to_owned());
            }
            total
                .checked_add(u64::from(*blocks))
                .ok_or_else(|| "mint hash claim stage count overflowed u64".to_owned())
        })?;
        if counted_stages != total_stages {
            return Err("mint hash claim job block counts do not sum to total stages".to_owned());
        }
        let mut root = terminal_seed_native::<F>(release_id, total_stages, total_jobs);
        for (job_index, (block_count, terminal)) in jobs.iter().enumerate() {
            root = terminal_step_native::<F>(
                root,
                u32::try_from(job_index).expect("bounded terminal index"),
                *block_count,
                *terminal,
            );
        }
        let expected_terminal_root = encode(root);
        let expected_message_root = encode(expected_message_root);
        let plan_binding = encode(plan_binding_native::<F>(
            release_id,
            total_stages,
            total_jobs,
            decode::<F>(expected_message_root).expect("fresh message root is canonical"),
            root,
        ));
        Ok(Self {
            parity,
            release_id,
            total_stages,
            total_jobs,
            expected_message_root,
            expected_terminal_root,
            plan_binding,
        })
    }

    /// Rebuild a plan from an independently constrained message root and exact job terminals.
    ///
    /// The monetary consumer uses this form after it has committed its own canonical padded
    /// message words in-circuit. A host-provided message root alone is not monetary authority.
    pub(crate) fn from_job_terminals_and_message_root<F: KagemushaPoseidonFieldV1>(
        release_id: DigestV1,
        total_stages: u64,
        jobs: &[(u32, [u32; DIGEST_SIZE])],
        expected_message_root: DigestV1,
    ) -> Result<Self, String> {
        if release_id == [0; 32]
            || total_stages == 0
            || jobs.is_empty()
            || decode::<F>(expected_message_root).is_none()
        {
            return Err("mint hash claim plan is empty or message/release-unbound".to_owned());
        }
        let total_jobs = u32::try_from(jobs.len())
            .map_err(|_| "mint hash claim job count exceeds u32".to_owned())?;
        let counted_stages = jobs.iter().try_fold(0_u64, |total, (blocks, _)| {
            if *blocks == 0 {
                return Err("mint hash claim contains a zero-block job".to_owned());
            }
            total
                .checked_add(u64::from(*blocks))
                .ok_or_else(|| "mint hash claim stage count overflowed u64".to_owned())
        })?;
        if counted_stages != total_stages {
            return Err("mint hash claim job block counts do not sum to total stages".to_owned());
        }
        let parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        let mut terminal = terminal_seed_native::<F>(release_id, total_stages, total_jobs);
        for (job_index, (block_count, output)) in jobs.iter().enumerate() {
            terminal = terminal_step_native::<F>(
                terminal,
                u32::try_from(job_index).expect("bounded terminal index"),
                *block_count,
                *output,
            );
        }
        let message = decode::<F>(expected_message_root).expect("checked canonical message root");
        Ok(Self {
            parity,
            release_id,
            total_stages,
            total_jobs,
            expected_message_root,
            expected_terminal_root: encode(terminal),
            plan_binding: encode(plan_binding_native::<F>(
                release_id,
                total_stages,
                total_jobs,
                message,
                terminal,
            )),
        })
    }

    fn validate<F: KagemushaPoseidonFieldV1>(&self) -> Result<(), String> {
        let expected_parity = if F::IS_EQ_PARITY {
            KagemushaPastaParityV1::Eq
        } else {
            KagemushaPastaParityV1::Ep
        };
        if self.parity != expected_parity
            || self.release_id == [0; 32]
            || self.total_stages == 0
            || self.total_jobs == 0
            || u64::from(self.total_jobs) > self.total_stages
        {
            return Err("mint hash claim plan shape is invalid".to_owned());
        }
        let message = decode::<F>(self.expected_message_root)
            .ok_or_else(|| "mint hash message root is not a canonical scalar".to_owned())?;
        let terminal = decode::<F>(self.expected_terminal_root)
            .ok_or_else(|| "mint hash terminal root is not a canonical scalar".to_owned())?;
        let expected = encode(plan_binding_native::<F>(
            self.release_id,
            self.total_stages,
            self.total_jobs,
            message,
            terminal,
        ));
        if self.plan_binding != expected {
            return Err("mint hash typed-plan binding does not match its totals/root".to_owned());
        }
        Ok(())
    }
}

/// Constant-size public progress claimed after consuming one or more shard proofs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimStateV1 {
    pub(crate) plan: KagemushaMintHashClaimPlanV1,
    /// Number of leaves consumed; also the exact next global stage index.
    pub(crate) next_stage: u64,
    /// Exact next job index.
    pub(crate) next_job: u32,
    /// Exact next block within `next_job`, or zero at a job boundary.
    pub(crate) next_block: u32,
    /// Fixed block count of the active job, or zero at a job boundary.
    pub(crate) active_job_blocks: u32,
    /// SHA chaining state for the active job; exactly IV at a job boundary.
    pub(crate) chaining_state: [u32; DIGEST_SIZE],
    /// Ordered field-native fold of every completed job digest.
    pub(crate) terminal_root: DigestV1,
    /// Ordered field-native fold of every exact padded message block consumed so far.
    pub(crate) message_root: DigestV1,
    /// True only for the exact final stage, job boundary, and terminal root.
    pub(crate) complete: bool,
}

impl KagemushaMintHashClaimStateV1 {
    /// Apply exactly one valid shard statement to the prior claim, or establish the first claim.
    pub(crate) fn apply<F: KagemushaPoseidonFieldV1>(
        plan: KagemushaMintHashClaimPlanV1,
        previous: Option<Self>,
        leaf: &KagemushaMintHashShardStatementV1,
    ) -> Result<Self, String> {
        plan.validate::<F>()?;
        if leaf.parity != plan.parity
            || leaf.release_id != plan.release_id
            || leaf.plan_binding != plan.plan_binding
            || leaf.stage_index >= plan.total_stages
            || leaf.job_index >= plan.total_jobs
            || leaf.job_block_count == 0
            || leaf.block_index >= leaf.job_block_count
        {
            return Err("mint hash leaf is outside its authenticated typed plan".to_owned());
        }
        let (
            prior_stage,
            prior_job,
            prior_block,
            prior_blocks,
            prior_state,
            prior_terminal_root,
            prior_message_root,
        ) = if let Some(previous) = previous {
            previous.validate::<F>()?;
            if previous.plan != plan || previous.complete {
                return Err("mint hash predecessor is from another or completed plan".to_owned());
            }
            (
                previous.next_stage,
                previous.next_job,
                previous.next_block,
                previous.active_job_blocks,
                previous.chaining_state,
                decode::<F>(previous.terminal_root).ok_or_else(|| {
                    "mint hash predecessor root is not a canonical scalar".to_owned()
                })?,
                decode::<F>(previous.message_root).ok_or_else(|| {
                    "mint hash predecessor message root is not a canonical scalar".to_owned()
                })?,
            )
        } else {
            (
                0,
                0,
                0,
                0,
                IV,
                terminal_seed_native::<F>(plan.release_id, plan.total_stages, plan.total_jobs),
                message_seed_native::<F>(plan.release_id, plan.total_stages, plan.total_jobs),
            )
        };
        if leaf.stage_index != prior_stage
            || leaf.job_index != prior_job
            || leaf.block_index != prior_block
            || leaf.initial_state != prior_state
            || (prior_block == 0 && prior_blocks != 0)
            || (prior_block != 0 && prior_blocks != leaf.job_block_count)
        {
            return Err(
                "mint hash leaf is omitted, reordered, duplicated, or mis-chained".to_owned(),
            );
        }
        let final_block = leaf.block_index + 1 == leaf.job_block_count;
        let (next_job, next_block, active_job_blocks, chaining_state, terminal_root) =
            if final_block {
                (
                    leaf.job_index
                        .checked_add(1)
                        .ok_or_else(|| "mint hash job index overflowed".to_owned())?,
                    0,
                    0,
                    IV,
                    terminal_step_native::<F>(
                        prior_terminal_root,
                        leaf.job_index,
                        leaf.job_block_count,
                        leaf.output_state,
                    ),
                )
            } else {
                (
                    leaf.job_index,
                    leaf.block_index
                        .checked_add(1)
                        .ok_or_else(|| "mint hash block index overflowed".to_owned())?,
                    leaf.job_block_count,
                    leaf.output_state,
                    prior_terminal_root,
                )
            };
        let message_root = message_step_native::<F>(prior_message_root, leaf);
        let next_stage = leaf
            .stage_index
            .checked_add(1)
            .ok_or_else(|| "mint hash stage index overflowed".to_owned())?;
        let expected_root = decode::<F>(plan.expected_terminal_root)
            .ok_or_else(|| "mint hash expected root is not canonical".to_owned())?;
        let expected_message_root = decode::<F>(plan.expected_message_root)
            .ok_or_else(|| "mint hash expected message root is not canonical".to_owned())?;
        let complete = next_stage == plan.total_stages
            && next_job == plan.total_jobs
            && next_block == 0
            && terminal_root == expected_root
            && message_root == expected_message_root;
        if next_stage == plan.total_stages && !complete {
            return Err("mint hash terminal stage does not complete the typed plan".to_owned());
        }
        let state = Self {
            plan,
            next_stage,
            next_job,
            next_block,
            active_job_blocks,
            chaining_state,
            terminal_root: encode(terminal_root),
            message_root: encode(message_root),
            complete,
        };
        state.validate::<F>()?;
        Ok(state)
    }

    fn validate<F: KagemushaPoseidonFieldV1>(&self) -> Result<(), String> {
        self.plan.validate::<F>()?;
        let root = decode::<F>(self.terminal_root)
            .ok_or_else(|| "mint hash claim root is not canonical".to_owned())?;
        let message_root = decode::<F>(self.message_root)
            .ok_or_else(|| "mint hash claim message root is not canonical".to_owned())?;
        let at_boundary = self.next_block == 0;
        if self.next_stage == 0
            || self.next_stage > self.plan.total_stages
            || self.next_job > self.plan.total_jobs
            || (at_boundary && (self.active_job_blocks != 0 || self.chaining_state != IV))
            || (!at_boundary && self.active_job_blocks <= self.next_block)
        {
            return Err("mint hash claim cursor/state shape is invalid".to_owned());
        }
        let exact_complete = self.next_stage == self.plan.total_stages
            && self.next_job == self.plan.total_jobs
            && at_boundary
            && root
                == decode::<F>(self.plan.expected_terminal_root).ok_or_else(|| {
                    "mint hash expected terminal root is not canonical".to_owned()
                })?
            && message_root
                == decode::<F>(self.plan.expected_message_root)
                    .ok_or_else(|| "mint hash expected message root is not canonical".to_owned())?;
        if self.complete != exact_complete {
            return Err(
                "mint hash completeness bit is not derived from exact terminal state".to_owned(),
            );
        }
        Ok(())
    }
}

/// The common paired claim state. Each parity proves its own plan/root while carrying the other
/// component as cross-audited public data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimPairStateV1 {
    pub(crate) eq: KagemushaMintHashClaimStateV1,
    pub(crate) ep: KagemushaMintHashClaimStateV1,
}

impl KagemushaMintHashClaimPairStateV1 {
    fn validate(&self) -> Result<(), String> {
        self.eq.validate::<Fp>()?;
        self.ep.validate::<Fq>()?;
        if self.eq.plan.parity != KagemushaPastaParityV1::Eq
            || self.ep.plan.parity != KagemushaPastaParityV1::Ep
            || self.eq.plan.release_id != self.ep.plan.release_id
            || self.eq.plan.total_stages != self.ep.plan.total_stages
            || self.eq.plan.total_jobs != self.ep.plan.total_jobs
            || self.eq.next_stage != self.ep.next_stage
            || self.eq.next_job != self.ep.next_job
            || self.eq.next_block != self.ep.next_block
            || self.eq.active_job_blocks != self.ep.active_job_blocks
            || self.eq.chaining_state != self.ep.chaining_state
            || self.eq.complete != self.ep.complete
        {
            return Err("mint hash paired claim state is not one common transition".to_owned());
        }
        Ok(())
    }
}

/// Release-pinned recursive protocol identities and per-step paired audit/proof bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaMintHashClaimMetadataV1 {
    pub(crate) eq_claim_protocol: DigestV1,
    pub(crate) ep_claim_protocol: DigestV1,
    pub(crate) eq_shard_protocol: DigestV1,
    pub(crate) ep_shard_protocol: DigestV1,
    pub(crate) eq_deferred_audit: DigestV1,
    pub(crate) ep_deferred_audit: DigestV1,
    /// Field-native chain root over the exact recursively consumed Eq ordinary proof bytes.
    pub(crate) eq_proof_chain_root: DigestV1,
    /// Field-native chain root over the exact recursively consumed Ep ordinary proof bytes.
    pub(crate) ep_proof_chain_root: DigestV1,
}

impl KagemushaMintHashClaimMetadataV1 {
    fn validate(&self) -> Result<(), String> {
        let identities = [
            self.eq_claim_protocol,
            self.ep_claim_protocol,
            self.eq_shard_protocol,
            self.ep_shard_protocol,
            self.eq_deferred_audit,
            self.ep_deferred_audit,
            self.eq_proof_chain_root,
            self.ep_proof_chain_root,
        ];
        if identities.contains(&[0; 32])
            || self.eq_claim_protocol == self.ep_claim_protocol
            || self.eq_shard_protocol == self.ep_shard_protocol
        {
            return Err("mint hash claim metadata is absent or parity-aliased".to_owned());
        }
        Ok(())
    }
}

/// Fixed public instance layout of the paired claim-fold carrier.
pub(crate) mod public_instance {
    pub(crate) const VERSION: usize = 0;
    pub(crate) const PARITY: usize = 1;
    pub(crate) const COMPLETE: usize = 2;
    pub(crate) const RELEASE_LO: usize = 3;
    pub(crate) const EQ_PLAN_LO: usize = 5;
    pub(crate) const EP_PLAN_LO: usize = 7;
    pub(crate) const TOTAL_STAGES: usize = 9;
    pub(crate) const TOTAL_JOBS: usize = 10;
    pub(crate) const NEXT_STAGE: usize = 11;
    pub(crate) const NEXT_JOB: usize = 12;
    pub(crate) const NEXT_BLOCK: usize = 13;
    pub(crate) const ACTIVE_JOB_BLOCKS: usize = 14;
    pub(crate) const CHAINING_STATE: usize = 15;
    pub(crate) const EQ_MESSAGE_ROOT_LO: usize = 23;
    pub(crate) const EP_MESSAGE_ROOT_LO: usize = 25;
    pub(crate) const EQ_TERMINAL_ROOT_LO: usize = 27;
    pub(crate) const EP_TERMINAL_ROOT_LO: usize = 29;
    pub(crate) const EQ_EXPECTED_MESSAGE_ROOT_LO: usize = 31;
    pub(crate) const EP_EXPECTED_MESSAGE_ROOT_LO: usize = 33;
    pub(crate) const EQ_EXPECTED_ROOT_LO: usize = 35;
    pub(crate) const EP_EXPECTED_ROOT_LO: usize = 37;
    pub(crate) const EQ_CLAIM_PROTOCOL_LO: usize = 39;
    pub(crate) const EP_CLAIM_PROTOCOL_LO: usize = 41;
    pub(crate) const EQ_SHARD_PROTOCOL_LO: usize = 43;
    pub(crate) const EP_SHARD_PROTOCOL_LO: usize = 45;
    pub(crate) const EQ_AUDIT_LO: usize = 47;
    pub(crate) const EP_AUDIT_LO: usize = 49;
    pub(crate) const EQ_PROOF_CHAIN_LO: usize = 51;
    pub(crate) const EP_PROOF_CHAIN_LO: usize = 53;
    pub(crate) const HISTORY_START: usize = 55;
}

/// Exact constant public cell count, including one 544-byte k=16 history.
pub(crate) const KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1: usize =
    public_instance::HISTORY_START + 34;

/// One parity's private recursive witnesses.
pub(crate) struct KagemushaMintHashClaimParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(crate) parent_protocol: &'a PlonkProtocol<C>,
    pub(crate) parent_instances: &'a [Vec<C::ScalarExt>],
    pub(crate) parent_proof: &'a [u8],
    pub(crate) parent_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(crate) parent_fold_proof: &'a [u8],
    pub(crate) shard_protocol: &'a PlonkProtocol<C>,
    pub(crate) shard_proof: &'a [u8],
    pub(crate) leaf_fold_proof: &'a [u8],
    pub(crate) successor_history: &'a [u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Complete mutually audited claim-fold witness.
pub(crate) struct KagemushaMintHashClaimPairWitnessV1<'a> {
    pub(crate) previous: Option<KagemushaMintHashClaimPairStateV1>,
    pub(crate) previous_metadata: Option<KagemushaMintHashClaimMetadataV1>,
    pub(crate) successor: KagemushaMintHashClaimPairStateV1,
    pub(crate) metadata: KagemushaMintHashClaimMetadataV1,
    pub(crate) eq_leaf: KagemushaMintHashShardStatementV1,
    pub(crate) ep_leaf: KagemushaMintHashShardStatementV1,
    pub(crate) eq: KagemushaMintHashClaimParityWitnessV1<'a, EqAffine>,
    pub(crate) ep: KagemushaMintHashClaimParityWitnessV1<'a, EpAffine>,
}

/// Base-only configuration of the narrow k=16 claim fold.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaMintHashClaimConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
}

/// Eq/Fp half of the paired claim fold.
#[derive(Clone)]
pub(crate) struct KagemushaMintHashClaimEqCircuitV1 {
    pub(crate) builder: BaseCircuitBuilder<Fp>,
}

/// Ep/Fq half of the paired claim fold.
#[derive(Clone)]
pub(crate) struct KagemushaMintHashClaimEpCircuitV1 {
    pub(crate) builder: BaseCircuitBuilder<Fq>,
}

macro_rules! impl_claim_circuit {
    ($circuit:ty, $field:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaMintHashClaimConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                }
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                KagemushaMintHashClaimConfigV1 { base }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )
            }
        }
    };
}

impl_claim_circuit!(
    KagemushaMintHashClaimEqCircuitV1,
    Fp,
    "Kagemusha Eq mint hash claim"
);
impl_claim_circuit!(
    KagemushaMintHashClaimEpCircuitV1,
    Fq,
    "Kagemusha Ep mint hash claim"
);

struct ClaimScalarHalfV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    builder: BaseCircuitBuilder<C::ScalarExt>,
    output: KagemushaDeferredParentOutputV1<C>,
    common_cells: Vec<AssignedValue<C::ScalarExt>>,
}

/// Build one mutually audited recursive claim step.
///
/// The returned circuits are not independently authoritative. Their ordinary proof openings and
/// the returned carried histories must still be terminally decided by the mint-authority caller.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_kagemusha_mint_hash_claim_pair_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintHashClaimEqCircuitV1,
        KagemushaMintHashClaimEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    validate_mint_hash_shard_basis_prefix_v1(eq_carrier_params, eq_shard_params)?;
    validate_mint_hash_shard_basis_prefix_v1(ep_carrier_params, ep_shard_params)?;
    witness.successor.validate()?;
    witness.metadata.validate()?;
    if witness.previous.is_some() != witness.previous_metadata.is_some() {
        return Err("mint hash predecessor state/metadata presence differs".to_owned());
    }
    if witness.previous.is_none() {
        let expected_eq = super::initial_kagemusha_eq_accumulator_v1(eq_carrier_params)
            .map_err(|error| format!("failed to derive Eq mint hash seed history: {error}"))?;
        let actual_eq = super::KagemushaEqAccumulatorV1::from_native(witness.eq.parent_history)
            .map_err(|error| format!("invalid Eq mint hash seed history: {error}"))?;
        let expected_ep = super::initial_kagemusha_ep_accumulator_v1(ep_carrier_params)
            .map_err(|error| format!("failed to derive Ep mint hash seed history: {error}"))?;
        let actual_ep = super::KagemushaEpAccumulatorV1::from_native(witness.ep.parent_history)
            .map_err(|error| format!("invalid Ep mint hash seed history: {error}"))?;
        if actual_eq != expected_eq || actual_ep != expected_ep {
            return Err("mint hash bootstrap history is not the canonical decided seed".to_owned());
        }
    }
    if let Some(previous_metadata) = witness.previous_metadata {
        previous_metadata.validate()?;
        if previous_metadata.eq_claim_protocol != witness.metadata.eq_claim_protocol
            || previous_metadata.ep_claim_protocol != witness.metadata.ep_claim_protocol
            || previous_metadata.eq_shard_protocol != witness.metadata.eq_shard_protocol
            || previous_metadata.ep_shard_protocol != witness.metadata.ep_shard_protocol
        {
            return Err("mint hash predecessor uses another recursive verifier suite".to_owned());
        }
    }
    validate_paired_leaf_v1(&witness.eq_leaf, &witness.ep_leaf)?;
    let expected_eq = KagemushaMintHashClaimStateV1::apply::<Fp>(
        witness.successor.eq.plan,
        witness.previous.map(|state| state.eq),
        &witness.eq_leaf,
    )?;
    let expected_ep = KagemushaMintHashClaimStateV1::apply::<Fq>(
        witness.successor.ep.plan,
        witness.previous.map(|state| state.ep),
        &witness.ep_leaf,
    )?;
    if witness.successor.eq != expected_eq || witness.successor.ep != expected_ep {
        return Err("mint hash successor is not the exact paired leaf transition".to_owned());
    }

    let eq_carrier_svk = super::composite::eq_succinct_vk(eq_carrier_params);
    let ep_carrier_svk = super::composite::ep_succinct_vk(ep_carrier_params);
    let eq_shard_svk = super::composite::eq_succinct_vk(eq_shard_params);
    let ep_shard_svk = super::composite::ep_succinct_vk(ep_shard_params);
    let ClaimScalarHalfV1 {
        builder: mut eq_builder,
        output: eq_output,
        common_cells: eq_common,
    } = build_claim_scalar_half_v1::<EqAffine>(
        &eq_carrier_svk,
        &eq_shard_svk,
        KagemushaPastaParityV1::Eq,
        witness.previous.map(|state| state.eq),
        witness.previous_metadata,
        &witness.successor,
        witness.metadata,
        &witness.eq_leaf,
        witness.eq,
    )?;
    let ClaimScalarHalfV1 {
        builder: mut ep_builder,
        output: ep_output,
        common_cells: ep_common,
    } = build_claim_scalar_half_v1::<EpAffine>(
        &ep_carrier_svk,
        &ep_shard_svk,
        KagemushaPastaParityV1::Ep,
        witness.previous.map(|state| state.ep),
        witness.previous_metadata,
        &witness.successor,
        witness.metadata,
        &witness.ep_leaf,
        witness.ep,
    )?;

    bind_own_audit_v1(&mut eq_builder, public_instance::EQ_AUDIT_LO, &eq_output)?;
    bind_own_audit_v1(&mut ep_builder, public_instance::EP_AUDIT_LO, &ep_output)?;
    let eq_expected_ep = public_digest_cells_v1(
        &eq_builder,
        public_instance::EP_AUDIT_LO,
        "Eq claim Ep audit",
    )?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EpAffine>(
        &mut eq_builder,
        &ep_output,
        &eq_expected_ep,
        &eq_common,
    )?;
    let ep_expected_eq = public_digest_cells_v1(
        &ep_builder,
        public_instance::EQ_AUDIT_LO,
        "Ep claim Eq audit",
    )?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EqAffine>(
        &mut ep_builder,
        &eq_output,
        &ep_expected_eq,
        &ep_common,
    )?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let eq_audit = assigned_digest_bytes_v1(&eq_output.audit_digest_limbs)?;
    let ep_audit = assigned_digest_bytes_v1(&ep_output.audit_digest_limbs)?;
    Ok((
        KagemushaMintHashClaimEqCircuitV1 {
            builder: eq_builder,
        },
        KagemushaMintHashClaimEpCircuitV1 {
            builder: ep_builder,
        },
        eq_audit,
        ep_audit,
    ))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_claim_scalar_half_v1<C>(
    carrier_svk: &IpaSuccinctVerifyingKey<C>,
    shard_svk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    previous: Option<KagemushaMintHashClaimStateV1>,
    previous_metadata: Option<KagemushaMintHashClaimMetadataV1>,
    successor: &KagemushaMintHashClaimPairStateV1,
    metadata: KagemushaMintHashClaimMetadataV1,
    leaf: &KagemushaMintHashShardStatementV1,
    witness: KagemushaMintHashClaimParityWitnessV1<'_, C>,
) -> Result<ClaimScalarHalfV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
{
    if witness.parent_protocol.num_instance != [KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1]
        || witness.parent_instances.len() != 1
        || witness.parent_instances[0].len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
        || witness.shard_protocol.num_instance
            != [KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1]
    {
        return Err("mint hash claim parent or shard public ABI is not fixed".to_owned());
    }
    let state = match parity {
        KagemushaPastaParityV1::Eq => successor.eq,
        KagemushaPastaParityV1::Ep => successor.ep,
    };
    let mut builder = BaseCircuitBuilder::<C::ScalarExt>::new(false)
        .use_k(KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
        .use_lookup_bits((KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize)
        .use_instance_columns(1);
    let public_values = claim_public_values_v1::<C::ScalarExt>(
        parity,
        successor,
        metadata,
        witness.successor_history,
    )?;
    let public = public_values
        .into_iter()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    if public.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash claim public instance shape drifted".to_owned());
    }
    range_check_claim_public_v1(&mut builder, &public)?;
    builder.assigned_instances = vec![public.clone()];

    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let parent_enabled = {
        let mut ctx = loader.ctx_mut();
        let first = loader.ecc_chip().range().gate().is_equal(
            ctx.main(),
            public[public_instance::NEXT_STAGE],
            Constant(C::ScalarExt::ONE),
        );
        loader
            .ecc_chip()
            .range()
            .gate()
            .not(ctx.main(), Existing(first))
    };

    let expected_claim_protocol = public_digest_cells_from_slice_v1(
        &public,
        match parity {
            KagemushaPastaParityV1::Eq => public_instance::EQ_CLAIM_PROTOCOL_LO,
            KagemushaPastaParityV1::Ep => public_instance::EP_CLAIM_PROTOCOL_LO,
        },
        "claim protocol",
    )?;
    let claim_structure = kagemusha_protocol_structure_digest_v1(witness.parent_protocol, parity)?;
    let loaded_parent = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.parent_protocol,
        parity,
        claim_structure,
        &expected_claim_protocol,
    )
    .map_err(|error| format!("failed to bind mint hash claim protocol: {error:?}"))?;
    let parent_instances = witness
        .parent_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let parent_assigned = verify_ordinary_proof_with_canonical_bytes_v1(
        &loader,
        carrier_svk,
        &loaded_parent.protocol,
        &parent_instances,
        witness.parent_proof,
    )
    .map_err(|error| format!("failed to verify mint hash claim predecessor: {error:?}"))?;
    let parent_column = parent_instances
        .first()
        .ok_or_else(|| "mint hash claim predecessor column is absent".to_owned())?;
    let parent_history = load_native_accumulator(&loader, witness.parent_history)
        .map_err(|error| format!("failed to load mint hash claim history: {error:?}"))?;
    let parent_history_cells = parent_column
        .get(public_instance::HISTORY_START..)
        .ok_or_else(|| "mint hash claim predecessor history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &parent_history, &parent_history_cells)
        .map_err(|error| format!("failed to bind mint hash predecessor history: {error:?}"))?;
    let parent_fold = verify_fold(
        &loader,
        carrier_svk,
        &[parent_assigned.accumulator, parent_history.clone()],
        witness.parent_fold_proof,
    )
    .map_err(|error| format!("failed to fold mint hash claim predecessor: {error:?}"))?;
    let prior_history =
        select_accumulator_v1(&loader, &parent_fold, &parent_history, parent_enabled).map_err(
            |error| format!("failed to select mint hash predecessor history: {error:?}"),
        )?;
    let parent_equations = loader.ecc_chip().equation_count();

    let expected_shard_protocol = public_digest_cells_from_slice_v1(
        &public,
        match parity {
            KagemushaPastaParityV1::Eq => public_instance::EQ_SHARD_PROTOCOL_LO,
            KagemushaPastaParityV1::Ep => public_instance::EP_SHARD_PROTOCOL_LO,
        },
        "shard protocol",
    )?;
    let shard_structure = kagemusha_protocol_structure_digest_v1(witness.shard_protocol, parity)?;
    let loaded_shard = load_and_constrain_parent_protocol_v1(
        &loader,
        witness.shard_protocol,
        parity,
        shard_structure,
        &expected_shard_protocol,
    )
    .map_err(|error| format!("failed to bind mint hash shard protocol: {error:?}"))?;
    let shard_values = shard_public_values_v1::<C::ScalarExt>(leaf)?;
    let shard_instances = vec![
        shard_values
            .iter()
            .copied()
            .map(|value| loader.assign_scalar(value))
            .collect::<Vec<_>>(),
    ];
    let shard_assigned = verify_ordinary_proof_with_canonical_bytes_at_k_v1(
        &loader,
        shard_svk,
        &loaded_shard.protocol,
        &shard_instances,
        witness.shard_proof,
        KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize,
    )
    .map_err(|error| format!("failed to verify mint hash shard proof: {error:?}"))?;
    bind_shard_instances_v1(&loader, &public, &shard_instances[0], parity)?;
    constrain_claim_parent_and_leaf_cursor_v1(
        &loader,
        &public,
        parent_column,
        &shard_instances[0],
        parent_enabled,
        previous,
        state,
    )?;
    let lifted = lift_mint_hash_shard_accumulator_v1(&loader, shard_assigned.accumulator)?;
    let successor_history = verify_fold(
        &loader,
        carrier_svk,
        &[lifted, prior_history],
        witness.leaf_fold_proof,
    )
    .map_err(|error| format!("failed to fold lifted mint hash shard: {error:?}"))?;
    bind_accumulator_limbs(
        &loader,
        &successor_history,
        public
            .get(public_instance::HISTORY_START..)
            .ok_or_else(|| "mint hash successor history is absent".to_owned())?,
    )
    .map_err(|error| format!("failed to bind mint hash successor history: {error:?}"))?;

    constrain_proof_chain_root_v1(
        &loader,
        &public,
        parent_column,
        parity,
        parent_enabled,
        previous_metadata,
        metadata,
        &parent_assigned.canonical_bytes,
        &shard_assigned.canonical_bytes,
    )?;
    let equation_count = loader.ecc_chip().equation_count();
    if parent_equations == 0 || equation_count <= parent_equations {
        return Err("mint hash claim verifier emitted an incomplete equation audit".to_owned());
    }
    let common_cells = common_public_cells_v1(&public);
    let mut tags = vec![CLAIM_PARENT_EQUATION_TAG_V1; parent_equations];
    tags.resize(equation_count, CLAIM_SHARD_EQUATION_TAG_V1);
    let mut assigned_selectors = vec![parent_enabled; parent_equations];
    assigned_selectors.extend(
        (parent_equations..equation_count)
            .map(|_| loader.ctx_mut().main().load_constant(C::ScalarExt::ONE)),
    );
    let mut selectors = vec![previous.is_some(); parent_equations];
    selectors.resize(equation_count, true);
    let output = finalize_deferred_audit_plan_with_u128_binding_v1(
        &mut builder,
        loader,
        tags,
        assigned_selectors,
        selectors,
        &common_cells,
    )
    .map_err(|error| format!("failed to finalize mint hash claim audit: {error:?}"))?;
    Ok(ClaimScalarHalfV1 {
        builder,
        output,
        common_cells,
    })
}

fn validate_paired_leaf_v1(
    eq: &KagemushaMintHashShardStatementV1,
    ep: &KagemushaMintHashShardStatementV1,
) -> Result<(), String> {
    if eq.parity != KagemushaPastaParityV1::Eq
        || ep.parity != KagemushaPastaParityV1::Ep
        || eq.release_id != ep.release_id
        || eq.stage_index != ep.stage_index
        || eq.job_index != ep.job_index
        || eq.block_index != ep.block_index
        || eq.job_block_count != ep.job_block_count
        || eq.initial_state != ep.initial_state
        || eq.block_words != ep.block_words
        || eq.output_state != ep.output_state
    {
        return Err("mint hash shard pair does not prove one common compression".to_owned());
    }
    Ok(())
}

fn claim_public_values_v1<F: KagemushaPoseidonFieldV1>(
    parity: KagemushaPastaParityV1,
    state: &KagemushaMintHashClaimPairStateV1,
    metadata: KagemushaMintHashClaimMetadataV1,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    state.validate()?;
    metadata.validate()?;
    let mut values = Vec::with_capacity(KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1);
    values.extend([
        F::ONE,
        F::from(match parity {
            KagemushaPastaParityV1::Eq => 0,
            KagemushaPastaParityV1::Ep => 1,
        }),
        F::from(u64::from(state.eq.complete)),
    ]);
    values.extend(digest_limbs::<F>(state.eq.plan.release_id));
    values.extend(digest_limbs::<F>(state.eq.plan.plan_binding));
    values.extend(digest_limbs::<F>(state.ep.plan.plan_binding));
    values.extend([
        F::from(state.eq.plan.total_stages),
        F::from(u64::from(state.eq.plan.total_jobs)),
        F::from(state.eq.next_stage),
        F::from(u64::from(state.eq.next_job)),
        F::from(u64::from(state.eq.next_block)),
        F::from(u64::from(state.eq.active_job_blocks)),
    ]);
    values.extend(state.eq.chaining_state.map(|word| F::from(u64::from(word))));
    for digest in [
        state.eq.message_root,
        state.ep.message_root,
        state.eq.terminal_root,
        state.ep.terminal_root,
        state.eq.plan.expected_message_root,
        state.ep.plan.expected_message_root,
        state.eq.plan.expected_terminal_root,
        state.ep.plan.expected_terminal_root,
        metadata.eq_claim_protocol,
        metadata.ep_claim_protocol,
        metadata.eq_shard_protocol,
        metadata.ep_shard_protocol,
        metadata.eq_deferred_audit,
        metadata.ep_deferred_audit,
        metadata.eq_proof_chain_root,
        metadata.ep_proof_chain_root,
    ] {
        values.extend(digest_limbs::<F>(digest));
    }
    let history_limbs = history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk.try_into().expect("history limb has sixteen bytes"),
        ))
    });
    values.extend(history_limbs);
    if values.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash claim public value count drifted".to_owned());
    }
    Ok(values)
}

fn shard_public_values_v1<F: KagemushaPoseidonFieldV1>(
    leaf: &KagemushaMintHashShardStatementV1,
) -> Result<Vec<F>, String> {
    let expected_parity = if F::IS_EQ_PARITY {
        KagemushaPastaParityV1::Eq
    } else {
        KagemushaPastaParityV1::Ep
    };
    if leaf.parity != expected_parity
        || leaf.release_id == [0; 32]
        || leaf.plan_binding == [0; 32]
        || leaf.job_block_count == 0
        || leaf.block_index >= leaf.job_block_count
        || (leaf.block_index == 0 && leaf.initial_state != IV)
    {
        return Err("mint hash shard public statement shape is invalid".to_owned());
    }
    let mut values = Vec::with_capacity(KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1);
    values.extend([
        F::ONE,
        F::from(match leaf.parity {
            KagemushaPastaParityV1::Eq => 0,
            KagemushaPastaParityV1::Ep => 1,
        }),
    ]);
    values.extend(digest_limbs::<F>(leaf.release_id));
    values.extend(digest_limbs::<F>(leaf.plan_binding));
    values.extend([
        F::from(leaf.stage_index),
        F::from(u64::from(leaf.job_index)),
        F::from(u64::from(leaf.block_index)),
        F::from(u64::from(leaf.job_block_count)),
    ]);
    values.extend(leaf.initial_state.map(|word| F::from(u64::from(word))));
    values.extend(leaf.block_words.map(|word| F::from(u64::from(word))));
    values.extend(leaf.output_state.map(|word| F::from(u64::from(word))));
    values.push(F::from(u64::from(
        leaf.block_index + 1 == leaf.job_block_count,
    )));
    if values.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash shard public value count drifted".to_owned());
    }
    Ok(values)
}

fn range_check_claim_public_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    public: &[AssignedValue<F>],
) -> Result<(), String> {
    if public.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash claim range-check shape mismatch".to_owned());
    }
    let range = builder.range_chip();
    for index in [
        public_instance::VERSION,
        public_instance::PARITY,
        public_instance::COMPLETE,
    ] {
        range.range_check(builder.main(0), public[index], 1);
    }
    for index in public_instance::RELEASE_LO..public_instance::TOTAL_STAGES {
        range.range_check(builder.main(0), public[index], 128);
    }
    for (index, bits) in [
        (public_instance::TOTAL_STAGES, 64),
        (public_instance::TOTAL_JOBS, 32),
        (public_instance::NEXT_STAGE, 64),
        (public_instance::NEXT_JOB, 32),
        (public_instance::NEXT_BLOCK, 32),
        (public_instance::ACTIVE_JOB_BLOCKS, 32),
    ] {
        range.range_check(builder.main(0), public[index], bits);
    }
    for index in public_instance::CHAINING_STATE..public_instance::EQ_TERMINAL_ROOT_LO {
        range.range_check(builder.main(0), public[index], 32);
    }
    for index in
        public_instance::EQ_MESSAGE_ROOT_LO..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
    {
        range.range_check(builder.main(0), public[index], 128);
    }
    Ok(())
}

fn public_digest_cells_from_slice_v1<F: halo2_base::utils::ScalarField>(
    public: &[AssignedValue<F>],
    offset: usize,
    label: &str,
) -> Result<[AssignedValue<F>; 2], String> {
    public
        .get(offset..offset + 2)
        .ok_or_else(|| format!("mint hash {label} digest is absent"))?
        .try_into()
        .map_err(|_| format!("mint hash {label} digest shape drifted"))
}

fn public_digest_cells_v1<F: halo2_base::utils::ScalarField>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
    label: &str,
) -> Result<[AssignedValue<F>; 2], String> {
    let public = builder
        .assigned_instances
        .first()
        .ok_or_else(|| "mint hash claim public column is absent".to_owned())?;
    public_digest_cells_from_slice_v1(public, offset, label)
}

fn bind_own_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    offset: usize,
    output: &KagemushaDeferredParentOutputV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let expected = public_digest_cells_v1(builder, offset, "own audit")?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok(())
}

fn assigned_digest_bytes_v1<F: halo2_base::utils::ScalarField>(
    limbs: &[AssignedValue<F>; 2],
) -> Result<DigestV1, String> {
    use halo2_base::utils::fe_to_biguint;

    let mut digest = [0_u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        let bytes = fe_to_biguint(limb.value()).to_bytes_le();
        if bytes.len() > 16 {
            return Err("mint hash claim audit limb exceeds u128".to_owned());
        }
        digest[index * 16..index * 16 + bytes.len()].copy_from_slice(&bytes);
    }
    if digest == [0; 32] {
        return Err("mint hash claim audit digest is zero".to_owned());
    }
    Ok(digest)
}

fn common_public_cells_v1<F: halo2_base::utils::ScalarField>(
    public: &[AssignedValue<F>],
) -> Vec<AssignedValue<F>> {
    (0..public_instance::HISTORY_START)
        .filter(|index| {
            *index != public_instance::PARITY
                && !(*index >= public_instance::EQ_AUDIT_LO
                    && *index < public_instance::EQ_PROOF_CHAIN_LO)
        })
        .map(|index| public[index])
        .collect()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn constrain_claim_parent_and_leaf_cursor_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    public: &[AssignedValue<C::ScalarExt>],
    parent: &[DeferredScalar<'chip, C>],
    shard: &[DeferredScalar<'chip, C>],
    parent_enabled: AssignedValue<C::ScalarExt>,
    previous: Option<KagemushaMintHashClaimStateV1>,
    state: KagemushaMintHashClaimStateV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if parent.len() != KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
        || shard.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("mint hash claim predecessor public column is truncated".to_owned());
    }
    let chip = loader.ecc_chip();
    let range = chip.range();
    let gate = range.gate();
    let mut ctx = loader.ctx_mut();
    let ctx = ctx.main();
    let leaf_stage = *shard[shard_public::STAGE].assigned();
    let leaf_job = *shard[shard_public::JOB].assigned();
    let leaf_block = *shard[shard_public::BLOCK].assigned();
    let leaf_blocks = *shard[shard_public::JOB_BLOCKS].assigned();

    let stage_in_range =
        range.is_less_than(ctx, leaf_stage, public[public_instance::TOTAL_STAGES], 64);
    gate.assert_is_const(ctx, &stage_in_range, &C::ScalarExt::ONE);
    let job_in_range = range.is_less_than(ctx, leaf_job, public[public_instance::TOTAL_JOBS], 32);
    gate.assert_is_const(ctx, &job_in_range, &C::ScalarExt::ONE);

    for (parent_offset, expected) in [
        (public_instance::VERSION, public[public_instance::VERSION]),
        (public_instance::PARITY, public[public_instance::PARITY]),
        (
            public_instance::RELEASE_LO,
            public[public_instance::RELEASE_LO],
        ),
        (
            public_instance::RELEASE_LO + 1,
            public[public_instance::RELEASE_LO + 1],
        ),
        (
            public_instance::EQ_PLAN_LO,
            public[public_instance::EQ_PLAN_LO],
        ),
        (
            public_instance::EQ_PLAN_LO + 1,
            public[public_instance::EQ_PLAN_LO + 1],
        ),
        (
            public_instance::EP_PLAN_LO,
            public[public_instance::EP_PLAN_LO],
        ),
        (
            public_instance::EP_PLAN_LO + 1,
            public[public_instance::EP_PLAN_LO + 1],
        ),
        (
            public_instance::TOTAL_STAGES,
            public[public_instance::TOTAL_STAGES],
        ),
        (
            public_instance::TOTAL_JOBS,
            public[public_instance::TOTAL_JOBS],
        ),
        (
            public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO,
            public[public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO],
        ),
        (
            public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO + 1,
            public[public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO + 1],
        ),
        (
            public_instance::EP_EXPECTED_MESSAGE_ROOT_LO,
            public[public_instance::EP_EXPECTED_MESSAGE_ROOT_LO],
        ),
        (
            public_instance::EP_EXPECTED_MESSAGE_ROOT_LO + 1,
            public[public_instance::EP_EXPECTED_MESSAGE_ROOT_LO + 1],
        ),
        (
            public_instance::EQ_EXPECTED_ROOT_LO,
            public[public_instance::EQ_EXPECTED_ROOT_LO],
        ),
        (
            public_instance::EQ_EXPECTED_ROOT_LO + 1,
            public[public_instance::EQ_EXPECTED_ROOT_LO + 1],
        ),
        (
            public_instance::EP_EXPECTED_ROOT_LO,
            public[public_instance::EP_EXPECTED_ROOT_LO],
        ),
        (
            public_instance::EP_EXPECTED_ROOT_LO + 1,
            public[public_instance::EP_EXPECTED_ROOT_LO + 1],
        ),
        (
            public_instance::EQ_CLAIM_PROTOCOL_LO,
            public[public_instance::EQ_CLAIM_PROTOCOL_LO],
        ),
        (
            public_instance::EQ_CLAIM_PROTOCOL_LO + 1,
            public[public_instance::EQ_CLAIM_PROTOCOL_LO + 1],
        ),
        (
            public_instance::EP_CLAIM_PROTOCOL_LO,
            public[public_instance::EP_CLAIM_PROTOCOL_LO],
        ),
        (
            public_instance::EP_CLAIM_PROTOCOL_LO + 1,
            public[public_instance::EP_CLAIM_PROTOCOL_LO + 1],
        ),
        (
            public_instance::EQ_SHARD_PROTOCOL_LO,
            public[public_instance::EQ_SHARD_PROTOCOL_LO],
        ),
        (
            public_instance::EQ_SHARD_PROTOCOL_LO + 1,
            public[public_instance::EQ_SHARD_PROTOCOL_LO + 1],
        ),
        (
            public_instance::EP_SHARD_PROTOCOL_LO,
            public[public_instance::EP_SHARD_PROTOCOL_LO],
        ),
        (
            public_instance::EP_SHARD_PROTOCOL_LO + 1,
            public[public_instance::EP_SHARD_PROTOCOL_LO + 1],
        ),
    ] {
        constrain_equal_if_v1(
            ctx,
            gate,
            *parent[parent_offset].assigned(),
            expected,
            parent_enabled,
        );
    }
    let zero = ctx.load_zero();
    constrain_equal_if_v1(
        ctx,
        gate,
        *parent[public_instance::COMPLETE].assigned(),
        zero,
        parent_enabled,
    );
    for (offset, expected) in [
        (public_instance::NEXT_STAGE, leaf_stage),
        (public_instance::NEXT_JOB, leaf_job),
        (public_instance::NEXT_BLOCK, leaf_block),
    ] {
        constrain_equal_if_v1(
            ctx,
            gate,
            *parent[offset].assigned(),
            expected,
            parent_enabled,
        );
    }
    let first_block = gate.is_zero(ctx, leaf_block);
    let expected_active = gate.select(
        ctx,
        Constant(C::ScalarExt::ZERO),
        Existing(leaf_blocks),
        first_block,
    );
    constrain_equal_if_v1(
        ctx,
        gate,
        *parent[public_instance::ACTIVE_JOB_BLOCKS].assigned(),
        expected_active,
        parent_enabled,
    );
    for index in 0..DIGEST_SIZE {
        let expected = *shard[shard_public::INITIAL_STATE + index].assigned();
        constrain_equal_if_v1(
            ctx,
            gate,
            *parent[public_instance::CHAINING_STATE + index].assigned(),
            expected,
            parent_enabled,
        );
    }

    let next_stage = gate.add(ctx, Existing(leaf_stage), Constant(C::ScalarExt::ONE));
    ctx.constrain_equal(&next_stage, &public[public_instance::NEXT_STAGE]);
    let final_block = *shard[shard_public::FINAL_BLOCK].assigned();
    let next_job = gate.add(ctx, Existing(leaf_job), Existing(final_block));
    ctx.constrain_equal(&next_job, &public[public_instance::NEXT_JOB]);
    let block_plus_one = gate.add(ctx, Existing(leaf_block), Constant(C::ScalarExt::ONE));
    let next_block = gate.select(
        ctx,
        Constant(C::ScalarExt::ZERO),
        Existing(block_plus_one),
        final_block,
    );
    ctx.constrain_equal(&next_block, &public[public_instance::NEXT_BLOCK]);
    let next_active = gate.select(
        ctx,
        Constant(C::ScalarExt::ZERO),
        Existing(leaf_blocks),
        final_block,
    );
    ctx.constrain_equal(&next_active, &public[public_instance::ACTIVE_JOB_BLOCKS]);
    for index in 0..DIGEST_SIZE {
        let output = *shard[shard_public::OUTPUT_STATE + index].assigned();
        let expected = gate.select(
            ctx,
            Constant(C::ScalarExt::from(u64::from(IV[index]))),
            Existing(output),
            final_block,
        );
        ctx.constrain_equal(&expected, &public[public_instance::CHAINING_STATE + index]);
    }

    let release =
        public_digest_cells_from_slice_v1(public, public_instance::RELEASE_LO, "release")?;
    let plan_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let terminal_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_TERMINAL_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_TERMINAL_ROOT_LO,
    };
    let message_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_MESSAGE_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_MESSAGE_ROOT_LO,
    };
    let expected_message_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_EXPECTED_MESSAGE_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_EXPECTED_MESSAGE_ROOT_LO,
    };
    let expected_offset = match state.plan.parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_EXPECTED_ROOT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_EXPECTED_ROOT_LO,
    };
    let plan_limbs = public_digest_cells_from_slice_v1(public, plan_offset, "plan")?;
    let expected_limbs =
        public_digest_cells_from_slice_v1(public, expected_offset, "expected root")?;
    let expected_message_limbs = public_digest_cells_from_slice_v1(
        public,
        expected_message_offset,
        "expected message root",
    )?;
    let expected_message_root = assign_encoded_scalar_v1(
        ctx,
        range,
        state.plan.expected_message_root,
        expected_message_limbs,
    )?;
    let expected_root = assign_encoded_scalar_v1(
        ctx,
        range,
        state.plan.expected_terminal_root,
        expected_limbs,
    )?;
    let plan_binding = assign_encoded_scalar_v1(ctx, range, state.plan.plan_binding, plan_limbs)?;
    constrain_plan_binding_v1(
        ctx,
        range,
        release,
        public[public_instance::TOTAL_STAGES],
        public[public_instance::TOTAL_JOBS],
        expected_message_root,
        expected_root,
        plan_binding,
    );

    // Both branches are always assigned so bootstrap and continuation compile to one circuit/VK.
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let seed_root = poseidon.hash(
        ctx,
        range,
        TERMINAL_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            public[public_instance::TOTAL_STAGES],
            public[public_instance::TOTAL_JOBS],
        ],
    );
    let seed_message_root = poseidon.hash(
        ctx,
        range,
        MESSAGE_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            public[public_instance::TOTAL_STAGES],
            public[public_instance::TOTAL_JOBS],
        ],
    );
    let parent_limbs: [AssignedValue<C::ScalarExt>; 2] = parent
        .get(terminal_offset..terminal_offset + 2)
        .ok_or_else(|| "mint hash parent terminal root is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash parent terminal root shape drifted".to_owned())?;
    let previous_digest = previous
        .map(|claim| claim.terminal_root)
        .unwrap_or(state.terminal_root);
    let (assigned_parent_root, assigned_parent_limbs) =
        assign_scalar_digest_v1(ctx, range, previous_digest)?;
    for (actual, expected) in assigned_parent_limbs.into_iter().zip(parent_limbs) {
        constrain_equal_if_v1(ctx, gate, actual, expected, parent_enabled);
    }
    let prior_root = gate.select(
        ctx,
        Existing(assigned_parent_root),
        Existing(seed_root),
        parent_enabled,
    );
    let parent_message_limbs: [AssignedValue<C::ScalarExt>; 2] = parent
        .get(message_offset..message_offset + 2)
        .ok_or_else(|| "mint hash parent message root is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash parent message root shape drifted".to_owned())?;
    let previous_message_digest = previous
        .map(|claim| claim.message_root)
        .unwrap_or(state.message_root);
    let (assigned_parent_message_root, assigned_parent_message_limbs) =
        assign_scalar_digest_v1(ctx, range, previous_message_digest)?;
    for (actual, expected) in assigned_parent_message_limbs
        .into_iter()
        .zip(parent_message_limbs)
    {
        constrain_equal_if_v1(ctx, gate, actual, expected, parent_enabled);
    }
    let prior_message_root = gate.select(
        ctx,
        Existing(assigned_parent_message_root),
        Existing(seed_message_root),
        parent_enabled,
    );
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let mut terminal_inputs = Vec::with_capacity(3 + DIGEST_SIZE);
    terminal_inputs.push(prior_root);
    terminal_inputs.push(leaf_job);
    terminal_inputs.push(leaf_blocks);
    terminal_inputs.extend(
        shard[shard_public::OUTPUT_STATE..shard_public::OUTPUT_STATE + DIGEST_SIZE]
            .iter()
            .map(|value| *value.assigned()),
    );
    let advanced_root = poseidon.hash(ctx, range, TERMINAL_STEP_DOMAIN_V1, &terminal_inputs);
    let current_root = gate.select(
        ctx,
        Existing(advanced_root),
        Existing(prior_root),
        final_block,
    );
    let current_limbs =
        public_digest_cells_from_slice_v1(public, terminal_offset, "terminal root")?;
    let expected_current =
        assign_encoded_scalar_v1(ctx, range, state.terminal_root, current_limbs)?;
    ctx.constrain_equal(&current_root, &expected_current);

    let mut message_inputs = Vec::with_capacity(5 + BLOCK_SIZE);
    message_inputs.extend([
        prior_message_root,
        leaf_stage,
        leaf_job,
        leaf_block,
        leaf_blocks,
    ]);
    message_inputs.extend(
        shard[shard_public::BLOCK_WORDS..shard_public::BLOCK_WORDS + BLOCK_SIZE]
            .iter()
            .map(|value| *value.assigned()),
    );
    let current_message_root = poseidon.hash(ctx, range, MESSAGE_STEP_DOMAIN_V1, &message_inputs);
    let current_message_limbs =
        public_digest_cells_from_slice_v1(public, message_offset, "message root")?;
    let expected_current_message =
        assign_encoded_scalar_v1(ctx, range, state.message_root, current_message_limbs)?;
    ctx.constrain_equal(&current_message_root, &expected_current_message);

    let stage_complete = gate.is_equal(
        ctx,
        public[public_instance::NEXT_STAGE],
        public[public_instance::TOTAL_STAGES],
    );
    let job_complete = gate.is_equal(
        ctx,
        public[public_instance::NEXT_JOB],
        public[public_instance::TOTAL_JOBS],
    );
    let block_boundary = gate.is_zero(ctx, public[public_instance::NEXT_BLOCK]);
    let root_complete = gate.is_equal(ctx, current_root, expected_root);
    let message_complete = gate.is_equal(ctx, current_message_root, expected_message_root);
    let complete = [
        job_complete,
        block_boundary,
        root_complete,
        message_complete,
    ]
    .into_iter()
    .fold(stage_complete, |value, condition| {
        gate.mul(ctx, Existing(value), Existing(condition))
    });
    ctx.constrain_equal(&complete, &public[public_instance::COMPLETE]);
    let not_complete = gate.not(ctx, Existing(complete));
    let terminal_mismatch = gate.mul(ctx, Existing(stage_complete), Existing(not_complete));
    gate.assert_is_const(ctx, &terminal_mismatch, &C::ScalarExt::ZERO);
    Ok(())
}

fn bind_shard_instances_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    public: &[AssignedValue<C::ScalarExt>],
    shard: &[DeferredScalar<'chip, C>],
    parity: KagemushaPastaParityV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if shard.len() != KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint hash shard recursive public column is truncated".to_owned());
    }
    let plan_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let mut ctx = loader.ctx_mut();
    let ctx = ctx.main();
    let shard_version = *shard[shard_public::VERSION].assigned();
    let shard_parity = *shard[shard_public::PARITY].assigned();
    ctx.constrain_equal(&shard_version, &public[public_instance::VERSION]);
    ctx.constrain_equal(&shard_parity, &public[public_instance::PARITY]);
    for (leaf, expected) in shard[shard_public::RELEASE_LO..shard_public::RELEASE_LO + 2]
        .iter()
        .zip(&public[public_instance::RELEASE_LO..public_instance::RELEASE_LO + 2])
    {
        let leaf = *leaf.assigned();
        ctx.constrain_equal(&leaf, expected);
    }
    for (leaf, expected) in shard[shard_public::PLAN_LO..shard_public::PLAN_LO + 2]
        .iter()
        .zip(&public[plan_offset..plan_offset + 2])
    {
        let leaf = *leaf.assigned();
        ctx.constrain_equal(&leaf, expected);
    }
    Ok(())
}

fn constrain_proof_chain_root_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    public: &[AssignedValue<C::ScalarExt>],
    parent: &[DeferredScalar<'chip, C>],
    parity: KagemushaPastaParityV1,
    parent_enabled: AssignedValue<C::ScalarExt>,
    previous_metadata: Option<KagemushaMintHashClaimMetadataV1>,
    metadata: KagemushaMintHashClaimMetadataV1,
    parent_bytes: &[crate::zk::pasta_sha256::PastaSha256ByteV1<C::ScalarExt>],
    shard_bytes: &[crate::zk::pasta_sha256::PastaSha256ByteV1<C::ScalarExt>],
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let proof_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PROOF_CHAIN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PROOF_CHAIN_LO,
    };
    let plan_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_PLAN_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_PLAN_LO,
    };
    let chip = loader.ecc_chip();
    let range = chip.range();
    let gate = range.gate();
    let mut ctx = loader.ctx_mut();
    let ctx = ctx.main();
    // As with the state root, assign both branches unconditionally so there is one circuit/VK.
    let release = &public[public_instance::RELEASE_LO..public_instance::RELEASE_LO + 2];
    let plan = &public[plan_offset..plan_offset + 2];
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let seed = poseidon.hash(
        ctx,
        range,
        PROOF_CHAIN_SEED_DOMAIN_V1,
        &[release[0], release[1], plan[0], plan[1]],
    );
    let digest = previous_metadata
        .map(|previous| match parity {
            KagemushaPastaParityV1::Eq => previous.eq_proof_chain_root,
            KagemushaPastaParityV1::Ep => previous.ep_proof_chain_root,
        })
        .unwrap_or(match parity {
            KagemushaPastaParityV1::Eq => metadata.eq_proof_chain_root,
            KagemushaPastaParityV1::Ep => metadata.ep_proof_chain_root,
        });
    let (assigned_parent, assigned_parent_limbs) = assign_scalar_digest_v1(ctx, range, digest)?;
    let parent_limbs: [AssignedValue<C::ScalarExt>; 2] = parent
        .get(proof_offset..proof_offset + 2)
        .ok_or_else(|| "mint hash predecessor proof-chain root is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "mint hash predecessor proof-chain root shape drifted".to_owned())?;
    for (actual, expected) in assigned_parent_limbs.into_iter().zip(parent_limbs) {
        constrain_equal_if_v1(ctx, gate, actual, expected, parent_enabled);
    }
    let prior = gate.select(
        ctx,
        Existing(assigned_parent),
        Existing(seed),
        parent_enabled,
    );
    let parent_chunks = proof_byte_chunks_v1(ctx, gate, parent_bytes)?;
    let shard_chunks = proof_byte_chunks_v1(ctx, gate, shard_bytes)?;
    let mut inputs = Vec::with_capacity(parent_chunks.len() + shard_chunks.len() + 4);
    inputs.extend([
        prior,
        public[public_instance::NEXT_STAGE],
        ctx.load_constant(C::ScalarExt::from(
            u64::try_from(parent_chunks.len())
                .map_err(|_| "mint hash parent proof chunk count exceeds u64".to_owned())?,
        )),
    ]);
    inputs.extend(parent_chunks);
    inputs.push(
        ctx.load_constant(C::ScalarExt::from(
            u64::try_from(shard_chunks.len())
                .map_err(|_| "mint hash shard proof chunk count exceeds u64".to_owned())?,
        )),
    );
    inputs.extend(shard_chunks);
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let root = poseidon.hash(ctx, range, PROOF_CHAIN_STEP_DOMAIN_V1, &inputs);
    let expected_limbs = public_digest_cells_from_slice_v1(public, proof_offset, "proof chain")?;
    let expected_digest = match parity {
        KagemushaPastaParityV1::Eq => metadata.eq_proof_chain_root,
        KagemushaPastaParityV1::Ep => metadata.ep_proof_chain_root,
    };
    let expected = assign_encoded_scalar_v1(ctx, range, expected_digest, expected_limbs)?;
    ctx.constrain_equal(&root, &expected);
    Ok(())
}

fn proof_byte_chunks_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    bytes: &[crate::zk::pasta_sha256::PastaSha256ByteV1<F>],
) -> Result<Vec<AssignedValue<F>>, String> {
    bytes
        .chunks(16)
        .map(|chunk| {
            let assigned = chunk
                .iter()
                .copied()
                .map(|byte| {
                    byte.assigned().ok_or_else(|| {
                        "canonical recursive proof byte lost assigned provenance".to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(gate.inner_product(
                ctx,
                assigned,
                (0..chunk.len()).map(|index| Constant(from_u128::<F>(1_u128 << (8 * index)))),
            ))
        })
        .collect()
}

fn assign_encoded_scalar_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    digest: DigestV1,
    expected_limbs: [AssignedValue<F>; 2],
) -> Result<AssignedValue<F>, String> {
    let (assigned, limbs) = assign_scalar_digest_v1(ctx, range, digest)?;
    for (actual, expected) in limbs.into_iter().zip(expected_limbs) {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(assigned)
}

fn assign_scalar_digest_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    digest: DigestV1,
) -> Result<(AssignedValue<F>, [AssignedValue<F>; 2]), String> {
    let scalar = decode::<F>(digest)
        .ok_or_else(|| "mint hash field-native digest is not canonical".to_owned())?;
    let assigned = ctx.load_witness(scalar);
    let limbs = scalar_digest_limbs_v1(ctx, range.gate(), assigned);
    Ok((assigned, limbs))
}

fn scalar_digest_limbs_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    scalar: AssignedValue<F>,
) -> [AssignedValue<F>; 2] {
    let bits = gate.num_to_bits(ctx, scalar, F::NUM_BITS as usize);
    std::array::from_fn(|limb| {
        let start = limb * 128;
        let end = (start + 128).min(bits.len());
        gate.inner_product(
            ctx,
            bits[start..end].iter().copied(),
            (0..end - start).map(|bit| Constant(from_u128::<F>(1_u128 << bit))),
        )
    })
}

fn constrain_equal_if_v1<F: halo2_base::utils::ScalarField>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    actual: AssignedValue<F>,
    expected: AssignedValue<F>,
    enabled: AssignedValue<F>,
) {
    let difference = gate.sub(ctx, Existing(actual), Existing(expected));
    let selected = gate.mul(ctx, Existing(difference), Existing(enabled));
    gate.assert_is_const(ctx, &selected, &F::ZERO);
}

fn terminal_seed_native<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    total_stages: u64,
    total_jobs: u32,
) -> F {
    let release = digest_limbs::<F>(release_id);
    hash(
        TERMINAL_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            F::from(total_stages),
            F::from(u64::from(total_jobs)),
        ],
    )
}

fn message_seed_native<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    total_stages: u64,
    total_jobs: u32,
) -> F {
    let release = digest_limbs::<F>(release_id);
    hash(
        MESSAGE_SEED_DOMAIN_V1,
        &[
            release[0],
            release[1],
            F::from(total_stages),
            F::from(u64::from(total_jobs)),
        ],
    )
}

fn message_step_native<F: KagemushaPoseidonFieldV1>(
    prior: F,
    leaf: &KagemushaMintHashShardStatementV1,
) -> F {
    let mut inputs = Vec::with_capacity(5 + BLOCK_SIZE);
    inputs.extend([
        prior,
        F::from(leaf.stage_index),
        F::from(u64::from(leaf.job_index)),
        F::from(u64::from(leaf.block_index)),
        F::from(u64::from(leaf.job_block_count)),
    ]);
    inputs.extend(leaf.block_words.map(|word| F::from(u64::from(word))));
    hash(MESSAGE_STEP_DOMAIN_V1, &inputs)
}

fn terminal_step_native<F: KagemushaPoseidonFieldV1>(
    prior: F,
    job_index: u32,
    job_block_count: u32,
    output_state: [u32; DIGEST_SIZE],
) -> F {
    let mut inputs = Vec::with_capacity(3 + DIGEST_SIZE);
    inputs.push(prior);
    inputs.push(F::from(u64::from(job_index)));
    inputs.push(F::from(u64::from(job_block_count)));
    inputs.extend(output_state.map(|word| F::from(u64::from(word))));
    hash(TERMINAL_STEP_DOMAIN_V1, &inputs)
}

fn plan_binding_native<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    total_stages: u64,
    total_jobs: u32,
    expected_message_root: F,
    expected_terminal_root: F,
) -> F {
    let release = digest_limbs::<F>(release_id);
    hash(
        PLAN_DOMAIN_V1,
        &[
            release[0],
            release[1],
            F::from(total_stages),
            F::from(u64::from(total_jobs)),
            expected_message_root,
            expected_terminal_root,
        ],
    )
}

/// Derive the public proof-chain root from the exact ordinary proof bytes consumed this step.
pub(crate) fn mint_hash_proof_chain_root_v1<F: KagemushaPoseidonFieldV1>(
    release_id: DigestV1,
    plan_binding: DigestV1,
    next_stage: u64,
    previous_root: Option<DigestV1>,
    parent_proof: &[u8],
    shard_proof: &[u8],
) -> Result<DigestV1, String> {
    if release_id == [0; 32] || plan_binding == [0; 32] || next_stage == 0 {
        return Err("mint hash proof-chain binding is missing its release/plan/stage".to_owned());
    }
    let prior = if let Some(previous) = previous_root {
        decode::<F>(previous)
            .ok_or_else(|| "mint hash prior proof-chain root is noncanonical".to_owned())?
    } else {
        let release = digest_limbs::<F>(release_id);
        let plan = digest_limbs::<F>(plan_binding);
        hash(
            PROOF_CHAIN_SEED_DOMAIN_V1,
            &[release[0], release[1], plan[0], plan[1]],
        )
    };
    let parent_chunks = canonical_byte_chunks_native_v1::<F>(parent_proof);
    let shard_chunks = canonical_byte_chunks_native_v1::<F>(shard_proof);
    let mut inputs = Vec::with_capacity(parent_chunks.len() + shard_chunks.len() + 4);
    inputs.extend([
        prior,
        F::from(next_stage),
        F::from(
            u64::try_from(parent_chunks.len())
                .map_err(|_| "mint hash parent proof chunk count exceeds u64".to_owned())?,
        ),
    ]);
    inputs.extend(parent_chunks);
    inputs.push(F::from(u64::try_from(shard_chunks.len()).map_err(
        |_| "mint hash shard proof chunk count exceeds u64".to_owned(),
    )?));
    inputs.extend(shard_chunks);
    Ok(encode(hash(PROOF_CHAIN_STEP_DOMAIN_V1, &inputs)))
}

fn canonical_byte_chunks_native_v1<F: PrimeField + From<u64>>(bytes: &[u8]) -> Vec<F> {
    bytes
        .chunks(16)
        .map(|chunk| {
            let mut canonical = [0_u8; 16];
            canonical[..chunk.len()].copy_from_slice(chunk);
            from_u128::<F>(u128::from_le_bytes(canonical))
        })
        .collect()
}

/// Require the helper basis to be the exact prefix of the authenticated monetary basis.
pub(crate) fn validate_mint_hash_shard_basis_prefix_v1<C>(
    carrier: &ParamsIPA<C>,
    shard: &ParamsIPA<C>,
) -> Result<(), String>
where
    C: CurveAffine + PartialEq,
{
    if carrier.k() != KAGEMUSHA_RECURSION_IPA_K_V1 || shard.k() != KAGEMUSHA_MINT_HASH_SHARD_K_V1 {
        return Err("mint hash shard/carrier IPA domains are not k=12/k=16".to_owned());
    }
    let expected = 1_usize << KAGEMUSHA_MINT_HASH_SHARD_K_V1;
    if shard.get_g().len() != expected
        || carrier.get_g().len() < expected
        || shard.get_g() != &carrier.get_g()[..expected]
    {
        return Err("mint hash shard generator basis is not the carrier prefix".to_owned());
    }
    Ok(())
}

/// Lift a `k = 12` opening claim into the exact first 4,096 slots of the `k = 16` history.
fn lift_mint_hash_shard_accumulator_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    shard: DeferredAccumulator<'chip, C>,
) -> Result<DeferredAccumulator<'chip, C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    if shard.xi.len() != KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize {
        return Err("mint hash shard accumulator has the wrong round count".to_owned());
    }
    let mut xi = Vec::with_capacity(KAGEMUSHA_RECURSION_IPA_K_V1 as usize);
    xi.extend((0..SHARD_TO_HISTORY_ZERO_ROUNDS_V1).map(|_| loader.load_const(&C::ScalarExt::ZERO)));
    xi.extend(shard.xi);
    if xi.len() != KAGEMUSHA_RECURSION_IPA_K_V1 as usize {
        return Err("mint hash shard accumulator lift shape drifted".to_owned());
    }
    Ok(IpaAccumulator::new(xi, shard.u))
}

/// Circuit-side typed-plan binding used by the eventual paired recursive step.
fn constrain_plan_binding_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    release: [AssignedValue<F>; 2],
    total_stages: AssignedValue<F>,
    total_jobs: AssignedValue<F>,
    expected_message_root: AssignedValue<F>,
    expected_terminal_root: AssignedValue<F>,
    expected_plan_binding: AssignedValue<F>,
) {
    let poseidon = KagemushaPoseidonChipV1::new(ctx, range);
    let actual = poseidon.hash(
        ctx,
        range,
        PLAN_DOMAIN_V1,
        &[
            release[0],
            release[1],
            total_stages,
            total_jobs,
            expected_message_root,
            expected_terminal_root,
        ],
    );
    ctx.constrain_equal(&actual, &expected_plan_binding);
}

const _: () = {
    assert!(KAGEMUSHA_MINT_HASH_SHARD_K_V1 < KAGEMUSHA_RECURSION_IPA_K_V1);
    assert!(SHARD_TO_HISTORY_ZERO_ROUNDS_V1 == 4);
};

#[cfg(test)]
mod tests {
    use super::*;
    use ff::Field;
    use halo2_proofs::halo2curves::{
        group::{Curve as _, Group as _},
        pasta::{Eq, EqAffine, Fp},
    };

    fn padded_coefficients<F: Field>(xi: &[F]) -> Vec<F> {
        let mut coefficients = vec![F::ZERO; 1 << xi.len()];
        coefficients[0] = F::ONE;
        for (len, challenge) in xi.iter().rev().enumerate().map(|(i, xi)| (1 << i, xi)) {
            let (left, right) = coefficients.split_at_mut(len);
            right[..len].copy_from_slice(left);
            for coefficient in &mut right[..len] {
                *coefficient *= challenge;
            }
        }
        coefficients
    }

    fn statements() -> (
        KagemushaMintHashClaimPlanV1,
        Vec<KagemushaMintHashShardStatementV1>,
    ) {
        use super::super::mint_hash_shard::KagemushaMintHashPlanV1;

        let release = [0x41; 32];
        let placeholder = [0x77; 32];
        let messages = vec![b"first mint job".to_vec(), vec![0x5a; 130]];
        let provisional = KagemushaMintHashPlanV1::from_messages(
            release,
            KagemushaPastaParityV1::Eq,
            placeholder,
            messages.clone(),
        )
        .unwrap();
        let plan =
            KagemushaMintHashClaimPlanV1::from_leaves::<Fp>(release, provisional.leaves()).unwrap();
        let exact = KagemushaMintHashPlanV1::from_messages(
            release,
            KagemushaPastaParityV1::Eq,
            plan.plan_binding,
            messages,
        )
        .unwrap();
        (plan, exact.leaves().to_vec())
    }

    #[test]
    fn zero_prefix_lift_preserves_exact_k12_coefficients_and_zeros_the_rest() {
        let xi = (1_u64..=u64::from(KAGEMUSHA_MINT_HASH_SHARD_K_V1))
            .map(Fp::from)
            .collect::<Vec<_>>();
        let small = padded_coefficients(&xi);
        let lifted_xi = [vec![Fp::ZERO; SHARD_TO_HISTORY_ZERO_ROUNDS_V1], xi].concat();
        let lifted = padded_coefficients(&lifted_xi);
        assert_eq!(&lifted[..small.len()], small);
        assert!(
            lifted[small.len()..]
                .iter()
                .all(|value| bool::from(value.is_zero()))
        );
    }

    #[test]
    fn generated_k12_basis_is_the_exact_k16_prefix_and_lift_decides() {
        let carrier = ParamsIPA::<EqAffine>::new(KAGEMUSHA_RECURSION_IPA_K_V1);
        let shard = ParamsIPA::<EqAffine>::new(KAGEMUSHA_MINT_HASH_SHARD_K_V1);
        validate_mint_hash_shard_basis_prefix_v1(&carrier, &shard).unwrap();

        let xi = (1_u64..=u64::from(KAGEMUSHA_MINT_HASH_SHARD_K_V1))
            .map(Fp::from)
            .collect::<Vec<_>>();
        let coefficients = padded_coefficients(&xi);
        let point = shard
            .get_g()
            .iter()
            .zip(coefficients)
            .fold(Eq::identity(), |sum, (base, scalar)| sum + *base * scalar)
            .to_affine();
        let lifted = IpaAccumulator::<EqAffine, NativeLoader>::new(
            [vec![Fp::ZERO; SHARD_TO_HISTORY_ZERO_ROUNDS_V1], xi].concat(),
            point,
        );
        let encoded = super::super::KagemushaEqAccumulatorV1::from_native(&lifted).unwrap();
        super::super::decide_kagemusha_eq_accumulator_v1(&carrier, &encoded).unwrap();
    }

    #[test]
    fn ordered_claim_rejects_missing_reordered_duplicated_and_substituted_leaves() {
        let (plan, leaves) = statements();
        let first = KagemushaMintHashClaimStateV1::apply::<Fp>(plan, None, &leaves[0]).unwrap();
        assert!(KagemushaMintHashClaimStateV1::apply::<Fp>(plan, Some(first), &leaves[0]).is_err());
        assert!(KagemushaMintHashClaimStateV1::apply::<Fp>(plan, Some(first), &leaves[2]).is_err());

        let mut substituted = leaves[1].clone();
        substituted.initial_state[0] ^= 1;
        assert!(
            KagemushaMintHashClaimStateV1::apply::<Fp>(plan, Some(first), &substituted).is_err()
        );
    }

    #[test]
    fn exact_plan_completes_and_no_count_admission_cap_exists() {
        let (plan, leaves) = statements();
        let mut state = None;
        for leaf in &leaves {
            state = Some(KagemushaMintHashClaimStateV1::apply::<Fp>(plan, state, leaf).unwrap());
        }
        let state = state.unwrap();
        assert!(state.complete);
        assert_eq!(state.next_stage, plan.total_stages);
        assert_eq!(state.next_job, plan.total_jobs);

        // The transition API accepts the arithmetic protocol range directly; there is no
        // hop/ancestry/proof-depth maximum or count-based admission constant.
        let huge = KagemushaMintHashClaimPlanV1::from_job_terminals_and_message_root::<Fp>(
            [0x31; 32],
            u64::from(u32::MAX),
            &[(u32::MAX, [0x1234_5678; DIGEST_SIZE])],
            encode(Fp::from(71)),
        )
        .unwrap();
        assert_eq!(huge.total_stages, u64::from(u32::MAX));
    }

    #[test]
    fn typed_plan_binding_rejects_count_and_terminal_substitution() {
        let (plan, _) = statements();
        let mut count = plan;
        count.total_stages += 1;
        assert!(count.validate::<Fp>().is_err());
        let mut terminal = plan;
        terminal.expected_terminal_root[0] ^= 1;
        assert!(terminal.validate::<Fp>().is_err());
        let mut message = plan;
        message.expected_message_root[0] ^= 1;
        assert!(message.validate::<Fp>().is_err());

        let terminals = [[0x1111_1111; DIGEST_SIZE], [0x2222_2222; DIGEST_SIZE]];
        let message_root = encode(Fp::from(73));
        let original = KagemushaMintHashClaimPlanV1::from_job_terminals_and_message_root::<Fp>(
            [0x31; 32],
            4,
            &[(1, terminals[0]), (3, terminals[1])],
            message_root,
        )
        .unwrap();
        let substituted = KagemushaMintHashClaimPlanV1::from_job_terminals_and_message_root::<Fp>(
            [0x31; 32],
            4,
            &[(2, terminals[0]), (2, terminals[1])],
            message_root,
        )
        .unwrap();
        assert_ne!(original.plan_binding, substituted.plan_binding);
    }

    #[test]
    fn proof_chain_rejects_omission_reordering_duplication_and_substitution() {
        let release = [0x31; 32];
        let plan = encode::<Fp>(Fp::from(41));
        let first = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            1,
            None,
            b"bootstrap-parent",
            b"shard-zero",
        )
        .unwrap();
        let ordered = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            b"parent-one",
            b"shard-one",
        )
        .unwrap();
        let omitted = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            None,
            b"parent-one",
            b"shard-one",
        )
        .unwrap();
        let reordered = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            b"shard-one",
            b"parent-one",
        )
        .unwrap();
        let duplicated = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            b"parent-one",
            b"parent-one",
        )
        .unwrap();
        let substituted = mint_hash_proof_chain_root_v1::<Fp>(
            release,
            plan,
            2,
            Some(first),
            b"parent-one",
            b"shard-two",
        )
        .unwrap();
        assert_ne!(ordered, omitted);
        assert_ne!(ordered, reordered);
        assert_ne!(ordered, duplicated);
        assert_ne!(ordered, substituted);
    }
}
