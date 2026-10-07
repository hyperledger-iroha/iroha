// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.junit.jupiter.api.Test

class KagemushaWalletLedgerV1Test {
    @Test fun `Load history progress is receipt bound and cannot advance beyond its height`() {
        val receipt = byteArrayOf(1)
        KagemushaWalletSetupInputV1(31, first = receipt)
        KagemushaWalletSetupInputV1(32, first = receipt, second = byteArrayOf(2))
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(31) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(31, first = ByteArray(513)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(32, first = receipt) }
        val bytes = ByteArray(8).also { it[7] = 2 }
        val result = KagemushaWalletCallV1(43, -1, 0, 3, 0, 0, bytes)
        val progress = KagemushaWalletLoadProofProgressV1(result)
        assertEquals(3L, progress.receiptHeightBits); assertEquals(2L, progress.verifiedHeightBits)
        assertFailsWith<KagemushaWalletExceptionV1> { result.completion() }
        bytes[7] = 4
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletLoadProofProgressV1(KagemushaWalletCallV1(43, -1, 0, 3, 0, 0, bytes)) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(43, -1, 0, 1, 0, 0, ByteArray(8)) }
    }

    @Test fun `Unload confirmation binds exact transaction hash and positive native inclusion`() {
        val hash = ByteArray(32) { 6 }; val original = ByteArray(65_536) { 5 }
        val input = KagemushaWalletSetupInputV1(30, identity = hash, first = original)
        hash.fill(0); original.fill(0)
        assertContentEquals(ByteArray(32) { 6 }, input.identity())
        assertContentEquals(ByteArray(65_536) { 5 }, input.first())
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(30, first = original) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(30, identity = input.identity()) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(30, identity = input.identity(), first = ByteArray(65_537)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(30, identity = input.identity(), first = original, token = 1) }
        val result = KagemushaWalletCallV1(42, -1, 0, -1, 0, 0, ByteArray(32) { 7 })
        assertEquals(-1L, KagemushaWalletUnloadConfirmationV1(result).heightBits)
        assertFailsWith<KagemushaWalletExceptionV1> { result.completion() }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(42, -1, 0, 0, 0, 0, ByteArray(32) { 1 }) }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(42, -1, 0, 1, 0, 0, ByteArray(32)) }
    }

    @Test fun `Load preparation binds only nonzero identity and unsigned amount`() {
        val request = ByteArray(32) { 7 }
        val input = KagemushaWalletSetupInputV1(27, identity = request, amount = KagemushaWalletUInt128V1(-1, -1))
        request.fill(0)
        assertContentEquals(ByteArray(32) { 7 }, input.identity())
        assertEquals(KagemushaWalletUInt128V1(-1, -1), input.amount)
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(27, identity = request, amount = input.amount) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(27, identity = input.identity()) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(27, identity = input.identity(), amount = input.amount, first = byteArrayOf(1)) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(27, identity = input.identity(), amount = input.amount, token = 1) }
    }

    @Test fun `Load finality intake retains both exact bounded originals without authority fields`() {
        val receipt = ByteArray(512) { 2 }; val proof = ByteArray(8192) { 3 }
        val input = KagemushaWalletSetupInputV1(28, first = receipt, second = proof)
        receipt.fill(0); proof.fill(0)
        assertContentEquals(ByteArray(512) { 2 }, input.first())
        assertContentEquals(ByteArray(8192) { 3 }, input.second())
        for ((a, b) in listOf(byteArrayOf() to proof, receipt to byteArrayOf(), ByteArray(513) to proof, receipt to ByteArray(8193))) {
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(28, first = a, second = b) }
        }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(28, identity = ByteArray(32) { 1 }, first = receipt, second = proof) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(28, token = 1, first = receipt, second = proof) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(28, amount = KagemushaWalletUInt128V1(1, 0), first = receipt, second = proof) }
    }

    @Test fun `ledger transport selects a closed purpose and cannot carry proof or time authority`() {
        for (kind in KagemushaWalletLedgerTransportV1.values()) {
            val original = ByteArray(65_536) { 4 }
            val input = KagemushaWalletSetupInputV1(29, token = kind.tag, first = original)
            original.fill(0)
            assertContentEquals(ByteArray(65_536) { 4 }, input.first())
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(29, token = kind.tag) }
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(29, token = kind.tag, first = ByteArray(65_537)) }
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(29, token = kind.tag, first = original, second = byteArrayOf(1)) }
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(29, token = kind.tag, first = original, identity = ByteArray(32) { 1 }) }
        }
        for (tag in listOf(-1L, 0L, 4L, Long.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException> { KagemushaWalletSetupInputV1(29, token = tag, first = byteArrayOf(1)) }
        }
    }

    @Test fun `instruction and recursive proof are bounded originals never monetary completion`() {
        for ((status, bound) in listOf(40 to 65_536, 41 to 16_384)) {
            val original = ByteArray(bound) { 8 }
            val result = KagemushaWalletCallV1(status, -1, 0, 0, 0, 0, original)
            original.fill(0)
            assertContentEquals(ByteArray(bound) { 8 }, result.bytes())
            assertFailsWith<KagemushaWalletExceptionV1> { result.completion() }
            assertFailsWith<KagemushaWalletExceptionV1> { result.original() }
            for (size in listOf(0, bound + 1)) assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(status, -1, 0, 0, 0, 0, ByteArray(size)) }
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(status, -1, 0, 1, 0, 0, byteArrayOf(1)) }
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(status, -1, 0, 0, 1, 0, byteArrayOf(1)) }
            assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(status, -1, 0, 0, 0, 1, byteArrayOf(1)) }
        }
        assertFailsWith<KagemushaWalletExceptionV1> { KagemushaWalletCallV1(44, -1, 0, 0, 0, 0, byteArrayOf()) }
    }
}
