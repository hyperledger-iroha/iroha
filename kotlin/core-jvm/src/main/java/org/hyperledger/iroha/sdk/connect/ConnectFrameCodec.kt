package org.hyperledger.iroha.sdk.connect

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Optional
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

private const val CONNECT_LAYOUT_FLAGS = NoritoHeader.MINOR_VERSION

private const val FRAME_KIND_CONTROL = 0
private const val FRAME_KIND_CIPHERTEXT = 1

private const val CONTROL_OPEN = 0
private const val CONTROL_APPROVE = 1
private const val CONTROL_REJECT = 2
private const val CONTROL_CLOSE = 3

private val UINT16 = NoritoAdapters.uint(16)
private val UINT32 = NoritoAdapters.uint(32)
private val UINT64 = NoritoAdapters.uint(64)
private val STRING = NoritoAdapters.stringAdapter()
private val BOOL = NoritoAdapters.boolAdapter()

/**
 * Rust-side `[u8; 32]` values are encoded in canonical AoS layout (32 elements, each prefixed with
 * its element length). This adapter mirrors that exact representation.
 */
private val FIXED_ARRAY_U8_32: TypeAdapter<ByteArray> =
    object : TypeAdapter<ByteArray> {
        private val LENGTH = 32

        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            require(value.size == LENGTH) {
                "expected $LENGTH bytes, found ${value.size}"
            }
            if ((encoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                var offset = 0L
                for (i in 0 until LENGTH) {
                    offset += 1L
                    encoder.writeUInt(offset, 64)
                }
                encoder.writeBytes(value)
                return
            }
            val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            for (b in value) {
                encoder.writeLength(1L, compactLen)
                encoder.writeByte(b.toInt() and 0xFF)
            }
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val out = ByteArray(LENGTH)
            if ((decoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                var previous = 0L
                for (i in 0 until LENGTH) {
                    val current = decoder.readUInt(64)
                    val delta = current - previous
                    if (delta != 1L) {
                        throw IllegalArgumentException("Invalid packed [u8;32] offset delta: $delta")
                    }
                    previous = current
                }
                for (i in 0 until LENGTH) {
                    out[i] = decoder.readByte().toByte()
                }
                return out
            }
            val compactLen = decoder.compactLenActive()
            for (i in 0 until LENGTH) {
                val elemLen = decoder.readLength(compactLen)
                if (elemLen != 1L) {
                    throw IllegalArgumentException("Invalid [u8;32] element length: $elemLen")
                }
                out[i] = decoder.readByte().toByte()
            }
            return out
        }

        override fun isSelfDelimiting(): Boolean = true
    }

private val RAW_BYTES: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()
private val BYTE_VECTOR: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
private val OPTIONAL_PLACEHOLDER: TypeAdapter<Optional<ByteArray>> = NoritoAdapters.option(RAW_BYTES)

private class WalletSignature(val algorithm: Int, signature: ByteArray) {
    val signature: ByteArray = signature.copyOf()
}

private class Constraints(val chainId: String)

private val DIRECTION_ADAPTER: TypeAdapter<ConnectDirection> =
    object : TypeAdapter<ConnectDirection> {
        override fun encode(encoder: NoritoEncoder, value: ConnectDirection) {
            val tag = if (value == ConnectDirection.WALLET_TO_APP) 1L else 0L
            UINT32.encode(encoder, tag)
        }

        override fun decode(decoder: NoritoDecoder): ConnectDirection {
            val tag = UINT32.decode(decoder).toInt()
            return ConnectDirection.fromTag(tag)
        }
    }

private val ROLE_ADAPTER: TypeAdapter<ConnectRole> =
    object : TypeAdapter<ConnectRole> {
        override fun encode(encoder: NoritoEncoder, value: ConnectRole) {
            val tag = if (value == ConnectRole.WALLET) 1L else 0L
            UINT32.encode(encoder, tag)
        }

        override fun decode(decoder: NoritoDecoder): ConnectRole {
            val tag = UINT32.decode(decoder).toInt()
            if (tag == 0) return ConnectRole.APP
            if (tag == 1) return ConnectRole.WALLET
            throw IllegalArgumentException("Unknown Connect role tag: $tag")
        }
    }

private val CONSTRAINTS_ADAPTER: TypeAdapter<Constraints> =
    object : TypeAdapter<Constraints> {
        override fun encode(encoder: NoritoEncoder, value: Constraints) {
            STRING.encode(encoder, value.chainId)
        }

        override fun decode(decoder: NoritoDecoder): Constraints =
            Constraints(STRING.decode(decoder))
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

private val CIPHERTEXT_ADAPTER: TypeAdapter<ConnectCiphertext> =
    object : TypeAdapter<ConnectCiphertext> {
        override fun encode(encoder: NoritoEncoder, value: ConnectCiphertext) {
            val directionField: ByteArray
            run {
                val child = encoder.childEncoder()
                DIRECTION_ADAPTER.encode(child, value.direction)
                directionField = child.toByteArray()
            }
            val aeadField: ByteArray
            run {
                val child = encoder.childEncoder()
                RAW_BYTES.encode(child, value.aead())
                aeadField = child.toByteArray()
            }
            val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            encoder.writeLength(directionField.size.toLong(), compactLen)
            encoder.writeBytes(directionField)
            encoder.writeLength(aeadField.size.toLong(), compactLen)
            encoder.writeBytes(aeadField)
        }

        override fun decode(decoder: NoritoDecoder): ConnectCiphertext {
            val compactLen = decoder.compactLenActive()

            val directionLength = decoder.readLength(compactLen)
            val directionBytes = decoder.readBytes(directionLength.toInt())
            val directionDecoder = NoritoDecoder(directionBytes, decoder.flags, decoder.flagsHint)
            val direction = DIRECTION_ADAPTER.decode(directionDecoder)
            if (directionDecoder.remaining() != 0) {
                throw IllegalArgumentException(
                    "ciphertext.direction trailing bytes: ${directionDecoder.remaining()}",
                )
            }

            val aeadLength = decoder.readLength(compactLen)
            val aeadBytes = decoder.readBytes(aeadLength.toInt())
            val aeadDecoder = NoritoDecoder(aeadBytes, decoder.flags, decoder.flagsHint)
            val aead = RAW_BYTES.decode(aeadDecoder)
            if (aeadDecoder.remaining() != 0) {
                throw IllegalArgumentException(
                    "ciphertext.aead trailing bytes: ${aeadDecoder.remaining()}",
                )
            }
            return ConnectCiphertext(direction, aead)
        }
    }

/** Connect wire codec for frame/control payloads used by wallet-role flows. */
object ConnectFrameCodec {

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun decode(rawFrame: ByteArray): DecodedFrame {
        val frameCursor = Cursor(rawFrame)

        val sid = decodeLengthPrefixedField(frameCursor, FIXED_ARRAY_U8_32, "sid")
        val direction = decodeLengthPrefixedField(frameCursor, DIRECTION_ADAPTER, "direction")
        val sequence = decodeLengthPrefixedField(frameCursor, UINT64, "sequence")

        val kindLength = frameCursor.readU64()
        val kindBytes = frameCursor.readBytes(asInt(kindLength, "kind length"))
        frameCursor.ensureFullyConsumed("connect frame")

        val kindCursor = Cursor(kindBytes)
        val kindTag = kindCursor.readU32()
        val kindPayloadLength = kindCursor.readU64()
        val kindPayload = kindCursor.readBytes(asInt(kindPayloadLength, "kind payload length"))
        kindCursor.ensureFullyConsumed("connect frame kind")

        if (kindTag == FRAME_KIND_CIPHERTEXT) {
            val ciphertext = decodeField(kindPayload, CIPHERTEXT_ADAPTER, "ciphertext")
            return DecodedFrame(sid, direction, sequence, FrameType.CIPHERTEXT, null, null, null, ciphertext)
        }

        if (kindTag != FRAME_KIND_CONTROL) {
            throw ConnectProtocolException("Unsupported connect frame kind tag: $kindTag")
        }

        val controlCursor = Cursor(kindPayload)
        val controlTag = controlCursor.readU32()
        val controlBodyLength = controlCursor.readU64()
        val controlBody = controlCursor.readBytes(asInt(controlBodyLength, "control payload length"))
        controlCursor.ensureFullyConsumed("connect control payload")

        return when (controlTag) {
            CONTROL_OPEN -> decodeOpenFrame(sid, direction, sequence, controlBody)
            CONTROL_REJECT -> decodeRejectFrame(sid, direction, sequence, controlBody)
            CONTROL_CLOSE -> decodeCloseFrame(sid, direction, sequence, controlBody)
            else -> DecodedFrame(sid, direction, sequence, FrameType.OTHER_CONTROL, null, null, null, null)
        }
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeApproveFrame(
        sessionId: ByteArray,
        sequence: Long,
        walletPublicKey: ByteArray,
        accountId: String,
        walletSignature: ByteArray,
    ): ByteArray {
        if (accountId.isBlank()) {
            throw ConnectProtocolException("accountId must not be blank")
        }
        val walletPkField = encodeField(walletPublicKey, FIXED_ARRAY_U8_32, "wallet_pk")
        val accountField = encodeField(accountId, STRING, "account_id")
        val permissionsField = encodeField(Optional.empty(), OPTIONAL_PLACEHOLDER, "permissions")
        val proofField = encodeField(Optional.empty(), OPTIONAL_PLACEHOLDER, "proof")
        val signature = WalletSignature(0, walletSignature)
        val signatureField = encodeField(signature, WALLET_SIGNATURE_ADAPTER, "wallet_signature")

        val body = ByteArrayOutputStream()
        writeLengthPrefixed(body, walletPkField)
        writeLengthPrefixed(body, accountField)
        writeLengthPrefixed(body, permissionsField)
        writeLengthPrefixed(body, proofField)
        writeLengthPrefixed(body, signatureField)

        val controlPayload = wrapTaggedPayload(CONTROL_APPROVE, body.toByteArray())
        val kindPayload = wrapTaggedPayload(FRAME_KIND_CONTROL, controlPayload)
        return encodeFrame(sessionId, ConnectDirection.WALLET_TO_APP, sequence, kindPayload)
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeRejectFrame(
        sessionId: ByteArray,
        sequence: Long,
        code: Int,
        codeId: String,
        reason: String,
    ): ByteArray {
        val codeField = encodeField(code.toLong(), UINT16, "reject_code")
        val codeIdField = encodeField(codeId, STRING, "reject_code_id")
        val reasonField = encodeField(reason, STRING, "reject_reason")

        val body = ByteArrayOutputStream()
        writeLengthPrefixed(body, codeField)
        writeLengthPrefixed(body, codeIdField)
        writeLengthPrefixed(body, reasonField)

        val controlPayload = wrapTaggedPayload(CONTROL_REJECT, body.toByteArray())
        val kindPayload = wrapTaggedPayload(FRAME_KIND_CONTROL, controlPayload)
        return encodeFrame(sessionId, ConnectDirection.WALLET_TO_APP, sequence, kindPayload)
    }

    @JvmStatic
    @Throws(ConnectProtocolException::class)
    fun encodeCiphertextFrame(
        sessionId: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
        aead: ByteArray,
    ): ByteArray {
        val ciphertext = ConnectCiphertext(direction, aead)
        val cipherPayload = encodeField(ciphertext, CIPHERTEXT_ADAPTER, "ciphertext")
        val kindPayload = wrapTaggedPayload(FRAME_KIND_CIPHERTEXT, cipherPayload)
        return encodeFrame(sessionId, direction, sequence, kindPayload)
    }

    private fun decodeOpenFrame(
        sid: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
        controlBody: ByteArray,
    ): DecodedFrame {
        val cursor = Cursor(controlBody)
        val appPk = decodeLengthPrefixedField(cursor, FIXED_ARRAY_U8_32, "open.app_pk")
        skipLengthPrefixedField(cursor, "open.app_meta")
        val constraints = decodeLengthPrefixedField(cursor, CONSTRAINTS_ADAPTER, "open.constraints")
        skipLengthPrefixedField(cursor, "open.permissions")
        cursor.ensureFullyConsumed("open control")

        val open = OpenControl(appPk, constraints.chainId)
        return DecodedFrame(sid, direction, sequence, FrameType.OPEN, open, null, null, null)
    }

    private fun decodeRejectFrame(
        sid: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
        controlBody: ByteArray,
    ): DecodedFrame {
        val cursor = Cursor(controlBody)
        val code = decodeLengthPrefixedField(cursor, UINT16, "reject.code").toInt()
        val codeId = decodeLengthPrefixedField(cursor, STRING, "reject.code_id")
        val reason = decodeLengthPrefixedField(cursor, STRING, "reject.reason")
        cursor.ensureFullyConsumed("reject control")

        val reject = RejectControl(code, codeId, reason)
        return DecodedFrame(sid, direction, sequence, FrameType.REJECT, null, reject, null, null)
    }

    private fun decodeCloseFrame(
        sid: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
        controlBody: ByteArray,
    ): DecodedFrame {
        val cursor = Cursor(controlBody)
        val role = decodeLengthPrefixedField(cursor, ROLE_ADAPTER, "close.role")
        val code = decodeLengthPrefixedField(cursor, UINT16, "close.code").toInt()
        val reason = decodeLengthPrefixedField(cursor, STRING, "close.reason")
        val retryable = decodeLengthPrefixedField(cursor, BOOL, "close.retryable")
        cursor.ensureFullyConsumed("close control")

        val close = CloseControl(role, code, reason, retryable)
        return DecodedFrame(sid, direction, sequence, FrameType.CLOSE, null, null, close, null)
    }

    private fun encodeFrame(
        sessionId: ByteArray,
        direction: ConnectDirection,
        sequence: Long,
        kindPayload: ByteArray,
    ): ByteArray {
        val sidField = encodeField(sessionId, FIXED_ARRAY_U8_32, "sid")
        val directionField = encodeField(direction, DIRECTION_ADAPTER, "direction")
        val sequenceField = encodeField(sequence, UINT64, "sequence")

        val frame = ByteArrayOutputStream()
        writeLengthPrefixed(frame, sidField)
        writeLengthPrefixed(frame, directionField)
        writeLengthPrefixed(frame, sequenceField)
        writeLengthPrefixed(frame, kindPayload)
        return frame.toByteArray()
    }

    private fun wrapTaggedPayload(tag: Int, payload: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, tag)
        writeU64(out, payload.size.toLong())
        out.write(payload, 0, payload.size)
        return out.toByteArray()
    }

    private fun <T> encodeField(
        value: T,
        adapter: TypeAdapter<T>,
        label: String,
    ): ByteArray {
        try {
            val encoding = NoritoCodec.encodeWithHeaderFlags(value, adapter)
            if (encoding.flags != CONNECT_LAYOUT_FLAGS) {
                throw ConnectProtocolException("Unsupported Norito flags in $label: ${encoding.flags}")
            }
            return encoding.payload()
        } catch (ex: RuntimeException) {
            throw ConnectProtocolException("Failed to encode $label", ex)
        }
    }

    private fun <T> decodeField(
        fieldBytes: ByteArray,
        adapter: TypeAdapter<T>,
        label: String,
    ): T {
        NoritoCodec.DecodeFlagsGuard.enterWithHint(CONNECT_LAYOUT_FLAGS, CONNECT_LAYOUT_FLAGS).use {
            try {
                val decoder = NoritoDecoder(fieldBytes, CONNECT_LAYOUT_FLAGS, CONNECT_LAYOUT_FLAGS)
                val value = adapter.decode(decoder)
                if (decoder.remaining() != 0) {
                    throw ConnectProtocolException(
                        "$label did not consume full payload (remaining=${decoder.remaining()})",
                    )
                }
                return value
            } catch (ex: ConnectProtocolException) {
                throw ex
            } catch (ex: RuntimeException) {
                throw ConnectProtocolException("Failed to decode $label", ex)
            }
        }
    }

    private fun <T> decodeLengthPrefixedField(
        cursor: Cursor,
        adapter: TypeAdapter<T>,
        label: String,
    ): T {
        val length = cursor.readU64()
        val field = cursor.readBytes(asInt(length, "$label length"))
        return decodeField(field, adapter, label)
    }

    private fun skipLengthPrefixedField(cursor: Cursor, label: String) {
        val length = cursor.readU64()
        cursor.readBytes(asInt(length, "$label length"))
    }

    private fun asInt(value: Long, label: String): Int {
        if (value < 0 || value > Int.MAX_VALUE) {
            throw ConnectProtocolException("Invalid $label: $value")
        }
        return value.toInt()
    }

    private fun writeLengthPrefixed(out: ByteArrayOutputStream, bytes: ByteArray) {
        writeU64(out, bytes.size.toLong())
        out.write(bytes, 0, bytes.size)
    }

    private fun writeU32(out: ByteArrayOutputStream, value: Int) {
        val buffer = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN)
        buffer.putInt(value)
        out.write(buffer.array(), 0, 4)
    }

    private fun writeU64(out: ByteArrayOutputStream, value: Long) {
        val buffer = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN)
        buffer.putLong(value)
        out.write(buffer.array(), 0, 8)
    }

    private class Cursor(private val data: ByteArray) {
        private var offset = 0

        fun readU32(): Int {
            val bytes = readBytes(4)
            return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).int
        }

        fun readU64(): Long {
            val bytes = readBytes(8)
            return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).long
        }

        fun readBytes(length: Int): ByteArray {
            if (length < 0) {
                throw ConnectProtocolException("Negative read length: $length")
            }
            val end = offset + length
            if (end < offset || end > data.size) {
                throw ConnectProtocolException(
                    "Connect payload truncated (offset=$offset, length=$length, total=${data.size})",
                )
            }
            val out = ByteArray(length)
            System.arraycopy(data, offset, out, 0, length)
            offset = end
            return out
        }

        fun ensureFullyConsumed(label: String) {
            if (offset != data.size) {
                throw ConnectProtocolException(
                    "$label has trailing bytes (used=$offset, total=${data.size})",
                )
            }
        }
    }
}
