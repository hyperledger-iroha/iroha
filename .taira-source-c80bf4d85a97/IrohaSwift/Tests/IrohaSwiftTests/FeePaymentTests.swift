import XCTest
@testable import IrohaSwift

final class FeePaymentTests: XCTestCase {
    private let authority =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
    private let tairaSponsor =
        "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A"
    private let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

    func testAuthorityIntentUsesCanonicalJSONAndNorito() throws {
        let intent = FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)

        XCTAssertEqual(
            String(decoding: try intent.canonicalJSONData(), as: UTF8.self),
            #"{"payer":"authority","value":{"charge_limits":[]}}"#
        )
        XCTAssertEqual(
            try intent.canonicalNorito().hexEncodedString(),
            "00000000190000000000000008000000000000000000000000000000010000000000000000"
        )
    }

    func testSponsorIntentRoundTripsWithoutRewritingSelection() throws {
        let program = try FeeSponsorProgramId(sponsor: authority, name: "wallet_fx")
        let limit = try FeeChargeLimit(
            kind: .pipelineGas,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "10.5"
        )
        let intent = FeePaymentIntent.sponsor(
            programId: program,
            programRevision: 7,
            chargeLimits: [limit],
            gasLimit: 9000
        )
        let encoded = try intent.canonicalJSONData()

        XCTAssertEqual(try JSONDecoder().decode(FeePaymentIntent.self, from: encoded), intent)
        XCTAssertTrue(String(decoding: encoded, as: UTF8.self).contains(#""program_revision":7"#))
        XCTAssertEqual(
            try AccountAddress.inspectI105NetworkPrefix(program.sponsor).chainDiscriminant,
            AccountId.defaultNetworkPrefix
        )
        XCTAssertNoThrow(try intent.compactNorito())
        XCTAssertNoThrow(try intent.canonicalNorito())
    }

    func testTairaSponsorProgramPreservesLiteralAndEncodesBothNoritoLayouts() throws {
        let programLiteral = "\(tairaSponsor)/cbsi_web"
        let program = try FeeSponsorProgramId(programLiteral)
        XCTAssertEqual(program.description, programLiteral)
        XCTAssertEqual(
            try AccountAddress.inspectI105NetworkPrefix(program.sponsor).chainDiscriminant,
            369
        )

        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let programJSON = try encoder.encode(program)
        XCTAssertEqual(
            String(decoding: programJSON, as: UTF8.self),
            #"{"name":"cbsi_web","sponsor":"\#(tairaSponsor)"}"#
        )
        XCTAssertEqual(
            try JSONDecoder().decode(FeeSponsorProgramId.self, from: programJSON),
            program
        )

        let intent = FeePaymentIntent.sponsor(
            programId: program,
            programRevision: 1,
            chargeLimits: [],
            gasLimit: nil
        )
        let intentJSON = try intent.canonicalJSONData()
        let decodedIntent = try JSONDecoder().decode(FeePaymentIntent.self, from: intentJSON)
        XCTAssertEqual(decodedIntent, intent)
        XCTAssertTrue(String(decoding: intentJSON, as: UTF8.self).contains(tairaSponsor))

        let address = try AccountAddress.parseEncoded(
            tairaSponsor,
            expectedPrefix: 369
        )
        let canonicalProgram = try sponsorProgramFields(
            in: intent.canonicalNorito(),
            compact: false
        )
        XCTAssertEqual(canonicalProgram.controller, try address.noritoAccountControllerPayload())
        XCTAssertEqual(canonicalProgram.name, CanonicalNorito.encodeString("cbsi_web"))

        let compactProgram = try sponsorProgramFields(
            in: intent.compactNorito(),
            compact: true
        )
        XCTAssertEqual(
            compactProgram.controller,
            try address.compactNoritoAccountControllerPayload()
        )
        XCTAssertEqual(compactProgram.name, CompactNorito.encodeString("cbsi_web"))

        // AccountId's Norito form is controller-only: changing only the
        // presentation discriminant must not alter consensus-visible bytes.
        let soraProgram = try FeeSponsorProgramId(
            sponsor: address.toI105(networkPrefix: AccountId.defaultNetworkPrefix),
            name: "cbsi_web"
        )
        let soraIntent = FeePaymentIntent.sponsor(
            programId: soraProgram,
            programRevision: 1,
            chargeLimits: [],
            gasLimit: nil
        )
        XCTAssertEqual(try soraIntent.canonicalNorito(), try intent.canonicalNorito())
        XCTAssertEqual(try soraIntent.compactNorito(), try intent.compactNorito())
    }

    func testSponsorProgramRejectsMalformedOrNonCanonicalI105Literals() throws {
        let suffix = tairaSponsor.dropFirst("test".count)
        var checksumMutation = tairaSponsor
        let finalDigit = checksumMutation.removeLast()
        checksumMutation.append(finalDigit == "1" ? "2" : "1")
        let canonicalHex = try AccountAddress
            .parseEncoded(tairaSponsor, expectedPrefix: 369)
            .canonicalHex()
        let invalidSponsors = [
            "",
            " \(tairaSponsor)",
            "\(tairaSponsor) ",
            "n369\(suffix)",
            "n0369\(suffix)",
            "n65536\(suffix)",
            tairaSponsor.replacingOccurrences(of: "ﾛ", with: "ロ"),
            checksumMutation,
            canonicalHex,
            "\(tairaSponsor)@cbsi",
        ]

        for sponsor in invalidSponsors {
            XCTAssertThrowsError(
                try FeeSponsorProgramId(sponsor: sponsor, name: "cbsi_web"),
                sponsor
            ) { error in
                XCTAssertEqual(
                    error as? FeePaymentIntentError,
                    .invalidSponsorAccount(sponsor)
                )
            }
        }

        let malformedJSON = Data(
            #"{"name":"cbsi_web","sponsor":"n369\#(suffix)"}"#.utf8
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(FeeSponsorProgramId.self, from: malformedJSON)
        )
    }

    func testSponsorProgramRejectsWrongProgramSyntaxAndNames() throws {
        XCTAssertNoThrow(
            try FeeSponsorProgramId(sponsor: tairaSponsor, name: "\u{e9}")
        )
        let malformedProgramIds = [
            tairaSponsor,
            "/cbsi_web",
            "\(tairaSponsor)/",
            "\(tairaSponsor)//cbsi_web",
            "\(tairaSponsor)/cbsi/web",
            " \(tairaSponsor)/cbsi_web",
            "\(tairaSponsor)/cbsi_web ",
        ]
        for literal in malformedProgramIds {
            XCTAssertThrowsError(try FeeSponsorProgramId(literal), literal)
        }

        for name in ["cbsi web", "cbsi@web", "cbsi#web", "cbsi$web", "cbsi/web", "e\u{301}"] {
            XCTAssertThrowsError(
                try FeeSponsorProgramId(sponsor: tairaSponsor, name: name),
                name
            ) { error in
                XCTAssertEqual(error as? FeePaymentIntentError, .invalidProgramName(name))
            }
        }
    }

    func testIntentRejectsUnknownFieldsAndNonCanonicalLimits() throws {
        let unknown = Data(
            #"{"payer":"authority","value":{"charge_limits":[]},"fee_sponsor":"legacy"}"#.utf8
        )
        XCTAssertThrowsError(try JSONDecoder().decode(FeePaymentIntent.self, from: unknown))

        let nexus = try FeeChargeLimit(
            kind: .nexus,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "1"
        )
        let gas = try FeeChargeLimit(
            kind: .pipelineGas,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "2"
        )
        let reversed = FeePaymentIntent.authority(chargeLimits: [gas, nexus], gasLimit: nil)
        XCTAssertThrowsError(try reversed.canonicalJSONData()) { error in
            XCTAssertEqual(error as? FeePaymentIntentError, .nonCanonicalChargeLimits)
        }
    }

    func testQuoteCanOnlyReplaceMaxima() throws {
        let draft = FeePaymentIntent.authority(chargeLimits: [], gasLimit: 100)
        let quoted = FeePaymentIntent.authority(
            chargeLimits: [try FeeChargeLimit(
                kind: .nexus,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "3"
            )],
            gasLimit: 100
        )
        let substituted = FeePaymentIntent.authority(chargeLimits: [], gasLimit: 101)

        XCTAssertTrue(draft.hasSamePayerAndGasBound(as: quoted))
        XCTAssertFalse(draft.hasSamePayerAndGasBound(as: substituted))
    }

    func testQuoteResponsePreservesCanonicalTairaAccountDebitSource() throws {
        func quoteJSON(account: String) -> Data {
            Data(
                """
                {
                  "intent":{"payer":"authority","value":{"charge_limits":[]}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":[],
                  "capacities":[],
                  "decision":{
                    "status":"accepted",
                    "value":{"debit_source":{"kind":"account","value":"\(account)"}}
                  }
                }
                """.utf8
            )
        }

        let quote = try JSONDecoder().decode(
            FeeQuoteResponse.self,
            from: quoteJSON(account: tairaSponsor)
        )
        XCTAssertEqual(quote.decision.debitSource, .account(tairaSponsor))
        XCTAssertEqual(
            try AccountAddress.inspectI105NetworkPrefix(tairaSponsor).chainDiscriminant,
            369
        )
        XCTAssertEqual(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: JSONEncoder().encode(quote)
            ),
            quote
        )

        let suffix = tairaSponsor.dropFirst("test".count)
        var checksumMutation = tairaSponsor
        let finalDigit = checksumMutation.removeLast()
        checksumMutation.append(finalDigit == "1" ? "2" : "1")
        for invalid in [
            " \(tairaSponsor)",
            "\(tairaSponsor) ",
            "n369\(suffix)",
            "n0369\(suffix)",
            tairaSponsor.replacingOccurrences(of: "ﾛ", with: "ロ"),
            checksumMutation,
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    FeeQuoteResponse.self,
                    from: quoteJSON(account: invalid)
                ),
                invalid
            )
        }
    }

    private func sponsorProgramFields(
        in intent: Data,
        compact: Bool
    ) throws -> (controller: Data, name: Data) {
        var intentReader = CanonicalNoritoReader(data: intent)
        XCTAssertEqual(try intentReader.readUInt32LE(), 1)
        let body = try readField(from: &intentReader, compact: compact)
        XCTAssertEqual(intentReader.remaining(), 0)

        var bodyReader = CanonicalNoritoReader(data: body)
        let program = try readField(from: &bodyReader, compact: compact)
        _ = try readField(from: &bodyReader, compact: compact)
        _ = try readField(from: &bodyReader, compact: compact)
        _ = try readField(from: &bodyReader, compact: compact)
        XCTAssertEqual(bodyReader.remaining(), 0)

        var programReader = CanonicalNoritoReader(data: program)
        let controller = try readField(from: &programReader, compact: compact)
        let name = try readField(from: &programReader, compact: compact)
        XCTAssertEqual(programReader.remaining(), 0)
        return (controller, name)
    }

    private func readField(
        from reader: inout CanonicalNoritoReader,
        compact: Bool
    ) throws -> Data {
        compact ? try reader.readCompactField() : try reader.readField()
    }
}
