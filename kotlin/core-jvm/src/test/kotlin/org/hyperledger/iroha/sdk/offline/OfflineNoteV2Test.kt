package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import java.util.Base64
import java.util.Locale
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
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
    fun proofVerifierAndHashValidationRejectsMalformedValues() {
        val publicInputsHash = audit(loadFixture()).publicInputsHash()
        val trimmedProof = OfflineNoteV2.ProofBox("  ${OfflineNoteV2.RECURSIVE_BACKEND}  ", byteArrayOf(1))
        assertEquals(OfflineNoteV2.RECURSIVE_BACKEND, trimmedProof.backend)

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.ProofBox(" \n ", byteArrayOf(1))
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf())
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = ByteArray(31) { 1 },
                proof = OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf(1)),
            )
        }
        val nonCanonicalHash = publicInputsHash.copyOf()
        nonCanonicalHash[31] = (nonCanonicalHash[31].toInt() and 0xfe).toByte()
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RecursiveProofV2(
                publicInputsHash = nonCanonicalHash,
                proof = OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf(1)),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "", name = "vk")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "halo2:ipa", name = "vk")
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.VerifyingKeyIdReference(backend = "halo2/ipa", name = "bad:vk")
        }
    }

    @Test
    fun certificateValidationRejectsMalformedValues() {
        val certJson = obj(obj(loadFixture(), "payment_token"), "sender_key_certificate")
        val publicKey = base64Bytes(string(certJson, "public_key"))
        val assertionPublicKey = base64Bytes(string(certJson, "assertion_public_key"))
        val issuerSignature = base64Bytes(string(certJson, "issuer_signature_base64"))

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                version = 1,
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = false,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey.copyOfRange(0, 31),
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = -1,
                oneUse = true,
                issuerSignature = issuerSignature,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.KeyCertificateV2(
                platform = string(certJson, "platform"),
                keyId = string(certJson, "key_id"),
                deviceId = string(certJson, "device_id"),
                accountId = string(certJson, "account_id"),
                publicKey = publicKey,
                assertionScheme = string(certJson, "assertion_scheme"),
                assertionKeyAlgorithm = string(certJson, "assertion_key_algorithm"),
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = nullableInt(certJson, "assertion_usage_count_limit"),
                oneUse = true,
                issuerSignature = issuerSignature.copyOfRange(0, 63),
            )
        }
    }

    @Test
    fun auditBundleRejectsInvalidShapesAndUncommittedOutputs() {
        val audit = audit(loadFixture())
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = emptyList(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = emptyList(),
                outputCommitments = audit.outputCommitments(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers() + audit.inputNullifiers()[0],
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = emptyList(),
                outputClaims = audit.outputClaims,
                recursiveProof = audit.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = emptyList(),
                recursiveProof = audit.recursiveProof,
            )
        }
        val uncommittedOutput = OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = OfflineNoteV2.hash("uncommitted-output".toByteArray()),
            keyCertificate = audit.outputClaims[0].keyCertificate,
            assetId = audit.outputClaims[0].assetId,
            amount = audit.outputClaims[0].amount,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.AuditBundleV2(
                tokenId = audit.tokenId(),
                senderKeyCertificate = audit.senderKeyCertificate,
                inputNullifiers = audit.inputNullifiers(),
                inputClaims = audit.inputClaims,
                outputCommitments = audit.outputCommitments(),
                outputClaims = listOf(uncommittedOutput),
                recursiveProof = audit.recursiveProof,
            )
        }
    }

    @Test
    fun issueRedeemPublicInputsAndInstancesRejectMalformedValues() {
        val fixture = loadFixture()
        val cert = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate"))
        val audit = audit(fixture)
        val redeem = redeem(fixture)

        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssueV2(
                noteCommitment = ByteArray(31) { 1 },
                keyCertificate = cert,
                assetId = string(obj(obj(fixture, "chain_vectors"), "issue"), "asset_id"),
                amount = "5",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssueV2(
                noteCommitment = redeem.sourceNoteCommitment(),
                keyCertificate = cert,
                assetId = "cash#branch.sbp",
                amount = "5",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.IssueV2(
                noteCommitment = redeem.sourceNoteCommitment(),
                keyCertificate = cert,
                assetId = redeem.assetId,
                amount = "not-a-number",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemV2(
                sourceNoteCommitment = redeem.sourceNoteCommitment(),
                inputNullifiers = emptyList(),
                senderKeyCertificate = redeem.senderKeyCertificate,
                recipient = redeem.recipient,
                assetId = redeem.assetId,
                amount = redeem.amount,
                recursiveProof = redeem.recursiveProof,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                sourceNoteCommitment = ByteArray(31) { 1 },
                inputNullifiers = redeem.inputNullifiers(),
                keyCertificatePayloadHash = redeem.senderKeyCertificate.payloadHash(),
                recipient = redeem.recipient,
                assetId = redeem.assetId,
                amount = redeem.amount,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                sourceNoteCommitment = redeem.sourceNoteCommitment(),
                inputNullifiers = redeem.inputNullifiers(),
                keyCertificatePayloadHash = ByteArray(31) { 1 },
                recipient = redeem.recipient,
                assetId = redeem.assetId,
                amount = redeem.amount,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.RedeemPublicInputsV2(
                sourceNoteCommitment = redeem.sourceNoteCommitment(),
                inputNullifiers = redeem.inputNullifiers(),
                keyCertificatePayloadHash = redeem.senderKeyCertificate.payloadHash(),
                recipient = "${redeem.recipient}@bad",
                assetId = redeem.assetId,
                amount = redeem.amount,
            )
        }

        val overLimitOutput = OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = OfflineNoteV2.hash("third-output".toByteArray()),
            keyCertificate = audit.outputClaims[0].keyCertificate,
            assetId = audit.outputClaims[0].assetId,
            amount = "0",
        )
        val tooManyOutputs = OfflineNoteV2.AuditBundleV2(
            tokenId = audit.tokenId(),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments() + overLimitOutput.noteCommitment(),
            outputClaims = audit.outputClaims + overLimitOutput,
            recursiveProof = audit.recursiveProof,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.InstanceBuilder.auditInstanceValues(tooManyOutputs)
        }

        val unconservedOutput = OfflineNoteV2.AuditOutputClaimV2(
            noteCommitment = audit.outputClaims[0].noteCommitment(),
            keyCertificate = audit.outputClaims[0].keyCertificate,
            assetId = audit.outputClaims[0].assetId,
            amount = "6",
        )
        val unconservedAudit = OfflineNoteV2.AuditBundleV2(
            tokenId = audit.tokenId(),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments(),
            outputClaims = listOf(unconservedOutput, audit.outputClaims[1]),
            recursiveProof = audit.recursiveProof,
        )
        assertFailsWith<IllegalArgumentException> {
            OfflineNoteV2.InstanceBuilder.auditInstanceValues(unconservedAudit)
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
            inputClaims = list(payment, "input_claims").map { issuedClaim(objValue(it, "input claim")) },
            outputCommitments = list(vector, "output_commitments").map { hexBytes(it as String) },
            outputClaims = list(payment, "output_claims").map { auditOutputClaim(objValue(it, "output claim")) },
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

    private fun objValue(value: Any?, label: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return value as? Map<String, Any?> ?: error("$label must be an object")
    }

    private fun string(map: Map<String, Any?>, key: String): String = map[key] as String
    private fun bool(map: Map<String, Any?>, key: String): Boolean = map[key] as Boolean
    private fun int(map: Map<String, Any?>, key: String): Int = (map[key] as Number).toInt()
    private fun nullableInt(map: Map<String, Any?>, key: String): Int? = (map[key] as Number?)?.toInt()

    private fun base64(bytes: ByteArray): String = Base64.getEncoder().encodeToString(bytes)
    private fun base64Bytes(value: String): ByteArray = Base64.getDecoder().decode(value)

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02x".format(it.toInt() and 0xFF) }

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
