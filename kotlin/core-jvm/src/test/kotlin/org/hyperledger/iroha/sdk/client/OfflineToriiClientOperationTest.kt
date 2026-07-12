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
import org.hyperledger.iroha.sdk.offline.OfflineOperationCodec
import org.hyperledger.iroha.sdk.offline.OfflineOperationKind
import org.hyperledger.iroha.sdk.offline.OfflineOperationReference
import org.hyperledger.iroha.sdk.offline.OfflineOperationState
import org.hyperledger.iroha.sdk.offline.OfflineOperationStatus
import org.hyperledger.iroha.sdk.offline.OfflineRedeemRequest
import org.hyperledger.iroha.sdk.offline.OfflineTopUpRequest
import org.hyperledger.iroha.sdk.offline.OfflineToriiException
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

class OfflineToriiClientOperationTest {
    @Test
    fun operationReferenceMatchesRustGoldenArchive() {
        val archive = hexBytes(RUST_OPERATION_REFERENCE_HEX)
        val decoded = OfflineOperationCodec.decodeReference(archive)
        val operationId = "1".repeat(64)

        assertEquals(operationId, decoded.operationId)
        assertEquals(OfflineOperationKind.TOP_UP, decoded.kind)
        assertEquals(OfflineOperationState.PENDING, decoded.state)
        assertEquals(TRANSACTION_HASH, decoded.transactionHash)
        assertEquals("/v1/offline/operations/$operationId", decoded.statusUri)
        assertEquals(BigInteger("18446744073709551615"), decoded.submittedAtMs)
        assertContentEquals(archive, OfflineOperationCodec.encodeReference(decoded))
    }

    @Test
    fun operationStatusesMatchRustGoldenArchives() {
        val operationId = "1".repeat(64)

        val pendingArchive = hexBytes(RUST_PENDING_STATUS_HEX)
        val pending = OfflineOperationCodec.decodeStatus(pendingArchive) as OfflineOperationStatus.Pending
        assertEquals(operationId, pending.operationId)
        assertEquals(OfflineOperationKind.TOP_UP, pending.kind)
        assertEquals(TRANSACTION_HASH, pending.transactionHash)
        assertEquals(BigInteger("18446744073709551615"), pending.submittedAtMs)
        assertContentEquals(pendingArchive, OfflineOperationCodec.encodeStatus(pending))
        val wrongSchema = pendingArchive.copyOf().also { it[6] = (it[6].toInt() xor 1).toByte() }
        assertFailsWith<IllegalArgumentException> {
            OfflineOperationCodec.decodeStatus(wrongSchema)
        }

        val rejectedArchive = hexBytes(RUST_REJECTED_STATUS_HEX)
        val rejected = OfflineOperationCodec.decodeStatus(rejectedArchive) as OfflineOperationStatus.Rejected
        assertEquals(operationId, rejected.operationId)
        assertEquals(OfflineOperationKind.REDEEM, rejected.kind)
        assertEquals("offline_operation_rejected", rejected.error.code)
        assertEquals("rejected", rejected.error.message)
        assertEquals(null, rejected.error.details)
        assertContentEquals(rejectedArchive, OfflineOperationCodec.encodeStatus(rejected))

        val appliedArchive = hexBytes(RUST_APPLIED_REDEEM_STATUS_HEX)
        val applied = OfflineOperationCodec.decodeStatus(appliedArchive) as OfflineOperationStatus.Applied
        assertEquals(operationId, applied.operationId)
        val result = (applied.result as OfflineOperationStatus.Result.Redeem).value
        assertEquals(TRANSACTION_HASH, result.transactionHash)
        assertEquals(BigInteger("18446744073709551615"), result.finalizedBlockHeight)
        assertEquals(BigInteger.valueOf(42), result.serverTimeMs)
        assertContentEquals(appliedArchive, OfflineOperationCodec.encodeStatus(applied))
    }

    @Test
    fun typedOperationStatusesRoundTrip() {
        val operationId = "11".repeat(32)
        val pending = OfflineOperationStatus.Pending(
            operationId,
            OfflineOperationKind.TOP_UP,
            TRANSACTION_HASH,
            BigInteger("18446744073709551615"),
        )
        val decodedPending = OfflineOperationCodec.decodeStatus(
            OfflineOperationCodec.encodeStatus(pending),
        ) as OfflineOperationStatus.Pending
        assertEquals(operationId, decodedPending.operationId)
        assertEquals(OfflineOperationKind.TOP_UP, decodedPending.kind)
        assertEquals(BigInteger("18446744073709551615"), decodedPending.submittedAtMs)

        val applied = OfflineOperationStatus.Applied(
            operationId,
            OfflineOperationStatus.Result.Redeem(
                OfflineOperationStatus.RedeemResult(
                    TRANSACTION_HASH,
                    BigInteger("18446744073709551615"),
                    BigInteger.valueOf(42),
                ),
            ),
        )
        val decodedApplied = OfflineOperationCodec.decodeStatus(
            OfflineOperationCodec.encodeStatus(applied),
        ) as OfflineOperationStatus.Applied
        val redeemResult = (decodedApplied.result as OfflineOperationStatus.Result.Redeem).value
        assertEquals(BigInteger("18446744073709551615"), redeemResult.finalizedBlockHeight)
        assertEquals(BigInteger.valueOf(42), redeemResult.serverTimeMs)

        val anchorArchive = opaqueArchive(TOP_UP_ANCHOR_SCHEMA, byteArrayOf(1, 2, 3))
        val finalityProofArchive = opaqueArchive(
            TOP_UP_FINALITY_PROOF_SCHEMA,
            byteArrayOf(4, 5, 6),
        )
        val appliedTopUp = OfflineOperationStatus.Applied(
            operationId,
            OfflineOperationStatus.Result.TopUp(
                OfflineOperationStatus.TopUpResult(
                    TRANSACTION_HASH,
                    BigInteger.valueOf(7),
                    BigInteger.valueOf(42),
                    OfflineOperationCodec.decodeTopUpAnchor(anchorArchive),
                    OfflineOperationCodec.decodeTopUpFinalityProof(finalityProofArchive),
                ),
            ),
        )
        val decodedTopUp = (
            OfflineOperationCodec.decodeStatus(
                OfflineOperationCodec.encodeStatus(appliedTopUp),
            ) as OfflineOperationStatus.Applied
        ).result as OfflineOperationStatus.Result.TopUp
        assertContentEquals(anchorArchive, decodedTopUp.value.anchor.noritoArchive())
        assertContentEquals(
            finalityProofArchive,
            decodedTopUp.value.finalityProof.noritoArchive(),
        )
        val proofCopy = decodedTopUp.value.finalityProof.noritoArchive()
        proofCopy[0] = (proofCopy[0].toInt() xor 0x7f).toByte()
        assertEquals('N'.code.toByte(), decodedTopUp.value.finalityProof.noritoArchive()[0])

        val details = OfflineOperationStatus.ErrorDetails(
            layer = "torii",
            rejectCode = "policy_rejected",
            queue = OfflineOperationStatus.QueueErrorSnapshot(
                "saturated",
                BigInteger.TEN,
                BigInteger.TEN,
                true,
            ),
            retryAfterSeconds = BigInteger.valueOf(5),
            endpoint = "/v1/offline/redeem",
            field = "proof",
            expected = "valid",
            actual = "invalid",
            profile = "taira",
            chainDiscriminant = 369,
            transactionHash = TRANSACTION_HASH,
            lastStatus = "rejected",
            hint = "refresh proof",
            axt = OfflineOperationStatus.AxtErrorDetails(
                "axt_rejected",
                "policy",
                BigInteger.ONE,
                BigInteger.valueOf(2),
                3,
                BigInteger.valueOf(4),
                BigInteger.valueOf(5),
            ),
        )
        val rejected = OfflineOperationStatus.Rejected(
            operationId,
            OfflineOperationKind.REDEEM,
            TRANSACTION_HASH,
            OfflineOperationStatus.Error("rejected", "Transaction rejected", details),
        )
        val decodedRejected = OfflineOperationCodec.decodeStatus(
            OfflineOperationCodec.encodeStatus(rejected),
        ) as OfflineOperationStatus.Rejected
        assertEquals(operationId, decodedRejected.operationId)
        assertEquals(OfflineOperationKind.REDEEM, decodedRejected.kind)
        assertEquals("rejected", decodedRejected.error.code)
        assertEquals("Transaction rejected", decodedRejected.error.message)
        assertEquals("policy_rejected", decodedRejected.error.details?.rejectCode)
        assertEquals(BigInteger.TEN, decodedRejected.error.details?.queue?.capacity)
        assertEquals(3L, decodedRejected.error.details?.axt?.lane)
    }

    @Test
    fun appliedResultsRejectZeroFinalityFields() {
        val anchor = OfflineOperationStatus.TopUpAnchor(byteArrayOf(1))
        val finalityProof = OfflineOperationStatus.TopUpFinalityProof(byteArrayOf(1))
        for ((finalizedBlockHeight, serverTimeMs) in listOf(
            BigInteger.ZERO to BigInteger.ONE,
            BigInteger.ONE to BigInteger.ZERO,
        )) {
            assertFailsWith<IllegalArgumentException> {
                OfflineOperationStatus.TopUpResult(
                    TRANSACTION_HASH,
                    finalizedBlockHeight,
                    serverTimeMs,
                    anchor,
                    finalityProof,
                )
            }
            assertFailsWith<IllegalArgumentException> {
                OfflineOperationStatus.RedeemResult(
                    TRANSACTION_HASH,
                    finalizedBlockHeight,
                    serverTimeMs,
                )
            }
        }
    }

    @Test
    fun topUpPostsCanonicalNoritoArchive() {
        val reference = reference(OfflineOperationKind.TOP_UP)
        val responseArchive = OfflineOperationCodec.encodeReference(reference)
        val requestArchive = topUpRequestArchive(ByteArray(32) { 0x11 })
        val expectedRequestArchive = requestArchive.copyOf()
        val executor = CapturingExecutor(responseArchive)
        val client = client(executor)

        val actual = client.submitOfflineTopUp(
            OfflineTopUpRequest(requestArchive),
        ).join()
        requestArchive.fill(0)

        assertEquals(reference, actual)
        assertEquals("POST", executor.lastRequest.method)
        assertEquals("/v1/offline/top-up", executor.lastRequest.uri.path)
        assertContentEquals(expectedRequestArchive, executor.lastRequest.body)
        assertEquals("application/x-norito", firstHeader(executor.lastRequest, "Content-Type"))
        assertEquals("application/x-norito", firstHeader(executor.lastRequest, "Accept"))
        assertEquals(reference.operationId, firstHeader(executor.lastRequest, "Idempotency-Key"))
    }

    @Test
    fun redeemAndOperationStatusUseCanonicalPaths() {
        val reference = reference(OfflineOperationKind.REDEEM)
        val responseArchive = OfflineOperationCodec.encodeReference(reference)
        val requestArchive = redeemRequestArchive(ByteArray(32) { 0x11 })
        val executor = CapturingExecutor(responseArchive)
        val client = client(executor)

        assertEquals(
            reference,
            client.submitOfflineRedeem(OfflineRedeemRequest(requestArchive)).join(),
        )
        assertEquals("/v1/offline/redeem", executor.lastRequest.uri.path)
        assertContentEquals(requestArchive, executor.lastRequest.body)

        executor.statusCode = 200
        executor.responseBody = OfflineOperationCodec.encodeStatus(
            OfflineOperationStatus.Pending(
                reference.operationId,
                reference.kind,
                reference.transactionHash,
                reference.submittedAtMs,
            ),
        )
        val status = client.getOfflineOperationStatus(reference.operationId).join()
        assertEquals("GET", executor.lastRequest.method)
        assertEquals(
            "/v1/offline/operations/${reference.operationId}",
            executor.lastRequest.uri.path,
        )
        assertEquals("application/x-norito", firstHeader(executor.lastRequest, "Accept"))
        assertEquals(reference.operationId, status.operationId)
        assertEquals(reference.kind, (status as OfflineOperationStatus.Pending).kind)
    }

    @Test
    fun requestsDeriveAndValidateCanonicalOperationIds() {
        val operationIdBytes = ByteArray(32) { index -> (index + 1).toByte() }
        operationIdBytes[0] = 0xAB.toByte()
        operationIdBytes[1] = 0xCD.toByte()
        val topUpArchive = topUpRequestArchive(operationIdBytes)
        val expectedTopUpArchive = topUpArchive.copyOf()
        val topUp = OfflineTopUpRequest(topUpArchive)
        assertEquals(lowercaseHex(operationIdBytes), topUp.operationId)
        topUpArchive.fill(0)
        assertContentEquals(expectedTopUpArchive, topUp.noritoArchive())
        val returnedArchive = topUp.noritoArchive()
        returnedArchive.fill(0)
        assertContentEquals(expectedTopUpArchive, topUp.noritoArchive())

        val redeem = OfflineRedeemRequest(redeemRequestArchive(operationIdBytes))
        assertEquals(lowercaseHex(operationIdBytes), redeem.operationId)

        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(redeemRequestArchive(operationIdBytes))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineRedeemRequest(redeemRequestArchive(ByteArray(32)))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(topUpRequestArchive(ByteArray(31) { 1 }))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA,
                    fieldCount = 7,
                    operationIdFieldIndex = 4,
                    operationId = operationIdBytes,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA,
                    fieldCount = 8,
                    operationIdFieldIndex = 5,
                    operationId = operationIdBytes,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA,
                    fieldCount = 7,
                    operationIdFieldIndex = 5,
                    operationId = operationIdBytes,
                    trailingBytes = byteArrayOf(0x7F),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineRedeemRequest(
                canonicalRequestArchive(
                    REDEEM_REQUEST_SCHEMA,
                    fieldCount = 10,
                    operationIdFieldIndex = 9,
                    operationId = operationIdBytes,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA,
                    fieldCount = 7,
                    operationIdFieldIndex = 5,
                    operationId = operationIdBytes,
                    flags = 0,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(withHeaderPadding(topUpRequestArchive(operationIdBytes)))
        }
    }

    @Test
    fun operationModelsRejectNonCanonicalResponseFields() {
        val operationId = "11".repeat(32)
        assertFailsWith<IllegalArgumentException> {
            OfflineOperationReference(
                operationId,
                OfflineOperationKind.TOP_UP,
                OfflineOperationState.PENDING,
                "ab".repeat(32).uppercase(),
                "/v1/offline/operations/$operationId",
                BigInteger.ZERO,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineOperationReference(
                operationId,
                OfflineOperationKind.TOP_UP,
                OfflineOperationState.PENDING,
                TRANSACTION_HASH,
                "/v1/offline/operations/${"33".repeat(32)}",
                BigInteger.ZERO,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineOperationStatus.Pending(
                operationId,
                OfflineOperationKind.TOP_UP,
                "2".repeat(63),
                BigInteger.ZERO,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineOperationStatus.Error("Bad-Code", "rejected", null)
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineOperationStatus.Error("a".repeat(65), "rejected", null)
        }
    }

    @Test
    fun referenceDecoderRejectsMalformedUtf8WithoutReplacement() {
        val archive = OfflineOperationCodec.encodeReference(reference(OfflineOperationKind.TOP_UP))
        val marker = ByteArray(64) { '2'.code.toByte() }
        val offset = archive.indexOf(marker)
        require(offset >= NoritoHeader.HEADER_LENGTH)
        archive[offset] = 0xc3.toByte()
        archive[offset + 1] = 0x28
        rewritePayloadChecksum(archive, NoritoHeader.HEADER_LENGTH)

        assertFailsWith<IllegalArgumentException> {
            OfflineOperationCodec.decodeReference(archive)
        }
    }

    @Test
    fun submissionBindsTypedReferenceAndResponseHeaders() {
        val operationIdBytes = ByteArray(32) { 0x11 }
        val request = OfflineTopUpRequest(topUpRequestArchive(operationIdBytes))
        val canonical = reference(OfflineOperationKind.TOP_UP)
        val executor = CapturingExecutor(OfflineOperationCodec.encodeReference(canonical))
        val client = client(executor)

        executor.responseBody = OfflineOperationCodec.encodeReference(
            reference(OfflineOperationKind.REDEEM),
        )
        assertOfflineClientRejects { client.submitOfflineTopUp(request).join() }

        val otherOperationId = "33".repeat(32)
        executor.responseBody = OfflineOperationCodec.encodeReference(
            OfflineOperationReference(
                otherOperationId,
                OfflineOperationKind.TOP_UP,
                OfflineOperationState.PENDING,
                TRANSACTION_HASH,
                "/v1/offline/operations/$otherOperationId",
                BigInteger.ZERO,
            ),
        )
        executor.responseHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
            "Location" to listOf("/v1/offline/operations/$otherOperationId"),
        )
        assertOfflineClientRejects { client.submitOfflineTopUp(request).join() }

        executor.responseBody = OfflineOperationCodec.encodeReference(canonical)
        executor.responseHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
        )
        assertOfflineClientRejects { client.submitOfflineTopUp(request).join() }

        executor.responseHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
            "Location" to listOf(canonical.statusUri, canonical.statusUri),
        )
        assertOfflineClientRejects { client.submitOfflineTopUp(request).join() }

        executor.responseHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
            "Location" to listOf("${canonical.statusUri}/extra"),
        )
        assertOfflineClientRejects { client.submitOfflineTopUp(request).join() }

        executor.responseHeaders = mapOf(
            "Content-Type" to listOf("application/json"),
            "Location" to listOf(canonical.statusUri),
        )
        assertOfflineClientRejects { client.submitOfflineTopUp(request).join() }

        executor.statusCode = 200
        executor.responseBody = OfflineOperationCodec.encodeStatus(
            OfflineOperationStatus.Pending(
                otherOperationId,
                OfflineOperationKind.TOP_UP,
                TRANSACTION_HASH,
                BigInteger.ZERO,
            ),
        )
        executor.responseHeaders = mapOf(
            "Content-Type" to listOf("application/x-norito"),
        )
        assertOfflineClientRejects { client.getOfflineOperationStatus(canonical.operationId).join() }
    }

    private fun client(executor: HttpTransportExecutor): OfflineToriiClient =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

    private fun reference(kind: OfflineOperationKind): OfflineOperationReference =
        OfflineOperationReference(
            operationId = "11".repeat(32),
            kind = kind,
            state = OfflineOperationState.PENDING,
            transactionHash = TRANSACTION_HASH,
            statusUri = "/v1/offline/operations/${"11".repeat(32)}",
            submittedAtMs = BigInteger("18446744073709551615"),
        )

    private class CapturingExecutor(
        responseBody: ByteArray,
    ) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var statusCode: Int = 202
        var responseBody: ByteArray = responseBody.copyOf()
        var responseHeaders: Map<String, List<String>> = mapOf(
            "Content-Type" to listOf("application/x-norito"),
            "Location" to listOf("/v1/offline/operations/${"11".repeat(32)}"),
        )

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(statusCode)
                    .setBody(responseBody.copyOf())
                    .setHeaders(responseHeaders)
                    .build(),
            )
        }
    }

    private fun firstHeader(request: TransportRequest, name: String): String? = request.headers
        .entries
        .firstOrNull { it.key.equals(name, ignoreCase = true) }
        ?.value
        ?.firstOrNull()

    private fun assertOfflineClientRejects(action: () -> Unit) {
        val error = assertFailsWith<CompletionException> { action() }
        assertTrue(error.cause is OfflineToriiException)
    }

    private fun ByteArray.indexOf(needle: ByteArray): Int {
        if (needle.isEmpty() || needle.size > size) return -1
        for (offset in 0..size - needle.size) {
            if (needle.indices.all { index -> this[offset + index] == needle[index] }) {
                return offset
            }
        }
        return -1
    }

    private fun rewritePayloadChecksum(archive: ByteArray, payloadOffset: Int) {
        val checksum = CRC64.compute(archive.copyOfRange(payloadOffset, archive.size))
        repeat(Long.SIZE_BYTES) { index ->
            archive[31 + index] = (checksum ushr (index * 8)).toByte()
        }
    }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0)
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun topUpRequestArchive(operationId: ByteArray): ByteArray =
        canonicalRequestArchive(
            TOP_UP_REQUEST_SCHEMA,
            fieldCount = 7,
            operationIdFieldIndex = 5,
            operationId = operationId,
        )

    private fun redeemRequestArchive(operationId: ByteArray): ByteArray =
        canonicalRequestArchive(
            REDEEM_REQUEST_SCHEMA,
            fieldCount = 11,
            operationIdFieldIndex = 9,
            operationId = operationId,
        )

    private fun canonicalRequestArchive(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: ByteArray,
        trailingBytes: ByteArray = ByteArray(0),
        flags: Int = NoritoHeader.COMPACT_LEN,
    ): ByteArray {
        val encoder = NoritoEncoder(flags)
        repeat(fieldCount) { fieldIndex ->
            val field = if (fieldIndex == operationIdFieldIndex) {
                operationId
            } else {
                byteArrayOf((fieldIndex + 1).toByte())
            }
            encoder.writeLength(
                field.size.toLong(),
                (flags and NoritoHeader.COMPACT_LEN) != 0,
            )
            encoder.writeBytes(field)
        }
        val payload = encoder.toByteArray() + trailingBytes
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            payload.size,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return header + payload
    }

    private fun opaqueArchive(schema: String, payload: ByteArray): ByteArray {
        val body = payload.copyOf()
        val header = NoritoHeader(
            SchemaHash.hash16(schema),
            body.size,
            CRC64.compute(body),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return header + body
    }

    private fun withHeaderPadding(archive: ByteArray): ByteArray {
        val padded = ByteArray(archive.size + 1)
        archive.copyInto(padded, endIndex = NoritoHeader.HEADER_LENGTH)
        archive.copyInto(
            padded,
            destinationOffset = NoritoHeader.HEADER_LENGTH + 1,
            startIndex = NoritoHeader.HEADER_LENGTH,
        )
        return padded
    }

    private fun lowercaseHex(value: ByteArray): String = value.joinToString("") {
        "%02x".format(it.toInt() and 0xFF)
    }

    private companion object {
        val TRANSACTION_HASH: String = "22".repeat(32)

        const val TOP_UP_REQUEST_SCHEMA =
            "iroha.torii.v1.offline.top_up.request"
        const val REDEEM_REQUEST_SCHEMA =
            "iroha.torii.v1.offline.redeem.request"
        const val TOP_UP_ANCHOR_SCHEMA =
            "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpAnchorV2"
        const val TOP_UP_FINALITY_PROOF_SCHEMA =
            "iroha_data_model::offline::model::KagemushaTopUpFinalityProofV2"

        const val RUST_OPERATION_REFERENCE_HEX =
            "4e5254300000e8e2244e45e4be2a975e34957141128b00f0000000000000001f5b5402d6dc2092024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323258572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff"

        const val RUST_PENDING_STATUS_HEX =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a009600000000000000bdfee2508f80055702000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff"

        const val RUST_REJECTED_STATUS_HEX =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a00b6000000000000009322104cda8e602a020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100"

        const val RUST_APPLIED_REDEEM_STATUS_HEX =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a00a00000000000000092cd6b32b062b3d30200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313159010000005441403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff082a00000000000000"
    }
}
