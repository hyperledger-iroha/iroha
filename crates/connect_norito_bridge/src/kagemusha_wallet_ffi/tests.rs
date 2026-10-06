//! Boundary tests; stand-in ownership is test-only and cannot open a production wallet.

use super::*;
use advance::{
    KagemushaWalletPlatformV1 as _, KagemushaWalletProbeV1 as Probe,
    KagemushaWalletUnavailableV1 as U,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestWallet {
    calls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
impl Drop for TestWallet {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl Wallet for TestWallet {
    fn commit(&mut self, _: state::FrozenTransition) -> Result<Response> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(Failure::code(PROOF_REJECTED))
    }
    fn retry(&mut self, op: &[u8; 32]) -> Result<Response> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match op[0] {
            0 => Ok(Response::default()),
            1 => Ok(completion(Some(state::Completion::Complete(vec![
                0, 255, 0, 7,
            ])))),
            2 => Ok(completion(Some(state::Completion::Pending))),
            3 => Ok(completion(Some(state::Completion::DeliveryDataLoss))),
            _ => Err(Failure::unavailable(UNAVAILABLE, U::Io(5))),
        }
    }
    fn resume(&mut self) -> Result<Response> {
        Ok(completion(Some(state::Completion::Pending)))
    }
    fn fold(&mut self) -> Result<Response> {
        Ok(Response {
            kind: 8,
            sequence: (1_u128 << 93) + 7,
            detail: 3,
            ..Response::default()
        })
    }
    fn credit(&mut self, _: &[u8; 32], _: &[u8; 32]) -> Result<Response> {
        Err(Failure::code(FOLD_REQUIRED))
    }
}
fn installed() -> (u64, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));
    let id = install(
        Box::new(TestWallet {
            calls: calls.clone(),
            drops: drops.clone(),
        }),
        state::Scheduler::new(),
    )
    .unwrap();
    (id, calls, drops)
}
#[test]
fn handles_never_repeat_close_releases_owner_and_stale_captures_cannot_act() {
    let (id, calls, drops) = installed();
    let stale = owner(id).unwrap();
    activity(id, true, false).unwrap();
    assert_eq!(retry(id, &[1; 32]).unwrap().bytes, [0, 255, 0, 7]);
    close(id).unwrap();
    assert!(stale.wallet.lock().unwrap().is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(close(id).unwrap_err().status, CLOSED);
    assert_eq!(retry(id, &[1; 32]).unwrap_err().status, CLOSED);
    assert_eq!(activity(id, false, false).unwrap_err().status, CLOSED);
    let (next, _, _) = installed();
    assert!(next > id);
    close(next).unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[test]
fn c_calls_preserve_exact_retry_unknown_pending_loss_and_platform_failure() {
    let (id, calls, _) = installed();
    for _ in 0..2 {
        let mut result = WalletResult::default();
        assert_eq!(
            unsafe { connect_norito_kagemusha_wallet_retry_v1(id, [1; 32].as_ptr(), &mut result) },
            0
        );
        assert_eq!(result.status, 1);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(result.bytes, result.length) },
            [0, 255, 0, 7]
        );
        crate::connect_norito_free(result.bytes);
    }
    assert_eq!(retry(id, &[0; 32]).unwrap().kind, 0);
    assert_eq!(retry(id, &[2; 32]).unwrap().kind, 2);
    assert_eq!(retry(id, &[3; 32]).unwrap().kind, 5);
    assert_eq!(
        retry(id, &[4; 32]).unwrap_err(),
        Failure {
            status: UNAVAILABLE,
            reason: 3,
            platform_code: 5
        }
    );
    assert_eq!(retry(id, &[0; 31]).unwrap_err().status, INVALID);
    assert_eq!(commit(id, &[0; 5]).unwrap_err().status, INVALID);
    assert_eq!(calls.load(Ordering::SeqCst), 6);
    let mut result = WalletResult::default();
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_fold_v1(id, &mut result) },
        0
    );
    assert_eq!(
        (
            result.status,
            result.sequence_low,
            result.sequence_high,
            result.detail
        ),
        (8, 7, 1 << 29, 3)
    );
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_commit_v1(id, std::ptr::null(), 10, &mut result) },
        INVALID
    );
    assert_eq!(
        unsafe { connect_norito_kagemusha_wallet_resume_v1(id, std::ptr::null_mut()) },
        INVALID
    );
    close(id).unwrap();
}
#[test]
fn failure_mapping_never_turns_uncertain_or_missing_custody_into_absence() {
    use advance::KagemushaWalletProviderErrorV1 as E;
    for reason in [
        U::Locked,
        U::BeforeFirstUnlock,
        U::Busy,
        U::Io(5),
        U::Platform(-1),
        U::KeyUnusable,
        U::PermanentlyInvalidated,
    ] {
        assert_eq!(Failure::from(E::Unavailable(reason)).status, UNAVAILABLE);
        assert_eq!(Failure::from(E::Uncertain(reason)).status, UNCERTAIN);
    }
    assert_eq!(
        Failure::from(E::UnavailableCustodyData {
            object: "archive checkpoint"
        })
        .status,
        CUSTODY_LOST
    );
    assert_eq!(
        Failure::from(state::Error::WitnessLost("Ω")).status,
        CUSTODY_LOST
    );
    assert_eq!(
        run::<()>(|| panic!("test contained panic"))
            .unwrap_err()
            .status,
        INTERNAL
    );
}

#[derive(Default)]
struct CallbackState {
    retained: AtomicUsize,
    released: AtomicUsize,
    calls: AtomicUsize,
    answer: Mutex<(PlatformReply, Vec<u8>)>,
}
unsafe extern "C" fn retain(pointer: *mut std::ffi::c_void) {
    unsafe { &*pointer.cast::<CallbackState>() }
        .retained
        .fetch_add(1, Ordering::SeqCst);
}
unsafe extern "C" fn release(pointer: *mut std::ffi::c_void) {
    unsafe { &*pointer.cast::<CallbackState>() }
        .released
        .fetch_add(1, Ordering::SeqCst);
}
unsafe extern "C" fn invoke(
    pointer: *mut std::ffi::c_void,
    _: u32,
    _: *const u8,
    _: *const u8,
    _: usize,
    _: u32,
    output: *mut u8,
    capacity: usize,
    reply: *mut PlatformReply,
) {
    let state = unsafe { &*pointer.cast::<CallbackState>() };
    state.calls.fetch_add(1, Ordering::SeqCst);
    let answer = state.answer.lock().unwrap();
    if answer.1.len() <= capacity {
        unsafe { std::ptr::copy_nonoverlapping(answer.1.as_ptr(), output, answer.1.len()) };
    }
    unsafe { reply.write(answer.0) };
}
fn callbacks(state: &CallbackState) -> PlatformCallbacks {
    PlatformCallbacks {
        version: 1,
        anchor_policy: 1,
        context: std::ptr::from_ref(state).cast_mut().cast(),
        retain: Some(retain),
        release: Some(release),
        invoke: Some(invoke),
    }
}
#[test]
fn callback_tri_state_bounds_anchor_policy_and_lifetime_are_explicit() {
    let state = CallbackState::default();
    let adapter = unsafe { CallbackPlatform::new(callbacks(&state)) }.unwrap();
    let slot = advance::KagemushaWalletSlotIdV1([1; 32]);
    assert_eq!(
        adapter.anchor_policy(),
        advance::KagemushaWalletAnchorPolicyV1::Keychain
    );
    assert_eq!(state.retained.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.key_probe(&slot), Probe::Unavailable(U::Platform(0)));
    *state.answer.lock().unwrap() = (
        PlatformReply {
            tag: 1,
            ..PlatformReply::default()
        },
        vec![],
    );
    assert_eq!(adapter.key_probe(&slot), Probe::Absent);
    assert_eq!(adapter.storage_state(), Err(U::Platform(0)));
    for (reason, expected) in [
        (0, U::Locked),
        (1, U::BeforeFirstUnlock),
        (2, U::Busy),
        (3, U::Io(9)),
        (4, U::Platform(9)),
        (5, U::KeyUnusable),
        (6, U::PermanentlyInvalidated),
        (77, U::Platform(0)),
    ] {
        *state.answer.lock().unwrap() = (
            PlatformReply {
                tag: 2,
                reason,
                code: 9,
                length: 0,
            },
            vec![],
        );
        assert_eq!(adapter.key_probe(&slot), Probe::Unavailable(expected));
        assert_eq!(adapter.storage_state(), Err(expected));
    }
    *state.answer.lock().unwrap() = (
        PlatformReply {
            tag: 0,
            length: 66,
            ..PlatformReply::default()
        },
        vec![],
    );
    assert_eq!(adapter.key_probe(&slot), Probe::Unavailable(U::Platform(0)));
    assert_eq!(
        adapter.anchor_create(&slot, b"a"),
        advance::KagemushaWalletPublishOutcomeV1::Uncertain(U::Platform(0))
    );
    *state.answer.lock().unwrap() = (
        PlatformReply {
            tag: 0,
            length: 4,
            ..PlatformReply::default()
        },
        b"/tmp".to_vec(),
    );
    assert_eq!(
        adapter.custody_root().unwrap(),
        std::path::Path::new("/tmp")
    );
    drop(adapter);
    assert_eq!(state.released.load(Ordering::SeqCst), 1);
    let mut invalid = callbacks(&state);
    invalid.anchor_policy = 0;
    assert!(unsafe { CallbackPlatform::new(invalid) }.is_err());
    assert_eq!(state.retained.load(Ordering::SeqCst), 1);
}
#[test]
fn foreign_open_has_no_custody_side_effect_or_proof_verdict_fallback() {
    let state = CallbackState::default();
    let mut id = 9;
    assert_eq!(
        unsafe {
            connect_norito_kagemusha_wallet_open_v1(
                &callbacks(&state),
                [1; 32].as_ptr(),
                [2; 32].as_ptr(),
                [3; 32].as_ptr(),
                [4; 32].as_ptr(),
                &mut id,
            )
        },
        ARTIFACTS_UNAVAILABLE
    );
    assert_eq!(id, 0);
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.retained.load(Ordering::SeqCst), 0);
}
