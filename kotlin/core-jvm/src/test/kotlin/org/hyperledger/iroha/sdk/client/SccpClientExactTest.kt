package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.net.URI
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.sccp.SccpLaneIdV1
import org.hyperledger.iroha.sdk.sccp.SccpNetworkV1
import org.hyperledger.iroha.sdk.sccp.SccpV1
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

class SccpClientExactTest {
    private val authority = AccountAddress
        .fromAccount(TestEd25519Keys.publicKey(0x11), "ed25519")
        .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
    private val otherAuthority = AccountAddress
        .fromAccount(TestEd25519Keys.publicKey(0x12), "ed25519")
        .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
    private val bridgeFeePayment = FeePaymentIntent.authority(emptyList())

    @Test
    fun submitDtosExposeOnlyClosedArtifactFields() {
        val artifact = canonicalArtifact()
        val nativeArtifact = canonicalNativeArtifact()
        val proof = destinationRequest(authority, artifact)
        assertEquals(
            setOf("authority", "fee_payment", "destination_proof_b64"),
            proof.toJsonMap().keys,
        )
        HttpClientTransport.preflightSccpBridgeSubmitJson(
            proof.toJsonBytes(),
            "/v1/bridge/proofs/submit",
        )

        val message = messageRequest(authority, nativeArtifact)
        assertEquals(
            setOf("authority", "fee_payment", "native_proof_b64"),
            message.toJsonMap().keys,
        )
        HttpClientTransport.preflightSccpBridgeSubmitJson(
            message.toJsonBytes(),
            "/v1/bridge/messages",
        )

        val transactionBytes = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
            TransactionPayload(
                chainId = TAIRA_CHAIN_ID,
                authority = authority,
                creationTimeMs = 7,
                executable = Executable.instructions(emptyList()),
                feePayment = FeePaymentIntent.authority(emptyList()),
            ),
        )
        val transaction = Base64.getEncoder().encodeToString(transactionBytes)
        val gasBoundTransaction = Base64.getEncoder().encodeToString(
            NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
                TransactionPayload(
                    chainId = TAIRA_CHAIN_ID,
                    authority = authority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = FeePaymentIntent.authority(emptyList(), 9),
                ),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                artifact,
                signatureB64 = Base64.getEncoder().encodeToString(ByteArray(64) { 1 }),
                transactionPayloadB64 = gasBoundTransaction,
                creationTimeMs = 7,
            )
        }
        val signature = Base64.getEncoder().encodeToString(ByteArray(64) { 1 })
        val genericSignature = Base64.getEncoder().encodeToString(ByteArray(65) { 1 })
        assertEquals(genericSignature, normalizeOptionalSignature(genericSignature))
        val signed = destinationRequest(
            authority = authority,
            destinationProofB64 = artifact,
            signatureB64 = signature,
            transactionPayloadB64 = transaction,
            creationTimeMs = 7,
        )
        assertEquals(
            setOf(
                "authority", "fee_payment", "destination_proof_b64", "signature_b64",
                "transaction_payload_b64", "creation_time_ms",
            ),
            signed.toJsonMap().keys,
        )
        HttpClientTransport.preflightSccpBridgeSubmitJson(
            signed.toJsonBytes(),
            "/v1/bridge/proofs/submit",
        )
        val signedMessage = messageRequest(
            authority = authority,
            nativeProofB64 = nativeArtifact,
            signatureB64 = signature,
            transactionPayloadB64 = transaction,
            creationTimeMs = 7,
        )
        assertEquals(
            setOf(
                "authority", "fee_payment", "native_proof_b64", "signature_b64",
                "transaction_payload_b64", "creation_time_ms",
            ),
            signedMessage.toJsonMap().keys,
        )
        HttpClientTransport.preflightSccpBridgeSubmitJson(
            signedMessage.toJsonBytes(),
            "/v1/bridge/messages",
        )

        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                message.toJsonBytes(),
                "/v1/bridge/proofs/submit",
            )
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                proof.toJsonBytes(),
                "/v1/bridge/messages",
            )
        }

        val submitExecutor = SccpSubmitExecutor(listOf("application/json"))
        val transport = HttpClientTransport.withExecutor(
            submitExecutor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )
        transport.submitSccpDestinationProof(proof).join()
        transport.submitSccpNativeMessage(message).join()
        assertEquals(
            listOf(64L * 1024L * 1024L, 64L * 1024L * 1024L),
            submitExecutor.requests.map { it.maximumResponseBytes },
        )

        val missingContentType = SccpSubmitExecutor(emptyList())
        val strictTransport = HttpClientTransport.withExecutor(
            missingContentType,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )
        assertFailsWith<CompletionException> {
            strictTransport.submitSccpDestinationProof(proof).join()
        }
    }

    @Test
    fun signedSubmitPreservesExactTairaSponsorAcrossControllerOnlyWireIdentity() {
        val selector =
            "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/cbsi_web"
        val program = FeeSponsorProgramId.parse(selector)
        val expectedFeePayment = FeePaymentIntent.sponsor(
            programId = program,
            programRevision = 1,
            chargeLimits = emptyList(),
            gasLimit = 9,
        )
        val codec = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
        val signature = Base64.getEncoder().encodeToString(ByteArray(64) { 1 })

        fun transactionBytes(feePayment: FeePaymentIntent): ByteArray =
            codec.encodeTransaction(
                TransactionPayload(
                    chainId = TAIRA_CHAIN_ID,
                    authority = authority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = feePayment,
                ),
            )

        fun signedRequest(feePayment: FeePaymentIntent): SccpDestinationProofSubmitRequest =
            SccpDestinationProofSubmitRequest(
                authority = authority,
                destinationProofB64 = canonicalArtifact(),
                feePayment = expectedFeePayment,
                signatureB64 = signature,
                transactionPayloadB64 = Base64.getEncoder()
                    .encodeToString(transactionBytes(feePayment)),
                creationTimeMs = 7,
            )

        val encoded = transactionBytes(expectedFeePayment)
        val decoded = codec.decodeTransaction(encoded)
        val decodedSponsor = decoded.feePayment as FeePaymentIntent.Sponsor
        assertEquals(
            SccpV1.TAIRA_I105_DISCRIMINANT_V1,
            AccountAddress.detectI105Discriminant(decodedSponsor.programId.sponsor),
        )
        assertTrue(codec.encodeTransaction(decoded).contentEquals(encoded))
        assertEquals(selector, program.literal())

        val request = signedRequest(expectedFeePayment)
        assertEquals(selector, (request.feePayment as FeePaymentIntent.Sponsor).programId.literal())
        SccpNativeMessageSubmitRequest(
            authority = authority,
            nativeProofB64 = canonicalNativeArtifact(),
            feePayment = expectedFeePayment,
            signatureB64 = signature,
            transactionPayloadB64 = Base64.getEncoder().encodeToString(encoded),
            creationTimeMs = 7,
        )

        val mutations = listOf(
            FeePaymentIntent.sponsor(
                FeeSponsorProgramId(otherAuthority, "cbsi_web"),
                1,
                emptyList(),
                9,
            ),
            FeePaymentIntent.sponsor(
                FeeSponsorProgramId(program.sponsor, "cbsi_fx"),
                1,
                emptyList(),
                9,
            ),
            FeePaymentIntent.sponsor(program, 2, emptyList(), 9),
            FeePaymentIntent.sponsor(program, 1, emptyList(), 10),
            FeePaymentIntent.authority(emptyList(), 9),
        )
        for (mutation in mutations) {
            assertFailsWith<IllegalArgumentException> {
                signedRequest(mutation)
            }
        }
        assertFailsWith<IllegalArgumentException> {
            FeeSponsorProgramId(program.sponsor, "cbsi_e\u0301")
        }
    }

    @Test
    fun submitAuthorityRequiresExactTairaDiscriminant() {
        val address = AccountAddress.fromAccount(TestEd25519Keys.publicKey(0x41), "ed25519")
        val tairaAuthority = address.toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
        val artifact = canonicalArtifact()
        val nativeArtifact = canonicalNativeArtifact()
        assertEquals(369, SccpV1.TAIRA_I105_DISCRIMINANT_V1)
        assertEquals(369, AccountAddress.detectI105Discriminant(tairaAuthority))
        destinationRequest(tairaAuthority, artifact)
        messageRequest(tairaAuthority, nativeArtifact)

        val checksumMutation = tairaAuthority.dropLast(1) +
            if (tairaAuthority.last() == '1') "2" else "1"
        val invalidAuthorities = linkedMapOf(
            "default discriminant 753" to address.toI105Default(),
            "generic canonical hex" to address.canonicalHex(),
            "development discriminant" to address.toI105(0),
            "custom discriminant" to address.toI105(42),
            "malformed account alias" to "alice",
            "checksum mutation" to checksumMutation,
        )
        for ((label, invalidAuthority) in invalidAuthorities) {
            assertFailsWith<IllegalArgumentException>(label) {
                destinationRequest(invalidAuthority, artifact)
            }
            assertFailsWith<IllegalArgumentException>(label) {
                messageRequest(invalidAuthority, nativeArtifact)
            }
            assertFailsWith<IllegalArgumentException>(label) {
                HttpClientTransport.preflightSccpBridgeSubmitJson(
                    jsonBytes(linkedMapOf(
                        "authority" to invalidAuthority,
                        "destination_proof_b64" to artifact,
                    )),
                    "/v1/bridge/proofs/submit",
                )
            }
        }
    }

    @Test
    fun submitPreflightRejectsRetiredOverridesSecretsAndUnknownFields() {
        val artifact = canonicalArtifact()
        for (field in listOf(
            "private_key",
            "public_key_hex",
            "message_bundle_b64",
            "proof_bytes_hex",
            "network_id_hex",
            "verifier_address_hex",
            "bridge_address_hex",
            "tron_verifier_address",
            "manifest",
            "job",
        )) {
            val body = jsonBytes(
                linkedMapOf(
                    "authority" to authority,
                    "destination_proof_b64" to artifact,
                    field to "retired",
                ),
            )
            assertFailsWith<IllegalArgumentException>(field) {
                HttpClientTransport.preflightSccpBridgeSubmitJson(
                    body,
                    "/v1/bridge/proofs/submit",
                )
            }
        }
        for (field in listOf(
            "private_key",
            "public_key_hex",
            "message_bundle_b64",
            "destination_proof_b64",
            "settlement",
            "asset_id",
            "recipient",
        )) {
            val body = jsonBytes(
                linkedMapOf(
                    "authority" to authority,
                    "native_proof_b64" to artifact,
                    field to "retired",
                ),
            )
            assertFailsWith<IllegalArgumentException>(field) {
                HttpClientTransport.preflightSccpBridgeSubmitJson(body, "/v1/bridge/messages")
            }
        }
        for (body in listOf("[]", "null", "{", "42")) {
            assertFailsWith<IllegalArgumentException> {
                HttpClientTransport.preflightSccpBridgeSubmitJson(
                    body.toByteArray(),
                    "/v1/bridge/proofs/submit",
                )
            }
        }

        val duplicate = """
            {"authority":"$authority","authority":"$authority","destination_proof_b64":"$artifact"}
        """.trimIndent().toByteArray(Charsets.UTF_8)
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                duplicate,
                "/v1/bridge/proofs/submit",
            )
        }

        val signature = Base64.getEncoder().encodeToString(ByteArray(64) { 1 })
        val transaction = Base64.getEncoder().encodeToString(
            NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
                TransactionPayload(
                    chainId = TAIRA_CHAIN_ID,
                    authority = authority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = FeePaymentIntent.authority(emptyList()),
                ),
            ),
        )
        for (body in listOf(
            linkedMapOf<String, Any?>(
                "authority" to authority,
                "destination_proof_b64" to artifact,
                "signature_b64" to signature,
                "creation_time_ms" to 7,
            ),
            linkedMapOf<String, Any?>(
                "authority" to authority,
                "destination_proof_b64" to artifact,
                "transaction_payload_b64" to transaction,
                "creation_time_ms" to 7,
            ),
            linkedMapOf<String, Any?>(
                "authority" to authority,
                "destination_proof_b64" to artifact,
                "signature_b64" to signature,
                "transaction_payload_b64" to transaction,
            ),
            linkedMapOf<String, Any?>(
                "authority" to authority,
                "destination_proof_b64" to artifact,
                "signature_b64" to null,
                "transaction_payload_b64" to null,
            ),
            linkedMapOf<String, Any?>(
                "authority" to authority,
                "destination_proof_b64" to artifact,
                "creation_time_ms" to null,
            ),
        )) {
            assertFailsWith<IllegalArgumentException> {
                HttpClientTransport.preflightSccpBridgeSubmitJson(
                    jsonBytes(body),
                    "/v1/bridge/proofs/submit",
                )
            }
        }
    }

    @Test
    fun submitArtifactValidationRejectsAliasesCorruptionTrailingAndZeroSchema() {
        val canonical = canonicalArtifactBytes(SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME)
        val encoded = Base64.getEncoder().encodeToString(canonical)
        assertFailsWith<IllegalArgumentException> {
            destinationRequest("alice", encoded)
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(authority, encoded.trimEnd('='))
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(authority, " $encoded")
        }
        val corrupted = canonical.copyOf().also { it[it.lastIndex] = (it.last() + 1).toByte() }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                Base64.getEncoder().encodeToString(corrupted),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                Base64.getEncoder().encodeToString(canonical + 0),
            )
        }
        val zeroSchema = canonical.copyOf().also { it.fill(0, 6, 22) }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                Base64.getEncoder().encodeToString(zeroSchema),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(authority, encoded, creationTimeMs = 0)
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(authority, encoded, signatureB64 = "AQ==")
        }
        val signature = Base64.getEncoder().encodeToString(ByteArray(64) { 1 })
        val transactionBytes = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
            TransactionPayload(
                chainId = TAIRA_CHAIN_ID,
                authority = authority,
                creationTimeMs = 7,
                executable = Executable.instructions(emptyList()),
                feePayment = FeePaymentIntent.authority(emptyList()),
            ),
        )
        val transaction = Base64.getEncoder().encodeToString(transactionBytes)
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                signatureB64 = signature,
                creationTimeMs = 7,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                transactionPayloadB64 = transaction,
                creationTimeMs = 7,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                signatureB64 = signature,
                transactionPayloadB64 = transaction,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                signatureB64 = signature,
                transactionPayloadB64 = transaction,
                creationTimeMs = 8,
            )
        }
        val wrongAuthorityTransaction = Base64.getEncoder().encodeToString(
            NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
                TransactionPayload(
                    chainId = TAIRA_CHAIN_ID,
                    authority = otherAuthority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = FeePaymentIntent.authority(emptyList()),
                ),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                signatureB64 = signature,
                transactionPayloadB64 = wrongAuthorityTransaction,
                creationTimeMs = 7,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                signatureB64 = Base64.getEncoder().encodeToString(ByteArray(64)),
                transactionPayloadB64 = transaction,
                creationTimeMs = 7,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                encoded,
                signatureB64 = Base64.getEncoder().encodeToString(
                    ByteArray(16 * 1024 + 1) { 1 },
                ),
                transactionPayloadB64 = transaction,
                creationTimeMs = 7,
            )
        }
        val native = canonicalArtifactBytes(SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME)
        messageRequest(authority, Base64.getEncoder().encodeToString(native))
        assertFailsWith<IllegalArgumentException> {
            messageRequest(authority, encoded)
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                Base64.getEncoder().encodeToString(native),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            messageRequest(
                authority,
                Base64.getEncoder().encodeToString(
                    canonicalArtifactBytes(SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME, 8),
                ),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority,
                Base64.getEncoder().encodeToString(
                    canonicalArtifactBytes(SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME, 8),
                ),
            )
        }
    }

    @Test
    fun capabilitiesAreExactAndContainNoRetiredDiscoverySurface() {
        val parsed = SccpJsonParser.parseCapabilities(jsonBytes(capabilities()))
        assertEquals("/v1/sccp/registry", parsed.registryPath)
        assertEquals("/v1/sccp/proof-requests/{message_id}", parsed.proofRequestPath)
        assertEquals(64, parsed.registryLimits.maxRetainedRoutesPerLane)
        assertEquals(4_096, parsed.registryLimits.maxRetainedNativeTrustAnchorsPerLane)
        assertEquals(512, parsed.resourceLimits.maxOutboundMessagesPerBlock)
        assertEquals(
            BigInteger.valueOf(4_096),
            parsed.resourceLimits.maxOutboundMessagePayloadBytes,
        )
        assertEquals(
            BigInteger.valueOf(65_536),
            parsed.resourceLimits.maxPendingOutboundMessages,
        )
        assertEquals(
            BigInteger.valueOf(268_435_456),
            parsed.resourceLimits.maxPendingOutboundPayloadBytes,
        )
        assertEquals(131_713, parsed.resourceLimits.maxBlsSignerContributionsPerTransaction)
        assertNull(parsed.proofSubmitPath)

        val enabled = capabilities().also {
            it["proof_submit_path"] = "/v1/bridge/proofs/submit"
            it["native_message_submit_path"] = "/v1/bridge/messages"
        }
        val enabledParsed = SccpJsonParser.parseCapabilities(jsonBytes(enabled))
        assertEquals("/v1/bridge/proofs/submit", enabledParsed.proofSubmitPath)
        assertEquals("/v1/bridge/messages", enabledParsed.nativeMessageSubmitPath)

        val proofOnly = capabilities().also {
            it["proof_submit_path"] = "/v1/bridge/proofs/submit"
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(jsonBytes(proofOnly))
        }
        val messageOnly = capabilities().also {
            it["native_message_submit_path"] = "/v1/bridge/messages"
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(jsonBytes(messageOnly))
        }

        for ((field, replacement) in listOf(
            "registry_path" to "/v1/sccp/manifests",
            "proof_request_path" to "/v1/sccp/jobs/message/{message_id}",
            "message_bundle_path" to "/v1/sccp/artifacts/message/{message_id}",
        )) {
            val hostile = capabilities().toMutableMap().also { it[field] = replacement }
            assertFailsWith<IllegalArgumentException>(field) {
                SccpJsonParser.parseCapabilities(jsonBytes(hostile))
            }
        }
        for (retired in listOf("outbound", "codecs", "inbound_lanes", "manifests")) {
            val hostile = capabilities().toMutableMap().also { it[retired] = emptyMap<String, Any>() }
            assertFailsWith<IllegalArgumentException>(retired) {
                SccpJsonParser.parseCapabilities(jsonBytes(hostile))
            }
        }

        val resourceKeys = listOf(
            "max_outbound_messages_per_block", "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages", "max_pending_outbound_payload_bytes",
            "max_proofs_per_transaction", "max_proofs_per_block", "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction", "max_proof_bytes_per_block",
            "max_native_headers_per_transaction", "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "max_native_header_bytes_per_transaction", "max_native_header_bytes_per_block",
            "max_secp256k1_recoveries_per_transaction", "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction", "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block",
        )
        for (key in resourceKeys) {
            val hostile = capabilities()
            @Suppress("UNCHECKED_CAST")
            (hostile["resource_limits"] as MutableMap<String, Any?>)[key] = 0
            assertFailsWith<IllegalArgumentException>(key) {
                SccpJsonParser.parseCapabilities(jsonBytes(hostile))
            }
        }
        for (fixedField in listOf(
            "max_outbound_messages_per_block",
            "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
        )) {
            val missing = capabilities()
            @Suppress("UNCHECKED_CAST")
            (missing["resource_limits"] as MutableMap<String, Any?>).remove(fixedField)
            assertFailsWith<IllegalArgumentException>("missing $fixedField") {
                SccpJsonParser.parseCapabilities(jsonBytes(missing))
            }
        }
        val unknownResourceLimit = capabilities()
        @Suppress("UNCHECKED_CAST")
        (unknownResourceLimit["resource_limits"] as MutableMap<String, Any?>)[
            "max_outbound_messages_per_transaction"
        ] = 1
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(jsonBytes(unknownResourceLimit))
        }
        for ((fixedField, hostileValues) in listOf(
            "max_outbound_messages_per_block" to listOf<Any?>(511, 513, "512", null),
            "max_outbound_message_payload_bytes" to listOf<Any?>(4_095, 4_097, "4096", null),
        )) {
            for (hostileValue in hostileValues) {
                val hostile = capabilities()
                @Suppress("UNCHECKED_CAST")
                (hostile["resource_limits"] as MutableMap<String, Any?>)[fixedField] = hostileValue
                assertFailsWith<IllegalArgumentException>("$fixedField=$hostileValue") {
                    SccpJsonParser.parseCapabilities(jsonBytes(hostile))
                }
            }
        }
        val jsSafeMaximum = 9_007_199_254_740_991L
        val byteLimitKeys = listOf(
            "max_pending_outbound_messages",
            "max_pending_outbound_payload_bytes",
            "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction",
            "max_proof_bytes_per_block",
            "max_native_header_bytes_per_transaction",
            "max_native_header_bytes_per_block",
        )
        for (key in resourceKeys) {
            val overflow = if (key in byteLimitKeys) {
                jsSafeMaximum + 1L
            } else {
                4_294_967_296L
            }
            for (replacement in listOf<Any?>(true, 1.5, overflow)) {
                val hostile = capabilities()
                @Suppress("UNCHECKED_CAST")
                (hostile["resource_limits"] as MutableMap<String, Any?>)[key] = replacement
                assertFailsWith<IllegalArgumentException>("$key=$replacement") {
                    SccpJsonParser.parseCapabilities(jsonBytes(hostile))
                }
            }
        }
        val boundary = capabilities()
        @Suppress("UNCHECKED_CAST")
        val boundaryLimits = boundary["resource_limits"] as MutableMap<String, Any?>
        byteLimitKeys.forEach { boundaryLimits[it] = jsSafeMaximum }
        val boundaryParsed = SccpJsonParser.parseCapabilities(jsonBytes(boundary))
        assertEquals(
            BigInteger.valueOf(jsSafeMaximum),
            boundaryParsed.resourceLimits.maxProofBytesPerBlock,
        )
        for (key in byteLimitKeys) {
            val overflow = capabilities()
            @Suppress("UNCHECKED_CAST")
            (overflow["resource_limits"] as MutableMap<String, Any?>)[key] =
                jsSafeMaximum + 1
            assertFailsWith<IllegalArgumentException>(key) {
                SccpJsonParser.parseCapabilities(jsonBytes(overflow))
            }
        }
        val canonicalJson = String(jsonBytes(capabilities()), Charsets.UTF_8)
        val integerToken = "\"max_proof_bytes_per_proof\":8388608"
        for (replacement in listOf("1.0", "1e0", "-1", "01", "١")) {
            val hostile = canonicalJson.replace(
                integerToken,
                "\"max_proof_bytes_per_proof\":$replacement",
            )
            assertTrue(hostile != canonicalJson)
            assertFailsWith<RuntimeException>(replacement) {
                SccpJsonParser.parseCapabilities(hostile.toByteArray(Charsets.UTF_8))
            }
        }
        val orderingRelations = listOf(
            "max_proof_bytes_per_proof" to "max_proof_bytes_per_transaction",
            "max_proofs_per_transaction" to "max_proofs_per_block",
            "max_proof_bytes_per_transaction" to "max_proof_bytes_per_block",
            "max_native_headers_per_transaction" to "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction" to
                "max_ethereum_light_client_updates_per_block",
            "max_native_header_bytes_per_transaction" to "max_native_header_bytes_per_block",
            "max_secp256k1_recoveries_per_transaction" to
                "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction" to
                "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction" to
                "max_bls_signer_contributions_per_block",
            "max_bn254_pairing_checks_per_transaction" to
                "max_bn254_pairing_checks_per_block",
        )
        for ((lowerField, upperField) in orderingRelations) {
            val hostile = capabilities()
            @Suppress("UNCHECKED_CAST")
            val limits = hostile["resource_limits"] as MutableMap<String, Any?>
            limits[lowerField] = (limits[upperField] as Number).toLong() + 1L
            assertFailsWith<IllegalArgumentException>("$lowerField <= $upperField") {
                SccpJsonParser.parseCapabilities(jsonBytes(hostile))
            }
        }
        val driftedRegistryLimits = capabilities()
        @Suppress("UNCHECKED_CAST")
        (driftedRegistryLimits["registry_limits"] as MutableMap<String, Any?>)[
            "max_retained_routes_per_lane"
        ] = 65
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseCapabilities(jsonBytes(driftedRegistryLimits))
        }
    }

    private fun destinationRequest(
        authority: String,
        destinationProofB64: String,
        signatureB64: String? = null,
        transactionPayloadB64: String? = null,
        creationTimeMs: Long? = null,
    ): SccpDestinationProofSubmitRequest = SccpDestinationProofSubmitRequest(
        authority,
        destinationProofB64,
        bridgeFeePayment,
        signatureB64,
        transactionPayloadB64,
        creationTimeMs,
    )

    private fun messageRequest(
        authority: String,
        nativeProofB64: String,
        signatureB64: String? = null,
        transactionPayloadB64: String? = null,
        creationTimeMs: Long? = null,
    ): SccpNativeMessageSubmitRequest = SccpNativeMessageSubmitRequest(
        authority,
        nativeProofB64,
        bridgeFeePayment,
        signatureB64,
        transactionPayloadB64,
        creationTimeMs,
    )

    @Test
    fun registryValidatesElevenSignalKeySemanticPolicyAndExactFamilies() {
        val parsed = SccpJsonParser.parseRegistry(jsonBytes(registry()))
        assertEquals(1, parsed.version)
        assertEquals(1, parsed.lanes.size)
        assertEquals(DEFAULT_ROUTE_CONFIG_HASH, fixtureRouteConfigurationHash(route(registry())))

        val badKey = registry()
        deployment(badKey)["verifier_key_hash"] = upper(0x2f, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(badKey))
        }

        val exactAnchors = registry()
        laneRecord(exactAnchors)["native_trust_anchors"] =
            MutableList<Any?>(4_096) { null }
        val exactAnchorError = assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(exactAnchors))
        }
        assertFalse(exactAnchorError.message.orEmpty().contains("more than 4,096"))
        val overAnchors = registry()
        laneRecord(overAnchors)["native_trust_anchors"] =
            MutableList<Any?>(4_097) { null }
        val overAnchorError = assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(overAnchors))
        }
        assertTrue(overAnchorError.message.orEmpty().contains("more than 4,096"))

        val exactRoutes = registry()
        laneRecord(exactRoutes)["routes"] = MutableList<Any?>(64) { linkedMapOf<String, Any?>() }
        val exactRouteError = assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(exactRoutes))
        }
        assertFalse(exactRouteError.message.orEmpty().contains("more than 64 retained"))
        val overRoutes = registry()
        laneRecord(overRoutes)["routes"] = MutableList<Any?>(65) { linkedMapOf<String, Any?>() }
        val overRouteError = assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(overRoutes))
        }
        assertTrue(overRouteError.message.orEmpty().contains("more than 64 retained"))

        val missingSignal = registry()
        @Suppress("UNCHECKED_CAST")
        val ic = ((deployment(missingSignal)["verifying_key"] as MutableMap<String, Any?>)["ic"]
            as MutableMap<String, Any?>)
        ic.remove("signal_10")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(missingSignal))
        }

        val retired = registry()
        @Suppress("UNCHECKED_CAST")
        val lane = ((retired["lanes"] as MutableList<Any?>)[0] as MutableMap<String, Any?>)["lane_id"]
            as MutableMap<String, Any?>
        lane["source"] = network("solana-mainnet-beta")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(retired))
        }

        val missingPolicy = registry()
        deployment(missingPolicy).remove("outbound_proof_policy")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(missingPolicy))
        }

        val wrongSchema = registry()
        @Suppress("UNCHECKED_CAST")
        val policy = deployment(wrongSchema)["outbound_proof_policy"] as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val semantic = (policy["semantic_profile"] as MutableMap<String, Any?>)["commitments"]
            as MutableMap<String, Any?>
        semantic["public_signal_schema_hash"] = upper(0x2e, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongSchema))
        }
    }

    @Test
    fun registryRequiresCanonicalTrustAnchorHistoryAndCurrentPointer() {
        val legacy = registry()
        laneRecord(legacy).also {
            it.remove("native_trust_anchors")
            it.remove("current_native_trust_anchor_hash")
            it["native_trust_anchor"] = null
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(legacy))
        }

        val nullAnchor = registry()
        anchors(nullAnchor).add(null)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(nullAnchor))
        }

        val valid = registry()
        anchors(valid).add(nativeTrustAnchor(0x61, 7))
        anchors(valid).add(nativeTrustAnchor(0x62, 8))
        laneRecord(valid)["current_native_trust_anchor_hash"] = upper(0x62, 32)
        SccpJsonParser.parseRegistry(jsonBytes(valid))

        val duplicate = registry()
        anchors(duplicate).add(nativeTrustAnchor(0x61, 7))
        anchors(duplicate).add(nativeTrustAnchor(0x61, 8))
        laneRecord(duplicate)["current_native_trust_anchor_hash"] = upper(0x61, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(duplicate))
        }

        val nonIncreasing = registry()
        anchors(nonIncreasing).add(nativeTrustAnchor(0x61, 8))
        anchors(nonIncreasing).add(nativeTrustAnchor(0x62, 8))
        laneRecord(nonIncreasing)["current_native_trust_anchor_hash"] = upper(0x62, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(nonIncreasing))
        }

        val wrongFamily = registry()
        anchors(wrongFamily).add(nativeTrustAnchor(0x61, 7, "ethereum_beacon_v1"))
        laneRecord(wrongFamily)["current_native_trust_anchor_hash"] = upper(0x61, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongFamily))
        }

        val wrongPointer = registry()
        anchors(wrongPointer).add(nativeTrustAnchor(0x61, 7))
        anchors(wrongPointer).add(nativeTrustAnchor(0x62, 8))
        laneRecord(wrongPointer)["current_native_trust_anchor_hash"] = upper(0x61, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongPointer))
        }

        val pointerWithoutHistory = registry()
        laneRecord(pointerWithoutHistory)["current_native_trust_anchor_hash"] = upper(0x61, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(pointerWithoutHistory))
        }

        val missingPointer = registry()
        anchors(missingPointer).add(nativeTrustAnchor(0x61, 7))
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(missingPointer))
        }

        val inboundWithoutAnchor = registry()
        activation(route(inboundWithoutAnchor))["activation"] = "bidirectional"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(inboundWithoutAnchor))
        }
        val inboundWithAnchor = registry()
        activation(route(inboundWithAnchor))["activation"] = "bidirectional"
        anchors(inboundWithAnchor).add(nativeTrustAnchor(0x61, 7))
        laneRecord(inboundWithAnchor)["current_native_trust_anchor_hash"] = upper(0x61, 32)
        SccpJsonParser.parseRegistry(jsonBytes(inboundWithAnchor))
    }

    @Test
    fun retiredRouteHistoryDoesNotConsumeTheEightRouteLiveCapacity() {
        val retiredHistory = registry()
        anchors(retiredHistory).add(nativeTrustAnchor(0x61, 7))
        anchors(retiredHistory).add(nativeTrustAnchor(0x62, 8))
        laneRecord(retiredHistory)["current_native_trust_anchor_hash"] = upper(0x62, 32)
        val template = route(retiredHistory)
        val routes = routes(retiredHistory)
        routes.clear()
        for (revision in 1..9) {
            val historical = deepMutableCopy(template)
            historical["revision"] = revision
            activation(historical)["activation"] = "retired"
            historical["inbound_finality_cutoff"] = inboundFinalityCutoff()
            val routeAddress = upper(0x31 + revision, 20)
            routeDeployment(historical)["route_address"] = routeAddress
            sourceIdentity(historical)["address"] = routeAddress
            refreshRouteConfigurationHash(historical)
            routes.add(historical)
        }
        SccpJsonParser.parseRegistry(jsonBytes(retiredHistory))

        val tooManyLive = registry()
        val liveTemplate = route(tooManyLive)
        val liveRoutes = routes(tooManyLive)
        liveRoutes.clear()
        for (revision in 1..9) {
            val live = deepMutableCopy(liveTemplate)
            live["revision"] = revision
            activation(live)["activation"] = "staged"
            val routeAddress = upper(0x31 + revision, 20)
            routeDeployment(live)["route_address"] = routeAddress
            sourceIdentity(live)["address"] = routeAddress
            refreshRouteConfigurationHash(live)
            liveRoutes.add(live)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(tooManyLive))
        }
    }

    @Test
    fun registryRequiresExactRetiredRouteInboundFinalityCutoff() {
        val missing = registry()
        route(missing).remove("inbound_finality_cutoff")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(missing))
        }

        val liveWithCutoff = registry()
        route(liveWithCutoff)["inbound_finality_cutoff"] = inboundFinalityCutoff()
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(liveWithCutoff))
        }

        val retiredWithoutCutoff = registry()
        activation(route(retiredWithoutCutoff))["activation"] = "retired"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(retiredWithoutCutoff))
        }

        val valid = registry()
        anchors(valid).add(nativeTrustAnchor(0x61, 7))
        anchors(valid).add(nativeTrustAnchor(0x62, 8))
        laneRecord(valid)["current_native_trust_anchor_hash"] = upper(0x62, 32)
        activation(route(valid))["activation"] = "retired"
        route(valid)["inbound_finality_cutoff"] = inboundFinalityCutoff()
        SccpJsonParser.parseRegistry(jsonBytes(valid))

        val unknownAnchor = deepMutableCopy(valid)
        @Suppress("UNCHECKED_CAST")
        (route(unknownAnchor)["inbound_finality_cutoff"] as MutableMap<String, Any?>)["trust_anchor_hash"] =
            upper(0x7f, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(unknownAnchor))
        }

        val openEndedCurrentAnchor = deepMutableCopy(valid)
        @Suppress("UNCHECKED_CAST")
        (route(openEndedCurrentAnchor)["inbound_finality_cutoff"] as MutableMap<String, Any?>).also {
            it["trust_anchor_hash"] = upper(0x62, 32)
            it["max_anchor_interval_height"] = 9
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(openEndedCurrentAnchor))
        }

        val partialInterval = deepMutableCopy(valid)
        @Suppress("UNCHECKED_CAST")
        (route(partialInterval)["inbound_finality_cutoff"] as MutableMap<String, Any?>)["max_anchor_interval_height"] = 7
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(partialInterval))
        }

        val legacyExtraField = deepMutableCopy(valid)
        @Suppress("UNCHECKED_CAST")
        (route(legacyExtraField)["inbound_finality_cutoff"] as MutableMap<String, Any?>)["height"] = 8
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(legacyExtraField))
        }
    }

    @Test
    fun registryAuthenticatesExactRouteIdentityConfigurationAndBothPolicyHashes() {
        SccpJsonParser.parseRegistry(jsonBytes(tronRegistry()))

        val wrongRouteId = registry()
        route(wrongRouteId)["route_id"] = "taira_eth_xor"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongRouteId))
        }

        val wrongConfiguration = registry()
        sourceIdentity(route(wrongConfiguration))["route_config_hash"] = upper(0x7f, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongConfiguration))
        }

        val changedSemanticPolicy = registry()
        @Suppress("UNCHECKED_CAST")
        val semanticCommitments = (((deployment(changedSemanticPolicy)["outbound_proof_policy"]
            as MutableMap<String, Any?>)["semantic_profile"] as MutableMap<String, Any?>)["commitments"]
            as MutableMap<String, Any?>)
        semanticCommitments["circuit_commitment"] = upper(0x18, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(changedSemanticPolicy))
        }

        val changedFinalityPolicy = registry()
        @Suppress("UNCHECKED_CAST")
        val finalityAnchor = (deployment(changedFinalityPolicy)["outbound_proof_policy"]
            as MutableMap<String, Any?>)["sora_finality_anchor"] as MutableMap<String, Any?>
        finalityAnchor["checkpoint_block_hash"] = upper(0x17, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(changedFinalityPolicy))
        }
    }

    @Test
    fun bn254CoordinatesMayBeZeroButWholePointsMayNotBeInfinity() {
        val changedKeyWithoutRouteRebind = registry()
        @Suppress("UNCHECKED_CAST")
        val changedKey = deployment(changedKeyWithoutRouteRebind)["verifying_key"]
            as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val changedAlpha = changedKey["alpha1"] as MutableMap<String, Any?>
        changedAlpha["x"] = upper(0, 32)
        deployment(changedKeyWithoutRouteRebind)["verifier_key_hash"] = keyHash(changedKey)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(changedKeyWithoutRouteRebind))
        }

        val partialZero = registry()
        @Suppress("UNCHECKED_CAST")
        val key = deployment(partialZero)["verifying_key"] as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val alpha = key["alpha1"] as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val beta = key["beta2"] as MutableMap<String, Any?>
        alpha["x"] = upper(0, 32)
        beta["x_c0"] = upper(0, 32)
        deployment(partialZero)["verifier_key_hash"] = keyHash(key)
        refreshRouteConfigurationHash(route(partialZero))
        SccpJsonParser.parseRegistry(jsonBytes(partialZero))

        val infiniteG1 = registry()
        @Suppress("UNCHECKED_CAST")
        val infiniteG1Key = deployment(infiniteG1)["verifying_key"] as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val infiniteAlpha = infiniteG1Key["alpha1"] as MutableMap<String, Any?>
        infiniteAlpha["x"] = upper(0, 32)
        infiniteAlpha["y"] = upper(0, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(infiniteG1))
        }

        val infiniteG2 = registry()
        @Suppress("UNCHECKED_CAST")
        val infiniteG2Key = deployment(infiniteG2)["verifying_key"] as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val infiniteBeta = infiniteG2Key["beta2"] as MutableMap<String, Any?>
        for (field in listOf("x_c0", "x_c1", "y_c0", "y_c1")) {
            infiniteBeta[field] = upper(0, 32)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(infiniteG2))
        }
    }

    @Test
    fun bundleAndProofRequestRejectRetiredVariantsHashAliasesAndBackendConfusion() {
        val bundle = SccpJsonParser.parseMessageBundle(jsonBytes(messageBundle()))
        assertEquals(MESSAGE_ID, bundle.messageIdHex)
        assertEquals("bsc-mainnet", bundle.targetNetwork.profileKey)

        val retiredPayload = messageBundle()
        retiredPayload["payload"] = linkedMapOf("TokenPause" to emptyMap<String, Any>())
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseMessageBundle(jsonBytes(retiredPayload))
        }

        val alias = messageBundle()
        @Suppress("UNCHECKED_CAST")
        val context = ((alias["commitment"] as MutableMap<String, Any?>)["context"]
            as MutableMap<String, Any?>)
        context["route_configuration_hash"] = context["destination_binding_hash"]
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseMessageBundle(jsonBytes(alias))
        }

        val request = SccpJsonParser.parseProofRequest(jsonBytes(proofRequest()))
        assertEquals(MESSAGE_ID, request.messageIdHex)
        assertEquals("evm_groth16_bn254_v1", request.backend)
        assertEquals("sora_taira_finality_inclusion_groth16_bn254", request.semanticProofProfile.profile)
        assertEquals(1, request.semanticProofProfile.commitments.version)
        assertEquals(upper(0x11, 32), request.semanticProofProfile.commitments.circuitCommitment)
        assertEquals(upper(0x12, 32), request.semanticProofProfile.commitments.witnessGeneratorCommitment)
        assertEquals(publicSignalSchemaHash(), request.semanticProofProfile.commitments.publicSignalSchemaHash)
        assertEquals("0x${semanticProfileHash().lowercase()}", request.semanticProofProfile.profileHash)
        assertEquals(1, request.soraFinalityAnchor.version)
        assertEquals(SccpNetworkV1.SORA_TAIRA, request.soraFinalityAnchor.sourceNetwork)
        assertEquals(tairaChainIdHash(), request.soraFinalityAnchor.chainIdHash)
        assertEquals(BigInteger.valueOf(7), request.soraFinalityAnchor.checkpointHeight)
        assertEquals(upper(0xa1, 32), request.soraFinalityAnchor.checkpointBlockHash)
        assertEquals(3, request.soraFinalityAnchor.protocolVersion)
        assertEquals(upper(0xa2, 32), request.soraFinalityAnchor.checkpointContextId)
        assertEquals(upper(0xa3, 32), request.soraFinalityAnchor.checkpointFinalityArtifactHash)
        assertEquals(
            "0xec6c821caf5fa74368c08e9101ab310f132fb7f627a09f6f9481aa9484054bba",
            request.soraFinalityAnchor.anchorHash,
        )
        assertEquals("0x${finalityAnchorHash().lowercase()}", request.soraFinalityAnchor.anchorHash)

        val current = proofRequest()
        @Suppress("UNCHECKED_CAST")
        val currentAnchor = current["sora_finality_anchor"] as MutableMap<String, Any?>
        currentAnchor["protocol_version"] = 4
        current["sora_finality_anchor_hash"] = "0x${finalityAnchorHash(4).lowercase()}"
        val currentRequest = SccpJsonParser.parseProofRequest(jsonBytes(current))
        assertEquals(4, currentRequest.soraFinalityAnchor.protocolVersion)
        assertTrue(
            currentRequest.soraFinalityAnchor.anchorHash != request.soraFinalityAnchor.anchorHash,
        )

        val invalidFinalityAnchors: List<(MutableMap<String, Any?>) -> Unit> = listOf(
            { it["protocol_version"] = 1 },
            { it["protocol_version"] = 5 },
            { it["protocol_version"] = "3" },
            { it["protocol_version"] = 3.0 },
            { it["protocol_version"] = true },
            { it["validator_set_epoch"] = 3 },
            { it["checkpoint_context_id"] = upper(0, 32) },
            { it["checkpoint_context_id"] = it["chain_id_hash"] },
            { it["checkpoint_block_hash"] = it["checkpoint_context_id"] },
            { it["checkpoint_finality_artifact_hash"] = upper(0, 32) },
            { it["checkpoint_finality_artifact_hash"] = it["checkpoint_block_hash"] },
            { it.remove("checkpoint_finality_artifact_hash") },
        )
        for (mutation in invalidFinalityAnchors) {
            val hostile = proofRequest()
            @Suppress("UNCHECKED_CAST")
            val anchor = hostile["sora_finality_anchor"] as MutableMap<String, Any?>
            mutation(anchor)
            assertFailsWith<IllegalArgumentException> {
                SccpJsonParser.parseProofRequest(jsonBytes(hostile))
            }
        }

        val wrongBackend = proofRequest()
        wrongBackend["backend"] = linkedMapOf("backend" to "tron_groth16_bn254_v1", "family" to null)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(wrongBackend))
        }
        val override = proofRequest().also { it["network_id_hex"] = prefixed(0x7f) }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(override))
        }
        val semanticHashMismatch = proofRequest().also {
            it["semantic_proof_profile_hash"] = prefixed(0x7d)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(semanticHashMismatch))
        }
        val anchorHashMismatch = proofRequest().also {
            it["sora_finality_anchor_hash"] = prefixed(0x7e)
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(anchorHashMismatch))
        }
        val archivedIdentity = proofRequest().also {
            @Suppress("UNCHECKED_CAST")
            val anchor = it["sora_finality_anchor"] as MutableMap<String, Any?>
            anchor["chain_id_hash"] = keccak(
                "809574f5fee75e69bfcf52451e42d50f".hexToBytes(),
            ).toUpperHex()
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(archivedIdentity))
        }
        val oversizedAmount = messageBundle()
        @Suppress("UNCHECKED_CAST")
        val transfer = (oversizedAmount["payload"] as MutableMap<String, Any?>)["Transfer"]
            as MutableMap<String, Any?>
        transfer["amount"] = BigInteger.ONE.shiftLeft(128).toString()
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseMessageBundle(jsonBytes(oversizedAmount))
        }
    }

    @Test
    fun recentMessagesRequireTransferOnlyBothHashRolesAndExactLinks() {
        val parsed = SccpJsonParser.parseRecentMessages(
            jsonBytes(linkedMapOf(
                "items" to mutableListOf(recent(9, MESSAGE_ID), recent(8, hash(0x12))),
                "next" to linkedMapOf("from" to 8, "after_index" to 0),
            )),
        )
        assertEquals(
            listOf(BigInteger.valueOf(9), BigInteger.valueOf(8)),
            parsed.items.map { it.height },
        )
        assertEquals(listOf(0, 0), parsed.items.map { it.commitmentIndex })
        assertEquals(SccpRecentCursor(BigInteger.valueOf(8), 0), parsed.next)
        assertEquals(prefixed(0x72), parsed.items.first().routeConfigurationHash)
        assertTrue(parsed.items.first().payloadProjection.containsKey("Transfer"))
        @Suppress("UNCHECKED_CAST")
        val immutableProjectionTransfer = parsed.items.first().payloadProjection["Transfer"]
            as MutableMap<String, Any?>
        assertFailsWith<UnsupportedOperationException> {
            immutableProjectionTransfer["version"] = 2
        }

        val sameHeight = SccpJsonParser.parseRecentMessages(
            jsonBytes(linkedMapOf(
                "items" to listOf(
                    recent(9, MESSAGE_ID, 0),
                    recent(9, hash(0x12), 1),
                ),
                "next" to null,
            )),
        )
        assertEquals(listOf(0, 1), sameHeight.items.map { it.commitmentIndex })
        assertNull(sameHeight.next)
        assertNull(
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to emptyList<Any>())),
            ).next,
        )
        val maxU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val maxHeight = SccpJsonParser.parseRecentMessages(
            jsonBytes(linkedMapOf(
                "items" to listOf(recent(maxU64, MESSAGE_ID, 511)),
                "next" to linkedMapOf("from" to maxU64, "after_index" to 511),
            )),
        )
        assertEquals(maxU64, maxHeight.items.single().height)
        assertEquals(SccpRecentCursor(maxU64, 511), maxHeight.next)

        for (replacement in listOf<Any?>(null, -1, 512, "0", 0.0, true)) {
            val hostile = recent(9, MESSAGE_ID).also { it["commitment_index"] = replacement }
            assertFailsWith<IllegalArgumentException>("commitment_index=$replacement") {
                SccpJsonParser.parseRecentMessages(
                    jsonBytes(linkedMapOf("items" to listOf(hostile))),
                )
            }
        }
        val missingCommitmentIndex = recent(9, MESSAGE_ID).also {
            it.remove("commitment_index")
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(missingCommitmentIndex))),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf(
                    "items" to listOf(recent(BigInteger.ONE.shiftLeft(64), MESSAGE_ID)),
                )),
            )
        }
        for (items in listOf(
            listOf(recent(9, MESSAGE_ID, 1), recent(9, hash(0x12), 0)),
            listOf(recent(9, MESSAGE_ID, 0), recent(9, hash(0x12), 0)),
        )) {
            assertFailsWith<IllegalArgumentException> {
                SccpJsonParser.parseRecentMessages(jsonBytes(linkedMapOf("items" to items)))
            }
        }
        val cursorHostiles = listOf(
            linkedMapOf<String, Any?>("from" to 9),
            linkedMapOf<String, Any?>("after_index" to 0),
            linkedMapOf<String, Any?>("from" to 0, "after_index" to 0),
            linkedMapOf<String, Any?>(
                "from" to BigInteger.ONE.shiftLeft(64),
                "after_index" to 0,
            ),
            linkedMapOf<String, Any?>("from" to 9, "after_index" to 512),
            linkedMapOf<String, Any?>("from" to 9, "after_index" to 0, "offset" to 0),
            linkedMapOf<String, Any?>("from" to 8, "after_index" to 0),
            linkedMapOf<String, Any?>("from" to 9, "after_index" to 1),
        )
        for (cursor in cursorHostiles) {
            assertFailsWith<IllegalArgumentException>(cursor.toString()) {
                SccpJsonParser.parseRecentMessages(
                    jsonBytes(linkedMapOf(
                        "items" to listOf(recent(9, MESSAGE_ID, 0)),
                        "next" to cursor,
                    )),
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf(
                    "items" to emptyList<Any>(),
                    "next" to linkedMapOf("from" to 9, "after_index" to 0),
                )),
            )
        }

        val tronProjection = recent(9, MESSAGE_ID).also {
            it["target_profile"] = "tron-mainnet"
            it["target_domain"] = 5
            it["route_id"] = "taira_tron_xor"
        }
        @Suppress("UNCHECKED_CAST")
        val tronTransfer = (tronProjection["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
            as MutableMap<String, Any?>
        tronTransfer["dest_domain"] = 5
        tronTransfer["recipient"] = linkedMapOf(
            "TronAddress21" to linkedMapOf("bytes" to "0x41${"11".repeat(20)}"),
        )
        tronTransfer["route_id"] = canonicalProjectionText("taira_tron_xor")
        SccpJsonParser.parseRecentMessages(
            jsonBytes(linkedMapOf("items" to listOf(tronProjection))),
        )

        val missingProjection = recent(9, MESSAGE_ID).also { it.remove("payload_projection") }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(missingProjection))),
            )
        }
        val nullProjection = recent(9, MESSAGE_ID).also { it["payload_projection"] = null }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(nullProjection))),
            )
        }
        val wrongProjectionRoute = recent(9, MESSAGE_ID)
        @Suppress("UNCHECKED_CAST")
        val projectedTransfer = (wrongProjectionRoute["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
            as MutableMap<String, Any?>
        projectedTransfer["route_id"] = canonicalProjectionText("taira_eth_xor")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(wrongProjectionRoute))),
            )
        }
        val zeroTronRecipient = deepMutableCopy(tronProjection)
        @Suppress("UNCHECKED_CAST")
        val zeroTronTransfer = (zeroTronRecipient["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
            as MutableMap<String, Any?>
        zeroTronTransfer["recipient"] = linkedMapOf(
            "TronAddress21" to linkedMapOf("bytes" to "0x41${"00".repeat(20)}"),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(zeroTronRecipient))),
            )
        }

        val retiredLink = recent(9, MESSAGE_ID)
        @Suppress("UNCHECKED_CAST")
        (retiredLink["links"] as MutableMap<String, Any?>)["artifact_path"] = "/retired"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(jsonBytes(linkedMapOf("items" to listOf(retiredLink))))
        }
        val token = recent(9, MESSAGE_ID).also { it["kind"] = "token_pause" }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(jsonBytes(linkedMapOf("items" to listOf(token))))
        }
        val duplicate = linkedMapOf("items" to listOf(recent(9, MESSAGE_ID), recent(8, MESSAGE_ID)))
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(jsonBytes(duplicate))
        }
    }

    @Test
    fun detachedSigningResponseAcceptsBothClosedBackendsAndRejectsCrossFamilyLabels() {
        val transactionBytes = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
            TransactionPayload(
                chainId = TAIRA_CHAIN_ID,
                authority = authority,
                creationTimeMs = 10,
                executable = Executable.instructions(emptyList()),
                feePayment = FeePaymentIntent.authority(emptyList()),
            ),
        )
        val transaction = Base64.getEncoder().encodeToString(transactionBytes)
        val signing = Base64.getEncoder().encodeToString(IrohaHash.prehash(transactionBytes))
        val response = linkedMapOf<String, Any?>(
            "submitted" to false,
            "payload_kind" to "transfer",
            "message_id_hex" to MESSAGE_ID,
            "backend" to "evm-groth16-bn254-v1",
            "counterparty_domain" to 2,
            "counterparty_chain" to "bsc-mainnet",
            "route_configuration_hash_hex" to hash(0x41),
            "range_start_height" to 9,
            "range_end_height" to 9,
            "creation_time_ms" to 10,
            "tx_hash_hex" to null,
            "transaction_payload_b64" to transaction,
            "signing_message_b64" to signing,
        )
        val parsed = SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        assertFalse(parsed.submitted)
        assertEquals(SccpPayloadKindV1.TRANSFER, parsed.payloadKind)
        assertEquals(hash(0x41), parsed.routeConfigurationHashHex)

        response["backend"] = "tron-groth16-bn254-v1"
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
        response["backend"] = "bridge/sccp/outbound-v1"
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
        response["backend"] = "evm-groth16-bn254-v1"
        response["payload_kind"] = "route_activate"
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
        response["payload_kind"] = "transfer"
        response["creation_time_ms"] = 11
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
        response["creation_time_ms"] = 10
        response["manifest_hash_hex"] = response["route_configuration_hash_hex"]
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
    }

    private fun capabilities(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "registry_revision" to prefixed(0x10),
        "registry_path" to "/v1/sccp/registry",
        "message_bundle_path" to "/v1/sccp/proofs/message/{message_id}",
        "proof_request_path" to "/v1/sccp/proof-requests/{message_id}",
        "recent_messages_path" to "/v1/sccp/messages/recent",
        "registry_limits" to linkedMapOf(
            "max_governed_lanes" to 16,
            "max_live_governed_routes" to 64,
            "max_live_routes_per_lane" to 8,
            "max_retained_routes_per_lane" to 64,
            "max_retained_native_trust_anchors_per_lane" to 4_096,
        ),
        "resource_limits" to linkedMapOf(
            "max_outbound_messages_per_block" to 512,
            "max_outbound_message_payload_bytes" to 4_096,
            "max_pending_outbound_messages" to 65_536,
            "max_pending_outbound_payload_bytes" to 268_435_456,
            "max_proofs_per_transaction" to 1,
            "max_proofs_per_block" to 4,
            "max_proof_bytes_per_proof" to 8 * 1024 * 1024,
            "max_proof_bytes_per_transaction" to 8 * 1024 * 1024,
            "max_proof_bytes_per_block" to 32 * 1024 * 1024,
            "max_native_headers_per_transaction" to 1_004,
            "max_native_headers_per_block" to 4_016,
            "max_ethereum_light_client_updates_per_transaction" to 128,
            "max_ethereum_light_client_updates_per_block" to 512,
            "max_native_header_bytes_per_transaction" to 8 * 1024 * 1024,
            "max_native_header_bytes_per_block" to 32 * 1024 * 1024,
            "max_secp256k1_recoveries_per_transaction" to 1_005,
            "max_secp256k1_recoveries_per_block" to 4_020,
            "max_bls_aggregate_checks_per_transaction" to 1_004,
            "max_bls_aggregate_checks_per_block" to 4_016,
            "max_bls_signer_contributions_per_transaction" to 131_713,
            "max_bls_signer_contributions_per_block" to 526_852,
            "max_bn254_pairing_checks_per_transaction" to 1,
            "max_bn254_pairing_checks_per_block" to 4,
        ),
        "proof_submit_path" to null,
        "native_message_submit_path" to null,
    )

    private fun network(profile: String): MutableMap<String, Any?> =
        linkedMapOf("network" to profile.replace('-', '_'), "profile" to null)

    private fun lane(): MutableMap<String, Any?> =
        linkedMapOf("source" to network("bsc-mainnet"), "target" to network("sora-taira"))

    private fun g1(x: Int = 1, y: Int = 2): MutableMap<String, Any?> =
        linkedMapOf("x" to upper(x, 32), "y" to upper(y, 32))

    private fun g2(seed: Int = 3): MutableMap<String, Any?> = linkedMapOf(
        "x_c0" to upper(seed, 32),
        "x_c1" to upper(seed + 1, 32),
        "y_c0" to upper(seed + 2, 32),
        "y_c1" to upper(seed + 3, 32),
    )

    private fun verifyingKey(): MutableMap<String, Any?> {
        val ic = linkedMapOf<String, Any?>("constant" to g1())
        for (index in 0..10) ic["signal_$index"] = g1()
        return linkedMapOf(
            "version" to 1,
            "alpha1" to g1(),
            "beta2" to g2(),
            "gamma2" to g2(),
            "delta2" to g2(),
            "ic" to ic,
        )
    }

    private fun keyHash(key: Map<String, Any?>): String {
        @Suppress("UNCHECKED_CAST")
        fun point(name: String): Map<String, Any?> = key[name] as Map<String, Any?>
        val words = mutableListOf<String>()
        words += listOf(point("alpha1")["x"] as String, point("alpha1")["y"] as String)
        for (name in listOf("beta2", "gamma2", "delta2")) {
            val value = point(name)
            words += listOf("x_c0", "x_c1", "y_c0", "y_c1").map { value[it] as String }
        }
        @Suppress("UNCHECKED_CAST")
        val ic = key["ic"] as Map<String, Any?>
        for (name in listOf("constant") + (0..10).map { "signal_$it" }) {
            @Suppress("UNCHECKED_CAST")
            val value = ic[name] as Map<String, Any?>
            words += listOf(value["x"] as String, value["y"] as String)
        }
        return keccak(words.joinToString("").hexToBytes()).toUpperHex()
    }

    private fun semanticProfile(): MutableMap<String, Any?> = linkedMapOf(
        "profile" to "sora_taira_finality_inclusion_groth16_bn254",
        "commitments" to linkedMapOf(
            "version" to 1,
            "circuit_commitment" to upper(0x11, 32),
            "witness_generator_commitment" to upper(0x12, 32),
            "public_signal_schema_hash" to publicSignalSchemaHash(),
        ),
    )

    private fun finalityAnchor(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "source_network" to network("sora-taira"),
        "protocol_version" to 3,
        "chain_id_hash" to tairaChainIdHash(),
        "checkpoint_height" to 7,
        "checkpoint_block_hash" to upper(0xa1, 32),
        "checkpoint_context_id" to upper(0xa2, 32),
        "checkpoint_finality_artifact_hash" to upper(0xa3, 32),
    )

    private fun inboundFinalityCutoff(): MutableMap<String, Any?> = linkedMapOf(
        "trust_anchor_hash" to upper(0x61, 32),
        "max_anchor_interval_height" to 8,
    )

    private fun outboundPolicy(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "semantic_profile" to semanticProfile(),
        "sora_finality_anchor" to finalityAnchor(),
    )

    private fun registry(): MutableMap<String, Any?> {
        val key = verifyingKey()
        val routeAddress = upper(0x31, 20)
        val routeCodeHash = upper(0x21, 32)
        val route = linkedMapOf<String, Any?>(
            "lane_id" to lane(),
            "route_id" to "taira_bsc_xor",
            "asset_key" to "xor",
            "revision" to 1,
            "activation" to linkedMapOf("activation" to "staged", "direction" to null),
            "inbound_finality_cutoff" to null,
            "source_identity" to linkedMapOf(
                "lane" to lane(),
                "emitter" to linkedMapOf(
                    "emitter" to "evm",
                    "identity" to linkedMapOf(
                        "address" to routeAddress,
                        "runtime_code_hash" to routeCodeHash,
                        "route_config_hash" to DEFAULT_ROUTE_CONFIG_HASH,
                    ),
                ),
            ),
            "destination" to linkedMapOf(
                "family" to "evm",
                "deployment" to linkedMapOf(
                    "token_address" to upper(0x11, 20),
                    "token_code_hash" to upper(0x23, 32),
                    "verifier_address" to upper(0x12, 20),
                    "verifier_code_hash" to upper(0x24, 32),
                    "verifying_key" to key,
                    "verifier_key_hash" to keyHash(key),
                    "outbound_proof_policy" to outboundPolicy(),
                    "route_address" to routeAddress,
                    "route_code_hash" to routeCodeHash,
                    "taira_to_token_multiplier" to 1_000_000_000,
                ),
            ),
            "settlement" to linkedMapOf(
                "asset_definition_id" to "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                "custody_account_id" to "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                "payload_amount_scale" to 9,
            ),
        )
        assertEquals(DEFAULT_ROUTE_CONFIG_HASH, fixtureRouteConfigurationHash(route))
        return linkedMapOf(
            "version" to 1,
            "lanes" to mutableListOf<Any?>(
                linkedMapOf(
                    "lane_id" to lane(),
                    "native_trust_anchors" to mutableListOf<Any?>(),
                    "current_native_trust_anchor_hash" to null,
                    "routes" to mutableListOf(route),
                ),
            ),
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun tronRegistry(): MutableMap<String, Any?> = registry().also { registry ->
        val route = route(registry)
        val tronLane = linkedMapOf<String, Any?>(
            "source" to network("tron-mainnet"),
            "target" to network("sora-taira"),
        )
        laneRecord(registry)["lane_id"] = deepMutableCopy(tronLane)
        route["lane_id"] = deepMutableCopy(tronLane)
        route["route_id"] = "taira_tron_xor"
        val source = route["source_identity"] as MutableMap<String, Any?>
        source["lane"] = deepMutableCopy(tronLane)
        val emitter = source["emitter"] as MutableMap<String, Any?>
        emitter["emitter"] = "tron"
        val destination = route["destination"] as MutableMap<String, Any?>
        destination["family"] = "tron"
        sourceIdentity(route)["route_config_hash"] = TRON_ROUTE_CONFIG_HASH
        assertEquals(
            TRON_ROUTE_CONFIG_HASH,
            fixtureRouteConfigurationHash(route, SccpNetworkV1.TRON_MAINNET),
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun deployment(registry: MutableMap<String, Any?>): MutableMap<String, Any?> {
        val lanes = registry["lanes"] as MutableList<Any?>
        val lane = lanes[0] as MutableMap<String, Any?>
        val routes = lane["routes"] as MutableList<Any?>
        val route = routes[0] as MutableMap<String, Any?>
        val destination = route["destination"] as MutableMap<String, Any?>
        return destination["deployment"] as MutableMap<String, Any?>
    }

    @Suppress("UNCHECKED_CAST")
    private fun laneRecord(registry: MutableMap<String, Any?>): MutableMap<String, Any?> =
        (registry["lanes"] as MutableList<Any?>)[0] as MutableMap<String, Any?>

    @Suppress("UNCHECKED_CAST")
    private fun routes(registry: MutableMap<String, Any?>): MutableList<Any?> =
        laneRecord(registry)["routes"] as MutableList<Any?>

    @Suppress("UNCHECKED_CAST")
    private fun route(registry: MutableMap<String, Any?>): MutableMap<String, Any?> =
        routes(registry)[0] as MutableMap<String, Any?>

    @Suppress("UNCHECKED_CAST")
    private fun anchors(registry: MutableMap<String, Any?>): MutableList<Any?> =
        laneRecord(registry)["native_trust_anchors"] as MutableList<Any?>

    @Suppress("UNCHECKED_CAST")
    private fun activation(route: MutableMap<String, Any?>): MutableMap<String, Any?> =
        route["activation"] as MutableMap<String, Any?>

    @Suppress("UNCHECKED_CAST")
    private fun sourceIdentity(route: MutableMap<String, Any?>): MutableMap<String, Any?> =
        ((route["source_identity"] as MutableMap<String, Any?>)["emitter"]
            as MutableMap<String, Any?>)["identity"] as MutableMap<String, Any?>

    @Suppress("UNCHECKED_CAST")
    private fun routeDeployment(route: MutableMap<String, Any?>): MutableMap<String, Any?> =
        (route["destination"] as MutableMap<String, Any?>)["deployment"]
            as MutableMap<String, Any?>

    private fun nativeTrustAnchor(
        hashByte: Int,
        checkpointHeight: Int,
        backend: String = "bsc_parlia_v1",
    ): MutableMap<String, Any?> = linkedMapOf(
        "backend" to linkedMapOf("backend" to backend, "protocol" to null),
        "anchor_hash" to upper(hashByte, 32),
        "checkpoint_height" to checkpointHeight,
    )

    @Suppress("UNCHECKED_CAST")
    private fun deepMutableCopy(value: Map<String, Any?>): MutableMap<String, Any?> =
        value.entries.associateTo(linkedMapOf()) { (key, entry) ->
            key to when (entry) {
                is Map<*, *> -> deepMutableCopy(entry as Map<String, Any?>)
                is List<*> -> entry.mapTo(mutableListOf()) { item ->
                    if (item is Map<*, *>) deepMutableCopy(item as Map<String, Any?>) else item
                }
                else -> entry
            }
        }

    private fun refreshRouteConfigurationHash(route: MutableMap<String, Any?>) {
        sourceIdentity(route)["route_config_hash"] = fixtureRouteConfigurationHash(route)
    }

    @Suppress("UNCHECKED_CAST")
    private fun fixtureRouteConfigurationHash(
        route: MutableMap<String, Any?>,
        sourceNetwork: SccpNetworkV1 = SccpNetworkV1.BSC_MAINNET,
    ): String {
        require(
            sourceNetwork == SccpNetworkV1.BSC_MAINNET ||
                sourceNetwork == SccpNetworkV1.TRON_MAINNET,
        ) { "fixture route hash supports only its BSC and TRON mainnet routes" }
        val destination = route["destination"] as MutableMap<String, Any?>
        val deployment = destination["deployment"] as MutableMap<String, Any?>
        val sourceLaneHash = SccpV1.laneHash(
            SccpLaneIdV1(sourceNetwork, SccpNetworkV1.SORA_TAIRA),
        )
        val destinationLaneHash = SccpV1.laneHash(
            SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, sourceNetwork),
        )
        val isTron = sourceNetwork == SccpNetworkV1.TRON_MAINNET
        val networkValue = if (isTron) 0x2b66_53dcL else 56L
        val semanticHash = semanticProfileHash().hexToBytes()
        val anchorHash = finalityAnchorHash().hexToBytes()
        val verifierAddress = deployment["verifier_address"] as String
        val routeAddress = deployment["route_address"] as String
        val verifierCodeHash = (deployment["verifier_code_hash"] as String).hexToBytes()
        val verifierKeyHash = (deployment["verifier_key_hash"] as String).hexToBytes()
        val destinationBindingHash = if (isTron) {
            keccak(
                concatenate(
                    listOf(
                        keccak("iroha:sccp:tron-destination-binding:v1".toByteArray(Charsets.UTF_8)),
                        keccak("tron-groth16-bn254-v1".toByteArray(Charsets.UTF_8)),
                        abiWord(networkValue),
                        abiWord(0),
                        abiWord(sourceNetwork.domainId.toLong()),
                        abiTronAddress(verifierAddress),
                        abiTronAddress(routeAddress),
                        verifierCodeHash,
                        verifierKeyHash,
                        semanticHash,
                        anchorHash,
                    ),
                ),
            )
        } else {
            null
        }
        val deploymentWords = mutableListOf(
            abiAddress(deployment["token_address"] as String),
            (deployment["token_code_hash"] as String).hexToBytes(),
            abiAddress(verifierAddress),
            verifierCodeHash,
            verifierKeyHash,
            semanticHash,
            anchorHash,
        )
        destinationBindingHash?.let(deploymentWords::add)
        val deploymentConfigurationHash = keccak(concatenate(deploymentWords))
        val routeId = route["route_id"] as String
        val revision = (route["revision"] as Number).toLong()
        val multiplier = (deployment["taira_to_token_multiplier"] as Number).toLong()
        val assetRouteConfigurationHash = keccak(
            concatenate(
                listOf(
                    keccak("xor".toByteArray(Charsets.US_ASCII)),
                    keccak(routeId.toByteArray(Charsets.US_ASCII)),
                    abiWord(revision),
                    abiWord(multiplier),
                ),
            ),
        )
        return keccak(
            concatenate(
                listOf(
                    keccak("sccp:concrete-route-config:v1".toByteArray(Charsets.UTF_8)),
                    abiWord(sourceNetwork.domainId.toLong()),
                    abiWord(sourceNetwork.tag.toLong()),
                    abiWord(networkValue),
                    sourceLaneHash,
                    destinationLaneHash,
                    deploymentConfigurationHash,
                    assetRouteConfigurationHash,
                ),
            ),
        ).toUpperHex()
    }

    private fun transferPayload(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "source_domain" to 0,
        "dest_domain" to 2,
        "nonce" to "7",
        "route_revision" to 1,
        "asset_home_domain" to 0,
        "asset_id_codec" to 1,
        "asset_id" to "0x786f72",
        "amount" to "1000",
        "sender_codec" to 1,
        "sender" to "0x616c696365407461697261",
        "recipient_codec" to 2,
        "recipient" to "0x${hash(0x11).take(40)}",
        "route_id_codec" to 1,
        "route_id" to "0x74616972615f6273635f786f72",
    )

    private fun messageBundle(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "commitment_root" to prefixed(0x51),
        "commitment" to linkedMapOf(
            "version" to 1,
            "kind" to "Transfer",
            "context" to linkedMapOf(
                "lane" to linkedMapOf(
                    "source" to network("sora-taira"),
                    "target" to network("bsc-mainnet"),
                ),
                "destination_binding_hash" to prefixed(0x52),
                "route_configuration_hash" to prefixed(0x53),
            ),
            "message_id" to "0x$MESSAGE_ID",
            "payload_hash" to prefixed(0x54),
        ),
        "merkle_proof" to linkedMapOf("steps" to emptyList<Any>()),
        "payload" to linkedMapOf("Transfer" to transferPayload()),
        "finality_proof" to "0x01",
    )

    private fun proofRequest(): MutableMap<String, Any?> {
        val key = verifyingKey()
        return linkedMapOf(
            "version" to 1,
            "backend" to linkedMapOf("backend" to "evm_groth16_bn254_v1", "family" to null),
            "source_network" to network("sora-taira"),
            "target_network" to network("bsc-mainnet"),
            "public_inputs" to linkedMapOf(
                "version" to 1,
                "message_id" to "0x$MESSAGE_ID",
                "payload_hash" to prefixed(0x51),
                "target_domain" to 2,
                "commitment_root" to prefixed(0x52),
                "finality_height" to "9",
                "finality_block_hash" to prefixed(0x53),
            ),
            "verifying_key" to key,
            "verifier_key_hash" to "0x${keyHash(key).lowercase()}",
            "semantic_proof_profile" to semanticProfile(),
            "semantic_proof_profile_hash" to "0x${semanticProfileHash().lowercase()}",
            "sora_finality_anchor" to finalityAnchor(),
            "sora_finality_anchor_hash" to "0x${finalityAnchorHash().lowercase()}",
            "bundle_bytes" to "0x0102",
            "statement_hash" to prefixed(0x63),
            "destination_binding_hash" to prefixed(0x64),
            "route_configuration_hash" to prefixed(0x65),
            "request_hash" to prefixed(0x66),
        )
    }

    private fun recent(
        height: Number,
        id: String,
        commitmentIndex: Int = 0,
    ): MutableMap<String, Any?> = linkedMapOf(
        "height" to height,
        "commitment_index" to commitmentIndex,
        "message_id_hex" to id,
        "kind" to "transfer",
        "source_profile" to "sora-taira",
        "target_profile" to "bsc-mainnet",
        "destination_binding_hash" to prefixed(0x71),
        "route_configuration_hash" to prefixed(0x72),
        "target_domain" to 2,
        "asset_id" to "xor",
        "route_id" to "taira_bsc_xor",
        "recipient" to null,
        "amount" to "1000",
        "payload_projection" to payloadProjection(),
        "links" to linkedMapOf(
            "bundle_path" to "/v1/sccp/proofs/message/$id",
            "proof_request_path" to "/v1/sccp/proof-requests/$id",
        ),
    )

    private fun payloadProjection(): MutableMap<String, Any?> = linkedMapOf(
        "Transfer" to linkedMapOf(
            "version" to 1,
            "source_domain" to 0,
            "dest_domain" to 2,
            "nonce" to 9,
            "route_revision" to 1,
            "asset_home_domain" to 0,
            "asset_id" to canonicalProjectionText("xor"),
            "amount" to 1000,
            "sender" to canonicalProjectionText("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"),
            "recipient" to linkedMapOf(
                "EvmAddress20" to linkedMapOf("bytes" to "0x${"11".repeat(20)}"),
            ),
            "route_id" to canonicalProjectionText("taira_bsc_xor"),
        ),
    )

    private fun canonicalProjectionText(value: String): MutableMap<String, Any?> = linkedMapOf(
        "CanonicalText" to linkedMapOf("value" to value),
    )

    private fun canonicalArtifact(): String =
        Base64.getEncoder().encodeToString(
            canonicalArtifactBytes(SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME),
        )

    private fun canonicalNativeArtifact(): String =
        Base64.getEncoder().encodeToString(
            canonicalArtifactBytes(SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME),
        )

    private fun canonicalArtifactBytes(schemaName: String, padding: Int = 0): ByteArray {
        val schema = SchemaHash.hash16(schemaName)
        val payload = byteArrayOf(1, 2, 3)
        val header = NoritoHeader(
            schema,
            payload.size,
            CRC64.compute(payload),
            NoritoCodec.DEFAULT_FLAGS,
            NoritoHeader.COMPRESSION_NONE,
        )
        return header.encode() + ByteArray(padding) + payload
    }

    private fun jsonBytes(value: Any?): ByteArray =
        JsonEncoder.encode(value).toByteArray(Charsets.UTF_8)

    private class SccpSubmitExecutor(
        private val contentTypes: List<String>,
    ) : HttpTransportExecutor {
        val requests = mutableListOf<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests += request
            val builder = TransportResponse.builder()
                .setStatusCode(200)
                .setBody("{}".toByteArray(Charsets.UTF_8))
            contentTypes.forEach { builder.addHeader("Content-Type", it) }
            return CompletableFuture.completedFuture(builder.build())
        }
    }

    private fun hash(byte: Int): String = byte.toString(16).padStart(2, '0').repeat(32)
    private fun prefixed(byte: Int): String = "0x${hash(byte)}"
    private fun upper(byte: Int, bytes: Int): String =
        byte.toString(16).padStart(2, '0').repeat(bytes).uppercase()

    private fun publicSignalSchemaHash(): String {
        val labels = listOf(
            "sccp:groth16-bn254:signal:message-id:v1",
            "sccp:groth16-bn254:signal:payload-hash:v1",
            "sccp:groth16-bn254:signal:target-domain:v1",
            "sccp:groth16-bn254:signal:commitment-root:v1",
            "sccp:groth16-bn254:signal:finality-height:v1",
            "sccp:groth16-bn254:signal:finality-block-hash:v1",
            "sccp:groth16-bn254:signal:source-domain:v1",
            "sccp:groth16-bn254:signal:statement-hash:v1",
            "sccp:groth16-bn254:signal:destination-binding-hash:v1",
            "sccp:groth16-bn254:signal:route-configuration-hash:v1",
            "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
        )
        val bytes = ByteArrayOutputStream().also { out ->
            out.write(1)
            writeU32(out, labels.size)
            for (label in labels) {
                val value = label.toByteArray(Charsets.UTF_8)
                writeU32(out, value.size)
                out.write(value)
            }
        }.toByteArray()
        return keccak(
            "sccp:groth16-bn254:public-signal-schema:v1".toByteArray(Charsets.UTF_8) + bytes,
        ).toUpperHex()
    }

    private fun tairaChainIdHash(): String = keccak(
        "fc56984b2be7431d840e21514d1883f0".hexToBytes(),
    ).toUpperHex()

    private fun semanticProfileHash(): String = keccak(
        "sccp:semantic-proof-profile:v1".toByteArray(Charsets.UTF_8) +
            byteArrayOf(1, 0, 1) +
            upper(0x11, 32).hexToBytes() +
            upper(0x12, 32).hexToBytes() +
            publicSignalSchemaHash().hexToBytes(),
    ).toUpperHex()

    private fun finalityAnchorHash(protocolVersion: Int = 3): String {
        val canonical = ByteArrayOutputStream().also { output ->
            output.write(1)
            output.write(1)
            writeU16(output, protocolVersion)
            output.write(tairaChainIdHash().hexToBytes())
            writeU64(output, 7)
            output.write(upper(0xa1, 32).hexToBytes())
            output.write(upper(0xa2, 32).hexToBytes())
            output.write(upper(0xa3, 32).hexToBytes())
        }.toByteArray()
        assertEquals(140, canonical.size)
        return keccak(
            "sccp:sora-finality-anchor:v1".toByteArray(Charsets.UTF_8) + canonical,
        ).toUpperHex()
    }

    private fun writeU32(out: ByteArrayOutputStream, value: Int) {
        repeat(4) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
    }

    private fun writeU64(out: ByteArrayOutputStream, value: Long) {
        repeat(8) { shift -> out.write(((value ushr (shift * 8)) and 0xff).toInt()) }
    }

    private fun writeU16(out: ByteArrayOutputStream, value: Int) {
        repeat(2) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
    }

    private fun keccak(value: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(value, 0, value.size)
        return ByteArray(32).also { digest.doFinal(it, 0) }
    }

    private fun concatenate(values: List<ByteArray>): ByteArray =
        ByteArrayOutputStream(values.sumOf { it.size }).also { output ->
            values.forEach { output.write(it) }
        }.toByteArray()

    private fun abiWord(value: Long): ByteArray {
        val encoded = BigInteger.valueOf(value).toByteArray().let {
            if (it.size > 1 && it[0] == 0.toByte()) it.copyOfRange(1, it.size) else it
        }
        return ByteArray(32).also { encoded.copyInto(it, 32 - encoded.size) }
    }

    private fun abiAddress(value: String): ByteArray = ByteArray(12) + value.hexToBytes()

    private fun abiTronAddress(value: String): ByteArray =
        ByteArray(11) + byteArrayOf(0x41) + value.hexToBytes()

    private fun String.hexToBytes(): ByteArray = ByteArray(length / 2) { index ->
        substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }

    private fun ByteArray.toUpperHex(): String = joinToString("") {
        "%02X".format(it.toInt() and 0xff)
    }

    private companion object {
        const val TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
        // These authenticate this fixture's semantic commitments and deployment code hashes.
        const val DEFAULT_ROUTE_CONFIG_HASH =
            "77D2C235AABDFFE9125F27F960FE58F34E9C418BBE452CCC926B10DE18B22BD1"
        const val TRON_ROUTE_CONFIG_HASH =
            "6D14339A4E342F0F5E72947A19133426EAC1DA9F9A01FB15DCAC55078A842AE2"
        val MESSAGE_ID: String = "11".repeat(32)
    }
}
