import Foundation

/// A wallet launch request bound to the caller's configured network and node.
///
/// Parsing does not authenticate a user, grant permissions, derive direction keys, or open a socket.
/// The request consumes one native-decoded application `Open` before allowing the canonical
/// approval preimage to be built. Approval signing and user consent remain with the wallet.
public final class ConnectWalletRequest: @unchecked Sendable, CustomStringConvertible,
                                         CustomDebugStringConvertible, CustomReflectable {
    /// The exact session identifier derived from the launch network, app key and nonce.
    public let sessionID: Data
    /// Canonical unpadded base64url representation of `sessionID`.
    public let sid: String
    /// The configured network that the launch request matched.
    public let networkID: NetworkId
    /// The application key retained from the launch request.
    public let appPublicKey: Data
    /// The nonzero, sixteen-byte nonce retained from the launch request.
    public let nonce: Data
    /// The configured HTTPS node that the launch request matched.
    public let baseURL: URL

    private let token: String
    private let relayToken: String
    // All mutable state is protected by this lock; identity and credentials are immutable.
    private let lock = NSLock()
    private var openAccepted = false
    private var approvalPrepared = false

    private init(sessionID: Data, sid: String, networkID: NetworkId, appPublicKey: Data,
                 nonce: Data, baseURL: URL, token: String, relayToken: String) {
        self.sessionID = sessionID
        self.sid = sid
        self.networkID = networkID
        self.appPublicKey = appPublicKey
        self.nonce = nonce
        self.baseURL = baseURL
        self.token = token
        self.relayToken = relayToken
    }

    /// Parses the canonical wallet URI against independently configured network and node inputs.
    /// The URI may not replace either input. Tokens must use the same canonical encoding as a
    /// Torii session response, and parsing never puts them in an error or description.
    public static func parse(_ literal: String, expectedNetworkID: NetworkId,
                             baseURL: URL) throws -> ConnectWalletRequest {
        let query = try parseConnectLaunchQuery(literal)
        guard query["v"] == "1", query["role"] == "wallet",
              query["network_id"] == expectedNetworkID.literal,
              query["node"] == baseURL.absoluteString else {
            throw ConnectSessionError.protocolViolation("Wallet launch context does not match the configured identity.")
        }
        guard let node = URLComponents(url: baseURL, resolvingAgainstBaseURL: false),
              node.scheme == "https", let host = node.host, !host.isEmpty,
              node.user == nil, node.password == nil, node.query == nil, node.fragment == nil else {
            throw ConnectSessionError.protocolViolation("Wallet launch requires a configured HTTPS node without credentials or query.")
        }
        guard let sid = query["sid"], let appKeyLiteral = query["app_pk"],
              let nonceLiteral = query["nonce"], let token = query["token"],
              let relayToken = query["relay"] else {
            throw ConnectSessionError.protocolViolation("Wallet launch is missing a required field.")
        }
        let sessionID = try decodeConnectBase64URL(sid, byteCount: 32, field: "sid")
        let appPublicKey = try decodeConnectBase64URL(appKeyLiteral, byteCount: 32, field: "app_pk")
        let nonce = try decodeConnectBase64URL(nonceLiteral, byteCount: 16, field: "nonce")
        _ = try decodeConnectBase64URL(token, byteCount: 32, field: "token")
        _ = try decodeConnectBase64URL(relayToken, byteCount: 32, field: "relay")
        guard try ConnectCrypto.deriveSessionID(networkID: expectedNetworkID,
                                               appPublicKey: appPublicKey, nonce: nonce) == sessionID else {
            throw ConnectSessionError.protocolViolation("Wallet launch SID does not match its network, app key and nonce.")
        }
        let request = ConnectWalletRequest(sessionID: sessionID, sid: sid,
            networkID: expectedNetworkID, appPublicKey: appPublicKey, nonce: nonce,
            baseURL: baseURL, token: token, relayToken: relayToken)
        // Apply the existing transport owner's credential policy before returning the request.
        _ = try request.makeWebSocketRequest()
        return request
    }

    /// Builds the canonical wallet-role request, retaining credentials only in its header.
    public func makeWebSocketRequest() throws -> URLRequest {
        try ConnectClient.makeWebSocketRequest(baseURL: baseURL, sid: sid,
                                               role: .wallet, token: token)
    }

    /// Decodes and consumes the one application-to-wallet sequence-one `Open` for this launch.
    /// Invalid inputs do not consume the request; a successfully accepted `Open` cannot replay.
    @discardableResult
    public func acceptOpen(_ bytes: Data) throws -> ConnectOpen {
        let frame = try ConnectCodec.decode(bytes)
        guard frame.sessionID == sessionID, frame.direction == .appToWallet, frame.sequence == 1,
              case .control(.open(let open)) = frame.kind,
              open.appPublicKey == appPublicKey, open.constraints.networkID == networkID else {
            throw ConnectSessionError.protocolViolation("Connect Open does not match the original wallet launch.")
        }
        lock.lock()
        defer { lock.unlock() }
        guard !openAccepted else {
            throw ConnectSessionError.protocolViolation("Connect Open was already accepted.")
        }
        openAccepted = true
        return open
    }

    /// Builds the canonical approval preimage after this request has consumed its original Open.
    /// No signature, account authority, or approval is produced by this operation.
    public func buildApprovalPreimage(walletPublicKey: Data, accountID: String,
                                      permissions: ConnectPermissions?, proof: ConnectSignInProof?) throws -> Data {
        lock.lock()
        let accepted = openAccepted
        lock.unlock()
        guard accepted else {
            throw ConnectSessionError.protocolViolation("Connect Open must be accepted before approval.")
        }
        return try ConnectCrypto.buildApprovalPreimage(networkID: networkID,
            sessionID: sessionID, appPublicKey: appPublicKey, walletPublicKey: walletPublicKey,
            accountID: accountID, permissions: permissions, proof: proof,
            relayAuthHash: ConnectCrypto.relayAuthHash(sessionID: sessionID, relayToken: relayToken))
    }

    /// Verifies and encodes the one wallet-to-app sequence-one approval for this request.
    /// Signing and user consent happen outside this owner. Invalid signatures or codec failures
    /// do not consume approval; a returned frame is bound to this original launch exactly once.
    public func prepareApproval(_ approval: ConnectApprove) throws -> Data {
        lock.lock()
        defer { lock.unlock() }
        guard openAccepted, !approvalPrepared else {
            throw ConnectSessionError.protocolViolation("Approval requires one accepted Open and may be prepared once.")
        }
        try ConnectCrypto.verifyApprovalSignature(networkID: networkID, sessionID: sessionID,
            appPublicKey: appPublicKey, walletPublicKey: approval.walletPublicKey,
            accountID: approval.accountID, permissions: approval.permissions, proof: approval.proof,
            relayAuthHash: ConnectCrypto.relayAuthHash(sessionID: sessionID, relayToken: relayToken),
            walletSignature: approval.walletSignature)
        let bytes = try ConnectCodec.encode(ConnectFrame(sessionID: sessionID,
            direction: .walletToApp, sequence: 1, kind: .control(.approve(approval))))
        approvalPrepared = true
        return bytes
    }

    /// A credential-free diagnostic description.
    public var description: String { "ConnectWalletRequest(launch-bound)" }
    /// A credential-free debug description.
    public var debugDescription: String { description }
    /// Reflection exposes identity only, never the launch URI or bearer/relay credentials.
    public var customMirror: Mirror { Mirror(self, children: ["sid": sid, "networkID": networkID.literal]) }
}
