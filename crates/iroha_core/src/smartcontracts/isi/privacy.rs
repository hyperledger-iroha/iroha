//! Canonical first-release privacy governance and proof-admission handlers.
//!
//! Governance checks and all deterministic validation precede storage writes.
//! Proof admission is added only through the exhaustive native verifier
//! boundary; there is deliberately no generic or opaque fallback verifier.

use iroha_data_model::{
    isi::{
        error::{InstructionExecutionError as Error, InvalidParameterError},
        privacy::{
            BootstrapPrivacyPgcAccountsV1, PublishPrivacyRootV1,
            RegisterPrivacyProtocolActivationV1, TransitionPrivacyProtocolLifecycleV1,
        },
    },
    permission::Permission,
    prelude::AccountId,
    privacy::{
        PrivacyProtocolLifecycleV1, PrivacyRootManagementV1, PrivacyRootPublicationV1,
        PrivacyRootRoleV1,
    },
};
use iroha_executor_data_model::permission::governance::CanEnactGovernance;
use mv::storage::StorageReadOnly;

use super::Execute;
use crate::{
    privacy::{validate_privacy_lifecycle_transition_v1, validate_privacy_registration_v1},
    privacy_state::{
        PrivacyActivationKeyV1, PrivacyPgcAccountKeyV1, PrivacyPgcAccountProvenanceV1,
        PrivacyPgcAccountStateV1, PrivacyRootHeadKeyV1, PrivacyRootHeadRecordV1, PrivacyRootKeyV1,
        PrivacyRootProvenanceV1, compute_privacy_pgc_account_state_root_v1,
        plan_privacy_root_history_update_v1,
    },
    state::StateTransaction,
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
            &iroha_data_model::privacy::PrivacyConsensusLimitsV1::taira_default(),
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
        let next_head =
            PrivacyRootHeadRecordV1::new(self.publication.epoch, self.publication.root, provenance)
                .map_err(invalid_privacy_parameter)?;
        let removals = plan_privacy_root_history_update_v1(
            &state_transaction.world.privacy_roots,
            &[root_key],
            activation.limits.retained_root_count,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("privacy root publication rejected: {error}"))
        })?;

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

impl Execute for BootstrapPrivacyPgcAccountsV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        ensure_privacy_governance(authority, state_transaction)?;

        self.bootstrap.validate().map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC account bootstrap rejected: {error}"))
        })?;
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
        activation.validate().map_err(|error| {
            invalid_privacy_parameter(format!(
                "registered Anonymous PGC activation is invalid: {error}"
            ))
        })?;
        if matches!(activation.lifecycle, PrivacyProtocolLifecycleV1::Retired(_)) {
            return Err(invalid_privacy_parameter(
                "cannot bootstrap accounts for a retired Anonymous PGC protocol",
            ));
        }

        let head_key =
            PrivacyRootHeadKeyV1::new(self.bootstrap.namespace, PrivacyRootRoleV1::PgcAccountState)
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
            &self.bootstrap.accounts,
        )
        .map_err(invalid_privacy_parameter)?;
        if computed_root != self.bootstrap.initial_root {
            return Err(invalid_privacy_parameter(
                "privacy PGC bootstrap root does not match the canonical account table",
            ));
        }

        let bootstrap_digest = self.bootstrap.digest().map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC bootstrap encoding failed: {error}"))
        })?;
        let account_provenance =
            PrivacyPgcAccountProvenanceV1::bootstrap(bootstrap_digest, current_height)
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

        let publication = PrivacyRootPublicationV1::new(
            self.bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
            self.bootstrap.initial_epoch,
            self.bootstrap.initial_root,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!(
                "privacy PGC bootstrap root publication is invalid: {error}"
            ))
        })?;
        let root_provenance = PrivacyRootProvenanceV1::governance(
            publication.digest().map_err(|error| {
                invalid_privacy_parameter(format!(
                    "privacy PGC root publication encoding failed: {error}"
                ))
            })?,
            current_height,
        )
        .map_err(invalid_privacy_parameter)?;
        let root_key = PrivacyRootKeyV1::new(
            publication.namespace,
            publication.role,
            publication.epoch,
            publication.root,
        )
        .map_err(invalid_privacy_parameter)?;
        let root_head =
            PrivacyRootHeadRecordV1::new(publication.epoch, publication.root, root_provenance)
                .map_err(invalid_privacy_parameter)?;
        let removals = plan_privacy_root_history_update_v1(
            &state_transaction.world.privacy_roots,
            &[root_key],
            activation.limits.retained_root_count,
        )
        .map_err(|error| {
            invalid_privacy_parameter(format!("privacy PGC bootstrap root rejected: {error}"))
        })?;
        if !removals.is_empty() {
            return Err(Error::InvariantViolation(
                "new Anonymous PGC root history unexpectedly requires pruning".into(),
            ));
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
