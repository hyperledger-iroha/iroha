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
              "offline_note": true,
              "offline_one_use_keys": true,
              "offline_recursive_note_proof": false,
              "offline_fountain_qr": true,
              "offline_sync_optional": true,
              "offline_telemetry": true,
              "offline_kagemusha_abi7": true,
              "offline_kagemusha_abi7_mode": "recursive_compact_v1",
              "offline_kagemusha_abi7_bridge_abi_version": 7,
              "offline_kagemusha_abi7_circuit_id": "kagemusha-recursive-compact-v1",
              "offline_kagemusha_abi7_artifacts": true
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
        assertEquals(true, readiness.offlineNote)
        assertEquals(true, readiness.offlineOneUseKeys)
        assertEquals(false, readiness.offlineRecursiveNoteProof)
        assertEquals(true, readiness.offlineFountainQr)
        assertEquals(true, readiness.offlineSyncOptional)
        assertEquals(true, readiness.offlineTelemetry)
        assertEquals(true, readiness.offlineKagemushaAbi7)
        assertEquals("recursive_compact_v1", readiness.offlineKagemushaAbi7Mode)
        assertEquals(7, readiness.offlineKagemushaAbi7BridgeAbiVersion)
        assertEquals("kagemusha-recursive-compact-v1", readiness.offlineKagemushaAbi7CircuitId)
        assertEquals(true, readiness.offlineKagemushaAbi7Artifacts)
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
