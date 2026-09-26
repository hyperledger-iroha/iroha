// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.content.Context
import android.system.ErrnoException
import android.system.Os
import android.system.OsConstants
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.FileDescriptor
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.KagemushaKeyMintRawSelectionEvidenceV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.preparedChallengeV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.requireKagemushaCoreSelectionFrameV1

/**
 * Collects raw Android KeyMint evidence for one Core-selected transition in an ordinary app.
 *
 * This is an evidence collector, not a monetary device lifecycle provider. The certificate chain
 * must independently prove the application identity, hardware security level, hardware-enforced
 * `USAGE_COUNT_LIMIT = 1`, rollback policy, and exact attestation challenge. Core must separately
 * bind the selected lane/index to the canonical frame and fold qualified evidence for every
 * ancestor. An app-private intent file only prevents accidental local retries; its contents are
 * not a rollback-resistant hardware counter. StrongBox is not universally required.
 */
object AndroidKeyMintOneUseSelectionCandidateV1 {
    /** Pre-provision an attested one-use key so its public key can be committed before Core builds S. */
    @JvmStatic
    fun prepare(
        context: Context,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
    ): KeyMintOneUsePreparationResultV1 = SelectionCandidateRunnerV1(
        AndroidSingleUseProbeDeviceV1(context),
        FileSelectionIntentStoreV1(context.noBackupFilesDir),
    ).prepare(laneCommitment, secureIndexBeforeLittleEndian, secureIndexAfterLittleEndian)

    /** Reserve S and sign only with the previously prepared, committed one-use key. */
    @JvmStatic
    fun collect(
        context: Context,
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
        expectedCommittedPublicKey: ByteArray,
    ): KeyMintOneUseSelectionResultV1 = SelectionCandidateRunnerV1(
        AndroidSingleUseProbeDeviceV1(context),
        FileSelectionIntentStoreV1(context.noBackupFilesDir),
    ).collect(
        canonicalSelectionFrame,
        laneCommitment,
        secureIndexBeforeLittleEndian,
        secureIndexAfterLittleEndian,
        expectedCommittedPublicKey,
    )
}

/** Raw preparation result; only independent governance enrollment may commit this public key. */
sealed interface KeyMintOneUsePreparationResultV1 {
    data class Unavailable(val reason: String) : KeyMintOneUsePreparationResultV1
    data class Frozen(val stage: String, val reason: String) : KeyMintOneUsePreparationResultV1
    class Prepared internal constructor(
        publicKey: ByteArray,
        attestationNonce: ByteArray,
        attestationChallenge: ByteArray,
        certificateChain: List<ByteArray>,
        val recovered: Boolean,
    ) : KeyMintOneUsePreparationResultV1 {
        private val key = publicKey.copyOf()
        private val nonce = attestationNonce.copyOf()
        private val challenge = attestationChallenge.copyOf()
        private val chain = certificateChain.map(ByteArray::copyOf)
        fun publicKey(): ByteArray = key.copyOf()
        fun attestationNonce(): ByteArray = nonce.copyOf()
        fun attestationChallenge(): ByteArray = challenge.copyOf()
        fun certificateChain(): List<ByteArray> = chain.map(ByteArray::copyOf)
        internal fun recovered(): Prepared = Prepared(key, nonce, challenge, chain, true)
    }
}

/** The raw result never certifies a device, app, counter, transition, or monetary proof. */
sealed interface KeyMintOneUseSelectionResultV1 {
    /** Android does not advertise hardware enforcement of one-use keys. */
    data class Unavailable(val reason: String) : KeyMintOneUseSelectionResultV1

    /** The predecessor slot is frozen; a caller must not regenerate or retry a signing key. */
    data class Frozen(val stage: String, val reason: String) : KeyMintOneUseSelectionResultV1

    /** Original KeyMint DER signature and certificate chain, retained byte-identically. */
    class Evidence internal constructor(
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
        attestationNonce: ByteArray,
        attestationChallenge: ByteArray,
        publicKey: ByteArray,
        certificateChain: List<ByteArray>,
        signatureDer: ByteArray,
        /** A prior fsynced result was returned without re-signing. */
        val recovered: Boolean,
        /** Key deletion failed after the signed evidence was made durable. */
        val cleanupExceptionClass: String?,
    ) : KeyMintOneUseSelectionResultV1 {
        private val frame = canonicalSelectionFrame.copyOf()
        private val lane = laneCommitment.copyOf()
        private val before = secureIndexBeforeLittleEndian.copyOf()
        private val after = secureIndexAfterLittleEndian.copyOf()
        private val nonce = attestationNonce.copyOf()
        private val challenge = attestationChallenge.copyOf()
        private val key = publicKey.copyOf()
        private val chain = certificateChain.map(ByteArray::copyOf)
        private val signature = signatureDer.copyOf()

        fun canonicalSelectionFrame(): ByteArray = frame.copyOf()
        fun laneCommitment(): ByteArray = lane.copyOf()
        fun secureIndexBeforeLittleEndian(): ByteArray = before.copyOf()
        fun secureIndexAfterLittleEndian(): ByteArray = after.copyOf()
        fun attestationNonce(): ByteArray = nonce.copyOf()
        fun attestationChallenge(): ByteArray = challenge.copyOf()
        fun publicKey(): ByteArray = key.copyOf()
        fun certificateChain(): List<ByteArray> = chain.map(ByteArray::copyOf)
        fun signatureDer(): ByteArray = signature.copyOf()

        internal fun recovered(): Evidence = Evidence(
            frame, lane, before, after, nonce, challenge, key, chain, signature, true, null,
        )
    }
}

/** Copy the exact collected bytes into the platform-neutral, nonadmitting offline verifier input. */
fun KeyMintOneUseSelectionResultV1.Evidence.toRawAttestationEvidenceV1():
    KagemushaKeyMintRawSelectionEvidenceV1 = KagemushaKeyMintRawSelectionEvidenceV1(
        canonicalSelectionFrame(), laneCommitment(), secureIndexBeforeLittleEndian(),
        secureIndexAfterLittleEndian(), attestationNonce(), attestationChallenge(), publicKey(),
        certificateChain(), signatureDer(),
    )

internal interface SelectionIntentStoreV1 {
    fun <T> withSlotLock(slot: String, action: () -> T): T
    fun lookupPrepared(slot: String, lane: ByteArray, before: ByteArray, after: ByteArray):
        PreparationLookupV1
    fun beginPreparation(
        slot: String,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
        nonce: ByteArray,
        challenge: ByteArray,
    )
    fun persistPrepared(slot: String, preparation: KeyMintOneUsePreparationResultV1.Prepared)
    fun lookup(
        slot: String,
        frameDigest: ByteArray,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
    ): SelectionLookupV1
    fun reserve(
        slot: String,
        frameDigest: ByteArray,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
        challenge: ByteArray,
    )
    fun persist(slot: String, frameDigest: ByteArray, evidence: KeyMintOneUseSelectionResultV1.Evidence)
}

/** Platform file operations use no-follow opens, create-new, fsync, and a process-shared lock. */
internal interface SelectionJournalIoV1 {
    fun exists(file: File): Boolean
    fun read(file: File, maximum: Int): ByteArray
    fun writeNew(file: File, bytes: ByteArray)
    fun <T> withLock(file: File, action: () -> T): T
}

internal object AndroidSelectionJournalIoV1 : SelectionJournalIoV1 {
    private val processLocks = Array(256) { Any() }

    private fun closeAfterFailure(descriptor: FileDescriptor) {
        try { Os.close(descriptor) } catch (_: Exception) { /* The stream may have closed it. */ }
    }

    override fun exists(file: File): Boolean = try {
        val stat = Os.lstat(file.absolutePath)
        require(OsConstants.S_ISREG(stat.st_mode)) { "selection journal entry is not regular" }
        true
    } catch (error: ErrnoException) {
        if (error.errno == OsConstants.ENOENT) false else throw error
    }

    override fun read(file: File, maximum: Int): ByteArray {
        val descriptor = Os.open(
            file.absolutePath,
            OsConstants.O_RDONLY or OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            0,
        )
        try {
            val stat = Os.fstat(descriptor)
            require(OsConstants.S_ISREG(stat.st_mode) && stat.st_size in 1L..maximum.toLong()) {
                "selection journal entry is not a bounded regular file"
            }
            return FileInputStream(descriptor).use { stream ->
                val bytes = ByteArrayOutputStream()
                val buffer = ByteArray(4096)
                while (true) {
                    val count = stream.read(buffer)
                    if (count < 0) break
                    bytes.write(buffer, 0, count)
                    require(bytes.size() <= maximum) { "selection journal entry grew during read" }
                }
                bytes.toByteArray()
            }
        } catch (error: Throwable) {
            closeAfterFailure(descriptor)
            throw error
        }
    }

    override fun writeNew(file: File, bytes: ByteArray) {
        val descriptor = Os.open(
            file.absolutePath,
            OsConstants.O_WRONLY or OsConstants.O_CREAT or OsConstants.O_EXCL or
                OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            384,
        )
        try {
            FileOutputStream(descriptor).use { stream ->
                stream.write(bytes)
                stream.fd.sync()
            }
        } catch (error: Throwable) {
            closeAfterFailure(descriptor)
            throw error
        }
        val directory = checkNotNull(file.parentFile)
        val directoryDescriptor = Os.open(
            directory.absolutePath,
            OsConstants.O_RDONLY or OsConstants.O_NOFOLLOW or OsConstants.O_CLOEXEC,
            0,
        )
        try {
            Os.fsync(directoryDescriptor)
        } finally {
            Os.close(directoryDescriptor)
        }
    }

    override fun <T> withLock(file: File, action: () -> T): T {
        val monitor = processLocks[(file.absolutePath.hashCode() and Int.MAX_VALUE) % processLocks.size]
        return synchronized(monitor) {
            val descriptor = Os.open(
                file.absolutePath,
                OsConstants.O_RDWR or OsConstants.O_CREAT or OsConstants.O_NOFOLLOW or
                    OsConstants.O_CLOEXEC,
                384,
            )
            try {
                require(OsConstants.S_ISREG(Os.fstat(descriptor).st_mode)) {
                    "selection lock entry is not regular"
                }
                FileOutputStream(descriptor).use { stream ->
                    stream.channel.lock().use { action() }
                }
            } catch (error: Throwable) {
                closeAfterFailure(descriptor)
                throw error
            }
        }
    }
}

internal sealed interface PreparationLookupV1 {
    object Empty : PreparationLookupV1
    object Frozen : PreparationLookupV1
    class Ready(val preparation: KeyMintOneUsePreparationResultV1.Prepared) : PreparationLookupV1
}

internal sealed interface SelectionLookupV1 {
    object Empty : SelectionLookupV1
    object Frozen : SelectionLookupV1
    class Recovered(val evidence: KeyMintOneUseSelectionResultV1.Evidence) : SelectionLookupV1
}

/** Publicly recomputable attestation challenge for the prepared predecessor key. */
internal fun keyMintPreparedChallengeV1(
    nonce: ByteArray,
    lane: ByteArray,
    before: ByteArray,
    after: ByteArray,
): ByteArray {
    return preparedChallengeV1(nonce, lane, before, after)
}

/** Durable per-lane/predecessor records. Existing or torn intents are never retried. */
internal class FileSelectionIntentStoreV1(
    private val directory: File,
    private val io: SelectionJournalIoV1 = AndroidSelectionJournalIoV1,
) : SelectionIntentStoreV1 {
    override fun <T> withSlotLock(slot: String, action: () -> T): T =
        io.withLock(file(slot, ".lock"), action)
    override fun lookupPrepared(
        slot: String,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
    ): PreparationLookupV1 {
        val preparing = file(slot, ".preparing")
        if (!io.exists(preparing)) {
            return if (io.exists(file(slot, ".prepared")) || io.exists(file(slot, ".intent")) ||
                io.exists(file(slot, ".evidence"))) PreparationLookupV1.Frozen
            else PreparationLookupV1.Empty
        }
        return try {
            val preparingBytes = readBounded(preparing, PREPARING_BYTES)
            if (preparingBytes.size != PREPARING_BYTES ||
                !preparingBytes.copyOfRange(0, PREPARING_BYTES - 64).contentEquals(
                    PREPARING_MAGIC + lane + before + after,
                )
            ) return PreparationLookupV1.Frozen
            val prepared = file(slot, ".prepared")
            if (!io.exists(prepared)) return PreparationLookupV1.Frozen
            val record = decodePrepared(readBounded(prepared, MAX_PREPARED_BYTES))
            if (!record.lane.contentEquals(lane) || !record.before.contentEquals(before) ||
                !record.after.contentEquals(after) ||
                !record.preparation.attestationNonce().contentEquals(
                    preparingBytes.copyOfRange(PREPARING_BYTES - 64, PREPARING_BYTES - 32),
                ) ||
                !record.preparation.attestationChallenge().contentEquals(
                    preparingBytes.copyOfRange(PREPARING_BYTES - 32, PREPARING_BYTES),
                ) ||
                !record.preparation.attestationChallenge().contentEquals(
                    keyMintPreparedChallengeV1(record.preparation.attestationNonce(),
                        lane, before, after),
                )
            ) PreparationLookupV1.Frozen
            else PreparationLookupV1.Ready(record.preparation.recovered())
        } catch (_: Exception) {
            PreparationLookupV1.Frozen
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
        require(challenge.contentEquals(keyMintPreparedChallengeV1(nonce, lane, before, after)))
        require(!io.exists(file(slot, ".prepared")) && !io.exists(file(slot, ".intent")) &&
            !io.exists(file(slot, ".evidence"))) { "orphaned predecessor records freeze slot" }
        writeNew(file(slot, ".preparing"), PREPARING_MAGIC + lane + before + after + nonce + challenge)
    }

    override fun persistPrepared(
        slot: String,
        preparation: KeyMintOneUsePreparationResultV1.Prepared,
    ) {
        val preparing = readBounded(file(slot, ".preparing"), PREPARING_BYTES)
        require(preparing.size == PREPARING_BYTES &&
            preparing.copyOfRange(PREPARING_BYTES - 64, PREPARING_BYTES - 32)
                .contentEquals(preparation.attestationNonce()) &&
            preparing.copyOfRange(PREPARING_BYTES - 32, PREPARING_BYTES)
                .contentEquals(preparation.attestationChallenge())) {
            "preparation challenge changed before persistence"
        }
        val lane = preparing.copyOfRange(8, 40)
        val before = preparing.copyOfRange(40, 56)
        val after = preparing.copyOfRange(56, 72)
        writeNew(file(slot, ".prepared"), encodePrepared(lane, before, after, preparation))
    }

    override fun lookup(
        slot: String,
        frameDigest: ByteArray,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
    ): SelectionLookupV1 {
        val intent = file(slot, ".intent")
        if (!io.exists(intent)) {
            return if (io.exists(file(slot, ".evidence"))) {
                SelectionLookupV1.Frozen
            } else {
                SelectionLookupV1.Empty
            }
        }
        return try {
            val intentBytes = readBounded(intent, INTENT_BYTES)
            if (!intentBytes.contentEquals(intentRecord(frameDigest, lane, before, after,
                    intentBytes.copyOfRange(INTENT_BYTES - 32, INTENT_BYTES)))) {
                return SelectionLookupV1.Frozen
            }
            val evidenceFile = file(slot, ".evidence")
            if (!io.exists(evidenceFile)) return SelectionLookupV1.Frozen
            val evidence = decodeEvidence(readBounded(evidenceFile, MAX_EVIDENCE_BYTES))
            val prepared = lookupPrepared(slot, lane, before, after)
            if (prepared !is PreparationLookupV1.Ready) return SelectionLookupV1.Frozen
            if (!MessageDigest.getInstance("SHA-256").digest(evidence.canonicalSelectionFrame())
                    .contentEquals(frameDigest) ||
                !evidence.laneCommitment().contentEquals(lane) ||
                !evidence.secureIndexBeforeLittleEndian().contentEquals(before) ||
                !evidence.secureIndexAfterLittleEndian().contentEquals(after) ||
                !evidence.attestationNonce().contentEquals(
                    prepared.preparation.attestationNonce()) ||
                !evidence.attestationChallenge().contentEquals(
                    intentBytes.copyOfRange(INTENT_BYTES - 32, INTENT_BYTES)) ||
                !prepared.preparation.publicKey().contentEquals(evidence.publicKey()) ||
                !sameChain(prepared.preparation.certificateChain(), evidence.certificateChain())
            ) {
                SelectionLookupV1.Frozen
            } else {
                SelectionLookupV1.Recovered(evidence.recovered())
            }
        } catch (_: Exception) {
            SelectionLookupV1.Frozen
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
        val prepared = lookupPrepared(slot, lane, before, after)
        require(prepared is PreparationLookupV1.Ready &&
            prepared.preparation.attestationChallenge().contentEquals(challenge)) {
            "one-use key was not prepared for this predecessor"
        }
        require(!io.exists(file(slot, ".evidence"))) { "orphaned evidence freezes slot" }
        writeNew(file(slot, ".intent"), intentRecord(frameDigest, lane, before, after, challenge))
    }

    override fun persist(
        slot: String,
        frameDigest: ByteArray,
        evidence: KeyMintOneUseSelectionResultV1.Evidence,
    ) {
        val intent = readBounded(file(slot, ".intent"), INTENT_BYTES)
        require(intent.contentEquals(intentRecord(
            frameDigest, evidence.laneCommitment(), evidence.secureIndexBeforeLittleEndian(),
            evidence.secureIndexAfterLittleEndian(), evidence.attestationChallenge(),
        ))) {
            "selection intent changed before evidence persistence"
        }
        require(MessageDigest.getInstance("SHA-256").digest(evidence.canonicalSelectionFrame())
            .contentEquals(frameDigest)) { "selection frame changed before persistence" }
        val prepared = lookupPrepared(
            slot, evidence.laneCommitment(), evidence.secureIndexBeforeLittleEndian(),
            evidence.secureIndexAfterLittleEndian(),
        )
        require(prepared is PreparationLookupV1.Ready &&
            prepared.preparation.publicKey().contentEquals(evidence.publicKey()) &&
            prepared.preparation.attestationNonce().contentEquals(evidence.attestationNonce()) &&
            sameChain(prepared.preparation.certificateChain(), evidence.certificateChain())) {
            "selected key or attestation chain changed before evidence persistence"
        }
        writeNew(file(slot, ".evidence"), encodeEvidence(evidence))
    }

    private fun file(slot: String, suffix: String): File {
        require(slot.length == 64 && slot.all { it in '0'..'9' || it in 'a'..'f' })
        require(directory.isDirectory) { "private intent directory is unavailable" }
        return File(directory, "kagemusha-keymint-selection-$slot$suffix")
    }

    private fun writeNew(file: File, bytes: ByteArray) = io.writeNew(file, bytes)

    private fun readBounded(file: File, maximum: Int): ByteArray = io.read(file, maximum)

    companion object {
        private val MAGIC = "IKMINTV1".toByteArray(Charsets.US_ASCII)
        private val PREPARING_MAGIC = "IKMPREV1".toByteArray(Charsets.US_ASCII)
        private const val PREPARING_BYTES = 8 + 32 + 16 + 16 + 32 + 32
        private const val INTENT_BYTES = 8 + 32 + 32 + 16 + 16 + 32
        private const val MAX_PREPARED_BYTES = 256 * 1024
        private const val MAX_EVIDENCE_BYTES = 256 * 1024

        private class PreparedRecord(
            val lane: ByteArray,
            val before: ByteArray,
            val after: ByteArray,
            val preparation: KeyMintOneUsePreparationResultV1.Prepared,
        )

        private fun sameChain(left: List<ByteArray>, right: List<ByteArray>): Boolean =
            left.size == right.size && left.indices.all { left[it].contentEquals(right[it]) }

        private fun encodePrepared(
            lane: ByteArray,
            before: ByteArray,
            after: ByteArray,
            preparation: KeyMintOneUsePreparationResultV1.Prepared,
        ): ByteArray {
            val bytes = ByteArrayOutputStream()
            DataOutputStream(bytes).use { output ->
                output.write("IKMPKSV1".toByteArray(Charsets.US_ASCII))
                output.write(lane)
                output.write(before)
                output.write(after)
                output.write(preparation.attestationNonce())
                output.write(preparation.attestationChallenge())
                writeBytes(output, preparation.publicKey(), 128)
                val chain = preparation.certificateChain()
                require(chain.size in 1..8)
                output.writeByte(chain.size)
                chain.forEach { writeBytes(output, it, 16 * 1024) }
            }
            return bytes.toByteArray()
        }

        private fun decodePrepared(bytes: ByteArray): PreparedRecord =
            DataInputStream(ByteArrayInputStream(bytes)).use { input ->
                val magic = ByteArray(8).also(input::readFully)
                require(magic.contentEquals("IKMPKSV1".toByteArray(Charsets.US_ASCII)))
                val lane = ByteArray(32).also(input::readFully)
                val before = ByteArray(16).also(input::readFully)
                val after = ByteArray(16).also(input::readFully)
                val nonce = ByteArray(32).also(input::readFully)
                val challenge = ByteArray(32).also(input::readFully)
                val key = readBytes(input, 128)
                val chainCount = input.readUnsignedByte()
                require(chainCount in 1..8)
                val chain = List(chainCount) { readBytes(input, 16 * 1024) }
                require(input.read() == -1)
                PreparedRecord(lane, before, after,
                    KeyMintOneUsePreparationResultV1.Prepared(key, nonce, challenge, chain, true))
            }

        private fun intentRecord(
            frameDigest: ByteArray,
            lane: ByteArray,
            before: ByteArray,
            after: ByteArray,
            challenge: ByteArray,
        ): ByteArray {
            require(frameDigest.size == 32 && lane.size == 32 && before.size == 16 &&
                after.size == 16 && challenge.size == 32)
            return MAGIC + frameDigest + lane + before + after + challenge
        }

        private fun encodeEvidence(evidence: KeyMintOneUseSelectionResultV1.Evidence): ByteArray {
            val bytes = ByteArrayOutputStream()
            DataOutputStream(bytes).use { output ->
                output.write("IKMEVDV1".toByteArray(Charsets.US_ASCII))
                writeBytes(output, evidence.canonicalSelectionFrame(), 1024)
                output.write(evidence.laneCommitment())
                output.write(evidence.secureIndexBeforeLittleEndian())
                output.write(evidence.secureIndexAfterLittleEndian())
                output.write(evidence.attestationNonce())
                output.write(evidence.attestationChallenge())
                writeBytes(output, evidence.publicKey(), 128)
                val chain = evidence.certificateChain()
                require(chain.size in 1..8)
                output.writeByte(chain.size)
                chain.forEach { writeBytes(output, it, 16 * 1024) }
                writeBytes(output, evidence.signatureDer(), 128)
            }
            return bytes.toByteArray()
        }

        private fun decodeEvidence(bytes: ByteArray): KeyMintOneUseSelectionResultV1.Evidence =
            DataInputStream(ByteArrayInputStream(bytes)).use { input ->
                val magic = ByteArray(8).also(input::readFully)
                require(magic.contentEquals("IKMEVDV1".toByteArray(Charsets.US_ASCII)))
                val frame = readBytes(input, 1024)
                val lane = ByteArray(32).also(input::readFully)
                val before = ByteArray(16).also(input::readFully)
                val after = ByteArray(16).also(input::readFully)
                val nonce = ByteArray(32).also(input::readFully)
                val challenge = ByteArray(32).also(input::readFully)
                val publicKey = readBytes(input, 128)
                val chainCount = input.readUnsignedByte()
                require(chainCount in 1..8)
                val chain = List(chainCount) { readBytes(input, 16 * 1024) }
                val signature = readBytes(input, 128)
                require(input.read() == -1)
                KeyMintOneUseSelectionResultV1.Evidence(
                    frame, lane, before, after, nonce, challenge, publicKey, chain,
                    signature, true, null,
                )
            }

        private fun writeBytes(output: DataOutputStream, bytes: ByteArray, maximum: Int) {
            require(bytes.size in 1..maximum)
            output.writeShort(bytes.size)
            output.write(bytes)
        }

        private fun readBytes(input: DataInputStream, maximum: Int): ByteArray {
            val size = input.readUnsignedShort()
            require(size in 1..maximum)
            return ByteArray(size).also(input::readFully)
        }
    }
}

internal class SelectionCandidateRunnerV1(
    private val device: SingleUseProbeDeviceV1,
    private val store: SelectionIntentStoreV1,
) {
    fun prepare(
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
    ): KeyMintOneUsePreparationResultV1 {
        val lane = laneCommitment.copyOf()
        val before = secureIndexBeforeLittleEndian.copyOf()
        val after = secureIndexAfterLittleEndian.copyOf()
        require(lane.size == 32 && lane.any { it != 0.toByte() })
        require(exactNext(before, after)) { "secure index must be exact-next" }
        val slot = slot(lane, before)
        return try {
            store.withSlotLock(slot) { prepareLocked(slot, lane, before, after) }
        } catch (error: Exception) {
            KeyMintOneUsePreparationResultV1.Frozen("lock", error.javaClass.name)
        }
    }

    private fun prepareLocked(
        slot: String,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
    ): KeyMintOneUsePreparationResultV1 {
        val alias = alias(slot)
        when (val existing = try { store.lookupPrepared(slot, lane, before, after) }
            catch (_: Exception) { PreparationLookupV1.Frozen }) {
            is PreparationLookupV1.Ready -> {
                if (!matchesPreparedKey(alias, existing.preparation)) {
                    return KeyMintOneUsePreparationResultV1.Frozen(
                        "recovery", "prepared KeyMint key no longer matches attestation",
                    )
                }
                return existing.preparation.recovered()
            }
            PreparationLookupV1.Frozen -> return KeyMintOneUsePreparationResultV1.Frozen(
                "recovery", "predecessor key preparation is incomplete or conflicting",
            )
            PreparationLookupV1.Empty -> Unit
        }
        if (device.apiLevel < 31) {
            return KeyMintOneUsePreparationResultV1.Unavailable("Android API 31 is required")
        }
        if (try { !device.hasHardwareSingleUseFeature() } catch (_: Exception) { true }) {
            return KeyMintOneUsePreparationResultV1.Unavailable(
                "hardware one-use KeyMint feature is unavailable",
            )
        }
        val random = try {
            device.newChallenge().copyOf().also {
                require(it.size == 32 && it.any { byte -> byte != 0.toByte() })
            }
        } catch (error: Exception) {
            return KeyMintOneUsePreparationResultV1.Frozen("challenge", error.javaClass.name)
        }
        val challenge = keyMintPreparedChallengeV1(random, lane, before, after)
        try {
            store.beginPreparation(slot, lane, before, after, random, challenge)
        } catch (error: Exception) {
            random.fill(0)
            return KeyMintOneUsePreparationResultV1.Frozen("reserve", error.javaClass.name)
        }
        val aliasExists = try { device.hasAlias(alias) } catch (_: Exception) { true }
        if (aliasExists) {
            random.fill(0)
            // The durable .preparing marker remains: even deletion of an orphan alias must not
            // turn this predecessor into permission to generate a second one-use key.
            return KeyMintOneUsePreparationResultV1.Frozen(
                "alias", "a one-use KeyMint alias already exists or cannot be inspected",
            )
        }
        var stage = "generate"
        var prepared: KeyMintOneUsePreparationResultV1.Prepared? = null
        var failure: String? = null
        try {
            val material = device.generate(alias, challenge)
            require(validMaterial(material))
            val result = KeyMintOneUsePreparationResultV1.Prepared(
                material.publicKey, random, challenge, material.certificateChain, false,
            )
            random.fill(0)
            require(matchesPreparedKey(alias, result)) {
                "generated key differs from its KeyMint alias or certificate chain"
            }
            stage = "persist"
            store.persistPrepared(slot, result)
            prepared = result
        } catch (error: Exception) {
            random.fill(0)
            failure = error.javaClass.name
        }
        if (failure != null || prepared == null) {
            // Generation or persistence may have partially succeeded. The slot is durably frozen;
            // deleting an ambiguous alias here could erase another process's retained key.
            return KeyMintOneUsePreparationResultV1.Frozen(
                stage, failure ?: IllegalStateException::class.java.name,
            )
        }
        return prepared
    }

    fun collect(
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
        expectedCommittedPublicKey: ByteArray,
    ): KeyMintOneUseSelectionResultV1 {
        val frame = canonicalSelectionFrame.copyOf()
        val lane = laneCommitment.copyOf()
        val before = secureIndexBeforeLittleEndian.copyOf()
        val after = secureIndexAfterLittleEndian.copyOf()
        val expectedKey = expectedCommittedPublicKey.copyOf()
        require(lane.size == 32 && lane.any { it != 0.toByte() })
        require(exactNext(before, after)) { "secure index must be exact-next" }
        requireKagemushaCoreSelectionFrameV1(frame, lane, before, after)
        require(expectedKey.size == 65 && expectedKey[0] == 0x04.toByte()) {
            "committed key must be uncompressed P-256 SEC1"
        }
        val frameDigest = MessageDigest.getInstance("SHA-256").digest(frame)
        val slot = slot(lane, before)
        return try {
            store.withSlotLock(slot) {
                collectLocked(slot, frame, frameDigest, lane, before, after, expectedKey)
            }
        } catch (error: Exception) {
            KeyMintOneUseSelectionResultV1.Frozen("lock", error.javaClass.name)
        }
    }

    private fun collectLocked(
        slot: String,
        frame: ByteArray,
        frameDigest: ByteArray,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
        expectedKey: ByteArray,
    ): KeyMintOneUseSelectionResultV1 {
        when (val existing = try { store.lookup(slot, frameDigest, lane, before, after) } catch (_: Exception) {
            SelectionLookupV1.Frozen
        }) {
            is SelectionLookupV1.Recovered -> return if (existing.evidence.publicKey()
                .contentEquals(expectedKey)) existing.evidence
            else KeyMintOneUseSelectionResultV1.Frozen(
                "binding", "recovered evidence key differs from the committed key",
            )
            SelectionLookupV1.Frozen -> return KeyMintOneUseSelectionResultV1.Frozen(
                "recovery", "predecessor slot has an incomplete or conflicting intent",
            )
            SelectionLookupV1.Empty -> Unit
        }
        val prepared = when (val found = try { store.lookupPrepared(slot, lane, before, after) }
            catch (_: Exception) { PreparationLookupV1.Frozen }) {
            is PreparationLookupV1.Ready -> found.preparation
            else -> return KeyMintOneUseSelectionResultV1.Frozen(
                "preparation", "a committed, durable one-use key is required before selection",
            )
        }
        if (!prepared.publicKey().contentEquals(expectedKey)) {
            return KeyMintOneUseSelectionResultV1.Frozen(
                "binding", "prepared key differs from the committed key",
            )
        }
        val alias = alias(slot)
        if (!matchesPreparedKey(alias, prepared)) {
            return KeyMintOneUseSelectionResultV1.Frozen(
                "binding", "selected KeyMint alias differs from prepared attestation",
            )
        }
        if (device.apiLevel < 31) {
            return KeyMintOneUseSelectionResultV1.Unavailable("Android API 31 is required")
        }
        if (try { !device.hasHardwareSingleUseFeature() } catch (_: Exception) { true }) {
            return KeyMintOneUseSelectionResultV1.Unavailable(
                "hardware one-use KeyMint feature is unavailable",
            )
        }
        try {
            store.reserve(slot, frameDigest, lane, before, after,
                prepared.attestationChallenge())
        } catch (error: Exception) {
            return KeyMintOneUseSelectionResultV1.Frozen("reserve", error.javaClass.name)
        }
        var stage = "sign"
        var evidence: KeyMintOneUseSelectionResultV1.Evidence? = null
        var failure: String? = null
        try {
            val signature = device.sign(alias, frame)
            require(signature.isNotEmpty() && signature.size <= 128)
            val signedEvidence = KeyMintOneUseSelectionResultV1.Evidence(
                frame, lane, before, after, prepared.attestationNonce(),
                prepared.attestationChallenge(),
                prepared.publicKey(), prepared.certificateChain(), signature, false, null,
            )
            evidence = signedEvidence
            stage = "persist"
            store.persist(slot, frameDigest, signedEvidence)
        } catch (error: Exception) {
            failure = error.javaClass.name
        }
        val cleanupFailure = try {
            device.delete(alias)
            null
        } catch (error: Exception) {
            error.javaClass.name
        }
        if (failure != null || evidence == null) {
            return KeyMintOneUseSelectionResultV1.Frozen(
                stage, failure ?: IllegalStateException::class.java.name,
            )
        }
        return KeyMintOneUseSelectionResultV1.Evidence(
            evidence.canonicalSelectionFrame(), evidence.laneCommitment(),
            evidence.secureIndexBeforeLittleEndian(), evidence.secureIndexAfterLittleEndian(),
            evidence.attestationNonce(), evidence.attestationChallenge(), evidence.publicKey(),
            evidence.certificateChain(),
            evidence.signatureDer(), false, cleanupFailure,
        )
    }

    private fun matchesPreparedKey(
        alias: String,
        prepared: KeyMintOneUsePreparationResultV1.Prepared,
    ): Boolean = try {
        val material = device.read(alias)
        material.publicKey.contentEquals(prepared.publicKey()) &&
            sameChain(material.certificateChain, prepared.certificateChain())
    } catch (_: Exception) {
        false
    }

    private fun validMaterial(material: ProbeKeyMaterialV1): Boolean =
        material.publicKey.size == 65 && material.publicKey[0] == 0x04.toByte() &&
            material.certificateChain.size in 1..8 &&
            material.certificateChain.all { it.isNotEmpty() && it.size <= 16 * 1024 }

    private fun sameChain(left: List<ByteArray>, right: List<ByteArray>): Boolean =
        left.size == right.size && left.indices.all { left[it].contentEquals(right[it]) }

    private fun slot(lane: ByteArray, before: ByteArray): String =
        hex(MessageDigest.getInstance("SHA-256").digest(lane + before))

    private fun alias(slot: String): String = "iroha_kagemusha_v1_$slot"

    private fun exactNext(before: ByteArray, after: ByteArray): Boolean {
        if (before.size != 16 || after.size != 16) return false
        var carry = 1
        for (index in 0 until 16) {
            val sum = (before[index].toInt() and 0xff) + carry
            if (after[index] != sum.toByte()) return false
            carry = sum ushr 8
        }
        return carry == 0
    }

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") {
        "%02x".format(it.toInt() and 0xff)
    }
}
