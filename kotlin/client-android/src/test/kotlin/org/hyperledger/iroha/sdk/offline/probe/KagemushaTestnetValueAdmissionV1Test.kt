// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.io.ByteArrayOutputStream
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
import kotlin.test.assertTrue

class KagemushaTestnetValueAdmissionV1Test {
    @Test
    fun `all native contract words are required`() {
        for (index in 0 until 3) {
            val endpoint = Endpoint().apply { contractWords[index] += 1 }
            assertFailsWith<IllegalStateException> {
                KagemushaTestnetValueAdmissionV1.openEndpoint(endpoint)
            }
            assertEquals(0, endpoint.calls)
        }
        assertFailsWith<IllegalStateException> {
            KagemushaTestnetValueAdmissionV1.openEndpoint(Endpoint().apply { missingContract = true })
        }
    }

    @Test
    fun `zero or malformed operation never reaches JNI`() {
        val endpoint = Endpoint()
        val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(endpoint)
        for (id in listOf(ByteArray(0), ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertFailsWith<IllegalArgumentException> { admission.admitFinalizedValue(id) }
        }
        assertEquals(0, endpoint.calls)
    }

    @Test
    fun `only an exact operation bound canonical archive is returned`() {
        val endpoint = Endpoint()
        val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(endpoint)
        val id = ByteArray(32) { 7 }
        assertEquals(480, endpoint.archive.size)
        assertEquals(8, ADMISSION_TEST_PADDING_BYTES_V1)
        assertContentEquals(admissionArchive(), admission.admitFinalizedValue(id))
        assertContentEquals(ByteArray(32) { 7 }, id)
        assertTrue(endpoint.direct)
        assertEquals(768, endpoint.capacity)
        assertEquals(1, endpoint.calls)
        endpoint.status = -312
        assertEquals(-312, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            admission.admitFinalizedValue(id)
        }.status)
        endpoint.status = 0
        assertEquals(0, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            admission.admitFinalizedValue(id)
        }.status)
        endpoint.status = 769
        assertEquals(769, assertFailsWith<KagemushaTestnetObservationExceptionV1> {
            admission.admitFinalizedValue(id)
        }.status)
        endpoint.missingAdmit = true
        assertFailsWith<IllegalStateException> { admission.admitFinalizedValue(id) }
    }

    @Test
    fun `foreign operation hardware claim and false value facts are rejected`() {
        val id = ByteArray(32) { 7 }
        val altered = listOf(
            admissionArchive(fields = validAdmissionFields().apply { set(0, byteArrayOf(2, 0)) }),
            admissionArchive(fields = validAdmissionFields().apply { set(1, byteArrayOf(1)) }),
            admissionArchive(fields = validAdmissionFields().apply { set(7, le32(29)) }),
            admissionArchive(fields = validAdmissionFields().apply { set(9, ByteArray(32) { 9 }) }),
            admissionArchive(fields = validAdmissionFields().apply { set(10, ByteArray(32)) }),
            admissionArchive(fields = validAdmissionFields().apply { set(11, ByteArray(16)) }),
            admissionArchive(fields = validAdmissionFields().apply { set(15, ByteArray(8)) }),
            admissionArchive(fields = validAdmissionFields().apply { set(16, ByteArray(32)) }),
        )
        for (archive in altered) {
            val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(
                Endpoint().apply { this.archive = archive },
            )
            assertFailsWith<IllegalArgumentException> { admission.admitFinalizedValue(id) }
        }
    }

    @Test
    fun `wrong schema framing field length and checksum are rejected`() {
        val canonical = admissionArchive()
        val wrongPadding = canonical.copyOf().apply { this[NoritoHeader.HEADER_LENGTH] = 1 }
        val wrongChecksum = canonical.copyOf().apply { this[lastIndex] = (this[lastIndex].toInt() xor 1).toByte() }
        val wrongFieldLength = admissionArchive(fields = validAdmissionFields().apply {
            set(10, ByteArray(31) { 8 })
            set(12, ByteArray(33) { 10 }) // Keep the frame length valid; reject the field prefix.
        })
        val invalid = listOf(
            byteArrayOf(3, 4, 5), canonical.copyOf(canonical.size - 1), canonical + byteArrayOf(0),
            admissionArchive(schema = "wrong.schema"), admissionArchive(flags = 0),
            wrongFieldLength, wrongPadding, wrongChecksum,
        )
        for (archive in invalid) {
            val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(
                Endpoint().apply { this.archive = archive },
            )
            assertFailsWith<IllegalArgumentException> {
                admission.admitFinalizedValue(ByteArray(32) { 7 })
            }
        }
    }

    @Test
    fun `full unsigned finality height remains valid`() {
        val fields = validAdmissionFields().apply {
            set(15, ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(-1L).array())
        }
        val archive = admissionArchive(fields = fields)
        val admission = KagemushaTestnetValueAdmissionV1.openEndpoint(
            Endpoint().apply { this.archive = archive },
        )
        assertContentEquals(archive, admission.admitFinalizedValue(ByteArray(32) { 7 }))
    }

    private class Endpoint : KagemushaTestnetValueAdmissionEndpointV1 {
        val contractWords = intArrayOf(1, 32, 768)
        var archive = admissionArchive()
        var status: Int? = null
        var calls = 0
        var direct = false
        var capacity = 0
        var missingContract = false
        var missingAdmit = false

        override fun contract(): IntArray? {
            if (missingContract) throw UnsatisfiedLinkError("missing contract")
            return contractWords.copyOf()
        }

        override fun admit(operationId: ByteArray, output: ByteBuffer): Int {
            if (missingAdmit) throw UnsatisfiedLinkError("missing admit")
            calls++
            operationId.fill(0)
            direct = output.isDirect
            capacity = output.capacity()
            if (archive.size <= output.remaining()) output.put(archive)
            return status ?: archive.size
        }
    }
}

private const val ADMISSION_TEST_SCHEMA_V1 =
    "connect_norito_bridge::KagemushaTestnetValueAdmissionArchiveV1"
private const val ADMISSION_TEST_PADDING_BYTES_V1 =
    (16 - NoritoHeader.HEADER_LENGTH % 16) % 16

private val ADMISSION_TEST_PAYLOAD_ADAPTER = object : TypeAdapter<ByteArray> {
    override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
    override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
}

private fun le32(value: Int): ByteArray =
    ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()

private fun validAdmissionFields(): MutableList<ByteArray> = mutableListOf(
    byteArrayOf(1, 0), byteArrayOf(0), ByteArray(32) { 1 }, ByteArray(32) { 2 },
    ByteArray(32) { 3 }, ByteArray(32) { 4 }, ByteArray(32) { 5 }, le32(2),
    ByteArray(32) { 6 }, ByteArray(32) { 7 }, ByteArray(32) { 8 },
    ByteArray(16).apply { this[0] = 9 }, ByteArray(32) { 10 }, ByteArray(32) { 11 },
    ByteArray(32) { 12 }, ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(13).array(),
    ByteArray(32) { 14 },
)

private fun admissionArchive(
    fields: List<ByteArray> = validAdmissionFields(),
    schema: String = ADMISSION_TEST_SCHEMA_V1,
    flags: Int = NoritoHeader.COMPACT_LEN,
): ByteArray {
    val payload = ByteArrayOutputStream()
    for (field in fields) {
        payload.write(field.size)
        payload.write(field)
    }
    val frame = NoritoCodec.encode(payload.toByteArray(), schema, ADMISSION_TEST_PAYLOAD_ADAPTER, flags)
    return frame.copyOfRange(0, NoritoHeader.HEADER_LENGTH) +
        ByteArray(ADMISSION_TEST_PADDING_BYTES_V1) +
        frame.copyOfRange(NoritoHeader.HEADER_LENGTH, frame.size)
}
