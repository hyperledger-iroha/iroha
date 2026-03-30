package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class OfflineToriiClientCashTest {
    @Test
    fun cashEndpointsPostCanonicalPathsAndBodies() {
        val cases = listOf(
            Triple("/v1/offline/cash/setup", """{"operation_id":"setup-1"}""", { client: OfflineToriiClient, body: String -> client.setupCash(body) }),
            Triple("/v1/offline/cash/load", """{"operation_id":"load-1"}""", { client: OfflineToriiClient, body: String -> client.loadCash(body) }),
            Triple("/v1/offline/cash/refresh", """{"operation_id":"refresh-1"}""", { client: OfflineToriiClient, body: String -> client.refreshCash(body) }),
            Triple("/v1/offline/cash/sync", """{"operation_id":"sync-1"}""", { client: OfflineToriiClient, body: String -> client.syncCash(body) }),
            Triple("/v1/offline/cash/redeem", """{"operation_id":"redeem-1"}""", { client: OfflineToriiClient, body: String -> client.redeemCash(body) }),
        )
        val responseJson = """{"lineage_state":{"lineage_id":"deadbeef"}}"""

        for ((path, requestJson, operation) in cases) {
            val executor = CapturingExecutor(responseJson)
            val client = OfflineToriiClient.builder()
                .executor(executor)
                .baseUri(URI.create("https://example.com"))
                .build()

            val response = operation(client, requestJson).join()

            assertEquals(responseJson, response)
            assertEquals("POST", executor.lastRequest.method)
            assertEquals(path, executor.lastRequest.uri.path)
            assertEquals(requestJson, executor.lastBody)
            assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
            assertEquals("application/json", firstHeader(executor.lastRequest, "Content-Type"))
        }
    }

    @Test
    fun revocationBundleUsesCanonicalGetPath() {
        val responseJson = """{"bundle":{"issued_at_ms":1}}"""
        val executor = CapturingExecutor(responseJson)
        val client = OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

        val response = client.getRevocationBundleJson().join()

        assertEquals(responseJson, response)
        assertEquals("GET", executor.lastRequest.method)
        assertEquals("/v1/offline/revocations/bundle", executor.lastRequest.uri.path)
        assertEquals("", executor.lastBody)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
    }

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
