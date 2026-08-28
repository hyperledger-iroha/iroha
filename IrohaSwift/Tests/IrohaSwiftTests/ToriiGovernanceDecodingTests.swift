import XCTest
@testable import IrohaSwift

final class ToriiGovernanceDecodingTests: XCTestCase {
    private static let governanceOwner =
        "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    private static let payoutAccounts = [
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "sorauﾛ1PULｦnUPﾀZ7ﾘﾕｻ2oｿSTfKｷﾋﾌﾀnTwEZヱVﾏｱﾐLZﾒZｾNVE5DS",
        "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L",
        "sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY",
        "sorauﾛ1PﾜdﾎｼﾋﾉNｸdﾁﾑkiﾇ3ｵﾓaPBQDTｲKqｼqｵrﾗｶwSQ1ﾌﾅQU61Y7",
    ]
    private static let contractAddress =
        "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
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

    private func fixedBytes(_ value: UInt8, count: Int = 32) -> String {
        "[" + Array(repeating: String(value), count: count).joined(separator: ",") + "]"
    }

    private func proposalKindJSON(kind: String, payload: String) -> Data {
        Data("{\"kind\":\"\(kind)\",\"payload\":\(payload)}".utf8)
    }

    private func payoutBindingJSON() -> String {
        let recipients = Array(Self.payoutAccounts.dropFirst()).map {
            "{\"account_id\":\"\($0)\",\"share\":\"0.25\"}"
        }.joined(separator: ",")
        return """
        {"contract_address":"\(Self.contractAddress)","code_hash":\(fixedBytes(7)),"entrypoint":"autonomous_validation_fee_tick","treasury_account_id":"\(Self.governanceOwner)","ds_asset_id":"5dHF5UNffENuEg9mhjYwY1jcZ1K5","xor_asset_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","pool_vault_account_id":"\(Self.payoutAccounts[0])","batch_ds":"10","min_xor_out":"4","max_xor_out":"100","recipients":[\(recipients)]}
        """
    }

    private func sccpG1JSON(_ value: UInt8) -> String {
        "{\"x\":\(fixedBytes(value)),\"y\":\(fixedBytes(value &+ 1))}"
    }

    private func sccpG2JSON(_ value: UInt8) -> String {
        """
        {"x_c0":\(fixedBytes(value)),"x_c1":\(fixedBytes(value &+ 1)),"y_c0":\(fixedBytes(value &+ 2)),"y_c1":\(fixedBytes(value &+ 3))}
        """
    }

    private func sccpVerifyingKeyJSON() -> String {
        let ic = (["constant"] + (0...10).map { "signal_\($0)" })
            .enumerated()
            .map { index, key in "\"\(key)\":\(sccpG1JSON(UInt8(index + 1)))" }
            .joined(separator: ",")
        return """
        {"version":1,"alpha1":\(sccpG1JSON(14)),"beta2":\(sccpG2JSON(16)),"gamma2":\(sccpG2JSON(20)),"delta2":\(sccpG2JSON(24)),"ic":{\(ic)}}
        """
    }

    private func sccpOutboundProofPolicyJSON() -> String {
        """
        {"version":1,"semantic_profile":{"profile":"sora_taira_finality_inclusion_groth16_bn254","commitments":{"version":1,"circuit_commitment":\(fixedBytes(28)),"witness_generator_commitment":\(fixedBytes(29)),"public_signal_schema_hash":\(fixedBytes(30))}},"sora_finality_anchor":{"version":1,"source_network":{"network":"sora_taira","profile":null},"protocol_version":4,"chain_id_hash":\(fixedBytes(31)),"checkpoint_height":1,"checkpoint_block_hash":\(fixedBytes(32)),"checkpoint_context_id":\(fixedBytes(33)),"checkpoint_finality_artifact_hash":\(fixedBytes(34))}}
        """
    }

    private func sccpEvmDestinationJSON(extraField: Bool = false) -> Data {
        let extra = extraField ? ",\"legacy_deployment\":null" : ""
        return Data(
            """
            {"family":"evm","deployment":{"token_address":\(fixedBytes(35, count: 20)),"token_code_hash":\(fixedBytes(36)),"verifier_address":\(fixedBytes(37, count: 20)),"verifier_code_hash":\(fixedBytes(38)),"verifying_key":\(sccpVerifyingKeyJSON()),"verifier_key_hash":\(fixedBytes(39)),"outbound_proof_policy":\(sccpOutboundProofPolicyJSON()),"route_address":\(fixedBytes(40, count: 20)),"route_code_hash":\(fixedBytes(41)),"taira_to_token_multiplier":1000000000,"max_wrapped_supply":1000000000000000000000\(extra)}}
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

    func testGovernanceProposalKindDecodesAllSevenV1Variants() throws {
        let deploy = proposalKindJSON(
            kind: "DeployContract",
            payload: """
            {"contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}
            """
        )
        guard case .deployContract(let deployPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: deploy
        ) else { return XCTFail("expected DeployContract") }
        XCTAssertEqual(deployPayload.codeHash, Data(repeating: 0x11, count: 32))

        let runtime = proposalKindJSON(
            kind: "RuntimeUpgrade",
            payload: """
            {"manifest":{"name":"runtime-v1","description":"upgrade","abi_version":1,"abi_hash":\(fixedBytes(3)),"added_syscalls":[],"added_pointer_types":[],"start_height":10,"end_height":20,"sbom_digests":[],"slsa_attestation":"","provenance":[]}}
            """
        )
        guard case .runtimeUpgrade(let runtimePayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: runtime
        ) else { return XCTFail("expected RuntimeUpgrade") }
        XCTAssertEqual(runtimePayload.manifest.endHeight, 20)

        let removeRoute = """
        {"network_id":"\(TestNetworkIds.canonical.literal)","action":{"action":"Remove","route":{"lane_id":{"source":{"network":"ethereum_sepolia","profile":null},"target":{"network":"sora_taira","profile":null}},"route_id":"taira_eth_xor","asset_key":"xor","revision":1}}}
        """
        let sccp = proposalKindJSON(
            kind: "SccpRouteGovernance",
            payload: "{\"anchor\":\(removeRoute)}"
        )
        guard case .sccpRouteGovernance(let sccpPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: sccp
        ), case .remove(let key) = sccpPayload.anchor.action else {
            return XCTFail("expected SCCP Remove")
        }
        XCTAssertEqual(key.revision, 1)

        let policy = proposalKindJSON(
            kind: "ValidationFeePolicy",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","policy":{"schema_version":1,"network_id":"\(TestNetworkIds.canonical.literal)","policy_version":"1","previous_policy_hash":null,"ds_asset_id":"5dHF5UNffENuEg9mhjYwY1jcZ1K5","ds_scale":2,"fee":"0","treasury_account_id":"\(Self.governanceOwner)","charging_mode":{"charging_mode":"DISABLED","value":null},"effective_from_height":"10","expires_after_height":null,"exemption_classes":[],"treasury_payout_binding":null},"payout_lifecycle_proposal_id":null}
            """
        )
        guard case .validationFeePolicy(let policyPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: policy
        ) else { return XCTFail("expected ValidationFeePolicy") }
        XCTAssertEqual(policyPayload.policy.fee, "0")

        let lifecycle = proposalKindJSON(
            kind: "ValidationFeePayoutLifecycle",
            payload: "{\"proposal_operator\":\"\(Self.governanceOwner)\",\"payout_binding\":\(payoutBindingJSON())}"
        )
        guard case .validationFeePayoutLifecycle(let lifecyclePayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: lifecycle
        ) else { return XCTFail("expected ValidationFeePayoutLifecycle") }
        XCTAssertEqual(lifecyclePayload.payoutBinding.recipients.count, 4)

        let musubi = proposalKindJSON(
            kind: "MusubiRegistryGovernance",
            payload: """
            {"kind":"RetargetAlias","value":{"alias":["demo"],"target":{"home_dataspace":1,"scope":{"kind":"DataspaceRoot","value":null},"name":["demo-package"]},"expected_revision":1}}
            """
        )
        guard case .musubiRegistryGovernance(let musubiPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: musubi
        ), case .retargetAlias(let action) = musubiPayload else {
            return XCTFail("expected Musubi RetargetAlias")
        }
        XCTAssertEqual(action.alias.value, "demo")

        let sorafs = proposalKindJSON(
            kind: "SorafsProviderGovernance",
            payload: """
            {"action":{"action":"establish","value":{"provider_id":[\(fixedBytes(9))],"owner":"\(Self.governanceOwner)"}}}
            """
        )
        guard case .sorafsProviderGovernance(let sorafsPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: sorafs
        ), case .establish(let action) = sorafsPayload.action else {
            return XCTFail("expected SoraFS establish")
        }
        XCTAssertEqual(action.providerId.bytes, Data(repeating: 9, count: 32))
    }

    func testGovernanceSccpDestinationUsesClosedTypedDeployment() throws {
        let decoded = try JSONDecoder().decode(
            ToriiGovernanceSccpDestination.self,
            from: sccpEvmDestinationJSON()
        )
        guard case let .evm(deployment) = decoded else {
            return XCTFail("expected typed EVM destination")
        }
        XCTAssertEqual(deployment.tokenAddress, Data(repeating: 35, count: 20))
        XCTAssertEqual(deployment.outboundProofPolicy.version, 1)
        XCTAssertEqual(deployment.maxWrappedSupply, "1000000000000000000000")
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpDestination.self,
                from: sccpEvmDestinationJSON(extraField: true)
            )
        )
    }

    func testGovernanceSccpSettlementRequiresExactUInt128Liability() throws {
        let canonical = Data(
            """
            {"asset_definition_id":"6TEAJqbb8oEPmLncoNiMRbLEK6tw","custody_owner":"\(Self.governanceOwner)","payload_amount_scale":9,"max_outstanding_liability":1000000000000}
            """.utf8
        )
        let settlement = try JSONDecoder().decode(
            ToriiGovernanceSccpSettlement.self,
            from: canonical
        )
        XCTAssertEqual(settlement.maxOutstandingLiability, "1000000000000")
        let maximum = String(UInt128.max)
        let maximumJSON = Data(
            String(decoding: canonical, as: UTF8.self).replacingOccurrences(
                of: "1000000000000",
                with: maximum
            ).utf8
        )
        XCTAssertEqual(
            try JSONDecoder().decode(
                ToriiGovernanceSccpSettlement.self,
                from: maximumJSON
            ).maxOutstandingLiability,
            maximum
        )
        for invalid in ["0", "340282366920938463463374607431768211456", "\"1000000000000\""] {
            let candidate = Data(
                String(decoding: canonical, as: UTF8.self).replacingOccurrences(
                    of: "1000000000000",
                    with: invalid
                ).utf8
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceSccpSettlement.self, from: candidate)
            )
        }
    }

    func testParliamentRawScannerPinsCanonicalSccpUInt128Spelling() throws {
        let maximum = String(UInt128.max)
        XCTAssertNoThrow(
            try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(
                in: Data("{\"max_wrapped_supply\":\(maximum)}".utf8),
                integerKeys: ["max_wrapped_supply", "max_outstanding_liability"]
            )
        )
        for literal in ["1.0", "1e0", "\"1\"", "-1", "01"] {
            XCTAssertThrowsError(
                try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(
                    in: Data("{\"max_outstanding_liability\":\(literal)}".utf8),
                    integerKeys: ["max_wrapped_supply", "max_outstanding_liability"]
                ),
                literal
            )
        }
    }

    func testGovernanceProposalKindRejectsUnknownAndRetiredShapes() {
        let unknown = proposalKindJSON(kind: "EnactReferendum", payload: "{}")
        let oldSingleKey = Data(
            "{\"DeployContract\":{\"contract_address\":\"\(Self.contractAddress)\"}}".utf8
        )
        let legacyDeploy = proposalKindJSON(
            kind: "DeployContract",
            payload: """
            {"contract_address":"\(Self.contractAddress)","code_hash_hex":"\(String(repeating: "11", count: 32))","abi_hash_hex":"\(String(repeating: "22", count: 32))","abi_version":"1","manifest_provenance":null}
            """
        )
        let missingProvenance = proposalKindJSON(
            kind: "DeployContract",
            payload: """
            {"contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1}
            """
        )
        let runtimeWithImplicitDefaults = proposalKindJSON(
            kind: "RuntimeUpgrade",
            payload: """
            {"manifest":{"name":"runtime-v1","description":"upgrade","abi_version":1,"abi_hash":\(fixedBytes(3)),"added_syscalls":[],"added_pointer_types":[],"start_height":10,"end_height":20}}
            """
        )
        let runtimeBeyondExactJSON = proposalKindJSON(
            kind: "RuntimeUpgrade",
            payload: """
            {"manifest":{"name":"runtime-v1","description":"upgrade","abi_version":1,"abi_hash":\(fixedBytes(3)),"added_syscalls":[],"added_pointer_types":[],"start_height":9007199254740992,"end_height":9007199254740993,"sbom_digests":[],"slsa_attestation":"","provenance":[]}}
            """
        )
        for json in [
            unknown,
            oldSingleKey,
            legacyDeploy,
            missingProvenance,
            runtimeWithImplicitDefaults,
            runtimeBeyondExactJSON,
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: json)
            )
        }
    }

    func testGovernanceProposalNestedActionsRejectUnknownTags() {
        let sccp = proposalKindJSON(
            kind: "SccpRouteGovernance",
            payload: """
            {"anchor":{"network_id":"\(TestNetworkIds.canonical.literal)","action":{"action":"LegacyRegister","route":{}}}}
            """
        )
        let musubi = proposalKindJSON(
            kind: "MusubiRegistryGovernance",
            payload: "{\"kind\":\"Unknown\",\"value\":{}}"
        )
        let sorafs = proposalKindJSON(
            kind: "SorafsProviderGovernance",
            payload: "{\"action\":{\"action\":\"replace\",\"value\":{}}}"
        )
        for json in [sccp, musubi, sorafs] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: json)
            )
        }
    }

    func testGovernanceProposalRecordIsExactAndStatusesAreClosed() throws {
        let kind = """
        {"kind":"DeployContract","payload":{"contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}}
        """
        for status in ["Proposed", "Rejected", "Enacted", "Superseded", "ExecutionFailed"] {
            let data = Data(
                "{\"proposer\":\"\(Self.governanceOwner)\",\"kind\":\(kind),\"created_height\":1,\"status\":\"\(status)\"}".utf8
            )
            XCTAssertNoThrow(
                try JSONDecoder().decode(ToriiGovernanceProposalRecord.self, from: data)
            )
        }
        for retired in [
            "{\"proposer\":\"\(Self.governanceOwner)\",\"kind\":\(kind),\"created_height\":1,\"status\":\"Approved\"}",
            "{\"proposer\":\"\(Self.governanceOwner)\",\"kind\":\(kind),\"created_height\":1}",
            "{\"proposer\":\"\(Self.governanceOwner)\",\"kind\":\(kind),\"created_height\":1,\"status\":\"Proposed\",\"pipeline\":{}}",
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceProposalRecord.self,
                    from: Data(retired.utf8)
                )
            )
        }
        let inexactHeight = Data(
            "{\"proposer\":\"\(Self.governanceOwner)\",\"kind\":\(kind),\"created_height\":9007199254740992,\"status\":\"Proposed\"}".utf8
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiGovernanceProposalRecord.self, from: inexactHeight)
        )
    }
}
