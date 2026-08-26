import Foundation
import zlib

/// Wire-stable application profiles carried by an `IPM1` peer message.
public enum IrohaPeerWireProfileV1: UInt16, CaseIterable, Sendable {
    /// Reserved on the wire so a zero-filled or omitted profile is never accepted.
    case reject = 0
    case kagemusha = 2
    case offlineCashV1 = 3
}

/// Wire-stable transfer phases carried by an `IPM1` peer message.
public enum IrohaPeerWireKindV1: UInt8, CaseIterable, Sendable {
    case receiveRequest = 1
    case payment = 2
    case acknowledgement = 3
}

/// Canonical cross-SDK spellings shared with the upstream Iroha SDK.
public typealias IrohaPeerPayloadProfile = IrohaPeerWireProfileV1
public typealias IrohaPeerPayloadKind = IrohaPeerWireKindV1

public extension IrohaPeerWireProfileV1 {
    static var kagemushaRecursiveSpend: Self { .kagemusha }

    /// The sole first-release canonical payload schema admitted by this profile.
    var requiredSchemaVersion: UInt16 {
        switch self {
        case .reject: return 0
        case .kagemusha: return 0x0102
        case .offlineCashV1: return 0x0100
        }
    }
}

public extension IrohaPeerWireKindV1 {
    /// Exact native archive schema admitted for this Kagemusha IPM1 kind.
    /// In particular, RECEIVE_REQUEST commits to the complete portable offer,
    /// not only its nested signed payment request.
    var requiredKagemushaCanonicalSchema: String {
        switch self {
        case .receiveRequest:
            return KagemushaRecursiveSpend.recipientReceiveOfferWireName
        case .payment:
            return KagemushaRecursiveSpend.peerPaymentWireNameV4
        case .acknowledgement:
            return KagemushaRecursiveSpend.acknowledgementWireName
        }
    }
}

public enum IrohaPeerWireEncodingV1: UInt8, Sendable {
    case none = 0
    case zlib = 1
}

/// Compression is opt-in. The shared peer policy emits zlib only when it
/// saves meaningful bytes and at least one fixed-size transport shard.
public enum IrohaPeerWireCompressionPolicyV1: Sendable {
    case disabled
    /// Cross-rail V1 encoding: use zlib only when it saves at least 32 bytes
    /// and at least one 256-byte shard.
    case peerOptimized
}

/// Allocation limits applied before an untrusted body is decompressed.
public struct IrohaPeerWireLimitsV1: Equatable, Sendable {
    /// The binary IPM1 Kagemusha profile deliberately admits one complete
    /// portable receiver-lineage offer on the smallest shared NFC rail. Text
    /// and static-QR codecs retain their independent, smaller rail limits.
    public static let maximumKagemushaProfileBytes = 24_576
    /// Largest single Offline Cash V1 peer message. The 12,288-byte value in
    /// the protocol is an aggregate text-session cap, not a per-message cap.
    public static let maximumOfflineCashProfileBytes =
        OfflineCashPeerAdapterV1.maximumPaymentTextBytes

    public let maximumCanonicalBytes: Int
    public let maximumKagemushaEncodedBytes: Int
    public let maximumOfflineCashEncodedBytes: Int

    public init(
        maximumCanonicalBytes: Int = 32 * 1024,
        maximumKagemushaEncodedBytes: Int = Self.maximumKagemushaProfileBytes,
        maximumOfflineCashEncodedBytes: Int = Self.maximumOfflineCashProfileBytes
    ) {
        precondition(
            Self.areValid(
                maximumCanonicalBytes: maximumCanonicalBytes,
                maximumKagemushaEncodedBytes: maximumKagemushaEncodedBytes,
                maximumOfflineCashEncodedBytes: maximumOfflineCashEncodedBytes
            )
        )
        self.maximumCanonicalBytes = maximumCanonicalBytes
        self.maximumKagemushaEncodedBytes = maximumKagemushaEncodedBytes
        self.maximumOfflineCashEncodedBytes = maximumOfflineCashEncodedBytes
    }

    public static let peerV1 = IrohaPeerWireLimitsV1()

    static func areValid(
        maximumCanonicalBytes: Int,
        maximumKagemushaEncodedBytes: Int,
        maximumOfflineCashEncodedBytes: Int = Self.maximumOfflineCashProfileBytes
    ) -> Bool {
        (1...(32 * 1_024)).contains(maximumCanonicalBytes) &&
            (1...maximumKagemushaProfileBytes).contains(maximumKagemushaEncodedBytes) &&
            (1...maximumOfflineCashProfileBytes).contains(maximumOfflineCashEncodedBytes)
    }

    public func maximumEncodedBytes(for profile: IrohaPeerWireProfileV1) throws -> Int {
        switch profile {
        case .reject:
            throw IrohaPeerWireMessageErrorV1.invalidProfile(profile.rawValue)
        case .kagemusha:
            return maximumKagemushaEncodedBytes
        case .offlineCashV1:
            return maximumOfflineCashEncodedBytes
        }
    }

    /// Kind-aware encoded-body ceiling. Offline Cash V1 has three distinct
    /// canonical text maxima and must never inherit the aggregate session cap.
    public func maximumEncodedBytes(
        for profile: IrohaPeerWireProfileV1,
        kind: IrohaPeerWireKindV1
    ) throws -> Int {
        switch profile {
        case .reject:
            throw IrohaPeerWireMessageErrorV1.invalidProfile(profile.rawValue)
        case .kagemusha:
            return maximumKagemushaEncodedBytes
        case .offlineCashV1:
            let protocolMaximum: Int
            switch kind {
            case .receiveRequest:
                protocolMaximum = OfflineCashPeerAdapterV1.maximumPaymentRequestTextBytes
            case .payment:
                protocolMaximum = OfflineCashPeerAdapterV1.maximumPaymentTextBytes
            case .acknowledgement:
                protocolMaximum = OfflineCashPeerAdapterV1.maximumAcknowledgementTextBytes
            }
            return min(maximumOfflineCashEncodedBytes, protocolMaximum)
        }
    }
}

public enum IrohaPeerWireMessageErrorV1: Error, Equatable, LocalizedError, Sendable {
    case invalidMagic
    case unsupportedWireVersion(UInt8)
    case unsupportedEncoding(UInt8)
    case invalidProfile(UInt16)
    case invalidKind(UInt8)
    case invalidFlags(UInt8)
    case invalidSchemaVersion
    case schemaVersionMismatch(
        profile: IrohaPeerWireProfileV1,
        expected: UInt16,
        actual: UInt16
    )
    case emptyCanonicalPayload
    case emptyEncodedBody
    case canonicalLengthOutOfRange(actual: Int, maximum: Int)
    case encodedLengthOutOfRange(actual: Int, maximum: Int)
    case lengthMismatch
    case compressionPolicyNotSatisfied
    case decompressionFailed
    case canonicalHashMismatch
    case wireHashMismatch
    case unexpectedProfile(expected: IrohaPeerWireProfileV1, actual: IrohaPeerWireProfileV1)
    case unexpectedKind(expected: IrohaPeerWireKindV1, actual: IrohaPeerWireKindV1)
    case invalidCanonicalPayload(
        profile: IrohaPeerWireProfileV1,
        kind: IrohaPeerWireKindV1
    )

    public var errorDescription: String? {
        switch self {
        case .invalidMagic: return "IPM1 peer message magic mismatch."
        case .unsupportedWireVersion(let value): return "Unsupported IPM1 wire version \(value)."
        case .unsupportedEncoding(let value): return "Unsupported IPM1 content encoding \(value)."
        case .invalidProfile(let value): return "Invalid IPM1 payload profile \(value)."
        case .invalidKind(let value): return "Invalid IPM1 payload kind \(value)."
        case .invalidFlags(let value): return "Invalid IPM1 flags \(value)."
        case .invalidSchemaVersion: return "IPM1 schema version zero is reserved."
        case .schemaVersionMismatch(let profile, let expected, let actual):
            return "IPM1 profile \(profile) requires schema \(expected), received \(actual)."
        case .emptyCanonicalPayload: return "IPM1 canonical payload must not be empty."
        case .emptyEncodedBody: return "IPM1 encoded body must not be empty."
        case .canonicalLengthOutOfRange(let actual, let maximum):
            return "IPM1 canonical length \(actual) exceeds the \(maximum)-byte limit."
        case .encodedLengthOutOfRange(let actual, let maximum):
            return "IPM1 encoded length \(actual) exceeds the \(maximum)-byte profile limit."
        case .lengthMismatch: return "IPM1 declared and actual lengths differ."
        case .compressionPolicyNotSatisfied: return "IPM1 zlib encoding does not satisfy the peer compression policy."
        case .decompressionFailed: return "IPM1 zlib body is invalid."
        case .canonicalHashMismatch: return "IPM1 canonical payload hash mismatch."
        case .wireHashMismatch: return "IPM1 wire hash mismatch."
        case .unexpectedProfile(let expected, let actual):
            return "Expected IPM1 profile \(expected), received \(actual)."
        case .unexpectedKind(let expected, let actual):
            return "Expected IPM1 kind \(expected), received \(actual)."
        case .invalidCanonicalPayload(let profile, let kind):
            return "IPM1 profile \(profile) has invalid canonical bytes for kind \(kind)."
        }
    }
}

/// Parsed, allocation-safe metadata from the fixed 84-byte `IPM1` header.
public struct IrohaPeerWireHeaderV1: Equatable, Sendable {
    public let encoding: IrohaPeerWireEncodingV1
    public let profile: IrohaPeerWireProfileV1
    public let kind: IrohaPeerWireKindV1
    public let schemaVersion: UInt16
    public let canonicalLength: Int
    public let encodedLength: Int
    public let canonicalHash: Data
    public let wireHash: Data
    public let bytes: Data

    public var streamID: Data { Data(wireHash.prefix(16)) }
    public var dataShardCount: Int {
        (encodedLength + IrohaPeerWireMessageV1.qrDataShardBytes - 1)
            / IrohaPeerWireMessageV1.qrDataShardBytes
    }
}

/// Transport-neutral `IPM1` envelope. It wraps already-canonical application
/// bytes and deliberately has no knowledge of domain payload construction.
public struct IrohaPeerWireMessageV1: Equatable, Sendable {
    public static let magic = Data("IPM1".utf8)
    public static let wireVersion: UInt8 = 1
    public static let headerBytes = 84
    public static let qrDataShardBytes = 256

    private static let canonicalHashDomain = Data("IROHA-PEER-PAYLOAD-V1\0".utf8)
    private static let wireHashDomain = Data("IROHA-PEER-MESSAGE-V1\0".utf8)

    public let header: IrohaPeerWireHeaderV1
    public let canonicalPayload: Data
    public let encodedBody: Data

    public var encoding: IrohaPeerWireEncodingV1 { header.encoding }
    public var profile: IrohaPeerWireProfileV1 { header.profile }
    public var kind: IrohaPeerWireKindV1 { header.kind }
    public var schemaVersion: UInt16 { header.schemaVersion }
    public var canonicalHash: Data { header.canonicalHash }
    public var wireHash: Data { header.wireHash }
    public var streamID: Data { header.streamID }
    public var encoded: Data { header.bytes + encodedBody }

    public init(
        profile: IrohaPeerWireProfileV1,
        kind: IrohaPeerWireKindV1,
        schemaVersion: UInt16,
        canonicalPayload: Data,
        compressionPolicy: IrohaPeerWireCompressionPolicyV1 = .disabled,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws {
        guard profile != .reject else {
            throw IrohaPeerWireMessageErrorV1.invalidProfile(profile.rawValue)
        }
        guard schemaVersion != 0 else {
            throw IrohaPeerWireMessageErrorV1.invalidSchemaVersion
        }
        guard schemaVersion == profile.requiredSchemaVersion else {
            throw IrohaPeerWireMessageErrorV1.schemaVersionMismatch(
                profile: profile,
                expected: profile.requiredSchemaVersion,
                actual: schemaVersion
            )
        }
        guard !canonicalPayload.isEmpty else {
            throw IrohaPeerWireMessageErrorV1.emptyCanonicalPayload
        }
        guard canonicalPayload.count <= limits.maximumCanonicalBytes else {
            throw IrohaPeerWireMessageErrorV1.canonicalLengthOutOfRange(
                actual: canonicalPayload.count,
                maximum: limits.maximumCanonicalBytes
            )
        }
        try Self.validateCanonicalPayload(
            profile: profile,
            kind: kind,
            canonicalPayload: canonicalPayload
        )
        let maximumEncoded = try limits.maximumEncodedBytes(for: profile, kind: kind)
        let selected = try Self.selectEncoding(
            canonicalPayload,
            policy: compressionPolicy,
            maximumEncodedBytes: maximumEncoded
        )
        let canonicalHash = Self.canonicalHash(
            profile: profile,
            kind: kind,
            schemaVersion: schemaVersion,
            canonicalPayload: canonicalPayload
        )
        let prefix = Self.makeHeaderPrefix(
            encoding: selected.encoding,
            profile: profile,
            kind: kind,
            schemaVersion: schemaVersion,
            canonicalLength: canonicalPayload.count,
            encodedLength: selected.body.count,
            canonicalHash: canonicalHash
        )
        let wireHash = Blake2b.hash256(Self.wireHashDomain + prefix + selected.body)
        let headerBytes = prefix + wireHash
        self.header = try Self.inspectHeader(headerBytes, limits: limits)
        self.canonicalPayload = canonicalPayload
        self.encodedBody = selected.body
    }

    private init(header: IrohaPeerWireHeaderV1, canonicalPayload: Data, encodedBody: Data) {
        self.header = header
        self.canonicalPayload = canonicalPayload
        self.encodedBody = encodedBody
    }

    public static func inspectHeader(
        _ data: Data,
        expectedProfile: IrohaPeerWireProfileV1? = nil,
        expectedKind: IrohaPeerWireKindV1? = nil,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> IrohaPeerWireHeaderV1 {
        guard data.count == headerBytes else { throw IrohaPeerWireMessageErrorV1.lengthMismatch }
        guard data.prefix(4) == magic else { throw IrohaPeerWireMessageErrorV1.invalidMagic }
        guard data[4] == wireVersion else {
            throw IrohaPeerWireMessageErrorV1.unsupportedWireVersion(data[4])
        }
        guard let encoding = IrohaPeerWireEncodingV1(rawValue: data[5]) else {
            throw IrohaPeerWireMessageErrorV1.unsupportedEncoding(data[5])
        }
        let rawProfile = data.ipmUInt16BE(at: 6)
        guard let profile = IrohaPeerWireProfileV1(rawValue: rawProfile), profile != .reject else {
            throw IrohaPeerWireMessageErrorV1.invalidProfile(rawProfile)
        }
        guard let kind = IrohaPeerWireKindV1(rawValue: data[8]) else {
            throw IrohaPeerWireMessageErrorV1.invalidKind(data[8])
        }
        guard data[9] == 0 else { throw IrohaPeerWireMessageErrorV1.invalidFlags(data[9]) }
        let schemaVersion = data.ipmUInt16BE(at: 10)
        guard schemaVersion != 0 else { throw IrohaPeerWireMessageErrorV1.invalidSchemaVersion }
        guard schemaVersion == profile.requiredSchemaVersion else {
            throw IrohaPeerWireMessageErrorV1.schemaVersionMismatch(
                profile: profile,
                expected: profile.requiredSchemaVersion,
                actual: schemaVersion
            )
        }
        let canonicalLength = Int(data.ipmUInt32BE(at: 12))
        let encodedLength = Int(data.ipmUInt32BE(at: 16))
        guard canonicalLength > 0 else {
            throw IrohaPeerWireMessageErrorV1.emptyCanonicalPayload
        }
        guard encodedLength > 0 else {
            throw IrohaPeerWireMessageErrorV1.emptyEncodedBody
        }
        guard canonicalLength <= limits.maximumCanonicalBytes else {
            throw IrohaPeerWireMessageErrorV1.canonicalLengthOutOfRange(
                actual: canonicalLength,
                maximum: limits.maximumCanonicalBytes
            )
        }
        let maximumEncoded = try limits.maximumEncodedBytes(for: profile, kind: kind)
        guard encodedLength <= maximumEncoded else {
            throw IrohaPeerWireMessageErrorV1.encodedLengthOutOfRange(
                actual: encodedLength,
                maximum: maximumEncoded
            )
        }
        guard encoding != .none || canonicalLength == encodedLength else {
            throw IrohaPeerWireMessageErrorV1.lengthMismatch
        }
        if encoding == .zlib {
            guard canonicalLength > 0,
                  encodedLength > 0,
                  canonicalLength >= encodedLength,
                  canonicalLength - encodedLength >= 32,
                  shardCount(encodedLength) < shardCount(canonicalLength) else {
                throw IrohaPeerWireMessageErrorV1.compressionPolicyNotSatisfied
            }
        }
        if let expectedProfile, expectedProfile != profile {
            throw IrohaPeerWireMessageErrorV1.unexpectedProfile(expected: expectedProfile, actual: profile)
        }
        if let expectedKind, expectedKind != kind {
            throw IrohaPeerWireMessageErrorV1.unexpectedKind(expected: expectedKind, actual: kind)
        }
        return IrohaPeerWireHeaderV1(
            encoding: encoding,
            profile: profile,
            kind: kind,
            schemaVersion: schemaVersion,
            canonicalLength: canonicalLength,
            encodedLength: encodedLength,
            canonicalHash: data.subdata(in: 20..<52),
            wireHash: data.subdata(in: 52..<84),
            bytes: data
        )
    }

    public static func decode(
        _ data: Data,
        expectedProfile: IrohaPeerWireProfileV1? = nil,
        expectedKind: IrohaPeerWireKindV1? = nil,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> Self {
        guard data.count >= headerBytes else { throw IrohaPeerWireMessageErrorV1.lengthMismatch }
        let headerData = data.subdata(in: 0..<headerBytes)
        let header = try inspectHeader(
            headerData,
            expectedProfile: expectedProfile,
            expectedKind: expectedKind,
            limits: limits
        )
        guard data.count == headerBytes + header.encodedLength else {
            throw IrohaPeerWireMessageErrorV1.lengthMismatch
        }
        let encodedBody = data.subdata(in: headerBytes..<data.count)
        let expectedWireHash = Blake2b.hash256(
            wireHashDomain + headerData.subdata(in: 0..<52) + encodedBody
        )
        guard expectedWireHash == header.wireHash else {
            throw IrohaPeerWireMessageErrorV1.wireHashMismatch
        }
        let canonicalPayload: Data
        switch header.encoding {
        case .none:
            canonicalPayload = encodedBody
        case .zlib:
            canonicalPayload = try decodeZlib(
                encodedBody,
                expectedLength: header.canonicalLength,
                maximumLength: limits.maximumCanonicalBytes
            )
        }
        guard canonicalPayload.count == header.canonicalLength else {
            throw IrohaPeerWireMessageErrorV1.lengthMismatch
        }
        try validateCanonicalPayload(
            profile: header.profile,
            kind: header.kind,
            canonicalPayload: canonicalPayload
        )
        let expectedCanonicalHash = canonicalHash(
            profile: header.profile,
            kind: header.kind,
            schemaVersion: header.schemaVersion,
            canonicalPayload: canonicalPayload
        )
        guard expectedCanonicalHash == header.canonicalHash else {
            throw IrohaPeerWireMessageErrorV1.canonicalHashMismatch
        }
        return Self(header: header, canonicalPayload: canonicalPayload, encodedBody: encodedBody)
    }

    private static func validateCanonicalPayload(
        profile: IrohaPeerWireProfileV1,
        kind: IrohaPeerWireKindV1,
        canonicalPayload: Data
    ) throws {
        if profile == .offlineCashV1 {
            try validateOfflineCashPeerText(canonicalPayload, kind: kind)
            return
        }
        guard profile == .kagemusha else { return }
        do {
            // Transport acceptance is deliberately native-independent:
            // canonical compact Norito framing, checksum, and the exact
            // kind-specific ABI-22 schema. Deeper semantic validation remains
            // in IrohaPeerKagemushaAdapterV1/KagemushaPeerPayload.
            try KagemushaRecursiveSpend.requireArchive(
                canonicalPayload,
                schema: kind.requiredKagemushaCanonicalSchema,
                field: "ipm1.kagemusha.\(kind)"
            )
        } catch {
            throw IrohaPeerWireMessageErrorV1.invalidCanonicalPayload(
                profile: profile,
                kind: kind
            )
        }
    }

    private static func validateOfflineCashPeerText(
        _ canonicalPayload: Data,
        kind: IrohaPeerWireKindV1
    ) throws {
        let textMaximum: Int
        let rawMaximum: Int
        let schema: String
        let requiredPadding: Int
        switch kind {
        case .receiveRequest:
            textMaximum = OfflineCashPeerAdapterV1.maximumPaymentRequestTextBytes
            rawMaximum = OfflineCashPaymentRequestV1.maximumCanonicalBytes
            schema =
                "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentRequestV1"
            requiredPadding = 8
        case .payment:
            textMaximum = OfflineCashPeerAdapterV1.maximumPaymentTextBytes
            rawMaximum = OfflineCashPaymentV1.maximumCanonicalBytes
            schema = "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentV1"
            requiredPadding = 8
        case .acknowledgement:
            textMaximum = OfflineCashPeerAdapterV1.maximumAcknowledgementTextBytes
            rawMaximum = OfflineCashAcknowledgementV1.maximumCanonicalBytes
            schema =
                "iroha_data_model::offline::offline_cash_v1::OfflineCashAcknowledgementV1"
            requiredPadding = 0
        }
        guard canonicalPayload.count <= textMaximum,
              let text = String(data: canonicalPayload, encoding: .utf8),
              Data(text.utf8) == canonicalPayload,
              text.hasPrefix(OfflineCashPeerAdapterV1.textPrefix) else {
            throw IrohaPeerWireMessageErrorV1.invalidCanonicalPayload(
                profile: .offlineCashV1,
                kind: kind
            )
        }
        let body = String(text.dropFirst(OfflineCashPeerAdapterV1.textPrefix.count))
        let allowed = CharacterSet(charactersIn:
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
        )
        guard !body.isEmpty,
              body.unicodeScalars.allSatisfy({ allowed.contains($0) }) else {
            throw IrohaPeerWireMessageErrorV1.invalidCanonicalPayload(
                profile: .offlineCashV1,
                kind: kind
            )
        }
        let padding = String(repeating: "=", count: (4 - body.utf8.count % 4) % 4)
        let standard = body.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/") + padding
        guard let decoded = Data(base64Encoded: standard),
              decoded.count <= rawMaximum,
              decoded.base64EncodedString()
                .replacingOccurrences(of: "+", with: "-")
                .replacingOccurrences(of: "/", with: "_")
                .replacingOccurrences(of: "=", with: "") == body,
              let archive = noritoDecodeFrame(decoded),
              archive.header.schema == noritoSchemaHash(forTypeName: schema),
              archive.header.compression == .none,
              archive.header.flags == NoritoHeader.compactLen,
              archive.paddingLength == requiredPadding,
              !archive.payload.isEmpty,
              archive.header.encode() == decoded.prefix(NoritoHeader.encodedLength) else {
            throw IrohaPeerWireMessageErrorV1.invalidCanonicalPayload(
                profile: .offlineCashV1,
                kind: kind
            )
        }
    }

    private static func selectEncoding(
        _ canonicalPayload: Data,
        policy: IrohaPeerWireCompressionPolicyV1,
        maximumEncodedBytes: Int
    ) throws -> (encoding: IrohaPeerWireEncodingV1, body: Data) {
        if policy == .peerOptimized,
           let compressed = encodeZlibIfSmaller(canonicalPayload),
           canonicalPayload.count - compressed.count >= 32,
           shardCount(compressed.count) < shardCount(canonicalPayload.count),
           compressed.count <= maximumEncodedBytes {
            return (.zlib, compressed)
        }
        guard canonicalPayload.count <= maximumEncodedBytes else {
            throw IrohaPeerWireMessageErrorV1.encodedLengthOutOfRange(
                actual: canonicalPayload.count,
                maximum: maximumEncodedBytes
            )
        }
        return (.none, canonicalPayload)
    }

    private static func shardCount(_ byteCount: Int) -> Int {
        (byteCount + qrDataShardBytes - 1) / qrDataShardBytes
    }

    private static func canonicalHash(
        profile: IrohaPeerWireProfileV1,
        kind: IrohaPeerWireKindV1,
        schemaVersion: UInt16,
        canonicalPayload: Data
    ) -> Data {
        var input = canonicalHashDomain
        input.ipmAppendUInt16BE(profile.rawValue)
        input.append(kind.rawValue)
        input.ipmAppendUInt16BE(schemaVersion)
        input.append(canonicalPayload)
        return Blake2b.hash256(input)
    }

    private static func makeHeaderPrefix(
        encoding: IrohaPeerWireEncodingV1,
        profile: IrohaPeerWireProfileV1,
        kind: IrohaPeerWireKindV1,
        schemaVersion: UInt16,
        canonicalLength: Int,
        encodedLength: Int,
        canonicalHash: Data
    ) -> Data {
        precondition(canonicalLength <= Int(UInt32.max))
        precondition(encodedLength <= Int(UInt32.max))
        precondition(canonicalHash.count == 32)
        var data = magic
        data.append(wireVersion)
        data.append(encoding.rawValue)
        data.ipmAppendUInt16BE(profile.rawValue)
        data.append(kind.rawValue)
        data.append(0)
        data.ipmAppendUInt16BE(schemaVersion)
        data.ipmAppendUInt32BE(UInt32(canonicalLength))
        data.ipmAppendUInt32BE(UInt32(encodedLength))
        data.append(canonicalHash)
        precondition(data.count == 52)
        return data
    }

    private static func encodeZlibIfSmaller(_ input: Data) -> Data? {
        guard !input.isEmpty else { return nil }
        // IPM1 fixes zlib's interoperable default level (6). Apple's
        // Compression.framework emits a different raw-DEFLATE stream, and
        // wrapping that stream in a 0x789c header is not byte-exact with the
        // Kotlin/Java reference encoder.
        let outputCapacity = Int(compressBound(uLong(input.count)))
        var output = [UInt8](repeating: 0, count: outputCapacity)
        var encodedCount = uLongf(outputCapacity)
        let status = input.withUnsafeBytes { inputBytes in
            output.withUnsafeMutableBytes { outputBytes in
                compress2(
                    outputBytes.bindMemory(to: UInt8.self).baseAddress!,
                    &encodedCount,
                    inputBytes.bindMemory(to: UInt8.self).baseAddress!,
                    uLong(input.count),
                    Z_DEFAULT_COMPRESSION
                )
            }
        }
        guard status == Z_OK,
              encodedCount > 0,
              encodedCount < uLongf(input.count) else { return nil }
        return Data(output.prefix(Int(encodedCount)))
    }

    private static func decodeZlib(
        _ encoded: Data,
        expectedLength: Int,
        maximumLength: Int
    ) throws -> Data {
        guard expectedLength <= maximumLength else {
            throw IrohaPeerWireMessageErrorV1.canonicalLengthOutOfRange(
                actual: expectedLength,
                maximum: maximumLength
            )
        }
        guard !encoded.isEmpty, expectedLength > 0 else {
            throw IrohaPeerWireMessageErrorV1.decompressionFailed
        }
        guard encoded.count >= 6, encoded[0] == 0x78, encoded[1] == 0x9C else {
            throw IrohaPeerWireMessageErrorV1.decompressionFailed
        }
        let declaredAdler32 = encoded.ipmUInt32BE(at: encoded.count - 4)
        var stream = z_stream()
        guard inflateInit_(&stream, ZLIB_VERSION, Int32(MemoryLayout<z_stream>.size)) == Z_OK else {
            throw IrohaPeerWireMessageErrorV1.decompressionFailed
        }
        defer { inflateEnd(&stream) }

        // One extra byte detects expansion beyond the declared canonical size.
        var output = [UInt8](repeating: 0, count: expectedLength + 1)
        let outputCapacity = output.count
        var status = Int32(Z_STREAM_ERROR)
        encoded.withUnsafeBytes { rawInput in
            output.withUnsafeMutableBytes { rawOutput in
                stream.next_in = UnsafeMutablePointer(
                    mutating: rawInput.bindMemory(to: Bytef.self).baseAddress!
                )
                stream.avail_in = uInt(encoded.count)
                stream.next_out = rawOutput.bindMemory(to: Bytef.self).baseAddress!
                stream.avail_out = uInt(outputCapacity)
                status = inflate(&stream, Z_FINISH)
            }
        }
        guard status == Z_STREAM_END,
              stream.avail_in == 0,
              stream.total_out == uLong(expectedLength) else {
            throw IrohaPeerWireMessageErrorV1.decompressionFailed
        }
        let canonical = Data(output.prefix(expectedLength))
        guard adler32(canonical) == declaredAdler32 else {
            throw IrohaPeerWireMessageErrorV1.decompressionFailed
        }
        return canonical
    }

    private static func adler32(_ data: Data) -> UInt32 {
        let modulus: UInt32 = 65_521
        var first: UInt32 = 1
        var second: UInt32 = 0
        for byte in data {
            first = (first + UInt32(byte)) % modulus
            second = (second + first) % modulus
        }
        return second << 16 | first
    }
}

private extension Data {
    mutating func ipmAppendUInt16BE(_ value: UInt16) {
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    mutating func ipmAppendUInt32BE(_ value: UInt32) {
        append(UInt8(truncatingIfNeeded: value >> 24))
        append(UInt8(truncatingIfNeeded: value >> 16))
        append(UInt8(truncatingIfNeeded: value >> 8))
        append(UInt8(truncatingIfNeeded: value))
    }

    func ipmUInt16BE(at offset: Int) -> UInt16 {
        UInt16(self[offset]) << 8 | UInt16(self[offset + 1])
    }

    func ipmUInt32BE(at offset: Int) -> UInt32 {
        UInt32(self[offset]) << 24
            | UInt32(self[offset + 1]) << 16
            | UInt32(self[offset + 2]) << 8
            | UInt32(self[offset + 3])
    }
}
