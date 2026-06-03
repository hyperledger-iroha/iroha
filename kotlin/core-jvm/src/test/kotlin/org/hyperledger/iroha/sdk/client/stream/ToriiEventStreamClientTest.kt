package org.hyperledger.iroha.sdk.client.stream

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class ToriiEventStreamClientTest {
    @Test
    fun rejectsUnsupportedProductionBackendFiltersBeforeRequest() {
        var requestSent = false
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    requestSent = true
                    return okSse()
                }
            },
        )

        for (filter in listOf(
            """{"VerifyingKey":{"id_matcher":{"backend":"halo2/ipa/orchard","name":"vk"},"event_set":{"Registered":true}}}""",
            """{"VerifyingKey":{"id_matcher":{"backend":" halo2/ipa","name":"vk"},"event_set":{"Registered":true}}}""",
            """{"Proof":{"id_matcher":{"backend":"mock/dev","hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"event_set":{"Verified":true}}}""",
            """{"Proof":{"id_matcher":{"backend":"groth16/bls12-377","hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"event_set":{"Verified":true}}}""",
        )) {
            assertFailsWith<IllegalArgumentException> {
                client.openSseStream("/v1/events/sse", eventFilterOptions(filter), noopListener())
            }
            assertFalse(requestSent)
        }
    }

    @Test
    fun rejectsMalformedVerifyingKeyEventNamesBeforeRequest() {
        var requestSent = false
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    requestSent = true
                    return okSse()
                }
            },
        )

        for (nameJson in listOf(
            "\"\"",
            "\"   \"",
            "\"\\t\"",
            "\"\\n\"",
            "\"vk:main\"",
            "42",
        )) {
            val filter = "{\"VerifyingKey\":{\"id_matcher\":{\"backend\":\"halo2/ipa\",\"name\":$nameJson},\"event_set\":{\"Registered\":true}}}"
            val error = assertFailsWith<IllegalArgumentException> {
                client.openSseStream("/v1/events/sse", eventFilterOptions(filter), noopListener())
            }
            val message = error.message.orEmpty()
            assertTrue(
                message.contains("non-empty string") ||
                    message.contains("must be a string") ||
                    message.contains("must not contain ':'"),
                "unexpected name rejection message: $message",
            )
            assertFalse(requestSent)
        }
    }

    @Test
    fun rejectsMalformedProofEventHashesBeforeRequest() {
        var requestSent = false
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    requestSent = true
                    return okSse()
                }
            },
        )

        for (hashJson in listOf(
            "\"\"",
            "\"abc\"",
            "\"" + "z".repeat(64) + "\"",
            "\"0x0x" + "a".repeat(64) + "\"",
            "42",
        )) {
            val filter = "{\"Proof\":{\"id_matcher\":{\"backend\":\"halo2/ipa\",\"hash_hex\":$hashJson},\"event_set\":{\"Verified\":true}}}"
            assertFailsWith<IllegalArgumentException> {
                client.openSseStream("/v1/events/sse", eventFilterOptions(filter), noopListener())
            }
            assertFalse(requestSent)
        }
    }

    @Test
    fun rejectsPathQueryEventFiltersBeforeRequest() {
        var requestSent = false
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    requestSent = true
                    return okSse()
                }
            },
        )
        val filter =
            """{"Proof":{"id_matcher":{"backend":"halo2/ipa/orchard","hash_hex":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"event_set":{"Verified":true}}}"""
        val path = "/v1/events/sse?filter=${URLEncoder.encode(filter, "UTF-8")}"

        assertFailsWith<IllegalArgumentException> {
            client.openSseStream(path, ToriiEventStreamOptions.defaultOptions(), noopListener())
        }
        assertFalse(requestSent)
    }

    @Test
    fun canonicalizesVerifyingKeyNameFiltersBeforeRequest() {
        var recorded: TransportRequest? = null
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return okSse()
                }
            },
        )
        val filter =
            """{"VerifyingKey":{"id_matcher":{"backend":"halo2/ipa","name":" vk_main "},"event_set":{"Registered":true}}}"""

        client.openSseStream("/v1/events/sse", eventFilterOptions(filter), noopListener())
            .completion()
            .get(1, TimeUnit.SECONDS)

        val request = assertNotNull(recorded)
        val normalizedFilter = queryParameter(request.uri.rawQuery, "filter")
        assertContains(normalizedFilter, "\"name\":\"vk_main\"")
    }

    @Test
    fun canonicalizesProofHashFiltersBeforeRequest() {
        var recorded: TransportRequest? = null
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return okSse()
                }
            },
        )
        val hash = "A".repeat(64)
        val proofHash = "B".repeat(64)
        val filter =
            "{\"Proof\":{\"id_matcher\":{\"backend\":\"halo2/ipa\",\"hash_hex\":\"0x$hash\",\"proof_hash_hex\":\"$proofHash\"},\"event_set\":{\"Verified\":true}}}"

        client.openSseStream("/v1/events/sse", eventFilterOptions(filter), noopListener())
            .completion()
            .get(1, TimeUnit.SECONDS)

        val request = assertNotNull(recorded)
        val normalizedFilter = queryParameter(request.uri.rawQuery, "filter")
        assertContains(normalizedFilter, "\"hash_hex\":\"${"a".repeat(64)}\"")
        assertContains(normalizedFilter, "\"proof_hash_hex\":\"${"b".repeat(64)}\"")
    }

    private fun eventFilterOptions(filter: String): ToriiEventStreamOptions =
        ToriiEventStreamOptions.builder()
            .putQueryParameter("filter", filter)
            .build()

    private fun noopListener(): ToriiEventStreamListener =
        object : ToriiEventStreamListener {
            override fun onEvent(event: ServerSentEvent) = Unit
        }

    private fun okSse(): CompletableFuture<TransportResponse> =
        CompletableFuture.completedFuture(
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(": keepalive\n\n".toByteArray(StandardCharsets.UTF_8))
                .build(),
        )

    private fun queryParameter(rawQuery: String, name: String): String {
        for (segment in rawQuery.split('&')) {
            val equals = segment.indexOf('=')
            val rawName = if (equals >= 0) segment.substring(0, equals) else segment
            if (URLDecoder.decode(rawName, "UTF-8") != name) continue
            val rawValue = if (equals >= 0) segment.substring(equals + 1) else ""
            return URLDecoder.decode(rawValue, "UTF-8")
        }
        error("missing query parameter $name")
    }
}
