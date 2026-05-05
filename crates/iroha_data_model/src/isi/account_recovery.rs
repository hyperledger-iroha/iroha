//! Native account controller replacement and social recovery instructions.

use super::*;

isi! {
    /// Replace the controller governing an existing account while preserving linked state.
    pub struct ReplaceAccountController {
        /// Canonical account identifier to replace.
        pub account: AccountId,
        /// New controller that should govern the account after replacement.
        pub new_controller: crate::account::AccountController,
    }
}

impl ReplaceAccountController {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.controller.replace";
}

impl crate::seal::Instruction for ReplaceAccountController {}

isi! {
    /// Set or replace the alias-keyed recovery policy for an account.
    pub struct SetAccountRecoveryPolicy {
        /// Canonical account identifier whose stable alias policy should be updated.
        pub account: AccountId,
        /// Recovery policy keyed by the account's stable alias.
        pub policy: crate::account::AccountRecoveryPolicy,
    }
}

impl SetAccountRecoveryPolicy {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.policy.set";
}

impl crate::seal::Instruction for SetAccountRecoveryPolicy {}

isi! {
    /// Clear the alias-keyed recovery policy for an account.
    pub struct ClearAccountRecoveryPolicy {
        /// Canonical account identifier whose recovery policy should be cleared.
        pub account: AccountId,
    }
}

impl ClearAccountRecoveryPolicy {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.policy.clear";
}

impl crate::seal::Instruction for ClearAccountRecoveryPolicy {}

isi! {
    /// Propose a controller replacement through the social-recovery workflow.
    pub struct ProposeAccountRecovery {
        /// Stable account alias whose active account should be recovered.
        pub alias: crate::account::AccountAlias,
        /// New controller requested for the alias.
        pub new_controller: crate::account::AccountController,
    }
}

impl ProposeAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.propose";
}

impl crate::seal::Instruction for ProposeAccountRecovery {}

isi! {
    /// Record a guardian approval for the active recovery request of an alias.
    pub struct ApproveAccountRecovery {
        /// Stable account alias whose pending recovery should receive an approval.
        pub alias: crate::account::AccountAlias,
    }
}

impl ApproveAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.approve";
}

impl crate::seal::Instruction for ApproveAccountRecovery {}

isi! {
    /// Cancel a pending social-recovery request for an alias.
    pub struct CancelAccountRecovery {
        /// Stable account alias whose pending recovery should be cancelled.
        pub alias: crate::account::AccountAlias,
    }
}

impl CancelAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.cancel";
}

impl crate::seal::Instruction for CancelAccountRecovery {}

isi! {
    /// Finalize a pending social-recovery request once quorum and timelock are satisfied.
    pub struct FinalizeAccountRecovery {
        /// Stable account alias whose pending recovery should be finalized.
        pub alias: crate::account::AccountAlias,
    }
}

impl FinalizeAccountRecovery {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.account.recovery.finalize";
}

impl crate::seal::Instruction for FinalizeAccountRecovery {}
