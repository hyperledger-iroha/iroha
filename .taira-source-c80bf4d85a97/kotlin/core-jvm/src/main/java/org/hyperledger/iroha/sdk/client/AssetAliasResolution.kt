package org.hyperledger.iroha.sdk.client

/** Parsed payload for asset alias resolution (`/v1/assets/aliases/resolve`). */
class AssetAliasResolution(
    @JvmField val alias: String,
    @JvmField val assetDefinitionId: String,
    @JvmField val assetName: String,
    @JvmField val description: String?,
    @JvmField val logo: String?,
    @JvmField val source: String?,
)
