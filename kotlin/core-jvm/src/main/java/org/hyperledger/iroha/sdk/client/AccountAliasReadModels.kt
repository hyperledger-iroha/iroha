package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.alias.AccountAliasName

/** Typed request for aliases bound to one canonical account. */
class AccountAliasesByAccountRequest(
    accountId: String,
    dataspace: String? = null,
    domain: String? = null,
) {
    /** Canonical domainless account identifier. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Optional canonical textual dataspace filter. */
    @JvmField
    val dataspace: String?

    /** Optional canonical domain-label filter. */
    @JvmField
    val domain: String?

    init {
        require(domain == null || dataspace != null) { "domain requires a dataspace filter" }
        if (dataspace == null) {
            this.dataspace = null
            this.domain = null
        } else {
            val normalized = AccountAliasName("filter", domain, dataspace)
            this.dataspace = normalized.dataspace
            this.domain = normalized.domain
        }
    }

    /** Norito-JSON-compatible request object. */
    fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "account_id" to accountId,
        "dataspace" to dataspace,
        "domain" to domain,
    )
}

/** One visible alias bound to an account. */
class AccountAliasListItem(
    alias: String,
    dataspace: String,
    domain: String?,
    /** Whether this is the account's primary alias. */
    @JvmField val isPrimary: Boolean,
) {
    /** Canonical alias literal. */
    @JvmField
    val alias: String

    /** Canonical textual dataspace. */
    @JvmField
    val dataspace: String

    /** Optional canonical domain label. */
    @JvmField
    val domain: String?

    init {
        val parsed = AccountAliasName.parse(alias)
        val normalized = AccountAliasName("item", domain, dataspace)
        require(parsed.dataspace == normalized.dataspace && parsed.domain == normalized.domain) {
            "alias scope does not match dataspace/domain fields"
        }
        require(parsed.canonicalText() == alias) { "alias must use its canonical representation" }
        this.alias = parsed.canonicalText()
        this.dataspace = normalized.dataspace
        this.domain = normalized.domain
    }
}

/** Visibility-filtered aliases bound to one account. */
class AccountAliasesByAccount(
    accountId: String,
    total: BigInteger,
    items: List<AccountAliasListItem>,
    source: String?,
) {
    /** Canonical domainless account identifier. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Total after visibility filtering. */
    @JvmField
    val total: BigInteger = requireAliasU64(total, "total")

    /** Visible aliases in server canonical order. */
    @JvmField
    val items: List<AccountAliasListItem> = items.toList().also {
        require(total == BigInteger.valueOf(it.size.toLong())) { "total must match the visible item count" }
        require(it.zipWithNext().all { pair -> pair.first.alias <= pair.second.alias }) {
            "items must be sorted by canonical alias"
        }
    }

    /** Resolution source, when supplied by Torii. */
    @JvmField
    val source: String? = source?.also { requireExactText(it, "source") }
}

/** Typed result from resolving a numeric alias index. */
class AccountAliasIndexResolution(
    index: BigInteger,
    alias: String,
    accountId: String,
    source: String?,
) {
    /** Numeric alias index. */
    @JvmField
    val index: BigInteger = requireAliasU64(index, "index")

    /** Canonical account alias. */
    @JvmField
    val alias: String = AccountAliasName.parse(alias).canonicalText().also {
        require(it == alias) { "alias must use its canonical representation" }
    }

    /** Canonical domainless target account. */
    @JvmField
    val accountId: String = requireCanonicalI105Address(accountId, "accountId")

    /** Resolution source, when supplied by Torii. */
    @JvmField
    val source: String? = source?.also { requireExactText(it, "source") }
}

/** Strict parsers for typed alias index and account-list responses. */
object AccountAliasReadJsonParser {
    /** Parses `/v1/aliases/resolve-index`. */
    @JvmStatic
    fun parseIndexResolution(payload: ByteArray): AccountAliasIndexResolution {
        val root = root(payload, "alias index resolution")
        exactKeys(root, setOf("index", "alias", "account_id", "source"), setOf("source"), "alias index resolution")
        return AccountAliasIndexResolution(
            aliasU64(root["index"], "alias index resolution.index"),
            string(root["alias"], "alias index resolution.alias"),
            string(root["account_id"], "alias index resolution.account_id"),
            optionalString(root["source"], "alias index resolution.source"),
        )
    }

    /** Parses `/v1/aliases/by-account`. */
    @JvmStatic
    fun parseByAccount(payload: ByteArray): AccountAliasesByAccount {
        val root = root(payload, "aliases by account")
        exactKeys(root, setOf("account_id", "total", "items", "source"), setOf("source"), "aliases by account")
        val rawItems = root["items"] as? List<*> ?: error("aliases by account.items must be an array")
        val items = rawItems.mapIndexed { index, value ->
            val item = value as? Map<*, *> ?: error("aliases by account.items[$index] must be an object")
            check(item.keys.all { it is String }) { "aliases by account.items[$index] keys must be strings" }
            @Suppress("UNCHECKED_CAST")
            val typed = item as Map<String, Any?>
            val path = "aliases by account.items[$index]"
            exactKeys(typed, setOf("alias", "dataspace", "domain", "is_primary"), setOf("domain"), path)
            AccountAliasListItem(
                string(typed["alias"], "$path.alias"),
                string(typed["dataspace"], "$path.dataspace"),
                optionalString(typed["domain"], "$path.domain"),
                typed["is_primary"] as? Boolean ?: error("$path.is_primary must be a boolean"),
            )
        }
        return AccountAliasesByAccount(
            string(root["account_id"], "aliases by account.account_id"),
            aliasU64(root["total"], "aliases by account.total"),
            items,
            optionalString(root["source"], "aliases by account.source"),
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun root(payload: ByteArray, path: String): Map<String, Any?> {
        require(payload.isNotEmpty()) { "$path returned an empty payload" }
        val parsed = JsonParser.parse(String(payload, StandardCharsets.UTF_8))
        check(parsed is Map<*, *> && parsed.keys.all { it is String }) { "$path must be an object" }
        return parsed as Map<String, Any?>
    }

    private fun exactKeys(
        root: Map<String, Any?>,
        allowed: Set<String>,
        optional: Set<String>,
        path: String,
    ) {
        check(root.keys.all { it in allowed }) { "$path contains unknown fields" }
        check((allowed - optional).all { root.containsKey(it) }) { "$path is missing required fields" }
    }

    private fun string(value: Any?, path: String): String =
        (value as? String)?.also { requireExactText(it, path) } ?: error("$path must be a string")

    private fun optionalString(value: Any?, path: String): String? =
        if (value == null) null else string(value, path)
}

private val ALIAS_U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

internal fun requireAliasU64(value: BigInteger, path: String): BigInteger {
    require(value.signum() >= 0 && value <= ALIAS_U64_MAX) { "$path must fit in unsigned 64-bit range" }
    return value
}

internal fun aliasU64(value: Any?, path: String): BigInteger {
    val integer = when (value) {
        is BigInteger -> value
        is Byte -> BigInteger.valueOf(value.toLong())
        is Short -> BigInteger.valueOf(value.toLong())
        is Int -> BigInteger.valueOf(value.toLong())
        is Long -> BigInteger.valueOf(value)
        else -> error("$path must be an integer")
    }
    check(integer.signum() >= 0 && integer <= ALIAS_U64_MAX) {
        "$path must fit in unsigned 64-bit range"
    }
    return integer
}

internal fun aliasOptionalU64(value: Any?, path: String): BigInteger? =
    if (value == null) null else aliasU64(value, path)

internal fun requireExactText(value: String, field: String) {
    require(value.isNotBlank() && value == value.trim() && value.none { it.isISOControl() }) {
        "$field must be non-blank without surrounding whitespace or controls"
    }
}
