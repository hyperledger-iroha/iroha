package org.hyperledger.iroha.sdk.norito

import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.Optional
import org.hyperledger.iroha.sdk.client.JsonParser
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

/**
 * The dense Norito layout (`norito.md`, "Dense layout (header minor 1)") against the shared
 * vectors in `fixtures/norito_dense/vectors.json`.
 *
 * Every vector pins one rule's dense bytes next to the current compact bytes of the same value.
 * The same adapters decode and re-encode both, so the layout is the only variable.
 */
class NoritoDenseLayoutTest {

    @Test
    fun `vector file holds exactly the rules this test covers`() {
        assertEquals("norito.md, Dense layout (header minor 1)", VECTORS.layout)
        assertEquals(BARE_CASES.keys, VECTORS.bare.map { it.name }.toSet())
        assertEquals(setOf("wide"), VECTORS.frames.map { it.name }.toSet())
    }

    @Test
    fun `every bare vector decodes and re-encodes byte-identically in both layouts`() {
        for (vector in VECTORS.bare) {
            val case = BARE_CASES.getValue(vector.name)
            for ((layout, bytes) in listOf(WireLayout.DENSE to vector.dense, WireLayout.COMPACT to vector.compact)) {
                val label = "${vector.name} ($layout)"
                val decoded = case.decode(bytes, layout)
                assertEquals(normalize(case.value), normalize(decoded), "$label: decoded value")
                assertContentEquals(bytes, case.encode(decoded, layout), "$label: re-encoding")
                assertContentEquals(bytes, case.encode(case.value, layout), "$label: encoding")
            }
            assertTrue(vector.dense.size <= vector.compact.size, "${vector.name}: dense is never larger")
        }
    }

    @Test
    fun `bare decoding refuses truncated input and trailing bytes`() {
        for (vector in VECTORS.bare) {
            val case = BARE_CASES.getValue(vector.name)
            for ((layout, bytes) in listOf(WireLayout.DENSE to vector.dense, WireLayout.COMPACT to vector.compact)) {
                assertRefused("${vector.name} ($layout): truncated") {
                    case.decode(bytes.copyOf(bytes.size - 1), layout)
                }
                assertRefused("${vector.name} ($layout): trailing byte") {
                    case.decode(bytes + byteArrayOf(0), layout)
                }
            }
        }
    }

    @Test
    fun `bare bytes of one layout are refused by the other`() {
        var compared = 0
        for (vector in VECTORS.bare) {
            // `Option::None` and `Result` are the same bytes in both layouts.
            if (vector.dense.contentEquals(vector.compact)) continue
            val case = BARE_CASES.getValue(vector.name)
            assertRefused("${vector.name}: dense bytes read as compact") {
                case.decode(vector.dense, WireLayout.COMPACT)
            }
            assertRefused("${vector.name}: compact bytes read as dense") {
                case.decode(vector.compact, WireLayout.DENSE)
            }
            compared++
        }
        assertEquals(VECTORS.bare.size - 2, compared)
    }

    @Test
    fun `framed vector decodes and re-encodes in the layout its header declares`() {
        val vector = VECTORS.frames.single()
        val schemaHash = SchemaHash.hash16(vector.schemaName)

        val denseHeader = NoritoHeader.decodeDeclared(vector.dense, schemaHash).header
        assertEquals(NoritoHeader.MINOR_VERSION_DENSE, denseHeader.minor)
        assertEquals(0, denseHeader.flags)
        assertEquals(WireLayout.DENSE, denseHeader.layout)
        // No alignment padding: the payload starts at byte 40.
        assertEquals(NoritoHeader.HEADER_LENGTH + denseHeader.payloadLength, vector.dense.size)

        val compactHeader = NoritoHeader.decodeDeclared(vector.compact, schemaHash).header
        assertEquals(NoritoHeader.MINOR_VERSION, compactHeader.minor)
        assertEquals(NoritoHeader.COMPACT_LEN, compactHeader.flags)
        assertEquals(WireLayout.COMPACT, compactHeader.layout)

        // The header alone selects the layout for both frames.
        for (frame in listOf(vector.dense, vector.compact)) {
            val layout = NoritoHeader.decodeDeclared(frame, schemaHash).header.layout
            val decoded = NoritoCodec.decode(frame, WIDE_ADAPTER, vector.schemaName, layout)
            assertEquals(normalize(WIDE_VALUE), normalize(decoded), "$layout: decoded value")
        }

        val decoded = NoritoCodec.decode(vector.dense, WIDE_ADAPTER, vector.schemaName, WireLayout.DENSE)
        assertContentEquals(
            vector.dense,
            NoritoCodec.encode(decoded, vector.schemaName, WIDE_ADAPTER, WireLayout.DENSE),
            "dense frame re-encoding",
        )

        // The current layout is unchanged: the default entry points read the minor-0 frame.
        val current = NoritoCodec.decode(vector.compact, WIDE_ADAPTER, vector.schemaName)
        assertEquals(normalize(WIDE_VALUE), normalize(current))
        // The vector carries the 8 bytes of alignment padding of a 16-aligned Rust type. The
        // Kotlin encoder never writes alignment padding, so only header and payload compare.
        val reencoded = NoritoCodec.encode(current, vector.schemaName, WIDE_ADAPTER)
        assertContentEquals(
            NoritoCodec.encode(current, vector.schemaName, WIDE_ADAPTER, WireLayout.COMPACT),
            reencoded,
            "the compact layout is the default encoding",
        )
        val padding = vector.compact.size - reencoded.size
        assertEquals(8, padding)
        assertContentEquals(vector.compact.copyOf(NoritoHeader.HEADER_LENGTH), reencoded.copyOf(NoritoHeader.HEADER_LENGTH))
        assertContentEquals(
            vector.compact.copyOfRange(NoritoHeader.HEADER_LENGTH + padding, vector.compact.size),
            reencoded.copyOfRange(NoritoHeader.HEADER_LENGTH, reencoded.size),
        )
        // Both layouts carry the same payload for this type: only header and padding differ.
        assertContentEquals(
            vector.dense.copyOfRange(NoritoHeader.HEADER_LENGTH, vector.dense.size),
            reencoded.copyOfRange(NoritoHeader.HEADER_LENGTH, reencoded.size),
        )
    }

    @Test
    fun `a frame is never read as the other layout`() {
        val vector = VECTORS.frames.single()
        val schemaHash = SchemaHash.hash16(vector.schemaName)

        // Readers of the current layout refuse a dense frame instead of misreading flags 0x00.
        assertFailsWith<IllegalArgumentException> { NoritoHeader.decode(vector.dense, schemaHash) }
        assertFailsWith<IllegalArgumentException> {
            NoritoHeader.decodeView(java.nio.ByteBuffer.wrap(vector.dense), schemaHash)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(vector.dense, WIDE_ADAPTER, vector.schemaName)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(java.nio.ByteBuffer.wrap(vector.dense), WIDE_ADAPTER, vector.schemaName)
        }
        assertFailsWith<IllegalArgumentException> { NoritoCodec.fromBytesView(vector.dense, vector.schemaName) }
        assertTrue(NoritoCodec.tryDecode(vector.dense, WIDE_ADAPTER, vector.schemaName) is Result.Err)

        // A reader that names a layout refuses a frame declaring another one.
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(vector.dense, WIDE_ADAPTER, vector.schemaName, WireLayout.COMPACT)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(vector.dense, WIDE_ADAPTER, vector.schemaName, WireLayout.FIXED)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(vector.compact, WIDE_ADAPTER, vector.schemaName, WireLayout.DENSE)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(vector.compact, WIDE_ADAPTER, vector.schemaName, WireLayout.FIXED)
        }

        // The header minor decides, not the payload: one layout's payload under the other
        // layout's header is decoded by the header's rules and refused.
        val record = BARE_CASES.getValue("record")
        val recordVector = VECTORS.bare.single { it.name == "record" }
        val schema = "norito.vectors.dense.Record"
        val compactUnderDense = frame(schema, recordVector.compact, NoritoHeader.MINOR_VERSION_DENSE, 0)
        assertRefused("compact payload under a dense header") {
            NoritoCodec.decode(compactUnderDense, record.adapter, schema, WireLayout.DENSE)
        }
        val denseUnderCompact =
            frame(schema, recordVector.dense, NoritoHeader.MINOR_VERSION, NoritoHeader.COMPACT_LEN)
        assertRefused("dense payload under a compact header") {
            NoritoCodec.decode(denseUnderCompact, record.adapter, schema)
        }
        // The same payloads under their own headers decode.
        val denseFrame = frame(schema, recordVector.dense, NoritoHeader.MINOR_VERSION_DENSE, 0)
        assertEquals(
            normalize(record.value),
            normalize(NoritoCodec.decode(denseFrame, record.adapter, schema, WireLayout.DENSE)),
        )
        assertContentEquals(denseFrame, record.encodeFrame(schema, WireLayout.DENSE))
        val compactFrame =
            frame(schema, recordVector.compact, NoritoHeader.MINOR_VERSION, NoritoHeader.COMPACT_LEN)
        assertEquals(normalize(record.value), normalize(NoritoCodec.decode(compactFrame, record.adapter, schema)))
        assertContentEquals(compactFrame, record.encodeFrame(schema, WireLayout.COMPACT))
    }

    @Test
    fun `dense headers refuse flags unknown minors padding truncation and trailing bytes`() {
        val vector = VECTORS.frames.single()
        val schemaHash = SchemaHash.hash16(vector.schemaName)
        val frame = vector.dense
        fun refused(label: String, mutated: ByteArray) {
            assertRefused("$label: header") { NoritoHeader.decodeDeclared(mutated, schemaHash) }
            assertRefused("$label: dense reader") {
                NoritoCodec.decode(mutated, WIDE_ADAPTER, vector.schemaName, WireLayout.DENSE)
            }
            assertRefused("$label: current reader") {
                NoritoCodec.decode(mutated, WIDE_ADAPTER, vector.schemaName)
            }
        }

        // Dense has no layout flags.
        for (flags in 1..0xFF) {
            refused("flags 0x${"%02x".format(flags)}", frame.copyOf().also { it[NoritoHeader.HEADER_LENGTH - 1] = flags.toByte() })
            assertFailsWith<IllegalArgumentException> { WireLayout.fromHeader(NoritoHeader.MINOR_VERSION_DENSE, flags) }
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader(schemaHash, 1, 0, flags, NoritoHeader.COMPRESSION_NONE, NoritoHeader.MINOR_VERSION_DENSE)
            }
        }
        // Unknown minor versions, with either flag byte.
        for (minor in 2..0xFF) {
            refused("minor $minor", frame.copyOf().also { it[5] = minor.toByte() })
            refused("minor $minor on a current frame", vector.compact.copyOf().also { it[5] = minor.toByte() })
            assertFailsWith<IllegalArgumentException> { WireLayout.fromHeader(minor, 0) }
            assertFailsWith<IllegalArgumentException> {
                NoritoHeader(schemaHash, 1, 0, 0, NoritoHeader.COMPRESSION_NONE, minor)
            }
        }
        // No alignment padding and no trailing data.
        val header = frame.copyOf(NoritoHeader.HEADER_LENGTH)
        val payload = frame.copyOfRange(NoritoHeader.HEADER_LENGTH, frame.size)
        refused("leading padding", header + ByteArray(8) + payload)
        refused("trailing byte", frame + byteArrayOf(0))
        // Truncation, in the header and in the payload.
        refused("truncated payload", frame.copyOf(frame.size - 1))
        refused("truncated header", frame.copyOf(NoritoHeader.HEADER_LENGTH - 1))
        refused("empty", ByteArray(0))
        // The checksum and the schema still bind the payload.
        assertRefused("checksum") {
            NoritoCodec.decode(
                frame.copyOf().also { it[frame.size - 1] = (it[frame.size - 1].toInt() xor 1).toByte() },
                WIDE_ADAPTER,
                vector.schemaName,
                WireLayout.DENSE,
            )
        }
        assertRefused("schema") {
            NoritoCodec.decode(frame, WIDE_ADAPTER, "norito.vectors.dense.Other", WireLayout.DENSE)
        }
    }

    @Test
    fun `dense frames round-trip compressed payloads without padding`() {
        val vector = VECTORS.frames.single()
        val compressed = NoritoCodec.encode(
            WIDE_VALUE,
            vector.schemaName,
            WIDE_ADAPTER,
            WireLayout.DENSE,
            CompressionConfig.zstd(3),
        )
        val header = NoritoHeader.decodeDeclared(compressed, null).header
        assertEquals(WireLayout.DENSE, header.layout)
        assertEquals(NoritoHeader.COMPRESSION_ZSTD, header.compression)
        assertEquals(
            normalize(WIDE_VALUE),
            normalize(NoritoCodec.decode(compressed, WIDE_ADAPTER, vector.schemaName, WireLayout.DENSE)),
        )
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.decode(compressed, WIDE_ADAPTER, vector.schemaName)
        }
    }

    @Test
    fun `dense counts tags and arrays refuse noncanonical and short input`() {
        val bytes = NoritoAdapters.rawByteVecAdapter()
        fun byteVec(vararg input: Int) = NoritoCodec.decodeBare(input.toBytes(), bytes, WireLayout.DENSE)
        assertContentEquals(byteArrayOf(1, 2, 3), byteVec(3, 1, 2, 3))
        // Overlong count, truncated body, trailing byte.
        assertRefused("overlong count") { byteVec(0x83, 0x00, 1, 2, 3) }
        assertRefused("truncated body") { byteVec(3, 1, 2) }
        assertRefused("trailing byte") { byteVec(3, 1, 2, 3, 0) }
        // A count beyond the payload is refused before any allocation.
        assertRefused("oversized count") { byteVec(0xFF, 0xFF, 0xFF, 0xFF, 0x07) }
        assertRefused("oversized sequence count") {
            NoritoCodec.decodeBare(
                intArrayOf(0xFF, 0xFF, 0xFF, 0xFF, 0x07).toBytes(),
                NoritoAdapters.sequence(NoritoAdapters.stringAdapter()),
                WireLayout.DENSE,
            )
        }
        assertRefused("oversized map count") {
            NoritoCodec.decodeBare(
                intArrayOf(0xFF, 0xFF, 0xFF, 0xFF, 0x07).toBytes(),
                NoritoAdapters.map(NoritoAdapters.stringAdapter(), NoritoAdapters.uint(16)),
                WireLayout.DENSE,
            )
        }

        fun choice(vararg input: Int) = NoritoCodec.decodeBare(input.toBytes(), ChoiceAdapter, WireLayout.DENSE)
        assertEquals(Choice.Unit, choice(0))
        // Overlong tag, unknown tag, tag beyond `u32`, compact-layout tag width.
        assertRefused("overlong tag") { choice(0x80, 0x00) }
        assertRefused("unknown tag") { choice(2) }
        assertRefused("tag beyond u32") { choice(0x80, 0x80, 0x80, 0x80, 0x10) }
        assertRefused("compact tag width") { choice(0, 0, 0, 0) }

        val array = NoritoAdapters.u8Array(4)
        fun array(vararg input: Int) = NoritoCodec.decodeBare(input.toBytes(), array, WireLayout.DENSE)
        assertContentEquals(byteArrayOf(1, 2, 3, 4), array(1, 2, 3, 4))
        assertRefused("short array") { array(1, 2, 3) }
        assertRefused("compact array framing") { array(1, 1, 1, 2, 1, 3, 1, 4) }
    }

    @Test
    fun `every u8 sequence is a varint count and raw bytes in the dense layout`() {
        val value = byteArrayOf(1, 2, 3)
        val expected = byteArrayOf(3, 1, 2, 3)
        // `Vec<u8>`, `ConstVec<u8>` and a generic element sequence of `u8` share one dense form.
        assertContentEquals(expected, NoritoCodec.encodeBare(value, NoritoAdapters.rawByteVecAdapter(), WireLayout.DENSE))
        assertContentEquals(expected, NoritoCodec.encodeBare(value, NoritoAdapters.byteVecAdapter(), WireLayout.DENSE))
        val elements = NoritoAdapters.sequence(NoritoAdapters.uint(8))
        assertContentEquals(expected, NoritoCodec.encodeBare(listOf(1L, 2L, 3L), elements, WireLayout.DENSE))
        assertContentEquals(value, NoritoCodec.decodeBare(expected, NoritoAdapters.byteVecAdapter(), WireLayout.DENSE))
        assertEquals(listOf(1L, 2L, 3L), NoritoCodec.decodeBare(expected, elements, WireLayout.DENSE))
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.encodeBare(listOf(256L), elements, WireLayout.DENSE)
        }

        // The current layout keeps its two distinct forms.
        val framed = byteArrayOf(3, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 1, 3)
        assertContentEquals(framed, NoritoCodec.encodeBare(value, NoritoAdapters.byteVecAdapter(), WireLayout.COMPACT))
        assertContentEquals(framed, NoritoCodec.encodeBare(listOf(1L, 2L, 3L), elements, WireLayout.COMPACT))
        assertContentEquals(
            byteArrayOf(3, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3),
            NoritoCodec.encodeBare(value, NoritoAdapters.rawByteVecAdapter(), WireLayout.COMPACT),
        )
        // A two-byte count.
        val long = ByteArray(300) { 0xEE.toByte() }
        val encoded = NoritoCodec.encodeBare(long, NoritoAdapters.rawByteVecAdapter(), WireLayout.DENSE)
        assertEquals(302, encoded.size)
        assertContentEquals(byteArrayOf(0xAC.toByte(), 0x02), encoded.copyOf(2))
        assertContentEquals(long, NoritoCodec.decodeBare(encoded, NoritoAdapters.rawByteVecAdapter(), WireLayout.DENSE))
    }

    @Test
    fun `layout codes select layouts and the dense code never frames a payload`() {
        for (layout in WireLayout.values()) {
            assertEquals(layout, WireLayout.fromCode(layout.code))
            assertEquals(layout, WireLayout.fromHeader(layout.minor, layout.headerFlags))
            assertEquals(layout, NoritoEncoder(layout).layout)
            assertEquals(layout, NoritoEncoder(layout.code).layout)
            assertEquals(layout, NoritoEncoder(layout).childEncoder().layout)
            assertEquals(layout, NoritoDecoder(ByteArray(0), layout).layout)
            assertEquals(layout.code, NoritoDecoder(ByteArray(0), layout).flags)
            assertEquals(layout == WireLayout.DENSE, NoritoEncoder(layout).denseLayoutActive())
            assertEquals(layout == WireLayout.DENSE, NoritoDecoder(ByteArray(0), layout).denseLayoutActive())
            assertEquals(layout != WireLayout.FIXED, NoritoDecoder(ByteArray(0), layout).compactLenActive())
        }
        assertEquals(0, WireLayout.FIXED.minor)
        assertEquals(0, WireLayout.COMPACT.minor)
        assertEquals(1, WireLayout.DENSE.minor)
        assertEquals(0x00, WireLayout.DENSE.headerFlags)
        assertEquals(NoritoHeader.COMPACT_LEN, WireLayout.COMPACT.headerFlags)

        // Only the exact dense code selects the dense layout.
        for (code in 0..0xFF) {
            val expected = when {
                code == WireLayout.DENSE.code -> WireLayout.DENSE
                code and NoritoHeader.COMPACT_LEN != 0 -> WireLayout.COMPACT
                else -> WireLayout.FIXED
            }
            assertEquals(expected, WireLayout.fromCode(code), "code 0x${"%02x".format(code)}")
        }
        // The dense code is a decode and encode context, never a header flag byte.
        val dense = WireLayout.DENSE.code
        assertFailsWith<IllegalArgumentException> { NoritoHeader.validateFlags(dense) }
        assertFailsWith<IllegalArgumentException> { WireLayout.fromHeader(NoritoHeader.MINOR_VERSION, dense) }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.encode(byteArrayOf(1), "norito.test.Bytes", NoritoAdapters.rawByteVecAdapter(), dense)
        }
        assertFailsWith<IllegalArgumentException> {
            NoritoCodec.encodeAdaptive(byteArrayOf(1), NoritoAdapters.rawByteVecAdapter(), dense)
        }
        assertFailsWith<IllegalArgumentException> { NoritoCodec.DecodeFlagsGuard.enter(dense) }
    }

    @Test
    fun `counts and enum tags follow the layout and dense has no fixed-width length`() {
        fun written(layout: WireLayout, write: (NoritoEncoder) -> Unit): ByteArray =
            NoritoEncoder(layout).also(write).toByteArray()

        for (layout in listOf(WireLayout.FIXED, WireLayout.COMPACT)) {
            assertContentEquals(byteArrayOf(5, 0, 0, 0, 0, 0, 0, 0), written(layout) { it.writeCount(5) })
            assertContentEquals(byteArrayOf(0x2C, 0x01, 0, 0), written(layout) { it.writeEnumTag(300) })
            assertEquals(5L, NoritoDecoder(byteArrayOf(5, 0, 0, 0, 0, 0, 0, 0), layout).readCount())
            assertEquals(300L, NoritoDecoder(byteArrayOf(0x2C, 0x01, 0, 0), layout).readEnumTag())
            assertEquals(0xFFFF_FFFFL, NoritoDecoder(ByteArray(4) { -1 }, layout).readEnumTag())
        }
        assertContentEquals(byteArrayOf(5), written(WireLayout.DENSE) { it.writeCount(5) })
        assertContentEquals(byteArrayOf(0xAC.toByte(), 0x02), written(WireLayout.DENSE) { it.writeCount(300) })
        assertContentEquals(byteArrayOf(0xAC.toByte(), 0x02), written(WireLayout.DENSE) { it.writeEnumTag(300) })
        assertEquals(300L, NoritoDecoder(byteArrayOf(0xAC.toByte(), 0x02), WireLayout.DENSE).readCount())
        assertEquals(300L, NoritoDecoder(byteArrayOf(0xAC.toByte(), 0x02), WireLayout.DENSE).readEnumTag())
        val maxTag = byteArrayOf(-1, -1, -1, -1, 0x0F)
        assertContentEquals(maxTag, written(WireLayout.DENSE) { it.writeEnumTag(0xFFFF_FFFFL) })
        assertEquals(0xFFFF_FFFFL, NoritoDecoder(maxTag, WireLayout.DENSE).readEnumTag())

        for (layout in WireLayout.values()) {
            assertFailsWith<IllegalArgumentException> { NoritoEncoder(layout).writeEnumTag(-1) }
            assertFailsWith<IllegalArgumentException> { NoritoEncoder(layout).writeEnumTag(0x1_0000_0000L) }
            val tag = NoritoAdapters.enumTag()
            assertEquals(300L, NoritoCodec.decodeBare(NoritoCodec.encodeBare(300L, tag, layout), tag, layout))
        }

        // A fixed-width length under the dense layout is a programming error, not a second form.
        assertFailsWith<IllegalStateException> { NoritoEncoder(WireLayout.DENSE).writeLength(1, false) }
        assertFailsWith<IllegalStateException> {
            NoritoDecoder(ByteArray(8), WireLayout.DENSE).readLength(false)
        }
        assertContentEquals(byteArrayOf(1), written(WireLayout.DENSE) { it.writeLength(1, true) })
        assertEquals(1L, NoritoDecoder(byteArrayOf(1), WireLayout.DENSE).readLength(true))
    }

    @Test
    fun `fixed byte arrays are framed per byte in minor 0 and raw in the dense layout`() {
        val array = NoritoAdapters.u8Array(4)
        val value = byteArrayOf(1, 2, 3, 4)
        assertContentEquals(value, NoritoCodec.encodeBare(value, array, WireLayout.DENSE))
        assertContentEquals(
            byteArrayOf(1, 1, 1, 2, 1, 3, 1, 4),
            NoritoCodec.encodeBare(value, array, WireLayout.COMPACT),
        )
        val fixed = NoritoCodec.encodeBare(value, array, WireLayout.FIXED)
        assertEquals(4 * 9, fixed.size)
        assertContentEquals(value, NoritoCodec.decodeBare(fixed, array, WireLayout.FIXED))
        for (layout in WireLayout.values()) {
            assertFailsWith<IllegalArgumentException> { NoritoCodec.encodeBare(byteArrayOf(1, 2, 3), array, layout) }
        }
        assertFailsWith<IllegalArgumentException> { NoritoAdapters.u8Array(0) }
        // A record field of the same type is raw behind its field prefix in both layouts.
        val field = sized(NoritoAdapters.fixedBytes(4))
        for (layout in listOf(WireLayout.COMPACT, WireLayout.DENSE)) {
            assertContentEquals(byteArrayOf(4, 1, 2, 3, 4), NoritoCodec.encodeBare(value, field, layout))
        }
    }

    @Test
    fun `header dump reports the layout a frame declares`() {
        val vector = VECTORS.frames.single()
        for ((frame, layout) in listOf(vector.compact to WireLayout.COMPACT, vector.dense to WireLayout.DENSE)) {
            val file = Files.createTempFile("norito-dump", ".norito")
            val printed = java.io.ByteArrayOutputStream()
            val stdout = System.out
            try {
                Files.write(file, frame)
                System.setOut(java.io.PrintStream(printed, true, "UTF-8"))
                NoritoDump.main(arrayOf(file.toString()))
            } finally {
                System.setOut(stdout)
                Files.delete(file)
            }
            val report = printed.toString("UTF-8")
            assertTrue(report.contains("version: 0.${layout.minor}"), report)
            assertTrue(report.contains("layout: $layout"), report)
            assertTrue(report.contains("flags: 0x${"%02x".format(layout.headerFlags)}"), report)
        }
    }

    @Test
    fun `explicit compact layout is the default encoding`() {
        for (vector in VECTORS.bare) {
            val case = BARE_CASES.getValue(vector.name)
            assertContentEquals(vector.compact, case.encodeDefault(), "${vector.name}: default encoding")
            assertEquals(normalize(case.value), normalize(case.decodeDefault(vector.compact)), vector.name)
        }
    }

    private class BareCase<T>(val adapter: TypeAdapter<T>, val value: T) {
        fun decode(bytes: ByteArray, layout: WireLayout): T = NoritoCodec.decodeBare(bytes, adapter, layout)

        @Suppress("UNCHECKED_CAST")
        fun encode(value: Any?, layout: WireLayout): ByteArray =
            NoritoCodec.encodeBare(value as T, adapter, layout)

        fun encodeFrame(schema: String, layout: WireLayout): ByteArray =
            NoritoCodec.encode(value, schema, adapter, layout)

        fun encodeDefault(): ByteArray = NoritoCodec.encodeAdaptive(value, adapter).payload()

        fun decodeDefault(bytes: ByteArray): T = NoritoCodec.decodeAdaptive(bytes, adapter)
    }

    private sealed class Choice {
        object Unit : Choice()
        data class One(val text: String) : Choice()
        data class Far(val value: Long) : Choice()
    }

    /** `Choice { Unit = 0, One(String) = 1, Far { value: u64 } = 300 }` as a derive encodes it. */
    private object ChoiceAdapter : TypeAdapter<Choice> {
        private val tag = NoritoAdapters.enumTag()
        private val text = sized(NoritoAdapters.stringAdapter())
        private val number = sized(NoritoAdapters.uint(64))

        override fun encode(encoder: NoritoEncoder, value: Choice) {
            when (value) {
                Choice.Unit -> tag.encode(encoder, 0)
                is Choice.One -> {
                    tag.encode(encoder, 1)
                    text.encode(encoder, value.text)
                }
                is Choice.Far -> {
                    tag.encode(encoder, 300)
                    number.encode(encoder, value.value)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): Choice = when (val discriminant = tag.decode(decoder)) {
            0L -> Choice.Unit
            1L -> Choice.One(text.decode(decoder))
            300L -> Choice.Far(number.decode(decoder))
            else -> throw IllegalArgumentException("unknown Choice tag $discriminant")
        }
    }

    /** `Result<u8, u8>`: a one-byte tag and a length-prefixed payload in every layout. */
    private object ResultAdapter : TypeAdapter<Result<Long, Long>> {
        private val payload = sized(NoritoAdapters.uint(8))

        override fun encode(encoder: NoritoEncoder, value: Result<Long, Long>) {
            when (value) {
                is Result.Ok -> {
                    encoder.writeByte(0)
                    payload.encode(encoder, value.value)
                }
                is Result.Err -> {
                    encoder.writeByte(1)
                    payload.encode(encoder, value.error)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): Result<Long, Long> = when (val tag = decoder.readByte()) {
            0 -> Result.Ok(payload.decode(decoder))
            1 -> Result.Err(payload.decode(decoder))
            else -> throw IllegalArgumentException("invalid Result tag $tag")
        }
    }

    private class Record(
        val id: Long,
        val key: ByteArray,
        val digests: List<ByteArray>,
        val label: Optional<String>,
        val choice: Choice,
    )

    /** `Record { id: u32, key: [u8; 4], digests: Vec<[u8; 4]>, label: Option<String>, choice }`. */
    private object RecordAdapter : TypeAdapter<Record> {
        private val id = sized(NoritoAdapters.uint(32))
        private val key = sized(NoritoAdapters.fixedBytes(4))
        private val digests = sized(NoritoAdapters.sequence(NoritoAdapters.u8Array(4)))
        private val label = sized(NoritoAdapters.option(NoritoAdapters.stringAdapter()))
        private val choice = sized(ChoiceAdapter)

        override fun encode(encoder: NoritoEncoder, value: Record) {
            id.encode(encoder, value.id)
            key.encode(encoder, value.key)
            digests.encode(encoder, value.digests)
            label.encode(encoder, value.label)
            choice.encode(encoder, value.choice)
        }

        override fun decode(decoder: NoritoDecoder): Record = Record(
            id.decode(decoder),
            key.decode(decoder),
            digests.decode(decoder),
            label.decode(decoder),
            choice.decode(decoder),
        )
    }

    private class Wide(val value: BigInteger, val tag: ByteArray)

    private class VectorFile(val layout: String, val bare: List<BareVector>, val frames: List<FrameVector>)

    private class BareVector(val name: String, val compact: ByteArray, val dense: ByteArray)

    private class FrameVector(val name: String, val schemaName: String, val compact: ByteArray, val dense: ByteArray)

    private companion object {
        private const val VECTORS_PATH = "fixtures/norito_dense/vectors.json"

        /** `Wide { value: u128, tag: [u8; 2] }`: both fields are raw behind their field prefixes. */
        private val WIDE_ADAPTER = object : TypeAdapter<Wide> {
            private val value = sized(NoritoAdapters.fixedBytes(16))
            private val tag = sized(NoritoAdapters.fixedBytes(2))

            override fun encode(encoder: NoritoEncoder, value: Wide) {
                val littleEndian = ByteArray(16)
                val bigEndian = value.value.toByteArray()
                require(value.value.signum() >= 0 && bigEndian.size <= 17) { "value must fit in u128" }
                for (index in bigEndian.indices) {
                    val target = bigEndian.size - 1 - index
                    if (target < 16) littleEndian[target] = bigEndian[index]
                }
                this.value.encode(encoder, littleEndian)
                tag.encode(encoder, value.tag)
            }

            override fun decode(decoder: NoritoDecoder): Wide =
                Wide(BigInteger(1, value.decode(decoder).reversedArray()), tag.decode(decoder))
        }
        private val WIDE_VALUE = Wide(
            BigInteger("0102030405060708090a0b0c0d0e0f10", 16),
            byteArrayOf(0xCA.toByte(), 0xFE.toByte()),
        )

        /** One typed adapter and the value `vectors.json` names, per bare vector. */
        private val BARE_CASES: Map<String, BareCase<*>> = linkedMapOf(
            "u8_vec" to BareCase(NoritoAdapters.rawByteVecAdapter(), byteArrayOf(1, 2, 3)),
            "u8_vec_128" to BareCase(NoritoAdapters.rawByteVecAdapter(), ByteArray(128) { 0x11 }),
            "u8_array" to BareCase(NoritoAdapters.u8Array(4), byteArrayOf(1, 2, 3, 4)),
            "option_u8_array" to BareCase(
                NoritoAdapters.option(NoritoAdapters.u8Array(4)),
                Optional.of(byteArrayOf(9, 9, 9, 9)),
            ),
            "option_none" to BareCase(
                NoritoAdapters.option(NoritoAdapters.u8Array(4)),
                Optional.empty<ByteArray>(),
            ),
            "vec_u8_array" to BareCase(
                NoritoAdapters.sequence(NoritoAdapters.u8Array(2)),
                listOf(byteArrayOf(7, 7), byteArrayOf(8, 8)),
            ),
            "vec_string" to BareCase(NoritoAdapters.sequence(NoritoAdapters.stringAdapter()), listOf("ab")),
            "vec_string_empty" to BareCase(
                NoritoAdapters.sequence(NoritoAdapters.stringAdapter()),
                emptyList<String>(),
            ),
            "map_string_u16" to BareCase(
                NoritoAdapters.map(NoritoAdapters.stringAdapter(), NoritoAdapters.uint(16)),
                mapOf("k" to 5L),
            ),
            "enum_unit" to BareCase<Choice>(ChoiceAdapter, Choice.Unit),
            "enum_tuple" to BareCase<Choice>(ChoiceAdapter, Choice.One("hi")),
            "enum_far" to BareCase<Choice>(ChoiceAdapter, Choice.Far(1)),
            "result_err" to BareCase<Result<Long, Long>>(ResultAdapter, Result.Err(7)),
            "record" to BareCase(
                RecordAdapter,
                Record(
                    id = 7,
                    key = byteArrayOf(1, 2, 3, 4),
                    digests = listOf(ByteArray(4) { 0xAA.toByte() }, ByteArray(4) { 0xBB.toByte() }),
                    label = Optional.of("hi"),
                    choice = Choice.Far(1),
                ),
            ),
        )

        private val VECTORS: VectorFile = loadVectors()

        /** A length-prefixed value: a derived record field or enum variant field. */
        private fun <T> sized(inner: TypeAdapter<T>): TypeAdapter<T> = object : TypeAdapter<T> {
            override fun encode(encoder: NoritoEncoder, value: T) {
                val child = encoder.childEncoder()
                inner.encode(child, value)
                val payload = child.toByteArray()
                encoder.writeLength(payload.size.toLong(), (encoder.flags and NoritoHeader.COMPACT_LEN) != 0)
                encoder.writeBytes(payload)
            }

            override fun decode(decoder: NoritoDecoder): T {
                val length = decoder.readLength(decoder.compactLenActive())
                val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags)
                val value = inner.decode(child)
                require(child.remaining() == 0) { "Trailing bytes after field payload" }
                return value
            }
        }

        /** A frame around an arbitrary payload, to place one layout's bytes under any header. */
        private fun frame(schema: String, payload: ByteArray, minor: Int, flags: Int): ByteArray =
            NoritoHeader(
                SchemaHash.hash16(schema),
                payload.size,
                CRC64.compute(payload),
                flags,
                NoritoHeader.COMPRESSION_NONE,
                minor,
            ).encode() + payload

        /** Structural form of a decoded value, with byte arrays rendered as hex. */
        private fun normalize(value: Any?): Any? = when (value) {
            is ByteArray -> "0x" + value.joinToString("") { "%02x".format(it) }
            is Optional<*> -> if (value.isPresent) listOf(normalize(value.get())) else emptyList<Any?>()
            is List<*> -> value.map(::normalize)
            is Map<*, *> -> value.entries.associate { normalize(it.key) to normalize(it.value) }
            is Record -> listOf(
                value.id,
                normalize(value.key),
                normalize(value.digests),
                normalize(value.label),
                value.choice,
            )
            is Wide -> listOf(value.value, normalize(value.tag))
            else -> value
        }

        private fun assertRefused(label: String, block: () -> Any?) {
            assertFailsWith<RuntimeException>(label) { block() }
        }

        private fun IntArray.toBytes(): ByteArray = ByteArray(size) { this[it].toByte() }

        private fun loadVectors(): VectorFile {
            val root = asObject(JsonParser.parse(String(Files.readAllBytes(resolve(VECTORS_PATH)), Charsets.UTF_8)))
            assertEquals(setOf("layout", "bare", "frames"), root.keys)
            val bare = asList(root["bare"]).map { entry ->
                val vector = asObject(entry)
                assertEquals(setOf("name", "type", "value", "compact", "dense"), vector.keys)
                BareVector(vector["name"] as String, hex(vector["compact"]), hex(vector["dense"]))
            }
            val frames = asList(root["frames"]).map { entry ->
                val vector = asObject(entry)
                assertEquals(setOf("name", "schema_name", "type", "value", "compact", "dense"), vector.keys)
                FrameVector(
                    vector["name"] as String,
                    vector["schema_name"] as String,
                    hex(vector["compact"]),
                    hex(vector["dense"]),
                )
            }
            return VectorFile(root["layout"] as String, bare, frames)
        }

        @Suppress("UNCHECKED_CAST")
        private fun asObject(value: Any?): Map<String, Any?> = value as Map<String, Any?>

        private fun asList(value: Any?): List<Any?> = value as List<Any?>

        private fun hex(value: Any?): ByteArray {
            val text = value as String
            require(text.length % 2 == 0) { "odd hex length" }
            return ByteArray(text.length / 2) { index ->
                text.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            }
        }

        /** Locate a repository file from the Gradle test working directory. */
        private fun resolve(relative: String): Path {
            var current: Path? = Paths.get("").toAbsolutePath().normalize()
            while (current != null) {
                val candidate = current.resolve(relative)
                if (Files.isRegularFile(candidate)) return candidate
                current = current.parent
            }
            error("$relative was not found from the test working directory")
        }
    }
}
