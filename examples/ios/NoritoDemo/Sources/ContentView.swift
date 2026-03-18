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
import OSLog
#if canImport(IrohaSwift)
import IrohaSwift
#endif

final class DemoConnectViewModel: ObservableObject {
  enum Role: String, CaseIterable, Identifiable { case app, wallet; var id: String { rawValue } }

  @Published var baseURL: String = "http://localhost:8080"
  @Published var role: Role = .app
  @Published var sid: String = ""
  @Published var tokenApp: String = ""
  @Published var tokenWallet: String = ""
  @Published var wsStatus: String = "Disconnected"
  @Published var logs: [String] = []
  @Published var aeadKeyB64: String = ""
  @Published var localPubB64: String = ""
  @Published var peerPubB64: String = ""
  @Published var sendKeyB64: String = ""
  @Published var recvKeyB64: String = ""
  @Published var saltIsBlake2b: Bool = false
  @Published var lastApproveAccount: String = ""
  @Published var lastApproveSigB64: String = ""
  @Published var lastApproveAccountName: String = ""
  @Published var lastApproveAccountDomain: String = ""
  @Published var approveSigValid: Bool? = nil
  @Published var verifiedAccount: String = ""
  @Published var manifestStatus: PosManifestStatus?
  // Permissions + proof state
  @Published var chainId: String = "testnet"
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
  @Published var handshakeStatus: String = "Idle"
  @Published var lastAppPubB64: String = ""
  @Published var approveAccountId: String = ""
  @Published var approvePrivKeyB64: String = ""
  @Published var approveSigB64: String = ""
  @Published var lastApproveAccount: String = ""
  @Published var lastApproveSigB64: String = ""
  @Published var addressPreview: AddressPreview?

  private var webSocketTask: URLSessionWebSocketTask?
  private let session = URLSession(configuration: .default)
  private var nextSeq: UInt64 = 1
  private let defaultsVerifiedKey = "NoritoDemo.VerifiedAccount"
  #if canImport(CryptoKit)
  private var localPriv: Curve25519.KeyAgreement.PrivateKey?
  private var keySend: SymmetricKey?
  private var keyRecv: SymmetricKey?
  #endif

  var walletDeepLink: String {
    guard !sid.isEmpty, !tokenWallet.isEmpty else { return "" }
    let nodeEnc = baseURL.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? baseURL
    return "iroha://connect?sid=\(sid)&chain_id=\(chainId)&node=\(nodeEnc)&v=1&role=wallet&token=\(tokenWallet)"
  }

  init() {
    addressPreview = DemoConnectViewModel.generateAddressPreview()
    loadVerifiedAccount()
    applyEnvironmentDefaults()
    loadManifestStatus()
  }

  private func loadVerifiedAccount() {
    if let saved = UserDefaults.standard.string(forKey: defaultsVerifiedKey), !saved.isEmpty {
      self.verifiedAccount = saved
    }
  }

  private func persistVerifiedAccount(_ id: String) {
    self.verifiedAccount = id
    UserDefaults.standard.set(id, forKey: defaultsVerifiedKey)
    log("Persisted verified account: \(id)")
  }

  struct AddressPreview: Equatable {
    let i105: String
    let i105Default: String
    let i105Warning: String
    let domain: String
    let implicitDefault: Bool
    let networkPrefix: UInt16
  }

  private static func generateAddressPreview() -> AddressPreview? {
#if canImport(IrohaSwift)
    let sampleDomain = AccountAddress.defaultDomainName
    do {
      let sampleKey = makeSampleKeyMaterial()
      let address = try AccountAddress.fromAccount(publicKey: sampleKey, algorithm: "ed25519")
      let formats = try address.displayFormats()
      return AddressPreview(
        i105: formats.i105,
        i105Default: formats.i105Default,
        i105Warning: formats.i105Warning,
        domain: sampleDomain,
        implicitDefault: sampleDomain == AccountAddress.defaultDomainName,
        networkPrefix: formats.networkPrefix
      )
    } catch {
      return AddressPreview(
        i105: "i105-unavailable",
        i105Default: "sora-unavailable",
        i105Warning: "Address preview unavailable: \(error.localizedDescription)",
        domain: sampleDomain,
        implicitDefault: sampleDomain == AccountAddress.defaultDomainName,
        networkPrefix: 42
      )
    }
#else
    return nil
#endif
  }

  private static func makeSampleKeyMaterial() -> Data {
    var bytes: [UInt8] = []
    for index in 0..<32 {
      bytes.append(UInt8((index * 13) & 0xFF))
    }
    return Data(bytes)
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
    if let sessionId = value(for: "CONNECT_SESSION_ID") {
      sid = sessionId
    }
    if let token = value(for: "CONNECT_TOKEN_APP") {
      tokenApp = token
    }
    if let token = value(for: "CONNECT_TOKEN_WALLET") {
      tokenWallet = token
    }
    if let chain = value(for: "CONNECT_CHAIN_ID") {
      chainId = chain
    }
    if let roleRaw = value(for: "CONNECT_ROLE"), let newRole = Role(rawValue: roleRaw.lowercased()) {
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

  private func loadManifestStatus() {
    do {
      let manifest = try PosManifestLoader.loadManifest()
      manifestStatus = PosManifestStatus.from(manifest: manifest)
    } catch {
      manifestStatus = PosManifestStatus.unavailable(message: "manifest unavailable: \(error.localizedDescription)")
    }
  }

  func createSession() {
#if canImport(CryptoKit)
    if localPriv == nil { generateEphemeral() }
    guard let appPk = localPriv?.publicKey.rawRepresentation else { log("Generate local key first"); return }
#else
    let appPk = Data()
#endif
    let sidBytes = computeSid(chainId: chainId, appPk: appPk)
    let sidB64 = base64url(sidBytes)
    guard let url = URL(string: baseURL + "/v1/connect/session") else { log("Invalid base URL"); return }
    var req = URLRequest(url: url)
    req.httpMethod = "POST"
    req.setValue("application/json", forHTTPHeaderField: "Content-Type")
    req.setValue("application/json", forHTTPHeaderField: "Accept")
    let body: [String: Any] = ["sid": sidB64, "node": baseURL]
    req.httpBody = try? JSONSerialization.data(withJSONObject: body)
    log("POST /v1/connect/session (client sid)…")
    session.dataTask(with: req) { [weak self] data, resp, err in
      guard let self = self else { return }
      if let err = err { self.log("Session error: \(err.localizedDescription)"); return }
      guard let http = resp as? HTTPURLResponse, (200..<300).contains(http.statusCode) else { self.log("HTTP \((resp as? HTTPURLResponse)?.statusCode ?? -1)"); return }
      guard let data = data else { self.log("Empty response"); return }
      do {
        if let json = try JSONSerialization.jsonObject(with: data) as? [String: Any] {
          let sidEcho = (json["sid"] as? String) ?? sidB64
          let tokApp = (json["token_app"] as? String) ?? ""
          let tokWal = (json["token_wallet"] as? String) ?? ""
          DispatchQueue.main.async { self.sid = sidEcho; self.tokenApp = tokApp; self.tokenWallet = tokWal }
          self.log("Session created. sid=\(sidEcho)")
        } else { self.log("Unexpected JSON format") }
      } catch { self.log("Decode error: \(error.localizedDescription)") }
    }.resume()
  }

  func joinWebSocket() {
    guard !sid.isEmpty else { log("No sid — create session first"); return }
    guard var comps = URLComponents(string: baseURL) else { log("Bad base URL"); return }
    comps.path = "/v1/connect/ws"
    let token = (role == .app) ? tokenApp : tokenWallet
    comps.queryItems = [
      URLQueryItem(name: "sid", value: sid),
      URLQueryItem(name: "role", value: role.rawValue),
      URLQueryItem(name: "token", value: token),
    ]
    if token.isEmpty { log("Warning: token is empty; WS may be rejected") }
    if comps.scheme == "http" { comps.scheme = "ws" }
    if comps.scheme == "https" { comps.scheme = "wss" }
    guard let wsURL = comps.url else { log("Bad WS URL"); return }
    log("WS connect → \(wsURL.absoluteString)")
    let task = session.webSocketTask(with: wsURL)
    webSocketTask = task
    task.resume()
    DispatchQueue.main.async { self.wsStatus = "Connected" }
    DispatchQueue.main.async { self.handshakeStatus = "WS connected" }
    receiveLoop()

#if canImport(CryptoKit)
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
    let bridge = NoritoBridgeKit(); let seq = nextSeq; nextSeq &+= 1
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
    let bridge = NoritoBridgeKit(); let seq = nextSeq; nextSeq &+= 1
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
    } catch { log("Frame decode failed: \(error.localizedDescription)") }
  }
  private func aadV1(sid: Data, dir: UInt8, seq: UInt64) -> Data {
    var out = Data(); out.append("connect:v1".data(using: .utf8)!); out.append(sid); out.append(Data([dir]))
    var le = seq.littleEndian; withUnsafeBytes(of: &le) { out.append($0) }
    out.append(Data([1]))
    return out
  }
  private func nonceFromSeq(_ seq: UInt64) -> ChaChaPoly.Nonce {
    var n = Data(count: 12); var le = seq.littleEndian
    n.replaceSubrange(4..<12, with: withUnsafeBytes(of: &le) { Data($0) })
    return try! ChaChaPoly.Nonce(data: n)
  }

  // Control frames via bridge (optional)
  private let ctrlKindOpen: UInt16 = 1
  private let ctrlKindApprove: UInt16 = 2
  func sendControlOpen() {
    guard let task = webSocketTask else { return }
    guard let sk = localPriv else { log("No local key; generate before Open"); return }
    guard let sidData = dataFromBase64OrBase64URL(sid), sidData.count == 32 else { log("sid invalid for Open"); return }
    let bridge = NoritoBridgeKit()
    do {
      if let perms = permsJson(request: true), let f = try? bridge.encodeControlOpenExt(sid: sidData, dir: 0, seq: nextSeq, appPub: Data(sk.publicKey.rawRepresentation), appMetaJson: nil, chainId: chainId, permissionsJson: perms) {
        nextSeq &+= 1
        task.send(.data(f)) { [weak self] err in if let err = err { self?.log("Open send error: \(err.localizedDescription)") } else { self?.log("Sent Open control (ext)"); self?.handshakeStatus = "Open sent" } }
        return
      }
      let frame = try bridge.encodeControlOpen(sid: sidData, dir: 0, seq: nextSeq, appPub: Data(sk.publicKey.rawRepresentation))
      nextSeq &+= 1
      task.send(.data(frame)) { [weak self] err in if let err = err { self?.log("Open send error: \(err.localizedDescription)") } else { self?.log("Sent Open control") } }
    } catch { log("Open encode not available: \(error)") }
  }
  
  
  // Control frames via bridge (optional)
  private let ctrlKindOpen: UInt16 = 1
  private let ctrlKindApprove: UInt16 = 2
  func sendControlOpen() {
    guard let task = webSocketTask else { return }
    guard let sk = localPriv else { log("No local key; generate before Open"); return }
    guard let sidData = dataFromBase64OrBase64URL(sid), sidData.count == 32 else { log("sid invalid for Open"); return }
    let bridge = NoritoBridgeKit()
    do {
      let frame = try bridge.encodeControlOpen(sid: sidData, dir: 0, seq: nextSeq, appPub: Data(sk.publicKey.rawRepresentation))
      nextSeq &+= 1
      task.send(.data(frame)) { [weak self] err in
        if let err = err { self?.log("Open send error: \(err.localizedDescription)") }
        else { self?.log("Sent Open control"); self?.handshakeStatus = "Open sent" }
      }
    } catch { log("Open encode not available: \(error)") }
  }
  func sendControlApprove() {
    guard let task = webSocketTask else { return }
    guard let sk = localPriv else { log("No local key; generate before Approve"); return }
    guard let sidData = dataFromBase64OrBase64URL(sid), sidData.count == 32 else { log("sid invalid for Approve"); return }
    let bridge = NoritoBridgeKit()
    do {
      let acct = Data(approveAccountId.utf8)
      guard let sig = dataFromBase64OrBase64URL(approveSigB64), sig.count == 64 else { log("Approve signature must be 64 bytes (base64)"); return }
      if let f = try? bridge.encodeControlApproveExt(sid: sidData, dir: 1, seq: nextSeq, walletPub: Data(sk.publicKey.rawRepresentation), accountId: approveAccountId, permissionsJson: permsJson(request: false), proofJson: proofJson(), sig: sig) {
        nextSeq &+= 1
        task.send(.data(f)) { [weak self] err in if let err = err { self?.log("Approve send error: \(err.localizedDescription)") } else { self?.log("Sent Approve control (ext)"); self?.handshakeStatus = "Approve sent" } }
        return
      }
      let frame = try bridge.encodeControlApprove(sid: sidData, dir: 1, seq: nextSeq, walletPub: Data(sk.publicKey.rawRepresentation), account: acct, sig: sig)
      nextSeq &+= 1
      task.send(.data(frame)) { [weak self] err in
        if let err = err { self?.log("Approve send error: \(err.localizedDescription)") }
        else { self?.log("Sent Approve control"); self?.handshakeStatus = "Approve sent" }
      }
    } catch { log("Approve encode not available: \(error)") }
  }
  private func tryHandleControl(_ data: Data) {
    let bridge = NoritoBridgeKit()
    do {
      let (sidOut, _, _, kind) = try bridge.decodeControlKind(data)
      guard sidOut == (dataFromBase64OrBase64URL(sid) ?? Data()) else { return }
      switch kind {
      case ctrlKindOpen:
        if role == .wallet {
          let appPk = try bridge.decodeControlOpenPub(data)
          self.lastAppPubB64 = Data(appPk).base64EncodedString()
          self.handshakeStatus = "Open received"
          self.log("Got Open; deriving keys…")
          self.deriveKeysFromPeerPub(appPk)
          if let json = try? bridge.decodeControlOpenPermissionsJson(data) { self.openRequestedPermsJson = json }
        }
      case ctrlKindApprove:
        if role == .app {
          let walletPk = try bridge.decodeControlApprovePub(data)
          self.log("Got Approve; deriving keys…")
          self.deriveKeysFromPeerPub(walletPk)
          if let acct = try? bridge.decodeControlApproveAccount(data) {
            self.lastApproveAccount = String(data: acct, encoding: .utf8) ?? acct.base64EncodedString()
            if let sig = try? bridge.decodeControlApproveSig(data) {
              self.lastApproveSigB64 = sig.base64EncodedString()
              self.verifyApproveSignature(walletPk: walletPk, acct: acct, sig: sig)
            }
          }
          if let json = try? bridge.decodeControlApproveAccountJson(data),
             let obj = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any] {
            if let name = obj["name"] as? String { self.lastApproveAccountName = name }
            let dom = (obj["domain"] as? String) ?? (obj["domain_id"] as? String) ?? (obj["domainId"] as? String)
            if let dom { self.lastApproveAccountDomain = dom }
          }
          if let pjson = try? bridge.decodeControlApprovePermissionsJson(data) { self.approvePermsJson = pjson }
          if let prjson = try? bridge.decodeControlApproveProofJson(data) { self.approveProofJson = prjson }
          if self.lastApproveAccountName.isEmpty && self.lastApproveAccountDomain.isEmpty {
            let raw = self.lastApproveAccount
            if let at = raw.firstIndex(of: "@") {
              self.lastApproveAccountName = String(raw[..<at])
              self.lastApproveAccountDomain = String(raw[raw.index(after: at)...])
            }
          }
          self.handshakeStatus = "Approve received"
        }
      default:
        break
      }
    } catch {
      // Not a control frame or helpers unavailable
    }
  }
#endif

  private func receiveLoop() {
    webSocketTask?.receive { [weak self] result in
      guard let self = self else { return }
      switch result {
      case .success(let msg):
        switch msg {
        case .string(let s): self.log("WS text (\(s.count))")
        case .data(let d):
          self.log("WS binary (\(d.count) bytes)")
#if canImport(NoritoBridge)
          self.tryDecodeIncoming(d)
          self.tryHandleControl(d)
#endif
        @unknown default: self.log("WS unknown message")
        }
        self.receiveLoop()
      case .failure(let err):
        self.log("WS recv error: \(err.localizedDescription)")
        DispatchQueue.main.async { self.wsStatus = "Disconnected" }
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
    let sk = Curve25519.KeyAgreement.PrivateKey()
    localPriv = sk
    localPubB64 = Data(sk.publicKey.rawRepresentation).base64EncodedString()
    log("Generated X25519 keypair; pub exported (base64)")
  }
  func deriveKeys() {
    guard let sidRaw = dataFromBase64OrBase64URL(sid), sidRaw.count == 32 else { log("sid must be base64/base64url (32 bytes)"); return }
    guard let sk = localPriv else { log("Generate local key first"); return }
    guard let peerRaw = dataFromBase64OrBase64URL(peerPubB64), peerRaw.count == 32 else { log("Peer pub must be base64/base64url (32 bytes)"); return }
    guard let peer = try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: peerRaw) else { log("Invalid peer public key"); return }
    do {
      let shared = try sk.sharedSecretFromKeyAgreement(with: peer)
      // Prefer BLAKE2b-256("iroha-connect|salt|" || sid) via NoritoBridge FFI; fallback to SHA256 if unavailable.
      let salt = computeSalt(sid: sidRaw)
      let infoApp = Data("iroha-connect|k_app".utf8)
      let infoWallet = Data("iroha-connect|k_wallet".utf8)
      let kApp = shared.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: infoApp, outputByteCount: 32)
      let kWallet = shared.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: infoWallet, outputByteCount: 32)
      if role == .app { keySend = kApp; keyRecv = kWallet } else { keySend = kWallet; keyRecv = kApp }
      sendKeyB64 = exportKeyB64(keySend)
      recvKeyB64 = exportKeyB64(keyRecv)
      log("Derived direction keys via HKDF-SHA256 (salt=blake2b256 or sha256 fallback)")
      DispatchQueue.main.async { self.handshakeStatus = "Keys ready (manual)" }
    } catch {
      log("Key agreement failed: \(error.localizedDescription)")
    }
  }
  private func deriveKeysFromPeerPub(_ peerRaw: Data) {
    guard let sidRaw = dataFromBase64OrBase64URL(sid), sidRaw.count == 32 else { log("sid must be 32 bytes"); return }
    guard let sk = localPriv else { log("Generate local key first"); return }
    guard let peer = try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: peerRaw) else { log("Invalid peer public key"); return }
    do {
      let shared = try sk.sharedSecretFromKeyAgreement(with: peer)
      let salt = computeSalt(sid: sidRaw)
      let infoApp = Data("iroha-connect|k_app".utf8)
      let infoWallet = Data("iroha-connect|k_wallet".utf8)
      let kApp = shared.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: infoApp, outputByteCount: 32)
      let kWallet = shared.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: infoWallet, outputByteCount: 32)
      if role == .app { keySend = kApp; keyRecv = kWallet } else { keySend = kWallet; keyRecv = kApp }
      sendKeyB64 = exportKeyB64(keySend)
      recvKeyB64 = exportKeyB64(keyRecv)
      log("Derived keys from peer pubkey")
      DispatchQueue.main.async { self.handshakeStatus = "Keys ready" }
    } catch { log("Key agreement failed: \(error.localizedDescription)") }
  }

  private func verifyApproveSignature(walletPk: Data, acct: Data, sig: Data) {
    guard let sidRaw = dataFromBase64OrBase64URL(sid), sidRaw.count == 32 else { log("sid invalid for verify"); self.approveSigValid = false; return }
    guard let appPk = localPriv?.publicKey.rawRepresentation else { log("no app ephemeral for verify"); self.approveSigValid = false; return }
    var msg = Data("iroha-connect|approve|".utf8)
    msg.append(sidRaw); msg.append(appPk); msg.append(walletPk); msg.append(acct)
    do {
      let pub = try Curve25519.Signing.PublicKey(rawRepresentation: walletPk)
      let ok = pub.isValidSignature(Data(base64Encoded: self.lastApproveSigB64) ?? sig, for: msg)
      DispatchQueue.main.async { self.approveSigValid = ok }
      log(ok ? "Approve signature valid" : "Approve signature INVALID")
      if ok {
        let acctId = String(data: acct, encoding: .utf8) ?? acct.base64EncodedString()
        DispatchQueue.main.async { self.persistVerifiedAccount(acctId) }
      }
    } catch {
      DispatchQueue.main.async { self.approveSigValid = false }
      log("Verify error: \(error.localizedDescription)")
    }
  }

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
    if proofDomain.isEmpty && proofUri.isEmpty && proofStatement.isEmpty && proofNonce.isEmpty { return nil }
    let issuedAt = ISO8601DateFormatter().string(from: Date())
    let obj: [String: Any] = ["domain": proofDomain, "uri": proofUri, "statement": proofStatement, "issued_at": issuedAt, "nonce": proofNonce]
    return try? JSONSerialization.data(withJSONObject: obj)
  }
  private func deriveKeysFromPeerPub(_ peerRaw: Data) {
    guard let sidRaw = dataFromBase64OrBase64URL(sid), sidRaw.count == 32 else { log("sid must be 32 bytes"); return }
    guard let sk = localPriv else { log("Generate local key first"); return }
    guard let peer = try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: peerRaw) else { log("Invalid peer public key"); return }
    do {
      let shared = try sk.sharedSecretFromKeyAgreement(with: peer)
      let salt = computeSalt(sid: sidRaw)
      let infoApp = Data("iroha-connect|k_app".utf8)
      let infoWallet = Data("iroha-connect|k_wallet".utf8)
      let kApp = shared.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: infoApp, outputByteCount: 32)
      let kWallet = shared.hkdfDerivedSymmetricKey(using: SHA256.self, salt: salt, sharedInfo: infoWallet, outputByteCount: 32)
      if role == .app { keySend = kApp; keyRecv = kWallet } else { keySend = kWallet; keyRecv = kApp }
      sendKeyB64 = exportKeyB64(keySend)
      recvKeyB64 = exportKeyB64(keyRecv)
      log("Derived keys from peer pubkey")
    } catch { log("Key agreement failed: \(error.localizedDescription)") }
  }
  private func exportKeyB64(_ key: SymmetricKey?) -> String {
    guard let key else { return "" }
    return key.withUnsafeBytes { Data($0).base64EncodedString() }
  }
  #endif

  // MARK: - Helpers
  func dataFromBase64OrBase64URL(_ s: String) -> Data? {
    if let d = Data(base64Encoded: s, options: [.ignoreUnknownCharacters]) { return d }
    var t = s.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
    let rem = t.count % 4
    if rem != 0 { t.append(String(repeating: "=", count: 4-rem)) }
    return Data(base64Encoded: t, options: [.ignoreUnknownCharacters])
  }

  // Compute salt = BLAKE2b-256("iroha-connect|salt|" || sid) via NoritoBridge FFI when available; fallback to SHA256 of same input.
  private func computeSalt(sid: Data) -> Data {
    var input = Data("iroha-connect|salt|".utf8); input.append(sid)
#if canImport(NoritoBridge)
    typealias BlakeFn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>) -> Int32
    if let handle = UnsafeMutableRawPointer(bitPattern: -2), // RTLD_DEFAULT
       let sym = dlsym(handle, "connect_norito_blake2b_256") {
      let fn = unsafeBitCast(sym, to: BlakeFn.self)
      var out = Data(count: 32)
      let rc = input.withUnsafeBytes { ip in
        out.withUnsafeMutableBytes { op in
          fn(ip.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(input.count), op.bindMemory(to: UInt8.self).baseAddress!)
        }
      }
      if rc == 0 { DispatchQueue.main.async { self.saltIsBlake2b = true }; return out }
      log("blake2b_256 FFI error rc=\(rc); falling back to SHA256")
    } else {
      log("blake2b_256 FFI not found; falling back to SHA256")
    }
#endif
    DispatchQueue.main.async { self.saltIsBlake2b = false }
    return Data(SHA256.hash(data: input))
  }

  // Compute sid = BLAKE2b-256("iroha-connect|sid|" || chain_id || app_pk || nonce16) via NoritoBridge FFI when available; fallback to SHA256 of same input.
  private func computeSid(chainId: String, appPk: Data) -> Data {
    var input = Data("iroha-connect|sid|".utf8)
    input.append(Data(chainId.utf8))
    input.append(appPk)
    var nonce = Data(count: 16); _ = nonce.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 16, $0.baseAddress!) }
    input.append(nonce)
#if canImport(NoritoBridge)
    typealias BlakeFn = @convention(c) (UnsafePointer<UInt8>, CUnsignedLong, UnsafeMutablePointer<UInt8>) -> Int32
    if let handle = UnsafeMutableRawPointer(bitPattern: -2),
       let sym = dlsym(handle, "connect_norito_blake2b_256") {
      let fn = unsafeBitCast(sym, to: BlakeFn.self)
      var out = Data(count: 32)
      let rc = input.withUnsafeBytes { ip in
        out.withUnsafeMutableBytes { op in
          fn(ip.bindMemory(to: UInt8.self).baseAddress!, CUnsignedLong(input.count), op.bindMemory(to: UInt8.self).baseAddress!)
        }
      }
      if rc == 0 { return out }
      log("blake2b_256 FFI error rc=\(rc); falling back to SHA256")
    }
#endif
    return Data(SHA256.hash(data: input))
  }

  private func base64url(_ data: Data) -> String {
    let b64 = data.base64EncodedString()
    return b64.replacingOccurrences(of: "+", with: "-")
             .replacingOccurrences(of: "/", with: "_")
             .replacingOccurrences(of: "=", with: "")
  }

  func signApprove() {
    guard role == .wallet else { log("Sign Approve used in wallet role"); return }
    guard let sidRaw = dataFromBase64OrBase64URL(sid), sidRaw.count == 32 else { log("sid invalid (need 32 bytes)"); return }
    guard let appPk = dataFromBase64OrBase64URL(lastAppPubB64), appPk.count == 32 else { log("Missing/invalid app pub (32 bytes)"); return }
    guard let sk = localPriv else { log("Generate local key first"); return }
    let walletPk = Data(sk.publicKey.rawRepresentation)
    let acct = Data(approveAccountId.utf8)
    var msg = Data("iroha-connect|approve|".utf8)
    msg.append(sidRaw); msg.append(appPk); msg.append(walletPk); msg.append(acct)
    guard let privRaw = dataFromBase64OrBase64URL(approvePrivKeyB64), privRaw.count == 32 else { log("Paste 32-byte Ed25519 private key (base64)"); return }
    do {
      let priv = try Curve25519.Signing.PrivateKey(rawRepresentation: privRaw)
      let sig = try priv.signature(for: msg)
      approveSigB64 = sig.base64EncodedString()
      log("Approve signature generated")
    } catch { log("Sign error: \(error.localizedDescription)") }
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

#if canImport(IrohaSwift)
struct AddressPreviewCard: View {
  let address: DemoConnectViewModel.AddressPreview
  @State private var copyStatus: CopyStatus?
  private let telemetry = AddressCopyTelemetry.shared

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text("Account Address Preview").font(.headline)
      Text(domainNote)
        .font(.caption)
        .foregroundColor(.secondary)

      VStack(alignment: .leading, spacing: 4) {
        Text("I105 (prefix \(address.networkPrefix))").font(.subheadline)
        ScrollView(.horizontal) {
          Text(address.i105)
            .font(.system(.body, design: .monospaced))
            .textSelection(.enabled)
        }
      }

      VStack(alignment: .leading, spacing: 4) {
        Text("i105-default Sora-only").font(.subheadline)
        ScrollView(.horizontal) {
          Text(address.i105Default)
            .font(.system(.body, design: .monospaced))
            .textSelection(.enabled)
        }
        Text(address.i105Warning)
          .font(.caption)
          .foregroundColor(.orange)
      }

      HStack(spacing: 12) {
        Button("Copy I105") {
          copy(value: address.i105, label: "I105", warning: nil, mode: .i105)
        }
        .help("Copy the canonical I105 string; safe for QR codes and shared screens.")
        .accessibilityLabel("Copy I105 address")
        .accessibilityHint("Copies the canonical I105 payload to the clipboard and records telemetry.")
        .accessibilityIdentifier("address-copy-i105")

        Button("Copy i105-default") {
          copy(
            value: address.i105Default,
            label: "i105_default",
            warning: address.i105Warning,
            mode: .i105Default
          )
        }
        .help("i105-default addresses are Sora-only; remind recipients before sharing.")
        .accessibilityLabel("Copy i105-default Sora-only address")
        .accessibilityHint("Copies the i105-default Sora-only literal and announces the inline warning.")
        .accessibilityIdentifier("address-copy-i105-default")
      }

      QRCodeView(text: address.i105)
        .frame(width: 160, height: 160)
        .accessibilityLabel("I105 QR for \(address.i105)")
        .accessibilityHint("Share this QR with explorers or wallets; payload always encodes the I105 form.")
        .accessibilityIdentifier("address-qr-preview")

      if let status = copyStatus {
        Text(status.message)
          .font(.caption)
          .foregroundColor(.secondary)
        if let warning = status.warning {
          Text(warning)
            .font(.caption)
            .foregroundColor(.orange)
        }
      }
    }
    .padding()
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(
      RoundedRectangle(cornerRadius: 12)
        .fill(Color(UIColor.secondarySystemBackground))
    )
  }

  private var domainNote: String {
    if address.implicitDefault {
      return "Implicit default domain: \(address.domain) (no suffix required when sharing I105)."
    }
    return "Domain selector: \(address.domain)"
  }

  private func copy(value: String, label: String, warning: String?, mode: AddressCopyMode) {
    UIPasteboard.general.string = value
    let message = "Copied \(label) address"
    copyStatus = CopyStatus(message: message, warning: warning)
    telemetry.record(mode: mode)
    if UIAccessibility.isVoiceOverRunning {
      UIAccessibility.post(notification: .announcement, argument: message)
      if let warning {
        UIAccessibility.post(notification: .announcement, argument: warning)
      }
    }
  }

  private struct CopyStatus {
    let message: String
    let warning: String?
  }
}

struct ManifestStatusCard: View {
  let status: PosManifestStatus

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text("Provisioning Manifest (OA12)")
        .font(.headline)
      Text("Manifest \(status.manifestId) · seq \(status.sequence)")
        .font(.subheadline)
      Text("Operator: \(status.operatorId)").font(.footnote)
      Text("Valid: \(status.validWindowLabel)").font(.footnote)
      Text("Rotation hint: \(status.rotationLabel)").font(.footnote)
      Text(status.dualStatusHealthy ? "Dual-signature OK — \(status.dualStatusLabel)" : "Dual-signature warning — \(status.dualStatusLabel)")
        .font(.footnote)
        .foregroundColor(status.dualStatusHealthy ? .green : .red)
      VStack(alignment: .leading, spacing: 2) {
        if status.warnings.isEmpty {
          Text("Warnings: none").font(.footnote)
        } else {
          Text("Warnings:").font(.footnote)
          ForEach(Array(status.warnings.enumerated()), id: \.offset) { entry in
            Text("• \(entry.element)")
              .font(.footnote)
          }
        }
      }
      Divider()
      VStack(alignment: .leading, spacing: 4) {
        Text("Backend roots").font(.subheadline)
        if status.backendRoots.isEmpty {
          Text("• none").font(.footnote)
        } else {
          ForEach(status.backendRoots) { root in
            Text("• \(root.label) (\(root.role)) — \(root.statusLabel)")
              .font(.footnote)
          }
        }
      }
    }
    .padding()
    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.2)))
  }
}
#endif

struct ContentView: View {
  @StateObject private var vm = DemoConnectViewModel()
  @State private var showShareSheet = false
  @State private var shareItems: [Any] = []

  var body: some View {
    VStack(spacing: 12) {
      Text("Norito Demo").font(.title)
      Text(status).font(.subheadline).foregroundColor(.secondary)

      TextField("Node URL (http://host:port)", text: $vm.baseURL)
        .autocapitalization(.none)
        .disableAutocorrection(true)
        .textFieldStyle(RoundedBorderTextFieldStyle())

      HStack {
        Text("Role:")
        Picker("Role", selection: $vm.role) {
          ForEach(DemoConnectViewModel.Role.allCases) { r in Text(r.rawValue).tag(r) }
        }.pickerStyle(SegmentedPickerStyle())
      }

      HStack(spacing: 10) {
        Button("Create Session") { vm.createSession() }
        Button("Join WS") { vm.joinWebSocket() }.disabled(vm.sid.isEmpty)
        Button("Disconnect") { vm.disconnect() }
      }

#if canImport(IrohaSwift)
      if let preview = vm.addressPreview {
        Divider()
        AddressPreviewCard(address: preview)
}

      if let manifest = vm.manifestStatus {
        Divider()
        ManifestStatusCard(status: manifest)
      }
#endif

enum AddressCopyMode: String {
  case i105 = "i105"
  case i105Default = "i105_default"
}

final class AddressCopyTelemetry {
  static let shared = AddressCopyTelemetry()
  private let logger = Logger(subsystem: "org.hyperledger.iroha.norito-demo", category: "address-copy")

  func record(mode: AddressCopyMode) {
    logger.info("address_copy_mode=\(mode.rawValue, privacy: .public)")
  }
}

      if !vm.tokenApp.isEmpty || !vm.tokenWallet.isEmpty {
        VStack(alignment: .leading, spacing: 6) {
          Text("Tokens").font(.headline)
          HStack(alignment: .top, spacing: 8) {
            Text("App").font(.footnote).foregroundColor(.secondary).frame(width: 36, alignment: .leading)
            ScrollView(.horizontal) { Text(vm.tokenApp.isEmpty ? "–" : vm.tokenApp).font(.system(.caption, design: .monospaced)) }
            Button("Copy") { UIPasteboard.general.string = vm.tokenApp }.disabled(vm.tokenApp.isEmpty)
          }
          HStack(alignment: .top, spacing: 8) {
            Text("Wallet").font(.footnote).foregroundColor(.secondary).frame(width: 36, alignment: .leading)
            ScrollView(.horizontal) { Text(vm.tokenWallet.isEmpty ? "–" : vm.tokenWallet).font(.system(.caption, design: .monospaced)) }
            Button("Copy") { UIPasteboard.general.string = vm.tokenWallet }.disabled(vm.tokenWallet.isEmpty)
          }
        }
      }

      if !vm.walletDeepLink.isEmpty {
        Divider()
        Text("Wallet Deep Link QR").font(.headline)
        QRCodeView(text: vm.walletDeepLink)
          .frame(width: 180, height: 180)
        ScrollView(.horizontal) {
          Text(vm.walletDeepLink)
            .font(.system(.caption, design: .monospaced))
            .textSelection(.enabled)
        }
        HStack(spacing: 12) {
          Button("Copy Deeplink") { UIPasteboard.general.string = vm.walletDeepLink }
          Button("Share Deeplink") {
            shareItems = [vm.walletDeepLink]
            showShareSheet = true
          }
        }
      }

      Text("sid: \(vm.sid.isEmpty ? "–" : vm.sid)").font(.footnote).foregroundColor(.secondary)
      Text("WS: \(vm.wsStatus)").font(.footnote)
        .foregroundColor(vm.wsStatus == "Connected" ? .green : .secondary)
      Text("Handshake: \(vm.handshakeStatus)").font(.footnote)
        .foregroundColor(vm.handshakeStatus.contains("Keys ready") ? .green : (vm.handshakeStatus.contains("Open") || vm.handshakeStatus.contains("Approve") ? .blue : .secondary))
      if !vm.verifiedAccount.isEmpty {
        Text("Verified: \(vm.verifiedAccount)").font(.footnote).foregroundColor(.green)
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

      Divider()
      Text("Logs").font(.headline)
      #if canImport(CryptoKit)
      Text("Key Derivation").font(.headline)
      HStack(spacing: 10) {
        Button("Generate Ephemeral Key") { vm.generateEphemeral() }
        Button("Derive Keys") { vm.deriveKeys() }
      }
      Text("Salt: \(vm.saltIsBlake2b ? "BLAKE2b-256" : "SHA-256 (fallback)")").font(.footnote)
        .foregroundColor(vm.saltIsBlake2b ? .green : .secondary)
      Text("Local X25519 Pub (base64)").font(.footnote)
      ScrollView(.horizontal) { Text(vm.localPubB64).font(.system(.caption, design: .monospaced)) }
      TextField("Peer X25519 Pub (base64)", text: $vm.peerPubB64)
        .autocapitalization(.none)
        .disableAutocorrection(true)
        .textFieldStyle(RoundedBorderTextFieldStyle())
      HStack { Text("Send key:").font(.footnote); ScrollView(.horizontal) { Text(vm.sendKeyB64).font(.system(.caption, design: .monospaced)) } }
      HStack { Text("Recv key:").font(.footnote); ScrollView(.horizontal) { Text(vm.recvKeyB64).font(.system(.caption, design: .monospaced)) } }
      #endif
      #if canImport(NoritoBridge)
      Divider()
      Text("NoritoBridge linked: encrypted send/receive").font(.footnote).foregroundColor(.green)
      TextField("Chain ID", text: $vm.chainId).autocapitalization(.none).disableAutocorrection(true).textFieldStyle(RoundedBorderTextFieldStyle())
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
    .sheet(isPresented: $showShareSheet) { ActivityView(activityItems: shareItems) }
  }
}
      #else
      Text("NoritoBridge not linked — Norito framing disabled").font(.footnote).foregroundColor(.secondary)
      #endif
      ScrollView {
        VStack(alignment: .leading, spacing: 6) {
          ForEach(Array(vm.logs.enumerated()), id: \.offset) { _, line in
            Text(line).font(.system(.caption, design: .monospaced)).frame(maxWidth: .infinity, alignment: .leading)
          }
        }
      }
    }.padding()
  }

  var status: String {
    #if canImport(NoritoBridge)
    return "NoritoBridge: available"
    #else
    return "NoritoBridge: not linked"
    #endif
  }
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
                                                        page: 1,
                                                        perPage: 25)
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

struct ActivityView: UIViewControllerRepresentable {
  let activityItems: [Any]
  func makeUIViewController(context: Context) -> UIActivityViewController {
    UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
  }
  func updateUIViewController(_ uiViewController: UIActivityViewController, context: Context) {}
}

struct ContentView_Previews: PreviewProvider {
  static var previews: some View { ContentView() }
}
