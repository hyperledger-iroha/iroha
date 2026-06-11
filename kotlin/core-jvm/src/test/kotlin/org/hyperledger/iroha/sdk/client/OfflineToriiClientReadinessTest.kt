package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class OfflineToriiClientReadinessTest {
    @Test
    fun readinessUsesCanonicalGetPathAndParsesBody() {
        val executor = CapturingExecutor(
            """
            {
              "offline_telemetry": true,
              "offline_kagemusha_recursive_compact_available": true,
              "offline_kagemusha_recursive_compact_mode": "recursive_compact_v1",
              "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 7,
              "offline_kagemusha_recursive_compact_circuit_id": "kagemusha-recursive-compact-v1",
              "offline_kagemusha_recursive_compact_artifacts_available": true
            }
            """.trimIndent(),
        )
        val client = OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

        val readiness = client.getOfflineReadiness().join()

        assertEquals("GET", executor.lastRequest.method)
        assertEquals("/v1/offline/readiness", executor.lastRequest.uri.path)
        assertEquals("", executor.lastBody)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
        assertEquals(false, readiness.offlineNote)
        assertEquals(false, readiness.offlineOneUseKeys)
        assertEquals(false, readiness.offlineRecursiveNoteProof)
        assertEquals(false, readiness.offlineFountainQr)
        assertEquals(false, readiness.offlineSyncOptional)
        assertEquals(true, readiness.offlineTelemetry)
        assertEquals(true, readiness.offlineKagemushaRecursiveCompactAvailable)
        assertEquals("recursive_compact_v1", readiness.offlineKagemushaRecursiveCompactMode)
        assertEquals(7, readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion)
        assertEquals("kagemusha-recursive-compact-v1", readiness.offlineKagemushaRecursiveCompactCircuitId)
        assertEquals(true, readiness.offlineKagemushaRecursiveCompactArtifactsAvailable)
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
