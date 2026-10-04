import SwiftUI
import Foundation
import Darwin
import Security
#if canImport(CryptoKit)
import CryptoKit
#endif
import UIKit
import CoreImage
import CoreImage.CIFilterBuiltins
#if canImport(IrohaSwift)
import IrohaSwift
#endif

final class ConnectViewModel: ObservableObject {
  enum Role: String, CaseIterable, Identifiable { case app, wallet; var id: String { rawValue } }

  @Published var baseURL: String = "http://localhost:8080"
  @Published var role: Role = .app
  @Published var sid: String = ""
  @Published var tokenApp: String = ""
  @Published var tokenWallet: String = ""
  @Published var tokenRelay: String = ""
  @Published var wsStatus: String = "Disconnected"
  @Published var logs: [String] = []
  @Published var aeadKeyB64: String = "" // 32-byte key, base64
  @Published var localPubB64: String = "" // Curve25519 (X25519) pubkey, base64
  @Published var peerPubB64: String = ""  // Peer pubkey input (base64)
  @Published var sendKeyB64: String = ""  // Derived send key (base64)
  @Published var recvKeyB64: String = ""  // Derived recv key (base64)
  @Published var saltIsBlake2b: Bool = false // Health indicator for salt function
  @Published var handshakeStatus: String = "Idle"
  @Published var lastAppPubB64: String = ""
  @Published var approveAccountId: String = ""
  @Published var approvePrivKeyB64: String = ""
  @Published var approveSigB64: String = ""
  @Published var lastApproveAccount: String = ""
  @Published var lastApproveSigB64: String = ""
  @Published var lastApproveAccountName: String = ""
  @Published var lastApproveAccountDomain: String = ""
  @Published var approveSigValid: Bool? = nil
  @Published var verifiedAccount: String = ""
  // Permissions + Proof UI state
  @Published var networkId: String = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
  @Published var reqPermSignRaw: Bool = true
  @Published var reqPermSignTx: Bool = true
  @Published var reqEventDisplay: Bool = true
  @Published var openRequestedPermsJson: String = ""
  @Published var approvePermsJson: String = ""
  @Published var approveProofJson: String = ""
  @Published var proofDomain: String = ""
  @Published var proofUri: String = ""
  @Published var proofStatement: String = ""
  @Published var proofNonce: String = ""

  private var webSocketTask: URLSessionWebSocketTask?
  private let session = URLSession(configuration: .default)
  private let randomBytes: (UnsafeMutableRawBufferPointer) -> OSStatus
  private var nextSeq: UInt64 = 1
  private var sessionGeneration: UInt64 = 0
  private var launchNonce = Data()
  private var signedApprovalPermissionsJSON: Data?
  private var signedApprovalProofJSON: Data?
#if canImport(IrohaSwift)
  private struct ApprovalBinding {
    let network: NetworkId
    let sessionID: Data
    let appPublicKey: Data
    let nonce: Data
    let relayToken: String
  }
  private var approvalBinding: ApprovalBinding?
  private var approvalAccepted = false
  private var approvalRejected = false
  private var walletOpenAccepted = false
  private var walletRequest: ConnectWalletRequest?
  private var walletRequestPublicKey: Data?
  private var signedWalletApproval: ConnectApprove?
#endif
  private let defaultsVerifiedKey = "NoritoDemo.VerifiedAccount"
  #if canImport(CryptoKit)
  private var localPriv: Curve25519.KeyAgreement.PrivateKey?
  private var keySend: SymmetricKey?
  private var keyRecv: SymmetricKey?
  #endif

  init(randomBytes: @escaping (UnsafeMutableRawBufferPointer) -> OSStatus = {
    SecRandomCopyBytes(kSecRandomDefault, $0.count, $0.baseAddress!)
  }) {
    self.randomBytes = randomBytes
    loadVerifiedAccount()
    applyEnvironmentDefaults()
  }

  private func loadVerifiedAccount() {
    if let saved = UserDefaults.standard.string(forKey: defaultsVerifiedKey), !saved.isEmpty {
      verifiedAccount = saved
    }
  }

  private func persistVerifiedAccount(_ id: String) {
    verifiedAccount = id
    UserDefaults.standard.set(id, forKey: defaultsVerifiedKey)
    log("Persisted verified account: \(id)")
  }

  private func applyEnvironmentDefaults() {
    func value(for key: String) -> String? {
      guard let cString = getenv(key) else { return nil }
      let raw = String(cString: cString).trimmingCharacters(in: .whitespacesAndNewlines)
      return raw.isEmpty ? nil : raw
    }

    if let url = value(for: "TORII_NODE_URL") {
      baseURL = url
    }
    if let token = value(for: "CONNECT_TOKEN_APP") {
      tokenApp = token
    }
    if let token = value(for: "CONNECT_TOKEN_WALLET") {
      tokenWallet = token
    }
    if let token = value(for: "CONNECT_TOKEN_RELAY") {
      tokenRelay = token
    }
    if let network = value(for: "CONNECT_NETWORK_ID") {
      networkId = network
    }
    if let roleRaw = value(for: "CONNECT_ROLE"),
       let newRole = Role(rawValue: roleRaw.lowercased()) {
      role = newRole
    }
    if let peer = value(for: "CONNECT_PEER_PUB_B64") {
      peerPubB64 = peer
    }
    if let shared = value(for: "CONNECT_SHARED_KEY_B64") {
      aeadKeyB64 = shared
    }
    if let approveAccount = value(for: "CONNECT_APPROVE_ACCOUNT_ID") {
      approveAccountId = approveAccount
    }
    if let approvePriv = value(for: "CONNECT_APPROVE_PRIVATE_KEY_B64") {
      approvePrivKeyB64 = approvePriv
    }
    if let approveSig = value(for: "CONNECT_APPROVE_SIGNATURE_B64") {
      approveSigB64 = approveSig
    }
  }

  var walletDeepLink: String {
    guard !sid.isEmpty, !tokenWallet.isEmpty, !tokenRelay.isEmpty,
          let appPublicKey = dataFromBase64OrBase64URL(lastAppPubB64),
          appPublicKey.count == 32, launchNonce.count == 16 else { return "" }
    var components = URLComponents()
    components.scheme = "iroha"
    components.host = "connect"
    components.queryItems = [
      URLQueryItem(name: "sid", value: sid),
      URLQueryItem(name: "network_id", value: networkId),
      URLQueryItem(name: "app_pk", value: base64url(appPublicKey)),
      URLQueryItem(name: "nonce", value: base64url(launchNonce)),
      URLQueryItem(name: "node", value: baseURL),
      URLQueryItem(name: "v", value: "1"),
      URLQueryItem(name: "role", value: "wallet"),
      URLQueryItem(name: "token", value: tokenWallet),
      URLQueryItem(name: "relay", value: tokenRelay),
    ]
    return components.string ?? ""
  }

  // The OS launch handler and tests use this SDK-owned parser; no URI is logged.
  @discardableResult
  func importWalletLaunch(_ literal: String) -> Bool {
#if canImport(IrohaSwift) && canImport(CryptoKit)
    do {
      guard webSocketTask == nil, walletRequest == nil, approvalBinding == nil,
            !walletOpenAccepted, !approvalRejected else {
        throw ConnectSessionError.protocolViolation("Reset the current session before importing a launch.")
      }
      guard let node = URL(string: baseURL) else {
        throw ConnectSessionError.protocolViolation("Configure the trusted wallet node first.")
      }
      let request = try ConnectWalletRequest.parse(literal,
        expectedNetworkID: NetworkId(literal: networkId), baseURL: node)
      let pair = try ConnectCrypto.generateKeyPair()
      let privateKey = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: pair.privateKey)
      guard privateKey.publicKey.rawRepresentation == pair.publicKey else {
        throw ConnectSessionError.protocolViolation("Wallet key generation failed.")
      }
      resetSessionHandshake()
      role = .wallet
      walletRequest = request
      walletRequestPublicKey = pair.publicKey
      localPriv = privateKey
      localPubB64 = pair.publicKey.base64EncodedString()
      launchNonce = request.nonce
      lastAppPubB64 = request.appPublicKey.base64EncodedString()
      sid = request.sid
      // The request owns launch credentials; do not publish them to editable UI fields.
      tokenApp = ""
      tokenWallet = ""
      tokenRelay = ""
      peerPubB64 = ""
      handshakeStatus = "Wallet launch imported; awaiting Open"
      log("Wallet launch imported")
      return true
    } catch {
      rejectApproval("Wallet launch refused")
      return false
    }
#else
    log("Wallet launch requires IrohaSwift and CryptoKit")
    return false
#endif
  }

#if canImport(IrohaSwift) && canImport(CryptoKit)
  private func requireWalletRequest() throws -> ConnectWalletRequest {
    guard role == .wallet, !approvalRejected, let request = walletRequest,
          networkId == request.networkID.literal, baseURL == request.baseURL.absoluteString,
          sid == request.sid, launchNonce == request.nonce,
          lastAppPubB64 == request.appPublicKey.base64EncodedString(),
          localPriv?.publicKey.rawRepresentation == walletRequestPublicKey else {
      throw ConnectSessionError.protocolViolation("The original wallet launch is no longer current.")
    }
    return request
  }
#endif

  func createSession() {
    // Ensure we have an app ephemeral key for sid computation when acting as app
#if canImport(CryptoKit)
    if localPriv == nil { generateEphemeral() }
    guard let appPk = localPriv?.publicKey.rawRepresentation else {
      log("Generate local key before creating session")
      return
    }
#else
    let appPk = Data()
#endif

    guard let networkIdBytes = decodeNetworkId(networkId) else {
      log("CONNECT_NETWORK_ID must be a canonical NetworkId")
      return
    }
    let launchNetwork = networkId
    var nonce = Data(count: 16)
    let entropyStatus = nonce.withUnsafeMutableBytes(randomBytes)
    guard entropyStatus == errSecSuccess else {
      log("Secure nonce generation failed")
      return
    }
    guard nonce.contains(where: { $0 != 0 }),
          let sidBytes = computeSid(networkId: networkIdBytes, appPk: appPk, nonce: nonce) else {
      log("Failed to derive an exact Connect SID")
      return
    }
    resetSessionHandshake()
    launchNonce = nonce
    lastAppPubB64 = appPk.base64EncodedString()
    let sidB64 = base64url(sidBytes)
    guard let url = URL(string: baseURL + "/v1/connect/session") else { log("Invalid base URL"); return }
    var req = URLRequest(url: url)
    req.httpMethod = "POST"
    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
    req.setValue("application/json", forHTTPHeaderField: "Accept")
    let body: [String: Any] = [
      "sid": sidB64,
      "network_id": launchNetwork,
      "app_pk": base64url(appPk),
      "nonce": base64url(nonce),
      "node": baseURL,
    ]
    req.httpBody = try? JSONSerialization.data(withJSONObject: body)
    log("POST /v1/connect/session (client sid)…")
    session.dataTask(with: req) { [weak self] data, resp, err in
      guard let self = self else { return }
      if let err = err { self.log("Session error: \(err.localizedDescription)"); return }
      guard let http = resp as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
        self.log("HTTP \((resp as? HTTPURLResponse)?.statusCode ?? -1)"); return }
      guard let data = data else { self.log("Empty response"); return }
      do {
        if let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] {
          guard (json["sid"] as? String) == sidB64,
                (json["network_id"] as? String) == launchNetwork,
                (json["app_pk"] as? String) == self.base64url(appPk),
                (json["nonce"] as? String) == self.base64url(nonce) else {
            self.log("Session response substituted the launch identity")
            return
          }
          let sidEcho = sidB64
          let tokApp = (json["token_app"] as? String) ?? ""
          let tokWal = (json["token_wallet"] as? String) ?? ""
          let tokRelay = (json["token_relay"] as? String) ?? ""
          DispatchQueue.main.async {
            _ = self.publishSessionResponse(network: launchNetwork, appPublicKey: appPk,
              nonce: nonce, sessionID: sidEcho, appToken: tokApp,
              walletToken: tokWal, relayToken: tokRelay)
          }
        } else { self.log("Unexpected JSON format") }
      } catch { self.log("Decode error: \(error.localizedDescription)") }
    }.resume()
  }

  // Used by the true main-queue HTTP completion and the stale-response regression.
  @discardableResult
  func publishSessionResponse(network: String, appPublicKey: Data, nonce: Data,
                              sessionID: String, appToken: String,
                              walletToken: String, relayToken: String) -> Bool {
    guard networkId == network, launchNonce == nonce,
          localPriv?.publicKey.rawRepresentation == appPublicKey,
          lastAppPubB64 == appPublicKey.base64EncodedString(),
          let networkBytes = decodeNetworkId(network),
          let derived = computeSid(networkId: networkBytes, appPk: appPublicKey, nonce: nonce),
          base64url(derived) == sessionID else {
      log("Ignored a superseded session response")
      return false
    }
    sid = sessionID
    tokenApp = appToken
    tokenWallet = walletToken
    tokenRelay = relayToken
    log("Session created")
    return true
  }

  // The real join path and tests share request preparation; credentials stay in the header.
  func prepareWebSocketRequest() throws -> URLRequest {
#if canImport(IrohaSwift)
    if role == .wallet, walletRequest != nil {
      let request = try requireWalletRequest().makeWebSocketRequest()
      log("WS connect requested")
      return request
    }
    guard let node = URL(string: baseURL) else { throw ToriiClientError.invalidURL(baseURL) }
    let request = try ConnectClient.makeWebSocketRequest(baseURL: node, sid: sid,
      role: role == .app ? .app : .wallet, token: role == .app ? tokenApp : tokenWallet)
    log("WS connect requested")
    return request
#else
    throw NSError(domain: "NoritoDemo.Connect", code: 1)
#endif
  }

  func joinWebSocket() {
    guard webSocketTask == nil else { log("Disconnect the current socket first"); return }
    let request: URLRequest
    do { request = try prepareWebSocketRequest() }
    catch { log("WS request rejected before connection"); return }
    let task = session.webSocketTask(with: request)
    webSocketTask = task
    task.resume()
    DispatchQueue.main.async { self.wsStatus = "Connected" }
    DispatchQueue.main.async { self.handshakeStatus = "WS connected" }
    receiveLoop()

#if canImport(CryptoKit)
    // Auto-send Open when acting as app (if bridge supports control frames)
    if role == .app {
      if localPriv == nil { generateEphemeral() }
      #if canImport(NoritoBridge)
      sendControlOpen()
      #endif
    }
#endif
  }

  func disconnect() {
    webSocketTask?.cancel(with: .normalClosure, reason: nil)
    webSocketTask = nil
    resetSessionHandshake()
    DispatchQueue.main.async { self.wsStatus = "Disconnected" }
    DispatchQueue.main.async { self.handshakeStatus = "Idle" }
    log("WS disconnected")
  }

  func sendPing() {
    guard let task = webSocketTask else { log("Not connected"); return }
    task.sendPing { [weak self] error in
      if let error = error { self?.log("Ping error: \(error.localizedDescription)") }
      else { self?.log("Ping sent") }
    }
  }

#if canImport(NoritoBridge)
  // MARK: - NoritoBridge helpers (conditional)
  private var manualSymmetricKey: SymmetricKey? {
    guard let d = dataFromBase64OrBase64URL(aeadKeyB64), d.count == 32 else { return nil }
    return SymmetricKey(data: d)
  }
  private var effectiveSendKey: SymmetricKey? { keySend ?? manualSymmetricKey }
  private var effectiveRecvKey: SymmetricKey? { keyRecv ?? manualSymmetricKey }

  func sendEncryptedSignRequestTx() {
    guard let task = webSocketTask else { log("Not connected"); return }
    guard let sk = effectiveSendKey else { log("No send key — derive or enter AEAD key"); return }
    guard let sidData = dataFromBase64OrBase64URL(sid), sidData.count == 32 else { log("sid must be base64/base64url (32 bytes)"); return }

    let bridge = NoritoBridgeKit()
    let seq = nextSeq; nextSeq &+= 1
    do {
      let env = try bridge.encodeEnvelopeSignRequestTx(seq: seq, tx: Data([1,2,3]))
      let dir: UInt8 = (role == .app) ? 0 : 1
      let aad = aadV1(sid: sidData, dir: dir, seq: seq)
      let nonce = nonceFromSeq(seq)
      let aead = try ChaChaPoly.seal(env, using: sk, nonce: nonce, authenticating: aad).combined
      let frame = try bridge.encodeCiphertextFrame(sid: sidData, dir: dir, seq: seq, aead: aead)
      task.send(.data(frame)) { [weak self] err in if let err = err { self?.log("send error: \(err.localizedDescription)") } else { self?.log("Sent SignRequestTx seq=\(seq)") } }
    } catch { log("bridge/send error: \(error.localizedDescription)") }
  }

  func sendEncryptedClose() {
    guard let task = webSocketTask else { log("Not connected"); return }
    guard let sk = effectiveSendKey else { log("No send key — derive or enter AEAD key"); return }
    guard let sidData = dataFromBase64OrBase64URL(sid), sidData.count == 32 else { log("sid must be base64/base64url (32 bytes)"); return }

    let bridge = NoritoBridgeKit()
    let seq = nextSeq; nextSeq &+= 1
    do {
      let env = try bridge.encodeEnvelopeClose(seq: seq, who: (role == .app) ? 0 : 1, code: 1000, reason: "demo", retryable: false)
      let dir: UInt8 = (role == .app) ? 0 : 1
      let aad = aadV1(sid: sidData, dir: dir, seq: seq)
      let nonce = nonceFromSeq(seq)
      let aead = try ChaChaPoly.seal(env, using: sk, nonce: nonce, authenticating: aad).combined
      let frame = try bridge.encodeCiphertextFrame(sid: sidData, dir: dir, seq: seq, aead: aead)
      task.send(.data(frame)) { [weak self] err in if let err = err { self?.log("send close error: \(err.localizedDescription)") } else { self?.log("Sent Close seq=\(seq)") } }
    } catch { log("bridge/send error: \(error.localizedDescription)") }
  }

  private func tryDecodeIncoming(_ data: Data) {
    guard let sk = effectiveRecvKey else { return }
    do {
      let bridge = NoritoBridgeKit()
      let (sidOut, dirOut, seqOut, aead) = try bridge.decodeCiphertextFrame(data)
      let aad = aadV1(sid: sidOut, dir: dirOut, seq: seqOut)
      let pt = try ChaChaPoly.open(ChaChaPoly.SealedBox(combined: aead), using: sk, authenticating: aad)
      let json = try bridge.decodeEnvelopeJson(pt)
      log("Decoded frame seq=\(seqOut) dir=\(dirOut) env=\(json)")
    } catch {
      // Not a ciphertext frame or wrong key — log verbose once
      log("Frame decode failed: \(error.localizedDescription)")
    }
  }

  private func aadV1(sid: Data, dir: UInt8, seq: UInt64) -> Data {
    var out = Data(); out.append("connect:v1".data(using: .utf8)!); out.append(sid); out.append(Data([dir]))
    var le = seq.littleEndian; withUnsafeBytes(of: &le) { out.append(contentsOf: $0) }
    out.append(Data([1])) // kind=ciphertext
    return out
  }
  private func nonceFromSeq(_ seq: UInt64) -> ChaChaPoly.Nonce {
    var n = Data(count: 12); var le = seq.littleEndian
    n.replaceSubrange(4..<12, with: withUnsafeBytes(of: &le) { Data($0) })
    return try! ChaChaPoly.Nonce(data: n)
  }

  // MARK: Control frames (optional via bridge)
  private let ctrlKindOpen: UInt16 = 1
  private let ctrlKindApprove: UInt16 = 2

  private func permsJson(request: Bool) -> Data? {
    var methods = [String]()
    var events = [String]()
    if request {
      if reqPermSignRaw { methods.append("SIGN_REQUEST_RAW") }
      if reqPermSignTx { methods.append("SIGN_REQUEST_TX") }
      if reqEventDisplay { events.append("DISPLAY_REQUEST") }
    } else {
      if reqPermSignRaw { methods.append("SIGN_REQUEST_RAW") }
      if reqPermSignTx { methods.append("SIGN_REQUEST_TX") }
      if reqEventDisplay { events.append("DISPLAY_REQUEST") }
    }
    if methods.isEmpty && events.isEmpty { return nil }
    let obj: [String: Any] = ["methods": methods, "events": events]
    return try? JSONSerialization.data(withJSONObject: obj)
  }

  private func proofJson() -> Data? {
    if proofDomain.isEmpty && proofUri.isEmpty && proofStatement.isEmpty && proofNonce.isEmpty {
      return nil
    }
    let issuedAt = ISO8601DateFormatter().string(from: Date())
    let obj: [String: Any] = [
      "domain": proofDomain,
      "uri": proofUri,
      "statement": proofStatement,
      "issued_at": issuedAt,
      "nonce": proofNonce
    ]
    return try? JSONSerialization.data(withJSONObject: obj)
  }

  // The actual outbound path retains the exact inputs before accepting an approval.
  func prepareControlOpenFrame() throws -> Data {
#if canImport(IrohaSwift)
    guard role == .app, nextSeq == 1, approvalBinding == nil,
          let sk = localPriv, let sidData = dataFromBase64OrBase64URL(sid),
          let network = try? NetworkId(literal: networkId), launchNonce.count == 16,
          try ConnectCrypto.deriveSessionID(networkID: network,
            appPublicKey: sk.publicKey.rawRepresentation, nonce: launchNonce) == sidData else {
      throw ConnectSessionError.protocolViolation("Open requires the original launch identity")
    }
    _ = try ConnectCrypto.relayAuthHash(sessionID: sidData, relayToken: tokenRelay)
    let frame = try NoritoBridgeKit().encodeControlOpenExt(
      sid: sidData, dir: 0, seq: 1, appPub: sk.publicKey.rawRepresentation,
      nonce: launchNonce, appMetaJson: nil, networkId: network.bytes,
      permissionsJson: permsJson(request: true)
    )
    approvalBinding = ApprovalBinding(network: network, sessionID: sidData,
      appPublicKey: sk.publicKey.rawRepresentation, nonce: launchNonce, relayToken: tokenRelay)
    approvalAccepted = false
    approvalRejected = false
    nextSeq = 2
    return frame
#else
    throw NSError(domain: "NoritoDemo.Connect", code: 1)
#endif
  }

  func sendControlOpen() {
    guard let task = webSocketTask else { return }
    do {
      let frame = try prepareControlOpenFrame()
      task.send(.data(frame)) { [weak self] err in
        DispatchQueue.main.async {
          guard let self else { return }
          if let err {
            self.rejectApproval("Open send error: \(err.localizedDescription)")
          } else {
            self.log("Sent identity-bound Open control")
            if !self.approvalAccepted && !self.approvalRejected { self.handshakeStatus = "Open sent" }
          }
        }
      }
    } catch { log("Open encode not available: \(error)") }
  }

  // The real send path uses these exact canonical bytes; tests inspect the same result.
  func prepareControlApproveFrame() throws -> Data {
#if canImport(IrohaSwift) && canImport(CryptoKit)
    let request = try requireWalletRequest()
    guard walletOpenAccepted, nextSeq == 1, let approval = signedWalletApproval,
          approval.walletPublicKey == localPriv?.publicKey.rawRepresentation,
          approval.accountID == approveAccountId,
          approval.walletSignature.signature.base64EncodedString() == approveSigB64 else {
      throw ConnectSessionError.protocolViolation("Sign the original wallet approval before sending it.")
    }
    let bytes = try request.prepareApproval(approval)
    nextSeq = 2
    return bytes
#else
    throw NSError(domain: "NoritoDemo.Connect", code: 1)
#endif
  }

  func sendControlApprove() {
    guard let task = webSocketTask else { return }
    do {
      let frame = try prepareControlApproveFrame()
      let generation = sessionGeneration
      task.send(.data(frame)) { [weak self] err in
        DispatchQueue.main.async {
          guard let self, self.sessionGeneration == generation,
                self.webSocketTask === task, !self.approvalRejected else { return }
          if err != nil { self.rejectApproval("Approval send failed") }
          else { self.log("Sent identity-bound Approve control"); self.handshakeStatus = "Approve sent" }
        }
      }
    } catch { rejectApproval("Approval could not be prepared") }
  }

  private func clearApprovalState() {
    clearDerivedKeys()
    signedApprovalPermissionsJSON = nil
    signedApprovalProofJSON = nil
    approveSigB64 = ""
#if canImport(IrohaSwift)
    signedWalletApproval = nil
#endif
    approveSigValid = nil
    approvePermsJson = ""
    approveProofJson = ""
    lastApproveAccount = ""
    lastApproveSigB64 = ""
    lastApproveAccountName = ""
    lastApproveAccountDomain = ""
    verifiedAccount = ""
    UserDefaults.standard.removeObject(forKey: defaultsVerifiedKey)
  }

  private func resetSessionHandshake() {
    sessionGeneration &+= 1
    clearApprovalState()
#if canImport(IrohaSwift)
    approvalBinding = nil
    approvalAccepted = false
    approvalRejected = false
    walletOpenAccepted = false
    walletRequest = nil
    walletRequestPublicKey = nil
#endif
    nextSeq = 1
  }

  private func rejectApproval(_ reason: String) {
    clearApprovalState()
#if canImport(IrohaSwift)
    approvalRejected = true
#endif
    approveSigValid = false
    log("Approval rejected: \(reason)")
  }

  // Called on the main queue by the real WebSocket receive path and by XCTest.
  func handleIncomingFrame(_ data: Data) {
#if canImport(IrohaSwift)
    do {
      let frame = try ConnectCodec.decode(data)
      guard case .control(let control) = frame.kind else {
        tryDecodeIncoming(data)
        return
      }
      switch control {
      case .open:
        let request = try requireWalletRequest()
        guard !walletOpenAccepted, let sk = localPriv else {
          throw ConnectSessionError.protocolViolation("Unexpected Open identity")
        }
        let open = try request.acceptOpen(data)
        let permissions = try open.permissions.map {
          String(decoding: try JSONEncoder().encode($0), as: UTF8.self)
        } ?? ""
        try installDirectionKeys(privateKey: sk.rawRepresentation,
          peerPublicKey: open.appPublicKey, sessionID: frame.sessionID)
        walletOpenAccepted = true
        lastAppPubB64 = open.appPublicKey.base64EncodedString()
        openRequestedPermsJson = permissions
        handshakeStatus = "Open accepted; keys ready"
      case .approve(let approval):
        guard role == .app, !approvalAccepted, !approvalRejected,
              let binding = approvalBinding, let sk = localPriv,
              frame.direction == .walletToApp, frame.sequence == 1,
              frame.sessionID == binding.sessionID,
              dataFromBase64OrBase64URL(sid) == binding.sessionID,
              try NetworkId(literal: networkId) == binding.network,
              sk.publicKey.rawRepresentation == binding.appPublicKey,
              launchNonce == binding.nonce, tokenRelay == binding.relayToken else {
          throw ConnectSessionError.protocolViolation("Unexpected, replayed, or substituted approval")
        }
        let relayAuth = try ConnectCrypto.relayAuthHash(
          sessionID: binding.sessionID, relayToken: binding.relayToken)
        try ConnectCrypto.verifyApprovalSignature(
          networkID: binding.network, sessionID: binding.sessionID,
          appPublicKey: binding.appPublicKey, walletPublicKey: approval.walletPublicKey,
          accountID: approval.accountID, permissions: approval.permissions, proof: approval.proof,
          relayAuthHash: relayAuth, walletSignature: approval.walletSignature
        )
        let permissions = try approval.permissions.map {
          String(decoding: try JSONEncoder().encode($0), as: UTF8.self)
        } ?? ""
        let proof = try approval.proof.map {
          String(decoding: try JSONEncoder().encode($0), as: UTF8.self)
        } ?? ""
        // No key or success state is published until all validation has succeeded.
        try installDirectionKeys(privateKey: sk.rawRepresentation,
          peerPublicKey: approval.walletPublicKey, sessionID: binding.sessionID)
        approvalAccepted = true
        approveSigValid = true
        approvePermsJson = permissions
        approveProofJson = proof
        lastApproveAccount = approval.accountID
        lastApproveSigB64 = approval.walletSignature.signature.base64EncodedString()
        persistVerifiedAccount(approval.accountID)
        handshakeStatus = "Approved; keys ready"
        log("Approve signature valid; direction keys installed")
      case .reject, .close:
        rejectApproval("Peer refused or closed the session")
      default:
        break
      }
    } catch {
      rejectApproval(error.localizedDescription)
    }
#else
    rejectApproval("Exact approval verification requires IrohaSwift")
#endif
  }

#endif

  // Captured before the actual receive callback; a retired session cannot consume new state.
  func makeIncomingFrameConsumer() -> (Data) -> Bool {
    let generation = sessionGeneration
    let task = webSocketTask
    return { [weak self] data in
      guard let self, self.sessionGeneration == generation,
            self.webSocketTask === task else { return false }
      self.handleIncomingFrame(data)
      return true
    }
  }

  private func receiveLoop() {
    guard let task = webSocketTask else { return }
    let generation = sessionGeneration
    let consume = makeIncomingFrameConsumer()
    task.receive { [weak self] result in
      guard let self = self else { return }
      switch result {
      case .success(let msg):
        switch msg {
        case .string(let s): self.log("WS text (\(s.count))")
        case .data(let d):
          self.log("WS binary (\(d.count) bytes)")
#if canImport(NoritoBridge)
          DispatchQueue.main.async { _ = consume(d) }
#endif
        @unknown default: self.log("WS unknown message")
        }
        DispatchQueue.main.async {
          guard self.sessionGeneration == generation, self.webSocketTask === task else { return }
          self.receiveLoop()
        }
      case .failure(let err):
        self.log("WS recv error: \(err.localizedDescription)")
        DispatchQueue.main.async {
          guard self.sessionGeneration == generation, self.webSocketTask === task else { return }
          self.wsStatus = "Disconnected"
        }
      }
    }
  }

  private func log(_ line: String) {
    let ts = ISO8601DateFormatter().string(from: Date())
    DispatchQueue.main.async { self.logs.append("[\(ts)] \(line)") }
  }

  // MARK: - Key derivation (X25519 + HKDF-SHA256)
  #if canImport(CryptoKit)
  func generateEphemeral() {
    resetSessionHandshake()
    let sk = Curve25519.KeyAgreement.PrivateKey()
    localPriv = sk
    let pk = sk.publicKey.rawRepresentation
    localPubB64 = Data(pk).base64EncodedString()
    log("Generated X25519 keypair; pub exported (base64)")
  }

  private func clearDerivedKeys() {
    keySend = nil
    keyRecv = nil
    sendKeyB64 = ""
    recvKeyB64 = ""
    saltIsBlake2b = false
    handshakeStatus = "Keys unavailable"
  }

  private func installDirectionKeys(privateKey: Data, peerPublicKey: Data, sessionID: Data) throws {
#if canImport(IrohaSwift)
    let keys = try ConnectCrypto.deriveDirectionKeys(
      localPrivateKey: privateKey, peerPublicKey: peerPublicKey, sessionID: sessionID
    )
    let kApp = SymmetricKey(data: keys.appToWallet)
    let kWallet = SymmetricKey(data: keys.walletToApp)
    if role == .app { keySend = kApp; keyRecv = kWallet } else { keySend = kWallet; keyRecv = kApp }
    sendKeyB64 = exportKeyB64(keySend)
    recvKeyB64 = exportKeyB64(keyRecv)
    saltIsBlake2b = true
#else
    throw NSError(domain: "NoritoDemo.Connect", code: 1, userInfo: [
      NSLocalizedDescriptionKey: "Connect key derivation requires the IrohaSwift package"
    ])
#endif
  }

  func deriveKeys() {
    clearDerivedKeys()
    guard let sidRaw = dataFromBase64OrBase64URL(sid), sidRaw.count == 32 else { log("sid must be base64/base64url (32 bytes)"); return }
    guard let sk = localPriv else { log("Generate local key first"); return }
    guard let peerRaw = dataFromBase64OrBase64URL(peerPubB64), peerRaw.count == 32 else { log("Peer pub must be base64/base64url (32 bytes)"); return }
    do {
      try installDirectionKeys(privateKey: sk.rawRepresentation, peerPublicKey: peerRaw, sessionID: sidRaw)
      log("Derived direction keys via HKDF-SHA256 (BLAKE2b-256 salt)")
      handshakeStatus = "Keys ready (manual)"
    } catch {
      log("Key agreement failed: \(error.localizedDescription)")
    }
  }

#if canImport(CryptoKit)

#endif

  private func exportKeyB64(_ key: SymmetricKey?) -> String {
    guard let key else { return "" }
    return key.withUnsafeBytes { Data($0).base64EncodedString() }
  }
  #endif

  // MARK: - Helpers
  func dataFromBase64OrBase64URL(_ s: String) -> Data? {
    if let d = Data(base64Encoded: s), d.base64EncodedString() == s { return d }
    var t = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
    let rem = t.count % 4
    if rem != 0 { t.append(String(repeating: "=", count: 4-rem)) }
    guard let d = Data(base64Encoded: t), base64url(d) == s else { return nil }
    return d
  }

  private func computeSid(networkId: Data, appPk: Data, nonce: Data) -> Data? {
    guard networkId.count == 32, appPk.count == 32, nonce.count == 16,
          let sym = dlsym(UnsafeMutableRawPointer(bitPattern: UInt(bitPattern: -2)), "connect_norito_connect_derive_session_id") else {
      return nil
    }
    typealias DeriveFn = @convention(c) (
      UnsafePointer<UInt8>, CUnsignedLong, UnsafePointer<UInt8>, CUnsignedLong,
      UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>, CUnsignedLong
    ) -> Int32
    let fn = unsafeBitCast(sym, to: DeriveFn.self)
    var out = Data(count: 32)
    let rc = networkId.withUnsafeBytes { np in
      appPk.withUnsafeBytes { ap in
        nonce.withUnsafeBytes { op in
          out.withUnsafeMutableBytes { sp in
            fn(np.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(networkId.count),
               ap.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(appPk.count),
               op.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(nonce.count),
               sp.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(sp.count))
          }
        }
      }
    }
    return rc == 0 ? out : nil
  }

  private func decodeNetworkId(_ literal: String) -> Data? {
    let bytes = Array(literal.utf8)
    guard bytes.count == 74, Array(bytes[0..<5]) == Array("hash:".utf8),
          bytes[69] == 0x23, bytes[5..<69].allSatisfy(isUpperHex),
          bytes[70..<74].allSatisfy(isUpperHex),
          let checksum = UInt16(String(decoding: bytes[70..<74], as: UTF8.self), radix: 16),
          checksum == crc16(bytes[0..<69]) else { return nil }
    var raw = Data(capacity: 32)
    for index in stride(from: 5, to: 69, by: 2) {
      guard let high = hexNibble(bytes[index]), let low = hexNibble(bytes[index + 1]) else { return nil }
      raw.append((high << 4) | low)
    }
    return raw.count == 32 && raw[31] & 1 == 1 ? raw : nil
  }

  private func isUpperHex(_ byte: UInt8) -> Bool {
    (byte >= 0x30 && byte <= 0x39) || (byte >= 0x41 && byte <= 0x46)
  }

  private func hexNibble(_ byte: UInt8) -> UInt8? {
    byte <= 0x39 ? byte - 0x30 : (byte >= 0x41 && byte <= 0x46 ? byte - 0x41 + 10 : nil)
  }

  private func crc16<S: Sequence>(_ bytes: S) -> UInt16 where S.Element == UInt8 {
    var crc: UInt16 = 0xFFFF
    for byte in bytes {
      crc ^= UInt16(byte) << 8
      for _ in 0..<8 { crc = crc & 0x8000 != 0 ? (crc << 1) ^ 0x1021 : crc << 1 }
    }
    return crc
  }

  private func base64url(_ data: Data) -> String {
    let b64 = data.base64EncodedString()
    return b64.replacingOccurrences(of: "+", with: "-")
             .replacingOccurrences(of: "/", with: "_")
             .replacingOccurrences(of: "=", with: "")
  }

#if canImport(CryptoKit)
  func signApprove() {
#if canImport(IrohaSwift)
    signedWalletApproval = nil
    approveSigB64 = ""
    signedApprovalPermissionsJSON = nil
    signedApprovalProofJSON = nil
    do {
      let request = try requireWalletRequest()
      guard walletOpenAccepted, let sk = localPriv,
            let privRaw = dataFromBase64OrBase64URL(approvePrivKeyB64), privRaw.count == 32 else {
        throw ConnectSessionError.protocolViolation("Approval requires the original Open and a signing key.")
      }
      let methods = ([reqPermSignRaw ? "SIGN_REQUEST_RAW" : nil, reqPermSignTx ? "SIGN_REQUEST_TX" : nil]).compactMap { $0 }
      let events = ([reqEventDisplay ? "DISPLAY_REQUEST" : nil]).compactMap { $0 }
      let permissions = methods.isEmpty && events.isEmpty ? nil : ConnectPermissions(methods: methods, events: events)
      let proof: ConnectSignInProof? = proofDomain.isEmpty && proofUri.isEmpty && proofStatement.isEmpty && proofNonce.isEmpty ? nil : ConnectSignInProof(
        domain: proofDomain, uri: proofUri, statement: proofStatement,
        issuedAt: ISO8601DateFormatter().string(from: Date()), nonce: proofNonce
      )
      let walletKey = sk.publicKey.rawRepresentation
      let preimage = try request.buildApprovalPreimage(walletPublicKey: walletKey,
        accountID: approveAccountId, permissions: permissions, proof: proof)
      let signer = try SigningKey.ed25519(privateKey: privRaw)
      let signature = try signer.sign(preimage)
      signedWalletApproval = ConnectApprove(walletPublicKey: walletKey, accountID: approveAccountId,
        permissions: permissions, proof: proof,
        walletSignature: ConnectWalletSignature(algorithm: "ed25519", signature: signature))
      signedApprovalPermissionsJSON = try permissions.map { try JSONEncoder().encode($0) }
      signedApprovalProofJSON = try proof.map { try JSONEncoder().encode($0) }
      approveSigB64 = signature.base64EncodedString()
      log("Identity- and relay-bound Approve signature generated")
    } catch { log("Approval signing refused") }
#else
    log("Exact approval signing requires the IrohaSwift package")
#endif
  }
#endif
}

struct ContentView: View {
  @StateObject private var vm = ConnectViewModel()
  @State private var showShareSheet = false
  @State private var shareItems: [Any] = []

  var body: some View {
    NavigationView {
      VStack(alignment: .leading, spacing: 12) {
        Group {
          TextField("Node URL (http://host:port)", text: $vm.baseURL)
            .autocapitalization(.none)
            .disableAutocorrection(true)
            .textFieldStyle(RoundedBorderTextFieldStyle())

          HStack {
            Text("Role:")
            Picker("Role", selection: $vm.role) {
              ForEach(ConnectViewModel.Role.allCases) { r in
                Text(r.rawValue).tag(r)
              }
            }.pickerStyle(SegmentedPickerStyle())
          }

          HStack(spacing: 10) {
            Button("Create Session") { vm.createSession() }
            Button("Join WS") { vm.joinWebSocket() }
              .disabled(vm.sid.isEmpty)
            Button("Disconnect") { vm.disconnect() }
          }

          Text("sid: \(vm.sid.isEmpty ? "–" : vm.sid)")
            .font(.footnote)
            .foregroundColor(.secondary)
          if !vm.tokenApp.isEmpty || !vm.tokenWallet.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
              Text("Tokens").font(.headline)
              HStack(alignment: .top, spacing: 8) {
                Text("App")
                  .font(.footnote)
                  .foregroundColor(.secondary)
                  .frame(width: 36, alignment: .leading)
                ScrollView(.horizontal) {
                  Text(vm.tokenApp.isEmpty ? "–" : vm.tokenApp)
                    .font(.system(.caption, design: .monospaced))
                }
                Button("Copy") { UIPasteboard.general.string = vm.tokenApp }
                  .disabled(vm.tokenApp.isEmpty)
              }
              HStack(alignment: .top, spacing: 8) {
                Text("Wallet")
                  .font(.footnote)
                  .foregroundColor(.secondary)
                  .frame(width: 36, alignment: .leading)
                ScrollView(.horizontal) {
                  Text(vm.tokenWallet.isEmpty ? "–" : vm.tokenWallet)
                    .font(.system(.caption, design: .monospaced))
                }
                Button("Copy") { UIPasteboard.general.string = vm.tokenWallet }
                  .disabled(vm.tokenWallet.isEmpty)
              }
            }
          }
          Text("WS: \(vm.wsStatus)")
            .font(.footnote)
            .foregroundColor(vm.wsStatus == "Connected" ? .green : .secondary)
          Text("Handshake: \(vm.handshakeStatus)")
            .font(.footnote)
            .foregroundColor(vm.handshakeStatus.contains("Keys ready") ? .green : (vm.handshakeStatus.contains("Open") || vm.handshakeStatus.contains("Approve") ? .blue : .secondary))
          if !vm.verifiedAccount.isEmpty {
            Text("Verified: \(vm.verifiedAccount)")
              .font(.footnote)
              .foregroundColor(.green)
          }
#if canImport(IrohaSwift)
          if #available(iOS 15.0, macOS 12.0, *) {
            Divider()
            TransferHistorySection(baseURL: vm.baseURL,
                                   defaultAccountId: vm.verifiedAccount)
          } else {
            Divider()
            Text("Transfer history requires iOS 15/macOS 12+")
              .font(.footnote)
              .foregroundColor(.secondary)
          }
#endif
          if !vm.walletDeepLink.isEmpty {
            Divider()
            Text("Wallet Deep Link QR").font(.headline)
            QRCodeView(text: vm.walletDeepLink)
              .frame(width: 180, height: 180)
            ScrollView(.horizontal) {
              Text(vm.walletDeepLink)
                .font(.system(.caption, design: .monospaced))
            }
            HStack(spacing: 12) {
              Button("Copy Deeplink") {
                UIPasteboard.general.string = vm.walletDeepLink
              }
              Button("Share Deeplink") {
                shareItems = [vm.walletDeepLink]
                showShareSheet = true
              }
            }
          }
        }

        #if canImport(CryptoKit)
        Divider()
        Text("Key Derivation").font(.headline)
        HStack(spacing: 10) {
          Button("Generate Ephemeral Key") { vm.generateEphemeral() }
          Button("Derive Keys") { vm.deriveKeys() }
        }
        Text("Salt: \(vm.saltIsBlake2b ? "BLAKE2b-256" : "unavailable")")
          .font(.footnote)
          .foregroundColor(vm.saltIsBlake2b ? .green : .secondary)
        Text("Local X25519 Pub (base64)").font(.footnote)
        ScrollView(.horizontal) { Text(vm.localPubB64).font(.system(.caption, design: .monospaced)) }
        TextField("Peer X25519 Pub (base64)", text: $vm.peerPubB64)
          .autocapitalization(.none)
          .disableAutocorrection(true)
          .textFieldStyle(RoundedBorderTextFieldStyle())
        HStack {
          Text("Send key:").font(.footnote); ScrollView(.horizontal) { Text(vm.sendKeyB64).font(.system(.caption, design: .monospaced)) }
        }
        HStack {
          Text("Recv key:").font(.footnote); ScrollView(.horizontal) { Text(vm.recvKeyB64).font(.system(.caption, design: .monospaced)) }
        }
        #endif

#if canImport(NoritoBridge)
        Divider()
        Text("NoritoBridge").font(.headline)
        TextField("Network ID", text: $vm.networkId)
          .autocapitalization(.none)
          .disableAutocorrection(true)
          .textFieldStyle(RoundedBorderTextFieldStyle())
        Text("Request permissions (Open)").font(.footnote)
        Toggle("SIGN_REQUEST_RAW", isOn: $vm.reqPermSignRaw)
        Toggle("SIGN_REQUEST_TX", isOn: $vm.reqPermSignTx)
        Toggle("DISPLAY_REQUEST", isOn: $vm.reqEventDisplay)
        TextField("AEAD Key (base64 32 bytes) — fallback", text: $vm.aeadKeyB64)
          .autocapitalization(.none)
          .disableAutocorrection(true)
          .textFieldStyle(RoundedBorderTextFieldStyle())
        HStack(spacing: 10) {
          Button("Send SignRequestTx") { vm.sendEncryptedSignRequestTx() }
          Button("Send Close") { vm.sendEncryptedClose() }
        }

        if vm.role == .wallet {
          Divider()
          Text("Approve (wallet)").font(.headline)
          if !vm.openRequestedPermsJson.isEmpty {
            Text("Requested permissions (JSON)").font(.footnote)
            ScrollView(.horizontal) { Text(vm.openRequestedPermsJson).font(.system(.caption, design: .monospaced)) }
          }
          if !vm.lastAppPubB64.isEmpty {
            Text("App pub (base64)").font(.footnote)
            ScrollView(.horizontal) { Text(vm.lastAppPubB64).font(.system(.caption, design: .monospaced)) }
          }
          TextField("Account ID", text: $vm.approveAccountId)
            .autocapitalization(.none)
            .disableAutocorrection(true)
            .textFieldStyle(RoundedBorderTextFieldStyle())
          Text("Sign-in proof (optional)").font(.footnote)
          TextField("Domain", text: $vm.proofDomain).textFieldStyle(RoundedBorderTextFieldStyle())
          TextField("URI", text: $vm.proofUri).textFieldStyle(RoundedBorderTextFieldStyle())
          TextField("Statement", text: $vm.proofStatement).textFieldStyle(RoundedBorderTextFieldStyle())
          TextField("Nonce", text: $vm.proofNonce).textFieldStyle(RoundedBorderTextFieldStyle())
          TextField("Wallet Ed25519 private key (base64 32 bytes)", text: $vm.approvePrivKeyB64)
            .autocapitalization(.none)
            .disableAutocorrection(true)
            .textFieldStyle(RoundedBorderTextFieldStyle())
          HStack(spacing: 10) {
            Button("Sign Approve") { vm.signApprove() }
            Button("Send Approve") { vm.sendControlApprove() }
          }
          Text("Approve signature (base64)").font(.footnote)
          ScrollView(.horizontal) { Text(vm.approveSigB64).font(.system(.caption, design: .monospaced)) }
        } else {
          // App role: show received Approve details
          if !vm.lastApproveAccount.isEmpty || !vm.lastApproveSigB64.isEmpty {
            Divider()
            Text("Approve (received)").font(.headline)
            HStack(spacing: 12) {
              if !vm.lastApproveAccountName.isEmpty { Text("Name: \(vm.lastApproveAccountName)").font(.footnote) }
              if !vm.lastApproveAccountDomain.isEmpty { Text("Domain: \(vm.lastApproveAccountDomain)").font(.footnote) }
              if let ok = vm.approveSigValid {
                Text(ok ? "Signature: ✓ Valid" : "Signature: ✗ Invalid").font(.footnote)
                  .foregroundColor(ok ? .green : .red)
              }
            }
            if !vm.approvePermsJson.isEmpty {
              Text("Approved permissions (JSON)").font(.footnote)
              ScrollView(.horizontal) { Text(vm.approvePermsJson).font(.system(.caption, design: .monospaced)) }
            }
            if !vm.approveProofJson.isEmpty {
              Text("Sign-in proof (JSON)").font(.footnote)
              ScrollView(.horizontal) { Text(vm.approveProofJson).font(.system(.caption, design: .monospaced)) }
            }
            if !vm.lastApproveAccount.isEmpty {
              Text("Account ID").font(.footnote)
              ScrollView(.horizontal) { Text(vm.lastApproveAccount).font(.system(.caption, design: .monospaced)) }
            }
            if !vm.lastApproveSigB64.isEmpty {
              Text("Signature (base64)").font(.footnote)
              ScrollView(.horizontal) { Text(vm.lastApproveSigB64).font(.system(.caption, design: .monospaced)) }
            }
          }
        }
#endif

        Divider()
        Text("Logs").font(.headline)
        ScrollView {
          VStack(alignment: .leading, spacing: 6) {
            ForEach(Array(vm.logs.enumerated()), id: \.offset) { _, line in
              Text(line).font(.system(.caption, design: .monospaced))
                .frame(maxWidth: .infinity, alignment: .leading)
            }
          }
        }

        Spacer(minLength: 0)
      }
      .padding()
      .navigationBarTitle("Norito Connect Demo")
      .onOpenURL { _ = vm.importWalletLaunch($0.absoluteString) }
      .sheet(isPresented: $showShareSheet) {
        ActivityView(activityItems: shareItems)
      }
    }
  }
}

struct QRCodeView: View {
  let text: String
  private let context = CIContext()
  private let filter = CIFilter.qrCodeGenerator()

  func makeUIImage() -> UIImage? {
    guard !text.isEmpty else { return nil }
    filter.setValue(Data(text.utf8), forKey: "inputMessage")
    let scale = CGAffineTransform(scaleX: 10, y: 10)
    guard let output = filter.outputImage?.transformed(by: scale),
          let cgimg = context.createCGImage(output, from: output.extent) else { return nil }
    return UIImage(cgImage: cgimg)
  }

  var body: some View {
    if let img = makeUIImage() {
      Image(uiImage: img).interpolation(.none).resizable().scaledToFit()
    } else {
      Color.clear
    }
  }
}

struct ActivityView: UIViewControllerRepresentable {
  let activityItems: [Any]
  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
  }
  func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

#if canImport(IrohaSwift)
@available(iOS 15.0, macOS 12.0, *)
final class TransferHistoryViewModel: ObservableObject {
  @Published var summaries: [ToriiExplorerTransferSummary] = []
  @Published var isLoading = false
  @Published var errorMessage: String?

  func load(baseURL: String, accountId: String) {
    let trimmedAccount = accountId.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmedAccount.isEmpty else {
      errorMessage = "Enter an account id."
      return
    }
    let trimmedURL = baseURL.trimmingCharacters(in: .whitespacesAndNewlines)
    guard let url = URL(string: trimmedURL), !trimmedURL.isEmpty else {
      errorMessage = "Invalid node URL."
      return
    }
    isLoading = true
    errorMessage = nil

    Task { @MainActor in
      do {
        let sdk = IrohaSDK(baseURL: url)
        summaries = try await sdk.getTransactionHistory(accountId: trimmedAccount,
                                                        limit: 25)
      } catch {
        errorMessage = error.localizedDescription
      }
      isLoading = false
    }
  }

  func clear() {
    summaries.removeAll()
    errorMessage = nil
  }
}

@available(iOS 15.0, macOS 12.0, *)
struct TransferHistorySection: View {
  let baseURL: String
  let defaultAccountId: String
  @StateObject private var history = TransferHistoryViewModel()
  @State private var accountId: String = ""

  private func directionLabel(for summary: ToriiExplorerTransferSummary) -> String {
    if summary.isIncoming { return "In" }
    if summary.isOutgoing { return "Out" }
    if summary.isSelfTransfer { return "Self" }
    return "?"
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 6) {
      Text("Transfer History").font(.headline)
      TextField("Account ID", text: $accountId)
        .autocapitalization(.none)
        .disableAutocorrection(true)
        .textFieldStyle(RoundedBorderTextFieldStyle())

      HStack(spacing: 10) {
        Button("Use Verified") { accountId = defaultAccountId }
          .disabled(defaultAccountId.isEmpty)
        Button("Load") { history.load(baseURL: baseURL, accountId: accountId) }
        Button("Clear") { history.clear() }
          .disabled(history.summaries.isEmpty && history.errorMessage == nil)
      }

      if history.isLoading {
        Text("Loading...").font(.footnote).foregroundColor(.secondary)
      }
      if let error = history.errorMessage {
        Text(error).font(.footnote).foregroundColor(.red)
      }

      ForEach(history.summaries.prefix(5)) { summary in
        VStack(alignment: .leading, spacing: 2) {
          Text("\(directionLabel(for: summary)) \(summary.amount) \(summary.assetDefinitionId)")
            .font(.footnote)
          Text("\(summary.senderAccountId) -> \(summary.receiverAccountId)")
            .font(.caption)
            .foregroundColor(.secondary)
          Text(summary.createdAt)
            .font(.caption2)
            .foregroundColor(.secondary)
        }
      }

      if history.summaries.count > 5 {
        Text("Showing first 5 transfers.")
          .font(.footnote)
          .foregroundColor(.secondary)
      }
    }
    .onAppear {
      if accountId.isEmpty {
        accountId = defaultAccountId
      }
    }
  }
}
#endif

struct ContentView_Previews: PreviewProvider {
  static var previews: some View { ContentView() }
}
