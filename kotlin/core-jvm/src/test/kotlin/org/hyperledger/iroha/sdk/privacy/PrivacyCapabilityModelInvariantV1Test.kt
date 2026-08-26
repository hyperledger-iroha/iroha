// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

@Suppress("DEPRECATION")
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
    fun legacyInspectionRowRejectsUnavailableCrossProtocolAndSubstitutedBindings() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val compiled = profile(protocol, pgcLimits(64, 8))
        val governed = profile(protocol, pgcLimits(32, 4))
        val valid = LegacyPrivacyCapabilityRowInspectionV1(
            protocol,
            PrivacyCompiledProfileResultV1.Available(compiled),
            activation(governed),
        )
        assertEquals(protocol, valid.protocolId)

        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilityRowInspectionV1(
                PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
                PrivacyCompiledProfileResultV1.Available(compiled),
                null,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilityRowInspectionV1(
                protocol,
                unavailable(),
                activation(governed),
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
            LegacyPrivacyCapabilityRowInspectionV1(
                protocol,
                PrivacyCompiledProfileResultV1.Available(compiled),
                activation(otherProfile),
            )
        }

        val substitutedDigest = profile(
            protocol,
            pgcLimits(32, 4),
            parameterDigestByte = 99,
        )
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilityRowInspectionV1(
                protocol,
                PrivacyCompiledProfileResultV1.Available(compiled),
                activation(substitutedDigest),
            )
        }

        val lowCompiledCeiling = profile(protocol, pgcLimits(32, 4))
        val excessiveActivation = profile(protocol, pgcLimits(64, 8))
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilityRowInspectionV1(
                protocol,
                PrivacyCompiledProfileResultV1.Available(lowCompiledCeiling),
                activation(excessiveActivation),
            )
        }
    }

    @Test
    fun legacyInspectionSnapshotRejectsInvalidVersionHeightPolicyRegistryAndMutableInput() {
        val rows = canonicalUnavailableRows().toMutableList()
        val snapshot = LegacyPrivacyCapabilitySnapshotInspectionV1(
            1,
            BigInteger.ZERO,
            consensusPolicy(),
            rows,
        )
        rows.clear()
        assertEquals(PrivacyProtocolIdV1.values().size, snapshot.protocols.size)
        assertFailsWith<UnsupportedOperationException> {
            @Suppress("UNCHECKED_CAST")
            (snapshot.protocols as MutableList<LegacyPrivacyCapabilityRowInspectionV1>).clear()
        }

        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilitySnapshotInspectionV1(
                0,
                BigInteger.ZERO,
                consensusPolicy(),
                canonicalUnavailableRows(),
            )
        }
        for (height in listOf(BigInteger.ONE.negate(), U64_MAX_TEST.add(BigInteger.ONE))) {
            assertFailsWith<IllegalArgumentException> {
                LegacyPrivacyCapabilitySnapshotInspectionV1(
                    1,
                    height,
                    consensusPolicy(),
                    canonicalUnavailableRows(),
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilitySnapshotInspectionV1(
                1,
                BigInteger.ZERO,
                consensusPolicy(),
                canonicalUnavailableRows().dropLast(1),
            )
        }
        val reordered = canonicalUnavailableRows().toMutableList().also {
            val first = it[0]
            it[0] = it[1]
            it[1] = first
        }
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilitySnapshotInspectionV1(
                1,
                BigInteger.ZERO,
                consensusPolicy(),
                reordered,
            )
        }

        val next = consensusLimits(retainedRootCount = 1024)
        val pending = PrivacyConsensusPolicyTighteningV1(height(10), height(310), next)
        val scheduledAfterSnapshot = PrivacyConsensusPolicyV1(consensusLimits(), pending)
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilitySnapshotInspectionV1(
                1,
                height(9),
                scheduledAfterSnapshot,
                canonicalUnavailableRows(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            LegacyPrivacyCapabilitySnapshotInspectionV1(
                1,
                height(310),
                scheduledAfterSnapshot,
                canonicalUnavailableRows(),
            )
        }
    }

    @Test
    fun legacyInspectionSnapshotRejectsLifecycleAndProtocolScheduleClaimsOutsideCommittedState() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val compiled = profile(protocol, pgcLimits(64, 8))

        val proposedAfterSnapshot = activation(
            compiled,
            lifecycle(
                PrivacyProtocolLifecycleStateV1.PROPOSED,
                proposedAtHeight = height(11),
                activateAtHeight = height(500),
            ),
        )
        assertSnapshotRejectsRow(height(10), row(compiled, proposedAfterSnapshot))

        val dueProposal = activation(
            compiled,
            lifecycle(
                PrivacyProtocolLifecycleStateV1.PROPOSED,
                proposedAtHeight = height(1),
                activateAtHeight = height(40),
            ),
        )
        assertSnapshotRejectsRow(height(40), row(compiled, dueProposal))

        val futureActivation = activation(
            compiled,
            lifecycle(
                PrivacyProtocolLifecycleStateV1.ACTIVE,
                proposedAtHeight = height(1),
                activatedAtHeight = height(50),
                stateSinceHeight = height(50),
            ),
        )
        assertSnapshotRejectsRow(height(40), row(compiled, futureActivation))

        val futureRetirement = activation(
            compiled,
            retiredLifecycle(activatedAtHeight = null, stateSinceHeight = height(50)),
        )
        assertSnapshotRejectsRow(height(40), row(compiled, futureRetirement))

        val nextLimits = pgcLimits(32, 4)
        val scheduledAfterSnapshot = activation(
            compiled,
            pending = PrivacyProtocolLimitsTighteningV1(height(20), height(320), nextLimits),
        )
        assertSnapshotRejectsRow(height(10), row(compiled, scheduledAfterSnapshot))

        val dueTightening = activation(
            compiled,
            pending = PrivacyProtocolLimitsTighteningV1(height(10), height(310), nextLimits),
        )
        assertSnapshotRejectsRow(height(310), row(compiled, dueTightening))

        val validPending = activation(
            compiled,
            pending = PrivacyProtocolLimitsTighteningV1(height(10), height(310), nextLimits),
        )
        val validSnapshot = snapshotWithRow(height(42), row(compiled, validPending))
        assertEquals(height(42), validSnapshot.committedHeight)
        assertTrue(validSnapshot.protocols[protocol.ordinal].activation != null)
    }

    private fun assertSnapshotRejectsRow(
        committedHeight: BigInteger,
        capabilityRow: LegacyPrivacyCapabilityRowInspectionV1,
    ) {
        assertFailsWith<IllegalArgumentException> {
            snapshotWithRow(committedHeight, capabilityRow)
        }
    }

    private fun snapshotWithRow(
        committedHeight: BigInteger,
        capabilityRow: LegacyPrivacyCapabilityRowInspectionV1,
    ): LegacyPrivacyCapabilitySnapshotInspectionV1 {
        val rows = canonicalUnavailableRows().toMutableList()
        rows[capabilityRow.protocolId.ordinal] = capabilityRow
        return LegacyPrivacyCapabilitySnapshotInspectionV1(
            1,
            committedHeight,
            consensusPolicy(),
            rows,
        )
    }

    private fun canonicalUnavailableRows(): List<LegacyPrivacyCapabilityRowInspectionV1> =
        PrivacyProtocolIdV1.values().map { protocol ->
            LegacyPrivacyCapabilityRowInspectionV1(protocol, unavailable(), null)
        }

    private fun unavailable(): PrivacyCompiledProfileResultV1.Unavailable =
        PrivacyCompiledProfileResultV1.Unavailable(
            PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE,
            null,
        )

    private fun row(
        compiled: PrivacyCompiledProfileV1,
        governed: PrivacyProtocolActivationRecordV1,
    ): LegacyPrivacyCapabilityRowInspectionV1 = LegacyPrivacyCapabilityRowInspectionV1(
        compiled.protocolId,
        PrivacyCompiledProfileResultV1.Available(compiled),
        governed,
    )

    private fun activation(
        profile: PrivacyCompiledProfileV1,
        lifecycle: PrivacyProtocolLifecycleV1 = activeLifecycle(),
        pending: PrivacyProtocolLimitsTighteningV1? = null,
    ): PrivacyProtocolActivationRecordV1 = PrivacyProtocolActivationRecordV1(
        profile,
        lifecycle,
        pending,
        PrivacyAssuranceV1.EXPERIMENTAL,
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

    private fun consensusPolicy(): PrivacyConsensusPolicyV1 =
        PrivacyConsensusPolicyV1(consensusLimits(), null)

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
