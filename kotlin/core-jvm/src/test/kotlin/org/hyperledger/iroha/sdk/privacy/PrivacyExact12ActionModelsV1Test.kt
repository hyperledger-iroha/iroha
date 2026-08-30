// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.core.model.NetworkId

class PrivacyExact12ActionModelsV1Test {
    @Test
    fun closedOperationProtocolAndEffectMappings() {
        val operations = PrivacyExact12ActionOperationV1.values().toList()
        assertEquals(13, operations.size)
        assertEquals(10, PrivacyLedgerEffectKindV1.values().size)
        assertEquals(
            listOf(
                "zk_ace_authorization_action_v1",
                "anonymous_pgc_payment_action_v1",
                "verange_range_proof_v1",
                "zk_ams_batch_admission_action_v1",
                "zk_ams_provision_account_action_v1",
                "vega_credential_presentation_v1",
                "zk_x509_identity_presentation_v1",
                "jindo_polynomial_evaluation_v1",
                "bootle_lantern_credential_presentation_v1",
                "orchard_note_action_v1",
                "fcmp_membership_payment_v1",
                "ivm_private_note_action_v1",
                "pq_masp_note_action_v1",
            ),
            operations.map { it.canonicalLabel },
        )
        assertEquals(
            listOf(
                PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0,
                PrivacyProtocolIdV1.ANONYMOUS_PGC_K_OUT_OF_N_V1,
                PrivacyProtocolIdV1.VERANGE_TRANSPARENT_RANGE_V1,
                PrivacyProtocolIdV1.IROHA_ZK_AMS_V1,
                PrivacyProtocolIdV1.IROHA_ZK_AMS_V1,
                PrivacyProtocolIdV1.VEGA_EXISTING_CREDENTIAL_ZK_V0,
                PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V0,
                PrivacyProtocolIdV1.IROHA_JINDO_POLYNOMIAL_COMMITMENT_V0,
                PrivacyProtocolIdV1.IROHA_BOOTLE_LANTERN_ANONCRED_V1,
                PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
                PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1,
                PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1,
                PrivacyProtocolIdV1.PQ_MASP_STARK_V0,
            ),
            operations.map { it.protocolId },
        )
        assertEquals(
            listOf(
                PrivacyLedgerEffectKindV1.ZK_ACE_TRANSPARENT_TRANSFER,
                PrivacyLedgerEffectKindV1.ANONYMOUS_PGC_ACCOUNT_STATE_TRANSITION,
                PrivacyLedgerEffectKindV1.VERIFICATION_ONLY,
                PrivacyLedgerEffectKindV1.ZK_AMS_BATCH_ADMISSION,
                PrivacyLedgerEffectKindV1.ZK_AMS_PROVISION_ACCOUNT,
                PrivacyLedgerEffectKindV1.VERIFICATION_ONLY,
                PrivacyLedgerEffectKindV1.ZK_X509_CERTIFICATE_NULLIFIER,
                PrivacyLedgerEffectKindV1.VERIFICATION_ONLY,
                PrivacyLedgerEffectKindV1.VERIFICATION_ONLY,
                PrivacyLedgerEffectKindV1.ORCHARD_NOTE_STATE_TRANSITION,
                PrivacyLedgerEffectKindV1.FCMP_MEMBERSHIP_PAYMENT,
                PrivacyLedgerEffectKindV1.IVM_PRIVATE_NOTE_STATE_TRANSITION,
                PrivacyLedgerEffectKindV1.PQ_MASP_NOTE_STATE_TRANSITION,
            ),
            operations.map { it.ledgerEffectKind },
        )
        assertEquals(
            PrivacyLedgerEffectKindV1.values().map { it.canonicalLabel }.toSet(),
            operations.map { it.ledgerEffectKind.canonicalLabel }.toSet(),
        )
    }

    @Test
    fun requestBoundsAndSnapshotsWireAndOptionalManifestDigest() {
        val wire = byteArrayOf(1, 2)
        val digest = fixed32(0x21)
        val request = PrivacyExact12ActionRequestV1(
            PrivacyExact12ActionOperationV1.ZK_AMS_PROVISION_ACCOUNT_ACTION_V1,
            wire,
            digest,
        )
        wire[0] = 0x7f
        digest[0] = 0x7f
        assertContentEquals(byteArrayOf(1, 2), request.signedTransactionVersioned)
        assertContentEquals(fixed32(0x21), request.expectedManifestDigest)

        PrivacyExact12ActionRequestV1(
            PrivacyExact12ActionOperationV1.VERANGE_RANGE_PROOF_V1,
            ByteArray(PrivacyExact12ActionRequestV1.MAX_SIGNED_TRANSACTION_BYTES) { 1 },
        )
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12ActionRequestV1(
                PrivacyExact12ActionOperationV1.VERANGE_RANGE_PROOF_V1,
                byteArrayOf(),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12ActionRequestV1(
                PrivacyExact12ActionOperationV1.VERANGE_RANGE_PROOF_V1,
                ByteArray(PrivacyExact12ActionRequestV1.MAX_SIGNED_TRANSACTION_BYTES + 1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12ActionRequestV1(
                PrivacyExact12ActionOperationV1.VERANGE_RANGE_PROOF_V1,
                byteArrayOf(1),
                ByteArray(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyExact12ActionRequestV1(
                PrivacyExact12ActionOperationV1.VERANGE_RANGE_PROOF_V1,
                byteArrayOf(1),
                ByteArray(31) { 1 },
            )
        }
    }

    @Test
    fun validSubmittedAndTerminalViews() {
        val submitted = view(
            PrivacyActionLocalStateV1.SUBMITTED,
            terminalChainState = null,
            committedHeight = null,
            rejectionReason = null,
        )
        assertEquals(PrivacyActionLocalStateV1.SUBMITTED, submitted.localState)
        assertNull(submitted.terminalChainState)

        for (terminal in listOf(
            PrivacyActionTerminalChainStateV1.COMMITTED,
            PrivacyActionTerminalChainStateV1.APPLIED,
        )) {
            val success = view(
                PrivacyActionLocalStateV1.TERMINAL,
                terminal,
                height(42),
                null,
            )
            assertEquals(height(42), success.committedHeight)
            assertEquals(terminal, success.terminalChainState)
            if (terminal == PrivacyActionTerminalChainStateV1.APPLIED) {
                assertContentEquals(fixed32(6), success.executionCapabilityManifestDigest)
                assertEquals(height(9), success.executionCapabilityCommittedHeight)
                assertEquals(height(43), success.executionReceiptFinalizedHeight)
                assertContentEquals(fixed32(7), success.executionReceiptFinalizedBlockHash)
            }
        }

        val rejected = view(
            PrivacyActionLocalStateV1.TERMINAL,
            PrivacyActionTerminalChainStateV1.REJECTED,
            height(43),
            "proof envelope expired",
        )
        assertEquals("proof envelope expired", rejected.rejectionReason)

        val expired = view(
            PrivacyActionLocalStateV1.TERMINAL,
            PrivacyActionTerminalChainStateV1.EXPIRED,
            null,
            null,
        )
        assertNull(expired.committedHeight)
    }

    @Test
    fun impossibleViewStateShapesFailClosed() {
        val hostile = listOf<() -> Unit>(
            {
                view(
                    PrivacyActionLocalStateV1.SUBMITTED,
                    PrivacyActionTerminalChainStateV1.COMMITTED,
                    null,
                    null,
                )
            },
            { view(PrivacyActionLocalStateV1.SUBMITTED, null, height(1), null) },
            { view(PrivacyActionLocalStateV1.TERMINAL, null, null, null) },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.COMMITTED,
                    null,
                    null,
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.APPLIED,
                    height(20),
                    null,
                    includeExecutionReceipt = false,
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.APPLIED,
                    height(20),
                    "unexpected",
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.REJECTED,
                    null,
                    "rejected",
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.REJECTED,
                    height(1),
                    " rejected ",
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.REJECTED,
                    height(1),
                    "policy\u0001rejected",
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.REJECTED,
                    height(1),
                    "é".repeat(513),
                )
            },
            {
                view(
                    PrivacyActionLocalStateV1.TERMINAL,
                    PrivacyActionTerminalChainStateV1.EXPIRED,
                    height(1),
                    null,
                )
            },
        )
        hostile.forEachIndexed { index, construct ->
            assertFailsWith<IllegalArgumentException>("accepted hostile state shape $index") {
                construct()
            }
        }
    }

    @Test
    fun viewRejectsMappingHashesAndHeightsThatCannotBeAuthenticated() {
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.SUBMITTED,
                null,
                null,
                null,
                protocolId = PrivacyProtocolIdV1.IROHA_ZK_AMS_V1,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.SUBMITTED,
                null,
                null,
                null,
                ledgerEffectKind = PrivacyLedgerEffectKindV1.VERIFICATION_ONLY,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.SUBMITTED,
                null,
                null,
                null,
                transactionHash = ByteArray(32),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.SUBMITTED,
                null,
                null,
                null,
                capabilityManifestDigest = ByteArray(31) { 1 },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.SUBMITTED,
                null,
                null,
                null,
                capabilityCommittedHeight = BigInteger.ZERO,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.TERMINAL,
                PrivacyActionTerminalChainStateV1.COMMITTED,
                BigInteger.ZERO,
                null,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.SUBMITTED,
                null,
                null,
                null,
                capabilityCommittedHeight = BigInteger.ONE.shiftLeft(64),
            )
        }
    }

    @Test
    fun appliedReceiptEvidenceIsCompleteOrderedAndDefensivelySnapshotted() {
        val executionDigest = fixed32(6)
        val finalizedBlockHash = fixed32(7)
        val applied = PrivacyActionOperationViewV1(
            PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
            PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1,
            fixed32(1),
            fixed32(2),
            fixed32(3),
            fixed32(4),
            PrivacyActionLocalStateV1.TERMINAL,
            PrivacyActionTerminalChainStateV1.APPLIED,
            height(42),
            null,
            PrivacyLedgerEffectKindV1.ORCHARD_NOTE_STATE_TRANSITION,
            fixed32(5),
            height(10),
            executionDigest,
            height(40),
            height(44),
            finalizedBlockHash,
        )
        executionDigest[0] = 0
        finalizedBlockHash[0] = 0
        assertContentEquals(fixed32(6), applied.executionCapabilityManifestDigest)
        assertContentEquals(fixed32(7), applied.executionReceiptFinalizedBlockHash)

        assertFailsWith<IllegalArgumentException> {
            PrivacyActionOperationViewV1(
                PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
                PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1,
                fixed32(1),
                fixed32(2),
                fixed32(3),
                fixed32(4),
                PrivacyActionLocalStateV1.TERMINAL,
                PrivacyActionTerminalChainStateV1.APPLIED,
                height(42),
                null,
                PrivacyLedgerEffectKindV1.ORCHARD_NOTE_STATE_TRANSITION,
                fixed32(5),
                height(10),
                fixed32(6),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            PrivacyActionOperationViewV1(
                PrivacyProtocolIdV1.ORCHARD_HALO2_ACTIONS_V1,
                PrivacyOperationSchemaV1.ORCHARD_NOTE_ACTION_V1,
                fixed32(1),
                fixed32(2),
                fixed32(3),
                fixed32(4),
                PrivacyActionLocalStateV1.TERMINAL,
                PrivacyActionTerminalChainStateV1.APPLIED,
                height(42),
                null,
                PrivacyLedgerEffectKindV1.ORCHARD_NOTE_STATE_TRANSITION,
                fixed32(5),
                height(10),
                fixed32(6),
                height(43),
                height(44),
                fixed32(7),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            view(
                PrivacyActionLocalStateV1.TERMINAL,
                PrivacyActionTerminalChainStateV1.REJECTED,
                height(9),
                "rejected",
            )
        }
    }

    @Test
    fun nativeInspectionProjectionIsExactAndSnapshotsBytes() {
        val projection = ByteArray(128) { index -> (index / 32 + 1).toByte() }
        val inspection = PrivacyExact12ActionInspectionV1(projection)
        projection[0] = 0x7f
        assertContentEquals(fixed32(1), inspection.transactionHash)
        assertContentEquals(fixed32(2), inspection.transactionIntentDigest)
        assertContentEquals(fixed32(3), inspection.statementDigest)
        assertContentEquals(fixed32(4), inspection.proofEnvelopeHash)

        assertFailsWith<IllegalStateException> {
            PrivacyExact12ActionInspectionV1(ByteArray(127) { 1 })
        }
        assertFailsWith<IllegalStateException> {
            PrivacyExact12ActionInspectionV1(
                ByteArray(128) { index -> if (index in 64 until 96) 0 else 1 },
            )
        }
    }

    @Test
    fun authenticatedProvenanceBindsClientNetworkAndSurvivesTerminalCopy() {
        val detached = view(
            PrivacyActionLocalStateV1.SUBMITTED,
            null,
            null,
            null,
        )
        val owner = PrivacyActionOperationProvenanceOwnerV1()
        val otherOwner = PrivacyActionOperationProvenanceOwnerV1()
        val network = NetworkId.fromBytes(ByteArray(32) { 1 })
        val otherNetwork = NetworkId.fromBytes(ByteArray(32) { index ->
            if (index == 0) 2 else 1
        })
        assertFailsWith<IllegalStateException> {
            detached.requireAuthenticatedProvenanceV1(owner, network)
        }

        detached.bindAuthenticatedSubmissionV1(owner, network)
        detached.requireAuthenticatedProvenanceV1(owner, network)
        assertFailsWith<IllegalStateException> {
            detached.requireAuthenticatedProvenanceV1(otherOwner, network)
        }
        assertFailsWith<IllegalStateException> {
            detached.requireAuthenticatedProvenanceV1(owner, otherNetwork)
        }

        val terminal = detached.withAuthenticatedTerminalStateV1(
            PrivacyActionTerminalChainStateV1.APPLIED,
            height(17),
            null,
            fixed32(6),
            height(9),
            height(18),
            fixed32(7),
        )
        terminal.requireAuthenticatedProvenanceV1(owner, network)
        assertTrue(terminal.localState == PrivacyActionLocalStateV1.TERMINAL)
        assertFalse(terminal === detached)
    }

    private fun view(
        localState: PrivacyActionLocalStateV1,
        terminalChainState: PrivacyActionTerminalChainStateV1?,
        committedHeight: BigInteger?,
        rejectionReason: String?,
        protocolId: PrivacyProtocolIdV1? = null,
        ledgerEffectKind: PrivacyLedgerEffectKindV1? = null,
        transactionHash: ByteArray? = null,
        capabilityManifestDigest: ByteArray? = null,
        capabilityCommittedHeight: BigInteger = height(10),
        includeExecutionReceipt: Boolean =
            terminalChainState == PrivacyActionTerminalChainStateV1.APPLIED,
    ): PrivacyActionOperationViewV1 {
        val operation = PrivacyExact12ActionOperationV1.ORCHARD_NOTE_ACTION_V1
        val executionFinalizedHeight = committedHeight?.add(BigInteger.ONE)
        return PrivacyActionOperationViewV1(
            protocolId ?: operation.protocolId,
            operation,
            transactionHash ?: fixed32(1),
            fixed32(2),
            fixed32(3),
            fixed32(4),
            localState,
            terminalChainState,
            committedHeight,
            rejectionReason,
            ledgerEffectKind ?: operation.ledgerEffectKind,
            capabilityManifestDigest ?: fixed32(5),
            capabilityCommittedHeight,
            if (includeExecutionReceipt) fixed32(6) else null,
            if (includeExecutionReceipt) height(9) else null,
            if (includeExecutionReceipt) executionFinalizedHeight else null,
            if (includeExecutionReceipt) fixed32(7) else null,
        )
    }

    private fun fixed32(value: Int): ByteArray = ByteArray(32) { value.toByte() }

    private fun height(value: Long): BigInteger = BigInteger.valueOf(value)
}
