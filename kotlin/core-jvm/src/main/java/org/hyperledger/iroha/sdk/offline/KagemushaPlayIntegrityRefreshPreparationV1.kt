// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/**
 * Immutable public projection of the sole signed 514-byte Integrity refresh original.
 * Parsing admits no issuer, key, policy, enrolled owner or signing permission. Native owns
 * those checks and supplies the held original message before any platform or Google request.
 * Refresh is separate from the unchanged enrollment credential and financial epoch.
 */
class KagemushaPlayIntegrityRefreshPreparationV1 private constructor(raw: ByteArray) {
    private val original = raw.copyOf()
    @JvmField val policyEpoch: BigInteger = unsigned64(418)
    @JvmField val hardwareEpoch: BigInteger = unsigned64(426)
    @JvmField val issuedAtMs: BigInteger = unsigned64(434)
    @JvmField val expiresAtMs: BigInteger = unsigned64(442)

    init {
        require(original[0] == 1.toByte() && original[1] == 0.toByte())
        repeat(13) { require(field(it).any { byte -> byte != 0.toByte() }) { "Missing refresh selector" } }
        require(policyEpoch.signum() > 0 && hardwareEpoch.signum() > 0 && issuedAtMs.signum() > 0 &&
            expiresAtMs > issuedAtMs && expiresAtMs.subtract(issuedAtMs) <= BigInteger.valueOf(120_000)) {
            "Integrity refresh epoch or original interval differs"
        }
    }

    fun transportBytes(): ByteArray = original.copyOf()
    fun signatureBytes(): ByteArray = original.copyOfRange(BODY_BYTES, TRANSPORT_BYTES)
    /** Data correlation only; a Native-created holder supplies actual platform signing bytes. */
    fun canonicalSigningBytes(): ByteArray = message(CHALLENGE_DOMAIN, original.copyOfRange(0, BODY_BYTES))
    fun operationId(): ByteArray = sha(canonicalSigningBytes())
    fun playIntegrityRequestHash(): ByteArray = sha(REQUEST_DOMAIN + canonicalSigningBytes() + attestedKeyId())
    /** Original public model equation, without a key selection or signing capability. */
    fun possessionSigningBytes(): ByteArray = message(POSSESSION_DOMAIN, canonicalSigningBytes())

    fun credentialDigest(): ByteArray = field(0)
    fun attestedKeyId(): ByteArray = field(1)
    fun accountBinding(): ByteArray = field(2)
    fun networkId(): ByteArray = field(3)
    fun laneId(): ByteArray = field(4)
    fun releaseId(): ByteArray = field(5)
    fun hardwareProfileId(): ByteArray = field(6)
    fun suiteId(): ByteArray = field(7)
    fun trustPolicyDigest(): ByteArray = field(8)
    fun appAuthorityPolicyDigest(): ByteArray = field(9)
    fun playIntegrityPolicyDigest(): ByteArray = field(10)
    fun nonce(): ByteArray = field(11)
    fun originalEnrollmentChallengeDigest(): ByteArray = field(12)

    private fun field(index: Int): ByteArray = original.copyOfRange(2 + index * 32, 34 + index * 32)
    private fun unsigned64(offset: Int): BigInteger = BigInteger(1, original.copyOfRange(offset, offset + 8).reversedArray())
    private fun sha(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)
    private fun message(domain: ByteArray, body: ByteArray): ByteArray = domain +
        ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(body.size.toLong()).array() + body

    companion object {
        const val BODY_BYTES: Int = 450
        const val TRANSPORT_BYTES: Int = 514
        private val CHALLENGE_DOMAIN = "iroha:kagemusha:v1:play-integrity-refresh-challenge\u0000".toByteArray(Charsets.US_ASCII)
        private val REQUEST_DOMAIN = "iroha:kagemusha:v1:play-integrity-refresh-request\u0000".toByteArray(Charsets.US_ASCII)
        private val POSSESSION_DOMAIN = "iroha:kagemusha:v1:play-integrity-refresh-possession\u0000".toByteArray(Charsets.US_ASCII)
        @JvmStatic fun parseOriginal(bytes: ByteArray): KagemushaPlayIntegrityRefreshPreparationV1 {
            require(bytes.size == TRANSPORT_BYTES) { "Integrity refresh preparation must be exactly 514 bytes" }
            return KagemushaPlayIntegrityRefreshPreparationV1(bytes)
        }
    }
}
