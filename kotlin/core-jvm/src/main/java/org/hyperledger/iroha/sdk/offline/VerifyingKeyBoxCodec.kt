package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Norito encoder for chain-supplied Offline Note `VerifyingKeyBox` records. */
object VerifyingKeyBoxCodec {
    private const val SCHEMA = "iroha_data_model::proof::VerifyingKeyBox"

    @JvmStatic
    fun encodeNorito(backend: String, bytes: ByteArray): ByteArray {
        val value = VerifyingKeyBox(backend, bytes)
        return NoritoCodec.encode(value, SCHEMA, Adapter, NoritoHeader.COMPACT_LEN)
    }

    @JvmStatic
    fun encode(backend: String, bytes: ByteArray): ByteArray = encodeNorito(backend, bytes)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): VerifyingKeyBox =
        NoritoCodec.decode(payload, Adapter, SCHEMA)

    @JvmStatic
    fun decode(payload: ByteArray): VerifyingKeyBox = decodeNorito(payload)

    class VerifyingKeyBox(
        val backend: String,
        bytes: ByteArray,
    ) {
        private val bytes: ByteArray = bytes.copyOf()

        init {
            requireNonBlankUnpadded(backend, "backend")
            require(this.bytes.isNotEmpty()) { "bytes must not be empty" }
        }

        fun bytes(): ByteArray = bytes.copyOf()

        override fun equals(other: Any?): Boolean {
            return other is VerifyingKeyBox &&
                backend == other.backend &&
                bytes.contentEquals(other.bytes)
        }

        override fun hashCode(): Int = 31 * backend.hashCode() + bytes.contentHashCode()
    }

    private object Adapter : TypeAdapter<VerifyingKeyBox> {
        override fun encode(encoder: NoritoEncoder, value: VerifyingKeyBox) {
            writeField(encoder) { writeString(it, value.backend) }
            writeField(encoder) { writeBytesVec(it, value.bytes()) }
        }

        override fun decode(decoder: NoritoDecoder): VerifyingKeyBox =
            VerifyingKeyBox(
                backend = readField(decoder) { readString(it) },
                bytes = readField(decoder) { readBytesVec(it) },
            )
    }

    private fun writeField(parent: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
        val child = parent.childEncoder()
        writePayload(child)
        val payload = child.toByteArray()
        parent.writeLength(payload.size.toLong(), compact(parent))
        parent.writeBytes(payload)
    }

    private fun writeString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), compact(encoder))
        encoder.writeBytes(bytes)
    }

    private fun writeBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun <T> readField(parent: NoritoDecoder, readPayload: (NoritoDecoder) -> T): T {
        val length = checkedLength(parent.readLength(compact(parent)), "field length")
        val child = NoritoDecoder(parent.readBytes(length), parent.flags, parent.flagsHint)
        val value = readPayload(child)
        require(child.remaining() == 0) {
            "Trailing bytes after VerifyingKeyBox field decode"
        }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = checkedLength(decoder.readLength(compact(decoder)), "string length")
        return String(decoder.readBytes(length), StandardCharsets.UTF_8)
    }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = checkedLength(decoder.readUInt(64), "byte vector length")
        return decoder.readBytes(length)
    }

    private fun checkedLength(value: Long, field: String): Int {
        require(value >= 0) { "$field must be non-negative" }
        require(value <= Int.MAX_VALUE) { "$field exceeds JVM array limit" }
        return value.toInt()
    }

    private fun compact(encoder: NoritoEncoder): Boolean =
        encoder.flags and NoritoHeader.COMPACT_LEN != 0

    private fun compact(decoder: NoritoDecoder): Boolean =
        decoder.flags and NoritoHeader.COMPACT_LEN != 0

    private fun requireNonBlankUnpadded(value: String, field: String): String {
        require(value.trim().isNotEmpty()) { "$field must not be blank" }
        require(value.trim() == value) { "$field must not contain surrounding whitespace" }
        return value
    }
}
