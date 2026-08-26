package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import java.util.Optional
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.alias.*
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.nexus.UaidPortfolioQuery
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.testing.TestNetworkIds
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class HttpClientTransportTest {
    private val verifyingKeyNetworkId = NetworkId.parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
    )
    private val otherNetworkId = NetworkId.parse(
        "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22",
    )
    private val validEd25519PublicKeyHex = TestEd25519Keys.publicKeyHex(0x22)
    private val ed25519IdentityKeyHex = "01" + "00".repeat(31)

    private fun testFeePayment(gasLimit: Long? = null): FeePaymentIntent =
        FeePaymentIntent.authority(emptyList(), gasLimit)

    private fun testMultisigAccountId(): String =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x37), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    private fun testAccountId(seed: Int): String =
        AccountAddress.fromAccount(TestEd25519Keys.publicKey(seed), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

    @Test
    fun issueIdentifierClaimReceiptBindsCanonicalPathAccount() {
        val executor = CapturingExecutor()
        val accountId = testAccountId(0x33)
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )

        transport.issueIdentifierClaimReceipt(
            accountId,
            IdentifierResolveRequest.encrypted("phone#retail", "abcd", sampleOpening()),
            applicationAuth(accountId),
        ).join()

        assertEquals(
            "/api/v1/accounts/$accountId/identifiers/claim-receipt",
            executor.lastRequest.uri.path,
        )
        assertEquals(AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null).canonicalHex(), executor.lastRequest.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.single())
        assertEquals(
            org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy.ONE_SHOT,
            executor.lastRequest.replayPolicy,
        )
        @Suppress("UNCHECKED_CAST")
        val body = JsonParser.parse(readBody(executor.lastRequest)) as Map<String, Any?>
        assertEquals("phone#retail", body["policy_id"])
        assertEquals("abcd", body["encrypted_input"])
        assertTrue(body["output_opening"] is Map<*, *>)
    }

    @Test
    fun applicationPostsRejectPathSubstitutionAndPrecomputedAuthBeforeDispatch() {
        val executor = CapturingExecutor()
        val accountId = testAccountId(0x33)
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )

        assertFailsWith<IllegalArgumentException> {
            transport.issueIdentifierClaimReceipt(
                accountId,
                IdentifierResolveRequest.encrypted("phone#retail", "abcd", sampleOpening()),
                applicationAuth(testAccountId(0x34)),
            )
        }
        val injected = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example").toBuilder()
                .putDefaultHeader(CanonicalRequestSigner.HEADER_SIGNATURE, "precomputed")
                .build(),
        )
        assertFailsWith<IllegalArgumentException> {
            injected.resolveIdentifier(
                IdentifierResolveRequest.encrypted("phone#retail", "abcd", sampleOpening()),
                applicationAuth(),
            )
        }

        assertEquals(0, executor.requestCount)
    }

    @Test
    fun uaidPortfolioQueryRejectsPaddedAssetIdBeforeDispatch() {
        val hex = "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff0102030405060708090a0b0c0d0e0f11"
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "uaid": "uaid:$hex",
                  "totals": {"accounts": 0, "positions": 0},
                  "dataspaces": []
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        transport.getUaidPortfolio("uaid:$hex", UaidPortfolioQuery(assetId = "pkr#paynet")).join()
        assertEquals(
            "https://torii.example/v1/accounts/uaid%3A$hex/portfolio?asset_id=pkr%23paynet",
            executor.lastRequest.uri.toString(),
        )
        val requestCount = executor.requestCount

        val leading = assertFailsWith<IllegalArgumentException> {
            transport.getUaidPortfolio("uaid:$hex", UaidPortfolioQuery(assetId = " pkr#paynet"))
        }
        assertTrue(leading.message?.contains("assetId must not contain surrounding whitespace") == true)
        val trailing = assertFailsWith<IllegalArgumentException> {
            transport.getUaidPortfolio("uaid:$hex", UaidPortfolioQuery(assetId = "pkr#paynet "))
        }
        assertTrue(trailing.message?.contains("assetId must not contain surrounding whitespace") == true)
        val empty = assertFailsWith<IllegalArgumentException> {
            transport.getUaidPortfolio("uaid:$hex", UaidPortfolioQuery(assetId = ""))
        }
        assertTrue(empty.message?.contains("assetId must not be blank") == true)
        assertEquals(requestCount, executor.requestCount)
    }

    @Test
    fun uaidPathLiteralRejectsNoncanonicalInputBeforeDispatch() {
        val hex = "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff0102030405060708090a0b0c0d0e0f11"
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "uaid": "uaid:$hex",
                  "totals": {"accounts": 0, "positions": 0},
                  "dataspaces": []
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        transport.getUaidPortfolio("uaid:$hex").join()
        assertEquals(
            "https://torii.example/v1/accounts/uaid%3A$hex/portfolio",
            executor.lastRequest.uri.toString(),
        )
        val requestCount = executor.requestCount

        for (uaid in listOf(hex, "UAID:$hex", "uaid:${hex.uppercase()}", " uaid:$hex", "uaid:$hex ", "uaid: $hex")) {
            assertFailsWith<IllegalArgumentException> {
                transport.getUaidPortfolio(uaid)
            }
            assertEquals(requestCount, executor.requestCount)
        }
    }

    @Test
    fun identifierHiddenFunctionRequestsCarryOutputOpening() {
        val opening = sampleOpening()
        val request = IdentifierResolveRequest.encrypted("phone#retail", "abcd", opening)

        assertEquals(opening, request.outputOpening)
    }

    @Test
    fun identifierHiddenFunctionRequestsRejectMalformedCiphertextEnvelopeFields() {
        assertFailsWith<IllegalArgumentException> {
            IdentifierResolveRequest.encrypted("phone#retail", "abc", sampleOpening())
        }
        assertFailsWith<IllegalArgumentException> {
            IdentifierResolveRequest.encrypted(" ", "abcd", sampleOpening())
        }
        assertFailsWith<IllegalArgumentException> {
            RamLfeExecuteRequest.encrypted("abc")
        }
        assertFailsWith<IllegalArgumentException> {
            RamLfeExecuteRequest.encrypted("zz")
        }
        assertFailsWith<IllegalArgumentException> {
            IdentifierResolveRequest.encrypted(samplePlaintextOnlyPolicy(), "abcd", sampleOpening())
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildIdentifierResolvePayload("phone#retail", "abc", sampleOpening())
        }
    }

    @Test
    fun identifierReceiptVerifierRejectsAdversarialReceipts() {
        val payload = sampleIdentifierResolutionPayload()
        val fixture = signedIdentifierReceiptFixture(payload)
        val policy = sampleIdentifierVerifierPolicy(fixture.resolverPublicKey)
        val receipt = IdentifierResolutionReceipt(
            payload,
            IdentifierReceiptAttestation("signed", fixture.signatureHex, null, null),
        )

        assertTrue(receipt.verifyAttestation(policy))

        val tamperedReceipt = IdentifierResolutionReceipt(
            sampleIdentifierResolutionPayload(outputCiphertextHash = "67".repeat(32)),
            receipt.attestation,
        )
        assertFalse(tamperedReceipt.verifyAttestation(policy))

        assertFailsWith<IllegalArgumentException> {
            receipt.verifyAttestation(
                sampleIdentifierVerifierPolicy("ed25519:ed0120${"11".repeat(32)}"),
            )
        }

        assertFailsWith<IllegalArgumentException> {
            IdentifierResolutionReceipt(
                payload,
                IdentifierReceiptAttestation("proof", null, "halo2/ipa", "AQID"),
            ).verifyAttestation(policy)
        }

        assertFailsWith<IllegalArgumentException> {
            receipt.verifyAttestation(sampleIdentifierVerifierPolicy(fixture.resolverPublicKey, policyId = "email#retail"))
        }

        assertFailsWith<IllegalArgumentException> {
            IdentifierResolutionReceipt(
                payload,
                IdentifierReceiptAttestation("signed", "abc", null, null),
            ).verifyAttestation(policy)
        }
    }

    @Test
    fun identifierResolutionReceiptParserRejectsNonExactReceiptTags() {
        val payload = sampleIdentifierResolutionPayload()
        val fixture = signedIdentifierReceiptFixture(payload)

        fun jsonString(value: String): String =
            "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

        fun openingJson(
            opening: RamLfeOutputOpening,
            signature: String = opening.signature,
            inputCiphertextHash: String = opening.payload.inputCiphertextHash,
            openedAtMsJson: String = opening.payload.openedAtMs.toString(),
            expiresAtMsJson: String? = opening.payload.expiresAtMs?.toString(),
        ): String {
            val opened = opening.payload
            val expires = expiresAtMsJson?.let { ",\"expires_at_ms\":$it" } ?: ""
            return ("{"
                + "\"payload\":{"
                + "\"program_id\":" + jsonString(opened.programId)
                + ",\"input_ciphertext_hash\":" + jsonString(inputCiphertextHash)
                + ",\"output_ciphertext_hash\":" + jsonString(opened.outputCiphertextHash)
                + ",\"parameter_digest\":" + jsonString(opened.parameterDigest)
                + ",\"evaluation_key_digest\":" + jsonString(opened.evaluationKeyDigest)
                + ",\"opened_output_hash\":" + jsonString(opened.openedOutputHash)
                + ",\"opened_at_ms\":" + openedAtMsJson
                + expires
                + "},\"signature\":" + jsonString(signature)
                + "}")
        }

        fun receiptJson(
            backend: String = payload.execution.backend,
            verificationMode: String = payload.execution.verificationMode,
            programDigest: String = payload.execution.programDigest,
            opaqueId: String = payload.opaqueId,
            receiptHash: String = payload.receiptHash,
            uaid: String = payload.uaid,
            openingInputCiphertextHash: String = payload.opening.payload.inputCiphertextHash,
            executedAtMsJson: String = payload.execution.executedAtMs.toString(),
            executionExpiresAtMsJson: String? = payload.execution.expiresAtMs?.toString(),
            openingOpenedAtMsJson: String = payload.opening.payload.openedAtMs.toString(),
            openingExpiresAtMsJson: String? = payload.opening.payload.expiresAtMs?.toString(),
            openingSignature: String = payload.opening.signature,
            attestationJson: String = "{"
                + "\"kind\":\"signed\","
                + "\"signature\":" + jsonString(fixture.signatureHex)
                + "}",
        ): String {
            val execution = payload.execution
            val expires = executionExpiresAtMsJson?.let { ",\"expires_at_ms\":$it" } ?: ""
            return ("{"
                + "\"payload\":{"
                + "\"policy_id\":" + jsonString(payload.policyId)
                + ",\"execution\":{"
                + "\"program_id\":" + jsonString(execution.programId)
                + ",\"program_digest\":" + jsonString(programDigest)
                + ",\"backend\":" + jsonString(backend)
                + ",\"verification_mode\":" + jsonString(verificationMode)
                + ",\"input_ciphertext_hash\":" + jsonString(execution.inputCiphertextHash)
                + ",\"output_ciphertext_hash\":" + jsonString(execution.outputCiphertextHash)
                + ",\"parameter_digest\":" + jsonString(execution.parameterDigest)
                + ",\"evaluation_key_digest\":" + jsonString(execution.evaluationKeyDigest)
                + ",\"output_hash\":" + jsonString(execution.outputHash)
                + ",\"associated_data_hash\":" + jsonString(execution.associatedDataHash)
                + ",\"executed_at_ms\":" + executedAtMsJson
                + expires
                + "},\"opening\":" + openingJson(
                    payload.opening,
                    openingSignature,
                    openingInputCiphertextHash,
                    openingOpenedAtMsJson,
                    openingExpiresAtMsJson,
                )
                + ",\"opaque_id\":" + jsonString(opaqueId)
                + ",\"receipt_hash\":" + jsonString(receiptHash)
                + ",\"uaid\":" + jsonString(uaid)
                + ",\"account_id\":" + jsonString(payload.accountId)
                + "},\"attestation\":" + attestationJson
                + "}")
        }

        fun assertRejects(
            json: String,
            message: String = "identifier receipt parser must reject malformed input",
        ) {
            assertFailsWith<IllegalStateException>(message) {
                IdentifierJsonParser.parseResolutionReceipt(json.toByteArray(StandardCharsets.UTF_8))
            }
        }

        for (backend in listOf(" bfv-affine-sha3-256-v1", "bfv-affine-sha3-256-v1 ", "BFV-AFFINE-SHA3-256-V1")) {
            assertRejects(receiptJson(backend = backend))
        }
        for (mode in listOf(" signed", "signed ", "Signed")) {
            assertRejects(receiptJson(verificationMode = mode))
        }
        for (kind in listOf(" signed", "signed ", "Signed")) {
            val attestationJson = ("{"
                + "\"kind\":" + jsonString(kind)
                + ",\"signature\":\"" + fixture.signatureHex + "\""
                + "}")
            assertRejects(receiptJson(attestationJson = attestationJson))
        }
        for (signature in listOf(" ${fixture.signatureHex}", "${fixture.signatureHex} ")) {
            val attestationJson = ("{"
                + "\"kind\":\"signed\","
                + "\"signature\":" + jsonString(signature)
                + "}")
            assertRejects(receiptJson(attestationJson = attestationJson), "attestation signature $signature")
        }
        for (signature in listOf(" ${payload.opening.signature}", "${payload.opening.signature} ")) {
            assertRejects(receiptJson(openingSignature = signature), "opening signature $signature")
        }
        for ((label, json) in listOf(
            "opaque_id" to receiptJson(opaqueId = " ${payload.opaqueId}"),
            "receipt_hash" to receiptJson(receiptHash = "${payload.receiptHash} "),
            "uaid" to receiptJson(uaid = " ${payload.uaid}"),
            "program_digest" to receiptJson(programDigest = " ${payload.execution.programDigest}"),
            "opening input_ciphertext_hash" to receiptJson(
                openingInputCiphertextHash = "${payload.opening.payload.inputCiphertextHash} ",
            ),
        )) {
            assertRejects(json, "hash exactness $label")
        }
        for ((label, json) in listOf(
            "executed_at_ms" to receiptJson(executedAtMsJson = "-1"),
            "execution expires_at_ms" to receiptJson(executionExpiresAtMsJson = "-1"),
            "opened_at_ms" to receiptJson(openingOpenedAtMsJson = "-1"),
            "opening expires_at_ms" to receiptJson(openingExpiresAtMsJson = "-1"),
        )) {
            assertRejects(json, "timestamp u64 $label")
        }
        for (proofBackend in listOf(" halo2/ipa", "halo2/ipa ")) {
            val attestationJson = ("{"
                + "\"kind\":\"proof\","
                + "\"proof_backend\":" + jsonString(proofBackend)
                + ",\"proof_b64\":\"AQID\""
                + "}")
            assertRejects(receiptJson(attestationJson = attestationJson))
        }
        for (proofB64 in listOf(" AQID", "AQID ")) {
            val attestationJson = ("{"
                + "\"kind\":\"proof\","
                + "\"proof_backend\":\"halo2/ipa\""
                + ",\"proof_b64\":" + jsonString(proofB64)
                + "}")
            assertRejects(receiptJson(attestationJson = attestationJson))
        }
        assertRejects(
            receiptJson(
                attestationJson = ("{"
                    + "\"kind\":\"proof\","
                    + "\"proof_backend\":\"halo2/ipa\","
                    + "\"proof_b64\":\"@@@\""
                    + "}"),
            ),
        )
    }

    @Test
    fun identifierClaimRecordParserRejectsNonExactClaimFields() {
        val payload = sampleIdentifierResolutionPayload()

        fun jsonString(value: String): String =
            "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

        fun claimRecordJson(
            policyId: String = payload.policyId,
            opaqueId: String = payload.opaqueId,
            receiptHash: String = payload.receiptHash,
            uaid: String = payload.uaid,
            accountId: String = payload.accountId,
        ): String =
            ("{"
                + "\"policy_id\":" + jsonString(policyId)
                + ",\"opaque_id\":" + jsonString(opaqueId)
                + ",\"receipt_hash\":" + jsonString(receiptHash)
                + ",\"uaid\":" + jsonString(uaid)
                + ",\"account_id\":" + jsonString(accountId)
                + ",\"verified_at_ms\":42"
                + ",\"expires_at_ms\":142"
                + "}")

        val claim = IdentifierJsonParser.parseClaimRecord(
            claimRecordJson().toByteArray(StandardCharsets.UTF_8),
        )
        assertEquals(payload.policyId, claim.policyId)
        assertEquals(payload.opaqueId, claim.opaqueId)
        assertEquals(payload.receiptHash, claim.receiptHash)
        assertEquals(payload.uaid, claim.uaid)
        assertEquals(payload.accountId, claim.accountId)
        assertEquals(42L, claim.verifiedAtMs)
        assertEquals(142L, claim.expiresAtMs)

        for ((label, json) in listOf(
            "policy_id" to claimRecordJson(policyId = " ${payload.policyId}"),
            "opaque_id" to claimRecordJson(opaqueId = "${payload.opaqueId} "),
            "receipt_hash" to claimRecordJson(receiptHash = " ${payload.receiptHash}"),
            "uaid" to claimRecordJson(uaid = "${payload.uaid} "),
            "account_id" to claimRecordJson(accountId = " ${payload.accountId}"),
        )) {
            assertFailsWith<IllegalStateException>("identifier claim record $label exactness") {
                IdentifierJsonParser.parseClaimRecord(json.toByteArray(StandardCharsets.UTF_8))
            }
        }
    }

    @Test
    fun identifierPolicyParserRejectsNonExactPolicyAndProofVerifierFields() {
        val canonical =
            """
                {
                  "total": 1,
                  "items": [
                    {
                      "policy_id": "phone#retail",
                      "owner": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                      "active": true,
                      "normalization": "phone_e164",
                      "resolver_public_key": "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
                      "backend": "bfv-affine-sha3-256-v1",
                      "input_encryption": "bfv-v1",
                      "input_encryption_public_parameters": "ABCD",
                      "input_encryption_public_parameters_decoded": {
                        "parameters": {
                          "polynomial_degree": 64,
                          "plaintext_modulus": 257,
                          "ciphertext_modulus": 1099511627776,
                          "decomposition_base_log": 12
                        },
                        "public_key": {
                          "b": [1, 2, 3],
                          "a": [4, 5, 6]
                        },
                        "max_input_bytes": 32,
                        "norito_length_encoding": "u64-v1"
                      },
                      "proof_verifier": {
                        "proof_backend": "halo2-ipa",
                        "circuit_id": "identifier-ram-lfe-v1",
                        "public_inputs_schema_hash": "${"66".repeat(32)}",
                        "verifying_key_bytes_b64": "AQID"
                      },
                      "note": "retail phone policy"
                    }
                  ]
                }
            """.trimIndent()

        val response = IdentifierJsonParser.parsePolicyList(canonical.toByteArray(StandardCharsets.UTF_8))
        assertEquals(response.items.first().resolverPublicKey, response.items.first().outputOpeningPublicKey)
        val proofVerifier = assertNotNull(response.items.first().proofVerifier)
        assertEquals("u64-v1", response.items.first().inputEncryptionPublicParametersDecoded?.noritoLengthEncoding)
        assertEquals("halo2-ipa", proofVerifier.proofBackend)
        assertEquals("66".repeat(32), proofVerifier.publicInputsSchemaHash)

        val cases = listOf(
            "identifier policy list.items[0].owner" to canonical.replace(
                "\"owner\": \"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"",
                "\"owner\": \" sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"",
            ),
            "identifier policy list.items[0].normalization" to canonical.replace(
                "\"normalization\": \"phone_e164\"",
                "\"normalization\": \"Phone_E164\"",
            ),
            "identifier policy list.items[0].backend" to canonical.replace(
                "\"backend\": \"bfv-affine-sha3-256-v1\"",
                "\"backend\": \"bfv-affine-sha3-256-v1 \"",
            ),
            "identifier policy list.items[0].input_encryption" to canonical.replace(
                "\"input_encryption\": \"bfv-v1\"",
                "\"input_encryption\": \"BFV-v1\"",
            ),
            "identifier policy list.items[0].input_encryption_public_parameters" to canonical.replace(
                "\"input_encryption_public_parameters\": \"ABCD\"",
                "\"input_encryption_public_parameters\": \" ABCD\"",
            ),
            "identifier policy list.items[0].input_encryption_public_parameters_decoded.norito_length_encoding" to canonical.replace(
                "\"norito_length_encoding\": \"u64-v1\"",
                "\"norito_length_encoding\": \" u64-v1\"",
            ),
            "identifier policy list.items[0].note" to canonical.replace(
                "\"note\": \"retail phone policy\"",
                "\"note\": \"retail phone policy \"",
            ),
            "identifier policy list.items[0].proof_verifier.proof_backend" to canonical.replace(
                "\"proof_backend\": \"halo2-ipa\"",
                "\"proof_backend\": \" halo2-ipa\"",
            ),
            "identifier policy list.items[0].proof_verifier.circuit_id" to canonical.replace(
                "\"circuit_id\": \"identifier-ram-lfe-v1\"",
                "\"circuit_id\": \"identifier-ram-lfe-v1 \"",
            ),
            "identifier policy list.items[0].proof_verifier.public_inputs_schema_hash" to canonical.replace(
                "\"public_inputs_schema_hash\": \"${"66".repeat(32)}\"",
                "\"public_inputs_schema_hash\": \" ${"66".repeat(32)}\"",
            ),
            "identifier policy list.items[0].proof_verifier.verifying_key_bytes_b64" to canonical.replace(
                "\"verifying_key_bytes_b64\": \"AQID\"",
                "\"verifying_key_bytes_b64\": \"AQID \"",
            ),
        )
        for ((field, body) in cases) {
            val error = assertFailsWith<RuntimeException> {
                IdentifierJsonParser.parsePolicyList(body.toByteArray(StandardCharsets.UTF_8))
            }
            assertTrue(
                error.message?.contains(field) == true,
                "expected $field failure, got $error",
            )
        }
    }

    @Test
    fun identifierReceiptVerifierMatchesSharedReceiptVectors() {
        val fixture = loadSharedReceiptFixture()
        assertEquals("identifier-receipt-attestation-v1", fixture["vector_set"])
        val policy = identifierPolicyFromReceiptFixture(obj(fixture, "policy"))
        val receipt = identifierReceiptFromFixture(obj(fixture, "receipt"))

        assertEquals(
            string(fixture, "canonical_payload_sha256"),
            sha256Hex(IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload)),
        )
        assertTrue(receipt.verifyAttestation(policy))

        for (policyId in listOf(" phone#retail", "phone#retail ", "phone #retail", "phone# retail")) {
            val mutatedReceipt = identifierReceiptFromFixture(
                obj(fixture, "receipt"),
                policyIdOverride = policyId,
            )
            assertFailsWith<IllegalArgumentException>("policy_id exactness $policyId") {
                mutatedReceipt.verifyAttestation(policy)
            }
        }

        for (programId in listOf(" identifier_lookup_retail", "identifier_lookup_retail ")) {
            val mutatedExecutionProgram = identifierReceiptFromFixture(
                obj(fixture, "receipt"),
                executionProgramIdOverride = programId,
            )
            assertFailsWith<IllegalArgumentException>("execution program_id exactness $programId") {
                mutatedExecutionProgram.verifyAttestation(policy)
            }

            val mutatedOpeningProgram = identifierReceiptFromFixture(
                obj(fixture, "receipt"),
                openingProgramIdOverride = programId,
            )
            assertFailsWith<IllegalArgumentException>("opening program_id exactness $programId") {
                mutatedOpeningProgram.verifyAttestation(policy)
            }
        }

        val accountId = string(obj(fixture, "receipt").let { obj(it, "payload") }, "account_id")
        for (paddedAccountId in listOf(" $accountId", "$accountId ")) {
            val mutatedAccount = identifierReceiptFromFixture(
                obj(fixture, "receipt"),
                accountIdOverride = paddedAccountId,
            )
            assertFailsWith<IllegalArgumentException>("account_id exactness $paddedAccountId") {
                mutatedAccount.verifyAttestation(policy)
            }
        }

        for (vector in listOfMaps(fixture, "attestation_vectors")) {
            val name = string(vector, "name")
            val attestation = identifierAttestationFromFixture(obj(vector, "attestation"))
            val encoded = IdentifierReceiptCanonicalEncoder.encodeAttestation(attestation)
            assertEquals(
                long(vector, "expected_attestation_bytes").toInt(),
                encoded.size,
                "$name attestation byte length",
            )
            assertEquals(
                string(vector, "expected_attestation_sha256"),
                sha256Hex(encoded),
                "$name attestation digest",
            )
            val decoded = IdentifierReceiptCanonicalEncoder.decodeAttestation(encoded)
            assertEquals(attestation.kind, decoded.kind, "$name attestation kind")
            when (attestation.kind) {
                "signed" -> assertEquals(
                    attestation.signature?.lowercase(),
                    decoded.signature,
                    "$name signature roundtrip",
                )
                "proof" -> {
                    assertEquals(attestation.proofBackend, decoded.proofBackend, "$name proof backend")
                    assertEquals(attestation.proofB64, decoded.proofB64, "$name proof payload")
                    assertFailsWith<IllegalArgumentException>("$name proof verifier gate") {
                        IdentifierResolutionReceipt(receipt.payload, attestation).verifyAttestation(policy)
                    }
                }
                else -> error("Unhandled attestation kind ${attestation.kind}")
            }
        }

        for (negative in listOfMaps(fixture, "negative_cases")) {
            val mutation = string(negative, "mutation")
            val mutatedPolicy = {
                when (mutation) {
                    "policy.resolver_public_key" -> identifierPolicyFromReceiptFixture(
                        obj(fixture, "policy"),
                        resolverPublicKeyOverride = string(negative, "value"),
                    )
                    "policy.policy_id" -> identifierPolicyFromReceiptFixture(
                        obj(fixture, "policy"),
                        policyIdOverride = string(negative, "value"),
                    )
                    else -> policy
                }
            }
            val mutatedReceipt = when (mutation) {
                "receipt.payload.execution.output_ciphertext_hash" -> identifierReceiptFromFixture(
                    obj(fixture, "receipt"),
                    outputCiphertextHashOverride = string(negative, "value"),
                )
                "receipt.attestation.signature" -> identifierReceiptFromFixture(
                    obj(fixture, "receipt"),
                    signatureOverride = string(negative, "value"),
                )
                "receipt.attestation" -> identifierReceiptFromFixture(
                    obj(fixture, "receipt"),
                    attestationOverride = obj(negative, "value"),
                )
                else -> receipt
            }

            if (negative["expected_error_contains"] is String) {
                assertFailsWith<IllegalArgumentException>(string(negative, "name")) {
                    mutatedReceipt.verifyAttestation(mutatedPolicy())
                }
            } else {
                assertEquals(
                    negative["expected_result"],
                    mutatedReceipt.verifyAttestation(mutatedPolicy()),
                    string(negative, "name"),
                )
            }
        }
    }

    @Test
    fun identifierBfvEnvelopeBuilderMatchesSharedSoracloudVectors() {
        val fixture = loadSharedBfvFixture()
        assertEquals("soracloud-bfv-identifier-envelope-v1", fixture["vector_set"])
        assertBfvOperationKeyComponentVectors(obj(fixture, "operation_vectors"))
        val policy = bfvPolicyFromFixture(obj(fixture, "policy"))
        val vectors = listOfMaps(fixture, "vectors")
        val observedDigests = mutableSetOf<String>()

        for (vector in vectors) {
            val ciphertextHex = policy.encryptInput(
                string(vector, "input_utf8"),
                hexToBytes(string(vector, "seed_hex")),
            )
            assertEquals(
                long(vector, "expected_ciphertext_bytes").toInt(),
                ciphertextHex.length / 2,
                "${string(vector, "name")}: ciphertext byte length",
            )
            val digest = sha256Hex(hexToBytes(ciphertextHex))
            assertEquals(
                string(vector, "expected_ciphertext_sha256"),
                digest,
                "${string(vector, "name")}: ciphertext digest",
            )
            observedDigests.add(digest)
        }

        assertEquals(vectors.size, observedDigests.size, "fixture ciphertext digests must be unique")
    }

    @Test
    fun identifierBfvEnvelopeBuilderMatchesSharedSoracloudOperationInputVectors() {
        val fixture = loadSharedBfvFixture()
        val operationVectors = obj(fixture, "operation_vectors")
        assertEquals("soracloud-bfv-operation-v1", operationVectors["vector_set"])
        val policy = IdentifierPolicySummary(
            policyId = "soracloud-operation#fixture",
            owner = "owner",
            active = true,
            normalization = IdentifierNormalization.EXACT,
            resolverPublicKey = "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            backend = "bfv-programmed-sha3-256-v1",
            inputEncryption = "bfv-v1",
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = bfvParametersFromFixture(
                obj(operationVectors, "public_parameters_decoded"),
            ),
            note = null,
        )
        val observedDigests = mutableSetOf<String>()
        var checkedInputs = 0
        for (vector in listOfMaps(operationVectors, "vectors")) {
            val vectorName = string(vector, "name")
            for (input in listOfMaps(vector, "inputs")) {
                if (input["packed_slots"] != null) continue
                val seedUtf8 = string(input, "seed_utf8")
                val inputBytes = hexToBytes(string(input, "input_hex"))
                val ciphertextHex = policy.encryptInput(
                    String(inputBytes, StandardCharsets.UTF_8),
                    seedUtf8.toByteArray(StandardCharsets.UTF_8),
                )
                assertEquals(
                    long(input, "expected_ciphertext_bytes").toInt(),
                    ciphertextHex.length / 2,
                    "$vectorName/$seedUtf8 ciphertext byte length",
                )
                val digest = sha256Hex(hexToBytes(ciphertextHex))
                assertEquals(
                    string(input, "expected_ciphertext_sha256"),
                    digest,
                    "$vectorName/$seedUtf8 ciphertext digest",
                )
                assertTrue(observedDigests.add(digest), "operation input digest must be unique: $digest")
                checkedInputs += 1
            }
        }
        assertEquals(8, checkedInputs, "fixture should cover every non-packed operation input")
    }

    @Test
    fun sharedSoracloudBfvKeyBundleComponentVectorsAreComplete() {
        val fixture = loadSharedBfvFixture()

        assertBfvOperationKeyComponentVectors(obj(fixture, "operation_vectors"))
    }

    @Test
    fun sharedSoracloudBfvKeyBundleComponentVectorsRejectAdversarialDrift() {
        for ((name, mutate) in listOf<Pair<String, (MutableMap<String, Any?>) -> Unit>>(
            "missing relinearization component digest" to { operationVectors ->
                val evaluationKey = mutableObj(operationVectors, "evaluation_key_bundle")
                mutableListOfMaps(evaluationKey, "relinearization_entries")[0].remove("b_sha256")
            },
            "duplicate BFV component digest" to { operationVectors ->
                val evaluationKey = mutableObj(operationVectors, "evaluation_key_bundle")
                val entries = mutableListOfMaps(evaluationKey, "relinearization_entries")
                entries[1]["a_sha256"] = string(entries[0], "b_sha256")
            },
            "noncanonical lowercase component digest" to { operationVectors ->
                val evaluationKey = mutableObj(operationVectors, "evaluation_key_bundle")
                val entries = mutableListOfMaps(evaluationKey, "relinearization_entries")
                entries[0]["b_sha256"] = string(entries[0], "b_sha256").lowercase()
            },
            "zero rotation refresh component digest" to { operationVectors ->
                val rotationKey = mutableListOfMaps(operationVectors, "rotation_keys")[0]
                mutableObj(rotationKey, "zero_refresh_components")["c1_sha256"] = "0".repeat(64)
            },
            "bootstrap refresh coefficient-count drift" to { operationVectors ->
                val bootstrap = mutableObj(operationVectors, "bootstrap_key")
                mutableObj(bootstrap, "zero_refresh_components")["coefficient_count"] = 63L
            },
            "rotation key count drift" to { operationVectors ->
                mutableObj(operationVectors, "evaluation_key_bundle")["rotation_key_count"] = 99L
            },
            "missing full-bootstrap material fixture" to { operationVectors ->
                operationVectors.remove("full_bootstrap_material")
            },
            "full-bootstrap verifier commitment drift" to { operationVectors ->
                val material = mutableObj(operationVectors, "full_bootstrap_material")
                material["vk_commitment_hex"] = string(material, "expected_material_digest_hex")
            },
            "noncanonical full-bootstrap material digest" to { operationVectors ->
                val material = mutableObj(operationVectors, "full_bootstrap_material")
                material["expected_material_digest_hex"] = string(material, "expected_material_digest_hex").uppercase()
            },
        )) {
            val operationVectors = mutableObj(loadSharedBfvFixture(), "operation_vectors")
            mutate(operationVectors)
            assertFailsWith<Throwable>(name) {
                assertBfvOperationKeyComponentVectors(operationVectors)
            }
        }
    }

    @Test
    fun identifierBfvEnvelopeBuilderRejectsAdversarialPublicParameters() {
        val seed = ByteArray(32) { it.toByte() }
        val baseParameters = sampleBfvParameters()

        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(baseParameters).encryptInput("abcd", seed)
        }

        val nonDivisibleModulus = IdentifierBfvPublicParameters(
            IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_753L, 12),
            baseParameters.publicKey,
            3,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(nonDivisibleModulus).encryptInput("ab", seed)
        }

        val nonPowerOfTwoDegree = IdentifierBfvPublicParameters(
            IdentifierBfvPublicParameters.Parameters(7L, 257L, 16_842_752L, 12),
            baseParameters.publicKey,
            3,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(nonPowerOfTwoDegree).encryptInput("ab", seed)
        }

        val invalidDecompositionBase = IdentifierBfvPublicParameters(
            IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_752L, 17),
            baseParameters.publicKey,
            3,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(invalidDecompositionBase).encryptInput("ab", seed)
        }

        val truncatedPublicKey = IdentifierBfvPublicParameters(
            baseParameters.parameters,
            IdentifierBfvPublicParameters.PublicKey(
                baseParameters.publicKey.b.drop(1),
                baseParameters.publicKey.a,
            ),
            3,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(truncatedPublicKey).encryptInput("ab", seed)
        }

        val zeroInputLimit = IdentifierBfvPublicParameters(
            baseParameters.parameters,
            baseParameters.publicKey,
            0,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(zeroInputLimit).encryptInput("ab", seed)
        }

        val overwideInputLimit = IdentifierBfvPublicParameters(
            baseParameters.parameters,
            baseParameters.publicKey,
            64,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(overwideInputLimit).encryptInput("ab", seed)
        }

        val oversizedCoefficient = IdentifierBfvPublicParameters(
            baseParameters.parameters,
            IdentifierBfvPublicParameters.PublicKey(
                listOf(16_842_752L, 15_791_131L, 10_301_391L, 6_321_610L, 502_045L, 1_948_157L, 5_332_249L, 12_641_494L),
                baseParameters.publicKey.a,
            ),
            3,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(oversizedCoefficient).encryptInput("ab", seed)
        }

        val oversizedInputLimit = IdentifierBfvPublicParameters(
            baseParameters.parameters,
            baseParameters.publicKey,
            257,
        )
        assertFailsWith<IllegalArgumentException> {
            sampleBfvPolicy(oversizedInputLimit).encryptInput("ab", seed)
        }
    }

    @Test
    fun prepareContractCallPostsSecretFreeSelectorPayloadAndParsesDraft() {
        val transactionPayload = sampleTransaction(7).encodedPayload()
        val transactionPayloadB64 = Base64.getEncoder().encodeToString(transactionPayload)
        val signingMessageB64 = Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionPayload))
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "ok": true,
                  "submitted": false,
                  "dataspace": "router",
                  "code_hash_hex": "${"44".repeat(32)}",
                  "abi_hash_hex": "${"55".repeat(32)}",
                  "creation_time_ms": 1712345678901,
                  "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
                  "entrypoint": "contribute",
                  "transaction_ttl_ms": 60000,
                  "transaction_payload_b64": "$transactionPayloadB64",
                  "signing_message_b64": "$signingMessageB64",
                  "operation_receipt": {
                    "operation_kind": "contract_call",
                    "status": "pending_signature",
                    "transport": "torii",
                    "dataspace": "router",
                    "contract_alias": "router::universal",
                    "contract_address": "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
                    "code_hash_hex": "${"44".repeat(32)}",
                    "abi_hash_hex": "${"55".repeat(32)}",
                    "entrypoint": "contribute",
                    "gas_limit": 5000,
                    "gas_used": 17,
                    "fee_payment": {
                      "payer": "authority",
                      "value": {"charge_limits": [], "gas_limit": 5000}
                    },
                    "payload_digest_hex": "${"88".repeat(32)}"
                  }
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )
        val response = transport.prepareContractCall(
            authority = "alice",
            feePayment = testFeePayment(5_000L),
            contractAlias = "router::universal",
            entrypoint = "contribute",
            payload = linkedMapOf("buyer" to "alice", "payment_amount" to 1L),
        ).join()

        assertTrue(response.ok)
        assertFalse(response.submitted)
        assertEquals("router", response.dataspace)
        assertEquals("contribute", response.entrypoint)
        assertEquals(60_000L, response.transactionTtlMs)
        assertEquals(null, response.entrypointHashHex)
        assertNull(response.pipelineStatus)
        assertEquals("contract_call", response.operationReceipt.operationKind)
        assertEquals(5_000L, response.operationReceipt.gasLimit)
        assertEquals(5_000L, response.operationReceipt.feePayment?.gasLimit)
        assertEquals("88".repeat(32), response.operationReceipt.payloadDigestHex)
        assertEquals(transactionPayloadB64, response.transactionPayloadB64)
        assertEquals(signingMessageB64, response.signingMessageB64)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/contracts/call", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice", payload["authority"])
        assertFalse(payload.containsKey("private_key"))
        assertEquals("router::universal", payload["contract_alias"])
        assertFalse(payload.containsKey("contract_address"))
        assertEquals("contribute", payload["entrypoint"])
        assertFalse(payload.containsKey("gas_limit"))
        assertFalse(payload.containsKey("gas_asset_id"))
        @Suppress("UNCHECKED_CAST")
        val feePayment = payload["fee_payment"] as Map<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val feeValue = feePayment["value"] as Map<String, Any?>
        assertEquals(5_000L, (feeValue["gas_limit"] as Number).toLong())
        @Suppress("UNCHECKED_CAST")
        val args = payload["payload"] as Map<String, Any?>
        assertEquals("alice", args["buyer"])
        assertEquals(1L, (args["payment_amount"] as Number).toLong())
    }

    @Test
    fun contractCallBoundaryConsumesSharedRustArgumentRecordFixture() {
        val fixture = loadSharedFixture("fixtures/kotodama/entrypoint_argument_record_v1.json")
        assertEquals("EntrypointArgumentRecordV1", fixture["codec"])
        assertEquals("ivm::encode_argument_record_from_json", fixture["generator"])
        val schema = obj(fixture, "entrypoint_argument_schema_v1")
        assertTrue(Regex("[0-9a-f]{64}").matches(string(schema, "schema_hash_hex")))
        val record = obj(fixture, "entrypoint_argument_record_v1")
        assertTrue(Regex("(?:[0-9a-f]{2})+").matches(string(record, "norito_hex")))

        val boundary = obj(fixture, "torii_boundary")
        val boundaryFeePayment = FeePaymentJson.parse(
            boundary["fee_payment"],
            "torii_boundary.fee_payment",
        )
        val request = HttpClientTransport.buildContractCallDraftPayload(
            authority = string(boundary, "authority"),
            feePayment = boundaryFeePayment,
            contractAddress = null,
            contractAlias = string(boundary, "contract_alias"),
            entrypoint = string(boundary, "entrypoint"),
            payload = boundary["payload"],
        )

        assertEquals(string(boundary, "authority"), request["authority"])
        assertFalse(request.containsKey("private_key"))
        assertEquals(string(boundary, "contract_alias"), request["contract_alias"])
        assertFalse(request.containsKey("contract_address"))
        assertEquals(string(boundary, "entrypoint"), request["entrypoint"])
        assertEquals(boundary["payload"], request["payload"])
        assertEquals(boundaryFeePayment.toJsonMap(), request["fee_payment"])
        assertFalse(request.containsKey("argument_record"))
        assertFalse(request.containsKey("argument_record_norito_hex"))
    }

    @Test
    fun proposeMultisigPostsNativeNoritoInstructionPayloadsAndParsesResponse() {
        val instructionBytes = byteArrayOf(1, 2, 3, 4)
        val proposalId = "aa".repeat(32)
        val multisigAccountId = testMultisigAccountId()
        val transactionPayload = sampleTransaction(8).encodedPayload()
        val transactionPayloadB64 = Base64.getEncoder().encodeToString(transactionPayload)
        val signingMessageB64 = Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionPayload))
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "ok": true,
                  "resolved_multisig_account_id": "$multisigAccountId",
                  "submitted": false,
                  "proposal_id": "$proposalId",
                  "instructions_hash": "$proposalId",
                  "tx_hash_hex": null,
                  "executed_tx_hash_hex": null,
                  "creation_time_ms": 123,
                  "transaction_payload_b64": "$transactionPayloadB64",
                  "signing_message_b64": "$signingMessageB64"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )
        val response = transport.proposeMultisig(
            MultisigProposeRequest(
                feePayment = testFeePayment(),
                multisigAccountAlias = "cbdc@banka",
                signerAccountId = "alice",
                instructions = listOf(instructionBytes),
                publicKeyHex = "0X${validEd25519PublicKeyHex.uppercase()}",
                creationTimeMs = 123,
                memo = "QR invoice 42",
                validationFeePolicyVersion = 7,
                validationFeePolicyHash = "AB".repeat(32),
                validationFeeInstructionIndex = 1,
                validationFeeTransferEntryIndex = 2,
            )
        ).join()

        assertTrue(response.ok)
        assertEquals(multisigAccountId, response.resolvedMultisigAccountId)
        assertEquals(false, response.submitted)
        assertEquals(proposalId, response.instructionsHash)
        assertEquals(transactionPayloadB64, response.transactionPayloadB64)
        assertEquals(signingMessageB64, response.signingMessageB64)

        val request = executor.lastRequest
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/multisig/propose", request.uri.toString())
        assertEquals("application/json", request.headers["Content-Type"]?.first())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("cbdc@banka", payload["multisig_account_alias"])
        assertEquals("alice", payload["signer_account_id"])
        assertEquals(validEd25519PublicKeyHex, payload["public_key_hex"])
        assertFalse(payload.containsKey("fee_sponsor"))
        @Suppress("UNCHECKED_CAST")
        val feePayment = payload["fee_payment"] as Map<String, Any?>
        assertEquals("authority", feePayment["payer"])
        assertEquals("QR invoice 42", payload["memo"])
        assertEquals(123L, (payload["creation_time_ms"] as Number).toLong())
        assertEquals("7", payload["validation_fee_policy_version"])
        assertEquals("ab".repeat(32), payload["validation_fee_policy_hash"])
        assertEquals("1", payload["validation_fee_instruction_index"])
        assertEquals("2", payload["validation_fee_transfer_entry_index"])
        @Suppress("UNCHECKED_CAST")
        val instructions = payload["instructions"] as List<String>
        assertEquals(listOf(Base64.getEncoder().encodeToString(instructionBytes)), instructions)
    }

    @Test
    fun proposeMultisigRejectsEmptyInstructionPayloads() {
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(byteArrayOf()),
                )
            )
        }
    }

    @Test
    fun proposeMultisigRejectsAdversarialRequestShapes() {
        val instruction = byteArrayOf(1)
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountId = "aid:multisig",
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                )
            )
        }
        val canonicalSignature = Base64.getEncoder().encodeToString(ByteArray(64) { 0x01 })
        for (signatureB64 in listOf(" $canonicalSignature", noncanonicalStandardBase64PadBitAlias(canonicalSignature))) {
            assertFailsWith<IllegalArgumentException> {
                HttpClientTransport.buildMultisigProposePayload(
                    MultisigProposeRequest(
                        feePayment = testFeePayment(),
                        multisigAccountAlias = "cbdc@banka",
                        signerAccountId = "alice",
                        instructions = listOf(instruction),
                        signatureB64 = signatureB64,
                    )
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    signatureB64 = "not base64",
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    publicKeyHex = "aa",
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    creationTimeMs = -1,
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    validationFeeInstructionIndex = 1,
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    validationFeeTransferEntryIndex = 2,
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    validationFeePolicyVersion = 7,
                    validationFeePolicyHash = "ab".repeat(32),
                    validationFeeTransferEntryIndex = 2,
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    validationFeePolicyVersion = 7,
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    validationFeePolicyVersion = 7,
                    validationFeePolicyHash = "ab".repeat(32),
                    validationFeeInstructionIndex = -1,
                )
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(instruction),
                    validationFeePolicyVersion = 7,
                    validationFeePolicyHash = "ab".repeat(32),
                    validationFeeInstructionIndex = 1,
                    validationFeeTransferEntryIndex = -2,
                )
            )
        }
    }

    @Test
    fun ed25519KeyRoutesRejectSmallOrderIdentityPoint() {
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildVpnQuoteCreatePayload("standard", ed25519IdentityKeyHex)
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildVpnSessionCreatePayload(
                "standard",
                "11".repeat(32),
                "22".repeat(32),
                ed25519IdentityKeyHex,
            )
        }
        assertFailsWith<IllegalStateException> {
            VpnJsonParser.parseQuote(
                vpnQuoteJson("11".repeat(32), ed25519IdentityKeyHex)
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.buildMultisigProposePayload(
                MultisigProposeRequest(
                    feePayment = testFeePayment(),
                    multisigAccountAlias = "cbdc@banka",
                    signerAccountId = "alice",
                    instructions = listOf(byteArrayOf(1)),
                    publicKeyHex = ed25519IdentityKeyHex,
                ),
            )
        }
    }

    @Test
    fun multisigResponseParserRejectsMalformedFields() {
        val multisigAccountId = testMultisigAccountId()
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": false,
                      "resolved_multisig_account_id": "$multisigAccountId"
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "$multisigAccountId "
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "multisig"
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "$multisigAccountId",
                      "submitted": "false"
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "$multisigAccountId",
                      "instructions_hash": "aa"
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "$multisigAccountId",
                      "signing_message_b64": "not base64"
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "$multisigAccountId",
                      "signing_message_b64": ""
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<RuntimeException> {
            ContractJsonParser.parseMultisigResponse(
                """
                    {
                      "ok": true,
                      "resolved_multisig_account_id": "$multisigAccountId",
                      "creation_time_ms": -1
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8)
            )
        }
    }

    @Test
    fun callContractRejectsAmbiguousSelector() {
        val transport = HttpClientTransport.withExecutor(
            executor = CapturingExecutor(),
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<IllegalArgumentException> {
            transport.prepareContractCall(
                authority = "alice",
                feePayment = testFeePayment(5_000L),
                contractAddress = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
                contractAlias = "router::universal",
                entrypoint = "contribute",
            )
        }

        assertTrue(error.message?.contains("Exactly one") == true)
    }

    @Test
    fun callContractRejectsBlankEntrypointAndNonPositiveGas() {
        for (entrypoint in listOf("", "   ")) {
            val error = assertFailsWith<IllegalArgumentException> {
                HttpClientTransport.buildContractCallDraftPayload(
                    authority = "alice",
                    feePayment = testFeePayment(1),
                    contractAddress = null,
                    contractAlias = "router::universal",
                    entrypoint = entrypoint,
                    payload = null,
                )
            }
            assertTrue(error.message?.contains("entrypoint") == true)
        }
        for (gasLimit in listOf(0L, -1L)) {
            val error = assertFailsWith<IllegalArgumentException> {
                testFeePayment(gasLimit)
            }
            assertTrue(error.message?.contains("positive") == true)
        }
    }

    @Test
    fun callContractResponseRequiresOperationReceipt() {
        val error = assertFailsWith<IllegalStateException> {
            ContractJsonParser.parseCallResponse(
                """
                    {
                      "ok": true,
                      "submitted": true,
                      "dataspace": "router",
                      "code_hash_hex": "${"44".repeat(32)}",
                      "abi_hash_hex": "${"55".repeat(32)}",
                      "creation_time_ms": 1,
                      "entrypoint": "contribute"
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertTrue(error.message?.contains("operation_receipt must be a JSON object") == true)
    }

    @Test
    fun contractAndMultisigTransactionHashesRequireIrohaHashOfMarker() {
        val canonical = "ab".repeat(32)
        val evenMarker = "aa".repeat(32)
        fun contractResponse(
            txHash: String = canonical,
            entrypointHash: String = canonical,
            receiptTxHash: String = canonical,
            receiptEntrypointHash: String = canonical,
        ): ByteArray =
            """
                {
                  "ok": true,
                  "submitted": true,
                  "dataspace": "router",
                  "code_hash_hex": "${"44".repeat(32)}",
                  "abi_hash_hex": "${"55".repeat(32)}",
                  "creation_time_ms": 1,
                  "tx_hash_hex": "$txHash",
                  "entrypoint_hash_hex": "$entrypointHash",
                  "operation_receipt": {
                    "operation_kind": "contract_call",
                    "status": "queued",
                    "transport": "torii",
                    "dataspace": "router",
                    "tx_hash_hex": "$receiptTxHash",
                    "entrypoint_hash_hex": "$receiptEntrypointHash",
                    "payload_digest_hex": "${"88".repeat(32)}"
                  }
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8)

        assertEquals(canonical, ContractJsonParser.parseCallResponse(contractResponse()).txHashHex)
        for (payload in listOf(
            contractResponse(txHash = evenMarker),
            contractResponse(txHash = " $canonical"),
            contractResponse(entrypointHash = evenMarker),
            contractResponse(receiptTxHash = evenMarker),
            contractResponse(receiptEntrypointHash = evenMarker),
        )) {
            assertFailsWith<IllegalStateException> {
                ContractJsonParser.parseCallResponse(payload)
            }
        }

        val multisigAccountId = testMultisigAccountId()
        fun multisigResponse(txHash: String, executedTxHash: String): ByteArray =
            """
                {
                  "ok": true,
                  "resolved_multisig_account_id": "$multisigAccountId",
                  "submitted": true,
                  "tx_hash_hex": "$txHash",
                  "executed_tx_hash_hex": "$executedTxHash"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8)
        assertEquals(
            canonical,
            ContractJsonParser.parseMultisigResponse(
                multisigResponse(canonical, canonical),
            ).executedTxHashHex,
        )
        assertFailsWith<IllegalStateException> {
            ContractJsonParser.parseMultisigResponse(
                multisigResponse(evenMarker, canonical),
            )
        }
        assertFailsWith<IllegalStateException> {
            ContractJsonParser.parseMultisigResponse(
                multisigResponse(canonical, evenMarker),
            )
        }
    }

    @Test
    fun listRamLfeProgramPoliciesParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = ramLfeProgramPoliciesJson().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )

        val response = transport.listRamLfeProgramPolicies().join()

        assertEquals(1L, response.total)
        assertEquals(1, response.items.size)
        val item = response.items.first()
        assertEquals("identifier_lookup_retail", item.programId)
        assertEquals("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", item.owner)
        assertTrue(item.active)
        assertEquals(item.resolverPublicKey, item.outputOpeningPublicKey)
        assertEquals("signed", item.verificationMode)
        assertEquals("bfv-v1", item.inputEncryption)
        val decodedParameters = assertNotNull(item.inputEncryptionPublicParametersDecoded)
        assertEquals(64L, decodedParameters.parameters.polynomialDegree)
        val proofVerifier = assertNotNull(item.proofVerifier)
        assertEquals("halo2-ipa", proofVerifier.proofBackend)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("GET", request.method)
        assertEquals("https://torii.example/v1/ram-lfe/program-policies", request.uri.toString())
        assertTrue(request.headers["Accept"]?.contains("application/json") == true)
    }

    @Test
    fun ramLfeProgramPolicyParserRejectsNonExactFields() {
        val canonical = ramLfeProgramPoliciesJson()
        val cases = listOf(
            "ram-lfe program policy list.items[0].program_id" to canonical.replace(
                "\"program_id\": \"identifier_lookup_retail\"",
                "\"program_id\": \" identifier_lookup_retail\"",
            ),
            "ram-lfe program policy list.items[0].owner" to canonical.replace(
                "\"owner\": \"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV\"",
                "\"owner\": \"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV \"",
            ),
            "ram-lfe program policy list.items[0].resolver_public_key" to canonical.replace(
                "\"resolver_public_key\": \"ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29\"",
                "\"resolver_public_key\": \" ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29\"",
            ),
            "ram-lfe program policy list.items[0].backend" to canonical.replace(
                "\"backend\": \"bfv-programmed-sha3-256-v1\"",
                "\"backend\": \"BFV-programmed-sha3-256-v1\"",
            ),
            "ram-lfe program policy list.items[0].verification_mode" to canonical.replace(
                "\"verification_mode\": \"signed\"",
                "\"verification_mode\": \" signed\"",
            ),
            "ram-lfe program policy list.items[0].input_encryption" to canonical.replace(
                "\"input_encryption\": \"bfv-v1\"",
                "\"input_encryption\": \"bfv-v1 \"",
            ),
            "ram-lfe program policy list.items[0].input_encryption_public_parameters" to canonical.replace(
                "\"input_encryption_public_parameters\": \"ABCD\"",
                "\"input_encryption_public_parameters\": \" ABCD\"",
            ),
            "ram-lfe program policy list.items[0].proof_verifier.proof_backend" to canonical.replace(
                "\"proof_backend\": \"halo2-ipa\"",
                "\"proof_backend\": \" halo2-ipa\"",
            ),
            "ram-lfe program policy list.items[0].proof_verifier.circuit_id" to canonical.replace(
                "\"circuit_id\": \"ram-lfe-v1\"",
                "\"circuit_id\": \"ram-lfe-v1 \"",
            ),
            "ram-lfe program policy list.items[0].proof_verifier.public_inputs_schema_hash" to canonical.replace(
                "\"public_inputs_schema_hash\": \"${"44".repeat(32)}\"",
                "\"public_inputs_schema_hash\": \" ${"44".repeat(32)}\"",
            ),
            "ram-lfe program policy list.items[0].proof_verifier.verifying_key_bytes_b64" to canonical.replace(
                "\"verifying_key_bytes_b64\": \"AQID\"",
                "\"verifying_key_bytes_b64\": \"AQID \"",
            ),
        )
        for ((field, body) in cases) {
            val error = assertFailsWith<RuntimeException> {
                RamLfeJsonParser.parsePolicyList(body.toByteArray(StandardCharsets.UTF_8))
            }
            assertTrue(
                error.message?.contains(field) == true,
                "expected $field failure, got $error",
            )
        }
    }

    private fun ramLfeProgramPoliciesJson(): String =
        """
            {
              "total": 1,
              "items": [
                {
                  "program_id": "identifier_lookup_retail",
                  "owner": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                  "active": true,
                  "resolver_public_key": "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
                  "backend": "bfv-programmed-sha3-256-v1",
                  "verification_mode": "signed",
                  "input_encryption": "bfv-v1",
                  "input_encryption_public_parameters": "ABCD",
                  "input_encryption_public_parameters_decoded": {
                    "parameters": {
                      "polynomial_degree": 64,
                      "plaintext_modulus": 257,
                      "ciphertext_modulus": 1099511627776,
                      "decomposition_base_log": 12
                    },
                    "public_key": {
                      "b": [1, 2, 3],
                      "a": [4, 5, 6]
                    },
                    "max_input_bytes": 32
                  },
                  "note": "retail programmed policy",
                  "proof_verifier": {
                    "proof_backend": "halo2-ipa",
                    "circuit_id": "ram-lfe-v1",
                    "public_inputs_schema_hash": "${"44".repeat(32)}",
                    "verifying_key_bytes_b64": "AQID"
                  }
                }
              ]
            }
        """.trimIndent()

    @Test
    fun executeRamLfeProgramParsesResponseAndPostsEncryptedHex() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = ramLfeExecuteResponseJson().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )

        val response = transport
            .executeRamLfeProgram(
                "identifier_lookup_retail",
                RamLfeExecuteRequest.encrypted("0xABCD"),
                applicationAuth(),
            )
            .join()

        assertTrue(response.isPresent)
        val execute = response.get()
        assertEquals("identifier_lookup_retail", execute.programId)
        assertEquals("44".repeat(32), execute.outputHash)
        assertEquals("signed", execute.verificationMode)
        assertTrue(execute.receipt.containsKey("payload"))

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals(
            "https://torii.example/v1/ram-lfe/programs/identifier_lookup_retail/execute",
            request.uri.toString(),
        )
        assertEquals("""{"encrypted_input":"abcd"}""", readBody(request))
    }

    @Test
    fun executeRamLfeProgramReturnsEmptyOnNotFoundAndPostsEncryptedHex() {
        val executor = StubResponseExecutor(
            statusCode = 404,
            body = byteArrayOf(),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )

        val response = transport
            .executeRamLfeProgram(
                "identifier_lookup_retail",
                RamLfeExecuteRequest.encrypted("ABCD"),
                applicationAuth(),
            )
            .join()

        assertFalse(response.isPresent)
        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("""{"encrypted_input":"abcd"}""", readBody(request))
    }

    @Test
    fun verifyRamLfeReceiptPostsRawReceiptAndParsesResponse() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = ramLfeReceiptVerifyResponseJson().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )
        val receipt = linkedMapOf<String, Any>(
            "payload" to linkedMapOf<String, Any?>(
                "program_id" to mapOf("name" to "identifier_lookup_retail"),
                "backend" to "bfv-programmed-sha3-256-v1",
                "verification_mode" to mapOf("mode" to "Signed", "value" to null),
                "program_digest" to "hash:${"11".repeat(32).uppercase()}#ABCD",
                "output_hash" to "hash:${"22".repeat(32).uppercase()}#BCDE",
                "associated_data_hash" to "hash:${"33".repeat(32).uppercase()}#CDEF",
                "executed_at_ms" to 42L,
                "expires_at_ms" to 142L,
            ),
            "signature" to "aa".repeat(64),
        )

        val response = transport.verifyRamLfeReceipt(receipt, "C0FFEE", applicationAuth()).join()

        assertTrue(response.valid)
        assertEquals("identifier_lookup_retail", response.programId)
        assertEquals(true, response.outputHashMatches)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("https://torii.example/api/v1/ram-lfe/receipts/verify", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("c0ffee", payload["output_hex"])
        assertTrue(payload["receipt"] is Map<*, *>)
    }

    @Test
    fun ramLfeResponseParsersRejectNonExactFields() {
        val canonicalExecute = ramLfeExecuteResponseJson()
        val executeCases = listOf(
            "program_id" to canonicalExecute.replace(
                "\"program_id\": \"identifier_lookup_retail\"",
                "\"program_id\": \" identifier_lookup_retail\"",
            ),
            "opaque_hash" to canonicalExecute.replace(
                "\"opaque_hash\": \"${"11".repeat(32)}\"",
                "\"opaque_hash\": \" ${"11".repeat(32)}\"",
            ),
            "receipt_hash" to canonicalExecute.replace(
                "\"receipt_hash\": \"${"22".repeat(32)}\"",
                "\"receipt_hash\": \"${"22".repeat(32)} \"",
            ),
            "output_hash" to canonicalExecute.replace(
                "\"output_hash\": \"${"44".repeat(32)}\"",
                "\"output_hash\": \" ${"44".repeat(32)}\"",
            ),
            "associated_data_hash" to canonicalExecute.replace(
                "\"associated_data_hash\": \"${"55".repeat(32)}\"",
                "\"associated_data_hash\": \"${"55".repeat(32)} \"",
            ),
            "backend" to canonicalExecute.replace(
                "\"backend\": \"bfv-programmed-sha3-256-v1\"",
                "\"backend\": \" bfv-programmed-sha3-256-v1\"",
            ),
            "verification_mode" to canonicalExecute.replace(
                "\"verification_mode\": \"signed\"",
                "\"verification_mode\": \"Signed\"",
            ),
        )
        for ((field, body) in executeCases) {
            val error = assertFailsWith<RuntimeException> {
                RamLfeJsonParser.parseExecuteResponse(body.toByteArray(StandardCharsets.UTF_8))
            }
            assertTrue(
                error.message?.contains("ram-lfe execute response.$field") == true,
                "expected ram-lfe execute response.$field failure, got $error",
            )
        }

        val canonicalVerify = ramLfeReceiptVerifyResponseJson()
        val verifyCases = listOf(
            "program_id" to canonicalVerify.replace(
                "\"program_id\": \"identifier_lookup_retail\"",
                "\"program_id\": \"identifier_lookup_retail \"",
            ),
            "backend" to canonicalVerify.replace(
                "\"backend\": \"bfv-programmed-sha3-256-v1\"",
                "\"backend\": \"BFV-programmed-sha3-256-v1\"",
            ),
            "verification_mode" to canonicalVerify.replace(
                "\"verification_mode\": \"signed\"",
                "\"verification_mode\": \" signed\"",
            ),
            "output_hash" to canonicalVerify.replace(
                "\"output_hash\": \"${"44".repeat(32)}\"",
                "\"output_hash\": \"${"44".repeat(32)} \"",
            ),
            "associated_data_hash" to canonicalVerify.replace(
                "\"associated_data_hash\": \"${"55".repeat(32)}\"",
                "\"associated_data_hash\": \" ${"55".repeat(32)}\"",
            ),
        )
        for ((field, body) in verifyCases) {
            val error = assertFailsWith<RuntimeException> {
                RamLfeJsonParser.parseReceiptVerifyResponse(body.toByteArray(StandardCharsets.UTF_8))
            }
            assertTrue(
                error.message?.contains("ram-lfe receipt verify response.$field") == true,
                "expected ram-lfe receipt verify response.$field failure, got $error",
            )
        }
    }

    @Test
    fun createVpnQuoteSignsCanonicalBodyAndParsesOpenLeaseInstruction() {
        val quoteId = "11".repeat(32)
        val meteringKey = validEd25519PublicKeyHex
        val executor = StubResponseExecutor(
            statusCode = 201,
            body = vpnQuoteJson(quoteId, meteringKey).toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private, 1_700_000_000_000L, "vpn-nonce-1")
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )

        val quote = transport.createVpnQuote(
            VpnQuoteCreateRequest("low-latency", "0x$meteringKey"),
            auth,
        ).join()

        assertEquals(quoteId, quote.quoteId)
        assertEquals(quoteId, quote.leaseIdHex)
        assertEquals(meteringKey, quote.meteringPublicKeyHex)
        assertEquals("55".repeat(1_952), quote.relayMldsa65PublicKeyHex)
        assertEquals("iroha_data_model::isi::vpn::OpenVpnLeaseEscrow", quote.openLeaseInstruction.wireId)

        val request = executor.lastRequest
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/vpn/quotes", request.uri.toString())
        assertEquals("""{"exit_class":"low-latency","metering_public_key_hex":"$meteringKey"}""", readBody(request))
        assertEquals("alice@universal", request.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        assertEquals("1700000000000", request.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS]?.first())
        assertEquals("vpn-nonce-1", request.headers[CanonicalRequestSigner.HEADER_NONCE]?.first())
        assertCanonicalSignature(request, keyPair.public, 1_700_000_000_000L, "vpn-nonce-1")
    }

    @Test
    fun vpnSessionIdNormalizerAccepts16BytesAndRejects32Bytes() {
        val sessionId = "ab".repeat(16)
        assertEquals(
            sessionId,
            HttpClientTransport.normalizeHex16("0X${sessionId.uppercase()}", "sessionId"),
        )
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.normalizeHex16("ab".repeat(32), "sessionId")
        }
    }

    @Test
    fun quoteFeesSignsExactUnsignedPayloadAndPreservesPayer() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "intent": {
                    "payer": "authority",
                    "value": {"charge_limits": [], "gas_limit": 9000}
                  },
                  "observation": {"schedule_revision": 4},
                  "components": [],
                  "capacities": [],
                  "decision": {"accepted": true}
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private, 1_700_000_000_020L, "fee-quote-1")
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )
        val unsignedPayload = linkedMapOf<String, Any?>(
            "domain" to linkedMapOf(
                "kind" to "network",
                "value" to verifyingKeyNetworkId.literal,
            ),
            "authority" to "alice",
            "fee_payment" to testFeePayment(9_000L).toJsonMap(),
        )

        val quote = transport.quoteFees(unsignedPayload, auth).join()

        assertIs<FeePaymentIntent.Authority>(quote.intent)
        assertEquals(9_000L, quote.intent.gasLimit)
        assertEquals(4L, (quote.observation["schedule_revision"] as Number).toLong())
        assertEquals("POST", executor.lastRequest.method)
        assertEquals("https://torii.example/api/v1/fees/quote", executor.lastRequest.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val request = JsonParser.parse(readBody(executor.lastRequest)) as Map<String, Any?>
        assertEquals(unsignedPayload, request["payload"])
        assertCanonicalSignature(executor.lastRequest, keyPair.public, 1_700_000_000_020L, "fee-quote-1")

        val requestCount = executor.requestCount
        assertFailsWith<IllegalArgumentException> {
            transport.quoteFees(unsignedPayload, ToriiCanonicalRequestAuth("bob@universal", keyPair.private))
        }
        assertEquals(requestCount, executor.requestCount)
    }

    @Test
    fun feePaymentJsonRequiresExplicitNullableGasLimit() {
        val missingGas = JsonParser.parse(
            """{"payer":"authority","value":{"charge_limits":[]}}""",
        )
        assertFailsWith<IllegalArgumentException> {
            FeePaymentJson.parse(missingGas, "fee payment")
        }

        val explicitNull = JsonParser.parse(
            """{"payer":"authority","value":{"charge_limits":[],"gas_limit":null}}""",
        )
        val parsed = FeePaymentJson.parse(explicitNull, "fee payment")
        assertIs<FeePaymentIntent.Authority>(parsed)
        assertNull(parsed.gasLimit)
    }

    @Test
    fun quoteFeesRejectsLegacyIdentityAndGenesisDomainsBeforeDispatch() {
        val executor = StubResponseExecutor(200, ByteArray(0))
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private)
        fun validPayload(): MutableMap<String, Any?> = linkedMapOf(
            "domain" to linkedMapOf(
                "kind" to "network",
                "value" to verifyingKeyNetworkId.literal,
            ),
            "authority" to "alice",
            "fee_payment" to testFeePayment(9_000L).toJsonMap(),
        )

        val invalid = buildList {
            for (field in listOf("chain", "chainId", "chain_id")) {
                add(validPayload().apply { this[field] = "legacy" })
            }
            add(validPayload().apply { remove("domain") })
            add(validPayload().apply { this["domain"] = null })
            add(validPayload().apply { this["domain"] = mapOf("kind" to "genesis") })
            add(validPayload().apply {
                this["domain"] = mapOf(
                    "kind" to "network",
                    "value" to verifyingKeyNetworkId.literal.lowercase(),
                )
            })
            add(validPayload().apply {
                this["domain"] = mapOf(
                    "kind" to "network",
                    "value" to verifyingKeyNetworkId.literal,
                    "chain" to "legacy",
                )
            })
        }

        invalid.forEach { payload ->
            assertFailsWith<IllegalArgumentException> {
                transport.quoteFees(payload, auth)
            }
        }
        assertEquals(0, executor.requestCount)
    }

    @Test
    fun quoteFeesRejectsPayerRevisionAndGasSubstitution() {
        val sponsor = testMultisigAccountId()
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private)
        val sponsorIntent = FeePaymentIntent.sponsor(
            FeeSponsorProgramId(sponsor, "wallet_fx"),
            3,
            emptyList(),
            9_000L,
        )

        fun assertRejected(requested: FeePaymentIntent, responseIntent: String) {
            val executor = StubResponseExecutor(
                statusCode = 200,
                body = """
                    {
                      "intent": $responseIntent,
                      "observation": {},
                      "components": [],
                      "capacities": [],
                      "decision": {}
                    }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
            val transport = HttpClientTransport.withExecutor(
                executor = executor,
                config = signedClientConfig("https://torii.example"),
            )
            val error = assertFailsWith<java.util.concurrent.CompletionException> {
                transport.quoteFees(
                    linkedMapOf(
                        "domain" to linkedMapOf(
                            "kind" to "network",
                            "value" to verifyingKeyNetworkId.literal,
                        ),
                        "authority" to "alice",
                        "fee_payment" to requested.toJsonMap(),
                    ),
                    auth,
                ).join()
            }
            assertIs<IllegalArgumentException>(error.cause)
        }

        assertRejected(
            testFeePayment(9_000L),
            """{"payer":"authority","value":{"charge_limits":[],"gas_limit":9001}}""",
        )
        assertRejected(
            sponsorIntent,
            """{"payer":"authority","value":{"charge_limits":[],"gas_limit":9000}}""",
        )
        assertRejected(
            sponsorIntent,
            """{"payer":"sponsor","value":{"program_id":{"sponsor":"$sponsor","name":"wallet_fx"},"program_revision":4,"charge_limits":[],"gas_limit":9000}}""",
        )
    }

    @Test
    fun getFeeSponsorProgramSignsExactSelectorAndParsesLifecycle() {
        val sponsor = testMultisigAccountId()
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "id": {"sponsor": "$sponsor", "name": "wallet_fx"},
                  "payout_account": "$sponsor",
                  "lifecycle": {"state": "active", "value": null},
                  "active_revision": 3,
                  "staged_revision": 4,
                  "scheduled_activation": {"revision": 4, "activate_at_height": 100}
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_700_000_000_021L,
            "fee-program-1",
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )

        val program = transport.getFeeSponsorProgram(
            FeeSponsorProgramId(sponsor, "wallet_fx"),
            auth,
        ).join()

        assertEquals(sponsor, program.id.sponsor)
        assertEquals("wallet_fx", program.id.name)
        assertEquals(sponsor, program.payoutAccount)
        assertEquals(FeeSponsorProgramLifecycle.ACTIVE, program.lifecycle)
        assertEquals(3L, program.activeRevision)
        assertEquals(4L, program.stagedRevision)
        assertEquals(4L, program.scheduledActivation?.revision)
        assertEquals(100L, program.scheduledActivation?.activateAtHeight)
        assertEquals("POST", executor.lastRequest.method)
        assertEquals(
            "https://torii.example/api/v1/fee-sponsor-programs/by-id",
            executor.lastRequest.uri.toString(),
        )
        assertEquals("""{"program_id":"$sponsor/wallet_fx"}""", readBody(executor.lastRequest))
        assertCanonicalSignature(
            executor.lastRequest,
            keyPair.public,
            1_700_000_000_021L,
            "fee-program-1",
        )
    }

    @Test
    fun getFeeSponsorProgramRejectsSubstitutedResponseId() {
        val sponsor = testMultisigAccountId()
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "id": {"sponsor": "$sponsor", "name": "other"},
                  "lifecycle": {"state": "active", "value": null}
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )

        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            transport.getFeeSponsorProgram(
                FeeSponsorProgramId(sponsor, "wallet_fx"),
                ToriiCanonicalRequestAuth("alice@universal", keyPair.private),
            ).join()
        }
        assertIs<IllegalArgumentException>(error.cause)
    }

    @Test
    fun pushDeviceRegisterAndUnregisterSignCanonicalBody() {
        val executor = QueueResponseExecutor(listOf(202 to "", 202 to ""))
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )
        val requestBody = PushDeviceRequest(" alice ", "FCM", " token-1 ", listOf(" activity "))

        transport.registerPushDevice(
            requestBody,
            ToriiCanonicalRequestAuth("alice@universal", keyPair.private, 1_700_000_000_010L, "push-nonce-1"),
        ).join()
        transport.unregisterPushDevice(
            requestBody,
            ToriiCanonicalRequestAuth("alice@universal", keyPair.private, 1_700_000_000_011L, "push-nonce-2"),
        ).join()

        val register = executor.requests[0]
        assertEquals("POST", register.method)
        assertEquals("https://torii.example/v1/notify/devices", register.uri.toString())
        assertEquals("""{"account_id":"alice","platform":"FCM","token":"token-1","topics":["activity"]}""", readBody(register))
        assertEquals("alice@universal", register.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        assertCanonicalSignature(register, keyPair.public, 1_700_000_000_010L, "push-nonce-1")

        val unregister = executor.requests[1]
        assertEquals("DELETE", unregister.method)
        assertEquals("https://torii.example/v1/notify/devices", unregister.uri.toString())
        assertEquals(readBody(register), readBody(unregister))
        assertCanonicalSignature(unregister, keyPair.public, 1_700_000_000_011L, "push-nonce-2")
    }

    @Test
    fun vpnSessionAndReceiptMethodsUseNativeLeaseDtos() {
        val sessionId = "33".repeat(16)
        val quoteId = "34".repeat(32)
        val leaseId = "35".repeat(32)
        val paymentTxHash = "44".repeat(32)
        val meteringKey = validEd25519PublicKeyHex
        val receiptJson = vpnReceiptJson(sessionId, quoteId, leaseId, paymentTxHash, settled = true)
        val pendingReceiptJson = receiptJson.replace(
            "\"status\": \"settled\"",
            "\"status\": \"settlement_pending\"",
        )
        val executor = QueueResponseExecutor(
            listOf(
                201 to vpnSessionJson(sessionId, quoteId, paymentTxHash),
                200 to vpnSessionJson(sessionId, quoteId, paymentTxHash),
                201 to pendingReceiptJson,
                200 to """{"items":[$receiptJson],"total":1}""",
            )
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private, 1_700_000_000_001L, "vpn-nonce-2")
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example"),
        )

        val session = transport.createVpnSession(
            VpnSessionCreateRequest("standard", quoteId, "0x$paymentTxHash", meteringKey),
            auth,
        ).join()
        val fetched = transport.getVpnSession(sessionId, auth).join()
        val submitted = transport.submitVpnReceipt(
            VpnReceiptSubmitRequest("0xCAFE", "BEEF", "0x$leaseId"),
            auth,
        ).join()
        val receipts = transport.listVpnReceipts(auth).join()

        assertEquals(sessionId, session.sessionId)
        assertEquals("55".repeat(1_952), session.relayMldsa65PublicKeyHex)
        assertEquals(vpnHelperTicketHex(), session.helperTicketHex)
        assertEquals(1_576, session.helperTicketHex.length)
        assertTrue(fetched.isPresent)
        assertEquals(quoteId, fetched.get().quoteId)
        assertEquals("settlement_pending", submitted.status)
        assertEquals("750000.125", submitted.earnedFee)
        assertEquals("250000.125", submitted.refundedFee)
        assertEquals("iroha_data_model::isi::vpn::SettleVpnLease", submitted.settleLeaseInstruction?.wireId)
        assertEquals(1L, receipts.total)
        assertEquals(leaseId, receipts.items.first().leaseIdHex)
        assertEquals("settled", receipts.items.first().status)

        assertEquals("""{"exit_class":"standard","metering_public_key_hex":"$meteringKey","payment_tx_hash":"$paymentTxHash","quote_id":"$quoteId"}""", readBody(executor.requests[0]))
        assertEquals("GET", executor.requests[1].method)
        assertEquals("https://torii.example/v1/vpn/sessions/$sessionId", executor.requests[1].uri.toString())
        assertEquals("""{"client_voucher_hex":"beef","lease_id_hex":"$leaseId","relay_receipt_hex":"cafe"}""", readBody(executor.requests[2]))
        assertEquals("https://torii.example/v1/vpn/receipts", executor.requests[3].uri.toString())
    }

    @Test
    fun verifierKeyRegisterAndUpdateReturnUnsignedDrafts() {
        val backend = "halo2/ipa"
        val registerBytes = byteArrayOf(1, 2, 3)
        val updateBytes = byteArrayOf(10)
        val authority = testMultisigAccountId()
        val registerRequestBody = verifierKeyRegisterRequest(
            authority = " $authority ",
            backend = backend,
            name = " transfer_vk ",
            publicInputsSchemaHashHex = "0x${"AA".repeat(32)}",
            gasScheduleId = " halo2-default ",
            activationHeight = 10,
            withdrawHeight = 10,
            commitmentHex = verifierKeyCommitment(backend, registerBytes).uppercase(),
            verifyingKeyBytes = registerBytes,
            status = "active",
        )
        val updateRequestBody = verifierKeyUpdateRequest(
            authority = authority,
            backend = backend,
            name = "transfer_vk",
            version = 2,
            commitmentHex = verifierKeyCommitment(backend, updateBytes),
            verifyingKeyBytes = updateBytes,
            verifyingKeyLength = 1,
            status = "withdrawn",
        )
        val expectedRegisterPayload =
            HttpClientTransport.buildVerifyingKeyRegisterPayload(registerRequestBody)
        val expectedUpdatePayload =
            HttpClientTransport.buildVerifyingKeyUpdatePayload(updateRequestBody)
        val registerTransactionPayload = verifyingKeyTransactionPayload(
            expectedRegisterPayload,
            VerifyingKeyDraftOperation.REGISTER,
        )
        val updateTransactionPayload = verifyingKeyTransactionPayload(
            expectedUpdatePayload,
            VerifyingKeyDraftOperation.UPDATE,
        )
        val executor = QueueResponseExecutor(
            listOf(
                200 to verifyingKeyDraftJson(registerTransactionPayload),
                200 to verifyingKeyDraftJson(updateTransactionPayload),
            ),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setLocalSigningContext(
                    LocalSigningContext(verifyingKeyNetworkId),
                )
                .setBaseUri(URI.create("https://torii.example/api"))
                .build(),
        )

        val registerResponse = transport.registerVerifyingKey(registerRequestBody).join()
        val updateResponse = transport.updateVerifyingKey(updateRequestBody).join()

        assertFalse(registerResponse.submitted)
        assertTrue(registerResponse.transactionPayloadBytes().contentEquals(registerTransactionPayload))
        assertTrue(registerResponse.signingMessageBytes().contentEquals(IrohaHash.prehash(registerTransactionPayload)))
        assertFalse(updateResponse.submitted)
        assertTrue(updateResponse.transactionPayloadBytes().contentEquals(updateTransactionPayload))
        assertTrue(updateResponse.signingMessageBytes().contentEquals(IrohaHash.prehash(updateTransactionPayload)))
        assertEquals(2, executor.requests.size)

        val registerRequest = executor.requests[0]
        assertEquals("POST", registerRequest.method)
        assertEquals("https://torii.example/api/v1/zk/vk/register", registerRequest.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val registerPayload = JsonParser.parse(readBody(registerRequest)) as Map<String, Any?>
        assertEquals(authority, registerPayload["authority"])
        assertFalse(registerPayload.containsKey("private_key"))
        assertEquals(backend, registerPayload["backend"])
        assertEquals("transfer_vk", registerPayload["name"])
        assertEquals(1L, (registerPayload["version"] as Number).toLong())
        assertEquals("transfer-v1", registerPayload["circuit_id"])
        assertEquals("aa".repeat(32), registerPayload["public_inputs_schema_hash_hex"])
        assertEquals("halo2-default", registerPayload["gas_schedule_id"])
        assertEquals(10L, (registerPayload["activation_height"] as Number).toLong())
        assertEquals(10L, (registerPayload["withdraw_height"] as Number).toLong())
        assertEquals(verifierKeyCommitment(backend, registerBytes), registerPayload["commitment_hex"])
        assertEquals(Base64.getEncoder().encodeToString(registerBytes), registerPayload["vk_bytes"])
        assertEquals(3L, (registerPayload["vk_len"] as Number).toLong())
        assertEquals("Active", registerPayload["status"])

        val updateRequest = executor.requests[1]
        assertEquals("POST", updateRequest.method)
        assertEquals("https://torii.example/api/v1/zk/vk/update", updateRequest.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val updatePayload = JsonParser.parse(readBody(updateRequest)) as Map<String, Any?>
        assertEquals(authority, updatePayload["authority"])
        assertFalse(updatePayload.containsKey("private_key"))
        assertEquals(backend, updatePayload["backend"])
        assertEquals("transfer_vk", updatePayload["name"])
        assertEquals(2L, (updatePayload["version"] as Number).toLong())
        assertEquals("transfer-v1", updatePayload["circuit_id"])
        assertEquals("aa".repeat(32), updatePayload["public_inputs_schema_hash_hex"])
        assertFalse(updatePayload.containsKey("gas_schedule_id"))
        assertEquals(verifierKeyCommitment(backend, updateBytes), updatePayload["commitment_hex"])
        assertEquals(Base64.getEncoder().encodeToString(updateBytes), updatePayload["vk_bytes"])
        assertEquals(1L, (updatePayload["vk_len"] as Number).toLong())
        assertEquals("Withdrawn", updatePayload["status"])
    }

    @Test
    fun verifierKeyRequestsRejectMalformedInputsBeforeRequest() {
        val backend = "halo2/ipa"
        val bytes = byteArrayOf(1, 2, 3)
        val commitment = verifierKeyCommitment(backend, bytes)
        val executor = CapturingExecutor()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setLocalSigningContext(
                    LocalSigningContext(verifyingKeyNetworkId),
                )
                .setBaseUri(URI.create("https://torii.example/api"))
                .build(),
        )

        fun expectReject(block: () -> Unit) {
            val before = executor.requestCount
            assertFailsWith<IllegalArgumentException> { block() }
            assertEquals(before, executor.requestCount)
        }

        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(backend = "mock/dev")) }
        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(authority = " ")) }
        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(name = "scope:vk")) }
        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(version = 0)) }
        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(publicInputsSchemaHashHex = "abc")) }
        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(gasScheduleId = " ")) }
        expectReject { transport.registerVerifyingKey(verifierKeyRegisterRequest(verifyingKeyBytes = byteArrayOf())) }
        expectReject {
            transport.registerVerifyingKey(
                verifierKeyRegisterRequest(verifyingKeyBytes = bytes, verifyingKeyLength = 2),
            )
        }
        expectReject {
            transport.registerVerifyingKey(
                verifierKeyRegisterRequest(
                    backend = backend,
                    verifyingKeyBytes = bytes,
                    commitmentHex = "00".repeat(32),
                ),
            )
        }
        expectReject { transport.updateVerifyingKey(verifierKeyUpdateRequest(activationHeight = 8, withdrawHeight = 7)) }
        expectReject { transport.updateVerifyingKey(verifierKeyUpdateRequest(status = "retired")) }
        expectReject {
            transport.registerVerifyingKey(
                verifierKeyRegisterRequest(
                    verifyingKeyBytes = null,
                    verifyingKeyLength = 3,
                    commitmentHex = null,
                ),
            )
        }
        expectReject {
            transport.registerVerifyingKey(
                verifierKeyRegisterRequest(
                    backend = backend,
                    verifyingKeyBytes = bytes,
                    commitmentHex = commitment,
                    maxProofBytes = 4_294_967_296L,
                ),
            )
        }
    }

    @Test
    fun verifierKeyDraftCanonicalInstructionUsesU8StatusDiscriminant() {
        val fixture = loadSharedFixture("fixtures/zk/verifying_key_record_v1.json")
        @Suppress("UNCHECKED_CAST")
        val request = obj(fixture, "request") as Map<String, Any>
        val payload = (
            VerifyingKeyDraftBinding.expectedInstruction(
                request,
                VerifyingKeyDraftOperation.REGISTER,
            ).payload as WirePayload
        ).payloadBytes
        val actualHex = hex(payload)
        val backendTagFrame = obj(fixture, "backend_tag_frame")
        val statusBoundary = obj(fixture, "status_boundary")
        val backendTagOffset = (backendTagFrame.getValue("offset") as Number).toInt()
        val backendTagHex = string(backendTagFrame, "hex")
        val statusOffset = (statusBoundary.getValue("offset") as Number).toInt()
        val statusHex = string(statusBoundary, "hex")

        assertEquals(string(fixture, "expected_inner_frame_hex"), actualHex)
        assertEquals(
            (fixture.getValue("expected_inner_frame_bytes") as Number).toInt(),
            payload.size,
        )
        assertEquals(
            backendTagHex,
            hex(payload.copyOfRange(backendTagOffset, backendTagOffset + backendTagHex.length / 2)),
            "backend tag must remain a four-byte u32 field at the canonical offset",
        )
        assertEquals(
            statusHex,
            hex(payload.copyOfRange(statusOffset, statusOffset + statusHex.length / 2)),
            "absent inline key must end immediately before the one-byte status field",
        )
    }

    @Test
    fun verifierKeyDraftParserRejectsNonExactOrTamperedResponses() {
        val request = HttpClientTransport.buildVerifyingKeyRegisterPayload(
            verifierKeyRegisterRequest(),
        )
        val transactionPayload = verifyingKeyTransactionPayload(
            request,
            VerifyingKeyDraftOperation.REGISTER,
        )
        val valid = verifyingKeyDraftJson(transactionPayload)
        val parsed = VerifyingKeyTransactionDraftParser.parseRegister(
            valid.toByteArray(StandardCharsets.UTF_8),
            verifyingKeyNetworkId,
            request,
        )
        assertFalse(parsed.submitted)
        assertTrue(parsed.transactionPayloadBytes().contentEquals(transactionPayload))

        fun expectReject(json: String) {
            assertFailsWith<IllegalArgumentException> {
                VerifyingKeyTransactionDraftParser.parseRegister(
                    json.toByteArray(StandardCharsets.UTF_8),
                    verifyingKeyNetworkId,
                    request,
                )
            }
        }

        expectReject(
            valid.replace(
                "\"submitted\":false",
                "\"submitted\":false,\"retired_private_key\":\"secret\"",
            ),
        )
        expectReject(valid.replace("\"submitted\":false", "\"submitted\":true"))
        val payloadB64 = Base64.getEncoder().encodeToString(transactionPayload)
        expectReject(valid.replace(payloadB64, "$payloadB64="))
        val signingMessageB64 =
            Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionPayload))
        expectReject(
            valid.replace(
                signingMessageB64,
                Base64.getEncoder().encodeToString(ByteArray(31)),
            ),
        )
        expectReject(
            valid.replace(
                signingMessageB64,
                Base64.getEncoder().encodeToString(ByteArray(32) { 7 }),
            ),
        )
        expectReject(verifyingKeyDraftJson(byteArrayOf(1, 2, 3, 4)))
        expectReject(valid.replace("\"transaction_payload_b64\":", "\"payload_b64\":"))

        val wrongStatusTransport = HttpClientTransport.withExecutor(
            executor = StubResponseExecutor(202, valid.toByteArray(StandardCharsets.UTF_8)),
            config = ClientConfig.builder()
                .setLocalSigningContext(
                    LocalSigningContext(verifyingKeyNetworkId),
                )
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )
        val error = assertFailsWith<CompletionException> {
            val verifyingKeyBytes = byteArrayOf(9)
            wrongStatusTransport.registerVerifyingKey(
                verifierKeyRegisterRequest(
                    verifyingKeyBytes = verifyingKeyBytes,
                    commitmentHex = verifierKeyCommitment("halo2/ipa", verifyingKeyBytes),
                ),
            ).join()
        }
        assertTrue(error.cause?.message?.contains("status 202") == true)
    }

    @Test
    fun verifierKeyDraftRejectsSemanticSubstitutionBeforeSigning() {
        val request = HttpClientTransport.buildVerifyingKeyRegisterPayload(
            verifierKeyRegisterRequest(
                verifyingKeyBytes = byteArrayOf(1, 2, 3),
            ),
        )
        val expectedInstruction = VerifyingKeyDraftBinding.expectedInstruction(
            request,
            VerifyingKeyDraftOperation.REGISTER,
        )

        fun expectReject(payload: ByteArray) {
            assertFailsWith<IllegalArgumentException> {
                VerifyingKeyTransactionDraftParser.parseRegister(
                    verifyingKeyDraftJson(payload).toByteArray(StandardCharsets.UTF_8),
                    verifyingKeyNetworkId,
                    request,
                )
            }
        }

        expectReject(
            verifyingKeyTransactionPayload(
                request,
                VerifyingKeyDraftOperation.UPDATE,
            ),
        )
        expectReject(
            verifyingKeyTransactionPayload(
                request,
                VerifyingKeyDraftOperation.REGISTER,
                instructions = listOf(expectedInstruction, expectedInstruction),
            ),
        )
        expectReject(
            verifyingKeyTransactionPayload(
                request,
                VerifyingKeyDraftOperation.REGISTER,
                networkId = otherNetworkId,
            ),
        )
        expectReject(
            verifyingKeyTransactionPayload(
                request,
                VerifyingKeyDraftOperation.REGISTER,
                authority = testAccountId(0x59),
            ),
        )
        expectReject(
            verifyingKeyTransactionPayload(
                request,
                VerifyingKeyDraftOperation.REGISTER,
                admissionIntent = TransactionAdmissionIntent.ORDINARY,
            ),
        )

        val changedRecord = LinkedHashMap(request)
        changedRecord["curve"] = "pasta"
        expectReject(
            verifyingKeyTransactionPayload(
                changedRecord,
                VerifyingKeyDraftOperation.REGISTER,
            ),
        )

        val canonical = verifyingKeyTransactionPayload(
            request,
            VerifyingKeyDraftOperation.REGISTER,
        )
        require(canonical[0].toInt() and 0x80 == 0)
        val noncanonical = ByteArray(canonical.size + 1)
        noncanonical[0] = (canonical[0].toInt() or 0x80).toByte()
        noncanonical[1] = 0
        canonical.copyInto(noncanonical, destinationOffset = 2, startIndex = 1)
        expectReject(noncanonical)
    }

    @Test
    fun verifierKeyDraftRejectsGenesisTransactionDomain() {
        val request = HttpClientTransport.buildVerifyingKeyRegisterPayload(
            verifierKeyRegisterRequest(),
        )
        val canonical = verifyingKeyTransactionPayload(
            request,
            VerifyingKeyDraftOperation.REGISTER,
        )
        val networkDomainLength = canonical[0].toInt() and 0xff
        require(
            canonical[0].toInt() and 0x80 == 0 &&
                networkDomainLength >= 4 &&
                canonical.size > 1 + networkDomainLength,
        ) { "fixture requires one exact one-byte-sized Network domain" }
        val genesis = ByteArray(canonical.size - networkDomainLength + 4)
        genesis[0] = 4
        genesis[1] = 1
        canonical.copyInto(
            genesis,
            destinationOffset = 5,
            startIndex = 1 + networkDomainLength,
        )

        assertFailsWith<IllegalArgumentException> {
            VerifyingKeyTransactionDraftParser.parseRegister(
                verifyingKeyDraftJson(genesis).toByteArray(StandardCharsets.UTF_8),
                verifyingKeyNetworkId,
                request,
            )
        }
    }

    @Test
    fun verifierKeyDraftRequiresLocalSigningContextBeforeRequest() {
        val executor = CapturingExecutor()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )

        assertFailsWith<IllegalStateException> {
            transport.registerVerifyingKey(verifierKeyRegisterRequest())
        }
        assertFailsWith<IllegalStateException> {
            transport.updateVerifyingKey(verifierKeyUpdateRequest())
        }
        assertEquals(0, executor.requestCount)
    }

    @Test
    fun resolveAccountAliasParsesSuccessfulResponse() {
        val accountId = testAccountId(0x11)
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "alice@universal",
                  "account_id": "$accountId",
                  "index": 42,
                  "source": "directory"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.resolveAccountAlias("alice@universal").join()

        assertTrue(response.isPresent)
        val parsed = response.get()
        assertEquals("alice@universal", parsed.alias)
        assertEquals(accountId, parsed.accountId)
        assertEquals(BigInteger.valueOf(42), parsed.index)
        assertEquals("directory", parsed.source)

        val request = executor.lastRequest
        assertNotNull(request)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/aliases/resolve", request.uri.toString())
        @Suppress("UNCHECKED_CAST")
        val payload = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals("alice@universal", payload["alias"])
        assertEquals(1, payload.size)
    }

    @Test
    fun resolveRestrictedAccountAliasUsesCanonicalAuthentication() {
        val accountId = testAccountId(0x42)
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "merchant@private",
                  "account_id": "$accountId",
                  "source": "world_state"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_700_000_000_000L,
            "alias-resolve-nonce-1",
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )

        val response = transport.resolveAccountAlias("merchant@private", auth).join()

        assertTrue(response.isPresent)
        assertEquals(accountId, response.get().accountId)
        val request = assertNotNull(executor.lastRequest)
        assertEquals("alice@universal", request.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        assertEquals("1700000000000", request.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS]?.first())
        assertEquals("alias-resolve-nonce-1", request.headers[CanonicalRequestSigner.HEADER_NONCE]?.first())
        assertCanonicalSignature(request, keyPair.public, 1_700_000_000_000L, "alias-resolve-nonce-1")
    }

    @Test
    fun aliasSetupPlanningIsCanonicalSignedReadOnlyAndParsesTypedPlan() {
        val authority = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x41), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val assetBytes = ByteArray(16) { it.toByte() }.also {
            it[6] = 0x46
            it[8] = 0x88.toByte()
        }
        val asset = AssetDefinitionIdEncoder.encodeFromBytes(assetBytes)
        val alias = ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L)
        val guard = AliasQuoteGuardV1(3, asset, "5", 1_700_000_100_000L)
        val intent = AliasIntentV1.AccountAlias(
            AliasAccountIntentV1(
                alias,
                authority,
                AccountProvisionV1.CREATE,
                AccountAliasRoleV1.PRIMARY,
            ),
        )
        val requestBody = AliasSetupPlanRequestV1(
            listOf(EnsureAlias(intent, AliasLeaseAcquisitionV1(1), guard)),
        )
        val planBody = AliasTransactionPlanBodyV1(
            1,
            authority,
            verifyingKeyNetworkId,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            listOf(
                AliasPlanResourceV1(
                    intent,
                    AliasPlanDispositionV1.CREATE,
                    AliasLeaseQuoteV1(
                        AliasTargetV1.AccountAlias(alias),
                        1,
                        "3",
                        guard,
                        1_800_000_000_000L,
                        1_800_000_100_000L,
                        1_800_000_200_000L,
                    ),
                    0,
                ),
            ),
            listOf(AliasFramedInstructionV1(EnsureAlias.WIRE_ID, byteArrayOf(0x4e, 0x52, 0x54, 0x30))),
            listOf(AliasAssetTotalV1(asset, "3")),
            emptyList(),
            emptyList(),
            1_700_000_100_000L,
        )
        val responsePlan = AliasTransactionPlanV1(planBody, "03".repeat(32))
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = JsonEncoder.encode(responsePlan.toJsonMap()).toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            authority,
            keyPair.private,
            1_700_000_000_000L,
            "alias-plan-nonce-1",
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = signedClientConfig("https://torii.example/api"),
        )

        val plan = transport.planAliasSetup(requestBody, auth).join()

        assertEquals(authority, plan.body.authority)
        assertEquals(AliasPlanDispositionV1.CREATE, plan.body.resources.single().disposition)
        val request = assertNotNull(executor.lastRequest)
        assertEquals("POST", request.method)
        assertEquals("https://torii.example/api/v1/aliases/setup/plan", request.uri.toString())
        assertEquals(AccountAddress.parseEncodedIgnoringCurveSupport(authority, null).canonicalHex(), request.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        @Suppress("UNCHECKED_CAST")
        val sent = JsonParser.parse(readBody(request)) as Map<String, Any?>
        assertEquals(1L, sent["schema_version"])
        assertEquals(1, (sent["intents"] as List<*>).size)
        assertFalse(sent.containsKey("private_key"))
        assertFalse(sent.containsKey("payment_proof"))
        assertCanonicalSignature(request, keyPair.public, 1_700_000_000_000L, "alias-plan-nonce-1")
    }

    @Test
    fun lifecyclePlanningAndSponsoredOnboardingUseOnlySafePlannerRoutes() {
        val authority = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x41), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val targetAccount = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x42), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val assetBytes = ByteArray(16) { it.toByte() }.also {
            it[6] = 0x46
            it[8] = 0x88.toByte()
        }
        val asset = AssetDefinitionIdEncoder.encodeFromBytes(assetBytes)
        val alias = ResolvedAccountAliasV1(AccountAliasName.parse("merchant@banka.paynet"), 7L)
        val target = AliasTargetV1.AccountAlias(alias)
        val guard = AliasQuoteGuardV1(3, asset, "5", 1_700_000_100_000L)
        val renewal = RenewAliasLease(target, 1_800_000_000_000L, 1_900_000_000_000L, guard)
        val renewalRequest = AliasLeaseRenewPlanRequestV1(renewal)
        val lifecycleBody = AliasLifecycleTransactionPlanBodyV1(
            1,
            authority,
            verifyingKeyNetworkId,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            renewalRequest.operation,
            AliasLifecyclePlanDispositionV1.APPLY,
            AliasFramedInstructionV1(RenewAliasLease.WIRE_ID, byteArrayOf(1, 2, 3)),
            AliasLeaseQuoteV1(
                target,
                1,
                "3",
                guard,
                1_900_000_000_000L,
                1_900_000_100_000L,
                1_900_000_200_000L,
            ),
            listOf(AliasAssetTotalV1(asset, "3")),
            emptyList(),
            emptyList(),
            guard.validUntilMs,
        )
        val lifecyclePlan = AliasLifecycleTransactionPlanV1(lifecycleBody, "03".repeat(32))
        val lifecycleExecutor = StubResponseExecutor(
            200,
            JsonEncoder.encode(lifecyclePlan.toJsonMap()).toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            authority,
            keyPair.private,
            1_700_000_000_000L,
            "alias-lifecycle-nonce-1",
        )
        val lifecycleTransport = HttpClientTransport.withExecutor(
            lifecycleExecutor,
            signedClientConfig("https://torii.example/api"),
        )

        lifecycleTransport.planAliasLeaseRenewal(renewalRequest, auth).join()

        val lifecycleHttpRequest = assertNotNull(lifecycleExecutor.lastRequest)
        assertEquals(
            "https://torii.example/api/v1/aliases/lease/renew/plan",
            lifecycleHttpRequest.uri.toString(),
        )
        assertEquals(AccountAddress.parseEncodedIgnoringCurveSupport(authority, null).canonicalHex(), lifecycleHttpRequest.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.first())
        val lifecycleJson = readBody(lifecycleHttpRequest)
        assertFalse(lifecycleJson.contains("private_key"))
        assertFalse(lifecycleJson.contains("payment_proof"))
        assertCanonicalSignature(
            lifecycleHttpRequest,
            keyPair.public,
            1_700_000_000_000L,
            "alias-lifecycle-nonce-1",
        )

        val intent = AliasIntentV1.AccountAlias(
            AliasAccountIntentV1(alias, targetAccount, AccountProvisionV1.CREATE, AccountAliasRoleV1.PRIMARY),
        )
        val onboardingRequest = AccountOnboardingPlanRequestV1(
            alias.canonicalName.canonicalText(),
            targetAccount,
        )
        val onboardingSigner = Ed25519PrivateKeyParameters(ByteArray(32) { 0x53.toByte() }, 0)
        val onboardingAuthority = AccountAddress.fromAccount(
            onboardingSigner.generatePublicKey().encoded,
            "ed25519",
        ).toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val onboardingBody = AccountOnboardingPlanBodyV1(
            1,
            onboardingRequest,
            onboardingAuthority,
            verifyingKeyNetworkId,
            AliasPlanAnchorV1(9, "01".repeat(32)),
            AliasPlanResourceV1(intent, AliasPlanDispositionV1.CREATE, null, 0),
            AliasLeaseAcquisitionV1(1),
            guard,
            listOf(AliasFramedInstructionV1(EnsureAlias.WIRE_ID, byteArrayOf(4, 5, 6))),
            null,
            guard.validUntilMs,
        )
        val receipt = signedOnboardingReceipt(onboardingBody, onboardingSigner)
        val onboardingExecutor = StubResponseExecutor(
            200,
            JsonEncoder.encode(receipt.toJsonMap()).toByteArray(StandardCharsets.UTF_8),
        )
        val onboardingTransport = HttpClientTransport.withExecutor(
            onboardingExecutor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )
        val token = "onboarding-token-value-1234567890abcd"

        onboardingTransport.planSponsoredAccountOnboarding(
            onboardingRequest,
            token,
            onboardingAuthority,
            verifyingKeyNetworkId,
        ).join()

        val onboardingHttpRequest = assertNotNull(onboardingExecutor.lastRequest)
        assertEquals("https://torii.example/api/v1/accounts/onboard/plan", onboardingHttpRequest.uri.toString())
        assertEquals(token, onboardingHttpRequest.headers["X-Iroha-Onboarding-Token"]?.single())
        val onboardingJson = readBody(onboardingHttpRequest)
        assertFalse(onboardingJson.contains(token))
        assertFalse(onboardingJson.contains("private_key"))
        assertFalse(onboardingJson.contains("payment_proof"))

        val readinessExecutor = StubResponseExecutor(
            200,
            """{"version":1,"status":{"status":"ready","value":null},"diagnostics":[]}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        val readinessTransport = HttpClientTransport.withExecutor(
            readinessExecutor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )
        val readiness = readinessTransport.getAccountOnboardingReadiness(token).join()
        assertEquals(AliasSetupStatusV1.READY, readiness.status)
        val readinessRequest = assertNotNull(readinessExecutor.lastRequest)
        assertEquals("GET", readinessRequest.method)
        assertEquals(
            "https://torii.example/api/v1/accounts/onboarding/readiness",
            readinessRequest.uri.toString(),
        )
        assertTrue(readinessRequest.body.isEmpty())
        assertEquals(token, readinessRequest.headers["X-Iroha-Onboarding-Token"]?.single())
    }

    @Test
    fun sponsoredOnboardingLegacyApplyResponseIsRejected() {
        val accountId = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x22), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        assertFailsWith<IllegalStateException> {
            AccountOnboardingJsonParser.parsePrepareResponse(
                """{"account_id":"$accountId","alias":"merchant@banka.paynet","status":"Unchanged","disposition":{"kind":"no_op","value":null}}"""
                    .toByteArray(StandardCharsets.UTF_8),
            )
        }
    }

    @Test
    fun proofRequiredCurrentStateUsesOneAtomicPostAndClassifiesExactSnapshot() {
        val fixture = atomicOnboardingProofFixture()
        val canonicalAuth = applicationAuth()
        val blockHash = AccountOnboardingBlockHashV1(verifyingKeyNetworkId.literal)
        val responses = listOf(
            fixture.accountId to AccountOnboardingCurrentStateV1.Outcome.APPLIED,
            null to AccountOnboardingCurrentStateV1.Outcome.ALIAS_ABSENT,
            testAccountId(0x49) to AccountOnboardingCurrentStateV1.Outcome.ALIAS_CONFLICT,
        )
        responses.forEach { (target, expectedOutcome) ->
            val response = AccountOnboardingCurrentStateResponseV1(
                AccountOnboardingCurrentStateResponseV1.VERSION,
                verifyingKeyNetworkId,
                fixture.accountId,
                fixture.alias,
                true,
                target,
                BigInteger.valueOf(41),
                blockHash,
            )
            val executor = StubResponseExecutor(
                200,
                JsonEncoder.encode(response.toJsonMap()).toByteArray(StandardCharsets.UTF_8),
                "application/json",
            )
            val transport = HttpClientTransport.withExecutor(
                executor,
                signedClientConfig("https://torii.example/api"),
            )

            val result = transport.verifyAccountOnboardingCurrentState(
                fixture.proofRequired,
                fixture.request,
                fixture.receipt,
                fixture.binding,
                fixture.authority,
                verifyingKeyNetworkId,
                canonicalAuth,
            ).join()

            assertEquals(expectedOutcome, result.outcome)
            assertEquals(BigInteger.valueOf(41), result.blockHeight)
            assertEquals(blockHash, result.blockHash)
            assertEquals(1, executor.requestCount)
            assertEquals("POST", executor.lastRequest.method)
            assertEquals(
                "https://torii.example/api/v1/accounts/onboarding/current-state",
                executor.lastRequest.uri.toString(),
            )
            assertNull(executor.lastRequest.headers["X-Iroha-Onboarding-Token"])
            assertNotNull(executor.lastRequest.headers[CanonicalRequestSigner.HEADER_ACCOUNT])
            assertNotNull(executor.lastRequest.headers[CanonicalRequestSigner.HEADER_SIGNATURE])
            assertNotNull(executor.lastRequest.headers[CanonicalRequestSigner.HEADER_TIMESTAMP_MS])
            assertNotNull(executor.lastRequest.headers[CanonicalRequestSigner.HEADER_NONCE])
            @Suppress("UNCHECKED_CAST")
            val sent = JsonParser.parse(readBody(executor.lastRequest)) as Map<String, Any?>
            assertEquals(setOf("version", "account_id", "alias"), sent.keys)
            assertEquals(fixture.accountId, sent["account_id"])
            assertEquals(fixture.alias, sent["alias"])
        }
    }

    @Test
    fun proofRequiredCurrentStateRejectsSubstitutionOpenShapeAndInvalidAnchor() {
        val fixture = atomicOnboardingProofFixture()
        val canonicalAuth = applicationAuth()
        val exact = linkedMapOf<String, Any?>(
            "version" to 1,
            "network_id" to verifyingKeyNetworkId.literal,
            "account_id" to fixture.accountId,
            "alias" to fixture.alias,
            "account_exists" to true,
            "alias_target_account_id" to fixture.accountId,
            "observed_block_height" to 51,
            "observed_block_hash" to verifyingKeyNetworkId.literal,
        )
        fun changed(field: String, value: Any?): Map<String, Any?> = LinkedHashMap(exact).also {
            it[field] = value
        }
        val invalid = listOf(
            changed("version", 2),
            changed("network_id", otherNetworkId.literal),
            changed("account_id", testAccountId(0x4a)),
            changed("alias", "other@banka.paynet"),
            changed("account_exists", false).toMutableMap().also { it["alias_target_account_id"] = null },
            changed("observed_block_height", 0),
            changed("observed_block_hash", verifyingKeyNetworkId.literal.lowercase()),
            changed("alias_target_account_id", " ${fixture.accountId}"),
            LinkedHashMap(exact).also { it["legacy_account_state"] = "Applied" },
            LinkedHashMap(exact).also { it.remove("alias_target_account_id") },
        )
        invalid.forEachIndexed { index, body ->
            val executor = StubResponseExecutor(
                200,
                JsonEncoder.encode(body).toByteArray(StandardCharsets.UTF_8),
                "application/json",
            )
            val transport = HttpClientTransport.withExecutor(
                executor,
                signedClientConfig("https://torii.example/api"),
            )
            assertFailsWith<CompletionException>("invalid response $index") {
                transport.verifyAccountOnboardingCurrentState(
                    fixture.proofRequired,
                    fixture.request,
                    fixture.receipt,
                    fixture.binding,
                    fixture.authority,
                    verifyingKeyNetworkId,
                    canonicalAuth,
                ).join()
            }
            assertEquals(1, executor.requestCount, "invalid response $index")
        }
    }

    @Test
    fun typedRestrictedAliasListsSendCanonicalRequestHeaders() {
        val account = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x45), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val executor = StubResponseExecutor(
            200,
            """{"account_id":"$account","total":1,"items":[{"alias":"merchant@banka.paynet","dataspace":"paynet","domain":"banka","is_primary":true}],"source":"on_chain"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            account,
            keyPair.private,
            1_700_000_000_000L,
            "alias-list-nonce-1",
        )
        val transport = HttpClientTransport.withExecutor(
            executor,
            signedClientConfig("https://torii.example/api"),
        )

        val response = transport.listAccountAliases(
            AccountAliasesByAccountRequest(account, "paynet", "banka"),
            auth,
        ).join()

        assertTrue(response.isPresent)
        assertEquals("merchant@banka.paynet", response.get().items.single().alias)
        val request = assertNotNull(executor.lastRequest)
        assertEquals("https://torii.example/api/v1/aliases/by-account", request.uri.toString())
        assertEquals(AccountAddress.parseEncodedIgnoringCurveSupport(account, null).canonicalHex(), request.headers[CanonicalRequestSigner.HEADER_ACCOUNT]?.single())
        assertCanonicalSignature(request, keyPair.public, 1_700_000_000_000L, "alias-list-nonce-1")
    }

    @Test
    fun typedAliasReadsRejectSubstitutedSelectors() {
        val account = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x45), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val otherAccount = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x46), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)

        val indexExecutor = StubResponseExecutor(
            200,
            """{"index":8,"alias":"merchant@paynet","account_id":"$account"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        val indexTransport = HttpClientTransport.withExecutor(
            indexExecutor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )
        assertFailsWith<CompletionException> {
            indexTransport.resolveAccountAliasIndex(BigInteger.valueOf(7)).join()
        }

        val accountExecutor = StubResponseExecutor(
            200,
            """{"account_id":"$otherAccount","total":0,"items":[]}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        val accountTransport = HttpClientTransport.withExecutor(
            accountExecutor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )
        assertFailsWith<CompletionException> {
            accountTransport.listAccountAliases(AccountAliasesByAccountRequest(account)).join()
        }

        val aliasExecutor = StubResponseExecutor(
            200,
            """{"alias":"other@paynet","account_id":"$account"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        val aliasTransport = HttpClientTransport.withExecutor(
            aliasExecutor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example")).build(),
        )
        assertFailsWith<CompletionException> {
            aliasTransport.resolveAccountAlias("merchant@paynet").join()
        }
    }

    @Test
    fun resolveAccountAliasParsesSuccessfulResponseWithoutIndex() {
        val accountId = testAccountId(0x13)
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "banking@centralbank.universal",
                  "account_id": "$accountId",
                  "source": "rekey_record"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.resolveAccountAlias("banking@centralbank.universal").join()

        assertTrue(response.isPresent)
        val parsed = response.get()
        assertEquals("banking@centralbank.universal", parsed.alias)
        assertEquals(accountId, parsed.accountId)
        assertNull(parsed.index)
        assertEquals("rekey_record", parsed.source)
    }

    @Test
    fun resolveAccountAliasReturnsEmptyOnNotFound() {
        val executor = StubResponseExecutor(
            statusCode = 404,
            body = byteArrayOf(),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val response = transport.resolveAccountAlias("missing@universal").join()

        assertFalse(response.isPresent)
        assertNull(response.orElse(null))
    }

    @Test
    fun resolveAccountAliasRejectsNonIntegerIndex() {
        val accountId = testAccountId(0x14)
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "alias": "alice@universal",
                  "account_id": "$accountId",
                  "index": 3.5
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<java.util.concurrent.ExecutionException> {
            transport.resolveAccountAlias("alice@universal").get()
        }
        assertNotNull(error.cause)
    }

    @Test
    fun accountAliasParserRejectsNonExactResponseFields() {
        val accountId = testAccountId(0x15)
        val canonical =
            """
                {
                  "alias": "alice@universal",
                  "account_id": "$accountId",
                  "index": 7,
                  "source": "directory"
                }
            """.trimIndent()
        val cases = listOf(
            "account alias resolution.alias" to canonical.replace(
                "\"alias\": \"alice@universal\"",
                "\"alias\": \" alice@universal\"",
            ),
            "account alias resolution.account_id" to canonical.replace(
                "\"account_id\": \"$accountId\"",
                "\"account_id\": \"$accountId \"",
            ),
            "account alias resolution.source" to canonical.replace(
                "\"source\": \"directory\"",
                "\"source\": \" directory\"",
            ),
        )
        for ((field, body) in cases) {
            val error = assertFailsWith<RuntimeException> {
                AccountAliasJsonParser.parseResolution(body.toByteArray(StandardCharsets.UTF_8))
            }
            assertTrue(
                error.message?.contains(field) == true,
                "expected $field failure, got $error",
            )
        }
    }

    @Test
    fun resolveAccountAliasPropagatesMalformedJson() {
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = "not a json object".toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<java.util.concurrent.ExecutionException> {
            transport.resolveAccountAlias("alice@universal").get()
        }
        assertNotNull(error.cause)
    }

    @Test
    fun submitTransactionPrefersAuthoritativeReceiptHashHeaderForPolling() {
        val transaction = sampleTransaction(0x11)
        val localHash = SignedTransactionHasher.hashHex(transaction)
        val authoritativeHash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        val executor = TrackingExecutor(
            expectedHash = authoritativeHash,
            submitHeaderHash = authoritativeHash,
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .build(),
        )

        val response = transport.submitTransaction(transaction).join()

        assertFalse(localHash == response.hashHex())
        assertEquals(authoritativeHash, response.hashHex())

        val payload = transport
            .waitForTransactionStatus(response.hashHex()!!, PipelineStatusOptions(intervalMillis = 0L))
            .join()

        assertEquals("Applied", PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        assertTrue(executor.observedExpectedHash)
        assertTrue(executor.observedCommittedAsPending)
    }

    @Test
    fun submitTransactionRejectsNonCanonicalReceiptHashHeaders() {
        val transaction = sampleTransaction(0x12)
        val localHash = SignedTransactionHasher.hashHex(transaction)
        val invalidHeaders = listOf(
            localHash.uppercase(),
            " $localHash",
            "$localHash ",
            "0x$localHash",
            localHash.dropLast(2),
            localHash.dropLast(1) + "0",
        )

        for (header in invalidHeaders) {
            val transport = HttpClientTransport.withExecutor(
                executor = TrackingExecutor(localHash, header),
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example/api"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException>(header) {
                transport.submitTransaction(transaction).join()
            }
            assertIs<IllegalStateException>(error.cause)
        }
    }

    @Test
    fun pipelineSuccessStatusIsNotPubliclyConfigurable() {
        val publicNames = PipelineStatusOptions::class.java.methods
            .map { it.name }
            .toSet()

        assertFalse("getSuccessStatuses" in publicNames)
        assertFalse("successStatuses" in publicNames)
    }

    @Test
    fun pipelineFailureClassificationIsFixed() {
        assertFailsWith<IllegalArgumentException> {
            PipelineStatusOptions(intervalMillis = -1L)
        }
        assertFailsWith<IllegalArgumentException> {
            PipelineStatusOptions(timeoutMillis = -1L)
        }
        val publicNames = PipelineStatusOptions::class.java.methods.map { it.name }.toSet()
        assertFalse("getFailureStatuses" in publicNames)
        assertFalse("failureStatuses" in publicNames)

        for (kind in listOf("Rejected", "Expired")) {
            val hash = "ab".repeat(32)
            val executor = StubResponseExecutor(
                statusCode = 200,
                body = """{"hash":"$hash","status":{"kind":"$kind"},"scope":"global","resolved_from":"state"}"""
                    .toByteArray(StandardCharsets.UTF_8),
            )
            val transport = HttpClientTransport.withExecutor(
                executor = executor,
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example/api"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException> {
                transport.waitForTransactionStatus(
                    hash,
                    PipelineStatusOptions(
                        intervalMillis = 0L,
                        maxAttempts = 1,
                    ),
                ).join()
            }
            assertIs<TransactionStatusException>(error.cause)
        }

        for ((kind, statusFields) in listOf(
            "Applied" to ",\"block_height\":7",
            "Rejected" to "",
            "Expired" to "",
        )) {
            for (source in listOf("cache", "queue")) {
                val hash = "ab".repeat(32)
                val executor = StubResponseExecutor(
                    statusCode = 200,
                    body =
                        """{"hash":"$hash","status":{"kind":"$kind"$statusFields},"scope":"global","resolved_from":"$source"}"""
                            .toByteArray(StandardCharsets.UTF_8),
                )
                val transport = HttpClientTransport.withExecutor(
                    executor = executor,
                    config = ClientConfig.builder()
                        .setBaseUri(URI.create("https://torii.example/api"))
                        .build(),
                )
                val error = assertFailsWith<CompletionException> {
                    transport.waitForTransactionStatus(
                        hash,
                        PipelineStatusOptions(intervalMillis = 0L, maxAttempts = 1),
                    ).join()
                }
                assertIs<TransactionTimeoutException>(error.cause)
            }
        }

        val cachedApplied = linkedMapOf<String, Any>(
            "hash" to "ab".repeat(32),
            "status" to mapOf("kind" to "Applied", "block_height" to 7),
            "scope" to "global",
            "resolved_from" to "cache",
        )
        assertFailsWith<IllegalStateException> {
            TransactionFinality.requireApplied(cachedApplied, "ab".repeat(32))
        }
    }

    @Test
    fun pipelineStatusRejectsHttp202And204() {
        val hash = "ab".repeat(32)
        for (statusCode in listOf(202, 204)) {
            val transport = HttpClientTransport.withExecutor(
                executor = StubResponseExecutor(statusCode = statusCode, body = byteArrayOf()),
                config = ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example/api"))
                    .build(),
            )

            val error = assertFailsWith<CompletionException> {
                transport.waitForTransactionStatus(
                    hash,
                    PipelineStatusOptions(intervalMillis = 0L),
                ).join()
            }
            assertIs<TransactionStatusHttpException>(error.cause)
            assertEquals(statusCode, (error.cause as TransactionStatusHttpException).statusCode)
        }
    }

    @Test
    fun publicPipelineStatusRejectsRetiredSensitiveFields() {
        val hash = "ab".repeat(32)
        val payload = mapOf<String, Any>(
            "hash" to hash,
            "status" to mapOf(
                "kind" to "Rejected",
                "rejection_reason" to "secret",
            ),
            "summary" to "Rejected: secret",
            "diagnostics" to listOf(mapOf("message" to "secret")),
            "scope" to "global",
            "resolved_from" to "state",
        )

        val error = assertFailsWith<IllegalStateException> {
            PipelineStatusExtractor.normalizePublicStatus(payload)
        }
        assertTrue(error.message?.contains("retired or unsupported fields") == true)
    }

    @Test
    fun publicPipelineStatusAcceptsOnlyExactLocalOrGlobalScopes() {
        val hash = "ab".repeat(32)
        val payload = linkedMapOf<String, Any>(
            "hash" to hash,
            "status" to mapOf("kind" to "Queued"),
            "scope" to "local",
            "resolved_from" to "queue",
        )

        assertEquals("local", PipelineStatusExtractor.normalizePublicStatus(payload)["scope"])
        payload["scope"] = "auto"
        val error = assertFailsWith<IllegalStateException> {
            PipelineStatusExtractor.normalizePublicStatus(payload)
        }
        assertTrue(error.message?.contains("unsupported scope") == true)
    }

    @Test
    fun publicPipelineStatusRejectsNonCanonicalHashRepresentations() {
        val hash = "ab".repeat(32)
        val payload = linkedMapOf<String, Any>(
            "hash" to hash,
            "status" to mapOf("kind" to "Applied", "block_height" to 7),
            "scope" to "global",
            "resolved_from" to "state",
        )
        val invalidHashes = listOf<Any>(
            hash.uppercase(),
            " $hash",
            "$hash ",
            "0x$hash",
            hash.dropLast(2),
            "aa".repeat(32),
            ByteArray(32) { 0xab.toByte() },
            List(32) { 0xab },
        )

        for (invalid in invalidHashes) {
            payload["hash"] = invalid
            assertFailsWith<IllegalStateException>(invalid.toString()) {
                PipelineStatusExtractor.normalizePublicStatus(payload)
            }
        }

        payload["hash"] = hash
        for (invalid in listOf(
            hash.uppercase(),
            " $hash",
            "$hash ",
            "0x$hash",
            hash.dropLast(2),
            "aa".repeat(32),
        )) {
            assertFailsWith<IllegalStateException>(invalid) {
                PipelineStatusExtractor.requireAuthoritativeStatus(payload, invalid)
            }
        }
    }

    @Test
    fun waitForTransactionStatusRejectsNonCanonicalRequestHashes() {
        val hash = "ab".repeat(32)
        val transport = HttpClientTransport.withExecutor(
            executor = StubResponseExecutor(statusCode = 404, body = byteArrayOf()),
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        for (invalid in listOf(
            hash.uppercase(),
            " $hash",
            "$hash ",
            "0x$hash",
            hash.dropLast(2),
            "aa".repeat(32),
        )) {
            assertFailsWith<IllegalArgumentException> {
                transport.waitForTransactionStatus(invalid, PipelineStatusOptions()).join()
            }
        }
    }

    @Test
    fun transactionStatusFailureSurfaceIsMetadataOnly() {
        val error = TransactionStatusException(
            "cd".repeat(32),
            "Rejected",
            mapOf(
                "hash" to "cd".repeat(32),
                "status" to mapOf("kind" to "Rejected"),
                "scope" to "global",
                "resolved_from" to "state",
            ),
        )

        assertFalse(error.javaClass.methods.any { it.name == "getRejectionReason" })
        assertFalse(error.message.orEmpty().contains("reason="))
    }

    @Test
    fun waitForTransactionStatusSaturatesOverflowingDeadline() {
        val hash = "fd".repeat(32)
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """{"hash":"$hash","status":{"kind":"Queued"},"scope":"global","resolved_from":"queue"}"""
                .toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            transport.waitForTransactionStatus(
                hash,
                PipelineStatusOptions(
                    intervalMillis = 0L,
                    timeoutMillis = Long.MAX_VALUE,
                    maxAttempts = 2,
                ),
            ).join()
        }

        assertTrue(error.cause is TransactionTimeoutException)
        assertEquals(2, (error.cause as TransactionTimeoutException).attempts)
        assertEquals(2, executor.requestCount)
    }

    @Test
    fun errorEnvelopeDetailsProvideRejectCode() {
        val body = """
            {
              "code": "queue_full",
              "message": "transaction queue is at capacity",
              "details": {
                "reject_code": "TX_QUEUE_FULL",
                "retry_after_seconds": 1,
                "queue": {
                  "state": "saturated",
                  "queued": 128,
                  "capacity": 128,
                  "saturated": true
                }
              }
            }
        """.trimIndent().toByteArray(StandardCharsets.UTF_8)

        assertEquals(
            "TX_QUEUE_FULL",
            HttpErrorMessageExtractor.extractRejectCode(emptyMap(), "x-iroha-reject-code", body),
        )
        assertEquals("transaction queue is at capacity", HttpErrorMessageExtractor.extractMessage(body))
    }

    @Test
    fun noritoErrorEnvelopeDetailsProvideRejectCode() {
        val body = encodeErrorEnvelope("queue_full", "transaction queue is at capacity", "TX_QUEUE_FULL")

        assertEquals(
            "TX_QUEUE_FULL",
            HttpErrorMessageExtractor.extractRejectCode(emptyMap(), "x-iroha-reject-code", body),
        )
        assertEquals("transaction queue is at capacity", HttpErrorMessageExtractor.extractMessage(body))
    }

    private fun encodeErrorEnvelope(code: String, message: String, rejectCode: String): ByteArray {
        val optionalString = NoritoAdapters.option(NoritoAdapters.stringAdapter())
        val detailsAdapter = NoritoAdapters.struct(
            listOf(
                NoritoAdapters.field("reject_code", optionalString),
                NoritoAdapters.field("queue", optionalString),
                NoritoAdapters.field("retry_after_seconds", NoritoAdapters.option(NoritoAdapters.uint(64))),
                NoritoAdapters.field("endpoint", optionalString),
                NoritoAdapters.field("axt", optionalString),
            )
        )
        val envelopeAdapter = NoritoAdapters.struct(
            listOf(
                NoritoAdapters.field("code", NoritoAdapters.stringAdapter()),
                NoritoAdapters.field("message", NoritoAdapters.stringAdapter()),
                NoritoAdapters.field("details", NoritoAdapters.option(detailsAdapter)),
            )
        )
        val details = linkedMapOf<String, Any>(
            "reject_code" to Optional.of(rejectCode),
            "queue" to Optional.empty<String>(),
            "retry_after_seconds" to Optional.empty<Long>(),
            "endpoint" to Optional.empty<String>(),
            "axt" to Optional.empty<String>(),
        )
        val envelope = linkedMapOf<String, Any>(
            "code" to code,
            "message" to message,
            "details" to Optional.of(details),
        )
        return NoritoCodec.encode(envelope as Any, "iroha_torii_shared::ErrorEnvelope", envelopeAdapter)
    }

    private fun signedClientConfig(baseUri: String): ClientConfig =
        ClientConfig.builder()
            .setBaseUri(URI.create(baseUri))
            .setLocalSigningContext(LocalSigningContext(verifyingKeyNetworkId))
            .build()

    private fun readBody(request: TransportRequest): String =
        String(request.body, StandardCharsets.UTF_8)

    private fun hexToBytes(hex: String): ByteArray {
        require(hex.length % 2 == 0) { "hex must be even length" }
        return ByteArray(hex.length / 2) { index ->
            hex.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private fun assertCanonicalSignature(
        request: TransportRequest,
        publicKey: java.security.PublicKey,
        timestampMs: Long,
        nonce: String,
    ) {
        val encodedSignature = assertNotNull(request.headers[CanonicalRequestSigner.HEADER_SIGNATURE]?.first())
        val signature = Base64.getDecoder().decode(encodedSignature)
        val message = CanonicalRequestSigner.canonicalRequestSignatureMessage(
            verifyingKeyNetworkId,
            request.method,
            request.uri,
            request.body,
            timestampMs,
            nonce,
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(publicKey)
        verifier.update(message)
        assertTrue(verifier.verify(signature))
    }

    private fun vpnProfileJson(): String =
        """
            {
              "available": true,
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "supported_exit_classes": ["standard", "low-latency", "high-security"],
              "default_exit_class": "standard",
              "lease_secs": 600,
              "dns_push_interval_secs": 60,
              "meter_family": "soranet.vpn.standard",
              "route_pushes": ["0.0.0.0/0"],
              "excluded_routes": ["10.0.0.0/8"],
              "dns_servers": ["1.1.1.1"],
              "tunnel_addresses": ["10.208.0.2/32"],
              "mtu_bytes": 1280,
              "display_billing_label": "standard XOR",
              "operator_account_id": "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT",
              "lease_fee": "1000000.25",
              "settlement_grace_secs": 120,
              "flow_label_bits": 24,
              "padding_budget_ms": 15,
              "relay_id_hex": "$validEd25519PublicKeyHex",
              "relay_mldsa65_public_key_hex": "${"55".repeat(1_952)}",
              "descriptor_commit_hex": "${"cd".repeat(32)}",
              "tls_server_name": "relay.example",
              "relay_tls_spki_sha256_hex": "${"ab".repeat(32)}",
              "relay_certificate_sha256_hex": "${"ef".repeat(32)}",
              "directory_snapshot_digest_hex": "${"42".repeat(32)}"
            }
        """.trimIndent()

    private fun vpnQuoteJson(quoteId: String, meteringKey: String): String =
        """
            {
              "quote_id": "$quoteId",
              "lease_id_hex": "$quoteId",
              "session_id_hex": "${"aa".repeat(16)}",
              "payment_reference": "$quoteId",
              "account_id": "alice",
              "exit_class": "low-latency",
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "lease_secs": 600,
              "quote_expires_at_ms": 1700000600000,
              "fee_asset_id": "xor#universal.universal",
              "escrow_account_id": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
              "operator_account_id": "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT",
              "lease_fee": "1000000.25",
              "route_pushes": ["0.0.0.0/0"],
              "excluded_routes": [],
              "dns_servers": ["1.1.1.1"],
              "tunnel_addresses": ["10.208.0.2/32"],
              "mtu_bytes": 1280,
              "meter_family": "soranet.vpn.standard",
              "flow_label_bits": 24,
              "padding_budget_ms": 15,
              "relay_id_hex": "$meteringKey",
              "relay_mldsa65_public_key_hex": "${"55".repeat(1_952)}",
              "descriptor_commit_hex": "${"cd".repeat(32)}",
              "tls_server_name": "relay.example",
              "relay_tls_spki_sha256_hex": "${"ab".repeat(32)}",
              "relay_certificate_sha256_hex": "${"ef".repeat(32)}",
              "directory_snapshot_digest_hex": "${"42".repeat(32)}",
              "metering_public_key_hex": "$meteringKey",
              "open_lease_instruction": {
                "wire_id": "iroha_data_model::isi::vpn::OpenVpnLeaseEscrow",
                "payload_hex": "cafe"
              }
            }
        """.trimIndent()

    private fun vpnHelperTicketHex(): String = "5356504e48543100" + "00".repeat(780)

    private fun vpnSessionJson(sessionId: String, quoteId: String, paymentTxHash: String): String =
        """
            {
              "session_id": "$sessionId",
              "account_id": "alice",
              "exit_class": "standard",
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "lease_secs": 600,
              "expires_at_ms": 1700000600000,
              "connected_at_ms": 1700000000000,
              "meter_family": "soranet.vpn.standard",
              "quote_id": "$quoteId",
              "payment_reference": "$quoteId",
              "payment_tx_hash": "$paymentTxHash",
              "fee_asset_id": "xor#universal.universal",
              "escrow_account_id": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
              "operator_account_id": "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT",
              "lease_fee": "1000000.25",
              "flow_label_bits": 24,
              "padding_budget_ms": 15,
              "relay_id_hex": "$validEd25519PublicKeyHex",
              "relay_mldsa65_public_key_hex": "${"55".repeat(1_952)}",
              "descriptor_commit_hex": "${"cd".repeat(32)}",
              "tls_server_name": "relay.example",
              "relay_tls_spki_sha256_hex": "${"ab".repeat(32)}",
              "relay_certificate_sha256_hex": "${"ef".repeat(32)}",
              "directory_snapshot_digest_hex": "${"42".repeat(32)}",
              "route_pushes": ["0.0.0.0/0"],
              "excluded_routes": [],
              "dns_servers": ["1.1.1.1"],
              "tunnel_addresses": ["10.208.0.2/32"],
              "mtu_bytes": 1280,
              "helper_ticket_hex": "${vpnHelperTicketHex()}",
              "bytes_in": 0,
              "bytes_out": 0,
              "status": "active"
            }
        """.trimIndent()

    private fun vpnReceiptJson(
        sessionId: String,
        quoteId: String,
        leaseId: String,
        paymentTxHash: String,
        settled: Boolean,
    ): String {
        val status = if (settled) "settled" else "disconnected"
        val source = if (settled) "relay" else "torii"
        val earned = if (settled) "750000.125" else "0"
        val refunded = if (settled) "250000.125" else "1000000.25"
        val settle = if (settled) {
            """,
              "settle_lease_instruction": {
                "wire_id": "iroha_data_model::isi::vpn::SettleVpnLease",
                "payload_hex": "f00d"
              }"""
        } else {
            """,
              "settle_lease_instruction": null"""
        }
        return """
            {
              "session_id": "$sessionId",
              "account_id": "alice",
              "exit_class": "standard",
              "relay_endpoint": "/dns/relay.example/udp/9443/quic",
              "meter_family": "soranet.vpn.standard",
              "connected_at_ms": 1700000000000,
              "disconnected_at_ms": 1700000010000,
              "duration_ms": 10000,
              "bytes_in": 1024,
              "bytes_out": 2048,
              "status": "$status",
              "receipt_source": "$source",
              "quote_id": "$quoteId",
              "payment_tx_hash": "$paymentTxHash",
              "fee_asset_id": "xor#universal.universal",
              "escrow_account_id": "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
              "operator_account_id": "sorauﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT",
              "lease_fee": "1000000.25",
              "earned_fee": "$earned",
              "refunded_fee": "$refunded",
              "lease_id_hex": "$leaseId"$settle
            }
        """.trimIndent()
    }

    private fun verifierKeyRegisterRequest(
        authority: String = testMultisigAccountId(),
        backend: String = "halo2/ipa",
        name: String = "transfer_vk",
        version: Long = 1,
        circuitId: String = "transfer-v1",
        publicInputsSchemaHashHex: String = "aa".repeat(32),
        gasScheduleId: String = "halo2-default",
        curve: String? = null,
        maxProofBytes: Long? = null,
        metadataUriCid: String? = null,
        verifyingKeyBytesCid: String? = null,
        activationHeight: Long? = null,
        withdrawHeight: Long? = null,
        commitmentHex: String? = null,
        verifyingKeyBytes: ByteArray? = byteArrayOf(1),
        verifyingKeyLength: Long? = null,
        status: String? = null,
    ): VerifyingKeyRegisterRequest =
        VerifyingKeyRegisterRequest(
            authority = authority,
            backend = backend,
            name = name,
            version = version,
            circuitId = circuitId,
            publicInputsSchemaHashHex = publicInputsSchemaHashHex,
            gasScheduleId = gasScheduleId,
            curve = curve,
            maxProofBytes = maxProofBytes,
            metadataUriCid = metadataUriCid,
            verifyingKeyBytesCid = verifyingKeyBytesCid,
            activationHeight = activationHeight,
            withdrawHeight = withdrawHeight,
            commitmentHex = commitmentHex,
            verifyingKeyBytes = verifyingKeyBytes,
            verifyingKeyLength = verifyingKeyLength,
            status = status,
        )

    private fun verifierKeyUpdateRequest(
        authority: String = testMultisigAccountId(),
        backend: String = "halo2/ipa",
        name: String = "transfer_vk",
        version: Long = 1,
        circuitId: String = "transfer-v1",
        publicInputsSchemaHashHex: String = "aa".repeat(32),
        gasScheduleId: String? = null,
        curve: String? = null,
        maxProofBytes: Long? = null,
        metadataUriCid: String? = null,
        verifyingKeyBytesCid: String? = null,
        activationHeight: Long? = null,
        withdrawHeight: Long? = null,
        commitmentHex: String? = null,
        verifyingKeyBytes: ByteArray? = byteArrayOf(1),
        verifyingKeyLength: Long? = null,
        status: String? = null,
    ): VerifyingKeyUpdateRequest =
        VerifyingKeyUpdateRequest(
            authority = authority,
            backend = backend,
            name = name,
            version = version,
            circuitId = circuitId,
            publicInputsSchemaHashHex = publicInputsSchemaHashHex,
            gasScheduleId = gasScheduleId,
            curve = curve,
            maxProofBytes = maxProofBytes,
            metadataUriCid = metadataUriCid,
            verifyingKeyBytesCid = verifyingKeyBytesCid,
            activationHeight = activationHeight,
            withdrawHeight = withdrawHeight,
            commitmentHex = commitmentHex,
            verifyingKeyBytes = verifyingKeyBytes,
            verifyingKeyLength = verifyingKeyLength,
            status = status,
        )

    private fun verifyingKeyDraftJson(transactionPayload: ByteArray): String {
        val payloadB64 = Base64.getEncoder().encodeToString(transactionPayload)
        val signingMessageB64 =
            Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionPayload))
        return """{"submitted":false,"transaction_payload_b64":"$payloadB64","signing_message_b64":"$signingMessageB64"}"""
    }

    private fun verifyingKeyTransactionPayload(
        request: Map<String, Any>,
        operation: VerifyingKeyDraftOperation,
        networkId: NetworkId = verifyingKeyNetworkId,
        authority: String = request["authority"] as String,
        instructions: List<InstructionBox>? = null,
        admissionIntent: TransactionAdmissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
    ): ByteArray {
        val discriminant = requireNotNull(AccountAddress.detectI105Discriminant(authority))
        val instructionList = instructions ?: listOf(
            VerifyingKeyDraftBinding.expectedInstruction(request, operation),
        )
        return NoritoJavaCodecAdapter(discriminant).encodeTransaction(
            TransactionPayload(
                networkId = networkId,
                authority = authority,
                creationTimeMs = 1_700_000_000_000L,
                executable = Executable.instructions(instructionList),
                timeToLiveMs = 5_000L,
                nonce = 1L,
                feePayment = testFeePayment(),
                admissionIntent = admissionIntent,
            ),
        )
    }

    private fun verifierKeyCommitment(backend: String, bytes: ByteArray): String {
        val digest = MessageDigest.getInstance("SHA-256")
        val backendBytes = backend.toByteArray(StandardCharsets.UTF_8)
        digest.update("iroha:zk:v1:vk".toByteArray(StandardCharsets.UTF_8))
        digest.update(u64Be(backendBytes.size.toLong()))
        digest.update(backendBytes)
        digest.update(u64Be(bytes.size.toLong()))
        digest.update(bytes)
        return hex(digest.digest()).lowercase()
    }

    private fun u64Be(value: Long): ByteArray {
        var remaining = value
        val out = ByteArray(8)
        for (index in 7 downTo 0) {
            out[index] = (remaining and 0xffL).toByte()
            remaining = remaining ushr 8
        }
        return out
    }

    private fun sampleTransaction(seed: Int): SignedTransaction {
        val codec = NoritoJavaCodecAdapter(org.hyperledger.iroha.sdk.address.AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val encoded = codec.encodeTransaction(
            TransactionPayload(
                networkId = TestNetworkIds.fromSeed(seed.toLong()),
                authority = testMultisigAccountId(),
                creationTimeMs = 1_700_000_000_000L + seed,
                executable = Executable.instructions(emptyList()),
                timeToLiveMs = 5_000L,
                nonce = seed.toLong() + 1L,
                feePayment = testFeePayment(),
                admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
                metadata = mapOf("note" to JsonValue.string("tx-$seed")),
            ),
        )
        val signature = ByteArray(64) { (seed + 1).toByte() }
        val publicKey = TestEd25519Keys.publicKey(seed + 2)
        return SignedTransaction(
            encoded,
            signature,
            publicKey,
            codec.schemaName(),
        )
    }

    private fun sampleOpening(): RamLfeOutputOpening =
        RamLfeOutputOpening(
            RamLfeOutputOpeningPayload(
                programId = "identifier_lookup_retail",
                inputCiphertextHash = "11".repeat(32),
                outputCiphertextHash = "22".repeat(32),
                parameterDigest = "33".repeat(32),
                evaluationKeyDigest = "44".repeat(32),
                openedOutputHash = "55".repeat(32),
                openedAtMs = 42L,
                expiresAtMs = 142L,
            ),
            signature = "aa".repeat(64),
        )

    private data class IdentifierReceiptFixture(
        val resolverPublicKey: String,
        val signatureHex: String,
    )

    private fun signedIdentifierReceiptFixture(payload: IdentifierResolutionPayload): IdentifierReceiptFixture {
        val generator = KeyPairGenerator.getInstance("Ed25519")
        val keyPair = generator.generateKeyPair()
        val rawPublicKey = rawEd25519PublicKey(keyPair)
        val payloadBytes = IdentifierReceiptCanonicalEncoder.encodePayload(payload)
        val message = IrohaHash.prehash(payloadBytes)
        val signer = Signature.getInstance("Ed25519")
        signer.initSign(keyPair.private)
        signer.update(message)
        return IdentifierReceiptFixture(
            resolverPublicKey = "ed25519:" + encodePublicKeyMultihash(0x01, rawPublicKey),
            signatureHex = hex(signer.sign()),
        )
    }

    private fun rawEd25519PublicKey(keyPair: KeyPair): ByteArray {
        val encoded = keyPair.public.encoded
        return encoded.copyOfRange(encoded.size - 32, encoded.size)
    }

    private fun sampleIdentifierResolutionPayload(
        outputCiphertextHash: String = "66".repeat(32),
    ): IdentifierResolutionPayload =
        IdentifierResolutionPayload(
            policyId = "phone#retail",
            execution = IdentifierResolutionExecutionPayload(
                programId = "identifier_lookup_retail",
                programDigest = "44".repeat(32),
                backend = "bfv-programmed-sha3-256-v1",
                verificationMode = "signed",
                inputCiphertextHash = "55".repeat(32),
                outputCiphertextHash = outputCiphertextHash,
                parameterDigest = "77".repeat(32),
                evaluationKeyDigest = "88".repeat(32),
                outputHash = "99".repeat(32),
                associatedDataHash = "aa".repeat(32),
                executedAtMs = 42L,
                expiresAtMs = 142L,
            ),
            opening = sampleOpening(),
            opaqueId = "opaque:" + "11".repeat(32),
            receiptHash = "22".repeat(32),
            uaid = "uaid:" + "33".repeat(31) + "35",
            accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
        )

    private fun sampleIdentifierVerifierPolicy(
        resolverPublicKey: String,
        policyId: String = "phone#retail",
    ): IdentifierPolicySummary =
        IdentifierPolicySummary(
            policyId = policyId,
            owner = "owner",
            active = true,
            normalization = IdentifierNormalization.PHONE_E164,
            resolverPublicKey = resolverPublicKey,
            backend = "bfv-programmed-sha3-256-v1",
            inputEncryption = "bfv-v1",
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = null,
            note = null,
        )

    private fun hex(bytes: ByteArray): String =
        bytes.joinToString(separator = "") { "%02X".format(it.toInt() and 0xFF) }

    private fun signedOnboardingReceipt(
        body: AccountOnboardingPlanBodyV1,
        privateKey: Ed25519PrivateKeyParameters,
    ): AccountOnboardingPlanReceiptV1 {
        val hash = AccountOnboardingReceiptVerifier.canonicalHash(body)
        val signer = Ed25519Signer()
        signer.init(true, privateKey)
        signer.update(hash, 0, hash.size)
        return AccountOnboardingPlanReceiptV1(body, hex(hash), hex(signer.generateSignature()))
    }

    private data class AtomicOnboardingProofFixture(
        val request: AccountOnboardingPlanRequestV1,
        val receipt: AccountOnboardingPlanReceiptV1,
        val binding: TairaPublicResetMutationBindingV1,
        val proofRequired: AccountOnboardingProofRequiredPrepareResponseV1,
        val authority: String,
        val accountId: String,
        val alias: String,
    )

    private fun atomicOnboardingProofFixture(): AtomicOnboardingProofFixture {
        val privateKey = Ed25519PrivateKeyParameters(ByteArray(32) { 0x53.toByte() }, 0)
        val authority = AccountAddress.fromAccount(privateKey.generatePublicKey().encoded, "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val accountId = testAccountId(0x48)
        val alias = "merchant@banka.paynet"
        val resolvedAlias = ResolvedAccountAliasV1(AccountAliasName.parse(alias), 7L)
        val request = AccountOnboardingPlanRequestV1(alias, accountId)
        val intent = AliasIntentV1.AccountAlias(
            AliasAccountIntentV1(
                resolvedAlias,
                accountId,
                AccountProvisionV1.CREATE,
                AccountAliasRoleV1.PRIMARY,
            ),
        )
        val assetBytes = ByteArray(16) { it.toByte() }.also {
            it[6] = 0x46
            it[8] = 0x88.toByte()
        }
        val guard = AliasQuoteGuardV1(
            3,
            AssetDefinitionIdEncoder.encodeFromBytes(assetBytes),
            "5",
            4_102_444_800_000L,
        )
        val body = AccountOnboardingPlanBodyV1(
            1,
            request,
            authority,
            verifyingKeyNetworkId,
            AliasPlanAnchorV1(9, verifyingKeyNetworkId.literal),
            AliasPlanResourceV1(intent, AliasPlanDispositionV1.NO_OP, null, null),
            AliasLeaseAcquisitionV1(1),
            guard,
            emptyList(),
            null,
            guard.validUntilMs,
        )
        val receipt = signedOnboardingReceipt(body, privateKey)
        val binding = TairaPublicResetMutationBindingV1(
            authorizationSha256 = "11".repeat(32),
            authorizationNonce = "onboarding-fixture-nonce-0000001",
            kind = TairaPublicResetMutationBindingV1.ONBOARDING,
            phase = "onboarding",
            idempotencyKey = "22".repeat(32),
            executionExpiresAtUnixMs = 4_102_444_800_000L,
        )
        val unsigned = AccountOnboardingProofRequiredPrepareResponseV1(
            binding,
            receipt.planHash.lowercase(),
            accountId,
            alias,
            AliasPlanDispositionV1.NO_OP,
            "00",
        )
        val digest = PreparedTransactionSignatureV1.digest(
            PreparedTransactionSignatureV1.onboardingProofRequired(unsigned),
        )
        val signer = Ed25519Signer()
        signer.init(true, privateKey)
        signer.update(digest, 0, digest.size)
        val proofRequired = AccountOnboardingProofRequiredPrepareResponseV1(
            binding,
            receipt.planHash.lowercase(),
            accountId,
            alias,
            AliasPlanDispositionV1.NO_OP,
            hex(signer.generateSignature()),
        )
        return AtomicOnboardingProofFixture(
            request,
            receipt,
            binding,
            proofRequired,
            authority,
            accountId,
            alias,
        )
    }

    private fun sha256Hex(bytes: ByteArray): String =
        hex(MessageDigest.getInstance("SHA-256").digest(bytes))

    private fun assertBfvOperationKeyComponentVectors(operationVectors: Map<String, Any?>) {
        assertEquals("soracloud-bfv-operation-v1", operationVectors["vector_set"])
        val publicParameters = obj(operationVectors, "public_parameters")
        val publicDegree = long(publicParameters, "polynomial_degree")
        assertBfvRnsModulusChainFixture(operationVectors, publicDegree)
        val evaluationKey = obj(operationVectors, "evaluation_key_bundle")
        assertEquals(long(publicParameters, "decomposition_base_log"), long(evaluationKey, "decomposition_base_log"))
        assertEquals(long(evaluationKey, "relinearization_entry_count"), long(evaluationKey, "decomposition_digit_count"))
        val relinearizationEntries = listOfMaps(evaluationKey, "relinearization_entries")
        assertEquals(long(evaluationKey, "relinearization_entry_count").toInt(), relinearizationEntries.size)
        val componentDigests = mutableSetOf<String>()
        for ((index, entry) in relinearizationEntries.withIndex()) {
            assertEquals(index.toLong(), long(entry, "index"), "relinearization entry index")
            assertEquals(publicDegree, long(entry, "coefficient_count"), "relinearization entry coefficient count")
            assertBfvComponentDigest("relinearization entry $index b", string(entry, "b_sha256"), componentDigests)
            assertBfvComponentDigest("relinearization entry $index a", string(entry, "a_sha256"), componentDigests)
        }
        val galoisKeys = listOfMaps(operationVectors, "galois_keys")
        assertEquals(long(evaluationKey, "galois_key_count").toInt(), galoisKeys.size)
        for (key in galoisKeys) {
            val power = long(key, "automorphism_power")
            val entries = listOfMaps(key, "entries")
            assertEquals(long(key, "entry_count").toInt(), entries.size)
            for ((index, entry) in entries.withIndex()) {
                assertEquals(index.toLong(), long(entry, "index"), "Galois key $power entry index")
                assertEquals(publicDegree, long(entry, "coefficient_count"), "Galois key $power entry coefficient count")
                assertBfvComponentDigest("Galois key $power entry $index b", string(entry, "b_sha256"), componentDigests)
                assertBfvComponentDigest("Galois key $power entry $index a", string(entry, "a_sha256"), componentDigests)
            }
        }
        val galoisSwitchVectors = listOfMaps(operationVectors, "galois_switch_vectors")
        assert(galoisSwitchVectors.isNotEmpty()) { "Galois switch vectors must not be empty" }
        for (vector in galoisSwitchVectors) {
            val name = string(vector, "name")
            val power = long(vector, "automorphism_power")
            assert(galoisKeys.any { long(it, "automorphism_power") == power }) { "Galois switch vector $name has no matching key" }
            val plaintextSlots = longList(vector, "input_plaintext_slots")
            assert(plaintextSlots.isNotEmpty()) { "Galois switch vector $name plaintext slots must not be empty" }
            assert(plaintextSlots.all { it >= 0 }) { "Galois switch vector $name plaintext slots must be non-negative" }
            assert(long(vector, "expected_input_ciphertext_bytes") > 0) { "Galois switch vector $name input bytes must be positive" }
            assert(long(vector, "expected_output_ciphertext_bytes") > 0) { "Galois switch vector $name output bytes must be positive" }
            assertBfvUpperSha256("Galois switch vector $name input", string(vector, "expected_input_ciphertext_sha256"))
            assertBfvUpperSha256("Galois switch vector $name output", string(vector, "expected_output_ciphertext_sha256"))
            assertBfvUpperSha256("Galois switch vector $name plaintext", string(vector, "expected_plaintext_sha256"))
            val components = obj(vector, "output_components")
            assertEquals(publicDegree, long(components, "coefficient_count"), "Galois switch vector $name coefficient count")
            assertBfvComponentDigest("Galois switch vector $name c0", string(components, "c0_sha256"), componentDigests)
            assertBfvComponentDigest("Galois switch vector $name c1", string(components, "c1_sha256"), componentDigests)
        }
        val packedGaloisSwitchVectors = listOfMaps(operationVectors, "packed_galois_switch_vectors")
        assert(packedGaloisSwitchVectors.isNotEmpty()) { "packed Galois switch vectors must not be empty" }
        for (vector in packedGaloisSwitchVectors) {
            val name = string(vector, "name")
            val power = long(vector, "automorphism_power")
            assert(galoisKeys.any { long(it, "automorphism_power") == power }) { "packed Galois switch vector $name has no matching key" }
            val inputSlots = longList(vector, "input_packed_slots")
            val permutation = longList(vector, "expected_slot_permutation")
            val outputSlots = longList(vector, "expected_packed_slots")
            assertEquals(publicDegree.toInt(), inputSlots.size, "packed Galois switch vector $name input slot count")
            assertEquals(publicDegree.toInt(), permutation.size, "packed Galois switch vector $name permutation count")
            assertEquals(publicDegree.toInt(), outputSlots.size, "packed Galois switch vector $name output slot count")
            assert(inputSlots.all { it >= 0 }) { "packed Galois switch vector $name input slots must be non-negative" }
            assert(permutation.all { it >= 0 }) { "packed Galois switch vector $name permutation slots must be non-negative" }
            assert(outputSlots.all { it >= 0 }) { "packed Galois switch vector $name output slots must be non-negative" }
            assertBfvUpperSha256("packed Galois switch vector $name packed plaintext", string(vector, "expected_packed_plaintext_sha256"))
            assertBfvUpperSha256("packed Galois switch vector $name input", string(vector, "expected_input_ciphertext_sha256"))
            assertBfvUpperSha256("packed Galois switch vector $name output", string(vector, "expected_output_ciphertext_sha256"))
            assertBfvUpperSha256("packed Galois switch vector $name plaintext", string(vector, "expected_plaintext_coefficients_sha256"))
            val components = obj(vector, "output_components")
            assertEquals(publicDegree, long(components, "coefficient_count"), "packed Galois switch vector $name coefficient count")
            assertBfvComponentDigest("packed Galois switch vector $name c0", string(components, "c0_sha256"), componentDigests)
            assertBfvComponentDigest("packed Galois switch vector $name c1", string(components, "c1_sha256"), componentDigests)
        }
        val rotationKeys = listOfMaps(operationVectors, "rotation_keys")
        assertEquals(long(evaluationKey, "rotation_key_count").toInt(), rotationKeys.size)
        for (key in rotationKeys) {
            val components = obj(key, "zero_refresh_components")
            val steps = long(key, "rotation_steps")
            assertEquals(publicDegree, long(components, "coefficient_count"), "rotation key $steps coefficient count")
            assertBfvComponentDigest("rotation key $steps c0", string(components, "c0_sha256"), componentDigests)
            assertBfvComponentDigest("rotation key $steps c1", string(components, "c1_sha256"), componentDigests)
        }
        val bootstrap = obj(operationVectors, "bootstrap_key")
        assertEquals(string(evaluationKey, "bootstrap_key_id"), string(bootstrap, "key_id"))
        assertEquals(long(evaluationKey, "bootstrap_max_refresh_rounds"), long(bootstrap, "max_refresh_rounds"), "bootstrap max refresh rounds")
        assert(long(bootstrap, "max_refresh_rounds") > 0) { "bootstrap max refresh rounds must be positive" }
        val bootstrapComponents = obj(bootstrap, "zero_refresh_components")
        assertEquals(publicDegree, long(bootstrapComponents, "coefficient_count"), "bootstrap coefficient count")
        assertBfvComponentDigest("bootstrap c0", string(bootstrapComponents, "c0_sha256"), componentDigests)
        assertBfvComponentDigest("bootstrap c1", string(bootstrapComponents, "c1_sha256"), componentDigests)
        val roundRefreshes = listOfMaps(bootstrap, "round_refreshes")
        assertEquals(long(bootstrap, "max_refresh_rounds").toInt(), roundRefreshes.size, "bootstrap round refresh count")
        for ((index, refresh) in roundRefreshes.withIndex()) {
            assertEquals(index.toLong(), long(refresh, "round_index"), "bootstrap round $index index")
            assert(long(refresh, "expected_refresh_bytes") > 0) { "bootstrap round $index bytes must be positive" }
            assertBfvUpperSha256("bootstrap round $index refresh", string(refresh, "expected_refresh_sha256"))
            val components = obj(refresh, "components")
            assertEquals(publicDegree, long(components, "coefficient_count"), "bootstrap round $index coefficient count")
            if (index == 0) {
                assertEquals(string(bootstrapComponents, "c0_sha256"), string(components, "c0_sha256"), "bootstrap round 0 c0 mirror")
                assertEquals(string(bootstrapComponents, "c1_sha256"), string(components, "c1_sha256"), "bootstrap round 0 c1 mirror")
                assertBfvUpperSha256("bootstrap round 0 c0", string(components, "c0_sha256"))
                assertBfvUpperSha256("bootstrap round 0 c1", string(components, "c1_sha256"))
            } else {
                assertBfvComponentDigest("bootstrap round $index c0", string(components, "c0_sha256"), componentDigests)
                assertBfvComponentDigest("bootstrap round $index c1", string(components, "c1_sha256"), componentDigests)
            }
        }
        assertEquals(string(bootstrap, "expected_zero_refresh_sha256"), string(roundRefreshes[0], "expected_refresh_sha256"), "bootstrap first round mirrors zero refresh")
        if (roundRefreshes.size > 1) {
            assert(string(roundRefreshes[0], "expected_refresh_sha256") != string(roundRefreshes[1], "expected_refresh_sha256")) {
                "bootstrap round refresh material must be domain separated"
            }
        }
        assertBfvFullBootstrapMaterialFixture(operationVectors)
        val bootstrapRefreshVectors = listOfMaps(operationVectors, "bootstrap_refresh_vectors")
        assert(bootstrapRefreshVectors.isNotEmpty()) { "bootstrap refresh vectors must not be empty" }
        for (vector in bootstrapRefreshVectors) {
            val name = string(vector, "name")
            assertEquals(string(bootstrap, "key_id"), string(vector, "key_id"), "bootstrap refresh vector $name key id")
            val refreshRounds = long(vector, "refresh_rounds")
            assert(refreshRounds > 0) { "bootstrap refresh vector $name rounds must be positive" }
            assert(refreshRounds <= long(bootstrap, "max_refresh_rounds")) { "bootstrap refresh vector $name exceeds key rounds" }
            val plaintextSlots = longList(vector, "input_plaintext_slots")
            assert(plaintextSlots.isNotEmpty()) { "bootstrap refresh vector $name plaintext slots must not be empty" }
            assert(plaintextSlots.all { it >= 0 }) { "bootstrap refresh vector $name plaintext slots must be non-negative" }
            assert(long(vector, "expected_input_ciphertext_bytes") > 0) { "bootstrap refresh vector $name input bytes must be positive" }
            assert(long(vector, "expected_output_ciphertext_bytes") > 0) { "bootstrap refresh vector $name output bytes must be positive" }
            assertBfvUpperSha256("bootstrap refresh vector $name input", string(vector, "expected_input_ciphertext_sha256"))
            assertBfvUpperSha256("bootstrap refresh vector $name output", string(vector, "expected_output_ciphertext_sha256"))
            assertBfvUpperSha256("bootstrap refresh vector $name plaintext", string(vector, "expected_plaintext_sha256"))
            val components = obj(vector, "output_components")
            assertEquals(publicDegree, long(components, "coefficient_count"), "bootstrap refresh vector $name coefficient count")
            assertBfvComponentDigest("bootstrap refresh vector $name c0", string(components, "c0_sha256"), componentDigests)
            assertBfvComponentDigest("bootstrap refresh vector $name c1", string(components, "c1_sha256"), componentDigests)
        }
        val runtimeVectors = listOfMaps(operationVectors, "vectors")
        for (vector in runtimeVectors) {
            val expectedDepth = if (string(vector, "operation") == "Multiply") {
                balancedBfvMultiplicationDepth(listOfMaps(vector, "inputs").size)
            } else {
                0
            }
            assertEquals(expectedDepth, long(vector, "requested_multiplication_depth").toInt(), "${string(vector, "name")} requested multiplication depth")
        }
        val packedRotate = runtimeVectors.firstOrNull { string(it, "name") == "soracloud-packed-rotate-left-output" }
            ?: error("packed RotateLeft operation vector must be present")
        assertEquals("RotateLeft", string(packedRotate, "operation"), "packed RotateLeft operation")
        assertEquals(publicDegree / 2, long(packedRotate, "rotation_steps"), "packed RotateLeft rotation steps")
        val packedRotatePower = long(packedRotate, "automorphism_power")
        assertEquals(publicDegree + 1, packedRotatePower, "packed RotateLeft Galois power")
        assert(galoisKeys.any { long(it, "automorphism_power") == packedRotatePower }) { "packed RotateLeft vector has no matching Galois key" }
        val packedRotateInputs = listOfMaps(packedRotate, "inputs")
        assertEquals(1, packedRotateInputs.size, "packed RotateLeft input count")
        val packedRotateInput = packedRotateInputs[0]
        val inputSlots = longList(packedRotateInput, "packed_slots")
        val outputSlots = longList(packedRotate, "expected_packed_slots")
        assertEquals(publicDegree.toInt(), inputSlots.size, "packed RotateLeft input slot count")
        assertEquals(publicDegree.toInt(), outputSlots.size, "packed RotateLeft output slot count")
        assert(inputSlots.all { it >= 0 }) { "packed RotateLeft input slots must be non-negative" }
        assert(outputSlots.all { it >= 0 }) { "packed RotateLeft output slots must be non-negative" }
        assert(long(packedRotateInput, "expected_ciphertext_bytes") > 0) { "packed RotateLeft input bytes must be positive" }
        assert(long(packedRotate, "expected_output_ciphertext_bytes") > 0) { "packed RotateLeft output bytes must be positive" }
        assertBfvUpperSha256("packed RotateLeft input plaintext", string(packedRotateInput, "expected_packed_plaintext_sha256"))
        assertBfvUpperSha256("packed RotateLeft input", string(packedRotateInput, "expected_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft output", string(packedRotate, "expected_output_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft plaintext", string(packedRotate, "expected_plaintext_coefficients_sha256"))
        val packedRotateComponents = obj(packedRotate, "output_components")
        assertEquals(publicDegree, long(packedRotateComponents, "coefficient_count"), "packed RotateLeft coefficient count")
        assertBfvComponentDigest("packed RotateLeft c0", string(packedRotateComponents, "c0_sha256"), componentDigests)
        assertBfvComponentDigest("packed RotateLeft c1", string(packedRotateComponents, "c1_sha256"), componentDigests)

        val packedRotateSchedule = runtimeVectors.firstOrNull { string(it, "name") == "soracloud-packed-rotate-left-schedule-output" }
            ?: error("packed RotateLeft schedule vector must be present")
        assertEquals("RotateLeft", string(packedRotateSchedule, "operation"), "packed RotateLeft schedule operation")
        assertEquals(1, long(packedRotateSchedule, "rotation_steps").toInt(), "packed RotateLeft schedule rotation steps")
        val schedulePowers = longList(packedRotateSchedule, "automorphism_powers")
        assert(schedulePowers.size > 1) { "packed RotateLeft schedule must use multiple powers" }
        for (power in schedulePowers) {
            assert(power > 0) { "packed RotateLeft schedule power must be positive" }
            assert(galoisKeys.any { long(it, "automorphism_power") == power }) { "packed RotateLeft schedule power $power has no matching Galois key" }
        }
        val packedRotateScheduleInputs = listOfMaps(packedRotateSchedule, "inputs")
        assertEquals(1, packedRotateScheduleInputs.size, "packed RotateLeft schedule input count")
        val packedRotateScheduleInput = packedRotateScheduleInputs[0]
        val scheduleInputSlots = longList(packedRotateScheduleInput, "packed_slots")
        val scheduleOutputSlots = longList(packedRotateSchedule, "expected_packed_slots")
        assertEquals(publicDegree.toInt(), scheduleInputSlots.size, "packed RotateLeft schedule input slot count")
        assertEquals(publicDegree.toInt(), scheduleOutputSlots.size, "packed RotateLeft schedule output slot count")
        assertEquals(scheduleInputSlots.drop(1) + scheduleInputSlots.first(), scheduleOutputSlots, "packed RotateLeft schedule output slots")
        assert(scheduleInputSlots.all { it >= 0 }) { "packed RotateLeft schedule input slots must be non-negative" }
        assert(scheduleOutputSlots.all { it >= 0 }) { "packed RotateLeft schedule output slots must be non-negative" }
        assert(long(packedRotateScheduleInput, "expected_ciphertext_bytes") > 0) { "packed RotateLeft schedule input bytes must be positive" }
        assert(long(packedRotateSchedule, "expected_output_ciphertext_bytes") > 0) { "packed RotateLeft schedule output bytes must be positive" }
        assertBfvUpperSha256("packed RotateLeft schedule input plaintext", string(packedRotateScheduleInput, "expected_packed_plaintext_sha256"))
        assertBfvUpperSha256("packed RotateLeft schedule input", string(packedRotateScheduleInput, "expected_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft schedule output", string(packedRotateSchedule, "expected_output_ciphertext_sha256"))
        assertBfvUpperSha256("packed RotateLeft schedule plaintext", string(packedRotateSchedule, "expected_plaintext_coefficients_sha256"))
        val packedRotateScheduleComponents = obj(packedRotateSchedule, "output_components")
        assertEquals(publicDegree, long(packedRotateScheduleComponents, "coefficient_count"), "packed RotateLeft schedule coefficient count")
        assertBfvComponentDigest("packed RotateLeft schedule c0", string(packedRotateScheduleComponents, "c0_sha256"), componentDigests)
        assertBfvComponentDigest("packed RotateLeft schedule c1", string(packedRotateScheduleComponents, "c1_sha256"), componentDigests)
    }

    private fun assertBfvFullBootstrapMaterialFixture(operationVectors: Map<String, Any?>) {
        val material = obj(operationVectors, "full_bootstrap_material")
        assertEquals("iroha_bfv_full_bootstrap_v1", string(material, "circuit_id"), "full-bootstrap circuit id")
        assertEquals(1L, long(material, "max_bootstrap_depth"), "full-bootstrap max depth")

        val digestFields = listOf(
            "parameter_digest_hex",
            "rns_modulus_chain_digest_hex",
            "key_switch_decomposition_chain_digest_hex",
            "coefficient_to_slot_key_digest_hex",
            "slot_to_coefficient_key_digest_hex",
            "blind_rotation_key_digest_hex",
            "sample_extraction_key_digest_hex",
            "accumulator_digest_hex",
            "proof_public_input_schema_digest_hex",
            "prover_key_digest_hex",
            "prover_key_material_commitment_hex",
            "verifier_key_digest_hex",
            "verifier_key_material_commitment_hex",
            "vk_commitment_hex",
            "expected_material_digest_hex",
        )
        val digestValues = digestFields.map { field ->
            val value = string(material, field)
            assertBfvLowerDigest("full-bootstrap material $field", value)
            value
        }
        assertEquals(
            string(obj(operationVectors, "rns_modulus_chain"), "expected_digest_hex"),
            string(material, "rns_modulus_chain_digest_hex"),
            "full-bootstrap RNS digest",
        )
        assertEquals(
            string(material, "verifier_key_material_commitment_hex"),
            string(material, "vk_commitment_hex"),
            "full-bootstrap verifier-key commitment",
        )
        val uniqueDigestValues = digestFields.zip(digestValues)
            .filter { (field, _) -> field != "vk_commitment_hex" }
            .map { (_, value) -> value }
        assertEquals(
            uniqueDigestValues.size,
            uniqueDigestValues.toSet().size,
            "full-bootstrap material digest roles must be unique",
        )
    }

    private fun assertBfvRnsModulusChainFixture(operationVectors: Map<String, Any?>, publicDegree: Long) {
        val rns = obj(operationVectors, "rns_modulus_chain")
        val moduli = longList(rns, "moduli")
        assert(moduli.isNotEmpty()) { "RNS modulus-chain limbs must not be empty" }
        assertEquals(moduli.sorted(), moduli, "RNS modulus-chain limbs must be sorted")
        assert(moduli.all { it > 2 && it % 2L == 1L }) { "RNS modulus-chain limbs must be odd prime candidates" }
        assert(string(rns, "product").all { it.isDigit() }) { "RNS modulus-chain product must be decimal" }
        assertBfvLowerDigest("RNS modulus-chain digest", string(rns, "expected_digest_hex"))

        val samples = obj(rns, "sample_polynomials")
        assertEquals(publicDegree.toInt(), longList(samples, "lhs_coefficients").size, "RNS lhs coefficient count")
        assertEquals(publicDegree.toInt(), longList(samples, "rhs_coefficients").size, "RNS rhs coefficient count")
        for (label in listOf("lhs", "rhs", "sum", "negacyclic_product")) {
            assertBfvRnsPolynomialFixture(label, obj(samples, label), publicDegree, moduli.size)
        }
    }

    private fun assertBfvRnsPolynomialFixture(
        label: String,
        polynomial: Map<String, Any?>,
        publicDegree: Long,
        limbCount: Int,
    ) {
        assertEquals(publicDegree, long(polynomial, "coefficient_count"), "$label RNS coefficient count")
        val limbHashes = stringList(polynomial, "residue_limb_sha256")
        assertEquals(limbCount, limbHashes.size, "$label RNS residue limb count")
        assertBfvUpperSha256("$label RNS reconstructed coefficients", string(polynomial, "reconstructed_sha256"))
        for ((index, digest) in limbHashes.withIndex()) {
            assertBfvUpperSha256("$label RNS residue limb $index", digest)
        }
    }

    private fun assertBfvComponentDigest(label: String, value: String, seen: MutableSet<String>) {
        assertBfvUpperSha256(label, value)
        assertTrue(seen.add(value), "$label must be unique")
    }

    private fun assertBfvUpperSha256(label: String, value: String) {
        assertTrue(Regex("[0-9A-F]{64}").matches(value), "$label must be canonical uppercase SHA-256")
        assertFalse(value == "0".repeat(64), "$label must not be zero")
    }

    private fun balancedBfvMultiplicationDepth(inputCount: Int): Int {
        assertTrue(inputCount > 0, "BFV multiplication depth requires at least one input")
        var covered = 1
        var depth = 0
        while (covered < inputCount) {
            covered *= 2
            depth += 1
        }
        return depth
    }

    private fun assertBfvLowerDigest(label: String, value: String) {
        assertTrue(Regex("[0-9a-f]{64}").matches(value), "$label must be canonical lowercase hex")
        assertFalse(value == "0".repeat(64), "$label must not be zero")
    }

    private fun loadSharedFixture(relativePath: String): Map<String, Any?> {
        var cursor = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate = cursor.resolve(relativePath)
            if (Files.exists(candidate)) {
                @Suppress("UNCHECKED_CAST")
                return JsonParser.parse(String(Files.readAllBytes(candidate), StandardCharsets.UTF_8)) as Map<String, Any?>
            }
            cursor = cursor.parent ?: break
        }
        error("$relativePath was not found")
    }

    private fun loadSharedBfvFixture(): Map<String, Any?> =
        loadSharedFixture("fixtures/soracloud/bfv_identifier_vectors_v1.json")

    private fun loadSharedReceiptFixture(): Map<String, Any?> =
        loadSharedFixture("fixtures/soracloud/identifier_receipt_vectors_v1.json")

    private fun identifierPolicyFromReceiptFixture(
        policy: Map<String, Any?>,
        policyIdOverride: String? = null,
        resolverPublicKeyOverride: String? = null,
    ): IdentifierPolicySummary =
        IdentifierPolicySummary(
            policyId = policyIdOverride ?: string(policy, "policy_id"),
            owner = string(policy, "owner"),
            active = policy["active"] == true,
            normalization = IdentifierNormalization.PHONE_E164,
            resolverPublicKey = resolverPublicKeyOverride ?: string(policy, "resolver_public_key"),
            backend = string(policy, "backend"),
            inputEncryption = policy["input_encryption"] as? String,
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = null,
            note = null,
        )

    private fun identifierReceiptFromFixture(
        receipt: Map<String, Any?>,
        outputCiphertextHashOverride: String? = null,
        signatureOverride: String? = null,
        attestationOverride: Map<String, Any?>? = null,
        policyIdOverride: String? = null,
        executionProgramIdOverride: String? = null,
        openingProgramIdOverride: String? = null,
        accountIdOverride: String? = null,
    ): IdentifierResolutionReceipt =
        IdentifierResolutionReceipt(
            identifierPayloadFromFixture(
                obj(receipt, "payload"),
                outputCiphertextHashOverride,
                policyIdOverride,
                executionProgramIdOverride,
                openingProgramIdOverride,
                accountIdOverride,
            ),
            identifierAttestationFromFixture(
                attestationOverride ?: obj(receipt, "attestation"),
                signatureOverride,
            ),
        )

    private fun identifierPayloadFromFixture(
        payload: Map<String, Any?>,
        outputCiphertextHashOverride: String? = null,
        policyIdOverride: String? = null,
        executionProgramIdOverride: String? = null,
        openingProgramIdOverride: String? = null,
        accountIdOverride: String? = null,
    ): IdentifierResolutionPayload =
        IdentifierResolutionPayload(
            policyId = policyIdOverride ?: string(payload, "policy_id"),
            execution = identifierExecutionFromFixture(
                obj(payload, "execution"),
                outputCiphertextHashOverride,
                executionProgramIdOverride,
            ),
            opening = outputOpeningFromFixture(obj(payload, "opening"), openingProgramIdOverride),
            opaqueId = string(payload, "opaque_id"),
            receiptHash = string(payload, "receipt_hash"),
            uaid = string(payload, "uaid"),
            accountId = accountIdOverride ?: string(payload, "account_id"),
        )

    private fun identifierExecutionFromFixture(
        execution: Map<String, Any?>,
        outputCiphertextHashOverride: String? = null,
        programIdOverride: String? = null,
    ): IdentifierResolutionExecutionPayload =
        IdentifierResolutionExecutionPayload(
            programId = programIdOverride ?: string(execution, "program_id"),
            programDigest = string(execution, "program_digest"),
            backend = string(execution, "backend"),
            verificationMode = string(execution, "verification_mode"),
            inputCiphertextHash = string(execution, "input_ciphertext_hash"),
            outputCiphertextHash = outputCiphertextHashOverride ?: string(execution, "output_ciphertext_hash"),
            parameterDigest = string(execution, "parameter_digest"),
            evaluationKeyDigest = string(execution, "evaluation_key_digest"),
            outputHash = string(execution, "output_hash"),
            associatedDataHash = string(execution, "associated_data_hash"),
            executedAtMs = long(execution, "executed_at_ms"),
            expiresAtMs = optionalLong(execution, "expires_at_ms"),
        )

    private fun outputOpeningFromFixture(
        opening: Map<String, Any?>,
        programIdOverride: String? = null,
    ): RamLfeOutputOpening {
        val payload = obj(opening, "payload")
        return RamLfeOutputOpening(
            RamLfeOutputOpeningPayload(
                programId = programIdOverride ?: string(payload, "program_id"),
                inputCiphertextHash = string(payload, "input_ciphertext_hash"),
                outputCiphertextHash = string(payload, "output_ciphertext_hash"),
                parameterDigest = string(payload, "parameter_digest"),
                evaluationKeyDigest = string(payload, "evaluation_key_digest"),
                openedOutputHash = string(payload, "opened_output_hash"),
                openedAtMs = long(payload, "opened_at_ms"),
                expiresAtMs = optionalLong(payload, "expires_at_ms"),
            ),
            signature = string(opening, "signature"),
        )
    }

    private fun identifierAttestationFromFixture(
        attestation: Map<String, Any?>,
        signatureOverride: String? = null,
    ): IdentifierReceiptAttestation =
        IdentifierReceiptAttestation(
            kind = string(attestation, "kind"),
            signature = signatureOverride ?: attestation["signature"] as? String,
            proofBackend = attestation["proof_backend"] as? String,
            proofB64 = attestation["proof_b64"] as? String,
        )

    private fun bfvPolicyFromFixture(policy: Map<String, Any?>): IdentifierPolicySummary =
        IdentifierPolicySummary(
            policyId = string(policy, "policy_id"),
            owner = string(policy, "owner"),
            active = policy["active"] == true,
            normalization = IdentifierNormalization.EXACT,
            resolverPublicKey = string(policy, "resolver_public_key"),
            backend = string(policy, "backend"),
            inputEncryption = string(policy, "input_encryption"),
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = bfvParametersFromFixture(
                obj(policy, "input_encryption_public_parameters_decoded"),
            ),
            note = null,
        )

    private fun bfvParametersFromFixture(params: Map<String, Any?>): IdentifierBfvPublicParameters {
        val parameters = obj(params, "parameters")
        val publicKey = obj(params, "public_key")
        return IdentifierBfvPublicParameters(
            IdentifierBfvPublicParameters.Parameters(
                long(parameters, "polynomial_degree"),
                long(parameters, "plaintext_modulus"),
                long(parameters, "ciphertext_modulus"),
                long(parameters, "decomposition_base_log").toInt(),
            ),
            IdentifierBfvPublicParameters.PublicKey(
                longList(publicKey, "b"),
                longList(publicKey, "a"),
            ),
            long(params, "max_input_bytes").toInt(),
            params["norito_length_encoding"] as? String,
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun obj(root: Map<String, Any?>, key: String): Map<String, Any?> =
        root[key] as? Map<String, Any?>
            ?: error("$key must be an object")
    @Suppress("UNCHECKED_CAST")
    private fun listOfMaps(root: Map<String, Any?>, key: String): List<Map<String, Any?>> =
        root[key] as? List<Map<String, Any?>>
            ?: error("$key must be a list of objects")
    @Suppress("UNCHECKED_CAST")
    private fun mutableObj(root: Map<String, Any?>, key: String): MutableMap<String, Any?> =
        root[key] as? MutableMap<String, Any?>
            ?: error("$key must be a mutable object")
    @Suppress("UNCHECKED_CAST")
    private fun mutableListOfMaps(
        root: Map<String, Any?>,
        key: String,
    ): MutableList<MutableMap<String, Any?>> =
        root[key] as? MutableList<MutableMap<String, Any?>>
            ?: error("$key must be a mutable list of objects")
    private fun string(root: Map<String, Any?>, key: String): String =
        root[key] as? String ?: error("$key must be a string")
    private fun stringList(root: Map<String, Any?>, key: String): List<String> =
        (root[key] as? List<*> ?: error("$key must be a list")).mapIndexed { index, value ->
            value as? String ?: error("$key[$index] must be a string")
        }
    private fun long(root: Map<String, Any?>, key: String): Long =
        when (val value = root[key]) {
            is Number -> value.toLong()
            is String -> value.toLongOrNull() ?: error("$key must be an integer string")
            else -> error("$key must be a number")
        }
    private fun optionalLong(root: Map<String, Any?>, key: String): Long? =
        (root[key] as? Number)?.toLong()
    private fun longList(root: Map<String, Any?>, key: String): List<Long> =
        (root[key] as? List<*> ?: error("$key must be a list")).mapIndexed { index, value ->
            when (value) {
                is Number -> value.toLong()
                is String -> value.toLongOrNull() ?: error("$key[$index] must be an integer string")
                else -> error("$key[$index] must be a number")
            }
        }
    private fun sampleBfvPolicy(parameters: IdentifierBfvPublicParameters?): IdentifierPolicySummary =
        IdentifierPolicySummary(
            policyId = "string#retail",
            owner = "owner",
            active = true,
            normalization = IdentifierNormalization.EXACT,
            resolverPublicKey = "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            backend = "bfv-affine-sha3-256-v1",
            inputEncryption = "bfv-v1",
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = parameters,
            note = null,
        )
    private fun samplePlaintextOnlyPolicy(): IdentifierPolicySummary =
        IdentifierPolicySummary(
            policyId = "string#retail",
            owner = "owner",
            active = true,
            normalization = IdentifierNormalization.EXACT,
            resolverPublicKey = "ed25519:ed01203B6A27BCCEB6A42D62A3A8D02A6F0D73653215771DE243A63AC048A18B59DA29",
            backend = "hkdf-sha3-512-prf-v1",
            inputEncryption = null,
            inputEncryptionPublicParameters = null,
            inputEncryptionPublicParametersDecoded = null,
            note = null,
        )
    private fun sampleBfvParameters(): IdentifierBfvPublicParameters =
        IdentifierBfvPublicParameters(
            IdentifierBfvPublicParameters.Parameters(8L, 257L, 16_842_752L, 12),
            IdentifierBfvPublicParameters.PublicKey(
                listOf(11_472_226L, 15_791_131L, 10_301_391L, 6_321_610L, 502_045L, 1_948_157L, 5_332_249L, 12_641_494L),
                listOf(3_503_246L, 2_379_264L, 12_091_019L, 30_169L, 15_804_162L, 8_155_629L, 2_418_997L, 3_003_107L),
            ),
            3,
        )

    private open class CapturingExecutor : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var requestCount: Int = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requestCount += 1
            lastRequest = request
            if (request.uri.path.endsWith("/v1/node/capabilities")) {
                return CompletableFuture.completedFuture(compatibleCapabilitiesResponse())
            }
            return CompletableFuture.completedFuture(
                TransportResponse.builder().setStatusCode(404).setBody(byteArrayOf()).build(),
            )
        }
    }

    private class StubResponseExecutor(
        private val statusCode: Int,
        private val body: ByteArray,
        private val contentType: String? = null,
    ) : CapturingExecutor() {
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requestCount += 1
            lastRequest = request
            if (request.uri.path.endsWith("/v1/node/capabilities")) {
                return CompletableFuture.completedFuture(compatibleCapabilitiesResponse())
            }
            val response = TransportResponse.builder().setStatusCode(statusCode).setBody(body)
            contentType?.let { response.addHeader("Content-Type", it) }
            return CompletableFuture.completedFuture(response.build())
        }
    }

    private class QueueResponseExecutor(
        responses: List<Pair<Int, String>>,
    ) : HttpTransportExecutor {
        val requests = mutableListOf<TransportRequest>()
        private val responses = java.util.ArrayDeque(responses)

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests.add(request)
            val (statusCode, body) = responses.removeFirst()
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(statusCode)
                    .setBody(body.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }
    }

    private class TrackingExecutor(
        private val expectedHash: String,
        private val submitHeaderHash: String?,
    ) : HttpTransportExecutor {
        var observedExpectedHash = false
            private set
        var observedCommittedAsPending = false
            private set
        private var pollCount = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            if (request.uri.path.endsWith("/v1/node/capabilities")) {
                return CompletableFuture.completedFuture(compatibleCapabilitiesResponse())
            }
            if (request.method == "POST") {
                val builder = TransportResponse.builder()
                    .setStatusCode(202)
                    .setBody(byteArrayOf())
                if (submitHeaderHash != null) {
                    builder.addHeader("x-iroha-entrypoint-hash", submitHeaderHash)
                }
                return CompletableFuture.completedFuture(builder.build())
            }
            if (request.method == "GET") {
                if (request.uri.query == "hash=$expectedHash&scope=global") {
                    observedExpectedHash = true
                }
                val kind = when (pollCount++) {
                    0 -> "Queued"
                    1 -> "Committed".also { observedCommittedAsPending = true }
                    else -> "Applied"
                }
                return CompletableFuture.completedFuture(
                    TransportResponse.builder()
                        .setStatusCode(200)
                        .setBody(
                            """{"hash":"$expectedHash","status":{"kind":"$kind"${if (kind == "Applied") ",\"block_height\":7" else ""}},"scope":"global","resolved_from":"${if (kind == "Applied") "state" else "cache"}"}"""
                                .toByteArray(StandardCharsets.UTF_8),
                        )
                        .build(),
                )
            }
            throw IllegalStateException("Unexpected HTTP method ${request.method}")
        }
    }

    private companion object {
        // Shared by executors that model the signed-transaction capability probe.
        fun compatibleCapabilitiesResponse(): TransportResponse =
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(
                    (
                        "{\"data_model_version\":4,\"signed_transaction_schema_hash_hex\":" +
                            "\"7ab5ff9c572efb316deac478f19209c5\"}"
                        ).toByteArray(StandardCharsets.UTF_8),
                )
                .build()
    }
}
