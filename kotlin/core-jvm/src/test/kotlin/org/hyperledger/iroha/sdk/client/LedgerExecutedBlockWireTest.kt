package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class LedgerExecutedBlockWireTest {
    @Test
    fun fetchesExactBoundedNoritoWireAtFullU64Height() {
        val body = byteArrayOf(0x4e, 0x52, 0x54, 0x30)
        val executor = OneResponseExecutor(
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(body)
                .addHeader("Content-Type", "application/x-norito")
                .addHeader("Content-Length", body.size.toString())
                .build(),
        )
        val client = client(executor)

        val received = client.getLedgerExecutedBlockWire(BigInteger("18446744073709551615")).join()

        assertContentEquals(body, received)
        assertEquals("GET", executor.request.method)
        assertEquals(
            "/v1/ledger/block/18446744073709551615",
            executor.request.uri.rawPath,
        )
        assertEquals(listOf("application/x-norito"), executor.request.headers["Accept"])
        assertEquals(32L * 1024L * 1024L, executor.request.maximumResponseBytes)
    }

    @Test
    fun rejectsInvalidHeightBeforeDispatch() {
        val executor = OneResponseExecutor(success(byteArrayOf(1)))
        val client = client(executor)
        for (height in listOf(BigInteger.ZERO, BigInteger.valueOf(-1), BigInteger.ONE.shiftLeft(64))) {
            assertFailsWith<IllegalArgumentException> {
                client.getLedgerExecutedBlockWire(height)
            }
        }
        assertEquals(0, executor.requestCount)
    }

    @Test
    fun rejectsNonExactStatusMediaTypeLengthAndEmptyBodies() {
        val hostile = listOf(
            TransportResponse.builder()
                .setStatusCode(201)
                .setBody(byteArrayOf(1))
                .addHeader("Content-Type", "application/x-norito")
                .build(),
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(byteArrayOf(1))
                .addHeader("Content-Type", "application/x-norito; charset=binary")
                .build(),
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(byteArrayOf(1))
                .addHeader("Content-Type", "application/x-norito")
                .addHeader("Content-Type", "application/x-norito")
                .build(),
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(byteArrayOf(1))
                .addHeader("Content-Type", "application/x-norito")
                .addHeader("Content-Length", "01")
                .build(),
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(byteArrayOf(1))
                .addHeader("Content-Type", "application/x-norito")
                .addHeader("Content-Length", "2")
                .build(),
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(byteArrayOf())
                .addHeader("Content-Type", "application/x-norito")
                .build(),
        )
        for (response in hostile) {
            val error = assertFailsWith<CompletionException> {
                client(OneResponseExecutor(response)).getLedgerExecutedBlockWire(1).join()
            }
            assertTrue(error.cause is RuntimeException)
        }
    }

    @Test
    fun rejectsExecutorThatViolatesTheDeclaredResponseCeiling() {
        val oversized = ByteArray(32 * 1024 * 1024 + 1)
        val error = assertFailsWith<CompletionException> {
            client(OneResponseExecutor(success(oversized))).getLedgerExecutedBlockWire(1).join()
        }
        assertTrue(error.cause?.message?.contains("exceeds") == true)
    }

    private fun client(executor: HttpTransportExecutor): HttpClientTransport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

    private fun success(body: ByteArray): TransportResponse =
        TransportResponse.builder()
            .setStatusCode(200)
            .setBody(body)
            .addHeader("Content-Type", "application/x-norito")
            .build()

    private class OneResponseExecutor(
        private val response: TransportResponse,
    ) : HttpTransportExecutor {
        lateinit var request: TransportRequest
        var requestCount = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            this.request = request
            requestCount += 1
            return CompletableFuture.completedFuture(response)
        }
    }
}
