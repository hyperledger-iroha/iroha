package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.crypto.SigningException
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.crypto.SignatureAdmission
import org.hyperledger.iroha.sdk.crypto.SigningAlgorithm
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
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
     * Public submission requires the signature-bound QueuePlan admission intent. The caller's
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
        val algorithm = try {
            SigningAlgorithm.fromAlgorithmName(signer.algorithm())
        } catch (error: IllegalArgumentException) {
            throw SigningException("Unsupported signer algorithm", error)
        }
        val encoded = codecAdapter.encodeTransaction(payload)
        val signature = signer.sign(encoded)
        if (!SignatureAdmission.isValid(algorithm, signature)) {
            val expectedLength = when (algorithm) {
                SigningAlgorithm.ED25519 -> SignatureAdmission.ED25519_SIGNATURE_LENGTH
                SigningAlgorithm.ML_DSA -> SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH
                else -> throw SigningException("${algorithm.providerName} signer returned no signature")
            }
            throw SigningException(
                "${algorithm.providerName} signer returned a malformed signature; expected $expectedLength nonzero bytes",
            )
        }
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
        return copy(admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
    }
}
