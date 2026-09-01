package org.hyperledger.iroha.sdk.offline

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class KagemushaNfcProtocolTest {
    @Test
    fun applicationIdentifierRejectsOversizedHexBeforeDecoding() {
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.applicationIdentifier("AA".repeat(17))
        }
        assertEquals(
            16,
            KagemushaNfcProtocol.applicationIdentifier("AA".repeat(16)).size,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.applicationIdentifier(
                "AA".repeat(16) + " ".repeat(9),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.applicationIdentifier(
                "AA".repeat(5) + " ".repeat(9),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.applicationIdentifier("\u2003" + "AA".repeat(5))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.applicationIdentifier(" ".repeat(1_000_000))
        }
    }

    @Test
    fun bulkWriterRequiresCanonicalMinimumButAllowsASmallerFinalChunk() {
        val payload = ByteArray(KagemushaNfcProtocol.SAFE_CHUNK_BYTES + 1) { 0x5a }
        assertFailsWith<IllegalArgumentException> {
            KagemushaNfcProtocol.writePayloadCommands(
                KagemushaPeerPayloadKind.PAYMENT,
                payload,
                KagemushaNfcProtocol.SAFE_CHUNK_BYTES - 1,
            )
        }

        val commands = KagemushaNfcProtocol.writePayloadCommands(
            KagemushaPeerPayloadKind.PAYMENT,
            payload,
            KagemushaNfcProtocol.SAFE_CHUNK_BYTES,
        )
        assertEquals(4, commands.size)
        val first = KagemushaNfcProtocol.parseCommand(
            commands[1],
        ) as KagemushaNfcCommand.WriteChunk
        val final = KagemushaNfcProtocol.parseCommand(
            commands[2],
        ) as KagemushaNfcCommand.WriteChunk
        assertEquals(0, first.offset)
        assertEquals(KagemushaNfcProtocol.SAFE_CHUNK_BYTES, first.bytes.size)
        assertEquals(KagemushaNfcProtocol.SAFE_CHUNK_BYTES, final.offset)
        assertContentEquals(byteArrayOf(0x5a), final.bytes)
    }

    @Test
    fun payloadAssemblerBuffersOnlyAcceptedSparseBytes() {
        val maximum = KagemushaNfcPayloadAssembler(
            KagemushaPeerPayloadKind.PAYMENT,
            KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES,
            ByteArray(32) { 0x5a },
        )
        assertEquals(0, maximum.bufferedByteCount)
        assertFalse(maximum.isComplete)
        assertTrue(
            maximum.write(
                KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES - 3,
                byteArrayOf(7, 8, 9),
            ),
        )
        assertEquals(3, maximum.bufferedByteCount)
        maximum.clear()
        assertEquals(0, maximum.bufferedByteCount)

        val payload = "abcdefgh".toByteArray()
        val assembler = KagemushaNfcPayloadAssembler(
            KagemushaPeerPayloadKind.PAYMENT,
            payload.size,
            KagemushaNfcProtocol.sha256(payload),
        )
        assertTrue(assembler.write(4, "efgh".toByteArray()))
        assertEquals(4, assembler.bufferedByteCount)
        assertTrue(assembler.write(2, "cdef".toByteArray()))
        assertEquals(6, assembler.bufferedByteCount)
        assertTrue(assembler.write(3, "def".toByteArray()))
        assertEquals(6, assembler.bufferedByteCount)
        assertFalse(assembler.write(3, "dXf".toByteArray()))
        assertEquals(6, assembler.bufferedByteCount)
        assertTrue(assembler.write(0, "ab".toByteArray()))
        assertEquals(payload.size, assembler.bufferedByteCount)
        assertTrue(assembler.isComplete)
        assertContentEquals(payload, assembler.commit())
    }

    @Test
    fun fragmentBudgetViolationTerminallyClearsAssembler() {
        val assembler = KagemushaNfcPayloadAssembler(
            KagemushaPeerPayloadKind.PAYMENT,
            131,
            ByteArray(32) { 1 },
        )
        var accepted = 0
        for (offset in 0 until 131 step 2) {
            if (assembler.write(offset, byteArrayOf(offset.toByte()))) {
                accepted += 1
            } else {
                assertEquals(130, offset)
                break
            }
        }
        assertEquals(65, accepted)
        assertEquals(0, assembler.bufferedByteCount)
        assertFalse(assembler.isComplete)
        assertFalse(assembler.write(1, byteArrayOf(1)))
        assertFailsWith<IllegalStateException> { assembler.commit() }
    }

    @Test
    fun completeBadDigestIsTerminalAndCannotBeCommittedAgain() {
        val payload = "abcdef".toByteArray()
        val assembler = KagemushaNfcPayloadAssembler(
            KagemushaPeerPayloadKind.PAYMENT,
            payload.size,
            KagemushaNfcProtocol.sha256("abcdeg".toByteArray()),
        )
        assertTrue(assembler.write(0, payload))
        assertFailsWith<IllegalStateException> { assembler.commit() }
        assertEquals(0, assembler.bufferedByteCount)
        assertFalse(assembler.write(0, payload))
        assertFailsWith<IllegalStateException> { assembler.commit() }
    }

    @Test
    fun incompleteCommitIsRetryableAndSuccessConsumesAssembler() {
        val payload = "abcdefgh".toByteArray()
        val assembler = KagemushaNfcPayloadAssembler(
            KagemushaPeerPayloadKind.PAYMENT,
            payload.size,
            KagemushaNfcProtocol.sha256(payload),
        )
        assertTrue(assembler.write(0, "abcd".toByteArray()))
        assertFailsWith<IllegalStateException> { assembler.commit() }
        assertEquals(4, assembler.bufferedByteCount)
        assertTrue(assembler.write(4, "efgh".toByteArray()))
        assertContentEquals(payload, assembler.commit())
        assertEquals(0, assembler.bufferedByteCount)
        assertFalse(assembler.isComplete)
        assertFalse(assembler.write(0, payload))
        assertFailsWith<IllegalStateException> { assembler.commit() }
    }
}
