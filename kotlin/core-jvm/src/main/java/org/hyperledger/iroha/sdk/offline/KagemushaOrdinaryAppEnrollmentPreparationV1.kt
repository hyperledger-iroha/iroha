package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.security.MessageDigest

/** Exact model-owned public preparation bytes. Parsing is never issuer or native admission.
 * The held native owner authenticates the complete original signer, selected policy and interval
 * before any platform key, Integrity request or possession signature may use this projection.
 */
class KagemushaOrdinaryAppEnrollmentPreparationV1 private constructor(raw: ByteArray) {
    private val original = raw.copyOf()
    @JvmField val platformClass: KagemushaHardwarePlatformClassV1 = when (original[2].toInt()) {
        1 -> KagemushaHardwarePlatformClassV1.ANDROID_KEYMINT
        2 -> KagemushaHardwarePlatformClassV1.APPLE_APP_ATTEST
        else -> throw IllegalArgumentException("Ordinary preparation platform differs")
    }
    @JvmField val policyEpoch: BigInteger = unsigned64(419)
    @JvmField val hardwareEpoch: BigInteger = unsigned64(427)
    @JvmField val issuedAtMs: BigInteger = unsigned64(435)
    @JvmField val expiresAtMs: BigInteger = unsigned64(443)

    init {
        require(original[0] == 1.toByte() && original[1] == 0.toByte())
        for (index in 0 until 13) require(field(index).any { it != 0.toByte() }) { "Missing ordinary preparation selector" }
        require(!clientNonce().contentEquals(serverNonce()))
        require(policyEpoch.signum() > 0 && hardwareEpoch.signum() > 0 && issuedAtMs.signum() > 0 && expiresAtMs > issuedAtMs &&
            expiresAtMs.subtract(issuedAtMs) <= BigInteger.valueOf(120_000)) { "Ordinary preparation epoch or interval differs" }
    }

    fun transportBytes(): ByteArray = original.copyOf()
    fun canonicalSigningBytes(): ByteArray = DOMAIN + byteArrayOf(0xc3.toByte(), 1, 0, 0, 0, 0, 0, 0) + original.copyOfRange(0, BODY_BYTES)
    fun signatureBytes(): ByteArray = original.copyOfRange(BODY_BYTES, TRANSPORT_BYTES)
    fun attestationChallenge(): ByteArray = sha(canonicalSigningBytes())
    fun playIntegrityRequestHash(attestedKeyId: ByteArray): ByteArray {
        require(attestedKeyId.size == 32 && attestedKeyId.any { it != 0.toByte() })
        return sha(INTEGRITY_DOMAIN + canonicalSigningBytes() + attestedKeyId.copyOf())
    }

    fun enrollmentId(): ByteArray = field(0)
    fun clientNonce(): ByteArray = field(1)
    fun serverNonce(): ByteArray = field(2)
    fun accountBinding(): ByteArray = field(3)
    fun networkId(): ByteArray = field(4)
    fun laneId(): ByteArray = field(5)
    fun releaseId(): ByteArray = field(6)
    fun hardwareProfileId(): ByteArray = field(7)
    fun suiteId(): ByteArray = field(8)
    fun trustPolicyDigest(): ByteArray = field(9)
    fun appAuthorityPolicyDigest(): ByteArray = field(10)
    fun financialAuthorityCommitment(): ByteArray = field(11)
    fun issuerPolicyDigest(): ByteArray = field(12)

    private fun field(index: Int): ByteArray = original.copyOfRange(3 + index * 32, 3 + (index + 1) * 32)
    private fun unsigned64(offset: Int): BigInteger = BigInteger(1, original.copyOfRange(offset, offset + 8).reversedArray())
    private fun sha(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

    companion object {
        const val BODY_BYTES: Int = 451
        const val TRANSPORT_BYTES: Int = 515
        private val DOMAIN = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)
        private val INTEGRITY_DOMAIN = "iroha:kagemusha:v1:play-integrity-enrollment\u0000".toByteArray(Charsets.US_ASCII)
        @JvmStatic fun parseOriginal(bytes: ByteArray): KagemushaOrdinaryAppEnrollmentPreparationV1 {
            require(bytes.size == TRANSPORT_BYTES) { "Ordinary preparation transport must be exactly 515 bytes" }
            return KagemushaOrdinaryAppEnrollmentPreparationV1(bytes)
        }
    }
}
