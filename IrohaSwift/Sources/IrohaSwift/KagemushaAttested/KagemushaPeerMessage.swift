import Foundation

/// One attested-suite peer message: a Request, a Payment or an Acknowledgement.
///
/// The payload is opaque to applications. It travels as IQR1 QR frames over an IPM1 envelope
/// with profile `KAGEMUSHA_ATTESTED_V1 = 2`, or as `kga1:` text (copy, share sheet or NFC).
public struct KagemushaPeerMessage: Sendable, Equatable {
    /// IPM1 kinds shared with KAGEMUSHA V1.
    public enum Kind: UInt8, Sendable, CaseIterable {
        case request = 1
        case payment = 2
        case acknowledgement = 3

        var schemaType: String {
            switch self {
            case .request: return KagemushaAttestedPaymentRequestV1.schemaType
            case .payment: return KagemushaAttestedPaymentV1.schemaType
            case .acknowledgement: return KagemushaAttestedAcknowledgementV1.schemaType
            }
        }
    }

    /// IPM1 profile code of the attested-app suite.
    public static let peerProfileCode: UInt16 = 2
    /// IPM1 schema version of the attested-app suite.
    public static let peerSchemaVersion: UInt16 = 1
    /// Text discriminator, distinct from V1 `kgm1:`.
    public static let textPrefix = "kga1:"

    public let kind: Kind
    /// The complete canonical Norito frame of the message.
    public let canonicalBytes: Data

    init(kind: Kind, canonicalBytes: Data) {
        self.kind = kind
        self.canonicalBytes = canonicalBytes
    }

    /// Decode a message from its canonical Norito bytes, identifying its kind by schema.
    public init(canonicalBytes: Data) throws {
        guard canonicalBytes.count <= KagemushaAttestedLimits.maximumPeerMessageBytes,
              let type = KagemushaAttestedFraming.schemaType(
                of: canonicalBytes, among: Kind.allCases.map(\.schemaType)),
              let kind = Kind.allCases.first(where: { $0.schemaType == type })
        else { throw KagemushaError.invalidMessage("unknown message schema") }
        do {
            switch kind {
            case .request: _ = try KagemushaAttestedPaymentRequestV1.decodeCanonical(canonicalBytes)
            case .payment: _ = try KagemushaAttestedPaymentV1.decodeCanonical(canonicalBytes)
            case .acknowledgement: _ = try KagemushaAttestedAcknowledgementV1.decodeCanonical(canonicalBytes)
            }
        } catch {
            throw KagemushaError.invalidMessage("non-canonical \(kind)")
        }
        self.init(kind: kind, canonicalBytes: canonicalBytes)
    }

    /// Decode strict `kga1:` text: unpadded base64url of one canonical value.
    public init(text: String) throws {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(Self.textPrefix),
              trimmed.utf8.count <= Self.textPrefix.utf8.count
                + (KagemushaAttestedLimits.maximumPeerMessageBytes * 4 + 2) / 3,
              let bytes = Data(kagemushaBase64URL: String(trimmed.dropFirst(Self.textPrefix.count))),
              !bytes.isEmpty
        else { throw KagemushaError.invalidMessage("malformed kga1 text") }
        try self.init(canonicalBytes: bytes)
    }

    /// Reassemble a message from scanned IQR1 frame texts in any order.
    public init(qrFrames: [String]) throws {
        let profile = try Self.wireProfile()
        let session = IrohaPeerQRScanSessionV1(
            expectedProfile: profile, expectedSchemaVersion: Self.peerSchemaVersion)
        for frame in qrFrames {
            let event: IrohaPeerQRScanEventV1
            do {
                event = try session.ingest(frame, atUptime: 0)
            } catch {
                throw KagemushaError.invalidMessage("invalid QR frame: \(error.localizedDescription)")
            }
            if case .completed(let wire) = event {
                try self.init(wireMessage: wire)
                return
            }
        }
        throw KagemushaError.invalidMessage("incomplete QR sequence")
    }

    /// Unwrap a completed IPM1 message.
    public init(wireMessage: IrohaPeerWireMessageV1) throws {
        guard wireMessage.profile.rawValue == Self.peerProfileCode,
              wireMessage.schemaVersion == Self.peerSchemaVersion
        else { throw KagemushaError.invalidMessage("wrong IPM1 profile") }
        try self.init(canonicalBytes: wireMessage.canonicalPayload)
        guard kind.rawValue == wireMessage.kind.rawValue else {
            throw KagemushaError.invalidMessage("IPM1 kind does not match the payload schema")
        }
    }

    /// `kga1:` text form.
    public func text() -> String {
        Self.textPrefix + canonicalBytes.kagemushaBase64URL
    }

    /// The IPM1 envelope (profile 2, schema version 1, no compression).
    public func wireMessage() throws -> IrohaPeerWireMessageV1 {
        guard let kind = IrohaPeerWireKindV1(rawValue: kind.rawValue) else {
            throw KagemushaError.invalidMessage("unknown IPM1 kind")
        }
        do {
            return try IrohaPeerWireMessageV1(
                profile: Self.wireProfile(), kind: kind, schemaVersion: Self.peerSchemaVersion,
                canonicalPayload: canonicalBytes)
        } catch let error as KagemushaError {
            throw error
        } catch {
            throw KagemushaError.invalidMessage("IPM1 envelope: \(error.localizedDescription)")
        }
    }

    /// IQR1 frame texts: one static frame when it fits, otherwise the animated sequence of
    /// 256-byte data shards with XOR parity.
    public func qrFrames() throws -> [String] {
        let wire = try wireMessage()
        do {
            if let single = try IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: wire) {
                return [single]
            }
            return try IrohaPeerQRCodecV1.animatedFrameTexts(for: wire)
        } catch {
            throw KagemushaError.invalidMessage("QR framing: \(error.localizedDescription)")
        }
    }

    /// The IPM1 profile for this suite. It is resolved from the shared wire enum so QR transport
    /// is available exactly when the shared IPM1 codec admits and validates profile 2.
    static func wireProfile() throws -> IrohaPeerWireProfileV1 {
        guard let profile = IrohaPeerWireProfileV1(rawValue: peerProfileCode) else {
            throw KagemushaError.invalidMessage("IPM1 profile 2 is not supported by this SDK build")
        }
        return profile
    }
}
