package org.hyperledger.iroha.sdk.tx.offline

import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.tx.norito.NoritoException

private const val DEFAULT_SCHEMA = "iroha.android.offline.Envelope.v1"

/** Encodes and decodes `OfflineSigningEnvelope` instances using the shared Norito codec. */
class OfflineSigningEnvelopeCodec @JvmOverloads constructor(
    @JvmField val schemaName: String = DEFAULT_SCHEMA,
) {
    private val adapter = OfflineSigningEnvelopeAdapter()

    @Throws(NoritoException::class)
    fun encode(envelope: OfflineSigningEnvelope): ByteArray {
        try {
            return NoritoCodec.encode(envelope, schemaName, adapter)
        } catch (ex: Exception) {
            throw NoritoException("Failed to encode offline signing envelope", ex)
        }
    }

    @Throws(NoritoException::class)
    fun decode(encoded: ByteArray): OfflineSigningEnvelope {
        try {
            return NoritoCodec.decode(encoded, adapter, schemaName)
        } catch (ex: Exception) {
            throw NoritoException("Failed to decode offline signing envelope", ex)
        }
    }
}
