//! Canonical-auth hard-cut tests for protected runtime and governance requests.

import Foundation
import XCTest
@testable import IrohaSwift

final class RuntimeGovernanceCanonicalAuthTests: XCTestCase {
    private let seed = Data(repeating: 0x41, count: 32)

    override func tearDown() {
        RuntimeGovernanceURLProtocol.handler = nil
        super.tearDown()
    }

    private var auth: ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: try! Keypair(privateKeyBytes: seed)
                .accountId(networkPrefix: AccountId.defaultNetworkPrefix),
            privateKey: seed,
            timestampMs: 4_102_444_801_000,
            nonce: "runtime-governance-auth-test"
        )
    }

    private func client(networkId: NetworkId) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [RuntimeGovernanceURLProtocol.self]
        return ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: URLSession(configuration: configuration),
            localSigningContext: ToriiLocalSigningContext(networkId: networkId)
        )
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testSamePrincipalAndRouteBindDistinctGenesisNetworks() async throws {
        var signatures: [String] = []
        RuntimeGovernanceURLProtocol.handler = { request in
            signatures.append(try XCTUnwrap(request.value(forHTTPHeaderField: "X-Iroha-Signature")))
            let response = HTTPURLResponse(
                url: try XCTUnwrap(request.url), statusCode: 200,
                httpVersion: nil, headerFields: ["Content-Type": "application/json"]
            )!
            return (
                response,
                Data(#"{"abi_version":1,"upgrade_events_total":{"proposed":0,"activated":0,"canceled":0}}"#.utf8)
            )
        }

        _ = try await client(networkId: TestNetworkIds.canonical)
            .getRuntimeMetrics(canonicalAuth: auth)
        _ = try await client(networkId: TestNetworkIds.other)
            .getRuntimeMetrics(canonicalAuth: auth)

        XCTAssertEqual(signatures.count, 2)
        XCTAssertNotEqual(signatures[0], signatures[1])
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testNoncanonicalPrincipalFailsBeforeDispatch() async {
        var dispatchCount = 0
        RuntimeGovernanceURLProtocol.handler = { request in
            dispatchCount += 1
            return (
                HTTPURLResponse(url: request.url!, statusCode: 500,
                                httpVersion: nil, headerFields: nil)!,
                Data()
            )
        }
        let invalid = ToriiCanonicalRequestAuth(accountId: "alice", privateKey: seed)

        await XCTAssertThrowsErrorAsync(
            try await client(networkId: TestNetworkIds.canonical)
                .getRuntimeMetrics(canonicalAuth: invalid)
        ) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("unexpected error: \(error)")
            }
        }
        XCTAssertEqual(dispatchCount, 0)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testNonceBearingRequestDoesNotRedirectOrRetry() async {
        for status in [307, 308, 503] {
            var dispatchCount = 0
            RuntimeGovernanceURLProtocol.handler = { request in
                dispatchCount += 1
                let response = HTTPURLResponse(
                    url: request.url!, statusCode: status, httpVersion: nil,
                    headerFields: ["Location": "https://other.example/v1/runtime/metrics"]
                )!
                return (response, Data())
            }
            await XCTAssertThrowsErrorAsync(
                try await client(networkId: TestNetworkIds.canonical)
                    .getRuntimeMetrics(canonicalAuth: auth)
            ) { error in
                guard case let ToriiClientError.httpStatus(code, _, _) = error else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertEqual(code, status)
            }
            XCTAssertEqual(dispatchCount, 1, "status \(status) was replayed")
        }
    }
}

private final class RuntimeGovernanceURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        do {
            guard let handler = Self.handler else { throw URLError(.badServerResponse) }
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: data)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
