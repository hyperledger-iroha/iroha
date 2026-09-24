//! Staged fixed-frame terminal-body SHA relation for both Pasta parities.
//!
//! This hashes assigned cells only. The caller must supply the exact recursively verified
//! candidate and State/Guard cells, plus separately proved terminal openings. In particular,
//! arbitrary witness bytes for journal or recovery are not monetary authority.

use halo2_base::{
    AssignedValue, Context,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WIRE_VERSION_V1, KagemushaHardwareTerminalBodyCommitmentLayoutV1,
};

use super::{
    KagemushaPoseidonFieldV1,
    guard_bundle::{constant_bytes, digest_limbs_assigned, hash},
};
use crate::zk::pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1};

pub(super) const TERMINAL_BODY_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:hardware-terminal-body";

/// Assigned body fields. The State and Guard pairs are equality-constrained here.
///
/// `transition_nullifier`, `outbox_reservation_commitment`, `evidence_*`,
/// `private_journal_commitment`, and `private_recovery_commitment` still require
/// independently authenticated source cells before this hash can authorize money.
pub(super) struct KagemushaAssignedTerminalBodyFieldsV1<F: KagemushaPoseidonFieldV1> {
    /// Candidate digest from the authenticated 93-cell recursive State semantic prefix.
    pub candidate_envelope_digest: [PastaSha256ByteV1<F>; 32],
    /// Released lifecycle digest in the State relation.
    pub state_lifecycle_binding_digest: [AssignedValue<F>; 2],
    /// The same lifecycle digest in the verified Guard relation.
    pub guard_lifecycle_binding_digest: [AssignedValue<F>; 2],
    /// Outgoing transition nullifier from a separately proved terminal relation.
    pub transition_nullifier: [AssignedValue<F>; 2],
    /// Consumed outbox reservation commitment from a separately proved terminal relation.
    pub outbox_reservation_commitment: [AssignedValue<F>; 2],
    /// Selected evidence variant, constrained to zero or one.
    pub evidence_tag: AssignedValue<F>,
    /// Commitment opened by the selected evidence proof.
    pub evidence_commitment: [AssignedValue<F>; 2],
    /// Hardware profile ID in the State relation.
    pub state_hardware_profile_id: [AssignedValue<F>; 2],
    /// The same profile ID in the verified Guard relation.
    pub guard_hardware_profile_id: [AssignedValue<F>; 2],
    /// Policy epoch in the State relation.
    pub state_policy_epoch: AssignedValue<F>,
    /// The same epoch in the verified Guard relation.
    pub guard_policy_epoch: AssignedValue<F>,
    /// Compact successor state commitment in the State relation.
    pub state_successor_commitment: [AssignedValue<F>; 2],
    /// The same compact successor commitment in the verified Guard relation.
    pub guard_successor_commitment: [AssignedValue<F>; 2],
    /// Digest of the durable terminal journal preimage, still requiring proof.
    pub private_journal_commitment: [AssignedValue<F>; 2],
    /// Digest of the sealed recovery preimage, still requiring proof.
    pub private_recovery_commitment: [AssignedValue<F>; 2],
}

/// Sources that the terminal circuit already has after verifying its candidate and Guard proofs.
///
/// A caller must pass the actual derived SHA outputs for the nullifier, reservation and evidence;
/// reassigning their native values as fresh witnesses would not establish this relation.
pub(super) struct KagemushaAuthenticatedTerminalBodyPrefixV1<F: KagemushaPoseidonFieldV1> {
    pub candidate_envelope_digest: [AssignedValue<F>; 2],
    pub state_lifecycle_binding_digest: [AssignedValue<F>; 2],
    pub guard_lifecycle_binding_digest: [AssignedValue<F>; 2],
    pub derived_transition_nullifier: [AssignedValue<F>; 2],
    pub derived_outbox_reservation_commitment: [AssignedValue<F>; 2],
    pub derived_evidence_tag: AssignedValue<F>,
    pub derived_evidence_commitment: [AssignedValue<F>; 2],
    pub state_hardware_profile_id: [AssignedValue<F>; 2],
    pub guard_hardware_profile_id: [AssignedValue<F>; 2],
    pub state_policy_epoch: AssignedValue<F>,
    pub guard_policy_epoch: AssignedValue<F>,
    pub state_successor_commitment: [AssignedValue<F>; 2],
    pub guard_successor_commitment: [AssignedValue<F>; 2],
}

/// SHA outputs of the canonical journal and recovery preimages.
///
/// Construct this only after both durable preimages have been constrained from the same verified
/// prepared intent. The current terminal witness does not yet carry that authenticated opening;
/// assigning matching host digests here would not authorize money.
pub(super) struct KagemushaAuthenticatedTerminalBodyDurableSourcesV1<F: KagemushaPoseidonFieldV1> {
    pub(super) private_journal_commitment: [AssignedValue<F>; 2],
    pub(super) private_recovery_commitment: [AssignedValue<F>; 2],
}

/// Bind a terminal body to already-authenticated source cells and its signed certificate digest.
///
/// Missing canonical journal or recovery SHA outputs are an error. This function must only be
/// called after the candidate and Guard proofs, terminal SHA jobs, and durable-artifact preimages
/// have been constrained in the same parity; it does not authenticate source cells by itself.
pub(super) fn constrain_terminal_body_commitment_from_sources_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    body: &KagemushaAssignedTerminalBodyFieldsV1<F>,
    prefix: &KagemushaAuthenticatedTerminalBodyPrefixV1<F>,
    durable: Option<&KagemushaAuthenticatedTerminalBodyDurableSourcesV1<F>>,
    certificate_commitment: [AssignedValue<F>; 2],
) -> Result<(), String> {
    let durable = durable.ok_or_else(|| {
        "terminal body lacks canonical journal and recovery SHA sources".to_owned()
    })?;
    let candidate = digest_limbs_assigned(ctx, &body.candidate_envelope_digest);
    for (body_digest, source_digest) in [
        (candidate, prefix.candidate_envelope_digest),
        (
            body.state_lifecycle_binding_digest,
            prefix.state_lifecycle_binding_digest,
        ),
        (
            body.guard_lifecycle_binding_digest,
            prefix.guard_lifecycle_binding_digest,
        ),
        (
            body.transition_nullifier,
            prefix.derived_transition_nullifier,
        ),
        (
            body.outbox_reservation_commitment,
            prefix.derived_outbox_reservation_commitment,
        ),
        (body.evidence_commitment, prefix.derived_evidence_commitment),
        (
            body.state_hardware_profile_id,
            prefix.state_hardware_profile_id,
        ),
        (
            body.guard_hardware_profile_id,
            prefix.guard_hardware_profile_id,
        ),
        (
            body.state_successor_commitment,
            prefix.state_successor_commitment,
        ),
        (
            body.guard_successor_commitment,
            prefix.guard_successor_commitment,
        ),
        (
            body.private_journal_commitment,
            durable.private_journal_commitment,
        ),
        (
            body.private_recovery_commitment,
            durable.private_recovery_commitment,
        ),
    ] {
        for (body_cell, source_cell) in body_digest.into_iter().zip(source_digest) {
            ctx.constrain_equal(&body_cell, &source_cell);
        }
    }
    ctx.constrain_equal(&body.evidence_tag, &prefix.derived_evidence_tag);
    ctx.constrain_equal(&body.state_policy_epoch, &prefix.state_policy_epoch);
    ctx.constrain_equal(&body.guard_policy_epoch, &prefix.guard_policy_epoch);
    let actual = hash_terminal_body_commitment_from_assigned_v1(ctx, range, jobs, body)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &actual)
        .into_iter()
        .zip(certificate_commitment)
    {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(())
}

fn append_digest_limbs_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    message: &mut Vec<PastaSha256ByteV1<F>>,
    digest: [AssignedValue<F>; 2],
) {
    for limb in digest {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), limb, 128);
        message.extend(
            bits.chunks_exact(8)
                .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), bits)),
        );
    }
}

/// Queue SHA-256 of the exact 299-byte flat terminal body with canonical domain/length framing.
///
/// The digest output is not sufficient for monetary admission until the unresolved fields
/// above are tied to their verified terminal and durable-artifact source relations in both
/// parities, and the returned bytes are equality-bound to the signed Core subject.
pub(super) fn hash_terminal_body_commitment_from_assigned_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    fields: &KagemushaAssignedTerminalBodyFieldsV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    use KagemushaHardwareTerminalBodyCommitmentLayoutV1 as Layout;

    for (state, guard) in [
        (
            fields.state_lifecycle_binding_digest,
            fields.guard_lifecycle_binding_digest,
        ),
        (
            fields.state_hardware_profile_id,
            fields.guard_hardware_profile_id,
        ),
        (
            fields.state_successor_commitment,
            fields.guard_successor_commitment,
        ),
    ] {
        for (state, guard) in state.into_iter().zip(guard) {
            ctx.constrain_equal(&state, &guard);
        }
    }
    ctx.constrain_equal(&fields.state_policy_epoch, &fields.guard_policy_epoch);
    range.gate().assert_bit(ctx, fields.evidence_tag);

    let mut message = constant_bytes(TERMINAL_BODY_DOMAIN_V1);
    message.extend(constant_bytes(&[0]));
    message.extend(constant_bytes(&(Layout::BODY_BYTES as u64).to_le_bytes()));
    message.extend(constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()));
    message.extend_from_slice(&fields.candidate_envelope_digest);
    append_digest_limbs_v1(
        ctx,
        range,
        &mut message,
        fields.state_lifecycle_binding_digest,
    );
    append_digest_limbs_v1(ctx, range, &mut message, fields.transition_nullifier);
    append_digest_limbs_v1(
        ctx,
        range,
        &mut message,
        fields.outbox_reservation_commitment,
    );
    message.push(PastaSha256ByteV1::range_checked(
        ctx,
        range,
        fields.evidence_tag,
    ));
    append_digest_limbs_v1(ctx, range, &mut message, fields.evidence_commitment);
    append_digest_limbs_v1(ctx, range, &mut message, fields.state_hardware_profile_id);
    let epoch_bits = PastaSha256BitV1::decompose(ctx, range.gate(), fields.state_policy_epoch, 64);
    message.extend(
        epoch_bits
            .chunks_exact(8)
            .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), bits)),
    );
    append_digest_limbs_v1(ctx, range, &mut message, fields.state_successor_commitment);
    append_digest_limbs_v1(ctx, range, &mut message, fields.private_journal_commitment);
    append_digest_limbs_v1(ctx, range, &mut message, fields.private_recovery_commitment);
    if message.len() != TERMINAL_BODY_DOMAIN_V1.len() + 1 + 8 + Layout::BODY_BYTES {
        return Err("terminal body commitment frame width changed".to_owned());
    }
    hash(ctx, jobs, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::{
        kagemusha_v1_poseidon::digest_limbs, kagemusha_v1_recursion::guard_bundle::assign_bytes,
        pasta_sha256::PastaSha256ConfigV1,
    };
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_data_model::kagemusha::{
        KagemushaCommitEvidenceV1, KagemushaHardwareTerminalBodyV1, KagemushaTrustedCommitTimeV1,
    };

    const K: u32 = 16;
    const UNUSABLE_ROWS: usize = 9;

    #[derive(Clone, Debug)]
    struct Config<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct TestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for TestCircuit<F> {
        type Config = Config<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable = (1_usize << params.k) - UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable);
            Config {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("flat terminal body test uses parameterized Base config")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "terminal body Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << self.builder.config_params.k) - UNUSABLE_ROWS,
            )
        }
    }

    fn body() -> KagemushaHardwareTerminalBodyV1 {
        KagemushaHardwareTerminalBodyV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            candidate_envelope_digest: [1; 32],
            lifecycle_binding_digest: [2; 32],
            transition_nullifier: [3; 32],
            outbox_reservation_commitment: [4; 32],
            commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
                time_evidence_commitment: [5; 32],
            }),
            hardware_profile_id: [6; 32],
            policy_epoch: 7,
            private_successor_commitment: [8; 32],
            private_journal_commitment: [9; 32],
            private_recovery_commitment: [10; 32],
        }
    }

    #[derive(Clone, Copy)]
    enum SourceMutation {
        None,
        MissingDurable,
        Candidate,
        GuardLifecycle,
        Nullifier,
        Reservation,
        EvidenceTag,
        Evidence,
        Profile,
        PolicyEpoch,
        Successor,
        Journal,
        Recovery,
    }

    fn circuit<F: KagemushaPoseidonFieldV1>(
        body: KagemushaHardwareTerminalBodyV1,
        expected: [u8; 32],
        mutation: SourceMutation,
    ) -> Result<TestCircuit<F>, String> {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(15);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let assign_digest = |ctx: &mut Context<F>, digest: [u8; 32]| {
            digest_limbs::<F>(digest).map(|value| ctx.load_witness(value))
        };
        let lifecycle = assign_digest(ctx, body.lifecycle_binding_digest);
        let guard_lifecycle = assign_digest(ctx, body.lifecycle_binding_digest);
        let profile = assign_digest(ctx, body.hardware_profile_id);
        let epoch = ctx.load_witness(F::from(body.policy_epoch));
        let successor = assign_digest(ctx, body.private_successor_commitment);
        let evidence = match body.commit_evidence {
            KagemushaCommitEvidenceV1::TrustedTime(value) => {
                (0_u64, value.time_evidence_commitment)
            }
            KagemushaCommitEvidenceV1::MonotonicLease(value) => {
                (1_u64, value.lease_evidence_commitment)
            }
        };
        let fields = KagemushaAssignedTerminalBodyFieldsV1 {
            candidate_envelope_digest: assign_bytes(ctx, &range, &body.candidate_envelope_digest)
                .try_into()
                .expect("candidate width"),
            state_lifecycle_binding_digest: lifecycle,
            guard_lifecycle_binding_digest: guard_lifecycle,
            transition_nullifier: assign_digest(ctx, body.transition_nullifier),
            outbox_reservation_commitment: assign_digest(ctx, body.outbox_reservation_commitment),
            evidence_tag: ctx.load_witness(F::from(evidence.0)),
            evidence_commitment: assign_digest(ctx, evidence.1),
            state_hardware_profile_id: profile,
            guard_hardware_profile_id: assign_digest(ctx, body.hardware_profile_id),
            state_policy_epoch: epoch,
            guard_policy_epoch: ctx.load_witness(F::from(body.policy_epoch)),
            state_successor_commitment: successor,
            guard_successor_commitment: assign_digest(ctx, body.private_successor_commitment),
            private_journal_commitment: assign_digest(ctx, body.private_journal_commitment),
            private_recovery_commitment: assign_digest(ctx, body.private_recovery_commitment),
        };
        let mutate = |ctx: &mut Context<F>, digest: [u8; 32], selected: bool| {
            let mut assigned = assign_digest(ctx, digest);
            if selected {
                assigned[0] = ctx.load_witness(*assigned[0].value() + F::from(1_u64));
            }
            assigned
        };
        let sources = KagemushaAuthenticatedTerminalBodyPrefixV1 {
            candidate_envelope_digest: mutate(
                ctx,
                body.candidate_envelope_digest,
                matches!(mutation, SourceMutation::Candidate),
            ),
            state_lifecycle_binding_digest: lifecycle,
            guard_lifecycle_binding_digest: mutate(
                ctx,
                body.lifecycle_binding_digest,
                matches!(mutation, SourceMutation::GuardLifecycle),
            ),
            derived_transition_nullifier: mutate(
                ctx,
                body.transition_nullifier,
                matches!(mutation, SourceMutation::Nullifier),
            ),
            derived_outbox_reservation_commitment: mutate(
                ctx,
                body.outbox_reservation_commitment,
                matches!(mutation, SourceMutation::Reservation),
            ),
            derived_evidence_tag: ctx.load_witness(
                F::from(evidence.0)
                    + F::from(u64::from(matches!(mutation, SourceMutation::EvidenceTag))),
            ),
            derived_evidence_commitment: mutate(
                ctx,
                evidence.1,
                matches!(mutation, SourceMutation::Evidence),
            ),
            state_hardware_profile_id: profile,
            guard_hardware_profile_id: mutate(
                ctx,
                body.hardware_profile_id,
                matches!(mutation, SourceMutation::Profile),
            ),
            state_policy_epoch: epoch,
            guard_policy_epoch: ctx.load_witness(
                F::from(body.policy_epoch)
                    + F::from(u64::from(matches!(mutation, SourceMutation::PolicyEpoch))),
            ),
            state_successor_commitment: successor,
            guard_successor_commitment: mutate(
                ctx,
                body.private_successor_commitment,
                matches!(mutation, SourceMutation::Successor),
            ),
        };
        let durable = KagemushaAuthenticatedTerminalBodyDurableSourcesV1 {
            private_journal_commitment: mutate(
                ctx,
                body.private_journal_commitment,
                matches!(mutation, SourceMutation::Journal),
            ),
            private_recovery_commitment: mutate(
                ctx,
                body.private_recovery_commitment,
                matches!(mutation, SourceMutation::Recovery),
            ),
        };
        let mut jobs = PastaSha256JobsV1::default();
        let certificate_commitment = assign_digest(ctx, expected);
        constrain_terminal_body_commitment_from_sources_v1(
            ctx,
            &range,
            &mut jobs,
            &fields,
            &sources,
            (!matches!(mutation, SourceMutation::MissingDurable)).then_some(&durable),
            certificate_commitment,
        )?;
        assert_eq!(jobs.compression_blocks().expect("fixed SHA geometry"), 6);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        Ok(TestCircuit { builder, jobs })
    }

    #[test]
    fn flat_terminal_body_sha_matches_native_and_rejects_unbound_fields_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            let original = body();
            let expected = original.canonical_commitment().expect("valid body");
            MockProver::run(
                K,
                &circuit::<F>(original, expected, SourceMutation::None).expect("complete sources"),
                vec![],
            )
            .expect("flat terminal body circuit")
            .assert_satisfied();
            let mut mutated = original;
            mutated.private_journal_commitment[0] ^= 1;
            assert!(
                MockProver::run(
                    K,
                    &circuit::<F>(mutated, expected, SourceMutation::None)
                        .expect("mutated body has complete synthetic sources"),
                    vec![],
                )
                .expect("mutated journal circuit")
                .verify()
                .is_err()
            );
            assert!(circuit::<F>(original, expected, SourceMutation::MissingDurable).is_err());
            for mutation in [
                SourceMutation::Candidate,
                SourceMutation::GuardLifecycle,
                SourceMutation::Nullifier,
                SourceMutation::Reservation,
                SourceMutation::EvidenceTag,
                SourceMutation::Evidence,
                SourceMutation::Profile,
                SourceMutation::PolicyEpoch,
                SourceMutation::Successor,
                SourceMutation::Journal,
                SourceMutation::Recovery,
            ] {
                assert!(
                    MockProver::run(
                        K,
                        &circuit::<F>(original, expected, mutation)
                            .expect("source mismatch has complete synthetic sources"),
                        vec![],
                    )
                    .expect("source mismatch circuit")
                    .verify()
                    .is_err()
                );
            }
        }
        check::<Fp>();
        check::<Fq>();
    }
}
