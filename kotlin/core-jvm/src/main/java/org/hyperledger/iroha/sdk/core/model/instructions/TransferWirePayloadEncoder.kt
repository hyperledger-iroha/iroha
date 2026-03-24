// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetIdDecoder
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.algorithmForCurveId
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import java.math.BigDecimal
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
 * - object: Numeric (amount to transfer)
 * - destination: AccountId (recipient account)
 */
object TransferWirePayloadEncoder {

    /** Wire name for transfer instructions in Iroha. */
    const val WIRE_NAME: String = "iroha.transfer"

    /** Schema path for TransferBox payloads. Must match Rust type name exactly. */
    private const val SCHEMA_PATH = "iroha_data_model::isi::transfer::TransferBox"

    /** TransferBox enum discriminant for Asset variant. */
    private const val TRANSFER_BOX_ASSET_DISCRIMINANT = 2
    private const val MULTISIG_POLICY_VERSION_V1 = 1

    private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
    private val UINT8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
    private val UINT16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
    private val UINT32_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)

    /**
     * Encodes an asset transfer instruction as a wire-framed InstructionBox.
     *
     * @param assetId The full asset ID (e.g., "rose#wonderland##alice@wonderland")
     * @param amount The amount to transfer as a string (e.g., "10" or "10.50")
     * @param destinationAccountId The recipient's account ID
     * @return InstructionBox with wire payload ready for Norito encoding
     */
    @JvmStatic
    fun encodeAssetTransfer(assetId: String, amount: String, destinationAccountId: String): InstructionBox {
        val wirePayload = encodeTransferBox(assetId, amount, destinationAccountId)
        return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload)
    }

    /**
     * Encodes an `AccountId` bare payload using the same layout expected by transaction
     * instruction fields.
     */
    @JvmStatic
    internal fun encodeAccountIdPayload(accountId: String): ByteArray {
        val parsed = AccountId.parse(accountId)
        val encoder = NoritoEncoder(0)
        AccountIdAdapter().encode(encoder, parsed)
        return encoder.toByteArray()
    }

    /**
     * Encodes a fixed-size byte array as per-element length-prefixed bytes for `[u8; N]`.
     * Each element is written as `u64_le(1) + byte`, producing 9 bytes per element.
     *
     * This matches Rust's `[T; N]::NoritoSerialize` only when `COMPACT_LEN` is off
     * (`flags=0`). With `COMPACT_LEN` active, Rust uses varint lengths instead of
     * fixed u64. The current signing path always uses `flags=0`.
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
        val numeric = parseNumericAmount(amount)
        val assetId = AssetId.parse(assetIdStr)
        val destinationAccountId = AccountId.parse(destinationAccountIdStr)
        val payloadAdapter = TransferAssetPayloadAdapter()
        val payload = TransferAssetPayload(assetId, numeric, destinationAccountId)
        return NoritoCodec.encode(payload, SCHEMA_PATH, payloadAdapter)
    }

    private fun parseNumericAmount(amount: String): NumericValue {
        val decimal = BigDecimal(amount)
        val scale = maxOf(0, decimal.scale())
        require(scale <= 28) { "Numeric scale exceeds Iroha limit of 28: $scale" }
        val mantissa = decimal.movePointRight(scale).toBigIntegerExact()
        require(mantissa.bitLength() < 512) { "Numeric mantissa exceeds Iroha limit of 512 bits: ${mantissa.bitLength()}" }
        return NumericValue(mantissa, scale)
    }

    private class NumericValue(val mantissa: BigInteger, val scale: Int)

    private class TransferAssetPayload(val source: AssetId, val amount: NumericValue, val destination: AccountId)

    private class AssetDefinitionId(aidBytes: ByteArray) {
        private val _aidBytes: ByteArray = aidBytes.clone()
        init { require(_aidBytes.size == 16) { "aidBytes must be 16 bytes, got ${_aidBytes.size}" } }
        fun aidBytes(): ByteArray = _aidBytes.clone()

        companion object {
            fun fromAid(aidString: String): AssetDefinitionId =
                AssetDefinitionId(AssetDefinitionIdEncoder.parseAidBytes(aidString))

            fun fromNameDomain(name: String, domain: String): AssetDefinitionId =
                AssetDefinitionId(AssetDefinitionIdEncoder.computeAidBytes(name, domain))

            fun parse(assetDefinitionId: String): AssetDefinitionId {
                if (AssetDefinitionIdEncoder.isAidEncoded(assetDefinitionId)) return fromAid(assetDefinitionId)
                val hashIndex = assetDefinitionId.indexOf('#')
                require(hashIndex >= 0) { "Invalid AssetDefinitionId format: $assetDefinitionId" }
                val name = assetDefinitionId.substring(0, hashIndex)
                val domainName = assetDefinitionId.substring(hashIndex + 1)
                return fromNameDomain(name, domainName)
            }
        }
    }

    private class AccountController private constructor(
        private val publicKeyMultihash: String?,
        private val multisigPolicy: MultisigPolicyPayload?,
    ) {
        fun isSingle(): Boolean = publicKeyMultihash != null
        fun publicKeyMultihash(): String = publicKeyMultihash!!
        fun multisigPolicy(): MultisigPolicyPayload = multisigPolicy!!

        companion object {
            fun single(publicKeyMultihash: String): AccountController =
                AccountController(publicKeyMultihash, null)
            fun multisig(policy: MultisigPolicyPayload): AccountController =
                AccountController(null, policy)
        }
    }

    private class AccountId(val controller: AccountController) {
        companion object {
            fun parse(accountIdStr: String): AccountId {
                val atIndex = accountIdStr.lastIndexOf('@')
                val signatory = if (atIndex >= 0) accountIdStr.substring(0, atIndex) else accountIdStr

                val pk = decodePublicKeyLiteral(signatory)
                if (pk != null) {
                    val multihash = encodePublicKeyMultihash(pk.curveId, pk.keyBytes)
                    return AccountId(AccountController.single(multihash))
                }

                val address: AccountAddress
                try {
                    address = AccountAddress.parseEncodedIgnoringCurveSupport(signatory, null).address
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException("Failed to parse account identifier: $signatory", e)
                }

                try {
                    val singleKey = address.singleKeyPayloadIgnoringCurveSupport()
                    if (singleKey != null) {
                        val multihash = encodePublicKeyMultihash(singleKey.curveId, singleKey.publicKey)
                        return AccountId(AccountController.single(multihash))
                    }
                    val multisig = address.multisigPolicyPayloadIgnoringCurveSupport()
                    if (multisig != null) {
                        return AccountId(AccountController.multisig(multisig))
                    }
                } catch (e: AccountAddressException) {
                    throw IllegalArgumentException("Failed to extract controller from I105 address", e)
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

        companion object {
            fun parse(assetIdStr: String): AssetId {
                if (AssetIdDecoder.isNoritoEncoded(assetIdStr)) return parseNoritoEncoded(assetIdStr)

                val lastHashIndex = assetIdStr.lastIndexOf('#')
                require(lastHashIndex >= 0) { "Invalid AssetId format: $assetIdStr" }

                val accountIdPart = assetIdStr.substring(lastHashIndex + 1)
                val assetDefPart = assetIdStr.substring(0, lastHashIndex)
                val accountId = AccountId.parse(accountIdPart)

                val atIndex = accountIdPart.lastIndexOf('@')
                val accountDomain = if (atIndex >= 0) accountIdPart.substring(atIndex + 1) else ""

                val assetDef: AssetDefinitionId
                if (AssetDefinitionIdEncoder.isAidEncoded(assetDefPart)) {
                    assetDef = AssetDefinitionId.fromAid(assetDefPart)
                } else if (assetDefPart.endsWith("#")) {
                    val assetName = assetDefPart.substring(0, assetDefPart.length - 1)
                    assetDef = AssetDefinitionId.fromNameDomain(assetName, accountDomain)
                } else {
                    val hashIndex = assetDefPart.indexOf('#')
                    require(hashIndex >= 0) { "Invalid AssetId format: $assetIdStr" }
                    val assetName = assetDefPart.substring(0, hashIndex)
                    val assetDomain = assetDefPart.substring(hashIndex + 1)
                    assetDef = AssetDefinitionId.fromNameDomain(assetName, assetDomain)
                }
                return AssetId(accountId, assetDef, null, globalScopePayload())
            }

            private fun parseNoritoEncoded(noritoAssetId: String): AssetId {
                val raw = extractNoritoBytes(noritoAssetId)
                val decoded = NoritoHeader.decode(raw, SchemaHash.hash16("iroha_data_model::asset::id::model::AssetId"))
                val header = decoded.header
                val payload = decoded.payload
                header.validateChecksum(payload)
                val sourceFlags = header.flags
                val unsupportedFlags = sourceFlags and NoritoHeader.COMPACT_LEN.inv()
                require(unsupportedFlags == 0) {
                    "Unsupported norito AssetId layout flags for transfer encoding: 0x%02x".format(unsupportedFlags)
                }

                val compactLen = (sourceFlags and NoritoHeader.COMPACT_LEN) != 0
                val decoder = NoritoDecoder(payload, sourceFlags, header.minor)

                val encodedAccountPayload = readSizedField(decoder, compactLen, "AssetId.account")
                val account: AccountId
                try {
                    account = decodeEncodedAccountPayload(encodedAccountPayload, sourceFlags, header.minor)
                } catch (ex: IllegalArgumentException) {
                    throw IllegalArgumentException("Invalid AssetId.account payload", ex)
                }
                val definitionPayload = readSizedField(decoder, compactLen, "AssetId.definition")
                val aidBytes = decodeFixedByteArray(definitionPayload, 16, sourceFlags, header.minor)
                val scopePayload = readSizedField(decoder, compactLen, "AssetId.scope")
                val scope: AssetBalanceScopePayload
                try {
                    scope = decodeAssetBalanceScopePayload(scopePayload, sourceFlags, header.minor)
                } catch (ex: IllegalArgumentException) {
                    throw IllegalArgumentException("Invalid AssetId.scope payload", ex)
                }
                require(decoder.remaining() == 0) { "Trailing bytes after AssetId payload" }

                return if (sourceFlags != 0) {
                    AssetId(account, AssetDefinitionId(aidBytes), null, encodeAssetBalanceScopePayload(scope))
                } else {
                    AssetId(null, AssetDefinitionId(aidBytes), encodedAccountPayload, scopePayload)
                }
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
            encoder.writeUInt(variantPayload.size.toLong(), 64)
            encoder.writeBytes(variantPayload)
        }

        private fun encodeTransferStruct(encoder: NoritoEncoder, value: TransferAssetPayload) {
            encodeFieldWithLength(encoder, assetIdAdapter, value.source)
            encodeFieldWithLength(encoder, NumericAdapter(), value.amount)
            encodeFieldWithLength(encoder, accountIdAdapter, value.destination)
        }

        private fun <T> encodeFieldWithLength(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
            val child = encoder.childEncoder()
            adapter.encode(child, value)
            val payload = child.toByteArray()
            encoder.writeUInt(payload.size.toLong(), 64)
            encoder.writeBytes(payload)
        }

        override fun decode(decoder: NoritoDecoder): TransferAssetPayload =
            throw UnsupportedOperationException("Decoding transfer payloads is not supported")
    }

    private class AssetDefinitionIdAdapter : TypeAdapter<AssetDefinitionId> {
        override fun encode(encoder: NoritoEncoder, value: AssetDefinitionId) {
            encodeFixedByteArray(encoder, value.aidBytes())
        }

        override fun decode(decoder: NoritoDecoder): AssetDefinitionId =
            throw UnsupportedOperationException("Decoding AssetDefinitionId is not supported")
    }

    private class AccountIdAdapter : TypeAdapter<AccountId> {
        private val controllerAdapter: TypeAdapter<AccountController> = AccountControllerAdapter()

        override fun encode(encoder: NoritoEncoder, value: AccountId) {
            controllerAdapter.encode(encoder, value.controller)
        }

        override fun decode(decoder: NoritoDecoder): AccountId =
            throw UnsupportedOperationException("Decoding AccountId is not supported")
    }

    private class AccountControllerAdapter : TypeAdapter<AccountController> {
        override fun encode(encoder: NoritoEncoder, value: AccountController) {
            if (value.isSingle()) {
                encodeSingle(encoder, value.publicKeyMultihash())
            } else {
                encodeMultisig(encoder, value.multisigPolicy())
            }
        }

        private fun encodeSingle(encoder: NoritoEncoder, publicKeyMultihash: String) {
            UINT32_ADAPTER.encode(encoder, 0L)
            val child = encoder.childEncoder()
            STRING_ADAPTER.encode(child, publicKeyMultihash)
            val payload = child.toByteArray()
            encoder.writeUInt(payload.size.toLong(), 64)
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
            encoder.writeUInt(policyPayload.size.toLong(), 64)
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
                val memberMultihash = encodePublicKeyMultihash(member.curveId, member.publicKey)
                encodeSizedField(memberEncoder, STRING_ADAPTER, memberMultihash)
                encodeSizedField(memberEncoder, UINT16_ADAPTER, member.weight.toLong())
                val memberPayload = memberEncoder.toByteArray()
                vecEncoder.writeUInt(memberPayload.size.toLong(), 64)
                vecEncoder.writeBytes(memberPayload)
            }
            val vecPayload = vecEncoder.toByteArray()
            encoder.writeUInt(vecPayload.size.toLong(), 64)
            encoder.writeBytes(vecPayload)
        }

        override fun decode(decoder: NoritoDecoder): AccountController =
            throw UnsupportedOperationException("Decoding AccountController is not supported")
    }

    private class AssetIdAdapter : TypeAdapter<AssetId> {
        private val accountIdAdapter: TypeAdapter<AccountId> = AccountIdAdapter()
        private val assetDefIdAdapter: TypeAdapter<AssetDefinitionId> = AssetDefinitionIdAdapter()

        override fun encode(encoder: NoritoEncoder, value: AssetId) {
            val encodedAccountPayload = value.encodedAccountPayload()
            if (encodedAccountPayload != null) {
                encoder.writeUInt(encodedAccountPayload.size.toLong(), 64)
                encoder.writeBytes(encodedAccountPayload)
            } else {
                encodeFieldWithLength(encoder, accountIdAdapter, value.account!!)
            }
            encodeFieldWithLength(encoder, assetDefIdAdapter, value.definition)
            val scopePayload = value.scopePayload()
            encoder.writeUInt(scopePayload.size.toLong(), 64)
            encoder.writeBytes(scopePayload)
        }

        override fun decode(decoder: NoritoDecoder): AssetId =
            throw UnsupportedOperationException("Decoding AssetId is not supported")

        private fun <T> encodeFieldWithLength(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
            val child = encoder.childEncoder()
            adapter.encode(child, value)
            val payload = child.toByteArray()
            encoder.writeUInt(payload.size.toLong(), 64)
            encoder.writeBytes(payload)
        }
    }

    private class NumericAdapter : TypeAdapter<NumericValue> {
        override fun encode(encoder: NoritoEncoder, value: NumericValue) {
            encodeFieldBigInt(encoder, value.mantissa)
            encodeFieldU32(encoder, value.scale)
        }

        override fun decode(decoder: NoritoDecoder): NumericValue =
            throw UnsupportedOperationException("Decoding numeric values is not supported")

        private fun encodeFieldBigInt(encoder: NoritoEncoder, value: BigInteger) {
            val child = encoder.childEncoder()
            encodeBigInt(child, value)
            val payload = child.toByteArray()
            encoder.writeUInt(payload.size.toLong(), 64)
            encoder.writeBytes(payload)
        }

        private fun encodeFieldU32(encoder: NoritoEncoder, value: Int) {
            encoder.writeUInt(4, 64)
            UINT32_ADAPTER.encode(encoder, value.toLong())
        }

        private fun encodeBigInt(encoder: NoritoEncoder, value: BigInteger) {
            val twosCompBytes = toTwosComplementLittleEndian(value)
            encoder.writeUInt(twosCompBytes.size.toLong(), 32)
            encoder.writeBytes(twosCompBytes)
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
        encoder.writeUInt(payload.size.toLong(), 64)
        encoder.writeBytes(payload)
    }

    private fun globalScopePayload(): ByteArray {
        val encoder = NoritoEncoder(0)
        UINT32_ADAPTER.encode(encoder, 0L)
        return encoder.toByteArray()
    }

    private fun extractNoritoBytes(noritoString: String): ByteArray {
        val prefix = "norito:"
        require(noritoString.regionMatches(0, prefix, 0, prefix.length, ignoreCase = true)) {
            "Value must start with norito: prefix"
        }
        val hex = noritoString.substring(prefix.length)
        require(hex.length % 2 == 0) { "Hex string must have even length" }
        return ByteArray(hex.length / 2) { i ->
            val hi = Character.digit(hex[i * 2], 16)
            val lo = Character.digit(hex[i * 2 + 1], 16)
            require(hi >= 0 && lo >= 0) { "Invalid hex character at position ${i * 2}" }
            ((hi shl 4) or lo).toByte()
        }
    }

    private fun readSizedField(decoder: NoritoDecoder, compactLen: Boolean, fieldName: String): ByteArray {
        val fieldLength = checkedLength(decoder.readLength(compactLen), "$fieldName field")
        return decoder.readBytes(fieldLength)
    }

    private fun decodeFixedByteArray(payload: ByteArray, expectedLen: Int, flags: Int, flagsHint: Int): ByteArray {
        if (payload.size == expectedLen) return payload.clone()
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
        val result = ByteArray(expectedLen)
        for (i in 0 until expectedLen) {
            val elementLen = decoder.readLength(compactLen)
            require(elementLen == 1L) { "Expected 1-byte element, got $elementLen" }
            result[i] = decoder.readByte().toByte()
        }
        require(decoder.remaining() == 0) { "Trailing bytes after fixed byte array" }
        return result
    }

    private fun decodeEncodedAccountPayload(payload: ByteArray, flags: Int, flagsHint: Int): AccountId {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
        val controllerTag = UINT32_ADAPTER.decode(decoder)
        val variantLength = checkedLength(decoder.readLength(compactLen), "AccountController variant payload")
        val variantPayload = decoder.readBytes(variantLength)
        require(decoder.remaining() == 0) { "Trailing bytes after AssetId.account payload" }

        if (controllerTag == 0L) {
            val canonicalMultihash = decodeSingleControllerVariant(variantPayload, flags, flagsHint)
            return AccountId(AccountController.single(canonicalMultihash))
        }
        if (controllerTag == 1L) {
            val multisigPolicy = decodeMultisigControllerVariant(variantPayload, flags, flagsHint)
            return AccountId(AccountController.multisig(multisigPolicy))
        }
        throw IllegalArgumentException("Unknown AccountController discriminant in AssetId.account: $controllerTag")
    }

    private fun decodeSingleControllerVariant(payload: ByteArray, flags: Int, flagsHint: Int): String {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val multihash = STRING_ADAPTER.decode(decoder)
        require(decoder.remaining() == 0) { "Trailing bytes after AssetId.account single controller" }
        val publicKey = decodePublicKeyLiteral(multihash)
            ?: throw IllegalArgumentException("Invalid public key multihash in AssetId.account")
        return encodePublicKeyMultihash(publicKey.curveId, publicKey.keyBytes)
    }

    private fun decodeMultisigControllerVariant(payload: ByteArray, flags: Int, flagsHint: Int): MultisigPolicyPayload {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val version = Math.toIntExact(decodeSizedTypedField(decoder, UINT8_ADAPTER, "MultisigPolicy.version"))
        val threshold = Math.toIntExact(decodeSizedTypedField(decoder, UINT16_ADAPTER, "MultisigPolicy.threshold"))
        val membersPayloadLen = checkedLength(
            decoder.readLength((flags and NoritoHeader.COMPACT_LEN) != 0), "MultisigPolicy.members payload")
        val membersPayload = decoder.readBytes(membersPayloadLen)
        require(decoder.remaining() == 0) { "Trailing bytes after AssetId.account multisig policy" }

        val membersDecoder = NoritoDecoder(membersPayload, flags, flagsHint)
        val membersCount = checkedLength(membersDecoder.readLength(false), "Multisig members count")
        val members = mutableListOf<MultisigMemberPayload>()
        for (i in 0 until membersCount) {
            val memberLen = checkedLength(
                membersDecoder.readLength((flags and NoritoHeader.COMPACT_LEN) != 0), "Multisig member payload")
            val memberPayload = membersDecoder.readBytes(memberLen)
            val memberDecoder = NoritoDecoder(memberPayload, flags, flagsHint)
            val memberMultihash = decodeSizedTypedField(memberDecoder, STRING_ADAPTER, "Multisig member public key")
            val weight = Math.toIntExact(decodeSizedTypedField(memberDecoder, UINT16_ADAPTER, "Multisig member weight"))
            require(memberDecoder.remaining() == 0) { "Trailing bytes after multisig member payload" }
            val keyPayload = decodePublicKeyLiteral(memberMultihash)
                ?: throw IllegalArgumentException("Invalid multisig member public key")
            members.add(MultisigMemberPayload(keyPayload.curveId, weight, keyPayload.keyBytes))
        }
        require(membersDecoder.remaining() == 0) { "Trailing bytes after multisig member vector payload" }
        validateMultisigPolicySemantics(version, threshold, members)
        return MultisigPolicyPayload.of(version, threshold, members)
    }

    private fun validateMultisigPolicySemantics(version: Int, threshold: Int, members: List<MultisigMemberPayload>) {
        require(version == MULTISIG_POLICY_VERSION_V1) { "Invalid multisig policy: unsupported version $version" }
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

    private fun decodeAssetBalanceScopePayload(payload: ByteArray, flags: Int, flagsHint: Int): AssetBalanceScopePayload {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val scopeTag = UINT32_ADAPTER.decode(decoder)
        if (scopeTag == 0L) {
            require(decoder.remaining() == 0) { "Trailing bytes after AssetBalanceScope::Global" }
            return AssetBalanceScopePayload.global()
        }
        if (scopeTag == 1L) {
            val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
            val variantLen = checkedLength(decoder.readLength(compactLen), "AssetBalanceScope::Dataspace payload")
            val variantPayload = decoder.readBytes(variantLen)
            require(decoder.remaining() == 0) { "Trailing bytes after AssetBalanceScope payload" }
            val variantDecoder = NoritoDecoder(variantPayload, flags, flagsHint)
            val dataspaceId = variantDecoder.readUInt(64)
            require(variantDecoder.remaining() == 0) { "Trailing bytes after AssetBalanceScope::Dataspace value" }
            return AssetBalanceScopePayload.dataspace(dataspaceId)
        }
        throw IllegalArgumentException("Unknown AssetBalanceScope discriminant in AssetId.scope: $scopeTag")
    }

    private fun encodeAssetBalanceScopePayload(scope: AssetBalanceScopePayload): ByteArray {
        if (scope.isGlobal) return globalScopePayload()
        val encoder = NoritoEncoder(0)
        UINT32_ADAPTER.encode(encoder, 1L)
        encoder.writeUInt(8, 64)
        encoder.writeUInt(scope.dataspaceId, 64)
        return encoder.toByteArray()
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

    private class AssetBalanceScopePayload private constructor(val isGlobal: Boolean, val dataspaceId: Long) {
        companion object {
            fun global(): AssetBalanceScopePayload = AssetBalanceScopePayload(true, 0L)
            fun dataspace(dataspaceId: Long): AssetBalanceScopePayload = AssetBalanceScopePayload(false, dataspaceId)
        }
    }
}
