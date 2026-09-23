// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.StandardOpenOption
import java.nio.ByteBuffer
import java.nio.channels.FileChannel
import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test

private fun fakeSec1(byte: Byte): ByteArray = byteArrayOf(0x04) + ByteArray(64) { byte }

private class TestJournalIo(private val onSync: () -> Unit) : SelectionJournalIoV1 {
    companion object { private val processLocks = java.util.concurrent.ConcurrentHashMap<String, Any>() }
    var tearEvidenceWrite = false
    override fun exists(file: java.io.File): Boolean {
        val path = file.toPath()
        require(!Files.isSymbolicLink(path)) { "symbolic journal entry" }
        return Files.exists(path, LinkOption.NOFOLLOW_LINKS)
    }

    override fun read(file: java.io.File, maximum: Int): ByteArray {
        val path = file.toPath()
        require(Files.isRegularFile(path, LinkOption.NOFOLLOW_LINKS))
        return FileChannel.open(path, setOf(StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS))
            .use { channel ->
                require(channel.size() in 1L..maximum.toLong())
                val buffer = ByteBuffer.allocate(channel.size().toInt())
                while (buffer.hasRemaining()) check(channel.read(buffer) > 0)
                buffer.array()
            }
    }

    override fun writeNew(file: java.io.File, bytes: ByteArray) {
        val torn = tearEvidenceWrite && file.name.endsWith(".evidence")
        FileChannel.open(file.toPath(), setOf(
            StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS,
        )).use { channel ->
            val buffer = ByteBuffer.wrap(if (torn) byteArrayOf(0) else bytes)
            while (buffer.hasRemaining()) channel.write(buffer)
            channel.force(true)
        }
        onSync()
        if (torn) throw IllegalStateException("evidence write torn after signing")
    }

    override fun <T> withLock(file: java.io.File, action: () -> T): T =
        synchronized(processLocks.computeIfAbsent(file.absolutePath) { Any() }) {
            FileChannel.open(file.toPath(), setOf(
                StandardOpenOption.CREATE, StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS,
            )).use { channel -> channel.lock().use { action() } }
        }
}

class AndroidKeyMintOneUseSelectionCandidateV1Test {
    @Test fun collectedEvidenceCopiesEveryVerifierInputByte() {
        val nonce = ByteArray(32) { 6 }
        val challenge = keyMintPreparedChallengeV1(nonce, lane, before, after)
        val evidence = KeyMintOneUseSelectionResultV1.Evidence(
            frame(), lane, before, after, nonce, challenge, fakeSec1(1),
            listOf(byteArrayOf(2, 3)), byteArrayOf(0x30, 0x00), false, null,
        )
        val raw = evidence.toRawAttestationEvidenceV1()
        assertContentEquals(evidence.canonicalSelectionFrame(), raw.canonicalSelectionFrame())
        assertContentEquals(lane, raw.laneCommitment())
        assertContentEquals(before, raw.secureIndexBeforeLittleEndian())
        assertContentEquals(after, raw.secureIndexAfterLittleEndian())
        assertContentEquals(nonce, raw.attestationNonce())
        assertContentEquals(challenge, raw.attestationChallenge())
        assertContentEquals(fakeSec1(1), raw.publicKeySec1())
        assertContentEquals(evidence.certificateChain()[0], raw.certificateChainDer()[0])
        assertContentEquals(evidence.signatureDer(), raw.signatureDer())
        raw.certificateChainDer()[0][0] = 0
        assertContentEquals(byteArrayOf(2, 3), raw.certificateChainDer()[0])
    }

    private class FakeDevice : SingleUseProbeDeviceV1 {
        override var apiLevel = 31
        var feature = true
        var signFails = false
        var generateFailsAfterCreation = false
        var generateCalls = 0
        var signCalls = 0
        var deleteCalls = 0
        var readCalls = 0
        var active = false
        var selectedPublicKey = fakeSec1(1)
        var onGenerate: (() -> Unit)? = null
        var onSign: (() -> Unit)? = null
        var signedBytes = byteArrayOf()
        var attestationChallenge = byteArrayOf()
        override fun hasHardwareSingleUseFeature() = feature
        override fun newChallenge() = ByteArray(32) { 7 }
        override fun hasAlias(alias: String) = active
        override fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1 {
            check(!active) { "one-use alias already exists" }
            onGenerate?.invoke()
            generateCalls += 1
            active = true
            if (generateFailsAfterCreation) throw IllegalStateException("key generated, response lost")
            attestationChallenge = challenge.copyOf()
            return ProbeKeyMaterialV1(selectedPublicKey.copyOf(), listOf(byteArrayOf(3, 4)))
        }
        override fun read(alias: String): ProbeKeyMaterialV1 {
            readCalls += 1
            check(active)
            return ProbeKeyMaterialV1(selectedPublicKey.copyOf(), listOf(byteArrayOf(3, 4)))
        }
        override fun sign(alias: String, message: ByteArray): ByteArray {
            onSign?.invoke()
            signCalls += 1
            signedBytes = message.copyOf()
            if (signFails) throw IllegalStateException("possibly consumed")
            return byteArrayOf(0x30, 0x02, 0x01, 0x01)
        }
        override fun delete(alias: String) { deleteCalls += 1; active = false }
    }

    private class FakeStore : SelectionIntentStoreV1 {
        override fun <T> withSlotLock(slot: String, action: () -> T): T = synchronized(this) {
            action()
        }
        val events = mutableListOf<String>()
        var preparing = false
        var prepared: KeyMintOneUsePreparationResultV1.Prepared? = null
        var reserved = false
        var persisted: KeyMintOneUseSelectionResultV1.Evidence? = null
        var failPersist = false
        override fun lookupPrepared(
            slot: String,
            lane: ByteArray,
            before: ByteArray,
            after: ByteArray,
        ): PreparationLookupV1 {
            events.add("lookupPrepared")
            return when {
                prepared != null -> PreparationLookupV1.Ready(checkNotNull(prepared))
                preparing -> PreparationLookupV1.Frozen
                else -> PreparationLookupV1.Empty
            }
        }
        override fun beginPreparation(
            slot: String,
            lane: ByteArray,
            before: ByteArray,
            after: ByteArray,
            nonce: ByteArray,
            challenge: ByteArray,
        ) {
            events.add("beginPreparation")
            check(!preparing)
            preparing = true
        }
        override fun persistPrepared(
            slot: String,
            preparation: KeyMintOneUsePreparationResultV1.Prepared,
        ) {
            events.add("persistPrepared")
            prepared = preparation
        }
        override fun lookup(
            slot: String,
            frameDigest: ByteArray,
            lane: ByteArray,
            before: ByteArray,
            after: ByteArray,
        ): SelectionLookupV1 {
            events.add("lookup")
            return when {
                persisted != null -> SelectionLookupV1.Recovered(checkNotNull(persisted).recovered())
                reserved -> SelectionLookupV1.Frozen
                else -> SelectionLookupV1.Empty
            }
        }
        override fun reserve(
            slot: String,
            frameDigest: ByteArray,
            lane: ByteArray,
            before: ByteArray,
            after: ByteArray,
            challenge: ByteArray,
        ) {
            events.add("reserve")
            check(!reserved)
            reserved = true
        }
        override fun persist(
            slot: String,
            frameDigest: ByteArray,
            evidence: KeyMintOneUseSelectionResultV1.Evidence,
        ) {
            events.add("persist")
            if (failPersist) throw IllegalStateException("disk full after signature")
            persisted = evidence
        }
    }

    private fun frame(body: ByteArray = byteArrayOf(1, 2, 3)): ByteArray {
        val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000"
            .toByteArray(Charsets.US_ASCII)
        return domain + ByteArray(8) { offset ->
            if (offset == 0) body.size.toByte() else 0
        } + body
    }

    private val lane = ByteArray(32) { 5 }
    private val before = ByteArray(16) { if (it == 0) 9 else 0 }
    private val after = ByteArray(16) { if (it == 0) 10 else 0 }

    @Test fun fsyncedReservationPrecedesOneUseSigningAndReturnsExactRawEvidence() {
        val device = FakeDevice()
        val store = FakeStore()
        device.onGenerate = { assertTrue(store.preparing) }
        device.onSign = { assertTrue(store.reserved) }
        val selected = frame()
        val runner = SelectionCandidateRunnerV1(device, store)
        val prepared = runner.prepare(lane, before, after)
            as KeyMintOneUsePreparationResultV1.Prepared
        assertEquals(0, device.signCalls)
        val result = runner.collect(selected, lane, before, after, prepared.publicKey())
            as KeyMintOneUseSelectionResultV1.Evidence
        assertEquals(listOf("lookupPrepared", "beginPreparation", "persistPrepared",
            "lookup", "lookupPrepared", "reserve", "persist"), store.events)
        assertEquals(1, device.signCalls)
        assertContentEquals(selected, device.signedBytes)
        assertContentEquals(selected, result.canonicalSelectionFrame())
        assertContentEquals(device.attestationChallenge, result.attestationChallenge())
        assertContentEquals(prepared.attestationNonce(), result.attestationNonce())
        assertContentEquals(keyMintPreparedChallengeV1(result.attestationNonce(), lane, before, after),
            result.attestationChallenge())
        assertContentEquals(byteArrayOf(0x30, 0x02, 0x01, 0x01), result.signatureDer())
        assertFalse(result.recovered)
        assertEquals(1, device.deleteCalls)
        result.signatureDer().fill(0)
        assertEquals(0x30.toByte(), result.signatureDer()[0])
    }

    @Test fun ambiguousSignFreezesThePredecessorAndNeverSignsAgain() {
        val device = FakeDevice().apply { signFails = true }
        val store = FakeStore()
        val runner = SelectionCandidateRunnerV1(device, store)
        val committedKey = (runner.prepare(lane, before, after)
            as KeyMintOneUsePreparationResultV1.Prepared).publicKey()
        val first = runner.collect(frame(), lane, before, after, committedKey)
        assertTrue(first is KeyMintOneUseSelectionResultV1.Frozen)
        assertEquals("sign", first.stage)
        assertTrue(runner.collect(frame(), lane, before, after, committedKey)
            is KeyMintOneUseSelectionResultV1.Frozen)
        assertEquals(1, device.generateCalls)
        assertEquals(1, device.signCalls)
        assertEquals(1, device.deleteCalls)
    }

    @Test fun failedEvidencePersistenceFreezesAfterTheOnlySignature() {
        val device = FakeDevice()
        val store = FakeStore().apply { failPersist = true }
        val runner = SelectionCandidateRunnerV1(device, store)
        val committedKey = (runner.prepare(lane, before, after)
            as KeyMintOneUsePreparationResultV1.Prepared).publicKey()
        val result = runner.collect(frame(), lane, before, after, committedKey)
            as KeyMintOneUseSelectionResultV1.Frozen
        assertEquals("persist", result.stage)
        assertTrue(runner.collect(frame(), lane, before, after, committedKey)
            is KeyMintOneUseSelectionResultV1.Frozen)
        assertEquals(1, device.signCalls)
    }

    @Test fun orphanAliasCreatesADurableFreezeBeforeAnyReplacementKey() {
        val directory = Files.createTempDirectory("kagemusha-keymint-orphan-").toFile()
        try {
            val device = FakeDevice().apply { active = true }
            val first = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { }))
            val frozen = first.prepare(lane, before, after)
                as KeyMintOneUsePreparationResultV1.Frozen
            assertEquals("alias", frozen.stage)
            assertEquals(0, device.generateCalls)
            assertEquals(0, device.deleteCalls)
            device.active = false
            val recovered = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { })).prepare(lane, before, after)
            assertTrue(recovered is KeyMintOneUsePreparationResultV1.Frozen)
            assertEquals(0, device.generateCalls)
        } finally {
            directory.deleteRecursively()
        }
    }

    @Test fun partiallyGeneratedKeyRemainsUntouchedAndSlotFreezesAcrossRestart() {
        val directory = Files.createTempDirectory("kagemusha-keymint-generation-").toFile()
        try {
            val device = FakeDevice().apply { generateFailsAfterCreation = true }
            val first = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { })).prepare(lane, before, after)
                as KeyMintOneUsePreparationResultV1.Frozen
            assertEquals("generate", first.stage)
            assertEquals(1, device.generateCalls)
            assertTrue(device.active)
            assertEquals(0, device.deleteCalls)
            device.active = false
            val recovered = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { })).prepare(lane, before, after)
            assertTrue(recovered is KeyMintOneUsePreparationResultV1.Frozen)
            assertEquals(1, device.generateCalls)
        } finally {
            directory.deleteRecursively()
        }
    }

    @Test fun consumedButLostSignatureCannotBeReissuedAfterStoreRecreation() {
        val directory = Files.createTempDirectory("kagemusha-keymint-lost-signature-").toFile()
        try {
            var durableWrites = 0
            val device = FakeDevice().apply { signFails = true }
            val first = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { durableWrites += 1 }))
            val key = (first.prepare(lane, before, after)
                as KeyMintOneUsePreparationResultV1.Prepared).publicKey()
            assertTrue(first.collect(frame(), lane, before, after, key)
                is KeyMintOneUseSelectionResultV1.Frozen)
            assertEquals(3, durableWrites) // Preparing, prepared, and pre-sign intent.
            val second = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { durableWrites += 1 }))
            assertTrue(second.collect(frame(), lane, before, after, key)
                is KeyMintOneUseSelectionResultV1.Frozen)
            assertTrue(second.prepare(lane, before, after)
                is KeyMintOneUsePreparationResultV1.Frozen)
            assertEquals(1, device.generateCalls)
            assertEquals(1, device.signCalls)
            assertEquals(3, durableWrites)
        } finally {
            directory.deleteRecursively()
        }
    }

    @Test fun tornEvidenceAfterSuccessfulSigningFreezesAcrossRestart() {
        val directory = Files.createTempDirectory("kagemusha-keymint-torn-evidence-").toFile()
        try {
            val io = TestJournalIo { }.apply { tearEvidenceWrite = true }
            val device = FakeDevice()
            val first = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(directory, io))
            val key = (first.prepare(lane, before, after)
                as KeyMintOneUsePreparationResultV1.Prepared).publicKey()
            val result = first.collect(frame(), lane, before, after, key)
                as KeyMintOneUseSelectionResultV1.Frozen
            assertEquals("persist", result.stage)
            assertEquals(1, device.signCalls)
            val recovered = SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                directory, TestJournalIo { }))
            assertTrue(recovered.collect(frame(), lane, before, after, key)
                is KeyMintOneUseSelectionResultV1.Frozen)
            assertTrue(recovered.prepare(lane, before, after)
                is KeyMintOneUsePreparationResultV1.Frozen)
            assertEquals(1, device.generateCalls)
            assertEquals(1, device.signCalls)
        } finally {
            directory.deleteRecursively()
        }
    }

    @Test fun independentRunnersArbitrateOneKeyForTheSameLaneAndIndex() {
        val directory = Files.createTempDirectory("kagemusha-keymint-concurrent-").toFile()
        val executor = java.util.concurrent.Executors.newFixedThreadPool(2)
        try {
            val device = FakeDevice()
            val start = java.util.concurrent.CountDownLatch(1)
            val runners = List(2) {
                SelectionCandidateRunnerV1(device, FileSelectionIntentStoreV1(
                    directory, TestJournalIo { }))
            }
            val tasks = runners.map { runner -> executor.submit<KeyMintOneUsePreparationResultV1> {
                start.await()
                runner.prepare(lane, before, after)
            } }
            start.countDown()
            val results = tasks.map { it.get(5, java.util.concurrent.TimeUnit.SECONDS) }
            assertTrue(results.all { it is KeyMintOneUsePreparationResultV1.Prepared })
            assertEquals(1, results.count { (it as KeyMintOneUsePreparationResultV1.Prepared).recovered })
            assertContentEquals((results[0] as KeyMintOneUsePreparationResultV1.Prepared).publicKey(),
                (results[1] as KeyMintOneUsePreparationResultV1.Prepared).publicKey())
            assertEquals(1, device.generateCalls)
        } finally {
            executor.shutdownNow()
            directory.deleteRecursively()
        }
    }

    @Test fun recoveredEvidenceIsReturnedWithoutASecondKeyOrSignature() {
        val device = FakeDevice()
        val store = FakeStore()
        val runner = SelectionCandidateRunnerV1(device, store)
        val committedKey = (runner.prepare(lane, before, after)
            as KeyMintOneUsePreparationResultV1.Prepared).publicKey()
        runner.collect(frame(), lane, before, after, committedKey)
        val recovered = runner.collect(frame(), lane, before, after, committedKey)
            as KeyMintOneUseSelectionResultV1.Evidence
        assertTrue(recovered.recovered)
        assertEquals(1, device.generateCalls)
        assertEquals(1, device.signCalls)
        device.feature = false
        assertTrue((runner.collect(frame(), lane, before, after, committedKey)
            as KeyMintOneUseSelectionResultV1.Evidence).recovered)
    }

    @Test fun collectRequiresAPreviouslyCommittedKeyAndChecksTheSelectedAliasBeforeSigning() {
        val device = FakeDevice()
        val store = FakeStore()
        val runner = SelectionCandidateRunnerV1(device, store)
        assertTrue(runner.collect(frame(), lane, before, after, fakeSec1(1))
            is KeyMintOneUseSelectionResultV1.Frozen)
        assertEquals(0, device.generateCalls)
        val prepared = runner.prepare(lane, before, after)
            as KeyMintOneUsePreparationResultV1.Prepared
        val wrongKey = runner.collect(frame(), lane, before, after, fakeSec1(9))
            as KeyMintOneUseSelectionResultV1.Frozen
        assertEquals("binding", wrongKey.stage)
        assertFalse(store.reserved)
        device.selectedPublicKey = fakeSec1(8)
        val substitutedAlias = runner.collect(frame(), lane, before, after, prepared.publicKey())
            as KeyMintOneUseSelectionResultV1.Frozen
        assertEquals("binding", substitutedAlias.stage)
        assertFalse(store.reserved)
        assertEquals(0, device.signCalls)
    }

    @Test fun unsupportedHardwareAndMalformedFramesNeverReserve() {
        val device = FakeDevice().apply { feature = false }
        val store = FakeStore()
        val runner = SelectionCandidateRunnerV1(device, store)
        assertTrue(runner.prepare(lane, before, after)
            is KeyMintOneUsePreparationResultV1.Unavailable)
        assertTrue(runner.collect(frame(), lane, before, after, fakeSec1(1))
            is KeyMintOneUseSelectionResultV1.Frozen)
        assertFalse(store.reserved)
        try {
            runner.collect(frame().copyOfRange(1, frame().size), lane, before, after, fakeSec1(1))
            throw AssertionError("malformed frame accepted")
        } catch (_: IllegalArgumentException) { }
        try {
            runner.collect(frame(), lane, before, before, fakeSec1(1))
            throw AssertionError("reused index accepted")
        } catch (_: IllegalArgumentException) { }
        assertFalse(store.reserved)
    }

    @Test fun fileIntentSurvivesRecreationAndDifferentFrameCannotReuseTheSameSlot() {
        val directory = Files.createTempDirectory("kagemusha-keymint-intent-").toFile()
        try {
            var directorySyncs = 0
            val io = TestJournalIo { directorySyncs += 1 }
            val store = FileSelectionIntentStoreV1(directory, io)
            val selected = frame()
            val digest = MessageDigest.getInstance("SHA-256").digest(selected)
            val slot = "a".repeat(64)
            val nonce = ByteArray(32) { 6 }
            val challenge = keyMintPreparedChallengeV1(nonce, lane, before, after)
            assertTrue(store.lookupPrepared(slot, lane, before, after) is PreparationLookupV1.Empty)
            store.beginPreparation(slot, lane, before, after, nonce, challenge)
            assertEquals(1, directorySyncs)
            assertTrue(store.lookupPrepared(slot, lane, before, after) is PreparationLookupV1.Frozen)
            store.persistPrepared(slot, KeyMintOneUsePreparationResultV1.Prepared(
                fakeSec1(1), nonce, challenge, listOf(byteArrayOf(2)), false,
            ))
            assertEquals(2, directorySyncs)
            assertTrue(store.lookupPrepared(slot, lane, before, after) is PreparationLookupV1.Ready)
            assertTrue(store.lookup(slot, digest, lane, before, after) is SelectionLookupV1.Empty)
            store.reserve(slot, digest, lane, before, after, challenge)
            assertEquals(3, directorySyncs)
            assertFailsWith<java.nio.file.FileAlreadyExistsException> {
                store.reserve(slot, digest, lane, before, after, challenge)
            }
            assertTrue(FileSelectionIntentStoreV1(directory, io)
                .lookup(slot, digest, lane, before, after) is SelectionLookupV1.Frozen)
            val evidence = KeyMintOneUseSelectionResultV1.Evidence(
                selected, lane, before, after, nonce, challenge, fakeSec1(1),
                listOf(byteArrayOf(2)), byteArrayOf(0x30), false, null,
            )
            store.persist(slot, digest, evidence)
            assertEquals(4, directorySyncs)
            val recreated = FileSelectionIntentStoreV1(directory, io)
            val recovered = recreated.lookup(slot, digest, lane, before, after)
                as SelectionLookupV1.Recovered
            assertContentEquals(selected, recovered.evidence.canonicalSelectionFrame())
            assertTrue(recreated.lookup(slot, MessageDigest.getInstance("SHA-256")
                .digest(frame(byteArrayOf(9))), lane, before, after) is SelectionLookupV1.Frozen)
            val orphan = "b".repeat(64)
            java.io.File(directory, "kagemusha-keymint-selection-$orphan.evidence")
                .writeBytes(byteArrayOf(1))
            assertTrue(recreated.lookup(orphan, digest, lane, before, after)
                is SelectionLookupV1.Frozen)
            assertFailsWith<IllegalArgumentException> {
                recreated.reserve(orphan, digest, lane, before, after, challenge)
            }
        } finally {
            directory.deleteRecursively()
        }
    }

    @Test fun symbolicJournalEntryFreezesBeforeKeyGeneration() {
        val directory = Files.createTempDirectory("kagemusha-keymint-symlink-").toFile()
        try {
            val slot = MessageDigest.getInstance("SHA-256").digest(lane + before)
                .joinToString("") { "%02x".format(it.toInt() and 0xff) }
            val target = java.io.File(directory, "unrelated").apply { writeBytes(byteArrayOf(1)) }
            Files.createSymbolicLink(
                java.io.File(directory, "kagemusha-keymint-selection-$slot.preparing").toPath(),
                target.toPath(),
            )
            val device = FakeDevice()
            val store = FileSelectionIntentStoreV1(directory, TestJournalIo { })
            assertTrue(SelectionCandidateRunnerV1(device, store).prepare(lane, before, after)
                is KeyMintOneUsePreparationResultV1.Frozen)
            assertEquals(0, device.generateCalls)
        } finally {
            directory.deleteRecursively()
        }
    }
}
