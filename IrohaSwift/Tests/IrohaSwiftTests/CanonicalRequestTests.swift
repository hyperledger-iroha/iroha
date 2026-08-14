import XCTest
@testable import IrohaSwift
import CryptoKit

final class CanonicalRequestTests: XCTestCase {
    func testCanonicalQuerySorting() throws {
        let rendered = try CanonicalRequest.canonicalQueryString(from: "b=2&a=3&b=1&space=a+b")
        XCTAssertEqual(rendered, "a=3&b=1&b=2&space=a+b")
        XCTAssertEqual(try CanonicalRequest.canonicalQueryString(from: "&&b=2&&a=1&"), "a=1&b=2")
    }

    func testCanonicalQueryEncodesNonAscii() throws {
        let cafe = "caf\u{00E9}"
        let rendered = try CanonicalRequest.canonicalQueryString(from: "name=\(cafe)")
        XCTAssertEqual(rendered, "name=caf%C3%A9")
    }

    func testCanonicalQueryMatchesRustFormEncodingAndUtf8Ordering() throws {
        XCTAssertEqual(
            try CanonicalRequest.canonicalQueryString(from: "b=!*()~'&a=1"),
            "a=1&b=%21*%28%29%7E%27"
        )
        XCTAssertEqual(
            try CanonicalRequest.canonicalQueryString(from: "x=%41%zz%FF"),
            "x=A%25zz%EF%BF%BD"
        )
        XCTAssertEqual(
            try CanonicalRequest.canonicalQueryString(from: "\u{E000}=bmp&\u{10000}=supplementary"),
            "%EE%80%80=bmp&%F0%90%80%80=supplementary"
        )
        XCTAssertEqual(
            try CanonicalRequest.canonicalQueryString(from: "k=\u{10000}&k=\u{E000}"),
            "k=%EE%80%80&k=%F0%90%80%80"
        )
    }

    func testSigningHeadersAreVerifiable() throws {
        let seed = Data(repeating: 5, count: 32)
        let signingKey = try SigningKey.ed25519(privateKey: seed)
        let timestampMs: UInt64 = 1_717_171_717_000
        let nonce = "swift-canonical-nonce"
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let body = Data("{\"account_id\":\"\(accountId)\"}".utf8)
        let message = try CanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts/\(accountId)/assets",
            query: "limit=1",
            body: body,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let headers = try CanonicalRequest.signingHeaders(
            accountId: accountId,
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts/\(accountId)/assets",
            query: "limit=1",
            body: body,
            signer: signingKey,
            timestampMs: timestampMs,
            nonce: nonce
        )
        let sigB64 = headers["X-Iroha-Signature"] ?? ""
        let signature = Data(base64Encoded: sigB64) ?? Data()
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: signingKey.publicKey())
        let accountHeader = try XCTUnwrap(headers["X-Iroha-Account"])
        XCTAssertEqual(
            accountHeader,
            try AccountAddress.parseEncoded(accountId).canonicalHex()
        )
        XCTAssertTrue(accountHeader.utf8.allSatisfy({ $0 <= 0x7f }))
        XCTAssertEqual(headers["X-Iroha-Timestamp-Ms"], String(timestampMs))
        XCTAssertEqual(headers["X-Iroha-Nonce"], nonce)
        XCTAssertEqual(String(data: body, encoding: .utf8), "{\"account_id\":\"\(accountId)\"}")
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))

        let aliasHeaders = try CanonicalRequest.signingHeaders(
            accountId: "alice@universal",
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts",
            signer: signingKey,
            timestampMs: timestampMs,
            nonce: "swift-alias-nonce"
        )
        XCTAssertEqual(aliasHeaders["X-Iroha-Account"], "alice@universal")
    }

    func testV1QueryLimitsAcceptExactAndRejectPlusOne() throws {
        let exactPairs = (0..<CanonicalRequest.maxQueryPairsV1)
            .map { "k\($0)=v" }
            .joined(separator: "&")
        _ = try CanonicalRequest.canonicalQueryString(from: exactPairs)
        XCTAssertThrowsError(
            try CanonicalRequest.canonicalQueryString(from: "\(exactPairs)&overflow=v")
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .tooManyQueryPairs)
        }
        _ = try CanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts",
            query: exactPairs,
            timestampMs: 1,
            nonce: "pair-limit"
        )
        XCTAssertThrowsError(
            try CanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                query: "\(exactPairs)&overflow=v",
                timestampMs: 1,
                nonce: "pair-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .tooManyQueryPairs)
        }

        let exactBytes = String(repeating: "x", count: CanonicalRequest.maxRawQueryBytesV1)
        XCTAssertEqual(exactBytes.utf8.count, CanonicalRequest.maxRawQueryBytesV1)
        _ = try CanonicalRequest.canonicalQueryString(from: exactBytes)
        XCTAssertThrowsError(
            try CanonicalRequest.canonicalQueryString(from: exactBytes + "x")
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .queryTooLarge)
        }
        _ = try CanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts",
            query: exactBytes,
            timestampMs: 1,
            nonce: "byte-limit"
        )
        XCTAssertThrowsError(
            try CanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                query: exactBytes + "x",
                timestampMs: 1,
                nonce: "byte-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .queryTooLarge)
        }
    }

    func testSigningHeadersEnforcesPreparedWireQueryByteLimit() throws {
        let rawQuery = "x=" + String(repeating: "é", count: 32_767)
        XCTAssertEqual(rawQuery.utf8.count, CanonicalRequest.maxRawQueryBytesV1)
        _ = try CanonicalRequest.canonicalQueryString(from: rawQuery)
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 9, count: 32))

        XCTAssertThrowsError(
            try CanonicalRequest.signingHeaders(
                accountId: "alice@universal",
                networkId: TestNetworkIds.canonical,
                method: "GET",
                path: "/v1/test",
                query: rawQuery,
                signer: signingKey,
                timestampMs: 1,
                nonce: "prepared-query-cap"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .queryTooLarge)
        }
    }

    func testV1MethodLimitAcceptsExactAndRejectsPlusOne() throws {
        _ = try CanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: String(repeating: "A", count: CanonicalRequest.maxMethodBytesV1),
            path: "/v1/test",
            timestampMs: 1,
            nonce: "method-limit"
        )
        XCTAssertThrowsError(
            try CanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: String(repeating: "A", count: CanonicalRequest.maxMethodBytesV1 + 1),
                path: "/v1/test",
                timestampMs: 1,
                nonce: "method-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .methodTooLarge)
        }

        for method in ["", "po st", "post/get", "méthod", "get\n"] {
            XCTAssertThrowsError(
                try CanonicalRequest.canonicalMessage(
                    method: method,
                    path: "/v1/test"
                )
            ) { error in
                XCTAssertEqual(error as? CanonicalRequestError, .invalidMethod)
            }
        }
    }

    func testV1PathLimitAcceptsExactAndRejectsPlusOne() throws {
        _ = try CanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/" + String(repeating: "x", count: CanonicalRequest.maxPathBytesV1 - 1),
            timestampMs: 1,
            nonce: "path-limit"
        )
        XCTAssertThrowsError(
            try CanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/" + String(repeating: "x", count: CanonicalRequest.maxPathBytesV1),
                timestampMs: 1,
                nonce: "path-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .pathTooLarge)
        }

        let validEscapedPath = "/v1/a%2Fb/%20/query:http/!$&'()*+,-.;=@A_Z~/%FF/%00"
        let escapedMessage = try CanonicalRequest.canonicalMessage(
            method: "get",
            path: validEscapedPath
        )
        XCTAssertTrue(
            String(decoding: escapedMessage, as: UTF8.self)
                .hasPrefix("GET\n\(validEscapedPath)\n")
        )
        for path in [
            "",
            "v1/test",
            "//example.test/v1/test",
            "/v1/test?admin=1",
            "/v1/test#fragment",
            "/v1/raw path",
            "/v1/a<b",
            "/v1/a>b",
            "/v1/a[b",
            "/v1/a]b",
            "/v1/a^b",
            "/v1/a`b",
            "/v1/a{b",
            "/v1/a|b",
            "/v1/a}b",
            "/v1/./admin",
            "/v1/../admin",
            "/v1/%2E/admin",
            "/v1/%2e%2e/admin",
            "/v1/%2",
            "/v1/%GG",
            "/v1\\test",
            "/v1/tést",
        ] {
            XCTAssertThrowsError(
                try CanonicalRequest.canonicalMessage(method: "get", path: path)
            ) { error in
                XCTAssertEqual(error as? CanonicalRequestError, .invalidPath)
            }
        }
    }

    func testV1AccountAndNonceLimitsAcceptExactAndRejectPlusOne() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 9, count: 32))
        let validAlias = String(repeating: "a", count: 63) + "@universal"
        _ = try CanonicalRequest.signingHeaders(
            accountId: validAlias,
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts",
            signer: signingKey,
            timestampMs: 1,
            nonce: "account-limit"
        )

        let oversizedAlias = String(
            repeating: "a",
            count: CanonicalRequest.maxAccountLiteralBytesV1 - 2
        ) + "@a"
        XCTAssertEqual(oversizedAlias.utf8.count, CanonicalRequest.maxAccountLiteralBytesV1)
        XCTAssertThrowsError(
            try CanonicalRequest.signingHeaders(
                accountId: oversizedAlias,
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                signer: signingKey,
                timestampMs: 1,
                nonce: "oversized-alias"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .invalidAccountId)
        }
        XCTAssertThrowsError(
            try CanonicalRequest.signingHeaders(
                accountId: "a" + oversizedAlias,
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                signer: signingKey,
                timestampMs: 1,
                nonce: "account-limit-plus-one"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .accountTooLarge)
        }

        let exactNonce = String(repeating: "n", count: 256)
        _ = try CanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts",
            timestampMs: 1,
            nonce: exactNonce
        )
        for nonce in [exactNonce + "n", "internal space", "control\u{0001}", "nönce"] {
            XCTAssertThrowsError(
                try CanonicalRequest.signatureMessage(
                    networkId: TestNetworkIds.canonical,
                    method: "get",
                    path: "/v1/accounts",
                    timestampMs: 1,
                    nonce: nonce
                )
            ) { error in
                XCTAssertEqual(error as? CanonicalRequestError, .invalidNonce)
            }
        }
    }

    func testSigningUsesStructuralAliasHeaderPreflight() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 9, count: 32))
        for validAlias in [
            "alice@xn--fa-hia",
            "alice@xn--3xa",
            "alice@xn--11b2ezcw70k",
            "alice@xn--mgba3gch31f060k",
            "alice@xn--ngba7iz95i",
            "alice@xn--jqa59mba",
            "alice@xn--ab-0ea",
            "alice@xn--a-jib",
            "alice@xn--ab-3n4a",
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
            _ = try CanonicalRequest.signingHeaders(
                accountId: validAlias,
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                signer: signingKey,
                timestampMs: 1,
                nonce: "valid-ace-alias"
            )
        }
        let invalidAliases = [
            "alice",
            "0xalice@universal",
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
                try CanonicalRequest.signingHeaders(
                    accountId: alias,
                    networkId: TestNetworkIds.canonical,
                    method: "get",
                    path: "/v1/accounts",
                    signer: signingKey,
                    timestampMs: 1,
                    nonce: "invalid-alias"
                ),
                alias
            ) { error in
                XCTAssertEqual(error as? CanonicalRequestError, .invalidAccountId)
            }
        }
    }

    func testSigningRejectsPaddedAccountAndNonce() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 6, count: 32))

        XCTAssertThrowsError(
            try CanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                timestampMs: 1,
                nonce: " nonce"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .invalidNonce)
        }

        XCTAssertThrowsError(
            try CanonicalRequest.signingHeaders(
                accountId: " account",
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                signer: signingKey,
                timestampMs: 1,
                nonce: "nonce"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .invalidAccountId)
        }

        XCTAssertThrowsError(
            try CanonicalRequest.signingHeaders(
                accountId: "alice@universal",
                networkId: TestNetworkIds.canonical,
                method: "get",
                path: "/v1/accounts",
                signer: signingKey,
                timestampMs: 1,
                nonce: "nonce\n"
            )
        ) { error in
            XCTAssertEqual(error as? CanonicalRequestError, .invalidNonce)
        }
    }

    func testSigningRejectsNonV1AndControllerClassInconsistentI105Headers() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 6, count: 32))
        let malformedAccounts: [(String, AccountAddressError)] = [
            (
                "sora9vzVｴﾚﾃsﾏEﾐﾎab4MWgzsﾗﾎﾎｾﾄCﾔﾉｱﾖUﾋbﾍｹﾂW9ｹaﾒ3w9TUG68",
                .invalidHeaderVersion(1)
            ),
            (
                "sora21DヱnmﾐE9ｷﾂphVﾈ8Jpﾆ39ﾍnﾋBﾘﾛ2ﾖﾗﾅzHﾀTRwdｷLﾂjｼ3EJ9PD",
                .invalidNormVersion(2)
            ),
            (
                "sora3uｵﾔDｶﾕｽﾘｲfﾃfﾃﾉXヰﾏﾓZｸｵVfﾍbﾄﾊEｼTmｽWfﾂｴYXｸﾛxｺWHHMWJ",
                .unsupportedAddressFormat
            ),
        ]

        for (accountId, expectedError) in malformedAccounts {
            XCTAssertThrowsError(try AccountAddress.parseEncoded(accountId)) { error in
                XCTAssertEqual(error as? AccountAddressError, expectedError)
            }
            XCTAssertThrowsError(
                try CanonicalRequest.signingHeaders(
                    accountId: accountId,
                    networkId: TestNetworkIds.canonical,
                    method: "get",
                    path: "/v1/accounts",
                    signer: signingKey,
                    timestampMs: 1,
                    nonce: "malformed-account-header"
                )
            ) { error in
                XCTAssertEqual(error as? CanonicalRequestError, .invalidAccountId)
            }
        }
    }
}
