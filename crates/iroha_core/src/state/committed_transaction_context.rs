//! Exact signed-transaction context used while replaying committed blocks.

use iroha_crypto::Hash;
use iroha_data_model::transaction::SignedTransaction;

use super::StateTransaction;
use crate::{
    smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1,
    tx::AcceptedTransaction,
};

/// Bind the immutable transaction identities that committed replay may consume.
pub(crate) fn seed_committed_transaction_context(
    state_transaction: &mut StateTransaction<'_, '_>,
    transaction: &SignedTransaction,
    entrypoint_index: usize,
) {
    state_transaction.tx_call_hash = Some(Hash::from(transaction.hash_as_entrypoint()));
    state_transaction.current_tx_hash =
        Some(AcceptedTransaction::prepare_signed_metadata(transaction).signed_hash);
    state_transaction.kagemusha_taira_canary_wire_identity =
        signed_kagemusha_taira_canary_wire_identity_v1(transaction)
            .expect("committed canary wire must encode");
    state_transaction.current_entrypoint_index =
        Some(u64::try_from(entrypoint_index).unwrap_or(u64::MAX));
}
