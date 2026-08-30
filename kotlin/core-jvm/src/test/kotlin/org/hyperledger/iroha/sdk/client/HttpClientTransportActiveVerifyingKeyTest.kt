package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.zk.VerifyingKeyBackendTag

class HttpClientTransportActiveVerifyingKeyTest {
    @Test
    fun activeProjectionIsFixedBoundedOrderedAndBearerFree() {
        val payload =
            """
                [
                  {
                    "backend": "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
                    "name": "kagemusha_topup_v3"
                  },
                  {"backend": "halo2/ipa", "name": "same_name"},
                  {"backend": "stark/fri", "name": "same_name"},
                  {"backend": "stark/fri", "name": "stark_transfer_v1"}
                ]
            """.trimIndent().toByteArray(StandardCharsets.UTF_8)
        val executor = ExactJsonResponseExecutor(payload)
        val client = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .putDefaultHeader("authorization", "Bearer must-not-leak")
                .putDefaultHeader("proxy-authorization", "proxy-must-not-leak")
                .putDefaultHeader("COOKIE", "session-must-not-leak")
                .putDefaultHeader("X-API-Token", "api-token-must-not-leak")
                .putDefaultHeader("x-account-id", "account-id-must-not-leak")
                .putDefaultHeader("X-Dataspace-Id", "dataspace-id-must-not-leak")
                .putDefaultHeader("x-iroha-account", "account-must-not-leak")
                .putDefaultHeader("X-Iroha-Signature", "signature-must-not-leak")
                .putDefaultHeader("X-Iroha-Timestamp-Ms", "1")
                .putDefaultHeader("X-Iroha-Nonce", "nonce-must-not-leak")
                .putDefaultHeader("X-Iroha-Witness", "witness-must-not-leak")
                .putDefaultHeader("x-iroha-onboarding-token", "onboarding-must-not-leak")
                .putDefaultHeader("x-iroha-operator-signature", "operator-must-not-leak")
                .putDefaultHeader("X-Trace-Id", "trace-123")
                .build(),
        )

        val ids = client.listActiveVerifyingKeyIds().join()

        assertEquals(4, ids.size)
        assertEquals(
            "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            ids[0].backend,
        )
        assertEquals("kagemusha_topup_v3", ids[0].name)
        assertEquals(VerifyingKeyBackendTag.HALO2_IPA_PASTA, ids[0].engine())
        assertEquals("halo2/ipa", ids[1].backend)
        assertEquals("stark/fri", ids[2].backend)
        assertEquals("same_name", ids[1].name)
        assertEquals("same_name", ids[2].name)

        val request = executor.lastRequest
        assertEquals("GET", request.method)
        assertEquals("/api/v1/zk/vk", request.uri.rawPath)
        assertEquals(
            "status=Active&ids_only=true&limit=1000&order=asc",
            request.uri.rawQuery,
        )
        assertEquals(listOf("application/json"), request.headers["Accept"])
        assertEquals(listOf("identity"), request.headers["Accept-Encoding"])
        assertEquals(512L * 1024L, request.maximumResponseBytes)
        assertFalse(request.allowAmbientCredentials)
        assertEquals(listOf("trace-123"), request.headers["X-Trace-Id"])
        val credentialHeaders = setOf(
            "Authorization",
            "Proxy-Authorization",
            "Cookie",
            "X-API-Token",
            "X-Account-Id",
            "X-Dataspace-Id",
            "X-Iroha-Account",
            "X-Iroha-Signature",
            "X-Iroha-Timestamp-Ms",
            "X-Iroha-Nonce",
            "X-Iroha-Witness",
            "X-Iroha-Onboarding-Token",
        )
        assertFalse(
            request.headers.keys.any { candidate ->
                credentialHeaders.any { it.equals(candidate, ignoreCase = true) } ||
                    candidate.startsWith("X-Iroha-Operator-", ignoreCase = true)
            },
        )
    }

    @Test
    fun activeProjectionRejectsUriUserInfoBeforeExecutorDispatch() {
        var dispatches = 0
        val executor = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                dispatches += 1
                return CompletableFuture.completedFuture(
                    TransportResponse(200, ByteArray(0), "", emptyMap()),
                )
            }
        }
        val client = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://ambient:must-not-leak@torii.example"))
                .build(),
        )

        val failure = assertFailsWith<IllegalArgumentException> {
            client.listActiveVerifyingKeyIds()
        }

        assertTrue(failure.message?.contains("reject URI user-info") == true)
        assertEquals(0, dispatches)
    }

    @Test
    fun activeProjectionRejectsAcceptEncodingOverrideBeforeExecutorDispatch() {
        var dispatches = 0
        val executor = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                dispatches += 1
                return CompletableFuture.completedFuture(
                    TransportResponse(200, ByteArray(0), "", emptyMap()),
                )
            }
        }
        val client = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .putDefaultHeader("aCcEpT-eNcOdInG", "gzip")
                .build(),
        )

        val failure = assertFailsWith<IllegalArgumentException> {
            client.listActiveVerifyingKeyIds()
        }

        assertTrue(failure.message?.contains("Accept-Encoding") == true)
        assertEquals(0, dispatches)
    }

    @Test
    fun activeProjectionRejectsHostilePayloads() {
        val hostilePayloads = mutableListOf(
            """[{"backend":"halo2/ipa","name":"vk","status":"Active"}]""",
            """[{"backend":"unsupported","name":"vk"}]""",
            """[{"backend":"HALO2/IPA","name":"vk"}]""",
            """[{"backend":"halo2//ipa","name":"vk"}]""",
            """[{"backend":"halø2/ipa","name":"vk"}]""",
            """[{"backend":"${"a".repeat(257)}","name":"vk"}]""",
            """[{"backend":"halo2/ipa","name":""}]""",
            """[{"backend":"halo2/ipa","name":" vk"}]""",
            """[{"backend":"halo2/ipa","name":"vk "}]""",
            """[{"backend":"halo2/ipa","name":"Vk"}]""",
            """[{"backend":"halo2/ipa","name":"vé"}]""",
            """[{"backend":"halo2/ipa","name":"a..b"}]""",
            """[{"backend":"halo2/ipa","name":"a//b"}]""",
            """[{"backend":"halo2/ipa","name":"a:::b"}]""",
            """[{"backend":"halo2/ipa","name":"a/:b"}]""",
            """[{"backend":"halo2/ipa","name":"a:/b"}]""",
            """[{"backend":"halo2/ipa","name":"a/.b"}]""",
            """[{"backend":"halo2/ipa","name":"a./b"}]""",
            """[{"backend":"halo2/ipa","name":"a:.b"}]""",
            """[{"backend":"halo2/ipa","name":"a.:b"}]""",
            """[{"backend":"halo2/ipa","name":"-vk"}]""",
            """[{"backend":"halo2/ipa","name":"vk/"}]""",
            """[{"backend":"halo2/ipa","name":"${"a".repeat(257)}"}]""",
            """[{"backend":"halo2/ipa","backend":"stark/fri","name":"vk"}]""",
            """[{"backend":"halo2/ipa","name":"vk"},{"backend":"halo2/ipa","name":"vk"}]""",
            """[{"backend":"stark/fri","name":"z"},{"backend":"halo2/ipa","name":"a"}]""",
            """[{"backend":"stark/fri","name":"same_name"},{"backend":"halo2/ipa","name":"same_name"}]""",
            """{"items":[{"backend":"halo2/ipa","name":"vk"}]}""",
            """[{"backend":"halo2/ipa"}]""",
            """[{"backend":"halo2/ipa","name":1}]""",
            """["halo2/ipa:vk"]""",
        )
        val tooMany = (0..1_000).joinToString(",") { index ->
            "{\"backend\":\"halo2/ipa\",\"name\":\"vk_${index.toString().padStart(4, '0')}\"}"
        }
        hostilePayloads.add("[$tooMany]")

        hostilePayloads.forEach { payload ->
            assertFailsWith<IllegalStateException>(payload) {
                VerifyingKeyJsonParser.parseActiveIds(payload.toByteArray(StandardCharsets.UTF_8))
            }
        }

        val malformedUtf8 =
            """[{"backend":"halo2/ipa","name":"vk_""".toByteArray(StandardCharsets.UTF_8) +
                byteArrayOf(0xc3.toByte(), 0x28) +
                """"}]""".toByteArray(StandardCharsets.UTF_8)
        assertFailsWith<IllegalStateException> {
            VerifyingKeyJsonParser.parseActiveIds(malformedUtf8)
        }

        val maximumName = "a".repeat(256)
        val maximumNamePayload =
            """[{"backend":"halo2/ipa","name":"$maximumName"}]"""
                .toByteArray(StandardCharsets.UTF_8)
        assertEquals(
            maximumName,
            VerifyingKeyJsonParser.parseActiveIds(maximumNamePayload).single().name,
        )
        val portableColonName = "halo2/ipa::transfer_v1"
        val portableColonPayload =
            """[{"backend":"halo2/ipa","name":"$portableColonName"}]"""
                .toByteArray(StandardCharsets.UTF_8)
        assertEquals(
            portableColonName,
            VerifyingKeyJsonParser.parseActiveIds(portableColonPayload).single().name,
        )
        val portableSeparatorsName = "a-b_c/d:e.f"
        val portableSeparatorsPayload =
            """[{"backend":"halo2/ipa","name":"$portableSeparatorsName"}]"""
                .toByteArray(StandardCharsets.UTF_8)
        assertEquals(
            portableSeparatorsName,
            VerifyingKeyJsonParser.parseActiveIds(portableSeparatorsPayload).single().name,
        )
    }

    @Test
    fun activeProjectionRequiresExactJsonAndEnforcesBodyBound() {
        val valid = """[{"backend":"halo2/ipa","name":"vk"}]"""
            .toByteArray(StandardCharsets.UTF_8)
        val maximumSizePayload = ByteArray(512 * 1024) { ' '.code.toByte() }
        valid.copyInto(maximumSizePayload)
        val maximumSizeExecutor = ExactJsonResponseExecutor(maximumSizePayload)
        val maximumSizeClient = HttpClientTransport.withExecutor(
            maximumSizeExecutor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )
        assertEquals(1, maximumSizeClient.listActiveVerifyingKeyIds().join().size)
        assertEquals(512L * 1024L, maximumSizeExecutor.lastRequest.maximumResponseBytes)

        val hostileResponses = listOf(
            ExactJsonResponseExecutor(valid, contentTypes = listOf("application/json; charset=utf-8")),
            ExactJsonResponseExecutor(valid, contentTypes = listOf("text/json")),
            ExactJsonResponseExecutor(valid, contentTypes = emptyList()),
            ExactJsonResponseExecutor(
                valid,
                contentTypes = listOf("application/json", "application/json"),
            ),
            ExactJsonResponseExecutor(valid, statusCode = 201),
            ExactJsonResponseExecutor(valid, contentLength = (valid.size + 1).toString()),
            ExactJsonResponseExecutor(valid, contentEncoding = "gzip"),
            ExactJsonResponseExecutor(ByteArray(0)),
            ExactJsonResponseExecutor(ByteArray(512 * 1024 + 1) { ' '.code.toByte() }),
        )

        hostileResponses.forEach { executor ->
            val client = HttpClientTransport.withExecutor(
                executor,
                ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example"))
                    .build(),
            )
            assertFailsWith<CompletionException> {
                client.listActiveVerifyingKeyIds().join()
            }
            assertTrue(executor.lastRequest.maximumResponseBytes == 512L * 1024L)
        }
    }

    private class ExactJsonResponseExecutor(
        private val body: ByteArray,
        private val statusCode: Int = 200,
        private val contentTypes: List<String> = listOf("application/json"),
        private val contentLength: String? = body.size.toString(),
        private val contentEncoding: String? = null,
    ) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            val builder = TransportResponse.builder()
                .setStatusCode(statusCode)
                .setBody(body)
            contentTypes.forEach { builder.addHeader("Content-Type", it) }
            contentLength?.let { builder.addHeader("Content-Length", it) }
            contentEncoding?.let { builder.addHeader("Content-Encoding", it) }
            return CompletableFuture.completedFuture(builder.build())
        }
    }
}
