package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.alias.AccountAliasName

/** Parsed payload for account alias resolution (`/v1/aliases/resolve`). */
class AccountAliasResolution(
    alias: String,
    accountId: String,
    index: BigInteger?,
    source: String?,
) {
    /** Exact canonical account alias returned by Torii. */
    @JvmField
    val alias: String = AccountAliasName.parse(alias).canonicalText().also {
        require(it == alias) { "alias must use its canonical representation" }
    }

    /** Exact canonical domainless account identifier returned by Torii. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Optional numeric alias index. */
    @JvmField
    val index: BigInteger? = index?.let { requireAliasU64(it, "index") }

    /** Resolution source, when supplied by Torii. */
    @JvmField
    val source: String? = source?.also { requireExactText(it, "source") }
}
