//! Shared exact signed native execution on a certified test chain ([`CertifiedTestChain`]): a
//! signed genesis with a fixed four-validator committee, blocks built, executed and applied
//! through the node's block path, each certified by a real BLS `CommitQC`.
//!
//! Test-only World setup (accounts, permissions, owners) is part of the initial World, not of
//! genesis or finalized history. Transactions go through ordinary acceptance and execution;
//! results are never forged.
use crate::{
    state::{State, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::AccountId,
    isi::InstructionBox,
    transaction::{FeePaymentIntent, SignedTransaction, TransactionBuilder},
};
use std::{sync::Arc, time::Duration};

/// Creation time of the fixture chains' first genesis transaction (milliseconds).
pub(crate) const GENESIS_TIME_MS: u64 = 1;

pub(crate) fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap()
}

/// A certified chain over `world` (genesis at [`GENESIS_TIME_MS`]).
///
/// # Panics
/// Genesis does not apply.
pub(crate) fn chain(world: World) -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(world, GENESIS_TIME_MS))
        .map_err(|failure| failure.error)
        .expect("fixture chain starts")
}

/// `instruction` signed by `key(seed)` on `state`'s network, created one millisecond before
/// `now` so that a block committed at `now` carries it.
pub(crate) fn sign(
    state: &Arc<State>,
    instruction: InstructionBox,
    seed: u8,
    now: u64,
) -> SignedTransaction {
    let key = key(seed);
    let mut builder = TransactionBuilder::new(
        *state.network_id_ref(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(now.saturating_sub(1)));
    builder
        .with_instructions([instruction])
        .try_sign(key.private_key())
        .unwrap()
}

/// Commit `transactions` in one certified block at time `now`; whether each executed.
pub(crate) fn commit(
    chain: &mut CertifiedTestChain,
    now: u64,
    transactions: Vec<SignedTransaction>,
) -> Vec<bool> {
    chain.commit_at(now, transactions)
}

/// [`commit`] with a local `CommitQC` of two of four signers: the block is committed and
/// applied, but its certificate does not verify (signer finality is unavailable for it).
#[cfg(test)]
pub(crate) fn commit_uncertified(
    chain: &mut CertifiedTestChain,
    now: u64,
    transactions: Vec<SignedTransaction>,
) -> Vec<bool> {
    chain.commit_with(
        Some(now),
        transactions,
        crate::sumeragi::test_chain::Signers::BelowQuorum,
    )
}

/// Corrupt the State's entrypoint index: the committed `transactions` are indexed at genesis
/// instead of their block (a State whose membership does not match its blocks).
#[cfg(test)]
pub(crate) fn misplace_membership(chain: &CertifiedTestChain, transactions: &[SignedTransaction]) {
    for transaction in transactions {
        chain
            .state()
            .transactions
            .overwrite_committed_entrypoint_membership_for_tests(
                transaction.hash_as_entrypoint(),
                core::num::NonZeroUsize::MIN,
            );
    }
}
