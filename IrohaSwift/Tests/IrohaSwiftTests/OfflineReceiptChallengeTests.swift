import XCTest
@testable import IrohaSwift

final class OfflineReceiptChallengeTests: XCTestCase {
    func testReceiptChallengeRejectsPaddedHashInputFields() throws {
        let fields = try challengeFields()

        let result = try OfflineReceiptChallenge.encode(
            chainId: fields.chainId,
            invoiceId: fields.invoiceId,
            receiverAccountId: fields.receiverAccountId,
            assetId: fields.assetId,
            amount: fields.amount,
            issuedAtMs: fields.issuedAtMs,
            senderCertificateIdHex: fields.senderCertificateIdHex,
            nonceHex: fields.nonceHex,
            expectedScale: 2
        )
        XCTAssertFalse(result.preimage.isEmpty)
        XCTAssertEqual(result.irohaHash.count, 32)
        XCTAssertEqual(result.clientDataHash.count, 32)

        XCTAssertThrowsError(try OfflineReceiptChallenge.encode(
            chainId: " \(fields.chainId)",
            invoiceId: fields.invoiceId,
            receiverAccountId: fields.receiverAccountId,
            assetId: fields.assetId,
            amount: fields.amount,
            issuedAtMs: fields.issuedAtMs,
            senderCertificateIdHex: fields.senderCertificateIdHex,
            nonceHex: fields.nonceHex,
            expectedScale: 2
        ))
        XCTAssertThrowsError(try OfflineReceiptChallenge.encode(
            chainId: fields.chainId,
            invoiceId: fields.invoiceId,
            receiverAccountId: fields.receiverAccountId,
            assetId: fields.assetId,
            amount: "\(fields.amount)\n",
            issuedAtMs: fields.issuedAtMs,
            senderCertificateIdHex: fields.senderCertificateIdHex,
            nonceHex: fields.nonceHex,
            expectedScale: 2
        ))
        XCTAssertThrowsError(try OfflineReceiptChallenge.encode(
            chainId: fields.chainId,
            invoiceId: fields.invoiceId,
            receiverAccountId: fields.receiverAccountId,
            assetId: fields.assetId,
            amount: fields.amount,
            issuedAtMs: fields.issuedAtMs,
            senderCertificateIdHex: "\t\(fields.senderCertificateIdHex)",
            nonceHex: fields.nonceHex,
            expectedScale: 2
        ))
        XCTAssertThrowsError(try OfflineReceiptChallenge.encode(
            chainId: fields.chainId,
            invoiceId: fields.invoiceId,
            receiverAccountId: fields.receiverAccountId,
            assetId: fields.assetId,
            amount: fields.amount,
            issuedAtMs: fields.issuedAtMs,
            senderCertificateIdHex: fields.senderCertificateIdHex,
            nonceHex: "\(fields.nonceHex) ",
            expectedScale: 2
        ))
    }

    private func challengeFields() throws -> ChallengeFields {
        let receiverAccountId = try accountId(seed: 0x51)
        return ChallengeFields(
            chainId: "chain-1",
            invoiceId: "invoice-1",
            receiverAccountId: receiverAccountId,
            assetId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(receiverAccountId)",
            amount: "12.34",
            issuedAtMs: 1_700_000_000_000,
            senderCertificateIdHex: String(repeating: "11", count: 32),
            nonceHex: String(repeating: "33", count: 32)
        )
    }

    private func accountId(seed: UInt8) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        return try AccountAddress.fromAccount(publicKey: keypair.publicKey).toI105(networkPrefix: 0x02F1)
    }
}

private struct ChallengeFields {
    let chainId: String
    let invoiceId: String
    let receiverAccountId: String
    let assetId: String
    let amount: String
    let issuedAtMs: UInt64
    let senderCertificateIdHex: String
    let nonceHex: String
}
