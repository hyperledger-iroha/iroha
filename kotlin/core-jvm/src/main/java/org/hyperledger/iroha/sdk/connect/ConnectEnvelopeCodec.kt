package org.hyperledger.iroha.sdk.connect

import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Norito codec for encrypted Connect envelopes used by wallet-role flows. */
object ConnectEnvelopeCodec {

    private const val ENVELOPE_SCHEMA_PATH = "iroha_torii_shared::connect::EnvelopeV1"
    private const val CONNECT_LAYOUT_FLAGS = 0

    private const val PAYLOAD_CONTROL = 0
    private const val PAYLOAD_SIGN_REQUEST_RAW = 1
    private const val PAYLOAD_SIGN_REQUEST_TX = 2
    private const val PAYLOAD_SIGN_RESULT_OK = 3
    private const val PAYLOAD_SIGN_RESULT_ERR = 4
    private const val PAYLOAD_DISPLAY_REQUEST = 5

    private const val CONTROL_CLOSE = 0
    private const val CONTROL_REJECT = 1

    private val UINT16 = NoritoAdapters.uint(16)
    private val UINT32 = NoritoAdapters.uint(32)
    private val UINT64 = NoritoAdapters.uint(64)
    private val STRING = NoritoAdapters.stringAdapter()
    private val BOOL = NoritoAdapters.boolAdapter()
    private val BYTE_VECTOR = NoritoAdapters.byteVecAdapter()
    private val RAW_BYTES = NoritoAdapters.rawByteVecAdapter()

    enum class PayloadKind {
        CONTROL_CLOSE,
        CONTROL_REJECT,
        SIGN_REQUEST_RAW,
        SIGN_REQUEST_TX,
        SIGN_RESULT_OK,
        SIGN_RESULT_ERR,
        DISPLAY_REQUEST,
        UNKNOWN,
    }

    interface EnvelopePayload {
        fun kind(): PayloadKind
    }

    class UnknownPayload internal constructor(@JvmField val tag: Int) : EnvelopePayload {
        override fun kind(): PayloadKind = PayloadKind.UNKNOWN
    }

    class ControlClosePayload internal constructor(
        @JvmField val role: ConnectRole,
        @JvmField val code: Int,
        @JvmField val reason: String,
        @JvmField val retryable: Boolean,
    ) : EnvelopePayload {
        override fun kind(): PayloadKind = PayloadKind.CONTROL_CLOSE
    }

    class ControlRejectPayload internal constructor(
        @JvmField val code: Int,
        @JvmField val codeId: String,
        @JvmField val reason: String,
    ) : EnvelopePayload {
        override fun kind(): PayloadKind = PayloadKind.CONTROL_REJECT
    }

    class SignRequestRawPayload internal constructor(
        @JvmField val domainTag: String,
        bytes: ByteArray,
    ) : EnvelopePayload {
        private val _bytes: ByteArray = bytes.copyOf()
        fun bytes(): ByteArray = _bytes.clone()
        override fun kind(): PayloadKind = PayloadKind.SIGN_REQUEST_RAW
    }

    class SignRequestTxPayload internal constructor(txBytes: ByteArray) : EnvelopePayload {
        private val _txBytes: ByteArray = txBytes.copyOf()
        fun txBytes(): ByteArray = _txBytes.clone()
        override fun kind(): PayloadKind = PayloadKind.SIGN_REQUEST_TX
    }

    class SignResultOkPayload internal constructor(
        @JvmField val algorithm: String,
        signature: ByteArray,
    ) : EnvelopePayload {
        private val _signature: ByteArray = signature.copyOf()
        fun signature(): ByteArray = _signature.clone()
        override fun kind(): PayloadKind = PayloadKind.SIGN_RESULT_OK
    }

    class SignResultErrPayload internal constructor(
        @JvmField val code: String,
        @JvmField val message: String,
    ) : EnvelopePayload {
        override fun kind(): PayloadKind = PayloadKind.SIGN_RESULT_ERR
    }

    class DisplayRequestPayload internal constructor(
        @JvmField val title: String,
        @JvmField val body: String,
    ) : EnvelopePayload {
        override fun kind(): PayloadKind = PayloadKind.DISPLAY_REQUEST
    }

    class DecodedEnvelope internal constructor(
        @JvmField val sequence: Long,
        @JvmField val payload: EnvelopePayload,
    )

    private class EnvelopeValue(val sequence: Long, val payload: EnvelopePayload)

    private class WalletSignature(val algorithm: Int, signature: ByteArray) {
        val signature: ByteArray = signature.copyOf()
    }

    private val ROLE_ADAPTER: TypeAdapter<ConnectRole> =
        object : TypeAdapter<ConnectRole> {
            override fun encode(encoder: NoritoEncoder, value: ConnectRole) {
                UINT32.encode(
                    encoder,
                    if (value == ConnectRole.WALLET) 1L else 0L,
                )
            }

            override fun decode(decoder: NoritoDecoder): ConnectRole {
                val tag = UINT32.decode(decoder).toInt()
                if (tag == 0) return ConnectRole.APP
                if (tag == 1) return ConnectRole.WALLET
                throw IllegalArgumentException("Unknown Connect role tag: $tag")
            }
        }

    private val WALLET_SIGNATURE_ADAPTER: TypeAdapter<WalletSignature> =
        object : TypeAdapter<WalletSignature> {
            override fun encode(encoder: NoritoEncoder, value: WalletSignature) {
                val algorithmField: ByteArray
                run {
                    val child = encoder.childEncoder()
                    UINT32.encode(child, value.algorithm.toLong())
                    algorithmField = child.toByteArray()
                }
                val signatureField: ByteArray
                run {
                    val child = encoder.childEncoder()
                    BYTE_VECTOR.encode(child, value.signature)
                    signatureField = child.toByteArray()
                }
                val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
                encoder.writeLength(algorithmField.size.toLong(), compactLen)
                encoder.writeBytes(algorithmField)
                encoder.writeLength(signatureField.size.toLong(), compactLen)
                encoder.writeBytes(signatureField)
            }

            override fun decode(decoder: NoritoDecoder): WalletSignature {
                val compactLen = decoder.compactLenActive()

                val algorithmLength = decoder.readLength(compactLen)
                val algorithmBytes = decoder.readBytes(algorithmLength.toInt())
                val algorithmDecoder = NoritoDecoder(algorithmBytes, decoder.flags, decoder.flagsHint)
                val algorithm = UINT32.decode(algorithmDecoder).toInt()
                if (algorithmDecoder.remaining() != 0) {
                    throw IllegalArgumentException(
                        "wallet_signature.algorithm trailing bytes: ${algorithmDecoder.remaining()}",
                    )
                }

                val signatureLength = decoder.readLength(compactLen)
                val signatureBytes = decoder.readBytes(signatureLength.toInt())
                val signatureDecoder = NoritoDecoder(signatureBytes, decoder.flags, decoder.flagsHint)
                val signature = BYTE_VECTOR.decode(signatureDecoder)
                if (signatureDecoder.remaining() != 0) {
                    throw IllegalArgumentException(
                        "wallet_signature.signature trailing bytes: ${signatureDecoder.remaining()}",
                    )
                }
                return WalletSignature(algorithm, signature)
            }
        }

    private val CONTROL_AFTER_KEY_ADAPTER: TypeAdapter<EnvelopePayload> =
        object : TypeAdapter<EnvelopePayload> {
            override fun encode(encoder: NoritoEncoder, value: EnvelopePayload) {
                when (value) {
                    is ControlClosePayload -> {
                        UINT32.encode(encoder, CONTROL_CLOSE.toLong())
                        writeField(encoder, ROLE_ADAPTER, value.role)
                        writeField(encoder, UINT16, value.code.toLong())
                        writeField(encoder, STRING, value.reason)
                        writeField(encoder, BOOL, value.retryable)
                    }
                    is ControlRejectPayload -> {
                        UINT32.encode(encoder, CONTROL_REJECT.toLong())
                        writeField(encoder, UINT16, value.code.toLong())
                        writeField(encoder, STRING, value.codeId)
                        writeField(encoder, STRING, value.reason)
                    }
                    else -> throw IllegalArgumentException(
                        "Unsupported control payload: ${value.javaClass.name}",
                    )
                }
            }

            override fun decode(decoder: NoritoDecoder): EnvelopePayload {
                val tag = UINT32.decode(decoder).toInt()
                if (tag == CONTROL_CLOSE) {
                    val role = decodeField(decoder, ROLE_ADAPTER, "control.close.role")
                    val code = decodeField(decoder, UINT16, "control.close.code").toInt()
                    val reason = decodeField(decoder, STRING, "control.close.reason")
                    val retryable = decodeField(decoder, BOOL, "control.close.retryable")
                    return ControlClosePayload(role, code, reason, retryable)
                }
                if (tag == CONTROL_REJECT) {
                    val code = decodeField(decoder, UINT16, "control.reject.code").toInt()
                    val codeId = decodeField(decoder, STRING, "control.reject.code_id")
                    val reason = decodeField(decoder, STRING, "control.reject.reason")
                    return ControlRejectPayload(code, codeId, reason)
                }
                return UnknownPayload(tag)
            }
        }

    private val PAYLOAD_ADAPTER: TypeAdapter<EnvelopePayload> =
        object : TypeAdapter<EnvelopePayload> {
            override fun encode(encoder: NoritoEncoder, value: EnvelopePayload) {
                when (value) {
                    is ControlClosePayload, is ControlRejectPayload -> {
                        UINT32.encode(encoder, PAYLOAD_CONTROL.toLong())
                        writeField(encoder, CONTROL_AFTER_KEY_ADAPTER, value)
                    }
                    is SignRequestRawPayload -> {
                        UINT32.encode(encoder, PAYLOAD_SIGN_REQUEST_RAW.toLong())
                        writeField(encoder, STRING, value.domainTag)
                        writeField(encoder, RAW_BYTES, value.bytes())
                    }
                    is SignRequestTxPayload -> {
                        UINT32.encode(encoder, PAYLOAD_SIGN_REQUEST_TX.toLong())
                        writeField(encoder, RAW_BYTES, value.txBytes())
                    }
                    is SignResultOkPayload -> {
                        UINT32.encode(encoder, PAYLOAD_SIGN_RESULT_OK.toLong())
                        writeField(
                            encoder,
                            WALLET_SIGNATURE_ADAPTER,
                            WalletSignature(algorithmTag(value.algorithm), value.signature()),
                        )
                    }
                    is SignResultErrPayload -> {
                        UINT32.encode(encoder, PAYLOAD_SIGN_RESULT_ERR.toLong())
                        writeField(encoder, STRING, value.code)
                        writeField(encoder, STRING, value.message)
                    }
                    is DisplayRequestPayload -> {
                        UINT32.encode(encoder, PAYLOAD_DISPLAY_REQUEST.toLong())
                        writeField(encoder, STRING, value.title)
                        writeField(encoder, STRING, value.body)
                    }
                    else -> throw IllegalArgumentException(
                        "Unsupported envelope payload: ${value.javaClass.name}",
                    )
                }
            }

            override fun decode(decoder: NoritoDecoder): EnvelopePayload {
                val tag = UINT32.decode(decoder).toInt()
                return when (tag) {
                    PAYLOAD_CONTROL ->
                        decodeField(decoder, CONTROL_AFTER_KEY_ADAPTER, "payload.control")
                    PAYLOAD_SIGN_REQUEST_RAW -> {
                        val domainTag = decodeField(decoder, STRING, "payload.sign_request_raw.domain_tag")
                        val bytes = decodeField(decoder, RAW_BYTES, "payload.sign_request_raw.bytes")
                        SignRequestRawPayload(domainTag, bytes)
                    }
                    PAYLOAD_SIGN_REQUEST_TX -> {
                        val txBytes = decodeField(decoder, RAW_BYTES, "payload.sign_request_tx.tx_bytes")
                        SignRequestTxPayload(txBytes)
                    }
                    PAYLOAD_SIGN_RESULT_OK -> {
                        val signature = decodeField(
                            decoder, WALLET_SIGNATURE_ADAPTER, "payload.sign_result_ok.signature",
                        )
                        SignResultOkPayload(algorithmName(signature.algorithm), signature.signature)
                    }
                    PAYLOAD_SIGN_RESULT_ERR -> {
                        val code = decodeField(decoder, STRING, "payload.sign_result_err.code")
                        val message = decodeField(decoder, STRING, "payload.sign_result_err.message")
                        SignResultErrPayload(code, message)
                    }
                    PAYLOAD_DISPLAY_REQUEST -> {
                        val title = decodeField(decoder, STRING, "payload.display_request.title")
                        val body = decodeField(decoder, STRING, "payload.display_request.body")
                        DisplayRequestPayload(title, body)
                    }
                    else -> {
                        decoder.readBytes(decoder.remaining())
                        UnknownPayload(tag)
                    }
                }
            }
        }

    private val ENVELOPE_ADAPTER: TypeAdapter<EnvelopeValue> =
        object : TypeAdapter<EnvelopeValue> {
            override fun encode(encoder: NoritoEncoder, value: EnvelopeValue) {
                writeField(encoder, UINT64, value.sequence)
                writeField(encoder, PAYLOAD_ADAPTER, value.payload)
            }

            override fun decode(decoder: NoritoDecoder): EnvelopeValue {
                val sequence = decodeField(decoder, UINT64, "envelope.seq")
                val payload = decodeField(decoder, PAYLOAD_ADAPTER, "envelope.payload")
                return EnvelopeValue(sequence, payload)
            }
        }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun decodeEnvelope(framedEnvelope: ByteArray): DecodedEnvelope {
        try {
            val decoded = NoritoCodec.decode(framedEnvelope, ENVELOPE_ADAPTER, null)
            return DecodedEnvelope(decoded.sequence, decoded.payload)
        } catch (ex: RuntimeException) {
            val compatDecoded = tryDecodeEnvelopeCompat(framedEnvelope)
            if (compatDecoded != null) return compatDecoded
            throw ConnectProtocolException("Failed to decode Connect envelope", ex)
        }
    }

    private fun tryDecodeEnvelopeCompat(framedEnvelope: ByteArray): DecodedEnvelope? {
        try {
            val archive = NoritoCodec.fromBytesView(framedEnvelope, null)
            val decoder = NoritoDecoder(archive.asBytes(), archive.flags, archive.flagsHint)

            val compactLen = decoder.compactLenActive()
            val seqFieldLen = decoder.readLength(compactLen)
            if (seqFieldLen <= 0 || seqFieldLen > Int.MAX_VALUE || seqFieldLen > decoder.remaining()) {
                return null
            }
            val seqFieldBytes = decoder.readBytes(seqFieldLen.toInt())
            val seqDecoder = NoritoDecoder(seqFieldBytes, archive.flags, archive.flagsHint)
            val sequence = UINT64.decode(seqDecoder)
            if (seqDecoder.remaining() != 0) return null

            val payloadFieldLen = decoder.readLength(compactLen)
            if (payloadFieldLen <= 0 || payloadFieldLen > Int.MAX_VALUE || payloadFieldLen > decoder.remaining()) {
                return null
            }
            val payloadFieldBytes = decoder.readBytes(payloadFieldLen.toInt())
            if (decoder.remaining() != 0) return null

            val payload = decodeCompatPayload(payloadFieldBytes, archive.flags, archive.flagsHint)
                ?: return null
            return DecodedEnvelope(sequence, payload)
        } catch (_: RuntimeException) {
            return null
        }
    }

    private fun decodeCompatPayload(
        payloadFieldBytes: ByteArray,
        flags: Int,
        flagsHint: Int,
    ): EnvelopePayload? {
        val payloadDecoder = NoritoDecoder(payloadFieldBytes, flags, flagsHint)
        val tag = UINT32.decode(payloadDecoder).toInt()
        val compactLen = payloadDecoder.compactLenActive()
        val bodyLength = payloadDecoder.readLength(compactLen)
        if (bodyLength < 0 || bodyLength > Int.MAX_VALUE || bodyLength > payloadDecoder.remaining()) {
            return null
        }
        val bodyBytes = payloadDecoder.readBytes(bodyLength.toInt())
        if (payloadDecoder.remaining() != 0) return null

        val bodyDecoder = NoritoDecoder(bodyBytes, flags, flagsHint)
        when (tag) {
            PAYLOAD_CONTROL -> {
                val payload = CONTROL_AFTER_KEY_ADAPTER.decode(bodyDecoder)
                if (bodyDecoder.remaining() != 0) return null
                return payload
            }
            PAYLOAD_SIGN_REQUEST_RAW -> {
                val domainTag = decodeField(bodyDecoder, STRING, "payload.sign_request_raw.domain_tag")
                val bytes = decodeField(bodyDecoder, RAW_BYTES, "payload.sign_request_raw.bytes")
                if (bodyDecoder.remaining() != 0) return null
                return SignRequestRawPayload(domainTag, bytes)
            }
            PAYLOAD_SIGN_REQUEST_TX -> {
                val txBytes = decodeRawBytesOrBody(bodyBytes, flags, flagsHint)
                return SignRequestTxPayload(txBytes)
            }
            PAYLOAD_SIGN_RESULT_OK -> {
                val signature = WALLET_SIGNATURE_ADAPTER.decode(bodyDecoder)
                if (bodyDecoder.remaining() != 0) return null
                return SignResultOkPayload(algorithmName(signature.algorithm), signature.signature)
            }
            PAYLOAD_SIGN_RESULT_ERR -> {
                val code = decodeField(bodyDecoder, STRING, "payload.sign_result_err.code")
                val message = decodeField(bodyDecoder, STRING, "payload.sign_result_err.message")
                if (bodyDecoder.remaining() != 0) return null
                return SignResultErrPayload(code, message)
            }
            PAYLOAD_DISPLAY_REQUEST -> {
                val title = decodeField(bodyDecoder, STRING, "payload.display_request.title")
                val body = decodeField(bodyDecoder, STRING, "payload.display_request.body")
                if (bodyDecoder.remaining() != 0) return null
                return DisplayRequestPayload(title, body)
            }
            else -> return null
        }
    }

    private fun decodeRawBytesOrBody(
        bodyBytes: ByteArray,
        flags: Int,
        flagsHint: Int,
    ): ByteArray {
        try {
            val bodyDecoder = NoritoDecoder(bodyBytes, flags, flagsHint)
            val decoded = RAW_BYTES.decode(bodyDecoder)
            if (bodyDecoder.remaining() == 0) return decoded
        } catch (_: RuntimeException) {
            // Fallback to raw body bytes below.
        }
        return bodyBytes.copyOf()
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeSignResultOkEnvelope(
        sequence: Long,
        signature: ByteArray,
        algorithm: String,
    ): ByteArray =
        encodeEnvelope(EnvelopeValue(sequence, SignResultOkPayload(algorithm, signature)))

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeSignRequestRawEnvelope(
        sequence: Long,
        domainTag: String,
        bytes: ByteArray,
    ): ByteArray =
        encodeEnvelope(EnvelopeValue(sequence, SignRequestRawPayload(domainTag, bytes)))

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeSignRequestTxEnvelope(sequence: Long, txBytes: ByteArray): ByteArray =
        encodeEnvelope(EnvelopeValue(sequence, SignRequestTxPayload(txBytes)))

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeSignResultErrEnvelope(
        sequence: Long,
        code: String,
        message: String,
    ): ByteArray =
        encodeEnvelope(EnvelopeValue(sequence, SignResultErrPayload(code, message)))

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeControlRejectEnvelope(
        sequence: Long,
        code: Int,
        codeId: String,
        reason: String,
    ): ByteArray =
        encodeEnvelope(EnvelopeValue(sequence, ControlRejectPayload(code, codeId, reason)))

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeControlCloseEnvelope(
        sequence: Long,
        role: ConnectRole,
        code: Int,
        reason: String,
        retryable: Boolean,
    ): ByteArray =
        encodeEnvelope(EnvelopeValue(sequence, ControlClosePayload(role, code, reason, retryable)))

    @Throws(ConnectProtocolException::class)
    private fun encodeEnvelope(value: EnvelopeValue): ByteArray {
        try {
            return NoritoCodec.encode(
                value,
                ENVELOPE_SCHEMA_PATH,
                ENVELOPE_ADAPTER,
                CONNECT_LAYOUT_FLAGS,
            )
        } catch (ex: RuntimeException) {
            throw ConnectProtocolException("Failed to encode Connect envelope", ex)
        }
    }

    private fun <T> writeField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val fieldBytes = child.toByteArray()
        val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(fieldBytes.size.toLong(), compactLen)
        encoder.writeBytes(fieldBytes)
    }

    private fun <T> decodeField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        fieldName: String,
    ): T {
        val compactLen = decoder.compactLenActive()
        val fieldLength = decoder.readLength(compactLen)
        if (fieldLength < 0 || fieldLength > Int.MAX_VALUE) {
            throw IllegalArgumentException("$fieldName field too large: $fieldLength")
        }
        val fieldBytes = decoder.readBytes(fieldLength.toInt())
        val child = NoritoDecoder(fieldBytes, decoder.flags, decoder.flagsHint)
        val value = adapter.decode(child)
        if (child.remaining() != 0) {
            throw IllegalArgumentException("$fieldName trailing bytes: ${child.remaining()}")
        }
        return value
    }

    private fun algorithmTag(algorithm: String?): Int {
        if (algorithm.isNullOrBlank()) return 0
        if ("ed25519".equals(algorithm.trim(), ignoreCase = true)) return 0
        throw IllegalArgumentException("Unsupported wallet signature algorithm: $algorithm")
    }

    private fun algorithmName(tag: Int): String =
        if (tag == 0) "ed25519" else "unknown($tag)"
}
