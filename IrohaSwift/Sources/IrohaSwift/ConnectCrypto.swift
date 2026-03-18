import Foundation

public enum ConnectCryptoError: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case invalidPrivateKeyLength(expected: Int, actual: Int)
    case invalidPublicKeyLength(expected: Int, actual: Int)
    case invalidSessionIdentifierLength(expected: Int, actual: Int)

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

    private static func ensureBridgeAvailable() throws {
        if !NoritoNativeBridge.shared.isConnectCryptoAvailable {
            throw ConnectCryptoError.bridgeUnavailable
        }
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
}
