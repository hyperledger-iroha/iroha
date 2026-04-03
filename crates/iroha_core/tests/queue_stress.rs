//! Stress tests for the transaction queue to guard against Arc drain panics.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{borrow::Cow, num::NonZeroUsize, sync::Arc, thread, time::Duration};

use iroha_config::parameters::actual::Queue as QueueConfig;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{Queue, TransactionGuard},
    state::{State, World},
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{domain::Domain, prelude::*};
use nonzero_ext::nonzero;

fn build_state() -> (State, ChainId, AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let key_pair = KeyPair::random();
    let (public_key, _) = key_pair.clone().into_parts();
    let domain_id: DomainId =
        DomainId::try_new("queue-stress", "universal").expect("static domain id");
    let account_id = AccountId::of(public_key);
    let domain = Domain::new(domain_id.clone()).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty());

    let mut state = State::new_for_testing(world, kura, query_handle);
    let chain_id = ChainId::from("queue-stress-chain");
    state.chain_id = chain_id.clone();

    (state, chain_id, account_id, key_pair)
}

fn queue_config(capacity: usize, ttl: Duration) -> QueueConfig {
    QueueConfig {
        capacity: NonZeroUsize::new(capacity).expect("non-zero capacity"),
        capacity_per_user: NonZeroUsize::new(capacity).expect("non-zero per-user capacity"),
        transaction_time_to_live: ttl,
        ..QueueConfig::default()
    }
}

fn make_transaction(
    chain_id: &ChainId,
    authority: &AccountId,
    key_pair: &KeyPair,
    nonce: usize,
    ttl: Duration,
) -> AcceptedTransaction<'static> {
    let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, format!("noop-{nonce}"))]);
    builder.set_ttl(ttl);
    let tx = builder.sign(key_pair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(tx))
}

#[test]
fn expired_transactions_drain_without_panic() {
    let (state, chain_id, authority, key_pair) = build_state();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(Queue::from_config(
        queue_config(4, Duration::from_secs(1)),
        events_sender,
    ));

    // Transactions expire quickly, triggering queue drain paths that previously panicked when
    // draining Arc-backed transactions with outstanding clones.
    let iterations = 32;
    for i in 0..iterations {
        let tx = make_transaction(
            &chain_id,
            &authority,
            &key_pair,
            i,
            Duration::from_millis(20),
        );
        queue
            .push(tx, state.view())
            .expect("queue accepts new transaction");

        thread::sleep(Duration::from_millis(30));

        let mut guards: Vec<TransactionGuard> = Vec::new();
        let view = state.view();
        queue.get_transactions_for_block(&view, nonzero!(1_usize), &mut guards);
        drop(view);

        assert!(guards.is_empty(), "expired tx should not remain available");
        assert_eq!(queue.queued_len(), 0, "queue drained expired transaction");
    }
}
