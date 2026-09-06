//! Fail-closed semantic profiles for FASTPQ statements.
//!
//! The proof protocol is shared by the root-bound V1 state-transition relation
//! and two narrowly scoped AXT statement families. The wire and trace schemas
//! contain exactly the six release operations, but the production semantic gate
//! admits an operation only after its tree relation is fully authenticated.
//! AXT proofs remain restricted to either witnessed transfers or opaque effect
//! carriers selected by the authenticated outer statement. Batch metadata never
//! selects or relaxes a profile.

use crate::{
    Error, OperationKind, Result, TransitionBatch, axt_binding::AXT_FASTPQ_BINDING_METADATA_KEY,
};

/// Semantics that a FASTPQ prover and verifier apply to a transition batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofSemantics {
    /// Root-bound first-release state-transition proof.
    ///
    /// Transfer rows carry the currently implemented sparse-Merkle update
    /// relation. The other five final operation tags remain unavailable through
    /// this production profile until their supply, permission, membership,
    /// non-membership, and metadata tree paths are bound. An empty batch is
    /// valid only when it leaves the state root unchanged.
    StateTransition,
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
            Self::StateTransition => "state_transition",
            Self::AxtTransferClaim => "axt_transfer_claim",
            Self::AxtOpaqueEffect => "axt_opaque_effect",
        }
    }
}

/// Validate the operation shape allowed by an explicitly selected profile.
///
/// This gate deliberately does not inspect metadata to select or relax the
/// profile. In particular, inserting an AXT-looking metadata key cannot make a
/// generic state proof accept a row without its production tree relation.
///
/// # Errors
///
/// Returns [`Error::InvalidProofSemantics`] when the batch is empty where an
/// execution statement is required, changes roots without witnessed rows, or
/// contains an operation not supported by the selected profile.
pub fn validate_batch_semantics(batch: &TransitionBatch, semantics: ProofSemantics) -> Result<()> {
    match semantics {
        ProofSemantics::StateTransition => validate_state_transition(batch),
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

fn validate_state_transition(batch: &TransitionBatch) -> Result<()> {
    if batch.metadata.contains_key(AXT_FASTPQ_BINDING_METADATA_KEY) {
        return Err(invalid(
            ProofSemantics::StateTransition,
            "AXT-bound batches require an explicitly authenticated AXT semantic profile",
        ));
    }
    if batch.transitions.is_empty() {
        if batch.public_inputs.old_root == batch.public_inputs.new_root {
            return Ok(());
        }
        return Err(invalid(
            ProofSemantics::StateTransition,
            "empty batch changes the public state root",
        ));
    }
    // TODO: Admit each remaining final operation here only with its canonical
    // supply/permission/metadata membership or non-membership path bound to the
    // corresponding public roots. Host-side row checks alone are not a proof of
    // a state transition.
    require_all_operations(
        batch,
        ProofSemantics::StateTransition,
        OperationClass::RootBoundStateTransition,
    )
}

#[derive(Debug, Clone, Copy)]
enum OperationClass {
    RootBoundStateTransition,
    Transfer,
    MetaSet,
}

impl OperationClass {
    const fn accepts(self, operation: &OperationKind) -> bool {
        match self {
            Self::RootBoundStateTransition => matches!(operation, OperationKind::Transfer),
            Self::Transfer => matches!(operation, OperationKind::Transfer),
            Self::MetaSet => matches!(operation, OperationKind::MetaSet),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::RootBoundStateTransition => "a root-bound final V1 state transition",
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
            "fastpq-state-transition-stark-v1",
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
        validate_batch_semantics(&unchanged, ProofSemantics::StateTransition)
            .expect("unchanged empty state batch");

        let changed = batch([0x11; 32], [0x22; 32]);
        let error = validate_batch_semantics(&changed, ProofSemantics::StateTransition)
            .expect_err("root-changing empty state batch must fail closed");
        assert!(matches!(
            error,
            Error::InvalidProofSemantics {
                profile: "state_transition",
                ..
            }
        ));
    }

    #[test]
    fn generic_state_profile_rejects_operations_without_tree_witnesses() {
        for operation in [
            OperationKind::Mint,
            OperationKind::Burn,
            OperationKind::RoleGrant {
                role_id: [0x11; 32],
                permission_id: [0x22; 32],
                epoch: 7,
            },
            OperationKind::RoleRevoke {
                role_id: [0x33; 32],
                permission_id: [0x44; 32],
                epoch: 8,
            },
            OperationKind::MetaSet,
        ] {
            let mut candidate = batch([0x11; 32], [0x22; 32]);
            push(&mut candidate, operation);
            assert!(matches!(
                validate_batch_semantics(&candidate, ProofSemantics::StateTransition),
                Err(Error::InvalidProofSemantics {
                    profile: "state_transition",
                    ..
                })
            ));
        }
    }

    #[test]
    fn generic_state_profile_admits_the_transfer_witness_class() {
        let mut candidate = batch([0x11; 32], [0x22; 32]);
        push(&mut candidate, OperationKind::Transfer);
        validate_batch_semantics(&candidate, ProofSemantics::StateTransition)
            .expect("transfer rows have a canonical root-bound witness relation");
    }

    #[test]
    fn generic_state_profile_rejects_axt_bound_batches() {
        let mut candidate = batch([0x11; 32], [0x22; 32]);
        push(&mut candidate, OperationKind::MetaSet);
        candidate.metadata.insert(
            AXT_FASTPQ_BINDING_METADATA_KEY.into(),
            b"authenticated-outer-binding".to_vec(),
        );

        let error = validate_batch_semantics(&candidate, ProofSemantics::StateTransition)
            .expect_err("generic verification must not infer AXT semantics from metadata");
        assert!(matches!(
            error,
            Error::InvalidProofSemantics {
                profile: "state_transition",
                ..
            }
        ));
    }

    #[test]
    fn axt_transfer_profile_rejects_appended_metadata_row() {
        let mut candidate = batch([0x11; 32], [0x22; 32]);
        push(&mut candidate, OperationKind::Transfer);
        push(&mut candidate, OperationKind::MetaSet);
        assert!(matches!(
            validate_batch_semantics(&candidate, ProofSemantics::AxtTransferClaim),
            Err(Error::InvalidProofSemantics {
                profile: "axt_transfer_claim",
                ..
            })
        ));
    }

    #[test]
    fn axt_opaque_profile_accepts_only_metadata_carriers() {
        let mut legitimate = batch([0x11; 32], [0x22; 32]);
        push(&mut legitimate, OperationKind::MetaSet);
        validate_batch_semantics(&legitimate, ProofSemantics::AxtOpaqueEffect)
            .expect("opaque AXT metadata carrier");

        let mut transfer_attack = batch([0x11; 32], [0x22; 32]);
        push(&mut transfer_attack, OperationKind::Transfer);
        assert!(matches!(
            validate_batch_semantics(&transfer_attack, ProofSemantics::AxtOpaqueEffect),
            Err(Error::InvalidProofSemantics {
                profile: "axt_opaque_effect",
                ..
            })
        ));
    }
}
