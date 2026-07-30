import Foundation
import XCTest
@testable import IrohaSwift

final class AliasSetupV1Tests: XCTestCase {
    func testAccountAliasIntentPreservesTairaTargetAccount() throws {
        let target = try AccountAddress
            .fromAccount(publicKey: Data(repeating: 0x17, count: 32))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
        let intent = try AliasAccountIntentV1(
            alias: ResolvedAccountAliasV1(
                canonicalName: "merchant@paynet",
                dataspaceId: 7
            ),
            targetAccount: target,
            provision: .existing,
            role: .primary
        )

        XCTAssertEqual(intent.targetAccount, target)
        XCTAssertThrowsError(try AliasAccountIntentV1(
            alias: intent.alias,
            targetAccount: " \(target)",
            provision: .existing,
            role: .primary
        ))
    }

    func testSharedCatalogFreeNameCases() throws {
        let fixture = try loadSharedFixture()
        for testCase in fixture.accountAliasCases {
            let parsed = try AccountAliasName(parsing: testCase.input)
            XCTAssertEqual(parsed.canonicalText, testCase.canonical)
            XCTAssertEqual(parsed.label, testCase.label)
            XCTAssertEqual(parsed.domain, testCase.domain)
            XCTAssertEqual(parsed.dataspace, testCase.dataspace)
        }

        XCTAssertEqual(
            try AccountAliasName(parsing: "merchant@例え").canonicalText,
            "merchant@xn--r8jz45g"
        )
        for invalid in ["", " merchant@paynet", "merchant", "merchant@", "@paynet", "a@b.c.d", "merchant@Ḁ"] {
            XCTAssertThrowsError(try AccountAliasName(parsing: invalid), invalid)
        }
    }

    func testSharedPlanHashAndInstructionFrame() throws {
        let fixture = try loadSharedFixture()
        let setupVector = try XCTUnwrap(
            fixture.planHashVectors.first { $0.name == "setup_account_alias_create" }
        )
        let bodyBytes = try XCTUnwrap(Data(hexString: setupVector.canonicalBodyNoritoHex))
        XCTAssertEqual(
            AliasPlanVerifier.canonicalHash(canonicalBodyNorito: bodyBytes).hexEncodedString(),
            setupVector.canonicalPlanHashHex
        )

        let alias = try ResolvedAccountAliasV1(canonicalName: "merchant@banka.paynet", dataspaceId: 7)
        let fixtureKeypair = try Keypair(privateKeyBytes: Data(repeating: 0xC1, count: 32))
        let authority = try AccountId.makeI105(publicKey: fixtureKeypair.publicKey)
        let intent = AliasIntentV1.accountAlias(
            try AliasAccountIntentV1(
                alias: alias,
                targetAccount: authority,
                provision: .create,
                role: .primary
            )
        )
        let ensureVector = try XCTUnwrap(
            fixture.instructionFrameVectors.first { $0.name == "ensure_account_alias" }
        )
        let frame = try AliasFramedInstructionV1(
            wireId: ensureVector.wireId,
            framedPayload: try XCTUnwrap(Data(hexString: ensureVector.framedPayloadHex))
        )
        let ensure = EnsureAlias(
            intent: intent,
            acquisition: try AliasLeaseAcquisitionV1(termYears: 1),
            quoteGuard: try AliasQuoteGuardV1(
                expectedPolicyVersion: 2,
                expectedPaymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
                maxAmount: "10",
                validUntilMs: 50_000
            )
        )
        let request = try AliasSetupPlanRequestV1(intents: [ensure])
        let plan = try AliasTransactionPlanV1(
            body: try AliasTransactionPlanBodyV1(
                version: AliasTransactionPlanBodyV1.version,
                authority: authority,
                chainId: "test-chain",
                anchor: try AliasPlanAnchorV1(
                    blockHeight: 9,
                    blockHash: String(repeating: "01", count: 32)
                ),
                resources: [
                    AliasPlanResourceV1(intent: intent, disposition: .repair, quote: nil, instructionIndex: 0)
                ],
                instructions: [frame],
                totalsByAsset: [],
                warnings: [],
                blockers: [],
                validUntilMs: 49_000
            ),
            planHash: setupVector.canonicalPlanHashHex
        )

        XCTAssertTrue(AliasPlanVerifier.verifyHash(plan, canonicalBodyNorito: bodyBytes))
        XCTAssertEqual(AliasPlanVerifier.validateExecutable(plan), [])
        XCTAssertTrue(AliasPlanVerifier.verifyExactFrames(plan) { _, payload in payload })
        XCTAssertFalse(AliasPlanVerifier.verifyExactFrames(plan) { _, payload in
            var changed = payload
            changed[changed.startIndex] ^= 1
            return changed
        })
        XCTAssertNoThrow(
            try AliasPlanVerifier.requireExecutable(
                plan,
                canonicalBodyNorito: bodyBytes,
                roundTrip: { _, payload in payload }
            )
        )
        XCTAssertNoThrow(
            try AliasPlanVerifier.requireExecutableForRequest(
                request,
                plan: plan,
                canonicalBodyNorito: bodyBytes,
                decodeAndReencode: { _, payload in
                    DecodedEnsureAliasFrame(
                        instruction: ensure,
                        reencodedFrame: payload
                    )
                }
            )
        )
        XCTAssertThrowsError(
            try AliasPlanVerifier.requireExecutableForRequest(
                request,
                plan: plan,
                canonicalBodyNorito: bodyBytes,
                codec: UnavailableAliasNoritoRegistryCodec()
            )
        )
        do {
            let decoded = try NativeAliasNoritoRegistryCodec.shared.decodeAndReencodeEnsureAlias(
                wireId: frame.wireId,
                framedPayload: frame.framedPayload
            )
            XCTAssertEqual(decoded.instruction, ensure)
            XCTAssertEqual(decoded.reencodedFrame, frame.framedPayload)
        } catch AliasNoritoRegistryCodecError.unavailable(let wireId) {
            #if IROHASWIFT_BRIDGE_REQUIRED
            throw AliasNoritoRegistryCodecError.unavailable(wireId: wireId)
            #else
            // Source-only test environments may not package the optional bridge symbol.
            #endif
        }
    }

    func testNativeAliasRegistryCodecRoundTripsEverySharedRustFrameOrFailsClosed() throws {
        let vectors = try loadSharedFixture().instructionFrameVectors
        let codec = NativeAliasNoritoRegistryCodec.shared
        var bridgeAvailable: Bool?
        for vector in vectors {
            let original = try XCTUnwrap(Data(hexString: vector.framedPayloadHex), vector.name)
            do {
                let reencoded = try codec.decodeAndReencodeFrame(
                    wireId: vector.wireId,
                    framedPayload: original
                )
                if bridgeAvailable == false {
                    XCTFail("native alias bridge availability changed within one process")
                }
                bridgeAvailable = true
                XCTAssertEqual(reencoded, original, vector.name)
            } catch AliasNoritoRegistryCodecError.unavailable(let wireId) {
                #if IROHASWIFT_BRIDGE_REQUIRED
                throw AliasNoritoRegistryCodecError.unavailable(wireId: wireId)
                #else
                if bridgeAvailable == true {
                    XCTFail("native alias bridge availability changed within one process")
                }
                bridgeAvailable = false
                #endif
            }
        }
    }

    func testEverySharedRustFrameAndLifecycleHash() throws {
        let fixture = try loadSharedFixture()
        let expectedWireIds = [
            "ensure_account_alias": EnsureAlias.wireId,
            "renew_account_alias": RenewAliasLease.wireId,
            "configure_auto_renew_enable": ConfigureAliasAutoRenew.wireId,
            "configure_auto_renew_disable": ConfigureAliasAutoRenew.wireId,
            "rebind_account_alias": RebindAccountAlias.wireId,
            "compare_and_set_primary_account_alias": CompareAndSetPrimaryAccountAlias.wireId,
        ]
        XCTAssertEqual(Set(fixture.instructionFrameVectors.map(\.name)), Set(expectedWireIds.keys))
        let unavailableCodec = UnavailableAliasNoritoRegistryCodec()
        for vector in fixture.instructionFrameVectors {
            XCTAssertEqual(vector.wireId, expectedWireIds[vector.name])
            let original = try XCTUnwrap(Data(hexString: vector.framedPayloadHex))
            let decoded = try XCTUnwrap(noritoDecodeFrame(original), vector.name)
            let reencoded = decoded.header.encode()
                + Data(repeating: 0, count: decoded.paddingLength)
                + decoded.payload
            XCTAssertEqual(reencoded, original, vector.name)
            XCTAssertThrowsError(
                try unavailableCodec.decodeAndReencodeFrame(
                    wireId: vector.wireId,
                    framedPayload: original
                )
            ) { error in
                XCTAssertEqual(
                    error as? AliasNoritoRegistryCodecError,
                    .unavailable(wireId: vector.wireId)
                )
            }
        }

        let expectedDomains = [
            "setup_account_alias_create": "iroha:alias-transaction-plan-body:v1\0",
            "renew_account_alias": "iroha:alias-lifecycle-transaction-plan-body:v1\0",
        ]
        XCTAssertEqual(Set(fixture.planHashVectors.map(\.name)), Set(expectedDomains.keys))
        for vector in fixture.planHashVectors {
            XCTAssertEqual(vector.domain, expectedDomains[vector.name], vector.name)
            let body = try XCTUnwrap(Data(hexString: vector.canonicalBodyNoritoHex), vector.name)
            let hash: Data
            switch vector.name {
            case "setup_account_alias_create":
                hash = AliasPlanVerifier.canonicalHash(canonicalBodyNorito: body)
            case "renew_account_alias":
                hash = AliasPlanVerifier.canonicalLifecycleHash(canonicalBodyNorito: body)
            default:
                return XCTFail("unexpected plan hash fixture \(vector.name)")
            }
            XCTAssertEqual(hash.hexEncodedString(), vector.canonicalPlanHashHex, vector.name)
            XCTAssertEqual(hash.last.map { $0 & 1 }, 1, vector.name)
        }
    }

    func testSharedBlockedReportRoundTripsAsTypedSecretFreeJSON() throws {
        let report = try loadSharedFixture().reportJSONVector
        XCTAssertEqual(report.status, .blocked)
        XCTAssertEqual(report.diagnostics.map(\.code), ["alias.catalog.mapping_conflict"])
        let encoded = try JSONEncoder().encode(report)
        XCTAssertEqual(try JSONDecoder().decode(AliasSetupReportV1.self, from: encoded), report)
        XCTAssertFalse(String(decoding: encoded, as: UTF8.self).contains("private_key"))
    }

    func testSharedRustGeneratedOnboardingReceiptBytesHashAndSignature() throws {
        let vector = try loadSharedFixture().accountOnboardingReceiptVector
        XCTAssertEqual(vector.name, "sponsored_account_alias_create")
        XCTAssertEqual(
            vector.domain,
            "iroha:account-onboarding-plan-receipt:v1\0"
        )
        let canonicalBody = try XCTUnwrap(
            Data(hexString: vector.canonicalBodyNoritoHex)
        )
        let canonicalHash = try ToriiAccountOnboardingReceiptVerifier.canonicalHash(
            canonicalBodyNorito: canonicalBody
        )
        XCTAssertEqual(canonicalHash.hexEncodedString(), vector.canonicalPlanHashHex)
        XCTAssertEqual(canonicalHash.last.map { $0 & 1 }, 1)
        XCTAssertEqual(
            vector.receiptJSON.planHash,
            try ToriiAccountOnboardingReceiptVerifier.canonicalHashLiteral(
                canonicalBodyNorito: canonicalBody
            )
        )
        XCTAssertEqual(vector.receiptJSON.body.authority, vector.authority)
        guard case let .string(signatureHex) = vector.receiptJSON.signature else {
            return XCTFail("shared onboarding signature must be canonical raw hex")
        }
        XCTAssertEqual(signatureHex, vector.signatureHex)

        XCTAssertNoThrow(
            try ToriiAccountOnboardingReceiptVerifier.verify(
                vector.receiptJSON,
                for: vector.receiptJSON.body.request,
                canonicalBodyNorito: canonicalBody,
                expectedAuthority: vector.authority,
                expectedChainId: vector.receiptJSON.body.chainId
            )
        )

        do {
            let bridgeBody = try ToriiAccountOnboardingPlanBodyNorito.encode(
                vector.receiptJSON.body
            )
            XCTAssertEqual(bridgeBody, canonicalBody)
            XCTAssertNoThrow(
                try ToriiAccountOnboardingReceiptVerifier.verify(
                    vector.receiptJSON,
                    for: vector.receiptJSON.body.request,
                    expectedAuthority: vector.authority,
                    expectedChainId: vector.receiptJSON.body.chainId,
                    bodyEncoder: ToriiAccountOnboardingPlanBodyNorito.encode
                )
            )
        } catch ToriiAccountOnboardingReceiptVerificationError
            .canonicalBodyEncodingUnavailable {
            #if IROHASWIFT_BRIDGE_REQUIRED
            throw ToriiAccountOnboardingReceiptVerificationError
                .canonicalBodyEncodingUnavailable
            #else
            // Source-only test environments may not package the optional bridge symbol.
            #endif
        }
    }

    func testSharedResolvedNamesQuoteGuardAndExactPermissionRoundTrip() throws {
        let fixture = try loadSharedFixture()
        XCTAssertEqual(
            fixture.resolvedNameJSONVectors.dataspace,
            try ResolvedDataSpaceV1(canonicalName: "paynet", dataspaceId: 7)
        )
        XCTAssertEqual(
            fixture.resolvedNameJSONVectors.domain,
            try ResolvedDomainV1(canonicalName: "banka.paynet", dataspaceId: 7)
        )
        let alias = try ResolvedAccountAliasV1(
            canonicalName: "merchant@banka.paynet",
            dataspaceId: 7
        )
        XCTAssertEqual(fixture.resolvedNameJSONVectors.accountAlias, alias)
        XCTAssertEqual(
            fixture.quoteGuardJSONVector,
            try AliasQuoteGuardV1(
                expectedPolicyVersion: 2,
                expectedPaymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
                maxAmount: "10",
                validUntilMs: 50_000
            )
        )
        XCTAssertEqual(fixture.permissionScopeJSONVector, .alias(alias))
        let permissionJSON = try JSONEncoder().encode(fixture.permissionScopeJSONVector)
        XCTAssertEqual(
            try JSONDecoder().decode(AccountAliasPermissionScope.self, from: permissionJSON),
            fixture.permissionScopeJSONVector
        )
    }

    func testLifecycleBuildersNeverCarryLeaseExpiryInBindingChanges() throws {
        let alias = try ResolvedAccountAliasV1(canonicalName: "merchant@paynet", dataspaceId: 7)
        let oldAccount = try AccountId.makeI105(publicKey: Data(repeating: 0x21, count: 32))
        let newAccount = try AccountId.makeI105(publicKey: Data(repeating: 0x22, count: 32))
        let rebind = try RebindAccountAlias(
            alias: alias,
            expectedTargetAccount: oldAccount,
            newTargetAccount: newAccount
        )
        let encoded = try JSONEncoder().encode(rebind)
        let object = try XCTUnwrap(JSONSerialization.jsonObject(with: encoded) as? [String: Any])
        XCTAssertNil(object["lease_expiry_ms"])
        XCTAssertEqual(RebindAccountAlias.wireId, "iroha.account.alias.rebind")

        let disabled = ConfigureAliasAutoRenew(
            target: .accountAlias(alias),
            expectedRevision: 4,
            config: nil
        )
        let disabledObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(disabled)) as? [String: Any]
        )
        XCTAssertTrue(disabledObject["config"] is NSNull)
    }

    func testLifecyclePlanVerifiesRenewalAndExactAutoRenewNoOp() throws {
        let authority = try AccountId.makeI105(publicKey: Data(repeating: 0x31, count: 32))
        let alias = try ResolvedAccountAliasV1(canonicalName: "merchant@paynet", dataspaceId: 7)
        let guardValue = try AliasQuoteGuardV1(
            expectedPolicyVersion: 2,
            expectedPaymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
            maxAmount: "10",
            validUntilMs: 50_000
        )
        let renewal = try RenewAliasLease(
            target: .accountAlias(alias),
            expectedCurrentExpiryMs: 1_000,
            targetExpiryMs: 2_000,
            quoteGuard: guardValue
        )
        let frame = try AliasFramedInstructionV1(
            wireId: RenewAliasLease.wireId,
            framedPayload: Data([0x4e, 0x52, 0x54, 0x30])
        )
        let quote = try AliasLeaseQuoteV1(
            target: renewal.target,
            pricingClass: 1,
            exactAmount: "5",
            quoteGuard: guardValue,
            expiresAtMs: renewal.targetExpiryMs,
            graceExpiresAtMs: 3_000,
            redemptionExpiresAtMs: 4_000
        )
        let renewalBodyBytes = Data([1, 2, 3, 4])
        let renewalPlan = try AliasLifecycleTransactionPlanV1(
            body: try AliasLifecycleTransactionPlanBodyV1(
                authority: authority,
                chainId: "test-chain",
                anchor: try AliasPlanAnchorV1(
                    blockHeight: 9,
                    blockHash: String(repeating: "01", count: 32)
                ),
                operation: .renewLease(renewal),
                disposition: .apply,
                instruction: frame,
                quote: quote,
                totalsByAsset: [try AliasAssetTotalV1(
                    paymentAsset: guardValue.expectedPaymentAsset,
                    amount: quote.exactAmount
                )],
                warnings: [],
                blockers: [],
                validUntilMs: guardValue.validUntilMs
            ),
            planHash: AliasPlanVerifier.canonicalLifecycleHash(
                canonicalBodyNorito: renewalBodyBytes
            ).hexEncodedString()
        )
        XCTAssertEqual(AliasPlanVerifier.validateExecutable(renewalPlan), [])
        XCTAssertNoThrow(
            try AliasPlanVerifier.requireExecutable(
                renewalPlan,
                canonicalBodyNorito: renewalBodyBytes,
                roundTrip: { _, payload in payload }
            )
        )
        XCTAssertNoThrow(
            try AliasPlanVerifier.requireExecutableForRequest(
                .leaseRenewal(AliasLeaseRenewPlanRequestV1(renewal: renewal)),
                plan: renewalPlan,
                canonicalBodyNorito: renewalBodyBytes,
                decodeAndReencode: { _, payload in
                    DecodedAliasLifecycleFrame(
                        operation: .renewLease(renewal),
                        reencodedFrame: payload
                    )
                }
            )
        )

        let configuration = ConfigureAliasAutoRenew(
            target: .accountAlias(alias),
            expectedRevision: 4,
            config: nil
        )
        let noOpBodyBytes = Data([5, 6, 7, 8])
        let noOpPlan = try AliasLifecycleTransactionPlanV1(
            body: try AliasLifecycleTransactionPlanBodyV1(
                authority: authority,
                chainId: "test-chain",
                anchor: renewalPlan.body.anchor,
                operation: .configureAutoRenew(configuration),
                disposition: .noOp,
                instruction: nil,
                quote: nil,
                totalsByAsset: [],
                warnings: [],
                blockers: [],
                validUntilMs: 60_000
            ),
            planHash: AliasPlanVerifier.canonicalLifecycleHash(
                canonicalBodyNorito: noOpBodyBytes
            ).hexEncodedString()
        )
        XCTAssertEqual(AliasPlanVerifier.validateExecutable(noOpPlan), [])
        XCTAssertNoThrow(
            try AliasPlanVerifier.requireExecutable(
                noOpPlan,
                canonicalBodyNorito: noOpBodyBytes,
                roundTrip: { _, payload in payload }
            )
        )
        XCTAssertNoThrow(
            try AliasPlanVerifier.requireExecutableForRequest(
                .autoRenew(AliasAutoRenewPlanRequestV1(configuration: configuration)),
                plan: noOpPlan,
                canonicalBodyNorito: noOpBodyBytes,
                decodeAndReencode: { _, payload in
                    DecodedAliasLifecycleFrame(
                        operation: .configureAutoRenew(configuration),
                        reencodedFrame: payload
                    )
                }
            )
        )
        let encodedOperation = try JSONEncoder().encode(noOpPlan.body.operation)
        XCTAssertEqual(
            (try JSONSerialization.jsonObject(with: encodedOperation) as? [String: Any])?["kind"] as? String,
            "configure_auto_renew"
        )
    }

    func testCanonicalAccountsAssetsAndAutoRenewRangesFailClosed() throws {
        let alias = try ResolvedAccountAliasV1(canonicalName: "merchant@paynet", dataspaceId: 7)
        XCTAssertThrowsError(try AliasSetupPlanRequestV1(intents: []))
        XCTAssertThrowsError(
            try AliasAccountIntentV1(
                alias: alias,
                targetAccount: "merchant@paynet",
                provision: .existing,
                role: .primary
            )
        )
        XCTAssertThrowsError(
            try AliasQuoteGuardV1(
                expectedPolicyVersion: 1,
                expectedPaymentAsset: "xor#paynet",
                maxAmount: "1",
                validUntilMs: 1
            )
        )
        XCTAssertThrowsError(
            try AliasAutoRenewConfigV1(
                termYears: 1,
                policyVersion: 1,
                paymentAsset: "4rPeAP6jAjiLVZThZYwwPRBuQagt",
                maxAmount: "1",
                renewBeforeExpiryMs: 31_536_000_000,
                retryBackoffMs: 1,
                maxFailures: 1
            )
        )
    }

    private func loadSharedFixture() throws -> SharedAliasSetupFixture {
        var root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<3 { root.deleteLastPathComponent() }
        let url = root.appendingPathComponent("fixtures/norito_rpc/alias_setup_v1/alias_setup_v1.json")
        return try JSONDecoder().decode(SharedAliasSetupFixture.self, from: Data(contentsOf: url))
    }
}

private struct SharedAliasSetupFixture: Decodable {
    let accountAliasCases: [AccountAliasCase]
    let accountOnboardingReceiptVector: AccountOnboardingReceiptVector
    let resolvedNameJSONVectors: SharedResolvedNameVectors
    let quoteGuardJSONVector: AliasQuoteGuardV1
    let permissionScopeJSONVector: AccountAliasPermissionScope
    let planHashVectors: [PlanHashVector]
    let instructionFrameVectors: [InstructionFrameVector]
    let reportJSONVector: AliasSetupReportV1

    private enum CodingKeys: String, CodingKey {
        case accountAliasCases = "account_alias_cases"
        case accountOnboardingReceiptVector = "account_onboarding_receipt_vector"
        case resolvedNameJSONVectors = "resolved_name_json_vectors"
        case quoteGuardJSONVector = "quote_guard_json_vector"
        case permissionScopeJSONVector = "permission_scope_json_vector"
        case planHashVectors = "plan_hash_vectors"
        case instructionFrameVectors = "instruction_frame_vectors"
        case reportJSONVector = "report_json_vector"
    }
}

private struct AccountOnboardingReceiptVector: Decodable {
    let name: String
    let domain: String
    let canonicalBodyNoritoHex: String
    let canonicalPlanHashHex: String
    let authority: String
    let signatureHex: String
    let receiptJSON: ToriiAccountOnboardingPlanReceipt

    private enum CodingKeys: String, CodingKey {
        case name, domain, authority
        case canonicalBodyNoritoHex = "canonical_body_norito_hex"
        case canonicalPlanHashHex = "canonical_plan_hash_hex"
        case signatureHex = "signature_hex"
        case receiptJSON = "receipt_json"
    }
}

private struct SharedResolvedNameVectors: Decodable {
    let dataspace: ResolvedDataSpaceV1
    let domain: ResolvedDomainV1
    let accountAlias: ResolvedAccountAliasV1

    private enum CodingKeys: String, CodingKey {
        case dataspace, domain
        case accountAlias = "account_alias"
    }
}

private struct AccountAliasCase: Decodable {
    let input: String
    let canonical: String
    let label: String
    let domain: String?
    let dataspace: String
}

private struct PlanHashVector: Decodable {
    let name: String
    let domain: String
    let canonicalBodyNoritoHex: String
    let canonicalPlanHashHex: String

    private enum CodingKeys: String, CodingKey {
        case name, domain
        case canonicalBodyNoritoHex = "canonical_body_norito_hex"
        case canonicalPlanHashHex = "canonical_plan_hash_hex"
    }
}

private struct InstructionFrameVector: Decodable {
    let name: String
    let wireId: String
    let framedPayloadHex: String

    private enum CodingKeys: String, CodingKey {
        case name
        case wireId = "wire_id"
        case framedPayloadHex = "framed_payload_hex"
    }
}
