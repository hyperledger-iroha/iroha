package org.hyperledger.iroha.sdk.tx.norito

import org.hyperledger.iroha.sdk.core.model.TransactionPayload

interface NoritoCodecAdapter {

    @Throws(NoritoException::class)
    fun encodeTransaction(payload: TransactionPayload): ByteArray

    @Throws(NoritoException::class)
    fun decodeTransaction(encoded: ByteArray): TransactionPayload

    fun schemaName(): String = "iroha.android.transaction"
}
