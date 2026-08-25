import Foundation

/// The three peer-carried Kagemusha archives.
///
/// Raw values are part of the first-release NFC/QR/Nearby wire contract.
public enum KagemushaPeerPayloadKind: UInt8, CaseIterable, Codable, Sendable {
    case receiveRequest = 1
    case payment = 2
    case acknowledgement = 3

    public var textPrefix: String {
        switch self {
        case .receiveRequest:
            return KagemushaPeerTransportContract.receiveRequestTextPrefix
        case .payment:
            return KagemushaPeerTransportContract.paymentTextPrefix
        case .acknowledgement:
            return KagemushaPeerTransportContract.acknowledgementTextPrefix
        }
    }

    public var contentType: String {
        switch self {
        case .receiveRequest:
            return KagemushaPeerTransportContract.receiveRequestContentType
        case .payment:
            return KagemushaPeerTransportContract.paymentContentType
        case .acknowledgement:
            return KagemushaPeerTransportContract.acknowledgementContentType
        }
    }

    public init?(textPrefix: String) {
        guard let value = Self.allCases.first(where: { $0.textPrefix == textPrefix }) else {
            return nil
        }
        self = value
    }

    public init?(contentType: String) {
        guard let value = Self.allCases.first(where: { $0.contentType == contentType }) else {
            return nil
        }
        self = value
    }
}

/// Stable identifiers shared by every first-release Kagemusha peer transport.
public enum KagemushaPeerTransportContract {
    public static let receiveRequestTextPrefix = "PKK2R."
    public static let paymentTextPrefix = "PKK2P."
    public static let acknowledgementTextPrefix = "PKK2A."
    public static let qrStreamTextPrefix = "PKKQ1."

    /// Kagemusha uses the canonical SDK NFC AID; it does not register a
    /// parallel application identifier that could route the same transfer to
    /// a different protocol implementation.
    public static let nfcApplicationIdentifierHex =
        IrohaPeerNfcV1.applicationIdentifierHex
    public static let nearbyServiceName = "pk-kagemusha"
    public static let nearbyBonjourService = "_pk-kagemusha._tcp"

    public static let receiveRequestContentType =
        "text/vnd.pk.kagemusha-v2.receive-request"
    public static let paymentContentType = "text/vnd.pk.kagemusha-v2.payment"
    public static let acknowledgementContentType = "text/vnd.pk.kagemusha-v2.ack"

    public static let maximumArchiveBytesV2 =
        KagemushaRecursiveSpend.maximumPeerArchiveBytesV2
    public static let maximumArchiveBytesV4 =
        KagemushaRecursiveSpend.maximumPeerArchiveBytesV4
    public static let maximumArchiveBytes = maximumArchiveBytesV4
    /// Largest raw archive that fits in a direct `PKK2?.` text envelope.
    public static let maximumTextArchiveBytes =
        KagemushaRecursiveSpend.maximumPeerTextArchiveBytes
    public static let maximumTextEnvelopeBytes =
        KagemushaRecursiveSpend.maximumPeerTextEnvelopeBytes
}

/// A canonical, decoded Kagemusha peer archive.
public enum KagemushaPeerPayload: Equatable, Sendable {
    /// Kagemusha V4 carries the whole portable offer (signed request, reusable
    /// lineage, and publisher checkpoint envelope) under wire kind 1.
    case receiveRequest(KagemushaRecipientReceiveOfferV2)
    case payment(KagemushaRecursiveSpendPeerPaymentV4)
    case acknowledgement(KagemushaReceiverAcknowledgement)

    public var kind: KagemushaPeerPayloadKind {
        switch self {
        case .receiveRequest:
            return .receiveRequest
        case .payment:
            return .payment
        case .acknowledgement:
            return .acknowledgement
        }
    }

    public var archive: Data {
        switch self {
        case .receiveRequest(let offer):
            return offer.noritoArchive
        case .payment(let payment):
            return payment.archive
        case .acknowledgement(let acknowledgement):
            return acknowledgement.archive
        }
    }

    public static func decode(
        archive: Data,
        kind: KagemushaPeerPayloadKind,
        chainDiscriminant: UInt16
    ) throws -> Self {
        guard !archive.isEmpty else {
            throw KagemushaPeerTransportError.emptyPayload
        }
        guard archive.count <= KagemushaPeerTransportContract.maximumArchiveBytes else {
            throw KagemushaPeerTransportError.archiveTooLarge(
                actual: archive.count,
                maximum: KagemushaPeerTransportContract.maximumArchiveBytes
            )
        }
        do {
            switch kind {
            case .receiveRequest:
                return .receiveRequest(
                    try KagemushaRecipientReceiveOfferV2(
                        noritoArchive: archive,
                        chainDiscriminant: chainDiscriminant
                    )
                )
            case .payment:
                return .payment(try KagemushaRecursiveSpendPeerPaymentV4(noritoArchive: archive))
            case .acknowledgement:
                return .acknowledgement(
                    try KagemushaRecursiveSpendCodecs.decodeAcknowledgement(archive)
                )
            }
        } catch let error as KagemushaPeerTransportError {
            throw error
        } catch {
            throw KagemushaPeerTransportError.invalidArchive(kind: kind)
        }
    }
}

/// A completed sender-side peer exchange. Callers must still perform the
/// acknowledgement's cryptographic sender verification before releasing any
/// locally reserved inputs.
public struct KagemushaPeerSendResult: Equatable, Sendable {
    public let payment: KagemushaRecursiveSpendPeerPaymentV4
    public let acknowledgement: KagemushaReceiverAcknowledgement

    public init(
        payment: KagemushaRecursiveSpendPeerPaymentV4,
        acknowledgement: KagemushaReceiverAcknowledgement
    ) {
        self.payment = payment
        self.acknowledgement = acknowledgement
    }
}

public enum KagemushaPeerTransportError: Error, Equatable, LocalizedError, Sendable {
    case emptyPayload
    case archiveTooLarge(actual: Int, maximum: Int)
    case textEnvelopeTooLarge(actual: Int, maximum: Int)
    case invalidPrefix
    case unexpectedKind(expected: KagemushaPeerPayloadKind, actual: KagemushaPeerPayloadKind)
    case invalidBase64URL
    case nonCanonicalEncoding
    case invalidArchive(kind: KagemushaPeerPayloadKind)

    public var errorDescription: String? {
        switch self {
        case .emptyPayload:
            return "Kagemusha peer payload is empty."
        case .archiveTooLarge(let actual, let maximum):
            return "Kagemusha peer archive is \(actual) bytes; the limit is \(maximum)."
        case .textEnvelopeTooLarge(let actual, let maximum):
            return "Kagemusha peer text is \(actual) bytes; the limit is \(maximum)."
        case .invalidPrefix:
            return "Kagemusha peer text has an invalid or missing prefix."
        case .unexpectedKind(let expected, let actual):
            return "Expected Kagemusha payload kind \(expected), received \(actual)."
        case .invalidBase64URL:
            return "Kagemusha peer text is not canonical unpadded base64url."
        case .nonCanonicalEncoding:
            return "Kagemusha peer text is not canonically encoded."
        case .invalidArchive(let kind):
            return "Kagemusha peer archive is not a valid \(kind) archive."
        }
    }
}

/// Canonical `PKK2R.`, `PKK2P.`, and `PKK2A.` text envelopes.
public enum KagemushaPeerTextCodec {
    public static func encode(_ payload: KagemushaPeerPayload) throws -> String {
        let archive = payload.archive
        guard !archive.isEmpty else {
            throw KagemushaPeerTransportError.emptyPayload
        }
        guard archive.count <= KagemushaPeerTransportContract.maximumTextArchiveBytes else {
            throw KagemushaPeerTransportError.archiveTooLarge(
                actual: archive.count,
                maximum: KagemushaPeerTransportContract.maximumTextArchiveBytes
            )
        }
        let value = payload.kind.textPrefix + base64URLEncode(archive)
        let byteCount = value.utf8.count
        guard byteCount <= KagemushaPeerTransportContract.maximumTextEnvelopeBytes else {
            throw KagemushaPeerTransportError.textEnvelopeTooLarge(
                actual: byteCount,
                maximum: KagemushaPeerTransportContract.maximumTextEnvelopeBytes
            )
        }
        return value
    }

    /// Strictly decodes already-normalized text. No whitespace is ignored.
    public static func decode(
        _ value: String,
        chainDiscriminant: UInt16,
        expectedKind: KagemushaPeerPayloadKind? = nil
    ) throws -> KagemushaPeerPayload {
        let byteCount = value.utf8.count
        guard byteCount <= KagemushaPeerTransportContract.maximumTextEnvelopeBytes else {
            throw KagemushaPeerTransportError.textEnvelopeTooLarge(
                actual: byteCount,
                maximum: KagemushaPeerTransportContract.maximumTextEnvelopeBytes
            )
        }
        guard let kind = kind(of: value) else {
            throw KagemushaPeerTransportError.invalidPrefix
        }
        if let expectedKind, expectedKind != kind {
            throw KagemushaPeerTransportError.unexpectedKind(
                expected: expectedKind,
                actual: kind
            )
        }
        let body = String(value.dropFirst(kind.textPrefix.count))
        guard let archive = base64URLDecode(body) else {
            throw KagemushaPeerTransportError.invalidBase64URL
        }
        guard kind.textPrefix + base64URLEncode(archive) == value else {
            throw KagemushaPeerTransportError.nonCanonicalEncoding
        }
        guard archive.count <= KagemushaPeerTransportContract.maximumTextArchiveBytes else {
            throw KagemushaPeerTransportError.archiveTooLarge(
                actual: archive.count,
                maximum: KagemushaPeerTransportContract.maximumTextArchiveBytes
            )
        }
        return try KagemushaPeerPayload.decode(
            archive: archive,
            kind: kind,
            chainDiscriminant: chainDiscriminant
        )
    }

    /// Decodes user-presented scanner text after removing only ASCII SP, TAB,
    /// CR, and LF from the two boundaries. Embedded characters remain invalid.
    public static func decodeUserPresented(
        _ value: String,
        chainDiscriminant: UInt16,
        expectedKind: KagemushaPeerPayloadKind? = nil
    ) throws -> KagemushaPeerPayload {
        // Bound the scanner-owned string before boundary normalization.  In
        // particular, do not reverse or copy an attacker-controlled amount of
        // leading/trailing whitespace before applying the wire-size limit.
        let byteCount = value.utf8.count
        guard byteCount <= KagemushaPeerTransportContract.maximumTextEnvelopeBytes else {
            throw KagemushaPeerTransportError.textEnvelopeTooLarge(
                actual: byteCount,
                maximum: KagemushaPeerTransportContract.maximumTextEnvelopeBytes
            )
        }
        return try decode(
            canonicalizeUserPresented(value),
            chainDiscriminant: chainDiscriminant,
            expectedKind: expectedKind
        )
    }

    /// Performs the deliberately narrow scan-boundary normalization without
    /// interpreting or decoding the payload.
    public static func canonicalizeUserPresented(_ value: String) -> String {
        // Work on Unicode scalars rather than extended grapheme clusters:
        // Swift may present CRLF as one `Character`, but the transport contract
        // deliberately trims the two ASCII control bytes independently.
        let leadingTrimmed = value.unicodeScalars.drop(while: isBoundaryWhitespace)
        let trimmed = leadingTrimmed.reversed()
            .drop(while: isBoundaryWhitespace)
            .reversed()
        return String(decoding: trimmed.flatMap { String($0).utf8 }, as: UTF8.self)
    }

    public static func kind(of value: String) -> KagemushaPeerPayloadKind? {
        KagemushaPeerPayloadKind.allCases.first { value.hasPrefix($0.textPrefix) }
    }

    private static func isBoundaryWhitespace(_ scalar: Unicode.Scalar) -> Bool {
        scalar.value == 0x20 || scalar.value == 0x09
            || scalar.value == 0x0D || scalar.value == 0x0A
    }

    static func base64URLEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    static func base64URLDecode(_ value: String) -> Data? {
        guard !value.isEmpty,
              value.unicodeScalars.allSatisfy({ scalar in
                  switch scalar.value {
                  case 48...57, 65...90, 97...122, 45, 95:
                      return true
                  default:
                      return false
                  }
              }),
              value.utf8.count % 4 != 1 else {
            return nil
        }
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        normalized.append(String(repeating: "=", count: (4 - normalized.count % 4) % 4))
        guard let decoded = Data(base64Encoded: normalized),
              !decoded.isEmpty,
              base64URLEncode(decoded) == value else {
            return nil
        }
        return decoded
    }
}
