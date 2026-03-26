// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

/** Parses canonical public asset identifiers. */
object AssetIdDecoder {

    class AssetId(
        @JvmField val definition: AssetDefinition,
        @JvmField val accountId: String,
        @JvmField val dataspaceId: Long?,
    )

    /**
     * Checks whether the given value is a canonical Base58 asset-definition id.
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
     * Parses a canonical Base58 asset-definition id.
     */
    @JvmStatic
    fun decode(assetId: String): AssetId {
        val trimmed = assetId.trim()
        require(trimmed == assetId && trimmed.isNotEmpty()) {
            "AssetId must use canonical unprefixed Base58 asset-definition form"
        }

        AssetDefinitionIdEncoder.parseAddressBytes(trimmed)
        return AssetId(AssetDefinition(trimmed), "", null)
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
}
