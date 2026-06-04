package org.hyperledger.iroha.sdk.connect

import java.nio.file.Files
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import kotlin.test.Test
import kotlin.test.assertFailsWith

class ConnectSequenceTest {
    @Test
    fun negativeSequenceRejectedAcrossConnectSurfaces() {
        val sessionId = ByteArray(32) { 0x02 }
        val key = ByteArray(32) { 0x03 }

        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectCrypto.nonceFromSequence(-1L)
            },
        )
        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectCrypto.encryptEnvelope(
                    byteArrayOf(0x01),
                    key,
                    sessionId,
                    ConnectDirection.APP_TO_WALLET,
                    -1L,
                )
            },
        )
        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectCrypto.decryptCiphertext(
                    byteArrayOf(0x01),
                    key,
                    sessionId,
                    ConnectDirection.APP_TO_WALLET,
                    -1L,
                )
            },
        )
        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectEnvelopeCodec.encodeSignResultErrEnvelope(-1L, "ERR", "message")
            },
        )
        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectFrameCodec.encodeCiphertextFrame(
                    sessionId,
                    ConnectDirection.APP_TO_WALLET,
                    -1L,
                    byteArrayOf(0x01),
                )
            },
        )

        val journal = ConnectQueueJournal(
            sessionId,
            JournalConfiguration(
                Files.createTempDirectory("connect-negative-sequence"),
                4,
                4096,
                60_000L,
            ),
        )
        assertSequenceFailure(
            assertFailsWith<ConnectJournalException> {
                journal.append(
                    ConnectDirection.APP_TO_WALLET,
                    -1L,
                    byteArrayOf(0x01),
                    100L,
                    1_000L,
                )
            },
        )
    }

    @Test
    fun envelopeDecodeRejectsHighBitUint64Sequence() {
        val envelope = ConnectEnvelopeCodec.encodeSignResultErrEnvelope(0L, "ERR", "message")
        val mutated = envelope.copyOf()
        val sequenceOffset = envelopeSequencePayloadOffset(mutated)
        for (i in 0 until 8) {
            mutated[sequenceOffset + i] = 0xff.toByte()
        }
        rewriteNoritoChecksum(mutated)

        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectEnvelopeCodec.decodeEnvelope(mutated)
            },
        )
    }

    @Test
    fun frameDecodeRejectsHighBitUint64Sequence() {
        val frame = ConnectFrameCodec.encodeCiphertextFrame(
            ByteArray(32) { 0x04 },
            ConnectDirection.APP_TO_WALLET,
            0L,
            byteArrayOf(0x01),
        )
        val mutated = frame.copyOf()
        val sequenceOffset = lengthPrefixedFieldPayloadOffset(mutated, 2)
        for (i in 0 until 8) {
            mutated[sequenceOffset + i] = 0xff.toByte()
        }

        assertSequenceFailure(
            assertFailsWith<ConnectProtocolException> {
                ConnectFrameCodec.decode(mutated)
            },
        )
    }

    private fun envelopeSequencePayloadOffset(envelope: ByteArray): Int {
        val fieldLengthOffset = NoritoHeader.HEADER_LENGTH
        val length = readU64Le(envelope, fieldLengthOffset)
        require(length == Long.SIZE_BYTES.toLong()) {
            "unexpected envelope sequence field length: $length"
        }
        return fieldLengthOffset + Long.SIZE_BYTES
    }

    private fun lengthPrefixedFieldPayloadOffset(frame: ByteArray, fieldIndex: Int): Int {
        var offset = 0
        for (i in 0 until fieldIndex) {
            val length = readU64Le(frame, offset)
            require(length >= 0L && length <= Int.MAX_VALUE) {
                "invalid field length at index $i: $length"
            }
            offset += Long.SIZE_BYTES + length.toInt()
        }
        val length = readU64Le(frame, offset)
        require(length == Long.SIZE_BYTES.toLong()) {
            "unexpected sequence field length: $length"
        }
        return offset + Long.SIZE_BYTES
    }

    private fun readU64Le(bytes: ByteArray, offset: Int): Long {
        var value = 0L
        for (i in 0 until Long.SIZE_BYTES) {
            value = value or ((bytes[offset + i].toLong() and 0xffL) shl (8 * i))
        }
        return value
    }

    private fun rewriteNoritoChecksum(envelope: ByteArray) {
        val payload = envelope.copyOfRange(NoritoHeader.HEADER_LENGTH, envelope.size)
        writeU64Le(envelope, NORITO_CHECKSUM_OFFSET, CRC64.compute(payload))
    }

    private fun writeU64Le(bytes: ByteArray, offset: Int, value: Long) {
        for (i in 0 until Long.SIZE_BYTES) {
            bytes[offset + i] = ((value ushr (8 * i)) and 0xffL).toByte()
        }
    }

    private fun assertSequenceFailure(error: Throwable) {
        var current: Throwable? = error
        while (current != null) {
            if (current.message.orEmpty().contains("sequence")) {
                return
            }
            current = current.cause
        }
        throw AssertionError("expected sequence error, got: ${error.message}", error)
    }

    private companion object {
        private const val NORITO_CHECKSUM_OFFSET = 4 + 1 + 1 + 16 + 1 + 8
    }
}
