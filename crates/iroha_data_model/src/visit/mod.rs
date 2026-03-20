//! Visitor that visits every node in the Iroha syntax tree.

use iroha_primitives::numeric::Numeric;

use crate::{
    isi::{
        ActivateIdentifierPolicy, ClaimIdentifier, Log, RegisterIdentifierPolicy,
        RegisterPeerWithPop, RevokeIdentifier,
        soracloud::{
            AcknowledgeSoracloudAgentMessage, AdvanceSoracloudRollout,
            AllowSoracloudAgentAutonomyArtifact, ApproveSoracloudAgentWalletSpend,
            CheckpointSoracloudTrainingJob, DeploySoracloudAgentApartment, DeploySoracloudService,
            EnqueueSoracloudAgentMessage, MutateSoracloudState, PromoteSoracloudModelWeight,
            RecordSoracloudDecryptionRequest, RecordSoracloudMailboxMessage,
            RecordSoracloudRuntimeReceipt, RegisterSoracloudModelArtifact,
            RegisterSoracloudModelWeight, RenewSoracloudAgentLease,
            RequestSoracloudAgentWalletSpend, RestartSoracloudAgentApartment,
            RetrySoracloudTrainingJob, RevokeSoracloudAgentPolicy, RollbackSoracloudModelWeight,
            RollbackSoracloudService, RunSoracloudAgentAutonomy, RunSoracloudFheJob,
            SetSoracloudRuntimeState, StartSoracloudTrainingJob, UpgradeSoracloudService,
        },
        staking::{
            ActivatePublicLaneValidator, ExitPublicLaneValidator, RegisterPublicLaneValidator,
        },
    },
    prelude::*,
    query::{AnyQueryBox, QueryWithParams, SingularQueryBox},
};

mod visit_instruction;
mod visit_query;

pub use visit_instruction::*;
pub use visit_query::*;

/// Helper macro to forward visitor trait methods to standalone functions.
macro_rules! visit_all {
    ( $( $visitor:ident($operation:ty) ),+ $(,)? ) => { $(
        #[doc = concat!("Visit ", stringify!($operation), ".")]
        fn $visitor(&mut self, operation: $operation) { $visitor(self, operation); }
    )+ };
}

/// Trait to validate Iroha entities.
/// Default implementation of non-leaf visitors runs `visit_` functions for leafs.
/// Default implementation for leaf visitors is blank.
///
/// This trait is based on the visitor pattern.
///
/// Each method delegates to the free function with the same name. This
/// repetitive pattern is kept explicit for clarity.
pub trait Visit {
    /// Visit a transaction, dispatching to either bytecode or instruction walkers.
    fn visit_transaction(&mut self, operation: &SignedTransaction) {
        visit_transaction(self, operation);
    }
    /// Visit a single instruction.
    fn visit_instruction(&mut self, operation: &InstructionBox) {
        visit_instruction(self, operation);
    }
    /// Visit IVM bytecode payload.
    fn visit_ivm(&mut self, operation: &IvmBytecode) {
        visit_ivm(self, operation);
    }
    /// Visit any query wrapper.
    fn visit_query(&mut self, operation: &AnyQueryBox) {
        visit_query(self, operation);
    }
    /// Visit a singular query (non-iterable).
    fn visit_singular_query(&mut self, operation: &SingularQueryBox) {
        visit_singular_query(self, operation);
    }
    /// Visit an iterable query with parameters.
    fn visit_iter_query(&mut self, operation: &QueryWithParams) {
        visit_iter_query(self, operation);
    }

    /// Visit a burn instruction.
    fn visit_burn(&mut self, operation: &BurnBox) {
        visit_burn(self, operation);
    }
    /// Visit a grant instruction.
    fn visit_grant(&mut self, operation: &GrantBox) {
        visit_grant(self, operation);
    }
    /// Visit a mint instruction.
    fn visit_mint(&mut self, operation: &MintBox) {
        visit_mint(self, operation);
    }
    /// Visit a register instruction.
    fn visit_register(&mut self, operation: &RegisterBox) {
        visit_register(self, operation);
    }
    /// Visit a remove-key-value instruction.
    fn visit_remove_key_value(&mut self, operation: &RemoveKeyValueBox) {
        visit_remove_key_value(self, operation);
    }
    /// Visit a revoke instruction.
    fn visit_revoke(&mut self, operation: &RevokeBox) {
        visit_revoke(self, operation);
    }
    /// Visit a set-key-value instruction.
    fn visit_set_key_value(&mut self, operation: &SetKeyValueBox) {
        visit_set_key_value(self, operation);
    }
    /// Visit a transfer instruction.
    fn visit_transfer(&mut self, operation: &TransferBox) {
        visit_transfer(self, operation);
    }
    /// Visit an unregister instruction.
    fn visit_unregister(&mut self, operation: &UnregisterBox) {
        visit_unregister(self, operation);
    }

    crate::query_visitors!(visit_all);
    crate::instruction_visitors!(visit_all);
}

/// Walk a transaction and delegate to the provided visitor.
pub fn visit_transaction<V: Visit + ?Sized>(visitor: &mut V, transaction: &SignedTransaction) {
    match transaction.instructions() {
        Executable::Ivm(bytecode) => visitor.visit_ivm(bytecode),
        Executable::IvmProved(proved) => {
            // For proved execution the semantic state transition is expressed by the overlay, but
            // we also expose the original bytecode for visitors that inspect Kotodama programs.
            visitor.visit_ivm(&proved.bytecode);
            for isi in &proved.overlay {
                visitor.visit_instruction(isi);
            }
        }
        Executable::Instructions(instructions) => {
            for isi in instructions {
                visitor.visit_instruction(isi);
            }
        }
    }
}

/// Visit IVM bytecode. Override this method to inspect raw Kotodama programs.
pub fn visit_ivm<V: Visit + ?Sized>(_visitor: &mut V, _bytecode: &IvmBytecode) {}
