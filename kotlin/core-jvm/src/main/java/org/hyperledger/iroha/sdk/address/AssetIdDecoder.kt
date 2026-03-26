// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

/**
 * Parses canonical public asset identifiers.
 */
object AssetIdDecoder {

    class AssetId(
        @JvmField val definition: AssetDefinition,
        @JvmField val accountId: String,
        @JvmField val dataspaceId: Long?,
    )

    /**
     * Checks whether the given value is a canonical public asset literal.
     */
    @JvmStatic
    fun isCanonical(value: String?): Boolean {
        if (value == null) {
            return false
        }
        return try {
            decode(value)
            true
        } catch (_: IllegalArgumentException) {
            false
        }
    }

    /**
     * Parses `<asset-definition-id>#<i105-account-id>` with an optional `#dataspace:<id>` suffix.
     */
    @JvmStatic
    fun decode(assetId: String): AssetId {
        val trimmed = assetId.trim()
        require(trimmed == assetId && trimmed.isNotEmpty()) {
            "AssetId must use canonical public form"
        }

        val parts = trimmed.split('#')
        require(parts.size == 2 || parts.size == 3) {
            "AssetId must use '<asset-definition-id>#<i105-account-id>' with optional '#dataspace:<id>' suffix"
        }

        val definitionAddress = parts[0]
        AssetDefinitionIdEncoder.parseAddressBytes(definitionAddress)

        val parsedAccount = try {
            AccountAddress.parseEncodedIgnoringCurveSupport(parts[1], null).address
        } catch (ex: AccountAddressException) {
            throw IllegalArgumentException("AssetId.account must use canonical i105 form", ex)
        }
        val canonicalAccountId = try {
            parsedAccount.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        } catch (ex: AccountAddressException) {
            throw IllegalArgumentException("AssetId.account must use canonical i105 form", ex)
        }

        val dataspaceId = when (parts.size) {
            2 -> null
            else -> parseDataspace(parts[2])
        }

        return AssetId(AssetDefinition(definitionAddress), canonicalAccountId, dataspaceId)
    }

    /**
     * Validates a canonical asset-definition address and returns it as an [AssetDefinition].
     */
    @JvmStatic
    fun decodeDefinition(definitionId: String): AssetDefinition {
        val trimmed = definitionId.trim()
        require(trimmed == definitionId && trimmed.isNotEmpty()) {
            "Asset definition id must use canonical unprefixed Base58 form"
        }
        AssetDefinitionIdEncoder.parseAddressBytes(trimmed)
        return AssetDefinition(trimmed)
    }

    private fun parseDataspace(scopeLiteral: String): Long {
        val match = Regex("^dataspace:(\\d+)$").matchEntire(scopeLiteral)
            ?: throw IllegalArgumentException(
                "AssetId.scope must use 'dataspace:<id>' when present"
            )
        return match.groupValues[1].toLong()
    }
}
