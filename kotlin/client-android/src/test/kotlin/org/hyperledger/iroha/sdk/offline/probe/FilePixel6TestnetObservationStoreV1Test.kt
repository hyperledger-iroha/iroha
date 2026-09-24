// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.probe

import java.io.File
import java.nio.file.Files
import java.security.KeyFactory
import java.security.MessageDigest
import java.security.Signature
import java.security.cert.CertificateFactory
import java.security.cert.X509Certificate
import java.security.spec.PKCS8EncodedKeySpec
import java.util.Base64
import kotlin.test.assertContentEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
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
    private val certificate = Base64.getDecoder().decode(TEST_CERTIFICATE_DER)
    private val privateKey = KeyFactory.getInstance("EC")
        .generatePrivate(PKCS8EncodedKeySpec(Base64.getDecoder().decode(TEST_PRIVATE_KEY_PKCS8)))
    private val publicKey = uncompressedP256Sec1V1(
        (CertificateFactory.getInstance("X.509")
            .generateCertificate(certificate.inputStream()) as X509Certificate).publicKey,
    )
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
    fun attestationChallengeMustMatchTheReservedPixel6Intent() {
        val directory = Files.createTempDirectory("iroha-pixel6-challenge-").toFile()
        try {
            val io = MemoryIo()
            val store = FilePixel6TestnetObservationStoreV1(directory, io)
            val context = "iroha:kagemusha:v1:pixel6-testnet-context\u0000"
                .toByteArray(Charsets.US_ASCII) + network + release + lane + before + after +
                sha256(frame)
            val digest = sha256(context)
            val alternateNonce = ByteArray(32) { 5 }
            val alternateChallenge = sha256(
                "iroha:kagemusha:v1:pixel6-testnet-attestation\u0000"
                    .toByteArray(Charsets.US_ASCII) + digest + alternateNonce,
            )
            val slot = sha256(lane + before).joinToString("") { "%02x".format(it.toInt() and 0xff) }
            val intent = digest + alternateChallenge
            store.reserve(slot, intent)
            val unsigned = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, alternateNonce,
                alternateChallenge, publicKey, listOf(certificate), byteArrayOf(0x30, 0), false,
            )
            val signature = Signature.getInstance("SHA256withECDSA").run {
                initSign(privateKey)
                update(unsigned.signedMessage())
                sign()
            }
            val evidence = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, alternateNonce,
                alternateChallenge, publicKey, listOf(certificate), signature, false,
            )
            assertFailsWith<IllegalArgumentException> { store.persist(slot, intent, evidence) }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
        } finally {
            directory.delete()
        }
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
            val evidenceFile = File(directory, "kagemusha-pixel6-testnet-$slot.evidence")
            assertIs<Pixel6TestnetObservationLookupV1.Empty>(store.lookup(slot, digest))
            store.reserve(slot, intent)
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))

            val unsigned = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, nonce, challenge,
                publicKey, listOf(certificate), byteArrayOf(0x30, 0), false,
            )
            assertFailsWith<IllegalArgumentException> {
                store.persist(slot, intent, unsigned)
            }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
            val wrongMessageSignature = Signature.getInstance("SHA256withECDSA").run {
                initSign(privateKey)
                update(unsigned.signedMessage() + byteArrayOf(1))
                sign()
            }
            val misbound = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, nonce, challenge,
                publicKey, listOf(certificate), wrongMessageSignature, false,
            )
            assertFailsWith<IllegalArgumentException> {
                store.persist(slot, intent, misbound)
            }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
            assertFalse(io.exists(evidenceFile))
            val signature = Signature.getInstance("SHA256withECDSA").run {
                initSign(privateKey)
                update(unsigned.signedMessage())
                sign()
            }
            val unattested = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, nonce, challenge,
                publicKey, listOf(Base64.getDecoder().decode(TEST_CERTIFICATE_WITHOUT_ATTESTATION_DER)),
                signature, false,
            )
            assertFailsWith<IllegalArgumentException> {
                store.persist(slot, intent, unattested)
            }
            assertFalse(io.exists(evidenceFile))
            val evidence = Pixel6TestnetObservationResultV1.Evidence(
                network, release, frame, lane, before, after, nonce, challenge,
                publicKey, listOf(certificate), signature, false,
            )
            store.persist(slot, intent, evidence)
            val recovered = assertIs<Pixel6TestnetObservationLookupV1.Recovered>(
                store.lookup(slot, digest),
            )
            assertContentEquals(frame, recovered.evidence.canonicalSelectionFrame())
            assertContentEquals(challenge, recovered.evidence.attestationChallenge())
            assertFalse(recovered.evidence.hardwareOneUseQualified)
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(
                store.lookup(slot, ByteArray(32) { 9 }),
            )

            val original = requireNotNull(io.files[evidenceFile.absolutePath]).copyOf()
            val keyOffset = original.indexOf(publicKey)
            val certificateOffset = original.indexOf(certificate)
            val signatureOffset = original.size - signature.size
            check(keyOffset >= 0 && certificateOffset > keyOffset && signatureOffset > certificateOffset)

            // The signed frame and reservation still match in these cases. Only the recovered
            // key material or signature changed, so cryptographic recovery must reject them.
            io.files[evidenceFile.absolutePath] = original.copyOf().also {
                it[it.lastIndex] = (it.last().toInt() xor 1).toByte()
            }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
            io.files[evidenceFile.absolutePath] = original.copyOf().also {
                it[keyOffset + 1] = (it[keyOffset + 1].toInt() xor 1).toByte()
            }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
            io.files[evidenceFile.absolutePath] = original.copyOf().also {
                it[certificateOffset] = 0x31
            }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))

            // A BER long-form length for a short ECDSA signature is not canonical DER.
            val noncanonical = original.copyOfRange(0, signatureOffset) +
                byteArrayOf(0x30, 0x81.toByte(), signature[1]) + signature.copyOfRange(2, signature.size)
            noncanonical[signatureOffset - 1] = (signature.size + 1).toByte()
            io.files[evidenceFile.absolutePath] = noncanonical
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))

            io.files[evidenceFile.absolutePath] = original.copyOf().also { it[0] = 0 }
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
            io.files.remove(File(directory, "kagemusha-pixel6-testnet-$slot.intent").absolutePath)
            assertIs<Pixel6TestnetObservationLookupV1.Frozen>(store.lookup(slot, digest))
        } finally {
            directory.delete()
        }
    }

    private fun sha256(bytes: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(bytes)

    private fun ByteArray.indexOf(needle: ByteArray): Int =
        (0..size - needle.size).firstOrNull { offset ->
            copyOfRange(offset, offset + needle.size).contentEquals(needle)
        } ?: -1

    companion object {
        // Disposable P-256 self-signed fixture with a synthetic StrongBox key-description
        // extension bound to this test's challenge. Its root is deliberately not a trusted
        // attestation anchor; this test checks recovery integrity only.
        private const val TEST_CERTIFICATE_DER =
            "MIIB8DCCAZagAwIBAgIUULFZ/0MRK8WO0vsHfO5+D9VYTCYwCgYIKoZIzj0EAwIwIjEgMB4GA1UEAwwX" +
                "UGl4ZWw2IFRlc3QgT2JzZXJ2YXRpb24wHhcNMjYwOTIzMjE1MjM4WhcNMzYwOTIwMjE1MjM4WjAi" +
                "MSAwHgYDVQQDDBdQaXhlbDYgVGVzdCBPYnNlcnZhdGlvbjBZMBMGByqGSM49AgEGCCqGSM49AwEH" +
                "A0IABN73yR6FEJ11TDyKRgdKA9ghlWth5MkcSbQo0KVcsiZwRif6y4604+X23/cM4yXzwWa8iyty" +
                "aS/VHIZrEnnACm6jgakwgaYwHQYDVR0OBBYEFIGvkDRMoR89r29aIiIkVAOBn7vXMB8GA1UdIwQY" +
                "MBaAFIGvkDRMoR89r29aIiIkVAOBn7vXMEYGCisGAQQB1nkCAREEODA2AgIBLAoBAgICASwKAQIE" +
                "IPQtfOGYQ9YSptS7ynk6hEyh2dsuXVXva1LRSFDVkHimBAAwADAAMAwGA1UdEwEB/wQCMAAwDgYD" +
                "VR0PAQH/BAQDAgeAMAoGCCqGSM49BAMCA0gAMEUCIQCedSQhir4yFaTtcECQ8P7r7LfD5iGEgf98" +
                "VzcbhZlcpwIgc4R+6iAR2zhK3LTK9TN7sDLOALr0EV5EeoGIZDy2qk0="
        private const val TEST_PRIVATE_KEY_PKCS8 =
            "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgIck5030gf9buTp1iyAeuQtrjcY1E" +
                "18vjIQHuLZPcsL6hRANCAATe98kehRCddUw8ikYHSgPYIZVrYeTJHEm0KNClXLImcEYn+suOtOP" +
                "l9t/3DOMl88FmvIsrcmkv1RyGaxJ5wApu"
        private const val TEST_CERTIFICATE_WITHOUT_ATTESTATION_DER =
            "MIIBmDCCAT+gAwIBAgIULAE4QMrizh16uZ4LA56kjbMRZfswCgYIKoZIzj0EAwIwIjEgMB4GA1UEAwwX" +
                "UGl4ZWw2IFRlc3QgT2JzZXJ2YXRpb24wHhcNMjYwOTIzMTU1NjQ2WhcNMzYwOTIwMTU1NjQ2WjAi" +
                "MSAwHgYDVQQDDBdQaXhlbDYgVGVzdCBPYnNlcnZhdGlvbjBZMBMGByqGSM49AgEGCCqGSM49AwEH" +
                "A0IABN73yR6FEJ11TDyKRgdKA9ghlWth5MkcSbQo0KVcsiZwRif6y4604+X23/cM4yXzwWa8iyty" +
                "aS/VHIZrEnnACm6jUzBRMB0GA1UdDgQWBBSBr5A0TKEfPa9vWiIiJFQDgZ+71zAfBgNVHSMEGDAW" +
                "gBSBr5A0TKEfPa9vWiIiJFQDgZ+71zAPBgNVHRMBAf8EBTADAQH/MAoGCCqGSM49BAMCA0cAMEQC" +
                "IHiVMxE0ZA+faRAWymb5yxPKH+VPA9ne452xD6doq1yrAiBzLMkaZhteB/2VAJk602DqGn345OVY" +
                "sqpffzeQnqpLTw=="
    }
}
