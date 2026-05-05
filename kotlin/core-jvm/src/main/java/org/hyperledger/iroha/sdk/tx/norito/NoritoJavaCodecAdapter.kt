package org.hyperledger.iroha.sdk.tx.norito

import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoHeader

class NoritoJavaCodecAdapter @JvmOverloads constructor(
    private val schemaName: String = DEFAULT_SCHEMA,
) : NoritoCodecAdapter {

    private val adapter = TransactionPayloadAdapter()

    @Throws(NoritoException::class)
    override fun encodeTransaction(payload: TransactionPayload): ByteArray {
        try {
            return NoritoCodec.encodeAdaptive(payload, adapter).payload()
        } catch (ex: Exception) {
            throw NoritoException("Failed to encode Norito transaction payload", ex)
        }
    }

    @Throws(NoritoException::class)
    override fun decodeTransaction(encoded: ByteArray): TransactionPayload {
        try {
            if (hasHeader(encoded)) {
                return NoritoCodec.decode(encoded, adapter, schemaName)
            }
            return NoritoCodec.decodeAdaptive(encoded, adapter)
        } catch (ex: Exception) {
            throw NoritoException("Failed to decode Norito transaction payload", ex)
        }
    }

    override fun schemaName(): String = schemaName

    companion object {
        private const val DEFAULT_SCHEMA = "iroha.android.transaction.Payload.v1"

        private fun hasHeader(encoded: ByteArray): Boolean {
            if (encoded.size < NoritoHeader.HEADER_LENGTH) return false
            return encoded[0] == 'N'.code.toByte()
                && encoded[1] == 'R'.code.toByte()
                && encoded[2] == 'T'.code.toByte()
                && encoded[3] == '0'.code.toByte()
        }
    }
}
