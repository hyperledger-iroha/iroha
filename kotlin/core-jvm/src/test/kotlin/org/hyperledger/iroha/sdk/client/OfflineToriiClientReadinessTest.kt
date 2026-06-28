package org.hyperledger.iroha.sdk.client

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
              "offline_telemetry": true,
              "offline_kagemusha_recursive_compact_available": true,
              "offline_kagemusha_recursive_compact_mode": "recursive_compact_v1",
              "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 7,
              "offline_kagemusha_recursive_compact_circuit_id": "kagemusha-recursive-compact-v1",
              "offline_kagemusha_recursive_compact_artifacts_available": false
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
        assertEquals(false, readiness.offlineKagemushaRecursiveCompactArtifactsAvailable)
    }

    @Test
    fun readinessRejectsRemovedAbi7Aliases() {
        for ((field, message) in removedAbi7ReadinessFieldCases()) {
            assertReadinessFails(
                canonicalReadinessBody(extra = """"$field": true,"""),
                message,
            )
        }
    }

    @Test
    fun readinessRejectsMalformedCanonicalValues() {
        for ((body, message) in malformedCanonicalBodies()) {
            assertReadinessFails(body, message)
        }
    }

    private fun readinessFromBody(responseBody: String) = OfflineToriiClient.builder()
        .executor(CapturingExecutor(responseBody))
        .baseUri(URI.create("https://example.com"))
        .build()
        .getOfflineReadiness()
        .join()

    private fun assertReadinessFails(responseBody: String, expectedMessage: String) {
        val error = assertFailsWith<CompletionException> { readinessFromBody(responseBody) }
        val cause = error.cause
        assertTrue(cause is OfflineToriiException)
        assertTrue(cause.cause?.message?.contains(expectedMessage) == true, cause.cause?.message)
    }

    private fun malformedCanonicalBodies(): List<Pair<String, String>> = listOf(
        canonicalReadinessBody(compactAvailable = "\"true\"") to
            "offline_kagemusha_recursive_compact_available must be a boolean",
        canonicalReadinessBody(compactMode = "\" recursive_compact_v1\"") to
            "offline_kagemusha_recursive_compact_mode must be an exact non-empty string",
        canonicalReadinessBody(compactBridge = "\"007\"") to
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an exact integer string",
        canonicalReadinessBody(compactBridge = "-1") to
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be a positive integer",
        canonicalReadinessBody(compactBridge = "7.5") to
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an integer",
        canonicalReadinessBody(compactBridge = "2147483648") to
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must fit in signed 32-bit range",
        canonicalReadinessBody(compactCircuit = "\"\"") to
            "offline_kagemusha_recursive_compact_circuit_id must be an exact non-empty string",
        canonicalReadinessBody(compactArtifacts = "\"true\"") to
            "offline_kagemusha_recursive_compact_artifacts_available must be a boolean",
    )

    private fun removedAbi7ReadinessFieldCases(): List<Pair<String, String>> = listOf(
        "offline_kagemusha_abi7" to
            "offline_kagemusha_abi7 is not supported; use offline_kagemusha_recursive_compact_*",
        "offline_kagemusha_abi7_mode" to
            "offline_kagemusha_abi7_mode is not supported; use offline_kagemusha_recursive_compact_*",
        "offline_kagemusha_abi7_bridge_abi_version" to
            "offline_kagemusha_abi7_bridge_abi_version is not supported; use offline_kagemusha_recursive_compact_*",
        "offline_kagemusha_abi7_circuit_id" to
            "offline_kagemusha_abi7_circuit_id is not supported; use offline_kagemusha_recursive_compact_*",
        "offline_kagemusha_abi7_artifacts" to
            "offline_kagemusha_abi7_artifacts is not supported; use offline_kagemusha_recursive_compact_*",
    )

    private fun canonicalReadinessBody(
        extra: String = "",
        compactAvailable: String = "true",
        compactMode: String = "\"recursive_compact_v1\"",
        compactBridge: String = "7",
        compactCircuit: String = "\"kagemusha-recursive-compact-v1\"",
        compactArtifacts: String = "true",
    ): String = """
        {
          "offline_telemetry": true,
          $extra
          "offline_kagemusha_recursive_compact_available": $compactAvailable,
          "offline_kagemusha_recursive_compact_mode": $compactMode,
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": $compactBridge,
          "offline_kagemusha_recursive_compact_circuit_id": $compactCircuit,
          "offline_kagemusha_recursive_compact_artifacts_available": $compactArtifacts
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
