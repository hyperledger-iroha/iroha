//! Runtime reclamation binds lower allocation credits to actual storage owners.
use crate::allocation_test_support::*;
use iroha_allocation::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::SeqCst},
    },
    task::{Context, Wake, Waker},
    time::{Duration, Instant},
};
#[test]
fn real_epoch_reclamation_returns_capacity_and_its_release_notification() {
    use concread::ebrcell::EbrCell;

    let allocation = EbrCell::<u64, AllocationCharge>::allocation_layout();
    let registration_bytes =
        iroha_allocation::release::ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(allocation.size() + registration_bytes);
    let mut registration = crate::release_test_support::registration(&budget);
    let mut prepaid = budget.try_reserve(allocation).unwrap();
    let cell = EbrCell::new_charged(7_u64, prepaid.try_split(allocation).unwrap());
    drop(prepaid);
    let unrelated_pin = crossbeam_epoch::pin();
    let mut wait = capacity_wait(
        budget.try_reserve(allocation).unwrap_err(),
        &mut registration,
    );
    let wakes = Arc::new(WakeCount::default());
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(cell);
    unrelated_pin.flush();
    std::thread::spawn(|| {
        for _ in 0..256 {
            crossbeam_epoch::pin().flush();
        }
    })
    .join()
    .unwrap();
    assert_eq!(
        budget.reserved_bytes(),
        allocation.size() + registration_bytes
    );
    assert_eq!(wakes.0.load(SeqCst), 0);
    assert!(poll(&mut wait, &wakes).is_pending());
    drop(unrelated_pin);
    let deadline = Instant::now() + Duration::from_secs(5);
    while budget.reserved_bytes() != registration_bytes {
        assert!(
            Instant::now() < deadline,
            "retired allocation was not reclaimed"
        );
        crossbeam_epoch::pin().flush();
        std::thread::yield_now();
    }
    assert!(poll(&mut wait, &wakes).is_ready());
    drop(budget.try_reserve(allocation).unwrap());
    drop(wait);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn actual_retired_reader_refund_under_a_new_writer_waits_for_its_scope_to_unlock() {
    use concread::internals::lincowcell::{
        LinCowCell, LinCowCellCapable, WriterAdmission, WriterCharges,
    };

    struct Data(u64);
    impl LinCowCellCapable<u64, u64> for Data {
        type WriterInput = ();

        fn create_reader(&self) -> u64 {
            self.0
        }
        fn create_writer(&self, (): Self::WriterInput) -> u64 {
            self.0
        }
        fn pre_commit(&mut self, value: u64, _previous: &u64) -> u64 {
            self.0 = value;
            value
        }
    }
    type Owner = LinCowCell<Data, u64, u64, AllocationCharge>;
    struct Retry {
        owner: Arc<Owner>,
        wakes: AtomicUsize,
    }
    impl Wake for Retry {
        fn wake(self: Arc<Self>) {
            assert_eq!(*self.owner.read(), 11);
            assert!(matches!(
                self.owner
                    .try_write_charged(|_, _| Err::<WriterAdmission<AllocationCharge, ()>, _>(())),
                Err(())
            ));
            self.wakes.fetch_add(1, SeqCst);
        }
    }
    let layouts = Owner::writer_allocation_layouts();
    let initial_layouts = Owner::initial_allocation_layouts();
    let registration_bytes =
        iroha_allocation::release::ReleaseRegistration::allocation_layout().size();
    let budget = AllocationBudget::new(
        initial_layouts.root.size()
            + 3 * layouts.reader.size()
            + layouts.cursor.size()
            + registration_bytes,
    );
    let mut registration = crate::release_test_support::registration(&budget);
    let mut initial = budget
        .try_reserve_layouts([initial_layouts.root, initial_layouts.reader])
        .unwrap();
    let owner = Arc::new(Owner::new_charged(
        Data(7),
        concread::internals::lincowcell::InitialCharges {
            notification: iroha_allocation::release::ReleaseNotification::default(),
            root: initial.try_split(initial_layouts.root).unwrap(),
            reader: initial.try_split(initial_layouts.reader).unwrap(),
        },
    ));
    drop(initial);
    let oldest = owner.read();
    let admit = |_: &Data, layouts: concread::internals::lincowcell::WriterLayouts| {
        let mut prepaid = budget.try_reserve_layouts([layouts.cursor, layouts.reader])?;
        Ok::<_, AllocationRefusal>(WriterAdmission {
            charges: WriterCharges {
                cursor: prepaid.try_split(layouts.cursor).unwrap(),
                reader: prepaid.try_split(layouts.reader).unwrap(),
            },
            input: (),
        })
    };
    let mut writer = owner.write_charged(admit).unwrap();
    *writer = 11;
    writer.commit();
    let wake = Arc::new(Retry {
        owner: Arc::clone(&owner),
        wakes: AtomicUsize::new(0),
    });
    let waker = Waker::from(Arc::clone(&wake));
    let mut context = Context::from_waker(&waker);
    let mut wait = None;
    budget.with_deferred_refund_notifications(|_| {
        let held = owner.write_charged(admit).unwrap();
        let mut pending = capacity_wait(
            budget.try_reserve(layout(1)).unwrap_err(),
            &mut registration,
        );
        assert!(Pin::new(&mut pending).poll(&mut context).is_pending());
        wait = Some(pending);
        drop(oldest);
        assert_eq!(
            budget.reserved_bytes(),
            initial_layouts.root.size()
                + 2 * layouts.reader.size()
                + layouts.cursor.size()
                + registration_bytes
        );
        assert_eq!(wake.wakes.load(SeqCst), 0);
        drop(held);
        assert_eq!(
            budget.reserved_bytes(),
            initial_layouts.root.size() + layouts.reader.size() + registration_bytes
        );
        assert_eq!(wake.wakes.load(SeqCst), 0);
    });
    assert_eq!(wake.wakes.load(SeqCst), 1);
    assert!(
        Pin::new(wait.as_mut().unwrap())
            .poll(&mut context)
            .is_ready()
    );
    drop(wait);
    drop(waker);
    drop(wake);
    drop(owner);
    drop(registration);
    assert_eq!(budget.reserved_bytes(), 0);
}
