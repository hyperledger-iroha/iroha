package org.hyperledger.iroha.sdk.client.stream

import org.hyperledger.iroha.sdk.client.RequestSigner

import java.net.URI
import java.net.URLDecoder
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
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
import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.testing.TestNetworkIds

class ToriiEventStreamClientTest {
    @Test
    fun canonicalSigningBindsTheExactFinalStreamUri() {
        var recorded: TransportRequest? = null
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val networkId = TestNetworkIds.canonical()
        val timestampMs = 1_700_000_000_000L
        val nonce = "contract-stream-1"
        val client = ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("https://example.com/api"))
            .setTransportExecutor(object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return okSse()
                }
            })
            .canonicalRequestAuth(
                LocalSigningContext(networkId),
                ToriiCanonicalRequestAuth(
                    "alice@universal",
                    RequestSigner.ed25519(keyPair.private),
                    timestampMs,
                    nonce,
                ),
            )
            .build()
        val options = ToriiEventStreamOptions.builder()
            .putQueryParameter("kind", "applied")
            .putQueryParameter("cursor", "opaque cursor")
            .build()

        client.openSseStream(
            "/v1/contracts/events/sse?z=last",
            options,
            noopListener(),
        ).completion().get(1, TimeUnit.SECONDS)

        val request = assertNotNull(recorded)
        assertEquals(
            "https://example.com/api/v1/contracts/events/sse?z=last&kind=applied&cursor=opaque+cursor",
            request.uri.toString(),
        )
        assertEquals(
            listOf("alice@universal"),
            request.headers[CanonicalRequestSigner.HEADER_ACCOUNT],
        )
        assertEquals(
            listOf(timestampMs.toString()),
            request.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS],
        )
        assertEquals(listOf(nonce), request.headers[CanonicalRequestSigner.HEADER_NONCE])
        val encodedSignature = assertNotNull(
            request.headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.single(),
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(keyPair.public)
        verifier.update(
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                networkId,
                "GET",
                request.uri,
                null,
                timestampMs,
                nonce,
            ),
        )
        assertTrue(verifier.verify(Base64.getDecoder().decode(encodedSignature)))
    }

    @Test
    fun contractStreamRemainsAnonymousWithoutSigningConfiguration() {
        var recorded: TransportRequest? = null
        val client = ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("https://example.com"))
            .setTransportExecutor(object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return okSse()
                }
            })
            .build()

        client.openSseStream(
            "/v1/contracts/events/sse",
            ToriiEventStreamOptions.defaultOptions(),
            noopListener(),
        ).completion().get(1, TimeUnit.SECONDS)

        val headers = assertNotNull(recorded).headers
        assertFalse(headers.containsKey(CanonicalRequestSigner.HEADER_ACCOUNT))
        assertFalse(headers.containsKey(CanonicalRequestSigner.HEADER_SIGNATURE))
        assertFalse(headers.containsKey(CanonicalRequestSigner.HEADER_TIMESTAMP_MS))
        assertFalse(headers.containsKey(CanonicalRequestSigner.HEADER_NONCE))
    }

    @Test
    fun rejectsPrecomputedOrPartialCanonicalHeadersBeforeDispatch() {
        var dispatches = 0
        val transport = object : TransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                dispatches++
                return okSse()
            }
        }
        val defaultHeaderClient = ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("https://example.com"))
            .setTransportExecutor(transport)
            .putDefaultHeader("x-iroha-signature", "precomputed")
            .build()
        val optionHeaderClient = ToriiEventStreamClient.builder()
            .setBaseUri(URI.create("https://example.com"))
            .setTransportExecutor(transport)
            .build()

        for ((client, options) in listOf(
            defaultHeaderClient to ToriiEventStreamOptions.defaultOptions(),
            optionHeaderClient to ToriiEventStreamOptions.builder()
                .putHeader("X-IROHA-ACCOUNT", "alice@universal")
                .build(),
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                client.openSseStream("/v1/contracts/events/sse", options, noopListener())
            }
            assertContains(error.message.orEmpty(), "canonicalRequestAuth")
        }
        assertEquals(0, dispatches)
    }

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

        """.trimIndent() + "\n"
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

        """.trimIndent() + "\n"
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
    fun typedEventFiltersAreSentAsCanonicalText() {
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
        val filter = (EventFields.TX_HASH eq "ab".repeat(32)) and EventFields.TX_STATUS.isIn("Applied", "Rejected")

        client.openEventStream(filter, noopListener()).completion().get(1, TimeUnit.SECONDS)

        val request = assertNotNull(recorded)
        assertEquals("/base/v1/events/sse", request.uri.rawPath)
        assertEquals(
            "tx_hash = \"${"ab".repeat(32)}\" and tx_status in [\"Applied\", \"Rejected\"]",
            queryParameter(request.uri.rawQuery, "filter"),
        )
    }

    @Test
    fun textEventFiltersPassThroughUnchanged() {
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
        val text = "block_height >= 10   AND proof_backend = 'halo2/ipa'"
        val options = ToriiEventStreamOptions.builder().setFilter(text).build()

        client.openSseStream("/v1/events/sse", options, noopListener()).completion().get(1, TimeUnit.SECONDS)

        assertEquals(text, queryParameter(assertNotNull(recorded).uri.rawQuery, "filter"))
        assertFailsWith<IllegalArgumentException> {
            client.openSseStream(
                "/v1/events/sse",
                ToriiEventStreamOptions.builder().setFilter(text).putQueryParameter("filter", text).build(),
                noopListener(),
            )
        }
    }

    @Test
    fun rejectedFiltersSurfaceTheTypedErrorEnvelope() {
        val failure = AtomicReference<Throwable?>()
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                    CompletableFuture.completedFuture(
                        TransportResponse.builder()
                            .setStatusCode(400)
                            .setBody(
                                """{"code":"invalid_filter","message":"invalid `filter`: unexpected character `~` (column 9)","details":{"field":"filter"}}"""
                                    .toByteArray(StandardCharsets.UTF_8),
                            )
                            .build(),
                    )
            },
        )
        val stream = client.openSseStream(
            "/v1/events/sse",
            ToriiEventStreamOptions.builder().setFilter("tx_hash ~ 1").build(),
            object : ToriiEventStreamListener {
                override fun onEvent(event: ServerSentEvent) = Unit

                override fun onError(error: Throwable) {
                    failure.set(error)
                }
            },
        )
        assertFailsWith<ExecutionException> { stream.completion().get(5, TimeUnit.SECONDS) }
        val error = assertIs<org.hyperledger.iroha.sdk.client.ToriiApiException>(failure.get())
        assertEquals(400, error.status)
        assertEquals("invalid_filter", error.code)
        assertEquals("filter", error.field)
    }

    @Test
    fun subscribeDecodesTypedEventsAndToleratesUnknownOnes() {
        val payloads = listOf(
            """{"category":"Pipeline","event":"Transaction","hash":"ab","lane_id":0,"dataspace_id":7,"block_height":null,"status":"Rejected","rejection_code":"validation","rejection_reason":"Transaction validation failed."}""",
            """{"category":"Pipeline","event":"Block","status":"Rejected","rejection_code":"EmptyBlock"}""",
            """{"category":"Pipeline","event":"Warning","kind":"slow","details":"late commit","height":4}""",
            """{"category":"Pipeline","event":"Witness","block_hash":"cd","height":5,"view":1,"epoch":0,"read_count":3,"write_count":2}""",
            """{"category":"Data","event":"ProofVerified","backend":"halo2/ipa","proof_hash":"aa","call_hash":null,"envelope_hash":"bb","vk_ref":"halo2/ipa::vk","vk_commitment":null}""",
            """{"category":"Data","event":"ProofRejected","backend":"halo2/ipa","proof_hash":"aa","call_hash":"cc","envelope_hash":null,"vk_ref":null,"vk_commitment":null}""",
            """{"category":"Data","event":"ProofPruned","backend":"halo2/ipa","removed_count":1,"remaining":9,"cap":10,"grace_blocks":2,"prune_batch":4,"pruned_at_height":77,"pruned_by":"alice@wonderland","origin":"Manual","removed":[{"backend":"halo2/ipa","proof_hash":"aa"}]}""",
            """{"category":"Data","event":"Asset","summary":"AssetEvent(..)"}""",
            """{"category":"Other","event":"Time","summary":"TimeEvent(..)"}""",
            """{"category":"Pipeline","event":"Transaction","hash":"ab","lane_id":0,"dataspace_id":7,"block_height":3,"status":"Teleported"}""",
            """{"category":"Data","event":"Holograms","summary":"?"}""",
            """{"category":"Future","event":"Thing"}""",
            "not json",
        )
        val body = payloads.joinToString("") { "data: $it\n\n" } +
            "event: stream_error\ndata: {\"code\":\"stream_lagged\",\"message\":\"receiver lagged\",\"dropped_messages\":3,\"replay_available\":false}\n\n"
        var recorded: TransportRequest? = null
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                    recorded = request
                    return CompletableFuture.completedFuture(
                        TransportResponse.builder().setStatusCode(200).setBody(body.toByteArray(StandardCharsets.UTF_8)).build(),
                    )
                }
            },
        )
        val events = ArrayList<ToriiEvent>()
        val terminal = AtomicReference<ToriiStreamException?>()

        client.subscribe(
            EventFields.TX_STATUS eq TransactionEventStatus.REJECTED.wireName,
            object : ToriiEventListener {
                override fun onEvent(event: ToriiEvent) {
                    events.add(event)
                }

                override fun onStreamError(error: ToriiStreamException) {
                    terminal.set(error)
                }
            },
        ).completion().get(5, TimeUnit.SECONDS)

        assertEquals("tx_status = \"Rejected\"", queryParameter(assertNotNull(recorded).uri.rawQuery, "filter"))
        assertEquals(payloads.size, events.size)
        val transaction = assertIs<ToriiEvent.Transaction>(events[0])
        assertEquals(TransactionEventStatus.REJECTED, transaction.status)
        assertEquals(TransactionRejectionCode.VALIDATION, transaction.rejectionCode)
        assertEquals("Transaction validation failed.", transaction.rejectionReason)
        assertEquals(7L, transaction.dataspaceId)
        assertNull(transaction.blockHeight)
        val block = assertIs<ToriiEvent.Block>(events[1])
        assertEquals(BlockEventStatus.REJECTED, block.status)
        assertEquals("EmptyBlock", block.rejectionCode)
        assertEquals("late commit", assertIs<ToriiEvent.Warning>(events[2]).details)
        assertEquals(2L, assertIs<ToriiEvent.Witness>(events[3]).writeCount)
        val verified = assertIs<ToriiEvent.ProofVerified>(events[4])
        assertEquals("halo2/ipa::vk", verified.vkRef)
        assertNull(verified.callHash)
        assertEquals("cc", assertIs<ToriiEvent.ProofRejected>(events[5]).callHash)
        val pruned = assertIs<ToriiEvent.ProofPruned>(events[6])
        assertEquals(ProofPruneOrigin.MANUAL, pruned.origin)
        assertEquals("aa", pruned.removed.single().proofHash)
        assertEquals(DataEventKind.ASSET, assertIs<ToriiEvent.DataChange>(events[7]).kind)
        assertEquals(OtherEventKind.TIME, assertIs<ToriiEvent.Other>(events[8]).kind)
        events.subList(9, events.size).forEach { assertIs<ToriiEvent.Unknown>(it) }
        assertEquals("not json", (events.last() as ToriiEvent.Unknown).raw)
        assertEquals("stream_lagged", assertNotNull(terminal.get()).code)
        assertNull(ServerSentEvent("stream_error", "{}", null).toriiEvent())
        assertIs<ToriiEvent.Block>(ServerSentEvent("message", payloads[1], null).toriiEvent())
    }

    @Test
    fun eventsCutOffByTheEndOfTheStreamAreDiscarded() {
        val body = "data: one\n\n" +
            "event: ping\nid: 7\n\n" +
            "data: two\r\ndata: lines\r\n\r\n" +
            "data: three\rid: 9\r\r" +
            "data: partial"
        val events = collectEvents(body)
        assertEquals(listOf("one", "two\nlines", "three"), events.map { it.data })
        assertEquals("9", events.last().id)
    }

    @Test
    fun oversizedLinesFailTheStream() {
        val failure = AtomicReference<Throwable?>()
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
            transport = object : TransportExecutor {
                override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                    CompletableFuture.completedFuture(
                        TransportResponse.builder()
                            .setStatusCode(200)
                            .setBody(("data: " + "x".repeat(1024 * 1024 + 1) + "\n\n").toByteArray(StandardCharsets.UTF_8))
                            .build(),
                    )
            },
        )
        val stream = client.openSseStream(
            "/v1/events/sse",
            null,
            object : ToriiEventStreamListener {
                override fun onEvent(event: ServerSentEvent) = Unit

                override fun onError(error: Throwable) {
                    failure.set(error)
                }
            },
        )
        assertFailsWith<ExecutionException> { stream.completion().get(5, TimeUnit.SECONDS) }
        assertContains(assertNotNull(failure.get()).cause?.message.orEmpty(), "exceeds")
    }

    private fun collectEvents(body: String): List<ServerSentEvent> {
        val events = mutableListOf<ServerSentEvent>()
        val client = ToriiEventStreamClient(
            baseUri = URI.create("http://example.com/base"),
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
            null,
            object : ToriiEventStreamListener {
                override fun onEvent(event: ServerSentEvent) {
                    events.add(event)
                }
            },
        ).completion().get(5, TimeUnit.SECONDS)
        return events
    }

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
