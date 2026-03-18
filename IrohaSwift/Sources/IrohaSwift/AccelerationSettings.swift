import Foundation

/// Controls hardware acceleration options for the IVM bridge.
/// Defaults mirror the Rust `AccelerationConfig` behaviour (Metal on, CUDA off).
public struct AccelerationSettings: Codable, Sendable {
    public var enableMetal: Bool
    public var enableCUDA: Bool
    public var maxGPUs: Int?
    public var merkleMinLeavesGPU: Int?
    public var enableSIMD: Bool
    public var merkleMinLeavesMetal: Int?
    public var merkleMinLeavesCUDA: Int?
    public var preferCpuSha2MaxLeavesAarch64: Int?
    public var preferCpuSha2MaxLeavesX86: Int?

    public init(enableSIMD: Bool = true,
                enableMetal: Bool = true,
                enableCUDA: Bool = false,
                maxGPUs: Int? = nil,
                merkleMinLeavesGPU: Int? = nil,
                merkleMinLeavesMetal: Int? = nil,
                merkleMinLeavesCUDA: Int? = nil,
                preferCpuSha2MaxLeavesAarch64: Int? = nil,
                preferCpuSha2MaxLeavesX86: Int? = nil) {
        self.enableSIMD = enableSIMD
        self.enableMetal = enableMetal
        self.enableCUDA = enableCUDA
        self.maxGPUs = (maxGPUs ?? -1) >= 0 ? maxGPUs : nil
        self.merkleMinLeavesGPU = (merkleMinLeavesGPU ?? -1) >= 0 ? merkleMinLeavesGPU : nil
        self.merkleMinLeavesMetal = (merkleMinLeavesMetal ?? -1) >= 0 ? merkleMinLeavesMetal : nil
        self.merkleMinLeavesCUDA = (merkleMinLeavesCUDA ?? -1) >= 0 ? merkleMinLeavesCUDA : nil
        self.preferCpuSha2MaxLeavesAarch64 = (preferCpuSha2MaxLeavesAarch64 ?? -1) >= 0 ? preferCpuSha2MaxLeavesAarch64 : nil
        self.preferCpuSha2MaxLeavesX86 = (preferCpuSha2MaxLeavesX86 ?? -1) >= 0 ? preferCpuSha2MaxLeavesX86 : nil
    }

    /// Applies the settings to the shared native bridge.
    /// If the bridge does not expose acceleration configuration yet, this call is a no-op.
    public func apply() {
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
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let enableMetal = try container.decodeIfPresent(Bool.self, forKey: .enableMetal) ?? true
        let enableCUDA = try container.decodeIfPresent(Bool.self, forKey: .enableCUDA) ?? false
        let maxGPUs = try container.decodeIfPresent(Int.self, forKey: .maxGPUs)
        let merkleMinLeavesGPU = try container.decodeIfPresent(Int.self, forKey: .merkleMinLeavesGPU)
        let merkleMinLeavesMetal = try container.decodeIfPresent(Int.self, forKey: .merkleMinLeavesMetal)
        let merkleMinLeavesCUDA = try container.decodeIfPresent(Int.self, forKey: .merkleMinLeavesCUDA)
        let preferCpuSha2MaxLeavesAarch64 = try container.decodeIfPresent(Int.self, forKey: .preferCpuSha2MaxLeavesAarch64)
        let preferCpuSha2MaxLeavesX86 = try container.decodeIfPresent(Int.self, forKey: .preferCpuSha2MaxLeavesX86)

        self.init(enableSIMD: try container.decodeIfPresent(Bool.self, forKey: .enableSIMD) ?? true,
                  enableMetal: enableMetal,
                  enableCUDA: enableCUDA,
                  maxGPUs: maxGPUs,
                  merkleMinLeavesGPU: merkleMinLeavesGPU,
                  merkleMinLeavesMetal: merkleMinLeavesMetal,
                  merkleMinLeavesCUDA: merkleMinLeavesCUDA,
                  preferCpuSha2MaxLeavesAarch64: preferCpuSha2MaxLeavesAarch64,
                  preferCpuSha2MaxLeavesX86: preferCpuSha2MaxLeavesX86)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(enableSIMD, forKey: .enableSIMD)
        try container.encode(enableMetal, forKey: .enableMetal)
        try container.encode(enableCUDA, forKey: .enableCUDA)
        try container.encodeIfPresent(maxGPUs, forKey: .maxGPUs)
        try container.encodeIfPresent(merkleMinLeavesGPU, forKey: .merkleMinLeavesGPU)
        try container.encodeIfPresent(merkleMinLeavesMetal, forKey: .merkleMinLeavesMetal)
        try container.encodeIfPresent(merkleMinLeavesCUDA, forKey: .merkleMinLeavesCUDA)
        try container.encodeIfPresent(preferCpuSha2MaxLeavesAarch64, forKey: .preferCpuSha2MaxLeavesAarch64)
        try container.encodeIfPresent(preferCpuSha2MaxLeavesX86, forKey: .preferCpuSha2MaxLeavesX86)
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

public extension AccelerationSettings {
    /// Decode settings from the JSON payload used by `iroha_config`.
    /// Missing fields fall back to the Swift defaults while negative thresholds are ignored.
    static func fromJSON(_ data: Data, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        decoder.keyDecodingStrategy = .useDefaultKeys
        let decoded = try decoder.decode(AccelerationSettings.self, from: data)
        return decoded.normalizedForConfigSemantics()
    }

    /// Convenience helper for loading settings from a JSON file on disk.
    static func fromJSONFile(at url: URL, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        let data = try Data(contentsOf: url)
        return try fromJSON(data, decoder: decoder)
    }

    /// Load acceleration settings from an `iroha_config` document (JSON or TOML).
    /// The loader searches for an `accel`/`acceleration` section wherever it appears.
    /// When no section or fields are present the default settings are returned.
    static func fromIrohaConfig(_ data: Data, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        if let settings = try decodeFromIrohaConfigJSON(data, decoder: decoder) {
            return settings
        }
        if let settings = try decodeFromIrohaConfigTOML(data, decoder: decoder) {
            return settings
        }
        return AccelerationSettings()
    }

    /// Convenience helper for loading acceleration settings from an `iroha_config` file on disk.
    static func fromIrohaConfigFile(at url: URL, decoder: JSONDecoder = JSONDecoder()) throws -> AccelerationSettings {
        let data = try Data(contentsOf: url)
        return try fromIrohaConfig(data, decoder: decoder)
    }
}

#if canImport(Darwin)
public extension AccelerationSettings {
    /// Returns the last applied acceleration configuration from the native bridge, if available.
    static func currentAppliedSettings() -> AccelerationSettings? {
        NoritoNativeBridge.shared.currentAccelerationSettings()
    }

    /// Returns the combined acceleration configuration + runtime status, if available.
    static func runtimeState() -> AccelerationState? {
        NoritoNativeBridge.shared.currentAccelerationState()
    }
}
#endif

private extension AccelerationSettings {
    static let accelerationKeys: Set<String> = [
        "enable_metal",
        "enable_cuda",
        "max_gpus",
        "merkle_min_leaves_gpu",
        "merkle_min_leaves_metal",
        "merkle_min_leaves_cuda",
        "prefer_cpu_sha2_max_leaves_aarch64",
        "prefer_cpu_sha2_max_leaves_x86"
    ]

    static func decodeFromIrohaConfigJSON(_ data: Data, decoder: JSONDecoder) throws -> AccelerationSettings? {
        guard let jsonObject = try? JSONSerialization.jsonObject(with: data) else {
            return nil
        }
        if let dictionary = jsonObject as? [String: Any],
           let direct = try decodeAccelerationDictionary(dictionary, decoder: decoder) {
            return direct
        }
        return try searchAccelerationSection(in: jsonObject, decoder: decoder)
    }

    static func decodeFromIrohaConfigTOML(_ data: Data, decoder: JSONDecoder) throws -> AccelerationSettings? {
        guard let text = String(data: data, encoding: .utf8) else {
            return nil
        }
        guard let section = parseTomlAccelerationSection(text) else {
            return nil
        }
        return try decodeAccelerationDictionary(section, decoder: decoder)
    }

    static func searchAccelerationSection(in object: Any, decoder: JSONDecoder) throws -> AccelerationSettings? {
        if let dict = object as? [String: Any] {
            if let accel = dict["accel"] {
                if let found = try searchAccelerationSection(in: accel, decoder: decoder) {
                    return found
                }
            }
            if let accel = dict["acceleration"] {
                if let found = try searchAccelerationSection(in: accel, decoder: decoder) {
                    return found
                }
            }
            if let found = try decodeAccelerationDictionary(dict, decoder: decoder) {
                return found
            }
            for value in dict.values {
                if let found = try searchAccelerationSection(in: value, decoder: decoder) {
                    return found
                }
            }
        } else if let array = object as? [Any] {
            for item in array {
                if let found = try searchAccelerationSection(in: item, decoder: decoder) {
                    return found
                }
            }
        }
        return nil
    }

    static func decodeAccelerationDictionary(_ dict: [String: Any], decoder: JSONDecoder) throws -> AccelerationSettings? {
        guard dict.keys.contains(where: { accelerationKeys.contains($0) }) else {
            return nil
        }
        guard JSONSerialization.isValidJSONObject(dict) else {
            return nil
        }
        let data = try JSONSerialization.data(withJSONObject: dict, options: [])
        decoder.keyDecodingStrategy = .useDefaultKeys
        guard let decoded = try? decoder.decode(AccelerationSettings.self, from: data) else {
            return nil
        }
        return decoded.normalizedForConfigSemantics()
    }

    static func parseTomlAccelerationSection(_ text: String) -> [String: Any]? {
        var inAccelSection = false
        var result: [String: Any] = [:]

        for rawLine in text.split(whereSeparator: \.isNewline) {
            var line = String(rawLine)
            if let hashIndex = line.firstIndex(of: "#") {
                line = String(line[..<hashIndex])
            }
            line = line.trimmingCharacters(in: .whitespacesAndNewlines)
            if line.isEmpty {
                continue
            }
            if line.hasPrefix("[") {
                inAccelSection = line == "[accel]" || line == "[acceleration]"
                continue
            }
            guard inAccelSection, let equalIndex = line.firstIndex(of: "=") else {
                continue
            }
            let key = line[..<equalIndex].trimmingCharacters(in: .whitespacesAndNewlines)
            let valuePortion = line[line.index(after: equalIndex)...].trimmingCharacters(in: .whitespacesAndNewlines)
            guard !key.isEmpty, !valuePortion.isEmpty else { continue }

            if valuePortion == "true" {
                result[key] = true
            } else if valuePortion == "false" {
                result[key] = false
            } else if let intValue = parseTomlInteger(valuePortion) {
                result[key] = intValue
            } else if valuePortion.hasPrefix("\""), valuePortion.hasSuffix("\"") {
                let trimmed = valuePortion.dropFirst().dropLast()
                result[key] = String(trimmed)
            }
        }

        return result.isEmpty ? nil : result
    }

    static func parseTomlInteger(_ value: String) -> Int? {
        let sanitized = value.replacingOccurrences(of: "_", with: "")
        return Int(sanitized, radix: 10)
    }

    func normalizedForConfigSemantics() -> AccelerationSettings {
        var copy = self
        copy.maxGPUs = AccelerationSettings.normalizeOptionalInt(copy.maxGPUs)
        copy.merkleMinLeavesGPU = AccelerationSettings.normalizeOptionalInt(copy.merkleMinLeavesGPU)
        copy.merkleMinLeavesMetal = AccelerationSettings.normalizeOptionalInt(copy.merkleMinLeavesMetal)
        copy.merkleMinLeavesCUDA = AccelerationSettings.normalizeOptionalInt(copy.merkleMinLeavesCUDA)
        copy.preferCpuSha2MaxLeavesAarch64 = AccelerationSettings.normalizeOptionalInt(copy.preferCpuSha2MaxLeavesAarch64)
        copy.preferCpuSha2MaxLeavesX86 = AccelerationSettings.normalizeOptionalInt(copy.preferCpuSha2MaxLeavesX86)
        return copy
    }

    static func normalizeOptionalInt(_ value: Int?) -> Int? {
        guard let value else { return nil }
        return value > 0 ? value : nil
    }
}

#if canImport(Darwin)
extension AccelerationSettings {
    init(nativeConfig config: ConnectNoritoAccelerationConfig) {
        func decodeOptional(present: UInt8, value: UInt64) -> Int? {
            present != 0 ? Int(value) : nil
        }
        let settings = AccelerationSettings(
            enableSIMD: config.enable_simd != 0,
            enableMetal: config.enable_metal != 0,
            enableCUDA: config.enable_cuda != 0,
            maxGPUs: decodeOptional(present: config.max_gpus_present, value: config.max_gpus),
            merkleMinLeavesGPU: decodeOptional(present: config.merkle_min_leaves_gpu_present, value: config.merkle_min_leaves_gpu),
            merkleMinLeavesMetal: decodeOptional(present: config.merkle_min_leaves_metal_present, value: config.merkle_min_leaves_metal),
            merkleMinLeavesCUDA: decodeOptional(present: config.merkle_min_leaves_cuda_present, value: config.merkle_min_leaves_cuda),
            preferCpuSha2MaxLeavesAarch64: decodeOptional(present: config.prefer_cpu_sha2_max_leaves_aarch64_present,
                                                          value: config.prefer_cpu_sha2_max_leaves_aarch64),
            preferCpuSha2MaxLeavesX86: decodeOptional(present: config.prefer_cpu_sha2_max_leaves_x86_present,
                                                      value: config.prefer_cpu_sha2_max_leaves_x86)
        )
        self = settings.normalizedForConfigSemantics()
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
