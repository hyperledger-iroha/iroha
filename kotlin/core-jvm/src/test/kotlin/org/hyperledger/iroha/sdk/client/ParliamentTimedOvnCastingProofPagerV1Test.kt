package org.hyperledger.iroha.sdk.client

import java.io.File
import java.math.BigInteger
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

class ParliamentTimedOvnCastingProofPagerV1Test {
    @Test
    fun `complete signed checkpoints survive promotion and asynchronous persistence`() {
        val first = checkpoint("genesis-checkpoint.nrt")
        val second = checkpoint("height-2-checkpoint.nrt")
        assertTrue(first.size > 32 && second.size > 32)
        val pending = CompletableFuture<Void>()
        var fetched = 0
        val persisted = mutableListOf<ParliamentTimedOvnCastingProofPageVerificationV1>()
        val operation = ParliamentTimedOvnCastingProofPagerV1.synchronize(
            BigInteger.ONE,
            first,
            { fetched++; CompletableFuture.completedFuture(response()) },
            { _, height, checkpoint ->
                assertContentEquals(if (height == BigInteger.ONE) first else second, checkpoint)
                checkpoint.fill(0)
                ParliamentTimedOvnCastingProofPageVerificationV1(
                    BigInteger.valueOf(2), ByteArray(32) { 1 }, height == BigInteger.ONE, second,
                )
            },
            { verification ->
                persisted.add(verification)
                if (persisted.size == 1) pending else CompletableFuture.completedFuture(null)
            },
        )
        assertEquals(1, fetched)
        assertTrue(!operation.isDone)
        pending.complete(null)
        val terminal = operation.join()
        assertEquals(2, fetched)
        assertEquals(2, terminal.verifiedPageCount)
        assertContentEquals(second, terminal.verificationAnchorCheckpointNorito())
        assertContentEquals(second, persisted[0].promotedCheckpointNorito())
        terminal.verificationAnchorCheckpointNorito().fill(0)
        persisted[0].promotedCheckpointNorito().fill(0)
        assertContentEquals(second, terminal.verificationAnchorCheckpointNorito())
        assertContentEquals(second, persisted[0].promotedCheckpointNorito())
    }

    @Test
    fun `invalid height and empty checkpoint fail before fetch`() {
        var fetched = 0
        val fetcher = ParliamentTimedOvnCastingProofPageFetcherV1 {
            fetched++; CompletableFuture.completedFuture(response())
        }
        val verifier = ParliamentTimedOvnCastingProofPageVerifierV1 { _, _, _ ->
            error("invalid input must not reach verification")
        }
        val persister = ParliamentTimedOvnCastingCheckpointPersisterV1 {
            error("invalid input must not reach persistence")
        }
        for (height in listOf(BigInteger.ZERO, BigInteger.valueOf(-1), BigInteger.ONE.shiftLeft(64))) {
            assertFailsWith<IllegalArgumentException> {
                ParliamentTimedOvnCastingProofPagerV1.synchronize(height, byteArrayOf(1), fetcher, verifier, persister)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            ParliamentTimedOvnCastingProofPagerV1.synchronize(BigInteger.ONE, byteArrayOf(), fetcher, verifier, persister)
        }
        assertFailsWith<IllegalArgumentException> {
            ParliamentTimedOvnCastingProofPagerV1.synchronize(
                BigInteger.ONE, ByteArray(68 * 1024 * 1024 + 1), fetcher, verifier, persister,
            )
        }
        assertEquals(0, fetched)
    }

    @Test
    fun `regression excessive page advance and nonterminal zero advance never persist`() {
        for (height in listOf(6L, 71L, 7L)) {
            var persisted = 0
            val operation = ParliamentTimedOvnCastingProofPagerV1.synchronize(
                BigInteger.valueOf(7), byteArrayOf(1),
                { CompletableFuture.completedFuture(response()) },
                { _, _, _ ->
                    ParliamentTimedOvnCastingProofPageVerificationV1(
                        BigInteger.valueOf(height), ByteArray(32) { 1 }, true, byteArrayOf(2),
                    )
                },
                { persisted++; CompletableFuture.completedFuture(null) },
            )
            val failure = assertFailsWith<CompletionException> { operation.join() }
            assertTrue(failure.cause is IllegalArgumentException)
            assertEquals(0, persisted)
        }
    }

    @Test
    fun `failed persistence prevents another request`() {
        var fetched = 0
        val failure = IllegalStateException("durable checkpoint write refused")
        val operation = ParliamentTimedOvnCastingProofPagerV1.synchronize(
            BigInteger.ONE, byteArrayOf(1),
            { fetched++; CompletableFuture.completedFuture(response()) },
            { _, _, _ ->
                ParliamentTimedOvnCastingProofPageVerificationV1(
                    BigInteger.valueOf(2), ByteArray(32) { 1 }, true, byteArrayOf(2),
                )
            },
            { CompletableFuture<Void>().also { it.completeExceptionally(failure) } },
        )
        assertEquals(failure, assertFailsWith<CompletionException> { operation.join() }.cause)
        assertEquals(1, fetched)
    }

    @Test
    fun `maximum pages stop before an extra fetch or verification`() {
        var fetched = 0
        var verified = 0
        var persisted = 0
        val operation = ParliamentTimedOvnCastingProofPagerV1.synchronize(
            BigInteger.ONE, byteArrayOf(1),
            { fetched++; CompletableFuture.completedFuture(response()) },
            { _, height, _ ->
                verified++
                ParliamentTimedOvnCastingProofPageVerificationV1(
                    height.add(BigInteger.valueOf(63)), ByteArray(32) { 1 }, true, byteArrayOf(2),
                )
            },
            { persisted++; CompletableFuture.completedFuture(null) },
        )
        val cause = assertFailsWith<CompletionException> { operation.join() }.cause
        assertTrue(cause is IllegalStateException)
        assertEquals("Parliament casting-proof page limit was reached", cause.message)
        assertEquals(64, fetched)
        assertEquals(64, verified)
        assertEquals(64, persisted)
    }

    private fun response() = ParliamentTimedOvnCastingProofResponseV1(byteArrayOf(1), byteArrayOf(2))

    private fun checkpoint(name: String): ByteArray =
        generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, "fixtures/sumeragi/native-finality/$name") }
            .first(File::isFile).readBytes()
}
