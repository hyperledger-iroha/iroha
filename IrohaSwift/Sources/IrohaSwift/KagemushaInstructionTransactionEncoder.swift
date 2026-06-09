import Foundation

public enum KagemushaInstructionTransactionError: Error, Equatable, LocalizedError {
    case emptyInstructionArchive
    case oversizedInstructionArchive
    case invalidInstructionArchive
    case unsupportedInstructionArchiveType
    case unexpectedInstructionArchiveType(expected: KagemushaInstructionType, actual: KagemushaInstructionType)

    public var errorDescription: String? {
        switch self {
        case .emptyInstructionArchive:
            return "Kagemusha instruction archive must not be empty."
        case .oversizedInstructionArchive:
            return "Kagemusha instruction archive must not exceed \(KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes) bytes."
        case .invalidInstructionArchive:
            return "Kagemusha instruction archive must be a valid Norito instruction archive."
        case .unsupportedInstructionArchiveType:
            return "Kagemusha instruction archive type is not supported by this transaction builder."
        case let .unexpectedInstructionArchiveType(expected, actual):
            return "Kagemusha instruction archive type \(actual.rawValue) does not match expected \(expected.rawValue)."
        }
    }
}

public enum KagemushaInstructionType: String, Equatable, Sendable {
    case transfer = "KagemushaTransfer"
    case redeemRecursive = "RedeemKagemushaRecursive"

    public static func validatedArchiveType(for archive: Data) throws -> KagemushaInstructionType {
        try KagemushaInstructionTransactionEncoder.validateInstructionArchive(archive)
    }

    public var wireName: String {
        switch self {
        case .transfer:
            return KagemushaWireNames.transferInstruction
        case .redeemRecursive:
            return KagemushaWireNames.redeemRecursiveInstruction
        }
    }
}

public enum KagemushaWireNames {
    public static let transferInstruction = "iroha_data_model::isi::offline::KagemushaTransfer"
    public static let redeemRecursiveInstruction = "iroha_data_model::isi::offline::RedeemKagemushaRecursive"
    public static let recursiveRedeemRequest = "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
}

public struct KagemushaInstructionTransactionRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]
    public let instructionArchive: Data

    public init(
        chainId: String,
        authority: String,
        ttlMs: UInt64? = nil,
        nonce: UInt32? = nil,
        metadata: [String: ToriiJSONValue] = [:],
        instructionArchive: Data
    ) {
        self.chainId = chainId
        self.authority = authority
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
        self.instructionArchive = instructionArchive
    }

    public static func validateInputs(chainId: String, authority: String) throws {
        _ = try TransactionInputValidator.validate(chainId: chainId, authorityId: authority)
    }

    public func validateInputs() throws {
        try Self.validateInputs(chainId: chainId, authority: authority)
    }
}

public struct KagemushaRecursiveRedeemTransactionRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]
    public let redeemRequestArchive: Data

    public init(
        chainId: String,
        authority: String,
        ttlMs: UInt64? = nil,
        nonce: UInt32? = nil,
        metadata: [String: ToriiJSONValue] = [:],
        redeemRequestArchive: Data
    ) {
        self.chainId = chainId
        self.authority = authority
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
        self.redeemRequestArchive = redeemRequestArchive
    }

    public static func validateInputs(chainId: String, authority: String) throws {
        _ = try TransactionInputValidator.validate(chainId: chainId, authorityId: authority)
    }

    public func validateInputs() throws {
        try Self.validateInputs(chainId: chainId, authority: authority)
    }
}

public enum KagemushaRecursiveRedeemRequestArchiveError: Error, Equatable, LocalizedError {
    case emptyRequestArchive
    case oversizedRequestArchive
    case invalidRequestArchive
    case unsupportedRequestArchiveType

    public var errorDescription: String? {
        switch self {
        case .emptyRequestArchive:
            return "Kagemusha recursive redeem request archive must not be empty."
        case .oversizedRequestArchive:
            return "Kagemusha recursive redeem request archive must not exceed \(KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes) bytes."
        case .invalidRequestArchive:
            return "Kagemusha recursive redeem request archive must be a valid Norito archive."
        case .unsupportedRequestArchiveType:
            return "Kagemusha recursive redeem request archive type is not supported by ABI-7 admission."
        }
    }
}

enum KagemushaInstructionTransactionEncoder {
    private static let signedTransactionWireVersion: UInt8 = 1
    fileprivate static let maxNoritoHeaderPaddingBytes = 64

    static func encode(
        request: KagemushaInstructionTransactionRequest,
        signingKey: SigningKey,
        creationTimeMs: UInt64
    ) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        let instructionType = try validateInstructionArchive(request.instructionArchive)
        let instructionPayload = encodeInstructionBox(
            wireName: instructionType.wireName,
            instructionArchive: request.instructionArchive
        )
        return try encodeTransaction(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            instructionPayload: instructionPayload,
            metadata: request.metadata,
            signingKey: signingKey
        )
    }

    static func encodeRecursiveRedeem(
        request: KagemushaRecursiveRedeemTransactionRequest,
        signingKey: SigningKey,
        creationTimeMs: UInt64,
        redeem: (Data) throws -> Data = { try KagemushaRecursiveSpendProver.redeemSpend(requestArchive: $0) }
    ) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        try KagemushaRecursiveRedeemRequestArchive.validate(request.redeemRequestArchive)
        let instructionArchive = try redeem(request.redeemRequestArchive)
        let instructionType = try validateInstructionArchive(instructionArchive)
        guard instructionType == .redeemRecursive else {
            throw KagemushaInstructionTransactionError.unexpectedInstructionArchiveType(
                expected: .redeemRecursive,
                actual: instructionType
            )
        }
        let instructionPayload = encodeInstructionBox(
            wireName: instructionType.wireName,
            instructionArchive: instructionArchive
        )
        return try encodeTransaction(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            instructionPayload: instructionPayload,
            metadata: request.metadata,
            signingKey: signingKey
        )
    }

    static func validateInstructionArchive(_ archive: Data) throws -> KagemushaInstructionType {
        guard !archive.isEmpty else {
            throw KagemushaInstructionTransactionError.emptyInstructionArchive
        }
        guard archive.count <= KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes else {
            throw KagemushaInstructionTransactionError.oversizedInstructionArchive
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.length > 0,
              frame.paddingLength <= maxNoritoHeaderPaddingBytes else {
            throw KagemushaInstructionTransactionError.invalidInstructionArchive
        }
        for type in [KagemushaInstructionType.transfer, .redeemRecursive] {
            if frame.header.schema == noritoSchemaHash(forTypeName: type.wireName) {
                return type
            }
        }
        throw KagemushaInstructionTransactionError.unsupportedInstructionArchiveType
    }

    private static func encodeInstructionBox(wireName: String, instructionArchive: Data) -> Data {
        var instructionBox = OfflineNoritoWriter()
        instructionBox.writeField(OfflineNorito.encodeString(wireName))
        instructionBox.writeField(OfflineNorito.encodeBytesVec(instructionArchive))
        return instructionBox.data
    }

    private static func encodeTransaction(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32?,
        instructionPayload: Data,
        metadata: [String: ToriiJSONValue],
        signingKey: SigningKey
    ) throws -> SignedTransactionEnvelope {
        let transactionPayload = try encodeTransactionPayload(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instructionPayload,
            metadata: metadata
        )
        let signature = try signingKey.sign(IrohaHash.hash(transactionPayload))
        let signedTransaction = encodeSignedTransaction(
            signature: signature,
            transactionPayload: transactionPayload
        )
        let transactionHash = IrohaHash.hash(encodeTransactionEntrypoint(signedTransaction))
        var norito = Data([signedTransactionWireVersion])
        norito.append(signedTransaction)
        return SignedTransactionEnvelope(
            norito: norito,
            signedTransaction: signedTransaction,
            payload: nil,
            transactionHash: transactionHash
        )
    }

    private static func encodeTransactionPayload(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32?,
        instructionPayload: Data,
        metadata: [String: ToriiJSONValue]
    ) throws -> Data {
        var transactionPayload = OfflineNoritoWriter()
        transactionPayload.writeField(OfflineNorito.encodeString(chainId))
        transactionPayload.writeField(OfflineNorito.encodeString(authority))
        transactionPayload.writeField(OfflineNorito.encodeUInt64(creationTimeMs))
        transactionPayload.writeField(encodeExecutable(instructionPayload: instructionPayload))
        transactionPayload.writeField(try OfflineNorito.encodeOption(ttlMs, encode: OfflineNorito.encodeUInt64))
        transactionPayload.writeField(try OfflineNorito.encodeOption(nonce, encode: OfflineNorito.encodeUInt32))
        transactionPayload.writeField(try OfflineNorito.encodeMetadata(metadata))
        return transactionPayload.data
    }

    private static func encodeExecutable(instructionPayload: Data) -> Data {
        var instructions = OfflineNoritoWriter()
        instructions.writeLength(1)
        instructions.writeField(instructionPayload)

        var executable = OfflineNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)
        return executable.data
    }

    private static func encodeSignedTransaction(
        signature: Data,
        transactionPayload: Data
    ) -> Data {
        var signedTransaction = OfflineNoritoWriter()
        signedTransaction.writeField(OfflineNorito.encodeConstVec(signature))
        signedTransaction.writeField(transactionPayload)
        signedTransaction.writeField(Data([0]))
        signedTransaction.writeField(Data([0]))
        return signedTransaction.data
    }

    private static func encodeTransactionEntrypoint(_ signedTransaction: Data) -> Data {
        var entrypoint = OfflineNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(signedTransaction)
        return entrypoint.data
    }
}

public enum KagemushaRecursiveRedeemRequestArchive {
    public static let typeName = "KagemushaRecursiveSpendRedeemRequestV1"
    public static let schemaName = KagemushaWireNames.recursiveRedeemRequest

    public static func validate(_ archive: Data) throws {
        guard !archive.isEmpty else {
            throw KagemushaRecursiveRedeemRequestArchiveError.emptyRequestArchive
        }
        guard archive.count <= KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes else {
            throw KagemushaRecursiveRedeemRequestArchiveError.oversizedRequestArchive
        }
        guard let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.paddingLength <= KagemushaInstructionTransactionEncoder.maxNoritoHeaderPaddingBytes else {
            throw KagemushaRecursiveRedeemRequestArchiveError.invalidRequestArchive
        }
        guard noritoSchemaHash(forTypeName: schemaName) == frame.header.schema else {
            throw KagemushaRecursiveRedeemRequestArchiveError.unsupportedRequestArchiveType
        }
        guard !frame.payload.isEmpty else {
            throw KagemushaRecursiveRedeemRequestArchiveError.invalidRequestArchive
        }
    }
}

extension SwiftTransactionEncoder {
    static func encodeKagemushaInstruction(
        request: KagemushaInstructionTransactionRequest,
        keypair: Keypair,
        creationTimeMs: UInt64
    ) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeKagemushaInstruction(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeKagemushaInstruction(
        request: KagemushaInstructionTransactionRequest,
        signingKey: SigningKey,
        creationTimeMs: UInt64
    ) throws -> SignedTransactionEnvelope {
        try KagemushaInstructionTransactionEncoder.encode(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeKagemushaRecursiveRedeem(
        request: KagemushaRecursiveRedeemTransactionRequest,
        keypair: Keypair,
        creationTimeMs: UInt64
    ) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeKagemushaRecursiveRedeem(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeKagemushaRecursiveRedeem(
        request: KagemushaRecursiveRedeemTransactionRequest,
        signingKey: SigningKey,
        creationTimeMs: UInt64,
        redeem: (Data) throws -> Data = { try KagemushaRecursiveSpendProver.redeemSpend(requestArchive: $0) }
    ) throws -> SignedTransactionEnvelope {
        try KagemushaInstructionTransactionEncoder.encodeRecursiveRedeem(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs,
            redeem: redeem
        )
    }
}

public extension IrohaSDK {
    func buildKagemushaInstruction(
        request: KagemushaInstructionTransactionRequest,
        keypair: Keypair
    ) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeKagemushaInstruction(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildKagemushaInstruction(
        request: KagemushaInstructionTransactionRequest,
        signingKey: SigningKey
    ) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeKagemushaInstruction(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildKagemushaRecursiveRedeem(
        request: KagemushaRecursiveRedeemTransactionRequest,
        keypair: Keypair
    ) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildKagemushaRecursiveRedeem(
        request: KagemushaRecursiveRedeemTransactionRequest,
        signingKey: SigningKey
    ) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }
}
