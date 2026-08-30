// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class AuthenticatedCommittedTransactionResultV1Test {
    @Test
    fun successAndRejectionRemainMutuallyExclusive() {
        val success = fixture(resultOk = true, rejectionMessage = null)
        assertTrue(success.resultOk)
        assertNull(success.rejectionMessage)

        val rejection = fixture(resultOk = false, rejectionMessage = "policy epoch is stale")
        assertEquals("policy epoch is stale", rejection.rejectionMessage)

        assertFailsWith<IllegalArgumentException> {
            fixture(resultOk = true, rejectionMessage = "contradiction")
        }
        assertFailsWith<IllegalArgumentException> {
            fixture(resultOk = false, rejectionMessage = null)
        }
    }

    @Test
    fun hashesAndUnsignedCommittedHeightFailClosed() {
        assertFailsWith<IllegalArgumentException> {
            fixture(transactionHashHex = "AB".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            fixture(committedBlockHeight = BigInteger.ZERO)
        }
        assertFailsWith<IllegalArgumentException> {
            fixture(committedBlockHeight = BigInteger.ONE.shiftLeft(64))
        }
        assertFailsWith<IllegalArgumentException> {
            fixture(resultOk = false, rejectionMessage = " padded ")
        }
        assertFailsWith<IllegalArgumentException> {
            fixture(resultOk = false, rejectionMessage = "policy\u0001rejected")
        }
        assertFailsWith<IllegalArgumentException> {
            fixture(resultOk = false, rejectionMessage = "é".repeat(513))
        }
    }

    private fun fixture(
        transactionHashHex: String = "ab".repeat(32),
        resultOk: Boolean = true,
        rejectionMessage: String? = null,
        committedBlockHeight: BigInteger = BigInteger.valueOf(7),
    ): AuthenticatedCommittedTransactionResultV1 = AuthenticatedCommittedTransactionResultV1(
        transactionHashHex = transactionHashHex,
        transactionAuthorityAccountId = "canonical-authority",
        blockHashHex = "cd".repeat(32),
        resultHashHex = "ef".repeat(32),
        resultOk = resultOk,
        rejectionMessage = rejectionMessage,
        committedBlockHeight = committedBlockHeight,
    )
}
