package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class KagemushaRecursiveSpendProverTest {
    private companion object {
        private const val TEST_NORITO_COMPACT_LEN_FLAG = 0x02
        private const val TEST_NORITO_PACKED_STRUCT_FLAG = 0x04
        private const val TEST_NORITO_FIELD_BITSET_FLAG = 0x20
    }

    @Test
    fun exposesStableModesAndCircuitIds() {
        assertEquals(6, KagemushaRecursiveSpendProver.REQUIRED_BRIDGE_ABI_VERSION)
        assertEquals(
            "kagemusha-recursive-aggregation-v1",
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        )
        assertEquals(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
        )
        assertEquals(
            "kagemusha-recursive-spend-lineage-onehop-v1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
        )
        assertEquals(
            "kagemusha-recursive-spend-lineage-append-v1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
        )
        assertEquals(64, KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1)
        assertTrue(KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1)
        assertEquals(
            1,
            KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
        )
        assertEquals(
            8 * 1024 * 1024,
            KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
        )
        assertEquals(
            128,
            KagemushaRecursiveSpendProver.RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES,
        )
        assertEquals(
            64 * 1024 * 1024,
            KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES,
        )
        assertEquals(
            "iroha:kagemusha:v1:recursive-spend-transition-profile",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN,
        )
        assertEquals(
            "iroha:kagemusha:v1:recursive-spend-transition-profile-digest",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN,
        )
        assertEquals(
            "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN,
        )
        assertEquals(
            "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1,
        )
        assertEquals(
            "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1,
        )
        assertEquals(
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1",
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1,
        )
        assertEquals(
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1",
            KagemushaRecursiveSpendProver
                .RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1,
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                2,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                0,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                2,
            ),
        )
        assertTrue(KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(null, 1))
        for ((circuitId, hopCount) in listOf(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 to -1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 to Int.MAX_VALUE,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 to 0,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 to Int.MAX_VALUE,
            "" to 1,
            "unknown-kagemusha-recursive-spend-circuit" to Int.MAX_VALUE,
            null to Int.MAX_VALUE,
        )) {
            assertFalse(KagemushaRecursiveSpendProver.canRedeemWitnessless(circuitId, hopCount))
            assertTrue(KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(circuitId, hopCount))
        }
        assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(0))
        assertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(1))
        assertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(63))
        assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(64))
        assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(-1))
        assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(Int.MAX_VALUE))
        assertEquals(64, KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS)
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(null),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(""),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertEquals(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
            ),
        )
        assertTrue(KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(null))
        assertTrue(KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(""))
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isLineageAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForInit())
        assertTrue(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ),
        )
        for (outputCircuitId in listOf(
            null,
            "",
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            "unknown-kagemusha-recursive-spend-circuit",
        )) {
            assertFalse(
                KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(
                    outputCircuitId,
                ),
            )
        }
        assertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                "unknown-kagemusha-recursive-spend-circuit",
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                "",
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                "unknown-kagemusha-recursive-spend-circuit",
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.isSupportedAppendProofTransition(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                "unknown-kagemusha-recursive-spend-circuit",
            ),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(1),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(63),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(64),
            "preferred append selector falls back at the witnessless hop cap",
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(0),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(null, 1))
        assertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS - 1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                0,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                63,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                64,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canProveAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit",
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1,
            ),
            "semantic previous proofs cannot select Reserved-lineage output",
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                "unknown-kagemusha-recursive-spend-circuit",
                1,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                0,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                64,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                0,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1,
            ),
        )
        assertFalse(KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(null, 1))
        assertFalse(KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend("", 1))
        assertEquals("checked_prefold_v1", KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1.wireName)
        assertEquals("recursive_compact_v1", KagemushaRecursiveSpendProver.Mode.RECURSIVE_COMPACT_V1.wireName)
        assertEquals("recursive_spend_v1", KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName)
        assertEquals(
            KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
            KagemushaRecursiveSpendProver.preferredMode(
                recursiveCompactAvailable = true,
                recursiveSpendAvailable = true,
            ),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1,
            KagemushaRecursiveSpendProver.preferredMode(
                recursiveCompactAvailable = true,
                recursiveSpendAvailable = false,
            ),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
            KagemushaRecursiveSpendProver.preferredMode(true),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1,
            KagemushaRecursiveSpendProver.preferredMode(false),
        )
        assertEquals(7, KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION)
        assertEquals(
            "kagemusha-recursive-compact-v1",
            KagemushaRecursiveCompactPaymentTokenProver.RECURSIVE_COMPACT_CIRCUIT_ID_V1,
        )
        val verifierNativeAvailable =
            KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable()
        assertEquals(
            verifierNativeAvailable,
            KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable(),
        )
        val projectionVerifierNativeAvailable =
            KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable()
        assertEquals(
            projectionVerifierNativeAvailable,
            KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable(),
        )
        assertTrue(
            KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
                IllegalArgumentException(
                    "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch",
                ),
            ),
        )
        assertTrue(
            KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
                IllegalArgumentException(
                    "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch",
                ),
            ),
        )
        assertFalse(KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(null))
        assertFalse(
            KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
                IllegalArgumentException(),
            ),
        )
        assertFalse(
            KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
                IllegalArgumentException("recordBundleArchive must be a valid Norito archive"),
            ),
        )
        assertFalse(
            KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
                IllegalArgumentException(
                    "Kagemusha recursive compact token public instance column 0 must contain exactly one row; found 2",
                ),
            ),
        )
        assertFalse(
            KagemushaRecursiveCompactPaymentTokenProver.isRecursiveCompactUnavailable(
                IllegalArgumentException(
                    "Kagemusha recursive compact token envelope verifier-key hash mismatch",
                ),
            ),
        )
        val validRecursiveCompactInput = kagemushaNoritoFrameWithPayload(0x4b)
        val validRecursiveCompactKeyArtifacts = kagemushaNoritoFrameWithPayload(0xe1)
        val validRecursiveCompactVerifierKeys = kagemushaNoritoFrameWithPayload(0xe2)
        val recursiveCompactCopyInput = kagemushaNoritoFrameWithPayload(0x4c)
        val expectedRecursiveCompactInput = recursiveCompactCopyInput.copyOf()
        val ownedRecursiveCompactInput =
            KagemushaRecursiveCompactPaymentTokenProver.ownedNativeInput(
                recursiveCompactCopyInput,
                "compactTokenArchive",
            )
        recursiveCompactCopyInput[6] = 0x7f.toByte()
        assertFalse(ownedRecursiveCompactInput === recursiveCompactCopyInput)
        assertContentEquals(expectedRecursiveCompactInput, ownedRecursiveCompactInput)
        val oversizedRecursiveCompactInput =
            ByteArray(KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1)
        assertIllegalArgumentContains("recordBundleArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    ByteArray(0),
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    ByteArray(0),
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("recursiveCompactKeyArtifactsArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    ByteArray(0),
                )
        }
        assertIllegalArgumentContains("recordBundleArchive must not exceed") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    oversizedRecursiveCompactInput,
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must not exceed") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    oversizedRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("recursiveCompactKeyArtifactsArchive must not exceed") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    oversizedRecursiveCompactInput,
                )
        }
        assertIllegalArgumentContains("recordBundleArchive must be a valid Norito archive") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    byteArrayOf(0x01, 0x02),
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must be a valid Norito archive") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    byteArrayOf(0x01, 0x02),
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("recursiveCompactKeyArtifactsArchive must be a valid Norito archive") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    byteArrayOf(0x01, 0x02),
                )
        }
        assertIllegalArgumentContains("recordBundleArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    kagemushaNoritoFrame(0x4b),
                    validRecursiveCompactInput,
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    kagemushaNoritoFrame(0x4b),
                    validRecursiveCompactKeyArtifacts,
                )
        }
        assertIllegalArgumentContains("recursiveCompactKeyArtifactsArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    kagemushaNoritoFrame(0xe1),
                )
        }
        assertIllegalArgumentContains("bundleArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(ByteArray(0))
        }
        assertIllegalArgumentContains("bundleArchive must not exceed") {
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(oversizedRecursiveCompactInput)
        }
        assertIllegalArgumentContains("bundleArchive must be a valid Norito archive") {
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(byteArrayOf(0x01, 0x02))
        }
        assertIllegalArgumentContains("bundleArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveCompactPaymentTokenProver
                .recursiveSpendCompactPaymentTokenFromBundle(kagemushaNoritoFrame(0x4b))
        }
        val emptyCompactToken = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                ByteArray(0),
                validRecursiveCompactVerifierKeys,
            )
        }
        assertTrue(emptyCompactToken.message.orEmpty().contains("compactTokenArchive"))
        val oversizedCompactToken = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                oversizedRecursiveCompactInput,
                validRecursiveCompactVerifierKeys,
            )
        }
        assertTrue(oversizedCompactToken.message.orEmpty().contains("compactTokenArchive must not exceed"))
        val malformedCompactToken = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                byteArrayOf(0x01, 0x02),
                validRecursiveCompactVerifierKeys,
            )
        }
        assertTrue(
            malformedCompactToken.message.orEmpty().contains("valid Norito archive"),
        )
        val emptyPayloadCompactToken = assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                kagemushaNoritoFrame(0x4b),
                validRecursiveCompactVerifierKeys,
            )
        }
        assertTrue(
            emptyPayloadCompactToken.message.orEmpty().contains("non-empty Norito payload"),
        )
        assertIllegalArgumentContains("recursiveCompactVerifierKeysArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput,
                ByteArray(0),
            )
        }
        assertIllegalArgumentContains("recursiveCompactVerifierKeysArchive must not exceed") {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput,
                oversizedRecursiveCompactInput,
            )
        }
        assertIllegalArgumentContains("recursiveCompactVerifierKeysArchive must be a valid Norito archive") {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput,
                byteArrayOf(0x01, 0x02),
            )
        }
        assertIllegalArgumentContains("recursiveCompactVerifierKeysArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validRecursiveCompactInput,
                kagemushaNoritoFrame(0xe2),
            )
        }
        assertIllegalArgumentContains("verifierRecordArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(validRecursiveCompactInput, ByteArray(0))
        }
        assertIllegalArgumentContains("verifierRecordArchive must not exceed") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput,
                    oversizedRecursiveCompactInput,
                )
        }
        assertIllegalArgumentContains("verifierRecordArchive must be a valid Norito archive") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput,
                    byteArrayOf(0x01, 0x02),
                )
        }
        assertIllegalArgumentContains("verifierRecordArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(
                    validRecursiveCompactInput,
                    kagemushaNoritoFrame(0x4b),
                )
        }
        assertIllegalArgumentContains("blockHeight must be non-negative") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    -1L,
                )
        }
        assertIllegalArgumentContains("compactTokenArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    ByteArray(0),
                    validRecursiveCompactInput,
                    Long.MAX_VALUE,
                )
        }
        assertIllegalArgumentContains("compactTokenArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    ByteArray(0),
                    validRecursiveCompactInput,
                    "9223372036854775808",
                )
        }
        assertIllegalArgumentContains("compactTokenArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    ByteArray(0),
                    validRecursiveCompactInput,
                    BigInteger("18446744073709551615"),
                )
        }
        assertIllegalArgumentContains("blockHeight must be a canonical unsigned decimal integer") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    "01",
                )
        }
        assertIllegalArgumentContains("blockHeight must be a canonical unsigned decimal integer") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    " 1",
                )
        }
        assertIllegalArgumentContains("blockHeight must fit in u64") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    "18446744073709551616",
                )
        }
        assertIllegalArgumentContains("blockHeight must be non-negative") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    BigInteger("-1"),
                )
        }
        assertIllegalArgumentContains("blockHeight must not be null") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    null as String?,
                )
        }
        assertIllegalArgumentContains("blockHeight must not be null") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    validRecursiveCompactInput,
                    validRecursiveCompactInput,
                    null as BigInteger?,
                )
        }
    }

    @Test
    fun lineageKeyArtifactPackagesValidateReleaseProfiles() {
        assertTrue(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(2))
        assertTrue(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(128))
        assertFalse(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(3))
        assertFalse(KagemushaRecursiveSpendProver.isSupportedLineageKeyArtifactOpeningLen(0))

        val initVerifierKey = lineageVerifierKey(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            0xa1.toByte(),
        )
        val initProvingKeyArchive = lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            initVerifierKey,
            0xa2.toByte(),
        )
        val appendVerifierKey = lineageVerifierKey(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            0xa3.toByte(),
        )
        val appendProvingKeyArchive = lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            appendVerifierKey,
            0xa4.toByte(),
        )
        val verifierKey = initVerifierKey.copyOf()
        val provingKeyArchive = initProvingKeyArchive.copyOf()
        val initArtifacts = KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
            2,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            verifierKey,
            provingKeyArchive,
        )
        assertTrue(initArtifacts.isInitArtifact())
        assertFalse(initArtifacts.isAppendArtifact())
        assertContentEquals(initVerifierKey, initArtifacts.lineageVerifierKey())
        assertContentEquals(initProvingKeyArchive, initArtifacts.lineageProvingKeyArchive())
        assertTrue(
            KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(initArtifacts) === initArtifacts,
        )

        verifierKey[0] = 0
        provingKeyArchive[0] = 0
        assertEquals(0x5a.toByte(), initArtifacts.lineageVerifierKey()[0])
        assertContentEquals(initProvingKeyArchive, initArtifacts.lineageProvingKeyArchive())
        val exposedVerifierKey = initArtifacts.lineageVerifierKey()
        exposedVerifierKey[0] = 0
        assertEquals(0x5a.toByte(), initArtifacts.lineageVerifierKey()[0])
        val exposedProvingKeyArchive = initArtifacts.lineageProvingKeyArchive()
        exposedProvingKeyArchive[0] = 0
        assertContentEquals(initProvingKeyArchive, initArtifacts.lineageProvingKeyArchive())

        val appendArtifacts = KagemushaRecursiveSpendProver.lineageKeyArtifactsForAppend(
            2,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            appendVerifierKey,
            appendProvingKeyArchive,
        )
        assertFalse(appendArtifacts.isInitArtifact())
        assertTrue(appendArtifacts.isAppendArtifact())

        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    appendVerifierKey,
                    appendProvingKeyArchive,
                )
            }.message,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    appendProvingKeyArchive,
                )
            }.message,
        )
        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    "not-zk1".toByteArray(Charsets.UTF_8),
                    initProvingKeyArchive,
                )
            }.message,
        )
        val duplicateCidVerifierKey = initVerifierKey + zk1Tlv(
            "CID1",
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                .toByteArray(Charsets.UTF_8),
        )
        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    duplicateCidVerifierKey,
                    initProvingKeyArchive,
                )
            }.message,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    "not-norito".toByteArray(Charsets.UTF_8),
                )
            }.message,
        )
        val missingCircuitArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(64) { 0xa5.toByte() },
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    missingCircuitArchive,
                )
            }.message,
        )
        val smuggledCircuitArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey =
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                    .toByteArray(Charsets.UTF_8) +
                    ByteArray(64) { 0xa6.toByte() },
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    smuggledCircuitArchive,
                )
            }.message,
        )
        val wrongCommitmentArchive = lineageProvingKeyArchive(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            appendVerifierKey,
            0xa6.toByte(),
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    wrongCommitmentArchive,
                )
            }.message,
        )
        val smuggledCommitmentArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(appendVerifierKey),
            provingKey = verifierKeyCommitment(initVerifierKey) + ByteArray(64) { 0xa7.toByte() },
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    smuggledCommitmentArchive,
                )
            }.message,
        )
        val wrongVersionArchive = lineageProvingKeyArchiveRaw(
            version = 2,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(64) { 0xa8.toByte() },
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    wrongVersionArchive,
                )
            }.message,
        )
        val emptyProvingKeyArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(0),
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    emptyProvingKeyArchive,
                )
            }.message,
        )
        val trailingPayloadArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(64) { 0xa9.toByte() },
            trailingPayload = byteArrayOf(0x7f),
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    trailingPayloadArchive,
                )
            }.message,
        )
        val oldSchemaArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(64) { 0xaa.toByte() },
            schemaHash = oldLineageProvingKeyArchiveSchemaHash,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    oldSchemaArchive,
                )
            }.message,
        )
        val packedStructArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(64) { 0xab.toByte() },
            flags = TEST_NORITO_COMPACT_LEN_FLAG or TEST_NORITO_PACKED_STRUCT_FLAG,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    packedStructArchive,
                )
            }.message,
        )
        val fieldBitsetArchive = lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifierKeyCommitment = verifierKeyCommitment(initVerifierKey),
            provingKey = ByteArray(64) { 0xac.toByte() },
            flags = TEST_NORITO_COMPACT_LEN_FLAG or TEST_NORITO_FIELD_BITSET_FLAG,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    fieldBitsetArchive,
                )
            }.message,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    initVerifierKey,
                    kagemushaNoritoFrame(0x9a),
                )
            }.message,
        )

        assertEquals(
            "proof_circuit_id",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifacts(
                    "kagemusha-recursive-spend-lineage-forged-circuit",
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    ByteArray(64) { 0xE7.toByte() },
                    ByteArray(64) { 0xE8.toByte() },
                )
            }.message,
        )
        assertEquals(
            "verifier_opening_len",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    3,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    ByteArray(64) { 0xE7.toByte() },
                    ByteArray(64) { 0xE8.toByte() },
                )
            }.message,
        )
        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    "halo2/kzg",
                    ByteArray(64) { 0xE7.toByte() },
                    ByteArray(64) { 0xE8.toByte() },
                )
            }.message,
        )
        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    ByteArray(0),
                    ByteArray(64) { 0xE8.toByte() },
                )
            }.message,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    2,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    ByteArray(64) { 0xE7.toByte() },
                    ByteArray(0),
                )
            }.message,
        )
    }

    @Test
    fun lineageKeyArtifactsRejectJavaNullsWithStableFieldMarkers() {
        assertEquals(
            "lineage_key_artifacts",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(null)
            }.message,
        )
        assertEquals(
            "proof_circuit_id",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifacts(
                    null,
                    128,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    byteArrayOf(1),
                    byteArrayOf(2),
                )
            }.message,
        )
        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    128,
                    null,
                    byteArrayOf(1),
                    byteArrayOf(2),
                )
            }.message,
        )
        assertEquals(
            "lineage_verifier_key",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    null,
                    byteArrayOf(2),
                )
            }.message,
        )
        assertEquals(
            "lineage_proving_key_archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                    byteArrayOf(1),
                    null,
                )
            }.message,
        )
    }

    @Test
    fun nativeArchiveEntrypointsRejectJavaNullsWithStableFieldMarkers() {
        val validArchive = kagemushaNoritoFrameWithPayload(0x4b)
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.initSpend(null)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.appendSpend(null)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.transitionProfileInit(null)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.transitionProfileAppend(null)
        }
        assertIllegalArgumentContains("profileArchive must not be empty") {
            KagemushaRecursiveSpendProver.lineageAppendBoundary(null)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(null, validArchive)
        }
        assertIllegalArgumentContains("bundleArchive must not be empty") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(validArchive, null)
        }
        assertIllegalArgumentContains("previousWitnessArchive must not be empty") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(null, validArchive, validArchive)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(validArchive, null, validArchive)
        }
        assertIllegalArgumentContains("bundleArchive must not be empty") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(validArchive, validArchive, null)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.verifySpend(null)
        }
        assertIllegalArgumentContains("requestArchive must not be empty") {
            KagemushaRecursiveSpendProver.redeemSpend(null)
        }
        assertIllegalArgumentContains("recordBundleArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    null,
                    validArchive,
                    validArchive,
                )
        }
        assertIllegalArgumentContains("pallasOpenEnvelopesArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    null,
                    validArchive,
                )
        }
        assertIllegalArgumentContains("recursiveCompactKeyArtifactsArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    validArchive,
                    null,
                )
        }
        assertIllegalArgumentContains("compactTokenArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                null,
                validArchive,
            )
        }
        assertIllegalArgumentContains("recursiveCompactVerifierKeysArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver.verifyRecursiveCompactPaymentToken(
                validArchive,
                null,
            )
        }
        assertIllegalArgumentContains("compactTokenArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(null, validArchive)
        }
        assertIllegalArgumentContains("verifierRecordArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjection(validArchive, null)
        }
        assertIllegalArgumentContains("compactTokenArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(null, validArchive, 0L)
        }
        assertIllegalArgumentContains("verifierRecordArchive must not be empty") {
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(validArchive, null, 0L)
        }
    }

    @Test
    fun sharedRecursiveSpendAbi6FixtureMatchesSdkSurface() {
        val manifest = sharedRecursiveSpendManifest()
        assertContains(manifest, "\"schema\": \"iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1\"")
        assertContains(
            manifest,
            "\"bridge_abi_version\": ${KagemushaRecursiveSpendProver.REQUIRED_BRIDGE_ABI_VERSION}",
        )
        assertContains(manifest, "\"operation_count\": 9")
        assertContains(
            manifest,
            "\"recursive_aggregation\": \"${KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1}\"",
        )
        assertContains(
            manifest,
            "\"reserved_lineage\": \"${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1}\"",
        )
        assertContains(
            manifest,
            "\"reserved_lineage_one_hop\": \"${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1}\"",
        )
        assertContains(
            manifest,
            "\"reserved_lineage_append\": \"${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1}\"",
        )
        assertContains(
            manifest,
            "\"compact_token_max_hops\": ${KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS}",
        )
        assertContains(
            manifest,
            "\"reserved_lineage_witnessless_max_hops\": ${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1}",
        )
        assertContains(
            manifest,
            "\"previous_proof_open_envelopes_required_count\": ${KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1}",
        )
        assertContains(
            manifest,
            "\"previous_proof_open_envelopes_max_bytes\": ${KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES}",
        )
        assertContains(
            manifest,
            "\"pallas_open_envelope_max_transcript_label_bytes\": ${KagemushaRecursiveSpendProver.RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES}",
        )
        assertContains(
            manifest,
            "\"native_archive_max_bytes\": ${KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES}",
        )
        assertContains(
            manifest,
            "\"transition_profile\": \"${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN}\"",
        )
        assertContains(
            manifest,
            "\"lineage_append_boundary_final_note_binding\": \"${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1}\"",
        )
        for (symbol in listOf(
            "connect_norito_kagemusha_recursive_spend_init",
            "connect_norito_kagemusha_recursive_spend_append",
            "connect_norito_kagemusha_recursive_spend_transition_profile_init",
            "connect_norito_kagemusha_recursive_spend_transition_profile_append",
            "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
            "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
            "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
            "connect_norito_kagemusha_recursive_spend_verify",
            "connect_norito_kagemusha_recursive_spend_redeem",
        )) {
            assertContains(manifest, "\"symbol\": \"$symbol\"")
        }
        assertContains(manifest, "\"reserved_lineage_payload_bytes\": 3847")
        assertContains(manifest, "\"reserved_lineage_transition_profile_bytes\": 2817")
        val archives = sharedRecursiveSpendFixture("archives.json")
        assertContains(
            archives,
            "\"schema\": \"iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1\"",
        )
        for (archiveName in listOf(
            "init_request",
            "init_bundle",
            "transition_profile_init",
            "append_request",
            "append_bundle",
            "transition_profile_append",
            "lineage_append_boundary",
            "lineage_witness_from_init_result",
            "lineage_witness_append_result",
            "verify_request",
            "verify_result",
            "redeem_request",
            "redeem_instruction",
        )) {
            assertContains(archives, "\"name\": \"$archiveName\"")
        }
        assertContains(archives, "\"operation\": \"redeem\"")
        assertContains(archives, "\"norito_type\": \"KagemushaRecursiveSpendRedeemRequestV1\"")
        assertContains(archives, "\"norito_type\": \"RedeemKagemushaRecursive\"")
        assertContains(archives, "\"request_archive_fields\"")
        assertContains(archives, "\"norito_type\": \"KagemushaRecursiveSpendInitRequestV1\"")
        assertContains(archives, "\"norito_type\": \"KagemushaRecursiveSpendAppendRequestV1\"")
        assertContains(archives, "\"name\": \"lineage_verifier_key\"")
        assertContains(archives, "\"name\": \"lineage_proving_key_archive\"")
        assertContains(archives, "\"name\": \"previous_recursive_proof_open_envelopes_archive\"")
        assertContains(archives, "\"name\": \"lineage_verifier_record\"")
        assertContains(archives, "\"name\": \"lineage_witness\"")
        assertContains(archives, "\"name\": \"block_height\"")
        assertContains(archives, "\"type\": \"Option<u64>\"")
        assertContains(archives, "\"norito_default\": true")
        assertContains(archives, "\"semantics\": \"verifier_record_activation_height\"")
        assertContains(
            archives,
            "\"sha256_hex\": \"f5a4a6a25fd9bfd8a121893ddb0c977753c16d8b9dfd835477d2965957c7c03e\"",
        )
        assertContains(
            archives,
            "\"sha256_hex\": \"88f293dccb455b6fbcd85d7c06426ce45f02a42fc330e68afda490d504903c03\"",
        )

        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(1),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(63),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.preferredAppendOutputCircuitId(64),
        )
        assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(0))
        assertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(63))
        assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(64))
        assertTrue(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                2,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.canRedeemWitnessless(
                KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                65,
            ),
        )
    }

    @Test
    fun rejectsEmptyArchivesBeforeNativeDispatch() {
        val validArchive = kagemushaNoritoFrameWithPayload(0x4b)

        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.initSpend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.appendSpend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.transitionProfileInit(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.transitionProfileAppend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageAppendBoundary(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(ByteArray(0), validArchive)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(validArchive, ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                ByteArray(0),
                validArchive,
                validArchive,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                ByteArray(0),
                validArchive,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                validArchive,
                ByteArray(0),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.verifySpend(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.redeemSpend(ByteArray(0))
        }
    }

    @Test
    fun copiesNativeInputArchivesBeforeDispatch() {
        val archive = kagemushaNoritoFrameWithPayload(0x4c)
        val expected = archive.copyOf()
        val ownedArchive = KagemushaRecursiveSpendProver.ownedNativeInput(
            archive,
            "requestArchive",
        )

        archive[6] = 0x7f.toByte()

        assertFalse(ownedArchive === archive)
        assertContentEquals(expected, ownedArchive)
        assertEquals(
            "requestArchive must not be empty",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.ownedNativeInput(ByteArray(0), "requestArchive")
            }.message,
        )
        assertIllegalArgumentContains("requestArchive must not exceed") {
            KagemushaRecursiveSpendProver.ownedNativeInput(
                ByteArray(KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1),
                "requestArchive",
            )
        }
        assertEquals(
            "requestArchive must be a valid Norito archive",
            assertFailsWith<IllegalArgumentException> {
                KagemushaRecursiveSpendProver.ownedNativeInput(byteArrayOf(0x01), "requestArchive")
            }.message,
        )
    }

    @Test
    fun rejectsMalformedAndEmptyPayloadArchivesBeforeNativeDispatch() {
        val validArchive = kagemushaNoritoFrameWithPayload(0x4b)
        val malformedArchive = byteArrayOf(0x01, 0x02)
        val emptyPayloadArchive = kagemushaNoritoFrame(0x4b)
        val oversizedArchive = ByteArray(KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1)

        for (entrypoint in listOf(
            KagemushaRecursiveSpendProver::initSpend,
            KagemushaRecursiveSpendProver::appendSpend,
            KagemushaRecursiveSpendProver::transitionProfileInit,
            KagemushaRecursiveSpendProver::transitionProfileAppend,
            KagemushaRecursiveSpendProver::verifySpend,
            KagemushaRecursiveSpendProver::redeemSpend,
        )) {
            assertIllegalArgumentContains("requestArchive must be a valid Norito archive") {
                entrypoint(malformedArchive)
            }
            assertIllegalArgumentContains("requestArchive must contain a non-empty Norito payload") {
                entrypoint(emptyPayloadArchive)
            }
        }

        assertIllegalArgumentContains("requestArchive must not exceed") {
            KagemushaRecursiveSpendProver.initSpend(oversizedArchive)
        }

        assertIllegalArgumentContains("profileArchive must be a valid Norito archive") {
            KagemushaRecursiveSpendProver.lineageAppendBoundary(malformedArchive)
        }
        assertIllegalArgumentContains("profileArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveSpendProver.lineageAppendBoundary(emptyPayloadArchive)
        }
        assertIllegalArgumentContains("profileArchive must not exceed") {
            KagemushaRecursiveSpendProver.lineageAppendBoundary(oversizedArchive)
        }

        assertIllegalArgumentContains("requestArchive must be a valid Norito archive") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                malformedArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("bundleArchive must be a valid Norito archive") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive,
                malformedArchive,
            )
        }
        assertIllegalArgumentContains("requestArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                emptyPayloadArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("bundleArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive,
                emptyPayloadArchive,
            )
        }
        assertIllegalArgumentContains("requestArchive must not exceed") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                oversizedArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("bundleArchive must not exceed") {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(
                validArchive,
                oversizedArchive,
            )
        }

        assertIllegalArgumentContains("previousWitnessArchive must be a valid Norito archive") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                malformedArchive,
                validArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("requestArchive must be a valid Norito archive") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                malformedArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("bundleArchive must be a valid Norito archive") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                validArchive,
                malformedArchive,
            )
        }
        assertIllegalArgumentContains("previousWitnessArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                emptyPayloadArchive,
                validArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("requestArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                emptyPayloadArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("bundleArchive must contain a non-empty Norito payload") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                validArchive,
                emptyPayloadArchive,
            )
        }
        assertIllegalArgumentContains("previousWitnessArchive must not exceed") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                oversizedArchive,
                validArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("requestArchive must not exceed") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                oversizedArchive,
                validArchive,
            )
        }
        assertIllegalArgumentContains("bundleArchive must not exceed") {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                validArchive,
                validArchive,
                oversizedArchive,
            )
        }
    }

    @Test
    fun nativeProbeRequiresAbiSixAndAllSymbols() {
        assertTrue(
            KagemushaRecursiveSpendProver.expectIllegalArgumentProbe {
                throw IllegalArgumentException("malformed probe")
            },
        )
        assertFalse(KagemushaRecursiveSpendProver.expectIllegalArgumentProbe {})
        assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.expectIllegalArgumentProbe {
                throw IllegalStateException("accepted malformed probe before backend work")
            }
        }

        assertTrue(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertTrue(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 7 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
                requiredBridgeAbiVersion = 7,
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 5 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { false },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = { throw UnsatisfiedLinkError("missing bridge") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw UnsatisfiedLinkError("missing abi symbol") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { throw UnsatisfiedLinkError("missing recursive symbol") },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = { throw IllegalArgumentException("bad bridge load") },
                bridgeAbiVersion = { 6 },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { throw IllegalArgumentException("bad abi probe") },
                probeSymbol = { true },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { throw IllegalArgumentException("bad malformed probe") },
            ),
        )
        assertFalse(
            KagemushaRecursiveSpendProver.detectNativeAvailability(
                loadLibrary = {},
                bridgeAbiVersion = { 6 },
                probeSymbol = { throw IllegalStateException("bad malformed probe") },
            ),
        )
    }

    @Test
    fun rejectsNullAndEmptyNativeRedeemOutput() {
        val missing = assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(null, "redeem")
        }
        assertTrue(missing.message.orEmpty().contains("native redeem returned no output"))

        val empty = assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(ByteArray(0), "redeem")
        }
        assertTrue(empty.message.orEmpty().contains("native redeem returned empty output"))

        val oversized = assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                ByteArray(KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES + 1),
                "redeem",
            )
        }
        assertTrue(oversized.message.orEmpty().contains("native redeem returned oversized output"))

        val malformed = assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                byteArrayOf(0x01, 0x02),
                "redeem",
            )
        }
        assertTrue(malformed.message.orEmpty().contains("native redeem returned invalid Norito archive"))

        val compressed = kagemushaNoritoFrameWithPayload(0x4b)
        compressed[22] = 1
        assertRejectsMalformedNativeRedeemOutput(compressed)

        val unsupportedFlags = kagemushaNoritoFrameWithPayload(0x4b)
        unsupportedFlags[39] = 0x08
        assertRejectsMalformedNativeRedeemOutput(unsupportedFlags)

        val invalidFieldBitset = kagemushaNoritoFrameWithPayload(0x4b)
        invalidFieldBitset[39] = 0x20
        assertRejectsMalformedNativeRedeemOutput(invalidFieldBitset)

        assertRejectsMalformedNativeRedeemOutput(
            withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), byteArrayOf(0x7f)),
        )
        assertRejectsMalformedNativeRedeemOutput(
            withHeaderPadding(kagemushaNoritoFrameWithPayload(0x4b), ByteArray(65)),
        )

        val emptyPayload = assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(
                kagemushaNoritoFrame(0x4b),
                "redeem",
            )
        }
        assertTrue(emptyPayload.message.orEmpty().contains("native redeem returned empty Norito payload"))

        val output = kagemushaNoritoFrameWithPayload(0x4b)
        assertTrue(
            output === KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(output, "redeem"),
        )
    }

    private fun assertRejectsMalformedNativeRedeemOutput(output: ByteArray) {
        val error = assertFailsWith<IllegalStateException> {
            KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(output, "redeem")
        }
        assertTrue(error.message.orEmpty().contains("native redeem returned invalid Norito archive"))
    }

    private fun sharedRecursiveSpendManifest(): String {
        return sharedRecursiveSpendFixture("manifest.json")
    }

    private fun sharedRecursiveSpendFixture(fileName: String): String {
        var directory: Path? = Paths.get("").toAbsolutePath()
        while (directory != null) {
            val candidate = directory.resolve("fixtures/kagemusha_recursive_spend_abi6").resolve(fileName)
            if (Files.isRegularFile(candidate)) {
                return String(Files.readAllBytes(candidate), Charsets.UTF_8)
            }
            directory = directory.parent
        }
        error("missing shared recursive spend ABI-6 fixture $fileName")
    }

    private fun assertContains(
        text: String,
        needle: String,
    ) {
        assertTrue(text.contains(needle), "missing shared fixture marker: $needle")
    }

    private fun assertIllegalArgumentContains(
        expected: String,
        block: () -> Unit,
    ) {
        val error = assertFailsWith<IllegalArgumentException> {
            block()
        }
        assertTrue(
            error.message.orEmpty().contains(expected),
            "expected IllegalArgumentException to contain '$expected', actual: '${error.message}'",
        )
    }

    private fun kagemushaNoritoFrame(schemaByte: Int): ByteArray {
        val frame = ByteArray(40)
        frame[0] = 'N'.code.toByte()
        frame[1] = 'R'.code.toByte()
        frame[2] = 'T'.code.toByte()
        frame[3] = '0'.code.toByte()
        frame.fill(schemaByte.toByte(), 6, 22)
        return frame
    }

    private fun kagemushaNoritoFrameWithPayload(schemaByte: Int): ByteArray {
        val frame = ByteArray(45)
        kagemushaNoritoFrame(schemaByte).copyInto(frame, 0)
        frame[23] = 3.toByte()
        byteArrayOf(
            0xb9.toByte(),
            0xd3.toByte(),
            0xa8.toByte(),
            0x0c.toByte(),
            0xcd.toByte(),
            0x5d.toByte(),
            0x13.toByte(),
            0x24.toByte(),
        ).copyInto(frame, 31)
        frame[42] = 0xa5.toByte()
        frame[43] = 0x5a.toByte()
        frame[44] = 0x11.toByte()
        return frame
    }

    private fun withHeaderPadding(archive: ByteArray, padding: ByteArray): ByteArray {
        val padded = ByteArray(archive.size + padding.size)
        archive.copyInto(padded, endIndex = 40)
        padding.copyInto(padded, destinationOffset = 40)
        archive.copyInto(
            destination = padded,
            destinationOffset = 40 + padding.size,
            startIndex = 40,
        )
        return padded
    }

    private fun kagemushaNoritoFrameFromPayload(schemaByte: Int, payload: ByteArray): ByteArray {
        val frame = kagemushaNoritoFrame(schemaByte) + payload
        writeLongLittleEndian(frame, 23, payload.size.toLong())
        writeLongLittleEndian(frame, 31, testCrc64(payload))
        return frame
    }

    private val lineageProvingKeyArchiveSchemaHash =
        byteArrayOf(
            0xc8.toByte(), 0x84.toByte(), 0x89.toByte(), 0x61.toByte(),
            0x8a.toByte(), 0x01, 0x2c, 0x28,
            0x3f, 0xf3.toByte(), 0xbb.toByte(), 0x2e.toByte(),
            0xba.toByte(), 0xbc.toByte(), 0x77, 0x75,
        )

    private val oldLineageProvingKeyArchiveSchemaHash =
        byteArrayOf(
            0x11, 0x9f.toByte(), 0x4d, 0xf3.toByte(),
            0x8a.toByte(), 0x98.toByte(), 0xef.toByte(), 0x58,
            0x48, 0xad.toByte(), 0x0a, 0xad.toByte(),
            0xb9.toByte(), 0x71, 0x57, 0x79,
        )

    private fun kagemushaNoritoFrameFromSchemaHash(
        schemaHash: ByteArray,
        payload: ByteArray,
        flags: Int = TEST_NORITO_COMPACT_LEN_FLAG,
    ): ByteArray {
        val frame = ByteArray(40 + payload.size)
        "NRT0".toByteArray(Charsets.US_ASCII).copyInto(frame, 0)
        schemaHash.copyInto(frame, 6)
        frame[39] = flags.toByte()
        payload.copyInto(frame, 40)
        writeLongLittleEndian(frame, 23, payload.size.toLong())
        writeLongLittleEndian(frame, 31, testCrc64(payload))
        return frame
    }

    private fun kagemushaNoritoLength(
        value: Int,
        flags: Int = TEST_NORITO_COMPACT_LEN_FLAG,
    ): ByteArray {
        if ((flags and TEST_NORITO_COMPACT_LEN_FLAG) == 0) {
            val encoded = ByteArray(8)
            writeLongLittleEndian(encoded, 0, value.toLong())
            return encoded
        }
        var remaining = value
        val bytes = ArrayList<Byte>()
        while (remaining >= 0x80) {
            bytes.add(((remaining and 0x7f) or 0x80).toByte())
            remaining = remaining ushr 7
        }
        bytes.add(remaining.toByte())
        return bytes.toByteArray()
    }

    private fun kagemushaNoritoField(
        payload: ByteArray,
        flags: Int = TEST_NORITO_COMPACT_LEN_FLAG,
    ): ByteArray = kagemushaNoritoLength(payload.size, flags) + payload

    private fun kagemushaNoritoString(
        value: String,
        flags: Int = TEST_NORITO_COMPACT_LEN_FLAG,
    ): ByteArray {
        val bytes = value.toByteArray(Charsets.UTF_8)
        return kagemushaNoritoLength(bytes.size, flags) + bytes
    }

    private fun kagemushaNoritoByteVec(bytes: ByteArray): ByteArray {
        val encoded = ByteArray(8)
        writeLongLittleEndian(encoded, 0, bytes.size.toLong())
        return encoded + bytes
    }

    private fun zk1Tlv(tag: String, payload: ByteArray): ByteArray {
        val tagBytes = tag.toByteArray(Charsets.US_ASCII)
        val encoded = ByteArray(8 + payload.size)
        tagBytes.copyInto(encoded, 0)
        writeIntLittleEndian(encoded, 4, payload.size)
        payload.copyInto(encoded, 8)
        return encoded
    }

    private fun lineageVerifierKey(circuitId: String, seed: Byte): ByteArray =
        byteArrayOf(0x5a, 0x4b, 0x31, 0x00) +
            zk1Tlv("IPAK", byteArrayOf(8, 0, 0, 0)) +
            zk1Tlv("CID1", circuitId.toByteArray(Charsets.UTF_8)) +
            zk1Tlv("H2VK", ByteArray(32) { seed })

    private fun lineageProvingKeyArchive(
        circuitId: String,
        verifierKey: ByteArray,
        seed: Byte,
    ): ByteArray =
        lineageProvingKeyArchiveRaw(
            version = 1,
            circuitId = circuitId,
            verifierKeyCommitment = verifierKeyCommitment(verifierKey),
            provingKey = ByteArray(64) { seed },
        )

    private fun lineageProvingKeyArchiveRaw(
        version: Int,
        circuitId: String,
        verifierKeyCommitment: ByteArray,
        provingKey: ByteArray,
        flags: Int = TEST_NORITO_COMPACT_LEN_FLAG,
        schemaHash: ByteArray = lineageProvingKeyArchiveSchemaHash,
        trailingPayload: ByteArray = ByteArray(0),
    ): ByteArray {
        val versionBytes = ByteArray(2)
        writeShortLittleEndian(versionBytes, 0, version)
        val payload =
            kagemushaNoritoField(versionBytes, flags) +
                kagemushaNoritoField(kagemushaNoritoString(circuitId, flags), flags) +
                kagemushaNoritoField(verifierKeyCommitment, flags) +
                kagemushaNoritoField(kagemushaNoritoByteVec(provingKey), flags) +
                trailingPayload
        return kagemushaNoritoFrameFromSchemaHash(
            schemaHash,
            payload,
            flags,
        )
    }

    private fun verifierKeyCommitment(verifierKey: ByteArray): ByteArray {
        val backend =
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND.toByteArray(Charsets.UTF_8)
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update("iroha:zk:v1:vk".toByteArray(Charsets.US_ASCII))
        digest.update(longBigEndian(backend.size.toLong()))
        digest.update(backend)
        digest.update(longBigEndian(verifierKey.size.toLong()))
        digest.update(verifierKey)
        return digest.digest()
    }

    private val testCrc64Table: LongArray = run {
        val table = LongArray(256)
        val reflectedPoly = -3932672073523589310L
        for (index in table.indices) {
            var crc = index.toLong()
            for (bit in 0 until 8) {
                crc = if ((crc and 1L) != 0L) {
                    (crc ushr 1) xor reflectedPoly
                } else {
                    crc ushr 1
                }
            }
            table[index] = crc
        }
        table
    }

    private fun testCrc64(payload: ByteArray): Long {
        var crc = -1L
        for (byte in payload) {
            crc = testCrc64Table[(crc.toInt() xor byte.toInt()) and 0xff] xor (crc ushr 8)
        }
        return crc xor -1L
    }

    private fun writeIntLittleEndian(bytes: ByteArray, offset: Int, value: Int) {
        for (index in 0 until 4) {
            bytes[offset + index] = ((value ushr (index * 8)) and 0xff).toByte()
        }
    }

    private fun writeShortLittleEndian(bytes: ByteArray, offset: Int, value: Int) {
        for (index in 0 until 2) {
            bytes[offset + index] = ((value ushr (index * 8)) and 0xff).toByte()
        }
    }

    private fun writeLongLittleEndian(bytes: ByteArray, offset: Int, value: Long) {
        for (index in 0 until 8) {
            bytes[offset + index] = ((value ushr (index * 8)) and 0xff).toByte()
        }
    }

    private fun longBigEndian(value: Long): ByteArray {
        val output = ByteArray(8)
        for (index in output.indices) {
            output[index] = ((value ushr ((7 - index) * 8)) and 0xff).toByte()
        }
        return output
    }
}
