//! Delivery-state regressions only; these payloads do not model proof validity.
use super::*;
use crate::kagemusha_wallet_ffi::{WalletResult, exports};

fn same_reply(a: &Response, b: &Response) {
    assert_eq!(
        (a.kind, a.sequence, a.detail, a.backlog),
        (b.kind, b.sequence, b.detail, b.backlog)
    );
    assert_eq!(a.bytes, b.bytes);
}

#[test]
fn exact_install_originals_recover_after_reply_allocation_failure() {
    let parameters = [7; 32];
    let signature = [9; 64];
    let installed = InstalledOriginals::authenticated(parameters, b"ab", b"c", &signature);
    let mut out = WalletResult::default();
    let status = unsafe {
        exports::output_with(
            &mut out,
            || installed.retry(b"ab", b"c", &signature),
            |_, _| Err(()),
        )
    };
    assert_eq!(status, RESOURCE);
    assert!(out.bytes.is_null());
    let recovered = installed.retry(b"ab", b"c", &signature).unwrap();
    assert_eq!(recovered.kind, 81);
    assert_eq!(recovered.bytes, parameters);
    // Identical concatenation with different original boundaries must not match.
    assert_eq!(
        installed.retry(b"a", b"bc", &signature).unwrap_err().status,
        CONFLICT
    );
    assert_eq!(
        installed.retry(b"ax", b"c", &signature).unwrap_err().status,
        CONFLICT
    );
    let mut changed = signature;
    changed[63] ^= 1;
    assert_eq!(
        installed.retry(b"ab", b"c", &changed).unwrap_err().status,
        CONFLICT
    );
    assert_ne!(
        installed.fingerprint,
        InstalledOriginals::authenticated([8; 32], b"ab", b"c", &signature).fingerprint
    );
    // Refused retries leave the admitted originals recoverable.
    same_reply(
        &recovered,
        &installed.retry(b"ab", b"c", &signature).unwrap(),
    );
}

#[test]
fn every_terminal_reply_is_replayed_without_reentering_advance() {
    for (kind, detail) in [(1, 0), (3, 0), (3, 1), (3, 2), (4, 0), (5, 0)] {
        let mut cache = ReviewCache::default();
        let token = cache.insert(23).unwrap();
        let expected = Response {
            kind,
            sequence: (1_u128 << 96) + 17,
            detail,
            bytes: if kind == 1 {
                vec![1, 0, 255, 7]
            } else {
                Vec::new()
            },
            backlog: None,
        };
        let mut out = WalletResult::default();
        // A real C delivery failure occurs after retaining a Released reply.
        if kind == 1 {
            assert_eq!(
                unsafe {
                    exports::output_with(
                        &mut out,
                        || {
                            cache.execute(token, |payload| {
                                assert_eq!(*payload, 23);
                                Ok(copy_reply(&expected))
                            })
                        },
                        |_, _| Err(()),
                    )
                },
                RESOURCE
            );
        } else {
            // Simulate the caller losing an otherwise completed result.
            drop(cache.execute(token, |_| Ok(copy_reply(&expected))).unwrap());
        }
        let replay = cache
            .execute(token, |_| panic!("terminal result re-entered Advance"))
            .unwrap();
        same_reply(&replay, &expected);
        assert!(cache.find(|payload| *payload == 23).is_none());
        let fresh = cache.insert(23).unwrap();
        assert_ne!(fresh, token);
        assert_eq!(cache.find(|payload| *payload == 23).unwrap().0, fresh);
        same_reply(
            &cache
                .execute(token, |_| panic!("old token lost its reply"))
                .unwrap(),
            &expected,
        );
    }
}

#[test]
fn pending_and_uncertain_execution_remain_retryable() {
    let mut cache = ReviewCache::default();
    let token = cache.insert(()).unwrap();
    assert_eq!(
        cache
            .execute(token, |_| Err(Failure::code(UNCERTAIN)))
            .unwrap_err()
            .status,
        UNCERTAIN
    );
    assert_eq!(
        cache
            .execute(token, |_| Ok(response(2, Vec::new())))
            .unwrap()
            .kind,
        2
    );
    assert!(cache.entries[&token].terminal.is_none());
    assert_eq!(
        cache
            .execute(token, |_| Ok(response(1, vec![91])))
            .unwrap()
            .bytes,
        [91]
    );
    assert_eq!(
        cache
            .execute(token, |_| panic!("released execution repeated"))
            .unwrap()
            .bytes,
        [91]
    );
}

#[test]
fn terminal_results_release_capacity_without_evicting_live_reviews() {
    let mut cache = ReviewCache::default();
    let ids = (0..32)
        .map(|i| cache.insert(i).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(cache.insert(32).unwrap_err().status, RESOURCE);
    // Every terminal status, including NotPerformed, must permit the next preparation.
    for kind in [3, 4, 5, 1] {
        let token = *cache.entries.keys().next_back().unwrap();
        cache
            .execute(token, |_| {
                Ok(response(kind, if kind == 1 { vec![8] } else { Vec::new() }))
            })
            .unwrap();
        let replacement = cache.insert(99).unwrap();
        assert!(replacement > token);
        assert!(!cache.entries.contains_key(&token));
        assert_eq!(cache.entries.len(), 32);
        assert!(cache.entries.contains_key(&ids[0]));
    }
    let pending = ids[0];
    cache
        .execute(pending, |_| Ok(response(2, Vec::new())))
        .unwrap();
    assert_eq!(cache.insert(100).unwrap_err().status, RESOURCE);
    cache.discard(pending);
    cache.insert(100).unwrap();
    assert!(cache.find(|v| *v == 100).is_some());
}
