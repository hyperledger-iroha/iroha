package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.net.URI
import java.net.InetSocketAddress
import java.net.Proxy
import java.time.Duration
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.zip.GZIPOutputStream
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import okio.Buffer
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class OkHttpTransportLifecycleTest {
    @Test
    fun signedReadsAndSubmissionsReturnRetryHintsWithoutRedispatch() {
        for (method in listOf("GET", "POST")) {
            for (status in listOf(307, 308, 401, 408, 503)) {
                MockWebServer().use { server ->
                    server.enqueue(MockResponse().setResponseCode(status)
                        .setHeader("Retry-After", "0").setHeader("Location", "/other")
                        .setHeader("WWW-Authenticate", "Basic realm=private").setBody("original"))
                    server.enqueue(MockResponse().setBody("must not dispatch"))
                    OkHttpTransportExecutor.create().use { transport ->
                        val request = request(server, method, signed = true)
                        val result = transport.execute(request).get(5, TimeUnit.SECONDS)
                        assertEquals(status, result.statusCode)
                        assertEquals(listOf("0"), result.headers["Retry-After"])
                        assertEquals("original", String(result.body, Charsets.UTF_8))
                        assertEquals(request.uri, result.finalUri)
                        assertFalse(result.redirected)
                        assertEquals(1, server.requestCount)
                        val observed = assertNotNull(server.takeRequest(1, TimeUnit.SECONDS))
                        assertContentEquals(request.body, observed.body.readByteArray())
                        assertEquals("signed-input", observed.getHeader("X-Iroha-Signature"))
                    }
                }
            }
        }
    }

    @Test
    fun proxyAuthenticationCannotReplayASignedRequest() {
        MockWebServer().use { proxy ->
            proxy.start()
            proxy.enqueue(MockResponse().setResponseCode(407)
                .setHeader("Proxy-Authenticate", "Basic realm=proxy"))
            val client = OkHttpClient.Builder().proxy(
                Proxy(Proxy.Type.HTTP, InetSocketAddress("127.0.0.1", proxy.port)),
            ).build()
            try {
                OkHttpTransportExecutor(client).use { transport ->
                    val request = TransportRequest.builder().setUri(URI("http://example.invalid/signed"))
                        .addHeader("X-Iroha-Signature", "signed-input").build()
                    assertEquals(407, transport.execute(request).get(5, TimeUnit.SECONDS).statusCode)
                    assertEquals(1, proxy.requestCount)
                }
            } finally { closeClient(client) }
        }
    }

    @Test
    fun connectionLossAfterSubmissionNeverReplaysTheSignedBytes() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AFTER_REQUEST))
            server.enqueue(MockResponse().setResponseCode(202))
            OkHttpTransportExecutor.create().use { transport ->
                assertFailsWith<ExecutionException> {
                    transport.execute(request(server, "POST", signed = true)).get(5, TimeUnit.SECONDS)
                }
                assertEquals(1, server.requestCount)
            }
        }
    }

    @Test
    fun publicReadsRetainExplicitlyRetrySafeConnectionBehavior() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().setResponseCode(503).setHeader("Retry-After", "0"))
            server.enqueue(MockResponse().setBody("read"))
            OkHttpTransportExecutor.create().use { transport ->
                val result = transport.execute(request(server)).get(5, TimeUnit.SECONDS)
                assertEquals("read", String(result.body, Charsets.UTF_8))
                assertFalse(result.redirected)
                assertEquals(2, server.requestCount)
            }
        }
    }

    @Test
    fun closeCancelsOnlyThisAdaptersCallsAndOpenStreams() {
        val shared = OkHttpClient.Builder().readTimeout(Duration.ofSeconds(30)).build()
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().setBody("stream").setBodyDelay(2, TimeUnit.SECONDS))
                server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
                server.enqueue(MockResponse().setBody("unrelated"))
                OkHttpTransportExecutor(shared).use { first ->
                    OkHttpTransportExecutor(shared).use { second ->
                        val stream = first.openStream(request(server)).get(5, TimeUnit.SECONDS)
                        assertNotNull(server.takeRequest(1, TimeUnit.SECONDS))
                        val reader = CompletableFuture.supplyAsync { stream.body.read() }
                        val pending = first.execute(request(server))
                        assertNotNull(server.takeRequest(1, TimeUnit.SECONDS))
                        first.close()
                        first.close()
                        assertTrue(pending.isCancelled)
                        assertFailsWith<ExecutionException> { reader.get(5, TimeUnit.SECONDS) }
                        assertFalse(shared.dispatcher.executorService.isShutdown)
                        val result = second.execute(request(server)).get(5, TimeUnit.SECONDS)
                        assertEquals("unrelated", String(result.body, Charsets.UTF_8))
                        assertFailsWith<ExecutionException> {
                            first.execute(request(server)).get(1, TimeUnit.SECONDS)
                        }
                        assertFailsWith<ExecutionException> {
                            first.openStream(request(server)).get(1, TimeUnit.SECONDS)
                        }
                        assertEquals(3, server.requestCount)
                    }
                }
            }
        } finally { closeClient(shared) }
    }

    @Test
    fun futureCancellationStopsAnActiveBufferedRead() {
        val client = OkHttpClient()
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
                OkHttpTransportExecutor(client).use { transport ->
                    val future = transport.execute(request(server))
                    assertNotNull(server.takeRequest(1, TimeUnit.SECONDS))
                    assertTrue(future.cancel(true))
                    awaitIdle(client)
                    assertTrue(future.isCancelled)
                    assertEquals(1, server.requestCount)
                }
            }
        } finally { closeClient(client) }
    }

    @Test
    fun cancellingAQueuedRequestDoesNotStartNetworkIo() {
        val commands = ArrayList<Runnable>()
        MockWebServer().use { server ->
            server.start()
            OkHttpTransportExecutor.create(asyncExecutor = Executor { commands.add(it) }).use { transport ->
                val future = transport.execute(request(server, "POST", signed = true))
                assertEquals(1, commands.size)
                assertTrue(future.cancel(false))
                commands.single().run()
                assertTrue(future.isCancelled)
                assertEquals(0, server.requestCount)
            }
        }
    }

    @Test
    fun closingAnOwnedAdapterPreservesAnInjectedSchedulingExecutor() {
        val executor = Executors.newSingleThreadExecutor()
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().setBody("ok"))
                val transport = OkHttpTransportExecutor.create(asyncExecutor = executor)
                assertEquals(200, transport.execute(request(server)).get(5, TimeUnit.SECONDS).statusCode)
                transport.close()
                assertFalse(executor.isShutdown)
                assertEquals(42, executor.submit<Int> { 42 }.get(1, TimeUnit.SECONDS))
            }
        } finally { executor.shutdownNow() }
    }

    @Test
    fun closingDuringStreamDeliveryDisposesTheDeliveredResponse() {
        val client = OkHttpClient()
        try {
            MockWebServer().use { server ->
                server.enqueue(MockResponse().setBody("long body").setBodyDelay(2, TimeUnit.SECONDS))
                val transport = OkHttpTransportExecutor(client)
                val future = transport.openStream(request(server))
                val closed = future.thenApply { response -> transport.close(); response }
                val response = closed.get(5, TimeUnit.SECONDS)
                assertFailsWith<IOException> { response.body.read() }
                response.close()
                awaitIdle(client)
            }
        } finally { closeClient(client) }
    }

    @Test
    fun gzipLimitCountsDecodedBytes() {
        val encoded = ByteArrayOutputStream().apply {
            GZIPOutputStream(this).use { it.write(ByteArray(4096) { 42 }) }
        }.toByteArray()
        assertTrue(encoded.size < 128)
        MockWebServer().use { server ->
            server.enqueue(MockResponse().setHeader("Content-Encoding", "gzip").setBody(Buffer().write(encoded)))
            OkHttpTransportExecutor.create(maximumResponseBytes = 128).use { transport ->
                val error = assertFailsWith<ExecutionException> {
                    transport.execute(request(server)).get(5, TimeUnit.SECONDS)
                }
                assertTrue(error.cause?.message.orEmpty().contains("body exceeds the 128-byte limit"))
            }
        }
    }

    @Test
    fun requestDeadlineCancelsWithoutReplaying() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
            OkHttpTransportExecutor.create().use { transport ->
                val request = TransportRequest.builder().setUri(server.url("/signed").toUri())
                    .addHeader("X-Iroha-Signature", "signed-input")
                    .setTimeout(Duration.ofMillis(250)).build()
                val error = assertFailsWith<ExecutionException> {
                    transport.execute(request).get(3, TimeUnit.SECONDS)
                }
                assertTrue(error.cause is IOException)
                assertEquals(1, server.requestCount)
            }
        }
    }

    @Test
    fun requestAndStreamingHeadersAreImmutableOwnedSnapshots() {
        val values = arrayListOf("signed-input")
        val headers = linkedMapOf("X-Iroha-Signature" to values as List<String>)
        val request = TransportRequest("GET", URI("https://example.invalid"), headers, byteArrayOf())
        values.clear()
        headers.clear()
        assertEquals(listOf("signed-input"), request.headers["X-Iroha-Signature"])
        assertFailsWith<UnsupportedOperationException> {
            (request.headers as MutableMap<String, List<String>>).clear()
        }
        assertFailsWith<UnsupportedOperationException> {
            (request.headers["X-Iroha-Signature"] as MutableList<String>).clear()
        }
        TransportStreamResponse(200, null, null, request.headers, null).use { response ->
            assertFailsWith<UnsupportedOperationException> {
                (response.headers as MutableMap<String, List<String>>).clear()
            }
            assertFailsWith<UnsupportedOperationException> {
                (response.headers["X-Iroha-Signature"] as MutableList<String>).clear()
            }
        }
        assertFailsWith<IllegalArgumentException> {
            TransportRequest("GET", URI("https://example.invalid"), emptyMap(), byteArrayOf(), Duration.ofSeconds(-1))
        }
    }

    @Test
    fun applicationInterceptorsCannotChangeCanonicalDispatch() {
        val client = OkHttpClient.Builder().addInterceptor { it.proceed(it.request()) }.build()
        try {
            assertFailsWith<IllegalArgumentException> { OkHttpTransportExecutor(client) }
        } finally { closeClient(client) }
    }

    private fun request(server: MockWebServer, method: String = "GET", signed: Boolean = false): TransportRequest =
        TransportRequest.builder().setMethod(method).setUri(server.url("/capability").toUri())
            .apply {
                if (signed) addHeader("X-Iroha-Signature", "signed-input")
                if (method == "POST") setBody(byteArrayOf(0, 42, -1))
            }.build()

    private fun awaitIdle(client: OkHttpClient) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (client.dispatcher.runningCallsCount() != 0 && System.nanoTime() < deadline) Thread.sleep(10)
        assertEquals(0, client.dispatcher.runningCallsCount())
    }

    private fun closeClient(client: OkHttpClient) {
        client.dispatcher.cancelAll()
        client.connectionPool.evictAll()
        client.dispatcher.executorService.shutdownNow()
    }
}
