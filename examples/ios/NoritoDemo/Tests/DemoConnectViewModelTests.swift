import XCTest
import Darwin
@testable import NoritoDemo

final class DemoConnectViewModelTests: XCTestCase {
  private var originalEnv: [String: String?] = [:]
  private let keys = [
    "TORII_NODE_URL",
    "CONNECT_SESSION_ID",
    "CONNECT_TOKEN_APP",
    "CONNECT_TOKEN_WALLET",
    "CONNECT_CHAIN_ID",
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
      "CONNECT_SESSION_ID": "c2Vzc2lvbg==",
      "CONNECT_TOKEN_APP": "app-token",
      "CONNECT_TOKEN_WALLET": "wallet-token",
      "CONNECT_CHAIN_ID": "nexus",
      "CONNECT_ROLE": "wallet",
      "CONNECT_PEER_PUB_B64": "cGVlci1wdWI=",
      "CONNECT_SHARED_KEY_B64": "c2hhcmVkLWtleQ==",
      "CONNECT_APPROVE_ACCOUNT_ID": "soraゴヂアネタアニャヴウザニキニョゾビバヤチョトテスシコダオョキュロッベイゴビチャヰショチャパヨツサツホキマイニキ",
      "CONNECT_APPROVE_PRIVATE_KEY_B64": "cHJpdmF0ZS1rZXk=",
      "CONNECT_APPROVE_SIGNATURE_B64": "c2lnbmF0dXJl"
    ]

    for key in keys {
      originalEnv[key] = ProcessInfo.processInfo.environment[key]
    }

    for (key, value) in overrides {
      setenv(key, value, 1)
    }

    let viewModel = DemoConnectViewModel()

    XCTAssertEqual(viewModel.baseURL, "https://unit.test:8443")
    XCTAssertEqual(viewModel.sid, "c2Vzc2lvbg==")
    XCTAssertEqual(viewModel.tokenApp, "app-token")
    XCTAssertEqual(viewModel.tokenWallet, "wallet-token")
    XCTAssertEqual(viewModel.chainId, "nexus")
    XCTAssertEqual(viewModel.role, .wallet)
    XCTAssertEqual(viewModel.peerPubB64, "cGVlci1wdWI=")
    XCTAssertEqual(viewModel.aeadKeyB64, "c2hhcmVkLWtleQ==")
    XCTAssertEqual(
      viewModel.approveAccountId,
      "soraゴヂアネタアニャヴウザニキニョゾビバヤチョトテスシコダオョキュロッベイゴビチャヰショチャパヨツサツホキマイニキ"
    )
    XCTAssertEqual(viewModel.approvePrivKeyB64, "cHJpdmF0ZS1rZXk=")
    XCTAssertEqual(viewModel.approveSigB64, "c2lnbmF0dXJl")
  }

  func testAddressPreviewAvailableWhenIrohaSwiftIsPresent() throws {
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
    throw XCTSkip("IrohaSwift framework is unavailable on this platform")
#endif
  }
}
