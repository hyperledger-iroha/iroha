// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.Signature
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.security.spec.ECGenParameterSpec

/**
 * Collects raw, explicitly non-qualified Pixel 6 testnet observations.
 *
 * StrongBox is requested, but Android may enforce the one-use limit in software. An app-private
 * journal prevents accidental retries only; it supplies no rollback-resistant no-fork guarantee.
 * This result has no conversion to production hardware evidence or monetary authority.
 */
object AndroidPixel6TestnetStrongBoxObservationV1 {
    const val PROFILE = "android-pixel6-strongbox-experimental-v1"

    /** Validate an exact testnet selection before an app retains it or invokes StrongBox. */
    @JvmStatic
    fun requireCanonicalFrame(
        networkId: ByteArray,
        releaseId: ByteArray,
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
    ) = requirePixel6CanonicalFrameV1(
        canonicalSelectionFrame, networkId, releaseId, laneCommitment,
        secureIndexBeforeLittleEndian, secureIndexAfterLittleEndian,
    )

    @JvmStatic
    fun collect(
        context: Context,
        networkId: ByteArray,
        releaseId: ByteArray,
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
    ): Pixel6TestnetObservationResultV1 = Pixel6TestnetObservationRunnerV1(
        AndroidPixel6StrongBoxDeviceV1(context),
        FilePixel6TestnetObservationStoreV1(context.noBackupFilesDir),
    ).collect(networkId, releaseId, canonicalSelectionFrame, laneCommitment,
        secureIndexBeforeLittleEndian, secureIndexAfterLittleEndian)
}

/** A raw observation, never a qualified hardware one-use certificate. */
sealed interface Pixel6TestnetObservationResultV1 {
    data class Unavailable(val reason: String) : Pixel6TestnetObservationResultV1
    data class Frozen(val stage: String, val reason: String) : Pixel6TestnetObservationResultV1

    class Evidence internal constructor(
        networkId: ByteArray,
        releaseId: ByteArray,
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
        attestationNonce: ByteArray,
        attestationChallenge: ByteArray,
        publicKey: ByteArray,
        certificateChain: List<ByteArray>,
        signatureDer: ByteArray,
        val recovered: Boolean,
    ) : Pixel6TestnetObservationResultV1 {
        val profile: String = AndroidPixel6TestnetStrongBoxObservationV1.PROFILE
        val hardwareOneUseQualified: Boolean = false
        private val network = networkId.copyOf()
        private val release = releaseId.copyOf()
        private val frame = canonicalSelectionFrame.copyOf()
        private val lane = laneCommitment.copyOf()
        private val before = secureIndexBeforeLittleEndian.copyOf()
        private val after = secureIndexAfterLittleEndian.copyOf()
        private val nonce = attestationNonce.copyOf()
        private val challenge = attestationChallenge.copyOf()
        private val key = publicKey.copyOf()
        private val chain = certificateChain.map(ByteArray::copyOf)
        private val signature = signatureDer.copyOf()

        fun networkId(): ByteArray = network.copyOf()
        fun releaseId(): ByteArray = release.copyOf()
        fun canonicalSelectionFrame(): ByteArray = frame.copyOf()
        fun laneCommitment(): ByteArray = lane.copyOf()
        fun secureIndexBeforeLittleEndian(): ByteArray = before.copyOf()
        fun secureIndexAfterLittleEndian(): ByteArray = after.copyOf()
        fun attestationNonce(): ByteArray = nonce.copyOf()
        fun attestationChallenge(): ByteArray = challenge.copyOf()
        fun publicKey(): ByteArray = key.copyOf()
        fun certificateChain(): List<ByteArray> = chain.map(ByteArray::copyOf)
        fun signatureDer(): ByteArray = signature.copyOf()
        fun signedMessage(): ByteArray = pixel6TestnetSignedMessageV1(
            network, release, frame, lane, before, after, challenge,
        )
    }
}

internal interface Pixel6TestnetObservationDeviceV1 {
    val apiLevel: Int
    fun isPixel6(): Boolean
    fun hasStrongBox(): Boolean
    fun newNonce(): ByteArray
    fun hasAlias(alias: String): Boolean
    fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1
    fun sign(alias: String, message: ByteArray): ByteArray
    fun delete(alias: String)
}

internal interface Pixel6TestnetObservationStoreV1 {
    fun <T> withSlotLock(slot: String, action: () -> T): T
    fun lookup(slot: String, intent: ByteArray): Pixel6TestnetObservationLookupV1
    fun reserve(slot: String, intent: ByteArray)
    fun persist(slot: String, intent: ByteArray, evidence: Pixel6TestnetObservationResultV1.Evidence)
}

internal sealed interface Pixel6TestnetObservationLookupV1 {
    object Empty : Pixel6TestnetObservationLookupV1
    object Frozen : Pixel6TestnetObservationLookupV1
    class Recovered(val evidence: Pixel6TestnetObservationResultV1.Evidence) :
        Pixel6TestnetObservationLookupV1
}

internal class Pixel6TestnetObservationRunnerV1(
    private val device: Pixel6TestnetObservationDeviceV1,
    private val store: Pixel6TestnetObservationStoreV1,
) {
    fun collect(
        networkId: ByteArray,
        releaseId: ByteArray,
        canonicalSelectionFrame: ByteArray,
        laneCommitment: ByteArray,
        secureIndexBeforeLittleEndian: ByteArray,
        secureIndexAfterLittleEndian: ByteArray,
    ): Pixel6TestnetObservationResultV1 {
        val network = networkId.copyOf()
        val release = releaseId.copyOf()
        val frame = canonicalSelectionFrame.copyOf()
        val lane = laneCommitment.copyOf()
        val before = secureIndexBeforeLittleEndian.copyOf()
        val after = secureIndexAfterLittleEndian.copyOf()
        requirePixel6CanonicalFrameV1(frame, network, release, lane, before, after)
        // A slot is scoped to the predecessor, across all network/release requests. Changing
        // either scope cannot create a second local selection for the same lane predecessor.
        val slot = pixel6HexV1(pixel6Sha256V1(lane + before))
        return try {
            store.withSlotLock(slot) { collectLocked(slot, network, release, frame, lane, before, after) }
        } catch (error: Exception) {
            Pixel6TestnetObservationResultV1.Frozen("lock", error.javaClass.name)
        }
    }

    private fun collectLocked(
        slot: String,
        network: ByteArray,
        release: ByteArray,
        frame: ByteArray,
        lane: ByteArray,
        before: ByteArray,
        after: ByteArray,
    ): Pixel6TestnetObservationResultV1 {
        // Look up the slot before using hardware. A prior intent with another frame, network,
        // release, or index is frozen, even when its device call failed or the process crashed.
        val digest = pixel6Sha256V1(pixel6TestnetContextV1(network, release, frame, lane, before, after))
        val alias = "iroha_kagemusha_pixel6_testnet_$slot"
        if (!device.isPixel6()) {
            return Pixel6TestnetObservationResultV1.Unavailable("Pixel 6 is required")
        }
        val existing = try { store.lookup(slot, digest) }
            catch (_: Exception) { Pixel6TestnetObservationLookupV1.Frozen }
        when (existing) {
            is Pixel6TestnetObservationLookupV1.Recovered -> return existing.evidence
            Pixel6TestnetObservationLookupV1.Frozen -> return Pixel6TestnetObservationResultV1.Frozen(
                "recovery", "predecessor has an incomplete or conflicting testnet intent",
            )
            Pixel6TestnetObservationLookupV1.Empty -> Unit
        }
        if (device.apiLevel < 31) {
            return Pixel6TestnetObservationResultV1.Unavailable("Android API 31 is required")
        }
        if (try { !device.hasStrongBox() } catch (_: Exception) { true }) {
            return Pixel6TestnetObservationResultV1.Unavailable("StrongBox is unavailable")
        }
        val nonce = try {
            device.newNonce().copyOf().also {
                require(it.size == 32 && it.any { byte -> byte != 0.toByte() })
            }
        } catch (error: Exception) {
            return Pixel6TestnetObservationResultV1.Frozen("nonce", error.javaClass.name)
        }
        val challenge = pixel6TestnetAttestationChallengeV1(digest, nonce)
        val intent = digest + challenge
        try {
            store.reserve(slot, intent)
        } catch (error: Exception) {
            nonce.fill(0)
            return Pixel6TestnetObservationResultV1.Frozen("reserve", error.javaClass.name)
        }
        val attestationNonce = nonce.copyOf()
        nonce.fill(0)
        // The reservation must precede every alias inspection, key generation, and signature.
        val aliasExists = try { device.hasAlias(alias) } catch (_: Exception) { true }
        if (aliasExists) {
            return Pixel6TestnetObservationResultV1.Frozen("alias", "testnet alias exists or cannot be inspected")
        }
        var stage = "generate"
        var evidence: Pixel6TestnetObservationResultV1.Evidence? = null
        var failure: String? = null
        try {
            val material = device.generate(alias, challenge)
            require(material.publicKey.size == 65 && material.publicKey[0] == 0x04.toByte())
            require(material.certificateChain.size in 1..8 &&
                material.certificateChain.all { it.isNotEmpty() && it.size <= 16 * 1024 })
            val message = pixel6TestnetSignedMessageV1(
                network, release, frame, lane, before, after, challenge,
            )
            stage = "sign"
            val signature = device.sign(alias, message)
            require(signature.size in 1..128)
            evidence = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, attestationNonce, challenge,
                material.publicKey, material.certificateChain, signature, false,
            )
        } catch (error: Exception) {
            failure = error.javaClass.name
        }
        val cleanup = try { device.delete(alias); null } catch (error: Exception) { error.javaClass.name }
        if (failure != null || evidence == null || cleanup != null) {
            return Pixel6TestnetObservationResultV1.Frozen(
                if (cleanup != null && failure == null) "cleanup" else stage,
                failure ?: cleanup ?: IllegalStateException::class.java.name,
            )
        }
        return try {
            store.persist(slot, intent, evidence)
            evidence
        } catch (error: Exception) {
            Pixel6TestnetObservationResultV1.Frozen("persist", error.javaClass.name)
        }
    }
}

/** Hardware codename is stable across localized marketing-model strings. */
internal fun isPixel6HardwareV1(manufacturer: String, device: String): Boolean =
    manufacturer.equals("Google", ignoreCase = true) && device == "oriole"

private class AndroidPixel6StrongBoxDeviceV1(private val context: Context) :
    Pixel6TestnetObservationDeviceV1 {
    override val apiLevel: Int get() = Build.VERSION.SDK_INT
    override fun isPixel6(): Boolean = isPixel6HardwareV1(Build.MANUFACTURER, Build.DEVICE)
    override fun hasStrongBox(): Boolean =
        context.packageManager.hasSystemFeature(PackageManager.FEATURE_STRONGBOX_KEYSTORE)
    override fun newNonce(): ByteArray = ByteArray(32).also(SecureRandom()::nextBytes)
    override fun hasAlias(alias: String): Boolean =
        KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.containsAlias(alias)

    override fun generate(alias: String, challenge: ByteArray): ProbeKeyMaterialV1 {
        check(apiLevel >= 31)
        check(!hasAlias(alias))
        val spec = KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_SIGN)
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setAttestationChallenge(challenge.copyOf())
            .setIsStrongBoxBacked(true)
            .setMaxUsageCount(1)
            .build()
        val generator = KeyPairGenerator.getInstance(KeyProperties.KEY_ALGORITHM_EC, "AndroidKeyStore")
        generator.initialize(spec)
        val pair = generator.generateKeyPair()
        val chain = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            .getCertificateChain(alias)?.map { it.encoded }
            ?: throw IllegalStateException("no StrongBox attestation chain")
        val leaf = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            .getCertificate(alias) ?: throw IllegalStateException("no StrongBox certificate")
        val publicKey = uncompressedP256Sec1V1(pair.public)
        check(publicKey.contentEquals(uncompressedP256Sec1V1(leaf.publicKey))) {
            "generated key differs from attested certificate"
        }
        return ProbeKeyMaterialV1(publicKey, chain)
    }

    override fun sign(alias: String, message: ByteArray): ByteArray {
        val entry = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            .getEntry(alias, null) as? KeyStore.PrivateKeyEntry
            ?: throw IllegalStateException("testnet StrongBox key unavailable")
        return Signature.getInstance("SHA256withECDSA").run {
            initSign(entry.privateKey)
            update(message)
            sign()
        }
    }

    override fun delete(alias: String) {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        if (store.containsAlias(alias)) store.deleteEntry(alias)
    }
}

/** App-private persistence is only a local accident guard; it is not hardware rollback protection. */
internal class FilePixel6TestnetObservationStoreV1(
    private val directory: File,
    private val io: SelectionJournalIoV1 = AndroidSelectionJournalIoV1,
) : Pixel6TestnetObservationStoreV1 {
    override fun <T> withSlotLock(slot: String, action: () -> T): T =
        io.withLock(file(slot, ".lock"), action)

    override fun lookup(slot: String, intent: ByteArray): Pixel6TestnetObservationLookupV1 {
        val intentFile = file(slot, ".intent")
        val evidenceFile = file(slot, ".evidence")
        if (!io.exists(intentFile)) {
            return if (io.exists(evidenceFile)) Pixel6TestnetObservationLookupV1.Frozen
            else Pixel6TestnetObservationLookupV1.Empty
        }
        return try {
            val storedIntent = io.read(intentFile, 64)
            if (storedIntent.size != 64 ||
                !storedIntent.copyOfRange(0, 32).contentEquals(intent) ||
                !io.exists(evidenceFile)) {
                return Pixel6TestnetObservationLookupV1.Frozen
            }
            val evidence = decode(io.read(evidenceFile, MAX_EVIDENCE_BYTES))
            val context = pixel6TestnetContextV1(
                evidence.networkId(), evidence.releaseId(), evidence.canonicalSelectionFrame(),
                evidence.laneCommitment(), evidence.secureIndexBeforeLittleEndian(),
                evidence.secureIndexAfterLittleEndian(),
            )
            val digest = pixel6Sha256V1(context)
            if (!digest.contentEquals(storedIntent.copyOfRange(0, 32)) ||
                !evidence.attestationChallenge().contentEquals(storedIntent.copyOfRange(32, 64)) ||
                !evidence.attestationChallenge().contentEquals(
                    pixel6TestnetAttestationChallengeV1(digest, evidence.attestationNonce())) ||
                !pixel6HexV1(pixel6Sha256V1(evidence.laneCommitment() +
                    evidence.secureIndexBeforeLittleEndian())).contentEquals(slot) ||
                !pixel6RecoveredEvidenceSignatureValidV1(evidence)) {
                Pixel6TestnetObservationLookupV1.Frozen
            } else {
                Pixel6TestnetObservationLookupV1.Recovered(evidence)
            }
        } catch (_: Exception) {
            Pixel6TestnetObservationLookupV1.Frozen
        }
    }

    override fun reserve(slot: String, intent: ByteArray) {
        require(intent.size == 64)
        check(!io.exists(file(slot, ".evidence")))
        io.writeNew(file(slot, ".intent"), intent)
    }

    override fun persist(
        slot: String,
        intent: ByteArray,
        evidence: Pixel6TestnetObservationResultV1.Evidence,
    ) {
        require(io.read(file(slot, ".intent"), 64).contentEquals(intent))
        val digest = pixel6Sha256V1(pixel6TestnetContextV1(
            evidence.networkId(), evidence.releaseId(), evidence.canonicalSelectionFrame(),
            evidence.laneCommitment(), evidence.secureIndexBeforeLittleEndian(),
            evidence.secureIndexAfterLittleEndian(),
        ))
        require(intent.copyOfRange(0, 32).contentEquals(digest) &&
            intent.copyOfRange(32, 64).contentEquals(evidence.attestationChallenge()) &&
            evidence.attestationChallenge().contentEquals(
                pixel6TestnetAttestationChallengeV1(digest, evidence.attestationNonce())) &&
            pixel6HexV1(pixel6Sha256V1(evidence.laneCommitment() +
                evidence.secureIndexBeforeLittleEndian())) == slot)
        // Reject a malformed local observation before it can be returned or recovered. This
        // checks self-consistency only; an app-supplied certificate root is not monetary trust.
        require(pixel6RecoveredEvidenceSignatureValidV1(evidence))
        io.writeNew(file(slot, ".evidence"), encode(evidence))
    }

    private fun file(slot: String, suffix: String): File {
        require(slot.length == 64 && slot.all { it in '0'..'9' || it in 'a'..'f' })
        require(directory.isDirectory) { "private testnet observation directory is unavailable" }
        return File(directory, "kagemusha-pixel6-testnet-$slot$suffix")
    }

    private fun encode(evidence: Pixel6TestnetObservationResultV1.Evidence): ByteArray {
        val bytes = ByteArrayOutputStream()
        DataOutputStream(bytes).use { output ->
            output.write("IKP6TEV1".toByteArray(Charsets.US_ASCII))
            output.write(evidence.networkId())
            output.write(evidence.releaseId())
            output.write(evidence.laneCommitment())
            output.write(evidence.secureIndexBeforeLittleEndian())
            output.write(evidence.secureIndexAfterLittleEndian())
            output.write(evidence.attestationNonce())
            output.write(evidence.attestationChallenge())
            writeBounded(output, evidence.canonicalSelectionFrame(), 1024)
            writeBounded(output, evidence.publicKey(), 65)
            val chain = evidence.certificateChain()
            require(chain.size in 1..8)
            output.writeByte(chain.size)
            chain.forEach { writeBounded(output, it, 16 * 1024) }
            writeBounded(output, evidence.signatureDer(), 128)
        }
        return bytes.toByteArray()
    }

    private fun decode(bytes: ByteArray): Pixel6TestnetObservationResultV1.Evidence =
        DataInputStream(ByteArrayInputStream(bytes)).use { input ->
            val magic = ByteArray(8).also(input::readFully)
            require(magic.contentEquals("IKP6TEV1".toByteArray(Charsets.US_ASCII)))
            val network = ByteArray(32).also(input::readFully)
            val release = ByteArray(32).also(input::readFully)
            val lane = ByteArray(32).also(input::readFully)
            val before = ByteArray(16).also(input::readFully)
            val after = ByteArray(16).also(input::readFully)
            val nonce = ByteArray(32).also(input::readFully)
            val challenge = ByteArray(32).also(input::readFully)
            val frame = readBounded(input, 1024)
            val key = readBounded(input, 65)
            require(key.size == 65 && key[0] == 0x04.toByte())
            val count = input.readUnsignedByte()
            require(count in 1..8)
            val chain = List(count) { readBounded(input, 16 * 1024) }
            val signature = readBounded(input, 128)
            require(input.read() == -1)
            requirePixel6CanonicalFrameV1(frame, network, release, lane, before, after)
            Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, nonce, challenge, key, chain,
                signature, true,
            )
        }

    private fun writeBounded(output: DataOutputStream, bytes: ByteArray, maximum: Int) {
        require(bytes.size in 1..maximum)
        output.writeShort(bytes.size)
        output.write(bytes)
    }

    private fun readBounded(input: DataInputStream, maximum: Int): ByteArray {
        val size = input.readUnsignedShort()
        require(size in 1..maximum)
        return ByteArray(size).also(input::readFully)
    }

    companion object { private const val MAX_EVIDENCE_BYTES = 256 * 1024 }
}

/** Checks recovered evidence self-consistency; its certificate root and device remain untrusted. */
private fun pixel6RecoveredEvidenceSignatureValidV1(
    evidence: Pixel6TestnetObservationResultV1.Evidence,
): Boolean {
    val chain = evidence.certificateChain()
    require(chain.size in 1..8)
    val certificates = chain.map { der ->
        require(der.size in 1..16 * 1024)
        val (contentStart, contentEnd) = pixel6CanonicalDerHeaderV1(der, 0, 0x30)
        require(contentStart < contentEnd && contentEnd == der.size)
        val input = ByteArrayInputStream(der)
        val certificate = CertificateFactory.getInstance("X.509")
            .generateCertificate(input) as X509Certificate
        require(input.available() == 0 && certificate.encoded.contentEquals(der))
        certificate
    }
    val leaf = certificates.first()
    require(leaf.basicConstraints < 0 && leaf.keyUsage?.firstOrNull() == true)
    require(uncompressedP256Sec1V1(leaf.publicKey).contentEquals(evidence.publicKey()))
    require(pixel6StrongBoxAttestationMatchesChallengeV1(leaf, evidence.attestationChallenge()))
    certificates.zipWithNext().forEach { (child, issuer) ->
        require(child.issuerX500Principal == issuer.subjectX500Principal)
        child.verify(issuer.publicKey)
    }
    // A self-issued tail must also have a valid self-signature. This authenticates its bytes,
    // but does not make an app-supplied root trusted by KAGEMUSHA.
    certificates.last().let { root ->
        if (root.issuerX500Principal == root.subjectX500Principal) root.verify(root.publicKey)
    }
    val signature = evidence.signatureDer()
    pixel6RequireCanonicalEcdsaDerV1(signature)
    return Signature.getInstance("SHA256withECDSA").run {
        initVerify(leaf.publicKey)
        update(evidence.signedMessage())
        verify(signature)
    }
}

/** Check the raw StrongBox claim and exact challenge, without trusting the app-supplied root. */
private fun pixel6StrongBoxAttestationMatchesChallengeV1(
    certificate: X509Certificate,
    expectedChallenge: ByteArray,
): Boolean {
    require(expectedChallenge.size == 32)
    val extension = certificate.getExtensionValue("1.3.6.1.4.1.11129.2.1.17")
        ?: return false
    val (outerStart, outerEnd) = pixel6CanonicalDerHeaderV1(extension, 0, 0x04)
    require(outerEnd == extension.size)
    val description = extension.copyOfRange(outerStart, outerEnd)
    val (sequenceStart, sequenceEnd) = pixel6CanonicalDerHeaderV1(description, 0, 0x30)
    require(sequenceEnd == description.size)
    var offset = sequenceStart
    fun field(tag: Int): ByteArray {
        val (start, end) = pixel6CanonicalDerHeaderV1(description, offset, tag)
        offset = end
        return description.copyOfRange(start, end)
    }
    fun positiveInteger(tag: Int): Int {
        val bytes = field(tag)
        require(bytes.size in 1..4 && bytes[0].toInt() >= 0)
        require(bytes.size == 1 || bytes[0] != 0.toByte() ||
            (bytes[1].toInt() and 0x80) != 0)
        return bytes.fold(0) { value, byte -> (value shl 8) or (byte.toInt() and 0xff) }
    }
    val attestationVersion = positiveInteger(0x02)
    val attestationLevel = positiveInteger(0x0a)
    val keyMintVersion = positiveInteger(0x02)
    val keyMintLevel = positiveInteger(0x0a)
    val challenge = field(0x04)
    field(0x04) // Unique ID is not part of this non-authorizing observation.
    field(0x30) // Software authorization list.
    field(0x30) // Hardware authorization list.
    if (offset < sequenceEnd) field(0x30) // Optional StrongBox list on newer layouts.
    require(offset == sequenceEnd)
    return attestationVersion > 0 && keyMintVersion > 0 &&
        attestationLevel == 2 && keyMintLevel == 2 &&
        MessageDigest.isEqual(challenge, expectedChallenge)
}

/** Returns the content interval of a short, definite, minimally encoded DER element. */
private fun pixel6CanonicalDerHeaderV1(
    bytes: ByteArray,
    offset: Int,
    expectedTag: Int,
): Pair<Int, Int> {
    require(offset >= 0 && offset + 2 <= bytes.size)
    require((bytes[offset].toInt() and 0xff) == expectedTag)
    val lengthByte = bytes[offset + 1].toInt() and 0xff
    val headerLength: Int
    val contentLength: Int
    when {
        lengthByte < 0x80 -> {
            headerLength = 2
            contentLength = lengthByte
        }
        lengthByte == 0x81 -> {
            require(offset + 3 <= bytes.size)
            headerLength = 3
            contentLength = bytes[offset + 2].toInt() and 0xff
            require(contentLength >= 0x80)
        }
        lengthByte == 0x82 -> {
            require(offset + 4 <= bytes.size)
            headerLength = 4
            contentLength = ((bytes[offset + 2].toInt() and 0xff) shl 8) or
                (bytes[offset + 3].toInt() and 0xff)
            require(contentLength >= 0x100)
        }
        else -> throw IllegalArgumentException("noncanonical DER length")
    }
    val start = offset + headerLength
    val end = start + contentLength
    require(end <= bytes.size)
    return start to end
}

private fun pixel6RequireCanonicalEcdsaDerV1(signature: ByteArray) {
    require(signature.size in 8..72)
    val (sequenceStart, sequenceEnd) = pixel6CanonicalDerHeaderV1(signature, 0, 0x30)
    require(sequenceEnd == signature.size)
    val (rStart, rEnd) = pixel6CanonicalDerHeaderV1(signature, sequenceStart, 0x02)
    val (sStart, sEnd) = pixel6CanonicalDerHeaderV1(signature, rEnd, 0x02)
    require(sEnd == sequenceEnd)
    for ((start, end) in listOf(rStart to rEnd, sStart to sEnd)) {
        require(end - start in 1..33)
        val first = signature[start].toInt() and 0xff
        require(first < 0x80)
        require(end - start == 1 || first != 0 ||
            (signature[start + 1].toInt() and 0x80) != 0)
        require((start until end).any { signature[it] != 0.toByte() })
    }
}

private fun pixel6TestnetContextV1(
    network: ByteArray,
    release: ByteArray,
    frame: ByteArray,
    lane: ByteArray,
    before: ByteArray,
    after: ByteArray,
): ByteArray = "iroha:kagemusha:v1:pixel6-testnet-context\u0000".toByteArray(Charsets.US_ASCII) +
    network + release + lane + before + after + pixel6Sha256V1(frame)

private fun pixel6TestnetSignedMessageV1(
    network: ByteArray,
    release: ByteArray,
    frame: ByteArray,
    lane: ByteArray,
    before: ByteArray,
    after: ByteArray,
    challenge: ByteArray,
): ByteArray = "iroha:kagemusha:v1:pixel6-testnet-selection\u0000".toByteArray(Charsets.US_ASCII) +
    network + release + lane + before + after + challenge + frame

private fun pixel6TestnetAttestationChallengeV1(contextDigest: ByteArray, nonce: ByteArray): ByteArray {
    require(contextDigest.size == 32 && nonce.size == 32 && nonce.any { it != 0.toByte() })
    return pixel6Sha256V1(
        "iroha:kagemusha:v1:pixel6-testnet-attestation\u0000".toByteArray(Charsets.US_ASCII) +
            contextDigest + nonce,
    )
}

private fun pixel6Sha256V1(bytes: ByteArray): ByteArray =
    MessageDigest.getInstance("SHA-256").digest(bytes)

private fun pixel6HexV1(bytes: ByteArray): String =
    bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }

private fun pixel6ExactNextV1(before: ByteArray, after: ByteArray): Boolean {
    if (before.size != 16 || after.size != 16) return false
    var carry = 1
    for (index in before.indices) {
        val next = (before[index].toInt() and 0xff) + carry
        if (after[index] != next.toByte()) return false
        carry = next ushr 8
    }
    return carry == 0
}

private fun requirePixel6CanonicalFrameV1(
    frame: ByteArray,
    network: ByteArray,
    release: ByteArray,
    lane: ByteArray,
    before: ByteArray,
    after: ByteArray,
) {
    // These absolute offsets mirror KagemushaHardwareSelectionSigningLayoutV1 in
    // iroha_data_model. S = 49-byte domain || u64-LE(403) || 403 fixed body bytes.
    require(network.size == 32 && network.any { it != 0.toByte() })
    require(release.size == 32 && release.any { it != 0.toByte() })
    require(lane.size == 32 && lane.any { it != 0.toByte() })
    require(pixel6ExactNextV1(before, after))
    val domain = "iroha:kagemusha:v1:hardware-transition-selection\u0000"
        .toByteArray(Charsets.US_ASCII)
    require(domain.size == 49 && frame.size == 460)
    require(frame.copyOfRange(0, domain.size).contentEquals(domain))
    require(frame.copyOfRange(49, 57).contentEquals(
        byteArrayOf(0x93.toByte(), 0x01, 0, 0, 0, 0, 0, 0)))
    require(frame[57] == 1.toByte() && frame[58] == 0.toByte())
    require(frame.copyOfRange(59, 91).contentEquals(release))
    require(frame.copyOfRange(187, 219).contentEquals(network))
    require(frame.copyOfRange(219, 251).contentEquals(lane))
    require(frame.copyOfRange(428, 444).contentEquals(before))
    require(frame.copyOfRange(444, 460).contentEquals(after))
    // Match KagemushaHardwareTransitionSelectionV1.validate_shape. The canonical signed
    // frame must not describe an impossible monetary transition, even for a raw probe.
    fun nonzero(start: Int, end: Int): Boolean =
        frame.copyOfRange(start, end).any { it != 0.toByte() }
    require(nonzero(91, 123) && nonzero(123, 155) && nonzero(155, 187))
    require(nonzero(251, 283) && nonzero(283, 291) && nonzero(291, 323))
    require(nonzero(323, 331) && nonzero(332, 364))
    val operation = frame[331].toInt() and 0xff
    require(operation in 1..5)
    val outgoing = operation == 2 || operation == 4
    require(nonzero(364, 396) == outgoing && nonzero(396, 428) == outgoing)
}
