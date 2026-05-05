package org.hyperledger.iroha.sdk.connect

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Arrays
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Single Connect queue journal entry encoded as `ConnectJournalRecordV1`. */
class ConnectJournalRecord @Throws(ConnectJournalException::class) constructor(
    @JvmField val direction: ConnectDirection,
    @JvmField val sequence: Long,
    payloadHash: ByteArray,
    ciphertext: ByteArray,
    @JvmField val receivedAtMs: Long,
    @JvmField val expiresAtMs: Long,
) {
    private val _payloadHash: ByteArray = normalizeHash(payloadHash)
    private val _ciphertext: ByteArray = ciphertext.copyOf()

    init {
        if (_ciphertext.size.toLong() > 0xFFFF_FFFFL) {
            throw ConnectJournalException("ciphertext too large for journal entry")
        }
    }

    fun payloadHash(): ByteArray = _payloadHash.clone()
    fun ciphertext(): ByteArray = _ciphertext.clone()

    fun encodedLength(): Int = NoritoHeader.HEADER_LENGTH + PAYLOAD_FIXED_LENGTH + _ciphertext.size

    @Throws(ConnectJournalException::class)
    fun encode(): ByteArray {
        try {
            return NoritoCodec.encode(this, SCHEMA_NAME, ADAPTER)
        } catch (ex: RuntimeException) {
            throw ConnectJournalException("failed to encode journal record", ex)
        }
    }

    class DecodeResult internal constructor(
        @JvmField val record: ConnectJournalRecord,
        @JvmField val bytesConsumed: Int,
    )

    private class RecordAdapter : TypeAdapter<ConnectJournalRecord> {
        override fun encode(encoder: NoritoEncoder, value: ConnectJournalRecord) {
            UINT8.encode(encoder, value.direction.tag.toLong())
            UINT64.encode(encoder, value.sequence)
            UINT64.encode(encoder, value.receivedAtMs)
            UINT64.encode(encoder, value.expiresAtMs)
            UINT32.encode(encoder, Integer.toUnsignedLong(value._ciphertext.size))
            HASH.encode(encoder, value._payloadHash)
            encoder.writeBytes(value._ciphertext)
        }

        override fun decode(decoder: NoritoDecoder): ConnectJournalRecord {
            val directionTag = UINT8.decode(decoder).toInt()
            val direction = ConnectDirection.fromTag(directionTag)
            val sequence = UINT64.decode(decoder)
            val receivedAt = UINT64.decode(decoder)
            val expiresAt = UINT64.decode(decoder)
            val ciphertextLength = UINT32.decode(decoder)
            if (ciphertextLength < 0 || ciphertextLength > Int.MAX_VALUE) {
                throw IllegalArgumentException("ciphertext length too large")
            }
            val hash = HASH.decode(decoder)
            val ciphertext = decoder.readBytes(ciphertextLength.toInt())
            try {
                return ConnectJournalRecord(
                    direction, sequence, hash, ciphertext, receivedAt, expiresAt,
                )
            } catch (ex: ConnectJournalException) {
                throw IllegalArgumentException("invalid journal record payload", ex)
            }
        }

        companion object {
            private val UINT8 = NoritoAdapters.uint(8)
            private val UINT32 = NoritoAdapters.uint(32)
            private val UINT64 = NoritoAdapters.uint(64)
            private val HASH = NoritoAdapters.fixedBytes(32)
        }
    }

    companion object {
        internal const val SCHEMA_NAME = "ConnectJournalRecordV1"
        private val SCHEMA_HASH = SchemaHash.hash16(SCHEMA_NAME)
        private const val PAYLOAD_FIXED_LENGTH = 1 + 8 + 8 + 8 + 4 + 32
        private val ADAPTER: TypeAdapter<ConnectJournalRecord> = RecordAdapter()

        @JvmStatic
        @Throws(ConnectJournalException::class)
        fun decode(data: ByteArray, offset: Int): DecodeResult {
            if (offset < 0 || offset >= data.size) {
                throw ConnectJournalException("offset outside journal bounds")
            }
            val remaining = data.size - offset
            if (remaining < NoritoHeader.HEADER_LENGTH) {
                throw ConnectJournalException("insufficient bytes for Norito header")
            }
            val header = ByteBuffer.wrap(data, offset, NoritoHeader.HEADER_LENGTH)
                .order(ByteOrder.LITTLE_ENDIAN)
            val magic = ByteArray(4)
            header.get(magic)
            if (!magic.contentEquals(NoritoHeader.MAGIC)) {
                throw ConnectJournalException("invalid Norito magic")
            }
            val major = header.get().toInt() and 0xFF
            val minor = header.get().toInt() and 0xFF
            if (major != NoritoHeader.MAJOR_VERSION || minor != NoritoHeader.MINOR_VERSION) {
                throw ConnectJournalException("unsupported Norito version: $major.$minor")
            }
            val schemaHash = ByteArray(16)
            header.get(schemaHash)
            if (!schemaHash.contentEquals(SCHEMA_HASH)) {
                throw ConnectJournalException("schema hash mismatch in journal entry")
            }
            val compression = header.get().toInt() and 0xFF
            if (compression != NoritoHeader.COMPRESSION_NONE) {
                throw ConnectJournalException("compressed journal entries are not supported")
            }
            val payloadLength = header.getLong()
            if (payloadLength > Int.MAX_VALUE) {
                throw ConnectJournalException("payload too large for journal entry")
            }
            val checksum = header.getLong()
            header.get() // flags
            val intPayloadLength = payloadLength.toInt()
            val paddingLength = detectPaddingLength(data, offset, intPayloadLength, checksum)
            val recordLength = NoritoHeader.HEADER_LENGTH + paddingLength + intPayloadLength
            if (recordLength < 0 || recordLength > remaining) {
                throw ConnectJournalException("record length exceeds file bounds")
            }
            val slice = Arrays.copyOfRange(data, offset, offset + recordLength)
            try {
                val record = NoritoCodec.decode(slice, ADAPTER, SCHEMA_NAME)
                return DecodeResult(record, recordLength)
            } catch (ex: RuntimeException) {
                throw ConnectJournalException("failed to decode journal entry", ex)
            }
        }

        @JvmStatic
        fun computePayloadHash(ciphertext: ByteArray): ByteArray =
            Blake2b.digest(ciphertext, 32)

        @Throws(ConnectJournalException::class)
        private fun normalizeHash(hash: ByteArray?): ByteArray {
            if (hash == null || hash.size != 32) {
                throw ConnectJournalException("payload hash must contain 32 bytes")
            }
            return hash.copyOf()
        }

        @Throws(ConnectJournalException::class)
        private fun detectPaddingLength(
            data: ByteArray,
            offset: Int,
            payloadLength: Int,
            checksum: Long,
        ): Int {
            if (payloadLength < 0) {
                throw ConnectJournalException("payload too large for journal entry")
            }
            val headerEnd = offset + NoritoHeader.HEADER_LENGTH
            val maxAvailable = data.size - headerEnd - payloadLength
            if (maxAvailable < 0) {
                throw ConnectJournalException("record length exceeds file bounds")
            }
            val maxPadding = minOf(NoritoHeader.MAX_HEADER_PADDING, maxAvailable)
            for (padding in 0..maxPadding) {
                var paddingOk = true
                for (i in 0 until padding) {
                    if (data[headerEnd + i] != 0.toByte()) {
                        paddingOk = false
                        break
                    }
                }
                if (!paddingOk) continue
                val payloadStart = headerEnd + padding
                val payloadEnd = payloadStart + payloadLength
                if (payloadEnd > data.size) break
                val payload = Arrays.copyOfRange(data, payloadStart, payloadEnd)
                if (CRC64.compute(payload) == checksum) {
                    return padding
                }
            }
            throw ConnectJournalException("failed to locate payload bytes for journal entry")
        }
    }
}
