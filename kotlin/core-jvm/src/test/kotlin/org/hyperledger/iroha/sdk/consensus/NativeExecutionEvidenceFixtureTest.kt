package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class NativeExecutionEvidenceFixtureTest {
    @Test fun `Rust native captures retain every source on one and four lanes`() {
        for (lanes in listOf(1, 4)) {
            val rows = NativeExecutionEvidenceFixtures.inspect(NativeExecutionEvidenceFixtures.load(lanes))
            assertEquals(8, rows.size)
            assertEquals(lanes, rows.map { it["lane_id"] to it["dataspace_id"] }.toSet().size)
            assertEquals(8, rows.map { it["logical_id"] }.toSet().size)
            assertEquals(setOf("warmup", "measurement"), rows.map { it["phase"] }.toSet())
        }
    }

    @Test fun `every JSON source and route field binds the canonical native row`() {
        val payload = NativeExecutionEvidenceFixtures.load(4)
        val p = SumeragiJsonPrimitives
        val root = p.parseObject(p.decodeUtf8(payload, "fixture"), "fixture")
        val rows = p.array(root["requests"], "requests", 4096).map { p.objectValue(it, "request") }
        rows.first().forEach { (field, value) ->
            val changed: Any? = when (value) {
                is Long -> value + 1
                is BigInteger -> value + BigInteger.ONE
                is String -> when (field) {
                    "phase" -> "measurement"
                    "authority" -> "$value@retired"
                    else -> value.dropLast(1) + if (value.last() == 'f') "b" else "f"
                }
                null -> emptyMap<String, Any>()
                is Map<*, *> -> null
                else -> error("unexpected projection value")
            }
            assertFailsWith<IllegalArgumentException>(field) {
                NativeExecutionEvidenceFixtures.inspect(JsonEncoder.encode(root + ("requests" to listOf(rows.first() + (field to changed)) + rows.drop(1))))
            }
        }
        val sourceIndex = rows.indexOfFirst { it["lane_source"] != null }
        assertTrue(sourceIndex >= 0)
        val source = p.objectValue(rows[sourceIndex]["lane_source"], "lane_source")
        for (field in source.keys) {
            val modified = rows.toMutableList()
            modified[sourceIndex] = rows[sourceIndex] + ("lane_source" to (source - field))
            assertFailsWith<IllegalArgumentException>(field) {
                NativeExecutionEvidenceFixtures.inspect(JsonEncoder.encode(root + ("requests" to modified)))
            }
        }
        for (changed in listOf(rows.drop(1), rows.reversed(), rows + rows.first())) {
            assertFailsWith<IllegalArgumentException> {
                NativeExecutionEvidenceFixtures.inspect(JsonEncoder.encode(root + ("requests" to changed)))
            }
        }
        for (field in root.keys) {
            assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.inspect(JsonEncoder.encode(root - field)) }
        }
        assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.inspect(JsonEncoder.encode(root + ("finality" to null))) }
    }

    @Test fun `canonical frame decoder rejects compression layout padding truncation and corruption`() {
        // A scalar codec unit vector, deliberately not presented as block evidence.
        val scalar = byteArrayOf(1, 2, 3, 4)
        val schema = "sdk::native_evidence::ScalarCodecTest"
        val frame = NoritoHeader(SchemaHash.hash16(schema), scalar.size, CRC64.compute(scalar), NoritoHeader.COMPACT_LEN, 0).encode() + scalar
        assertContentEquals(scalar, NativeExecutionEvidenceFixtures.frame(frame, schema))
        for (end in frame.indices) {
            assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.frame(frame.copyOf(end), schema) }
        }
        for (offset in listOf(0, 4, 5, 6, 22, 23, 31, 39, 40)) {
            val bad = frame.copyOf().also { it[offset] = (it[offset].toInt() xor 1).toByte() }
            assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.frame(bad, schema) }
        }
        assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.frame(frame + 0.toByte(), schema) }
        assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.frame(frame, "other::schema") }
    }

    @Test fun `bounded record decoder preserves full unsigned scalars and refuses hostile lengths`() {
        val maximum = BigInteger.ONE.shiftLeft(64) - BigInteger.ONE
        val r = NativeExecutionEvidenceFixtures.Reader(NativeExecutionEvidenceFixtures.record(NativeExecutionEvidenceFixtures.integer(maximum, 8)))
        assertEquals(maximum, r.number(8)); r.finish()
        listOf(byteArrayOf(0x80.toByte(), 0), byteArrayOf(0xff.toByte(), 0xff.toByte(), 0xff.toByte(), 0xff.toByte(), 0x7f), byteArrayOf(0x80.toByte())).forEach { bad ->
            assertFailsWith<IllegalArgumentException> { NativeExecutionEvidenceFixtures.Reader(bad).field() }
        }
        assertFailsWith<IllegalArgumentException> {
            NativeExecutionEvidenceFixtures.Reader(ByteArray(8) { 0xff.toByte() }).sequence { it }
        }
        val plain = byteArrayOf(8, 9)
        assertContentEquals(plain, NativeExecutionEvidenceFixtures.Reader(NativeExecutionEvidenceFixtures.integer(BigInteger.valueOf(2), 8) + plain).byteVector())
        assertTrue(NativeExecutionEvidenceFixtures.Reader(ByteArray(8)).sequence { it }.isEmpty())
    }
}
