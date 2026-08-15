import XCTest
import CryptoKit
@testable import IrohaSwift

final class ToriiCanonicalRequestTests: XCTestCase {
    func testCanonicalQuerySorting() throws {
        let rendered = try ToriiCanonicalRequest.canonicalQueryString(from: "b=2&a=3&b=1&space=a+b")
        XCTAssertEqual(rendered, "a=3&b=1&b=2&space=a+b")
        XCTAssertEqual(try ToriiCanonicalRequest.canonicalQueryString(from: "&&b=2&&a=1&"), "a=1&b=2")
    }

    func testCanonicalQueryMatchesRustFormEncodingAndUtf8Ordering() throws {
        XCTAssertEqual(
            try ToriiCanonicalRequest.canonicalQueryString(from: "b=!*()~'&a=1"),
            "a=1&b=%21*%28%29%7E%27"
        )
        XCTAssertEqual(
            try ToriiCanonicalRequest.canonicalQueryString(from: "x=%41%zz%FF"),
            "x=A%25zz%EF%BF%BD"
        )
        XCTAssertEqual(
            try ToriiCanonicalRequest.canonicalQueryString(from: "\u{E000}=bmp&\u{10000}=supplementary"),
            "%EE%80%80=bmp&%F0%90%80%80=supplementary"
        )
        XCTAssertEqual(
            try ToriiCanonicalRequest.canonicalQueryString(from: "k=\u{10000}&k=\u{E000}"),
            "k=%EE%80%80&k=%F0%90%80%80"
        )
    }

    func testHeadersProduceVerifiableSignature() throws {
        let seed = Data(repeating: 7, count: 32)
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let url = URL(string: "https://example.com/v1/accounts/\(accountId)/assets?limit=5")!
        let body = Data("{\"account_id\":\"\(accountId)\"}".utf8)
        let timestampMs: UInt64 = 1_717_171_717_000
        let nonce = "swift-torii-canonical-nonce"

        let headers = try ToriiCanonicalRequest.buildHeaders(
            method: "get",
            url: url,
            body: body,
            accountId: accountId,
            privateKey: seed,
            networkId: TestNetworkIds.canonical,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let message = try ToriiCanonicalRequest.signatureMessage(
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
        let accountHeader = try XCTUnwrap(headers[ToriiCanonicalRequest.headerAccount])
        XCTAssertEqual(
            accountHeader,
            try AccountAddress.parseEncoded(accountId).canonicalHex()
        )
        XCTAssertTrue(accountHeader.utf8.allSatisfy({ $0 <= 0x7f }))
        XCTAssertEqual(headers[ToriiCanonicalRequest.headerTimestampMs], String(timestampMs))
        XCTAssertEqual(headers[ToriiCanonicalRequest.headerNonce], nonce)
        XCTAssertEqual(String(data: body, encoding: .utf8), "{\"account_id\":\"\(accountId)\"}")
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))

        let aliasHeaders = try ToriiCanonicalRequest.buildHeaders(
            method: "get",
            url: url,
            accountId: "alice@universal",
            privateKey: seed,
            networkId: TestNetworkIds.canonical,
            timestampMs: timestampMs,
            nonce: "swift-torii-alias-nonce"
        )
        XCTAssertEqual(aliasHeaders[ToriiCanonicalRequest.headerAccount], "alice@universal")
    }

    func testExactNetworkHeadersCannotReplayAcrossGenesisHashes() throws {
        let seed = Data(repeating: 7, count: 32)
        let url = URL(string: "https://example.com/v1/gov/ballots/plain")!
        let body = Data("{\"network_id\":\"exact\"}".utf8)
        let timestampMs: UInt64 = 1_717_171_717_000
        let nonce = "swift-governance-network-nonce"
        let canonical = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "POST",
            url: url,
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let foreign = try ToriiCanonicalRequest.signatureMessage(
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

    func testV1QueryLimitsAcceptExactAndRejectPlusOne() throws {
        let exactPairs = (0..<ToriiCanonicalRequest.maxQueryPairsV1)
            .map { "k\($0)=v" }
            .joined(separator: "&")
        _ = try ToriiCanonicalRequest.canonicalQueryString(from: exactPairs)
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.canonicalQueryString(from: "\(exactPairs)&overflow=v")
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .tooManyQueryPairs)
        }
        _ = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            url: try XCTUnwrap(URL(string: "https://example.com/v1/accounts?\(exactPairs)")),
            timestampMs: 1,
            nonce: "pair-limit"
        )
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                url: XCTUnwrap(URL(string: "https://example.com/v1/accounts?\(exactPairs)&overflow=v")),
                timestampMs: 1,
                nonce: "pair-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .tooManyQueryPairs)
        }

        let exactBytes = String(repeating: "x", count: ToriiCanonicalRequest.maxRawQueryBytesV1)
        XCTAssertEqual(exactBytes.utf8.count, ToriiCanonicalRequest.maxRawQueryBytesV1)
        _ = try ToriiCanonicalRequest.canonicalQueryString(from: exactBytes)
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.canonicalQueryString(from: exactBytes + "x")
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .queryTooLarge)
        }
        _ = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            url: try XCTUnwrap(URL(string: "https://example.com/v1/accounts?\(exactBytes)")),
            timestampMs: 1,
            nonce: "byte-limit"
        )
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                url: XCTUnwrap(URL(string: "https://example.com/v1/accounts?\(exactBytes)x")),
                timestampMs: 1,
                nonce: "byte-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .queryTooLarge)
        }
    }

    func testV1MethodLimitAcceptsExactAndRejectsPlusOne() throws {
        let url = try XCTUnwrap(URL(string: "https://example.com/v1/test"))
        _ = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: String(repeating: "A", count: ToriiCanonicalRequest.maxMethodBytesV1),
            url: url,
            timestampMs: 1,
            nonce: "method-limit"
        )
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: String(repeating: "A", count: ToriiCanonicalRequest.maxMethodBytesV1 + 1),
                url: url,
                timestampMs: 1,
                nonce: "method-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .methodTooLarge)
        }

        for method in ["", "po st", "post/get", "méthod", "get\n"] {
            XCTAssertThrowsError(
                try ToriiCanonicalRequest.canonicalRequestMessage(
                    method: method,
                    url: url
                )
            ) { error in
                XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidMethod)
            }
        }
    }

    func testV1PathLimitAcceptsExactAndRejectsPlusOne() throws {
        let exactPath = "/" + String(
            repeating: "x",
            count: ToriiCanonicalRequest.maxPathBytesV1 - 1
        )
        _ = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            url: try XCTUnwrap(URL(string: "https://example.com\(exactPath)")),
            timestampMs: 1,
            nonce: "path-limit"
        )

        let excessivePath = "/" + String(
            repeating: "x",
            count: ToriiCanonicalRequest.maxPathBytesV1
        )
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                url: XCTUnwrap(URL(string: "https://example.com\(excessivePath)")),
                timestampMs: 1,
                nonce: "path-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .pathTooLarge)
        }

        let invalidTargets = [
            "v1/test",
            "//evil.example/v1/test",
            "ftp://example.com/v1/test",
            "https://example.com//evil/v1/test",
            "https://example.com/v1/test#fragment",
        ]
        for literal in invalidTargets {
            let invalidURL = try XCTUnwrap(URL(string: literal), literal)
            XCTAssertThrowsError(
                try ToriiCanonicalRequest.canonicalRequestMessage(
                    method: "get",
                    url: invalidURL
                ),
                literal
            ) { error in
                XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidPath)
            }
        }

        for (raw, encoded) in [
            (" ", "%20"), ("<", "%3C"), (">", "%3E"),
            ("[", "%5B"), ("]", "%5D"), ("^", "%5E"),
            ("`", "%60"), ("{", "%7B"), ("|", "%7C"), ("}", "%7D"),
        ] {
            let url = try XCTUnwrap(URL(string: "https://example.com/v1/a\(raw)b"))
            let request = URLRequest(url: url)
            XCTAssertEqual(request.url?.absoluteString, "https://example.com/v1/a\(encoded)b")
            let message = try ToriiCanonicalRequest.canonicalRequestMessage(
                method: "get",
                url: url
            )
            XCTAssertTrue(
                String(decoding: message, as: UTF8.self)
                    .hasPrefix("GET\n/v1/a\(encoded)b\n")
            )
        }
    }

    func testV1AccountAndNonceLimitsAcceptExactAndRejectPlusOne() throws {
        let seed = Data(repeating: 10, count: 32)
        let url = try XCTUnwrap(URL(string: "https://example.com/v1/accounts"))
        let validAlias = String(repeating: "a", count: 63) + "@universal"
        _ = try ToriiCanonicalRequest.buildHeaders(
            method: "get",
            url: url,
            accountId: validAlias,
            privateKey: seed,
            networkId: TestNetworkIds.canonical,
            timestampMs: 1,
            nonce: "account-limit"
        )

        let oversizedAlias = String(
            repeating: "a",
            count: ToriiCanonicalRequest.maxAccountLiteralBytesV1 - 2
        ) + "@a"
        XCTAssertEqual(oversizedAlias.utf8.count, ToriiCanonicalRequest.maxAccountLiteralBytesV1)
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.buildHeaders(
                method: "get",
                url: url,
                accountId: oversizedAlias,
                privateKey: seed,
                networkId: TestNetworkIds.canonical,
                timestampMs: 1,
                nonce: "oversized-alias"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidAccountId)
        }
        XCTAssertThrowsError(
            try ToriiCanonicalRequest.buildHeaders(
                method: "get",
                url: url,
                accountId: "a" + oversizedAlias,
                privateKey: seed,
                networkId: TestNetworkIds.canonical,
                timestampMs: 1,
                nonce: "account-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiCanonicalRequestError, .accountTooLarge)
        }

        let exactNonce = String(repeating: "n", count: 256)
        _ = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            url: url,
            timestampMs: 1,
            nonce: exactNonce
        )
        for nonce in [exactNonce + "n", "internal space", "control\u{0001}", "nönce"] {
            XCTAssertThrowsError(
                try ToriiCanonicalRequest.signatureMessage(
                    networkId: TestNetworkIds.canonical,
                    method: "get",
                    url: url,
                    timestampMs: 1,
                    nonce: nonce
                )
            ) { error in
                XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidNonce)
            }
        }
    }

    func testHeadersUseStructuralAliasPreflight() throws {
        let seed = Data(repeating: 10, count: 32)
        let url = try XCTUnwrap(URL(string: "https://example.com/v1/accounts"))
        for alias in [
            "xn--alice@universal",
            "xn--a@universal",
            "alice@xn--ab-uuba211bca8057b",
            "alice@xn--ab-j1t",
            "alice@xn--11b2er09f",
            "alice@xn--4u8c",
            "alice@xn--pq1d",
            "alice@xn--kx7e",
            "alice@xn--5h0f",
            "alice@xn--zo5h",
            "alice@xn--fi3d",
            "alice@xn--d4f",
        ] {
            _ = try ToriiCanonicalRequest.buildHeaders(
                method: "get",
                url: url,
                accountId: alias,
                privateKey: seed,
                networkId: TestNetworkIds.canonical,
                timestampMs: 1,
                nonce: "structural-alias"
            )
        }
        let invalidAliases = [
            "alice",
            "Alice@universal",
            "alice@Universal",
            "alice@bank.universal.extra",
            "alice@univérsal",
            "ab--invalid@universal",
            "alice@xn--",
            "\(String(repeating: "a", count: 64))@universal",
            "alice@\(String(repeating: "a", count: 64))",
        ]

        for alias in invalidAliases {
            XCTAssertThrowsError(
                try ToriiCanonicalRequest.buildHeaders(
                    method: "get",
                    url: url,
                    accountId: alias,
                    privateKey: seed,
                    networkId: TestNetworkIds.canonical,
                    timestampMs: 1,
                    nonce: "invalid-alias"
                ),
                alias
            ) { error in
                XCTAssertEqual(error as? ToriiCanonicalRequestError, .invalidAccountId)
            }
        }
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
                accountId: "alice@universal",
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
