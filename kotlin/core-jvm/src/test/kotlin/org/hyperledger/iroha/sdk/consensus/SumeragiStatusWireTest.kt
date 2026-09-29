package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import kotlin.test.*
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader

class SumeragiStatusWireTest {
    private fun bytes(hex: String) = ByteArray(hex.length / 2) { hex.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    private fun sample() = bytes(NativeStatusFixtures.rows().getValue("validator").second)
    private fun frame(payload: ByteArray): ByteArray {
        val original = NoritoHeader.decode(sample(), null).header
        return NoritoHeader(original.schemaHash, payload.size, CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN, NoritoHeader.COMPRESSION_NONE).encode() + payload
    }
    @Test fun `all Rust frames equal JSON and reencode byte for byte`() {
        assertEquals(8, NativeStatusFixtures.rows().size)
        for ((name, row) in NativeStatusFixtures.rows()) {
            val wire = bytes(row.second)
            val expected = SumeragiStatus.parseJson(row.first)
            val actual = SumeragiStatusWire.decodeCanonical(wire)
            assertEquals(expected, actual, name)
            assertContentEquals(wire, SumeragiStatusWire.encode(actual), name)
            wire.fill(0)
            assertEquals(expected, actual, "decoded value must own every field")
            assertContentEquals(bytes(row.second), SumeragiStatusWire.encode(actual), name)
            val halt = actual.halted
            if (halt != null) {
                assertEquals(name, halt.kind.wireName)
                assertEquals(if (halt.kind.hasHeight) BigInteger("18446744073709551615") else null, halt.height)
            }
        }
    }
    @Test fun `header compression layout schema checksum and bounded sizes fail closed`() {
        val wire = sample()
        for (offset in listOf(0, 4, 5, 6, 22, 23, 31, 39)) {
            val mutated = wire.copyOf(); mutated[offset] = (mutated[offset].toInt() xor 1).toByte()
            assertFails("header byte $offset") { SumeragiStatusWire.decodeCanonical(mutated) }
        }
        assertFails { SumeragiStatusWire.decodeCanonical(wire.copyOfRange(40, wire.size)) }
        assertFails { SumeragiStatusWire.decodeCanonical(wire + byteArrayOf(0)) }
        assertFails { SumeragiStatusWire.decodeCanonical(wire.copyOfRange(0, 40) + byteArrayOf(0) + wire.copyOfRange(40, wire.size)) }
        assertFails { SumeragiStatusWire.decodeCanonical(ByteArray(1_048_577)) }
        // Canonical status uses no compression: rejection must precede decompression/allocation.
        val compressed = wire.copyOf(); compressed[22] = 1
        assertFails { SumeragiStatusWire.decodeCanonical(compressed) }
        for (size in wire.indices) assertFails("prefix $size") { SumeragiStatusWire.decodeCanonical(wire.copyOf(size)) }
    }
    @Test fun `checksummed payloads reject retired protocol malformed lengths and trailing fields`() {
        val payload = NoritoHeader.decode(sample(), null).payload
        assertEquals(2, payload[0].toInt())
        for (version in listOf(0, 4, 6, 7, 9)) {
            val bad = payload.copyOf(); bad[1] = version.toByte(); bad[2] = 0
            assertFails { SumeragiStatusWire.decodeCanonical(frame(bad)) }
        }
        assertFails { SumeragiStatusWire.decodeCanonical(frame(byteArrayOf(0x82.toByte(), 0) + payload.copyOfRange(1, payload.size))) }
        assertFails { SumeragiStatusWire.decodeCanonical(frame(byteArrayOf(0xff.toByte(), 0xff.toByte(), 0xff.toByte(), 1))) }
        assertFails { SumeragiStatusWire.decodeCanonical(frame(payload + byteArrayOf(0))) }
        // The third field is the mandatory nullable beacon record. An unassigned Option tag fails.
        val invalidOption = payload.copyOf(); invalidOption[37] = 2
        assertFails { SumeragiStatusWire.decodeCanonical(frame(invalidOption)) }
    }
    @Test fun `beacon session requires the canonical nested fixed array layout`() {
        val payload = NativeExecutionEvidenceFixtures.Reader(NoritoHeader.decode(sample(), null).payload)
        val fields = MutableList(21) { payload.field() }
        payload.finish()
        val option = NativeExecutionEvidenceFixtures.Reader(fields[2])
        assertContentEquals(byteArrayOf(1), option.raw(1))
        val horizon = NativeExecutionEvidenceFixtures.Reader(option.field())
        option.finish()
        val nested = MutableList(5) { horizon.field() }
        horizon.finish()
        // Option<[u8;32]> uses generic element framing. A raw32 body is a
        // different layout even when the outer frame has a correct checksum.
        nested[2] = byteArrayOf(1) + NativeExecutionEvidenceFixtures.record(ByteArray(32) { 0xab.toByte() })
        fields[2] = byteArrayOf(1) + NativeExecutionEvidenceFixtures.record(NativeExecutionEvidenceFixtures.record(*nested.toTypedArray()))
        assertFailsWith<IllegalArgumentException> {
            SumeragiStatusWire.decodeCanonical(frame(NativeExecutionEvidenceFixtures.record(*fields.toTypedArray())))
        }
    }
}
