//! Regression coverage for the transaction queue expiration and drain paths.
//!
//! These tests focus on making sure the queue rejects expired payloads and that the
//! ready/pending drain helpers stay panic-free under concurrent pressure.

use std::{borrow::Cow, num::NonZeroUsize, sync::Arc, time::Duration};

use iroha_config::parameters::actual::Queue as QueueConfig;
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    queue::{Error as QueueError, Queue, TransactionGuard},
    state::{State, StateView, World},
    tx::AcceptedTransaction,
};
use iroha_crypto::KeyPair;
use iroha_data_model::{domain::Domain, prelude::*};
use iroha_primitives::time::TimeSource;
use nonzero_ext::nonzero;
use tokio::time::sleep;

/// Helper trait exposing queue drain helpers for the integration tests.
///
/// The production queue exposes `get_transactions_for_block` and
/// `all_transactions`; the trait simply wraps them to allow expressive calls that
/// mirror the original `drain_ready`/`drain_pending` helper terminology.
trait QueueDrainExt {
    fn drain_ready(&self, view: &StateView<'_>, limit: NonZeroUsize) -> Vec<TransactionGuard>;
    fn drain_pending(&self, view: &StateView<'_>) -> Vec<AcceptedTransaction<'static>>;
}

impl QueueDrainExt for Arc<Queue> {
    fn drain_ready(&self, view: &StateView<'_>, limit: NonZeroUsize) -> Vec<TransactionGuard> {
        let mut drained = Vec::with_capacity(limit.get());
        self.get_transactions_for_block(view, limit, &mut drained);
        drained
    }

    fn drain_pending(&self, view: &StateView<'_>) -> Vec<AcceptedTransaction<'static>> {
        self.all_transactions(view).collect()
    }
}

fn build_state() -> (Arc<State>, ChainId, AccountId, KeyPair) {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();

    let key_pair = KeyPair::random();
    let (public_key, _) = key_pair.clone().into_parts();
    let domain_id: DomainId = "queue-regressions".parse().expect("static domain id");
    let account_id = AccountId::of(domain_id.clone(), public_key);
    let domain = Domain::new(domain_id).build(&account_id);
    let account = Account::new(account_id.clone()).build(&account_id);
    let world = World::with([domain], [account], std::iter::empty());

    let state = Arc::new(State::new_for_testing(world, kura, query_handle));
    let chain_id = ChainId::from("queue-regressions-chain");

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
    ttl: Option<Duration>,
    creation_time: Duration,
) -> AcceptedTransaction<'static> {
    let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone())
        .with_instructions([Log::new(Level::INFO, format!("noop-{nonce}"))]);
    if let Some(ttl) = ttl {
        builder.set_ttl(ttl);
    }
    builder.set_creation_time(creation_time);
    let tx = builder.sign(key_pair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(tx))
}

#[test]
fn queue_rejects_explicitly_expired_transactions() {
    // Coverage: Queue::is_expired TTL override path (`queue.rs`).
    let (state, chain_id, authority, key_pair) = build_state();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Queue::from_config(queue_config(8, Duration::from_secs(60)), events_sender);

    let expired = make_transaction(
        &chain_id,
        &authority,
        &key_pair,
        0,
        Some(Duration::from_secs(1)),
        Duration::from_secs(0),
    );

    assert!(queue.is_expired(&expired), "transaction should be expired");

    let view = state.view();
    let result = queue.push(expired, view);

    match result {
        Err(failure) => assert!(matches!(failure.err, QueueError::Expired)),
        Ok(()) => panic!("expired transaction unexpectedly enqueued"),
    }

    assert_eq!(queue.queued_len(), 0, "expired transaction must not linger");
}

#[test]
fn queue_rejects_transactions_expiring_by_config_ttl() {
    // Coverage: Queue::is_expired fallback to config TTL (`queue.rs`).
    let (state, chain_id, authority, key_pair) = build_state();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(4);
    let queue = Queue::from_config(queue_config(4, Duration::from_millis(10)), events_sender);

    let expired = make_transaction(
        &chain_id,
        &authority,
        &key_pair,
        1,
        None,
        Duration::from_secs(0),
    );

    assert!(
        queue.is_expired(&expired),
        "config TTL should expire transaction"
    );

    let view = state.view();
    let result = queue.push(expired, view);

    match result {
        Err(failure) => assert!(matches!(failure.err, QueueError::Expired)),
        Ok(()) => panic!("expired transaction unexpectedly enqueued"),
    }

    assert_eq!(queue.queued_len(), 0, "expired transaction must not linger");
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_ready_and_pending_drains_stay_consistent() {
    // Coverage: concurrent access to the queue drain loops (ready vs. pending).
    let (state, chain_id, authority, key_pair) = build_state();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(16);
    let queue = Arc::new(Queue::from_config(
        queue_config(64, Duration::from_secs(60)),
        events_sender,
    ));

    let now = TimeSource::new_system().get_unix_time();

    // Pre-seed a few transactions to exercise both drain paths immediately.
    for nonce in 0..8 {
        let tx = make_transaction(
            &chain_id,
            &authority,
            &key_pair,
            nonce,
            Some(Duration::from_secs(120)),
            now,
        );
        assert!(
            !queue.is_expired(&tx),
            "seed transactions should remain valid during stress"
        );
        queue
            .push(tx, state.view())
            .expect("queue accepts seed transaction");
    }

    let queue_for_ready = Arc::clone(&queue);
    let state_for_ready = Arc::clone(&state);
    let ready_task = async move {
        for _ in 0..32 {
            let view = state_for_ready.view();
            let mut guards = queue_for_ready.drain_ready(&view, nonzero!(4_usize));
            drop(view);
            // Drop guards outside of the view to unblock subsequent drains.
            guards.clear();
            sleep(Duration::from_millis(2)).await;
        }
    };

    let queue_for_pending = Arc::clone(&queue);
    let state_for_pending = Arc::clone(&state);
    let pending_task = async move {
        for _ in 0..32 {
            let view = state_for_pending.view();
            let _pending = queue_for_pending.drain_pending(&view);
            drop(view);
            sleep(Duration::from_millis(1)).await;
        }
    };

    let queue_for_push = Arc::clone(&queue);
    let state_for_push = Arc::clone(&state);
    let chain_id_for_push = chain_id.clone();
    let authority_for_push = authority.clone();
    let key_pair_for_push = key_pair.clone();
    let push_task = async move {
        for nonce in 8..64 {
            let tx = make_transaction(
                &chain_id_for_push,
                &authority_for_push,
                &key_pair_for_push,
                nonce,
                Some(Duration::from_secs(120)),
                TimeSource::new_system().get_unix_time(),
            );
            if queue_for_push.is_expired(&tx) {
                continue;
            }
            queue_for_push
                .push(tx, state_for_push.view())
                .expect("queue accepts injected transaction during stress");
            sleep(Duration::from_millis(1)).await;
        }
    };

    tokio::join!(ready_task, pending_task, push_task);

    // The queue should stay internally consistent despite the concurrent drains.
    let remaining = queue.queued_len();
    if remaining > 0 {
        // Verify that any residual transactions remain accessible via the ready drain.
        let view = state.view();
        let limit = NonZeroUsize::new(remaining).expect("non-zero queue length");
        let guards = queue.drain_ready(&view, limit);
        drop(view);
        assert_eq!(guards.len(), remaining);
    }
}
