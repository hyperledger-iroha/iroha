package org.hyperledger.iroha.sdk.client.stream

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class ToriiEventStreamClientTest {
    @Test
    fun rejectsCaseVariantAndRepeatedLastEventIdBeforeCanonicalDispatch() {
        var dispatches = 0
        val transport = object : TransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                dispatches++
                return okSse()
            }
        }

        for (path in listOf("/v1/events/sse", "/v1/contracts/events/sse?kind=applied")) {
            val client = ToriiEventStreamClient(
                baseUri = URI.create("http://example.com/base"),
                transport = transport,
                defaultHeaders = linkedMapOf("Last-Event-ID" to "first"),
            )
            val options = ToriiEventStreamOptions.builder()
                .headers(linkedMapOf("lAsT-eVeNt-Id" to "second", "last-event-id" to "third"))
                .build()

            val error = assertFailsWith<IllegalArgumentException> {
                client.openSseStream(path, options, noopListener())
            }
            assertContains(error.message.orEmpty(), "no replay log")
        }
        assertEquals(0, dispatches, "resume headers must be rejected before HTTP dispatch")
    }

    @Test
    fun preservesLastEventIdForReplayCapableCustomStreams() {
        var recorded: TransportRequest? = null
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return okSse()
                }
            },
        )
        val options = ToriiEventStreamOptions.builder()
            .putHeader("Last-Event-ID", "registry-42")
            .build()

        client.openSseStream(
            "/v1/sorafs/reputation/events/stream",
            options,
            noopListener(),
        ).completion().get(1, TimeUnit.SECONDS)

        val request = assertNotNull(recorded)
        assertEquals(listOf("registry-42"), request.headers["Last-Event-ID"])
    }

    @Test
    fun insertsOptionQueryBeforeUriFragment() {
        var recorded: TransportRequest? = null
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return okSse()
                }
            },
        )
        val options = ToriiEventStreamOptions.builder()
            .putQueryParameter("kind", "blocks")
            .build()

        for ((path, expectedQuery) in listOf(
            "/v1/events/sse#client-state" to "kind=blocks",
            "/v1/events/sse?existing=1#client-state" to "existing=1&kind=blocks",
        )) {
            recorded = null
            client.openSseStream(path, options, noopListener())
                .completion()
                .get(1, TimeUnit.SECONDS)

            val request = assertNotNull(recorded)
            assertEquals(expectedQuery, request.uri.rawQuery)
            assertEquals("client-state", request.uri.rawFragment)
        }
    }

    @Test
    fun exposesTerminalStreamErrorAsStrictTypedEvent() {
        val events = ArrayList<ServerSentEvent>()
        val body = """
            event: stream_error
            data: {"code":"stream_lagged","message":"receiver lagged","dropped_messages":18446744073709551615,"replay_available":false}

        """.trimIndent()
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                    CompletableFuture.completedFuture(
                        TransportResponse.builder()
                            .setStatusCode(200)
                            .setBody(body.toByteArray(StandardCharsets.UTF_8))
                            .build(),
                    )
            },
        )

        client.openSseStream(
            "/v1/events/sse",
            ToriiEventStreamOptions.defaultOptions(),
            object : ToriiEventStreamListener {
                override fun onEvent(event: ServerSentEvent) {
                    events.add(event)
                }
            },
        ).completion().get(1, TimeUnit.SECONDS)

        assertEquals(1, events.size)
        val terminal = assertNotNull(events.single().terminalStreamError())
        assertEquals("stream_lagged", terminal.code)
        assertEquals("receiver lagged", terminal.serverMessage)
        assertEquals(java.math.BigInteger.ONE.shiftLeft(64).subtract(java.math.BigInteger.ONE), terminal.droppedMessages)
        assertFalse(terminal.replayAvailable)
        assertEquals(events.single().data, terminal.rawData)
        assertNull(ServerSentEvent("message", "{}", null).terminalStreamError())
    }

    @Test
    fun listenerProjectionPropagatesUnwrappedTypedTerminalFailure() {
        val body = """
            event: stream_error
            data: {"code":"stream_source_closed","message":"source closed","dropped_messages":null,"replay_available":false}

        """.trimIndent()
        val observedError = AtomicReference<Throwable?>()
        val errorLatch = CountDownLatch(1)
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                    CompletableFuture.completedFuture(
                        TransportResponse.builder()
                            .setStatusCode(200)
                            .setBody(body.toByteArray(StandardCharsets.UTF_8))
                            .build(),
                    )
            },
        )
        val stream = client.openSseStream(
            "/v1/events/sse",
            ToriiEventStreamOptions.defaultOptions(),
            object : ToriiEventStreamListener {
                override fun onEvent(event: ServerSentEvent) {
                    event.terminalStreamError()?.let { throw it }
                }

                override fun onError(error: Throwable) {
                    observedError.set(error)
                    errorLatch.countDown()
                }
            },
        )

        val completionError = assertFailsWith<ExecutionException> {
            stream.completion().get(1, TimeUnit.SECONDS)
        }
        assertIs<ToriiStreamException>(completionError.cause)
        assertTrue(errorLatch.await(1, TimeUnit.SECONDS), "listener did not receive terminal failure")
        val listenerError = assertIs<ToriiStreamException>(observedError.get())
        assertEquals("stream_source_closed", listenerError.code)
    }

    @Test
    fun malformedTerminalStreamErrorsFailClosed() {
        val malformedPayloads = listOf(
            "not-json",
            "[]",
            "{}",
            """{"code":"stream_lagged","code":"other","message":"lagged","dropped_messages":1,"replay_available":false}""",
            """{"code":"stream_lagged","message":"lagged","dropped_messages":1,"replay_available":false,"extra":true}""",
            """{"code":" stream_lagged","message":"lagged","dropped_messages":1,"replay_available":false}""",
            """{"code":"stream lagged","message":"lagged","dropped_messages":1,"replay_available":false}""",
            """{"code":"stream_lagged","message":" lagged","dropped_messages":1,"replay_available":false}""",
            "{\"code\":\"stream_lagged\",\"message\":\"\\uD800\",\"dropped_messages\":1,\"replay_available\":false}",
            """{"code":"stream_lagged","message":"lagged","dropped_messages":-1,"replay_available":false}""",
            """{"code":"stream_lagged","message":"lagged","dropped_messages":1.0,"replay_available":false}""",
            """{"code":"stream_lagged","message":"lagged","dropped_messages":18446744073709551616,"replay_available":false}""",
            """{"code":"stream_lagged","message":"lagged","dropped_messages":1,"replay_available":null}""",
        )

        for (payload in malformedPayloads) {
            val event = ServerSentEvent("stream_error", payload, null)
            val error = assertFailsWith<ToriiStreamProtocolException> {
                event.terminalStreamError()
            }
            assertEquals(payload, error.rawData)
            assertTrue(error.reason.isNotEmpty())
        }
    }

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
