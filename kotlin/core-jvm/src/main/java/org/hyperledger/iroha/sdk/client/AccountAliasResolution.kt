package org.hyperledger.iroha.sdk.client

import java.math.BigInteger

/** Parsed payload for account alias resolution (`/v1/aliases/resolve`). */
class AccountAliasResolution(
    @JvmField val alias: String,
    @JvmField val accountId: String,
    @JvmField val index: BigInteger?,
    @JvmField val source: String?,
) {
    init {
        index?.let { requireAliasU64(it, "index") }
    }
}
