import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineSpendReceiptTests: XCTestCase {
    func testSigningBytesIgnoreSignatureAndSnapshot() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let txId = IrohaHash.hash(Data("tx-id".utf8))
        let challengeHash = IrohaHash.hash(Data("challenge".utf8))
        let keyId = Data("swift-tests".utf8).base64EncodedString()
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(keyId: keyId,
                                counter: 7,
                                assertion: Data([1, 2, 3]),
                                challengeHash: challengeHash)
        )
        let issuedAtMs = certificate.issuedAtMs + 1000
        let base = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: certificate.allowance.amount,
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-001",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0, count: 64)
        )
        let withSignature = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: certificate.allowance.amount,
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-001",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 1, count: 64)
        )
        let snapshot = OfflinePlatformTokenSnapshot(policy: "play_integrity",
                                                     attestationJwsB64: "token")
        let withSnapshot = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: certificate.allowance.amount,
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-001",
            platformProof: proof,
            platformSnapshot: snapshot,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0, count: 64)
        )

        let baseBytes = try base.signingBytes()
        XCTAssertEqual(baseBytes, try withSignature.signingBytes())
        XCTAssertEqual(baseBytes, try withSnapshot.signingBytes())
    }

    func testSigningBytesCanonicalizeAccountIds() throws {
        let publicKey = Data(repeating: 0x11, count: 32)
        let domain = "wonderland"
        let rawAccountId = AccountId.make(publicKey: publicKey, domain: domain)
        let address = try AccountAddress.fromAccount(domain: domain, publicKey: publicKey, algorithm: "ed25519")
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        let canonicalAccountId = "\(ih58)@\(domain)"
        let rawAssetId = "xor##\(rawAccountId)"
        let canonicalAssetId = "xor##\(canonicalAccountId)"

        let allowanceRaw = OfflineAllowanceCommitment(
            assetId: rawAssetId,
            amount: "10",
            commitment: Data(repeating: 0x01, count: 32)
        )
        let allowanceCanonical = OfflineAllowanceCommitment(
            assetId: canonicalAssetId,
            amount: "10",
            commitment: Data(repeating: 0x01, count: 32)
        )
        let policy = OfflineWalletPolicy(maxBalance: "100", maxTxValue: "50", expiresAtMs: 1)
        let spendPublicKey = "ed0120" + publicKey.hexUppercased()
        let issuedAtMs: UInt64 = 1_700_000_000_000
        let operatorSignature = Data(repeating: 0x02, count: 64)

        let certificateRaw = OfflineWalletCertificate(
            controller: rawAccountId,
            allowance: allowanceRaw,
            spendPublicKey: spendPublicKey,
            attestationReport: Data([0x01, 0x02]),
            issuedAtMs: issuedAtMs,
            expiresAtMs: issuedAtMs + 10,
            policy: policy,
            operatorSignature: operatorSignature
        )
        let certificateCanonical = OfflineWalletCertificate(
            controller: canonicalAccountId,
            allowance: allowanceCanonical,
            spendPublicKey: spendPublicKey,
            attestationReport: Data([0x01, 0x02]),
            issuedAtMs: issuedAtMs,
            expiresAtMs: issuedAtMs + 10,
            policy: policy,
            operatorSignature: operatorSignature
        )

        let txId = IrohaHash.hash(Data("tx-id".utf8))
        let challengeHash = IrohaHash.hash(Data("challenge".utf8))
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(
                keyId: "swift-tests",
                counter: 1,
                assertion: Data([0x01]),
                challengeHash: challengeHash
            )
        )

        let receiptRaw = OfflineSpendReceipt(
            txId: txId,
            from: rawAccountId,
            to: rawAccountId,
            assetId: rawAssetId,
            amount: "10",
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-raw",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificateRaw,
            senderSignature: Data(repeating: 0x00, count: 64)
        )
        let receiptCanonical = OfflineSpendReceipt(
            txId: txId,
            from: canonicalAccountId,
            to: canonicalAccountId,
            assetId: canonicalAssetId,
            amount: "10",
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-raw",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificateCanonical,
            senderSignature: Data(repeating: 0x00, count: 64)
        )

        XCTAssertEqual(try receiptRaw.signingBytes(), try receiptCanonical.signingBytes())
    }

    // MARK: - Helpers

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }
}
