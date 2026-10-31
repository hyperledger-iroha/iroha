//! Envelope checks of the live-upgrade mechanism (`specs/live_runtime_upgrades.md` §5.2).
//!
//! An instruction travels in a stable envelope: its wire ID and framed payload. A release decodes
//! the envelope of a wire ID it registers and keeps every other envelope opaque
//! ([`iroha_data_model::isi::OpaqueInstructionV1`]). Validation walks the envelopes a transaction
//! carries in canonical order and stops at the first one execution can never apply, before any
//! work that depends on what an instruction is: one whose wire ID no interface of the root's
//! selection answers (`definition_inactive`), or one whose interface is active but whose envelope
//! this release kept opaque (`malformed_payload`). A release that knows the wire ID and one that
//! kept the envelope opaque therefore stop at the same envelope with the same rejection, and the
//! block stays valid.
//!
//! A decoded envelope's restrictions are checked when it executes ([`super::resolve`]), where the
//! obligation it acts on is known. An opaque envelope continues no obligation, so its
//! restrictions are checked here, in the order execution checks them.

use iroha_data_model::{
    ValidationFail,
    isi::{InstructionBox, error::InstructionExecutionError, instruction_wire_id},
    runtime::{definition::RootSelectionV1, lifecycle::RuntimeRejectionV1},
    transaction::Executable,
};

/// The rejection of `envelope` when `selection` binds no interface to its wire ID.
///
/// It is the rejection execution-time resolution ([`super::resolve`]) produces for the same
/// envelope. An instruction without a wire ID is never carried by a transaction or contract (it is
/// a private primitive of native code) and is not refused here, as resolution does not refuse it.
pub fn inactive_envelope(
    selection: &RootSelectionV1,
    envelope: &InstructionBox,
) -> Option<RuntimeRejectionV1> {
    let wire_id = instruction_wire_id(envelope)?;
    selection
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
        .is_err()
        .then(|| RuntimeRejectionV1::DefinitionInactive {
            wire_id: wire_id.to_owned(),
        })
}

/// The stable rejection of `instruction` when this release kept its envelope opaque.
///
/// Execution calls it once resolution has admitted the envelope: its wire ID is then bound to an
/// active interface, so the frame must be one this release does not accept as that interface's
/// payload, and the envelope is refused with `malformed_payload` (§5.2). `None` for an instruction
/// this release decoded.
pub fn opaque_rejection(instruction: &InstructionBox) -> Option<RuntimeRejectionV1> {
    instruction
        .as_opaque()
        .map(|opaque| RuntimeRejectionV1::MalformedPayload {
            wire_id: opaque.wire_id().to_owned(),
        })
}

/// Resolve `instruction` where it executes (§5.2, §7.1): against the selection its root executes
/// at `height`, with its restrictions and the live obligation it acts on (`obligation_step`),
/// then refuse an envelope this release kept opaque.
///
/// Every native path that executes an instruction calls it before any check that depends on what
/// the instruction is, including the paths that execute an instruction without the native
/// dispatch table (controlled locks, linked-FI transfers, multisig), so a restriction or
/// containment holds whichever path runs the instruction.
///
/// Returns the implementation the selection binds, or `None` for a private primitive.
///
/// # Errors
/// The stable rejection the live-upgrade mechanism commits for the instruction.
pub fn resolve_envelope(
    selection: &RootSelectionV1,
    height: u64,
    instruction: &InstructionBox,
    obligation_step: impl FnOnce() -> Option<super::obligations::Step>,
) -> Result<Option<iroha_data_model::runtime::definition::DefinitionId>, InstructionExecutionError>
{
    let implementation = super::resolve(instruction, selection, height, obligation_step)?;
    if let Some(rejection) = opaque_rejection(instruction) {
        return Err(rejection.into());
    }
    Ok(implementation)
}

/// Refuse `envelope` if execution can never apply it at `height` (§5.2).
///
/// An unbound wire ID is `definition_inactive`. An opaque envelope is resolved like execution
/// resolves it, continuing no obligation, and then refused with `malformed_payload`. A decoded,
/// bound envelope passes; its restrictions wait for execution and its obligation context.
///
/// # Errors
/// Exactly the rejection execution commits for the same envelope, so the committed result does
/// not depend on which check catches it.
pub fn refuse_unexecutable_envelope(
    selection: &RootSelectionV1,
    height: u64,
    envelope: &InstructionBox,
) -> Result<(), InstructionExecutionError> {
    if let Some(rejection) = inactive_envelope(selection, envelope) {
        return Err(rejection.into());
    }
    if let Some(rejection) = opaque_rejection(envelope) {
        super::resolve(envelope, selection, height, || None)?;
        return Err(rejection.into());
    }
    Ok(())
}

/// Refuse `executable` at its first top-level envelope execution can never apply at `height`:
/// the instructions of an instruction executable, or the instruction items of a batch, in order
/// (§5.2).
///
/// This is the first instruction-dependent step of transaction validation. Contract calls, raw
/// IVM programs and proved-VM overlays carry no top-level envelope; the instructions they emit
/// are refused when they execute.
///
/// # Errors
/// The failure execution commits for that envelope ([`refuse_unexecutable_envelope`]).
pub fn admit_envelopes(
    selection: &RootSelectionV1,
    height: u64,
    executable: &Executable,
) -> Result<(), ValidationFail> {
    executable
        .explicit_instructions()
        .try_for_each(|envelope| refuse_unexecutable_envelope(selection, height, envelope))
        .map_err(ValidationFail::InstructionFailed)
}

/// The rejection of the first top-level envelope of `executable` that the committed `selection`
/// would refuse: an unbound wire ID or an envelope this release kept opaque.
///
/// Queue admission uses it so honest proposers never carry such a transaction. Restrictions are
/// left to execution, where the obligation an instruction acts on is known.
pub fn first_unadmitted_envelope(
    selection: &RootSelectionV1,
    executable: &Executable,
) -> Option<RuntimeRejectionV1> {
    executable.explicit_instructions().find_map(|envelope| {
        inactive_envelope(selection, envelope).or_else(|| opaque_rejection(envelope))
    })
}

#[cfg(test)]
mod tests;
