package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.crypto.SigningException
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.tx.norito.NoritoCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException

/**
 * Encodes transaction payloads via Norito and attaches signatures.
 */
class TransactionBuilder(
    private val codecAdapter: NoritoCodecAdapter,
) {

    /**
     * Encodes the payload for public Torii submission and signs it using the provided signer.
     *
     * Public submission requires the signature-bound QueuePlan admission marker. The caller's
     * payload remains unchanged so direct codec users continue to produce ordinary transactions.
     */
    @Throws(NoritoException::class, SigningException::class)
    fun encodeAndSign(payload: TransactionPayload, signer: Signer): SignedTransaction =
        encodeAndSignInternal(payload.withQueuePlanSyncedAdmission(), signer, null)

    /** Encodes a public-submission payload and signs it using the provided signer with a key alias. */
    @Throws(NoritoException::class, SigningException::class)
    fun encodeAndSign(payload: TransactionPayload, signer: Signer, alias: String): SignedTransaction =
        encodeAndSignInternal(payload.withQueuePlanSyncedAdmission(), signer, alias)

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

    private fun TransactionPayload.withQueuePlanSyncedAdmission(): TransactionPayload {
        val signedMetadata = LinkedHashMap(metadata)
        signedMetadata[QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY] = JsonValue.bool(true)
        return copy(metadata = signedMetadata)
    }

    companion object {
        /** Signature-bound metadata key selecting globally certified QueuePlan admission. */
        const val QUEUE_PLAN_SYNCED_ADMISSION_METADATA_KEY =
            "iroha_transaction_admission_queue_plan_synced"
    }
}
