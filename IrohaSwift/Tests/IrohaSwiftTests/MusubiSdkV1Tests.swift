import Foundation
import XCTest
@testable import IrohaSwift

private final class MusubiV1StubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?
    static var lastRequest: URLRequest?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lastRequest = request
        do {
            guard let handler = Self.handler else {
                throw URLError(.badServerResponse)
            }
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

final class MusubiSdkV1Tests: XCTestCase {
    override func tearDown() {
        MusubiV1StubURLProtocol.handler = nil
        MusubiV1StubURLProtocol.lastRequest = nil
        super.tearDown()
    }

    func testCanonicalNamesVersionsAndRequirementsMatchRustFixture() throws {
        let root = try fixture()
        XCTAssertEqual(root["format"] as? String, "iroha-musubi-sdk-v1")
        XCTAssertEqual(root["fixture_version"] as? Int, 1)
        XCTAssertEqual(root["rust_owner"] as? String, "iroha_data_model::musubi")

        let canonical = try object(root["canonical"])
        let namespaceWire = try array(canonical["namespace"])
        let packageNameWire = try array(canonical["package_name"])
        let namespace = try MusubiNamespaceV1(try XCTUnwrap(namespaceWire.single as? String))
        let packageName = try MusubiPackageNameV1(
            try XCTUnwrap(packageNameWire.single as? String)
        )
        try assertWireEqual(canonical["namespace"], namespace)
        try assertWireEqual(canonical["package_name"], packageName)

        let version = try MusubiVersionV1.parse("1.2.3-rc.1")
        XCTAssertEqual(version.canonicalText, "1.2.3-rc.1")
        try assertWireEqual(canonical["version"], version)

        for raw in try array(canonical["requirements"]) {
            let item = try object(raw)
            let requirement = try MusubiVersionReqV1.parse(try XCTUnwrap(item["text"] as? String))
            try assertWireEqual(item["wire"], requirement)
        }
        for raw in try array(canonical["requirement_aliases"]) {
            let item = try object(raw)
            let requirement = try MusubiVersionReqV1.parse(
                try XCTUnwrap(item["input"] as? String)
            )
            XCTAssertEqual(requirement.canonicalText, item["canonical"] as? String)
            try assertWireEqual(item["wire"], requirement)
        }
        for raw in try array(canonical["requirement_matches"]) {
            let item = try object(raw)
            let requirement = try MusubiVersionReqV1.parse(
                try XCTUnwrap(item["requirement"] as? String)
            )
            let candidate = try MusubiVersionV1.parse(
                try XCTUnwrap(item["candidate"] as? String)
            )
            XCTAssertEqual(
                requirement.matches(candidate),
                try XCTUnwrap(item["matches"] as? Bool)
            )
        }
    }

    func testDecodedComparatorRequirementsRejectNoncanonicalExactForms() throws {
        let first = MusubiVersionComparatorV1(
            op: .equal,
            version: try MusubiVersionV1.parse("1.0.0")
        )
        let second = MusubiVersionComparatorV1(
            op: .equal,
            version: try MusubiVersionV1.parse("2.0.0")
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(MusubiVersionReqV1.comparators([first]))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(MusubiVersionReqV1.comparators([first, second]))
        )

        let singletonWire: [String: Any] = [
            "kind": "Comparators",
            "value": [[
                "op": ["kind": "Equal", "value": NSNull()],
                "version": ["major": 1, "minor": 0, "patch": 0, "prerelease": []],
            ]],
        ]
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiVersionReqV1.self,
                from: JSONSerialization.data(withJSONObject: singletonWire)
            )
        )
    }

    func testNameBackedFieldsRejectEveryUnicodeBidiControl() throws {
        let controls = [
            "\u{061C}", "\u{200E}", "\u{200F}",
            "\u{202A}", "\u{202B}", "\u{202C}", "\u{202D}", "\u{202E}",
            "\u{2066}", "\u{2067}", "\u{2068}", "\u{2069}",
        ]
        for control in controls {
            XCTAssertThrowsError(try MusubiNamespaceV1("domain\(control).dataspace"))
            XCTAssertThrowsError(
                try JSONEncoder().encode(MusubiPackageScopeV1.domain("domain\(control)"))
            )
        }
    }

    func testEveryTypedRouteRoundTripsExactRequestAndResponseJSON() throws {
        let routes = try fixtureRoutes()
        XCTAssertEqual(routes.count, 11)
        XCTAssertEqual(Set(try routes.map { try path($0) }), expectedPaths)
        for route in routes {
            let routePath = try path(route)
            let request = try jsonData(route["request"])
            let response = try jsonData(route["response"])
            XCTAssertEqual(
                try roundTripRequest(routePath, data: request),
                try jsonObject(request)
            )
            XCTAssertEqual(
                try roundTripResponse(routePath, data: response),
                try jsonObject(response)
            )
        }
    }

    func testArchiveRetentionIsBoundedTypedAndBindsTheExactRequest() throws {
        let fixtureRoute = try route(MusubiToriiClientV1.archiveRetentionPath)
        let decoder = JSONDecoder()
        let request = try decoder.decode(
            MusubiArchiveRetentionQueryV1.self,
            from: jsonData(fixtureRoute["request"])
        )
        let page = try decoder.decode(
            MusubiArchiveRetentionPageV1.self,
            from: jsonData(fixtureRoute["response"])
        )
        try page.requireMatches(request)
        XCTAssertEqual(page.items.count, 4)
        XCTAssertEqual(page.items.map(\.mustRetain), [true, true, false, false])

        var mismatched = try object(deepMutableCopy(fixtureRoute["response"]))
        var items = try array(mismatched["items"])
        var first = try object(items[0])
        first["archive_id"] = [Array(repeating: 17, count: 32)]
        items[0] = first
        mismatched["items"] = items
        let mismatchedPage = try decoder.decode(
            MusubiArchiveRetentionPageV1.self,
            from: jsonData(mismatched)
        )
        XCTAssertThrowsError(try mismatchedPage.requireMatches(request))
    }

    func testMaintainerDirectoryDecodesAcceptedAndPendingInvitationVariants() throws {
        let fixtureRoute = try route(MusubiToriiClientV1.maintainersPath)
        let page = try JSONDecoder().decode(
            MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
            from: jsonData(fixtureRoute["response"])
        )
        XCTAssertEqual(page.items.count, 2)

        guard case .accepted(let member) = page.items[0] else {
            return XCTFail("First maintainer-directory entry must be accepted.")
        }
        XCTAssertEqual(member.roleKind, "Owner")
        XCTAssertEqual(member.acceptedAtHeight, 42)

        guard case .pendingInvitation(let invitation) = page.items[1] else {
            return XCTFail("Second maintainer-directory entry must be a pending invitation.")
        }
        XCTAssertEqual(invitation.roleKind, "Maintainer")
        XCTAssertEqual(invitation.stateKind, "Pending")
        XCTAssertEqual(invitation.expectedGovernanceRevision, 2)
        XCTAssertEqual(invitation.inviteId.bytes, Array(repeating: 13, count: 32))

        var malformed = try object(deepMutableCopy(fixtureRoute["response"]))
        var items = try array(malformed["items"])
        var pending = try object(items[1])
        var pendingValue = try object(pending["value"])
        var state = try object(pendingValue["state"])
        state["kind"] = "Accepted"
        pendingValue["state"] = state
        pending["value"] = pendingValue
        items[1] = pending
        malformed["items"] = items
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
                from: jsonData(malformed)
            )
        )
    }

    func testReadOnlyClientPostsEachTypedQueryToExactV1Route() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MusubiV1StubURLProtocol.self]
        let client = MusubiToriiClientV1(
            baseURL: try XCTUnwrap(URL(string: "https://example.test/")),
            session: URLSession(configuration: configuration)
        )

        for route in try fixtureRoutes() {
            let routePath = try path(route)
            let expectedRequest = try jsonData(route["request"])
            let responseBody = try jsonData(route["response"])
            MusubiV1StubURLProtocol.handler = { request in
                let response = try XCTUnwrap(
                    HTTPURLResponse(
                        url: try XCTUnwrap(request.url),
                        statusCode: 200,
                        httpVersion: "HTTP/1.1",
                        headerFields: ["Content-Type": "application/json"]
                    )
                )
                return (response, responseBody)
            }

            try await invoke(client, path: routePath, request: expectedRequest)
            let captured = try XCTUnwrap(MusubiV1StubURLProtocol.lastRequest)
            XCTAssertEqual(captured.httpMethod, "POST")
            XCTAssertEqual(captured.url?.path, routePath)
            XCTAssertEqual(
                try jsonObject(try requestBody(captured)),
                try jsonObject(expectedRequest)
            )
        }
    }

    private func requestBody(_ request: URLRequest) throws -> Data {
        if let body = request.httpBody {
            return body
        }
        let stream = try XCTUnwrap(request.httpBodyStream)
        stream.open()
        defer { stream.close() }

        var body = Data()
        var buffer = [UInt8](repeating: 0, count: 4_096)
        while true {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count < 0 {
                throw try XCTUnwrap(stream.streamError)
            }
            if count == 0 {
                return body
            }
            body.append(contentsOf: buffer.prefix(count))
        }
    }

    func testGovernedTakedownRequiresOnlyAppliedHeight() throws {
        let exactRelease = try route(MusubiToriiClientV1.exactReleasePath)
        var canonical = try object(deepMutableCopy(exactRelease["response"]))
        let actionDigest = try XCTUnwrap(canonical["release_digest"])
        canonical["artifact_governance"] = [
            "kind": "TakenDown",
            "value": [
                "action_digest": actionDigest,
                "reason": ["security response"],
                "applied_at_height": 50
            ]
        ]
        var revisions = try object(canonical["revisions"])
        revisions["artifact_governance"] = 2
        canonical["revisions"] = revisions
        XCTAssertNoThrow(
            try JSONDecoder().decode(MusubiReleaseRecordV1.self, from: jsonData(canonical))
        )

        var legacy = try object(deepMutableCopy(canonical))
        var legacyGovernance = try object(legacy["artifact_governance"])
        var legacyPayload = try object(legacyGovernance["value"])
        legacyPayload["enacted_at_height"] = legacyPayload.removeValue(
            forKey: "applied_at_height"
        )
        legacyGovernance["value"] = legacyPayload
        legacy["artifact_governance"] = legacyGovernance
        XCTAssertThrowsError(
            try JSONDecoder().decode(MusubiReleaseRecordV1.self, from: jsonData(legacy))
        )

        var zeroHeight = try object(deepMutableCopy(canonical))
        var zeroGovernance = try object(zeroHeight["artifact_governance"])
        var zeroPayload = try object(zeroGovernance["value"])
        zeroPayload["applied_at_height"] = 0
        zeroGovernance["value"] = zeroPayload
        zeroHeight["artifact_governance"] = zeroGovernance
        XCTAssertThrowsError(
            try JSONDecoder().decode(MusubiReleaseRecordV1.self, from: jsonData(zeroHeight))
        )
    }

    func testRejectsNoncanonicalInputsUnknownFieldsAndUnknownABIVersions() throws {
        let rejected = try object(try fixture()["reject"])
        for raw in try array(rejected["names"]) {
            XCTAssertThrowsError(try MusubiPackageNameV1(try XCTUnwrap(raw as? String)))
        }
        for raw in try array(rejected["versions"]) {
            XCTAssertThrowsError(try MusubiVersionV1.parse(try XCTUnwrap(raw as? String)))
        }
        for raw in try array(rejected["requirements"]) {
            XCTAssertThrowsError(try MusubiVersionReqV1.parse(try XCTUnwrap(raw as? String)))
        }
        XCTAssertFalse(try array(rejected["fixture_versions"]).contains { ($0 as? Int) == 1 })

        let packageRoute = try route(MusubiToriiClientV1.exactPackagePath)
        var request = try object(packageRoute["request"])
        request["legacy"] = true
        XCTAssertThrowsError(
            try JSONDecoder().decode(MusubiExactPackageQueryV1.self, from: jsonData(request))
        )
        var response = try object(packageRoute["response"])
        response["legacy"] = true
        XCTAssertThrowsError(
            try JSONDecoder().decode(MusubiPackageRecordV1.self, from: jsonData(response))
        )

        let releaseRoute = try route(MusubiToriiClientV1.exactReleasePath)
        var releaseResponse = try object(deepMutableCopy(releaseRoute["response"]))
        var manifest = try object(releaseResponse["manifest"])
        var abi = try object(manifest["abi"])
        abi["abi_version"] = 2
        manifest["abi"] = abi
        releaseResponse["manifest"] = manifest
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiReleaseRecordV1.self,
                from: jsonData(releaseResponse)
            )
        )

        let archiveRoute = try route(MusubiToriiClientV1.archiveLocationsPath)
        var archiveResponse = try object(deepMutableCopy(archiveRoute["response"]))
        var archive = try object(archiveResponse["archive"])
        var receipt = try object(archive["staging_receipt"])
        var payload = try object(receipt["payload"])
        payload["version"] = 2
        receipt["payload"] = payload
        archive["staging_receipt"] = receipt
        archiveResponse["archive"] = archive
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiArchiveLocationPageV1.self,
                from: jsonData(archiveResponse)
            )
        )
    }

    private func invoke(
        _ client: MusubiToriiClientV1,
        path: String,
        request: Data
    ) async throws {
        let decoder = JSONDecoder()
        switch path {
        case MusubiToriiClientV1.exactPackagePath:
            _ = try await client.findExactPackage(
                decoder.decode(MusubiExactPackageQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.exactReleasePath:
            _ = try await client.findExactRelease(
                decoder.decode(MusubiExactReleaseQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.resolverIndexPath:
            _ = try await client.findResolverIndex(
                decoder.decode(MusubiResolverIndexQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.versionsPath:
            _ = try await client.findVersions(
                decoder.decode(MusubiPackagePageQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.maintainersPath:
            _ = try await client.findMaintainers(
                decoder.decode(MusubiPackagePageQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.archiveLocationsPath:
            _ = try await client.findArchiveLocations(
                decoder.decode(MusubiArchiveLocationQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.archiveRetentionPath:
            _ = try await client.findArchiveRetention(
                decoder.decode(MusubiArchiveRetentionQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.aliasPath:
            _ = try await client.findAlias(
                decoder.decode(MusubiAliasQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.aliasHistoryPath:
            _ = try await client.findAliasHistory(
                decoder.decode(MusubiAliasQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.orderedPrefixPath:
            _ = try await client.findOrderedPrefix(
                decoder.decode(MusubiOrderedPrefixQueryV1.self, from: request)
            )
        case MusubiToriiClientV1.searchPath:
            _ = try await client.search(
                decoder.decode(MusubiSearchQueryV1.self, from: request)
            )
        default:
            XCTFail("Unhandled Musubi fixture path \(path)")
        }
    }

    private func roundTripRequest(_ path: String, data: Data) throws -> NSObject {
        let decoder = JSONDecoder()
        switch path {
        case MusubiToriiClientV1.exactPackagePath:
            return try encodedObject(decoder.decode(MusubiExactPackageQueryV1.self, from: data))
        case MusubiToriiClientV1.exactReleasePath:
            return try encodedObject(decoder.decode(MusubiExactReleaseQueryV1.self, from: data))
        case MusubiToriiClientV1.resolverIndexPath:
            return try encodedObject(decoder.decode(MusubiResolverIndexQueryV1.self, from: data))
        case MusubiToriiClientV1.versionsPath, MusubiToriiClientV1.maintainersPath:
            return try encodedObject(decoder.decode(MusubiPackagePageQueryV1.self, from: data))
        case MusubiToriiClientV1.archiveLocationsPath:
            return try encodedObject(decoder.decode(MusubiArchiveLocationQueryV1.self, from: data))
        case MusubiToriiClientV1.archiveRetentionPath:
            return try encodedObject(
                decoder.decode(MusubiArchiveRetentionQueryV1.self, from: data)
            )
        case MusubiToriiClientV1.aliasPath, MusubiToriiClientV1.aliasHistoryPath:
            return try encodedObject(decoder.decode(MusubiAliasQueryV1.self, from: data))
        case MusubiToriiClientV1.orderedPrefixPath:
            return try encodedObject(decoder.decode(MusubiOrderedPrefixQueryV1.self, from: data))
        case MusubiToriiClientV1.searchPath:
            return try encodedObject(decoder.decode(MusubiSearchQueryV1.self, from: data))
        default:
            throw MusubiV1Error.invalidValue("Unhandled Musubi fixture path \(path).")
        }
    }

    private func roundTripResponse(_ path: String, data: Data) throws -> NSObject {
        let decoder = JSONDecoder()
        switch path {
        case MusubiToriiClientV1.exactPackagePath:
            return try encodedObject(decoder.decode(MusubiPackageRecordV1.self, from: data))
        case MusubiToriiClientV1.exactReleasePath:
            return try encodedObject(decoder.decode(MusubiReleaseRecordV1.self, from: data))
        case MusubiToriiClientV1.resolverIndexPath:
            return try encodedObject(decoder.decode(MusubiResolverIndexPageV1.self, from: data))
        case MusubiToriiClientV1.versionsPath:
            return try encodedObject(
                decoder.decode(MusubiPageV1<MusubiVersionV1>.self, from: data)
            )
        case MusubiToriiClientV1.maintainersPath:
            return try encodedObject(
                decoder.decode(MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self, from: data)
            )
        case MusubiToriiClientV1.archiveLocationsPath:
            return try encodedObject(
                decoder.decode(MusubiArchiveLocationPageV1.self, from: data)
            )
        case MusubiToriiClientV1.archiveRetentionPath:
            return try encodedObject(
                decoder.decode(MusubiArchiveRetentionPageV1.self, from: data)
            )
        case MusubiToriiClientV1.aliasPath:
            return try encodedObject(decoder.decode(MusubiAliasRecordV1.self, from: data))
        case MusubiToriiClientV1.aliasHistoryPath:
            return try encodedObject(
                decoder.decode(MusubiPageV1<MusubiAliasHistoryEntryV1>.self, from: data)
            )
        case MusubiToriiClientV1.orderedPrefixPath:
            return try encodedObject(
                decoder.decode(MusubiOrderedPrefixPageV1.self, from: data)
            )
        case MusubiToriiClientV1.searchPath:
            return try encodedObject(decoder.decode(MusubiSearchPageV1.self, from: data))
        default:
            throw MusubiV1Error.invalidValue("Unhandled Musubi fixture path \(path).")
        }
    }

    private func assertWireEqual<T: Encodable>(_ expected: Any?, _ value: T) throws {
        XCTAssertEqual(try encodedObject(value), try jsonObject(jsonData(expected)))
    }

    private func encodedObject<T: Encodable>(_ value: T) throws -> NSObject {
        try jsonObject(JSONEncoder().encode(value))
    }

    private func jsonObject(_ data: Data) throws -> NSObject {
        try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? NSObject)
    }

    private func fixture() throws -> [String: Any] {
        try object(JSONSerialization.jsonObject(with: Data(contentsOf: try fixtureURL())))
    }

    private func fixtureRoutes() throws -> [[String: Any]] {
        try array(try fixture()["routes"]).map(object)
    }

    private func route(_ path: String) throws -> [String: Any] {
        try XCTUnwrap(fixtureRoutes().first { ($0["path"] as? String) == path })
    }

    private func path(_ route: [String: Any]) throws -> String {
        try XCTUnwrap(route["path"] as? String)
    }

    private func fixtureURL() throws -> URL {
        var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = current.appendingPathComponent("fixtures/musubi/sdk_v1.json")
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            current.deleteLastPathComponent()
        }
        throw MusubiV1Error.invalidValue("fixtures/musubi/sdk_v1.json was not found.")
    }

    private func jsonData(_ value: Any?) throws -> Data {
        guard let value else { return Data("null".utf8) }
        return try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    }

    private func object(_ value: Any?) throws -> [String: Any] {
        try XCTUnwrap(value as? [String: Any])
    }

    private func array(_ value: Any?) throws -> [Any] {
        try XCTUnwrap(value as? [Any])
    }

    private func deepMutableCopy(_ value: Any?) -> Any {
        if let object = value as? [String: Any] {
            return object.mapValues(deepMutableCopy)
        }
        if let array = value as? [Any] { return array.map(deepMutableCopy) }
        return value as Any
    }

    private var expectedPaths: Set<String> {
        [
            MusubiToriiClientV1.exactPackagePath,
            MusubiToriiClientV1.exactReleasePath,
            MusubiToriiClientV1.resolverIndexPath,
            MusubiToriiClientV1.versionsPath,
            MusubiToriiClientV1.maintainersPath,
            MusubiToriiClientV1.archiveLocationsPath,
            MusubiToriiClientV1.archiveRetentionPath,
            MusubiToriiClientV1.aliasPath,
            MusubiToriiClientV1.aliasHistoryPath,
            MusubiToriiClientV1.orderedPrefixPath,
            MusubiToriiClientV1.searchPath,
        ]
    }
}

private extension Array {
    var single: Element? { count == 1 ? self[0] : nil }
}
