import Foundation

public enum CommittedTransactionInclusionErrorV1: Error, Equatable, Sendable {
    case bridgeUnavailable
    case invalidInput
    case invalidNativeResult
    case nativeRejected(Int32)
}

/// Exact wallet-self signed selective query. The caller owns nonce persistence
/// and must sign `payloadHash()` using the wallet controller's Ed25519 key.
public struct CommittedTransactionQueryV1: Sendable {
    public let networkId: NetworkId
    public let walletAccountId: String
    public let transactionHash: Data
    public let creationTimeMs: UInt64
    public let nonce: Data

    public init(
        networkId: NetworkId, walletAccountId: String, transactionHash: Data,
        creationTimeMs: UInt64, nonce: Data
    ) throws {
        guard transactionHash.count == 32, transactionHash[31] & 1 == 1,
              creationTimeMs > 0, nonce.count == 32, nonce.contains(where: { $0 != 0 }),
              !walletAccountId.isEmpty, walletAccountId == walletAccountId.trimmingCharacters(in: .whitespacesAndNewlines),
              walletAccountId.utf8.count <= 1024 else {
            throw CommittedTransactionInclusionErrorV1.invalidInput
        }
        do {
            let account = try AccountAddress.fromI105(walletAccountId)
            guard try account.toI105(networkPrefix: AccountAddress.inspectI105NetworkPrefix(walletAccountId).chainDiscriminant) == walletAccountId else {
                throw CommittedTransactionInclusionErrorV1.invalidInput
            }
        } catch {
            throw CommittedTransactionInclusionErrorV1.invalidInput
        }
        self.networkId = networkId
        self.walletAccountId = walletAccountId
        self.transactionHash = Data(transactionHash)
        self.creationTimeMs = creationTimeMs
        self.nonce = Data(nonce)
    }

    #if canImport(Darwin)
    private typealias HashFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias FinalizeFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UInt64,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?
    ) -> Int32
    private typealias FreeFn = @convention(c) (UnsafeMutableRawPointer?) -> Void

    private func withInputs<R>(
        _ body: (UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?, UnsafePointer<UInt8>?) -> R
    ) -> R {
        networkId.bytes.withUnsafeBytes { network in
            walletAccountId.data(using: .utf8)!.withUnsafeBytes { account in
                transactionHash.withUnsafeBytes { transaction in
                    nonce.withUnsafeBytes { nonce in
                        body(network.bindMemory(to: UInt8.self).baseAddress,
                             account.bindMemory(to: UInt8.self).baseAddress,
                             transaction.bindMemory(to: UInt8.self).baseAddress,
                             nonce.bindMemory(to: UInt8.self).baseAddress)
                    }
                }
            }
        }
    }
    #endif

    public func payloadHash() throws -> Data {
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable,
              let function: HashFn = bridge.resolveNativeSymbol(
                "connect_norito_committed_transaction_query_payload_hash_v1", as: HashFn.self
              ) else { throw CommittedTransactionInclusionErrorV1.bridgeUnavailable }
        var hash = [UInt8](repeating: 0, count: 32)
        let status = hash.withUnsafeMutableBufferPointer { output in
            withInputs { network, account, transaction, nonce in
                function(network, UInt(networkId.bytes.count),
                         account, UInt(walletAccountId.utf8.count),
                         transaction, UInt(transactionHash.count),
                         creationTimeMs, nonce, UInt(self.nonce.count), output.baseAddress)
            }
        }
        guard status == 0 else { throw CommittedTransactionInclusionErrorV1.nativeRejected(status) }
        guard hash.contains(where: { $0 != 0 }) else { throw CommittedTransactionInclusionErrorV1.invalidNativeResult }
        return Data(hash)
        #else
        throw CommittedTransactionInclusionErrorV1.bridgeUnavailable
        #endif
    }

    /// Versioned Norito `SignedQuery` for exactly one POST `/query` attempt.
    public func finalize(signature: Data) throws -> Data {
        guard signature.count == 64 else { throw CommittedTransactionInclusionErrorV1.invalidInput }
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable,
              let function: FinalizeFn = bridge.resolveNativeSymbol(
                "connect_norito_committed_transaction_query_finalize_v1", as: FinalizeFn.self
              ),
              let free: FreeFn = bridge.resolveNativeSymbol("connect_norito_free", as: FreeFn.self)
        else { throw CommittedTransactionInclusionErrorV1.bridgeUnavailable }
        var outputPointer: UnsafeMutablePointer<UInt8>?
        var outputLength: UInt = 0
        let status = signature.withUnsafeBytes { sig in
            withInputs { network, account, transaction, nonce in
                function(network, UInt(networkId.bytes.count),
                         account, UInt(walletAccountId.utf8.count),
                         transaction, UInt(transactionHash.count),
                         creationTimeMs, nonce, UInt(self.nonce.count),
                         sig.bindMemory(to: UInt8.self).baseAddress, UInt(signature.count),
                         &outputPointer, &outputLength)
            }
        }
        defer { if let outputPointer { free(UnsafeMutableRawPointer(outputPointer)) } }
        guard status == 0 else { throw CommittedTransactionInclusionErrorV1.nativeRejected(status) }
        guard let outputPointer, outputLength > 0, outputLength <= 16 * 1024 else {
            throw CommittedTransactionInclusionErrorV1.invalidNativeResult
        }
        return Data(bytes: outputPointer, count: Int(outputLength))
        #else
        throw CommittedTransactionInclusionErrorV1.bridgeUnavailable
        #endif
    }
}

public struct VerifiedCommittedTransactionV1: Sendable {
    public let canonicalRow: Data
    public let outputHash: Data
    public let blockHash: Data
    public let blockHeight: UInt64
    public let resultOk: Bool
}

/// Binds a selected full output to a caller-pinned NetworkId, trusted initial
/// height-context anchor and exact transaction hash through current finality.
public enum CommittedTransactionInclusionV1 {
    #if canImport(Darwin)
    private typealias VerifyFn = @convention(c) (
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafePointer<UInt8>?, UInt,
        UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<UInt>?,
        UnsafeMutablePointer<UInt8>?, UnsafeMutablePointer<UInt8>?,
        UnsafeMutablePointer<UInt64>?, UnsafeMutablePointer<UInt8>?
    ) -> Int32
    private typealias FreeFn = @convention(c) (UnsafeMutableRawPointer?) -> Void
    #endif

    public static func verify(
        response: Data, finalityBundleChainJSON: Data,
        networkId: NetworkId, trustedHeightContextId: String,
        transactionHash: Data
    ) throws -> VerifiedCommittedTransactionV1 {
        guard !response.isEmpty, response.count <= 32 * 1024 * 1024,
              !finalityBundleChainJSON.isEmpty, finalityBundleChainJSON.count <= 16 * 1024 * 1024,
              !trustedHeightContextId.isEmpty, trustedHeightContextId.utf8.count <= 128,
              trustedHeightContextId == trustedHeightContextId.trimmingCharacters(in: .whitespacesAndNewlines),
              transactionHash.count == 32, transactionHash[31] & 1 == 1 else {
            throw CommittedTransactionInclusionErrorV1.invalidInput
        }
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable,
              let function: VerifyFn = bridge.resolveNativeSymbol(
                "connect_norito_verify_committed_transaction_inclusion_v1", as: VerifyFn.self
              ),
              let free: FreeFn = bridge.resolveNativeSymbol("connect_norito_free", as: FreeFn.self)
        else { throw CommittedTransactionInclusionErrorV1.bridgeUnavailable }
        var rowPointer: UnsafeMutablePointer<UInt8>?
        var rowLength: UInt = 0
        var outputHash = [UInt8](repeating: 0, count: 32)
        var blockHash = [UInt8](repeating: 0, count: 32)
        var blockHeight: UInt64 = 0
        var resultOk: UInt8 = 0
        let anchor = Data(trustedHeightContextId.utf8)
        let status = response.withUnsafeBytes { responseBuffer in
            finalityBundleChainJSON.withUnsafeBytes { chainBuffer in
                networkId.bytes.withUnsafeBytes { networkBuffer in
                    anchor.withUnsafeBytes { anchorBuffer in
                        transactionHash.withUnsafeBytes { transactionBuffer in
                            outputHash.withUnsafeMutableBufferPointer { outputBuffer in
                                blockHash.withUnsafeMutableBufferPointer { blockBuffer in
                                    function(
                                        responseBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(response.count),
                                        chainBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(finalityBundleChainJSON.count),
                                        networkBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(networkId.bytes.count),
                                        anchorBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(anchor.count),
                                        transactionBuffer.bindMemory(to: UInt8.self).baseAddress, UInt(transactionHash.count),
                                        &rowPointer, &rowLength, outputBuffer.baseAddress, blockBuffer.baseAddress,
                                        &blockHeight, &resultOk
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        defer { if let rowPointer { free(UnsafeMutableRawPointer(rowPointer)) } }
        guard status == 0 else { throw CommittedTransactionInclusionErrorV1.nativeRejected(status) }
        guard let rowPointer, rowLength > 0, rowLength <= 4 * 1024 * 1024,
              blockHeight > 0, resultOk == 0 || resultOk == 1 else {
            throw CommittedTransactionInclusionErrorV1.invalidNativeResult
        }
        return VerifiedCommittedTransactionV1(
            canonicalRow: Data(bytes: rowPointer, count: Int(rowLength)),
            outputHash: Data(outputHash), blockHash: Data(blockHash),
            blockHeight: blockHeight, resultOk: resultOk == 1
        )
        #else
        throw CommittedTransactionInclusionErrorV1.bridgeUnavailable
        #endif
    }
}
