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
    fun readinessRejectsConflictingAbi7AliasesAndVerboseValues() {
        for ((body, message) in conflictingAliasBodies()) {
            assertReadinessFails(body, message)
        }
    }

    @Test
    fun readinessRejectsMalformedPresentAliasValues() {
        for ((body, message) in malformedAliasBodies()) {
            assertReadinessFails(body, message)
        }
    }

    @Test
    fun readinessAcceptsMatchingAbi7AliasesAndVerboseValues() {
        val readiness = readinessFromBody(
            """
            {
              "offline_telemetry": true,
              "offline_kagemusha_abi7": true,
              "offline_kagemusha_recursive_compact_available": true,
              "offline_kagemusha_abi7_mode": "recursive_compact_v1",
              "offline_kagemusha_recursive_compact_mode": "recursive_compact_v1",
              "offline_kagemusha_abi7_bridge_abi_version": 7,
              "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 7,
              "offline_kagemusha_abi7_circuit_id": "kagemusha-recursive-compact-v1",
              "offline_kagemusha_recursive_compact_circuit_id": "kagemusha-recursive-compact-v1",
              "offline_kagemusha_abi7_artifacts": true,
              "offline_kagemusha_recursive_compact_artifacts_available": true
            }
            """.trimIndent(),
        )

        assertEquals(true, readiness.offlineKagemushaRecursiveCompactAvailable)
        assertEquals("recursive_compact_v1", readiness.offlineKagemushaRecursiveCompactMode)
        assertEquals(7, readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion)
        assertEquals("kagemusha-recursive-compact-v1", readiness.offlineKagemushaRecursiveCompactCircuitId)
        assertEquals(true, readiness.offlineKagemushaRecursiveCompactArtifactsAvailable)
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

    private fun conflictingAliasBodies(): List<Pair<String, String>> = listOf(
        aliasReadinessBody(abi7 = "false") to
            "offline_kagemusha_abi7 and offline_kagemusha_recursive_compact_available must match",
        aliasReadinessBody(abi7Mode = "\"legacy-mode\"") to
            "offline_kagemusha_abi7_mode and offline_kagemusha_recursive_compact_mode must match",
        aliasReadinessBody(abi7Bridge = "8") to
            "offline_kagemusha_abi7_bridge_abi_version and offline_kagemusha_recursive_compact_required_native_bridge_abi_version must match",
        aliasReadinessBody(abi7Circuit = "\"legacy-circuit\"") to
            "offline_kagemusha_abi7_circuit_id and offline_kagemusha_recursive_compact_circuit_id must match",
        aliasReadinessBody(abi7Artifacts = "false") to
            "offline_kagemusha_abi7_artifacts and offline_kagemusha_recursive_compact_artifacts_available must match",
    )

    private fun malformedAliasBodies(): List<Pair<String, String>> = listOf(
        aliasReadinessBody(abi7 = "\"true\"") to
            "offline_kagemusha_abi7 must be a boolean",
        aliasReadinessBody(abi7Mode = "{}") to
            "offline_kagemusha_abi7_mode must be a string",
        aliasReadinessBody(compactMode = "\" recursive_compact_v1\"") to
            "offline_kagemusha_recursive_compact_mode must be an exact non-empty string",
        aliasReadinessBody(abi7Bridge = "\" 7\"") to
            "offline_kagemusha_abi7_bridge_abi_version must be an exact integer string",
        aliasReadinessBody(compactBridge = "7.5") to
            "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an integer",
        aliasReadinessBody(compactCircuit = "\"\"") to
            "offline_kagemusha_recursive_compact_circuit_id must be an exact non-empty string",
        aliasReadinessBody(compactArtifacts = "\"true\"") to
            "offline_kagemusha_recursive_compact_artifacts_available must be a boolean",
    )

    private fun aliasReadinessBody(
        abi7: String = "true",
        compactAvailable: String = "true",
        abi7Mode: String = "\"recursive_compact_v1\"",
        compactMode: String = "\"recursive_compact_v1\"",
        abi7Bridge: String = "7",
        compactBridge: String = "7",
        abi7Circuit: String = "\"kagemusha-recursive-compact-v1\"",
        compactCircuit: String = "\"kagemusha-recursive-compact-v1\"",
        abi7Artifacts: String = "true",
        compactArtifacts: String = "true",
    ): String = """
        {
          "offline_telemetry": true,
          "offline_kagemusha_abi7": $abi7,
          "offline_kagemusha_recursive_compact_available": $compactAvailable,
          "offline_kagemusha_abi7_mode": $abi7Mode,
          "offline_kagemusha_recursive_compact_mode": $compactMode,
          "offline_kagemusha_abi7_bridge_abi_version": $abi7Bridge,
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": $compactBridge,
          "offline_kagemusha_abi7_circuit_id": $abi7Circuit,
          "offline_kagemusha_recursive_compact_circuit_id": $compactCircuit,
          "offline_kagemusha_abi7_artifacts": $abi7Artifacts,
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
