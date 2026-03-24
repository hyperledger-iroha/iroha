package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.crypto.SigningException
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.tx.norito.NoritoCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.offline.OfflineEnvelopeOptions
import org.hyperledger.iroha.sdk.tx.offline.OfflineSigningEnvelope
import org.hyperledger.iroha.sdk.tx.offline.OfflineTransactionBundle

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

    /**
     * Encodes, signs, and packages the transaction into an [OfflineSigningEnvelope].
     */
    @Throws(NoritoException::class, SigningException::class)
    fun encodeAndSignEnvelope(
        payload: TransactionPayload,
        signer: Signer,
        alias: String,
        options: OfflineEnvelopeOptions = OfflineEnvelopeOptions(),
    ): OfflineSigningEnvelope {
        val transaction = encodeAndSign(payload, signer, alias)
        return envelopeFromSignedTransaction(transaction, alias, options)
    }

    /**
     * Encodes and signs the transaction, returning an [OfflineTransactionBundle]
     * that pairs the envelope with optional attestation material.
     */
    @Throws(NoritoException::class, SigningException::class)
    fun encodeAndSignEnvelopeWithAttestation(
        payload: TransactionPayload,
        signer: Signer,
        alias: String,
        options: OfflineEnvelopeOptions = OfflineEnvelopeOptions(),
        attestation: ByteArray? = null,
    ): OfflineTransactionBundle {
        val envelope = encodeAndSignEnvelope(payload, signer, alias, options)
        return OfflineTransactionBundle(envelope, attestation)
    }

    private fun envelopeFromSignedTransaction(
        transaction: SignedTransaction,
        alias: String,
        options: OfflineEnvelopeOptions,
    ): OfflineSigningEnvelope = OfflineSigningEnvelope(
        encodedPayload = transaction.encodedPayload(),
        signature = transaction.signature(),
        publicKey = transaction.publicKey(),
        schemaName = transaction.schemaName(),
        keyAlias = transaction.keyAlias().orElse(alias),
        issuedAtMs = options.issuedAtMs,
        metadata = options.metadata,
        exportedKeyBundle = options.exportedKeyBundle,
    )

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
