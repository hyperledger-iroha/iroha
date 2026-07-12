package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.crypto.SigningException
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.tx.norito.NoritoCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException

/**
 * Encodes transaction payloads via Norito and attaches signatures.
 */
class TransactionBuilder(
    private val codecAdapter: NoritoCodecAdapter,
) {

    /** Encodes the payload and signs it using the provided signer. */
    @Throws(NoritoException::class, SigningException::class)
    fun encodeAndSign(payload: TransactionPayload, signer: Signer): SignedTransaction =
        encodeAndSignInternal(payload, signer, null)

    /** Encodes the payload and signs it using the provided signer with a key alias. */
    @Throws(NoritoException::class, SigningException::class)
    fun encodeAndSign(payload: TransactionPayload, signer: Signer, alias: String): SignedTransaction =
        encodeAndSignInternal(payload, signer, alias)

    private fun encodeAndSignInternal(
        payload: TransactionPayload,
        signer: Signer,
        alias: String?,
    ): SignedTransaction {
        val encoded = codecAdapter.encodeTransaction(payload)
        val signature = signer.sign(encoded)
        return SignedTransaction.builder()
            .setEncodedPayload(encoded)
            .setSignature(signature)
            .setPublicKey(signer.publicKey())
            .setSchemaName(codecAdapter.schemaName())
            .setKeyAlias(alias)
            .setBlsPublicKey(signer.blsPublicKey())
            .build()
    }
}
