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
        val value = VerifyingKeyBox(backend.trim(), bytes.copyOf())
        require(value.backend.isNotBlank()) { "backend must not be blank" }
        require(value.bytes.isNotEmpty()) { "bytes must not be empty" }
        return NoritoCodec.encode(value, SCHEMA, Adapter, NoritoHeader.COMPACT_LEN)
    }

    @JvmStatic
    fun encode(backend: String, bytes: ByteArray): ByteArray = encodeNorito(backend, bytes)

    private data class VerifyingKeyBox(val backend: String, val bytes: ByteArray) {
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
            writeField(encoder) { writeBytesVec(it, value.bytes) }
        }

        override fun decode(decoder: NoritoDecoder): VerifyingKeyBox {
            throw UnsupportedOperationException("VerifyingKeyBox decoding is not supported")
        }
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

    private fun compact(encoder: NoritoEncoder): Boolean =
        encoder.flags and NoritoHeader.COMPACT_LEN != 0
}
