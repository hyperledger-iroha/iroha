package org.hyperledger.iroha.sdk.tx.norito

import java.util.Optional
import org.hyperledger.iroha.sdk.address.decodeCompactPublicKeyPayload
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
    private const val SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1"
    private val PAYLOAD_ADAPTER = TransactionPayloadAdapter()
    private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
    private val SIGNATURE_ADAPTER: TypeAdapter<ByteArray> = TransactionSignatureAdapter()
    private val ATTACHMENTS_OPTION_ADAPTER: TypeAdapter<Optional<ByteArray>> =
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

    @JvmStatic
    @Throws(NoritoException::class)
    fun decode(encoded: ByteArray): SignedTransaction {
        try {
            val record = NoritoCodec.decodeAdaptive(encoded, SignedTransactionAdapter)
            val payloadBytes = PAYLOAD_CODEC.encodeTransaction(record.payload)
            return SignedTransaction.builder()
                .setEncodedPayload(payloadBytes)
                .setSignature(record.signature)
                .setPublicKey(ByteArray(0))
                .setSchemaName(SIGNED_SCHEMA)
                .setMultisigSignatures(record.multisigSignatures.orElse(null))
                .build()
        } catch (ex: Exception) {
            throw NoritoException("Failed to decode signed transaction", ex)
        }
    }

    @JvmStatic
    @Throws(NoritoException::class)
    fun decodeVersioned(encoded: ByteArray): SignedTransaction {
        try {
            require(encoded.isNotEmpty()) { "Versioned signed transaction must not be empty" }
            require(encoded[0] == VERSION_BYTE) {
                "Unsupported signed transaction version byte: ${encoded[0].toInt() and 0xFF}"
            }
            return decode(encoded.copyOfRange(1, encoded.size))
        } catch (ex: NoritoException) {
            throw ex
        } catch (ex: Exception) {
            throw NoritoException("Failed to decode versioned signed transaction", ex)
        }
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
            encodeSizedField(encoder, ATTACHMENTS_OPTION_ADAPTER, Optional.empty())
            encodeSizedField(encoder, MULTISIG_SIGNATURES_OPTION_ADAPTER, value.multisigSignatures)
        }

        override fun decode(decoder: NoritoDecoder): SignedRecord {
            val signature = decodeSizedField(decoder, SIGNATURE_ADAPTER, "signature")
            val payload = decodeSizedField(decoder, PAYLOAD_ADAPTER, "payload")
            val attachments = decodeSizedField(decoder, ATTACHMENTS_OPTION_ADAPTER, "attachments")
            require(!attachments.isPresent) { "Signed transaction attachments are not supported" }
            val multisigSignatures = decodeSizedField(
                decoder,
                MULTISIG_SIGNATURES_OPTION_ADAPTER,
                "multisig_signatures",
            )
            return SignedRecord(signature, payload, multisigSignatures)
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
            return decodeSizedField(decoder, BYTE_VECTOR_ADAPTER, "signature.bytes")
        }
    }

    private class MultisigSignatureNoritoAdapter : TypeAdapter<MultisigSignature> {
        override fun encode(encoder: NoritoEncoder, value: MultisigSignature) {
            BYTE_VECTOR_ADAPTER.encode(encoder, value.publicKeyNoritoPayload())
            BYTE_VECTOR_ADAPTER.encode(encoder, value.signature())
        }

        override fun decode(decoder: NoritoDecoder): MultisigSignature {
            val publicKeyPayload = BYTE_VECTOR_ADAPTER.decode(decoder)
            val signature = BYTE_VECTOR_ADAPTER.decode(decoder)
            val publicKey = decodeCompactPublicKeyPayload(publicKeyPayload)
                ?: throw IllegalArgumentException("Invalid multisig public key payload")
            return MultisigSignature.fromCurveId(publicKey.curveId, publicKey.keyBytes, signature)
        }
    }

    private class MultisigSignaturesNoritoAdapter : TypeAdapter<MultisigSignatures> {
        override fun encode(encoder: NoritoEncoder, value: MultisigSignatures) {
            MULTISIG_SIGNATURE_LIST_ADAPTER.encode(encoder, value.signatures)
        }

        override fun decode(decoder: NoritoDecoder): MultisigSignatures {
            return MultisigSignatures.of(MULTISIG_SIGNATURE_LIST_ADAPTER.decode(decoder))
        }
    }

    private fun <T> decodeSizedField(decoder: NoritoDecoder, adapter: TypeAdapter<T>, fieldName: String): T {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$fieldName payload too large" }
        val payload = decoder.readBytes(length.toInt())
        val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after $fieldName payload" }
        return value
    }
}
