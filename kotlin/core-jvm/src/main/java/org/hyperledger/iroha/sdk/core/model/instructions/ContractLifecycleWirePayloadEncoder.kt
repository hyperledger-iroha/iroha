package org.hyperledger.iroha.sdk.core.model.instructions

import java.math.BigInteger
import java.util.Collections
import java.util.Locale
import java.util.Optional
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.requireCanonicalV1ContractAddress
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Canonical Norito encoders for the account-owner contract lifecycle instructions.
 *
 * Every mutation carries the exact non-zero lifecycle revision observed by the caller. Ownership
 * offers keep account and Parliament targets distinct so a textual value cannot silently change
 * the native enum variant.
 */
object ContractLifecycleWirePayloadEncoder {
    const val DEACTIVATE_INSTANCE_WIRE_NAME: String =
        "iroha.instruction.v1::smart_contract_code::DeactivateContractInstance"
    const val ACTIVATE_INSTANCE_WIRE_NAME: String =
        "iroha.instruction.v1::smart_contract_code::ActivateContractInstance"
    const val SET_PARLIAMENT_DELEGATION_WIRE_NAME: String =
        "iroha.instruction.v1::smart_contract_code::SetContractParliamentDelegation"
    const val OFFER_OWNERSHIP_WIRE_NAME: String =
        "iroha.instruction.v1::smart_contract_code::OfferContractOwnership"
    const val ACCEPT_OWNERSHIP_WIRE_NAME: String =
        "iroha.instruction.v1::smart_contract_code::AcceptContractOwnership"
    const val CANCEL_OWNERSHIP_OFFER_WIRE_NAME: String =
        "iroha.instruction.v1::smart_contract_code::CancelContractOwnershipOffer"

    /** Closed first-release lifecycle instruction catalog in registry order. */
    @JvmField
    val WIRE_NAMES: List<String> = Collections.unmodifiableList(
        listOf(
            DEACTIVATE_INSTANCE_WIRE_NAME,
            ACTIVATE_INSTANCE_WIRE_NAME,
            SET_PARLIAMENT_DELEGATION_WIRE_NAME,
            OFFER_OWNERSHIP_WIRE_NAME,
            ACCEPT_OWNERSHIP_WIRE_NAME,
            CANCEL_OWNERSHIP_OFFER_WIRE_NAME,
        ),
    )

    private const val DEACTIVATE_INSTANCE_SCHEMA =
        "iroha_data_model::isi::smart_contract_code::DeactivateContractInstance"
    private const val ACTIVATE_INSTANCE_SCHEMA =
        "iroha_data_model::isi::smart_contract_code::ActivateContractInstance"
    private const val SET_PARLIAMENT_DELEGATION_SCHEMA =
        "iroha_data_model::isi::smart_contract_code::SetContractParliamentDelegation"
    private const val OFFER_OWNERSHIP_SCHEMA =
        "iroha_data_model::isi::smart_contract_code::OfferContractOwnership"
    private const val ACCEPT_OWNERSHIP_SCHEMA =
        "iroha_data_model::isi::smart_contract_code::AcceptContractOwnership"
    private const val CANCEL_OWNERSHIP_OFFER_SCHEMA =
        "iroha_data_model::isi::smart_contract_code::CancelContractOwnershipOffer"
    private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val STRING: TypeAdapter<String> = NoritoAdapters.stringAdapter()
    private val BOOL: TypeAdapter<Boolean> = NoritoAdapters.boolAdapter()
    private val UINT32: TypeAdapter<Long> = NoritoAdapters.uint(32)
    private val OPTIONAL_STRING: TypeAdapter<Optional<String>> = NoritoAdapters.option(STRING)

    /** Encode `DeactivateContractInstance` with an exact revision guard and audit reason. */
    @JvmStatic
    @JvmOverloads
    fun encodeDeactivateContractInstance(
        contractAddress: String,
        expectedRevision: BigInteger,
        reason: String? = null,
    ): InstructionBox {
        val payload = DeactivationPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            Optional.ofNullable(reason),
        )
        return wire(
            DEACTIVATE_INSTANCE_WIRE_NAME,
            NoritoCodec.encode(payload, DEACTIVATE_INSTANCE_SCHEMA, DeactivationAdapter),
        )
    }

    /** Encode `ActivateContractInstance` with an exact revision guard and artifact hash. */
    @JvmStatic
    fun encodeActivateContractInstance(
        contractAddress: String,
        expectedRevision: BigInteger,
        codeHashHex: String,
    ): InstructionBox {
        val payload = ActivationPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            canonicalHash(codeHashHex),
        )
        return wire(
            ACTIVATE_INSTANCE_WIRE_NAME,
            NoritoCodec.encode(payload, ACTIVATE_INSTANCE_SCHEMA, ActivationAdapter),
        )
    }

    /** Encode `SetContractParliamentDelegation` with an exact revision guard. */
    @JvmStatic
    fun encodeSetContractParliamentDelegation(
        contractAddress: String,
        expectedRevision: BigInteger,
        delegated: Boolean,
    ): InstructionBox {
        val payload = DelegationPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            delegated,
        )
        return wire(
            SET_PARLIAMENT_DELEGATION_WIRE_NAME,
            NoritoCodec.encode(payload, SET_PARLIAMENT_DELEGATION_SCHEMA, DelegationAdapter),
        )
    }

    /** Encode an account-targeted `OfferContractOwnership`. */
    @JvmStatic
    fun encodeOfferContractOwnershipToAccount(
        contractAddress: String,
        expectedRevision: BigInteger,
        newOwnerAccountId: String,
    ): InstructionBox {
        val owner = OwnerValue.account(newOwnerAccountId)
        val payload = OfferPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            owner,
        )
        return wire(
            OFFER_OWNERSHIP_WIRE_NAME,
            NoritoCodec.encode(payload, OFFER_OWNERSHIP_SCHEMA, OfferAdapter(null)),
        )
    }

    /** Encode a Parliament-targeted `OfferContractOwnership`. */
    @JvmStatic
    fun encodeOfferContractOwnershipToParliament(
        contractAddress: String,
        expectedRevision: BigInteger,
    ): InstructionBox {
        val payload = OfferPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
            OwnerValue.parliament(),
        )
        return wire(
            OFFER_OWNERSHIP_WIRE_NAME,
            NoritoCodec.encode(payload, OFFER_OWNERSHIP_SCHEMA, OfferAdapter(null)),
        )
    }

    /** Encode `AcceptContractOwnership` with an exact revision guard. */
    @JvmStatic
    fun encodeAcceptContractOwnership(
        contractAddress: String,
        expectedRevision: BigInteger,
    ): InstructionBox = encodeRevisionGuard(
        ACCEPT_OWNERSHIP_WIRE_NAME,
        ACCEPT_OWNERSHIP_SCHEMA,
        contractAddress,
        expectedRevision,
    )

    /** Encode `CancelContractOwnershipOffer` with an exact revision guard. */
    @JvmStatic
    fun encodeCancelContractOwnershipOffer(
        contractAddress: String,
        expectedRevision: BigInteger,
    ): InstructionBox = encodeRevisionGuard(
        CANCEL_OWNERSHIP_OFFER_WIRE_NAME,
        CANCEL_OWNERSHIP_OFFER_SCHEMA,
        contractAddress,
        expectedRevision,
    )

    internal fun decodeSetContractParliamentDelegation(payload: ByteArray): DecodedDelegation {
        val decoded = NoritoCodec.decode(payload, DelegationAdapter, SET_PARLIAMENT_DELEGATION_SCHEMA)
        return DecodedDelegation(decoded.address, decoded.revision, decoded.delegated)
    }

    internal fun decodeDeactivateContractInstance(payload: ByteArray): DecodedDeactivation {
        val decoded = NoritoCodec.decode(payload, DeactivationAdapter, DEACTIVATE_INSTANCE_SCHEMA)
        return DecodedDeactivation(
            decoded.address,
            decoded.revision,
            decoded.reason.orElse(null),
        )
    }

    internal fun decodeActivateContractInstance(payload: ByteArray): DecodedActivation {
        val decoded = NoritoCodec.decode(payload, ActivationAdapter, ACTIVATE_INSTANCE_SCHEMA)
        return DecodedActivation(
            decoded.address,
            decoded.revision,
            encodeHashHex(decoded.codeHash),
        )
    }

    internal fun decodeOfferContractOwnership(
        payload: ByteArray,
        chainDiscriminant: Int,
    ): DecodedOwnershipOffer {
        val decoded = NoritoCodec.decode(
            payload,
            OfferAdapter(chainDiscriminant),
            OFFER_OWNERSHIP_SCHEMA,
        )
        return DecodedOwnershipOffer(
            decoded.address,
            decoded.revision,
            decoded.owner.accountId,
        )
    }

    internal fun decodeAcceptContractOwnership(payload: ByteArray): DecodedRevisionGuard =
        decodeRevisionGuard(payload, ACCEPT_OWNERSHIP_SCHEMA)

    internal fun decodeCancelContractOwnershipOffer(payload: ByteArray): DecodedRevisionGuard =
        decodeRevisionGuard(payload, CANCEL_OWNERSHIP_OFFER_SCHEMA)

    internal class DecodedDelegation(
        val contractAddress: String,
        val expectedRevision: BigInteger,
        val delegated: Boolean,
    )

    internal class DecodedDeactivation(
        val contractAddress: String,
        val expectedRevision: BigInteger,
        val reason: String?,
    )

    internal class DecodedActivation(
        val contractAddress: String,
        val expectedRevision: BigInteger,
        val codeHashHex: String,
    )

    internal class DecodedOwnershipOffer(
        val contractAddress: String,
        val expectedRevision: BigInteger,
        /** Null denotes the native `Parliament` owner variant. */
        val newOwnerAccountId: String?,
    )

    internal class DecodedRevisionGuard(
        val contractAddress: String,
        val expectedRevision: BigInteger,
    )

    private class RevisionGuardPayload(
        val address: String,
        val revision: BigInteger,
    )

    private class DeactivationPayload(
        val address: String,
        val revision: BigInteger,
        val reason: Optional<String>,
    )

    private class ActivationPayload(
        val address: String,
        val revision: BigInteger,
        val codeHash: ByteArray,
    )

    private class DelegationPayload(
        val address: String,
        val revision: BigInteger,
        val delegated: Boolean,
    )

    private class OfferPayload(
        val address: String,
        val revision: BigInteger,
        val owner: OwnerValue,
    )

    private class OwnerValue private constructor(val accountId: String?) {
        companion object {
            fun account(accountId: String): OwnerValue {
                TransferWirePayloadEncoder.encodeAccountIdPayload(accountId)
                return OwnerValue(accountId)
            }

            fun parliament(): OwnerValue = OwnerValue(null)
        }
    }

    private object U64Adapter : TypeAdapter<BigInteger> {
        override fun encode(encoder: NoritoEncoder, value: BigInteger) {
            encoder.writeUInt(requireU64(value).toLong(), 64)
        }

        override fun decode(decoder: NoritoDecoder): BigInteger {
            val value = decoder.readUInt(64)
            return if (value >= 0) {
                BigInteger.valueOf(value)
            } else {
                BigInteger.valueOf(value and Long.MAX_VALUE).setBit(63)
            }
        }
    }

    private object RevisionGuardAdapter : TypeAdapter<RevisionGuardPayload> {
        override fun encode(encoder: NoritoEncoder, value: RevisionGuardPayload) {
            encodeField(encoder, STRING, value.address)
            encodeField(encoder, U64Adapter, value.revision)
        }

        override fun decode(decoder: NoritoDecoder): RevisionGuardPayload =
            RevisionGuardPayload(
                canonicalAddress(decodeField(decoder, STRING, "contract_address")),
                positiveU64(decodeField(decoder, U64Adapter, "expected_revision")),
            )
    }

    private object HashAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            requireCanonicalHashBytes(value)
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray =
            decoder.readBytes(32).also(::requireCanonicalHashBytes)
    }

    private object DeactivationAdapter : TypeAdapter<DeactivationPayload> {
        override fun encode(encoder: NoritoEncoder, value: DeactivationPayload) {
            encodeField(encoder, STRING, value.address)
            encodeField(encoder, U64Adapter, value.revision)
            encodeField(encoder, OPTIONAL_STRING, value.reason)
        }

        override fun decode(decoder: NoritoDecoder): DeactivationPayload =
            DeactivationPayload(
                canonicalAddress(decodeField(decoder, STRING, "contract_address")),
                positiveU64(decodeField(decoder, U64Adapter, "expected_revision")),
                decodeField(decoder, OPTIONAL_STRING, "reason"),
            )
    }

    private object ActivationAdapter : TypeAdapter<ActivationPayload> {
        override fun encode(encoder: NoritoEncoder, value: ActivationPayload) {
            encodeField(encoder, STRING, value.address)
            encodeField(encoder, U64Adapter, value.revision)
            encodeField(encoder, HashAdapter, value.codeHash)
        }

        override fun decode(decoder: NoritoDecoder): ActivationPayload =
            ActivationPayload(
                canonicalAddress(decodeField(decoder, STRING, "contract_address")),
                positiveU64(decodeField(decoder, U64Adapter, "expected_revision")),
                decodeField(decoder, HashAdapter, "code_hash"),
            )
    }

    private object DelegationAdapter : TypeAdapter<DelegationPayload> {
        override fun encode(encoder: NoritoEncoder, value: DelegationPayload) {
            encodeField(encoder, STRING, value.address)
            encodeField(encoder, U64Adapter, value.revision)
            encodeField(encoder, BOOL, value.delegated)
        }

        override fun decode(decoder: NoritoDecoder): DelegationPayload =
            DelegationPayload(
                canonicalAddress(decodeField(decoder, STRING, "contract_address")),
                positiveU64(decodeField(decoder, U64Adapter, "expected_revision")),
                decodeField(decoder, BOOL, "delegated"),
            )
    }

    private class OfferAdapter(private val chainDiscriminant: Int?) : TypeAdapter<OfferPayload> {
        override fun encode(encoder: NoritoEncoder, value: OfferPayload) {
            encodeField(encoder, STRING, value.address)
            encodeField(encoder, U64Adapter, value.revision)
            encodeField(encoder, OwnerAdapter(chainDiscriminant), value.owner)
        }

        override fun decode(decoder: NoritoDecoder): OfferPayload =
            OfferPayload(
                canonicalAddress(decodeField(decoder, STRING, "contract_address")),
                positiveU64(decodeField(decoder, U64Adapter, "expected_revision")),
                decodeField(decoder, OwnerAdapter(chainDiscriminant), "new_owner"),
            )
    }

    private class OwnerAdapter(private val chainDiscriminant: Int?) : TypeAdapter<OwnerValue> {
        override fun encode(encoder: NoritoEncoder, value: OwnerValue) {
            val accountId = value.accountId
            UINT32.encode(encoder, if (accountId == null) 1L else 0L)
            if (accountId != null) {
                val payload = TransferWirePayloadEncoder.encodeAccountIdPayload(accountId)
                writeLength(encoder, payload.size)
                encoder.writeBytes(payload)
            }
        }

        override fun decode(decoder: NoritoDecoder): OwnerValue {
            val discriminant = UINT32.decode(decoder)
            return when (discriminant) {
                0L -> {
                    val payload = readSizedBytes(decoder, "new_owner account variant")
                    OwnerValue.account(
                        TransferWirePayloadEncoder.decodeAccountIdPayload(
                            payload,
                            requireNotNull(chainDiscriminant) {
                                "chainDiscriminant is required to decode an account owner"
                            },
                            decoder.flags,
                        ),
                    )
                }
                1L -> OwnerValue.parliament()
                else -> throw IllegalArgumentException(
                    "unsupported ContractLifecycleOwnerV1 discriminant: $discriminant",
                )
            }
        }
    }

    private fun encodeRevisionGuard(
        wireName: String,
        schema: String,
        contractAddress: String,
        expectedRevision: BigInteger,
    ): InstructionBox {
        val payload = RevisionGuardPayload(
            canonicalAddress(contractAddress),
            positiveU64(expectedRevision),
        )
        return wire(wireName, NoritoCodec.encode(payload, schema, RevisionGuardAdapter))
    }

    private fun decodeRevisionGuard(payload: ByteArray, schema: String): DecodedRevisionGuard {
        val decoded = NoritoCodec.decode(payload, RevisionGuardAdapter, schema)
        return DecodedRevisionGuard(decoded.address, decoded.revision)
    }

    private fun wire(wireName: String, payload: ByteArray): InstructionBox =
        InstructionBox.fromWirePayload(wireName, payload)

    private fun canonicalAddress(value: String): String =
        requireCanonicalV1ContractAddress(value)

    private fun canonicalHash(value: String): ByteArray {
        val normalized = value.lowercase(Locale.ROOT)
        require(value.isNotEmpty() && value == value.trim()) {
            "codeHashHex must be an exact non-empty string"
        }
        require(normalized.length == 64 && normalized.all { it in '0'..'9' || it in 'a'..'f' }) {
            "codeHashHex must contain exactly 64 hexadecimal characters"
        }
        return ByteArray(32) {
            normalized.substring(it * 2, it * 2 + 2).toInt(16).toByte()
        }.also(::requireCanonicalHashBytes)
    }

    private fun encodeHashHex(value: ByteArray): String {
        requireCanonicalHashBytes(value)
        val digits = "0123456789abcdef"
        return buildString(64) {
            value.forEach { byte ->
                val unsigned = byte.toInt() and 0xff
                append(digits[unsigned ushr 4])
                append(digits[unsigned and 0x0f])
            }
        }
    }

    private fun requireCanonicalHashBytes(value: ByteArray) {
        require(value.size == 32) { "code_hash must contain exactly 32 bytes" }
        require((value.last().toInt() and 1) == 1) {
            "code_hash must carry the canonical iroha_crypto::Hash marker bit"
        }
    }

    private fun requireU64(value: BigInteger): BigInteger {
        require(value.signum() >= 0 && value <= U64_MAX) { "expectedRevision must fit u64" }
        return value
    }

    private fun positiveU64(value: BigInteger): BigInteger {
        requireU64(value)
        require(value.signum() > 0) { "expectedRevision must be non-zero" }
        return value
    }

    private fun <T> encodeField(
        encoder: NoritoEncoder,
        adapter: TypeAdapter<T>,
        value: T,
    ) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        writeLength(encoder, payload.size)
        encoder.writeBytes(payload)
    }

    private fun <T> decodeField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        field: String,
    ): T {
        val payload = readSizedBytes(decoder, field)
        val child = NoritoDecoder(payload, decoder.flags)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "trailing bytes after $field" }
        return value
    }

    private fun readSizedBytes(decoder: NoritoDecoder, field: String): ByteArray {
        val length = decoder.readLength(decoder.compactLenActive())
        require(length <= Int.MAX_VALUE) { "$field exceeds the supported JVM size" }
        return decoder.readBytes(length.toInt())
    }

    private fun writeLength(encoder: NoritoEncoder, size: Int) {
        encoder.writeLength(
            size.toLong(),
            (encoder.flags and NoritoHeader.COMPACT_LEN) != 0,
        )
    }
}
