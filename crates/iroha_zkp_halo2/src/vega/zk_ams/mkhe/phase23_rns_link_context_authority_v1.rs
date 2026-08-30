//! Sole field and construction authority for the private RNS-Link context.

#![forbid(unsafe_code)]

use super::super::super::{ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1, ZkAmsProofContextV1, context_frame};
use super::super::{
    ZkAmsMkheErrorV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectEvaluatedKeySetAdmissionV1,
    manifest::release_profile_v1,
    phase23_encrypted::zk_ams_phase23_release_map_set_digest_v1,
    terminal::{
        ZkAmsPhase3GovernedBatchV1, ZkAmsPhase3TerminalContextV1,
        terminal_composition_context_frame, zk_ams_phase3_nifs_verifier_digest_v1,
    },
};
use super::{CONTEXT_DOMAIN_V1, RNS_LINK_VERSION_V1, immutable_algorithm_manifest_digest_v1};
use crate::vega::sponge::keccak256;

/// Leaf-private non-duplicable storage for one context axis. Its missing
/// `Clone` and `Copy` implementations make an external `Copy` implementation
/// for the containing context ill-formed in safe Rust.
struct ContextAxisDigestV1([u8; 32]);

/// Opaque binding of every immutable context axis that precedes an RNS-Link commitment.
///
/// The type is visible at the existing MKHE-private scope, while its fields remain visible only in
/// this leaf. Production construction is confined to the move-only native-source owner below.
pub(in super::super) struct ZkAmsPhase23RnsLinkContextV1 {
    profile_digest: ContextAxisDigestV1,
    algorithm_manifest_digest: ContextAxisDigestV1,
    network_context_digest: ContextAxisDigestV1,
    statement_context_digest: ContextAxisDigestV1,
    transcript_digest: ContextAxisDigestV1,
    batch_digest: ContextAxisDigestV1,
    roster_digest: ContextAxisDigestV1,
    direct_key_admission_digest: ContextAxisDigestV1,
    canonical_map_set_digest: ContextAxisDigestV1,
}

/// Move-only owner of one validated native-source correspondence.
///
/// It retains the terminal and direct-key cross-bindings until the exact
/// materialization call returns the sole context. There is no clone, codec,
/// borrowed context accessor, or tuple-decomposition path.
#[must_use = "dropping this owner destroys the sole Phase-23 context-return capability"]
pub(in super::super) struct ZkAmsPhase23RnsLinkContextOwnerV1 {
    profile_digest: [u8; 32],
    algorithm_manifest_digest: [u8; 32],
    network_context_digest: [u8; 32],
    statement_context_digest: [u8; 32],
    transcript_digest: [u8; 32],
    batch_digest: [u8; 32],
    roster_digest: [u8; 32],
    direct_key_admission_digest: [u8; 32],
    canonical_map_set_digest: [u8; 32],
    terminal_context: ZkAmsPhase3TerminalContextV1,
    governed_fold_count: u8,
    direct_collective_public_key_digest: [u8; 32],
    direct_key_material_digest: [u8; 32],
}

impl ZkAmsPhase23RnsLinkContextV1 {
    pub(super) fn digest(&self) -> [u8; 32] {
        let mut frame = Vec::with_capacity(CONTEXT_DOMAIN_V1.len() + 2 + 9 * 32);
        frame.extend_from_slice(CONTEXT_DOMAIN_V1);
        frame.push(RNS_LINK_VERSION_V1);
        frame.extend_from_slice(&self.profile_digest.0);
        frame.extend_from_slice(&self.algorithm_manifest_digest.0);
        frame.extend_from_slice(&self.network_context_digest.0);
        frame.extend_from_slice(&self.statement_context_digest.0);
        frame.extend_from_slice(&self.transcript_digest.0);
        frame.extend_from_slice(&self.batch_digest.0);
        frame.extend_from_slice(&self.roster_digest.0);
        frame.extend_from_slice(&self.direct_key_admission_digest.0);
        frame.extend_from_slice(&self.canonical_map_set_digest.0);
        keccak256(&frame)
    }

    pub(super) fn validated_release_binding_digests_v1(
        &self,
    ) -> Result<([u8; 32], [u8; 32]), ZkAmsMkheErrorV1> {
        let profile_digest = release_profile_v1().digest()?;
        let algorithm_manifest_digest = immutable_algorithm_manifest_digest_v1()?;
        let canonical_map_set_digest = zk_ams_phase23_release_map_set_digest_v1()?;
        if self.profile_digest.0 != profile_digest
            || self.algorithm_manifest_digest.0 != algorithm_manifest_digest
            || self.canonical_map_set_digest.0 != canonical_map_set_digest
            || [
                self.profile_digest.0,
                self.algorithm_manifest_digest.0,
                self.network_context_digest.0,
                self.statement_context_digest.0,
                self.transcript_digest.0,
                self.batch_digest.0,
                self.roster_digest.0,
                self.direct_key_admission_digest.0,
                self.canonical_map_set_digest.0,
            ]
            .contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok((profile_digest, algorithm_manifest_digest))
    }
}

impl ZkAmsPhase23RnsLinkContextOwnerV1 {
    /// Validate every native source and mint the sole production context owner.
    pub(in super::super) fn from_native_sources_v1(
        proof_context: &ZkAmsProofContextV1<'_>,
        terminal_context: ZkAmsPhase3TerminalContextV1,
        governed_batch: &ZkAmsPhase3GovernedBatchV1,
        direct_key_admission: ZkAmsMkheDirectEvaluatedKeySetAdmissionV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        // `context_frame` performs the canonical generic-context validation;
        // hashing caller fields directly would create an alternate network axis.
        let generic_context_frame =
            context_frame(proof_context).map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let network_context_digest = keccak256(&generic_context_frame);
        // This revalidates the terminal context, governed-batch digest, ordered
        // public inputs, and the batch-to-terminal context binding.
        drop(terminal_composition_context_frame(
            proof_context,
            terminal_context,
            governed_batch,
        )?);
        let profile_digest = release_profile_v1().digest()?;
        let algorithm_manifest_digest = immutable_algorithm_manifest_digest_v1()?;
        let canonical_map_set_digest = zk_ams_phase23_release_map_set_digest_v1()?;
        let governed_fold_count = u8::try_from(governed_batch.strict_public_inputs.len())
            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if terminal_context.profile_digest != profile_digest
            || terminal_context.nifs_verifier_digest != zk_ams_phase3_nifs_verifier_digest_v1()?
            || governed_batch.context_digest != terminal_context.digest
            || governed_batch.digest == [0; 32]
            || governed_fold_count == 0
            || governed_batch
                .strict_public_inputs
                .iter()
                .any(|row| row.len() != ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1)
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let (
            direct_key_admission_digest,
            direct_collective_public_key_digest,
            direct_key_material_digest,
        ) = direct_key_admission.validated_phase23_context_axes_v1(
            terminal_context.profile_digest,
            terminal_context.roster_digest,
            terminal_context.epoch,
            terminal_context.transcript_digest,
        )?;
        let statement_context_digest = proof_context.statement_digest;
        let transcript_digest = terminal_context.transcript_digest;
        let batch_digest = governed_batch.digest;
        let roster_digest = terminal_context.roster_digest;
        if [
            profile_digest,
            algorithm_manifest_digest,
            network_context_digest,
            statement_context_digest,
            transcript_digest,
            batch_digest,
            roster_digest,
            direct_key_admission_digest,
            canonical_map_set_digest,
        ]
        .contains(&[0; 32])
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(Self {
            profile_digest,
            algorithm_manifest_digest,
            network_context_digest,
            statement_context_digest,
            transcript_digest,
            batch_digest,
            roster_digest,
            direct_key_admission_digest,
            canonical_map_set_digest,
            terminal_context,
            governed_fold_count,
            direct_collective_public_key_digest,
            direct_key_material_digest,
        })
    }

    /// Consume the owner and return its context only for the exact live
    /// materialization and encryption-authority axes.
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn into_context_for_materialization_v1(
        self,
        profile_digest: [u8; 32],
        roster_digest: [u8; 32],
        epoch: u64,
        transcript_digest: [u8; 32],
        batch_id: [u8; 32],
        ordered_batch_input_digest: [u8; 32],
        fold_count: u8,
        collective_public_key_digest: [u8; 32],
        key_material_digest: [u8; 32],
    ) -> Result<ZkAmsPhase23RnsLinkContextV1, ZkAmsMkheErrorV1> {
        let Self {
            profile_digest: context_profile_digest,
            algorithm_manifest_digest,
            network_context_digest,
            statement_context_digest,
            transcript_digest: context_transcript_digest,
            batch_digest,
            roster_digest: context_roster_digest,
            direct_key_admission_digest,
            canonical_map_set_digest,
            terminal_context,
            governed_fold_count,
            direct_collective_public_key_digest,
            direct_key_material_digest,
        } = self;
        if profile_digest != terminal_context.profile_digest
            || roster_digest != terminal_context.roster_digest
            || epoch != terminal_context.epoch
            || transcript_digest != terminal_context.transcript_digest
            || batch_id != terminal_context.batch_id
            || ordered_batch_input_digest != terminal_context.ordered_batch_input_digest
            || fold_count != governed_fold_count
            || collective_public_key_digest != direct_collective_public_key_digest
            || key_material_digest != direct_key_material_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let context = ZkAmsPhase23RnsLinkContextV1 {
            profile_digest: ContextAxisDigestV1(context_profile_digest),
            algorithm_manifest_digest: ContextAxisDigestV1(algorithm_manifest_digest),
            network_context_digest: ContextAxisDigestV1(network_context_digest),
            statement_context_digest: ContextAxisDigestV1(statement_context_digest),
            transcript_digest: ContextAxisDigestV1(context_transcript_digest),
            batch_digest: ContextAxisDigestV1(batch_digest),
            roster_digest: ContextAxisDigestV1(context_roster_digest),
            direct_key_admission_digest: ContextAxisDigestV1(direct_key_admission_digest),
            canonical_map_set_digest: ContextAxisDigestV1(canonical_map_set_digest),
        };
        context.validated_release_binding_digests_v1()?;
        Ok(context)
    }
}

#[cfg(test)]
impl ZkAmsPhase23RnsLinkContextV1 {
    #[allow(clippy::too_many_arguments)]
    pub(in super::super) fn new(
        network_context_digest: [u8; 32],
        statement_context_digest: [u8; 32],
        transcript_digest: [u8; 32],
        batch_digest: [u8; 32],
        roster_digest: [u8; 32],
        direct_key_admission_digest: [u8; 32],
        canonical_map_set_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        let context = Self {
            profile_digest: ContextAxisDigestV1(release_profile_v1().digest()?),
            algorithm_manifest_digest: ContextAxisDigestV1(
                immutable_algorithm_manifest_digest_v1()?
            ),
            network_context_digest: ContextAxisDigestV1(network_context_digest),
            statement_context_digest: ContextAxisDigestV1(statement_context_digest),
            transcript_digest: ContextAxisDigestV1(transcript_digest),
            batch_digest: ContextAxisDigestV1(batch_digest),
            roster_digest: ContextAxisDigestV1(roster_digest),
            direct_key_admission_digest: ContextAxisDigestV1(direct_key_admission_digest),
            canonical_map_set_digest: ContextAxisDigestV1(canonical_map_set_digest),
        };
        context.validated_release_binding_digests_v1()?;
        Ok(context)
    }

    /// Produce one bounded hostile fixture without exposing a field or a raw
    /// production constructor. There are exactly nine 32-byte context axes.
    pub(super) fn with_test_axis_byte_flipped_v1(&self, axis: usize, byte: usize) -> Option<Self> {
        if axis >= 9 || byte >= 32 {
            return None;
        }
        let mut changed = Self {
            profile_digest: ContextAxisDigestV1(self.profile_digest.0),
            algorithm_manifest_digest: ContextAxisDigestV1(self.algorithm_manifest_digest.0),
            network_context_digest: ContextAxisDigestV1(self.network_context_digest.0),
            statement_context_digest: ContextAxisDigestV1(self.statement_context_digest.0),
            transcript_digest: ContextAxisDigestV1(self.transcript_digest.0),
            batch_digest: ContextAxisDigestV1(self.batch_digest.0),
            roster_digest: ContextAxisDigestV1(self.roster_digest.0),
            direct_key_admission_digest: ContextAxisDigestV1(self.direct_key_admission_digest.0),
            canonical_map_set_digest: ContextAxisDigestV1(self.canonical_map_set_digest.0),
        };
        let selected = match axis {
            0 => &mut changed.profile_digest,
            1 => &mut changed.algorithm_manifest_digest,
            2 => &mut changed.network_context_digest,
            3 => &mut changed.statement_context_digest,
            4 => &mut changed.transcript_digest,
            5 => &mut changed.batch_digest,
            6 => &mut changed.roster_digest,
            7 => &mut changed.direct_key_admission_digest,
            8 => &mut changed.canonical_map_set_digest,
            _ => return None,
        };
        selected.0[byte] ^= 1;
        Some(changed)
    }
}
