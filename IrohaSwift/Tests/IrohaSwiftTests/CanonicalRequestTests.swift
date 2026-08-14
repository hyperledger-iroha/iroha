import XCTest
@testable import IrohaSwift
import CryptoKit

final class CanonicalRequestTests: XCTestCase {
    func testCanonicalQuerySorting() {
        let rendered = CanonicalRequest.canonicalQueryString(from: "b=2&a=3&b=1&space=a+b")
        XCTAssertEqual(rendered, "a=3&b=1&b=2&space=a+b")
        XCTAssertEqual(CanonicalRequest.canonicalQueryString(from: "&&b=2&&a=1&"), "a=1&b=2")
    }

    func testCanonicalQueryEncodesNonAscii() {
        let cafe = "caf\u{00E9}"
        let rendered = CanonicalRequest.canonicalQueryString(from: "name=\(cafe)")
        XCTAssertEqual(rendered, "name=caf%C3%A9")
    }

    func testCanonicalQueryMatchesRustFormEncodingAndUtf8Ordering() {
        XCTAssertEqual(
            CanonicalRequest.canonicalQueryString(from: "b=!*()~'&a=1"),
            "a=1&b=%21*%28%29%7E%27"
        )
        XCTAssertEqual(
            CanonicalRequest.canonicalQueryString(from: "x=%41%zz%FF"),
            "x=A%25zz%EF%BF%BD"
        )
        XCTAssertEqual(
            CanonicalRequest.canonicalQueryString(from: "\u{E000}=bmp&\u{10000}=supplementary"),
            "%EE%80%80=bmp&%F0%90%80%80=supplementary"
        )
        XCTAssertEqual(
            CanonicalRequest.canonicalQueryString(from: "k=\u{10000}&k=\u{E000}"),
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
    }

    func testV1AccountAndNonceLimitsAcceptExactAndRejectPlusOne() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 9, count: 32))
        let exactAccount = String(
            repeating: "a",
            count: CanonicalRequest.maxAccountLiteralBytesV1 - 2
        ) + "@a"
        XCTAssertEqual(exactAccount.utf8.count, CanonicalRequest.maxAccountLiteralBytesV1)
        _ = try CanonicalRequest.signingHeaders(
            accountId: exactAccount,
            networkId: TestNetworkIds.canonical,
            method: "get",
            path: "/v1/accounts",
            signer: signingKey,
            timestampMs: 1,
            nonce: "account-limit"
        )
        XCTAssertThrowsError(
            try CanonicalRequest.signingHeaders(
                accountId: "a" + exactAccount,
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
                accountId: "account",
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
