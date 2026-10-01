package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Inert shape/original hashing tests. Genuine native golden parity is a separate required gate. */
class KagemushaOrdinaryAppEnrollmentPreparationV1Test {
    private fun fixture(): ByteArray = ByteArray(515).also { bytes ->
        bytes[0] = 1; bytes[2] = 1
        for (i in 0 until 13) bytes.fill((i + 1).toByte(), 3 + i * 32, 3 + (i + 1) * 32)
        put64(bytes, 419, 1); put64(bytes, 427, 2); put64(bytes, 435, 1000); put64(bytes, 443, 2000)
        bytes.fill(23, 451, 515)
    }
    private fun put64(bytes: ByteArray, offset: Int, value: Long) {
        ByteBuffer.wrap(bytes, offset, 8).order(ByteOrder.LITTLE_ENDIAN).putLong(value)
    }

    @Test fun exactOriginalBodyEpochAndSignatureRemainImmutableWithoutIssuerAdmission() {
        val bytes = fixture(); val expected = bytes.copyOf(); val projection = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(bytes)
        bytes.fill(0); assertArrayEquals(expected, projection.transportBytes())
        projection.transportBytes().fill(0); projection.clientNonce().fill(0); projection.signatureBytes().fill(0)
        assertArrayEquals(expected, projection.transportBytes())
        assertEquals(BigInteger.valueOf(2), projection.hardwareEpoch)
        val domain = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)
        val signing = projection.canonicalSigningBytes()
        assertArrayEquals(domain, signing.copyOfRange(0, domain.size))
        assertEquals(451L, ByteBuffer.wrap(signing, domain.size, 8).order(ByteOrder.LITTLE_ENDIAN).long)
        assertArrayEquals(expected.copyOfRange(0, 451), signing.copyOfRange(domain.size + 8, signing.size))
        assertArrayEquals(MessageDigest.getInstance("SHA-256").digest(signing), projection.attestationChallenge())
        // Decoding an invalid placeholder signature is still only public byte projection.
        KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(expected.copyOf().also { it.fill(0, 451, 515) })
    }

    @Test fun signedHardwareEpochAndActualGeneratedKeySeparatelyBindAttestationAndIntegrity() {
        val first = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(fixture())
        val changed = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(fixture().also { put64(it, 427, 3) })
        val key = ByteArray(32) { 91 }
        assertFalse(first.attestationChallenge().contentEquals(changed.attestationChallenge()))
        assertFalse(first.attestationChallenge().contentEquals(first.playIntegrityRequestHash(key)))
        assertFalse(first.playIntegrityRequestHash(key).contentEquals(changed.playIntegrityRequestHash(key)))
        assertFalse(first.playIntegrityRequestHash(key).contentEquals(first.playIntegrityRequestHash(ByteArray(32) { 92 })))
        val anotherSignature = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(fixture().also { it.fill(24, 451, 515) })
        assertArrayEquals(first.attestationChallenge(), anotherSignature.attestationChallenge())
        assertThrows(IllegalArgumentException::class.java) { first.playIntegrityRequestHash(ByteArray(32)) }
    }

    @Test fun supersededWidthsInvalidTagsMissingSelectorsAndUnboundedIntervalsAreRejected() {
        for (size in listOf(273, 507, 514, 516)) assertThrows(IllegalArgumentException::class.java) {
            KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(ByteArray(size))
        }
        val invalid = mutableListOf(fixture().also { it[0] = 2 }, fixture().also { it[1] = 1 }, fixture().also { it[2] = 0 },
            fixture().also { it[2] = 3 }, fixture().also { it.copyOfRange(35, 67).copyInto(it, 67) },
            fixture().also { put64(it, 427, 0) }, fixture().also { put64(it, 443, 1000) }, fixture().also { put64(it, 443, 121001) })
        for (i in 0 until 13) invalid += fixture().also { it.fill(0, 3 + i * 32, 3 + (i + 1) * 32) }
        for (bytes in invalid) assertThrows(IllegalArgumentException::class.java) { KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(bytes) }
    }

    @Test fun nativeUnsigned64EpochIsPreservedWithoutJvmSignedLongTruncation() {
        val projection = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(fixture().also { it.fill(0xff.toByte(), 427, 435) })
        assertEquals(BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE), projection.hardwareEpoch)
    }
}
