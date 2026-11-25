package org.hyperledger.iroha.sdk.tx.norito

import java.util.Optional
import org.hyperledger.iroha.sdk.address.decodeCompactPublicKeyPayload
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.norito.WireLayout
import org.hyperledger.iroha.sdk.tx.MultisigSignature
import org.hyperledger.iroha.sdk.tx.MultisigSignatures
import org.hyperledger.iroha.sdk.tx.SignedTransaction

object SignedTransactionEncoder {

    private const val VERSION_BYTE: Byte = 0x01
    private const val SIGNED_SCHEMA = "iroha.transaction.SignedTransaction.v1"
    private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
    private val SIGNATURE_ADAPTER: TypeAdapter<ByteArray> = TransactionSignatureAdapter()
    private val MULTISIG_SIGNATURE_ADAPTER: TypeAdapter<MultisigSignature> =
        MultisigSignatureNoritoAdapter()
    private val MULTISIG_SIGNATURE_LIST_ADAPTER: TypeAdapter<List<MultisigSignature>> =
        NoritoAdapters.sequence(MULTISIG_SIGNATURE_ADAPTER)
    private val MULTISIG_SIGNATURES_ADAPTER: TypeAdapter<MultisigSignatures> =
        MultisigSignaturesNoritoAdapter()
    private val MULTISIG_SIGNATURES_OPTION_ADAPTER: TypeAdapter<Optional<MultisigSignatures>> =
        NoritoAdapters.option(MULTISIG_SIGNATURES_ADAPTER)
    @JvmStatic
    @Throws(NoritoException::class)
    fun encode(transaction: SignedTransaction): ByteArray = encode(transaction, WireLayout.COMPACT)

    /**
     * Encodes the bare signed transaction in an explicit layout.
     *
     * The transaction's payload bytes must already be the exact canonical payload in that
     * layout ([NoritoJavaCodecAdapter] with the same `layout`): the signature covers their hash,
     * and payload bytes of the other layout are refused. [WireLayout.COMPACT] is the current
     * transaction layout; [WireLayout.DENSE] is `norito.md`, "Dense layout (header minor 1)".
     */
    @JvmStatic
    @Throws(NoritoException::class)
    fun encode(transaction: SignedTransaction, layout: WireLayout): ByteArray {
        try {
            val payloadBytes = transaction.encodedPayload()
            TransactionPayloadAdapter.validateCanonicalPayloadBytes(payloadBytes, layout)
            val record = SignedRecord(
                transaction.signature(),
                payloadBytes,
                transaction.multisigSignatures(),
            )
            return TransactionPayloadAdapter.encodeBare(record, SignedTransactionAdapter, layout)
        } catch (ex: Exception) {
            throw NoritoException("Failed to encode signed transaction", ex)
        }
    }

    /**
     * Encodes the versioned wire form. Its version byte names the current compact layout.
     *
     * TODO(dense-activation): no version byte selects the dense layout yet, so there is no
     * versioned dense form.
     */
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
    fun decode(encoded: ByteArray): SignedTransaction = decode(encoded, WireLayout.COMPACT)

    /**
     * Decodes a bare signed transaction in the layout its owner names. Bytes of the other
     * layout are refused rather than reinterpreted.
     */
    @JvmStatic
    @Throws(NoritoException::class)
    fun decode(encoded: ByteArray, layout: WireLayout): SignedTransaction {
        try {
            val record = TransactionPayloadAdapter.decodeBare(encoded, SignedTransactionAdapter, layout)
            TransactionPayloadAdapter.validateCanonicalPayloadBytes(record.payloadBytes, layout)
            return SignedTransaction.builder()
                .setEncodedPayload(record.payloadBytes)
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
        val payloadBytes: ByteArray,
        val multisigSignatures: Optional<MultisigSignatures>,
    )

    private object SignedTransactionAdapter : TypeAdapter<SignedRecord> {
        override fun encode(encoder: NoritoEncoder, value: SignedRecord) {
            encodeSizedField(encoder, SIGNATURE_ADAPTER, value.signature)
            encodeSizedRawField(encoder, value.payloadBytes)
            encodeSizedField(encoder, MULTISIG_SIGNATURES_OPTION_ADAPTER, value.multisigSignatures)
        }

        override fun decode(decoder: NoritoDecoder): SignedRecord {
            val signature = decodeSizedField(decoder, SIGNATURE_ADAPTER, "signature")
            val payloadBytes = decodeSizedRawField(decoder, "payload")
            val multisigSignatures = decodeSizedField(
                decoder,
                MULTISIG_SIGNATURES_OPTION_ADAPTER,
                "multisig_signatures",
            )
            return SignedRecord(signature, payloadBytes, multisigSignatures)
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

    private fun encodeSizedRawField(encoder: NoritoEncoder, payload: ByteArray) {
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private fun decodeSizedRawField(decoder: NoritoDecoder, fieldName: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$fieldName payload too large" }
        return decoder.readBytes(length.toInt())
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
            encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value.publicKeyNoritoPayload())
            encodeSizedField(encoder, BYTE_VECTOR_ADAPTER, value.signature())
        }

        override fun decode(decoder: NoritoDecoder): MultisigSignature {
            val publicKeyPayload = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER, "multisig.signer")
            val signature = decodeSizedField(decoder, BYTE_VECTOR_ADAPTER, "multisig.signature")
            val publicKey = decodeCompactPublicKeyPayload(publicKeyPayload)
                ?: throw IllegalArgumentException("Invalid multisig public key payload")
            return MultisigSignature.fromCurveId(publicKey.curveId, publicKey.keyBytes, signature)
        }
    }

    private class MultisigSignaturesNoritoAdapter : TypeAdapter<MultisigSignatures> {
        override fun encode(encoder: NoritoEncoder, value: MultisigSignatures) {
            encodeSizedField(encoder, MULTISIG_SIGNATURE_LIST_ADAPTER, value.signatures)
        }

        override fun decode(decoder: NoritoDecoder): MultisigSignatures {
            return MultisigSignatures.of(decodeSizedField(decoder, MULTISIG_SIGNATURE_LIST_ADAPTER, "multisig.signatures"))
        }
    }

    private fun <T> decodeSizedField(decoder: NoritoDecoder, adapter: TypeAdapter<T>, fieldName: String): T {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$fieldName payload too large" }
        val payload = decoder.readBytes(length.toInt())
        val child = NoritoDecoder(payload, decoder.flags)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after $fieldName payload" }
        return value
    }
}
