// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.security.KeyPairGenerator
import java.util.Locale
import java.util.Optional
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.TimeUnit
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.testing.TestNetworkIds

class AnonymousBallotApiV1Test {
    private val base = URI.create("https://torii.example/api")
    private val envelope = ByteArray(1_024) { (it * 31 + 7).toByte() }
    private val scope = ByteArray(AnonymousBallotApiV1.SCOPE_BYTES) { 0xcd.toByte() }
    private val attemptId = ByteArray(AnonymousBallotApiV1.GOVERNANCE_ATTEMPT_ID_BYTES) { 0x5a }

    /** Every header a configured client could carry that must never reach the ballot route. */
    private val configuredHeaders = linkedMapOf(
        "X-API-Token" to "listener-secret",
        "Authorization" to "Bearer session",
        "Cookie" to "session=identifying",
        CanonicalRequestSigner.HEADER_ACCOUNT to "alice@universal",
        CanonicalRequestSigner.HEADER_SIGNATURE to "c2lnbmF0dXJl",
        "X-Iroha-Witness" to "d2l0bmVzcw",
        "X-Request-Id" to "correlate-me",
        "traceparent" to "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01",
    )

    @Test
    fun pathsCarryOneCanonicalLowercaseKey() {
        val expectedBoard = "/v1/gov/anonymous-ballots/boards/" + "cd".repeat(64)
        assertEquals(expectedBoard, AnonymousBallotApiV1.boardReadPath(scope))
        assertEquals(expectedBoard, AnonymousBallotApiV1.boardReadPath("cd".repeat(64)))
        val expectedAttempt = "/v1/gov/anonymous-ballots/attempts/" + "5a".repeat(32)
        assertEquals(expectedAttempt, AnonymousBallotApiV1.attemptReadPath(attemptId))
        assertEquals(expectedAttempt, AnonymousBallotApiV1.attemptReadPath("5a".repeat(32)))
        assertEquals("00ab5f0f", AnonymousBallotApiV1.lowerHex(byteArrayOf(0x00, 0xab.toByte(), 0x5f, 0x0f)))
        val mixed = ByteArray(32) { it.toByte() }
        assertEquals(
            "/v1/gov/anonymous-ballots/attempts/" +
                (0 until 32).joinToString("") { String.format(Locale.ROOT, "%02x", it) },
            AnonymousBallotApiV1.attemptReadPath(mixed),
        )

        for (size in listOf(0, 63, 65, 128)) {
            assertFailsWith<IllegalArgumentException> { AnonymousBallotApiV1.boardReadPath(ByteArray(size)) }
        }
        for (size in listOf(0, 31, 33, 64)) {
            assertFailsWith<IllegalArgumentException> { AnonymousBallotApiV1.attemptReadPath(ByteArray(size)) }
        }
        for (hex in listOf("cd".repeat(63) + "c", "cd".repeat(65), "CD".repeat(64), "cd".repeat(63) + "zz", "")) {
            assertFailsWith<IllegalArgumentException> { AnonymousBallotApiV1.boardReadPath(hex) }
        }
        for (hex in listOf("5a".repeat(31), "5a".repeat(33), "5A".repeat(32), "5a".repeat(31) + "g0", " " + "5a".repeat(31) + "5")) {
            assertFailsWith<IllegalArgumentException> { AnonymousBallotApiV1.attemptReadPath(hex) }
        }
    }

    @Test
    fun aBallotSubmissionCarriesNoneOfTheConfiguredCredentials() {
        val executor = StubExecutor(TransportResponse(202, ByteArray(0), "Accepted", emptyMap(), null, false))
        val observer = RecordingObserver()
        val keys = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val config = ClientConfig.builder()
            .setBaseUri(base)
            .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
            .setCanonicalAuth(ToriiCanonicalRequestAuth("alice@universal", RequestSigner.ed25519(keys.private)))
            .setDefaultHeaders(configuredHeaders)
            .addObserver(observer)
            .build()

        HttpClientTransport(executor, config).use { transport ->
            transport.newAnonymousBallotToriiClientV1().use { client ->
                assertNull(client.submitAnonymousBallot(envelope).join())
                val submission = executor.requests.single()
                assertEquals("POST", submission.method)
                assertEquals("/api/v1/gov/anonymous-ballots", submission.uri.rawPath)
                // Only the media type: no token, account authentication, witness, cookie or correlation.
                assertEquals(mapOf("Content-Type" to listOf("application/x-norito")), submission.headers)
                assertContentEquals(envelope, submission.body)
                assertEquals(RequestReplayPolicy.ONE_SHOT, submission.replayPolicy)

                // A read of the same client keeps the configured listener credential but adds no
                // fresh canonical signature from the client-wide signer.
                executor.next = noritoResponse(byteArrayOf(1, 2, 3))
                client.getAnonymousBallotBoard(scope).join()
                val read = executor.requests.last()
                assertEquals(listOf("listener-secret"), read.headers["X-API-Token"])
                assertFalse(read.headers.keys.any {
                    it.equals(CanonicalRequestSigner.HEADER_NONCE, true) ||
                        it.equals(CanonicalRequestSigner.HEADER_TIMESTAMP_MS, true)
                })
            }
        }
        // No observer or telemetry hook ever saw a ballot request or response.
        assertEquals(0, observer.calls)
    }

    @Test
    fun theWireSubmissionHasOnlyTheMediaTypeAndTransportFraming() {
        MockWebServer().use { server ->
            server.enqueue(MockResponse().setResponseCode(202))
            server.start()
            AnonymousBallotToriiClientV1.builder()
                .baseUri(server.url("/").toUri())
                .setAllowLocalDevelopmentHttp(true)
                .defaultHeaders(configuredHeaders)
                .build()
                .use { client -> client.submitAnonymousBallot(envelope).get(10, TimeUnit.SECONDS) }
            val recorded = assertNotNull(server.takeRequest(10, TimeUnit.SECONDS))
            assertEquals("POST", recorded.method)
            assertEquals("/v1/gov/anonymous-ballots", recorded.path)
            assertEquals("application/x-norito", recorded.getHeader("Content-Type"))
            assertContentEquals(envelope, recorded.body.readByteArray())
            val names = recorded.headers.names().map { it.lowercase(Locale.ROOT) }.toSet()
            val transportFraming = setOf("host", "content-length", "connection", "accept-encoding", "user-agent")
            assertTrue((names - "content-type").all { it in transportFraming }, "unexpected headers $names")
            for (configured in configuredHeaders.keys) {
                assertFalse(configured.lowercase(Locale.ROOT) in names, configured)
            }
        }
    }

    @Test
    fun submissionValidatesTheEnvelopeAndEndpointBeforeDispatch() {
        val executor = StubExecutor(TransportResponse(202, ByteArray(0), "Accepted", emptyMap(), null, false))
        client(executor).use { client ->
            assertFailsWith<IllegalArgumentException> { client.submitAnonymousBallot(ByteArray(0)) }
            assertFailsWith<IllegalArgumentException> {
                client.submitAnonymousBallot(ByteArray(AnonymousBallotApiV1.ENVELOPE_MAX_BYTES + 1))
            }
            assertTrue(executor.requests.isEmpty())
            client.submitAnonymousBallot(ByteArray(AnonymousBallotApiV1.ENVELOPE_MAX_BYTES)).join()
            assertEquals(1, executor.requests.size)
        }
        for (url in listOf("http://torii.example", "http://8.8.8.8:8080")) {
            val refused = StubExecutor(TransportResponse(202, ByteArray(0), "", emptyMap(), null, false))
            client(refused, URI.create(url), allowLocalHttp = true).use { client ->
                assertFailsWith<IllegalArgumentException> { client.submitAnonymousBallot(envelope) }
            }
            assertTrue(refused.requests.isEmpty(), url)
        }
        val local = StubExecutor(TransportResponse(202, ByteArray(0), "", emptyMap(), null, false))
        client(local, URI.create("http://127.0.0.1:8080")).use { client ->
            assertFailsWith<IllegalArgumentException> { client.submitAnonymousBallot(envelope) }
        }
        client(local, URI.create("http://127.0.0.1:8080"), allowLocalHttp = true).use { client ->
            client.submitAnonymousBallot(envelope).join()
        }
        assertEquals(1, local.requests.size)
        assertFailsWith<IllegalArgumentException> {
            client(local, URI.create("https://torii.example/api?x=1"))
        }
        assertFailsWith<IllegalArgumentException> { client(local, URI.create("ftp://torii.example")) }
    }

    @Test
    fun submissionSucceedsOnlyOnAnEmpty202AndSurfacesStableRefusals() {
        val cases = listOf(
            Triple(400, "anonymous_ballot_malformed", null),
            Triple(400, "anonymous_ballot_proof_rejected", null),
            Triple(413, "anonymous_ballot_oversize", null),
            Triple(415, "anonymous_ballot_unsupported_media_type", null),
            Triple(429, "proof_rate_limited", 1L),
            Triple(503, "anonymous_ballot_ingress_unavailable", 7L),
        )
        for ((status, code, retryAfter) in cases) {
            val headers = LinkedHashMap<String, List<String>>()
            headers["Content-Type"] = listOf("application/json")
            if (retryAfter != null) headers["Retry-After"] = listOf(retryAfter.toString())
            val body = """{"code":"$code","message":"server text that must not be echoed"}"""
            val refusal = assertIs<AnonymousBallotToriiExceptionV1>(
                submitFailure(TransportResponse(status, body.toByteArray(), "", headers, null, false)),
            )
            assertEquals(status, refusal.status)
            assertEquals(code, refusal.code)
            assertEquals(retryAfter, refusal.retryAfterSeconds)
            assertNull(refusal.details)
            assertNull(refusal.rejectCode)
            assertEquals("anonymous ballot submission was refused with HTTP $status ($code)", refusal.message)
            assertIs<ToriiApiException>(refusal)
        }

        // A Norito error envelope (the negotiated format of a Norito client) yields the same code.
        val noritoRefusal = assertIs<AnonymousBallotToriiExceptionV1>(
            submitFailure(TransportResponse(400, noritoErrorEnvelope("anonymous_ballot_malformed"), "", emptyMap(), null, false)),
        )
        assertEquals("anonymous_ballot_malformed", noritoRefusal.code)

        // Unparseable, unstable or absent codes and HTTP-date Retry-After values are dropped.
        for ((body, retry) in listOf(
            "<html>proxy</html>" to "Wed, 21 Oct 2026 07:28:00 GMT",
            """{"code":"bad code!","message":"m"}""" to "-1",
            "" to "1, 2",
        )) {
            val refusal = assertIs<AnonymousBallotToriiExceptionV1>(
                submitFailure(TransportResponse(502, body.toByteArray(), "", mapOf("Retry-After" to listOf(retry)), null, false)),
            )
            assertEquals(502, refusal.status)
            assertNull(refusal.code)
            assertNull(refusal.retryAfterSeconds)
            assertEquals("anonymous ballot submission was refused with HTTP 502", refusal.message)
        }

        // 202 is the only success, and it carries no body; redirects are contract violations.
        for (response in listOf(
            TransportResponse(200, ByteArray(0), "", emptyMap(), null, false),
            TransportResponse(204, ByteArray(0), "", emptyMap(), null, false),
            TransportResponse(202, byteArrayOf(1), "", emptyMap(), null, false),
            TransportResponse(202, ByteArray(0), "", emptyMap(), URI.create("https://elsewhere.example/x"), false),
            TransportResponse(202, ByteArray(0), "", emptyMap(), null, true),
        )) {
            val violation = assertIs<ToriiProtocolException>(submitFailure(response))
            assertEquals(ToriiProtocolException.CODE, violation.code)
        }

        // A transport failure keeps its cause and never renders the ballot.
        val failure = submitFailure(null)
        assertEquals("anonymous ballot submission request failed", failure.message)
        assertEquals("connection reset", failure.cause?.message)
    }

    @Test
    fun boardAndAttemptReadsArePublicBoundedNoritoReads() {
        val record = ByteArray(4_096) { (it % 251).toByte() }
        val executor = StubExecutor(noritoResponse(record))
        client(executor, defaultHeaders = mapOf("X-API-Token" to "listener-secret")).use { client ->
            assertContentEquals(record, client.getAnonymousBallotBoard(scope).join())
            assertContentEquals(record, client.getAnonymousBallotBoard("cd".repeat(64)).join())
            assertContentEquals(record, client.getAnonymousParliamentAttempt(attemptId).join())
            assertContentEquals(record, client.getAnonymousParliamentAttempt("5a".repeat(32)).join())
        }
        val (board, boardHex, attempt, attemptHex) = executor.requests
        for (request in executor.requests) {
            assertEquals("GET", request.method)
            assertContentEquals(ByteArray(0), request.body)
            assertEquals(listOf("application/x-norito"), request.headers["Accept"])
            assertEquals(listOf("listener-secret"), request.headers["X-API-Token"])
            assertEquals(RequestReplayPolicy.RETRY_SAFE, request.replayPolicy)
            assertFalse(request.headers.keys.any { it.startsWith("X-Iroha-", ignoreCase = true) })
        }
        assertEquals("/api/v1/gov/anonymous-ballots/boards/" + "cd".repeat(64), board.uri.rawPath)
        assertEquals(board.uri, boardHex.uri)
        assertEquals("/api/v1/gov/anonymous-ballots/attempts/" + "5a".repeat(32), attempt.uri.rawPath)
        assertEquals(attempt.uri, attemptHex.uri)
        assertEquals(1_048_576L, board.maximumResponseBytes)
        assertEquals(16_777_216L, attempt.maximumResponseBytes)

        // Records at their bound are returned; one byte above fails.
        val atBound = StubExecutor(noritoResponse(ByteArray(AnonymousBallotApiV1.BOARD_MAX_BYTES)))
        client(atBound).use { it.getAnonymousBallotBoard(scope).join() }
        assertIs<ToriiProtocolException>(readFailure(noritoResponse(ByteArray(AnonymousBallotApiV1.BOARD_MAX_BYTES + 1))))
        val attemptAbove = StubExecutor(noritoResponse(ByteArray(AnonymousBallotApiV1.ATTEMPT_MAX_BYTES + 1)))
        client(attemptAbove).use { client ->
            val error = assertFailsWith<CompletionException> { client.getAnonymousParliamentAttempt(attemptId).join() }
            assertIs<ToriiProtocolException>(error.cause)
        }
    }

    @Test
    fun readsRejectContractViolationsAndSurfaceAbsence() {
        val notFound = assertIs<AnonymousBallotToriiExceptionV1>(readFailure(TransportResponse(
            404,
            """{"code":"anonymous_ballot_board_not_found","message":"no sealed board"}""".toByteArray(),
            "",
            mapOf("Content-Type" to listOf("application/json")),
            null,
            false,
        )))
        assertEquals(404, notFound.status)
        assertEquals("anonymous_ballot_board_not_found", notFound.code)
        assertEquals("anonymous-ballot board read was refused with HTTP 404 (anonymous_ballot_board_not_found)", notFound.message)

        val norito = mapOf("Content-Type" to listOf("application/x-norito"))
        for (response in listOf(
            TransportResponse(200, byteArrayOf(1), "", mapOf("Content-Type" to listOf("application/json")), null, false),
            TransportResponse(200, byteArrayOf(1), "", mapOf("Content-Type" to listOf("application/x-norito; charset=utf-8")), null, false),
            TransportResponse(200, byteArrayOf(1), "", emptyMap(), null, false),
            TransportResponse(200, ByteArray(0), "", norito, null, false),
            TransportResponse(200, byteArrayOf(1), "", norito + ("Content-Encoding" to listOf("gzip")), null, false),
            TransportResponse(200, byteArrayOf(1), "", norito, null, true),
            TransportResponse(204, byteArrayOf(1), "", norito, null, false),
        )) {
            assertIs<ToriiProtocolException>(readFailure(response))
        }
        val identity = StubExecutor(TransportResponse(
            200, byteArrayOf(9), "", norito + ("Content-Encoding" to listOf("identity")), null, false,
        ))
        client(identity).use { assertContentEquals(byteArrayOf(9), it.getAnonymousBallotBoard(scope).join()) }
        // A response that names the exact request URL as its provenance is accepted.
        val exact = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                CompletableFuture.completedFuture(TransportResponse(200, byteArrayOf(8), "", norito, request.uri, false))
        }
        client(exact).use { assertContentEquals(byteArrayOf(8), it.getAnonymousBallotBoard(scope).join()) }

        // Reads own Accept, and a credentialed read refuses public plain HTTP before dispatch.
        val unused = StubExecutor(noritoResponse(byteArrayOf(1)))
        client(unused, defaultHeaders = mapOf("accept" to "application/json")).use { client ->
            assertFailsWith<IllegalArgumentException> { client.getAnonymousBallotBoard(scope) }
        }
        client(unused, URI.create("http://torii.example"), defaultHeaders = mapOf("X-API-Token" to "t")).use { client ->
            assertFailsWith<IllegalArgumentException> { client.getAnonymousParliamentAttempt(attemptId) }
        }
        assertTrue(unused.requests.isEmpty())
        // A public read without credentials follows the ordinary public-route transport policy.
        client(unused, URI.create("http://torii.example")).use { client ->
            client.getAnonymousBallotBoard(scope).join()
        }
        assertEquals(1, unused.requests.size)
    }

    @Test
    fun closingTheClientCancelsItsCallsWithoutClosingTheBorrowedExecutor() {
        val pending = CompletableFuture<TransportResponse>()
        val executor = object : HttpTransportExecutor {
            var closed = false
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> = pending
            override fun close() { closed = true }
        }
        val client = AnonymousBallotToriiClientV1.builder().executor(executor).baseUri(base).build()
        val call = client.submitAnonymousBallot(envelope)
        client.close()
        assertFailsWith<CompletionException> { call.join() }
        assertFalse(executor.closed)
        assertFailsWith<CompletionException> { client.getAnonymousBallotBoard(scope).join() }
    }

    private fun client(
        executor: HttpTransportExecutor,
        baseUri: URI = base,
        allowLocalHttp: Boolean = false,
        defaultHeaders: Map<String, String> = emptyMap(),
    ): AnonymousBallotToriiClientV1 = AnonymousBallotToriiClientV1.builder()
        .executor(executor)
        .baseUri(baseUri)
        .setAllowLocalDevelopmentHttp(allowLocalHttp)
        .defaultHeaders(defaultHeaders)
        .build()

    /** The failure of one submission answered with [response] (`null`: a transport failure). */
    private fun submitFailure(response: TransportResponse?): Throwable {
        val executor = StubExecutor(response)
        val error = client(executor).use { client ->
            assertFailsWith<CompletionException> { client.submitAnonymousBallot(envelope).join() }
        }
        val cause = assertNotNull(error.cause)
        val rendered = cause.toString() + cause.message
        assertFalse(rendered.contains("server text"), rendered)
        assertFalse(rendered.contains(AnonymousBallotApiV1.lowerHex(envelope.copyOf(16))), rendered)
        return cause
    }

    private fun readFailure(response: TransportResponse): Throwable {
        val error = client(StubExecutor(response)).use { client ->
            assertFailsWith<CompletionException> { client.getAnonymousBallotBoard(scope).join() }
        }
        return assertNotNull(error.cause)
    }

    private fun noritoResponse(body: ByteArray): TransportResponse =
        TransportResponse(
            200,
            body,
            "OK",
            mapOf("Content-Type" to listOf("application/x-norito"), "Cache-Control" to listOf("no-store")),
            null,
            false,
        )

    /** A Norito-framed `ErrorEnvelope { code, message, details: None }` as Torii renders it. */
    private fun noritoErrorEnvelope(code: String): ByteArray {
        val optionalString = NoritoAdapters.option(NoritoAdapters.stringAdapter())
        val detailsAdapter = NoritoAdapters.struct(
            listOf(
                NoritoAdapters.field("reject_code", optionalString),
                NoritoAdapters.field("queue", optionalString),
                NoritoAdapters.field("retry_after_seconds", NoritoAdapters.option(NoritoAdapters.uint(64))),
                NoritoAdapters.field("endpoint", optionalString),
                NoritoAdapters.field("axt", optionalString),
            ),
        )
        val envelopeAdapter = NoritoAdapters.struct(
            listOf(
                NoritoAdapters.field("code", NoritoAdapters.stringAdapter()),
                NoritoAdapters.field("message", NoritoAdapters.stringAdapter()),
                NoritoAdapters.field("details", NoritoAdapters.option(detailsAdapter)),
            ),
        )
        val envelope = linkedMapOf<String, Any>(
            "code" to code,
            "message" to "server text that must not be echoed",
            "details" to Optional.empty<Any>(),
        )
        return NoritoCodec.encode(envelope as Any, "iroha_torii_shared::ErrorEnvelope", envelopeAdapter)
    }

    private class StubExecutor(var next: TransportResponse?) : HttpTransportExecutor {
        val requests = mutableListOf<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val response = next
                ?: return CompletableFuture<TransportResponse>().apply {
                    completeExceptionally(java.io.IOException("connection reset"))
                }
            return CompletableFuture.completedFuture(response)
        }
    }

    private class RecordingObserver : ClientObserver {
        var calls = 0
        override fun onRequest(request: TransportRequest) { calls += 1 }
        override fun onResponse(request: TransportRequest, response: ClientResponse) { calls += 1 }
        override fun onFailure(request: TransportRequest, error: Throwable) { calls += 1 }
    }
}
