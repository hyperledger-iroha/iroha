import Foundation

// These records mirror the single native C layout. Native target tests must
// compare their sizes, offsets, and raw readback with the rebuilt bridge.
#if canImport(Darwin)
struct ConnectNoritoAccelerationResourceLimits {
    var host_bytes: UInt64
    var pinned_bytes: UInt64
    var device_bytes: UInt64
    var in_flight: UInt64
    var metadata_bytes: UInt64
    var observed_devices: UInt64
    var discovery_ordinals: UInt64
    var modules: UInt64
    var streams: UInt64
    var artifact_bytes: UInt64
}

struct ConnectNoritoAccelerationConfig {
    var enable_simd: UInt8
    var enable_metal: UInt8
    var enable_cuda: UInt8
    var max_gpus: UInt64
    var max_gpus_present: UInt8
    var merkle_min_leaves_gpu: UInt64
    var merkle_min_leaves_gpu_present: UInt8
    var merkle_min_leaves_metal: UInt64
    var merkle_min_leaves_metal_present: UInt8
    var merkle_min_leaves_cuda: UInt64
    var merkle_min_leaves_cuda_present: UInt8
    var prefer_cpu_sha2_max_leaves_aarch64: UInt64
    var prefer_cpu_sha2_max_leaves_aarch64_present: UInt8
    var prefer_cpu_sha2_max_leaves_x86: UInt64
    var prefer_cpu_sha2_max_leaves_x86_present: UInt8
    var resource_limits: ConnectNoritoAccelerationResourceLimits
}

struct ConnectNoritoAccelerationBackendStatus {
    var supported: UInt8
    var configured: UInt8
    var available: UInt8
    var parity_ok: UInt8
    var last_error_ptr: UnsafeMutablePointer<UInt8>?
    var last_error_len: UInt
}

struct ConnectNoritoAccelerationState {
    var config: ConnectNoritoAccelerationConfig
    var simd: ConnectNoritoAccelerationBackendStatus
    var metal: ConnectNoritoAccelerationBackendStatus
    var cuda: ConnectNoritoAccelerationBackendStatus
}

extension ConnectNoritoAccelerationConfig {
    static var empty: Self {
        Self(enable_simd: 0, enable_metal: 0, enable_cuda: 0,
             max_gpus: 0, max_gpus_present: 0,
             merkle_min_leaves_gpu: 0, merkle_min_leaves_gpu_present: 0,
             merkle_min_leaves_metal: 0, merkle_min_leaves_metal_present: 0,
             merkle_min_leaves_cuda: 0, merkle_min_leaves_cuda_present: 0,
             prefer_cpu_sha2_max_leaves_aarch64: 0, prefer_cpu_sha2_max_leaves_aarch64_present: 0,
             prefer_cpu_sha2_max_leaves_x86: 0, prefer_cpu_sha2_max_leaves_x86_present: 0,
             resource_limits: ConnectNoritoAccelerationResourceLimits(
                 host_bytes: 0,
                 pinned_bytes: 0,
                 device_bytes: 0,
                 in_flight: 0,
                 metadata_bytes: 0,
                 observed_devices: 0,
                 discovery_ordinals: 0,
                 modules: 0,
                 streams: 0,
                 artifact_bytes: 0))
    }
}

extension AccelerationSettings {
    var nativeConfig: ConnectNoritoAccelerationConfig {
        ConnectNoritoAccelerationConfig(
            enable_simd: enableSIMD ? 1 : 0,
            enable_metal: enableMetal ? 1 : 0,
            enable_cuda: enableCUDA ? 1 : 0,
            max_gpus: maxGPUs ?? 0, max_gpus_present: maxGPUs == nil ? 0 : 1,
            merkle_min_leaves_gpu: merkleMinLeavesGPU ?? 0, merkle_min_leaves_gpu_present: merkleMinLeavesGPU == nil ? 0 : 1,
            merkle_min_leaves_metal: merkleMinLeavesMetal ?? 0, merkle_min_leaves_metal_present: merkleMinLeavesMetal == nil ? 0 : 1,
            merkle_min_leaves_cuda: merkleMinLeavesCUDA ?? 0, merkle_min_leaves_cuda_present: merkleMinLeavesCUDA == nil ? 0 : 1,
            prefer_cpu_sha2_max_leaves_aarch64: preferCpuSha2MaxLeavesAarch64 ?? 0, prefer_cpu_sha2_max_leaves_aarch64_present: preferCpuSha2MaxLeavesAarch64 == nil ? 0 : 1,
            prefer_cpu_sha2_max_leaves_x86: preferCpuSha2MaxLeavesX86 ?? 0, prefer_cpu_sha2_max_leaves_x86_present: preferCpuSha2MaxLeavesX86 == nil ? 0 : 1,
            resource_limits: ConnectNoritoAccelerationResourceLimits(
                host_bytes: UInt64(resourceLimits.hostBytes),
                pinned_bytes: UInt64(resourceLimits.pinnedBytes),
                device_bytes: UInt64(resourceLimits.deviceBytes),
                in_flight: UInt64(resourceLimits.inFlight),
                metadata_bytes: UInt64(resourceLimits.metadataBytes),
                observed_devices: UInt64(resourceLimits.observedDevices),
                discovery_ordinals: UInt64(resourceLimits.discoveryOrdinals),
                modules: UInt64(resourceLimits.modules),
                streams: UInt64(resourceLimits.streams),
                artifact_bytes: UInt64(resourceLimits.artifactBytes)))
    }

    init(nativeConfig config: ConnectNoritoAccelerationConfig) {
        self.init(
            enableSIMD: config.enable_simd != 0,
            enableMetal: config.enable_metal != 0,
            enableCUDA: config.enable_cuda != 0,
            maxGPUs: config.max_gpus_present == 0 ? nil : config.max_gpus,
            merkleMinLeavesGPU: config.merkle_min_leaves_gpu_present == 0 ? nil : config.merkle_min_leaves_gpu,
            merkleMinLeavesMetal: config.merkle_min_leaves_metal_present == 0 ? nil : config.merkle_min_leaves_metal,
            merkleMinLeavesCUDA: config.merkle_min_leaves_cuda_present == 0 ? nil : config.merkle_min_leaves_cuda,
            preferCpuSha2MaxLeavesAarch64: config.prefer_cpu_sha2_max_leaves_aarch64_present == 0 ? nil : config.prefer_cpu_sha2_max_leaves_aarch64,
            preferCpuSha2MaxLeavesX86: config.prefer_cpu_sha2_max_leaves_x86_present == 0 ? nil : config.prefer_cpu_sha2_max_leaves_x86,
            resourceLimits: AccelerationResourceLimits(
                hostBytes: config.resource_limits.host_bytes,
                pinnedBytes: config.resource_limits.pinned_bytes,
                deviceBytes: config.resource_limits.device_bytes,
                inFlight: config.resource_limits.in_flight,
                metadataBytes: config.resource_limits.metadata_bytes,
                observedDevices: config.resource_limits.observed_devices,
                discoveryOrdinals: UInt32(config.resource_limits.discovery_ordinals),
                modules: config.resource_limits.modules,
                streams: config.resource_limits.streams,
                artifactBytes: config.resource_limits.artifact_bytes))
    }
}

public extension AccelerationSettings {
    /// Read the native process owner's applied policy.
    static func currentAppliedSettings() -> AccelerationSettings? {
        NoritoNativeBridge.shared.currentAccelerationSettings()
    }

    /// Read backend availability and qualification independently of requested policy.
    static func runtimeState() -> AccelerationState? {
        NoritoNativeBridge.shared.currentAccelerationState()
    }
}

extension AccelerationBackendStatus {
    init(nativeStatus status: ConnectNoritoAccelerationBackendStatus) {
        let message: String?
        if status.last_error_len > 0, let ptr = status.last_error_ptr {
            let count = Int(status.last_error_len)
            let data = Data(bytes: ptr, count: count)
            message = String(data: data, encoding: .utf8) ?? String(decoding: data, as: UTF8.self)
        } else {
            message = nil
        }
        self.init(supported: status.supported != 0,
                  configured: status.configured != 0,
                  available: status.available != 0,
                  parityOK: status.parity_ok != 0,
                  lastError: message)
    }
}

extension AccelerationState {
    init(nativeState state: ConnectNoritoAccelerationState) {
        self.init(settings: AccelerationSettings(nativeConfig: state.config),
                  simd: AccelerationBackendStatus(nativeStatus: state.simd),
                  metal: AccelerationBackendStatus(nativeStatus: state.metal),
                  cuda: AccelerationBackendStatus(nativeStatus: state.cuda))
    }
}
#endif
