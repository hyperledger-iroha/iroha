package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

private const val CHAIN_ID = "00000000"
private const val CREATION_TIME_MS = 1_745_000_000_000L
private const val NONCE = 7
private const val TTL_MS = 60_000L
private const val SIGNATURE_LENGTH = 64

private const val MODEL_ISSUE_SCHEMA = "iroha_data_model::offline::model::OfflineNoteIssue"
private const val MODEL_REDEEM_SCHEMA = "iroha_data_model::offline::model::OfflineNoteRedeem"
private const val MODEL_AUDIT_SCHEMA = "iroha_data_model::offline::model::OfflineNoteAuditBundle"

class OfflineNoteTransactionEncoderTest {

    @Test
    fun buildIssueOfflineNote_producesCorrectlyFramedInstruction() {
        val fixture = loadFixture()
        val expectedModelBody = modelBodyFromFixture(fixture, "issue", MODEL_ISSUE_SCHEMA)
        val request = OfflineNoteTransactionEncoder.IssueOfflineNoteRequest(
            chainId = CHAIN_ID,
            authority = senderAuthority(fixture),
            issue = issue(fixture),
            ttlMs = TTL_MS,
            nonce = NONCE,
        )
        val signed = encoder().buildIssueOfflineNote(request, signer(), CREATION_TIME_MS)

        val box = singleInstruction(signed)
        assertWireFraming(
            box = box,
            expectedWireName = INSTRUCTION_ISSUE_WIRE_NAME,
            modelSchemaPath = MODEL_ISSUE_SCHEMA,
            expectedModelBody = expectedModelBody,
        )
    }

    @Test
    fun buildRedeemOfflineNote_producesCorrectlyFramedInstruction() {
        val fixture = loadFixture()
        val expectedModelBody = modelBodyFromFixture(fixture, "redeem", MODEL_REDEEM_SCHEMA)
        val request = OfflineNoteTransactionEncoder.RedeemOfflineNoteRequest(
            chainId = CHAIN_ID,
            authority = recipientAuthority(fixture),
            redemption = redeem(fixture),
            ttlMs = TTL_MS,
            nonce = NONCE,
        )
        val signed = encoder().buildRedeemOfflineNote(request, signer(), CREATION_TIME_MS)

        val box = singleInstruction(signed)
        assertWireFraming(
            box = box,
            expectedWireName = INSTRUCTION_REDEEM_WIRE_NAME,
            modelSchemaPath = MODEL_REDEEM_SCHEMA,
            expectedModelBody = expectedModelBody,
        )
    }

    @Test
    fun buildAuditOfflineNote_producesCorrectlyFramedInstruction() {
        val fixture = loadFixture()
        val expectedModelBody = modelBodyFromFixture(fixture, "audit", MODEL_AUDIT_SCHEMA)
        val request = OfflineNoteTransactionEncoder.AuditOfflineNoteRequest(
            chainId = CHAIN_ID,
            authority = senderAuthority(fixture),
            audit = audit(fixture),
            ttlMs = TTL_MS,
            nonce = NONCE,
        )
        val signed = encoder().buildAuditOfflineNote(request, signer(), CREATION_TIME_MS)

        val box = singleInstruction(signed)
        assertWireFraming(
            box = box,
            expectedWireName = INSTRUCTION_AUDIT_WIRE_NAME,
            modelSchemaPath = MODEL_AUDIT_SCHEMA,
            expectedModelBody = expectedModelBody,
        )
    }

    @Test
    fun directOfflineNoteInstructions_produceCorrectlyFramedInstructions() {
        val fixture = loadFixture()

        assertWireFraming(
            box = OfflineNote.issueInstruction(issue(fixture)),
            expectedWireName = INSTRUCTION_ISSUE_WIRE_NAME,
            modelSchemaPath = MODEL_ISSUE_SCHEMA,
            expectedModelBody = modelBodyFromFixture(fixture, "issue", MODEL_ISSUE_SCHEMA),
        )
        assertWireFraming(
            box = OfflineNote.redeemInstruction(redeem(fixture)),
            expectedWireName = INSTRUCTION_REDEEM_WIRE_NAME,
            modelSchemaPath = MODEL_REDEEM_SCHEMA,
            expectedModelBody = modelBodyFromFixture(fixture, "redeem", MODEL_REDEEM_SCHEMA),
        )
        assertWireFraming(
            box = OfflineNote.auditInstruction(audit(fixture)),
            expectedWireName = INSTRUCTION_AUDIT_WIRE_NAME,
            modelSchemaPath = MODEL_AUDIT_SCHEMA,
            expectedModelBody = modelBodyFromFixture(fixture, "audit", MODEL_AUDIT_SCHEMA),
        )
    }

    @Test
    fun buildRedeemOfflineNote_rejectsMismatchedProofBinding() {
        val fixture = loadFixture()
        val redeem = redeem(fixture)
        val forged = OfflineNote.Redeem(
            sourceNoteCommitment = redeem.sourceNoteCommitment(),
            inputNullifiers = redeem.inputNullifiers(),
            senderKeyCertificate = redeem.senderKeyCertificate,
            recipient = redeem.recipient,
            assetId = redeem.assetId,
            amount = redeem.amount,
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = OfflineNote.hash("forged-redeem-public-inputs".toByteArray()),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "offline-vector-redeem-proof".toByteArray(),
                ),
            ),
        )
        val request = OfflineNoteTransactionEncoder.RedeemOfflineNoteRequest(
            chainId = CHAIN_ID,
            authority = recipientAuthority(fixture),
            redemption = forged,
        )

        assertFailsWith<IllegalArgumentException> {
            encoder().buildRedeemOfflineNote(request, signer(), CREATION_TIME_MS)
        }
    }

    @Test
    fun buildAuditOfflineNote_rejectsMismatchedProofBinding() {
        val fixture = loadFixture()
        val audit = audit(fixture)
        val forged = OfflineNote.AuditBundle(
            tokenId = audit.tokenId(),
            senderKeyCertificate = audit.senderKeyCertificate,
            inputNullifiers = audit.inputNullifiers(),
            inputClaims = audit.inputClaims,
            outputCommitments = audit.outputCommitments(),
            outputClaims = audit.outputClaims,
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = OfflineNote.hash("forged-audit-public-inputs".toByteArray()),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "offline-vector-audit-proof".toByteArray(),
                ),
            ),
        )
        val request = OfflineNoteTransactionEncoder.AuditOfflineNoteRequest(
            chainId = CHAIN_ID,
            authority = senderAuthority(fixture),
            audit = forged,
        )

        assertFailsWith<IllegalArgumentException> {
            encoder().buildAuditOfflineNote(request, signer(), CREATION_TIME_MS)
        }
    }

    @Test
    fun buildIssueOfflineNote_isDeterministic() {
        val fixture = loadFixture()
        val request = OfflineNoteTransactionEncoder.IssueOfflineNoteRequest(
            chainId = CHAIN_ID,
            authority = senderAuthority(fixture),
            issue = issue(fixture),
            ttlMs = TTL_MS,
            nonce = NONCE,
        )
        val first = encoder().buildIssueOfflineNote(request, signer(), CREATION_TIME_MS)
        val second = encoder().buildIssueOfflineNote(request, signer(), CREATION_TIME_MS)

        assertContentEquals(first.encodedPayload(), second.encodedPayload(), "encoded payload must be deterministic")
        assertContentEquals(first.signature(), second.signature(), "signature must be deterministic given the same signer")
        assertContentEquals(first.publicKey(), second.publicKey(), "public key must be stable across builds")
        assertEquals(first.schemaName(), second.schemaName(), "schema name must be stable")
    }

    private fun assertWireFraming(
        box: InstructionBox,
        expectedWireName: String,
        modelSchemaPath: String,
        expectedModelBody: ByteArray,
    ) {
        val payload = box.payload
        assertTrue(payload is WirePayload, "instruction payload must be a WirePayload, got ${payload::class.simpleName}")
        assertEquals(expectedWireName, payload.wireName, "instruction wire name must match")

        val framed = payload.payloadBytes
        val instructionSchema = SchemaHash.hash16(expectedWireName)
        val modelSchema = SchemaHash.hash16(modelSchemaPath)
        val decoded = NoritoHeader.decode(framed, expectedHash = instructionSchema)
        assertEquals(
            NoritoHeader.COMPACT_LEN,
            decoded.header.flags,
            "outer instruction frame must use the chain's default v1 flags (COMPACT_LEN)",
        )
        assertEquals(NoritoHeader.COMPRESSION_NONE, decoded.header.compression, "outer instruction frame must be uncompressed")
        assertContentEquals(instructionSchema, decoded.header.schemaHash, "frame schema hash must equal the instruction wire name hash")
        assertTrue(
            !decoded.header.schemaHash.contentEquals(modelSchema),
            "frame schema hash must NOT equal the inner model schema hash ($modelSchemaPath)",
        )

        // The chain expects the outer ISI single-field newtype wrapper layout:
        // `[compact-varint(inner.len)][inner_archive_bytes]`. Verify the wallet's
        // bytes follow that contract — varint then the raw model body verbatim.
        val innerDecoder = NoritoDecoder(decoded.payload, decoded.header.flags, 0)
        val declaredLength = innerDecoder.readLength(compact = true)
        assertEquals(
            expectedModelBody.size.toLong(),
            declaredLength,
            "inner field varint length must equal the raw model body size",
        )
        val innerField = innerDecoder.readBytes(declaredLength.toInt())
        assertEquals(0, innerDecoder.remaining(), "no trailing bytes after inner field")
        assertContentEquals(
            expectedModelBody,
            innerField,
            "inner field bytes must equal the raw model body extracted from the fixture",
        )
    }

    private fun modelBodyFromFixture(
        fixture: Map<String, Any?>,
        chainKey: String,
        modelSchemaPath: String,
    ): ByteArray {
        val full = base64Bytes(string(obj(obj(fixture, "chain_vectors"), chainKey), "norito_base64"))
        return NoritoHeader.decode(full, expectedHash = SchemaHash.hash16(modelSchemaPath)).payload
    }

    private fun singleInstruction(signed: org.hyperledger.iroha.sdk.tx.SignedTransaction): InstructionBox {
        val payload = NoritoJavaCodecAdapter().decodeTransaction(signed.encodedPayload())
        val executable = payload.executable
        assertTrue(executable is Executable.Instructions, "executable must carry instructions")
        assertEquals(1, executable.instructions.size, "exactly one instruction must be carried")
        assertEquals(SIGNATURE_LENGTH, signed.signature().size, "signer must produce a 64-byte signature")
        return executable.instructions.single()
    }

    private fun encoder(): OfflineNoteTransactionEncoder =
        OfflineNoteTransactionEncoder(TransactionBuilder(NoritoJavaCodecAdapter()))

    private fun senderAuthority(fixture: Map<String, Any?>): String =
        string(obj(fixture, "payment_token"), "sender_account_id")

    private fun recipientAuthority(fixture: Map<String, Any?>): String =
        string(obj(fixture, "payment_token"), "recipient_account_id")

    private fun signer(): Signer = DeterministicSigner(
        publicKey = ByteArray(32) { (it + 1).toByte() },
        signaturePrefix = "offline-note-test",
    )

    private fun issue(fixture: Map<String, Any?>): OfflineNote.Issue {
        val chainIssue = obj(obj(fixture, "chain_vectors"), "issue")
        return OfflineNote.Issue(
            noteCommitment = hexBytes(string(chainIssue, "note_commitment")),
            keyCertificate = certificate(obj(obj(fixture, "payment_token"), "sender_key_certificate")),
            assetId = string(chainIssue, "asset_id"),
            amount = string(chainIssue, "amount"),
        )
    }

    private fun redeem(fixture: Map<String, Any?>): OfflineNote.Redeem {
        val vector = obj(obj(fixture, "chain_vectors"), "redeem")
        val payment = obj(fixture, "payment_token")
        return OfflineNote.Redeem(
            sourceNoteCommitment = hexBytes(string(vector, "source_note_commitment")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            senderKeyCertificate = certificate(obj(payment, "recipient_key_certificate")),
            recipient = string(payment, "recipient_account_id"),
            assetId = string(vector, "asset_id"),
            amount = string(vector, "amount"),
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "offline-vector-redeem-proof".toByteArray(),
                ),
            ),
        )
    }

    private fun audit(fixture: Map<String, Any?>): OfflineNote.AuditBundle {
        val vector = obj(obj(fixture, "chain_vectors"), "audit")
        val payment = obj(fixture, "payment_token")
        return OfflineNote.AuditBundle(
            tokenId = hexBytes(string(vector, "token_id")),
            senderKeyCertificate = certificate(obj(payment, "sender_key_certificate")),
            inputNullifiers = list(vector, "input_nullifiers").map { hexBytes(it as String) },
            inputClaims = list(payment, "input_claims").map { issuedClaim(it as Map<String, Any?>) },
            outputCommitments = list(vector, "output_commitments").map { hexBytes(it as String) },
            outputClaims = list(payment, "output_claims").map { auditOutputClaim(it as Map<String, Any?>) },
            recursiveProof = OfflineNote.RecursiveProof(
                publicInputsHash = hexBytes(string(vector, "public_inputs_hash")),
                proof = OfflineNote.ProofBox(
                    OfflineNote.RECURSIVE_BACKEND,
                    "offline-vector-audit-proof".toByteArray(),
                ),
            ),
        )
    }

    private fun certificate(json: Map<String, Any?>): OfflineNote.KeyCertificate =
        OfflineNote.KeyCertificate(
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

    private fun issuedClaim(json: Map<String, Any?>): OfflineNote.IssuedClaim =
        OfflineNote.IssuedClaim(
            domain = string(json, "domain"),
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificatePayloadHash = hexBytes(string(json, "key_certificate_payload_hash")),
            assetId = string(json, "asset_id"),
            amount = string(json, "amount"),
        )

    private fun auditOutputClaim(json: Map<String, Any?>): OfflineNote.AuditOutputClaim =
        OfflineNote.AuditOutputClaim(
            noteCommitment = hexBytes(string(json, "note_commitment")),
            keyCertificate = certificate(obj(json, "key_certificate")),
            assetId = "${string(json, "asset_definition_id")}#${string(json, "account_id")}",
            amount = string(json, "amount"),
        )

    private fun loadFixture(): Map<String, Any?> {
        val path = Paths.get("..", "..", "fixtures", "offline", "interop_contract.json")
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
    private fun nullableInt(map: Map<String, Any?>, key: String): Int? = (map[key] as Number?)?.toInt()

    private fun base64Bytes(value: String): ByteArray = Base64.getDecoder().decode(value)

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

private class DeterministicSigner(
    publicKey: ByteArray,
    private val signaturePrefix: String,
) : Signer {
    private val _publicKey = publicKey.copyOf()

    override fun sign(message: ByteArray): ByteArray {
        val md = MessageDigest.getInstance("SHA-256")
        val first = md.digest(message)
        val tail = ByteArray(signaturePrefix.length + first.size)
        System.arraycopy(first, 0, tail, 0, first.size)
        System.arraycopy(signaturePrefix.toByteArray(), 0, tail, first.size, signaturePrefix.length)
        val second = md.digest(tail)
        val out = ByteArray(SIGNATURE_LENGTH)
        System.arraycopy(first, 0, out, 0, first.size)
        System.arraycopy(second, 0, out, first.size, second.size)
        return out
    }

    override fun publicKey(): ByteArray = _publicKey.copyOf()

    override fun algorithm(): String = "Ed25519"
}
