package org.hyperledger.iroha.sdk.alias

import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity
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
        val expectedFeePayment = FeePaymentIntent.authority(emptyList())
        AccountOnboardingPreparedVerifier.requireValidPrepared(
            prepared,
            prepared.receipt.body.request,
            independentlyParsedPrepared.receipt,
            independentlyParsedPrepared.binding,
            expectedFeePayment,
            networkId,
            string(preparedVector, "signer_account_id"),
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingPreparedVerifier.requireValidPrepared(
                prepared,
                prepared.receipt.body.request,
                independentlyParsedPrepared.receipt,
                independentlyParsedPrepared.binding,
                FeePaymentIntent.authority(emptyList(), 1L),
                networkId,
                string(preparedVector, "signer_account_id"),
            )
        }
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
        AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
            submitResponse,
            prepared,
            expectedFeePayment,
            200,
        )
        AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
            submitResponse,
            prepared,
            expectedFeePayment,
            202,
        )
        assertFailsWith<IllegalArgumentException> {
            AccountOnboardingPreparedVerifier.requireValidSubmitResponse(
                submitResponse,
                prepared,
                FeePaymentIntent.authority(emptyList(), 1L),
                200,
            )
        }
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
                expectedFeePayment,
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
                expectedFeePayment,
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
                    FeePaymentIntent.authority(emptyList()),
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
        val prepared = AccountOnboardingJsonParser.parseFaucetPrepareResponse(
            JsonEncoder.encode(response).toByteArray(StandardCharsets.UTF_8),
        )
        assertVectorTranscript(
            vector,
            PreparedTransactionSignatureV1.faucetPrepared(prepared),
            prepared.serverSignature,
        )
        assertEquals(prepared.claim.semanticHashHex(), prepared.semanticHashHex)
        val expectedFeePayment = FeePaymentIntent.authority(emptyList())
        val policy = AccountFaucetPolicyV1(
            string(vector, "signer_account_id"),
            FAUCET_ASSET_DEFINITION_ID,
            KotodamaQuantity.parseCanonical(FAUCET_AMOUNT),
        )
        AccountFaucetPreparedVerifier.requireValidPrepared(
            prepared,
            prepared.claim,
            prepared.binding,
            expectedFeePayment,
            policy,
            networkId,
        )
        val submitResponse = PreparedTransactionSubmitResponseV1(
            prepared.binding,
            prepared.operation,
            prepared.transactionHashHex,
            PreparedTransactionOutcomeV1.PENDING,
        )
        AccountFaucetPreparedVerifier.requireValidSubmitResponse(
            submitResponse,
            prepared,
            expectedFeePayment,
            policy,
            networkId,
            202,
        )
        assertFailsWith<IllegalArgumentException> {
            AccountFaucetPreparedVerifier.requireValidPrepared(
                prepared,
                prepared.claim,
                prepared.binding,
                FeePaymentIntent.authority(emptyList(), 1L),
                policy,
                networkId,
            )
        }
        listOf(
            AccountFaucetPolicyV1(
                prepared.accountId,
                FAUCET_ASSET_DEFINITION_ID,
                KotodamaQuantity.parseCanonical(FAUCET_AMOUNT),
            ),
            AccountFaucetPolicyV1(
                policy.faucetAuthority,
                otherAssetDefinition(),
                KotodamaQuantity.parseCanonical(FAUCET_AMOUNT),
            ),
            AccountFaucetPolicyV1(
                policy.faucetAuthority,
                FAUCET_ASSET_DEFINITION_ID,
                KotodamaQuantity.parseCanonical("6"),
            ),
        ).forEach { substitutedPolicy ->
            assertFailsWith<IllegalArgumentException> {
                AccountFaucetPreparedVerifier.requireValidPrepared(
                    prepared,
                    prepared.claim,
                    prepared.binding,
                    expectedFeePayment,
                    substitutedPolicy,
                    networkId,
                )
            }
            assertFailsWith<IllegalArgumentException> {
                AccountFaucetPreparedVerifier.requireValidSubmitResponse(
                    submitResponse,
                    prepared,
                    expectedFeePayment,
                    substitutedPolicy,
                    networkId,
                    202,
                )
            }
        }
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

    private fun otherAssetDefinition(): String {
        val bytes = ByteArray(16) { (it + 1).toByte() }
        bytes[6] = 0x47
        bytes[8] = 0x89.toByte()
        return AssetDefinitionIdEncoder.encodeFromBytes(bytes)
    }

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
        const val FAUCET_ASSET_DEFINITION_ID = "4rPeAP6jAjiLVZThZYwwPRBuQagt"
        const val FAUCET_AMOUNT = "5"
    }
}
