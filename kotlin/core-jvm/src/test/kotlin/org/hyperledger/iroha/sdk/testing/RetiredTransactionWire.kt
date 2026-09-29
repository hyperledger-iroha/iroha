package org.hyperledger.iroha.sdk.testing

import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder

/** Malformed first-release transaction vectors for strict decoder rejection. */
object RetiredTransactionWire {
    /** Inserts the removed admission slot before metadata in a nine-field payload. */
    @JvmStatic
    fun insertAdmissionSlot(canonical: ByteArray, tag: Int): ByteArray {
        val reader = NoritoDecoder(canonical, NoritoCodec.DEFAULT_FLAGS)
        repeat(7) {
            val length = reader.readLength(reader.compactLenActive()).toInt()
            reader.readBytes(length)
        }
        val offset = canonical.size - reader.remaining()
        val slot = byteArrayOf(4, tag.toByte(), 0, 0, 0)
        return canonical.copyOfRange(0, offset) + slot + canonical.copyOfRange(offset, canonical.size)
    }
}
