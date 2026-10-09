//! Synthetic producer bytes exercise thread custody, never financial proof admission.
use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{account::AccountId, isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1};
use std::{sync::atomic::AtomicUsize, time::Duration};

fn job(key: u8) -> Job {
    Job {
        payer: AccountId::new(
            KeyPair::from_seed(vec![key; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        ),
        key: [key; 32],
        receipt: KagemushaWalletLoadReceiptV1 {
            version: 1,
            scheme_id: [1; 32],
            asset_digest: [2; 32],
            wallet_id: [3; 32],
            request_id: [key; 32],
            ordinal: 0,
            amount: 1,
            online_charge: 0,
            charge_quote: [0; 32],
            transaction_hash: [4; 32],
            block_height: 2,
            payer_account_digest: [5; 32],
        },
    }
}
struct Custody(Arc<AtomicBool>);
impl Custody {
    fn held(&self) {
        assert!(!self.0.load(Ordering::Acquire));
    }
}
impl Drop for Custody {
    fn drop(&mut self) {
        assert!(tokio::runtime::Handle::try_current().is_err());
        self.0.store(true, Ordering::Release);
    }
}
async fn stopped(owner: &Worker) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !owner.stop.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test(flavor = "current_thread")]
async fn shutdown_cancels_active_work_and_joins_custody_before_completion() {
    let dropped = Arc::new(AtomicBool::new(false));
    let custody = Custody(Arc::clone(&dropped));
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let (started, began) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let owner = Arc::new(
        Worker::start(
            2,
            ServerFinalityCancellationV1::default(),
            move |_, stop| {
                custody.held();
                observed.fetch_add(1, Ordering::AcqRel);
                started.send(()).unwrap();
                released.recv().unwrap();
                assert!(stop.load(Ordering::Acquire));
                Ok(vec![1, 2, 3])
            },
        )
        .unwrap(),
    );
    let shutdown = ShutdownSignal::new();
    let supervisor = owner.supervise(shutdown.clone()).unwrap();
    assert!(owner.supervise(shutdown.clone()).is_err());
    assert_eq!(owner.read_or_schedule(job(1)).unwrap(), None);
    began.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(owner.read_or_schedule(job(2)).unwrap(), None);
    shutdown.send();
    stopped(&owner).await;
    assert!(!supervisor.is_finished());
    assert!(!dropped.load(Ordering::Acquire));
    assert!(owner.read_or_schedule(job(3)).is_err());
    release.send(()).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), supervisor)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        result,
        crate::ToriiCriticalWorkerExit::StoppedByShutdown
    ));
    assert_eq!(calls.load(Ordering::Acquire), 1);
    assert!(dropped.load(Ordering::Acquire));
    assert!(owner.thread.lock().unwrap().is_none());
    assert!(
        owner
            .queue
            .lock()
            .unwrap()
            .entries
            .values()
            .all(|v| matches!(v, Status::Pending))
    );
}
#[tokio::test(flavor = "current_thread")]
async fn panic_remains_unexpected_even_when_shutdown_races() {
    for request_shutdown in [false, true] {
        let dropped = Arc::new(AtomicBool::new(false));
        let custody = Custody(Arc::clone(&dropped));
        let (started, began) = mpsc::sync_channel(1);
        let (release, released) = mpsc::sync_channel(1);
        let owner = Arc::new(
            Worker::start(1, ServerFinalityCancellationV1::default(), move |_, _| {
                custody.held();
                started.send(()).unwrap();
                released.recv().unwrap();
                panic!("fixture finality producer panic");
            })
            .unwrap(),
        );
        let shutdown = ShutdownSignal::new();
        let supervisor = owner.supervise(shutdown.clone()).unwrap();
        owner.read_or_schedule(job(1)).unwrap();
        began.recv_timeout(Duration::from_secs(5)).unwrap();
        if request_shutdown {
            shutdown.send();
            stopped(&owner).await;
        }
        release.send(()).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), supervisor)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            crate::ToriiCriticalWorkerExit::UnexpectedExit
        ));
        assert!(dropped.load(Ordering::Acquire));
        assert!(owner.thread.lock().unwrap().is_none());
        assert!(owner.read_or_schedule(job(2)).is_err());
    }
}
#[tokio::test(flavor = "current_thread")]
async fn unregistered_drop_retains_join_until_custody_drops() {
    let scratch =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/qualification");
    std::fs::create_dir_all(&scratch).unwrap();
    let directory = tempfile::tempdir_in(&scratch).unwrap();
    let path = directory.path().join("custody.lock");
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    locked.try_lock().unwrap();
    let subsequent = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let custody = Custody(Arc::clone(&dropped));
    let (started, began) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let owner = Worker::start(
        1,
        ServerFinalityCancellationV1::default(),
        move |_, stop| {
            custody.held();
            assert!(locked.metadata().unwrap().is_file());
            started.send(()).unwrap();
            released.recv().unwrap();
            assert!(stop.load(Ordering::Acquire));
            Err("cancelled fixture")
        },
    )
    .unwrap();
    owner.read_or_schedule(job(1)).unwrap();
    began.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(owner);
    // Rollback has returned, but the actual producer still holds the real lock.
    assert!(subsequent.try_lock().is_err());
    assert!(!dropped.load(Ordering::Acquire));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if dropped.load(Ordering::Acquire) && subsequent.try_lock().is_ok() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[test]
fn completed_data_transfers_once_and_full_queue_adds_no_work() {
    let (release, released) = mpsc::sync_channel(1);
    let (started, began) = mpsc::sync_channel(1);
    let owner = Worker::start(1, ServerFinalityCancellationV1::default(), move |_, _| {
        started.send(()).unwrap();
        released.recv().unwrap();
        Ok(vec![7])
    })
    .unwrap();
    assert_eq!(owner.read_or_schedule(job(1)).unwrap(), None);
    began.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(owner.read_or_schedule(job(1)).unwrap(), None);
    assert!(owner.read_or_schedule(job(2)).is_err());
    release.send(()).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(bytes) = owner.read_or_schedule(job(1)).unwrap() {
            assert_eq!(bytes, vec![7]);
            break;
        }
        assert!(std::time::Instant::now() < until);
        thread::yield_now();
    }
    assert!(owner.queue.lock().unwrap().entries.is_empty());
    assert!(owner.close_and_join());
    assert!(owner.read_or_schedule(job(1)).is_err());
}
#[test]
fn invalid_bounds_and_unsupervised_runtime_refuse() {
    for capacity in [0, 65, usize::MAX] {
        assert!(
            Worker::start(
                capacity,
                ServerFinalityCancellationV1::default(),
                |_, _| unreachable!()
            )
            .is_err()
        );
    }
    let owner = Arc::new(
        Worker::start(
            1,
            ServerFinalityCancellationV1::default(),
            |_, _| unreachable!(),
        )
        .unwrap(),
    );
    assert!(owner.supervise(ShutdownSignal::new()).is_err());
    assert!(owner.close_and_join());
}

#[test]
fn factory_keeps_non_send_recipe_state_on_the_actual_worker() {
    let caller = thread::current().id();
    let owner = Worker::start_with(1, ServerFinalityCancellationV1::default(), move || {
        let thread = thread::current().id();
        assert_ne!(thread, caller);
        let state = std::rc::Rc::new(std::cell::Cell::new(0_u8));
        move |_: &Job, _: &AtomicBool| {
            assert_eq!(thread::current().id(), thread);
            state.set(state.get() + 1);
            Ok(vec![state.get()])
        }
    })
    .unwrap();
    assert_eq!(owner.read_or_schedule(job(1)).unwrap(), None);
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(bytes) = owner.read_or_schedule(job(1)).unwrap() {
            assert_eq!(bytes, [1]);
            break;
        }
        assert!(std::time::Instant::now() < until);
        thread::yield_now();
    }
    assert!(owner.close_and_join());
}
