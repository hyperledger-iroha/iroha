package org.hyperledger.iroha.sdk.alias

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder
import org.hyperledger.iroha.sdk.tx.norito.TransactionPayloadAdapter

/** Cross-language golden coverage for exact prepared-transaction authentication. */
class PreparedTransactionSignatureFixtureTest {
    @Test
    fun sharedGoldenAuthenticatesPreparedProofRequiredAndFaucetVectors() {
        val root = obj(
            JsonParser.parse(String(Files.readAllBytes(resolveFixture()), StandardCharsets.UTF_8)),
            "fixture",
        )
        assertEquals("iroha.taira.prepared-transaction-signature-fixture.v1", string(root, "schema"))
        assertEquals("u64_be", string(root, "frame_length_encoding"))
        assertEquals("iroha_blake2b_256", string(root, "digest_algorithm"))
        assertEquals(PreparedTransactionSignatureV1.TRANSCRIPT_SCHEMA, string(root, "transcript_schema"))
        val vectors = array(root, "vectors").associateBy { string(obj(it, "vector"), "name") }

        val preparedVector = obj(requireNotNull(vectors["onboarding_prepared"]), "onboarding prepared")
        val networkId = NetworkId.parse(string(preparedVector, "network_id"))
        val prepared = assertIs<AccountOnboardingPreparedTransactionV1>(parseResponse(preparedVector))
        assertFailsWith<IllegalArgumentException> {
            copyPrepared(prepared, transactionHashHex = "aa".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            PreparedTransactionSubmitResponseV1(
                prepared.binding,
                prepared.operation,
                "aa".repeat(32),
                PreparedTransactionOutcomeV1.PENDING,
            )
        }
        assertEquals(networkId, prepared.receipt.body.networkId)
        assertVectorTranscript(
            preparedVector,
            PreparedTransactionSignatureV1.onboardingPrepared(prepared),
            prepared.serverSignature,
        )
        val preparedWire = decodeHex(prepared.signedTransactionWireHex)
        val preparedPayload = TransactionPayloadAdapter.validateCanonicalPayloadBytes(
            SignedTransactionEncoder.decodeVersioned(preparedWire).encodedPayload(),
        )
        assertEquals(networkId, preparedPayload.networkId)
        assertTrue(
            AccountOnboardingReceiptVerifier.sameAccountIdentity(
                prepared.receipt.body.authority,
                preparedPayload.authority,
            ),
        )
        val independentlyParsedPrepared =
            assertIs<AccountOnboardingPreparedTransactionV1>(parseResponse(preparedVector))
        AccountOnboardingPreparedVerifier.requireValidPrepared(
            prepared,
            prepared.receipt.body.request,
            independentlyParsedPrepared.receipt,
            independentlyParsedPrepared.binding,
            networkId,
            string(preparedVector, "signer_account_id"),
        )
        val submitResponse = AccountOnboardingJsonParser.parseSubmitResponse(
            JsonEncoder.encode(
                linkedMapOf(
                    "schema" to PreparedTransactionSubmitResponseV1.SCHEMA,
                    "binding" to prepared.binding.toJsonMap(),
                    "operation" to prepared.operation,
                    "transaction_hash_hex" to prepared.transactionHashHex,
                    "outcome" to PreparedTransactionOutcomeV1.PENDING.wireValue,
                ),
            ).toByteArray(StandardCharsets.UTF_8),
        )
        AccountOnboardingPreparedVerifier.requireValidSubmitResponse(submitResponse, prepared, 200)
        AccountOnboardingPreparedVerifier.requireValidSubmitResponse(submitResponse, prepared, 202)
        val incorrectlyAppliedAtAcceptance = PreparedTransactionSubmitResponseV1(
            prepared.binding,
            prepared.operation,
            prepared.transactionHashHex,
            PreparedTransactionOutcomeV1.APPLIED,
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
                incorrectlyAppliedAtAcceptance,
                prepared,
                202,
            )
        }

        val proofRequiredVector = obj(
            requireNotNull(vectors["onboarding_proof_required"]),
            "onboarding proof required",
        )
        assertEquals(networkId, NetworkId.parse(string(proofRequiredVector, "network_id")))
        val proofRequired =
            assertIs<AccountOnboardingProofRequiredPrepareResponseV1>(parseResponse(proofRequiredVector))
        assertEquals("ProofRequired", proofRequired.outcome)
        assertEquals("account_alias_current_state", proofRequired.proofKind)
        assertVectorTranscript(
            proofRequiredVector,
            PreparedTransactionSignatureV1.onboardingProofRequired(proofRequired),
            proofRequired.serverSignature,
        )
        AccountOnboardingPreparedVerifier.requireValidProofRequired(
            proofRequired,
            prepared.receipt.body.request,
            prepared.receipt,
            assertIs<AccountOnboardingProofRequiredPrepareResponseV1>(parseResponse(proofRequiredVector)).binding,
            networkId,
            string(proofRequiredVector, "signer_account_id"),
        )
        val substitutedRequest = AccountOnboardingPlanRequestV1(
            prepared.receipt.body.request.alias,
            prepared.receipt.body.request.accountId,
            listOf("CanSetKeyValueInAccount"),
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingPreparedVerifier.requireValidPrepared(
                prepared,
                substitutedRequest,
                prepared.receipt,
                prepared.binding,
                networkId,
                string(preparedVector, "signer_account_id"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingPreparedVerifier.requireValidProofRequired(
                proofRequired,
                substitutedRequest,
                prepared.receipt,
                proofRequired.binding,
                networkId,
                string(proofRequiredVector, "signer_account_id"),
            )
        }

        assertPreparedTamperRejected(prepared, networkId, string(preparedVector, "signer_account_id"))
        val faucetVector = obj(requireNotNull(vectors["faucet_prepared"]), "faucet prepared")
        assertFaucetVector(faucetVector, NetworkId.parse(string(faucetVector, "network_id")))
    }

    private fun assertPreparedTamperRejected(
        prepared: AccountOnboardingPreparedTransactionV1,
        networkId: NetworkId,
        authority: String,
    ) {
        fun rejected(candidate: AccountOnboardingPreparedTransactionV1) {
            assertFailsWith<IllegalArgumentException> {
                AccountOnboardingPreparedVerifier.requireValidPrepared(
                    candidate,
                    prepared.receipt.body.request,
                    prepared.receipt,
                    candidate.binding,
                    networkId,
                    authority,
                )
            }
        }
        rejected(copyPrepared(prepared, serverSignature = flipHex(prepared.serverSignature)))
        rejected(copyPrepared(prepared, signedTransactionWireHex = flipHex(prepared.signedTransactionWireHex)))
        rejected(copyPrepared(prepared, transactionHashHex = flipHex(prepared.transactionHashHex)))
        val binding = prepared.binding
        val alteredBinding = TairaPublicResetMutationBindingV1(
            authorizationSha256 = flipHex(binding.authorizationSha256),
            authorizationNonce = binding.authorizationNonce,
            kind = binding.kind,
            phase = binding.phase,
            idempotencyKey = binding.idempotencyKey,
            executionExpiresAtUnixMs = binding.executionExpiresAtUnixMs,
        )
        rejected(copyPrepared(prepared, binding = alteredBinding))
    }

    private fun assertFaucetVector(vector: Map<String, Any?>, networkId: NetworkId) {
        val response = obj(vector["response"], "faucet response")
        val transcript = faucetTranscript(response)
        assertVectorTranscript(vector, transcript, string(response, "server_signature"))
        val wire = decodeHex(string(response, "signed_transaction_wire_hex"))
        assertEquals(
            string(response, "signed_transaction_wire_sha256"),
            hex(MessageDigest.getInstance("SHA-256").digest(wire)),
        )
        val transaction = SignedTransactionEncoder.decodeVersioned(wire)
        assertContentEquals(wire, SignedTransactionEncoder.encodeVersioned(transaction))
        assertEquals(string(response, "transaction_hash_hex"), SignedTransactionHasher.hashHex(transaction))
        val payload = TransactionPayloadAdapter.validateCanonicalPayloadBytes(transaction.encodedPayload())
        assertEquals(networkId, payload.networkId)
        assertTrue(
            AccountOnboardingReceiptVerifier.sameAccountIdentity(
                string(vector, "signer_account_id"),
                payload.authority,
            ),
        )
        assertTrue(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                payload.authority,
                IrohaHash.prehash(transaction.encodedPayload()),
                transaction.signature(),
            ),
        )
    }

    private fun assertVectorTranscript(
        vector: Map<String, Any?>,
        transcript: ByteArray,
        responseSignature: String,
    ) {
        assertEquals(string(vector, "transcript_hex"), hex(transcript))
        assertEquals(string(vector, "digest_hex"), hex(PreparedTransactionSignatureV1.digest(transcript)))
        assertEquals(string(vector, "server_signature_hex"), responseSignature.lowercase())
        assertTrue(
            AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
                string(vector, "signer_account_id"),
                PreparedTransactionSignatureV1.digest(transcript),
                responseSignature,
            ),
        )
    }

    private fun faucetTranscript(response: Map<String, Any?>): ByteArray {
        val binding = obj(response["binding"], "faucet binding")
        val claim = obj(response["claim"], "faucet claim")
        val output = ByteArrayOutputStream()
        frame(output, decodeHex("69726f68613a74616972613a70726570617265642d7472616e73616374696f6e3a763100"))
        field(output, "transcript_schema", PreparedTransactionSignatureV1.TRANSCRIPT_SCHEMA)
        field(output, "envelope_schema", string(response, "schema"))
        field(output, "operation", string(response, "operation"))
        field(output, "binding.schema", string(binding, "schema"))
        field(output, "binding.authorization_sha256", string(binding, "authorization_sha256"))
        field(output, "binding.authorization_nonce", string(binding, "authorization_nonce"))
        field(output, "binding.kind", string(binding, "kind"))
        field(output, "binding.phase", string(binding, "phase"))
        field(output, "binding.idempotency_key", string(binding, "idempotency_key"))
        field(output, "binding.execution_expires_at_unix_ms", number(binding, "execution_expires_at_unix_ms").toString())
        field(output, "claim.account_id", string(claim, "account_id"))
        field(output, "claim.pow_anchor_height", optionalNumber(claim, "pow_anchor_height"))
        field(output, "claim.pow_nonce_hex", optionalString(claim, "pow_nonce_hex"))
        field(output, "semantic_hash_hex", string(response, "semantic_hash_hex"))
        field(output, "account_id", string(response, "account_id"))
        field(output, "asset_definition_id", string(response, "asset_definition_id"))
        field(output, "asset_id", string(response, "asset_id"))
        field(output, "amount", string(response, "amount"))
        field(output, "transaction_hash_hex", string(response, "transaction_hash_hex"))
        field(output, "signed_transaction_wire_sha256", string(response, "signed_transaction_wire_sha256"))
        field(output, "signed_transaction_wire", decodeHex(string(response, "signed_transaction_wire_hex")))
        return output.toByteArray()
    }

    private fun copyPrepared(
        source: AccountOnboardingPreparedTransactionV1,
        binding: TairaPublicResetMutationBindingV1 = source.binding,
        transactionHashHex: String = source.transactionHashHex,
        signedTransactionWireHex: String = source.signedTransactionWireHex,
        serverSignature: String = source.serverSignature,
    ): AccountOnboardingPreparedTransactionV1 = AccountOnboardingPreparedTransactionV1(
        binding = binding,
        receipt = source.receipt,
        semanticHashHex = source.semanticHashHex,
        accountId = source.accountId,
        alias = source.alias,
        disposition = source.disposition,
        transactionHashHex = transactionHashHex,
        signedTransactionWireHex = signedTransactionWireHex,
        signedTransactionWireSha256 = source.signedTransactionWireSha256,
        feePayment = source.feePayment,
        serverSignature = serverSignature,
    )

    private fun parseResponse(vector: Map<String, Any?>): AccountOnboardingPrepareResponseV1 =
        AccountOnboardingJsonParser.parsePrepareResponse(
            JsonEncoder.encode(obj(vector["response"], "response")).toByteArray(StandardCharsets.UTF_8),
        )

    private fun field(output: ByteArrayOutputStream, label: String, value: String) =
        field(output, label, value.toByteArray(StandardCharsets.UTF_8))

    private fun field(output: ByteArrayOutputStream, label: String, value: ByteArray) {
        frame(output, label.toByteArray(StandardCharsets.UTF_8))
        frame(output, value)
    }

    private fun frame(output: ByteArrayOutputStream, value: ByteArray) {
        val length = value.size.toLong()
        for (shift in 56 downTo 0 step 8) output.write(((length ushr shift) and 0xffL).toInt())
        output.write(value)
    }

    private fun optionalNumber(map: Map<String, Any?>, key: String): String =
        map[key]?.let { "some:${(it as Number).toLong()}" } ?: "none"

    private fun optionalString(map: Map<String, Any?>, key: String): String =
        map[key]?.let { "some:${it as String}" } ?: "none"

    private fun flipHex(value: String): String =
        (if (value[0].lowercaseChar() == '0') '1' else '0') + value.substring(1)

    private fun decodeHex(value: String): ByteArray = ByteArray(value.length / 2) { index ->
        ((Character.digit(value[index * 2], 16) shl 4) or Character.digit(value[index * 2 + 1], 16)).toByte()
    }

    private fun hex(bytes: ByteArray): String = bytes.joinToString("") { "%02x".format(it.toInt() and 0xff) }

    @Suppress("UNCHECKED_CAST")
    private fun obj(value: Any?, path: String): Map<String, Any?> =
        value as? Map<String, Any?> ?: error("$path must be an object")

    @Suppress("UNCHECKED_CAST")
    private fun array(map: Map<String, Any?>, key: String): List<Any?> =
        map[key] as? List<Any?> ?: error("$key must be an array")

    private fun string(map: Map<String, Any?>, key: String): String =
        map[key] as? String ?: error("$key must be a string")

    private fun number(map: Map<String, Any?>, key: String): Long =
        (map[key] as? Number)?.toLong() ?: error("$key must be a number")

    private fun resolveFixture(): Path {
        var directory = Paths.get("").toAbsolutePath()
        repeat(8) {
            val candidate = directory.resolve(FIXTURE_PATH)
            if (Files.isRegularFile(candidate)) return candidate
            directory = directory.parent ?: return@repeat
        }
        error("$FIXTURE_PATH was not found from the test working directory")
    }

    private companion object {
        const val FIXTURE_PATH = "fixtures/prepared_transactions/prepared_transaction_signature_v1.json"
    }
}
