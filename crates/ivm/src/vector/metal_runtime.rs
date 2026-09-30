//! One exact physical lease for discovery, calibration, execution and acceptance.

use super::*;
use metal_owner::{DeviceLease, DeviceRegistry, DiscoveryGate, FairProgress, HealthLease};
use std::{alloc::Layout, cell::RefCell};

static DEVICES: DeviceRegistry<MetalState> = DeviceRegistry::new();
static DISCOVERY: DiscoveryGate = DiscoveryGate::new();
static DISCOVERY_PROGRESS: FairProgress = FairProgress::new();
static MERKLE_PROGRESS: FairProgress = FairProgress::new();
static BATCH_PROGRESS: [FairProgress; metal_cost::BATCH_FAMILIES] =
    [const { FairProgress::new() }; metal_cost::BATCH_FAMILIES];
const CALIBRATION_BUDGET: Duration = Duration::from_secs(8);
const DISCOVERY_RETRY: Duration = Duration::from_secs(30);
const DISCOVERY_BUDGET: Duration = Duration::from_secs(1);
thread_local! {
    static ACTIVE: RefCell<Option<DeviceLease<MetalState>>> = const { RefCell::new(None) };
    static HEALTH: RefCell<Option<HealthLease>> = const { RefCell::new(None) };
}

fn limit() -> usize {
    let config = crate::acceleration_config();
    config
        .max_gpus
        .unwrap_or(config.resource_limits.devices)
        .min(config.resource_limits.devices)
}

pub(super) fn current_health() -> Option<HealthLease> {
    HEALTH.with(|slot| slot.borrow().clone())
}

fn physical_limit() -> usize {
    crate::acceleration_config().resource_limits.devices
}
fn record_allowed(lease: &DeviceLease<MetalState>) -> bool {
    DEVICES.eligible(lease, physical_limit(), limit())
}
pub(super) fn current_allowed() -> bool {
    ACTIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_none_or(|lease| record_allowed(lease) && lease.health().usable())
    })
}

fn bind<R>(lease: &DeviceLease<MetalState>, call: impl FnOnce() -> R) -> R {
    ACTIVE.with(|state| {
        HEALTH.with(|health| metal_owner::with_device_binding(state, health, lease, call))
    })
}

fn discover() {
    if !metal_policy_enabled() || limit() == 0 || METAL_ARTIFACT_INVALID.load(Ordering::Acquire) {
        return;
    }
    let now = Instant::now();
    let Some(_pass) = DISCOVERY.try_enter(now, DISCOVERY_RETRY) else {
        return;
    };
    let config = crate::acceleration_config();
    let resources = iroha_accel::ProcessResources::get_or_initialize(config.resource_limits);
    if !DEVICES.prepare(config.resource_limits.devices, |layout| {
        resources.try_consumer_metadata(layout)
    }) {
        return;
    }
    warm_up_core_graphics_display();
    // Register the complete bounded inventory before spending time qualifying.
    // A slow first device must not conceal later devices from required evidence.
    let devices = objc2_metal::MTLCopyAllDevices();
    let fallback = devices
        .iter()
        .next()
        .is_none()
        .then(|| objc2_metal::MTLCreateSystemDefaultDevice())
        .flatten();
    let complete = devices.len() <= config.resource_limits.discovery_ordinals as usize;
    DEVICES.reconcile_presence(physical_limit(), complete, |identity| {
        devices.iter().any(|device| device.registryID() == identity)
            || fallback
                .as_ref()
                .is_some_and(|device| device.registryID() == identity)
    });
    let observed = || {
        devices
            .iter()
            .chain(fallback.iter().cloned())
            .take(config.resource_limits.discovery_ordinals as usize)
    };
    for device in observed() {
        if !metal_policy_enabled() || limit() == 0 {
            return;
        }
        let _ = DEVICES.observe(device.registryID(), physical_limit(), |bytes| {
            resources.try_consumer_metadata(Layout::array::<u8>(bytes).ok()?)
        });
    }
    let count = DEVICES.len().min(physical_limit());
    let Some(mut progress) = DISCOVERY_PROGRESS.try_pass(count, now, DISCOVERY_BUDGET) else {
        return;
    };
    DEVICES.qualify_fair(
        &mut progress,
        physical_limit(),
        limit(),
        Instant::now,
        |record| {
            if !metal_policy_enabled() || !record_allowed(record) {
                return None;
            }
            let device =
                observed().find(|device| device.registryID() == record.health().identity())?;
            bind(record, || MetalState::new(device))
        },
    );
}

#[cfg(all(test, feature = "metal-hardware-tests"))]
fn qualify(record: &DeviceLease<MetalState>, device: &ProtocolObject<dyn MTLDevice>) {
    use objc2::Message;

    if record.value().is_some() || !record.health().usable() || !record_allowed(record) {
        return;
    }
    // Golden-vector native calls carry physical health before MetalState exists.
    bind(record, || {
        record.initialize(|| MetalState::new(device.retain()));
    });
}

pub(super) fn all_quarantined() -> bool {
    DEVICES.all_quarantined(physical_limit())
}

pub(super) fn with_state<R>(call: impl FnOnce(&MetalState) -> R) -> Option<R> {
    if !metal_runtime_allowed() {
        return None;
    }
    if let Some(lease) = ACTIVE.with(|slot| slot.borrow().clone()) {
        let state = lease.value()?;
        // The operation checks its exact command owner before publishing bytes.
        // Never turn a completed caller copy into a refusal afterwards.
        return Some(call(state));
    }
    discover();
    for index in 0..DEVICES.len().min(physical_limit()) {
        let Some(lease) = DEVICES.record(index, physical_limit()) else {
            continue;
        };
        if !record_allowed(&lease) {
            continue;
        }
        let Some(state) = lease.value() else {
            continue;
        };
        return bind(&lease, || Some(call(state)));
    }
    None
}

/// The private token keeps measured geometry and physical execution inseparable.
pub(crate) struct MetalSelection {
    lease: DeviceLease<MetalState>,
}
impl MetalSelection {
    pub(crate) fn run<R>(self, call: impl FnOnce() -> R) -> Option<R> {
        if !metal_runtime_allowed() || !record_allowed(&self.lease) || self.lease.value().is_none()
        {
            return None;
        }
        bind(&self.lease, || Some(call()))
    }
}

fn select(
    progress: &FairProgress,
    cost: impl Fn(&MetalState, &mut dyn FnMut() -> Option<Instant>) -> Option<u64>,
) -> Option<MetalSelection> {
    if !metal_runtime_allowed() {
        return None;
    }
    let active = ACTIVE.with(|slot| slot.borrow().clone());
    if active.is_none() {
        discover();
    }
    let count = DEVICES.len().min(physical_limit());
    let mut pass = progress.try_pass(count, Instant::now(), CALIBRATION_BUDGET);
    let start = pass.as_ref().map_or(0, metal_owner::FairPass::start);
    if let Some(lease) = active {
        let state = lease.value()?;
        let index = (0..count).find(|&index| {
            DEVICES
                .record(index, physical_limit())
                .is_some_and(|other| iroha_allocation::ChargedShared::ptr_eq(&lease, &other))
        })?;
        cost(state, &mut || {
            pass.as_mut()?.begin_attempt(index, Instant::now())
        })?;
        return record_allowed(&lease).then_some(MetalSelection { lease });
    }
    let lease =
        DEVICES.select_costed(physical_limit(), limit(), start, |index, lease, state| {
            bind(lease, || {
                cost(state, &mut || {
                    pass.as_mut()?.begin_attempt(index, Instant::now())
                })
            })
        })?;
    Some(MetalSelection { lease })
}

/// Retain the current exact qualified owner through delayed output acceptance.
pub(super) fn current_selection() -> Option<MetalSelection> {
    let lease = ACTIVE.with(|slot| slot.borrow().clone())?;
    if !record_allowed(&lease) || lease.value().is_none() {
        return None;
    }
    Some(MetalSelection { lease })
}

pub(super) fn select_merkle(work: MetalMerkleWork, leaves: usize) -> Option<MetalSelection> {
    select(&MERKLE_PROGRESS, |state, begin| {
        let mut cache = state.merkle_cost.try_lock().ok()?;
        cache
            .get_or_calibrate(Instant::now(), || {
                metal_cost::calibrate(begin().ok_or(metal_cost::CalibrationFailure::Deferred)?)
            })?
            .qualified_cost(work, leaves)
    })
}

pub(super) fn select_batch(
    work: metal_cost::MetalBatchWork,
    items: usize,
) -> Option<MetalSelection> {
    if !work.calibration_supported() || items < work.min_items() {
        return None;
    }
    select(&BATCH_PROGRESS[work.family_index()], |state, begin| {
        if matches!(work, metal_cost::MetalBatchWork::Ed25519) && state.ed25519_signature.is_none()
        {
            return None;
        }
        let mut cache = state.batch_cost[work.family_index()].try_lock().ok()?;
        cache
            .get_or_calibrate(Instant::now(), work, || {
                metal_cost::calibrate_batch(
                    work,
                    begin().ok_or(metal_cost::CalibrationFailure::Deferred)?,
                )
            })?
            .qualified_cost(items)
    })
}

pub(super) fn restart_discovery() {
    DISCOVERY.restart();
}

/// Physical identity, qualification and custody are retained on operator opt-out.
pub(super) fn release() {
    restart_discovery();
}

#[cfg(all(test, feature = "metal-hardware-tests"))]
pub(super) fn device_slots() -> Option<usize> {
    if !metal_policy_enabled() || limit() == 0 {
        return None;
    }
    discover();
    let devices = objc2_metal::MTLCopyAllDevices();
    let fallback = devices
        .iter()
        .next()
        .is_none()
        .then(|| objc2_metal::MTLCreateSystemDefaultDevice())
        .flatten();
    let count = DEVICES.len().min(physical_limit());
    let mut expected = 0usize;
    // Qualification starts in an isolated process: every allowed physical
    // identity must have a charged record, even if its initialization is pending.
    // Budget pressure therefore refuses evidence rather than shrinking its census.
    for device in devices.iter().chain(fallback.iter().cloned()).take(
        physical_limit().min(
            crate::acceleration_config()
                .resource_limits
                .discovery_ordinals as usize,
        ),
    ) {
        if !(0..count).any(|index| {
            DEVICES
                .record(index, physical_limit())
                .is_some_and(|record| record.health().identity() == device.registryID())
        }) {
            return None;
        }
        expected += 1;
    }
    (count == expected).then_some(count)
}
#[cfg(all(test, feature = "metal-hardware-tests"))]
pub(super) fn with_device_for_qualification<R>(
    index: usize,
    call: impl FnOnce() -> R,
) -> Option<R> {
    if !metal_policy_enabled() {
        return None;
    }
    discover();
    let lease = DEVICES.record(index, physical_limit())?;
    if lease.value().is_none() {
        // Required qualification explicitly visits every recorded physical device;
        // ordinary discovery's bounded warm-up cannot turn missing work into a pass.
        let devices = objc2_metal::MTLCopyAllDevices();
        let fallback = devices
            .iter()
            .next()
            .is_none()
            .then(|| objc2_metal::MTLCreateSystemDefaultDevice())
            .flatten();
        for device in devices.iter().chain(fallback.iter().cloned()) {
            if device.registryID() == lease.health().identity() {
                qualify(&lease, &device);
                break;
            }
        }
    }
    lease.value()?;
    Some(bind(&lease, call))
}
