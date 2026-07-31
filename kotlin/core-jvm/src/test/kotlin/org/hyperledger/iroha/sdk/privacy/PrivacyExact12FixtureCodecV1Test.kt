// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.io.ByteArrayOutputStream
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

class PrivacyExact12FixtureCodecV1Test {
    @Test
    fun canonicalFixtureDecodesAndReencodesByteIdentically() {
        val fixture = loadFixture()
        val bundle = PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(fixture.base64)

        assertEquals(PrivacyExact12FixtureCodecV1.VERSION, bundle.version)
        assertEquals(PrivacyExact12FixtureCodecV1.ROW_COUNT, bundle.rows.size)
        assertEquals(
            PrivacyNativeBridge.ProtocolIdV1.values().toList(),
            bundle.rows.map(PrivacyExact12TypedFixtureRowV1::protocolId),
        )
        bundle.rows.forEach { row ->
            assertTrue(row.statementNorito.isNotEmpty())
            assertTrue(row.envelopeNorito.isNotEmpty())
            assertEquals(PrivacyExact12FixtureCodecV1.SUBMIT_PROOF_WIRE_ID, row.submitProofWireId)
            assertTrue(row.submitProofInstructionNorito.isNotEmpty())
            assertTrue(row.transactionIntentProjectionNorito.isNotEmpty())
            assertEquals(PrivacyExact12FixtureCodecV1.HASH_BYTES, row.transactionIntentDigest.size)
            assertTrue(row.unsignedTransactionPayloadNorito.isNotEmpty())
            assertTrue(row.signedTransactionVersionedNorito.isNotEmpty())
            assertEquals(PrivacyExact12FixtureCodecV1.HASH_BYTES, row.signedTransactionHash.size)
        }

        assertContentEquals(fixture.archive, PrivacyExact12FixtureCodecV1.encodeCanonical(bundle))
        assertEquals(fixture.base64, PrivacyExact12FixtureCodecV1.encodeCanonicalBase64(bundle))
        assertEquals(
            bundle,
            PrivacyExact12FixtureCodecV1.requireCanonicalArchive(fixture.archive, fixture.archive),
        )

        val first = bundle.rows.first()
        val statementCopy = first.statementNorito
        val originalFirstByte = first.statementNorito[0]
        statementCopy[0] = (statementCopy[0].toInt() xor 0xff).toByte()
        assertEquals(originalFirstByte, first.statementNorito[0])
    }

    @Test
    fun canonicalBase64RejectsWhitespaceAlternateSpellingsAndOverflow() {
        val encoded = loadFixture().base64
        listOf(
            "$encoded\n",
            " $encoded",
            "$encoded ",
            "$encoded=",
            encoded.dropLast(1),
        ).forEach { malformed ->
            assertFailsWith<IllegalArgumentException> {
                PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(malformed)
            }
        }

        assertEquals(0L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(0L))
        assertEquals(4L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(1L))
        assertEquals(4L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(3L))
        assertEquals(8L, PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(4L))
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(-1L)
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureCodecV1.canonicalBase64EncodedLength(Long.MAX_VALUE)
        }
        val oversizedBase64 = "A".repeat(
            PrivacyExact12FixtureCodecV1
                .canonicalBase64EncodedLength(
                    PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES.toLong(),
                ).toInt() + 1,
        )
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureCodecV1.decodeCanonicalBase64(oversizedBase64)
        }
    }

    @Test
    fun malformedHeadersLengthsAndTruncationAreRejected() {
        val canonical = loadFixture().archive
        val declaredTooLarge = canonical.copyOf().also {
            ByteBuffer.wrap(it)
                .order(ByteOrder.LITTLE_ENDIAN)
                .putLong(23, PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES.toLong())
        }
        val wrongSchema = canonical.copyOf().also { it[6] = (it[6].toInt() xor 0x80).toByte() }
        val wrongFlags = canonical.copyOf().also {
            it[NoritoHeader.HEADER_LENGTH - 1] = 0
        }
        val wrongCompression = canonical.copyOf().also { it[22] = NoritoHeader.COMPRESSION_ZSTD.toByte() }
        val wrongChecksum = canonical.copyOf().also {
            it[it.lastIndex] = (it.last().toInt() xor 0x01).toByte()
        }
        listOf(
            canonical.copyOfRange(0, canonical.size - 1),
            canonical + byteArrayOf(0),
            declaredTooLarge,
            wrongSchema,
            wrongFlags,
            wrongCompression,
            wrongChecksum,
            appendUnknownByteToFirstRow(canonical),
            ByteArray(NoritoHeader.HEADER_LENGTH - 1),
            ByteArray(PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES + 1),
        ).forEach { malformed ->
            assertFailsWith<IllegalArgumentException> {
                PrivacyExact12FixtureCodecV1.decodeCanonical(malformed)
            }
        }
    }

    @Test
    fun hostileNestedCountsAndDeclaredLengthsAreRejectedBeforeAllocation() {
        val wrongRowCount = frame(bundlePayload(u64(11L)))
        val oversizedRows = frame(
            concat(
                field(u32(PrivacyExact12FixtureCodecV1.VERSION.toLong())),
                compactLength(PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES.toLong() + 1L),
            ),
        )
        val oversizedFirstRow = frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong()),
                    compactLength(PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES.toLong() + 1L),
                ),
            ),
        )
        val unknownProtocol = frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong()),
                    field(field(u32(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong()))),
                ),
            ),
        )
        val truncatedFirstRow = frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong()),
                    field(field(u32(0L))),
                ),
            ),
        )
        val oversizedStatementFrame = frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong()),
                    field(
                        concat(
                            field(u32(0L)),
                            compactLength(
                                PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES.toLong() + 9L,
                            ),
                        ),
                    ),
                ),
            ),
        )
        val oversizedStatementVector = frame(
            bundlePayload(
                concat(
                    u64(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong()),
                    field(
                        concat(
                            field(u32(0L)),
                            field(
                                concat(
                                    u64(PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES.toLong() + 1L),
                                    byteArrayOf(0),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
        val nonMinimalVersionLength = frame(
            concat(
                byteArrayOf(0x84.toByte(), 0),
                u32(PrivacyExact12FixtureCodecV1.VERSION.toLong()),
                field(u64(PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong())),
            ),
        )
        listOf(
            wrongRowCount,
            oversizedRows,
            oversizedFirstRow,
            unknownProtocol,
            truncatedFirstRow,
            oversizedStatementFrame,
            oversizedStatementVector,
            nonMinimalVersionLength,
        ).forEach { hostile ->
            assertTrue(hostile.size < 512, "hostile test archive unexpectedly allocated a large payload")
            assertFailsWith<IllegalArgumentException> {
                PrivacyExact12FixtureCodecV1.decodeCanonical(hostile)
            }
        }
    }

    @Test
    fun reorderAndSameShapeSubstitutionAreRejected() {
        val fixture = loadFixture()
        val bundle = PrivacyExact12FixtureCodecV1.decodeCanonical(fixture.archive)

        val swappedRows = bundle.rows.toMutableList().also {
            val first = it[0]
            it[0] = it[1]
            it[1] = first
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureBundleV1(PrivacyExact12FixtureCodecV1.VERSION, swappedRows)
        }

        val reorderedArchive = swapFirstTwoRowFrames(fixture.archive)
        assertFalse(reorderedArchive.contentEquals(fixture.archive))
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureCodecV1.decodeCanonical(reorderedArchive)
        }

        val source = bundle.rows[0]
        val substituted = copyRow(source, statementNorito = bundle.rows[1].statementNorito)
        val substitutedRows = bundle.rows.toMutableList().also { it[0] = substituted }
        val candidate = PrivacyExact12FixtureCodecV1.encodeCanonical(
            PrivacyExact12FixtureBundleV1(PrivacyExact12FixtureCodecV1.VERSION, substitutedRows),
        )
        assertFalse(candidate.contentEquals(fixture.archive))
        PrivacyExact12FixtureCodecV1.decodeCanonical(candidate)
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureCodecV1.requireCanonicalArchive(candidate, fixture.archive)
        }
    }

    @Test
    fun modelEnforcesPerFieldAndAggregateBounds() {
        assertFailsWith<IllegalArgumentException> {
            syntheticRow(
                PrivacyNativeBridge.ProtocolIdV1.values().first(),
                statement = ByteArray(PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES + 1),
            )
        }
        val aggregateRows = PrivacyNativeBridge.ProtocolIdV1.values().map { protocol ->
            syntheticRow(protocol, signed = ByteArray(180_000) { 0x5a })
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12FixtureBundleV1(PrivacyExact12FixtureCodecV1.VERSION, aggregateRows)
        }
    }

    private fun loadFixture(): Fixture {
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, FIXTURE_PATH) }
            .firstOrNull(File::isFile)
            ?: error("cannot locate $FIXTURE_PATH")
        val bytes = file.readBytes()
        check(bytes.isNotEmpty() && bytes.last() == '\n'.code.toByte()) {
            "$FIXTURE_PATH must end in exactly one LF"
        }
        check('\r'.code.toByte() !in bytes) { "$FIXTURE_PATH must not contain CR bytes" }
        check(bytes.dropLast(1).none { it == '\n'.code.toByte() }) {
            "$FIXTURE_PATH must contain one base64 line"
        }
        val encoded = String(bytes, 0, bytes.size - 1, StandardCharsets.US_ASCII)
        val archive = Base64.getDecoder().decode(encoded)
        check(Base64.getEncoder().encodeToString(archive) == encoded) {
            "$FIXTURE_PATH is not canonical standard base64"
        }
        check(archive.size <= PrivacyExact12FixtureCodecV1.MAX_ARCHIVE_BYTES) {
            "$FIXTURE_PATH exceeds the decoded archive ceiling"
        }
        return Fixture(encoded, archive)
    }

    private fun bundlePayload(rowsPayload: ByteArray): ByteArray =
        concat(
            field(u32(PrivacyExact12FixtureCodecV1.VERSION.toLong())),
            field(rowsPayload),
        )

    private fun frame(payload: ByteArray): ByteArray {
        val header = NoritoHeader(
            SchemaHash.hash16(PrivacyExact12FixtureCodecV1.SCHEMA_NAME),
            payload.size,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return concat(header, payload)
    }

    private fun swapFirstTwoRowFrames(archive: ByteArray): ByteArray {
        val payload = archive.copyOfRange(NoritoHeader.HEADER_LENGTH, archive.size)
        val version = readFrame(payload, 0)
        val rowsLength = readCompactLength(payload, version.end)
        val rowsStart = rowsLength.end
        val rowsEnd = Math.addExact(rowsStart, rowsLength.value.toInt())
        check(rowsEnd == payload.size)
        val rowsPayload = payload.copyOfRange(rowsStart, rowsEnd)
        check(ByteBuffer.wrap(rowsPayload).order(ByteOrder.LITTLE_ENDIAN).long ==
            PrivacyExact12FixtureCodecV1.ROW_COUNT.toLong())
        val frames = ArrayList<Frame>(PrivacyExact12FixtureCodecV1.ROW_COUNT)
        var cursor = Long.SIZE_BYTES
        repeat(PrivacyExact12FixtureCodecV1.ROW_COUNT) {
            val frame = readFrame(rowsPayload, cursor)
            frames.add(frame)
            cursor = frame.end
        }
        check(cursor == rowsPayload.size)

        val reorderedRows = ByteArrayOutputStream(rowsPayload.size)
        reorderedRows.write(rowsPayload, 0, Long.SIZE_BYTES)
        for (index in listOf(1, 0) + (2 until PrivacyExact12FixtureCodecV1.ROW_COUNT)) {
            val span = frames[index]
            reorderedRows.write(rowsPayload, span.start, span.end - span.start)
        }
        val modifiedPayload = ByteArrayOutputStream(payload.size)
        modifiedPayload.write(payload, 0, version.end)
        modifiedPayload.write(payload, version.end, rowsStart - version.end)
        modifiedPayload.write(reorderedRows.toByteArray())
        return frame(modifiedPayload.toByteArray())
    }

    private fun appendUnknownByteToFirstRow(archive: ByteArray): ByteArray {
        val payload = archive.copyOfRange(NoritoHeader.HEADER_LENGTH, archive.size)
        val version = readFrame(payload, 0)
        val rowsLength = readCompactLength(payload, version.end)
        val rowsStart = rowsLength.end
        val rowsEnd = Math.addExact(rowsStart, rowsLength.value.toInt())
        check(rowsEnd == payload.size)
        val rowsPayload = payload.copyOfRange(rowsStart, rowsEnd)
        val firstRowLength = readCompactLength(rowsPayload, Long.SIZE_BYTES)
        val firstRowEnd = Math.addExact(firstRowLength.end, firstRowLength.value.toInt())
        check(firstRowEnd <= rowsPayload.size)
        val firstRowPayload = rowsPayload.copyOfRange(firstRowLength.end, firstRowEnd)
        val modifiedRows = concat(
            rowsPayload.copyOfRange(0, Long.SIZE_BYTES),
            field(firstRowPayload + byteArrayOf(0)),
            rowsPayload.copyOfRange(firstRowEnd, rowsPayload.size),
        )
        val modifiedPayload = concat(
            payload.copyOfRange(0, version.end),
            field(modifiedRows),
        )
        return frame(modifiedPayload)
    }

    private fun readFrame(bytes: ByteArray, offset: Int): Frame {
        val length = readCompactLength(bytes, offset)
        val end = Math.addExact(length.end, length.value.toInt())
        check(end <= bytes.size)
        return Frame(offset, end)
    }

    private fun readCompactLength(bytes: ByteArray, offset: Int): CompactLength {
        var value = 0L
        var shift = 0
        var cursor = offset
        while (true) {
            check(cursor < bytes.size && shift <= 63)
            val octet = bytes[cursor++].toInt() and 0xff
            value = value or ((octet and 0x7f).toLong() shl shift)
            if (octet and 0x80 == 0) return CompactLength(value, cursor)
            shift += 7
        }
    }

    private fun compactLength(value: Long): ByteArray =
        NoritoEncoder(NoritoHeader.COMPACT_LEN).also { it.writeLength(value, true) }.toByteArray()

    private fun field(payload: ByteArray): ByteArray = concat(compactLength(payload.size.toLong()), payload)

    private fun u32(value: Long): ByteArray =
        ByteBuffer.allocate(Int.SIZE_BYTES).order(ByteOrder.LITTLE_ENDIAN).putInt(value.toInt()).array()

    private fun u64(value: Long): ByteArray =
        ByteBuffer.allocate(Long.SIZE_BYTES).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()

    private fun copyRow(
        row: PrivacyExact12TypedFixtureRowV1,
        statementNorito: ByteArray = row.statementNorito,
    ): PrivacyExact12TypedFixtureRowV1 = PrivacyExact12TypedFixtureRowV1(
        row.protocolId,
        statementNorito,
        row.envelopeNorito,
        row.submitProofWireId,
        row.submitProofInstructionNorito,
        row.transactionIntentProjectionNorito,
        row.transactionIntentDigest,
        row.unsignedTransactionPayloadNorito,
        row.signedTransactionVersionedNorito,
        row.signedTransactionHash,
    )

    private fun syntheticRow(
        protocol: PrivacyNativeBridge.ProtocolIdV1,
        statement: ByteArray = byteArrayOf(1),
        signed: ByteArray = byteArrayOf(1),
    ): PrivacyExact12TypedFixtureRowV1 = PrivacyExact12TypedFixtureRowV1(
        protocol,
        statement,
        byteArrayOf(2),
        PrivacyExact12FixtureCodecV1.SUBMIT_PROOF_WIRE_ID,
        byteArrayOf(3),
        byteArrayOf(4),
        ByteArray(PrivacyExact12FixtureCodecV1.HASH_BYTES) { 5 },
        byteArrayOf(6),
        signed,
        ByteArray(PrivacyExact12FixtureCodecV1.HASH_BYTES) { 7 },
    )

    private fun concat(vararg parts: ByteArray): ByteArray {
        val result = ByteArray(parts.fold(0) { total, part -> Math.addExact(total, part.size) })
        var offset = 0
        parts.forEach { part ->
            part.copyInto(result, offset)
            offset += part.size
        }
        return result
    }

    private data class Fixture(val base64: String, val archive: ByteArray)
    private data class Frame(val start: Int, val end: Int)
    private data class CompactLength(val value: Long, val end: Int)

    private companion object {
        private const val FIXTURE_PATH =
            "fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64"
    }
}
