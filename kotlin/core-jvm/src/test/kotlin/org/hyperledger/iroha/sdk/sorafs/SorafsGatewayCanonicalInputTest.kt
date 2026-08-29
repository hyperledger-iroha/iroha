package org.hyperledger.iroha.sdk.sorafs

import java.io.InputStream
import java.net.URI
import java.time.Duration
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.atomic.AtomicBoolean
import java.util.stream.Stream
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportStreamResponse
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.junit.jupiter.api.Assertions.assertEquals
import org.junit.jupiter.api.Assertions.assertNull
import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Test
import org.junit.jupiter.params.ParameterizedTest
import org.junit.jupiter.params.provider.MethodSource

private const val MANIFEST_ID_HEX =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
private const val PROVIDER_ID_HEX =
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
private val GATEWAY_PUBLIC_KEY_HEX = TestEd25519Keys.publicKeyHex(0x2B)
private val ED25519_IDENTITY_KEY_HEX = "01" + "00".repeat(31)
private const val MANIFEST_CID_HEX =
    "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
private const val CHUNKER_HANDLE = "sorafs.sf1@1.0.0"
private const val STREAM_TOKEN_BASE64 = "c3RyZWFtLXRva2Vu"

class SorafsGatewayCanonicalInputTest {
    @Test
    fun policyLabelParsersAcceptOnlyExactV1Labels() {
        assertEquals(TransportPolicy.SORANET_FIRST, TransportPolicy.fromLabel("soranet-first"))
        assertEquals(TransportPolicy.SORANET_STRICT, TransportPolicy.fromLabel("soranet-strict"))
        assertEquals(TransportPolicy.DIRECT_ONLY, TransportPolicy.fromLabel("direct-only"))
        assertEquals(AnonymityPolicy.ANON_GUARD_PQ, AnonymityPolicy.fromLabel("anon-guard-pq"))
        assertEquals(
            AnonymityPolicy.ANON_MAJORITY_PQ,
            AnonymityPolicy.fromLabel("anon-majority-pq"),
        )
        assertEquals(AnonymityPolicy.ANON_STRICT_PQ, AnonymityPolicy.fromLabel("anon-strict-pq"))
        assertEquals(WriteModeHint.READ_ONLY, WriteModeHint.fromLabel("read-only"))
        assertEquals(WriteModeHint.UPLOAD_PQ_ONLY, WriteModeHint.fromLabel("upload-pq-only"))

        listOf(
            "soranet_first",
            "soranet-only",
            "SORANET-FIRST",
            " soranet-first",
            "soranet-first ",
        ).forEach { assertNull(TransportPolicy.fromLabel(it), "accepted transport alias '$it'") }
        listOf(
            "anon_guard_pq",
            "stage-a",
            "stage_b",
            "stagec",
            "ANON-GUARD-PQ",
            " anon-guard-pq",
            "anon-guard-pq ",
        ).forEach { assertNull(AnonymityPolicy.fromLabel(it), "accepted anonymity alias '$it'") }
        listOf(
            "read_only",
            "upload_pq_only",
            "READ-ONLY",
            " read-only",
            "read-only ",
        ).forEach { assertNull(WriteModeHint.fromLabel(it), "accepted write-mode alias '$it'") }
        assertNull(TransportPolicy.fromLabel(null))
        assertNull(AnonymityPolicy.fromLabel(null))
        assertNull(WriteModeHint.fromLabel(null))
    }

    @Test
    fun canonicalProviderValuesAreStoredAndSerializedUnchanged() {
        val provider = sampleProvider()

        assertEquals("alpha", provider.name)
        assertEquals(PROVIDER_ID_HEX, provider.providerIdHex)
        assertEquals(GATEWAY_PUBLIC_KEY_HEX, provider.gatewayPublicKeyHex)
        assertEquals("https://provider.example/", provider.baseUrl)
        assertEquals(STREAM_TOKEN_BASE64, provider.streamTokenBase64)
        assertEquals(
            linkedMapOf(
                "name" to "alpha",
                "provider_id_hex" to PROVIDER_ID_HEX,
                "gateway_public_key_hex" to GATEWAY_PUBLIC_KEY_HEX,
                "base_url" to "https://provider.example/",
                "stream_token_b64" to STREAM_TOKEN_BASE64,
            ),
            provider.toJson(),
        )
    }

    @ParameterizedTest(name = "provider id rejects [{0}]")
    @MethodSource("nonCanonicalHex32")
    fun providerRejectsNonCanonicalProviderId(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                value,
                GATEWAY_PUBLIC_KEY_HEX,
                "https://provider.example/",
                STREAM_TOKEN_BASE64,
            )
        }
    }

    @ParameterizedTest(name = "gateway key rejects [{0}]")
    @MethodSource("nonCanonicalHex32")
    fun providerRejectsNonCanonicalGatewayPublicKey(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                PROVIDER_ID_HEX,
                value,
                "https://provider.example/",
                STREAM_TOKEN_BASE64,
            )
        }
    }

    @Test
    fun providerRejectsSmallOrderGatewayPublicKey() {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                PROVIDER_ID_HEX,
                ED25519_IDENTITY_KEY_HEX,
                "https://provider.example/",
                STREAM_TOKEN_BASE64,
            )
        }
    }

    @ParameterizedTest(name = "stream token rejects [{0}]")
    @MethodSource("nonCanonicalBase64")
    fun providerRejectsNonCanonicalStreamToken(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                PROVIDER_ID_HEX,
                GATEWAY_PUBLIC_KEY_HEX,
                "https://provider.example/",
                value,
            )
        }
    }

    @ParameterizedTest(name = "provider name rejects [{0}]")
    @MethodSource("nonExactText")
    fun providerRejectsNonExactName(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                value,
                PROVIDER_ID_HEX,
                GATEWAY_PUBLIC_KEY_HEX,
                "https://provider.example/",
                STREAM_TOKEN_BASE64,
            )
        }
    }

    @Test
    fun providerRejectsOversizedNameAndStreamTokenBeforeDispatch() {
        GatewayProvider(
            "alpha",
            PROVIDER_ID_HEX,
            GATEWAY_PUBLIC_KEY_HEX,
            "https://provider.example/",
            Base64.getEncoder().encodeToString(ByteArray(2 * 1024)),
        )
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "a".repeat(129),
                PROVIDER_ID_HEX,
                GATEWAY_PUBLIC_KEY_HEX,
                "https://provider.example/",
                STREAM_TOKEN_BASE64,
            )
        }
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                PROVIDER_ID_HEX,
                GATEWAY_PUBLIC_KEY_HEX,
                "https://provider.example/",
                "A".repeat(4 * 1024 + 1),
            )
        }
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                PROVIDER_ID_HEX,
                GATEWAY_PUBLIC_KEY_HEX,
                "https://provider.example/",
                Base64.getEncoder().encodeToString(ByteArray(2 * 1024 + 1)),
            )
        }
    }

    @ParameterizedTest(name = "provider URL rejects [{0}]")
    @MethodSource("nonExactUrls")
    fun providerRejectsNonExactBaseUrl(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayProvider(
                "alpha",
                PROVIDER_ID_HEX,
                GATEWAY_PUBLIC_KEY_HEX,
                value,
                STREAM_TOKEN_BASE64,
            )
        }
    }

    @Test
    fun canonicalOptionsAreStoredAndSerializedUnchanged() {
        val options = GatewayFetchOptions(
            manifestEnvelopeBase64 = "YQ==",
            manifestCidHex = MANIFEST_CID_HEX,
            clientId = "kotlin-sdk",
            telemetryRegion = "ap-northeast-1",
            rolloutPhase = "ramp",
            maxPeers = 3,
            retryBudget = 5,
            transportPolicy = TransportPolicy.DIRECT_ONLY,
            anonymityPolicy = AnonymityPolicy.ANON_MAJORITY_PQ,
            writeModeHint = WriteModeHint.UPLOAD_PQ_ONLY,
        )

        assertEquals("YQ==", options.manifestEnvelopeBase64)
        assertEquals(MANIFEST_CID_HEX, options.manifestCidHex)
        assertEquals("kotlin-sdk", options.clientId)
        assertEquals("ap-northeast-1", options.telemetryRegion)
        assertEquals("ramp", options.rolloutPhase)
        assertEquals(
            linkedMapOf(
                "manifest_envelope_b64" to "YQ==",
                "manifest_cid_hex" to MANIFEST_CID_HEX,
                "client_id" to "kotlin-sdk",
                "telemetry_region" to "ap-northeast-1",
                "rollout_phase" to "ramp",
                "max_peers" to 3,
                "retry_budget" to 5,
                "transport_policy" to "direct-only",
                "anonymity_policy" to "anon-majority-pq",
                "write_mode_hint" to "upload-pq-only",
            ),
            options.toJson(),
        )
    }

    @Test
    fun nullOptionsRemainAbsent() {
        val options = GatewayFetchOptions()

        assertNull(options.manifestEnvelopeBase64)
        assertNull(options.manifestCidHex)
        assertNull(options.clientId)
        assertNull(options.telemetryRegion)
        assertNull(options.rolloutPhase)
    }

    @ParameterizedTest(name = "manifest envelope rejects [{0}]")
    @MethodSource("nonCanonicalBase64")
    fun optionsRejectNonCanonicalManifestEnvelope(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchOptions(manifestEnvelopeBase64 = value)
        }
    }

    @ParameterizedTest(name = "manifest CID rejects [{0}]")
    @MethodSource("nonCanonicalHex32")
    fun optionsRejectNonCanonicalManifestCid(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchOptions(manifestCidHex = value)
        }
    }

    @ParameterizedTest(name = "optional text rejects [{0}]")
    @MethodSource("nonExactText")
    fun optionsRejectNonExactOptionalText(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchOptions(clientId = value)
        }
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchOptions(telemetryRegion = value)
        }
    }

    @ParameterizedTest(name = "rollout phase rejects [{0}]")
    @MethodSource("nonCanonicalRolloutPhases")
    fun optionsRejectNonCanonicalRolloutPhase(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchOptions(rolloutPhase = value)
        }
    }

    @ParameterizedTest(name = "manifest id rejects [{0}]")
    @MethodSource("nonCanonicalHex32")
    fun requestRejectsNonCanonicalManifestId(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchRequest(value, CHUNKER_HANDLE, providers = listOf(sampleProvider()))
        }
    }

    @ParameterizedTest(name = "chunker handle rejects [{0}]")
    @MethodSource("nonCanonicalChunkerHandles")
    fun requestRejectsNonCanonicalChunkerHandle(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchRequest(MANIFEST_ID_HEX, value, providers = listOf(sampleProvider()))
        }
    }

    @Test
    fun requestPreservesCanonicalHandleAndAllowsExplicitAbsence() {
        val provider = sampleProvider()
        val request = GatewayFetchRequest(MANIFEST_ID_HEX, CHUNKER_HANDLE, providers = listOf(provider))
        val requestWithoutHandle = GatewayFetchRequest(MANIFEST_ID_HEX, providers = listOf(provider))

        assertEquals(CHUNKER_HANDLE, request.chunkerHandle)
        assertEquals(CHUNKER_HANDLE, request.toJson()["chunker_handle"])
        assertNull(requestWithoutHandle.chunkerHandle)
        assertEquals(false, requestWithoutHandle.toJson().containsKey("chunker_handle"))
    }

    @Test
    fun requestRejectsEmptyProviderSet() {
        assertThrows(IllegalStateException::class.java) {
            GatewayFetchRequest(MANIFEST_ID_HEX, CHUNKER_HANDLE, providers = emptyList())
        }
    }

    @Test
    fun requestRejectsMoreThanMaximumProviders() {
        assertThrows(IllegalStateException::class.java) {
            GatewayFetchRequest(
                MANIFEST_ID_HEX,
                CHUNKER_HANDLE,
                providers = List(257) { sampleProvider() },
            )
        }
    }

    @Test
    fun clientRejectsNegativeTimeoutAndRetainsZero() {
        assertThrows(IllegalArgumentException::class.java) {
            SorafsGatewayClient(
                baseUri = URI.create("https://gateway.example/"),
                executor = NoopExecutor,
                timeout = Duration.ofNanos(-1),
            )
        }

        val client = SorafsGatewayClient(
            baseUri = URI.create("https://gateway.example/"),
            executor = NoopExecutor,
            timeout = Duration.ZERO,
        )
        assertEquals(Duration.ZERO, client.timeout)
    }

    @ParameterizedTest(name = "client base URI rejects [{0}]")
    @MethodSource("nonExactUrls")
    fun clientRejectsUnsafeBaseUri(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            SorafsGatewayClient(baseUri = URI.create(value), executor = NoopExecutor)
        }
    }

    @ParameterizedTest(name = "fetch path rejects [{0}]")
    @MethodSource("unsafeFetchPaths")
    fun clientRejectsAbsoluteOrAmbiguousFetchPath(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            SorafsGatewayClient(
                baseUri = URI.create("https://gateway.example/"),
                executor = NoopExecutor,
                fetchPath = value,
            )
        }
    }

    @Test
    fun clientBoundsDeclaredAndChunkedStreamingResponses() {
        val declaredClosed = AtomicBoolean(false)
        val declared = BoundedStreamingExecutor(
            emptyInputStream(),
            mapOf("Content-Length" to listOf((16 * 1024 * 1024 + 1).toString())),
            declaredClosed,
        )
        val declaredClient = SorafsGatewayClient(
            baseUri = URI.create("https://gateway.example/"),
            executor = declared,
        )
        assertThrows(CompletionException::class.java) {
            declaredClient.fetch(sampleRequest()).join()
        }
        assertEquals(true, declaredClosed.get())

        val chunkedClosed = AtomicBoolean(false)
        val chunked = BoundedStreamingExecutor(
            RepeatingInputStream(16 * 1024 * 1024 + 1),
            emptyMap(),
            chunkedClosed,
        )
        val chunkedClient = SorafsGatewayClient(
            baseUri = URI.create("https://gateway.example/"),
            executor = chunked,
        )
        assertThrows(CompletionException::class.java) {
            chunkedClient.fetch(sampleRequest()).join()
        }
        assertEquals(true, chunkedClosed.get())

        val ambiguousClosed = AtomicBoolean(false)
        val ambiguous = BoundedStreamingExecutor(
            RepeatingInputStream(1),
            linkedMapOf(
                "Content-Length" to listOf("1"),
                "content-length" to listOf("1"),
            ),
            ambiguousClosed,
        )
        val ambiguousClient = SorafsGatewayClient(
            baseUri = URI.create("https://gateway.example/"),
            executor = ambiguous,
        )
        assertThrows(CompletionException::class.java) {
            ambiguousClient.fetch(sampleRequest()).join()
        }
        assertEquals(true, ambiguousClosed.get())

        val stalledClosed = AtomicBoolean(false)
        val stalled = BoundedStreamingExecutor(
            ZeroProgressInputStream,
            emptyMap(),
            stalledClosed,
        )
        val stalledClient = SorafsGatewayClient(
            baseUri = URI.create("https://gateway.example/"),
            executor = stalled,
        )
        assertThrows(CompletionException::class.java) {
            stalledClient.fetch(sampleRequest()).join()
        }
        assertEquals(true, stalledClosed.get())
    }

    @Test
    fun summaryRejectsNonCanonicalManifestId() {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchSummary.fromJsonBytes(
                summaryJson(MANIFEST_ID_HEX.uppercase(), CHUNKER_HANDLE).toByteArray(Charsets.UTF_8),
            )
        }
    }

    @ParameterizedTest(name = "summary chunker handle rejects [{0}]")
    @MethodSource("nonCanonicalChunkerHandles")
    fun summaryRejectsNonCanonicalChunkerHandle(value: String) {
        assertThrows(IllegalArgumentException::class.java) {
            GatewayFetchSummary.fromJsonBytes(
                summaryJson(MANIFEST_ID_HEX, value).toByteArray(Charsets.UTF_8),
            )
        }
    }

    @Test
    fun summaryAcceptsCanonicalBoundaryValues() {
        val summary = GatewayFetchSummary.fromJsonBytes(
            summaryJson(MANIFEST_ID_HEX, CHUNKER_HANDLE).toByteArray(Charsets.UTF_8),
        )

        assertEquals(MANIFEST_ID_HEX, summary.manifestIdHex)
        assertEquals(CHUNKER_HANDLE, summary.chunkerHandle)
        assertEquals(0, summary.chunkCount)
        assertEquals(0, summary.contentLength)
        assertEquals(0, summary.assembledBytes)
    }

    @Test
    fun summaryRejectsNegativeUnsignedCounters() {
        val rootFields = listOf(
            "chunk_count",
            "content_length",
            "assembled_bytes",
            "anonymity_soranet_selected",
            "anonymity_pq_selected",
            "anonymity_classical_selected",
        )
        for (field in rootFields) {
            assertThrows(SorafsStorageException::class.java) {
                GatewayFetchSummary.fromJsonBytes(
                    summaryJson(MANIFEST_ID_HEX, CHUNKER_HANDLE, mapOf(field to -1))
                        .toByteArray(Charsets.UTF_8),
                )
            }
        }

        for (field in listOf("successes", "failures")) {
            val report = linkedMapOf<String, Any>(
                "provider" to "alpha",
                "successes" to 0,
                "failures" to 0,
                "disabled" to false,
            ).apply { this[field] = -1 }
            assertThrows(SorafsStorageException::class.java) {
                GatewayFetchSummary.fromJsonBytes(
                    summaryJson(
                        MANIFEST_ID_HEX,
                        CHUNKER_HANDLE,
                        mapOf("provider_reports" to listOf(report)),
                    ).toByteArray(Charsets.UTF_8),
                )
            }
        }

        assertThrows(SorafsStorageException::class.java) {
            GatewayFetchSummary.fromJsonBytes(
                summaryJson(
                    MANIFEST_ID_HEX,
                    CHUNKER_HANDLE,
                    mapOf(
                        "chunk_receipts" to listOf(
                            mapOf("chunk_index" to 0, "provider" to "alpha", "attempts" to -1),
                        ),
                    ),
                ).toByteArray(Charsets.UTF_8),
            )
        }
    }

    companion object {
        @JvmStatic
        fun nonCanonicalHex32(): Stream<String> = Stream.of(
            MANIFEST_ID_HEX.uppercase(),
            "0x$MANIFEST_ID_HEX",
            "0X$MANIFEST_ID_HEX",
            " $MANIFEST_ID_HEX",
            "$MANIFEST_ID_HEX ",
            "$MANIFEST_ID_HEX\n",
            MANIFEST_ID_HEX.substring(0, 63),
            MANIFEST_ID_HEX + "00",
            MANIFEST_ID_HEX.substring(0, 62) + "gg",
            "00".repeat(32),
        )

        @JvmStatic
        fun nonCanonicalBase64(): Stream<String> = Stream.of(
            "",
            " ",
            "YQ",
            " YQ==",
            "YQ== ",
            "Y Q==",
            "YQ==\n",
            "YQ===",
            "YR==",
            "_w==",
            "-w==",
            "not!base64",
        )

        @JvmStatic
        fun nonExactText(): Stream<String> = Stream.of(
            "",
            " ",
            " alpha",
            "alpha ",
            "\u00a0alpha",
            "alpha\u2003",
            "alpha\nbeta",
            "alpha\u0000beta",
        )

        @JvmStatic
        fun nonExactUrls(): Stream<String> = Stream.of(
            "",
            " ",
            " https://provider.example",
            "https://provider.example ",
            "\u00a0https://provider.example",
            "https://provider.example\u2003",
            "https://provider.example/\npath",
            "http://provider.example/",
            "https://user@provider.example/",
            "https://provider.example:443/",
            "https://provider.example:444/",
            "https://Provider.Example/",
            "https://provider.example/path",
            "https://provider.example/?query=1",
            "https://provider.example/#fragment",
            "https://localhost/",
            "https://127.0.0.1/",
            "https://10.0.0.1/",
            "https://169.254.169.254/",
            "https://192.0.2.1/",
            "https://198.51.100.1/",
            "https://203.0.113.1/",
            "https://[::1]/",
            "https://[fc00::1]/",
            "https://[fe80::1]/",
            "https://[2001:db8::1]/",
            "https://[::ffff:127.0.0.1]/",
        )

        @JvmStatic
        fun unsafeFetchPaths(): Stream<String> = Stream.of(
            "",
            " ",
            "v1/sorafs/gateway/fetch",
            "/",
            "/v1/sorafs/gateway/fetch/",
            "/v1//sorafs/gateway/fetch",
            "/v1/./sorafs/gateway/fetch",
            "/v1/../sorafs/gateway/fetch",
            "/v1/sorafs/gateway/%66etch",
            "/v1/sorafs/gateway/fetch?target=https://evil.example/",
            "/v1/sorafs/gateway/fetch#fragment",
            "http://evil.example/v1/sorafs/gateway/fetch",
            "https://evil.example/v1/sorafs/gateway/fetch",
        )

        @JvmStatic
        fun nonCanonicalRolloutPhases(): Stream<String> = Stream.of(
            "",
            " ",
            " ramp",
            "ramp ",
            "RAMP",
            "stage-b",
            "stage_b",
            "ga",
            "stable",
            "unknown",
        )

        @JvmStatic
        fun nonCanonicalChunkerHandles(): Stream<String> = Stream.of(
            "",
            " ",
            " $CHUNKER_HANDLE",
            "$CHUNKER_HANDLE ",
            "Sorafs.sf1@1.0.0",
            "sorafs.SF1@1.0.0",
            "sorafs/sf1@1.0.0",
            "sorafs-sf1",
            "sorafs.sf1",
            ".sf1@1.0.0",
            "sorafs.@1.0.0",
            "sorafs.sf1@",
            "sorafs.sf1@1.0",
            "sorafs.sf1@1.0.0.0",
            "sorafs.sf1@01.0.0",
            "sorafs.sf1@1.00.0",
            "sorafs.sf1@1.0.00",
            "sorafs.sf1@+1.0.0",
            "sorafs.sf1@1.0.0-beta",
            "sorafs..sf1@1.0.0",
            "sorafs.sf1@1.0.0@extra",
            "sorafs.sf1@\u0661.0.0",
            "sorafs.sf1@1.0.0\n",
        )
    }
}

private object NoopExecutor : HttpTransportExecutor {
    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
        CompletableFuture.completedFuture(
            TransportResponse(200, ByteArray(0), "OK", emptyMap(), null, false),
        )
}

private class BoundedStreamingExecutor(
    private val body: InputStream,
    private val headers: Map<String, List<String>>,
    private val closed: AtomicBoolean,
) : HttpTransportExecutor, StreamingTransportExecutor {
    override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
        val result = CompletableFuture<TransportResponse>()
        result.completeExceptionally(AssertionError("buffering execute path must not be used"))
        return result
    }

    override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> =
        CompletableFuture.completedFuture(
            TransportStreamResponse(200, body, "OK", headers, Runnable { closed.set(true) }),
        )
}

private class RepeatingInputStream(private var remaining: Int) : InputStream() {
    override fun read(): Int {
        if (remaining == 0) return -1
        remaining -= 1
        return 0
    }

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        if (remaining == 0) return -1
        val count = minOf(length, remaining)
        java.util.Arrays.fill(buffer, offset, offset + count, 0.toByte())
        remaining -= count
        return count
    }
}

private object ZeroProgressInputStream : InputStream() {
    override fun read(): Int = 0

    override fun read(buffer: ByteArray, offset: Int, length: Int): Int = 0
}

private fun emptyInputStream(): InputStream =
    object : InputStream() {
        override fun read(): Int = -1
    }

private fun sampleProvider(): GatewayProvider = GatewayProvider(
    name = "alpha",
    providerIdHex = PROVIDER_ID_HEX,
    gatewayPublicKeyHex = GATEWAY_PUBLIC_KEY_HEX,
    baseUrl = "https://provider.example/",
    streamTokenBase64 = STREAM_TOKEN_BASE64,
)

private fun sampleRequest(): GatewayFetchRequest = GatewayFetchRequest(
    manifestIdHex = MANIFEST_ID_HEX,
    chunkerHandle = CHUNKER_HANDLE,
    providers = listOf(sampleProvider()),
)

private fun summaryJson(
    manifestIdHex: String,
    chunkerHandle: String,
    overrides: Map<String, Any?> = emptyMap(),
): String = JsonWriter.encode(
    linkedMapOf<String, Any?>(
        "manifest_id_hex" to manifestIdHex,
        "chunker_handle" to chunkerHandle,
        "client_id" to null,
        "chunk_count" to 0,
        "content_length" to 0,
        "assembled_bytes" to 0,
        "provider_reports" to emptyList<Any>(),
        "chunk_receipts" to emptyList<Any>(),
        "anonymity_policy" to "anon-guard-pq",
        "anonymity_status" to "met",
        "anonymity_reason" to null,
        "anonymity_soranet_selected" to 0,
        "anonymity_pq_selected" to 0,
        "anonymity_classical_selected" to 0,
        "anonymity_classical_ratio" to 0.0,
        "anonymity_pq_ratio" to 1.0,
        "anonymity_candidate_ratio" to 1.0,
        "anonymity_deficit_ratio" to 0.0,
        "anonymity_supply_delta" to 0.0,
        "anonymity_brownout" to false,
        "anonymity_brownout_effective" to false,
        "anonymity_uses_classical" to false,
    ).apply { putAll(overrides) },
)
