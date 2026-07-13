import XCTest
@testable import IrohaSwift

final class ZkAssetMerklePathTests: XCTestCase {
    override func tearDown() {
        ZkMerklePathStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testPathValidationUsesPastaPoseidonHasher() throws {
        let commitment = scalar(7)
        let sibling = scalar(11)
        let root = try PastaPoseidonNodeHasher().hashPair(left: commitment, right: sibling)
        let path = try ZkAssetMerklePath(
            leafIndex: 0,
            siblings: [sibling],
            directions: Data([0]),
            rootAtHeight: root,
            heightOrIndex: 1
        )

        XCTAssertTrue(try path.verify(commitment: commitment, expectedRoot: root))
        XCTAssertFalse(try path.verify(commitment: commitment, expectedRoot: scalar(12)))
        XCTAssertThrowsError(try ZkAssetMerklePath(
            leafIndex: 1,
            siblings: [sibling],
            directions: Data([0]),
            rootAtHeight: root,
            heightOrIndex: 1
        ))
    }

    func testLocalProviderComputesAndRejectsAmbiguousFrontiers() async throws {
        let commitments = [scalar(1), scalar(2), scalar(3)]
        let root = try computeRoot(commitments)
        let provider = try LocalZkAssetMerklePathProvider(
            rootHistory: [root],
            commitmentHistory: commitments
        )

        let path = try await provider.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitments[1]
        )

        XCTAssertEqual(path.leafIndex, 1)
        XCTAssertEqual(path.siblings.count, LocalZkAssetMerklePathProvider.confidentialTreeDepthV2)
        XCTAssertEqual(path.directions.count, LocalZkAssetMerklePathProvider.confidentialTreeDepthV2)
        XCTAssertEqual(path.rootAtHeight, root)
        XCTAssertTrue(try path.verify(commitment: commitments[1], expectedRoot: root))

        let duplicate = scalar(4)
        let duplicateProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [],
            commitmentHistory: [duplicate, duplicate]
        )
        do {
            _ = try await duplicateProvider.getMerklePathForCommitment(
                asset: "usd#bank",
                commitment: duplicate
            )
            XCTFail("duplicate commitment path should fail")
        } catch ZkAssetMerklePathError.invalidField("commitment") {}

        let mismatchedProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [scalar(9)],
            commitmentHistory: [scalar(1)]
        )
        do {
            _ = try await mismatchedProvider.getMerklePathForCommitment(
                asset: "usd#bank",
                commitment: scalar(1)
            )
            XCTFail("mismatched root history should fail")
        } catch ZkAssetMerklePathError.verificationFailed("rootHistory") {}
    }

    func testNextZeroPathAfterInsertionMatchesCompleteFrontier() throws {
        let before = [scalar(1), scalar(2), scalar(3)]
        let inserted = scalar(4)
        let beforeProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [],
            commitmentHistory: before
        )
        let insertionPath = try beforeProvider.nextZeroPath(asset: "usd#bank")
        let postInsertionRoot = try insertionPath.root(replacingLeafWith: inserted)
        let derived = try insertionPath.nextZeroPathAfterInsertion(
            commitment: inserted,
            expectedRoot: postInsertionRoot
        )

        let afterProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [postInsertionRoot],
            commitmentHistory: before + [inserted]
        )
        let complete = try afterProvider.nextZeroPath(asset: "usd#bank")
        XCTAssertEqual(derived, complete)
        XCTAssertTrue(try derived.verify(
            commitment: Data(repeating: 0, count: 32),
            expectedRoot: postInsertionRoot
        ))
        XCTAssertThrowsError(try insertionPath.nextZeroPathAfterInsertion(
            commitment: inserted,
            expectedRoot: scalar(99)
        ))
    }

    func testToriiClientFetchesAndValidatesMerklePath() async throws {
        let commitment = scalar(7)
        let sibling = scalar(11)
        let root = try PastaPoseidonNodeHasher().hashPair(left: commitment, right: sibling)
        let response = """
        {
          "root": "\(root.hexEncodedString())",
          "frontier_len": 1,
          "tree_depth": 1,
          "paths": [{
            "commitment": "\(commitment.hexEncodedString())",
            "leaf_index": 0,
            "siblings": ["\(sibling.hexEncodedString())"],
            "directions": [0],
            "witness_nodes": ["\(sibling.hexEncodedString())"],
            "root": "\(root.hexEncodedString())"
          }]
        }
        """.data(using: .utf8)!

        ZkMerklePathStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/merkle-path")
            let body = try requestBodyData(request)
            let object = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(object["asset_id"] as? String, "usd#bank")
            XCTAssertEqual(object["commitments"] as? [String], [commitment.hexEncodedString()])
            let http = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (http, response)
        }

        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ZkMerklePathStubURLProtocol.self]
        let client = ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: URLSession(configuration: config)
        )
        let path = try await client.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitment
        )
        XCTAssertEqual(path.rootAtHeight, root)
        XCTAssertEqual(path.siblings, [sibling])
        XCTAssertEqual(path.directions, Data([0]))
    }

    func testToriiSnapshotPreservesAndVerifiesAuthoritativeNextZeroPath() async throws {
        let commitment = scalar(7)
        let root = try computeRoot([commitment])
        let provider = try LocalZkAssetMerklePathProvider(
            rootHistory: [root],
            commitmentHistory: [commitment]
        )
        let inputPath = try await provider.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitment
        )
        let nextZeroPath = try provider.nextZeroPath(asset: "usd#bank")
        let client = clientReturning(merklePathResponse(
            root: root,
            entries: [(commitment, inputPath, inputPath.siblings)],
            frontierLen: 1,
            nextZeroPath: nextZeroPath
        ))

        let snapshot = try await client.getZkAssetMerklePathSnapshot(
            asset: "usd#bank",
            commitments: [commitment]
        )
        XCTAssertEqual(snapshot.nextZeroPath?.leafIndex, 1)
        XCTAssertEqual(try snapshot.validatedNextZeroPath(), nextZeroPath)
        let replacement = scalar(9)
        XCTAssertEqual(
            try snapshot.validatedNextZeroPath().root(replacingLeafWith: replacement),
            try nextZeroPath.root(replacingLeafWith: replacement)
        )
    }

    func testToriiClientRejectsPathCountDriftAndReorderedResponses() async throws {
        let commitments = [scalar(1), scalar(2)]
        let root = try computeRoot(commitments)
        let localProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [root],
            commitmentHistory: commitments
        )
        let first = try await localProvider.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitments[0]
        )
        let second = try await localProvider.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitments[1]
        )

        var client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[0], first, first.siblings)
        ]))
        do {
            _ = try await client.getZkAssetMerklePaths(asset: "usd#bank", commitments: commitments)
            XCTFail("short response should fail")
        } catch ZkAssetMerklePathError.invalidField("paths") {}

        client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[0], first, first.siblings),
            (commitments[1], second, second.siblings),
            (commitments[0], first, first.siblings)
        ]))
        do {
            _ = try await client.getZkAssetMerklePaths(asset: "usd#bank", commitments: commitments)
            XCTFail("long response should fail")
        } catch ZkAssetMerklePathError.invalidField("paths") {}

        client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[1], second, second.siblings),
            (commitments[0], first, first.siblings)
        ]))
        do {
            _ = try await client.getZkAssetMerklePaths(asset: "usd#bank", commitments: commitments)
            XCTFail("reordered response should fail")
        } catch ZkAssetMerklePathError.invalidField("paths[0].commitment") {}
    }

    func testToriiClientRejectsMismatchedAndNonVerifyingNodePaths() async throws {
        let commitments = [scalar(1), scalar(2)]
        let root = try computeRoot(commitments)
        let localProvider = try LocalZkAssetMerklePathProvider(
            rootHistory: [root],
            commitmentHistory: commitments
        )
        let path = try await localProvider.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitments[1]
        )

        var client = clientReturning(merklePathResponse(root: root, entries: [
            (scalar(9), path, path.siblings)
        ]))
        do {
            _ = try await client.getMerklePathForCommitment(asset: "usd#bank", commitment: commitments[1])
            XCTFail("commitment mismatch should fail")
        } catch ZkAssetMerklePathError.invalidField("paths[0].commitment") {}

        var badSiblings = path.siblings
        badSiblings[0] = scalar(9)
        client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[1], path, badSiblings)
        ]))
        do {
            _ = try await client.getMerklePathForCommitment(asset: "usd#bank", commitment: commitments[1])
            XCTFail("tampered sibling path should fail")
        } catch ZkAssetMerklePathError.verificationFailed("paths[0]") {}
    }

    func testStrictMerklePathResponseParserRejectsAdversarialJsonShapes() throws {
        let commitment = String(repeating: "02", count: 32)
        let sibling = String(repeating: "00", count: 32)
        let root = String(repeating: "ab", count: 32)
        let otherRoot = String(repeating: "cd", count: 32)
        let canonical = merklePathPayload(
            root: root,
            commitment: commitment,
            leafIndex: "0",
            siblings: [sibling],
            directions: ["0"],
            witnessNodes: [sibling],
            pathRoot: root
        )
        XCTAssertNoThrow(try ToriiZkMerklePathResponse.decodeStrict(canonical))

        let adversarialPayloads = [
            """
            {"root":"\(root)","frontier_len":3,"frontier_len":1,"tree_depth":1,"paths":[]}
            """,
            """
            {"root":"\(root)","frontier_len":3,"tree_depth":1,"paths":[{
              "commitment":"\(commitment)",
              "commitment":"\(otherRoot)",
              "leaf_index":0,
              "siblings":["\(sibling)"],
              "directions":[0],
              "witness_nodes":["\(sibling)"],
              "root":"\(root)"
            }]}
            """,
            """
            {"root":"\(root)","frontier_len":3,"tree_depth":1,"paths":[{
              "commitment":"\(commitment)",
              "leaf\\u005findex":0,
              "leaf_index":0,
              "siblings":["\(sibling)"],
              "directions":[0],
              "witness_nodes":["\(sibling)"],
              "root":"\(root)"
            }]}
            """,
            String(data: merklePathPayload(
                root: root.uppercased(),
                commitment: commitment,
                leafIndex: "0",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: root
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "0",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [],
                pathRoot: root
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "0",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: otherRoot
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "1",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: root
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "0",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: root,
                frontierLen: "2147483648"
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "0.0",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: root
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "0",
                siblings: [sibling],
                directions: ["0.0"],
                witnessNodes: [sibling],
                pathRoot: root
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "0",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: root,
                frontierLen: "1.0"
            ), encoding: .utf8)!,
            String(data: merklePathPayload(
                root: root,
                commitment: commitment,
                leafIndex: "\"0\"",
                siblings: [sibling],
                directions: ["0"],
                witnessNodes: [sibling],
                pathRoot: root
            ), encoding: .utf8)!
        ]

        for payload in adversarialPayloads {
            XCTAssertThrowsError(
                try ToriiZkMerklePathResponse.decodeStrict(Data(payload.utf8)),
                "payload should be rejected: \(payload)"
            )
        }
    }

    private func scalar(_ value: UInt8) -> Data {
        var data = Data(repeating: 0, count: 32)
        data[0] = value
        return data
    }

    private func computeRoot(_ commitments: [Data]) throws -> Data {
        var layer = commitments.map { Data($0) }
        while layer.count < LocalZkAssetMerklePathProvider.confidentialTreeCapacityV2 {
            layer.append(Data(repeating: 0, count: 32))
        }
        let hasher = PastaPoseidonNodeHasher()
        for _ in 0..<LocalZkAssetMerklePathProvider.confidentialTreeDepthV2 {
            var next: [Data] = []
            next.reserveCapacity(layer.count / 2)
            var index = 0
            while index < layer.count {
                next.append(try hasher.hashPair(left: layer[index], right: layer[index + 1]))
                index += 2
            }
            layer = next
        }
        XCTAssertEqual(layer.count, 1)
        return try XCTUnwrap(layer.first)
    }

    private func clientReturning(_ data: Data) -> ToriiClient {
        ZkMerklePathStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/merkle-path")
            let http = HTTPURLResponse(
                url: request.url!,
                statusCode: 200,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            return (http, data)
        }
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [ZkMerklePathStubURLProtocol.self]
        return ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: URLSession(configuration: config)
        )
    }

    private func merklePathResponse(
        root: Data,
        entries: [(commitment: Data, path: ZkAssetMerklePath, siblings: [Data])],
        frontierLen: Int = 2,
        nextZeroPath: ZkAssetMerklePath? = nil
    ) -> Data {
        let treeDepth = entries.first?.path.siblings.count ?? 0
        let paths = entries.map { entry in
            let siblings = entry.siblings.map { "\"\($0.hexEncodedString())\"" }.joined(separator: ",")
            let directions = entry.path.directions.map { String($0) }.joined(separator: ",")
            let witnessNodes = entry.path.siblings.map { "\"\($0.hexEncodedString())\"" }.joined(separator: ",")
            return """
            {
              "commitment": "\(entry.commitment.hexEncodedString())",
              "leaf_index": \(entry.path.leafIndex),
              "siblings": [\(siblings)],
              "directions": [\(directions)],
              "witness_nodes": [\(witnessNodes)],
              "root": "\(root.hexEncodedString())"
            }
            """
        }.joined(separator: ",")
        let nextZeroJSON = nextZeroPath.map { path -> String in
            let siblings = path.siblings.map { "\"\($0.hexEncodedString())\"" }
                .joined(separator: ",")
            let directions = path.directions.map { String($0) }.joined(separator: ",")
            let witnessNodes = path.siblings.map { "\"\($0.hexEncodedString())\"" }
                .joined(separator: ",")
            return """
            {
              "commitment": "\(Data(repeating: 0, count: 32).hexEncodedString())",
              "leaf_index": \(path.leafIndex),
              "siblings": [\(siblings)],
              "directions": [\(directions)],
              "witness_nodes": [\(witnessNodes)],
              "root": "\(root.hexEncodedString())"
            }
            """
        } ?? "null"
        let json = """
        {
          "root": "\(root.hexEncodedString())",
          "frontier_len": \(frontierLen),
          "tree_depth": \(treeDepth),
          "next_zero_path": \(nextZeroJSON),
          "paths": [\(paths)]
        }
        """
        return Data(json.utf8)
    }

    private func merklePathPayload(
        root: String,
        commitment: String,
        leafIndex: String,
        siblings: [String],
        directions: [String],
        witnessNodes: [String],
        pathRoot: String,
        frontierLen: String = "1",
        treeDepth: String? = nil
    ) -> Data {
        let siblingJson = siblings.map { "\"\($0)\"" }.joined(separator: ",")
        let directionJson = directions.joined(separator: ",")
        let witnessJson = witnessNodes.map { "\"\($0)\"" }.joined(separator: ",")
        let depth = treeDepth ?? String(siblings.count)
        let json = """
        {
          "root": "\(root)",
          "frontier_len": \(frontierLen),
          "tree_depth": \(depth),
          "paths": [{
            "commitment": "\(commitment)",
            "leaf_index": \(leafIndex),
            "siblings": [\(siblingJson)],
            "directions": [\(directionJson)],
            "witness_nodes": [\(witnessJson)],
            "root": "\(pathRoot)"
          }]
        }
        """
        return Data(json.utf8)
    }
}

private func requestBodyData(_ request: URLRequest) throws -> Data {
    if let body = request.httpBody {
        return body
    }
    guard let stream = request.httpBodyStream else {
        throw ZkAssetMerklePathError.invalidField("requestBody")
    }
    stream.open()
    defer { stream.close() }
    var out = Data()
    var buffer = [UInt8](repeating: 0, count: 4096)
    while stream.hasBytesAvailable {
        let read = stream.read(&buffer, maxLength: buffer.count)
        if read < 0 {
            throw stream.streamError ?? ZkAssetMerklePathError.invalidField("requestBody")
        }
        if read == 0 {
            break
        }
        out.append(buffer, count: read)
    }
    return out
}

private final class ZkMerklePathStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if let data {
                client?.urlProtocol(self, didLoad: data)
            }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}
