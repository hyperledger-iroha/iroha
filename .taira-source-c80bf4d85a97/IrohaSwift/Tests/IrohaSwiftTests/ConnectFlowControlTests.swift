import XCTest
@testable import IrohaSwift

final class ConnectFlowControlTests: XCTestCase {
    func testConsumesTokensAndExhausts() throws {
        let controller = ConnectFlowController(window: ConnectFlowControlWindow(appToWallet: 1, walletToApp: 2))

        try controller.consume(direction: .appToWallet)
        XCTAssertThrowsError(try controller.consume(direction: .appToWallet)) { error in
            guard case ConnectFlowController.FlowError.exhausted(let dir) = error else {
                XCTFail("Expected exhausted error")
                return
            }
            XCTAssertEqual(dir, .appToWallet)
        }
    }

    func testGrantAddsTokens() throws {
        let controller = ConnectFlowController(window: ConnectFlowControlWindow(appToWallet: 0, walletToApp: 0))
        controller.grant(direction: .walletToApp, tokens: 2)
        try controller.consume(direction: .walletToApp)
        try controller.consume(direction: .walletToApp)
        XCTAssertThrowsError(try controller.consume(direction: .walletToApp))
    }

    func testGrantClampsOnOverflow() throws {
        let controller = ConnectFlowController(
            window: ConnectFlowControlWindow(appToWallet: UInt64.max - 1, walletToApp: 0)
        )
        controller.grant(direction: .appToWallet, tokens: 10)

        for _ in 0..<9 {
            try controller.consume(direction: .appToWallet)
        }
    }
}
