//! Final zero-knowledge terminal authorization for Kagemusha V1.
//!
//! A state proof is generated before hardware commit and therefore cannot contain terminal
//! evidence without creating a digest cycle. This module keeps the phases explicit: the private
//! candidate relation binds request/state/outbox commitments, hardware commits that exact
//! candidate once, and this terminal-authorization relation recursively verifies both the candidate and a postcommit
//! terminal-guard proof. Only an unlinkable projection is exposed by the final proof.

use iroha_data_model::kagemusha::{
    KAGEMUSHA_CREDIT_ID_DOMAIN_V1, KAGEMUSHA_PAYMENT_OUTBOX_MIN_BYTES_V1,
    KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1,
    KagemushaCommitCertificateV1, KagemushaCommitEvidenceV1, KagemushaHardwareCredentialV1,
    KagemushaHardwareProfileV1, KagemushaLifecycleBindingV1, KagemushaOperationKindV1,
    KagemushaOutboxReservationV1, KagemushaPaymentOutputV1, KagemushaPaymentRequestV1,
    kagemusha_asset_identity_digest_v1, kagemusha_payment_body_digest_from_digests_v1,
    kagemusha_prepared_transfer_digest_v1,
};
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use sha2::{Digest as _, Sha256};

use halo2_base::utils::{BigPrimeField, fe_to_biguint};

use super::{DigestV1, KagemushaOperationV1, KagemushaStateRelationPublicInputsV1, state_relation};
use crate::zk::{kagemusha_v1_poseidon::decode, kagemusha_v1_state::KagemushaStateV1};

#[cfg(feature = "zk-halo2-ipa")]
use ff::Field as _;
#[cfg(feature = "zk-halo2-ipa")]
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::CurveAffineExt,
};
#[cfg(feature = "zk-halo2-ipa")]
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::ipa::commitment::ParamsIPA,
};
#[cfg(feature = "zk-halo2-ipa")]
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

#[cfg(feature = "zk-halo2-ipa")]
use super::{
    KagemushaEpAccumulatorV1, KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1,
    KagemushaEqFoldProofV1, KagemushaGuardBundleRelationWitnessV1, KagemushaPastaParityV1,
    deferred_parent::{
        DeferredLoader, DeferredScalar, KagemushaDeferredParentOutputV1, accumulator_limb_count,
        bind_accumulator_limbs, constrain_reciprocal_output_with_u128_binding_serialized_v1,
        constrain_reciprocal_parent_pass_v1, deferred_field_chips_v1, deferred_loader_v1,
        finalize_deferred_audit_plan_v1, finalize_tagged_deferred_audit_with_u128_binding_v1,
        load_native_accumulator, native_parent_protocol_digest_v1, verify_fold,
        verify_ordinary_proof_v1,
    },
    guard_bundle::{
        GUARD_EP_AUDIT_OFFSET_V1, GUARD_EQ_AUDIT_OFFSET_V1, GUARD_HISTORY_OFFSET_V1,
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, KagemushaAssignedGuardBundleV1, assign_bytes,
        constant_bytes, constrain_guard_bundle_semantics_v1, digest_limbs_assigned, hash,
    },
};
#[cfg(feature = "zk-halo2-ipa")]
use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};
#[cfg(feature = "zk-halo2-ipa")]
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HARDWARE_CREDENTIAL_ID_LANE_OFFSET_V1,
    KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1,
    kagemusha_hardware_credential_id_preimage_layout_v1,
};

const COMMIT_CERTIFICATE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:commit-certificate";
const COMMIT_CERTIFICATE_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:commit-certificate-id";
const OUTBOX_RESERVATION_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:outbox-reservation";
const PREPARED_TRANSITION_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:prepared-transition-binding\0";
const PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:prepared-one-use-authorization\0";
const COMMIT_EVIDENCE_OPENING_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:commit-evidence-opening\0";
const TERMINAL_COMMIT_BINDING_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-commit-binding\0";
pub(crate) const TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:terminal-send-output-binding\0";
const PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:predecessor-conflict-nullifier\0";

const OUTBOX_RESERVATION_CANONICAL_BYTES_V1: usize = 56;
const COMMIT_CERTIFICATE_ID_CANONICAL_BYTES_V1: usize = 238;
const COMMIT_CERTIFICATE_CANONICAL_BYTES_V1: usize = 270;
/// Fixed release-pinned hardware-profile table width.
pub(crate) const TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1: usize = 64;

/// Number of public field elements in one terminal-authorization parity, including history.
pub(crate) const TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1: usize = 81;
/// Number of non-history public field elements in one terminal-authorization parity.
pub(crate) const TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1: usize = 47;

/// Public-instance offsets shared by both terminal-authorization parities.
pub(crate) mod public_instance {
    pub(crate) const OPERATION: usize = 0;
    pub(crate) const PROTOCOL_VERSION: usize = 1;
    pub(crate) const SUITE_LO: usize = 2;
    pub(crate) const VK_LO: usize = 4;
    pub(crate) const RELEASE_LO: usize = 6;
    pub(crate) const NETWORK_LO: usize = 8;
    pub(crate) const ASSET_LO: usize = 10;
    pub(crate) const ASSET_INCARNATION_LO: usize = 12;
    pub(crate) const ASSET_SCALE: usize = 14;
    pub(crate) const LIABILITY_POOL_LO: usize = 15;
    pub(crate) const HARDWARE_PROFILE_LO: usize = 17;
    pub(crate) const POLICY_EPOCH: usize = 19;
    pub(crate) const LIFECYCLE_LO: usize = 20;
    pub(crate) const SEMANTIC_LO: usize = 22;
    pub(crate) const CANDIDATE_LO: usize = 24;
    pub(crate) const COMMIT_CERTIFICATE_LO: usize = 26;
    pub(crate) const TRANSITION_NULLIFIER_LO: usize = 28;
    pub(crate) const REQUEST_LO: usize = 30;
    pub(crate) const RECEIVER_BINDING_LO: usize = 32;
    pub(crate) const CIPHERTEXT_LO: usize = 34;
    pub(crate) const AMOUNT: usize = 36;
    pub(crate) const OUTPUT_BINDING_LO: usize = 37;
    pub(crate) const EQ_DEFERRED_AUDIT_LO: usize = 39;
    pub(crate) const EP_DEFERRED_AUDIT_LO: usize = 41;
    pub(crate) const EQ_PROTOCOL_LO: usize = 43;
    pub(crate) const EP_PROTOCOL_LO: usize = 45;
    pub(crate) const HISTORY_START: usize = 47;
}

/// Unlinkable public values shared by both terminal-authorization parities.
///
/// Network and asset digests are normalized solely so the circuit can bind the private aggregate
/// state to the typed lifecycle. They reveal no state head, lane, credential, epoch, key,
/// sequence, journal, or replay-root value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaTerminalAuthorizationPublicInputsV1 {
    pub(crate) operation: KagemushaOperationV1,
    pub(crate) protocol_version: u16,
    pub(crate) suite_id: DigestV1,
    pub(crate) vk_digest: DigestV1,
    pub(crate) release_id: DigestV1,
    pub(crate) network_id: DigestV1,
    pub(crate) asset_id: DigestV1,
    pub(crate) asset_incarnation: AxtAssetIncarnationV1,
    pub(crate) asset_scale: u32,
    pub(crate) liability_pool_id: DigestV1,
    pub(crate) hardware_profile_id: DigestV1,
    pub(crate) policy_epoch: u64,
    pub(crate) lifecycle_binding_digest: DigestV1,
    pub(crate) semantic_digest: DigestV1,
    pub(crate) candidate_envelope_digest: DigestV1,
    pub(crate) commit_certificate_digest: DigestV1,
    pub(crate) transition_nullifier: DigestV1,
    pub(crate) request_digest: DigestV1,
    pub(crate) receiver_binding_digest: DigestV1,
    pub(crate) ciphertext_commitment: DigestV1,
    pub(crate) amount: u128,
    /// Operation-specific terminal output binding. For a send this commits the receiver credit,
    /// recipient lane, request, ciphertext, and amount; for a redemption this is the
    /// terminal redemption commitment.
    pub(crate) terminal_output_binding: DigestV1,
    pub(crate) eq_deferred_audit: DigestV1,
    pub(crate) ep_deferred_audit: DigestV1,
    pub(crate) eq_protocol_digest: DigestV1,
    pub(crate) ep_protocol_digest: DigestV1,
}

impl KagemushaTerminalAuthorizationPublicInputsV1 {
    /// Construct the normalized circuit projection from a validated lifecycle.
    pub(crate) fn from_lifecycle(
        lifecycle: &KagemushaLifecycleBindingV1,
        semantic_digest: DigestV1,
        candidate_envelope_digest: DigestV1,
        commit_certificate_digest: DigestV1,
        transition_nullifier: DigestV1,
        request_digest: DigestV1,
        receiver_binding_digest: DigestV1,
        ciphertext_commitment: DigestV1,
        amount: u128,
        terminal_output_binding: DigestV1,
        eq_deferred_audit: DigestV1,
        ep_deferred_audit: DigestV1,
        eq_protocol_digest: DigestV1,
        ep_protocol_digest: DigestV1,
    ) -> Result<Self, String> {
        lifecycle.validate().map_err(|error| error.to_string())?;
        let operation = operation_from_wire_v1(lifecycle.operation_kind);
        let value = Self {
            operation,
            protocol_version: lifecycle.protocol_version,
            suite_id: lifecycle.suite_id,
            vk_digest: lifecycle.vk_digest,
            release_id: lifecycle.release_id,
            network_id: *lifecycle.network_id.as_bytes(),
            asset_id: kagemusha_asset_identity_digest_v1(&lifecycle.asset)
                .map_err(|error| error.to_string())?,
            asset_incarnation: lifecycle.asset_incarnation,
            asset_scale: lifecycle.scale,
            liability_pool_id: lifecycle.liability_pool_id,
            hardware_profile_id: lifecycle.hardware_profile_id,
            policy_epoch: lifecycle.policy_epoch,
            lifecycle_binding_digest: lifecycle
                .canonical_digest()
                .map_err(|error| error.to_string())?,
            semantic_digest,
            candidate_envelope_digest,
            commit_certificate_digest,
            transition_nullifier,
            request_digest,
            receiver_binding_digest,
            ciphertext_commitment,
            amount,
            terminal_output_binding,
            eq_deferred_audit,
            ep_deferred_audit,
            eq_protocol_digest,
            ep_protocol_digest,
        };
        value.validate()?;
        Ok(value)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.operation,
            KagemushaOperationV1::SendSplit | KagemushaOperationV1::RedeemSplit
        ) || self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
            || self.asset_incarnation.validate().is_err()
            || self.policy_epoch == 0
            || self.amount == 0
        {
            return Err(
                "terminal authorization requires a positive postcommit send/redemption".to_owned(),
            );
        }
        for (name, digest) in [
            ("suite", self.suite_id),
            ("verifier key", self.vk_digest),
            ("release", self.release_id),
            ("network", self.network_id),
            ("asset", self.asset_id),
            ("liability pool", self.liability_pool_id),
            ("hardware profile", self.hardware_profile_id),
            ("lifecycle", self.lifecycle_binding_digest),
            ("semantic", self.semantic_digest),
            ("candidate envelope", self.candidate_envelope_digest),
            ("commit certificate", self.commit_certificate_digest),
            ("transition nullifier", self.transition_nullifier),
            ("terminal output", self.terminal_output_binding),
            ("Eq deferred audit", self.eq_deferred_audit),
            ("Ep deferred audit", self.ep_deferred_audit),
            ("Eq protocol", self.eq_protocol_digest),
            ("Ep protocol", self.ep_protocol_digest),
        ] {
            if digest == [0; 32] {
                return Err(format!(
                    "Kagemusha terminal authorization {name} digest is zero"
                ));
            }
        }
        if self.candidate_envelope_digest == self.commit_certificate_digest
            || self.eq_deferred_audit == self.ep_deferred_audit
            || self.eq_protocol_digest == self.ep_protocol_digest
            || decode::<halo2_proofs::halo2curves::pasta::Fp>(self.eq_protocol_digest).is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fq>(self.ep_protocol_digest).is_none()
        {
            return Err("terminal authorization digest roles are noncanonical".to_owned());
        }
        let send = self.operation == KagemushaOperationV1::SendSplit;
        if [
            self.request_digest,
            self.receiver_binding_digest,
            self.ciphertext_commitment,
        ]
        .into_iter()
        .any(|digest| (digest != [0; 32]) != send)
        {
            return Err(
                "terminal authorization has noncanonical operation-specific bindings".to_owned(),
            );
        }
        Ok(())
    }

    #[cfg(feature = "zk-halo2-ipa")]
    pub(crate) fn public_prefix<F: crate::zk::kagemusha_v1_poseidon::KagemushaPoseidonFieldV1>(
        &self,
    ) -> Result<Vec<F>, String> {
        use crate::zk::kagemusha_v1_poseidon::{digest_limbs, from_u128};

        self.validate()?;
        let incarnation = digest_limbs::<F>(*self.asset_incarnation.as_bytes());
        let mut output = vec![
            F::from(operation_tag_v1(self.operation)),
            F::from(u64::from(self.protocol_version)),
        ];
        for digest in [
            self.suite_id,
            self.vk_digest,
            self.release_id,
            self.network_id,
            self.asset_id,
        ] {
            output.extend(digest_limbs::<F>(digest));
        }
        output.extend(incarnation);
        output.push(F::from(u64::from(self.asset_scale)));
        for digest in [self.liability_pool_id, self.hardware_profile_id] {
            output.extend(digest_limbs::<F>(digest));
        }
        output.push(F::from(self.policy_epoch));
        for digest in [
            self.lifecycle_binding_digest,
            self.semantic_digest,
            self.candidate_envelope_digest,
            self.commit_certificate_digest,
            self.transition_nullifier,
            self.request_digest,
            self.receiver_binding_digest,
            self.ciphertext_commitment,
        ] {
            output.extend(digest_limbs::<F>(digest));
        }
        output.push(from_u128(self.amount));
        for digest in [
            self.terminal_output_binding,
            self.eq_deferred_audit,
            self.ep_deferred_audit,
            self.eq_protocol_digest,
            self.ep_protocol_digest,
        ] {
            output.extend(digest_limbs::<F>(digest));
        }
        if output.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1 {
            return Err("terminal authorization public prefix has wrong fixed shape".to_owned());
        }
        Ok(output)
    }
}

/// Complete private values checked by terminal authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaCommitEvidenceOpeningV1 {
    /// Fresh hiding opening owned by the committing hardware.
    pub(crate) opening: DigestV1,
    /// Trusted commit time, or zero for monotonic-lease evidence.
    pub(crate) trusted_commit_time_ms: u64,
    /// Unique private lease identity, or zero for trusted-time evidence.
    pub(crate) lease_id: DigestV1,
    /// Inclusive lease authorization boundary, or zero for trusted-time evidence.
    pub(crate) lease_valid_from_ms: u64,
    /// Exclusive lease authorization boundary, or zero for trusted-time evidence.
    pub(crate) lease_expires_at_ms: u64,
}

impl KagemushaCommitEvidenceOpeningV1 {
    fn kind(self) -> u8 {
        if self.trusted_commit_time_ms == 0 {
            1
        } else {
            0
        }
    }

    fn validate(self) -> Result<(), String> {
        if self.opening == [0; 32] {
            return Err("commit evidence opening is zero".to_owned());
        }
        match self.kind() {
            0 if self.lease_id == [0; 32]
                && self.lease_valid_from_ms == 0
                && self.lease_expires_at_ms == 0 =>
            {
                Ok(())
            }
            1 if self.lease_id != [0; 32]
                && self.lease_valid_from_ms > 0
                && self.lease_valid_from_ms < self.lease_expires_at_ms =>
            {
                Ok(())
            }
            _ => Err("commit evidence opening has noncanonical inactive fields".to_owned()),
        }
    }
}

/// Complete private values checked by terminal authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaTerminalAuthorizationPrivateTransitionV1 {
    pub(crate) lifecycle: KagemushaLifecycleBindingV1,
    pub(crate) predecessor: KagemushaStateV1,
    pub(crate) successor: KagemushaStateV1,
    pub(crate) outbox_reservation: KagemushaOutboxReservationV1,
    pub(crate) commit_certificate: KagemushaCommitCertificateV1,
    /// Private opening of the opaque public time-or-lease evidence commitment.
    pub(crate) commit_evidence_opening: KagemushaCommitEvidenceOpeningV1,
    /// Hardware-only one-use transition authorization consumed by this exact predecessor.
    pub(crate) one_use_hardware_authorization: DigestV1,
    /// Proof-independent terminal body digest fixed before proving; never the final envelope hash.
    pub(crate) terminal_payload_digest: DigestV1,
    /// Exact receiver opening for SendSplit; absent for redemption.
    pub(crate) send: Option<KagemushaTerminalSendPrivateV1>,
    pub(crate) journal_revision_before: u128,
    pub(crate) journal_revision_after: u128,
    pub(crate) authorization_counter_before: u128,
    pub(crate) authorization_counter_after: u128,
    pub(crate) hardware_profile: KagemushaHardwareProfileV1,
    pub(crate) hardware_credential: KagemushaHardwareCredentialV1,
}

/// Exact private receiver context authenticated by a postcommit send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KagemushaTerminalSendPrivateV1 {
    pub(crate) request: KagemushaPaymentRequestV1,
    pub(crate) output: KagemushaPaymentOutputV1,
    /// Digest of the actual encrypted bytes, distinct from the amount/opening commitment.
    pub(crate) encrypted_credit_digest: DigestV1,
}

impl KagemushaTerminalAuthorizationPrivateTransitionV1 {
    pub(crate) fn validate_against(
        &self,
        public: &KagemushaTerminalAuthorizationPublicInputsV1,
    ) -> Result<(), String> {
        public.validate()?;
        self.lifecycle
            .validate()
            .map_err(|error| error.to_string())?;
        self.predecessor
            .validate()
            .map_err(|error| format!("invalid private predecessor: {error}"))?;
        self.successor
            .validate()
            .map_err(|error| format!("invalid private successor: {error}"))?;
        self.hardware_credential
            .validate_against_profile(&self.hardware_profile)
            .map_err(|error| {
                format!("invalid terminal-authorization hardware credential: {error}")
            })?;
        let lifecycle_digest = self
            .lifecycle
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let reservation_commitment = self.outbox_reservation;
        let reservation_commitment =
            canonical_outbox_reservation_commitment_v1(reservation_commitment)?;
        let certificate_digest = canonical_commit_certificate_digest_v1(&self.commit_certificate)?;
        let evidence_commitment = canonical_commit_evidence_commitment_v1(
            self.commit_evidence_opening,
            self.authorization_counter_before,
            self.authorization_counter_after,
        )?;
        if lifecycle_digest != public.lifecycle_binding_digest
            || operation_from_wire_v1(self.lifecycle.operation_kind) != public.operation
            || self.lifecycle.protocol_version != public.protocol_version
            || self.lifecycle.suite_id != public.suite_id
            || self.lifecycle.vk_digest != public.vk_digest
            || self.lifecycle.release_id != public.release_id
            || self.lifecycle.network_id.as_bytes() != &public.network_id
            || kagemusha_asset_identity_digest_v1(&self.lifecycle.asset)
                .map_err(|error| error.to_string())?
                != public.asset_id
            || self.lifecycle.asset_incarnation != public.asset_incarnation
            || self.lifecycle.scale != public.asset_scale
            || self.lifecycle.liability_pool_id != public.liability_pool_id
            || self.lifecycle.hardware_profile_id != public.hardware_profile_id
            || self.lifecycle.policy_epoch != public.policy_epoch
            || self.one_use_hardware_authorization == [0; 32]
            || self.terminal_payload_digest == [0; 32]
            || self.commit_certificate.candidate_envelope_digest != public.candidate_envelope_digest
            || self.commit_certificate.lifecycle_binding_digest != lifecycle_digest
            || self.commit_certificate.transition_nullifier != public.transition_nullifier
            || self.commit_certificate.outbox_reservation_commitment != reservation_commitment
            || self.commit_certificate.hardware_profile_id != public.hardware_profile_id
            || self.commit_certificate.policy_epoch != public.policy_epoch
            || self.commit_certificate.hardware_terminal_commitment == [0; 32]
            || certificate_digest != public.commit_certificate_digest
            || evidence_commitment
                != evidence_commitment_v1(self.commit_certificate.commit_evidence)
        {
            return Err(
                "terminal authorization lifecycle/candidate/certificate binding mismatch"
                    .to_owned(),
            );
        }
        if self.authorization_counter_after
            != self
                .authorization_counter_before
                .checked_add(1)
                .ok_or_else(|| "hardware authorization counter overflow".to_owned())?
            || self.journal_revision_after
                != self
                    .journal_revision_before
                    .checked_add(1)
                    .ok_or_else(|| "hardware journal revision overflow".to_owned())?
            || self.successor.logical_sequence
                != self
                    .predecessor
                    .logical_sequence
                    .checked_add(1)
                    .ok_or_else(|| "aggregate sequence overflow".to_owned())?
        {
            return Err("terminal authorization terminal transition is not exact-next".to_owned());
        }
        match (public.operation, self.send.as_ref()) {
            (KagemushaOperationV1::SendSplit, Some(send)) => send.validate_against(public, self)?,
            (KagemushaOperationV1::RedeemSplit, None) => {}
            _ => return Err("terminal send opening does not match its operation".to_owned()),
        }
        let prepared_authorization = canonical_prepared_one_use_authorization_digest_v1(
            public.operation,
            self.one_use_hardware_authorization,
            &self.predecessor,
            self.journal_revision_before,
            self.authorization_counter_before,
        );
        let expected_prepared_transition = canonical_prepared_transition_binding_digest_v1(
            public.lifecycle_binding_digest,
            public.request_digest,
            self.send
                .as_ref()
                .map(|send| send.output.sender_before_commitment)
                .unwrap_or([0; 32]),
            self.send
                .as_ref()
                .map(|send| send.output.sender_after_commitment)
                .unwrap_or([0; 32]),
            public.amount,
            reservation_commitment,
            prepared_authorization,
        );
        if expected_prepared_transition == [0; 32] {
            return Err("terminal authorization prepared-transition binding is zero".to_owned());
        }
        if operation_from_wire_v1(self.outbox_reservation.operation_kind) != public.operation
            || self.outbox_reservation.issued_at_ms >= self.outbox_reservation.expires_at_ms
        {
            return Err("terminal authorization outbox reservation mismatch".to_owned());
        }
        let valid_deadline = match self.commit_evidence_opening.kind() {
            0 => {
                let committed = self.commit_evidence_opening.trusted_commit_time_ms;
                committed >= self.outbox_reservation.issued_at_ms
                    && committed < self.outbox_reservation.expires_at_ms
            }
            1 => {
                let lease_start = self.commit_evidence_opening.lease_valid_from_ms;
                let lease_end = self.commit_evidence_opening.lease_expires_at_ms;
                lease_start >= self.outbox_reservation.issued_at_ms
                    && lease_end <= self.outbox_reservation.expires_at_ms
            }
            _ => false,
        };
        if !valid_deadline {
            return Err("commit evidence did not authorize an in-window commit".to_owned());
        }
        if self.predecessor.protocol_version != self.successor.protocol_version
            || self.predecessor.suite_id != self.successor.suite_id
            || self.predecessor.vk_digest != self.successor.vk_digest
            || self.predecessor.release_id != self.successor.release_id
            || self.predecessor.asset_incarnation != self.successor.asset_incarnation
            || self.predecessor.liability_pool_id != self.successor.liability_pool_id
            || self.predecessor.hardware_profile_id != self.successor.hardware_profile_id
            || self.predecessor.policy_epoch != self.successor.policy_epoch
            || self.predecessor.lane != self.successor.lane
            || self.predecessor.hardware_epoch != self.successor.hardware_epoch
            || self.predecessor.device_policy_binding != self.successor.device_policy_binding
            || self.predecessor.consumed_credit_root != self.successor.consumed_credit_root
            || self.predecessor.state_nonce_commitment == self.successor.state_nonce_commitment
            || self.predecessor.state_commitment == self.successor.state_commitment
            || self.predecessor.protocol_version != public.protocol_version
            || self.predecessor.suite_id != public.suite_id
            || self.predecessor.vk_digest != public.vk_digest
            || self.predecessor.release_id != public.release_id
            || self.predecessor.asset_incarnation != public.asset_incarnation
            || self.predecessor.liability_pool_id != public.liability_pool_id
            || self.predecessor.hardware_profile_id != public.hardware_profile_id
            || self.predecessor.policy_epoch != public.policy_epoch
            || self.predecessor.lane.normalized_network_id() != public.network_id
            || self
                .predecessor
                .lane
                .normalized_asset_id()
                .map_err(|error| error.to_string())?
                != public.asset_id
            || self.predecessor.lane.scale != public.asset_scale
        {
            return Err("terminal authorization private aggregate context mismatch".to_owned());
        }
        let expected_predecessor_balance = self
            .successor
            .balance
            .checked_add(public.amount)
            .ok_or_else(|| "outbound balance overflow".to_owned())?;
        if expected_predecessor_balance != self.predecessor.balance {
            return Err("terminal authorization does not conserve outbound value".to_owned());
        }
        if self.hardware_credential.hardware_profile_id != public.hardware_profile_id
            || self.hardware_credential.policy_epoch != public.policy_epoch
            || self.hardware_credential.suite_id != public.suite_id
            || self.hardware_credential.network_id.as_bytes() != &public.network_id
            || self.hardware_credential.hardware_epoch_id
                != self.predecessor.hardware_epoch.epoch_id
            || u128::from(self.hardware_credential.hardware_epoch_generation)
                != self.predecessor.hardware_epoch.generation
            || self.hardware_credential.device_key_reference
                != self.predecessor.device_policy_binding.device_key_reference
        {
            return Err(
                "terminal authorization credential/private-state binding mismatch".to_owned(),
            );
        }
        Ok(())
    }
}

impl KagemushaTerminalSendPrivateV1 {
    fn validate_against(
        &self,
        public: &KagemushaTerminalAuthorizationPublicInputsV1,
        terminal: &KagemushaTerminalAuthorizationPrivateTransitionV1,
    ) -> Result<(), String> {
        let request_digest = self
            .request
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let output_digest = self
            .output
            .canonical_digest_against(&self.request)
            .map_err(|error| error.to_string())?;
        let body = kagemusha_payment_body_digest_from_digests_v1(
            output_digest,
            self.encrypted_credit_digest,
        );
        let state_pair_digest = super::canonical_sender_state_pair_digest_v1(
            self.output.sender_before_commitment,
            self.output.sender_after_commitment,
        );
        let expected_output = canonical_terminal_send_output_binding_v1(
            self.output.credit_id,
            self.request.recipient_encryption_key,
            self.request.hardware_credential.lane_commitment,
            kagemusha_prepared_transfer_digest_v1(
                &self.request,
                self.output.sender_before_commitment,
                self.output.sender_after_commitment,
                self.output.transition_nullifier,
                self.output.ciphertext_commitment,
            )
            .map_err(|error| error.to_string())?,
            output_digest,
            super::canonical_incoming_payment_claims_binding_v1([
                request_digest,
                self.request.hardware_credential.credential_id,
                state_pair_digest,
                output_digest,
                self.encrypted_credit_digest,
                public.candidate_envelope_digest,
                public.commit_certificate_digest,
            ]),
        );
        let evidence_time_valid = match terminal.commit_evidence_opening.kind() {
            0 => {
                self.output.committed_at_ms
                    == terminal.commit_evidence_opening.trusted_commit_time_ms
            }
            1 => {
                self.output.committed_at_ms >= terminal.commit_evidence_opening.lease_valid_from_ms
                    && self.output.committed_at_ms
                        < terminal.commit_evidence_opening.lease_expires_at_ms
            }
            _ => false,
        };
        if self.encrypted_credit_digest == [0; 32]
            || self.request.amount != public.amount
            || self.output.amount != public.amount
            || self.output.sender_before_commitment != terminal.predecessor.state_commitment
            || self.output.sender_after_commitment != terminal.successor.state_commitment
            || self.output.transition_nullifier != public.transition_nullifier
            || self.output.ciphertext_commitment != public.ciphertext_commitment
            || self.output.commit_evidence != terminal.commit_certificate.commit_evidence
            || !evidence_time_valid
            || request_digest != public.request_digest
            || self.request.hardware_credential.credential_id != public.receiver_binding_digest
            || expected_output != public.terminal_output_binding
            || body != terminal.terminal_payload_digest
            || body != public.semantic_digest
            || self.request.release_id != public.release_id
            || self.request.network_id.as_bytes() != &public.network_id
            || kagemusha_asset_identity_digest_v1(&self.request.asset)
                .map_err(|error| error.to_string())?
                != public.asset_id
            || self.request.asset_incarnation != public.asset_incarnation
            || self.request.scale != public.asset_scale
            || self.request.liability_pool_id != public.liability_pool_id
        {
            return Err(
                "postcommit send opening does not match the exact public output".to_owned(),
            );
        }
        for (issued, expires) in [(self.request.issued_at_ms, self.request.expires_at_ms)] {
            let authorized = match terminal.commit_evidence_opening.kind() {
                0 => {
                    terminal.commit_evidence_opening.trusted_commit_time_ms >= issued
                        && terminal.commit_evidence_opening.trusted_commit_time_ms < expires
                }
                1 => {
                    terminal.commit_evidence_opening.lease_valid_from_ms >= issued
                        && terminal.commit_evidence_opening.lease_expires_at_ms <= expires
                }
                _ => false,
            };
            if !authorized {
                return Err("send committed outside its request authorization".to_owned());
            }
        }
        Ok(())
    }
}

fn hash_fixed_v1(domain: &[u8], chunks: &[&[u8]]) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for chunk in chunks {
        hasher.update(chunk);
    }
    hasher.finalize().into()
}

pub(super) fn canonical_predecessor_conflict_nullifier_v1(
    prepared_authorization: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1,
        &[&1_u16.to_le_bytes(), &prepared_authorization],
    )
}

/// Return the terminal send-output binding consumed by a receiver fold.
///
/// The sender authorization opens the request credential ID to its recipient lane and binds this
/// value to the recursively verified state-candidate output, including the recipient encryption
/// key bound directly by the request.
/// A receiver recomputes it from the accepted payment and its own lane before admitting value,
/// preventing one terminal proof from being replayed under a substituted credit identity or lane.
/// The prepared-transfer input is its 32-byte digest, not its 210-byte canonical transcript.
/// The final claims digest authenticates exact staged metadata without including proof bytes.
pub(crate) fn canonical_terminal_send_output_binding_v1(
    credit_id: DigestV1,
    recipient_encryption_key: DigestV1,
    recipient_lane_id: DigestV1,
    prepared_transfer_digest: DigestV1,
    payment_output_digest: DigestV1,
    incoming_claims_digest: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &credit_id,
            &recipient_encryption_key,
            &recipient_lane_id,
            &prepared_transfer_digest,
            &payment_output_digest,
            &incoming_claims_digest,
        ],
    )
}

fn hash_canonical_bytes_v1(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    hash_fixed_v1(
        domain,
        &[
            &[0],
            &u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes(),
            bytes,
        ],
    )
}

fn canonical_outbox_reservation_bytes_v1(reservation: KagemushaOutboxReservationV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(OUTBOX_RESERVATION_CANONICAL_BYTES_V1);
    bytes.extend_from_slice(&reservation.reservation_id);
    bytes.extend_from_slice(
        &(operation_tag_v1(operation_from_wire_v1(reservation.operation_kind)) as u32)
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&reservation.reserved_outbox_bytes.to_le_bytes());
    bytes.extend_from_slice(&reservation.issued_at_ms.to_le_bytes());
    bytes.extend_from_slice(&reservation.expires_at_ms.to_le_bytes());
    bytes
}

/// Return the exact fixed-layout outbox reservation commitment constrained by terminal authorization.
pub(crate) fn canonical_outbox_reservation_commitment_v1(
    reservation: KagemushaOutboxReservationV1,
) -> Result<DigestV1, String> {
    let bytes = canonical_outbox_reservation_bytes_v1(reservation);
    if bytes.len() != OUTBOX_RESERVATION_CANONICAL_BYTES_V1 {
        return Err("outbox-reservation canonical layout drift".to_owned());
    }
    let digest = hash_canonical_bytes_v1(OUTBOX_RESERVATION_COMMITMENT_DOMAIN_V1, &bytes);
    if digest
        != reservation
            .canonical_commitment()
            .map_err(|error| error.to_string())?
    {
        return Err("outbox-reservation commitment drift".to_owned());
    }
    Ok(digest)
}

fn evidence_commitment_v1(evidence: KagemushaCommitEvidenceV1) -> DigestV1 {
    match evidence {
        KagemushaCommitEvidenceV1::TrustedTime(value) => value.time_evidence_commitment,
        KagemushaCommitEvidenceV1::MonotonicLease(value) => value.lease_evidence_commitment,
    }
}

fn evidence_tag_v1(evidence: KagemushaCommitEvidenceV1) -> u8 {
    match evidence {
        KagemushaCommitEvidenceV1::TrustedTime(_) => 0,
        KagemushaCommitEvidenceV1::MonotonicLease(_) => 1,
    }
}

fn canonical_evidence_bytes_v1(evidence: KagemushaCommitEvidenceV1) -> [u8; 36] {
    let mut bytes = [0_u8; 36];
    bytes[..4].copy_from_slice(&u32::from(evidence_tag_v1(evidence)).to_le_bytes());
    bytes[4..].copy_from_slice(&evidence_commitment_v1(evidence));
    bytes
}

/// Recompute the opaque public commit-evidence commitment from its complete private opening.
pub(crate) fn canonical_commit_evidence_commitment_v1(
    opening: KagemushaCommitEvidenceOpeningV1,
    authorization_counter_before: u128,
    authorization_counter_after: u128,
) -> Result<DigestV1, String> {
    opening.validate()?;
    if authorization_counter_after
        != authorization_counter_before
            .checked_add(1)
            .ok_or_else(|| "hardware authorization counter overflow".to_owned())?
    {
        return Err("commit evidence authorization counter is not exact-next".to_owned());
    }
    let version = 1_u16.to_le_bytes();
    let kind = [opening.kind()];
    Ok(hash_fixed_v1(
        COMMIT_EVIDENCE_OPENING_DOMAIN_V1,
        &[
            &version,
            &kind,
            &opening.opening,
            &opening.trusted_commit_time_ms.to_le_bytes(),
            &opening.lease_id,
            &opening.lease_valid_from_ms.to_le_bytes(),
            &opening.lease_expires_at_ms.to_le_bytes(),
            &authorization_counter_before.to_le_bytes(),
            &authorization_counter_after.to_le_bytes(),
        ],
    ))
}

fn canonical_commit_certificate_id_bytes_v1(certificate: &KagemushaCommitCertificateV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(COMMIT_CERTIFICATE_ID_CANONICAL_BYTES_V1);
    bytes.extend_from_slice(&certificate.version.to_le_bytes());
    bytes.extend_from_slice(&certificate.candidate_envelope_digest);
    bytes.extend_from_slice(&certificate.lifecycle_binding_digest);
    bytes.extend_from_slice(&certificate.transition_nullifier);
    bytes.extend_from_slice(&certificate.outbox_reservation_commitment);
    bytes.extend_from_slice(&canonical_evidence_bytes_v1(certificate.commit_evidence));
    bytes.extend_from_slice(&certificate.hardware_profile_id);
    bytes.extend_from_slice(&certificate.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(&certificate.hardware_terminal_commitment);
    bytes
}

fn canonical_commit_certificate_bytes_v1(certificate: &KagemushaCommitCertificateV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(COMMIT_CERTIFICATE_CANONICAL_BYTES_V1);
    bytes.extend_from_slice(&certificate.version.to_le_bytes());
    bytes.extend_from_slice(&certificate.certificate_id);
    bytes.extend_from_slice(&certificate.candidate_envelope_digest);
    bytes.extend_from_slice(&certificate.lifecycle_binding_digest);
    bytes.extend_from_slice(&certificate.transition_nullifier);
    bytes.extend_from_slice(&certificate.outbox_reservation_commitment);
    bytes.extend_from_slice(&canonical_evidence_bytes_v1(certificate.commit_evidence));
    bytes.extend_from_slice(&certificate.hardware_profile_id);
    bytes.extend_from_slice(&certificate.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(&certificate.hardware_terminal_commitment);
    bytes
}

/// Return the exact canonical commit-certificate digest constrained by terminal authorization.
pub(crate) fn canonical_commit_certificate_digest_v1(
    certificate: &KagemushaCommitCertificateV1,
) -> Result<DigestV1, String> {
    let id_bytes = canonical_commit_certificate_id_bytes_v1(certificate);
    let expected_id = hash_canonical_bytes_v1(COMMIT_CERTIFICATE_ID_DOMAIN_V1, &id_bytes);
    if id_bytes.len() != COMMIT_CERTIFICATE_ID_CANONICAL_BYTES_V1
        || expected_id != certificate.certificate_id
        || expected_id
            != certificate
                .expected_certificate_id()
                .map_err(|error| error.to_string())?
    {
        return Err("commit certificate identity/layout mismatch".to_owned());
    }
    let bytes = canonical_commit_certificate_bytes_v1(certificate);
    if bytes.len() != COMMIT_CERTIFICATE_CANONICAL_BYTES_V1 {
        return Err("commit-certificate canonical layout drift".to_owned());
    }
    Ok(hash_canonical_bytes_v1(
        COMMIT_CERTIFICATE_DIGEST_DOMAIN_V1,
        &bytes,
    ))
}

/// Bind the exact private predecessor to one consumed hardware authorization.
pub(crate) fn canonical_prepared_one_use_authorization_digest_v1(
    operation: KagemushaOperationV1,
    one_use_hardware_authorization: DigestV1,
    predecessor: &KagemushaStateV1,
    journal_revision_before: u128,
    authorization_counter_before: u128,
) -> DigestV1 {
    hash_fixed_v1(
        PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &[operation_tag_v1(operation) as u8],
            &one_use_hardware_authorization,
            &predecessor.state_commitment,
            &predecessor.state_nonce_commitment,
            &predecessor.lane.device_lane_id,
            &predecessor.hardware_epoch.epoch_id,
            &predecessor.device_policy_binding.device_key_reference,
            &predecessor.logical_sequence.to_le_bytes(),
            &journal_revision_before.to_le_bytes(),
            &authorization_counter_before.to_le_bytes(),
        ],
    )
}

/// Return the exact prepared-transition binding shared by State and terminal Guard proofs.
pub(crate) fn canonical_prepared_transition_binding_digest_v1(
    lifecycle_binding_digest: DigestV1,
    request_digest: DigestV1,
    sender_before_commitment: DigestV1,
    sender_after_commitment: DigestV1,
    amount: u128,
    reservation_commitment: DigestV1,
    prepared_authorization_digest: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        PREPARED_TRANSITION_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &lifecycle_binding_digest,
            &request_digest,
            &sender_before_commitment,
            &sender_after_commitment,
            &amount.to_le_bytes(),
            &reservation_commitment,
            &prepared_authorization_digest,
        ],
    )
}

/// Return the no-cycle terminal binding authenticated by the postcommit Guard proof.
pub(crate) fn canonical_terminal_commit_binding_digest_v1(
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
    private: &KagemushaTerminalAuthorizationPrivateTransitionV1,
    prepared_transition_binding_digest: DigestV1,
    sender_authorization_digest: DigestV1,
    transition_intent_digest: DigestV1,
    transition_effect_digest: DigestV1,
    recovery_record_digest: DigestV1,
    durable_inbox_effect_digest: DigestV1,
    durable_outbox_effect_digest: DigestV1,
) -> Result<DigestV1, String> {
    let reservation = private.outbox_reservation;
    let reservation_commitment = canonical_outbox_reservation_commitment_v1(reservation)?;
    let evidence_commitment = canonical_commit_evidence_commitment_v1(
        private.commit_evidence_opening,
        private.authorization_counter_before,
        private.authorization_counter_after,
    )?;
    Ok(hash_fixed_v1(
        TERMINAL_COMMIT_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &[operation_tag_v1(public.operation) as u8],
            &public.lifecycle_binding_digest,
            &prepared_transition_binding_digest,
            &public.candidate_envelope_digest,
            &public.commit_certificate_digest,
            &private.commit_certificate.certificate_id,
            &private.commit_certificate.hardware_terminal_commitment,
            &public.transition_nullifier,
            &reservation_commitment,
            &[evidence_tag_v1(private.commit_certificate.commit_evidence)],
            &evidence_commitment,
            &public.request_digest,
            &public.receiver_binding_digest,
            &public.amount.to_le_bytes(),
            &public.hardware_profile_id,
            &public.policy_epoch.to_le_bytes(),
            &private
                .send
                .as_ref()
                .map_or(0, |send| send.request.issued_at_ms)
                .to_le_bytes(),
            &private
                .send
                .as_ref()
                .map_or(0, |send| send.request.expires_at_ms)
                .to_le_bytes(),
            &private
                .send
                .as_ref()
                .map_or([0; 32], |send| send.output.sender_before_commitment),
            &private
                .send
                .as_ref()
                .map_or([0; 32], |send| send.output.sender_after_commitment),
            &reservation.issued_at_ms.to_le_bytes(),
            &reservation.expires_at_ms.to_le_bytes(),
            &transition_intent_digest,
            &transition_effect_digest,
            &recovery_record_digest,
            &durable_inbox_effect_digest,
            &durable_outbox_effect_digest,
            &private.terminal_payload_digest,
            &sender_authorization_digest,
        ],
    ))
}

const fn operation_tag_v1(operation: KagemushaOperationV1) -> u64 {
    match operation {
        KagemushaOperationV1::Bootstrap => 0,
        KagemushaOperationV1::MintFold => 1,
        KagemushaOperationV1::SendSplit => 2,
        KagemushaOperationV1::ReceiveFold => 3,
        KagemushaOperationV1::RedeemSplit => 4,
        KagemushaOperationV1::Rotate => 5,
    }
}

const fn operation_from_wire_v1(operation: KagemushaOperationKindV1) -> KagemushaOperationV1 {
    match operation {
        KagemushaOperationKindV1::Bootstrap => KagemushaOperationV1::Bootstrap,
        KagemushaOperationKindV1::MintFold => KagemushaOperationV1::MintFold,
        KagemushaOperationKindV1::SendSplit => KagemushaOperationV1::SendSplit,
        KagemushaOperationKindV1::ReceiveFold => KagemushaOperationV1::ReceiveFold,
        KagemushaOperationKindV1::RedeemSplit => KagemushaOperationV1::RedeemSplit,
        KagemushaOperationKindV1::Rotate => KagemushaOperationV1::Rotate,
    }
}

#[cfg(feature = "zk-halo2-ipa")]
const MINIMUM_UNUSABLE_ROWS: usize = 9;
#[cfg(feature = "zk-halo2-ipa")]
const CANDIDATE_EQUATION_TAG_V1: u32 = 1;
#[cfg(feature = "zk-halo2-ipa")]
const TERMINAL_GUARD_EQUATION_TAG_V1: u32 = 2;
const TERMINAL_AUTHORIZATION_EQUATION_TAG_V1: u32 = 3;
const CANDIDATE_BINDING_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-authorization-candidate\0";

/// Hash the parity-invariant candidate semantic cells bound by terminal authorization.
///
/// The 85-cell state ABI contains two redundant parity-native state components at indices 30 and
/// 31. Their complete paired Eq/Ep values already occur as four canonical digest-limb pairs at
/// indices 22 through 29. This transcript writes fixed zero markers for the two redundant cells
/// so Eq and Ep derive one candidate identity, while retaining every one of the other 83 semantic
/// cells. Every retained cell must have the ABI's canonical unsigned-128 representation.
/// This native helper accepts only that semantic column; recursive proof columns additionally
/// contain history-accumulator limbs, whose shape and binding are checked by the recursive circuit.
pub(crate) fn canonical_terminal_authorization_candidate_digest_v1<F: BigPrimeField>(
    candidate_instances: &[Vec<F>],
) -> Result<DigestV1, String> {
    if candidate_instances.len() != 1
        || candidate_instances[0].len() != state_relation::PUBLIC_INSTANCE_COUNT
    {
        return Err("terminal authorization candidate has wrong fixed public shape".to_owned());
    }
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_BINDING_DOMAIN_V1);
    for (index, value) in candidate_instances[0][..state_relation::PUBLIC_INSTANCE_COUNT]
        .iter()
        .enumerate()
    {
        if matches!(
            index,
            state_relation::public_instance::PREDECESSOR_STATE
                | state_relation::public_instance::SUCCESSOR_STATE
        ) {
            hasher.update([0_u8; 16]);
            continue;
        }
        let bytes = fe_to_biguint(value).to_bytes_le();
        if bytes.len() > 16 {
            return Err("terminal authorization candidate field exceeds canonical u128".to_owned());
        }
        let mut limb = [0_u8; 16];
        limb[..bytes.len()].copy_from_slice(&bytes);
        hasher.update(limb);
    }
    Ok(hasher.finalize().into())
}

/// Derive the feature-independent candidate envelope identity used by persistence and hardware.
///
/// Both Pasta projections are reconstructed and required to produce the same normalized digest,
/// preventing a caller from choosing one parity's representation as the durable candidate name.
pub(crate) fn kagemusha_candidate_envelope_digest_v1(
    public_inputs: &KagemushaStateRelationPublicInputsV1,
) -> Result<DigestV1, String> {
    let eq = public_inputs.public_instances::<halo2_proofs::halo2curves::pasta::Fp>()?;
    let ep = public_inputs.public_instances::<halo2_proofs::halo2curves::pasta::Fq>()?;
    let eq_digest = canonical_terminal_authorization_candidate_digest_v1(&[eq])?;
    let ep_digest = canonical_terminal_authorization_candidate_digest_v1(&[ep])?;
    if eq_digest != ep_digest {
        return Err("normalized Eq/Ep candidate envelope digests differ".to_owned());
    }
    Ok(eq_digest)
}

/// Eq/Fp recursive inputs consumed by one terminal-authorization proof.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaTerminalAuthorizationEqWitnessV1<'a> {
    pub(crate) candidate_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) candidate_instances: &'a [Vec<Fp>],
    pub(crate) candidate_proof: &'a [u8],
    pub(crate) candidate_history: &'a KagemushaEqAccumulatorV1,
    pub(crate) candidate_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) terminal_guard_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) terminal_guard_instances: &'a [Vec<Fp>],
    pub(crate) terminal_guard_proof: &'a [u8],
    pub(crate) terminal_guard_history: &'a KagemushaEqAccumulatorV1,
    pub(crate) terminal_guard_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) merge_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) successor_history: &'a KagemushaEqAccumulatorV1,
}

/// Ep/Fq recursive inputs consumed by one terminal-authorization proof.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaTerminalAuthorizationEpWitnessV1<'a> {
    pub(crate) candidate_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) candidate_instances: &'a [Vec<Fq>],
    pub(crate) candidate_proof: &'a [u8],
    pub(crate) candidate_history: &'a KagemushaEpAccumulatorV1,
    pub(crate) candidate_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) terminal_guard_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) terminal_guard_instances: &'a [Vec<Fq>],
    pub(crate) terminal_guard_proof: &'a [u8],
    pub(crate) terminal_guard_history: &'a KagemushaEpAccumulatorV1,
    pub(crate) terminal_guard_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) merge_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) successor_history: &'a KagemushaEpAccumulatorV1,
}

/// Complete paired candidate and terminal-Guard witness.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaTerminalAuthorizationWitnessV1<'a> {
    pub(crate) public: KagemushaTerminalAuthorizationPublicInputsV1,
    pub(crate) private_transition: KagemushaTerminalAuthorizationPrivateTransitionV1,
    /// Complete private statement whose digest is recursively authenticated by terminal Guard.
    pub(crate) terminal_guard_relation: KagemushaGuardBundleRelationWitnessV1,
    /// Sorted, nonzero-prefix release-enabled profile IDs, padded with canonical zeroes.
    pub(crate) enabled_hardware_profiles:
        [DigestV1; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    pub(crate) eq: KagemushaTerminalAuthorizationEqWitnessV1<'a>,
    pub(crate) ep: KagemushaTerminalAuthorizationEpWitnessV1<'a>,
}

/// Eq/Fp internal terminal-authorization proof consumed by the transported commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaCommitWrapperEqWitnessV1<'a> {
    pub(crate) terminal_authorization_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) terminal_authorization_instances: &'a [Vec<Fp>],
    pub(crate) terminal_authorization_proof: &'a [u8],
    pub(crate) terminal_authorization_history: &'a KagemushaEqAccumulatorV1,
    pub(crate) terminal_authorization_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) successor_history: &'a KagemushaEqAccumulatorV1,
}

/// Ep/Fq internal terminal-authorization proof consumed by the transported commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaCommitWrapperEpWitnessV1<'a> {
    pub(crate) terminal_authorization_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) terminal_authorization_instances: &'a [Vec<Fq>],
    pub(crate) terminal_authorization_proof: &'a [u8],
    pub(crate) terminal_authorization_history: &'a KagemushaEpAccumulatorV1,
    pub(crate) terminal_authorization_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) successor_history: &'a KagemushaEpAccumulatorV1,
}

/// Complete one-proof recursive witness for the transported commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaCommitWrapperWitnessV1<'a> {
    pub(crate) public: KagemushaTerminalAuthorizationPublicInputsV1,
    pub(crate) eq: KagemushaCommitWrapperEqWitnessV1<'a>,
    pub(crate) ep: KagemushaCommitWrapperEpWitnessV1<'a>,
}

#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KagemushaTerminalRelationV1 {
    TerminalAuthorization,
    CommitWrapper,
}

#[cfg(feature = "zk-halo2-ipa")]
struct TerminalAuthorizationParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    private_transition: &'a KagemushaTerminalAuthorizationPrivateTransitionV1,
    terminal_guard_relation: &'a KagemushaGuardBundleRelationWitnessV1,
    enabled_hardware_profiles: &'a [DigestV1; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    candidate_protocol: &'a PlonkProtocol<C>,
    candidate_instances: &'a [Vec<C::ScalarExt>],
    candidate_proof: &'a [u8],
    candidate_history: &'a IpaAccumulator<C, NativeLoader>,
    candidate_history_fold_proof: &'a [u8],
    terminal_guard_protocol: &'a PlonkProtocol<C>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
    terminal_guard_instances: &'a [Vec<C::ScalarExt>],
    terminal_guard_proof: &'a [u8],
    terminal_guard_history: &'a IpaAccumulator<C, NativeLoader>,
    terminal_guard_history_fold_proof: &'a [u8],
    merge_fold_proof: &'a [u8],
    successor_history: &'a [u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Base, SHA-256, and reciprocal dense-MSM configuration for a terminal-authorization parity.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub(crate) struct KagemushaTerminalAuthorizationCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Base-only configuration for the transported commit wrapper relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub(crate) struct KagemushaCommitWrapperCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
}

/// Eq/Fp half of the final terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct KagemushaTerminalAuthorizationEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq half of the final terminal authorization.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct KagemushaTerminalAuthorizationEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

/// Eq/Fp half of the postcommit compact wrapper relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct KagemushaCommitWrapperEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
}

/// Ep/Fq half of the postcommit compact wrapper relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct KagemushaCommitWrapperEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
}

#[cfg(feature = "zk-halo2-ipa")]
macro_rules! impl_terminal_authorization_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaTerminalAuthorizationCircuitConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    sha_jobs: self.sha_jobs.unknown(),
                    dense_jobs: self.dense_jobs.unknown(),
                }
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                KagemushaTerminalAuthorizationCircuitConfigV1 {
                    base,
                    sha: PastaSha256ConfigV1::configure(meta),
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let usable_rows = (1_usize << self.builder.config_params.k) - MINIMUM_UNUSABLE_ROWS;
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )?;
                self.sha_jobs.synthesize(
                    &config.sha,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    usable_rows,
                )?;
                self.dense_jobs.synthesize(
                    &config.dense,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                    usable_rows,
                )
            }
        }
    };
}

#[cfg(feature = "zk-halo2-ipa")]
impl_terminal_authorization_circuit!(
    KagemushaTerminalAuthorizationEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq terminal authorization"
);
#[cfg(feature = "zk-halo2-ipa")]
impl_terminal_authorization_circuit!(
    KagemushaTerminalAuthorizationEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep terminal authorization"
);
#[cfg(feature = "zk-halo2-ipa")]
macro_rules! impl_commit_wrapper_circuit {
    ($circuit:ty, $field:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaCommitWrapperCircuitConfigV1<$field>;
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
                KagemushaCommitWrapperCircuitConfigV1 { base }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
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

#[cfg(feature = "zk-halo2-ipa")]
impl_commit_wrapper_circuit!(
    KagemushaCommitWrapperEqCircuitV1,
    Fp,
    "Kagemusha Eq commit wrapper"
);
#[cfg(feature = "zk-halo2-ipa")]
impl_commit_wrapper_circuit!(
    KagemushaCommitWrapperEpCircuitV1,
    Fq,
    "Kagemusha Ep commit wrapper"
);

fn validate_enabled_hardware_profiles_v1(
    profiles: &[DigestV1; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
) -> Result<(), String> {
    let mut previous = None;
    let mut padding = false;
    for profile in profiles {
        if *profile == [0; 32] {
            padding = true;
            continue;
        }
        if padding || previous.is_some_and(|value| value >= *profile) {
            return Err(
                "enabled hardware profiles must be a sorted distinct nonzero prefix".to_owned(),
            );
        }
        previous = Some(*profile);
    }
    if previous.is_none() {
        return Err("enabled hardware profile table is empty".to_owned());
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_terminal_guard_relation_v1(
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
    private: &KagemushaTerminalAuthorizationPrivateTransitionV1,
    relation: &KagemushaGuardBundleRelationWitnessV1,
) -> Result<(), String> {
    relation.validate()?;
    let guard = &relation.statement;
    let prepared_authorization = canonical_prepared_one_use_authorization_digest_v1(
        public.operation,
        private.one_use_hardware_authorization,
        &private.predecessor,
        private.journal_revision_before,
        private.authorization_counter_before,
    );
    if public.transition_nullifier
        != canonical_predecessor_conflict_nullifier_v1(prepared_authorization)
    {
        return Err(
            "terminal transition nullifier does not identify its exact predecessor".to_owned(),
        );
    }
    let prepared_transition = canonical_prepared_transition_binding_digest_v1(
        public.lifecycle_binding_digest,
        public.request_digest,
        private
            .send
            .as_ref()
            .map(|send| send.output.sender_before_commitment)
            .unwrap_or([0; 32]),
        private
            .send
            .as_ref()
            .map(|send| send.output.sender_after_commitment)
            .unwrap_or([0; 32]),
        public.amount,
        canonical_outbox_reservation_commitment_v1(private.outbox_reservation)?,
        prepared_authorization,
    );
    let sender_authorization = if private.send.is_some() {
        prepared_authorization
    } else {
        [0; 32]
    };
    let terminal = canonical_terminal_commit_binding_digest_v1(
        public,
        private,
        prepared_transition,
        sender_authorization,
        guard.transition_intent_digest,
        guard.transition_effect_digest,
        guard.recovery_record_digest,
        guard.durable_inbox_effect_digest,
        guard.durable_outbox_effect_digest,
    )?;
    if let Some(send) = &private.send
        && (guard.peer_credit_id != send.output.credit_id
            || guard.recipient_encryption_key_binding != send.request.recipient_encryption_key)
    {
        return Err(
            "terminal Guard does not bind the request's exact credit and recipient key".to_owned(),
        );
    }
    let predecessor = &private.predecessor;
    let successor = &private.successor;
    if guard.operation != public.operation
        || guard.protocol_version != public.protocol_version
        || guard.predecessor_suite_id != public.suite_id
        || guard.predecessor_vk_digest != public.vk_digest
        || guard.successor_suite_id != public.suite_id
        || guard.successor_vk_digest != public.vk_digest
        || guard.amount != public.amount
        || guard.release_id != public.release_id
        || guard.network_id != public.network_id
        || guard.asset_id != public.asset_id
        || guard.asset_incarnation != public.asset_incarnation
        || guard.asset_scale != public.asset_scale
        || guard.liability_pool_id != public.liability_pool_id
        || guard.hardware_profile_id != public.hardware_profile_id
        || guard.policy_epoch != public.policy_epoch
        || guard.lifecycle_binding_digest != public.lifecycle_binding_digest
        || guard.prepared_transition_binding_digest != prepared_transition
        || guard.terminal_commit_binding_digest != terminal
        || guard.sender_one_time_authorization_digest != sender_authorization
        || guard.predecessor_state_commitment != predecessor.state_commitment
        || guard.successor_state_commitment != successor.state_commitment
        || guard.predecessor_state_nonce_commitment != predecessor.state_nonce_commitment
        || guard.successor_state_nonce_commitment != successor.state_nonce_commitment
        || guard.predecessor_logical_sequence != predecessor.logical_sequence
        || guard.successor_logical_sequence != successor.logical_sequence
        || guard.predecessor_hardware_epoch_generation != predecessor.hardware_epoch.generation
        || guard.successor_hardware_epoch_generation != successor.hardware_epoch.generation
        || guard.predecessor_hardware_epoch_id != predecessor.hardware_epoch.epoch_id
        || guard.successor_hardware_epoch_id != successor.hardware_epoch.epoch_id
        || guard.predecessor_key_reference != predecessor.device_policy_binding.device_key_reference
        || guard.successor_key_reference != successor.device_policy_binding.device_key_reference
        || guard.predecessor_hardware_policy_id
            != predecessor.device_policy_binding.hardware_policy_id
        || guard.successor_hardware_policy_id != successor.device_policy_binding.hardware_policy_id
        || guard.journal_revision_before != private.journal_revision_before
        || guard.journal_revision_after != private.journal_revision_after
    {
        return Err("terminal Guard/private terminal-authorization relation mismatch".to_owned());
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_candidate_guard_protocol_binding_v1<F: KagemushaPoseidonFieldV1>(
    candidate_instances: &[Vec<F>],
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<(), String> {
    let candidate = candidate_instances
        .first()
        .ok_or_else(|| "terminal authorization candidate public column is absent".to_owned())?;
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT {
        return Err("terminal authorization candidate public column is truncated".to_owned());
    }
    for (offset, digest) in [
        (
            state_relation::public_instance::GUARD_EQ_PROTOCOL_LO,
            terminal_guard_eq_protocol_digest,
        ),
        (
            state_relation::public_instance::GUARD_EP_PROTOCOL_LO,
            terminal_guard_ep_protocol_digest,
        ),
    ] {
        let expected = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest);
        if candidate[offset..offset + 2] != expected {
            return Err(
                "terminal authorization candidate selects a different GuardBundle protocol"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

/// Compact native deferred audits retained after the discovery builders are dropped.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaTerminalAuthorizationDeferredAuditsV1 {
    eq: KagemushaDeferredParentOutputV1<EqAffine>,
    ep: KagemushaDeferredParentOutputV1<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaTerminalAuthorizationDeferredAuditsV1 {
    #[must_use]
    pub(crate) const fn eq_digest(&self) -> DigestV1 {
        self.eq_digest
    }

    #[must_use]
    pub(crate) const fn ep_digest(&self) -> DigestV1 {
        self.ep_digest
    }
}

/// Compact native deferred audits for the key-distinct postcommit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaCommitWrapperDeferredAuditsV1 {
    eq: KagemushaDeferredParentOutputV1<EqAffine>,
    ep: KagemushaDeferredParentOutputV1<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaCommitWrapperDeferredAuditsV1 {
    #[must_use]
    pub(crate) const fn eq_digest(&self) -> DigestV1 {
        self.eq_digest
    }

    #[must_use]
    pub(crate) const fn ep_digest(&self) -> DigestV1 {
        self.ep_digest
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_terminal_authorization_pair_witness_v1(
    witness: &KagemushaTerminalAuthorizationWitnessV1<'_>,
) -> Result<(DigestV1, DigestV1), String> {
    validate_enabled_hardware_profiles_v1(&witness.enabled_hardware_profiles)?;
    let terminal_guard_eq_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq.terminal_guard_protocol,
        KagemushaPastaParityV1::Eq,
    )?;
    let terminal_guard_ep_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep.terminal_guard_protocol,
        KagemushaPastaParityV1::Ep,
    )?;
    validate_candidate_guard_protocol_binding_v1(
        witness.eq.candidate_instances,
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    )?;
    validate_candidate_guard_protocol_binding_v1(
        witness.ep.candidate_instances,
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    )?;
    witness
        .private_transition
        .validate_against(&witness.public)?;
    validate_terminal_guard_relation_v1(
        &witness.public,
        &witness.private_transition,
        &witness.terminal_guard_relation,
    )?;
    if !witness
        .enabled_hardware_profiles
        .iter()
        .take_while(|profile| **profile != [0; 32])
        .any(|profile| *profile == witness.public.hardware_profile_id)
    {
        return Err("terminal sender hardware profile is not release-enabled".to_owned());
    }
    Ok((
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_terminal_authorization_eq_scalar_from_witness_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &KagemushaTerminalAuthorizationWitnessV1<'_>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<Fp>,
        PastaSha256JobsV1<Fp>,
        KagemushaDeferredParentOutputV1<EqAffine>,
    ),
    String,
> {
    let eq_candidate_history = witness
        .eq
        .candidate_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_terminal_history = witness
        .eq
        .terminal_guard_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_svk = super::composite::eq_succinct_vk(eq_params);
    build_terminal_authorization_scalar_half_v1::<EqAffine>(
        &eq_svk,
        KagemushaPastaParityV1::Eq,
        KagemushaTerminalRelationV1::TerminalAuthorization,
        &witness.public,
        TerminalAuthorizationParityWitnessV1 {
            private_transition: &witness.private_transition,
            terminal_guard_relation: &witness.terminal_guard_relation,
            enabled_hardware_profiles: &witness.enabled_hardware_profiles,
            candidate_protocol: witness.eq.candidate_protocol,
            candidate_instances: witness.eq.candidate_instances,
            candidate_proof: witness.eq.candidate_proof,
            candidate_history: &eq_candidate_history,
            candidate_history_fold_proof: witness.eq.candidate_history_fold_proof.as_bytes(),
            terminal_guard_protocol: witness.eq.terminal_guard_protocol,
            terminal_guard_eq_protocol_digest,
            terminal_guard_ep_protocol_digest,
            terminal_guard_instances: witness.eq.terminal_guard_instances,
            terminal_guard_proof: witness.eq.terminal_guard_proof,
            terminal_guard_history: &eq_terminal_history,
            terminal_guard_history_fold_proof: witness
                .eq
                .terminal_guard_history_fold_proof
                .as_bytes(),
            merge_fold_proof: witness.eq.merge_fold_proof.as_bytes(),
            successor_history: witness.eq.successor_history.as_bytes(),
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_terminal_authorization_ep_scalar_from_witness_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaTerminalAuthorizationWitnessV1<'_>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<Fq>,
        PastaSha256JobsV1<Fq>,
        KagemushaDeferredParentOutputV1<EpAffine>,
    ),
    String,
> {
    let ep_candidate_history = witness
        .ep
        .candidate_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_terminal_history = witness
        .ep
        .terminal_guard_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_svk = super::composite::ep_succinct_vk(ep_params);
    build_terminal_authorization_scalar_half_v1::<EpAffine>(
        &ep_svk,
        KagemushaPastaParityV1::Ep,
        KagemushaTerminalRelationV1::TerminalAuthorization,
        &witness.public,
        TerminalAuthorizationParityWitnessV1 {
            private_transition: &witness.private_transition,
            terminal_guard_relation: &witness.terminal_guard_relation,
            enabled_hardware_profiles: &witness.enabled_hardware_profiles,
            candidate_protocol: witness.ep.candidate_protocol,
            candidate_instances: witness.ep.candidate_instances,
            candidate_proof: witness.ep.candidate_proof,
            candidate_history: &ep_candidate_history,
            candidate_history_fold_proof: witness.ep.candidate_history_fold_proof.as_bytes(),
            terminal_guard_protocol: witness.ep.terminal_guard_protocol,
            terminal_guard_eq_protocol_digest,
            terminal_guard_ep_protocol_digest,
            terminal_guard_instances: witness.ep.terminal_guard_instances,
            terminal_guard_proof: witness.ep.terminal_guard_proof,
            terminal_guard_history: &ep_terminal_history,
            terminal_guard_history_fold_proof: witness
                .ep
                .terminal_guard_history_fold_proof
                .as_bytes(),
            merge_fold_proof: witness.ep.merge_fold_proof.as_bytes(),
            successor_history: witness.ep.successor_history.as_bytes(),
        },
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn terminal_authorization_public_values_v1<F: KagemushaPoseidonFieldV1>(
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
    successor_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    use crate::zk::kagemusha_v1_poseidon::from_u128;

    let mut values = public.public_prefix::<F>()?;
    values.extend(successor_history.chunks_exact(16).map(|chunk| {
        from_u128::<F>(u128::from_le_bytes(
            chunk
                .try_into()
                .expect("fixed history chunks are sixteen bytes"),
        ))
    }));
    if values.len() != TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("terminal-authorization public instance ABI mismatch".to_owned());
    }
    Ok(values)
}

/// Discover both terminal-authorization audits while retaining no scalar Base graph.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn derive_kagemusha_terminal_authorization_deferred_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaTerminalAuthorizationWitnessV1<'_>,
) -> Result<KagemushaTerminalAuthorizationDeferredAuditsV1, String> {
    let (terminal_guard_eq_protocol_digest, terminal_guard_ep_protocol_digest) =
        validate_terminal_authorization_pair_witness_v1(witness)?;
    let (eq_builder, eq_sha, eq_output) = build_terminal_authorization_eq_scalar_from_witness_v1(
        eq_params,
        witness,
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    )?;
    let eq_digest = super::composite::assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    drop(eq_builder);
    drop(eq_sha);
    halo2_proofs::release_allocator_slack();

    let (ep_builder, ep_sha, ep_output) = build_terminal_authorization_ep_scalar_from_witness_v1(
        ep_params,
        witness,
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    )?;
    let ep_digest = super::composite::assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    drop(ep_builder);
    drop(ep_sha);
    halo2_proofs::release_allocator_slack();

    Ok(KagemushaTerminalAuthorizationDeferredAuditsV1 {
        eq: eq_output,
        ep: ep_output,
        eq_digest,
        ep_digest,
    })
}

/// Build the exact Eq terminal-authorization circuit from compact reciprocal audits.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_terminal_authorization_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &KagemushaTerminalAuthorizationWitnessV1<'_>,
    audits: &KagemushaTerminalAuthorizationDeferredAuditsV1,
) -> Result<(KagemushaTerminalAuthorizationEqCircuitV1, Vec<Fp>), String> {
    let (terminal_guard_eq_protocol_digest, terminal_guard_ep_protocol_digest) =
        validate_terminal_authorization_pair_witness_v1(witness)?;
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err(
            "terminal authorization public values do not bind the derived audit pair".to_owned(),
        );
    }
    let public_values = terminal_authorization_public_values_v1::<Fp>(
        &witness.public,
        witness.eq.successor_history.as_bytes(),
    )?;
    let (mut builder, sha_jobs, output) = build_terminal_authorization_eq_scalar_from_witness_v1(
        eq_params,
        witness,
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    )?;
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EpAffine>(
        &mut builder,
        KagemushaPastaParityV1::Ep,
        &audits.ep,
        &mut dense_jobs,
    )?;
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << super::KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS;
    sha_jobs.validate_capacity(usable_rows)?;
    dense_jobs.validate_capacity(usable_rows)?;
    if super::composite::assigned_digest_bytes(&output.audit_digest_limbs)? != audits.eq_digest {
        return Err(
            "Eq terminal-authorization audit changed after exact public rebinding".to_owned(),
        );
    }
    Ok((
        KagemushaTerminalAuthorizationEqCircuitV1 {
            builder,
            sha_jobs,
            dense_jobs,
        },
        public_values,
    ))
}

/// Build the exact Ep terminal-authorization circuit from compact reciprocal audits.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_terminal_authorization_ep_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaTerminalAuthorizationWitnessV1<'_>,
    audits: &KagemushaTerminalAuthorizationDeferredAuditsV1,
) -> Result<(KagemushaTerminalAuthorizationEpCircuitV1, Vec<Fq>), String> {
    let (terminal_guard_eq_protocol_digest, terminal_guard_ep_protocol_digest) =
        validate_terminal_authorization_pair_witness_v1(witness)?;
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err(
            "terminal authorization public values do not bind the derived audit pair".to_owned(),
        );
    }
    let public_values = terminal_authorization_public_values_v1::<Fq>(
        &witness.public,
        witness.ep.successor_history.as_bytes(),
    )?;
    let (mut builder, sha_jobs, output) = build_terminal_authorization_ep_scalar_from_witness_v1(
        ep_params,
        witness,
        terminal_guard_eq_protocol_digest,
        terminal_guard_ep_protocol_digest,
    )?;
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EqAffine>(
        &mut builder,
        KagemushaPastaParityV1::Eq,
        &audits.eq,
        &mut dense_jobs,
    )?;
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << super::KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS;
    sha_jobs.validate_capacity(usable_rows)?;
    dense_jobs.validate_capacity(usable_rows)?;
    if super::composite::assigned_digest_bytes(&output.audit_digest_limbs)? != audits.ep_digest {
        return Err(
            "Ep terminal-authorization audit changed after exact public rebinding".to_owned(),
        );
    }
    Ok((
        KagemushaTerminalAuthorizationEpCircuitV1 {
            builder,
            sha_jobs,
            dense_jobs,
        },
        public_values,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_commit_wrapper_pair_witness_v1(
    witness: &KagemushaCommitWrapperWitnessV1<'_>,
) -> Result<(DigestV1, DigestV1), String> {
    witness.public.validate()?;
    let terminal_authorization_eq_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq.terminal_authorization_protocol,
        KagemushaPastaParityV1::Eq,
    )?;
    let terminal_authorization_ep_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep.terminal_authorization_protocol,
        KagemushaPastaParityV1::Ep,
    )?;
    if terminal_authorization_eq_protocol_digest == terminal_authorization_ep_protocol_digest {
        return Err("terminal-authorization parity protocols alias".to_owned());
    }
    Ok((
        terminal_authorization_eq_protocol_digest,
        terminal_authorization_ep_protocol_digest,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_commit_wrapper_eq_scalar_from_witness_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &KagemushaCommitWrapperWitnessV1<'_>,
    terminal_authorization_eq_protocol_digest: DigestV1,
    terminal_authorization_ep_protocol_digest: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<Fp>,
        KagemushaDeferredParentOutputV1<EqAffine>,
        Vec<AssignedValue<Fp>>,
    ),
    String,
> {
    let eq_terminal_history = witness
        .eq
        .terminal_authorization_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_svk = super::composite::eq_succinct_vk(eq_params);
    build_commit_wrapper_scalar_half_v1::<EqAffine>(
        &eq_svk,
        KagemushaPastaParityV1::Eq,
        &witness.public,
        terminal_authorization_eq_protocol_digest,
        terminal_authorization_ep_protocol_digest,
        witness.eq.terminal_authorization_protocol,
        witness.eq.terminal_authorization_instances,
        witness.eq.terminal_authorization_proof,
        &eq_terminal_history,
        witness
            .eq
            .terminal_authorization_history_fold_proof
            .as_bytes(),
        witness.eq.successor_history.as_bytes(),
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_commit_wrapper_ep_scalar_from_witness_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaCommitWrapperWitnessV1<'_>,
    terminal_authorization_eq_protocol_digest: DigestV1,
    terminal_authorization_ep_protocol_digest: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<Fq>,
        KagemushaDeferredParentOutputV1<EpAffine>,
        Vec<AssignedValue<Fq>>,
    ),
    String,
> {
    let ep_terminal_history = witness
        .ep
        .terminal_authorization_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_svk = super::composite::ep_succinct_vk(ep_params);
    build_commit_wrapper_scalar_half_v1::<EpAffine>(
        &ep_svk,
        KagemushaPastaParityV1::Ep,
        &witness.public,
        terminal_authorization_eq_protocol_digest,
        terminal_authorization_ep_protocol_digest,
        witness.ep.terminal_authorization_protocol,
        witness.ep.terminal_authorization_instances,
        witness.ep.terminal_authorization_proof,
        &ep_terminal_history,
        witness
            .ep
            .terminal_authorization_history_fold_proof
            .as_bytes(),
        witness.ep.successor_history.as_bytes(),
    )
}

/// Discover both commit-wrapper audits while retaining no scalar Base graph.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn derive_kagemusha_commit_wrapper_deferred_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaCommitWrapperWitnessV1<'_>,
) -> Result<KagemushaCommitWrapperDeferredAuditsV1, String> {
    let (terminal_authorization_eq_protocol_digest, terminal_authorization_ep_protocol_digest) =
        validate_commit_wrapper_pair_witness_v1(witness)?;
    let (eq_builder, eq_output, eq_inner_binding_cells) =
        build_commit_wrapper_eq_scalar_from_witness_v1(
            eq_params,
            witness,
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
        )?;
    let eq_digest = super::composite::assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    drop(eq_builder);
    drop(eq_inner_binding_cells);
    halo2_proofs::release_allocator_slack();

    let (ep_builder, ep_output, ep_inner_binding_cells) =
        build_commit_wrapper_ep_scalar_from_witness_v1(
            ep_params,
            witness,
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
        )?;
    let ep_digest = super::composite::assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    drop(ep_builder);
    drop(ep_inner_binding_cells);
    halo2_proofs::release_allocator_slack();

    Ok(KagemushaCommitWrapperDeferredAuditsV1 {
        eq: eq_output,
        ep: ep_output,
        eq_digest,
        ep_digest,
    })
}

/// Build the exact Eq commit-wrapper circuit from compact reciprocal audits.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_commit_wrapper_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &KagemushaCommitWrapperWitnessV1<'_>,
    audits: &KagemushaCommitWrapperDeferredAuditsV1,
) -> Result<(KagemushaCommitWrapperEqCircuitV1, Vec<Fp>), String> {
    let (terminal_authorization_eq_protocol_digest, terminal_authorization_ep_protocol_digest) =
        validate_commit_wrapper_pair_witness_v1(witness)?;
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err("commit wrapper public values do not bind the derived audit pair".to_owned());
    }
    let public_values = terminal_authorization_public_values_v1::<Fp>(
        &witness.public,
        witness.eq.successor_history.as_bytes(),
    )?;
    let (mut eq_builder, eq_output, eq_inner_binding_cells) =
        build_commit_wrapper_eq_scalar_from_witness_v1(
            eq_params,
            witness,
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
        )?;
    let eq_expected_ep_audit: [AssignedValue<Fp>; 2] = eq_builder
        .assigned_instances
        .first()
        .and_then(|column| {
            column.get(
                public_instance::EP_DEFERRED_AUDIT_LO..public_instance::EP_DEFERRED_AUDIT_LO + 2,
            )
        })
        .ok_or_else(|| "Eq authorization Ep audit is absent".to_owned())?
        .try_into()
        .map_err(|_| "Eq authorization Ep audit has the wrong shape".to_owned())?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EpAffine>(
        &mut eq_builder,
        &audits.ep,
        &eq_expected_ep_audit,
        &eq_inner_binding_cells,
    )?;
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    if super::composite::assigned_digest_bytes(&eq_output.audit_digest_limbs)? != audits.eq_digest {
        return Err("Eq commit-wrapper audit changed after exact public rebinding".to_owned());
    }
    Ok((
        KagemushaCommitWrapperEqCircuitV1 {
            builder: eq_builder,
        },
        public_values,
    ))
}

/// Build the exact Ep commit-wrapper circuit from compact reciprocal audits.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_commit_wrapper_ep_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaCommitWrapperWitnessV1<'_>,
    audits: &KagemushaCommitWrapperDeferredAuditsV1,
) -> Result<(KagemushaCommitWrapperEpCircuitV1, Vec<Fq>), String> {
    let (terminal_authorization_eq_protocol_digest, terminal_authorization_ep_protocol_digest) =
        validate_commit_wrapper_pair_witness_v1(witness)?;
    if witness.public.eq_deferred_audit != audits.eq_digest
        || witness.public.ep_deferred_audit != audits.ep_digest
    {
        return Err("commit wrapper public values do not bind the derived audit pair".to_owned());
    }
    let public_values = terminal_authorization_public_values_v1::<Fq>(
        &witness.public,
        witness.ep.successor_history.as_bytes(),
    )?;
    let (mut ep_builder, ep_output, ep_inner_binding_cells) =
        build_commit_wrapper_ep_scalar_from_witness_v1(
            ep_params,
            witness,
            terminal_authorization_eq_protocol_digest,
            terminal_authorization_ep_protocol_digest,
        )?;
    let ep_expected_eq_audit: [AssignedValue<Fq>; 2] = ep_builder
        .assigned_instances
        .first()
        .and_then(|column| {
            column.get(
                public_instance::EQ_DEFERRED_AUDIT_LO..public_instance::EQ_DEFERRED_AUDIT_LO + 2,
            )
        })
        .ok_or_else(|| "Ep authorization Eq audit is absent".to_owned())?
        .try_into()
        .map_err(|_| "Ep authorization Eq audit has the wrong shape".to_owned())?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EqAffine>(
        &mut ep_builder,
        &audits.eq,
        &ep_expected_eq_audit,
        &ep_inner_binding_cells,
    )?;
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    if super::composite::assigned_digest_bytes(&ep_output.audit_digest_limbs)? != audits.ep_digest {
        return Err("Ep commit-wrapper audit changed after exact public rebinding".to_owned());
    }
    Ok((
        KagemushaCommitWrapperEpCircuitV1 {
            builder: ep_builder,
        },
        public_values,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments)]
fn build_commit_wrapper_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
    terminal_authorization_eq_protocol_digest: DigestV1,
    terminal_authorization_ep_protocol_digest: DigestV1,
    terminal_authorization_protocol: &PlonkProtocol<C>,
    terminal_authorization_instances: &[Vec<C::ScalarExt>],
    terminal_authorization_proof: &[u8],
    terminal_authorization_history: &IpaAccumulator<C, NativeLoader>,
    terminal_authorization_history_fold_proof: &[u8],
    successor_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        KagemushaDeferredParentOutputV1<C>,
        Vec<AssignedValue<C::ScalarExt>>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    public.validate()?;
    if terminal_authorization_protocol.num_instance
        != [TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1]
        || terminal_authorization_instances.len() != 1
        || terminal_authorization_instances[0].len()
            != TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("commit wrapper nested proof has wrong fixed public shape".to_owned());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(
            usize::try_from(super::KAGEMUSHA_RECURSION_IPA_K_V1).expect("Kagemusha k fits usize"),
        )
        .use_lookup_bits(
            usize::try_from(super::KAGEMUSHA_RECURSION_IPA_K_V1 - 1)
                .expect("Kagemusha lookup bits fit usize"),
        )
        .use_instance_columns(1);
    let range = builder.range_chip();
    let public_cells = assign_public_prefix_v1(&mut builder, &range, public)?;
    let history_cells = assign_history_v1(&mut builder, &range, successor_history)?;
    builder.assigned_instances = vec![
        public_cells
            .iter()
            .copied()
            .chain(history_cells.iter().copied())
            .collect(),
    ];
    constrain_terminal_relation_domain_v1(
        &mut builder,
        &range,
        KagemushaTerminalRelationV1::CommitWrapper,
    );

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let nested_instances = assign_nested_instances_v1(&loader, terminal_authorization_instances);
    let nested_column = nested_instances
        .first()
        .ok_or_else(|| "nested terminal-authorization public column is absent".to_owned())?;
    for (actual, expected) in nested_column[..public_instance::EQ_DEFERRED_AUDIT_LO]
        .iter()
        .zip(&public_cells[..public_instance::EQ_DEFERRED_AUDIT_LO])
    {
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&actual.assigned(), expected);
    }
    for (offset, digest) in [
        (
            public_instance::EQ_PROTOCOL_LO,
            terminal_authorization_eq_protocol_digest,
        ),
        (
            public_instance::EP_PROTOCOL_LO,
            terminal_authorization_ep_protocol_digest,
        ),
    ] {
        let expected = crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(digest);
        for (actual, expected) in nested_column[offset..offset + 2].iter().zip(expected) {
            let constant = loader.ctx_mut().main().load_constant(expected);
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &constant);
        }
    }
    let inner_binding_cells = nested_column
        [public_instance::EQ_DEFERRED_AUDIT_LO..public_instance::HISTORY_START]
        .iter()
        .map(|value| {
            let assigned = *value.assigned();
            range.range_check(loader.ctx_mut().main(), assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    let protocol = terminal_authorization_protocol.loaded(&loader);
    let current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &protocol,
        &nested_instances,
        terminal_authorization_proof,
    )
    .map_err(|error| format!("terminal-authorization verifier failed: {error:?}"))?;
    let prior = load_native_accumulator(&loader, terminal_authorization_history)
        .map_err(|error| format!("terminal-authorization history failed: {error:?}"))?;
    let prior_limbs = nested_column[public_instance::HISTORY_START..]
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &prior, &prior_limbs)
        .map_err(|error| format!("terminal-authorization history binding failed: {error:?}"))?;
    let complete = verify_fold(
        &loader,
        succinct_vk,
        &[current, prior],
        terminal_authorization_history_fold_proof,
    )
    .map_err(|error| format!("terminal-authorization history fold failed: {error:?}"))?;
    bind_accumulator_limbs(&loader, &complete, &history_cells)
        .map_err(|error| format!("authorization successor history binding failed: {error:?}"))?;
    let equation_count = loader.ecc_chip().equation_count();
    if equation_count == 0 {
        return Err("terminal-authorization verifier emitted no equations".to_owned());
    }
    let output = finalize_tagged_deferred_audit_with_u128_binding_v1(
        &mut builder,
        loader,
        TERMINAL_AUTHORIZATION_EQUATION_TAG_V1,
        &inner_binding_cells,
    )
    .map_err(|error| format!("authorization deferred audit failed: {error:?}"))?;
    let expected_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_DEFERRED_AUDIT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_DEFERRED_AUDIT_LO,
    };
    for (actual, expected) in output
        .audit_digest_limbs
        .iter()
        .zip(&public_cells[expected_offset..expected_offset + 2])
    {
        builder.main(0).constrain_equal(actual, expected);
    }
    Ok((builder, output, inner_binding_cells))
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_terminal_authorization_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    relation: KagemushaTerminalRelationV1,
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
    witness: TerminalAuthorizationParityWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        PastaSha256JobsV1<C::ScalarExt>,
        KagemushaDeferredParentOutputV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    public.validate()?;
    if witness.candidate_protocol.num_instance
        != [state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count()]
        || witness.candidate_instances.len() != 1
        || witness.candidate_instances[0].len()
            != state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count()
        || witness.terminal_guard_protocol.num_instance
            != [GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]
        || witness.terminal_guard_instances.len() != 1
        || witness.terminal_guard_instances[0].len() != GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("terminal authorization nested proof has wrong fixed public shape".to_owned());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(
            usize::try_from(super::KAGEMUSHA_RECURSION_IPA_K_V1).expect("Kagemusha k fits usize"),
        )
        .use_lookup_bits(
            usize::try_from(super::KAGEMUSHA_RECURSION_IPA_K_V1 - 1)
                .expect("Kagemusha lookup bits fit usize"),
        )
        .use_instance_columns(1);
    let range = builder.range_chip();
    let public_cells = assign_public_prefix_v1(&mut builder, &range, public)?;
    let history_cells = assign_history_v1(&mut builder, &range, witness.successor_history)?;
    builder.assigned_instances = vec![
        public_cells
            .iter()
            .copied()
            .chain(history_cells.iter().copied())
            .collect(),
    ];
    if builder.assigned_instances[0].len() != TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("terminal authorization public instance has wrong fixed shape".to_owned());
    }

    let mut sha_jobs = PastaSha256JobsV1::default();
    let assigned_terminal_guard = constrain_guard_bundle_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        witness.terminal_guard_relation,
    )?;
    constrain_terminal_relation_domain_v1(&mut builder, &range, relation);
    constrain_terminal_commit_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        witness.private_transition,
        &assigned_terminal_guard,
    )?;
    let profile_enabled = builder.main(0).load_constant(C::ScalarExt::ONE);
    constrain_enabled_hardware_profile_membership_v1(
        builder.main(0),
        &range,
        profile_enabled,
        assigned_terminal_guard.hardware_profile_id,
        witness.enabled_hardware_profiles,
    );
    let candidate_protocol_digest =
        native_parent_protocol_digest_v1(witness.candidate_protocol, parity)?;
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let candidate_instances = assign_nested_instances_v1(&loader, witness.candidate_instances);
    let candidate_column = candidate_instances
        .first()
        .ok_or_else(|| "terminal authorization candidate public column is absent".to_owned())?;
    constrain_candidate_projection_v1(
        &loader,
        candidate_column,
        &public_cells,
        candidate_protocol_digest,
        parity,
        &mut sha_jobs,
    )?;
    constrain_candidate_terminal_guard_binding_v1(
        &loader,
        candidate_column,
        &public_cells,
        &assigned_terminal_guard,
        witness.terminal_guard_eq_protocol_digest,
        witness.terminal_guard_ep_protocol_digest,
    )?;
    // Candidate and terminal-Guard protocols are loaded as constants. Their exact verifying-key
    // material is consequently committed by the authenticated terminal-authorization verifying key; a prover
    // cannot select an arbitrary protocol through witness bytes.
    let candidate_protocol = witness.candidate_protocol.loaded(&loader);
    let candidate_current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &candidate_protocol,
        &candidate_instances,
        witness.candidate_proof,
    )
    .map_err(|error| format!("terminal authorization candidate verifier failed: {error:?}"))?;
    let candidate_history = load_native_accumulator(&loader, witness.candidate_history)
        .map_err(|error| format!("terminal authorization candidate history failed: {error:?}"))?;
    let candidate_history_limbs = candidate_column
        .get(state_relation::PUBLIC_INSTANCE_COUNT..)
        .ok_or_else(|| "terminal authorization candidate history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &candidate_history, &candidate_history_limbs).map_err(
        |error| format!("terminal authorization candidate history binding failed: {error:?}"),
    )?;
    let complete_candidate = verify_fold(
        &loader,
        succinct_vk,
        &[candidate_current, candidate_history],
        witness.candidate_history_fold_proof,
    )
    .map_err(|error| format!("terminal authorization candidate fold failed: {error:?}"))?;
    let candidate_end = loader.ecc_chip().equation_count();
    if candidate_end == 0 {
        return Err("terminal authorization candidate verifier emitted no equations".to_owned());
    }

    let terminal_instances = assign_nested_instances_v1(&loader, witness.terminal_guard_instances);
    let terminal_column = terminal_instances.first().ok_or_else(|| {
        "terminal authorization terminal Guard public column is absent".to_owned()
    })?;
    let gate = halo2_base::gates::GateChip::default();
    let receiver_binding_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_cells[public_instance::RECEIVER_BINDING_LO],
    );
    let receiver_binding_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_cells[public_instance::RECEIVER_BINDING_LO + 1],
    );
    let receiver_binding_zero = gate.and(
        loader.ctx_mut().main(),
        receiver_binding_low_zero,
        receiver_binding_high_zero,
    );
    let nullifier_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_cells[public_instance::TRANSITION_NULLIFIER_LO],
    );
    let nullifier_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_cells[public_instance::TRANSITION_NULLIFIER_LO + 1],
    );
    let nullifier_zero = gate.and(
        loader.ctx_mut().main(),
        nullifier_low_zero,
        nullifier_high_zero,
    );
    let authorization_branch = gate.and(
        loader.ctx_mut().main(),
        receiver_binding_zero,
        nullifier_zero,
    );
    for (guard_offset, authorization_offset) in [
        (GUARD_EQ_AUDIT_OFFSET_V1, public_instance::CIPHERTEXT_LO),
        (GUARD_EP_AUDIT_OFFSET_V1, public_instance::OUTPUT_BINDING_LO),
    ] {
        for limb in 0..2 {
            let difference = gate.sub(
                loader.ctx_mut().main(),
                *terminal_column[guard_offset + limb].assigned(),
                public_cells[authorization_offset + limb],
            );
            let selected = gate.mul(loader.ctx_mut().main(), authorization_branch, difference);
            gate.assert_is_const(loader.ctx_mut().main(), &selected, &C::ScalarExt::ZERO);
        }
    }
    let terminal_guard_digest = digest_limbs_assigned(
        loader.ctx_mut().main(),
        &assigned_terminal_guard.guard_digest,
    );
    for (actual, expected) in terminal_column.iter().take(2).zip(terminal_guard_digest) {
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&actual.assigned(), &expected);
    }
    let terminal_protocol = witness.terminal_guard_protocol.loaded(&loader);
    let terminal_current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &terminal_protocol,
        &terminal_instances,
        witness.terminal_guard_proof,
    )
    .map_err(|error| format!("terminal authorization terminal Guard verifier failed: {error:?}"))?;
    let terminal_history = load_native_accumulator(&loader, witness.terminal_guard_history)
        .map_err(|error| {
            format!("terminal authorization terminal Guard history failed: {error:?}")
        })?;
    let terminal_history_limbs = terminal_column
        .get(GUARD_HISTORY_OFFSET_V1..)
        .ok_or_else(|| "terminal authorization terminal Guard history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &terminal_history, &terminal_history_limbs).map_err(
        |error| format!("terminal authorization terminal Guard history binding failed: {error:?}"),
    )?;
    let complete_terminal = verify_fold(
        &loader,
        succinct_vk,
        &[terminal_current, terminal_history],
        witness.terminal_guard_history_fold_proof,
    )
    .map_err(|error| format!("terminal authorization terminal Guard fold failed: {error:?}"))?;
    let complete = verify_fold(
        &loader,
        succinct_vk,
        &[complete_candidate, complete_terminal],
        witness.merge_fold_proof,
    )
    .map_err(|error| format!("terminal authorization merged history fold failed: {error:?}"))?;
    bind_accumulator_limbs(&loader, &complete, &history_cells).map_err(|error| {
        format!("terminal authorization successor history binding failed: {error:?}")
    })?;
    let terminal_end = loader.ecc_chip().equation_count();
    if terminal_end <= candidate_end {
        return Err(
            "terminal authorization terminal Guard verifier emitted no equations".to_owned(),
        );
    }

    let mut tags = Vec::with_capacity(terminal_end);
    tags.extend(std::iter::repeat_n(
        CANDIDATE_EQUATION_TAG_V1,
        candidate_end,
    ));
    tags.extend(std::iter::repeat_n(
        TERMINAL_GUARD_EQUATION_TAG_V1,
        terminal_end - candidate_end,
    ));
    let enabled = loader.ctx_mut().main().load_constant(C::ScalarExt::ONE);
    let output = finalize_deferred_audit_plan_v1(
        &mut builder,
        loader,
        tags,
        vec![enabled; terminal_end],
        vec![true; terminal_end],
    )
    .map_err(|error| format!("terminal authorization deferred audit failed: {error:?}"))?;
    let expected_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_DEFERRED_AUDIT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_DEFERRED_AUDIT_LO,
    };
    for (actual, expected) in output
        .audit_digest_limbs
        .iter()
        .zip(&public_cells[expected_offset..][..2])
    {
        builder.main(0).constrain_equal(actual, expected);
    }
    Ok((builder, sha_jobs, output))
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_terminal_relation_domain_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &RangeChip<F>,
    relation: KagemushaTerminalRelationV1,
) {
    // The compact wrapper recursively verifies TerminalAuthorization without sharing its
    // proving authority. This fixed
    // tag changes the fixed-column assignment (and therefore the VK/protocol) even if a future
    // refactor makes the two surrounding constraint graphs otherwise structurally identical.
    let tag = match relation {
        KagemushaTerminalRelationV1::TerminalAuthorization => 1_u64,
        KagemushaTerminalRelationV1::CommitWrapper => 2,
    };
    let cell = builder.main(0).load_witness(F::from(tag));
    range.range_check(builder.main(0), cell, 8);
    range
        .gate()
        .assert_is_const(builder.main(0), &cell, &F::from(tag));
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_public_prefix_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &RangeChip<F>,
    public: &KagemushaTerminalAuthorizationPublicInputsV1,
) -> Result<Vec<AssignedValue<F>>, String> {
    let cells = public
        .public_prefix::<F>()?
        .into_iter()
        .map(|value| {
            let cell = builder.main(0).load_witness(value);
            range.range_check(builder.main(0), cell, 128);
            cell
        })
        .collect::<Vec<_>>();
    let gate = range.gate();
    gate.assert_is_const(
        builder.main(0),
        &cells[public_instance::PROTOCOL_VERSION],
        &F::ONE,
    );
    let send = gate.is_equal(
        builder.main(0),
        cells[public_instance::OPERATION],
        QuantumCell::Constant(F::from(2)),
    );
    let redeem = gate.is_equal(
        builder.main(0),
        cells[public_instance::OPERATION],
        QuantumCell::Constant(F::from(4)),
    );
    let valid_operation = gate.add(builder.main(0), send, redeem);
    gate.assert_is_const(builder.main(0), &valid_operation, &F::ONE);
    for index in [public_instance::AMOUNT, public_instance::POLICY_EPOCH] {
        let zero = gate.is_zero(builder.main(0), cells[index]);
        gate.assert_is_const(builder.main(0), &zero, &F::ZERO);
    }
    range.range_check(builder.main(0), cells[public_instance::POLICY_EPOCH], 64);
    range.range_check(builder.main(0), cells[public_instance::ASSET_SCALE], 32);
    for offset in [
        public_instance::SUITE_LO,
        public_instance::VK_LO,
        public_instance::RELEASE_LO,
        public_instance::NETWORK_LO,
        public_instance::ASSET_LO,
        public_instance::ASSET_INCARNATION_LO,
        public_instance::LIABILITY_POOL_LO,
        public_instance::HARDWARE_PROFILE_LO,
        public_instance::LIFECYCLE_LO,
        public_instance::SEMANTIC_LO,
        public_instance::CANDIDATE_LO,
        public_instance::COMMIT_CERTIFICATE_LO,
        public_instance::TRANSITION_NULLIFIER_LO,
        public_instance::OUTPUT_BINDING_LO,
        public_instance::EQ_DEFERRED_AUDIT_LO,
        public_instance::EP_DEFERRED_AUDIT_LO,
        public_instance::EQ_PROTOCOL_LO,
        public_instance::EP_PROTOCOL_LO,
    ] {
        let nonzero =
            assigned_digest_nonzero_v1(builder, range, [cells[offset], cells[offset + 1]]);
        gate.assert_is_const(builder.main(0), &nonzero, &F::ONE);
    }
    for offset in [
        public_instance::REQUEST_LO,
        public_instance::RECEIVER_BINDING_LO,
        public_instance::CIPHERTEXT_LO,
    ] {
        let nonzero =
            assigned_digest_nonzero_v1(builder, range, [cells[offset], cells[offset + 1]]);
        builder.main(0).constrain_equal(&nonzero, &send);
    }
    for (left, right) in [
        (
            public_instance::CANDIDATE_LO,
            public_instance::COMMIT_CERTIFICATE_LO,
        ),
        (
            public_instance::EQ_DEFERRED_AUDIT_LO,
            public_instance::EP_DEFERRED_AUDIT_LO,
        ),
        (
            public_instance::EQ_PROTOCOL_LO,
            public_instance::EP_PROTOCOL_LO,
        ),
    ] {
        let lo_equal = gate.is_equal(builder.main(0), cells[left], cells[right]);
        let hi_equal = gate.is_equal(builder.main(0), cells[left + 1], cells[right + 1]);
        let equal = gate.and(builder.main(0), lo_equal, hi_equal);
        gate.assert_is_const(builder.main(0), &equal, &F::ZERO);
    }
    Ok(cells)
}

#[cfg(feature = "zk-halo2-ipa")]
fn assigned_digest_nonzero_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &halo2_base::gates::RangeChip<F>,
    digest: [AssignedValue<F>; 2],
) -> AssignedValue<F> {
    let lo_zero = range.gate().is_zero(builder.main(0), digest[0]);
    let hi_zero = range.gate().is_zero(builder.main(0), digest[1]);
    let both_zero = range.gate().and(builder.main(0), lo_zero, hi_zero);
    range.gate().not(builder.main(0), both_zero)
}

#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
struct AssignedFixedUintV1<F: KagemushaPoseidonFieldV1> {
    value: AssignedValue<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_fixed_uint_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u128,
    bits: usize,
) -> AssignedFixedUintV1<F> {
    let value = ctx.load_witness(F::from_u128(value));
    range.range_check(ctx, value, bits);
    let bits = PastaSha256BitV1::decompose(ctx, range.gate(), value, bits);
    let bytes = bits
        .chunks_exact(8)
        .map(|chunk| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), chunk))
        .collect();
    AssignedFixedUintV1 { value, bytes }
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_fixed_digest_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: DigestV1,
) -> [PastaSha256ByteV1<F>; 32] {
    assign_bytes(ctx, range, &digest)
        .try_into()
        .expect("fixed digest width")
}

#[cfg(feature = "zk-halo2-ipa")]
fn assigned_limbs_to_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    limbs: [AssignedValue<F>; 2],
) -> Vec<PastaSha256ByteV1<F>> {
    let mut bytes = Vec::with_capacity(32);
    for limb in limbs {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), limb, 128);
        for chunk in bits.chunks_exact(8) {
            bytes.push(PastaSha256ByteV1::from_bits_le(ctx, range.gate(), chunk));
        }
    }
    bytes
}

#[cfg(feature = "zk-halo2-ipa")]
fn assigned_value_to_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
    bits: usize,
) -> Vec<PastaSha256ByteV1<F>> {
    let bits = PastaSha256BitV1::decompose(ctx, range.gate(), value, bits);
    bits.chunks_exact(8)
        .map(|chunk| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), chunk))
        .collect()
}

#[cfg(feature = "zk-halo2-ipa")]
fn digest_nonzero_from_limbs_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: [AssignedValue<F>; 2],
) -> AssignedValue<F> {
    let low_zero = range.gate().is_zero(ctx, digest[0]);
    let high_zero = range.gate().is_zero(ctx, digest[1]);
    let both_zero = range.gate().and(ctx, low_zero, high_zero);
    range.gate().not(ctx, both_zero)
}

#[cfg(feature = "zk-halo2-ipa")]
fn select_digest_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    when_true: &[PastaSha256ByteV1<F>; 32],
    selector: AssignedValue<F>,
) -> [PastaSha256ByteV1<F>; 32] {
    core::array::from_fn(|index| {
        let selected = range.gate().select(
            ctx,
            when_true[index].quantum_cell(),
            QuantumCell::Constant(F::ZERO),
            selector,
        );
        PastaSha256ByteV1::range_checked(ctx, range, selected)
    })
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_equal_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
) {
    let difference = range.gate().sub(ctx, left, right);
    let selected = range.gate().mul(ctx, selector, difference);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_zero_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    value: AssignedValue<F>,
) {
    let selected = range.gate().mul(ctx, selector, value);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

/// Constrain a hidden profile ID to one release-pinned fixed-table entry.
///
/// Every slot is loaded as a circuit constant, including zero padding, so key generation and
/// witness generation have identical topology. The caller selects whether membership is active.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn constrain_enabled_hardware_profile_membership_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    hidden_profile: [AssignedValue<F>; 2],
    enabled_profiles: &[DigestV1; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
) {
    let gate = range.gate();
    let mut matched = ctx.load_constant(F::ZERO);
    for profile in enabled_profiles {
        let limbs = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(*profile);
        let low = ctx.load_constant(limbs[0]);
        let high = ctx.load_constant(limbs[1]);
        let low_equal = gate.is_equal(ctx, hidden_profile[0], low);
        let high_equal = gate.is_equal(ctx, hidden_profile[1], high);
        let slot_match = gate.and(ctx, low_equal, high_equal);
        let product = gate.mul(ctx, matched, slot_match);
        let sum = gate.add(ctx, matched, slot_match);
        matched = gate.sub(ctx, sum, product);
    }
    let missing = gate.not(ctx, matched);
    constrain_zero_if_v1(ctx, range, selector, missing);
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_digest_limbs_equal_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    actual: &[PastaSha256ByteV1<F>; 32],
    expected: [AssignedValue<F>; 2],
) {
    for (actual, expected) in digest_limbs_assigned(ctx, actual).into_iter().zip(expected) {
        ctx.constrain_equal(&actual, &expected);
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_less_than_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
    bits: usize,
) {
    let less = range.is_less_than(ctx, left, right, bits);
    let one = ctx.load_constant(F::ONE);
    constrain_equal_if_v1(ctx, range, selector, less, one);
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_not_less_than_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
    bits: usize,
) {
    let less = range.is_less_than(ctx, left, right, bits);
    constrain_zero_if_v1(ctx, range, selector, less);
}

/// Bind equal fixed transcript bytes without a host authorization flag.
#[cfg(feature = "zk-halo2-ipa")]
fn constrain_transcript_bytes_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    actual: &[PastaSha256ByteV1<F>],
    expected: &[PastaSha256ByteV1<F>],
) {
    assert_eq!(actual.len(), expected.len(), "fixed transcript field width");
    for (actual, expected) in actual.iter().zip(expected) {
        let difference = range
            .gate()
            .sub(ctx, actual.quantum_cell(), expected.quantum_cell());
        constrain_zero_if_v1(ctx, range, enabled, difference);
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn transcript_uint_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) -> AssignedValue<F> {
    assert!(bytes.len() <= 16, "fixed integer transcript width");
    range.gate().inner_product(
        ctx,
        bytes.iter().map(|byte| byte.quantum_cell()),
        (0..bytes.len()).map(|index| QuantumCell::Constant(F::from_u128(1_u128 << (index * 8)))),
    )
}

#[cfg(feature = "zk-halo2-ipa")]
fn hash_terminal_transcript_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    bytes: Vec<PastaSha256ByteV1<F>>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    hash(
        ctx,
        jobs,
        [
            constant_bytes(domain),
            constant_bytes(&[0]),
            constant_bytes(&(bytes.len() as u64).to_le_bytes()),
            bytes,
        ]
        .concat(),
    )
}

/// Open the exact request credential ID once to its receiver lane.
///
/// The entire canonical Norito preimage, including its CRC64, is hashed against the credential ID
/// in the already-authenticated signed request. Pinned schema/header/flags/field prefixes establish
/// the lane's exact offset. CRC bytes are bound by that authenticated ID; this is not a standalone
/// codec validator. Another CRC or value cannot open the same ID without a SHA-256 collision.
/// No receiver epoch/profile is equated with the sender or with a later receiving wallet epoch.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn constrain_receiver_credential_lane_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    preimage: Option<&[u8]>,
    requested_credential: &[PastaSha256ByteV1<F>],
    enabled: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let padding = [0; KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1];
    let preimage = preimage.unwrap_or(&padding);
    if preimage.len() != KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1
        || requested_credential.len() != 32
    {
        return Err("terminal receiver credential opening width changed".to_owned());
    }
    let bytes = assign_bytes(ctx, range, preimage);
    let framing =
        kagemusha_hardware_credential_id_preimage_layout_v1().map_err(|error| error.to_string())?;
    for (index, fixed) in framing.into_iter().enumerate() {
        if let Some(fixed) = fixed {
            constrain_transcript_bytes_if_v1(
                ctx,
                range,
                enabled,
                &bytes[index..index + 1],
                &constant_bytes(&[fixed]),
            );
        }
    }
    constrain_transcript_bytes_if_v1(
        ctx,
        range,
        enabled,
        &bytes[41..43],
        &constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()),
    );
    let offset = KAGEMUSHA_HARDWARE_CREDENTIAL_ID_LANE_OFFSET_V1;
    let lane = bytes[offset..offset + 32]
        .try_into()
        .expect("fixed receiver lane width");
    let digest = hash_terminal_transcript_v1(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:hardware-credential-id",
        bytes,
    )?;
    constrain_transcript_bytes_if_v1(ctx, range, enabled, &digest, requested_credential);
    Ok(lane)
}

/// Shared private transcript for the sender's terminal proof and each receiver slot.
///
/// The six digests are credit ID, recipient encryption key, receiver lane, prepared transfer,
/// payment output, and incoming claims. Only the resulting digest is public, never the lane.
/// Amount, request, sender-state pair, and opening commitment also retain their direct
/// public-instance equalities.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn hash_terminal_send_output_binding_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    digests: [&[PastaSha256ByteV1<F>]; 6],
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if digests.iter().any(|digest| digest.len() != 32) {
        return Err("terminal send-output binding width changed".to_owned());
    }
    let mut message = constant_bytes(TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1);
    message.extend(constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()));
    for digest in digests {
        message.extend_from_slice(digest);
    }
    hash(ctx, jobs, message)
}

/// Constrain the exact existing seven-digest incoming claims transcript once, at the sender.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn hash_incoming_payment_claims_binding_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    digests: [&[PastaSha256ByteV1<F>]; 7],
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if digests.iter().any(|digest| digest.len() != 32) {
        return Err("incoming payment claims binding width changed".to_owned());
    }
    let mut message = constant_bytes(super::INCOMING_PROOF_BINDING_DOMAIN_V1);
    message.extend(constant_bytes(&[0]));
    for digest in digests {
        message.extend_from_slice(digest);
    }
    hash(ctx, jobs, message)
}

/// Constrain the canonical 210-byte prepared-transfer transcript, returning its 32-byte digest.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn hash_terminal_prepared_transfer_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    digests: [&[PastaSha256ByteV1<F>]; 6],
    amount: &[PastaSha256ByteV1<F>],
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if digests.iter().any(|digest| digest.len() != 32) || amount.len() != 16 {
        return Err("terminal prepared-transfer width changed".to_owned());
    }
    let mut transcript = constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
    // Request, sender before/after, amount, nullifier, recipient key, ciphertext commitment.
    for digest in &digests[..3] {
        transcript.extend_from_slice(digest);
    }
    transcript.extend_from_slice(amount);
    for digest in &digests[3..] {
        transcript.extend_from_slice(digest);
    }
    hash_terminal_transcript_v1(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:prepared-transfer",
        transcript,
    )
}

/// Hash the exact signed receiver transcripts and bind their monetary opening to the candidate.
#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments)]
fn constrain_terminal_send_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    send: AssignedValue<F>,
    private: &KagemushaTerminalAuthorizationPrivateTransitionV1,
    public: &[AssignedValue<F>],
    guard: &KagemushaAssignedGuardBundleV1<F>,
    request_issued: &AssignedFixedUintV1<F>,
    request_expires: &AssignedFixedUintV1<F>,
) -> Result<
    (
        [PastaSha256ByteV1<F>; 32],
        [PastaSha256ByteV1<F>; 32],
        AssignedValue<F>,
    ),
    String,
> {
    let opening = private.send.as_ref();
    let present = ctx.load_witness(F::from(u64::from(opening.is_some())));
    ctx.constrain_equal(&present, &send);
    let request_bytes = opening
        .map(|value| value.request.circuit_transcript_bytes())
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| vec![0; 390]);
    let output_bytes = opening
        .map(|value| value.output.circuit_transcript_bytes().to_vec())
        .unwrap_or_else(|| vec![0; 254]);
    if request_bytes.len() != 390 || output_bytes.len() != 254 {
        return Err("terminal send fixed transcript shape changed".to_owned());
    }
    let request = assign_bytes(ctx, range, &request_bytes);
    let output = assign_bytes(ctx, range, &output_bytes);
    for bytes in [&request, &output] {
        constrain_transcript_bytes_if_v1(
            ctx,
            range,
            send,
            &bytes[..2],
            &constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()),
        );
    }
    let request_digest = hash_terminal_transcript_v1(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:payment-request",
        request.clone(),
    )?;
    let credential_preimage = opening
        .map(|value| {
            value
                .request
                .hardware_credential
                .canonical_id_preimage_bytes()
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let receiver_lane = constrain_receiver_credential_lane_v1(
        ctx,
        range,
        jobs,
        credential_preimage.as_deref(),
        &request[246..278],
        send,
    )?;
    let output_digest = hash_terminal_transcript_v1(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:send-split-statement",
        output.clone(),
    )?;
    for (bytes, offset) in [
        (&request_digest[..], public_instance::REQUEST_LO),
        (&request[246..278], public_instance::RECEIVER_BINDING_LO),
        (&output[114..146], public_instance::TRANSITION_NULLIFIER_LO),
        (&output[178..210], public_instance::CIPHERTEXT_LO),
    ] {
        let expected = assigned_limbs_to_bytes_v1(ctx, range, [public[offset], public[offset + 1]]);
        constrain_transcript_bytes_if_v1(ctx, range, send, bytes, &expected);
    }
    for (bytes, offset) in [
        (&request[2..34], public_instance::RELEASE_LO),
        (&request[34..66], public_instance::NETWORK_LO),
        (&request[66..98], public_instance::ASSET_LO),
        (&request[98..130], public_instance::ASSET_INCARNATION_LO),
        (&request[134..166], public_instance::LIABILITY_POOL_LO),
    ] {
        let expected = assigned_limbs_to_bytes_v1(ctx, range, [public[offset], public[offset + 1]]);
        constrain_transcript_bytes_if_v1(ctx, range, send, bytes, &expected);
    }
    let scale = assigned_value_to_bytes_v1(ctx, range, public[public_instance::ASSET_SCALE], 32);
    constrain_transcript_bytes_if_v1(ctx, range, send, &request[130..134], &scale);
    constrain_transcript_bytes_if_v1(ctx, range, send, &output[2..34], &request_digest);
    let amount_bytes = assigned_value_to_bytes_v1(ctx, range, public[public_instance::AMOUNT], 128);
    constrain_transcript_bytes_if_v1(ctx, range, send, &request[198..214], &amount_bytes);
    constrain_transcript_bytes_if_v1(ctx, range, send, &output[34..50], &amount_bytes);
    for (actual, expected) in [
        (&request[310..318], &request_issued.bytes[..]),
        (&request[318..326], &request_expires.bytes[..]),
    ] {
        constrain_transcript_bytes_if_v1(ctx, range, send, actual, expected);
    }
    for bytes in [
        &request[166..198],
        &request[214..246],
        &request[246..278],
        &request[278..310],
        &request[326..358],
        &request[358..390],
    ] {
        let digest: [PastaSha256ByteV1<F>; 32] = bytes.try_into().expect("fixed transcript digest");
        let limbs = digest_limbs_assigned(ctx, &digest);
        let nonzero = digest_nonzero_from_limbs_v1(ctx, range, limbs);
        constrain_equal_if_v1(ctx, range, send, nonzero, send);
    }
    constrain_less_than_if_v1(
        ctx,
        range,
        send,
        request_issued.value,
        request_expires.value,
        64,
    );
    let peer_credit = assigned_limbs_to_bytes_v1(ctx, range, guard.peer_credit_id);
    constrain_transcript_bytes_if_v1(ctx, range, send, &output[146..178], &peer_credit);
    // SendSplit's peer-recipient cell carries the request-owned encryption key.
    let recipient_encryption_key =
        assigned_limbs_to_bytes_v1(ctx, range, guard.recipient_encryption_key_binding);
    constrain_transcript_bytes_if_v1(
        ctx,
        range,
        send,
        &request[214..246],
        &recipient_encryption_key,
    );
    let evidence_tag = assign_fixed_uint_v1(
        ctx,
        range,
        u128::from(evidence_tag_v1(private.commit_certificate.commit_evidence)),
        32,
    );
    let evidence = assign_fixed_digest_v1(
        ctx,
        range,
        evidence_commitment_v1(private.commit_certificate.commit_evidence),
    );
    constrain_transcript_bytes_if_v1(ctx, range, send, &output[210..214], &evidence_tag.bytes);
    constrain_transcript_bytes_if_v1(ctx, range, send, &output[214..246], &evidence);
    let credit_id = hash(
        ctx,
        jobs,
        [
            constant_bytes(KAGEMUSHA_CREDIT_ID_DOMAIN_V1),
            constant_bytes(&[0]),
            output[114..146].to_vec(),
            request_digest.to_vec(),
        ]
        .concat(),
    )?;
    constrain_transcript_bytes_if_v1(ctx, range, send, &output[146..178], &credit_id);
    let encrypted_digest = assign_fixed_digest_v1(
        ctx,
        range,
        opening.map_or([0; 32], |value| value.encrypted_credit_digest),
    );
    let encrypted_limbs = digest_limbs_assigned(ctx, &encrypted_digest);
    let encrypted_nonzero = digest_nonzero_from_limbs_v1(ctx, range, encrypted_limbs);
    constrain_equal_if_v1(ctx, range, send, encrypted_nonzero, send);
    let prepared_transfer = hash_terminal_prepared_transfer_v1(
        ctx,
        jobs,
        [
            &request_digest,
            &output[50..82],
            &output[82..114],
            &output[114..146],
            &request[214..246],
            &output[178..210],
        ],
        &amount_bytes,
    )?;
    // These public cells are independently constrained to the verified State candidate and the
    // exact postcommit certificate below. Neither candidate nor certificate contains this claims
    // digest or the output binding, so this additional terminal commitment is acyclic.
    let candidate = assigned_limbs_to_bytes_v1(
        ctx,
        range,
        [
            public[public_instance::CANDIDATE_LO],
            public[public_instance::CANDIDATE_LO + 1],
        ],
    );
    let certificate = assigned_limbs_to_bytes_v1(
        ctx,
        range,
        [
            public[public_instance::COMMIT_CERTIFICATE_LO],
            public[public_instance::COMMIT_CERTIFICATE_LO + 1],
        ],
    );
    let state_pair = hash(
        ctx,
        jobs,
        [
            constant_bytes(b"iroha:kagemusha:v1:incoming-sender-state-pair"),
            constant_bytes(&[0]),
            output[50..82].to_vec(),
            output[82..114].to_vec(),
        ]
        .concat(),
    )?;
    let incoming_claims = hash_incoming_payment_claims_binding_v1(
        ctx,
        jobs,
        [
            &request_digest,
            &request[246..278],
            &state_pair,
            &output_digest,
            &encrypted_digest,
            &candidate,
            &certificate,
        ],
    )?;
    let terminal_output = hash_terminal_send_output_binding_v1(
        ctx,
        jobs,
        [
            &output[146..178],
            &request[214..246],
            &receiver_lane,
            &prepared_transfer,
            &output_digest,
            &incoming_claims,
        ],
    )?;
    let expected_output = assigned_limbs_to_bytes_v1(
        ctx,
        range,
        [
            public[public_instance::OUTPUT_BINDING_LO],
            public[public_instance::OUTPUT_BINDING_LO + 1],
        ],
    );
    constrain_transcript_bytes_if_v1(ctx, range, send, &terminal_output, &expected_output);
    let body = hash_terminal_transcript_v1(
        ctx,
        jobs,
        b"iroha:kagemusha:v1:payment-body",
        [output_digest.to_vec(), encrypted_digest.to_vec()].concat(),
    )?;
    let expected_semantic = assigned_limbs_to_bytes_v1(
        ctx,
        range,
        [
            public[public_instance::SEMANTIC_LO],
            public[public_instance::SEMANTIC_LO + 1],
        ],
    );
    constrain_transcript_bytes_if_v1(ctx, range, send, &body, &expected_semantic);
    let payload = assign_fixed_digest_v1(ctx, range, private.terminal_payload_digest);
    constrain_transcript_bytes_if_v1(ctx, range, send, &body, &payload);
    let committed_at = transcript_uint_v1(ctx, range, &output[246..254]);
    Ok((
        select_digest_bytes_v1(
            ctx,
            range,
            &output[50..82].try_into().expect("sender-before digest"),
            send,
        ),
        select_digest_bytes_v1(
            ctx,
            range,
            &output[82..114].try_into().expect("sender-after digest"),
            send,
        ),
        committed_at,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_terminal_commit_semantics_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    public: &[AssignedValue<F>],
    private: &KagemushaTerminalAuthorizationPrivateTransitionV1,
    guard: &KagemushaAssignedGuardBundleV1<F>,
) -> Result<(), String> {
    if public.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1 {
        return Err("terminal-authorization public prefix is truncated".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let raw_send = gate.is_equal(
        ctx,
        public[public_instance::OPERATION],
        QuantumCell::Constant(F::from(2)),
    );
    let certificate_low_zero = gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO]);
    let certificate_high_zero =
        gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO + 1]);
    let certificate_zero = gate.and(ctx, certificate_low_zero, certificate_high_zero);
    let certificate_nonzero = gate.not(ctx, certificate_zero);
    let nullifier_low_zero = gate.is_zero(ctx, public[public_instance::TRANSITION_NULLIFIER_LO]);
    let nullifier_high_zero =
        gate.is_zero(ctx, public[public_instance::TRANSITION_NULLIFIER_LO + 1]);
    let nullifier_zero = gate.and(ctx, nullifier_low_zero, nullifier_high_zero);
    let nullifier_nonzero = gate.not(ctx, nullifier_zero);
    let terminal_branch = gate.and(ctx, certificate_nonzero, nullifier_nonzero);
    let send = gate.and(ctx, raw_send, terminal_branch);
    let operation_byte =
        PastaSha256ByteV1::range_checked(ctx, &range, public[public_instance::OPERATION]);

    for (actual, expected) in [
        (guard.operation, public[public_instance::OPERATION]),
        (
            guard.protocol_version,
            public[public_instance::PROTOCOL_VERSION],
        ),
        (guard.amount, public[public_instance::AMOUNT]),
        (guard.asset_scale, public[public_instance::ASSET_SCALE]),
        (guard.policy_epoch, public[public_instance::POLICY_EPOCH]),
    ] {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }
    for (actual, offset) in [
        (guard.predecessor_suite_id, public_instance::SUITE_LO),
        (guard.predecessor_vk_digest, public_instance::VK_LO),
        (guard.successor_suite_id, public_instance::SUITE_LO),
        (guard.successor_vk_digest, public_instance::VK_LO),
        (guard.release_id, public_instance::RELEASE_LO),
        (guard.network_id, public_instance::NETWORK_LO),
        (guard.asset_id, public_instance::ASSET_LO),
        (
            guard.asset_incarnation,
            public_instance::ASSET_INCARNATION_LO,
        ),
        (guard.liability_pool_id, public_instance::LIABILITY_POOL_LO),
        (
            guard.hardware_profile_id,
            public_instance::HARDWARE_PROFILE_LO,
        ),
        (
            guard.lifecycle_binding_digest,
            public_instance::LIFECYCLE_LO,
        ),
    ] {
        for (actual, expected) in actual.into_iter().zip(&public[offset..offset + 2]) {
            constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
        }
    }

    let request_issued = assign_fixed_uint_v1(
        ctx,
        &range,
        private
            .send
            .as_ref()
            .map_or(0, |value| u128::from(value.request.issued_at_ms)),
        64,
    );
    let request_expires = assign_fixed_uint_v1(
        ctx,
        &range,
        private
            .send
            .as_ref()
            .map_or(0, |value| u128::from(value.request.expires_at_ms)),
        64,
    );
    let one_use_authorization =
        assign_fixed_digest_v1(ctx, &range, private.one_use_hardware_authorization);
    let one_use_limbs = digest_limbs_assigned(ctx, &one_use_authorization);
    let one_use_nonzero = digest_nonzero_from_limbs_v1(ctx, &range, one_use_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        one_use_nonzero,
        terminal_branch,
    );
    let authorization_counter_before =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_before, 128);
    let authorization_counter_after =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_after, 128);
    let incremented_authorization_counter = gate.inc(ctx, authorization_counter_before.value);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        incremented_authorization_counter,
        authorization_counter_after.value,
    );
    let journal_revision_before =
        assign_fixed_uint_v1(ctx, &range, private.journal_revision_before, 128);
    let journal_revision_after =
        assign_fixed_uint_v1(ctx, &range, private.journal_revision_after, 128);
    let incremented_journal_revision = gate.inc(ctx, journal_revision_before.value);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        incremented_journal_revision,
        journal_revision_after.value,
    );
    let predecessor_state =
        assign_fixed_digest_v1(ctx, &range, private.predecessor.state_commitment);
    let predecessor_nonce =
        assign_fixed_digest_v1(ctx, &range, private.predecessor.state_nonce_commitment);
    let predecessor_lane =
        assign_fixed_digest_v1(ctx, &range, private.predecessor.lane.device_lane_id);
    let predecessor_epoch =
        assign_fixed_digest_v1(ctx, &range, private.predecessor.hardware_epoch.epoch_id);
    let predecessor_key = assign_fixed_digest_v1(
        ctx,
        &range,
        private
            .predecessor
            .device_policy_binding
            .device_key_reference,
    );
    let predecessor_sequence =
        assign_fixed_uint_v1(ctx, &range, private.predecessor.logical_sequence, 128);
    let prepared_message = [
        constant_bytes(PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        vec![operation_byte.clone()],
        one_use_authorization.to_vec(),
        predecessor_state.to_vec(),
        predecessor_nonce.to_vec(),
        predecessor_lane.to_vec(),
        predecessor_epoch.to_vec(),
        predecessor_key.to_vec(),
        predecessor_sequence.bytes,
        journal_revision_before.bytes,
        authorization_counter_before.bytes.clone(),
    ]
    .concat();
    let prepared_authorization = hash(ctx, jobs, prepared_message)?;
    let derived_transition_nullifier = hash(
        ctx,
        jobs,
        [
            constant_bytes(PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1),
            constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()),
            prepared_authorization.to_vec(),
        ]
        .concat(),
    )?;
    for (actual, expected) in digest_limbs_assigned(ctx, &derived_transition_nullifier)
        .into_iter()
        .zip(&public[public_instance::TRANSITION_NULLIFIER_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
    }

    let (selected_sender_before, selected_sender_after, payment_committed_at) =
        constrain_terminal_send_opening_v1(
            ctx,
            &range,
            jobs,
            send,
            private,
            public,
            guard,
            &request_issued,
            &request_expires,
        )?;

    let reservation = private.outbox_reservation;
    let reservation_id = assign_fixed_digest_v1(ctx, &range, reservation.reservation_id);
    let reservation_operation = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(operation_tag_v1(operation_from_wire_v1(
            reservation.operation_kind,
        ))),
        32,
    );
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        reservation_operation.value,
        public[public_instance::OPERATION],
    );
    let reserved_outbox_bytes = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(reservation.reserved_outbox_bytes),
        32,
    );
    let reservation_issued =
        assign_fixed_uint_v1(ctx, &range, u128::from(reservation.issued_at_ms), 64);
    let reservation_expires =
        assign_fixed_uint_v1(ctx, &range, u128::from(reservation.expires_at_ms), 64);
    let redemption_min =
        ctx.load_constant(F::from(u64::from(KAGEMUSHA_REDEMPTION_OUTBOX_MIN_BYTES_V1)));
    let payment_min = ctx.load_constant(F::from(u64::from(KAGEMUSHA_PAYMENT_OUTBOX_MIN_BYTES_V1)));
    let selected_min = gate.select(ctx, payment_min, redemption_min, send);
    let reservation_too_small =
        range.is_less_than(ctx, reserved_outbox_bytes.value, selected_min, 32);
    constrain_zero_if_v1(ctx, &range, terminal_branch, reservation_too_small);
    constrain_less_than_if_v1(
        ctx,
        &range,
        terminal_branch,
        reservation_issued.value,
        reservation_expires.value,
        64,
    );
    let reservation_message = [
        constant_bytes(OUTBOX_RESERVATION_COMMITMENT_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(OUTBOX_RESERVATION_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        reservation_id.to_vec(),
        reservation_operation.bytes,
        reserved_outbox_bytes.bytes,
        reservation_issued.bytes.clone(),
        reservation_expires.bytes.clone(),
    ]
    .concat();
    let reservation_commitment = hash(ctx, jobs, reservation_message)?;

    let evidence = private.commit_evidence_opening;
    let evidence_kind = assign_fixed_uint_v1(ctx, &range, u128::from(evidence.kind()), 8);
    let certificate_evidence_kind = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(evidence_tag_v1(private.commit_certificate.commit_evidence)),
        8,
    );
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        evidence_kind.value,
        certificate_evidence_kind.value,
    );
    let lease = evidence_kind.value;
    gate.assert_bit(ctx, lease);
    let trusted = gate.not(ctx, lease);
    let evidence_opening = assign_fixed_digest_v1(ctx, &range, evidence.opening);
    let evidence_opening_limbs = digest_limbs_assigned(ctx, &evidence_opening);
    let evidence_opening_nonzero =
        digest_nonzero_from_limbs_v1(ctx, &range, evidence_opening_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        evidence_opening_nonzero,
        terminal_branch,
    );
    let trusted_commit_time =
        assign_fixed_uint_v1(ctx, &range, u128::from(evidence.trusted_commit_time_ms), 64);
    let lease_id = assign_fixed_digest_v1(ctx, &range, evidence.lease_id);
    let lease_valid_from =
        assign_fixed_uint_v1(ctx, &range, u128::from(evidence.lease_valid_from_ms), 64);
    let lease_expires =
        assign_fixed_uint_v1(ctx, &range, u128::from(evidence.lease_expires_at_ms), 64);
    let terminal_lease = gate.and(ctx, terminal_branch, lease);
    let terminal_trusted = gate.and(ctx, terminal_branch, trusted);
    constrain_zero_if_v1(ctx, &range, terminal_lease, trusted_commit_time.value);
    constrain_zero_if_v1(ctx, &range, terminal_trusted, lease_valid_from.value);
    constrain_zero_if_v1(ctx, &range, terminal_trusted, lease_expires.value);
    let lease_id_limbs = digest_limbs_assigned(ctx, &lease_id);
    let lease_id_present = digest_nonzero_from_limbs_v1(ctx, &range, lease_id_limbs);
    constrain_equal_if_v1(ctx, &range, terminal_branch, lease_id_present, lease);
    let trusted_time_zero = gate.is_zero(ctx, trusted_commit_time.value);
    let trusted_time_present = gate.not(ctx, trusted_time_zero);
    constrain_equal_if_v1(ctx, &range, terminal_branch, trusted_time_present, trusted);
    let evidence_message = [
        constant_bytes(COMMIT_EVIDENCE_OPENING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        evidence_kind.bytes.clone(),
        evidence_opening.to_vec(),
        trusted_commit_time.bytes.clone(),
        lease_id.to_vec(),
        lease_valid_from.bytes.clone(),
        lease_expires.bytes.clone(),
        authorization_counter_before.bytes,
        authorization_counter_after.bytes,
    ]
    .concat();
    let evidence_commitment = hash(ctx, jobs, evidence_message)?;
    let certificate_evidence_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        evidence_commitment_v1(private.commit_certificate.commit_evidence),
    );
    let certificate_evidence_limbs = digest_limbs_assigned(ctx, &certificate_evidence_commitment);
    for (actual, expected) in digest_limbs_assigned(ctx, &evidence_commitment)
        .into_iter()
        .zip(certificate_evidence_limbs)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }

    constrain_not_less_than_if_v1(
        ctx,
        &range,
        terminal_trusted,
        trusted_commit_time.value,
        reservation_issued.value,
        64,
    );
    constrain_less_than_if_v1(
        ctx,
        &range,
        terminal_trusted,
        trusted_commit_time.value,
        reservation_expires.value,
        64,
    );
    constrain_less_than_if_v1(
        ctx,
        &range,
        terminal_lease,
        lease_valid_from.value,
        lease_expires.value,
        64,
    );
    constrain_not_less_than_if_v1(
        ctx,
        &range,
        terminal_lease,
        lease_valid_from.value,
        reservation_issued.value,
        64,
    );
    constrain_not_less_than_if_v1(
        ctx,
        &range,
        terminal_lease,
        reservation_expires.value,
        lease_expires.value,
        64,
    );
    let send_trusted = gate.and(ctx, send, trusted);
    let send_lease = gate.and(ctx, send, lease);
    for (issued, expires) in [(request_issued.value, request_expires.value)] {
        constrain_not_less_than_if_v1(
            ctx,
            &range,
            send_trusted,
            trusted_commit_time.value,
            issued,
            64,
        );
        constrain_less_than_if_v1(
            ctx,
            &range,
            send_trusted,
            trusted_commit_time.value,
            expires,
            64,
        );
        constrain_not_less_than_if_v1(ctx, &range, send_lease, lease_valid_from.value, issued, 64);
        constrain_not_less_than_if_v1(ctx, &range, send_lease, expires, lease_expires.value, 64);
    }
    constrain_equal_if_v1(
        ctx,
        &range,
        send_trusted,
        payment_committed_at,
        trusted_commit_time.value,
    );
    constrain_not_less_than_if_v1(
        ctx,
        &range,
        send_lease,
        payment_committed_at,
        lease_valid_from.value,
        64,
    );
    constrain_less_than_if_v1(
        ctx,
        &range,
        send_lease,
        payment_committed_at,
        lease_expires.value,
        64,
    );
    let certificate = &private.commit_certificate;
    let certificate_version =
        assign_fixed_uint_v1(ctx, &range, u128::from(certificate.version), 16);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        certificate_version.value,
        terminal_branch,
    );
    let certificate_id = assign_fixed_digest_v1(ctx, &range, certificate.certificate_id);
    let candidate_envelope =
        assign_fixed_digest_v1(ctx, &range, certificate.candidate_envelope_digest);
    let certificate_lifecycle =
        assign_fixed_digest_v1(ctx, &range, certificate.lifecycle_binding_digest);
    let transition_nullifier =
        assign_fixed_digest_v1(ctx, &range, certificate.transition_nullifier);
    let certificate_reservation =
        assign_fixed_digest_v1(ctx, &range, certificate.outbox_reservation_commitment);
    let certificate_profile = assign_fixed_digest_v1(ctx, &range, certificate.hardware_profile_id);
    let certificate_policy_epoch =
        assign_fixed_uint_v1(ctx, &range, u128::from(certificate.policy_epoch), 64);
    let hardware_terminal =
        assign_fixed_digest_v1(ctx, &range, certificate.hardware_terminal_commitment);
    for (actual, offset) in [
        (&candidate_envelope, public_instance::CANDIDATE_LO),
        (&certificate_lifecycle, public_instance::LIFECYCLE_LO),
        (
            &transition_nullifier,
            public_instance::TRANSITION_NULLIFIER_LO,
        ),
        (&certificate_profile, public_instance::HARDWARE_PROFILE_LO),
    ] {
        for (actual, expected) in digest_limbs_assigned(ctx, actual)
            .into_iter()
            .zip(&public[offset..offset + 2])
        {
            constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
        }
    }
    let reservation_commitment_limbs = digest_limbs_assigned(ctx, &reservation_commitment);
    for (actual, expected) in digest_limbs_assigned(ctx, &certificate_reservation)
        .into_iter()
        .zip(reservation_commitment_limbs)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        certificate_policy_epoch.value,
        public[public_instance::POLICY_EPOCH],
    );
    let hardware_terminal_limbs = digest_limbs_assigned(ctx, &hardware_terminal);
    let hardware_terminal_nonzero =
        digest_nonzero_from_limbs_v1(ctx, &range, hardware_terminal_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        hardware_terminal_nonzero,
        terminal_branch,
    );

    let evidence_tag_u32 = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(evidence_tag_v1(certificate.commit_evidence)),
        32,
    );
    let certificate_id_message = [
        constant_bytes(COMMIT_CERTIFICATE_ID_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(COMMIT_CERTIFICATE_ID_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        certificate_version.bytes.clone(),
        candidate_envelope.to_vec(),
        certificate_lifecycle.to_vec(),
        transition_nullifier.to_vec(),
        certificate_reservation.to_vec(),
        evidence_tag_u32.bytes.clone(),
        certificate_evidence_commitment.to_vec(),
        certificate_profile.to_vec(),
        certificate_policy_epoch.bytes.clone(),
        hardware_terminal.to_vec(),
    ]
    .concat();
    let certificate_id_digest = hash(ctx, jobs, certificate_id_message)?;
    let certificate_id_limbs = digest_limbs_assigned(ctx, &certificate_id);
    for (actual, expected) in digest_limbs_assigned(ctx, &certificate_id_digest)
        .into_iter()
        .zip(certificate_id_limbs)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }
    let certificate_message = [
        constant_bytes(COMMIT_CERTIFICATE_DIGEST_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(COMMIT_CERTIFICATE_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        certificate_version.bytes,
        certificate_id.to_vec(),
        candidate_envelope.to_vec(),
        certificate_lifecycle.to_vec(),
        transition_nullifier.to_vec(),
        certificate_reservation.to_vec(),
        evidence_tag_u32.bytes,
        certificate_evidence_commitment.to_vec(),
        certificate_profile.to_vec(),
        certificate_policy_epoch.bytes,
        hardware_terminal.to_vec(),
    ]
    .concat();
    let certificate_digest = hash(ctx, jobs, certificate_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &certificate_digest)
        .into_iter()
        .zip(&public[public_instance::COMMIT_CERTIFICATE_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
    }

    let lifecycle_bytes = assigned_limbs_to_bytes_v1(
        ctx,
        &range,
        [
            public[public_instance::LIFECYCLE_LO],
            public[public_instance::LIFECYCLE_LO + 1],
        ],
    );
    let request_digest_bytes = assigned_limbs_to_bytes_v1(
        ctx,
        &range,
        [
            public[public_instance::REQUEST_LO],
            public[public_instance::REQUEST_LO + 1],
        ],
    );
    let receiver_binding_bytes = assigned_limbs_to_bytes_v1(
        ctx,
        &range,
        [
            public[public_instance::RECEIVER_BINDING_LO],
            public[public_instance::RECEIVER_BINDING_LO + 1],
        ],
    );
    let amount_bytes =
        assigned_value_to_bytes_v1(ctx, &range, public[public_instance::AMOUNT], 128);
    let prepared_transition_message = [
        constant_bytes(PREPARED_TRANSITION_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        lifecycle_bytes.clone(),
        request_digest_bytes.clone(),
        selected_sender_before.to_vec(),
        selected_sender_after.to_vec(),
        amount_bytes.clone(),
        reservation_commitment.to_vec(),
        prepared_authorization.to_vec(),
    ]
    .concat();
    let prepared_transition = hash(ctx, jobs, prepared_transition_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &prepared_transition)
        .into_iter()
        .zip(guard.prepared_transition_binding_digest)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }

    let selected_sender_authorization =
        select_digest_bytes_v1(ctx, &range, &prepared_authorization, send);
    for (actual, expected) in digest_limbs_assigned(ctx, &selected_sender_authorization)
        .into_iter()
        .zip(guard.sender_one_time_authorization_digest)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }

    let terminal_envelope = assign_fixed_digest_v1(ctx, &range, private.terminal_payload_digest);
    let terminal_envelope_limbs = digest_limbs_assigned(ctx, &terminal_envelope);
    let terminal_envelope_nonzero =
        digest_nonzero_from_limbs_v1(ctx, &range, terminal_envelope_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        terminal_envelope_nonzero,
        terminal_branch,
    );
    let profile_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.hardware_profile_id);
    let policy_epoch_bytes = assigned_value_to_bytes_v1(ctx, &range, guard.policy_epoch, 64);
    let transition_intent_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.transition_intent);
    let transition_effect_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.transition_effect);
    let recovery_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.recovery_record);
    let inbox_effect_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.durable_inbox_effect);
    let outbox_effect_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.durable_outbox_effect);
    let terminal_message = [
        constant_bytes(TERMINAL_COMMIT_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        vec![operation_byte],
        lifecycle_bytes,
        prepared_transition.to_vec(),
        candidate_envelope.to_vec(),
        certificate_digest.to_vec(),
        certificate_id.to_vec(),
        hardware_terminal.to_vec(),
        transition_nullifier.to_vec(),
        reservation_commitment.to_vec(),
        evidence_kind.bytes,
        evidence_commitment.to_vec(),
        request_digest_bytes,
        receiver_binding_bytes,
        amount_bytes,
        profile_bytes,
        policy_epoch_bytes,
        request_issued.bytes,
        request_expires.bytes,
        selected_sender_before.to_vec(),
        selected_sender_after.to_vec(),
        reservation_issued.bytes,
        reservation_expires.bytes,
        transition_intent_bytes,
        transition_effect_bytes,
        recovery_bytes,
        inbox_effect_bytes,
        outbox_effect_bytes,
        terminal_envelope.to_vec(),
        selected_sender_authorization.to_vec(),
    ]
    .concat();
    let terminal_digest = hash(ctx, jobs, terminal_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &terminal_digest)
        .into_iter()
        .zip(guard.terminal_commit_binding_digest)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_history_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &halo2_base::gates::RangeChip<F>,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<AssignedValue<F>>, String> {
    let limbs = history
        .chunks_exact(16)
        .map(|chunk| {
            let value = F::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history chunk has sixteen bytes"),
            ));
            let cell = builder.main(0).load_witness(value);
            range.range_check(builder.main(0), cell, 128);
            cell
        })
        .collect::<Vec<_>>();
    if limbs.len() != accumulator_limb_count() {
        return Err("terminal authorization history has wrong fixed shape".to_owned());
    }
    Ok(limbs)
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_nested_instances_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    instances: &[Vec<C::ScalarExt>],
) -> Vec<Vec<DeferredScalar<'chip, C>>>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect()
        })
        .collect()
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_candidate_terminal_guard_binding_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    candidate: &[DeferredScalar<'chip, C>],
    public_authorization: &[AssignedValue<C::ScalarExt>],
    guard: &KagemushaAssignedGuardBundleV1<C::ScalarExt>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || public_authorization.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1
    {
        return Err("terminal Guard candidate prefix is truncated".to_owned());
    }
    for (offset, digest) in [
        (
            state_relation::public_instance::GUARD_EQ_PROTOCOL_LO,
            terminal_guard_eq_protocol_digest,
        ),
        (
            state_relation::public_instance::GUARD_EP_PROTOCOL_LO,
            terminal_guard_ep_protocol_digest,
        ),
    ] {
        let expected = crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(digest);
        for (actual, expected) in candidate[offset..offset + 2].iter().zip(expected) {
            let constant = loader.ctx_mut().main().load_constant(expected);
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &constant);
        }
    }
    for (index, expected) in [
        (state_relation::public_instance::OPERATION, guard.operation),
        (state_relation::public_instance::AMOUNT, guard.amount),
        (
            state_relation::public_instance::PROTOCOL_VERSION,
            guard.protocol_version,
        ),
        (
            state_relation::public_instance::POLICY_EPOCH,
            guard.policy_epoch,
        ),
        (
            state_relation::public_instance::ASSET_SCALE,
            guard.asset_scale,
        ),
    ] {
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&candidate[index].assigned(), &expected);
    }
    for (offset, expected) in [
        (
            state_relation::public_instance::PREDECESSOR_OUTER_LO,
            guard.predecessor_state,
        ),
        (
            state_relation::public_instance::SUCCESSOR_OUTER_LO,
            guard.successor_state,
        ),
        (
            state_relation::public_instance::RELEASE_LO,
            guard.release_id,
        ),
        (
            state_relation::public_instance::LIABILITY_POOL_LO,
            guard.liability_pool_id,
        ),
        (
            state_relation::public_instance::PEER_CREDIT_LO,
            guard.peer_credit_id,
        ),
        (
            state_relation::public_instance::RECIPIENT_ENCRYPTION_KEY_LO,
            guard.recipient_encryption_key_binding,
        ),
        (
            state_relation::public_instance::MINT_PROOF_BINDING_LO,
            guard.mint_finality_proof_binding_digest,
        ),
        (
            state_relation::public_instance::LIFECYCLE_LO,
            guard.lifecycle_binding_digest,
        ),
        (
            state_relation::public_instance::PREPARED_TRANSITION_LO,
            guard.prepared_transition_binding_digest,
        ),
        (
            state_relation::public_instance::PREDECESSOR_SUITE_LO,
            guard.predecessor_suite_id,
        ),
        (
            state_relation::public_instance::PREDECESSOR_VK_LO,
            guard.predecessor_vk_digest,
        ),
        (
            state_relation::public_instance::SUCCESSOR_SUITE_LO,
            guard.successor_suite_id,
        ),
        (
            state_relation::public_instance::SUCCESSOR_VK_LO,
            guard.successor_vk_digest,
        ),
        (
            state_relation::public_instance::ASSET_INCARNATION_LO,
            guard.asset_incarnation,
        ),
        (
            state_relation::public_instance::HARDWARE_PROFILE_LO,
            guard.hardware_profile_id,
        ),
        (
            state_relation::public_instance::NETWORK_LO,
            guard.network_id,
        ),
        (state_relation::public_instance::ASSET_LO, guard.asset_id),
    ] {
        for (actual, expected) in candidate[offset..offset + 2].iter().zip(expected) {
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &expected);
        }
    }

    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_candidate_projection_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    candidate: &[DeferredScalar<'chip, C>],
    public_authorization: &[AssignedValue<C::ScalarExt>],
    candidate_protocol_digest: DigestV1,
    parity: KagemushaPastaParityV1,
    sha_jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || public_authorization.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1
    {
        return Err("terminal authorization candidate projection is truncated".to_owned());
    }
    let gate = halo2_base::gates::GateChip::default();
    let nullifier_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_authorization[public_instance::TRANSITION_NULLIFIER_LO],
    );
    let nullifier_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_authorization[public_instance::TRANSITION_NULLIFIER_LO + 1],
    );
    let nullifier_zero = gate.and(
        loader.ctx_mut().main(),
        nullifier_low_zero,
        nullifier_high_zero,
    );
    let terminal = gate.not(loader.ctx_mut().main(), nullifier_zero);
    let scalar_bindings = [
        (
            state_relation::public_instance::OPERATION,
            public_instance::OPERATION,
        ),
        (
            state_relation::public_instance::AMOUNT,
            public_instance::AMOUNT,
        ),
        (
            state_relation::public_instance::PROTOCOL_VERSION,
            public_instance::PROTOCOL_VERSION,
        ),
    ];
    for (candidate_index, authorization_index) in scalar_bindings {
        loader.ctx_mut().main().constrain_equal(
            &candidate[candidate_index].assigned(),
            &public_authorization[authorization_index],
        );
    }
    loader.ctx_mut().main().constrain_equal(
        &candidate[state_relation::public_instance::ASSET_SCALE].assigned(),
        &public_authorization[public_instance::ASSET_SCALE],
    );
    loader.ctx_mut().main().constrain_equal(
        &candidate[state_relation::public_instance::POLICY_EPOCH].assigned(),
        &public_authorization[public_instance::POLICY_EPOCH],
    );
    let always_digest_bindings = [
        (
            state_relation::public_instance::RELEASE_LO,
            public_instance::RELEASE_LO,
        ),
        (
            state_relation::public_instance::SUCCESSOR_SUITE_LO,
            public_instance::SUITE_LO,
        ),
        (
            state_relation::public_instance::SUCCESSOR_VK_LO,
            public_instance::VK_LO,
        ),
    ];
    for (candidate_offset, authorization_offset) in always_digest_bindings {
        for limb in 0..2 {
            loader.ctx_mut().main().constrain_equal(
                &candidate[candidate_offset + limb].assigned(),
                &public_authorization[authorization_offset + limb],
            );
        }
    }
    let digest_bindings = [
        (
            state_relation::public_instance::TRANSPORT_LO,
            public_instance::SEMANTIC_LO,
        ),
        (
            state_relation::public_instance::LIFECYCLE_LO,
            public_instance::LIFECYCLE_LO,
        ),
        (
            state_relation::public_instance::ASSET_INCARNATION_LO,
            public_instance::ASSET_INCARNATION_LO,
        ),
        (
            state_relation::public_instance::LIABILITY_POOL_LO,
            public_instance::LIABILITY_POOL_LO,
        ),
        (
            state_relation::public_instance::NETWORK_LO,
            public_instance::NETWORK_LO,
        ),
        (
            state_relation::public_instance::ASSET_LO,
            public_instance::ASSET_LO,
        ),
    ];
    for (candidate_offset, authorization_offset) in digest_bindings {
        for limb in 0..2 {
            loader.ctx_mut().main().constrain_equal(
                &candidate[candidate_offset + limb].assigned(),
                &public_authorization[authorization_offset + limb],
            );
        }
    }
    for limb in 0..2 {
        loader.ctx_mut().main().constrain_equal(
            &candidate[state_relation::public_instance::HARDWARE_PROFILE_LO + limb].assigned(),
            &public_authorization[public_instance::HARDWARE_PROFILE_LO + limb],
        );
    }

    let protocol_offset = match parity {
        KagemushaPastaParityV1::Eq => state_relation::public_instance::EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_relation::public_instance::EP_PROTOCOL_LO,
    };
    let expected_protocol =
        crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(candidate_protocol_digest);
    for (candidate_limb, expected) in candidate[protocol_offset..protocol_offset + 2]
        .iter()
        .zip(expected_protocol)
    {
        let constant = loader.ctx_mut().main().load_constant(expected);
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&candidate_limb.assigned(), &constant);
    }

    let mut ctx = loader.ctx_mut();
    let mut message = constant_bytes(CANDIDATE_BINDING_DOMAIN_V1);
    for (index, value) in candidate[..state_relation::PUBLIC_INSTANCE_COUNT]
        .iter()
        .enumerate()
    {
        if matches!(
            index,
            state_relation::public_instance::PREDECESSOR_STATE
                | state_relation::public_instance::SUCCESSOR_STATE
        ) {
            message.extend(constant_bytes(&[0_u8; 16]));
            continue;
        }
        let bits = PastaSha256BitV1::decompose(ctx.main(), &gate, *value.assigned(), 128);
        for byte_bits in bits.chunks_exact(8) {
            message.push(PastaSha256ByteV1::from_bits_le(
                ctx.main(),
                &gate,
                byte_bits,
            ));
        }
    }
    let digest = hash(ctx.main(), sha_jobs, message)?;
    let digest = digest_limbs_assigned(ctx.main(), &digest);
    for (actual, expected) in digest
        .into_iter()
        .zip(&public_authorization[public_instance::CANDIDATE_LO..][..2])
    {
        let difference = gate.sub(ctx.main(), actual, *expected);
        let selected = gate.mul(ctx.main(), terminal, difference);
        gate.assert_is_const(ctx.main(), &selected, &C::ScalarExt::ZERO);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::state_relation;
    #[cfg(feature = "zk-halo2-ipa")]
    use super::validate_candidate_guard_protocol_binding_v1;
    use super::{
        TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1,
        canonical_predecessor_conflict_nullifier_v1,
        canonical_terminal_authorization_candidate_digest_v1,
        canonical_terminal_send_output_binding_v1, validate_enabled_hardware_profiles_v1,
    };

    fn terminal_public() -> super::KagemushaTerminalAuthorizationPublicInputsV1 {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        super::KagemushaTerminalAuthorizationPublicInputsV1 {
            operation: super::KagemushaOperationV1::SendSplit,
            protocol_version: 1,
            suite_id: [1; 32],
            vk_digest: [2; 32],
            release_id: [3; 32],
            network_id: [4; 32],
            asset_id: [5; 32],
            asset_incarnation: super::AxtAssetIncarnationV1::try_from_bytes([1; 32])
                .expect("canonical incarnation"),
            asset_scale: 2,
            liability_pool_id: [6; 32],
            hardware_profile_id: [7; 32],
            policy_epoch: 1,
            lifecycle_binding_digest: [8; 32],
            semantic_digest: [9; 32],
            candidate_envelope_digest: [10; 32],
            commit_certificate_digest: [11; 32],
            transition_nullifier: [12; 32],
            request_digest: [13; 32],
            receiver_binding_digest: [14; 32],
            ciphertext_commitment: [15; 32],
            amount: 17,
            terminal_output_binding: [16; 32],
            eq_deferred_audit: [17; 32],
            ep_deferred_audit: [18; 32],
            eq_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fp::from(101)),
            ep_protocol_digest: crate::zk::kagemusha_v1_poseidon::encode(Fq::from(102)),
        }
    }

    #[test]
    fn terminal_public_requires_postcommit_send_and_preserves_redemption_padding() {
        let send = terminal_public();
        send.validate().expect("actual postcommit send projection");
        let mut missing_nullifier = send.clone();
        missing_nullifier.transition_nullifier = [0; 32];
        missing_nullifier.receiver_binding_digest = [0; 32];
        assert!(missing_nullifier.validate().is_err());
        let mut missing_receiver_binding = send.clone();
        missing_receiver_binding.receiver_binding_digest = [0; 32];
        assert!(missing_receiver_binding.validate().is_err());
        let mut missing_commit = send.clone();
        missing_commit.commit_certificate_digest = [0; 32];
        assert!(missing_commit.validate().is_err());
        let mut aliased_commit = send.clone();
        aliased_commit.commit_certificate_digest = send.candidate_envelope_digest;
        assert!(aliased_commit.validate().is_err());

        let mut redemption = send;
        redemption.operation = super::KagemushaOperationV1::RedeemSplit;
        redemption.request_digest = [0; 32];
        redemption.receiver_binding_digest = [0; 32];
        redemption.ciphertext_commitment = [0; 32];
        redemption
            .validate()
            .expect("canonical redemption projection");
        redemption.receiver_binding_digest = [14; 32];
        assert!(redemption.validate().is_err());
    }

    #[test]
    fn terminal_payload_digest_binds_output_and_ciphertext_without_proof_bytes() {
        let output = [1; 32];
        let ciphertext = [2; 32];
        let expected = super::hash_canonical_bytes_v1(
            b"iroha:kagemusha:v1:payment-body",
            &[output.as_slice(), ciphertext.as_slice()].concat(),
        );
        assert_eq!(
            expected,
            super::kagemusha_payment_body_digest_from_digests_v1(output, ciphertext)
        );
        for (output, ciphertext) in [([3; 32], ciphertext), (output, [3; 32])] {
            assert_ne!(
                expected,
                super::kagemusha_payment_body_digest_from_digests_v1(output, ciphertext)
            );
        }
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn terminal_public_circuit_accepts_direct_request_send_and_redeem() {
        use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
        use halo2_proofs::{dev::MockProver, halo2curves::pasta::Fp};

        for send in [true, false] {
            let mut public = terminal_public();
            if !send {
                public.operation = super::KagemushaOperationV1::RedeemSplit;
                public.request_digest = [0; 32];
                public.receiver_binding_digest = [0; 32];
                public.ciphertext_commitment = [0; 32];
            }
            let instances = public.public_prefix::<Fp>().expect("valid terminal prefix");
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(12)
                .use_lookup_bits(11)
                .use_instance_columns(1);
            let range = builder.range_chip();
            let cells = super::assign_public_prefix_v1(&mut builder, &range, &public)
                .expect("fixed terminal prefix");
            builder.assigned_instances = vec![cells];
            builder.calculate_params(Some(super::MINIMUM_UNUSABLE_ROWS));
            MockProver::run(12, &builder, vec![instances])
                .expect("terminal prefix circuit")
                .assert_satisfied();
        }
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn candidate_guard_protocol_binding_rejects_eq_substitution() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let eq = crate::zk::kagemusha_v1_poseidon::encode(Fp::from(0x71));
        let ep = crate::zk::kagemusha_v1_poseidon::encode(Fq::from(0x72));
        let mut column = vec![Fp::from(0); state_relation::PUBLIC_INSTANCE_COUNT];
        column[state_relation::public_instance::GUARD_EQ_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EQ_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<Fp>(eq));
        column[state_relation::public_instance::GUARD_EP_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EP_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<Fp>(ep));
        validate_candidate_guard_protocol_binding_v1(&[column.clone()], eq, ep)
            .expect("exact GuardBundle protocols");
        column[state_relation::public_instance::GUARD_EQ_PROTOCOL_LO] = Fp::from(0);
        assert!(validate_candidate_guard_protocol_binding_v1(&[column], eq, ep).is_err());
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn candidate_guard_protocol_binding_rejects_ep_substitution() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let eq = crate::zk::kagemusha_v1_poseidon::encode(Fp::from(0x71));
        let ep = crate::zk::kagemusha_v1_poseidon::encode(Fq::from(0x72));
        let mut column = vec![Fq::from(0); state_relation::PUBLIC_INSTANCE_COUNT];
        column[state_relation::public_instance::GUARD_EQ_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EQ_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<Fq>(eq));
        column[state_relation::public_instance::GUARD_EP_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EP_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<Fq>(ep));
        validate_candidate_guard_protocol_binding_v1(&[column.clone()], eq, ep)
            .expect("exact GuardBundle protocols");
        column[state_relation::public_instance::GUARD_EP_PROTOCOL_LO] = Fq::from(0);
        assert!(validate_candidate_guard_protocol_binding_v1(&[column], eq, ep).is_err());
    }

    #[test]
    fn candidate_binding_is_equal_across_parities_and_rejects_semantic_mutation() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let mut eq = (0..state_relation::PUBLIC_INSTANCE_COUNT)
            .map(|value| Fp::from(value as u64 + 1))
            .collect::<Vec<_>>();
        let mut ep = (0..state_relation::PUBLIC_INSTANCE_COUNT)
            .map(|value| Fq::from(value as u64 + 1))
            .collect::<Vec<_>>();
        eq[state_relation::public_instance::PREDECESSOR_STATE] = Fp::from(0xaaaa);
        eq[state_relation::public_instance::SUCCESSOR_STATE] = Fp::from(0xbbbb);
        ep[state_relation::public_instance::PREDECESSOR_STATE] = Fq::from(0xcccc);
        ep[state_relation::public_instance::SUCCESSOR_STATE] = Fq::from(0xdddd);

        let expected = canonical_terminal_authorization_candidate_digest_v1(&[eq.clone()])
            .expect("canonical Eq candidate");
        assert_eq!(
            expected,
            canonical_terminal_authorization_candidate_digest_v1(&[ep.clone()])
                .expect("canonical Ep candidate")
        );

        for index in 0..state_relation::PUBLIC_INSTANCE_COUNT {
            if matches!(
                index,
                state_relation::public_instance::PREDECESSOR_STATE
                    | state_relation::public_instance::SUCCESSOR_STATE
            ) {
                continue;
            }
            let mut mutated_eq = eq.clone();
            let mut mutated_ep = ep.clone();
            mutated_eq[index] += Fp::from(1);
            mutated_ep[index] += Fq::from(1);
            let mutated_digest =
                canonical_terminal_authorization_candidate_digest_v1(&[mutated_eq])
                    .expect("mutated Eq candidate");
            assert_ne!(mutated_digest, expected, "semantic cell {index}");
            assert_eq!(
                mutated_digest,
                canonical_terminal_authorization_candidate_digest_v1(&[mutated_ep])
                    .expect("mutated Ep candidate"),
                "semantic cell {index} normalizes across parities"
            );
        }
    }

    #[test]
    fn candidate_binding_requires_one_exact_semantic_column() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let semantic_count = state_relation::PUBLIC_INSTANCE_COUNT;
        let history_limb_count = super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 / 16;
        let eq = vec![Fp::from(0); semantic_count];
        let ep = vec![Fq::from(0); semantic_count];
        assert_eq!(
            canonical_terminal_authorization_candidate_digest_v1(&[eq.clone()])
                .expect("exact Eq semantic column"),
            canonical_terminal_authorization_candidate_digest_v1(&[ep.clone()])
                .expect("exact Ep semantic column")
        );

        // Representative malformed widths remain literal regression cases. A complete recursive
        // column is also invalid here: this helper consumes only semantic state cells.
        for length in [
            0,
            81,
            115,
            semantic_count - 1,
            semantic_count + 1,
            semantic_count + history_limb_count,
        ] {
            assert!(
                canonical_terminal_authorization_candidate_digest_v1(&[vec![Fp::from(0); length]])
                    .is_err(),
                "reject Eq column length {length}"
            );
            assert!(
                canonical_terminal_authorization_candidate_digest_v1(&[vec![Fq::from(0); length]])
                    .is_err(),
                "reject Ep column length {length}"
            );
        }
        assert!(canonical_terminal_authorization_candidate_digest_v1::<Fp>(&[]).is_err());
        assert!(canonical_terminal_authorization_candidate_digest_v1::<Fq>(&[]).is_err());
        assert!(canonical_terminal_authorization_candidate_digest_v1(&[eq.clone(), eq]).is_err());
        assert!(canonical_terminal_authorization_candidate_digest_v1(&[ep.clone(), ep]).is_err());
    }

    #[test]
    fn predecessor_conflict_nullifier_is_successor_independent() {
        let prepared = [0x51; 32];
        let terminal = canonical_predecessor_conflict_nullifier_v1(prepared);
        let repeated = canonical_predecessor_conflict_nullifier_v1(prepared);
        assert_ne!(terminal, [0; 32]);
        assert_eq!(terminal, repeated);
        assert_ne!(
            terminal,
            canonical_predecessor_conflict_nullifier_v1([0x52; 32])
        );
    }

    #[test]
    fn terminal_send_output_binding_commits_every_exact_component() {
        let values = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]];
        let bind = |[credit, key, lane, prepared, output, claims]: [[u8; 32]; 6]| {
            canonical_terminal_send_output_binding_v1(credit, key, lane, prepared, output, claims)
        };
        let baseline = bind(values);
        for index in 0..values.len() {
            let mut altered = values;
            altered[index][0] ^= 1;
            assert_ne!(baseline, bind(altered), "binding component {index}");
        }
        assert_eq!(
            super::TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1.len() + 2 + 6 * 32,
            242
        );
    }

    #[test]
    fn enabled_profiles_require_sorted_nonzero_prefix() {
        let mut profiles = [[0_u8; 32]; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
        profiles[0][31] = 1;
        profiles[1][31] = 2;
        validate_enabled_hardware_profiles_v1(&profiles).expect("canonical profile table");

        profiles.swap(0, 1);
        assert!(validate_enabled_hardware_profiles_v1(&profiles).is_err());
    }

    #[test]
    fn enabled_profiles_reject_holes_and_duplicates() {
        let mut hole = [[0_u8; 32]; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
        hole[0][31] = 1;
        hole[2][31] = 2;
        assert!(validate_enabled_hardware_profiles_v1(&hole).is_err());

        let mut duplicate = [[0_u8; 32]; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1];
        duplicate[0][31] = 1;
        duplicate[1][31] = 1;
        assert!(validate_enabled_hardware_profiles_v1(&duplicate).is_err());
    }
}
