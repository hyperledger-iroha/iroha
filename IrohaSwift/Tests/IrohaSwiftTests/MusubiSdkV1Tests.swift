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
        XCTAssertEqual(routes.count, 12)
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
        XCTAssertEqual(
            page.finalizedTimeMs,
            try XCTUnwrap(
                try object(fixtureRoute["response"])["finalized_time_ms"] as? NSNumber
            ).uint64Value
        )

        var zeroFinalizedTime = try object(deepMutableCopy(fixtureRoute["response"]))
        zeroFinalizedTime["finalized_time_ms"] = UInt64(0)
        let zeroFinalizedTimePage = try decoder.decode(
            MusubiArchiveRetentionPageV1.self,
            from: jsonData(zeroFinalizedTime)
        )
        XCTAssertEqual(zeroFinalizedTimePage.finalizedTimeMs, 0)
        try zeroFinalizedTimePage.requireMatches(request)

        var missingFinalizedTime = try object(deepMutableCopy(fixtureRoute["response"]))
        missingFinalizedTime.removeValue(forKey: "finalized_time_ms")
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiArchiveRetentionPageV1.self,
                from: jsonData(missingFinalizedTime)
            )
        )

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

    func testArchiveLocationPageDecodesCurrentSortedLocations() throws {
        let response = try populatedArchiveLocationResponse(locationFills: [1, 2])
        let page = try JSONDecoder().decode(
            MusubiArchiveLocationPageV1.self,
            from: jsonData(response)
        )

        XCTAssertEqual(page.items.map { $0.locationId.bytes[0] }, [1, 2])
        XCTAssertEqual(page.items.map { $0.providerAttestationSetDigest.bytes[0] }, [23, 23])
        XCTAssertEqual(page.items.map(\.finalizedHeight), [50, 50])
        XCTAssertEqual(page.items.map(\.revision), [1, 1])
    }

    func testArchiveLocationRejectsLegacyOrZeroAttestationCommitment() throws {
        var legacy = try populatedArchiveLocationResponse(locationFills: [1])
        var items = try array(legacy["items"])
        var item = try object(items[0])
        item.removeValue(forKey: "provider_attestation_set_digest")
        item["provider_attestations"] = [Any]()
        items[0] = item
        legacy["items"] = items
        XCTAssertThrowsError(
            try JSONDecoder().decode(MusubiArchiveLocationPageV1.self, from: jsonData(legacy))
        )

        var zeroCommitment = try populatedArchiveLocationResponse(locationFills: [1])
        items = try array(zeroCommitment["items"])
        item = try object(items[0])
        item["provider_attestation_set_digest"] = [Array(repeating: 0, count: 32)]
        items[0] = item
        zeroCommitment["items"] = items
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiArchiveLocationPageV1.self,
                from: jsonData(zeroCommitment)
            )
        )
    }

    func testArchiveLocationPageRejectsExcessUnorderedAndDuplicateItems() throws {
        var tooMany = try populatedArchiveLocationResponse(locationFills: [1, 2, 3, 4])
        var items = try array(tooMany["items"])
        items.append(deepMutableCopy(items[3]))
        tooMany["items"] = items
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiArchiveLocationPageV1.self,
                from: jsonData(tooMany)
            )
        )

        var unordered = try populatedArchiveLocationResponse(locationFills: [1, 2])
        items = try array(unordered["items"])
        items.swapAt(0, 1)
        unordered["items"] = items
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiArchiveLocationPageV1.self,
                from: jsonData(unordered)
            )
        )

        var duplicate = try populatedArchiveLocationResponse(locationFills: [1, 2])
        items = try array(duplicate["items"])
        items[1] = deepMutableCopy(items[0])
        duplicate["items"] = items
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiArchiveLocationPageV1.self,
                from: jsonData(duplicate)
            )
        )
    }

    func testArchiveLocationPageRejectsNoncurrentItems() throws {
        let decoder = JSONDecoder()

        var wrongArchive = try populatedArchiveLocationResponse(locationFills: [1])
        var items = try array(wrongArchive["items"])
        var item = try object(items[0])
        item["archive_id"] = [Array(repeating: 9, count: 32)]
        items[0] = item
        wrongArchive["items"] = items
        XCTAssertThrowsError(
            try decoder.decode(MusubiArchiveLocationPageV1.self, from: jsonData(wrongArchive))
        )

        var nonmember = try populatedArchiveLocationResponse(locationFills: [1])
        items = try array(nonmember["items"])
        item = try object(items[0])
        item["location_id"] = [Array(repeating: 2, count: 32)]
        items[0] = item
        nonmember["items"] = items
        XCTAssertThrowsError(
            try decoder.decode(MusubiArchiveLocationPageV1.self, from: jsonData(nonmember))
        )

        var retired = try populatedArchiveLocationResponse(locationFills: [1])
        items = try array(retired["items"])
        item = try object(items[0])
        item["state"] = ["kind": "Retired", "value": NSNull()] as [String: Any]
        items[0] = item
        retired["items"] = items
        XCTAssertThrowsError(
            try decoder.decode(MusubiArchiveLocationPageV1.self, from: jsonData(retired))
        )

        var future = try populatedArchiveLocationResponse(locationFills: [1])
        items = try array(future["items"])
        item = try object(items[0])
        item["finalized_height"] = 51
        items[0] = item
        future["items"] = items
        XCTAssertThrowsError(
            try decoder.decode(MusubiArchiveLocationPageV1.self, from: jsonData(future))
        )

        var futureRevision = try populatedArchiveLocationResponse(locationFills: [1])
        items = try array(futureRevision["items"])
        item = try object(items[0])
        item["revision"] = 2
        items[0] = item
        futureRevision["items"] = items
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiArchiveLocationPageV1.self,
                from: jsonData(futureRevision)
            )
        )
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

    func testQueryResponsesBindExactRequestIdentityAndSelectors() throws {
        let decoder = JSONDecoder()

        let packageRoute = try route(MusubiToriiClientV1.exactPackagePath)
        let packageRequest = try decoder.decode(
            MusubiExactPackageQueryV1.self,
            from: jsonData(packageRoute["request"])
        )
        let packageRecord = try decoder.decode(
            MusubiPackageRecordV1.self,
            from: jsonData(packageRoute["response"])
        )
        try packageRecord.requireMatches(packageRequest)
        var wrongPackageRequest = try object(deepMutableCopy(packageRoute["request"]))
        var wrongPackage = try object(wrongPackageRequest["package"])
        wrongPackage["name"] = ["other-package"]
        wrongPackageRequest["package"] = wrongPackage
        XCTAssertThrowsError(
            try packageRecord.requireMatches(
                decoder.decode(
                    MusubiExactPackageQueryV1.self,
                    from: jsonData(wrongPackageRequest)
                )
            )
        )

        let releaseRoute = try route(MusubiToriiClientV1.exactReleasePath)
        let releaseRequest = try decoder.decode(
            MusubiExactReleaseQueryV1.self,
            from: jsonData(releaseRoute["request"])
        )
        let releaseSnapshot = try decoder.decode(
            MusubiExactReleaseSnapshotV1.self,
            from: jsonData(releaseRoute["response"])
        )
        try releaseSnapshot.requireMatches(releaseRequest)
        var wrongReleaseRequest = try object(deepMutableCopy(releaseRoute["request"]))
        var wrongRelease = try object(wrongReleaseRequest["release"])
        var wrongVersion = try object(wrongRelease["version"])
        wrongVersion["patch"] = 4
        wrongRelease["version"] = wrongVersion
        wrongReleaseRequest["release"] = wrongRelease
        XCTAssertThrowsError(
            try releaseSnapshot.requireMatches(
                decoder.decode(
                    MusubiExactReleaseQueryV1.self,
                    from: jsonData(wrongReleaseRequest)
                )
            )
        )

        let providerRoute = try route(MusubiToriiClientV1.providerBundleAttestationPath)
        let providerRequest = try decoder.decode(
            MusubiProviderBundleAttestationKeyV1.self,
            from: jsonData(providerRoute["request"])
        )
        let providerRecord = try decoder.decode(
            MusubiProviderBundleAttestationRecordV1.self,
            from: jsonData(providerRoute["response"])
        )
        try providerRecord.requireMatches(providerRequest)
        var wrongProviderRequest = try object(deepMutableCopy(providerRoute["request"]))
        wrongProviderRequest["provider_id"] = [String(repeating: "FE", count: 32)]
        XCTAssertThrowsError(
            try providerRecord.requireMatches(
                decoder.decode(
                    MusubiProviderBundleAttestationKeyV1.self,
                    from: jsonData(wrongProviderRequest)
                )
            )
        )

        let maintainerRoute = try route(MusubiToriiClientV1.maintainersPath)
        let maintainerPage = try decoder.decode(
            MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
            from: jsonData(maintainerRoute["response"])
        )
        var wrongMaintainerRequest = try object(deepMutableCopy(maintainerRoute["request"]))
        var wrongMaintainerPackage = try object(wrongMaintainerRequest["package"])
        wrongMaintainerPackage["name"] = ["other-package"]
        wrongMaintainerRequest["package"] = wrongMaintainerPackage
        XCTAssertThrowsError(
            try maintainerPage.requireMatches(
                decoder.decode(
                    MusubiPackagePageQueryV1.self,
                    from: jsonData(wrongMaintainerRequest)
                )
            )
        )

        let archiveRoute = try route(MusubiToriiClientV1.archiveLocationsPath)
        let archivePage = try decoder.decode(
            MusubiArchiveLocationPageV1.self,
            from: jsonData(archiveRoute["response"])
        )
        var wrongArchiveRequest = try object(deepMutableCopy(archiveRoute["request"]))
        wrongArchiveRequest["archive_id"] = [Array(repeating: 99, count: 32)]
        XCTAssertThrowsError(
            try archivePage.requireMatches(
                decoder.decode(
                    MusubiArchiveLocationQueryV1.self,
                    from: jsonData(wrongArchiveRequest)
                )
            )
        )

        let aliasRoute = try route(MusubiToriiClientV1.aliasPath)
        let aliasRecord = try decoder.decode(
            MusubiAliasRecordV1.self,
            from: jsonData(aliasRoute["response"])
        )
        var wrongAliasRequest = try object(deepMutableCopy(aliasRoute["request"]))
        wrongAliasRequest["alias"] = ["other-alias"]
        let mismatchedAlias = try decoder.decode(
            MusubiAliasQueryV1.self,
            from: jsonData(wrongAliasRequest)
        )
        XCTAssertThrowsError(try aliasRecord.requireMatches(mismatchedAlias))

        let historyRoute = try route(MusubiToriiClientV1.aliasHistoryPath)
        let historyPage = try decoder.decode(
            MusubiPageV1<MusubiAliasHistoryEntryV1>.self,
            from: jsonData(historyRoute["response"])
        )
        XCTAssertThrowsError(try historyPage.requireMatches(mismatchedAlias))

        let prefixRoute = try route(MusubiToriiClientV1.orderedPrefixPath)
        let prefixPage = try decoder.decode(
            MusubiOrderedPrefixPageV1.self,
            from: jsonData(prefixRoute["response"])
        )
        var wrongPrefixRequest = try object(deepMutableCopy(prefixRoute["request"]))
        wrongPrefixRequest["prefix"] = ["unrelated/"]
        XCTAssertThrowsError(
            try prefixPage.requireMatches(
                decoder.decode(
                    MusubiOrderedPrefixQueryV1.self,
                    from: jsonData(wrongPrefixRequest)
                )
            )
        )
    }

    func testEveryEchoedPageQueryBindsEmptyFirstPagesToTheOriginatingRequest() throws {
        let decoder = JSONDecoder()

        let resolverRoute = try route(MusubiToriiClientV1.resolverIndexPath)
        let resolverRequest = try decoder.decode(
            MusubiResolverIndexQueryV1.self,
            from: jsonData(resolverRoute["request"])
        )
        var resolverResponse = try emptyFirstPage(resolverRoute["response"])
        let resolverPage = try decoder.decode(
            MusubiResolverIndexPageV1.self,
            from: jsonData(resolverResponse)
        )
        XCTAssertNoThrow(try resolverPage.requireMatches(resolverRequest))
        resolverResponse["query"] = try packageQueryWithOtherName(resolverResponse["query"])
        let otherResolverPage = try decoder.decode(
            MusubiResolverIndexPageV1.self,
            from: jsonData(resolverResponse)
        )
        XCTAssertThrowsError(try otherResolverPage.requireMatches(resolverRequest))

        for routePath in [MusubiToriiClientV1.versionsPath, MusubiToriiClientV1.maintainersPath] {
            let fixtureRoute = try route(routePath)
            let request = try decoder.decode(
                MusubiPackagePageQueryV1.self,
                from: jsonData(fixtureRoute["request"])
            )
            var response = try emptyFirstPage(fixtureRoute["response"])
            if routePath == MusubiToriiClientV1.versionsPath {
                let page = try decoder.decode(
                    MusubiPageV1<MusubiVersionV1>.self,
                    from: jsonData(response)
                )
                XCTAssertNoThrow(try page.requireMatches(request))
                response["query"] = try packageQueryWithOtherName(response["query"])
                let otherPage = try decoder.decode(
                    MusubiPageV1<MusubiVersionV1>.self,
                    from: jsonData(response)
                )
                XCTAssertThrowsError(try otherPage.requireMatches(request))
            } else {
                let page = try decoder.decode(
                    MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
                    from: jsonData(response)
                )
                XCTAssertNoThrow(try page.requireMatches(request))
                response["query"] = try packageQueryWithOtherName(response["query"])
                let otherPage = try decoder.decode(
                    MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
                    from: jsonData(response)
                )
                XCTAssertThrowsError(try otherPage.requireMatches(request))
            }
        }

        let historyRoute = try route(MusubiToriiClientV1.aliasHistoryPath)
        let historyRequest = try decoder.decode(
            MusubiAliasQueryV1.self,
            from: jsonData(historyRoute["request"])
        )
        var historyResponse = try emptyFirstPage(historyRoute["response"])
        let historyPage = try decoder.decode(
            MusubiPageV1<MusubiAliasHistoryEntryV1>.self,
            from: jsonData(historyResponse)
        )
        XCTAssertNoThrow(try historyPage.requireMatches(historyRequest))
        var otherAliasQuery = try object(historyResponse["query"])
        otherAliasQuery["alias"] = ["other"]
        historyResponse["query"] = otherAliasQuery
        let otherHistoryPage = try decoder.decode(
            MusubiPageV1<MusubiAliasHistoryEntryV1>.self,
            from: jsonData(historyResponse)
        )
        XCTAssertThrowsError(try otherHistoryPage.requireMatches(historyRequest))

        let prefixRoute = try route(MusubiToriiClientV1.orderedPrefixPath)
        let prefixRequest = try decoder.decode(
            MusubiOrderedPrefixQueryV1.self,
            from: jsonData(prefixRoute["request"])
        )
        var prefixResponse = try emptyFirstPage(prefixRoute["response"])
        let prefixPage = try decoder.decode(
            MusubiOrderedPrefixPageV1.self,
            from: jsonData(prefixResponse)
        )
        XCTAssertNoThrow(try prefixPage.requireMatches(prefixRequest))
        var otherPrefixQuery = try object(prefixResponse["query"])
        otherPrefixQuery["prefix"] = ["sora/other"]
        prefixResponse["query"] = otherPrefixQuery
        let otherPrefixPage = try decoder.decode(
            MusubiOrderedPrefixPageV1.self,
            from: jsonData(prefixResponse)
        )
        XCTAssertThrowsError(try otherPrefixPage.requireMatches(prefixRequest))

        let searchRoute = try route(MusubiToriiClientV1.searchPath)
        let searchRequest = try decoder.decode(
            MusubiSearchQueryV1.self,
            from: jsonData(searchRoute["request"])
        )
        var searchResponse = try emptyFirstPage(searchRoute["response"])
        let searchPage = try decoder.decode(
            MusubiSearchPageV1.self,
            from: jsonData(searchResponse)
        )
        XCTAssertNoThrow(try searchPage.requireMatches(searchRequest))
        var otherSearchQuery = try object(searchResponse["query"])
        otherSearchQuery["query"] = "different search"
        searchResponse["query"] = otherSearchQuery
        let otherSearchPage = try decoder.decode(
            MusubiSearchPageV1.self,
            from: jsonData(searchResponse)
        )
        XCTAssertThrowsError(try otherSearchPage.requireMatches(searchRequest))

        for routePath in [
            MusubiToriiClientV1.resolverIndexPath,
            MusubiToriiClientV1.versionsPath,
            MusubiToriiClientV1.maintainersPath,
            MusubiToriiClientV1.aliasHistoryPath,
            MusubiToriiClientV1.orderedPrefixPath,
            MusubiToriiClientV1.searchPath,
        ] {
            let fixtureRoute = try route(routePath)
            var response = try object(deepMutableCopy(fixtureRoute["response"]))
            response.removeValue(forKey: "query")
            XCTAssertThrowsError(try roundTripResponse(routePath, data: jsonData(response)))
        }
    }

    func testGenericAndResolverPagesRejectNoncanonicalOrdering() throws {
        let decoder = JSONDecoder()

        let versionsRoute = try route(MusubiToriiClientV1.versionsPath)
        var duplicateVersions = try object(deepMutableCopy(versionsRoute["response"]))
        let versionItems = try array(duplicateVersions["items"])
        duplicateVersions["items"] = versionItems + versionItems
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiVersionV1>.self,
                from: jsonData(duplicateVersions)
            )
        )

        let maintainersRoute = try route(MusubiToriiClientV1.maintainersPath)
        var reversedMaintainers = try object(deepMutableCopy(maintainersRoute["response"]))
        reversedMaintainers["items"] = Array(
            try array(reversedMaintainers["items"]).reversed()
        )
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
                from: jsonData(reversedMaintainers)
            )
        )

        let historyRoute = try route(MusubiToriiClientV1.aliasHistoryPath)
        var duplicateHistory = try object(deepMutableCopy(historyRoute["response"]))
        let historyItems = try array(duplicateHistory["items"])
        duplicateHistory["items"] = historyItems + historyItems
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiAliasHistoryEntryV1>.self,
                from: jsonData(duplicateHistory)
            )
        )

        let resolverRoute = try route(MusubiToriiClientV1.resolverIndexPath)
        var duplicateResolver = try object(deepMutableCopy(resolverRoute["response"]))
        let releaseRoute = try route(MusubiToriiClientV1.exactReleasePath)
        let releaseSnapshot = try object(releaseRoute["response"])
        let release = try object(releaseSnapshot["home_release"])
        let manifest = try object(release["manifest"])
        let resolverRow: [String: Any] = [
            "release": try XCTUnwrap(manifest["release"]),
            "release_digest": try XCTUnwrap(release["release_digest"]),
            "archive_id": try XCTUnwrap(manifest["archive_id"]),
            "source_digest": [Array(repeating: 6, count: 32)],
            "interface_digest": try XCTUnwrap(manifest["interface_digest"]),
            "abi": try XCTUnwrap(manifest["abi"]),
            "dependencies": [Any](),
            "selection": ["kind": "Selectable", "value": NSNull()] as [String: Any],
            "index_revision": 9,
        ]
        duplicateResolver["items"] = [resolverRow, resolverRow]
        XCTAssertThrowsError(
            try decoder.decode(MusubiResolverIndexPageV1.self, from: jsonData(duplicateResolver))
        )
    }

    func testPageResponseBindingUsesDefaultLimitAndRejectsCursorSnapshotMismatch() throws {
        let decoder = JSONDecoder()
        let versionsRoute = try route(MusubiToriiClientV1.versionsPath)
        let page = try decoder.decode(
            MusubiPageV1<MusubiVersionV1>.self,
            from: jsonData(versionsRoute["response"])
        )
        let canonicalVersionRequest = try decoder.decode(
            MusubiPackagePageQueryV1.self,
            from: jsonData(versionsRoute["request"])
        )
        XCTAssertNoThrow(try page.requireMatches(canonicalVersionRequest))

        var zeroLimit = try object(deepMutableCopy(versionsRoute["request"]))
        var zeroLimitPage = try object(zeroLimit["page"])
        zeroLimitPage["limit"] = 0
        zeroLimit["page"] = zeroLimitPage
        var zeroLimitResponse = try object(deepMutableCopy(versionsRoute["response"]))
        zeroLimitResponse["query"] = zeroLimit
        let zeroLimitResponsePage = try decoder.decode(
            MusubiPageV1<MusubiVersionV1>.self,
            from: jsonData(zeroLimitResponse)
        )
        XCTAssertNoThrow(
            try zeroLimitResponsePage.requireMatches(
                decoder.decode(MusubiPackagePageQueryV1.self, from: jsonData(zeroLimit))
            )
        )
        XCTAssertThrowsError(try page.requireMatches(
            decoder.decode(MusubiPackagePageQueryV1.self, from: jsonData(zeroLimit))
        ))

        var oversizedDefaultPageResponse = try object(
            deepMutableCopy(zeroLimitResponse)
        )
        oversizedDefaultPageResponse["items"] = (0..<51).map { patch in
            [
                "major": 1,
                "minor": 0,
                "patch": patch,
                "prerelease": [Any](),
            ] as [String: Any]
        }
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiVersionV1>.self,
                from: jsonData(oversizedDefaultPageResponse)
            )
        )

        let searchRoute = try route(MusubiToriiClientV1.searchPath)
        let searchPage = try decoder.decode(
            MusubiSearchPageV1.self,
            from: jsonData(searchRoute["response"])
        )
        var zeroSearchLimit = try object(deepMutableCopy(searchRoute["request"]))
        var zeroSearchPage = try object(zeroSearchLimit["page"])
        zeroSearchPage["limit"] = 0
        zeroSearchLimit["page"] = zeroSearchPage
        var zeroSearchResponse = try object(deepMutableCopy(searchRoute["response"]))
        zeroSearchResponse["query"] = zeroSearchLimit
        let zeroSearchResponsePage = try decoder.decode(
            MusubiSearchPageV1.self,
            from: jsonData(zeroSearchResponse)
        )
        XCTAssertNoThrow(
            try zeroSearchResponsePage.requireMatches(
                decoder.decode(MusubiSearchQueryV1.self, from: jsonData(zeroSearchLimit))
            )
        )
        XCTAssertThrowsError(try searchPage.requireMatches(
            decoder.decode(MusubiSearchQueryV1.self, from: jsonData(zeroSearchLimit))
        ))

        var oversizedDefaultSearchResponse = try object(
            deepMutableCopy(zeroSearchResponse)
        )
        let baseSearchHit = try XCTUnwrap(
            try array(oversizedDefaultSearchResponse["items"]).first
        )
        oversizedDefaultSearchResponse["items"] = try (0..<51).map { index -> Any in
            var hit = try object(deepMutableCopy(baseSearchHit))
            var package = try object(hit["package"])
            package["name"] = [String(format: "package-%02d", index)]
            hit["package"] = package
            return hit
        }
        oversizedDefaultSearchResponse["next_cursor"] = NSNull()
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiSearchPageV1.self,
                from: jsonData(oversizedDefaultSearchResponse)
            )
        )

        var staleCursor = try object(deepMutableCopy(versionsRoute["request"]))
        var staleCursorPage = try object(staleCursor["page"])
        var staleSnapshot = try object(
            try object(versionsRoute["response"])["snapshot"]
        )
        staleSnapshot["finalized_height"] = 49
        staleCursorPage["cursor"] = [
            "snapshot": staleSnapshot,
            "query_hash": [Array(repeating: 1, count: 32)],
            "last_key": "1.0.0",
            "caller": NSNull(),
        ] as [String: Any]
        staleCursor["page"] = staleCursorPage
        var staleCursorResponse = try object(deepMutableCopy(versionsRoute["response"]))
        staleCursorResponse["query"] = staleCursor
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiVersionV1>.self,
                from: jsonData(staleCursorResponse)
            )
        )
    }

    func testEchoedPagesRejectTamperedStructuredCursorBoundariesAndContinuations() throws {
        let decoder = JSONDecoder()
        let versionsRoute = try route(MusubiToriiClientV1.versionsPath)
        var versionResponse = try object(deepMutableCopy(versionsRoute["response"]))
        let snapshot = try object(versionResponse["snapshot"])
        let versionItems = try array(versionResponse["items"])
        let tailVersion = try decoder.decode(
            MusubiVersionV1.self,
            from: jsonData(try XCTUnwrap(versionItems.last))
        ).canonicalText
        var versionQuery = try object(versionResponse["query"])
        var versionPage = try object(versionQuery["page"])
        versionPage["limit"] = 1
        versionPage["cursor"] = finalizedCursor(snapshot: snapshot, lastKey: "1.0.0")
        versionQuery["page"] = versionPage
        versionResponse["query"] = versionQuery
        versionResponse["next_cursor"] = finalizedCursor(
            snapshot: snapshot,
            lastKey: tailVersion
        )
        XCTAssertNoThrow(
            try decoder.decode(
                MusubiPageV1<MusubiVersionV1>.self,
                from: jsonData(versionResponse)
            )
        )

        var wrongTail = try object(deepMutableCopy(versionResponse))
        var wrongTailCursor = try object(wrongTail["next_cursor"])
        wrongTailCursor["last_key"] = "9.9.9"
        wrongTail["next_cursor"] = wrongTailCursor
        XCTAssertThrowsError(
            try decoder.decode(MusubiPageV1<MusubiVersionV1>.self, from: jsonData(wrongTail))
        )

        var shortPage = try object(deepMutableCopy(versionResponse))
        var shortQuery = try object(shortPage["query"])
        var shortControls = try object(shortQuery["page"])
        shortControls["limit"] = 2
        shortQuery["page"] = shortControls
        shortPage["query"] = shortQuery
        XCTAssertThrowsError(
            try decoder.decode(MusubiPageV1<MusubiVersionV1>.self, from: jsonData(shortPage))
        )

        var wrongHash = try object(deepMutableCopy(versionResponse))
        var wrongHashCursor = try object(wrongHash["next_cursor"])
        wrongHashCursor["query_hash"] = [Array(repeating: 2, count: 32)]
        wrongHash["next_cursor"] = wrongHashCursor
        XCTAssertThrowsError(
            try decoder.decode(MusubiPageV1<MusubiVersionV1>.self, from: jsonData(wrongHash))
        )

        var wrongNextSnapshot = try object(deepMutableCopy(versionResponse))
        var wrongSnapshotCursor = try object(wrongNextSnapshot["next_cursor"])
        var wrongSnapshot = try object(wrongSnapshotCursor["snapshot"])
        wrongSnapshot["finalized_height"] = 49
        wrongSnapshotCursor["snapshot"] = wrongSnapshot
        wrongNextSnapshot["next_cursor"] = wrongSnapshotCursor
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiVersionV1>.self,
                from: jsonData(wrongNextSnapshot)
            )
        )

        var nextCallerBound = try object(deepMutableCopy(versionResponse))
        var nextCallerCursor = try object(nextCallerBound["next_cursor"])
        nextCallerCursor["caller"] = "unexpected-caller"
        nextCallerBound["next_cursor"] = nextCallerCursor
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiVersionV1>.self,
                from: jsonData(nextCallerBound)
            )
        )

        var callerBound = try object(deepMutableCopy(versionResponse))
        var callerQuery = try object(callerBound["query"])
        var callerControls = try object(callerQuery["page"])
        var callerCursor = try object(callerControls["cursor"])
        callerCursor["caller"] = "unexpected-caller"
        callerControls["cursor"] = callerCursor
        callerQuery["page"] = callerControls
        callerBound["query"] = callerQuery
        XCTAssertThrowsError(
            try decoder.decode(MusubiPageV1<MusubiVersionV1>.self, from: jsonData(callerBound))
        )

        let resolverRoute = try route(MusubiToriiClientV1.resolverIndexPath)
        var badResolver = try object(deepMutableCopy(resolverRoute["response"]))
        var resolverQuery = try object(badResolver["query"])
        var resolverControls = try object(resolverQuery["page"])
        resolverControls["cursor"] = finalizedCursor(
            snapshot: try object(badResolver["snapshot"]),
            lastKey: "not-semver"
        )
        resolverQuery["page"] = resolverControls
        badResolver["query"] = resolverQuery
        XCTAssertThrowsError(
            try decoder.decode(MusubiResolverIndexPageV1.self, from: jsonData(badResolver))
        )

        let maintainersRoute = try route(MusubiToriiClientV1.maintainersPath)
        var badMaintainers = try object(deepMutableCopy(maintainersRoute["response"]))
        var maintainerQuery = try object(badMaintainers["query"])
        var maintainerControls = try object(maintainerQuery["page"])
        maintainerControls["cursor"] = finalizedCursor(
            snapshot: try object(badMaintainers["snapshot"]),
            lastKey: "not-a-maintainer-key"
        )
        maintainerQuery["page"] = maintainerControls
        badMaintainers["query"] = maintainerQuery
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiMaintainerDirectoryEntryV1>.self,
                from: jsonData(badMaintainers)
            )
        )

        let historyRoute = try route(MusubiToriiClientV1.aliasHistoryPath)
        var badHistory = try object(deepMutableCopy(historyRoute["response"]))
        var historyQuery = try object(badHistory["query"])
        var historyControls = try object(historyQuery["page"])
        historyControls["cursor"] = finalizedCursor(
            snapshot: try object(badHistory["snapshot"]),
            lastKey: "math:1"
        )
        historyQuery["page"] = historyControls
        badHistory["query"] = historyQuery
        XCTAssertThrowsError(
            try decoder.decode(
                MusubiPageV1<MusubiAliasHistoryEntryV1>.self,
                from: jsonData(badHistory)
            )
        )

        let prefixRoute = try route(MusubiToriiClientV1.orderedPrefixPath)
        var badPrefix = try object(deepMutableCopy(prefixRoute["response"]))
        var prefixQuery = try object(badPrefix["query"])
        var prefixControls = try object(prefixQuery["page"])
        prefixControls["cursor"] = finalizedCursor(
            snapshot: try object(badPrefix["snapshot"]),
            lastKey: "sora/zzzz"
        )
        prefixQuery["page"] = prefixControls
        badPrefix["query"] = prefixQuery
        XCTAssertThrowsError(
            try decoder.decode(MusubiOrderedPrefixPageV1.self, from: jsonData(badPrefix))
        )

        let searchRoute = try route(MusubiToriiClientV1.searchPath)
        var badSearch = try object(deepMutableCopy(searchRoute["response"]))
        var searchQuery = try object(badSearch["query"])
        var searchControls = try object(searchQuery["page"])
        let firstSearchItem = try object(
            try XCTUnwrap(try array(badSearch["items"]).first)
        )
        searchControls["cursor"] = [
            "snapshot": try XCTUnwrap(badSearch["snapshot"]),
            "query_hash": [Array(repeating: 1, count: 32)],
            "last_package": try XCTUnwrap(firstSearchItem["package"]),
        ] as [String: Any]
        searchQuery["page"] = searchControls
        badSearch["query"] = searchQuery
        XCTAssertThrowsError(
            try decoder.decode(MusubiSearchPageV1.self, from: jsonData(badSearch))
        )
    }

    func testGenericPageRequestRejectsAboveMaximumAndPreservesZeroDefault() throws {
        XCTAssertEqual(try MusubiPageRequestV1(limit: 0).limit, 0)
        XCTAssertThrowsError(try MusubiPageRequestV1(limit: 101))

        let versionsRoute = try route(MusubiToriiClientV1.versionsPath)
        var oversizedRequest = try object(deepMutableCopy(versionsRoute["request"]))
        var page = try object(oversizedRequest["page"])
        page["limit"] = 101
        oversizedRequest["page"] = page
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiPackagePageQueryV1.self,
                from: jsonData(oversizedRequest)
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
        var homeRelease = try object(canonical["home_release"])
        let actionDigest = try XCTUnwrap(homeRelease["release_digest"])
        let governedState: [String: Any] = [
            "kind": "TakenDown",
            "value": [
                "action_digest": actionDigest,
                "reason": ["security response"],
                "applied_at_height": 50
            ]
        ]
        homeRelease["artifact_governance"] = governedState
        var revisions = try object(homeRelease["revisions"])
        revisions["artifact_governance"] = 2
        homeRelease["revisions"] = revisions
        canonical["home_release"] = homeRelease
        var universalRelease = try object(canonical["universal_release"])
        var selection = try object(universalRelease["selection"])
        selection["governance"] = deepMutableCopy(governedState)
        universalRelease["selection"] = selection
        canonical["universal_release"] = universalRelease
        XCTAssertNoThrow(
            try JSONDecoder().decode(MusubiExactReleaseSnapshotV1.self, from: jsonData(canonical))
        )

        var legacy = try object(deepMutableCopy(canonical))
        var legacyHomeRelease = try object(legacy["home_release"])
        var legacyGovernance = try object(legacyHomeRelease["artifact_governance"])
        var legacyPayload = try object(legacyGovernance["value"])
        legacyPayload["enacted_at_height"] = legacyPayload.removeValue(
            forKey: "applied_at_height"
        )
        legacyGovernance["value"] = legacyPayload
        legacyHomeRelease["artifact_governance"] = legacyGovernance
        legacy["home_release"] = legacyHomeRelease
        XCTAssertThrowsError(
            try JSONDecoder().decode(MusubiExactReleaseSnapshotV1.self, from: jsonData(legacy))
        )

        var zeroHeight = try object(deepMutableCopy(canonical))
        var zeroHomeRelease = try object(zeroHeight["home_release"])
        var zeroGovernance = try object(zeroHomeRelease["artifact_governance"])
        var zeroPayload = try object(zeroGovernance["value"])
        zeroPayload["applied_at_height"] = 0
        zeroGovernance["value"] = zeroPayload
        zeroHomeRelease["artifact_governance"] = zeroGovernance
        zeroHeight["home_release"] = zeroHomeRelease
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiExactReleaseSnapshotV1.self,
                from: jsonData(zeroHeight)
            )
        )
    }

    func testExactReleaseRejectsSubstitutedProjectionsAndNonfinalAnchors() throws {
        let exactRelease = try route(MusubiToriiClientV1.exactReleasePath)

        func assertRejected(
            _ mutation: (inout [String: Any]) throws -> Void,
            file: StaticString = #filePath,
            line: UInt = #line
        ) throws {
            var response = try object(deepMutableCopy(exactRelease["response"]))
            try mutation(&response)
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    MusubiExactReleaseSnapshotV1.self,
                    from: jsonData(response)
                ),
                file: file,
                line: line
            )
        }

        try assertRejected { response in
            var universal = try object(response["universal_release"])
            universal["release_digest"] = [Array(repeating: 64, count: 32)]
            response["universal_release"] = universal
        }
        try assertRejected { response in
            let replacement = [Array(repeating: 64, count: 32)]
            var home = try object(response["home_release"])
            home["release_digest"] = replacement
            response["home_release"] = home
            var universal = try object(response["universal_release"])
            universal["release_digest"] = replacement
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            home["published_by"] = "not-an-account"
            response["home_release"] = home
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            home["published_at_height"] = 0
            response["home_release"] = home
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            universal["archive_id"] = [Array(repeating: 65, count: 32)]
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            universal["interface_digest"] = [Array(repeating: 66, count: 32)]
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            var abi = try object(universal["abi"])
            abi["abi_hash"] = Array(repeating: 67, count: 32)
            universal["abi"] = abi
            response["universal_release"] = universal
        }
        try assertRejected { response in
            let home = try object(response["home_release"])
            let manifest = try object(home["manifest"])
            let release = try object(manifest["release"])
            var dependencyPackage = try object(deepMutableCopy(release["package"]))
            dependencyPackage["name"] = ["dependency"]
            var universal = try object(response["universal_release"])
            universal["dependencies"] = [[
                "alias": "dependency",
                "package": dependencyPackage,
                "requirement": ["kind": "Any", "value": NSNull()]
            ]]
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var yank = try object(selection["yank"])
            yank["reason"] = ["substituted state"]
            selection["yank"] = yank
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            let home = try object(response["home_release"])
            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            selection["governance"] = [
                "kind": "TakenDown",
                "value": [
                    "action_digest": try XCTUnwrap(home["release_digest"]),
                    "reason": ["substituted governance"],
                    "applied_at_height": 50
                ]
            ]
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            var homeYank = try object(home["yank"])
            homeYank["revision"] = 10
            home["yank"] = homeYank
            var revisions = try object(home["revisions"])
            revisions["yank"] = 10
            home["revisions"] = revisions
            response["home_release"] = home

            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var universalYank = try object(selection["yank"])
            universalYank["revision"] = 10
            selection["yank"] = universalYank
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            var homeYank = try object(home["yank"])
            homeYank["changed_at_height"] = 51
            home["yank"] = homeYank
            response["home_release"] = home

            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var universalYank = try object(selection["yank"])
            universalYank["changed_at_height"] = 51
            selection["yank"] = universalYank
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            var revisions = try object(home["revisions"])
            revisions["artifact_governance"] = 10
            home["revisions"] = revisions
            response["home_release"] = home
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            var homeYank = try object(home["yank"])
            homeYank["changed_at_height"] = 42
            home["yank"] = homeYank
            response["home_release"] = home

            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var universalYank = try object(selection["yank"])
            universalYank["changed_at_height"] = 42
            selection["yank"] = universalYank
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            let governed: [String: Any] = [
                "kind": "TakenDown",
                "value": [
                    "action_digest": try XCTUnwrap(home["release_digest"]),
                    "reason": ["nonfinal takedown"],
                    "applied_at_height": 51
                ]
            ]
            home["artifact_governance"] = governed
            var revisions = try object(home["revisions"])
            revisions["artifact_governance"] = 2
            home["revisions"] = revisions
            response["home_release"] = home

            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            selection["governance"] = deepMutableCopy(governed)
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            home["published_at_height"] = 51
            response["home_release"] = home
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            universal["index_revision"] = 10
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var home = try object(response["home_release"])
            let governed: [String: Any] = [
                "kind": "TakenDown",
                "value": [
                    "action_digest": try XCTUnwrap(home["release_digest"]),
                    "reason": ["premature takedown"],
                    "applied_at_height": 42
                ]
            ]
            home["artifact_governance"] = governed
            var revisions = try object(home["revisions"])
            revisions["artifact_governance"] = 2
            home["revisions"] = revisions
            response["home_release"] = home

            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            selection["governance"] = deepMutableCopy(governed)
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var storage = try object(selection["storage"])
            storage["finalized_height"] = 51
            selection["storage"] = storage
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var storage = try object(selection["storage"])
            storage["finalized_block_hash"] = Array(repeating: 6, count: 32)
            selection["storage"] = storage
            universal["selection"] = selection
            response["universal_release"] = universal
        }
        try assertRejected { response in
            var snapshot = try object(response["snapshot"])
            snapshot["finalized_height"] = 1
            response["snapshot"] = snapshot

            var home = try object(response["home_release"])
            home["published_at_height"] = 1
            var homeYank = try object(home["yank"])
            homeYank["changed_at_height"] = 1
            home["yank"] = homeYank
            response["home_release"] = home

            var universal = try object(response["universal_release"])
            var selection = try object(universal["selection"])
            var universalYank = try object(selection["yank"])
            universalYank["changed_at_height"] = 1
            selection["yank"] = universalYank
            var storage = try object(selection["storage"])
            storage["finalized_height"] = 1
            selection["storage"] = storage
            universal["selection"] = selection
            response["universal_release"] = universal
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
        var homeRelease = try object(releaseResponse["home_release"])
        var manifest = try object(homeRelease["manifest"])
        var abi = try object(manifest["abi"])
        abi["abi_version"] = 2
        manifest["abi"] = abi
        homeRelease["manifest"] = manifest
        releaseResponse["home_release"] = homeRelease
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiExactReleaseSnapshotV1.self,
                from: jsonData(releaseResponse)
            )
        )

        var futureStorage = try object(deepMutableCopy(releaseRoute["response"]))
        var universalRelease = try object(futureStorage["universal_release"])
        universalRelease["index_revision"] = 8
        futureStorage["universal_release"] = universalRelease
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiExactReleaseSnapshotV1.self,
                from: jsonData(futureStorage)
            )
        )

        let providerRoute = try route(MusubiToriiClientV1.providerBundleAttestationPath)
        var substitutedAttestationDigest = try object(
            deepMutableCopy(providerRoute["response"])
        )
        substitutedAttestationDigest["attestation_digest"] = [
            Array(repeating: 64, count: 32)
        ]
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                MusubiProviderBundleAttestationRecordV1.self,
                from: jsonData(substitutedAttestationDigest)
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
        case MusubiToriiClientV1.providerBundleAttestationPath:
            _ = try await client.findProviderBundleAttestation(
                decoder.decode(MusubiProviderBundleAttestationKeyV1.self, from: request)
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
        case MusubiToriiClientV1.providerBundleAttestationPath:
            return try encodedObject(
                decoder.decode(MusubiProviderBundleAttestationKeyV1.self, from: data)
            )
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
            return try encodedObject(
                decoder.decode(MusubiExactReleaseSnapshotV1.self, from: data)
            )
        case MusubiToriiClientV1.providerBundleAttestationPath:
            return try encodedObject(
                decoder.decode(MusubiProviderBundleAttestationRecordV1.self, from: data)
            )
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

    private func populatedArchiveLocationResponse(locationFills: [Int]) throws -> [String: Any] {
        let fixtureRoute = try route(MusubiToriiClientV1.archiveLocationsPath)
        var response = try object(deepMutableCopy(fixtureRoute["response"]))
        var archive = try object(response["archive"])
        let archiveID = try XCTUnwrap(archive["archive_id"])
        let locationIDs: [Any] = locationFills.map {
            [Array(repeating: $0, count: 32)]
        }
        archive["location_ids"] = locationIDs
        response["archive"] = archive
        response["items"] = locationIDs.map {
            archiveLocationItem(locationID: $0, archiveID: archiveID)
        }
        return response
    }

    private func archiveLocationItem(locationID: Any, archiveID: Any) -> [String: Any] {
        [
            "location_id": locationID,
            "archive_id": archiveID,
            "pin_manifest": [Array(repeating: 21, count: 32)],
            "replication_order": [Array(repeating: 22, count: 32)],
            "providers": [[String(repeating: "3F", count: 32)]],
            "provider_attestation_set_digest": [Array(repeating: 23, count: 32)],
            "renew_after_epoch": 60,
            "expires_at_epoch": 120,
            "finalized_height": 50,
            "revision": 1,
            "state": ["kind": "Healthy", "value": NSNull()] as [String: Any],
        ]
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

    private func emptyFirstPage(_ value: Any?) throws -> [String: Any] {
        var response = try object(deepMutableCopy(value))
        response["items"] = [Any]()
        response["next_cursor"] = NSNull()
        return response
    }

    private func packageQueryWithOtherName(_ value: Any?) throws -> [String: Any] {
        var query = try object(deepMutableCopy(value))
        var package = try object(query["package"])
        package["name"] = ["other-package"]
        query["package"] = package
        return query
    }

    private func finalizedCursor(
        snapshot: [String: Any],
        lastKey: String,
        queryHashFill: Int = 1
    ) -> [String: Any] {
        [
            "snapshot": snapshot,
            "query_hash": [Array(repeating: queryHashFill, count: 32)],
            "last_key": lastKey,
            "caller": NSNull(),
        ]
    }

    private var expectedPaths: Set<String> {
        [
            MusubiToriiClientV1.exactPackagePath,
            MusubiToriiClientV1.exactReleasePath,
            MusubiToriiClientV1.providerBundleAttestationPath,
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
