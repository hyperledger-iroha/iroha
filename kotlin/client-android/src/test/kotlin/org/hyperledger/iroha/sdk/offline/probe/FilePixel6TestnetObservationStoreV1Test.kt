// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.io.File
import java.nio.file.Files
import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertIs
import org.junit.jupiter.api.Test

class FilePixel6TestnetObservationStoreV1Test {
    private class MemoryIo : SelectionJournalIoV1 {
        val files = mutableMapOf<String, ByteArray>()

        override fun exists(file: File): Boolean = files.containsKey(file.absolutePath)

        override fun read(file: File, maximum: Int): ByteArray =
            requireNotNull(files[file.absolutePath]) { "missing journal file" }.copyOf().also {
                require(it.size in 1..maximum)
            }

        override fun writeNew(file: File, bytes: ByteArray) {
            check(files.putIfAbsent(file.absolutePath, bytes.copyOf()) == null)
        }

        override fun <T> withLock(file: File, action: () -> T): T = action()
    }

    private val network = ByteArray(32) { 1 }
    private val release = ByteArray(32) { 2 }
    private val lane = ByteArray(32) { 3 }
    private val before = ByteArray(16)
    private val after = byteArrayOf(1) + ByteArray(15)
    private val nonce = ByteArray(32) { 4 }
    private val frame = ByteArray(460).also {
        val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000"
            .toByteArray(Charsets.US_ASCII)
        domain.copyInto(it)
        it[49] = 0x93.toByte()
        it[50] = 1
        it[57] = 1
        release.copyInto(it, 59)
        it[91] = 1
        it[123] = 1
        it[155] = 1
        network.copyInto(it, 187)
        lane.copyInto(it, 219)
        it[251] = 1
        it[283] = 1
        it[291] = 1
        it[323] = 1
        it[331] = 1
        it[332] = 1
        before.copyInto(it, 428)
        after.copyInto(it, 444)
    }

    @Test
    fun incompleteReservationAndCorruptEvidenceFreezeRecovery() {
        val directory = Files.createTempDirectory("iroha-pixel6-testnet-store-").toFile()
        try {
            val io = MemoryIo()
            val store = FilePixel6TestnetObservationStoreV1(directory, io)
            val context = "iroha:kagemusha:v1:pixel6-testnet-context\u0000"
                .toByteArray(Charsets.US_ASCII) + network + release + lane + before + after +
                sha256(frame)
            val digest = sha256(context)
            val challenge = sha256(
                "iroha:kagemusha:v1:pixel6-testnet-attestation\u0000"
                    .toByteArray(Charsets.US_ASCII) + digest + nonce,
            )
            val slot = sha256(lane + before).joinToString("") { "%02x".format(it.toInt() and 0xff) }
            val intent = digest + challenge
            assertIs<Pixel6TestnetObservationLookupV1.Empty>(store.lookup(slot, digest))
            store.reserve(slot, intent)
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))

            val evidence = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, nonce, challenge,
                byteArrayOf(0x04) + ByteArray(64) { 5 }, listOf(byteArrayOf(1, 2, 3)),
                byteArrayOf(0x30, 0x01), false,
            )
            store.persist(slot, intent, evidence)
            val recovered = assertIs<Pixel6TestnetObservationLookupV1.Recovered>(
                store.lookup(slot, digest),
            )
            assertContentEquals(frame, recovered.evidence.canonicalSelectionFrame())
            assertContentEquals(challenge, recovered.evidence.attestationChallenge())
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(
                store.lookup(slot, ByteArray(32) { 9 }),
            )

            val evidenceFile = File(directory, "kagemusha-pixel6-testnet-$slot.evidence")
            val stored = requireNotNull(io.files[evidenceFile.absolutePath])
            stored[0] = 0
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
            io.files.remove(File(directory, "kagemusha-pixel6-testnet-$slot.intent").absolutePath)
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
        } finally {
            directory.delete()
        }
    }

    private fun sha256(bytes: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(bytes)
}
