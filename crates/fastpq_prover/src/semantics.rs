//! Fail-closed semantic profiles for FASTPQ statements.
//!
//! The proof protocol is shared by two statement families. Generic transfer
//! proofs authenticate fully witnessed changes in the transfer gadget's
//! touched-balance tree, while AXT proofs may also authenticate an opaque effect
//! carrier whose state roots are interpreted by the surrounding, independently
//! authenticated AXT statement. Callers must select the family explicitly;
//! batch metadata never selects a profile.

use crate::{Error, OperationKind, Result, TransitionBatch};

/// Semantics that a FASTPQ prover and verifier apply to a transition batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofSemantics {
    /// Generic transfer-balance state-transition proof.
    ///
    /// Non-empty batches must consist entirely of fully witnessed transfers.
    /// `old_root` and `new_root` are roots of the transfer gadget's private
    /// touched-balance tree, not consensus-wide world-state roots. An empty
    /// batch is valid only when it leaves that root unchanged.
    TransferStateTransition,
    /// AXT transfer statement selected by a trusted, canonical outer binding.
    ///
    /// Every row must be a transfer. Transfer transcript and sparse-Merkle
    /// witness validation is performed by the canonical trace builder.
    AxtTransferClaim,
    /// AXT opaque effect selected by a trusted, canonical outer binding.
    ///
    /// Every row must be a metadata carrier. The surrounding AXT statement,
    /// not this profile, gives the opaque old/new roots their meaning. This
    /// profile does not prove authorization or compliance; consumers must
    /// establish those properties through an independently authenticated
    /// authority.
    AxtOpaqueEffect,
}

impl ProofSemantics {
    /// Return the stable diagnostic name for this profile.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TransferStateTransition => "transfer_state_transition",
            Self::AxtTransferClaim => "axt_transfer_claim",
            Self::AxtOpaqueEffect => "axt_opaque_effect",
        }
    }
}

/// Validate the operation shape allowed by an explicitly selected profile.
///
/// This gate deliberately does not inspect metadata to select or relax the
/// profile. In particular, inserting an AXT-looking metadata key cannot make a
/// generic transfer-state proof accept an opaque metadata, supply, or role
/// operation.
///
/// # Errors
///
/// Returns [`Error::InvalidProofSemantics`] when the batch is empty where an
/// execution statement is required, changes roots without witnessed rows, or
/// contains an operation not supported by the selected profile.
pub fn validate_batch_semantics(batch: &TransitionBatch, semantics: ProofSemantics) -> Result<()> {
    match semantics {
        ProofSemantics::TransferStateTransition => validate_transfer_state_transition(batch),
        ProofSemantics::AxtTransferClaim => {
            require_non_empty(batch, semantics)?;
            require_all_operations(batch, semantics, OperationClass::Transfer)
        }
        ProofSemantics::AxtOpaqueEffect => {
            require_non_empty(batch, semantics)?;
            require_all_operations(batch, semantics, OperationClass::MetaSet)
        }
    }
}

fn validate_transfer_state_transition(batch: &TransitionBatch) -> Result<()> {
    if batch.transitions.is_empty() {
        if batch.public_inputs.old_root == batch.public_inputs.new_root {
            return Ok(());
        }
        return Err(invalid(
            ProofSemantics::TransferStateTransition,
            "empty batch changes the public state root",
        ));
    }
    // TODO: Admit Mint, Burn, role, and generic metadata updates only after their supply,
    // permission-table, and state-tree witnesses are constrained and adversarially tested.
    require_all_operations(
        batch,
        ProofSemantics::TransferStateTransition,
        OperationClass::Transfer,
    )
}

#[derive(Debug, Clone, Copy)]
enum OperationClass {
    Transfer,
    MetaSet,
}

impl OperationClass {
    const fn accepts(self, operation: &OperationKind) -> bool {
        match self {
            Self::Transfer => matches!(operation, OperationKind::Transfer),
            Self::MetaSet => matches!(operation, OperationKind::MetaSet),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Transfer => "Transfer",
            Self::MetaSet => "MetaSet",
        }
    }
}

fn require_non_empty(batch: &TransitionBatch, semantics: ProofSemantics) -> Result<()> {
    if batch.transitions.is_empty() {
        Err(invalid(semantics, "execution batch must not be empty"))
    } else {
        Ok(())
    }
}

fn require_all_operations(
    batch: &TransitionBatch,
    semantics: ProofSemantics,
    expected: OperationClass,
) -> Result<()> {
    if let Some((index, transition)) = batch
        .transitions
        .iter()
        .enumerate()
        .find(|(_, transition)| !expected.accepts(&transition.operation))
    {
        return Err(invalid(
            semantics,
            format!(
                "row {index} uses operation rank {}, expected only {} rows",
                transition.operation_rank(),
                expected.name()
            ),
        ));
    }
    Ok(())
}

fn invalid(semantics: ProofSemantics, details: impl Into<String>) -> Error {
    Error::InvalidProofSemantics {
        profile: semantics.name(),
        details: details.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PublicInputs, StateTransition};

    fn batch(old_root: [u8; 32], new_root: [u8; 32]) -> TransitionBatch {
        TransitionBatch::new(
            "fastpq-lane-balanced",
            PublicInputs {
                old_root,
                new_root,
                ..PublicInputs::default()
            },
        )
    }

    fn push(batch: &mut TransitionBatch, operation: OperationKind) {
        batch.push(StateTransition::new(
            b"asset/example/alice".to_vec(),
            7_u64.to_le_bytes().to_vec(),
            8_u64.to_le_bytes().to_vec(),
            operation,
        ));
    }

    #[test]
    fn generic_empty_batch_must_preserve_root() {
        let unchanged = batch([0x11; 32], [0x11; 32]);
        validate_batch_semantics(&unchanged, ProofSemantics::TransferStateTransition)
            .expect("unchanged empty state batch");

        let changed = batch([0x11; 32], [0x22; 32]);
        let error = validate_batch_semantics(&changed, ProofSemantics::TransferStateTransition)
            .expect_err("root-changing empty state batch must fail closed");
        assert!(matches!(
            error,
            Error::InvalidProofSemantics {
                profile: "transfer_state_transition",
                ..
            }
        ));
    }

    #[test]
    fn generic_state_profile_rejects_opaque_metadata_even_with_axt_metadata() {
        let mut candidate = batch([0x11; 32], [0x22; 32]);
        push(&mut candidate, OperationKind::MetaSet);
        candidate
            .metadata
            .insert("axt_fastpq_binding".into(), b"attacker-selected".to_vec());

        let error = validate_batch_semantics(&candidate, ProofSemantics::TransferStateTransition)
            .expect_err("metadata must not select opaque semantics");
        assert!(matches!(
            error,
            Error::InvalidProofSemantics {
                profile: "transfer_state_transition",
                ..
            }
        ));
    }

    #[test]
    fn axt_transfer_profile_rejects_appended_supply_or_metadata_rows() {
        for operation in [OperationKind::Burn, OperationKind::MetaSet] {
            let mut candidate = batch([0x11; 32], [0x22; 32]);
            push(&mut candidate, OperationKind::Transfer);
            push(&mut candidate, operation);
            assert!(matches!(
                validate_batch_semantics(&candidate, ProofSemantics::AxtTransferClaim),
                Err(Error::InvalidProofSemantics {
                    profile: "axt_transfer_claim",
                    ..
                })
            ));
        }
    }

    #[test]
    fn axt_opaque_profile_accepts_only_metadata_carriers() {
        let mut legitimate = batch([0x11; 32], [0x22; 32]);
        push(&mut legitimate, OperationKind::MetaSet);
        validate_batch_semantics(&legitimate, ProofSemantics::AxtOpaqueEffect)
            .expect("opaque AXT metadata carrier");

        let mut role_attack = batch([0x11; 32], [0x22; 32]);
        push(
            &mut role_attack,
            OperationKind::RoleGrant {
                role_id: vec![0x11; 32],
                permission_id: vec![0x22; 32],
                epoch: 7,
            },
        );
        assert!(matches!(
            validate_batch_semantics(&role_attack, ProofSemantics::AxtOpaqueEffect),
            Err(Error::InvalidProofSemantics {
                profile: "axt_opaque_effect",
                ..
            })
        ));
    }
}
