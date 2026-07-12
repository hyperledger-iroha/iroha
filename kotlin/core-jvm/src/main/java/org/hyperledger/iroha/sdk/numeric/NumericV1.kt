package org.hyperledger.iroha.sdk.numeric

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.norito.CRC64

/** Stable categories reported by the strict Kotodama V1 numeric codec. */
enum class NumericV1ErrorCode {
    MANTISSA_OVERFLOW,
    NONCANONICAL_MANTISSA,
    INVALID_SCALE,
    NONCANONICAL_DECIMAL,
    NEGATIVE_QUANTITY,
    INVALID_TEXT,
    FRAME_TOO_SHORT,
    FRAME_TOO_LARGE,
    INVALID_HEADER,
    SCHEMA_MISMATCH,
    COMPRESSION_NOT_ALLOWED,
    LAYOUT_FLAGS_NOT_ALLOWED,
    LENGTH_MISMATCH,
    CHECKSUM_MISMATCH,
    TRUNCATED_ENVELOPE,
    UNKNOWN_TYPE,
    TYPE_NOT_ALLOWED,
    WRONG_TYPE,
    INVALID_ENVELOPE_VERSION,
    OVERSIZED_LENGTH,
    PAYLOAD_HASH_MISMATCH,
}

/** Strict Kotodama V1 numeric validation failure. */
class NumericV1Exception(
    val code: NumericV1ErrorCode,
    message: String,
) : IllegalArgumentException(message)

private val INT_MIN: BigInteger = BigInteger.ONE.shiftLeft(511).negate()
private val INT_MAX: BigInteger = BigInteger.ONE.shiftLeft(511).subtract(BigInteger.ONE)

private fun fail(code: NumericV1ErrorCode, message: String): Nothing =
    throw NumericV1Exception(code, message)

private fun checkedMantissa(value: BigInteger): BigInteger {
    if (value < INT_MIN || value > INT_MAX) {
        fail(NumericV1ErrorCode.MANTISSA_OVERFLOW, "mantissa is outside the signed 512-bit domain")
    }
    return value
}

/** Lossless Kotodama V1 signed integer. */
class KotodamaInt private constructor(val value: BigInteger) {
    override fun toString(): String = value.toString()

    override fun equals(other: Any?): Boolean = other is KotodamaInt && value == other.value

    override fun hashCode(): Int = value.hashCode()

    companion object {
        /** Construct from an arbitrary-precision integer after checking the V1 range. */
        @JvmStatic
        fun of(value: BigInteger): KotodamaInt = KotodamaInt(checkedMantissa(value))

        /** Parse a canonical base-10 integer string without using a lossy host number. */
        @JvmStatic
        fun parse(value: String): KotodamaInt {
            if (!CANONICAL_INTEGER.matches(value) || value == "-0") {
                fail(NumericV1ErrorCode.INVALID_TEXT, "int must use canonical base-10 syntax")
            }
            if (value.length > MAX_INT_TEXT_BYTES) {
                fail(NumericV1ErrorCode.MANTISSA_OVERFLOW, "integer text exceeds the signed 512-bit input bound")
            }
            return of(BigInteger(value))
        }
    }
}

/** Lossless exact Kotodama V1 decimal. */
class KotodamaDecimal private constructor(
    val mantissa: BigInteger,
    val scale: Int,
) {
    override fun toString(): String = scaledText(mantissa, scale)

    override fun equals(other: Any?): Boolean =
        other is KotodamaDecimal && mantissa == other.mantissa && scale == other.scale

    override fun hashCode(): Int = 31 * mantissa.hashCode() + scale

    companion object {
        /** Construct and canonicalize a mantissa/scale pair. */
        @JvmStatic
        fun of(mantissa: BigInteger, scale: Int): KotodamaDecimal {
            val normalized = normalizeScaled(mantissa, scale, false)
            return KotodamaDecimal(normalized.first, normalized.second)
        }

        /** Parse and canonicalize an exact decimal string. */
        @JvmStatic
        fun parse(value: String): KotodamaDecimal {
            val normalized = parseScaled(value, false)
            return KotodamaDecimal(normalized.first, normalized.second)
        }
    }
}

/** Lossless nominal non-negative Kotodama V1 asset quantity. */
class KotodamaQuantity private constructor(
    val mantissa: BigInteger,
    val scale: Int,
) {
    override fun toString(): String = scaledText(mantissa, scale)

    override fun equals(other: Any?): Boolean =
        other is KotodamaQuantity && mantissa == other.mantissa && scale == other.scale

    override fun hashCode(): Int = 31 * mantissa.hashCode() + scale

    companion object {
        /** Construct and canonicalize a non-negative mantissa/scale pair. */
        @JvmStatic
        fun of(mantissa: BigInteger, scale: Int): KotodamaQuantity {
            val normalized = normalizeScaled(mantissa, scale, true)
            return KotodamaQuantity(normalized.first, normalized.second)
        }

        /** Parse and canonicalize an exact non-negative quantity string. */
        @JvmStatic
        fun parse(value: String): KotodamaQuantity {
            val normalized = parseScaled(value, true)
            return KotodamaQuantity(normalized.first, normalized.second)
        }

        /** Parse an exact non-negative quantity only when [value] already uses canonical spelling. */
        @JvmStatic
        fun parseCanonical(value: String): KotodamaQuantity = parse(value).also {
            if (it.toString() != value) {
                fail(NumericV1ErrorCode.INVALID_TEXT, "quantity must use canonical spelling")
            }
        }
    }
}

private val CANONICAL_INTEGER = Regex("-?(?:0|[1-9][0-9]*)")
private val EXACT_DECIMAL = Regex("(-?)(0|[1-9][0-9]*)(?:\\.([0-9]+))?")

private fun parseScaled(value: String, quantity: Boolean): Pair<BigInteger, Int> {
    val match = EXACT_DECIMAL.matchEntire(value)
        ?: fail(NumericV1ErrorCode.INVALID_TEXT, "value must use exact decimal syntax")
    if (value == "-0") fail(NumericV1ErrorCode.INVALID_TEXT, "negative zero is not canonical")
    val fraction = match.groupValues[3]
    val rawDigits = match.groupValues[2] + fraction
    var first = 0
    while (first < rawDigits.length && rawDigits[first] == '0') first++
    if (first == rawDigits.length) return normalizeScaled(BigInteger.ZERO, 0, quantity)
    var end = rawDigits.length
    var scale = fraction.length
    while (scale > 0 && rawDigits[end - 1] == '0') {
        end--
        scale--
    }
    if (scale > 28) fail(NumericV1ErrorCode.INVALID_SCALE, "canonical scale exceeds 28")
    if (end - first > MAX_SIGNIFICANT_DIGITS) {
        fail(NumericV1ErrorCode.MANTISSA_OVERFLOW, "decimal mantissa exceeds the signed 512-bit input bound")
    }
    val magnitude = BigInteger(rawDigits.substring(first, end))
    val mantissa = if (match.groupValues[1] == "-") magnitude.negate() else magnitude
    return normalizeScaled(mantissa, scale, quantity)
}

private fun normalizeScaled(
    rawMantissa: BigInteger,
    rawScale: Int,
    quantity: Boolean,
): Pair<BigInteger, Int> {
    if (rawScale < 0) fail(NumericV1ErrorCode.INVALID_SCALE, "scale cannot be negative")
    var mantissa = rawMantissa
    var scale = rawScale
    if (mantissa.signum() == 0) {
        scale = 0
    } else {
        while (scale > 0 && mantissa.remainder(BigInteger.TEN).signum() == 0) {
            mantissa = mantissa.divide(BigInteger.TEN)
            scale--
        }
    }
    if (scale > 28) fail(NumericV1ErrorCode.INVALID_SCALE, "canonical scale exceeds 28")
    checkedMantissa(mantissa)
    if (quantity && mantissa.signum() < 0) {
        fail(NumericV1ErrorCode.NEGATIVE_QUANTITY, "quantity cannot be negative")
    }
    return mantissa to scale
}

private fun scaledText(mantissa: BigInteger, scale: Int): String {
    if (scale == 0) return mantissa.toString()
    val negative = mantissa.signum() < 0
    var digits = mantissa.abs().toString()
    if (digits.length <= scale) digits = "0".repeat(scale + 1 - digits.length) + digits
    val split = digits.length - scale
    return (if (negative) "-" else "") + digits.substring(0, split) + "." + digits.substring(split)
}

private enum class NumericKind(
    val schemaHash: ByteArray,
    val pointerType: Int,
    val scaled: Boolean,
) {
    INT("07c039457363b9e1d36bbd31d93dec4a".hexBytes(), 0x0011, false),
    DECIMAL("ba2ffed52e4d8ee16f17efefe1828524".hexBytes(), 0x0012, true),
    QUANTITY("e4769984c81ce0e8b678f2eb06274ee3".hexBytes(), 0x0013, true),
}

/** Canonical schema-bound frames and pointer envelopes for Kotodama V1 numerics. */
object NumericV1Codec {
    /** Minimum signed V1 integer. */
    @JvmField
    val intMin: BigInteger = INT_MIN

    /** Maximum signed V1 integer. */
    @JvmField
    val intMax: BigInteger = INT_MAX

    /** Encode an integer as its canonical lossless JSON string value. */
    @JvmStatic
    fun encodeIntJson(value: KotodamaInt): String = value.toString()

    /** Encode a decimal as its canonical lossless JSON string value. */
    @JvmStatic
    fun encodeDecimalJson(value: KotodamaDecimal): String = value.toString()

    /** Encode a quantity as its canonical lossless JSON string value. */
    @JvmStatic
    fun encodeQuantityJson(value: KotodamaQuantity): String = value.toString()

    /** Decode a canonical integer JSON string. */
    @JvmStatic
    fun decodeIntJson(value: String): KotodamaInt = KotodamaInt.parse(value)

    /** Decode a JSON scalar, requiring the lossless string representation mandated by V1. */
    @JvmStatic
    fun decodeIntJsonValue(value: Any?): KotodamaInt =
        if (value is String) decodeIntJson(value)
        else fail(NumericV1ErrorCode.INVALID_TEXT, "int JSON value must be a string")

    /** Decode a canonical decimal JSON string, rejecting alternate spellings. */
    @JvmStatic
    fun decodeDecimalJson(value: String): KotodamaDecimal = KotodamaDecimal.parse(value).also {
        if (it.toString() != value) fail(NumericV1ErrorCode.INVALID_TEXT, "decimal JSON must use canonical spelling")
    }

    /** Decode a JSON scalar, requiring the lossless string representation mandated by V1. */
    @JvmStatic
    fun decodeDecimalJsonValue(value: Any?): KotodamaDecimal =
        if (value is String) decodeDecimalJson(value)
        else fail(NumericV1ErrorCode.INVALID_TEXT, "decimal JSON value must be a string")

    /** Decode a canonical quantity JSON string, rejecting alternate spellings. */
    @JvmStatic
    fun decodeQuantityJson(value: String): KotodamaQuantity = KotodamaQuantity.parseCanonical(value)

    /** Decode a JSON scalar, requiring the lossless string representation mandated by V1. */
    @JvmStatic
    fun decodeQuantityJsonValue(value: Any?): KotodamaQuantity =
        if (value is String) decodeQuantityJson(value)
        else fail(NumericV1ErrorCode.INVALID_TEXT, "quantity JSON value must be a string")

    /** Encode an integer Norito frame. */
    @JvmStatic
    fun encodeIntFrame(value: KotodamaInt): ByteArray = encodeFrame(NumericKind.INT, value.value, 0)

    /** Encode an exact-decimal Norito frame. */
    @JvmStatic
    fun encodeDecimalFrame(value: KotodamaDecimal): ByteArray =
        encodeFrame(NumericKind.DECIMAL, value.mantissa, value.scale)

    /** Encode a quantity Norito frame. */
    @JvmStatic
    fun encodeQuantityFrame(value: KotodamaQuantity): ByteArray =
        encodeFrame(NumericKind.QUANTITY, value.mantissa, value.scale)

    /** Decode a strict integer Norito frame. */
    @JvmStatic
    fun decodeIntFrame(frame: ByteArray): KotodamaInt =
        KotodamaInt.of(decodeFrame(NumericKind.INT, frame).first)

    /** Decode a strict exact-decimal Norito frame. */
    @JvmStatic
    fun decodeDecimalFrame(frame: ByteArray): KotodamaDecimal {
        val value = decodeFrame(NumericKind.DECIMAL, frame)
        return KotodamaDecimal.of(value.first, value.second)
    }

    /** Decode a strict quantity Norito frame. */
    @JvmStatic
    fun decodeQuantityFrame(frame: ByteArray): KotodamaQuantity {
        val value = decodeFrame(NumericKind.QUANTITY, frame)
        return KotodamaQuantity.of(value.first, value.second)
    }

    /** Encode an integer pointer envelope. */
    @JvmStatic
    fun encodeIntEnvelope(value: KotodamaInt): ByteArray =
        encodeEnvelope(NumericKind.INT, encodeIntFrame(value))

    /** Encode an exact-decimal pointer envelope. */
    @JvmStatic
    fun encodeDecimalEnvelope(value: KotodamaDecimal): ByteArray =
        encodeEnvelope(NumericKind.DECIMAL, encodeDecimalFrame(value))

    /** Encode a quantity pointer envelope. */
    @JvmStatic
    fun encodeQuantityEnvelope(value: KotodamaQuantity): ByteArray =
        encodeEnvelope(NumericKind.QUANTITY, encodeQuantityFrame(value))

    /** Decode a strict integer pointer envelope. */
    @JvmStatic
    fun decodeIntEnvelope(envelope: ByteArray): KotodamaInt =
        decodeIntFrame(decodeEnvelope(NumericKind.INT, envelope))

    /** Decode a strict exact-decimal pointer envelope. */
    @JvmStatic
    fun decodeDecimalEnvelope(envelope: ByteArray): KotodamaDecimal =
        decodeDecimalFrame(decodeEnvelope(NumericKind.DECIMAL, envelope))

    /** Decode a strict quantity pointer envelope. */
    @JvmStatic
    fun decodeQuantityEnvelope(envelope: ByteArray): KotodamaQuantity =
        decodeQuantityFrame(decodeEnvelope(NumericKind.QUANTITY, envelope))

    private fun encodeFrame(kind: NumericKind, mantissa: BigInteger, scale: Int): ByteArray {
        val twos = encodeTwos(mantissa)
        val body = ByteBuffer.allocate(4 + twos.size + if (kind.scaled) 1 else 0)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(twos.size)
            .put(twos)
            .apply { if (kind.scaled) put(scale.toByte()) }
            .array()
        return ByteBuffer.allocate(FRAME_HEADER_BYTES + body.size)
            .order(ByteOrder.LITTLE_ENDIAN)
            .put(MAGIC)
            .put(0.toByte())
            .put(0.toByte())
            .put(kind.schemaHash)
            .put(0.toByte())
            .putLong(body.size.toLong())
            .putLong(CRC64.compute(body))
            .put(0.toByte())
            .put(body)
            .array()
    }

    private fun decodeFrame(kind: NumericKind, frame: ByteArray): Pair<BigInteger, Int> {
        val maximum = FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + if (kind.scaled) 1 else 0
        if (frame.size < FRAME_HEADER_BYTES) fail(NumericV1ErrorCode.FRAME_TOO_SHORT, "frame is truncated")
        if (frame.size > maximum) fail(NumericV1ErrorCode.FRAME_TOO_LARGE, "frame is oversized")
        if (!frame.copyOfRange(0, 4).contentEquals(MAGIC) || frame[4] != 0.toByte() || frame[5] != 0.toByte()) {
            fail(NumericV1ErrorCode.INVALID_HEADER, "frame has the wrong magic or version")
        }
        if (!frame.copyOfRange(6, 22).contentEquals(kind.schemaHash)) {
            fail(NumericV1ErrorCode.SCHEMA_MISMATCH, "frame schema does not match")
        }
        if (frame[22] != 0.toByte()) fail(NumericV1ErrorCode.COMPRESSION_NOT_ALLOWED, "compression is forbidden")
        if (frame[39] != 0.toByte()) fail(NumericV1ErrorCode.LAYOUT_FLAGS_NOT_ALLOWED, "layout flags must be zero")
        val header = ByteBuffer.wrap(frame).order(ByteOrder.LITTLE_ENDIAN)
        val bodyLength = header.getLong(23)
        if (bodyLength < 0 || bodyLength != (frame.size - FRAME_HEADER_BYTES).toLong()) {
            fail(NumericV1ErrorCode.LENGTH_MISMATCH, "frame length is inconsistent")
        }
        val body = frame.copyOfRange(FRAME_HEADER_BYTES, frame.size)
        if (header.getLong(31) != CRC64.compute(body)) {
            fail(NumericV1ErrorCode.CHECKSUM_MISMATCH, "frame checksum failed")
        }
        if (body.size < 4) fail(NumericV1ErrorCode.LENGTH_MISMATCH, "body has no mantissa length")
        val mantissaLength = ByteBuffer.wrap(body).order(ByteOrder.LITTLE_ENDIAN).int
        if (mantissaLength < 0 || mantissaLength > MAX_MANTISSA_BYTES) {
            fail(NumericV1ErrorCode.MANTISSA_OVERFLOW, "mantissa length exceeds 64 bytes")
        }
        val expected = 4 + mantissaLength + if (kind.scaled) 1 else 0
        if (expected != body.size) fail(NumericV1ErrorCode.LENGTH_MISMATCH, "body length is inconsistent")
        val mantissa = decodeTwos(body.copyOfRange(4, 4 + mantissaLength))
        if (!kind.scaled) return mantissa to 0
        val scale = body.last().toInt() and 0xFF
        if (scale > 28) fail(NumericV1ErrorCode.INVALID_SCALE, "scale exceeds 28")
        if ((mantissa.signum() == 0 && scale != 0)
            || (scale > 0 && mantissa.remainder(BigInteger.TEN).signum() == 0)
        ) {
            fail(NumericV1ErrorCode.NONCANONICAL_DECIMAL, "scaled value is not canonical")
        }
        if (kind == NumericKind.QUANTITY && mantissa.signum() < 0) {
            fail(NumericV1ErrorCode.NEGATIVE_QUANTITY, "quantity cannot be negative")
        }
        return mantissa to scale
    }

    private fun encodeEnvelope(kind: NumericKind, frame: ByteArray): ByteArray =
        ByteBuffer.allocate(ENVELOPE_HEADER_BYTES + frame.size + HASH_BYTES)
            .order(ByteOrder.BIG_ENDIAN)
            .putShort(kind.pointerType.toShort())
            .put(1.toByte())
            .putInt(frame.size)
            .put(frame)
            .put(payloadHash(frame))
            .array()

    private fun decodeEnvelope(kind: NumericKind, envelope: ByteArray): ByteArray {
        if (envelope.size < ENVELOPE_HEADER_BYTES) {
            fail(NumericV1ErrorCode.TRUNCATED_ENVELOPE, "envelope is truncated")
        }
        val header = ByteBuffer.wrap(envelope).order(ByteOrder.BIG_ENDIAN)
        val pointerType = header.short.toInt() and 0xFFFF
        if (pointerType == 0x0010) {
            fail(NumericV1ErrorCode.TYPE_NOT_ALLOWED, "retired Amount pointer type is permanently reserved")
        }
        val knownAllowedType = pointerType in 0x0001..0x000F || pointerType in 0x0011..0x0013
        if (!knownAllowedType) fail(NumericV1ErrorCode.UNKNOWN_TYPE, "unknown pointer type")
        if (pointerType != kind.pointerType) fail(NumericV1ErrorCode.WRONG_TYPE, "pointer type does not match")
        if ((header.get().toInt() and 0xFF) != 1) {
            fail(NumericV1ErrorCode.INVALID_ENVELOPE_VERSION, "envelope version must be 1")
        }
        val frameLength = header.int
        val maximum = FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + if (kind.scaled) 1 else 0
        if (frameLength < 0 || frameLength > maximum) {
            fail(NumericV1ErrorCode.OVERSIZED_LENGTH, "declared frame is oversized")
        }
        if (ENVELOPE_HEADER_BYTES + frameLength + HASH_BYTES != envelope.size) {
            fail(NumericV1ErrorCode.TRUNCATED_ENVELOPE, "envelope length is inconsistent")
        }
        val frame = envelope.copyOfRange(ENVELOPE_HEADER_BYTES, ENVELOPE_HEADER_BYTES + frameLength)
        val suppliedHash = envelope.copyOfRange(ENVELOPE_HEADER_BYTES + frameLength, envelope.size)
        if (!constantTimeEquals(payloadHash(frame), suppliedHash)) {
            fail(NumericV1ErrorCode.PAYLOAD_HASH_MISMATCH, "payload hash failed")
        }
        return frame
    }
}

private fun payloadHash(frame: ByteArray): ByteArray = Blake2b.digest256(frame).also {
    it[it.lastIndex] = (it.last().toInt() or 1).toByte()
}

private fun encodeTwos(value: BigInteger): ByteArray {
    checkedMantissa(value)
    if (value.signum() == 0) return ByteArray(0)
    val bytes = value.toByteArray().reversedArray()
    if (bytes.size > MAX_MANTISSA_BYTES) fail(NumericV1ErrorCode.MANTISSA_OVERFLOW, "mantissa is too wide")
    return bytes
}

private fun decodeTwos(bytes: ByteArray): BigInteger {
    if (bytes.size > MAX_MANTISSA_BYTES) fail(NumericV1ErrorCode.MANTISSA_OVERFLOW, "mantissa is too wide")
    if (bytes.isEmpty()) return BigInteger.ZERO
    val last = bytes.last().toInt() and 0xFF
    if (bytes.size == 1 && last == 0) {
        fail(NumericV1ErrorCode.NONCANONICAL_MANTISSA, "zero must use an empty mantissa")
    }
    if (bytes.size > 1) {
        val previous = bytes[bytes.lastIndex - 1].toInt() and 0xFF
        if ((last == 0 && previous and 0x80 == 0) || (last == 0xFF && previous and 0x80 != 0)) {
            fail(NumericV1ErrorCode.NONCANONICAL_MANTISSA, "mantissa has redundant sign extension")
        }
    }
    return checkedMantissa(BigInteger(bytes.reversedArray()))
}

private fun String.hexBytes(): ByteArray {
    val out = ByteArray(length / 2)
    for (index in out.indices) out[index] = substring(index * 2, index * 2 + 2).toInt(16).toByte()
    return out
}

private fun constantTimeEquals(left: ByteArray, right: ByteArray): Boolean {
    if (left.size != right.size) return false
    var difference = 0
    for (index in left.indices) difference = difference or (left[index].toInt() xor right[index].toInt())
    return difference == 0
}

private const val MAX_MANTISSA_BYTES = 64
private const val MAX_INT_TEXT_BYTES = 155
private const val MAX_SIGNIFICANT_DIGITS = 154
private const val FRAME_HEADER_BYTES = 40
private const val ENVELOPE_HEADER_BYTES = 7
private const val HASH_BYTES = 32
private val MAGIC = byteArrayOf(0x4E, 0x52, 0x54, 0x30)
