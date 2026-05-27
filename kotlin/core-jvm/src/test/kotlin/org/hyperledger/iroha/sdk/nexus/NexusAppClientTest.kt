package org.hyperledger.iroha.sdk.nexus

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.PipelineStatusOptions
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class NexusAppClientTest {

    @Test
    fun `transferWithWallet builds signs submits and waits`() {
        val connect = FakeConnect()
        val torii = FakeToriiClient()
        val client = NexusAppClient(
            config = NexusAppConfig(
                chainId = "test-chain",
                appId = "sample-app",
                signingPublicKey = PUBLIC_KEY,
            ),
            connectTransport = connect,
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = torii,
        )

        val session = client.startConnect(NexusConnectOptions(walletUriBase = "sora://wallet/connect"))
        val approved = client.awaitApproval(session)
        val approvedSession = assertNotNull(approved.session)
        val receipt = client.transferWithWallet(
            approvedSession,
            sampleInput(),
            NexusFinalizeOptions(waitForFinalStatus = true),
        )

        assertEquals(ACCOUNT_ID, approved.accountId)
        assertEquals("Committed", receipt.finalStatus?.get("status"))
        assertEquals(receipt.transactionHashHex, torii.submittedHash)
        assertEquals(receipt.transactionHashHex, SignedTransactionHasher.hashHex(receipt.signedTransaction))
        assertContentEquals(PUBLIC_KEY, receipt.signedTransaction.publicKey())
        assertContentEquals(connect.signature, receipt.signedTransaction.signature())
        assertTrue(assertNotNull(connect.lastSignable).payloadBytes.isNotEmpty())
    }

    @Test
    fun `buildTransferDraft fails closed without signing public key`() {
        val client = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain", authority = ACCOUNT_ID),
            codecAdapter = NoritoJavaCodecAdapter(),
        )

        val error = assertFailsWith<NexusAppError> {
            client.buildTransferDraft(sampleInput())
        }

        assertEquals("missing_signing_public_key", error.code)
    }

    @Test
    fun `buildTransferDraft matches shared fixture payload`() {
        val fixture = loadNexusFixture()
        val expected = obj(fixture, "expected")
        val client = NexusAppClient(
            config = NexusAppConfig(
                chainId = "test-chain",
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(),
        )

        val draft = client.buildTransferDraft(sampleInput())

        assertEquals(string(expected, "payload_hash_hex"), draft.signable.payloadHashHex)
        assertContentEquals(hexToBytes(string(expected, "payload_bytes_hex")), draft.signable.payloadBytes)
    }

    @Test
    fun `finalizeAndSubmit rejects unsupported signature algorithm`() {
        val client = NexusAppClient(
            config = NexusAppConfig(
                chainId = "test-chain",
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(),
        )
        val draft = client.buildTransferDraft(sampleInput())

        val error = assertFailsWith<NexusAppError> {
            client.finalizeAndSubmit(
                draft.signable,
                NexusWalletSignature(ByteArray(64) { 0x07 }, "secp256k1"),
            )
        }

        assertEquals("unsupported_signature_algorithm", error.code)
    }

    @Test
    fun `awaitApproval rejects missing account and signing key`() {
        val missingAccount = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain"),
            connectTransport = ApprovalConnect(NexusApprovedAccount(accountId = "")),
            codecAdapter = NoritoJavaCodecAdapter(),
        )
        val missingAccountError = assertFailsWith<NexusAppError> {
            missingAccount.awaitApproval(NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"))
        }
        assertEquals("approval_missing_account", missingAccountError.code)

        val missingKey = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain"),
            connectTransport = ApprovalConnect(NexusApprovedAccount(accountId = ACCOUNT_ID)),
            codecAdapter = NoritoJavaCodecAdapter(),
        )
        val missingKeyError = assertFailsWith<NexusAppError> {
            missingKey.awaitApproval(NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"))
        }
        assertEquals("missing_signing_public_key", missingKeyError.code)

        val invalidKey = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain"),
            connectTransport = ApprovalConnect(NexusApprovedAccount(accountId = ACCOUNT_ID, signingPublicKey = ByteArray(31) { 0x01 })),
            codecAdapter = NoritoJavaCodecAdapter(),
        )
        val invalidKeyError = assertFailsWith<NexusAppError> {
            invalidKey.awaitApproval(NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"))
        }
        assertEquals("invalid_signing_public_key", invalidKeyError.code)
    }

    @Test
    fun `transferWithWallet rejects authority mismatch before signing`() {
        val connect = FakeConnect()
        val client = NexusAppClient(
            config = NexusAppConfig(
                chainId = "test-chain",
                signingPublicKey = PUBLIC_KEY,
            ),
            connectTransport = connect,
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(),
        )
        val session = NexusConnectSession(
            sessionId = "session-1",
            walletLaunchUri = "sora://wallet/connect?session=session-1",
            approvedAccount = ACCOUNT_ID,
            signingPublicKey = PUBLIC_KEY,
        )

        val error = assertFailsWith<NexusAppError> {
            client.transferWithWallet(
                session,
                sampleInput().copy(authority = DESTINATION_ACCOUNT_ID),
            )
        }

        assertEquals("approval_account_mismatch", error.code)
        assertEquals(null, connect.lastSignable)
    }

    @Test
    fun `finalizeAndSubmit rejects invalid signature length`() {
        val client = NexusAppClient(
            config = NexusAppConfig(
                chainId = "test-chain",
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(),
        )
        val draft = client.buildTransferDraft(sampleInput())

        val error = assertFailsWith<NexusAppError> {
            client.finalizeAndSubmit(
                draft.signable,
                NexusWalletSignature(ByteArray(63) { 0x07 }),
            )
        }

        assertEquals("invalid_signature", error.code)
    }

    @Test
    fun `finalizeAndSubmit rejects hash mismatch and maps submit status failures`() {
        val draftClient = NexusAppClient(
            config = NexusAppConfig(
                chainId = "test-chain",
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(),
        )
        val draft = draftClient.buildTransferDraft(sampleInput())
        val localHash = SignedTransactionHasher.hashHex(
            SignedTransaction.builder()
                .setEncodedPayload(draft.signable.payloadBytes)
                .setSignature(ByteArray(64) { 0x07 })
                .setPublicKey(PUBLIC_KEY)
                .setSchemaName(NoritoJavaCodecAdapter().schemaName())
                .build(),
        )

        val mismatchClient = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain"),
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(responseHash = "f".repeat(64)),
        )
        val mismatchError = assertFailsWith<NexusAppError> {
            mismatchClient.finalizeAndSubmit(draft.signable, NexusWalletSignature(ByteArray(64) { 0x07 }))
        }
        assertEquals("transaction_hash_mismatch", mismatchError.code)

        val submitFailureClient = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain"),
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(submitFailure = RuntimeException("down")),
        )
        val submitError = assertFailsWith<NexusAppError> {
            submitFailureClient.finalizeAndSubmit(draft.signable, NexusWalletSignature(ByteArray(64) { 0x07 }))
        }
        assertEquals("submit_failed", submitError.code)

        val statusFailureClient = NexusAppClient(
            config = NexusAppConfig(chainId = "test-chain"),
            codecAdapter = NoritoJavaCodecAdapter(),
            toriiClient = FakeToriiClient(responseHash = localHash, statusFailure = RuntimeException("timeout")),
        )
        val statusError = assertFailsWith<NexusAppError> {
            statusFailureClient.finalizeAndSubmit(draft.signable, NexusWalletSignature(ByteArray(64) { 0x07 }))
        }
        assertEquals("status_wait_failed", statusError.code)
    }

    private class FakeConnect : NexusConnectTransport {
        val signature = ByteArray(64) { 0x07 }
        var lastSignable: NexusSignableTransaction? = null

        override fun startConnect(
            options: NexusConnectOptions,
            config: NexusAppConfig,
        ): NexusConnectSession = NexusConnectSession(
            sessionId = options.sessionId ?: "session-1",
            walletLaunchUri = "${options.walletUriBase ?: "sora://wallet/connect"}?session=${options.sessionId ?: "session-1"}",
            appId = config.appId,
            relayUrl = config.relayUrl,
            node = options.node ?: config.node,
        )

        override fun awaitApproval(
            session: NexusConnectSession,
            config: NexusAppConfig,
        ): NexusApprovedAccount = NexusApprovedAccount(
            accountId = ACCOUNT_ID,
            signingPublicKey = PUBLIC_KEY,
        )

        override fun requestSignature(
            session: NexusConnectSession,
            signable: NexusSignableTransaction,
            config: NexusAppConfig,
        ): NexusWalletSignature {
            lastSignable = signable
            assertEquals(NEXUS_SIGNATURE_ALGORITHM_ED25519, signable.signatureAlgorithm)
            return NexusWalletSignature(signature)
        }
    }

    private class ApprovalConnect(private val approval: NexusApprovedAccount) : NexusConnectTransport {
        override fun startConnect(
            options: NexusConnectOptions,
            config: NexusAppConfig,
        ): NexusConnectSession = NexusConnectSession("session-1", "sora://wallet/connect?session=session-1")

        override fun awaitApproval(
            session: NexusConnectSession,
            config: NexusAppConfig,
        ): NexusApprovedAccount = approval

        override fun requestSignature(
            session: NexusConnectSession,
            signable: NexusSignableTransaction,
            config: NexusAppConfig,
        ): NexusWalletSignature = throw AssertionError("signature request should not be called")
    }

    private class FakeToriiClient(
        private val responseHash: String? = null,
        private val submitFailure: RuntimeException? = null,
        private val statusFailure: RuntimeException? = null,
    ) : IrohaClient {
        var submittedHash: String? = null

        override fun submitTransaction(transaction: SignedTransaction): CompletableFuture<ClientResponse> {
            submitFailure?.let {
                return CompletableFuture<ClientResponse>().also { future ->
                    future.completeExceptionally(it)
                }
            }
            submittedHash = SignedTransactionHasher.hashHex(transaction)
            return CompletableFuture.completedFuture(
                ClientResponse(202, ByteArray(0), "accepted", responseHash ?: submittedHash),
            )
        }

        override fun waitForTransactionStatus(
            hashHex: String,
            options: PipelineStatusOptions?,
        ): CompletableFuture<Map<String, Any>> {
            statusFailure?.let {
                return CompletableFuture<Map<String, Any>>().also { future ->
                    future.completeExceptionally(it)
                }
            }
            return CompletableFuture.completedFuture(mapOf("status" to "Committed", "hash" to hashHex))
        }
    }

    companion object {
        private const val ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
        private val PUBLIC_KEY =
            hexToBytes("d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737")
        private const val ACCOUNT_ID =
            "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"
        private const val DESTINATION_ACCOUNT_ID =
            "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L"

        private fun sampleInput(): NexusTransferInput = NexusTransferInput(
            sourceAssetId = "$ASSET_DEFINITION_ID#$ACCOUNT_ID",
            quantity = "12.34",
            destinationAccountId = DESTINATION_ACCOUNT_ID,
            creationTimeMs = 1_700_000_000_000L,
            ttlMs = 30_000L,
            nonce = 7,
            metadata = mapOf("purpose" to "nexus-app-fixture"),
        )

        private fun loadNexusFixture(): Map<String, Any?> {
            var cursor: Path? = Paths.get("").toAbsolutePath()
            while (cursor != null) {
                val candidate = cursor.resolve("fixtures/sdk/nexus_connect_transfer_v1.json")
                if (Files.isRegularFile(candidate)) {
                    val parsed = JsonParser.parse(candidate.toFile().readText())
                    @Suppress("UNCHECKED_CAST")
                    return parsed as Map<String, Any?>
                }
                cursor = cursor.parent
            }
            error("fixtures/sdk/nexus_connect_transfer_v1.json was not found")
        }

        private fun obj(map: Map<String, Any?>, key: String): Map<String, Any?> {
            @Suppress("UNCHECKED_CAST")
            return map[key] as Map<String, Any?>
        }

        private fun string(map: Map<String, Any?>, key: String): String = map[key] as String

        private fun hexToBytes(hex: String): ByteArray {
            require(hex.length % 2 == 0) { "hex length must be even" }
            return ByteArray(hex.length / 2) { index ->
                hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            }
        }
    }
}
