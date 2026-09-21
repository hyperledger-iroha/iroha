//! Bounded-digit ownership controls; actual-generator equality lives in MKHE.
use super::*;

pub(crate) mod test_controls_v1 {
    use super::*;
    use core::cell::Cell;
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub(crate) struct WorkV1 {
        pub(crate) table_adds: usize,
        pub(crate) windows: usize,
        pub(crate) doubles: usize,
        pub(crate) selects: usize,
        pub(crate) window_adds: usize,
        pub(crate) folds: usize,
        pub(crate) rho_terms: usize,
        pub(crate) combines: usize,
    }
    pub(crate) enum WorkKindV1 {
        TableAdd,
        Window,
        Double,
        Select,
        WindowAdd,
        Fold,
        RhoTerm,
        Combine,
    }
    std::thread_local! {
        static WORK: Cell<WorkV1> = Cell::new(WorkV1::default());
        static CLEAR: Cell<(usize, bool)> = const { Cell::new((0, false)) };
        static FAIL: Cell<u8> = const { Cell::new(0) };
        static ALLOCS: Cell<usize> = const { Cell::new(0) };
    }
    pub(crate) fn reset_v1() {
        WORK.with(|x| x.set(WorkV1::default()));
        CLEAR.with(|x| x.set((0, false)));
        FAIL.with(|x| x.set(0));
        ALLOCS.with(|x| x.set(0));
    }
    pub(crate) fn work_v1() -> WorkV1 {
        WORK.with(Cell::get)
    }
    pub(crate) fn clear_v1() -> (usize, bool) {
        CLEAR.with(Cell::get)
    }
    pub(crate) fn allocations_v1() -> usize {
        ALLOCS.with(Cell::get)
    }
    pub(crate) fn fail_table_v1() {
        FAIL.with(|x| x.set(1));
    }
    pub(crate) fn panic_table_v1() {
        FAIL.with(|x| x.set(3));
    }
    pub(crate) fn fail_digits_v1() {
        FAIL.with(|x| x.set(2));
    }
    pub(in crate::generalized_bulletproof::secret_u15_msm_v1) fn allocation_v1(
        site: AllocationSiteV1,
    ) -> Result<(), GeneralizedBulletproofErrorV1> {
        ALLOCS.with(|x| x.set(x.get() + 1));
        let site = match site {
            AllocationSiteV1::Table => 1,
            AllocationSiteV1::Digits => 2,
        };
        if site == 1
            && FAIL.with(|x| {
                if x.get() == 3 {
                    x.set(0);
                    true
                } else {
                    false
                }
            })
        {
            panic!("injected table allocation unwind before generator access");
        }
        if FAIL.with(|x| {
            let fail = x.get() == site;
            if fail {
                x.set(0);
            }
            fail
        }) {
            return Err(GeneralizedBulletproofErrorV1::ResourceOverflow);
        }
        Ok(())
    }
    pub(in crate::generalized_bulletproof::secret_u15_msm_v1) fn record_digit_clear_v1(
        values: &[u16],
    ) {
        CLEAR.with(|x| x.set((values.len(), values.iter().all(|x| *x == 0))));
    }
    pub(in crate::generalized_bulletproof::secret_u15_msm_v1) fn record_work_v1(kind: WorkKindV1) {
        WORK.with(|slot| {
            let mut w = slot.get();
            match kind {
                WorkKindV1::TableAdd => w.table_adds += 1,
                WorkKindV1::Window => w.windows += 1,
                WorkKindV1::Double => w.doubles += 1,
                WorkKindV1::Select => w.selects += 1,
                WorkKindV1::WindowAdd => w.window_adds += 1,
                WorkKindV1::Fold => w.folds += 1,
                WorkKindV1::RhoTerm => w.rho_terms += 1,
                WorkKindV1::Combine => w.combines += 1,
            }
            slot.set(w);
        });
    }
}

#[test]
fn u15_plane_rejects_shape_before_allocation_or_read() {
    test_controls_v1::reset_v1();
    for len in [0, 255, 256, 257, 16_383, 16_385, usize::MAX] {
        assert!(matches!(
            SecretU15PlaneV1::from_source_v1(len, |_| panic!(
                "shape refusal must precede source read"
            )),
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        ));
    }
    assert_eq!(test_controls_v1::allocations_v1(), 0);
}

#[test]
fn u15_plane_bounds_error_and_unwind_erase_initialized_prefix() {
    for bad in [32768, u16::MAX] {
        test_controls_v1::reset_v1();
        assert!(
            SecretU15PlaneV1::from_source_v1(16384, |i| Ok(if i == 257 { bad } else { 32767 }))
                .is_err()
        );
        assert_eq!(test_controls_v1::clear_v1(), (257, true));
    }
    test_controls_v1::reset_v1();
    assert!(
        SecretU15PlaneV1::from_source_v1(16384, |i| if i == 255 {
            Err(GeneralizedBulletproofErrorV1::ArithmeticInvariant)
        } else {
            Ok(0x7123)
        })
        .is_err()
    );
    assert_eq!(test_controls_v1::clear_v1(), (255, true));
    let result = std::panic::catch_unwind(|| {
        let _ = SecretU15PlaneV1::from_source_v1(16384, |i| {
            assert_ne!(i, 256, "injected source unwind");
            Ok(0x7123)
        });
    });
    assert!(result.is_err());
    assert_eq!(test_controls_v1::clear_v1(), (256, true));
}

#[test]
fn u15_plane_allocation_failure_precedes_secret_read_and_success_erases() {
    test_controls_v1::reset_v1();
    test_controls_v1::fail_digits_v1();
    assert!(matches!(
        SecretU15PlaneV1::from_source_v1(16384, |_| panic!(
            "allocation failure must precede source read"
        )),
        Err(GeneralizedBulletproofErrorV1::ResourceOverflow)
    ));
    assert_eq!(test_controls_v1::clear_v1(), (0, false));
    let plane = SecretU15PlaneV1::from_source_v1(16384, |i| Ok((i % 32768) as u16)).unwrap();
    assert_eq!(plane.values.capacity(), 16384);
    drop(plane);
    assert_eq!(test_controls_v1::clear_v1(), (16384, true));
}
