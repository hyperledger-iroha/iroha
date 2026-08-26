package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/** Focused transaction-submission wire and capability-probe contract tests. */
class HttpClientTransportSubmissionContractTest {
    @Test
    fun submitTransactionJsonUsesJsonIngressWithConfiguredAccept() {
        val executor = CapturingExecutor()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://127.0.0.1:8080"))
                .setWireFormatPreference(WireFormatPreference.JSON_PREFERRED)
                .build(),
        )
        val body = """{"version":1,"content":{}}""".toByteArray(StandardCharsets.UTF_8)

        val response = transport.submitTransactionJson(body).join()

        assertEquals(202, response.statusCode)
        val request = executor.lastRequest
        assertEquals("POST", request.method)
        assertEquals("https://127.0.0.1:8080/v1/pipeline/transactions", request.uri.toString())
        assertEquals("application/json", request.headers["Content-Type"]?.first())
        assertEquals(WireFormatPreference.JSON_PREFERRED.acceptHeader(), request.headers["Accept"]?.first())
        assertTrue(body.contentEquals(request.body))
        assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy)
    }

    @Test
    fun binaryTransactionEntrypointRejectsNon202WithoutRedispatch() {
        val body = byteArrayOf(1, 2, 3)
        for (statusCode in listOf(200, 201, 204)) {
            val executor = CapturingExecutor(submitStatus = statusCode)
            val transport = HttpClientTransport.withExecutor(
                executor = executor,
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://127.0.0.1:8080"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException> {
                transport.submitTransactionEntrypoint(body).join()
            }

            assertTrue(error.cause?.message?.contains("status $statusCode") == true)
            assertEquals(2, executor.callCount)
            assertEquals(1, executor.submissionCallCount)
            assertBinaryEntrypointRequest(executor.lastRequest, body)
        }
    }

    @Test
    fun binaryTransactionEntrypointAccepts202WithOneDispatch() {
        val body = byteArrayOf(1, 2, 3)
        val entrypointHash = "1".repeat(64)
        val executor = CapturingExecutor(
            submitStatus = 202,
            entrypointHash = entrypointHash,
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://127.0.0.1:8080"))
                .build(),
        )

        val response = transport.submitTransactionEntrypoint(body).join()

        assertEquals(202, response.statusCode)
        assertEquals(entrypointHash, response.transactionHashHex)
        assertEquals(2, executor.callCount)
        assertEquals(1, executor.submissionCallCount)
        assertBinaryEntrypointRequest(executor.lastRequest, body)
    }

    private fun assertBinaryEntrypointRequest(request: TransportRequest, expectedBody: ByteArray) {
        assertEquals("POST", request.method)
        assertEquals(
            "https://127.0.0.1:8080/v1/pipeline/transaction-entrypoints",
            request.uri.toString(),
        )
        assertEquals("application/x-norito", request.headers["Content-Type"]?.first())
        assertTrue(expectedBody.contentEquals(request.body))
        assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy)
    }

    @Test
    fun signedTransactionSubmissionAcceptsOnlyHttp202() {
        for (statusCode in listOf(200, 201, 204, 400)) {
            val executor = CapturingExecutor(
                submitStatus = statusCode,
                submitBody = "rejected-$statusCode".toByteArray(StandardCharsets.UTF_8),
                rejectCode = "submit_$statusCode",
            )
            val transport = HttpClientTransport.withExecutor(
                executor = executor,
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://127.0.0.1:8080"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException> {
                transport.submitTransaction(sampleSignedTransaction(statusCode)).join()
            }
            val rejected = assertIs<TransactionSubmissionHttpException>(error.cause)

            assertEquals(statusCode, rejected.statusCode)
            assertEquals("submit_$statusCode", rejected.rejectCode)
            assertEquals("rejected-$statusCode", rejected.responseBody)
            assertEquals(2, executor.callCount)
            assertEquals(RequestReplayPolicy.ONE_SHOT, executor.lastRequest.replayPolicy)
        }
    }

    @Test
    fun ambiguousSubmissionRetainsHttpEvidenceWithoutReplay() {
        for ((statusCode, rejectCode) in listOf(
            409 to "already_enqueued",
            503 to "route_unavailable",
        )) {
            val responseBody = "ambiguous-$statusCode"
            val executor = CapturingExecutor(
                submitStatus = statusCode,
                submitBody = responseBody.toByteArray(StandardCharsets.UTF_8),
                rejectCode = rejectCode,
            )
            val transport = HttpClientTransport.withExecutor(
                executor = executor,
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://127.0.0.1:8080"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException> {
                transport.submitTransaction(sampleSignedTransaction(statusCode)).join()
            }
            val ambiguous = assertIs<AmbiguousTransactionSubmissionException>(error.cause)

            assertEquals(statusCode, ambiguous.statusCode)
            assertEquals(rejectCode, ambiguous.rejectCode)
            assertEquals(responseBody, ambiguous.responseBody)
            assertEquals(2, executor.callCount)
            assertEquals(1, executor.submissionCallCount)
            assertEquals(RequestReplayPolicy.ONE_SHOT, executor.lastRequest.replayPolicy)
        }
    }

    @Test
    fun nonSuccessClassifiesBeforeReceiptHashValidation() {
        for (statusCode in listOf(400, 503)) {
            val transaction = sampleSignedTransaction(statusCode + 1)
            val expectedHash = SignedTransactionHasher.hashHex(transaction)
            val executor = CapturingExecutor(
                submitStatus = statusCode,
                submitBody = "evidence-$statusCode".toByteArray(StandardCharsets.UTF_8),
                rejectCode = "submit_$statusCode",
                entrypointHash = "malformed-receipt-hash",
            )
            val transport = HttpClientTransport.withExecutor(
                executor = executor,
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://127.0.0.1:8080"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException> {
                transport.submitTransaction(transaction).join()
            }
            val submissionError = error.cause
            when (statusCode) {
                400 -> assertIs<TransactionSubmissionHttpException>(submissionError).also {
                    assertEquals(expectedHash, it.hashHex)
                    assertEquals("submit_400", it.rejectCode)
                    assertEquals("evidence-400", it.responseBody)
                }
                503 -> assertIs<AmbiguousTransactionSubmissionException>(submissionError).also {
                    assertEquals(expectedHash, it.hashHex)
                    assertEquals("submit_503", it.rejectCode)
                    assertEquals("evidence-503", it.responseBody)
                }
            }
            assertEquals(2, executor.callCount)
            assertEquals(RequestReplayPolicy.ONE_SHOT, executor.lastRequest.replayPolicy)
        }
    }

    @Test
    fun replayPolicyIsSafeOnlyForUnsignedBodylessReads() {
        val unsignedRead = TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("https://127.0.0.1:8080/status"))
            .build()
        val signedQuery = TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("https://127.0.0.1:8080/v1/query"))
            .addHeader("X-Iroha-Signature", "signature")
            .addHeader("X-Iroha-Nonce", "nonce")
            .build()

        assertEquals(RequestReplayPolicy.RETRY_SAFE, unsignedRead.replayPolicy)
        assertEquals(RequestReplayPolicy.ONE_SHOT, signedQuery.replayPolicy)
    }

    @Test
    fun canonicalAuthRedirectStatusAndNetworkFailuresAreOneShot() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_717_171_717_000L,
            "canonical-one-shot-nonce",
        )
        for (status in listOf(307, 308, 503)) {
            assertCanonicalAliasFailsOnce(auth, OutcomeExecutor(status = status))
        }
        assertCanonicalAliasFailsOnce(
            auth,
            OutcomeExecutor(failure = RuntimeException("ambiguous network failure")),
        )
    }

    private fun assertCanonicalAliasFailsOnce(
        auth: ToriiCanonicalRequestAuth,
        executor: OutcomeExecutor,
    ) {
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://127.0.0.1:8080"))
                .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
                .build(),
        )

        assertFailsWith<CompletionException> {
            transport.resolveAccountAlias("merchant@private", auth).join()
        }
        assertEquals(1, executor.callCount)
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.lastRequest.replayPolicy)
    }

    private fun sampleSignedTransaction(seed: Int): SignedTransaction {
        val codec = NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val authority = AccountAddress
            .fromAccount(TestEd25519Keys.publicKey(0x37), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val payload = TransactionPayload(
            networkId = TestNetworkIds.fromSeed(seed.toLong()),
            authority = authority,
            creationTimeMs = 1_700_000_000_000L + seed,
            executable = Executable.instructions(emptyList()),
            timeToLiveMs = 5_000L,
            nonce = seed.toLong() + 1L,
            feePayment = FeePaymentIntent.authority(emptyList(), 1L),
            admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
            metadata = emptyMap(),
        )
        return SignedTransaction(
            codec.encodeTransaction(payload),
            ByteArray(64) { (seed + 1).toByte() },
            TestEd25519Keys.publicKey(seed + 2),
            codec.schemaName(),
        )
    }

    private class CapturingExecutor(
        private val submitStatus: Int = 202,
        private val submitBody: ByteArray = byteArrayOf(),
        private val rejectCode: String? = null,
        private val entrypointHash: String? = null,
    ) : HttpTransportExecutor {
        var callCount = 0
        var submissionCallCount = 0
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            callCount += 1
            lastRequest = request
            if (request.uri.path.endsWith("/v1/node/capabilities")) {
                return CompletableFuture.completedFuture(compatibleCapabilitiesResponse())
            }
            submissionCallCount += 1
            val response = TransportResponse.builder()
                .setStatusCode(submitStatus)
                .setBody(submitBody)
            if (rejectCode != null) {
                response.addHeader("x-iroha-reject-code", rejectCode)
            }
            if (entrypointHash != null) {
                response.addHeader("x-iroha-entrypoint-hash", entrypointHash)
            }
            return CompletableFuture.completedFuture(response.build())
        }
    }

    private class OutcomeExecutor(
        private val status: Int? = null,
        private val failure: RuntimeException? = null,
    ) : HttpTransportExecutor {
        var callCount = 0
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            callCount += 1
            lastRequest = request
            val future = CompletableFuture<TransportResponse>()
            if (failure != null) {
                future.completeExceptionally(failure)
            } else {
                future.complete(
                    TransportResponse.builder()
                        .setStatusCode(requireNotNull(status))
                        .setBody(byteArrayOf())
                        .build(),
                )
            }
            return future
        }
    }

    private companion object {
        fun compatibleCapabilitiesResponse(): TransportResponse =
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(
                    (
                        "{\"data_model_version\":4,\"signed_transaction_schema_hash_hex\":" +
                            "\"7ab5ff9c572efb316deac478f19209c5\"}"
                        ).toByteArray(StandardCharsets.UTF_8),
                )
                .build()
    }
}
