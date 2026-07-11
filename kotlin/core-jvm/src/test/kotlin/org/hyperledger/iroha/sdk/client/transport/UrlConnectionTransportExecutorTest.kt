package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.ServerSocket
import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.time.Duration
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicReference
import javax.tools.ToolProvider
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.ClientConfig
import org.hyperledger.iroha.sdk.client.HttpClientTransport

class UrlConnectionTransportExecutorTest {

    @Test
    fun executeRunsOnSuppliedAsyncExecutor() {
        val capturedThreadName = AtomicReference<String?>(null)
        val pool = Executors.newSingleThreadExecutor { runnable ->
            Thread(runnable, "url-connection-test-pool-1").apply { isDaemon = true }
        }
        try {
            ServerSocket(0).use { server ->
                val port = server.localPort
                val serverThread = Thread {
                    server.accept().use { socket ->
                        val input = socket.getInputStream()
                        val sb = StringBuilder()
                        while (true) {
                            val b = input.read()
                            if (b == -1) break
                            sb.append(b.toChar())
                            if (sb.endsWith("\r\n\r\n")) break
                        }
                        val out = socket.getOutputStream()
                        out.write("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".toByteArray())
                        out.flush()
                    }
                }
                serverThread.isDaemon = true
                serverThread.start()

                val executor = UrlConnectionTransportExecutor(
                    connectTimeout = null,
                    readTimeout = null,
                    asyncExecutor = { runnable ->
                        pool.execute {
                            capturedThreadName.set(Thread.currentThread().name)
                            runnable.run()
                        }
                    },
                )
                val request = TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("http://localhost:$port/health"))
                    .build()

                val response = executor.execute(request).get()

                assertEquals(200, response.statusCode, "Status code should be 200")
                val name = assertNotNull(capturedThreadName.get(), "Async executor should have been invoked")
                assertEquals("url-connection-test-pool-1", name, "Executor must run on the supplied pool, not commonPool")

                serverThread.join(2000)
            }
        } finally {
            pool.shutdownNow()
        }
    }

    @Test
    fun executeReturns404WithEmptyBodyWhenServerSendsNoContent() {
        ServerSocket(0).use { server ->
            val port = server.localPort
            val serverThread = Thread {
                server.accept().use { socket ->
                    val input = socket.getInputStream()
                    val sb = StringBuilder()
                    while (true) {
                        val b = input.read()
                        if (b == -1) break
                        sb.append(b.toChar())
                        if (sb.endsWith("\r\n\r\n")) break
                    }
                    val out = socket.getOutputStream()
                    out.write("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".toByteArray())
                    out.flush()
                }
            }
            serverThread.isDaemon = true
            serverThread.start()

            val executor = UrlConnectionTransportExecutor()
            val request = TransportRequest.builder()
                .setMethod("POST")
                .setUri(URI.create("http://localhost:$port/v1/aliases/resolve"))
                .addHeader("Content-Type", "application/json")
                .setBody("""{"alias":"missing@test"}""".toByteArray())
                .build()

            val response = executor.execute(request).get()

            assertEquals(404, response.statusCode, "Status code should be 404")
            assertTrue(response.body.isEmpty(), "Body should be empty for 404 with Content-Length: 0")

            serverThread.join(2000)
        }
    }

    @Test
    fun bufferedResponseAcceptsExactConfiguredLimit() {
        val response = executeRawResponse(
            "HTTP/1.1 200 OK\r\nContent-Length: 8\r\nConnection: close\r\n\r\n12345678",
            maximumResponseBytes = 8,
        )

        assertEquals("12345678", response.body.toString(StandardCharsets.UTF_8))
    }

    @Test
    fun headResponseDoesNotTreatRepresentationLengthAsBufferedBodyLength() {
        val response = executeRawResponse(
            "HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\n",
            maximumResponseBytes = 8,
            requestMethod = "HEAD",
        )

        assertTrue(response.body.isEmpty())
    }

    @Test
    fun bufferedResponseRejectsDeclaredLengthAboveLimitBeforeReadingBody() {
        val failure = assertFailsWith<ExecutionException> {
            executeRawResponse(
                "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789",
                maximumResponseBytes = 8,
            )
        }

        assertTrue(failure.hasCauseMessage("Content-Length 9 exceeds the 8-byte limit"))
    }

    @Test
    fun bufferedResponseRejectsActualOverflowWithoutContentLength() {
        val failure = assertFailsWith<ExecutionException> {
            executeRawResponse(
                "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n123456789",
                maximumResponseBytes = 8,
            )
        }

        assertTrue(failure.hasCauseMessage("body exceeds the 8-byte limit"))
    }

    @Test
    fun perRequestLimitAppliesToDeclaredAndActualNonSuccessBodies() {
        val declared = assertFailsWith<ExecutionException> {
            executeRawResponse(
                "HTTP/1.1 500 Error\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789",
                maximumResponseBytes = 64,
                requestMaximumResponseBytes = 8,
            )
        }
        assertTrue(declared.hasCauseMessage("Content-Length 9 exceeds the 8-byte limit"))

        val actual = assertFailsWith<ExecutionException> {
            executeRawResponse(
                "HTTP/1.1 500 Error\r\nConnection: close\r\n\r\n123456789",
                maximumResponseBytes = 64,
                requestMaximumResponseBytes = 8,
            )
        }
        assertTrue(actual.hasCauseMessage("body exceeds the 8-byte limit"))
    }

    @Test
    fun perRequestLimitCannotRaiseExecutorMaximum() {
        val failure = assertFailsWith<ExecutionException> {
            executeRawResponse(
                "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n123456789",
                maximumResponseBytes = 8,
                requestMaximumResponseBytes = 16,
            )
        }

        assertTrue(failure.hasCauseMessage("body exceeds the 8-byte limit"))
    }

    @Test
    fun boundedReaderRejectsActualOverflowWithUnderstatedContentLength() {
        val failure = assertFailsWith<IOException> {
            UrlConnectionTransportExecutor.readBoundedBody(
                ByteArrayInputStream("123456789".toByteArray(StandardCharsets.UTF_8)),
                maximumResponseBytes = 8,
                declaredLength = 1,
            )
        }

        assertTrue(failure.message.orEmpty().contains("body exceeds the 8-byte limit"))
    }

    @Test
    fun boundedReaderRejectsTruncatedDeclaredBody() {
        val failure = assertFailsWith<IOException> {
            UrlConnectionTransportExecutor.readBoundedBody(
                ByteArrayInputStream("1234".toByteArray(StandardCharsets.UTF_8)),
                maximumResponseBytes = 8,
                declaredLength = 5,
            )
        }

        assertTrue(failure.message.orEmpty().contains("does not match Content-Length 5"))
    }

    @Test
    fun boundedReaderRejectsHostileInputStreamReadCounts() {
        val noProgress = object : InputStream() {
            override fun read(): Int = 0
            override fun read(bytes: ByteArray, offset: Int, length: Int): Int = 0
        }
        assertFailsWith<IOException> {
            UrlConnectionTransportExecutor.readBoundedBody(noProgress, 8, null)
        }

        val overReported = object : InputStream() {
            override fun read(): Int = 0
            override fun read(bytes: ByteArray, offset: Int, length: Int): Int = length + 1
        }
        assertFailsWith<IOException> {
            UrlConnectionTransportExecutor.readBoundedBody(overReported, 8, null)
        }
    }

    @Test
    fun encodedContentLengthIsNotComparedToDecodedBufferedBytes() {
        val headers = mapOf<String?, List<String>?>(
            "Content-Length" to listOf("1"),
            "Content-Encoding" to listOf("gzip"),
        )
        val bufferedLength = UrlConnectionTransportExecutor.contentLengthForBufferedBody(headers, 1L)
        assertNull(bufferedLength)
        val exact = UrlConnectionTransportExecutor.readBoundedBody(
            ByteArrayInputStream("12345678".toByteArray(StandardCharsets.UTF_8)),
            maximumResponseBytes = 8,
            declaredLength = bufferedLength,
        )
        assertEquals("12345678", exact.toString(StandardCharsets.UTF_8))

        val overflow = assertFailsWith<IOException> {
            UrlConnectionTransportExecutor.readBoundedBody(
                ByteArrayInputStream("123456789".toByteArray(StandardCharsets.UTF_8)),
                maximumResponseBytes = 8,
                declaredLength = bufferedLength,
            )
        }
        assertTrue(overflow.message.orEmpty().contains("body exceeds the 8-byte limit"))
        assertEquals(
            1L,
            UrlConnectionTransportExecutor.contentLengthForBufferedBody(
                mapOf("Content-Encoding" to listOf("identity")),
                1L,
            ),
        )
    }

    @Test
    fun contentLengthMustBeCanonicalUnsignedDecimal() {
        assertEquals(0L, UrlConnectionTransportExecutor.parseCanonicalContentLength("0"))
        assertEquals(8L, UrlConnectionTransportExecutor.parseCanonicalContentLength("8"))
        assertEquals(
            Long.MAX_VALUE,
            UrlConnectionTransportExecutor.parseCanonicalContentLength(Long.MAX_VALUE.toString()),
        )

        val invalid = listOf(
            "",
            "-1",
            "+1",
            "01",
            " 1",
            "1 ",
            "1,1",
            "1x",
            "١",
            "9223372036854775808",
        )
        for (value in invalid) {
            assertFailsWith<IOException>("Content-Length '$value' must be rejected") {
                UrlConnectionTransportExecutor.parseCanonicalContentLength(value)
            }
        }
    }

    @Test
    fun bufferedResponseRejectsAmbiguousLengthAndTransferEncoding() {
        val failure = assertFailsWith<ExecutionException> {
            executeRawResponse(
                "HTTP/1.1 200 OK\r\n" +
                    "Content-Length: 1\r\n" +
                    "Transfer-Encoding: chunked\r\n" +
                    "Connection: close\r\n\r\n" +
                    "1\r\na\r\n0\r\n\r\n",
                maximumResponseBytes = 8,
            )
        }

        assertTrue(failure.hasCauseMessage("must not combine Content-Length with Transfer-Encoding"))
    }

    @Test
    fun bufferedResponseLimitMustFitInByteArray() {
        assertFailsWith<IllegalArgumentException> { UrlConnectionTransportExecutor(0) }
        assertFailsWith<IllegalArgumentException> {
            UrlConnectionTransportExecutor(Int.MAX_VALUE.toLong() + 1L)
        }
        assertEquals(
            64L * 1024L * 1024L,
            UrlConnectionTransportExecutor.DEFAULT_MAXIMUM_RESPONSE_BYTES,
        )
        assertFailsWith<IllegalArgumentException> {
            TransportRequest.builder().setMaximumResponseBytes(0)
        }
    }

    @Test
    fun injectedDefaultExecutorPreservesUrlConnectionTimeoutDefaults() {
        val asyncExecutor = Executor { runnable -> runnable.run() }
        val config = ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRequestTimeout(Duration.ofMillis(123))
            .build()

        val transport = HttpClientTransport.withDefaultExecutor(config, asyncExecutor)
        val executor = fieldValue(transport, "executor")

        assertTrue(executor is UrlConnectionTransportExecutor, "Injected default executor should use URLConnection")
        assertNull(fieldValue(executor, "connectTimeout"), "Connect timeout should use URLConnection defaults")
        assertNull(fieldValue(executor, "readTimeout"), "Read timeout should use URLConnection defaults")
        assertSame(asyncExecutor, fieldValue(executor, "asyncExecutor"), "Async executor should be injected unchanged")
    }

    @Test
    fun javaCallersCanUseTwoTimeoutConstructor() {
        val compiler = assertNotNull(ToolProvider.getSystemJavaCompiler(), "JDK compiler should be available")
        val sourceDir = Files.createTempDirectory("iroha-java-interop")
        val outputDir = Files.createTempDirectory("iroha-java-interop-classes")
        val sourceFile = sourceDir.resolve("UrlConnectionConstructorCheck.java")
        val source = """
            import java.time.Duration;
            import java.util.concurrent.Executor;
            import org.hyperledger.iroha.sdk.client.transport.UrlConnectionTransportExecutor;

            final class UrlConnectionConstructorCheck {
                static UrlConnectionTransportExecutor withDistinctTimeouts() {
                    return new UrlConnectionTransportExecutor(Duration.ofMillis(1), Duration.ofMillis(2));
                }

                static UrlConnectionTransportExecutor withDefaultTimeouts() {
                    return new UrlConnectionTransportExecutor((Duration) null, (Duration) null);
                }

                static UrlConnectionTransportExecutor withAsyncExecutor(Executor executor) {
                    return new UrlConnectionTransportExecutor(null, null, executor);
                }

                static UrlConnectionTransportExecutor withMaximumResponseBytes() {
                    return new UrlConnectionTransportExecutor(null, null, null, 8L);
                }
            }
        """.trimIndent()
        Files.write(sourceFile, source.toByteArray(StandardCharsets.UTF_8))
        val errors = ByteArrayOutputStream()

        val result = compiler.run(
            null,
            null,
            errors,
            "-classpath",
            System.getProperty("java.class.path"),
            "-d",
            outputDir.toString(),
            sourceFile.toString(),
        )

        assertEquals(0, result, errors.toString("UTF-8"))
    }

    private fun fieldValue(target: Any, name: String): Any? {
        val field = target.javaClass.getDeclaredField(name)
        field.isAccessible = true
        return field.get(target)
    }

    private fun executeRawResponse(
        rawResponse: String,
        maximumResponseBytes: Long,
        requestMethod: String = "GET",
        requestMaximumResponseBytes: Long? = null,
    ): TransportResponse =
        ServerSocket(0).use { server ->
            val serverThread = Thread {
                runCatching {
                    server.accept().use { socket ->
                        val input = socket.getInputStream()
                        val requestHeaders = StringBuilder()
                        while (!requestHeaders.endsWith("\r\n\r\n")) {
                            val next = input.read()
                            if (next == -1) break
                            requestHeaders.append(next.toChar())
                        }
                        socket.getOutputStream().use { output ->
                            output.write(rawResponse.toByteArray(StandardCharsets.UTF_8))
                            output.flush()
                        }
                    }
                }
            }.apply {
                isDaemon = true
                start()
            }
            try {
                val request = TransportRequest.builder()
                    .setMethod(requestMethod)
                    .setUri(URI.create("http://127.0.0.1:${server.localPort}/bounded"))
                    .apply {
                        if (requestMaximumResponseBytes != null) {
                            setMaximumResponseBytes(requestMaximumResponseBytes)
                        }
                    }
                    .build()
                UrlConnectionTransportExecutor(maximumResponseBytes).execute(request).get()
            } finally {
                serverThread.join(2000)
            }
        }

    private fun Throwable.hasCauseMessage(fragment: String): Boolean =
        generateSequence(this) { it.cause }
            .any { it.message.orEmpty().contains(fragment) }
}
