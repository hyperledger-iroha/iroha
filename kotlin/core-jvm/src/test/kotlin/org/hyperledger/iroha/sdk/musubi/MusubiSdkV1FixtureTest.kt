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
    }

    @Test
    fun everyTypedRouteRoundTripsExactRequestAndResponseJson() {
        val routes = routes()
        assertEquals(EXPECTED_PATHS, routes.map { it["path"] }.toSet())
        assertEquals(9, routes.size)

        routes.forEach { route ->
            val path = route["path"] as String
            val request = MusubiJsonV1.decodeQuery(path, route["request"])
            val response = MusubiJsonV1.decodeResponse(path, route["response"])
            assertWireEquals(route["request"], request)
            assertWireEquals(route["response"], response)
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
            when (path) {
                MusubiToriiClientV1.EXACT_PACKAGE_PATH ->
                    client.findExactPackage(request as MusubiExactPackageQueryV1).join()
                MusubiToriiClientV1.EXACT_RELEASE_PATH ->
                    client.findExactRelease(request as MusubiExactReleaseQueryV1).join()
                MusubiToriiClientV1.RESOLVER_INDEX_PATH ->
                    client.findResolverIndex(request as MusubiResolverIndexQueryV1).join()
                MusubiToriiClientV1.VERSIONS_PATH ->
                    client.findVersions(request as MusubiPackagePageQueryV1).join()
                MusubiToriiClientV1.MAINTAINERS_PATH ->
                    client.findMaintainers(request as MusubiPackagePageQueryV1).join()
                MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH ->
                    client.findArchiveLocations(request as MusubiArchiveLocationQueryV1).join()
                MusubiToriiClientV1.ALIAS_PATH ->
                    client.findAlias(request as MusubiAliasQueryV1).join()
                MusubiToriiClientV1.ALIAS_HISTORY_PATH ->
                    client.findAliasHistory(request as MusubiAliasQueryV1).join()
                MusubiToriiClientV1.ORDERED_PREFIX_PATH ->
                    client.findOrderedPrefix(request as MusubiOrderedPrefixQueryV1).join()
                else -> error("unhandled fixture path $path")
            }
            val captured = executor.requests.last()
            assertEquals("POST", captured.method)
            assertEquals(path, captured.uri.path)
            assertEquals(route["request"], parseJson(captured.body))
        }
        assertEquals(EXPECTED_PATHS, executor.requests.map { it.uri.path }.toSet())
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
        val manifest = objectValue(objectValue(response)["manifest"])
        val abi = objectValue(manifest["abi"])
        abi["abi_version"] = 2L
        assertFails {
            MusubiJsonV1.decodeResponse(MusubiToriiClientV1.EXACT_RELEASE_PATH, response)
        }
    }

    private fun routes(): List<MutableMap<String, Any?>> =
        arrayValue(fixture()["routes"]).map(::objectValue)

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
            MusubiToriiClientV1.RESOLVER_INDEX_PATH,
            MusubiToriiClientV1.VERSIONS_PATH,
            MusubiToriiClientV1.MAINTAINERS_PATH,
            MusubiToriiClientV1.ARCHIVE_LOCATIONS_PATH,
            MusubiToriiClientV1.ALIAS_PATH,
            MusubiToriiClientV1.ALIAS_HISTORY_PATH,
            MusubiToriiClientV1.ORDERED_PREFIX_PATH,
        )
    }
}
