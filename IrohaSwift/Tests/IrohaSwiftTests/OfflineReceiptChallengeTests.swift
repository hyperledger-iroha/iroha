import XCTest
@testable import IrohaSwift
#if canImport(CryptoKit)
import CryptoKit
#endif

final class OfflineReceiptChallengeTests: XCTestCase {
    private static let fixtureReceiverAccountId: String = {
        let publicKey = Data(repeating: 0x44, count: 32)
        let address = try! AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        return try! address.toI105(networkPrefix: 0x02F1)
    }()

    func testEncodeRejectsEmptyChainId() throws {
        let nonce = sampleNonceHex()
        let senderCertificateIdHex = sampleSenderCertificateIdHex()
        let fixtureAssetId = try makeAssetId(name: "xor",
                                             domain: "sora",
                                             accountId: Self.fixtureReceiverAccountId)
        XCTAssertThrowsError(
            try OfflineReceiptChallenge.encode(
                chainId: " ",
                invoiceId: "inv-empty",
                receiverAccountId: Self.fixtureReceiverAccountId,
                assetId: fixtureAssetId,
                amount: "1",
                issuedAtMs: 1_700_000_000_000,
                senderCertificateIdHex: senderCertificateIdHex,
                nonceHex: nonce
            )
        ) { error in
            guard case let OfflineReceiptChallenge.Error.invalidInput(message) = error else {
                return XCTFail("expected invalidInput error")
            }
            XCTAssertEqual(message, "chainId must not be empty")
        }
    }

    func testEncodeAcceptsScaledAmount() throws {
        let nonce = sampleNonceHex()
        let senderCertificateIdHex = sampleSenderCertificateIdHex()
        let fixtureAssetId = try makeAssetId(name: "xor",
                                             domain: "sora",
                                             accountId: Self.fixtureReceiverAccountId)
        XCTAssertNoThrow(
            try OfflineReceiptChallenge.encode(
                chainId: "testnet",
                invoiceId: "inv-frac",
                receiverAccountId: Self.fixtureReceiverAccountId,
                assetId: fixtureAssetId,
                amount: "1.5",
                issuedAtMs: 1_700_000_000_000,
                senderCertificateIdHex: senderCertificateIdHex,
                nonceHex: nonce
            )
        )
    }

    func testEncodeRejectsScaleMismatchWhenExpectedScaleProvided() throws {
        let nonce = sampleNonceHex()
        let senderCertificateIdHex = sampleSenderCertificateIdHex()
        let fixtureAssetId = try makeAssetId(name: "xor",
                                             domain: "sora",
                                             accountId: Self.fixtureReceiverAccountId)
        XCTAssertThrowsError(
            try OfflineReceiptChallenge.encode(
                chainId: "testnet",
                invoiceId: "inv-frac",
                receiverAccountId: Self.fixtureReceiverAccountId,
                assetId: fixtureAssetId,
                amount: "1.5",
                issuedAtMs: 1_700_000_000_000,
                senderCertificateIdHex: senderCertificateIdHex,
                nonceHex: nonce,
                expectedScale: 0
            )
        ) { error in
            guard case let OfflineReceiptChallenge.Error.invalidInput(message) = error else {
                return XCTFail("expected invalidInput error")
            }
            XCTAssertEqual(message, "amount must use scale 0: 1.5")
        }
    }

    func testEncodeProducesDeterministicHashes() throws {
        #if canImport(Darwin)
        let nonce = IrohaHash.hash(Data("receipt-nonce".utf8)).hexUppercased()
        let senderCertificateIdHex = sampleSenderCertificateIdHex()
        let fixtureAssetId = try makeAssetId(name: "xor",
                                             domain: "sora",
                                             accountId: Self.fixtureReceiverAccountId)
        let result = try OfflineReceiptChallenge.encode(
            chainId: "testnet",
            invoiceId: "inv-swift-tests",
            receiverAccountId: Self.fixtureReceiverAccountId,
            assetId: fixtureAssetId,
            amount: "250",
            issuedAtMs: 1_700_000_000_000,
            senderCertificateIdHex: senderCertificateIdHex,
            nonceHex: nonce
        )

        XCTAssertEqual(result.irohaHash.count, 32)
        XCTAssertEqual(result.clientDataHash.count, 32)
        XCTAssertTrue(result.preimage.starts(with: Data([0x4E, 0x52, 0x54, 0x30]))) // NRT0
        XCTAssertEqual(result.irohaHash.last.map { $0 & 1 }, 1)
        #if canImport(CryptoKit)
        let expected = Data(SHA256.hash(data: result.irohaHash))
        XCTAssertEqual(result.clientDataHash, expected)
        #endif
        #else
        throw XCTSkip("Offline receipt challenge bridge unavailable on this platform")
        #endif
    }

    func testChallengePreimageCanonicalizesReceiverAccountId() throws {
        let publicKey = Data(repeating: 0x22, count: 32)
        let domain = AccountAddress.defaultDomainName
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        let rawAccountId = try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        let canonicalAccountId = try address.toI105(networkPrefix: 0x02F1)
        let rawAssetId = try makeAssetId(name: "xor", domain: domain, accountId: rawAccountId)
        let canonicalAssetId = try makeAssetId(name: "xor", domain: domain, accountId: canonicalAccountId)
        let nonceHex = IrohaHash.hash(Data("receipt-nonce".utf8)).hexUppercased()
        let nonce = try XCTUnwrap(Data(hexString: nonceHex))
        let senderCertificateId = IrohaHash.hash(Data("sender-certificate".utf8))

        let rawPreimage = OfflineReceiptChallengePreimage(
            invoiceId: "inv-raw",
            receiverAccountId: rawAccountId,
            assetId: rawAssetId,
            amount: "10",
            issuedAtMs: 1_700_000_000_000,
            senderCertificateId: senderCertificateId,
            nonce: nonce
        )
        let canonicalPreimage = OfflineReceiptChallengePreimage(
            invoiceId: "inv-raw",
            receiverAccountId: canonicalAccountId,
            assetId: canonicalAssetId,
            amount: "10",
            issuedAtMs: 1_700_000_000_000,
            senderCertificateId: senderCertificateId,
            nonce: nonce
        )

        XCTAssertEqual(try rawPreimage.noritoPayload(), try canonicalPreimage.noritoPayload())
    }

    /// Calls the native bridge `offlineReceiptChallenge` directly (no Swift fallback)
    /// to verify that the C function pointer obtained via dlsym is valid and callable.
    /// This reproduces the EXC_BAD_ACCESS that occurs when dlopen returns a stale handle.
    func testNativeBridgeOfflineReceiptChallengeDoesNotCrash() throws {
        #if canImport(Darwin)
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAvailable else {
            throw XCTSkip("NoritoBridge not available")
        }
        let publicKey = Data(repeating: 0x11, count: 32)
        let receiverAccountId = AccountId.make(publicKey: publicKey)
        let assetId = try fixtureAllowanceAssetId()
        let senderCertificateIdHex = sampleSenderCertificateIdHex()
        let nonceHex = sampleNonceHex()
        let result = try bridge.offlineReceiptChallenge(
            chainId: "00000000-0000-0000-0000-000000000000",
            invoiceId: "5C91387D-5210-4908-913F-608F1BB4FE9A",
            receiverId: receiverAccountId,
            assetId: assetId,
            amount: "4.00",
            issuedAtMs: 1_700_000_000_000,
            senderCertificateIdHex: senderCertificateIdHex,
            nonceHex: nonceHex
        )
        guard let native = result else {
            throw XCTSkip("offline receipt challenge ABI unavailable in loaded bridge")
        }
        XCTAssertEqual(native.irohaHash.count, 32)
        XCTAssertEqual(native.clientHash.count, 32)
        XCTAssertFalse(native.preimage.isEmpty)
        #else
            throw XCTSkip("NoritoBridge not available on this platform")
        #endif
    }

    private func fixtureAllowanceAssetId() throws -> String {
        let url = fixtureURL("summary.json")
        let data = try Data(contentsOf: url)
        let object = try JSONSerialization.jsonObject(with: data, options: [])
        guard let map = object as? [String: Any],
              let value = map["allowance_asset"] as? String else {
            throw NSError(domain: "OfflineReceiptChallengeTests", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "fixtures/offline_allowance/ios-demo/summary.json is missing allowance_asset"
            ])
        }
        return value
    }

    private func sampleNonceHex() -> String {
        IrohaHash.hash(Data("receipt-nonce".utf8)).hexUppercased()
    }

    private func sampleSenderCertificateIdHex() -> String {
        IrohaHash.hash(Data("sender-certificate".utf8)).hexUppercased()
    }

    private func makeAssetId(name: String,
                             domain: String,
                             accountId: String) throws -> String {
        let definitionId: String
        switch (name, domain) {
        case ("xor", "sora"), ("xor", AccountAddress.defaultDomainName):
            definitionId = "61CtjvNd9T3THAR65GsMVHr82Bjc"
        default:
            XCTFail("unsupported test asset definition \(name)#\(domain)")
            throw OfflineReceiptChallenge.Error.invalidInput("unsupported test asset definition")
        }
        return try OfflineNorito.canonicalAssetIdLiteral("\(definitionId)#\(accountId)")
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }
}
