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

    // MARK: - Helpers

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }
}
