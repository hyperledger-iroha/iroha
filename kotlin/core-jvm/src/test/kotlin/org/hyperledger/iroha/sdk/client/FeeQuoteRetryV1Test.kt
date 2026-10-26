package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.security.KeyPairGenerator
import java.security.Signature
import java.time.Duration
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CancellationException
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.junit.jupiter.api.Test

/** Real SDK request signing and cancellation against controlled HTTP responses; no ledger writes. */
class FeeQuoteRetryV1Test {
    private val key = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
    private val authority = AccountAddress.fromAccount(key.public.encoded.takeLast(32).toByteArray(), "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
    private val network = NetworkId.parse("hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")
    private val payload = mapOf("domain" to mapOf("kind" to "network", "value" to network.literal),
        "authority" to authority, "fee_payment" to FeePaymentIntent.authority(emptyList(), 9000).toJsonMap())

    private fun auth(explicit: Boolean = false) = if (explicit)
        ToriiCanonicalRequestAuth(authority, RequestSigner.ed25519(key.private), 1_700_000_000_000L, "fixed-fee-quote")
        else ToriiCanonicalRequestAuth(authority, RequestSigner.ed25519(key.private))

    private fun response(status: Int, retry: String? = null, typedSeconds: Int? = null): TransportResponse {
        val body = if (status == 200)
            """{"intent":{"payer":"authority","value":{"charge_limits":[],"gas_limit":9000}},"observation":{"ledger_time_ms":42,"next_block_height":7,"route_dataspace_id":0},"components":[],"capacities":[],"decision":{"status":"accepted","value":{"debit_source":{"kind":"account","value":"$authority"},"program_revision":null}}}"""
        else """{"code":"query_validation_failed","message":"capacity","details":${if (typedSeconds == null) "{}" else "{\"retry_after_seconds\":$typedSeconds}"}}"""
        return TransportResponse.builder().setStatusCode(status).setBody(body.toByteArray())
            .addHeader("Content-Type", "application/json").apply { retry?.let { addHeader("Retry-After", it) } }.build()
    }

    private class Wire(val answer: (Int) -> CompletableFuture<TransportResponse>) : HttpTransportExecutor {
        val requests = CopyOnWriteArrayList<TransportRequest>()
        val times = CopyOnWriteArrayList<Long>()
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request); times.add(System.nanoTime())
            return answer(requests.size)
        }
    }

    private fun client(wire: Wire, timeout: Duration = Duration.ofSeconds(10)) = HttpClientTransport(wire,
        ClientConfig.builder().setBaseUri(URI("https://torii.example"))
            .setLocalSigningContext(LocalSigningContext(network)).setRequestTimeout(timeout).build())

    @Test fun `capacity retry honors typed wait and signs same payload with fresh nonces`() {
        val wire = Wire { CompletableFuture.completedFuture(if (it == 1) response(429, "0", 1) else response(200)) }
        client(wire).use { transport ->
            transport.quoteFees(payload, auth()).get(5, TimeUnit.SECONDS)
            assertEquals(2, wire.requests.size)
            assertTrue(wire.times[1] - wire.times[0] >= TimeUnit.SECONDS.toNanos(1))
            val (first, second) = wire.requests
            assertContentEquals(first.body, second.body)
            assertNotEquals(first.headers[CanonicalRequestSigner.HEADER_NONCE], second.headers[CanonicalRequestSigner.HEADER_NONCE])
            for (request in wire.requests) {
                val message = CanonicalRequestSigner.canonicalRequestSignatureMessage(network, request.method,
                    request.uri, request.body, request.headers.getValue(CanonicalRequestSigner.HEADER_TIMESTAMP_MS).single().toLong(),
                    request.headers.getValue(CanonicalRequestSigner.HEADER_NONCE).single())
                assertTrue(Signature.getInstance("Ed25519").run { initVerify(key.public); update(message)
                    verify(Base64.getDecoder().decode(request.headers.getValue(CanonicalRequestSigner.HEADER_SIGNATURE).single())) })
            }
        }
    }

    @Test fun `capacity exhaustion stops after four requests preserving the last error`() {
        val wire = Wire { CompletableFuture.completedFuture(response(429)) }
        client(wire).use { transport ->
            val error = assertFailsWith<ExecutionException> { transport.quoteFees(payload, auth()).get(5, TimeUnit.SECONDS) }
            assertEquals(429, assertIs<ToriiApiException>(error.cause).status)
            assertEquals(4, wire.requests.size)
        }
    }

    @Test fun `cancelling backoff or closing client prevents another request`() {
        for (close in listOf(false, true)) {
            val wire = Wire { CompletableFuture.completedFuture(response(429, "1")) }
            client(wire).use { transport ->
                val future = transport.quoteFees(payload, auth())
                if (close) transport.close() else future.cancel(false)
                Thread.sleep(1200)
                assertTrue(future.isCancelled)
                assertEquals(1, wire.requests.size)
            }
        }
    }

    @Test fun `cancellation and absolute deadline cancel active HTTP`() {
        for (cancel in listOf(false, true)) {
            val upstream = CompletableFuture<TransportResponse>()
            val wire = Wire { upstream }
            client(wire, Duration.ofMillis(80)).use { transport ->
                val future = transport.quoteFees(payload, auth())
                if (cancel) future.cancel(false)
                else assertIs<TimeoutException>(assertFailsWith<ExecutionException> { future.get(2, TimeUnit.SECONDS) }.cause)
                assertFailsWith<CancellationException> { upstream.get(2, TimeUnit.SECONDS) }
                assertEquals(1, wire.requests.size)
            }
        }
    }

    @Test fun `auth policy failures fixed nonces and excessive server delay never retry`() {
        for ((status, fixed, wait) in listOf(Triple(403, false, null), Triple(422, false, null),
            Triple(503, false, null), Triple(429, true, null), Triple(429, false, "60"))) {
            val wire = Wire { CompletableFuture.completedFuture(response(status, wait)) }
            client(wire).use { transport ->
                assertEquals(status, assertIs<ToriiApiException>(assertFailsWith<ExecutionException> {
                    transport.quoteFees(payload, auth(fixed)).get(2, TimeUnit.SECONDS)
                }.cause).status)
                assertEquals(1, wire.requests.size)
            }
        }
    }

    @Test fun `retry delay handles seconds dates malformed values and overflow without tight loops`() {
        fun delay(header: String?, typed: Int? = null): Long {
            val response = response(429, header, typed)
            return FeeQuoteRetryV1.retryDelayMillis(1, response, ToriiApiException.fromResponse(429, response.headers, response.body))
        }
        assertTrue(delay("2") in 2000..2100)
        assertTrue(delay("garbage") in 250..350)
        assertEquals(Long.MAX_VALUE, delay("9999999999999999999999999999"))
        val date = ZonedDateTime.now(java.time.ZoneOffset.UTC).plusSeconds(5).format(DateTimeFormatter.RFC_1123_DATE_TIME)
        assertTrue(delay(date) in 3500..5100)
        assertTrue(delay("1", 2) in 2000..2100)
        val lowercase = TransportResponse.builder().setStatusCode(429).addHeader("retry-after", "2").build()
        assertTrue(FeeQuoteRetryV1.retryDelayMillis(1, lowercase,
            ToriiApiException.fromResponse(429, lowercase.headers, lowercase.body)) in 2000..2100)
    }
}
