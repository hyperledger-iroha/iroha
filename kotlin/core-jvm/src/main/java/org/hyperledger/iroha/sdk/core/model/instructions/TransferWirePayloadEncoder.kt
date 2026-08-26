// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.address.compactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.decodeCompactPublicKeyPayload
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity
import java.math.BigInteger
import java.nio.charset.StandardCharsets

/**
 * Encodes asset transfer instructions in wire-framed Norito format.
 *
 * This encoder creates wire payloads for transfer instructions that can be properly decoded by
 * the Rust Iroha server. The wire format uses:
 *
 * - Wire name: "iroha.transfer"
 * - Payload: Norito-framed TransferBox enum with Asset variant
 *
 * The TransferBox::Asset variant contains:
 *
 * - source: AssetId (the asset to transfer from)
 * - object: Quantity (amount to transfer)
 * - destination: AccountId (recipient account)
 */
object TransferWirePayloadEncoder {

    /** Wire name for transfer instructions in Iroha. */
    const val WIRE_NAME: String = "iroha.transfer"

    /** Schema path for TransferBox payloads. Must match Rust type name exactly. */
    private const val SCHEMA_PATH = "iroha_data_model::isi::transfer::TransferBox"

    /** TransferBox enum discriminant for Asset variant. */
    private const val TRANSFER_BOX_ASSET_DISCRIMINANT = 2
    private const val MULTISIG_POLICY_VERSION = 1

    private val UINT8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
    private val UINT16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
    private val UINT32_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)
    private val BYTE_VECTOR_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()

    /**
     * Encodes an asset transfer instruction as a wire-framed InstructionBox.
     *
     * @param assetId The internal asset balance-bucket literal
     * (`<base58-asset-definition-id>#<i105-account-id>` with an optional
     * `#dataspace:<id>` suffix; canonical asset-definition ids are Base58)
     * @param amount The canonical quantity to transfer as a string (e.g., "10" or "10.5")
     * @param destinationAccountId The recipient's account ID
     * @return InstructionBox with wire payload ready for Norito encoding
     */
    @JvmStatic
    fun encodeAssetTransfer(assetId: String, amount: String, destinationAccountId: String): InstructionBox {
        val wirePayload = encodeTransferBox(assetId, amount, destinationAccountId)
        return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload)
    }

    /** Encodes an asset transfer from a lossless validated quantity value. */
    @JvmStatic
    fun encodeAssetTransfer(
        assetId: String,
        quantity: KotodamaQuantity,
        destinationAccountId: String,
    ): InstructionBox = encodeAssetTransfer(assetId, quantity.toString(), destinationAccountId)

    /**
     * Encodes an `AccountId` bare payload using the same layout expected by transaction
     * instruction fields and larger authenticated compact-Norito structures.
     */
    @JvmStatic
    fun encodeAccountIdPayload(accountId: String): ByteArray {
        val parsed = AccountId.parse(accountId)
        val encoder = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        AccountIdAdapter().encode(encoder, parsed)
        return encoder.toByteArray()
    }

    /** Encodes a canonical `AssetDefinitionId` as a bare Norito payload. */
    @JvmStatic
    fun encodeAssetDefinitionIdPayload(assetDefinitionId: String): ByteArray {
        val parsed = AssetDefinitionId.fromAddress(assetDefinitionId)
        val encoder = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        AssetDefinitionIdAdapter().encode(encoder, parsed)
        return encoder.toByteArray()
    }

    /** Encodes a canonical non-negative `Quantity` as a bare Norito payload. */
    @JvmStatic
    fun encodeQuantityPayload(quantity: String): ByteArray {
        val parsed = parseQuantityAmount(quantity)
        val encoder = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        QuantityAdapter().encode(encoder, parsed)
        return encoder.toByteArray()
    }

    /** Encodes a lossless validated `Quantity` as a bare Norito payload. */
    @JvmStatic
    fun encodeQuantityPayload(quantity: KotodamaQuantity): ByteArray =
        encodeQuantityPayload(quantity.toString())

    /** Decodes a bare `AccountId` payload produced by [encodeAccountIdPayload]. */
    @JvmStatic
    internal fun decodeAccountIdPayload(
        payload: ByteArray,
        chainDiscriminant: Int,
        flags: Int = NoritoCodec.DEFAULT_FLAGS,
        flagsHint: Int = NoritoHeader.MINOR_VERSION,
    ): String {
        requireChainDiscriminant(chainDiscriminant)
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val accountId = AccountIdAdapter().decode(decoder)
        require(decoder.remaining() == 0) { "Trailing bytes after AccountId payload" }
        return accountId.renderI105(chainDiscriminant)
    }

    /** Decodes a Norito-framed `TransferBox::Asset` payload. */
    @JvmStatic
    internal fun decodeAssetTransferPayload(
        wirePayload: ByteArray,
        chainDiscriminant: Int,
    ): DecodedAssetTransfer {
        requireChainDiscriminant(chainDiscriminant)
        val payload = NoritoCodec.decode(wirePayload, TransferAssetPayloadAdapter(), SCHEMA_PATH)
        return DecodedAssetTransfer(
            assetId = payload.source.render(chainDiscriminant),
            amount = payload.amount.render(),
            destinationAccountId = payload.destination.renderI105(chainDiscriminant),
        )
    }

    internal data class DecodedAssetTransfer(
        val assetId: String,
        val amount: String,
        val destinationAccountId: String,
    )

    /**
     * Encodes a fixed-size byte array as per-element length-prefixed bytes for `[u8; N]`.
     * Canonical SDK encoders use `COMPACT_LEN`, so each element length is a compact varint.
     */
    @JvmStatic
    fun encodeFixedByteArray(encoder: NoritoEncoder, bytes: ByteArray) {
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        for (b in bytes) {
            encoder.writeLength(1, compact)
            encoder.writeByte(b.toInt())
        }
    }

    private fun encodeTransferBox(assetIdStr: String, amount: String, destinationAccountIdStr: String): ByteArray {
        val quantity = parseQuantityAmount(amount)
        val assetId = AssetId.parse(assetIdStr)
        val destinationAccountId = AccountId.parse(destinationAccountIdStr)
        val payloadAdapter = TransferAssetPayloadAdapter()
        val payload = TransferAssetPayload(assetId, quantity, destinationAccountId)
        return NoritoCodec.encode(payload, SCHEMA_PATH, payloadAdapter)
    }

    private fun parseQuantityAmount(amount: String): QuantityValue {
        val quantity = KotodamaQuantity.parseCanonical(amount)
        return QuantityValue(quantity.mantissa, quantity.scale)
    }

    private class QuantityValue(val mantissa: BigInteger, val scale: Int) {
        init {
            val canonical = KotodamaQuantity.of(mantissa, scale)
            require(canonical.mantissa == mantissa && canonical.scale == scale) {
                "Asset transfer quantity is not canonically encoded"
            }
        }

        fun render(): String = KotodamaQuantity.of(mantissa, scale).toString()
    }

    private class TransferAssetPayload(val source: AssetId, val amount: QuantityValue, val destination: AccountId)

    private class AssetDefinitionId(definitionBytes: ByteArray) {
        private val _definitionBytes: ByteArray = definitionBytes.clone()
        init {
            require(_definitionBytes.size == 16) {
                "definitionBytes must be 16 bytes, got ${_definitionBytes.size}"
            }
        }
        fun definitionBytes(): ByteArray = _definitionBytes.clone()

        companion object {
            fun fromAddress(address: String): AssetDefinitionId =
                AssetDefinitionId(AssetDefinitionIdEncoder.parseAddressBytes(address))
        }
    }

    private class AccountController private constructor(
        publicKeyPayload: ByteArray?,
        private val multisigPolicy: MultisigPolicyPayload?,
    ) {
        private val publicKeyPayload: ByteArray? = publicKeyPayload?.copyOf()

        fun isSingle(): Boolean = publicKeyPayload != null
        fun publicKeyPayload(): ByteArray = publicKeyPayload!!.copyOf()
        fun multisigPolicy(): MultisigPolicyPayload = multisigPolicy!!
        fun renderI105(chainDiscriminant: Int): String {
            return if (isSingle()) {
                val decoded = decodeCompactPublicKeyPayload(publicKeyPayload())
                    ?: throw IllegalArgumentException("Invalid single-key AccountController payload")
                val algorithm = algorithmForCurveId(decoded.curveId)
                    ?: throw IllegalArgumentException(
                        "Unsupported curve id in AccountController payload: ${decoded.curveId}"
                    )
                try {
                    AccountAddress
                        .fromAccount(decoded.keyBytes, algorithm)
                        .toI105(chainDiscriminant)
                } catch (ex: AccountAddressException) {
                    throw IllegalArgumentException("Invalid single-key AccountController payload", ex)
                }
            } else {
                try {
                    AccountAddress
                        .fromMultisigPolicy(multisigPolicy())
                        .toI105(chainDiscriminant)
                } catch (ex: AccountAddressException) {
                    throw IllegalArgumentException("Invalid multisig AccountController payload", ex)
                }
            }
        }

        companion object {
            fun single(publicKeyPayload: ByteArray): AccountController =
                AccountController(publicKeyPayload, null)
            fun multisig(policy: MultisigPolicyPayload): AccountController =
                AccountController(null, policy)
        }
    }

    private class AccountId(val controller: AccountController) {
        fun renderI105(chainDiscriminant: Int): String =
            controller.renderI105(chainDiscriminant)

        companion object {
            fun parse(accountIdStr: String): AccountId {
                val address: AccountAddress
                try {
                    address = AccountAddress.parseEncodedIgnoringCurveSupport(accountIdStr, null)
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException(
                        "AssetId.account must use canonical I105 form",
                        e,
                    )
                }

                try {
                    val singleKey = address.singleKeyPayloadIgnoringCurveSupport()
                    if (singleKey != null) {
                        val publicKeyPayload = compactPublicKeyPayload(singleKey.curveId, singleKey.publicKey)
                        return AccountId(AccountController.single(publicKeyPayload))
                    }
                    val multisig = address.multisigPolicyPayloadIgnoringCurveSupport()
                    if (multisig != null) {
                        return AccountId(AccountController.multisig(multisig))
                    }
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException("Failed to extract controller from i105 address", e)
                }
                throw IllegalArgumentException("Address contains neither single-key nor multisig controller")
            }
        }
    }

    private class AssetId(
        val account: AccountId?,
        val definition: AssetDefinitionId,
        encodedAccountPayload: ByteArray?,
        scopePayload: ByteArray,
    ) {
        private val _encodedAccountPayload: ByteArray? = encodedAccountPayload?.clone()
        private val _scopePayload: ByteArray = scopePayload.clone()

        init {
            require(account != null || _encodedAccountPayload != null) {
                "AssetId requires either account or encodedAccountPayload"
            }
        }

        fun encodedAccountPayload(): ByteArray? = _encodedAccountPayload?.clone()
        fun scopePayload(): ByteArray = _scopePayload.clone()
        fun render(chainDiscriminant: Int): String {
            val accountId = account?.renderI105(chainDiscriminant)
                ?: decodeAccountIdPayload(
                    _encodedAccountPayload!!,
                    chainDiscriminant,
                    NoritoCodec.DEFAULT_FLAGS,
                    NoritoHeader.MINOR_VERSION,
                )
            val definitionAddress = AssetDefinitionIdEncoder.encodeFromBytes(definition.definitionBytes())
            val scope = decodeAssetBalanceScopePayload(_scopePayload)
            return if (scope.isGlobal) {
                "$definitionAddress#$accountId"
            } else {
                "$definitionAddress#$accountId#dataspace:${scope.dataspaceId}"
            }
        }

        companion object {
            fun parse(assetIdStr: String): AssetId {
                val parts = assetIdStr.split('#')
                require(parts.size == 2 || parts.size == 3) {
                    "AssetId must use internal '<base58-asset-definition-id>#<i105-account-id>' with optional '#dataspace:<id>' suffix; canonical asset-definition ids are Base58"
                }

                val assetDef = AssetDefinitionId.fromAddress(parts[0])
                val accountId = AccountId.parse(parts[1])
                val scopePayload = if (parts.size == 2) {
                    globalScopePayload()
                } else {
                    val match = Regex("^dataspace:(\\d+)$").matchEntire(parts[2])
                        ?: throw IllegalArgumentException(
                            "AssetId.scope must use 'dataspace:<id>' when present"
                        )
                    encodeAssetBalanceScopePayload(
                        AssetBalanceScopePayload.dataspace(match.groupValues[1].toLong())
                    )
                }
                return AssetId(accountId, assetDef, null, scopePayload)
            }
        }
    }

    private class TransferAssetPayloadAdapter : TypeAdapter<TransferAssetPayload> {
        private val assetIdAdapter: TypeAdapter<AssetId> = AssetIdAdapter()
        private val accountIdAdapter: TypeAdapter<AccountId> = AccountIdAdapter()

        override fun encode(encoder: NoritoEncoder, value: TransferAssetPayload) {
            UINT32_ADAPTER.encode(encoder, TRANSFER_BOX_ASSET_DISCRIMINANT.toLong())
            val child = encoder.childEncoder()
            encodeTransferStruct(child, value)
            val variantPayload = child.toByteArray()
            writePayloadLength(encoder, variantPayload.size)
            encoder.writeBytes(variantPayload)
        }

        private fun encodeTransferStruct(encoder: NoritoEncoder, value: TransferAssetPayload) {
            encodeFieldWithLength(encoder, assetIdAdapter, value.source)
            encodeFieldWithLength(encoder, QuantityAdapter(), value.amount)
            encodeFieldWithLength(encoder, accountIdAdapter, value.destination)
        }

        private fun <T> encodeFieldWithLength(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
            val child = encoder.childEncoder()
            adapter.encode(child, value)
            val payload = child.toByteArray()
            writePayloadLength(encoder, payload.size)
            encoder.writeBytes(payload)
        }

        override fun decode(decoder: NoritoDecoder): TransferAssetPayload =
            decodeTransferBox(decoder)

        private fun decodeTransferBox(decoder: NoritoDecoder): TransferAssetPayload {
            val discriminant = UINT32_ADAPTER.decode(decoder)
            require(discriminant == TRANSFER_BOX_ASSET_DISCRIMINANT.toLong()) {
                "Unsupported TransferBox discriminant: $discriminant"
            }
            val payloadLength = checkedLength(
                decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
                "TransferBox::Asset payload",
            )
            val payload = decoder.readBytes(payloadLength)
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val transfer = decodeTransferStruct(child)
            require(child.remaining() == 0) { "Trailing bytes after TransferBox::Asset payload" }
            return transfer
        }

        private fun decodeTransferStruct(decoder: NoritoDecoder): TransferAssetPayload {
            val source = decodeSizedTypedField(decoder, assetIdAdapter, "source")
            val amount = decodeSizedTypedField(decoder, QuantityAdapter(), "amount")
            val destination = decodeSizedTypedField(decoder, accountIdAdapter, "destination")
            return TransferAssetPayload(source, amount, destination)
        }
    }

    private class AssetDefinitionIdAdapter : TypeAdapter<AssetDefinitionId> {
        override fun encode(encoder: NoritoEncoder, value: AssetDefinitionId) {
            encodeFixedByteArray(encoder, value.definitionBytes())
        }

        override fun decode(decoder: NoritoDecoder): AssetDefinitionId =
            AssetDefinitionId(decodeFixedByteArray(decoder, 16, "AssetDefinitionId"))
    }

    private class AccountIdAdapter : TypeAdapter<AccountId> {
        private val controllerAdapter: TypeAdapter<AccountController> = AccountControllerAdapter()

        override fun encode(encoder: NoritoEncoder, value: AccountId) {
            controllerAdapter.encode(encoder, value.controller)
        }

        override fun decode(decoder: NoritoDecoder): AccountId =
            AccountId(controllerAdapter.decode(decoder))
    }

    private class AccountControllerAdapter : TypeAdapter<AccountController> {
        override fun encode(encoder: NoritoEncoder, value: AccountController) {
            if (value.isSingle()) {
                encodeSingle(encoder, value.publicKeyPayload())
            } else {
                encodeMultisig(encoder, value.multisigPolicy())
            }
        }

        private fun encodeSingle(encoder: NoritoEncoder, publicKeyPayload: ByteArray) {
            UINT32_ADAPTER.encode(encoder, 0L)
            val child = encoder.childEncoder()
            BYTE_VECTOR_ADAPTER.encode(child, publicKeyPayload)
            val payload = child.toByteArray()
            writePayloadLength(encoder, payload.size)
            encoder.writeBytes(payload)
        }

        private fun encodeMultisig(encoder: NoritoEncoder, policy: MultisigPolicyPayload) {
            validateMultisigPolicySemantics(policy.version, policy.threshold, policy.members)
            UINT32_ADAPTER.encode(encoder, 1L)

            val policyEncoder = encoder.childEncoder()
            encodeSizedField(policyEncoder, UINT8_ADAPTER, policy.version.toLong())
            encodeSizedField(policyEncoder, UINT16_ADAPTER, policy.threshold.toLong())
            encodeMultisigMembers(policyEncoder, policy.members)

            val policyPayload = policyEncoder.toByteArray()
            writePayloadLength(encoder, policyPayload.size)
            encoder.writeBytes(policyPayload)
        }

        private fun encodeMultisigMembers(encoder: NoritoEncoder, members: List<MultisigMemberPayload>) {
            val sorted = members.sortedWith(Comparator { a, b -> compareUnsigned(canonicalSortKey(a), canonicalSortKey(b)) })
            for (i in 1 until sorted.size) {
                require(!canonicalSortKey(sorted[i - 1]).contentEquals(canonicalSortKey(sorted[i]))) {
                    "Duplicate multisig member"
                }
            }

            val vecEncoder = encoder.childEncoder()
            vecEncoder.writeUInt(sorted.size.toLong(), 64)
            for (member in sorted) {
                val memberEncoder = vecEncoder.childEncoder()
                val publicKeyPayload = compactPublicKeyPayload(member.curveId, member.publicKey)
                encodeSizedField(memberEncoder, BYTE_VECTOR_ADAPTER, publicKeyPayload)
                encodeSizedField(memberEncoder, UINT16_ADAPTER, member.weight.toLong())
                val memberPayload = memberEncoder.toByteArray()
                writePayloadLength(vecEncoder, memberPayload.size)
                vecEncoder.writeBytes(memberPayload)
            }
            val vecPayload = vecEncoder.toByteArray()
            writePayloadLength(encoder, vecPayload.size)
            encoder.writeBytes(vecPayload)
        }

        override fun decode(decoder: NoritoDecoder): AccountController =
            decodeController(decoder)

        private fun decodeController(decoder: NoritoDecoder): AccountController {
            val tag = UINT32_ADAPTER.decode(decoder)
            return when (tag) {
                0L -> {
                    val publicKeyPayload = decodeSizedTypedField(decoder, BYTE_VECTOR_ADAPTER, "single-key controller")
                    AccountController.single(publicKeyPayload)
                }
                1L -> AccountController.multisig(decodeSizedMultisigPolicy(decoder))
                else -> throw IllegalArgumentException("Unsupported AccountController discriminant: $tag")
            }
        }

        private fun decodeSizedMultisigPolicy(decoder: NoritoDecoder): MultisigPolicyPayload {
            val payloadLength = checkedLength(
                decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
                "multisig policy payload",
            )
            val payload = decoder.readBytes(payloadLength)
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val version = Math.toIntExact(decodeSizedTypedField(child, UINT8_ADAPTER, "multisig version"))
            val threshold = Math.toIntExact(decodeSizedTypedField(child, UINT16_ADAPTER, "multisig threshold"))
            val members = decodeSizedMultisigMembers(child)
            require(child.remaining() == 0) { "Trailing bytes after multisig policy payload" }
            validateMultisigPolicySemantics(version, threshold, members)
            return MultisigPolicyPayload.of(version, threshold, members)
        }

        private fun decodeSizedMultisigMembers(decoder: NoritoDecoder): List<MultisigMemberPayload> {
            val payloadLength = checkedLength(
                decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
                "multisig members payload",
            )
            val payload = decoder.readBytes(payloadLength)
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val count = checkedLength(child.readUInt(64), "multisig member count")
            val members = ArrayList<MultisigMemberPayload>(count)
            for (index in 0 until count) {
                val memberLength = checkedLength(
                    child.readLength((child.flags and NoritoHeader.COMPACT_LEN) != 0),
                    "multisig member $index payload",
                )
                val memberPayload = child.readBytes(memberLength)
                val memberDecoder = NoritoDecoder(memberPayload, child.flags, child.flagsHint)
                val publicKeyPayload = decodeSizedTypedField(
                    memberDecoder,
                    BYTE_VECTOR_ADAPTER,
                    "multisig member $index public key",
                )
                val weight = Math.toIntExact(
                    decodeSizedTypedField(memberDecoder, UINT16_ADAPTER, "multisig member $index weight"),
                )
                require(memberDecoder.remaining() == 0) { "Trailing bytes after multisig member $index payload" }
                val decodedKey = decodeCompactPublicKeyPayload(publicKeyPayload)
                    ?: throw IllegalArgumentException("Invalid multisig member $index public key payload")
                members.add(MultisigMemberPayload(decodedKey.curveId, weight, decodedKey.keyBytes))
            }
            require(child.remaining() == 0) { "Trailing bytes after multisig members payload" }
            return members
        }
    }

    private class AssetIdAdapter : TypeAdapter<AssetId> {
        private val accountIdAdapter: TypeAdapter<AccountId> = AccountIdAdapter()
        private val assetDefIdAdapter: TypeAdapter<AssetDefinitionId> = AssetDefinitionIdAdapter()

        override fun encode(encoder: NoritoEncoder, value: AssetId) {
            val encodedAccountPayload = value.encodedAccountPayload()
            if (encodedAccountPayload != null) {
                writePayloadLength(encoder, encodedAccountPayload.size)
                encoder.writeBytes(encodedAccountPayload)
            } else {
                encodeFieldWithLength(encoder, accountIdAdapter, value.account!!)
            }
            encodeFieldWithLength(encoder, assetDefIdAdapter, value.definition)
            val scopePayload = value.scopePayload()
            writePayloadLength(encoder, scopePayload.size)
            encoder.writeBytes(scopePayload)
        }

        override fun decode(decoder: NoritoDecoder): AssetId =
            AssetId(
                account = decodeSizedTypedField(decoder, accountIdAdapter, "AssetId.account"),
                definition = decodeSizedTypedField(decoder, assetDefIdAdapter, "AssetId.definition"),
                encodedAccountPayload = null,
                scopePayload = decodeSizedRawField(decoder, "AssetId.scope"),
            )

        private fun <T> encodeFieldWithLength(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
            val child = encoder.childEncoder()
            adapter.encode(child, value)
            val payload = child.toByteArray()
            writePayloadLength(encoder, payload.size)
            encoder.writeBytes(payload)
        }
    }

    private class QuantityAdapter : TypeAdapter<QuantityValue> {
        override fun encode(encoder: NoritoEncoder, value: QuantityValue) {
            encodeFieldBigInt(encoder, value.mantissa)
            encodeFieldU32(encoder, value.scale)
        }

        override fun decode(decoder: NoritoDecoder): QuantityValue =
            decodeQuantity(decoder)

        private fun decodeQuantity(decoder: NoritoDecoder): QuantityValue {
            val mantissa = decodeFieldBigInt(decoder)
            val scale = Math.toIntExact(decodeFieldU32(decoder))
            require(scale in 0..28) { "Quantity scale exceeds Iroha limit of 28: $scale" }
            return QuantityValue(mantissa, scale)
        }

        private fun encodeFieldBigInt(encoder: NoritoEncoder, value: BigInteger) {
            val child = encoder.childEncoder()
            encodeBigInt(child, value)
            val payload = child.toByteArray()
            writePayloadLength(encoder, payload.size)
            encoder.writeBytes(payload)
        }

        private fun encodeFieldU32(encoder: NoritoEncoder, value: Int) {
            writePayloadLength(encoder, 4)
            UINT32_ADAPTER.encode(encoder, value.toLong())
        }

        private fun encodeBigInt(encoder: NoritoEncoder, value: BigInteger) {
            val twosCompBytes = toTwosComplementLittleEndian(value)
            encoder.writeUInt(twosCompBytes.size.toLong(), 32)
            encoder.writeBytes(twosCompBytes)
        }

        private fun decodeFieldBigInt(decoder: NoritoDecoder): BigInteger {
            val payload = decodeSizedRawField(decoder, "quantity mantissa")
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val byteLength = checkedLength(child.readUInt(32), "quantity mantissa byte length")
            val twosComplement = child.readBytes(byteLength)
            require(child.remaining() == 0) { "Trailing bytes after quantity mantissa payload" }
            val value = decodeBigInt(twosComplement)
            require(toTwosComplementLittleEndian(value).contentEquals(twosComplement)) {
                "Quantity mantissa is not canonical"
            }
            require(value.bitLength() < 512) {
                "Quantity mantissa exceeds Iroha limit of 512 bits: ${value.bitLength()}"
            }
            return value
        }

        private fun decodeFieldU32(decoder: NoritoDecoder): Long {
            val payload = decodeSizedRawField(decoder, "quantity scale")
            require(payload.size == 4) { "quantity scale payload must be 4 bytes" }
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = UINT32_ADAPTER.decode(child)
            require(child.remaining() == 0) { "Trailing bytes after quantity scale payload" }
            return value
        }

        private fun decodeBigInt(payload: ByteArray): BigInteger {
            if (payload.isEmpty()) return BigInteger.ZERO
            val bigEndian = ByteArray(payload.size)
            for (index in payload.indices) {
                bigEndian[index] = payload[payload.size - 1 - index]
            }
            return BigInteger(bigEndian)
        }

        private fun toTwosComplementLittleEndian(value: BigInteger): ByteArray {
            if (value.signum() == 0) return ByteArray(0)
            val twosCompBE = value.toByteArray()
            val result = ByteArray(twosCompBE.size)
            for (i in twosCompBE.indices) {
                result[i] = twosCompBE[twosCompBE.size - 1 - i]
            }
            var trimLen = result.size
            if (value.signum() > 0) {
                while (trimLen > 1 && result[trimLen - 1] == 0.toByte() && (result[trimLen - 2].toInt() and 0x80) == 0) {
                    trimLen--
                }
            } else {
                while (trimLen > 1 && result[trimLen - 1] == 0xFF.toByte() && (result[trimLen - 2].toInt() and 0x80) != 0) {
                    trimLen--
                }
            }
            return if (trimLen == result.size) result else result.copyOf(trimLen)
        }
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        writePayloadLength(encoder, payload.size)
        encoder.writeBytes(payload)
    }

    private fun writePayloadLength(encoder: NoritoEncoder, size: Int) {
        encoder.writeLength(size.toLong(), (encoder.flags and NoritoHeader.COMPACT_LEN) != 0)
    }

    private fun decodeSizedRawField(decoder: NoritoDecoder, fieldName: String): ByteArray {
        val payloadLength = checkedLength(
            decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
            "$fieldName payload",
        )
        return decoder.readBytes(payloadLength)
    }

    private fun globalScopePayload(): ByteArray {
        val encoder = NoritoEncoder(0)
        UINT32_ADAPTER.encode(encoder, 0L)
        return encoder.toByteArray()
    }

    private fun validateMultisigPolicySemantics(version: Int, threshold: Int, members: List<MultisigMemberPayload>) {
        require(version == MULTISIG_POLICY_VERSION) { "Invalid multisig policy: unsupported version $version" }
        require(members.isNotEmpty()) { "Invalid multisig policy: zero members" }
        var totalWeight = 0L
        val sortKeys = mutableListOf<ByteArray>()
        for (member in members) {
            require(member.weight > 0) { "Invalid multisig policy: non-positive weight" }
            require(member.publicKey.isNotEmpty()) { "Invalid multisig policy: empty public key" }
            totalWeight += member.weight
            sortKeys.add(canonicalSortKey(member))
        }
        require(threshold > 0) { "Invalid multisig policy: zero threshold" }
        require(totalWeight >= threshold) { "Invalid multisig policy: threshold exceeds total weight" }
        sortKeys.sortWith(::compareUnsigned)
        for (i in 1 until sortKeys.size) {
            require(!sortKeys[i - 1].contentEquals(sortKeys[i])) { "Invalid multisig policy: duplicate member" }
        }
    }

    private fun canonicalSortKey(member: MultisigMemberPayload): ByteArray {
        val algorithm = algorithmForCurveId(member.curveId)
            ?: throw IllegalArgumentException("Invalid multisig policy: unknown curve id")
        val algorithmBytes = algorithm.toByteArray(StandardCharsets.UTF_8)
        val keyBytes = member.publicKey
        val sortKey = ByteArray(algorithmBytes.size + 1 + keyBytes.size)
        System.arraycopy(algorithmBytes, 0, sortKey, 0, algorithmBytes.size)
        sortKey[algorithmBytes.size] = 0
        System.arraycopy(keyBytes, 0, sortKey, algorithmBytes.size + 1, keyBytes.size)
        return sortKey
    }

    private fun compareUnsigned(a: ByteArray, b: ByteArray): Int {
        val len = minOf(a.size, b.size)
        for (i in 0 until len) {
            val cmp = (a[i].toInt() and 0xFF) - (b[i].toInt() and 0xFF)
            if (cmp != 0) return cmp
        }
        return a.size.compareTo(b.size)
    }

    private fun encodeAssetBalanceScopePayload(scope: AssetBalanceScopePayload): ByteArray {
        if (scope.isGlobal) return globalScopePayload()
        val encoder = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        UINT32_ADAPTER.encode(encoder, 1L)
        val child = encoder.childEncoder()
        writePayloadLength(child, 8)
        child.writeUInt(scope.dataspaceId, 64)
        val payload = child.toByteArray()
        writePayloadLength(encoder, payload.size)
        encoder.writeBytes(payload)
        return encoder.toByteArray()
    }

    private fun decodeAssetBalanceScopePayload(payload: ByteArray): AssetBalanceScopePayload {
        val decoder = NoritoDecoder(payload, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val tag = UINT32_ADAPTER.decode(decoder)
        return when (tag) {
            0L -> {
                require(decoder.remaining() == 0) { "Trailing bytes after global asset scope" }
                AssetBalanceScopePayload.global()
            }
            1L -> {
                val field = decodeSizedRawField(decoder, "dataspace asset scope")
                val child = NoritoDecoder(field, decoder.flags, decoder.flagsHint)
                val idPayload = decodeSizedRawField(child, "dataspace asset scope id")
                require(idPayload.size == 8) { "dataspace asset scope must contain a u64" }
                val idDecoder = NoritoDecoder(idPayload, child.flags, child.flagsHint)
                val dataspaceId = idDecoder.readUInt(64)
                require(idDecoder.remaining() == 0) { "Trailing bytes after dataspace asset scope id payload" }
                require(child.remaining() == 0) { "Trailing bytes after dataspace asset scope id" }
                require(decoder.remaining() == 0) { "Trailing bytes after dataspace asset scope" }
                AssetBalanceScopePayload.dataspace(dataspaceId)
            }
            else -> throw IllegalArgumentException("Unsupported AssetBalanceScope discriminant: $tag")
        }
    }

    private fun <T> decodeSizedTypedField(decoder: NoritoDecoder, adapter: TypeAdapter<T>, fieldName: String): T {
        val payloadLength = checkedLength(
            decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0), "$fieldName payload")
        val payload = decoder.readBytes(payloadLength)
        val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after $fieldName payload" }
        return value
    }

    private fun checkedLength(length: Long, fieldName: String): Int {
        require(length >= 0L) { "$fieldName must be non-negative" }
        require(length <= Int.MAX_VALUE) { "$fieldName too large" }
        return length.toInt()
    }

    private fun requireChainDiscriminant(value: Int): Int {
        require(value in 0..0xffff) { "chainDiscriminant must fit in u16" }
        return value
    }

    private fun decodeFixedByteArray(decoder: NoritoDecoder, length: Int, fieldName: String): ByteArray {
        val out = ByteArray(length)
        val compact = (decoder.flags and NoritoHeader.COMPACT_LEN) != 0
        for (index in 0 until length) {
            val elementLength = decoder.readLength(compact)
            require(elementLength == 1L) { "$fieldName element $index must contain exactly one byte" }
            out[index] = decoder.readByte().toByte()
        }
        return out
    }

    private class AssetBalanceScopePayload private constructor(val isGlobal: Boolean, val dataspaceId: Long) {
        companion object {
            fun global(): AssetBalanceScopePayload = AssetBalanceScopePayload(true, 0L)
            fun dataspace(dataspaceId: Long): AssetBalanceScopePayload = AssetBalanceScopePayload(false, dataspaceId)
        }
    }
}
