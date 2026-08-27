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
            #"{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}}"#
        )
        XCTAssertEqual(
            try intent.canonicalNorito().hexEncodedString(),
            "00000000190000000000000008000000000000000000000000000000010000000000000000"
        )
        XCTAssertEqual(
            try intent.compactNorito().hexEncodedString(),
            "000000000b0800000000000000000100"
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

    func testSponsorProgramEqualityUsesUniversalAccountIdentity() throws {
        let alternateSponsor = try exactCanonicalToriiAccountAddress(authority)
            .address.toI105(networkPrefix: 369)
        let defaultProgram = try FeeSponsorProgramId(
            sponsor: authority,
            name: "wallet_fx"
        )
        let alternateProgram = try FeeSponsorProgramId(
            sponsor: alternateSponsor,
            name: "wallet_fx"
        )
        XCTAssertEqual(defaultProgram, alternateProgram)
        XCTAssertEqual(Set([defaultProgram, alternateProgram]).count, 1)

        let draft = FeePaymentIntent.sponsor(
            programId: defaultProgram,
            programRevision: 7,
            chargeLimits: [],
            gasLimit: nil
        )
        let response = try JSONDecoder().decode(
            FeeQuoteResponse.self,
            from: Data(
                """
                {
                  "intent":{"payer":"sponsor","value":{"program_id":{"sponsor":"\(alternateSponsor)","name":"wallet_fx"},"program_revision":7,"charge_limits":[],"gas_limit":null}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":[],
                  "capacities":[],
                  "decision":{"status":"accepted","value":{"debit_source":{"kind":"sponsor_program","value":{"sponsor":"\(alternateSponsor)","name":"wallet_fx"}},"program_revision":7}}
                }
                """.utf8
            )
        )
        XCTAssertNoThrow(try response.applying(to: draft, authority: authority))
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
        XCTAssertNoThrow(
            try FeeSponsorProgramId(
                sponsor: tairaSponsor,
                name: "cbsi\u{200b}\u{2060}\u{feff}\u{ad}"
            )
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

        for name in [
            "cbsi web",
            "cbsi@web",
            "cbsi#web",
            "cbsi$web",
            "cbsi/web",
            "e\u{301}",
            String(repeating: "x", count: 256),
            "cbsi\u{91}",
            "cbsi\u{202e}",
        ] {
            XCTAssertThrowsError(
                try FeeSponsorProgramId(sponsor: tairaSponsor, name: name),
                name
            ) { error in
                XCTAssertEqual(error as? FeePaymentIntentError, .invalidProgramName(name))
            }
        }
    }

    func testFeeSponsorProgramRequiresCanonicalPayoutAccount() throws {
        let canonical = Data(
            #"{"id":{"sponsor":"\#(authority)","name":"wallet_fx"},"payout_account":"\#(authority)","lifecycle":{"state":"paused","value":null}}"#.utf8
        )
        let program = try JSONDecoder().decode(FeeSponsorProgram.self, from: canonical)
        XCTAssertEqual(program.payoutAccount, authority)

        let missing = Data(
            #"{"id":{"sponsor":"\#(authority)","name":"wallet_fx"},"lifecycle":{"state":"paused","value":null}}"#.utf8
        )
        XCTAssertThrowsError(try JSONDecoder().decode(FeeSponsorProgram.self, from: missing))

        for field in ["active_revision", "staged_revision", "scheduled_activation"] {
            let explicitNull = Data(
                """
                {"id":{"sponsor":"\(authority)","name":"wallet_fx"},"payout_account":"\(authority)","lifecycle":{"state":"paused","value":null},"\(field)":null}
                """.utf8
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(FeeSponsorProgram.self, from: explicitNull),
                field
            )
        }
    }

    func testIntentRejectsUnknownFieldsAndNonCanonicalLimits() throws {
        let unknown = Data(
            #"{"payer":"authority","value":{"charge_limits":[],"gas_limit":null},"fee_sponsor":"legacy"}"#.utf8
        )
        XCTAssertThrowsError(try JSONDecoder().decode(FeePaymentIntent.self, from: unknown))

        let missingGasLimit = Data(
            #"{"payer":"authority","value":{"charge_limits":[]}}"#.utf8
        )
        XCTAssertThrowsError(try JSONDecoder().decode(FeePaymentIntent.self, from: missingGasLimit))

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

        XCTAssertThrowsError(
            try FeeChargeLimit(
                kind: .nexus,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "1.0"
            )
        )
    }

    func testApplyingQuoteRejectsNonCanonicalDraftBeforePayerBinding() throws {
        let quote = try JSONDecoder().decode(
            FeeQuoteResponse.self,
            from: Data(
                """
                {
                  "intent":{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":[],
                  "capacities":[],
                  "decision":{"status":"accepted","value":{"debit_source":{"kind":"account","value":"\(authority)"},"program_revision":null}}
                }
                """.utf8
            )
        )
        let nexus = try FeeChargeLimit(
            kind: .nexus,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "1"
        )
        let gas = try FeeChargeLimit(
            kind: .pipelineGas,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "1"
        )

        XCTAssertThrowsError(
            try quote.applying(
                to: .authority(chargeLimits: [gas, nexus], gasLimit: nil),
                authority: authority
            )
        ) { error in
            XCTAssertEqual(error as? FeePaymentIntentError, .nonCanonicalChargeLimits)
        }
    }

    func testFeeQuoteNumericArithmeticMatchesRustQuantitySemantics() throws {
        let maximum =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
        let normalizedSum =
            "670390396497129854978701249910292306373968291029619668886178072186088201503677348840093714908345171384501592909324302542687694140597328497321682450304205"
        let scaleOneMaximum = "\(maximum.dropLast()).\(maximum.last!)"
        let scaleTwentyEightIndex = maximum.index(maximum.endIndex, offsetBy: -28)
        let scaleTwentyEightMaximum =
            "\(maximum[..<scaleTwentyEightIndex]).\(maximum[scaleTwentyEightIndex...])"

        let sum = try CanonicalNumeric(
            isNegative: false,
            scale: 1,
            digits: maximum
        ).adding(
            CanonicalNumeric(isNegative: false, scale: 1, digits: "3"),
            maxBytes: CanonicalNorito.maxBigIntBytes
        )
        XCTAssertEqual(sum.canonicalString, normalizedSum)
        XCTAssertEqual(
            CanonicalNumeric(
                isNegative: false,
                scale: 28,
                digits: maximum
            ).compared(
                to: CanonicalNumeric(isNegative: false, scale: 0, digits: maximum)
            ),
            .orderedAscending
        )

        func quoteJSON(
            nexusMaximum: String,
            gasMaximum: String?,
            vaultBalance: String,
            blockRemaining: String
        ) -> Data {
            let gasLimit = gasMaximum.map {
                ",{\"kind\":{\"kind\":\"pipeline_gas\",\"value\":null},\"asset_definition_id\":\"\(assetDefinitionId)\",\"max_amount\":\"\($0)\"}"
            } ?? ""
            let limits =
                "[{\"kind\":{\"kind\":\"nexus\",\"value\":null},\"asset_definition_id\":\"\(assetDefinitionId)\",\"max_amount\":\"\(nexusMaximum)\"}\(gasLimit)]"
            return Data(
                """
                {
                  "intent":{"payer":"sponsor","value":{"program_id":{"sponsor":"\(authority)","name":"wallet_fx"},"program_revision":7,"charge_limits":\(limits),"gas_limit":null}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":\(limits),
                  "capacities":[{"asset_definition_id":"\(assetDefinitionId)","vault_balance":"\(vaultBalance)","reserve_floor":"0","block_remaining":"\(blockRemaining)","program_epoch_remaining":"\(vaultBalance)","beneficiary_epoch_remaining":"\(vaultBalance)"}],
                  "decision":{"status":"accepted","value":{"debit_source":{"kind":"sponsor_program","value":{"sponsor":"\(authority)","name":"wallet_fx"}},"program_revision":7}}
                }
                """.utf8
            )
        }

        XCTAssertNoThrow(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: quoteJSON(
                    nexusMaximum: scaleOneMaximum,
                    gasMaximum: "0.3",
                    vaultBalance: normalizedSum,
                    blockRemaining: normalizedSum
                )
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: quoteJSON(
                    nexusMaximum: maximum,
                    gasMaximum: nil,
                    vaultBalance: maximum,
                    blockRemaining: scaleTwentyEightMaximum
                )
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: quoteJSON(
                    nexusMaximum: "1",
                    gasMaximum: nil,
                    vaultBalance: "1",
                    blockRemaining: "1.0"
                )
            )
        )
    }

    func testFeeQuoteRejectsUnknownFieldsAtEveryNestedV1Boundary() throws {
        enum PathElement {
            case key(String)
            case index(Int)
        }

        let canonical = Data(
            """
            {
              "intent":{"payer":"sponsor","value":{"program_id":{"sponsor":"\(authority)","name":"wallet_fx"},"program_revision":7,"charge_limits":[{"kind":{"kind":"nexus","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"1"}],"gas_limit":null}},
              "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
              "components":[{"kind":{"kind":"nexus","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"1"}],
              "capacities":[{"asset_definition_id":"\(assetDefinitionId)","vault_balance":"1","reserve_floor":"0","block_remaining":"1","program_epoch_remaining":"1","beneficiary_epoch_remaining":"1"}],
              "decision":{"status":"accepted","value":{"debit_source":{"kind":"sponsor_program","value":{"sponsor":"\(authority)","name":"wallet_fx"}},"program_revision":7}}
            }
            """.utf8
        )
        XCTAssertNoThrow(try JSONDecoder().decode(FeeQuoteResponse.self, from: canonical))

        func insertingUnknownField(_ value: Any, at path: ArraySlice<PathElement>) -> Any {
            guard let head = path.first else {
                var object = value as! [String: Any]
                object["retired_v0"] = true
                return object
            }
            switch head {
            case let .key(key):
                var object = value as! [String: Any]
                object[key] = insertingUnknownField(object[key]!, at: path.dropFirst())
                return object
            case let .index(index):
                var array = value as! [Any]
                array[index] = insertingUnknownField(array[index], at: path.dropFirst())
                return array
            }
        }

        let paths: [[PathElement]] = [
            [],
            [.key("intent")],
            [.key("intent"), .key("value")],
            [.key("intent"), .key("value"), .key("program_id")],
            [.key("intent"), .key("value"), .key("charge_limits"), .index(0)],
            [.key("intent"), .key("value"), .key("charge_limits"), .index(0), .key("kind")],
            [.key("observation")],
            [.key("components"), .index(0)],
            [.key("components"), .index(0), .key("kind")],
            [.key("capacities"), .index(0)],
            [.key("decision")],
            [.key("decision"), .key("value")],
            [.key("decision"), .key("value"), .key("debit_source")],
            [.key("decision"), .key("value"), .key("debit_source"), .key("value")],
        ]
        let root = try JSONSerialization.jsonObject(with: canonical)
        for path in paths {
            let mutation = insertingUnknownField(root, at: path[...])
            let encoded = try JSONSerialization.data(withJSONObject: mutation)
            XCTAssertThrowsError(
                try JSONDecoder().decode(FeeQuoteResponse.self, from: encoded),
                "accepted unknown field at path \(path)"
            )
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
                  "intent":{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":[],
                  "capacities":[],
                  "decision":{
                    "status":"accepted",
                    "value":{"debit_source":{"kind":"account","value":"\(account)"},"program_revision":null}
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
            try quote.applying(
                to: .authority(chargeLimits: [], gasLimit: nil),
                authority: tairaSponsor
            ),
            quote.intent
        )
        XCTAssertThrowsError(
            try quote.applying(
                to: .authority(chargeLimits: [], gasLimit: nil),
                authority: authority
            )
        )
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

        let alternateAuthorityDisplay = try exactCanonicalToriiAccountAddress(authority)
            .address.toI105(networkPrefix: 369)
        let alternateDisplayQuote = try JSONDecoder().decode(
            FeeQuoteResponse.self,
            from: quoteJSON(account: alternateAuthorityDisplay)
        )
        XCTAssertEqual(
            try alternateDisplayQuote.applying(
                to: .authority(chargeLimits: [], gasLimit: nil),
                authority: authority
            ),
            alternateDisplayQuote.intent
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

        let missingProgramRevision = String(
            decoding: quoteJSON(account: tairaSponsor),
            as: UTF8.self
        ).replacingOccurrences(of: ",\"program_revision\":null", with: "")
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: Data(missingProgramRevision.utf8)
            )
        )
    }

    func testSponsorQuoteRejectsDecisionAndAggregateCapacityTampering() throws {
        let program = try FeeSponsorProgramId(sponsor: authority, name: "wallet_fx")

        func quoteJSON(
            capacities: String,
            gasComponentMaximum: String = "4",
            decisionRevision: UInt64 = 7
        ) -> Data {
            Data(
                """
                {
                  "intent":{"payer":"sponsor","value":{
                    "program_id":{"sponsor":"\(authority)","name":"wallet_fx"},
                    "program_revision":7,
                    "charge_limits":[
                      {"kind":{"kind":"nexus","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"3"},
                      {"kind":{"kind":"pipeline_gas","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"4"}
                    ],
                    "gas_limit":null
                  }},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":[
                    {"kind":{"kind":"nexus","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"3"},
                    {"kind":{"kind":"pipeline_gas","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"\(gasComponentMaximum)"}
                  ],
                  "capacities":\(capacities),
                  "decision":{"status":"accepted","value":{
                    "debit_source":{"kind":"sponsor_program","value":{"sponsor":"\(authority)","name":"wallet_fx"}},
                    "program_revision":\(decisionRevision)
                  }}
                }
                """.utf8
            )
        }

        func capacity(
            asset: String = assetDefinitionId,
            vault: String = "10",
            block: String = "7",
            programEpoch: String = "7",
            beneficiaryEpoch: String = "7"
        ) -> String {
            """
            [{"asset_definition_id":"\(asset)","vault_balance":"\(vault)","reserve_floor":"3","block_remaining":"\(block)","program_epoch_remaining":"\(programEpoch)","beneficiary_epoch_remaining":"\(beneficiaryEpoch)"}]
            """
        }

        let quote = try JSONDecoder().decode(
            FeeQuoteResponse.self,
            from: quoteJSON(capacities: capacity())
        )
        let draft = FeePaymentIntent.sponsor(
            programId: program,
            programRevision: 7,
            chargeLimits: [
                try FeeChargeLimit(
                    kind: .nexus,
                    assetDefinitionId: assetDefinitionId,
                    maxAmount: "30"
                ),
                try FeeChargeLimit(
                    kind: .pipelineGas,
                    assetDefinitionId: assetDefinitionId,
                    maxAmount: "40"
                ),
            ],
            gasLimit: nil
        )
        XCTAssertEqual(
            try quote.applying(to: draft, authority: authority),
            quote.intent
        )

        let unrelatedAsset = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        for mutation in [
            quoteJSON(capacities: "[]"),
            quoteJSON(capacities: capacity(vault: "9")),
            quoteJSON(capacities: capacity(block: "6")),
            quoteJSON(capacities: capacity(programEpoch: "6")),
            quoteJSON(capacities: capacity(beneficiaryEpoch: "6")),
            quoteJSON(capacities: capacity(asset: unrelatedAsset)),
            quoteJSON(capacities: capacity(), gasComponentMaximum: "5"),
            quoteJSON(capacities: capacity(), decisionRevision: 8),
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(FeeQuoteResponse.self, from: mutation)
            )
        }
    }

    func testSponsorQuoteRequiresCanonicalCapacityOrderAndPreservesFeeFreeQuotes() throws {
        let secondAsset = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
        let unrelatedAsset = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
        let orderedAssets = [assetDefinitionId, secondAsset].sorted { lhs, rhs in
            AssetDefinitionAddressCodec.uuidBytes(lhs)!.lexicographicallyPrecedes(
                AssetDefinitionAddressCodec.uuidBytes(rhs)!
            )
        }

        func capacity(for asset: String) -> String {
            let amount = asset == assetDefinitionId ? "2" : "3"
            return """
            {"asset_definition_id":"\(asset)","vault_balance":"\(amount)","reserve_floor":"0","block_remaining":"\(amount)","program_epoch_remaining":"\(amount)","beneficiary_epoch_remaining":"\(amount)"}
            """
        }

        func quoteJSON(capacities: String, feeFree: Bool = false) -> Data {
            let limits = feeFree ? "[]" : """
            [
              {"kind":{"kind":"nexus","value":null},"asset_definition_id":"\(assetDefinitionId)","max_amount":"2"},
              {"kind":{"kind":"pipeline_gas","value":null},"asset_definition_id":"\(secondAsset)","max_amount":"3"}
            ]
            """
            return Data(
                """
                {
                  "intent":{"payer":"sponsor","value":{"program_id":{"sponsor":"\(authority)","name":"wallet_fx"},"program_revision":7,"charge_limits":\(limits),"gas_limit":null}},
                  "observation":{"ledger_time_ms":1,"next_block_height":1,"route_dataspace_id":0},
                  "components":\(limits),
                  "capacities":\(capacities),
                  "decision":{"status":"accepted","value":{"debit_source":{"kind":"sponsor_program","value":{"sponsor":"\(authority)","name":"wallet_fx"}},"program_revision":7}}
                }
                """.utf8
            )
        }

        let canonicalCapacities = "[\(orderedAssets.map(capacity).joined(separator: ","))]"
        XCTAssertNoThrow(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: quoteJSON(capacities: canonicalCapacities)
            )
        )
        let reversedCapacities = "[\(orderedAssets.reversed().map(capacity).joined(separator: ","))]"
        let duplicateCapacities = "[\(capacity(for: orderedAssets[0])),\(capacity(for: orderedAssets[0]))]"
        let unrelatedCapacities = "[\(capacity(for: orderedAssets[0])),\(capacity(for: unrelatedAsset))]"
        for mutation in [reversedCapacities, duplicateCapacities, unrelatedCapacities] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    FeeQuoteResponse.self,
                    from: quoteJSON(capacities: mutation)
                )
            )
        }

        XCTAssertNoThrow(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: quoteJSON(capacities: "[]", feeFree: true)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                FeeQuoteResponse.self,
                from: quoteJSON(capacities: "[\(capacity(for: assetDefinitionId))]", feeFree: true)
            )
        )
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
