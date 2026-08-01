package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.text.Normalizer
import java.util.Collections
import java.util.LinkedHashMap

/** Shared base for exact first-release Musubi Norito-JSON values. */
abstract class MusubiWireValueV1 {
    internal abstract fun wireValue(): Any?

    /** Returns the canonical, sorted-key JSON representation used by Torii. */
    fun toJsonBytes(): ByteArray = MusubiJsonV1.encode(wireValue())

    final override fun equals(other: Any?): Boolean =
        other != null && javaClass == other.javaClass &&
            wireValue() == (other as MusubiWireValueV1).wireValue()

    final override fun hashCode(): Int = wireValue()?.hashCode() ?: 0

    final override fun toString(): String = String(toJsonBytes(), StandardCharsets.UTF_8)
}

/** Canonical human-facing namespace (`dataspace` or `domain.dataspace`). */
class MusubiNamespaceV1(@JvmField val value: String) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireNamespace(value)
    }

    override fun wireValue(): Any = listOf(value)
}

/** Canonical lowercase ASCII kebab package name. */
class MusubiPackageNameV1(@JvmField val value: String) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireAsciiKebab(value, 64, "package name")
    }

    override fun wireValue(): Any = listOf(value)
}

/** Structural package scope encoded with Norito's adjacent `kind`/`value` form. */
class MusubiPackageScopeV1 private constructor(
    @JvmField val kind: Kind,
    @JvmField val domain: String?,
) : MusubiWireValueV1() {
    enum class Kind { DATASPACE_ROOT, DOMAIN }

    init {
        when (kind) {
            Kind.DATASPACE_ROOT -> require(domain == null) {
                "dataspace-root Musubi scope must not carry a domain"
            }
            Kind.DOMAIN -> MusubiValidationV1.requireName(domain ?: "", "package scope domain")
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "kind" to if (kind == Kind.DATASPACE_ROOT) "DataspaceRoot" else "Domain",
        "value" to domain,
    )

    companion object {
        @JvmStatic fun dataspaceRoot(): MusubiPackageScopeV1 =
            MusubiPackageScopeV1(Kind.DATASPACE_ROOT, null)

        @JvmStatic fun domain(value: String): MusubiPackageScopeV1 =
            MusubiPackageScopeV1(Kind.DOMAIN, value)
    }
}

/** Stable structural identity stored in releases and lock graphs. */
class MusubiPackageIdV1(
    @JvmField val homeDataspace: BigInteger,
    @JvmField val scope: MusubiPackageScopeV1,
    @JvmField val name: MusubiPackageNameV1,
) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireU64(homeDataspace, "homeDataspace")
    }

    constructor(
        homeDataspace: Long,
        scope: MusubiPackageScopeV1,
        name: MusubiPackageNameV1,
    ) : this(BigInteger.valueOf(homeDataspace), scope, name)

    override fun wireValue(): Any = linkedMapOf(
        "home_dataspace" to homeDataspace,
        "scope" to scope.wireValue(),
        "name" to name.wireValue(),
    )
}

/** Public namespace/package selector used by ordered directory results. */
class MusubiPackageSelectorV1(
    @JvmField val namespace: MusubiNamespaceV1,
    @JvmField val name: MusubiPackageNameV1,
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "namespace" to namespace.wireValue(),
        "name" to name.wireValue(),
    )
}

/** One structured SemVer prerelease identifier. */
class MusubiPrereleaseIdentifierV1 private constructor(
    @JvmField val numeric: BigInteger?,
    @JvmField val alphaNumeric: String?,
) : MusubiWireValueV1(), Comparable<MusubiPrereleaseIdentifierV1> {
    init {
        require((numeric == null) != (alphaNumeric == null)) {
            "Musubi prerelease identifier must have exactly one representation"
        }
        numeric?.let { MusubiValidationV1.requireU64(it, "numeric prerelease identifier") }
        alphaNumeric?.let { text ->
            require(text.isNotEmpty() && text.toByteArray(StandardCharsets.UTF_8).size <= 64) {
                "Musubi prerelease identifier is out of bounds"
            }
            require(text.any { !it.isDigit() } && text.all { it.isLetterOrDigit() || it == '-' }) {
                "Musubi alphanumeric prerelease identifier is noncanonical"
            }
            require(text.all { it.code < 128 }) {
                "Musubi prerelease identifiers must be ASCII"
            }
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "kind" to if (numeric != null) "Numeric" else "AlphaNumeric",
        "value" to (numeric ?: alphaNumeric),
    )

    override fun compareTo(other: MusubiPrereleaseIdentifierV1): Int = when {
        numeric != null && other.numeric != null -> numeric.compareTo(other.numeric)
        numeric != null -> -1
        other.numeric != null -> 1
        else -> alphaNumeric!!.compareTo(other.alphaNumeric!!)
    }

    fun canonicalText(): String = (numeric ?: alphaNumeric).toString()

    companion object {
        @JvmStatic fun numeric(value: BigInteger): MusubiPrereleaseIdentifierV1 =
            MusubiPrereleaseIdentifierV1(value, null)

        @JvmStatic fun alphaNumeric(value: String): MusubiPrereleaseIdentifierV1 =
            MusubiPrereleaseIdentifierV1(null, value)

        internal fun parse(value: String): MusubiPrereleaseIdentifierV1 {
            require(value.isNotEmpty()) { "Musubi prerelease identifier must not be empty" }
            if (value.all { it.isDigit() }) {
                require(value.length == 1 || value[0] != '0') {
                    "Musubi numeric prerelease identifiers must not have leading zeroes"
                }
                return numeric(MusubiValidationV1.parseU64(value, "prerelease identifier"))
            }
            return alphaNumeric(value)
        }
    }
}

/** Structured canonical SemVer. Build metadata is deliberately unsupported. */
class MusubiVersionV1(
    @JvmField val major: BigInteger,
    @JvmField val minor: BigInteger,
    @JvmField val patch: BigInteger,
    prerelease: List<MusubiPrereleaseIdentifierV1> = emptyList(),
) : MusubiWireValueV1(), Comparable<MusubiVersionV1> {
    @JvmField val prerelease: List<MusubiPrereleaseIdentifierV1> = prerelease.toList()

    init {
        MusubiValidationV1.requireU64(major, "version.major")
        MusubiValidationV1.requireU64(minor, "version.minor")
        MusubiValidationV1.requireU64(patch, "version.patch")
        require(this.prerelease.size <= 16) {
            "Musubi version has too many prerelease identifiers"
        }
    }

    constructor(
        major: Long,
        minor: Long,
        patch: Long,
        prerelease: List<MusubiPrereleaseIdentifierV1> = emptyList(),
    ) : this(
        BigInteger.valueOf(major),
        BigInteger.valueOf(minor),
        BigInteger.valueOf(patch),
        prerelease,
    )

    override fun wireValue(): Any = linkedMapOf(
        "major" to major,
        "minor" to minor,
        "patch" to patch,
        "prerelease" to prerelease.map { it.wireValue() },
    )

    override fun compareTo(other: MusubiVersionV1): Int {
        major.compareTo(other.major).takeIf { it != 0 }?.let { return it }
        minor.compareTo(other.minor).takeIf { it != 0 }?.let { return it }
        patch.compareTo(other.patch).takeIf { it != 0 }?.let { return it }
        if (prerelease.isEmpty() && other.prerelease.isNotEmpty()) return 1
        if (prerelease.isNotEmpty() && other.prerelease.isEmpty()) return -1
        for (index in 0 until minOf(prerelease.size, other.prerelease.size)) {
            prerelease[index].compareTo(other.prerelease[index]).takeIf { it != 0 }?.let {
                return it
            }
        }
        return prerelease.size.compareTo(other.prerelease.size)
    }

    fun canonicalText(): String = buildString {
        append(major).append('.').append(minor).append('.').append(patch)
        if (prerelease.isNotEmpty()) {
            append('-').append(prerelease.joinToString(".") { it.canonicalText() })
        }
    }

    companion object {
        @JvmStatic
        fun parse(value: String): MusubiVersionV1 {
            MusubiValidationV1.requireExactText(value, "Musubi version")
            require('+' !in value) { "Musubi V1 versions do not permit build metadata" }
            val split = value.split('-', limit = 2)
            val core = split[0].split('.')
            require(core.size == 3) { "Musubi version must use MAJOR.MINOR.PATCH" }
            val identifiers = if (split.size == 1) {
                emptyList()
            } else {
                require(split[1].isNotEmpty()) { "Musubi prerelease must not be empty" }
                split[1].split('.').map(MusubiPrereleaseIdentifierV1::parse)
            }
            return MusubiVersionV1(
                MusubiValidationV1.parseCanonicalU64(core[0], "version.major"),
                MusubiValidationV1.parseCanonicalU64(core[1], "version.minor"),
                MusubiValidationV1.parseCanonicalU64(core[2], "version.patch"),
                identifiers,
            )
        }
    }
}

/** Comparator operator in a canonical comma-separated requirement. */
enum class MusubiComparatorOpV1(@JvmField val wireName: String, @JvmField val token: String) {
    GREATER("Greater", ">"),
    GREATER_OR_EQUAL("GreaterOrEqual", ">="),
    LESS("Less", "<"),
    LESS_OR_EQUAL("LessOrEqual", "<="),
    EQUAL("Equal", "=");

    internal fun wireValue(): Any = linkedMapOf("kind" to wireName, "value" to null)
}

/** One exact comparator node. */
class MusubiVersionComparatorV1(
    @JvmField val op: MusubiComparatorOpV1,
    @JvmField val version: MusubiVersionV1,
) : MusubiWireValueV1(), Comparable<MusubiVersionComparatorV1> {
    override fun wireValue(): Any = linkedMapOf(
        "op" to op.wireValue(),
        "version" to version.wireValue(),
    )

    override fun compareTo(other: MusubiVersionComparatorV1): Int =
        op.ordinal.compareTo(other.op.ordinal).takeIf { it != 0 }
            ?: version.compareTo(other.version)
}

/** Canonical Cargo-style requirement AST used by published dependencies. */
class MusubiVersionReqV1 private constructor(
    @JvmField val kind: Kind,
    @JvmField val version: MusubiVersionV1?,
    @JvmField val major: BigInteger?,
    @JvmField val minor: BigInteger?,
    comparators: List<MusubiVersionComparatorV1>,
) : MusubiWireValueV1() {
    enum class Kind { ANY, CARET, TILDE, MAJOR_WILDCARD, MINOR_WILDCARD, EXACT, COMPARATORS }

    @JvmField val comparators: List<MusubiVersionComparatorV1> = comparators.toList()

    init {
        require(this.comparators.size <= 16) { "Musubi requirement has too many comparators" }
        if (kind == Kind.COMPARATORS) {
            require(this.comparators.isNotEmpty()) { "Musubi comparator list must not be empty" }
            require(this.comparators.zipWithNext().all { (left, right) -> left < right }) {
                "Musubi comparator list must be sorted and distinct"
            }
        }
    }

    override fun wireValue(): Any {
        val wireName = when (kind) {
            Kind.ANY -> "Any"
            Kind.CARET -> "Caret"
            Kind.TILDE -> "Tilde"
            Kind.MAJOR_WILDCARD -> "MajorWildcard"
            Kind.MINOR_WILDCARD -> "MinorWildcard"
            Kind.EXACT -> "Exact"
            Kind.COMPARATORS -> "Comparators"
        }
        val payload: Any? = when (kind) {
            Kind.ANY -> null
            Kind.CARET, Kind.TILDE, Kind.EXACT -> version!!.wireValue()
            Kind.MAJOR_WILDCARD -> major
            Kind.MINOR_WILDCARD -> linkedMapOf("major" to major, "minor" to minor)
            Kind.COMPARATORS -> comparators.map { it.wireValue() }
        }
        return linkedMapOf("kind" to wireName, "value" to payload)
    }

    fun canonicalText(): String = when (kind) {
        Kind.ANY -> "*"
        Kind.CARET -> "^${version!!.canonicalText()}"
        Kind.TILDE -> "~${version!!.canonicalText()}"
        Kind.MAJOR_WILDCARD -> "$major.*"
        Kind.MINOR_WILDCARD -> "$major.$minor.*"
        Kind.EXACT -> "=${version!!.canonicalText()}"
        Kind.COMPARATORS -> comparators.joinToString(",") { it.op.token + it.version.canonicalText() }
    }

    companion object {
        @JvmStatic
        fun parse(value: String): MusubiVersionReqV1 {
            val raw = value.trim()
            require(raw == value && raw.isNotEmpty()) {
                "Musubi version requirement must be exact non-empty text"
            }
            if (raw == "*") return MusubiVersionReqV1(Kind.ANY, null, null, null, emptyList())
            if (raw.startsWith("=") && ',' !in raw) {
                return withVersion(Kind.EXACT, MusubiVersionV1.parse(raw.substring(1)))
            }
            if (',' in raw || raw.startsWith(">") || raw.startsWith("<")) {
                val values = raw.split(',').map { parseComparator(it.trim()) }.distinct().sorted()
                return MusubiVersionReqV1(Kind.COMPARATORS, null, null, null, values)
            }
            if (raw.startsWith("^")) return withVersion(Kind.CARET, MusubiVersionV1.parse(raw.substring(1)))
            if (raw.startsWith("~")) return withVersion(Kind.TILDE, MusubiVersionV1.parse(raw.substring(1)))
            if (raw.endsWith(".*")) {
                val components = raw.dropLast(2).split('.')
                return when (components.size) {
                    1 -> MusubiVersionReqV1(
                        Kind.MAJOR_WILDCARD,
                        null,
                        MusubiValidationV1.parseCanonicalU64(components[0], "wildcard major"),
                        null,
                        emptyList(),
                    )
                    2 -> MusubiVersionReqV1(
                        Kind.MINOR_WILDCARD,
                        null,
                        MusubiValidationV1.parseCanonicalU64(components[0], "wildcard major"),
                        MusubiValidationV1.parseCanonicalU64(components[1], "wildcard minor"),
                        emptyList(),
                    )
                    else -> throw IllegalArgumentException(
                        "Musubi wildcard must be MAJOR.* or MAJOR.MINOR.*",
                    )
                }
            }
            return withVersion(Kind.CARET, MusubiVersionV1.parse(raw))
        }

        internal fun fromWire(
            kind: Kind,
            version: MusubiVersionV1? = null,
            major: BigInteger? = null,
            minor: BigInteger? = null,
            comparators: List<MusubiVersionComparatorV1> = emptyList(),
        ): MusubiVersionReqV1 = MusubiVersionReqV1(kind, version, major, minor, comparators)

        private fun withVersion(kind: Kind, version: MusubiVersionV1) =
            MusubiVersionReqV1(kind, version, null, null, emptyList())

        private fun parseComparator(value: String): MusubiVersionComparatorV1 {
            val pair = when {
                value.startsWith(">=") -> MusubiComparatorOpV1.GREATER_OR_EQUAL to value.substring(2)
                value.startsWith("<=") -> MusubiComparatorOpV1.LESS_OR_EQUAL to value.substring(2)
                value.startsWith(">") -> MusubiComparatorOpV1.GREATER to value.substring(1)
                value.startsWith("<") -> MusubiComparatorOpV1.LESS to value.substring(1)
                value.startsWith("=") -> MusubiComparatorOpV1.EQUAL to value.substring(1)
                else -> throw IllegalArgumentException("Musubi comparator has no supported operator")
            }
            return MusubiVersionComparatorV1(pair.first, MusubiVersionV1.parse(pair.second))
        }
    }
}

/** Exact release identity. */
class MusubiReleaseIdV1(
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val version: MusubiVersionV1,
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "package" to packageId.wireValue(),
        "version" to version.wireValue(),
    )
}

/** Canonical Norito JSON wrapper for any Musubi 32-byte digest newtype. */
class MusubiDigest32V1(bytes: ByteArray) : MusubiWireValueV1() {
    private val value: ByteArray = bytes.copyOf()

    init {
        require(value.size == 32) { "Musubi digest must contain exactly 32 bytes" }
    }

    fun bytes(): ByteArray = value.copyOf()

    override fun wireValue(): Any = listOf(value.map { it.toInt() and 0xff })

    companion object {
        @JvmStatic fun fromBytes(bytes: ByteArray): MusubiDigest32V1 = MusubiDigest32V1(bytes)

        @JvmStatic fun fromHex(hex: String): MusubiDigest32V1 {
            val body = hex.removePrefix("0x").removePrefix("0X")
            require(body.length == 64 && body.all { it in "0123456789abcdefABCDEF" }) {
                "Musubi digest must be 64 hexadecimal characters"
            }
            return MusubiDigest32V1(ByteArray(32) { index ->
                body.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            })
        }
    }
}

/** Finalized universal registry snapshot. */
class MusubiRegistrySnapshotV1(
    @JvmField val finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    @JvmField val indexRevision: BigInteger,
) : MusubiWireValueV1() {
    private val hash = finalizedBlockHash.copyOf()

    init {
        MusubiValidationV1.requireU64(finalizedHeight, "snapshot.finalizedHeight")
        require(finalizedHeight > BigInteger.ZERO) { "Musubi snapshot height must be non-zero" }
        require(hash.size == 32) { "Musubi finalized block hash must contain 32 bytes" }
        MusubiValidationV1.requireU64(indexRevision, "snapshot.indexRevision")
        require(indexRevision > BigInteger.ZERO) { "Musubi index revision must be non-zero" }
    }

    fun finalizedBlockHash(): ByteArray = hash.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "finalized_height" to finalizedHeight,
        "finalized_block_hash" to hash.map { it.toInt() and 0xff },
        "index_revision" to indexRevision,
    )
}

/** Cursor bound to a finalized snapshot, exact query hash, last key, and optional caller. */
class MusubiFinalizedCursorV1(
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
    @JvmField val queryHash: MusubiDigest32V1,
    @JvmField val lastKey: String,
    @JvmField val caller: String?,
) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireExactText(lastKey, "Musubi cursor last key")
        require(lastKey.toByteArray(StandardCharsets.UTF_8).size <= 512) {
            "Musubi cursor last key exceeds 512 UTF-8 bytes"
        }
        caller?.let { MusubiValidationV1.requireExactText(it, "Musubi cursor caller") }
    }

    override fun wireValue(): Any = linkedMapOf(
        "snapshot" to snapshot.wireValue(),
        "query_hash" to queryHash.wireValue(),
        "last_key" to lastKey,
        "caller" to caller,
    )
}

/** Shared bounded page request. A zero limit selects the registry default of 50. */
class MusubiPageRequestV1(
    @JvmField val limit: Long = 50,
    @JvmField val cursor: MusubiFinalizedCursorV1? = null,
) : MusubiWireValueV1() {
    init {
        require(limit in 0..4_294_967_295L) { "Musubi page limit must fit u32" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "limit" to limit,
        "cursor" to cursor?.wireValue(),
    )
}

/** Exact structural package query. */
class MusubiExactPackageQueryV1(@JvmField val packageId: MusubiPackageIdV1) :
    MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf("package" to packageId.wireValue())
}

/** Exact structural release query. */
class MusubiExactReleaseQueryV1(@JvmField val release: MusubiReleaseIdV1) :
    MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf("release" to release.wireValue())
}

/** Universal sparse resolver-index query. */
class MusubiResolverIndexQueryV1(
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val requirement: MusubiVersionReqV1?,
    @JvmField val page: MusubiPageRequestV1 = MusubiPageRequestV1(),
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "package" to packageId.wireValue(),
        "requirement" to requirement?.wireValue(),
        "page" to page.wireValue(),
    )
}

/** Package-scoped versions or maintainers page query. */
class MusubiPackagePageQueryV1(
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val page: MusubiPageRequestV1 = MusubiPageRequestV1(),
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "package" to packageId.wireValue(),
        "page" to page.wireValue(),
    )
}

/** Archive-location page query. */
class MusubiArchiveLocationQueryV1(
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val page: MusubiPageRequestV1 = MusubiPageRequestV1(),
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "archive_id" to archiveId.wireValue(),
        "page" to page.wireValue(),
    )
}

/** Exact alias or alias-history query. */
class MusubiAliasQueryV1(
    alias: String,
    @JvmField val page: MusubiPageRequestV1 = MusubiPageRequestV1(),
) : MusubiWireValueV1() {
    @JvmField val alias: String = alias

    init {
        MusubiValidationV1.requireAsciiKebab(alias, 32, "alias")
    }

    override fun wireValue(): Any = linkedMapOf(
        "alias" to listOf(alias),
        "page" to page.wireValue(),
    )
}

/** Deterministic ordered-prefix query. */
class MusubiOrderedPrefixQueryV1(
    prefix: String,
    @JvmField val page: MusubiPageRequestV1 = MusubiPageRequestV1(),
) : MusubiWireValueV1() {
    @JvmField val prefix: String = prefix

    init {
        MusubiValidationV1.requireExactText(prefix, "Musubi ordered prefix")
        require(prefix.toByteArray(StandardCharsets.UTF_8).size <= 512) {
            "Musubi ordered prefix exceeds 512 UTF-8 bytes"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "prefix" to listOf(prefix),
        "page" to page.wireValue(),
    )
}

/** Compare-and-set revisions carried by an exact package record. */
class MusubiPackageRevisionsV1(
    @JvmField val governance: BigInteger,
    @JvmField val metadata: BigInteger,
    @JvmField val archiveLocations: BigInteger,
) : MusubiWireValueV1() {
    init {
        listOf(governance, metadata, archiveLocations).forEach {
            MusubiValidationV1.requireU64(it, "package revision")
            require(it > BigInteger.ZERO) { "Musubi package revisions must be non-zero" }
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "governance" to governance,
        "metadata" to metadata,
        "archive_locations" to archiveLocations,
    )
}

/** Exact authoritative package response. */
class MusubiPackageRecordV1(
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val claimedNamespace: MusubiNamespaceV1,
    @JvmField val claimedNamespaceBinding: MusubiDigest32V1,
    owners: List<String>,
    memberAccounts: List<String>,
    @JvmField val claimedAtHeight: BigInteger,
    @JvmField val revisions: MusubiPackageRevisionsV1,
) : MusubiWireValueV1() {
    @JvmField val owners: List<String> = owners.toList()
    @JvmField val memberAccounts: List<String> = memberAccounts.toList()

    init {
        require(this.owners.isNotEmpty()) { "Musubi package must retain at least one owner" }
        this.owners.forEach { MusubiValidationV1.requireExactText(it, "package owner") }
        this.memberAccounts.forEach { MusubiValidationV1.requireExactText(it, "package member") }
        MusubiValidationV1.requireU64(claimedAtHeight, "claimedAtHeight")
        require(claimedAtHeight > BigInteger.ZERO) { "Musubi claim height must be non-zero" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "package" to packageId.wireValue(),
        "claimed_namespace" to claimedNamespace.wireValue(),
        "claimed_namespace_binding" to claimedNamespaceBinding.wireValue(),
        "owners" to owners,
        "member_accounts" to memberAccounts,
        "claimed_at_height" to claimedAtHeight,
        "revisions" to revisions.wireValue(),
    )
}

/** Exact release response with a fully validated release identity and strict wire document. */
class MusubiReleaseRecordV1 internal constructor(
    @JvmField val release: MusubiReleaseIdV1,
    @JvmField val publishedBy: String,
    @JvmField val publishedAtHeight: BigInteger,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    private val rawValue = MusubiJsonV1.immutableObject(raw)
    override fun wireValue(): Any = rawValue
}

/** Compact resolver row response with strict fields and typed release identity. */
class MusubiResolverReleaseRowV1 internal constructor(
    @JvmField val release: MusubiReleaseIdV1,
    @JvmField val indexRevision: BigInteger,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    private val rawValue = MusubiJsonV1.immutableObject(raw)
    override fun wireValue(): Any = rawValue
}

/** Accepted package member response. */
class MusubiPackageMemberV1 internal constructor(
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val account: String,
    @JvmField val roleKind: String,
    @JvmField val acceptedAtHeight: BigInteger,
    @JvmField val governanceRevision: BigInteger,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    private val rawValue = MusubiJsonV1.immutableObject(raw)
    override fun wireValue(): Any = rawValue
}

/** Renewable archive-location response with strict outer fields. */
class MusubiArchiveLocationV1 internal constructor(
    @JvmField val locationId: MusubiDigest32V1,
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val revision: BigInteger,
    @JvmField val stateKind: String,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    private val rawValue = MusubiJsonV1.immutableObject(raw)
    override fun wireValue(): Any = rawValue
}

/** Permanent global alias response. */
class MusubiAliasRecordV1(
    @JvmField val alias: String,
    @JvmField val target: MusubiPackageIdV1,
    @JvmField val registeredBy: String,
    @JvmField val pricingRevision: BigInteger,
    @JvmField val paidXor: BigInteger,
    @JvmField val registeredAtHeight: BigInteger,
    @JvmField val historyRevision: BigInteger,
) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireAsciiKebab(alias, 32, "alias")
    }

    override fun wireValue(): Any = linkedMapOf(
        "alias" to listOf(alias),
        "target" to target.wireValue(),
        "registered_by" to registeredBy,
        "pricing_revision" to pricingRevision,
        "paid_xor" to paidXor,
        "registered_at_height" to registeredAtHeight,
        "history_revision" to historyRevision,
    )
}

/** Immutable alias-history entry. */
class MusubiAliasHistoryEntryV1 internal constructor(
    @JvmField val alias: String,
    @JvmField val revision: BigInteger,
    @JvmField val actionKind: String,
    @JvmField val previousTarget: MusubiPackageIdV1?,
    @JvmField val target: MusubiPackageIdV1,
    @JvmField val governanceAction: MusubiDigest32V1?,
    @JvmField val finalizedHeight: BigInteger,
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "alias" to listOf(alias),
        "revision" to revision,
        "action" to linkedMapOf("kind" to actionKind, "value" to null),
        "previous_target" to previousTarget?.wireValue(),
        "target" to target.wireValue(),
        "governance_action" to governanceAction?.wireValue(),
        "finalized_height" to finalizedHeight,
    )
}

/** Ordered public directory entry. */
class MusubiOrderedPackageEntryV1(
    @JvmField val selector: MusubiPackageSelectorV1,
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val latestSelectable: MusubiVersionV1?,
    @JvmField val metadataRevision: BigInteger,
    @JvmField val indexRevision: BigInteger,
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "selector" to selector.wireValue(),
        "package" to packageId.wireValue(),
        "latest_selectable" to latestSelectable?.wireValue(),
        "metadata_revision" to metadataRevision,
        "index_revision" to indexRevision,
    )
}

/** Typed bounded page shared by all finalized Musubi list responses. */
class MusubiPageV1<T : MusubiWireValueV1> internal constructor(
    items: List<T>,
    @JvmField val nextCursor: MusubiFinalizedCursorV1?,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
) : MusubiWireValueV1() {
    @JvmField val items: List<T> = items.toList()

    init {
        require(this.items.size <= 100) { "Musubi response page exceeds 100 items" }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi page cursor must use the page snapshot"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

/** Resolver page carrying the exact chain/genesis identity required by lockfiles. */
class MusubiResolverIndexPageV1 internal constructor(
    @JvmField val chainId: String,
    genesisHash: ByteArray,
    items: List<MusubiResolverReleaseRowV1>,
    @JvmField val nextCursor: MusubiFinalizedCursorV1?,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
) : MusubiWireValueV1() {
    private val genesisHashValue = genesisHash.copyOf()
    @JvmField val items: List<MusubiResolverReleaseRowV1> = items.toList()

    init {
        MusubiValidationV1.requireExactText(chainId, "Musubi resolver chain ID")
        require(genesisHashValue.size == 32) { "Musubi genesis hash must contain 32 bytes" }
        require(this.items.size <= 100) { "Musubi resolver page exceeds 100 items" }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi resolver cursor must use the page snapshot"
        }
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

/** Ordered-directory page carrying the exact chain/genesis identity for lock creation. */
class MusubiOrderedPrefixPageV1 internal constructor(
    @JvmField val chainId: String,
    genesisHash: ByteArray,
    items: List<MusubiOrderedPackageEntryV1>,
    @JvmField val nextCursor: MusubiFinalizedCursorV1?,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
) : MusubiWireValueV1() {
    private val genesisHashValue = genesisHash.copyOf()
    @JvmField val items: List<MusubiOrderedPackageEntryV1> = items.toList()

    init {
        MusubiValidationV1.requireExactText(chainId, "Musubi directory chain ID")
        require(genesisHashValue.size == 32) { "Musubi genesis hash must contain 32 bytes" }
        require(this.items.size <= 100) { "Musubi ordered-prefix page exceeds 100 items" }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi ordered-prefix cursor must use the page snapshot"
        }
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

internal object MusubiValidationV1 {
    private val U64_MAX = BigInteger("18446744073709551615")

    fun requireU64(value: BigInteger, field: String) {
        require(value >= BigInteger.ZERO && value <= U64_MAX) { "$field must fit u64" }
    }

    fun parseU64(value: String, field: String): BigInteger {
        require(value.isNotEmpty() && value.all { it in '0'..'9' }) { "$field must be numeric" }
        return try {
            BigInteger(value).also { requireU64(it, field) }
        } catch (error: NumberFormatException) {
            throw IllegalArgumentException("$field must fit u64", error)
        }
    }

    fun parseCanonicalU64(value: String, field: String): BigInteger {
        require(value.length == 1 || !value.startsWith('0')) { "$field has a leading zero" }
        return parseU64(value, field)
    }

    fun requireExactText(value: String, field: String) {
        require(value.isNotEmpty() && value == value.trim() && value.none { it.isISOControl() }) {
            "$field must be exact non-empty text"
        }
    }

    fun requireAsciiKebab(value: String, maximum: Int, field: String) {
        requireExactText(value, "Musubi $field")
        require(
            value.length <= maximum &&
                !value.startsWith('-') && !value.endsWith('-') && "--" !in value &&
                value.all { it in 'a'..'z' || it in '0'..'9' || it == '-' },
        ) { "Musubi $field must be lowercase ASCII kebab text" }
    }

    fun requireName(value: String, field: String) {
        requireExactText(value, field)
        require(value.toByteArray(StandardCharsets.UTF_8).size <= 255) { "$field exceeds 255 bytes" }
        require(value.none { it.isWhitespace() || it == '@' || it == '#' || it == '$' }) {
            "$field contains a forbidden character"
        }
        require(Normalizer.normalize(value, Normalizer.Form.NFC) == value) {
            "$field must be NFC-normalized"
        }
    }

    fun requireNamespace(value: String) {
        requireExactText(value, "Musubi namespace")
        require(value.toByteArray(StandardCharsets.UTF_8).size <= 255) {
            "Musubi namespace exceeds 255 bytes"
        }
        require(value.none { it == '/' || it == '@' || it == ':' }) {
            "Musubi namespace contains a reserved character"
        }
        val segments = value.split('.')
        require(segments.size in 1..2) {
            "Musubi namespace must be dataspace or domain.dataspace"
        }
        segments.forEach { requireName(it, "Musubi namespace segment") }
    }
}
