import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

final class OfflineSettlementProofsTests: XCTestCase {
    func testCanonicalAmountStringPreservesScaleAndRustBounds() throws {
        XCTAssertEqual(try ToriiOfflineCashCodec.canonicalAmountString("1.00"), "1.00")
        XCTAssertEqual(try ToriiOfflineCashCodec.canonicalAmountString("000.010"), "0.010")
        XCTAssertEqual(try ToriiOfflineCashCodec.canonicalAmountString("-0.00"), "0.00")

        let maxPositiveMantissa = decimalSubtractOne(decimalPowerOfTwo(511))
        XCTAssertEqual(
            try ToriiOfflineCashCodec.canonicalAmountString(maxPositiveMantissa),
            maxPositiveMantissa
        )

        for invalid in [
            ".",
            "+",
            "-",
            "-.",
            "1,000",
            decimalPowerOfTwo(511),
            "0." + String(repeating: "1", count: 29)
        ] {
            XCTAssertThrowsError(try ToriiOfflineCashCodec.canonicalAmountString(invalid)) { error in
                XCTAssertEqual(error as? ToriiOfflineAmountError, .invalidAmount(invalid))
            }
        }
    }

    func testAmountArithmeticUsesRustScaleRules() throws {
        XCTAssertEqual(try ToriiOfflineCashCodec.addAmounts("1.20", "0.30"), "1.50")
        XCTAssertEqual(try ToriiOfflineCashCodec.addAmounts("1", "2.00"), "3.00")
        XCTAssertEqual(try ToriiOfflineCashCodec.subtractAmounts("3.00", "1"), "2.00")
        XCTAssertEqual(try ToriiOfflineCashCodec.compareAmounts("1.0", "1.00"), .orderedSame)

        XCTAssertThrowsError(try ToriiOfflineCashCodec.addAmounts("1,000", "1")) { error in
            XCTAssertEqual(error as? ToriiOfflineAmountError, .invalidAmount("1,000"))
        }
        XCTAssertThrowsError(try ToriiOfflineCashCodec.subtractAmounts("1.00", "2.00")) { error in
            XCTAssertEqual(error as? ToriiOfflineAmountError, .negativeResult)
        }

        let maxPositiveMantissa = decimalSubtractOne(decimalPowerOfTwo(511))
        XCTAssertEqual(
            try ToriiOfflineCashCodec.compareAmounts(maxPositiveMantissa, "0.1"),
            .orderedDescending
        )
        XCTAssertThrowsError(try ToriiOfflineCashCodec.addAmounts(maxPositiveMantissa, "1"))
    }

    func testPublicOfflineModelInitializersCanonicalizeInputs() throws {
        let deviceBinding = sampleDeviceBinding()
        let deviceProof = sampleDeviceProof()
        let authorization = try ToriiOfflineSpendAuthorization(
            authorizationId: "auth-1",
            lineageId: "lineage-1",
            accountId: "account-1",
            verdictId: "verdict-1",
            policyMaxBalance: "001000.00",
            policyMaxTxValue: "000200.50",
            issuedAtMs: 1,
            refreshAtMs: 2,
            expiresAtMs: 3,
            deviceBinding: deviceBinding,
            issuerSignatureBase64: "authorization-signature"
        )
        XCTAssertEqual(authorization.policyMaxBalance, "1000.00")
        XCTAssertEqual(authorization.policyMaxTxValue, "200.50")

        let loadRequest = try ToriiOfflineCashLoadRequest(
            operationId: "load-1",
            lineageId: "lineage-1",
            accountId: "account-1",
            assetDefinitionId: "asset-1",
            amount: "0001.20",
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        XCTAssertEqual(loadRequest.amount, "1.20")

        let commitments = try ToriiOfflineStarkCommitmentsV1(
            version: 1,
            roots: ["AABB"],
            compRoot: "CCDD"
        )
        XCTAssertEqual(commitments.roots, ["aabb"])
        XCTAssertEqual(commitments.compRoot, "ccdd")

        let path = try ToriiOfflineMerklePath(dirs: " AQ== ", siblings: ["AABB"])
        XCTAssertEqual(path.dirs, "AQ==")
        XCTAssertEqual(path.siblings, ["aabb"])
    }

    func testPublicOfflineModelInitializersRejectInvalidCanonicalFields() {
        let deviceBinding = sampleDeviceBinding()
        let deviceProof = sampleDeviceProof()

        XCTAssertThrowsError(
            try ToriiOfflineCashLoadRequest(
                operationId: "load-1",
                lineageId: nil,
                accountId: "account-1",
                assetDefinitionId: "asset-1",
                amount: "1,000",
                deviceBinding: deviceBinding,
                deviceProof: deviceProof
            )
        )
        XCTAssertThrowsError(
            try ToriiOfflineStarkCommitmentsV1(version: 1, roots: ["0xaabb"], compRoot: nil)
        )
        XCTAssertThrowsError(
            try ToriiOfflineMerklePath(dirs: "not-base64", siblings: [])
        )
    }

    func testSettlementCommitmentUsesServerCanonicalJSONWithoutEscapedSlashes() throws {
        let chainTxHash = "fae019f40d05859a9af5613cb96ba6939149fa960babc06d718ecde27c987b61"
        let preStateHash = "c0acca41d103901fe944f196c4b34e145a4deaa854b99b8acdb94effb0ba1fe0"
        let postStateHash = "09c885f971c3529ab4e69d4adf682fca6c1bc57df83afed349faff3145583e75"
        let expectedCommitment = "7cffb614fa11477fa1b9fef44b92a51d90a14cc3f263798be75f98fa9995a147"
        let expectedCompositionRoot = "a799551c93e6d325971bbec1a093856dc7cbca61d5c0978b821741ad19bc95d7"

        let settlement = try ToriiOfflineSettlementProofs.buildSettlement(
            kind: "load",
            operationId: "cash_load_slash_key_regression",
            accountId: "i105-test-account",
            lineageId: "lineage-1",
            assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
            amount: "200.00",
            offlinePublicKey: "abc/def+ghi==",
            authorizationId: "auth-1",
            preStateHash: preStateHash,
            postStateHash: postStateHash,
            chainTxHash: chainTxHash,
            entryHash: chainTxHash,
            blockHeight: 710
        )

        XCTAssertEqual(settlement.settlementCommitmentHex, expectedCommitment)
        XCTAssertEqual(settlement.proof.publicInputsHex, expectedCommitment)
        XCTAssertEqual(settlement.proof.envelope.proof.commits.compRoot, expectedCompositionRoot)

        try ToriiOfflineSettlementProofs.verifySettlement(
            settlement: settlement,
            kind: "load",
            operationId: "cash_load_slash_key_regression",
            accountId: "i105-test-account",
            lineageId: "lineage-1",
            assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
            amount: "200.00",
            offlinePublicKey: "abc/def+ghi==",
            authorizationId: "auth-1",
            preStateHash: preStateHash,
            envelopeStateHash: postStateHash
        )
    }

    func testRedeemRequestProofJSONUsesCanonicalHexAndByteArrayDirs() throws {
        let request = try makeRedeemRequest()
        let jsonObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: try JSONEncoder().encode(request)) as? [String: Any]
        )

        XCTAssertEqual(jsonObject["amount"] as? String, "30.00")

        let redeemProof = try XCTUnwrap(jsonObject["redeem_proof"] as? [String: Any])
        XCTAssertEqual(redeemProof["public_inputs_hex"] as? String, request.redeemProof.publicInputsHex)
        XCTAssertFalse(
            request.redeemProof.publicInputsHex.hasPrefix("0x"),
            "redeem public inputs must use canonical plain hex"
        )

        let envelope = try XCTUnwrap(redeemProof["envelope"] as? [String: Any])
        let proof = try XCTUnwrap(envelope["proof"] as? [String: Any])
        let commits = try XCTUnwrap(proof["commits"] as? [String: Any])
        let roots = try XCTUnwrap(commits["roots"] as? [String])
        XCTAssertFalse(roots.isEmpty)
        XCTAssertTrue(roots.allSatisfy { !$0.hasPrefix("0x") })

        let queryChains = try XCTUnwrap(proof["queries"] as? [Any])
        let firstChain = try XCTUnwrap(queryChains.first as? [Any])
        let firstDecommit = try XCTUnwrap(firstChain.first as? [String: Any])
        let pathY0 = try XCTUnwrap(firstDecommit["path_y0"] as? [String: Any])
        let dirs = try XCTUnwrap(pathY0["dirs"] as? [NSNumber])
        XCTAssertFalse(dirs.isEmpty, "Merkle direction bits must encode as a JSON byte array")
        XCTAssertNil(pathY0["dirs"] as? String)

        let siblings = try XCTUnwrap(pathY0["siblings"] as? [String])
        XCTAssertFalse(siblings.isEmpty)
        XCTAssertTrue(siblings.allSatisfy { !$0.hasPrefix("0x") })
    }

    func testRedeemProofDecodingRejectsStringDirs() throws {
        let proof = try makeRedeemRequest().redeemProof
        var jsonObject = try proofJSONObject(for: proof)
        let pathY0 = try mutatedPathY0(in: jsonObject) { path in
            var path = path
            path["dirs"] = "AQ=="
            return path
        }
        jsonObject = pathY0

        let data = try JSONSerialization.data(withJSONObject: jsonObject, options: [.sortedKeys])
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineRedeemRequestProof.self, from: data))
    }

    func testRedeemProofDecodingRejectsPrefixedHex() throws {
        let proof = try makeRedeemRequest().redeemProof
        var jsonObject = try proofJSONObject(for: proof)
        var envelope = try XCTUnwrap(jsonObject["envelope"] as? [String: Any])
        var proofObject = try XCTUnwrap(envelope["proof"] as? [String: Any])
        var commits = try XCTUnwrap(proofObject["commits"] as? [String: Any])
        var roots = try XCTUnwrap(commits["roots"] as? [String])
        roots[0] = "0x" + roots[0]
        commits["roots"] = roots
        proofObject["commits"] = commits
        envelope["proof"] = proofObject
        jsonObject["envelope"] = envelope

        let data = try JSONSerialization.data(withJSONObject: jsonObject, options: [.sortedKeys])
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineRedeemRequestProof.self, from: data))
    }

    func testCashModelDecodingRejectsInvalidAmounts() throws {
        let request = try makeRedeemRequest()
        let jsonObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: try JSONEncoder().encode(request)) as? [String: Any]
        )
        var mutated = jsonObject
        mutated["amount"] = "1,000"

        let data = try JSONSerialization.data(withJSONObject: mutated, options: [.sortedKeys])
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineCashRedeemRequest.self, from: data))
    }

    func testSourceLineageEnvelopeVerifierAcceptsWitnessBoundReceipt() throws {
        let fixture = try makeSourceLineageFixture()

        try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
            fixture.envelope,
            expectedTransferId: fixture.transferId,
            recipientLineageId: fixture.recipientLineageId,
            assetDefinitionId: fixture.assetDefinitionId,
            amount: fixture.amount,
            issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
        )
    }

    func testSourceLineageEnvelopeVerifierRejectsTamperedAirOpening() throws {
        let fixture = try makeSourceLineageFixture()
        let envelope = fixture.envelope.proof.envelope
        let proof = envelope.proof
        let air = try XCTUnwrap(proof.air)
        var openings = air.openings
        var row = openings[0].row
        row[0] += 1
        openings[0] = ToriiOfflineStarkAirOpeningV1(
            index: openings[0].index,
            row: row,
            nextRow: openings[0].nextRow,
            rowPath: openings[0].rowPath,
            nextRowPath: openings[0].nextRowPath,
            compositionValue: openings[0].compositionValue,
            compositionPath: openings[0].compositionPath
        )
        let tamperedProof = try ToriiOfflineTransparentZkProof(
            backend: fixture.envelope.proof.backend,
            circuitId: fixture.envelope.proof.circuitId,
            recursionDepth: fixture.envelope.proof.recursionDepth,
            publicInputsHex: fixture.envelope.proof.publicInputsHex,
            envelope: ToriiOfflineStarkVerifyEnvelopeV1(
                params: envelope.params,
                proof: ToriiOfflineStarkProofV1(
                    version: proof.version,
                    commits: proof.commits,
                    queries: proof.queries,
                    compValues: proof.compValues,
                    air: try ToriiOfflineStarkAirProofV1(
                        version: air.version,
                        circuitId: air.circuitId,
                        publicDigest: air.publicDigest,
                        traceRoot: air.traceRoot,
                        compositionRoot: air.compositionRoot,
                        traceWidth: air.traceWidth,
                        openings: openings
                    )
                ),
                transcriptLabel: envelope.transcriptLabel
            )
        )
        let tamperedEnvelope = ToriiOfflineSourceLineageEnvelope(
            publicInputs: fixture.envelope.publicInputs,
            witnessPayload: fixture.envelope.witnessPayload,
            proof: tamperedProof
        )

        XCTAssertThrowsError(
            try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
                tamperedEnvelope,
                expectedTransferId: fixture.transferId,
                recipientLineageId: fixture.recipientLineageId,
                assetDefinitionId: fixture.assetDefinitionId,
                amount: fixture.amount,
                issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
            )
        )
    }

    func testSourceLineageEnvelopeVerifierAcceptsPrefixedBase64WitnessPayload() throws {
        let fixture = try makeSourceLineageFixture()
        let encodedWitness = base64URL(Data(fixture.envelope.witnessPayload.utf8))
        let envelope = try ToriiOfflineSettlementProofs.buildSourceLineageEnvelope(
            publicInputs: fixture.envelope.publicInputs,
            witnessPayload: "wallet-offline-transfer:\(encodedWitness)"
        )

        try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
            envelope,
            expectedTransferId: fixture.transferId,
            recipientLineageId: fixture.recipientLineageId,
            assetDefinitionId: fixture.assetDefinitionId,
            amount: fixture.amount,
            issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
        )
    }

    func testSourceLineageEnvelopeVerifierAcceptsProductionDeviceProofWitnessPayload() throws {
        let fixture = try makeSourceLineageFixture()
        let envelope = try ToriiOfflineSettlementProofs.buildSourceLineageEnvelope(
            publicInputs: fixture.envelope.publicInputs,
            witnessPayload: fixture.productionWitnessPayload
        )

        try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
            envelope,
            expectedTransferId: fixture.transferId,
            recipientLineageId: fixture.recipientLineageId,
            assetDefinitionId: fixture.assetDefinitionId,
            amount: fixture.amount,
            issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
        )
    }

    func testSourceLineageEnvelopeVerifierRejectsWitnessMismatchedPublicInputs() throws {
        let fixture = try makeSourceLineageFixture()

        try assertSourceLineageMutationRejected(fixture) {
            try sourceInputs(from: $0, sourceReceiptHash: String(repeating: "0", count: 64))
        }
        try assertSourceLineageMutationRejected(fixture) {
            try sourceInputs(from: $0, senderLineageId: "other-sender-lineage")
        }
        try assertSourceLineageMutationRejected(fixture) {
            try sourceInputs(from: $0, sourcePreStateHash: String(repeating: "1", count: 64))
        }
        try assertSourceLineageMutationRejected(fixture) {
            try sourceInputs(from: $0, deviceProofKeyId: "other-key")
        }
        try assertSourceLineageMutationRejected(fixture) {
            try sourceInputs(from: $0, deviceProofCounter: 9)
        }
    }

    func testSourceLineageEnvelopeVerifierRejectsAssetNotBoundToWitnessAnchor() throws {
        let fixture = try makeSourceLineageFixture()
        let tampered = try rebindSourceLineageEnvelope(fixture.envelope) {
            try sourceInputs(from: $0, assetDefinitionId: "usd#paynet")
        }

        XCTAssertThrowsError(
            try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
                tampered,
                expectedTransferId: fixture.transferId,
                recipientLineageId: fixture.recipientLineageId,
                assetDefinitionId: "usd#paynet",
                amount: fixture.amount,
                issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
            )
        )
    }

    func testSourceLineageEnvelopeVerifierRejectsNoncanonicalWitnessAssetDefinition() throws {
        let fixture = try makeSourceLineageFixture(assetDefinitionId: "asset-1")

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsNoncanonicalWitnessAccount() throws {
        let fixture = try makeSourceLineageFixture(useNoncanonicalSenderAccountId: true)

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsNoncanonicalCounterpartyAccount() throws {
        let fixture = try makeSourceLineageFixture(useNoncanonicalCounterpartyAccountId: true)

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsRevokedAuthorization() throws {
        let fixture = try makeSourceLineageFixture()

        XCTAssertThrowsError(
            try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
                fixture.envelope,
                expectedTransferId: fixture.transferId,
                recipientLineageId: fixture.recipientLineageId,
                assetDefinitionId: fixture.assetDefinitionId,
                amount: fixture.amount,
                issuerPublicKeyBase64: fixture.issuerPublicKeyBase64,
                revokedVerdictIds: ["SENDER-VERDICT"]
            )
        )
    }

    func testSourceLineageEnvelopeVerifierAcceptsWitnessWithAncestry() throws {
        let fixture = try makeSourceLineageFixture(includeAncestry: true)

        try verifySourceLineageFixture(fixture)
    }

    func testSourceLineageEnvelopeVerifierRejectsInvalidIssuerSignature() throws {
        let fixture = try makeSourceLineageFixture(tamperAnchorIssuerSignature: true)

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsInvalidAuthorizationIssuerSignature() throws {
        let fixture = try makeSourceLineageFixture(tamperAuthorizationIssuerSignature: true)

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsInvalidAttestationChallengeHash() throws {
        let fixture = try makeSourceLineageFixture(tamperFinalAttestationHash: true)

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsCounterReplayAcrossAncestry() throws {
        let fixture = try makeSourceLineageFixture(
            includeAncestry: true,
            replayFinalCounter: true
        )

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsAncestryContinuityMismatch() throws {
        let fixture = try makeSourceLineageFixture(
            includeAncestry: true,
            tamperAncestryPostStateHash: true
        )

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    func testSourceLineageEnvelopeVerifierRejectsSourceProofOnOutgoingReceipt() throws {
        let nested = try makeSourceLineageFixture(
            transferId: "nested-source-transfer",
            recipientLineageId: "nested-recipient-lineage"
        )
        let fixture = try makeSourceLineageFixture(
            transferId: "source-transfer-with-invalid-proof-shape",
            outgoingSourceLineageProof: nested.envelope
        )

        XCTAssertThrowsError(try verifySourceLineageFixture(fixture))
    }

    private func makeRedeemRequest() throws -> ToriiOfflineCashRedeemRequest {
        let accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        let deviceBinding = sampleDeviceBinding()
        let deviceProof = sampleDeviceProof()
        let authorization = try sampleAuthorization(deviceBinding: deviceBinding)
        let receipt = try ToriiOfflineTransferReceipt(
            transferId: "transfer-1",
            direction: .incoming,
            lineageId: "lineage-1",
            accountId: accountId,
            deviceId: deviceBinding.deviceId,
            offlinePublicKey: deviceBinding.offlinePublicKey,
            preBalance: "67.50",
            postBalance: "97.50",
            preLockedBalance: "0",
            postLockedBalance: "0",
            preStateHash: String(repeating: "b", count: 64),
            postStateHash: String(repeating: "c", count: 64),
            localRevision: 3,
            counterpartyLineageId: "lineage-2",
            counterpartyAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            counterpartyDeviceId: "device-2",
            counterpartyOfflinePublicKey: "receiver-public-key",
            amount: "30.25",
            authorization: authorization,
            deviceProof: deviceProof,
            sourcePayload: nil,
            senderSignatureBase64: "sender-signature",
            createdAtMs: 1_700_000_123_456
        )
        let redeemProof = try ToriiOfflineSettlementProofs.buildRedeemRequestProof(
            operationId: "redeem-1",
            accountId: accountId,
            lineageId: "lineage-1",
            assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
            amount: "30.00",
            offlinePublicKey: deviceBinding.offlinePublicKey,
            authorizationId: authorization.authorizationId,
            preStateHash: String(repeating: "a", count: 64),
            receipts: [receipt]
        )
        return try ToriiOfflineCashRedeemRequest(
            operationId: "redeem-1",
            lineageId: "lineage-1",
            accountId: accountId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof,
            amount: "30.00",
            receipts: [receipt],
            redeemProof: redeemProof
        )
    }

    private struct SourceLineageFixture {
        let envelope: ToriiOfflineSourceLineageEnvelope
        let productionWitnessPayload: String
        let transferId: String
        let recipientLineageId: String
        let assetDefinitionId: String
        let amount: String
        let issuerPublicKeyBase64: String
    }

    private func makeSourceLineageFixture(
        assetDefinitionId: String = "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
        transferId: String = "source-transfer",
        recipientLineageId: String = "receiver-lineage",
        amount: String = "10",
        includeAncestry: Bool = false,
        replayFinalCounter: Bool = false,
        tamperFinalAttestationHash: Bool = false,
        tamperAuthorizationIssuerSignature: Bool = false,
        tamperAnchorIssuerSignature: Bool = false,
        tamperAncestryPostStateHash: Bool = false,
        useNoncanonicalSenderAccountId: Bool = false,
        useNoncanonicalCounterpartyAccountId: Bool = false,
        outgoingSourceLineageProof: ToriiOfflineSourceLineageEnvelope? = nil
    ) throws -> SourceLineageFixture {
        let issuerSigningKey = Curve25519.Signing.PrivateKey()
        let issuerPublicKeyBase64 = issuerSigningKey.publicKey.rawRepresentation.base64EncodedString()
        let signingKey = P256.Signing.PrivateKey()
        let offlinePublicKey = signingKey.publicKey.x963Representation.base64EncodedString()
        let senderAccountId = useNoncanonicalSenderAccountId
            ? "sender-account"
            : try AccountId.makeI105(publicKey: Data(repeating: 0x11, count: 32))
        let counterpartyAccountId = useNoncanonicalCounterpartyAccountId
            ? "receiver-account"
            : try AccountId.makeI105(publicKey: Data(repeating: 0x22, count: 32))
        let deviceBinding = ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "sender-key",
            deviceId: "sender-device",
            offlinePublicKey: offlinePublicKey,
            attestationReportBase64: ""
        )
        let unsignedAuthorization = try ToriiOfflineSpendAuthorization(
            authorizationId: "source-auth",
            lineageId: "sender-lineage",
            accountId: senderAccountId,
            verdictId: "sender-verdict",
            policyMaxBalance: "1000",
            policyMaxTxValue: "1000",
            issuedAtMs: 1,
            refreshAtMs: 2,
            expiresAtMs: 1_700_000_000_000,
            deviceBinding: deviceBinding,
            issuerSignatureBase64: ""
        )
        let authorizationSignature = tamperAuthorizationIssuerSignature
            ? Data("invalid authorization issuer signature".utf8).base64EncodedString()
            : try issuerSigningKey.signature(
                for: ToriiOfflineCashCodec.authorizationUnsignedPayload(unsignedAuthorization)
            ).base64EncodedString()
        let authorization = try ToriiOfflineSpendAuthorization(
            authorizationId: unsignedAuthorization.authorizationId,
            lineageId: unsignedAuthorization.lineageId,
            accountId: unsignedAuthorization.accountId,
            verdictId: unsignedAuthorization.verdictId,
            policyMaxBalance: unsignedAuthorization.policyMaxBalance,
            policyMaxTxValue: unsignedAuthorization.policyMaxTxValue,
            issuedAtMs: unsignedAuthorization.issuedAtMs,
            refreshAtMs: unsignedAuthorization.refreshAtMs,
            expiresAtMs: unsignedAuthorization.expiresAtMs,
            deviceBinding: unsignedAuthorization.deviceBinding,
            issuerSignatureBase64: authorizationSignature
        )
        let unsignedAnchor = try ToriiOfflineCashState(
            lineageId: "sender-lineage",
            accountId: senderAccountId,
            deviceId: deviceBinding.deviceId,
            offlinePublicKey: offlinePublicKey,
            assetDefinitionId: assetDefinitionId,
            balance: "25",
            lockedBalance: "0",
            serverRevision: 1,
            serverStateHash: String(repeating: "a", count: 64),
            pendingLocalRevision: 0,
            authorization: authorization,
            issuerSignatureBase64: ""
        )
        let anchorSignature = tamperAnchorIssuerSignature
            ? Data("invalid issuer signature".utf8).base64EncodedString()
            : try issuerSigningKey.signature(
                for: ToriiOfflineCashCodec.lineageStateUnsignedPayload(unsignedAnchor)
            ).base64EncodedString()
        let anchor = try ToriiOfflineCashState(
            lineageId: unsignedAnchor.lineageId,
            accountId: unsignedAnchor.accountId,
            deviceId: unsignedAnchor.deviceId,
            offlinePublicKey: unsignedAnchor.offlinePublicKey,
            assetDefinitionId: unsignedAnchor.assetDefinitionId,
            balance: unsignedAnchor.balance,
            lockedBalance: unsignedAnchor.lockedBalance,
            serverRevision: unsignedAnchor.serverRevision,
            serverStateHash: unsignedAnchor.serverStateHash,
            pendingLocalRevision: unsignedAnchor.pendingLocalRevision,
            authorization: authorization,
            issuerSignatureBase64: anchorSignature
        )

        func signedOutgoingReceipt(
            transferId: String,
            recipientLineageId: String,
            amount: String,
            localRevision: UInt64,
            preBalance: String,
            preLockedBalance: String,
            preStateHash: String,
            counter: UInt64,
            createdAtMs: UInt64,
            tamperPostStateHash: Bool = false,
            tamperChallengeHash: Bool = false,
            sourceLineageProof: ToriiOfflineSourceLineageEnvelope? = nil
        ) throws -> ToriiOfflineTransferReceipt {
            let postBalance = try ToriiOfflineCashCodec.subtractAmounts(preBalance, amount)
            let postLockedBalance = "0"
            let computedPostStateHash = try ToriiOfflineCashCodec.nextLocalStateHash(
                lineageId: anchor.lineageId,
                previousStateHash: preStateHash,
                transferId: transferId,
                direction: .outgoing,
                counterpartyLineageId: recipientLineageId,
                amount: amount,
                localRevision: localRevision,
                postBalance: postBalance,
                postLockedBalance: postLockedBalance
            )
            let challengeHash: String
            if tamperChallengeHash {
                challengeHash = String(repeating: "f", count: 64)
            } else {
                challengeHash = try attestationChallengeHash(
                    accountId: anchor.accountId,
                    lineageId: anchor.lineageId,
                    operation: "send",
                    innerPayload: [
                        "amount": amount,
                        "lineage_id": anchor.lineageId,
                        "receiver_lineage_id": recipientLineageId,
                        "transfer_id": transferId
                    ]
                )
            }
            let unsignedReceipt = try ToriiOfflineTransferReceipt(
                transferId: transferId,
                direction: .outgoing,
                lineageId: anchor.lineageId,
                accountId: anchor.accountId,
                deviceId: anchor.deviceId,
                offlinePublicKey: anchor.offlinePublicKey,
                preBalance: preBalance,
                postBalance: postBalance,
                preLockedBalance: preLockedBalance,
                postLockedBalance: postLockedBalance,
                preStateHash: preStateHash,
                postStateHash: tamperPostStateHash ? String(repeating: "e", count: 64) : computedPostStateHash,
                localRevision: localRevision,
                counterpartyLineageId: recipientLineageId,
                counterpartyAccountId: counterpartyAccountId,
                counterpartyDeviceId: "receiver-device",
                counterpartyOfflinePublicKey: "receiver-public-key",
                amount: amount,
                authorization: authorization,
                deviceProof: ToriiOfflineDeviceProof(
                    platform: "ios",
                    attestationKeyId: deviceBinding.attestationKeyId,
                    challengeHashHex: challengeHash,
                    assertionBase64: "YXNzZXJ0aW9u",
                    counter: counter
                ),
                sourceLineageProof: sourceLineageProof,
                sourcePayload: nil,
                senderSignatureBase64: "",
                createdAtMs: createdAtMs
            )
            let signature = try signingKey.signature(
                for: ToriiOfflineCashCodec.cashTransferReceiptUnsignedPayload(unsignedReceipt)
            )
            return try ToriiOfflineTransferReceipt(
                transferId: unsignedReceipt.transferId,
                direction: unsignedReceipt.direction,
                lineageId: unsignedReceipt.lineageId,
                accountId: unsignedReceipt.accountId,
                deviceId: unsignedReceipt.deviceId,
                offlinePublicKey: unsignedReceipt.offlinePublicKey,
                preBalance: unsignedReceipt.preBalance,
                postBalance: unsignedReceipt.postBalance,
                preLockedBalance: unsignedReceipt.preLockedBalance,
                postLockedBalance: unsignedReceipt.postLockedBalance,
                preStateHash: unsignedReceipt.preStateHash,
                postStateHash: unsignedReceipt.postStateHash,
                localRevision: unsignedReceipt.localRevision,
                counterpartyLineageId: unsignedReceipt.counterpartyLineageId,
                counterpartyAccountId: unsignedReceipt.counterpartyAccountId,
                counterpartyDeviceId: unsignedReceipt.counterpartyDeviceId,
                counterpartyOfflinePublicKey: unsignedReceipt.counterpartyOfflinePublicKey,
                amount: unsignedReceipt.amount,
                authorization: unsignedReceipt.authorization,
                deviceProof: unsignedReceipt.deviceProof,
                sourceLineageProof: unsignedReceipt.sourceLineageProof,
                sourcePayload: nil,
                senderSignatureBase64: signature.derRepresentation.base64EncodedString(),
                createdAtMs: unsignedReceipt.createdAtMs
            )
        }

        var ancestryReceipts: [ToriiOfflineTransferReceipt] = []
        var currentBalance = anchor.balance
        var currentLockedBalance = anchor.lockedBalance
        var currentStateHash = anchor.serverStateHash
        var currentRevision = anchor.pendingLocalRevision
        if includeAncestry {
            let ancestryReceipt = try signedOutgoingReceipt(
                transferId: "source-ancestry-transfer",
                recipientLineageId: "ancestry-recipient-lineage",
                amount: "5",
                localRevision: currentRevision + 1,
                preBalance: currentBalance,
                preLockedBalance: currentLockedBalance,
                preStateHash: currentStateHash,
                counter: 1,
                createdAtMs: 900,
                tamperPostStateHash: tamperAncestryPostStateHash
            )
            ancestryReceipts = [ancestryReceipt]
            currentBalance = ancestryReceipt.postBalance
            currentLockedBalance = ancestryReceipt.postLockedBalance
            currentStateHash = ancestryReceipt.postStateHash
            currentRevision = ancestryReceipt.localRevision
        }

        let receipt = try signedOutgoingReceipt(
            transferId: transferId,
            recipientLineageId: recipientLineageId,
            amount: amount,
            localRevision: currentRevision + 1,
            preBalance: currentBalance,
            preLockedBalance: currentLockedBalance,
            preStateHash: currentStateHash,
            counter: replayFinalCounter ? 1 : (includeAncestry ? 2 : 1),
            createdAtMs: 1_000,
            tamperChallengeHash: tamperFinalAttestationHash,
            sourceLineageProof: outgoingSourceLineageProof
        )
        let witnessPayload = try sourceLineageWitnessPayload(
            anchor: anchor,
            ancestryReceipts: ancestryReceipts,
            receipt: receipt
        )
        let productionPayload = ToriiOfflineOutgoingTransferPayload(
            anchor: anchor,
            ancestryReceipts: ancestryReceipts,
            receipt: receipt
        )
        let productionWitnessData = try ToriiOfflineCashCodec.canonicalData(productionPayload)
        let productionWitnessPayload = "wallet-offline-transfer:\(base64URL(productionWitnessData))"
        let sourceReceiptHash = try sha256Hex(rustReceiptData(receipt))
        let unsignedInputs = try ToriiOfflineSourceLineagePublicInputs(
            transferId: transferId,
            sourceReceiptHash: sourceReceiptHash,
            senderLineageId: receipt.lineageId,
            recipientLineageId: recipientLineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            sourcePreStateHash: receipt.preStateHash,
            sourcePostStateHash: receipt.postStateHash,
            sourceLocalRevision: receipt.localRevision,
            deviceProofKeyId: receipt.deviceProof.attestationKeyId,
            deviceProofCounter: receipt.deviceProof.counter ?? 0,
            sourceNullifier: ""
        )
        let inputs = try sourceInputs(
            from: unsignedInputs,
            sourceNullifier: ToriiOfflineSettlementProofs.sourceLineageNullifierHex(unsignedInputs)
        )
        return SourceLineageFixture(
            envelope: try ToriiOfflineSettlementProofs.buildSourceLineageEnvelope(
                publicInputs: inputs,
                witnessPayload: witnessPayload
            ),
            productionWitnessPayload: productionWitnessPayload,
            transferId: transferId,
            recipientLineageId: recipientLineageId,
            assetDefinitionId: assetDefinitionId,
            amount: amount,
            issuerPublicKeyBase64: issuerPublicKeyBase64
        )
    }

    private func sampleDeviceBinding() -> ToriiOfflineDeviceBinding {
        ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attest-key",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "YXR0ZXN0YXRpb24tcmVwb3J0",
            iosTeamId: "TEAMID1234",
            iosBundleId: "io.example.wallet",
            iosEnvironment: "development"
        )
    }

    private func sampleDeviceProof() -> ToriiOfflineDeviceProof {
        ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: "attest-key",
            challengeHashHex: "challenge-hash",
            assertionBase64: "YXNzZXJ0aW9u",
            counter: 7
        )
    }

    private func sampleAuthorization(
        deviceBinding: ToriiOfflineDeviceBinding
    ) throws -> ToriiOfflineSpendAuthorization {
        try ToriiOfflineSpendAuthorization(
            authorizationId: "auth-1",
            lineageId: "lineage-1",
            accountId: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB",
            verdictId: "verdict-1",
            policyMaxBalance: "1000.00",
            policyMaxTxValue: "200.00",
            issuedAtMs: 1_700_000_000_000,
            refreshAtMs: 1_700_003_600_000,
            expiresAtMs: 1_700_086_400_000,
            deviceBinding: deviceBinding,
            issuerSignatureBase64: "authorization-signature"
        )
    }

    private func proofJSONObject(
        for proof: ToriiOfflineRedeemRequestProof
    ) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: try JSONEncoder().encode(proof)) as? [String: Any])
    }

    private func mutatedPathY0(
        in jsonObject: [String: Any],
        update: ([String: Any]) -> [String: Any]
    ) throws -> [String: Any] {
        var jsonObject = jsonObject
        var envelope = try XCTUnwrap(jsonObject["envelope"] as? [String: Any])
        var proof = try XCTUnwrap(envelope["proof"] as? [String: Any])
        var queryChains = try XCTUnwrap(proof["queries"] as? [Any])
        var firstChain = try XCTUnwrap(queryChains.first as? [Any])
        var firstDecommit = try XCTUnwrap(firstChain.first as? [String: Any])
        let pathY0 = try XCTUnwrap(firstDecommit["path_y0"] as? [String: Any])
        firstDecommit["path_y0"] = update(pathY0)
        firstChain[0] = firstDecommit
        queryChains[0] = firstChain
        proof["queries"] = queryChains
        envelope["proof"] = proof
        jsonObject["envelope"] = envelope
        return jsonObject
    }

    private func assertSourceLineageMutationRejected(
        _ fixture: SourceLineageFixture,
        mutate: (ToriiOfflineSourceLineagePublicInputs) throws -> ToriiOfflineSourceLineagePublicInputs,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        let tampered = try rebindSourceLineageEnvelope(fixture.envelope, mutate: mutate)
        XCTAssertThrowsError(
            try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
                tampered,
                expectedTransferId: fixture.transferId,
                recipientLineageId: fixture.recipientLineageId,
                assetDefinitionId: fixture.assetDefinitionId,
                amount: fixture.amount,
                issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
            ),
            file: file,
            line: line
        )
    }

    private func verifySourceLineageFixture(_ fixture: SourceLineageFixture) throws {
        try ToriiOfflineSettlementProofs.verifySourceLineageEnvelope(
            fixture.envelope,
            expectedTransferId: fixture.transferId,
            recipientLineageId: fixture.recipientLineageId,
            assetDefinitionId: fixture.assetDefinitionId,
            amount: fixture.amount,
            issuerPublicKeyBase64: fixture.issuerPublicKeyBase64
        )
    }

    private func rebindSourceLineageEnvelope(
        _ envelope: ToriiOfflineSourceLineageEnvelope,
        mutate: (ToriiOfflineSourceLineagePublicInputs) throws -> ToriiOfflineSourceLineagePublicInputs
    ) throws -> ToriiOfflineSourceLineageEnvelope {
        let mutatedInputs = try mutate(envelope.publicInputs)
        let reboundInputs = try sourceInputs(
            from: mutatedInputs,
            sourceNullifier: ToriiOfflineSettlementProofs.sourceLineageNullifierHex(mutatedInputs)
        )
        return try ToriiOfflineSettlementProofs.buildSourceLineageEnvelope(
            publicInputs: reboundInputs,
            witnessPayload: envelope.witnessPayload
        )
    }

    private func sourceInputs(
        from inputs: ToriiOfflineSourceLineagePublicInputs,
        transferId: String? = nil,
        sourceReceiptHash: String? = nil,
        senderLineageId: String? = nil,
        recipientLineageId: String? = nil,
        assetDefinitionId: String? = nil,
        amount: String? = nil,
        sourcePreStateHash: String? = nil,
        sourcePostStateHash: String? = nil,
        sourceLocalRevision: UInt64? = nil,
        deviceProofKeyId: String? = nil,
        deviceProofCounter: UInt64? = nil,
        sourceNullifier: String? = nil
    ) throws -> ToriiOfflineSourceLineagePublicInputs {
        try ToriiOfflineSourceLineagePublicInputs(
            transferId: transferId ?? inputs.transferId,
            sourceReceiptHash: sourceReceiptHash ?? inputs.sourceReceiptHash,
            senderLineageId: senderLineageId ?? inputs.senderLineageId,
            recipientLineageId: recipientLineageId ?? inputs.recipientLineageId,
            assetDefinitionId: assetDefinitionId ?? inputs.assetDefinitionId,
            amount: amount ?? inputs.amount,
            sourcePreStateHash: sourcePreStateHash ?? inputs.sourcePreStateHash,
            sourcePostStateHash: sourcePostStateHash ?? inputs.sourcePostStateHash,
            sourceLocalRevision: sourceLocalRevision ?? inputs.sourceLocalRevision,
            deviceProofKeyId: deviceProofKeyId ?? inputs.deviceProofKeyId,
            deviceProofCounter: deviceProofCounter ?? inputs.deviceProofCounter,
            sourceNullifier: sourceNullifier ?? inputs.sourceNullifier
        )
    }

    private func sourceLineageWitnessPayload(
        anchor: ToriiOfflineCashState,
        ancestryReceipts: [ToriiOfflineTransferReceipt] = [],
        receipt: ToriiOfflineTransferReceipt
    ) throws -> String {
        var anchorObject = try jsonObject(anchor)
        var receiptObject = try rustReceiptObject(receipt)
        let ancestryObjects = try ancestryReceipts.map { try rustReceiptObject($0) }
        try enrichAuthorization(in: &anchorObject)
        try enrichAuthorization(in: &receiptObject)
        let wrapper: [String: Any] = [
            "version": 1,
            "anchor": anchorObject,
            "ancestry_receipts": ancestryObjects,
            "receipt": receiptObject
        ]
        let data = try JSONSerialization.data(
            withJSONObject: wrapper,
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
        return String(data: data, encoding: .utf8)!
    }

    private func rustReceiptData(_ receipt: ToriiOfflineTransferReceipt) throws -> Data {
        try JSONSerialization.data(
            withJSONObject: rustReceiptObject(receipt),
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
    }

    private func rustReceiptObject(_ receipt: ToriiOfflineTransferReceipt) throws -> [String: Any] {
        var receiptObject = try jsonObject(receipt)
        try enrichAuthorization(in: &receiptObject)
        if var deviceProof = receiptObject["device_proof"] as? [String: Any] {
            let keyId = deviceProof["attestation_key_id"] ?? ""
            deviceProof["key_id"] = keyId
            deviceProof.removeValue(forKey: "attestation_key_id")
            deviceProof.removeValue(forKey: "platform")
            receiptObject["attestation"] = deviceProof
            receiptObject.removeValue(forKey: "device_proof")
        }
        return receiptObject
    }

    private func enrichAuthorization(in object: inout [String: Any]) throws {
        guard var authorization = object["authorization"] as? [String: Any],
              let binding = authorization["device_binding"] as? [String: Any] else {
            return
        }
        authorization["device_id"] = authorization["device_id"] ?? binding["device_id"]
        authorization["offline_public_key"] = authorization["offline_public_key"] ?? binding["offline_public_key"]
        authorization["app_attest_key_id"] = authorization["app_attest_key_id"] ?? binding["attestation_key_id"]
        object["authorization"] = authorization
    }

    private func jsonObject<T: Encodable>(_ value: T) throws -> [String: Any] {
        try XCTUnwrap(
            JSONSerialization.jsonObject(with: ToriiOfflineCashCodec.canonicalData(value))
                as? [String: Any]
        )
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private func attestationChallengeHash(
        accountId: String,
        lineageId: String,
        operation: String,
        innerPayload: [String: Any]
    ) throws -> String {
        let innerData = try JSONSerialization.data(
            withJSONObject: innerPayload,
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
        let challengeData = try JSONSerialization.data(
            withJSONObject: [
                "account_id": accountId,
                "lineage_id": lineageId,
                "operation": operation,
                "payload_hash": sha256Hex(innerData)
            ],
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
        return sha256Hex(challengeData)
    }

    private func base64URL(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    private func decimalPowerOfTwo(_ exponent: Int) -> String {
        var digits = [1]
        for _ in 0..<exponent {
            var carry = 0
            for index in 0..<digits.count {
                let next = digits[index] * 2 + carry
                digits[index] = next % 10
                carry = next / 10
            }
            while carry > 0 {
                digits.append(carry % 10)
                carry /= 10
            }
        }
        return digits.reversed().map(String.init).joined()
    }

    private func decimalSubtractOne(_ value: String) -> String {
        var digits = Array(value)
        var index = digits.endIndex
        while index > digits.startIndex {
            index = digits.index(before: index)
            if digits[index] == "0" {
                digits[index] = "9"
                continue
            }
            let scalar = digits[index].unicodeScalars.first!.value - 1
            digits[index] = Character(UnicodeScalar(scalar)!)
            break
        }
        while digits.count > 1 && digits.first == "0" {
            digits.removeFirst()
        }
        return String(digits)
    }
}
