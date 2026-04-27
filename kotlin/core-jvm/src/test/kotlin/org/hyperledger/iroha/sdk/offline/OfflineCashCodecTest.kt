package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import kotlin.test.Test
import kotlin.test.assertEquals

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
    fun idempotencyKeyUsesOperationIdForMutationsAndSha256ForSetup() {
        val binding = OfflineCashDeviceBinding(
            platform = "android",
            attestationKeyId = "k",
            deviceId = "d",
            offlinePublicKey = "",
            attestationReportBase64 = "",
        )
        val proof = OfflineCashDeviceProof(
            platform = "android",
            attestationKeyId = "k",
            challengeHashHex = "",
            assertionBase64 = "",
            counter = null,
        )
        val load = OfflineCashLoadRequest(
            operationId = "op-load",
            lineageId = null,
            accountId = "acct",
            assetDefinitionId = "asset",
            amount = "1",
            deviceBinding = binding,
            deviceProof = proof,
        )
        assertEquals(
            "offline-cash:op-load",
            OfflineCashCodec.stableIdempotencyKey(load, "/v1/offline/cash/load", ByteArray(0)),
        )

        val refresh = OfflineCashRefreshRequest(
            operationId = "op-refresh",
            lineageId = "lineage",
            accountId = "acct",
            deviceBinding = binding,
            deviceProof = proof,
        )
        assertEquals(
            "offline-cash:op-refresh",
            OfflineCashCodec.stableIdempotencyKey(refresh, "/v1/offline/cash/refresh", ByteArray(0)),
        )

        val sync = OfflineCashSyncRequest(
            operationId = "op-sync",
            lineageId = "lineage",
            accountId = "acct",
            deviceBinding = binding,
            deviceProof = proof,
            receipts = emptyList(),
        )
        assertEquals(
            "offline-cash:op-sync",
            OfflineCashCodec.stableIdempotencyKey(sync, "/v1/offline/cash/sync", ByteArray(0)),
        )

        val envelopeProof = OfflineRedeemRequestProof(
            backend = "stark/fri/sha256-goldilocks",
            circuitId = "offline-bearer-redeem-request-v1",
            recursionDepth = 1,
            publicInputsHex = "",
            envelope = OfflineStarkVerifyEnvelopeV1(
                params = OfflineStarkFriParamsV1(1, 4, 3, 2, 8, 2, 1, ""),
                proof = OfflineStarkProofV1(
                    version = 1,
                    commits = OfflineStarkCommitmentsV1(1, emptyList(), null),
                    queries = emptyList(),
                    compValues = null,
                    air = null,
                ),
                transcriptLabel = "",
            ),
        )
        val redeem = OfflineCashRedeemRequest(
            operationId = "op-redeem",
            lineageId = "lineage",
            accountId = "acct",
            deviceBinding = binding,
            deviceProof = proof,
            amount = "1",
            receipts = emptyList(),
            redeemProof = envelopeProof,
        )
        assertEquals(
            "offline-cash:op-redeem",
            OfflineCashCodec.stableIdempotencyKey(redeem, "/v1/offline/cash/redeem", ByteArray(0)),
        )

        val setup = OfflineCashSetupRequest(
            accountId = "acct",
            assetDefinitionId = "asset",
            deviceBinding = binding,
            deviceProof = proof,
        )
        val path = "/v1/offline/cash/setup"
        val body = "{}".toByteArray(Charsets.UTF_8)
        // SHA-256("/v1/offline/cash/setup" || 0x00 || "{}") expected hex (computed once via MessageDigest).
        val digestInput = path.toByteArray(Charsets.UTF_8) + byteArrayOf(0) + body
        val expected = java.security.MessageDigest.getInstance("SHA-256").digest(digestInput)
        val expectedHex = expected.joinToString("") { "%02x".format(it.toInt() and 0xFF) }
        assertEquals(
            "offline-cash:setup:$expectedHex",
            OfflineCashCodec.stableIdempotencyKey(setup, path, body),
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
        OfflineTransferReceipt(
            version = 1,
            transferId = transferId,
            direction = "incoming",
            lineageId = "lin",
            accountId = "acct",
            deviceId = "dev",
            offlinePublicKey = "",
            preBalance = "0",
            postBalance = "0",
            preLockedBalance = "0",
            postLockedBalance = "0",
            preStateHash = "",
            postStateHash = "",
            localRevision = localRevision,
            counterpartyLineageId = "",
            counterpartyAccountId = "",
            counterpartyDeviceId = "",
            counterpartyOfflinePublicKey = "",
            amount = "0",
            authorization = null,
            deviceProof = OfflineCashDeviceProof("android", "k", "", "", null),
            sourcePayload = null,
            senderSignatureBase64 = "",
            createdAtMs = 0,
        )

    // Silence unused warnings for BigInteger import in case future tests need it.
    @Suppress("unused")
    private val unused: BigInteger = BigInteger.ZERO
}
