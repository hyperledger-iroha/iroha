import Foundation

/// Capability-selected acceleration policy shared with the native process owner.
/// All backends are enabled by default; unsupported devices use qualified CPU execution.
/// Optional counts inherit native defaults only when absent. Zero stays explicit.
public struct AccelerationSettings: Codable, Sendable {
    public var enableSIMD: Bool
    public var enableMetal: Bool
    public var enableCUDA: Bool
    public var maxGPUs: UInt64?
    public var merkleMinLeavesGPU: UInt64?
    public var merkleMinLeavesMetal: UInt64?
    public var merkleMinLeavesCUDA: UInt64?
    public var preferCpuSha2MaxLeavesAarch64: UInt64?
    public var preferCpuSha2MaxLeavesX86: UInt64?
    public var resourceLimits: AccelerationResourceLimits

    public init(enableSIMD: Bool = true,
                enableMetal: Bool = true,
                enableCUDA: Bool = true,
                maxGPUs: UInt64? = nil,
                merkleMinLeavesGPU: UInt64? = nil,
                merkleMinLeavesMetal: UInt64? = nil,
                merkleMinLeavesCUDA: UInt64? = nil,
                preferCpuSha2MaxLeavesAarch64: UInt64? = nil,
                preferCpuSha2MaxLeavesX86: UInt64? = nil,
                resourceLimits: AccelerationResourceLimits = AccelerationResourceLimits()) {
        self.enableSIMD = enableSIMD
        self.enableMetal = enableMetal
        self.enableCUDA = enableCUDA
        self.maxGPUs = maxGPUs
        self.merkleMinLeavesGPU = merkleMinLeavesGPU
        self.merkleMinLeavesMetal = merkleMinLeavesMetal
        self.merkleMinLeavesCUDA = merkleMinLeavesCUDA
        self.preferCpuSha2MaxLeavesAarch64 = preferCpuSha2MaxLeavesAarch64
        self.preferCpuSha2MaxLeavesX86 = preferCpuSha2MaxLeavesX86
        self.resourceLimits = resourceLimits
    }

    /// Apply policy to the shared native owner; return whether it accepted the request.
    /// An unavailable bridge returns false. Inspect runtime state for device qualification.
    @discardableResult
    public func apply() -> Bool {
        NoritoNativeBridge.shared.applyAccelerationSettings(self)
    }

    private enum CodingKeys: String, CodingKey {
        case enableSIMD = "enable_simd"
        case enableMetal = "enable_metal"
        case enableCUDA = "enable_cuda"
        case maxGPUs = "max_gpus"
        case merkleMinLeavesGPU = "merkle_min_leaves_gpu"
        case merkleMinLeavesMetal = "merkle_min_leaves_metal"
        case merkleMinLeavesCUDA = "merkle_min_leaves_cuda"
        case preferCpuSha2MaxLeavesAarch64 = "prefer_cpu_sha2_max_leaves_aarch64"
        case preferCpuSha2MaxLeavesX86 = "prefer_cpu_sha2_max_leaves_x86"
        case resourceLimits = "resource_limits"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            enableSIMD: values.contains(.enableSIMD) ? try values.decode(Bool.self, forKey: .enableSIMD) : true,
            enableMetal: values.contains(.enableMetal) ? try values.decode(Bool.self, forKey: .enableMetal) : true,
            enableCUDA: values.contains(.enableCUDA) ? try values.decode(Bool.self, forKey: .enableCUDA) : true,
            maxGPUs: try values.decodeIfPresent(UInt64.self, forKey: .maxGPUs),
            merkleMinLeavesGPU: try values.decodeIfPresent(UInt64.self, forKey: .merkleMinLeavesGPU),
            merkleMinLeavesMetal: try values.decodeIfPresent(UInt64.self, forKey: .merkleMinLeavesMetal),
            merkleMinLeavesCUDA: try values.decodeIfPresent(UInt64.self, forKey: .merkleMinLeavesCUDA),
            preferCpuSha2MaxLeavesAarch64: try values.decodeIfPresent(UInt64.self, forKey: .preferCpuSha2MaxLeavesAarch64),
            preferCpuSha2MaxLeavesX86: try values.decodeIfPresent(UInt64.self, forKey: .preferCpuSha2MaxLeavesX86),
            resourceLimits: values.contains(.resourceLimits) ? try values.decode(AccelerationResourceLimits.self, forKey: .resourceLimits) : AccelerationResourceLimits()
        )
    }
}

/// Runtime information for a single acceleration backend.
public struct AccelerationBackendStatus: Sendable {
    public let supported: Bool
    public let configured: Bool
    public let available: Bool
    public let parityOK: Bool
    public let lastError: String?

    public init(supported: Bool,
                configured: Bool,
                available: Bool,
                parityOK: Bool,
                lastError: String?) {
        self.supported = supported
        self.configured = configured
        self.available = available
        self.parityOK = parityOK
        self.lastError = lastError
    }
}

/// Combined configuration + runtime status for acceleration backends.
public struct AccelerationState: Sendable {
    public let settings: AccelerationSettings
    public let simd: AccelerationBackendStatus
    public let metal: AccelerationBackendStatus
    public let cuda: AccelerationBackendStatus

    public init(settings: AccelerationSettings,
                simd: AccelerationBackendStatus,
                metal: AccelerationBackendStatus,
                cuda: AccelerationBackendStatus) {
        self.settings = settings
        self.simd = simd
        self.metal = metal
        self.cuda = cuda
    }
}

