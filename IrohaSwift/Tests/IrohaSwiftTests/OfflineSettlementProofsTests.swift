import Foundation
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
