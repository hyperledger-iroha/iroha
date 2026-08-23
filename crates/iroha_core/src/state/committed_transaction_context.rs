//! Exact signed-transaction context used while replaying committed blocks.

use iroha_crypto::Hash;
use iroha_data_model::transaction::TransactionEntrypoint;

use super::StateTransaction;
use crate::{
    smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1,
    tx::AcceptedTransaction,
};

/// Bind immutable replay identities without upgrading a sealed reveal to `External` provenance.
pub(crate) fn seed_committed_transaction_context(
    state_transaction: &mut StateTransaction<'_, '_>,
    entrypoint: &TransactionEntrypoint,
    entrypoint_index: usize,
) {
    state_transaction.kagemusha_taira_canary_external_entrypoint = false;
    state_transaction.kagemusha_taira_canary_wire_identity = None;
    let transaction = match entrypoint {
        TransactionEntrypoint::External(transaction) => {
            state_transaction.kagemusha_taira_canary_external_entrypoint = true;
            transaction
        }
        TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
        TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => return,
    };
    state_transaction.tx_call_hash = Some(Hash::from(entrypoint.execution_call_hash()));
    state_transaction.current_tx_hash =
        Some(AcceptedTransaction::prepare_signed_metadata(transaction).signed_hash);
    if state_transaction.kagemusha_taira_canary_external_entrypoint {
        state_transaction.kagemusha_taira_canary_wire_identity =
            signed_kagemusha_taira_canary_wire_identity_v1(transaction)
                .expect("committed external canary wire must encode");
    }
    state_transaction.current_entrypoint_index =
        Some(u64::try_from(entrypoint_index).unwrap_or(u64::MAX));
}
