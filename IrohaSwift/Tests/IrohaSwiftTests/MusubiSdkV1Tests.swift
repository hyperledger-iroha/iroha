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
    }

    func testEveryTypedRouteRoundTripsExactRequestAndResponseJSON() throws {
        let routes = try fixtureRoutes()
        XCTAssertEqual(routes.count, 9)
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
                try jsonObject(try XCTUnwrap(captured.httpBody)),
                try jsonObject(expectedRequest)
            )
        }
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
        case MusubiToriiClientV1.aliasPath, MusubiToriiClientV1.aliasHistoryPath:
            return try encodedObject(decoder.decode(MusubiAliasQueryV1.self, from: data))
        case MusubiToriiClientV1.orderedPrefixPath:
            return try encodedObject(decoder.decode(MusubiOrderedPrefixQueryV1.self, from: data))
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
                decoder.decode(MusubiPageV1<MusubiPackageMemberV1>.self, from: data)
            )
        case MusubiToriiClientV1.archiveLocationsPath:
            return try encodedObject(
                decoder.decode(MusubiPageV1<MusubiArchiveLocationV1>.self, from: data)
            )
        case MusubiToriiClientV1.aliasPath:
            return try encodedObject(decoder.decode(MusubiAliasRecordV1.self, from: data))
        case MusubiToriiClientV1.aliasHistoryPath:
            return try encodedObject(
                decoder.decode(MusubiPageV1<MusubiAliasHistoryEntryV1>.self, from: data)
            )
        case MusubiToriiClientV1.orderedPrefixPath:
            return try encodedObject(
                decoder.decode(MusubiPageV1<MusubiOrderedPackageEntryV1>.self, from: data)
            )
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
            MusubiToriiClientV1.aliasPath,
            MusubiToriiClientV1.aliasHistoryPath,
            MusubiToriiClientV1.orderedPrefixPath,
        ]
    }
}

private extension Array {
    var single: Element? { count == 1 ? self[0] : nil }
}
