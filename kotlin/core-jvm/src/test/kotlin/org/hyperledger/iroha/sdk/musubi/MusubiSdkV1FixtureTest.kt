package org.hyperledger.iroha.sdk.musubi

import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.MusubiToriiClientV1
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

/** Cross-SDK checks for the Rust-owned Musubi first-release JSON fixture. */
class MusubiSdkV1FixtureTest {
    @Test
    fun canonicalNamesVersionsAndRequirementsMatchRustFixture() {
        val root = fixture()
        assertEquals("iroha-musubi-sdk-v1", root["format"])
        assertEquals(1L, root["fixture_version"])
        assertEquals("iroha_data_model::musubi", root["rust_owner"])

        val canonical = objectValue(root["canonical"])
        val namespace = MusubiNamespaceV1(newtypeText(canonical["namespace"]))
        val packageName = MusubiPackageNameV1(newtypeText(canonical["package_name"]))
        assertWireEquals(canonical["namespace"], namespace)
        assertWireEquals(canonical["package_name"], packageName)

        val expectedVersion = canonical["version"]
        val version = MusubiVersionV1.parse("1.2.3-rc.1")
        assertEquals("1.2.3-rc.1", version.canonicalText())
        assertWireEquals(expectedVersion, version)

        arrayValue(canonical["requirements"]).forEach { fixtureValue ->
            val item = objectValue(fixtureValue)
            val requirement = MusubiVersionReqV1.parse(item["text"] as String)
            assertWireEquals(item["wire"], requirement)
        }
        arrayValue(canonical["requirement_aliases"]).forEach { fixtureValue ->
            val item = objectValue(fixtureValue)
            val requirement = MusubiVersionReqV1.parse(item["input"] as String)
            assertEquals(item["canonical"], requirement.canonicalText())
            assertWireEquals(item["wire"], requirement)
        }
        arrayValue(canonical["requirement_matches"]).forEach { fixtureValue ->
            val item = objectValue(fixtureValue)
            val requirement = MusubiVersionReqV1.parse(item["requirement"] as String)
            val candidate = MusubiVersionV1.parse(item["candidate"] as String)
            assertEquals(item["matches"], requirement.matches(candidate))
        }
    }

    @Test
    fun archiveCommitmentUsesTheFullBundlePayloadCeiling() {
        val sourceCeilingPlusOne = java.math.BigInteger.valueOf(
            64L * 1024L * 1024L + 1L,
        )
        val bundleCeiling = java.math.BigInteger.valueOf(MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1)

        assertEquals(96L * 1024L * 1024L, MUSUBI_MAX_BUNDLE_PAYLOAD_BYTES_V1)
        assertEquals(
            sourceCeilingPlusOne,
            archiveCommitment(sourceCeilingPlusOne).contentLength,
        )
        assertEquals(bundleCeiling, archiveCommitment(bundleCeiling).contentLength)
        assertFailsWith<IllegalArgumentException> {
            archiveCommitment(bundleCeiling.add(java.math.BigInteger.ONE))
        }
    }

    @Test
    fun finalizedCursorAcceptsMaximumVersionAndMaintainerKeys() {
        val maximumU64 = java.math.BigInteger("18446744073709551615")
        val maximumIdentifier = MusubiPrereleaseIdentifierV1.alphaNumeric("z".repeat(64))
        val maximumVersion = MusubiVersionV1(
            maximumU64,
            maximumU64,
            maximumU64,
            List(16) { maximumIdentifier },
        )
        assertEquals(
            1_102,
            maximumVersion.canonicalText().toByteArray(StandardCharsets.UTF_8).size,
        )

        val snapshot = MusubiRegistrySnapshotV1(
            java.math.BigInteger.ONE,
            ByteArray(32) { 7 },
            java.math.BigInteger.ONE,
        )
        val queryHash = MusubiDigest32V1(ByteArray(32) { 8 })
        MusubiFinalizedCursorV1(
            snapshot,
            queryHash,
            maximumVersion.canonicalText(),
            null,
        )

        val maximumMaintainerKey = "ab".repeat(8_192) +
            "|pending-" + "cd".repeat(32)
        assertEquals(16_457, MUSUBI_MAX_CURSOR_KEY_BYTES_V1)
        assertEquals(
            MUSUBI_MAX_CURSOR_KEY_BYTES_V1,
            maximumMaintainerKey.toByteArray(StandardCharsets.UTF_8).size,
        )
        MusubiFinalizedCursorV1(snapshot, queryHash, maximumMaintainerKey, null)
        assertFails {
            MusubiFinalizedCursorV1(snapshot, queryHash, maximumMaintainerKey + "0", null)
        }
    }

    @Test
    fun orderedPrefixUsesExactStructuralMaximumAndPortablePackagePrefix() {
        val maximumPrefix = "n".repeat(255) + "/" + "p".repeat(64)
        assertEquals(320, MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1)
        assertEquals(
            MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1,
            maximumPrefix.toByteArray(StandardCharsets.UTF_8).size,
        )
        MusubiOrderedPrefixQueryV1(maximumPrefix)
        MusubiOrderedPrefixQueryV1("sora/")
        MusubiOrderedPrefixQueryV1("sora/pkg-")

        val overlong = "n".repeat(255) + "/" + "p".repeat(65)
        assertEquals(
            MUSUBI_MAX_ORDERED_PREFIX_BYTES_V1 + 1,
            overlong.toByteArray(StandardCharsets.UTF_8).size,
        )
        assertFails { MusubiOrderedPrefixQueryV1(overlong) }

        listOf(
            "sora",
            "/pkg",
            "a.b.c/pkg",
            "sora/pkg/extra",
            "sora/-pkg",
            "sora/pkg--extra",
            "sora/Pkg",
            "sora/pkg_name",
            "sora/päkg",
        ).forEach { malformed ->
            assertFails("accepted malformed ordered prefix $malformed") {
                MusubiOrderedPrefixQueryV1(malformed)
            }
        }
    }

    @Test
    fun decodedComparatorRequirementsRejectNoncanonicalExactForms() {
        val first = MusubiVersionComparatorV1(
            MusubiComparatorOpV1.EQUAL,
            MusubiVersionV1.parse("1.0.0"),
        )
        val second = MusubiVersionComparatorV1(
            MusubiComparatorOpV1.EQUAL,
            MusubiVersionV1.parse("2.0.0"),
        )
        assertFails {
            MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.COMPARATORS,
                comparators = listOf(first),
            )
        }
        assertFails {
            MusubiVersionReqV1.fromWire(
                MusubiVersionReqV1.Kind.COMPARATORS,
                comparators = listOf(first, second),
            )
        }
    }

    @Test
    fun nameBackedFieldsRejectEveryUnicodeBidiControl() {
        val controls = charArrayOf(
            '\u061C',
            '\u200E',
            '\u200F',
            '\u202A',
            '\u202B',
            '\u202C',
            '\u202D',
            '\u202E',
            '\u2066',
            '\u2067',
            '\u2068',
            '\u2069',
        )
        controls.forEach { control ->
            assertFails { MusubiNamespaceV1("domain${control}.dataspace") }
            assertFails { MusubiPackageScopeV1.domain("domain$control") }
        }
    }

    @Test
    fun everyTypedRouteRoundTripsExactRequestAndResponseJson() {
        val routes = routes()
        assertEquals(EXPECTED_PATHS, routes.map { it["path"] }.toSet())
        assertEquals(12, routes.size)

        routes.forEach { route ->
            val path = route["path"] as String
            val request = MusubiJsonV1.decodeQuery(path, route["request"])
            val response = MusubiJsonV1.decodeResponse(path, route["response"])
            assertWireEquals(route["request"], request)
            assertWireEquals(route["response"], response)
        }
    }

    @Test
    fun archiveRetentionIsBoundedTypedAndBindsTheExactRequest() {
        val route = routes().first {
            it["path"] == MusubiToriiClientV1.ARCHIVE_RETENTION_PATH
        }
        val request = MusubiJsonV1.decodeQuery(
            MusubiToriiClientV1.ARCHIVE_RETENTION_PATH,
            route["request"],
        ) as MusubiArchiveRetentionQueryV1
        val page = MusubiJsonV1.parseArchiveRetentionPage(
            MusubiJsonV1.encode(route["response"]),
        )
        page.requireMatches(request)
        assertEquals(4, page.items.size)
        assertEquals(java.math.BigInteger("1700000000000"), page.finalizedTimeMs)
        assertEquals(listOf(true, true, false, false), page.items.map { it.mustRetain() })

        val zeroTime = objectValue(deepMutableCopy(route["response"]))
        zeroTime["finalized_time_ms"] = 0L
        MusubiJsonV1.parseArchiveRetentionPage(MusubiJsonV1.encode(zeroTime))
            .requireMatches(request)
        val negativeTime = objectValue(deepMutableCopy(route["response"]))
        negativeTime["finalized_time_ms"] = -1L
        assertFails {
            MusubiJsonV1.parseArchiveRetentionPage(MusubiJsonV1.encode(negativeTime))
        }
        val missingTime = objectValue(deepMutableCopy(route["response"]))
        missingTime.remove("finalized_time_ms")
        assertFails {
            MusubiJsonV1.parseArchiveRetentionPage(MusubiJsonV1.encode(missingTime))
        }

        val mismatched = objectValue(deepMutableCopy(route["response"]))
        val first = objectValue(arrayValue(mismatched["items"])[0])
        first["archive_id"] = listOf(List(32) { 17L })
        val executor = FixtureExecutor(
            mapOf(
                MusubiToriiClientV1.ARCHIVE_RETENTION_PATH to MusubiJsonV1.encode(mismatched),
            ),
        )
        val client = MusubiToriiClientV1.builder()
            .baseUri(URI.create("http://localhost:8080"))
            .executor(executor)
            .build()
        assertFails { client.findArchiveRetention(request).join() }
    }

    @Test
    fun archiveLocationPageRejectsNoncurrentOrUnorderedItems() {
        val valid = populatedArchiveLocationResponse()
        val page = MusubiJsonV1.decodeResponse(
            MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
            valid,
        ) as MusubiArchiveLocationPageV1
        assertEquals(2, page.items.size)
        assertEquals(49L, page.items[0].finalizedHeight.toLong())

        fun assertRejected(mutate: (MutableMap<String, Any?>) -> Unit) {
            val response = objectValue(deepMutableCopy(valid))
            mutate(response)
            assertFails {
                MusubiJsonV1.decodeResponse(
                    MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
                    response,
                )
            }
        }

        assertRejected { response ->
            response["items"] = arrayValue(response["items"])
                .reversed()
                .map(::deepMutableCopy)
                .toMutableList()
        }
        assertRejected { response ->
            val first = arrayValue(response["items"])[0]
            response["items"] = MutableList(2) { deepMutableCopy(first) }
        }
        assertRejected { response ->
            val item = arrayValue(response["items"])[0]
            response["items"] = MutableList(5) { deepMutableCopy(item) }
        }
        assertRejected { response ->
            val archive = objectValue(response["archive"])
            response["items"] = mutableListOf(
                archiveLocationWire(3L, archive["archive_id"], 49L, 1L, "Healthy"),
            )
        }
        assertRejected { response ->
            val first = objectValue(arrayValue(response["items"])[0])
            first["archive_id"] = digestWire(9L)
        }
        assertRejected { response ->
            val first = objectValue(arrayValue(response["items"])[0])
            objectValue(first["state"])["kind"] = "Retired"
        }
        assertRejected { response ->
            objectValue(arrayValue(response["items"])[0])["finalized_height"] = 51L
        }
        assertRejected { response ->
            objectValue(arrayValue(response["items"])[0])["revision"] = 3L
        }
        assertRejected { response ->
            objectValue(arrayValue(response["items"])[0])["provider_attestation_set_digest"] =
                digestWire(0L)
        }
        assertRejected { response ->
            val first = objectValue(arrayValue(response["items"])[0])
            first.remove("provider_attestation_set_digest")
            first["provider_attestations"] = emptyList<Any?>()
        }
    }

    @Test
    fun maintainerDirectoryDecodesAcceptedAndPendingInvitationVariants() {
        val route = routes().first { it["path"] == MusubiToriiClientV1.MAINTAINERS_PATH }
        val page = MusubiJsonV1.parseMaintainerPage(MusubiJsonV1.encode(route["response"]))
        assertEquals(2, page.items.size)

        val accepted = page.items[0] as MusubiMaintainerDirectoryEntryV1.Accepted
        assertEquals(MusubiMaintainerDirectoryEntryV1.Kind.ACCEPTED, accepted.kind)
        assertEquals("Owner", accepted.member.roleKind)
        assertEquals(42L, accepted.member.acceptedAtHeight.toLong())

        val pending = page.items[1] as MusubiMaintainerDirectoryEntryV1.PendingInvitation
        assertEquals(MusubiMaintainerDirectoryEntryV1.Kind.PENDING_INVITATION, pending.kind)
        assertEquals("Maintainer", pending.invitation.roleKind)
        assertEquals("Pending", pending.invitation.stateKind)
        assertEquals(2L, pending.invitation.expectedGovernanceRevision.toLong())
        assertTrue(pending.invitation.inviteId.bytes().all { it.toInt() == 13 })

        val malformedResponse = objectValue(deepMutableCopy(route["response"]))
        val malformedPending = objectValue(
            objectValue(arrayValue(malformedResponse["items"])[1])["value"],
        )
        objectValue(malformedPending["state"])["kind"] = "Accepted"
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.MAINTAINERS_PATH,
                malformedResponse,
            )
        }
    }

    @Test
    fun maintainerCursorUsesOpaqueAccountEqualityAndCanonicalShape() {
        val route = routes().first { it["path"] == MusubiToriiClientV1.MAINTAINERS_PATH }
        val page = MusubiJsonV1.parseMaintainerPage(MusubiJsonV1.encode(route["response"]))

        fun response(firstItem: Int, lastKey: String): MutableMap<String, Any?> {
            val response = objectValue(deepMutableCopy(route["response"]))
            response["items"] = mutableListOf(
                deepMutableCopy(arrayValue(response["items"])[firstItem]),
            )
            val controls = objectValue(objectValue(response["query"])["page"])
            controls["cursor"] = finalizedCursorWire(response["snapshot"], lastKey, 1L)
            return response
        }

        val acceptedKey = MusubiValidationV1.maintainerCursorKey(page.items[0])
        val pendingKey = MusubiValidationV1.maintainerCursorKey(page.items[1])
        val accountToken = acceptedKey.substringBefore('|')
        MusubiJsonV1.decodeResponse(
            MusubiToriiClientV1.MAINTAINERS_PATH,
            response(0, "$accountToken|pending-${"01".repeat(32)}"),
        )
        MusubiJsonV1.decodeResponse(
            MusubiToriiClientV1.MAINTAINERS_PATH,
            response(1, "$accountToken|accepted"),
        )

        page.items.forEachIndexed { index, item ->
            assertFails {
                MusubiJsonV1.decodeResponse(
                    MusubiToriiClientV1.MAINTAINERS_PATH,
                    response(index, MusubiValidationV1.maintainerCursorKey(item)),
                )
            }
        }
        val repeatedLater = objectValue(deepMutableCopy(route["response"]))
        objectValue(objectValue(repeatedLater["query"])["page"])["cursor"] =
            finalizedCursorWire(repeatedLater["snapshot"], pendingKey, 1L)
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.MAINTAINERS_PATH,
                repeatedLater,
            )
        }
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.MAINTAINERS_PATH,
                response(0, "00|accepted"),
            )
        }
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.MAINTAINERS_PATH,
                response(0, accountToken.dropLast(2) + "|accepted"),
            )
        }
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.MAINTAINERS_PATH,
                response(0, "ab".repeat(8_193) + "|accepted"),
            )
        }
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.MAINTAINERS_PATH,
                response(1, "ff|pending-${"00".repeat(32)}"),
            )
        }
    }

    @Test
    fun readOnlyClientPostsEachTypedQueryToItsExactV1Route() {
        val routes = routes()
        val executor = FixtureExecutor(routes.associate {
            it["path"] as String to MusubiJsonV1.encode(it["response"])
        })
        val client = MusubiToriiClientV1.builder()
            .baseUri(URI.create("http://localhost:8080"))
            .executor(executor)
            .build()

        routes.forEach { route ->
            val path = route["path"] as String
            val request = MusubiJsonV1.decodeQuery(path, route["request"])
            invoke(client, path, request)
            val captured = executor.requests.last()
            assertEquals("POST", captured.method)
            assertEquals(path, captured.uri.path)
            assertEquals(route["request"], parseJson(captured.body))
            assertEquals(32L * 1024L * 1024L, captured.maximumResponseBytes)
        }
        assertEquals(EXPECTED_PATHS, executor.requests.map { it.uri.path }.toSet())
    }

    @Test
    fun readOnlyClientRejectsResponsesThatDoNotMatchRequests() {
        fun assertRejected(path: String, mutate: (MutableMap<String, Any?>) -> Unit) {
            val route = routes().first { it["path"] == path }
            val requestValue = objectValue(deepMutableCopy(route["request"]))
            mutate(requestValue)
            val request = MusubiJsonV1.decodeQuery(path, requestValue)
            val executor = FixtureExecutor(mapOf(path to MusubiJsonV1.encode(route["response"])))
            val client = MusubiToriiClientV1.builder()
                .baseUri(URI.create("http://localhost:8080"))
                .executor(executor)
                .build()
            assertFails { invoke(client, path, request) }
        }

        assertRejected(MusubiToriiClientV1.EXACT_PACKAGE_PATH) { request ->
            objectValue(request["package"])["name"] = listOf("another-package")
        }
        assertRejected(MusubiToriiClientV1.EXACT_RELEASE_PATH) { request ->
            objectValue(objectValue(request["release"])["package"])["name"] =
                listOf("another-package")
        }
        assertRejected(MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH) { request ->
            request["provider_id"] = listOf("FE".repeat(32))
        }
        for (path in listOf(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            MusubiToriiClientV1.VERSIONS_PATH,
        )) {
            assertRejected(path) { request ->
                val snapshot = objectValue(
                    deepMutableCopy(
                        objectValue(routes().first { it["path"] == path }["response"])["snapshot"],
                    ),
                )
                snapshot["finalized_height"] = 49L
                objectValue(request["page"])["cursor"] = linkedMapOf(
                    "snapshot" to snapshot,
                    "query_hash" to digestWire(19L),
                    "last_key" to "fixture-last-key",
                    "caller" to null,
                )
            }
        }
        assertRejected(MusubiToriiClientV1.MAINTAINERS_PATH) { request ->
            objectValue(request["package"])["name"] = listOf("another-package")
        }
        assertRejected(MusubiToriiClientV1.MAINTAINERS_PATH) { request ->
            objectValue(request["page"])["limit"] = 1L
        }
        assertRejected(MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH) { request ->
            request["archive_id"] = digestWire(9L)
        }
        assertRejected(MusubiToriiClientV1.ALIAS_PATH) { request ->
            request["alias"] = listOf("another-alias")
        }
        assertRejected(MusubiToriiClientV1.ALIAS_HISTORY_PATH) { request ->
            request["alias"] = listOf("another-alias")
        }
        assertRejected(MusubiToriiClientV1.ORDERED_PREFIX_PATH) { request ->
            request["prefix"] = listOf("another/")
        }
        assertRejected(MusubiToriiClientV1.SEARCH_PATH) { request ->
            val response = objectValue(
                routes().first { it["path"] == MusubiToriiClientV1.SEARCH_PATH }["response"],
            )
            val firstPackage = objectValue(arrayValue(response["items"])[0])["package"]
            objectValue(request["page"])["cursor"] = linkedMapOf(
                "snapshot" to deepMutableCopy(response["snapshot"]),
                "query_hash" to digestWire(20L),
                "last_package" to deepMutableCopy(firstPackage),
            )
        }

        val versionsRoute = routes().first { it["path"] == MusubiToriiClientV1.VERSIONS_PATH }
        val requestValue = objectValue(deepMutableCopy(versionsRoute["request"]))
        val responseValue = objectValue(deepMutableCopy(versionsRoute["response"]))
        val snapshot = deepMutableCopy(responseValue["snapshot"])
        objectValue(requestValue["page"])["cursor"] = linkedMapOf(
            "snapshot" to deepMutableCopy(snapshot),
            "query_hash" to digestWire(19L),
            "last_key" to "1.0.0",
            "caller" to null,
        )
        responseValue["next_cursor"] = linkedMapOf(
            "snapshot" to deepMutableCopy(snapshot),
            "query_hash" to digestWire(20L),
            "last_key" to "1.2.3",
            "caller" to null,
        )
        val executor = FixtureExecutor(
            mapOf(MusubiToriiClientV1.VERSIONS_PATH to MusubiJsonV1.encode(responseValue)),
        )
        val client = MusubiToriiClientV1.builder()
            .baseUri(URI.create("http://localhost:8080"))
            .executor(executor)
            .build()
        assertFails {
            invoke(
                client,
                MusubiToriiClientV1.VERSIONS_PATH,
                MusubiJsonV1.decodeQuery(MusubiToriiClientV1.VERSIONS_PATH, requestValue),
            )
        }
    }

    @Test
    fun pagedClientsBindEchoedQueriesOnEmptyFirstPages() {
        val paths = listOf(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            MusubiToriiClientV1.VERSIONS_PATH,
            MusubiToriiClientV1.MAINTAINERS_PATH,
            MusubiToriiClientV1.ALIAS_HISTORY_PATH,
            MusubiToriiClientV1.ORDERED_PREFIX_PATH,
            MusubiToriiClientV1.SEARCH_PATH,
        )
        paths.forEach { path ->
            val route = routes().first { it["path"] == path }
            val request = MusubiJsonV1.decodeQuery(path, route["request"])
            val response = objectValue(deepMutableCopy(route["response"]))
            response["items"] = mutableListOf<Any?>()
            response["next_cursor"] = null
            objectValue(objectValue(response["query"])["page"])["limit"] = 49L
            val executor = FixtureExecutor(mapOf(path to MusubiJsonV1.encode(response)))
            val client = MusubiToriiClientV1.builder()
                .baseUri(URI.create("http://localhost:8080"))
                .executor(executor)
                .build()
            assertFails { invoke(client, path, request) }
        }
    }

    @Test
    fun echoedPagesRejectTamperedStructuredCursorBoundariesAndContinuations() {
        fun assertRejected(path: String, response: MutableMap<String, Any?>) {
            assertFails { MusubiJsonV1.decodeResponse(path, response) }
        }

        val versionsRoute = routes().first { it["path"] == MusubiToriiClientV1.VERSIONS_PATH }
        val canonical = objectValue(deepMutableCopy(versionsRoute["response"]))
        val snapshot = deepMutableCopy(canonical["snapshot"])
        val versionQuery = objectValue(canonical["query"])
        val versionControls = objectValue(versionQuery["page"])
        versionControls["limit"] = 1L
        versionControls["cursor"] = finalizedCursorWire(snapshot, "1.0.0", 19L)
        canonical["next_cursor"] = finalizedCursorWire(snapshot, "1.2.3", 19L)
        MusubiJsonV1.decodeResponse(MusubiToriiClientV1.VERSIONS_PATH, canonical)

        val wrongTail = objectValue(deepMutableCopy(canonical))
        objectValue(wrongTail["next_cursor"])["last_key"] = "9.9.9"
        assertRejected(MusubiToriiClientV1.VERSIONS_PATH, wrongTail)

        val shortPage = objectValue(deepMutableCopy(canonical))
        objectValue(objectValue(shortPage["query"])["page"])["limit"] = 2L
        assertRejected(MusubiToriiClientV1.VERSIONS_PATH, shortPage)

        val wrongHash = objectValue(deepMutableCopy(canonical))
        objectValue(wrongHash["next_cursor"])["query_hash"] = digestWire(20L)
        assertRejected(MusubiToriiClientV1.VERSIONS_PATH, wrongHash)

        val wrongSnapshot = objectValue(deepMutableCopy(canonical))
        objectValue(objectValue(wrongSnapshot["next_cursor"])["snapshot"])["finalized_height"] = 49L
        assertRejected(MusubiToriiClientV1.VERSIONS_PATH, wrongSnapshot)

        for (cursorField in listOf("request", "next")) {
            val callerBound = objectValue(deepMutableCopy(canonical))
            val cursor = if (cursorField == "request") {
                objectValue(objectValue(objectValue(callerBound["query"])["page"])["cursor"])
            } else {
                objectValue(callerBound["next_cursor"])
            }
            cursor["caller"] = "unexpected-caller"
            assertRejected(MusubiToriiClientV1.VERSIONS_PATH, callerBound)
        }

        for (boundary in listOf("01.0.0", "1.2.3")) {
            val badBoundary = objectValue(deepMutableCopy(canonical))
            objectValue(
                objectValue(objectValue(badBoundary["query"])["page"])["cursor"],
            )["last_key"] = boundary
            assertRejected(MusubiToriiClientV1.VERSIONS_PATH, badBoundary)
        }

        val malformedFinalizedBoundaries = mapOf(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH to "not-semver",
            MusubiToriiClientV1.MAINTAINERS_PATH to "not-a-maintainer-key",
            MusubiToriiClientV1.ALIAS_HISTORY_PATH to "math:1",
            MusubiToriiClientV1.ORDERED_PREFIX_PATH to "sora/zzzz",
        )
        malformedFinalizedBoundaries.forEach { (path, lastKey) ->
            val route = routes().first { it["path"] == path }
            val response = objectValue(deepMutableCopy(route["response"]))
            val controls = objectValue(objectValue(response["query"])["page"])
            controls["cursor"] = finalizedCursorWire(response["snapshot"], lastKey, 19L)
            assertRejected(path, response)
        }

        val searchRoute = routes().first { it["path"] == MusubiToriiClientV1.SEARCH_PATH }
        val searchBoundary = objectValue(deepMutableCopy(searchRoute["response"]))
        val firstSearchPackage = objectValue(arrayValue(searchBoundary["items"])[0])["package"]
        objectValue(objectValue(searchBoundary["query"])["page"])["cursor"] = linkedMapOf(
            "snapshot" to deepMutableCopy(searchBoundary["snapshot"]),
            "query_hash" to digestWire(19L),
            "last_package" to deepMutableCopy(firstSearchPackage),
        )
        assertRejected(MusubiToriiClientV1.SEARCH_PATH, searchBoundary)

        val searchContinuation = objectValue(deepMutableCopy(searchRoute["response"]))
        objectValue(objectValue(searchContinuation["query"])["page"])["limit"] = 1L
        searchContinuation["next_cursor"] = linkedMapOf(
            "snapshot" to deepMutableCopy(searchContinuation["snapshot"]),
            "query_hash" to digestWire(19L),
            "last_package" to deepMutableCopy(firstSearchPackage),
        )
        MusubiJsonV1.decodeResponse(MusubiToriiClientV1.SEARCH_PATH, searchContinuation)

        val shortSearch = objectValue(deepMutableCopy(searchContinuation))
        objectValue(objectValue(shortSearch["query"])["page"])["limit"] = 2L
        assertRejected(MusubiToriiClientV1.SEARCH_PATH, shortSearch)

        val wrongSearchTail = objectValue(deepMutableCopy(searchContinuation))
        objectValue(objectValue(wrongSearchTail["next_cursor"])["last_package"])["name"] =
            listOf("zzz")
        assertRejected(MusubiToriiClientV1.SEARCH_PATH, wrongSearchTail)

        val wrongSearchHash = objectValue(deepMutableCopy(searchContinuation))
        val previousPackage = objectValue(deepMutableCopy(firstSearchPackage))
        previousPackage["name"] = listOf("aaa")
        objectValue(objectValue(wrongSearchHash["query"])["page"])["cursor"] = linkedMapOf(
            "snapshot" to deepMutableCopy(wrongSearchHash["snapshot"]),
            "query_hash" to digestWire(20L),
            "last_package" to previousPackage,
        )
        assertRejected(MusubiToriiClientV1.SEARCH_PATH, wrongSearchHash)
    }

    @Test
    fun resolverIndexAcceptsShortByteBudgetPageAndBindsContinuationTail() {
        val resolverRoute = routes().first {
            it["path"] == MusubiToriiClientV1.RESOLVER_INDEX_PATH
        }
        val response = objectValue(deepMutableCopy(resolverRoute["response"]))
        val releaseResponse = objectValue(
            routes().first { it["path"] == MusubiToriiClientV1.EXACT_RELEASE_PATH }["response"],
        )
        response["items"] = mutableListOf(deepMutableCopy(releaseResponse["universal_release"]))

        val query = objectValue(response["query"])
        val page = objectValue(query["page"])
        page["limit"] = 2L
        page["cursor"] = finalizedCursorWire(response["snapshot"], "1.2.2", 19L)
        response["next_cursor"] = finalizedCursorWire(response["snapshot"], "1.2.3", 19L)

        MusubiJsonV1.decodeResponse(MusubiToriiClientV1.RESOLVER_INDEX_PATH, response)

        val duplicateAliases = objectValue(deepMutableCopy(response))
        val resolverRow = objectValue(arrayValue(duplicateAliases["items"]).single())
        val packageTemplate = objectValue(
            objectValue(resolverRow["release"])["package"],
        )
        val requirementTemplate = objectValue(
            arrayValue(objectValue(fixture()["canonical"])["requirements"]).first(),
        )["wire"]
        fun dependency(name: String): MutableMap<String, Any?> {
            val packageId = objectValue(deepMutableCopy(packageTemplate))
            packageId["name"] = mutableListOf(name)
            return linkedMapOf(
                "alias" to "shared",
                "package" to packageId,
                "requirement" to deepMutableCopy(requirementTemplate),
            )
        }
        resolverRow["dependencies"] = mutableListOf(
            dependency("alpha-dependency"),
            dependency("beta-dependency"),
        )
        val duplicateAliasError = assertFailsWith<IllegalArgumentException> {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.RESOLVER_INDEX_PATH,
                duplicateAliases,
            )
        }
        assertTrue(
            duplicateAliasError.message.orEmpty().contains("unique parent-local aliases"),
        )

        val wrongTail = objectValue(deepMutableCopy(response))
        objectValue(wrongTail["next_cursor"])["last_key"] = "1.2.4"
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.RESOLVER_INDEX_PATH, wrongTail)
        }

        val wrongPriorHash = objectValue(deepMutableCopy(response))
        objectValue(
            objectValue(objectValue(wrongPriorHash["query"])["page"])["cursor"],
        )["query_hash"] = digestWire(20L)
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.RESOLVER_INDEX_PATH, wrongPriorHash)
        }
    }

    @Test
    fun versionMaintainerAliasHistoryAndResolverPagesRequireStrictOrder() {
        val versionsRoute = routes().first { it["path"] == MusubiToriiClientV1.VERSIONS_PATH }
        val ascending = objectValue(deepMutableCopy(versionsRoute["response"]))
        val upper = objectValue(arrayValue(ascending["items"])[0])
        val lower = objectValue(deepMutableCopy(upper))
        lower["patch"] = 2L
        ascending["items"] = mutableListOf(lower, upper)
        objectValue(objectValue(ascending["query"])["page"])["limit"] = 0L
        val zeroLimitRequest = MusubiJsonV1.decodeQuery(
            MusubiToriiClientV1.VERSIONS_PATH,
            ascending["query"],
        ) as MusubiPackagePageQueryV1
        MusubiJsonV1.parseVersionPage(MusubiJsonV1.encode(ascending))
            .requireVersionMatches(zeroLimitRequest)
        assertFails { MusubiPageRequestV1(101) }

        for (items in listOf(
            mutableListOf(deepMutableCopy(upper), deepMutableCopy(lower)),
            MutableList(2) { deepMutableCopy(upper) },
        )) {
            val response = objectValue(deepMutableCopy(versionsRoute["response"]))
            response["items"] = items
            assertFails {
                MusubiJsonV1.decodeResponse(MusubiToriiClientV1.VERSIONS_PATH, response)
            }
        }

        for (path in listOf(
            MusubiToriiClientV1.MAINTAINERS_PATH,
            MusubiToriiClientV1.ALIAS_HISTORY_PATH,
        )) {
            val route = routes().first { it["path"] == path }
            val original = arrayValue(objectValue(route["response"])["items"])
            val malformed = objectValue(deepMutableCopy(route["response"]))
            malformed["items"] = if (original.size > 1) {
                original.reversed().map(::deepMutableCopy).toMutableList()
            } else {
                MutableList(2) { deepMutableCopy(original[0]) }
            }
            assertFails { MusubiJsonV1.decodeResponse(path, malformed) }
        }

        val resolverRequest = MusubiJsonV1.decodeQuery(
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            routes().first { it["path"] == MusubiToriiClientV1.RESOLVER_INDEX_PATH }["request"],
        ) as MusubiResolverIndexQueryV1
        val snapshot = MusubiRegistrySnapshotV1(
            java.math.BigInteger.ONE,
            ByteArray(32) { 7 },
            java.math.BigInteger.ONE,
        )
        val first = MusubiResolverReleaseRowV1(
            MusubiReleaseIdV1(resolverRequest.packageId, MusubiVersionV1.parse("1.0.0")),
            java.math.BigInteger.ONE,
            java.math.BigInteger.ONE,
            emptyMap(),
        )
        val second = MusubiResolverReleaseRowV1(
            MusubiReleaseIdV1(resolverRequest.packageId, MusubiVersionV1.parse("2.0.0")),
            java.math.BigInteger.ONE,
            java.math.BigInteger.ONE,
            emptyMap(),
        )
        assertFails {
            MusubiResolverIndexPageV1(
                resolverRequest,
                "fixture-chain",
                ByteArray(32) { 8 },
                listOf(second, first),
                null,
                snapshot,
            )
        }
        assertFails {
            MusubiResolverIndexPageV1(
                resolverRequest,
                "fixture-chain",
                ByteArray(32) { 8 },
                listOf(first, first),
                null,
                snapshot,
            )
        }
        val page = MusubiResolverIndexPageV1(
            resolverRequest,
            "fixture-chain",
            ByteArray(32) { 8 },
            listOf(first),
            null,
            snapshot,
        )
        assertFails { page.requireMatches(resolverRequest) }
        val differentPackage = MusubiPackageIdV1(
            resolverRequest.packageId.homeDataspace,
            resolverRequest.packageId.scope,
            MusubiPackageNameV1("another-package"),
        )
        assertFails {
            page.requireMatches(MusubiResolverIndexQueryV1(differentPackage, null))
        }
    }

    @Test
    fun governedTakedownRequiresOnlyAppliedHeight() {
        val exactRelease = routes().first {
            it["path"] == MusubiToriiClientV1.EXACT_RELEASE_PATH
        }
        val canonical = objectValue(deepMutableCopy(exactRelease["response"]))
        val homeRelease = objectValue(canonical["home_release"])
        val governedState = linkedMapOf<String, Any?>(
            "kind" to "TakenDown",
            "value" to linkedMapOf(
                "action_digest" to homeRelease["release_digest"],
                "reason" to listOf("security response"),
                "applied_at_height" to 50L,
            ),
        )
        homeRelease["artifact_governance"] = governedState
        objectValue(homeRelease["revisions"])["artifact_governance"] = 2L
        val universalRelease = objectValue(canonical["universal_release"])
        objectValue(universalRelease["selection"])["governance"] =
            deepMutableCopy(governedState)
        MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, canonical)

        val legacy = objectValue(deepMutableCopy(canonical))
        val legacyGovernance = objectValue(objectValue(legacy["home_release"])["artifact_governance"])
        val legacyPayload = objectValue(legacyGovernance["value"])
        legacyPayload["enacted_at_height"] = legacyPayload.remove("applied_at_height")
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, legacy)
        }

        val zeroHeight = objectValue(deepMutableCopy(canonical))
        val zeroGovernance = objectValue(
            objectValue(zeroHeight["home_release"])["artifact_governance"],
        )
        objectValue(zeroGovernance["value"])["applied_at_height"] = 0L
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, zeroHeight)
        }
    }

    @Test
    fun exactReleaseRejectsSubstitutedProjectionsAndNonfinalAnchors() {
        val exactRelease = routes().first {
            it["path"] == MusubiToriiClientV1.EXACT_RELEASE_PATH
        }

        fun assertRejected(mutate: (MutableMap<String, Any?>) -> Unit) {
            val response = objectValue(deepMutableCopy(exactRelease["response"]))
            mutate(response)
            assertFails {
                MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, response)
            }
        }

        assertRejected { response ->
            objectValue(response["universal_release"])["release_digest"] = digestWire(64L)
        }
        assertRejected { response ->
            val replacement = digestWire(64L)
            objectValue(response["home_release"])["release_digest"] =
                deepMutableCopy(replacement)
            objectValue(response["universal_release"])["release_digest"] =
                deepMutableCopy(replacement)
        }
        assertRejected { response ->
            objectValue(response["home_release"])["published_by"] = "not-an-account"
        }
        assertRejected { response ->
            objectValue(response["home_release"])["published_at_height"] = 0L
        }
        assertRejected { response ->
            objectValue(response["universal_release"])["archive_id"] = digestWire(65L)
        }
        assertRejected { response ->
            objectValue(response["universal_release"])["interface_digest"] = digestWire(66L)
        }
        assertRejected { response ->
            val universal = objectValue(response["universal_release"])
            objectValue(universal["abi"])["abi_hash"] = MutableList<Any?>(32) { 67L }
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            val release = objectValue(objectValue(home["manifest"])["release"])
            val dependencyPackage = objectValue(deepMutableCopy(release["package"]))
            dependencyPackage["name"] = listOf("dependency")
            objectValue(response["universal_release"])["dependencies"] = mutableListOf(
                linkedMapOf(
                    "alias" to "dependency",
                    "package" to dependencyPackage,
                    "requirement" to linkedMapOf("kind" to "Any", "value" to null),
                ),
            )
        }
        assertRejected { response ->
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            objectValue(selection["yank"])["reason"] = listOf("substituted state")
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            selection["governance"] = linkedMapOf(
                "kind" to "TakenDown",
                "value" to linkedMapOf(
                    "action_digest" to deepMutableCopy(home["release_digest"]),
                    "reason" to listOf("substituted governance"),
                    "applied_at_height" to 50L,
                ),
            )
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            val yank = objectValue(home["yank"])
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            yank["revision"] = 10L
            objectValue(home["revisions"])["yank"] = 10L
            objectValue(selection["yank"])["revision"] = 10L
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            objectValue(home["yank"])["changed_at_height"] = 42L
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            objectValue(selection["yank"])["changed_at_height"] = 42L
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            objectValue(home["yank"])["changed_at_height"] = 51L
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            objectValue(selection["yank"])["changed_at_height"] = 51L
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            objectValue(home["revisions"])["artifact_governance"] = 10L
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            val governed = linkedMapOf<String, Any?>(
                "kind" to "TakenDown",
                "value" to linkedMapOf(
                    "action_digest" to deepMutableCopy(home["release_digest"]),
                    "reason" to listOf("premature takedown"),
                    "applied_at_height" to 42L,
                ),
            )
            home["artifact_governance"] = governed
            objectValue(home["revisions"])["artifact_governance"] = 2L
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            selection["governance"] = deepMutableCopy(governed)
        }
        assertRejected { response ->
            val home = objectValue(response["home_release"])
            val governed = linkedMapOf<String, Any?>(
                "kind" to "TakenDown",
                "value" to linkedMapOf(
                    "action_digest" to deepMutableCopy(home["release_digest"]),
                    "reason" to listOf("nonfinal takedown"),
                    "applied_at_height" to 51L,
                ),
            )
            home["artifact_governance"] = governed
            objectValue(home["revisions"])["artifact_governance"] = 2L
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            selection["governance"] = deepMutableCopy(governed)
        }
        assertRejected { response ->
            objectValue(response["home_release"])["published_at_height"] = 51L
        }
        assertRejected { response ->
            objectValue(response["universal_release"])["index_revision"] = 10L
        }
        assertRejected { response ->
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            objectValue(selection["storage"])["finalized_height"] = 51L
        }
        assertRejected { response ->
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            objectValue(selection["storage"])["finalized_block_hash"] =
                MutableList<Any?>(32) { 6L }
        }
        assertRejected { response ->
            val snapshot = objectValue(response["snapshot"])
            snapshot["finalized_height"] = 1L
            val home = objectValue(response["home_release"])
            home["published_at_height"] = 1L
            objectValue(home["yank"])["changed_at_height"] = 1L
            val selection = objectValue(objectValue(response["universal_release"])["selection"])
            objectValue(selection["yank"])["changed_at_height"] = 1L
            objectValue(selection["storage"])["finalized_height"] = 1L
        }
    }

    @Test
    fun rejectsNoncanonicalInputsUnknownFieldsAndUnknownAbiVersions() {
        val rejected = objectValue(fixture()["reject"])
        arrayValue(rejected["names"]).forEach {
            assertFails { MusubiPackageNameV1(it as String) }
        }
        arrayValue(rejected["versions"]).forEach {
            assertFails { MusubiVersionV1.parse(it as String) }
        }
        arrayValue(rejected["requirements"]).forEach {
            assertFails { MusubiVersionReqV1.parse(it as String) }
        }
        assertTrue(arrayValue(rejected["fixture_versions"]).none { it == 1L })

        val exactPackage = routes().first { it["path"] == MusubiToriiClientV1.EXACT_PACKAGE_PATH }
        val requestWithUnknown = LinkedHashMap(objectValue(exactPackage["request"]))
        requestWithUnknown["legacy"] = true
        assertFails {
            MusubiJsonV1.decodeQuery(MusubiToriiClientV1.EXACT_PACKAGE_PATH, requestWithUnknown)
        }
        val responseWithUnknown = LinkedHashMap(objectValue(exactPackage["response"]))
        responseWithUnknown["legacy"] = true
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_PACKAGE_PATH, responseWithUnknown)
        }

        val exactRelease = routes().first { it["path"] == MusubiToriiClientV1.EXACT_RELEASE_PATH }
        val response = deepMutableCopy(exactRelease["response"])
        val manifest = objectValue(objectValue(objectValue(response)["home_release"])["manifest"])
        val abi = objectValue(manifest["abi"])
        abi["abi_version"] = 2L
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, response)
        }

        val futureStorage = objectValue(deepMutableCopy(exactRelease["response"]))
        objectValue(futureStorage["universal_release"])["index_revision"] = 8L
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.EXACT_RELEASE_PATH,
                futureStorage,
            )
        }

        val providerRoute = routes().first {
            it["path"] == MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH
        }
        val substitutedAttestationDigest =
            objectValue(deepMutableCopy(providerRoute["response"]))
        substitutedAttestationDigest["attestation_digest"] = digestWire(64L)
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH,
                substitutedAttestationDigest,
            )
        }

        val archiveRoute = routes().first {
            it["path"] == MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH
        }
        val archiveResponse = objectValue(deepMutableCopy(archiveRoute["response"]))
        val archive = objectValue(archiveResponse["archive"])
        val receipt = objectValue(archive["staging_receipt"])
        val payload = objectValue(receipt["payload"])
        payload["version"] = 2L
        assertFails {
            MusubiJsonV1.decodeResponse(
                MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
                archiveResponse,
            )
        }
    }

    private fun archiveCommitment(
        contentLength: java.math.BigInteger,
    ): MusubiArchiveCommitmentV1 {
        val rootCid = ByteArray(36) { 7 }
        rootCid[0] = 1
        rootCid[1] = 113
        rootCid[2] = 31
        rootCid[3] = 32
        val digest = MusubiDigest32V1(ByteArray(32) { 9 })
        return MusubiArchiveCommitmentV1(
            rootCid,
            MusubiChunkerProfileHandleV1(
                1L,
                "sorafs",
                "sf1",
                "1.0.0",
                java.math.BigInteger.valueOf(31L),
            ),
            digest,
            digest,
            contentLength,
            digest,
            java.math.BigInteger.valueOf(MUSUBI_MAX_CAR_BYTES_V1),
            digest,
            digest,
            digest,
            1L,
            1L,
        )
    }

    private fun invoke(
        client: MusubiToriiClientV1,
        path: String,
        request: MusubiWireValueV1,
    ) {
        when (path) {
            MusubiToriiClientV1.EXACT_PACKAGE_PATH ->
                client.findExactPackage(request as MusubiExactPackageQueryV1).join()
            MusubiToriiClientV1.EXACT_RELEASE_PATH ->
                client.findExactRelease(request as MusubiExactReleaseQueryV1).join()
            MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH ->
                client.findProviderBundleAttestation(
                    request as MusubiProviderBundleAttestationKeyV1,
                ).join()
            MusubiToriiClientV1.RESOLVER_INDEX_PATH ->
                client.findResolverIndex(request as MusubiResolverIndexQueryV1).join()
            MusubiToriiClientV1.VERSIONS_PATH ->
                client.findVersions(request as MusubiPackagePageQueryV1).join()
            MusubiToriiClientV1.MAINTAINERS_PATH ->
                client.findMaintainers(request as MusubiPackagePageQueryV1).join()
            MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH ->
                client.findArchiveLocations(request as MusubiArchiveLocationQueryV1).join()
            MusubiToriiClientV1.ARCHIVE_RETENTION_PATH ->
                client.findArchiveRetention(request as MusubiArchiveRetentionQueryV1).join()
            MusubiToriiClientV1.ALIAS_PATH ->
                client.findAlias(request as MusubiAliasQueryV1).join()
            MusubiToriiClientV1.ALIAS_HISTORY_PATH ->
                client.findAliasHistory(request as MusubiAliasQueryV1).join()
            MusubiToriiClientV1.ORDERED_PREFIX_PATH ->
                client.findOrderedPrefix(request as MusubiOrderedPrefixQueryV1).join()
            MusubiToriiClientV1.SEARCH_PATH ->
                client.search(request as MusubiSearchQueryV1).join()
            else -> error("unhandled fixture path $path")
        }
    }

    private fun routes(): List<MutableMap<String, Any?>> =
        arrayValue(fixture()["routes"]).map(::objectValue)

    private fun populatedArchiveLocationResponse(): MutableMap<String, Any?> {
        val route = routes().first {
            it["path"] == MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH
        }
        val response = objectValue(deepMutableCopy(route["response"]))
        val archive = objectValue(response["archive"])
        val first = archiveLocationWire(1L, archive["archive_id"], 49L, 1L, "Healthy")
        val second = archiveLocationWire(2L, archive["archive_id"], 50L, 2L, "Degraded")
        archive["location_revision"] = 2L
        archive["location_ids"] = mutableListOf(
            deepMutableCopy(first["location_id"]),
            deepMutableCopy(second["location_id"]),
        )
        response["items"] = mutableListOf(first, second)
        return response
    }

    private fun archiveLocationWire(
        locationByte: Long,
        archiveId: Any?,
        finalizedHeight: Long,
        revision: Long,
        state: String,
    ): MutableMap<String, Any?> = linkedMapOf(
        "location_id" to digestWire(locationByte),
        "archive_id" to deepMutableCopy(archiveId),
        "pin_manifest" to digestWire(61L),
        "replication_order" to digestWire(62L),
        "providers" to listOf(listOf("3F".repeat(32))),
        "provider_attestation_set_digest" to digestWire(63L),
        "renew_after_epoch" to 1L,
        "expires_at_epoch" to 2L,
        "finalized_height" to finalizedHeight,
        "revision" to revision,
        "state" to linkedMapOf<String, Any?>("kind" to state, "value" to null),
    )

    private fun digestWire(fill: Long): MutableList<Any?> =
        mutableListOf(MutableList<Any?>(32) { fill })

    private fun finalizedCursorWire(
        snapshot: Any?,
        lastKey: String,
        queryHashByte: Long,
    ): MutableMap<String, Any?> = linkedMapOf(
        "snapshot" to deepMutableCopy(snapshot),
        "query_hash" to digestWire(queryHashByte),
        "last_key" to lastKey,
        "caller" to null,
    )

    private fun fixture(): MutableMap<String, Any?> =
        objectValue(parseJson(Files.readAllBytes(findFixture())))

    private fun findFixture(): Path {
        var current = Paths.get("").toAbsolutePath().normalize()
        repeat(8) {
            val candidate = current.resolve("fixtures/musubi/sdk_v1.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent ?: return@repeat
        }
        error("fixtures/musubi/sdk_v1.json was not found from the test working directory")
    }

    private fun assertWireEquals(expected: Any?, value: MusubiWireValueV1) {
        assertEquals(expected, parseJson(value.toJsonBytes()))
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectValue(value: Any?): MutableMap<String, Any?> =
        value as? MutableMap<String, Any?> ?: error("fixture value must be an object")

    @Suppress("UNCHECKED_CAST")
    private fun arrayValue(value: Any?): List<Any?> =
        value as? List<Any?> ?: error("fixture value must be an array")

    private fun newtypeText(value: Any?): String = arrayValue(value).single() as String

    private fun parseJson(bytes: ByteArray): Any? =
        JsonParser.parse(String(bytes, StandardCharsets.UTF_8))

    private fun deepMutableCopy(value: Any?): Any? = when (value) {
        is Map<*, *> -> LinkedHashMap<String, Any?>().also { copy ->
            value.forEach { (key, item) -> copy[key as String] = deepMutableCopy(item) }
        }
        is List<*> -> value.map(::deepMutableCopy).toMutableList()
        else -> value
    }

    private class FixtureExecutor(private val responses: Map<String, ByteArray>) :
        HttpTransportExecutor {
        val requests = ArrayList<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val response = responses[request.uri.path] ?: error("unexpected route ${request.uri.path}")
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(response)
                    .addHeader("Content-Type", "application/json")
                    .build(),
            )
        }
    }

    companion object {
        private val EXPECTED_PATHS = setOf(
            MusubiToriiClientV1.EXACT_PACKAGE_PATH,
            MusubiToriiClientV1.EXACT_RELEASE_PATH,
            MusubiToriiClientV1.PROVIDER_BUNDLE_ATTESTATION_PATH,
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            MusubiToriiClientV1.VERSIONS_PATH,
            MusubiToriiClientV1.MAINTAINERS_PATH,
            MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
            MusubiToriiClientV1.ARCHIVE_RETENTION_PATH,
            MusubiToriiClientV1.ALIAS_PATH,
            MusubiToriiClientV1.ALIAS_HISTORY_PATH,
            MusubiToriiClientV1.ORDERED_PREFIX_PATH,
            MusubiToriiClientV1.SEARCH_PATH,
        )
    }
}
