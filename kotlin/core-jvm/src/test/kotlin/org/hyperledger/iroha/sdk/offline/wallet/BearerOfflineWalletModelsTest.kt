package org.hyperledger.iroha.sdk.offline.wallet

import java.util.Base64
import java.time.Instant
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.offline.OfflineNote
import org.hyperledger.iroha.sdk.offline.AttestedOfflineNote
import kotlinx.serialization.SerializationException
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
        assertEquals(1, certificate.assertionUsageCountLimit)
    }

    @Test
    fun compactKeyCertificateRejectsNonCanonicalProfileFields() {
        assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(version = 2)
        }
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
        assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(assertionUsageCountLimit = null)
        }
        assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(assertionUsageCountLimit = 2)
        }
        assertEquals(
            AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_SCHEME,
            compactKeyCertificate(
                platform = AttestedOfflineNote.IOS_APP_ATTEST_PLATFORM,
                assertionScheme = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_SCHEME,
                assertionKeyAlgorithm = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM,
                assertionUsageCountLimit = null,
            ).assertionScheme,
        )
        assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(
                platform = AttestedOfflineNote.IOS_APP_ATTEST_PLATFORM,
                assertionScheme = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_SCHEME,
                assertionKeyAlgorithm = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM,
                assertionUsageCountLimit = 1,
            )
        }
        val retiredAliasError = assertFailsWith<IllegalArgumentException> {
            Json.decodeFromString<OfflineCompactKeyCertificate>(
                """
                {
                  "platform":"android-keymint",
                  "key_id":"attest-key",
                  "device_id":"device-1",
                  "account_id":"alice@hbl.sbp",
                  "public_key":"${base64(ByteArray(32) { 1 })}",
                  "assertion_scheme":"android-keymint-ecdsa-p256-usage-limit-v1",
                  "assertion_key_algorithm":"ecdsa-p256-sha256",
                  "assertion_public_key":"${base64(ByteArray(65) { 2 })}",
                  "app_attest_public_key_base64":"${base64(ByteArray(65) { 4 })}",
                  "issuer_signature_base64":"${base64(ByteArray(64) { 3 })}"
                }
                """.trimIndent()
            )
        }
        assertEquals(
            "app_attest_public_key_base64 is retired; use assertion_public_key",
            retiredAliasError.message,
        )
    }

    @Test
    fun compactKeyCertificateRejectsNonCanonicalBase64Fields() {
        val canonicalPublicKey = base64(ByteArray(32) { 1 })
        val canonicalAssertionPublicKey = base64(ByteArray(65) { 2 })
        val canonicalIssuerSignature = base64(ByteArray(64) { 3 })

        for (invalidPublicKey in listOf("", " $canonicalPublicKey", "@@@", "_w==", canonicalPublicKey.dropLast(1))) {
            val error = assertFailsWith<IllegalArgumentException> {
                compactKeyCertificate(publicKey = invalidPublicKey)
            }
            assertEquals("public_key must be canonical base64", error.message)
        }
        for (invalidAssertionPublicKey in listOf(
            "",
            " $canonicalAssertionPublicKey",
            "@@@",
            "_w==",
            canonicalAssertionPublicKey.dropLast(1),
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                compactKeyCertificate(assertionPublicKey = invalidAssertionPublicKey)
            }
            assertEquals("assertion_public_key must be canonical base64", error.message)
        }
        val shortPublicKey = assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(publicKey = base64(ByteArray(31) { 1 }))
        }
        assertEquals("public_key must be 32 bytes", shortPublicKey.message)
        val shortAssertionPublicKey = assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(assertionPublicKey = base64(ByteArray(64) { 2 }))
        }
        assertEquals("assertion_public_key must be 65 bytes", shortAssertionPublicKey.message)
        for (invalidIssuerSignature in listOf("", " $canonicalIssuerSignature", "@@@", "_w==")) {
            val error = assertFailsWith<IllegalArgumentException> {
                compactKeyCertificate(issuerSignatureBase64 = invalidIssuerSignature)
            }
            assertEquals("issuer_signature_base64 must be canonical base64", error.message)
        }
        val shortSignature = assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(issuerSignatureBase64 = base64(ByteArray(63) { 3 }))
        }
        assertEquals("issuer_signature_base64 must be 64 bytes", shortSignature.message)
        val payloadError = assertFailsWith<IllegalArgumentException> {
            compactKeyCertificate(issuerSignaturePayloadBase64 = " $canonicalIssuerSignature")
        }
        assertEquals("issuer_signature_payload_base64 must be canonical base64", payloadError.message)

        val json = """
            {
              "platform":"android-keymint",
              "key_id":"attest-key",
              "device_id":"device-1",
              "account_id":"alice@hbl.sbp",
              "public_key":"$canonicalPublicKey",
              "assertion_scheme":"android-keymint-ecdsa-p256-usage-limit-v1",
              "assertion_key_algorithm":"ecdsa-p256-sha256",
              "assertion_usage_count_limit":1,
              "assertion_public_key":"${base64(ByteArray(65) { 0xff.toByte() }).replace('/', '_')}",
              "issuer_signature_base64":"$canonicalIssuerSignature"
            }
        """.trimIndent()
        val jsonError = assertFailsWith<IllegalArgumentException> {
            Json.decodeFromString<OfflineCompactKeyCertificate>(json)
        }
        assertEquals("assertion_public_key must be canonical base64", jsonError.message)
    }

    @Test
    fun recursiveProofRejectsNonCanonicalHashAndProofBytes() {
        val canonicalProofBytes = base64(byteArrayOf(4, 5))
        assertEquals(canonicalProofBytes, recursiveProof(proofBytesBase64 = canonicalProofBytes).proofBytesBase64)

        for (invalidHash in listOf("", "0x" + "01".repeat(32), " " + "01".repeat(32), "AB".repeat(32))) {
            val error = assertFailsWith<IllegalArgumentException> {
                recursiveProof(publicInputsHashHex = invalidHash)
            }
            assertEquals("public_inputs_hash_hex must be 32-byte lowercase hex", error.message)
        }
        for (invalidProofBytes in listOf("", " $canonicalProofBytes", "@@@", "_w==", canonicalProofBytes.dropLast(1))) {
            val error = assertFailsWith<IllegalArgumentException> {
                recursiveProof(proofBytesBase64 = invalidProofBytes)
            }
            assertEquals("proof_bytes_base64 must be canonical base64", error.message)
        }

        val json = """
            {
              "verifier_key_backend":"halo2/ipa",
              "verifier_key_id":"${OfflineNote.RECURSIVE_VERIFIER_NAME}",
              "proof_backend":"halo2/ipa",
              "public_inputs_hash_hex":"${"01".repeat(32)}",
              "proof_bytes_base64":" ${base64(ByteArray(2) { 7 })}"
            }
        """.trimIndent()
        val error = assertFailsWith<IllegalArgumentException> {
            Json.decodeFromString<OfflineRecursiveProof>(json)
        }
        assertEquals("proof_bytes_base64 must be canonical base64", error.message)
    }

    @Test
    fun recursiveProofRejectsRetiredVerifierMetadataAliases() {
        val canonical = recursiveProof()
        assertEquals(OfflineNote.RECURSIVE_BACKEND, canonical.verifierKeyBackend)
        assertEquals(OfflineNote.RECURSIVE_VERIFIER_NAME, canonical.verifierKeyId)
        assertEquals(OfflineNote.RECURSIVE_BACKEND, canonical.proofBackend)

        for (invalidBackend in listOf("", " ${OfflineNote.RECURSIVE_BACKEND}", "halo2/kzg", "HALO2/IPA")) {
            val error = assertFailsWith<IllegalArgumentException> {
                recursiveProof(verifierKeyBackend = invalidBackend)
            }
            assertEquals("verifier_key_backend must be ${OfflineNote.RECURSIVE_BACKEND}", error.message)
        }
        for (invalidVerifierKeyId in listOf(
            "",
            "${OfflineNote.RECURSIVE_VERIFIER_NAME} ",
            "${OfflineNote.RECURSIVE_BACKEND}:${OfflineNote.RECURSIVE_VERIFIER_NAME}",
            "vk-1",
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                recursiveProof(verifierKeyId = invalidVerifierKeyId)
            }
            assertEquals("verifier_key_id must be ${OfflineNote.RECURSIVE_VERIFIER_NAME}", error.message)
        }
        for (invalidProofBackend in listOf("", " ${OfflineNote.RECURSIVE_BACKEND}", "halo2/kzg", "HALO2/IPA")) {
            val error = assertFailsWith<IllegalArgumentException> {
                recursiveProof(proofBackend = invalidProofBackend)
            }
            assertEquals("proof_backend must be ${OfflineNote.RECURSIVE_BACKEND}", error.message)
        }

        val objectVerifierJson = """
            {
              "verifier_key_backend":"${OfflineNote.RECURSIVE_BACKEND}",
              "verifier_key_id":{"name":"${OfflineNote.RECURSIVE_VERIFIER_NAME}"},
              "proof_backend":"${OfflineNote.RECURSIVE_BACKEND}",
              "public_inputs_hash_hex":"${"01".repeat(32)}",
              "proof_bytes_base64":"${base64(ByteArray(2) { 7 })}"
            }
        """.trimIndent()
        val objectError = assertFailsWith<SerializationException> {
            Json.decodeFromString<OfflineRecursiveProof>(objectVerifierJson)
        }
        assertEquals(true, objectError.message?.contains("verifier_key_id must be a string"))

        val prefixedVerifierJson = """
            {
              "verifier_key_backend":"${OfflineNote.RECURSIVE_BACKEND}",
              "verifier_key_id":"${OfflineNote.RECURSIVE_BACKEND}:${OfflineNote.RECURSIVE_VERIFIER_NAME}",
              "proof_backend":"${OfflineNote.RECURSIVE_BACKEND}",
              "public_inputs_hash_hex":"${"01".repeat(32)}",
              "proof_bytes_base64":"${base64(ByteArray(2) { 7 })}"
            }
        """.trimIndent()
        val prefixedError = assertFailsWith<IllegalArgumentException> {
            Json.decodeFromString<OfflineRecursiveProof>(prefixedVerifierJson)
        }
        assertEquals("verifier_key_id must be ${OfflineNote.RECURSIVE_VERIFIER_NAME}", prefixedError.message)
    }

    @Test
    fun offlineDeviceBindingRejectsRetiredAssertionPublicKeyAliases() {
        for (retiredKey in listOf("device_public_key", "app_attest_public_key_base64")) {
            val json = """
                {
                  "platform":"android",
                  "attestation_key_id":"attest-key",
                  "device_id":"device-1",
                  "offline_public_key":"${base64(ByteArray(32) { 1 })}",
                  "attestation_report_base64":"${base64(ByteArray(64) { 2 })}",
                  "$retiredKey":"${base64(ByteArray(65) { 3 })}",
                  "assertion_public_key":"${base64(ByteArray(65) { 4 })}"
                }
            """.trimIndent()
            val error = assertFailsWith<IllegalArgumentException> {
                Json.decodeFromString<OfflineDeviceBinding>(json)
            }
            assertEquals("$retiredKey is retired; use assertion_public_key", error.message)
        }
        for (invalidPlatform in listOf("android-keymint", "ios-appattest", "ios-app-attest", "android-keymint ", "Android")) {
            val error = assertFailsWith<IllegalArgumentException> {
                deviceBinding(platform = invalidPlatform)
            }
            assertEquals("platform must be a supported first-release value", error.message)
        }
    }

    @Test
    fun attestationReceiptRejectsNonCanonicalProfileAndEncodingFields() {
        assertEquals(1, attestationReceipt().assertionUsageCountLimit)
        assertEquals(
            AttestedOfflineNote.IOS_APP_ATTEST_PLATFORM,
            attestationReceipt(
                platform = AttestedOfflineNote.IOS_APP_ATTEST_PLATFORM,
                assertionScheme = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_SCHEME,
                assertionKeyAlgorithm = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM,
                assertionUsageCountLimit = null,
            ).platform,
        )

        assertEquals(
            "version must be 1",
            assertFailsWith<IllegalArgumentException> { attestationReceipt(version = 2) }.message,
        )
        assertEquals(
            "account_id must be an exact non-empty string",
            assertFailsWith<IllegalArgumentException> { attestationReceipt(accountId = " alice@hbl.sbp") }.message,
        )
        for (invalidPlatform in listOf("android", "ios-app-attest", "android-keymint ")) {
            assertEquals(
                "platform must be a supported first-release value",
                assertFailsWith<IllegalArgumentException> {
                    attestationReceipt(platform = invalidPlatform)
                }.message,
            )
        }
        assertEquals(
            "assertion_scheme must be ${AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_SCHEME}",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(assertionScheme = "android-keymint-ecdsa-p256-usage-limit")
            }.message,
        )
        assertEquals(
            "assertion_key_algorithm must be ${AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM}",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(assertionKeyAlgorithm = "ed25519")
            }.message,
        )
        assertEquals(
            "assertion_usage_count_limit must be 1",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(assertionUsageCountLimit = null)
            }.message,
        )
        assertEquals(
            "assertion_usage_count_limit must be absent",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(
                    platform = AttestedOfflineNote.IOS_APP_ATTEST_PLATFORM,
                    assertionScheme = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_SCHEME,
                    assertionKeyAlgorithm = AttestedOfflineNote.IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM,
                    assertionUsageCountLimit = 1,
                )
            }.message,
        )
        assertEquals(
            "hardware_one_use must be true",
            assertFailsWith<IllegalArgumentException> { attestationReceipt(hardwareOneUse = false) }.message,
        )
        assertEquals(
            "offline_public_key_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(offlinePublicKeyBase64 = " ${base64(ByteArray(32) { 1 })}")
            }.message,
        )
        assertEquals(
            "offline_public_key_base64 must be 32 bytes",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(offlinePublicKeyBase64 = base64(ByteArray(31) { 1 }))
            }.message,
        )
        assertEquals(
            "assertion_public_key_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(assertionPublicKeyBase64 = "_w==")
            }.message,
        )
        assertEquals(
            "assertion_public_key_base64 must be 65 bytes",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(assertionPublicKeyBase64 = base64(ByteArray(64) { 2 }))
            }.message,
        )
        for (invalidHash in listOf("", "AB".repeat(32), "01".repeat(31))) {
            assertEquals(
                "attestation_report_hash_hex must be 32-byte lowercase hex",
                assertFailsWith<IllegalArgumentException> {
                    attestationReceipt(attestationReportHashHex = invalidHash)
                }.message,
            )
        }
        assertEquals(
            "receipt validity window must be increasing",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(issuedAtMs = 20, expiresAtMs = 20)
            }.message,
        )
        assertEquals(
            "signature_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(signatureBase64 = " ${base64(ByteArray(64) { 3 })}")
            }.message,
        )
        assertEquals(
            "signature_base64 must be 64 bytes",
            assertFailsWith<IllegalArgumentException> {
                attestationReceipt(signatureBase64 = base64(ByteArray(63) { 3 }))
            }.message,
        )

        val json = """
            {
              "version":1,
              "platform":"android-keymint",
              "account_id":"alice@hbl.sbp",
              "device_id":"device-1",
              "offline_public_key_base64":"${base64(ByteArray(32) { 1 })}",
              "assertion_public_key_base64":"${base64(ByteArray(65) { 2 })}",
              "assertion_scheme":"android-keymint-ecdsa-p256-usage-limit-v1",
              "assertion_key_algorithm":"ecdsa-p256-sha256",
              "assertion_usage_count_limit":1,
              "attestation_key_id":"attest-key",
              "hardware_one_use":true,
              "attestation_report_hash_hex":"${"01".repeat(32)}",
              "issued_at_ms":10,
              "expires_at_ms":20,
              "signature_base64":"_w=="
            }
        """.trimIndent()
        assertEquals(
            "signature_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                Json.decodeFromString<OfflineAttestationReceipt>(json)
            }.message,
        )
    }

    @Test
    fun deviceProofRejectsNonCanonicalPlatformHashAndAssertion() {
        assertEquals("android", deviceProof().platform)

        for (invalidPlatform in listOf("android-keymint", "ios-appattest", "ios-app-attest", "android-keymint ", "Android")) {
            assertEquals(
                "platform must be a supported first-release value",
                assertFailsWith<IllegalArgumentException> {
                    deviceProof(platform = invalidPlatform)
                }.message,
            )
        }
        assertEquals(
            "attestation_key_id must be an exact non-empty string",
            assertFailsWith<IllegalArgumentException> { deviceProof(attestationKeyId = " attest-key") }.message,
        )
        for (invalidHash in listOf("", "AB".repeat(32), "0x" + "01".repeat(32))) {
            assertEquals(
                "challenge_hash_hex must be 32-byte lowercase hex",
                assertFailsWith<IllegalArgumentException> {
                    deviceProof(challengeHashHex = invalidHash)
                }.message,
            )
        }
        for (invalidAssertion in listOf("", " ${base64(ByteArray(8) { 7 })}", "@@@", "_w==")) {
            assertEquals(
                "assertion_base64 must be canonical base64",
                assertFailsWith<IllegalArgumentException> {
                    deviceProof(assertionBase64 = invalidAssertion)
                }.message,
            )
        }
        assertEquals(
            "counter must be non-negative",
            assertFailsWith<IllegalArgumentException> { deviceProof(counter = -1) }.message,
        )

        val json = """
            {
              "platform":"android",
              "attestation_key_id":"attest-key",
              "challenge_hash_hex":"${"01".repeat(32)}",
              "assertion_base64":"_w=="
            }
        """.trimIndent()
        assertEquals(
            "assertion_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                Json.decodeFromString<OfflineDeviceProof>(json)
            }.message,
        )
    }

    @Test
    fun signedWalletPayloadsRejectMalformedHashesAmountsAndSignatures() {
        assertEquals("auth-1", spendAuthorization().authorizationId)
        assertEquals(
            "max_balance must be a non-negative amount",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(policyMaxBalance = "not-an-amount")
            }.message,
        )
        assertEquals(
            "authorization validity window must be increasing",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(issuedAtMs = 10, refreshAtMs = 9)
            }.message,
        )
        assertEquals(
            "issuer_signature_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(issuerSignatureBase64 = " ${base64(ByteArray(64) { 3 })}")
            }.message,
        )
        assertEquals(
            "issuer_signature_base64 must be 64 bytes",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(issuerSignatureBase64 = base64(ByteArray(63) { 3 }))
            }.message,
        )

        assertEquals("lineage-1", cashStatePayload().lineageId)
        assertEquals(
            "balance must be a non-negative amount",
            assertFailsWith<IllegalArgumentException> {
                cashStatePayload(balance = "not-an-amount")
            }.message,
        )
        assertEquals(
            "server_revision must be non-negative",
            assertFailsWith<IllegalArgumentException> {
                cashStatePayload(serverRevision = -1)
            }.message,
        )
        for (invalidHash in listOf("", "AB".repeat(32), "01".repeat(31))) {
            assertEquals(
                "server_state_hash must be 32-byte lowercase hex",
                assertFailsWith<IllegalArgumentException> {
                    cashStatePayload(serverStateHash = invalidHash)
                }.message,
            )
        }

        assertEquals("limit-asset#xor", assetSendLimit().assetDefinitionId)
        assertEquals(
            "daily_send_limit must be a non-negative amount",
            assertFailsWith<IllegalArgumentException> {
                assetSendLimit(dailySendLimit = "bad")
            }.message,
        )
        assertEquals(
            "revocation bundle validity window must be increasing",
            assertFailsWith<IllegalArgumentException> {
                revocationBundle(issuedAtMs = 20, expiresAtMs = 20)
            }.message,
        )
        assertEquals(
            "verdict_ids must be exact non-empty strings",
            assertFailsWith<IllegalArgumentException> {
                revocationBundle(verdictIds = listOf("verdict-1", ""))
            }.message,
        )

        assertEquals("transfer-1", transferReceipt().transferId)
        assertEquals(
            "version must be 1",
            assertFailsWith<IllegalArgumentException> {
                transferReceipt(version = 2)
            }.message,
        )
        assertEquals(
            "direction must be incoming or outgoing",
            assertFailsWith<IllegalArgumentException> {
                transferReceipt(direction = "sent")
            }.message,
        )
        assertEquals(
            "amount must be a non-negative amount",
            assertFailsWith<IllegalArgumentException> {
                transferReceipt(amount = "bad")
            }.message,
        )
        for (invalidHash in listOf("", "AB".repeat(32), "01".repeat(31))) {
            assertEquals(
                "pre_state_hash must be 32-byte lowercase hex",
                assertFailsWith<IllegalArgumentException> {
                    transferReceipt(preStateHash = invalidHash)
                }.message,
            )
        }
        assertEquals(
            "sender_signature_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                transferReceipt(senderSignatureBase64 = "_w==")
            }.message,
        )
        assertEquals(
            "sender_signature_base64 must be 64 bytes",
            assertFailsWith<IllegalArgumentException> {
                transferReceipt(senderSignatureBase64 = base64(ByteArray(63) { 9 }))
            }.message,
        )

        val json = """
            {
              "version":1,
              "transfer_id":"transfer-1",
              "direction":"outgoing",
              "lineage_id":"lineage-1",
              "account_id":"alice@hbl.sbp",
              "device_id":"device-1",
              "offline_public_key":"${base64(ByteArray(32) { 1 })}",
              "pre_balance":"10",
              "post_balance":"9",
              "pre_locked_balance":"0",
              "post_locked_balance":"0",
              "pre_state_hash":"${"01".repeat(32)}",
              "post_state_hash":"${"02".repeat(32)}",
              "local_revision":1,
              "counterparty_lineage_id":"lineage-2",
              "counterparty_account_id":"bob@hbl.sbp",
              "counterparty_device_id":"device-2",
              "counterparty_offline_public_key":"${base64(ByteArray(32) { 2 })}",
              "amount":"1",
              "device_proof":{
                "platform":"android",
                "attestation_key_id":"attest-key",
                "challenge_hash_hex":"${"03".repeat(32)}",
                "assertion_base64":"${base64(ByteArray(8) { 7 })}"
              },
              "sender_signature_base64":"_w==",
              "created_at_ms":10
            }
        """.trimIndent()
        assertEquals(
            "sender_signature_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                Json.decodeFromString<OfflineTransferReceipt>(json)
            }.message,
        )
    }

    @Test
    fun walletPolicyRejectsMalformedAmountsInsteadOfNormalizingToZero() {
        assertEquals("1.25", BearerOfflineWalletPolicy.normalizeAmountString("1.2500"))
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.normalizeAmountString("not-an-amount")
        }
        assertFailsWith<IllegalArgumentException> {
            BearerOfflineWalletPolicy.normalizeAmountString(" 1")
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
        assertEquals(
            "previous_state_hash must be empty or 32-byte lowercase hex",
            assertFailsWith<IllegalArgumentException> {
                BearerOfflineWalletPolicy.nextLocalStateHash(
                    lineageId = "lineage-1",
                    previousStateHash = "AB".repeat(32),
                    transferId = "transfer-1",
                    direction = "outgoing",
                    counterpartyLineageId = "lineage-2",
                    amount = "1",
                    localRevision = 1,
                    postBalance = "0",
                    postLockedBalance = "0",
                )
            }.message,
        )
        assertEquals(
            "direction must be incoming or outgoing",
            assertFailsWith<IllegalArgumentException> {
                BearerOfflineWalletPolicy.nextLocalStateHash(
                    lineageId = "lineage-1",
                    previousStateHash = "01".repeat(32),
                    transferId = "transfer-1",
                    direction = "Outgoing",
                    counterpartyLineageId = "lineage-2",
                    amount = "1",
                    localRevision = 1,
                    postBalance = "0",
                    postLockedBalance = "0",
                )
            }.message,
        )
        assertEquals(
            "local_revision must be non-negative",
            assertFailsWith<IllegalArgumentException> {
                BearerOfflineWalletPolicy.nextLocalStateHash(
                    lineageId = "lineage-1",
                    previousStateHash = "01".repeat(32),
                    transferId = "transfer-1",
                    direction = "outgoing",
                    counterpartyLineageId = "lineage-2",
                    amount = "1",
                    localRevision = -1,
                    postBalance = "0",
                    postLockedBalance = "0",
                )
            }.message,
        )
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
        assertEquals(
            "direction must be incoming or outgoing",
            assertFailsWith<IllegalArgumentException> {
                BearerOfflineWalletPolicy.challengeHashHex(
                    lineageId = "lineage-1",
                    transferId = "transfer-1",
                    amount = "1",
                    direction = "Incoming",
                    counterpartyLineageId = "lineage-2",
                    accountId = "alice@hbl.sbp",
                )
            }.message,
        )
        assertEquals(
            "account_id must be an exact non-empty string",
            assertFailsWith<IllegalArgumentException> {
                BearerOfflineWalletPolicy.challengeHashHex(
                    lineageId = "lineage-1",
                    transferId = "transfer-1",
                    amount = "1",
                    direction = "incoming",
                    counterpartyLineageId = "lineage-2",
                    accountId = " alice@hbl.sbp",
                )
            }.message,
        )
    }

    @Test
    fun walletStateRejectsPermissiveNormalizationDrift() {
        assertEquals("0", BearerOfflineWalletPolicy.normalizeState(walletState()).localBalance)
        assertEquals(
            "local_state_hash must be empty or 32-byte lowercase hex",
            assertFailsWith<IllegalArgumentException> {
                walletState(localStateHash = "AB".repeat(32))
            }.message,
        )
        assertEquals(
            "source_nullifiers must be 32-byte lowercase hex strings",
            assertFailsWith<IllegalArgumentException> {
                walletState(sourceNullifiers = listOf("AB".repeat(32)))
            }.message,
        )
        assertEquals(
            "window_kind must be daily or monthly",
            assertFailsWith<IllegalArgumentException> {
                OfflineAssetTransferUsage(
                    assetDefinitionId = "xor",
                    windowKind = "Daily",
                    windowKey = "2026-06-30",
                    amount = "1",
                )
            }.message,
        )
        assertEquals(
            "amount must be a non-negative amount",
            assertFailsWith<IllegalArgumentException> {
                OfflineAssetTransferUsage(
                    assetDefinitionId = "xor",
                    windowKind = "daily",
                    windowKey = "2026-06-30",
                    amount = " 1",
                )
            }.message,
        )
        assertEquals(
            "remaining_capacity must match available_key_ids size",
            assertFailsWith<IllegalArgumentException> {
                OfflineOneUseKeyPoolState(
                    capacity = 2,
                    remainingCapacity = 0,
                    availableKeyIds = listOf("key-1"),
                )
            }.message,
        )
        assertEquals(
            "note_secret_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                noteRecord(noteSecretBase64 = "_w==")
            }.message,
        )
        assertEquals(
            "chain_tx_hash must be 32-byte lowercase hex",
            assertFailsWith<IllegalArgumentException> {
                OfflineCashMutationHistoryEntry(
                    operationId = "op-1",
                    kind = "load",
                    amount = "1",
                    chainTxHash = "AB".repeat(32),
                    entryHash = "01".repeat(32),
                    blockHeight = 1,
                    createdAtMs = 1,
                )
            }.message,
        )
        assertEquals(
            "payload must be an exact non-empty string",
            assertFailsWith<IllegalArgumentException> {
                OfflineTransferJournalEntry(
                    transferId = "transfer-1",
                    direction = OfflineTransferDirection.OUTGOING,
                    counterpartyAccountId = "bob@hbl.sbp",
                    amount = "1",
                    createdAtMs = 1,
                    payload = " payload",
                )
            }.message,
        )
        assertEquals(
            "pending audit receipt must carry a payment token or settlement batch",
            assertFailsWith<IllegalArgumentException> {
                OfflinePendingAuditReceipt(
                    receiptId = "receipt-1",
                    tokenId = "token-1",
                    createdAtMs = 1,
                )
            }.message,
        )
        assertEquals(
            "token_id must be an exact non-empty string",
            assertFailsWith<IllegalArgumentException> {
                OfflinePendingOutboxEntry(
                    tokenId = "",
                    recipientAccountId = "bob@hbl.sbp",
                    assetDefinitionId = validAssetDefinitionId(),
                    amount = "1",
                    payload = "payload",
                    createdAtMs = 1,
                )
            }.message,
        )

        val revocation = revocationBundle(verdictIds = listOf("VERDICT-1"))
        assertEquals(false, BearerOfflineWalletPolicy.isRevoked(spendAuthorization(), revocation))
        assertEquals(false, BearerOfflineWalletPolicy.isAccountBlacklisted(" mallory@hbl.sbp", revocation))
        assertNull(BearerOfflineWalletPolicy.assetSendLimit(" limit-asset#xor", revocation, Instant.ofEpochMilli(11)))

        val json = """
            {
              "schema_version":5,
              "account_id":"alice@hbl.sbp",
              "device_id":"device-1",
              "offline_public_key":"${base64(ByteArray(32) { 1 })}",
              "local_state_hash":"${"AB".repeat(32)}"
            }
        """.trimIndent()
        assertEquals(
            "local_state_hash must be empty or 32-byte lowercase hex",
            assertFailsWith<IllegalArgumentException> {
                Json.decodeFromString<OfflineWalletState>(json)
            }.message,
        )
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
        val canonicalAssetId = validAssetId()
        val nonCanonicalAssetId = "${validAssetDefinitionId()}#alice@wonderland"
        assertEquals(canonicalAssetId, OfflineNote.canonicalAssetId(canonicalAssetId))
        assertFailsWith<IllegalArgumentException> {
            paymentTokenInputClaim(assetId = nonCanonicalAssetId)
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

    @Test
    fun walletPolicyRejectsNonPositiveSpendSelectionMaxInputs() {
        val state = walletState(
            noteRecords = listOf(
                noteRecord(noteId = "note-1", amount = "1", updatedAtMs = 1),
                noteRecord(noteId = "note-2", amount = "2", updatedAtMs = 2),
            )
        )

        for (invalidMaxInputs in listOf(0, -1, Int.MIN_VALUE)) {
            val error = assertFailsWith<IllegalArgumentException> {
                BearerOfflineWalletPolicy.selectSpendableNoteRecordsForBalance(
                    state,
                    "1",
                    maxInputs = invalidMaxInputs
                )
            }
            assertEquals("maxInputs must be positive", error.message)
        }

        assertNull(
            BearerOfflineWalletPolicy.selectSpendableNoteRecordsForBalance(
                state,
                "3",
                maxInputs = 1
            )
        )
        assertEquals(
            listOf("note-1", "note-2"),
            BearerOfflineWalletPolicy.selectSpendableNoteRecordsForBalance(
                state,
                "3",
                maxInputs = 2
            )?.map { it.noteId }
        )
    }

    private fun compactKeyCertificate(
        version: Int = 1,
        platform: String = "android-keymint",
        assertionScheme: String = "android-keymint-ecdsa-p256-usage-limit-v1",
        assertionKeyAlgorithm: String = "ecdsa-p256-sha256",
        assertionUsageCountLimit: Int? = 1,
        publicKey: String = base64(ByteArray(32) { 1 }),
        assertionPublicKey: String = base64(ByteArray(65) { 2 }),
        issuerSignatureBase64: String = base64(ByteArray(64) { 3 }),
        issuerSignaturePayloadBase64: String? = null,
    ): OfflineCompactKeyCertificate =
        OfflineCompactKeyCertificate(
            version = version,
            platform = platform,
            keyId = "attest-key",
            deviceId = "device-1",
            accountId = "alice@hbl.sbp",
            publicKey = publicKey,
            assertionScheme = assertionScheme,
            assertionKeyAlgorithm = assertionKeyAlgorithm,
            assertionPublicKey = assertionPublicKey,
            assertionUsageCountLimit = assertionUsageCountLimit,
            issuerSignatureBase64 = issuerSignatureBase64,
            issuerSignaturePayloadBase64 = issuerSignaturePayloadBase64,
        )

    private fun attestationReceipt(
        version: Long = 1,
        platform: String = AttestedOfflineNote.ANDROID_KEYMINT_PLATFORM,
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKeyBase64: String = base64(ByteArray(32) { 1 }),
        assertionPublicKeyBase64: String = base64(ByteArray(65) { 2 }),
        assertionScheme: String = AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_SCHEME,
        assertionKeyAlgorithm: String = AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
        assertionUsageCountLimit: Int? = 1,
        attestationKeyId: String = "attest-key",
        hardwareOneUse: Boolean = true,
        attestationReportHashHex: String = "01".repeat(32),
        issuedAtMs: Long = 10,
        expiresAtMs: Long = 20,
        signatureBase64: String = base64(ByteArray(64) { 3 }),
    ): OfflineAttestationReceipt =
        OfflineAttestationReceipt(
            version = version,
            platform = platform,
            accountId = accountId,
            deviceId = deviceId,
            offlinePublicKeyBase64 = offlinePublicKeyBase64,
            assertionPublicKeyBase64 = assertionPublicKeyBase64,
            assertionScheme = assertionScheme,
            assertionKeyAlgorithm = assertionKeyAlgorithm,
            assertionUsageCountLimit = assertionUsageCountLimit,
            attestationKeyId = attestationKeyId,
            hardwareOneUse = hardwareOneUse,
            attestationReportHashHex = attestationReportHashHex,
            issuedAtMs = issuedAtMs,
            expiresAtMs = expiresAtMs,
            signatureBase64 = signatureBase64,
        )

    private fun deviceProof(
        platform: String = "android",
        attestationKeyId: String = "attest-key",
        challengeHashHex: String = "01".repeat(32),
        assertionBase64: String = base64(ByteArray(8) { 7 }),
        counter: Long? = null,
    ): OfflineDeviceProof =
        OfflineDeviceProof(
            platform = platform,
            attestationKeyId = attestationKeyId,
            challengeHashHex = challengeHashHex,
            assertionBase64 = assertionBase64,
            counter = counter,
        )

    private fun deviceBinding(platform: String = "android"): OfflineDeviceBinding =
        OfflineDeviceBinding(
            platform = platform,
            attestationKeyId = "attest-key",
            deviceId = "device-1",
            offlinePublicKey = base64(ByteArray(32) { 1 }),
            attestationReportBase64 = base64(ByteArray(8) { 2 }),
            assertionScheme = AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_SCHEME,
            assertionKeyAlgorithm = AttestedOfflineNote.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
            assertionPublicKey = base64(ByteArray(65) { 4 }),
            assertionUsageCountLimit = 1,
        )

    private fun spendAuthorization(
        policyMaxBalance: String = "100",
        policyMaxTxValue: String = "10",
        issuedAtMs: Long = 10,
        refreshAtMs: Long = 15,
        expiresAtMs: Long = 20,
        issuerSignatureBase64: String = base64(ByteArray(64) { 3 }),
    ): OfflineSpendAuthorization =
        OfflineSpendAuthorization(
            authorizationId = "auth-1",
            lineageId = "lineage-1",
            accountId = "alice@hbl.sbp",
            verdictId = "verdict-1",
            policyMaxBalance = policyMaxBalance,
            policyMaxTxValue = policyMaxTxValue,
            issuedAtMs = issuedAtMs,
            refreshAtMs = refreshAtMs,
            expiresAtMs = expiresAtMs,
            deviceBinding = deviceBinding(),
            issuerSignatureBase64 = issuerSignatureBase64,
        )

    private fun cashStatePayload(
        balance: String = "10",
        lockedBalance: String = "0",
        serverRevision: Long = 1,
        serverStateHash: String = "01".repeat(32),
        pendingLocalRevision: Long = 1,
        issuerSignatureBase64: String = base64(ByteArray(64) { 4 }),
    ): OfflineCashStatePayload =
        OfflineCashStatePayload(
            lineageId = "lineage-1",
            accountId = "alice@hbl.sbp",
            deviceId = "device-1",
            offlinePublicKey = base64(ByteArray(32) { 1 }),
            assetDefinitionId = validAssetDefinitionId(),
            balance = balance,
            lockedBalance = lockedBalance,
            serverRevision = serverRevision,
            serverStateHash = serverStateHash,
            pendingLocalRevision = pendingLocalRevision,
            authorization = spendAuthorization(),
            issuerSignatureBase64 = issuerSignatureBase64,
        )

    private fun assetSendLimit(
        dailySendLimit: String = "10",
        monthlySendLimit: String = "100",
    ): OfflineAssetSendLimit =
        OfflineAssetSendLimit(
            assetDefinitionId = "limit-asset#xor",
            dailySendLimit = dailySendLimit,
            monthlySendLimit = monthlySendLimit,
        )

    private fun revocationBundle(
        issuedAtMs: Long = 10,
        expiresAtMs: Long = 20,
        verdictIds: List<String> = listOf("verdict-1"),
        issuerSignatureBase64: String = base64(ByteArray(64) { 5 }),
    ): OfflineRevocationBundlePayload =
        OfflineRevocationBundlePayload(
            issuedAtMs = issuedAtMs,
            expiresAtMs = expiresAtMs,
            verdictIds = verdictIds,
            blacklistedAccountIds = listOf("mallory@hbl.sbp"),
            assetSendLimits = listOf(assetSendLimit()),
            issuerSignatureBase64 = issuerSignatureBase64,
        )

    private fun transferReceipt(
        version: Int = 1,
        direction: String = "outgoing",
        preStateHash: String = "01".repeat(32),
        postStateHash: String = "02".repeat(32),
        amount: String = "1",
        senderSignatureBase64: String = base64(ByteArray(64) { 9 }),
    ): OfflineTransferReceipt =
        OfflineTransferReceipt(
            version = version,
            transferId = "transfer-1",
            direction = direction,
            lineageId = "lineage-1",
            accountId = "alice@hbl.sbp",
            deviceId = "device-1",
            offlinePublicKey = base64(ByteArray(32) { 1 }),
            preBalance = "10",
            postBalance = "9",
            preLockedBalance = "0",
            postLockedBalance = "0",
            preStateHash = preStateHash,
            postStateHash = postStateHash,
            localRevision = 1,
            counterpartyLineageId = "lineage-2",
            counterpartyAccountId = "bob@hbl.sbp",
            counterpartyDeviceId = "device-2",
            counterpartyOfflinePublicKey = base64(ByteArray(32) { 2 }),
            amount = amount,
            deviceProof = deviceProof(),
            senderSignatureBase64 = senderSignatureBase64,
            createdAtMs = 10,
        )

    private fun recursiveProof(
        verifierKeyBackend: String = OfflineNote.RECURSIVE_BACKEND,
        verifierKeyId: String = OfflineNote.RECURSIVE_VERIFIER_NAME,
        proofBackend: String = OfflineNote.RECURSIVE_BACKEND,
        publicInputsHashHex: String = "01".repeat(32),
        proofBytesBase64: String = base64(ByteArray(2) { 7 }),
    ): OfflineRecursiveProof =
        OfflineRecursiveProof(
            verifierKeyBackend = verifierKeyBackend,
            verifierKeyId = verifierKeyId,
            proofBackend = proofBackend,
            publicInputsHashHex = publicInputsHashHex,
            proofBytesBase64 = proofBytesBase64,
        )

    private fun walletState(
        localBalance: String = "0",
        localStateHash: String = "",
        sourceNullifiers: List<String> = emptyList(),
        assetTransferUsage: List<OfflineAssetTransferUsage> = emptyList(),
        noteRecords: List<OfflineNoteRecord> = emptyList(),
    ): OfflineWalletState =
        OfflineWalletState(
            accountId = "alice@hbl.sbp",
            deviceId = "device-1",
            offlinePublicKey = base64(ByteArray(32) { 1 }),
            localBalance = localBalance,
            localStateHash = localStateHash,
            sourceNullifiers = sourceNullifiers,
            assetTransferUsage = assetTransferUsage,
            noteRecords = noteRecords,
        )

    private fun noteRecord(
        noteId: String = "note-1",
        amount: String = "1",
        updatedAtMs: Long = 1,
        noteSecretBase64: String? = null,
    ): OfflineNoteRecord =
        OfflineNoteRecord(
            noteId = noteId,
            commitment = "commitment-$noteId",
            assetDefinitionId = validAssetDefinitionId(),
            amount = amount,
            source = OfflineNoteRecordSource.ISSUED,
            updatedAtMs = updatedAtMs,
            noteSecretBase64 = noteSecretBase64,
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
