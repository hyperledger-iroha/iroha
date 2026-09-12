//! Genuine Bootstrap -> MintFold -> SendSplit proofs through Core's diagnostic state machine.
//!
//! Test-provider signatures establish explicit fixture inputs only. This module neither admits a
//! production release nor qualifies physical hardware. Its separate terminal diagnostic also
//! proves internal TerminalAuthorization, and its separate wrapper diagnostic measures the
//! genuine CommitWrapper protocol seed. All entry points stop before payment finalization,
//! transport/decryption, receiver staging, and ReceiveFold; the 1024-handoff gate remains closed.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use super::*;
use crate::zk::{
    kagemusha_v1_recursion::{
        KagemushaMintFinalityHelperVerificationRequestV1, KagemushaParityVerificationRequestV1,
        KagemushaStateProofVerificationRequestV1, VerifiedKagemushaMintFinalityHelperV1,
        mint_authorization::mint_authorization_public_instances_v1,
        terminal_authorization::{
            KagemushaCommitEvidenceOpeningV1, canonical_commit_evidence_commitment_v1,
            canonical_predecessor_conflict_nullifier_v1,
            canonical_prepared_one_use_authorization_digest_v1,
        },
        transport_decider::{
            KagemushaTransportDeciderParityWitnessV1, KagemushaTransportDeciderWitnessV1,
            build_kagemusha_transport_decider_pair_v1,
        },
        verify_kagemusha_mint_finality_helper_v1,
    },
    kagemusha_v1_state::{
        BootstrapAuthorizationV1, BootstrapStatementV1, CreditStageStatementV1,
        DurabilityAnchorStatementV1, HardwareTransitionCertificateV1,
        HardwareTransitionStatementV1, KagemushaDurableCapacityV1, KagemushaGuardBundleVerifierV1,
        KagemushaMemoryAuthenticatedHistoryStoreV1, KagemushaRecoveryCheckpointStatementV1,
        KagemushaRecoveryEnrollmentBindingV1, KagemushaRecoveryJournalsV1, KagemushaStateErrorV1,
        KagemushaStateMachineV1, KagemushaStateProofReleaseV1, MintInboxReservationV1,
        MintReservationCertificateV1, MintReservationStatementV1, MintStageCertificateV1,
        MintStageStatementV1, PreparedOutgoingRecoveryViewV1, SendSplitPreparationV1,
        TransitionAuthorizationV1, TransitionProofStatementV1, VerifiedMintStageV1,
        canonical_empty_durable_effect_digest_v1, mint_envelope_digest_v1,
    },
};
use halo2_proofs::halo2curves::ff::Field as _;
use iroha_data_model::kagemusha::{
    KagemushaCommitEvidenceV1, KagemushaMintCreditV1, KagemushaOutboxReservationV1,
    KagemushaRetailEnrollmentOwnerV1, KagemushaRetailEnrollmentRuntimeV1,
    KagemushaTrustedCommitTimeV1,
};
use norito::SerializePayload;

#[path = "state_milestone/recovery_checkpoint.rs"]
mod recovery_checkpoint;
#[path = "state_milestone/terminal.rs"]
mod terminal;
#[path = "state_milestone/wrapper.rs"]
mod wrapper;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiagnosticMilestoneV1 {
    State,
    Terminal,
    Wrapper,
}

const RESERVATION_DOMAIN: &[u8] = b"iroha:kagemusha:diagnostic:mint-reservation\0";
const STAGE_DOMAIN: &[u8] = b"iroha:kagemusha:diagnostic:mint-stage\0";
const BOOTSTRAP_TIME: u64 = 50;
const STAGE_TIME: u64 = 150;
const MINT_TIME: u64 = 200;
const SEND_TIME: u64 = 300;

fn ensure(condition: bool, label: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| label.to_owned())
}

fn history_values<F: KagemushaPoseidonFieldV1>(history: &[u8]) -> Vec<F> {
    assert_eq!(history.len(), KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1);
    history
        .chunks_exact(16)
        .map(|chunk| from_u128::<F>(u128::from_le_bytes(chunk.try_into().expect("history limb"))))
        .collect()
}

fn decide_eq(
    params: &ParamsIPA<EqAffine>,
    protocol: &PlonkProtocol<EqAffine>,
    proof: &[u8],
    instances: &[Fp],
    history: &KagemushaEqAccumulatorV1,
) -> Result<KagemushaEqAccumulatorV1, String> {
    let current = KagemushaEqAccumulatorV1::from_native(&verify_eq_succinct_protocol(
        params, protocol, proof, instances,
    )?)
    .map_err(|error| error.to_string())?;
    decide_kagemusha_eq_accumulator_v1(params, &current).map_err(|error| error.to_string())?;
    decide_kagemusha_eq_accumulator_v1(params, history).map_err(|error| error.to_string())?;
    Ok(current)
}

fn decide_ep(
    params: &ParamsIPA<EpAffine>,
    protocol: &PlonkProtocol<EpAffine>,
    proof: &[u8],
    instances: &[Fq],
    history: &KagemushaEpAccumulatorV1,
) -> Result<KagemushaEpAccumulatorV1, String> {
    let current = KagemushaEpAccumulatorV1::from_native(&verify_ep_succinct_protocol(
        params, protocol, proof, instances,
    )?)
    .map_err(|error| error.to_string())?;
    decide_kagemusha_ep_accumulator_v1(params, &current).map_err(|error| error.to_string())?;
    decide_kagemusha_ep_accumulator_v1(params, history).map_err(|error| error.to_string())?;
    Ok(current)
}

/// Private proof-authenticated staging evidence for this concrete diagnostic verifier.
///
/// No decoder, default value, public constructor, or generic accepting callback exists. Both
/// current proof equations and exact transported histories have been decided before construction.
pub(crate) struct DiagnosticMintStageProofV1 {
    reservation_digest: DigestV1,
    authorization_digest: DigestV1,
    envelope_digest: DigestV1,
    artifacts: KagemushaRecursionArtifactsV1,
    finality: VerifiedKagemushaMintFinalityHelperV1,
}

impl DiagnosticMintStageProofV1 {
    /// Consume evidence only for the exact reservation and finalized envelope it authenticated.
    pub(crate) fn into_finality(
        self,
        reservation: &MintInboxReservationV1,
        credit: &KagemushaMintCreditV1,
    ) -> Result<VerifiedKagemushaMintFinalityHelperV1, KagemushaStateErrorV1> {
        let authorization = reservation.authorization();
        if reservation.digest()? != self.reservation_digest
            || authorization
                .canonical_digest()
                .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?
                != self.authorization_digest
            || mint_envelope_digest_v1(credit)? != self.envelope_digest
            || authorization.statement.context.release_id != self.artifacts.release_id
            || authorization.statement.context.artifact_manifest_digest
                != self.artifacts.artifact_manifest_digest
            || authorization.proof.eq_protocol_digest
                != self.artifacts.mint_authorization_eq_protocol_digest
            || authorization.proof.ep_protocol_digest
                != self.artifacts.mint_authorization_ep_protocol_digest
            || credit.proof.eq_protocol_digest != self.artifacts.mint_finality_eq_protocol_digest
            || credit.proof.ep_protocol_digest != self.artifacts.mint_finality_ep_protocol_digest
        {
            return Err(KagemushaStateErrorV1::MintFinalityMismatch);
        }
        Ok(self.finality)
    }
}

/// Concrete test verifier pinned to generated protocols and retained private State proofs.
#[derive(Clone)]
struct DiagnosticVerifier<'a> {
    funded: &'a RealFundedPrerequisite,
    keys: &'a StateKeys,
    artifacts: KagemushaRecursionArtifactsV1,
    states: Rc<RefCell<BTreeMap<DigestV1, Rc<KagemushaGeneratedRecursiveStateProofV1>>>>,
}

impl DiagnosticVerifier<'_> {
    fn verify_authorization(
        &self,
        authorization: &KagemushaMintAuthorizationV1,
    ) -> Result<(), String> {
        authorization
            .validate_shape()
            .map_err(|error| error.to_string())?;
        ensure(
            authorization == &self.funded.authorization.authorization,
            "diagnostic authorization does not match retained recipient material",
        )?;
        let proof = &authorization.proof;
        ensure(
            proof.eq_protocol_digest == self.artifacts.mint_authorization_eq_protocol_digest
                && proof.ep_protocol_digest == self.artifacts.mint_authorization_ep_protocol_digest
                && authorization.statement.context.artifact_manifest_digest
                    == self.artifacts.artifact_manifest_digest
                && authorization.statement.context.release_id == self.artifacts.release_id,
            "diagnostic authorization protocol/release mismatch",
        )?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .map_err(|error| error.to_string())?;
        let eq_instances = mint_authorization_public_instances_v1::<Fp>(
            &authorization.statement,
            proof.guard_ep_credential_audit,
            proof.eq_deferred_audit,
            proof.ep_deferred_audit,
            eq_history.as_bytes(),
        )?;
        let ep_instances = mint_authorization_public_instances_v1::<Fq>(
            &authorization.statement,
            proof.guard_ep_credential_audit,
            proof.eq_deferred_audit,
            proof.ep_deferred_audit,
            ep_history.as_bytes(),
        )?;
        let retained = &self.funded.authorization.generated;
        ensure(
            eq_instances == retained.eq_public_instances
                && ep_instances == retained.ep_public_instances,
            "diagnostic authorization exact public-column mismatch",
        )?;
        let eq_current = decide_eq(
            &self.funded.eq,
            &self.funded.authorization_protocols.eq_protocol,
            &proof.eq_proof,
            &eq_instances,
            &eq_history,
        )?;
        let ep_current = decide_ep(
            &self.funded.ep,
            &self.funded.authorization_protocols.ep_protocol,
            &proof.ep_proof,
            &ep_instances,
            &ep_history,
        )?;
        ensure(
            eq_current == retained.eq_current_accumulator
                && ep_current == retained.ep_current_accumulator,
            "diagnostic authorization extracted-current mismatch",
        )
    }

    fn verify_stage(
        &self,
        reservation: &MintInboxReservationV1,
        credit: &KagemushaMintCreditV1,
    ) -> Result<DiagnosticMintStageProofV1, String> {
        reservation
            .validate_inputs()
            .map_err(|error| error.to_string())?;
        credit
            .validate_shape_against_authorization(reservation.authorization())
            .map_err(|error| error.to_string())?;
        self.verify_authorization(reservation.authorization())?;
        let finality = verify_kagemusha_mint_finality_helper_v1(self, self.artifacts, credit)
            .map_err(|error| error.to_string())?;
        Ok(DiagnosticMintStageProofV1 {
            reservation_digest: reservation.digest().map_err(|error| error.to_string())?,
            authorization_digest: reservation
                .authorization()
                .canonical_digest()
                .map_err(|error| error.to_string())?,
            envelope_digest: mint_envelope_digest_v1(credit).map_err(|error| error.to_string())?,
            artifacts: self.artifacts,
            finality,
        })
    }

    fn retain_state(&self, proof: Rc<KagemushaGeneratedRecursiveStateProofV1>) {
        terminally_verify_state_proof(self.keys, &proof);
        assert!(
            self.states
                .borrow_mut()
                .insert(proof.proof.semantic_digest, proof)
                .is_none()
        );
    }
}

fn mint_instances<F: KagemushaPoseidonFieldV1>(
    request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    history: &[u8],
) -> Vec<F> {
    let mut values = vec![F::from(KagemushaMintAuthorityStepV1::FinalizedMint as u64)];
    values.extend(digest_limbs::<F>(request.semantic_digest));
    values.push(from_u128::<F>(request.statement.amount));
    for digest in [
        request.finality_certificate_binding,
        request.finality_authority_head,
        request.statement.lifecycle.release_id,
        request.finality_genesis_roster_id,
        request.eq_protocol_digest,
        request.ep_protocol_digest,
        request.proof.eq_deferred_audit,
        request.proof.ep_deferred_audit,
        request.finality_proof_binding_digest,
    ] {
        values.extend(digest_limbs::<F>(digest));
    }
    assert_eq!(values.len(), mint_instance::HISTORY_START);
    values.extend(history_values::<F>(history));
    assert_eq!(
        values.len(),
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1
    );
    values
}

impl KagemushaRecursiveVerifierV1 for DiagnosticVerifier<'_> {
    fn verify_state_proof_and_decide(
        &self,
        request: &KagemushaStateProofVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        let proof = request.proof;
        let retained = self
            .states
            .borrow()
            .get(&proof.semantic_digest)
            .cloned()
            .ok_or_else(|| "no genuine diagnostic State proof retained".to_owned())?;
        ensure(
            proof == &retained.proof
                && proof.eq_protocol_digest == self.keys.eq_protocol_digest
                && proof.ep_protocol_digest == self.keys.ep_protocol_digest,
            "diagnostic State proof/protocol substitution",
        )?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&proof.ep_history)
            .map_err(|error| error.to_string())?;
        let mut eq_instances = request.public_inputs.public_instances::<Fp>()?;
        let mut ep_instances = request.public_inputs.public_instances::<Fq>()?;
        eq_instances.extend(history_values::<Fp>(eq_history.as_bytes()));
        ep_instances.extend(history_values::<Fq>(ep_history.as_bytes()));
        ensure(
            eq_instances == retained.eq_transport_public_instances
                && ep_instances == retained.ep_transport_public_instances,
            "Core State projection differs from genuine transport proof",
        )?;
        // Reverify both retained private carriers and their exact pre-transport histories. The
        // independently verified transport equations below bind those inner proofs recursively.
        let eq_inner = decide_eq(
            &self.funded.eq,
            &self.keys.eq_protocol,
            &retained.eq_inner_proof,
            &retained.eq_public_instances,
            &retained.eq_history,
        )?;
        let ep_inner = decide_ep(
            &self.funded.ep,
            &self.keys.ep_protocol,
            &retained.ep_inner_proof,
            &retained.ep_public_instances,
            &retained.ep_history,
        )?;
        ensure(
            eq_inner == retained.eq_current_accumulator
                && ep_inner == retained.ep_current_accumulator,
            "diagnostic State inner-current substitution",
        )?;
        decide_eq(
            &self.funded.eq,
            &self.keys.eq_transport_protocol,
            &proof.eq_proof,
            &eq_instances,
            &eq_history,
        )?;
        decide_ep(
            &self.funded.ep,
            &self.keys.ep_transport_protocol,
            &proof.ep_proof,
            &ep_instances,
            &ep_history,
        )?;
        Ok(())
    }

    fn verify_mint_finality_helper(
        &self,
        request: &KagemushaMintFinalityHelperVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        let credit = &self.funded.mint_credit;
        let retained = &self.funded.funded.proof;
        ensure(
            request.statement == &credit.statement
                && request.proof == &credit.proof
                && request.semantic_digest
                    == credit
                        .statement
                        .canonical_digest()
                        .map_err(|error| error.to_string())?
                && request.eq_protocol_digest == self.artifacts.mint_finality_eq_protocol_digest
                && request.ep_protocol_digest == self.artifacts.mint_finality_ep_protocol_digest
                && request.finality_certificate_binding == credit.finality_certificate_binding
                && request.finality_authority_head == credit.finality_authority_head
                && request.finality_genesis_roster_id == self.funded.genesis_roster_id
                && request.finality_proof_binding_digest == credit.finality_proof_binding_digest
                && request.artifact_manifest_digest == self.artifacts.artifact_manifest_digest,
            "diagnostic finality statement/protocol/certificate substitution",
        )?;
        let eq_history = KagemushaEqAccumulatorV1::try_from_bytes(&request.proof.eq_history)
            .map_err(|error| error.to_string())?;
        let ep_history = KagemushaEpAccumulatorV1::try_from_bytes(&request.proof.ep_history)
            .map_err(|error| error.to_string())?;
        let eq_instances = mint_instances::<Fp>(request, eq_history.as_bytes());
        let ep_instances = mint_instances::<Fq>(request, ep_history.as_bytes());
        ensure(
            eq_instances == retained.eq_public_instances
                && ep_instances == retained.ep_public_instances,
            "diagnostic finality public-column substitution",
        )?;
        let eq_current = decide_eq(
            &self.funded.eq,
            &self.funded.mint_protocols.eq_protocol,
            &request.proof.eq_proof,
            &eq_instances,
            &eq_history,
        )?;
        let ep_current = decide_ep(
            &self.funded.ep,
            &self.funded.mint_protocols.ep_protocol,
            &request.proof.ep_proof,
            &ep_instances,
            &ep_history,
        )?;
        ensure(
            eq_current == retained.eq_current_accumulator
                && ep_current == retained.ep_current_accumulator,
            "diagnostic finality current-accumulator substitution",
        )
    }

    fn verify_payment_and_decide(
        &self,
        _: &KagemushaPaymentRequestV1,
        _: &KagemushaPaymentV1,
    ) -> Result<(), String> {
        Err("diagnostic milestone stops before Payment".to_owned())
    }

    fn verify_terminal_authorization_and_decide(
        &self,
        _: &KagemushaParityVerificationRequestV1<'_>,
    ) -> Result<(), String> {
        Err("diagnostic milestone has no terminal authority".to_owned())
    }
}

// The diagnostic domains select fixed-v1 payloads; production journal frame types are separate.
fn diagnostic_payload<T: SerializePayload>(value: &T) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    norito::codec::encode_adaptive_into(value, &mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn journal_message<T: SerializePayload>(domain: &[u8], statement: &T) -> Result<Vec<u8>, String> {
    let mut message = domain.to_vec();
    message.extend(diagnostic_payload(statement)?);
    Ok(message)
}

fn sign_journal<T: SerializePayload>(key: &SigningKey, domain: &[u8], statement: &T) -> Vec<u8> {
    device_signature(
        key,
        &journal_message(domain, statement).expect("diagnostic journal message"),
    )
    .as_raw_bytes()
    .to_vec()
}

fn verify_journal<T: SerializePayload>(
    key: &KagemushaDevicePublicKeyV1,
    domain: &[u8],
    statement: &T,
    bytes: &[u8],
) -> Result<(), String> {
    KagemushaDeviceSignatureV1::from_raw_bytes(bytes)
        .map_err(|error| error.to_string())?
        .verify(key, &journal_message(domain, statement)?)
        .map_err(|error| error.to_string())
}

fn guard_frame(proof: &GuardProof) -> Vec<u8> {
    // This is private diagnostic fixture framing, not a production hardware receipt.
    norito::encode_canonical(&(
        proof.relation.statement_digest(),
        proof.eq_credential_audit,
        proof.ep_credential_audit,
        proof.relation.credential_digests(),
        proof.eq_proof.clone(),
        proof.ep_proof.clone(),
        proof.eq_history.as_bytes().to_vec(),
        proof.ep_history.as_bytes().to_vec(),
    ))
    .expect("canonical diagnostic Guard proof frame")
}

fn guard_relation(
    material: &MintRecipientMaterial,
    normalized: KagemushaNormalizedGuardStatementV1,
) -> KagemushaGuardBundleRelationWitnessV1 {
    let credential = material.platform_credential.statement;
    let relation = KagemushaGuardBundleRelationWitnessV1 {
        statement: normalized,
        canonical_empty_effect_digest: credential.canonical_empty_effect_digest,
        predecessor_credential: credential,
        successor_credential: credential,
        predecessor_device_authority_secret: material
            .authorization_relation
            .device_authority_secret,
        successor_device_authority_secret: material.authorization_relation.device_authority_secret,
    };
    relation
        .validate()
        .expect("exact Core diagnostic Guard relation");
    relation
}

fn context_from_normalized(
    normalized: &KagemushaNormalizedGuardStatementV1,
    empty: DigestV1,
) -> crate::zk::kagemusha_v1_recursion::KagemushaGuardContextV1 {
    crate::zk::kagemusha_v1_recursion::KagemushaGuardContextV1 {
        release_id: normalized.release_id,
        liability_pool_id: normalized.liability_pool_id,
        lifecycle_binding_digest: normalized.lifecycle_binding_digest,
        prepared_transition_binding_digest: normalized.prepared_transition_binding_digest,
        terminal_commit_binding_digest: normalized.terminal_commit_binding_digest,
        sender_one_time_authorization_digest: normalized.sender_one_time_authorization_digest,
        receive_credit_binding_digest: normalized.receive_credit_binding_digest,
        transition_intent_digest: normalized.transition_intent_digest,
        transition_effect_digest: normalized.transition_effect_digest,
        recovery_record_digest: normalized.recovery_record_digest,
        durable_inbox_effect_digest: normalized.durable_inbox_effect_digest,
        durable_outbox_effect_digest: normalized.durable_outbox_effect_digest,
        canonical_empty_effect_digest: empty,
    }
}

#[derive(Clone)]
struct DiagnosticGuardVerifier<'a> {
    funded: &'a RealFundedPrerequisite,
    eq_protocol: PlonkProtocol<EqAffine>,
    ep_protocol: PlonkProtocol<EpAffine>,
    journal_key: KagemushaDevicePublicKeyV1,
    checkpoint: recovery_checkpoint::DiagnosticCheckpointRegister,
    proofs: Rc<RefCell<BTreeMap<DigestV1, Rc<GuardProof>>>>,
}

impl DiagnosticGuardVerifier<'_> {
    fn retain(&self, proof: Rc<GuardProof>) -> Vec<u8> {
        let frame = guard_frame(&proof);
        let statement = proof.relation.statement;
        assert!(
            self.proofs
                .borrow_mut()
                .insert(proof.relation.statement_digest(), proof)
                .is_none()
        );
        self.verify_normalized(&statement, &frame)
            .expect("verify retained actual Guard44 proof");
        frame
    }

    fn verify_normalized(
        &self,
        normalized: &KagemushaNormalizedGuardStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        let digest = normalized
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let proof = self
            .proofs
            .borrow()
            .get(&digest)
            .cloned()
            .ok_or_else(|| "no genuine diagnostic Guard proof retained".to_owned())?;
        ensure(
            proof.relation.statement == *normalized && guard_frame(&proof) == bytes,
            "diagnostic Guard statement/frame substitution",
        )?;
        let expected = guard_relation(&self.funded.material, *normalized);
        ensure(
            proof.relation.credential_digests() == expected.credential_digests(),
            "diagnostic Guard credential substitution",
        )?;
        let eq_instances = guard_public_instances::<Fp>(
            &expected,
            proof.eq_credential_audit,
            proof.ep_credential_audit,
            proof.eq_history.as_bytes(),
        );
        let ep_instances = guard_public_instances::<Fq>(
            &expected,
            proof.eq_credential_audit,
            proof.ep_credential_audit,
            proof.ep_history.as_bytes(),
        );
        let eq_current = decide_eq(
            &self.funded.eq,
            &self.eq_protocol,
            &proof.eq_proof,
            &eq_instances,
            &proof.eq_history,
        )?;
        let ep_current = decide_ep(
            &self.funded.ep,
            &self.ep_protocol,
            &proof.ep_proof,
            &ep_instances,
            &proof.ep_history,
        )?;
        ensure(
            eq_current == proof.eq_current && ep_current == proof.ep_current,
            "diagnostic Guard current-accumulator substitution",
        )
    }
}

impl KagemushaGuardBundleVerifierV1 for DiagnosticGuardVerifier<'_> {
    fn verify_mint_reservation(
        &self,
        statement: &MintReservationStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        verify_journal(&self.journal_key, RESERVATION_DOMAIN, statement, bytes)
    }

    fn verify_mint_stage(
        &self,
        statement: &MintStageStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        verify_journal(&self.journal_key, STAGE_DOMAIN, statement, bytes)
    }

    fn verify_bootstrap(
        &self,
        statement: &BootstrapStatementV1,
        normalized: &KagemushaNormalizedGuardStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        let expected = KagemushaNormalizedGuardStatementV1::from_bootstrap_state(
            statement,
            context_from_normalized(
                normalized,
                self.funded
                    .material
                    .platform_credential
                    .statement
                    .canonical_empty_effect_digest,
            ),
        )
        .map_err(|error| error.to_string())?;
        ensure(
            expected == *normalized,
            "diagnostic Core bootstrap/Guard mismatch",
        )?;
        self.verify_normalized(normalized, bytes)
    }

    fn verify_transition(
        &self,
        hardware: &HardwareTransitionStatementV1,
        proof: &TransitionProofStatementV1,
        normalized: &KagemushaNormalizedGuardStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        normalized
            .validate_hardware_binding(proof, hardware)
            .map_err(|error| error.to_string())?;
        self.verify_normalized(normalized, bytes)
    }

    fn verify_credit_stage(&self, _: &CreditStageStatementV1, _: &[u8]) -> Result<(), String> {
        Err("diagnostic milestone does not stage peer payments".to_owned())
    }

    fn verify_durability_anchor(
        &self,
        statement: &DurabilityAnchorStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.checkpoint.verify_anchor(statement, bytes)
    }

    fn verify_recovery_checkpoint_cas(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.checkpoint.verify_cas(statement, bytes)
    }

    fn verify_current_recovery_checkpoint(
        &self,
        statement: &DurabilityAnchorStatementV1,
        journals: &KagemushaRecoveryJournalsV1,
    ) -> Result<(), String> {
        self.checkpoint.verify_current(statement, journals)
    }
}

type DiagnosticMachine<'a> =
    KagemushaStateMachineV1<DiagnosticVerifier<'a>, DiagnosticGuardVerifier<'a>>;

fn diagnostic_release(
    material: &MintRecipientMaterial,
    artifacts: KagemushaRecursionArtifactsV1,
) -> KagemushaStateProofReleaseV1 {
    let profile = material.hardware_profile;
    KagemushaStateProofReleaseV1::from_test_artifacts(
        artifacts,
        vec![KagemushaEnabledProfileV1 {
            hardware_profile: profile,
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: material.authorization_relation.statement.context.suite_id,
            vk_digest: material.authorization_relation.statement.context.vk_digest,
            qualification_digest: digest(b"diagnostic-not-hardware-qualification", 0),
            policy_epoch: profile.policy_epoch,
            qualification_report: KagemushaEvidenceFileV1 {
                sha256: profile.qualification_report_digest,
                byte_len: 1,
            },
        }],
    )
    .expect("explicit test-only Core proof release")
}

fn provisional_artifacts(material: &MintRecipientMaterial) -> KagemushaRecursionArtifactsV1 {
    let mut artifacts = crate::zk::kagemusha_v1_recursion::tests::artifacts();
    artifacts.release_id = material.authorization_relation.statement.context.release_id;
    artifacts.artifact_manifest_digest = material
        .authorization_relation
        .statement
        .context
        .artifact_manifest_digest;
    artifacts.canonical_empty_effect_digest =
        canonical_empty_durable_effect_digest_v1(artifacts.release_id)
            .expect("Core canonical empty effect");
    artifacts
}

fn bootstrap_preview(
    material: &MintRecipientMaterial,
    artifacts: KagemushaRecursionArtifactsV1,
) -> crate::zk::kagemusha_v1_state::BootstrapPreviewV1 {
    let seed = aggregate_state(
        material.authorization_relation.statement.context.release_id,
        &material.platform_credential.statement,
        digest(b"state-milestone-bootstrap-nonce", 0),
    );
    DiagnosticMachine::preview_bootstrap(
        diagnostic_release(material, artifacts),
        seed.context(),
        seed.lane,
        seed.hardware_epoch,
        seed.device_policy_binding,
        seed.state_nonce_commitment,
        BOOTSTRAP_TIME,
    )
    .expect("Core zero-balance bootstrap preview")
}

// Explicit test ownership for this Unix diagnostic only. This selector is not an enrollment
// certificate; Core's test-only constructor still verifies the actual Bootstrap and Guard.
fn diagnostic_enrollment_binding(
    material: &MintRecipientMaterial,
    state: &crate::zk::kagemusha_v1_state::KagemushaStateV1,
) -> Result<KagemushaRecoveryEnrollmentBindingV1, String> {
    let owner = KagemushaRetailEnrollmentOwnerV1 {
        account_id: material.recipient.clone(),
        runtime: KagemushaRetailEnrollmentRuntimeV1 {
            fi_id: "diagnostic-fi".parse().expect("fixed diagnostic FI name"),
            ledger_dataspace_id: iroha_model_base::topology::DataSpaceId::new(10),
            authentication_namespace: "diagnostic-auth"
                .parse()
                .expect("fixed diagnostic authentication namespace"),
            network_id: state.lane.network_id,
            asset: state.lane.asset.clone(),
            asset_incarnation: state.asset_incarnation,
            scale: state.lane.scale,
        },
        lane_id: state.lane.device_lane_id,
    };
    Ok(KagemushaRecoveryEnrollmentBindingV1 {
        enrollment_id: owner.enrollment_id().map_err(|error| error.to_string())?,
        owner,
    })
}

fn exact_artifacts(
    funded: &RealFundedPrerequisite,
    keys: &StateKeys,
    guard: &GuardKeys,
    incoming: &IncomingStateProofMaterial,
) -> KagemushaRecursionArtifactsV1 {
    let mut artifacts = provisional_artifacts(&funded.material);
    artifacts.eq_protocol_digest = keys.eq_protocol_digest;
    artifacts.ep_protocol_digest = keys.ep_protocol_digest;
    artifacts.guard_bundle_eq_protocol_digest = guard.eq_protocol_digest;
    artifacts.guard_bundle_ep_protocol_digest = guard.ep_protocol_digest;
    artifacts.mint_authorization_eq_protocol_digest = funded.authorization_protocols.eq_digest;
    artifacts.mint_authorization_ep_protocol_digest = funded.authorization_protocols.ep_digest;
    artifacts.mint_finality_eq_protocol_digest = funded.mint_protocols.eq_digest;
    artifacts.mint_finality_ep_protocol_digest = funded.mint_protocols.ep_digest;
    artifacts.commit_wrapper_eq_protocol_digest =
        native_parent_protocol_digest_v1(&incoming.eq_protocol, KagemushaPastaParityV1::Eq)
            .expect("inactive Eq incoming protocol identity");
    artifacts.commit_wrapper_ep_protocol_digest =
        native_parent_protocol_digest_v1(&incoming.ep_protocol, KagemushaPastaParityV1::Ep)
            .expect("inactive Ep incoming protocol identity");
    artifacts
        .validate()
        .expect("shape-valid diagnostic artifacts, not a release attestation");
    artifacts
}

fn mint_reservation(funded: &RealFundedPrerequisite) -> MintInboxReservationV1 {
    let material = &funded.material;
    let opening = material.authorization_relation.credit_opening;
    let key_handle_binding = digest_bytes(
        b"diagnostic-mint-key-handle",
        &material.authorization_relation.recipient_key_handle_opening,
    );
    let bytes = MintInboxReservationV1::required_reservation_bytes(
        &funded.authorization.authorization,
        &material.hardware_credential,
        &opening,
        key_handle_binding,
    )
    .expect("exact diagnostic mint allocation");
    MintInboxReservationV1::new(
        funded.authorization.authorization.clone(),
        material.hardware_credential.clone(),
        opening,
        key_handle_binding,
        bytes,
    )
    .expect("retained original recipient reservation")
}

/// Retained openings for a later real terminal witness, with no hardware-authority capability.
///
/// The reproducible secrets and authorization counter are explicit test-provider inputs. The
/// predecessor and journal revision come from Core before preparation; none is reconstructed
/// from an opaque digest after the State proof has already committed to it.
#[derive(Clone)]
struct DiagnosticSenderOpeningsV1 {
    predecessor: KagemushaStateV1,
    journal_revision_before: u128,
    journal_revision_after: u128,
    authorization_counter_before: u128,
    authorization_counter_after: u128,
    one_use_hardware_authorization: DigestV1,
    commit_evidence_opening: KagemushaCommitEvidenceOpeningV1,
}

impl DiagnosticSenderOpeningsV1 {
    fn prepared_authorization_digest(&self) -> DigestV1 {
        canonical_prepared_one_use_authorization_digest_v1(
            KagemushaOperationV1::SendSplit,
            self.one_use_hardware_authorization,
            &self.predecessor,
            self.journal_revision_before,
            self.authorization_counter_before,
        )
    }

    fn commit_evidence(&self) -> Result<KagemushaCommitEvidenceV1, String> {
        ensure(
            self.commit_evidence_opening.trusted_commit_time_ms > 0,
            "diagnostic sender retains trusted-time evidence only",
        )?;
        Ok(KagemushaCommitEvidenceV1::TrustedTime(
            KagemushaTrustedCommitTimeV1 {
                time_evidence_commitment: canonical_commit_evidence_commitment_v1(
                    self.commit_evidence_opening,
                    self.authorization_counter_before,
                    self.authorization_counter_after,
                )?,
            },
        ))
    }

    fn validate_preparation(&self, preparation: &SendSplitPreparationV1) -> Result<(), String> {
        ensure(
            self.one_use_hardware_authorization != [0; 32],
            "diagnostic one-use authorization opening is zero",
        )?;
        ensure(
            self.journal_revision_before.checked_add(1) == Some(self.journal_revision_after),
            "diagnostic sender journal is not exact-next",
        )?;
        let authorization = self.prepared_authorization_digest();
        ensure(
            preparation.prepared_one_use_authorization_digest == authorization
                && preparation.transition_nullifier
                    == canonical_predecessor_conflict_nullifier_v1(authorization),
            "diagnostic sender preparation substituted its predecessor authorization",
        )?;
        ensure(
            self.commit_evidence_opening.trusted_commit_time_ms > 0
                && preparation.commit_authorization_reference_ms
                    == self.commit_evidence_opening.trusted_commit_time_ms
                && preparation.commit_evidence == self.commit_evidence()?,
            "diagnostic sender preparation substituted its original commit evidence",
        )
    }
}

fn send_preparation(
    material: &MintRecipientMaterial,
    state: &KagemushaStateV1,
    journal_revision_before: u128,
) -> (SendSplitPreparationV1, DiagnosticSenderOpeningsV1) {
    let openings = DiagnosticSenderOpeningsV1 {
        predecessor: state.clone(),
        journal_revision_before,
        journal_revision_after: journal_revision_before
            .checked_add(1)
            .expect("diagnostic sender journal has an exact successor"),
        // This fixture has no prior terminal sender authorization. These counters are retained
        // test-provider inputs, not a claim that software supplies rollback-resistant hardware.
        authorization_counter_before: 0,
        authorization_counter_after: 1,
        one_use_hardware_authorization: digest(b"diagnostic-send-one-use-opening", 1),
        commit_evidence_opening: KagemushaCommitEvidenceOpeningV1 {
            opening: digest(b"diagnostic-send-time-opening", 1),
            trusted_commit_time_ms: SEND_TIME,
            lease_id: [0; 32],
            lease_valid_from_ms: 0,
            lease_expires_at_ms: 0,
        },
    };
    let prepared_one_use_authorization_digest = openings.prepared_authorization_digest();
    // A distinct, issuer-signed receiver credential is sufficient for this candidate-only
    // milestone. Ciphertext bytes are only shape/commitment inputs; no delivery is claimed.
    let mut receiver = material.hardware_credential.clone();
    let receiver_key = deterministic_signing_key(1);
    receiver.device_public_key = device_public_key(&receiver_key);
    receiver.device_key_reference = kagemusha_device_key_reference_v1(&receiver.device_public_key);
    receiver.lane_commitment = digest(b"diagnostic-receiver-lane", 1);
    receiver.hardware_epoch_id = digest(b"diagnostic-receiver-epoch", 1);
    receiver.credential_id = [0; 32];
    receiver = receiver
        .seal_credential_id()
        .expect("diagnostic receiver identity");
    receiver.governance_signature = device_signature(
        &deterministic_signing_key(0x7000),
        &receiver
            .canonical_signing_bytes()
            .expect("diagnostic receiver issuer message"),
    );
    receiver
        .validate_against_profile(&material.hardware_profile)
        .expect("diagnostic receiver issuer signature");
    let mut request = KagemushaPaymentRequestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: state.release_id,
        network_id: state.lane.network_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        recipient: material.recipient.clone(),
        amount: 400,
        recipient_encryption_key: digest(b"diagnostic-send-x25519", 1),
        hardware_credential: receiver,
        request_id: digest(b"diagnostic-send-request", 1),
        issued_at_ms: 250,
        expires_at_ms: 1_000,
        signature: device_signature(&receiver_key, b"unsealed diagnostic receiver request"),
    };
    request.signature = device_signature(
        &receiver_key,
        &request
            .canonical_signing_bytes()
            .expect("diagnostic receiver request message"),
    );
    request
        .validate_against_profile(&material.hardware_profile)
        .expect("signed diagnostic receiver request");
    let preparation = SendSplitPreparationV1 {
        request,
        encrypted_credit: material.authorization_relation.encrypted_credit.clone(),
        transition_nullifier: canonical_predecessor_conflict_nullifier_v1(
            prepared_one_use_authorization_digest,
        ),
        ciphertext_commitment: digest(b"diagnostic-send-ciphertext-commitment", 1),
        successor_state_nonce_commitment: digest(b"diagnostic-send-successor-nonce", 1),
        commit_evidence: openings
            .commit_evidence()
            .expect("canonical original commit evidence"),
        commit_authorization_reference_ms: openings.commit_evidence_opening.trusted_commit_time_ms,
        outbox_reservation: KagemushaOutboxReservationV1 {
            reservation_id: digest(b"diagnostic-send-outbox", 1),
            operation_kind: KagemushaOperationKindV1::SendSplit,
            reserved_outbox_bytes: u32::try_from(KagemushaDurableCapacityV1::MINIMUM_OUTBOX_BYTES)
                .expect("diagnostic outbox fits u32"),
            issued_at_ms: 250,
            expires_at_ms: 1_000,
        },
        prepared_one_use_authorization_digest,
        sealed_transition_inputs: vec![0x51; 32],
        sealed_recovery_seeds: vec![0x52; 32],
    };
    openings
        .validate_preparation(&preparation)
        .expect("sender commitments have their exact retained private openings");
    (preparation, openings)
}

fn assert_altered_private_fold_proof_rejected(
    state_keys: &StateKeys,
    proof: &KagemushaGeneratedRecursiveStateProofV1,
) {
    let recovery_seed = test_only_recovery_seed();
    let eq_history = proof
        .eq_history
        .to_native()
        .expect("decode Eq private history for negative transport witness");
    let ep_history = proof
        .ep_history
        .to_native()
        .expect("decode Ep private history for negative transport witness");
    let eq_fold = fold_kagemusha_eq_accumulators_v1(
        &state_keys.eq.parameters,
        &proof.eq_current_accumulator,
        &proof.eq_history,
        &recovery_seed,
    )
    .expect("construct Eq fold control for negative transport witness");
    let ep_fold = fold_kagemusha_ep_accumulators_v1(
        &state_keys.ep.parameters,
        &proof.ep_current_accumulator,
        &proof.ep_history,
        &recovery_seed,
    )
    .expect("construct Ep fold control for negative transport witness");
    let mut altered_eq_fold = eq_fold.proof().as_bytes().to_vec();
    altered_eq_fold[64..96].fill(0xff);
    let Err(error) = build_kagemusha_transport_decider_pair_v1(
        &state_keys.eq.parameters,
        &state_keys.ep.parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.eq_protocol,
                inner_instances: &proof.eq_public_instances,
                inner_proof: &proof.eq_inner_proof,
                inner_history: &eq_history,
                inner_history_fold_proof: &altered_eq_fold,
                outer_instances: &proof.eq_transport_public_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.ep_protocol,
                inner_instances: &proof.ep_public_instances,
                inner_proof: &proof.ep_inner_proof,
                inner_history: &ep_history,
                inner_history_fold_proof: ep_fold.proof().as_bytes(),
                outer_instances: &proof.ep_transport_public_instances,
            },
        },
    ) else {
        panic!("the outer circuit builder accepted an altered Eq history-fold proof");
    };
    assert!(
        error.contains("failed to fold current carrier into history"),
        "unexpected altered-fold rejection: {error}",
    );

    let mut altered_ep_fold = ep_fold.proof().as_bytes().to_vec();
    altered_ep_fold[64..96].fill(0xff);
    let Err(error) = build_kagemusha_transport_decider_pair_v1(
        &state_keys.eq.parameters,
        &state_keys.ep.parameters,
        KagemushaTransportDeciderWitnessV1 {
            eq: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.eq_protocol,
                inner_instances: &proof.eq_public_instances,
                inner_proof: &proof.eq_inner_proof,
                inner_history: &eq_history,
                inner_history_fold_proof: eq_fold.proof().as_bytes(),
                outer_instances: &proof.eq_transport_public_instances,
            },
            ep: KagemushaTransportDeciderParityWitnessV1 {
                inner_protocol: &state_keys.ep_protocol,
                inner_instances: &proof.ep_public_instances,
                inner_proof: &proof.ep_inner_proof,
                inner_history: &ep_history,
                inner_history_fold_proof: &altered_ep_fold,
                outer_instances: &proof.ep_transport_public_instances,
            },
        },
    ) else {
        panic!("the outer circuit builder accepted an altered Ep history-fold proof");
    };
    assert!(
        error.contains("failed to fold current carrier into history"),
        "unexpected altered-fold rejection: {error}",
    );
}

fn assert_mixed_transport_pairs_rejected(
    state_keys: &StateKeys,
    first: &KagemushaGeneratedRecursiveStateProofV1,
    second: &KagemushaGeneratedRecursiveStateProofV1,
) {
    assert!(paired_transport_boundary_accepts(
        state_keys,
        &second.proof,
        second.proof.semantic_digest,
        &second.eq_transport_public_instances,
        &second.ep_transport_public_instances,
    ));

    let mut mixed = second.proof.clone();
    mixed.eq_proof.clone_from(&first.proof.eq_proof);
    mixed.eq_history.clone_from(&first.proof.eq_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject an old Eq proof/history mixed with a new Ep half",
    );

    mixed = second.proof.clone();
    mixed.eq_proof.clone_from(&first.proof.eq_proof);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject an old Eq proof paired with current histories",
    );

    mixed = second.proof.clone();
    mixed.ep_proof.clone_from(&first.proof.ep_proof);
    mixed.ep_history.clone_from(&first.proof.ep_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject a new Eq half mixed with an old Ep proof/history",
    );

    mixed = second.proof.clone();
    mixed.ep_proof.clone_from(&first.proof.ep_proof);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject an old Ep proof paired with current histories",
    );

    mixed = second.proof.clone();
    mixed.eq_history.clone_from(&first.proof.eq_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject a current Eq proof with an old Eq history",
    );

    mixed = second.proof.clone();
    mixed.ep_history.clone_from(&first.proof.ep_history);
    assert!(
        !paired_transport_boundary_accepts(
            state_keys,
            &mixed,
            second.proof.semantic_digest,
            &second.eq_transport_public_instances,
            &second.ep_transport_public_instances,
        ),
        "the outer boundary must reject a current Ep proof with an old Ep history",
    );
}

fn assert_state_mutations_rejected(
    keys: &StateKeys,
    proof: &KagemushaGeneratedRecursiveStateProofV1,
) {
    assert_transport_public_substitutions_rejected(keys, proof);
    assert_altered_private_fold_proof_rejected(keys, proof);
    let eq_terminal = KagemushaEqAccumulatorV1::try_from_bytes(&proof.proof.eq_history)
        .expect("Eq terminal history");
    let ep_terminal = KagemushaEpAccumulatorV1::try_from_bytes(&proof.proof.ep_history)
        .expect("Ep terminal history");
    for offset in [
        public_instance::AMOUNT,
        PUBLIC_INSTANCE_COUNT,
        RECURSIVE_PUBLIC_INSTANCE_COUNT - 1,
    ] {
        let mut eq_public = proof.eq_transport_public_instances.clone();
        eq_public[offset] += Fp::ONE;
        assert!(
            decide_eq(
                &keys.eq.parameters,
                &keys.eq_transport_protocol,
                &proof.proof.eq_proof,
                &eq_public,
                &eq_terminal
            )
            .is_err(),
            "retained Eq transport proof rejects public cell {offset}"
        );
        let mut ep_public = proof.ep_transport_public_instances.clone();
        ep_public[offset] += Fq::ONE;
        assert!(
            decide_ep(
                &keys.ep.parameters,
                &keys.ep_transport_protocol,
                &proof.proof.ep_proof,
                &ep_public,
                &ep_terminal
            )
            .is_err(),
            "retained Ep transport proof rejects public cell {offset}"
        );
        let mut eq_inner = proof.eq_public_instances.clone();
        eq_inner[offset] += Fp::ONE;
        assert!(
            decide_eq(
                &keys.eq.parameters,
                &keys.eq_protocol,
                &proof.eq_inner_proof,
                &eq_inner,
                &proof.eq_history
            )
            .is_err(),
            "retained Eq inner proof rejects public cell {offset}"
        );
        let mut ep_inner = proof.ep_public_instances.clone();
        ep_inner[offset] += Fq::ONE;
        assert!(
            decide_ep(
                &keys.ep.parameters,
                &keys.ep_protocol,
                &proof.ep_inner_proof,
                &ep_inner,
                &proof.ep_history
            )
            .is_err(),
            "retained Ep inner proof rejects public cell {offset}"
        );
    }
}

fn assert_guard_mutations_rejected(
    funded: &RealFundedPrerequisite,
    keys: &GuardKeys,
    proof: &GuardProof,
) {
    for offset in [6, 7, 8, 9, 10, GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1 - 1] {
        let mut eq_instances = guard_public_instances::<Fp>(
            &proof.relation,
            proof.eq_credential_audit,
            proof.ep_credential_audit,
            proof.eq_history.as_bytes(),
        );
        eq_instances[offset] += Fp::ONE;
        assert!(
            decide_eq(
                &funded.eq,
                &keys.eq_protocol,
                &proof.eq_proof,
                &eq_instances,
                &proof.eq_history
            )
            .is_err(),
            "genuine Eq Guard44 proof rejects credential/history cell {offset}"
        );
        let mut ep_instances = guard_public_instances::<Fq>(
            &proof.relation,
            proof.eq_credential_audit,
            proof.ep_credential_audit,
            proof.ep_history.as_bytes(),
        );
        ep_instances[offset] += Fq::ONE;
        assert!(
            decide_ep(
                &funded.ep,
                &keys.ep_protocol,
                &proof.ep_proof,
                &ep_instances,
                &proof.ep_history
            )
            .is_err(),
            "genuine Ep Guard44 proof rejects credential/history cell {offset}"
        );
    }
}

#[cfg(unix)]
fn run_state_milestone(milestone: DiagnosticMilestoneV1) {
    let started = std::time::Instant::now();
    let funded = prove_funded_prerequisite(
        digest(b"state-milestone-release", 0),
        digest(b"vk-set", 0),
        digest(b"state-milestone-manifest", 0),
        1_000,
    );
    let initial_preview =
        bootstrap_preview(&funded.material, provisional_artifacts(&funded.material));
    assert_eq!(
        initial_preview.state.balance, 0,
        "Core must derive the zero-balance base"
    );
    let mut guard_keys = None;
    let bootstrap_guard = Rc::new(prove_guard(
        &funded.eq,
        &funded.ep,
        &funded.credential_keys,
        &mut guard_keys,
        guard_relation(&funded.material, initial_preview.normalized_guard_statement),
        &funded.credential,
        &funded.credential,
    ));
    let incoming = IncomingStateProofMaterial::padding(
        &funded.eq,
        &funded.ep,
        guard_keys.as_ref().expect("Guard keys"),
    );
    let state_keys = generate_recursive_state_keys_for_corridor(
        &funded,
        &initial_preview.state,
        &bootstrap_guard,
        guard_keys.as_ref().expect("Guard keys"),
        &incoming,
    );
    let artifacts = exact_artifacts(
        &funded,
        &state_keys,
        guard_keys.as_ref().expect("Guard keys"),
        &incoming,
    );
    let preview = bootstrap_preview(&funded.material, artifacts);
    assert_eq!(
        initial_preview, preview,
        "key convergence cannot change Core's bootstrap statement"
    );
    let protocols = || {
        RecursiveStateProtocolBindings::new(
            state_keys.eq_protocol_digest,
            state_keys.ep_protocol_digest,
            guard_keys.as_ref().expect("Guard keys"),
            &funded,
            &incoming,
        )
    };
    let bootstrap_relation = bootstrap_relation_for_corridor(
        preview.state.clone(),
        &bootstrap_guard,
        preview.transport_semantic_digest,
        protocols(),
    );
    let parent = dummy_parent(
        &state_keys.eq_protocol,
        &state_keys.ep_protocol,
        initial_kagemusha_eq_accumulator_v1(&funded.eq).expect("zero Eq history"),
        initial_kagemusha_ep_accumulator_v1(&funded.ep).expect("zero Ep history"),
    );
    let bootstrap = Rc::new(prove_recursive_state_step(
        &funded,
        &state_keys,
        &bootstrap_guard,
        guard_keys.as_ref().expect("Guard keys"),
        &parent,
        &incoming,
        bootstrap_relation,
        None,
    ));
    let recursive_verifier = DiagnosticVerifier {
        funded: &funded,
        keys: &state_keys,
        artifacts,
        states: Rc::new(RefCell::new(BTreeMap::new())),
    };
    recursive_verifier.retain_state(bootstrap.clone());
    assert_state_mutations_rejected(&state_keys, &bootstrap);
    assert_guard_mutations_rejected(
        &funded,
        guard_keys.as_ref().expect("Guard keys"),
        &bootstrap_guard,
    );
    // The diagnostic journal key uses the very test-provider secret whose commitment belongs
    // to the issuer-approved policy row proved by PlatformCredential/Guard. This is fixture
    // provenance only: it supplies no rollback-resistant physical-hardware claim.
    let journal_key =
        SigningKey::from_bytes((&funded.material.provider_policy.provider_secret).into())
            .expect("diagnostic provider secret is a P-256 scalar");
    let checkpoint =
        recovery_checkpoint::DiagnosticCheckpointRegister::new(&funded.material, &preview.state)
            .expect("fixed diagnostic credential and simulated journal identity");
    let guard_verifier = DiagnosticGuardVerifier {
        funded: &funded,
        eq_protocol: guard_keys.as_ref().expect("Guard keys").eq_protocol.clone(),
        ep_protocol: guard_keys.as_ref().expect("Guard keys").ep_protocol.clone(),
        journal_key: device_public_key(&journal_key),
        checkpoint: checkpoint.clone(),
        proofs: Rc::new(RefCell::new(BTreeMap::new())),
    };
    let bootstrap_frame = guard_verifier.retain(bootstrap_guard);
    let verified_bootstrap = DiagnosticMachine::stage_bootstrap_for_test(
        diagnostic_release(&funded.material, artifacts),
        preview.state.context(),
        preview.state.lane.clone(),
        preview.state.hardware_epoch,
        preview.state.device_policy_binding,
        preview.state.state_nonce_commitment,
        BOOTSTRAP_TIME,
        KagemushaDurableCapacityV1 {
            inbox_bytes: 32 * 1024 * 1024,
            outbox_bytes: 32 * 1024 * 1024,
        },
        KagemushaMemoryAuthenticatedHistoryStoreV1::new(8 * 1024 * 1024),
        BootstrapAuthorizationV1 {
            proof: bootstrap.proof.clone(),
            guard_bundle: bootstrap_frame,
        },
        funded.material.hardware_credential,
        diagnostic_enrollment_binding(&funded.material, &preview.state)
            .expect("explicit synthetic recipient and exact runtime ownership"),
        recursive_verifier.clone(),
        guard_verifier.clone(),
    )
    .expect("Core accepts actual zero-balance Bootstrap proofs and exact Guard");
    let native_bootstrap_storage =
        tempfile::tempdir().expect("private bootstrap journal directory");
    // Resolve the platform's temporary-root aliases before Core checks every component without
    // following symlinks. Retain the TempDir owner alongside this canonical path and both stores.
    let native_bootstrap_directory = native_bootstrap_storage
        .path()
        .canonicalize()
        .expect("canonical private bootstrap journal directory");
    let pending_bootstrap = verified_bootstrap
        .initialize_journals(
            &native_bootstrap_directory.join("journals"),
            32 * 1024 * 1024,
            digest(b"state-milestone-bootstrap-checkpoint", 0),
        )
        .expect("Core creates and fsyncs the actual descriptor-owned initialization journals");
    checkpoint
        .retain_pending(&pending_bootstrap)
        .expect("retain exact pending snapshot in explicitly simulated storage");
    let checkpoint_certificate = checkpoint
        .commit(pending_bootstrap.statement())
        .expect("signed simulated initial CAS from canonical empty predecessor");
    // Named bindings retain both actual descriptor locks through the entire diagnostic.
    let (mut machine, _held_coordinator, _held_responses) = pending_bootstrap
        .finish(checkpoint_certificate)
        .expect("Core checks actual journal material, original CAS and fresh simulated selection")
        .into_parts();
    assert_eq!(machine.state(), &preview.state);

    let reservation = mint_reservation(&funded);
    let reservation_statement = machine
        .preview_mint_reservation(&reservation)
        .expect("Core mint allocation preview");
    let mut reservation_certificate = MintReservationCertificateV1 {
        guard_bundle: sign_journal(&journal_key, RESERVATION_DOMAIN, &reservation_statement),
        statement: reservation_statement,
    };
    reservation_certificate.guard_bundle[0] ^= 1;
    assert!(
        machine
            .reserve_mint_credit(&reservation, &reservation_certificate)
            .is_err(),
        "invalid test-provider signature cannot stage an allocation"
    );
    reservation_certificate.guard_bundle = sign_journal(
        &journal_key,
        RESERVATION_DOMAIN,
        &reservation_certificate.statement,
    );
    machine
        .reserve_mint_credit(&reservation, &reservation_certificate)
        .expect("Core installs signed diagnostic allocation");
    let mut substituted_credit = funded.mint_credit.clone();
    substituted_credit.proof.eq_proof[0] ^= 1;
    assert!(
        recursive_verifier
            .verify_stage(&reservation, &substituted_credit)
            .is_err()
    );
    substituted_credit = funded.mint_credit.clone();
    substituted_credit.proof.ep_proof[0] ^= 1;
    assert!(
        recursive_verifier
            .verify_stage(&reservation, &substituted_credit)
            .is_err()
    );
    let token = recursive_verifier
        .verify_stage(&reservation, &funded.mint_credit)
        .expect("actual recipient authorization and finality proofs");
    substituted_credit = funded.mint_credit.clone();
    substituted_credit.encrypted_credit[0] ^= 1;
    assert!(
        VerifiedMintStageV1::from_genuine_diagnostic_proofs(
            reservation.clone(),
            substituted_credit,
            token
        )
        .is_err(),
        "verified diagnostic token cannot move to another envelope"
    );
    let token = recursive_verifier
        .verify_stage(&reservation, &funded.mint_credit)
        .expect("fresh exact proof verification token");
    let verified = VerifiedMintStageV1::from_genuine_diagnostic_proofs(
        reservation,
        funded.mint_credit.clone(),
        token,
    )
    .expect("state-owned finalized mint capability after real proof verification");
    let finality = verified.mint_finality();
    let stage_statement = machine
        .preview_stage_mint_credit(&verified, STAGE_TIME)
        .expect("Core exact mint stage preview");
    let stage_certificate = MintStageCertificateV1 {
        guard_bundle: sign_journal(&journal_key, STAGE_DOMAIN, &stage_statement),
        statement: stage_statement,
    };
    machine
        .stage_mint_credit(
            &funded.authorization.authorization,
            &funded.mint_credit,
            Some(&verified),
            Some(&stage_certificate),
        )
        .expect("Core stages the exact proof-authenticated mint");
    assert_eq!(
        machine.state().balance,
        0,
        "staging alone must not create balance"
    );

    let mint_preview = machine
        .preview_mint_fold(
            &funded.mint_credit,
            digest(b"state-milestone-mint-nonce", 1),
            MINT_TIME,
        )
        .expect("Core derives authenticated replay path and state-owned mint opening");
    assert_eq!(mint_preview.transition.successor.balance, 1_000);
    let mint_guard = Rc::new(prove_guard(
        &funded.eq,
        &funded.ep,
        &funded.credential_keys,
        &mut guard_keys,
        guard_relation(
            &funded.material,
            mint_preview.transition.normalized_guard_statement,
        ),
        &funded.credential,
        &funded.credential,
    ));
    let mint_frame = guard_verifier.retain(mint_guard.clone());
    let mint_relation = transition_relation_for_corridor(
        machine.state().clone(),
        &mint_preview.transition,
        &mint_guard,
        RecursiveStateProtocolBindings::new(
            state_keys.eq_protocol_digest,
            state_keys.ep_protocol_digest,
            guard_keys.as_ref().expect("same Guard keys"),
            &funded,
            &incoming,
        ),
        Some(KagemushaReplayInsertWitnessV1::from(
            &mint_preview.replay_insert_witness,
        )),
        None,
    );
    let parent = parent_from_generated((*bootstrap).clone());
    let mint = Rc::new(prove_recursive_state_step(
        &funded,
        &state_keys,
        &mint_guard,
        guard_keys.as_ref().expect("same Guard keys"),
        &parent,
        &incoming,
        mint_relation,
        Some(mint_preview.mint_fold_opening()),
    ));
    recursive_verifier.retain_state(mint.clone());
    assert_state_mutations_rejected(&state_keys, &mint);
    assert_mixed_transport_pairs_rejected(&state_keys, &bootstrap, &mint);
    let mint_authorization = TransitionAuthorizationV1::new(
        HardwareTransitionCertificateV1 {
            statement: mint_preview.transition.hardware_statement.clone(),
            guard_bundle: mint_frame,
        },
        mint.proof.clone(),
    );
    let device_key = deterministic_signing_key(0);
    assert_eq!(
        device_public_key(&device_key),
        funded.material.hardware_credential.device_public_key
    );
    let root_message = machine
        .mint_fold_history_root_selection_signing_bytes(&mint_preview)
        .expect("Core exact root-selection bytes");
    let mint_authorization = machine
        .authorize_mint_fold_history(
            &mint_preview,
            mint_authorization,
            &device_public_key(&device_key),
            device_signature(&device_key, &root_message),
        )
        .expect("actual paired mint proof and diagnostic signed history root selection");
    let expected_funded = mint_preview.transition.successor.clone();
    machine
        .mint_fold_prepared(
            funded.mint_credit.clone(),
            mint_preview,
            finality,
            mint_authorization,
        )
        .expect("Core installs balance only through the genuine MintFold");
    assert_eq!(machine.state(), &expected_funded);
    assert_eq!(machine.state().balance, 1_000);
    assert!(
        machine
            .preview_mint_fold(
                &funded.mint_credit,
                digest(b"state-milestone-duplicate-nonce", 1),
                MINT_TIME + 1
            )
            .is_err(),
        "the actual mint credit cannot increase balance twice"
    );

    let (send_inputs, sender_openings) = send_preparation(
        &funded.material,
        machine.state(),
        machine.journal_revision(),
    );
    let candidate = machine
        .prepare_send_split(send_inputs.clone())
        .expect("Core prepares SendSplit from genuine funded balance");
    let send_preview = machine
        .diagnostic_send_split_preview(&candidate, SEND_TIME)
        .expect("exact original Core send preview");
    assert_eq!(candidate.predecessor_state, sender_openings.predecessor);
    assert_eq!(
        send_preview.proof_statement.journal_revision_before,
        sender_openings.journal_revision_before
    );
    assert_eq!(
        send_preview.proof_statement.journal_revision_after,
        sender_openings.journal_revision_after
    );
    assert_eq!(
        candidate.prepared_one_use_authorization_digest,
        sender_openings.prepared_authorization_digest()
    );
    let PreparedOutgoingRecoveryViewV1::Send { output, .. } = candidate.recovery_view() else {
        panic!("Core SendSplit must retain its original payment projection");
    };
    assert_eq!(
        output.transition_nullifier,
        send_inputs.transition_nullifier
    );
    assert_eq!(
        output.commit_evidence,
        sender_openings.commit_evidence().unwrap()
    );
    assert_eq!(
        output.committed_at_ms,
        sender_openings
            .commit_evidence_opening
            .trusted_commit_time_ms
    );
    assert!(
        machine
            .diagnostic_send_split_preview(&candidate, SEND_TIME + 1)
            .is_err(),
        "prepared time cannot be refreshed"
    );
    let mut detached = candidate.clone();
    detached.predecessor_state.state_nonce_commitment[0] ^= 1;
    assert!(
        machine
            .diagnostic_send_split_preview(&detached, SEND_TIME)
            .is_err(),
        "candidate from another predecessor cannot be reconstructed"
    );
    detached = candidate.clone();
    detached.proof_statement.amount += 1;
    assert!(
        machine
            .diagnostic_send_split_preview(&detached, SEND_TIME)
            .is_err(),
        "detached statement cannot replace Core derivation"
    );
    assert_eq!(send_preview.successor.balance, 600);
    let send_guard = Rc::new(prove_guard(
        &funded.eq,
        &funded.ep,
        &funded.credential_keys,
        &mut guard_keys,
        guard_relation(&funded.material, send_preview.normalized_guard_statement),
        &funded.credential,
        &funded.credential,
    ));
    let send_frame = guard_verifier.retain(send_guard.clone());
    let mut send_relation = transition_relation_for_corridor(
        machine.state().clone(),
        &send_preview,
        &send_guard,
        RecursiveStateProtocolBindings::new(
            state_keys.eq_protocol_digest,
            state_keys.ep_protocol_digest,
            guard_keys.as_ref().expect("same Guard keys"),
            &funded,
            &incoming,
        ),
        None,
        None,
    );
    // Outgoing candidates are verified against the canonical payment body. Core's local
    // preview digest instead identifies its journal transition and is not the candidate's
    // public semantic digest. Bind the actual body before generating either parity.
    send_relation.transport_semantic_digest = candidate
        .semantic_digest()
        .expect("Core candidate's proof-independent payment body");
    let parent = parent_from_generated((*mint).clone());
    let send = Rc::new(prove_recursive_state_step(
        &funded,
        &state_keys,
        &send_guard,
        guard_keys.as_ref().expect("same Guard keys"),
        &parent,
        &incoming,
        send_relation,
        None,
    ));
    recursive_verifier.retain_state(send.clone());
    assert_state_mutations_rejected(&state_keys, &send);
    assert_mixed_transport_pairs_rejected(&state_keys, &mint, &send);
    let public = candidate
        .candidate_public_inputs(artifacts, &send.proof)
        .expect("Core reconstructs candidate public inputs");
    crate::zk::kagemusha_v1_recursion::verify_kagemusha_state_proof_v1(
        &recursive_verifier,
        artifacts,
        &public,
        &send.proof,
    )
    .expect("actual SendSplit state proof matches Core candidate");
    guard_verifier
        .verify_transition(
            &send_preview.hardware_statement,
            &send_preview.proof_statement,
            &send_preview.normalized_guard_statement,
            &send_frame,
        )
        .expect("actual SendSplit Guard44 proof matches Core candidate");
    let mut changed_public = public.clone();
    changed_public.amount += 1;
    assert!(
        crate::zk::kagemusha_v1_recursion::verify_kagemusha_state_proof_v1(
            &recursive_verifier,
            artifacts,
            &changed_public,
            &send.proof
        )
        .is_err()
    );
    assert_eq!(recursive_verifier.states.borrow().len(), 3);
    sender_openings
        .validate_preparation(&send_inputs)
        .expect("retain the original private sender openings after State proof generation");
    assert_eq!(
        machine.state().balance,
        1_000,
        "an uncommitted SendSplit proof is not a terminal hardware commit"
    );
    eprintln!(
        "KAGEMUSHA diagnostic genuine paired State milestone: Bootstrap 0 -> MintFold 1000 -> SendSplit candidate 600; three inner/transport/history pairs verified in {:?}; no hardware or handoff qualification",
        started.elapsed()
    );
    if milestone != DiagnosticMilestoneV1::State {
        let terminal = terminal::prove_sender_terminal(
            &funded,
            &state_keys,
            &mut guard_keys,
            &recursive_verifier,
            artifacts,
            candidate,
            &send,
            &send_guard,
            &send_inputs,
            &sender_openings,
        );
        if milestone == DiagnosticMilestoneV1::Wrapper {
            let (eq_wrapper_seed, ep_wrapper_seed) =
                wrapper::prove_sender_wrapper(&funded, artifacts, &incoming, terminal);
            assert_eq!(eq_wrapper_seed.num_instance, [81]);
            assert_eq!(ep_wrapper_seed.num_instance, [81]);
        }
        assert_eq!(machine.state().balance, 1_000);
    }
}

/// Run only in the exclusive expensive real-proof lane; no generated model evidence is accepted.
#[test]
#[cfg(unix)]
#[ignore = "expensive genuine Bootstrap/MintFold/SendSplit diagnostic; not 1024-handoff qualification"]
fn real_bootstrap_mint_fold_send_split_state_milestone() {
    let _exclusive_proof = exclusive_real_proof_test_lock();
    std::thread::Builder::new()
        .name("kagemusha-real-state-milestone".to_owned())
        .stack_size(REAL_PROOF_TEST_STACK_BYTES)
        .spawn(|| run_state_milestone(DiagnosticMilestoneV1::State))
        .expect("start genuine state proof milestone")
        .join()
        .expect("genuine state proof milestone");
}

/// Extend the genuine State chain through internal terminal authorization, without a wrapper.
#[test]
#[cfg(unix)]
#[ignore = "expensive genuine State/TerminalAuthorization diagnostic; no CommitWrapper or handoff qualification"]
fn real_bootstrap_mint_fold_send_split_terminal_authorization_milestone() {
    let _exclusive_proof = exclusive_real_proof_test_lock();
    std::thread::Builder::new()
        .name("kagemusha-real-terminal-milestone".to_owned())
        .stack_size(REAL_PROOF_TEST_STACK_BYTES)
        .spawn(|| run_state_milestone(DiagnosticMilestoneV1::Terminal))
        .expect("start genuine terminal proof milestone")
        .join()
        .expect("genuine terminal proof milestone");
}

/// Measure a genuine CommitWrapper seed after the actual State and terminal proof pairs.
#[test]
#[cfg(unix)]
#[ignore = "expensive genuine State/Terminal/CommitWrapper seed; final graph and handoff remain unqualified"]
fn real_bootstrap_mint_fold_send_split_commit_wrapper_milestone() {
    let _exclusive_proof = exclusive_real_proof_test_lock();
    std::thread::Builder::new()
        .name("kagemusha-real-wrapper-milestone".to_owned())
        .stack_size(REAL_PROOF_TEST_STACK_BYTES)
        .spawn(|| run_state_milestone(DiagnosticMilestoneV1::Wrapper))
        .expect("start genuine wrapper seed milestone")
        .join()
        .expect("genuine wrapper seed milestone");
}

#[test]
fn diagnostic_core_bootstrap_preflight_is_zero_and_recipient_bound() {
    let material = core_bound_mint_recipient_material(
        digest(b"cheap-state-preflight-release", 0),
        digest(b"vk-set", 0),
        digest(b"cheap-state-preflight-manifest", 0),
        1_000,
    );
    let preview = bootstrap_preview(&material, provisional_artifacts(&material));
    assert_eq!(
        material
            .platform_credential
            .statement
            .canonical_empty_effect_digest,
        canonical_empty_durable_effect_digest_v1(preview.state.release_id).unwrap()
    );
    assert_eq!(
        material.authorization_relation.platform_credential,
        material.platform_credential.statement
    );
    let guard = guard_relation(&material, preview.normalized_guard_statement);
    assert_eq!(
        guard.canonical_empty_effect_digest,
        material
            .platform_credential
            .statement
            .canonical_empty_effect_digest
    );
    assert_eq!(
        guard.credential_digests(),
        [material.platform_credential.statement.canonical_digest(); 2]
    );
    assert_eq!(
        preview.state.device_policy_binding.hardware_policy_id,
        material.provider_policy.root
    );
    // This is a public-column constructor check only. Zero history bytes are never verified or
    // accepted here; the expensive test supplies genuine histories and mutates all four limbs.
    let eq_columns = guard_public_instances::<Fp>(
        &guard,
        [1; 32],
        [2; 32],
        &[0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    );
    let ep_columns = guard_public_instances::<Fq>(
        &guard,
        [1; 32],
        [2; 32],
        &[0; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    );
    let credential_digest = material.platform_credential.statement.canonical_digest();
    assert_eq!(eq_columns.len(), 44);
    assert_eq!(ep_columns.len(), 44);
    assert_eq!(&eq_columns[6..8], &digest_limbs::<Fp>(credential_digest));
    assert_eq!(&eq_columns[8..10], &digest_limbs::<Fp>(credential_digest));
    assert_eq!(&ep_columns[6..8], &digest_limbs::<Fq>(credential_digest));
    assert_eq!(&ep_columns[8..10], &digest_limbs::<Fq>(credential_digest));
    assert_eq!(preview.state.balance, 0);
    assert_eq!(preview.state.logical_sequence, 0);
    assert_eq!(
        preview.state.device_policy_binding.device_key_reference,
        material.hardware_credential.device_key_reference
    );
    assert_eq!(
        preview.state.hardware_epoch.epoch_id,
        material.hardware_credential.hardware_epoch_id
    );
    assert_eq!(material.authorization_relation.credit_opening.amount, 1_000);
    material
        .authorization_relation
        .validate_shape()
        .expect("exact original recipient authorization preflight");
    let (send, openings) = send_preparation(&material, &preview.state, 0);
    openings.validate_preparation(&send).unwrap();
    send.request
        .validate_against_profile(&material.hardware_profile)
        .expect("test receiver issuer/device signatures");
    assert_eq!(send.commit_authorization_reference_ms, SEND_TIME);
    assert!(
        send.request.amount > preview.state.balance,
        "preflight has not invented spendable balance"
    );
}

#[test]
fn diagnostic_sender_openings_bind_original_predecessor_counters_and_commit_time() {
    let material = core_bound_mint_recipient_material(
        digest(b"sender-opening-preflight-release", 0),
        digest(b"vk-set", 0),
        digest(b"sender-opening-preflight-manifest", 0),
        1_000,
    );
    let preview = bootstrap_preview(&material, provisional_artifacts(&material));
    let (preparation, openings) = send_preparation(&material, &preview.state, 0);
    openings.validate_preparation(&preparation).unwrap();
    assert_eq!(openings.predecessor, preview.state);
    assert_eq!(preview.state.balance, 0);
    assert!(preparation.request.amount > preview.state.balance);
    assert_eq!(
        openings.commit_evidence_opening.trusted_commit_time_ms,
        SEND_TIME
    );
    assert_eq!(
        (
            openings.authorization_counter_before,
            openings.authorization_counter_after
        ),
        (0, 1)
    );
    // This cheap check hashes authentic zero-balance Bootstrap context only. It never creates a
    // machine, claims a funded state, commits an outgoing candidate, or accepts a proof.
    type OpeningMutation = (&'static str, fn(&mut DiagnosticSenderOpeningsV1));
    let mutations: [OpeningMutation; 15] = [
        ("one-use secret", |changed| {
            changed.one_use_hardware_authorization[0] ^= 1
        }),
        ("zero one-use secret", |changed| {
            changed.one_use_hardware_authorization = [0; 32]
        }),
        ("predecessor commitment", |changed| {
            changed.predecessor.state_commitment[0] ^= 1
        }),
        ("predecessor nonce", |changed| {
            changed.predecessor.state_nonce_commitment[0] ^= 1
        }),
        ("device lane", |changed| {
            changed.predecessor.lane.device_lane_id[0] ^= 1
        }),
        ("hardware epoch", |changed| {
            changed.predecessor.hardware_epoch.epoch_id[0] ^= 1
        }),
        ("device key", |changed| {
            changed
                .predecessor
                .device_policy_binding
                .device_key_reference[0] ^= 1
        }),
        ("logical sequence", |changed| {
            changed.predecessor.logical_sequence += 1
        }),
        ("journal pair", |changed| {
            changed.journal_revision_before += 1;
            changed.journal_revision_after += 1;
        }),
        ("non-next journal", |changed| {
            changed.journal_revision_after += 1
        }),
        ("authorization counter pair", |changed| {
            changed.authorization_counter_before += 1;
            changed.authorization_counter_after += 1;
        }),
        ("non-next authorization counter", |changed| {
            changed.authorization_counter_after += 1
        }),
        ("evidence opening", |changed| {
            changed.commit_evidence_opening.opening[0] ^= 1
        }),
        ("original trusted time", |changed| {
            changed.commit_evidence_opening.trusted_commit_time_ms += 1
        }),
        ("inactive lease identity", |changed| {
            changed.commit_evidence_opening.lease_id[0] = 1
        }),
    ];
    for (label, mutate) in mutations {
        let mut changed = openings.clone();
        mutate(&mut changed);
        assert!(
            changed.validate_preparation(&preparation).is_err(),
            "retaining the original preparation must reject substituted {label}"
        );
    }

    type PreparationMutation = (&'static str, fn(&mut SendSplitPreparationV1));
    let mutations: [PreparationMutation; 4] = [
        ("authorization digest", |changed| {
            changed.prepared_one_use_authorization_digest[0] ^= 1
        }),
        ("transition nullifier", |changed| {
            changed.transition_nullifier[0] ^= 1
        }),
        ("evidence commitment", |changed| {
            let KagemushaCommitEvidenceV1::TrustedTime(evidence) = &mut changed.commit_evidence
            else {
                panic!("this fixture retains trusted-time evidence");
            };
            evidence.time_evidence_commitment[0] ^= 1;
        }),
        ("refreshed reference time", |changed| {
            changed.commit_authorization_reference_ms += 1
        }),
    ];
    for (label, mutate) in mutations {
        let mut changed = preparation.clone();
        mutate(&mut changed);
        assert!(
            openings.validate_preparation(&changed).is_err(),
            "retaining the original openings must reject substituted {label}"
        );
    }
    let mut refreshed_time = openings.clone();
    refreshed_time
        .commit_evidence_opening
        .trusted_commit_time_ms += 1;
    let mut matching_reference = preparation.clone();
    matching_reference.commit_authorization_reference_ms += 1;
    assert!(
        refreshed_time
            .validate_preparation(&matching_reference)
            .is_err(),
        "refreshing both private time and public reference cannot preserve the original commitment"
    );
    let mut changed_counter = openings.clone();
    changed_counter.authorization_counter_before += 1;
    changed_counter.authorization_counter_after += 1;
    let mut matching_authorization = preparation.clone();
    matching_authorization.prepared_one_use_authorization_digest =
        changed_counter.prepared_authorization_digest();
    matching_authorization.transition_nullifier = canonical_predecessor_conflict_nullifier_v1(
        matching_authorization.prepared_one_use_authorization_digest,
    );
    assert!(
        changed_counter
            .validate_preparation(&matching_authorization)
            .is_err(),
        "updating the predecessor authorization still cannot reuse the original counter-bound evidence"
    );
    assert!(
        canonical_commit_evidence_commitment_v1(openings.commit_evidence_opening, u128::MAX, 0)
            .is_err()
    );
    let mut noncanonical = openings.commit_evidence_opening;
    noncanonical.opening = [0; 32];
    assert!(canonical_commit_evidence_commitment_v1(noncanonical, 0, 1).is_err());
    noncanonical = openings.commit_evidence_opening;
    noncanonical.trusted_commit_time_ms = 0;
    assert!(canonical_commit_evidence_commitment_v1(noncanonical, 0, 1).is_err());
    assert_ne!(
        canonical_prepared_one_use_authorization_digest_v1(
            KagemushaOperationV1::RedeemSplit,
            openings.one_use_hardware_authorization,
            &openings.predecessor,
            openings.journal_revision_before,
            openings.authorization_counter_before,
        ),
        preparation.prepared_one_use_authorization_digest,
        "a redemption authorization cannot replace this send authorization"
    );
}

#[test]
fn diagnostic_provider_journal_signatures_bind_operation_and_exact_bytes() {
    #[derive(norito::Encode)]
    struct PayloadOnlyStatement {
        operation: u16,
        identity: [u8; 32],
        amount: u128,
    }
    let statement = PayloadOnlyStatement {
        operation: 1,
        identity: [7; 32],
        amount: 3,
    };
    let key = deterministic_signing_key(0x7100);
    let signature = sign_journal(&key, RESERVATION_DOMAIN, &statement);
    verify_journal(
        &device_public_key(&key),
        RESERVATION_DOMAIN,
        &statement,
        &signature,
    )
    .unwrap();
    let mut expected = RESERVATION_DOMAIN.to_vec();
    expected.extend(norito::codec::Encode::encode(&statement));
    assert_eq!(
        journal_message(RESERVATION_DOMAIN, &statement).unwrap(),
        expected
    );
    assert!(
        verify_journal(
            &device_public_key(&key),
            STAGE_DOMAIN,
            &statement,
            &signature
        )
        .is_err()
    );
    let changed = PayloadOnlyStatement {
        amount: 4,
        ..statement
    };
    assert!(
        verify_journal(
            &device_public_key(&key),
            RESERVATION_DOMAIN,
            &changed,
            &signature
        )
        .is_err()
    );
    let key = deterministic_signing_key(0x7100);
    let public = device_public_key(&key);
    let signature = sign_journal(&key, RESERVATION_DOMAIN, &(1_u16, [7_u8; 32], 3_u128));
    verify_journal(
        &public,
        RESERVATION_DOMAIN,
        &(1_u16, [7_u8; 32], 3_u128),
        &signature,
    )
    .unwrap();
    assert!(
        verify_journal(
            &public,
            STAGE_DOMAIN,
            &(1_u16, [7_u8; 32], 3_u128),
            &signature
        )
        .is_err()
    );
    assert!(
        verify_journal(
            &public,
            RESERVATION_DOMAIN,
            &(1_u16, [7_u8; 32], 4_u128),
            &signature
        )
        .is_err()
    );
    assert!(
        verify_journal(
            &device_public_key(&deterministic_signing_key(0x7101)),
            RESERVATION_DOMAIN,
            &(1_u16, [7_u8; 32], 3_u128),
            &signature
        )
        .is_err()
    );
}
