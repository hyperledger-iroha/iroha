import Foundation

/// Direction of a Connect frame. Encoded via the Norito bridge when available,
/// otherwise falls back to a lightweight JSON shim for environments without the XCFramework.
public enum ConnectDirection: String, Codable, Equatable, Sendable {
    case appToWallet = "AppToWallet"
    case walletToApp = "WalletToApp"
}

public enum ConnectRole: String, Codable, Equatable, Sendable {
    case app
    case wallet
}

public struct ConnectFrame: Codable, Equatable, Sendable {
    public var sessionID: Data
    public var direction: ConnectDirection
    public var sequence: UInt64
    public var kind: ConnectFrameKind

    public init(sessionID: Data, direction: ConnectDirection, sequence: UInt64, kind: ConnectFrameKind) {
        self.sessionID = sessionID
        self.direction = direction
        self.sequence = sequence
        self.kind = kind
    }
}

public enum ConnectFrameKind: Equatable, Sendable {
    case control(ConnectControl)
    case ciphertext(ConnectCiphertext)
}

extension ConnectFrameKind: Codable {
    private enum CodingKeys: String, CodingKey { case tag, control, ciphertext }
    private enum Tag: String, Codable { case control = "Control", ciphertext = "Ciphertext" }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let tag = try container.decode(Tag.self, forKey: .tag)
        switch tag {
        case .control:
            let control = try container.decode(ConnectControl.self, forKey: .control)
            self = .control(control)
        case .ciphertext:
            let payload = try container.decode(ConnectCiphertext.self, forKey: .ciphertext)
            self = .ciphertext(payload)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .control(let control):
            try container.encode(Tag.control, forKey: .tag)
            try container.encode(control, forKey: .control)
        case .ciphertext(let ciphertext):
            try container.encode(Tag.ciphertext, forKey: .tag)
            try container.encode(ciphertext, forKey: .ciphertext)
        }
    }
}

public enum ConnectControl: Equatable, Sendable {
    case open(ConnectOpen)
    case approve(ConnectApprove)
    case reject(ConnectReject)
    case close(ConnectClose)
    case ping(ConnectPing)
    case pong(ConnectPong)
}

extension ConnectControl: Codable {
    private enum CodingKeys: String, CodingKey { case tag, open, approve, reject, close, ping, pong }
    private enum Tag: String, Codable { case open = "Open", approve = "Approve", reject = "Reject", close = "Close", ping = "Ping", pong = "Pong" }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let tag = try container.decode(Tag.self, forKey: .tag)
        switch tag {
        case .open:
            let value = try container.decode(ConnectOpen.self, forKey: .open)
            self = .open(value)
        case .approve:
            let value = try container.decode(ConnectApprove.self, forKey: .approve)
            self = .approve(value)
        case .reject:
            let value = try container.decode(ConnectReject.self, forKey: .reject)
            self = .reject(value)
        case .close:
            let value = try container.decode(ConnectClose.self, forKey: .close)
            self = .close(value)
        case .ping:
            let value = try container.decode(ConnectPing.self, forKey: .ping)
            self = .ping(value)
        case .pong:
            let value = try container.decode(ConnectPong.self, forKey: .pong)
            self = .pong(value)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .open(let value):
            try container.encode(Tag.open, forKey: .tag)
            try container.encode(value, forKey: .open)
        case .approve(let value):
            try container.encode(Tag.approve, forKey: .tag)
            try container.encode(value, forKey: .approve)
        case .reject(let value):
            try container.encode(Tag.reject, forKey: .tag)
            try container.encode(value, forKey: .reject)
        case .close(let value):
            try container.encode(Tag.close, forKey: .tag)
            try container.encode(value, forKey: .close)
        case .ping(let value):
            try container.encode(Tag.ping, forKey: .tag)
            try container.encode(value, forKey: .ping)
        case .pong(let value):
            try container.encode(Tag.pong, forKey: .tag)
            try container.encode(value, forKey: .pong)
        }
    }
}

public struct ConnectCiphertext: Codable, Equatable, Sendable {
    public var payload: Data

    public init(payload: Data) {
        self.payload = payload
    }
}

public struct ConnectOpen: Codable, Equatable, Sendable {
    public var appPublicKey: Data
    public var appMetadata: ConnectAppMetadata?
    public var constraints: ConnectConstraints
    public var permissions: ConnectPermissions?

    public init(appPublicKey: Data,
                appMetadata: ConnectAppMetadata?,
                constraints: ConnectConstraints,
                permissions: ConnectPermissions?) {
        self.appPublicKey = appPublicKey
        self.appMetadata = appMetadata
        self.constraints = constraints
        self.permissions = permissions
    }
}

public struct ConnectApprove: Codable, Equatable, Sendable {
    public var walletPublicKey: Data
    public var accountID: String
    public var permissions: ConnectPermissions?
    public var proof: ConnectSignInProof?
    public var walletSignature: ConnectWalletSignature
    public var walletMetadata: ConnectWalletMetadata?

    public init(walletPublicKey: Data,
                accountID: String,
                permissions: ConnectPermissions? = nil,
                proof: ConnectSignInProof? = nil,
                walletSignature: ConnectWalletSignature,
                walletMetadata: ConnectWalletMetadata? = nil) {
        self.walletPublicKey = walletPublicKey
        self.accountID = accountID
        self.permissions = permissions
        self.proof = proof
        self.walletSignature = walletSignature
        self.walletMetadata = walletMetadata
    }
}

public struct ConnectReject: Codable, Equatable, Sendable {
    public var code: UInt16
    public var codeID: String
    public var reason: String

    public init(code: UInt16, codeID: String, reason: String) {
        self.code = code
        self.codeID = codeID
        self.reason = reason
    }

    private enum CodingKeys: String, CodingKey {
        case code
        case codeID = "code_id"
        case reason
    }
}

public struct ConnectClose: Codable, Equatable, Sendable {
    public var role: ConnectRole
    public var code: UInt16
    public var reason: String?
    public var retryable: Bool

    public init(role: ConnectRole, code: UInt16, reason: String?, retryable: Bool) {
        self.role = role
        self.code = code
        self.reason = reason
        self.retryable = retryable
    }
}

public struct ConnectPing: Codable, Equatable, Sendable {
    public var nonce: UInt64
    public init(nonce: UInt64) { self.nonce = nonce }
}

public struct ConnectPong: Codable, Equatable, Sendable {
    public var nonce: UInt64
    public init(nonce: UInt64) { self.nonce = nonce }
}

public struct ConnectConstraints: Codable, Equatable, Sendable {
    public var chainID: String
    public init(chainID: String) { self.chainID = chainID }
}

public struct ConnectPermissions: Codable, Equatable, Sendable {
    public var methods: [String]
    public var events: [String]
    public var resources: [String]?

    public init(methods: [String] = [], events: [String] = [], resources: [String]? = nil) {
        self.methods = methods
        self.events = events
        self.resources = resources
    }
}

public struct ConnectAppMetadata: Codable, Equatable, Sendable {
    public var name: String?
    public var iconURL: String?
    public var description: String?
    public var iconHash: String?

    public init(name: String?,
                iconURL: String?,
                description: String?,
                iconHash: String? = nil) {
        self.name = name
        self.iconURL = iconURL
        self.description = description
        self.iconHash = iconHash
    }
}

public struct ConnectWalletMetadata: Codable, Equatable, Sendable {
    public var name: String?
    public var iconURL: String?

    public init(name: String?, iconURL: String?) {
        self.name = name
        self.iconURL = iconURL
    }
}

public struct ConnectSignInProof: Codable, Equatable, Sendable {
    public var domain: String?
    public var uri: String?
    public var statement: String?
    public var issuedAt: String?
    public var nonce: String?

    public init(domain: String? = nil,
                uri: String? = nil,
                statement: String? = nil,
                issuedAt: String? = nil,
                nonce: String? = nil) {
        self.domain = domain
        self.uri = uri
        self.statement = statement
        self.issuedAt = issuedAt
        self.nonce = nonce
    }

    private enum CodingKeys: String, CodingKey {
        case domain
        case uri
        case statement
        case issuedAt = "issued_at"
        case nonce
    }
}

public struct ConnectWalletSignature: Codable, Equatable, Sendable {
    public var algorithm: String
    public var signature: Data

    public init(algorithm: String, signature: Data) {
        self.algorithm = algorithm
        self.signature = signature
    }
}
