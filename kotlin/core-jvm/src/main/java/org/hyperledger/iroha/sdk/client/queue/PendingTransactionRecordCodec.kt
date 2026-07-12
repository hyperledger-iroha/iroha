package org.hyperledger.iroha.sdk.client.queue

import java.util.Optional
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.norito.NoritoException

/** Canonical private record codec used only by online transaction retry queues. */
internal object PendingTransactionRecordCodec {
    private const val SCHEMA = "iroha.android.client.PendingTransactionRecord.v1"
    private val stringAdapter: TypeAdapter<String> = NoritoAdapters.stringAdapter()
    private val bytesAdapter: TypeAdapter<ByteArray> = NoritoAdapters.bytesAdapter()
    private val optionalStringAdapter: TypeAdapter<Optional<String>> =
        NoritoAdapters.option(stringAdapter)
    private val optionalBytesAdapter: TypeAdapter<Optional<ByteArray>> =
        NoritoAdapters.option(bytesAdapter)

    @Throws(NoritoException::class)
    fun encode(transaction: SignedTransaction): ByteArray {
        if (transaction.multisigSignatures().isPresent) {
            throw NoritoException("Pending transaction queue does not accept multisig transactions")
        }
        try {
            return NoritoCodec.encode(transaction, SCHEMA, adapter)
        } catch (ex: Exception) {
            throw NoritoException("Failed to encode pending transaction record", ex)
        }
    }

    @Throws(NoritoException::class)
    fun decode(encoded: ByteArray): SignedTransaction {
        try {
            return NoritoCodec.decode(encoded, adapter, SCHEMA)
        } catch (ex: Exception) {
            throw NoritoException("Failed to decode pending transaction record", ex)
        }
    }

    private val adapter = object : TypeAdapter<SignedTransaction> {
        override fun encode(encoder: NoritoEncoder, value: SignedTransaction) {
            stringAdapter.encode(encoder, value.schemaName())
            optionalStringAdapter.encode(encoder, value.keyAlias())
            bytesAdapter.encode(encoder, value.encodedPayload())
            bytesAdapter.encode(encoder, value.signature())
            bytesAdapter.encode(encoder, value.publicKey())
            optionalBytesAdapter.encode(encoder, value.exportedKeyBundle())
            optionalBytesAdapter.encode(encoder, value.blsPublicKey())
        }

        override fun decode(decoder: NoritoDecoder): SignedTransaction =
            SignedTransaction.builder()
                .setSchemaName(stringAdapter.decode(decoder))
                .setKeyAlias(optionalStringAdapter.decode(decoder).orElse(null))
                .setEncodedPayload(bytesAdapter.decode(decoder))
                .setSignature(bytesAdapter.decode(decoder))
                .setPublicKey(bytesAdapter.decode(decoder))
                .setExportedKeyBundle(optionalBytesAdapter.decode(decoder).orElse(null))
                .setBlsPublicKey(optionalBytesAdapter.decode(decoder).orElse(null))
                .build()
    }
}
