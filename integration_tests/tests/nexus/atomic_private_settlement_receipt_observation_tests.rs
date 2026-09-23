//! Closure-level receipt observation controls using the actual diagnostic TLS.
//!
//! Controlled closure values exercise ownership and timing wiring only. They do
//! not stand in for SDK receipt validation, signed finality, or a live network.

use super::{
    SmokeDiagnosticPhaseV1, SmokeDiagnosticScopeV1, SmokeDiagnosticSpanV1, TEST_STACK_BYTES,
    collect_bounded_observations, observe_smoke_receipt_job_v1,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

#[test]
fn workers_keep_context_and_emit_one_milestone_across_rounds() {
    let scope = SmokeDiagnosticScopeV1::start();
    let parent = SmokeDiagnosticSpanV1::start(SmokeDiagnosticPhaseV1::AllPeerReceipt, None);
    let parent_event = parent.event.unwrap();
    let original = SmokeDiagnosticScopeV1::capture().unwrap();
    let reported = AtomicBool::new(false);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx): (Vec<_>, Vec<_>) = (0..4).map(|_| mpsc::channel::<()>()).unzip();
    let mut releases = release_rx.into_iter();
    let jobs = (0..16)
        .map(|peer| (peer, (peer < 4).then(|| releases.next().unwrap())))
        .collect();
    std::thread::scope(|threads| {
        let collector = threads.spawn(|| {
            collect_bounded_observations(jobs, TEST_STACK_BYTES, |(peer, release)| {
                assert!(SmokeDiagnosticScopeV1::capture().is_none());
                let response = observe_smoke_receipt_job_v1(
                    Some(original.clone()),
                    &reported,
                    || {
                        let installed = SmokeDiagnosticScopeV1::capture().unwrap();
                        assert_eq!(installed.origin, original.origin);
                        assert_eq!(installed.active, [parent_event.span]);
                        assert!(Arc::ptr_eq(&installed.next_span, &original.next_span));
                        if let Some(release) = release {
                            started_tx.send(peer).unwrap();
                            release.recv_timeout(Duration::from_secs(10)).unwrap();
                        }
                        (peer, 4242_u64)
                    },
                    |response| Some(response.1),
                );
                assert!(SmokeDiagnosticScopeV1::capture().is_none());
                response
            })
        });
        let mut started = (0..4)
            .map(|_| started_rx.recv_timeout(Duration::from_secs(10)).unwrap())
            .collect::<Vec<_>>();
        started.sort_unstable();
        assert_eq!(started, [0, 1, 2, 3]);
        assert!(!reported.load(Ordering::Relaxed));
        for release in release_tx.into_iter().rev() {
            release.send(()).unwrap();
        }
        assert_eq!(
            collector.join().unwrap(),
            (0..16).map(|peer| (peer, 4242)).collect::<Vec<_>>()
        );
    });
    assert!(reported.load(Ordering::Relaxed));
    let responses = collect_bounded_observations((0..16).collect(), TEST_STACK_BYTES, |peer| {
        observe_smoke_receipt_job_v1(
            Some(original.clone()),
            &reported,
            || (peer, 4242_u64),
            |response| Some(response.1),
        )
    });
    assert_eq!(
        responses,
        (0..16).map(|peer| (peer, 4242)).collect::<Vec<_>>()
    );
    let restored = SmokeDiagnosticScopeV1::capture().unwrap();
    assert_eq!(restored.active, original.active);
    assert_eq!(restored.origin, original.origin);
    assert!(Arc::ptr_eq(&restored.next_span, &original.next_span));
    // The standalone runner also checks the actual stderr stream for exactly
    // one finalized_receipt_reported event, with this parent and height 4242.
    parent.complete();
    drop(scope);
    assert!(SmokeDiagnosticScopeV1::capture().is_none());
}

#[test]
fn failed_query_and_pending_value_are_returned_without_a_milestone() {
    let _scope = SmokeDiagnosticScopeV1::start();
    let original = SmokeDiagnosticScopeV1::capture().unwrap();
    let reported = AtomicBool::new(false);
    let original_error = Arc::new(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "controlled query timeout",
    ));
    let responses = collect_bounded_observations((0..16).collect(), TEST_STACK_BYTES, |peer| {
        let response: Result<Option<u64>, Arc<std::io::Error>> = observe_smoke_receipt_job_v1(
            Some(original.clone()),
            &reported,
            || {
                assert!(SmokeDiagnosticScopeV1::capture().is_some());
                if peer % 2 == 0 {
                    Err(Arc::clone(&original_error))
                } else {
                    Ok(None)
                }
            },
            |response| response.as_ref().ok().copied().flatten(),
        );
        assert!(SmokeDiagnosticScopeV1::capture().is_none());
        response
    });
    assert_eq!(responses.len(), 16);
    for (peer, response) in responses.into_iter().enumerate() {
        if peer % 2 == 0 {
            assert!(Arc::ptr_eq(&response.unwrap_err(), &original_error));
        } else {
            assert_eq!(response.unwrap(), None);
        }
    }
    assert!(!reported.load(Ordering::Relaxed));
    assert_eq!(
        SmokeDiagnosticScopeV1::capture().unwrap().origin,
        original.origin
    );
}

#[test]
fn receipt_job_unwind_restores_existing_context_without_a_milestone() {
    let _scope = SmokeDiagnosticScopeV1::start();
    let parent = SmokeDiagnosticSpanV1::start(SmokeDiagnosticPhaseV1::AllPeerReceipt, None);
    let original = SmokeDiagnosticScopeV1::capture().unwrap();
    let mut installed = original.clone();
    installed.active = vec![100];
    let reported = AtomicBool::new(false);
    let result = std::panic::catch_unwind(|| {
        observe_smoke_receipt_job_v1(
            Some(installed),
            &reported,
            || -> u64 {
                assert_eq!(SmokeDiagnosticScopeV1::capture().unwrap().active, [100]);
                panic!("controlled receipt query unwind");
            },
            |height| Some(*height),
        )
    });
    assert!(result.is_err());
    assert!(!reported.load(Ordering::Relaxed));
    let restored = SmokeDiagnosticScopeV1::capture().unwrap();
    assert_eq!(restored.active, original.active);
    assert_eq!(restored.origin, original.origin);
    assert!(Arc::ptr_eq(&restored.next_span, &original.next_span));
    parent.complete();
}

#[test]
fn receipt_job_keeps_diagnostics_disabled_without_a_parent_scope() {
    assert!(SmokeDiagnosticScopeV1::capture().is_none());
    let reported = AtomicBool::new(false);
    let responses = collect_bounded_observations((0..16).collect(), TEST_STACK_BYTES, |peer| {
        observe_smoke_receipt_job_v1(
            None,
            &reported,
            || {
                assert!(SmokeDiagnosticScopeV1::capture().is_none());
                (peer, 707_u64)
            },
            |response| Some(response.1),
        )
    });
    assert_eq!(
        responses,
        (0..16).map(|peer| (peer, 707)).collect::<Vec<_>>()
    );
    assert!(reported.load(Ordering::Relaxed));
    assert!(SmokeDiagnosticScopeV1::capture().is_none());
}
