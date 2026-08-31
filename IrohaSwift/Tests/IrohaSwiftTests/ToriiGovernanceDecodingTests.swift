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

    private func byteArrayJSON(_ value: Data) -> String {
        "[" + value.map(String.init).joined(separator: ",") + "]"
    }

    private func appendUInt32LE(_ value: UInt32, to output: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { output.append(contentsOf: $0) }
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

    private func sccpVerifyingKeyBytes() -> Data {
        func repeated(_ value: UInt8) -> Data { Data(repeating: value, count: 32) }
        func g1(_ value: UInt8) -> Data { repeated(value) + repeated(value &+ 1) }
        func g2(_ value: UInt8) -> Data {
            repeated(value) + repeated(value &+ 1) + repeated(value &+ 2) + repeated(value &+ 3)
        }
        var result = g1(14) + g2(16) + g2(20) + g2(24)
        for value in UInt8(1)...UInt8(12) { result.append(g1(value)) }
        return result
    }

    private func sccpBn254SchemaHash() -> Data {
        let labels = [
            "sccp:groth16-bn254:signal:message-id:v1",
            "sccp:groth16-bn254:signal:payload-hash:v1",
            "sccp:groth16-bn254:signal:target-domain:v1",
            "sccp:groth16-bn254:signal:commitment-root:v1",
            "sccp:groth16-bn254:signal:finality-height:v1",
            "sccp:groth16-bn254:signal:finality-block-hash:v1",
            "sccp:groth16-bn254:signal:source-domain:v1",
            "sccp:groth16-bn254:signal:statement-hash:v1",
            "sccp:groth16-bn254:signal:destination-binding-hash:v1",
            "sccp:groth16-bn254:signal:route-configuration-hash:v1",
            "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
        ]
        var canonical = Data([1])
        appendUInt32LE(UInt32(labels.count), to: &canonical)
        for label in labels {
            let bytes = Data(label.utf8)
            appendUInt32LE(UInt32(bytes.count), to: &canonical)
            canonical.append(bytes)
        }
        return irohaKeccak256(
            Data("sccp:groth16-bn254:public-signal-schema:v1".utf8) + canonical
        )
    }

    private func sccpTairaChainIdHash() -> Data {
        irohaKeccak256(Data([
            0xfc, 0x56, 0x98, 0x4b, 0x2b, 0xe7, 0x43, 0x1d,
            0x84, 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83, 0xf0,
        ]))
    }

    private func sccpOutboundProofPolicyJSON() -> String {
        """
        {"version":1,"semantic_profile":{"profile":"sora_taira_finality_inclusion_groth16_bn254","commitments":{"version":1,"circuit_commitment":\(fixedBytes(28)),"witness_generator_commitment":\(fixedBytes(29)),"public_signal_schema_hash":\(byteArrayJSON(sccpBn254SchemaHash()))}},"sora_finality_anchor":{"version":1,"source_network":{"network":"sora_taira","profile":null},"protocol_version":4,"chain_id_hash":\(byteArrayJSON(sccpTairaChainIdHash())),"checkpoint_height":1,"checkpoint_block_hash":\(fixedBytes(32)),"checkpoint_context_id":\(fixedBytes(33)),"checkpoint_finality_artifact_hash":\(fixedBytes(34))}}
        """
    }

    private func sccpEvmDestinationJSON(
        extraField: Bool = false,
        maxWrappedSupply: String = "9000000000"
    ) -> Data {
        let extra = extraField ? ",\"legacy_deployment\":null" : ""
        return Data(
            """
            {"family":"evm","deployment":{"token_address":\(fixedBytes(35, count: 20)),"token_code_hash":\(fixedBytes(36)),"verifier_address":\(fixedBytes(37, count: 20)),"verifier_code_hash":\(fixedBytes(38)),"verifying_key":\(sccpVerifyingKeyJSON()),"verifier_key_hash":\(byteArrayJSON(irohaKeccak256(sccpVerifyingKeyBytes()))),"outbound_proof_policy":\(sccpOutboundProofPolicyJSON()),"route_address":\(fixedBytes(40, count: 20)),"route_code_hash":\(fixedBytes(41)),"replay_verifier_address":\(fixedBytes(42, count: 20)),"replay_verifier_code_hash":\(fixedBytes(43)),"mint_breaker_address":\(fixedBytes(44, count: 20)),"mint_breaker_code_hash":\(fixedBytes(45)),"taira_to_token_multiplier":1000000000,"max_wrapped_supply":\(maxWrappedSupply)\(extra)}}
            """.utf8
        )
    }

    private func sccpInboundEthereumLaneJSON() -> String {
        """
        {"source":{"network":"ethereum_mainnet","profile":null},"target":{"network":"sora_taira","profile":null}}
        """
    }

    private func sccpGovernedRouteJSON(
        routeConfigurationHash: Data,
        routeId: String = "taira_eth_xor",
        sourceAddress: String? = nil,
        sourceRuntimeCodeHash: String? = nil,
        maxWrappedSupply: String = "9000000000",
        maxOutstandingLiability: String = "9"
    ) -> Data {
        let lane = sccpInboundEthereumLaneJSON()
        let destination = String(
            decoding: sccpEvmDestinationJSON(maxWrappedSupply: maxWrappedSupply),
            as: UTF8.self
        )
        return Data(
            """
            {"lane_id":\(lane),"route_id":"\(routeId)","asset_key":"xor","revision":1,"activation":{"activation":"staged","direction":null},"inbound_finality_cutoff":null,"source_identity":{"lane":\(lane),"emitter":{"emitter":"evm","identity":{"address":\(sourceAddress ?? fixedBytes(40, count: 20)),"runtime_code_hash":\(sourceRuntimeCodeHash ?? fixedBytes(41)),"route_config_hash":\(byteArrayJSON(routeConfigurationHash))}}},"destination":\(destination),"sora_outbound_execution_policy":{"version":1,"semantics":"ivm_proved_record_sccp_message_v1","contract_artifact_sha256":\(fixedBytes(70)),"vk_ref":{"backend":"stark-fri-v1","name":"ivm-execution-v1","version":1,"commitment":\(fixedBytes(71))},"gas_limit":50000000},"settlement":{"asset_definition_id":"6TEAJqbb8oEPmLncoNiMRbLEK6tw","payload_amount_scale":9,"max_outstanding_liability":\(maxOutstandingLiability)}}
            """.utf8
        )
    }

    private func sccpRegisterProposalJSON(route: Data) -> Data {
        proposalKindJSON(
            kind: "SccpRouteGovernance",
            payload: """
            {"anchor":{"network_id":"\(TestNetworkIds.canonical.literal)","action":{"action":"Register","route":{"route":\(String(decoding: route, as: UTF8.self)),"native_trust_anchor":null}}}}
            """
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

    func testGovernanceProposalKindDecodesAllTenV1Variants() throws {
        let deploy = proposalKindJSON(
            kind: "DeployContract",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}
            """
        )
        guard case .deployContract(let deployPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: deploy
        ) else { return XCTFail("expected DeployContract") }
        XCTAssertEqual(deployPayload.proposalOperator, Self.governanceOwner)
        XCTAssertEqual(deployPayload.codeHash, Data(repeating: 0x11, count: 32))

        let runtime = proposalKindJSON(
            kind: "RuntimeUpgrade",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","manifest":{"name":"runtime-v1","description":"upgrade","abi_version":1,"abi_hash":\(fixedBytes(3)),"added_syscalls":[],"added_pointer_types":[],"start_height":10,"end_height":20,"sbom_digests":[],"slsa_attestation":"","provenance":[]}}
            """
        )
        guard case .runtimeUpgrade(let runtimePayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: runtime
        ) else { return XCTFail("expected RuntimeUpgrade") }
        XCTAssertEqual(runtimePayload.proposalOperator, Self.governanceOwner)
        XCTAssertEqual(runtimePayload.manifest.endHeight, 20)

        let removeRoute = """
        {"network_id":"\(TestNetworkIds.canonical.literal)","action":{"action":"Remove","route":{"lane_id":{"source":{"network":"ethereum_mainnet","profile":null},"target":{"network":"sora_taira","profile":null}},"route_id":"taira_eth_xor","asset_key":"xor","revision":1}}}
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

        let contractLifecycle = proposalKindJSON(
            kind: "ContractLifecycleGovernance",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","contract_address":"\(Self.contractAddress)","expected_revision":1,"action":{"action":"CompleteEmergencyHoldRetrospective","payload":{"hold_proposal_content_id":\(fixedBytes(17)),"hold_governance_attempt_id":\(fixedBytes(34)),"incident_digest":\(fixedBytes(51)),"retrospective_finding_root":\(fixedBytes(68))}}}
            """
        )
        guard case .contractLifecycleGovernance(let contractLifecyclePayload) =
            try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: contractLifecycle),
            case .completeEmergencyHoldRetrospective(let retrospective) =
                contractLifecyclePayload.action else {
            return XCTFail("expected contract emergency-hold retrospective")
        }
        XCTAssertEqual(contractLifecyclePayload.proposalOperator, Self.governanceOwner)
        XCTAssertEqual(retrospective.holdProposalContentId, Data(repeating: 17, count: 32))
        XCTAssertEqual(retrospective.holdGovernanceAttemptId, Data(repeating: 34, count: 32))
        XCTAssertEqual(retrospective.incidentDigest, Data(repeating: 51, count: 32))
        XCTAssertEqual(retrospective.retrospectiveFindingRoot, Data(repeating: 68, count: 32))

        let emergencyHold = proposalKindJSON(
            kind: "ContractEmergencyHold",
            payload: """
            {"contract_address":"\(Self.contractAddress)","expected_revision":2,"expected_code_hash":"\(String(repeating: "ab", count: 32))","incident_digest":\(fixedBytes(85)),"reason":"contain compromised entrypoint","duration_blocks":3600}
            """
        )
        guard case .contractEmergencyHold(let emergencyHoldPayload) =
            try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: emergencyHold) else {
            return XCTFail("expected ContractEmergencyHold")
        }
        XCTAssertEqual(emergencyHoldPayload.expectedRevision, 2)
        XCTAssertEqual(emergencyHoldPayload.expectedCodeHash, Data(repeating: 0xab, count: 32))
        XCTAssertEqual(emergencyHoldPayload.incidentDigest, Data(repeating: 85, count: 32))
        XCTAssertEqual(emergencyHoldPayload.durationBlocks, 3_600)

        let triggerPermission = proposalKindJSON(
            kind: "GlobalDataTriggerPermissionGovernance",
            payload: """
            {"authority":"\(Self.governanceOwner)","action":{"action":"grant","value":null}}
            """
        )
        guard case .globalDataTriggerPermissionGovernance(let triggerPermissionPayload) =
            try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: triggerPermission) else {
            return XCTFail("expected GlobalDataTriggerPermissionGovernance")
        }
        XCTAssertEqual(triggerPermissionPayload.authority, Self.governanceOwner)
        XCTAssertEqual(triggerPermissionPayload.action, .grant)
    }

    func testGlobalDataTriggerPermissionRequiresCanonicalAccountAndClosedUnitAction() throws {
        for action in ["grant", "revoke"] {
            let proposal = proposalKindJSON(
                kind: "GlobalDataTriggerPermissionGovernance",
                payload: """
                {"authority":"\(Self.governanceOwner)","action":{"action":"\(action)","value":null}}
                """
            )
            XCTAssertNoThrow(
                try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: proposal)
            )
        }

        for payload in [
            "{\"authority\":\"\(Self.governanceOwner)\",\"action\":{\"action\":\"grant\"}}",
            "{\"authority\":\"\(Self.governanceOwner)\",\"action\":{\"action\":\"grant\",\"value\":{}}}",
            "{\"authority\":\"\(Self.governanceOwner)\",\"action\":{\"action\":\"delegate\",\"value\":null}}",
            "{\"authority\":\"alice@wonderland\",\"action\":{\"action\":\"grant\",\"value\":null}}",
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceProposalKind.self,
                    from: proposalKindJSON(
                        kind: "GlobalDataTriggerPermissionGovernance",
                        payload: payload
                    )
                )
            )
        }
    }

    func testContractLifecycleActionInventoryUsesCanonicalPayloadShapes() throws {
        let actionPayloads = [
            """
            {"action":"Activate","payload":{"code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}}
            """,
            """
            {"action":"Deactivate","payload":{"expected_code_hash":"\(String(repeating: "33", count: 32))","reason":null}}
            """,
            """
            {"action":"OfferOwnership","payload":{"new_owner":"\(Self.governanceOwner)"}}
            """,
            "{\"action\":\"CancelOwnershipOffer\",\"payload\":null}",
            "{\"action\":\"AcceptParliamentOwnership\",\"payload\":null}",
            """
            {"action":"CompleteEmergencyHoldRetrospective","payload":{"hold_proposal_content_id":\(fixedBytes(17)),"hold_governance_attempt_id":\(fixedBytes(34)),"incident_digest":\(fixedBytes(51)),"retrospective_finding_root":\(fixedBytes(68))}}
            """,
        ]
        let actionTags = try actionPayloads.map { payload in
            let object = try XCTUnwrap(
                JSONSerialization.jsonObject(with: Data(payload.utf8)) as? [String: Any]
            )
            return try XCTUnwrap(object["action"] as? String)
        }
        XCTAssertEqual(actionTags, ToriiParliamentAPIV1.contractLifecycleActions)
        for payload in actionPayloads {
            XCTAssertNoThrow(
                try JSONDecoder().decode(
                    ToriiGovernanceContractLifecycleActionV1.self,
                    from: Data(payload.utf8)
                )
            )
        }

        let zeroRoot = fixedBytes(0)
        let invalidActions = [
            "{\"action\":\"CancelOwnershipOffer\"}",
            "{\"action\":\"AcceptParliamentOwnership\",\"payload\":{}}",
            "{\"action\":\"LegacyActivate\",\"payload\":null}",
            """
            {"action":"Activate","payload":{"code_hash":"\(String(repeating: "AA", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}}
            """,
            """
            {"action":"CompleteEmergencyHoldRetrospective","payload":{"hold_proposal_content_id":\(fixedBytes(17)),"hold_governance_attempt_id":\(fixedBytes(34)),"incident_digest":\(fixedBytes(51)),"retrospective_finding_root":\(zeroRoot)}}
            """,
        ]
        for payload in invalidActions {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceContractLifecycleActionV1.self,
                    from: Data(payload.utf8)
                )
            )
        }
    }

    func testContractEmergencyHoldRejectsInvalidContainmentFields() {
        let validPrefix =
            "{\"kind\":\"ContractEmergencyHold\",\"payload\":{\"contract_address\":\"\(Self.contractAddress)\",\"expected_revision\":1,\"expected_code_hash\":\"\(String(repeating: "11", count: 32))\",\"incident_digest\":\(fixedBytes(7)),"
        for suffix in [
            "\"reason\":\"   \",\"duration_blocks\":1}}",
            "\"reason\":\"containment\",\"duration_blocks\":0}}",
            "\"reason\":\"containment\",\"duration_blocks\":3601}}",
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceProposalKind.self,
                    from: Data((validPrefix + suffix).utf8)
                )
            )
        }
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
        XCTAssertEqual(deployment.replayVerifierAddress, Data(repeating: 42, count: 20))
        XCTAssertEqual(deployment.mintBreakerAddress, Data(repeating: 44, count: 20))
        XCTAssertEqual(deployment.maxWrappedSupply, "9000000000")
        XCTAssertEqual(deployment.outboundProofPolicy.version, 1)
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpDestination.self,
                from: sccpEvmDestinationJSON(extraField: true)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpDestination.self,
                from: sccpEvmDestinationJSON(maxWrappedSupply: "1.5")
            )
        )
    }

    func testGovernanceSccpDestinationBindsKeySchemaAndTairaChain() throws {
        let canonical = String(decoding: sccpEvmDestinationJSON(), as: UTF8.self)
        let substitutions = [
            (byteArrayJSON(irohaKeccak256(sccpVerifyingKeyBytes())), fixedBytes(39)),
            (byteArrayJSON(sccpBn254SchemaHash()), fixedBytes(30)),
            (byteArrayJSON(sccpTairaChainIdHash()), fixedBytes(31)),
        ]
        for (expected, invalid) in substitutions {
            XCTAssertTrue(canonical.contains(expected))
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceSccpDestination.self,
                    from: Data(canonical.replacingOccurrences(of: expected, with: invalid).utf8)
                )
            )
        }

        let decoded = try JSONDecoder().decode(
            ToriiGovernanceSccpDestination.self,
            from: sccpEvmDestinationJSON()
        )
        guard case let .evm(deployment) = decoded else {
            return XCTFail("expected typed EVM destination")
        }
        let semanticProfileHash = deployment.outboundProofPolicy.discoveryValue
            .semanticProfile.profileHash
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpDestination.self,
                from: Data(
                    canonical.replacingOccurrences(
                        of: fixedBytes(36),
                        with: byteArrayJSON(semanticProfileHash)
                    ).utf8
                )
            ),
            "deployment hashes must remain distinct from derived proof-policy hashes"
        )
    }

    func testGovernanceSccpRouteBindsEmitterAndConfigurationHash() throws {
        let lane = try JSONDecoder().decode(
            ToriiGovernanceSccpLane.self,
            from: Data(sccpInboundEthereumLaneJSON().utf8)
        )
        let destination = try JSONDecoder().decode(
            ToriiGovernanceSccpDestination.self,
            from: sccpEvmDestinationJSON()
        )
        let discoveryLane = try lane.discoveryValue
        let discoveryDestination = try destination.discoveryValue(for: discoveryLane)
        let configuration = try SccpExactParser.routeConfigurationHash(
            lane: discoveryLane,
            routeId: "taira_eth_xor",
            assetKey: "xor",
            revision: 1,
            destination: discoveryDestination
        )
        XCTAssertNoThrow(
            try JSONDecoder().decode(
                ToriiGovernanceSccpGovernedRoute.self,
                from: sccpGovernedRouteJSON(routeConfigurationHash: configuration)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpGovernedRoute.self,
                from: sccpGovernedRouteJSON(routeConfigurationHash: Data(repeating: 99, count: 32))
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpGovernedRoute.self,
                from: sccpGovernedRouteJSON(
                    routeConfigurationHash: configuration,
                    sourceAddress: fixedBytes(99, count: 20)
                )
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpGovernedRoute.self,
                from: sccpGovernedRouteJSON(
                    routeConfigurationHash: configuration,
                    routeId: "taira_bsc_xor"
                )
            )
        )

        let canonicalRoute = String(
            decoding: sccpGovernedRouteJSON(routeConfigurationHash: configuration),
            as: UTF8.self
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiGovernanceSccpGovernedRoute.self,
                from: Data(
                    canonicalRoute.replacingOccurrences(
                        of: fixedBytes(70),
                        with: fixedBytes(36)
                    ).utf8
                )
            ),
            "execution-policy hashes must remain distinct from all deployment hashes"
        )
    }

    func testGovernanceSccpCapsRemainLexemeExactThroughDraftEncoding() throws {
        let liability = "18446744073709551616"
        let wrappedSupply = "18446744073709551616000000000"
        let destinationData = sccpEvmDestinationJSON(maxWrappedSupply: wrappedSupply)
        let destinationDecoder = JSONDecoder()
        destinationDecoder.userInfo[governanceExactIntegerLexemesUserInfoKey] =
            try governanceExactJSONIntegerLexemes(destinationData)
        let destination = try destinationDecoder.decode(
            ToriiGovernanceSccpDestination.self,
            from: destinationData
        )
        let lane = try JSONDecoder().decode(
            ToriiGovernanceSccpLane.self,
            from: Data(sccpInboundEthereumLaneJSON().utf8)
        )
        let discoveryLane = try lane.discoveryValue
        let discoveryDestination = try destination.discoveryValue(for: discoveryLane)
        let configuration = try SccpExactParser.routeConfigurationHash(
            lane: discoveryLane,
            routeId: "taira_eth_xor",
            assetKey: "xor",
            revision: 1,
            destination: discoveryDestination
        )
        let route = sccpGovernedRouteJSON(
            routeConfigurationHash: configuration,
            maxWrappedSupply: wrappedSupply,
            maxOutstandingLiability: liability
        )
        let proposalData = sccpRegisterProposalJSON(route: route)
        let proposal = try ToriiParliamentProposalV1(validating: proposalData)
        guard case let .sccpRouteGovernance(payload) = proposal.kind,
              case let .register(register) = payload.anchor.action,
              case let .evm(governedDestination) = register.route.destination else {
            return XCTFail("expected SCCP route registration")
        }
        XCTAssertEqual(governedDestination.maxWrappedSupply, wrappedSupply)
        XCTAssertEqual(register.route.settlement.maxOutstandingLiability, liability)

        let draft = try ToriiParliamentAPIV1.attemptDraftRequestData(
            proposal: proposal,
            attemptSequence: 1
        )
        let draftText = String(decoding: draft, as: UTF8.self)
        XCTAssertTrue(draftText.contains("\"max_wrapped_supply\":\(wrappedSupply)"))
        XCTAssertTrue(draftText.contains("\"max_outstanding_liability\":\(liability)"))
        XCTAssertFalse(draftText.contains("\"max_wrapped_supply\":\""))
        XCTAssertThrowsError(try JSONEncoder().encode(proposal))

        for invalidCap in [
            "340282366920938463463374607431768211456",
            "18446744073709551616000000000.0",
            "018446744073709551616000000000",
        ] {
            let invalidRoute = sccpGovernedRouteJSON(
                routeConfigurationHash: configuration,
                maxWrappedSupply: invalidCap,
                maxOutstandingLiability: liability
            )
            XCTAssertThrowsError(
                try ToriiParliamentProposalV1(
                    validating: sccpRegisterProposalJSON(route: invalidRoute)
                )
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
            {"proposal_operator":"\(Self.governanceOwner)","contract_address":"\(Self.contractAddress)","code_hash_hex":"\(String(repeating: "11", count: 32))","abi_hash_hex":"\(String(repeating: "22", count: 32))","abi_version":"1","manifest_provenance":null}
            """
        )
        let missingProvenance = proposalKindJSON(
            kind: "DeployContract",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1}
            """
        )
        let missingOperator = proposalKindJSON(
            kind: "DeployContract",
            payload: """
            {"contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}
            """
        )
        let malformedOperator = proposalKindJSON(
            kind: "ContractLifecycleGovernance",
            payload: """
            {"proposal_operator":"not-an-account","contract_address":"\(Self.contractAddress)","expected_revision":1,"action":{"action":"AcceptParliamentOwnership","payload":null}}
            """
        )
        let runtimeWithImplicitDefaults = proposalKindJSON(
            kind: "RuntimeUpgrade",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","manifest":{"name":"runtime-v1","description":"upgrade","abi_version":1,"abi_hash":\(fixedBytes(3)),"added_syscalls":[],"added_pointer_types":[],"start_height":10,"end_height":20}}
            """
        )
        let runtimeBeyondExactJSON = proposalKindJSON(
            kind: "RuntimeUpgrade",
            payload: """
            {"proposal_operator":"\(Self.governanceOwner)","manifest":{"name":"runtime-v1","description":"upgrade","abi_version":1,"abi_hash":\(fixedBytes(3)),"added_syscalls":[],"added_pointer_types":[],"start_height":9007199254740992,"end_height":9007199254740993,"sbom_digests":[],"slsa_attestation":"","provenance":[]}}
            """
        )
        for json in [
            unknown,
            oldSingleKey,
            legacyDeploy,
            missingProvenance,
            missingOperator,
            malformedOperator,
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
        {"kind":"DeployContract","payload":{"proposal_operator":"\(Self.governanceOwner)","contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}}
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
