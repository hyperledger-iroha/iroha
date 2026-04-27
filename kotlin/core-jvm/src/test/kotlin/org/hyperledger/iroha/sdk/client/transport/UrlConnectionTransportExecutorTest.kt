package org.hyperledger.iroha.sdk.client.transport

import java.net.ServerSocket
import java.net.URI
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

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
}
