package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

class OfflineCashToriiV1Test {
    @Test
    fun `fixture loader enforces the exact eager 40 row contract`() {
        val rows = OfflineCashToriiV1Fixtures.canonicalRowsForTest()
        assertEquals(40, rows.size)
        assertEquals(40, OfflineCashToriiV1Fixtures.parseRows(rows).size)

        val networkId = fixtureValue(rows, "network_id")
        val topUpRequest = fixtureValue(rows, "top_up_request")
        val invalidRowSets = listOf(
            rows + "unexpected_fixture=00",
            rows.dropLast(1),
            rows + rows.first(),
            replaceFixtureRow(rows, "network_id", networkId.uppercase()),
            replaceFixtureRow(rows, "network_id", networkId.dropLast(1) + "0"),
            replaceFixtureRow(rows, "top_up_submitted_at_ms", "01"),
            replaceFixtureRow(rows, "top_up_request", "0"),
            replaceFixtureRow(rows, "top_up_request", topUpRequest.uppercase()),
            replaceFixtureRow(rows, "top_up_request", ""),
            rows + "missing_separator",
        )
        for (invalidRows in invalidRowSets) {
            assertFailsWith<IllegalStateException> {
                OfflineCashToriiV1Fixtures.parseRows(invalidRows)
            }
        }
    }

    @Test
    fun `canonical request and response wrappers enforce schema bounds and ownership`() {
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.Companion.SubmissionRequestProjection(
                ByteArray(32),
                ByteArray(32) { 1 },
                "1",
            )
        }
        val topUpBytes = OfflineCashToriiV1Fixtures.topUpRequest
        val retainedTopUp = topUpBytes.copyOf()
        val topUp = OfflineCashTopUpRequestV1(topUpBytes)
        topUpBytes.fill(0)
        assertContentEquals(retainedTopUp, topUp.encodeCanonical())
        topUp.encodeCanonical().fill(0)
        assertContentEquals(retainedTopUp, topUp.encodeCanonical())
        assertEquals(topUp, OfflineCashTopUpRequestV1.decodeCanonical(retainedTopUp))

        val redeemBytes = OfflineCashToriiV1Fixtures.redeemRequest
        val retainedRedeem = redeemBytes.copyOf()
        val redeem = OfflineCashRedeemRequestV1(redeemBytes)
        redeemBytes.fill(0)
        assertContentEquals(retainedRedeem, redeem.encodeCanonical())
        redeem.encodeCanonical().fill(0)
        assertContentEquals(retainedRedeem, redeem.encodeCanonical())

        val referenceBytes = OfflineCashToriiV1Fixtures.topUpReference
        val reference = OfflineCashOperationReferenceV1(referenceBytes)
        referenceBytes.fill(0)
        assertEquals(
            OfflineCashOperationReferenceV1.decodeCanonical(reference.encodeCanonical()),
            reference,
        )

        val statusBytes = OfflineCashToriiV1Fixtures.topUpPendingStatus
        val status = OfflineCashOperationStatusV1(statusBytes)
        statusBytes.fill(0)
        assertEquals(OfflineCashOperationStatusV1.decodeCanonical(status.encodeCanonical()), status)

        for (invalid in listOf(
            ByteArray(0),
            archive(REDEEM_SCHEMA),
            archive(TOP_UP_SCHEMA),
            OfflineCashToriiV1Fixtures.invalidBindingTopUpRequest,
            retainedTopUp.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() },
            ByteArray(OfflineCashTopUpRequestV1.MAX_CANONICAL_BYTES + 1),
        )) {
            assertFailsWith<IllegalArgumentException> { OfflineCashTopUpRequestV1(invalid) }
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashRedeemRequestV1(
                ByteArray(OfflineCashRedeemRequestV1.MAX_CANONICAL_BYTES + 1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationReferenceV1(archive(OPERATION_STATUS_SCHEMA))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationStatusV1(archive(OPERATION_REFERENCE_SCHEMA))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationReferenceV1(OfflineCashToriiV1Fixtures.zeroTimeReference)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationReferenceV1(
                OfflineCashToriiV1Fixtures.invalidTransactionHashReference,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationStatusV1(OfflineCashToriiV1Fixtures.zeroSubmittedPendingStatus)
        }
        for (invalidStatus in listOf(
            OfflineCashToriiV1Fixtures.invalidTransactionHashStatus,
            OfflineCashToriiV1Fixtures.wrongRejectionCodeStatus,
            OfflineCashToriiV1Fixtures.rejectionDetailsStatus,
            OfflineCashToriiV1Fixtures.oversizedRejectionMessageStatus,
        )) {
            assertFailsWith<IllegalArgumentException> {
                OfflineCashOperationStatusV1(invalidStatus)
            }
        }
        assertEquals(
            OfflineCashOperationStateV1.APPLIED,
            OfflineCashOperationStatusV1(
                OfflineCashToriiV1Fixtures.foreignNetworkTopUpStatus,
            ).project().state,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationRejectionV1.fromValidatedProjection(
                "another_rejection",
                "rejected",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationRejectionV1.fromValidatedProjection(
                "offline_operation_rejected",
                "界".repeat(1_025),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationRejectionV1.fromValidatedProjection(
                "offline_operation_rejected",
                "\u00a0rejected",
            )
        }
    }

    @Test
    fun `readiness is the strict asset-neutral DTO and preserves canonical blockers`() {
        val transport = RecordingTransport { request ->
            assertEquals("GET", request.method)
            assertEquals(OfflineCashToriiClientV1.READINESS_PATH, request.uri.path)
            response(200, OfflineCashToriiClientV1.JSON_MEDIA_TYPE, READINESS_JSON)
        }
        val readiness = client(transport).getReadiness().join()

        assertFalse(readiness.mandatory)
        assertEquals("cash_handoff_v1", readiness.cashHandoffCapability)
        assertEquals(22, readiness.requiredBridgeAbiVersion)
        assertEquals(8, readiness.maximumHops)
        assertFalse(readiness.ready)
        assertTrue(readiness.assets.isEmpty())
        assertEquals(
            listOf(
                "offline_cash_authenticated_release_unavailable",
                "offline_cash_eligible_asset_unavailable",
                "offline_cash_proof_backend_unavailable",
            ),
            readiness.blockers.map { blocker -> blocker.code },
        )
        assertEquals(
            OfflineCashReadinessBlockerV1.fromValidatedProjection(
                "offline_cash_authenticated_release_unavailable",
                "No authenticated Offline Cash V1 release is selected by this asset-neutral response.",
            ),
            readiness.blockers.first(),
        )
        @Suppress("UNCHECKED_CAST")
        val mutableBlockerView =
            readiness.blockers as MutableList<OfflineCashReadinessBlockerV1>
        assertFailsWith<UnsupportedOperationException> { mutableBlockerView.clear() }

        val invalidBodies = listOf(
            READINESS_JSON.replace("\"ready\":false", "\"ready\":true"),
            READINESS_JSON.replace("\"assets\":[]", "\"assets\":[{}]"),
            READINESS_JSON.replace(
                "\"offline_cash_authenticated_release_unavailable\"",
                "\"unexpected_blocker\"",
            ),
            READINESS_JSON.dropLast(1) + ",\"future\":true}",
        )
        for (body in invalidBodies) {
            val invalidTransport = RecordingTransport {
                response(200, OfflineCashToriiClientV1.JSON_MEDIA_TYPE, body)
            }
            assertFutureFailure { client(invalidTransport).getReadiness() }
        }
    }

    @Test
    fun `client uses exactly four routes and byte-identical idempotent command bodies`() {
        assertEquals("/v1/offline/readiness", OfflineCashToriiClientV1.READINESS_PATH)
        assertEquals("/v1/offline/top-up", OfflineCashToriiClientV1.TOP_UP_PATH)
        assertEquals("/v1/offline/redeem", OfflineCashToriiClientV1.REDEEM_PATH)
        assertEquals("/v1/offline/operations", OfflineCashToriiClientV1.OPERATIONS_PATH)
        assertEquals("application/json", OfflineCashToriiClientV1.JSON_MEDIA_TYPE)
        assertEquals("application/x-norito", OfflineCashToriiClientV1.NORITO_MEDIA_TYPE)

        val invalidTransport = RecordingTransport {
            throw AssertionError("invalid base URI reached transport")
        }
        for (uri in listOf("http:opaque", "https:/missing-host", "https://user@torii.example")) {
            assertFailsWith<IllegalArgumentException> {
                OfflineCashToriiClientV1.create(
                    URI.create(uri),
                    invalidTransport,
                    signingContext(),
                )
            }
        }

        val topUpOperationId = OfflineCashToriiV1Fixtures.topUpOperationId
        val redeemOperationId = OfflineCashToriiV1Fixtures.redeemOperationId
        val transport = RecordingTransport { request ->
            when {
                request.uri.path.endsWith(OfflineCashToriiClientV1.READINESS_PATH) ->
                    response(200, OfflineCashToriiClientV1.JSON_MEDIA_TYPE, READINESS_JSON)
                request.uri.path.endsWith(OfflineCashToriiClientV1.TOP_UP_PATH) ->
                    acceptedResponse(
                        OfflineCashToriiV1Fixtures.topUpReference,
                        topUpOperationId,
                    )
                request.uri.path.endsWith(OfflineCashToriiClientV1.REDEEM_PATH) ->
                    acceptedResponse(
                        OfflineCashToriiV1Fixtures.redeemReference,
                        redeemOperationId,
                    )
                request.uri.path.contains("${OfflineCashToriiClientV1.OPERATIONS_PATH}/") ->
                    response(
                        200,
                        OfflineCashToriiClientV1.NORITO_MEDIA_TYPE,
                        OfflineCashToriiV1Fixtures.topUpPendingStatus,
                    )
                else -> error("unexpected route ${request.uri}")
            }
        }
        val client = client(transport, URI.create("https://torii.example/api/"))

        client.getReadiness().join()
        val readinessRequest = transport.requests.last()
        assertEquals("https://torii.example/api/v1/offline/readiness", readinessRequest.uri.toString())
        assertEquals(listOf(OfflineCashToriiClientV1.JSON_MEDIA_TYPE), readinessRequest.headers["Accept"])
        assertEquals(
            OfflineCashOperationStatusV1.MAX_CANONICAL_BYTES.toLong(),
            readinessRequest.maximumResponseBytes,
        )
        assertEquals(RequestReplayPolicy.RETRY_SAFE, readinessRequest.replayPolicy)

        val topUpSource = OfflineCashToriiV1Fixtures.topUpRequest
        val retainedTopUp = topUpSource.copyOf()
        val topUp = OfflineCashTopUpRequestV1(topUpSource)
        topUpSource.fill(0)
        client.submitTopUp(topUp, topUpOperationId).join()
        val firstTopUp = transport.requests.last()
        client.submitTopUp(topUp, topUpOperationId).join()
        val secondTopUp = transport.requests.last()
        assertEquals("POST", firstTopUp.method)
        assertEquals("/api/v1/offline/top-up", firstTopUp.uri.path)
        assertEquals(listOf(OfflineCashToriiClientV1.NORITO_MEDIA_TYPE), firstTopUp.headers["Accept"])
        assertEquals(
            listOf(OfflineCashToriiClientV1.NORITO_MEDIA_TYPE),
            firstTopUp.headers["Content-Type"],
        )
        assertEquals(listOf(topUpOperationId), firstTopUp.headers["Idempotency-Key"])
        assertEquals(RequestReplayPolicy.ONE_SHOT, firstTopUp.replayPolicy)
        assertContentEquals(retainedTopUp, firstTopUp.body)
        assertContentEquals(firstTopUp.body, secondTopUp.body)
        assertEquals(firstTopUp.headers["Idempotency-Key"], secondTopUp.headers["Idempotency-Key"])

        val redeemSource = OfflineCashToriiV1Fixtures.redeemRequest
        val retainedRedeem = redeemSource.copyOf()
        val redeem = OfflineCashRedeemRequestV1(redeemSource)
        redeemSource.fill(0)
        client.submitRedeem(redeem, redeemOperationId).join()
        val firstRedeem = transport.requests.last()
        client.submitRedeem(redeem, redeemOperationId).join()
        val secondRedeem = transport.requests.last()
        assertEquals("/api/v1/offline/redeem", firstRedeem.uri.path)
        assertContentEquals(retainedRedeem, firstRedeem.body)
        assertContentEquals(firstRedeem.body, secondRedeem.body)
        assertEquals(firstRedeem.headers["Idempotency-Key"], secondRedeem.headers["Idempotency-Key"])

        client.getOperation(topUpOperationId).join()
        val poll = transport.requests.last()
        assertEquals("GET", poll.method)
        assertEquals("/api/v1/offline/operations/$topUpOperationId", poll.uri.path)
        assertEquals(RequestReplayPolicy.RETRY_SAFE, poll.replayPolicy)

        val requestCount = transport.requests.size
        for (invalidId in listOf("0".repeat(64), "AB".repeat(32), "11".repeat(31), "11".repeat(32) + "00")) {
            assertFailsWith<IllegalArgumentException> { client.submitTopUp(topUp, invalidId) }
            assertFailsWith<IllegalArgumentException> { client.submitRedeem(redeem, invalidId) }
            assertFailsWith<IllegalArgumentException> { client.getOperation(invalidId) }
        }
        assertEquals(requestCount, transport.requests.size)
    }

    @Test
    fun `response status and media type fail closed`() {
        val topUp = OfflineCashTopUpRequestV1(OfflineCashToriiV1Fixtures.topUpRequest)
        val operationId = OfflineCashToriiV1Fixtures.topUpOperationId
        val reference = OfflineCashToriiV1Fixtures.topUpReference

        for (badResponse in listOf(
            response(201, OfflineCashToriiClientV1.JSON_MEDIA_TYPE, READINESS_JSON),
            response(200, "application/json; charset=utf-8", READINESS_JSON),
            response(
                200,
                OfflineCashToriiClientV1.JSON_MEDIA_TYPE,
                READINESS_JSON,
                duplicateContentType = true,
            ),
        )) {
            assertFutureFailure {
                client(RecordingTransport { badResponse }).getReadiness()
            }
        }

        for (badResponse in listOf(
            response(200, OfflineCashToriiClientV1.NORITO_MEDIA_TYPE, reference),
            response(202, OfflineCashToriiClientV1.JSON_MEDIA_TYPE, reference),
            response(
                202,
                OfflineCashToriiClientV1.NORITO_MEDIA_TYPE,
                reference,
                duplicateContentType = true,
            ),
        )) {
            assertFutureFailure {
                client(RecordingTransport { badResponse }).submitTopUp(topUp, operationId)
            }
        }

        assertFutureFailure {
            client(
                RecordingTransport {
                    response(
                        202,
                        OfflineCashToriiClientV1.NORITO_MEDIA_TYPE,
                        OfflineCashToriiV1Fixtures.topUpPendingStatus,
                    )
                },
            ).getOperation(operationId)
        }
    }

    @Test
    fun `signed request accepted response and poll resource remain exactly bound`() {
        val topUp = OfflineCashTopUpRequestV1(OfflineCashToriiV1Fixtures.topUpRequest)
        val operationId = OfflineCashToriiV1Fixtures.topUpOperationId

        assertFailsWith<IllegalArgumentException> {
            OfflineCashTopUpRequestV1(OfflineCashToriiV1Fixtures.invalidBindingTopUpRequest)
        }
        for (invalidAppliedStatus in listOf(
            OfflineCashToriiV1Fixtures.zeroHeightStatus,
            OfflineCashToriiV1Fixtures.zeroTimeStatus,
        )) {
            assertFailsWith<IllegalArgumentException> {
                OfflineCashOperationStatusV1(invalidAppliedStatus)
            }
        }

        val noTransport = RecordingTransport {
            throw AssertionError("locally rejected request reached transport")
        }
        val correctlyBoundClient = client(noTransport)
        assertFailsWith<IllegalArgumentException> {
            correctlyBoundClient.submitTopUp(topUp, OfflineCashToriiV1Fixtures.redeemOperationId)
        }
        val foreignNetworkClient = OfflineCashToriiClientV1.create(
            URI.create("https://torii.example"),
            noTransport,
            LocalSigningContext(
                NetworkId.parse("32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            foreignNetworkClient.submitTopUp(topUp, operationId)
        }
        assertTrue(noTransport.requests.isEmpty())

        val invalidReferences = listOf(
            acceptedResponse(OfflineCashToriiV1Fixtures.wrongIdReference, operationId),
            acceptedResponse(OfflineCashToriiV1Fixtures.wrongKindReference, operationId),
            acceptedResponse(OfflineCashToriiV1Fixtures.wrongTimeReference, operationId),
            acceptedResponse(OfflineCashToriiV1Fixtures.zeroTimeReference, operationId),
            acceptedResponse(OfflineCashToriiV1Fixtures.wrongUriReference, operationId),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.invalidTransactionHashReference,
                operationId,
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                location = null,
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                retryAfter = null,
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                location = "/v1/offline/operations/${OfflineCashToriiV1Fixtures.redeemOperationId}",
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                retryAfter = "0",
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                retryAfter = "01",
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                retryAfter = "1\u0661",
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                retryAfter = "18446744073709551616",
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                duplicateLocation = true,
            ),
            acceptedResponse(
                OfflineCashToriiV1Fixtures.topUpReference,
                operationId,
                duplicateRetryAfter = true,
            ),
        )
        for (badResponse in invalidReferences) {
            assertFutureFailure {
                client(RecordingTransport { badResponse }).submitTopUp(topUp, operationId)
            }
        }

        val validReference = client(
            RecordingTransport {
                acceptedResponse(
                    OfflineCashToriiV1Fixtures.topUpReference,
                    operationId,
                    retryAfter = "18446744073709551615",
                )
            },
        ).submitTopUp(topUp, operationId).join()
        assertContentEquals(
            OfflineCashToriiV1Fixtures.topUpReference,
            validReference.encodeCanonical(),
        )

        for (invalidStatus in listOf(
            OfflineCashToriiV1Fixtures.wrongIdStatus,
            OfflineCashToriiV1Fixtures.zeroSubmittedPendingStatus,
            OfflineCashToriiV1Fixtures.zeroHeightStatus,
            OfflineCashToriiV1Fixtures.zeroTimeStatus,
            OfflineCashToriiV1Fixtures.invalidTransactionHashStatus,
            OfflineCashToriiV1Fixtures.foreignNetworkTopUpStatus,
            OfflineCashToriiV1Fixtures.wrongRejectionCodeStatus,
            OfflineCashToriiV1Fixtures.rejectionDetailsStatus,
            OfflineCashToriiV1Fixtures.oversizedRejectionMessageStatus,
        )) {
            assertFutureFailure {
                client(
                    RecordingTransport {
                        response(200, OfflineCashToriiClientV1.NORITO_MEDIA_TYPE, invalidStatus)
                    },
                ).getOperation(operationId)
            }
        }
    }

    @Test
    fun `native operation projection maps pending rejected and applied redeem states`() {
        assertTrue(
            KagemushaRecursiveSpendProver.isArtifactStreamingAvailable(),
            "A freshly built connect_norito_bridge ABI 22 library is required",
        )
        val pendingArchive = OfflineCashToriiV1Fixtures.topUpPendingStatus
        val pendingHeader = NoritoHeader.decode(
            pendingArchive,
            SchemaHash.hash16(OPERATION_STATUS_SCHEMA),
        )
        assertEquals(NoritoHeader.COMPRESSION_NONE, pendingHeader.header.compression)
        assertEquals(NoritoHeader.COMPACT_LEN, pendingHeader.header.flags)
        assertEquals(
            NoritoHeader.HEADER_LENGTH + 8 + pendingHeader.payload.size,
            pendingArchive.size,
        )
        assertContentEquals(
            pendingHeader.header.encode(),
            pendingArchive.copyOfRange(0, NoritoHeader.HEADER_LENGTH),
        )
        val pending = OfflineCashOperationStatusV1(pendingArchive).project()
        assertEquals(OfflineCashOperationStateV1.PENDING, pending.state)
        assertEquals(OfflineCashOperationKindV1.TOP_UP, pending.kind)
        assertContentEquals(
            OfflineCashToriiV1Fixtures.topUpOperationId.hexBytes(),
            pending.operationId(),
        )
        assertContentEquals(ByteArray(32) { 0xc1.toByte() }, pending.transactionHash())
        assertEquals(
            OfflineCashToriiV1Fixtures.topUpSubmittedAtMilliseconds,
            pending.submittedAtMilliseconds,
        )
        assertNull(pending.finalizedBlockHeight)
        assertNull(pending.serverTimeMilliseconds)
        assertNull(pending.finalizedTopUp)
        assertNull(pending.rejection)
        pending.operationId().fill(0)
        assertContentEquals(
            OfflineCashToriiV1Fixtures.topUpOperationId.hexBytes(),
            pending.operationId(),
        )

        val rejected = OfflineCashOperationStatusV1(
            OfflineCashToriiV1Fixtures.rejectedStatus,
        ).project()
        assertEquals(OfflineCashOperationStateV1.REJECTED, rejected.state)
        assertEquals(OfflineCashOperationKindV1.REDEEM, rejected.kind)
        assertEquals(
            OfflineCashOperationRejectionV1.fromValidatedProjection(
                "offline_operation_rejected",
                "rejected",
            ),
            rejected.rejection,
        )
        assertContentEquals(
            OfflineCashToriiV1Fixtures.redeemOperationId.hexBytes(),
            rejected.operationId(),
        )
        assertContentEquals(ByteArray(32) { 0xc3.toByte() }, rejected.transactionHash())
        assertNull(rejected.submittedAtMilliseconds)
        assertNull(rejected.finalizedBlockHeight)

        val applied = OfflineCashOperationStatusV1(
            OfflineCashToriiV1Fixtures.redeemAppliedStatus,
        ).project()
        assertEquals(OfflineCashOperationStateV1.APPLIED, applied.state)
        assertEquals(OfflineCashOperationKindV1.REDEEM, applied.kind)
        assertContentEquals(
            OfflineCashToriiV1Fixtures.redeemOperationId.hexBytes(),
            applied.operationId(),
        )
        assertContentEquals(ByteArray(32) { 0xc3.toByte() }, applied.transactionHash())
        assertEquals(9L, applied.finalizedBlockHeight)
        assertEquals(1_725_000_000_102L, applied.serverTimeMilliseconds)
        assertNull(applied.finalizedTopUp)
        assertNull(applied.rejection)
    }

    @Test
    fun `native applied top-up projection validates every terminal evidence binding`() {
        val status = OfflineCashOperationStatusV1(OfflineCashToriiV1Fixtures.topUpAppliedStatus)
        val projection = status.project()
        assertEquals(OfflineCashOperationStateV1.APPLIED, projection.state)
        assertEquals(OfflineCashOperationKindV1.TOP_UP, projection.kind)
        assertContentEquals(
            OfflineCashToriiV1Fixtures.topUpOperationId.hexBytes(),
            projection.operationId(),
        )
        assertEquals(
            OfflineCashToriiV1Fixtures.topUpFinalizedBlockHeight,
            projection.finalizedBlockHeight,
        )
        assertEquals(
            OfflineCashToriiV1Fixtures.topUpServerTimeMilliseconds,
            projection.serverTimeMilliseconds,
        )
        val finalized = assertNotNull(projection.finalizedTopUp)
        assertEquals(projection.finalizedBlockHeight, finalized.finalizedBlockHeight)
        assertEquals(projection.serverTimeMilliseconds, finalized.serverTimeMilliseconds)
        assertTrue(finalized.anchorCanonical().isNotEmpty())
        assertTrue(finalized.finalityProofCanonical().isNotEmpty())

        for (invalidStatus in listOf(
            OfflineCashToriiV1Fixtures.invalidTopUpAnchorStatus,
            OfflineCashToriiV1Fixtures.invalidTopUpProofStatus,
            OfflineCashToriiV1Fixtures.wrongTopUpOperationStatus,
            OfflineCashToriiV1Fixtures.wrongTopUpTransactionStatus,
            OfflineCashToriiV1Fixtures.wrongTopUpHeightStatus,
            OfflineCashToriiV1Fixtures.wrongTopUpProofNetworkStatus,
            OfflineCashToriiV1Fixtures.wrongTopUpProofAnchorStatus,
            OfflineCashToriiV1Fixtures.wrongTopUpProofHeightStatus,
        )) {
            assertFailsWith<IllegalArgumentException> {
                OfflineCashOperationStatusV1(invalidStatus)
            }
        }
    }

    @Test
    fun `finalized top-up projection exposes only defensive canonical evidence`() {
        val anchorSource = archive(TOP_UP_ANCHOR_SCHEMA, 0x41)
        val proofSource = archive(TOP_UP_FINALITY_PROOF_SCHEMA, 0x42)
        val expectedAnchor = anchorSource.copyOf()
        val expectedProof = proofSource.copyOf()
        val finalized = OfflineCashFinalizedTopUpV1.fromValidatedProjection(
            anchorSource,
            proofSource,
            91,
            92,
        )
        anchorSource.fill(0)
        proofSource.fill(0)
        assertContentEquals(expectedAnchor, finalized.anchorCanonical())
        assertContentEquals(expectedProof, finalized.finalityProofCanonical())
        finalized.anchorCanonical().fill(0)
        finalized.finalityProofCanonical().fill(0)
        assertContentEquals(expectedAnchor, finalized.anchorCanonical())
        assertContentEquals(expectedProof, finalized.finalityProofCanonical())

        val projection = OfflineCashOperationStatusProjectionV1.fromValidatedProjection(
            OfflineCashOperationStateV1.APPLIED,
            OfflineCashOperationKindV1.TOP_UP,
            ByteArray(32) { 1 },
            ByteArray(32) { 2 },
            null,
            91,
            92,
            finalized,
            null,
        )
        assertEquals(finalized, projection.finalizedTopUp)

        assertFailsWith<IllegalArgumentException> {
            OfflineCashFinalizedTopUpV1.fromValidatedProjection(
                archive(TOP_UP_FINALITY_PROOF_SCHEMA),
                expectedProof,
                91,
                92,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationStatusProjectionV1.fromValidatedProjection(
                OfflineCashOperationStateV1.APPLIED,
                OfflineCashOperationKindV1.REDEEM,
                ByteArray(32) { 1 },
                ByteArray(32) { 2 },
                null,
                91,
                92,
                finalized,
                null,
            )
        }
        for ((height, time) in listOf(0L to 92L, 91L to 0L)) {
            assertFailsWith<IllegalArgumentException> {
                OfflineCashOperationStatusProjectionV1.fromValidatedProjection(
                    OfflineCashOperationStateV1.APPLIED,
                    OfflineCashOperationKindV1.REDEEM,
                    ByteArray(32) { 1 },
                    ByteArray(32) { 2 },
                    null,
                    height,
                    time,
                    null,
                    null,
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineCashOperationStatusProjectionV1.fromValidatedProjection(
                OfflineCashOperationStateV1.PENDING,
                OfflineCashOperationKindV1.TOP_UP,
                ByteArray(32) { 1 },
                ByteArray(32) { 2 },
                0,
                null,
                null,
                null,
                null,
            )
        }
    }

    private fun client(
        transport: TransportExecutor,
        baseUri: URI = URI.create("https://torii.example"),
    ): OfflineCashToriiClientV1 = OfflineCashToriiClientV1.create(
        baseUri,
        transport,
        signingContext(),
    )

    private fun signingContext(): LocalSigningContext = LocalSigningContext(
        NetworkId.parse(OfflineCashToriiV1Fixtures.networkId),
    )

    private fun acceptedResponse(
        body: ByteArray,
        operationId: String,
        location: String? = "/v1/offline/operations/$operationId",
        retryAfter: String? = "1",
        duplicateLocation: Boolean = false,
        duplicateRetryAfter: Boolean = false,
    ): TransportResponse = TransportResponse.builder()
        .setStatusCode(202)
        .addHeader("Content-Type", OfflineCashToriiClientV1.NORITO_MEDIA_TYPE)
        .also { builder ->
            if (location != null) {
                builder.addHeader("Location", location)
                if (duplicateLocation) builder.addHeader("location", location)
            }
            if (retryAfter != null) {
                builder.addHeader("Retry-After", retryAfter)
                if (duplicateRetryAfter) builder.addHeader("retry-after", retryAfter)
            }
        }
        .setBody(body)
        .build()

    private fun response(
        status: Int,
        mediaType: String,
        body: String,
        duplicateContentType: Boolean = false,
    ): TransportResponse = response(
        status,
        mediaType,
        body.toByteArray(StandardCharsets.UTF_8),
        duplicateContentType,
    )

    private fun response(
        status: Int,
        mediaType: String,
        body: ByteArray,
        duplicateContentType: Boolean = false,
    ): TransportResponse = TransportResponse.builder()
        .setStatusCode(status)
        .addHeader("Content-Type", mediaType)
        .also { builder ->
            if (duplicateContentType) builder.addHeader("content-type", mediaType)
        }
        .setBody(body)
        .build()

    private fun assertFutureFailure(action: () -> CompletableFuture<*>) {
        val failure = assertFailsWith<CompletionException> { action().join() }
        assertTrue(failure.cause is RuntimeException, "unexpected future failure: $failure")
    }

    private fun archive(schema: String, marker: Int = 0x51): ByteArray {
        val payload = byteArrayOf(marker.toByte())
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        )
        val padding = when (schema) {
            OPERATION_STATUS_SCHEMA,
            TOP_UP_ANCHOR_SCHEMA,
            -> ByteArray(8)
            else -> byteArrayOf()
        }
        return header.encode() + padding + payload
    }

    private fun fixtureValue(rows: List<String>, name: String): String =
        rows.single { it.startsWith("$name=") }.substringAfter('=')

    private fun replaceFixtureRow(
        rows: List<String>,
        name: String,
        value: String,
    ): List<String> = rows.map { row ->
        if (row.startsWith("$name=")) "$name=$value" else row
    }

    private fun String.hexBytes(): ByteArray {
        require(length % 2 == 0)
        return ByteArray(length / 2) { index ->
            substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private class RecordingTransport(
        private val responder: (TransportRequest) -> TransportResponse,
    ) : TransportExecutor {
        val requests = mutableListOf<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests += request
            return CompletableFuture.completedFuture(responder(request))
        }
    }

    private companion object {
        const val TOP_UP_SCHEMA = "iroha.torii.v1.offline.top_up.request"
        const val REDEEM_SCHEMA = "iroha.torii.v1.offline.redeem.request"
        const val OPERATION_REFERENCE_SCHEMA =
            "iroha_torii_shared::offline_api::OfflineOperationReference"
        const val OPERATION_STATUS_SCHEMA =
            "iroha_torii_shared::offline_api::OfflineOperationStatus"
        const val TOP_UP_ANCHOR_SCHEMA =
            "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpAnchorV4"
        const val TOP_UP_FINALITY_PROOF_SCHEMA =
            "iroha_data_model::offline::model::KagemushaTopUpFinalityProofV2"

        const val READINESS_JSON =
            "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\"," +
                "\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":false," +
                "\"assets\":[],\"blockers\":[" +
                "{\"code\":\"offline_cash_authenticated_release_unavailable\"," +
                "\"message\":\"No authenticated Offline Cash V1 release is selected " +
                "by this asset-neutral response.\"}," +
                "{\"code\":\"offline_cash_eligible_asset_unavailable\"," +
                "\"message\":\"No eligible Offline Cash V1 asset is selected by this asset-neutral response.\"}," +
                "{\"code\":\"offline_cash_proof_backend_unavailable\"," +
                "\"message\":\"No reviewed production Offline Cash V1 proof and secure-device " +
                "backend is authenticated by this response.\"}]}"

    }
}
