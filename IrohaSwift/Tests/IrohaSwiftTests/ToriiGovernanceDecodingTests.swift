import CryptoKit
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

    private func kagemushaSigner(_ seed: UInt8) throws -> String {
        let key = try Curve25519.Signing.PrivateKey(
            rawRepresentation: Data(repeating: seed, count: 32)
        ).publicKey.rawRepresentation
        return CanonicalNorito.publicKeyMultihash(algorithm: .ed25519, payload: key)
    }

    private func kagemushaPolicyInstallJSON(
        proposalOperator: String,
        networkId: String,
        predecessor: String,
        authoritySetId: String,
        threshold: Int,
        signers: [String]
    ) -> Data {
        let signersJSON = signers.map { "\"\($0)\"" }.joined(separator: ",")
        return proposalKindJSON(
            kind: "KagemushaVerifierPolicyInstall",
            payload: """
            {"proposal_operator":"\(proposalOperator)","network_id":"\(networkId)","expected_predecessor":\(predecessor),"authority_policy":{"version":1,"authority_set_id":\(authoritySetId),"threshold":\(threshold),"authorized_signers":[\(signersJSON)]}}
            """
        )
    }

    private func proposalKindJSON(kind: String, payload: String) -> Data {
        Data("{\"kind\":\"\(kind)\",\"payload\":\(payload)}".utf8)
    }

    private func sccpGovernanceProposalJSON() -> String {
        """
        {"network_id":"\(TestNetworkIds.canonical.literal)","base_revisions":[{"subject":{"subject":"route","key":{"network":"bsc_mainnet","profile":null}},"revision":1}],"actions":[{"action":"remove_staged","payload":{"network":{"network":"bsc_mainnet","profile":null},"revision":1}}]}
        """
    }

    private func payoutBindingJSON() -> String {
        let recipients = Array(Self.payoutAccounts.dropFirst()).map {
            "{\"account_id\":\"\($0)\",\"share\":\"0.25\"}"
        }.joined(separator: ",")
        return """
        {"contract_address":"\(Self.contractAddress)","code_hash":\(fixedBytes(7)),"entrypoint":"autonomous_validation_fee_tick","treasury_account_id":"\(Self.governanceOwner)","ds_asset_id":"5dHF5UNffENuEg9mhjYwY1jcZ1K5","xor_asset_id":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM","pool_vault_account_id":"\(Self.payoutAccounts[0])","batch_ds":"10","min_xor_out":"4","max_xor_out":"100","recipients":[\(recipients)]}
        """
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

    func testGovernanceProposalKindDecodesExistingV1Variants() throws {
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

        let sccp = proposalKindJSON(
            kind: "SccpRouteGovernance",
            payload: "{\"proposal\":\(sccpGovernanceProposalJSON())}"
        )
        guard case .sccpRouteGovernance(let sccpPayload) = try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: sccp
        ), case .object(let sccpProposal) = sccpPayload.proposal else {
            return XCTFail("expected SccpRouteGovernance proposal")
        }
        XCTAssertEqual(sccpProposal["network_id"], .string(TestNetworkIds.canonical.literal))

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

        let signers = try [kagemushaSigner(1), kagemushaSigner(2)].sorted()
        let kagemusha = kagemushaPolicyInstallJSON(
            proposalOperator: Self.governanceOwner,
            networkId: TestNetworkIds.canonical.literal,
            predecessor: "{\"version\":1,\"authority_policy\":null,\"active_release_id\":null,\"releases\":[]}",
            authoritySetId: fixedBytes(7), threshold: 2, signers: signers
        )
        guard case .kagemushaVerifierPolicyInstall(let kagemushaPayload) =
            try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: kagemusha) else {
            return XCTFail("expected KagemushaVerifierPolicyInstall")
        }
        XCTAssertEqual(kagemushaPayload.proposalOperator, Self.governanceOwner)
        XCTAssertEqual(kagemushaPayload.networkId, TestNetworkIds.canonical)
        XCTAssertEqual(kagemushaPayload.expectedPredecessor.version, 1)
        XCTAssertEqual(kagemushaPayload.authorityPolicy.authoritySetId, Data(repeating: 7, count: 32))
        XCTAssertEqual(kagemushaPayload.authorityPolicy.threshold, 2)
        XCTAssertEqual(kagemushaPayload.authorityPolicy.authorizedSigners, signers)
        XCTAssertNoThrow(try ToriiParliamentProposalV1(validating: kagemusha))
    }

    func testKagemushaPolicyInstallRejectsNoncanonicalOrNoninitialPayloads() throws {
        let ordered = try [kagemushaSigner(1), kagemushaSigner(2)].sorted()
        let canonicalEmpty = "{\"version\":1,\"authority_policy\":null,\"active_release_id\":null,\"releases\":[]}"
        let valid = kagemushaPolicyInstallJSON(
            proposalOperator: Self.governanceOwner,
            networkId: TestNetworkIds.canonical.literal,
            predecessor: canonicalEmpty,
            authoritySetId: fixedBytes(7), threshold: 1, signers: ordered
        )
        XCTAssertNoThrow(try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: valid))

        let predecessors = [
            "{\"version\":2,\"authority_policy\":null,\"active_release_id\":null,\"releases\":[]}",
            "{\"version\":1,\"authority_policy\":null,\"active_release_id\":null,\"releases\":[{}]}",
            "{\"version\":1,\"authority_policy\":{},\"active_release_id\":null,\"releases\":[]}",
            "{\"version\":1,\"authority_policy\":null,\"active_release_id\":\(fixedBytes(7)),\"releases\":[]}",
            "{\"version\":1,\"authority_policy\":null,\"releases\":[]}",
            "{\"version\":1,\"authority_policy\":null,\"active_release_id\":null,\"releases\":[],\"retired\":null}",
        ]
        for predecessor in predecessors {
            let malformed = kagemushaPolicyInstallJSON(
                proposalOperator: Self.governanceOwner,
                networkId: TestNetworkIds.canonical.literal,
                predecessor: predecessor,
                authoritySetId: fixedBytes(7), threshold: 1, signers: ordered
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: malformed)
            )
        }

        let policies: [(String, Int, [String])] = [
            (fixedBytes(0), 1, ordered),
            (fixedBytes(7, count: 31), 1, ordered),
            (fixedBytes(7), 0, ordered),
            (fixedBytes(7), 3, ordered),
            (fixedBytes(7), 1, []),
            (fixedBytes(7), 1, Array(repeating: ordered[0], count: 33)),
            (fixedBytes(7), 1, [ordered[0], ordered[0]]),
            (fixedBytes(7), 1, Array(ordered.reversed())),
            (fixedBytes(7), 1, [ordered[0].lowercased()]),
            (fixedBytes(7), 1, ["ed25519:\(ordered[0])"]),
            (fixedBytes(7), 1, ["ed0120" + String(repeating: "00", count: 32)]),
            (fixedBytes(7), 1, ["ed0121" + String(repeating: "11", count: 32)]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .secp256k1, payload: Data(repeating: 1, count: 32)
            )]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .blsNormal, payload: Data(repeating: 1, count: 47)
            )]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .blsSmall, payload: Data(repeating: 1, count: 95)
            )]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .mlDsa, payload: Data(repeating: 1, count: 1_951)
            )]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .gost2012_256A, payload: Data(repeating: 1, count: 63)
            )]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .sm2, payload: Data(repeating: 1, count: 64)
            )]),
            (fixedBytes(7), 1, [CanonicalNorito.publicKeyMultihash(
                algorithm: .sm2, payload: Data([0x04] + Array(repeating: 1, count: 64))
            )]),
        ]
        for (authoritySetId, threshold, signers) in policies {
            let malformed = kagemushaPolicyInstallJSON(
                proposalOperator: Self.governanceOwner,
                networkId: TestNetworkIds.canonical.literal,
                predecessor: canonicalEmpty,
                authoritySetId: authoritySetId, threshold: threshold, signers: signers
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: malformed)
            )
        }

        for (operatorId, networkId) in [
            ("alice@wonderland", TestNetworkIds.canonical.literal),
            (Self.governanceOwner, "sora-taira"),
        ] {
            let malformed = kagemushaPolicyInstallJSON(
                proposalOperator: operatorId, networkId: networkId,
                predecessor: canonicalEmpty,
                authoritySetId: fixedBytes(7), threshold: 1, signers: ordered
            )
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: malformed)
            )
        }

        let validText = String(decoding: valid, as: UTF8.self)
        for malformed in [
            validText.replacingOccurrences(
                of: "\"authority_policy\":{\"version\":1",
                with: "\"authority_policy\":{\"version\":2"
            ),
            validText.replacingOccurrences(
                of: "\"threshold\":1",
                with: "\"threshold\":1,\"retired\":true"
            ),
            validText.replacingOccurrences(
                of: ",\"expected_predecessor\":\(canonicalEmpty)",
                with: ""
            ),
            validText.replacingOccurrences(
                of: "\"network_id\":",
                with: "\"retired\":null,\"network_id\":"
            ),
        ] {
            XCTAssertNotEqual(malformed, validText)
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceProposalKind.self,
                    from: Data(malformed.utf8)
                )
            )
        }
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

    func testSccpRouteGovernancePayloadIsAnExactProposalWrapper() throws {
        let proposal = sccpGovernanceProposalJSON()
        XCTAssertNoThrow(
            try ToriiParliamentProposalV1(
                validating: proposalKindJSON(
                    kind: "SccpRouteGovernance",
                    payload: "{\"proposal\":\(proposal)}"
                )
            )
        )
        for payload in [
            "{}",
            "{\"proposal\":\(proposal),\"network_id\":\"\(TestNetworkIds.canonical.literal)\"}",
            "{\"anchor\":\(proposal)}",
            "{\"proposal\":[\(proposal)]}",
            "{\"proposal\":null}",
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiGovernanceProposalKind.self,
                    from: proposalKindJSON(kind: "SccpRouteGovernance", payload: payload)
                ),
                payload
            )
        }
    }

    private static let sccpBscRouteSubject =
        #"{"subject":"route","key":{"network":"bsc_mainnet","profile":null}}"#
    private static let sccpRemoveStagedAction =
        #"{"action":"remove_staged","payload":{"network":{"network":"bsc_mainnet","profile":null},"revision":1}}"#

    /// One `SccpGovernanceProposalV1` body with a single base revision unless overridden.
    private func sccpProposalBody(
        networkId: String? = nil,
        subject: String = ToriiGovernanceDecodingTests.sccpBscRouteSubject,
        revision: String = "1",
        baseRevisions: String? = nil,
        actions: String? = nil,
        extra: String = ""
    ) -> String {
        let network = networkId ?? "\"\(TestNetworkIds.canonical.literal)\""
        let base = baseRevisions ?? "[{\"subject\":\(subject),\"revision\":\(revision)}]"
        let actionList = actions ?? "[\(Self.sccpRemoveStagedAction)]"
        return "{\"network_id\":\(network),\"base_revisions\":\(base),\"actions\":\(actionList)\(extra)}"
    }

    private func decodeSccpRouteProposal(_ proposal: String) throws -> ToriiGovernanceProposalKind {
        try JSONDecoder().decode(
            ToriiGovernanceProposalKind.self,
            from: proposalKindJSON(
                kind: "SccpRouteGovernance",
                payload: "{\"proposal\":\(proposal)}"
            )
        )
    }

    func testSccpRouteGovernanceChecksTheClosedProposalEnvelope() throws {
        let bscRoute = Self.sccpBscRouteSubject
        let removeStaged = Self.sccpRemoveStagedAction
        let blsValidator = "ea0130" + String(repeating: "A1", count: 48)

        // Only the envelope is checked here; whether `base_revisions` lists exactly the
        // subjects the actions touch stays with Torii until typed action decoding lands.
        for accepted in [
            sccpProposalBody(),
            sccpProposalBody(subject: #"{"subject":"route_control","key":{"network":"ethereum_mainnet","profile":null}}"#),
            sccpProposalBody(subject: #"{"subject":"light_client","key":{"network":"ton_mainnet","profile":null}}"#),
            sccpProposalBody(subject: #"{"subject":"parameters","key":null}"#, revision: "0"),
            sccpProposalBody(subject: "{\"subject\":\"bridge_key_fault\",\"key\":\"\(blsValidator)\"}"),
            sccpProposalBody(actions: "[" + Array(repeating: removeStaged, count: 16).joined(separator: ",") + "]"),
        ] {
            XCTAssertNoThrow(try decodeSccpRouteProposal(accepted), accepted)
        }
        let tronLightClient = sccpProposalBody(
            subject: #"{"subject":"light_client","key":{"network":"tron_mainnet","profile":null}}"#
        )
        XCTAssertNoThrow(
            try ToriiParliamentProposalV1(
                validating: proposalKindJSON(
                    kind: "SccpRouteGovernance",
                    payload: "{\"proposal\":\(tronLightClient)}"
                )
            )
        )

        for rejected in [
            "{}",
            sccpProposalBody(extra: ",\"memo\":null"),
            sccpProposalBody(networkId: "\"sora\""),
            sccpProposalBody(networkId: "1"),
            sccpProposalBody(baseRevisions: "[]"),
            sccpProposalBody(baseRevisions: "[" + Array(repeating: "{\"subject\":\(bscRoute),\"revision\":1}", count: 17)
                .joined(separator: ",") + "]"),
            sccpProposalBody(baseRevisions: "[{\"subject\":\(bscRoute),\"revision\":1,\"extra\":0}]"),
            sccpProposalBody(revision: "\"1\""),
            sccpProposalBody(revision: "9007199254740992"),
            sccpProposalBody(subject: #"{"subject":"unknown","key":null}"#),
            sccpProposalBody(subject: #"{"subject":"parameters","key":{"network":"bsc_mainnet","profile":null}}"#),
            sccpProposalBody(subject: #"{"subject":"route","key":{"network":"sora","profile":null}}"#),
            sccpProposalBody(subject: #"{"subject":"route","key":{"network":"solana_mainnet","profile":null}}"#),
            sccpProposalBody(subject: #"{"subject":"route","key":{"network":"bsc_mainnet","profile":"default"}}"#),
            sccpProposalBody(subject: #"{"subject":"route","key":{"network":"bsc_mainnet"}}"#),
            sccpProposalBody(subject: #"{"subject":"route","key":null}"#),
            sccpProposalBody(subject: "{\"subject\":\"bridge_key_fault\",\"key\":\"\(blsValidator.lowercased())\"}"),
            sccpProposalBody(subject: #"{"subject":"bridge_key_fault","key":"ea0130"}"#),
            sccpProposalBody(actions: "[]"),
            sccpProposalBody(actions: "[" + Array(repeating: removeStaged, count: 17).joined(separator: ",") + "]"),
            sccpProposalBody(actions: #"[{"action":"unknown","payload":{}}]"#),
            sccpProposalBody(actions: #"[{"action":"remove_staged","payload":[]}]"#),
            sccpProposalBody(actions: #"[{"action":"remove_staged","payload":{},"memo":null}]"#),
        ] {
            XCTAssertThrowsError(try decodeSccpRouteProposal(rejected), rejected)
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
        let musubi = proposalKindJSON(
            kind: "MusubiRegistryGovernance",
            payload: "{\"kind\":\"Unknown\",\"value\":{}}"
        )
        let sorafs = proposalKindJSON(
            kind: "SorafsProviderGovernance",
            payload: "{\"action\":{\"action\":\"replace\",\"value\":{}}}"
        )
        for json in [musubi, sorafs] {
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

    private func kagemushaReleaseInstallFixture() throws -> Data {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            root.deleteLastPathComponent()
        }
        return try Data(contentsOf: root.appendingPathComponent(
            "fixtures/governance/kagemusha_verifier_release_install_v1.json"
        ))
    }

    private func mutatedKagemushaReleaseInstallFixture(
        _ mutation: (inout [String: Any]) -> Void
    ) throws -> Data {
        var fixture = try XCTUnwrap(
            JSONSerialization.jsonObject(with: kagemushaReleaseInstallFixture()) as? [String: Any]
        )
        mutation(&fixture)
        return try JSONSerialization.data(withJSONObject: fixture, options: [.sortedKeys])
    }

    func testKagemushaHardwareCapabilityMaskUsesCompleteUInt32Range() throws {
        let fixture = try XCTUnwrap(
            JSONSerialization.jsonObject(with: kagemushaReleaseInstallFixture()) as? [String: Any])
        let payload = try XCTUnwrap(fixture["payload"] as? [String: Any])
        let manifest = try XCTUnwrap(payload["manifest"] as? [String: Any])
        let profiles = try XCTUnwrap(manifest["enabled_profiles"] as? [[String: Any]])
        var hardware = try XCTUnwrap(profiles.first?["hardware_profile"] as? [String: Any])
        hardware["capability_mask"] = UInt64(UInt32.max)
        let encoded = try JSONSerialization.data(withJSONObject: hardware)
        let decoded = try JSONDecoder().decode(
            ToriiGovernanceKagemushaHardwareProfileV1.self, from: encoded)
        XCTAssertEqual(decoded.capabilityMask, UInt32.max)
        hardware["capability_mask"] = UInt64(UInt32.max) + 1
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiGovernanceKagemushaHardwareProfileV1.self,
            from: JSONSerialization.data(withJSONObject: hardware)))
    }

    func testKagemushaReleaseInstallDecodesExactFixture() throws {
        let fixture = try kagemushaReleaseInstallFixture()
        let proposal = try ToriiParliamentProposalV1(validating: fixture)
        guard case .kagemushaVerifierReleaseInstall(let release) = proposal.kind else {
            return XCTFail("expected KagemushaVerifierReleaseInstall")
        }
        XCTAssertNotNil(release.expectedPredecessor.authorityPolicy)
        XCTAssertNil(release.expectedPredecessor.activeReleaseId)
        XCTAssertTrue(release.expectedPredecessor.releases.isEmpty)
        XCTAssertEqual(release.manifest.version, 1)
        XCTAssertEqual(release.manifest.networkId, release.networkId)
        XCTAssertEqual(release.manifest.purpose, .production)
        XCTAssertEqual(release.manifest.releaseId.bytes.count, 32)
        XCTAssertEqual(release.manifest.helperProtocols.count, 6)
        XCTAssertEqual(release.manifest.enabledProfiles.count, 2)
        XCTAssertEqual(release.manifest.artifacts.count, 50)
        XCTAssertEqual(release.receipt.profileQualifications.count, 2)
        XCTAssertEqual(release.receipt.fuzzCases, 10_000_000)
        XCTAssertEqual(release.attestation.approvals.count, 2)
        XCTAssertEqual(release.manifest.releaseId, release.attestation.subject.releaseId)
    }

    func testKagemushaReleasePurposeUsesClosedTaggedShapes() throws {
        let decoder = JSONDecoder()
        let production = Data(#"{"kind":"production","value":null}"#.utf8)
        XCTAssertEqual(
            try decoder.decode(ToriiGovernanceKagemushaReleasePurposeV1.self, from: production),
            .production
        )
        let scope = "{\"asset_identity_digest\":\(fixedBytes(1)),\"asset_incarnation\":\(fixedBytes(2)),\"asset_scale\":28,\"liability_pool_id\":\(fixedBytes(3))}"
        let experiment = Data("{\"kind\":\"testnet_experiment\",\"value\":\(scope)}".utf8)
        guard case .testnetExperiment(let parsed) = try decoder.decode(
            ToriiGovernanceKagemushaReleasePurposeV1.self, from: experiment
        ) else { return XCTFail("expected scoped testnet experiment") }
        XCTAssertEqual(parsed.assetScale, 28)
        for invalid in [
            #"{"kind":"production"}"#,
            #"{"kind":"production","value":{}}"#,
            #"{"kind":"unknown","value":null}"#,
            #"{"kind":"testnet_experiment","value":null}"#,
            String(decoding: experiment, as: UTF8.self).replacingOccurrences(
                of: "\"asset_scale\":28", with: "\"asset_scale\":29"),
            String(decoding: experiment, as: UTF8.self).replacingOccurrences(
                of: "\"asset_scale\":28", with: "\"asset_scale\":28,\"retired\":null"),
        ] {
            XCTAssertThrowsError(try decoder.decode(
                ToriiGovernanceKagemushaReleasePurposeV1.self, from: Data(invalid.utf8)))
        }
    }

    func testKagemushaReleaseInstallRejectsNestedSchemaMutations() throws {
        let unknownNestedField = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var manifest = payload["manifest"] as! [String: Any]
            manifest["retired"] = true
            payload["manifest"] = manifest
            fixture["payload"] = payload
        }
        let missingRequiredField = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var manifest = payload["manifest"] as! [String: Any]
            manifest.removeValue(forKey: "halo2_k")
            payload["manifest"] = manifest
            fixture["payload"] = payload
        }
        let shortDigest = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var receipt = payload["receipt"] as! [String: Any]
            receipt["source_tree_digest"] = Array(repeating: 1, count: 31)
            payload["receipt"] = receipt
            fixture["payload"] = payload
        }
        let retiredArtifactRole = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var manifest = payload["manifest"] as! [String: Any]
            var artifacts = manifest["artifacts"] as! [[String: Any]]
            var role = artifacts[0]["role"] as! [String: Any]
            role["role"] = "retired_artifact"
            artifacts[0]["role"] = role
            manifest["artifacts"] = artifacts
            payload["manifest"] = manifest
            fixture["payload"] = payload
        }
        let missingUnitValue = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var manifest = payload["manifest"] as! [String: Any]
            var helpers = manifest["helper_protocols"] as! [[String: Any]]
            var helper = helpers[0]["helper"] as! [String: Any]
            helper.removeValue(forKey: "value")
            helpers[0]["helper"] = helper
            manifest["helper_protocols"] = helpers
            payload["manifest"] = manifest
            fixture["payload"] = payload
        }
        let ungovernedPredecessor = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var predecessor = payload["expected_predecessor"] as! [String: Any]
            predecessor["authority_policy"] = NSNull()
            payload["expected_predecessor"] = predecessor
            fixture["payload"] = payload
        }
        let inexactNumber = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var receipt = payload["receipt"] as! [String: Any]
            receipt["fuzz_cases"] = 9_007_199_254_740_992 as UInt64
            payload["receipt"] = receipt
            fixture["payload"] = payload
        }
        let invalidP256Key = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var manifest = payload["manifest"] as! [String: Any]
            var profiles = manifest["enabled_profiles"] as! [[String: Any]]
            var profile = profiles[0]["hardware_profile"] as! [String: Any]
            profile["governance_credential_public_key"] = ["04" + String(repeating: "00", count: 64)]
            profiles[0]["hardware_profile"] = profile
            manifest["enabled_profiles"] = profiles
            payload["manifest"] = manifest
            fixture["payload"] = payload
        }
        let highSSignature = try mutatedKagemushaReleaseInstallFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var receipt = payload["receipt"] as! [String: Any]
            var providers = receipt["provider_policy"] as! [[String: Any]]
            let original = (providers[0]["issuer_signature"] as! [String])[0]
            providers[0]["issuer_signature"] = [
                String(original.prefix(64))
                    + "FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632550"
            ]
            receipt["provider_policy"] = providers
            payload["receipt"] = receipt
            fixture["payload"] = payload
        }
        for (name, data) in [
            ("unknown nested field", unknownNestedField),
            ("missing required field", missingRequiredField),
            ("short digest", shortDigest),
            ("retired artifact role", retiredArtifactRole),
            ("missing unit value", missingUnitValue),
            ("ungoverned predecessor", ungovernedPredecessor),
            ("inexact integer", inexactNumber),
            ("invalid P-256 point", invalidP256Key),
            ("high-S P-256 signature", highSSignature),
        ] {
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: data), name)
        }
    }

    private func kagemushaReleaseActivateFixture() throws -> Data {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            root.deleteLastPathComponent()
        }
        return try Data(contentsOf: root.appendingPathComponent(
            "fixtures/governance/kagemusha_verifier_release_activate_v1.json"
        ))
    }

    private func mutatedKagemushaReleaseActivateFixture(
        _ mutation: (inout [String: Any]) -> Void
    ) throws -> Data {
        var fixture = try XCTUnwrap(
            JSONSerialization.jsonObject(with: kagemushaReleaseActivateFixture()) as? [String: Any]
        )
        mutation(&fixture)
        return try JSONSerialization.data(withJSONObject: fixture, options: [.sortedKeys])
    }

    func testKagemushaReleaseActivateDecodesExactFixture() throws {
        let proposal = try ToriiParliamentProposalV1(validating: kagemushaReleaseActivateFixture())
        guard case .kagemushaVerifierReleaseActivate(let activation) = proposal.kind else {
            return XCTFail("expected KagemushaVerifierReleaseActivate")
        }
        XCTAssertNotNil(activation.expectedPredecessor.authorityPolicy)
        XCTAssertNil(activation.expectedPredecessor.activeReleaseId)
        XCTAssertEqual(activation.expectedPredecessor.releases.count, 1)
        XCTAssertEqual(activation.expectedPredecessor.releases[0].status, .standby)
        XCTAssertEqual(
            activation.successorReleaseId,
            activation.expectedPredecessor.releases[0].releaseId
        )
    }

    func testKagemushaReleaseActivateRejectsMalformedAndStaleTargets() throws {
        let extraPayloadField = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            payload["retired"] = true
            fixture["payload"] = payload
        }
        let shortSuccessor = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            payload["successor_release_id"] = Array(repeating: 1, count: 31)
            fixture["payload"] = payload
        }
        let missingSuccessor = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            payload.removeValue(forKey: "successor_release_id")
            fixture["payload"] = payload
        }
        let ungoverned = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var predecessor = payload["expected_predecessor"] as! [String: Any]
            predecessor["authority_policy"] = NSNull()
            payload["expected_predecessor"] = predecessor
            fixture["payload"] = payload
        }
        let alreadyActive = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var predecessor = payload["expected_predecessor"] as! [String: Any]
            predecessor["active_release_id"] = String(repeating: "AA", count: 32)
            payload["expected_predecessor"] = predecessor
            fixture["payload"] = payload
        }
        let unknownTarget = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            payload["successor_release_id"] = Array(repeating: 99, count: 32)
            fixture["payload"] = payload
        }
        let nonstandbyTarget = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var predecessor = payload["expected_predecessor"] as! [String: Any]
            var releases = predecessor["releases"] as! [[String: Any]]
            releases[0]["status"] = 3
            predecessor["releases"] = releases
            payload["expected_predecessor"] = predecessor
            fixture["payload"] = payload
        }
        let secondStandby = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var predecessor = payload["expected_predecessor"] as! [String: Any]
            var releases = predecessor["releases"] as! [[String: Any]]
            var second = releases[0]
            second["release_id"] = Array(repeating: 255, count: 32)
            releases.append(second)
            predecessor["releases"] = releases
            payload["expected_predecessor"] = predecessor
            fixture["payload"] = payload
        }
        let extraNestedField = try mutatedKagemushaReleaseActivateFixture { fixture in
            var payload = fixture["payload"] as! [String: Any]
            var predecessor = payload["expected_predecessor"] as! [String: Any]
            var releases = predecessor["releases"] as! [[String: Any]]
            releases[0]["legacy_alias"] = "retired"
            predecessor["releases"] = releases
            payload["expected_predecessor"] = predecessor
            fixture["payload"] = payload
        }
        for (name, data) in [
            ("unknown payload field", extraPayloadField),
            ("short successor", shortSuccessor),
            ("missing successor", missingSuccessor),
            ("ungoverned predecessor", ungoverned),
            ("already active", alreadyActive),
            ("unknown target", unknownTarget),
            ("nonstandby target", nonstandbyTarget),
            ("second standby", secondStandby),
            ("unknown release field", extraNestedField),
        ] {
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: data), name)
        }
    }
}
