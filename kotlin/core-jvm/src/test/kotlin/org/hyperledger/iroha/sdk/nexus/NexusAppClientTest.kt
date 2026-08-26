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
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.alias.AccountFaucetClaimV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPolicyV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingCurrentStateV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanReceiptV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingProofRequiredPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.PreparedTransactionSubmitResponseV1
import org.hyperledger.iroha.sdk.alias.TairaPublicResetMutationBindingV1
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.PipelineStatusOptions
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class NexusAppClientTest {

    @Test
    fun `transfer input requires canonical quantity strings`() {
        listOf(" ", "+1", "01", "1e0", "-1", "1.0", "1.2300").forEach { quantity ->
            assertFailsWith<IllegalArgumentException> {
                NexusTransferInput("asset", quantity, "destination", TEST_FEE_PAYMENT)
            }
        }
        assertEquals(
            "1.25",
            NexusTransferInput(
                "asset",
                KotodamaQuantity.parseCanonical("1.25"),
                "destination",
                TEST_FEE_PAYMENT,
            ).quantity,
        )
    }

    @Test
    fun `transferWithWallet builds signs submits and waits`() {
        val connect = FakeConnect()
        val torii = FakeToriiClient()
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                appId = "sample-app",
                signingPublicKey = PUBLIC_KEY,
            ),
            connectTransport = connect,
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
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
        @Suppress("UNCHECKED_CAST")
        val finalStatus = receipt.finalStatus?.get("status") as? Map<String, Any>
        assertEquals("Applied", finalStatus?.get("kind"))
        assertEquals(receipt.transactionHashHex, torii.submittedHash)
        assertEquals(receipt.transactionHashHex, SignedTransactionHasher.hashHex(receipt.signedTransaction))
        assertContentEquals(PUBLIC_KEY, receipt.signedTransaction.publicKey())
        assertContentEquals(connect.signature, receipt.signedTransaction.signature())
        assertTrue(assertNotNull(connect.lastSignable).payloadBytes.isNotEmpty())
        assertFailsWith<IllegalArgumentException> {
            NexusTransferReceipt(
                "aa".repeat(32),
                receipt.signedTransaction,
                receipt.submission,
                receipt.finalStatus,
            )
        }
        val wrongMarkedHash =
            (if (receipt.transactionHashHex[0] == '0') "1" else "0") + receipt.transactionHashHex.drop(1)
        assertFailsWith<IllegalArgumentException> {
            NexusTransferReceipt(
                wrongMarkedHash,
                receipt.signedTransaction,
                receipt.submission,
                receipt.finalStatus,
            )
        }
    }

    @Test
    fun `buildTransferDraft fails closed without signing public key`() {
        val client = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                    authority = ACCOUNT_ID,
                ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
        )

        val error = assertFailsWith<NexusAppError> {
            client.buildTransferDraft(sampleInput())
        }

        assertEquals("missing_signing_public_key", error.code)
    }

    @Test
    fun `buildTransferDraft matches shared fixture payload`() {
        val fixture = loadNexusFixture()
        val transfer = obj(fixture, "transfer_input")
        val expected = obj(fixture, "expected")
        val approval = obj(obj(fixture, "connect"), "approval_frame")
        val chainDiscriminant = number(transfer, "account_chain_discriminant").toInt()
        val authority = string(transfer, "authority")
        val destination = string(transfer, "destination_account_id")
        val sourceAssetId = string(transfer, "source_asset_id")
        val signingPublicKey = hexToBytes(string(approval, "signing_public_key_hex"))
        val fixtureInput = NexusTransferInput(
            sourceAssetId = sourceAssetId,
            quantity = string(transfer, "quantity"),
            destinationAccountId = destination,
            feePayment = TEST_FEE_PAYMENT,
            creationTimeMs = number(transfer, "creation_time_ms").toLong(),
            ttlMs = number(transfer, "ttl_ms").toLong(),
            nonce = number(transfer, "nonce").toLong(),
            metadata = obj(transfer, "metadata").mapValues { (_, value) -> value as String },
        )
        for (account in listOf(authority, destination, sourceAssetId.substringAfter('#').substringBefore('#'))) {
            val parsed = AccountAddress.parseEncodedIgnoringCurveSupport(account, chainDiscriminant)
            assertEquals(account, parsed.address.toI105(chainDiscriminant))
        }
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = NetworkId.parse(string(transfer, "network_id")),
                chainId = string(obj(fixture, "connect"), "chain_id"),
                chainDiscriminant = chainDiscriminant,
                authority = authority,
                signingPublicKey = signingPublicKey,
            ),
            codecAdapter = NoritoJavaCodecAdapter(chainDiscriminant),
        )

        val draft = client.buildTransferDraft(fixtureInput)
        val fixtureSignature = hexToBytes(string(expected, "wallet_signature_hex"))
        val signed = SignedTransaction.builder()
            .setEncodedPayload(draft.signable.payloadBytes)
            .setSignature(fixtureSignature)
            .setPublicKey(signingPublicKey)
            .setSchemaName(NoritoJavaCodecAdapter(chainDiscriminant).schemaName())
            .build()

        assertEquals(string(expected, "payload_hash_hex"), draft.signable.payloadHashHex)
        assertContentEquals(hexToBytes(string(expected, "payload_bytes_hex")), draft.signable.payloadBytes)
        assertTrue(verifyPayloadSignature(signingPublicKey, draft.signable.payloadBytes, fixtureSignature))
        assertEquals(string(expected, "signed_transaction_hash_hex"), SignedTransactionHasher.hashHex(signed))
        assertEquals(listOf("Submitted", "Applied"), expected["status_sequence"])
    }

    @Test
    fun `Nexus facade rejects wrong-chain transfer and approval accounts`() {
        val fixture = loadNexusFixture()
        val transfer = obj(fixture, "transfer_input")
        val approval = obj(obj(fixture, "connect"), "approval_frame")
        val fixtureChain = number(transfer, "account_chain_discriminant").toInt()
        val authority = string(transfer, "authority")
        val destination = string(transfer, "destination_account_id")
        val sourceAssetId = string(transfer, "source_asset_id")
        val signingPublicKey = hexToBytes(string(approval, "signing_public_key_hex"))
        val wrongChainDestination = AccountAddress
            .parseEncodedIgnoringCurveSupport(destination, fixtureChain)
            .address
            .toI105(fixtureChain + 1)
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = NetworkId.parse(string(transfer, "network_id")),
                chainId = string(obj(fixture, "connect"), "chain_id"),
                chainDiscriminant = fixtureChain,
                authority = authority,
                signingPublicKey = signingPublicKey,
            ),
        )

        val transferError = assertFailsWith<NexusAppError> {
            client.buildTransferDraft(
                NexusTransferInput(
                    sourceAssetId = sourceAssetId,
                    quantity = string(transfer, "quantity"),
                    destinationAccountId = wrongChainDestination,
                    feePayment = TEST_FEE_PAYMENT,
                ),
            )
        }
        assertEquals("invalid_account_id", transferError.code)

        val scopeError = assertFailsWith<NexusAppError> {
            client.buildTransferDraft(
                NexusTransferInput(
                    sourceAssetId = "$sourceAssetId#dataspace:01",
                    quantity = string(transfer, "quantity"),
                    destinationAccountId = destination,
                    feePayment = TEST_FEE_PAYMENT,
                ),
            )
        }
        assertEquals("invalid_account_id", scopeError.code)

        val wrongChainAuthority = AccountAddress
            .parseEncodedIgnoringCurveSupport(authority, fixtureChain)
            .address
            .toI105(fixtureChain + 1)
        val approvalClient = NexusAppClient(
            config = NexusAppConfig(
                networkId = NetworkId.parse(string(transfer, "network_id")),
                chainId = string(obj(fixture, "connect"), "chain_id"),
                chainDiscriminant = fixtureChain,
            ),
            connectTransport = ApprovalConnect(
                NexusApprovedAccount(wrongChainAuthority, signingPublicKey),
            ),
        )
        val approvalError = assertFailsWith<NexusAppError> {
            approvalClient.awaitApproval(
                NexusConnectSession("wrong-chain", "sora://wallet/connect?session=wrong-chain"),
            )
        }
        assertEquals("invalid_account_id", approvalError.code)
    }

    @Test
    fun `finalizeAndSubmit accepts exact zero signature algorithm alias`() {
        val torii = FakeToriiClient()
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = torii,
        )
        val draft = client.buildTransferDraft(sampleInput())
        val signable = draft.signable.copy(signatureAlgorithm = "0")
        val walletSignature = signPayload(signable.payloadBytes)

        val receipt = client.finalizeAndSubmit(
            signable,
            NexusWalletSignature(walletSignature, "0"),
            NexusFinalizeOptions(waitForFinalStatus = false),
        )

        assertEquals(receipt.transactionHashHex, torii.submittedHash)
        assertEquals(receipt.transactionHashHex, SignedTransactionHasher.hashHex(receipt.signedTransaction))
        assertContentEquals(walletSignature, receipt.signedTransaction.signature())
        assertContentEquals(PUBLIC_KEY, receipt.signedTransaction.publicKey())
    }

    @Test
    fun `finalizeAndSubmit rejects unsupported signature algorithm`() {
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(),
        )
        val draft = client.buildTransferDraft(sampleInput())

        for (algorithm in listOf(
            "",
            " ",
            "\t",
            "\n",
            "\u00A0",
            "ed25519 ",
            " ed25519",
            "\ted25519",
            "ed25519\n",
            "ed25519\u00A0",
            "0 ",
            " 0",
            "\t0",
            "00",
            "\uFF10",
            "secp256k1",
            "ed\t25519",
            "ed\u000025519",
            "ed\u001F25519",
            "ed\u007F25519",
            "ed\u200B25519",
            "\u0435d25519",
            "ed\uFF0D25519",
            "ED25519",
            "Ed25519",
            " ED25519 ",
        )) {
            val error = assertFailsWith<NexusAppError> {
                client.finalizeAndSubmit(
                    draft.signable,
                    NexusWalletSignature(ByteArray(64) { 0x07 }, algorithm),
                )
            }

            assertEquals("unsupported_signature_algorithm", error.code, algorithm)
        }

        for (algorithm in listOf(
            "",
            " ",
            "ed25519 ",
            " ed25519",
            "0 ",
            " 0",
            "00",
            "ED25519",
            "ed\u000025519",
            "ed\u200B25519",
            "\u0435d25519",
        )) {
            val signableError = assertFailsWith<NexusAppError> {
                client.finalizeAndSubmit(
                    draft.signable.copy(signatureAlgorithm = algorithm),
                    NexusWalletSignature(WALLET_SIGNATURE),
                )
            }
            assertEquals("unsupported_signature_algorithm", signableError.code, algorithm)
        }
    }

    @Test
    fun `requestSignature rejects unsupported algorithms at transport boundary`() {
        val session = NexusConnectSession(
            sessionId = "session-1",
            walletLaunchUri = "sora://wallet/connect?session=session-1",
            approvedAccount = ACCOUNT_ID,
            signingPublicKey = PUBLIC_KEY,
        )
        val signable = NexusSignableTransaction(
            payloadBytes = byteArrayOf(0x01, 0x02, 0x03),
            payloadHashHex = "0".repeat(64),
            authority = ACCOUNT_ID,
            signingPublicKey = PUBLIC_KEY,
        )

        for (algorithm in listOf("", "ed25519 ", " 0", "ED25519", "ed\u200B25519")) {
            val connect = SignatureConnect(WALLET_SIGNATURE)
            val client = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
                connectTransport = connect,
                codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            )

            val error = assertFailsWith<NexusAppError> {
                client.requestSignature(session, signable.copy(signatureAlgorithm = algorithm))
            }

            assertEquals("unsupported_signature_algorithm", error.code, algorithm)
            assertEquals(null, connect.lastSignable)
        }

        for (algorithm in listOf("ed25519 ", " 0", "\uFF10", "ed\u000025519", "\u0435d25519")) {
            val connect = SignatureConnect(WALLET_SIGNATURE, algorithm)
            val client = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
                connectTransport = connect,
                codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            )

            val error = assertFailsWith<NexusAppError> {
                client.requestSignature(session, signable)
            }

            assertEquals("unsupported_signature_algorithm", error.code, algorithm)
            assertNotNull(connect.lastSignable)
        }
    }

    @Test
    fun `awaitApproval rejects missing account and signing key`() {
        val missingAccount = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            connectTransport = ApprovalConnect(NexusApprovedAccount(accountId = "")),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
        )
        val missingAccountError = assertFailsWith<NexusAppError> {
            missingAccount.awaitApproval(NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"))
        }
        assertEquals("approval_missing_account", missingAccountError.code)

        val missingKey = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            connectTransport = ApprovalConnect(NexusApprovedAccount(accountId = ACCOUNT_ID)),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
        )
        val missingKeyError = assertFailsWith<NexusAppError> {
            missingKey.awaitApproval(NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"))
        }
        assertEquals("missing_signing_public_key", missingKeyError.code)

        val invalidKey = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            connectTransport = ApprovalConnect(NexusApprovedAccount(accountId = ACCOUNT_ID, signingPublicKey = ByteArray(31) { 0x01 })),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
        )
        val invalidKeyError = assertFailsWith<NexusAppError> {
            invalidKey.awaitApproval(NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"))
        }
        assertEquals("invalid_signing_public_key", invalidKeyError.code)

        val mixedTorsionKey = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            connectTransport = ApprovalConnect(
                NexusApprovedAccount(accountId = ACCOUNT_ID, signingPublicKey = ByteArray(32) { 0x11 }),
            ),
        )
        val mixedTorsionError = assertFailsWith<NexusAppError> {
            mixedTorsionKey.awaitApproval(
                NexusConnectSession("session-1", "sora://wallet/connect?session=session-1"),
            )
        }
        assertEquals("invalid_signing_public_key", mixedTorsionError.code)
    }

    @Test
    fun `awaitApproval rejects fixture key and session substitution`() {
        val fixture = loadNexusFixture()
        val connectFixture = obj(fixture, "connect")
        val transferFixture = obj(fixture, "transfer_input")
        val chainDiscriminant = number(transferFixture, "account_chain_discriminant").toInt()
        val callerSession = NexusConnectSession(
            sessionId = string(connectFixture, "sid"),
            walletLaunchUri = string(connectFixture, "wallet_launch_uri"),
        )

        val keyCase = errorCase(fixture, "approval signing key mismatch")
        val keyApproval = obj(keyCase, "approval_frame")
        val keyClient = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = string(connectFixture, "chain_id"),
                chainDiscriminant = chainDiscriminant,
            ),
            connectTransport = ApprovalConnect(
                NexusApprovedAccount(
                    accountId = string(keyApproval, "account_id"),
                    signingPublicKey = hexToBytes(string(keyApproval, "signing_public_key_hex")),
                ),
            ),
        )
        val keyError = assertFailsWith<NexusAppError> {
            keyClient.awaitApproval(callerSession)
        }
        assertEquals(string(keyCase, "expected_code"), keyError.code)

        val sessionCase = errorCase(fixture, "approval session substitution")
        val sessionApproval = obj(sessionCase, "approval_frame")
        val substituted = obj(sessionApproval, "session")
        val sessionClient = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = string(connectFixture, "chain_id"),
                chainDiscriminant = chainDiscriminant,
            ),
            connectTransport = ApprovalConnect(
                NexusApprovedAccount(
                    accountId = string(sessionApproval, "account_id"),
                    signingPublicKey = hexToBytes(
                        string(sessionApproval, "signing_public_key_hex"),
                    ),
                    session = NexusConnectSession(
                        sessionId = string(substituted, "sid"),
                        walletLaunchUri = string(substituted, "wallet_launch_uri"),
                    ),
                ),
            ),
        )
        val sessionError = assertFailsWith<NexusAppError> {
            sessionClient.awaitApproval(callerSession)
        }
        assertEquals(string(sessionCase, "expected_code"), sessionError.code)
        assertEquals(string(connectFixture, "sid"), callerSession.sessionId)
        assertEquals(string(connectFixture, "wallet_launch_uri"), callerSession.walletLaunchUri)
    }

    @Test
    fun `transferWithWallet rejects authority mismatch before signing`() {
        val connect = FakeConnect()
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                signingPublicKey = PUBLIC_KEY,
            ),
            connectTransport = connect,
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
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
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
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
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(),
        )
        val draft = draftClient.buildTransferDraft(sampleInput())
        val walletSignature = signPayload(draft.signable.payloadBytes)
        val localHash = SignedTransactionHasher.hashHex(
            SignedTransaction.builder()
                .setEncodedPayload(draft.signable.payloadBytes)
                .setSignature(walletSignature)
                .setPublicKey(PUBLIC_KEY)
                .setSchemaName(NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).schemaName())
                .build(),
        )

        val mismatchClient = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(responseHash = "f".repeat(64)),
        )
        val mismatchError = assertFailsWith<NexusAppError> {
            mismatchClient.finalizeAndSubmit(draft.signable, NexusWalletSignature(walletSignature))
        }
        assertEquals("transaction_hash_mismatch", mismatchError.code)

        val submitFailureClient = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(submitFailure = RuntimeException("down")),
        )
        val submitError = assertFailsWith<NexusAppError> {
            submitFailureClient.finalizeAndSubmit(draft.signable, NexusWalletSignature(walletSignature))
        }
        assertEquals("submit_failed", submitError.code)

        val statusFailureClient = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(responseHash = localHash, statusFailure = RuntimeException("timeout")),
        )
        val statusError = assertFailsWith<NexusAppError> {
            statusFailureClient.finalizeAndSubmit(draft.signable, NexusWalletSignature(walletSignature))
        }
        assertEquals("status_wait_failed", statusError.code)

        val committedOnlyClient = NexusAppClient(
                config = NexusAppConfig(
                    networkId = TEST_NETWORK_ID,
                    chainId = "test-chain",
                    chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(statusKind = "Committed"),
        )
        val committedOnlyError = assertFailsWith<NexusAppError> {
            committedOnlyClient.finalizeAndSubmit(
                draft.signable,
                NexusWalletSignature(walletSignature),
            )
        }
        assertEquals("status_wait_non_applied", committedOnlyError.code)
    }

    @Test
    fun `finalizeAndSubmit rejects a valid signature bound to different payload bytes`() {
        val torii = FakeToriiClient()
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = torii,
        )
        val draft = client.buildTransferDraft(sampleInput())
        val signature = signPayload(draft.signable.payloadBytes)
        val tamperedPayload = draft.signable.payloadBytes.copyOf().also { bytes ->
            bytes[bytes.lastIndex] = (bytes.last().toInt() xor 0x01).toByte()
        }

        val error = assertFailsWith<NexusAppError> {
            client.finalizeAndSubmit(
                draft.signable.copy(payloadBytes = tamperedPayload),
                NexusWalletSignature(signature),
            )
        }

        assertEquals("invalid_signature", error.code)
        assertEquals(null, torii.submittedHash)
    }

    @Test
    fun `finalizeAndSubmit rejects invalid signature bytes`() {
        val client = NexusAppClient(
            config = NexusAppConfig(
                networkId = TEST_NETWORK_ID,
                chainId = "test-chain",
                chainDiscriminant = AccountAddress.DEFAULT_I105_DISCRIMINANT,
                authority = ACCOUNT_ID,
                signingPublicKey = PUBLIC_KEY,
            ),
            codecAdapter = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT),
            toriiClient = FakeToriiClient(),
        )
        val draft = client.buildTransferDraft(sampleInput())

        val error = assertFailsWith<NexusAppError> {
            client.finalizeAndSubmit(
                draft.signable,
                NexusWalletSignature(ByteArray(64) { 0x07 }),
            )
        }

        assertEquals("invalid_signature", error.code)
    }

    private class FakeConnect : NexusConnectTransport {
        lateinit var signature: ByteArray
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
            signature = signPayload(signable.payloadBytes)
            return NexusWalletSignature(signature)
        }
    }

    private class SignatureConnect(
        private val signature: ByteArray,
        private val algorithm: String = NEXUS_SIGNATURE_ALGORITHM_ED25519,
    ) : NexusConnectTransport {
        var lastSignable: NexusSignableTransaction? = null

        override fun startConnect(
            options: NexusConnectOptions,
            config: NexusAppConfig,
        ): NexusConnectSession = NexusConnectSession("session-1", "sora://wallet/connect?session=session-1")

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
            return NexusWalletSignature(signature.copyOf(), algorithm)
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
        private val statusKind: String = "Applied",
    ) : IrohaClient {
        var submittedHash: String? = null

        override fun planSponsoredAccountOnboarding(
            request: AccountOnboardingPlanRequestV1,
            onboardingToken: String,
            expectedAuthority: String,
            expectedNetworkId: NetworkId,
        ): CompletableFuture<AccountOnboardingPlanReceiptV1> =
            throw AssertionError("account onboarding is not used by this test fake")

        override fun prepareSponsoredAccountOnboarding(
            request: AccountOnboardingPlanRequestV1,
            receipt: AccountOnboardingPlanReceiptV1,
            binding: TairaPublicResetMutationBindingV1,
            feePayment: FeePaymentIntent,
            onboardingToken: String,
            expectedAuthority: String,
            expectedNetworkId: NetworkId,
        ): CompletableFuture<AccountOnboardingPrepareResponseV1> =
            throw AssertionError("account onboarding is not used by this test fake")

        override fun verifyAccountOnboardingCurrentState(
            proofRequired: AccountOnboardingProofRequiredPrepareResponseV1,
            request: AccountOnboardingPlanRequestV1,
            receipt: AccountOnboardingPlanReceiptV1,
            binding: TairaPublicResetMutationBindingV1,
            expectedAuthority: String,
            expectedNetworkId: NetworkId,
            canonicalAuth: ToriiCanonicalRequestAuth,
        ): CompletableFuture<AccountOnboardingCurrentStateV1> =
            throw AssertionError("account onboarding is not used by this test fake")

        override fun submitPreparedAccountOnboarding(
            request: AccountOnboardingPlanRequestV1,
            prepared: AccountOnboardingPreparedTransactionV1,
            expectedFeePayment: FeePaymentIntent,
            onboardingToken: String,
            expectedAuthority: String,
            expectedNetworkId: NetworkId,
        ): CompletableFuture<PreparedTransactionSubmitResponseV1> =
            throw AssertionError("account onboarding is not used by this test fake")

        override fun prepareAccountFaucetTransaction(
            claim: AccountFaucetClaimV1,
            binding: TairaPublicResetMutationBindingV1,
            feePayment: FeePaymentIntent,
            policy: AccountFaucetPolicyV1,
            expectedNetworkId: NetworkId,
        ): CompletableFuture<AccountFaucetPreparedTransactionV1> =
            throw AssertionError("account faucet is not used by this test fake")

        override fun submitPreparedAccountFaucetTransaction(
            prepared: AccountFaucetPreparedTransactionV1,
            expectedFeePayment: FeePaymentIntent,
            policy: AccountFaucetPolicyV1,
            expectedNetworkId: NetworkId,
        ): CompletableFuture<PreparedTransactionSubmitResponseV1> =
            throw AssertionError("account faucet is not used by this test fake")

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
            val status = if (statusKind == "Applied") {
                mapOf<String, Any>("kind" to statusKind, "block_height" to 7)
            } else {
                mapOf<String, Any>("kind" to statusKind)
            }
            return CompletableFuture.completedFuture(
                mapOf(
                    "hash" to hashHex,
                    "status" to status,
                    "scope" to "global",
                    "resolved_from" to if (statusKind == "Applied") "state" else "cache",
                ),
            )
        }
    }

    companion object {
        private const val ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
        private val SIGNING_PRIVATE_KEY_SEED = ByteArray(32) { 0x11 }
        private val PUBLIC_KEY =
            Ed25519PrivateKeyParameters(SIGNING_PRIVATE_KEY_SEED, 0).generatePublicKey().encoded
        private val WALLET_SIGNATURE by lazy {
            val fixture = loadNexusFixture()
            hexToBytes(string(obj(fixture, "expected"), "wallet_signature_hex"))
        }
        private const val ACCOUNT_ID =
            "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"
        private const val DESTINATION_ACCOUNT_ID =
            "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L"
        private val TEST_NETWORK_ID = NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
        )
        private val TEST_FEE_PAYMENT = FeePaymentIntent.authority(emptyList())

        private fun sampleInput(): NexusTransferInput = NexusTransferInput(
            sourceAssetId = "$ASSET_DEFINITION_ID#$ACCOUNT_ID",
            quantity = "12.34",
            destinationAccountId = DESTINATION_ACCOUNT_ID,
            feePayment = TEST_FEE_PAYMENT,
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

        private fun errorCase(fixture: Map<String, Any?>, name: String): Map<String, Any?> {
            @Suppress("UNCHECKED_CAST")
            val cases = fixture["error_cases"] as List<Map<String, Any?>>
            return cases.first { string(it, "name") == name }
        }

        private fun string(map: Map<String, Any?>, key: String): String = map[key] as String

        private fun number(map: Map<String, Any?>, key: String): Number = map[key] as Number

        private fun verifyPayloadSignature(
            publicKey: ByteArray,
            payloadBytes: ByteArray,
            signature: ByteArray,
        ): Boolean {
            val message = IrohaHash.prehash(payloadBytes)
            val verifier = Ed25519Signer()
            verifier.init(false, org.bouncycastle.crypto.params.Ed25519PublicKeyParameters(publicKey, 0))
            verifier.update(message, 0, message.size)
            return verifier.verifySignature(signature)
        }

        private fun signPayload(payloadBytes: ByteArray): ByteArray {
            val message = IrohaHash.prehash(payloadBytes)
            val signer = Ed25519Signer()
            signer.init(true, Ed25519PrivateKeyParameters(SIGNING_PRIVATE_KEY_SEED, 0))
            signer.update(message, 0, message.size)
            return signer.generateSignature()
        }

        private fun hexToBytes(hex: String): ByteArray {
            require(hex.length % 2 == 0) { "hex length must be even" }
            return ByteArray(hex.length / 2) { index ->
                hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
            }
        }
    }
}
