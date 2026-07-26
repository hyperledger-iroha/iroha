//! Canonical first-release privacy governance and proof-admission handlers.
//!
//! Governance checks and all deterministic validation precede storage writes.
//! Proof admission is added only through the exhaustive native verifier
//! boundary; there is deliberately no generic or opaque fallback verifier.

use std::collections::BTreeSet;

use iroha_data_model::{
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        privacy::{
            BootstrapPrivacyPgcAccountsV1, BootstrapPrivacyZkAmsRegistryV1, PublishPrivacyRootV1,
            RegisterPrivacyProtocolActivationV1, RegisterPrivacyZkAcePolicyV1,
            RevokePrivacyZkAcePolicyV1, RotatePrivacyZkAcePolicyV1,
            SchedulePrivacyConsensusPolicyTighteningV1, SchedulePrivacyProtocolLimitsTighteningV1,
            SubmitPrivacyProofV1, TransitionPrivacyProtocolLifecycleV1,
        },
    },
    permission::Permission,
    prelude::{Account, AccountId, Quantity, Register, Transfer},
    privacy::{
        PRIVACY_ZK_ACE_MAX_POLICIES_V1, PrivacyConsensusPolicyTighteningV1, PrivacyNamespaceV1,
        PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1, PrivacyProtocolLimitsTighteningV1,
        PrivacyRootManagementV1, PrivacyRootRoleV1, PrivacyStatementV1,
        PrivacyZkAcePolicyLifecycleV1, PrivacyZkAmsActionV1,
        TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1, validate_zk_ace_policy_revocation_v1,
        validate_zk_ace_policy_rotation_v1, zk_ams_issuer_policy_record_digest_v1,
        zk_ams_registry_record_digest_v1,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use mv::storage::StorageReadOnly;

use super::Execute;
use crate::{
    privacy::{validate_privacy_lifecycle_transition_v1, validate_privacy_registration_v1},
    privacy_engines::{
        anonymous_pgc::{
            AnonymousPgcParametersV1, TwistedElGamalCiphertextV1, TwistedElGamalPublicKeyV1,
            bootstrap::{
                AnonymousPgcBootstrapStatementV1, MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
                verify_bootstrap_encoded,
            },
        },
        p256::{CompressedPointV1, TranscriptBindingV1},
        zk_ams::zk_ams_registry_transition_root_v1,
    },
    privacy_profiles::validate_compiled_privacy_activation_v1,
    privacy_state::{
        PrivacyActivationKeyV1, PrivacyCommitmentKeyV1, PrivacyNullifierKeyV1,
        PrivacyPgcAccountKeyV1, PrivacyPgcAccountProvenanceV1, PrivacyPgcAccountStateV1,
        PrivacyPgcPoolInvariantKeyV1, PrivacyPgcPoolInvariantV1, PrivacyRootHeadKeyV1,
        PrivacyRootHeadRecordV1, PrivacyRootKeyV1, PrivacyRootProvenanceV1,
        PrivacyRootRetentionAnchorV1, PrivacyStateItemRecordV1,
        compute_privacy_pgc_account_state_root_v1, load_privacy_pgc_pool_snapshot_v1,
        load_privacy_zk_ace_policy_v1, load_privacy_zk_ams_registry_snapshot_v1,
        plan_privacy_root_history_update_v1, privacy_zk_ace_policy_count_v1,
        validate_non_pgc_privacy_root_retention_v1,
    },
    privacy_verifier::{
        PrivacyAnonymousPgcStateFailureCodeV1, PrivacyPgcVerificationStateV1,
        PrivacyVerificationContextFailureCodeV1, PrivacyVerificationContextV1,
        PrivacyVerificationErrorV1, VerifiedPrivacyLedgerEffectsV1, verify_privacy_envelope_v1,
    },
    state::{StateTransaction, WorldReadOnly},
};

fn invalid_privacy_parameter(message: impl Into<String>) -> Error {
    Error::InvalidParameter(InvalidParameterError::SmartContract(message.into()))
}

fn has_exact_permission(
    state_transaction: &StateTransaction<'_, '_>,
    authority: &AccountId,
    required: &Permission,
) -> bool {
    if state_transaction
        .world
        .account_permissions
        .get(authority)
        .is_some_and(|permissions| permissions.contains(required))
    {
        return true;
    }

    state_transaction
        .world
        .account_roles
        .iter()
        .filter_map(|(role_key, ())| {
            if &role_key.account == authority {
                state_transaction.world.roles.get(&role_key.id)
            } else {
                None
            }
        })
        .any(|role| role.permissions().any(|permission| permission == required))
}

fn ensure_privacy_governance(
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let required: Permission = CanEnactGovernance.into();
    if !has_exact_permission(state_transaction, authority, &required) {
        return Err(Error::InvariantViolation(
            "not permitted: CanEnactGovernance".into(),
        ));
    }
    Ok(())
}

fn privacy_verification_error(error: PrivacyVerificationErrorV1) -> Error {
    let message = format!("privacy proof admission rejected: {error}");
    let invariant = match &error {
        PrivacyVerificationErrorV1::CompiledActivation(_)
        | PrivacyVerificationErrorV1::CanonicalEncoding(_) => true,
        PrivacyVerificationErrorV1::Context(detail) => {
            detail.code == PrivacyVerificationContextFailureCodeV1::ZeroGenesisHash
        }
        PrivacyVerificationErrorV1::AnonymousPgcState(detail) => !matches!(
            detail.code,
            PrivacyAnonymousPgcStateFailureCodeV1::StaleHead
                | PrivacyAnonymousPgcStateFailureCodeV1::AccountTableMismatch
                | PrivacyAnonymousPgcStateFailureCodeV1::NextRootMismatch
        ),
        PrivacyVerificationErrorV1::Envelope(_)
        | PrivacyVerificationErrorV1::EngineUnavailable(_)
        | PrivacyVerificationErrorV1::NativeVeRange(_)
        | PrivacyVerificationErrorV1::NativeVega(_)
        | PrivacyVerificationErrorV1::NativeJindo(_)
        | PrivacyVerificationErrorV1::NativeZkAce(_)
        | PrivacyVerificationErrorV1::NativeZkAms(_)
        | PrivacyVerificationErrorV1::NativeAnonymousPgc(_) => false,
    };
    if invariant {
        Error::InvariantViolation(message.into())
    } else {
        invalid_privacy_parameter(message)
    }
}

impl Execute for RegisterPrivacyProtocolActivationV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        let current_height = state_transaction._curr_block.height().get();
        let key = PrivacyActivationKeyV1::new(self.activation.protocol_id);
        validate_privacy_registration_v1(
            &state_transaction
                .world
                .privacy_consensus_policy
                .get()
                .current_limits,
            state_transaction.world.privacy_activations.get(&key),
            &self.activation,
            current_height,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("privacy activation registration rejected: {error}"))
        })?;

        state_transaction
            .world
            .privacy_activations
            .insert(key, self.activation);
        Ok(())
    }
}

impl Execute for SchedulePrivacyConsensusPolicyTighteningV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        let incoming_height = state_transaction.block_height();
        let current = *state_transaction.world.privacy_consensus_policy.get();
        current.validate().map_err(|error| {
            Error::InvariantViolation(
                format!("persisted privacy consensus policy is invalid: {error}").into(),
            )
        })?;
        if current.pending_tightening.is_some() {
            return Err(invalid_privacy_parameter(
                "privacy consensus policy already has a pending tightening",
            ));
        }
        let pending = PrivacyConsensusPolicyTighteningV1 {
            scheduled_at_height: incoming_height,
            effective_at_height: self.effective_at_height,
            next_limits: self.next_limits,
        };
        pending
            .validate_against(&current.current_limits)
            .map_err(|error| {
                invalid_privacy_parameter(format!(
                    "privacy consensus policy tightening rejected: {error}"
                ))
            })?;
        validate_non_pgc_privacy_root_retention_v1(
            &state_transaction.world.privacy_roots,
            self.next_limits.retained_root_count,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!(
                "privacy consensus policy tightening rejected: {error}"
            ))
        })?;

        *state_transaction.world.privacy_consensus_policy.get_mut() =
            iroha_data_model::privacy::PrivacyConsensusPolicyV1 {
                current_limits: current.current_limits,
                pending_tightening: Some(pending),
            };
        Ok(())
    }
}

impl Execute for SchedulePrivacyProtocolLimitsTighteningV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        let incoming_height = state_transaction.block_height();
        let key = PrivacyActivationKeyV1::new(self.protocol_id);
        let current = state_transaction
            .world
            .privacy_activations
            .get(&key)
            .copied()
            .ok_or_else(|| {
                invalid_privacy_parameter(format!(
                    "privacy protocol {:?} is not registered",
                    self.protocol_id
                ))
            })?;
        current.validate().map_err(|error| {
            Error::InvariantViolation(
                format!("persisted privacy activation is invalid: {error}").into(),
            )
        })?;
        if current.pending_protocol_limits_tightening.is_some() {
            return Err(invalid_privacy_parameter(format!(
                "privacy protocol {:?} already has a pending limit tightening",
                self.protocol_id
            )));
        }
        let pending = PrivacyProtocolLimitsTighteningV1 {
            scheduled_at_height: incoming_height,
            effective_at_height: self.effective_at_height,
            next_limits: self.next_limits,
        };
        pending
            .validate_against(&current.protocol_limits)
            .map_err(|error| {
                invalid_privacy_parameter(format!(
                    "privacy protocol-limit tightening rejected: {error}"
                ))
            })?;
        let mut executable_successor = current;
        executable_successor.protocol_limits = self.next_limits;
        executable_successor.pending_protocol_limits_tightening = None;
        validate_compiled_privacy_activation_v1(&executable_successor).map_err(|error| {
            invalid_privacy_parameter(format!(
                "privacy protocol-limit tightening is not executable: {error}"
            ))
        })?;

        let mut next = current;
        next.pending_protocol_limits_tightening = Some(pending);
        state_transaction
            .world
            .privacy_activations
            .insert(key, next);
        Ok(())
    }
}

impl Execute for TransitionPrivacyProtocolLifecycleV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        let current_height = state_transaction._curr_block.height().get();
        let key = PrivacyActivationKeyV1::new(self.protocol_id);
        let current = state_transaction
            .world
            .privacy_activations
            .get(&key)
            .cloned()
            .ok_or_else(|| {
                invalid_privacy_parameter(format!(
                    "privacy protocol {:?} is not registered",
                    self.protocol_id
                ))
            })?;
        validate_privacy_lifecycle_transition_v1(&current, self.next_lifecycle, current_height)
            .map_err(|error| {
                invalid_privacy_parameter(format!("privacy lifecycle transition rejected: {error}"))
            })?;

        let mut next = current;
        next.lifecycle = self.next_lifecycle;
        state_transaction
            .world
            .privacy_activations
            .insert(key, next);
        Ok(())
    }
}

impl Execute for PublishPrivacyRootV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        self.publication.validate().map_err(|error| {
            invalid_privacy_parameter(format!("privacy root publication rejected: {error}"))
        })?;
        if self.publication.role == PrivacyRootRoleV1::PgcAccountState {
            return Err(invalid_privacy_parameter(
                "PGC account-state roots require a complete typed account bootstrap",
            ));
        }
        if self.publication.role == PrivacyRootRoleV1::AccountRegistry {
            return Err(invalid_privacy_parameter(
                "ZK-AMS AccountRegistry roots require the typed registry bootstrap and verified proof successors",
            ));
        }
        let current_height = state_transaction._curr_block.height().get();
        let activation_key = PrivacyActivationKeyV1::new(self.publication.namespace.protocol_id());
        let activation = state_transaction
            .world
            .privacy_activations
            .get(&activation_key)
            .copied()
            .ok_or_else(|| {
                invalid_privacy_parameter(format!(
                    "privacy protocol {:?} is not registered",
                    self.publication.namespace.protocol_id()
                ))
            })?;
        activation.validate().map_err(|error| {
            invalid_privacy_parameter(format!("registered privacy activation is invalid: {error}"))
        })?;
        if matches!(activation.lifecycle, PrivacyProtocolLifecycleV1::Retired(_)) {
            return Err(invalid_privacy_parameter(
                "cannot publish a root for a retired privacy protocol",
            ));
        }

        let head_key = PrivacyRootHeadKeyV1::new(self.publication.namespace, self.publication.role)
            .map_err(invalid_privacy_parameter)?;
        let current_head = state_transaction
            .world
            .privacy_root_heads
            .get(&head_key)
            .copied();
        match (self.publication.role.management(), current_head) {
            (PrivacyRootManagementV1::ProofManaged, Some(_)) => {
                return Err(invalid_privacy_parameter(
                    "governance may initialize but cannot advance a proof-managed privacy root",
                ));
            }
            (PrivacyRootManagementV1::GovernanceManaged, Some(head))
                if self.publication.epoch <= head.epoch() =>
            {
                return Err(invalid_privacy_parameter(format!(
                    "governance-managed privacy root epoch must advance current epoch {}",
                    head.epoch()
                )));
            }
            _ => {}
        }

        if let Some(head) = current_head {
            let retained_head = PrivacyRootKeyV1::new(
                head_key.namespace(),
                head_key.role(),
                head.epoch(),
                head.root(),
            )
            .map_err(invalid_privacy_parameter)?;
            if state_transaction.world.privacy_roots.get(&retained_head) != Some(&head.provenance())
            {
                return Err(Error::InvariantViolation(
                    "privacy root head is inconsistent with retained history".into(),
                ));
            }
        } else if state_transaction
            .world
            .privacy_roots
            .range(PrivacyRootKeyV1::history_range(
                head_key.namespace(),
                head_key.role(),
            ))
            .next()
            .is_some()
        {
            return Err(Error::InvariantViolation(
                "privacy root history exists without a current head".into(),
            ));
        }

        let root_key = PrivacyRootKeyV1::new(
            self.publication.namespace,
            self.publication.role,
            self.publication.epoch,
            self.publication.root,
        )
        .map_err(invalid_privacy_parameter)?;
        let publication_digest = self.publication.digest().map_err(|error| {
            invalid_privacy_parameter(format!("privacy root publication encoding failed: {error}"))
        })?;
        let provenance = PrivacyRootProvenanceV1::governance(publication_digest, current_height)
            .map_err(invalid_privacy_parameter)?;
        let removals = plan_privacy_root_history_update_v1(
            &state_transaction.world.privacy_roots,
            &[root_key],
            state_transaction
                .world
                .privacy_consensus_policy
                .get()
                .admission_retained_root_count(),
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("privacy root publication rejected: {error}"))
        })?;
        if !removals.is_empty() {
            return Err(invalid_privacy_parameter(
                "non-PGC privacy root retention rollover is unavailable without a typed anchor-chain validator",
            ));
        }
        let retention_anchor = removals
            .last()
            .map(|key| PrivacyRootRetentionAnchorV1::new(key.epoch(), key.root()))
            .transpose()
            .map_err(invalid_privacy_parameter)?
            .or_else(|| current_head.and_then(PrivacyRootHeadRecordV1::retention_anchor));
        let next_head = PrivacyRootHeadRecordV1::new(
            self.publication.epoch,
            self.publication.root,
            provenance,
            retention_anchor,
        )
        .map_err(invalid_privacy_parameter)?;

        for key in removals {
            state_transaction.world.privacy_roots.remove(key);
        }
        state_transaction
            .world
            .privacy_roots
            .insert(root_key, provenance);
        state_transaction
            .world
            .privacy_root_heads
            .insert(head_key, next_head);
        Ok(())
    }
}

impl Execute for BootstrapPrivacyZkAmsRegistryV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;
        let encoded_action_bytes = norito::to_bytes(&self)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
            .ok_or_else(|| {
                Error::InvariantViolation(
                    "ZK-AMS registry bootstrap canonical encoding failed".into(),
                )
            })?;
        let expected_action_index = state_transaction.next_privacy_action_index();
        state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
        self.bootstrap.validate().map_err(|error| {
            invalid_privacy_parameter(format!("ZK-AMS registry bootstrap rejected: {error}"))
        })?;
        CompressedPointV1::from_slice(self.bootstrap.issuer_public_key.as_bytes()).map_err(
            |error| {
                invalid_privacy_parameter(format!(
                    "ZK-AMS registry bootstrap issuer key rejected: {error}"
                ))
            },
        )?;

        let current_height = state_transaction._curr_block.height().get();
        let namespace = self.bootstrap.namespace();
        let activation_key = PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::IrohaZkAmsV1);
        let activation = state_transaction
            .world
            .privacy_activations
            .get(&activation_key)
            .copied()
            .ok_or_else(|| {
                invalid_privacy_parameter("ZK-AMS privacy protocol is not registered")
            })?;
        validate_compiled_privacy_activation_v1(&activation).map_err(|error| {
            Error::InvariantViolation(
                format!("registered ZK-AMS activation is not executable: {error}").into(),
            )
        })?;
        activation.validate().map_err(|error| {
            invalid_privacy_parameter(format!("registered ZK-AMS activation is invalid: {error}"))
        })?;
        let PrivacyProtocolLifecycleV1::Active(active) = activation.lifecycle else {
            return Err(invalid_privacy_parameter(
                "cannot bootstrap a registry before ZK-AMS is active",
            ));
        };
        if current_height < active.state_since_height {
            return Err(invalid_privacy_parameter(format!(
                "ZK-AMS activation is not effective until block {}",
                active.state_since_height
            )));
        }

        let head_key = PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::AccountRegistry)
            .map_err(invalid_privacy_parameter)?;
        if state_transaction
            .world
            .privacy_root_heads
            .get(&head_key)
            .is_some()
        {
            return Err(invalid_privacy_parameter(
                "ZK-AMS AccountRegistry is already initialized",
            ));
        }
        if state_transaction
            .world
            .privacy_roots
            .range(PrivacyRootKeyV1::history_range(
                namespace,
                PrivacyRootRoleV1::AccountRegistry,
            ))
            .next()
            .is_some()
        {
            return Err(Error::InvariantViolation(
                "ZK-AMS AccountRegistry history exists without a current head".into(),
            ));
        }
        if state_transaction
            .world
            .privacy_commitments
            .range(PrivacyCommitmentKeyV1::zk_ams_issuer_policy_record_range(
                namespace,
            ))
            .next()
            .is_some()
            || state_transaction
                .world
                .privacy_commitments
                .range(PrivacyCommitmentKeyV1::zk_ams_phc_range(namespace))
                .next()
                .is_some()
            || state_transaction
                .world
                .privacy_commitments
                .range(PrivacyCommitmentKeyV1::zk_ams_seed_key_range(namespace))
                .next()
                .is_some()
            || state_transaction
                .world
                .privacy_nullifiers
                .range(
                    crate::privacy_state::PrivacyNullifierKeyV1::zk_ams_key_image_range(namespace),
                )
                .next()
                .is_some()
        {
            return Err(Error::InvariantViolation(
                "ZK-AMS state items exist without a current registry head".into(),
            ));
        }

        let bootstrap_digest = self.bootstrap.digest();
        let issuer_record_key = PrivacyCommitmentKeyV1::zk_ams_issuer_policy_record(
            namespace,
            self.bootstrap.issuer_policy_record_digest(),
        )
        .map_err(invalid_privacy_parameter)?;
        let issuer_record =
            PrivacyStateItemRecordV1::zk_ams_governance(bootstrap_digest, current_height)
                .map_err(invalid_privacy_parameter)?;
        let root_key = PrivacyRootKeyV1::new(
            namespace,
            PrivacyRootRoleV1::AccountRegistry,
            self.bootstrap.initial_registry_epoch,
            self.bootstrap.initial_registry_root,
        )
        .map_err(invalid_privacy_parameter)?;
        let root_provenance =
            PrivacyRootProvenanceV1::zk_ams_registry_bootstrap(bootstrap_digest, current_height)
                .map_err(invalid_privacy_parameter)?;
        let root_head = PrivacyRootHeadRecordV1::new(
            self.bootstrap.initial_registry_epoch,
            self.bootstrap.initial_registry_root,
            root_provenance,
            None,
        )
        .map_err(invalid_privacy_parameter)?;
        let removals = plan_privacy_root_history_update_v1(
            &state_transaction.world.privacy_roots,
            &[root_key],
            state_transaction
                .world
                .privacy_consensus_policy
                .get()
                .current_limits
                .retained_root_count,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("ZK-AMS registry bootstrap root rejected: {error}"))
        })?;
        if !removals.is_empty() {
            return Err(Error::InvariantViolation(
                "new ZK-AMS registry history unexpectedly requires pruning".into(),
            ));
        }

        state_transaction.reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
        state_transaction
            .world
            .privacy_commitments
            .insert(issuer_record_key, issuer_record);
        state_transaction
            .world
            .privacy_roots
            .insert(root_key, root_provenance);
        state_transaction
            .world
            .privacy_root_heads
            .insert(head_key, root_head);
        Ok(())
    }
}

impl Execute for BootstrapPrivacyPgcAccountsV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        let encoded_action_bytes = norito::to_bytes(&self)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
            .ok_or_else(|| {
                Error::InvariantViolation("privacy PGC bootstrap canonical encoding failed".into())
            })?;
        let expected_action_index = state_transaction.next_privacy_action_index();
        state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
        self.bootstrap.validate().map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC account bootstrap rejected: {error}"))
        })?;
        self.proof.validate().map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC bootstrap proof rejected: {error}"))
        })?;
        let native_proof_cap = u32::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1).map_err(|_| {
            Error::InvariantViolation(
                "native PGC bootstrap proof cap cannot be represented by consensus".into(),
            )
        })?;
        if native_proof_cap != TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1 {
            return Err(Error::InvariantViolation(
                "native and data-model PGC bootstrap proof caps differ".into(),
            ));
        }

        let current_height = state_transaction._curr_block.height().get();
        let activation_key = PrivacyActivationKeyV1::new(self.bootstrap.namespace.protocol_id());
        let activation = state_transaction
            .world
            .privacy_activations
            .get(&activation_key)
            .copied()
            .ok_or_else(|| {
                invalid_privacy_parameter("Anonymous PGC privacy protocol is not registered")
            })?;
        validate_compiled_privacy_activation_v1(&activation).map_err(|error| {
            Error::InvariantViolation(
                format!("registered Anonymous PGC activation is not executable: {error}").into(),
            )
        })?;
        activation.validate().map_err(|error| {
            invalid_privacy_parameter(format!(
                "registered Anonymous PGC activation is invalid: {error}"
            ))
        })?;
        let PrivacyProtocolLifecycleV1::Active(active) = activation.lifecycle else {
            return Err(invalid_privacy_parameter(
                "cannot bootstrap accounts before the Anonymous PGC protocol is active",
            ));
        };
        if current_height < active.state_since_height {
            return Err(invalid_privacy_parameter(format!(
                "Anonymous PGC activation is not effective until block {}",
                active.state_since_height
            )));
        }

        let head_key =
            PrivacyRootHeadKeyV1::new(self.bootstrap.namespace, PrivacyRootRoleV1::PgcAccountState)
                .map_err(invalid_privacy_parameter)?;
        let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(self.bootstrap.namespace)
            .map_err(invalid_privacy_parameter)?;
        if state_transaction
            .world
            .privacy_root_heads
            .get(&head_key)
            .is_some()
        {
            return Err(invalid_privacy_parameter(
                "Anonymous PGC account state is already initialized",
            ));
        }
        if state_transaction
            .world
            .privacy_pgc_pool_invariants
            .get(&invariant_key)
            .is_some()
        {
            return Err(Error::InvariantViolation(
                "Anonymous PGC pool invariant exists without a current head".into(),
            ));
        }
        if state_transaction
            .world
            .privacy_roots
            .range(PrivacyRootKeyV1::history_range(
                self.bootstrap.namespace,
                PrivacyRootRoleV1::PgcAccountState,
            ))
            .next()
            .is_some()
        {
            return Err(Error::InvariantViolation(
                "Anonymous PGC root history exists without a current head".into(),
            ));
        }
        if state_transaction
            .world
            .privacy_pgc_accounts
            .range(PrivacyPgcAccountKeyV1::pool_range(self.bootstrap.namespace))
            .next()
            .is_some()
        {
            return Err(Error::InvariantViolation(
                "Anonymous PGC accounts exist without a current head".into(),
            ));
        }

        let computed_root = compute_privacy_pgc_account_state_root_v1(
            self.bootstrap.namespace,
            self.bootstrap.initial_epoch,
            self.bootstrap.total_supply,
            &self.bootstrap.accounts,
        )
        .map_err(invalid_privacy_parameter)?;
        if computed_root != self.bootstrap.initial_root {
            return Err(invalid_privacy_parameter(
                "privacy PGC bootstrap root does not match the canonical account table",
            ));
        }

        let bootstrap_digest = self.bootstrap.digest().map_err(|error| {
            Error::InvariantViolation(
                format!("privacy PGC bootstrap canonical encoding failed: {error}").into(),
            )
        })?;
        let namespace_encoding = norito::to_bytes(&self.bootstrap.namespace).map_err(|error| {
            Error::InvariantViolation(
                format!("privacy PGC namespace canonical encoding failed: {error}").into(),
            )
        })?;
        let native_public_keys = self
            .bootstrap
            .accounts
            .iter()
            .map(|account| {
                TwistedElGamalPublicKeyV1::from_sec1_bytes(account.public_key.as_bytes())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                invalid_privacy_parameter(format!(
                    "privacy PGC bootstrap public key rejected: {error}"
                ))
            })?;
        let native_balances = self
            .bootstrap
            .accounts
            .iter()
            .map(|account| {
                TwistedElGamalCiphertextV1::from_sec1_bytes(
                    account.encrypted_balance.left.as_bytes(),
                    account.encrypted_balance.right.as_bytes(),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                invalid_privacy_parameter(format!(
                    "privacy PGC bootstrap ciphertext rejected: {error}"
                ))
            })?;
        let genesis_hash = state_transaction
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                Error::InvariantViolation(
                    "privacy PGC bootstrap requires a committed genesis block".into(),
                )
            })?;
        let parameters = AnonymousPgcParametersV1::get().map_err(|error| {
            Error::InvariantViolation(
                format!("native Anonymous PGC parameters are unavailable: {error}").into(),
            )
        })?;
        let native_statement = AnonymousPgcBootstrapStatementV1::new(
            &namespace_encoding,
            *self.bootstrap.initial_root.as_bytes(),
            self.bootstrap.initial_epoch,
            self.bootstrap.total_supply,
            &native_public_keys,
            &native_balances,
            TranscriptBindingV1 {
                chain_id: state_transaction.chain_id.as_str().as_bytes(),
                genesis_hash,
                action_index: expected_action_index,
                statement_digest: *bootstrap_digest.as_bytes(),
                parameter_id: *activation.parameter_id.as_bytes(),
                parameter_digest: *activation.parameter_digest.as_bytes(),
                verifier_digest: *activation.verifier_digest.as_bytes(),
                statement_schema_digest: *activation.statement_schema_digest.as_bytes(),
                engine_manifest_digest: *activation.engine_manifest_digest.as_bytes(),
                generator_digest: parameters.generator_digest(),
            },
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC bootstrap statement rejected: {error}"))
        })?;
        let verified = verify_bootstrap_encoded(&native_statement, self.proof.as_bytes()).map_err(
            |error| {
                invalid_privacy_parameter(format!(
                    "privacy PGC bootstrap proof verification failed: {error}"
                ))
            },
        )?;
        if verified.total_supply() != self.bootstrap.total_supply
            || verified.account_count() != self.bootstrap.accounts.len()
            || verified.bootstrap_table_digest() != native_statement.bootstrap_table_digest()
        {
            return Err(Error::InvariantViolation(
                "native PGC bootstrap verifier returned inconsistent effects".into(),
            ));
        }
        let bootstrap_proof_digest = self.proof.digest().map_err(|error| {
            Error::InvariantViolation(
                format!("verified PGC bootstrap proof digest failed: {error}").into(),
            )
        })?;
        let pool_invariant = PrivacyPgcPoolInvariantV1::new(
            self.bootstrap.total_supply,
            self.bootstrap.initial_root,
            bootstrap_digest,
            bootstrap_proof_digest,
        )
        .map_err(invalid_privacy_parameter)?;
        let account_provenance = PrivacyPgcAccountProvenanceV1::bootstrap(
            bootstrap_digest,
            bootstrap_proof_digest,
            current_height,
        )
        .map_err(invalid_privacy_parameter)?;
        let mut account_updates = Vec::with_capacity(self.bootstrap.accounts.len());
        for account in &self.bootstrap.accounts {
            let key = PrivacyPgcAccountKeyV1::new(self.bootstrap.namespace, account.public_key)
                .map_err(invalid_privacy_parameter)?;
            let state = PrivacyPgcAccountStateV1::new(
                account.encrypted_balance,
                self.bootstrap.initial_epoch,
                account_provenance,
            )
            .map_err(invalid_privacy_parameter)?;
            account_updates.push((key, state));
        }

        let root_provenance = PrivacyRootProvenanceV1::verified_bootstrap(
            bootstrap_digest,
            bootstrap_proof_digest,
            current_height,
        )
        .map_err(invalid_privacy_parameter)?;
        let root_key = PrivacyRootKeyV1::new(
            self.bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
            self.bootstrap.initial_epoch,
            self.bootstrap.initial_root,
        )
        .map_err(invalid_privacy_parameter)?;
        let root_head = PrivacyRootHeadRecordV1::new(
            self.bootstrap.initial_epoch,
            self.bootstrap.initial_root,
            root_provenance,
            None,
        )
        .map_err(invalid_privacy_parameter)?;
        let removals = plan_privacy_root_history_update_v1(
            &state_transaction.world.privacy_roots,
            &[root_key],
            state_transaction
                .world
                .privacy_consensus_policy
                .get()
                .current_limits
                .retained_root_count,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC bootstrap root rejected: {error}"))
        })?;
        if !removals.is_empty() {
            return Err(Error::InvariantViolation(
                "new Anonymous PGC root history unexpectedly requires pruning".into(),
            ));
        }
        state_transaction.reserve_privacy_action(expected_action_index, encoded_action_bytes)?;

        state_transaction
            .world
            .privacy_pgc_pool_invariants
            .insert(invariant_key, pool_invariant);
        for (key, state) in account_updates {
            state_transaction
                .world
                .privacy_pgc_accounts
                .insert(key, state);
        }
        state_transaction
            .world
            .privacy_roots
            .insert(root_key, root_provenance);
        state_transaction
            .world
            .privacy_root_heads
            .insert(head_key, root_head);
        Ok(())
    }
}

fn validate_zk_ace_policy_references(
    policy: &iroha_data_model::privacy::PrivacyZkAcePolicyRecordV1,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    state_transaction
        .world
        .asset_definition(&policy.asset_definition_id)
        .map_err(Error::from)?;
    for account_id in &policy.source_allowlist {
        if state_transaction.world.accounts.get(account_id).is_none() {
            return Err(invalid_privacy_parameter(format!(
                "ZK-ACE source account `{account_id}` does not exist"
            )));
        }
    }
    Ok(())
}

fn require_registered_zk_ace_protocol(
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let activation_key = PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
    let activation = state_transaction
        .world
        .privacy_activations
        .get(&activation_key)
        .ok_or_else(|| invalid_privacy_parameter("ZK-ACE privacy protocol is not registered"))?;
    activation.validate().map_err(|error| {
        Error::InvariantViolation(
            format!("registered ZK-ACE activation is invalid: {error}").into(),
        )
    })?;
    validate_compiled_privacy_activation_v1(activation).map_err(|error| {
        Error::InvariantViolation(
            format!("registered ZK-ACE activation is not executable: {error}").into(),
        )
    })
}

impl Execute for RegisterPrivacyZkAcePolicyV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;
        let encoded_action_bytes = norito::to_bytes(&self)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
            .ok_or_else(|| {
                Error::InvariantViolation(
                    "ZK-ACE policy registration canonical encoding failed".into(),
                )
            })?;
        let expected_action_index = state_transaction.next_privacy_action_index();
        state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
        require_registered_zk_ace_protocol(state_transaction)?;
        self.policy.validate_initial().map_err(|error| {
            invalid_privacy_parameter(format!("ZK-ACE policy registration rejected: {error}"))
        })?;
        validate_zk_ace_policy_references(&self.policy, state_transaction)?;
        let policy_count = privacy_zk_ace_policy_count_v1(
            &state_transaction.world.privacy_commitments,
        )
        .map_err(|error| {
            Error::InvariantViolation(
                format!("persisted ZK-ACE policy registry is invalid: {error}").into(),
            )
        })?;
        if policy_count >= PRIVACY_ZK_ACE_MAX_POLICIES_V1 {
            return Err(invalid_privacy_parameter(format!(
                "ZK-ACE policy registry is full at {} policies",
                PRIVACY_ZK_ACE_MAX_POLICIES_V1
            )));
        }
        let key = PrivacyCommitmentKeyV1::zk_ace_policy(self.policy.policy_id)
            .map_err(invalid_privacy_parameter)?;
        if state_transaction
            .world
            .privacy_commitments
            .get(&key)
            .is_some()
        {
            return Err(invalid_privacy_parameter(
                "ZK-ACE policy id is already registered",
            ));
        }
        let record = PrivacyStateItemRecordV1::zk_ace_policy_governance(
            self.policy,
            state_transaction.block_height(),
        )
        .map_err(invalid_privacy_parameter)?;

        state_transaction.reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
        state_transaction
            .world
            .privacy_commitments
            .insert(key, record);
        Ok(())
    }
}

impl Execute for RotatePrivacyZkAcePolicyV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;
        let encoded_action_bytes = norito::to_bytes(&self)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
            .ok_or_else(|| {
                Error::InvariantViolation("ZK-ACE policy rotation canonical encoding failed".into())
            })?;
        let expected_action_index = state_transaction.next_privacy_action_index();
        state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
        require_registered_zk_ace_protocol(state_transaction)?;
        if self.expected_current_record_digest.is_zero() {
            return Err(invalid_privacy_parameter(
                "ZK-ACE expected current policy digest must be non-zero",
            ));
        }
        let current = load_privacy_zk_ace_policy_v1(
            self.successor.policy_id,
            &state_transaction.world.privacy_commitments,
        )
        .map_err(|error| {
            Error::InvariantViolation(
                format!("trusted ZK-ACE policy state failed validation: {error}").into(),
            )
        })?;
        if current.record_digest != self.expected_current_record_digest {
            return Err(invalid_privacy_parameter(
                "ZK-ACE policy rotation expected a stale or substituted current record",
            ));
        }
        validate_zk_ace_policy_rotation_v1(&current, &self.successor).map_err(|error| {
            invalid_privacy_parameter(format!("ZK-ACE policy rotation rejected: {error}"))
        })?;
        validate_zk_ace_policy_references(&self.successor, state_transaction)?;
        let key = PrivacyCommitmentKeyV1::zk_ace_policy(current.policy_id)
            .map_err(invalid_privacy_parameter)?;
        let record = PrivacyStateItemRecordV1::zk_ace_policy_governance(
            self.successor,
            state_transaction.block_height(),
        )
        .map_err(invalid_privacy_parameter)?;

        state_transaction.reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
        state_transaction
            .world
            .privacy_commitments
            .insert(key, record);
        Ok(())
    }
}

impl Execute for RevokePrivacyZkAcePolicyV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;
        let encoded_action_bytes = norito::to_bytes(&self)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
            .ok_or_else(|| {
                Error::InvariantViolation(
                    "ZK-ACE policy revocation canonical encoding failed".into(),
                )
            })?;
        let expected_action_index = state_transaction.next_privacy_action_index();
        state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
        require_registered_zk_ace_protocol(state_transaction)?;
        if self.expected_current_record_digest.is_zero() {
            return Err(invalid_privacy_parameter(
                "ZK-ACE expected current policy digest must be non-zero",
            ));
        }
        let current = load_privacy_zk_ace_policy_v1(
            self.successor.policy_id,
            &state_transaction.world.privacy_commitments,
        )
        .map_err(|error| {
            Error::InvariantViolation(
                format!("trusted ZK-ACE policy state failed validation: {error}").into(),
            )
        })?;
        if current.record_digest != self.expected_current_record_digest {
            return Err(invalid_privacy_parameter(
                "ZK-ACE policy revocation expected a stale or substituted current record",
            ));
        }
        validate_zk_ace_policy_revocation_v1(&current, &self.successor).map_err(|error| {
            invalid_privacy_parameter(format!("ZK-ACE policy revocation rejected: {error}"))
        })?;
        if self.successor.lifecycle != PrivacyZkAcePolicyLifecycleV1::Revoked {
            return Err(invalid_privacy_parameter(
                "ZK-ACE policy revocation successor must be revoked",
            ));
        }
        let key = PrivacyCommitmentKeyV1::zk_ace_policy(current.policy_id)
            .map_err(invalid_privacy_parameter)?;
        let record = PrivacyStateItemRecordV1::zk_ace_policy_governance(
            self.successor,
            state_transaction.block_height(),
        )
        .map_err(invalid_privacy_parameter)?;

        state_transaction.reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
        state_transaction
            .world
            .privacy_commitments
            .insert(key, record);
        Ok(())
    }
}

impl Execute for SubmitPrivacyProofV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let transaction_intent_digest = self.envelope.statement.context().transaction_intent_digest;
        let signed_submission_hash = crate::privacy::privacy_signed_submission_hash_v1(&self)
            .map_err(|error| {
                Error::InvariantViolation(
                    format!("privacy submission canonical encoding failed: {error}").into(),
                )
            })?;
        state_transaction
            .consume_privacy_transaction_intent_v1(
                transaction_intent_digest,
                signed_submission_hash,
            )
            .map_err(|error| {
                invalid_privacy_parameter(format!(
                    "privacy transaction-intent binding rejected: {error}"
                ))
            })?;
        let encoded_action_bytes = norito::to_bytes(&self.envelope)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok())
            .ok_or_else(|| {
                Error::InvariantViolation("privacy proof envelope canonical encoding failed".into())
            })?;
        let expected_action_index = state_transaction.next_privacy_action_index();
        state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
        let activation_key = PrivacyActivationKeyV1::new(self.envelope.protocol_id);
        let activation = state_transaction
            .world
            .privacy_activations
            .get(&activation_key)
            .copied()
            .ok_or_else(|| {
                invalid_privacy_parameter(format!(
                    "privacy protocol {:?} is not registered",
                    self.envelope.protocol_id
                ))
            })?;
        let genesis_hash = state_transaction
            .block_hashes()
            .first()
            .map(|hash| *hash.as_ref())
            .ok_or_else(|| {
                Error::InvariantViolation(
                    "privacy proof admission requires a committed genesis block".into(),
                )
            })?;
        let pgc_snapshot =
            if self.envelope.protocol_id == PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
                let namespace = PrivacyNamespaceV1::from_statement(&self.envelope.statement);
                Some(
                    load_privacy_pgc_pool_snapshot_v1(
                        namespace,
                        state_transaction
                            .world
                            .privacy_consensus_policy
                            .get()
                            .current_limits
                            .retained_root_count,
                        &state_transaction.world.privacy_pgc_accounts,
                        &state_transaction.world.privacy_pgc_pool_invariants,
                        &state_transaction.world.privacy_roots,
                        &state_transaction.world.privacy_root_heads,
                    )
                    .map_err(|error| {
                        Error::InvariantViolation(
                            format!("trusted Anonymous PGC pool state failed validation: {error}")
                                .into(),
                        )
                    })?,
                )
            } else {
                None
            };
        let zk_ams_snapshot = if self.envelope.protocol_id == PrivacyProtocolIdV1::IrohaZkAmsV1 {
            let PrivacyStatementV1::IrohaZkAmsV1(statement) = &self.envelope.statement else {
                return Err(invalid_privacy_parameter(
                    "ZK-AMS protocol envelope carries a different statement type",
                ));
            };
            let namespace = PrivacyNamespaceV1::from_statement(&self.envelope.statement);
            let snapshot = load_privacy_zk_ams_registry_snapshot_v1(
                namespace,
                state_transaction
                    .world
                    .privacy_consensus_policy
                    .get()
                    .admission_retained_root_count(),
                &state_transaction.world.privacy_commitments,
                &state_transaction.world.privacy_roots,
                &state_transaction.world.privacy_root_heads,
            )
            .map_err(|error| {
                Error::InvariantViolation(
                    format!("trusted ZK-AMS registry state failed validation: {error}").into(),
                )
            })?;
            if snapshot.retained_current_root()
                != Some((snapshot.current_epoch(), snapshot.current_root()))
            {
                return Err(Error::InvariantViolation(
                    "trusted ZK-AMS current root is not retained".into(),
                ));
            }
            let expected_issuer_record = zk_ams_issuer_policy_record_digest_v1(
                statement.issuer_id,
                statement.policy_id,
                statement.issuer_public_key,
                statement.policy_digest,
            );
            if statement.issuer_policy_record_digest != expected_issuer_record
                || expected_issuer_record != snapshot.issuer_policy_record_digest()
            {
                return Err(invalid_privacy_parameter(
                    "ZK-AMS statement issuer key/policy does not match governed state",
                ));
            }
            let expected_registry_record = zk_ams_registry_record_digest_v1(
                statement.issuer_id,
                statement.registry_id,
                statement.policy_id,
                expected_issuer_record,
                statement.policy_digest,
                snapshot.current_root(),
                snapshot.current_epoch(),
            );
            if statement.registry_record_digest != expected_registry_record {
                return Err(invalid_privacy_parameter(
                    "ZK-AMS statement registry record does not match the authoritative head",
                ));
            }

            match &statement.action {
                PrivacyZkAmsActionV1::BatchAdmission(batch) => {
                    if batch.account_registry_root != snapshot.current_root()
                        || batch.account_registry_root_epoch != snapshot.current_epoch()
                    {
                        return Err(invalid_privacy_parameter(
                            "ZK-AMS batch admission references a stale or future registry head",
                        ));
                    }
                    let expected_next_epoch =
                        snapshot.current_epoch().checked_add(1).ok_or_else(|| {
                            Error::InvariantViolation("ZK-AMS registry epoch overflow".into())
                        })?;
                    if batch.next_account_registry_root_epoch != expected_next_epoch {
                        return Err(invalid_privacy_parameter(
                            "ZK-AMS batch admission does not advance exactly one epoch",
                        ));
                    }
                    let batch_size = u32::try_from(batch.anchors.len()).map_err(|_| {
                        invalid_privacy_parameter("ZK-AMS batch anchor count cannot be represented")
                    })?;
                    let mut computed_root = snapshot.current_root();
                    for (index, anchor) in batch.anchors.iter().copied().enumerate() {
                        let anchor_index = u32::try_from(index).map_err(|_| {
                            invalid_privacy_parameter(
                                "ZK-AMS batch anchor index cannot be represented",
                            )
                        })?;
                        computed_root = zk_ams_registry_transition_root_v1(
                            statement.registry_id,
                            computed_root,
                            snapshot.current_epoch(),
                            expected_next_epoch,
                            batch_size,
                            anchor_index,
                            anchor,
                        );
                        let phc_key =
                            PrivacyCommitmentKeyV1::zk_ams_phc(namespace, anchor.phc_hash)
                                .map_err(invalid_privacy_parameter)?;
                        if let Some(record) =
                            state_transaction.world.privacy_commitments.get(&phc_key)
                        {
                            record.validate().map_err(|error| {
                                Error::InvariantViolation(
                                    format!("persisted ZK-AMS PHC provenance is invalid: {error}")
                                        .into(),
                                )
                            })?;
                            if !matches!(
                                record,
                                PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                                    bootstrap_digest,
                                    ..
                                } if *bootstrap_digest == snapshot.bootstrap_digest()
                            ) {
                                return Err(Error::InvariantViolation(
                                    "persisted ZK-AMS PHC has cross-bootstrap provenance".into(),
                                ));
                            }
                            return Err(invalid_privacy_parameter(
                                "ZK-AMS PHC hash was already admitted",
                            ));
                        }
                        let seed_key = PrivacyCommitmentKeyV1::zk_ams_seed_key(
                            namespace,
                            anchor.seed_public_key,
                        )
                        .map_err(invalid_privacy_parameter)?;
                        if let Some(record) =
                            state_transaction.world.privacy_commitments.get(&seed_key)
                        {
                            record.validate().map_err(|error| {
                                Error::InvariantViolation(
                                    format!(
                                        "persisted ZK-AMS seed-key provenance is invalid: {error}"
                                    )
                                    .into(),
                                )
                            })?;
                            if !matches!(
                                record,
                                PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                                    bootstrap_digest,
                                    ..
                                } if *bootstrap_digest == snapshot.bootstrap_digest()
                            ) {
                                return Err(Error::InvariantViolation(
                                    "persisted ZK-AMS seed key has cross-bootstrap provenance"
                                        .into(),
                                ));
                            }
                            return Err(invalid_privacy_parameter(
                                "ZK-AMS seed key was already admitted",
                            ));
                        }
                    }
                    if computed_root != batch.next_account_registry_root {
                        return Err(invalid_privacy_parameter(
                            "ZK-AMS batch successor root is not the canonical ordered transition",
                        ));
                    }
                }
                PrivacyZkAmsActionV1::ProvisionAccount(provision) => {
                    if provision.account_registry_root != snapshot.current_root()
                        || provision.account_registry_root_epoch != snapshot.current_epoch()
                    {
                        return Err(invalid_privacy_parameter(
                            "ZK-AMS provisioning references a stale or future registry head",
                        ));
                    }
                    for seed_public_key in &provision.admitted_seed_key_ring {
                        let seed_key =
                            PrivacyCommitmentKeyV1::zk_ams_seed_key(namespace, *seed_public_key)
                                .map_err(invalid_privacy_parameter)?;
                        let record = state_transaction
                            .world
                            .privacy_commitments
                            .get(&seed_key)
                            .ok_or_else(|| {
                                invalid_privacy_parameter(
                                    "ZK-AMS provisioning ring contains an unadmitted seed key",
                                )
                            })?;
                        record.validate().map_err(|error| {
                            Error::InvariantViolation(
                                format!("persisted ZK-AMS seed-key provenance is invalid: {error}")
                                    .into(),
                            )
                        })?;
                        if !matches!(
                            record,
                            PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                                bootstrap_digest,
                                ..
                            } if *bootstrap_digest == snapshot.bootstrap_digest()
                        ) {
                            return Err(Error::InvariantViolation(
                                "persisted ZK-AMS seed key has cross-bootstrap provenance".into(),
                            ));
                        }
                    }
                    let image_key =
                        PrivacyNullifierKeyV1::zk_ams_key_image(namespace, provision.key_image)
                            .map_err(invalid_privacy_parameter)?;
                    if let Some(record) = state_transaction.world.privacy_nullifiers.get(&image_key)
                    {
                        record.validate().map_err(|error| {
                            Error::InvariantViolation(
                                format!(
                                    "persisted ZK-AMS key-image provenance is invalid: {error}"
                                )
                                .into(),
                            )
                        })?;
                        if !matches!(
                            record,
                            PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                                bootstrap_digest,
                                ..
                            } if *bootstrap_digest == snapshot.bootstrap_digest()
                        ) {
                            return Err(Error::InvariantViolation(
                                "persisted ZK-AMS key image has cross-bootstrap provenance".into(),
                            ));
                        }
                        return Err(invalid_privacy_parameter(
                            "ZK-AMS provisioning key image was already consumed",
                        ));
                    }
                    if state_transaction
                        .world
                        .accounts
                        .get(&provision.account_id)
                        .is_some()
                    {
                        return Err(invalid_privacy_parameter(
                            "ZK-AMS provisioning target account already exists",
                        ));
                    }
                }
            }
            Some(snapshot)
        } else {
            None
        };
        let (zk_ace_policy, zk_ace_replay_key) = if self.envelope.protocol_id
            == PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
        {
            let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) = &self.envelope.statement
            else {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE protocol envelope carries a different statement type",
                ));
            };
            let policy_key = PrivacyCommitmentKeyV1::zk_ace_policy(statement.policy_id)
                .map_err(invalid_privacy_parameter)?;
            if state_transaction
                .world
                .privacy_commitments
                .get(&policy_key)
                .is_none()
            {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE authoritative policy is not registered",
                ));
            }
            let policy = load_privacy_zk_ace_policy_v1(
                statement.policy_id,
                &state_transaction.world.privacy_commitments,
            )
            .map_err(|error| {
                Error::InvariantViolation(
                    format!("trusted ZK-ACE policy state failed validation: {error}").into(),
                )
            })?;
            if policy.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE authoritative policy is revoked",
                ));
            }
            if policy.policy_id != statement.policy_id
                || policy.policy_digest != statement.policy_digest
                || policy.identity_commitment != statement.identity_commitment
                || policy.authorization_epoch != statement.authorization_epoch
                || policy.asset_definition_id != statement.asset_definition_id
            {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE statement does not exactly match authoritative policy state",
                ));
            }
            if policy
                .source_allowlist
                .binary_search(&statement.source)
                .is_err()
            {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE source account is not authorized by policy",
                ));
            }
            if state_transaction
                .world
                .accounts
                .get(&statement.source)
                .is_none()
            {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE source account does not exist",
                ));
            }
            if state_transaction
                .world
                .accounts
                .get(&statement.destination)
                .is_none()
            {
                return Err(invalid_privacy_parameter(
                    "ZK-ACE destination account does not exist",
                ));
            }
            state_transaction
                .world
                .asset_definition(&statement.asset_definition_id)
                .map_err(Error::from)?;
            let replay_key = PrivacyNullifierKeyV1::zk_ace_replay(
                statement.policy_id,
                statement.replay_nullifier,
            )
            .map_err(invalid_privacy_parameter)?;
            if let Some(record) = state_transaction.world.privacy_nullifiers.get(&replay_key) {
                record.validate().map_err(|error| {
                    Error::InvariantViolation(
                        format!("persisted ZK-ACE replay provenance is invalid: {error}").into(),
                    )
                })?;
                if !matches!(
                    record,
                    PrivacyStateItemRecordV1::ZkAceVerifiedAuthorization {
                        policy_id,
                        ..
                    } if *policy_id == statement.policy_id
                ) {
                    return Err(Error::InvariantViolation(
                        "persisted ZK-ACE replay marker has wrong-policy provenance".into(),
                    ));
                }
                return Err(invalid_privacy_parameter(
                    "ZK-ACE replay nullifier was already consumed",
                ));
            }
            (Some(policy), Some(replay_key))
        } else {
            (None, None)
        };
        let pgc_verification_state =
            pgc_snapshot
                .as_ref()
                .map(|snapshot| PrivacyPgcVerificationStateV1 {
                    namespace: snapshot.namespace(),
                    total_supply: snapshot.invariant().total_supply(),
                    bootstrap_digest: snapshot.invariant().bootstrap_digest(),
                    bootstrap_proof_digest: snapshot.invariant().bootstrap_proof_digest(),
                    current_root: snapshot.current_root(),
                    current_epoch: snapshot.current_epoch(),
                    retained_current_root: snapshot.retained_current_root(),
                    accounts: snapshot.accounts(),
                });
        let effects = verify_privacy_envelope_v1(
            &self.envelope,
            PrivacyVerificationContextV1 {
                activation: &activation,
                consensus_limits: &state_transaction
                    .world
                    .privacy_consensus_policy
                    .get()
                    .current_limits,
                chain_id: &state_transaction.chain_id,
                genesis_hash,
                current_height: state_transaction.block_height(),
                expected_action_index,
                block_timestamp_ms: state_transaction.block_unix_timestamp_ms(),
                pgc_state: pgc_verification_state,
            },
        )
        .map_err(privacy_verification_error)?;

        if effects.protocol_id() != self.envelope.protocol_id
            || effects.statement_digest() != self.envelope.statement_digest
            || effects.action_index() != expected_action_index
            || effects.encoded_action_bytes() != encoded_action_bytes
        {
            return Err(Error::InvariantViolation(
                "native privacy verifier returned effects inconsistent with its envelope".into(),
            ));
        }

        match effects.into_ledger() {
            VerifiedPrivacyLedgerEffectsV1::None => state_transaction
                .reserve_privacy_action(expected_action_index, encoded_action_bytes),
            VerifiedPrivacyLedgerEffectsV1::ZkAceAuthorization(effect) => {
                let policy = zk_ace_policy.as_ref().ok_or_else(|| {
                    Error::InvariantViolation(
                        "native ZK-ACE effect has no trusted policy state".into(),
                    )
                })?;
                let replay_key = zk_ace_replay_key.ok_or_else(|| {
                    Error::InvariantViolation(
                        "native ZK-ACE effect has no trusted replay key".into(),
                    )
                })?;
                let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) =
                    &self.envelope.statement
                else {
                    return Err(Error::InvariantViolation(
                        "native ZK-ACE effect has a different statement type".into(),
                    ));
                };
                if effect.policy_id != statement.policy_id
                    || effect.policy_digest != statement.policy_digest
                    || effect.identity_commitment != statement.identity_commitment
                    || effect.authorization_epoch != statement.authorization_epoch
                    || effect.source != statement.source
                    || effect.destination != statement.destination
                    || effect.asset_definition_id != statement.asset_definition_id
                    || effect.amount != statement.amount
                    || effect.replay_nullifier != statement.replay_nullifier
                    || effect.policy_id != policy.policy_id
                    || effect.policy_digest != policy.policy_digest
                    || effect.identity_commitment != policy.identity_commitment
                    || effect.authorization_epoch != policy.authorization_epoch
                    || effect.asset_definition_id != policy.asset_definition_id
                    || policy
                        .source_allowlist
                        .binary_search(&effect.source)
                        .is_err()
                {
                    return Err(Error::InvariantViolation(
                        "native ZK-ACE effect is inconsistent with trusted state or its statement"
                            .into(),
                    ));
                }
                let expected_replay_key =
                    PrivacyNullifierKeyV1::zk_ace_replay(effect.policy_id, effect.replay_nullifier)
                        .map_err(|error| {
                            Error::InvariantViolation(
                                format!("verified ZK-ACE replay key is invalid: {error}").into(),
                            )
                        })?;
                if replay_key != expected_replay_key {
                    return Err(Error::InvariantViolation(
                        "native ZK-ACE effect changed its replay key".into(),
                    ));
                }
                if state_transaction
                    .world
                    .privacy_nullifiers
                    .get(&replay_key)
                    .is_some()
                {
                    return Err(invalid_privacy_parameter(
                        "verified ZK-ACE replay nullifier was already consumed",
                    ));
                }
                let replay_record = PrivacyStateItemRecordV1::zk_ace_verified_authorization(
                    policy.policy_id,
                    policy.record_digest,
                    self.envelope.statement_digest,
                    state_transaction.block_height(),
                    expected_action_index,
                )
                .map_err(invalid_privacy_parameter)?;
                let source_asset_id = crate::smartcontracts::world::isi::privacy_public_asset_id(
                    state_transaction,
                    &effect.asset_definition_id,
                    &effect.source,
                )?;
                let amount = Quantity::from(effect.amount);

                // `fee` is already committed by the transaction intent and
                // proof statement. This handler has no independent fee sink,
                // so it is deliberately neither added to `amount` nor charged
                // a second time here.
                let _bound_fee_without_second_charge = statement.fee;
                state_transaction
                    .reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
                Transfer::asset_quantity(source_asset_id, amount, effect.destination)
                    .execute(&effect.source, state_transaction)?;
                state_transaction
                    .world
                    .privacy_nullifiers
                    .insert(replay_key, replay_record);
                Ok(())
            }
            VerifiedPrivacyLedgerEffectsV1::ZkAmsBatchAdmission(effect) => {
                let snapshot = zk_ams_snapshot.as_ref().ok_or_else(|| {
                    Error::InvariantViolation(
                        "native ZK-AMS batch effect has no trusted registry snapshot".into(),
                    )
                })?;
                let PrivacyStatementV1::IrohaZkAmsV1(statement) = &self.envelope.statement else {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS batch effect has a different statement type".into(),
                    ));
                };
                let PrivacyZkAmsActionV1::BatchAdmission(batch) = &statement.action else {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS batch effect has a provisioning statement".into(),
                    ));
                };
                let expected_next_epoch =
                    snapshot.current_epoch().checked_add(1).ok_or_else(|| {
                        Error::InvariantViolation("ZK-AMS registry epoch overflow".into())
                    })?;
                if effect.issuer_id != statement.issuer_id
                    || effect.policy_id != statement.policy_id
                    || effect.policy_digest != statement.policy_digest
                    || effect.issuer_policy_record_digest != snapshot.issuer_policy_record_digest()
                    || effect.registry_id != statement.registry_id
                    || effect.registry_record_digest != statement.registry_record_digest
                    || effect.current_root != snapshot.current_root()
                    || effect.current_epoch != snapshot.current_epoch()
                    || effect.next_root != batch.next_account_registry_root
                    || effect.next_epoch != expected_next_epoch
                    || effect.anchors != batch.anchors
                {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS batch effect is inconsistent with trusted state or its statement"
                            .into(),
                    ));
                }
                let expected_registry_record = zk_ams_registry_record_digest_v1(
                    effect.issuer_id,
                    effect.registry_id,
                    effect.policy_id,
                    effect.issuer_policy_record_digest,
                    effect.policy_digest,
                    snapshot.current_root(),
                    snapshot.current_epoch(),
                );
                if expected_registry_record != effect.registry_record_digest {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS batch effect carries a non-authoritative registry record"
                            .into(),
                    ));
                }
                let batch_size = u32::try_from(effect.anchors.len()).map_err(|_| {
                    Error::InvariantViolation(
                        "verified ZK-AMS anchor count cannot be represented".into(),
                    )
                })?;
                let mut computed_next_root = snapshot.current_root();
                let mut item_keys = Vec::with_capacity(effect.anchors.len().saturating_mul(2));
                let mut seen_item_keys = BTreeSet::new();
                for (index, anchor) in effect.anchors.iter().copied().enumerate() {
                    let anchor_index = u32::try_from(index).map_err(|_| {
                        Error::InvariantViolation(
                            "verified ZK-AMS anchor index cannot be represented".into(),
                        )
                    })?;
                    computed_next_root = zk_ams_registry_transition_root_v1(
                        effect.registry_id,
                        computed_next_root,
                        snapshot.current_epoch(),
                        expected_next_epoch,
                        batch_size,
                        anchor_index,
                        anchor,
                    );
                    let phc_key =
                        PrivacyCommitmentKeyV1::zk_ams_phc(snapshot.namespace(), anchor.phc_hash)
                            .map_err(|error| {
                            Error::InvariantViolation(
                                format!("verified ZK-AMS PHC key is invalid: {error}").into(),
                            )
                        })?;
                    let seed_key = PrivacyCommitmentKeyV1::zk_ams_seed_key(
                        snapshot.namespace(),
                        anchor.seed_public_key,
                    )
                    .map_err(|error| {
                        Error::InvariantViolation(
                            format!("verified ZK-AMS seed key is invalid: {error}").into(),
                        )
                    })?;
                    if !seen_item_keys.insert(phc_key) || !seen_item_keys.insert(seed_key) {
                        return Err(Error::InvariantViolation(
                            "verified ZK-AMS batch contains duplicate typed state keys".into(),
                        ));
                    }
                    if state_transaction
                        .world
                        .privacy_commitments
                        .get(&phc_key)
                        .is_some()
                        || state_transaction
                            .world
                            .privacy_commitments
                            .get(&seed_key)
                            .is_some()
                    {
                        return Err(invalid_privacy_parameter(
                            "verified ZK-AMS batch attempts to re-admit existing state",
                        ));
                    }
                    item_keys.push(phc_key);
                    item_keys.push(seed_key);
                }
                if computed_next_root != effect.next_root {
                    return Err(Error::InvariantViolation(
                        "verified ZK-AMS successor root is inconsistent".into(),
                    ));
                }

                let item_provenance = PrivacyStateItemRecordV1::zk_ams_verified_proof(
                    snapshot.bootstrap_digest(),
                    self.envelope.statement_digest,
                    state_transaction.block_height(),
                    expected_action_index,
                )
                .map_err(invalid_privacy_parameter)?;
                let root_provenance = PrivacyRootProvenanceV1::zk_ams_registry_successor(
                    snapshot.bootstrap_digest(),
                    self.envelope.statement_digest,
                    state_transaction.block_height(),
                    expected_action_index,
                    snapshot.current_epoch(),
                    snapshot.current_root(),
                )
                .map_err(invalid_privacy_parameter)?;
                let root_key = PrivacyRootKeyV1::new(
                    snapshot.namespace(),
                    PrivacyRootRoleV1::AccountRegistry,
                    expected_next_epoch,
                    computed_next_root,
                )
                .map_err(invalid_privacy_parameter)?;
                let head_key = PrivacyRootHeadKeyV1::new(
                    snapshot.namespace(),
                    PrivacyRootRoleV1::AccountRegistry,
                )
                .map_err(invalid_privacy_parameter)?;
                let removals = plan_privacy_root_history_update_v1(
                    &state_transaction.world.privacy_roots,
                    &[root_key],
                    state_transaction
                        .world
                        .privacy_consensus_policy
                        .get()
                        .admission_retained_root_count(),
                )
                .map_err(|error| {
                    invalid_privacy_parameter(format!(
                        "ZK-AMS AccountRegistry successor rejected: {error}"
                    ))
                })?;
                let retention_anchor = removals
                    .last()
                    .map(|key| PrivacyRootRetentionAnchorV1::new(key.epoch(), key.root()))
                    .transpose()
                    .map_err(invalid_privacy_parameter)?
                    .or(snapshot.retention_anchor());
                let root_head = PrivacyRootHeadRecordV1::new(
                    expected_next_epoch,
                    computed_next_root,
                    root_provenance,
                    retention_anchor,
                )
                .map_err(invalid_privacy_parameter)?;

                state_transaction
                    .reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
                for key in removals {
                    state_transaction.world.privacy_roots.remove(key);
                }
                for key in item_keys {
                    state_transaction
                        .world
                        .privacy_commitments
                        .insert(key, item_provenance.clone());
                }
                state_transaction
                    .world
                    .privacy_roots
                    .insert(root_key, root_provenance);
                state_transaction
                    .world
                    .privacy_root_heads
                    .insert(head_key, root_head);
                Ok(())
            }
            VerifiedPrivacyLedgerEffectsV1::ZkAmsProvisionAccount(effect) => {
                let snapshot = zk_ams_snapshot.as_ref().ok_or_else(|| {
                    Error::InvariantViolation(
                        "native ZK-AMS provisioning effect has no trusted registry snapshot".into(),
                    )
                })?;
                let PrivacyStatementV1::IrohaZkAmsV1(statement) = &self.envelope.statement else {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS provisioning effect has a different statement type".into(),
                    ));
                };
                let PrivacyZkAmsActionV1::ProvisionAccount(provision) = &statement.action else {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS provisioning effect has a batch statement".into(),
                    ));
                };
                if effect.issuer_id != statement.issuer_id
                    || effect.policy_id != statement.policy_id
                    || effect.policy_digest != statement.policy_digest
                    || effect.issuer_policy_record_digest != snapshot.issuer_policy_record_digest()
                    || effect.registry_id != statement.registry_id
                    || effect.registry_record_digest != statement.registry_record_digest
                    || effect.current_root != snapshot.current_root()
                    || effect.current_epoch != snapshot.current_epoch()
                    || effect.ring != provision.admitted_seed_key_ring
                    || effect.account_id != provision.account_id
                    || effect.key_image != provision.key_image
                {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS provisioning effect is inconsistent with trusted state or its statement"
                            .into(),
                    ));
                }
                let expected_registry_record = zk_ams_registry_record_digest_v1(
                    effect.issuer_id,
                    effect.registry_id,
                    effect.policy_id,
                    effect.issuer_policy_record_digest,
                    effect.policy_digest,
                    snapshot.current_root(),
                    snapshot.current_epoch(),
                );
                if expected_registry_record != effect.registry_record_digest {
                    return Err(Error::InvariantViolation(
                        "native ZK-AMS provisioning effect carries a non-authoritative registry record"
                            .into(),
                    ));
                }
                for seed_public_key in &effect.ring {
                    let seed_key = PrivacyCommitmentKeyV1::zk_ams_seed_key(
                        snapshot.namespace(),
                        *seed_public_key,
                    )
                    .map_err(|error| {
                        Error::InvariantViolation(
                            format!("verified ZK-AMS ring key is invalid: {error}").into(),
                        )
                    })?;
                    let record = state_transaction
                        .world
                        .privacy_commitments
                        .get(&seed_key)
                        .ok_or_else(|| {
                            invalid_privacy_parameter(
                                "verified ZK-AMS ring contains an unadmitted seed key",
                            )
                        })?;
                    record.validate().map_err(|error| {
                        Error::InvariantViolation(
                            format!("persisted ZK-AMS ring provenance is invalid: {error}").into(),
                        )
                    })?;
                    if !matches!(
                        record,
                        PrivacyStateItemRecordV1::ZkAmsVerifiedProof {
                            bootstrap_digest,
                            ..
                        } if *bootstrap_digest == snapshot.bootstrap_digest()
                    ) {
                        return Err(Error::InvariantViolation(
                            "verified ZK-AMS ring contains cross-bootstrap state".into(),
                        ));
                    }
                }
                let image_key =
                    PrivacyNullifierKeyV1::zk_ams_key_image(snapshot.namespace(), effect.key_image)
                        .map_err(|error| {
                            Error::InvariantViolation(
                                format!("verified ZK-AMS key image is invalid: {error}").into(),
                            )
                        })?;
                if state_transaction
                    .world
                    .privacy_nullifiers
                    .get(&image_key)
                    .is_some()
                {
                    return Err(invalid_privacy_parameter(
                        "verified ZK-AMS provisioning key image was already consumed",
                    ));
                }
                if state_transaction
                    .world
                    .accounts
                    .get(&effect.account_id)
                    .is_some()
                {
                    return Err(invalid_privacy_parameter(
                        "verified ZK-AMS provisioning target account already exists",
                    ));
                }
                super::domain::isi::ensure_controller_capabilities(
                    effect.account_id.controller(),
                    &state_transaction.crypto.allowed_signing,
                    &state_transaction.crypto.allowed_curve_ids,
                )?;
                let item_provenance = PrivacyStateItemRecordV1::zk_ams_verified_proof(
                    snapshot.bootstrap_digest(),
                    self.envelope.statement_digest,
                    state_transaction.block_height(),
                    expected_action_index,
                )
                .map_err(invalid_privacy_parameter)?;

                state_transaction
                    .reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
                Register::account(Account::new(effect.account_id.clone()))
                    .execute(authority, state_transaction)?;
                state_transaction
                    .world
                    .privacy_nullifiers
                    .insert(image_key, item_provenance);
                Ok(())
            }
            VerifiedPrivacyLedgerEffectsV1::AnonymousPgcPayment(effect) => {
                let snapshot = pgc_snapshot.as_ref().ok_or_else(|| {
                    Error::InvariantViolation(
                        "native Anonymous PGC effect has no trusted pool snapshot".into(),
                    )
                })?;
                let expected_next_epoch =
                    snapshot.current_epoch().checked_add(1).ok_or_else(|| {
                        Error::InvariantViolation(
                            "Anonymous PGC account-state epoch overflow".into(),
                        )
                    })?;
                if effect.namespace() != snapshot.namespace()
                    || effect.total_supply() != snapshot.invariant().total_supply()
                    || effect.current_root() != snapshot.current_root()
                    || effect.current_epoch() != snapshot.current_epoch()
                    || effect.next_epoch() != expected_next_epoch
                    || effect.accounts().len() != snapshot.accounts().len()
                    || effect
                        .accounts()
                        .iter()
                        .zip(snapshot.accounts())
                        .any(|(next, current)| next.public_key != current.public_key)
                {
                    return Err(Error::InvariantViolation(
                        "native Anonymous PGC verifier returned effects inconsistent with trusted state"
                            .into(),
                    ));
                }
                let computed_next_root = compute_privacy_pgc_account_state_root_v1(
                    effect.namespace(),
                    effect.next_epoch(),
                    effect.total_supply(),
                    effect.accounts(),
                )
                .map_err(|error| {
                    Error::InvariantViolation(
                        format!("verified Anonymous PGC successor table is not canonical: {error}")
                            .into(),
                    )
                })?;
                if computed_next_root != effect.next_root() {
                    return Err(Error::InvariantViolation(
                        "verified Anonymous PGC successor root is inconsistent".into(),
                    ));
                }

                let account_provenance = PrivacyPgcAccountProvenanceV1::verified_proof(
                    self.envelope.statement_digest,
                    state_transaction.block_height(),
                    expected_action_index,
                )
                .map_err(invalid_privacy_parameter)?;
                let pool_invariant_digest = snapshot
                    .invariant()
                    .digest(effect.namespace())
                    .map_err(|error| {
                        Error::InvariantViolation(
                            format!("verified Anonymous PGC pool invariant digest failed: {error}")
                                .into(),
                        )
                    })?;
                let root_provenance = PrivacyRootProvenanceV1::verified_pgc_successor(
                    self.envelope.statement_digest,
                    state_transaction.block_height(),
                    expected_action_index,
                    snapshot.current_epoch(),
                    snapshot.current_root(),
                    pool_invariant_digest,
                )
                .map_err(invalid_privacy_parameter)?;
                let mut seen_keys = BTreeSet::new();
                let mut account_updates = Vec::with_capacity(effect.accounts().len());
                for account in effect.accounts() {
                    let key = PrivacyPgcAccountKeyV1::new(effect.namespace(), account.public_key)
                        .map_err(invalid_privacy_parameter)?;
                    if !seen_keys.insert(key) {
                        return Err(Error::InvariantViolation(
                            "native Anonymous PGC successor contains duplicate account keys".into(),
                        ));
                    }
                    let state = PrivacyPgcAccountStateV1::new(
                        account.encrypted_balance,
                        effect.next_epoch(),
                        account_provenance,
                    )
                    .map_err(invalid_privacy_parameter)?;
                    account_updates.push((key, state));
                }
                let root_key = PrivacyRootKeyV1::new(
                    effect.namespace(),
                    PrivacyRootRoleV1::PgcAccountState,
                    effect.next_epoch(),
                    effect.next_root(),
                )
                .map_err(invalid_privacy_parameter)?;
                let head_key = PrivacyRootHeadKeyV1::new(
                    effect.namespace(),
                    PrivacyRootRoleV1::PgcAccountState,
                )
                .map_err(invalid_privacy_parameter)?;
                let removals = plan_privacy_root_history_update_v1(
                    &state_transaction.world.privacy_roots,
                    &[root_key],
                    state_transaction
                        .world
                        .privacy_consensus_policy
                        .get()
                        .current_limits
                        .retained_root_count,
                )
                .map_err(|error| {
                    invalid_privacy_parameter(format!(
                        "Anonymous PGC successor root rejected: {error}"
                    ))
                })?;
                let retention_anchor = removals
                    .last()
                    .map(|key| PrivacyRootRetentionAnchorV1::new(key.epoch(), key.root()))
                    .transpose()
                    .map_err(invalid_privacy_parameter)?
                    .or(snapshot.retention_anchor());
                let root_head = PrivacyRootHeadRecordV1::new(
                    effect.next_epoch(),
                    effect.next_root(),
                    root_provenance,
                    retention_anchor,
                )
                .map_err(invalid_privacy_parameter)?;

                state_transaction
                    .reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
                for key in removals {
                    state_transaction.world.privacy_roots.remove(key);
                }
                for (key, state) in account_updates {
                    state_transaction
                        .world
                        .privacy_pgc_accounts
                        .insert(key, state);
                }
                state_transaction
                    .world
                    .privacy_roots
                    .insert(root_key, root_provenance);
                state_transaction
                    .world
                    .privacy_root_heads
                    .insert(head_key, root_head);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;
    use std::{str::FromStr as _, sync::OnceLock};

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        Registrable,
        account::Account,
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::DomainId,
        name::Name,
        privacy::{
            AnonymousPgcActivationLimitsV1, AnonymousPgcKOutOfNStatementV1,
            PrivacyActiveLifecycleV1, PrivacyCommitmentV1, PrivacyConsensusLimitsV1,
            PrivacyNamespaceScopeV1, PrivacyNamespaceV1, PrivacyP256CiphertextV1,
            PrivacyP256PointV1, PrivacyPgcAccountBootstrapV1, PrivacyPgcAccountV1,
            PrivacyPgcBootstrapProofBytesV1, PrivacyPolicyDigestV1, PrivacyPolicyIdV1,
            PrivacyPoolIdV1, PrivacyPoolNamespaceV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1,
            PrivacyProofV1, PrivacyProposedLifecycleV1, PrivacyProtocolActivationLimitsV1,
            PrivacyProtocolIdV1, PrivacyRootV1, PrivacyStatementContextV1, PrivacyStatementV1,
            PrivacyTransactionIntentDigestV1, PrivacyZkAcePolicyLifecycleV1,
            PrivacyZkAcePolicyRecordV1, TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
        },
    };
    use iroha_test_samples::ALICE_ID;
    use rand_core_06::{CryptoRng, Error as RngError, RngCore};
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        kura::Kura,
        privacy_engines::{
            anonymous_pgc::{
                AnonymousPgcPoolInvariantV1, TwistedElGamalKeyPairV1, add_ciphertexts,
                bootstrap::{AnonymousPgcBootstrapWitnessV1, prove_bootstrap},
                encrypt_with_randomness,
                payment::{
                    AnonymousPgcPaymentStatementV1, AnonymousPgcPaymentWitnessV1,
                    encrypt_signed_with_randomness, prove_payment,
                },
            },
            p256::SecretScalarV1,
        },
        privacy_profiles::compiled_privacy_profile_v1,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    const TEST_CHAIN_ID: &str = "taira-pgc-runtime-test";
    const TEST_GENESIS_HASH: [u8; 32] = [0x91; 32];
    const TEST_BLOCK_HEIGHT: u64 = 2;

    struct KatRng {
        seed: [u8; 32],
        counter: u64,
    }

    impl KatRng {
        const fn new(seed: [u8; 32]) -> Self {
            Self { seed, counter: 0 }
        }
    }

    impl RngCore for KatRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0; 4];
            self.fill_bytes(&mut bytes);
            u32::from_be_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0; 8];
            self.fill_bytes(&mut bytes);
            u64::from_be_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for chunk in destination.chunks_mut(32) {
                let mut hash = Sha256::new();
                hash.update(b"iroha.privacy.pgc-runtime-test-rng.v1");
                hash.update(self.seed);
                hash.update(self.counter.to_be_bytes());
                self.counter = self.counter.wrapping_add(1);
                let block: [u8; 32] = hash.finalize().into();
                chunk.copy_from_slice(&block[..chunk.len()]);
            }
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), RngError> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for KatRng {}

    fn secret(value: u64) -> SecretScalarV1 {
        let mut bytes = [0; 32];
        bytes[24..].copy_from_slice(&value.to_be_bytes());
        SecretScalarV1::from_bytes(bytes).expect("canonical non-zero scalar")
    }

    fn active_lifecycle() -> PrivacyProtocolLifecycleV1 {
        PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 2,
        })
    }

    fn valid_bootstrap_instruction() -> BootstrapPrivacyPgcAccountsV1 {
        static INSTRUCTION: OnceLock<BootstrapPrivacyPgcAccountsV1> = OnceLock::new();
        INSTRUCTION
            .get_or_init(|| {
                let namespace = PrivacyNamespaceV1::new(
                    PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                    PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                        pool_id: PrivacyPoolIdV1::new([0xA1; 32]),
                    }),
                );
                let balances = vec![100_u32; 16];
                let total_supply = balances.iter().copied().sum();
                let mut key_pairs = (2_u64..18)
                    .map(|value| {
                        TwistedElGamalKeyPairV1::from_secret(secret(value)).expect("PGC key pair")
                    })
                    .collect::<Vec<_>>();
                key_pairs.sort_by_key(TwistedElGamalKeyPairV1::public_key);
                let public_keys = key_pairs
                    .iter()
                    .map(TwistedElGamalKeyPairV1::public_key)
                    .collect::<Vec<_>>();
                let randomness = (0_u64..16)
                    .map(|index| secret(200 + index))
                    .collect::<Vec<_>>();
                let encrypted_balances = public_keys
                    .iter()
                    .copied()
                    .zip(&balances)
                    .zip(&randomness)
                    .map(|((public_key, balance), randomness)| {
                        encrypt_with_randomness(public_key, *balance, randomness)
                            .expect("PGC encrypted balance")
                    })
                    .collect::<Vec<_>>();
                let accounts = public_keys
                    .iter()
                    .zip(&encrypted_balances)
                    .map(|(public_key, encrypted_balance)| PrivacyPgcAccountV1 {
                        public_key: PrivacyP256PointV1::new(*public_key.as_point().as_bytes()),
                        encrypted_balance: PrivacyP256CiphertextV1 {
                            left: PrivacyP256PointV1::new(*encrypted_balance.left().as_bytes()),
                            right: PrivacyP256PointV1::new(*encrypted_balance.right().as_bytes()),
                        },
                    })
                    .collect::<Vec<_>>();
                let initial_epoch = 1;
                let initial_root = compute_privacy_pgc_account_state_root_v1(
                    namespace,
                    initial_epoch,
                    total_supply,
                    &accounts,
                )
                .expect("canonical PGC root");
                let bootstrap = PrivacyPgcAccountBootstrapV1 {
                    namespace,
                    initial_root,
                    initial_epoch,
                    total_supply,
                    accounts,
                };
                let bootstrap_digest = bootstrap.digest().expect("bootstrap digest");
                let namespace_encoding = norito::to_bytes(&namespace).expect("namespace encoding");
                let compiled =
                    compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
                        .expect("compiled Anonymous PGC profile");
                let parameters = AnonymousPgcParametersV1::get().expect("Anonymous PGC parameters");
                let statement = AnonymousPgcBootstrapStatementV1::new(
                    &namespace_encoding,
                    *initial_root.as_bytes(),
                    initial_epoch,
                    total_supply,
                    &public_keys,
                    &encrypted_balances,
                    TranscriptBindingV1 {
                        chain_id: TEST_CHAIN_ID.as_bytes(),
                        genesis_hash: TEST_GENESIS_HASH,
                        action_index: 0,
                        statement_digest: *bootstrap_digest.as_bytes(),
                        parameter_id: *compiled.parameter_id.as_bytes(),
                        parameter_digest: *compiled.parameter_digest.as_bytes(),
                        verifier_digest: *compiled.verifier_digest.as_bytes(),
                        statement_schema_digest: *compiled.statement_schema_digest.as_bytes(),
                        engine_manifest_digest: *compiled.engine_manifest_digest.as_bytes(),
                        generator_digest: parameters.generator_digest(),
                    },
                )
                .expect("native bootstrap statement");
                let witness = AnonymousPgcBootstrapWitnessV1 {
                    balances: &balances,
                    randomness: &randomness,
                };
                let proof = prove_bootstrap(&statement, &witness, &mut KatRng::new([0xB7; 32]))
                    .expect("native bootstrap proof")
                    .encode();
                BootstrapPrivacyPgcAccountsV1::new(
                    bootstrap,
                    PrivacyPgcBootstrapProofBytesV1::new(proof),
                )
            })
            .clone()
    }

    fn valid_payment_instruction() -> SubmitPrivacyProofV1 {
        static INSTRUCTION: OnceLock<SubmitPrivacyProofV1> = OnceLock::new();
        INSTRUCTION
            .get_or_init(|| {
                let bootstrap_instruction = valid_bootstrap_instruction();
                let bootstrap = &bootstrap_instruction.bootstrap;
                let compiled =
                    compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
                        .expect("compiled Anonymous PGC profile");
                let mut key_pairs = (2_u64..18)
                    .map(|value| {
                        TwistedElGamalKeyPairV1::from_secret(secret(value)).expect("PGC key pair")
                    })
                    .collect::<Vec<_>>();
                key_pairs.sort_by_key(TwistedElGamalKeyPairV1::public_key);
                let public_keys = key_pairs
                    .iter()
                    .map(TwistedElGamalKeyPairV1::public_key)
                    .collect::<Vec<_>>();
                let current_balances = bootstrap
                    .accounts
                    .iter()
                    .zip(&public_keys)
                    .map(|(account, public_key)| {
                        assert_eq!(
                            account.public_key.as_bytes(),
                            public_key.as_point().as_bytes(),
                            "payment key order must equal the bootstrapped table"
                        );
                        TwistedElGamalCiphertextV1::from_sec1_bytes(
                            account.encrypted_balance.left.as_bytes(),
                            account.encrypted_balance.right.as_bytes(),
                        )
                        .expect("bootstrapped ciphertext")
                    })
                    .collect::<Vec<_>>();
                let sender_index = 7;
                let recipient_count = 2;
                let mut transfer_values = vec![0_i64; public_keys.len()];
                transfer_values[2] = 20;
                transfer_values[12] = 30;
                transfer_values[sender_index] = -50;
                let transfer_randomness = (0..public_keys.len())
                    .map(|index| {
                        secret(100 + u64::try_from(index).expect("payment fixture index fits u64"))
                    })
                    .collect::<Vec<_>>();
                let transfers = public_keys
                    .iter()
                    .copied()
                    .zip(&transfer_values)
                    .zip(&transfer_randomness)
                    .map(|((public_key, value), randomness)| {
                        encrypt_signed_with_randomness(public_key, *value, randomness)
                            .expect("signed transfer ciphertext")
                    })
                    .collect::<Vec<_>>();
                let next_balances = current_balances
                    .iter()
                    .copied()
                    .zip(&transfers)
                    .map(|(current, transfer)| {
                        add_ciphertexts(current, *transfer).expect("successor balance")
                    })
                    .collect::<Vec<_>>();
                let transfer_ciphertexts = transfers
                    .iter()
                    .map(|ciphertext| PrivacyP256CiphertextV1 {
                        left: PrivacyP256PointV1::new(*ciphertext.left().as_bytes()),
                        right: PrivacyP256PointV1::new(*ciphertext.right().as_bytes()),
                    })
                    .collect::<Vec<_>>();
                let next_accounts = public_keys
                    .iter()
                    .zip(&next_balances)
                    .map(|(public_key, ciphertext)| PrivacyPgcAccountV1 {
                        public_key: PrivacyP256PointV1::new(*public_key.as_point().as_bytes()),
                        encrypted_balance: PrivacyP256CiphertextV1 {
                            left: PrivacyP256PointV1::new(*ciphertext.left().as_bytes()),
                            right: PrivacyP256PointV1::new(*ciphertext.right().as_bytes()),
                        },
                    })
                    .collect::<Vec<_>>();
                let next_epoch = bootstrap.initial_epoch + 1;
                let next_root = compute_privacy_pgc_account_state_root_v1(
                    bootstrap.namespace,
                    next_epoch,
                    bootstrap.total_supply,
                    &next_accounts,
                )
                .expect("successor account root");
                let context = PrivacyStatementContextV1 {
                    chain_id: TEST_CHAIN_ID.into(),
                    action_index: 0,
                    transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0xD0; 32]),
                    parameter_id: compiled.parameter_id,
                    parameter_digest: compiled.parameter_digest,
                    verifier_digest: compiled.verifier_digest,
                    statement_schema_digest: compiled.statement_schema_digest,
                    engine_manifest_digest: compiled.engine_manifest_digest,
                };
                let statement =
                    PrivacyStatementV1::AnonymousPgcKOutOfNV1(AnonymousPgcKOutOfNStatementV1 {
                        context,
                        asset_definition_id: AssetDefinitionId::new(
                            DomainId::try_new("privacy", "universal").expect("privacy domain"),
                            Name::from_str("pgc_cash").expect("asset name"),
                        ),
                        pool_id: PrivacyPoolIdV1::new([0xA1; 32]),
                        account_state_root: bootstrap.initial_root,
                        account_state_root_epoch: bootstrap.initial_epoch,
                        next_account_state_root: next_root,
                        next_account_state_root_epoch: next_epoch,
                        anonymity_set_public_keys: bootstrap
                            .accounts
                            .iter()
                            .map(|account| account.public_key)
                            .collect(),
                        transfer_ciphertexts,
                        recipient_count: u32::try_from(recipient_count)
                            .expect("recipient count fits u32"),
                    });
                let statement_digest = statement.digest().expect("payment statement digest");
                let parameters = AnonymousPgcParametersV1::get().expect("Anonymous PGC parameters");
                let bootstrap_digest = bootstrap.digest().expect("bootstrap digest");
                let bootstrap_proof_digest = bootstrap_instruction
                    .proof
                    .digest()
                    .expect("bootstrap proof digest");
                let pool_invariant = AnonymousPgcPoolInvariantV1::new(
                    bootstrap.total_supply,
                    *bootstrap_digest.as_bytes(),
                    *bootstrap_proof_digest.as_bytes(),
                )
                .expect("native pool invariant");
                let native_statement = AnonymousPgcPaymentStatementV1::new(
                    &public_keys,
                    &transfers,
                    &current_balances,
                    recipient_count,
                    pool_invariant,
                    TranscriptBindingV1 {
                        chain_id: TEST_CHAIN_ID.as_bytes(),
                        genesis_hash: TEST_GENESIS_HASH,
                        action_index: 0,
                        statement_digest: *statement_digest.as_bytes(),
                        parameter_id: *compiled.parameter_id.as_bytes(),
                        parameter_digest: *compiled.parameter_digest.as_bytes(),
                        verifier_digest: *compiled.verifier_digest.as_bytes(),
                        statement_schema_digest: *compiled.statement_schema_digest.as_bytes(),
                        engine_manifest_digest: *compiled.engine_manifest_digest.as_bytes(),
                        generator_digest: parameters.generator_digest(),
                    },
                )
                .expect("native payment statement");
                let witness = AnonymousPgcPaymentWitnessV1 {
                    transfer_values: &transfer_values,
                    transfer_randomness: &transfer_randomness,
                    sender_index,
                    sender_secret: key_pairs[sender_index].secret_scalar(),
                };
                let proof =
                    prove_payment(&native_statement, &witness, &mut KatRng::new([0xC7; 32]))
                        .expect("native payment proof")
                        .encode();
                SubmitPrivacyProofV1::new(PrivacyProofEnvelopeV1 {
                    protocol_id: compiled.protocol_id,
                    proof_system_id: compiled.proof_system_id,
                    engine_id: compiled.engine_id,
                    parameter_id: compiled.parameter_id,
                    parameter_digest: compiled.parameter_digest,
                    verifier_digest: compiled.verifier_digest,
                    statement_schema_digest: compiled.statement_schema_digest,
                    engine_manifest_digest: compiled.engine_manifest_digest,
                    statement_digest,
                    statement,
                    proof: PrivacyProofV1::AnonymousPgcKOutOfNV1(PrivacyProofBytesV1::new(proof)),
                })
            })
            .clone()
    }

    fn bind_payment_instruction(
        transaction: &mut StateTransaction<'_, '_>,
        instruction: &SubmitPrivacyProofV1,
    ) {
        let digest = instruction
            .envelope
            .statement
            .context()
            .transaction_intent_digest;
        let submission_hash = crate::privacy::privacy_signed_submission_hash_v1(instruction)
            .expect("payment instruction encodes canonically");
        transaction.bind_privacy_transaction_intent_v1(Some((digest, submission_hash)));
    }

    fn state_with_activation(lifecycle: PrivacyProtocolLifecycleV1) -> State {
        let activation = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("compiled Anonymous PGC profile")
            .activation_record(lifecycle);
        let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut world = World::with([], [alice], []);
        world.privacy_activations.insert(
            PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1),
            activation,
        );
        let mut state = State::new_with_chain_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            TEST_CHAIN_ID.into(),
        );
        state.push_block_hash_for_testing(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed(TEST_GENESIS_HASH),
        ));
        state
    }

    fn test_header() -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(TEST_BLOCK_HEIGHT).expect("non-zero height"),
            None,
            None,
            None,
            1_800_000_000_000,
            0,
        )
    }

    fn grant_governance(state_transaction: &mut StateTransaction<'_, '_>) {
        state_transaction
            .world
            .add_account_permission(&ALICE_ID, Permission::from(CanEnactGovernance));
    }

    fn privacy_map_counts(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> (usize, usize, usize, usize) {
        (
            state_transaction
                .world
                .privacy_pgc_pool_invariants
                .iter()
                .count(),
            state_transaction.world.privacy_pgc_accounts.iter().count(),
            state_transaction.world.privacy_roots.iter().count(),
            state_transaction.world.privacy_root_heads.iter().count(),
        )
    }

    fn smart_contract_parameter_message(error: &Error) -> &str {
        let Error::InvalidParameter(InvalidParameterError::SmartContract(message)) = error else {
            panic!("expected a typed smart-contract parameter error, got {error:?}");
        };
        message
    }

    fn assert_empty_and_unbudgeted(state_transaction: &StateTransaction<'_, '_>) {
        assert_eq!(privacy_map_counts(state_transaction), (0, 0, 0, 0));
        assert_eq!(state_transaction.privacy_budget_for_testing(), (0, 0, 0, 0));
    }

    #[test]
    fn consensus_policy_schedule_rejects_bad_authority_timing_limits_and_overwrite() {
        let mut next_limits = PrivacyConsensusLimitsV1::taira_default();
        next_limits.max_actions_per_block -= 1;
        next_limits.retained_root_count -= 1;
        let valid =
            SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT + 300, next_limits);
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        let original = *transaction.world.privacy_consensus_policy.get();

        let error = valid
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("governance permission is mandatory");
        assert!(error.to_string().contains("CanEnactGovernance"), "{error}");
        assert_eq!(*transaction.world.privacy_consensus_policy.get(), original);

        grant_governance(&mut transaction);
        for invalid in [
            SchedulePrivacyConsensusPolicyTighteningV1::new(1, next_limits),
            SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT, next_limits),
            SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT + 299, next_limits),
            SchedulePrivacyConsensusPolicyTighteningV1::new(
                TEST_BLOCK_HEIGHT + 300,
                PrivacyConsensusLimitsV1::taira_default(),
            ),
            {
                let mut increased = next_limits;
                increased.max_actions_per_block =
                    PrivacyConsensusLimitsV1::taira_default().max_actions_per_block + 1;
                SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT + 300, increased)
            },
        ] {
            invalid
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("malformed or non-tightening schedule");
            assert_eq!(
                *transaction.world.privacy_consensus_policy.get(),
                original,
                "rejected scheduling must be read-only"
            );
        }

        valid
            .execute(&ALICE_ID, &mut transaction)
            .expect("exact +300 strict tightening");
        let scheduled = *transaction.world.privacy_consensus_policy.get();
        assert_eq!(scheduled.current_limits, original.current_limits);
        let pending = scheduled.pending_tightening.expect("pending tightening");
        assert_eq!(pending.scheduled_at_height, TEST_BLOCK_HEIGHT);
        assert_eq!(pending.effective_at_height, TEST_BLOCK_HEIGHT + 300);
        assert_eq!(pending.next_limits, next_limits);

        let mut other_limits = next_limits;
        other_limits.retained_root_count -= 1;
        let error =
            SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT + 301, other_limits)
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("a pending schedule cannot be overwritten");
        assert!(
            error.to_string().contains("already has a pending"),
            "{error}"
        );
        assert_eq!(
            *transaction.world.privacy_consensus_policy.get(),
            scheduled,
            "overwrite rejection must preserve the first schedule byte-for-byte"
        );
    }

    #[test]
    fn protocol_limit_schedule_rejects_bad_authority_mismatch_increase_noop_and_overwrite() {
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        let key = PrivacyActivationKeyV1::new(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1);
        let mut current = *transaction
            .world
            .privacy_activations
            .get(&key)
            .expect("Anonymous PGC activation");
        current.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 32,
                max_recipient_count: 8,
            },
        );
        transaction.world.privacy_activations.insert(key, current);
        let next = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 8,
            },
        );
        let valid = SchedulePrivacyProtocolLimitsTighteningV1::new(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            TEST_BLOCK_HEIGHT + 300,
            next,
        );

        let error = valid
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("governance permission is mandatory");
        assert!(error.to_string().contains("CanEnactGovernance"), "{error}");
        assert_eq!(
            transaction.world.privacy_activations.get(&key),
            Some(&current)
        );

        grant_governance(&mut transaction);
        let invalid = [
            SchedulePrivacyProtocolLimitsTighteningV1::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                TEST_BLOCK_HEIGHT + 299,
                next,
            ),
            SchedulePrivacyProtocolLimitsTighteningV1::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                TEST_BLOCK_HEIGHT + 300,
                current.protocol_limits,
            ),
            SchedulePrivacyProtocolLimitsTighteningV1::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                TEST_BLOCK_HEIGHT + 300,
                PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                    AnonymousPgcActivationLimitsV1 {
                        max_anonymity_set_size: 64,
                        max_recipient_count: 8,
                    },
                ),
            ),
            SchedulePrivacyProtocolLimitsTighteningV1::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                TEST_BLOCK_HEIGHT + 300,
                PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                    iroha_data_model::privacy::VeRangeActivationLimitsV1 {
                        max_aggregation_count: 8,
                    },
                ),
            ),
            SchedulePrivacyProtocolLimitsTighteningV1::new(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                TEST_BLOCK_HEIGHT + 300,
                PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(
                    iroha_data_model::privacy::PqMaspActivationLimitsV1 {
                        max_input_count: 1,
                        max_output_count: 1,
                    },
                ),
            ),
        ];
        for instruction in invalid {
            instruction
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("invalid protocol schedule");
            assert_eq!(
                transaction.world.privacy_activations.get(&key),
                Some(&current),
                "rejected scheduling must preserve the activation"
            );
        }

        valid
            .execute(&ALICE_ID, &mut transaction)
            .expect("exact +300 strict protocol tightening");
        let scheduled = *transaction
            .world
            .privacy_activations
            .get(&key)
            .expect("scheduled activation");
        assert_eq!(scheduled.protocol_limits, current.protocol_limits);
        let pending = scheduled
            .pending_protocol_limits_tightening
            .expect("pending protocol limits");
        assert_eq!(pending.scheduled_at_height, TEST_BLOCK_HEIGHT);
        assert_eq!(pending.effective_at_height, TEST_BLOCK_HEIGHT + 300);
        assert_eq!(pending.next_limits, next);

        let error = SchedulePrivacyProtocolLimitsTighteningV1::new(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            TEST_BLOCK_HEIGHT + 301,
            next,
        )
        .execute(&ALICE_ID, &mut transaction)
        .expect_err("a pending protocol schedule cannot be overwritten");
        assert!(
            error.to_string().contains("already has a pending"),
            "{error}"
        );
        assert_eq!(
            transaction.world.privacy_activations.get(&key),
            Some(&scheduled)
        );
    }

    #[test]
    fn pgc_bootstrap_rejects_authority_inactive_and_future_activation_without_mutation() {
        let instruction = valid_bootstrap_instruction();

        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        let error = instruction
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("authority without exact governance permission");
        assert!(error.to_string().contains("CanEnactGovernance"), "{error}");
        assert_empty_and_unbudgeted(&transaction);

        for lifecycle in [
            PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
                proposed_at_height: 1,
                activate_at_height: 20,
            }),
            PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 20,
                state_since_height: 20,
            }),
        ] {
            let state = state_with_activation(lifecycle);
            let mut block = state.block(test_header());
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            let error = instruction
                .clone()
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("inactive or future activation");
            assert!(
                error.to_string().contains("before") || error.to_string().contains("not effective"),
                "{error}"
            );
            assert_empty_and_unbudgeted(&transaction);
        }
    }

    #[test]
    fn pgc_bootstrap_cap_and_action_budget_rejections_are_non_mutating() {
        let mut oversized = valid_bootstrap_instruction();
        oversized.proof = PrivacyPgcBootstrapProofBytesV1::new(vec![
            1;
            usize::try_from(
                TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1
            )
            .expect("proof cap fits usize")
                + 1
        ]);
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        let error = oversized
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("oversized bootstrap proof");
        assert!(error.to_string().contains("exceeding maximum"), "{error}");
        assert_empty_and_unbudgeted(&transaction);

        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        transaction
            .reserve_privacy_action(0, 64)
            .expect("consume the sole transaction action");
        let budget_before = transaction.privacy_budget_for_testing();
        let error = valid_bootstrap_instruction()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("exhausted action budget");
        assert!(
            error
                .to_string()
                .contains("action count per transaction exceeded"),
            "{error}"
        );
        assert_eq!(privacy_map_counts(&transaction), (0, 0, 0, 0));
        assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
    }

    #[test]
    fn altered_pgc_bootstrap_fields_and_proof_leave_all_state_unchanged() {
        let mut altered_root = valid_bootstrap_instruction();
        altered_root.bootstrap.initial_root = PrivacyRootV1::new([0xD1; 32]);
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        let error = altered_root
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("altered public bootstrap root");
        assert!(error.to_string().contains("root does not match"), "{error}");
        assert_empty_and_unbudgeted(&transaction);

        let mut altered_supply = valid_bootstrap_instruction();
        altered_supply.bootstrap.total_supply += 1;
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        let error = altered_supply
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("altered public aggregate supply");
        assert!(error.to_string().contains("root does not match"), "{error}");
        assert_empty_and_unbudgeted(&transaction);

        let mut altered_proof = valid_bootstrap_instruction();
        let middle = altered_proof.proof.bytes.len() / 2;
        altered_proof.proof.bytes[middle] ^= 1;
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        let error = altered_proof
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("altered native proof");
        assert!(
            error.to_string().contains("proof verification failed"),
            "{error}"
        );
        assert_empty_and_unbudgeted(&transaction);
    }

    #[test]
    fn pgc_bootstrap_rejects_each_preexisting_orphan_without_new_mutation() {
        #[derive(Clone, Copy, Debug)]
        enum Orphan {
            Invariant,
            Account,
            Root,
            Head,
        }

        let instruction = valid_bootstrap_instruction();
        let bootstrap_digest = instruction.bootstrap.digest().expect("bootstrap digest");
        let proof_digest = instruction.proof.digest().expect("proof digest");
        let root_provenance =
            PrivacyRootProvenanceV1::verified_bootstrap(bootstrap_digest, proof_digest, 9)
                .expect("root provenance");
        let account_provenance =
            PrivacyPgcAccountProvenanceV1::bootstrap(bootstrap_digest, proof_digest, 9)
                .expect("account provenance");
        let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(instruction.bootstrap.namespace)
            .expect("invariant key");
        let invariant = PrivacyPgcPoolInvariantV1::new(
            instruction.bootstrap.total_supply,
            instruction.bootstrap.initial_root,
            bootstrap_digest,
            proof_digest,
        )
        .expect("invariant");
        let first_account = &instruction.bootstrap.accounts[0];
        let account_key =
            PrivacyPgcAccountKeyV1::new(instruction.bootstrap.namespace, first_account.public_key)
                .expect("account key");
        let account_state = PrivacyPgcAccountStateV1::new(
            first_account.encrypted_balance,
            instruction.bootstrap.initial_epoch,
            account_provenance,
        )
        .expect("account state");
        let root_key = PrivacyRootKeyV1::new(
            instruction.bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
            instruction.bootstrap.initial_epoch,
            instruction.bootstrap.initial_root,
        )
        .expect("root key");
        let head_key = PrivacyRootHeadKeyV1::new(
            instruction.bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
        )
        .expect("head key");
        let head = PrivacyRootHeadRecordV1::new(
            instruction.bootstrap.initial_epoch,
            instruction.bootstrap.initial_root,
            root_provenance,
            None,
        )
        .expect("root head");

        for orphan in [
            Orphan::Invariant,
            Orphan::Account,
            Orphan::Root,
            Orphan::Head,
        ] {
            let state = state_with_activation(active_lifecycle());
            let mut block = state.block(test_header());
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            match orphan {
                Orphan::Invariant => {
                    transaction
                        .world
                        .privacy_pgc_pool_invariants
                        .insert(invariant_key, invariant);
                }
                Orphan::Account => {
                    transaction
                        .world
                        .privacy_pgc_accounts
                        .insert(account_key, account_state);
                }
                Orphan::Root => {
                    transaction
                        .world
                        .privacy_roots
                        .insert(root_key, root_provenance);
                }
                Orphan::Head => {
                    transaction.world.privacy_root_heads.insert(head_key, head);
                }
            }
            let counts_before = privacy_map_counts(&transaction);
            let budget_before = transaction.privacy_budget_for_testing();
            assert!(
                valid_bootstrap_instruction()
                    .execute(&ALICE_ID, &mut transaction)
                    .is_err(),
                "{orphan:?} must reject"
            );
            assert_eq!(
                privacy_map_counts(&transaction),
                counts_before,
                "{orphan:?}: rejection added privacy state"
            );
            assert_eq!(
                transaction.privacy_budget_for_testing(),
                budget_before,
                "{orphan:?}: rejection consumed privacy budget"
            );
        }
    }

    #[test]
    fn successful_pgc_bootstrap_is_atomic_and_double_bootstrap_rejects() {
        let instruction = valid_bootstrap_instruction();
        let encoded_action_bytes = u64::try_from(
            norito::to_bytes(&instruction)
                .expect("instruction encoding")
                .len(),
        )
        .expect("encoded length");
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());

        {
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            instruction
                .clone()
                .execute(&ALICE_ID, &mut transaction)
                .expect("complete native bootstrap");
            assert_eq!(privacy_map_counts(&transaction), (1, 16, 1, 1));
            assert_eq!(
                transaction.privacy_budget_for_testing(),
                (1, encoded_action_bytes, 1, encoded_action_bytes)
            );
            transaction.apply();
        }

        {
            let mut transaction = block.transaction();
            let counts_before = privacy_map_counts(&transaction);
            let budget_before = transaction.privacy_budget_for_testing();
            let error = instruction
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("double bootstrap");
            assert!(
                smart_contract_parameter_message(&error).contains("already initialized"),
                "{error:?}"
            );
            assert_eq!(privacy_map_counts(&transaction), counts_before);
            assert_eq!(transaction.privacy_budget_for_testing(), budget_before);
        }
    }

    #[test]
    fn payment_rejects_missing_stale_substituted_and_consumed_intent_before_effects() {
        let bootstrap = valid_bootstrap_instruction();
        let payment = valid_payment_instruction();
        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        {
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            bootstrap
                .execute(&ALICE_ID, &mut transaction)
                .expect("complete native bootstrap");
            transaction.apply();
        }

        let assert_unchanged =
            |transaction: &StateTransaction<'_, '_>,
             expected_maps: (usize, usize, usize, usize),
             expected_budget: (u32, u64, u32, u64)| {
                assert_eq!(privacy_map_counts(transaction), expected_maps);
                assert_eq!(
                    transaction.privacy_budget_for_testing(),
                    expected_budget,
                    "intent rejection must not reserve privacy budget"
                );
            };

        {
            let mut transaction = block.transaction();
            let before = privacy_map_counts(&transaction);
            let budget_before = transaction.privacy_budget_for_testing();
            let error = payment
                .clone()
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("contract, trigger, IVM, and ad-hoc paths have no direct binding");
            assert!(
                smart_contract_parameter_message(&error).contains("no bound direct"),
                "{error:?}"
            );
            assert_unchanged(&transaction, before, budget_before);
        }

        {
            let mut transaction = block.transaction();
            let before = privacy_map_counts(&transaction);
            let budget_before = transaction.privacy_budget_for_testing();
            let submission_hash = crate::privacy::privacy_signed_submission_hash_v1(&payment)
                .expect("payment submission hash");
            transaction.bind_privacy_transaction_intent_v1(Some((
                PrivacyTransactionIntentDigestV1::new([0xEE; 32]),
                submission_hash,
            )));
            let error = payment
                .clone()
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("stale signed-payload binding");
            assert!(
                smart_contract_parameter_message(&error).contains("digest differs"),
                "{error:?}"
            );
            assert_unchanged(&transaction, before, budget_before);
        }

        {
            let mut substituted = payment.clone();
            substituted.envelope.proof.bytes_mut().bytes[0] ^= 1;
            let mut transaction = block.transaction();
            let before = privacy_map_counts(&transaction);
            let budget_before = transaction.privacy_budget_for_testing();
            bind_payment_instruction(&mut transaction, &payment);
            let error = substituted
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("child overlay substituted another proof");
            assert!(
                smart_contract_parameter_message(&error).contains("differs from the exact direct"),
                "{error:?}"
            );
            assert_unchanged(&transaction, before, budget_before);
        }

        {
            let mut transaction = block.transaction();
            let before = privacy_map_counts(&transaction);
            let budget_before = transaction.privacy_budget_for_testing();
            bind_payment_instruction(&mut transaction, &payment);
            let digest = payment
                .envelope
                .statement
                .context()
                .transaction_intent_digest;
            let submission_hash = crate::privacy::privacy_signed_submission_hash_v1(&payment)
                .expect("payment submission hash");
            transaction
                .consume_privacy_transaction_intent_v1(digest, submission_hash)
                .expect("simulate prior child consumption");
            let error = payment
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("the exact submission cannot be replayed in a child overlay");
            assert!(
                smart_contract_parameter_message(&error).contains("already been consumed"),
                "{error:?}"
            );
            assert_unchanged(&transaction, before, budget_before);
        }
    }

    #[test]
    fn tampered_pgc_payment_proof_preserves_every_state_map_and_budget() {
        let bootstrap = valid_bootstrap_instruction();
        let mut payment = valid_payment_instruction();
        let PrivacyProofV1::AnonymousPgcKOutOfNV1(proof) = &mut payment.envelope.proof else {
            unreachable!("Anonymous PGC payment fixture")
        };
        let middle = proof.bytes.len() / 2;
        proof.bytes[middle] ^= 1;

        let state = state_with_activation(active_lifecycle());
        let mut block = state.block(test_header());
        {
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            bootstrap
                .execute(&ALICE_ID, &mut transaction)
                .expect("complete native bootstrap");
            transaction.apply();
        }

        let mut transaction = block.transaction();
        let invariants_before = transaction
            .world
            .privacy_pgc_pool_invariants
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>();
        let accounts_before = transaction
            .world
            .privacy_pgc_accounts
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>();
        let roots_before = transaction
            .world
            .privacy_roots
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>();
        let heads_before = transaction
            .world
            .privacy_root_heads
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>();
        let budget_before = transaction.privacy_budget_for_testing();

        bind_payment_instruction(&mut transaction, &payment);
        let error = payment
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("one-bit proof mutation");
        assert_eq!(
            smart_contract_parameter_message(&error),
            "privacy proof admission rejected: native Anonymous-PGC verification failed: \
             Anonymous-PGC payment proof equation failed",
            "unexpected typed proof rejection: {error:?}"
        );
        assert_eq!(
            transaction
                .world
                .privacy_pgc_pool_invariants
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            invariants_before
        );
        assert_eq!(
            transaction
                .world
                .privacy_pgc_accounts
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            accounts_before
        );
        assert_eq!(
            transaction
                .world
                .privacy_roots
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            roots_before
        );
        assert_eq!(
            transaction
                .world
                .privacy_root_heads
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            heads_before
        );
        assert_eq!(
            transaction.privacy_budget_for_testing(),
            budget_before,
            "failed native verification cannot reserve transaction or block budget"
        );
    }

    #[test]
    fn verified_pgc_payment_replaces_complete_table_atomically_and_replay_rejects() {
        let bootstrap = valid_bootstrap_instruction();
        let payment = valid_payment_instruction();
        let payment_bytes = u64::try_from(
            norito::to_bytes(&payment.envelope)
                .expect("payment encoding")
                .len(),
        )
        .expect("payment length");
        let state = state_with_activation(active_lifecycle());
        let header = test_header();
        let header_hash = header.hash();
        let mut block = state.block(header);

        {
            let mut transaction = block.transaction();
            grant_governance(&mut transaction);
            bootstrap
                .clone()
                .execute(&ALICE_ID, &mut transaction)
                .expect("complete native bootstrap");
            transaction.apply();
        }

        {
            let mut transaction = block.transaction();
            let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(bootstrap.bootstrap.namespace)
                .expect("invariant key");
            let invariant_before = *transaction
                .world
                .privacy_pgc_pool_invariants
                .get(&invariant_key)
                .expect("bootstrapped invariant");
            let first_key = PrivacyPgcAccountKeyV1::new(
                bootstrap.bootstrap.namespace,
                bootstrap.bootstrap.accounts[0].public_key,
            )
            .expect("first account key");
            let first_balance_before = transaction
                .world
                .privacy_pgc_accounts
                .get(&first_key)
                .expect("first account")
                .encrypted_balance();

            bind_payment_instruction(&mut transaction, &payment);
            payment
                .clone()
                .execute(&ALICE_ID, &mut transaction)
                .expect("complete native payment");
            assert_eq!(privacy_map_counts(&transaction), (1, 16, 2, 1));
            assert_eq!(
                transaction
                    .world
                    .privacy_pgc_pool_invariants
                    .get(&invariant_key),
                Some(&invariant_before),
                "payments cannot replace bootstrap supply provenance"
            );
            assert_ne!(
                transaction
                    .world
                    .privacy_pgc_accounts
                    .get(&first_key)
                    .expect("updated first account")
                    .encrypted_balance(),
                first_balance_before,
                "the complete successor table must replace current ciphertexts"
            );
            let head_key = PrivacyRootHeadKeyV1::new(
                bootstrap.bootstrap.namespace,
                PrivacyRootRoleV1::PgcAccountState,
            )
            .expect("head key");
            assert_eq!(
                transaction
                    .world
                    .privacy_root_heads
                    .get(&head_key)
                    .expect("payment head")
                    .epoch(),
                2
            );
            let budget = transaction.privacy_budget_for_testing();
            assert_eq!(budget.0, 1);
            assert_eq!(budget.1, payment_bytes);
            assert_eq!(budget.2, 2);
            transaction.apply();
        }

        {
            let mut transaction = block.transaction();
            let mut next_limits = PrivacyConsensusLimitsV1::taira_default();
            next_limits.retained_root_count = 1;
            SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT + 300, next_limits)
                .execute(&ALICE_ID, &mut transaction)
                .expect("schedule exact delayed PGC retention tightening");
            transaction.apply();
        }
        block.commit().expect("commit bootstrap and payment block");

        let next_header = BlockHeader::new(
            NonZeroU64::new(TEST_BLOCK_HEIGHT + 300).expect("effective height"),
            Some(header_hash),
            None,
            None,
            1_800_000_000_001,
            0,
        );
        let mut next_block = state.block(next_header);
        let mut transaction = next_block.transaction();
        assert_eq!(
            privacy_map_counts(&transaction),
            (1, 16, 1, 1),
            "effective-height hook must prune PGC history to the tightened cap"
        );
        let head_key = PrivacyRootHeadKeyV1::new(
            bootstrap.bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
        )
        .expect("PGC head key");
        let head = transaction
            .world
            .privacy_root_heads
            .get(&head_key)
            .expect("pruned PGC head");
        let anchor = head
            .retention_anchor()
            .expect("pruning must commit the removed prefix anchor");
        assert_eq!(anchor.epoch(), bootstrap.bootstrap.initial_epoch);
        assert_eq!(anchor.root(), bootstrap.bootstrap.initial_root);
        assert_eq!(
            transaction
                .world
                .privacy_consensus_policy
                .get()
                .current_limits
                .retained_root_count,
            1
        );
        assert_eq!(
            transaction
                .world
                .privacy_consensus_policy
                .get()
                .pending_tightening,
            None
        );
        let counts_before = privacy_map_counts(&transaction);
        bind_payment_instruction(&mut transaction, &payment);
        let error = payment
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("stale payment replay");
        assert!(
            smart_contract_parameter_message(&error).contains("StaleHead"),
            "unexpected replay rejection: {error:?}"
        );
        assert_eq!(privacy_map_counts(&transaction), counts_before);
        assert_eq!(
            transaction.privacy_budget_for_testing(),
            (0, 0, 0, 0),
            "failed replay must not consume the new block budget"
        );
    }
}
