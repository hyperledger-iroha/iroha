package org.hyperledger.iroha.sdk.client.transport

import java.net.ServerSocket
import java.net.URI
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class UrlConnectionTransportExecutorTest {

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
