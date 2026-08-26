package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.escrow.EscrowId
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.Blake2b
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

private const val CANCEL_ASSET_LOCK_ESCROW_ID = "escrow_id"
private const val CANCEL_ASSET_LOCK_EXPECTED_REMAINING_AMOUNT = "expected_remaining_amount"

/**
 * Strict typed representation of the native V1 `CancelAssetLock` instruction.
 *
 * The public constructor accepts the clean lock identifier used by an application and derives the
 * native [EscrowId] with Blake2b-256. Use [fromEscrowId] only when rebuilding an already committed
 * canonical instruction.
 */
class CancelAssetLockInstruction private constructor(
    @JvmField val escrowId: EscrowId,
    @JvmField val expectedRemainingAmount: KotodamaQuantity,
) : InstructionTemplate {

    /**
     * Construct a compare-and-cancel instruction from an exact application lock identifier.
     *
     * The identifier must be well-formed UTF-16 so its UTF-8 hash preimage cannot depend on
     * replacement-character behavior.
     */
    constructor(
        lockId: String,
        expectedRemainingAmount: String,
    ) : this(
        escrowId = deriveEscrowId(lockId),
        expectedRemainingAmount = requirePositiveQuantity(expectedRemainingAmount),
    )

    /** Construct from a previously validated lossless quantity. */
    constructor(
        lockId: String,
        expectedRemainingAmount: KotodamaQuantity,
    ) : this(
        escrowId = deriveEscrowId(lockId),
        expectedRemainingAmount = requirePositiveQuantity(expectedRemainingAmount),
    )

    override val kind: InstructionKind = InstructionKind.CUSTOM

    override val arguments: Map<String, String> = Collections.unmodifiableMap(
        linkedMapOf(
            CANCEL_ASSET_LOCK_ESCROW_ID to escrowId.value,
            CANCEL_ASSET_LOCK_EXPECTED_REMAINING_AMOUNT to expectedRemainingAmount.toString(),
        ),
    )

    /**
     * Return a wire-framed instruction box.
     *
     * This deliberately bypasses the local custom argument-map representation: native
     * `CancelAssetLock` is submitted only under its registered Norito wire identifier.
     */
    override fun toInstructionBox(): InstructionBox =
        CancelAssetLockWirePayloadEncoder.encode(this)

    override fun equals(other: Any?): Boolean =
        other is CancelAssetLockInstruction &&
            escrowId == other.escrowId &&
            expectedRemainingAmount == other.expectedRemainingAmount

    override fun hashCode(): Int =
        31 * escrowId.hashCode() + expectedRemainingAmount.hashCode()

    companion object {
        /** Canonical native instruction wire identifier. */
        const val WIRE_NAME: String =
            "iroha.instruction.v1::escrow::CancelAssetLock"

        /** Concrete Norito schema path used only for the typed payload. */
        internal const val SCHEMA_NAME: String =
            "iroha_data_model::isi::escrow::CancelAssetLock"

        /** Maximum UTF-8 bytes accepted for the lock-id preimage in V1. */
        const val MAX_LOCK_ID_UTF8_BYTES_V1: Int = 4_096

        /**
         * Rebuild from the exact native JSON field surface.
         *
         * Missing fields, legacy aliases, and unknown fields are all rejected.
         */
        @JvmStatic
        fun fromCanonicalFields(fields: Map<String, String>): CancelAssetLockInstruction {
            require(fields.keys == CANONICAL_FIELDS) {
                "CancelAssetLock must contain exactly escrow_id and expected_remaining_amount"
            }
            return fromEscrowId(
                requireNotNull(fields[CANCEL_ASSET_LOCK_ESCROW_ID]),
                requireNotNull(fields[CANCEL_ASSET_LOCK_EXPECTED_REMAINING_AMOUNT]),
            )
        }

        /** Rebuild from an exact canonical native escrow hash literal. */
        @JvmStatic
        fun fromEscrowId(
            escrowId: String,
            expectedRemainingAmount: String,
        ): CancelAssetLockInstruction =
            CancelAssetLockInstruction(
                requireCanonicalEscrowId(escrowId),
                requirePositiveQuantity(expectedRemainingAmount),
            )

        /** Rebuild from an exact canonical native escrow id and lossless quantity. */
        @JvmStatic
        fun fromEscrowId(
            escrowId: EscrowId,
            expectedRemainingAmount: KotodamaQuantity,
        ): CancelAssetLockInstruction =
            CancelAssetLockInstruction(
                requireCanonicalEscrowId(escrowId.value),
                requirePositiveQuantity(expectedRemainingAmount),
            )

        /** Rebuild from a canonical escrow literal and lossless quantity. */
        @JvmStatic
        fun fromEscrowId(
            escrowId: String,
            expectedRemainingAmount: KotodamaQuantity,
        ): CancelAssetLockInstruction =
            CancelAssetLockInstruction(
                requireCanonicalEscrowId(escrowId),
                requirePositiveQuantity(expectedRemainingAmount),
            )

        /** Decode a canonical native Norito payload and reject legacy or trailing layouts. */
        @JvmStatic
        fun fromWirePayload(payload: ByteArray): CancelAssetLockInstruction =
            CancelAssetLockWirePayloadEncoder.decodePayload(payload)

        private val CANONICAL_FIELDS = setOf(
            CANCEL_ASSET_LOCK_ESCROW_ID,
            CANCEL_ASSET_LOCK_EXPECTED_REMAINING_AMOUNT,
        )
    }
}

/** Canonical native Norito encoder/decoder for [CancelAssetLockInstruction]. */
object CancelAssetLockWirePayloadEncoder {
    /** Canonical native instruction wire identifier. */
    const val WIRE_NAME: String = CancelAssetLockInstruction.WIRE_NAME

    private val uint32Adapter: TypeAdapter<Long> = NoritoAdapters.uint(32)

    /** Encode a typed cancellation as a wire-framed [InstructionBox]. */
    @JvmStatic
    fun encode(instruction: CancelAssetLockInstruction): InstructionBox =
        InstructionBox.fromWirePayload(WIRE_NAME, encodePayload(instruction))

    /** Encode only the canonical native `CancelAssetLock` Norito frame. */
    @JvmStatic
    fun encodePayload(instruction: CancelAssetLockInstruction): ByteArray =
        NoritoCodec.encode(instruction, CancelAssetLockInstruction.SCHEMA_NAME, PayloadAdapter)

    /** Decode a canonical native `CancelAssetLock` Norito frame. */
    @JvmStatic
    fun decodePayload(payload: ByteArray): CancelAssetLockInstruction =
        NoritoCodec.decode(payload, PayloadAdapter, CancelAssetLockInstruction.SCHEMA_NAME)

    private object PayloadAdapter : TypeAdapter<CancelAssetLockInstruction> {
        override fun encode(
            encoder: NoritoEncoder,
            value: CancelAssetLockInstruction,
        ) {
            encodeSizedRawField(encoder, canonicalEscrowBytes(value.escrowId))
            encodeSizedField(encoder, QuantityAdapter, value.expectedRemainingAmount)
        }

        override fun decode(decoder: NoritoDecoder): CancelAssetLockInstruction {
            val escrowBytes = decodeSizedRawField(decoder, "CancelAssetLock.escrow_id")
            require(escrowBytes.size == 32) {
                "CancelAssetLock.escrow_id must contain exactly 32 bytes"
            }
            require((escrowBytes.last().toInt() and 1) == 1) {
                "CancelAssetLock.escrow_id must use a native hash with its marker bit set"
            }
            val expected = decodeSizedField(
                decoder,
                QuantityAdapter,
                "CancelAssetLock.expected_remaining_amount",
            )
            return CancelAssetLockInstruction.fromEscrowId(
                HashLiteral.canonicalize(escrowBytes),
                expected,
            )
        }
    }

    private object QuantityAdapter : TypeAdapter<KotodamaQuantity> {
        override fun encode(
            encoder: NoritoEncoder,
            value: KotodamaQuantity,
        ) {
            val canonical = requirePositiveQuantity(value)
            val mantissaBytes = toTwosComplementLittleEndian(canonical.mantissa)
            val mantissa = encoder.childEncoder()
            mantissa.writeUInt(mantissaBytes.size.toLong(), 32)
            mantissa.writeBytes(mantissaBytes)
            encodeSizedRawField(encoder, mantissa.toByteArray())

            val scale = encoder.childEncoder()
            uint32Adapter.encode(scale, canonical.scale.toLong())
            encodeSizedRawField(encoder, scale.toByteArray())
        }

        override fun decode(decoder: NoritoDecoder): KotodamaQuantity {
            val mantissaPayload = decodeSizedRawField(decoder, "Quantity.mantissa")
            val mantissaDecoder = NoritoDecoder(
                mantissaPayload,
                decoder.flags,
            )
            val byteLength = checkedLength(
                mantissaDecoder.readUInt(32),
                "Quantity.mantissa byte length",
            )
            val encodedMantissa = mantissaDecoder.readBytes(byteLength)
            require(mantissaDecoder.remaining() == 0) {
                "Trailing bytes after Quantity.mantissa"
            }
            val mantissa = decodeTwosComplementLittleEndian(encodedMantissa)
            require(toTwosComplementLittleEndian(mantissa).contentEquals(encodedMantissa)) {
                "Quantity.mantissa is not canonical"
            }

            val scalePayload = decodeSizedRawField(decoder, "Quantity.scale")
            require(scalePayload.size == 4) { "Quantity.scale must contain exactly four bytes" }
            val scaleDecoder = NoritoDecoder(
                scalePayload,
                decoder.flags,
            )
            val scale = Math.toIntExact(uint32Adapter.decode(scaleDecoder))
            require(scaleDecoder.remaining() == 0) { "Trailing bytes after Quantity.scale" }

            val quantity = KotodamaQuantity.of(mantissa, scale)
            require(quantity.mantissa == mantissa && quantity.scale == scale) {
                "Quantity is not canonically encoded"
            }
            return requirePositiveQuantity(quantity)
        }
    }

    private fun canonicalEscrowBytes(escrowId: EscrowId): ByteArray =
        requireCanonicalEscrowId(escrowId.value).let { HashLiteral.decode(it.value) }

    private fun <T> encodeSizedField(
        encoder: NoritoEncoder,
        adapter: TypeAdapter<T>,
        value: T,
    ) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        encodeSizedRawField(encoder, child.toByteArray())
    }

    private fun encodeSizedRawField(
        encoder: NoritoEncoder,
        payload: ByteArray,
    ) {
        encoder.writeLength(
            payload.size.toLong(),
            (encoder.flags and NoritoHeader.COMPACT_LEN) != 0,
        )
        encoder.writeBytes(payload)
    }

    private fun <T> decodeSizedField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        fieldName: String,
    ): T {
        val payload = decodeSizedRawField(decoder, fieldName)
        val child = NoritoDecoder(payload, decoder.flags)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after $fieldName" }
        return value
    }

    private fun decodeSizedRawField(
        decoder: NoritoDecoder,
        fieldName: String,
    ): ByteArray {
        val length = checkedLength(
            decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
            "$fieldName length",
        )
        return decoder.readBytes(length)
    }

    private fun checkedLength(
        value: Long,
        fieldName: String,
    ): Int {
        require(value >= 0L && value <= Int.MAX_VALUE.toLong()) {
            "$fieldName is outside the supported range"
        }
        return value.toInt()
    }
}

private fun deriveEscrowId(lockId: String): EscrowId {
    require(lockId.isNotEmpty() && !lockId.all(::isAssetLockWhitespace)) {
        "lockId must be an exact non-empty string"
    }
    require(
        !isAssetLockWhitespace(lockId.first()) &&
            !isAssetLockWhitespace(lockId.last()),
    ) {
        "lockId must not contain surrounding whitespace"
    }
    requireWellFormedUtf16(lockId)
    val lockIdBytes = lockId.toByteArray(StandardCharsets.UTF_8)
    require(lockIdBytes.size <= CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1) {
        "lockId must be at most " +
            "${CancelAssetLockInstruction.MAX_LOCK_ID_UTF8_BYTES_V1} UTF-8 bytes"
    }
    return EscrowId(
        HashLiteral.canonicalize(
            Blake2b.digest256(lockIdBytes),
        ),
    )
}

private fun requireCanonicalEscrowId(value: String): EscrowId {
    require(CANONICAL_HASH_LITERAL.matches(value)) {
        "escrow_id must be a canonical uppercase native hash literal"
    }
    val bytes = HashLiteral.decode(value)
    require((bytes.last().toInt() and 1) == 1) {
        "escrow_id must use a native hash with its marker bit set"
    }
    require(HashLiteral.canonicalize(bytes) == value) {
        "escrow_id must be a canonical uppercase native hash literal"
    }
    return EscrowId(value)
}

private fun requirePositiveQuantity(value: String): KotodamaQuantity =
    try {
        requirePositiveQuantity(KotodamaQuantity.parseCanonical(value))
    } catch (error: IllegalArgumentException) {
        throw IllegalArgumentException(
            "expected_remaining_amount must be a positive canonical Quantity",
            error,
        )
    }

private fun requirePositiveQuantity(value: KotodamaQuantity): KotodamaQuantity {
    require(value.mantissa.signum() > 0) {
        "expected_remaining_amount must be greater than zero"
    }
    return value
}

private fun toTwosComplementLittleEndian(value: BigInteger): ByteArray {
    if (value.signum() == 0) return ByteArray(0)
    val bigEndian = value.toByteArray()
    return ByteArray(bigEndian.size) { index ->
        bigEndian[bigEndian.lastIndex - index]
    }
}

private fun decodeTwosComplementLittleEndian(value: ByteArray): BigInteger {
    if (value.isEmpty()) return BigInteger.ZERO
    return BigInteger(
        ByteArray(value.size) { index ->
            value[value.lastIndex - index]
        },
    )
}

private val CANONICAL_HASH_LITERAL =
    Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")

private fun isAssetLockWhitespace(value: Char): Boolean =
    value.isWhitespace() || value == '\uFEFF'

private fun requireWellFormedUtf16(value: String) {
    var index = 0
    while (index < value.length) {
        val current = value[index]
        when {
            Character.isHighSurrogate(current) -> {
                require(
                    index + 1 < value.length &&
                        Character.isLowSurrogate(value[index + 1]),
                ) {
                    "lockId must not contain unpaired UTF-16 surrogates"
                }
                index += 2
            }

            Character.isLowSurrogate(current) -> {
                throw IllegalArgumentException(
                    "lockId must not contain unpaired UTF-16 surrogates",
                )
            }

            else -> index++
        }
    }
}
