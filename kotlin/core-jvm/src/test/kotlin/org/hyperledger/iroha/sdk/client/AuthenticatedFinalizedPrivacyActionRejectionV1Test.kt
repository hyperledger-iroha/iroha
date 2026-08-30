// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.privacy.PrivacyLedgerEffectKindV1
import org.hyperledger.iroha.sdk.privacy.PrivacyOperationSchemaV1
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1

class AuthenticatedFinalizedPrivacyActionRejectionV1Test {
    @Test
    fun closedSixCaseModelPreservesTypedActionAndDefensiveDigests() {
        assertEquals(
            listOf(
                "account_does_not_exist",
                "limit_check",
                "validation",
                "instruction_execution",
                "ivm_execution",
                "trigger_execution",
            ),
            AuthenticatedPrivacyActionRejectionCodeV1.values().map { it.canonicalLabel },
        )
        AuthenticatedPrivacyActionRejectionCodeV1.values().forEach { code ->
            val source = repeated(0x22)
            val rejection = rejection(code, source)
            source[0] = 0x44.toByte()
            assertEquals(code.canonicalLabel, rejection.rejectionCode.canonicalLabel)
            assertEquals(PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1,
                rejection.operationSchema)
            assertEquals(9L, rejection.committedBlockHeight)
            assertContentEquals(repeated(0x22), rejection.transactionIntentDigest)
            val escaped = rejection.transactionIntentDigest
            escaped[0] = 0x44.toByte()
            assertContentEquals(repeated(0x22), rejection.transactionIntentDigest)
        }
    }

    @Test
    fun unknownCodesAndContradictoryFinalityFailClosed() {
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedPrivacyActionRejectionCodeV1.fromCanonicalLabel("server_error")
        }
        assertFailsWith<IllegalArgumentException> {
            rejection(
                AuthenticatedPrivacyActionRejectionCodeV1.VALIDATION,
                repeated(0x22),
                checkpointHeight = 8,
            )
        }
    }

    @Test
    fun publicBridgeExposesPageAndProofArrayOverloads() {
        val methods = AuthenticatedPrivacyActionReceiptNativeBridge::class.java.methods
            .filter { it.name == "projectFinalizedPrivacyActionRejectionV1" }
        assertEquals(2, methods.size)
        assertTrue(methods.all { it.parameterCount == 6 })
    }

    private fun rejection(
        code: AuthenticatedPrivacyActionRejectionCodeV1,
        intent: ByteArray,
        checkpointHeight: Long = 9,
    ) = AuthenticatedFinalizedPrivacyActionRejectionV1(
        networkIdHex = hash(0x11),
        protocolId = PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0,
        operationSchema = PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1,
        ledgerEffectKind = PrivacyLedgerEffectKindV1.ZK_ACE_TRANSPARENT_TRANSFER,
        transactionHashHex = hash(0x21),
        actionIndex = 0,
        transactionIntentDigest = intent,
        statementDigest = repeated(0x24),
        proofEnvelopeHash = repeated(0x26),
        queryAuthorityAccountId = "wallet-query-authority",
        transactionAuthorityAccountId = "exact12-transaction-authority",
        blockHashHex = hash(0x31),
        resultHashHex = hash(0x41),
        rejectionCode = code,
        rejectionMessage = "Exact12 validation rejected the action",
        committedBlockHeight = 9,
        finalizedCheckpoint = AuthenticatedFinalityCheckpointV1(
            checkpointHeight,
            repeated(0x11),
        ),
        executedBlockWireHashHex = hash(0x51),
        evidenceIdHex = hash(0x61),
        transactionDetailsHashHex = hash(0x71),
        finalityPageHashHex = hash(0x81),
    )

    private fun repeated(value: Int): ByteArray = ByteArray(32) { value.toByte() }

    private fun hash(value: Int): String {
        val digit = Character.forDigit((value or 1) and 0x0f, 16)
        return digit.toString().repeat(64)
    }
}
