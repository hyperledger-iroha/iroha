import XCTest
@testable import NoritoDemoXcode

final class ConnectViewModelTests: XCTestCase {
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

  private var original: [String: String?] = [:]

  override func tearDown() {
    super.tearDown()
    for (key, value) in original {
      if let value {
        setenv(key, value, 1)
      } else {
        unsetenv(key)
      }
    }
    original.removeAll()
  }

  func testEnvironmentDefaults() {
    let overrides: [String: String] = [
      "TORII_NODE_URL": "https://swift-demo.iroha.test",
      "CONNECT_SESSION_ID": "c2Vzc2lvbg==",
      "CONNECT_TOKEN_APP": "app-token",
      "CONNECT_TOKEN_WALLET": "wallet-token",
      "CONNECT_CHAIN_ID": "nexus",
      "CONNECT_ROLE": "wallet",
      "CONNECT_PEER_PUB_B64": "cGVlci1wdWI=",
      "CONNECT_SHARED_KEY_B64": "c2hhcmVkLWtleQ==",
      "CONNECT_APPROVE_ACCOUNT_ID": "sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7",
      "CONNECT_APPROVE_PRIVATE_KEY_B64": "cHJpdmF0ZS1rZXk=",
      "CONNECT_APPROVE_SIGNATURE_B64": "c2lnbmF0dXJl"
    ]

    for key in keys {
      original[key] = ProcessInfo.processInfo.environment[key]
    }

    for (key, value) in overrides {
      setenv(key, value, 1)
    }

    let viewModel = ConnectViewModel()

    XCTAssertEqual(viewModel.baseURL, "https://swift-demo.iroha.test")
    XCTAssertEqual(viewModel.sid, "c2Vzc2lvbg==")
    XCTAssertEqual(viewModel.tokenApp, "app-token")
    XCTAssertEqual(viewModel.tokenWallet, "wallet-token")
    XCTAssertEqual(viewModel.chainId, "nexus")
    XCTAssertEqual(viewModel.role, .wallet)
    XCTAssertEqual(viewModel.peerPubB64, "cGVlci1wdWI=")
    XCTAssertEqual(viewModel.aeadKeyB64, "c2hhcmVkLWtleQ==")
    XCTAssertEqual(
      viewModel.approveAccountId,
      "sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7"
    )
    XCTAssertEqual(viewModel.approvePrivKeyB64, "cHJpdmF0ZS1rZXk=")
    XCTAssertEqual(viewModel.approveSigB64, "c2lnbmF0dXJl")
  }
}
