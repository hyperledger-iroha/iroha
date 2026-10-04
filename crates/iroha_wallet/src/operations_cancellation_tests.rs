//! Cancellation during real wallet preparation and submission retains exact original custody.

use super::{
    tests::{request, service},
    *,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[test]
fn cancellation_binding_survives_deadline_copies_and_cannot_be_replaced() {
    let (service, transport) = service();
    let signal = Arc::new(AtomicBool::new(false));
    let deadline = Instant::now() + Duration::from_secs(30);
    let bounded = service
        .with_cancellation(Arc::clone(&signal))
        .unwrap()
        .with_deadline(deadline)
        .unwrap()
        .with_deadline(deadline + Duration::from_secs(30))
        .unwrap()
        .with_cancellation(Arc::clone(&signal))
        .unwrap();
    assert_eq!(bounded.deadline, Some(deadline));
    assert!(Arc::ptr_eq(bounded.cancellation.as_ref().unwrap(), &signal));
    assert!(
        bounded
            .with_cancellation(Arc::new(AtomicBool::new(false)))
            .is_err()
    );
    assert_eq!(transport.requests.load(Ordering::SeqCst), 0);
}

#[test]
fn cancellation_during_quote_preserves_request_without_payload_signature_or_dispatch() {
    let (service, transport) = service();
    let signal = Arc::new(AtomicBool::new(false));
    *transport.cancel_on_path.lock().unwrap() =
        Some(("/v1/fees/quote".into(), Arc::clone(&signal)));
    let service = service.with_cancellation(signal).unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("cancelled-quote");
    let error = service.prepare_transfer(&request(), &path).unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert_eq!(transport.quote_count.load(Ordering::SeqCst), 1);
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
    assert!(path.join("preparation.json").is_file());
    assert!(!path.join("payload.json").exists());
    assert!(!path.join("operation.json").exists());
    assert!(!path.join("submission.json").exists());
    let before = transport.requests.load(Ordering::SeqCst);
    let inspected = service
        .inspect_transfer_preparation(&path, &request())
        .unwrap();
    assert_eq!(inspected.phase(), NativePreparationPhase::RequestOnly);
    assert_eq!(transport.requests.load(Ordering::SeqCst), before);
    assert!(service.prepare_transfer(&request(), &path).is_err());
    assert_eq!(transport.requests.load(Ordering::SeqCst), before);
}

#[test]
fn cancellation_during_submit_preflight_preserves_signature_without_dispatch_marker() {
    let (service, transport) = service();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("cancelled-submit");
    service.prepare_transfer(&request(), &path).unwrap();
    let original = std::fs::read(path.join("operation.json")).unwrap();
    let signal = Arc::new(AtomicBool::new(false));
    *transport.cancel_on_path.lock().unwrap() =
        Some(("/v1/node/capabilities".into(), Arc::clone(&signal)));
    let service = service.with_cancellation(signal).unwrap();
    let error = service
        .submit(&path, NativeOperationKind::Transfer)
        .unwrap_err();
    assert!(error.to_string().contains("cancelled"));
    assert_eq!(
        std::fs::read(path.join("operation.json")).unwrap(),
        original
    );
    assert!(!path.join("submission.json").exists());
    assert_eq!(transport.dispatch_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        service
            .inspect_transfer_preparation(&path, &request())
            .unwrap()
            .phase(),
        NativePreparationPhase::Signed
    );
}
