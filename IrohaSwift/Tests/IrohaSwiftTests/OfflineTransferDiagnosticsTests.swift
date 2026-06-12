import XCTest
@testable import IrohaSwift

final class OfflineTransferDiagnosticsTests: XCTestCase {
    func testPrecheckDiagnosticsExposeOnlyPresenceAndRelationships() {
        let diagnostics = IrohaOfflineTransferDiagnosticPolicy.precheckDiagnostics(
            amount: "125.5000",
            balance: "1000",
            locked: "25",
            spendable: "975",
            maxTx: "500",
            maxBalance: "1000"
        )

        XCTAssertEqual(diagnostics.amountPresent, "present")
        XCTAssertEqual(diagnostics.balancePresent, "present")
        XCTAssertEqual(diagnostics.lockedPresent, "present")
        XCTAssertEqual(diagnostics.spendablePresent, "present")
        XCTAssertEqual(diagnostics.maxTxPresent, "present")
        XCTAssertEqual(diagnostics.maxBalancePresent, "present")
        XCTAssertEqual(diagnostics.amountToSpendable, .lessThan)
        XCTAssertEqual(diagnostics.amountToMaxTx, .lessThan)
        XCTAssertEqual(diagnostics.balanceToMaxBalance, .equal)

        let renderedFields = diagnostics.logFields
            .sorted { $0.key < $1.key }
            .map { "\($0.key)=\($0.value)" }
            .joined(separator: " ")
        for sensitiveValue in ["125.5000", "125.5", "1000", "975", "500", "25"] {
            XCTAssertFalse(renderedFields.contains(sensitiveValue), sensitiveValue)
        }
    }

    func testAmountDiagnosticsClassifyMissingInvalidAndOrderOnly() {
        XCTAssertEqual(IrohaOfflineTransferDiagnosticPolicy.presence(" \n\t "), "missing")
        XCTAssertEqual(IrohaOfflineTransferDiagnosticPolicy.presence("0"), "present")

        XCTAssertEqual(
            IrohaOfflineTransferDiagnosticPolicy.compareAmountsForDiagnostics("9", to: "10"),
            .lessThan
        )
        XCTAssertEqual(
            IrohaOfflineTransferDiagnosticPolicy.compareAmountsForDiagnostics("10.0", to: "10"),
            .equal
        )
        XCTAssertEqual(
            IrohaOfflineTransferDiagnosticPolicy.compareAmountsForDiagnostics("11", to: "10"),
            .greaterThan
        )
        XCTAssertEqual(
            IrohaOfflineTransferDiagnosticPolicy.compareAmountsForDiagnostics(nil, to: "10"),
            .missing
        )
        XCTAssertEqual(
            IrohaOfflineTransferDiagnosticPolicy.compareAmountsForDiagnostics("not-money", to: "10"),
            .invalid
        )
        XCTAssertEqual(
            IrohaOfflineTransferDiagnosticPolicy.compareAmountsForDiagnostics("-1", to: "10"),
            .invalid
        )
    }

    func testRedactedIdentifierHashesRawValuesWithStableEmptySentinel() {
        let rawIdentifier = "bokolo-sensitive-peer-name#12345"
        let redacted = IrohaOfflineTransferDiagnosticPolicy.redactedIdentifier(rawIdentifier)

        XCTAssertEqual(redacted.count, 12)
        XCTAssertEqual(redacted, IrohaOfflineTransferDiagnosticPolicy.redactedIdentifier(rawIdentifier))
        XCTAssertFalse(redacted.contains("bokolo"))
        XCTAssertFalse(redacted.contains("sensitive"))
        XCTAssertFalse(redacted.contains("12345"))
        XCTAssertEqual(IrohaOfflineTransferDiagnosticPolicy.redactedIdentifier(" \n\t "), "empty")
        XCTAssertEqual(IrohaOfflineTransferDiagnosticPolicy.redactedIdentifier(rawIdentifier, prefixLength: 4).count, 4)
    }
}
