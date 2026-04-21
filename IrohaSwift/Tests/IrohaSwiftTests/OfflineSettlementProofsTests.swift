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

    func testRedeemProofDecodingRejectsLegacyStringDirs() throws {
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

    func testVerifyRedeemRequestProofRejectsPrefixedHex() throws {
        let request = try makeRedeemRequest()
        var commits = request.redeemProof.envelope.proof.commits
        var roots = commits.roots
        roots[0] = "0x" + roots[0]
        commits = ToriiOfflineStarkCommitmentsV1(
            version: commits.version,
            roots: roots,
            compRoot: commits.compRoot
        )
        let mutatedEnvelope = ToriiOfflineStarkVerifyEnvelopeV1(
            params: request.redeemProof.envelope.params,
            proof: ToriiOfflineStarkProofV1(
                version: request.redeemProof.envelope.proof.version,
                commits: commits,
                queries: request.redeemProof.envelope.proof.queries
            ),
            transcriptLabel: request.redeemProof.envelope.transcriptLabel
        )
        let mutatedProof = ToriiOfflineRedeemRequestProof(
            backend: request.redeemProof.backend,
            circuitId: request.redeemProof.circuitId,
            recursionDepth: request.redeemProof.recursionDepth,
            publicInputsHex: request.redeemProof.publicInputsHex,
            envelope: mutatedEnvelope
        )

        XCTAssertThrowsError(
            try ToriiOfflineSettlementProofs.verifyRedeemRequestProof(
                proof: mutatedProof,
                operationId: request.operationId,
                accountId: request.accountId,
                lineageId: request.lineageId,
                assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                amount: request.amount,
                offlinePublicKey: request.deviceBinding.offlinePublicKey,
                authorizationId: try sampleAuthorization(deviceBinding: request.deviceBinding).authorizationId,
                preStateHash: String(repeating: "a", count: 64),
                receipts: request.receipts
            )
        ) { error in
            guard case .invalidSettlement = error as? ToriiOfflineSettlementProofError else {
                return XCTFail("Expected invalidSettlement, got \(error)")
            }
        }
    }

    func testMerklePathEncodingRejectsInvalidBase64Dirs() {
        let path = ToriiOfflineMerklePath(dirs: "not-base64", siblings: [])
        XCTAssertThrowsError(try JSONEncoder().encode(path))
    }

    private func makeRedeemRequest() throws -> ToriiOfflineCashRedeemRequest {
        let accountId = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB"
        let deviceBinding = sampleDeviceBinding()
        let deviceProof = sampleDeviceProof()
        let authorization = try sampleAuthorization(deviceBinding: deviceBinding)
        let receipt = ToriiOfflineTransferReceipt(
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
        return ToriiOfflineCashRedeemRequest(
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
        ToriiOfflineSpendAuthorization(
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
