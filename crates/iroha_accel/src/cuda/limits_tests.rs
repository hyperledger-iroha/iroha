//! Driver-independent controls for complete physical registry policy bounds.

use super::*;
use crate::GpuResourceLimits;

#[test]
fn install_and_reload_keep_one_owner_without_driver_initialization() {
    // Zero slots authorize no contexts or payloads. Installation and reload are
    // still real production methods, and must work on a driverless CPU host.
    let zero = RegistryLimits {
        metadata_bytes: 0,
        devices: 0,
        discovery_ordinals: 0,
        modules: 0,
        streams: 0,
        artifact_bytes: 0,
        work: GpuResourceLimits {
            host_bytes: 0,
            pinned_bytes: 0,
            device_bytes: 0,
            in_flight: 0,
        },
    };
    let original = CudaProcess::install(zero).unwrap();
    assert!(std::ptr::eq(CudaProcess::get().unwrap(), original));
    assert_eq!(original.record_capacity(), 0);
    assert_eq!(original.record_count(), 0);
    assert_eq!(original.usable_device_count(), 0);
    assert_eq!(original.usage().observed_devices, 0);
    assert!(std::ptr::eq(CudaProcess::install(zero).unwrap(), original));
    let grown = RegistryLimits { devices: 1, ..zero };
    assert!(matches!(
        CudaProcess::install(grown),
        Err(CudaFailure::InvalidRequest)
    ));
    assert!(std::ptr::eq(CudaProcess::get().unwrap(), original));
    assert_eq!(original.usage().metadata_bytes, [0, 0]);
}

// This isolated fixture constructs no driver-owned handles and shares no global
// installation state with the public install/reload test.
pub(super) fn test_registry() -> CudaProcess {
    use crate::{ProcessResources, resources::ResourcePools, slots::Slots};
    let limits = RegistryLimits::STANDARD;
    let owner = Box::leak(Box::new(ProcessResources {
        metadata: mv::allocation::AllocationBudget::new(limits.metadata_bytes),
        modules: Slots::new(limits.modules),
        streams: Slots::new(limits.streams),
        resources: ResourcePools::new(limits.work),
    }));
    let mut reserve = owner.metadata.try_reserve_bytes(4096).unwrap();
    let mut records = ChargedBuffer::from_reservation(1, &mut reserve).unwrap();
    let primary = test_primary(&mut reserve);
    let modules = ChargedBuffer::from_reservation(1, &mut reserve).unwrap();
    records.push_reserved(
        ChargedShared::from_reservation(
            Record {
                primary,
                modules: Mutex::new(modules),
            },
            &mut reserve,
        )
        .unwrap_or_else(|_| panic!("test record reservation")),
    );
    CudaProcess {
        inventory: Mutex::new(Inventory {
            records,
            last_attempt: None,
        }),
        owner,
        limits: Mutex::new(limits),
        capacity: 1,
    }
}

pub(super) fn test_primary(reserve: &mut AllocationReservation) -> ChargedShared<Primary> {
    ChargedShared::from_reservation(
        Primary {
            identity: DeviceIdentity {
                uuid: [1; 16],
                driver_version: 1,
            },
            device: 0,
            capabilities: Capabilities::test_device(),
            capacity: DeviceCapacity::new(1024),
            native: Mutex::new(ptr::null_mut()),
            health: DeviceHealth::default(),
            gate: Mutex::new(()),
        },
        reserve,
    )
    .unwrap_or_else(|_| panic!("test primary reservation"))
}

#[test]
fn retained_device_handle_cannot_start_work_after_device_cap_shrink() {
    let process = test_registry();
    let device = process.device(0).unwrap();
    let mut limits = *process.limits.lock();
    limits.devices = 0;
    process.reconfigure(limits).unwrap();
    assert!(process.device(0).is_none());
    let artifact = PtxArtifact::new(c"test");
    assert!(matches!(
        device.prepare(
            &[artifact],
            WorkRequest {
                host_bytes: 0,
                pinned_bytes: 0,
                device_bytes: 0,
            }
        ),
        Err(CudaFailure::Capacity)
    ));
    assert_eq!(process.owner.usage().in_flight, [0, 0]);
    assert_eq!(process.owner.usage().streams, [0, 0]);
}

#[test]
fn count_probe_refuses_an_in_progress_discovery_without_waiting() {
    let process = test_registry();
    let _discovery = process.inventory.lock();
    assert_eq!(process.try_record_count(), Err(CudaFailure::Busy));
}

#[test]
fn uncreated_module_rollback_releases_count_without_taking_the_native_gate() {
    let process = test_registry();
    let device = process.device(0).unwrap();
    let primary = device.record.primary.clone();
    let _busy = primary.gate.lock();
    let module = ModuleOwner {
        artifact: PtxArtifact::new(c"test"),
        handle: Mutex::new(ptr::null_mut()),
        primary: ManuallyDrop::new(primary.clone()),
        slot: ManuallyDrop::new(process.owner.modules.try_acquire().unwrap()),
    };
    assert_eq!(process.owner.usage().modules[0], 1);
    drop(module);
    assert_eq!(process.owner.usage().modules[0], 0);
}
