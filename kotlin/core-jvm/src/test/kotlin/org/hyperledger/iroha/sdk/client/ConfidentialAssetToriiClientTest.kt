package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.offline.OfflineToriiException

class ConfidentialAssetToriiClientTest {
    @Test
    fun rootsUsesCanonicalPostPathAndParsesBody() {
        val root = "01".repeat(32)
        val executor = CapturingExecutor(
            """
            {
              "latest": "$root",
              "roots": ["$root"],
              "height": 1
            }
            """.trimIndent(),
        )
        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

        val response = client.getZkAssetRoots(ZkRootsRequest("usd#bank", 7)).join()

        assertEquals("POST", executor.lastRequest.method)
        assertEquals("/v1/zk/roots", executor.lastRequest.uri.path)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
        assertEquals("application/json", firstHeader(executor.lastRequest, "Content-Type"))
        assertEquals("""{"asset_id":"usd#bank","max":7}""", executor.lastBody)
        assertEquals(root, response.latest)
        assertEquals(listOf(root), response.roots)
        assertEquals(1, response.height)
        assertContentEquals(ByteArray(32) { 1 }, response.getLatestRootBytes())
        assertContentEquals(ByteArray(32) { 1 }, response.getRootBytes(0))
    }

    @Test
    fun emptyLatestRootIsNotNullableOnWireButNullInByteHelper() {
        val response = ZkRootsResponse.parse(
            """
            {"latest":"","roots":[],"height":0}
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )

        assertEquals("", response.latest)
        assertNull(response.getLatestRootBytes())
    }

    @Test
    fun rootsRejectNonCanonicalHexAndNullLatest() {
        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse("AA".repeat(32), emptyList(), 0)
        }
        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse.parse("""{"latest":null,"roots":[],"height":0}""".toByteArray(StandardCharsets.UTF_8))
        }
    }

    @Test
    fun nonSuccessResponsesSurfaceOfflineToriiException() {
        val client = ConfidentialAssetToriiClient.builder()
            .executor(CapturingExecutor("""{"error":"not ready"}""", status = 503, message = "Unavailable"))
            .baseUri(URI.create("https://example.com"))
            .build()

        val error = assertFailsWith<CompletionException> {
            client.getZkAssetRoots(ZkRootsRequest("usd#bank")).join()
        }.cause

        require(error is OfflineToriiException)
        assertEquals(503, error.statusCode)
        require(error.message?.contains("/v1/zk/roots") == true)
    }

    private class CapturingExecutor(
        private val responseBody: String,
        private val status: Int = 200,
        private val message: String = "",
    ) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var lastBody: String = ""

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            lastBody = String(request.body, StandardCharsets.UTF_8)
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(status)
                    .setMessage(message)
                    .setBody(responseBody.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }
    }

    private fun firstHeader(request: TransportRequest, name: String): String? = request.headers
        .entries
        .firstOrNull { it.key.equals(name, ignoreCase = true) }
        ?.value
        ?.firstOrNull()
}
