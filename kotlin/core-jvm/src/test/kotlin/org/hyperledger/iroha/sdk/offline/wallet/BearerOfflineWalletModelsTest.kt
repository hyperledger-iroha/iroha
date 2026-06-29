package org.hyperledger.iroha.sdk.offline.wallet

import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.offline.OfflineNote
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
    fun compactKeyCertificateRejectsNonCanonicalProfileFields() {
        assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(
                assertionScheme = "android-keymint-ecdsa-p256-usage-limit",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(
                assertionKeyAlgorithm = "ed25519",
            )
        }
        for (invalidPlatform in listOf("android", "android-keymint ", "Android-keymint", "ios-appattest-android")) {
            assertFailsWith<IllegalArgumentException> {
                compactKeyCertificate(platform = invalidPlatform)
            }
        }
    }

    @Test
    fun walletPolicyRejectsMalformedAmountsInsteadOfNormalizingToZero() {
        assertEquals("1.25", BearerOfflineWalletPolicy.normalizeAmountString("1.2500"))
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.normalizeAmountString("not-an-amount")
        }
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.balanceAmount("1..2")
        }
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.normalizeState(
                walletState(localBalance = "not-an-amount")
            )
        }
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.normalizeState(
                walletState(
                    assetTransferUsage = listOf(
                        OfflineAssetTransferUsage(
                            assetDefinitionId = "xor",
                            windowKind = "daily",
                            windowKey = "2026-06-29",
                            amount = "not-an-amount",
                        )
                    )
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.nextLocalStateHash(
                lineageId = "lineage-1",
                previousStateHash = "00",
                transferId = "transfer-1",
                direction = "outgoing",
                counterpartyLineageId = "lineage-2",
                amount = "not-an-amount",
                localRevision = 1,
                postBalance = "0",
                postLockedBalance = "0",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.challengeHashHex(
                lineageId = "lineage-1",
                transferId = "transfer-1",
                amount = "not-an-amount",
                direction = "incoming",
                counterpartyLineageId = "lineage-2",
                accountId = "alice@hbl.sbp",
            )
        }
    }

    @Test
    fun paymentTokenInputClaimRejectsMalformedAmountAndClaimHashDrift() {
        val claim = paymentTokenInputClaim()
        assertEquals("1.00", claim.amount)
        assertFailsWith<NumberFormatException> {
            paymentTokenInputClaim(amount = "not-an-amount")
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(noteCommitment = "zz")
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(claimHash = "f".repeat(64))
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(noteCommitment = "0x" + "01".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(noteCommitment = " " + "01".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(
                keyCertificatePayloadHash = ("ab".repeat(31) + "ad").uppercase()
            )
        }
        val computedClaimHash = validClaimHash()
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(claimHash = "")
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(claimHash = " $computedClaimHash")
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(claimHash = "0x$computedClaimHash")
        }
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(claimHash = computedClaimHash.uppercase())
        }

        val json = """
            {
              "domain":"${OfflineNote.ISSUED_CLAIM_DOMAIN}",
              "note_commitment":"${"01".repeat(32)}",
              "key_certificate_payload_hash":"${"02".repeat(31)}03",
              "asset_id":"${validAssetId()}",
              "amount":"1.00",
              "claim_hash":"${"ff".repeat(32)}"
            }
        """.trimIndent()
        assertFailsWith<IllegalArgumentException> {
            Json.decodeFromString<OfflinePaymentTokenInputClaim>(json)
        }
    }

    private fun compactKeyCertificate(
        platform: String = "android-keymint",
        assertionScheme: String = "android-keymint-ecdsa-p256-usage-limit-v1",
        assertionKeyAlgorithm: String = "ecdsa-p256-sha256",
    ): OfflineCompactKeyCertificate =
        OfflineCompactKeyCertificate(
            platform = platform,
            keyId = "attest-key",
            deviceId = "device-1",
            accountId = "alice@hbl.sbp",
            publicKey = base64(ByteArray(32) { 1 }),
            assertionScheme = assertionScheme,
            assertionKeyAlgorithm = assertionKeyAlgorithm,
            assertionPublicKey = base64(ByteArray(65) { 2 }),
            assertionUsageCountLimit = 1,
            issuerSignatureBase64 = base64(ByteArray(64) { 3 }),
        )

    private fun walletState(
        localBalance: String = "0",
        assetTransferUsage: List<OfflineAssetTransferUsage> = emptyList(),
    ): OfflineWalletState =
        OfflineWalletState(
            accountId = "alice@hbl.sbp",
            deviceId = "device-1",
            offlinePublicKey = base64(ByteArray(32) { 1 }),
            localBalance = localBalance,
            assetTransferUsage = assetTransferUsage,
        )

    private fun paymentTokenInputClaim(
        noteCommitment: String = "01".repeat(32),
        keyCertificatePayloadHash: String = "02".repeat(31) + "03",
        assetId: String = validAssetId(),
        amount: String = "1.00",
        claimHash: String? = null,
    ): OfflinePaymentTokenInputClaim =
        OfflinePaymentTokenInputClaim(
            domain = OfflineNote.ISSUED_CLAIM_DOMAIN,
            noteCommitment = noteCommitment,
            keyCertificatePayloadHash = keyCertificatePayloadHash,
            assetId = assetId,
            amount = amount,
            claimHash = claimHash,
        )

    private fun validAssetId(): String = "${validAssetDefinitionId()}#${validAccountId()}"

    private fun validAssetDefinitionId(): String {
        val bytes = ByteArray(16) { index -> (index + 1).toByte() }
        bytes[6] = ((bytes[6].toInt() and 0x0f) or 0x40).toByte()
        bytes[8] = ((bytes[8].toInt() and 0x3f) or 0x80).toByte()
        return AssetDefinitionIdEncoder.encodeFromBytes(bytes)
    }

    private fun validAccountId(): String = AccountAddress
        .fromAccount(ByteArray(32) { 0x2a.toByte() }, "ed25519")
        .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    private fun validClaimHash(): String = OfflineNote.IssuedClaim(
        domain = OfflineNote.ISSUED_CLAIM_DOMAIN,
        noteCommitment = ByteArray(32) { 0x01.toByte() },
        keyCertificatePayloadHash = ByteArray(32) { index ->
            if (index == 31) 0x03.toByte() else 0x02.toByte()
        },
        assetId = validAssetId(),
        amount = "1.00",
    ).claimHash().hexLower()

    private fun ByteArray.hexLower(): String =
        joinToString(separator = "") { byte ->
            String.format(java.util.Locale.ROOT, "%02x", byte.toInt() and 0xff)
        }

    private fun base64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)
}
