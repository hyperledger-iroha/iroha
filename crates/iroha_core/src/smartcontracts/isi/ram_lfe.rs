//! Generic RAM-LFE program-policy instruction handlers.

use iroha_data_model::ram_lfe::RamLfeProgramPolicy;
use iroha_telemetry::metrics;

use super::prelude::*;

/// Execution handlers for RAM-LFE program-policy ISIs.
pub mod isi {
    use super::*;
    use crate::state::StateTransaction;

    impl Execute for iroha_data_model::isi::ram_lfe::RegisterRamLfeProgramPolicy {
        #[metrics(+"register_ram_lfe_program_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = self.policy;
            if authority != &policy.owner {
                return Err(Error::InvariantViolation(
                    "Only the program owner can register a RAM-LFE program policy"
                        .to_owned()
                        .into(),
                ));
            }
            if state_transaction
                .world
                .ram_lfe_program_policies
                .get(&policy.program_id)
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is already registered",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            if policy.backend != policy.commitment.backend {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} backend does not match commitment backend",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            state_transaction
                .world
                .ram_lfe_program_policies
                .insert(policy.program_id.clone(), policy);
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::ram_lfe::ActivateRamLfeProgramPolicy {
        #[metrics(+"activate_ram_lfe_program_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = state_transaction
                .world
                .ram_lfe_program_policies
                .get_mut(&self.program_id)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!(
                            "RAM-LFE program policy {} is not registered",
                            self.program_id
                        )
                        .into(),
                    )
                })?;
            ensure_program_policy_owner(authority, policy)?;
            if policy.active {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is already active",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            policy.active = true;
            Ok(())
        }
    }

    impl Execute for iroha_data_model::isi::ram_lfe::DeactivateRamLfeProgramPolicy {
        #[metrics(+"deactivate_ram_lfe_program_policy")]
        fn execute(
            self,
            authority: &AccountId,
            state_transaction: &mut StateTransaction<'_, '_>,
        ) -> Result<(), Error> {
            let policy = state_transaction
                .world
                .ram_lfe_program_policies
                .get_mut(&self.program_id)
                .ok_or_else(|| {
                    Error::InvariantViolation(
                        format!(
                            "RAM-LFE program policy {} is not registered",
                            self.program_id
                        )
                        .into(),
                    )
                })?;
            ensure_program_policy_owner(authority, policy)?;
            if !policy.active {
                return Err(Error::InvariantViolation(
                    format!(
                        "RAM-LFE program policy {} is already inactive",
                        policy.program_id
                    )
                    .into(),
                ));
            }
            policy.active = false;
            Ok(())
        }
    }

    fn ensure_program_policy_owner(
        authority: &AccountId,
        policy: &RamLfeProgramPolicy,
    ) -> Result<(), Error> {
        if authority == &policy.owner {
            return Ok(());
        }
        Err(Error::InvariantViolation(
            "Only the program owner can mutate this RAM-LFE program policy"
                .to_owned()
                .into(),
        ))
    }
}
