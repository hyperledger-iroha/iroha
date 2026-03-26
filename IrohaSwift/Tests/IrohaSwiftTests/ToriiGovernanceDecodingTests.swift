import XCTest
@testable import IrohaSwift

final class ToriiGovernanceDecodingTests: XCTestCase {
    func testGovernanceLockRecordAcceptsStringAmount() throws {
        let json = """
        {"owner":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","amount":"123","expiry_height":10,"direction":1,"duration_blocks":5}
        """.data(using: .utf8)!
        let record = try JSONDecoder().decode(ToriiGovernanceLockRecord.self, from: json)
        XCTAssertEqual(record.amount, "123")
    }

    func testGovernanceLockRecordRejectsFloatAmount() {
        let json = """
        {"owner":"sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB","amount":1.5,"expiry_height":10,"direction":1,"duration_blocks":5}
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiGovernanceLockRecord.self, from: json))
    }

    func testGovernanceTallyRejectsFloatFields() {
        let json = """
        {"referendum_id":"ref-1","approve":1.5,"reject":"2","abstain":"3"}
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiGovernanceTallyResponse.self, from: json))
    }

    func testGovernanceProposalKindRejectsMultipleKeys() {
        let json = """
        {
            "DeployContract": {
                "namespace": "apps",
                "contract_id": "demo",
                "code_hash_hex": "01",
                "abi_hash_hex": "02",
                "abi_version": "1"
            },
            "Extra": {
                "foo": 1
            }
        }
        """.data(using: .utf8)!

        XCTAssertThrowsError(try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: json))
    }
}
