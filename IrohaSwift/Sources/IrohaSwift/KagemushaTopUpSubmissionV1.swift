import Foundation

/// Immutable signed bytes authenticated against the complete original reviewed top-up request.
/// Reconstruct this value through the native validator after loading a durable pending operation.
/// App-owned session, MiBank approval, fee review and persistence still precede every dispatch.
public struct KagemushaPreparedTopUpSubmissionV1: Sendable {
    private let bytes: TopUpSubmissionBytesV1
    public let expectedRequest: KagemushaTopUpRequestV1

    public init(signedTransaction: Data, expectedRequest: KagemushaTopUpRequestV1) throws {
        let request = try KagemushaNoritoV1.encodeTopUpRequestShape(expectedRequest)
        bytes = try TopUpSubmissionBytesV1(signed: signedTransaction, request: request) { signed, request in
            try NoritoNativeBridge.shared.validateTopUpSubmission(signed: signed, request: request)
        }
        self.expectedRequest = expectedRequest
    }
    /// Exact canonical signed V1 transaction; callers never rebuild it from status metadata.
    public var signedTransactionBytes: Data { bytes.signedTransaction }
    /// Exact reviewed canonical request, retained independently from the signed transaction.
    public var canonicalRequestBytes: Data { bytes.canonicalRequest }
}

public enum KagemushaTopUpSubmissionErrorV1: Error, Equatable {
    case invalidInput
    case bridgeUnavailable
    case requestMismatch
}

// The production constructor above supplies only the fixed native validator. This byte owner
// permits isolated copy/lifetime tests without treating their injected callback as native proof.
struct TopUpSubmissionBytesV1: Sendable {
    private let signed: [UInt8]
    private let request: [UInt8]
    init(signed: Data, request: Data, validate: (Data, Data) throws -> Void) throws {
        guard !signed.isEmpty, signed.count <= 16 * 1024 * 1024,
              !request.isEmpty, request.count <= 16 * 1024 else {
            throw KagemushaTopUpSubmissionErrorV1.invalidInput
        }
        let signedCopy = Array(signed), requestCopy = Array(request)
        try validate(Data(signedCopy), Data(requestCopy))
        self.signed = signedCopy
        self.request = requestCopy
    }
    var signedTransaction: Data { Data(signed) }
    var canonicalRequest: Data { Data(request) }
}

extension NoritoNativeBridge {
    func validateTopUpSubmission(signed: Data, request: Data) throws {
        #if canImport(Darwin)
        typealias Validate = @convention(c) (UnsafePointer<UInt8>?, CUnsignedLong,
            UnsafePointer<UInt8>?, CUnsignedLong) -> Int32
        typealias Abi = @convention(c) () -> UInt32
        guard isAvailable,
              let abi = resolveNativeSymbol("connect_norito_bridge_abi_version", as: Abi.self),
              abi() == NoritoBridgeLoader.expectedBridgeAbiVersion,
              let validate = resolveNativeSymbol("connect_norito_kagemusha_top_up_signed_request_validate_v1", as: Validate.self)
        else { throw KagemushaTopUpSubmissionErrorV1.bridgeUnavailable }
        let status = signed.withUnsafeBytes { tx in
            request.withUnsafeBytes { intent in
                validate(tx.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(signed.count),
                    intent.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(request.count))
            }
        }
        guard status == 0 else { throw KagemushaTopUpSubmissionErrorV1.requestMismatch }
        #else
        throw KagemushaTopUpSubmissionErrorV1.bridgeUnavailable
        #endif
    }
}
