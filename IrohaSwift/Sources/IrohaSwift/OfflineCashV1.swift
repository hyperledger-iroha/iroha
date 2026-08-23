import Foundation

public enum OfflineCashV1Error: Error, Equatable, Sendable {
    case nativeBridgeABI22Unavailable
    case invalidCanonicalMessage
    case invalidPeerText
    case authenticatedReleaseUnavailable(String)
    case installedReleaseMismatch
    case invalidDigest(String)
    case invalidStateTransition(String)
}

private typealias OfflineCashFreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
private typealias OfflineCashBridgeABIFn = @convention(c) () -> UInt32
private typealias OfflineCashUnaryFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias OfflineCashBinaryFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias OfflineCashTernaryFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias OfflineCashReleaseProbeFn = @convention(c) (
    UnsafeMutablePointer<UInt8>?,
    UnsafeMutablePointer<UInt8>?, CUnsignedLong,
    UnsafeMutablePointer<UInt8>?, CUnsignedLong
) -> Int32

extension NoritoNativeBridge {
    private func copyOfflineCashOutput(
        status: Int32,
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong
    ) throws -> Data {
        guard let free = resolveKagemushaV2Symbol(
            "connect_norito_free",
            as: OfflineCashFreeFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        defer { if let pointer { free(pointer) } }
        if NativeBridgeError.fromStatus(status) != nil {
            throw OfflineCashV1Error.invalidCanonicalMessage
        }
        guard let pointer, length > 0, length <= 12_288 else {
            throw OfflineCashV1Error.invalidCanonicalMessage
        }
        return Data(bytes: pointer, count: Int(length))
    }

    fileprivate func offlineCashUnary(symbol: String, input: Data) throws -> Data {
        guard !input.isEmpty, input.count <= 12_288,
              let function = resolveKagemushaV2Symbol(symbol, as: OfflineCashUnaryFn.self)
        else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = input.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                &output,
                &outputLength
            )
        }
        return try copyOfflineCashOutput(
            status: status,
            pointer: output,
            length: outputLength
        )
    }

    fileprivate func offlineCashBinary(
        symbol: String,
        first: Data,
        second: Data
    ) throws -> Data {
        guard !first.isEmpty, !second.isEmpty,
              first.count <= 12_288, second.count <= 12_288,
              let function = resolveKagemushaV2Symbol(symbol, as: OfflineCashBinaryFn.self)
        else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = first.withUnsafeBytes { firstBuffer in
            second.withUnsafeBytes { secondBuffer in
                function(
                    firstBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(firstBuffer.count),
                    secondBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(secondBuffer.count),
                    &output,
                    &outputLength
                )
            }
        }
        return try copyOfflineCashOutput(
            status: status,
            pointer: output,
            length: outputLength
        )
    }

    fileprivate func offlineCashTernary(
        symbol: String,
        first: Data,
        second: Data,
        third: Data
    ) throws -> Data {
        guard !first.isEmpty, !second.isEmpty, !third.isEmpty,
              first.count <= 12_288, second.count <= 12_288, third.count <= 12_288,
              let function = resolveKagemushaV2Symbol(symbol, as: OfflineCashTernaryFn.self)
        else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = first.withUnsafeBytes { firstBuffer in
            second.withUnsafeBytes { secondBuffer in
                third.withUnsafeBytes { thirdBuffer in
                    function(
                        firstBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(firstBuffer.count),
                        secondBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(secondBuffer.count),
                        thirdBuffer.bindMemory(to: UInt8.self).baseAddress,
                        CUnsignedLong(thirdBuffer.count),
                        &output,
                        &outputLength
                    )
                }
            }
        }
        return try copyOfflineCashOutput(
            status: status,
            pointer: output,
            length: outputLength
        )
    }

    fileprivate func offlineCashReleaseStatusV1() -> OfflineCashReleaseStatusV1 {
        guard let abiFunction = resolveKagemushaV2Symbol(
            "connect_norito_bridge_abi_version",
            as: OfflineCashBridgeABIFn.self
        ), let probe = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_release_probe_v1",
            as: OfflineCashReleaseProbeFn.self
        ) else {
            return .unavailable(
                nativeBridgeABIVersion: nil,
                blocker: OfflineCashReleaseStatusV1.nativeABI22Unavailable
            )
        }
        let abi = abiFunction()
        guard abi == OfflineCashReleaseStatusV1.requiredNativeBridgeABIVersion else {
            return .unavailable(
                nativeBridgeABIVersion: abi,
                blocker: OfflineCashReleaseStatusV1.nativeABI22Unavailable
            )
        }
        var available: UInt8 = 0
        var release = [UInt8](repeating: 0, count: 32)
        var artifactManifest = [UInt8](repeating: 0, count: 32)
        let status = release.withUnsafeMutableBufferPointer { releaseBuffer in
            artifactManifest.withUnsafeMutableBufferPointer { manifestBuffer in
                probe(
                    &available,
                    releaseBuffer.baseAddress,
                    CUnsignedLong(releaseBuffer.count),
                    manifestBuffer.baseAddress,
                    CUnsignedLong(manifestBuffer.count)
                )
            }
        }
        guard status == 0, available == 1,
              release.contains(where: { $0 != 0 }),
              artifactManifest.contains(where: { $0 != 0 })
        else {
            return .unavailable(
                nativeBridgeABIVersion: abi,
                blocker: OfflineCashReleaseStatusV1.authenticatedReleaseUnavailable
            )
        }
        return OfflineCashReleaseStatusV1(
            available: true,
            nativeBridgeABIVersion: abi,
            installedReleaseId: Data(release),
            installedArtifactManifestSHA256: Data(artifactManifest),
            blocker: nil
        )
    }
}

public struct OfflineCashPaymentRequestV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 768
    public let canonicalNorito: Data

    public init(canonicalNorito: Data) throws {
        guard canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashV1Error.invalidCanonicalMessage
        }
        self.canonicalNorito = try NoritoNativeBridge.shared.offlineCashUnary(
            symbol: "connect_norito_offline_cash_payment_request_canonicalize_v1",
            input: canonicalNorito
        )
    }

    public static func decodeCanonical(_ canonicalNorito: Data) throws -> Self {
        try Self(canonicalNorito: canonicalNorito)
    }

    public func encodeCanonical() -> Data { canonicalNorito }
}

public struct OfflineCashPaymentV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 7_936
    public let request: OfflineCashPaymentRequestV1
    public let canonicalNorito: Data

    public init(request: OfflineCashPaymentRequestV1, canonicalNorito: Data) throws {
        guard canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashV1Error.invalidCanonicalMessage
        }
        self.request = request
        self.canonicalNorito = try NoritoNativeBridge.shared.offlineCashBinary(
            symbol: "connect_norito_offline_cash_payment_canonicalize_v1",
            first: request.canonicalNorito,
            second: canonicalNorito
        )
    }

    public static func decodeCanonical(
        request: OfflineCashPaymentRequestV1,
        canonicalNorito: Data
    ) throws -> Self {
        try Self(request: request, canonicalNorito: canonicalNorito)
    }

    public func encodeCanonical() -> Data { canonicalNorito }
}

public struct OfflineCashAcknowledgementV1: Equatable, Sendable {
    public static let maximumCanonicalBytes = 256
    public let request: OfflineCashPaymentRequestV1
    public let payment: OfflineCashPaymentV1
    public let canonicalNorito: Data

    public init(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        canonicalNorito: Data
    ) throws {
        guard canonicalNorito.count <= Self.maximumCanonicalBytes else {
            throw OfflineCashV1Error.invalidCanonicalMessage
        }
        self.request = request
        self.payment = payment
        self.canonicalNorito = try NoritoNativeBridge.shared.offlineCashTernary(
            symbol: "connect_norito_offline_cash_acknowledgement_canonicalize_v1",
            first: request.canonicalNorito,
            second: payment.canonicalNorito,
            third: canonicalNorito
        )
    }

    public static func decodeCanonical(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        canonicalNorito: Data
    ) throws -> Self {
        try Self(request: request, payment: payment, canonicalNorito: canonicalNorito)
    }

    public func encodeCanonical() -> Data { canonicalNorito }
}

public struct OfflineCashReleaseStatusV1: Equatable, Sendable {
    public static let requiredNativeBridgeABIVersion: UInt32 = 22
    public static let nativeABI22Unavailable = "offline-cash-v1-native-abi22-unavailable"
    public static let authenticatedReleaseUnavailable =
        "offline-cash-v1-authenticated-release-unavailable"

    public let available: Bool
    public let nativeBridgeABIVersion: UInt32?
    public let installedReleaseId: Data?
    public let installedArtifactManifestSHA256: Data?
    public let blocker: String?

    public static func installed() -> Self {
        NoritoNativeBridge.shared.offlineCashReleaseStatusV1()
    }

    fileprivate static func unavailable(
        nativeBridgeABIVersion: UInt32?,
        blocker: String
    ) -> Self {
        Self(
            available: false,
            nativeBridgeABIVersion: nativeBridgeABIVersion,
            installedReleaseId: nil,
            installedArtifactManifestSHA256: nil,
            blocker: blocker
        )
    }

    public func matches(
        expectedReleaseId: Data,
        expectedArtifactManifestSHA256: Data
    ) -> Bool {
        available && expectedReleaseId.count == 32 &&
            expectedArtifactManifestSHA256.count == 32 &&
            installedReleaseId == expectedReleaseId &&
            installedArtifactManifestSHA256 == expectedArtifactManifestSHA256
    }
}

public enum OfflineCashWalletSessionStateV1: String, Equatable, Sendable {
    case receiveRequestReady
    case paymentCommitted
    case acknowledged
}

public enum OfflineCashWalletSessionEventV1: String, Equatable, Sendable {
    case paymentCommitted
    case paymentReplay
    case acknowledged
    case acknowledgementReplay
}

/// Opaque, monotonic and fail-closed receiver session. No production bypass is exposed.
public final class OfflineCashWalletSessionV1 {
    public let request: OfflineCashPaymentRequestV1
    public let expectedReleaseId: Data
    public let expectedArtifactManifestSHA256: Data
    private let lock = NSLock()
    private var committedPayment: OfflineCashPaymentV1?
    private var acceptedAcknowledgement: OfflineCashAcknowledgementV1?

    public init(
        request: OfflineCashPaymentRequestV1,
        expectedReleaseId: Data,
        expectedArtifactManifestSHA256: Data
    ) throws {
        guard expectedReleaseId.count == 32,
              expectedReleaseId.contains(where: { $0 != 0 }) else {
            throw OfflineCashV1Error.invalidDigest("expectedReleaseId")
        }
        guard expectedArtifactManifestSHA256.count == 32,
              expectedArtifactManifestSHA256.contains(where: { $0 != 0 }) else {
            throw OfflineCashV1Error.invalidDigest("expectedArtifactManifestSHA256")
        }
        let status = OfflineCashReleaseStatusV1.installed()
        guard status.available else {
            throw OfflineCashV1Error.authenticatedReleaseUnavailable(
                status.blocker ?? OfflineCashReleaseStatusV1.authenticatedReleaseUnavailable
            )
        }
        guard status.matches(
            expectedReleaseId: expectedReleaseId,
            expectedArtifactManifestSHA256: expectedArtifactManifestSHA256
        ) else {
            throw OfflineCashV1Error.installedReleaseMismatch
        }
        self.request = request
        self.expectedReleaseId = expectedReleaseId
        self.expectedArtifactManifestSHA256 = expectedArtifactManifestSHA256
    }

    public var state: OfflineCashWalletSessionStateV1 {
        lock.lock()
        defer { lock.unlock() }
        if acceptedAcknowledgement != nil { return .acknowledged }
        if committedPayment != nil { return .paymentCommitted }
        return .receiveRequestReady
    }

    public var payment: OfflineCashPaymentV1? {
        lock.lock()
        defer { lock.unlock() }
        return committedPayment
    }

    public var acknowledgement: OfflineCashAcknowledgementV1? {
        lock.lock()
        defer { lock.unlock() }
        return acceptedAcknowledgement
    }

    public func acceptPayment(
        canonicalNorito: Data
    ) throws -> OfflineCashWalletSessionEventV1 {
        let sessionCanonical = try NoritoNativeBridge.shared.offlineCashTernary(
            symbol: "connect_norito_offline_cash_payment_canonicalize_for_session_v1",
            first: request.canonicalNorito,
            second: canonicalNorito,
            third: expectedArtifactManifestSHA256
        )
        let candidate = try OfflineCashPaymentV1(
            request: request,
            canonicalNorito: sessionCanonical
        )
        lock.lock()
        defer { lock.unlock() }
        guard acceptedAcknowledgement == nil else {
            throw OfflineCashV1Error.invalidStateTransition("payment_after_acknowledgement")
        }
        if let committedPayment {
            guard committedPayment == candidate else {
                throw OfflineCashV1Error.invalidStateTransition("conflicting_payment")
            }
            return .paymentReplay
        }
        committedPayment = candidate
        return .paymentCommitted
    }

    public func acceptAcknowledgement(
        canonicalNorito: Data
    ) throws -> OfflineCashWalletSessionEventV1 {
        lock.lock()
        guard let payment = committedPayment else {
            lock.unlock()
            throw OfflineCashV1Error.invalidStateTransition("acknowledgement_before_payment")
        }
        lock.unlock()
        let candidate = try OfflineCashAcknowledgementV1(
            request: request,
            payment: payment,
            canonicalNorito: canonicalNorito
        )
        lock.lock()
        defer { lock.unlock() }
        if let acceptedAcknowledgement {
            guard acceptedAcknowledgement == candidate else {
                throw OfflineCashV1Error.invalidStateTransition("conflicting_acknowledgement")
            }
            return .acknowledgementReplay
        }
        acceptedAcknowledgement = candidate
        return .acknowledged
    }
}

/// Strict `kgm2:` peer adapter. It is intentionally disjoint from PKK1 transport.
public struct OfflineCashPeerAdapterV1: Sendable {
    public static let textPrefix = "kgm2:"
    public static let maximumTextSessionBytes = 12_288

    public init() {}

    public func encodePaymentRequest(_ request: OfflineCashPaymentRequestV1) throws -> String {
        try text(
            NoritoNativeBridge.shared.offlineCashUnary(
                symbol: "connect_norito_offline_cash_peer_encode_payment_request_v1",
                input: request.canonicalNorito
            )
        )
    }

    public func decodePaymentRequest(_ text: String) throws -> OfflineCashPaymentRequestV1 {
        try OfflineCashPaymentRequestV1(
            canonicalNorito: peerDecodeUnary(
                symbol: "connect_norito_offline_cash_peer_decode_payment_request_v1",
                text: text
            )
        )
    }

    public func encodePayment(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1
    ) throws -> String {
        try text(
            NoritoNativeBridge.shared.offlineCashBinary(
                symbol: "connect_norito_offline_cash_peer_encode_payment_v1",
                first: request.canonicalNorito,
                second: payment.canonicalNorito
            )
        )
    }

    public func decodePayment(
        request: OfflineCashPaymentRequestV1,
        text: String
    ) throws -> OfflineCashPaymentV1 {
        guard let bytes = text.data(using: .utf8), bytes.count <= Self.maximumTextSessionBytes else {
            throw OfflineCashV1Error.invalidPeerText
        }
        let canonical = try NoritoNativeBridge.shared.offlineCashBinary(
            symbol: "connect_norito_offline_cash_peer_decode_payment_v1",
            first: request.canonicalNorito,
            second: bytes
        )
        return try OfflineCashPaymentV1(request: request, canonicalNorito: canonical)
    }

    public func encodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1
    ) throws -> String {
        try text(
            NoritoNativeBridge.shared.offlineCashTernary(
                symbol: "connect_norito_offline_cash_peer_encode_acknowledgement_v1",
                first: request.canonicalNorito,
                second: payment.canonicalNorito,
                third: acknowledgement.canonicalNorito
            )
        )
    }

    public func decodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        text: String
    ) throws -> OfflineCashAcknowledgementV1 {
        guard let bytes = text.data(using: .utf8), bytes.count <= Self.maximumTextSessionBytes else {
            throw OfflineCashV1Error.invalidPeerText
        }
        let canonical = try NoritoNativeBridge.shared.offlineCashTernary(
            symbol: "connect_norito_offline_cash_peer_decode_acknowledgement_v1",
            first: request.canonicalNorito,
            second: payment.canonicalNorito,
            third: bytes
        )
        return try OfflineCashAcknowledgementV1(
            request: request,
            payment: payment,
            canonicalNorito: canonical
        )
    }

    private func peerDecodeUnary(symbol: String, text: String) throws -> Data {
        guard let bytes = text.data(using: .utf8), bytes.count <= Self.maximumTextSessionBytes else {
            throw OfflineCashV1Error.invalidPeerText
        }
        return try NoritoNativeBridge.shared.offlineCashUnary(symbol: symbol, input: bytes)
    }

    private func text(_ data: Data) throws -> String {
        guard data.count <= Self.maximumTextSessionBytes,
              let value = String(data: data, encoding: .utf8),
              value.hasPrefix(Self.textPrefix) else {
            throw OfflineCashV1Error.invalidPeerText
        }
        return value
    }
}
