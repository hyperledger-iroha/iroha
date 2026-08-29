// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class PrivacyCapabilityModelInvariantV1Test {
    @Test
    fun consensusLimitsRejectInvalidDirectConstruction() {
        val limits = consensusLimits()
        val hostile = listOf<() -> Unit>(
            { consensusLimits(maxActionsPerTransaction = 0) },
            { consensusLimits(maxActionsPerBlock = 3) },
            { consensusLimits(maxProofBytesPerAction = -1) },
            { consensusLimits(maxActionBytes = 8 * 1024 * 1024) },
            { consensusLimits(maxPrivacyBytesPerTransaction = 8 * 1024 * 1024) },
            { consensusLimits(maxPrivacyBytesPerBlock = 8 * 1024 * 1024) },
            {
                consensusLimits(
                    maxProofBytesPerAction = 1,
                    maxActionBytes = 1,
                    maxStatementAndEncryptedOutputBytesPerTransaction = 2,
                )
            },
            { consensusLimits(retainedRootCount = 2049) },
        )
        hostile.forEachIndexed { index, construct ->
            assertFailsWith<IllegalArgumentException>("hostile consensus limits $index") {
                construct()
            }
        }
        assertEquals(2048, limits.retainedRootCount)
    }

    @Test
    fun consensusPolicyRejectsMalformedSchedulesIncreasesAndNoOps() {
        val current = consensusLimits()
        val next = consensusLimits(retainedRootCount = 1024)
        val validPending = PrivacyConsensusPolicyTighteningV1(height(10), height(310), next)
        assertEquals(validPending, PrivacyConsensusPolicyV1(current, validPending).pendingTightening)

        val invalidSchedules = listOf(
            BigInteger.ZERO to height(300),
            height(10) to height(309),
            height(10) to height(9),
            U64_MAX_TEST to U64_MAX_TEST,
            height(10) to U64_MAX_TEST.add(BigInteger.ONE),
        )
        invalidSchedules.forEachIndexed { index, (scheduled, effective) ->
            assertFailsWith<IllegalArgumentException>("hostile consensus schedule $index") {
                PrivacyConsensusPolicyTighteningV1(scheduled, effective, next)
            }
        }

        val noOp = PrivacyConsensusPolicyTighteningV1(height(10), height(310), current)
        assertFailsWith<IllegalArgumentException> {
            PrivacyConsensusPolicyV1(current, noOp)
        }
        val lowerCurrent = consensusLimits(retainedRootCount = 1024)
        val increase = PrivacyConsensusPolicyTighteningV1(height(10), height(310), current)
        assertFailsWith<IllegalArgumentException> {
            PrivacyConsensusPolicyV1(lowerCurrent, increase)
        }
    }

    @Test
    fun lifecycleRejectsWrongVariantShapeAndHostileHeightOrdering() {
        assertEquals(height(2), activeLifecycle().stateSinceHeight)
        assertEquals(
            PrivacyProtocolLifecycleStateV1.RETIRED,
            retiredLifecycle(activatedAtHeight = null, stateSinceHeight = height(2)).state,
        )

        val hostile = listOf<() -> Unit>(
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.PROPOSED,
                    activateAtHeight = null,
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.PROPOSED,
                    activateAtHeight = height(2),
                    activatedAtHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.PROPOSED,
                    proposedAtHeight = height(2),
                    activateAtHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.ACTIVE,
                    activateAtHeight = height(3),
                    activatedAtHeight = height(2),
                    stateSinceHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.ACTIVE,
                    activatedAtHeight = null,
                    stateSinceHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.ACTIVE,
                    activatedAtHeight = height(2),
                    stateSinceHeight = null,
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.SUSPENDED,
                    activatedAtHeight = height(2),
                    stateSinceHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.RETIRED,
                    activatedAtHeight = null,
                    stateSinceHeight = null,
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.RETIRED,
                    activatedAtHeight = height(2),
                    stateSinceHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.ACTIVE,
                    proposedAtHeight = BigInteger.ZERO,
                    activatedAtHeight = height(2),
                    stateSinceHeight = height(2),
                )
            },
            {
                lifecycle(
                    PrivacyProtocolLifecycleStateV1.ACTIVE,
                    proposedAtHeight = U64_MAX_TEST.add(BigInteger.ONE),
                    activatedAtHeight = height(2),
                    stateSinceHeight = height(2),
                )
            },
        )
        hostile.forEachIndexed { index, construct ->
            assertFailsWith<IllegalArgumentException>("hostile lifecycle $index") {
                construct()
            }
        }
    }

    @Test
    fun activationRejectsMalformedOrNonMonotonicProtocolTightening() {
        val currentLimits = pgcLimits(64, 8)
        val currentProfile = profile(
            PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
            currentLimits,
        )
        val validPending = PrivacyProtocolLimitsTighteningV1(
            height(10),
            height(310),
            pgcLimits(32, 4),
        )
        assertEquals(validPending, activation(currentProfile, pending = validPending).pendingProtocolLimitsTightening)

        assertFailsWith<IllegalArgumentException> {
            PrivacyProtocolLimitsTighteningV1(height(10), height(309), pgcLimits(32, 4))
        }
        assertFailsWith<IllegalArgumentException> {
            pgcLimits(48, 4)
        }

        val noOp = PrivacyProtocolLimitsTighteningV1(height(10), height(310), currentLimits)
        assertFailsWith<IllegalArgumentException> {
            activation(currentProfile, pending = noOp)
        }

        val restrictedProfile = profile(
            PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
            pgcLimits(32, 4),
        )
        val increase = PrivacyProtocolLimitsTighteningV1(
            height(10),
            height(310),
            pgcLimits(64, 4),
        )
        assertFailsWith<IllegalArgumentException> {
            activation(restrictedProfile, pending = increase)
        }

        val otherProtocol = PrivacyProtocolLimitsTighteningV1(
            height(10),
            height(310),
            PrivacyProtocolLimitsV1(
                PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
                mapOf("max_aggregation_count" to 4),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            activation(currentProfile, pending = otherProtocol)
        }
    }

    @Test
    fun exactCapabilityRowRejectsUnavailableCrossProtocolAndSubstitutedBindings() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val compiled = profile(protocol, pgcLimits(64, 8))
        val governed = profile(protocol, pgcLimits(32, 4))
        val valid = exactRow(
            protocol,
            PrivacyCompiledProfileResultV1.Available(compiled),
            activation(governed),
            unavailableReadiness(
                PrivacyCapabilityUnavailableReasonV1.MissingProductionQualification,
            ),
        )
        assertEquals(protocol, valid.protocolId)
        assertTrue(!valid.isNetworkAvailable())

        assertFailsWith<IllegalArgumentException> {
            exactRow(
                PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
                PrivacyCompiledProfileResultV1.Available(compiled),
                null,
                unavailableReadiness(PrivacyCapabilityUnavailableReasonV1.NotRegistered),
            )
        }

        val unavailable = unavailable()
        assertFailsWith<IllegalArgumentException> {
            exactRow(
                protocol,
                unavailable,
                activation(governed),
                unavailableReadiness(
                    PrivacyCapabilityUnavailableReasonV1.CompiledProfile(unavailable),
                ),
            )
        }

        val otherProfile = profile(
            PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
            PrivacyProtocolLimitsV1(
                PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
                mapOf("max_aggregation_count" to 8),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            exactRow(
                protocol,
                PrivacyCompiledProfileResultV1.Available(compiled),
                activation(otherProfile),
                unavailableReadiness(
                    PrivacyCapabilityUnavailableReasonV1.MissingProductionQualification,
                ),
            )
        }

        val substitutedDigest = profile(
            protocol,
            pgcLimits(32, 4),
            parameterDigestByte = 99,
        )
        assertFailsWith<IllegalArgumentException> {
            exactRow(
                protocol,
                PrivacyCompiledProfileResultV1.Available(compiled),
                activation(substitutedDigest),
                unavailableReadiness(
                    PrivacyCapabilityUnavailableReasonV1.MissingProductionQualification,
                ),
            )
        }

        val lowCompiledCeiling = profile(protocol, pgcLimits(32, 4))
        val excessiveActivation = profile(protocol, pgcLimits(64, 8))
        assertFailsWith<IllegalArgumentException> {
            exactRow(
                protocol,
                PrivacyCompiledProfileResultV1.Available(lowCompiledCeiling),
                activation(excessiveActivation),
                unavailableReadiness(
                    PrivacyCapabilityUnavailableReasonV1.MissingProductionQualification,
                ),
            )
        }
    }

    private fun unavailable(): PrivacyCompiledProfileResultV1.Unavailable =
        PrivacyCompiledProfileResultV1.Unavailable(
            PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
            null,
        )

    private fun exactRow(
        protocol: PrivacyProtocolIdV1,
        compiled: PrivacyCompiledProfileResultV1,
        activation: PrivacyProtocolActivationRecordV1?,
        readiness: PrivacyCapabilityReadinessV1,
    ): PrivacyExact12CapabilityRowV1 = PrivacyExact12CapabilityRowV1(
        protocol,
        expectedOperationSchema(protocol),
        expectedExecutionMode(protocol),
        expectedFeatureMask(protocol),
        compiled,
        readiness,
        activation,
        false,
        null,
        height(42),
    )

    private fun unavailableReadiness(
        reason: PrivacyCapabilityUnavailableReasonV1,
    ): PrivacyCapabilityReadinessV1 = PrivacyCapabilityReadinessV1.Unavailable(reason)

    private fun activation(
        profile: PrivacyCompiledProfileV1,
        lifecycle: PrivacyProtocolLifecycleV1 = activeLifecycle(),
        pending: PrivacyProtocolLimitsTighteningV1? = null,
    ): PrivacyProtocolActivationRecordV1 = PrivacyProtocolActivationRecordV1(
        profile,
        lifecycle,
        pending,
    )

    private fun activeLifecycle(): PrivacyProtocolLifecycleV1 = lifecycle(
        PrivacyProtocolLifecycleStateV1.ACTIVE,
        activatedAtHeight = height(2),
        stateSinceHeight = height(2),
    )

    private fun retiredLifecycle(
        activatedAtHeight: BigInteger?,
        stateSinceHeight: BigInteger,
    ): PrivacyProtocolLifecycleV1 = lifecycle(
        PrivacyProtocolLifecycleStateV1.RETIRED,
        activatedAtHeight = activatedAtHeight,
        stateSinceHeight = stateSinceHeight,
    )

    private fun lifecycle(
        state: PrivacyProtocolLifecycleStateV1,
        proposedAtHeight: BigInteger = BigInteger.ONE,
        activateAtHeight: BigInteger? = null,
        activatedAtHeight: BigInteger? = null,
        stateSinceHeight: BigInteger? = null,
    ): PrivacyProtocolLifecycleV1 = PrivacyProtocolLifecycleV1(
        state,
        proposedAtHeight,
        activateAtHeight,
        activatedAtHeight,
        stateSinceHeight,
    )

    private fun profile(
        protocol: PrivacyProtocolIdV1,
        limits: PrivacyProtocolLimitsV1,
        parameterDigestByte: Int = 2,
    ): PrivacyCompiledProfileV1 = PrivacyCompiledProfileV1(
        protocol,
        protocol.expectedProofSystem,
        protocol.expectedEngine,
        fixed32(1),
        fixed32(parameterDigestByte),
        fixed32(3),
        fixed32(4),
        fixed32(5),
        limits,
    )

    private fun pgcLimits(anonymitySetSize: Int, recipientCount: Int): PrivacyProtocolLimitsV1 =
        PrivacyProtocolLimitsV1(
            PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
            linkedMapOf(
                "max_anonymity_set_size" to anonymitySetSize,
                "max_recipient_count" to recipientCount,
            ),
        )

    private fun fixed32(byte: Int): PrivacyFixed32V1 =
        PrivacyFixed32V1(ByteArray(32) { byte.toByte() })

    private fun consensusLimits(
        maxActionsPerTransaction: Int = 1,
        maxActionsPerBlock: Int = 2,
        maxProofBytesPerAction: Int = 9 * 1024 * 1024,
        maxActionBytes: Int = 9 * 1024 * 1024,
        maxPrivacyBytesPerTransaction: Int = 9 * 1024 * 1024,
        maxPrivacyBytesPerBlock: Int = 18 * 1024 * 1024,
        maxStatementAndEncryptedOutputBytesPerTransaction: Int = 256 * 1024,
        maxNullifiersPerAction: Int = 8,
        maxCommitmentsPerAction: Int = 8,
        retainedRootCount: Int = 2048,
    ): PrivacyConsensusLimitsV1 = PrivacyConsensusLimitsV1(
        maxActionsPerTransaction = maxActionsPerTransaction,
        maxActionsPerBlock = maxActionsPerBlock,
        maxProofBytesPerAction = maxProofBytesPerAction,
        maxActionBytes = maxActionBytes,
        maxPrivacyBytesPerTransaction = maxPrivacyBytesPerTransaction,
        maxPrivacyBytesPerBlock = maxPrivacyBytesPerBlock,
        maxStatementAndEncryptedOutputBytesPerTransaction =
            maxStatementAndEncryptedOutputBytesPerTransaction,
        maxNullifiersPerAction = maxNullifiersPerAction,
        maxCommitmentsPerAction = maxCommitmentsPerAction,
        retainedRootCount = retainedRootCount,
    )

    private fun height(value: Long): BigInteger = BigInteger.valueOf(value)

    private companion object {
        val U64_MAX_TEST: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    }
}
