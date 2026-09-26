package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import java.util.Collections
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

/**
 * Choice-free update of an existing public standalone ballot.
 *
 * The choice remains in finalized state. This type emits the direct registered native instruction,
 * rather than a custom JSON instruction. The amount is the new total bond, not an increment.
 */
class UpdatePlainConvictionInstruction(
    referendumId: String,
    ownerAccountId: String,
    amount: String,
    durationBlocks: String,
) : InstructionTemplate {
    val referendumId: String =
        GovernanceInstructionUtils.requireGovernanceSelectorV1(referendumId, "referendumId")
    val ownerAccountId: String = requireCanonicalI105Address(ownerAccountId, "ownerAccountId")
    val amount: String = requireCanonicalQuantity(amount)
    /** Canonical decimal spelling of the full unsigned 64-bit lock duration. */
    val durationBlocks: String = requireCanonicalDuration(durationBlocks)

    constructor(
        referendumId: String,
        ownerAccountId: String,
        amount: String,
        durationBlocks: Long,
    ) : this(
        referendumId,
        ownerAccountId,
        amount,
        durationBlocks.also { require(it >= 0) { "durationBlocks must be non-negative" } }.toString(),
    )

    constructor(
        referendumId: String,
        ownerAccountId: String,
        amount: BigInteger,
        durationBlocks: Long,
    ) : this(referendumId, ownerAccountId, amount.toString(), durationBlocks)

    constructor(
        referendumId: String,
        ownerAccountId: String,
        amount: KotodamaQuantity,
        durationBlocks: Long,
    ) : this(referendumId, ownerAccountId, amount.toString(), durationBlocks)

    override val kind: InstructionKind = InstructionKind.CUSTOM

    /** Exact native fields; no action alias or choice field is represented. */
    override val arguments: Map<String, String> = Collections.unmodifiableMap(linkedMapOf(
        "referendum_id" to this.referendumId,
        "owner" to this.ownerAccountId,
        "amount" to this.amount,
        "duration_blocks" to this.durationBlocks,
    ))

    override fun toInstructionBox(): InstructionBox =
        UpdatePlainConvictionWirePayloadEncoder.encode(this)

    override fun equals(other: Any?): Boolean =
        other is UpdatePlainConvictionInstruction && arguments == other.arguments

    override fun hashCode(): Int = arguments.hashCode()

    companion object {
        const val WIRE_NAME: String =
            "iroha.instruction.v1::governance::UpdatePlainConviction"
        internal const val SCHEMA_NAME: String =
            "iroha_data_model::isi::governance::UpdatePlainConviction"

        private val FIELDS = setOf("referendum_id", "owner", "amount", "duration_blocks")
        private val CANONICAL_U64 = Regex("(?:0|[1-9][0-9]*)")

        /** Rebuild only the four final V1 native fields. */
        @JvmStatic
        fun fromCanonicalFields(fields: Map<String, String>): UpdatePlainConvictionInstruction {
            require(fields.keys == FIELDS) {
                "UpdatePlainConviction requires exactly referendum_id, owner, amount, duration_blocks"
            }
            return UpdatePlainConvictionInstruction(
                fields.getValue("referendum_id"),
                fields.getValue("owner"),
                fields.getValue("amount"),
                fields.getValue("duration_blocks"),
            )
        }

        /** Decode one exact native frame, rendering the chain-neutral account on the selected chain. */
        @JvmStatic
        fun fromWirePayload(
            frame: ByteArray,
            chainDiscriminant: Int,
        ): UpdatePlainConvictionInstruction =
            UpdatePlainConvictionWirePayloadEncoder.decodePayload(frame, chainDiscriminant)

        private fun requireCanonicalQuantity(value: String): String =
            KotodamaQuantity.parseCanonical(value).toString()

        private fun requireCanonicalDuration(value: String): String {
            require(CANONICAL_U64.matches(value)) {
                "durationBlocks must be a canonical unsigned 64-bit decimal"
            }
            val parsed = try {
                java.lang.Long.parseUnsignedLong(value)
            } catch (error: NumberFormatException) {
                throw IllegalArgumentException(
                    "durationBlocks must fit unsigned 64 bits",
                    error,
                )
            }
            require(java.lang.Long.toUnsignedString(parsed) == value) {
                "durationBlocks must use canonical unsigned 64-bit spelling"
            }
            return value
        }
    }
}

/** Final V1 Norito owner for the direct [UpdatePlainConvictionInstruction] payload. */
object UpdatePlainConvictionWirePayloadEncoder {
    const val WIRE_NAME: String = UpdatePlainConvictionInstruction.WIRE_NAME
    private val STRING_ADAPTER = NoritoAdapters.stringAdapter()
    private val UINT64_ADAPTER = NoritoAdapters.uint(64)

    @JvmStatic
    fun encode(value: UpdatePlainConvictionInstruction): InstructionBox =
        InstructionBox.fromWirePayload(WIRE_NAME, encodePayload(value))

    @JvmStatic
    fun encodePayload(value: UpdatePlainConvictionInstruction): ByteArray =
        NoritoCodec.encode(
            value,
            UpdatePlainConvictionInstruction.SCHEMA_NAME,
            PayloadAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT),
        )

    /** Reject changed header flags, malformed fields, trailing bytes, or alternate encodings. */
    @JvmStatic
    fun decodePayload(
        frame: ByteArray,
        chainDiscriminant: Int,
    ): UpdatePlainConvictionInstruction {
        require(chainDiscriminant in 0..0xffff) { "chainDiscriminant must fit u16" }
        val value = NoritoCodec.decode(
            frame,
            PayloadAdapter(chainDiscriminant),
            UpdatePlainConvictionInstruction.SCHEMA_NAME,
        )
        require(frame.contentEquals(encodePayload(value))) {
            "UpdatePlainConviction frame is not the canonical V1 encoding"
        }
        return value
    }

    /** Enforce the direct instruction shape even for caller-supplied wire payloads. */
    @JvmStatic
    fun requireCanonicalIfKnown(wireName: String, frame: ByteArray) {
        if (wireName == WIRE_NAME) {
            decodePayload(frame, AccountAddress.DEFAULT_I105_DISCRIMINANT)
        }
    }

    private class PayloadAdapter(
        private val chainDiscriminant: Int,
    ) : TypeAdapter<UpdatePlainConvictionInstruction> {
        override fun encode(encoder: NoritoEncoder, value: UpdatePlainConvictionInstruction) {
            encodeSizedField(encoder, STRING_ADAPTER, value.referendumId)
            encodeSizedRawField(
                encoder,
                TransferWirePayloadEncoder.encodeAccountIdPayload(value.ownerAccountId),
            )
            encodeSizedRawField(
                encoder,
                TransferWirePayloadEncoder.encodeQuantityPayload(value.amount),
            )
            encodeSizedField(
                encoder,
                UINT64_ADAPTER,
                java.lang.Long.parseUnsignedLong(value.durationBlocks),
            )
        }

        override fun decode(decoder: NoritoDecoder): UpdatePlainConvictionInstruction {
            val referendumId = decodeSizedField(
                decoder,
                STRING_ADAPTER,
                "UpdatePlainConviction.referendum_id",
            )
            val owner = TransferWirePayloadEncoder.decodeAccountIdPayload(
                decodeSizedRawField(decoder, "UpdatePlainConviction.owner"),
                chainDiscriminant,
                decoder.flags,
            )
            val amount = TransferWirePayloadEncoder.decodeQuantityPayload(
                decodeSizedRawField(decoder, "UpdatePlainConviction.amount"),
                decoder.flags,
            )
            val duration = decodeSizedField(
                decoder,
                UINT64_ADAPTER,
                "UpdatePlainConviction.duration_blocks",
            )
            return UpdatePlainConvictionInstruction(
                referendumId,
                owner,
                amount,
                java.lang.Long.toUnsignedString(duration),
            )
        }
    }

    private fun <T> encodeSizedField(
        encoder: NoritoEncoder,
        adapter: TypeAdapter<T>,
        value: T,
    ) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        encodeSizedRawField(encoder, child.toByteArray())
    }

    private fun encodeSizedRawField(encoder: NoritoEncoder, payload: ByteArray) {
        encoder.writeLength(payload.size.toLong(), (encoder.flags and NoritoHeader.COMPACT_LEN) != 0)
        encoder.writeBytes(payload)
    }

    private fun <T> decodeSizedField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        fieldName: String,
    ): T {
        val child = NoritoDecoder(decodeSizedRawField(decoder, fieldName), decoder.flags)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after $fieldName" }
        return value
    }

    private fun decodeSizedRawField(decoder: NoritoDecoder, fieldName: String): ByteArray {
        val length = decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0)
        require(length in 0..Int.MAX_VALUE.toLong()) { "$fieldName exceeds supported size" }
        return decoder.readBytes(length.toInt())
    }
}
