package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.net.URI
import java.nio.file.Files
import java.nio.file.Paths
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.util.Base64
import java.util.Locale
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.WirePayload

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
    fun publicNoritoDecodersRoundTripFixturePayloads() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val certificates = obj(chain, "certificates")
        val derivation = obj(chain, "derivation")
        val issueVector = obj(chain, "issue")
        val redeemVector = obj(chain, "redeem")
        val senderCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val senderPayloadBytes = base64Bytes(string(certificates, "sender_payload_base64"))
        val issueBytes = base64Bytes(string(issueVector, "norito_base64"))
        val auditBytes = base64Bytes(string(obj(chain, "audit"), "norito_base64"))
        val redeemBytes = base64Bytes(string(redeemVector, "norito_base64"))

        assertEquals(
            base64(senderPayloadBytes),
            base64(OfflineNoteV2.decodeCertificatePayload(senderPayloadBytes).noritoEncoded()),
        )
        assertEquals(
            base64(senderCertificate.noritoEncoded()),
            base64(OfflineNoteV2.decodeCertificate(senderCertificate.noritoEncoded()).noritoEncoded()),
        )
        assertEquals(base64(issueBytes), base64(OfflineNoteV2.decodeIssue(issueBytes).noritoEncoded()))

        val decodedAudit = OfflineNoteV2.decodeAudit(auditBytes)
        assertEquals(base64(auditBytes), base64(decodedAudit.noritoEncoded()))
        assertEquals(
            base64(decodedAudit.inputClaims.first().noritoEncoded()),
            base64(OfflineNoteV2.decodeIssuedClaim(decodedAudit.inputClaims.first().noritoEncoded()).noritoEncoded()),
        )
        assertEquals(
            base64(decodedAudit.publicInputs().noritoEncoded()),
            base64(OfflineNoteV2.decodeAuditPublicInputs(decodedAudit.publicInputs().noritoEncoded()).noritoEncoded()),
        )

        val decodedRedeem = OfflineNoteV2.decodeRedeem(redeemBytes)
        assertEquals(base64(redeemBytes), base64(decodedRedeem.noritoEncoded()))
        assertEquals(
            base64(decodedRedeem.publicInputs().noritoEncoded()),
            base64(OfflineNoteV2.decodeRedeemPublicInputs(decodedRedeem.publicInputs().noritoEncoded()).noritoEncoded()),
        )

        val commitmentPreimage = OfflineNoteV2.NoteCommitmentPreimageV2(
            chainId = string(derivation, "chain_id"),
            ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            assetId = string(issueVector, "asset_id"),
            amount = string(redeemVector, "amount"),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
            origin = OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                operationId = string(derivation, "issuer_load_operation_id"),
                lineageId = string(derivation, "issuer_load_lineage_id"),
                localRevision = long(derivation, "issuer_load_local_revision"),
            ),
        )
        assertEquals(
            base64(commitmentPreimage.noritoEncoded()),
            base64(OfflineNoteV2.decodeNoteCommitmentPreimage(commitmentPreimage.noritoEncoded()).noritoEncoded()),
        )

        val nullifierPreimage = OfflineNoteV2.InputNullifierPreimageV2(
            chainId = string(derivation, "chain_id"),
            sourceNoteCommitment = hexBytes(string(derivation, "source_note_commitment")),
            ownerKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            noteSecret = hexBytes(string(derivation, "source_note_secret_hex")),
        )
        assertEquals(
            base64(nullifierPreimage.noritoEncoded()),
            base64(OfflineNoteV2.decodeInputNullifierPreimage(nullifierPreimage.noritoEncoded()).noritoEncoded()),
        )

        val tokenPreimage = OfflineNoteV2.PaymentTokenIdPreimageV2(
            chainId = string(derivation, "chain_id"),
            tokenNonce = hexBytes(string(derivation, "token_nonce_hex")),
            senderKeyCertificatePayloadHash = hexBytes(string(derivation, "sender_key_certificate_payload_hash")),
            inputNullifiers = listOf(hexBytes(string(derivation, "input_nullifier"))),
            outputCommitments = listOf(
                hexBytes(string(derivation, "recipient_output_commitment")),
                hexBytes(string(derivation, "change_output_commitment")),
            ),
        )
        assertEquals(
            base64(tokenPreimage.noritoEncoded()),
            base64(OfflineNoteV2.decodePaymentTokenIdPreimage(tokenPreimage.noritoEncoded()).noritoEncoded()),
        )
    }

    @Test
    fun publicNoritoInstructionDecodersReadExplorerEnvelopeBytes() {
        val fixture = loadFixture()
        val issue = issue(fixture)
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertEquals(
            base64(issue.noritoEncoded()),
            base64(OfflineNoteV2.decodeIssueInstruction(rawInstructionPair(
                OfflineNoteV2.ISSUE_INSTRUCTION_SCHEMA,
                wirePayloadBytes(OfflineNoteV2.issueInstruction(issue)),
            )).noritoEncoded()),
        )
        assertEquals(
            base64(audit.noritoEncoded()),
            base64(OfflineNoteV2.decodeAuditInstruction(rawInstructionPair(
                OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
                wirePayloadBytes(OfflineNoteV2.auditInstruction(audit)),
            )).noritoEncoded()),
        )
        assertEquals(
            base64(redeem.noritoEncoded()),
            base64(OfflineNoteV2.decodeRedeemInstruction(rawInstructionPair(
                OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
                wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem)),
            )).noritoEncoded()),
        )
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
    fun paymentTokenCodecRoundTripsJsonTextAndQrFrames() {
        val fixture = loadFixture()
        val payment = obj(fixture, "payment_token")
        val token = OfflineNoteV2PaymentToken(
            paymentRequestId = string(payment, "invoice_id"),
            tokenId = hexBytes(string(payment, "token_id")),
            audit = audit(fixture),
            createdAtMs = long(payment, "created_at_ms"),
        )

        val jsonDecoded = OfflineNoteV2PaymentTokenCodec.decodeJson(
            OfflineNoteV2PaymentTokenCodec.encodeJson(token)
        )
        assertEquals(token.tokenIdHex(), jsonDecoded.tokenIdHex())
        assertEquals(token.paymentRequestId, jsonDecoded.paymentRequestId)
        assertEquals(base64(token.audit.noritoEncoded()), base64(jsonDecoded.audit.noritoEncoded()))

        val text = OfflineNoteV2PaymentTokenCodec.encodeText(token)
        assertTrue(text.startsWith(OfflineNoteV2PaymentTokenCodec.TEXT_PREFIX))
        assertEquals(token.tokenIdHex(), OfflineNoteV2PaymentTokenCodec.decodeText(text).tokenIdHex())

        val frames = OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(
            token,
            OfflineQrStream.Options(chunkSize = 180, parityGroup = 2),
        )
        val decoder = OfflineQrStream.Decoder()
        var payload: ByteArray? = null
        for (frame in frames) {
            payload = decoder.ingest(frame).payload ?: payload
        }
        val qrDecoded = OfflineNoteV2PaymentTokenCodec.decodeQrPayload(assertNotNull(payload))
        assertEquals(token.tokenIdHex(), qrDecoded.tokenIdHex())
        assertEquals(base64(token.audit.noritoEncoded()), base64(qrDecoded.audit.noritoEncoded()))
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
    fun toriiIssuerClientBodySignsRefillAndIssuesWalletCommitment() {
        val fixture = loadFixture()
        val certificateJson = obj(obj(fixture, "payment_token"), "sender_key_certificate")
        val accountId = string(certificateJson, "account_id")
        val assetDefinitionId = assetDefinitionFromAssetId(string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"))
        val offlinePublicKey = "a5".repeat(32)
        val deviceBinding = OfflineNoteV2IssuerDeviceBinding(
            deviceId = "device-1",
            offlinePublicKey = offlinePublicKey,
            deviceBinding = linkedMapOf(
                "device_id" to "device-1",
                "offline_public_key" to offlinePublicKey,
                "signature_base64" to "nested-device-signature-is-not-body-auth",
            ),
        )
        val executor = OfflineIssuerExecutor(certificateJson)
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val client = ToriiOfflineNoteV2IssuerClient(
            canonicalAuth = ToriiCanonicalRequestAuth(accountId, keyPair.private),
            deviceBindingProvider = object : OfflineNoteV2IssuerDeviceBindingProvider {
                override fun currentDeviceBinding(
                    chainId: String,
                    accountId: String,
                    assetDefinitionId: String,
                ): OfflineNoteV2IssuerDeviceBinding = deviceBinding
            },
            executor = executor,
            baseUri = URI.create("https://torii.example"),
            clock = java.util.function.LongSupplier { 1_700_000_000_000L },
            nonceGenerator = SequenceIdGenerator(
                "operation-refill-1",
                "auth-refill-1",
                "auth-issue-1",
            ),
        )

        val context = client.prepareLoad("chain-1", accountId, assetDefinitionId, "5").join()
        assertEquals("operation-refill-1", context.operationId)
        assertEquals("lineage-1", context.lineageId)
        assertEquals(1L, context.localRevision)

        val commitment = ByteArray(32) { (it + 1).toByte() }
        val response = client.issueNote(
            OfflineNoteV2IssueRequest(
                chainId = "chain-1",
                accountId = accountId,
                assetDefinitionId = assetDefinitionId,
                assetId = "$assetDefinitionId#$accountId",
                amount = "5",
                loadContext = context,
                noteCommitment = commitment,
            )
        ).join()

        assertEquals(hex(commitment), hex(response.noteCommitment()))
        assertEquals("settlement-entry-hash", response.settlementEntryHashHex)
        assertEquals(2, executor.requests.size)
        assertEquals("/v1/offline/v2/keys/refill", executor.requests[0].uri.path)
        assertEquals("/v1/offline/v2/notes/issue", executor.requests[1].uri.path)
        for (request in executor.requests) {
            assertFalse(request.headers.keys.any { it.startsWith("X-Iroha-", ignoreCase = true) })
        }

        val refillBody = executor.requestBody(0)
        assertEquals(accountId, string(refillBody, "account_id"))
        assertEquals("operation-refill-1", string(refillBody, "operation_id"))
        assertEquals("auth-refill-1", string(refillBody, "nonce"))
        assertTrue(string(refillBody, "signature_base64").isNotBlank())
        assertEquals(
            "nested-device-signature-is-not-body-auth",
            string(obj(refillBody, "device_binding"), "signature_base64"),
        )

        val issueBody = executor.requestBody(1)
        assertEquals(hex(commitment), string(issueBody, "note_commitment"))
        assertEquals(0L, long(issueBody, "local_revision"))
        assertEquals("0", string(issueBody, "local_balance"))
        assertEquals("auth-issue-1", string(issueBody, "nonce"))
        assertNotNull(obj(issueBody, "lineage_state"))
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

    @Test
    fun walletSyncReconcilesPendingSpendChangeAndRedeemStates() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val payment = obj(fixture, "payment_token")
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val recipientCertificate = certificate(obj(payment, "recipient_key_certificate"))
        val senderStore = InMemoryOfflineNoteV2Store()
        senderStore.upsert(sourceWalletNote(fixture, senderCertificate))
        val resolutions = linkedMapOf<String, OfflineNoteV2WalletNoteState>(
            string(derivation, "source_note_commitment") to OfflineNoteV2WalletNoteState.SPENT,
            string(derivation, "change_output_commitment") to OfflineNoteV2WalletNoteState.SPENDABLE,
        )
        val syncResolver = RecordingSyncResolver(resolutions)
        val senderWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(chainIssue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            syncResolver = syncResolver,
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(
                listOf(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")),
                )
            ),
            clock = { 1_700_000_002_000L },
        )
        val recipientWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = string(payment, "recipient_account_id"),
            attestationProvider = StaticAttestationProvider(recipientCertificate),
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_100L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            amount = string(chainRedeem, "amount"),
        )
        senderWallet.pay(receiveRequest)
        senderWallet.sync().get()

        assertEquals(
            OfflineNoteV2WalletNoteState.SPENT,
            senderStore.findNote(hexBytes(string(derivation, "source_note_commitment")))?.state,
        )
        val spendableChange = senderStore.findNote(hexBytes(string(derivation, "change_output_commitment")))
        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, spendableChange?.state)
        assertEquals(
            listOf(
                string(derivation, "source_note_commitment"),
                string(derivation, "change_output_commitment"),
            ),
            syncResolver.resolvedCommitments,
        )

        resolutions[string(derivation, "change_output_commitment")] = OfflineNoteV2WalletNoteState.REDEEMED
        val redeeming = senderWallet.redeem(requireNotNull(spendableChange)).get()
        assertEquals(OfflineNoteV2WalletNoteState.REDEEM_PENDING, redeeming.state)

        senderWallet.sync().get()

        assertEquals(
            OfflineNoteV2WalletNoteState.REDEEMED,
            senderStore.findNote(hexBytes(string(derivation, "change_output_commitment")))?.state,
        )
    }

    @Test
    fun walletRejectsDuplicateTokenAndAlreadyPendingInputs() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val payment = obj(fixture, "payment_token")
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val recipientCertificate = certificate(obj(payment, "recipient_key_certificate"))
        val senderStore = InMemoryOfflineNoteV2Store()
        senderStore.upsert(sourceWalletNote(fixture, senderCertificate))
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
            clock = { 1_700_000_002_200L },
        )
        val recipientWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = string(payment, "recipient_account_id"),
            attestationProvider = StaticAttestationProvider(recipientCertificate),
            transactionSubmitter = RecordingTransactionSubmitter(),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_300L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            amount = string(chainRedeem, "amount"),
        )
        val token = senderWallet.pay(receiveRequest)

        assertFailsWith<IllegalArgumentException> {
            senderWallet.pay(receiveRequest)
        }

        val accepted = recipientWallet.accept(token).join()
        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, accepted.state)
        assertFutureFails(recipientWallet.accept(token))
    }

    @Test
    fun walletSyncReconcilesFailedAuditAndRedeemOutcomes() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val chainIssue = obj(chain, "issue")
        val chainRedeem = obj(chain, "redeem")
        val payment = obj(fixture, "payment_token")
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val recipientCertificate = certificate(obj(payment, "recipient_key_certificate"))
        val senderStore = InMemoryOfflineNoteV2Store()
        senderStore.upsert(sourceWalletNote(fixture, senderCertificate))
        val senderResolutions = linkedMapOf(
            string(derivation, "source_note_commitment") to OfflineNoteV2WalletNoteState.SPENDABLE,
            string(derivation, "change_output_commitment") to OfflineNoteV2WalletNoteState.CANCELLED,
        )
        val senderWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(chainIssue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            store = senderStore,
            transactionSubmitter = RecordingTransactionSubmitter(),
            syncResolver = RecordingSyncResolver(senderResolutions),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(
                listOf(
                    hexBytes(string(derivation, "token_nonce_hex")),
                    hexBytes(string(derivation, "change_note_secret_hex")),
                )
            ),
            clock = { 1_700_000_002_400L },
        )
        val recipientStore = InMemoryOfflineNoteV2Store()
        val recipientResolutions = linkedMapOf(
            string(derivation, "recipient_output_commitment") to OfflineNoteV2WalletNoteState.CANCELLED,
        )
        val recipientWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = string(payment, "recipient_account_id"),
            attestationProvider = StaticAttestationProvider(recipientCertificate),
            store = recipientStore,
            transactionSubmitter = RejectingTransactionSubmitter(),
            syncResolver = RecordingSyncResolver(recipientResolutions),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(listOf(hexBytes(string(derivation, "recipient_note_secret_hex")))),
            idGenerator = FixedIdGenerator(string(derivation, "payment_request_id")),
            clock = { 1_700_000_002_500L },
        )

        val receiveRequest = recipientWallet.prepareReceive(
            assetDefinitionId = assetDefinitionFromAssetId(string(chainIssue, "asset_id")),
            amount = string(chainRedeem, "amount"),
        )
        val token = senderWallet.pay(receiveRequest)

        assertFutureFails(recipientWallet.accept(token))
        assertEquals(
            OfflineNoteV2WalletNoteState.RECEIVE_PENDING,
            recipientStore.findNote(hexBytes(string(derivation, "recipient_output_commitment")))?.state,
        )

        senderWallet.sync().join()
        recipientWallet.sync().join()

        assertEquals(
            OfflineNoteV2WalletNoteState.SPENDABLE,
            senderStore.findNote(hexBytes(string(derivation, "source_note_commitment")))?.state,
        )
        assertEquals(
            OfflineNoteV2WalletNoteState.CANCELLED,
            senderStore.findNote(hexBytes(string(derivation, "change_output_commitment")))?.state,
        )
        assertEquals(
            OfflineNoteV2WalletNoteState.CANCELLED,
            recipientStore.findNote(hexBytes(string(derivation, "recipient_output_commitment")))?.state,
        )

        val redeemStore = InMemoryOfflineNoteV2Store()
        val redeemNote = sourceWalletNote(fixture, senderCertificate)
        redeemStore.upsert(redeemNote)
        val redeemWallet = OfflineNoteV2Wallet(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(chainIssue, "asset_id")),
            attestationProvider = StaticAttestationProvider(senderCertificate),
            store = redeemStore,
            transactionSubmitter = RejectingTransactionSubmitter(),
            syncResolver = RecordingSyncResolver(
                mapOf(string(derivation, "source_note_commitment") to OfflineNoteV2WalletNoteState.SPENDABLE)
            ),
            proofProvider = BindingProofProvider,
            randomSource = QueueRandomSource(emptyList()),
            clock = { 1_700_000_002_600L },
        )

        assertFutureFails(redeemWallet.redeem(redeemNote))
        assertEquals(
            OfflineNoteV2WalletNoteState.REDEEM_PENDING,
            redeemStore.findNote(hexBytes(string(derivation, "source_note_commitment")))?.state,
        )

        redeemWallet.sync().join()

        assertEquals(
            OfflineNoteV2WalletNoteState.SPENDABLE,
            redeemStore.findNote(hexBytes(string(derivation, "source_note_commitment")))?.state,
        )
    }

    @Test
    fun outcomeIndexResolvesCommittedAndRejectedExplorerInstructions() {
        val fixture = loadFixture()
        val chain = obj(fixture, "chain_vectors")
        val derivation = obj(chain, "derivation")
        val issueVector = obj(chain, "issue")
        val payment = obj(fixture, "payment_token")
        val audit = audit(fixture)
        val redeem = redeem(fixture)
        val senderCertificate = certificate(obj(payment, "sender_key_certificate"))
        val recipientCertificate = certificate(obj(payment, "recipient_key_certificate"))
        val sourceSpendPending = sourceWalletNote(fixture, senderCertificate)
            .withState(OfflineNoteV2WalletNoteState.SPEND_PENDING, 1_700_000_003_000L)
        val changePending = OfflineNoteV2WalletNote(
            chainId = string(derivation, "chain_id"),
            accountId = accountFromAssetId(string(issueVector, "asset_id")),
            assetId = string(issueVector, "asset_id"),
            amount = string(payment, "change_amount"),
            keyCertificate = senderCertificate,
            noteCommitment = hexBytes(string(derivation, "change_output_commitment")),
            noteSecret = hexBytes(string(derivation, "change_note_secret_hex")),
            origin = OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                paymentRequestId = string(derivation, "payment_request_id"),
                outputIndex = 1,
            ),
            state = OfflineNoteV2WalletNoteState.CHANGE_PENDING,
            createdAtMs = 1_700_000_003_000L,
            updatedAtMs = 1_700_000_003_000L,
        )
        val redeemPending = OfflineNoteV2WalletNote(
            chainId = string(derivation, "chain_id"),
            accountId = string(payment, "recipient_account_id"),
            assetId = string(issueVector, "asset_id"),
            amount = string(obj(chain, "redeem"), "amount"),
            keyCertificate = recipientCertificate,
            noteCommitment = redeem.sourceNoteCommitment(),
            noteSecret = hexBytes(string(derivation, "recipient_note_secret_hex")),
            origin = OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                paymentRequestId = string(derivation, "payment_request_id"),
                outputIndex = 0,
            ),
            state = OfflineNoteV2WalletNoteState.REDEEM_PENDING,
            createdAtMs = 1_700_000_003_100L,
            updatedAtMs = 1_700_000_003_100L,
        )

        val committed = OfflineNoteV2OutcomeIndex.fromExplorerOutcomes(
            listOf(
                OfflineNoteV2ExplorerInstructionOutcome(
                    kind = OfflineNoteV2OutcomeIndex.KIND_AUDIT,
                    transactionStatus = "Committed",
                    transactionHashHex = "audit-tx",
                    encodedInstruction = rawInstructionPair(
                        OfflineNoteV2.AUDIT_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.auditInstruction(audit)),
                    ),
                ),
                OfflineNoteV2ExplorerInstructionOutcome(
                    kind = OfflineNoteV2OutcomeIndex.KIND_REDEEM,
                    transactionStatus = "Committed",
                    transactionHashHex = "redeem-tx",
                    encodedInstruction = rawInstructionPair(
                        OfflineNoteV2.REDEEM_INSTRUCTION_SCHEMA,
                        wirePayloadBytes(OfflineNoteV2.redeemInstruction(redeem)),
                    ),
                ),
            )
        )
        assertEquals(OfflineNoteV2WalletNoteState.SPENT, committed.resolve(sourceSpendPending)?.state)
        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, committed.resolve(changePending)?.state)
        assertEquals(OfflineNoteV2WalletNoteState.REDEEMED, committed.resolve(redeemPending)?.state)

        val rejected = OfflineNoteV2OutcomeIndex()
            .recordRejectedAudit(audit, "audit-rejected")
            .recordRejectedRedeem(redeem, "redeem-rejected")
        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, rejected.resolve(sourceSpendPending)?.state)
        assertEquals(OfflineNoteV2WalletNoteState.CANCELLED, rejected.resolve(changePending)?.state)
        assertEquals(OfflineNoteV2WalletNoteState.SPENDABLE, rejected.resolve(redeemPending)?.state)
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

    private class SequenceIdGenerator(
        private vararg val ids: String,
    ) : OfflineNoteV2IdGenerator {
        private var index = 0

        override fun nextId(prefix: String): String {
            require(index < ids.size) { "test id generator exhausted" }
            return ids[index++]
        }
    }

    private inner class OfflineIssuerExecutor(
        private val certificateJson: Map<String, Any?>,
    ) : HttpTransportExecutor {
        val requests = ArrayList<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val body = requestBody(request)
            val response = when (request.uri.path) {
                "/v1/offline/v2/keys/refill" -> linkedMapOf<String, Any?>(
                    "operation_id" to string(body, "operation_id"),
                    "lineage_state" to lineageState(0, "0"),
                    "key_certificate" to certificateWithExpiry(),
                    "key_certificates" to listOf(certificateWithExpiry()),
                )
                "/v1/offline/v2/notes/issue" -> linkedMapOf<String, Any?>(
                    "operation_id" to string(body, "operation_id"),
                    "settlement" to linkedMapOf("entry_hash" to "settlement-entry-hash"),
                    "lineage_state" to lineageState(1, "5"),
                    "local_balance" to "5",
                    "locked_balance" to "0",
                    "local_revision" to 1L,
                    "local_state_hash" to "lineage-state-hash",
                    "issued_note_commitment" to string(body, "note_commitment"),
                    "key_certificate" to certificateWithExpiry(),
                    "key_certificates" to listOf(certificateWithExpiry()),
                )
                else -> throw IllegalStateException("unexpected path ${request.uri.path}")
            }
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(JsonEncoder.encode(response).toByteArray(StandardCharsets.UTF_8))
                    .build()
            )
        }

        fun requestBody(index: Int): Map<String, Any?> = requestBody(requests[index])

        private fun requestBody(request: TransportRequest): Map<String, Any?> {
            @Suppress("UNCHECKED_CAST")
            return JsonParser.parse(String(request.body, StandardCharsets.UTF_8)) as Map<String, Any?>
        }

        private fun certificateWithExpiry(): Map<String, Any?> {
            val copy = LinkedHashMap(certificateJson)
            copy["expires_at_ms"] = 1_700_000_060_000L
            return copy
        }

        private fun lineageState(revision: Long, balance: String): Map<String, Any?> =
            linkedMapOf(
                "lineage_id" to "lineage-1",
                "server_revision" to revision,
                "pending_local_revision" to revision,
                "balance" to balance,
                "locked_balance" to "0",
                "authorization" to linkedMapOf(
                    "expires_at_ms" to 1_700_000_060_000L,
                ),
            )
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

    private class RejectingTransactionSubmitter : OfflineNoteV2TransactionSubmitter {
        override fun submitAudit(audit: OfflineNoteV2.AuditBundleV2): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(409, byteArrayOf(), "rejected"))

        override fun submitRedeem(redemption: OfflineNoteV2.RedeemV2): CompletableFuture<ClientResponse> =
            CompletableFuture.completedFuture(ClientResponse(409, byteArrayOf(), "rejected"))
    }

    private class RecordingSyncResolver(
        private val resolutions: Map<String, OfflineNoteV2WalletNoteState>,
    ) : OfflineNoteV2SyncResolver {
        val resolvedCommitments = ArrayList<String>()

        override fun resolvePendingNote(
            note: OfflineNoteV2WalletNote,
        ): CompletableFuture<OfflineNoteV2SyncResolution?> {
            val commitment = note.noteCommitmentHex()
            resolvedCommitments.add(commitment)
            return CompletableFuture.completedFuture(
                resolutions[commitment]?.let { OfflineNoteV2SyncResolution(it, "tx-$commitment") }
            )
        }
    }

    private fun wirePayloadBytes(instruction: org.hyperledger.iroha.sdk.core.model.InstructionBox): ByteArray =
        (instruction.payload as WirePayload).payloadBytes

    private fun rawInstructionPair(wireName: String, wirePayload: ByteArray, compact: Boolean = true): ByteArray {
        val out = ByteArrayOutputStream()
        writeField(out, encodeString(wireName, compact), compact)
        writeField(out, encodeBytesVec(wirePayload), compact)
        return out.toByteArray()
    }

    private fun encodeString(value: String, compact: Boolean): ByteArray {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        val out = ByteArrayOutputStream()
        writeLength(out, bytes.size.toLong(), compact)
        out.write(bytes)
        return out.toByteArray()
    }

    private fun encodeBytesVec(value: ByteArray): ByteArray {
        val out = ByteArrayOutputStream()
        writeUInt64(out, value.size.toLong())
        out.write(value)
        return out.toByteArray()
    }

    private fun writeField(out: ByteArrayOutputStream, payload: ByteArray, compact: Boolean) {
        writeLength(out, payload.size.toLong(), compact)
        out.write(payload)
    }

    private fun writeLength(out: ByteArrayOutputStream, value: Long, compact: Boolean) {
        if (!compact) {
            writeUInt64(out, value)
            return
        }
        var remaining = value
        while (remaining >= 0x80) {
            out.write(((remaining and 0x7f) or 0x80).toInt())
            remaining = remaining ushr 7
        }
        out.write(remaining.toInt())
    }

    private fun writeUInt64(out: ByteArrayOutputStream, value: Long) {
        var remaining = value
        repeat(8) {
            out.write((remaining and 0xff).toInt())
            remaining = remaining ushr 8
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

    private fun assertFutureFails(future: CompletableFuture<*>) {
        assertFailsWith<CompletionException> {
            future.join()
        }
    }
}
