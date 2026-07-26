import Foundation

public enum IrohaPeerCanonicalTextPayloadCodecErrorV1: Error, Equatable, LocalizedError, Sendable {
    case unsupportedProfile(IrohaPeerWireProfileV1)
    case emptyPayload
    case payloadTooLarge(
        profile: IrohaPeerWireProfileV1,
        actual: Int,
        maximum: Int
    )
    case invalidUTF8

    public var errorDescription: String? {
        switch self {
        case .unsupportedProfile(let profile):
            return "IPM1 profile \(profile) does not use the generic canonical-text codec."
        case .emptyPayload:
            return "IPM1 canonical text must not be empty."
        case let .payloadTooLarge(profile, actual, maximum):
            return "IPM1 profile \(profile) canonical text is \(actual) bytes; the uncompressed limit is \(maximum) bytes."
        case .invalidUTF8:
            return "IPM1 canonical text payload is not exact UTF-8."
        }
    }
}

/// Exact UTF-8 conversion for Offline Note payloads.
///
/// Kagemusha profile 2 is a typed native archive and must use
/// `KagemushaPeerPayload`/`IrohaPeerKagemushaAdapterV1`; admitting it here
/// would bypass that archive validation boundary.
public enum IrohaPeerCanonicalTextPayloadCodecV1 {
    public static func maximumCanonicalTextBytes(
        for profile: IrohaPeerWireProfileV1,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> Int {
        try requireOfflineNote(profile)
        return min(
            limits.maximumCanonicalBytes,
            try limits.maximumEncodedBytes(for: profile)
        )
    }

    public static func canonicalBytes(
        for text: String,
        profile: IrohaPeerWireProfileV1,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> Data {
        guard !text.isEmpty else {
            throw IrohaPeerCanonicalTextPayloadCodecErrorV1.emptyPayload
        }
        let bytes = Data(text.utf8)
        try requireValidLength(bytes.count, profile: profile, limits: limits)
        return bytes
    }

    public static func canonicalText(
        from bytes: Data,
        profile: IrohaPeerWireProfileV1,
        limits: IrohaPeerWireLimitsV1 = .peerV1
    ) throws -> String {
        guard !bytes.isEmpty else {
            throw IrohaPeerCanonicalTextPayloadCodecErrorV1.emptyPayload
        }
        try requireValidLength(bytes.count, profile: profile, limits: limits)
        guard let text = String(data: bytes, encoding: .utf8),
              Data(text.utf8) == bytes else {
            throw IrohaPeerCanonicalTextPayloadCodecErrorV1.invalidUTF8
        }
        return text
    }

    private static func requireValidLength(
        _ byteCount: Int,
        profile: IrohaPeerWireProfileV1,
        limits: IrohaPeerWireLimitsV1
    ) throws {
        let maximum = try maximumCanonicalTextBytes(for: profile, limits: limits)
        guard byteCount <= maximum else {
            throw IrohaPeerCanonicalTextPayloadCodecErrorV1.payloadTooLarge(
                profile: profile,
                actual: byteCount,
                maximum: maximum
            )
        }
    }

    private static func requireOfflineNote(
        _ profile: IrohaPeerWireProfileV1
    ) throws {
        guard profile == .offlineNote else {
            throw IrohaPeerCanonicalTextPayloadCodecErrorV1.unsupportedProfile(profile)
        }
    }
}
