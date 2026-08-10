package org.hyperledger.iroha.sdk.sorafs

import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.TransportStreamResponse
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import java.io.ByteArrayInputStream
import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.ArrayDeque
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SorafsReputationClientTest {
    @Test
    fun finiteReadsUseExactAuthenticatedGetTargetsAndTypedProjection() {
        val executor = RecordingStreamingExecutor()
        val client = SorafsReputationClient(BASE_URI, NETWORK_ID, executor)
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()

        executor.enqueueJson(SNAPSHOT_JSON)
        val latestAuth = auth(keyPair, "latest")
        val latest = client.getLatest(latestAuth, 500).join()
        assertNotNull(latest)
        assertEquals(SNAPSHOT_ID, latest.snapshotIdHex)
        assertEquals("18446744073709551615", latest.generatedAtUnix)
        assertEquals("provider:alpha", latest.providers.single().providerId)
        assertExactSignedGet(
            executor.requests[0],
            "/v1/sorafs/reputation/latest",
            "limit=500",
            keyPair,
            latestAuth,
        )

        executor.enqueueJson(PROVIDER_JSON)
        val providerAuth = auth(keyPair, "provider")
        val provider = client.getProvider("provider:alpha", providerAuth).join()
        assertNotNull(provider)
        assertEquals("provider:alpha", provider.proof.providerId)
        assertExactSignedGet(
            executor.requests[1],
            "/v1/sorafs/reputation/providers/provider:alpha",
            null,
            keyPair,
            providerAuth,
        )
        assertEquals(
            "/v1/sorafs/reputation/providers/provider:alpha",
            executor.requests[1].uri.rawPath,
        )
        assertFalse(executor.requests[1].uri.toASCIIString().contains("%3A", ignoreCase = true))

        executor.enqueueJson(SNAPSHOT_JSON.replace(""""limit":500""", """"limit":1"""))
        val snapshotAuth = auth(keyPair, "snapshot")
        val snapshot = client.getSnapshot(SNAPSHOT_ID, snapshotAuth, 1).join()
        assertEquals(SNAPSHOT_ID, snapshot?.snapshotIdHex)
        assertExactSignedGet(
            executor.requests[2],
            "/v1/sorafs/reputation/snapshots/$SNAPSHOT_ID",
            "limit=1",
            keyPair,
            snapshotAuth,
        )

        executor.enqueueJson(WEIGHTS_JSON)
        val weightsAuth = auth(keyPair, "weights")
        val weights = client.getWeights(weightsAuth).join()
        assertEquals(2_200, weights.weights.porSuccessBps)
        assertExactSignedGet(
            executor.requests[3],
            "/v1/sorafs/reputation/weights",
            null,
            keyPair,
            weightsAuth,
        )

        executor.enqueueJson(EVENT_PAGE_JSON)
        val eventsAuth = auth(keyPair, "events")
        val events = client.listEvents(eventsAuth, "7", 1).join()
        assertEquals("8", events.nextSince)
        assertEquals("8", events.events.single().sequence)
        assertExactSignedGet(
            executor.requests[4],
            "/v1/sorafs/reputation/events",
            "since=7&limit=1",
            keyPair,
            eventsAuth,
        )

        executor.enqueueStatus(404)
        assertNull(client.getLatest(auth(keyPair, "missing")).join())
        assertEquals(6, executor.requests.size)
    }

    @Test
    fun rejectsNoncanonicalInputsAndPartialAuthenticationBeforeTransport() {
        val executor = RecordingStreamingExecutor()
        val client = SorafsReputationClient(BASE_URI, NETWORK_ID, executor)
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val canonicalAuth = auth(keyPair, "valid")

        for (snapshot in listOf(
            "AB".repeat(16),
            "0x" + "ab".repeat(16),
            " " + "ab".repeat(16),
            "0".repeat(32),
            "ab".repeat(15),
        )) {
            assertFailsWith<IllegalArgumentException> {
                client.getSnapshot(snapshot, canonicalAuth)
            }
        }
        for (provider in listOf(
            "",
            "provider alias",
            "provider/alpha",
            "provider%3Aalpha",
            "provider\u00e9",
            ".",
            "..",
            "a".repeat(257),
        )) {
            assertFailsWith<IllegalArgumentException> {
                client.getProvider(provider, canonicalAuth)
            }
        }
        for (since in listOf(
            "",
            "01",
            "-1",
            "+1",
            " 1",
            "18446744073709551616",
        )) {
            assertFailsWith<IllegalArgumentException> {
                client.listEvents(canonicalAuth, since, 1)
            }
        }
        for (limit in listOf(0, 501, -1)) {
            assertFailsWith<IllegalArgumentException> {
                client.listEvents(canonicalAuth, "0", limit)
            }
        }
        val partialAuth = ToriiCanonicalRequestAuth(
            "reputation-reader@sora",
            keyPair.private,
            TIMESTAMP_MS,
            null,
        )
        assertFailsWith<IllegalArgumentException> {
            client.getWeights(partialAuth)
        }
        assertTrue(executor.requests.isEmpty())
    }

    @Test
    fun authenticatedSseUsesOneStreamingAttemptWithoutResumeSurface() {
        val executor = RecordingStreamingExecutor()
        val client = SorafsReputationClient(BASE_URI, NETWORK_ID, executor)
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val canonicalAuth = auth(keyPair, "stream")
        executor.enqueueStream(
            """
            id: 8
            event: reputation_snapshot
            data: $EVENT_JSON

            event: lagged
            data: 2

            """.trimIndent(),
        )
        val snapshots = ArrayList<SorafsReputationSnapshotEventV1>()
        val lagged = ArrayList<String>()
        val closed = CountDownLatch(1)
        val stream = client.openEventStream(
            canonicalAuth = canonicalAuth,
            listener = object : SorafsReputationEventStreamListener {
                override fun onSnapshot(event: SorafsReputationSnapshotEventV1) {
                    snapshots.add(event)
                }

                override fun onLagged(skipped: String) {
                    lagged.add(skipped)
                }

                override fun onClosed() {
                    closed.countDown()
                }
            },
            since = "7",
            limit = 1,
        )

        stream.completion().get(5, TimeUnit.SECONDS)
        assertTrue(closed.await(5, TimeUnit.SECONDS))
        assertEquals(listOf("8"), snapshots.map { it.sequence })
        assertEquals(listOf("2"), lagged)
        assertEquals(1, executor.streamRequests.size)
        val request = executor.streamRequests.single()
        assertExactSignedGet(
            request,
            "/v1/sorafs/reputation/events/stream",
            "since=7&limit=1",
            keyPair,
            canonicalAuth,
        )
        assertFalse(request.headers.keys.any { it.equals("Last-Event-ID", ignoreCase = true) })
        assertEquals(listOf("text/event-stream"), request.headers["Accept"])
    }

    @Test
    fun sseRejectsBufferedFallbackBeforeSigningOrRequest() {
        var calls = 0
        val bufferedOnly = object : TransportExecutor {
            override fun execute(
                request: TransportRequest,
            ): CompletableFuture<TransportResponse> {
                calls += 1
                return CompletableFuture.completedFuture(
                    TransportResponse.builder().setStatusCode(200).setBody(ByteArray(0)).build(),
                )
            }
        }
        val client = SorafsReputationClient(BASE_URI, NETWORK_ID, bufferedOnly)
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()

        assertFailsWith<IllegalArgumentException> {
            client.openEventStream(
                auth(keyPair, "no-fallback"),
                object : SorafsReputationEventStreamListener {
                    override fun onSnapshot(event: SorafsReputationSnapshotEventV1) = Unit
                },
            )
        }
        assertEquals(0, calls)
    }

    @Test
    fun parserRejectsNoncanonicalOrderingAndEventGaps() {
        val unsortedProviders = SNAPSHOT_JSON.replace(
            """"providers":[$PROVIDER_OBJECT]""",
            """"providers":[$PROVIDER_OBJECT,${PROVIDER_OBJECT.replace("provider:alpha", "provider:aardvark")}]""",
        ).replace(""""provider_count":1""", """"provider_count":2""")
            .replace(""""returned_provider_count":1""", """"returned_provider_count":2""")
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseSnapshot(unsortedProviders.toByteArray())
        }

        val reversedFlags = SNAPSHOT_JSON.replace(
            """"degradation_flags":[{"flag":"reserve_warning","value":null},{"flag":"low_score","value":null}]""",
            """"degradation_flags":[{"flag":"low_score","value":null},{"flag":"reserve_warning","value":null}]""",
        )
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseSnapshot(reversedFlags.toByteArray())
        }

        val underfilledProviderPrefix = SNAPSHOT_JSON
            .replace(""""provider_count":1""", """"provider_count":2""")
            .replace(""""limit":500""", """"limit":2""")
            .replace(""""truncated_providers":false""", """"truncated_providers":true""")
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseSnapshot(underfilledProviderPrefix.toByteArray())
        }

        val selfPredecessor = SNAPSHOT_JSON.replace(
            """"previous_snapshot_id_hex":null""",
            """"previous_snapshot_id_hex":"$SNAPSHOT_ID"""",
        )
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseSnapshot(selfPredecessor.toByteArray())
        }

        val wrongProofDepth = PROVIDER_JSON.replace(
            """"siblings_hex":[]""",
            """"siblings_hex":["$MERKLE_ROOT"]""",
        )
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseProviderResponse(wrongProofDepth.toByteArray())
        }

        val retainedFirst = EVENT_JSON.replace(""""sequence":8""", """"sequence":9""")
            .replace(
                """"generated_at_unix":18446744073709551615""",
                """"generated_at_unix":9""",
            )
        val retainedGap =
            """{"since":7,"limit":2,"count":1,"next_since":9,"events":[$retainedFirst]}"""
        assertEquals(
            "9",
            SorafsReputationJsonParser.parseEventPage(retainedGap.toByteArray()).nextSince,
        )
        val retainedSecond = EVENT_JSON.replace(""""sequence":8""", """"sequence":11""")
            .replace(SNAPSHOT_ID, NEXT_SNAPSHOT_ID)
            .replace(
                """"generated_at_unix":18446744073709551615""",
                """"generated_at_unix":10""",
            )
            .replace(
                """"previous_snapshot_id_hex":null""",
                """"previous_snapshot_id_hex":"$SNAPSHOT_ID"""",
            )
        val overLimitSecond = retainedSecond.replace(""""sequence":11""", """"sequence":10""")
        val overLimitPage =
            """{"since":7,"limit":1,"count":2,"next_since":10,"events":[$retainedFirst,$overLimitSecond]}"""
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseEventPage(overLimitPage.toByteArray())
        }
        val gapped =
            """{"since":7,"limit":2,"count":2,"next_since":11,"events":[$retainedFirst,$retainedSecond]}"""
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseEventPage(gapped.toByteArray())
        }

        val unlinkedSecond = retainedSecond.replace(""""sequence":11""", """"sequence":10""")
            .replace(
                """"previous_snapshot_id_hex":"$SNAPSHOT_ID"""",
                """"previous_snapshot_id_hex":null""",
            )
        val unlinkedPage =
            """{"since":7,"limit":2,"count":2,"next_since":10,"events":[$retainedFirst,$unlinkedSecond]}"""
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseEventPage(unlinkedPage.toByteArray())
        }

        val nonIncreasingSecond =
            retainedSecond.replace(""""sequence":11""", """"sequence":10""")
                .replace(""""generated_at_unix":10""", """"generated_at_unix":9""")
        val nonIncreasingPage =
            """{"since":7,"limit":2,"count":2,"next_since":10,"events":[$retainedFirst,$nonIncreasingSecond]}"""
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseEventPage(nonIncreasingPage.toByteArray())
        }

        val selfLinkedEvent = EVENT_JSON.replace(
            """"previous_snapshot_id_hex":null""",
            """"previous_snapshot_id_hex":"$SNAPSHOT_ID"""",
        )
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseEventJson(selfLinkedEvent)
        }
        assertFailsWith<IllegalStateException> {
            SorafsReputationJsonParser.parseEventJson(EVENT_JSON.replace(",", ", "))
        }
    }

    private fun auth(keyPair: KeyPair, nonceSuffix: String): ToriiCanonicalRequestAuth =
        ToriiCanonicalRequestAuth(
            "reputation-reader@sora",
            keyPair.private,
            TIMESTAMP_MS,
            "nonce-$nonceSuffix",
        )

    private fun assertExactSignedGet(
        request: TransportRequest,
        path: String,
        query: String?,
        keyPair: KeyPair,
        auth: ToriiCanonicalRequestAuth,
    ) {
        assertEquals("GET", request.method)
        assertEquals(path, request.uri.rawPath)
        assertEquals(query, request.uri.rawQuery)
        assertTrue(request.body.isEmpty())
        val signature = request.headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.single()
        assertNotNull(signature)
        val timestampMs = requireNotNull(auth.timestampMs)
        val nonce = requireNotNull(auth.nonce)
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(keyPair.public)
        verifier.update(
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                NETWORK_ID,
                "GET",
                request.uri,
                ByteArray(0),
                timestampMs,
                nonce,
            ),
        )
        val signatureBytes = Base64.getDecoder().decode(signature)
        assertTrue(verifier.verify(signatureBytes))
        verifier.initVerify(keyPair.public)
        verifier.update(
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                TestNetworkIds.fromSeed(99),
                "GET",
                request.uri,
                ByteArray(0),
                timestampMs,
                nonce,
            ),
        )
        assertFalse(verifier.verify(signatureBytes))
    }

    private class RecordingStreamingExecutor : StreamingTransportExecutor {
        val requests = ArrayList<TransportRequest>()
        val streamRequests = ArrayList<TransportRequest>()
        private val responses = ArrayDeque<TransportResponse>()
        private val streams = ArrayDeque<TransportStreamResponse>()

        fun enqueueJson(json: String) {
            responses.add(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(json.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }

        fun enqueueStatus(status: Int) {
            responses.add(TransportResponse.builder().setStatusCode(status).build())
        }

        fun enqueueStream(body: String) {
            streams.add(
                TransportStreamResponse(
                    200,
                    ByteArrayInputStream(body.toByteArray(StandardCharsets.UTF_8)),
                    "",
                    emptyMap(),
                    null,
                ),
            )
        }

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            return CompletableFuture.completedFuture(responses.removeFirst())
        }

        override fun openStream(
            request: TransportRequest,
        ): CompletableFuture<TransportStreamResponse> {
            streamRequests.add(request)
            return CompletableFuture.completedFuture(streams.removeFirst())
        }
    }

    private companion object {
        private val BASE_URI = URI.create("https://torii.example")
        private val NETWORK_ID = TestNetworkIds.canonical()
        private const val TIMESTAMP_MS = 1_717_171_717_000L
        private const val SNAPSHOT_ID = "abababababababababababababababab"
        private const val NEXT_SNAPSHOT_ID = "bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc"
        private const val MERKLE_ROOT =
            "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"
        private const val METRICS_HASH =
            "efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef"
        private const val WEIGHTS =
            """"version":1,"por_success_bps":2200,"pdp_success_bps":2000,"potr_success_bps":1800,"latency_bps":1500,"dispute_bps":1000,"token_violation_bps":500,"repair_breach_bps":1000"""
        private const val METRICS =
            """"version":1,"por_success_bps":9500,"pdp_success_bps":9600,"potr_success_bps":9700,"latency_health_bps":9800,"dispute_rate_bps":100,"token_violation_rate_bps":0,"repair_breach_rate_bps":50"""
        private const val PROVIDER_OBJECT =
            """{"provider_id":"provider:alpha","score_bps":900,"degradation_flags":[{"flag":"reserve_warning","value":null},{"flag":"low_score","value":null}],"raw_metrics":{$METRICS},"raw_metrics_hash_hex":"$METRICS_HASH"}"""
        private const val SNAPSHOT_JSON =
            """{"snapshot_id_hex":"$SNAPSHOT_ID","generated_at_unix":18446744073709551615,"previous_snapshot_id_hex":null,"merkle_root_hex":"$MERKLE_ROOT","provider_count":1,"returned_provider_count":1,"limit":500,"truncated_providers":false,"alpha_bps":8500,"current_score_weight_bps":7000,"weights":{$WEIGHTS},"providers":[$PROVIDER_OBJECT]}"""
        private const val PROVIDER_JSON =
            """{"snapshot_id_hex":"$SNAPSHOT_ID","generated_at_unix":18446744073709551615,"merkle_root_hex":"$MERKLE_ROOT","provider":$PROVIDER_OBJECT,"proof":{"provider_id":"provider:alpha","leaf_index":0,"leaf_count":1,"siblings_hex":[]}}"""
        private const val WEIGHTS_JSON =
            """{"snapshot_id_hex":"$SNAPSHOT_ID","generated_at_unix":18446744073709551615,"alpha_bps":8500,"current_score_weight_bps":7000,"weights":{$WEIGHTS}}"""
        private const val EVENT_JSON =
            """{"version":1,"sequence":8,"snapshot_id_hex":"$SNAPSHOT_ID","generated_at_unix":18446744073709551615,"merkle_root_hex":"$MERKLE_ROOT","provider_count":1,"previous_snapshot_id_hex":null}"""
        private const val EVENT_PAGE_JSON =
            """{"since":7,"limit":1,"count":1,"next_since":8,"events":[$EVENT_JSON]}"""
    }
}
