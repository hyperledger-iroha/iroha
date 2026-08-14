//! Typed authority for deterministic direct-ceremony Galois target `a`.
//!
//! Target `a` is ceremony-global for one schedule entry and gadget digit, so
//! its derivation deliberately has no party axis. The exact ordered CPK
//! binding set is the sole production authority. This precursor mints only a
//! typed selector; it does not mint a proof, receipt, capability, admission,
//! or release signal.

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
        packing::{
            ZK_AMS_T256_GALOIS_KEY_SCHEDULE_DIGEST_V1, validate_zk_ams_t256_galois_key_schedule_v1,
            zk_ams_t256_galois_key_schedule_v1,
        },
    },
};

const DIRECT_GALOIS_TARGET_A_DERIVATION_ALGORITHM_V1: u8 = 1;
const DIRECT_GALOIS_TARGET_A_SAMPLER_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-galois-target-a-limb";
const DIRECT_GALOIS_TARGET_A_STATEMENT_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-galois-target-a-statement";
const DIRECT_GALOIS_TARGET_A_AUTHORITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.direct-collective-galois-target-a-authority";
const DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1: usize = 305;
const DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1: usize = 38;
const DIRECT_GALOIS_TARGET_A_RESIDUES_V1: usize =
    DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1;
const DIRECT_GALOIS_TARGET_A_CANONICAL_RESIDUE_BYTES_V1: usize =
    DIRECT_GALOIS_TARGET_A_RESIDUES_V1 * core::mem::size_of::<u64>();
const DIRECT_GALOIS_TARGET_A_LIMB_WORKSPACE_BYTES_V1: usize =
    ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * core::mem::size_of::<u64>();
const DIRECT_GALOIS_TARGET_A_MAX_CANDIDATES_V1: u64 =
    DIRECT_GALOIS_TARGET_A_RESIDUES_V1 as u64 * MAX_RANDOM_REJECTION_ATTEMPTS_V1 as u64;
const DIRECT_GALOIS_TARGET_A_SAMPLER_FRAME_BYTES_V1: usize =
    DIRECT_GALOIS_TARGET_A_SAMPLER_DOMAIN_V1.len()
        + 2
        + 4
        + DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1
        + 2
        + 8
        + 4;
const _: () = {
    assert!(DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1 == 38);
    assert!(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 == 131_072);
    assert!(DIRECT_GALOIS_TARGET_A_RESIDUES_V1 == 4_980_736);
    assert!(DIRECT_GALOIS_TARGET_A_CANONICAL_RESIDUE_BYTES_V1 == 39_845_888);
    assert!(DIRECT_GALOIS_TARGET_A_LIMB_WORKSPACE_BYTES_V1 == 1_048_576);
    assert!(DIRECT_GALOIS_TARGET_A_MAX_CANDIDATES_V1 == 637_534_208);
    assert!(DIRECT_GALOIS_TARGET_A_SAMPLER_DOMAIN_V1.len() == 59);
    assert!(
        DIRECT_GALOIS_TARGET_A_SAMPLER_FRAME_BYTES_V1
            == DIRECT_GALOIS_TARGET_A_SAMPLER_DOMAIN_V1.len() + 325
    );
    assert!(DIRECT_GALOIS_TARGET_A_SAMPLER_FRAME_BYTES_V1 == 384);
    let mut galois_schedule_digest_or = 0_u8;
    let mut galois_schedule_digest_index = 0;
    while galois_schedule_digest_index < ZK_AMS_T256_GALOIS_KEY_SCHEDULE_DIGEST_V1.len() {
        galois_schedule_digest_or |=
            ZK_AMS_T256_GALOIS_KEY_SCHEDULE_DIGEST_V1[galois_schedule_digest_index];
        galois_schedule_digest_index += 1;
    }
    assert!(galois_schedule_digest_or != 0);
};

#[derive(PartialEq, Eq)]
struct DirectGaloisTargetAAxesV1 {
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
    target_a_seed: [u8; 32],
    initial_round_digest: [u8; 32],
}
impl DirectGaloisTargetAAxesV1 {
    fn from_context(context: ZkAmsMkheDirectCeremonyContextV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let profile = release_profile_v1();
        profile.validate()?;
        let schedule = zk_ams_t256_galois_key_schedule_v1()?;
        validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
        if schedule.digest != ZK_AMS_T256_GALOIS_KEY_SCHEDULE_DIGEST_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let schedule_index = match context.target() {
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index } => schedule_index,
        };
        let entry = schedule
            .entries
            .get(usize::from(schedule_index))
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let expected_ordinal = schedule_index
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)?;
        let axes = Self {
            profile_digest: context.profile_digest(),
            context_digest: context.digest(),
            roster_digest: context.roster_digest(),
            key_material_digest: context.key_material_digest(),
            epoch: context.epoch(),
            transcript_digest: context.transcript_digest(),
            collective_public_key_digest: context.collective_public_key_digest(),
            secret_lineage_root: context.secret_lineage_root(),
            target_tag: 2,
            evaluated_key_ordinal: context.evaluated_key_ordinal(),
            digit_index: context.digit_index(),
            galois_exponent: context.galois_exponent(),
            target_a_seed: context.target_a_seed(),
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
            || axes.target_tag != 2
            || axes.evaluated_key_ordinal != expected_ordinal
            || axes.galois_exponent != entry.exponent
            || axes.galois_exponent <= 1
            || axes.galois_exponent % 2 != 1
            || axes.target_a_seed == [0; 32]
            || axes.initial_round_digest == [0; 32]
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(axes)
    }

    fn encode(&self) -> [u8; DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1] {
        let mut bytes = [0_u8; DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1];
        let mut cursor = 0;
        put(
            &mut bytes,
            &mut cursor,
            &[
                MKHE_VERSION_V1,
                DIRECT_GALOIS_TARGET_A_DERIVATION_ALGORITHM_V1,
            ],
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
        put(&mut bytes, &mut cursor, &self.target_a_seed);
        put(&mut bytes, &mut cursor, &self.initial_round_digest);
        debug_assert_eq!(cursor, bytes.len());
        bytes
    }
}

/// Sequential one-limb derivation and statement-hash transaction.
struct DirectGaloisTargetAStatementStreamV1 {
    axes: DirectGaloisTargetAAxesV1,
    context_frame: [u8; DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1],
    next_limb: usize,
    remaining_candidates: u64,
    statement_hash: Keccak256,
    failed: bool,
    #[cfg(test)]
    inject_unwind_on_next_derive: bool,
}
impl DirectGaloisTargetAStatementStreamV1 {
    fn begin(context: ZkAmsMkheDirectCeremonyContextV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let axes = DirectGaloisTargetAAxesV1::from_context(context)?;
        Self::begin_with_axes(axes)
    }

    fn begin_with_axes(axes: DirectGaloisTargetAAxesV1) -> Result<Self, ZkAmsMkheErrorV1> {
        let context_frame = axes.encode();
        let mut statement_hash = Keccak256::new();
        statement_hash.update(DIRECT_GALOIS_TARGET_A_STATEMENT_DOMAIN_V1);
        statement_hash.update(&[
            MKHE_VERSION_V1,
            DIRECT_GALOIS_TARGET_A_DERIVATION_ALGORITHM_V1,
        ]);
        statement_hash.update(&(DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1 as u32).to_be_bytes());
        statement_hash.update(&context_frame);
        statement_hash.update(
            &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        statement_hash.update(
            &u16::try_from(DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1)
                .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
                .to_be_bytes(),
        );
        Ok(Self {
            axes,
            context_frame,
            next_limb: 0,
            remaining_candidates: DIRECT_GALOIS_TARGET_A_MAX_CANDIDATES_V1,
            statement_hash,
            failed: false,
            #[cfg(test)]
            inject_unwind_on_next_derive: false,
        })
    }

    #[cfg(test)]
    fn begin_for_test(axes: DirectGaloisTargetAAxesV1) -> Result<Self, ZkAmsMkheErrorV1> {
        Self::begin_with_axes(axes)
    }

    fn derive_next_limb_into(&mut self, output: &mut [u64]) -> Result<(), ZkAmsMkheErrorV1> {
        if self.failed {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.failed = true;
        #[cfg(test)]
        if core::mem::replace(&mut self.inject_unwind_on_next_derive, false) {
            panic!("injected direct galois target-a replay derive unwind");
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
        if profile.moduli.len() != DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1
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

    fn finish(self) -> Result<VerifiedDirectGaloisTargetAStatementV1, ZkAmsMkheErrorV1> {
        if self.failed || self.next_limb != DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let statement_digest = self.statement_hash.finalize();
        if statement_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let authority_digest = authority_digest(&self.axes, statement_digest);
        if authority_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(VerifiedDirectGaloisTargetAStatementV1 {
            axes: self.axes,
            statement_digest,
            authority_digest,
        })
    }
}

/// Move-only proof that the exact ordered CPK authority derived target `a`.
struct VerifiedDirectGaloisTargetAStatementV1 {
    axes: DirectGaloisTargetAAxesV1,
    statement_digest: [u8; 32],
    authority_digest: [u8; 32],
}
impl VerifiedDirectGaloisTargetAStatementV1 {
    fn statement_digest_for(
        self,
        context: ZkAmsMkheDirectCeremonyContextV1,
    ) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        let axes = DirectGaloisTargetAAxesV1::from_context(context)?;
        if axes != self.axes
            || self.statement_digest == [0; 32]
            || self.authority_digest == [0; 32]
            || self.authority_digest != authority_digest(&self.axes, self.statement_digest)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(self.statement_digest)
    }
}

/// Single-use replay of deterministic Galois target `a`.
///
/// The expected statement stays inside this transaction. Callers may borrow
/// each complete limb for relation reconstruction, but cannot extract a
/// digest or turn a partial replay into authority.
pub(super) struct DirectGaloisTargetAReplayV1 {
    context: ZkAmsMkheDirectCeremonyContextV1,
    expected_statement_digest: [u8; 32],
    stream: DirectGaloisTargetAStatementStreamV1,
}
impl core::fmt::Debug for DirectGaloisTargetAReplayV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DirectGaloisTargetAReplayV1")
            .field("context", &"[REDACTED]")
            .field("expected_statement_digest", &"[REDACTED]")
            .field("stream", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

/// Opaque proof that all 38 target-`a` limbs matched the typed selector.
struct DirectGaloisTargetACompletionSealV1 {
    _non_copy: Vec<core::convert::Infallible>,
}
pub(super) struct CompletedDirectGaloisTargetAReplayV1 {
    _seal: DirectGaloisTargetACompletionSealV1,
}
impl DirectGaloisTargetAReplayV1 {
    /// Begin only from the exact Galois capability and ceremony context.
    pub(super) fn begin(
        context: ZkAmsMkheDirectCeremonyContextV1,
        capability: &super::VerifiedPersistentWitnessDirectRelationUseV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        capability.validate()?;
        let selector = capability.selector;
        if selector.relation != PersistentDirectRelationV1::Galois
            || selector.context_digest != context.digest()
            || selector.prior_round_digest != context.initial_round_digest()
            || selector.evaluated_key_ordinal != context.evaluated_key_ordinal()
            || selector.digit_index != context.digit_index()
            || selector.galois_exponent != context.galois_exponent()
            || selector.common_a_statement_digest != [0; 32]
            || selector.target_a_statement_digest == [0; 32]
            || selector.aggregate_h0_statement_digest != [0; 32]
            || selector.aggregate_h1_statement_digest != [0; 32]
            || capability.binding_set_root != context.secret_lineage_root()
            || capability.collective_public_key_digest != context.collective_public_key_digest()
            || context.direct_secret_lineage_digest(capability.party_index)?
                != capability.secret_identity_digest
            || capability.ephemeral_identity_digest != [0; 32]
            || capability.ephemeral_commitment_set_digest != [0; 32]
            || capability.ephemeral_source_context_digest != [0; 32]
            || capability.ephemeral_source_statement_digest != [0; 32]
            || capability.ephemeral_record_index != 0
            || capability.ephemeral_commitments.is_some()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(Self {
            context,
            expected_statement_digest: selector.target_a_statement_digest,
            stream: DirectGaloisTargetAStatementStreamV1::begin(context)?,
        })
    }

    /// Derive the next complete limb into caller-owned fixed workspace.
    ///
    /// The stream poisons itself before every fallible operation, including an
    /// unwind, and therefore cannot resume after failure.
    pub(super) fn derive_next_limb_into(
        &mut self,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        self.stream.derive_next_limb_into(output)
    }

    /// Arm one deterministic unwind after the underlying stream is poisoned.
    #[cfg(test)]
    pub(super) fn inject_unwind_on_next_derive_for_test(&mut self) {
        self.stream.inject_unwind_on_next_derive = true;
    }

    /// Consume a complete replay and compare its typed statement internally.
    pub(super) fn finish(self) -> Result<CompletedDirectGaloisTargetAReplayV1, ZkAmsMkheErrorV1> {
        let observed = self.stream.finish()?.statement_digest_for(self.context)?;
        if observed != self.expected_statement_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(CompletedDirectGaloisTargetAReplayV1 {
            _seal: DirectGaloisTargetACompletionSealV1 {
                _non_copy: Vec::new(),
            },
        })
    }
}

/// Derive target `a` only from the exact ordered CPK binding set and a context
/// reconstructed from that same sealed authority.
fn derive_verified_direct_galois_target_a_statement_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
) -> Result<VerifiedDirectGaloisTargetAStatementV1, ZkAmsMkheErrorV1> {
    bindings.validate_for_consumer(roster, PersistentWitnessConsumerV1::Galois)?;
    let target = match context.target() {
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization => {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        target @ ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { .. } => target,
    };
    let expected = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        roster,
        bindings,
        target,
        usize::from(context.digit_index()),
    )?;
    if expected != context {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let mut workspace = Vec::new();
    workspace
        .try_reserve_exact(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    workspace.resize(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1, 0_u64);
    let mut stream = DirectGaloisTargetAStatementStreamV1::begin(context)?;
    for _ in 0..DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1 {
        stream.derive_next_limb_into(&mut workspace)?;
    }
    stream.finish()
}

/// Consume one private target-`a` authority into the only valid Galois
/// selector shape. Neither the authority nor its raw digest crosses this
/// module boundary.
fn new_galois_selector_v1(
    context: ZkAmsMkheDirectCeremonyContextV1,
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
    target_a: VerifiedDirectGaloisTargetAStatementV1,
) -> Result<PersistentDirectRelationUseSelectorV1, ZkAmsMkheErrorV1> {
    let target_a_statement_digest = target_a.statement_digest_for(context)?;
    let selector = PersistentDirectRelationUseSelectorV1 {
        relation: PersistentDirectRelationV1::Galois,
        context_digest: context.digest(),
        prior_round_digest: context.initial_round_digest(),
        evaluated_key_ordinal: context.evaluated_key_ordinal(),
        digit_index: context.digit_index(),
        galois_exponent: context.galois_exponent(),
        common_a_statement_digest: [0; 32],
        target_a_statement_digest,
        aggregate_h0_statement_digest: [0; 32],
        aggregate_h1_statement_digest: [0; 32],
        contribution_statement_digest,
        proof_commitment_transcript_digest,
    };
    selector.validate()?;
    Ok(selector)
}

/// Mint the only production Galois selector from exact CPK authority.
pub(super) fn mint_galois_selector_v1(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    bindings: &VerifiedPersistentWitnessBindingSetV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
    contribution_statement_digest: [u8; 32],
    proof_commitment_transcript_digest: [u8; 32],
) -> Result<PersistentDirectRelationUseSelectorV1, ZkAmsMkheErrorV1> {
    let target_a = derive_verified_direct_galois_target_a_statement_v1(roster, bindings, context)?;
    new_galois_selector_v1(
        context,
        contribution_statement_digest,
        proof_commitment_transcript_digest,
        target_a,
    )
}

fn sampler_frame(
    context_frame: [u8; DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1],
    limb: u16,
    modulus: u64,
) -> Result<Vec<u8>, ZkAmsMkheErrorV1> {
    if release_profile_v1().moduli.get(usize::from(limb)).copied() != Some(modulus) {
        return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
    }
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(DIRECT_GALOIS_TARGET_A_SAMPLER_FRAME_BYTES_V1)
        .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    frame.extend_from_slice(DIRECT_GALOIS_TARGET_A_SAMPLER_DOMAIN_V1);
    frame.extend_from_slice(&[
        MKHE_VERSION_V1,
        DIRECT_GALOIS_TARGET_A_DERIVATION_ALGORITHM_V1,
    ]);
    frame.extend_from_slice(&(DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1 as u32).to_be_bytes());
    frame.extend_from_slice(&context_frame);
    frame.extend_from_slice(&limb.to_be_bytes());
    frame.extend_from_slice(&modulus.to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1)
            .map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?
            .to_be_bytes(),
    );
    if frame.len() != DIRECT_GALOIS_TARGET_A_SAMPLER_FRAME_BYTES_V1 {
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

fn authority_digest(axes: &DirectGaloisTargetAAxesV1, statement_digest: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_GALOIS_TARGET_A_AUTHORITY_DOMAIN_V1);
    hash.update(&[
        MKHE_VERSION_V1,
        DIRECT_GALOIS_TARGET_A_DERIVATION_ALGORITHM_V1,
    ]);
    hash.update(&(DIRECT_GALOIS_TARGET_A_CONTEXT_BYTES_V1 as u32).to_be_bytes());
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
#[path = "direct_galois_target_a_v1_tests.rs"]
mod tests;
