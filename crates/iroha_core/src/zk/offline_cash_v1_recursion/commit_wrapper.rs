//! Final zero-knowledge commit wrapper for Offline Cash V1.
//!
//! A state proof is generated before hardware commit and therefore cannot contain terminal
//! evidence without creating a digest cycle. This module keeps the phases explicit: the private
//! candidate relation binds request/ticket/outbox reservations, hardware commits that exact
//! candidate once, and this wrapper recursively verifies both the candidate and a postcommit
//! terminal-guard proof. Only an unlinkable projection is exposed by the final proof.

use iroha_data_model::nexus::AxtAssetIncarnationV1;
use iroha_data_model::offline::{
    OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1, OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
    OfflineCashAcceptanceIntentAuthorizationStatementV1,
    OfflineCashAcceptanceIntentAuthorizationV1, OfflineCashAcceptanceIntentV1,
    OfflineCashAcceptanceTicketV1, OfflineCashCommitCertificateV1, OfflineCashCommitEvidenceV1,
    OfflineCashHardwareCredentialV1, OfflineCashHardwareProfileV1, OfflineCashLifecycleBindingV1,
    OfflineCashNoCommitClosureStatementV1, OfflineCashOperationKindV1,
    OfflineCashOutboxReservationV1, OfflineCashPaymentRequestV1,
    offline_cash_asset_identity_digest_v1,
};
use sha2::{Digest as _, Sha256};

use super::{DigestV1, OfflineCashOperationV1};
use crate::zk::{offline_cash_v1_poseidon::decode, offline_cash_v1_state::OfflineCashStateV1};

#[cfg(feature = "zk-halo2-ipa")]
use ff::Field as _;
#[cfg(feature = "zk-halo2-ipa")]
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt, fe_to_biguint},
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
    OfflineCashEpAccumulatorV1, OfflineCashEpFoldProofV1, OfflineCashEqAccumulatorV1,
    OfflineCashEqFoldProofV1, OfflineCashGuardBundleRelationWitnessV1, OfflineCashPastaParityV1,
    deferred_parent::{
        DeferredLoader, DeferredScalar, OfflineCashDeferredParentOutputV1, accumulator_limb_count,
        bind_accumulator_limbs, constrain_reciprocal_parent_pass_v1, deferred_field_chips_v1,
        deferred_loader_v1, finalize_deferred_audit_plan_v1, load_native_accumulator,
        native_parent_protocol_digest_v1, verify_fold, verify_ordinary_proof_v1,
    },
    guard_bundle::{
        GUARD_EP_AUDIT_OFFSET_V1, GUARD_EQ_AUDIT_OFFSET_V1, GUARD_HISTORY_OFFSET_V1,
        GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1, OfflineCashAssignedGuardBundleV1, assign_bytes,
        constant_bytes, constrain_guard_bundle_semantics_v1, digest_limbs_assigned, hash,
    },
    state_relation,
};
#[cfg(feature = "zk-halo2-ipa")]
use crate::zk::{
    offline_cash_v1_poseidon::OfflineCashPoseidonFieldV1,
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};

const COMMIT_CERTIFICATE_DIGEST_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:commit-certificate";
const COMMIT_CERTIFICATE_ID_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:commit-certificate-id";
const ACCEPTANCE_INTENT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:acceptance-intent";
const ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:acceptance-intent-authorization-statement";
const OUTBOX_RESERVATION_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:outbox-reservation";
const PRECOMMIT_BINDING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:precommit-binding\0";
const PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:prepared-one-use-authorization\0";
const SENDER_ONE_TIME_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:sender-one-time-authorization\0";
const SENDER_TERMINAL_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:sender-terminal-authorization\0";
const COMMIT_EVIDENCE_OPENING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:commit-evidence-opening\0";
const TERMINAL_COMMIT_BINDING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:terminal-commit-binding\0";
pub(crate) const TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:terminal-send-output-binding\0";
const NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:no-commit-closure-statement";
const NO_COMMIT_CANCELLATION_SUCCESSOR_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:no-commit-cancellation-successor\0";
const PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:predecessor-conflict-nullifier\0";
const NO_COMMIT_RECOVERY_ID_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:no-commit-recovery-id\0";
const NO_COMMIT_HARDWARE_BINDING_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:no-commit-hardware-binding\0";
const NO_COMMIT_INCARNATION_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:no-commit-incarnation\0";

const ACCEPTANCE_INTENT_CANONICAL_BYTES_V1: usize = 114;
const ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_CANONICAL_BYTES_V1: usize = 244;
const OUTBOX_RESERVATION_CANONICAL_BYTES_V1: usize = 56;
const COMMIT_CERTIFICATE_ID_CANONICAL_BYTES_V1: usize = 238;
const COMMIT_CERTIFICATE_CANONICAL_BYTES_V1: usize = 270;
const NO_COMMIT_CLOSURE_STATEMENT_CANONICAL_BYTES_V1: usize = 498;
/// Fixed release-pinned hardware-profile table width.
pub(crate) const COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1: usize = 64;

/// Number of public field elements in one final wrapper parity, including history.
pub(crate) const COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1: usize = 81;
/// Number of non-history public field elements in one final wrapper parity.
pub(crate) const COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1: usize = 47;

/// Public-instance offsets shared by both final wrapper parities.
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
    pub(crate) const ACCEPTANCE_TICKET_LO: usize = 32;
    pub(crate) const CIPHERTEXT_LO: usize = 34;
    pub(crate) const AMOUNT: usize = 36;
    pub(crate) const OUTPUT_BINDING_LO: usize = 37;
    pub(crate) const EQ_DEFERRED_AUDIT_LO: usize = 39;
    pub(crate) const EP_DEFERRED_AUDIT_LO: usize = 41;
    pub(crate) const EQ_PROTOCOL_LO: usize = 43;
    pub(crate) const EP_PROTOCOL_LO: usize = 45;
    pub(crate) const HISTORY_START: usize = 47;
}

/// Unlinkable public values shared by both final wrapper parities.
///
/// Network and asset digests are normalized solely so the circuit can bind the private aggregate
/// state to the typed lifecycle. They reveal no state head, lane, credential, epoch, key,
/// sequence, journal, or replay-root value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashCommitWrapperPublicInputsV1 {
    pub(crate) operation: OfflineCashOperationV1,
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
    pub(crate) acceptance_ticket_digest: DigestV1,
    pub(crate) ciphertext_commitment: DigestV1,
    pub(crate) amount: u128,
    /// Operation-specific terminal output binding. For a send this commits the receiver credit,
    /// recipient lane, request, ticket, ciphertext, and amount; for a redemption this is the
    /// terminal redemption commitment.
    pub(crate) terminal_output_binding: DigestV1,
    pub(crate) eq_deferred_audit: DigestV1,
    pub(crate) ep_deferred_audit: DigestV1,
    pub(crate) eq_protocol_digest: DigestV1,
    pub(crate) ep_protocol_digest: DigestV1,
}

impl OfflineCashCommitWrapperPublicInputsV1 {
    /// Construct the normalized circuit projection from a validated lifecycle.
    pub(crate) fn from_lifecycle(
        lifecycle: &OfflineCashLifecycleBindingV1,
        semantic_digest: DigestV1,
        candidate_envelope_digest: DigestV1,
        commit_certificate_digest: DigestV1,
        transition_nullifier: DigestV1,
        request_digest: DigestV1,
        acceptance_ticket_digest: DigestV1,
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
            asset_id: offline_cash_asset_identity_digest_v1(&lifecycle.asset)
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
            acceptance_ticket_digest,
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

    /// Construct the unlinkable pre-ticket authorization projection.
    ///
    /// The terminal-only columns remain canonical zeroes. The authenticated wrapper verifying
    /// key fixes the enabled-profile set, while the private circuit branch binds the hidden
    /// sender profile and one-use predecessor reservation to `semantic_digest`.
    pub(crate) fn from_acceptance_intent_authorization(
        request: &OfflineCashPaymentRequestV1,
        statement: &OfflineCashAcceptanceIntentAuthorizationStatementV1,
        guard_eq_credential_audit: DigestV1,
        guard_ep_credential_audit: DigestV1,
        eq_deferred_audit: DigestV1,
        ep_deferred_audit: DigestV1,
        eq_protocol_digest: DigestV1,
        ep_protocol_digest: DigestV1,
    ) -> Result<Self, String> {
        statement
            .validate_shape_against(request)
            .map_err(|error| error.to_string())?;
        let semantic_digest =
            canonical_acceptance_intent_authorization_statement_digest_v1(statement, request)?;
        let value = Self {
            operation: OfflineCashOperationV1::SendSplit,
            protocol_version: statement.version,
            suite_id: statement.suite_id,
            vk_digest: statement.vk_digest,
            release_id: statement.release_id,
            network_id: *request.network_id.as_bytes(),
            asset_id: offline_cash_asset_identity_digest_v1(&request.asset)
                .map_err(|error| error.to_string())?,
            asset_incarnation: request.asset_incarnation,
            asset_scale: request.scale,
            liability_pool_id: request.liability_pool_id,
            hardware_profile_id: [0; 32],
            policy_epoch: 0,
            lifecycle_binding_digest: semantic_digest,
            semantic_digest,
            candidate_envelope_digest: guard_eq_credential_audit,
            commit_certificate_digest: [0; 32],
            transition_nullifier: [0; 32],
            request_digest: request
                .canonical_digest()
                .map_err(|error| error.to_string())?,
            acceptance_ticket_digest: guard_ep_credential_audit,
            ciphertext_commitment: statement.artifact_manifest_digest,
            amount: statement.intent.exact_amount,
            terminal_output_binding: [0; 32],
            eq_deferred_audit,
            ep_deferred_audit,
            eq_protocol_digest,
            ep_protocol_digest,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct the unlinkable sender-hardware no-commit closure projection.
    pub(crate) fn from_no_commit_closure(
        statement: &OfflineCashNoCommitClosureStatementV1,
        guard_eq_credential_audit: DigestV1,
        guard_ep_credential_audit: DigestV1,
        eq_deferred_audit: DigestV1,
        ep_deferred_audit: DigestV1,
        eq_protocol_digest: DigestV1,
        ep_protocol_digest: DigestV1,
    ) -> Result<Self, String> {
        statement
            .validate_shape()
            .map_err(|error| error.to_string())?;
        if guard_eq_credential_audit == [0; 32]
            || guard_ep_credential_audit == [0; 32]
            || guard_eq_credential_audit == guard_ep_credential_audit
        {
            return Err("no-commit Guard credential audits are noncanonical".to_owned());
        }
        let semantic_digest = statement
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let value = Self {
            operation: OfflineCashOperationV1::SendSplit,
            protocol_version: statement.version,
            suite_id: statement.suite_id,
            vk_digest: statement.vk_digest,
            release_id: statement.release_id,
            network_id: statement.request_id,
            asset_id: statement.acceptance_ticket_id,
            asset_incarnation: no_commit_incarnation_v1(statement.recovery_id)?,
            asset_scale: 0,
            liability_pool_id: statement.intent_digest,
            hardware_profile_id: statement.sender_hardware_binding_commitment,
            policy_epoch: 1,
            lifecycle_binding_digest: statement.artifact_manifest_digest,
            semantic_digest,
            candidate_envelope_digest: guard_eq_credential_audit,
            commit_certificate_digest: [0; 32],
            transition_nullifier: statement.cancellation_nullifier,
            request_digest: statement.request_digest,
            acceptance_ticket_digest: guard_ep_credential_audit,
            ciphertext_commitment: statement.sender_one_time_commitment,
            amount: statement.exact_amount,
            terminal_output_binding: statement.equivalent_delivery_slot_commitment,
            eq_deferred_audit,
            ep_deferred_audit,
            eq_protocol_digest,
            ep_protocol_digest,
        };
        value.validate()?;
        Ok(value)
    }

    /// Whether these columns encode the pre-ticket authorization branch.
    pub(crate) fn is_acceptance_intent_authorization(&self) -> bool {
        self.commit_certificate_digest == [0; 32] && self.transition_nullifier == [0; 32]
    }

    /// Whether these columns encode the sender-hardware no-commit closure branch.
    pub(crate) fn is_no_commit_closure(&self) -> bool {
        self.commit_certificate_digest == [0; 32] && self.transition_nullifier != [0; 32]
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        let authorization = self.is_acceptance_intent_authorization();
        let no_commit_closure = self.is_no_commit_closure();
        if !matches!(
            self.operation,
            OfflineCashOperationV1::SendSplit | OfflineCashOperationV1::RedeemSplit
        ) || self.protocol_version != 1
            || self.asset_incarnation.validate().is_err()
            || (!authorization && self.policy_epoch == 0)
            || self.amount == 0
        {
            return Err(
                "commit wrapper is only defined for positive send/redemption outputs".to_owned(),
            );
        }
        for (name, digest) in [
            ("suite", self.suite_id),
            ("verifier key", self.vk_digest),
            ("release", self.release_id),
            ("network", self.network_id),
            ("asset", self.asset_id),
            ("liability pool", self.liability_pool_id),
            ("lifecycle", self.lifecycle_binding_digest),
            ("semantic", self.semantic_digest),
            ("Eq deferred audit", self.eq_deferred_audit),
            ("Ep deferred audit", self.ep_deferred_audit),
            ("Eq protocol", self.eq_protocol_digest),
            ("Ep protocol", self.ep_protocol_digest),
        ] {
            if digest == [0; 32] {
                return Err(format!("Offline Cash commit wrapper {name} digest is zero"));
            }
        }
        if self.eq_deferred_audit == self.ep_deferred_audit
            || self.eq_protocol_digest == self.ep_protocol_digest
            || decode::<halo2_proofs::halo2curves::pasta::Fp>(self.eq_protocol_digest).is_none()
            || decode::<halo2_proofs::halo2curves::pasta::Fq>(self.ep_protocol_digest).is_none()
        {
            return Err("commit wrapper protocol roles are noncanonical".to_owned());
        }
        if authorization {
            if self.operation != OfflineCashOperationV1::SendSplit
                || self.hardware_profile_id != [0; 32]
                || self.policy_epoch != 0
                || self.lifecycle_binding_digest != self.semantic_digest
                || self.candidate_envelope_digest == [0; 32]
                || self.transition_nullifier != [0; 32]
                || self.request_digest == [0; 32]
                || self.acceptance_ticket_digest == [0; 32]
                || self.candidate_envelope_digest == self.acceptance_ticket_digest
                || self.ciphertext_commitment == [0; 32]
                || self.terminal_output_binding != [0; 32]
            {
                return Err("invalid acceptance-intent authorization projection".to_owned());
            }
            return Ok(());
        }
        if no_commit_closure {
            if self.operation != OfflineCashOperationV1::SendSplit
                || self.hardware_profile_id == [0; 32]
                || self.policy_epoch != 1
                || self.candidate_envelope_digest == [0; 32]
                || self.commit_certificate_digest != [0; 32]
                || self.transition_nullifier == [0; 32]
                || self.request_digest == [0; 32]
                || self.acceptance_ticket_digest == [0; 32]
                || self.candidate_envelope_digest == self.acceptance_ticket_digest
                || self.ciphertext_commitment == [0; 32]
                || self.terminal_output_binding == [0; 32]
            {
                return Err("invalid no-commit closure projection".to_owned());
            }
            return Ok(());
        }
        for (name, digest) in [
            ("hardware profile", self.hardware_profile_id),
            ("candidate envelope", self.candidate_envelope_digest),
            ("commit certificate", self.commit_certificate_digest),
            ("transition nullifier", self.transition_nullifier),
        ] {
            if digest == [0; 32] {
                return Err(format!("Offline Cash commit wrapper {name} digest is zero"));
            }
        }
        let payment_values = [
            self.request_digest,
            self.acceptance_ticket_digest,
            self.ciphertext_commitment,
        ];
        match self.operation {
            OfflineCashOperationV1::SendSplit
                if payment_values.iter().all(|value| *value != [0; 32])
                    && self.terminal_output_binding != [0; 32] => {}
            OfflineCashOperationV1::RedeemSplit
                if payment_values.iter().all(|value| *value == [0; 32])
                    && self.terminal_output_binding != [0; 32] => {}
            _ => return Err("invalid operation-specific commit wrapper projection".to_owned()),
        }
        Ok(())
    }

    #[cfg(feature = "zk-halo2-ipa")]
    pub(crate) fn public_prefix<
        F: crate::zk::offline_cash_v1_poseidon::OfflineCashPoseidonFieldV1,
    >(
        &self,
    ) -> Result<Vec<F>, String> {
        use crate::zk::offline_cash_v1_poseidon::{digest_limbs, from_u128};

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
            self.acceptance_ticket_digest,
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
        if output.len() != COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1 {
            return Err("commit wrapper public prefix has wrong fixed shape".to_owned());
        }
        Ok(output)
    }
}

/// Complete private values checked by the final wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashCommitEvidenceOpeningV1 {
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

impl OfflineCashCommitEvidenceOpeningV1 {
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

/// Complete private values checked by the final wrapper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashCommitWrapperPrivateTransitionV1 {
    pub(crate) lifecycle: OfflineCashLifecycleBindingV1,
    pub(crate) predecessor: OfflineCashStateV1,
    pub(crate) successor: OfflineCashStateV1,
    /// Signed request, present only for `SendSplit`.
    pub(crate) request: Option<OfflineCashPaymentRequestV1>,
    /// Sender one-use intent, present only for `SendSplit`.
    pub(crate) acceptance_intent: Option<OfflineCashAcceptanceIntentV1>,
    /// Receiver-hardware ticket, present only for `SendSplit`.
    pub(crate) acceptance_ticket: Option<OfflineCashAcceptanceTicketV1>,
    pub(crate) outbox_reservation: OfflineCashOutboxReservationV1,
    pub(crate) commit_certificate: OfflineCashCommitCertificateV1,
    /// Private opening of the opaque public time-or-lease evidence commitment.
    pub(crate) commit_evidence_opening: OfflineCashCommitEvidenceOpeningV1,
    /// Hardware-only one-use transition authorization consumed by this exact predecessor.
    pub(crate) one_use_hardware_authorization: DigestV1,
    /// Fresh private opening of `sender_one_time_commitment`.
    pub(crate) sender_one_time_opening: DigestV1,
    /// Digest of the byte-identical terminal payment or redemption envelope.
    pub(crate) terminal_envelope_digest: DigestV1,
    pub(crate) journal_revision_before: u128,
    pub(crate) journal_revision_after: u128,
    pub(crate) authorization_counter_before: u128,
    pub(crate) authorization_counter_after: u128,
    pub(crate) hardware_profile: OfflineCashHardwareProfileV1,
    pub(crate) hardware_credential: OfflineCashHardwareCredentialV1,
}

/// Private request and public semantic statement for the pre-ticket authorization branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashCommitWrapperIntentAuthorizationPrivateV1 {
    pub(crate) request: OfflineCashPaymentRequestV1,
    pub(crate) statement: OfflineCashAcceptanceIntentAuthorizationStatementV1,
}

/// Complete private opening for the sender-hardware no-commit closure branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfflineCashCommitWrapperNoCommitClosurePrivateV1 {
    pub(crate) statement: OfflineCashNoCommitClosureStatementV1,
    /// Original proof-bearing sender authorization whose exact envelope digest is cancelled.
    pub(crate) intent_authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    /// Hardware-private nonce making the lane-scoped recovery identity unique.
    pub(crate) hardware_recovery_nonce: DigestV1,
}

impl OfflineCashCommitWrapperIntentAuthorizationPrivateV1 {
    pub(crate) fn validate_against(
        &self,
        public: &OfflineCashCommitWrapperPublicInputsV1,
    ) -> Result<(), String> {
        if !public.is_acceptance_intent_authorization() {
            return Err("intent authorization private witness used for terminal proof".to_owned());
        }
        self.statement
            .validate_shape_against(&self.request)
            .map_err(|error| error.to_string())?;
        let expected =
            OfflineCashCommitWrapperPublicInputsV1::from_acceptance_intent_authorization(
                &self.request,
                &self.statement,
                public.candidate_envelope_digest,
                public.acceptance_ticket_digest,
                public.eq_deferred_audit,
                public.ep_deferred_audit,
                public.eq_protocol_digest,
                public.ep_protocol_digest,
            )?;
        if &expected != public {
            return Err("intent authorization public/private binding mismatch".to_owned());
        }
        Ok(())
    }
}

impl OfflineCashCommitWrapperPrivateTransitionV1 {
    pub(crate) fn validate_against(
        &self,
        public: &OfflineCashCommitWrapperPublicInputsV1,
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
            .map_err(|error| format!("invalid wrapper hardware credential: {error}"))?;
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
            || offline_cash_asset_identity_digest_v1(&self.lifecycle.asset)
                .map_err(|error| error.to_string())?
                != public.asset_id
            || self.lifecycle.asset_incarnation != public.asset_incarnation
            || self.lifecycle.scale != public.asset_scale
            || self.lifecycle.liability_pool_id != public.liability_pool_id
            || self.lifecycle.hardware_profile_id != public.hardware_profile_id
            || self.lifecycle.policy_epoch != public.policy_epoch
            || self.one_use_hardware_authorization == [0; 32]
            || self.terminal_envelope_digest == [0; 32]
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
                "commit wrapper lifecycle/candidate/certificate binding mismatch".to_owned(),
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
            return Err("commit wrapper terminal transition is not exact-next".to_owned());
        }
        let is_send = public.operation == OfflineCashOperationV1::SendSplit;
        let prepared_authorization = canonical_prepared_one_use_authorization_digest_v1(
            public.operation,
            self.one_use_hardware_authorization,
            &self.predecessor,
            self.journal_revision_before,
            self.authorization_counter_before,
        );
        let intent_digest = match (
            self.request.as_ref(),
            self.acceptance_intent.as_ref(),
            self.acceptance_ticket.as_ref(),
        ) {
            (Some(request), Some(intent), Some(ticket)) if is_send => {
                request
                    .validate_shape()
                    .map_err(|error| error.to_string())?;
                intent
                    .validate_shape_against(request)
                    .map_err(|error| error.to_string())?;
                ticket
                    .validate_shape_against(request, intent)
                    .map_err(|error| error.to_string())?;
                let request_digest = request
                    .canonical_digest()
                    .map_err(|error| error.to_string())?;
                let intent_digest = canonical_acceptance_intent_digest_v1(intent)?;
                let ticket_digest = ticket
                    .canonical_digest_against(request, intent)
                    .map_err(|error| error.to_string())?;
                let expected_sender_commitment = canonical_sender_one_time_commitment_v1(
                    self.sender_one_time_opening,
                    prepared_authorization,
                    request_digest,
                    intent.intent_id,
                    public.amount,
                );
                if self.sender_one_time_opening == [0; 32]
                    || intent.sender_one_time_commitment != expected_sender_commitment
                    || intent.exact_amount != public.amount
                    || ticket.exact_amount != public.amount
                    || request_digest != public.request_digest
                    || ticket_digest != public.acceptance_ticket_digest
                    || ticket.intent_digest != intent_digest
                {
                    return Err("commit wrapper sender intent/ticket mismatch".to_owned());
                }
                intent_digest
            }
            (None, None, None) if !is_send && self.sender_one_time_opening == [0; 32] => [0; 32],
            _ => {
                return Err(
                    "commit wrapper operation-specific private request shape mismatch".to_owned(),
                );
            }
        };
        if is_send {
            let request = self
                .request
                .as_ref()
                .ok_or_else(|| "commit wrapper sender request is absent".to_owned())?;
            let expected_output_binding = canonical_terminal_send_output_binding_v1(
                self.lifecycle.credit_id,
                request.hardware_credential.lane_commitment,
                public.request_digest,
                public.acceptance_ticket_digest,
                public.ciphertext_commitment,
                public.amount,
            );
            if expected_output_binding != public.terminal_output_binding {
                return Err("commit wrapper terminal send-output binding mismatch".to_owned());
            }
        }
        let expected_precommit = canonical_precommit_binding_digest_v1(
            public.lifecycle_binding_digest,
            public.request_digest,
            intent_digest,
            public.acceptance_ticket_digest,
            public.amount,
            reservation_commitment,
            prepared_authorization,
        );
        if expected_precommit == [0; 32] {
            return Err("commit wrapper precommit binding is zero".to_owned());
        }
        if operation_from_wire_v1(self.outbox_reservation.operation_kind) != public.operation
            || self.outbox_reservation.issued_at_ms >= self.outbox_reservation.expires_at_ms
        {
            return Err("commit wrapper outbox reservation mismatch".to_owned());
        }
        let (request_issued, request_expires, ticket_issued, ticket_expires) = self
            .request
            .as_ref()
            .zip(self.acceptance_ticket.as_ref())
            .map_or((0, 0, 0, 0), |(request, ticket)| {
                (
                    request.issued_at_ms,
                    request.expires_at_ms,
                    ticket.issued_at_ms,
                    ticket.expires_at_ms,
                )
            });
        let valid_deadline = match self.commit_evidence_opening.kind() {
            0 => {
                let committed = self.commit_evidence_opening.trusted_commit_time_ms;
                committed >= self.outbox_reservation.issued_at_ms
                    && committed < self.outbox_reservation.expires_at_ms
                    && (!is_send
                        || (committed >= request_issued
                            && committed < request_expires
                            && committed >= ticket_issued
                            && committed < ticket_expires))
            }
            1 => {
                let lease_start = self.commit_evidence_opening.lease_valid_from_ms;
                let lease_end = self.commit_evidence_opening.lease_expires_at_ms;
                lease_start >= self.outbox_reservation.issued_at_ms
                    && lease_end <= self.outbox_reservation.expires_at_ms
                    && (!is_send
                        || (lease_start >= request_issued
                            && lease_end <= request_expires
                            && lease_start >= ticket_issued
                            && lease_end <= ticket_expires))
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
            return Err("commit wrapper private aggregate context mismatch".to_owned());
        }
        let expected_predecessor_balance = self
            .successor
            .balance
            .checked_add(public.amount)
            .ok_or_else(|| "outbound balance overflow".to_owned())?;
        if expected_predecessor_balance != self.predecessor.balance {
            return Err("commit wrapper does not conserve outbound value".to_owned());
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
            return Err("commit wrapper credential/private-state binding mismatch".to_owned());
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

fn no_commit_incarnation_v1(recovery_id: DigestV1) -> Result<AxtAssetIncarnationV1, String> {
    let mut bytes = hash_fixed_v1(
        NO_COMMIT_INCARNATION_DOMAIN_V1,
        &[&1_u16.to_le_bytes(), &recovery_id],
    );
    bytes[31] |= 1;
    AxtAssetIncarnationV1::try_from_bytes(bytes).map_err(|error| error.to_string())
}

pub(super) fn canonical_no_commit_cancellation_successor_v1(
    prepared_authorization: DigestV1,
    recovery_id: DigestV1,
    intent_authorization_digest: DigestV1,
    ticket_digest: DigestV1,
    equivalent_delivery_slot_commitment: DigestV1,
    journal_revision_after: u128,
    authorization_counter_after: u128,
) -> DigestV1 {
    hash_fixed_v1(
        NO_COMMIT_CANCELLATION_SUCCESSOR_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &prepared_authorization,
            &recovery_id,
            &intent_authorization_digest,
            &ticket_digest,
            &equivalent_delivery_slot_commitment,
            &journal_revision_after.to_le_bytes(),
            &authorization_counter_after.to_le_bytes(),
        ],
    )
}

pub(super) fn canonical_no_commit_recovery_id_v1(
    prepared_authorization: DigestV1,
    request_digest: DigestV1,
    ticket_digest: DigestV1,
    receiver_lane_commitment: DigestV1,
    hardware_recovery_nonce: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        NO_COMMIT_RECOVERY_ID_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &prepared_authorization,
            &request_digest,
            &ticket_digest,
            &receiver_lane_commitment,
            &hardware_recovery_nonce,
        ],
    )
}

pub(super) fn canonical_predecessor_conflict_nullifier_v1(
    prepared_authorization: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1,
        &[&1_u16.to_le_bytes(), &prepared_authorization],
    )
}

pub(super) fn canonical_no_commit_hardware_binding_v1(
    guard: &super::OfflineCashNormalizedGuardStatementV1,
) -> DigestV1 {
    hash_fixed_v1(
        NO_COMMIT_HARDWARE_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &guard.hardware_profile_id,
            &guard.policy_epoch.to_le_bytes(),
            &guard.lane_id,
            &guard.predecessor_hardware_epoch_generation.to_le_bytes(),
            &guard.predecessor_hardware_epoch_id,
            &guard.predecessor_key_reference,
            &guard.predecessor_hardware_policy_id,
        ],
    )
}

fn canonical_no_commit_closure_statement_bytes_v1(
    statement: &OfflineCashNoCommitClosureStatementV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(NO_COMMIT_CLOSURE_STATEMENT_CANONICAL_BYTES_V1);
    bytes.extend_from_slice(&statement.version.to_le_bytes());
    for digest in [
        statement.release_id,
        statement.suite_id,
        statement.vk_digest,
        statement.artifact_manifest_digest,
        statement.sender_hardware_binding_commitment,
        statement.request_id,
        statement.request_digest,
        statement.acceptance_ticket_id,
        statement.ticket_digest,
        statement.intent_authorization_digest,
        statement.intent_digest,
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(&statement.exact_amount.to_le_bytes());
    for digest in [
        statement.sender_one_time_commitment,
        statement.recovery_id,
        statement.cancellation_nullifier,
        statement.equivalent_delivery_slot_commitment,
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes
}

fn canonical_no_commit_closure_statement_digest_v1(
    statement: &OfflineCashNoCommitClosureStatementV1,
) -> Result<DigestV1, String> {
    statement
        .validate_shape()
        .map_err(|error| error.to_string())?;
    let bytes = canonical_no_commit_closure_statement_bytes_v1(statement);
    if bytes.len() != NO_COMMIT_CLOSURE_STATEMENT_CANONICAL_BYTES_V1 {
        return Err("no-commit closure statement canonical layout drift".to_owned());
    }
    let digest = hash_canonical_bytes_v1(NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN_V1, &bytes);
    if digest
        != statement
            .canonical_digest()
            .map_err(|error| error.to_string())?
    {
        return Err("no-commit closure statement digest drift".to_owned());
    }
    Ok(digest)
}

/// Return the terminal send-output binding consumed by a receiver fold.
///
/// The sender wrapper computes this value from its recursively verified state-candidate cells.
/// A receiver recomputes it from the accepted payment and its own lane before admitting value,
/// preventing one terminal proof from being replayed under a substituted credit identity or lane.
pub(crate) fn canonical_terminal_send_output_binding_v1(
    credit_id: DigestV1,
    recipient_lane_id: DigestV1,
    request_digest: DigestV1,
    acceptance_ticket_digest: DigestV1,
    ciphertext_commitment: DigestV1,
    amount: u128,
) -> DigestV1 {
    hash_fixed_v1(
        TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &credit_id,
            &recipient_lane_id,
            &request_digest,
            &acceptance_ticket_digest,
            &ciphertext_commitment,
            &amount.to_le_bytes(),
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

fn canonical_intent_bytes_v1(intent: &OfflineCashAcceptanceIntentV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ACCEPTANCE_INTENT_CANONICAL_BYTES_V1);
    bytes.extend_from_slice(&intent.version.to_le_bytes());
    bytes.extend_from_slice(&intent.request_digest);
    bytes.extend_from_slice(&intent.intent_id);
    bytes.extend_from_slice(&intent.exact_amount.to_le_bytes());
    bytes.extend_from_slice(&intent.sender_one_time_commitment);
    bytes
}

/// Return the exact canonical intent digest constrained by the wrapper circuit.
pub(crate) fn canonical_acceptance_intent_digest_v1(
    intent: &OfflineCashAcceptanceIntentV1,
) -> Result<DigestV1, String> {
    let bytes = canonical_intent_bytes_v1(intent);
    if bytes.len() != ACCEPTANCE_INTENT_CANONICAL_BYTES_V1 {
        return Err("acceptance-intent canonical layout drift".to_owned());
    }
    Ok(hash_canonical_bytes_v1(
        ACCEPTANCE_INTENT_DIGEST_DOMAIN_V1,
        &bytes,
    ))
}

fn canonical_acceptance_intent_authorization_statement_bytes_v1(
    statement: &OfflineCashAcceptanceIntentAuthorizationStatementV1,
) -> Vec<u8> {
    let mut bytes =
        Vec::with_capacity(ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_CANONICAL_BYTES_V1);
    bytes.extend_from_slice(&statement.version.to_le_bytes());
    bytes.extend_from_slice(&canonical_intent_bytes_v1(&statement.intent));
    bytes.extend_from_slice(&statement.release_id);
    bytes.extend_from_slice(&statement.suite_id);
    bytes.extend_from_slice(&statement.vk_digest);
    bytes.extend_from_slice(&statement.artifact_manifest_digest);
    bytes
}

/// Return the exact pre-ticket authorization semantic digest constrained by both parities.
pub(crate) fn canonical_acceptance_intent_authorization_statement_digest_v1(
    statement: &OfflineCashAcceptanceIntentAuthorizationStatementV1,
    request: &OfflineCashPaymentRequestV1,
) -> Result<DigestV1, String> {
    statement
        .validate_shape_against(request)
        .map_err(|error| error.to_string())?;
    let bytes = canonical_acceptance_intent_authorization_statement_bytes_v1(statement);
    if bytes.len() != ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_CANONICAL_BYTES_V1 {
        return Err("acceptance-intent authorization statement layout drift".to_owned());
    }
    let digest = hash_canonical_bytes_v1(
        ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN_V1,
        &bytes,
    );
    if digest
        != statement
            .canonical_digest_against(request)
            .map_err(|error| error.to_string())?
    {
        return Err("acceptance-intent authorization statement digest drift".to_owned());
    }
    Ok(digest)
}

fn canonical_outbox_reservation_bytes_v1(reservation: OfflineCashOutboxReservationV1) -> Vec<u8> {
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

/// Return the exact fixed-layout outbox reservation commitment constrained by the wrapper.
pub(crate) fn canonical_outbox_reservation_commitment_v1(
    reservation: OfflineCashOutboxReservationV1,
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

fn evidence_commitment_v1(evidence: OfflineCashCommitEvidenceV1) -> DigestV1 {
    match evidence {
        OfflineCashCommitEvidenceV1::TrustedTime(value) => value.time_evidence_commitment,
        OfflineCashCommitEvidenceV1::MonotonicLease(value) => value.lease_evidence_commitment,
    }
}

fn evidence_tag_v1(evidence: OfflineCashCommitEvidenceV1) -> u8 {
    match evidence {
        OfflineCashCommitEvidenceV1::TrustedTime(_) => 0,
        OfflineCashCommitEvidenceV1::MonotonicLease(_) => 1,
    }
}

fn canonical_evidence_bytes_v1(evidence: OfflineCashCommitEvidenceV1) -> [u8; 36] {
    let mut bytes = [0_u8; 36];
    bytes[..4].copy_from_slice(&u32::from(evidence_tag_v1(evidence)).to_le_bytes());
    bytes[4..].copy_from_slice(&evidence_commitment_v1(evidence));
    bytes
}

/// Recompute the opaque public commit-evidence commitment from its complete private opening.
pub(crate) fn canonical_commit_evidence_commitment_v1(
    opening: OfflineCashCommitEvidenceOpeningV1,
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

fn canonical_commit_certificate_id_bytes_v1(
    certificate: &OfflineCashCommitCertificateV1,
) -> Vec<u8> {
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

fn canonical_commit_certificate_bytes_v1(certificate: &OfflineCashCommitCertificateV1) -> Vec<u8> {
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

/// Return the exact canonical commit-certificate digest constrained by the wrapper circuit.
pub(crate) fn canonical_commit_certificate_digest_v1(
    certificate: &OfflineCashCommitCertificateV1,
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
    operation: OfflineCashOperationV1,
    one_use_hardware_authorization: DigestV1,
    predecessor: &OfflineCashStateV1,
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

/// Derive the intent-visible commitment to one exact private predecessor authorization.
pub(crate) fn canonical_sender_one_time_commitment_v1(
    opening: DigestV1,
    prepared_authorization_digest: DigestV1,
    request_digest: DigestV1,
    intent_id: DigestV1,
    amount: u128,
) -> DigestV1 {
    hash_fixed_v1(
        SENDER_ONE_TIME_COMMITMENT_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &opening,
            &prepared_authorization_digest,
            &request_digest,
            &intent_id,
            &amount.to_le_bytes(),
        ],
    )
}

/// Return the exact candidate precommit binding shared by State and terminal Guard proofs.
pub(crate) fn canonical_precommit_binding_digest_v1(
    lifecycle_binding_digest: DigestV1,
    request_digest: DigestV1,
    intent_digest: DigestV1,
    acceptance_ticket_digest: DigestV1,
    amount: u128,
    reservation_commitment: DigestV1,
    prepared_authorization_digest: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        PRECOMMIT_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &lifecycle_binding_digest,
            &request_digest,
            &intent_digest,
            &acceptance_ticket_digest,
            &amount.to_le_bytes(),
            &reservation_commitment,
            &prepared_authorization_digest,
        ],
    )
}

/// Return the terminal sender authorization hidden inside the terminal Guard statement.
pub(crate) fn canonical_sender_terminal_authorization_digest_v1(
    intent_digest: DigestV1,
    sender_one_time_commitment: DigestV1,
    prepared_authorization_digest: DigestV1,
    acceptance_ticket_digest: DigestV1,
) -> DigestV1 {
    hash_fixed_v1(
        SENDER_TERMINAL_AUTHORIZATION_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &intent_digest,
            &sender_one_time_commitment,
            &prepared_authorization_digest,
            &acceptance_ticket_digest,
        ],
    )
}

/// Return the no-cycle terminal binding authenticated by the postcommit Guard proof.
pub(crate) fn canonical_terminal_commit_binding_digest_v1(
    public: &OfflineCashCommitWrapperPublicInputsV1,
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    precommit_binding_digest: DigestV1,
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
    let (request_issued, request_expires, ticket_issued, ticket_expires) = private
        .request
        .as_ref()
        .zip(private.acceptance_ticket.as_ref())
        .map_or((0, 0, 0, 0), |(request, ticket)| {
            (
                request.issued_at_ms,
                request.expires_at_ms,
                ticket.issued_at_ms,
                ticket.expires_at_ms,
            )
        });
    Ok(hash_fixed_v1(
        TERMINAL_COMMIT_BINDING_DOMAIN_V1,
        &[
            &1_u16.to_le_bytes(),
            &[operation_tag_v1(public.operation) as u8],
            &public.lifecycle_binding_digest,
            &precommit_binding_digest,
            &public.candidate_envelope_digest,
            &public.commit_certificate_digest,
            &private.commit_certificate.certificate_id,
            &private.commit_certificate.hardware_terminal_commitment,
            &public.transition_nullifier,
            &reservation_commitment,
            &[evidence_tag_v1(private.commit_certificate.commit_evidence)],
            &evidence_commitment,
            &public.request_digest,
            &public.acceptance_ticket_digest,
            &public.amount.to_le_bytes(),
            &public.hardware_profile_id,
            &public.policy_epoch.to_le_bytes(),
            &request_issued.to_le_bytes(),
            &request_expires.to_le_bytes(),
            &ticket_issued.to_le_bytes(),
            &ticket_expires.to_le_bytes(),
            &reservation.issued_at_ms.to_le_bytes(),
            &reservation.expires_at_ms.to_le_bytes(),
            &transition_intent_digest,
            &transition_effect_digest,
            &recovery_record_digest,
            &durable_inbox_effect_digest,
            &durable_outbox_effect_digest,
            &private.terminal_envelope_digest,
            &sender_authorization_digest,
        ],
    ))
}

const fn operation_tag_v1(operation: OfflineCashOperationV1) -> u64 {
    match operation {
        OfflineCashOperationV1::Bootstrap => 0,
        OfflineCashOperationV1::MintFold => 1,
        OfflineCashOperationV1::SendSplit => 2,
        OfflineCashOperationV1::ReceiveFoldBatch => 3,
        OfflineCashOperationV1::RedeemSplit => 4,
        OfflineCashOperationV1::SuiteUpgrade => 5,
        OfflineCashOperationV1::Rotate => 6,
    }
}

const fn operation_from_wire_v1(operation: OfflineCashOperationKindV1) -> OfflineCashOperationV1 {
    match operation {
        OfflineCashOperationKindV1::Bootstrap => OfflineCashOperationV1::Bootstrap,
        OfflineCashOperationKindV1::MintFold => OfflineCashOperationV1::MintFold,
        OfflineCashOperationKindV1::SendSplit => OfflineCashOperationV1::SendSplit,
        OfflineCashOperationKindV1::ReceiveFoldBatch => OfflineCashOperationV1::ReceiveFoldBatch,
        OfflineCashOperationKindV1::RedeemSplit => OfflineCashOperationV1::RedeemSplit,
        OfflineCashOperationKindV1::SuiteUpgrade => OfflineCashOperationV1::SuiteUpgrade,
        OfflineCashOperationKindV1::Rotate => OfflineCashOperationV1::Rotate,
    }
}

#[cfg(feature = "zk-halo2-ipa")]
const MINIMUM_UNUSABLE_ROWS: usize = 9;
#[cfg(feature = "zk-halo2-ipa")]
const CANDIDATE_EQUATION_TAG_V1: u32 = 1;
#[cfg(feature = "zk-halo2-ipa")]
const TERMINAL_GUARD_EQUATION_TAG_V1: u32 = 2;
#[cfg(feature = "zk-halo2-ipa")]
const CANDIDATE_BINDING_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:commit-wrapper-candidate\0";

/// Hash the exact private candidate public-prefix values bound by the terminal wrapper.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn canonical_commit_wrapper_candidate_digest_v1<F: BigPrimeField>(
    candidate_instances: &[Vec<F>],
) -> Result<DigestV1, String> {
    if candidate_instances.len() != 1
        || candidate_instances[0].len()
            != state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count()
    {
        return Err("commit wrapper candidate has wrong fixed public shape".to_owned());
    }
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_BINDING_DOMAIN_V1);
    for value in &candidate_instances[0][..state_relation::PUBLIC_INSTANCE_COUNT] {
        let bytes = fe_to_biguint(value).to_bytes_le();
        if bytes.len() > 16 {
            return Err("commit wrapper candidate field exceeds canonical u128".to_owned());
        }
        let mut limb = [0_u8; 16];
        limb[..bytes.len()].copy_from_slice(&bytes);
        hasher.update(limb);
    }
    Ok(hasher.finalize().into())
}

/// Eq/Fp recursive inputs consumed by one terminal wrapper proof.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct OfflineCashCommitWrapperEqWitnessV1<'a> {
    pub(crate) candidate_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) candidate_instances: &'a [Vec<Fp>],
    pub(crate) candidate_proof: &'a [u8],
    pub(crate) candidate_history: &'a OfflineCashEqAccumulatorV1,
    pub(crate) candidate_history_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(crate) terminal_guard_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) terminal_guard_instances: &'a [Vec<Fp>],
    pub(crate) terminal_guard_proof: &'a [u8],
    pub(crate) terminal_guard_history: &'a OfflineCashEqAccumulatorV1,
    pub(crate) terminal_guard_history_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(crate) merge_fold_proof: &'a OfflineCashEqFoldProofV1,
    pub(crate) successor_history: &'a OfflineCashEqAccumulatorV1,
}

/// Ep/Fq recursive inputs consumed by one terminal wrapper proof.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct OfflineCashCommitWrapperEpWitnessV1<'a> {
    pub(crate) candidate_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) candidate_instances: &'a [Vec<Fq>],
    pub(crate) candidate_proof: &'a [u8],
    pub(crate) candidate_history: &'a OfflineCashEpAccumulatorV1,
    pub(crate) candidate_history_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(crate) terminal_guard_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) terminal_guard_instances: &'a [Vec<Fq>],
    pub(crate) terminal_guard_proof: &'a [u8],
    pub(crate) terminal_guard_history: &'a OfflineCashEpAccumulatorV1,
    pub(crate) terminal_guard_history_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(crate) merge_fold_proof: &'a OfflineCashEpFoldProofV1,
    pub(crate) successor_history: &'a OfflineCashEpAccumulatorV1,
}

/// Complete paired candidate and terminal-Guard witness.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct OfflineCashCommitWrapperWitnessV1<'a> {
    pub(crate) public: OfflineCashCommitWrapperPublicInputsV1,
    pub(crate) private_transition: OfflineCashCommitWrapperPrivateTransitionV1,
    /// Present only for the pre-ticket sender-authorization branch.
    pub(crate) intent_authorization: Option<OfflineCashCommitWrapperIntentAuthorizationPrivateV1>,
    /// Present only for the sender-hardware no-commit closure branch.
    pub(crate) no_commit_closure: Option<OfflineCashCommitWrapperNoCommitClosurePrivateV1>,
    /// Complete private statement whose digest is recursively authenticated by terminal Guard.
    pub(crate) terminal_guard_relation: OfflineCashGuardBundleRelationWitnessV1,
    /// Sorted, nonzero-prefix release-enabled profile IDs, padded with canonical zeroes.
    pub(crate) enabled_hardware_profiles: [DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    pub(crate) eq: OfflineCashCommitWrapperEqWitnessV1<'a>,
    pub(crate) ep: OfflineCashCommitWrapperEpWitnessV1<'a>,
}

struct CommitWrapperParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    private_transition: &'a OfflineCashCommitWrapperPrivateTransitionV1,
    intent_authorization: Option<&'a OfflineCashCommitWrapperIntentAuthorizationPrivateV1>,
    no_commit_closure: Option<&'a OfflineCashCommitWrapperNoCommitClosurePrivateV1>,
    terminal_guard_relation: &'a OfflineCashGuardBundleRelationWitnessV1,
    enabled_hardware_profiles: &'a [DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
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
    successor_history: &'a [u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Base, SHA-256, and reciprocal dense-MSM configuration for a wrapper parity.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub(crate) struct OfflineCashCommitWrapperCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp half of the final commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct OfflineCashCommitWrapperEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq half of the final commit wrapper.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct OfflineCashCommitWrapperEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

#[cfg(feature = "zk-halo2-ipa")]
macro_rules! impl_commit_wrapper_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = OfflineCashCommitWrapperCircuitConfigV1<$field>;
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
                OfflineCashCommitWrapperCircuitConfigV1 {
                    base,
                    sha: PastaSha256ConfigV1::configure(meta),
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
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
impl_commit_wrapper_circuit!(
    OfflineCashCommitWrapperEqCircuitV1,
    Fp,
    EpAffine,
    "Offline Cash Eq commit wrapper"
);
#[cfg(feature = "zk-halo2-ipa")]
impl_commit_wrapper_circuit!(
    OfflineCashCommitWrapperEpCircuitV1,
    Fq,
    EqAffine,
    "Offline Cash Ep commit wrapper"
);

fn validate_enabled_hardware_profiles_v1(
    profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
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
fn validate_intent_authorization_relation_v1(
    public: &OfflineCashCommitWrapperPublicInputsV1,
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    authorization: &OfflineCashCommitWrapperIntentAuthorizationPrivateV1,
    relation: &OfflineCashGuardBundleRelationWitnessV1,
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
) -> Result<(), String> {
    authorization.validate_against(public)?;
    relation.validate()?;
    validate_enabled_hardware_profiles_v1(enabled_profiles)?;
    private
        .predecessor
        .validate()
        .map_err(|error| format!("invalid authorization predecessor: {error}"))?;
    private
        .successor
        .validate()
        .map_err(|error| format!("invalid authorization successor: {error}"))?;
    let guard = &relation.statement;
    if !enabled_profiles
        .iter()
        .take_while(|profile| **profile != [0; 32])
        .any(|profile| *profile == guard.hardware_profile_id)
        || private.one_use_hardware_authorization == [0; 32]
        || private.sender_one_time_opening == [0; 32]
        || private.authorization_counter_after
            != private
                .authorization_counter_before
                .checked_add(1)
                .ok_or_else(|| "authorization counter overflow".to_owned())?
        || guard.operation != OfflineCashOperationV1::SendSplit
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
        || guard.lifecycle_binding_digest != public.semantic_digest
        || guard.transition_intent_digest != public.semantic_digest
        || guard.terminal_commit_binding_digest != [0; 32]
        || guard.sender_one_time_authorization_digest != [0; 32]
        || guard.predecessor_state_commitment != private.predecessor.state_commitment
        || guard.successor_state_commitment != private.successor.state_commitment
        || guard.predecessor_state_nonce_commitment != private.predecessor.state_nonce_commitment
        || guard.successor_state_nonce_commitment != private.successor.state_nonce_commitment
        || guard.predecessor_logical_sequence != private.predecessor.logical_sequence
        || guard.successor_logical_sequence != private.successor.logical_sequence
        || guard.predecessor_hardware_epoch_generation
            != private.predecessor.hardware_epoch.generation
        || guard.successor_hardware_epoch_generation != private.successor.hardware_epoch.generation
        || guard.predecessor_hardware_epoch_id != private.predecessor.hardware_epoch.epoch_id
        || guard.successor_hardware_epoch_id != private.successor.hardware_epoch.epoch_id
        || guard.predecessor_key_reference
            != private
                .predecessor
                .device_policy_binding
                .device_key_reference
        || guard.successor_key_reference
            != private.successor.device_policy_binding.device_key_reference
        || guard.predecessor_hardware_policy_id
            != private.predecessor.device_policy_binding.hardware_policy_id
        || guard.successor_hardware_policy_id
            != private.successor.device_policy_binding.hardware_policy_id
        || guard.journal_revision_before != private.journal_revision_before
        || guard.journal_revision_after != private.journal_revision_after
    {
        return Err("intent authorization State/Guard binding mismatch".to_owned());
    }
    let predecessor_balance = private
        .successor
        .balance
        .checked_add(public.amount)
        .ok_or_else(|| "authorization balance overflow".to_owned())?;
    if predecessor_balance != private.predecessor.balance {
        return Err("intent authorization does not reserve sufficient balance".to_owned());
    }
    let prepared = canonical_prepared_one_use_authorization_digest_v1(
        OfflineCashOperationV1::SendSplit,
        private.one_use_hardware_authorization,
        &private.predecessor,
        private.journal_revision_before,
        private.authorization_counter_before,
    );
    let statement = &authorization.statement;
    let expected_sender_commitment = canonical_sender_one_time_commitment_v1(
        private.sender_one_time_opening,
        prepared,
        statement.intent.request_digest,
        statement.intent.intent_id,
        statement.intent.exact_amount,
    );
    let intent_digest = canonical_acceptance_intent_digest_v1(&statement.intent)?;
    let precommit = canonical_precommit_binding_digest_v1(
        public.semantic_digest,
        public.request_digest,
        intent_digest,
        [0; 32],
        public.amount,
        guard.durable_outbox_effect_digest,
        prepared,
    );
    if statement.intent.sender_one_time_commitment != expected_sender_commitment
        || guard.precommit_binding_digest != precommit
    {
        return Err("intent authorization one-use reservation mismatch".to_owned());
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_no_commit_closure_relation_v1(
    public: &OfflineCashCommitWrapperPublicInputsV1,
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    closure: &OfflineCashCommitWrapperNoCommitClosurePrivateV1,
    relation: &OfflineCashGuardBundleRelationWitnessV1,
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
) -> Result<(), String> {
    closure
        .statement
        .validate_shape()
        .map_err(|error| error.to_string())?;
    relation.validate()?;
    validate_enabled_hardware_profiles_v1(enabled_profiles)?;
    let request = private
        .request
        .as_ref()
        .ok_or_else(|| "no-commit request is absent".to_owned())?;
    let intent = private
        .acceptance_intent
        .as_ref()
        .ok_or_else(|| "no-commit sender intent is absent".to_owned())?;
    let ticket = private
        .acceptance_ticket
        .as_ref()
        .ok_or_else(|| "no-commit acceptance ticket is absent".to_owned())?;
    request
        .validate_shape()
        .map_err(|error| error.to_string())?;
    intent
        .validate_shape_against(request)
        .map_err(|error| error.to_string())?;
    ticket
        .validate_shape_against(request, intent)
        .map_err(|error| error.to_string())?;
    closure
        .intent_authorization
        .validate_shape_against(request)
        .map_err(|error| error.to_string())?;
    if closure.intent_authorization.statement.intent != *intent {
        return Err("no-commit authorization does not open the exact sender intent".to_owned());
    }
    let statement = &closure.statement;
    let expected_public = OfflineCashCommitWrapperPublicInputsV1::from_no_commit_closure(
        statement,
        public.candidate_envelope_digest,
        public.acceptance_ticket_digest,
        public.eq_deferred_audit,
        public.ep_deferred_audit,
        public.eq_protocol_digest,
        public.ep_protocol_digest,
    )?;
    if &expected_public != public {
        return Err("no-commit public/private statement binding mismatch".to_owned());
    }
    let request_digest = request
        .canonical_digest()
        .map_err(|error| error.to_string())?;
    let intent_digest = canonical_acceptance_intent_digest_v1(intent)?;
    let ticket_digest = ticket
        .canonical_digest_against(request, intent)
        .map_err(|error| error.to_string())?;
    let intent_authorization_digest = closure
        .intent_authorization
        .canonical_digest_against(request)
        .map_err(|error| error.to_string())?;
    if request.request_id != statement.request_id
        || request_digest != statement.request_digest
        || ticket.acceptance_ticket_id != statement.acceptance_ticket_id
        || ticket_digest != statement.ticket_digest
        || intent_authorization_digest != statement.intent_authorization_digest
        || intent_digest != statement.intent_digest
        || intent.exact_amount != statement.exact_amount
        || ticket.exact_amount != statement.exact_amount
        || intent.sender_one_time_commitment != statement.sender_one_time_commitment
        || closure.intent_authorization.statement.release_id != statement.release_id
        || closure.intent_authorization.statement.suite_id != statement.suite_id
        || closure.intent_authorization.statement.vk_digest != statement.vk_digest
        || closure
            .intent_authorization
            .statement
            .artifact_manifest_digest
            != statement.artifact_manifest_digest
    {
        return Err("no-commit request/ticket/authorization binding mismatch".to_owned());
    }

    private
        .predecessor
        .validate()
        .map_err(|error| format!("invalid no-commit predecessor: {error}"))?;
    private
        .successor
        .validate()
        .map_err(|error| format!("invalid no-commit prepared successor: {error}"))?;
    if private.one_use_hardware_authorization == [0; 32]
        || private.sender_one_time_opening == [0; 32]
        || closure.hardware_recovery_nonce == [0; 32]
        || private.authorization_counter_after
            != private
                .authorization_counter_before
                .checked_add(1)
                .ok_or_else(|| "no-commit authorization counter overflow".to_owned())?
        || private.journal_revision_after
            != private
                .journal_revision_before
                .checked_add(1)
                .ok_or_else(|| "no-commit journal overflow".to_owned())?
        || private
            .successor
            .balance
            .checked_add(statement.exact_amount)
            != Some(private.predecessor.balance)
    {
        return Err("no-commit private predecessor transition is noncanonical".to_owned());
    }
    let prepared = canonical_prepared_one_use_authorization_digest_v1(
        OfflineCashOperationV1::SendSplit,
        private.one_use_hardware_authorization,
        &private.predecessor,
        private.journal_revision_before,
        private.authorization_counter_before,
    );
    let expected_sender_commitment = canonical_sender_one_time_commitment_v1(
        private.sender_one_time_opening,
        prepared,
        request_digest,
        intent.intent_id,
        statement.exact_amount,
    );
    let recovery_id = canonical_no_commit_recovery_id_v1(
        prepared,
        request_digest,
        ticket_digest,
        request.hardware_credential.lane_commitment,
        closure.hardware_recovery_nonce,
    );
    let cancellation_successor = canonical_no_commit_cancellation_successor_v1(
        prepared,
        recovery_id,
        intent_authorization_digest,
        ticket_digest,
        statement.equivalent_delivery_slot_commitment,
        private.journal_revision_after,
        private.authorization_counter_after,
    );
    let cancellation_nullifier = canonical_predecessor_conflict_nullifier_v1(prepared);
    let semantic_digest = canonical_no_commit_closure_statement_digest_v1(statement)?;
    let precommit = canonical_precommit_binding_digest_v1(
        semantic_digest,
        request_digest,
        intent_digest,
        ticket_digest,
        statement.exact_amount,
        statement.equivalent_delivery_slot_commitment,
        prepared,
    );
    let guard = &relation.statement;
    if expected_sender_commitment != statement.sender_one_time_commitment
        || recovery_id != statement.recovery_id
        || cancellation_successor == prepared
        || cancellation_nullifier != statement.cancellation_nullifier
        || canonical_no_commit_hardware_binding_v1(guard)
            != statement.sender_hardware_binding_commitment
        || !enabled_profiles
            .iter()
            .take_while(|profile| **profile != [0; 32])
            .any(|profile| *profile == guard.hardware_profile_id)
        || guard.operation != OfflineCashOperationV1::SendSplit
        || guard.amount != 0
        || guard.peer_credit_id != [0; 32]
        || guard.peer_recipient_lane_id != [0; 32]
        || guard.release_id != statement.release_id
        || guard.predecessor_suite_id != statement.suite_id
        || guard.predecessor_vk_digest != statement.vk_digest
        || guard.successor_suite_id != statement.suite_id
        || guard.successor_vk_digest != statement.vk_digest
        || guard.predecessor_state_commitment != private.predecessor.state_commitment
        || guard.successor_state_commitment != private.predecessor.state_commitment
        || guard.predecessor_state_nonce_commitment != private.predecessor.state_nonce_commitment
        || guard.successor_state_nonce_commitment != private.predecessor.state_nonce_commitment
        || guard.predecessor_logical_sequence != private.predecessor.logical_sequence
        || guard.successor_logical_sequence != private.predecessor.logical_sequence
        || guard.predecessor_hardware_epoch_generation
            != private.predecessor.hardware_epoch.generation
        || guard.successor_hardware_epoch_generation
            != private.predecessor.hardware_epoch.generation
        || guard.predecessor_hardware_epoch_id != private.predecessor.hardware_epoch.epoch_id
        || guard.successor_hardware_epoch_id != private.predecessor.hardware_epoch.epoch_id
        || guard.predecessor_key_reference
            != private
                .predecessor
                .device_policy_binding
                .device_key_reference
        || guard.successor_key_reference
            != private
                .predecessor
                .device_policy_binding
                .device_key_reference
        || guard.predecessor_hardware_policy_id
            != private.predecessor.device_policy_binding.hardware_policy_id
        || guard.successor_hardware_policy_id
            != private.predecessor.device_policy_binding.hardware_policy_id
        || guard.journal_revision_before != private.journal_revision_before
        || guard.journal_revision_after != private.journal_revision_after
        || guard.lifecycle_binding_digest != semantic_digest
        || guard.precommit_binding_digest != precommit
        || guard.terminal_commit_binding_digest != [0; 32]
        || guard.sender_one_time_authorization_digest != cancellation_successor
        || guard.transition_intent_digest != statement.recovery_id
        || guard.transition_effect_digest != intent_authorization_digest
        || guard.recovery_record_digest != ticket_digest
        || guard.durable_outbox_effect_digest != statement.equivalent_delivery_slot_commitment
        || guard.durable_inbox_effect_digest != relation.canonical_empty_effect_digest
    {
        return Err("no-commit cancellation Guard binding mismatch".to_owned());
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_terminal_guard_relation_v1(
    public: &OfflineCashCommitWrapperPublicInputsV1,
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    relation: &OfflineCashGuardBundleRelationWitnessV1,
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
    let (intent_digest, sender_commitment) = match private.acceptance_intent.as_ref() {
        Some(intent) => (
            canonical_acceptance_intent_digest_v1(intent)?,
            intent.sender_one_time_commitment,
        ),
        None => ([0; 32], [0; 32]),
    };
    let precommit = canonical_precommit_binding_digest_v1(
        public.lifecycle_binding_digest,
        public.request_digest,
        intent_digest,
        public.acceptance_ticket_digest,
        public.amount,
        canonical_outbox_reservation_commitment_v1(private.outbox_reservation)?,
        prepared_authorization,
    );
    let sender_authorization = if public.operation == OfflineCashOperationV1::SendSplit {
        canonical_sender_terminal_authorization_digest_v1(
            intent_digest,
            sender_commitment,
            prepared_authorization,
            public.acceptance_ticket_digest,
        )
    } else {
        [0; 32]
    };
    let terminal = canonical_terminal_commit_binding_digest_v1(
        public,
        private,
        precommit,
        sender_authorization,
        guard.transition_intent_digest,
        guard.transition_effect_digest,
        guard.recovery_record_digest,
        guard.durable_inbox_effect_digest,
        guard.durable_outbox_effect_digest,
    )?;
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
        || guard.precommit_binding_digest != precommit
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
        return Err("terminal Guard/private wrapper relation mismatch".to_owned());
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn validate_candidate_guard_protocol_binding_v1<F: OfflineCashPoseidonFieldV1>(
    candidate_instances: &[Vec<F>],
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<(), String> {
    let candidate = candidate_instances
        .first()
        .ok_or_else(|| "commit wrapper candidate public column is absent".to_owned())?;
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT {
        return Err("commit wrapper candidate public column is truncated".to_owned());
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
        let expected = crate::zk::offline_cash_v1_poseidon::digest_limbs::<F>(digest);
        if candidate[offset..offset + 2] != expected {
            return Err(
                "commit wrapper candidate selects a different GuardBundle protocol".to_owned(),
            );
        }
    }
    Ok(())
}

/// Build the mutually audited terminal wrapper pair.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_offline_cash_commit_wrapper_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: OfflineCashCommitWrapperWitnessV1<'_>,
) -> Result<
    (
        OfflineCashCommitWrapperEqCircuitV1,
        OfflineCashCommitWrapperEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    validate_enabled_hardware_profiles_v1(&witness.enabled_hardware_profiles)?;
    let terminal_guard_eq_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq.terminal_guard_protocol,
        OfflineCashPastaParityV1::Eq,
    )?;
    let terminal_guard_ep_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep.terminal_guard_protocol,
        OfflineCashPastaParityV1::Ep,
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
    match (
        witness.public.is_acceptance_intent_authorization(),
        witness.public.is_no_commit_closure(),
        witness.intent_authorization.as_ref(),
        witness.no_commit_closure.as_ref(),
    ) {
        (true, false, Some(authorization), None) => validate_intent_authorization_relation_v1(
            &witness.public,
            &witness.private_transition,
            authorization,
            &witness.terminal_guard_relation,
            &witness.enabled_hardware_profiles,
        )?,
        (false, true, None, Some(closure)) => validate_no_commit_closure_relation_v1(
            &witness.public,
            &witness.private_transition,
            closure,
            &witness.terminal_guard_relation,
            &witness.enabled_hardware_profiles,
        )?,
        (false, false, None, None) => {
            witness
                .private_transition
                .validate_against(&witness.public)?;
            validate_terminal_guard_relation_v1(
                &witness.public,
                &witness.private_transition,
                &witness.terminal_guard_relation,
            )?;
        }
        _ => return Err("commit wrapper branch witness is noncanonical".to_owned()),
    }
    let eq_candidate_history = witness
        .eq
        .candidate_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_candidate_history = witness
        .ep
        .candidate_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_terminal_history = witness
        .eq
        .terminal_guard_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let ep_terminal_history = witness
        .ep
        .terminal_guard_history
        .to_native()
        .map_err(|error| error.to_string())?;
    let eq_svk = super::composite::eq_succinct_vk(eq_params);
    let ep_svk = super::composite::ep_succinct_vk(ep_params);
    let (mut eq_builder, eq_sha, eq_output) = build_commit_wrapper_scalar_half_v1::<EqAffine>(
        &eq_svk,
        OfflineCashPastaParityV1::Eq,
        &witness.public,
        CommitWrapperParityWitnessV1 {
            private_transition: &witness.private_transition,
            intent_authorization: witness.intent_authorization.as_ref(),
            no_commit_closure: witness.no_commit_closure.as_ref(),
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
    )?;
    let (mut ep_builder, ep_sha, ep_output) = build_commit_wrapper_scalar_half_v1::<EpAffine>(
        &ep_svk,
        OfflineCashPastaParityV1::Ep,
        &witness.public,
        CommitWrapperParityWitnessV1 {
            private_transition: &witness.private_transition,
            intent_authorization: witness.intent_authorization.as_ref(),
            no_commit_closure: witness.no_commit_closure.as_ref(),
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
    )?;

    let mut eq_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EpAffine>(
        &mut eq_builder,
        OfflineCashPastaParityV1::Ep,
        &ep_output,
        &mut eq_dense,
    )?;
    let mut ep_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_parent_pass_v1::<EqAffine>(
        &mut ep_builder,
        OfflineCashPastaParityV1::Eq,
        &eq_output,
        &mut ep_dense,
    )?;
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << super::OFFLINE_CASH_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS;
    eq_sha.validate_capacity(usable_rows)?;
    ep_sha.validate_capacity(usable_rows)?;
    eq_dense.validate_capacity(usable_rows)?;
    ep_dense.validate_capacity(usable_rows)?;
    let eq_audit = super::composite::assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = super::composite::assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    Ok((
        OfflineCashCommitWrapperEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_sha,
            dense_jobs: eq_dense,
        },
        OfflineCashCommitWrapperEpCircuitV1 {
            builder: ep_builder,
            sha_jobs: ep_sha,
            dense_jobs: ep_dense,
        },
        eq_audit,
        ep_audit,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_commit_wrapper_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: OfflineCashPastaParityV1,
    public: &OfflineCashCommitWrapperPublicInputsV1,
    witness: CommitWrapperParityWitnessV1<'_, C>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        PastaSha256JobsV1<C::ScalarExt>,
        OfflineCashDeferredParentOutputV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
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
        return Err("commit wrapper nested proof has wrong fixed public shape".to_owned());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(
            usize::try_from(super::OFFLINE_CASH_RECURSION_IPA_K_V1)
                .expect("Offline Cash k fits usize"),
        )
        .use_lookup_bits(
            usize::try_from(super::OFFLINE_CASH_RECURSION_IPA_K_V1 - 1)
                .expect("Offline Cash lookup bits fit usize"),
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
    if builder.assigned_instances[0].len() != COMMIT_WRAPPER_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("commit wrapper public instance has wrong fixed shape".to_owned());
    }

    let mut sha_jobs = PastaSha256JobsV1::default();
    let assigned_terminal_guard = constrain_guard_bundle_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        witness.terminal_guard_relation,
    )?;
    constrain_terminal_commit_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        witness.private_transition,
        &assigned_terminal_guard,
    )?;
    constrain_intent_authorization_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        witness.private_transition,
        witness.intent_authorization,
        &assigned_terminal_guard,
        witness.enabled_hardware_profiles,
    )?;
    constrain_no_commit_closure_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        witness.private_transition,
        witness.no_commit_closure,
        &assigned_terminal_guard,
        witness.enabled_hardware_profiles,
    )?;
    let candidate_protocol_digest =
        native_parent_protocol_digest_v1(witness.candidate_protocol, parity)?;
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let candidate_instances = assign_nested_instances_v1(&loader, witness.candidate_instances);
    let candidate_column = candidate_instances
        .first()
        .ok_or_else(|| "commit wrapper candidate public column is absent".to_owned())?;
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
    // material is consequently committed by the authenticated wrapper verifying key; a prover
    // cannot select an arbitrary protocol through witness bytes.
    let candidate_protocol = witness.candidate_protocol.loaded(&loader);
    let candidate_current = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &candidate_protocol,
        &candidate_instances,
        witness.candidate_proof,
    )
    .map_err(|error| format!("commit wrapper candidate verifier failed: {error:?}"))?;
    let candidate_history = load_native_accumulator(&loader, witness.candidate_history)
        .map_err(|error| format!("commit wrapper candidate history failed: {error:?}"))?;
    let candidate_history_limbs = candidate_column
        .get(state_relation::PUBLIC_INSTANCE_COUNT..)
        .ok_or_else(|| "commit wrapper candidate history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &candidate_history, &candidate_history_limbs)
        .map_err(|error| format!("commit wrapper candidate history binding failed: {error:?}"))?;
    let complete_candidate = verify_fold(
        &loader,
        succinct_vk,
        &[candidate_current, candidate_history],
        witness.candidate_history_fold_proof,
    )
    .map_err(|error| format!("commit wrapper candidate fold failed: {error:?}"))?;
    let candidate_end = loader.ecc_chip().equation_count();
    if candidate_end == 0 {
        return Err("commit wrapper candidate verifier emitted no equations".to_owned());
    }

    let terminal_instances = assign_nested_instances_v1(&loader, witness.terminal_guard_instances);
    let terminal_column = terminal_instances
        .first()
        .ok_or_else(|| "commit wrapper terminal Guard public column is absent".to_owned())?;
    let gate = halo2_base::gates::GateChip::default();
    let certificate_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_cells[public_instance::COMMIT_CERTIFICATE_LO],
    );
    let certificate_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_cells[public_instance::COMMIT_CERTIFICATE_LO + 1],
    );
    let authorization_branch = gate.and(
        loader.ctx_mut().main(),
        certificate_low_zero,
        certificate_high_zero,
    );
    for (guard_offset, wrapper_offset) in [
        (GUARD_EQ_AUDIT_OFFSET_V1, public_instance::CANDIDATE_LO),
        (
            GUARD_EP_AUDIT_OFFSET_V1,
            public_instance::ACCEPTANCE_TICKET_LO,
        ),
    ] {
        for limb in 0..2 {
            let difference = gate.sub(
                loader.ctx_mut().main(),
                *terminal_column[guard_offset + limb].assigned(),
                public_cells[wrapper_offset + limb],
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
    .map_err(|error| format!("commit wrapper terminal Guard verifier failed: {error:?}"))?;
    let terminal_history = load_native_accumulator(&loader, witness.terminal_guard_history)
        .map_err(|error| format!("commit wrapper terminal Guard history failed: {error:?}"))?;
    let terminal_history_limbs = terminal_column
        .get(GUARD_HISTORY_OFFSET_V1..)
        .ok_or_else(|| "commit wrapper terminal Guard history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &terminal_history, &terminal_history_limbs).map_err(
        |error| format!("commit wrapper terminal Guard history binding failed: {error:?}"),
    )?;
    let complete_terminal = verify_fold(
        &loader,
        succinct_vk,
        &[terminal_current, terminal_history],
        witness.terminal_guard_history_fold_proof,
    )
    .map_err(|error| format!("commit wrapper terminal Guard fold failed: {error:?}"))?;
    let complete = verify_fold(
        &loader,
        succinct_vk,
        &[complete_candidate, complete_terminal],
        witness.merge_fold_proof,
    )
    .map_err(|error| format!("commit wrapper merged history fold failed: {error:?}"))?;
    bind_accumulator_limbs(&loader, &complete, &history_cells)
        .map_err(|error| format!("commit wrapper successor history binding failed: {error:?}"))?;
    let terminal_end = loader.ecc_chip().equation_count();
    if terminal_end <= candidate_end {
        return Err("commit wrapper terminal Guard verifier emitted no equations".to_owned());
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
    .map_err(|error| format!("commit wrapper deferred audit failed: {error:?}"))?;
    let expected_offset = match parity {
        OfflineCashPastaParityV1::Eq => public_instance::EQ_DEFERRED_AUDIT_LO,
        OfflineCashPastaParityV1::Ep => public_instance::EP_DEFERRED_AUDIT_LO,
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
fn assign_public_prefix_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &halo2_base::gates::RangeChip<F>,
    public: &OfflineCashCommitWrapperPublicInputsV1,
) -> Result<Vec<AssignedValue<F>>, String> {
    let values = public.public_prefix::<F>()?;
    let cells = values
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
        halo2_base::QuantumCell::Constant(F::from(2)),
    );
    let redeem = gate.is_equal(
        builder.main(0),
        cells[public_instance::OPERATION],
        halo2_base::QuantumCell::Constant(F::from(4)),
    );
    let valid_operation = gate.add(builder.main(0), send, redeem);
    gate.assert_is_const(builder.main(0), &valid_operation, &F::ONE);
    let amount_is_zero = gate.is_zero(builder.main(0), cells[public_instance::AMOUNT]);
    gate.assert_is_const(builder.main(0), &amount_is_zero, &F::ZERO);

    let terminal = assigned_digest_nonzero_v1(
        builder,
        range,
        [
            cells[public_instance::COMMIT_CERTIFICATE_LO],
            cells[public_instance::COMMIT_CERTIFICATE_LO + 1],
        ],
    );
    let transition_nullifier_nonzero = assigned_digest_nonzero_v1(
        builder,
        range,
        [
            cells[public_instance::TRANSITION_NULLIFIER_LO],
            cells[public_instance::TRANSITION_NULLIFIER_LO + 1],
        ],
    );
    let transition_nullifier_zero = gate.not(builder.main(0), transition_nullifier_nonzero);
    let certificate_zero = gate.not(builder.main(0), terminal);
    let authorization = gate.and(builder.main(0), certificate_zero, transition_nullifier_zero);
    let closure = gate.and(
        builder.main(0),
        certificate_zero,
        transition_nullifier_nonzero,
    );
    let terminal_or_closure = gate.add(builder.main(0), terminal, closure);
    let authorization_redeem = gate.and(builder.main(0), authorization, redeem);
    gate.assert_is_const(builder.main(0), &authorization_redeem, &F::ZERO);
    let closure_redeem = gate.and(builder.main(0), closure, redeem);
    gate.assert_is_const(builder.main(0), &closure_redeem, &F::ZERO);

    for offset in [
        public_instance::SUITE_LO,
        public_instance::VK_LO,
        public_instance::RELEASE_LO,
        public_instance::NETWORK_LO,
        public_instance::ASSET_LO,
        public_instance::ASSET_INCARNATION_LO,
        public_instance::LIABILITY_POOL_LO,
        public_instance::LIFECYCLE_LO,
        public_instance::SEMANTIC_LO,
        public_instance::EQ_DEFERRED_AUDIT_LO,
        public_instance::EP_DEFERRED_AUDIT_LO,
        public_instance::EQ_PROTOCOL_LO,
        public_instance::EP_PROTOCOL_LO,
    ] {
        let nonzero =
            assigned_digest_nonzero_v1(builder, range, [cells[offset], cells[offset + 1]]);
        gate.assert_is_const(builder.main(0), &nonzero, &F::ONE);
    }
    for offset in [public_instance::HARDWARE_PROFILE_LO] {
        let nonzero =
            assigned_digest_nonzero_v1(builder, range, [cells[offset], cells[offset + 1]]);
        builder
            .main(0)
            .constrain_equal(&nonzero, &terminal_or_closure);
    }
    let candidate_or_guard_audit_nonzero = assigned_digest_nonzero_v1(
        builder,
        range,
        [
            cells[public_instance::CANDIDATE_LO],
            cells[public_instance::CANDIDATE_LO + 1],
        ],
    );
    gate.assert_is_const(builder.main(0), &candidate_or_guard_audit_nonzero, &F::ONE);
    builder
        .main(0)
        .constrain_equal(&transition_nullifier_nonzero, &terminal_or_closure);
    let policy_zero = gate.is_zero(builder.main(0), cells[public_instance::POLICY_EPOCH]);
    let policy_nonzero = gate.not(builder.main(0), policy_zero);
    builder
        .main(0)
        .constrain_equal(&policy_nonzero, &terminal_or_closure);
    for offset in [public_instance::REQUEST_LO, public_instance::CIPHERTEXT_LO] {
        let nonzero =
            assigned_digest_nonzero_v1(builder, range, [cells[offset], cells[offset + 1]]);
        builder.main(0).constrain_equal(&nonzero, &send);
    }
    let ticket_nonzero = assigned_digest_nonzero_v1(
        builder,
        range,
        [
            cells[public_instance::ACCEPTANCE_TICKET_LO],
            cells[public_instance::ACCEPTANCE_TICKET_LO + 1],
        ],
    );
    builder.main(0).constrain_equal(&ticket_nonzero, &send);
    let guard_audit_low_equal = gate.is_equal(
        builder.main(0),
        cells[public_instance::CANDIDATE_LO],
        cells[public_instance::ACCEPTANCE_TICKET_LO],
    );
    let guard_audit_high_equal = gate.is_equal(
        builder.main(0),
        cells[public_instance::CANDIDATE_LO + 1],
        cells[public_instance::ACCEPTANCE_TICKET_LO + 1],
    );
    let guard_audits_equal = gate.and(
        builder.main(0),
        guard_audit_low_equal,
        guard_audit_high_equal,
    );
    constrain_zero_if_v1(builder.main(0), range, certificate_zero, guard_audits_equal);
    let output_binding_nonzero = assigned_digest_nonzero_v1(
        builder,
        range,
        [
            cells[public_instance::OUTPUT_BINDING_LO],
            cells[public_instance::OUTPUT_BINDING_LO + 1],
        ],
    );
    builder
        .main(0)
        .constrain_equal(&output_binding_nonzero, &terminal_or_closure);
    for limb in 0..2 {
        constrain_equal_if_v1(
            builder.main(0),
            range,
            authorization,
            cells[public_instance::LIFECYCLE_LO + limb],
            cells[public_instance::SEMANTIC_LO + limb],
        );
    }
    Ok(cells)
}

#[cfg(feature = "zk-halo2-ipa")]
fn assigned_digest_nonzero_v1<F: OfflineCashPoseidonFieldV1>(
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
struct AssignedFixedUintV1<F: OfflineCashPoseidonFieldV1> {
    value: AssignedValue<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_fixed_uint_v1<F: OfflineCashPoseidonFieldV1>(
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
fn assign_fixed_digest_v1<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: DigestV1,
) -> [PastaSha256ByteV1<F>; 32] {
    assign_bytes(ctx, range, &digest)
        .try_into()
        .expect("fixed digest width")
}

#[cfg(feature = "zk-halo2-ipa")]
fn assigned_limbs_to_bytes_v1<F: OfflineCashPoseidonFieldV1>(
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
fn assigned_value_to_bytes_v1<F: OfflineCashPoseidonFieldV1>(
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
fn digest_nonzero_from_limbs_v1<F: OfflineCashPoseidonFieldV1>(
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
fn select_digest_bytes_v1<F: OfflineCashPoseidonFieldV1>(
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
fn constrain_equal_if_v1<F: OfflineCashPoseidonFieldV1>(
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
fn constrain_zero_if_v1<F: OfflineCashPoseidonFieldV1>(
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
pub(crate) fn constrain_enabled_hardware_profile_membership_v1<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    hidden_profile: [AssignedValue<F>; 2],
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
) {
    let gate = range.gate();
    let mut matched = ctx.load_constant(F::ZERO);
    for profile in enabled_profiles {
        let limbs = crate::zk::offline_cash_v1_poseidon::digest_limbs::<F>(*profile);
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
fn constrain_digest_limbs_equal_v1<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    actual: &[PastaSha256ByteV1<F>; 32],
    expected: [AssignedValue<F>; 2],
) {
    for (actual, expected) in digest_limbs_assigned(ctx, actual).into_iter().zip(expected) {
        ctx.constrain_equal(&actual, &expected);
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_less_than_if_v1<F: OfflineCashPoseidonFieldV1>(
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
fn constrain_not_less_than_if_v1<F: OfflineCashPoseidonFieldV1>(
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

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_terminal_commit_semantics_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    public: &[AssignedValue<F>],
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    guard: &OfflineCashAssignedGuardBundleV1<F>,
) -> Result<(), String> {
    if public.len() != COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1 {
        return Err("terminal wrapper public prefix is truncated".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let raw_send = gate.is_equal(
        ctx,
        public[public_instance::OPERATION],
        QuantumCell::Constant(F::from(2)),
    );
    let raw_redeem = gate.is_equal(
        ctx,
        public[public_instance::OPERATION],
        QuantumCell::Constant(F::from(4)),
    );
    let certificate_low_zero = gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO]);
    let certificate_high_zero =
        gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO + 1]);
    let certificate_zero = gate.and(ctx, certificate_low_zero, certificate_high_zero);
    let terminal_branch = gate.not(ctx, certificate_zero);
    let send = gate.and(ctx, raw_send, terminal_branch);
    let redeem = gate.and(ctx, raw_redeem, terminal_branch);
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

    let request = private.request.as_ref();
    let intent = private.acceptance_intent.as_ref();
    let ticket = private.acceptance_ticket.as_ref();
    let intent_version = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(intent.map_or(0, |value| value.version)),
        16,
    );
    constrain_equal_if_v1(ctx, &range, terminal_branch, intent_version.value, send);
    let intent_request = assign_fixed_digest_v1(
        ctx,
        &range,
        intent.map_or([0; 32], |value| value.request_digest),
    );
    for (actual, expected) in digest_limbs_assigned(ctx, &intent_request)
        .into_iter()
        .zip(&public[public_instance::REQUEST_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
    }
    let intent_id =
        assign_fixed_digest_v1(ctx, &range, intent.map_or([0; 32], |value| value.intent_id));
    let intent_id_limbs = digest_limbs_assigned(ctx, &intent_id);
    let intent_id_nonzero = digest_nonzero_from_limbs_v1(ctx, &range, intent_id_limbs);
    constrain_equal_if_v1(ctx, &range, terminal_branch, intent_id_nonzero, send);
    let intent_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        intent.map_or(0, |value| value.exact_amount),
        128,
    );
    let expected_intent_amount = gate.mul(ctx, public[public_instance::AMOUNT], send);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        intent_amount.value,
        expected_intent_amount,
    );
    let intent_sender_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        intent.map_or([0; 32], |value| value.sender_one_time_commitment),
    );
    let intent_sender_limbs = digest_limbs_assigned(ctx, &intent_sender_commitment);
    let sender_commitment_present = digest_nonzero_from_limbs_v1(ctx, &range, intent_sender_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        sender_commitment_present,
        send,
    );

    let request_issued = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(request.map_or(0, |value| value.issued_at_ms)),
        64,
    );
    let request_expires = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(request.map_or(0, |value| value.expires_at_ms)),
        64,
    );
    let ticket_version = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(ticket.map_or(0, |value| value.version)),
        16,
    );
    constrain_equal_if_v1(ctx, &range, terminal_branch, ticket_version.value, send);
    let ticket_request = assign_fixed_digest_v1(
        ctx,
        &range,
        ticket.map_or([0; 32], |value| value.request_digest),
    );
    for (actual, expected) in digest_limbs_assigned(ctx, &ticket_request)
        .into_iter()
        .zip(&public[public_instance::REQUEST_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
    }
    let ticket_intent = assign_fixed_digest_v1(
        ctx,
        &range,
        ticket.map_or([0; 32], |value| value.intent_digest),
    );
    let ticket_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        ticket.map_or(0, |value| value.exact_amount),
        128,
    );
    constrain_equal_if_v1(
        ctx,
        &range,
        terminal_branch,
        ticket_amount.value,
        expected_intent_amount,
    );
    let ticket_issued = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(ticket.map_or(0, |value| value.issued_at_ms)),
        64,
    );
    let ticket_expires = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(ticket.map_or(0, |value| value.expires_at_ms)),
        64,
    );

    let one_use_authorization =
        assign_fixed_digest_v1(ctx, &range, private.one_use_hardware_authorization);
    let one_use_authorization_limbs = digest_limbs_assigned(ctx, &one_use_authorization);
    let authorization_nonzero =
        digest_nonzero_from_limbs_v1(ctx, &range, one_use_authorization_limbs);
    gate.assert_is_const(ctx, &authorization_nonzero, &F::ONE);
    let authorization_counter_before =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_before, 128);
    let authorization_counter_after =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_after, 128);
    let incremented_counter = gate.inc(ctx, authorization_counter_before.value);
    ctx.constrain_equal(&incremented_counter, &authorization_counter_after.value);
    let predecessor_state_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_state);
    let predecessor_nonce_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_nonce);
    let lane_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.lane_id);
    let predecessor_epoch_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_epoch);
    let predecessor_key_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_key);
    let predecessor_sequence_bytes =
        assigned_value_to_bytes_v1(ctx, &range, guard.predecessor_sequence, 128);
    let journal_before_bytes = assigned_value_to_bytes_v1(ctx, &range, guard.journal_before, 128);
    let prepared_message = [
        constant_bytes(PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        vec![operation_byte],
        one_use_authorization.to_vec(),
        predecessor_state_bytes,
        predecessor_nonce_bytes,
        lane_bytes,
        predecessor_epoch_bytes,
        predecessor_key_bytes,
        predecessor_sequence_bytes,
        journal_before_bytes,
        authorization_counter_before.bytes.clone(),
    ]
    .concat();
    let prepared_authorization = hash(ctx, jobs, prepared_message)?;
    let conflict_nullifier = hash(
        ctx,
        jobs,
        [
            constant_bytes(PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1),
            constant_bytes(&1_u16.to_le_bytes()),
            prepared_authorization.to_vec(),
        ]
        .concat(),
    )?;
    for (actual, expected) in digest_limbs_assigned(ctx, &conflict_nullifier)
        .into_iter()
        .zip(&public[public_instance::TRANSITION_NULLIFIER_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, *expected);
    }

    let sender_opening = assign_fixed_digest_v1(ctx, &range, private.sender_one_time_opening);
    let sender_opening_limbs = digest_limbs_assigned(ctx, &sender_opening);
    let sender_opening_nonzero = digest_nonzero_from_limbs_v1(ctx, &range, sender_opening_limbs);
    constrain_equal_if_v1(ctx, &range, terminal_branch, sender_opening_nonzero, send);
    let sender_commitment_message = [
        constant_bytes(SENDER_ONE_TIME_COMMITMENT_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        sender_opening.to_vec(),
        prepared_authorization.to_vec(),
        intent_request.to_vec(),
        intent_id.to_vec(),
        intent_amount.bytes.clone(),
    ]
    .concat();
    let computed_sender_commitment = hash(ctx, jobs, sender_commitment_message)?;
    let computed_sender_limbs = digest_limbs_assigned(ctx, &computed_sender_commitment);
    let intent_sender_limbs = digest_limbs_assigned(ctx, &intent_sender_commitment);
    for (actual, expected) in computed_sender_limbs.into_iter().zip(intent_sender_limbs) {
        constrain_equal_if_v1(ctx, &range, send, actual, expected);
    }

    let intent_message = [
        constant_bytes(ACCEPTANCE_INTENT_DIGEST_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(ACCEPTANCE_INTENT_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        intent_version.bytes,
        intent_request.to_vec(),
        intent_id.to_vec(),
        intent_amount.bytes,
        intent_sender_commitment.to_vec(),
    ]
    .concat();
    let computed_intent_digest = hash(ctx, jobs, intent_message)?;
    let selected_intent_digest = select_digest_bytes_v1(ctx, &range, &computed_intent_digest, send);
    let selected_intent_limbs = digest_limbs_assigned(ctx, &selected_intent_digest);
    for (actual, expected) in digest_limbs_assigned(ctx, &ticket_intent)
        .into_iter()
        .zip(selected_intent_limbs)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }

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
    let payment_min =
        ctx.load_constant(F::from(u64::from(OFFLINE_CASH_PAYMENT_OUTBOX_MIN_BYTES_V1)));
    let redemption_min = ctx.load_constant(F::from(u64::from(
        OFFLINE_CASH_REDEMPTION_OUTBOX_MIN_BYTES_V1,
    )));
    let minimum = gate.select(ctx, payment_min, redemption_min, send);
    let reservation_too_small = range.is_less_than(ctx, reserved_outbox_bytes.value, minimum, 32);
    constrain_zero_if_v1(ctx, &range, terminal_branch, reservation_too_small);
    let outbound = gate.add(ctx, send, redeem);
    constrain_less_than_if_v1(
        ctx,
        &range,
        outbound,
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
    let trusted_send = gate.and(ctx, trusted, send);
    for (issued, expires) in [
        (request_issued.value, request_expires.value),
        (ticket_issued.value, ticket_expires.value),
    ] {
        constrain_not_less_than_if_v1(
            ctx,
            &range,
            trusted_send,
            trusted_commit_time.value,
            issued,
            64,
        );
        constrain_less_than_if_v1(
            ctx,
            &range,
            trusted_send,
            trusted_commit_time.value,
            expires,
            64,
        );
    }
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
    let lease_send = gate.and(ctx, lease, send);
    for (issued, expires) in [
        (request_issued.value, request_expires.value),
        (ticket_issued.value, ticket_expires.value),
    ] {
        constrain_not_less_than_if_v1(ctx, &range, lease_send, lease_valid_from.value, issued, 64);
        constrain_not_less_than_if_v1(ctx, &range, lease_send, expires, lease_expires.value, 64);
    }
    constrain_less_than_if_v1(
        ctx,
        &range,
        send,
        request_issued.value,
        request_expires.value,
        64,
    );
    constrain_less_than_if_v1(
        ctx,
        &range,
        send,
        ticket_issued.value,
        ticket_expires.value,
        64,
    );
    constrain_not_less_than_if_v1(
        ctx,
        &range,
        send,
        ticket_issued.value,
        request_issued.value,
        64,
    );
    constrain_not_less_than_if_v1(
        ctx,
        &range,
        send,
        request_expires.value,
        ticket_expires.value,
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
    let ticket_digest_bytes = assigned_limbs_to_bytes_v1(
        ctx,
        &range,
        [
            public[public_instance::ACCEPTANCE_TICKET_LO],
            public[public_instance::ACCEPTANCE_TICKET_LO + 1],
        ],
    );
    let amount_bytes =
        assigned_value_to_bytes_v1(ctx, &range, public[public_instance::AMOUNT], 128);
    let precommit_message = [
        constant_bytes(PRECOMMIT_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        lifecycle_bytes.clone(),
        request_digest_bytes.clone(),
        selected_intent_digest.to_vec(),
        ticket_digest_bytes.clone(),
        amount_bytes.clone(),
        reservation_commitment.to_vec(),
        prepared_authorization.to_vec(),
    ]
    .concat();
    let precommit = hash(ctx, jobs, precommit_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &precommit)
        .into_iter()
        .zip(guard.precommit_binding_digest)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }

    let sender_authorization_message = [
        constant_bytes(SENDER_TERMINAL_AUTHORIZATION_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        selected_intent_digest.to_vec(),
        intent_sender_commitment.to_vec(),
        prepared_authorization.to_vec(),
        ticket_digest_bytes.clone(),
    ]
    .concat();
    let sender_authorization = hash(ctx, jobs, sender_authorization_message)?;
    let selected_sender_authorization =
        select_digest_bytes_v1(ctx, &range, &sender_authorization, send);
    for (actual, expected) in digest_limbs_assigned(ctx, &selected_sender_authorization)
        .into_iter()
        .zip(guard.sender_one_time_authorization_digest)
    {
        constrain_equal_if_v1(ctx, &range, terminal_branch, actual, expected);
    }

    let terminal_envelope = assign_fixed_digest_v1(ctx, &range, private.terminal_envelope_digest);
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
        precommit.to_vec(),
        candidate_envelope.to_vec(),
        certificate_digest.to_vec(),
        certificate_id.to_vec(),
        hardware_terminal.to_vec(),
        transition_nullifier.to_vec(),
        reservation_commitment.to_vec(),
        evidence_kind.bytes,
        evidence_commitment.to_vec(),
        request_digest_bytes,
        ticket_digest_bytes,
        amount_bytes,
        profile_bytes,
        policy_epoch_bytes,
        request_issued.bytes,
        request_expires.bytes,
        ticket_issued.bytes,
        ticket_expires.bytes,
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
fn constrain_intent_authorization_semantics_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    public: &[AssignedValue<F>],
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    authorization: Option<&OfflineCashCommitWrapperIntentAuthorizationPrivateV1>,
    guard: &OfflineCashAssignedGuardBundleV1<F>,
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
) -> Result<(), String> {
    if public.len() != COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1 {
        return Err("intent authorization public prefix is truncated".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let certificate_low_zero = gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO]);
    let certificate_high_zero =
        gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO + 1]);
    let certificate_zero = gate.and(ctx, certificate_low_zero, certificate_high_zero);
    let nullifier_low_zero = gate.is_zero(ctx, public[public_instance::TRANSITION_NULLIFIER_LO]);
    let nullifier_high_zero =
        gate.is_zero(ctx, public[public_instance::TRANSITION_NULLIFIER_LO + 1]);
    let nullifier_zero = gate.and(ctx, nullifier_low_zero, nullifier_high_zero);
    let authorization_branch = gate.and(ctx, certificate_zero, nullifier_zero);
    let statement = authorization.map(|value| &value.statement);

    let statement_version = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(statement.map_or(0, |value| value.version)),
        16,
    );
    let intent_version = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(statement.map_or(0, |value| value.intent.version)),
        16,
    );
    let one = ctx.load_constant(F::ONE);
    constrain_equal_if_v1(
        ctx,
        &range,
        authorization_branch,
        statement_version.value,
        one,
    );
    constrain_equal_if_v1(ctx, &range, authorization_branch, intent_version.value, one);
    let intent_request = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.intent.request_digest),
    );
    let intent_id = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.intent.intent_id),
    );
    let intent_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        statement.map_or(0, |value| value.intent.exact_amount),
        128,
    );
    let sender_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.intent.sender_one_time_commitment),
    );
    let statement_release = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.release_id),
    );
    let statement_suite = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.suite_id),
    );
    let statement_vk = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.vk_digest),
    );
    let statement_manifest = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.artifact_manifest_digest),
    );
    for (actual, offset) in [
        (&intent_request, public_instance::REQUEST_LO),
        (&statement_release, public_instance::RELEASE_LO),
        (&statement_suite, public_instance::SUITE_LO),
        (&statement_vk, public_instance::VK_LO),
        (&statement_manifest, public_instance::CIPHERTEXT_LO),
    ] {
        for (actual, expected) in digest_limbs_assigned(ctx, actual)
            .into_iter()
            .zip(&public[offset..offset + 2])
        {
            constrain_equal_if_v1(ctx, &range, authorization_branch, actual, *expected);
        }
    }
    constrain_equal_if_v1(
        ctx,
        &range,
        authorization_branch,
        intent_amount.value,
        public[public_instance::AMOUNT],
    );
    for digest in [&intent_id, &sender_commitment, &statement_manifest] {
        let limbs = digest_limbs_assigned(ctx, digest);
        let nonzero = digest_nonzero_from_limbs_v1(ctx, &range, limbs);
        constrain_equal_if_v1(
            ctx,
            &range,
            authorization_branch,
            nonzero,
            authorization_branch,
        );
    }

    let intent_message = [
        constant_bytes(ACCEPTANCE_INTENT_DIGEST_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(ACCEPTANCE_INTENT_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        intent_version.bytes.clone(),
        intent_request.to_vec(),
        intent_id.to_vec(),
        intent_amount.bytes.clone(),
        sender_commitment.to_vec(),
    ]
    .concat();
    let intent_digest = hash(ctx, jobs, intent_message)?;
    let statement_message = [
        constant_bytes(ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_DIGEST_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(
            &(ACCEPTANCE_INTENT_AUTHORIZATION_STATEMENT_CANONICAL_BYTES_V1 as u64).to_le_bytes(),
        ),
        statement_version.bytes,
        intent_version.bytes,
        intent_request.to_vec(),
        intent_id.to_vec(),
        intent_amount.bytes.clone(),
        sender_commitment.to_vec(),
        statement_release.to_vec(),
        statement_suite.to_vec(),
        statement_vk.to_vec(),
        statement_manifest.to_vec(),
    ]
    .concat();
    let semantic_digest = hash(ctx, jobs, statement_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &semantic_digest)
        .into_iter()
        .zip(&public[public_instance::SEMANTIC_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, authorization_branch, actual, *expected);
    }

    for (actual, expected) in [
        (guard.operation, public[public_instance::OPERATION]),
        (
            guard.protocol_version,
            public[public_instance::PROTOCOL_VERSION],
        ),
        (guard.amount, public[public_instance::AMOUNT]),
        (guard.asset_scale, public[public_instance::ASSET_SCALE]),
    ] {
        constrain_equal_if_v1(ctx, &range, authorization_branch, actual, expected);
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
        (guard.lifecycle_binding_digest, public_instance::SEMANTIC_LO),
        (guard.transition_intent, public_instance::SEMANTIC_LO),
    ] {
        for (actual, expected) in actual.into_iter().zip(&public[offset..offset + 2]) {
            constrain_equal_if_v1(ctx, &range, authorization_branch, actual, *expected);
        }
    }
    let guard_policy_zero = gate.is_zero(ctx, guard.policy_epoch);
    constrain_zero_if_v1(ctx, &range, authorization_branch, guard_policy_zero);
    for value in guard
        .terminal_commit_binding_digest
        .into_iter()
        .chain(guard.sender_one_time_authorization_digest)
    {
        constrain_zero_if_v1(ctx, &range, authorization_branch, value);
    }
    constrain_enabled_hardware_profile_membership_v1(
        ctx,
        &range,
        authorization_branch,
        guard.hardware_profile_id,
        enabled_profiles,
    );

    let one_use_authorization =
        assign_fixed_digest_v1(ctx, &range, private.one_use_hardware_authorization);
    let one_use_limbs = digest_limbs_assigned(ctx, &one_use_authorization);
    let one_use_nonzero = digest_nonzero_from_limbs_v1(ctx, &range, one_use_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        authorization_branch,
        one_use_nonzero,
        authorization_branch,
    );
    let authorization_counter_before =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_before, 128);
    let authorization_counter_after =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_after, 128);
    let incremented = gate.inc(ctx, authorization_counter_before.value);
    constrain_equal_if_v1(
        ctx,
        &range,
        authorization_branch,
        incremented,
        authorization_counter_after.value,
    );
    let operation_byte =
        PastaSha256ByteV1::range_checked(ctx, &range, public[public_instance::OPERATION]);
    let predecessor_state_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_state);
    let predecessor_nonce_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_nonce);
    let lane_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.lane_id);
    let predecessor_epoch_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_epoch);
    let predecessor_key_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_key);
    let predecessor_sequence_bytes =
        assigned_value_to_bytes_v1(ctx, &range, guard.predecessor_sequence, 128);
    let journal_before_bytes = assigned_value_to_bytes_v1(ctx, &range, guard.journal_before, 128);
    let prepared_message = [
        constant_bytes(PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        vec![operation_byte],
        one_use_authorization.to_vec(),
        predecessor_state_bytes,
        predecessor_nonce_bytes,
        lane_bytes,
        predecessor_epoch_bytes,
        predecessor_key_bytes,
        predecessor_sequence_bytes,
        journal_before_bytes,
        authorization_counter_before.bytes,
    ]
    .concat();
    let prepared_authorization = hash(ctx, jobs, prepared_message)?;
    let sender_opening = assign_fixed_digest_v1(ctx, &range, private.sender_one_time_opening);
    let opening_limbs = digest_limbs_assigned(ctx, &sender_opening);
    let opening_nonzero = digest_nonzero_from_limbs_v1(ctx, &range, opening_limbs);
    constrain_equal_if_v1(
        ctx,
        &range,
        authorization_branch,
        opening_nonzero,
        authorization_branch,
    );
    let sender_message = [
        constant_bytes(SENDER_ONE_TIME_COMMITMENT_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        sender_opening.to_vec(),
        prepared_authorization.to_vec(),
        intent_request.to_vec(),
        intent_id.to_vec(),
        intent_amount.bytes,
    ]
    .concat();
    let computed_sender_commitment = hash(ctx, jobs, sender_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &computed_sender_commitment)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, &sender_commitment))
    {
        constrain_equal_if_v1(ctx, &range, authorization_branch, actual, expected);
    }

    let lifecycle_bytes = assigned_limbs_to_bytes_v1(
        ctx,
        &range,
        [
            public[public_instance::SEMANTIC_LO],
            public[public_instance::SEMANTIC_LO + 1],
        ],
    );
    let request_bytes = assigned_limbs_to_bytes_v1(
        ctx,
        &range,
        [
            public[public_instance::REQUEST_LO],
            public[public_instance::REQUEST_LO + 1],
        ],
    );
    let amount_bytes =
        assigned_value_to_bytes_v1(ctx, &range, public[public_instance::AMOUNT], 128);
    let reservation_bytes = assigned_limbs_to_bytes_v1(ctx, &range, guard.durable_outbox_effect);
    let zero_ticket = constant_bytes(&[0; 32]);
    let precommit_message = [
        constant_bytes(PRECOMMIT_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        lifecycle_bytes,
        request_bytes,
        intent_digest.to_vec(),
        zero_ticket,
        amount_bytes,
        reservation_bytes,
        prepared_authorization.to_vec(),
    ]
    .concat();
    let precommit = hash(ctx, jobs, precommit_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &precommit)
        .into_iter()
        .zip(guard.precommit_binding_digest)
    {
        constrain_equal_if_v1(ctx, &range, authorization_branch, actual, expected);
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_lines)]
fn constrain_no_commit_closure_semantics_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    public: &[AssignedValue<F>],
    private: &OfflineCashCommitWrapperPrivateTransitionV1,
    closure: Option<&OfflineCashCommitWrapperNoCommitClosurePrivateV1>,
    guard: &OfflineCashAssignedGuardBundleV1<F>,
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
) -> Result<(), String> {
    if public.len() != COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1 {
        return Err("no-commit closure public prefix is truncated".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let certificate_low_zero = gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO]);
    let certificate_high_zero =
        gate.is_zero(ctx, public[public_instance::COMMIT_CERTIFICATE_LO + 1]);
    let certificate_zero = gate.and(ctx, certificate_low_zero, certificate_high_zero);
    let nullifier_low_zero = gate.is_zero(ctx, public[public_instance::TRANSITION_NULLIFIER_LO]);
    let nullifier_high_zero =
        gate.is_zero(ctx, public[public_instance::TRANSITION_NULLIFIER_LO + 1]);
    let nullifier_zero = gate.and(ctx, nullifier_low_zero, nullifier_high_zero);
    let nullifier_nonzero = gate.not(ctx, nullifier_zero);
    let closure_branch = gate.and(ctx, certificate_zero, nullifier_nonzero);
    let statement = closure.map(|value| &value.statement);

    let version = assign_fixed_uint_v1(
        ctx,
        &range,
        u128::from(statement.map_or(0, |value| value.version)),
        16,
    );
    let one = ctx.load_constant(F::ONE);
    constrain_equal_if_v1(ctx, &range, closure_branch, version.value, one);
    let release = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.release_id),
    );
    let suite = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.suite_id),
    );
    let vk = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.vk_digest),
    );
    let manifest = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.artifact_manifest_digest),
    );
    let hardware_binding = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.sender_hardware_binding_commitment),
    );
    let request_id = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.request_id),
    );
    let request_digest = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.request_digest),
    );
    let ticket_id = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.acceptance_ticket_id),
    );
    let ticket_digest = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.ticket_digest),
    );
    let intent_authorization_digest = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.intent_authorization_digest),
    );
    let intent_digest = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.intent_digest),
    );
    let exact_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        statement.map_or(0, |value| value.exact_amount),
        128,
    );
    let sender_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.sender_one_time_commitment),
    );
    let recovery_id = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.recovery_id),
    );
    let cancellation_nullifier = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.cancellation_nullifier),
    );
    let delivery_slot = assign_fixed_digest_v1(
        ctx,
        &range,
        statement.map_or([0; 32], |value| value.equivalent_delivery_slot_commitment),
    );
    for digest in [
        &release,
        &suite,
        &vk,
        &manifest,
        &hardware_binding,
        &request_id,
        &request_digest,
        &ticket_id,
        &ticket_digest,
        &intent_authorization_digest,
        &intent_digest,
        &sender_commitment,
        &recovery_id,
        &cancellation_nullifier,
        &delivery_slot,
    ] {
        let limbs = digest_limbs_assigned(ctx, digest);
        let nonzero = digest_nonzero_from_limbs_v1(ctx, &range, limbs);
        constrain_equal_if_v1(ctx, &range, closure_branch, nonzero, closure_branch);
    }
    constrain_equal_if_v1(
        ctx,
        &range,
        closure_branch,
        exact_amount.value,
        public[public_instance::AMOUNT],
    );

    let statement_message = [
        constant_bytes(NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(NO_COMMIT_CLOSURE_STATEMENT_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        version.bytes,
        release.to_vec(),
        suite.to_vec(),
        vk.to_vec(),
        manifest.to_vec(),
        hardware_binding.to_vec(),
        request_id.to_vec(),
        request_digest.to_vec(),
        ticket_id.to_vec(),
        ticket_digest.to_vec(),
        intent_authorization_digest.to_vec(),
        intent_digest.to_vec(),
        exact_amount.bytes.clone(),
        sender_commitment.to_vec(),
        recovery_id.to_vec(),
        cancellation_nullifier.to_vec(),
        delivery_slot.to_vec(),
    ]
    .concat();
    let semantic_digest = hash(ctx, jobs, statement_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &semantic_digest)
        .into_iter()
        .zip(&public[public_instance::SEMANTIC_LO..][..2])
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, *expected);
    }

    for (actual, expected) in [
        (&release, public_instance::RELEASE_LO),
        (&suite, public_instance::SUITE_LO),
        (&vk, public_instance::VK_LO),
        (&manifest, public_instance::LIFECYCLE_LO),
        (&hardware_binding, public_instance::HARDWARE_PROFILE_LO),
        (&request_id, public_instance::NETWORK_LO),
        (&ticket_id, public_instance::ASSET_LO),
        (&intent_digest, public_instance::LIABILITY_POOL_LO),
        (&request_digest, public_instance::REQUEST_LO),
        (&sender_commitment, public_instance::CIPHERTEXT_LO),
        (
            &cancellation_nullifier,
            public_instance::TRANSITION_NULLIFIER_LO,
        ),
        (&delivery_slot, public_instance::OUTPUT_BINDING_LO),
    ] {
        for (actual, expected) in digest_limbs_assigned(ctx, actual)
            .into_iter()
            .zip(&public[expected..expected + 2])
        {
            constrain_equal_if_v1(ctx, &range, closure_branch, actual, *expected);
        }
    }

    let request = private.request.as_ref();
    let intent = private.acceptance_intent.as_ref();
    let ticket = private.acceptance_ticket.as_ref();
    let authorization = closure.map(|value| &value.intent_authorization.statement);
    let private_request_id = assign_fixed_digest_v1(
        ctx,
        &range,
        request.map_or([0; 32], |value| value.request_id),
    );
    let receiver_lane_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        request.map_or([0; 32], |value| value.hardware_credential.lane_commitment),
    );
    let private_intent_request = assign_fixed_digest_v1(
        ctx,
        &range,
        intent.map_or([0; 32], |value| value.request_digest),
    );
    let private_intent_id =
        assign_fixed_digest_v1(ctx, &range, intent.map_or([0; 32], |value| value.intent_id));
    let private_intent_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        intent.map_or(0, |value| value.exact_amount),
        128,
    );
    let private_sender_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        intent.map_or([0; 32], |value| value.sender_one_time_commitment),
    );
    let private_ticket_id = assign_fixed_digest_v1(
        ctx,
        &range,
        ticket.map_or([0; 32], |value| value.acceptance_ticket_id),
    );
    let private_ticket_request = assign_fixed_digest_v1(
        ctx,
        &range,
        ticket.map_or([0; 32], |value| value.request_digest),
    );
    let private_ticket_intent = assign_fixed_digest_v1(
        ctx,
        &range,
        ticket.map_or([0; 32], |value| value.intent_digest),
    );
    let private_ticket_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        ticket.map_or(0, |value| value.exact_amount),
        128,
    );
    let authorization_request = assign_fixed_digest_v1(
        ctx,
        &range,
        authorization.map_or([0; 32], |value| value.intent.request_digest),
    );
    let authorization_intent_id = assign_fixed_digest_v1(
        ctx,
        &range,
        authorization.map_or([0; 32], |value| value.intent.intent_id),
    );
    let authorization_amount = assign_fixed_uint_v1(
        ctx,
        &range,
        authorization.map_or(0, |value| value.intent.exact_amount),
        128,
    );
    let authorization_sender_commitment = assign_fixed_digest_v1(
        ctx,
        &range,
        authorization.map_or([0; 32], |value| value.intent.sender_one_time_commitment),
    );
    for (left, right) in [
        (&private_request_id, &request_id),
        (&private_intent_request, &request_digest),
        (&private_ticket_id, &ticket_id),
        (&private_ticket_request, &request_digest),
        (&private_ticket_intent, &intent_digest),
        (&private_sender_commitment, &sender_commitment),
        (&authorization_request, &request_digest),
        (&authorization_intent_id, &private_intent_id),
        (&authorization_sender_commitment, &sender_commitment),
    ] {
        for (left, right) in digest_limbs_assigned(ctx, left)
            .into_iter()
            .zip(digest_limbs_assigned(ctx, right))
        {
            constrain_equal_if_v1(ctx, &range, closure_branch, left, right);
        }
    }
    for value in [private_intent_id.clone()] {
        let limbs = digest_limbs_assigned(ctx, &value);
        let nonzero = digest_nonzero_from_limbs_v1(ctx, &range, limbs);
        constrain_equal_if_v1(ctx, &range, closure_branch, nonzero, closure_branch);
    }
    for value in [
        private_intent_amount.value,
        private_ticket_amount.value,
        authorization_amount.value,
    ] {
        constrain_equal_if_v1(ctx, &range, closure_branch, value, exact_amount.value);
    }
    for (authorization_value, statement_value) in [
        (
            authorization.map_or([0; 32], |value| value.release_id),
            &release,
        ),
        (
            authorization.map_or([0; 32], |value| value.suite_id),
            &suite,
        ),
        (authorization.map_or([0; 32], |value| value.vk_digest), &vk),
        (
            authorization.map_or([0; 32], |value| value.artifact_manifest_digest),
            &manifest,
        ),
    ] {
        let authorization_value = assign_fixed_digest_v1(ctx, &range, authorization_value);
        for (left, right) in digest_limbs_assigned(ctx, &authorization_value)
            .into_iter()
            .zip(digest_limbs_assigned(ctx, statement_value))
        {
            constrain_equal_if_v1(ctx, &range, closure_branch, left, right);
        }
    }

    let one_use_authorization =
        assign_fixed_digest_v1(ctx, &range, private.one_use_hardware_authorization);
    let sender_opening = assign_fixed_digest_v1(ctx, &range, private.sender_one_time_opening);
    for digest in [&one_use_authorization, &sender_opening] {
        let limbs = digest_limbs_assigned(ctx, digest);
        let nonzero = digest_nonzero_from_limbs_v1(ctx, &range, limbs);
        constrain_equal_if_v1(ctx, &range, closure_branch, nonzero, closure_branch);
    }
    let authorization_counter_before =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_before, 128);
    let authorization_counter_after =
        assign_fixed_uint_v1(ctx, &range, private.authorization_counter_after, 128);
    let expected_counter_after = gate.inc(ctx, authorization_counter_before.value);
    constrain_equal_if_v1(
        ctx,
        &range,
        closure_branch,
        authorization_counter_after.value,
        expected_counter_after,
    );
    let operation_byte =
        PastaSha256ByteV1::range_checked(ctx, &range, public[public_instance::OPERATION]);
    let prepared_message = [
        constant_bytes(PREPARED_ONE_USE_AUTHORIZATION_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        vec![operation_byte],
        one_use_authorization.to_vec(),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_state),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_nonce),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.lane_id),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_epoch),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_key),
        assigned_value_to_bytes_v1(ctx, &range, guard.predecessor_sequence, 128),
        assigned_value_to_bytes_v1(ctx, &range, guard.journal_before, 128),
        authorization_counter_before.bytes,
    ]
    .concat();
    let prepared = hash(ctx, jobs, prepared_message)?;
    let expected_sender_message = [
        constant_bytes(SENDER_ONE_TIME_COMMITMENT_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        sender_opening.to_vec(),
        prepared.to_vec(),
        request_digest.to_vec(),
        private_intent_id.to_vec(),
        exact_amount.bytes.clone(),
    ]
    .concat();
    let expected_sender = hash(ctx, jobs, expected_sender_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &expected_sender)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, &sender_commitment))
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }

    let expected_intent_message = [
        constant_bytes(ACCEPTANCE_INTENT_DIGEST_DOMAIN_V1),
        constant_bytes(&[0]),
        constant_bytes(&(ACCEPTANCE_INTENT_CANONICAL_BYTES_V1 as u64).to_le_bytes()),
        constant_bytes(&1_u16.to_le_bytes()),
        request_digest.to_vec(),
        private_intent_id.to_vec(),
        exact_amount.bytes.clone(),
        sender_commitment.to_vec(),
    ]
    .concat();
    let expected_intent = hash(ctx, jobs, expected_intent_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &expected_intent)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, &intent_digest))
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }

    let hardware_recovery_nonce = assign_fixed_digest_v1(
        ctx,
        &range,
        closure.map_or([0; 32], |value| value.hardware_recovery_nonce),
    );
    for digest in [&receiver_lane_commitment, &hardware_recovery_nonce] {
        let limbs = digest_limbs_assigned(ctx, digest);
        let nonzero = digest_nonzero_from_limbs_v1(ctx, &range, limbs);
        constrain_equal_if_v1(ctx, &range, closure_branch, nonzero, closure_branch);
    }
    let recovery_id_message = [
        constant_bytes(NO_COMMIT_RECOVERY_ID_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        prepared.to_vec(),
        request_digest.to_vec(),
        ticket_digest.to_vec(),
        receiver_lane_commitment.to_vec(),
        hardware_recovery_nonce.to_vec(),
    ]
    .concat();
    let expected_recovery_id = hash(ctx, jobs, recovery_id_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &expected_recovery_id)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, &recovery_id))
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }

    let cancellation_successor_message = [
        constant_bytes(NO_COMMIT_CANCELLATION_SUCCESSOR_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        prepared.to_vec(),
        recovery_id.to_vec(),
        intent_authorization_digest.to_vec(),
        ticket_digest.to_vec(),
        delivery_slot.to_vec(),
        assigned_value_to_bytes_v1(ctx, &range, guard.journal_after, 128),
        authorization_counter_after.bytes,
    ]
    .concat();
    let cancellation_successor = hash(ctx, jobs, cancellation_successor_message)?;
    let prepared_limbs = digest_limbs_assigned(ctx, &prepared);
    let successor_limbs = digest_limbs_assigned(ctx, &cancellation_successor);
    let same_low = gate.is_equal(ctx, prepared_limbs[0], successor_limbs[0]);
    let same_high = gate.is_equal(ctx, prepared_limbs[1], successor_limbs[1]);
    let same_successor = gate.and(ctx, same_low, same_high);
    constrain_zero_if_v1(ctx, &range, closure_branch, same_successor);
    for (actual, expected) in successor_limbs
        .into_iter()
        .zip(guard.sender_one_time_authorization_digest)
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }
    let cancellation_nullifier_message = [
        constant_bytes(PREDECESSOR_CONFLICT_NULLIFIER_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        prepared.to_vec(),
    ]
    .concat();
    let expected_nullifier = hash(ctx, jobs, cancellation_nullifier_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &expected_nullifier)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, &cancellation_nullifier))
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }

    let precommit_message = [
        constant_bytes(PRECOMMIT_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        semantic_digest.to_vec(),
        request_digest.to_vec(),
        intent_digest.to_vec(),
        ticket_digest.to_vec(),
        exact_amount.bytes,
        delivery_slot.to_vec(),
        prepared.to_vec(),
    ]
    .concat();
    let expected_precommit = hash(ctx, jobs, precommit_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &expected_precommit)
        .into_iter()
        .zip(guard.precommit_binding_digest)
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }

    let hardware_binding_message = [
        constant_bytes(NO_COMMIT_HARDWARE_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.hardware_profile_id),
        assigned_value_to_bytes_v1(ctx, &range, guard.policy_epoch, 64),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.lane_id),
        assigned_value_to_bytes_v1(ctx, &range, guard.predecessor_generation, 128),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_epoch),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_key),
        assigned_limbs_to_bytes_v1(ctx, &range, guard.predecessor_policy),
    ]
    .concat();
    let expected_hardware_binding = hash(ctx, jobs, hardware_binding_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &expected_hardware_binding)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, &hardware_binding))
    {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }
    constrain_enabled_hardware_profile_membership_v1(
        ctx,
        &range,
        closure_branch,
        guard.hardware_profile_id,
        enabled_profiles,
    );

    let zero = ctx.load_constant(F::ZERO);
    for (actual, expected) in [
        (guard.operation, public[public_instance::OPERATION]),
        (
            guard.protocol_version,
            public[public_instance::PROTOCOL_VERSION],
        ),
        (guard.amount, zero),
    ] {
        constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
    }
    for (actual, expected) in [
        (guard.release_id, &release),
        (guard.predecessor_suite_id, &suite),
        (guard.successor_suite_id, &suite),
        (guard.predecessor_vk_digest, &vk),
        (guard.successor_vk_digest, &vk),
        (guard.lifecycle_binding_digest, &semantic_digest),
        (guard.transition_intent, &recovery_id),
        (guard.transition_effect, &intent_authorization_digest),
        (guard.recovery_record, &ticket_digest),
        (guard.durable_outbox_effect, &delivery_slot),
    ] {
        for (actual, expected) in actual.into_iter().zip(digest_limbs_assigned(ctx, expected)) {
            constrain_equal_if_v1(ctx, &range, closure_branch, actual, expected);
        }
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_history_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &halo2_base::gates::RangeChip<F>,
    history: &[u8; super::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
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
        return Err("commit wrapper history has wrong fixed shape".to_owned());
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
    C::ScalarExt: OfflineCashPoseidonFieldV1,
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
    wrapper: &[AssignedValue<C::ScalarExt>],
    guard: &OfflineCashAssignedGuardBundleV1<C::ScalarExt>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || wrapper.len() != COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1
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
        let expected = crate::zk::offline_cash_v1_poseidon::digest_limbs::<C::ScalarExt>(digest);
        for (actual, expected) in candidate[offset..offset + 2].iter().zip(expected) {
            let constant = loader.ctx_mut().main().load_constant(expected);
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &constant);
        }
    }
    let gate = halo2_base::gates::GateChip::default();
    let certificate_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::COMMIT_CERTIFICATE_LO],
    );
    let certificate_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::COMMIT_CERTIFICATE_LO + 1],
    );
    let certificate_zero = gate.and(
        loader.ctx_mut().main(),
        certificate_low_zero,
        certificate_high_zero,
    );
    let nullifier_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::TRANSITION_NULLIFIER_LO],
    );
    let nullifier_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::TRANSITION_NULLIFIER_LO + 1],
    );
    let nullifier_zero = gate.and(
        loader.ctx_mut().main(),
        nullifier_low_zero,
        nullifier_high_zero,
    );
    let nullifier_nonzero = gate.not(loader.ctx_mut().main(), nullifier_zero);
    let closure = gate.and(loader.ctx_mut().main(), certificate_zero, nullifier_nonzero);
    let ordinary = gate.not(loader.ctx_mut().main(), closure);
    let constrain_selected = |loader: &DeferredLoader<'chip, C>,
                              selector: AssignedValue<C::ScalarExt>,
                              left: AssignedValue<C::ScalarExt>,
                              right: AssignedValue<C::ScalarExt>| {
        let difference = gate.sub(loader.ctx_mut().main(), left, right);
        let selected = gate.mul(loader.ctx_mut().main(), selector, difference);
        gate.assert_is_const(loader.ctx_mut().main(), &selected, &C::ScalarExt::ZERO);
    };
    for (index, expected) in [
        (state_relation::public_instance::OPERATION, guard.operation),
        (state_relation::public_instance::AMOUNT, guard.amount),
        (
            state_relation::public_instance::RECEIVE_ACTIVE_COUNT,
            guard.receive_active_count,
        ),
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
        constrain_selected(loader, ordinary, *candidate[index].assigned(), expected);
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
            state_relation::public_instance::PEER_RECIPIENT_LANE_LO,
            guard.peer_recipient_lane_id,
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
            state_relation::public_instance::PRECOMMIT_LO,
            guard.precommit_binding_digest,
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
            state_relation::public_instance::SUITE_UPGRADE_AUTHORIZATION_LO,
            guard.suite_upgrade_authorization_digest,
        ),
        (
            state_relation::public_instance::RECEIVE_BATCH_BINDING_LO,
            guard.receive_batch_binding_digest,
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
            constrain_selected(loader, ordinary, *actual.assigned(), expected);
        }
    }

    for (index, expected) in [
        (
            state_relation::public_instance::OPERATION,
            wrapper[public_instance::OPERATION],
        ),
        (
            state_relation::public_instance::AMOUNT,
            wrapper[public_instance::AMOUNT],
        ),
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
        constrain_selected(loader, closure, *candidate[index].assigned(), expected);
    }
    for (offset, expected) in [
        (
            state_relation::public_instance::PREDECESSOR_OUTER_LO,
            guard.predecessor_state,
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
            constrain_selected(loader, closure, *actual.assigned(), expected);
        }
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn constrain_candidate_projection_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    candidate: &[DeferredScalar<'chip, C>],
    wrapper: &[AssignedValue<C::ScalarExt>],
    candidate_protocol_digest: DigestV1,
    parity: OfflineCashPastaParityV1,
    sha_jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || wrapper.len() != COMMIT_WRAPPER_PUBLIC_PREFIX_COUNT_V1
    {
        return Err("commit wrapper candidate projection is truncated".to_owned());
    }
    let gate = halo2_base::gates::GateChip::default();
    let certificate_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::COMMIT_CERTIFICATE_LO],
    );
    let certificate_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::COMMIT_CERTIFICATE_LO + 1],
    );
    let certificate_zero = gate.and(
        loader.ctx_mut().main(),
        certificate_low_zero,
        certificate_high_zero,
    );
    let terminal = gate.not(loader.ctx_mut().main(), certificate_zero);
    let nullifier_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::TRANSITION_NULLIFIER_LO],
    );
    let nullifier_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        wrapper[public_instance::TRANSITION_NULLIFIER_LO + 1],
    );
    let nullifier_zero = gate.and(
        loader.ctx_mut().main(),
        nullifier_low_zero,
        nullifier_high_zero,
    );
    let nullifier_nonzero = gate.not(loader.ctx_mut().main(), nullifier_zero);
    let closure = gate.and(loader.ctx_mut().main(), certificate_zero, nullifier_nonzero);
    let ordinary = gate.not(loader.ctx_mut().main(), closure);
    let raw_send = gate.is_equal(
        loader.ctx_mut().main(),
        wrapper[public_instance::OPERATION],
        QuantumCell::Constant(C::ScalarExt::from(2)),
    );
    let terminal_send = gate.and(loader.ctx_mut().main(), terminal, raw_send);
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
    for (candidate_index, wrapper_index) in scalar_bindings {
        loader.ctx_mut().main().constrain_equal(
            &candidate[candidate_index].assigned(),
            &wrapper[wrapper_index],
        );
    }
    {
        let difference = gate.sub(
            loader.ctx_mut().main(),
            *candidate[state_relation::public_instance::ASSET_SCALE].assigned(),
            wrapper[public_instance::ASSET_SCALE],
        );
        let selected = gate.mul(loader.ctx_mut().main(), ordinary, difference);
        gate.assert_is_const(loader.ctx_mut().main(), &selected, &C::ScalarExt::ZERO);
    }
    {
        let difference = gate.sub(
            loader.ctx_mut().main(),
            *candidate[state_relation::public_instance::POLICY_EPOCH].assigned(),
            wrapper[public_instance::POLICY_EPOCH],
        );
        let selected = gate.mul(loader.ctx_mut().main(), terminal, difference);
        gate.assert_is_const(loader.ctx_mut().main(), &selected, &C::ScalarExt::ZERO);
    }
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
    for (candidate_offset, wrapper_offset) in always_digest_bindings {
        for limb in 0..2 {
            loader.ctx_mut().main().constrain_equal(
                &candidate[candidate_offset + limb].assigned(),
                &wrapper[wrapper_offset + limb],
            );
        }
    }
    let ordinary_digest_bindings = [
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
    for (candidate_offset, wrapper_offset) in ordinary_digest_bindings {
        for limb in 0..2 {
            let difference = gate.sub(
                loader.ctx_mut().main(),
                *candidate[candidate_offset + limb].assigned(),
                wrapper[wrapper_offset + limb],
            );
            let selected = gate.mul(loader.ctx_mut().main(), ordinary, difference);
            gate.assert_is_const(loader.ctx_mut().main(), &selected, &C::ScalarExt::ZERO);
        }
    }
    for limb in 0..2 {
        let difference = gate.sub(
            loader.ctx_mut().main(),
            *candidate[state_relation::public_instance::HARDWARE_PROFILE_LO + limb].assigned(),
            wrapper[public_instance::HARDWARE_PROFILE_LO + limb],
        );
        let selected = gate.mul(loader.ctx_mut().main(), terminal, difference);
        gate.assert_is_const(loader.ctx_mut().main(), &selected, &C::ScalarExt::ZERO);
    }

    let credit_id = [
        *candidate[state_relation::public_instance::PEER_CREDIT_LO].assigned(),
        *candidate[state_relation::public_instance::PEER_CREDIT_LO + 1].assigned(),
    ];
    let recipient_lane_id = [
        *candidate[state_relation::public_instance::PEER_RECIPIENT_LANE_LO].assigned(),
        *candidate[state_relation::public_instance::PEER_RECIPIENT_LANE_LO + 1].assigned(),
    ];
    let ecc_chip = loader.ecc_chip();
    let range = ecc_chip.range();
    let mut ctx = loader.ctx_mut();
    let send_output_message = [
        constant_bytes(TERMINAL_SEND_OUTPUT_BINDING_DOMAIN_V1),
        constant_bytes(&1_u16.to_le_bytes()),
        assigned_limbs_to_bytes_v1(ctx.main(), range, credit_id),
        assigned_limbs_to_bytes_v1(ctx.main(), range, recipient_lane_id),
        assigned_limbs_to_bytes_v1(
            ctx.main(),
            range,
            [
                wrapper[public_instance::REQUEST_LO],
                wrapper[public_instance::REQUEST_LO + 1],
            ],
        ),
        assigned_limbs_to_bytes_v1(
            ctx.main(),
            range,
            [
                wrapper[public_instance::ACCEPTANCE_TICKET_LO],
                wrapper[public_instance::ACCEPTANCE_TICKET_LO + 1],
            ],
        ),
        assigned_limbs_to_bytes_v1(
            ctx.main(),
            range,
            [
                wrapper[public_instance::CIPHERTEXT_LO],
                wrapper[public_instance::CIPHERTEXT_LO + 1],
            ],
        ),
        assigned_value_to_bytes_v1(ctx.main(), range, wrapper[public_instance::AMOUNT], 128),
    ]
    .concat();
    let send_output_binding = hash(ctx.main(), sha_jobs, send_output_message)?;
    for (actual, expected) in digest_limbs_assigned(ctx.main(), &send_output_binding)
        .into_iter()
        .zip(&wrapper[public_instance::OUTPUT_BINDING_LO..][..2])
    {
        let difference = gate.sub(ctx.main(), actual, *expected);
        let selected = gate.mul(ctx.main(), difference, terminal_send);
        gate.assert_is_const(ctx.main(), &selected, &C::ScalarExt::ZERO);
    }
    drop(ctx);

    let protocol_offset = match parity {
        OfflineCashPastaParityV1::Eq => state_relation::public_instance::EQ_PROTOCOL_LO,
        OfflineCashPastaParityV1::Ep => state_relation::public_instance::EP_PROTOCOL_LO,
    };
    let expected_protocol = crate::zk::offline_cash_v1_poseidon::digest_limbs::<C::ScalarExt>(
        candidate_protocol_digest,
    );
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
    for value in &candidate[..state_relation::PUBLIC_INSTANCE_COUNT] {
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
        .zip(&wrapper[public_instance::CANDIDATE_LO..][..2])
    {
        let difference = gate.sub(ctx.main(), actual, *expected);
        let selected = gate.mul(ctx.main(), terminal, difference);
        gate.assert_is_const(ctx.main(), &selected, &C::ScalarExt::ZERO);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1, canonical_predecessor_conflict_nullifier_v1,
        canonical_terminal_send_output_binding_v1, validate_enabled_hardware_profiles_v1,
    };
    #[cfg(feature = "zk-halo2-ipa")]
    use super::{state_relation, validate_candidate_guard_protocol_binding_v1};

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn candidate_guard_protocol_binding_rejects_eq_substitution() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let eq = crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x71));
        let ep = crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x72));
        let mut column = vec![Fp::from(0); state_relation::PUBLIC_INSTANCE_COUNT];
        column[state_relation::public_instance::GUARD_EQ_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EQ_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::offline_cash_v1_poseidon::digest_limbs::<Fp>(eq));
        column[state_relation::public_instance::GUARD_EP_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EP_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::offline_cash_v1_poseidon::digest_limbs::<Fp>(ep));
        validate_candidate_guard_protocol_binding_v1(&[column.clone()], eq, ep)
            .expect("exact GuardBundle protocols");
        column[state_relation::public_instance::GUARD_EQ_PROTOCOL_LO] = Fp::from(0);
        assert!(validate_candidate_guard_protocol_binding_v1(&[column], eq, ep).is_err());
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn candidate_guard_protocol_binding_rejects_ep_substitution() {
        use halo2_proofs::halo2curves::pasta::{Fp, Fq};

        let eq = crate::zk::offline_cash_v1_poseidon::encode(Fp::from(0x71));
        let ep = crate::zk::offline_cash_v1_poseidon::encode(Fq::from(0x72));
        let mut column = vec![Fq::from(0); state_relation::PUBLIC_INSTANCE_COUNT];
        column[state_relation::public_instance::GUARD_EQ_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EQ_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::offline_cash_v1_poseidon::digest_limbs::<Fq>(eq));
        column[state_relation::public_instance::GUARD_EP_PROTOCOL_LO
            ..state_relation::public_instance::GUARD_EP_PROTOCOL_LO + 2]
            .copy_from_slice(&crate::zk::offline_cash_v1_poseidon::digest_limbs::<Fq>(ep));
        validate_candidate_guard_protocol_binding_v1(&[column.clone()], eq, ep)
            .expect("exact GuardBundle protocols");
        column[state_relation::public_instance::GUARD_EP_PROTOCOL_LO] = Fq::from(0);
        assert!(validate_candidate_guard_protocol_binding_v1(&[column], eq, ep).is_err());
    }

    #[test]
    fn predecessor_conflict_nullifier_is_successor_independent() {
        let prepared = [0x51; 32];
        let terminal = canonical_predecessor_conflict_nullifier_v1(prepared);
        let cancellation = canonical_predecessor_conflict_nullifier_v1(prepared);
        assert_ne!(terminal, [0; 32]);
        assert_eq!(terminal, cancellation);
        assert_ne!(
            terminal,
            canonical_predecessor_conflict_nullifier_v1([0x52; 32])
        );
    }

    #[test]
    fn terminal_send_output_binding_commits_credit_and_recipient_lane() {
        let baseline = canonical_terminal_send_output_binding_v1(
            [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], 6,
        );
        assert_ne!(
            baseline,
            canonical_terminal_send_output_binding_v1(
                [7; 32], [2; 32], [3; 32], [4; 32], [5; 32], 6,
            )
        );
        assert_ne!(
            baseline,
            canonical_terminal_send_output_binding_v1(
                [1; 32], [7; 32], [3; 32], [4; 32], [5; 32], 6,
            )
        );
    }

    #[test]
    fn enabled_profiles_require_sorted_nonzero_prefix() {
        let mut profiles = [[0_u8; 32]; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1];
        profiles[0][31] = 1;
        profiles[1][31] = 2;
        validate_enabled_hardware_profiles_v1(&profiles).expect("canonical profile table");

        profiles.swap(0, 1);
        assert!(validate_enabled_hardware_profiles_v1(&profiles).is_err());
    }

    #[test]
    fn enabled_profiles_reject_holes_and_duplicates() {
        let mut hole = [[0_u8; 32]; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1];
        hole[0][31] = 1;
        hole[2][31] = 2;
        assert!(validate_enabled_hardware_profiles_v1(&hole).is_err());

        let mut duplicate = [[0_u8; 32]; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1];
        duplicate[0][31] = 1;
        duplicate[1][31] = 1;
        assert!(validate_enabled_hardware_profiles_v1(&duplicate).is_err());
    }
}
