import Foundation

/// Capability-specific QR rail for production-sized eligibility payments.
///
/// IQR1 remains byte-for-byte unchanged and is restricted to the legacy
/// `0x0102` rail. IQR2 carries only Kagemusha PAYMENT `0x0103`, uses u32 shard
/// coordinates, and assembles the bounded body in an owner-controlled file.
public enum IrohaPeerQRV2 {
    public static let textPrefix = "IQR2:"
    public static let textSuffix = ":"
    public static let shardBytes = 256
    public static let maximumFrameTextBytes = 700
}

public enum IrohaPeerQRErrorV2: Error, Equatable, Sendable {
    case malformedFrame
    case nonCanonicalBase45
    case wrongCapability
    case messageTooLarge
    case headerRequired
    case conflictingDuplicate
    case storageFailure
    case invalidMessage
}

/// One CRC32C-protected IQR2 frame.
public struct IrohaPeerQRFrameV2: Equatable, Sendable {
    public enum Kind: UInt8, Sendable {
        case header = 1
        case data = 2
    }

    public static let magic = Data("IRQ2".utf8)
    public static let wireVersion: UInt8 = 2
    public static let payloadOffset = 38
    public static let checksumBytes = 4

    public let kind: Kind
    public let streamID: Data
    public let index: UInt32
    public let total: UInt32
    public let payload: Data

    public init(
        kind: Kind,
        streamID: Data,
        index: UInt32,
        total: UInt32,
        payload: Data
    ) throws {
        let maximumShards = UInt32(
            (IrohaPeerWireLimitsV1.maximumKagemushaEligibilityEnvelopeBytes
                + IrohaPeerQRV2.shardBytes - 1) / IrohaPeerQRV2.shardBytes
        )
        guard streamID.count == 16, total > 0, total <= maximumShards else {
            throw IrohaPeerQRErrorV2.messageTooLarge
        }
        switch kind {
        case .header:
            guard index == 0, payload.count == IrohaPeerWireMessageV1.headerBytes else {
                throw IrohaPeerQRErrorV2.malformedFrame
            }
        case .data:
            guard index < total, !payload.isEmpty,
                  payload.count <= IrohaPeerQRV2.shardBytes else {
                throw IrohaPeerQRErrorV2.malformedFrame
            }
        }
        self.kind = kind
        self.streamID = Data(streamID)
        self.index = index
        self.total = total
        self.payload = Data(payload)
    }

    public var encoded: Data {
        var out = Self.magic
        out.append(Self.wireVersion)
        out.append(kind.rawValue)
        out.iqr2AppendUInt16BE(IrohaPeerWireProfileV1.kagemusha.rawValue)
        out.append(IrohaPeerWireKindV1.payment.rawValue)
        out.append(0)
        out.iqr2AppendUInt16BE(
            IrohaPeerWireMessageV1.kagemushaEligibilityPaymentSchemaVersion
        )
        out.append(streamID)
        out.iqr2AppendUInt32BE(index)
        out.iqr2AppendUInt32BE(total)
        out.iqr2AppendUInt16BE(UInt16(payload.count))
        out.append(payload)
        out.iqr2AppendUInt32BE(IrohaPeerCRC32CV1.checksum(out))
        return out
    }

    public var text: String {
        IrohaPeerQRV2.textPrefix + IrohaBase45V1.encode(encoded)
            + IrohaPeerQRV2.textSuffix
    }

    public static func decode(_ data: Data) throws -> Self {
        guard data.count >= payloadOffset + checksumBytes,
              data.prefix(4) == magic,
              data[4] == wireVersion,
              let kind = Kind(rawValue: data[5]),
              data.iqr2UInt16BE(at: 6) == IrohaPeerWireProfileV1.kagemusha.rawValue,
              data[8] == IrohaPeerWireKindV1.payment.rawValue,
              data[9] == 0,
              data.iqr2UInt16BE(at: 10)
                == IrohaPeerWireMessageV1.kagemushaEligibilityPaymentSchemaVersion else {
            throw IrohaPeerQRErrorV2.wrongCapability
        }
        let payloadLength = Int(data.iqr2UInt16BE(at: 36))
        let payloadEnd = payloadOffset + payloadLength
        guard payloadEnd + checksumBytes == data.count,
              data.iqr2UInt32BE(at: payloadEnd)
                == IrohaPeerCRC32CV1.checksum(data.prefix(payloadEnd)) else {
            throw IrohaPeerQRErrorV2.malformedFrame
        }
        return try Self(
            kind: kind,
            streamID: data.subdata(in: 12..<28),
            index: data.iqr2UInt32BE(at: 28),
            total: data.iqr2UInt32BE(at: 32),
            payload: data.subdata(in: payloadOffset..<payloadEnd)
        )
    }

    public static func decode(text: String) throws -> Self {
        guard text.hasPrefix(IrohaPeerQRV2.textPrefix),
              text.hasSuffix(IrohaPeerQRV2.textSuffix),
              text.utf8.count <= IrohaPeerQRV2.maximumFrameTextBytes else {
            throw IrohaPeerQRErrorV2.malformedFrame
        }
        let start = text.index(text.startIndex, offsetBy: IrohaPeerQRV2.textPrefix.count)
        let end = text.index(text.endIndex, offsetBy: -IrohaPeerQRV2.textSuffix.count)
        let body = String(text[start..<end])
        guard let data = IrohaBase45V1.decode(body),
              IrohaBase45V1.encode(data) == body else {
            throw IrohaPeerQRErrorV2.nonCanonicalBase45
        }
        return try decode(data)
    }
}

/// Lazy IQR2 encoder; it never materializes every frame at once.
public struct IrohaPeerQREncoderV2: Sendable {
    private let header: Data
    private let body: Data
    public let streamID: Data
    public let dataShardCount: UInt32

    public init(message: IrohaPeerWireMessageV1) throws {
        guard message.profile == .kagemusha,
              message.kind == .payment,
              message.schemaVersion
                == IrohaPeerWireMessageV1.kagemushaEligibilityPaymentSchemaVersion else {
            throw IrohaPeerQRErrorV2.wrongCapability
        }
        let count = (message.encodedBody.count + IrohaPeerQRV2.shardBytes - 1)
            / IrohaPeerQRV2.shardBytes
        guard count > 0, count <= Int(UInt32.max) else {
            throw IrohaPeerQRErrorV2.messageTooLarge
        }
        header = message.header.bytes
        body = message.encodedBody
        streamID = message.streamID
        dataShardCount = UInt32(count)
    }

    public func headerFrame() throws -> IrohaPeerQRFrameV2 {
        try IrohaPeerQRFrameV2(
            kind: .header,
            streamID: streamID,
            index: 0,
            total: dataShardCount,
            payload: header
        )
    }

    public func dataFrame(index: UInt32) throws -> IrohaPeerQRFrameV2 {
        guard index < dataShardCount else { throw IrohaPeerQRErrorV2.malformedFrame }
        let start = Int(index) * IrohaPeerQRV2.shardBytes
        let end = min(start + IrohaPeerQRV2.shardBytes, body.count)
        return try IrohaPeerQRFrameV2(
            kind: .data,
            streamID: streamID,
            index: index,
            total: dataShardCount,
            payload: body.subdata(in: start..<end)
        )
    }
}

/// File-backed, strictly bounded IQR2 receiver.
///
/// The IPM1 header is authenticated structurally before the body file is
/// created or sized. Only the completion gate materializes the bounded body.
public final class IrohaPeerQRFileAssemblerV2 {
    private let directory: URL
    private let limits: IrohaPeerWireLimitsV1
    private var inspectedHeader: IrohaPeerWireHeaderV1?
    private var headerBytes: Data?
    private var streamID: Data?
    private var total: UInt32 = 0
    private var bitmap = Data()
    private var received: UInt32 = 0
    private var fileURL: URL?
    private var fileHandle: FileHandle?

    public init(
        directory: URL = FileManager.default.temporaryDirectory,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) {
        self.directory = directory
        self.limits = limits
    }

    deinit { cancel() }

    public func accept(_ frame: IrohaPeerQRFrameV2) throws -> IrohaPeerWireMessageV1? {
        switch frame.kind {
        case .header:
            try acceptHeader(frame)
            return nil
        case .data:
            return try acceptData(frame)
        }
    }

    public func cancel() {
        try? fileHandle?.close()
        fileHandle = nil
        if let fileURL { try? FileManager.default.removeItem(at: fileURL) }
        fileURL = nil
        inspectedHeader = nil
        headerBytes = nil
        streamID = nil
        total = 0
        bitmap.removeAll(keepingCapacity: false)
        received = 0
    }

    private func acceptHeader(_ frame: IrohaPeerQRFrameV2) throws {
        let inspected: IrohaPeerWireHeaderV1
        do {
            inspected = try IrohaPeerWireMessageV1.inspectHeader(
                frame.payload,
                expectedProfile: .kagemusha,
                expectedKind: .payment,
                limits: limits
            )
        } catch {
            throw IrohaPeerQRErrorV2.invalidMessage
        }
        guard inspected.schemaVersion
                == IrohaPeerWireMessageV1.kagemushaEligibilityPaymentSchemaVersion,
              inspected.streamID == frame.streamID,
              frame.total == UInt32(
                (inspected.encodedLength + IrohaPeerQRV2.shardBytes - 1)
                    / IrohaPeerQRV2.shardBytes
              ) else {
            throw IrohaPeerQRErrorV2.wrongCapability
        }
        if let headerBytes {
            guard headerBytes == frame.payload, streamID == frame.streamID,
                  total == frame.total else {
                throw IrohaPeerQRErrorV2.conflictingDuplicate
            }
            return
        }
        let url = directory.appendingPathComponent("iroha-iqr2-\(UUID().uuidString).part")
        guard FileManager.default.createFile(atPath: url.path, contents: nil) else {
            throw IrohaPeerQRErrorV2.storageFailure
        }
        do {
            let handle = try FileHandle(forUpdating: url)
            try handle.truncate(atOffset: UInt64(inspected.encodedLength))
            inspectedHeader = inspected
            headerBytes = frame.payload
            streamID = frame.streamID
            total = frame.total
            bitmap = Data(repeating: 0, count: (Int(frame.total) + 7) / 8)
            fileURL = url
            fileHandle = handle
        } catch {
            try? FileManager.default.removeItem(at: url)
            throw IrohaPeerQRErrorV2.storageFailure
        }
    }

    private func acceptData(
        _ frame: IrohaPeerQRFrameV2
    ) throws -> IrohaPeerWireMessageV1? {
        guard let inspectedHeader, let headerBytes, let streamID,
              let fileHandle, let fileURL else {
            throw IrohaPeerQRErrorV2.headerRequired
        }
        guard frame.streamID == streamID, frame.total == total,
              frame.index < total else {
            throw IrohaPeerQRErrorV2.conflictingDuplicate
        }
        let offset = Int(frame.index) * IrohaPeerQRV2.shardBytes
        let expected = min(IrohaPeerQRV2.shardBytes, inspectedHeader.encodedLength - offset)
        guard expected > 0, frame.payload.count == expected else {
            throw IrohaPeerQRErrorV2.malformedFrame
        }
        let byte = Int(frame.index / 8)
        let mask = UInt8(1 << Int(frame.index % 8))
        do {
            if bitmap[byte] & mask != 0 {
                try fileHandle.seek(toOffset: UInt64(offset))
                let existing = try fileHandle.read(upToCount: expected) ?? Data()
                guard existing == frame.payload else {
                    throw IrohaPeerQRErrorV2.conflictingDuplicate
                }
                return nil
            }
            try fileHandle.seek(toOffset: UInt64(offset))
            try fileHandle.write(contentsOf: frame.payload)
            bitmap[byte] = bitmap[byte] | mask
            received += 1
            guard received == total else { return nil }
            try fileHandle.synchronize()
            try fileHandle.close()
            self.fileHandle = nil
            let body = try Data(contentsOf: fileURL, options: [.mappedIfSafe])
            let message = try IrohaPeerWireMessageV1.decode(
                headerBytes + body,
                expectedProfile: .kagemusha,
                expectedKind: .payment,
                limits: limits
            )
            guard message.schemaVersion
                    == IrohaPeerWireMessageV1.kagemushaEligibilityPaymentSchemaVersion else {
                throw IrohaPeerQRErrorV2.wrongCapability
            }
            cancel()
            return message
        } catch let error as IrohaPeerQRErrorV2 {
            throw error
        } catch {
            throw IrohaPeerQRErrorV2.storageFailure
        }
    }
}

/// Independent rail readiness; no successful rail enables another one.
public enum IrohaPeerEligibilityTransportRailV1: Sendable {
    case qrIQR2
    case nfc
    case nearby
}

public struct IrohaPeerEligibilityTransportReadinessV1: Equatable, Sendable {
    public let qrIQR2Ready: Bool
    public let nfcReady: Bool
    public let nearbyReady: Bool

    public init(
        qrIQR2Ready: Bool = false,
        nfcReady: Bool = false,
        nearbyReady: Bool = false
    ) {
        self.qrIQR2Ready = qrIQR2Ready
        self.nfcReady = nfcReady
        self.nearbyReady = nearbyReady
    }

    public func isReady(for rail: IrohaPeerEligibilityTransportRailV1) -> Bool {
        switch rail {
        case .qrIQR2: return qrIQR2Ready
        case .nfc: return nfcReady
        case .nearby: return nearbyReady
        }
    }
}

private extension Data {
    mutating func iqr2AppendUInt16BE(_ value: UInt16) {
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    mutating func iqr2AppendUInt32BE(_ value: UInt32) {
        append(UInt8(truncatingIfNeeded: value >> 24))
        append(UInt8(truncatingIfNeeded: value >> 16))
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    func iqr2UInt16BE(at offset: Int) -> UInt16 {
        UInt16(self[offset]) << 8 | UInt16(self[offset + 1])
    }

    func iqr2UInt32BE(at offset: Int) -> UInt32 {
        UInt32(self[offset]) << 24 | UInt32(self[offset + 1]) << 16
            | UInt32(self[offset + 2]) << 8 | UInt32(self[offset + 3])
    }
}
