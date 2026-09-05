import XCTest
@testable import IrohaSwift

final class IrohaPeerNfcV1Tests: XCTestCase {
  func testV1NfcStateMachineCarriesRequestPaymentAcknowledgement() {
    XCTAssertEqual(
      IrohaPeerNfcPhaseV1.allCases,
      [.requestReady, .paymentReceiving, .acknowledgementReady, .complete]
    )
    XCTAssertEqual(IrohaPeerWireKindV1.allCases, [.request, .payment, .acknowledgement])
  }

  func testV1NfcInstructionNumbersAreStable() {
    XCTAssertEqual(IrohaPeerNfcInstructionV1.getInfo.rawValue, 0x10)
    XCTAssertEqual(IrohaPeerNfcInstructionV1.readRequest.rawValue, 0x11)
    XCTAssertEqual(IrohaPeerNfcInstructionV1.commitPayment.rawValue, 0x22)
    XCTAssertEqual(IrohaPeerNfcInstructionV1.confirmAcknowledgement.rawValue, 0x24)
  }
}
