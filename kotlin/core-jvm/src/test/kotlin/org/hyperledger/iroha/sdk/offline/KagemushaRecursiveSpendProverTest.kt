package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class KagemushaRecursiveSpendProverTest {
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
        assertEquals("recursive_spend_v1", KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1.wireName)
        assertEquals("checked_prefold_v1", KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1.wireName)
        assertEquals(
            KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
            KagemushaRecursiveSpendProver.preferredMode(true),
        )
        assertEquals(
            KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1,
            KagemushaRecursiveSpendProver.preferredMode(false),
        )
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
        assertContains(manifest, "\"native_archive_max_bytes\": 67108864")
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
        assertContains(
            archives,
            "\"sha256_hex\": \"b83b33541f50ab893ae356c1f42da60aaf81da95bc4daf871511509fc8eea5b2\"",
        )
        assertContains(
            archives,
            "\"sha256_hex\": \"a598660cbfe91a207b64a69b7a9dbdc985fd901c60fe886aecb4dead4115169e\"",
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
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(ByteArray(0), byteArrayOf(1))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessFromInitResult(byteArrayOf(1), ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                ByteArray(0),
                byteArrayOf(1),
                byteArrayOf(2),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                byteArrayOf(1),
                ByteArray(0),
                byteArrayOf(2),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRecursiveSpendProver.lineageWitnessAppendResult(
                byteArrayOf(1),
                byteArrayOf(2),
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
                ByteArray(KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES + 1),
                "redeem",
            )
        }
        assertTrue(oversized.message.orEmpty().contains("native redeem returned oversized output"))

        val output = byteArrayOf(0x01, 0x02)
        assertTrue(output === KagemushaRecursiveSpendProver.requireRecursiveSpendOutput(output, "redeem"))
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
}
