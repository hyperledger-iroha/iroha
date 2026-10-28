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
    fun consensusPolicyAllowsBoundedUpdatesAndRejectsMalformedSchedulesAndNoOps() {
        val current = consensusLimits()
        val next = consensusLimits(retainedRootCount = 1024)
        validPolicySchedules().forEach { (scheduled, effective) ->
            val pending = PrivacyConsensusPolicyUpdateV1(scheduled, effective, next)
            val policy = PrivacyConsensusPolicyV1(current, pending)
            assertEquals(pending, policy.pendingUpdate)
            assertEquals(policy, PrivacyConsensusPolicyV1(current, pending))
            assertEquals(policy.hashCode(), PrivacyConsensusPolicyV1(current, pending).hashCode())
            assertEquals(current, PrivacyConsensusPolicyV1(next,
                PrivacyConsensusPolicyUpdateV1(scheduled, effective, current)).pendingUpdate!!.nextLimits)
        }
        invalidPolicySchedules().forEach { (scheduled, effective) ->
            assertFailsWith<IllegalArgumentException> {
                PrivacyConsensusPolicyUpdateV1(scheduled, effective, next)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyConsensusPolicyV1(current, PrivacyConsensusPolicyUpdateV1(height(10), height(11), current))
        }
    }

    @Test
    fun protocolPolicyAllowsBoundedUpdatesAndRejectsMalformedSchedulesAndNoOps() {
        val current = pgcLimits(64, 8)
        val next = pgcLimits(32, 4)
        validPolicySchedules().forEach { (scheduled, effective) ->
            val update = PrivacyProtocolLimitsUpdateV1(scheduled, effective, next)
            val policy = PrivacyProtocolPolicyV1(current.protocolId, current, update)
            assertEquals(update, policy.pendingUpdate)
            assertEquals(policy, PrivacyProtocolPolicyV1(current.protocolId, current, update))
            assertEquals(policy.hashCode(), PrivacyProtocolPolicyV1(current.protocolId, current, update).hashCode())
            assertEquals(current, PrivacyProtocolPolicyV1(current.protocolId, next,
                PrivacyProtocolLimitsUpdateV1(scheduled, effective, current)).pendingUpdate!!.nextLimits)
        }
        invalidPolicySchedules().forEach { (scheduled, effective) ->
            assertFailsWith<IllegalArgumentException> { PrivacyProtocolLimitsUpdateV1(scheduled, effective, next) }
        }
        assertFailsWith<IllegalArgumentException> { pgcLimits(48, 4) }
        assertFailsWith<IllegalArgumentException> { pgcLimits(64, 9) }
        assertFailsWith<IllegalArgumentException> {
            PrivacyProtocolPolicyV1(current.protocolId, current,
                PrivacyProtocolLimitsUpdateV1(height(10), height(11), current))
        }
        val other = PrivacyProtocolLimitsV1(PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
            mapOf("max_aggregation_count" to 4))
        assertFailsWith<IllegalArgumentException> { PrivacyProtocolPolicyV1(other.protocolId, current, null) }
        assertFailsWith<IllegalArgumentException> {
            PrivacyProtocolPolicyV1(current.protocolId, current,
                PrivacyProtocolLimitsUpdateV1(height(10), height(11), other))
        }
    }

    @Test
    fun optionalPolicyUsesCanonicalLimitsWithoutGrantingUnavailableImplementation() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val canonical = pgcLimits(64, 8)
        val compiled = PrivacyCompiledProfileResultV1.Available(profile(protocol, canonical))
        val absent = exactRow(protocol, compiled, null, PrivacyCapabilityReadinessV1.Available)
        assertTrue(absent.isNetworkAvailable())
        assertEquals(canonical, absent.effectiveProtocolLimits())
        val policy = PrivacyProtocolPolicyV1(protocol, pgcLimits(32, 4), null)
        val governed = exactRow(protocol, compiled, policy, PrivacyCapabilityReadinessV1.Available)
        assertEquals(policy.currentLimits, governed.effectiveProtocolLimits())
        assertTrue(governed.isNetworkAvailable())
        val unavailable = unavailable()
        val closed = exactRow(protocol, unavailable, policy, PrivacyCapabilityReadinessV1.Unavailable(
            PrivacyCapabilityUnavailableReasonV1.CompiledProfile(unavailable)))
        assertEquals(null, closed.effectiveProtocolLimits())
        assertEquals(false, closed.isNetworkAvailable())
        assertFailsWith<IllegalArgumentException> {
            exactRow(protocol, unavailable, policy, PrivacyCapabilityReadinessV1.Available)
        }
    }

    @Test
    fun exactCapabilityRowsRejectCrossProtocolAndForgedAvailability() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val compiled = PrivacyCompiledProfileResultV1.Available(profile(protocol, pgcLimits(64, 8)))
        val other = PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1
        assertFailsWith<IllegalArgumentException> {
            exactRow(other, compiled, null, PrivacyCapabilityReadinessV1.Available)
        }
        val otherPolicy = PrivacyProtocolPolicyV1(other,
            PrivacyProtocolLimitsV1(other, mapOf("max_aggregation_count" to 4)), null)
        assertFailsWith<IllegalArgumentException> {
            exactRow(protocol, compiled, otherPolicy, PrivacyCapabilityReadinessV1.Available)
        }
        assertFailsWith<IllegalArgumentException> {
            exactRow(protocol, compiled, null, PrivacyCapabilityReadinessV1.Unavailable(
                PrivacyCapabilityUnavailableReasonV1.CompiledProfile(unavailable())))
        }
    }

    @Test
    fun canonicalProfileBindingCheckStillRejectsSubstitutedDigestsAndProtocols() {
        val protocol = PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1
        val expected = profile(protocol, pgcLimits(64, 8))
        requirePrivacyProfileBindingsEqualV1(expected, expected, "canonical profile")
        assertFailsWith<IllegalArgumentException> {
            requirePrivacyProfileBindingsEqualV1(profile(protocol, pgcLimits(64, 8), 99), expected, "canonical profile")
        }
        val other = PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1
        assertFailsWith<IllegalArgumentException> {
            requirePrivacyProfileBindingsEqualV1(profile(other,
                PrivacyProtocolLimitsV1(other, mapOf("max_aggregation_count" to 4))), expected, "canonical profile")
        }
    }

    private fun unavailable(): PrivacyCompiledProfileResultV1.Unavailable =
        PrivacyCompiledProfileResultV1.Unavailable(PrivacyCompiledProfileUnavailableReasonV1.ENGINE_UNAVAILABLE, null)

    private fun exactRow(
        protocol: PrivacyProtocolIdV1,
        compiled: PrivacyCompiledProfileResultV1,
        policy: PrivacyProtocolPolicyV1?,
        readiness: PrivacyCapabilityReadinessV1,
    ): PrivacyExact12CapabilityRowV1 = PrivacyExact12CapabilityRowV1(
        protocol, expectedOperationSchema(protocol), expectedExecutionMode(protocol), expectedFeatureMask(protocol),
        compiled, readiness, policy, false,
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
        maxActionBytes: Int = PRIVACY_ACTION_ENVELOPE_MAX_BYTES_V1,
        maxPrivacyBytesPerTransaction: Int = PRIVACY_ACTION_ENVELOPE_MAX_BYTES_V1,
        maxPrivacyBytesPerBlock: Int = PRIVACY_BLOCK_MAX_BYTES_V1,
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

    private fun validPolicySchedules(): List<Pair<BigInteger, BigInteger>> = listOf(
        height(10) to height(11),
        height(10) to height(12),
        height(10) to height(309),
        U64_MAX_TEST.subtract(BigInteger.ONE) to U64_MAX_TEST,
    )

    private fun invalidPolicySchedules(): List<Pair<BigInteger, BigInteger>> = listOf(
        height(-1) to height(11),
        BigInteger.ZERO to height(11),
        height(10) to height(-1),
        height(10) to BigInteger.ZERO,
        height(10) to height(10),
        height(10) to height(9),
        U64_MAX_TEST to U64_MAX_TEST,
        U64_MAX_TEST to U64_MAX_TEST.add(BigInteger.ONE),
        height(10) to U64_MAX_TEST.add(BigInteger.ONE),
        U64_MAX_TEST.add(BigInteger.ONE) to U64_MAX_TEST.add(height(2)),
    )

    private fun height(value: Long): BigInteger = BigInteger.valueOf(value)

    private companion object {
        val U64_MAX_TEST: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    }
}
