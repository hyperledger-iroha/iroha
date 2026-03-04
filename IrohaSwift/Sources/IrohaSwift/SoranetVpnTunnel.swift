import Foundation
#if canImport(NetworkExtension)
import NetworkExtension
#endif

public enum SoranetVpnError: Error {
    case invalidCircuitId
    case payloadTooLarge(max: Int, actual: Int)
}

public struct SoranetVpnCellHeader {
    public static let cellSize = 1024
    public static let headerSize = 42

    public var version: UInt8
    public var klass: UInt8
    public var flags: UInt8
    public var circuitId: Data
    public var flowLabel: UInt32
    public var sequence: UInt64
    public var ack: UInt64
    public var paddingBudgetMs: UInt16

    public init(
        circuitId: Data,
        flowLabel: UInt32,
        sequence: UInt64,
        ack: UInt64,
        klass: UInt8,
        flags: UInt8,
        paddingBudgetMs: UInt16
    ) throws {
        guard circuitId.count == 16 else {
            throw SoranetVpnError.invalidCircuitId
        }
        self.version = 1
        self.klass = klass
        self.flags = flags
        self.circuitId = circuitId
        self.flowLabel = flowLabel & 0x00FF_FFFF
        self.sequence = sequence
        self.ack = ack
        self.paddingBudgetMs = paddingBudgetMs
    }

    public func encode(payload: Data) throws -> Data {
        let maxPayload = SoranetVpnCellHeader.cellSize - SoranetVpnCellHeader.headerSize
        guard payload.count <= maxPayload else {
            throw SoranetVpnError.payloadTooLarge(max: maxPayload, actual: payload.count)
        }

        var frame = Data(count: SoranetVpnCellHeader.cellSize)
        var cursor = 0
        frame[cursor] = version
        cursor += 1
        frame[cursor] = klass
        cursor += 1
        frame[cursor] = flags
        cursor += 1
        frame.replaceSubrange(cursor..<(cursor + 16), with: circuitId)
        cursor += 16

        let flowBytes: [UInt8] = [
            UInt8((flowLabel >> 16) & 0xFF),
            UInt8((flowLabel >> 8) & 0xFF),
            UInt8(flowLabel & 0xFF),
        ]
        frame.replaceSubrange(cursor..<(cursor + 3), with: flowBytes)
        cursor += 3

        var seq = sequence.bigEndian
        withUnsafeBytes(of: &seq) { frame.replaceSubrange(cursor..<(cursor + 8), with: $0) }
        cursor += 8
        var ackValue = ack.bigEndian
        withUnsafeBytes(of: &ackValue) {
            frame.replaceSubrange(cursor..<(cursor + 8), with: $0)
        }
        cursor += 8
        var padding = paddingBudgetMs.bigEndian
        withUnsafeBytes(of: &padding) {
            frame.replaceSubrange(cursor..<(cursor + 2), with: $0)
        }
        cursor += 2

        var payloadLen = UInt16(payload.count).bigEndian
        withUnsafeBytes(of: &payloadLen) {
            frame.replaceSubrange(cursor..<(cursor + 2), with: $0)
        }
        cursor += 2

        frame.replaceSubrange(cursor..<(cursor + payload.count), with: payload)
        return frame
    }
}

public final class SoranetVpnTunnel {
    public static let cellSize = SoranetVpnCellHeader.cellSize

    public let circuitId: Data
    public var paddingBudgetMs: UInt16

    public init(circuitId: Data, paddingBudgetMs: UInt16 = 15) throws {
        guard circuitId.count == 16 else {
            throw SoranetVpnError.invalidCircuitId
        }
        self.circuitId = circuitId
        self.paddingBudgetMs = paddingBudgetMs
    }

    public func frame(
        payload: Data,
        sequence: UInt64,
        ack: UInt64 = 0,
        flowLabel: UInt32 = 0,
        cover: Bool = false
    ) throws -> Data {
        let header = try SoranetVpnCellHeader(
            circuitId: circuitId,
            flowLabel: flowLabel,
            sequence: sequence,
            ack: ack,
            klass: 0, // Data
            flags: cover ? 0x01 : 0x00,
            paddingBudgetMs: paddingBudgetMs
        )
        return try header.encode(payload: payload)
    }

    #if canImport(NetworkExtension)
    public func networkSettings(dns: [String], routes: [String]) -> NEPacketTunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "127.0.0.1")
        settings.mtu = NSNumber(value: 1280)

        let ipv4Settings = NEIPv4Settings(
            addresses: ["10.0.0.2"],
            subnetMasks: ["255.255.255.0"]
        )
        ipv4Settings.includedRoutes = routes.map { NEIPv4Route(destinationAddress: $0, subnetMask: "255.255.255.255") }
        settings.ipv4Settings = ipv4Settings
        settings.dnsSettings = NEDNSSettings(servers: dns)
        return settings
    }
    #endif
}
