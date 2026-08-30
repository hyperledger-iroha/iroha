// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.ByteBuffer
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver

class AuthenticatedTransactionDetailsAuthoritySplitV2Test {
    @Test
    fun publicV2ModelsPreserveDistinctQueryAndTransactionAuthorities() {
        val rejection = AuthenticatedCommittedRejectionV2(
            transactionHashHex = "11".repeat(32),
            queryAuthorityAccountId = "wallet-query-authority",
            transactionAuthorityAccountId = "issuer-transaction-authority",
            blockHashHex = "22".repeat(32),
            resultHashHex = "33".repeat(32),
            rejectionCode = "validation",
            rejectionMessage = "request rejected",
            committedBlockHeight = 9,
        )
        assertEquals("wallet-query-authority", rejection.queryAuthorityAccountId)
        assertEquals("issuer-transaction-authority", rejection.transactionAuthorityAccountId)

        val result = AuthenticatedCommittedTransactionResultV2(
            transactionHashHex = "11".repeat(32),
            queryAuthorityAccountId = "wallet-query-authority",
            transactionAuthorityAccountId = "issuer-transaction-authority",
            blockHashHex = "22".repeat(32),
            resultHashHex = "33".repeat(32),
            resultOk = true,
            rejectionMessage = null,
            committedBlockHeight = BigInteger.valueOf(9),
        )
        assertEquals("wallet-query-authority", result.queryAuthorityAccountId)
        assertEquals("issuer-transaction-authority", result.transactionAuthorityAccountId)
    }

    @Test
    fun V2InventoryKeepsGenericAndKagemushaBoundProjectorsExplicit() {
        val bridgeMethods = AuthenticatedTransactionDetailsNativeBridge::class.java.methods
            .associateBy { it.name }
        assertEquals(
            5,
            bridgeMethods.getValue("buildSignedTransactionDetailsQueryV2").parameterCount,
        )
        assertEquals(
            5,
            bridgeMethods.getValue("projectKagemushaCommittedRejectionV2").parameterCount,
        )
        assertEquals(
            2,
            bridgeMethods.getValue("projectCommittedTransactionResultV2").parameterCount,
        )

        val transportNames = HttpClientTransport::class.java.methods.map { it.name }.toSet()
        assertTrue("getAuthenticatedCommittedRejectionV2" in transportNames)
        assertTrue("getAuthenticatedKagemushaCommittedRejectionV2" in transportNames)
        assertTrue("getAuthenticatedCommittedTransactionResultV2" in transportNames)
        assertTrue("getBridgeFinalityProofV1" in transportNames)
        assertTrue("getAuthenticatedTransactionDetailsCarrierV2" in transportNames)

        val finalizedBridgeNames = AuthenticatedTransactionDetailsNativeBridge::class.java.methods
            .map { it.name }
            .toSet()
        assertTrue("bindFinalityProofPageV1" in finalizedBridgeNames)
        assertTrue("verifyFinalityPageV1" in finalizedBridgeNames)
        assertTrue("projectFinalizedKagemushaOutcomeV1" in finalizedBridgeNames)
        assertTrue("requireKagemushaTopUpFinalityAgreementV1" in finalizedBridgeNames)

        val proverNames = KagemushaRecursiveSpendProver::class.java.methods.map { it.name }.toSet()
        assertTrue("decodeTopUpFinalityProof" in proverNames)
        assertTrue("projectVerifiedTopUpFinalityV4" in proverNames)
    }

    @Test
    fun finalizedCheckpointProjectionIsExactlyFortyBytesAndDefensive() {
        val sourceContext = repeated(0x11)
        val checkpoint = AuthenticatedFinalityCheckpointV1(9, sourceContext)
        sourceContext[0] = 0x22.toByte()

        val projection = checkpoint.projectionBytes()
        assertEquals(AuthenticatedFinalityCheckpointV1.PROJECTION_BYTES, projection.size)
        assertEquals(40, projection.size)
        assertEquals(9, ByteBuffer.wrap(projection).long)
        assertEquals(0x11, checkpoint.heightContextId()[0].toInt())
        assertContentEquals(repeated(0x11), projection.copyOfRange(8, projection.size))

        projection[8] = 0x22.toByte()
        assertEquals(0x11, checkpoint.projectionBytes()[8].toInt())
    }

    @Test
    fun finalizedContentAddressesRequireTheIrohaHashMarker() {
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedFinalityProofPageV1(byteArrayOf(0x01), "22".repeat(32))
        }
        AuthenticatedFinalityProofPageV1(byteArrayOf(0x01), "23".repeat(32))
    }

    @Test
    fun routingHintsMustAgreeWithAuthenticatedTerminalResult() {
        val operationId = repeated(0x21)
        val applied = outcome(
            "top_up",
            AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED,
            operationId,
        )
        AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
            9,
            true,
            applied,
        )
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
                8,
                true,
                applied,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
                9,
                false,
                applied,
            )
        }

        val rejected = outcome(
            "top_up",
            AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED,
            operationId,
        )
        AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
            9,
            false,
            rejected,
        )
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
                9,
                true,
                rejected,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            outcome(
                "top_up",
                AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED,
                operationId,
                rejectionCode = "server_error",
            )
        }
    }

    @Test
    fun specializedTopUpAgreementRejectsKindAndOperationSubstitution() {
        val operationId = repeated(0x21)
        val applied = outcome(
            "top_up",
            AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED,
            operationId,
        )
        val specialized = verifiedTopUp(operationId)
        AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
            applied,
            specialized,
        )

        val anotherOperation = operationId.copyOf().also { it[0] = 0x20.toByte() }
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                applied,
                verifiedTopUp(anotherOperation),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                applied,
                verifiedTopUp(operationId, transactionHashHex = hash(0x12)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                applied,
                verifiedTopUp(operationId, height = 8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                applied,
                verifiedTopUp(operationId, blockHashHex = hash(0x23)),
            )
        }
        val anotherContext = repeated(0x11).also { it[0] = 0x10.toByte() }
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                applied,
                verifiedTopUp(operationId, heightContextId = anotherContext),
            )
        }

        val redeem = outcome(
            "redeem",
            AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED,
            operationId,
        )
        assertFailsWith<IllegalArgumentException> {
            AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                redeem,
                specialized,
            )
        }
    }

    private fun outcome(
        kind: String,
        state: AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState,
        operationId: ByteArray,
        rejectionCode: String = "validation",
    ): AuthenticatedFinalizedKagemushaOutcomeV1 {
        val rejected = state == AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED
        return AuthenticatedFinalizedKagemushaOutcomeV1(
            state,
            operationId,
            kind,
            hash(0x11),
            "wallet-query-authority",
            "issuer-transaction-authority",
            hash(0x22),
            hash(0x33),
            9,
            AuthenticatedFinalityCheckpointV1(9, repeated(0x11)),
            hash(0x44),
            if (rejected) rejectionCode else null,
            if (rejected) "request rejected" else null,
            hash(0x55),
            hash(0x66),
            hash(0x77),
        )
    }

    private fun verifiedTopUp(
        operationId: ByteArray,
        transactionHashHex: String = hash(0x11),
        height: Long = 9,
        blockHashHex: String = hash(0x22),
        heightContextId: ByteArray = repeated(0x11),
    ) =
        KagemushaRecursiveSpendProver.VerifiedTopUpFinalityV4(
            operationId,
            transactionHashHex,
            height,
            blockHashHex,
            heightContextId,
        )

    private fun repeated(value: Int): ByteArray = ByteArray(32) { value.toByte() }

    private fun hash(value: Int): String {
        val digit = Character.forDigit(value and 0x0f, 16)
        val markedDigit = Character.forDigit((value or 1) and 0x0f, 16)
        return digit.toString().repeat(63) + markedDigit
    }
}
