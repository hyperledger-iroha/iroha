package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class OfflineCashCodecTest {

    @Test
    fun canonicalAmountMatchesRustNumericDisplay() {
        assertEquals("100", OfflineCashCodec.canonicalAmountString("100"))
        assertEquals("1", OfflineCashCodec.canonicalAmountString("1"))
        assertEquals("12345", OfflineCashCodec.canonicalAmountString("12345"))
        assertEquals("1000000000000", OfflineCashCodec.canonicalAmountString("1000000000000"))
        assertEquals("42.125", OfflineCashCodec.canonicalAmountString("42.125"))
        assertEquals("0.005", OfflineCashCodec.canonicalAmountString("0.005"))
        assertEquals("100.00", OfflineCashCodec.canonicalAmountString("100.00"))
        assertEquals("1", OfflineCashCodec.canonicalAmountString("+1"))
        assertEquals("0.5", OfflineCashCodec.canonicalAmountString(".5"))
        assertEquals("1", OfflineCashCodec.canonicalAmountString("1."))
        assertEquals("42", OfflineCashCodec.canonicalAmountString("00042"))
        assertEquals("1.2300", OfflineCashCodec.canonicalAmountString("001.2300"))
        assertEquals("0.00", OfflineCashCodec.canonicalAmountString("-0.00"))
    }

    @Test
    fun canonicalAmountRejectsInvalidRustNumericForms() {
        val invalid = listOf(
            "1e3",
            "",
            ".",
            "1.2.3",
            "0.${"1".repeat(29)}",
            BigInteger.ONE.shiftLeft(511).toString(),
        )

        for (amount in invalid) {
            assertFailsWith<IllegalArgumentException>("expected invalid amount: $amount") {
                OfflineCashCodec.canonicalAmountString(amount)
            }
        }
    }

    @Test
    fun receiptKeysAreSortedAndFormatted() {
        val receipts = listOf(
            dummyReceipt("c", 3),
            dummyReceipt("a", 1),
            dummyReceipt("b", 2),
            dummyReceipt("a", 4),
        )
        assertEquals(listOf("a:1", "a:4", "b:2", "c:3"), OfflineCashCodec.receiptKeys(receipts))
    }

    @Test
    fun deviceProofRejectsNonCanonicalFields() {
        val canonicalChallenge = hashHex(0xab)
        assertEquals(canonicalChallenge, deviceProof().challengeHashHex)
        assertEquals("ios", deviceProof(platform = "ios").platform)
        assertEquals("android", deviceProof(platform = "android").platform)

        for (invalidPlatform in listOf("ios-appattest", "android-keymint", "android-keymint ", "Android")) {
            assertEquals(
                "platform must be a supported first-release value",
                assertFailsWith<IllegalArgumentException> {
                    deviceProof(platform = invalidPlatform)
                }.message,
            )
        }
        for (invalidKeyId in listOf("", " attest-key", "attest-key\n")) {
            assertEquals(
                "attestation_key_id must be an exact non-empty string",
                assertFailsWith<IllegalArgumentException> {
                    deviceProof(attestationKeyId = invalidKeyId)
                }.message,
            )
        }
        for (invalidHash in nonExactHashHexVariants(canonicalChallenge)) {
            assertEquals(
                "challenge_hash_hex must be 32-byte lowercase hex",
                assertFailsWith<IllegalArgumentException> {
                    deviceProof(challengeHashHex = invalidHash)
                }.message,
            )
        }
        for (invalidAssertion in listOf(
            "",
            " ${assertionBase64()}",
            "${assertionBase64()}\n",
            Base64.getEncoder().encodeToString(byteArrayOf(0xff.toByte())).replace("=", ""),
            Base64.getEncoder().encodeToString(ByteArray(4) { 0xff.toByte() }).replace('/', '_'),
        )) {
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
    }

    @Test
    fun deviceBindingRejectsNonCanonicalIdentityFields() {
        assertEquals("offline-public-key", deviceBinding().offlinePublicKey)
        assertEquals("production", deviceBinding(platform = "ios", iosEnvironment = "production").iosEnvironment)
        assertEquals("development", deviceBinding(platform = "ios", iosEnvironment = "development").iosEnvironment)

        for (invalidPlatform in listOf("ios-appattest", "android-keymint", "android-keymint ", "Android")) {
            assertEquals(
                "platform must be a supported first-release value",
                assertFailsWith<IllegalArgumentException> {
                    deviceBinding(platform = invalidPlatform)
                }.message,
            )
        }
        for ((field, expectedMessage, build) in listOf(
            Triple("attestation_key_id", "attestation_key_id must be an exact non-empty string") {
                deviceBinding(attestationKeyId = " attest-key")
            },
            Triple("device_id", "device_id must be an exact non-empty string") {
                deviceBinding(deviceId = "device-1\n")
            },
            Triple("offline_public_key", "offline_public_key must be an exact non-empty string") {
                deviceBinding(offlinePublicKey = "")
            },
            Triple("ios_team_id", "ios_team_id must be an exact non-empty string") {
                deviceBinding(platform = "ios", iosTeamId = " TEAMID1234")
            },
            Triple("ios_bundle_id", "ios_bundle_id must be an exact non-empty string") {
                deviceBinding(platform = "ios", iosBundleId = "jp.co.soramitsu.iroha.offline ")
            },
        )) {
            assertEquals(
                expectedMessage,
                assertFailsWith<IllegalArgumentException>("accepted invalid $field") {
                    build()
                }.message,
            )
        }
        for (invalidEnvironment in listOf("Production", " production", "sandbox", "")) {
            assertEquals(
                "ios_environment must be production or development",
                assertFailsWith<IllegalArgumentException> {
                    deviceBinding(platform = "ios", iosEnvironment = invalidEnvironment)
                }.message,
            )
        }
    }

    @Test
    fun spendAuthorizationRejectsNonCanonicalFields() {
        assertEquals(signatureBase64(), spendAuthorization().issuerSignatureBase64)

        for ((field, expectedMessage, build) in listOf(
            Triple("authorization_id", "authorization_id must be an exact non-empty string") {
                spendAuthorization(authorizationId = " authorization-1")
            },
            Triple("lineage_id", "lineage_id must be an exact non-empty string") {
                spendAuthorization(lineageId = "lineage-1\n")
            },
            Triple("account_id", "account_id must be an exact non-empty string") {
                spendAuthorization(accountId = "")
            },
            Triple("device_id", "device_id must be an exact non-empty string") {
                spendAuthorization(deviceId = "device-1 ")
            },
            Triple("offline_public_key", "offline_public_key must be an exact non-empty string") {
                spendAuthorization(offlinePublicKey = "")
            },
            Triple("verdict_id", "verdict_id must be an exact non-empty string") {
                spendAuthorization(verdictId = " verdict-1")
            },
            Triple("max_balance", "max_balance must be a non-negative amount") {
                spendAuthorization(maxBalance = "-1")
            },
            Triple("max_tx_value", "max_tx_value must be a non-negative amount") {
                spendAuthorization(maxTxValue = "-0.01")
            },
            Triple("issuer_signature_base64", "issuer_signature_base64 must be canonical base64") {
                spendAuthorization(issuerSignatureBase64 = " ${signatureBase64()}")
            },
        )) {
            assertEquals(
                expectedMessage,
                assertFailsWith<IllegalArgumentException>("accepted invalid $field") {
                    build()
                }.message,
            )
        }
        assertEquals(
            "authorization validity window must be increasing",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(refreshAtMs = 1_699_999_999_999)
            }.message,
        )
        assertEquals(
            "authorization validity window must be increasing",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(expiresAtMs = 1_700_000_000_000)
            }.message,
        )
        assertEquals(
            "issuer_signature_base64 must be 64 bytes",
            assertFailsWith<IllegalArgumentException> {
                spendAuthorization(issuerSignatureBase64 = Base64.getEncoder().encodeToString(ByteArray(63) { 3 }))
            }.message,
        )

        val payload = cashStateJson { state ->
            @Suppress("UNCHECKED_CAST")
            val authorization = LinkedHashMap(state["authorization"] as Map<String, Any?>)
            authorization["issuer_signature_base64"] = signatureBase64().replace("=", "")
            state["authorization"] = authorization
        }
        assertEquals(
            "issuer_signature_base64 must be canonical base64",
            assertFailsWith<IllegalArgumentException> {
                OfflineJsonParser.parseCashState(payload.toByteArray(Charsets.UTF_8))
            }.message,
        )
    }

    @Test
    fun cashStateRejectsNonCanonicalFields() {
        assertEquals(hashHex(4), cashState().serverStateHash)

        for ((field, expectedMessage, build) in listOf(
            Triple("lineage_id", "lineage_id must be an exact non-empty string") {
                cashState(lineageId = " lineage-1")
            },
            Triple("account_id", "account_id must be an exact non-empty string") {
                cashState(accountId = "")
            },
            Triple("device_id", "device_id must be an exact non-empty string") {
                cashState(deviceId = "device-1\n")
            },
            Triple("offline_public_key", "offline_public_key must be an exact non-empty string") {
                cashState(offlinePublicKey = "")
            },
            Triple("asset_definition_id", "asset_definition_id must be an exact non-empty string") {
                cashState(assetDefinitionId = "pkr#sbp ")
            },
            Triple("balance", "balance must be a non-negative amount") {
                cashState(balance = "-1")
            },
            Triple("locked_balance", "locked_balance must be a non-negative amount") {
                cashState(lockedBalance = "-0.01")
            },
            Triple("server_state_hash", "server_state_hash must be 32-byte lowercase hex") {
                cashState(serverStateHash = "server-state-4")
            },
            Triple("issuer_signature_base64", "issuer_signature_base64 must be canonical base64") {
                cashState(issuerSignatureBase64 = signatureBase64().replace("=", ""))
            },
        )) {
            assertEquals(
                expectedMessage,
                assertFailsWith<IllegalArgumentException>("accepted invalid $field") {
                    build()
                }.message,
            )
        }
        assertEquals(
            "server_revision must be non-negative",
            assertFailsWith<IllegalArgumentException> { cashState(serverRevision = -1) }.message,
        )
        assertEquals(
            "pending_local_revision must be non-negative",
            assertFailsWith<IllegalArgumentException> { cashState(pendingLocalRevision = -1) }.message,
        )

        val payload = cashStateJson { state ->
            state["server_state_hash"] = hashHex(0xab).uppercase()
        }
        assertEquals(
            "server_state_hash must be 32-byte lowercase hex",
            assertFailsWith<IllegalArgumentException> {
                OfflineJsonParser.parseCashState(payload.toByteArray(Charsets.UTF_8))
            }.message,
        )
    }

    @Test
    fun transferReceiptRejectsNonCanonicalFields() {
        assertEquals(hashHex(1), transferReceipt().preStateHash)

        for ((field, expectedMessage, build) in listOf(
            Triple("version", "version must be 1") {
                transferReceipt(version = 2)
            },
            Triple("direction", "direction must be incoming or outgoing") {
                transferReceipt(direction = "Inbound")
            },
            Triple("transfer_id", "transfer_id must be an exact non-empty string") {
                transferReceipt(transferId = " transfer-1")
            },
            Triple("lineage_id", "lineage_id must be an exact non-empty string") {
                transferReceipt(lineageId = "lineage-1\n")
            },
            Triple("account_id", "account_id must be an exact non-empty string") {
                transferReceipt(accountId = "")
            },
            Triple("device_id", "device_id must be an exact non-empty string") {
                transferReceipt(deviceId = "device-1 ")
            },
            Triple("offline_public_key", "offline_public_key must be an exact non-empty string") {
                transferReceipt(offlinePublicKey = "")
            },
            Triple("pre_balance", "pre_balance must be a non-negative amount") {
                transferReceipt(preBalance = "-1")
            },
            Triple("post_balance", "post_balance must be a non-negative amount") {
                transferReceipt(postBalance = "-1")
            },
            Triple("pre_locked_balance", "pre_locked_balance must be a non-negative amount") {
                transferReceipt(preLockedBalance = "-1")
            },
            Triple("post_locked_balance", "post_locked_balance must be a non-negative amount") {
                transferReceipt(postLockedBalance = "-1")
            },
            Triple("pre_state_hash", "pre_state_hash must be 32-byte lowercase hex") {
                transferReceipt(preStateHash = hashHex(0xab).uppercase())
            },
            Triple("post_state_hash", "post_state_hash must be 32-byte lowercase hex") {
                transferReceipt(postStateHash = "post-state")
            },
            Triple("local_revision", "local_revision must be non-negative") {
                transferReceipt(localRevision = -1)
            },
            Triple("counterparty_lineage_id", "counterparty_lineage_id must be an exact non-empty string") {
                transferReceipt(counterpartyLineageId = "")
            },
            Triple("counterparty_account_id", "counterparty_account_id must be an exact non-empty string") {
                transferReceipt(counterpartyAccountId = " bob@hbl.sbp")
            },
            Triple("counterparty_device_id", "counterparty_device_id must be an exact non-empty string") {
                transferReceipt(counterpartyDeviceId = "device-2\n")
            },
            Triple(
                "counterparty_offline_public_key",
                "counterparty_offline_public_key must be an exact non-empty string",
            ) {
                transferReceipt(counterpartyOfflinePublicKey = "")
            },
            Triple("amount", "amount must be a non-negative amount") {
                transferReceipt(amount = "-10")
            },
            Triple("source_payload", "source_payload must be an exact non-empty string") {
                transferReceipt(sourcePayload = "")
            },
            Triple("sender_signature_base64", "sender_signature_base64 must be canonical base64") {
                transferReceipt(senderSignatureBase64 = " ${signatureBase64(4)}")
            },
            Triple("created_at_ms", "created_at_ms must be non-negative") {
                transferReceipt(createdAtMs = -1)
            },
        )) {
            assertEquals(
                expectedMessage,
                assertFailsWith<IllegalArgumentException>("accepted invalid $field") {
                    build()
                }.message,
            )
        }
        assertEquals(
            "sender_signature_base64 must be 64 bytes",
            assertFailsWith<IllegalArgumentException> {
                transferReceipt(senderSignatureBase64 = Base64.getEncoder().encodeToString(ByteArray(63) { 4 }))
            }.message,
        )
    }

    @Test
    fun redeemRequestCommitmentHexMatchesExpected() {
        val hex = OfflineCashCodec.redeemRequestCommitmentHex(
            operationId = "op-1",
            accountId = "acct",
            lineageId = "lin",
            assetDefinitionId = "rose",
            amountCanonical = "100",
            offlinePublicKey = "pubkey",
            authorizationId = "auth",
            preStateHash = "00",
            receipts = emptyList(),
        )
        // Stable fingerprint — regenerate via Rust if the canonical payload shape changes.
        val sb = StringBuilder()
        val payload = """{"account_id":"acct","amount":"100","asset_definition_id":"rose","authorization_id":"auth","kind":"redeem_request","lineage_id":"lin","offline_public_key":"pubkey","operation_id":"op-1","pre_state_hash":"00","receipt_keys":[]}"""
        val expected = java.security.MessageDigest.getInstance("SHA-256")
            .digest(payload.toByteArray(Charsets.UTF_8))
        for (b in expected) sb.append("%02x".format(b.toInt() and 0xFF))
        assertEquals(sb.toString(), hex)
    }

    private fun dummyReceipt(transferId: String, localRevision: Long): OfflineTransferReceipt =
        transferReceipt(transferId = transferId, localRevision = localRevision)

    private fun deviceBinding(
        platform: String = "android",
        attestationKeyId: String = "attest-key",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        attestationReportBase64: String = "attestation-report",
        iosTeamId: String? = null,
        iosBundleId: String? = null,
        iosEnvironment: String? = null,
    ): OfflineCashDeviceBinding =
        OfflineCashDeviceBinding(
            platform = platform,
            attestationKeyId = attestationKeyId,
            deviceId = deviceId,
            offlinePublicKey = offlinePublicKey,
            attestationReportBase64 = attestationReportBase64,
            iosTeamId = iosTeamId,
            iosBundleId = iosBundleId,
            iosEnvironment = iosEnvironment,
        )

    private fun deviceProof(
        platform: String = "android",
        attestationKeyId: String = "attest-key",
        challengeHashHex: String = hashHex(0xab),
        assertionBase64: String = assertionBase64(),
        counter: Long? = null,
    ): OfflineCashDeviceProof =
        OfflineCashDeviceProof(
            platform = platform,
            attestationKeyId = attestationKeyId,
            challengeHashHex = challengeHashHex,
            assertionBase64 = assertionBase64,
            counter = counter,
        )

    private fun spendAuthorization(
        authorizationId: String = "authorization-1",
        lineageId: String = "lineage-1",
        accountId: String = "alice@hbl.sbp",
        deviceId: String? = null,
        offlinePublicKey: String? = null,
        verdictId: String = "verdict-1",
        maxBalance: String = "1000",
        maxTxValue: String = "250",
        issuedAtMs: Long = 1_700_000_000_000,
        refreshAtMs: Long = 1_700_000_100_000,
        expiresAtMs: Long = 1_700_000_200_000,
        deviceBinding: OfflineCashDeviceBinding? = deviceBinding(),
        issuerSignatureBase64: String = signatureBase64(),
    ): OfflineSpendAuthorization =
        OfflineSpendAuthorization(
            authorizationId = authorizationId,
            lineageId = lineageId,
            accountId = accountId,
            deviceId = deviceId,
            offlinePublicKey = offlinePublicKey,
            verdictId = verdictId,
            maxBalance = maxBalance,
            maxTxValue = maxTxValue,
            issuedAtMs = issuedAtMs,
            refreshAtMs = refreshAtMs,
            expiresAtMs = expiresAtMs,
            deviceBinding = deviceBinding,
            issuerSignatureBase64 = issuerSignatureBase64,
        )

    private fun cashState(
        lineageId: String = "lineage-1",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        assetDefinitionId: String = "pkr#sbp",
        balance: String = "100.00",
        lockedBalance: String = "0",
        serverRevision: Long = 4,
        serverStateHash: String = hashHex(4),
        pendingLocalRevision: Long = 4,
        authorization: OfflineSpendAuthorization = spendAuthorization(),
        issuerSignatureBase64: String = signatureBase64(),
    ): OfflineCashState =
        OfflineCashState(
            lineageId = lineageId,
            accountId = accountId,
            deviceId = deviceId,
            offlinePublicKey = offlinePublicKey,
            assetDefinitionId = assetDefinitionId,
            balance = balance,
            lockedBalance = lockedBalance,
            serverRevision = serverRevision,
            serverStateHash = serverStateHash,
            pendingLocalRevision = pendingLocalRevision,
            authorization = authorization,
            issuerSignatureBase64 = issuerSignatureBase64,
        )

    private fun cashStateJson(mutate: (LinkedHashMap<String, Any?>) -> Unit): String {
        val state = LinkedHashMap(cashState().toJsonMap())
        mutate(state)
        return JsonEncoder.encode(state)
    }

    private fun transferReceipt(
        version: Int = 1,
        transferId: String = "transfer-1",
        direction: String = "outgoing",
        lineageId: String = "lineage-1",
        accountId: String = "alice@hbl.sbp",
        deviceId: String = "device-1",
        offlinePublicKey: String = "offline-public-key",
        preBalance: String = "100",
        postBalance: String = "90",
        preLockedBalance: String = "0",
        postLockedBalance: String = "0",
        preStateHash: String = hashHex(1),
        postStateHash: String = hashHex(2),
        localRevision: Long = 5,
        counterpartyLineageId: String = "lineage-2",
        counterpartyAccountId: String = "bob@hbl.sbp",
        counterpartyDeviceId: String = "device-2",
        counterpartyOfflinePublicKey: String = "counterparty-offline-public-key",
        amount: String = "10",
        authorization: OfflineSpendAuthorization? = null,
        deviceProof: OfflineCashDeviceProof = deviceProof(),
        sourcePayload: String? = null,
        senderSignatureBase64: String = signatureBase64(4),
        createdAtMs: Long = 1_700_000_300_000,
    ): OfflineTransferReceipt =
        OfflineTransferReceipt(
            version = version,
            transferId = transferId,
            direction = direction,
            lineageId = lineageId,
            accountId = accountId,
            deviceId = deviceId,
            offlinePublicKey = offlinePublicKey,
            preBalance = preBalance,
            postBalance = postBalance,
            preLockedBalance = preLockedBalance,
            postLockedBalance = postLockedBalance,
            preStateHash = preStateHash,
            postStateHash = postStateHash,
            localRevision = localRevision,
            counterpartyLineageId = counterpartyLineageId,
            counterpartyAccountId = counterpartyAccountId,
            counterpartyDeviceId = counterpartyDeviceId,
            counterpartyOfflinePublicKey = counterpartyOfflinePublicKey,
            amount = amount,
            authorization = authorization,
            deviceProof = deviceProof,
            sourcePayload = sourcePayload,
            senderSignatureBase64 = senderSignatureBase64,
            createdAtMs = createdAtMs,
        )

    private fun nonExactHashHexVariants(canonical: String): List<String> =
        listOf(
            " $canonical",
            "$canonical\n",
            canonical.uppercase(),
            "0x$canonical",
            canonical.dropLast(1),
            "g".repeat(64),
            "",
        )

    private fun hashHex(lastByte: Int): String =
        "00".repeat(31) + "%02x".format(lastByte)

    private fun assertionBase64(): String =
        Base64.getEncoder().encodeToString("assertion".toByteArray(Charsets.UTF_8))

    private fun signatureBase64(byte: Int = 3): String =
        Base64.getEncoder().encodeToString(ByteArray(64) { byte.toByte() })

}
