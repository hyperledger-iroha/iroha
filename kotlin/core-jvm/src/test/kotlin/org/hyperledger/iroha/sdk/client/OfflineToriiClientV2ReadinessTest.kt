package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class OfflineToriiClientV2ReadinessTest {
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

        val readiness = client.getOfflineV2Readiness().join()

        assertEquals("GET", executor.lastRequest.method)
        assertEquals("/v1/offline/v2/readiness", executor.lastRequest.uri.path)
        assertEquals("", executor.lastBody)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
        assertEquals(true, readiness.offlineTelemetry)
        assertEquals(true, readiness.offlineKagemushaRecursiveCompactAvailable)
        assertEquals("recursive_compact_v1", readiness.offlineKagemushaRecursiveCompactMode)
        assertEquals(7, readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion)
        assertEquals("kagemusha-recursive-compact-v1", readiness.offlineKagemushaRecursiveCompactCircuitId)
        assertEquals(true, readiness.offlineKagemushaRecursiveCompactArtifactsAvailable)
    }

    @Test
    fun readinessParsesShortAbi7Aliases() {
        val readiness = readinessFromBody(
            """
            {
              "offline_telemetry": true,
              "offline_kagemusha_abi7": true,
              "offline_kagemusha_abi7_mode": "recursive_compact_v1",
              "offline_kagemusha_abi7_bridge_abi_version": "7",
              "offline_kagemusha_abi7_circuit_id": "kagemusha-recursive-compact-v1",
              "offline_kagemusha_abi7_artifacts": true
            }
            """.trimIndent(),
        )

        assertEquals(true, readiness.offlineKagemushaRecursiveCompactAvailable)
        assertEquals("recursive_compact_v1", readiness.offlineKagemushaRecursiveCompactMode)
        assertEquals(7, readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion)
        assertEquals("kagemusha-recursive-compact-v1", readiness.offlineKagemushaRecursiveCompactCircuitId)
        assertEquals(true, readiness.offlineKagemushaRecursiveCompactArtifactsAvailable)
    }

    @Test
    fun readinessPrefersShortAbi7AliasesOverVerboseValues() {
        val readiness = readinessFromBody(
            """
            {
              "offline_telemetry": true,
              "offline_kagemusha_abi7": false,
              "offline_kagemusha_recursive_compact_available": true,
              "offline_kagemusha_abi7_mode": "short-mode",
              "offline_kagemusha_recursive_compact_mode": "verbose-mode",
              "offline_kagemusha_abi7_bridge_abi_version": 7,
              "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 99,
              "offline_kagemusha_abi7_circuit_id": "short-circuit",
              "offline_kagemusha_recursive_compact_circuit_id": "verbose-circuit",
              "offline_kagemusha_abi7_artifacts": false,
              "offline_kagemusha_recursive_compact_artifacts_available": true
            }
            """.trimIndent(),
        )

        assertEquals(false, readiness.offlineKagemushaRecursiveCompactAvailable)
        assertEquals("short-mode", readiness.offlineKagemushaRecursiveCompactMode)
        assertEquals(7, readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion)
        assertEquals("short-circuit", readiness.offlineKagemushaRecursiveCompactCircuitId)
        assertEquals(false, readiness.offlineKagemushaRecursiveCompactArtifactsAvailable)
    }

    private fun readinessFromBody(responseBody: String) = OfflineToriiClient.builder()
        .executor(CapturingExecutor(responseBody))
        .baseUri(URI.create("https://example.com"))
        .build()
        .getOfflineV2Readiness()
        .join()

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
