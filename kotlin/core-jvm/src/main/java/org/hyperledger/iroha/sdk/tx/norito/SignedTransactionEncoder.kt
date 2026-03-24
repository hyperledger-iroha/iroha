package org.hyperledger.iroha.sdk.tx.norito

import java.util.Optional
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.MultisigSignature
import org.hyperledger.iroha.sdk.tx.MultisigSignatures
import org.hyperledger.iroha.sdk.tx.SignedTransaction

object SignedTransactionEncoder {

    private const val VERSION_BYTE: Byte = 0x01
    private val PAYLOAD_ADAPTER = TransactionPayloadAdapter()
    private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
    private val SIGNATURE_ADAPTER: TypeAdapter<ByteArray> = TransactionSignatureAdapter()
    private val EMPTY_OPTION_ADAPTER: TypeAdapter<Optional<ByteArray>> =
        NoritoAdapters.option(BYTE_VECTOR_ADAPTER)
    private val MULTISIG_SIGNATURE_ADAPTER: TypeAdapter<MultisigSignature> =
        MultisigSignatureNoritoAdapter()
    private val MULTISIG_SIGNATURE_LIST_ADAPTER: TypeAdapter<List<MultisigSignature>> =
        NoritoAdapters.sequence(MULTISIG_SIGNATURE_ADAPTER)
    private val MULTISIG_SIGNATURES_ADAPTER: TypeAdapter<MultisigSignatures> =
        MultisigSignaturesNoritoAdapter()
    private val MULTISIG_SIGNATURES_OPTION_ADAPTER: TypeAdapter<Optional<MultisigSignatures>> =
        NoritoAdapters.option(MULTISIG_SIGNATURES_ADAPTER)
    private val PAYLOAD_CODEC = NoritoJavaCodecAdapter()

    @JvmStatic
    @Throws(NoritoException::class)
    fun encode(transaction: SignedTransaction): ByteArray {
        val payload: TransactionPayload = PAYLOAD_CODEC.decodeTransaction(transaction.encodedPayload())
        val record = SignedRecord(
            transaction.signature(),
            payload,
            transaction.multisigSignatures(),
        )
        try {
            return NoritoCodec.encodeAdaptive(record, SignedTransactionAdapter).payload()
        } catch (ex: Exception) {
            throw NoritoException("Failed to encode signed transaction", ex)
        }
    }

    @JvmStatic
    @Throws(NoritoException::class)
    fun encodeVersioned(transaction: SignedTransaction): ByteArray {
        val bare = encode(transaction)
        val out = ByteArray(1 + bare.size)
        out[0] = VERSION_BYTE
        System.arraycopy(bare, 0, out, 1, bare.size)
        return out
    }

    private class SignedRecord(
        val signature: ByteArray,
        val payload: TransactionPayload,
        val multisigSignatures: Optional<MultisigSignatures>,
    )

    private object SignedTransactionAdapter : TypeAdapter<SignedRecord> {
        override fun encode(encoder: NoritoEncoder, value: SignedRecord) {
            encodeSizedField(encoder, SIGNATURE_ADAPTER, value.signature)
            encodeSizedField(encoder, PAYLOAD_ADAPTER, value.payload)
            encodeSizedField(encoder, EMPTY_OPTION_ADAPTER, Optional.empty())
            encodeSizedField(encoder, MULTISIG_SIGNATURES_OPTION_ADAPTER, value.multisigSignatures)
        }

        override fun decode(decoder: NoritoDecoder): SignedRecord {
            throw UnsupportedOperationException("Decoding signed transactions is not supported")
        }
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private class TransactionSignatureAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            throw UnsupportedOperationException("Decoding signatures is not supported")
        }
    }

    private class MultisigSignatureNoritoAdapter : TypeAdapter<MultisigSignature> {
        override fun encode(encoder: NoritoEncoder, value: MultisigSignature) {
            BYTE_VECTOR_ADAPTER.encode(encoder, value.publicKeyNoritoPayload())
            BYTE_VECTOR_ADAPTER.encode(encoder, value.signature())
        }

        override fun decode(decoder: NoritoDecoder): MultisigSignature {
            throw UnsupportedOperationException("Decoding multisig signatures is not supported")
        }
    }

    private class MultisigSignaturesNoritoAdapter : TypeAdapter<MultisigSignatures> {
        override fun encode(encoder: NoritoEncoder, value: MultisigSignatures) {
            MULTISIG_SIGNATURE_LIST_ADAPTER.encode(encoder, value.signatures)
        }

        override fun decode(decoder: NoritoDecoder): MultisigSignatures {
            throw UnsupportedOperationException("Decoding multisig signature bundles is not supported")
        }
    }
}
