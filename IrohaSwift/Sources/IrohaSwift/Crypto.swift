import Foundation
import CryptoKit

public enum SigningAlgorithm: UInt8, CaseIterable, Sendable {
    case ed25519 = 0
    case secp256k1 = 1
    case mlDsa = 4
    case sm2 = 10

    public var noritoDiscriminant: UInt8 { rawValue }

    public init?(noritoDiscriminant: UInt8) {
        self.init(rawValue: noritoDiscriminant)
    }

    public var requiresDistId: Bool {
        switch self {
        case .sm2:
            return true
        default:
            return false
        }
    }
}

public enum SigningStorageBackend: String, Sendable {
    case inMemory
    case secureEnclave
    case bridge
    case external
}

public struct SigningMetadata: Sendable {
    public var distid: String?
    public var label: String?
    public var storage: SigningStorageBackend

    public init(distid: String? = nil,
                label: String? = nil,
                storage: SigningStorageBackend = .inMemory) {
        self.distid = distid
        self.label = label
        self.storage = storage
    }

    public static func inMemory(label: String? = nil) -> SigningMetadata {
        SigningMetadata(distid: nil, label: label, storage: .inMemory)
    }

    public static func secureEnclave(label: String) -> SigningMetadata {
        SigningMetadata(distid: nil, label: label, storage: .secureEnclave)
    }
}

public struct SignatureEnvelope: Sendable {
    public let algorithm: SigningAlgorithm
    public let publicKey: Data
    public let signature: Data
    public let metadata: SigningMetadata?

    public init(algorithm: SigningAlgorithm,
                publicKey: Data,
                signature: Data,
                metadata: SigningMetadata?) {
        self.algorithm = algorithm
        self.publicKey = publicKey
        self.signature = signature
        self.metadata = metadata
    }

    public var noritoAlgorithmIdentifier: UInt8 { algorithm.noritoDiscriminant }
}

public enum SigningKeyError: Error, LocalizedError {
    case unsupportedAlgorithm(String)
    case publicKeyUnavailable

    public var errorDescription: String? {
        switch self {
        case let .unsupportedAlgorithm(label):
            return "Signing algorithm \(label) is not supported in this build."
        case .publicKeyUnavailable:
            return "Unable to load the public key for this signing key."
        }
    }
}

public enum MultihashPrivateKeyError: Error, LocalizedError {
    case invalidFormat(String)
    case invalidLength(expected: Int, actual: Int)
    case unsupportedAlgorithm(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidFormat(reason):
            return "Invalid multihash private key: \(reason)"
        case let .invalidLength(expected, actual):
            return "Multihash private key length mismatch (expected \(expected), got \(actual))."
        case let .unsupportedAlgorithm(reason):
            return "Unsupported multihash private key algorithm: \(reason)"
        }
    }
}

@available(macOS 10.15, iOS 13.0, *)
public struct SigningKey {
    public let algorithm: SigningAlgorithm
    public var metadata: SigningMetadata

    private let signer: (Data) throws -> Data
    private let publicKeyProvider: () throws -> Data
    private let rawPrivateKeyProvider: (() throws -> Data)?

    private init(algorithm: SigningAlgorithm,
                 metadata: SigningMetadata,
                 signer: @escaping (Data) throws -> Data,
                 publicKeyProvider: @escaping () throws -> Data,
                 rawPrivateKeyProvider: (() throws -> Data)?) {
        self.algorithm = algorithm
        self.metadata = metadata
        self.signer = signer
        self.publicKeyProvider = publicKeyProvider
        self.rawPrivateKeyProvider = rawPrivateKeyProvider
    }

    public func publicKey() throws -> Data {
        try publicKeyProvider()
    }

    public func sign(_ message: Data) throws -> Data {
        try signer(message)
    }

    public func makeEnvelope(message: Data) throws -> SignatureEnvelope {
        let signature = try sign(message)
        let publicKey = try publicKey()
        return SignatureEnvelope(algorithm: algorithm,
                                 publicKey: publicKey,
                                 signature: signature,
                                 metadata: metadata)
    }

    public static func ed25519(privateKey: Data,
                               metadata: SigningMetadata = SigningMetadata()) throws -> SigningKey {
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKey)
        let sanitizedMetadata = metadata.storage == .bridge
            ? SigningMetadata(distid: metadata.distid, label: metadata.label, storage: .inMemory)
            : metadata
        return SigningKey(algorithm: .ed25519,
                          metadata: sanitizedMetadata,
                          signer: { message in try key.signature(for: message) },
                          publicKeyProvider: { key.publicKey.rawRepresentation },
                          rawPrivateKeyProvider: { key.rawRepresentation })
    }

    public static func sm2(_ keypair: Sm2Keypair,
                           metadata: SigningMetadata? = nil) -> SigningKey {
        let resolvedMetadata: SigningMetadata = {
            if var provided = metadata {
                if provided.distid == nil {
                    provided.distid = keypair.distid
                }
                if provided.storage == .inMemory {
                    provided.storage = .bridge
                }
                return provided
            }
            return SigningMetadata(distid: keypair.distid,
                                   label: nil,
                                   storage: .bridge)
        }()
        return SigningKey(algorithm: .sm2,
                          metadata: resolvedMetadata,
                          signer: { message in try keypair.sign(message: message) },
                          publicKeyProvider: { keypair.publicKey },
                          rawPrivateKeyProvider: { keypair.privateKey })
    }

    public static func secp256k1(privateKey: Data,
                                 metadata: SigningMetadata = SigningMetadata()) throws -> SigningKey {
        let keypair = try Secp256k1Keypair(privateKey: privateKey)
        return secp256k1(keypair, metadata: metadata)
    }

    public static func secp256k1(_ keypair: Secp256k1Keypair,
                                 metadata: SigningMetadata = SigningMetadata()) -> SigningKey {
        let sanitizedMetadata = metadata.storage == .bridge
            ? SigningMetadata(distid: metadata.distid, label: metadata.label, storage: .inMemory)
            : metadata
        if let key = try? nativeSigningKey(algorithm: .secp256k1,
                                           privateKey: keypair.privateKey,
                                           metadata: sanitizedMetadata) {
            return key
        }
        return SigningKey(algorithm: .secp256k1,
                          metadata: sanitizedMetadata,
                          signer: { message in try keypair.sign(message: message) },
                          publicKeyProvider: { keypair.publicKey },
                          rawPrivateKeyProvider: { keypair.privateKey })
    }

    public static func mlDsa(privateKey: Data,
                             metadata: SigningMetadata = SigningMetadata()) throws -> SigningKey {
        try nativeSigningKey(algorithm: .mlDsa,
                             privateKey: privateKey,
                             metadata: metadata)
    }

    public static func mldsa(_ keypair: MlDsaKeypair,
                             metadata: SigningMetadata = SigningMetadata()) -> SigningKey {
        (try? nativeSigningKey(algorithm: .mlDsa,
                               privateKey: keypair.secretKey,
                               metadata: metadata))
        ?? SigningKey(algorithm: .mlDsa,
                      metadata: metadata,
                      signer: { message in try keypair.sign(message: message) },
                      publicKeyProvider: { keypair.publicKey },
                      rawPrivateKeyProvider: { keypair.secretKey })
    }
}

extension SigningKey {
    /// Build a signing key from multihash-encoded private key bytes.
    public static func fromMultihashPrivateKey(_ multihash: Data,
                                               metadata: SigningMetadata = SigningMetadata()) throws -> SigningKey {
        let (algorithm, payload) = try MultihashPrivateKey.decode(multihash)
        switch algorithm {
        case .ed25519:
            return try SigningKey.ed25519(privateKey: payload, metadata: metadata)
        case .secp256k1:
            return try SigningKey.secp256k1(privateKey: payload, metadata: metadata)
        case .mlDsa:
            return try SigningKey.mlDsa(privateKey: payload, metadata: metadata)
        case .sm2:
            throw MultihashPrivateKeyError.unsupportedAlgorithm("sm2 private keys require Sm2Keypair")
        }
    }

    func exportPrivateKeyBytes() -> Data? {
        guard let rawPrivateKeyProvider else {
            return nil
        }
        guard let raw = try? rawPrivateKeyProvider() else {
            return nil
        }
        if algorithm.requiresDistId {
            guard let distid = metadata.distid else {
                return nil
            }
            let distidBytes = Data(distid.utf8)
            guard distidBytes.count <= Int(UInt16.max),
                  raw.count == Sm2Keypair.privateKeyLength else {
                return nil
            }
            var length = UInt16(distidBytes.count).bigEndian
            var encoded = Data()
            withUnsafeBytes(of: &length) { encoded.append(contentsOf: $0) }
            encoded.append(distidBytes)
            encoded.append(raw)
            return encoded
        }
        return raw
    }

    private static func nativeSigningKey(algorithm: SigningAlgorithm,
                                         privateKey: Data,
                                         metadata: SigningMetadata) throws -> SigningKey {
        guard let publicKey = NoritoNativeBridge.shared.publicKeyFromPrivate(
            algorithm: algorithm,
            privateKey: privateKey
        ) else {
            throw SigningKeyError.publicKeyUnavailable
        }
        return SigningKey(algorithm: algorithm,
                          metadata: metadata,
                          signer: { message in
                              guard let signature = NoritoNativeBridge.shared.signDetached(
                                algorithm: algorithm,
                                privateKey: privateKey,
                                message: message
                              ) else {
                                  throw SigningKeyError.unsupportedAlgorithm(String(describing: algorithm))
                              }
                              return signature
                          },
                          publicKeyProvider: { publicKey },
                          rawPrivateKeyProvider: { privateKey })
    }
}

private enum MultihashPrivateKey {
    static func decode(_ bytes: Data) throws -> (SigningAlgorithm, Data) {
        let raw = [UInt8](bytes)
        let (functionCode, functionEnd) = try decodeVarint(raw, startIndex: 0)
        let (length, lengthEnd) = try decodeVarint(raw, startIndex: functionEnd)
        guard lengthEnd <= raw.count else {
            throw MultihashPrivateKeyError.invalidFormat("digest size not found")
        }
        let payload = Data(raw[lengthEnd...])
        guard payload.count == Int(length) else {
            throw MultihashPrivateKeyError.invalidLength(expected: Int(length), actual: payload.count)
        }
        let algorithm = try signingAlgorithm(multihashCode: functionCode)
        return (algorithm, payload)
    }

    private static func decodeVarint(_ bytes: [UInt8], startIndex: Int) throws -> (UInt64, Int) {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        var index = startIndex
        while index < bytes.count {
            let byte = bytes[index]
            let chunk = UInt64(byte & 0x7F)
            if shift >= 64 {
                throw MultihashPrivateKeyError.invalidFormat("varint overflow")
            }
            value |= chunk << shift
            index += 1
            if (byte & 0x80) == 0 {
                return (value, index)
            }
            shift += 7
        }
        throw MultihashPrivateKeyError.invalidFormat("varint truncated")
    }

    private static func signingAlgorithm(multihashCode: UInt64) throws -> SigningAlgorithm {
        switch multihashCode {
        case 0x1300:
            return .ed25519
        case 0x1301:
            return .secp256k1
        case 0x130b:
            return .mlDsa
        case 0x1311:
            return .sm2
        default:
            let reason = String(format: "multihash code 0x%X", multihashCode)
            throw MultihashPrivateKeyError.unsupportedAlgorithm(reason)
        }
    }
}

@available(macOS 10.15, iOS 13.0, *)
public struct Keypair {
    public let privateKey: Curve25519.Signing.PrivateKey
    public var publicKey: Data { privateKey.publicKey.rawRepresentation }
    public var privateKeyBytes: Data { privateKey.rawRepresentation }

    @available(macOS 10.15, iOS 13.0, *)
    public static func generate() throws -> Keypair {
        return Keypair(privateKey: Curve25519.Signing.PrivateKey())
    }

    @available(macOS 10.15, iOS 13.0, *)
    init(privateKey: Curve25519.Signing.PrivateKey) {
        self.privateKey = privateKey
    }

    @available(macOS 10.15, iOS 13.0, *)
    public init(privateKeyBytes: Data) throws {
        self.privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyBytes)
    }

    @available(macOS 10.15, iOS 13.0, *)
    public func sign(_ message: Data) throws -> Data {
        return try privateKey.signature(for: message)
    }

    /// Build i105 format account ID for this keypair.
    ///
    /// - Parameters:
    ///   - networkPrefix: Network prefix for i105 encoding (defaults to Iroha mainnet)
    /// - Returns: Account ID in format `<i105>`
    /// - Throws: `AccountAddressError` if conversion fails
    @available(macOS 10.15, iOS 13.0, *)
    public func accountId(networkPrefix: UInt16 = AccountId.defaultNetworkPrefix) throws -> String {
        try AccountId.makeI105(publicKey: publicKey, networkPrefix: networkPrefix)
    }
}

public enum AccountId {
    /// Default network prefix for i105 encoding (Iroha mainnet).
    public static let defaultNetworkPrefix: UInt16 = 0x02F1

    /// Build an encoded account id literal (i105).
    public static func make(publicKey: Data) -> String {
        do {
            return try makeI105(publicKey: publicKey)
        } catch {
            preconditionFailure("Invalid account id inputs: \(error)")
        }
    }

    /// Build i105 format account ID string required by Torii API.
    ///
    /// Torii API requires account IDs in i105 format for canonical output; this method
    /// converts a public key to the correct format.
    ///
    /// - Parameters:
    ///   - publicKey: Public key bytes (32 bytes for ed25519, 33 for secp256k1)
    ///   - algorithm: Signing algorithm ("ed25519" or "secp256k1"), defaults to "ed25519"
    ///   - networkPrefix: Network prefix for i105 encoding (defaults to Iroha mainnet)
    /// - Returns: Account ID in format `<i105>`
    /// - Throws: `AccountAddressError` if conversion fails
    public static func makeI105(
        publicKey: Data,
        algorithm: String = "ed25519",
        networkPrefix: UInt16 = defaultNetworkPrefix
    ) throws -> String {
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: algorithm)
        return try address.toI105(networkPrefix: networkPrefix)
    }

    /// Normalizes account id literals for equality checks.
    ///
    /// Semantics:
    /// - If the literal is an encoded `AccountAddress`, returns the canonical Katakana i105 rendering.
    /// - Otherwise, returns the trimmed literal unchanged.
    public static func normalizeForComparison(
        _ literal: String,
        expectedPrefix: UInt16 = defaultNetworkPrefix
    ) -> String {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }

        if let address = try? AccountAddress.parseEncoded(trimmed, expectedPrefix: expectedPrefix),
           let i105 = try? address.toI105(networkPrefix: expectedPrefix) {
            return i105
        }

        return trimmed
    }

    /// Returns true when both account id literals refer to the same address under `normalizeForComparison`.
    public static func matchesForComparison(
        _ lhs: String,
        _ rhs: String,
        expectedPrefix: UInt16 = defaultNetworkPrefix
    ) -> Bool {
        normalizeForComparison(lhs, expectedPrefix: expectedPrefix) == normalizeForComparison(rhs, expectedPrefix: expectedPrefix)
    }
}

public enum Secp256k1Error: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case invalidKeyLength
    case invalidSignatureLength
    case signFailed
    case verifyFailed

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "secp256k1 support is unavailable."
            )
        case .invalidKeyLength:
            return "secp256k1 keys must be 32-byte private / 33-byte compressed public values."
        case .invalidSignatureLength:
            return "secp256k1 signatures must be 64 bytes (r∥s)."
        case .signFailed:
            return "Failed to produce a secp256k1 signature."
        case .verifyFailed:
            return "secp256k1 verification could not be performed."
        }
    }
}

public struct Secp256k1Keypair: Sendable {
    public static let privateKeyLength = 32
    public static let publicKeyLength = 33
    public static let signatureLength = 64

    public let privateKey: Data
    public let publicKey: Data

    public init(privateKey: Data, publicKey: Data? = nil) throws {
        guard privateKey.count == Self.privateKeyLength else {
            throw Secp256k1Error.invalidKeyLength
        }
        let resolvedPublicKey: Data
        if let publicKey {
            guard publicKey.count == Self.publicKeyLength else {
                throw Secp256k1Error.invalidKeyLength
            }
            resolvedPublicKey = publicKey
        } else if let derived = NoritoNativeBridge.shared.publicKeyFromPrivate(
            algorithm: .secp256k1,
            privateKey: privateKey
        ) {
            guard derived.count == Self.publicKeyLength else {
                throw Secp256k1Error.invalidKeyLength
            }
            resolvedPublicKey = derived
        } else {
            guard let derived = NoritoNativeBridge.shared.secp256k1PublicKey(privateKey: privateKey) else {
                throw Secp256k1Error.bridgeUnavailable
            }
            resolvedPublicKey = derived
        }
        guard resolvedPublicKey.count == Self.publicKeyLength else {
            throw Secp256k1Error.invalidKeyLength
        }
        self.privateKey = privateKey
        self.publicKey = resolvedPublicKey
    }

    public func sign(message: Data) throws -> Data {
        guard privateKey.count == Self.privateKeyLength else {
            throw Secp256k1Error.invalidKeyLength
        }
        if let signature = NoritoNativeBridge.shared.signDetached(algorithm: .secp256k1,
                                                                  privateKey: privateKey,
                                                                  message: message) {
            guard signature.count == Self.signatureLength else {
                throw Secp256k1Error.invalidSignatureLength
            }
            return signature
        }
        guard let signature = NoritoNativeBridge.shared.secp256k1Sign(privateKey: privateKey, message: message),
              signature.count == Self.signatureLength else {
            throw Secp256k1Error.signFailed
        }
        return signature
    }

    public func verify(message: Data, signature: Data) throws -> Bool {
        guard signature.count == Self.signatureLength else {
            throw Secp256k1Error.invalidSignatureLength
        }
        if let verified = NoritoNativeBridge.shared.verifyDetached(algorithm: .secp256k1,
                                                                   publicKey: publicKey,
                                                                   message: message,
                                                                   signature: signature) {
            return verified
        }
        guard let verified = NoritoNativeBridge.shared.secp256k1Verify(publicKey: publicKey,
                                                                       message: message,
                                                                       signature: signature) else {
            throw Secp256k1Error.verifyFailed
        }
        return verified
    }
}

public enum Sm2Error: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case invalidKeyLength
    case invalidSignatureLength
    case signFailed
    case verifyFailed

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage("SM2 support is unavailable.")
        case .invalidKeyLength:
            return "SM2 keys must be 32-byte private / 65-byte public values."
        case .invalidSignatureLength:
            return "SM2 signatures must be 64 bytes (r∥s)."
        case .signFailed:
            return "Failed to produce an SM2 signature."
        case .verifyFailed:
            return "SM2 verification could not be performed."
        }
    }
}

public struct Sm2Keypair: Sendable {
    public static let privateKeyLength = 32
    public static let publicKeyLength = 65
    public static let signatureLength = 64

    public let distid: String
    public let privateKey: Data
    public let publicKey: Data

    public init(distid: String, privateKey: Data, publicKey: Data) throws {
        guard privateKey.count == Self.privateKeyLength else {
            throw Sm2Error.invalidKeyLength
        }
        guard publicKey.count == Self.publicKeyLength else {
            throw Sm2Error.invalidKeyLength
        }
        self.distid = distid
        self.privateKey = privateKey
        self.publicKey = publicKey
    }

    public static func defaultDistid() -> String {
        NoritoNativeBridge.shared.sm2DefaultDistid() ?? "1234567812345678"
    }

    public static func deriveFromSeed(distid: String? = nil, seed: Data) throws -> Sm2Keypair {
        let targetDistid = distid ?? defaultDistid()
        guard let pair = NoritoNativeBridge.shared.sm2KeypairFromSeed(distid: targetDistid, seed: seed) else {
            throw Sm2Error.bridgeUnavailable
        }
        return try Sm2Keypair(distid: targetDistid, privateKey: pair.privateKey, publicKey: pair.publicKey)
    }

    public func sign(message: Data) throws -> Data {
        guard let signature = NoritoNativeBridge.shared.sm2Sign(distid: distid, privateKey: privateKey, message: message),
              signature.count == Sm2Keypair.signatureLength else {
            throw Sm2Error.signFailed
        }
        return signature
    }

    public func verify(message: Data, signature: Data) throws -> Bool {
        guard signature.count == Sm2Keypair.signatureLength else {
            throw Sm2Error.invalidSignatureLength
        }
        guard let result = NoritoNativeBridge.shared.sm2Verify(distid: distid, publicKey: publicKey, message: message, signature: signature) else {
            throw Sm2Error.verifyFailed
        }
        return result
    }

    public func publicKeyPrefixed() throws -> String {
        guard let prefixed = NoritoNativeBridge.shared.sm2PublicKeyPrefixed(distid: distid, publicKey: publicKey) else {
            throw Sm2Error.bridgeUnavailable
        }
        return prefixed
    }

    public func publicKeyMultihash() throws -> String {
        guard let multihash = NoritoNativeBridge.shared.sm2PublicKeyMultihash(distid: distid, publicKey: publicKey) else {
            throw Sm2Error.bridgeUnavailable
        }
        return multihash
    }

    public func computeZA() throws -> Data {
        guard let za = NoritoNativeBridge.shared.sm2ComputeZa(distid: distid, publicKey: publicKey) else {
            throw Sm2Error.bridgeUnavailable
        }
        return za
    }
}

public enum MlDsaError: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case invalidKeyLength
    case invalidSignatureLength
    case generateFailed
    case signFailed
    case verifyFailed

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage("ML-DSA support is unavailable.")
        case .invalidKeyLength:
            return "ML-DSA keys do not match the expected length for this suite."
        case .invalidSignatureLength:
            return "ML-DSA signatures must match the suite's signature length."
        case .generateFailed:
            return "Failed to generate an ML-DSA keypair."
        case .signFailed:
            return "Failed to produce an ML-DSA signature."
        case .verifyFailed:
            return "ML-DSA verification could not be performed."
        }
    }
}

public struct MlDsaParameters: Sendable {
    public let publicKeyLength: Int
    public let secretKeyLength: Int
    public let signatureLength: Int
}

public enum MlDsaSuite: UInt8, CaseIterable, Sendable {
    case mlDsa44 = 0
    case mlDsa65 = 1
    case mlDsa87 = 2

    func parameters() throws -> MlDsaParameters {
        guard let params = NoritoNativeBridge.shared.mldsaParameters(suiteId: rawValue) else {
            throw MlDsaError.bridgeUnavailable
        }
        return MlDsaParameters(publicKeyLength: params.publicKeyLength,
                               secretKeyLength: params.secretKeyLength,
                               signatureLength: params.signatureLength)
    }
}

public struct MlDsaKeypair: Sendable {
    public let suite: MlDsaSuite
    public let publicKey: Data
    public let secretKey: Data
    private let params: MlDsaParameters

    public init(suite: MlDsaSuite, publicKey: Data, secretKey: Data) throws {
        let parameters = try suite.parameters()
        guard publicKey.count == parameters.publicKeyLength,
              secretKey.count == parameters.secretKeyLength else {
            throw MlDsaError.invalidKeyLength
        }
        self.suite = suite
        self.publicKey = publicKey
        self.secretKey = secretKey
        self.params = parameters
    }

    public static func generate(suite: MlDsaSuite) throws -> MlDsaKeypair {
        let parameters = try suite.parameters()
        guard let pair = NoritoNativeBridge.shared.mldsaGenerateKeypair(
            suiteId: suite.rawValue,
            publicKeyLength: parameters.publicKeyLength,
            secretKeyLength: parameters.secretKeyLength
        ) else {
            throw MlDsaError.generateFailed
        }
        return try MlDsaKeypair(suite: suite, publicKey: pair.publicKey, secretKey: pair.secretKey)
    }

    public func sign(message: Data) throws -> Data {
        guard let signature = NoritoNativeBridge.shared.mldsaSign(
            suiteId: suite.rawValue,
            secretKey: secretKey,
            message: message,
            signatureLength: params.signatureLength
        ) else {
            throw MlDsaError.signFailed
        }
        return signature
    }

    public func verify(message: Data, signature: Data) throws -> Bool {
        guard signature.count == params.signatureLength else {
            throw MlDsaError.invalidSignatureLength
        }
        guard let result = NoritoNativeBridge.shared.mldsaVerify(
            suiteId: suite.rawValue,
            publicKey: publicKey,
            message: message,
            signature: signature
        ) else {
            throw MlDsaError.verifyFailed
        }
        return result
    }
}
