package org.hyperledger.iroha.sdk.alias

import java.math.BigInteger
import java.net.IDN
import java.text.Normalizer
import java.util.Locale
import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Base for immutable alias planner values with structural equality. */
abstract class AliasJsonValue {
    /** Returns the Norito-JSON-compatible object shape for this value. */
    abstract fun toJsonMap(): Map<String, Any?>

    final override fun equals(other: Any?): Boolean =
        other != null && javaClass == other.javaClass &&
            toJsonMap() == (other as AliasJsonValue).toJsonMap()

    final override fun hashCode(): Int = toJsonMap().hashCode()

    override fun toString(): String = JsonEncoder.encode(toJsonMap())
}

/**
 * Catalog-free textual account alias.
 *
 * `merchant@banka.paynet` has a domain while `merchant@paynet` is rooted directly in a
 * dataspace. Construction applies the same segment canonicalization as parsing.
 */
class AccountAliasName(
    label: String,
    domain: String?,
    dataspace: String,
) : AliasJsonValue() {
    /** Canonical alias label. */
    @JvmField
    val label: String = AliasNameCanonicalizer.segment(label, "label")

    /** Optional canonical domain label. */
    @JvmField
    val domain: String? = domain?.let { AliasNameCanonicalizer.segment(it, "domain") }

    /** Canonical textual dataspace name. */
    @JvmField
    val dataspace: String = AliasNameCanonicalizer.segment(dataspace, "dataspace")

    /** Returns the canonical external alias literal. */
    fun canonicalText(): String = buildString {
        append(label)
        append('@')
        if (domain != null) {
            append(domain)
            append('.')
        }
        append(dataspace)
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "label" to label,
        "domain" to domain,
        "dataspace" to dataspace,
    )

    override fun toString(): String = canonicalText()

    companion object {
        /** Parses `label@domain.dataspace` or `label@dataspace` without consulting a catalog. */
        @JvmStatic
        fun parse(literal: String): AccountAliasName {
            require(literal.isNotEmpty()) { "account alias must not be empty" }
            require(literal == literal.trim()) {
                "account alias must not contain leading or trailing whitespace"
            }
            require(literal.none { it.isISOControl() }) {
                "account alias must not contain control characters"
            }
            val firstAt = literal.indexOf('@')
            require(firstAt > 0 && firstAt == literal.lastIndexOf('@')) {
                "account alias must contain exactly one '@' separator"
            }
            require(firstAt < literal.length - 1) {
                "account alias dataspace segment must not be empty"
            }
            val label = literal.substring(0, firstAt)
            val scope = literal.substring(firstAt + 1)
            val firstDot = scope.indexOf('.')
            return if (firstDot < 0) {
                AccountAliasName(label, null, scope)
            } else {
                require(firstDot > 0 && firstDot == scope.lastIndexOf('.') && firstDot < scope.length - 1) {
                    "account alias must contain one non-empty domain before the dataspace"
                }
                AccountAliasName(
                    label,
                    scope.substring(0, firstDot),
                    scope.substring(firstDot + 1),
                )
            }
        }
    }
}

/** Canonical dataspace text paired with the numeric ID expected by the caller. */
class ResolvedDataSpaceV1(
    canonicalName: String,
    dataspaceId: BigInteger,
) : AliasJsonValue() {
    /** Canonical textual dataspace name. */
    @JvmField
    val canonicalName: String = AliasNameCanonicalizer.segment(canonicalName, "canonicalName")

    /** Unsigned 64-bit dataspace identifier pinned by the plan. */
    @JvmField
    val dataspaceId: BigInteger = requireU64(dataspaceId, "dataspaceId")

    /** Convenience overload for non-negative signed identifiers. */
    constructor(canonicalName: String, dataspaceId: Long) : this(
        canonicalName,
        BigInteger.valueOf(dataspaceId).also {
            require(dataspaceId >= 0) { "dataspaceId must be an unsigned 64-bit integer" }
        },
    )

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "canonical_name" to canonicalName,
        "dataspace_id" to dataspaceId,
    )

    override fun toString(): String = canonicalName
}

/** Canonical `domain.dataspace` text paired with the expected numeric dataspace ID. */
class ResolvedDomainV1(
    canonicalName: String,
    dataspaceId: BigInteger,
) : AliasJsonValue() {
    /** Canonical fully-qualified domain. */
    @JvmField
    val canonicalName: String = AliasNameCanonicalizer.qualifiedDomain(canonicalName)

    /** Unsigned 64-bit parent dataspace identifier pinned by the plan. */
    @JvmField
    val dataspaceId: BigInteger = requireU64(dataspaceId, "dataspaceId")

    /** Convenience overload for non-negative signed identifiers. */
    constructor(canonicalName: String, dataspaceId: Long) : this(
        canonicalName,
        BigInteger.valueOf(dataspaceId).also {
            require(dataspaceId >= 0) { "dataspaceId must be an unsigned 64-bit integer" }
        },
    )

    /** Returns the resolved parent dataspace. */
    fun parentDataspace(): ResolvedDataSpaceV1 =
        ResolvedDataSpaceV1(canonicalName.substringAfter('.'), dataspaceId)

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "canonical_name" to canonicalName,
        "dataspace_id" to dataspaceId,
    )

    override fun toString(): String = canonicalName
}

/** Canonical account-alias text paired with the expected numeric dataspace ID. */
class ResolvedAccountAliasV1(
    /** Canonical catalog-free alias name. */
    @JvmField val canonicalName: AccountAliasName,
    dataspaceId: BigInteger,
) : AliasJsonValue() {
    /** Unsigned 64-bit parent dataspace identifier pinned by the plan. */
    @JvmField
    val dataspaceId: BigInteger = requireU64(dataspaceId, "dataspaceId")

    /** Convenience overload for non-negative signed identifiers. */
    constructor(canonicalName: AccountAliasName, dataspaceId: Long) : this(
        canonicalName,
        BigInteger.valueOf(dataspaceId).also {
            require(dataspaceId >= 0) { "dataspaceId must be an unsigned 64-bit integer" }
        },
    )

    /** Parses the external name and pins it to `dataspaceId`. */
    constructor(canonicalName: String, dataspaceId: BigInteger) : this(
        AccountAliasName.parse(canonicalName),
        dataspaceId,
    )

    /** Returns the optional resolved domain parent. */
    fun parentDomain(): ResolvedDomainV1? = canonicalName.domain?.let {
        ResolvedDomainV1("$it.${canonicalName.dataspace}", dataspaceId)
    }

    override fun toJsonMap(): Map<String, Any?> = linkedMapOf(
        "canonical_name" to canonicalName.toJsonMap(),
        "dataspace_id" to dataspaceId,
    )

    override fun toString(): String = canonicalName.canonicalText()
}

internal object AliasNameCanonicalizer {
    fun segment(raw: String, field: String): String {
        require(raw.isNotEmpty()) { "$field must not be empty" }
        require(raw == raw.trim()) { "$field must not contain surrounding whitespace" }
        require(
            raw.none {
                it.isWhitespace() || it.isISOControl() ||
                    it == '@' || it == '#' || it == '$' || it == '.'
            },
        ) {
            "$field is not a valid alias name segment"
        }
        val normalized = Normalizer.normalize(raw, Normalizer.Form.NFC)
        require(normalized.none { it.code in 0x1E00..0x1EFF }) {
            "$field is not a supported alias name segment"
        }
        val ascii = try {
            IDN.toASCII(normalized, IDN.ALLOW_UNASSIGNED)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field is not a valid alias name segment", ex)
        }.lowercase(Locale.ROOT)
        require(ascii.isNotEmpty() && ascii.all { it.isLetterOrDigit() || it == '-' || it == '_' }) {
            "$field is not a valid alias name segment"
        }
        require(!ascii.startsWith('-') && !ascii.endsWith('-')) {
            "$field is not a valid alias name segment"
        }
        return ascii
    }

    fun qualifiedDomain(raw: String): String {
        require(raw == raw.trim()) { "canonicalName must not contain surrounding whitespace" }
        val dot = raw.indexOf('.')
        require(dot > 0 && dot == raw.lastIndexOf('.') && dot < raw.length - 1) {
            "canonicalName must use domain.dataspace format"
        }
        return "${segment(raw.substring(0, dot), "domain")}.${segment(raw.substring(dot + 1), "dataspace")}"
    }
}

private val U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

internal fun requireU64(value: BigInteger, field: String): BigInteger {
    require(value.signum() >= 0 && value <= U64_MAX) { "$field must be an unsigned 64-bit integer" }
    return value
}
