import XCTest
import Security
import CryptoKit
import IrohaSwift
import Darwin
@testable import NoritoDemo

final class DemoConnectViewModelTests: XCTestCase {
  private var originalEnv: [String: String?] = [:]
  private let keys = [
    "TORII_NODE_URL",
    "CONNECT_TOKEN_APP",
    "CONNECT_TOKEN_WALLET",
    "CONNECT_TOKEN_RELAY",
    "CONNECT_NETWORK_ID",
    "CONNECT_ROLE",
    "CONNECT_PEER_PUB_B64",
    "CONNECT_SHARED_KEY_B64",
    "CONNECT_APPROVE_ACCOUNT_ID",
    "CONNECT_APPROVE_PRIVATE_KEY_B64",
    "CONNECT_APPROVE_SIGNATURE_B64"
  ]

  override func tearDown() {
    for (key, value) in originalEnv {
      if let value {
        setenv(key, value, 1)
      } else {
        unsetenv(key)
      }
    }
    originalEnv.removeAll()
    super.tearDown()
  }

  func testEnvironmentOverridesAppliedOnInit() {
    let overrides: [String: String] = [
      "TORII_NODE_URL": "https://unit.test:8443",
      "CONNECT_TOKEN_APP": "app-token",
      "CONNECT_TOKEN_WALLET": "wallet-token",
      "CONNECT_TOKEN_RELAY": "relay-token",
      "CONNECT_NETWORK_ID": "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
      "CONNECT_ROLE": "wallet",
      "CONNECT_PEER_PUB_B64": "cGVlci1wdWI=",
      "CONNECT_SHARED_KEY_B64": "c2hhcmVkLWtleQ==",
      "CONNECT_APPROVE_ACCOUNT_ID": "sorauﾛ1QG1ｼﾀ3vN7ﾋzﾄﾍcﾐLKDCAｲ5ｸｴjﾔﾘ2uﾄﾕmｷﾕﾙeJBJW7X2N7",
      "CONNECT_APPROVE_PRIVATE_KEY_B64": "QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkI=",
      "CONNECT_APPROVE_SIGNATURE_B64": "JCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJA=="
    ]

    for key in keys {
      originalEnv[key] = ProcessInfo.processInfo.environment[key]
    }

    for (key, value) in overrides {
      setenv(key, value, 1)
    }

    let viewModel = DemoConnectViewModel()

    XCTAssertEqual(viewModel.baseURL, "https://unit.test:8443")
    XCTAssertTrue(viewModel.sid.isEmpty)
    XCTAssertEqual(viewModel.tokenApp, "app-token")
    XCTAssertEqual(viewModel.tokenWallet, "wallet-token")
    XCTAssertEqual(viewModel.tokenRelay, "relay-token")
    XCTAssertEqual(viewModel.networkId, "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")
    XCTAssertEqual(viewModel.role, .wallet)
    XCTAssertEqual(viewModel.peerPubB64, "cGVlci1wdWI=")
    XCTAssertEqual(viewModel.aeadKeyB64, "c2hhcmVkLWtleQ==")
    XCTAssertEqual(
      viewModel.approveAccountId,
      "sorauﾛ1QG1ｼﾀ3vN7ﾋzﾄﾍcﾐLKDCAｲ5ｸｴjﾔﾘ2uﾄﾕmｷﾕﾙeJBJW7X2N7"
    )
    XCTAssertEqual(viewModel.approvePrivKeyB64, "QkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkI=")
    XCTAssertEqual(viewModel.approveSigB64, "JCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJA==")
    XCTAssertTrue(viewModel.walletDeepLink.isEmpty, "configuration without a freshly derived app key and nonce must fail closed")
  }

  func testAddressPreviewAvailableWhenIrohaSwiftIsPresent() {
#if canImport(IrohaSwift)
    let viewModel = DemoConnectViewModel()
    guard let preview = viewModel.addressPreview else {
      XCTFail("Expected address preview to be generated")
      return
    }
    XCTAssertFalse(preview.i105.isEmpty)
    XCTAssertFalse(preview.i105.contains("@"))
    XCTAssertTrue(preview.i105Warning.lowercased().contains("i105"))
#else
    XCTFail("IrohaSwift framework is required for the address preview release test")
#endif
  }

  func testHistoryRejectsInvalidInputsBeforeStartingARequest() {
    let history = TransferHistoryViewModel()
    history.load(baseURL: "https://unit.test", accountId: "  ")
    XCTAssertNotNil(history.errorMessage)
    XCTAssertFalse(history.isLoading)
    XCTAssertTrue(history.summaries.isEmpty)
    history.clear()
    history.load(baseURL: "http://[", accountId: "sorauﾛ1QG1ｼﾀ3vN7ﾋzﾄﾍcﾐLKDCAｲ5ｸｴjﾔﾘ2uﾄﾕmｷﾕﾙeJBJW7X2N7")
    XCTAssertNotNil(history.errorMessage)
    XCTAssertFalse(history.isLoading)
    XCTAssertTrue(history.summaries.isEmpty)
  }

  func testCurrentBridgeOpenBindsExactLaunchInputs() throws {
    let network = try NetworkId(literal: "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")
    let appKey = Data(repeating: 0x31, count: 32)
    let nonce = Data(repeating: 0x42, count: 16)
    let sid = try ConnectCrypto.deriveSessionID(networkID: network, appPublicKey: appKey, nonce: nonce)
    let bridge = NoritoBridgeKit()
    let frame = try bridge.encodeControlOpenExt(
      sid: sid, dir: 0, seq: 1, appPub: appKey, nonce: nonce,
      appMetaJson: nil, networkId: network.bytes, permissionsJson: nil
    )
    let kind = try bridge.decodeControlKind(frame)
    XCTAssertEqual(kind.sid, sid)
    XCTAssertEqual(kind.dir, 0)
    XCTAssertEqual(kind.seq, 1)
    XCTAssertEqual(kind.kind, 1)
    XCTAssertEqual(try bridge.decodeControlOpenPub(frame), appKey)

    var otherNetwork = network.bytes
    otherNetwork[0] ^= 1
    var otherNonce = nonce
    otherNonce[0] ^= 1
    for (candidateNetwork, candidateNonce, direction, sequence) in [
      (otherNetwork, nonce, UInt8(0), UInt64(1)),
      (network.bytes, otherNonce, UInt8(0), UInt64(1)),
      (network.bytes, Data(nonce.dropLast()), UInt8(0), UInt64(1)),
      (network.bytes, nonce, UInt8(1), UInt64(1)),
      (network.bytes, nonce, UInt8(0), UInt64(2)),
    ] {
      XCTAssertThrowsError(try bridge.encodeControlOpenExt(
        sid: sid, dir: direction, seq: sequence, appPub: appKey,
        nonce: candidateNonce, appMetaJson: nil, networkId: candidateNetwork,
        permissionsJson: nil
      ))
    }
  }

  func testPartialEntropyFailureCannotPublishSession() {
    var fills = 0
    let viewModel = DemoConnectViewModel(randomBytes: { buffer in
      XCTAssertEqual(buffer.count, 16)
      fills += 1
      for index in 0..<8 { buffer[index] = 0x5a }
      return errSecParam
    })
    viewModel.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    viewModel.baseURL = "http://["
    viewModel.sid = "existing-session"
    viewModel.tokenApp = "existing-app"
    viewModel.tokenWallet = "existing-wallet"
    viewModel.tokenRelay = "existing-relay"
    viewModel.lastAppPubB64 = "existing-public-key"
    viewModel.createSession()
    let drained = expectation(description: "entropy failure log")
    DispatchQueue.main.async { drained.fulfill() }
    wait(for: [drained], timeout: 2)
    XCTAssertEqual(fills, 1)
    XCTAssertEqual(viewModel.sid, "existing-session")
    XCTAssertEqual(viewModel.tokenApp, "existing-app")
    XCTAssertEqual(viewModel.tokenWallet, "existing-wallet")
    XCTAssertEqual(viewModel.tokenRelay, "existing-relay")
    XCTAssertEqual(viewModel.lastAppPubB64, "existing-public-key")
    XCTAssertTrue(viewModel.walletDeepLink.isEmpty)
    XCTAssertTrue(viewModel.logs.contains { $0.contains("Secure nonce generation failed") })
    XCTAssertFalse(viewModel.logs.contains { $0.contains("POST /v1/connect/session") })
  }

  func testSuccessfulEntropyPreservesExactNonceBeforeNetworkSubmission() throws {
    var fills = 0
    let viewModel = DemoConnectViewModel(randomBytes: { buffer in
      XCTAssertEqual(buffer.count, 16)
      fills += 1
      for index in buffer.indices { buffer[index] = 0x5a }
      return errSecSuccess
    })
    viewModel.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    viewModel.baseURL = "http://["
    viewModel.sid = "existing-session"
    viewModel.tokenWallet = "existing-wallet"
    viewModel.tokenRelay = "existing-relay"
    viewModel.createSession()
    let drained = expectation(description: "successful entropy reaches URL validation")
    DispatchQueue.main.async { drained.fulfill() }
    wait(for: [drained], timeout: 2)
    XCTAssertEqual(fills, 1)
    XCTAssertEqual(viewModel.lastAppPubB64, viewModel.localPubB64)
    XCTAssertFalse(viewModel.lastAppPubB64.isEmpty)
    let components = try XCTUnwrap(URLComponents(string: viewModel.walletDeepLink))
    XCTAssertEqual(components.queryItems?.first { $0.name == "nonce" }?.value, "WlpaWlpaWlpaWlpaWlpaWg")
    XCTAssertTrue(viewModel.logs.contains { $0.contains("Invalid base URL") })
    XCTAssertFalse(viewModel.logs.contains { $0.contains("Secure nonce generation failed") })
    XCTAssertFalse(viewModel.logs.contains { $0.contains("POST /v1/connect/session") })
  }

  func testSuccessfulAllZeroEntropyStillRefusesNonce() {
    var fills = 0
    let viewModel = DemoConnectViewModel(randomBytes: { buffer in
      XCTAssertEqual(buffer.count, 16)
      fills += 1
      for index in buffer.indices { buffer[index] = 0 }
      return errSecSuccess
    })
    viewModel.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    viewModel.baseURL = "http://["
    viewModel.lastAppPubB64 = "existing-public-key"
    viewModel.createSession()
    let drained = expectation(description: "zero nonce refusal")
    DispatchQueue.main.async { drained.fulfill() }
    wait(for: [drained], timeout: 2)
    XCTAssertEqual(fills, 1)
    XCTAssertEqual(viewModel.lastAppPubB64, "existing-public-key")
    XCTAssertTrue(viewModel.walletDeepLink.isEmpty)
    XCTAssertTrue(viewModel.logs.contains { $0.contains("Failed to derive an exact Connect SID") })
    XCTAssertFalse(viewModel.logs.contains { $0.contains("POST /v1/connect/session") })
  }

  func testCanonicalDirectionKeysAgreeAndRejectChangedLowOrderPeer() throws {
    let app = DemoConnectViewModel()
    let wallet = DemoConnectViewModel()
    app.role = .app
    wallet.role = .wallet
    let sessionID = Data(repeating: 0x39, count: 32).base64EncodedString()
    app.sid = sessionID
    wallet.sid = sessionID
    app.generateEphemeral()
    wallet.generateEphemeral()
    app.peerPubB64 = wallet.localPubB64
    wallet.peerPubB64 = app.localPubB64
    app.deriveKeys()
    wallet.deriveKeys()
    XCTAssertEqual(try XCTUnwrap(Data(base64Encoded: app.sendKeyB64)).count, 32)
    XCTAssertEqual(try XCTUnwrap(Data(base64Encoded: app.recvKeyB64)).count, 32)
    XCTAssertEqual(app.sendKeyB64, wallet.recvKeyB64)
    XCTAssertEqual(app.recvKeyB64, wallet.sendKeyB64)
    XCTAssertNotEqual(app.sendKeyB64, app.recvKeyB64)
    XCTAssertTrue(app.saltIsBlake2b)
    XCTAssertTrue(wallet.saltIsBlake2b)
    let manualKey = Data(repeating: 0x55, count: 32).base64EncodedString()
    app.aeadKeyB64 = manualKey
    for rejected in [Data(repeating: 0, count: 32), Data(repeating: 1, count: 31)] {
      app.peerPubB64 = wallet.localPubB64
      app.deriveKeys()
      XCTAssertFalse(app.sendKeyB64.isEmpty)
      app.peerPubB64 = rejected.base64EncodedString()
      app.deriveKeys()
      let drained = expectation(description: "failed derivation status")
      DispatchQueue.main.async { drained.fulfill() }
      wait(for: [drained], timeout: 2)
      XCTAssertTrue(app.sendKeyB64.isEmpty)
      XCTAssertTrue(app.recvKeyB64.isEmpty)
      XCTAssertFalse(app.saltIsBlake2b)
      XCTAssertEqual(app.handshakeStatus, "Keys unavailable")
      XCTAssertEqual(app.aeadKeyB64, manualKey, "explicit manual key configuration is independent")
    }
    XCTAssertTrue(app.logs.contains { $0.contains("Key agreement failed") })
  }

  private func approvalFixture() throws -> (DemoConnectViewModel, ConnectFrame, Data) {
    let vm = DemoConnectViewModel(randomBytes: { buffer in
      for index in buffer.indices { buffer[index] = 0x6a }
      return errSecSuccess
    })
    vm.role = .app
    vm.baseURL = "http://["
    vm.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    vm.tokenRelay = "approval-test-relay"
    vm.createSession()
    let appPublicKey = try XCTUnwrap(Data(base64Encoded: vm.localPubB64))
    let network = try NetworkId(literal: vm.networkId)
    let sid = try ConnectCrypto.deriveSessionID(networkID: network,
      appPublicKey: appPublicKey, nonce: Data(repeating: 0x6a, count: 16))
    vm.sid = sid.base64EncodedString()
    let open = try ConnectCodec.decode(vm.prepareControlOpenFrame())
    XCTAssertEqual(open.sessionID, sid)
    XCTAssertEqual(open.sequence, 1)
    XCTAssertEqual(open.direction, .appToWallet)
    guard case .control(.open(let original)) = open.kind else {
      throw ConnectSessionError.protocolViolation("Expected actual Open")
    }
    XCTAssertEqual(original.appPublicKey, appPublicKey)
    XCTAssertEqual(original.constraints.networkID, network)
    let wallet = Curve25519.KeyAgreement.PrivateKey()
    let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
    let account = try AccountId.makeI105(publicKey: signingKey.publicKey())
    let relay = try ConnectCrypto.relayAuthHash(sessionID: sid, relayToken: vm.tokenRelay)
    let preimage = try ConnectCrypto.buildApprovalPreimage(networkID: network,
      sessionID: sid, appPublicKey: appPublicKey, walletPublicKey: wallet.publicKey.rawRepresentation,
      accountID: account, permissions: nil, proof: nil, relayAuthHash: relay)
    let approval = ConnectApprove(walletPublicKey: wallet.publicKey.rawRepresentation,
      accountID: account, walletSignature: ConnectWalletSignature(algorithm: "ed25519",
      signature: try signingKey.sign(preimage)))
    let frame = ConnectFrame(sessionID: sid, direction: .walletToApp, sequence: 1,
      kind: .control(.approve(approval)))
    let walletKeys = try ConnectCrypto.deriveDirectionKeys(localPrivateKey: wallet.rawRepresentation,
      peerPublicKey: appPublicKey, sessionID: sid)
    return (vm, frame, walletKeys.appToWallet)
  }

  func testApprovalAuthenticatesBeforePublishingKeysAndRefusesAfterSuccess() throws {
    let defaultsKey = "NoritoDemo.VerifiedAccount"
    let saved = UserDefaults.standard.object(forKey: defaultsKey)
    defer { UserDefaults.standard.set(saved, forKey: defaultsKey) }
    for attack in ["forged", "replay", "wrong-session", "wrong-direction", "wrong-sequence", "missing-signature", "malformed"] {
      let (vm, frame, expectedSendKey) = try approvalFixture()
      let valid = try ConnectCodec.encode(frame)
      vm.handleIncomingFrame(valid)
      XCTAssertEqual(vm.approveSigValid, true, attack)
      XCTAssertEqual(Data(base64Encoded: vm.sendKeyB64), expectedSendKey, attack)
      XCTAssertFalse(vm.recvKeyB64.isEmpty, attack)
      XCTAssertTrue(vm.saltIsBlake2b, attack)
      XCTAssertEqual(vm.handshakeStatus, "Approved; keys ready", attack)
      XCTAssertFalse(vm.verifiedAccount.isEmpty, attack)
      let manual = Data(repeating: 0x77, count: 32).base64EncodedString()
      vm.aeadKeyB64 = manual
      var changed = frame
      let invalid: Data
      switch attack {
      case "forged":
        guard case .control(.approve(var approval)) = changed.kind else { return XCTFail("approve") }
        let signer = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        approval.walletSignature = ConnectWalletSignature(algorithm: "ed25519", signature: try signer.sign(Data("different approval".utf8)))
        changed.kind = .control(.approve(approval))
        invalid = try ConnectCodec.encode(changed)
      case "wrong-session":
        changed.sessionID[0] ^= 1
        invalid = try ConnectCodec.encode(changed)
      case "wrong-direction":
        changed.direction = .appToWallet
        invalid = try ConnectCodec.encode(changed)
      case "wrong-sequence":
        changed.sequence = 2
        invalid = try ConnectCodec.encode(changed)
      case "missing-signature": invalid = Data(valid.dropLast(64))
      case "malformed": invalid = Data([0xff, 0, 1])
      default: invalid = valid
      }
      vm.handleIncomingFrame(invalid)
      let drained = expectation(description: attack)
      DispatchQueue.main.async { drained.fulfill() }
      wait(for: [drained], timeout: 2)
      XCTAssertEqual(vm.approveSigValid, false, attack)
      XCTAssertTrue(vm.sendKeyB64.isEmpty, attack)
      XCTAssertTrue(vm.recvKeyB64.isEmpty, attack)
      XCTAssertFalse(vm.saltIsBlake2b, attack)
      XCTAssertEqual(vm.handshakeStatus, "Keys unavailable", attack)
      XCTAssertTrue(vm.verifiedAccount.isEmpty, attack)
      XCTAssertTrue(vm.lastApproveAccount.isEmpty, attack)
      XCTAssertEqual(vm.aeadKeyB64, manual, "manual configuration remains explicit")
      vm.handleIncomingFrame(valid)
      XCTAssertTrue(vm.sendKeyB64.isEmpty, "rejection must not reopen the approval: \(attack)")
    }
  }

  func testInitialForgedApprovalAndChangedBindingNeverInstallKeys() throws {
    let defaultsKey = "NoritoDemo.VerifiedAccount"
    let saved = UserDefaults.standard.object(forKey: defaultsKey)
    defer { UserDefaults.standard.set(saved, forKey: defaultsKey) }
    for attack in ["signature", "relay", "network", "sid", "key"] {
      let (vm, original, _) = try approvalFixture()
      var frame = original
      switch attack {
      case "signature":
        guard case .control(.approve(var approval)) = frame.kind else { return XCTFail("approve") }
        let signer = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        approval.walletSignature = ConnectWalletSignature(algorithm: "ed25519", signature: try signer.sign(Data("forged first approval".utf8)))
        frame.kind = .control(.approve(approval))
      case "relay": vm.tokenRelay = "another-relay"
      case "network": vm.networkId = "invalid-network"
      case "sid": vm.sid = Data(repeating: 0x23, count: 32).base64EncodedString()
      default: vm.generateEphemeral()
      }
      vm.handleIncomingFrame(try ConnectCodec.encode(frame))
      XCTAssertEqual(vm.approveSigValid, false, attack)
      XCTAssertTrue(vm.sendKeyB64.isEmpty, attack)
      XCTAssertTrue(vm.recvKeyB64.isEmpty, attack)
      XCTAssertFalse(vm.saltIsBlake2b, attack)
      XCTAssertEqual(vm.handshakeStatus, "Keys unavailable", attack)
      XCTAssertTrue(vm.verifiedAccount.isEmpty, attack)
    }
  }

  func testNewSessionClearsOldKeysAndReopensExactSequence() throws {
    var nonceByte: UInt8 = 0x10
    let vm = DemoConnectViewModel(randomBytes: { buffer in
      for index in buffer.indices { buffer[index] = nonceByte }
      nonceByte += 1
      return errSecSuccess
    })
    vm.role = .app
    vm.baseURL = "http://["
    vm.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    vm.tokenRelay = "test-relay"
    vm.createSession()
    let network = try NetworkId(literal: vm.networkId)
    let pub = try XCTUnwrap(Data(base64Encoded: vm.localPubB64))
    let firstSID = try ConnectCrypto.deriveSessionID(networkID: network,
      appPublicKey: pub, nonce: Data(repeating: 0x10, count: 16))
    vm.sid = firstSID.base64EncodedString()
    XCTAssertEqual(try ConnectCodec.decode(vm.prepareControlOpenFrame()).sequence, 1)
    vm.peerPubB64 = Curve25519.KeyAgreement.PrivateKey().publicKey.rawRepresentation.base64EncodedString()
    vm.deriveKeys()
    XCTAssertFalse(vm.sendKeyB64.isEmpty)
    let manual = Data(repeating: 0x71, count: 32).base64EncodedString()
    vm.aeadKeyB64 = manual
    vm.createSession()
    XCTAssertTrue(vm.sendKeyB64.isEmpty)
    XCTAssertTrue(vm.recvKeyB64.isEmpty)
    XCTAssertFalse(vm.saltIsBlake2b)
    XCTAssertNil(vm.approveSigValid)
    XCTAssertEqual(vm.aeadKeyB64, manual)
    let nextSID = try ConnectCrypto.deriveSessionID(networkID: network,
      appPublicKey: pub, nonce: Data(repeating: 0x11, count: 16))
    XCTAssertNotEqual(firstSID, nextSID)
    vm.sid = nextSID.base64EncodedString()
    let nextOpen = try ConnectCodec.decode(vm.prepareControlOpenFrame())
    XCTAssertEqual(nextOpen.sequence, 1)
    XCTAssertEqual(nextOpen.sessionID, nextSID)
  }

  func testWalletOpenRequiresOriginalNonceAndRefusesReplacement() throws {
    let (app, uri, original) = try launchFixture()
    let wallet = try freshWallet(for: app, uri: uri)
    let appPub = try XCTUnwrap(Data(base64Encoded: app.localPubB64))
    let approval = try ConnectCodec.decode(original)
    wallet.handleIncomingFrame(original)
    XCTAssertFalse(wallet.sendKeyB64.isEmpty)
    XCTAssertFalse(wallet.recvKeyB64.isEmpty)
    XCTAssertEqual(wallet.handshakeStatus, "Open accepted; keys ready")
    XCTAssertEqual(wallet.lastAppPubB64, appPub.base64EncodedString())
    wallet.handleIncomingFrame(original)
    XCTAssertTrue(wallet.sendKeyB64.isEmpty)
    XCTAssertTrue(wallet.recvKeyB64.isEmpty)
    XCTAssertEqual(wallet.approveSigValid, false)
    for hasForeignNonce in [false, true] {
      let other = DemoConnectViewModel(randomBytes: { buffer in
        for index in buffer.indices { buffer[index] = 0x6b }
        return errSecSuccess
      })
      other.networkId = wallet.networkId
      other.baseURL = "http://["
      if hasForeignNonce { other.createSession() } else { other.generateEphemeral() }
      other.role = .wallet
      other.sid = approval.sessionID.base64EncodedString()
      other.handleIncomingFrame(original)
      XCTAssertTrue(other.sendKeyB64.isEmpty)
      XCTAssertTrue(other.recvKeyB64.isEmpty)
      XCTAssertEqual(other.approveSigValid, false)
    }
  }


  func testBase64AndBase64URLPreserveEverySessionByteAndRejectIgnoredInput() {
    let vm = DemoConnectViewModel()
    for byte in UInt8.min...UInt8.max {
      let bytes = Data(repeating: byte, count: 32)
      let standard = bytes.base64EncodedString()
      let url = launchBase64URL(bytes)
      XCTAssertEqual(vm.dataFromBase64OrBase64URL(standard), bytes)
      XCTAssertEqual(vm.dataFromBase64OrBase64URL(url), bytes)
      for malformed in [standard + "!", " " + standard, standard + "\n", url + "!", url + "="] {
        // A padded spelling is valid only when it is exactly standard Base64.
        if malformed != standard {
          XCTAssertNil(vm.dataFromBase64OrBase64URL(malformed))
        }
      }
    }
    for noncanonical in ["AB==", "AB", "A", "-+__", "_/++"] {
      XCTAssertNil(vm.dataFromBase64OrBase64URL(noncanonical))
    }
  }

  private func launchBase64URL(_ bytes: Data) -> String {
    bytes.base64EncodedString().replacingOccurrences(of: "+", with: "-")
      .replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
  }

  private func launchFixture() throws -> (DemoConnectViewModel, String, Data) {
    let app = DemoConnectViewModel(randomBytes: { buffer in
      for index in buffer.indices { buffer[index] = 0x6a }
      return errSecSuccess
    })
    app.role = .app
    app.baseURL = "http://[" // Retain real key/nonce setup without starting a network request.
    app.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    app.createSession()
    let publicKey = try XCTUnwrap(Data(base64Encoded: app.localPubB64))
    let sid = try ConnectCrypto.deriveSessionID(networkID: NetworkId(literal: app.networkId),
      appPublicKey: publicKey, nonce: Data(repeating: 0x6a, count: 16))
    app.sid = launchBase64URL(sid)
    app.baseURL = "https://demo.iroha.test"
    app.tokenApp = launchBase64URL(Data(repeating: 0x41, count: 32))
    app.tokenWallet = launchBase64URL(Data(repeating: 0x52, count: 32))
    app.tokenRelay = launchBase64URL(Data(repeating: 0x63, count: 32))
    let uri = app.walletDeepLink
    XCTAssertFalse(uri.isEmpty)
    return (app, uri, try app.prepareControlOpenFrame())
  }

  private func freshWallet(for app: DemoConnectViewModel, uri: String) throws -> DemoConnectViewModel {
    let wallet = DemoConnectViewModel()
    wallet.baseURL = app.baseURL
    wallet.networkId = app.networkId
    XCTAssertTrue(wallet.localPubB64.isEmpty)
    XCTAssertTrue(wallet.sid.isEmpty)
    XCTAssertTrue(wallet.importWalletLaunch(uri))
    XCTAssertEqual(wallet.role, .wallet)
    XCTAssertEqual(wallet.sid, app.sid)
    XCTAssertTrue(wallet.tokenWallet.isEmpty, "the SDK request owns launch credentials")
    XCTAssertTrue(wallet.tokenRelay.isEmpty)
    return wallet
  }

  func testFreshWalletLaunchCompletesCanonicalApprovalAndDirectionKeys() throws {
    let defaultsKey = "NoritoDemo.VerifiedAccount"
    let saved = UserDefaults.standard.object(forKey: defaultsKey)
    defer { UserDefaults.standard.set(saved, forKey: defaultsKey) }
    let (app, uri, open) = try launchFixture()
    let wallet = try freshWallet(for: app, uri: uri)
    let request = try wallet.prepareWebSocketRequest()
    XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer " + app.tokenWallet)
    XCTAssertFalse(try XCTUnwrap(request.url).absoluteString.contains(app.tokenWallet))
    XCTAssertThrowsError(try wallet.prepareControlApproveFrame(), "Open and signing are mandatory")
    wallet.handleIncomingFrame(open)
    XCTAssertEqual(wallet.handshakeStatus, "Open accepted; keys ready")
    let signer = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
    wallet.approveAccountId = try AccountId.makeI105(publicKey: signer.publicKey())
    wallet.approvePrivKeyB64 = Data(repeating: 0x42, count: 32).base64EncodedString()
    wallet.reqPermSignRaw = true
    wallet.reqPermSignTx = false
    wallet.reqEventDisplay = true
    wallet.signApprove()
    XCTAssertFalse(wallet.approveSigB64.isEmpty)
    let approval = try wallet.prepareControlApproveFrame()
    let frame = try ConnectCodec.decode(approval)
    XCTAssertEqual(frame.direction, .walletToApp)
    XCTAssertEqual(frame.sequence, 1)
    XCTAssertEqual(frame.sessionID, try ConnectCodec.decode(open).sessionID)
    guard case .control(.approve(let value)) = frame.kind else { return XCTFail("Approve") }
    XCTAssertEqual(value.permissions?.methods, ["SIGN_REQUEST_RAW"])
    XCTAssertEqual(value.permissions?.events, ["DISPLAY_REQUEST"])
    app.handleIncomingFrame(approval)
    XCTAssertEqual(app.approveSigValid, true)
    XCTAssertEqual(app.verifiedAccount, wallet.approveAccountId)
    XCTAssertEqual(app.sendKeyB64, wallet.recvKeyB64)
    XCTAssertEqual(app.recvKeyB64, wallet.sendKeyB64)
    XCTAssertFalse(app.sendKeyB64.isEmpty)
    XCTAssertNotEqual(app.sendKeyB64, app.recvKeyB64)
    XCTAssertThrowsError(try wallet.prepareControlApproveFrame(), "approval is prepared once")
    let drained = expectation(description: "launch and approval logs")
    DispatchQueue.main.async { drained.fulfill() }
    wait(for: [drained], timeout: 2)
    for secret in [uri, app.tokenApp, app.tokenWallet, app.tokenRelay, wallet.approvePrivKeyB64] {
      XCTAssertFalse(wallet.logs.contains { $0.contains(secret) })
    }
  }

  func testWalletIngressRefusesMalformedReplacementAndChangedIdentity() throws {
    let (app, uri, open) = try launchFixture()
    for malformed in [uri + "&future=x", uri + "&token=x", "iroha://connect?token=private-launch-secret"] {
      let wallet = DemoConnectViewModel()
      wallet.baseURL = app.baseURL
      wallet.networkId = app.networkId
      XCTAssertFalse(wallet.importWalletLaunch(malformed))
      XCTAssertTrue(wallet.localPubB64.isEmpty)
      XCTAssertTrue(wallet.sid.isEmpty)
      XCTAssertTrue(wallet.sendKeyB64.isEmpty)
      XCTAssertThrowsError(try wallet.prepareControlApproveFrame())
    }
    for changed in ["network", "node", "sid", "key"] {
      let wallet = try freshWallet(for: app, uri: uri)
      switch changed {
      case "network": wallet.networkId = "invalid-network"
      case "node": wallet.baseURL = "https://different.iroha.test"
      case "sid": wallet.sid = launchBase64URL(Data(repeating: 0x34, count: 32))
      default: wallet.generateEphemeral()
      }
      wallet.handleIncomingFrame(open)
      XCTAssertTrue(wallet.sendKeyB64.isEmpty, changed)
      XCTAssertTrue(wallet.recvKeyB64.isEmpty, changed)
      XCTAssertEqual(wallet.approveSigValid, false, changed)
      XCTAssertThrowsError(try wallet.prepareControlApproveFrame())
    }
    let wallet = try freshWallet(for: app, uri: uri)
    wallet.handleIncomingFrame(open)
    XCTAssertFalse(wallet.sendKeyB64.isEmpty)
    XCTAssertFalse(wallet.importWalletLaunch(uri), "an imported request cannot be replaced")
    XCTAssertTrue(wallet.sendKeyB64.isEmpty)
    XCTAssertTrue(wallet.recvKeyB64.isEmpty)
    XCTAssertThrowsError(try wallet.prepareControlApproveFrame())
  }

  func testRetiredSocketCallbackCannotConsumeNewWalletLaunch() throws {
    let (app, uri, open) = try launchFixture()
    let wallet = try freshWallet(for: app, uri: uri)
    let retired = wallet.makeIncomingFrameConsumer()
    wallet.disconnect()
    XCTAssertTrue(wallet.importWalletLaunch(uri))
    XCTAssertFalse(retired(open), "the true receive publication seam pins session generation")
    XCTAssertTrue(wallet.sendKeyB64.isEmpty)
    XCTAssertNil(wallet.approveSigValid)
    XCTAssertTrue(wallet.makeIncomingFrameConsumer()(open))
    XCTAssertFalse(wallet.sendKeyB64.isEmpty)
    XCTAssertEqual(wallet.handshakeStatus, "Open accepted; keys ready")
  }

  func testWebSocketPreparationKeepsCredentialsOutOfURLAndLogs() throws {
    let vm = DemoConnectViewModel()
    vm.baseURL = "https://demo.iroha.test"
    vm.sid = Data(repeating: 0x21, count: 32).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    vm.tokenApp = Data(repeating: 0xa5, count: 32).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    vm.tokenWallet = Data(repeating: 0xb6, count: 32).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    vm.tokenRelay = Data(repeating: 0xc7, count: 32).base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    vm.approvePrivKeyB64 = Data(repeating: 0xd8, count: 32).base64EncodedString()
    for role in [DemoConnectViewModel.Role.app, .wallet] {
      vm.role = role
      let request = try vm.prepareWebSocketRequest()
      XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"),
        "Bearer " + (role == .app ? vm.tokenApp : vm.tokenWallet))
      let url = try XCTUnwrap(request.url)
      let query = try XCTUnwrap(URLComponents(url: url, resolvingAgainstBaseURL: false)).queryItems ?? []
      XCTAssertEqual(Set(query.map(\.name)), Set(["sid", "role"]))
      for secret in [vm.tokenApp, vm.tokenWallet, vm.tokenRelay, vm.approvePrivKeyB64] {
        XCTAssertFalse(url.absoluteString.contains(secret))
      }
    }
    let drained = expectation(description: "request preparation logs")
    DispatchQueue.main.async { drained.fulfill() }
    wait(for: [drained], timeout: 2)
    XCTAssertTrue(vm.logs.contains { $0.contains("WS connect requested") })
    for secret in [vm.tokenApp, vm.tokenWallet, vm.tokenRelay, vm.approvePrivKeyB64] {
      XCTAssertFalse(vm.logs.contains { $0.contains(secret) })
    }
  }

  func testSessionResponsePublicationRechecksOriginalNetworkAndLaunch() throws {
    var nonceByte: UInt8 = 0x31
    let vm = DemoConnectViewModel(randomBytes: { buffer in
      for index in buffer.indices { buffer[index] = nonceByte }
      nonceByte += 1
      return errSecSuccess
    })
    vm.baseURL = "http://["
    vm.networkId = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    vm.createSession()
    let network = try NetworkId(literal: vm.networkId)
    let pub = try XCTUnwrap(Data(base64Encoded: vm.localPubB64))
    let nonce = Data(repeating: 0x31, count: 16)
    let expectedSID = try ConnectCrypto.deriveSessionID(networkID: network, appPublicKey: pub, nonce: nonce)
      .base64EncodedString().replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_").replacingOccurrences(of: "=", with: "")
    vm.sid = "prior-session"
    vm.tokenApp = "prior-app"
    vm.tokenWallet = "prior-wallet"
    vm.tokenRelay = "prior-relay"
    func publish() -> Bool {
      vm.publishSessionResponse(network: network.literal, appPublicKey: pub, nonce: nonce,
        sessionID: expectedSID, appToken: "new-app", walletToken: "new-wallet", relayToken: "new-relay")
    }
    vm.networkId = try NetworkId(bytes: IrohaHash.hash(Data("foreign demo callback network".utf8))).literal
    XCTAssertFalse(publish(), "a main-queue network change must not publish the old response")
    XCTAssertEqual(vm.sid, "prior-session")
    XCTAssertEqual(vm.tokenApp, "prior-app")
    XCTAssertEqual(vm.tokenWallet, "prior-wallet")
    XCTAssertEqual(vm.tokenRelay, "prior-relay")
    vm.networkId = network.literal
    XCTAssertTrue(publish())
    XCTAssertEqual(vm.sid, expectedSID)
    XCTAssertEqual(vm.tokenApp, "new-app")
    vm.createSession()
    vm.sid = "newer-session-pending"
    XCTAssertFalse(publish(), "a later sampled nonce must supersede the old callback")
    XCTAssertEqual(vm.sid, "newer-session-pending")
    XCTAssertEqual(vm.tokenApp, "new-app")
  }

  func testEnvelopeTextFieldsPreserveExactUTF8() throws {
    let bridge = NoritoBridgeKit()
    for text in ["ascii", "日本語é", "", "embedded\0nul"] {
      let frames: [(Data, UInt16, [String], [String: String])] = [
        (try bridge.encodeEnvelopeSignRequestRaw(seq: 7, domainTag: text, bytes: Data([0, 255, 4])), 5, ["SignRequestRaw"], ["domain_tag": text, "bytes_b64": "AP8E"]),
        (try bridge.encodeEnvelopeSignResultErr(seq: 7, code: text, message: text), 6, ["SignResultErr"], ["code": text, "message": text]),
        (try bridge.encodeEnvelopeClose(seq: 7, who: 1, code: 19, reason: text, retryable: true), 2, ["Control", "Close"], ["reason": text, "who": "Wallet"]),
        (try bridge.encodeEnvelopeReject(seq: 7, code: 23, codeId: text, reason: text), 3, ["Control", "Reject"], ["code_id": text, "reason": text]),
      ]
      for (frame, expectedKind, path, fields) in frames {
        let kind = try bridge.decodeEnvelopeKind(frame)
        XCTAssertEqual(kind.seq, 7)
        XCTAssertEqual(kind.kind, expectedKind)
        let json = try JSONSerialization.jsonObject(with: Data(bridge.decodeEnvelopeJson(frame).utf8))
        let object = try XCTUnwrap(json as? [String: Any])
        var payload = try XCTUnwrap(object["payload"] as? [String: Any])
        for key in path { payload = try XCTUnwrap(payload[key] as? [String: Any]) }
        for (key, value) in fields { XCTAssertEqual(payload[key] as? String, value) }
        if expectedKind == 2 {
          XCTAssertEqual(payload["code"] as? Int, 19)
          XCTAssertEqual(payload["retryable"] as? Bool, true)
        } else if expectedKind == 3 {
          XCTAssertEqual(payload["code"] as? Int, 23)
        }
      }
    }
  }

}
