// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter

private const val NORITO_PREFIX = "norito:"
private const val AID_BYTES_LEN = 16
private const val MULTISIG_POLICY_VERSION_V1 = 1

/**
 * Decodes norito-encoded asset identifiers to extract the asset definition ID.
 *
 * Iroha returns asset IDs in `norito:<hex>` format. The binary payload encodes:
 *
 * ```
 *   struct AssetId {
 *       account: AccountId,
 *       definition: AssetDefinitionId,  // raw [u8; 16] aid_bytes
 *   }
 * ```
 *
 * `AssetDefinitionId` is a one-way blake3 hash -- name and domain cannot be recovered.
 * Use the app's cached asset definitions for display name lookup.
 */
object AssetIdDecoder {

    private val ASSET_ID_SCHEMA_HASH =
        SchemaHash.hash16("iroha_data_model::asset::id::model::AssetId")
    private val ASSET_DEF_ID_SCHEMA_HASH =
        SchemaHash.hash16("iroha_data_model::asset::id::model::AssetDefinitionId")
    private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
    private val UINT8_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(8)
    private val UINT16_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(16)
    private val UINT32_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(32)

    /**
     * Checks whether the given string is a norito-encoded identifier.
     *
     * @param value the string to test
     * @return `true` if the string starts with `norito:`
     */
    @JvmStatic
    fun isNoritoEncoded(value: String?): Boolean =
        value != null && value.regionMatches(0, NORITO_PREFIX, 0, NORITO_PREFIX.length, ignoreCase = true)

    /**
     * Decodes a `norito:<hex>` asset identifier (full AssetId with account + definition).
     *
     * @param noritoAssetId the full `norito:<hex>` string
     * @return decoded asset definition
     */
    @JvmStatic
    fun decode(noritoAssetId: String): AssetDefinition {
        val data = extractNoritoBytes(noritoAssetId)
        return decodeAssetIdBytes(data)
    }

    /**
     * Decodes a `norito:<hex>` asset definition identifier (AssetDefinitionId only).
     *
     * @param noritoDefinitionId the full `norito:<hex>` string
     * @return decoded asset definition
     */
    @JvmStatic
    fun decodeDefinition(noritoDefinitionId: String): AssetDefinition {
        val data = extractNoritoBytes(noritoDefinitionId)
        return decodeDefinitionIdBytes(data)
    }

    /**
     * Decodes raw norito bytes for a full AssetId (account + definition).
     * Skips the account field and extracts the definition's 16-byte aid.
     */
    private fun decodeAssetIdBytes(data: ByteArray): AssetDefinition {
        val headerResult = NoritoHeader.decode(data, ASSET_ID_SCHEMA_HASH)
        val header = headerResult.header
        val payload = headerResult.payload
        header.validateChecksum(payload)

        val flags = header.flags
        val unsupportedFlags = flags and NoritoHeader.COMPACT_LEN.inv()
        require(unsupportedFlags == 0) {
            "Unsupported norito AssetId layout flags for decoding: 0x%02x".format(unsupportedFlags)
        }
        val flagsHint = header.minor
        val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
        val decoder = NoritoDecoder(payload, flags, flagsHint)

        // AssetId struct: { account: AccountId, definition: AssetDefinitionId, scope: ... }
        val accountLen = checkedLength(decoder.readLength(compactLen), "Account field")
        val accountPayload = decoder.readBytes(accountLen)
        try {
            validateAccountPayload(accountPayload, flags, flagsHint)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("Invalid AssetId.account payload", ex)
        }

        val definitionLen = checkedLength(decoder.readLength(compactLen), "Definition field")
        val definitionPayload = decoder.readBytes(definitionLen)
        val aidBytes = decodeFixedByteArray(definitionPayload, AID_BYTES_LEN, flags, flagsHint)

        val scopeLen = checkedLength(decoder.readLength(compactLen), "Scope field")
        val scopePayload = decoder.readBytes(scopeLen)
        validateScopePayload(scopePayload, flags, flagsHint)

        require(decoder.remaining() == 0) { "Trailing bytes after AssetId payload" }

        return AssetDefinition(aidBytesToHex(aidBytes))
    }

    /**
     * Decodes raw norito bytes for an AssetDefinitionId (no account, just the [u8; 16] aid).
     */
    private fun decodeDefinitionIdBytes(data: ByteArray): AssetDefinition {
        val headerResult = NoritoHeader.decode(data, ASSET_DEF_ID_SCHEMA_HASH)
        val header = headerResult.header
        val payload = headerResult.payload
        header.validateChecksum(payload)
        val flags = header.flags
        val unsupportedFlags = flags and NoritoHeader.COMPACT_LEN.inv()
        require(unsupportedFlags == 0) {
            "Unsupported norito AssetDefinitionId layout flags for decoding: 0x%02x".format(unsupportedFlags)
        }

        val aidBytes = decodeFixedByteArray(payload, AID_BYTES_LEN, header.flags, header.minor)
        return AssetDefinition(aidBytesToHex(aidBytes))
    }

    /**
     * Decodes a Norito fixed-size byte array (`[u8; N]`) where each element is u64-prefixed.
     */
    private fun decodeFixedByteArray(
        payload: ByteArray,
        expectedLen: Int,
        flags: Int,
        flagsHint: Int,
    ): ByteArray {
        // Fast path: raw N bytes without per-element length headers (Rust core.rs:2071)
        if (payload.size == expectedLen) return payload.copyOf()
        // Slow path: per-element length-prefixed bytes
        val d = NoritoDecoder(payload, flags, flagsHint)
        val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
        val result = ByteArray(expectedLen)
        for (i in 0 until expectedLen) {
            val elemLen = d.readLength(compactLen)
            require(elemLen == 1L) { "Expected 1-byte element, got $elemLen" }
            result[i] = d.readByte().toByte()
        }
        require(d.remaining() == 0) { "Trailing bytes after fixed byte array" }
        return result
    }

    private fun validateScopePayload(payload: ByteArray, flags: Int, flagsHint: Int) {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val scopeTag = UINT32_ADAPTER.decode(decoder)
        if (scopeTag == 0L) {
            require(decoder.remaining() == 0) { "Trailing bytes after AssetBalanceScope::Global" }
            return
        }
        if (scopeTag == 1L) {
            val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
            val variantLen = checkedLength(decoder.readLength(compactLen), "Dataspace scope payload")
            val variantPayload = decoder.readBytes(variantLen)
            require(decoder.remaining() == 0) { "Trailing bytes after AssetBalanceScope payload" }
            val variantDecoder = NoritoDecoder(variantPayload, flags, flagsHint)
            variantDecoder.readUInt(64)
            require(variantDecoder.remaining() == 0) {
                "Trailing bytes after AssetBalanceScope::Dataspace value"
            }
            return
        }
        throw IllegalArgumentException(
            "Unknown AssetBalanceScope discriminant in AssetId.scope: $scopeTag"
        )
    }

    private fun validateAccountPayload(payload: ByteArray, flags: Int, flagsHint: Int) {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val compactLen = (flags and NoritoHeader.COMPACT_LEN) != 0
        val controllerTag = UINT32_ADAPTER.decode(decoder)
        val variantLen =
            checkedLength(decoder.readLength(compactLen), "AccountController variant payload")
        val variantPayload = decoder.readBytes(variantLen)
        require(decoder.remaining() == 0) { "Trailing bytes after AssetId.account payload" }

        when (controllerTag) {
            0L -> validateSingleControllerVariant(variantPayload, flags, flagsHint)
            1L -> validateMultisigControllerVariant(variantPayload, flags, flagsHint)
            else -> throw IllegalArgumentException(
                "Unknown AccountController discriminant in AssetId.account: $controllerTag"
            )
        }
    }

    private fun validateSingleControllerVariant(payload: ByteArray, flags: Int, flagsHint: Int) {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val multihash = STRING_ADAPTER.decode(decoder)
        require(decoder.remaining() == 0) {
            "Trailing bytes after AssetId.account single controller"
        }
        require(decodePublicKeyLiteral(multihash) != null) {
            "Invalid public key multihash in AssetId.account"
        }
    }

    private fun validateMultisigControllerVariant(payload: ByteArray, flags: Int, flagsHint: Int) {
        val decoder = NoritoDecoder(payload, flags, flagsHint)
        val version = Math.toIntExact(
            decodeSizedTypedField(decoder, UINT8_ADAPTER, "MultisigPolicy.version")
        )
        val threshold = Math.toIntExact(
            decodeSizedTypedField(decoder, UINT16_ADAPTER, "MultisigPolicy.threshold")
        )
        val membersPayloadLen = checkedLength(
            decoder.readLength((flags and NoritoHeader.COMPACT_LEN) != 0),
            "MultisigPolicy.members payload"
        )
        val membersPayload = decoder.readBytes(membersPayloadLen)
        require(decoder.remaining() == 0) {
            "Trailing bytes after AssetId.account multisig policy"
        }

        val membersDecoder = NoritoDecoder(membersPayload, flags, flagsHint)
        val membersCount = checkedLength(membersDecoder.readLength(false), "Multisig members count")
        val members = ArrayList<MultisigMemberPayload>(membersCount)
        for (i in 0 until membersCount) {
            val memberLen = checkedLength(
                membersDecoder.readLength((flags and NoritoHeader.COMPACT_LEN) != 0),
                "Multisig member payload"
            )
            val memberPayload = membersDecoder.readBytes(memberLen)
            val memberDecoder = NoritoDecoder(memberPayload, flags, flagsHint)

            val memberMultihash =
                decodeSizedTypedField(memberDecoder, STRING_ADAPTER, "Multisig member public key")
            val weight = Math.toIntExact(
                decodeSizedTypedField(memberDecoder, UINT16_ADAPTER, "Multisig member weight")
            )
            require(memberDecoder.remaining() == 0) {
                "Trailing bytes after multisig member payload"
            }

            val keyPayload = decodePublicKeyLiteral(memberMultihash)
                ?: throw IllegalArgumentException("Invalid multisig member public key")
            members.add(
                MultisigMemberPayload(
                    keyPayload.curveId, weight, keyPayload.keyBytes
                )
            )
        }
        require(membersDecoder.remaining() == 0) {
            "Trailing bytes after multisig member vector payload"
        }

        validateMultisigPolicySemantics(version, threshold, members)
    }

    private fun validateMultisigPolicySemantics(
        version: Int,
        threshold: Int,
        members: List<MultisigMemberPayload>,
    ) {
        require(version == MULTISIG_POLICY_VERSION_V1) {
            "Invalid multisig policy: unsupported version $version"
        }
        require(members.isNotEmpty()) { "Invalid multisig policy: zero members" }
        var totalWeight = 0L
        val sortKeys = ArrayList<ByteArray>(members.size)
        for (member in members) {
            require(member.weight > 0) { "Invalid multisig policy: non-positive weight" }
            require(member.publicKey.isNotEmpty()) { "Invalid multisig policy: empty public key" }
            totalWeight += member.weight
            sortKeys.add(canonicalSortKey(member))
        }
        require(threshold > 0) { "Invalid multisig policy: zero threshold" }
        require(totalWeight >= threshold) {
            "Invalid multisig policy: threshold exceeds total weight"
        }
        sortKeys.sortWith(::compareUnsigned)
        for (i in 1 until sortKeys.size) {
            require(!sortKeys[i - 1].contentEquals(sortKeys[i])) {
                "Invalid multisig policy: duplicate member"
            }
        }
    }

    private fun canonicalSortKey(member: MultisigMemberPayload): ByteArray {
        val algorithm = algorithmForCurveId(member.curveId)
            ?: throw IllegalArgumentException("Invalid multisig policy: unknown curve id")
        val algorithmBytes = algorithm.toByteArray(Charsets.UTF_8)
        val keyBytes = member.publicKey
        val sortKey = ByteArray(algorithmBytes.size + 1 + keyBytes.size)
        algorithmBytes.copyInto(sortKey)
        sortKey[algorithmBytes.size] = 0
        keyBytes.copyInto(sortKey, algorithmBytes.size + 1)
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

    private fun <T> decodeSizedTypedField(
        decoder: NoritoDecoder,
        adapter: TypeAdapter<T>,
        fieldName: String,
    ): T {
        val payloadLength = checkedLength(
            decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0),
            "$fieldName payload"
        )
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

    private fun aidBytesToHex(aidBytes: ByteArray): String = buildString(4 + aidBytes.size * 2) {
        append("aid:")
        for (b in aidBytes) {
            append("%02x".format(b.toInt() and 0xFF))
        }
    }

    private fun extractNoritoBytes(noritoString: String): ByteArray {
        require(noritoString.regionMatches(0, NORITO_PREFIX, 0, NORITO_PREFIX.length, ignoreCase = true)) {
            "Value must start with norito: prefix"
        }
        val hex = noritoString.substring(NORITO_PREFIX.length).lowercase()
        return hexToBytes(hex)
    }

    private fun hexToBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0) { "Hex string must have even length" }
        val bytes = ByteArray(hex.length / 2)
        for (i in bytes.indices) {
            val hi = Character.digit(hex[i * 2], 16)
            val lo = Character.digit(hex[i * 2 + 1], 16)
            require(hi >= 0 && lo >= 0) { "Invalid hex character at position ${i * 2}" }
            bytes[i] = ((hi shl 4) or lo).toByte()
        }
        return bytes
    }
}
