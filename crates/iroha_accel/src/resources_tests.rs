//! Driver-independent reservation and physical-health controls.

use super::*;

fn limits() -> GpuResourceLimits {
    GpuResourceLimits {
        host_bytes: 17,
        pinned_bytes: 31,
        device_bytes: 47,
        in_flight: 2,
    }
}

#[test]
fn separate_pools_refuse_before_payload_allocation_and_roll_back() {
    let pools = ResourcePools::new(limits());
    for (request, expected) in [
        (
            BufferRequest {
                host_bytes: 18,
                ..BufferRequest::default()
            },
            ResourceRefusal::Host,
        ),
        (
            BufferRequest {
                host_bytes: 1,
                pinned_bytes: 32,
                device_bytes: 0,
            },
            ResourceRefusal::Pinned,
        ),
        (
            BufferRequest {
                host_bytes: 1,
                pinned_bytes: 1,
                device_bytes: 48,
            },
            ResourceRefusal::Device,
        ),
    ] {
        assert!(matches!(pools.try_reserve(request), Err(actual) if actual == expected));
        assert_eq!(pools.usage().reserved, BufferRequest::default());
        assert_eq!(pools.usage().in_flight, 0);
    }
}

#[test]
fn exact_request_retains_original_pool_through_limit_shrink() {
    let pools = ResourcePools::new(limits());
    let request = BufferRequest {
        host_bytes: 17,
        pinned_bytes: 31,
        device_bytes: 47,
    };
    let borrowed = pools.try_reserve(request).unwrap();
    assert_eq!(borrowed.request(), request);
    assert!(Arc::ptr_eq(borrowed.owner(), &pools));
    pools.set_limits(GpuResourceLimits {
        host_bytes: 0,
        pinned_bytes: 0,
        device_bytes: 0,
        in_flight: 0,
    });
    assert_eq!(pools.usage().reserved, request);
    assert_eq!(pools.usage().in_flight, 1);
    assert!(matches!(
        pools.try_reserve(BufferRequest::default()),
        Err(ResourceRefusal::InFlight)
    ));
    drop(borrowed);
    assert_eq!(pools.usage().reserved, BufferRequest::default());
    assert_eq!(pools.usage().peak, request);
    assert_eq!(pools.usage().in_flight, 0);
    pools.set_limits(limits());
    assert!(pools.try_reserve(request).is_ok());
}

#[test]
fn independent_count_limit_never_waits_for_a_parent() {
    let pools = ResourcePools::new(limits());
    let parent = pools.try_reserve(BufferRequest::default()).unwrap();
    let child = pools.try_reserve(BufferRequest::default()).unwrap();
    assert!(matches!(
        pools.try_reserve(BufferRequest::default()),
        Err(ResourceRefusal::InFlight)
    ));
    assert_eq!(pools.usage().peak_in_flight, 2);
    drop(child);
    assert!(pools.try_reserve(BufferRequest::default()).is_ok());
    drop(parent);
    assert_eq!(pools.usage().in_flight, 0);
}

#[test]
fn zero_byte_limits_are_not_unlimited() {
    let pools = ResourcePools::new(GpuResourceLimits {
        host_bytes: 0,
        pinned_bytes: 0,
        device_bytes: 0,
        in_flight: 1,
    });
    assert!(matches!(
        pools.try_reserve(BufferRequest {
            device_bytes: 1,
            ..BufferRequest::default()
        }),
        Err(ResourceRefusal::Device)
    ));
    assert!(pools.try_reserve(BufferRequest::default()).is_ok());
}

#[test]
fn concurrent_admission_preserves_aggregate_byte_and_count_limits() {
    let pools = ResourcePools::new(limits());
    let start = Arc::new(std::sync::Barrier::new(9));
    let finish = Arc::new(std::sync::Barrier::new(9));
    let admitted = Arc::new(AtomicUsize::new(0));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let pools = Arc::clone(&pools);
            let start = Arc::clone(&start);
            let finish = Arc::clone(&finish);
            let admitted = Arc::clone(&admitted);
            std::thread::spawn(move || {
                start.wait();
                let lease = pools
                    .try_reserve(BufferRequest {
                        device_bytes: 23,
                        ..BufferRequest::default()
                    })
                    .ok();
                if lease.is_some() {
                    admitted.fetch_add(1, Ordering::Relaxed);
                }
                finish.wait();
                drop(lease);
            })
        })
        .collect();
    start.wait();
    finish.wait();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(admitted.load(Ordering::Relaxed), 2);
    assert_eq!(pools.usage().peak.device_bytes, 46);
    assert_eq!(pools.usage().peak_in_flight, 2);
    assert_eq!(pools.usage().in_flight, 0);
}

#[test]
fn device_failure_does_not_change_another_generation() {
    let failed = DeviceHealth::default();
    let healthy = DeviceHealth::default();
    assert!(failed.usable());
    failed.quarantine(false);
    assert!(!failed.usable());
    assert!(!failed.uncertain());
    failed.quarantine(true);
    failed.quarantine(false);
    assert!(failed.uncertain());
    assert!(healthy.usable());
    assert!(!healthy.uncertain());
}

#[test]
fn work_admission_refuses_during_count_configuration_without_taking_credit() {
    let pools = ResourcePools::new(limits());
    let _writer = pools.in_flight_limit.write();
    assert!(matches!(
        pools.try_reserve(BufferRequest::default()),
        Err(ResourceRefusal::InFlight)
    ));
    assert_eq!(pools.usage().in_flight, 0);
    assert_eq!(pools.usage().reserved, BufferRequest::default());
}
