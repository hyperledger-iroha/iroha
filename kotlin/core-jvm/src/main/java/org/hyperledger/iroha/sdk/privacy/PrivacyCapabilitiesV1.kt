// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

@file:Suppress("DEPRECATION")

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonParser

private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
private val U32_MAX = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE)
private val POLICY_DELAY_BLOCKS_V1 = BigInteger.valueOf(300L)
private val CONSENSUS_LIMIT_MAXIMA_V1 = linkedMapOf(
    "max_actions_per_transaction" to 1,
    "max_actions_per_block" to 2,
    "max_proof_bytes_per_action" to 9 * 1024 * 1024,
    "max_action_bytes" to 9 * 1024 * 1024,
    "max_privacy_bytes_per_transaction" to 9 * 1024 * 1024,
    "max_privacy_bytes_per_block" to 18 * 1024 * 1024,
    "max_statement_and_encrypted_output_bytes_per_transaction" to 256 * 1024,
    "max_nullifiers_per_action" to 8,
    "max_commitments_per_action" to 8,
    "retained_root_count" to 2048,
)

internal data class PrivacyProtocolLimitRuleV1(
    val name: String,
    val maximum: Int,
    val permitted: Set<Int>? = null,
)

internal fun privacyProtocolLimitRulesV1(
    protocol: PrivacyProtocolIdV1,
): List<PrivacyProtocolLimitRuleV1> = when (protocol) {
    PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1 -> listOf(
        PrivacyProtocolLimitRuleV1("max_anonymity_set_size", 64, setOf(16, 32, 64)),
        PrivacyProtocolLimitRuleV1("max_recipient_count", 8),
    )
    PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1 ->
        listOf(PrivacyProtocolLimitRuleV1("max_aggregation_count", 8))
    PrivacyProtocolIdV1.IROHA_ZK_AMS_V1 -> listOf(
        PrivacyProtocolLimitRuleV1("max_batch_size", 8),
        PrivacyProtocolLimitRuleV1("max_ring_size", 64, setOf(16, 32, 64)),
    )
    PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0 ->
        listOf(PrivacyProtocolLimitRuleV1("max_polynomial_count", 4))
    PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1 ->
        listOf(PrivacyProtocolLimitRuleV1("max_action_count", 2))
    PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1 -> listOf(
        PrivacyProtocolLimitRuleV1("max_input_count", 2),
        PrivacyProtocolLimitRuleV1("max_output_count", 4),
    )
    PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
    PrivacyProtocolIdV1.PQ_MASP_STARK_V0,
    -> listOf(
        PrivacyProtocolLimitRuleV1("max_input_count", 2),
        PrivacyProtocolLimitRuleV1("max_output_count", 2),
    )
    else -> emptyList()
}

/** Immutable exact 32-byte non-zero privacy binding. */
class PrivacyFixed32V1(bytes: ByteArray) {
    private val value = bytes.copyOf()

    init {
        require(value.size == 32) { "privacy binding must contain exactly 32 bytes" }
        require(value.any { it.toInt() != 0 }) { "privacy binding must not be all zero" }
    }

    fun bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is PrivacyFixed32V1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    override fun toString(): String = value.joinToString("") { "%02x".format(it.toInt() and 0xff) }
}

/** Chain-wide first-release admission limits. */
class PrivacyConsensusLimitsV1(
    @JvmField val maxActionsPerTransaction: Int,
    @JvmField val maxActionsPerBlock: Int,
    @JvmField val maxProofBytesPerAction: Int,
    @JvmField val maxActionBytes: Int,
    @JvmField val maxPrivacyBytesPerTransaction: Int,
    @JvmField val maxPrivacyBytesPerBlock: Int,
    @JvmField val maxStatementAndEncryptedOutputBytesPerTransaction: Int,
    @JvmField val maxNullifiersPerAction: Int,
    @JvmField val maxCommitmentsPerAction: Int,
    @JvmField val retainedRootCount: Int,
) {
    init {
        requireValidPrivacyConsensusLimitsV1(this)
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyConsensusLimitsV1 &&
            maxActionsPerTransaction == other.maxActionsPerTransaction &&
            maxActionsPerBlock == other.maxActionsPerBlock &&
            maxProofBytesPerAction == other.maxProofBytesPerAction &&
            maxActionBytes == other.maxActionBytes &&
            maxPrivacyBytesPerTransaction == other.maxPrivacyBytesPerTransaction &&
            maxPrivacyBytesPerBlock == other.maxPrivacyBytesPerBlock &&
            maxStatementAndEncryptedOutputBytesPerTransaction ==
            other.maxStatementAndEncryptedOutputBytesPerTransaction &&
            maxNullifiersPerAction == other.maxNullifiersPerAction &&
            maxCommitmentsPerAction == other.maxCommitmentsPerAction &&
            retainedRootCount == other.retainedRootCount

    override fun hashCode(): Int {
        var result = maxActionsPerTransaction
        result = 31 * result + maxActionsPerBlock
        result = 31 * result + maxProofBytesPerAction
        result = 31 * result + maxActionBytes
        result = 31 * result + maxPrivacyBytesPerTransaction
        result = 31 * result + maxPrivacyBytesPerBlock
        result = 31 * result + maxStatementAndEncryptedOutputBytesPerTransaction
        result = 31 * result + maxNullifiersPerAction
        result = 31 * result + maxCommitmentsPerAction
        return 31 * result + retainedRootCount
    }
}

class PrivacyConsensusPolicyTighteningV1(
    @JvmField val scheduledAtHeight: BigInteger,
    @JvmField val effectiveAtHeight: BigInteger,
    @JvmField val nextLimits: PrivacyConsensusLimitsV1,
) {
    init {
        requireValidPrivacyPolicyScheduleV1(
            scheduledAtHeight,
            effectiveAtHeight,
            "privacy consensus-policy tightening",
        )
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyConsensusPolicyTighteningV1 &&
            scheduledAtHeight == other.scheduledAtHeight &&
            effectiveAtHeight == other.effectiveAtHeight &&
            nextLimits == other.nextLimits

    override fun hashCode(): Int {
        var result = scheduledAtHeight.hashCode()
        result = 31 * result + effectiveAtHeight.hashCode()
        return 31 * result + nextLimits.hashCode()
    }
}

class PrivacyConsensusPolicyV1(
    @JvmField val currentLimits: PrivacyConsensusLimitsV1,
    @JvmField val pendingTightening: PrivacyConsensusPolicyTighteningV1?,
) {
    init {
        pendingTightening?.let { pending ->
            requireStrictConsensusTighteningV1(
                currentLimits,
                pending.nextLimits,
                "privacy consensus-policy tightening",
            )
        }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyConsensusPolicyV1 &&
            currentLimits == other.currentLimits &&
            pendingTightening == other.pendingTightening

    override fun hashCode(): Int =
        31 * currentLimits.hashCode() + (pendingTightening?.hashCode() ?: 0)
}

/** Closed protocol-tagged verifier ceilings. */
class PrivacyProtocolLimitsV1(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    values: Map<String, Int>?,
) {
    @JvmField
    val values: Map<String, Int>? = values?.let {
        Collections.unmodifiableMap(LinkedHashMap(it))
    }

    init {
        val rules = privacyProtocolLimitRulesV1(protocolId)
        if (rules.isEmpty()) {
            require(this.values == null) {
                "fixed privacy protocol limits must not carry fields"
            }
        } else {
            val actual = requireNotNull(this.values) {
                "privacy protocol limits must carry their closed fields"
            }
            require(actual.keys == rules.mapTo(linkedSetOf()) { it.name }) {
                "privacy protocol limits contain missing or unknown fields"
            }
            for (rule in rules) {
                val value = requireNotNull(actual[rule.name]) {
                    "privacy protocol limit ${rule.name} is missing"
                }
                require(value > 0 && value <= rule.maximum) {
                    "privacy protocol limit ${rule.name} is outside its first-release bound"
                }
                require(rule.permitted == null || value in rule.permitted) {
                    "privacy protocol limit ${rule.name} is outside its closed first-release set"
                }
            }
        }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyProtocolLimitsV1 &&
            protocolId == other.protocolId &&
            values == other.values

    override fun hashCode(): Int = 31 * protocolId.hashCode() + (values?.hashCode() ?: 0)
}

class PrivacyCompiledProfileV1(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val proofSystemId: PrivacyProofSystemIdV1,
    @JvmField val engineId: PrivacyEngineIdV1,
    @JvmField val parameterId: PrivacyFixed32V1,
    @JvmField val parameterDigest: PrivacyFixed32V1,
    @JvmField val verifierDigest: PrivacyFixed32V1,
    @JvmField val statementSchemaDigest: PrivacyFixed32V1,
    @JvmField val engineManifestDigest: PrivacyFixed32V1,
    @JvmField val protocolLimits: PrivacyProtocolLimitsV1,
) {
    init {
        require(proofSystemId == protocolId.expectedProofSystem) {
            "privacy compiled profile proof system does not match its protocol"
        }
        require(engineId == protocolId.expectedEngine) {
            "privacy compiled profile engine does not match its protocol"
        }
        require(protocolLimits.protocolId == protocolId) {
            "privacy compiled profile limits do not match its protocol"
        }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyCompiledProfileV1 &&
            protocolId == other.protocolId &&
            proofSystemId == other.proofSystemId &&
            engineId == other.engineId &&
            parameterId == other.parameterId &&
            parameterDigest == other.parameterDigest &&
            verifierDigest == other.verifierDigest &&
            statementSchemaDigest == other.statementSchemaDigest &&
            engineManifestDigest == other.engineManifestDigest &&
            protocolLimits == other.protocolLimits

    override fun hashCode(): Int {
        var result = protocolId.hashCode()
        result = 31 * result + proofSystemId.hashCode()
        result = 31 * result + engineId.hashCode()
        result = 31 * result + parameterId.hashCode()
        result = 31 * result + parameterDigest.hashCode()
        result = 31 * result + verifierDigest.hashCode()
        result = 31 * result + statementSchemaDigest.hashCode()
        result = 31 * result + engineManifestDigest.hashCode()
        return 31 * result + protocolLimits.hashCode()
    }
}

enum class PrivacyCompiledProfileUnavailableReasonV1 {
    ENGINE_UNAVAILABLE,
    PROFILE_INITIALIZATION_FAILED,
    STATEMENT_SCHEMA_INVALID,
}

enum class PrivacyCompiledStatementSchemaErrorV1 {
    CONFLICTING_STABLE_TYPE_ID,
    MISSING_TYPE_REFERENCE,
}

sealed class PrivacyCompiledProfileResultV1 {
    class Available(@JvmField val profile: PrivacyCompiledProfileV1) :
        PrivacyCompiledProfileResultV1() {
        override fun equals(other: Any?): Boolean =
            other is Available && profile == other.profile

        override fun hashCode(): Int = profile.hashCode()
    }

    class Unavailable(
        @JvmField val reason: PrivacyCompiledProfileUnavailableReasonV1,
        @JvmField val statementSchemaError: PrivacyCompiledStatementSchemaErrorV1?,
    ) : PrivacyCompiledProfileResultV1() {
        init {
            require(
                (reason == PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID) ==
                    (statementSchemaError != null),
            ) {
                "statement-schema detail must be present exactly for statement-schema-invalid"
            }
        }

        override fun equals(other: Any?): Boolean =
            other is Unavailable &&
                reason == other.reason &&
                statementSchemaError == other.statementSchemaError

        override fun hashCode(): Int =
            31 * reason.hashCode() + (statementSchemaError?.hashCode() ?: 0)
    }
}

enum class PrivacyProtocolLifecycleStateV1 {
    PROPOSED,
    ACTIVE,
    SUSPENDED,
    RETIRED,
}

/** Closed first-release assurance classification. */
enum class PrivacyAssuranceV1 {
    EXPERIMENTAL,
}

class PrivacyProtocolLifecycleV1(
    @JvmField val state: PrivacyProtocolLifecycleStateV1,
    @JvmField val proposedAtHeight: BigInteger,
    @JvmField val activateAtHeight: BigInteger?,
    @JvmField val activatedAtHeight: BigInteger?,
    @JvmField val stateSinceHeight: BigInteger?,
) {
    init {
        requirePositivePrivacyHeightV1(proposedAtHeight, "privacy proposal height")
        when (state) {
            PrivacyProtocolLifecycleStateV1.PROPOSED -> {
                val activate = requireNotNull(activateAtHeight) {
                    "proposed privacy lifecycle must carry activate-at height"
                }
                require(activatedAtHeight == null && stateSinceHeight == null) {
                    "proposed privacy lifecycle must not carry activated-at or state-since heights"
                }
                requirePositivePrivacyHeightV1(activate, "privacy activate-at height")
                require(activate > proposedAtHeight) {
                    "privacy activate-at height must be later than proposal height"
                }
            }
            PrivacyProtocolLifecycleStateV1.ACTIVE -> {
                require(activateAtHeight == null) {
                    "active privacy lifecycle must not carry activate-at height"
                }
                val activated = requireNotNull(activatedAtHeight) {
                    "active privacy lifecycle must carry activated-at height"
                }
                val since = requireNotNull(stateSinceHeight) {
                    "active privacy lifecycle must carry state-since height"
                }
                requirePositivePrivacyHeightV1(activated, "privacy activated-at height")
                requirePositivePrivacyHeightV1(since, "privacy state-since height")
                require(activated > proposedAtHeight && since >= activated) {
                    "active privacy lifecycle heights are out of order"
                }
            }
            PrivacyProtocolLifecycleStateV1.SUSPENDED -> {
                require(activateAtHeight == null) {
                    "suspended privacy lifecycle must not carry activate-at height"
                }
                val activated = requireNotNull(activatedAtHeight) {
                    "suspended privacy lifecycle must carry activated-at height"
                }
                val since = requireNotNull(stateSinceHeight) {
                    "suspended privacy lifecycle must carry state-since height"
                }
                requirePositivePrivacyHeightV1(activated, "privacy activated-at height")
                requirePositivePrivacyHeightV1(since, "privacy state-since height")
                require(activated > proposedAtHeight && since > activated) {
                    "suspended privacy lifecycle heights are out of order"
                }
            }
            PrivacyProtocolLifecycleStateV1.RETIRED -> {
                require(activateAtHeight == null) {
                    "retired privacy lifecycle must not carry activate-at height"
                }
                val since = requireNotNull(stateSinceHeight) {
                    "retired privacy lifecycle must carry state-since height"
                }
                requirePositivePrivacyHeightV1(since, "privacy state-since height")
                activatedAtHeight?.let { activated ->
                    requirePositivePrivacyHeightV1(activated, "privacy activated-at height")
                    require(activated > proposedAtHeight && since > activated) {
                        "retired privacy lifecycle heights are out of order"
                    }
                } ?: require(since > proposedAtHeight) {
                    "unactivated retired privacy lifecycle must retire after proposal"
                }
            }
        }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyProtocolLifecycleV1 &&
            state == other.state &&
            proposedAtHeight == other.proposedAtHeight &&
            activateAtHeight == other.activateAtHeight &&
            activatedAtHeight == other.activatedAtHeight &&
            stateSinceHeight == other.stateSinceHeight

    override fun hashCode(): Int {
        var result = state.hashCode()
        result = 31 * result + proposedAtHeight.hashCode()
        result = 31 * result + (activateAtHeight?.hashCode() ?: 0)
        result = 31 * result + (activatedAtHeight?.hashCode() ?: 0)
        return 31 * result + (stateSinceHeight?.hashCode() ?: 0)
    }
}

class PrivacyProtocolLimitsTighteningV1(
    @JvmField val scheduledAtHeight: BigInteger,
    @JvmField val effectiveAtHeight: BigInteger,
    @JvmField val nextLimits: PrivacyProtocolLimitsV1,
) {
    init {
        requireValidPrivacyPolicyScheduleV1(
            scheduledAtHeight,
            effectiveAtHeight,
            "privacy protocol-limit tightening",
        )
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyProtocolLimitsTighteningV1 &&
            scheduledAtHeight == other.scheduledAtHeight &&
            effectiveAtHeight == other.effectiveAtHeight &&
            nextLimits == other.nextLimits

    override fun hashCode(): Int {
        var result = scheduledAtHeight.hashCode()
        result = 31 * result + effectiveAtHeight.hashCode()
        return 31 * result + nextLimits.hashCode()
    }
}

class PrivacyProtocolActivationRecordV1(
    @JvmField val profileBindings: PrivacyCompiledProfileV1,
    @JvmField val lifecycle: PrivacyProtocolLifecycleV1,
    @JvmField val pendingProtocolLimitsTightening: PrivacyProtocolLimitsTighteningV1?,
    @JvmField val assurance: PrivacyAssuranceV1,
) {
    init {
        require(assurance == PrivacyAssuranceV1.EXPERIMENTAL) {
            "privacy activation assurance must be experimental in the first release"
        }
        pendingProtocolLimitsTightening?.let { pending ->
            require(pending.nextLimits.protocolId == profileBindings.protocolId) {
                "privacy protocol-limit tightening does not match its activation protocol"
            }
            requireStrictProtocolTighteningV1(
                profileBindings.protocolLimits,
                pending.nextLimits,
                "privacy protocol-limit tightening",
            )
        }
    }

    override fun equals(other: Any?): Boolean =
        other is PrivacyProtocolActivationRecordV1 &&
            profileBindings == other.profileBindings &&
            lifecycle == other.lifecycle &&
            pendingProtocolLimitsTightening == other.pendingProtocolLimitsTightening &&
            assurance == other.assurance

    override fun hashCode(): Int {
        var result = profileBindings.hashCode()
        result = 31 * result + lifecycle.hashCode()
        result = 31 * result + (pendingProtocolLimitsTightening?.hashCode() ?: 0)
        return 31 * result + assurance.hashCode()
    }
}

/**
 * Retired JSON snapshot row retained only for explicit legacy-payload inspection.
 *
 * This type is not live Torii capability state and cannot authorize privacy construction.
 */
@Deprecated(
    message = "Legacy JSON inspection only; use the Exact12 manifest and admission APIs for live capabilities",
)
class LegacyPrivacyCapabilityRowInspectionV1(
    @JvmField val protocolId: PrivacyProtocolIdV1,
    @JvmField val compiledProfile: PrivacyCompiledProfileResultV1,
    @JvmField val activation: PrivacyProtocolActivationRecordV1?,
) {
    init {
        val available = compiledProfile as? PrivacyCompiledProfileResultV1.Available
        require(available == null || available.profile.protocolId == protocolId) {
            "legacy privacy inspection row does not match its compiled-profile protocol"
        }
        require(activation == null || available != null) {
            "unavailable legacy privacy inspection row cannot carry an activation"
        }
        activation?.let { governed ->
            val compiled = requireNotNull(available).profile
            require(governed.profileBindings.protocolId == protocolId) {
                "legacy privacy inspection row does not match its activation protocol"
            }
            requirePrivacyProfileBindingsEqualV1(
                governed.profileBindings,
                compiled,
                "legacy privacy inspection activation",
            )
            requireProtocolLimitsAtMostV1(
                governed.profileBindings.protocolLimits,
                compiled.protocolLimits,
                "legacy privacy inspection activation limits",
            )
        }
    }

    override fun equals(other: Any?): Boolean =
        other is LegacyPrivacyCapabilityRowInspectionV1 &&
            protocolId == other.protocolId &&
            compiledProfile == other.compiledProfile &&
            activation == other.activation

    override fun hashCode(): Int {
        var result = protocolId.hashCode()
        result = 31 * result + compiledProfile.hashCode()
        return 31 * result + (activation?.hashCode() ?: 0)
    }
}

/**
 * Retired JSON snapshot retained only for explicit legacy-payload inspection.
 *
 * It is neither authoritative nor accepted by the live Exact12 admission path.
 */
@Deprecated(
    message = "Legacy JSON inspection only; use the Exact12 manifest and admission APIs for live capabilities",
)
class LegacyPrivacyCapabilitySnapshotInspectionV1(
    @JvmField val version: Int,
    @JvmField val committedHeight: BigInteger,
    @JvmField val consensusPolicy: PrivacyConsensusPolicyV1,
    protocols: List<LegacyPrivacyCapabilityRowInspectionV1>,
) {
    @JvmField
    val protocols: List<LegacyPrivacyCapabilityRowInspectionV1>

    init {
        require(version == LegacyPrivacyCapabilitySnapshotJsonInspectionV1.VERSION) {
            "legacy privacy capability inspection version must be ${LegacyPrivacyCapabilitySnapshotJsonInspectionV1.VERSION}"
        }
        requirePrivacyHeightV1(committedHeight, "legacy privacy inspection committed height")
        requirePrivacyConsensusPolicyAtHeightV1(consensusPolicy, committedHeight)
        val expected = PrivacyProtocolIdV1.values()
        require(protocols.size == expected.size) {
            "legacy privacy capability inspection must contain exactly ${expected.size} rows"
        }
        protocols.forEachIndexed { index, row ->
            require(row.protocolId == expected[index]) {
                "legacy privacy capability inspection row $index is out of canonical protocol order"
            }
            requirePrivacyCapabilityRowAtHeightV1(row, committedHeight)
        }
        this.protocols = Collections.unmodifiableList(protocols.toList())
    }

    override fun equals(other: Any?): Boolean =
        other is LegacyPrivacyCapabilitySnapshotInspectionV1 &&
            version == other.version &&
            committedHeight == other.committedHeight &&
            consensusPolicy == other.consensusPolicy &&
            protocols == other.protocols

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + committedHeight.hashCode()
        result = 31 * result + consensusPolicy.hashCode()
        return 31 * result + protocols.hashCode()
    }
}

private fun requireValidPrivacyConsensusLimitsV1(limits: PrivacyConsensusLimitsV1) {
    val values = privacyConsensusLimitValuesV1(limits)
    for ((name, value) in values) {
        require(value > 0) { "privacy consensus limit $name must be non-zero" }
        require(value <= CONSENSUS_LIMIT_MAXIMA_V1.getValue(name)) {
            "privacy consensus limit $name exceeds its first-release hard maximum"
        }
    }
    require(limits.maxActionsPerTransaction <= limits.maxActionsPerBlock) {
        "privacy actions-per-transaction limit exceeds actions-per-block limit"
    }
    require(limits.maxProofBytesPerAction <= limits.maxActionBytes) {
        "privacy proof-bytes-per-action limit exceeds action-bytes limit"
    }
    require(limits.maxActionBytes <= limits.maxPrivacyBytesPerTransaction) {
        "privacy action-bytes limit exceeds privacy-bytes-per-transaction limit"
    }
    require(limits.maxPrivacyBytesPerTransaction <= limits.maxPrivacyBytesPerBlock) {
        "privacy bytes-per-transaction limit exceeds privacy-bytes-per-block limit"
    }
    require(
        limits.maxStatementAndEncryptedOutputBytesPerTransaction <= limits.maxActionBytes,
    ) {
        "privacy statement-and-output limit exceeds action-bytes limit"
    }
}

private fun privacyConsensusLimitValuesV1(limits: PrivacyConsensusLimitsV1): Map<String, Int> =
    linkedMapOf(
        "max_actions_per_transaction" to limits.maxActionsPerTransaction,
        "max_actions_per_block" to limits.maxActionsPerBlock,
        "max_proof_bytes_per_action" to limits.maxProofBytesPerAction,
        "max_action_bytes" to limits.maxActionBytes,
        "max_privacy_bytes_per_transaction" to limits.maxPrivacyBytesPerTransaction,
        "max_privacy_bytes_per_block" to limits.maxPrivacyBytesPerBlock,
        "max_statement_and_encrypted_output_bytes_per_transaction" to
            limits.maxStatementAndEncryptedOutputBytesPerTransaction,
        "max_nullifiers_per_action" to limits.maxNullifiersPerAction,
        "max_commitments_per_action" to limits.maxCommitmentsPerAction,
        "retained_root_count" to limits.retainedRootCount,
    )

private fun requireStrictConsensusTighteningV1(
    current: PrivacyConsensusLimitsV1,
    next: PrivacyConsensusLimitsV1,
    subject: String,
) {
    val currentValues = privacyConsensusLimitValuesV1(current)
    val nextValues = privacyConsensusLimitValuesV1(next)
    require(currentValues.keys == nextValues.keys) { "$subject fields do not match" }
    var changed = false
    for ((name, currentValue) in currentValues) {
        val nextValue = nextValues.getValue(name)
        require(nextValue <= currentValue) { "$subject cannot increase $name" }
        changed = changed || nextValue != currentValue
    }
    require(changed) { "$subject must be a strict tightening" }
}

private fun requirePrivacyHeightV1(height: BigInteger, subject: String) {
    require(height.signum() >= 0 && height <= U64_MAX) {
        "$subject must be within the uint64 range"
    }
}

private fun requirePositivePrivacyHeightV1(height: BigInteger, subject: String) {
    requirePrivacyHeightV1(height, subject)
    require(height.signum() > 0) { "$subject must be non-zero" }
}

private fun requireValidPrivacyPolicyScheduleV1(
    scheduledAtHeight: BigInteger,
    effectiveAtHeight: BigInteger,
    subject: String,
) {
    requirePositivePrivacyHeightV1(scheduledAtHeight, "$subject scheduled-at height")
    requirePositivePrivacyHeightV1(effectiveAtHeight, "$subject effective-at height")
    require(scheduledAtHeight <= U64_MAX.subtract(POLICY_DELAY_BLOCKS_V1)) {
        "$subject scheduled-at height overflows the notice window"
    }
    require(effectiveAtHeight >= scheduledAtHeight.add(POLICY_DELAY_BLOCKS_V1)) {
        "$subject must provide at least ${POLICY_DELAY_BLOCKS_V1} blocks of notice"
    }
}

private fun requireProtocolLimitsAtMostV1(
    actual: PrivacyProtocolLimitsV1,
    ceiling: PrivacyProtocolLimitsV1,
    subject: String,
) {
    require(actual.protocolId == ceiling.protocolId) {
        "$subject protocol tag does not match its ceiling"
    }
    require((actual.values == null) == (ceiling.values == null)) {
        "$subject shape does not match its ceiling"
    }
    val actualValues = actual.values ?: return
    val ceilingValues = requireNotNull(ceiling.values)
    for ((name, value) in actualValues) {
        require(value <= ceilingValues.getValue(name)) { "$subject exceeds $name ceiling" }
    }
}

private fun requireStrictProtocolTighteningV1(
    current: PrivacyProtocolLimitsV1,
    next: PrivacyProtocolLimitsV1,
    subject: String,
) {
    requireProtocolLimitsAtMostV1(next, current, subject)
    require(next != current) { "$subject must be strict" }
}

private fun requirePrivacyProfileBindingsEqualV1(
    actual: PrivacyCompiledProfileV1,
    expected: PrivacyCompiledProfileV1,
    subject: String,
) {
    require(
        actual.protocolId == expected.protocolId &&
            actual.proofSystemId == expected.proofSystemId &&
            actual.engineId == expected.engineId &&
            actual.parameterId == expected.parameterId &&
            actual.parameterDigest == expected.parameterDigest &&
            actual.verifierDigest == expected.verifierDigest &&
            actual.statementSchemaDigest == expected.statementSchemaDigest &&
            actual.engineManifestDigest == expected.engineManifestDigest,
    ) { "$subject bindings do not match the compiled profile" }
}

private fun requirePrivacyConsensusPolicyAtHeightV1(
    policy: PrivacyConsensusPolicyV1,
    committedHeight: BigInteger,
) {
    policy.pendingTightening?.let { pending ->
        require(pending.scheduledAtHeight <= committedHeight) {
            "privacy consensus-policy tightening was scheduled after the committed height"
        }
        require(pending.effectiveAtHeight > committedHeight) {
            "privacy consensus-policy tightening is already due at the committed height"
        }
    }
}

private fun requirePrivacyCapabilityRowAtHeightV1(
    row: LegacyPrivacyCapabilityRowInspectionV1,
    committedHeight: BigInteger,
) {
    val activation = row.activation ?: return
    val lifecycle = activation.lifecycle
    require(lifecycle.proposedAtHeight <= committedHeight) {
        "privacy proposal height is after the committed height"
    }
    when (lifecycle.state) {
        PrivacyProtocolLifecycleStateV1.PROPOSED -> {
            require(requireNotNull(lifecycle.activateAtHeight) > committedHeight) {
                "due privacy proposal remained unpromoted at the committed height"
            }
        }
        PrivacyProtocolLifecycleStateV1.ACTIVE,
        PrivacyProtocolLifecycleStateV1.SUSPENDED,
        PrivacyProtocolLifecycleStateV1.RETIRED,
        -> {
            lifecycle.activatedAtHeight?.let { activated ->
                require(activated <= committedHeight) {
                    "privacy activation height is after the committed height"
                }
            }
            require(requireNotNull(lifecycle.stateSinceHeight) <= committedHeight) {
                "privacy lifecycle state height is after the committed height"
            }
        }
    }
    activation.pendingProtocolLimitsTightening?.let { pending ->
        require(pending.scheduledAtHeight <= committedHeight) {
            "privacy protocol-limit tightening was scheduled after the committed height"
        }
        require(pending.effectiveAtHeight > committedHeight) {
            "privacy protocol-limit tightening is already due at the committed height"
        }
    }
}

/** Parse failure raised only by the retired JSON snapshot inspection helper. */
@Deprecated(
    message = "Legacy JSON inspection only; live Exact12 responses use the native manifest decoder",
)
class LegacyPrivacyCapabilitySnapshotInspectionException(
    @JvmField val path: String,
    detail: String,
    cause: Throwable? = null,
) : IllegalArgumentException("$path: $detail", cause)

/**
 * Retired JSON decoder for inspecting historical capability payloads.
 *
 * Live `/v1/privacy/capabilities` responses are exact Norito manifests decoded by
 * `PrivacyNativeBridge`; this helper is not a transport decoder and cannot issue admission.
 */
@Deprecated(
    message = "Legacy JSON inspection only; use HttpClientTransport.getPrivacyCapabilities for live Exact12 state",
)
object LegacyPrivacyCapabilitySnapshotJsonInspectionV1 {
    const val VERSION: Int = 1
    const val MAX_RESPONSE_BYTES: Long = 256L * 1024L

    private val CONSENSUS_LIMIT_KEYS = CONSENSUS_LIMIT_MAXIMA_V1.keys
    private val CONSENSUS_MAXIMA = CONSENSUS_LIMIT_MAXIMA_V1

    @JvmStatic
    fun parse(payload: ByteArray): LegacyPrivacyCapabilitySnapshotInspectionV1 {
        require(payload.isNotEmpty()) { "legacy privacy inspection payload must not be empty" }
        require(payload.size.toLong() <= MAX_RESPONSE_BYTES) {
            "legacy privacy inspection payload exceeds $MAX_RESPONSE_BYTES bytes"
        }
        val json = String(payload, StandardCharsets.UTF_8)
        requireCanonicalUnsignedIntegerTokens(json)
        val decoded = try {
            JsonParser.parse(json)
        } catch (error: RuntimeException) {
            throw LegacyPrivacyCapabilitySnapshotInspectionException(
                "legacy privacy capability inspection",
                "contains invalid JSON",
                error,
            )
        }
        return parseValue(decoded)
    }

    @JvmStatic
    fun parse(json: String): LegacyPrivacyCapabilitySnapshotInspectionV1 =
        parse(json.toByteArray(StandardCharsets.UTF_8))

    private fun parseValue(value: Any?): LegacyPrivacyCapabilitySnapshotInspectionV1 {
        val path = "legacy privacy capability inspection"
        val root = exactObject(
            value,
            setOf("version", "committed_height", "consensus_policy", "protocols"),
            path,
        )
        val version = u32(root["version"], "$path.version")
        if (version != VERSION) fail("version must be exactly $VERSION", "$path.version")
        val committedHeight = u64(root["committed_height"], "$path.committed_height")
        val consensusPolicy = parseConsensusPolicy(root["consensus_policy"], committedHeight)
        val rows = list(root["protocols"], "$path.protocols")
        val expected = PrivacyProtocolIdV1.values()
        if (rows.size != expected.size) {
            fail("protocols must contain exactly ${expected.size} canonical rows", "$path.protocols")
        }
        val protocols = rows.mapIndexed { index, row ->
            parseCapabilityRow(
                row,
                expected[index],
                committedHeight,
                "$path.protocols[$index]",
            )
        }
        return LegacyPrivacyCapabilitySnapshotInspectionV1(
            version,
            committedHeight,
            consensusPolicy,
            protocols,
        )
    }

    private fun parseConsensusPolicy(value: Any?, committedHeight: BigInteger): PrivacyConsensusPolicyV1 {
        val path = "legacy privacy capability inspection.consensus_policy"
        val policy = exactObject(value, setOf("current_limits", "pending_tightening"), path)
        val current = parseConsensusLimits(policy["current_limits"], "$path.current_limits")
        val pendingValue = policy["pending_tightening"]
        val pending = if (pendingValue == null) {
            null
        } else {
            val pendingPath = "$path.pending_tightening"
            val tightening = exactObject(
                pendingValue,
                setOf("scheduled_at_height", "effective_at_height", "next_limits"),
                pendingPath,
            )
            val scheduled = positiveU64(tightening["scheduled_at_height"], "$pendingPath.scheduled_at_height")
            val effective = positiveU64(tightening["effective_at_height"], "$pendingPath.effective_at_height")
            validateSchedule(scheduled, effective, committedHeight, pendingPath)
            val next = parseConsensusLimits(tightening["next_limits"], "$pendingPath.next_limits")
            assertConsensusTightening(current, next, pendingPath)
            PrivacyConsensusPolicyTighteningV1(scheduled, effective, next)
        }
        return PrivacyConsensusPolicyV1(current, pending)
    }

    private fun parseConsensusLimits(value: Any?, path: String): PrivacyConsensusLimitsV1 {
        val limits = exactObject(value, CONSENSUS_LIMIT_KEYS, path)
        fun field(name: String): Int {
            val result = positiveU32(limits[name], "$path.$name")
            val maximum = CONSENSUS_MAXIMA.getValue(name)
            if (result > maximum) fail("exceeds the first-release hard maximum", "$path.$name")
            return result
        }
        val result = PrivacyConsensusLimitsV1(
            field("max_actions_per_transaction"),
            field("max_actions_per_block"),
            field("max_proof_bytes_per_action"),
            field("max_action_bytes"),
            field("max_privacy_bytes_per_transaction"),
            field("max_privacy_bytes_per_block"),
            field("max_statement_and_encrypted_output_bytes_per_transaction"),
            field("max_nullifiers_per_action"),
            field("max_commitments_per_action"),
            field("retained_root_count"),
        )
        if (
            result.maxActionsPerTransaction > result.maxActionsPerBlock ||
            result.maxProofBytesPerAction > result.maxActionBytes ||
            result.maxActionBytes > result.maxPrivacyBytesPerTransaction ||
            result.maxPrivacyBytesPerTransaction > result.maxPrivacyBytesPerBlock ||
            result.maxStatementAndEncryptedOutputBytesPerTransaction > result.maxActionBytes
        ) {
            fail("violates consensus resource-limit ordering", path)
        }
        return result
    }

    private fun parseCapabilityRow(
        value: Any?,
        expected: PrivacyProtocolIdV1,
        committedHeight: BigInteger,
        path: String,
    ): LegacyPrivacyCapabilityRowInspectionV1 {
        val row = exactObject(value, setOf("protocol_id", "compiled_profile", "activation"), path)
        val protocol = protocolTag(row["protocol_id"], "$path.protocol_id")
        if (protocol != expected) fail("must be canonical protocol ${expected.canonicalLabel}", "$path.protocol_id")
        val compiled = parseCompiledProfile(row["compiled_profile"], protocol, "$path.compiled_profile")
        val activation = row["activation"]?.let {
            parseActivation(it, protocol, compiled, committedHeight, "$path.activation")
        }
        if (activation != null && compiled !is PrivacyCompiledProfileResultV1.Available) {
            fail("cannot activate an unavailable compiled profile", "$path.activation")
        }
        return LegacyPrivacyCapabilityRowInspectionV1(protocol, compiled, activation)
    }

    private fun parseCompiledProfile(
        value: Any?,
        protocol: PrivacyProtocolIdV1,
        path: String,
    ): PrivacyCompiledProfileResultV1 {
        val result = exactObject(value, setOf("status", "value"), path)
        return when (text(result["status"], "$path.status")) {
            "available" -> PrivacyCompiledProfileResultV1.Available(
                parseProfile(result["value"], protocol, "$path.value"),
            )
            "unavailable" -> parseUnavailable(result["value"], "$path.value")
            else -> fail("status must be available or unavailable", "$path.status")
        }
    }

    private fun parseUnavailable(value: Any?, path: String): PrivacyCompiledProfileResultV1.Unavailable {
        val unavailable = exactObject(value, setOf("reason", "detail"), path)
        return when (text(unavailable["reason"], "$path.reason")) {
            "engine-unavailable" -> {
                if (unavailable["detail"] != null) fail("unit reason detail must be null", "$path.detail")
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
                    null,
                )
            }
            "profile-initialization-failed" -> {
                if (unavailable["detail"] != null) fail("unit reason detail must be null", "$path.detail")
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.PROFILE_INITIALIZATION_FAILED,
                    null,
                )
            }
            "statement-schema-invalid" -> {
                val tag = taggedUnit(
                    unavailable["detail"],
                    "schema_error",
                    "detail",
                    setOf("conflicting-stable-type-id", "missing-type-reference"),
                    "$path.detail",
                )
                PrivacyCompiledProfileResultV1.Unavailable(
                    PrivacyCompiledProfileUnavailableReasonV1.STATEMENT_SCHEMA_INVALID,
                    if (tag == "conflicting-stable-type-id") {
                        PrivacyCompiledStatementSchemaErrorV1.CONFLICTING_STABLE_TYPE_ID
                    } else {
                        PrivacyCompiledStatementSchemaErrorV1.MISSING_TYPE_REFERENCE
                    },
                )
            }
            else -> fail("unknown unavailable reason", "$path.reason")
        }
    }

    private fun parseProfile(value: Any?, protocol: PrivacyProtocolIdV1, path: String): PrivacyCompiledProfileV1 {
        val profile = exactObject(value, PROFILE_KEYS, path)
        return bindings(
            profile,
            protocol,
            parseProtocolLimits(profile["protocol_limits"], protocol, "$path.protocol_limits"),
            path,
        )
    }

    private fun parseActivation(
        value: Any?,
        protocol: PrivacyProtocolIdV1,
        compiled: PrivacyCompiledProfileResultV1,
        committedHeight: BigInteger,
        path: String,
    ): PrivacyProtocolActivationRecordV1 {
        val record = exactObject(value, ACTIVATION_KEYS, path)
        val limits = parseProtocolLimits(record["protocol_limits"], protocol, "$path.protocol_limits")
        val activationProfile = bindings(record, protocol, limits, path)
        val compiledProfile = (compiled as? PrivacyCompiledProfileResultV1.Available)?.profile
        if (compiledProfile != null) {
            assertProfileBindingsEqual(activationProfile, compiledProfile, path)
            assertLimitsAtMost(limits, compiledProfile.protocolLimits, "$path.protocol_limits")
        }
        val lifecycle = parseLifecycle(record["lifecycle"], committedHeight, "$path.lifecycle")
        val pending = parseProtocolTightening(
            record["pending_protocol_limits_tightening"],
            limits,
            committedHeight,
            "$path.pending_protocol_limits_tightening",
        )
        taggedUnit(record["assurance"], "assurance", "value", setOf("experimental"), "$path.assurance")
        return PrivacyProtocolActivationRecordV1(
            activationProfile,
            lifecycle,
            pending,
            PrivacyAssuranceV1.EXPERIMENTAL,
        )
    }

    private fun bindings(
        value: Map<String, Any?>,
        protocol: PrivacyProtocolIdV1,
        protocolLimits: PrivacyProtocolLimitsV1,
        path: String,
    ): PrivacyCompiledProfileV1 {
        val embeddedProtocol = protocolTag(value["protocol_id"], "$path.protocol_id")
        if (embeddedProtocol != protocol) fail("does not match its row protocol", "$path.protocol_id")
        val proofLabel = taggedUnit(
            value["proof_system_id"],
            "proof_system",
            "value",
            setOf(protocol.expectedProofSystem.canonicalLabel),
            "$path.proof_system_id",
        )
        val engineLabel = taggedUnit(
            value["engine_id"],
            "engine",
            "value",
            setOf(protocol.expectedEngine.canonicalLabel),
            "$path.engine_id",
        )
        return PrivacyCompiledProfileV1(
            protocol,
            PrivacyProofSystemIdV1.fromCanonicalLabel(proofLabel),
            PrivacyEngineIdV1.fromCanonicalLabel(engineLabel),
            fixed32(value["parameter_id"], "$path.parameter_id"),
            fixed32(value["parameter_digest"], "$path.parameter_digest"),
            fixed32(value["verifier_digest"], "$path.verifier_digest"),
            fixed32(value["statement_schema_digest"], "$path.statement_schema_digest"),
            fixed32(value["engine_manifest_digest"], "$path.engine_manifest_digest"),
            protocolLimits,
        )
    }

    private fun parseProtocolLimits(value: Any?, protocol: PrivacyProtocolIdV1, path: String): PrivacyProtocolLimitsV1 {
        val tagged = exactObject(value, setOf("protocol", "limits"), path)
        val tag = try {
            PrivacyProtocolIdV1.fromCanonicalLabel(text(tagged["protocol"], "$path.protocol"))
        } catch (error: IllegalArgumentException) {
            fail("has an unknown or non-canonical protocol tag", "$path.protocol", error)
        }
        if (tag != protocol) fail("does not match the protocol binding", "$path.protocol")
        val fields = privacyProtocolLimitRulesV1(protocol)
        if (fields.isEmpty()) {
            if (tagged["limits"] != null) fail("fixed protocol limits must be null", "$path.limits")
            return PrivacyProtocolLimitsV1(protocol, null)
        }
        val limitObject = exactObject(tagged["limits"], fields.map { it.name }.toSet(), "$path.limits")
        val normalized = LinkedHashMap<String, Int>()
        for (field in fields) {
            val number = positiveU32(limitObject[field.name], "$path.limits.${field.name}")
            if (number > field.maximum || (field.permitted != null && number !in field.permitted)) {
                fail("is outside the closed first-release limit set", "$path.limits.${field.name}")
            }
            normalized[field.name] = number
        }
        return PrivacyProtocolLimitsV1(protocol, normalized)
    }

    private fun parseLifecycle(value: Any?, committedHeight: BigInteger, path: String): PrivacyProtocolLifecycleV1 {
        val lifecycle = exactObject(value, setOf("state", "record"), path)
        val stateText = text(lifecycle["state"], "$path.state")
        val state = when (stateText) {
            "proposed" -> PrivacyProtocolLifecycleStateV1.PROPOSED
            "active" -> PrivacyProtocolLifecycleStateV1.ACTIVE
            "suspended" -> PrivacyProtocolLifecycleStateV1.SUSPENDED
            "retired" -> PrivacyProtocolLifecycleStateV1.RETIRED
            else -> fail("unknown lifecycle state", "$path.state")
        }
        val keys = if (state == PrivacyProtocolLifecycleStateV1.PROPOSED) {
            setOf("proposed_at_height", "activate_at_height")
        } else {
            setOf("proposed_at_height", "activated_at_height", "state_since_height")
        }
        val record = exactObject(lifecycle["record"], keys, "$path.record")
        val proposed = positiveU64(record["proposed_at_height"], "$path.record.proposed_at_height")
        if (proposed > committedHeight) fail("claims proposal after committed height", path)
        if (state == PrivacyProtocolLifecycleStateV1.PROPOSED) {
            val activate = positiveU64(record["activate_at_height"], "$path.record.activate_at_height")
            if (activate <= proposed || activate <= committedHeight) fail("has invalid proposed lifecycle heights", path)
            return PrivacyProtocolLifecycleV1(state, proposed, activate, null, null)
        }
        val activated = if (state == PrivacyProtocolLifecycleStateV1.RETIRED && record["activated_at_height"] == null) {
            null
        } else {
            positiveU64(record["activated_at_height"], "$path.record.activated_at_height")
        }
        val since = positiveU64(record["state_since_height"], "$path.record.state_since_height")
        if (since > committedHeight || (activated != null && activated > committedHeight)) {
            fail("claims a state after committed height", path)
        }
        val invalidOrder = if (activated == null) {
            state != PrivacyProtocolLifecycleStateV1.RETIRED || since <= proposed
        } else {
            activated <= proposed ||
                if (state == PrivacyProtocolLifecycleStateV1.ACTIVE) since < activated else since <= activated
        }
        if (invalidOrder) fail("has invalid lifecycle ordering", path)
        return PrivacyProtocolLifecycleV1(state, proposed, null, activated, since)
    }

    private fun parseProtocolTightening(
        value: Any?,
        current: PrivacyProtocolLimitsV1,
        committedHeight: BigInteger,
        path: String,
    ): PrivacyProtocolLimitsTighteningV1? {
        if (value == null) return null
        val tightening = exactObject(
            value,
            setOf("scheduled_at_height", "effective_at_height", "next_limits"),
            path,
        )
        val scheduled = positiveU64(tightening["scheduled_at_height"], "$path.scheduled_at_height")
        val effective = positiveU64(tightening["effective_at_height"], "$path.effective_at_height")
        validateSchedule(scheduled, effective, committedHeight, path)
        val next = parseProtocolLimits(tightening["next_limits"], current.protocolId, "$path.next_limits")
        assertLimitsAtMost(next, current, "$path.next_limits")
        if (next == current) fail("must be a strict tightening", path)
        return PrivacyProtocolLimitsTighteningV1(scheduled, effective, next)
    }

    private fun validateSchedule(
        scheduled: BigInteger,
        effective: BigInteger,
        committedHeight: BigInteger,
        path: String,
    ) {
        if (
            scheduled > U64_MAX.subtract(POLICY_DELAY_BLOCKS_V1) ||
            effective <= scheduled ||
            effective < scheduled.add(POLICY_DELAY_BLOCKS_V1) ||
            scheduled > committedHeight ||
            effective <= committedHeight
        ) {
            fail("has invalid committed-height schedule", path)
        }
    }

    private fun assertProfileBindingsEqual(
        actual: PrivacyCompiledProfileV1,
        expected: PrivacyCompiledProfileV1,
        path: String,
    ) {
        val equal = actual.protocolId == expected.protocolId &&
            actual.proofSystemId == expected.proofSystemId &&
            actual.engineId == expected.engineId &&
            actual.parameterId == expected.parameterId &&
            actual.parameterDigest == expected.parameterDigest &&
            actual.verifierDigest == expected.verifierDigest &&
            actual.statementSchemaDigest == expected.statementSchemaDigest &&
            actual.engineManifestDigest == expected.engineManifestDigest
        if (!equal) fail("does not match the compiled profile bindings", path)
    }

    private fun assertLimitsAtMost(actual: PrivacyProtocolLimitsV1, ceiling: PrivacyProtocolLimitsV1, path: String) {
        if (actual.protocolId != ceiling.protocolId || (actual.values == null) != (ceiling.values == null)) {
            fail("protocol-limit tag differs from compiled ceiling", path)
        }
        val actualValues = actual.values ?: return
        val ceilingValues = ceiling.values ?: fail("compiled limit ceiling is absent", path)
        for ((name, value) in actualValues) {
            if (value > (ceilingValues[name] ?: -1)) fail("exceeds the compiled profile ceiling", "$path.$name")
        }
    }

    private fun assertConsensusTightening(
        current: PrivacyConsensusLimitsV1,
        next: PrivacyConsensusLimitsV1,
        path: String,
    ) {
        val currentValues = consensusValues(current)
        val nextValues = consensusValues(next)
        var changed = false
        for (name in CONSENSUS_LIMIT_KEYS) {
            if (nextValues.getValue(name) > currentValues.getValue(name)) {
                fail("cannot increase a consensus limit", "$path.next_limits.$name")
            }
            changed = changed || nextValues.getValue(name) != currentValues.getValue(name)
        }
        if (!changed) fail("must be a strict tightening", path)
    }

    private fun consensusValues(value: PrivacyConsensusLimitsV1): Map<String, Int> = mapOf(
        "max_actions_per_transaction" to value.maxActionsPerTransaction,
        "max_actions_per_block" to value.maxActionsPerBlock,
        "max_proof_bytes_per_action" to value.maxProofBytesPerAction,
        "max_action_bytes" to value.maxActionBytes,
        "max_privacy_bytes_per_transaction" to value.maxPrivacyBytesPerTransaction,
        "max_privacy_bytes_per_block" to value.maxPrivacyBytesPerBlock,
        "max_statement_and_encrypted_output_bytes_per_transaction" to value.maxStatementAndEncryptedOutputBytesPerTransaction,
        "max_nullifiers_per_action" to value.maxNullifiersPerAction,
        "max_commitments_per_action" to value.maxCommitmentsPerAction,
        "retained_root_count" to value.retainedRootCount,
    )

    private fun protocolTag(value: Any?, path: String): PrivacyProtocolIdV1 {
        val label = taggedUnit(
            value,
            "protocol",
            "value",
            PrivacyProtocolIdV1.values().map { it.canonicalLabel }.toSet(),
            path,
        )
        return PrivacyProtocolIdV1.fromCanonicalLabel(label)
    }

    private fun taggedUnit(
        value: Any?,
        tagKey: String,
        contentKey: String,
        permitted: Set<String>,
        path: String,
    ): String {
        val tagged = exactObject(value, setOf(tagKey, contentKey), path)
        val label = text(tagged[tagKey], "$path.$tagKey")
        if (label !in permitted) fail("has an unknown or non-canonical tag", "$path.$tagKey")
        if (tagged[contentKey] != null) fail("unit enum content must be null", "$path.$contentKey")
        return label
    }

    private fun fixed32(value: Any?, path: String): PrivacyFixed32V1 {
        val bytes = list(value, path)
        if (bytes.size != 32) fail("must be exactly 32 bytes", path)
        return PrivacyFixed32V1(ByteArray(32) { index ->
            val number = u32(bytes[index], "$path[$index]")
            if (number > 255) fail("must contain only uint8 values", "$path[$index]")
            number.toByte()
        })
    }

    @Suppress("UNCHECKED_CAST")
    private fun exactObject(value: Any?, keys: Set<String>, path: String): Map<String, Any?> {
        val result = value as? Map<*, *> ?: fail("must be a JSON object", path)
        if (result.keys.any { it !is String }) fail("contains a non-string field", path)
        val actual = result.keys.map { it as String }.toSet()
        if (actual != keys || result.size != keys.size) {
            fail("must contain exactly: ${keys.sorted().joinToString(", ")}", path)
        }
        return result as Map<String, Any?>
    }

    private fun list(value: Any?, path: String): List<Any?> =
        value as? List<Any?> ?: fail("must be a JSON array", path)

    private fun text(value: Any?, path: String): String =
        value as? String ?: fail("must be a JSON string", path)

    private fun u32(value: Any?, path: String): Int {
        val number = integer(value, path)
        if (number.signum() < 0 || number > U32_MAX) fail("must be within the uint32 range", path)
        if (number > BigInteger.valueOf(Int.MAX_VALUE.toLong())) {
            fail("exceeds the supported first-release integer range", path)
        }
        return number.intValueExact()
    }

    private fun positiveU32(value: Any?, path: String): Int =
        u32(value, path).also { if (it == 0) fail("must be non-zero", path) }

    private fun u64(value: Any?, path: String): BigInteger {
        val number = integer(value, path)
        if (number.signum() < 0 || number > U64_MAX) fail("must be within the uint64 range", path)
        return number
    }

    private fun positiveU64(value: Any?, path: String): BigInteger =
        u64(value, path).also { if (it == BigInteger.ZERO) fail("must be non-zero", path) }

    private fun integer(value: Any?, path: String): BigInteger = when (value) {
        is Long -> BigInteger.valueOf(value)
        is BigInteger -> value
        else -> fail("must be one canonical integer", path)
    }

    private fun requireCanonicalUnsignedIntegerTokens(json: String) {
        var index = 0
        var inString = false
        var escaped = false
        while (index < json.length) {
            val character = json[index]
            if (inString) {
                if (escaped) {
                    escaped = false
                } else if (character == '\\') {
                    escaped = true
                } else if (character == '"') {
                    inString = false
                }
                index += 1
                continue
            }
            if (character == '"') {
                inString = true
                index += 1
                continue
            }
            if (character == '-') {
                fail("negative integers are not canonical", "legacy privacy capability inspection")
            }
            if (character in '0'..'9') {
                val start = index
                while (index < json.length && json[index] in '0'..'9') index += 1
                if (index - start > 1 && json[start] == '0') {
                    fail("integer tokens must not contain leading zeroes", "legacy privacy capability inspection")
                }
                if (index < json.length && json[index] in ".eE+") {
                    fail("numeric values must be canonical unsigned integers", "legacy privacy capability inspection")
                }
                continue
            }
            index += 1
        }
    }

    private fun fail(message: String, path: String, cause: Throwable? = null): Nothing =
        throw LegacyPrivacyCapabilitySnapshotInspectionException(path, message, cause)

    private val PROFILE_KEYS = setOf(
        "protocol_id",
        "proof_system_id",
        "engine_id",
        "parameter_id",
        "parameter_digest",
        "verifier_digest",
        "statement_schema_digest",
        "engine_manifest_digest",
        "protocol_limits",
    )
    private val ACTIVATION_KEYS = PROFILE_KEYS + setOf(
        "lifecycle",
        "pending_protocol_limits_tightening",
        "assurance",
    )
}
