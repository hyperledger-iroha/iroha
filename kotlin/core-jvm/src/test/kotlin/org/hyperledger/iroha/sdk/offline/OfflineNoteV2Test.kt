package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import java.util.Base64
import java.util.Locale
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.JsonParser

class OfflineNoteV2Test {
    @Test
    fun certificateSigningBytesMatchRustVector() {
        val fixture = loadFixture()
        val sender = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val certificates = obj(obj(fixture, "chain_vectors"), "certificates")

        assertEquals(string(certificates, "sender_payload_base64"), base64(sender.signingBytes()))
        assertEquals(string(certificates, "sender_payload_hash"), hex(sender.payloadHash()))
    }

    @Test
    fun offlineNoteV2ModelsMatchRustNoritoVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")

        assertEquals(string(obj(chain, "issue"), "norito_base64"), base64(issue(fixture).noritoEncoded()))
        assertEquals(string(obj(chain, "audit"), "norito_base64"), base64(audit(fixture).noritoEncoded()))
        assertEquals(string(obj(chain, "redeem"), "norito_base64"), base64(redeem(fixture).noritoEncoded()))
    }

    @Test
    fun walletDerivationsMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issueVector = obj(chain, "issue")
        val payment = obj(fixture, "payment_token")
        val outputClaims = list(payment, "output_claims").map { it as Map<String, Any?> }
        val recipientOutput = outputClaims[0]
        val changeOutput = outputClaims[1]
        val chainId = string(derivation, "chain_id")

        val sourcePreimage = OfflineNoteV2.NoteCommitmentPreimageV2(
            chainId = chainId,
            ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            assetId = string(issueVector, "asset_id"),
            amount = string(issueVector, "amount"),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            origin = OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                operationId = string(derivation, "issuer_load_operation_id"),
                lineageId = string(derivation, "issuer_load_lineage_id"),
                localRevision = long(derivation, "issuer_load_local_revision"),
            ),
        )
        assertEquals(
            string(derivation, "source_note_commitment_preimage_hex"),
            hex(OfflineNoteV2.encodeNoteCommitmentPreimage(sourcePreimage)),
        )
        val sourceCommitment = OfflineNoteV2.deriveNoteCommitment(sourcePreimage)
        assertEquals(string(derivation, "source_note_commitment"), hex(sourceCommitment))

        val inputNullifier = OfflineNoteV2.deriveInputNullifier(
            OfflineNoteV2.InputNullifierPreimageV2(
                chainId = chainId,
                sourceNoteCommitment = sourceCommitment,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            )
        )
        assertEquals(string(derivation, "input_nullifier"), hex(inputNullifier))

        val recipientCommitment = OfflineNoteV2.deriveNoteCommitment(
            OfflineNoteV2.NoteCommitmentPreimageV2(
                chainId = chainId,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                assetId = "${string(recipientOutput, "asset_definition_id")}#${string(recipientOutput, "account_id")}",
                amount = string(recipientOutput, "amount"),
                noteSecret = hexBytes(string(derivation, "recipient_note_secret_hex")),
                origin = OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                    paymentRequestId = string(derivation, "payment_request_id"),
                    outputIndex = 0,
                ),
            )
        )
        assertEquals(string(derivation, "recipient_output_commitment"), hex(recipientCommitment))

        val changeCommitment = OfflineNoteV2.deriveNoteCommitment(
            OfflineNoteV2.NoteCommitmentPreimageV2(
                chainId = chainId,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                assetId = "${string(changeOutput, "asset_definition_id")}#${string(changeOutput, "account_id")}",
                amount = string(changeOutput, "amount"),
                noteSecret = hexBytes(string(derivation, "change_note_secret_hex")),
                origin = OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                    paymentRequestId = string(derivation, "payment_request_id"),
                    outputIndex = 1,
                ),
            )
        )
        assertEquals(string(derivation, "change_output_commitment"), hex(changeCommitment))

        val tokenId = OfflineNoteV2.derivePaymentTokenId(
            OfflineNoteV2.PaymentTokenIdPreimageV2(
                chainId = chainId,
                tokenNonce = hexBytes(string(derivation, "token_nonce_hex")),
                senderKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
                inputNullifiers = listOf(inputNullifier),
                outputCommitments = listOf(recipientCommitment, changeCommitment),
            )
        )
        assertEquals(string(derivation, "payment_token_id"), hex(tokenId))

        val redeemNullifier = OfflineNoteV2.deriveInputNullifier(
            OfflineNoteV2.InputNullifierPreimageV2(
                chainId = chainId,
                sourceNoteCommitment = recipientCommitment,
                ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "recipient_key_certificate_payload_hash")),
                noteSecret = hexBytes(string(derivation, "recipient_note_secret_hex")),
            )
        )
        assertEquals(string(derivation, "redeem_nullifier"), hex(redeemNullifier))
    }

    @Test
    fun publicInputHashesMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertEquals(string(obj(chain, "audit"), "public_inputs_hash"), hex(audit.publicInputsHash()))
        assertEquals(string(obj(chain, "redeem"), "public_inputs_hash"), hex(redeem.publicInputsHash()))
        audit.validateProofBinding()
        redeem.validateProofBinding()
        audit.replacingRecursiveProof(audit.recursiveProof).validateProofBinding()
        redeem.replacingRecursiveProof(redeem.recursiveProof).validateProofBinding()
    }

    @Test
    fun proofBindingRejectsMismatch() {
        val fixture = loadFixture()
        val redeem = redeem(fixture)
        val badProof = OfflineNoteV2.RecursiveProofV2(
            publicInputsHash = OfflineNoteV2.hash("wrong-public-inputs".toByteArray()),
            proof = OfflineNoteV2.ProofBox(
                OfflineNoteV2.RECURSIVE_BACKEND,
                "offline-v2-vector-redeem-proof".toByteArray()
            )
        )
        val forged = OfflineNoteV2.RedeemV2(
            sourceNoteCommitment = redeem.sourceNoteCommitment(),
            inputNullifiers = redeem.inputNullifiers(),
            senderKeyCertificate = redeem.senderKeyCertificate,
            recipient = redeem.recipient,
            assetId = redeem.assetId,
            amount = redeem.amount,
            recursiveProof = badProof,
        )

        assertFailsWith<IllegalArgumentException> {
            forged.validateProofBinding()
        }
    }

    @Test
    fun instanceValuesMatchRustVectors() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val auditValues = OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit(fixture))
        val redeemValues = OfflineNoteV2.InstanceBuilder.redeemInstanceValues(redeem(fixture))
        val auditPublic = auditValues.publicValues()
        val redeemPublic = redeemValues.publicValues()

        assertEquals(
            string(obj(chain, "audit"), "public_inputs_hash"),
            hex(hashFromPublicValues(auditPublic)),
        )
        assertEquals(
            string(obj(chain, "redeem"), "public_inputs_hash"),
            hex(hashFromPublicValues(redeemPublic)),
        )
        assertEquals(2L, auditPublic[4])
        assertEquals(1L, auditPublic[5])
        assertEquals(2L, auditPublic[6])
        assertEquals(52L, auditPublic[7])
        assertEquals(52L, auditPublic[8])
        assertEquals(1L, redeemPublic[4])
        assertEquals(1L, redeemPublic[5])
        assertEquals(1L, redeemPublic[6])
        assertEquals(5L, redeemPublic[7])
        assertEquals(5L, redeemPublic[8])
        assertEquals(52L, auditValues.inputAmounts()[0])
        assertEquals(5L, auditValues.outputAmounts()[0])
        assertEquals(47L, auditValues.outputAmounts()[1])
        assertEquals(5L, redeemValues.inputAmounts()[0])
        assertEquals(5L, redeemValues.outputAmounts()[0])
        assertEquals(
            OfflineNoteV2.instanceScalarBytes(auditPublic[0]).toList(),
            auditValues.publicInstanceColumns()[0].toList(),
        )
    }

    @Test
    fun nativeHalo2ProverProducesVerifyingPayloadWhenRequested() {
        if (System.getenv("IROHA_JVM_OFFLINE_V2_PROVER_TEST") != "1") {
            return
        }
        val fixture = loadFixture()
        val audit = audit(fixture)
        val values = OfflineNoteV2.InstanceBuilder.auditInstanceValues(audit)
        OfflineNoteV2Halo2Prover.prewarm()
        val payload = OfflineNoteV2Halo2Prover.proveZk1Payload(values)
        System.getenv("IROHA_JVM_OFFLINE_V2_PAYLOAD_OUT")?.let {
            Files.write(Paths.get(it), payload)
        }

        assertTrue(OfflineNoteV2Halo2Prover.verifyZk1Payload(payload, values.publicValues()))
        val proof = OfflineNoteV2Halo2Prover.proveAudit(audit)
        audit.replacingRecursiveProof(proof).validateProofBinding()
        assertTrue(proof.proof.bytes().size <= OfflineNoteV2Halo2Prover.MAX_ENVELOPE_BYTES)
    }

    @Test
    fun nativeHalo2ProverPerformanceWhenRequested() {
        if (System.getenv("IROHA_JVM_OFFLINE_V2_BENCH") != "1") {
            return
        }
        val iterations = System.getenv("IROHA_JVM_OFFLINE_V2_BENCH_ITERATIONS")?.toInt() ?: 20
        assertTrue(iterations > 0)
        val fixture = loadFixture()
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        OfflineNoteV2Halo2Prover.prewarm()
        OfflineNoteV2Halo2Prover.proveAudit(audit)
        OfflineNoteV2Halo2Prover.proveRedeem(redeem)

        val auditSeconds = benchmarkSeconds(iterations) {
            OfflineNoteV2Halo2Prover.proveAudit(audit)
        }
        val redeemSeconds = benchmarkSeconds(iterations) {
            OfflineNoteV2Halo2Prover.proveRedeem(redeem)
        }
        println("offline_note_v2_jvm_bench audit=${summary(auditSeconds)} redeem=${summary(redeemSeconds)}")
    }

    @Test
    fun qrFixtureUsesSdkTextPrefix() {
        val fountain = obj(loadFixture(), "fountain_qr_v1")
        assertEquals("iroha:qr1:", string(fountain, "frame_prefix"))
    }

    @Test
    fun walletLoadDerivesCommitmentBeforeIssuerSubmission() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issue = obj(chain, "issue")
        val senderCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val loadContext = OfflineNoteV2LoadContext(
            operationId = string(derivation, "issuer_load_operation_id"),
            lineageId = string(derivation, "issuer_load_lineage_id"),
            localRevision = long(derivation, "issuer_load_local_revision"),
            keyCertificate = senderCertificate,
        )
        val issuerClient = RecordingIssuerClient(loadContext)
        val wallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(issue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            issuerClient = issuerClient,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "source_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_001_000L },
        )

        val note = wallet.load(assetDefinitionFromAssetId(string(issue, "asset_id")), string(issue, "amount")).get()

        assertEquals(string(derivation, "source_note_commitment"), note.noteCommitmentHex())
        assertEquals(
            string(derivation, "source_note_commitment"),
            issuerClient.lastIssueRequest?.noteCommitmentHex(),
        )
        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, note.state)
    }

    @Test
    fun walletLifecycleBuildsAuditAcceptAndRedeemTransactions() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainAudit = obj(chain, "audit")
        val chainRedeem = obj(chain, "redeem")
        val payment = obj(fixture, "payment_token")
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val recipientCertificate = certificate(obj(payment, "recipient_key_certificate"))
        val sourceNote = sourceWalletNote(fixture, senderCertificate)
        val senderStore = InMemoryOfflineNoteV2Store()
        senderStore.upsert(sourceNote)
        val senderWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(chainIssue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(
                listOf(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")),
                )
            ),
            clock = { 1_700_000_001_100L },
        )
        val recipientSubmitter = RecordingTransactionSubmitter()
        val recipientWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = string(payment, "recipient_account_id"),
            attestationProvider = StaticAttestationProvider(recipientCertificate),
            transactionSubmitter = recipientSubmitter,
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_001_200L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            amount = string(chainRedeem, "amount"),
        )
        assertEquals(string(derivation, "recipient_output_commitment"), receiveRequest.outputCommitmentHex())

        val token = senderWallet.pay(receiveRequest)

        assertEquals(string(derivation, "payment_token_id"), token.tokenIdHex())
        assertEquals(string(chainAudit, "public_inputs_hash"), hex(token.audit.publicInputsHash()))
        assertEquals(
            OfflineNoteV2WalletNoteState.SPEND_PENDING,
            senderStore.findNote(hexBytes(string(derivation, "source_note_commitment")))?.state,
        )
        assertEquals(
            OfflineNoteV2WalletNoteState.CHANGE_PENDING,
            senderStore.findNote(hexBytes(string(derivation, "change_output_commitment")))?.state,
        )

        val accepted = recipientWallet.accept(token).get()

        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, accepted.state)
        assertEquals(1, recipientSubmitter.audits.size)
        val redeeming = recipientWallet.redeem(accepted).get()
        assertEquals(OfflineNoteV2WalletNoteState.REDEEM_PENDING, redeeming.state)
        assertEquals(1, recipientSubmitter.redemptions.size)
        assertEquals(string(chainRedeem, "public_inputs_hash"), hex(recipientSubmitter.redemptions[0].publicInputsHash()))
    }

    private fun issue(fixture: Map<String, Any?>): OfflineNoteV2.IssueV2 {
        val chainIssue = obj(obj(fixture, "chain_vectors"), "issue")
        return OfflineNoteV2.IssueV2(
            noteCommitment = hexBytes(string(chainIssue, "note_commitment")),
            keyCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
            assetId = string(chainIssue, "asset_id"),
            amount = string(chainIssue, "amount"),
        )
    }

    private fun redeem(fixture: Map<String, Any?>): OfflineNoteV2.RedeemV2 {
        val vector = obj(obj(fixture, "chain_vectors"), "redeem")
        val payment = obj(fixture, "payment_token")
        return OfflineNoteV2.RedeemV2(
            sourceNoteCommitment = hexBytes(string(vector, "source_note_commitment")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            senderKeyCertificate = certificate(obj(payment, "recipient_key_certificate")),
            recipient = string(payment, "recipient_account_id"),
            assetId = string(vector, "asset_id"),
            amount = string(vector, "amount"),
            recursiveProof = OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNoteV2.ProofBox(
                    OfflineNoteV2.RECURSIVE_BACKEND,
                    "offline-v2-vector-redeem-proof".toByteArray()
                )
            )
        )
    }

    private fun audit(fixture: Map<String, Any?>): OfflineNoteV2.AuditBundleV2 {
        val vector = obj(obj(fixture, "chain_vectors"), "audit")
        val payment = obj(fixture, "payment_token")
        return OfflineNoteV2.AuditBundleV2(
            tokenId = hexBytes(string(vector, "token_id")),
            senderKeyCertificate = certificate(obj(payment, "sender_key_certificate")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            inputClaims = list(payment, "input_claims").map { issuedClaim(it as Map<String, Any?>) },
            outputCommitments = list(vector, "output_commitments").map { hexBytes(it as String) },
            outputClaims = list(payment, "output_claims").map { auditOutputClaim(it as Map<String, Any?>) },
            recursiveProof = OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNoteV2.ProofBox(
                    OfflineNoteV2.RECURSIVE_BACKEND,
                    "offline-v2-vector-audit-proof".toByteArray()
                )
            )
        )
    }

    private fun certificate(json: Map<String, Any?>): OfflineNoteV2.KeyCertificateV2 =
        OfflineNoteV2.KeyCertificateV2(
            version = int(json, "version"),
            platform = string(json, "platform"),
            keyId = string(json, "key_id"),
            deviceId = string(json, "device_id"),
            accountId = string(json, "account_id"),
            publicKey = base64Bytes(string(json, "public_key")),
            assertionScheme = string(json, "assertion_scheme"),
            assertionKeyAlgorithm = string(json, "assertion_key_algorithm"),
            assertionPublicKey = base64Bytes(string(json, "assertion_public_key")),
            assertionUsageCountLimit = nullableInt(json, "assertion_usage_count_limit"),
            oneUse = bool(json, "one_use"),
            issuerSignature = base64Bytes(string(json, "issuer_signature_base64")),
        )

    private fun issuedClaim(json: Map<String, Any?>): OfflineNoteV2.IssuedClaimV2 =
        OfflineNoteV2.IssuedClaimV2(
            domain = string(json, "domain"),
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificatePayloadHash = hexBytes(string(json, "key_certificate_payload_hash")),
            assetId = string(json, "asset_id"),
            amount = string(json, "amount"),
        )

    private fun auditOutputClaim(json: Map<String, Any?>): OfflineNoteV2.AuditOutputClaimV2 =
        OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificate = certificate(obj(json, "key_certificate")),
            assetId = "${string(json, "asset_definition_id")}#${string(json, "account_id")}",
            amount = string(json, "amount"),
        )

    private fun sourceWalletNote(
        fixture: Map<String, Any?>,
        certificate: OfflineNoteV2.KeyCertificateV2,
    ): OfflineNoteV2WalletNote {
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issue = obj(chain, "issue")
        return OfflineNoteV2WalletNote(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(issue, "asset_id")),
            assetId = string(issue, "asset_id"),
            amount = string(issue, "amount"),
            keyCertificate = certificate,
            noteCommitment = hexBytes(string(derivation, "source_note_commitment")),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            origin = OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                operationId = string(derivation, "issuer_load_operation_id"),
                lineageId = string(derivation, "issuer_load_lineage_id"),
                localRevision = long(derivation, "issuer_load_local_revision"),
            ),
            state = OfflineNoteV2WalletNoteState.SPENDABLE,
            createdAtMs = 1_700_000_000_000L,
            updatedAtMs = 1_700_000_000_000L,
        )
    }

    private class StaticAttestationProvider(
        private val certificate: OfflineNoteV2.KeyCertificateV2,
    ) : OfflineNoteV2AttestationProvider {
        override fun currentKeyCertificate(): OfflineNoteV2.KeyCertificateV2 = certificate
    }

    private class QueueRandomSource(
        private val values: List<ByteArray>,
    ) : OfflineNoteV2RandomSource {
        private var index = 0

        override fun nextBytes(length: Int): ByteArray {
            require(index < values.size) { "test random source exhausted" }
            val value = values[index++]
            require(value.size == length) { "test random source returned ${value.size} bytes" }
            return value.copyOf()
        }
    }

    private class FixedIdGenerator(
        private val id: String,
    ) : OfflineNoteV2IdGenerator {
        override fun nextId(prefix: String): String = id
    }

    private object BindingProofProvider : OfflineNoteV2ProofProvider {
        override fun proveAudit(audit: OfflineNoteV2.AuditBundleV2): OfflineNoteV2.RecursiveProofV2 =
            OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = audit.publicInputsHash(),
                proof = OfflineNoteV2.ProofBox(
                    OfflineNoteV2.RECURSIVE_BACKEND,
                    "wallet-audit-proof".toByteArray(),
                ),
            )

        override fun proveRedeem(redemption: OfflineNoteV2.RedeemV2): OfflineNoteV2.RecursiveProofV2 =
            OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = redemption.publicInputsHash(),
                proof = OfflineNoteV2.ProofBox(
                    OfflineNoteV2.RECURSIVE_BACKEND,
                    "wallet-redeem-proof".toByteArray(),
                ),
            )
    }

    private class RecordingIssuerClient(
        private val loadContext: OfflineNoteV2LoadContext,
    ) : OfflineNoteV2IssuerClient {
        var lastIssueRequest: OfflineNoteV2IssueRequest? = null

        override fun prepareLoad(
            chainId: String,
            accountId: String,
            assetDefinitionId: String,
            amount: String,
        ): CompletableFuture<OfflineNoteV2LoadContext> = CompletableFuture.completedFuture(loadContext)

        override fun issueNote(request: OfflineNoteV2IssueRequest): CompletableFuture<OfflineNoteV2IssueResponse> {
            lastIssueRequest = request
            return CompletableFuture.completedFuture(
                OfflineNoteV2IssueResponse(
                    noteCommitment = request.noteCommitment(),
                    operationId = request.loadContext.operationId,
                    lineageId = request.loadContext.lineageId,
                    localRevision = request.loadContext.localRevision,
                    keyCertificate = request.loadContext.keyCertificate,
                    settlementEntryHashHex = "settlement-entry-hash",
                )
            )
        }
    }

    private class RecordingTransactionSubmitter : OfflineNoteV2TransactionSubmitter {
        val audits = ArrayList<OfflineNoteV2.AuditBundleV2>()
        val redemptions = ArrayList<OfflineNoteV2.RedeemV2>()

        override fun submitAudit(audit: OfflineNoteV2.AuditBundleV2): CompletableFuture<ClientResponse> {
            audits.add(audit)
            return CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))
        }

        override fun submitRedeem(redemption: OfflineNoteV2.RedeemV2): CompletableFuture<ClientResponse> {
            redemptions.add(redemption)
            return CompletableFuture.completedFuture(ClientResponse(202, byteArrayOf(), "accepted"))
        }
    }

    private fun loadFixture(): Map<String, Any?> {
        val path = Paths.get("..", "..", "fixtures", "offline", "interop_contract_v2.json")
        val parsed = JsonParser.parse(String(Files.readAllBytes(path), Charsets.UTF_8))
        @Suppress("UNCHECKED_CAST")
        return parsed as Map<String, Any?>
    }

    private fun obj(map: Map<String, Any?>, key: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return map[key] as Map<String, Any?>
    }

    private fun list(map: Map<String, Any?>, key: String): List<Any?> {
        @Suppress("UNCHECKED_CAST")
        return map[key] as List<Any?>
    }

    private fun string(map: Map<String, Any?>, key: String): String = map[key] as String
    private fun bool(map: Map<String, Any?>, key: String): Boolean = map[key] as Boolean
    private fun int(map: Map<String, Any?>, key: String): Int = (map[key] as Number).toInt()
    private fun long(map: Map<String, Any?>, key: String): Long = (map[key] as Number).toLong()
    private fun nullableInt(map: Map<String, Any?>, key: String): Int? = (map[key] as Number?)?.toInt()

    private fun base64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)
    private fun base64Bytes(value: String): ByteArray = Base64.getDecoder().decode(value)

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xFF) }

    private fun assetDefinitionFromAssetId(assetId: String): String = assetId.substringBefore('#')

    private fun accountFromAssetId(assetId: String): String =
        assetId.substringAfter('#').substringBefore("#dataspace:")

    private fun hashFromPublicValues(values: LongArray): ByteArray {
        val out = ByteArray(32)
        for (idx in 0 until 4) {
            var word = values[idx]
            for (offset in 0 until 8) {
                out[idx * 8 + offset] = (word and 0xffL).toByte()
                word = word ushr 8
            }
        }
        return out
    }

    private fun benchmarkSeconds(iterations: Int, body: () -> Unit): DoubleArray {
        val durations = DoubleArray(iterations)
        for (idx in 0 until iterations) {
            val start = System.nanoTime()
            body()
            durations[idx] = (System.nanoTime() - start).toDouble() / 1_000_000_000.0
        }
        return durations
    }

    private fun summary(values: DoubleArray): String {
        val sorted = values.sorted()
        if (sorted.isEmpty()) {
            return "empty"
        }
        val median = if (sorted.size % 2 == 0) {
            (sorted[sorted.size / 2 - 1] + sorted[sorted.size / 2]) / 2.0
        } else {
            sorted[sorted.size / 2]
        }
        val p95Index = minOf(sorted.size - 1, maxOf(0, kotlin.math.ceil(sorted.size * 0.95).toInt() - 1))
        return "median=%.3fs p95=%.3fs max=%.3fs n=%d".format(
            Locale.ROOT,
            median,
            sorted[p95Index],
            sorted.last(),
            sorted.size,
        )
    }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0) { "hex length must be even" }
        val out = ByteArray(value.length / 2)
        var offset = 0
        while (offset < value.length) {
            out[offset / 2] = value.substring(offset, offset + 2).toInt(16).toByte()
            offset += 2
        }
        return out
    }
}
