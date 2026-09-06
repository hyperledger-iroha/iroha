import Foundation

#if canImport(Darwin)
import Darwin
#endif

/// Errors raised by the authoritative native-Norito Hijiri quote boundary.
public enum ValidationFeeHijiriQuoteError: Error, Equatable, Sendable {
    /// The exact ABI-23 bridge or either additive quote symbol is unavailable.
    case bridgeUnavailable
    /// Native request encoding or response verification rejected the supplied bytes.
    case nativeRejected(Int32)
    /// Native code returned an empty, oversized, or malformed projection.
    case invalidNativeOutput
}

/// Typed input for one aggregate V1 Hijiri validation-fee quote.
public struct ValidationFeeHijiriQuoteRequestV1: Equatable, Sendable {
    /// Current request and response layout.
    public static let version: UInt16 = 1
    /// Largest aggregate transfer count admitted by V1.
    public static let maximumQualifyingTransferCount: UInt32 = 100_000
    /// Largest native-Norito request admitted by the route.
    public static let maximumRequestBytes = 4 * 1_024

    /// Frozen layout version.
    public let version: UInt16
    /// Canonical universal account whose effective Hijiri risk is priced.
    public let accountId: String
    /// Transfers aggregated before the one required Q16 ceiling operation.
    public let qualifyingTransferCount: UInt32

    /// Creates one request bound to an exact canonical I105 account literal.
    public init(accountId: String, qualifyingTransferCount: UInt32) throws {
        guard accountId == accountId.trimmingCharacters(in: .whitespacesAndNewlines),
              !accountId.isEmpty,
              !accountId.contains("@") else {
            throw ToriiClientError.invalidPayload(
                "accountId must be one exact canonical domainless I105 account id"
            )
        }
        do {
            let prefix = try AccountAddress.inspectI105NetworkPrefix(accountId).chainDiscriminant
            let address = try AccountAddress.parseEncoded(accountId, expectedPrefix: prefix)
            guard try address.toI105(networkPrefix: prefix) == accountId else {
                throw ToriiClientError.invalidPayload(
                    "accountId must be one exact canonical domainless I105 account id"
                )
            }
        } catch let error as ToriiClientError {
            throw error
        } catch {
            throw ToriiClientError.invalidPayload(
                "accountId must be one exact canonical domainless I105 account id"
            )
        }
        guard (1...Self.maximumQualifyingTransferCount).contains(qualifyingTransferCount) else {
            throw ToriiClientError.invalidPayload(
                "qualifyingTransferCount must be within 1...100000"
            )
        }
        self.version = Self.version
        self.accountId = accountId
        self.qualifyingTransferCount = qualifyingTransferCount
    }

    /// Encodes this request with the authoritative native Norito implementation.
    public func noritoBytes() throws -> Data {
        try ValidationFeeHijiriQuoteNative.encodeRequestV1(self)
    }
}

/// A native-verified V1 Hijiri validation-fee quote from one committed state snapshot.
///
/// The assurance marker explicitly states that the authenticated live projection is not an
/// independent state witness. Transaction admission later binds the policy and Hijiri hashes and
/// rejects a stale quote.
public struct ValidationFeeHijiriQuoteV1: Decodable, Equatable, Sendable {
    /// Largest native-Norito response accepted by V1 clients.
    public static let maximumResponseBytes = 64 * 1_024
    /// Stable verified projection schema.
    public static let schemaV1 = "iroha.torii.v1.validation_fee.hijiri_quote.response"
    /// Honest assurance marker for a live evaluated projection.
    public static let evaluatedAssuranceV1 =
        "EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED"

    public let schema: String
    public let version: UInt16
    public let assurance: String
    public let evaluatedStateHeight: String
    public let quotedExecutionHeight: String
    public let accountId: String
    public let activePolicyVersion: String
    public let activePolicyHash: String
    public let feeAssetDefinitionId: String
    public let treasuryAccountId: String
    public let feeScale: UInt8
    public let hijiriParametersVersion: UInt16
    public let hijiriParametersRevision: String
    public let hijiriParametersDigest: String
    public let defaultAccountRiskQ16: UInt32
    public let effectiveAccountRiskQ16: UInt32
    public let accountRiskRevision: String?
    public let accountRiskDigest: String?
    public let feeMultiplierQ16: UInt32
    public let hijiriFeeQuoteHash: String
    public let basePerTransferFeeMinorUnits: String
    public let adjustedPerTransferFeeMinorUnits: String
    public let qualifyingTransferCount: UInt32
    public let aggregateBaseFeeMinorUnits: String
    public let aggregateAdjustedFeeMinorUnits: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case schema
        case version
        case assurance
        case evaluatedStateHeight
        case quotedExecutionHeight
        case accountId
        case activePolicyVersion
        case activePolicyHash
        case feeAssetDefinitionId
        case treasuryAccountId
        case feeScale
        case hijiriParametersVersion
        case hijiriParametersRevision
        case hijiriParametersDigest
        case defaultAccountRiskQ16
        case effectiveAccountRiskQ16
        case accountRiskRevision
        case accountRiskDigest
        case feeMultiplierQ16
        case hijiriFeeQuoteHash
        case basePerTransferFeeMinorUnits
        case adjustedPerTransferFeeMinorUnits
        case qualifyingTransferCount
        case aggregateBaseFeeMinorUnits
        case aggregateAdjustedFeeMinorUnits
    }

    static func parseNativeVerifiedProjection(_ data: Data) throws -> Self {
        guard !data.isEmpty, data.count <= Self.maximumResponseBytes else {
            throw ValidationFeeHijiriQuoteError.invalidNativeOutput
        }
        let object: Any
        do {
            object = try JSONSerialization.jsonObject(with: data)
        } catch {
            throw ToriiClientError.decoding(error)
        }
        guard let dictionary = object as? [String: Any],
              Set(dictionary.keys) == Set(CodingKeys.allCases.map(\.rawValue)) else {
            throw ToriiClientError.invalidPayload(
                "native Hijiri quote projection differs from the frozen V1 field set"
            )
        }
        let quote: Self
        do {
            quote = try JSONDecoder().decode(Self.self, from: data)
        } catch {
            throw ToriiClientError.decoding(error)
        }
        guard quote.schema == Self.schemaV1,
              quote.version == ValidationFeeHijiriQuoteRequestV1.version,
              quote.assurance == Self.evaluatedAssuranceV1,
              (quote.accountRiskRevision == nil) == (quote.accountRiskDigest == nil) else {
            throw ToriiClientError.invalidPayload(
                "native Hijiri quote projection violates the frozen V1 shape"
            )
        }
        return quote
    }
}

/// Authoritative native-Norito codec and verifier for V1 Hijiri quotes.
public enum ValidationFeeHijiriQuoteNative {
    /// Native bridge ABI carrying the additive quote symbols.
    public static let requiredBridgeAbiVersion: UInt32 = 23

    /// Encodes one exact canonical bare-Norito request.
    public static func encodeRequestV1(
        _ request: ValidationFeeHijiriQuoteRequestV1
    ) throws -> Data {
        try NoritoNativeBridge.shared.validationFeeHijiriQuoteRequestV1(request)
    }

    /// Verifies a canonical response against the exact request bytes sent to Torii.
    public static func verifyResponseV1(
        _ responseNorito: Data,
        requestNorito: Data
    ) throws -> ValidationFeeHijiriQuoteV1 {
        let projection = try NoritoNativeBridge.shared
            .validationFeeHijiriQuoteResponseVerifyV1(
                responseNorito,
                requestNorito: requestNorito
            )
        return try ValidationFeeHijiriQuoteV1.parseNativeVerifiedProjection(projection)
    }
}

protocol ValidationFeeHijiriQuoteCoding {
    func encode(_ request: ValidationFeeHijiriQuoteRequestV1) throws -> Data
    func verify(
        _ responseNorito: Data,
        requestNorito: Data
    ) throws -> ValidationFeeHijiriQuoteV1
}

struct NativeValidationFeeHijiriQuoteCodec: ValidationFeeHijiriQuoteCoding {
    func encode(_ request: ValidationFeeHijiriQuoteRequestV1) throws -> Data {
        try ValidationFeeHijiriQuoteNative.encodeRequestV1(request)
    }

    func verify(
        _ responseNorito: Data,
        requestNorito: Data
    ) throws -> ValidationFeeHijiriQuoteV1 {
        try ValidationFeeHijiriQuoteNative.verifyResponseV1(
            responseNorito,
            requestNorito: requestNorito
        )
    }
}

extension NoritoNativeBridge {
    #if canImport(Darwin)
    private typealias ValidationFeeHijiriQuoteRequestFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong, UInt32,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias ValidationFeeHijiriQuoteResponseVerifyFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
        UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias ValidationFeeHijiriQuoteFreeFn = @convention(c) (
        UnsafeMutablePointer<UInt8>?
    ) -> Void
    #endif

    func validationFeeHijiriQuoteRequestV1(
        _ request: ValidationFeeHijiriQuoteRequestV1
    ) throws -> Data {
        #if canImport(Darwin)
        guard let function = resolveNativeSymbol(
            "connect_norito_validation_fee_hijiri_quote_request_v1",
            as: ValidationFeeHijiriQuoteRequestFn.self
        ), let free = resolveNativeSymbol(
            "connect_norito_free",
            as: ValidationFeeHijiriQuoteFreeFn.self
        ) else {
            throw ValidationFeeHijiriQuoteError.bridgeUnavailable
        }
        let account = Data(request.accountId.utf8)
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = account.withUnsafeBytes { bytes in
            function(
                bytes.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(bytes.count),
                request.qualifyingTransferCount,
                &output,
                &outputLength
            )
        }
        return try Self.copyValidationFeeHijiriQuoteOutput(
            status: status,
            pointer: output,
            length: outputLength,
            maximumBytes: ValidationFeeHijiriQuoteRequestV1.maximumRequestBytes,
            free: free
        )
        #else
        _ = request
        throw ValidationFeeHijiriQuoteError.bridgeUnavailable
        #endif
    }

    func validationFeeHijiriQuoteResponseVerifyV1(
        _ responseNorito: Data,
        requestNorito: Data
    ) throws -> Data {
        guard !responseNorito.isEmpty,
              responseNorito.count <= ValidationFeeHijiriQuoteV1.maximumResponseBytes,
              !requestNorito.isEmpty,
              requestNorito.count <= ValidationFeeHijiriQuoteRequestV1.maximumRequestBytes else {
            throw ToriiClientError.invalidPayload(
                "Hijiri quote request or response exceeds its frozen V1 byte bound"
            )
        }
        #if canImport(Darwin)
        guard let function = resolveNativeSymbol(
            "connect_norito_validation_fee_hijiri_quote_response_verify_v1",
            as: ValidationFeeHijiriQuoteResponseVerifyFn.self
        ), let free = resolveNativeSymbol(
            "connect_norito_free",
            as: ValidationFeeHijiriQuoteFreeFn.self
        ) else {
            throw ValidationFeeHijiriQuoteError.bridgeUnavailable
        }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = responseNorito.withUnsafeBytes { response in
            requestNorito.withUnsafeBytes { request in
                function(
                    response.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(response.count),
                    request.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(request.count),
                    &output,
                    &outputLength
                )
            }
        }
        return try Self.copyValidationFeeHijiriQuoteOutput(
            status: status,
            pointer: output,
            length: outputLength,
            maximumBytes: ValidationFeeHijiriQuoteV1.maximumResponseBytes,
            free: free
        )
        #else
        throw ValidationFeeHijiriQuoteError.bridgeUnavailable
        #endif
    }

    #if canImport(Darwin)
    private static func copyValidationFeeHijiriQuoteOutput(
        status: Int32,
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong,
        maximumBytes: Int,
        free: ValidationFeeHijiriQuoteFreeFn
    ) throws -> Data {
        guard status == 0 else {
            if let pointer { free(pointer) }
            throw ValidationFeeHijiriQuoteError.nativeRejected(status)
        }
        guard let pointer,
              length > 0,
              length <= CUnsignedLong(maximumBytes),
              UInt64(length) <= UInt64(Int.max) else {
            if let pointer { free(pointer) }
            throw ValidationFeeHijiriQuoteError.invalidNativeOutput
        }
        defer { free(pointer) }
        return Data(bytes: pointer, count: Int(length))
    }
    #endif
}
