// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

/**
 * Composes canonical public asset identifiers.
 *
 * Public asset literals use:
 * - `<asset-definition-id>#<i105-account-id>`
 * - `<asset-definition-id>#<i105-account-id>#dataspace:<id>`
 */
object AssetIdEncoder {

    /**
     * Computes a canonical public asset identifier from asset name, domain, and account id.
     */
    @JvmStatic
    fun encodeAssetId(assetName: String, domainName: String, accountId: String): String =
        encodeAssetIdFromDefinition(AssetDefinitionIdEncoder.encode(assetName, domainName), accountId)

    /**
     * Composes a canonical public asset identifier from a definition address and account id.
     */
    @JvmStatic
    fun encodeAssetIdFromDefinition(definitionAddress: String, accountId: String): String =
        buildString {
            append(canonicalDefinitionAddress(definitionAddress))
            append('#')
            append(canonicalAccountId(accountId))
        }

    /**
     * Composes a dataspace-scoped canonical public asset identifier.
     */
    @JvmStatic
    fun encodeScopedAssetIdFromDefinition(
        definitionAddress: String,
        accountId: String,
        dataspaceId: Long,
    ): String {
        require(dataspaceId >= 0) { "dataspaceId must be non-negative" }
        return "${encodeAssetIdFromDefinition(definitionAddress, accountId)}#dataspace:$dataspaceId"
    }

    private fun canonicalDefinitionAddress(definitionAddress: String): String {
        val trimmed = definitionAddress.trim()
        require(trimmed == definitionAddress && trimmed.isNotEmpty()) {
            "assetDefinitionId must use canonical unprefixed Base58 form"
        }
        AssetDefinitionIdEncoder.parseAddressBytes(trimmed)
        return trimmed
    }

    private fun canonicalAccountId(accountId: String): String {
        val trimmed = accountId.trim()
        require(trimmed == accountId && trimmed.isNotEmpty()) {
            "accountId must use canonical i105 form"
        }
        val parsed = try {
            AccountAddress.parseEncodedIgnoringCurveSupport(trimmed, null).address
        } catch (ex: AccountAddressException) {
            throw IllegalArgumentException("accountId must use canonical i105 form", ex)
        }
        return try {
            parsed.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        } catch (ex: AccountAddressException) {
            throw IllegalArgumentException("accountId must use canonical i105 form", ex)
        }
    }
}
