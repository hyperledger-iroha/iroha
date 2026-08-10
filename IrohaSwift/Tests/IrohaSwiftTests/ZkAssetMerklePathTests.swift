import XCTest
@testable import IrohaSwift

final class ZkAssetMerklePathTests: XCTestCase {
    private let signingSeed = Data(repeating: 0x41, count: 32)
    private var canonicalAuth: ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: try! Keypair(privateKeyBytes: signingSeed)
                .accountId(networkPrefix: AccountId.defaultNetworkPrefix),
            privateKey: signingSeed,
            timestampMs: 4_102_444_801_000,
            nonce: "zk-merkle-path-test"
        )
    }

    override func tearDown() {
        ZkMerklePathStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testPathValidationUsesNativeCanonicalV3() throws {
        let commitment = scalar(7)
        let sibling = scalar(11)
        let native = try ConfidentialNoteNativeDerivation.deriveMerklePathV3(
            commitments: [commitment, sibling],
            leafIndex: 0
        )
        let path = try ZkAssetMerklePath(
            leafIndex: 0,
            siblings: native.siblings,
            directions: native.directions,
            rootAtHeight: native.root,
            heightOrIndex: 1
        )

        XCTAssertTrue(try path.verify(commitment: commitment, expectedRoot: native.root))
        XCTAssertFalse(try path.verify(commitment: commitment, expectedRoot: scalar(12)))
        var wrongDirections = native.directions
        wrongDirections[0] = 1
        XCTAssertThrowsError(try ZkAssetMerklePath(
            leafIndex: 0,
            siblings: native.siblings,
            directions: wrongDirections,
            rootAtHeight: native.root,
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
        let commitments = [commitment, sibling]
        let root = try computeRoot(commitments)
        let local = try LocalZkAssetMerklePathProvider(
            rootHistory: [root],
            commitmentHistory: commitments
        )
        let localPath = try await local.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitment
        )
        let response = merklePathResponse(
            root: root,
            entries: [(commitment, localPath, localPath.siblings)]
        )

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
            session: URLSession(configuration: config),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
        )
        let path = try await client.getMerklePathForCommitment(
            asset: "usd#bank",
            commitment: commitment,
            canonicalAuth: canonicalAuth
        )
        XCTAssertEqual(path.rootAtHeight, root)
        XCTAssertEqual(path.siblings, localPath.siblings)
        XCTAssertEqual(path.directions, localPath.directions)
    }

    func testToriiClientFetchesRootsBoundToCommittedSnapshot() async throws {
        let root = scalar(7)
        let blockHash = Data(repeating: 0x0a, count: 32)
        let response = """
        {
          "latest": "\(root.hexEncodedString())",
          "roots": ["\(root.hexEncodedString())"],
          "evaluated_block_height": 7,
          "evaluated_block_hash": "\(blockHash.hexEncodedString())"
        }
        """.data(using: .utf8)!

        ZkMerklePathStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/zk/roots")
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
            session: URLSession(configuration: config),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
        )

        let roots = try await client.getZkAssetRoots(
            asset: "usd#bank", max: 3, canonicalAuth: canonicalAuth
        )
        XCTAssertEqual(roots.latest, root)
        XCTAssertEqual(roots.roots, [root])
        XCTAssertNoThrow(try roots.requireEvaluatedSnapshot(height: 7, blockHash: blockHash))
        XCTAssertThrowsError(try roots.requireEvaluatedSnapshot(height: 8, blockHash: blockHash))
        XCTAssertThrowsError(try roots.requireEvaluatedSnapshot(
            height: 7,
            blockHash: Data(repeating: 0x0b, count: 32)
        ))
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
            commitments: [commitment],
            canonicalAuth: canonicalAuth
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
            _ = try await client.getZkAssetMerklePaths(asset: "usd#bank", commitments: commitments, canonicalAuth: canonicalAuth)
            XCTFail("short response should fail")
        } catch ZkAssetMerklePathError.invalidField("paths") {}

        client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[0], first, first.siblings),
            (commitments[1], second, second.siblings),
            (commitments[0], first, first.siblings)
        ]))
        do {
            _ = try await client.getZkAssetMerklePaths(asset: "usd#bank", commitments: commitments, canonicalAuth: canonicalAuth)
            XCTFail("long response should fail")
        } catch ZkAssetMerklePathError.invalidField("paths") {}

        client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[1], second, second.siblings),
            (commitments[0], first, first.siblings)
        ]))
        do {
            _ = try await client.getZkAssetMerklePaths(asset: "usd#bank", commitments: commitments, canonicalAuth: canonicalAuth)
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
            _ = try await client.getMerklePathForCommitment(asset: "usd#bank", commitment: commitments[1], canonicalAuth: canonicalAuth)
            XCTFail("commitment mismatch should fail")
        } catch ZkAssetMerklePathError.invalidField("paths[0].commitment") {}

        var badSiblings = path.siblings
        badSiblings[0] = scalar(9)
        client = clientReturning(merklePathResponse(root: root, entries: [
            (commitments[1], path, badSiblings)
        ]))
        do {
            _ = try await client.getMerklePathForCommitment(asset: "usd#bank", commitment: commitments[1], canonicalAuth: canonicalAuth)
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
            siblings: Array(repeating: sibling, count: 16),
            directions: Array(repeating: "0", count: 16),
            witnessNodes: Array(repeating: sibling, count: 16),
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

    func testWitnessSnapshotRequiresBothReadinessHeightAndHash() throws {
        let root = String(repeating: "ab", count: 32)
        let blockHash = Data(repeating: 0x0a, count: 32)
        let response = try ToriiZkMerklePathResponse.decodeStrict(merklePathPayload(
            root: root,
            commitment: String(repeating: "02", count: 32),
            leafIndex: "0",
            siblings: Array(repeating: String(repeating: "00", count: 32), count: 16),
            directions: Array(repeating: "0", count: 16),
            witnessNodes: Array(repeating: String(repeating: "00", count: 32), count: 16),
            pathRoot: root
        ))

        XCTAssertNoThrow(try response.requireEvaluatedSnapshot(height: 7, blockHash: blockHash))
        XCTAssertThrowsError(try response.requireEvaluatedSnapshot(height: 8, blockHash: blockHash))
        XCTAssertThrowsError(try response.requireEvaluatedSnapshot(
            height: 7,
            blockHash: Data(repeating: 0x0b, count: 32)
        ))
    }

    private func scalar(_ value: UInt8) -> Data {
        var data = Data(repeating: 0, count: 32)
        data[0] = value
        return data
    }

    private func computeRoot(_ commitments: [Data]) throws -> Data {
        try ConfidentialNoteNativeDerivation.deriveMerklePathV3(
            commitments: commitments,
            leafIndex: 0
        ).root
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
            session: URLSession(configuration: config),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
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
          "evaluated_block_height": 7,
          "evaluated_block_hash": "\(String(repeating: "0a", count: 32))",
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
          "evaluated_block_height": 7,
          "evaluated_block_hash": "\(String(repeating: "0a", count: 32))",
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
