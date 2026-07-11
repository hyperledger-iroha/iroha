package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.offline.OfflineOperationCodec
import org.hyperledger.iroha.sdk.offline.OfflineOperationKind
import org.hyperledger.iroha.sdk.offline.OfflineOperationReference
import org.hyperledger.iroha.sdk.offline.OfflineOperationState
import org.hyperledger.iroha.sdk.offline.OfflineOperationStatus
import org.hyperledger.iroha.sdk.offline.OfflineRedeemRequest
import org.hyperledger.iroha.sdk.offline.OfflineTopUpRequest
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
        assertEquals("transaction-hash", decoded.transactionHash)
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
        assertEquals("transaction-hash", pending.transactionHash)
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
        assertEquals("transaction-hash", result.transactionHash)
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
            "transaction-hash",
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
                    "transaction-hash",
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
            transactionHash = "transaction-hash",
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
            "transaction-hash",
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
    fun topUpPostsCanonicalNoritoArchive() {
        val reference = reference(OfflineOperationKind.TOP_UP)
        val responseArchive = OfflineOperationCodec.encodeReference(reference)
        val requestArchive = topUpRequestArchive(ByteArray(32) { 0x11 })
        val expectedRequestArchive = requestArchive.copyOf()
        val executor = CapturingExecutor(responseArchive)
        val client = client(executor)

        val actual = client.submitTopUp(
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
            client.submitRedeem(OfflineRedeemRequest(requestArchive)).join(),
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
        val status = client.getOperationStatus(reference.operationId).join()
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
                    fieldCount = 9,
                    operationIdFieldIndex = 6,
                    operationId = operationIdBytes,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA,
                    fieldCount = 8,
                    operationIdFieldIndex = 6,
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
                    fieldCount = 8,
                    operationIdFieldIndex = 6,
                    operationId = operationIdBytes,
                    flags = 0,
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineTopUpRequest(withHeaderPadding(topUpRequestArchive(operationIdBytes)))
        }
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
            transactionHash = "transaction-hash",
            statusUri = "/v1/offline/operations/${"11".repeat(32)}",
            submittedAtMs = BigInteger("18446744073709551615"),
        )

    private class CapturingExecutor(
        responseBody: ByteArray,
    ) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var statusCode: Int = 202
        var responseBody: ByteArray = responseBody.copyOf()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(statusCode)
                    .setBody(responseBody.copyOf())
                    .build(),
            )
        }
    }

    private fun firstHeader(request: TransportRequest, name: String): String? = request.headers
        .entries
        .firstOrNull { it.key.equals(name, ignoreCase = true) }
        ?.value
        ?.firstOrNull()

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0)
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun topUpRequestArchive(operationId: ByteArray): ByteArray =
        canonicalRequestArchive(
            TOP_UP_REQUEST_SCHEMA,
            fieldCount = 8,
            operationIdFieldIndex = 6,
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
        const val TOP_UP_REQUEST_SCHEMA =
            "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV2"
        const val REDEEM_REQUEST_SCHEMA =
            "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV2"

        const val RUST_OPERATION_REFERENCE_HEX =
            "4e5254300000e8e2244e45e4be2a975e34957141128b00c000000000000000fe" +
                "8a8b6e958d244702414031313131313131313131313131313131313131313131" +
                "3131313131313131313131313131313131313131313131313131313131313131" +
                "313131313131313131310400000000040000000011107472616e73616374696f" +
                "6e2d6861736858572f76312f6f66666c696e652f6f7065726174696f6e732f31" +
                "3131313131313131313131313131313131313131313131313131313131313131" +
                "3131313131313131313131313131313131313131313131313131313131313108" +
                "ffffffffffffffff"

        const val RUST_PENDING_STATUS_HEX =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a006600000000000000b3fae818809b7b8e02000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000011107472616e73616374696f6e2d6861736808ffffffffffffffff"

        const val RUST_REJECTED_STATUS_HEX =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a0086000000000000008878a32fe86d887302000000000000000002000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040100000011107472616e73616374696f6e2d68617368281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100"

        const val RUST_APPLIED_REDEEM_STATUS_HEX =
            "4e5254300000fb04214104df1bdcd39249bddd4db23a007000000000000000451e52608aefd9710200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313129010000002411107472616e73616374696f6e2d6861736808ffffffffffffffff082a00000000000000"
    }
}
