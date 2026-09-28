//! SCCP v1 core: state access, block hooks, admission and the ten v1 instructions
//! (`specs/sccp.md` §4).
//!
//! ws20 installed this skeleton so that later workstreams fill disjoint files without touching
//! shared ones. [`store`], [`leaves`], [`params`], [`height`] and [`witness`] are complete, as
//! are the queue/validation exemption-shape rule in [`admission`] and the [`Execute`]
//! implementations below, which delegate each v1 instruction to exactly one entry point. Every
//! other entry point already has its final signature and fails closed with [`not_wired`] (or
//! returns the neutral value its documentation names) until the owner in its module
//! documentation implements it:
//!
//! | Modules | Owner |
//! |---|---|
//! | [`init`], [`params`], [`bridge_keys`], [`attestations`], [`faults`], [`admission`], [`fees`] | ws31 |
//! | [`hook`], [`roster`], [`commitment`], [`subjects`], [`prune`] | ws30 |
//! | [`outbound`], [`escrow`], [`recipients`] | ws32 |
//! | [`registry`], [`governance`], [`controls`], [`light_clients`] (Parliament half) | ws33 |
//! | [`light_clients`] (advance half), [`inbound`], [`settle`], [`voids`], [`self_claim`] | ws41 |
//!
//! With SCCP absent (no `sccp_parameters`), every hook is a no-op and admission classifies no
//! transaction as SCCP-exempt, so block production and queue behavior are unchanged.

pub mod admission;
pub mod attestations;
pub mod bridge_keys;
pub mod commitment;
pub mod controls;
pub mod escrow;
pub mod faults;
pub mod fees;
pub mod governance;
pub mod height;
pub mod hook;
pub mod inbound;
pub mod init;
pub mod leaves;
pub mod light_clients;
pub mod outbound;
pub mod params;
pub mod prune;
pub mod recipients;
pub mod registry;
pub mod roster;
pub mod self_claim;
pub mod settle;
pub mod store;
pub mod subjects;
pub mod voids;
pub mod witness;

#[cfg(test)]
pub(crate) mod test_support;

use super::Execute;
use crate::state::StateTransaction;
use iroha_data_model::{
    account::AccountId,
    isi::{
        Instruction,
        error::InstructionExecutionError as Error,
        sccp::{
            AdvanceSccpLightClientV1, InitializeSccpV1, RecordSccpMessage,
            ReportSccpLightClientEquivocationV1, SetSccpBridgeKeyV1, SettleSccpV1,
            SubmitSccpAttestationFaultV1, SubmitSccpAttestationsV1, SubmitSccpInboundMessageV1,
            SubmitSccpOutboundVoidV1,
        },
    },
};

/// Build the fail-closed error of an SCCP entry point that is not implemented yet.
///
/// The message names the missing behavior and its owning workstream:
/// `SCCP: <what> not implemented yet (TODO(<owner>))`.
#[must_use]
pub(crate) fn not_wired(what: &str, owner: &str) -> Error {
    Error::InvariantViolation(format!("SCCP: {what} not implemented yet (TODO({owner}))").into())
}

/// Return whether `instruction` is one of the ten SCCP v1 instructions.
///
/// Every SCCP instruction routes to the universal dataspace (§4.19) and is admitted by the
/// Initial executor because core enforces every SCCP rule.
#[must_use]
pub fn is_sccp_instruction(instruction: &dyn Instruction) -> bool {
    let any = instruction.as_any();
    any.is::<InitializeSccpV1>()
        || any.is::<SetSccpBridgeKeyV1>()
        || any.is::<SubmitSccpAttestationsV1>()
        || any.is::<SubmitSccpAttestationFaultV1>()
        || any.is::<RecordSccpMessage>()
        || any.is::<SubmitSccpInboundMessageV1>()
        || any.is::<SettleSccpV1>()
        || any.is::<SubmitSccpOutboundVoidV1>()
        || any.is::<AdvanceSccpLightClientV1>()
        || any.is::<ReportSccpLightClientEquivocationV1>()
}

/// Implement [`Execute`] for each instruction by delegating to its single entry point.
macro_rules! delegate_execute {
    ($($instruction:ty => $entry:path),+ $(,)?) => {$(
        impl Execute for $instruction {
            fn execute(
                self,
                authority: &AccountId,
                state_transaction: &mut StateTransaction<'_, '_>,
            ) -> Result<(), Error> {
                $entry(self, authority, state_transaction)
            }
        }
    )+};
}

delegate_execute! {
    InitializeSccpV1 => init::execute_initialize,
    SetSccpBridgeKeyV1 => bridge_keys::execute_set_bridge_key,
    SubmitSccpAttestationsV1 => attestations::execute_submit_attestations,
    SubmitSccpAttestationFaultV1 => faults::execute_submit_fault,
    RecordSccpMessage => outbound::execute_record,
    SubmitSccpInboundMessageV1 => inbound::execute_submit_inbound,
    SettleSccpV1 => settle::execute_settle,
    SubmitSccpOutboundVoidV1 => voids::execute_submit_void,
    AdvanceSccpLightClientV1 => light_clients::execute_advance,
    ReportSccpLightClientEquivocationV1 => light_clients::execute_report_equivocation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        SampleInstructions, authority, blank_state, header,
    };
    use iroha_data_model::isi::InstructionBox;

    fn todo_owner(error: &Error) -> Option<String> {
        let Error::InvariantViolation(message) = error else {
            return None;
        };
        let (_, tail) = message.split_once("TODO(")?;
        let (owner, _) = tail.split_once(')')?;
        message.starts_with("SCCP: ").then(|| owner.to_owned())
    }

    #[test]
    fn not_wired_names_the_missing_behavior_and_its_owner() {
        let error = not_wired("widget", "ws99");
        assert_eq!(
            error,
            Error::InvariantViolation("SCCP: widget not implemented yet (TODO(ws99))".into())
        );
        assert_eq!(todo_owner(&error).as_deref(), Some("ws99"));
    }

    #[test]
    fn every_v1_instruction_is_recognized_and_nothing_else_is() {
        for instruction in SampleInstructions::all() {
            assert!(is_sccp_instruction(&*instruction), "{instruction:?}");
        }
        let log = InstructionBox::from(iroha_data_model::isi::Log::new(
            iroha_data_model::Level::INFO,
            "not SCCP".to_owned(),
        ));
        assert!(!is_sccp_instruction(&*log));
    }

    #[test]
    fn every_stub_instruction_fails_closed_with_its_owner() {
        let expected = [
            ("InitializeSccpV1", "ws31"),
            ("SetSccpBridgeKeyV1", "ws31"),
            ("SubmitSccpAttestationsV1", "ws31"),
            ("SubmitSccpAttestationFaultV1", "ws31"),
            ("RecordSccpMessage", "ws32"),
            ("SubmitSccpInboundMessageV1", "ws41"),
            ("SettleSccpV1", "ws41"),
            ("SubmitSccpOutboundVoidV1", "ws41"),
            ("AdvanceSccpLightClientV1", "ws41"),
            ("ReportSccpLightClientEquivocationV1", "ws41"),
        ];
        let state = blank_state();
        let mut block = state.block(header(2));
        let authority = authority(1);
        for (instruction, (name, owner)) in SampleInstructions::all().into_iter().zip(expected) {
            let mut stx = block.transaction();
            let error = crate::smartcontracts::isi::execute_borrowed_instruction(
                &instruction,
                &authority,
                &mut stx,
            )
            .expect_err("a skeleton SCCP instruction must fail closed");
            assert_eq!(
                todo_owner(&error).as_deref(),
                Some(owner),
                "{name}: {error}"
            );
            assert!(format!("{error}").contains(name), "{name}: {error}");
        }
    }
}
