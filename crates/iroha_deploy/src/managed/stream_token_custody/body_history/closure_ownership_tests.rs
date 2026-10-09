//! Consuming genuine closure ownership preserves wallet callbacks, custody and active charges.

use super::*;
use crate::managed::native_operation::attempts::UnsignedClosureVerification;
use std::cell::Cell;

fn complete(value: Result<UnsignedClosureVerification>) -> VerifiedUnsignedClosure {
    match value.unwrap_or_else(|error| panic!("genuine closure refused: {error}")) {
        UnsignedClosureVerification::Closed(value) => value,
        UnsignedClosureVerification::Pending(_) => panic!("genuine retired body remains pending"),
    }
}
fn same_closure(actual: &VerifiedUnsignedClosure, original: &VerifiedUnsignedClosure) {
    assert_eq!(actual.digest(), original.digest());
    assert_eq!(actual.purpose(), original.purpose());
    assert_eq!(actual.closed_semantic(), original.closed_semantic());
    assert_eq!(actual.successor_selection(), original.successor_selection());
    assert_eq!(
        actual.cumulative_reserved_count(),
        original.cumulative_reserved_count()
    );
    actual.retained_history().test_require_current(4).0.unwrap();
}
struct ChangedRecord {
    directory: Arc<PrivateDirectory>,
    name: &'static str,
    original: zeroize::Zeroizing<Vec<u8>>,
}
impl ChangedRecord {
    fn replace(directory: &Arc<PrivateDirectory>, name: &'static str) -> Self {
        let value = Self {
            directory: Arc::clone(directory),
            name,
            original: directory.read(name, journal::MAX_ORIGINAL_BYTES).unwrap(),
        };
        let mut changed = value.original.to_vec();
        changed[0] ^= 1;
        directory
            .write_atomic(name, &changed, PublishMode::Replace)
            .unwrap();
        value
    }
}
impl Drop for ChangedRecord {
    fn drop(&mut self) {
        self.directory
            .write_atomic(self.name, &self.original, PublishMode::Replace)
            .expect("restore only the original test-owned closure record");
    }
}
fn limits(allocation: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 64)
}

#[test]
fn owned_closed_history_moves_after_fresh_wallet_and_source_checks_with_active_recipe_parity() {
    let _guard = crate::managed::native_test_guard();
    let (fixture, history) = shared_snapshot_tests::history();
    let original = history.bodies[0].original.as_ref().unwrap();
    let oldest = history
        .current_history()
        .unwrap()
        .test_oldest_retained_history();
    let successor = history.successor(0).unwrap().unwrap();
    let account = fixture.owner.wallet().unwrap();
    let calls = Cell::new(0usize);
    let inspect = |attempt: &attempts::Attempt| {
        calls.set(calls.get() + 1);
        original
            .request(
                attempt.terms(),
                attempt.observation()?,
                fixture.options.deadline,
            )?
            .inspect_in_parent(&account, attempt.directory())
    };
    let borrowed = oldest.test_owned_closure_history().unwrap();
    let (baseline, reparses) =
        History::test_closure_parse_work(|| borrowed.verify_unsigned_closure(&successor, inspect));
    let baseline = baseline.unwrap().unwrap();
    assert_eq!(
        reparses, 1,
        "borrowed return still obtains its own parsed owner"
    );
    let original_calls = calls.replace(0);
    assert_eq!(
        original_calls, 2,
        "whole wallet pass and final exact tail inspection"
    );
    let owned = oldest.test_owned_closure_history().unwrap();
    let (moved, reparses) = History::test_closure_parse_work(|| {
        owned.into_unsigned_closure_with_pass(&successor, inspect, None)
    });
    assert_eq!(
        reparses, 0,
        "the actual parsed owner moves without another DTO reconstruction"
    );
    assert_eq!(calls.replace(0), original_calls);
    same_closure(&complete(moved), &baseline);

    // A changed source before entry is rejected before any wallet callback.
    for name in ["original.nrt", "closing.nrt", "closed.nrt", "dispatch.nrt"] {
        let owned = oldest.test_owned_closure_history().unwrap();
        let changed = ChangedRecord::replace(&history.bodies[0].directory, name);
        assert!(
            owned
                .into_unsigned_closure_with_pass(&successor, inspect, None)
                .is_err()
        );
        assert_eq!(calls.replace(0), 0);
        drop(changed);
    }
    // The last callback must not carry earlier source bytes, even when it returns an error.
    for name in ["original.nrt", "closing.nrt", "closed.nrt", "dispatch.nrt"] {
        for ordinary_error in [false, true] {
            let owned = oldest.test_owned_closure_history().unwrap();
            let mut changed = None;
            let result = owned.into_unsigned_closure_with_pass(
                &successor,
                |attempt| {
                    let value = inspect(attempt)?;
                    if calls.get() == original_calls {
                        changed = Some(ChangedRecord::replace(&history.bodies[0].directory, name));
                        if ordinary_error {
                            return Err(invalid("ordinary closure wallet refusal"));
                        }
                    }
                    Ok(value)
                },
                None,
            );
            let error = result.err().expect("last callback mutation must refuse");
            assert_eq!(calls.replace(0), original_calls);
            assert!(changed.is_some());
            assert_ne!(
                error.to_string(),
                invalid("ordinary closure wallet refusal").to_string()
            );
            drop(changed);
        }
    }
    let owned = oldest.test_owned_closure_history().unwrap();
    let unchanged_error = owned
        .into_unsigned_closure_with_pass(
            &successor,
            |attempt| {
                let value = inspect(attempt)?;
                if calls.get() == original_calls {
                    return Err(invalid("ordinary closure wallet refusal"));
                }
                Ok(value)
            },
            None,
        )
        .err()
        .expect("ordinary wallet refusal remains an error");
    assert_eq!(calls.replace(0), original_calls);
    assert_eq!(
        unchanged_error.to_string(),
        invalid("ordinary closure wallet refusal").to_string()
    );

    #[cfg(unix)]
    {
        // The late callback may replace a directory while returning an ordinary error.
        // Native retained handles, rather than matching path strings, must close the call.
        struct ReplacedDirectory {
            original: std::path::PathBuf,
            held: std::path::PathBuf,
        }
        impl Drop for ReplacedDirectory {
            fn drop(&mut self) {
                if self.original.exists() {
                    std::fs::remove_dir(&self.original).unwrap();
                }
                std::fs::rename(&self.held, &self.original).unwrap();
            }
        }
        let owned = oldest.test_owned_closure_history().unwrap();
        let mut replacement = None;
        let result = owned.into_unsigned_closure_with_pass(
            &successor,
            |attempt| {
                let value = inspect(attempt)?;
                if calls.get() == original_calls {
                    let original = history.bodies[0].directory.path().to_path_buf();
                    let held = original.with_extension("closure-owner-original");
                    std::fs::rename(&original, &held).unwrap();
                    let restore = ReplacedDirectory { original, held };
                    PrivateDirectory::open_or_create(&restore.original).unwrap();
                    replacement = Some(restore);
                    return Err(invalid("ordinary closure wallet refusal"));
                }
                Ok(value)
            },
            None,
        );
        assert_eq!(calls.replace(0), original_calls);
        assert!(replacement.is_some());
        assert_ne!(
            result
                .err()
                .expect("replaced original must refuse")
                .to_string(),
            invalid("ordinary closure wallet refusal").to_string()
        );
        drop(replacement);
    }

    // Active callers retain the original physical parser and every cumulative charge.
    let borrowed = oldest.test_owned_closure_history().unwrap();
    let owned = oldest.test_owned_closure_history().unwrap();
    let ((expected, old_parses), old_usage) =
        norito::core::with_decode_limits_measured(limits(usize::MAX), || {
            History::test_closure_parse_work(|| {
                borrowed.verify_unsigned_closure(&successor, inspect)
            })
        });
    let expected = expected.unwrap().unwrap();
    assert_eq!(calls.replace(0), original_calls);
    let ((actual, new_parses), new_usage) =
        norito::core::with_decode_limits_measured(limits(usize::MAX), || {
            History::test_closure_parse_work(|| {
                owned.into_unsigned_closure_with_pass(&successor, inspect, None)
            })
        });
    assert_eq!(calls.replace(0), original_calls);
    assert_eq!((old_parses, new_parses), (1, 1));
    assert_eq!(new_usage, old_usage);
    same_closure(&complete(actual), &expected);
    let exact = old_usage.total_allocated_bytes();
    assert!(exact > 1);
    for capacity in [0, 1, exact - 1] {
        let borrowed = oldest.test_owned_closure_history().unwrap();
        let owned = oldest.test_owned_closure_history().unwrap();
        let old = norito::with_decode_limits_scope(limits(capacity), || {
            borrowed.verify_unsigned_closure(&successor, inspect)
        })
        .err()
        .expect("original active refusal");
        let before = calls.replace(0);
        let new = norito::with_decode_limits_scope(limits(capacity), || {
            owned.into_unsigned_closure_with_pass(&successor, inspect, None)
        })
        .err()
        .expect("consuming active refusal");
        assert_eq!(calls.replace(0), before);
        assert_eq!(new.to_string(), old.to_string());
    }
    let owned = oldest.test_owned_closure_history().unwrap();
    let (retried, usage) = norito::core::with_decode_limits_measured(limits(exact), || {
        owned.into_unsigned_closure_with_pass(&successor, inspect, None)
    });
    assert_eq!(calls.replace(0), original_calls);
    assert_eq!(usage, old_usage);
    same_closure(&complete(retried), &baseline);
    assert_eq!(fixture.native.chain.height(), 4);
}
