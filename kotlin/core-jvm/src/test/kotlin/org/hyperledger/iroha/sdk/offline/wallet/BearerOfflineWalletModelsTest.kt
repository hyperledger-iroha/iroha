package org.hyperledger.iroha.sdk.offline.wallet

import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlinx.serialization.json.Json

class BearerOfflineWalletModelsTest {
    @Test
    fun compactKeyCertificateDefaultsToCanonicalAndroidAttestationScheme() {
        val certificate = Json.decodeFromString<OfflineCompactKeyCertificate>(
            """
            {
              "platform":"android-keymint",
              "key_id":"attest-key",
              "device_id":"device-1",
              "account_id":"alice@hbl.sbp",
              "public_key":"${base64(ByteArray(32) { 1 })}",
              "assertion_key_algorithm":"ecdsa-p256-sha256",
              "assertion_public_key":"${base64(ByteArray(65) { 2 })}",
              "assertion_usage_count_limit":1,
              "one_use":true,
              "issuer_signature_base64":"${base64(ByteArray(64) { 3 })}"
            }
            """.trimIndent()
        )

        assertEquals("android-keymint-ecdsa-p256-usage-limit-v1", certificate.assertionScheme)
    }

    @Test
    fun compactKeyCertificatePreservesExplicitLegacyAssertionScheme() {
        val certificate = OfflineCompactKeyCertificate(
            platform = "android-keymint",
            keyId = "attest-key",
            deviceId = "device-1",
            accountId = "alice@hbl.sbp",
            publicKey = base64(ByteArray(32) { 1 }),
            assertionScheme = "android-keymint-ecdsa-p256-usage-limit",
            assertionKeyAlgorithm = "ecdsa-p256-sha256",
            assertionPublicKey = base64(ByteArray(65) { 2 }),
            assertionUsageCountLimit = 1,
            issuerSignatureBase64 = base64(ByteArray(64) { 3 }),
        )

        assertEquals("android-keymint-ecdsa-p256-usage-limit", certificate.assertionScheme)
    }

    private fun base64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)
}
