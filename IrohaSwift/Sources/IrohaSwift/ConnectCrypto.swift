import Foundation
import CryptoKit

public enum ConnectCryptoError: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case invalidPrivateKeyLength(expected: Int, actual: Int)
    case invalidPublicKeyLength(expected: Int, actual: Int)
    case invalidSessionIdentifierLength(expected: Int, actual: Int)
    case invalidNonceLength(expected: Int, actual: Int)
    case invalidRelayToken
    case allZeroPublicKey
    case allZeroNonce
    case invalidApprovalInput(String)
    case invalidApprovalSignature

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "NoritoBridge connect crypto functions are unavailable."
            )
        case let .invalidPrivateKeyLength(expected, actual):
            return "Connect private keys must be \(expected) bytes (got \(actual))."
        case let .invalidPublicKeyLength(expected, actual):
            return "Connect public keys must be \(expected) bytes (got \(actual))."
        case let .invalidSessionIdentifierLength(expected, actual):
            return "Connect session identifiers must be \(expected) bytes (got \(actual))."
        case let .invalidNonceLength(expected, actual):
            return "Connect session nonces must be \(expected) bytes (got \(actual))."
        case .invalidRelayToken:
            return "Connect relay tokens must not be empty."
        case .allZeroPublicKey:
            return "Connect public keys must not be all-zero."
        case .allZeroNonce:
            return "Connect session nonces must not be all-zero."
        case .invalidApprovalInput(let field):
            return "Connect approval input \(field) is invalid."
        case .invalidApprovalSignature:
            return "Connect approval signature verification failed."
        }
    }
}

public struct ConnectKeyPair: Sendable {
    public let publicKey: Data
    public let privateKey: Data

    public init(publicKey: Data, privateKey: Data) {
        self.publicKey = publicKey
        self.privateKey = privateKey
    }
}

extension ConnectKeyPair: Equatable {
    public static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.publicKey == rhs.publicKey && lhs.privateKey == rhs.privateKey
    }
}

extension ConnectKeyPair: Codable {
    private enum CodingKeys: String, CodingKey {
        case publicKey
        case privateKey
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let publicKeyBase64 = try container.decode(String.self, forKey: .publicKey)
        let privateKeyBase64 = try container.decode(String.self, forKey: .privateKey)
        guard let publicKey = Data(base64Encoded: publicKeyBase64),
              let privateKey = Data(base64Encoded: privateKeyBase64) else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(codingPath: decoder.codingPath,
                                      debugDescription: "Invalid base64 in ConnectKeyPair")
            )
        }
        self.init(publicKey: publicKey, privateKey: privateKey)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(publicKey.base64EncodedString(), forKey: .publicKey)
        try container.encode(privateKey.base64EncodedString(), forKey: .privateKey)
    }
}

public struct ConnectDirectionKeys: Sendable {
    public let appToWallet: Data
    public let walletToApp: Data

    public init(appToWallet: Data, walletToApp: Data) {
        self.appToWallet = appToWallet
        self.walletToApp = walletToApp
    }
}

public enum ConnectCrypto {
    private static let keyLength = 32
    private static let nonceLength = 16
    private static let sidDomain = Data("iroha-connect|sid|".utf8)
    private static let relayAuthDomain = Data("iroha-connect|relay-auth|v1".utf8)

    private static func ensureBridgeAvailable() throws {
        if !NoritoNativeBridge.shared.isConnectCryptoAvailable {
            throw ConnectCryptoError.bridgeUnavailable
        }
    }

    /// Derive the canonical session id from an exact network, app key, and fresh nonce.
    public static func deriveSessionID(networkID: NetworkId,
                                       appPublicKey: Data,
                                       nonce: Data) throws -> Data {
        guard appPublicKey.count == keyLength else {
            throw ConnectCryptoError.invalidPublicKeyLength(expected: keyLength, actual: appPublicKey.count)
        }
        guard nonce.count == nonceLength else {
            throw ConnectCryptoError.invalidNonceLength(expected: nonceLength, actual: nonce.count)
        }
        guard appPublicKey.contains(where: { $0 != 0 }) else {
            throw ConnectCryptoError.allZeroPublicKey
        }
        guard nonce.contains(where: { $0 != 0 }) else {
            throw ConnectCryptoError.allZeroNonce
        }
        var preimage = sidDomain
        preimage.append(networkID.bytes)
        preimage.append(appPublicKey)
        preimage.append(nonce)
        return Blake2b.hash256(preimage)
    }

    @discardableResult
    public static func generateKeyPair() throws -> ConnectKeyPair {
        try ensureBridgeAvailable()
        guard let pair = NoritoNativeBridge.shared.connectGenerateKeypair() else {
            throw ConnectCryptoError.bridgeUnavailable
        }
        return ConnectKeyPair(publicKey: pair.publicKey, privateKey: pair.privateKey)
    }

    public static func publicKey(fromPrivateKey privateKey: Data) throws -> Data {
        try ensureBridgeAvailable()
        guard privateKey.count == keyLength else {
            throw ConnectCryptoError.invalidPrivateKeyLength(expected: keyLength, actual: privateKey.count)
        }
        guard let publicKey = NoritoNativeBridge.shared.connectPublicFromPrivate(privateKey) else {
            throw ConnectCryptoError.bridgeUnavailable
        }
        return publicKey
    }

    public static func deriveDirectionKeys(localPrivateKey: Data,
                                           peerPublicKey: Data,
                                           sessionID: Data) throws -> ConnectDirectionKeys {
        try ensureBridgeAvailable()
        guard localPrivateKey.count == keyLength else {
            throw ConnectCryptoError.invalidPrivateKeyLength(expected: keyLength, actual: localPrivateKey.count)
        }
        guard peerPublicKey.count == keyLength else {
            throw ConnectCryptoError.invalidPublicKeyLength(expected: keyLength, actual: peerPublicKey.count)
        }
        guard sessionID.count == keyLength else {
            throw ConnectCryptoError.invalidSessionIdentifierLength(expected: keyLength, actual: sessionID.count)
        }
        guard let derived = NoritoNativeBridge.shared.connectDeriveKeys(privateKey: localPrivateKey,
                                                                        peerPublicKey: peerPublicKey,
                                                                        sessionID: sessionID) else {
            throw ConnectCryptoError.bridgeUnavailable
        }
        return ConnectDirectionKeys(appToWallet: derived.appKey, walletToApp: derived.walletKey)
    }

    public static func relayAuthHash(sessionID: Data, relayToken: String) throws -> Data {
        guard sessionID.count == keyLength else {
            throw ConnectCryptoError.invalidSessionIdentifierLength(expected: keyLength, actual: sessionID.count)
        }
        guard !relayToken.isEmpty else {
            throw ConnectCryptoError.invalidRelayToken
        }
        var payload = relayAuthDomain
        payload.append(sessionID)
        payload.append(contentsOf: relayToken.utf8)
        return Data(SHA256.hash(data: payload))
    }

    /// Builds the canonical approval preimage bound to the exact deployment, request, and relay.
    public static func buildApprovalPreimage(networkID: NetworkId,
                                             sessionID: Data,
                                             appPublicKey: Data,
                                             walletPublicKey: Data,
                                             accountID: String,
                                             permissions: ConnectPermissions?,
                                             proof: ConnectSignInProof?,
                                             relayAuthHash: Data) throws -> Data {
        guard sessionID.count == keyLength else {
            throw ConnectCryptoError.invalidSessionIdentifierLength(
                expected: keyLength,
                actual: sessionID.count
            )
        }
        guard appPublicKey.count == keyLength, walletPublicKey.count == keyLength else {
            throw ConnectCryptoError.invalidPublicKeyLength(
                expected: keyLength,
                actual: min(appPublicKey.count, walletPublicKey.count)
            )
        }
        guard relayAuthHash.count == keyLength else {
            throw ConnectCryptoError.invalidApprovalInput("relayAuthHash")
        }
        let canonicalAccount: String
        do {
            let parsed = try exactCanonicalToriiAccountAddress(accountID)
            canonicalAccount = try parsed.address.toI105(networkPrefix: parsed.chainDiscriminant)
        } catch {
            throw ConnectCryptoError.invalidApprovalInput("accountID")
        }

        let constraints = noritoStruct([networkID.bytes])
        var fields: [(String, Data)] = [
            ("domain", Data("iroha-connect|approve|v1".utf8)),
            ("network_id", networkID.bytes),
            ("constraints", Blake2b.hash256(constraints)),
            ("sid", sessionID),
            ("app_pk", appPublicKey),
            ("wallet_pk", walletPublicKey),
            ("account_id", Data(canonicalAccount.utf8)),
        ]
        if let permissions {
            fields.append(("permissions", Blake2b.hash256(try encodePermissions(permissions))))
        }
        if let proof {
            fields.append(("proof", Blake2b.hash256(try encodeProof(proof))))
        }
        fields.append(("relay_auth", relayAuthHash))

        var output = Data()
        for (tag, value) in fields {
            let tagBytes = Data(tag.utf8)
            appendLittleEndian(UInt16(tagBytes.count), to: &output)
            output.append(tagBytes)
            appendLittleEndian(UInt64(value.count), to: &output)
            output.append(value)
        }
        return output
    }

    /// Verifies an approval against the exact single-key Ed25519 account and session inputs.
    public static func verifyApprovalSignature(networkID: NetworkId,
                                               sessionID: Data,
                                               appPublicKey: Data,
                                               walletPublicKey: Data,
                                               accountID: String,
                                               permissions: ConnectPermissions?,
                                               proof: ConnectSignInProof?,
                                               relayAuthHash: Data,
                                               walletSignature: ConnectWalletSignature) throws {
        guard walletSignature.algorithm == "ed25519",
              Ed25519SignatureAdmission.isValidSignature(walletSignature.signature) else {
            throw ConnectCryptoError.invalidApprovalSignature
        }
        let account: AccountAddress
        do {
            account = try exactCanonicalToriiAccountAddress(accountID).address
        } catch {
            throw ConnectCryptoError.invalidApprovalSignature
        }
        guard let controller = account.singleControllerInfo(),
              controller.algorithm == .ed25519,
              Ed25519PublicKeyAdmission.isValidPublicKey(controller.publicKey),
              let publicKey = try? Curve25519.Signing.PublicKey(
                  rawRepresentation: controller.publicKey
              ) else {
            throw ConnectCryptoError.invalidApprovalSignature
        }
        let preimage = try buildApprovalPreimage(
            networkID: networkID,
            sessionID: sessionID,
            appPublicKey: appPublicKey,
            walletPublicKey: walletPublicKey,
            accountID: accountID,
            permissions: permissions,
            proof: proof,
            relayAuthHash: relayAuthHash
        )
        guard publicKey.isValidSignature(walletSignature.signature, for: preimage) else {
            throw ConnectCryptoError.invalidApprovalSignature
        }
    }

    private static func encodePermissions(_ permissions: ConnectPermissions) throws -> Data {
        let methods = try noritoStringVector(permissions.methods, field: "permissions.methods")
        let events = try noritoStringVector(permissions.events, field: "permissions.events")
        let resources: Data
        if let values = permissions.resources {
            resources = noritoOption(try noritoStringVector(values, field: "permissions.resources"))
        } else {
            resources = Data([0])
        }
        return noritoStruct([methods, events, resources])
    }

    private static func encodeProof(_ proof: ConnectSignInProof) throws -> Data {
        try noritoStruct([
            noritoString(proof.domain, field: "proof.domain"),
            noritoString(proof.uri, field: "proof.uri"),
            noritoString(proof.statement, field: "proof.statement"),
            noritoString(proof.issuedAt, field: "proof.issuedAt"),
            noritoString(proof.nonce, field: "proof.nonce"),
        ])
    }

    private static func noritoStringVector(_ values: [String], field: String) throws -> Data {
        var encoded = Data()
        appendLittleEndian(UInt64(values.count), to: &encoded)
        for (index, value) in values.enumerated() {
            encoded.append(lengthPrefixed(try noritoString(value, field: "\(field)[\(index)]")))
        }
        return encoded
    }

    private static func noritoString(_ value: String, field: String) throws -> Data {
        guard !value.isEmpty,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw ConnectCryptoError.invalidApprovalInput(field)
        }
        return lengthPrefixed(Data(value.utf8))
    }

    private static func noritoOption(_ value: Data) -> Data {
        var output = Data([1])
        output.append(lengthPrefixed(value))
        return output
    }

    private static func noritoStruct(_ fields: [Data]) -> Data {
        var output = Data()
        for field in fields {
            output.append(lengthPrefixed(field))
        }
        return output
    }

    private static func lengthPrefixed(_ value: Data) -> Data {
        var output = Data()
        appendLittleEndian(UInt64(value.count), to: &output)
        output.append(value)
        return output
    }

    private static func appendLittleEndian<T: FixedWidthInteger>(_ value: T, to data: inout Data) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { data.append(contentsOf: $0) }
    }
}
