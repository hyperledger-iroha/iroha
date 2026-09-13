//! Deterministic controls of the shipping central owner; no node is started.
use super::*;
use crate::RelayRole;
use futures::FutureExt;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::peer::Peer;

fn peer(seed: u8) -> Peer {
    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap();
    Peer::new("127.0.0.1:1337".parse().unwrap(), key.public_key().clone())
}
fn candidate(
    peer: &Peer,
    id: u64,
    rank: u8,
) -> (
    Authenticated,
    oneshot::Receiver<ReaderPermit>,
    watch::Receiver<bool>,
) {
    let (cancel, receiver) = watch::channel(false);
    let (reply, permission) = oneshot::channel();
    (
        Authenticated {
            peer: peer.clone(),
            connection_id: id,
            session: [rank; 32],
            relay_role: RelayRole::Disabled,
            cancel,
            reply,
        },
        permission,
        receiver,
    )
}
fn reap(owner: &mut Arbitration) {
    while let Some(id) = owner.ready_release() {
        owner.released(id);
    }
}

#[test]
fn crossed_arrival_orders_choose_same_session_only_after_physical_release() {
    let a = peer(61);
    let b = peer(62);
    for left_order in [[1, 2], [2, 1]] {
        for right_order in [[1, 2], [2, 1]] {
            let mut left = Arbitration::default();
            let mut right = Arbitration::default();
            let mut permissions = Vec::new();
            for (owner, peer, order) in [(&mut left, &b, left_order), (&mut right, &a, right_order)]
            {
                let mut received = Vec::new();
                for rank in order {
                    let (candidate, mut permission, cancelled) =
                        candidate(peer, u64::from(rank), rank);
                    owner.admit(candidate);
                    received.push((rank, permission.try_recv().ok(), permission, cancelled));
                }
                // Retain the old physical owner after observing cancellation.
                for (rank, permit, _, cancelled) in &received {
                    if *rank == 1 && permit.is_some() {
                        assert!(*cancelled.borrow());
                    }
                }
                if order == [1, 2] {
                    assert!(received[1].1.is_none(), "no concurrent reader permission");
                    assert!(
                        owner.ready_release().is_none(),
                        "watch cancellation is not physical release"
                    );
                }
                for (rank, permit, _, _) in &mut received {
                    if *rank == 1 {
                        drop(permit.take());
                    }
                }
                reap(owner);
                for (rank, permit, mut waiting, cancelled) in received {
                    if rank == 2 {
                        permissions.push(
                            permit
                                .or_else(|| waiting.try_recv().ok())
                                .expect("same winning session"),
                        );
                        assert!(!*cancelled.borrow());
                        assert!(owner.claim_connected(2, peer.id(), u64::from_be_bytes([2; 8])));
                    }
                }
            }
            assert_eq!(permissions.len(), 2);
            drop(permissions);
            reap(&mut left);
            reap(&mut right);
            assert!(left.entries.is_empty() && right.entries.is_empty());
        }
    }
}

#[test]
fn full_hash_tail_orders_equal_compact_prefixes_and_exact_replay_fences_both() {
    let p = peer(63);
    let mut owner = Arbitration::default();
    let (mut first, mut r1, c1) = candidate(&p, 1, 0);
    first.session[31] = 1;
    assert!(owner.admit(first).accepted);
    let held = r1.try_recv().unwrap();
    let (mut second, mut r2, c2) = candidate(&p, 2, 0);
    second.session[31] = 2;
    assert!(owner.admit(second).accepted);
    assert!(*c1.borrow());
    assert!(r2.try_recv().is_err());
    drop(held);
    reap(&mut owner);
    let held = r2.try_recv().unwrap();
    assert!(owner.claim_connected(2, p.id(), 0));
    let (mut duplicate, mut r3, c3) = candidate(&p, 3, 0);
    duplicate.session[31] = 2;
    assert!(!owner.admit(duplicate).accepted);
    assert!(*c2.borrow() && *c3.borrow());
    assert!(r3.try_recv().is_err());
    assert!(!owner.claim_connected(2, p.id(), 0));
    drop(held);
    reap(&mut owner);
    assert!(owner.entries.is_empty());
}

#[test]
fn closed_or_cancelled_candidate_cannot_preempt_live_reader() {
    let p = peer(64);
    let mut owner = Arbitration::default();
    let (first, mut r1, c1) = candidate(&p, 1, 1);
    owner.admit(first);
    let _held = r1.try_recv().unwrap();
    let (second, r2, _) = candidate(&p, 2, 2);
    drop(r2);
    assert!(!owner.admit(second).accepted);
    assert!(!*c1.borrow());
    let (third, _r3, _) = candidate(&p, 3, 3);
    third.cancel.send_replace(true);
    assert!(!owner.admit(third).accepted);
    assert!(!*c1.borrow());
    assert_eq!(owner.entries.len(), 1);
}

#[test]
fn stale_and_duplicate_connected_or_termination_do_not_mutate_successor() {
    let p = peer(65);
    let foreign = peer(66);
    let mut owner = Arbitration::default();
    let (first, mut r1, _) = candidate(&p, 1, 1);
    owner.admit(first);
    let held = r1.try_recv().unwrap();
    let (second, mut r2, c2) = candidate(&p, 2, 2);
    owner.admit(second);
    assert!(!owner.claim_connected(1, p.id(), u64::from_be_bytes([1; 8])));
    assert!(!owner.claim_connected(2, p.id(), u64::from_be_bytes([2; 8])));
    drop(held);
    reap(&mut owner);
    let _held = r2.try_recv().unwrap();
    owner.cancel(1);
    owner.released(1);
    owner.cancel(999);
    assert!(!*c2.borrow());
    assert!(!owner.claim_connected(2, foreign.id(), u64::from_be_bytes([2; 8])));
    assert!(!owner.claim_connected(2, p.id(), 9));
    assert!(owner.claim_connected(2, p.id(), u64::from_be_bytes([2; 8])));
    assert!(!owner.claim_connected(2, p.id(), u64::from_be_bytes([2; 8])));
}

#[tokio::test]
async fn release_notification_wakes_owner_without_service_or_delivery_completion() {
    let p = peer(67);
    let mut owner = Arbitration::default();
    let (first, mut r1, _) = candidate(&p, 1, 1);
    owner.admit(first);
    let held = r1.try_recv().unwrap();
    let (second, mut r2, _) = candidate(&p, 2, 2);
    owner.admit(second);
    let mut waiting = Box::pin(std::future::poll_fn(|cx| owner.poll_released(cx)));
    assert!(waiting.as_mut().now_or_never().is_none());
    drop(held);
    assert_eq!(waiting.await, 1);
    owner.released(1);
    assert!(r2.try_recv().is_ok());
}

#[test]
fn unissued_cancel_and_owner_drop_retire_waiters_without_releasing_active_reader() {
    let p = peer(68);
    let mut owner = Arbitration::default();
    let (first, mut r1, c1) = candidate(&p, 1, 1);
    owner.admit(first);
    let held = r1.try_recv().unwrap();
    let (second, mut r2, c2) = candidate(&p, 2, 2);
    owner.admit(second);
    owner.cancel(2);
    assert!(r2.try_recv().is_err() && *c2.borrow());
    assert_eq!(owner.entries.len(), 1);
    assert!(owner.ready_release().is_none());
    let (third, mut r3, c3) = candidate(&p, 3, 3);
    owner.admit(third);
    drop(owner);
    assert!(*c1.borrow() && *c3.borrow());
    assert!(r3.try_recv().is_err());
    drop(held);
}

#[test]
fn physically_released_reader_cannot_claim_buffered_connected_before_owner_poll() {
    let p = peer(69);
    let mut owner = Arbitration::default();
    let (first, mut permission, _) = candidate(&p, 1, 1);
    owner.admit(first);
    let held = permission.try_recv().unwrap();
    drop(held);
    assert_eq!(
        owner.entries.len(),
        1,
        "release notice deliberately remains undrained"
    );
    assert!(!owner.claim_connected(1, p.id(), u64::from_be_bytes([1; 8])));
    reap(&mut owner);
    assert!(owner.entries.is_empty());
}

#[test]
fn third_higher_session_supersedes_ungranted_replacement_without_early_credit() {
    let p = peer(70);
    let mut owner = Arbitration::default();
    let (first, mut r1, c1) = candidate(&p, 1, 1);
    owner.admit(first);
    let held = r1.try_recv().unwrap();
    let (second, mut r2, c2) = candidate(&p, 2, 2);
    owner.admit(second);
    let (third, mut r3, c3) = candidate(&p, 3, 3);
    owner.admit(third);
    assert!(*c1.borrow() && *c2.borrow() && !*c3.borrow());
    assert!(matches!(
        r2.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    assert!(matches!(
        r3.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    assert_eq!(
        owner.entries.len(),
        2,
        "one issued predecessor and one waiting successor"
    );
    owner.cancel(2);
    assert!(owner.ready_release().is_none());
    drop(held);
    reap(&mut owner);
    let _held = r3.try_recv().unwrap();
    assert!(owner.claim_connected(3, p.id(), u64::from_be_bytes([3; 8])));
}

#[test]
fn completed_higher_session_does_not_reject_fresh_candidate_before_global_release_drain() {
    let p = peer(71);
    let mut owner = Arbitration::default();
    let (first, mut r1, _) = candidate(&p, 1, 3);
    owner.admit(first);
    drop(r1.try_recv().unwrap());
    let (second, mut r2, cancelled) = candidate(&p, 2, 1);
    assert!(owner.admit(second).accepted);
    let _held = r2.try_recv().unwrap();
    assert!(!*cancelled.borrow());
    assert!(owner.claim_connected(2, p.id(), u64::from_be_bytes([1; 8])));
}

#[test]
fn dead_waiting_replacement_cannot_reject_lower_fresh_candidate_after_predecessor_release() {
    for already_released in [false, true] {
        let p = peer(72);
        let mut owner = Arbitration::default();
        let (a, mut ra, _) = candidate(&p, 1, 2);
        owner.admit(a);
        let mut held_a = Some(ra.try_recv().unwrap());
        let (b, rb, cancelled_b) = candidate(&p, 2, 3);
        owner.admit(b);
        drop(rb);
        if already_released {
            drop(held_a.take());
        }
        // Neither the predecessor release nor cancelled waiter has been polled.
        let (c, mut rc, cancelled_c) = candidate(&p, 3, 1);
        assert!(owner.admit(c).accepted);
        if !already_released {
            assert!(matches!(
                rc.try_recv(),
                Err(oneshot::error::TryRecvError::Empty)
            ));
            drop(held_a.take());
            reap(&mut owner);
        }
        let _held_c = rc
            .try_recv()
            .expect("dead selected B must not block fresh C");
        assert!(*cancelled_b.borrow() && !*cancelled_c.borrow());
        assert_eq!(owner.entries.len(), 1);
        assert!(owner.claim_connected(3, p.id(), u64::from_be_bytes([1; 8])));
    }
}
