import Foundation

/// Finite process acceleration ceilings, distinct from transaction execution credit.
/// Zero is an explicit ceiling. Defaults match `iroha_config` and the native owner.
public struct AccelerationResourceLimits: Codable, Sendable, Equatable {
    public var hostBytes: UInt64
    public var pinnedBytes: UInt64
    public var deviceBytes: UInt64
    public var inFlight: UInt64
    public var metadataBytes: UInt64
    public var observedDevices: UInt64
    public var discoveryOrdinals: UInt32
    public var modules: UInt64
    public var streams: UInt64
    public var artifactBytes: UInt64

    public init(hostBytes: UInt64 = 268_435_456,
                pinnedBytes: UInt64 = 268_435_456,
                deviceBytes: UInt64 = 1_073_741_824,
                inFlight: UInt64 = 16,
                metadataBytes: UInt64 = 16_777_216,
                observedDevices: UInt64 = 16,
                discoveryOrdinals: UInt32 = 64,
                modules: UInt64 = 304,
                streams: UInt64 = 16,
                artifactBytes: UInt64 = 16_777_216) {
        self.hostBytes = hostBytes
        self.pinnedBytes = pinnedBytes
        self.deviceBytes = deviceBytes
        self.inFlight = inFlight
        self.metadataBytes = metadataBytes
        self.observedDevices = observedDevices
        self.discoveryOrdinals = discoveryOrdinals
        self.modules = modules
        self.streams = streams
        self.artifactBytes = artifactBytes
    }

    private enum CodingKeys: String, CodingKey {
        case hostBytes = "host_bytes"
        case pinnedBytes = "pinned_bytes"
        case deviceBytes = "device_bytes"
        case inFlight = "in_flight"
        case metadataBytes = "metadata_bytes"
        case observedDevices = "observed_devices"
        case discoveryOrdinals = "discovery_ordinals"
        case modules = "modules"
        case streams = "streams"
        case artifactBytes = "artifact_bytes"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let defaults = Self()
        self.init(
            hostBytes: values.contains(.hostBytes) ? try values.decode(UInt64.self, forKey: .hostBytes) : defaults.hostBytes,
            pinnedBytes: values.contains(.pinnedBytes) ? try values.decode(UInt64.self, forKey: .pinnedBytes) : defaults.pinnedBytes,
            deviceBytes: values.contains(.deviceBytes) ? try values.decode(UInt64.self, forKey: .deviceBytes) : defaults.deviceBytes,
            inFlight: values.contains(.inFlight) ? try values.decode(UInt64.self, forKey: .inFlight) : defaults.inFlight,
            metadataBytes: values.contains(.metadataBytes) ? try values.decode(UInt64.self, forKey: .metadataBytes) : defaults.metadataBytes,
            observedDevices: values.contains(.observedDevices) ? try values.decode(UInt64.self, forKey: .observedDevices) : defaults.observedDevices,
            discoveryOrdinals: values.contains(.discoveryOrdinals) ? try values.decode(UInt32.self, forKey: .discoveryOrdinals) : defaults.discoveryOrdinals,
            modules: values.contains(.modules) ? try values.decode(UInt64.self, forKey: .modules) : defaults.modules,
            streams: values.contains(.streams) ? try values.decode(UInt64.self, forKey: .streams) : defaults.streams,
            artifactBytes: values.contains(.artifactBytes) ? try values.decode(UInt64.self, forKey: .artifactBytes) : defaults.artifactBytes
        )
    }
}
