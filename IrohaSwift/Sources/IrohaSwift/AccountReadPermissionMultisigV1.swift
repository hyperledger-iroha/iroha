import Foundation

/// The only wallet-controlled account-read permission changes in the current bridge.
public enum AccountReadPermissionChangeV1: UInt8, Sendable {
    case grant = 1
    case revoke = 2
}

public struct AccountReadPermissionSignedTransactionV1: Sendable {
    public let signedBytes: Data
    public let transactionHash: Data

    public init(signedBytes: Data, transactionHash: Data) {
        self.signedBytes = signedBytes
        self.transactionHash = transactionHash
    }
}

public enum AccountReadPermissionSigningErrorV1: Error, Equatable, Sendable {
    case bridgeUnavailable
    case invalidAuthority
    case invalidReporter
    case invalidCreationTime
    case invalidFeePayment
    case invalidSignature
    case invalidNativeResult
    case nativeRejected(Int32)
}

/// Typed binding for a wallet-owned, 1-of-1 multisig CanReadAccountData grant
/// or revoke. The reporter must be authenticated by the caller's release pin.
/// This API never accepts a server-supplied prehash or transaction scaffold.
public struct AccountReadPermissionMultisigV1: Sendable {
    public let networkId: NetworkId
    public let authority: String
    public let reportingAccount: String
    public let change: AccountReadPermissionChangeV1
    public let creationTimeMs: UInt64
    public let feePayment: FeePaymentIntent
    private let feePaymentJSON: Data

    public init(
        networkId: NetworkId,
        authority: String,
        reportingAccount: String,
        change: AccountReadPermissionChangeV1,
        creationTimeMs: UInt64,
        feePayment: FeePaymentIntent,
        feePaymentJSON: Data
    ) throws {
        guard creationTimeMs > 0 else { throw AccountReadPermissionSigningErrorV1.invalidCreationTime }
        guard (1...2).contains(feePayment.chargeLimits.count),
              !feePaymentJSON.isEmpty, feePaymentJSON.count <= 16 * 1024,
              let parsed = try? JSONDecoder().decode(FeePaymentIntent.self, from: feePaymentJSON),
              parsed == feePayment,
              let object = try? JSONSerialization.jsonObject(with: feePaymentJSON),
              let canonical = try? JSONSerialization.data(
                withJSONObject: object, options: [.sortedKeys, .withoutEscapingSlashes]
              ), canonical == feePaymentJSON else {
            throw AccountReadPermissionSigningErrorV1.invalidFeePayment
        }
        do {
            let parsed = try AccountAddress.fromI105(authority)
            guard try parsed.toI105(networkPrefix: AccountAddress.inspectI105NetworkPrefix(authority).chainDiscriminant) == authority else {
                throw AccountReadPermissionSigningErrorV1.invalidAuthority
            }
        } catch {
            throw AccountReadPermissionSigningErrorV1.invalidAuthority
        }
        do {
            let parsed = try AccountAddress.fromI105(reportingAccount)
            guard try parsed.toI105(networkPrefix: AccountAddress.inspectI105NetworkPrefix(reportingAccount).chainDiscriminant) == reportingAccount,
                  reportingAccount != authority else {
                throw AccountReadPermissionSigningErrorV1.invalidReporter
            }
        } catch {
            throw AccountReadPermissionSigningErrorV1.invalidReporter
        }
        self.networkId = networkId
        self.authority = authority
        self.reportingAccount = reportingAccount
        self.change = change
        self.creationTimeMs = creationTimeMs
        self.feePayment = feePayment
        self.feePaymentJSON = feePaymentJSON
    }

    #if canImport(Darwin)
    private typealias PayloadHashFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt8, UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?, UInt
    ) -> Int32
    private typealias FinalizeFn = @convention(c) (
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UnsafePointer<CChar>?, UInt,
        UInt8, UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?, UInt
    ) -> Int32
    private typealias FreeFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private func invoke<R>(_ body: (UnsafePointer<CChar>?, UnsafePointer<CChar>?, UnsafePointer<CChar>?) -> R) -> R {
        networkId.literal.withCString { network in
            authority.withCString { source in
                reportingAccount.withCString { reporter in
                    body(network, source, reporter)
                }
            }
        }
    }
    #endif

    /// Exact Iroha transaction payload prehash, to sign with the wallet key.
    public func payloadHash() throws -> Data {
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable,
              let function: PayloadHashFn = bridge.resolveNativeSymbol(
                "connect_norito_account_read_permission_multisig_payload_hash", as: PayloadHashFn.self
              ) else { throw AccountReadPermissionSigningErrorV1.bridgeUnavailable }
        var hash = [UInt8](repeating: 0, count: 32)
        let status = feePaymentJSON.withUnsafeBytes { feeBuffer in
            hash.withUnsafeMutableBufferPointer { buffer in
                invoke { network, source, reporter in
                    function(network, UInt(networkId.literal.utf8.count),
                             source, UInt(authority.utf8.count),
                             reporter, UInt(reportingAccount.utf8.count),
                             change.rawValue, creationTimeMs,
                             feeBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(feePaymentJSON.count),
                             buffer.baseAddress, UInt(buffer.count))
                }
            }
        }
        guard status == 0 else { throw AccountReadPermissionSigningErrorV1.nativeRejected(status) }
        guard hash.contains(where: { $0 != 0 }) else { throw AccountReadPermissionSigningErrorV1.invalidNativeResult }
        return Data(hash)
        #else
        throw AccountReadPermissionSigningErrorV1.bridgeUnavailable
        #endif
    }

    /// Finalize with a raw Ed25519 signature over `payloadHash()`. Native code
    /// checks the key encoded in the source multisig policy and verifies the
    /// complete canonical SignedTransaction before returning any bytes.
    public func finalize(signature: Data) throws -> AccountReadPermissionSignedTransactionV1 {
        guard signature.count == 64 else { throw AccountReadPermissionSigningErrorV1.invalidSignature }
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable,
              let function: FinalizeFn = bridge.resolveNativeSymbol(
                "connect_norito_account_read_permission_multisig_finalize", as: FinalizeFn.self
              ),
              let free: FreeFn = bridge.resolveNativeSymbol("connect_norito_free", as: FreeFn.self)
        else { throw AccountReadPermissionSigningErrorV1.bridgeUnavailable }
        var signedPointer: UnsafeMutablePointer<UInt8>?
        var signedLength: UInt = 0
        var hash = [UInt8](repeating: 0, count: 32)
        let status = feePaymentJSON.withUnsafeBytes { feeBuffer in
            signature.withUnsafeBytes { signatureBuffer in
                hash.withUnsafeMutableBufferPointer { hashBuffer in
                    invoke { network, source, reporter in
                        function(network, UInt(networkId.literal.utf8.count),
                                 source, UInt(authority.utf8.count),
                                 reporter, UInt(reportingAccount.utf8.count),
                                 change.rawValue, creationTimeMs,
                                 feeBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(feePaymentJSON.count),
                                 signatureBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(signature.count),
                                 &signedPointer, &signedLength,
                                 hashBuffer.baseAddress, UInt(hashBuffer.count))
                    }
                }
            }
        }
        defer { if let signedPointer { free(UnsafeMutableRawPointer(signedPointer)) } }
        guard status == 0 else { throw AccountReadPermissionSigningErrorV1.nativeRejected(status) }
        guard let signedPointer, signedLength > 0, signedLength <= 64 * 1024,
              hash.contains(where: { $0 != 0 }) else {
            throw AccountReadPermissionSigningErrorV1.invalidNativeResult
        }
        return AccountReadPermissionSignedTransactionV1(
            signedBytes: Data(bytes: signedPointer, count: Int(signedLength)),
            transactionHash: Data(hash)
        )
        #else
        throw AccountReadPermissionSigningErrorV1.bridgeUnavailable
        #endif
    }
}
