import Foundation
import XCTest
@testable import IrohaSwift

private final class ElectionTallyURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: NSError(domain: "ElectionTally", code: 1))
            return
        }
        do {
            let (response, body) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

final class ToriiElectionTallyV1Tests: XCTestCase {
    private let zeroHash = String(repeating: "0", count: 64)
    private let committedHash = String(repeating: "a", count: 64)
    private let maximumWeight = "340282366920938463463374607431768211455"
    private let signingSeed = Data(repeating: 0x41, count: 32)

    override func tearDown() {
        ElectionTallyURLProtocol.handler = nil
        super.tearDown()
    }

    private func response(
        height: String = "7",
        hash: String? = nil,
        finalized: String = "true",
        weights: String = "[9007199254740993,2]"
    ) -> Data {
        Data("""
        {"evaluated_block_height":\(height),"evaluated_block_hash":"\(hash ?? committedHash)","finalized":\(finalized),"tally":\(weights)}
        """.utf8)
    }

    private func makeClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ElectionTallyURLProtocol.self]
        return ToriiClient(
            baseURL: URL(string: "https://example.test")!,
            session: URLSession(configuration: configuration),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
        )
    }

    private func auth() throws -> ToriiCanonicalRequestAuth {
        let account = try Keypair(privateKeyBytes: signingSeed)
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
        return ToriiCanonicalRequestAuth(
            accountId: account,
            privateKey: signingSeed,
            timestampMs: 4_102_444_801_000,
            nonce: "election-tally-test"
        )
    }

    func testExactNumericTokensAndMaximumAggregate() throws {
        let decoded = try ToriiElectionTallyResponseV1.decodeExact(
            from: response(height: "18446744073709551615", weights: "[\(maximumWeight),0]")
        )
        XCTAssertEqual(decoded.evaluatedBlockHeight, UInt64.max)
        XCTAssertEqual(decoded.evaluatedBlockHash, committedHash)
        XCTAssertTrue(decoded.finalized)
        XCTAssertEqual(decoded.tally.map(\.decimalString), [maximumWeight, "0"])

        let atGenesis = try ToriiElectionTallyResponseV1.decodeExact(
            from: response(height: "0", hash: zeroHash, finalized: "false", weights: "[0,0]")
        )
        XCTAssertEqual(atGenesis.evaluatedBlockHeight, 0)
        XCTAssertFalse(atGenesis.finalized)
        XCTAssertEqual(
            try ToriiElectionTallyResponseV1.decodeExact(
                from: response(weights: "[\(Array(repeating: "0", count: 64).joined(separator: ","))]")
            ).tally.count,
            64
        )
    }

    func testStrictShapeAndIntegerBoundsRejectSubstitutions() {
        let invalid = [
            response(height: "18446744073709551616"),
            response(height: "7.0"),
            response(height: "1e1"),
            response(height: "\"7\""),
            response(height: "-1"),
            response(hash: String(repeating: "A", count: 64)),
            response(height: "0"),
            response(hash: zeroHash),
            response(finalized: "1"),
            response(weights: "[0]"),
            response(weights: "[\(Array(repeating: "0", count: 65).joined(separator: ","))]"),
            response(weights: "[\(maximumWeight),1]"),
            response(weights: "[340282366920938463463374607431768211456,0]"),
            response(weights: "[1.0,0]"),
            response(weights: "[1e0,0]"),
            response(weights: "[\"1\",0]"),
            response(weights: "[-0,0]"),
            Data("{\"evaluated_block_height\":7,\"evaluated_block_hash\":\"\(committedHash)\",\"finalized\":true,\"tally\":[0,0],\"extra\":0}".utf8),
            Data("{\"evaluated_block_height\":7,\"evaluated_block_hash\":\"\(committedHash)\",\"finalized\":true}".utf8),
            Data("{\"evaluated_block_height\":7,\"evaluated_block_height\":7,\"evaluated_block_hash\":\"\(committedHash)\",\"finalized\":true,\"tally\":[0,0]}".utf8),
            Data(repeating: 0x20, count: 8 * 1024 + 1),
        ]
        for (index, payload) in invalid.enumerated() {
            XCTAssertThrowsError(try ToriiElectionTallyResponseV1.decodeExact(from: payload), "case \(index)")
        }
    }

    func testSignedBoundedQueryAndAbsentElection() async throws {
        let authorization = try auth()
        let expectedBody = Data(#"{"election_id":"election-1"}"#.utf8)
        ElectionTallyURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/vote/tally")
            XCTAssertEqual(request.httpBody, expectedBody)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/json")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let expectedHeaders = try ToriiCanonicalRequest.buildHeaders(
                method: "POST",
                url: request.url!,
                body: expectedBody,
                accountId: authorization.accountId,
                privateKey: authorization.privateKey,
                networkId: TestNetworkIds.canonical,
                timestampMs: authorization.timestampMs!,
                nonce: authorization.nonce!
            )
            for (key, value) in expectedHeaders {
                XCTAssertEqual(request.value(forHTTPHeaderField: key), value)
            }
            return (
                HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                headerFields: ["Content-Type": "application/json"])!,
                self.response()
            )
        }
        let tally = try await makeClient().getElectionTally(id: "election-1", canonicalAuth: authorization)
        XCTAssertEqual(tally?.tally.map(\.decimalString), ["9007199254740993", "2"])

        ElectionTallyURLProtocol.handler = { request in
            (
                HTTPURLResponse(url: request.url!, statusCode: 404, httpVersion: nil,
                                headerFields: ["Content-Length": "0"])!,
                Data()
            )
        }
        let absent = try await makeClient().getElectionTally(id: "election-1", canonicalAuth: authorization)
        XCTAssertNil(absent)
    }

    func testInvalidSelectorAndResponseEnvelopeFailClosed() async throws {
        let authorization = try auth()
        var dispatched = false
        ElectionTallyURLProtocol.handler = { request in
            dispatched = true
            return (
                HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                headerFields: ["Content-Type": "application/json"])!,
                self.response()
            )
        }
        do {
            _ = try await makeClient().getElectionTally(id: ".alias", canonicalAuth: authorization)
            XCTFail("invalid selector reached transport")
        } catch {
            XCTAssertFalse(dispatched)
        }
        let cases: [(Int, [String: String], Data)] = [
            (200, ["Content-Type": "text/plain"], response()),
            (200, ["Content-Type": "application/json", "Content-Encoding": "gzip"], response()),
            (200, ["Content-Type": "application/json", "Content-Length": "8193"], response()),
            (200, ["Content-Type": "application/json"], Data(repeating: 0x20, count: 8 * 1024 + 1)),
            (404, [:], response()),
        ]
        for (index, (status, headers, body)) in cases.enumerated() {
            ElectionTallyURLProtocol.handler = { request in
                (
                    HTTPURLResponse(url: request.url!, statusCode: status,
                                    httpVersion: nil, headerFields: headers)!,
                    body
                )
            }
            do {
                _ = try await makeClient().getElectionTally(id: "election-1", canonicalAuth: authorization)
                XCTFail("invalid response \(index) was accepted")
            } catch {}
        }
    }
}
