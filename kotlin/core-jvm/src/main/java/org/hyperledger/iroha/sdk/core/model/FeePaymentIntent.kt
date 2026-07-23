package org.hyperledger.iroha.sdk.core.model

import java.text.Normalizer
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

/** Fee component constrained by a signature-bound maximum charge. */
enum class FeeChargeKind {
    NEXUS,
    PIPELINE_GAS,
}

/** Exact asset and maximum amount authorized for one fee component. */
class FeeChargeLimit(
    @JvmField val kind: FeeChargeKind,
    @JvmField val assetDefinitionId: String,
    @JvmField val maxAmount: String,
) {
    init {
        require(AssetDefinitionIdEncoder.isCanonicalAddress(assetDefinitionId)) {
            "assetDefinitionId must be a canonical unprefixed Base58 asset definition id"
        }
        val amount = KotodamaQuantity.parseCanonical(maxAmount)
        require(amount.mantissa.signum() > 0) { "maxAmount must be positive" }
    }

    override fun equals(other: Any?): Boolean =
        other is FeeChargeLimit &&
            kind == other.kind &&
            assetDefinitionId == other.assetDefinitionId &&
            maxAmount == other.maxAmount

    override fun hashCode(): Int {
        var result = kind.hashCode()
        result = 31 * result + assetDefinitionId.hashCode()
        result = 31 * result + maxAmount.hashCode()
        return result
    }
}

/** Exact immutable sponsor-program identifier. */
class FeeSponsorProgramId(
    @JvmField val sponsor: String,
    @JvmField val name: String,
) {
    init {
        requireCanonicalI105Address(sponsor, "sponsor")
        require(name.isNotEmpty()) { "program name must not be empty" }
        require(name.none { it.isWhitespace() || it == '@' || it == '#' || it == '$' || it == '/' }) {
            "program name contains a reserved character"
        }
        require(Normalizer.normalize(name, Normalizer.Form.NFC) == name) {
            "program name must use NFC normalization"
        }
    }

    /** Canonical `sponsor/program` selector used by Torii routes. */
    fun literal(): String = "$sponsor/$name"

    override fun toString(): String = literal()

    override fun equals(other: Any?): Boolean =
        other is FeeSponsorProgramId && sponsor == other.sponsor && name == other.name

    override fun hashCode(): Int = 31 * sponsor.hashCode() + name.hashCode()

    companion object {
        /** Parse an exact `sponsor/program` selector without trimming or rewriting it. */
        @JvmStatic
        fun parse(literal: String): FeeSponsorProgramId {
            require(literal.trim() == literal) { "programId must not contain surrounding whitespace" }
            val slash = literal.indexOf('/')
            require(slash > 0 && slash == literal.lastIndexOf('/') && slash < literal.length - 1) {
                "programId must use sponsor/program"
            }
            return FeeSponsorProgramId(literal.substring(0, slash), literal.substring(slash + 1))
        }
    }
}

/** Required signature-bound choice of fee payer, charge maxima, and executable gas bound. */
sealed class FeePaymentIntent private constructor(
    chargeLimits: List<FeeChargeLimit>,
    @JvmField val gasLimit: Long?,
) {
    private val _chargeLimits = chargeLimits.toList()

    /** Canonically ordered component maxima. */
    val chargeLimits: List<FeeChargeLimit> get() = _chargeLimits.toList()

    init {
        if (gasLimit != null) require(gasLimit > 0) { "gasLimit must be positive when present" }
        var previous = -1
        _chargeLimits.forEach { limit ->
            val current = when (limit.kind) {
                FeeChargeKind.NEXUS -> 0
                FeeChargeKind.PIPELINE_GAS -> 1
            }
            require(current > previous) {
                "chargeLimits must be unique and ordered nexus before pipeline gas"
            }
            previous = current
        }
    }

    /** Authority-paid intent. */
    class Authority(
        chargeLimits: List<FeeChargeLimit>,
        gasLimit: Long? = null,
    ) : FeePaymentIntent(chargeLimits, gasLimit) {
        override fun equals(other: Any?): Boolean =
            other is Authority && chargeLimits == other.chargeLimits && gasLimit == other.gasLimit

        override fun hashCode(): Int = 31 * chargeLimits.hashCode() + (gasLimit?.hashCode() ?: 0)
    }

    /** Exact sponsor-program revision intent. */
    class Sponsor(
        @JvmField val programId: FeeSponsorProgramId,
        @JvmField val programRevision: Long,
        chargeLimits: List<FeeChargeLimit>,
        gasLimit: Long? = null,
    ) : FeePaymentIntent(chargeLimits, gasLimit) {
        init {
            require(programRevision > 0) { "programRevision must be positive" }
        }

        override fun equals(other: Any?): Boolean =
            other is Sponsor &&
                programId == other.programId &&
                programRevision == other.programRevision &&
                chargeLimits == other.chargeLimits &&
                gasLimit == other.gasLimit

        override fun hashCode(): Int {
            var result = programId.hashCode()
            result = 31 * result + programRevision.hashCode()
            result = 31 * result + chargeLimits.hashCode()
            result = 31 * result + (gasLimit?.hashCode() ?: 0)
            return result
        }
    }

    /** True only when a quote preserves the exact payer, revision, and signed gas bound. */
    fun hasSamePayerAndGasBound(other: FeePaymentIntent): Boolean {
        if (gasLimit != other.gasLimit) return false
        return when {
            this is Authority && other is Authority -> true
            this is Sponsor && other is Sponsor ->
                programId == other.programId && programRevision == other.programRevision
            else -> false
        }
    }

    /** Exact Norito JSON object used by Torii request bodies and native bridges. */
    fun toJsonMap(): Map<String, Any?> {
        val value = LinkedHashMap<String, Any?>()
        if (this is Sponsor) {
            value["program_id"] = linkedMapOf("sponsor" to programId.sponsor, "name" to programId.name)
            value["program_revision"] = programRevision
        }
        value["charge_limits"] = chargeLimits.map { limit ->
            linkedMapOf(
                "kind" to linkedMapOf(
                    "kind" to when (limit.kind) {
                        FeeChargeKind.NEXUS -> "nexus"
                        FeeChargeKind.PIPELINE_GAS -> "pipeline_gas"
                    },
                    "value" to null,
                ),
                "asset_definition_id" to limit.assetDefinitionId,
                "max_amount" to limit.maxAmount,
            )
        }
        value["gas_limit"] = gasLimit
        return linkedMapOf(
            "payer" to if (this is Authority) "authority" else "sponsor",
            "value" to value,
        )
    }

    companion object {
        /** Construct an authority-paid intent. */
        @JvmStatic
        @JvmOverloads
        fun authority(chargeLimits: List<FeeChargeLimit>, gasLimit: Long? = null): FeePaymentIntent =
            Authority(chargeLimits, gasLimit)

        /** Construct an exact sponsor-program revision intent. */
        @JvmStatic
        @JvmOverloads
        fun sponsor(
            programId: FeeSponsorProgramId,
            programRevision: Long,
            chargeLimits: List<FeeChargeLimit>,
            gasLimit: Long? = null,
        ): FeePaymentIntent = Sponsor(programId, programRevision, chargeLimits, gasLimit)
    }
}
