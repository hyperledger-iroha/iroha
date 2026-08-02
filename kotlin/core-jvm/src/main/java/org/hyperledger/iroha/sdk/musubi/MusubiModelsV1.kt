package org.hyperledger.iroha.sdk.musubi

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.text.Normalizer
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.compactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder

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

/** Canonical permanent global alias name. */
class MusubiAliasNameV1(@JvmField val value: String) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireAsciiKebab(value, 32, "alias")
    }

    override fun wireValue(): Any = listOf(value)
}

/** Bounded public reason attached to governance, yank, or takedown mutations. */
class MusubiReasonV1(@JvmField val value: String) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireExactText(value, "Musubi reason")
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        require(String(bytes, StandardCharsets.UTF_8) == value) {
            "Musubi reason must contain valid Unicode scalar values"
        }
        require(bytes.size <= 1_024) {
            "Musubi reason exceeds 1024 UTF-8 bytes"
        }
    }

    override fun wireValue(): Any = listOf(value)
}

/** Bounded human description committed by release or package metadata. */
class MusubiDescriptionV1(@JvmField val value: String) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireBoundedText(value, 4_096, "Musubi description")
    }

    override fun wireValue(): Any = listOf(value)
}

/** Bounded readme, license, or repository reference. */
class MusubiDocumentRefV1(@JvmField val value: String) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireBoundedText(value, 2_048, "Musubi document reference")
    }

    override fun wireValue(): Any = listOf(value)
}

/** Canonical lowercase ASCII keyword committed by Musubi metadata. */
class MusubiKeywordV1(@JvmField val value: String) : MusubiWireValueV1(),
    Comparable<MusubiKeywordV1> {
    init {
        MusubiValidationV1.requireAsciiKebab(value, 64, "keyword")
    }

    override fun wireValue(): Any = listOf(value)

    override fun compareTo(other: MusubiKeywordV1): Int =
        MusubiValidationV1.compareUtf8(value, other.value)
}

/** Complete immutable release metadata or mutable package-metadata replacement. */
class MusubiReleaseMetadataV1(
    @JvmField val description: MusubiDescriptionV1? = null,
    @JvmField val readme: MusubiDocumentRefV1? = null,
    @JvmField val license: MusubiDocumentRefV1? = null,
    @JvmField val repository: MusubiDocumentRefV1? = null,
    keywords: List<MusubiKeywordV1> = emptyList(),
) : MusubiWireValueV1() {
    @JvmField val keywords: List<MusubiKeywordV1> = keywords.toList()

    init {
        require(this.keywords.size <= 32) { "Musubi metadata exceeds 32 keywords" }
        require(this.keywords.zipWithNext().all { (left, right) -> left < right }) {
            "Musubi metadata keywords must be sorted and distinct"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "description" to description?.wireValue(),
        "readme" to readme?.wireValue(),
        "license" to license?.wireValue(),
        "repository" to repository?.wireValue(),
        "keywords" to keywords.map { it.wireValue() },
    )
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
) : MusubiWireValueV1(), Comparable<MusubiPackageIdV1> {
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

    override fun compareTo(other: MusubiPackageIdV1): Int =
        MusubiValidationV1.comparePackageIds(this, other)
}

/** Immutable public namespace binding used to authorize first publication. */
class MusubiNamespaceBindingV1(
    @JvmField val namespace: MusubiNamespaceV1,
    @JvmField val homeDataspace: BigInteger,
    @JvmField val scope: MusubiPackageScopeV1,
    @JvmField val generation: BigInteger,
) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireU64(homeDataspace, "namespaceBinding.homeDataspace")
        MusubiValidationV1.requireU64(generation, "namespaceBinding.generation")
        require(generation > BigInteger.ZERO) {
            "Musubi namespace binding generation must be non-zero"
        }
        val domain = namespace.value.substringBefore('.', missingDelimiterValue = "")
        require(
            (scope.kind == MusubiPackageScopeV1.Kind.DATASPACE_ROOT && domain.isEmpty()) ||
                (scope.kind == MusubiPackageScopeV1.Kind.DOMAIN && scope.domain == domain),
        ) { "Musubi namespace binding text and scope disagree" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "namespace" to namespace.wireValue(),
        "home_dataspace" to homeDataspace,
        "scope" to scope.wireValue(),
        "generation" to generation,
    )
}

/** Independent capabilities granted to an accepted package maintainer. */
class MusubiMaintainerPermissionsV1(
    @JvmField val publish: Boolean,
    @JvmField val yank: Boolean,
    @JvmField val metadata: Boolean,
    @JvmField val archiveLocations: Boolean,
) : MusubiWireValueV1() {
    init {
        require(publish || yank || metadata || archiveLocations) {
            "Musubi maintainer role must grant at least one permission"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "publish" to publish,
        "yank" to yank,
        "metadata" to metadata,
        "archive_locations" to archiveLocations,
    )
}

/** Owner or explicitly permissioned maintainer role for one package. */
class MusubiPackageRoleV1 private constructor(
    @JvmField val kind: Kind,
    @JvmField val permissions: MusubiMaintainerPermissionsV1?,
) : MusubiWireValueV1() {
    /** Stable first-release role variant. */
    enum class Kind { OWNER, MAINTAINER }

    init {
        when (kind) {
            Kind.OWNER -> require(permissions == null) {
                "Musubi owner role must not carry maintainer permissions"
            }
            Kind.MAINTAINER -> require(permissions != null) {
                "Musubi maintainer role must carry permissions"
            }
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "kind" to if (kind == Kind.OWNER) "Owner" else "Maintainer",
        "value" to permissions?.wireValue(),
    )

    companion object {
        private val OWNER = MusubiPackageRoleV1(Kind.OWNER, null)

        /** Construct the package-owner role. */
        @JvmStatic fun owner(): MusubiPackageRoleV1 = OWNER

        /** Construct a maintainer role with at least one independent permission. */
        @JvmStatic fun maintainer(
            permissions: MusubiMaintainerPermissionsV1,
        ): MusubiPackageRoleV1 = MusubiPackageRoleV1(Kind.MAINTAINER, permissions)
    }
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
            require(
                value.isNotEmpty() &&
                    value.toByteArray(StandardCharsets.UTF_8).size <= 64 &&
                    value.all { it.code < 128 },
            ) { "Musubi prerelease identifier is out of bounds or non-ASCII" }
            if (value.all { it in '0'..'9' }) {
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
) : MusubiWireValueV1(), Comparable<MusubiVersionReqV1> {
    enum class Kind { ANY, CARET, TILDE, MAJOR_WILDCARD, MINOR_WILDCARD, EXACT, COMPARATORS }

    @JvmField val comparators: List<MusubiVersionComparatorV1> = comparators.toList()

    init {
        require(this.comparators.size <= 16) { "Musubi requirement has too many comparators" }
        if (kind == Kind.COMPARATORS) {
            require(this.comparators.isNotEmpty()) { "Musubi comparator list must not be empty" }
            require(this.comparators.zipWithNext().all { (left, right) -> left < right }) {
                "Musubi comparator list must be sorted and distinct"
            }
            require(
                this.comparators.size != 1 ||
                    this.comparators.single().op != MusubiComparatorOpV1.EQUAL,
            ) { "Musubi singleton equality comparator must use the exact requirement variant" }
            require(this.comparators.count { it.op == MusubiComparatorOpV1.EQUAL } <= 1) {
                "Musubi comparator list contains contradictory exact versions"
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

    /** Return whether a candidate satisfies this canonical Cargo-compatible requirement. */
    fun matches(candidate: MusubiVersionV1): Boolean {
        if (candidate.prerelease.isNotEmpty() && !prereleaseEligible(candidate)) return false
        return when (kind) {
            Kind.ANY -> true
            Kind.CARET -> candidate >= version!! && caretCoreIsCompatible(version, candidate)
            Kind.TILDE -> candidate >= version!! &&
                candidate.major == version.major && candidate.minor == version.minor
            Kind.MAJOR_WILDCARD -> candidate.major == major
            Kind.MINOR_WILDCARD -> candidate.major == major && candidate.minor == minor
            Kind.EXACT -> candidate == version
            Kind.COMPARATORS -> comparators.all { comparator ->
                val comparison = candidate.compareTo(comparator.version)
                when (comparator.op) {
                    MusubiComparatorOpV1.GREATER -> comparison > 0
                    MusubiComparatorOpV1.GREATER_OR_EQUAL -> comparison >= 0
                    MusubiComparatorOpV1.LESS -> comparison < 0
                    MusubiComparatorOpV1.LESS_OR_EQUAL -> comparison <= 0
                    MusubiComparatorOpV1.EQUAL -> comparison == 0
                }
            }
        }
    }

    override fun compareTo(other: MusubiVersionReqV1): Int {
        kind.compareTo(other.kind).let { if (it != 0) return it }
        version?.compareTo(requireNotNull(other.version))?.let { if (it != 0) return it }
        major?.compareTo(requireNotNull(other.major))?.let { if (it != 0) return it }
        minor?.compareTo(requireNotNull(other.minor))?.let { if (it != 0) return it }
        for (index in 0 until minOf(comparators.size, other.comparators.size)) {
            comparators[index].compareTo(other.comparators[index]).let { if (it != 0) return it }
        }
        return comparators.size.compareTo(other.comparators.size)
    }

    private fun prereleaseEligible(candidate: MusubiVersionV1): Boolean {
        fun explicitlyNamesCore(value: MusubiVersionV1): Boolean =
            value.prerelease.isNotEmpty() && value.major == candidate.major &&
                value.minor == candidate.minor && value.patch == candidate.patch
        return when (kind) {
            Kind.CARET, Kind.TILDE, Kind.EXACT -> explicitlyNamesCore(version!!)
            Kind.COMPARATORS -> comparators.any { explicitlyNamesCore(it.version) }
            Kind.ANY, Kind.MAJOR_WILDCARD, Kind.MINOR_WILDCARD -> false
        }
    }

    private fun caretCoreIsCompatible(
        base: MusubiVersionV1,
        candidate: MusubiVersionV1,
    ): Boolean = when {
        base.major > BigInteger.ZERO -> candidate.major == base.major
        base.minor > BigInteger.ZERO ->
            candidate.major == BigInteger.ZERO && candidate.minor == base.minor
        else -> candidate.major == BigInteger.ZERO && candidate.minor == BigInteger.ZERO &&
            candidate.patch == base.patch
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
                val values = raw.split(',').map { parseComparator(it.trim(' ')) }.distinct().sorted()
                if (values.size == 1 && values.single().op == MusubiComparatorOpV1.EQUAL) {
                    return withVersion(Kind.EXACT, values.single().version)
                }
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
) : MusubiWireValueV1(), Comparable<MusubiReleaseIdV1> {
    override fun wireValue(): Any = linkedMapOf(
        "package" to packageId.wireValue(),
        "version" to version.wireValue(),
    )

    override fun compareTo(other: MusubiReleaseIdV1): Int =
        packageId.compareTo(other.packageId).takeIf { it != 0 } ?: version.compareTo(other.version)
}

/** Canonical Norito JSON wrapper for any Musubi 32-byte digest newtype. */
open class MusubiDigest32V1(bytes: ByteArray) : MusubiWireValueV1() {
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

/** Domain-separated digest of one complete provider bundle attestation. */
class MusubiProviderBundleAttestationDigestV1(bytes: ByteArray) : MusubiDigest32V1(bytes) {
    init {
        MusubiValidationV1.requireNonZeroDigest(this, "provider bundle attestation digest")
    }
}

/** Archive/order-bound digest of a provider-sorted attestation set. */
class MusubiProviderBundleAttestationSetDigestV1(bytes: ByteArray) : MusubiDigest32V1(bytes) {
    init {
        MusubiValidationV1.requireNonZeroDigest(this, "provider bundle attestation set digest")
    }
}

/** First-release Kotodama source edition. */
enum class MusubiKotodamaEditionV1 {
    V1;

    internal fun wireValue(): Any = linkedMapOf("kind" to "V1", "value" to null)
}

/** Exact IVM ABI V1 binding embedded in manifests and verification nodes. */
class MusubiAbiBindingV1(
    abiHash: ByteArray,
) : MusubiWireValueV1() {
    /** The only ABI version accepted by Musubi V1. */
    @JvmField val abiVersion: Int = 1
    private val abiHashValue = abiHash.copyOf()

    init {
        MusubiValidationV1.requireNonZeroFixed32(abiHashValue, "Musubi ABI hash")
    }

    /** Return a defensive copy of the exact ABI hash. */
    fun abiHash(): ByteArray = abiHashValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "abi_version" to abiVersion,
        "abi_hash" to abiHashValue.map { it.toInt() and 0xff },
    )
}

/** One normal dependency range committed by a published manifest. */
class MusubiDependencyReqV1(
    @JvmField val alias: String,
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val requirement: MusubiVersionReqV1,
) : MusubiWireValueV1(), Comparable<MusubiDependencyReqV1> {
    init {
        MusubiValidationV1.requireName(alias, "Musubi dependency alias")
    }

    override fun wireValue(): Any = linkedMapOf(
        "alias" to alias,
        "package" to packageId.wireValue(),
        "requirement" to requirement.wireValue(),
    )

    override fun compareTo(other: MusubiDependencyReqV1): Int {
        MusubiValidationV1.compareUtf8(alias, other.alias).let { if (it != 0) return it }
        packageId.compareTo(other.packageId).let { if (it != 0) return it }
        return requirement.compareTo(other.requirement)
    }
}

/** Exact dependency kind recorded in a publication verification lock. */
enum class MusubiDependencyKindV1 {
    NORMAL,
    DEVELOPMENT;

    internal fun wireValue(): Any = linkedMapOf(
        "kind" to if (this == NORMAL) "Normal" else "Development",
        "value" to null,
    )
}

/** One parent-local exact edge in a normalized verification lock. */
class MusubiExactDependencyEdgeV1(
    @JvmField val alias: String,
    @JvmField val kind: MusubiDependencyKindV1,
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val requirement: MusubiVersionReqV1,
    @JvmField val selected: MusubiReleaseIdV1,
) : MusubiWireValueV1(), Comparable<MusubiExactDependencyEdgeV1> {
    init {
        MusubiValidationV1.requireName(alias, "Musubi exact dependency alias")
        require(selected.packageId == packageId && requirement.matches(selected.version)) {
            "Musubi exact dependency must select a release satisfying its package requirement"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "alias" to alias,
        "kind" to kind.wireValue(),
        "package" to packageId.wireValue(),
        "requirement" to requirement.wireValue(),
        "selected" to selected.wireValue(),
    )

    override fun compareTo(other: MusubiExactDependencyEdgeV1): Int {
        MusubiValidationV1.compareUtf8(alias, other.alias).let { if (it != 0) return it }
        kind.compareTo(other.kind).let { if (it != 0) return it }
        packageId.compareTo(other.packageId).let { if (it != 0) return it }
        requirement.compareTo(other.requirement).let { if (it != 0) return it }
        return selected.compareTo(other.selected)
    }
}

/** Exact immutable dependency node carried in a publication proof. */
class MusubiVerificationNodeV1(
    @JvmField val release: MusubiReleaseIdV1,
    @JvmField val releaseDigest: MusubiDigest32V1,
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val sourceDigest: MusubiDigest32V1,
    @JvmField val interfaceDigest: MusubiDigest32V1,
    @JvmField val abi: MusubiAbiBindingV1,
    dependencies: List<MusubiExactDependencyEdgeV1> = emptyList(),
) : MusubiWireValueV1(), Comparable<MusubiVerificationNodeV1> {
    @JvmField val dependencies: List<MusubiExactDependencyEdgeV1> = dependencies.toList()

    init {
        listOf(releaseDigest, archiveId, sourceDigest, interfaceDigest).forEach {
            MusubiValidationV1.requireNonZeroDigest(it, "Musubi verification node digest")
        }
        require(this.dependencies.size <= 256 &&
            this.dependencies.zipWithNext().all { (left, right) -> left < right }) {
            "Musubi verification-node dependencies must be bounded, sorted, and distinct"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "release" to release.wireValue(),
        "release_digest" to releaseDigest.wireValue(),
        "archive_id" to archiveId.wireValue(),
        "source_digest" to sourceDigest.wireValue(),
        "interface_digest" to interfaceDigest.wireValue(),
        "abi" to abi.wireValue(),
        "dependencies" to dependencies.map { it.wireValue() },
    )

    override fun compareTo(other: MusubiVerificationNodeV1): Int {
        release.compareTo(other.release).let { if (it != 0) return it }
        MusubiValidationV1.compareDigests(releaseDigest, other.releaseDigest)
            .let { if (it != 0) return it }
        MusubiValidationV1.compareDigests(archiveId, other.archiveId)
            .let { if (it != 0) return it }
        MusubiValidationV1.compareDigests(sourceDigest, other.sourceDigest)
            .let { if (it != 0) return it }
        return MusubiValidationV1.compareDigests(interfaceDigest, other.interfaceDigest)
    }
}

/** Normalized, secret-free exact verification lock packaged with a release. */
class MusubiVerificationLockV1(
    @JvmField val root: MusubiReleaseIdV1,
    rootDependencies: List<MusubiExactDependencyEdgeV1> = emptyList(),
    nodes: List<MusubiVerificationNodeV1> = emptyList(),
) : MusubiWireValueV1() {
    /** Closed V1 verification-lock schema label. */
    @JvmField val schema: String = SCHEMA
    /** Closed V1 verification-lock version. */
    @JvmField val version: Int = 1
    @JvmField val rootDependencies: List<MusubiExactDependencyEdgeV1> = rootDependencies.toList()
    @JvmField val nodes: List<MusubiVerificationNodeV1> = nodes.toList()

    init {
        require(this.rootDependencies.size <= 256 &&
            this.rootDependencies.zipWithNext().all { (left, right) -> left < right }) {
            "Musubi root dependencies must be bounded, sorted, and distinct"
        }
        require(this.nodes.size <= 1_024 &&
            this.nodes.zipWithNext().all { (left, right) -> left.release < right.release }) {
            "Musubi verification nodes must be bounded and sorted by distinct release"
        }
        val byRelease = this.nodes.associateBy { it.release }
        require(byRelease.size == this.nodes.size) {
            "Musubi verification lock contains duplicate releases"
        }
        this.rootDependencies.forEach { dependency ->
            require(dependency.kind == MusubiDependencyKindV1.NORMAL &&
                byRelease.containsKey(dependency.selected)) {
                "Musubi root dependency must be normal and select a proof node"
            }
        }
        validateAcyclicGraph(byRelease)
    }

    override fun wireValue(): Any = linkedMapOf(
        "schema" to schema,
        "version" to version,
        "root" to root.wireValue(),
        "root_dependencies" to rootDependencies.map { it.wireValue() },
        "nodes" to nodes.map { it.wireValue() },
    )

    private fun validateAcyclicGraph(byRelease: Map<MusubiReleaseIdV1, MusubiVerificationNodeV1>) {
        val complete = HashSet<MusubiReleaseIdV1>()
        val visiting = HashSet<MusubiReleaseIdV1>()

        fun visit(release: MusubiReleaseIdV1, depth: Int) {
            require(depth <= 64) { "Musubi verification graph exceeds depth 64" }
            if (release in complete) return
            require(visiting.add(release)) { "Musubi verification graph contains a cycle" }
            val node = requireNotNull(byRelease[release]) {
                "Musubi verification graph references a missing node"
            }
            node.dependencies.filter { it.kind == MusubiDependencyKindV1.NORMAL }.forEach {
                visit(it.selected, depth + 1)
            }
            visiting.remove(release)
            complete.add(release)
        }

        byRelease.keys.forEach { visit(it, 1) }
    }

    companion object {
        /** Stable normalized verification-lock schema. */
        const val SCHEMA: String = "musubi-verification-lock"
    }
}

/** Bounded exact resolution proof supplied with a publication. */
class MusubiResolutionProofV1(
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
    @JvmField val lock: MusubiVerificationLockV1,
) : MusubiWireValueV1() {
    override fun wireValue(): Any = linkedMapOf(
        "snapshot" to snapshot.wireValue(),
        "lock" to lock.wireValue(),
    )
}

/** Immutable release manifest binding semantic content to one source archive. */
class MusubiReleaseManifestV1(
    @JvmField val release: MusubiReleaseIdV1,
    @JvmField val edition: MusubiKotodamaEditionV1,
    @JvmField val abi: MusubiAbiBindingV1,
    dependencies: List<MusubiDependencyReqV1> = emptyList(),
    exports: List<String> = emptyList(),
    @JvmField val interfaceDigest: MusubiDigest32V1,
    @JvmField val metadata: MusubiReleaseMetadataV1,
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val verificationLockDigest: MusubiDigest32V1,
) : MusubiWireValueV1() {
    @JvmField val dependencies: List<MusubiDependencyReqV1> = dependencies.toList()
    @JvmField val exports: List<String> = exports.toList()

    init {
        require(this.dependencies.size <= 256 &&
            this.dependencies.zipWithNext().all { (left, right) -> left < right }) {
            "Musubi manifest dependencies must be bounded, sorted, and distinct"
        }
        require(this.exports.size <= 1_024) { "Musubi manifest exceeds 1024 exports" }
        this.exports.forEach { MusubiValidationV1.requireName(it, "Musubi export") }
        require(this.exports.zipWithNext().all { (left, right) ->
            MusubiValidationV1.compareUtf8(left, right) < 0
        }) { "Musubi manifest exports must be sorted and distinct" }
        require(this.dependencies.none { it.packageId == release.packageId }) {
            "Musubi release cannot depend on its own package"
        }
        listOf(interfaceDigest, archiveId, verificationLockDigest).forEach {
            MusubiValidationV1.requireNonZeroDigest(it, "Musubi manifest digest")
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "release" to release.wireValue(),
        "edition" to edition.wireValue(),
        "abi" to abi.wireValue(),
        "dependencies" to dependencies.map { it.wireValue() },
        "exports" to exports,
        "interface_digest" to interfaceDigest.wireValue(),
        "metadata" to metadata.wireValue(),
        "archive_id" to archiveId.wireValue(),
        "verification_lock_digest" to verificationLockDigest.wireValue(),
    )
}

/** Publication payload binding one immutable manifest to its exact proof. */
class MusubiPublicationV1(
    @JvmField val manifest: MusubiReleaseManifestV1,
    @JvmField val resolution: MusubiResolutionProofV1,
) : MusubiWireValueV1() {
    init {
        require(resolution.lock.root == manifest.release) {
            "Musubi publication lock root must equal the manifest release"
        }
        require(manifest.dependencies.size == resolution.lock.rootDependencies.size) {
            "Musubi publication direct dependency counts disagree"
        }
        manifest.dependencies.zip(resolution.lock.rootDependencies).forEach { (range, exact) ->
            require(exact.kind == MusubiDependencyKindV1.NORMAL && exact.alias == range.alias &&
                exact.packageId == range.packageId && exact.requirement == range.requirement) {
                "Musubi publication proof does not bind a direct dependency"
            }
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "manifest" to manifest.wireValue(),
        "resolution" to resolution.wireValue(),
    )
}

/** Exact generation-bound namespace authority delegated to one publisher. */
class MusubiNamespaceDelegationPayloadV1(
    @JvmField val namespaceBinding: MusubiDigest32V1,
    @JvmField val ownerGeneration: BigInteger,
    @JvmField val owner: String,
    @JvmField val delegate: String,
    @JvmField val expiresAtHeight: BigInteger,
) : MusubiWireValueV1() {
    /** Closed delegation payload version. */
    @JvmField val version: Int = 1
    internal val ownerPayload: ByteArray
    internal val delegatePayload: ByteArray

    init {
        MusubiValidationV1.requireNonZeroDigest(
            namespaceBinding,
            "Musubi namespace-binding digest",
        )
        MusubiValidationV1.requireU64(ownerGeneration, "namespaceDelegation.ownerGeneration")
        MusubiValidationV1.requireU64(expiresAtHeight, "namespaceDelegation.expiresAtHeight")
        require(ownerGeneration > BigInteger.ZERO && expiresAtHeight > BigInteger.ZERO) {
            "Musubi namespace delegation generation and expiry must be non-zero"
        }
        ownerPayload = MusubiValidationV1.canonicalAccountPayload(
            owner,
            "namespaceDelegation.owner",
        )
        delegatePayload = MusubiValidationV1.canonicalAccountPayload(
            delegate,
            "namespaceDelegation.delegate",
        )
    }

    override fun wireValue(): Any = linkedMapOf(
        "version" to version,
        "namespace_binding" to namespaceBinding.wireValue(),
        "owner_generation" to ownerGeneration,
        "owner" to owner,
        "delegate" to delegate,
        "expires_at_height" to expiresAtHeight,
    )
}

/** One namespace-owner controller approval over a delegation payload. */
class MusubiNamespaceDelegationApprovalV1(
    @JvmField val publicKey: String,
    @JvmField val signature: String,
) : MusubiWireValueV1() {
    internal val publicKeyPayload = MusubiValidationV1.canonicalPublicKeyPayload(
        publicKey,
        "namespaceDelegation.approval.publicKey",
    )
    internal val signaturePayload = MusubiValidationV1.canonicalSignaturePayload(
        signature,
        "namespaceDelegation.approval.signature",
    )

    override fun wireValue(): Any = linkedMapOf(
        "public_key" to publicKey,
        "signature" to signature,
    )
}

/** Signed authority to claim an absent package in one namespace. */
class MusubiNamespaceDelegationV1(
    @JvmField val payload: MusubiNamespaceDelegationPayloadV1,
    approvals: List<MusubiNamespaceDelegationApprovalV1>,
) : MusubiWireValueV1() {
    @JvmField val approvals: List<MusubiNamespaceDelegationApprovalV1> = approvals.toList()

    init {
        require(this.approvals.size in 1..64) {
            "Musubi namespace delegation needs between 1 and 64 approvals"
        }
        require(this.approvals.zipWithNext().all { (left, right) ->
            MusubiValidationV1.compareUnsignedBytes(
                left.publicKeyPayload,
                right.publicKeyPayload,
            ) < 0
        }) { "Musubi namespace delegation approvals must be sorted and distinct" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "payload" to payload.wireValue(),
        "approvals" to approvals.map { it.wireValue() },
    )
}

/** Enacted Parliament decision authorizing one delayed Musubi governance action. */
class MusubiGovernanceDecisionV1(
    decisionId: ByteArray,
    @JvmField val actionDigest: MusubiDigest32V1,
    @JvmField val enactedAtHeight: BigInteger,
    @JvmField val executeAfterHeight: BigInteger,
) : MusubiWireValueV1() {
    private val decisionIdValue = decisionId.copyOf()

    init {
        require(decisionIdValue.size == 32 && decisionIdValue.any { it.toInt() != 0 }) {
            "Musubi governance decision ID must be a non-zero 32-byte value"
        }
        require(actionDigest.bytes().any { it.toInt() != 0 }) {
            "Musubi governance action digest must be non-zero"
        }
        MusubiValidationV1.requireU64(enactedAtHeight, "governanceDecision.enactedAtHeight")
        MusubiValidationV1.requireU64(
            executeAfterHeight,
            "governanceDecision.executeAfterHeight",
        )
        require(enactedAtHeight > BigInteger.ZERO && executeAfterHeight > enactedAtHeight) {
            "Musubi governance execution height must follow a non-zero enactment height"
        }
    }

    /** Return a defensive copy of the exact enacted proposal fingerprint. */
    fun decisionId(): ByteArray = decisionIdValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "decision_id" to decisionIdValue.map { it.toInt() and 0xff },
        "action_digest" to actionDigest.wireValue(),
        "enacted_at_height" to enactedAtHeight,
        "execute_after_height" to executeAfterHeight,
    )
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
        require(hash.size == 32 && hash.any { it.toInt() != 0 }) {
            "Musubi finalized block hash must contain a non-inert 32-byte value"
        }
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
        require(queryHash.bytes().any { it.toInt() != 0 }) {
            "Musubi cursor query hash must not be inert"
        }
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
        require(limit in 0..100) { "Musubi page limit exceeds 100" }
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

/** Fresh-selection availability for one finalized archive projection. */
enum class MusubiStorageAvailabilityV1(internal val wireKind: String) {
    SELECTABLE("Selectable"),
    BELOW_QUORUM("BelowQuorum"),
    UNAVAILABLE("Unavailable"),
}

/** Finalized aggregate storage projection carried by retention decisions. */
class MusubiArchiveAvailabilityV1 internal constructor(
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val availability: MusubiStorageAvailabilityV1,
    @JvmField val healthyReplicas: Int,
    @JvmField val activeLocations: Int,
    @JvmField val finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    @JvmField val indexRevision: BigInteger,
) : MusubiWireValueV1() {
    private val finalizedBlockHashValue = finalizedBlockHash.copyOf()

    init {
        require(archiveId.bytes().any { it.toInt() != 0 }) {
            "Musubi archive availability uses the zero archive identity"
        }
        require(healthyReplicas in 0..65_535 && activeLocations in 0..4 &&
            healthyReplicas <= activeLocations * 64) {
            "Musubi archive availability counts are invalid"
        }
        MusubiValidationV1.requireU64(finalizedHeight, "archiveAvailability.finalizedHeight")
        MusubiValidationV1.requireU64(indexRevision, "archiveAvailability.indexRevision")
        require(finalizedHeight > BigInteger.ZERO && indexRevision > BigInteger.ZERO &&
            finalizedBlockHashValue.size == 32 &&
            finalizedBlockHashValue.any { it.toInt() != 0 }) {
            "Musubi archive availability anchor is invalid"
        }
        val expected = when {
            healthyReplicas >= 3 -> MusubiStorageAvailabilityV1.SELECTABLE
            activeLocations > 0 && healthyReplicas > 0 ->
                MusubiStorageAvailabilityV1.BELOW_QUORUM
            else -> MusubiStorageAvailabilityV1.UNAVAILABLE
        }
        require(availability == expected) {
            "Musubi archive availability classification is inconsistent with its counts"
        }
    }

    fun finalizedBlockHash(): ByteArray = finalizedBlockHashValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "archive_id" to archiveId.wireValue(),
        "availability" to linkedMapOf("kind" to availability.wireKind, "value" to null),
        "healthy_replicas" to healthyReplicas,
        "active_locations" to activeLocations,
        "finalized_height" to finalizedHeight,
        "finalized_block_hash" to finalizedBlockHashValue.map { it.toInt() and 0xff },
        "index_revision" to indexRevision,
    )
}

/** Authoritative cache-retention classification for one exact archive. */
enum class MusubiArchiveRetentionDispositionV1(internal val wireKind: String) {
    RETAIN_UNKNOWN("RetainUnknown"),
    RETAIN_REFERENCED("RetainReferenced"),
    PRUNE_UNREFERENCED("PruneUnreferenced"),
    PRUNE_GOVERNED_TAKEDOWN("PruneGovernedTakedown"),
    ;

    /** Whether the finalized classification requires the cache entry to remain. */
    fun mustRetain(): Boolean =
        this == RETAIN_UNKNOWN || this == RETAIN_REFERENCED
}

/** One exact finalized cache-retention decision. */
class MusubiArchiveRetentionDecisionV1 internal constructor(
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val disposition: MusubiArchiveRetentionDispositionV1,
    @JvmField val activeReleases: Int,
    @JvmField val yankedReleases: Int,
    @JvmField val takenDownReleases: Int,
    @JvmField val storage: MusubiArchiveAvailabilityV1?,
) : MusubiWireValueV1() {
    init {
        require(archiveId.bytes().any { it.toInt() != 0 }) {
            "Musubi archive retention decision uses the zero archive identity"
        }
        require(activeReleases in 0..65_535 && yankedReleases in 0..65_535 &&
            takenDownReleases in 0..65_535) {
            "Musubi archive retention counts must fit u16"
        }
        val referenced = activeReleases + yankedReleases + takenDownReleases
        require(referenced <= 1_024) {
            "Musubi archive retention decision exceeds the release-reference bound"
        }
        require(storage == null || storage.archiveId == archiveId) {
            "Musubi archive retention storage projection has a different identity"
        }
        val available = activeReleases + yankedReleases
        val canonical = when (disposition) {
            MusubiArchiveRetentionDispositionV1.RETAIN_UNKNOWN ->
                referenced == 0 && storage == null
            MusubiArchiveRetentionDispositionV1.RETAIN_REFERENCED ->
                available > 0 && storage != null
            MusubiArchiveRetentionDispositionV1.PRUNE_UNREFERENCED ->
                referenced == 0 && storage != null
            MusubiArchiveRetentionDispositionV1.PRUNE_GOVERNED_TAKEDOWN ->
                available == 0 && takenDownReleases > 0 && storage != null
        }
        require(canonical) {
            "Musubi archive retention decision is internally inconsistent"
        }
    }

    /** Whether this exact finalized decision requires retention. */
    fun mustRetain(): Boolean = disposition.mustRetain()

    override fun wireValue(): Any = linkedMapOf(
        "archive_id" to archiveId.wireValue(),
        "disposition" to linkedMapOf("kind" to disposition.wireKind, "value" to null),
        "active_releases" to activeReleases,
        "yanked_releases" to yankedReleases,
        "taken_down_releases" to takenDownReleases,
        "storage" to storage?.wireValue(),
    )
}

/** Bounded, sorted exact archive identities for authoritative cache retention. */
class MusubiArchiveRetentionQueryV1(
    archiveIds: List<MusubiDigest32V1>,
    @JvmField val expectedSnapshot: MusubiRegistrySnapshotV1? = null,
) : MusubiWireValueV1() {
    @JvmField val archiveIds: List<MusubiDigest32V1> = archiveIds.toList()

    init {
        require(this.archiveIds.isNotEmpty() && this.archiveIds.size <= 100 &&
            this.archiveIds.all { digest -> digest.bytes().any { it.toInt() != 0 } } &&
            this.archiveIds.zipWithNext().all { (left, right) ->
                MusubiValidationV1.compareUnsignedBytes(left.bytes(), right.bytes()) < 0
            }) {
            "Musubi archive retention batch is empty, oversized, or noncanonical"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "archive_ids" to archiveIds.map { it.wireValue() },
        "expected_snapshot" to expectedSnapshot?.wireValue(),
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

/** Finalized anchor for the rebuildable package-search projection. */
class MusubiSearchSnapshotV1(
    @JvmField val finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    @JvmField val projectionRevision: BigInteger,
) : MusubiWireValueV1() {
    private val hash = finalizedBlockHash.copyOf()

    init {
        MusubiValidationV1.requireU64(finalizedHeight, "searchSnapshot.finalizedHeight")
        MusubiValidationV1.requireU64(projectionRevision, "searchSnapshot.projectionRevision")
        require(finalizedHeight > BigInteger.ZERO && projectionRevision > BigInteger.ZERO) {
            "Musubi search snapshot revisions must be non-zero"
        }
        require(hash.size == 32 && hash.any { it.toInt() != 0 }) {
            "Musubi search snapshot hash must be non-inert"
        }
    }

    fun finalizedBlockHash(): ByteArray = hash.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "finalized_height" to finalizedHeight,
        "finalized_block_hash" to hash.map { it.toInt() and 0xff },
        "projection_revision" to projectionRevision,
    )
}

/** Search continuation bound to one exact query and projection snapshot. */
class MusubiSearchCursorV1(
    @JvmField val snapshot: MusubiSearchSnapshotV1,
    @JvmField val queryHash: MusubiDigest32V1,
    @JvmField val lastPackage: MusubiPackageIdV1,
) : MusubiWireValueV1() {
    init {
        require(queryHash.bytes().any { it.toInt() != 0 }) {
            "Musubi search cursor query hash must not be inert"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "snapshot" to snapshot.wireValue(),
        "query_hash" to queryHash.wireValue(),
        "last_package" to lastPackage.wireValue(),
    )
}

/** Bounded page controls for rich package discovery. */
class MusubiSearchPageRequestV1(
    @JvmField val limit: Long = 50,
    @JvmField val cursor: MusubiSearchCursorV1? = null,
) : MusubiWireValueV1() {
    init {
        require(limit in 0..100) { "Musubi search page limit exceeds 100" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "limit" to limit,
        "cursor" to cursor?.wireValue(),
    )
}

/** Exact-token description and keyword search query. */
class MusubiSearchQueryV1(
    @JvmField val query: String,
    @JvmField val page: MusubiSearchPageRequestV1 = MusubiSearchPageRequestV1(),
) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireExactText(query, "Musubi search query")
        require(query.toByteArray(StandardCharsets.UTF_8).size <= 256) {
            "Musubi search query exceeds 256 UTF-8 bytes"
        }
        MusubiValidationV1.normalizedSearchTerms(query)
    }

    override fun wireValue(): Any = linkedMapOf(
        "query" to query,
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

    /** Rejects an exact-package response for a different structural package. */
    fun requireMatches(request: MusubiExactPackageQueryV1) {
        require(packageId == request.packageId) {
            "Musubi exact-package response does not match the request"
        }
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
    @JvmField val manifest: MusubiReleaseManifestV1,
    @JvmField val releaseDigest: MusubiDigest32V1,
    @JvmField val publishedBy: String,
    @JvmField val publishedAtHeight: BigInteger,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    @JvmField val release: MusubiReleaseIdV1 = manifest.release
    internal val rawValue = MusubiJsonV1.immutableObject(raw)

    init {
        MusubiValidationV1.canonicalAccountPayload(publishedBy, "release publisher")
        MusubiValidationV1.requireU64(publishedAtHeight, "publishedAtHeight")
        require(publishedAtHeight > BigInteger.ZERO) {
            "Musubi publication height must be non-zero"
        }
        require(
            releaseDigest.bytes().contentEquals(musubiReleaseManifestDigestV1(manifest)),
        ) { "Musubi release digest does not match its canonical manifest" }
    }

    /** Rejects an exact-release response for a different immutable release. */
    fun requireMatches(request: MusubiExactReleaseQueryV1) {
        require(release == request.release) {
            "Musubi exact-release response does not match the request"
        }
    }

    override fun wireValue(): Any = rawValue
}

/** Compact resolver row response with strict fields and typed release identity. */
class MusubiResolverReleaseRowV1 internal constructor(
    @JvmField val release: MusubiReleaseIdV1,
    @JvmField val indexRevision: BigInteger,
    @JvmField val storageIndexRevision: BigInteger,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    internal val rawValue = MusubiJsonV1.immutableObject(raw)
    override fun wireValue(): Any = rawValue
}

/** Finalized paired view of one release from its home dataspace and universal index. */
class MusubiExactReleaseSnapshotV1 internal constructor(
    @JvmField val chainId: String,
    genesisHash: ByteArray,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
    @JvmField val homeRelease: MusubiReleaseRecordV1,
    @JvmField val universalRelease: MusubiResolverReleaseRowV1,
) : MusubiWireValueV1() {
    private val genesisHashValue = genesisHash.copyOf()

    init {
        MusubiValidationV1.requireChainId(chainId, "exact release chain ID")
        MusubiValidationV1.requireNonZeroFixed32(
            genesisHashValue,
            "exact release genesis hash",
        )
        require(homeRelease.release == universalRelease.release &&
            homeRelease.publishedAtHeight <= snapshot.finalizedHeight &&
            universalRelease.storageIndexRevision <= universalRelease.indexRevision &&
            universalRelease.indexRevision <= snapshot.indexRevision) {
            "Musubi exact release projections are inconsistent with their finalized snapshot"
        }
        MusubiJsonV1.validateExactReleaseSnapshot(
            homeRelease.rawValue,
            universalRelease.rawValue,
            genesisHashValue,
            snapshot,
        )
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    /** Rejects an exact-release response for a different immutable release. */
    fun requireMatches(request: MusubiExactReleaseQueryV1) {
        require(homeRelease.release == request.release &&
            universalRelease.release == request.release) {
            "Musubi exact-release snapshot does not match the request"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "snapshot" to snapshot.wireValue(),
        "home_release" to homeRelease.wireValue(),
        "universal_release" to universalRelease.wireValue(),
    )
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

/** Pending package-governance invitation that has not created authority. */
class MusubiMaintainerInvitationV1 internal constructor(
    @JvmField val inviteId: MusubiDigest32V1,
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val invitedBy: String,
    @JvmField val invitedAccount: String,
    @JvmField val roleKind: String,
    @JvmField val expectedGovernanceRevision: BigInteger,
    @JvmField val expiresAtHeight: BigInteger,
    @JvmField val stateKind: String,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    private val rawValue = MusubiJsonV1.immutableObject(raw)
    override fun wireValue(): Any = rawValue
}

/** Accepted member or pending invitation returned by the maintainer directory. */
sealed class MusubiMaintainerDirectoryEntryV1 : MusubiWireValueV1() {
    /** Entry variant. */
    enum class Kind { ACCEPTED, PENDING_INVITATION }

    /** Stable variant discriminator. */
    abstract val kind: Kind

    /** Accepted owner or maintainer with current authority. */
    class Accepted internal constructor(
        @JvmField val member: MusubiPackageMemberV1,
    ) : MusubiMaintainerDirectoryEntryV1() {
        override val kind: Kind = Kind.ACCEPTED
        override fun wireValue(): Any = linkedMapOf(
            "kind" to "Accepted",
            "value" to member.wireValue(),
        )
    }

    /** Invitation that remains pending and has not created authority. */
    class PendingInvitation internal constructor(
        @JvmField val invitation: MusubiMaintainerInvitationV1,
    ) : MusubiMaintainerDirectoryEntryV1() {
        override val kind: Kind = Kind.PENDING_INVITATION
        override fun wireValue(): Any = linkedMapOf(
            "kind" to "PendingInvitation",
            "value" to invitation.wireValue(),
        )
    }
}

/** Renewable archive-location response with strict outer fields. */
class MusubiArchiveLocationV1 internal constructor(
    @JvmField val locationId: MusubiDigest32V1,
    @JvmField val archiveId: MusubiDigest32V1,
    providers: List<String>,
    @JvmField val providerAttestationSetDigest: MusubiProviderBundleAttestationSetDigestV1,
    @JvmField val finalizedHeight: BigInteger,
    @JvmField val revision: BigInteger,
    @JvmField val stateKind: String,
    raw: Map<String, Any?>,
) : MusubiWireValueV1() {
    @JvmField val providers: List<String> = providers.toList()
    private val rawValue = MusubiJsonV1.immutableObject(raw)

    init {
        require(this.providers.size in 1..64) {
            "Musubi archive location needs between 1 and 64 providers"
        }
        val providerIds = this.providers.map {
            MusubiValidationV1.canonicalFixed32Hex(it, "archive location provider ID")
        }
        require(providerIds.zipWithNext().all { (left, right) ->
            MusubiValidationV1.compareUnsignedBytes(left, right) < 0
        }) { "Musubi archive location providers must be sorted and distinct" }
    }

    override fun wireValue(): Any = rawValue
}

/** Exact SoraFS chunker profile bound into a Musubi archive commitment. */
class MusubiChunkerProfileHandleV1(
    @JvmField val profileId: Long,
    @JvmField val namespace: String,
    @JvmField val name: String,
    @JvmField val semver: String,
    @JvmField val multihashCode: BigInteger,
) : MusubiWireValueV1() {
    init {
        require(profileId in 0..0xffff_ffffL) { "Musubi chunker profile ID must fit u32" }
        MusubiValidationV1.requireU64(multihashCode, "chunker.multihashCode")
        listOf(namespace, name, semver).forEach {
            MusubiValidationV1.requireExactText(it, "Musubi chunker profile field")
        }
        require("$namespace.$name@$semver".toByteArray(StandardCharsets.UTF_8).size <= 128) {
            "Musubi chunker handle exceeds 128 UTF-8 bytes"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "profile_id" to profileId,
        "namespace" to namespace,
        "name" to name,
        "semver" to semver,
        "multihash_code" to multihashCode,
    )
}

/** Complete immutable source-archive commitment returned by the registry. */
class MusubiArchiveCommitmentV1(
    rootCid: ByteArray,
    @JvmField val chunker: MusubiChunkerProfileHandleV1,
    @JvmField val chunkPlanDigest: MusubiDigest32V1,
    @JvmField val porRoot: MusubiDigest32V1,
    @JvmField val contentLength: BigInteger,
    @JvmField val carDigest: MusubiDigest32V1,
    @JvmField val carSize: BigInteger,
    @JvmField val bundleDigest: MusubiDigest32V1,
    @JvmField val sourceTreeDigest: MusubiDigest32V1,
    @JvmField val descriptorDigest: MusubiDigest32V1,
    @JvmField val fileCount: Long,
    @JvmField val chunkCount: Long,
) : MusubiWireValueV1() {
    private val rootCidValue = rootCid.copyOf()

    init {
        require(rootCidValue.size == 36) { "Musubi SoraFS root CID must contain 36 bytes" }
        require(rootCidValue.take(4).map { it.toInt() and 0xff } == listOf(1, 113, 31, 32)) {
            "Musubi SoraFS root CID must use the canonical CIDv1/dag-cbor/BLAKE3-256 header"
        }
        require(rootCidValue.drop(4).any { it.toInt() != 0 }) {
            "Musubi SoraFS root CID must not contain an inert digest"
        }
        require(
            listOf(
                chunkPlanDigest, porRoot, carDigest, bundleDigest, sourceTreeDigest,
                descriptorDigest,
            ).none { digest -> digest.bytes().all { it.toInt() == 0 } },
        ) { "Musubi archive commitment digests must not be inert" }
        require(contentLength > BigInteger.ZERO && contentLength <= BigInteger.valueOf(64L shl 20)) {
            "Musubi archive source length is out of bounds"
        }
        require(carSize > BigInteger.ZERO && carSize <= BigInteger.valueOf(96L shl 20)) {
            "Musubi archive CAR length is out of bounds"
        }
        require(fileCount in 1..4_096 && chunkCount in 1..16_384) {
            "Musubi archive file or chunk count is out of bounds"
        }
    }

    fun rootCid(): ByteArray = rootCidValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "root_cid" to rootCidValue.map { it.toInt() and 0xff },
        "chunker" to chunker.wireValue(),
        "chunk_plan_digest" to chunkPlanDigest.wireValue(),
        "por_root" to porRoot.wireValue(),
        "content_length" to contentLength,
        "car_digest" to carDigest.wireValue(),
        "car_size" to carSize,
        "bundle_digest" to bundleDigest.wireValue(),
        "source_tree_digest" to sourceTreeDigest.wireValue(),
        "descriptor_digest" to descriptorDigest.wireValue(),
        "file_count" to fileCount,
        "chunk_count" to chunkCount,
    )
}

/** Exact deployment and CAR-body binding signed by seed ingress. */
class MusubiSeedIngressReceiptBindingV1(
    @JvmField val chainId: String,
    genesisBlockHash: ByteArray,
    @JvmField val publisher: String,
    @JvmField val ingressBroker: String,
    @JvmField val seedProvider: String,
    @JvmField val semanticReleaseManifestDigest: MusubiDigest32V1,
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val carBodyDigest: MusubiDigest32V1,
    @JvmField val carBodyLength: BigInteger,
    nonce: ByteArray,
) : MusubiWireValueV1() {
    private val genesisBlockHashValue = genesisBlockHash.copyOf()
    private val nonceValue = nonce.copyOf()
    internal val publisherPayload: ByteArray
    internal val ingressBrokerPayload: ByteArray
    internal val seedProviderPayload: ByteArray

    init {
        MusubiValidationV1.requireChainId(chainId, "Musubi seed-ingress chain ID")
        publisherPayload = MusubiValidationV1.canonicalAccountPayload(
            publisher,
            "seedIngress.publisher",
        )
        ingressBrokerPayload = MusubiValidationV1.canonicalAccountPayload(
            ingressBroker,
            "seedIngress.ingressBroker",
        )
        MusubiValidationV1.requireNonZeroFixed32(
            genesisBlockHashValue,
            "Musubi seed-ingress genesis hash",
        )
        MusubiValidationV1.requireNonZeroFixed32(nonceValue, "Musubi seed-ingress nonce")
        seedProviderPayload = MusubiValidationV1.canonicalFixed32Hex(
            seedProvider,
            "Musubi seed provider",
        )
        MusubiValidationV1.requireU64(carBodyLength, "carBodyLength")
        require(carBodyLength > BigInteger.ZERO &&
            carBodyLength <= BigInteger.valueOf(96L shl 20)) {
            "Musubi staged CAR length is out of bounds"
        }
        require(
            listOf(semanticReleaseManifestDigest, archiveId, carBodyDigest)
                .none { digest -> digest.bytes().all { it.toInt() == 0 } },
        ) { "Musubi seed-ingress digest bindings must not be inert" }
    }

    fun genesisBlockHash(): ByteArray = genesisBlockHashValue.copyOf()
    fun nonce(): ByteArray = nonceValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_block_hash" to genesisBlockHashValue.map { it.toInt() and 0xff },
        "publisher" to publisher,
        "ingress_broker" to ingressBroker,
        "seed_provider" to listOf(seedProvider),
        "semantic_release_manifest_digest" to semanticReleaseManifestDigest.wireValue(),
        "archive_id" to archiveId.wireValue(),
        "car_body_digest" to carBodyDigest.wireValue(),
        "car_body_length" to carBodyLength,
        "nonce" to nonceValue.map { it.toInt() and 0xff },
    )
}

/** One controller approval over a first-release seed-ingress receipt. */
class MusubiSeedIngressReceiptApprovalV1(
    @JvmField val publicKey: String,
    @JvmField val signature: String,
) : MusubiWireValueV1() {
    internal val publicKeyPayload = MusubiValidationV1.canonicalPublicKeyPayload(
        publicKey,
        "seedIngress.approval.publicKey",
    )
    internal val signaturePayload = MusubiValidationV1.canonicalSignaturePayload(
        signature,
        "seedIngress.approval.signature",
    )

    override fun wireValue(): Any = linkedMapOf("public_key" to publicKey, "signature" to signature)
}

/** Version-one signed seed-ingress receipt payload. */
class MusubiSeedIngressReceiptPayloadV1(
    @JvmField val binding: MusubiSeedIngressReceiptBindingV1,
    @JvmField val issuedAtMs: BigInteger,
    @JvmField val expiresAtMs: BigInteger,
) : MusubiWireValueV1() {
    init {
        MusubiValidationV1.requireU64(issuedAtMs, "issuedAtMs")
        MusubiValidationV1.requireU64(expiresAtMs, "expiresAtMs")
        require(issuedAtMs > BigInteger.ZERO && expiresAtMs > issuedAtMs &&
            expiresAtMs - issuedAtMs <= BigInteger.valueOf(86_400_000L)) {
            "Musubi seed-ingress receipt lifetime is invalid"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "version" to 1,
        "binding" to binding.wireValue(),
        "issued_at_ms" to issuedAtMs,
        "expires_at_ms" to expiresAtMs,
    )
}

/** Authenticated seed-ingress receipt retained by an archive registration. */
class MusubiSeedIngressReceiptV1(
    @JvmField val payload: MusubiSeedIngressReceiptPayloadV1,
    approvals: List<MusubiSeedIngressReceiptApprovalV1>,
) : MusubiWireValueV1() {
    @JvmField val approvals: List<MusubiSeedIngressReceiptApprovalV1> = approvals.toList()

    init {
        require(this.approvals.isNotEmpty() && this.approvals.size <= 64) {
            "Musubi seed-ingress receipt needs a bounded approval set"
        }
        require(this.approvals.zipWithNext().all { (left, right) ->
            MusubiValidationV1.compareUnsignedBytes(
                left.publicKeyPayload,
                right.publicKeyPayload,
            ) < 0
        }) { "Musubi seed-ingress receipt approvals must be sorted and distinct" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "payload" to payload.wireValue(),
        "approvals" to approvals.map { it.wireValue() },
    )
}

/** Exact governed signer-policy identity for one provider-ingest completion. */
class MusubiProviderIngestCompletionSignerPolicyV1(
    policyId: ByteArray,
    @JvmField val revision: BigInteger,
    predecessorDigest: ByteArray?,
    policyDigest: ByteArray,
) : MusubiWireValueV1() {
    private val policyIdValue = policyId.copyOf()
    private val predecessorDigestValue = predecessorDigest?.copyOf()
    private val policyDigestValue = policyDigest.copyOf()

    init {
        MusubiValidationV1.requireNonZeroFixed32(policyIdValue, "provider signer policy ID")
        MusubiValidationV1.requireU64(revision, "providerSignerPolicy.revision")
        require(revision > BigInteger.ZERO) { "Provider signer-policy revision must be non-zero" }
        when {
            revision == BigInteger.ONE -> require(predecessorDigestValue == null) {
                "Provider signer-policy revision one must not have a predecessor"
            }
            else -> MusubiValidationV1.requireNonZeroFixed32(
                requireNotNull(predecessorDigestValue) {
                    "Provider signer-policy successor requires a predecessor digest"
                },
                "provider signer-policy predecessor",
            )
        }
        MusubiValidationV1.requireNonZeroFixed32(
            policyDigestValue,
            "provider signer-policy digest",
        )
    }

    fun policyId(): ByteArray = policyIdValue.copyOf()
    fun predecessorDigest(): ByteArray? = predecessorDigestValue?.copyOf()
    fun policyDigest(): ByteArray = policyDigestValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "policy_id" to policyIdValue.map { it.toInt() and 0xff },
        "revision" to revision,
        "predecessor_digest" to predecessorDigestValue?.map { it.toInt() and 0xff },
        "policy_digest" to policyDigestValue.map { it.toInt() and 0xff },
    )
}

/** Chain-authoritative provider owner and governed completion signer policy. */
class MusubiProviderIngestCompletionAuthorityV1(
    @JvmField val providerOwner: String,
    @JvmField val signerPolicy: MusubiProviderIngestCompletionSignerPolicyV1,
) : MusubiWireValueV1() {
    internal val providerOwnerPayload = MusubiValidationV1.canonicalAccountPayload(
        providerOwner,
        "providerCompletionAuthority.providerOwner",
    )

    override fun wireValue(): Any = linkedMapOf(
        "provider_owner" to providerOwner,
        "signer_policy" to signerPolicy.wireValue(),
    )
}

/** Finalized committed-chain anchor carried by one provider completion. */
class MusubiProviderIngestFinalizedAnchorV1(
    @JvmField val height: BigInteger,
    blockHash: ByteArray,
) : MusubiWireValueV1() {
    private val blockHashValue = blockHash.copyOf()

    init {
        MusubiValidationV1.requireU64(height, "providerFinalizedAnchor.height")
        require(height > BigInteger.ZERO) { "Provider finalized-anchor height must be non-zero" }
        MusubiValidationV1.requireNonZeroFixed32(
            blockHashValue,
            "provider finalized-anchor block hash",
        )
    }

    fun blockHash(): ByteArray = blockHashValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "height" to height,
        "block_hash" to blockHashValue.map { it.toInt() and 0xff },
    )
}

/** Exact parsed-bundle and finalized replication completion bound by one provider. */
class MusubiProviderBundleVerificationBindingV1(
    @JvmField val chainId: String,
    genesisBlockHash: ByteArray,
    @JvmField val providerId: String,
    @JvmField val completedBy: String,
    @JvmField val completionAuthority: MusubiProviderIngestCompletionAuthorityV1,
    @JvmField val replicationOrder: MusubiDigest32V1,
    @JvmField val assignmentRevision: BigInteger,
    @JvmField val completionEpoch: BigInteger,
    @JvmField val finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1,
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val bundleDigest: MusubiDigest32V1,
    @JvmField val descriptorDigest: MusubiDigest32V1,
    @JvmField val semanticReleaseManifestDigest: MusubiDigest32V1,
    @JvmField val verificationLockDigest: MusubiDigest32V1,
    @JvmField val sourceTreeDigest: MusubiDigest32V1,
) : MusubiWireValueV1() {
    private val genesisBlockHashValue = genesisBlockHash.copyOf()
    internal val providerIdPayload: ByteArray
    internal val completedByPayload: ByteArray

    init {
        MusubiValidationV1.requireChainId(chainId, "provider attestation chain ID")
        MusubiValidationV1.requireNonZeroFixed32(
            genesisBlockHashValue,
            "provider attestation genesis hash",
        )
        providerIdPayload = MusubiValidationV1.canonicalFixed32Hex(
            providerId,
            "Musubi provider ID",
        )
        completedByPayload = MusubiValidationV1.canonicalAccountPayload(
            completedBy,
            "providerAttestation.completedBy",
        )
        require(completedByPayload.contentEquals(completionAuthority.providerOwnerPayload)) {
            "Provider attestation completer must equal the completion-authority owner"
        }
        MusubiValidationV1.requireU64(assignmentRevision, "provider assignment revision")
        MusubiValidationV1.requireU64(completionEpoch, "provider completion epoch")
        require(assignmentRevision > BigInteger.ZERO && completionEpoch > BigInteger.ZERO) {
            "Provider assignment revision and completion epoch must be non-zero"
        }
        listOf(
            replicationOrder,
            archiveId,
            bundleDigest,
            descriptorDigest,
            semanticReleaseManifestDigest,
            verificationLockDigest,
            sourceTreeDigest,
        ).forEach { MusubiValidationV1.requireNonZeroDigest(it, "provider attestation digest") }
    }

    fun genesisBlockHash(): ByteArray = genesisBlockHashValue.copyOf()

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_block_hash" to genesisBlockHashValue.map { it.toInt() and 0xff },
        "provider_id" to listOf(providerId),
        "completed_by" to completedBy,
        "completion_authority" to completionAuthority.wireValue(),
        "replication_order" to replicationOrder.wireValue(),
        "assignment_revision" to assignmentRevision,
        "completion_epoch" to completionEpoch,
        "finalized_anchor" to finalizedAnchor.wireValue(),
        "archive_id" to archiveId.wireValue(),
        "bundle_digest" to bundleDigest.wireValue(),
        "descriptor_digest" to descriptorDigest.wireValue(),
        "semantic_release_manifest_digest" to semanticReleaseManifestDigest.wireValue(),
        "verification_lock_digest" to verificationLockDigest.wireValue(),
        "source_tree_digest" to sourceTreeDigest.wireValue(),
    )
}

/** Canonical V1 statement that a provider parsed and verified one bundle. */
class MusubiProviderBundleVerificationPayloadV1(
    @JvmField val binding: MusubiProviderBundleVerificationBindingV1,
) : MusubiWireValueV1() {
    @JvmField val version: Int = 1

    override fun wireValue(): Any = linkedMapOf(
        "version" to version,
        "binding" to binding.wireValue(),
    )
}

/** One provider-owner controller approval over a bundle-verification payload. */
class MusubiProviderBundleVerificationApprovalV1(
    @JvmField val publicKey: String,
    @JvmField val signature: String,
) : MusubiWireValueV1() {
    internal val publicKeyPayload = MusubiValidationV1.canonicalPublicKeyPayload(
        publicKey,
        "providerAttestation.approval.publicKey",
    )
    internal val signaturePayload = MusubiValidationV1.canonicalSignaturePayload(
        signature,
        "providerAttestation.approval.signature",
    )

    override fun wireValue(): Any = linkedMapOf(
        "public_key" to publicKey,
        "signature" to signature,
    )
}

/** Signed proof that one provider parsed a canonical bundle before finalized completion. */
class MusubiProviderBundleVerificationAttestationV1(
    @JvmField val payload: MusubiProviderBundleVerificationPayloadV1,
    approvals: List<MusubiProviderBundleVerificationApprovalV1>,
) : MusubiWireValueV1() {
    @JvmField val approvals: List<MusubiProviderBundleVerificationApprovalV1> = approvals.toList()

    init {
        require(this.approvals.size in 1..64) {
            "Musubi provider attestation needs between 1 and 64 approvals"
        }
        require(this.approvals.zipWithNext().all { (left, right) ->
            MusubiValidationV1.compareUnsignedBytes(
                left.publicKeyPayload,
                right.publicKeyPayload,
            ) < 0
        }) { "Musubi provider approvals must be sorted and distinct" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "payload" to payload.wireValue(),
        "approvals" to approvals.map { it.wireValue() },
    )
}

/** Immutable archive/order/provider identity of one registered provider proof. */
class MusubiProviderBundleAttestationKeyV1(
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val replicationOrder: MusubiDigest32V1,
    @JvmField val providerId: String,
) : MusubiWireValueV1() {
    internal val providerIdPayload = MusubiValidationV1.canonicalFixed32Hex(
        providerId,
        "provider attestation key provider ID",
    )

    init {
        MusubiValidationV1.requireNonZeroDigest(archiveId, "provider attestation archive ID")
        MusubiValidationV1.requireNonZeroDigest(
            replicationOrder,
            "provider attestation replication order",
        )
    }

    override fun wireValue(): Any = linkedMapOf(
        "archive_id" to archiveId.wireValue(),
        "replication_order" to replicationOrder.wireValue(),
        "provider_id" to listOf(providerId),
    )
}

/** Complete immutable provider proof stored under its exact archive/order/provider key. */
class MusubiProviderBundleAttestationRecordV1(
    @JvmField val key: MusubiProviderBundleAttestationKeyV1,
    @JvmField val attestationDigest: MusubiProviderBundleAttestationDigestV1,
    @JvmField val attestation: MusubiProviderBundleVerificationAttestationV1,
    @JvmField val registeredBy: String,
    @JvmField val registeredAtHeight: BigInteger,
) : MusubiWireValueV1() {
    init {
        val binding = attestation.payload.binding
        require(key.archiveId == binding.archiveId &&
            key.replicationOrder == binding.replicationOrder &&
            key.providerIdPayload.contentEquals(binding.providerIdPayload)) {
            "Musubi provider attestation record key disagrees with its signed binding"
        }
        require(
            attestationDigest.bytes().contentEquals(
                musubiProviderBundleAttestationDigestV1(attestation),
            ),
        ) {
            "Musubi provider attestation digest disagrees with its canonical attestation bytes"
        }
        MusubiValidationV1.canonicalAccountPayload(
            registeredBy,
            "providerAttestationRecord.registeredBy",
        )
        MusubiValidationV1.requireU64(
            registeredAtHeight,
            "providerAttestationRecord.registeredAtHeight",
        )
        require(registeredAtHeight > BigInteger.ZERO) {
            "Musubi provider attestation registration height must be non-zero"
        }
    }

    /** Rejects an audit response for a different archive/order/provider identity. */
    fun requireMatches(request: MusubiProviderBundleAttestationKeyV1) {
        require(key == request) {
            "Musubi provider attestation response does not match the request"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "key" to key.wireValue(),
        "attestation_digest" to attestationDigest.wireValue(),
        "attestation" to attestation.wireValue(),
        "registered_by" to registeredBy,
        "registered_at_height" to registeredAtHeight,
    )
}

/** Authoritative immutable archive registration independent of renewable locations. */
class MusubiArchiveRecordV1 internal constructor(
    @JvmField val archiveId: MusubiDigest32V1,
    @JvmField val commitment: MusubiArchiveCommitmentV1,
    @JvmField val stagingReceipt: MusubiSeedIngressReceiptV1,
    @JvmField val registeredBy: String,
    @JvmField val registeredAtHeight: BigInteger,
    @JvmField val locationRevision: BigInteger,
    locationIds: List<MusubiDigest32V1>,
) : MusubiWireValueV1() {
    @JvmField val locationIds: List<MusubiDigest32V1> = locationIds.toList()

    init {
        require(stagingReceipt.payload.binding.archiveId == archiveId) {
            "Musubi staging receipt must bind the archive ID"
        }
        require(stagingReceipt.payload.binding.carBodyDigest == commitment.carDigest &&
            stagingReceipt.payload.binding.carBodyLength == commitment.carSize) {
            "Musubi staging receipt must bind the committed CAR"
        }
        require(stagingReceipt.payload.binding.publisher == registeredBy) {
            "Musubi staging receipt publisher must match the archive registrant"
        }
        require(registeredAtHeight > BigInteger.ZERO && locationRevision > BigInteger.ZERO) {
            "Musubi archive height and location revision must be non-zero"
        }
        require(this.locationIds.size <= 4) { "Musubi archive has too many locations" }
        require(this.locationIds.none { digest -> digest.bytes().all { it.toInt() == 0 } } &&
            this.locationIds.zipWithNext().all { (left, right) ->
                MusubiValidationV1.compareUnsignedBytes(left.bytes(), right.bytes()) < 0
            }) { "Musubi archive location IDs must be non-inert, sorted, and distinct" }
    }

    override fun wireValue(): Any = linkedMapOf(
        "archive_id" to archiveId.wireValue(),
        "commitment" to commitment.wireValue(),
        "staging_receipt" to stagingReceipt.wireValue(),
        "registered_by" to registeredBy,
        "registered_at_height" to registeredAtHeight,
        "location_revision" to locationRevision,
        "location_ids" to locationIds.map { it.wireValue() },
    )
}

/** Prospective permanent-alias prices denominated in whole XOR. */
class MusubiAliasPricingPolicyV1(
    @JvmField val revision: BigInteger,
    @JvmField val length1Xor: BigInteger,
    @JvmField val length2Xor: BigInteger,
    @JvmField val length3Xor: BigInteger,
    @JvmField val length4Xor: BigInteger,
    @JvmField val length5To32Xor: BigInteger,
) : MusubiWireValueV1() {
    init {
        listOf(
            "revision" to revision,
            "length1Xor" to length1Xor,
            "length2Xor" to length2Xor,
            "length3Xor" to length3Xor,
            "length4Xor" to length4Xor,
            "length5To32Xor" to length5To32Xor,
        ).forEach { (field, value) ->
            MusubiValidationV1.requireU64(value, "aliasPricing.$field")
            require(value > BigInteger.ZERO) { "Musubi alias pricing values must be non-zero" }
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "revision" to revision,
        "length_1_xor" to length1Xor,
        "length_2_xor" to length2Xor,
        "length_3_xor" to length3Xor,
        "length_4_xor" to length4Xor,
        "length_5_to_32_xor" to length5To32Xor,
    )
}

/** Admission mode for new archives, releases, and permanent aliases. */
enum class MusubiRegistryAdmissionModeV1 {
    CLOSED,
    ALLOWLISTED,
    OPEN;

    internal fun wireValue(): Any = linkedMapOf(
        "kind" to when (this) {
            CLOSED -> "Closed"
            ALLOWLISTED -> "Allowlisted"
            OPEN -> "Open"
        },
        "value" to null,
    )
}

/** Complete version-one Musubi registry admission and alias-pricing policy. */
class MusubiRegistryPolicyV1(
    @JvmField val revision: BigInteger,
    @JvmField val mode: MusubiRegistryAdmissionModeV1,
    allowlistedDataspaces: List<BigInteger>,
    @JvmField val aliasPricing: MusubiAliasPricingPolicyV1,
) : MusubiWireValueV1() {
    /** Closed registry-policy schema version. */
    @JvmField val version: Int = 1
    @JvmField val allowlistedDataspaces: List<BigInteger> = allowlistedDataspaces.toList()

    init {
        MusubiValidationV1.requireU64(revision, "registryPolicy.revision")
        require(revision > BigInteger.ZERO) { "Musubi registry-policy revision must be non-zero" }
        require(this.allowlistedDataspaces.size <= 1_024) {
            "Musubi registry allowlist exceeds 1024 dataspaces"
        }
        this.allowlistedDataspaces.forEach {
            MusubiValidationV1.requireU64(it, "registryPolicy.allowlistedDataspace")
        }
        require(this.allowlistedDataspaces.zipWithNext().all { (left, right) -> left < right }) {
            "Musubi registry allowlist must be sorted and distinct"
        }
        require(mode == MusubiRegistryAdmissionModeV1.ALLOWLISTED ||
            this.allowlistedDataspaces.isEmpty()) {
            "Only allowlisted Musubi admission may carry a dataspace allowlist"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "version" to version,
        "revision" to revision,
        "mode" to mode.wireValue(),
        "allowlisted_dataspaces" to allowlistedDataspaces,
        "alias_pricing" to aliasPricing.wireValue(),
    )
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
        MusubiValidationV1.canonicalAccountPayload(registeredBy, "alias registrant")
        listOf(pricingRevision, paidXor, registeredAtHeight, historyRevision).forEach {
            MusubiValidationV1.requireU64(it, "alias record counter")
            require(it > BigInteger.ZERO) { "Musubi alias record counters must be non-zero" }
        }
    }

    /** Rejects an exact-alias response for another permanent alias. */
    fun requireMatches(request: MusubiAliasQueryV1) {
        require(alias == request.alias) { "Musubi alias response does not match the request" }
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
    init {
        MusubiValidationV1.requireAsciiKebab(alias, 32, "alias history alias")
        MusubiValidationV1.requireU64(revision, "alias history revision")
        MusubiValidationV1.requireU64(finalizedHeight, "alias history finalized height")
        val actionIsValid = when (actionKind) {
            "Registered" -> revision == BigInteger.ONE &&
                previousTarget == null && governanceAction == null
            "ParliamentRetarget" -> revision > BigInteger.ONE &&
                previousTarget != null && governanceAction != null &&
                governanceAction.bytes().any { it.toInt() != 0 }
            else -> false
        }
        require(actionIsValid && finalizedHeight > BigInteger.ZERO) {
            "Musubi alias-history entry is invalid"
        }
    }

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
    init {
        require(selector.name == packageId.name) {
            "Musubi ordered entry selector and package names disagree"
        }
        require(metadataRevision > BigInteger.ZERO && indexRevision > BigInteger.ZERO) {
            "Musubi ordered entry revisions must be non-zero"
        }
    }

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
    @JvmField val query: MusubiWireValueV1,
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

    internal fun requireVersionMatches(request: MusubiPackagePageQueryV1) {
        val versions = items.map { item ->
            require(item is MusubiVersionV1) { "Musubi version page contains another item type" }
            item
        }
        require(query == request) { "Musubi version response query does not match the request" }
        require(MusubiValidationV1.semVerPageAdvances(request.page, versions.firstOrNull())) {
            "Musubi version response does not advance its structured cursor"
        }
        MusubiValidationV1.requireFinalizedPageMatches(
            request.page,
            versions.size,
            versions.firstOrNull()?.canonicalText(),
            versions.lastOrNull()?.canonicalText(),
            snapshot,
            nextCursor,
        )
    }

    internal fun requireMaintainerMatches(request: MusubiPackagePageQueryV1) {
        val maintainers = items.map { item ->
            require(item is MusubiMaintainerDirectoryEntryV1) {
                "Musubi maintainer page contains another item type"
            }
            item
        }
        require(query == request) { "Musubi maintainer response query does not match the request" }
        require(maintainers.all { MusubiValidationV1.maintainerPackageId(it) == request.packageId }) {
            "Musubi maintainer response contains another package"
        }
        require(MusubiValidationV1.maintainerPageAdvances(request.page, maintainers.firstOrNull())) {
            "Musubi maintainer response does not advance its structured cursor"
        }
        MusubiValidationV1.requireFinalizedPageMatches(
            request.page,
            maintainers.size,
            maintainers.firstOrNull()?.let(MusubiValidationV1::maintainerCursorKey),
            maintainers.lastOrNull()?.let(MusubiValidationV1::maintainerCursorKey),
            snapshot,
            nextCursor,
        )
    }

    internal fun requireAliasHistoryMatches(request: MusubiAliasQueryV1) {
        val history = items.map { item ->
            require(item is MusubiAliasHistoryEntryV1) {
                "Musubi alias-history page contains another item type"
            }
            item
        }
        require(query == request) { "Musubi alias-history response query does not match the request" }
        require(history.all { it.alias == request.alias } &&
            MusubiValidationV1.aliasHistoryPageAdvances(request, history.firstOrNull())) {
            "Musubi alias-history response does not advance its structured cursor"
        }
        MusubiValidationV1.requireFinalizedPageMatches(
            request.page,
            history.size,
            history.firstOrNull()?.let(MusubiValidationV1::aliasHistoryCursorKey),
            history.lastOrNull()?.let(MusubiValidationV1::aliasHistoryCursorKey),
            snapshot,
            nextCursor,
        )
    }

    override fun wireValue(): Any = linkedMapOf(
        "query" to query.wireValue(),
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

/** Resolver page carrying the exact chain/genesis identity required by lockfiles. */
class MusubiResolverIndexPageV1 internal constructor(
    @JvmField val query: MusubiResolverIndexQueryV1,
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
        require(genesisHashValue.size == 32 && genesisHashValue.any { it.toInt() != 0 }) {
            "Musubi genesis hash must contain a non-inert 32-byte value"
        }
        require(this.items.size <= 100 &&
            this.items.zipWithNext().all { (left, right) -> left.release < right.release }) {
            "Musubi resolver page exceeds 100 items or is not strictly ordered"
        }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi resolver cursor must use the page snapshot"
        }
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    /** Binds every resolver row and any continuation snapshot to the exact request. */
    fun requireMatches(request: MusubiResolverIndexQueryV1) {
        require(query == request) { "Musubi resolver response query does not match the request" }
        require(items.all { row ->
            row.release.packageId == request.packageId &&
                request.requirement?.matches(row.release.version) != false
        }) {
            "Musubi resolver response contains another package or an excluded version"
        }
        require(MusubiValidationV1.semVerPageAdvances(
            request.page,
            items.firstOrNull()?.release?.version,
        )) { "Musubi resolver response does not advance its structured cursor" }
        MusubiValidationV1.requireFinalizedPageMatches(
            request.page,
            items.size,
            items.firstOrNull()?.release?.version?.canonicalText(),
            items.lastOrNull()?.release?.version?.canonicalText(),
            snapshot,
            nextCursor,
        )
    }

    override fun wireValue(): Any = linkedMapOf(
        "query" to query.wireValue(),
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

/** Archive-location page carrying deployment identity and the immutable commitment. */
class MusubiArchiveLocationPageV1 internal constructor(
    @JvmField val chainId: String,
    genesisHash: ByteArray,
    @JvmField val archive: MusubiArchiveRecordV1,
    items: List<MusubiArchiveLocationV1>,
    @JvmField val nextCursor: MusubiFinalizedCursorV1?,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
) : MusubiWireValueV1() {
    private val genesisHashValue = genesisHash.copyOf()
    @JvmField val items: List<MusubiArchiveLocationV1> = items.toList()

    init {
        MusubiValidationV1.requireExactText(chainId, "Musubi archive-location chain ID")
        require(genesisHashValue.size == 32 && genesisHashValue.any { it.toInt() != 0 }) {
            "Musubi genesis hash must contain a non-inert 32-byte value"
        }
        require(archive.stagingReceipt.payload.binding.chainId == chainId &&
            archive.stagingReceipt.payload.binding.genesisBlockHash().contentEquals(genesisHashValue)) {
            "Musubi archive registration must use the page deployment identity"
        }
        require(archive.registeredAtHeight <= snapshot.finalizedHeight) {
            "Musubi archive registration is newer than the page snapshot"
        }
        require(this.items.size <= 4 &&
            this.items.zipWithNext().all { (left, right) ->
                MusubiValidationV1.compareUnsignedBytes(
                    left.locationId.bytes(),
                    right.locationId.bytes(),
                ) < 0
            } &&
            this.items.all { item ->
                item.archiveId == archive.archiveId &&
                    archive.locationIds.any { it == item.locationId } &&
                    item.stateKind != "Retired" &&
                    item.finalizedHeight <= snapshot.finalizedHeight &&
                    item.revision <= archive.locationRevision
            }) {
            "Musubi archive-location page has an invalid item set"
        }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi archive-location cursor must use the page snapshot"
        }
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    /** Binds the archive record and any continuation snapshot to the exact request. */
    fun requireMatches(request: MusubiArchiveLocationQueryV1) {
        require(archive.archiveId == request.archiveId) {
            "Musubi archive-location response does not match the request"
        }
        MusubiValidationV1.requirePageMatches(request.page, snapshot, nextCursor, items.size)
    }

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "archive" to archive.wireValue(),
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

/** Exact finalized cache-retention decisions for one bounded request batch. */
class MusubiArchiveRetentionPageV1 internal constructor(
    @JvmField val chainId: String,
    genesisHash: ByteArray,
    items: List<MusubiArchiveRetentionDecisionV1>,
    @JvmField val finalizedTimeMs: BigInteger,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
) : MusubiWireValueV1() {
    private val genesisHashValue = genesisHash.copyOf()
    @JvmField val items: List<MusubiArchiveRetentionDecisionV1> = items.toList()

    init {
        MusubiValidationV1.requireU64(finalizedTimeMs, "archiveRetention.finalizedTimeMs")
        MusubiValidationV1.requireExactText(chainId, "Musubi archive-retention chain ID")
        require(genesisHashValue.size == 32 && genesisHashValue.any { it.toInt() != 0 } &&
            this.items.isNotEmpty() && this.items.size <= 100 &&
            this.items.zipWithNext().all { (left, right) ->
                MusubiValidationV1.compareUnsignedBytes(
                    left.archiveId.bytes(),
                    right.archiveId.bytes(),
                ) < 0
            }) {
            "Musubi archive-retention page has an invalid deployment or item bound"
        }
        require(this.items.all { decision ->
            val storage = decision.storage ?: return@all true
            storage.finalizedHeight <= snapshot.finalizedHeight &&
                storage.indexRevision <= snapshot.indexRevision &&
                (storage.finalizedHeight != snapshot.finalizedHeight ||
                    storage.finalizedBlockHash().contentEquals(snapshot.finalizedBlockHash()))
        }) {
            "Musubi archive-retention storage projection exceeds the page snapshot"
        }
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    /** Enforces the exact request identity order and optional snapshot binding. */
    fun requireMatches(request: MusubiArchiveRetentionQueryV1) {
        require(request.expectedSnapshot == null || request.expectedSnapshot == snapshot) {
            "Musubi archive-retention response uses another finalized snapshot"
        }
        require(items.map { it.archiveId } == request.archiveIds) {
            "Musubi archive-retention response identities differ from the exact request"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "items" to items.map { it.wireValue() },
        "finalized_time_ms" to finalizedTimeMs,
        "snapshot" to snapshot.wireValue(),
    )
}

/** Ordered-directory page carrying the exact chain/genesis identity for lock creation. */
class MusubiOrderedPrefixPageV1 internal constructor(
    @JvmField val query: MusubiOrderedPrefixQueryV1,
    @JvmField val chainId: String,
    genesisHash: ByteArray,
    @JvmField val namespaceBinding: MusubiNamespaceBindingV1,
    items: List<MusubiOrderedPackageEntryV1>,
    @JvmField val nextCursor: MusubiFinalizedCursorV1?,
    @JvmField val snapshot: MusubiRegistrySnapshotV1,
) : MusubiWireValueV1() {
    private val genesisHashValue = genesisHash.copyOf()
    @JvmField val items: List<MusubiOrderedPackageEntryV1> = items.toList()

    init {
        MusubiValidationV1.requireExactText(chainId, "Musubi directory chain ID")
        require(genesisHashValue.size == 32 && genesisHashValue.any { it.toInt() != 0 }) {
            "Musubi genesis hash must contain a non-inert 32-byte value"
        }
        require(this.items.size <= 100) { "Musubi ordered-prefix page exceeds 100 items" }
        require(this.items.all { item ->
            item.selector.namespace == namespaceBinding.namespace &&
                item.packageId.homeDataspace == namespaceBinding.homeDataspace &&
                item.packageId.scope == namespaceBinding.scope
        }) { "Musubi ordered-prefix row does not match its namespace binding" }
        require(this.items.zipWithNext().all { (left, right) ->
            val namespaceOrder = MusubiValidationV1.compareUnsignedBytes(
                left.selector.namespace.value.toByteArray(StandardCharsets.UTF_8),
                right.selector.namespace.value.toByteArray(StandardCharsets.UTF_8),
            )
            namespaceOrder < 0 || namespaceOrder == 0 &&
                MusubiValidationV1.compareUnsignedBytes(
                    left.selector.name.value.toByteArray(StandardCharsets.UTF_8),
                    right.selector.name.value.toByteArray(StandardCharsets.UTF_8),
                ) < 0
        }) { "Musubi ordered-prefix rows must be sorted and distinct" }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi ordered-prefix cursor must use the page snapshot"
        }
    }

    fun genesisHash(): ByteArray = genesisHashValue.copyOf()

    /** Binds directory rows and any continuation snapshot to the requested prefix. */
    fun requireMatches(request: MusubiOrderedPrefixQueryV1) {
        require(query == request) {
            "Musubi ordered-prefix response query does not match the request"
        }
        val requestedNamespace = request.prefix.substringBefore('/', missingDelimiterValue = "")
        require(requestedNamespace == namespaceBinding.namespace.value && items.all { item ->
            "${item.selector.namespace.value}/${item.selector.name.value}"
                .startsWith(request.prefix)
        }) { "Musubi ordered-prefix response contains a selector outside the request" }
        require(MusubiValidationV1.orderedPrefixPageAdvances(request, items.firstOrNull())) {
            "Musubi ordered-prefix response does not advance its structured cursor"
        }
        MusubiValidationV1.requireFinalizedPageMatches(
            request.page,
            items.size,
            items.firstOrNull()?.let(MusubiValidationV1::orderedSelectorCursorKey),
            items.lastOrNull()?.let(MusubiValidationV1::orderedSelectorCursorKey),
            snapshot,
            nextCursor,
        )
    }

    override fun wireValue(): Any = linkedMapOf(
        "query" to query.wireValue(),
        "chain_id" to chainId,
        "genesis_hash" to genesisHashValue.map { it.toInt() and 0xff },
        "namespace_binding" to namespaceBinding.wireValue(),
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

/** One exact-token package metadata search result. */
class MusubiSearchHitV1(
    @JvmField val packageId: MusubiPackageIdV1,
    @JvmField val claimedNamespace: MusubiNamespaceV1,
    @JvmField val description: String?,
    keywords: List<String>,
    @JvmField val metadataRevision: BigInteger,
) : MusubiWireValueV1() {
    @JvmField val keywords: List<String> = keywords.toList()

    init {
        description?.let {
            MusubiValidationV1.requireExactText(it, "Musubi search description")
            require(it.toByteArray(StandardCharsets.UTF_8).size <= 4_096) {
                "Musubi search description exceeds 4096 UTF-8 bytes"
            }
        }
        require(this.keywords.size <= 32) { "Musubi search hit has too many keywords" }
        this.keywords.forEach { MusubiValidationV1.requireAsciiKebab(it, 64, "keyword") }
        require(this.keywords == this.keywords.distinct().sorted()) {
            "Musubi search keywords must be sorted and distinct"
        }
        MusubiValidationV1.requireU64(metadataRevision, "searchHit.metadataRevision")
        require(metadataRevision > BigInteger.ZERO) {
            "Musubi search metadata revision must be non-zero"
        }
        require(MusubiValidationV1.namespaceMatchesScope(packageId, claimedNamespace)) {
            "Musubi search namespace and package scope disagree"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "package" to packageId.wireValue(),
        "claimed_namespace" to claimedNamespace.wireValue(),
        "description" to description?.let(::listOf),
        "keywords" to keywords.map(::listOf),
        "metadata_revision" to metadataRevision,
    )
}

/** Bounded page from the finalized-event package-search projection. */
class MusubiSearchPageV1 internal constructor(
    @JvmField val query: MusubiSearchQueryV1,
    items: List<MusubiSearchHitV1>,
    @JvmField val nextCursor: MusubiSearchCursorV1?,
    @JvmField val snapshot: MusubiSearchSnapshotV1,
) : MusubiWireValueV1() {
    @JvmField val items: List<MusubiSearchHitV1> = items.toList()

    init {
        require(this.items.size <= 100) { "Musubi search page exceeds 100 items" }
        require(this.items.zipWithNext().all { (left, right) ->
            MusubiValidationV1.comparePackageIds(left.packageId, right.packageId) < 0
        }) { "Musubi search page items must be sorted and distinct" }
        require(nextCursor == null || nextCursor.snapshot == snapshot) {
            "Musubi search cursor must use the page snapshot"
        }
        require(nextCursor == null || nextCursor.lastPackage == this.items.lastOrNull()?.packageId) {
            "Musubi search cursor must bind the final page item"
        }
    }

    /** Binds the echoed query and any continuation to the exact request. */
    fun requireMatches(request: MusubiSearchQueryV1) {
        require(query == request) { "Musubi search response query does not match the request" }
        val effectiveLimit = MusubiValidationV1.effectivePageLimit(request.page.limit)
        val cursor = request.page.cursor
        require(items.size <= effectiveLimit &&
            (cursor == null || snapshot == cursor.snapshot &&
                items.firstOrNull()?.let {
                    MusubiValidationV1.comparePackageIds(cursor.lastPackage, it.packageId) < 0
                } != false &&
                (nextCursor == null || nextCursor.queryHash == cursor.queryHash)) &&
            (nextCursor == null || nextCursor.snapshot == snapshot &&
                items.size == effectiveLimit &&
                nextCursor.lastPackage == items.lastOrNull()?.packageId)) {
            "Musubi search response has an invalid structured cursor or page boundary"
        }
    }

    override fun wireValue(): Any = linkedMapOf(
        "query" to query.wireValue(),
        "items" to items.map { it.wireValue() },
        "next_cursor" to nextCursor?.wireValue(),
        "snapshot" to snapshot.wireValue(),
    )
}

internal object MusubiValidationV1 {
    val U64_MAX: BigInteger = BigInteger("18446744073709551615")

    fun requireU64(value: BigInteger, field: String) {
        require(value >= BigInteger.ZERO && value <= U64_MAX) { "$field must fit u64" }
    }

    fun compareUnsignedBytes(left: ByteArray, right: ByteArray): Int {
        for (index in 0 until minOf(left.size, right.size)) {
            val comparison = (left[index].toInt() and 0xff).compareTo(right[index].toInt() and 0xff)
            if (comparison != 0) return comparison
        }
        return left.size.compareTo(right.size)
    }

    fun compareUtf8(left: String, right: String): Int = compareUnsignedBytes(
        left.toByteArray(StandardCharsets.UTF_8),
        right.toByteArray(StandardCharsets.UTF_8),
    )

    fun compareDigests(left: MusubiDigest32V1, right: MusubiDigest32V1): Int =
        compareUnsignedBytes(left.bytes(), right.bytes())

    fun compareMaintainerEntries(
        left: MusubiMaintainerDirectoryEntryV1,
        right: MusubiMaintainerDirectoryEntryV1,
    ): Int {
        comparePackageIds(maintainerPackageId(left), maintainerPackageId(right))
            .let { if (it != 0) return it }
        compareAccountIds(maintainerAccount(left), maintainerAccount(right))
            .let { if (it != 0) return it }
        val leftInvitation = maintainerInvitation(left)
        val rightInvitation = maintainerInvitation(right)
        if (leftInvitation == null) return if (rightInvitation == null) 0 else -1
        if (rightInvitation == null) return 1
        return compareDigests(leftInvitation, rightInvitation)
    }

    fun maintainerPackageId(
        entry: MusubiMaintainerDirectoryEntryV1,
    ): MusubiPackageIdV1 = when (entry) {
        is MusubiMaintainerDirectoryEntryV1.Accepted -> entry.member.packageId
        is MusubiMaintainerDirectoryEntryV1.PendingInvitation -> entry.invitation.packageId
    }

    private fun maintainerAccount(entry: MusubiMaintainerDirectoryEntryV1): String = when (entry) {
        is MusubiMaintainerDirectoryEntryV1.Accepted -> entry.member.account
        is MusubiMaintainerDirectoryEntryV1.PendingInvitation -> entry.invitation.invitedAccount
    }

    private fun maintainerInvitation(
        entry: MusubiMaintainerDirectoryEntryV1,
    ): MusubiDigest32V1? = when (entry) {
        is MusubiMaintainerDirectoryEntryV1.Accepted -> null
        is MusubiMaintainerDirectoryEntryV1.PendingInvitation -> entry.invitation.inviteId
    }

    fun maintainerCursorKey(entry: MusubiMaintainerDirectoryEntryV1): String {
        val account = lowerHex(canonicalAccountPayload(
            maintainerAccount(entry),
            "maintainer cursor account",
        ))
        val invitation = maintainerInvitation(entry)?.let { "pending-${lowerHex(it.bytes())}" }
            ?: "accepted"
        return "$account|$invitation"
    }

    fun maintainerPageAdvances(
        request: MusubiPageRequestV1,
        first: MusubiMaintainerDirectoryEntryV1?,
    ): Boolean {
        val cursor = request.cursor ?: return true
        val previous = parseMaintainerCursorBoundary(cursor.lastKey) ?: return false
        first ?: return true
        val firstAccount = canonicalAccountPayload(
            maintainerAccount(first),
            "maintainer cursor account",
        )
        val accountOrder = compareUnsignedBytes(previous.account, firstAccount)
        if (accountOrder != 0) return accountOrder < 0
        val firstInvitation = maintainerInvitation(first)?.bytes()
        return when {
            previous.invitation == null && firstInvitation != null -> true
            previous.invitation == null || firstInvitation == null -> false
            else -> compareUnsignedBytes(previous.invitation, firstInvitation) < 0
        }
    }

    private class MaintainerCursorBoundary(
        val account: ByteArray,
        val invitation: ByteArray?,
    )

    private fun parseMaintainerCursorBoundary(value: String): MaintainerCursorBoundary? {
        val separator = value.indexOf('|')
        if (separator <= 0 || separator != value.lastIndexOf('|')) return null
        val account = decodeLowerHex(value.substring(0, separator)) ?: return null
        val invitation = value.substring(separator + 1)
        if (invitation == "accepted") return MaintainerCursorBoundary(account, null)
        if (!invitation.startsWith("pending-")) return null
        val invitationBytes = decodeLowerHex(invitation.substring("pending-".length)) ?: return null
        return if (invitationBytes.size == 32) {
            MaintainerCursorBoundary(account, invitationBytes)
        } else {
            null
        }
    }

    private fun lowerHex(bytes: ByteArray): String {
        val alphabet = "0123456789abcdef"
        val encoded = CharArray(bytes.size * 2)
        bytes.forEachIndexed { index, byte ->
            val value = byte.toInt() and 0xff
            encoded[index * 2] = alphabet[value ushr 4]
            encoded[index * 2 + 1] = alphabet[value and 0x0f]
        }
        return String(encoded)
    }

    private fun decodeLowerHex(value: String): ByteArray? {
        if (value.isEmpty() || value.length % 2 != 0 ||
            value.any { it !in '0'..'9' && it !in 'a'..'f' }) {
            return null
        }
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    fun semVerPageAdvances(
        request: MusubiPageRequestV1,
        first: MusubiVersionV1?,
    ): Boolean {
        val cursor = request.cursor ?: return true
        val previous = try {
            MusubiVersionV1.parse(cursor.lastKey)
        } catch (_: IllegalArgumentException) {
            return false
        }
        return first == null || previous < first
    }

    fun aliasHistoryCursorKey(entry: MusubiAliasHistoryEntryV1): String =
        "${entry.alias}:${entry.revision.toString().padStart(20, '0')}"

    fun aliasHistoryPageAdvances(
        request: MusubiAliasQueryV1,
        first: MusubiAliasHistoryEntryV1?,
    ): Boolean {
        val cursor = request.page.cursor ?: return true
        val separator = cursor.lastKey.lastIndexOf(':')
        if (separator <= 0) return false
        val alias = cursor.lastKey.substring(0, separator)
        val revisionText = cursor.lastKey.substring(separator + 1)
        if (alias != request.alias || revisionText.length != 20 ||
            revisionText.any { it !in '0'..'9' }) {
            return false
        }
        val revision = try {
            parseU64(revisionText, "alias-history cursor revision")
        } catch (_: IllegalArgumentException) {
            return false
        }
        if (revision.toString().padStart(20, '0') != revisionText) return false
        return first == null || first.alias == alias && first.revision > revision
    }

    fun orderedSelectorCursorKey(entry: MusubiOrderedPackageEntryV1): String =
        "${entry.selector.namespace.value}/${entry.selector.name.value}"

    fun orderedPrefixPageAdvances(
        request: MusubiOrderedPrefixQueryV1,
        first: MusubiOrderedPackageEntryV1?,
    ): Boolean {
        val cursor = request.page.cursor ?: return true
        val previous = parseSelectorCursor(cursor.lastKey) ?: return false
        val separator = request.prefix.indexOf('/')
        if (separator <= 0 || separator != request.prefix.lastIndexOf('/') ||
            previous.namespace.value != request.prefix.substring(0, separator) ||
            !cursor.lastKey.startsWith(request.prefix)) {
            return false
        }
        return first == null || compareSelectors(previous, first.selector) < 0
    }

    private fun parseSelectorCursor(value: String): MusubiPackageSelectorV1? {
        val separator = value.indexOf('/')
        if (separator <= 0 || separator != value.lastIndexOf('/') || separator == value.lastIndex) {
            return null
        }
        return try {
            val selector = MusubiPackageSelectorV1(
                MusubiNamespaceV1(value.substring(0, separator)),
                MusubiPackageNameV1(value.substring(separator + 1)),
            )
            if ("${selector.namespace.value}/${selector.name.value}" == value) selector else null
        } catch (_: IllegalArgumentException) {
            null
        }
    }

    private fun compareSelectors(
        left: MusubiPackageSelectorV1,
        right: MusubiPackageSelectorV1,
    ): Int {
        compareUtf8(left.namespace.value, right.namespace.value).let { if (it != 0) return it }
        return compareUtf8(left.name.value, right.name.value)
    }

    private fun compareAccountIds(left: String, right: String): Int {
        val leftAddress = canonicalAccountAddress(left)
        val rightAddress = canonicalAccountAddress(right)
        val leftSingle = leftAddress.singleKeyPayloadIgnoringCurveSupport()
        val rightSingle = rightAddress.singleKeyPayloadIgnoringCurveSupport()
        if (leftSingle != null || rightSingle != null) {
            if (leftSingle == null) return 1
            if (rightSingle == null) return -1
            return compareUnsignedBytes(
                compactPublicKeyPayload(leftSingle.curveId, leftSingle.publicKey),
                compactPublicKeyPayload(rightSingle.curveId, rightSingle.publicKey),
            )
        }

        val leftPolicy = requireNotNull(leftAddress.multisigPolicyPayloadIgnoringCurveSupport())
        val rightPolicy = requireNotNull(rightAddress.multisigPolicyPayloadIgnoringCurveSupport())
        leftPolicy.version.compareTo(rightPolicy.version).let { if (it != 0) return it }
        leftPolicy.threshold.compareTo(rightPolicy.threshold).let { if (it != 0) return it }
        for (index in 0 until minOf(leftPolicy.members.size, rightPolicy.members.size)) {
            val leftMember = leftPolicy.members[index]
            val rightMember = rightPolicy.members[index]
            compareUnsignedBytes(
                compactPublicKeyPayload(leftMember.curveId, leftMember.publicKey),
                compactPublicKeyPayload(rightMember.curveId, rightMember.publicKey),
            ).let { if (it != 0) return it }
            leftMember.weight.compareTo(rightMember.weight).let { if (it != 0) return it }
        }
        return leftPolicy.members.size.compareTo(rightPolicy.members.size)
    }

    private fun canonicalAccountAddress(value: String): AccountAddress {
        requireCanonicalI105Address(value, "maintainer account")
        return AccountAddress.parseEncodedIgnoringCurveSupport(value, null).address
    }

    fun effectivePageLimit(limit: Long): Int = when {
        limit == 0L -> 50
        limit > 100L -> 100
        else -> limit.toInt()
    }

    fun requirePageMatches(
        request: MusubiPageRequestV1,
        snapshot: MusubiRegistrySnapshotV1,
        nextCursor: MusubiFinalizedCursorV1?,
        itemCount: Int,
    ) {
        // Typed page carriers validate their echoed query identity before this shared page check.
        val cursor = request.cursor
        require(itemCount <= effectivePageLimit(request.limit) &&
            (cursor == null || cursor.snapshot == snapshot &&
                (nextCursor == null || nextCursor.queryHash == cursor.queryHash &&
                    nextCursor.caller == cursor.caller))) {
            "Musubi response does not use the requested page limit or cursor binding"
        }
    }

    fun requireFinalizedPageMatches(
        request: MusubiPageRequestV1,
        itemCount: Int,
        firstKey: String?,
        lastKey: String?,
        snapshot: MusubiRegistrySnapshotV1,
        nextCursor: MusubiFinalizedCursorV1?,
    ) {
        val effectiveLimit = effectivePageLimit(request.limit)
        require(itemCount <= effectiveLimit &&
            (itemCount == 0 && firstKey == null && lastKey == null ||
                itemCount > 0 && firstKey != null && lastKey != null)) {
            "Musubi response exceeds its requested bound or has invalid cursor keys"
        }
        request.cursor?.let { cursor ->
            require(cursor.snapshot == snapshot && cursor.caller == null) {
                "Musubi response does not continue its public finalized cursor"
            }
        }
        nextCursor?.let { cursor ->
            require(cursor.snapshot == snapshot && cursor.caller == null &&
                itemCount == effectiveLimit && cursor.lastKey == lastKey &&
                (request.cursor == null || request.cursor.queryHash == cursor.queryHash)) {
                "Musubi next cursor does not bind its exact full page"
            }
        }
    }

    fun requireNonZeroDigest(value: MusubiDigest32V1, field: String) {
        require(value.bytes().any { it.toInt() != 0 }) { "$field must be non-zero" }
    }

    fun requireNonZeroFixed32(value: ByteArray, field: String) {
        require(value.size == 32 && value.any { it.toInt() != 0 }) {
            "$field must contain exactly 32 non-inert bytes"
        }
    }

    fun requireBoundedText(value: String, maximumBytes: Int, field: String) {
        requireExactText(value, field)
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        require(String(bytes, StandardCharsets.UTF_8) == value) {
            "$field must contain valid Unicode scalar values"
        }
        require(bytes.size <= maximumBytes) { "$field exceeds $maximumBytes UTF-8 bytes" }
    }

    fun requireChainId(value: String, field: String) {
        val bytes = value.toByteArray(StandardCharsets.US_ASCII)
        require(value.isNotEmpty() && value.length <= 128 &&
            String(bytes, StandardCharsets.US_ASCII) == value &&
            value.first().isLetterOrDigit() && value.last().isLetterOrDigit() &&
            value.all { it.isLetterOrDigit() || it == '.' || it == '_' || it == ':' || it == '-' }) {
            "$field is not a canonical ChainId"
        }
    }

    fun canonicalAccountPayload(value: String, field: String): ByteArray =
        TransferWirePayloadEncoder.encodeAccountIdPayload(requireCanonicalI105Address(value, field))

    fun canonicalPublicKeyPayload(value: String, field: String): ByteArray {
        val parsed = requireNotNull(decodePublicKeyLiteral(value)) {
            "$field must be a supported canonical public-key multihash"
        }
        require(encodePublicKeyMultihash(parsed.curveId, parsed.keyBytes) == value) {
            "$field must use the canonical public-key spelling"
        }
        return compactPublicKeyPayload(parsed.curveId, parsed.keyBytes)
    }

    fun canonicalSignaturePayload(value: String, field: String): ByteArray {
        require(value.isNotEmpty() && value.length % 2 == 0 &&
            value.all { it in '0'..'9' || it in 'A'..'F' } && value == value.uppercase()) {
            "$field must be canonical uppercase hexadecimal"
        }
        val bytes = ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
        require(bytes.any { it.toInt() != 0 }) { "$field must not be all zero" }
        return bytes
    }

    fun canonicalFixed32Hex(value: String, field: String): ByteArray {
        require(value.length == 64 && value.all { it in '0'..'9' || it in 'A'..'F' } &&
            value == value.uppercase()) {
            "$field must be exactly 64 uppercase hexadecimal characters"
        }
        val bytes = ByteArray(32) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
        require(bytes.any { it.toInt() != 0 }) { "$field must not be all zero" }
        return bytes
    }

    fun comparePackageIds(left: MusubiPackageIdV1, right: MusubiPackageIdV1): Int {
        left.homeDataspace.compareTo(right.homeDataspace).let { if (it != 0) return it }
        left.scope.kind.compareTo(right.scope.kind).let { if (it != 0) return it }
        if (left.scope.domain != null) {
            compareUnsignedBytes(
                left.scope.domain.toByteArray(StandardCharsets.UTF_8),
                requireNotNull(right.scope.domain).toByteArray(StandardCharsets.UTF_8),
            ).let { if (it != 0) return it }
        }
        return compareUnsignedBytes(
            left.name.value.toByteArray(StandardCharsets.UTF_8),
            right.name.value.toByteArray(StandardCharsets.UTF_8),
        )
    }

    fun namespaceMatchesScope(
        packageId: MusubiPackageIdV1,
        namespace: MusubiNamespaceV1,
    ): Boolean {
        val domain = namespace.value.substringBefore('.', missingDelimiterValue = "")
        return when (packageId.scope.kind) {
            MusubiPackageScopeV1.Kind.DATASPACE_ROOT -> domain.isEmpty()
            MusubiPackageScopeV1.Kind.DOMAIN -> packageId.scope.domain == domain
        }
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

    fun normalizedSearchTerms(query: String): Set<String> {
        val terms = sortedSetOf<String>()
        query.split(Regex("\\s+")).forEach { component ->
            if (
                component.toByteArray(StandardCharsets.UTF_8).size <= 64 &&
                component.all { it.isLetterOrDigit() && it.code < 128 || it == '-' }
            ) {
                terms += component.lowercase()
            }
            component.split(Regex("[^\\p{L}\\p{N}]+")).filter(String::isNotEmpty).forEach { word ->
                val normalized = word.lowercase()
                require(normalized.toByteArray(StandardCharsets.UTF_8).size <= 64) {
                    "Musubi search term exceeds 64 UTF-8 bytes"
                }
                terms += normalized
                require(terms.size <= 16) {
                    "Musubi search query exceeds 16 normalized terms"
                }
            }
        }
        require(terms.isNotEmpty() && terms.size <= 16) {
            "Musubi search query has no bounded normalized terms"
        }
        return terms
    }

    fun requireName(value: String, field: String) {
        requireExactText(value, field)
        require(value.toByteArray(StandardCharsets.UTF_8).size <= 255) { "$field exceeds 255 bytes" }
        require(value.none {
            it.isWhitespace() || isBidiControl(it) || it == '@' || it == '#' || it == '$'
        }) {
            "$field contains a forbidden character"
        }
        require(Normalizer.normalize(value, Normalizer.Form.NFC) == value) {
            "$field must be NFC-normalized"
        }
    }

    private fun isBidiControl(value: Char): Boolean =
        value == '\u061C' || value == '\u200E' || value == '\u200F' ||
            value in '\u202A'..'\u202E' || value in '\u2066'..'\u2069'

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
