// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.IOException
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue

/** Managed private-original custody only; no Native signing, filesystem or device qualification. */
class KagemushaProbeReviewCustodyTest {
    private val identity = ByteArray(32) { 1 }
    private val digest = "72cd6e8422c407fb6d098690f1130b7ded7ec2f7f5e1d30bd9d521f015363793"
    private val root = "private/intents/executed-$digest"
    private val directory = "private/intents/review-originals-$digest"
    private class MemoryIo : KagemushaProbeReviewCustodyIo {
        val files = linkedMapOf<String, ByteArray>()
        val writes = mutableListOf<String>()
        var readFailure: Pair<String, IOException>? = null
        var publicationFailure: Pair<String, Boolean>? = null
        override fun exists(path: String) = path in files
        override fun read(path: String, maximum: Int): ByteArray {
            readFailure?.let { (selected, failure) -> if (selected == path) throw failure }
            val bytes = files[path] ?: throw EnrollmentProbeFailure("absent")
            probeRequire(bytes.size <= maximum)
            return bytes.copyOf()
        }
        override fun mkdir(path: String) { writes.add("mkdir:$path") }
        override fun publish(path: String, bytes: ByteArray) {
            writes.add("publish:$path")
            if (publicationFailure == (path to false)) throw IOException("publication interrupted")
            val previous = files[path]
            probeRequire(previous == null || previous.contentEquals(bytes), "collision")
            if (previous == null) files[path] = bytes.copyOf()
            if (publicationFailure == (path to true)) throw IOException("publication acknowledgement lost")
        }
    }
    private fun originals(family: String, charged: Boolean = false): Map<String, ByteArray> =
        if (family == "review-send") mapOf("request" to byteArrayOf(1, 2), "destination" to byteArrayOf(3))
        else mapOf("amount" to ByteArray(16).also { it[0] = 1 }) + if (charged)
            mapOf("quote" to byteArrayOf(4), "certificates" to byteArrayOf(5), "beneficiary" to byteArrayOf(6))
            else emptyMap()
    private fun projection(family: String): ByteArray {
        val send = family == "review-send"
        return ByteArray(if (send) 496 else 495).also { value ->
            byteArrayOf(75, 87, 79, 82, 86, 49, 0, 0).copyInto(value)
            value[8] = if (send) 1 else 8
            value[9] = 1; value[41] = 1; value[57] = 1
            for (at in listOf(106, 202, 234, 266, 298, 330, 362, 394)) value[at] = 1
            value[426] = 4; value[427] = 1
            if (send) { value[73] = 1; value[74] = 1; value[138] = 1; value[491] = 1; value[495] = 7 }
        }
    }
    private fun expect(io: MemoryIo, family: String, originals: Map<String, ByteArray>, continuation: Boolean) =
        KagemushaProbeReviewCustody.expectation(io, identity, family, originals, continuation)
    private fun prepared(family: String, originals: Map<String, ByteArray>): MemoryIo = MemoryIo().also {
        assertSame(KagemushaWalletReviewExpectationV1.Fresh, expect(it, family, originals, false))
        it.files[root] = projection(family)
        it.writes.clear()
    }
    private fun assertOriginalsUnchanged(before: Map<String, ByteArray>, after: Map<String, ByteArray>) {
        assertEquals(before.keys, after.keys)
        for ((path, bytes) in before) assertContentEquals(bytes, after.getValue(path))
    }

    @Test fun newInvocationReadsExactSendAndBothUnloadBundlesWithoutWriting() {
        for ((family, charged) in listOf("review-send" to false, "review-unload" to false, "review-unload" to true)) {
            val input = originals(family, charged)
            val first = prepared(family, input)
            val restarted = MemoryIo().also { second -> first.files.forEach { (path, bytes) -> second.files[path] = bytes.copyOf() } }
            val value = expect(restarted, family, input, true)
            assertTrue(value is KagemushaWalletReviewExpectationV1.OriginalUnstarted)
            assertContentEquals(projection(family), value.bytes())
            assertContentEquals(identity, restarted.files["$directory/request-id"])
            assertContentEquals(family.toByteArray(), restarted.files["$directory/family"])
            for ((name, bytes) in input) assertContentEquals(bytes, restarted.files["$directory/$name"])
            assertTrue(restarted.writes.isEmpty())
        }
    }

    @Test fun missingAnyRetainedOriginalOrRootCannotBeRecreatedFromIdenticalNewInputs() {
        for ((family, charged) in listOf("review-send" to false, "review-unload" to false, "review-unload" to true)) {
            val input = originals(family, charged)
            val paths = prepared(family, input).files.keys.toList()
            for (missing in paths) {
                val io = prepared(family, input); io.files.remove(missing)
                val before = io.files.mapValues { (_, bytes) -> bytes.copyOf() }
                assertFails { expect(io, family, input, true) }
                assertTrue(io.writes.isEmpty()); assertFalse(missing in io.files)
                assertOriginalsUnchanged(before, io.files)
            }
        }
    }

    @Test fun changedCompanionsNamesFamilyAndIdentityRefuseWithoutTouchingOldCustody() {
        for ((family, charged) in listOf("review-send" to false, "review-unload" to true)) {
            val input = originals(family, charged); val io = prepared(family, input)
            val before = io.files.mapValues { (_, bytes) -> bytes.copyOf() }
            for ((name, bytes) in input) {
                val changed = input + (name to bytes.copyOf().also { it[0] = 99 })
                assertFails { expect(io, family, changed, true) }
            }
            io.files["$directory/names"] = "unexpected".toByteArray()
            assertFails { expect(io, family, input, true) }
            io.files["$directory/names"] = before.getValue("$directory/names")
            io.files["$directory/family"] = "changed".toByteArray()
            assertFails { expect(io, family, input, true) }
            io.files["$directory/family"] = before.getValue("$directory/family")
            io.files["$directory/request-id"] = ByteArray(32) { 9 }
            assertFails { expect(io, family, input, true) }
            io.files["$directory/request-id"] = before.getValue("$directory/request-id")
            assertFails { KagemushaProbeReviewCustody.expectation(io, ByteArray(32) { 2 }, family, input, true) }
            assertTrue(io.writes.isEmpty()); assertOriginalsUnchanged(before, io.files)
        }
    }

    @Test fun malformedHashOnlyOversizedTrailingAndWrongKindRootsNeverGrantContinuation() {
        val family = "review-send"; val input = originals(family); val frame = projection(family)
        for (invalid in listOf(byteArrayOf(), ByteArray(64) { 65 }, frame.copyOf(frame.size - 1),
            frame + byteArrayOf(0), ByteArray(4_592), projection("review-unload"))) {
            val io = prepared(family, input); io.files[root] = invalid.copyOf()
            assertFails { expect(io, family, input, true) }
            assertTrue(io.writes.isEmpty()); assertContentEquals(invalid, io.files[root])
        }
    }

    @Test fun uncertainFreshPublicationOnlyResumesIdenticalBytesBeforeAnyRootExists() {
        val family = "review-unload"; val input = originals(family, true)
        val targets = prepared(family, input).files.keys.filter { it != root }
        for (target in targets) for (after in listOf(false, true)) {
            val io = MemoryIo(); io.publicationFailure = target to after
            assertFailsWith<IOException> { expect(io, family, input, false) }
            val retained = io.files.mapValues { (_, bytes) -> bytes.copyOf() }
            io.publicationFailure = null
            assertSame(KagemushaWalletReviewExpectationV1.Fresh, expect(io, family, input, false))
            for ((path, bytes) in retained) assertContentEquals(bytes, io.files[path])
            io.files[root] = projection(family); io.writes.clear()
            assertFails { expect(io, family, input, false) }
            io.files.remove("$directory/certificates")
            assertFails { expect(io, family, input, true) }
            assertTrue(io.writes.isEmpty()); assertFalse("$directory/certificates" in io.files)
        }
        val io = MemoryIo()
        expect(io, family, input, false)
        val before = io.files.mapValues { (_, bytes) -> bytes.copyOf() }
        assertFails { expect(io, family, input + ("amount" to ByteArray(16) { 2 }), false) }
        assertOriginalsUnchanged(before, io.files)
    }

    @Test fun readFailurePropagatesAndCopiesPreventInputOrProjectionMutation() {
        val family = "review-send"; val input = originals(family); val io = prepared(family, input)
        val failure = IOException("unavailable private original")
        io.readFailure = "$directory/request" to failure
        assertSame(failure, assertFailsWith<IOException> { expect(io, family, input, true) })
        assertTrue(io.writes.isEmpty())
        io.readFailure = null
        val value = expect(io, family, input, true)
        value.bytes().fill(9); input.getValue("request").fill(8)
        assertContentEquals(projection(family), value.bytes())
        assertContentEquals(byteArrayOf(1, 2), io.files["$directory/request"])
        assertContentEquals(projection(family), io.files[root])
    }

    @Test fun onlyClosedOriginalBundlesAndNonzeroExactIdentifiersReachTheFilesystem() {
        val io = MemoryIo(); val send = originals("review-send"); val unload = originals("review-unload", true)
        for (invalid in listOf(send - "destination", send + ("../request" to byteArrayOf(1)),
            send + ("family" to byteArrayOf(1)), send + ("request" to byteArrayOf()),
            send + ("request" to ByteArray(10_035)), send + ("destination" to ByteArray(4_097)))) {
            assertFails { expect(io, "review-send", invalid, false) }
        }
        for (name in listOf("quote", "certificates", "beneficiary")) {
            assertFails { expect(io, "review-unload", unload - name, false) }
        }
        assertFails { expect(io, "receive", send, false) }
        assertFails { expect(io, "review-unload", mapOf("amount" to ByteArray(15)), false) }
        for (id in listOf(ByteArray(32), ByteArray(31) { 1 }, ByteArray(33) { 1 })) {
            assertFails { KagemushaProbeReviewCustody.expectation(io, id, "review-send", send, false) }
        }
        assertTrue(io.writes.isEmpty()); assertTrue(io.files.isEmpty())
    }
}
