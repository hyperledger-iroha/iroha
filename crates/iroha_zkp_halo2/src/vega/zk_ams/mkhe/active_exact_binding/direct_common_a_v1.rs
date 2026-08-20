//! Typed authority for the deterministic direct-ceremony common `a`.
//!
//! The common polynomial is ceremony-global, so its derivation deliberately
//! has no party axis. Party identity remains bound by each contribution
//! statement and by the move-only direct-relation capability.

use super::super::direct_collective_eval_ceremony::{
    ZkAmsMkheDirectCeremonyContextV1, ZkAmsMkheDirectEvaluatedKeyTargetV1,
};
use super::{
    PersistentDirectRelationUseSelectorV1, PersistentDirectRelationV1, PersistentWitnessConsumerV1,
    VerifiedPersistentWitnessBindingSetV1,
};
use crate::vega::{
    sponge::{Keccak256, Shake256Reader},
    zk_ams::mkhe::{
        MAX_RANDOM_REJECTION_ATTEMPTS_V1, MKHE_VERSION_V1, ZkAmsMkheErrorV1,
        active::ZkAmsMkheGovernedActiveRosterV1,
        manifest::{ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, release_profile_v1},
    },
};
#[path = "direct_common_a_v1/creator_replay_v1.rs"]
mod creator_replay_v1;
pub(super) use creator_replay_v1::{
    CompletedDirectCommonACreatorAuthorityV1, DirectCommonACreatorH0ReadyV1,
    DirectCommonACreatorH0ReplayV1, DirectCommonACreatorH1ReadyV1, DirectCommonACreatorH1ReplayV1,
    consume_completed_creator_authority_v1, prepare_direct_common_a_creator_h0_v1,
};
const DIRECT_COMMON_A_DERIVATION_ALGORITHM_V1: u8 = 1;
const DIRECT_COMMON_A_SAMPLER_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-common-a-limb";
const DIRECT_COMMON_A_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-common-a-statement";
const DIRECT_COMMON_A_AUTHORITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-common-a-authority";
const DIRECT_COMMON_A_CONTEXT_BYTES_V1: usize = 305;
const DIRECT_COMMON_A_RELEASE_LIMBS_V1: usize = 38;
const DIRECT_COMMON_A_LIMB_WORKSPACE_BYTES_V1: usize =
    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * core::mem::size_of::<u64>();
const DIRECT_COMMON_A_MAX_CANDIDATES_V1: u64 = DIRECT_COMMON_A_RELEASE_LIMBS_V1 as u64
    * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u64
    * MAX_RANDOM_REJECTION_ATTEMPTS_V1 as u64;
const _: () = {
    assert!(DIRECT_COMMON_A_RELEASE_LIMBS_V1 == 38);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(DIRECT_COMMON_A_LIMB_WORKSPACE_BYTES_V1 == 1_048_576);
    assert!(DIRECT_COMMON_A_MAX_CANDIDATES_V1 == 637_534_208);
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectCommonAAxesV1 {
    profile_digest: [u8; 32],
    context_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_public_key_digest: [u8; 32],
    secret_lineage_root: [u8; 32],
    target_tag: u8,
    evaluated_key_ordinal: u8,
    digit_index: u8,
    galois_exponent: u32,
    common_a_seed: [u8; 32],
    initial_round_digest: [u8; 32],
}
impl DirectCommonAAxesV1 {
    fn from_context(context: ZkAmsMkheDirectCeremonyContextV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        profile.validate()?;
        let target_tag = match context.target() {
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => 1,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { .. } => {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        };
        let axes = Self {
            profile_digest: context.profile_digest(),
            context_digest: context.digest(),
            roster_digest: context.roster_digest(),
            key_material_digest: context.key_material_digest(),
            epoch: context.epoch(),
            transcript_digest: context.transcript_digest(),
            collective_public_key_digest: context.collective_public_key_digest(),
            secret_lineage_root: context.secret_lineage_root(),
            target_tag,
            evaluated_key_ordinal: context.evaluated_key_ordinal(),
            digit_index: context.digit_index(),
            galois_exponent: context.galois_exponent(),
            common_a_seed: context.common_a_seed(),
            initial_round_digest: context.initial_round_digest(),
        };
        if axes.profile_digest != profile.digest()?
            || axes.context_digest == [0; 32]
            || axes.roster_digest == [0; 32]
            || axes.key_material_digest == [0; 32]
            || axes.epoch == 0
            || axes.transcript_digest == [0; 32]
            || axes.collective_public_key_digest == [0; 32]
            || axes.secret_lineage_root == [0; 32]
            || axes.target_tag != 1
            || axes.evaluated_key_ordinal != 0
            || axes.galois_exponent != 0
            || axes.common_a_seed == [0; 32]
            || axes.initial_round_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(axes)
    }
    fn encode(self) -> [u8; DIRECT_COMMON_A_CONTEXT_BYTES_V1] {
        let mut bytes = [0_u8; DIRECT_COMMON_A_CONTEXT_BYTES_V1];
        let mut cursor = 0;
        put(
            &mut bytes,
            &mut cursor,
            &[MKHE_VERSION_V1, DIRECT_COMMON_A_DERIVATION_ALGORITHM_V1],
        );
        for digest in [
            self.profile_digest,
            self.context_digest,
            self.roster_digest,
            self.key_material_digest,
        ] {
            put(&mut bytes, &mut cursor, &digest);
        }
        put(&mut bytes, &mut cursor, &self.epoch.to_be_bytes());
        for digest in [
            self.transcript_digest,
            self.collective_public_key_digest,
            self.secret_lineage_root,
        ] {
            put(&mut bytes, &mut cursor, &digest);
        }
        put(
            &mut bytes,
            &mut cursor,
            &[
                self.target_tag,
                self.evaluated_key_ordinal,
                self.digit_index,
            ],
        );
        put(&mut bytes, &mut cursor, &self.galois_exponent.to_be_bytes());
        put(&mut bytes, &mut cursor, &self.common_a_seed);
        put(&mut bytes, &mut cursor, &self.initial_round_digest);
        debug_assert_eq!(cursor, bytes.len());
        bytes
    }
}
/// Sequential one-limb derivation and statement-hash transaction.
struct DirectCommonAStatementStreamV1 {
    axes: DirectCommonAAxesV1,
    context_frame: [u8; DIRECT_COMMON_A_CONTEXT_BYTES_V1],
    next_limb: usize,
    remaining_candidates: u64,
    statement_hash: Keccak256,
    failed: bool,
    #[cfg(test)]
    inject_unwind_on_next_derive: bool,
}
impl DirectCommonAStatementStreamV1 {
    fn begin(context: ZkAmsMkheDirectCeremonyContextV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let axes = DirectCommonAAxesV1::from_context(context)?;
        Self::begin_with_axes(axes)
    }
    fn begin_with_axes(axes: DirectCommonAAxesV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let context_frame = axes.encode();
        let mut statement_hash = Keccak256::new();
        statement_hash.update(DIRECT_COMMON_A_STATEMENT_DOMAIN_V1);
        statement_hash.update(&[MKHE_VERSION_V1, DIRECT_COMMON_A_DERIVATION_ALGORITHM_V1]);
        statement_hash.update(&(DIRECT_COMMON_A_CONTEXT_BYTES_V1 as u32).to_be_bytes());
        statement_hash.update(&context_frame);
        statement_hash.update(
            &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        statement_hash.update(
            &u16::try_from(DIRECT_COMMON_A_RELEASE_LIMBS_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        Ok(Self {
            axes,
            context_frame,
            next_limb: 0,
            remaining_candidates: DIRECT_COMMON_A_MAX_CANDIDATES_V1,
            statement_hash,
            failed: false,
            #[cfg(test)]
            inject_unwind_on_next_derive: false,
        })
    }
    #[cfg(test)]
    fn begin_for_test(axes: DirectCommonAAxesV1) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::begin_with_axes(axes)
    }
    fn derive_next_limb_into(&mut self, output: &mut [u64]) -> Result<(), ZkAmsMkheErrorV1> {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.failed = true;
        #[cfg(test)]
        if core::mem::replace(&mut self.inject_unwind_on_next_derive, false) {
            panic!("injected direct common-a replay derive unwind");
        }
        let result = self.derive_next_limb_inner(output);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }
    fn derive_next_limb_inner(&mut self, output: &mut [u64]) -> Result<(), ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        let modulus = profile
            .moduli
            .get(self.next_limb)
            .copied()
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        if profile.moduli.len() != DIRECT_COMMON_A_RELEASE_LIMBS_V1
            || output.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let limb =
            u16::try_from(self.next_limb).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let frame = sampler_frame(self.context_frame, limb, modulus)?;
        let zone = u64::MAX - u64::MAX % modulus;
        let mut stream = Shake256Reader::new(&frame);
        for coefficient in output.iter_mut() {
            *coefficient =
                sample_residue(modulus, zone, &mut self.remaining_candidates, |bytes| {
                    stream.read(bytes)
                })?;
        }
        self.statement_hash.update(&limb.to_be_bytes());
        self.statement_hash.update(&modulus.to_be_bytes());
        self.statement_hash.update(
            &u32::try_from(output.len())
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        for residue in output {
            self.statement_hash.update(&residue.to_be_bytes());
        }
        self.next_limb += 1;
        Ok(())
    }
    fn finish(self) -> Result<VerifiedDirectCommonAStatementV1, ZkAmsMkheErrorV1> {
        if self.failed || self.next_limb != DIRECT_COMMON_A_RELEASE_LIMBS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let statement_digest = self.statement_hash.finalize();
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let authority_digest = authority_digest(self.axes, statement_digest);
        Ok(VerifiedDirectCommonAStatementV1 {
            axes: self.axes,
            statement_digest,
            authority_digest,
        })
    }
}
/// Move-only proof that the exact ordered CPK authority derived common `a`.
struct VerifiedDirectCommonAStatementV1 {
    axes: DirectCommonAAxesV1,
    statement_digest: [u8; 32],
    authority_digest: [u8; 32],
}
impl VerifiedDirectCommonAStatementV1 {
    fn statement_digest_for(
        &self,
        context: ZkAmsMkheDirectCeremonyContextV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let axes = DirectCommonAAxesV1::from_context(context)?;
        if axes != self.axes
            || self.statement_digest == [0; 32]
            || self.authority_digest != authority_digest(self.axes, self.statement_digest)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(self.statement_digest)
    }
}
/// Single-use replay of the deterministic round-one common-`a` statement.
///
/// The expected statement remains inside this transaction. Callers may borrow each derived limb for
/// relation reconstruction, but cannot extract a digest or turn a partial replay into authority.
pub(super) struct DirectCommonAReplayV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    expected_statement_digest: [u8; 32],
    stream: DirectCommonAStatementStreamV1,
}
impl core::fmt::Debug for DirectCommonAReplayV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DirectCommonAReplayV1")
            .field("context", &"[REDACTED]")
            .field("expected_statement_digest", &"[REDACTED]")
            .field("stream", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}
/// Opaque proof that all 38 common-`a` limbs matched the typed selector.
pub(super) struct CompletedDirectCommonAReplayV1 {
    _seal: (),
}
impl DirectCommonAReplayV1 {
    /// Begin only from the exact round-one capability and ceremony context.
    pub(super) fn begin(
        context: ZkAmsMkheDirectCeremonyContextV1,
        capability: &super::VerifiedPersistentWitnessDirectRelationUseV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        capability.validate()?;
        let selector = capability.selector;
        if selector.relation != PersistentDirectRelationV1::RkgRoundOne
            || selector.context_digest != context.digest()
            || selector.evaluated_key_ordinal != context.evaluated_key_ordinal()
            || selector.digit_index != context.digit_index()
            || selector.galois_exponent != context.galois_exponent()
            || capability.binding_set_root != context.secret_lineage_root()
            || capability.collective_public_key_digest != context.collective_public_key_digest()
            || context.direct_secret_lineage_digest(capability.party_index)?
                != capability.secret_identity_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            context,
            expected_statement_digest: selector.common_a_statement_digest,
            stream: DirectCommonAStatementStreamV1::begin(context)?,
        })
    }
    /// Derive the next complete limb into caller-owned fixed workspace.
    ///
    /// The underlying stream poisons itself before every fallible operation,
    /// including an unwind, and therefore cannot resume after failure.
    pub(super) fn derive_next_limb_into(
        &mut self,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.stream.derive_next_limb_into(output)
    }
    /// Arm one deterministic unwind after the underlying stream poisons itself.
    #[cfg(test)]
    pub(super) fn inject_unwind_on_next_derive_for_test(&mut self) {
        self.stream.inject_unwind_on_next_derive = true;
    }
    /// Consume a complete replay and compare its typed statement internally.
    pub(super) fn finish(self) -> Result<CompletedDirectCommonAReplayV1, ZkAmsMkheErrorV1> {
        let observed = self.stream.finish()?.statement_digest_for(self.context)?;
        if observed != self.expected_statement_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(CompletedDirectCommonAReplayV1 { _seal: () })
    }
}
/// Derive one common-`a` statement only from the exact ordered CPK binding set.
fn derive_verified_direct_common_a_statement_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
) -> Result<VerifiedDirectCommonAStatementV1, ZkAmsMkheErrorV1> {
    bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::RkgRoundOne)?;
    context.validate_rkg_ephemeral_membership_axes(roster, bindings)?;
    let mut workspace = Vec::new();
    workspace
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    workspace.resize(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, 0_u64);
    let mut stream = DirectCommonAStatementStreamV1::begin(context)?;
    for _ in 0..DIRECT_COMMON_A_RELEASE_LIMBS_V1 {
        stream.derive_next_limb_into(&mut workspace)?;
    }
    stream.finish()
}

/// Consume one private common-`a` authority into the only valid round-one selector shape. Neither
/// the authority nor its raw digest crosses this module boundary.
fn new_rkg_round_one_selector_v1(
    context: ZkAmsMkheDirectCeremonyContextV1,
    prior_round_digest: [u8; 32],
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
    common_a: VerifiedDirectCommonAStatementV1,
) -> Result<PersistentDirectRelationUseSelectorV1, ZkAmsMkheErrorV1> {
    let common_a_statement_digest = common_a.statement_digest_for(context)?;
    let selector = PersistentDirectRelationUseSelectorV1 {
        relation: PersistentDirectRelationV1::RkgRoundOne,
        context_digest: context.digest(),
        prior_round_digest,
        evaluated_key_ordinal: context.evaluated_key_ordinal(),
        digit_index: context.digit_index(),
        galois_exponent: context.galois_exponent(),
        common_a_statement_digest,
        target_a_statement_digest: [0; 32],
        aggregate_h0_statement_digest: [0; 32],
        aggregate_h1_statement_digest: [0; 32],
        contribution_statement_digest,
        proof_commitment_transcript_digest,
    };
    selector.validate()?;
    Ok(selector)
}

/// Legacy selector wrapper retained only for verifier-side compatibility tests.
#[cfg(test)]
pub(super) fn mint_rkg_round_one_selector_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    prior_round_digest: [u8; 32],
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
) -> Result<PersistentDirectRelationUseSelectorV1, ZkAmsMkheErrorV1> {
    let common_a = derive_verified_direct_common_a_statement_v1(roster, bindings, context)?;
    new_rkg_round_one_selector_v1(
        context,
        prior_round_digest,
        contribution_statement_digest,
        proof_commitment_transcript_digest,
        common_a,
    )
}
#[cfg(test)]
pub(super) fn mint_mismatched_rkg_round_one_selector_for_test_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    authority_context: ZkAmsMkheDirectCeremonyContextV1,
    selector_context: ZkAmsMkheDirectCeremonyContextV1,
    prior_round_digest: [u8; 32],
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
) -> Result<(), ZkAmsMkheErrorV1> {
    let common_a =
        derive_verified_direct_common_a_statement_v1(roster, bindings, authority_context)?;
    new_rkg_round_one_selector_v1(
        selector_context,
        prior_round_digest,
        contribution_statement_digest,
        proof_commitment_transcript_digest,
        common_a,
    )
    .map(|_| ())
}
fn sampler_frame(
    context_frame: [u8; DIRECT_COMMON_A_CONTEXT_BYTES_V1],
    limb: u16,
    modulus: u64,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    if release_profile_v1().moduli.get(usize::from(limb)).copied() != Some(modulus) {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut frame = Vec::new();
    let capacity = DIRECT_COMMON_A_SAMPLER_DOMAIN_V1
        .len()
        .checked_add(2 + 4 + DIRECT_COMMON_A_CONTEXT_BYTES_V1 + 2 + 8 + 4)
        .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    frame
        .try_reserve_exact(capacity)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    frame.extend_from_slice(DIRECT_COMMON_A_SAMPLER_DOMAIN_V1);
    frame.extend_from_slice(&[MKHE_VERSION_V1, DIRECT_COMMON_A_DERIVATION_ALGORITHM_V1]);
    frame.extend_from_slice(&(DIRECT_COMMON_A_CONTEXT_BYTES_V1 as u32).to_be_bytes());
    frame.extend_from_slice(&context_frame);
    frame.extend_from_slice(&limb.to_be_bytes());
    frame.extend_from_slice(&modulus.to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    if frame.len() != capacity {
        return Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded);
    }
    Ok(frame)
}
fn sample_residue<F>(
    modulus: u64,
    zone: u64,
    remaining_candidates: &mut u64,
    mut read: F,
) -> Result<u64, ZkAmsMkheErrorV1>
where
    F: FnMut(&mut [u8; 8]),
{
    for _ in 0..MAX_RANDOM_REJECTION_ATTEMPTS_V1 {
        *remaining_candidates = remaining_candidates
            .checked_sub(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let mut bytes = [0_u8; 8];
        read(&mut bytes);
        let candidate = u64::from_le_bytes(bytes);
        if candidate < zone {
            return Ok(candidate % modulus);
        }
    }
    Err(ZkAmsMkheErrorV1::InvalidProfile)
}
fn authority_digest(axes: DirectCommonAAxesV1, statement_digest: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_COMMON_A_AUTHORITY_DOMAIN_V1);
    hash.update(&[MKHE_VERSION_V1, DIRECT_COMMON_A_DERIVATION_ALGORITHM_V1]);
    hash.update(&(DIRECT_COMMON_A_CONTEXT_BYTES_V1 as u32).to_be_bytes());
    hash.update(&axes.encode());
    hash.update(&statement_digest);
    hash.finalize()
}
fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) {
    let end = *cursor + bytes.len();
    output[*cursor..end].copy_from_slice(bytes);
    *cursor = end;
}

#[cfg(test)]
#[path = "direct_common_a_v1_tests.rs"]
mod tests;
