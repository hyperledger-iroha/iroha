package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

/** Shared exact Android enrollment transcript. Selectors must be independently admitted by Core. */
object KagemushaAndroidAppAttestationChallengeV1 {
    @JvmField val ZERO_KEY_ID_HEX: String = "00".repeat(32)
    private val domain = "iroha:kagemusha:v1:app-device-attestation-challenge\u0000".toByteArray(Charsets.US_ASCII)
    @JvmStatic fun transcript(clientNonceHex: String, serverNonceHex: String, releaseIdHex: String,
        profileIdHex: String, laneIdHex: String): ByteArray {
        require(clientNonceHex != serverNonceHex)
        return domain + hex32(clientNonceHex) + hex32(serverNonceHex) + hex32(releaseIdHex) +
            hex32(profileIdHex) + ByteArray(32) + hex32(laneIdHex)
    }
    @JvmStatic fun digest(clientNonceHex: String, serverNonceHex: String, releaseIdHex: String,
        profileIdHex: String, laneIdHex: String): ByteArray = MessageDigest.getInstance("SHA-256")
        .digest(transcript(clientNonceHex, serverNonceHex, releaseIdHex, profileIdHex, laneIdHex))
    private fun hex32(value: String): ByteArray {
        require(value.matches(Regex("[0-9a-f]{64}")) && value.any { it != '0' })
        return value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
}
