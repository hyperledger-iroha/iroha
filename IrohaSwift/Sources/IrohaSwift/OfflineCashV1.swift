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
private typealias OfflineCashWalletSessionOpenBoundFn = @convention(c) (
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafeMutablePointer<UInt64>?
) -> Int32
private typealias OfflineCashWalletSessionAcceptPaymentFn = @convention(c) (
    UInt64,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UInt64,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias OfflineCashWalletSessionAcceptAcknowledgementFn = @convention(c) (
    UInt64,
    UnsafePointer<UInt8>?, CUnsignedLong,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias OfflineCashWalletSessionStateFn = @convention(c) (
    UInt64, UnsafeMutablePointer<UInt8>?
) -> Int32
private typealias OfflineCashWalletSessionCloseFn = @convention(c) (UInt64) -> Int32

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

    fileprivate func offlineCashWalletSessionOpenBoundV1(
        request: Data,
        expectedReleaseId: Data,
        expectedArtifactManifestSHA256: Data,
        expectedNetworkIDLiteral: Data,
        expectedAssetDefinitionID: Data
    ) throws -> UInt64 {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_wallet_session_open_bound_v1",
            as: OfflineCashWalletSessionOpenBoundFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var handle: UInt64 = 0
        let status = request.withUnsafeBytes { requestBuffer in
            expectedReleaseId.withUnsafeBytes { releaseBuffer in
                expectedArtifactManifestSHA256.withUnsafeBytes { manifestBuffer in
                    expectedNetworkIDLiteral.withUnsafeBytes { networkBuffer in
                        expectedAssetDefinitionID.withUnsafeBytes { assetBuffer in
                            function(
                                requestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(requestBuffer.count),
                                releaseBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(releaseBuffer.count),
                                manifestBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(manifestBuffer.count),
                                networkBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(networkBuffer.count),
                                assetBuffer.bindMemory(to: UInt8.self).baseAddress,
                                CUnsignedLong(assetBuffer.count),
                                &handle
                            )
                        }
                    }
                }
            }
        }
        guard status == 0, handle != 0 else {
            throw OfflineCashV1Error.installedReleaseMismatch
        }
        return handle
    }

    fileprivate func offlineCashWalletSessionAcceptPaymentV1(
        handle: UInt64,
        payment: Data,
        observedNowMilliseconds: UInt64
    ) throws -> Data {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_wallet_session_accept_payment_v1",
            as: OfflineCashWalletSessionAcceptPaymentFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = payment.withUnsafeBytes { buffer in
            function(
                handle,
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                observedNowMilliseconds,
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

    fileprivate func offlineCashWalletSessionAcceptAcknowledgementV1(
        handle: UInt64,
        acknowledgement: Data
    ) throws -> Data {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_wallet_session_accept_acknowledgement_v1",
            as: OfflineCashWalletSessionAcceptAcknowledgementFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = acknowledgement.withUnsafeBytes { buffer in
            function(
                handle,
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

    fileprivate func offlineCashWalletSessionStateV1(handle: UInt64) throws -> UInt8 {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_wallet_session_state_v1",
            as: OfflineCashWalletSessionStateFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        var state: UInt8 = 0
        guard function(handle, &state) == 0, (1...3).contains(state) else {
            throw OfflineCashV1Error.invalidStateTransition("native_session_unavailable")
        }
        return state
    }

    fileprivate func offlineCashWalletSessionCloseV1(handle: UInt64) throws {
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_offline_cash_wallet_session_close_v1",
            as: OfflineCashWalletSessionCloseFn.self
        ) else {
            throw OfflineCashV1Error.nativeBridgeABI22Unavailable
        }
        guard function(handle) == 0 else {
            throw OfflineCashV1Error.invalidStateTransition("native_session_close_rejected")
        }
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
    case unavailable
    case receiveRequestReady
    case paymentVerified
    case acknowledgementVerified
}

public enum OfflineCashWalletSessionEventV1: String, Equatable, Sendable {
    case paymentVerified
    case paymentVerificationReplay
    case acknowledgementVerified
    case acknowledgementVerificationReplay
}

/// Opaque, monotonic and fail-closed receiver proof-verification session.
///
/// States `paymentVerified` and `acknowledgementVerified` describe only native cryptographic
/// verification and retention inside this process. They do not mean that the secure-device
/// journal, exact-next counter, wallet balance, payment outbox, or acknowledgement store was
/// mutated durably. Only the sealed Core lifecycle joined to a qualifying device backend may
/// authorize those effects. No production bypass is exposed.
public final class OfflineCashWalletSessionV1 {
    public let request: OfflineCashPaymentRequestV1
    public let expectedReleaseId: Data
    public let expectedArtifactManifestSHA256: Data
    public let expectedNetworkID: NetworkId
    public let expectedAssetDefinitionID: String
    private let lock = NSLock()
    private var nativeHandle: UInt64?
    private var verifiedPayment: OfflineCashPaymentV1?
    private var verifiedAcknowledgement: OfflineCashAcknowledgementV1?

    public init(
        request: OfflineCashPaymentRequestV1,
        expectedReleaseId: Data,
        expectedArtifactManifestSHA256: Data,
        expectedNetworkID: NetworkId,
        expectedAssetDefinitionID: String
    ) throws {
        guard expectedReleaseId.count == 32,
              expectedReleaseId.contains(where: { $0 != 0 }) else {
            throw OfflineCashV1Error.invalidDigest("expectedReleaseId")
        }
        guard expectedArtifactManifestSHA256.count == 32,
              expectedArtifactManifestSHA256.contains(where: { $0 != 0 }) else {
            throw OfflineCashV1Error.invalidDigest("expectedArtifactManifestSHA256")
        }
        let assetBytes = Data(expectedAssetDefinitionID.utf8)
        let isCanonicalBase58 = !assetBytes.isEmpty && assetBytes.count <= 64
            && assetBytes.allSatisfy { byte in
                (byte >= 49 && byte <= 57)
                    || (byte >= 65 && byte <= 72)
                    || (byte >= 74 && byte <= 78)
                    || (byte >= 80 && byte <= 90)
                    || (byte >= 97 && byte <= 107)
                    || (byte >= 109 && byte <= 122)
            }
        guard isCanonicalBase58 else {
            throw OfflineCashV1Error.invalidCanonicalMessage
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
        let nativeHandle = try NoritoNativeBridge.shared.offlineCashWalletSessionOpenBoundV1(
            request: request.canonicalNorito,
            expectedReleaseId: expectedReleaseId,
            expectedArtifactManifestSHA256: expectedArtifactManifestSHA256,
            expectedNetworkIDLiteral: Data(expectedNetworkID.literal.utf8),
            expectedAssetDefinitionID: assetBytes
        )
        self.request = request
        self.expectedReleaseId = expectedReleaseId
        self.expectedArtifactManifestSHA256 = expectedArtifactManifestSHA256
        self.expectedNetworkID = expectedNetworkID
        self.expectedAssetDefinitionID = expectedAssetDefinitionID
        self.nativeHandle = nativeHandle
    }

    deinit {
        if let nativeHandle {
            try? NoritoNativeBridge.shared.offlineCashWalletSessionCloseV1(handle: nativeHandle)
        }
    }

    public var state: OfflineCashWalletSessionStateV1 {
        lock.lock()
        defer { lock.unlock() }
        guard let nativeHandle,
              let state = try? NoritoNativeBridge.shared.offlineCashWalletSessionStateV1(
                handle: nativeHandle
              ) else {
            return .unavailable
        }
        switch state {
        case 1: return .receiveRequestReady
        case 2: return .paymentVerified
        case 3: return .acknowledgementVerified
        default: return .unavailable
        }
    }

    public var payment: OfflineCashPaymentV1? {
        lock.lock()
        defer { lock.unlock() }
        return verifiedPayment
    }

    public var acknowledgement: OfflineCashAcknowledgementV1? {
        lock.lock()
        defer { lock.unlock() }
        return verifiedAcknowledgement
    }

    public func acceptPayment(
        canonicalNorito: Data
    ) throws -> OfflineCashWalletSessionEventV1 {
        lock.lock()
        defer { lock.unlock() }
        guard let nativeHandle else {
            throw OfflineCashV1Error.invalidStateTransition("session_closed")
        }
        let observed = Date().timeIntervalSince1970 * 1_000
        guard observed.isFinite, observed > 0, observed <= Double(UInt64.max) else {
            throw OfflineCashV1Error.invalidStateTransition("invalid_observed_time")
        }
        let sessionCanonical = try NoritoNativeBridge.shared
            .offlineCashWalletSessionAcceptPaymentV1(
                handle: nativeHandle,
                payment: canonicalNorito,
                observedNowMilliseconds: UInt64(observed.rounded(.down))
            )
        let candidate = try OfflineCashPaymentV1(
            request: request,
            canonicalNorito: sessionCanonical
        )
        if let verifiedPayment {
            guard verifiedPayment == candidate else {
                throw OfflineCashV1Error.invalidStateTransition("conflicting_payment")
            }
            return .paymentVerificationReplay
        }
        verifiedPayment = candidate
        return .paymentVerified
    }

    public func acceptAcknowledgement(
        canonicalNorito: Data
    ) throws -> OfflineCashWalletSessionEventV1 {
        lock.lock()
        defer { lock.unlock() }
        guard let nativeHandle else {
            throw OfflineCashV1Error.invalidStateTransition("session_closed")
        }
        guard let payment = verifiedPayment else {
            throw OfflineCashV1Error.invalidStateTransition("acknowledgement_before_payment")
        }
        let sessionCanonical = try NoritoNativeBridge.shared
            .offlineCashWalletSessionAcceptAcknowledgementV1(
                handle: nativeHandle,
                acknowledgement: canonicalNorito
            )
        let candidate = try OfflineCashAcknowledgementV1(
            request: request,
            payment: payment,
            canonicalNorito: sessionCanonical
        )
        if let verifiedAcknowledgement {
            guard verifiedAcknowledgement == candidate else {
                throw OfflineCashV1Error.invalidStateTransition("conflicting_acknowledgement")
            }
            return .acknowledgementVerificationReplay
        }
        verifiedAcknowledgement = candidate
        return .acknowledgementVerified
    }

    public func close() throws {
        lock.lock()
        defer { lock.unlock() }
        guard let nativeHandle else { return }
        try NoritoNativeBridge.shared.offlineCashWalletSessionCloseV1(handle: nativeHandle)
        self.nativeHandle = nil
    }
}

/// Strict `kgm2:` peer adapter. It is intentionally disjoint from PKK1 transport.
public struct OfflineCashPeerAdapterV1: Sendable {
    public static let textPrefix = "kgm2:"
    public static let maximumRawSessionBytes = 9_211
    public static let maximumTextSessionBytes = 12_288
    public static let maximumPaymentRequestTextBytes = 1_029
    public static let maximumPaymentTextBytes = 10_587
    public static let maximumAcknowledgementTextBytes = 347

    public init() {}

    public func encodePaymentRequest(_ request: OfflineCashPaymentRequestV1) throws -> String {
        try text(
            NoritoNativeBridge.shared.offlineCashUnary(
                symbol: "connect_norito_offline_cash_peer_encode_payment_request_v1",
                input: request.canonicalNorito
            ),
            maximumTextBytes: Self.maximumPaymentRequestTextBytes
        )
    }

    public func decodePaymentRequest(_ text: String) throws -> OfflineCashPaymentRequestV1 {
        try OfflineCashPaymentRequestV1(
            canonicalNorito: peerDecodeUnary(
                symbol: "connect_norito_offline_cash_peer_decode_payment_request_v1",
                text: text,
                maximumTextBytes: Self.maximumPaymentRequestTextBytes
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
            ),
            maximumTextBytes: Self.maximumPaymentTextBytes
        )
    }

    public func decodePayment(
        request: OfflineCashPaymentRequestV1,
        text: String
    ) throws -> OfflineCashPaymentV1 {
        guard let bytes = text.data(using: .utf8),
              bytes.count <= Self.maximumPaymentTextBytes else {
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
            ),
            maximumTextBytes: Self.maximumAcknowledgementTextBytes
        )
    }

    public func decodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        text: String
    ) throws -> OfflineCashAcknowledgementV1 {
        guard let bytes = text.data(using: .utf8),
              bytes.count <= Self.maximumAcknowledgementTextBytes else {
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

    private func peerDecodeUnary(
        symbol: String,
        text: String,
        maximumTextBytes: Int
    ) throws -> Data {
        guard let bytes = text.data(using: .utf8), bytes.count <= maximumTextBytes else {
            throw OfflineCashV1Error.invalidPeerText
        }
        return try NoritoNativeBridge.shared.offlineCashUnary(symbol: symbol, input: bytes)
    }

    private func text(_ data: Data, maximumTextBytes: Int) throws -> String {
        guard data.count <= maximumTextBytes,
              let value = String(data: data, encoding: .utf8),
              value.hasPrefix(Self.textPrefix) else {
            throw OfflineCashV1Error.invalidPeerText
        }
        return value
    }
}
