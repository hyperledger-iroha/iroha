package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.offline.OfflineToriiException

class OfflineToriiClientReadinessTest {
    @Test
    fun readinessUsesCanonicalGetPathAndParsesBody() {
        val executor = CapturingExecutor(
            """
            {
              "asset_definition_id": "xor#wonderland",
              "evaluated_block_height": 18446744073709551615,
              "evaluated_block_hash": "abababababababababababababababababababababababababababababababab",
              "ready": false,
              "blockers": [
                {"code": "offline_disabled", "message": "Offline transfers are disabled"}
              ]
            }
            """.trimIndent(),
        )
        val client = OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

        val readiness = client.getOfflineReadiness("xor#wonderland").join()

        assertEquals("GET", executor.lastRequest.method)
        assertEquals("/v1/offline/readiness", executor.lastRequest.uri.path)
        assertEquals("asset_definition_id=xor%23wonderland", executor.lastRequest.uri.rawQuery)
        assertEquals("", executor.lastBody)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
        assertEquals("xor#wonderland", readiness.assetDefinitionId)
        assertEquals(BigInteger("18446744073709551615"), readiness.evaluatedBlockHeight)
        assertEquals("ab".repeat(32), readiness.evaluatedBlockHash)
        assertEquals(false, readiness.ready)
        assertEquals(1, readiness.blockers.size)
        assertEquals("offline_disabled", readiness.blockers.single().code)
        assertEquals("Offline transfers are disabled", readiness.blockers.single().message)
    }

    @Test
    fun readinessRejectsNonCanonicalResponses() {
        val cases = listOf(
            canonicalReadinessBody(height = "\"7\"") to
                "evaluated_block_height must be a JSON integer number",
            canonicalReadinessBody(height = "-1") to
                "evaluated_block_height must fit in an unsigned 64-bit integer",
            canonicalReadinessBody(height = "18446744073709551616") to
                "evaluated_block_height must fit in an unsigned 64-bit integer",
            canonicalReadinessBody(blockHash = "\"AB${"ab".repeat(31)}\"") to
                "evaluated_block_hash must be exact lowercase 32-byte hexadecimal",
            canonicalReadinessBody(ready = "1") to "ready must be a boolean",
            canonicalReadinessBody(asset = "\" xor#wonderland\"") to
                "asset_definition_id must be an exact non-empty string",
            canonicalReadinessBody(blockers = "[{\"code\":\"blocked\",\"message\":1}]") to
                "blockers[0].message must be a string",
            canonicalReadinessBody(
                ready = "false",
                blockers = "[{\"code\":\"Bad-Code\",\"message\":\"no\"}]",
            ) to "code must be a 1-64 character lowercase stable identifier",
            canonicalReadinessBody(ready = "false", blockers = "[]") to
                "ready must be true exactly when blockers is empty",
            canonicalReadinessBody(
                ready = "true",
                blockers = "[{\"code\":\"blocked\",\"message\":\"no\"}]",
            ) to "ready must be true exactly when blockers is empty",
        )

        for ((body, message) in cases) {
            val error = assertFailsWith<CompletionException> { readinessFromBody(body) }
            val cause = error.cause
            assertTrue(cause is OfflineToriiException)
            assertTrue(cause.cause?.message?.contains(message) == true, cause.cause?.message)
        }
    }

    @Test
    fun readinessIgnoresUnknownObjectMembers() {
        val readiness = readinessFromBody(
            canonicalReadinessBody(
                extra = "\"future_top_level\": {\"ignored\": true},",
                ready = "false",
                blockers =
                    "[{\"code\":\"2fa_required\",\"message\":\"no\",\"future_detail\":7}]",
            ),
        )

        assertEquals("2fa_required", readiness.blockers.single().code)
    }

    @Test
    fun readinessRequiresJsonResponseMediaType() {
        for (headers in listOf(emptyMap(), mapOf("Content-Type" to listOf("text/plain")))) {
            val client = OfflineToriiClient.builder()
                .executor(CapturingExecutor(canonicalReadinessBody(), headers))
                .baseUri(URI.create("https://example.com"))
                .build()
            val error = assertFailsWith<CompletionException> {
                client.getOfflineReadiness("xor#wonderland").join()
            }
            assertTrue(error.cause is OfflineToriiException)
        }
    }

    @Test
    fun readinessRejectsMalformedUtf8WithoutReplacement() {
        val payload = canonicalReadinessBody().toByteArray(StandardCharsets.UTF_8)
        val marker = "xor#wonderland".toByteArray(StandardCharsets.US_ASCII)
        val offset = payload.indexOf(marker)
        require(offset >= 0)
        payload[offset] = 0xc3.toByte()
        val client = OfflineToriiClient.builder()
            .executor(BinaryCapturingExecutor(payload))
            .baseUri(URI.create("https://example.com"))
            .build()

        val error = assertFailsWith<CompletionException> {
            client.getOfflineReadiness("xor#wonderland").join()
        }
        assertTrue(error.cause is OfflineToriiException)
        assertTrue(error.cause?.cause?.message?.contains("valid UTF-8") == true)
    }

    private fun readinessFromBody(responseBody: String) = OfflineToriiClient.builder()
        .executor(CapturingExecutor(responseBody))
        .baseUri(URI.create("https://example.com"))
        .build()
        .getOfflineReadiness("xor#wonderland")
        .join()

    private fun canonicalReadinessBody(
        extra: String = "",
        asset: String = "\"xor#wonderland\"",
        height: String = "7",
        blockHash: String = "\"${"ab".repeat(32)}\"",
        ready: String = "true",
        blockers: String = "[]",
    ): String = """
        {
          $extra
          "asset_definition_id": $asset,
          "evaluated_block_height": $height,
          "evaluated_block_hash": $blockHash,
          "ready": $ready,
          "blockers": $blockers
        }
    """.trimIndent()

    private class CapturingExecutor(
        private val responseBody: String,
        private val responseHeaders: Map<String, List<String>> = mapOf(
            "Content-Type" to listOf("application/json"),
        ),
    ) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var lastBody: String = ""

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            lastBody = String(request.body, StandardCharsets.UTF_8)
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(responseBody.toByteArray(StandardCharsets.UTF_8))
                    .setHeaders(responseHeaders)
                    .build(),
            )
        }
    }

    private class BinaryCapturingExecutor(
        private val responseBody: ByteArray,
    ) : HttpTransportExecutor {
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
            CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(responseBody)
                    .addHeader("Content-Type", "application/json")
                    .build(),
            )
    }

    private fun ByteArray.indexOf(needle: ByteArray): Int {
        if (needle.isEmpty() || needle.size > size) return -1
        for (offset in 0..size - needle.size) {
            if (needle.indices.all { index -> this[offset + index] == needle[index] }) {
                return offset
            }
        }
        return -1
    }

    private fun firstHeader(request: TransportRequest, name: String): String? = request.headers
        .entries
        .firstOrNull { it.key.equals(name, ignoreCase = true) }
        ?.value
        ?.firstOrNull()
}
