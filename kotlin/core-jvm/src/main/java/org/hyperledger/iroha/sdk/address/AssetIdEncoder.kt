// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

/** Public asset identifiers are canonical unprefixed Base58 asset-definition ids only. */
object AssetIdEncoder {

    /**
     * Returns the canonical public asset identifier from a definition address.
     */
    @JvmStatic
    fun encodeAssetIdFromDefinition(definitionAddress: String): String =
        canonicalDefinitionAddress(definitionAddress)

    private fun canonicalDefinitionAddress(definitionAddress: String): String {
        val trimmed = definitionAddress.trim()
        require(trimmed == definitionAddress && trimmed.isNotEmpty()) {
            "assetDefinitionId must use canonical unprefixed Base58 form"
        }
        AssetDefinitionIdEncoder.parseAddressBytes(trimmed)
        return trimmed
    }

}
