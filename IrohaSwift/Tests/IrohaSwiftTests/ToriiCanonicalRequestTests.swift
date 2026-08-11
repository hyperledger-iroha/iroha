import XCTest
import CryptoKit
@testable import IrohaSwift

final class ToriiCanonicalRequestTests: XCTestCase {
    func testCanonicalQuerySorting() {
        let rendered = ToriiCanonicalRequest.canonicalQueryString(from: "b=2&a=3&b=1&space=a+b")
        XCTAssertEqual(rendered, "a=3&b=1&b=2&space=a+b")
    }

    func testHeadersProduceVerifiableSignature() throws {
        let seed = Data(repeating: 7, count: 32)
        let url = URL(string: "https://example.com/v1/accounts/sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/assets?limit=5")!
        let body = Data("{\"foo\":1}".utf8)
        let timestampMs: UInt64 = 1_717_171_717_000
        let nonce = "swift-torii-canonical-nonce"

        let headers = try ToriiCanonicalRequest.buildHeaders(
            method: "get",
            url: url,
            body: body,
            accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            privateKey: seed,
            networkId: TestNetworkIds.canonical,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let message = ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            url: url,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        guard let signatureB64 = headers[ToriiCanonicalRequest.headerSignature],
              let signature = Data(base64Encoded: signatureB64) else {
            XCTFail("signature header missing")
            return
        }
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        XCTAssertEqual(headers[ToriiCanonicalRequest.headerTimestampMs], String(timestampMs))
        XCTAssertEqual(headers[ToriiCanonicalRequest.headerNonce], nonce)
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
    }

    func testExactNetworkHeadersCannotReplayAcrossGenesisHashes() throws {
        let seed = Data(repeating: 7, count: 32)
        let url = URL(string: "https://example.com/v1/gov/ballots/plain")!
        let body = Data("{\"network_id\":\"exact\"}".utf8)
        let timestampMs: UInt64 = 1_717_171_717_000
        let nonce = "swift-governance-network-nonce"
        let canonical = ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "POST",
            url: url,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let foreign = ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.other,
            method: "POST",
            url: url,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        XCTAssertNotEqual(canonical, foreign)
        let headers = try ToriiCanonicalRequest.buildHeaders(
            method: "POST",
            url: url,
            body: body,
            accountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            privateKey: seed,
            networkId: TestNetworkIds.canonical,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let signatureBase64 = try XCTUnwrap(headers[ToriiCanonicalRequest.headerSignature])
        let signature = try XCTUnwrap(Data(base64Encoded: signatureBase64))
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        XCTAssertTrue(publicKey.isValidSignature(signature, for: canonical))
        XCTAssertFalse(publicKey.isValidSignature(signature, for: foreign))
    }

    func testHeadersRejectPaddedAccountAndNonce() throws {
        let seed = Data(repeating: 8, count: 32)
        let url = URL(string: "https://example.com/v1/accounts")!

        XCTAssertThrowsError(
            try ToriiCanonicalRequest.buildHeaders(
                method: "get",
                url: url,
                accountId: " account",
                privateKey: seed,
                networkId: TestNetworkIds.canonical,
                timestampMs: 1,
                nonce: "nonce"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidAccountId)
        }

        XCTAssertThrowsError(
            try ToriiCanonicalRequest.buildHeaders(
                method: "get",
                url: url,
                accountId: "account",
                privateKey: seed,
                networkId: TestNetworkIds.canonical,
                timestampMs: 1,
                nonce: "nonce "
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidNonce)
        }
    }
}
