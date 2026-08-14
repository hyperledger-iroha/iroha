import CryptoKit
import XCTest
@testable import IrohaSwift

final class ZkAttachmentCanonicalAuthTests: XCTestCase {
    private let seed = Data(repeating: 0x31, count: 32)
    private let timestampMs: UInt64 = 4_102_444_801_000
    private let nonce = "swift-zk-attachment-auth"

    override func tearDown() {
        StubURLProtocol.handler = nil
        super.tearDown()
    }

    private var accountId: String {
        try! Keypair(privateKeyBytes: seed)
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    private var auth: ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: accountId,
            privateKey: seed,
            timestampMs: timestampMs,
            nonce: nonce
        )
    }

    private func makeClient(withContext: Bool = true) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [StubURLProtocol.self]
        return ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: URLSession(configuration: configuration),
            localSigningContext: withContext
                ? ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
                : nil
        )
    }

    func testAttachmentLifecycleUsesExactNetworkBodyAndEncodedPath() async throws {
        var requests: [URLRequest] = []
        StubURLProtocol.handler = { request in
            requests.append(request)
            let method = request.httpMethod ?? "GET"
            let path = request.url!.absoluteString
            let status: Int
            let body: Data
            let contentType: String?
            if method == "POST" {
                status = 201
                body = Data(#"{"id":"att/1","content_type":"text/plain","size":7,"created_ms":1}"#.utf8)
                contentType = "application/json"
            } else if method == "GET" && path.hasSuffix("/v1/zk/attachments") {
                status = 200
                body = Data("[]".utf8)
                contentType = "application/json"
            } else if method == "GET" {
                status = 200
                body = Data("payload".utf8)
                contentType = "text/plain"
            } else {
                status = 204
                body = Data()
                contentType = nil
            }
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: status,
                httpVersion: nil,
                headerFields: contentType.map { ["Content-Type": $0] }
            )!
            return (response, body)
        }

        let client = makeClient()
        let payload = Data("payload".utf8)
        _ = try await client.uploadAttachment(
            data: payload,
            contentType: "text/plain",
            canonicalAuth: auth
        )
        _ = try await client.listAttachments(canonicalAuth: auth)
        _ = try await client.getAttachment(id: "att/1", canonicalAuth: auth)
        try await client.deleteAttachment(id: "att/1", canonicalAuth: auth)

        XCTAssertEqual(requests.count, 4)
        let expected: [(String, String, Data)] = [
            ("POST", "/v1/zk/attachments", payload),
            ("GET", "/v1/zk/attachments", Data()),
            ("GET", "/v1/zk/attachments/att%2F1", Data()),
            ("DELETE", "/v1/zk/attachments/att%2F1", Data()),
        ]
        let publicKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey
        for (request, expectation) in zip(requests, expected) {
            let (method, encodedPath, body) = expectation
            XCTAssertEqual(request.httpMethod, method)
            XCTAssertTrue(request.url!.absoluteString.contains(encodedPath))
            XCTAssertEqual(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerAccount),
                try AccountAddress.parseEncoded(accountId).canonicalHex()
            )
            let signature = try XCTUnwrap(
                request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature)
            )
            let signatureData = try XCTUnwrap(Data(base64Encoded: signature))
            let message = try ToriiCanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.canonical,
                method: method,
                url: request.url!,
                body: body,
                timestampMs: timestampMs,
                nonce: nonce
            )
            let foreign = try ToriiCanonicalRequest.signatureMessage(
                networkId: TestNetworkIds.other,
                method: method,
                url: request.url!,
                body: body,
                timestampMs: timestampMs,
                nonce: nonce
            )
            XCTAssertTrue(publicKey.isValidSignature(signatureData, for: message))
            XCTAssertFalse(publicKey.isValidSignature(signatureData, for: foreign))
            XCTAssertTrue(String(decoding: message, as: UTF8.self).contains("\n\(encodedPath)\n"))
        }
    }

    func testAttachmentAuthenticationRequiresLocalSigningContext() async {
        StubURLProtocol.handler = { _ in
            XCTFail("request must fail before dispatch")
            throw URLError(.cannotConnectToHost)
        }
        do {
            _ = try await makeClient(withContext: false).listAttachments(canonicalAuth: auth)
            XCTFail("missing signing context must fail")
        } catch let ToriiClientError.invalidPayload(reason) {
            XCTAssertTrue(reason.contains("ToriiLocalSigningContext"))
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }
}
