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
        assertEquals(false, readiness.ready)
        assertEquals(1, readiness.blockers.size)
        assertEquals("offline_disabled", readiness.blockers.single().code)
        assertEquals("Offline transfers are disabled", readiness.blockers.single().message)
    }

    @Test
    fun readinessRejectsNonCanonicalResponses() {
        val cases = listOf(
            canonicalReadinessBody(extra = "\"offline_telemetry\": true,") to
                "root.offline_telemetry is not a supported field",
            canonicalReadinessBody(height = "\"7\"") to
                "evaluated_block_height must be a JSON integer number",
            canonicalReadinessBody(height = "-1") to
                "evaluated_block_height must fit in an unsigned 64-bit integer",
            canonicalReadinessBody(height = "18446744073709551616") to
                "evaluated_block_height must fit in an unsigned 64-bit integer",
            canonicalReadinessBody(ready = "1") to "ready must be a boolean",
            canonicalReadinessBody(asset = "\" xor#wonderland\"") to
                "asset_definition_id must be an exact non-empty string",
            canonicalReadinessBody(blockers = "[{\"code\":\"blocked\",\"message\":\"no\",\"extra\":1}]") to
                "blockers[0].extra is not a supported field",
        )

        for ((body, message) in cases) {
            val error = assertFailsWith<CompletionException> { readinessFromBody(body) }
            val cause = error.cause
            assertTrue(cause is OfflineToriiException)
            assertTrue(cause.cause?.message?.contains(message) == true, cause.cause?.message)
        }
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
        ready: String = "true",
        blockers: String = "[]",
    ): String = """
        {
          $extra
          "asset_definition_id": $asset,
          "evaluated_block_height": $height,
          "ready": $ready,
          "blockers": $blockers
        }
    """.trimIndent()

    private class CapturingExecutor(
        private val responseBody: String,
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
