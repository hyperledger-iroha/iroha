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
import org.hyperledger.iroha.sdk.offline.OfflineReadinessBlocker
import org.hyperledger.iroha.sdk.offline.OfflineToriiException
import org.hyperledger.iroha.sdk.offline.OfflineVerifierId

class OfflineToriiClientReadinessTest {
    @Test
    fun readinessUsesCanonicalGetPathAndParsesBody() {
        val executor = CapturingExecutor(
            canonicalReadinessBody(
                height = "18446744073709551615",
                ready = "false",
                blockers =
                    "[{\"code\":\"offline_disabled\",\"message\":\"Offline transfers are disabled\"}]",
            ),
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
        assertEquals(CANONICAL_ASSET_DEFINITION_ID, readiness.assetDefinitionId)
        assertEquals(9L, readiness.assetScale)
        assertEquals(BigInteger("18446744073709551615"), readiness.evaluatedBlockHeight)
        assertEquals("ab".repeat(32), readiness.evaluatedBlockHash)
        assertEquals("halo2/ipa", readiness.activeTransferVerifier?.id?.backend)
        assertEquals(4096L, readiness.activeTransferVerifier?.maxProofBytes)
        assertEquals(false, readiness.ready)
        assertEquals(1, readiness.blockers.size)
        assertEquals("offline_disabled", readiness.blockers.single().code)
        assertEquals("Offline transfers are disabled", readiness.blockers.single().message)
    }

    @Test
    fun readinessBindsCanonicalSelectorButAllowsAliasResolution() {
        val aliasReadiness = readinessFromBody(canonicalReadinessBody())
        assertEquals(CANONICAL_ASSET_DEFINITION_ID, aliasReadiness.assetDefinitionId)

        val client = OfflineToriiClient.builder()
            .executor(CapturingExecutor(canonicalReadinessBody()))
            .baseUri(URI.create("https://example.com"))
            .build()
        val error = assertFailsWith<CompletionException> {
            client.getOfflineReadiness(OTHER_CANONICAL_ASSET_DEFINITION_ID).join()
        }
        assertTrue(error.cause is OfflineToriiException)
        assertTrue(
            error.cause?.cause?.message?.contains("does not match the requested asset definition") == true,
        )
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
            canonicalReadinessBody(height = "1e1") to
                "evaluated_block_height must be a JSON integer number",
            canonicalReadinessBody(assetScale = "-1") to
                "asset_scale must fit in an unsigned 64-bit integer",
            canonicalReadinessBody(assetScale = "4294967296") to
                "asset_scale must fit in an unsigned 32-bit integer",
            canonicalReadinessBody(assetScale = "29") to
                "asset_scale_unsupported must be present exactly when assetScale exceeds 28",
            canonicalReadinessBody(
                assetScale = "null",
                ready = "false",
                blockers = "[{\"code\":\"blocked\",\"message\":\"no\"}]",
            ) to "asset_scale_unavailable must be present exactly when assetScale is null",
            canonicalReadinessBody(activeVerifier = "null") to
                "transfer_verifier_unavailable must be present exactly when no active verifier is reported",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(maxProofBytes = "0"),
            ) to "maxProofBytes must fit in a positive unsigned 32-bit integer",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(activationHeight = "8"),
            ) to "active_transfer_verifier must be active at evaluated_block_height",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(withdrawalHeight = "7"),
            ) to "active_transfer_verifier must be active at evaluated_block_height",
            canonicalReadinessBody(blockHash = "\"AB${"ab".repeat(31)}\"") to
                "evaluated_block_hash must be exact lowercase 32-byte hexadecimal",
            canonicalReadinessBody(ready = "1") to "ready must be a boolean",
            canonicalReadinessBody(asset = "\" $CANONICAL_ASSET_DEFINITION_ID\"") to
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
            canonicalReadinessBody(
                ready = "false",
                blockers =
                    "[{\"code\":\"blocked\",\"message\":\"one\"},{\"code\":\"blocked\",\"message\":\"two\"}]",
            ) to "blockers must not repeat blocker codes",
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
                extra = "\"future_top_level\": {\"ignored\": true, \"ratio\": 1.25},",
                activeVerifier = activeTransferVerifier(
                    extra = "\"future_verifier_field\": [1, 2, 3],",
                    idExtra = "\"future_id_field\": true,",
                ),
                ready = "false",
                blockers =
                    "[{\"code\":\"2fa_required\",\"message\":\"no\",\"future_detail\":7}]",
            ),
        )

        assertEquals("2fa_required", readiness.blockers.single().code)

        val unsupported = readinessFromBody(
            canonicalReadinessBody(
                assetScale = "29",
                ready = "false",
                blockers =
                    "[{\"code\":\"asset_scale_unsupported\",\"message\":\"unsupported\"}]",
            ),
        )
        assertEquals(29L, unsupported.assetScale)
    }

    @Test
    fun readinessTextUsesBoundedWellFormedUnicodeScalars() {
        val boundary = "x".repeat(1023) + "😀"
        assertEquals(1024, OfflineReadinessBlocker("blocked", boundary).message.codePointCount(0, boundary.length))
        assertFailsWith<IllegalArgumentException> {
            OfflineReadinessBlocker("blocked", "x".repeat(1024) + "😀")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineReadinessBlocker("blocked", "line\u0001break")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineVerifierId("\uD800", "transfer")
        }
    }

    @Test
    fun readinessRejectsMissingDuplicateAndMalformedVerifierMembers() {
        val canonical = canonicalReadinessBody()
        val cases = listOf(
            canonical.replace("  \"asset_scale\": 9,\n", "") to
                "root.asset_scale is required",
            canonical.replace(
                "\"active_transfer_verifier\": {",
                "\"future_transfer_verifier\": {",
            ) to "root.active_transfer_verifier is required",
            canonical.replace(
                "  \"asset_scale\": 9,",
                "  \"asset_scale\": 9,\n  \"asset_scale\": 9,",
            ) to "Duplicate JSON object key: asset_scale",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(
                    extra = "\"version\": 7,",
                ),
            ) to "Duplicate JSON object key: version",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(version = "4294967296"),
            ) to "active_transfer_verifier.version must fit in an unsigned 32-bit integer",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(commitment = "\"${"AA".repeat(32)}\""),
            ) to "active_transfer_verifier.commitment must be exact lowercase 32-byte hexadecimal",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(schemaHash = "\"${"55".repeat(31)}\""),
            ) to
                "active_transfer_verifier.public_inputs_schema_hash must be exact lowercase 32-byte hexadecimal",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(activationHeight = "7", withdrawalHeight = "7"),
            ) to "withdrawalHeight must be greater than activationHeight",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(backend = "\" halo2/ipa\""),
            ) to "active_transfer_verifier.id.backend must be an exact non-empty string",
            canonicalReadinessBody(
                activeVerifier = activeTransferVerifier(circuitId = "\"confidential\\ntransfer\""),
            ) to "circuitId must be exact non-empty text",
            canonicalReadinessBody(
                ready = "false",
                blockers = "[{\"code\":\"blocked\",\"message\":\"no\\ncontrol\"}]",
            ) to "message must be exact non-empty text",
            canonicalReadinessBody(
                ready = "false",
                blockers = "[{\"code\":\"blocked\",\"message\":\"${"x".repeat(1025)}\"}]",
            ) to "message must not exceed 1024 Unicode characters",
        )

        for ((body, message) in cases) {
            val error = assertFailsWith<CompletionException> { readinessFromBody(body) }
            assertTrue(error.cause is OfflineToriiException)
            assertTrue(error.cause?.cause?.message?.contains(message) == true, error.cause?.cause?.message)
        }
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
    fun readinessRejectsMalformedSelectorsBeforeTransport() {
        val executor = CapturingExecutor(canonicalReadinessBody())
        val client = OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

        for (selector in listOf(
            "",
            "different-asset",
            "XOR#wonderland",
            " xor#wonderland",
            "xor##wonderland",
            "xor..coin#wonderland",
            "xor#wonder_land",
        )) {
            assertFailsWith<IllegalArgumentException> {
                client.getOfflineReadiness(selector)
            }
        }
        assertEquals(0, executor.calls)
    }

    @Test
    fun readinessRejectsMalformedUtf8WithoutReplacement() {
        val payload = canonicalReadinessBody().toByteArray(StandardCharsets.UTF_8)
        val marker = CANONICAL_ASSET_DEFINITION_ID.toByteArray(StandardCharsets.US_ASCII)
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

    @Test
    fun readinessAcceptsOnlyJsonWhitespaceAroundTheDocument() {
        val canonical = canonicalReadinessBody()
        val accepted = readinessFromBody("\t\n$canonical\r ")
        assertEquals(CANONICAL_ASSET_DEFINITION_ID, accepted.assetDefinitionId)

        val error = assertFailsWith<CompletionException> {
            readinessFromBody("\u0000$canonical")
        }
        assertTrue(error.cause is OfflineToriiException)
    }

    private fun readinessFromBody(responseBody: String) = OfflineToriiClient.builder()
        .executor(CapturingExecutor(responseBody))
        .baseUri(URI.create("https://example.com"))
        .build()
        .getOfflineReadiness("xor#wonderland")
        .join()

    private fun canonicalReadinessBody(
        extra: String = "",
        asset: String = "\"$CANONICAL_ASSET_DEFINITION_ID\"",
        assetScale: String = "9",
        height: String = "7",
        blockHash: String = "\"${"ab".repeat(32)}\"",
        activeVerifier: String = activeTransferVerifier(),
        ready: String = "true",
        blockers: String = "[]",
    ): String = """
        {
          $extra
          "asset_definition_id": $asset,
          "asset_scale": $assetScale,
          "evaluated_block_height": $height,
          "evaluated_block_hash": $blockHash,
          "active_transfer_verifier": $activeVerifier,
          "ready": $ready,
          "blockers": $blockers
        }
    """.trimIndent()

    private fun activeTransferVerifier(
        extra: String = "",
        idExtra: String = "",
        backend: String = "\"halo2/ipa\"",
        name: String = "\"offline-transfer\"",
        version: String = "7",
        circuitId: String = "\"confidential-transfer-v2\"",
        commitment: String = "\"${"44".repeat(32)}\"",
        schemaHash: String = "\"${"55".repeat(32)}\"",
        maxProofBytes: String = "4096",
        activationHeight: String = "1",
        withdrawalHeight: String = "null",
    ): String = """
        {
          $extra
          "id": {$idExtra "backend": $backend, "name": $name},
          "version": $version,
          "circuit_id": $circuitId,
          "commitment": $commitment,
          "public_inputs_schema_hash": $schemaHash,
          "max_proof_bytes": $maxProofBytes,
          "activation_height": $activationHeight,
          "withdrawal_height": $withdrawalHeight
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
        var calls: Int = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            calls++
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

    private companion object {
        const val CANONICAL_ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
        const val OTHER_CANONICAL_ASSET_DEFINITION_ID = "61CtjvNd9T3THAR65GsMVHr82Bjc"
    }
}
