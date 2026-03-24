package org.hyperledger.iroha.sdk.client

/** Parsed payload for account alias resolution (`/v1/aliases/resolve`). */
class AccountAliasResolution(
    @JvmField val alias: String,
    @JvmField val accountId: String,
    @JvmField val index: Long?,
    @JvmField val source: String?,
)
