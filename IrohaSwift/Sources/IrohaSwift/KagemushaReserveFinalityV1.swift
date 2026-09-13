import Foundation
import CoreFoundation

/// Independently authenticated coordinates. A response lookup hint is never their trust source.
public struct KagemushaFinalityTrustAnchorV1: Equatable, Sendable {
    public let networkID: Data
    public let blockHeight: UInt64
    public let heightContextID: Data

    public init(networkID: Data, blockHeight: UInt64, heightContextID: Data) throws {
        self.networkID = try reserveFinalityHash(networkID)
        self.heightContextID = try reserveFinalityHash(heightContextID)
        guard blockHeight > 0 else { throw KagemushaReserveFinalityError.invalidInput }
        self.blockHeight = blockHeight
    }
}

/// Routing metadata decoded by native Core, carrying no monetary authority or anchor conversion.
public struct KagemushaUntrustedFinalityHintV1: Equatable, Sendable {
    public let networkID: Data
    public let blockHeight: UInt64
    public let heightContextID: Data
}

public enum KagemushaReserveFinalityError: Error, Equatable {
    case invalidInput
    case invalidProjection
    case bridgeUnavailable
    case verificationRejected
}

/// One native certificate and reserve-witness verifier shared with Android.
public enum KagemushaReserveFinalityV1 {
    /// Requires the whole verifier symbol cohort and the exact current bridge ABI.
    /// Availability establishes no network trust, hardware qualification or monetary readiness.
    public static var isAvailable: Bool {
        NoritoNativeBridge.shared.isReserveFinalityAvailable
    }

    public static func hint(_ status: ToriiUnverifiedKagemushaOperationStatusV1) throws
        -> KagemushaUntrustedFinalityHintV1? {
        try status.verifyAgainst(()) { json, _ in
            try parseReserveFinalityHint(NoritoNativeBridge.shared.reserveFinalityHint(json))
        }
    }

    /// Authenticate the entire saved top-up before exposing its existing canonical credit.
    /// Qualified local Core must still admit its exact release, hardware profile and proof.
    public static func verifyTopUp(
        _ status: ToriiUnverifiedKagemushaOperationStatusV1,
        expectedRequest: KagemushaTopUpRequestV1,
        trustAnchor: KagemushaFinalityTrustAnchorV1
    ) throws -> KagemushaMintCreditV1 {
        try status.verifyAgainst(trustAnchor) { json, anchor in
            let request = try KagemushaNoritoV1.encodeTopUpRequestShape(expectedRequest)
            let output = try NoritoNativeBridge.shared.reserveFinalityVerify(
                json, kind: 0, request: request, anchor: anchor,
                maximum: KagemushaWireV1.maximumMintCreditBytes)
            return try KagemushaNoritoV1.decodeMintCreditShapeExact(output, against: expectedRequest.mintAuthorization)
        }
    }

    /// Authenticate the entire saved redemption and return its exact finalized voucher.
    /// Persist original status evidence and independent anchor provenance before native retirement.
    public static func verifyRedemption(
        _ status: ToriiUnverifiedKagemushaOperationStatusV1,
        expectedRequest: KagemushaRedemptionRequestV1,
        trustAnchor: KagemushaFinalityTrustAnchorV1
    ) throws -> KagemushaRedemptionVoucherV1 {
        try status.verifyAgainst(trustAnchor) { json, anchor in
            let request = try KagemushaNoritoV1.encodeRedemptionRequestShape(expectedRequest)
            let output = try NoritoNativeBridge.shared.reserveFinalityVerify(
                json, kind: 1, request: request, anchor: anchor,
                maximum: KagemushaWireV1.maximumRedemptionVoucherBytes)
            guard output == (try KagemushaNoritoV1.encodeRedemptionVoucherShape(expectedRequest.voucher)) else {
                throw KagemushaReserveFinalityError.invalidProjection
            }
            return try KagemushaNoritoV1.decodeRedemptionVoucherShapeExact(output)
        }
    }
}

private func reserveFinalityHash(_ value: Data) throws -> Data {
    guard value.count == 32, let last = value.last, last & 1 == 1 else {
        throw KagemushaReserveFinalityError.invalidInput
    }
    return Data(value)
}

func parseReserveFinalityHint(_ bytes: Data) throws -> KagemushaUntrustedFinalityHintV1? {
    guard !bytes.isEmpty, bytes.count <= 512,
          String(data: bytes, encoding: .utf8) != nil else {
        throw KagemushaReserveFinalityError.invalidProjection
    }
    try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: bytes, integerKeys: ["version"])
    let parsed = try JSONSerialization.jsonObject(with: bytes, options: [.fragmentsAllowed])
    if parsed is NSNull { return nil }
    guard let value = parsed as? [String: Any],
          Set(value.keys) == ["version", "network_id", "block_height", "height_context_id"],
          let version = value["version"] as? NSNumber,
          CFGetTypeID(version) != CFBooleanGetTypeID(), version.intValue == 1, version.doubleValue == 1,
          let height = value["block_height"] as? String, height.utf8.allSatisfy({ (48...57).contains($0) }),
          let number = UInt64(height), number > 0, String(number) == height else {
        throw KagemushaReserveFinalityError.invalidProjection
    }
    func hash(_ key: String) throws -> Data {
        guard let encoded = value[key] as? String, encoded.utf8.count == 64 else {
            throw KagemushaReserveFinalityError.invalidProjection
        }
        let chars = Array(encoded.utf8)
        guard chars.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else {
            throw KagemushaReserveFinalityError.invalidProjection
        }
        func digit(_ value: UInt8) -> UInt8 { value < 58 ? value - 48 : value - 87 }
        let bytes = Data(stride(from: 0, to: 64, by: 2).map { digit(chars[$0]) * 16 + digit(chars[$0 + 1]) })
        return try reserveFinalityHash(bytes)
    }
    return try KagemushaUntrustedFinalityHintV1(
        networkID: hash("network_id"), blockHeight: number, heightContextID: hash("height_context_id"))
}

extension NoritoNativeBridge {
    var isReserveFinalityAvailable: Bool {
        #if canImport(Darwin)
        return (try? requireReserveFinalityAbi()) != nil
        #else
        return false
        #endif
    }

    #if canImport(Darwin)
    private typealias ReserveHintFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias ReserveVerifyFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UInt8,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong, UInt64,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias ReserveFreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    private typealias ReserveAbiFn = @convention(c) () -> UInt32

    private func requireReserveFinalityAbi() throws {
        guard isAvailable,
              let version = resolveNativeSymbol("connect_norito_bridge_abi_version", as: ReserveAbiFn.self),
              version() == NoritoBridgeLoader.expectedBridgeAbiVersion,
              resolveNativeSymbol("connect_norito_kagemusha_reserve_finality_hint_v1", as: ReserveHintFn.self) != nil,
              resolveNativeSymbol("connect_norito_kagemusha_reserve_finality_verify_v1", as: ReserveVerifyFn.self) != nil,
              resolveNativeSymbol("connect_norito_free", as: ReserveFreeFn.self) != nil else {
            throw KagemushaReserveFinalityError.bridgeUnavailable
        }
    }

    private func copyReserveFinalityOutput(status: Int32, pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong, maximum: Int, free: ReserveFreeFn) throws -> Data {
        defer { if let pointer { free(pointer) } }
        guard status == 0 else { throw KagemushaReserveFinalityError.verificationRejected }
        guard let pointer, length > 0, length <= CUnsignedLong(maximum) else {
            throw KagemushaReserveFinalityError.invalidProjection
        }
        return Data(bytes: pointer, count: Int(length))
    }
    #endif

    func reserveFinalityHint(_ response: Data) throws -> Data {
        guard !response.isEmpty, response.count <= 16 * 1024 * 1024 else {
            throw KagemushaReserveFinalityError.invalidInput
        }
        #if canImport(Darwin)
        try requireReserveFinalityAbi()
        guard let function = resolveNativeSymbol("connect_norito_kagemusha_reserve_finality_hint_v1", as: ReserveHintFn.self),
              let free = resolveNativeSymbol("connect_norito_free", as: ReserveFreeFn.self) else {
            throw KagemushaReserveFinalityError.bridgeUnavailable
        }
        var output: UnsafeMutablePointer<UInt8>?; var length: CUnsignedLong = 0
        let status = response.withUnsafeBytes { bytes in
            function(bytes.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(bytes.count), &output, &length)
        }
        return try copyReserveFinalityOutput(status: status, pointer: output, length: length, maximum: 512, free: free)
        #else
        throw KagemushaReserveFinalityError.bridgeUnavailable
        #endif
    }

    func reserveFinalityVerify(_ response: Data, kind: UInt8, request: Data,
        anchor: KagemushaFinalityTrustAnchorV1, maximum: Int) throws -> Data {
        guard !response.isEmpty, response.count <= 16 * 1024 * 1024,
              kind <= 1, !request.isEmpty, request.count <= (kind == 0 ? 16 * 1024 : 8 * 1024) else {
            throw KagemushaReserveFinalityError.invalidInput
        }
        #if canImport(Darwin)
        try requireReserveFinalityAbi()
        guard let function = resolveNativeSymbol("connect_norito_kagemusha_reserve_finality_verify_v1", as: ReserveVerifyFn.self),
              let free = resolveNativeSymbol("connect_norito_free", as: ReserveFreeFn.self) else {
            throw KagemushaReserveFinalityError.bridgeUnavailable
        }
        var output: UnsafeMutablePointer<UInt8>?; var length: CUnsignedLong = 0
        let status = response.withUnsafeBytes { response in
            request.withUnsafeBytes { request in
                anchor.networkID.withUnsafeBytes { network in
                    anchor.heightContextID.withUnsafeBytes { context in
                        function(response.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(response.count), kind,
                            request.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(request.count),
                            network.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(network.count), anchor.blockHeight,
                            context.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(context.count), &output, &length)
                    }
                }
            }
        }
        return try copyReserveFinalityOutput(status: status, pointer: output, length: length, maximum: maximum, free: free)
        #else
        throw KagemushaReserveFinalityError.bridgeUnavailable
        #endif
    }
}
