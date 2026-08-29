package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import org.hyperledger.iroha.sdk.validationfee.ValidationFeeHijiriQuoteCodec
import org.hyperledger.iroha.sdk.validationfee.ValidationFeeHijiriQuoteProjectionParser
import org.hyperledger.iroha.sdk.validationfee.ValidationFeeHijiriQuoteRequestV1
import org.hyperledger.iroha.sdk.validationfee.ValidationFeeHijiriQuoteV1
import org.hyperledger.iroha.sdk.validationfee.VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE_V1
import org.hyperledger.iroha.sdk.validationfee.VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA_V1

class HttpClientTransportHijiriQuoteTest {
    private val accountId =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x61), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
    private val signatoryAccountId =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x62), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    @Test
    fun `quote posts and verifies the same exact account-signed Norito request`() {
        val responseNorito = byteArrayOf(9, 8, 7, 6)
        val executor = ExactResponseExecutor(responseNorito)
        val codec = CapturingCodec(byteArrayOf(1, 3, 3, 7), projectionJson())
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            signatoryAccountId,
            keyPair.private,
            1_700_000_000_123L,
            "hijiri-quote-1",
        )
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
                .build(),
        )
        val request = ValidationFeeHijiriQuoteRequestV1(accountId, 2)

        val quote = transport.postValidationFeeHijiriQuote(request, auth, codec).join()

        assertEquals(2, quote.qualifyingTransferCount)
        assertContentEquals(responseNorito, codec.verifiedResponse)
        assertContentEquals(codec.encodedRequest, codec.verifiedRequest)
        val sent = assertNotNull(executor.lastRequest)
        assertEquals("POST", sent.method)
        assertEquals(
            "https://torii.example/api/v1/validation-fee/hijiri/quote",
            sent.uri.toString(),
        )
        assertContentEquals(codec.encodedRequest, sent.body)
        assertEquals(listOf("application/x-norito"), sent.headers["Content-Type"])
        assertEquals(listOf("application/x-norito"), sent.headers["Accept"])
        assertEquals(listOf("identity"), sent.headers["Accept-Encoding"])
        assertEquals(listOf("no-store"), sent.headers["Cache-Control"])
        assertEquals(64L * 1024L, sent.maximumResponseBytes)
        assertEquals(RequestReplayPolicy.ONE_SHOT, sent.replayPolicy)
        assertEquals(
            "1700000000123",
            sent.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS]?.single(),
        )
        assertEquals(
            "hijiri-quote-1",
            sent.headers[CanonicalRequestSigner.HEADER_NONCE]?.single(),
        )
        assertEquals(
            AccountAddress.parseEncodedIgnoringCurveSupport(signatoryAccountId, null).canonicalHex(),
            sent.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.single(),
        )
        val signature = Base64.getDecoder().decode(
            assertNotNull(sent.headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.single()),
        )
        val signedMessage = CanonicalRequestSigner.canonicalRequestSignatureMessage(
            TestNetworkIds.canonical(),
            sent.method,
            sent.uri,
            sent.body,
            1_700_000_000_123L,
            "hijiri-quote-1",
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(keyPair.public)
        verifier.update(signedMessage)
        assertTrue(verifier.verify(signature))
    }

    @Test
    fun `quote rejects encoded request drift and hostile success metadata`() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(accountId, keyPair.private)
        val request = ValidationFeeHijiriQuoteRequestV1(accountId, 2)
        val oversizedCodec = CapturingCodec(ByteArray(4 * 1024 + 1), projectionJson())
        val neverExecutor = ExactResponseExecutor(byteArrayOf(1))
        val transport = transport(neverExecutor)

        assertFailsWith<IllegalArgumentException> {
            transport.postValidationFeeHijiriQuote(request, auth, oversizedCodec)
        }
        assertEquals(0, neverExecutor.requestCount)

        val insecureTransport = HttpClientTransport.withExecutor(
            neverExecutor,
            ClientConfig.builder()
                .setBaseUri(URI.create("http://torii.example"))
                .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
                .build(),
        )
        assertFailsWith<IllegalStateException> {
            insecureTransport.postValidationFeeHijiriQuote(
                request,
                auth,
                CapturingCodec(byteArrayOf(1), projectionJson()),
            )
        }
        assertEquals(0, neverExecutor.requestCount)

        val rejectExecutor = ExactResponseExecutor(
            byteArrayOf(1),
            mapOf(
                "Content-Type" to listOf("application/x-norito"),
                "Cache-Control" to listOf("private, no-store"),
                "x-iroha-reject-code" to listOf("validation_fee_state_inconsistent"),
            ),
        )
        val rejection = assertFailsWith<CompletionException> {
            transport(rejectExecutor)
                .postValidationFeeHijiriQuote(
                    request,
                    auth,
                    CapturingCodec(byteArrayOf(1), projectionJson()),
                ).join()
        }
        assertTrue(rejection.cause?.message.orEmpty().contains("x-iroha-reject-code"))

        val emptyRejectExecutor = ExactResponseExecutor(
            byteArrayOf(1),
            mapOf(
                "Content-Type" to listOf("application/x-norito"),
                "Cache-Control" to listOf("private, no-store"),
                "x-iroha-reject-code" to emptyList(),
            ),
        )
        val emptyRejection = assertFailsWith<CompletionException> {
            transport(emptyRejectExecutor)
                .postValidationFeeHijiriQuote(
                    request,
                    auth,
                    CapturingCodec(byteArrayOf(1), projectionJson()),
                ).join()
        }
        assertTrue(emptyRejection.cause?.message.orEmpty().contains("x-iroha-reject-code"))

        val encodedExecutor = ExactResponseExecutor(
            byteArrayOf(1),
            mapOf(
                "Content-Type" to listOf("application/x-norito"),
                "Content-Encoding" to listOf("gzip"),
                "Cache-Control" to listOf("private, no-store"),
            ),
        )
        val compressed = assertFailsWith<CompletionException> {
            transport(encodedExecutor)
                .postValidationFeeHijiriQuote(
                    request,
                    auth,
                    CapturingCodec(byteArrayOf(1), projectionJson()),
                ).join()
        }
        assertTrue(compressed.cause?.message.orEmpty().contains("absent or identity"))

        val cacheable = assertFailsWith<CompletionException> {
            transport(
                ExactResponseExecutor(
                    byteArrayOf(1),
                    mapOf("Content-Type" to listOf("application/x-norito")),
                ),
            ).postValidationFeeHijiriQuote(
                request,
                auth,
                CapturingCodec(byteArrayOf(1), projectionJson()),
            ).join()
        }
        assertTrue(cacheable.cause?.message.orEmpty().contains("private and no-store"))

        val contradictoryCache = assertFailsWith<CompletionException> {
            transport(
                ExactResponseExecutor(
                    byteArrayOf(1),
                    mapOf(
                        "Content-Type" to listOf("application/x-norito"),
                        "Cache-Control" to listOf("private, no-store, public"),
                    ),
                ),
            ).postValidationFeeHijiriQuote(
                request,
                auth,
                CapturingCodec(byteArrayOf(1), projectionJson()),
            ).join()
        }
        assertTrue(contradictoryCache.cause?.message.orEmpty().contains("private and no-store"))

        for (parameterizedPublic in listOf("public=max-age", "PUBLIC = \"Set-Cookie\"")) {
            val parameterizedPublicCache = assertFailsWith<CompletionException> {
                transport(
                    ExactResponseExecutor(
                        byteArrayOf(1),
                        mapOf(
                            "Content-Type" to listOf("application/x-norito"),
                            "Cache-Control" to
                                listOf("private, no-store, $parameterizedPublic"),
                        ),
                    ),
                ).postValidationFeeHijiriQuote(
                    request,
                    auth,
                    CapturingCodec(byteArrayOf(1), projectionJson()),
                ).join()
            }
            assertTrue(
                parameterizedPublicCache.cause?.message.orEmpty()
                    .contains("private and no-store"),
            )
        }

        val hostileProvenance = listOf(
            ExactResponseExecutor(
                byteArrayOf(1),
                includeProvenance = false,
            ),
            ExactResponseExecutor(
                byteArrayOf(1),
                finalUriOverride = URI.create("https://redirect.example/hijiri/quote"),
            ),
            ExactResponseExecutor(
                byteArrayOf(1),
                redirected = true,
            ),
        )
        for (hostileExecutor in hostileProvenance) {
            val hostileCodec = CapturingCodec(byteArrayOf(1), projectionJson())
            val provenanceFailure = assertFailsWith<CompletionException> {
                transport(hostileExecutor)
                    .postValidationFeeHijiriQuote(request, auth, hostileCodec)
                    .join()
            }
            assertTrue(
                provenanceFailure.cause?.message.orEmpty()
                    .contains("exact signed URL without redirects"),
            )
            assertEquals(null, hostileCodec.verifiedResponse)
        }

        val exactErrorHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
            "Content-Encoding" to listOf("identity"),
            "Cache-Control" to listOf("private, no-store"),
        )
        val hostileErrors = listOf(
            ExactResponseExecutor(
                byteArrayOf(1),
                mapOf("Cache-Control" to listOf("private, no-store")),
                statusCode = 503,
            ) to "Content-Type",
            ExactResponseExecutor(
                byteArrayOf(1),
                mapOf(
                    "Content-Type" to listOf("application/x-norito"),
                    "Content-Encoding" to listOf("gzip"),
                    "Cache-Control" to listOf("private, no-store"),
                ),
                statusCode = 503,
            ) to "absent or identity",
            ExactResponseExecutor(
                byteArrayOf(1),
                mapOf("Content-Type" to listOf("application/x-norito")),
                statusCode = 503,
            ) to "private and no-store",
            ExactResponseExecutor(
                ByteArray(64 * 1024 + 1),
                exactErrorHeaders,
                statusCode = 503,
            ) to "response exceeds",
            ExactResponseExecutor(
                byteArrayOf(1),
                exactErrorHeaders + ("Content-Length" to listOf("2")),
                statusCode = 503,
            ) to "Content-Length",
        )
        for ((hostileError, expectedMessage) in hostileErrors) {
            val hostileCodec = CapturingCodec(byteArrayOf(1), projectionJson())
            val failure = assertFailsWith<CompletionException> {
                transport(hostileError)
                    .postValidationFeeHijiriQuote(request, auth, hostileCodec)
                    .join()
            }
            assertTrue(failure.cause?.message.orEmpty().contains(expectedMessage))
            assertEquals(null, hostileCodec.verifiedResponse)
        }
    }

    @Test
    fun `quote parses cache directives without trusting quoted comma decoys`() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(accountId, keyPair.private)
        val request = ValidationFeeHijiriQuoteRequestV1(accountId, 2)
        val invalidCacheControls = listOf(
            "private, x=\"a,no-store,b\"",
            "no-store, x=\"a,private,b\"",
            "private=\"Set-Cookie\", no-store",
            "private, no-store=extension",
            "private, no-store, x=\"unterminated",
            "private, no-store, x=\"dangling\\",
            "private, no-store, x=\"closed\"junk",
            "private, no-store, x=bad\\escape",
        )

        for (cacheControl in invalidCacheControls) {
            val codec = CapturingCodec(byteArrayOf(1), projectionJson())
            val failure = assertFailsWith<CompletionException> {
                transport(
                    ExactResponseExecutor(
                        byteArrayOf(1),
                        mapOf(
                            "Content-Type" to listOf("application/x-norito"),
                            "Cache-Control" to listOf(cacheControl),
                        ),
                    ),
                ).postValidationFeeHijiriQuote(request, auth, codec).join()
            }
            assertTrue(failure.cause?.message.orEmpty().contains("private and no-store"))
            assertEquals(null, codec.verifiedResponse)
        }

        val quotedExtensionCodec = CapturingCodec(byteArrayOf(1), projectionJson())
        val quote = transport(
            ExactResponseExecutor(
                byteArrayOf(1),
                mapOf(
                    "Content-Type" to listOf("application/x-norito"),
                    "Cache-Control" to
                        listOf("private, no-store, extension=\"a,public,b\""),
                ),
            ),
        ).postValidationFeeHijiriQuote(request, auth, quotedExtensionCodec).join()
        assertEquals(request.accountId, quote.accountId)
        assertContentEquals(byteArrayOf(1), quotedExtensionCodec.verifiedResponse)
    }

    private fun transport(executor: HttpTransportExecutor): HttpClientTransport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
                .build(),
        )

    private inner class CapturingCodec(
        encoded: ByteArray,
        private val projection: ByteArray,
    ) : ValidationFeeHijiriQuoteCodec {
        val encodedRequest = encoded.copyOf()
        var verifiedResponse: ByteArray? = null
        var verifiedRequest: ByteArray? = null

        override fun encode(request: ValidationFeeHijiriQuoteRequestV1): ByteArray =
            encodedRequest.copyOf()

        override fun verify(
            responseNorito: ByteArray,
            requestNorito: ByteArray,
        ): ValidationFeeHijiriQuoteV1 {
            verifiedResponse = responseNorito.copyOf()
            verifiedRequest = requestNorito.copyOf()
            return ValidationFeeHijiriQuoteProjectionParser.parse(projection)
        }
    }

    private class ExactResponseExecutor(
        private val responseBody: ByteArray,
        private val responseHeaders: Map<String, List<String>> =
            mapOf(
                "Content-Type" to listOf("application/x-norito"),
                "Content-Encoding" to listOf("identity"),
                "Cache-Control" to listOf("private, no-store"),
            ),
        private val statusCode: Int = 200,
        private val includeProvenance: Boolean = true,
        private val finalUriOverride: URI? = null,
        private val redirected: Boolean = false,
    ) : HttpTransportExecutor {
        var requestCount: Int = 0
        var lastRequest: TransportRequest? = null

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requestCount += 1
            lastRequest = request
            if (!includeProvenance) {
                return CompletableFuture.completedFuture(
                    TransportResponse(
                        statusCode,
                        responseBody,
                        "ok",
                        responseHeaders,
                        null,
                        false,
                    ),
                )
            }
            return CompletableFuture.completedFuture(
                TransportResponse(
                    statusCode,
                    responseBody,
                    "ok",
                    responseHeaders,
                    finalUriOverride ?: request.uri,
                    redirected,
                ),
            )
        }
    }

    private fun projectionJson(): ByteArray =
        """
        {
          "schema":"$VALIDATION_FEE_HIJIRI_QUOTE_SCHEMA_V1",
          "version":1,
          "assurance":"$VALIDATION_FEE_HIJIRI_QUOTE_ASSURANCE_V1",
          "evaluatedStateHeight":"42",
          "quotedExecutionHeight":"43",
          "accountId":"$accountId",
          "activePolicyVersion":"1",
          "activePolicyHash":"${"03".repeat(32)}",
          "feeAssetDefinitionId":"asset",
          "treasuryAccountId":"$accountId",
          "feeScale":2,
          "hijiriParametersVersion":1,
          "hijiriParametersRevision":"1",
          "hijiriParametersDigest":"${"05".repeat(32)}",
          "defaultAccountRiskQ16":0,
          "effectiveAccountRiskQ16":0,
          "accountRiskRevision":null,
          "accountRiskDigest":null,
          "feeMultiplierQ16":65536,
          "hijiriFeeQuoteHash":"${"07".repeat(32)}",
          "basePerTransferFeeMinorUnits":"10",
          "adjustedPerTransferFeeMinorUnits":"10",
          "qualifyingTransferCount":2,
          "aggregateBaseFeeMinorUnits":"20",
          "aggregateAdjustedFeeMinorUnits":"20"
        }
        """.trimIndent()
            .toByteArray(StandardCharsets.UTF_8)
}
