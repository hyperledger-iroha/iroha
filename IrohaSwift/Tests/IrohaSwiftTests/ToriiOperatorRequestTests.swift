import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiOperatorRequestTests: XCTestCase {
    private let privateKey = Data(repeating: 0x5A, count: 32)

    private func makeContext(
        networkId: NetworkId = TestNetworkIds.canonical
    ) throws -> ToriiOperatorSigningContext {
        try ToriiOperatorSigningContext(
            networkId: networkId,
            signingKey: SigningKey.ed25519(privateKey: privateKey)
        )
    }

    private func signatureMessage(
        networkId: NetworkId,
        method: String,
        url: URL,
        timestampMs: UInt64,
        nonce: String
    ) throws -> Data {
        var message = Data("iroha.operator.http-request.network.v1\0".utf8)
        message.append(networkId.bytes)
        message.append(
            try ToriiCanonicalRequest.canonicalRequestMessage(
                method: method,
                url: url,
                body: Data()
            )
        )
        message.append(Data("\n\(timestampMs)\n\(nonce)".utf8))
        return message
    }

    func testBuildHeadersSignsExactNetworkMethodPathQueryAndEmptyBody() throws {
        let context = try makeContext()
        let url = try XCTUnwrap(
            URL(string: "https://node.test/v1/pipeline/recovery/42?z=9&a=1")
        )
        let timestampMs: UInt64 = 4_102_444_801_000
        let nonce = "operator-request-test"

        let headers = try context.buildHeaders(
            method: "GET",
            url: url,
            timestampMs: timestampMs,
            nonce: nonce
        )

        XCTAssertEqual(headers["X-Iroha-Operator-Public-Key"], context.publicKey)
        XCTAssertEqual(headers["X-Iroha-Operator-Timestamp-Ms"], String(timestampMs))
        XCTAssertEqual(headers["X-Iroha-Operator-Nonce"], nonce)
        let signature = try XCTUnwrap(
            Data(base64Encoded: try XCTUnwrap(headers["X-Iroha-Operator-Signature"]))
        )
        let publicKey = try Curve25519.Signing.PublicKey(
            rawRepresentation: try SigningKey.ed25519(privateKey: privateKey).publicKey()
        )
        XCTAssertTrue(
            publicKey.isValidSignature(
                signature,
                for: try signatureMessage(
                    networkId: TestNetworkIds.canonical,
                    method: "GET",
                    url: url,
                    timestampMs: timestampMs,
                    nonce: nonce
                )
            )
        )
        XCTAssertFalse(
            publicKey.isValidSignature(
                signature,
                for: try signatureMessage(
                    networkId: TestNetworkIds.other,
                    method: "GET",
                    url: url,
                    timestampMs: timestampMs,
                    nonce: nonce
                )
            )
        )
        let otherPath = try XCTUnwrap(URL(string: "https://node.test/v1/policy?z=9&a=1"))
        XCTAssertFalse(
            publicKey.isValidSignature(
                signature,
                for: try signatureMessage(
                    networkId: TestNetworkIds.canonical,
                    method: "GET",
                    url: otherPath,
                    timestampMs: timestampMs,
                    nonce: nonce
                )
            )
        )
    }

    func testMakeOperatorGetRequestIsOneShotReadyAndRejectsFallbackHeaders() throws {
        let context = try makeContext()
        let client = ToriiClient(
            baseURL: try XCTUnwrap(URL(string: "https://node.test")),
            operatorSigningContext: context
        )
        let request = try client.makeOperatorGetRequest(
            path: "/v1/pipeline/recovery/42",
            queryItems: [
                URLQueryItem(name: "z", value: "9"),
                URLQueryItem(name: "a", value: "1"),
            ]
        )

        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertEqual(request.url?.path, "/v1/pipeline/recovery/42")
        XCTAssertEqual(request.url?.query, "z=9&a=1")
        XCTAssertTrue(request.httpBody?.isEmpty ?? true)
        for header in [
            "X-Iroha-Operator-Public-Key",
            "X-Iroha-Operator-Timestamp-Ms",
            "X-Iroha-Operator-Nonce",
            "X-Iroha-Operator-Signature",
        ] {
            XCTAssertNotNil(request.value(forHTTPHeaderField: header), header)
        }

        let fallbackClient = ToriiClient(
            baseURL: try XCTUnwrap(URL(string: "https://node.test")),
            defaultHeaders: ["Authorization": "Bearer retired"],
            operatorSigningContext: context
        )
        XCTAssertThrowsError(
            try fallbackClient.makeOperatorGetRequest(path: "/v1/time/status")
        ) { error in
            XCTAssertTrue(String(describing: error).contains("reject"))
        }
    }

    func testBuildHeadersRejectsInvalidNonceBeforeSigning() throws {
        let context = try makeContext()
        let url = try XCTUnwrap(URL(string: "https://node.test/v1/peers"))

        XCTAssertThrowsError(
            try context.buildHeaders(
                method: "GET",
                url: url,
                timestampMs: 1,
                nonce: "contains space"
            )
        ) { error in
            XCTAssertEqual(error as? ToriiOperatorRequestError, .invalidNonce)
        }
    }
}
