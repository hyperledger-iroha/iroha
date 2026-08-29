// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.KeyPairGenerator
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.testing.TestNetworkIds

class AtomicPrivateSettlementToriiClientV1Test {
    private val fixture = loadFixture()
    private val identifiers = fixture.objectField("identifiers")
    private val bundle = AtomicPrivateSettlementIdentifierV1.parse(
        identifiers.stringField("bundle_hex"),
    )
    private val payload = AtomicPrivateSettlementIdentifierV1.parse(
        identifiers.stringField("payload_hex"),
    )

    @Test
    fun sharedNoritoJsonFixturePinsEveryPreparedRouteAndTopLevelShape() {
        val routes = fixture.listField("request_routes")
        assertEquals(AtomicPrivateSettlementOperationV1.entries.size, routes.size)
        routes.forEach { entryValue ->
            val entry = entryValue.asObject("request route")
            val operation = AtomicPrivateSettlementOperationV1.valueOf(
                entry.stringField("operation"),
            )
            assertEquals(operation.path, entry.stringField("path"))
            assertEquals(operation.auth.name, entry.stringField("auth"))
            val fields = entry.listField("top_level_fields").map {
                require(it is String)
                it
            }.toSet()
            val body = LinkedHashMap<String, Any?>()
            fields.forEach { body[it] = emptyMap<String, Any?>() }
            val prepared = AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
                operation,
                JsonEncoder.encode(body).toByteArray(StandardCharsets.UTF_8),
            )
            assertEquals(operation, prepared.operation)
            assertTrue(prepared.toString().contains("[REDACTED]"))
            prepared.close()
            assertFailsWith<IllegalStateException> { prepared.bytes() }
        }
    }

    @Test
    fun sponsorLegStatusIsNetworkBoundOneShotAndIdentityChecked() {
        val response = fixture.objectField("responses").objectField("leg_status")
        val executor = CapturingSettlementExecutor(jsonResponse(response))
        val client = client(executor)
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_700_000_000_000L,
            "settlement-leg-status-1",
        )

        val received = client.getLegStatus(payload, auth).join()

        assertEquals(
            "/api/v1/nexus/private-settlements/legs/${payload.pathComponent()}/status",
            executor.request.uri.path,
        )
        assertEquals("GET", executor.request.method)
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.request.replayPolicy)
        assertTrue(executor.request.headers.containsKey(CanonicalRequestSigner.HEADER_SIGNATURE))
        assertFalse(executor.request.headers.containsKey(OperatorRequestSigner.HEADER_SIGNATURE))
        assertTrue(received.toString().contains("[REDACTED]"))
        assertFalse(received.toString().contains(identifiers.stringField("payload_json")))
    }

    @Test
    fun auditorApprovalUsesPurposeSeparatedRoleHeadersAndExactPayloadPath() {
        val response = fixture.objectField("responses").objectField("audit_approval")
        val executor = CapturingSettlementExecutor(jsonResponse(response))
        val client = client(executor)
        val request = AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
            """{"approval":{}}""".toByteArray(StandardCharsets.UTF_8),
        )
        val roleContext = OperatorSigningContext(
            TestNetworkIds.canonical(),
            "ed0120${"11".repeat(32)}",
        ) { message -> ByteArray(64) { index -> (message.size + index + 1).toByte() } }

        client.submitAuditApproval(payload, request, roleContext).join()

        assertEquals(
            "/api/v1/nexus/private-settlements/legs/${payload.pathComponent()}/audit-approvals",
            executor.request.uri.path,
        )
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.request.replayPolicy)
        assertTrue(executor.request.headers.containsKey(OperatorRequestSigner.HEADER_SIGNATURE))
        assertFalse(executor.request.headers.containsKey(CanonicalRequestSigner.HEADER_SIGNATURE))
    }

    @Test
    fun publicBundleQueriesRemainUnsignedBoundedAndValidateReceiptIdentity() {
        val responses = fixture.objectField("responses")
        val statusExecutor = CapturingSettlementExecutor(
            jsonResponse(responses.objectField("bundle_status_aborted")),
        )
        client(statusExecutor).getBundleStatus(bundle).join()
        assertEquals(RequestReplayPolicy.RETRY_SAFE, statusExecutor.request.replayPolicy)
        assertTrue(statusExecutor.request.headers.keys.none { it.startsWith("X-Iroha", true) })

        val receiptExecutor = CapturingSettlementExecutor(
            jsonResponse(responses.objectField("receipt_pending")),
        )
        client(receiptExecutor).getBundleReceipt(bundle).join()
        assertEquals(
            "/api/v1/nexus/private-settlements/bundles/${bundle.pathComponent()}/receipt",
            receiptExecutor.request.uri.path,
        )

        val substituted = LinkedHashMap(responses.objectField("receipt_pending"))
        val value = LinkedHashMap(substituted.objectField("value"))
        value["bundle_id"] = identifiers.stringField("payload_json")
        substituted["value"] = value
        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            client(CapturingSettlementExecutor(jsonResponse(substituted)))
                .getBundleReceipt(bundle)
                .join()
        }
        assertTrue(error.cause is AtomicPrivateSettlementToriiExceptionV1)
    }

    @Test
    fun operationSubstitutionMalformedJsonAndSecretErrorBodiesFailClosed() {
        assertFailsWith<IllegalArgumentException> {
            AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
                AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
                """{"approval":{},"approval":{}}""".toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
                AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
                byteArrayOf(0x7b, 0x22, 0xc3.toByte(), 0x28, 0x22, 0x7d),
            )
        }

        val executor = CapturingSettlementExecutor(
            TransportResponse.builder()
                .setStatusCode(400)
                .setBody("memo=LEAK_CANARY amount=987654".toByteArray(StandardCharsets.UTF_8))
                .addHeader("Content-Type", "text/plain")
                .build(),
        )
        val client = client(executor)
        val wrong = AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
            """{"transaction":{}}""".toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private)
        assertFailsWith<IllegalArgumentException> { client.uploadLeg(wrong, auth) }

        val approval = AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
            """{"approval":{}}""".toByteArray(StandardCharsets.UTF_8),
        )
        val roleContext = OperatorSigningContext(
            TestNetworkIds.canonical(),
            "ed0120${"22".repeat(32)}",
        ) { ByteArray(64) { 7 } }
        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            client.submitAuditApproval(payload, approval, roleContext).join()
        }
        val message = error.cause?.message.orEmpty()
        assertFalse(message.contains("LEAK_CANARY"))
        assertFalse(message.contains("987654"))

        val redirectedResponse = TransportResponse(
            200,
            JsonEncoder.encode(fixture.objectField("responses").objectField("bundle_status_aborted"))
                .toByteArray(StandardCharsets.UTF_8),
            "",
            mapOf("Content-Type" to listOf("application/json")),
            URI.create("https://collector.invalid/status"),
            true,
        )
        assertFailsWith<java.util.concurrent.CompletionException> {
            client(CapturingSettlementExecutor(redirectedResponse)).getBundleStatus(bundle).join()
        }
    }

    private fun client(executor: HttpTransportExecutor): AtomicPrivateSettlementToriiClientV1 =
        AtomicPrivateSettlementToriiClientV1.builder()
            .executor(executor)
            .baseUri(URI.create("https://torii.example/api"))
            .localSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
            .build()

    private fun jsonResponse(value: Map<String, Any?>): TransportResponse =
        TransportResponse.builder()
            .setStatusCode(200)
            .setBody(JsonEncoder.encode(value).toByteArray(StandardCharsets.UTF_8))
            .addHeader("Content-Type", "application/json")
            .build()

    companion object {
        @Suppress("UNCHECKED_CAST")
        private fun loadFixture(): Map<String, Any?> {
            var current: Path? = Paths.get("").toAbsolutePath()
            while (current != null) {
                val candidate = current.resolve(FIXTURE_PATH)
                if (Files.isRegularFile(candidate)) {
                    val parsed = JsonParser.parse(
                        String(Files.readAllBytes(candidate), StandardCharsets.UTF_8),
                    )
                    require(parsed is Map<*, *>) { "settlement fixture must be a JSON object" }
                    return parsed as Map<String, Any?>
                }
                current = current.parent
            }
            error("$FIXTURE_PATH was not found")
        }

        private const val FIXTURE_PATH =
            "fixtures/norito_rpc/atomic_private_settlement_sdk_v1.json"
    }
}

private class CapturingSettlementExecutor(
    private val response: TransportResponse,
) : HttpTransportExecutor {
    lateinit var request: TransportRequest

    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
        check(!this::request.isInitialized) { "settlement requests must be dispatched exactly once" }
        this.request = request
        return CompletableFuture.completedFuture(
            TransportResponse(
                response.statusCode,
                response.body,
                response.message,
                response.headers,
                response.finalUri ?: request.uri,
                response.redirected,
            ),
        )
    }
}

@Suppress("UNCHECKED_CAST")
private fun Any?.asObject(label: String): Map<String, Any?> {
    require(this is Map<*, *>) { "$label must be an object" }
    return this as Map<String, Any?>
}

private fun Map<String, Any?>.objectField(name: String): Map<String, Any?> =
    this[name].asObject(name)

private fun Map<String, Any?>.listField(name: String): List<Any?> {
    val value = this[name]
    require(value is List<*>) { "$name must be an array" }
    return value
}

private fun Map<String, Any?>.stringField(name: String): String {
    val value = this[name]
    require(value is String) { "$name must be a string" }
    return value
}
