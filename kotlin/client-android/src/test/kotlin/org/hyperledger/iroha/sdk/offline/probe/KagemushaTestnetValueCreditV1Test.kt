// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class KagemushaTestnetValueCreditV1Test {
    private val operationId = ByteArray(32) { 7 }

    @Test
    fun `credit contract rejects native drift`() {
        for (index in 0 until 3) {
            val endpoint = Endpoint().apply { contractWords[index]++ }
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
            }
            assertEquals(0, endpoint.calls)
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetValueCreditV1.openEndpoint(Endpoint().apply { missingContract = true })
        }
    }

    @Test
    fun `malformed operation never reaches native ledger`() {
        val endpoint = Endpoint()
        val credit = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
        for (id in listOf(ByteArray(0), ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> { credit.creditFinalizedValue(id) }
        }
        assertEquals(0, endpoint.calls)
    }

    @Test
    fun `credit exposes exact atomic and decimal value in its release scope`() {
        val endpoint = Endpoint()
        val credit = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
        assertEquals(356, endpoint.archive.size)
        assertEquals(8, CREDIT_TEST_PADDING_BYTES_V1)
        val counted = credit.creditFinalizedValue(operationId)
        assertContentEquals(ByteArray(32) { 7 }, operationId)
        assertTrue(endpoint.direct)
        assertEquals(512, endpoint.capacity)
        assertEquals(1, endpoint.calls)
        assertEquals("01".repeat(32), counted.scope.networkIdHex)
        assertEquals("02".repeat(32), counted.scope.releaseIdHex)
        assertEquals("03".repeat(32), counted.scope.releaseAttestationDigestHex)
        assertEquals("04".repeat(32), counted.scope.assetIdentityDigestHex)
        assertEquals("05".repeat(32), counted.scope.assetIncarnationHex)
        assertEquals(2, counted.scope.assetScale)
        assertEquals("06".repeat(32), counted.scope.liabilityPoolIdHex)
        assertEquals("07".repeat(32), counted.operationIdHex)
        assertEquals("08".repeat(32), counted.creditIdHex)
        assertEquals(BigInteger.valueOf(17), counted.amountAtomic)
        assertEquals(BigInteger.valueOf(12345), counted.totalAdmittedAtomic)
        assertEquals("0.17", counted.amountDecimal().toPlainString())
        assertEquals("123.45", counted.totalAdmittedDecimal().toPlainString())
        assertTrue(counted.testnetOnly)
        assertFalse(counted.hardwareQualified)
        assertFalse(counted.productionMonetaryAuthorized)
    }

    @Test
    fun `credit preserves full unsigned u128 precision`() {
        val amount = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)
        val total = amount
        val endpoint = Endpoint().apply {
            archive = creditArchive(amount = amount, total = total, scale = 28)
        }
        val counted = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
            .creditFinalizedValue(operationId)
        assertEquals(amount, counted.amountAtomic)
        assertEquals(total, counted.totalAdmittedAtomic)
        assertEquals(28, counted.scope.assetScale)
        assertEquals("34028236692.0938463463374607431768211455",
            counted.amountDecimal().toPlainString())
    }

    @Test
    fun `credit preserves native rejection and unavailable status`() {
        val endpoint = Endpoint()
        val credit = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
        for (status in listOf(-312, -311, 0, 513)) {
            endpoint.status = status
            assertEquals(status, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
                credit.creditFinalizedValue(operationId)
            }.status)
        }
        endpoint.missingCredit = true
        assertFailsWith<IllegalStateException> { credit.creditFinalizedValue(operationId) }
    }

    @Test
    fun `credit rejects malformed noncanonical and wrong-schema frames`() {
        val valid = creditArchive()
        val wrongSchema = creditArchive(schema = "wrong.schema")
        val wrongFlags = creditArchive(flags = 0)
        val wrongPrefix = creditArchive(lengthOverride = 3)
        val corruptedChecksum = valid.copyOf().apply { this[lastIndex] = (this[lastIndex].toInt() xor 1).toByte() }
        val wrongPadding = valid.copyOf().apply { this[NoritoHeader.HEADER_LENGTH] = 1 }
        val wrongOperation = creditArchive(operation = ByteArray(32) { 9 })
        for (archive in listOf(
            valid.copyOf(valid.size - 1), valid + byteArrayOf(0),
            wrongSchema, wrongFlags, wrongPrefix, corruptedChecksum, wrongPadding, wrongOperation,
        )) {
            val endpoint = Endpoint().apply { this.archive = archive }
            val credit = KagemushaTestnetValueCreditV1.openEndpoint(endpoint)
            assertFailsWith<IllegalArgumentException> { credit.creditFinalizedValue(operationId) }
        }
    }

    @Test
    fun `credit rejects version hardware qualification and invalid value facts`() {
        val invalid = listOf(
            creditArchive(version = 2),
            creditArchive(hardwareQualified = true),
            creditArchive(scale = 29),
            creditArchive(amount = BigInteger.ZERO),
            creditArchive(amount = BigInteger.valueOf(18), total = BigInteger.valueOf(17)),
            creditArchive(network = ByteArray(32)),
        )
        for (archive in invalid) {
            val credit = KagemushaTestnetValueCreditV1.openEndpoint(Endpoint().apply { this.archive = archive })
            assertFailsWith<IllegalArgumentException> { credit.creditFinalizedValue(operationId) }
        }
    }

    private class Endpoint : KagemushaTestnetValueCreditEndpointV1 {
        val contractWords = intArrayOf(1, 32, 512)
        var archive = creditArchive()
        var status: Int? = null
        var calls = 0
        var direct = false
        var capacity = 0
        var missingContract = false
        var missingCredit = false

        override fun contract(): IntArray? {
            if (missingContract) throw UnsatisfiedLinkError("missing contract")
            return contractWords.copyOf()
        }

        override fun credit(operationId: ByteArray, output: ByteBuffer): Int {
            if (missingCredit) throw UnsatisfiedLinkError("missing credit")
            calls++
            operationId.fill(0)
            direct = output.isDirect
            capacity = output.capacity()
            if (archive.size <= output.remaining()) output.put(archive)
            return status ?: archive.size
        }
    }
}

private const val CREDIT_TEST_SCHEMA_V1 =
    "connect_norito_bridge::KagemushaTestnetMintLedgerCreditArchiveV1"
private const val CREDIT_TEST_PADDING_BYTES_V1 =
    (16 - NoritoHeader.HEADER_LENGTH % 16) % 16

private val CREDIT_TEST_PAYLOAD_ADAPTER = object : TypeAdapter<ByteArray> {
    override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
    override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
}

private fun creditArchive(
    schema: String = CREDIT_TEST_SCHEMA_V1,
    flags: Int = NoritoHeader.COMPACT_LEN,
    version: Int = 1,
    hardwareQualified: Boolean = false,
    scale: Int = 2,
    amount: BigInteger = BigInteger.valueOf(17),
    total: BigInteger = BigInteger.valueOf(12345),
    network: ByteArray = ByteArray(32) { 1 },
    operation: ByteArray = ByteArray(32) { 7 },
    lengthOverride: Int? = null,
): ByteArray {
    val payload = ByteArrayOutputStream()
    fun field(bytes: ByteArray) {
        payload.write(bytes.size)
        payload.write(bytes)
    }
    fun fixed32(value: Int) = ByteArray(32) { value.toByte() }
    fun u128(value: BigInteger): ByteArray = ByteArray(16).also { result ->
        require(value.signum() >= 0 && value.bitLength() <= 128)
        val bigEndian = value.toByteArray()
        val magnitude = if (bigEndian.size == 17 && bigEndian[0] == 0.toByte()) {
            bigEndian.copyOfRange(1, bigEndian.size)
        } else {
            bigEndian
        }
        magnitude.reversedArray().copyInto(result)
    }
    field(ByteBuffer.allocate(2).order(ByteOrder.LITTLE_ENDIAN).putShort(version.toShort()).array())
    field(byteArrayOf(if (hardwareQualified) 1 else 0))
    field(network)
    field(fixed32(2))
    field(fixed32(3))
    field(fixed32(4))
    field(fixed32(5))
    field(ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(scale).array())
    field(fixed32(6))
    field(operation)
    field(fixed32(8))
    field(u128(amount))
    field(u128(total))
    val bytes = payload.toByteArray()
    if (lengthOverride != null) bytes[0] = lengthOverride.toByte()
    val frame = NoritoCodec.encode(bytes, schema, CREDIT_TEST_PAYLOAD_ADAPTER, flags)
    return frame.copyOfRange(0, NoritoHeader.HEADER_LENGTH) +
        ByteArray(CREDIT_TEST_PADDING_BYTES_V1) +
        frame.copyOfRange(NoritoHeader.HEADER_LENGTH, frame.size)
}
