import CryptoKit
import Foundation

/// Failure to obtain canonical X.509 ownership-signing material from the native holder.
public enum ZkX509SigningPreparationErrorV1: Error, Equatable, Sendable {
    /// The input contains no Norito statement archive.
    case emptyStatement
    /// The input exceeds the bounded native statement contract.
    case statementTooLarge(maximum: Int, actual: Int)
    /// The authenticated current native bridge or required exports are unavailable.
    case nativeUnavailable
    /// Native canonical decoding, protocol selection or message derivation refused the input.
    case nativeRejected(Int32)
    /// Native output violates its bounded message/digest contract.
    case invalidNativeResult
}

/// One atomic result from the canonical native X.509 holder signing preparation.
///
/// An external P-256/SHA-256 message signer signs `message`, hashing it exactly once.
/// A prehash signer signs `digest` directly. Do not pass `digest` to a message signer:
/// that would hash the message twice. The subject private key stays with the signer.
/// Normalize its ECDSA signature to low-s, fixed 64-byte `r || s` before witness use.
/// This result does not authenticate current State, prove a credential, authorize
/// submission, pair a device, or establish network activation or readiness.
public struct ZkX509SigningPreparationV1: Equatable, Sendable {
    /// Maximum canonical Norito `PrivacyStatementV1` archive accepted by the native ABI.
    public static let maximumStatementBytes = 256 * 1024
    /// Maximum canonical ownership-signing message returned by the native ABI.
    public static let maximumMessageBytes = 256 * 1024

    /// Exact canonical message for a signer that applies SHA-256 itself.
    public let message: Data
    /// SHA-256 of `message`, for a signer that accepts a precomputed digest.
    public let digest: Data

    private init(message: Data, digest: Data) {
        self.message = message
        self.digest = digest
    }

    #if canImport(Darwin)
    private typealias PrepareFn = @convention(c) (
        UnsafePointer<UInt8>?, CUnsignedLong,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
    ) -> Int32
    private typealias FreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
    #endif

    /// Whether the current authenticated bridge exposes this preparation and its allocator owner.
    public static var isNativeAvailable: Bool {
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        return bridge.isAvailable
            && bridge.resolveNativeSymbol(
                "connect_norito_zk_x509_prepare_ownership_signing_v1", as: PrepareFn.self
            ) != nil
            && bridge.resolveNativeSymbol("iroha_privacy_free_buffer", as: FreeFn.self) != nil
        #else
        return false
        #endif
    }

    /// Prepare the canonical message and digest for one exact X.509 statement.
    ///
    /// Supply the canonical Norito `PrivacyStatementV1` archive, including its header
    /// and X.509 variant. Native code owns decoding and message framing; other protocol
    /// variants, malformed archives and trailing bytes are rejected. No subject key,
    /// signature, private witness or transaction-signing algorithm is accepted.
    public static func prepareOwnershipSigningV1(statementNorito: Data) throws -> Self {
        guard !statementNorito.isEmpty else {
            throw ZkX509SigningPreparationErrorV1.emptyStatement
        }
        guard statementNorito.count <= maximumStatementBytes else {
            throw ZkX509SigningPreparationErrorV1.statementTooLarge(
                maximum: maximumStatementBytes, actual: statementNorito.count
            )
        }
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable,
              let prepare = bridge.resolveNativeSymbol(
                "connect_norito_zk_x509_prepare_ownership_signing_v1", as: PrepareFn.self
              ),
              let free = bridge.resolveNativeSymbol("iroha_privacy_free_buffer", as: FreeFn.self)
        else { throw ZkX509SigningPreparationErrorV1.nativeUnavailable }

        var messagePointer: UnsafeMutablePointer<UInt8>?
        var digestPointer: UnsafeMutablePointer<UInt8>?
        var messageLength: CUnsignedLong = 0
        var digestLength: CUnsignedLong = 0
        let status = statementNorito.withUnsafeBytes { bytes in
            prepare(
                bytes.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(bytes.count),
                &messagePointer, &messageLength, &digestPointer, &digestLength
            )
        }
        // Both buffers belong to the privacy allocator, including any output
        // returned with an error. No borrowed input pointer escapes the C call.
        defer {
            if let messagePointer { free(messagePointer) }
            if let digestPointer { free(digestPointer) }
        }
        guard status == 0 else {
            throw ZkX509SigningPreparationErrorV1.nativeRejected(status)
        }
        guard let messagePointer, messageLength > 0,
              messageLength <= CUnsignedLong(maximumMessageBytes),
              let digestPointer, digestLength == 32 else {
            throw ZkX509SigningPreparationErrorV1.invalidNativeResult
        }
        return try validateNativeResult(
            message: Data(bytes: messagePointer, count: Int(messageLength)),
            digest: Data(bytes: digestPointer, count: Int(digestLength))
        )
        #else
        throw ZkX509SigningPreparationErrorV1.nativeUnavailable
        #endif
    }

    // Verify output integrity without reproducing native statement decoding or framing.
    static func validateNativeResult(message: Data, digest: Data) throws -> Self {
        guard !message.isEmpty, message.count <= maximumMessageBytes,
              digest.count == 32, digest == Data(SHA256.hash(data: message)) else {
            throw ZkX509SigningPreparationErrorV1.invalidNativeResult
        }
        return Self(message: message, digest: digest)
    }
}
