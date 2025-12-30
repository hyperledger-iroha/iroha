import XCTest
@testable import IrohaSwift
#if canImport(CryptoKit)
import CryptoKit
#endif

final class OfflineReceiptChallengeTests: XCTestCase {
    func testEncodeRejectsEmptyChainId() {
        XCTAssertThrowsError(
            try OfflineReceiptChallenge.encode(
                chainId: " ",
                invoiceId: "inv-empty",
                receiverAccountId: "alice@wonderland",
                assetId: "xor##alice@wonderland",
                amount: "1",
                issuedAtMs: 1_700_000_000_000,
                nonceHex: "00"
            )
        ) { error in
            guard case let OfflineReceiptChallenge.Error.invalidInput(message) = error else {
                return XCTFail("expected invalidInput error")
            }
            XCTAssertEqual(message, "chainId must not be empty")
        }
    }

    func testEncodeAcceptsScaledAmount() {
        XCTAssertNoThrow(
            try OfflineReceiptChallenge.encode(
                chainId: "testnet",
                invoiceId: "inv-frac",
                receiverAccountId: "alice@wonderland",
                assetId: "xor##alice@wonderland",
                amount: "1.5",
                issuedAtMs: 1_700_000_000_000,
                nonceHex: "00"
            )
        )
    }

    func testEncodeRejectsScaleMismatchWhenExpectedScaleProvided() {
        XCTAssertThrowsError(
            try OfflineReceiptChallenge.encode(
                chainId: "testnet",
                invoiceId: "inv-frac",
                receiverAccountId: "alice@wonderland",
                assetId: "xor##alice@wonderland",
                amount: "1.5",
                issuedAtMs: 1_700_000_000_000,
                nonceHex: "00",
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
        let result = try OfflineReceiptChallenge.encode(
            chainId: "testnet",
            invoiceId: "inv-swift-tests",
            receiverAccountId: "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs@wonderland",
            assetId: "xor##34mSYn6ySFTASoiVzNGuyBkedDbYTxqhobNmoDbzdhfaNtveqVrm8N49uoqtcRNvAUcapufe1@sora",
            amount: "250",
            issuedAtMs: 1_700_000_000_000,
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
}
