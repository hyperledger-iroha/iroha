import XCTest
@testable import IrohaSwift

final class ToriiGovernanceDecodingTests: XCTestCase {
    private static let governanceOwner =
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    private static let canonicalCustodyJSON = """
    {"escrowed":true,"asset_definition_id":"5dHF5UNffENuEg9mhjYwY1jcZ1K5","bond_escrow_account":"bond-escrow-account","slash_receiver_account":"slash-receiver-account"}
    """

    private func governanceLockJSON(
        amountJSON: String = "\"1\"",
        slashedJSON: String = "\"0\"",
        custodyJSON: String? = "null"
    ) -> Data {
        let custodyField = custodyJSON.map { ",\"custody\":\($0)" } ?? ""
        return Data(
            """
            {"owner":"\(Self.governanceOwner)","amount":\(amountJSON),"slashed":\(slashedJSON),"expiry_height":10,"direction":1,"duration_blocks":5\(custodyField)}
            """.utf8
        )
    }

    func testGovernanceLockRecordAcceptsCanonicalFractionAboveUInt64() throws {
        let json = governanceLockJSON(
            amountJSON: "\"18446744073709551616.25\"",
            slashedJSON: "\"0.25\"",
            custodyJSON: Self.canonicalCustodyJSON
        )
        let record = try JSONDecoder().decode(ToriiGovernanceLockRecord.self, from: json)
        XCTAssertEqual(record.amount, "18446744073709551616.25")
        XCTAssertEqual(record.slashed, "0.25")
        XCTAssertEqual(record.custody?.escrowed, true)
        XCTAssertEqual(record.custody?.assetDefinitionId, "5dHF5UNffENuEg9mhjYwY1jcZ1K5")
        XCTAssertEqual(record.custody?.bondEscrowAccount, "bond-escrow-account")
        XCTAssertEqual(record.custody?.slashReceiverAccount, "slash-receiver-account")
    }

    func testGovernanceLockRecordRejectsNumericJSONAmount() {
        for amount in ["1", "1.5", "-1"] {
            let json = governanceLockJSON(amountJSON: amount)

            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceLockRecord.self, from: json),
                "numeric JSON amount \(amount) must be rejected"
            )
        }
    }

    func testGovernanceLockRecordRejectsNoncanonicalQuantityStrings() {
        let overflowing = String(repeating: "9", count: 155)
        for amount in ["+1", "01", "1.0", "1.2300", " 1", "1 ", "-1", overflowing] {
            let json = governanceLockJSON(amountJSON: "\"\(amount)\"")

            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceLockRecord.self, from: json),
                "noncanonical amount \(amount) must be rejected"
            )
        }
    }

    func testGovernanceLockRecordRejectsNoncanonicalSlashedQuantity() {
        let overflowing = String(repeating: "9", count: 155)
        for encoded in [
            "1",
            "\"+1\"",
            "\"01\"",
            "\"1.0\"",
            "\" 1\"",
            "\"-1\"",
            "\"\(overflowing)\"",
        ] {
            let json = governanceLockJSON(slashedJSON: encoded)

            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceLockRecord.self, from: json)
            )
        }
    }

    func testGovernanceLockRecordAcceptsExplicitNullLegacyCustody() throws {
        let record = try JSONDecoder().decode(
            ToriiGovernanceLockRecord.self,
            from: governanceLockJSON(custodyJSON: "null")
        )
        XCTAssertNil(record.custody)
    }

    func testGovernanceLockRecordRejectsMissingCustody() {
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceLockRecord.self,
                from: governanceLockJSON(custodyJSON: nil)
            )
        )
    }

    func testGovernanceLockRecordRejectsMissingOrExtraCustodyFields() {
        for custodyJSON in [
            """
            {"escrowed":true,"asset_definition_id":"asset","bond_escrow_account":"escrow"}
            """,
            """
            {"escrowed":true,"asset_definition_id":"asset","bond_escrow_account":"escrow","slash_receiver_account":"slash","asset_id":"retired"}
            """,
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceLockRecord.self,
                    from: governanceLockJSON(custodyJSON: custodyJSON)
                )
            )
        }
    }

    func testGovernanceLockRecordRejectsWrongCustodyFieldTypes() {
        for custodyJSON in [
            """
            {"escrowed":"true","asset_definition_id":"asset","bond_escrow_account":"escrow","slash_receiver_account":"slash"}
            """,
            """
            {"escrowed":true,"asset_definition_id":1,"bond_escrow_account":"escrow","slash_receiver_account":"slash"}
            """,
            """
            {"escrowed":true,"asset_definition_id":"asset","bond_escrow_account":false,"slash_receiver_account":"slash"}
            """,
            """
            {"escrowed":true,"asset_definition_id":"asset","bond_escrow_account":"escrow","slash_receiver_account":[]}
            """,
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceLockRecord.self,
                    from: governanceLockJSON(custodyJSON: custodyJSON)
                )
            )
        }
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
