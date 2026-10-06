import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiKagemushaWalletLoadIssuanceV1Tests: XCTestCase {
    private let seed = Data(repeating: 0x41, count: 32)
    private let network = try! NetworkId(literal: "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")
    private func selection() throws -> ToriiKagemushaWalletLoadSelectionV1 {
        try .init(schemeID: Data(repeating: 0xab, count: 32), walletID: Data(repeating: 0xcd, count: 32),
            requestID: Data(repeating: 0xef, count: 32))
    }
    private func auth() throws -> ToriiCanonicalRequestAuth {
        .init(accountId: try Keypair(privateKeyBytes: seed).accountId(networkPrefix: AccountId.defaultNetworkPrefix),
            privateKey: seed)
    }
    private func response(url: URL, headers: [String: String] = ["Content-Type": "application/x-norito"],
        status: Int = 200) -> HTTPURLResponse {
        HTTPURLResponse(url: url, statusCode: status, httpVersion: nil, headerFields: headers)!
    }
    private func original(_ bytes: Data, response: HTTPURLResponse, expected: URL) throws
        -> ToriiKagemushaWalletLoadIssuanceOriginalV1 {
        try .init(selection: selection(), payerAccountID: auth().accountId, networkID: network,
            expectedURL: expected, response: response, bytes: bytes)
    }

    func testSelectorsAreExactNonzero32Bytes() throws {
        let valid = Data(repeating: 1, count: 32)
        for bad in [Data(), Data(repeating: 1, count: 31), Data(repeating: 1, count: 33), Data(repeating: 0, count: 32)] {
            XCTAssertThrowsError(try ToriiKagemushaWalletLoadSelectionV1(schemeID: bad, walletID: valid, requestID: valid))
            XCTAssertThrowsError(try ToriiKagemushaWalletLoadSelectionV1(schemeID: valid, walletID: bad, requestID: valid))
            XCTAssertThrowsError(try ToriiKagemushaWalletLoadSelectionV1(schemeID: valid, walletID: valid, requestID: bad))
        }
    }

    func testExactEmptyGetUsesExistingAccountAndSigningNetwork() throws {
        let client = ToriiClient(baseURL: URL(string: "https://example.test/torii/")!,
            localSigningContext: .init(networkId: network))
        let signer = try auth(), selected = try selection()
        let request = try client.makeKagemushaWalletLoadIssuanceRequestV1(selection: selected, canonicalAuth: signer)
        XCTAssertEqual(request.url?.path, "/torii" + selected.path)
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertNil(request.httpBody)
        XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Accept-Encoding"), "identity")
        let accountHeader = try XCTUnwrap(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount))
        XCTAssertEqual(accountHeader, try AccountAddress.parseEncoded(signer.accountId).canonicalHex())
        XCTAssertTrue(accountHeader.utf8.allSatisfy { $0 <= 0x7f })
        let timestamp = try XCTUnwrap(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerTimestampMs)).flatMapUInt64()
        let nonce = try XCTUnwrap(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce))
        let signature = try XCTUnwrap(Data(base64Encoded: XCTUnwrap(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))))
        let message = try ToriiCanonicalRequest.signatureMessage(networkId: network, method: "GET",
            url: XCTUnwrap(request.url), timestampMs: timestamp, nonce: nonce)
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        XCTAssertEqual(accountHeader, try AccountAddress.fromAccount(publicKey: publicKey.rawRepresentation).canonicalHex())
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
        var other = network.bytes; other[0] ^= 1
        let substituted = try ToriiCanonicalRequest.signatureMessage(networkId: .init(bytes: other), method: "GET",
            url: XCTUnwrap(request.url), timestampMs: timestamp, nonce: nonce)
        XCTAssertFalse(publicKey.isValidSignature(signature, for: substituted))
    }

    func testEveryReadRequestGetsFreshNonceAndSignature() throws {
        let client = ToriiClient(baseURL: URL(string: "https://example.test")!,
            localSigningContext: .init(networkId: network))
        let first = try client.makeKagemushaWalletLoadIssuanceRequestV1(selection: selection(), canonicalAuth: auth())
        let second = try client.makeKagemushaWalletLoadIssuanceRequestV1(selection: selection(), canonicalAuth: auth())
        XCTAssertNotEqual(first.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce),
            second.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce))
        XCTAssertNotEqual(first.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature),
            second.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
    }

    func testChangedOwnerRejectsBeforeAnyRequestIsBuilt() async throws {
        enum OwnerFailure: Error { case changed }
        let client = ToriiClient(baseURL: URL(string: "https://example.test")!)
        do {
            _ = try await client.getKagemushaWalletLoadIssuanceOriginalV1(
                selection: selection(), canonicalAuth: auth(), requireCurrentOwner: { throw OwnerFailure.changed })
            XCTFail("changed owner must not proceed into signing or dispatch")
        } catch OwnerFailure.changed { }
    }

    func testMissingNetworkAndOfferedAuthHeadersAreRejected() throws {
        let unsigned = ToriiClient(baseURL: URL(string: "https://example.test")!)
        XCTAssertThrowsError(try unsigned.makeKagemushaWalletLoadIssuanceRequestV1(selection: selection(), canonicalAuth: auth()))
        for header in ["X-Iroha-Account", "x-iroha-signature", "X-Iroha-Timestamp-Ms", "x-iroha-nonce"] {
            let client = ToriiClient(baseURL: URL(string: "https://example.test")!, defaultHeaders: [header: "offered"],
                localSigningContext: .init(networkId: network))
            XCTAssertThrowsError(try client.makeKagemushaWalletLoadIssuanceRequestV1(selection: selection(), canonicalAuth: auth()))
        }
    }

    func testOriginalBytesArePreservedWithoutIssuanceOrCreditVerdict() throws {
        let url = URL(string: "https://example.test" + (try selection()).path)!
        let bytes = Data([0, 255, 0, 7]) // Transport-only fixture; Native must reject/admit its own actual codec.
        let value = try original(bytes, response: response(url: url), expected: url)
        XCTAssertEqual(value.canonicalResponseOriginal, bytes)
        XCTAssertEqual(value.payerAccountID, try auth().accountId)
        XCTAssertEqual(value.networkID, network)
    }

    func testRedirectJsonCompressionAndNonSuccessCannotProduceOriginal() throws {
        let url = URL(string: "https://example.test" + (try selection()).path)!
        let bytes = Data([1])
        XCTAssertThrowsError(try original(bytes, response: response(url: URL(string: "https://other.test")!), expected: url))
        for type in ["application/json", "application/x-norito; charset=binary", ""] {
            XCTAssertThrowsError(try original(bytes, response: response(url: url, headers: ["Content-Type": type]), expected: url))
        }
        XCTAssertThrowsError(try original(bytes, response: response(url: url,
            headers: ["Content-Type": "application/x-norito", "Content-Encoding": "gzip"]), expected: url))
        for status in [202, 401, 404, 406, 503] {
            XCTAssertThrowsError(try original(bytes, response: response(url: url, status: status), expected: url))
        }
    }

    func testMissingSourceOrEmptyBodyNeverBecomesAbsentWallet() throws {
        let url = URL(string: "https://example.test" + (try selection()).path)!
        XCTAssertThrowsError(try original(Data(), response: response(url: url), expected: url))
        XCTAssertThrowsError(try original(Data([1]), response: response(url: url, status: 503), expected: url))
    }

    func testFullResponseBoundAndLengthAreEnforced() throws {
        let url = URL(string: "https://example.test" + (try selection()).path)!
        XCTAssertThrowsError(try original(Data(repeating: 1, count: ToriiKagemushaWalletLoadIssuanceOriginalV1.maximumBytes + 1),
            response: response(url: url), expected: url))
        for length in ["0", "2", "-1", "+1", "1,1", String(repeating: "9", count: 40)] {
            XCTAssertThrowsError(try original(Data([1]), response: response(url: url,
                headers: ["Content-Type": "application/x-norito", "Content-Length": length]), expected: url))
        }
        XCTAssertEqual(try original(Data([1]), response: response(url: url,
            headers: ["Content-Type": "application/x-norito", "Content-Length": "1"]), expected: url).canonicalResponseOriginal, Data([1]))
    }
}

private extension String {
    func flatMapUInt64() throws -> UInt64 {
        try XCTUnwrap(UInt64(self))
    }
}
