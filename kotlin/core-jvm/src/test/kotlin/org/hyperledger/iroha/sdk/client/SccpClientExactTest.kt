package org.hyperledger.iroha.sdk.client

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.net.URI
import java.security.MessageDigest
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
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
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.sccp.SccpLaneIdV1
import org.hyperledger.iroha.sdk.sccp.SccpNetworkV1
import org.hyperledger.iroha.sdk.sccp.SccpReplayV1
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
        assertEquals(16 * 1024 * 1024 + 64 * 1024, SCCP_MAX_GROTH16_ARTIFACT_BYTES)
        assertEquals(16 * 1024 * 1024 + 128 * 1024, SCCP_MAX_DESTINATION_ARTIFACT_BYTES)
        assertEquals(22_544_384, SCCP_MAX_DESTINATION_ARTIFACT_BASE64_BYTES)
        assertEquals(
            "iroha_data_model::bridge::BridgeSccpDestinationProofV1",
            SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME,
        )
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
        val firstOuterEnvelopeByte = Base64.getEncoder().encodeToString(
            canonicalArtifactBytesWithTotalSize(
                SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME,
                SCCP_MAX_GROTH16_ARTIFACT_BYTES + 1,
            ),
        )
        val firstOuterEnvelopeRequest = destinationRequest(authority, firstOuterEnvelopeByte)
        HttpClientTransport.preflightSccpBridgeSubmitJson(
            firstOuterEnvelopeRequest.toJsonBytes(),
            "/v1/bridge/proofs/submit",
        )
        val legacyBn254Artifact = Base64.getEncoder().encodeToString(
            canonicalArtifactBytes("iroha_sccp::SccpGroth16Bn254ProofArtifactV1"),
        )
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(authority, legacyBn254Artifact)
        }

        val message = messageRequest(authority, nativeArtifact)
        assertEquals(
            setOf("authority", "fee_payment", "native_proof_b64", "replay_witness_b64"),
            message.toJsonMap().keys,
        )
        HttpClientTransport.preflightSccpBridgeSubmitJson(
            message.toJsonBytes(),
            "/v1/bridge/messages",
        )
        val occupiedWitness = Base64.getEncoder().encodeToString(
            canonicalReplayWitnessBytes(priorRecordDigest = ByteArray(32) { 1 }),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpNativeMessageSubmitRequest(
                authority,
                nativeArtifact,
                occupiedWitness,
                bridgeFeePayment,
            )
        }
        val explicitDefaultWitness = Base64.getEncoder().encodeToString(
            canonicalReplayWitnessBytes(
                siblingBitmap = ByteArray(32).also { it[31] = 1 },
                siblings = listOf(SccpReplayV1.emptyHashes().first()),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            SccpNativeMessageSubmitRequest(
                authority,
                nativeArtifact,
                explicitDefaultWitness,
                bridgeFeePayment,
            )
        }

        val transactionBytes = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
            TransactionPayload(
                networkId = TAIRA_NETWORK_ID,
                authority = authority,
                creationTimeMs = 7,
                executable = Executable.instructions(emptyList()),
                feePayment = FeePaymentIntent.authority(emptyList()),
                admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
            ),
        )
        val transaction = Base64.getEncoder().encodeToString(transactionBytes)
        val gasBoundTransaction = Base64.getEncoder().encodeToString(
            NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
                TransactionPayload(
                networkId = TAIRA_NETWORK_ID,
                    authority = authority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = FeePaymentIntent.authority(emptyList(), 9),
                    admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
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
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority = authority,
                destinationProofB64 = artifact,
                signatureB64 = signature,
                transactionPayloadB64 = transaction,
                creationTimeMs = 7,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            messageRequest(
                authority = authority,
                nativeProofB64 = nativeArtifact,
                signatureB64 = signature,
                transactionPayloadB64 = transaction,
                creationTimeMs = 7,
            )
        }
        val retiredSignedFields = proof.toJsonMap().toMutableMap().also {
            it["signature_b64"] = signature
            it["transaction_payload_b64"] = transaction
            it["creation_time_ms"] = 7
        }
        assertFailsWith<IllegalArgumentException> {
            HttpClientTransport.preflightSccpBridgeSubmitJson(
                jsonBytes(retiredSignedFields),
                "/v1/bridge/proofs/submit",
            )
        }
        val ordinaryTransaction = Base64.getEncoder().encodeToString(
            NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
                NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
                    .decodeTransaction(transactionBytes)
                    .copy(admissionIntent = TransactionAdmissionIntent.ORDINARY),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            destinationRequest(
                authority = authority,
                destinationProofB64 = artifact,
                signatureB64 = signature,
                transactionPayloadB64 = ordinaryTransaction,
                creationTimeMs = 7,
            )
        }

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

        for (removedWrite in listOf("submitSccpDestinationProof", "submitSccpNativeMessage")) {
            assertFalse(
                IrohaClient::class.java.methods.any { it.name == removedWrite },
                "$removedWrite must not expose an unbound prepared-transaction signing surface",
            )
            assertFalse(
                HttpClientTransport::class.java.methods.any { it.name == removedWrite },
                "$removedWrite must not dispatch an SCCP write before exact local proof binding exists",
            )
        }
    }

    @Test
    fun binaryProofRequestAcceptsOnlyTheTwoConcreteCurveTypes() {
        for (schemaName in SCCP_PROOF_REQUEST_SCHEMA_NAMES) {
            val frame = canonicalArtifactBytes(schemaName)
            val executor = SccpNoritoExecutor(frame)
            val transport = HttpClientTransport.withExecutor(
                executor,
                ClientConfig.builder()
                    .setBaseUri(URI.create("https://torii.example"))
                    .build(),
            )
            assertContentEquals(frame, transport.getSccpProofRequestNorito(MESSAGE_ID).join())
            assertEquals(SCCP_MAX_GROTH16_ARTIFACT_BYTES.toLong(), executor.request.maximumResponseBytes)
            assertEquals(listOf("application/x-norito"), executor.request.headers["Accept"])
        }

        val unknown = SccpNoritoExecutor(canonicalArtifactBytes("example::UnknownProofRequestV1"))
        val transport = HttpClientTransport.withExecutor(
            unknown,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )
        assertFailsWith<CompletionException> {
            transport.getSccpProofRequestNorito(MESSAGE_ID).join()
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
                networkId = TAIRA_NETWORK_ID,
                    authority = authority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = feePayment,
                    admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
                ),
            )

        fun signedRequest(feePayment: FeePaymentIntent): SccpDestinationProofSubmitRequest =
            destinationRequest(
                authority,
                canonicalArtifact(),
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

        assertFailsWith<IllegalArgumentException> { signedRequest(expectedFeePayment) }

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
                networkId = TAIRA_NETWORK_ID,
                    authority = authority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = FeePaymentIntent.authority(emptyList()),
                    admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
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
                networkId = TAIRA_NETWORK_ID,
                authority = authority,
                creationTimeMs = 7,
                executable = Executable.instructions(emptyList()),
                feePayment = FeePaymentIntent.authority(emptyList()),
                admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
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
                networkId = TAIRA_NETWORK_ID,
                    authority = otherAuthority,
                    creationTimeMs = 7,
                    executable = Executable.instructions(emptyList()),
                    feePayment = FeePaymentIntent.authority(emptyList()),
                    admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
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
            "max_ed25519_signature_checks_per_transaction",
            "max_ed25519_signature_checks_per_block",
            "max_ed25519_validator_key_checks_per_transaction",
            "max_ed25519_validator_key_checks_per_block",
            "max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block",
            "max_bls12_381_pairing_checks_per_transaction",
            "max_bls12_381_pairing_checks_per_block",
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
            "max_ed25519_signature_checks_per_transaction" to
                "max_ed25519_signature_checks_per_block",
            "max_ed25519_validator_key_checks_per_transaction" to
                "max_ed25519_validator_key_checks_per_block",
            "max_bn254_pairing_checks_per_transaction" to
                "max_bn254_pairing_checks_per_block",
            "max_bls12_381_pairing_checks_per_transaction" to
                "max_bls12_381_pairing_checks_per_block",
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
    ): SccpDestinationProofSubmitRequest {
        require(signatureB64 == null && transactionPayloadB64 == null && creationTimeMs == null) {
            "signed SCCP request fields are not part of the first-release SDK surface"
        }
        return SccpDestinationProofSubmitRequest(authority, destinationProofB64, bridgeFeePayment)
    }

    private fun messageRequest(
        authority: String,
        nativeProofB64: String,
        signatureB64: String? = null,
        transactionPayloadB64: String? = null,
        creationTimeMs: Long? = null,
    ): SccpNativeMessageSubmitRequest {
        require(signatureB64 == null && transactionPayloadB64 == null && creationTimeMs == null) {
            "signed SCCP request fields are not part of the first-release SDK surface"
        }
        return SccpNativeMessageSubmitRequest(
            authority,
            nativeProofB64,
            canonicalReplayWitnessArtifact(),
            bridgeFeePayment,
        )
    }

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

        val noncanonicalWireName = registry()
        @Suppress("UNCHECKED_CAST")
        val noncanonicalSource = (((noncanonicalWireName["lanes"] as MutableList<Any?>)[0]
            as MutableMap<String, Any?>)["lane_id"] as MutableMap<String, Any?>)["source"]
            as MutableMap<String, Any?>
        noncanonicalSource["network"] = "bsc-mainnet"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(noncanonicalWireName))
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

        val replayAddressAlias = registry()
        deployment(replayAddressAlias).let {
            it["replay_verifier_address"] = it["route_address"]
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(replayAddressAlias))
        }

        val replayAddressSubstitution = registry()
        deployment(replayAddressSubstitution)["replay_verifier_address"] = upper(0x73, 20)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(replayAddressSubstitution))
        }

        val replayRuntimeSubstitution = registry()
        deployment(replayRuntimeSubstitution)["replay_verifier_code_hash"] = upper(0x44, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(replayRuntimeSubstitution))
        }

        val breakerAddressSubstitution = registry()
        deployment(breakerAddressSubstitution)["mint_breaker_address"] = upper(0x74, 20)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(breakerAddressSubstitution))
        }

        val breakerRuntimeSubstitution = registry()
        deployment(breakerRuntimeSubstitution)["mint_breaker_code_hash"] = upper(0x45, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(breakerRuntimeSubstitution))
        }

        val swappedRoles = registry()
        deployment(swappedRoles).let {
            val replayAddress = it.getValue("replay_verifier_address")
            val replayCodeHash = it.getValue("replay_verifier_code_hash")
            it["replay_verifier_address"] = it.getValue("mint_breaker_address")
            it["replay_verifier_code_hash"] = it.getValue("mint_breaker_code_hash")
            it["mint_breaker_address"] = replayAddress
            it["mint_breaker_code_hash"] = replayCodeHash
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(swappedRoles))
        }

        val emptyRuntimeHash = registry()
        deployment(emptyRuntimeHash)["mint_breaker_code_hash"] =
            "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(emptyRuntimeHash))
        }

        val zeroCap = registry()
        deployment(zeroCap)["max_wrapped_supply"] = 0
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(zeroCap))
        }

        val missingExecutionPolicy = registry()
        route(missingExecutionPolicy).remove("sora_outbound_execution_policy")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(missingExecutionPolicy))
        }

        val wrongExecutionSemantics = registry()
        @Suppress("UNCHECKED_CAST")
        (route(wrongExecutionSemantics)["sora_outbound_execution_policy"] as MutableMap<String, Any?>)[
            "semantics"
        ] = "unproved_record_sccp_message_v1"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongExecutionSemantics))
        }

        val wrongLiabilityCap = registry()
        @Suppress("UNCHECKED_CAST")
        (route(wrongLiabilityCap)["settlement"] as MutableMap<String, Any?>)[
            "max_outstanding_liability"
        ] = 8
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongLiabilityCap))
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

    @Suppress("UNCHECKED_CAST")
    @Test
    fun tonRegistryAuthenticatesDistinctContractRolesAndExactProofProfile() {
        val canonical = tonRegistry()
        val canonicalRoute = route(canonical)
        val canonicalSource = sourceIdentity(canonicalRoute)
        val canonicalDeployment = routeDeployment(canonicalRoute)
        assertEquals(
            fixtureTonRouteConfigurationHash(canonicalRoute),
            canonicalSource["route_config_hash"],
        )
        assertEquals(
            tonAddressIdentity(canonicalDeployment["route_address"]),
            tonAddressIdentity(canonicalSource["address"]),
        )
        assertEquals(
            canonicalDeployment["route_code_hash"],
            canonicalSource["code_hash"],
        )
        SccpJsonParser.parseRegistry(jsonBytes(canonical))

        val changedInitialData = tonRegistry()
        val changedInitialDataRoute = route(changedInitialData)
        val changedInitialDataDeployment = routeDeployment(changedInitialDataRoute)
        changedInitialDataDeployment["jetton_master_initial_data_hash"] = upper(0x35, 32)
        changedInitialDataDeployment["route_initial_data_hash"] = upper(0x36, 32)
        assertEquals(
            fixtureTonDestinationBindingHash(canonicalDeployment),
            fixtureTonDestinationBindingHash(changedInitialDataDeployment),
        )
        assertEquals(
            canonicalSource["route_config_hash"],
            fixtureTonRouteConfigurationHash(changedInitialDataRoute),
        )
        SccpJsonParser.parseRegistry(jsonBytes(changedInitialData))

        val missingInitialData = tonRegistry()
        routeDeployment(route(missingInitialData)).remove("route_initial_data_hash")
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(missingInitialData))
        }

        val aliasedInitialData = tonRegistry()
        routeDeployment(route(aliasedInitialData)).let {
            it["route_initial_data_hash"] = it["jetton_master_code_hash"]
        }
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(aliasedInitialData))
        }

        val aliasedSource = tonRegistry()
        sourceIdentity(route(aliasedSource))["address"] = deepMutableCopy(
            routeDeployment(route(aliasedSource))["jetton_master_address"] as Map<String, Any?>,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(aliasedSource))
        }

        val aliasedDestination = tonRegistry()
        routeDeployment(route(aliasedDestination))["route_address"] = deepMutableCopy(
            routeDeployment(route(aliasedDestination))["jetton_master_address"] as Map<String, Any?>,
        )
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(aliasedDestination))
        }

        val wrongProfile = tonRegistry()
        @Suppress("UNCHECKED_CAST")
        val semanticProfile = ((routeDeployment(route(wrongProfile))["outbound_proof_policy"]
            as MutableMap<String, Any?>)["semantic_profile"] as MutableMap<String, Any?>)
        semanticProfile["profile"] = "sora_taira_finality_inclusion_groth16_bn254"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(wrongProfile))
        }

        val malformedKey = tonRegistry()
        @Suppress("UNCHECKED_CAST")
        val key = routeDeployment(route(malformedKey))["verifying_key"]
            as MutableMap<String, Any?>
        key["alpha1"] = "40${"00".repeat(47)}"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(malformedKey))
        }

        val unsortedGuardians = tonRegistry()
        val unsorted = routeDeployment(route(unsortedGuardians))["mint_breaker_guardian_keys"]
            as MutableMap<String, Any?>
        unsorted["guardian_1"] = unsorted["guardian_0"]
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(unsortedGuardians))
        }

        val zeroGuardian = tonRegistry()
        val zero = routeDeployment(route(zeroGuardian))["mint_breaker_guardian_keys"]
            as MutableMap<String, Any?>
        zero["guardian_0"] = upper(0, 32)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(zeroGuardian))
        }

        val zeroCap = tonRegistry()
        routeDeployment(route(zeroCap))["max_wrapped_supply"] = 0
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRegistry(jsonBytes(zeroCap))
        }

        requireSccpTonAmountWithinCapV1(BigInteger.valueOf(9), BigInteger.TEN)
        assertFailsWith<IllegalArgumentException> {
            requireSccpTonAmountWithinCapV1(BigInteger.valueOf(11), BigInteger.TEN)
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
        assertEquals(4, request.soraFinalityAnchor.protocolVersion)
        assertEquals(upper(0xa2, 32), request.soraFinalityAnchor.checkpointContextId)
        assertEquals(upper(0xa3, 32), request.soraFinalityAnchor.checkpointFinalityArtifactHash)
        assertEquals(
            "0xcdbec097fed4ad21e44a354fe09a3c43ad489f4ac78cff8944ba8bb5cc2fd577",
            request.soraFinalityAnchor.anchorHash,
        )
        assertEquals("0x${finalityAnchorHash().lowercase()}", request.soraFinalityAnchor.anchorHash)

        val retiredValue = proofRequest()
        @Suppress("UNCHECKED_CAST")
        val retiredAnchor =
            retiredValue["sora_finality_anchor"] as MutableMap<String, Any?>
        retiredAnchor["protocol_version"] = 3
        retiredValue["sora_finality_anchor_hash"] =
            "0x${finalityAnchorHash(3).lowercase()}"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(retiredValue))
        }

        val invalidFinalityAnchors: List<(MutableMap<String, Any?>) -> Unit> = listOf(
            { it["protocol_version"] = 1 },
            { it["protocol_version"] = 3 },
            { it["protocol_version"] = "4" },
            { it["protocol_version"] = 4.0 },
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
    fun tonProofRequestAuthenticatesBls12381SignalsAndExactRoleBinding() {
        val canonical = tonProofRequest()
        val parsed = SccpJsonParser.parseProofRequest(jsonBytes(canonical))
        assertEquals("ton_groth16_bls12381_v1", parsed.backend)
        assertEquals(SccpNetworkV1.TON_MAINNET, parsed.targetNetwork)
        assertEquals(tonPublicSignals(canonical), parsed.publicSignals)
        assertEquals(prefixed(0x25), parsed.verifierCircuitHash)
        assertEquals("0x${tonProofProfileCommitment().lowercase()}", parsed.proofProfileCommitment)

        val changedRole = tonProofRequest()
        changedRole["route_configuration_hash"] = prefixed(0x67)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(changedRole))
        }

        val changedSignal = tonProofRequest()
        @Suppress("UNCHECKED_CAST")
        val publicSignals = changedSignal["public_signals"] as MutableMap<String, Any?>
        publicSignals["route_configuration_hash"] = prefixed(0x68)
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseProofRequest(jsonBytes(changedSignal))
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
            it["target_domain"] = 3
            it["route_id"] = "taira_tron_xor"
        }
        @Suppress("UNCHECKED_CAST")
        val tronTransfer = (tronProjection["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
            as MutableMap<String, Any?>
        tronTransfer["dest_domain"] = 3
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
    fun recentMessagesAcceptOnlyCanonicalTonAccount36Projection() {
        val canonical = tonRecent()
        val parsed = SccpJsonParser.parseRecentMessages(
            jsonBytes(linkedMapOf("items" to listOf(canonical))),
        )
        assertEquals("ton-mainnet", parsed.items.single().targetProfile)
        @Suppress("UNCHECKED_CAST")
        val transfer = parsed.items.single().payloadProjection["Transfer"] as Map<String, Any?>
        @Suppress("UNCHECKED_CAST")
        val recipient = (transfer["recipient"] as Map<String, Any?>)["TonAccount36"]
            as Map<String, Any?>
        assertEquals(0, (recipient["workchain"] as Number).toInt())
        assertEquals("0x${"11".repeat(32)}", recipient["account"])

        val wrongWorkchain = tonRecent()
        @Suppress("UNCHECKED_CAST")
        val wrongWorkchainTransfer =
            (wrongWorkchain["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
                as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        (wrongWorkchainTransfer["recipient"] as MutableMap<String, Any?>)
            .let { it["TonAccount36"] as MutableMap<String, Any?> }["workchain"] = -1
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(wrongWorkchain))),
            )
        }

        val zeroAccount = tonRecent()
        @Suppress("UNCHECKED_CAST")
        val zeroTransfer =
            (zeroAccount["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
                as MutableMap<String, Any?>
        @Suppress("UNCHECKED_CAST")
        (zeroTransfer["recipient"] as MutableMap<String, Any?>)
            .let { it["TonAccount36"] as MutableMap<String, Any?> }["account"] =
            "0x${"00".repeat(32)}"
        assertFailsWith<IllegalArgumentException> {
            SccpJsonParser.parseRecentMessages(
                jsonBytes(linkedMapOf("items" to listOf(zeroAccount))),
            )
        }
    }

    @Test
    fun detachedSigningResponseAcceptsBothClosedBackendsAndRejectsCrossFamilyLabels() {
        val transactionBytes = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1).encodeTransaction(
            TransactionPayload(
                networkId = TAIRA_NETWORK_ID,
                authority = authority,
                creationTimeMs = 10,
                executable = Executable.instructions(emptyList()),
                feePayment = FeePaymentIntent.authority(emptyList()),
                admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
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

        response["submitted"] = true
        response["tx_hash_hex"] = "ab".repeat(32)
        response["transaction_payload_b64"] = null
        response["signing_message_b64"] = null
        assertTrue(SccpBridgeSubmitResponseParser.parse(jsonBytes(response)).submitted)
        response["tx_hash_hex"] = "aa".repeat(32)
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
        response["submitted"] = false
        response["tx_hash_hex"] = null
        response["transaction_payload_b64"] = transaction
        response["signing_message_b64"] = signing

        val ordinaryTransactionBytes = NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
            .encodeTransaction(
                NoritoJavaCodecAdapter(SccpV1.TAIRA_I105_DISCRIMINANT_V1)
                    .decodeTransaction(transactionBytes)
                    .copy(admissionIntent = TransactionAdmissionIntent.ORDINARY),
            )
        response["transaction_payload_b64"] = Base64.getEncoder().encodeToString(ordinaryTransactionBytes)
        response["signing_message_b64"] = Base64.getEncoder().encodeToString(IrohaHash.prehash(ordinaryTransactionBytes))
        assertFailsWith<IllegalArgumentException> {
            SccpBridgeSubmitResponseParser.parse(jsonBytes(response))
        }
        response["transaction_payload_b64"] = transaction
        response["signing_message_b64"] = signing

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
        "sora_outbound_material_path" to "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
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
            "max_ed25519_signature_checks_per_transaction" to 65_536,
            "max_ed25519_signature_checks_per_block" to 262_144,
            "max_ed25519_validator_key_checks_per_transaction" to 198_656,
            "max_ed25519_validator_key_checks_per_block" to 794_624,
            "max_bn254_pairing_checks_per_transaction" to 1,
            "max_bn254_pairing_checks_per_block" to 4,
            "max_bls12_381_pairing_checks_per_transaction" to 1,
            "max_bls12_381_pairing_checks_per_block" to 4,
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
        "protocol_version" to 4,
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

    private fun soraOutboundExecutionPolicy(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "semantics" to "ivm_proved_record_sccp_message_v1",
        "contract_artifact_sha256" to upper(0xb1, 32),
        "vk_ref" to linkedMapOf(
            "backend" to "stark/fri/v1",
            "name" to "ivm-execution-v1",
            "version" to 1,
            "commitment" to upper(0xb2, 32),
        ),
        "gas_limit" to 50_000_000,
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
                    "replay_verifier_address" to upper(0x71, 20),
                    "replay_verifier_code_hash" to upper(0x42, 32),
                    "mint_breaker_address" to upper(0x72, 20),
                    "mint_breaker_code_hash" to upper(0x43, 32),
                    "taira_to_token_multiplier" to 1_000_000_000,
                    "max_wrapped_supply" to 9_000_000_000L,
                ),
            ),
            "sora_outbound_execution_policy" to soraOutboundExecutionPolicy(),
            "settlement" to linkedMapOf(
                "asset_definition_id" to "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                "payload_amount_scale" to 9,
                "max_outstanding_liability" to 9,
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

    private fun tonRegistry(): MutableMap<String, Any?> {
        val tonLane = linkedMapOf<String, Any?>(
            "source" to network("ton-mainnet"),
            "target" to network("sora-taira"),
        )
        val key = bls12381VerifyingKey()
        val route = linkedMapOf<String, Any?>(
            "lane_id" to deepMutableCopy(tonLane),
            "route_id" to "taira_ton_xor",
            "asset_key" to "xor",
            "revision" to 1,
            "activation" to linkedMapOf("activation" to "staged", "direction" to null),
            "inbound_finality_cutoff" to null,
            "source_identity" to linkedMapOf(
                "lane" to deepMutableCopy(tonLane),
                "emitter" to linkedMapOf(
                    "emitter" to "ton",
                    "identity" to linkedMapOf(
                        "address" to tonAddress(0x33),
                        "code_hash" to upper(0x23, 32),
                        "route_config_hash" to upper(0x28, 32),
                    ),
                ),
            ),
            "destination" to linkedMapOf(
                "family" to "ton",
                "deployment" to linkedMapOf(
                    "jetton_master_address" to tonAddress(0x32),
                    "jetton_master_code_hash" to upper(0x21, 32),
                    "jetton_master_initial_data_hash" to upper(0x29, 32),
                    "jetton_wallet_code_hash" to upper(0x22, 32),
                    "route_address" to tonAddress(0x33),
                    "route_code_hash" to upper(0x23, 32),
                    "route_initial_data_hash" to upper(0x2a, 32),
                    "embedded_verifier_code_hash" to upper(0x24, 32),
                    "verifier_circuit_hash" to upper(0x25, 32),
                    "verifying_key" to key,
                    "verifier_key_hash" to bls12381KeyHash(key),
                    "proof_profile_commitment" to tonProofProfileCommitment(),
                    "mint_breaker_guardian_keys" to linkedMapOf(
                        "guardian_0" to upper(1, 32),
                        "guardian_1" to upper(2, 32),
                        "guardian_2" to upper(3, 32),
                        "guardian_3" to upper(4, 32),
                        "guardian_4" to upper(5, 32),
                    ),
                    "outbound_proof_policy" to tonOutboundPolicy(),
                    "taira_to_token_multiplier" to 1,
                    "max_wrapped_supply" to 9_000_000_000L,
                ),
            ),
            "sora_outbound_execution_policy" to soraOutboundExecutionPolicy(),
            "settlement" to linkedMapOf(
                "asset_definition_id" to "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
                "payload_amount_scale" to 9,
                "max_outstanding_liability" to 9_000_000_000L,
            ),
        )
        sourceIdentity(route)["route_config_hash"] = fixtureTonRouteConfigurationHash(route)
        return linkedMapOf(
            "version" to 1,
            "lanes" to mutableListOf<Any?>(
                linkedMapOf(
                    "lane_id" to deepMutableCopy(tonLane),
                    "native_trust_anchors" to mutableListOf<Any?>(),
                    "current_native_trust_anchor_hash" to null,
                    "routes" to mutableListOf(route),
                ),
            ),
        )
    }

    private fun tonAddress(seed: Int): MutableMap<String, Any?> = linkedMapOf(
        "workchain" to 0,
        "account" to upper(seed, 32),
    )

    @Suppress("UNCHECKED_CAST")
    private fun tonAddressIdentity(value: Any?): String {
        val address = value as Map<String, Any?>
        return "${address["workchain"]}:${address["account"]}"
    }

    private fun bls12381G1(seed: Int): String = ByteArray(48).also {
        it[0] = 0x80.toByte()
        it[it.lastIndex] = seed.toByte()
    }.toUpperHex()

    private fun bls12381G2(seed: Int): String = (
        bls12381G1(seed).hexToBytes() + ByteArray(48).also {
            it[it.lastIndex] = (seed + 1).toByte()
        }
    ).toUpperHex()

    private fun bls12381VerifyingKey(): MutableMap<String, Any?> {
        val ic = linkedMapOf<String, Any?>("constant" to bls12381G1(8))
        for (index in 0..10) ic["signal_$index"] = bls12381G1(9 + index)
        return linkedMapOf(
            "version" to 1,
            "alpha1" to bls12381G1(1),
            "beta2" to bls12381G2(2),
            "gamma2" to bls12381G2(4),
            "delta2" to bls12381G2(6),
            "ic" to ic,
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun bls12381KeyHash(key: Map<String, Any?>): String {
        val points = mutableListOf(
            key["alpha1"] as String,
            key["beta2"] as String,
            key["gamma2"] as String,
            key["delta2"] as String,
        )
        val ic = key["ic"] as Map<String, Any?>
        for (field in listOf("constant") + (0..10).map { "signal_$it" }) {
            points += ic[field] as String
        }
        return sha256(byteArrayOf(1) + points.joinToString("").hexToBytes()).toUpperHex()
    }

    private fun tonSemanticProfile(): MutableMap<String, Any?> = linkedMapOf(
        "profile" to "sora_taira_finality_inclusion_groth16_bls12381",
        "commitments" to linkedMapOf(
            "version" to 1,
            "circuit_commitment" to upper(0x25, 32),
            "witness_generator_commitment" to upper(0x26, 32),
            "public_signal_schema_hash" to tonPublicSignalSchemaHash(),
        ),
    )

    private fun tonOutboundPolicy(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "semantic_profile" to tonSemanticProfile(),
        "sora_finality_anchor" to finalityAnchor(),
    )

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
        val replayVerifierAddress = deployment["replay_verifier_address"] as String
        val replayVerifierCodeHash = (deployment["replay_verifier_code_hash"] as String).hexToBytes()
        val mintBreakerAddress = deployment["mint_breaker_address"] as String
        val mintBreakerCodeHash = (deployment["mint_breaker_code_hash"] as String).hexToBytes()
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
                        abiTronAddress(replayVerifierAddress),
                        replayVerifierCodeHash,
                        abiTronAddress(mintBreakerAddress),
                        mintBreakerCodeHash,
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
        deploymentWords.add(abiAddress(replayVerifierAddress))
        deploymentWords.add(replayVerifierCodeHash)
        deploymentWords.add(abiAddress(mintBreakerAddress))
        deploymentWords.add(mintBreakerCodeHash)
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
                    abiWord((deployment["max_wrapped_supply"] as Number).toLong()),
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

    @Suppress("UNCHECKED_CAST")
    private fun fixtureTonRouteConfigurationHash(route: MutableMap<String, Any?>): String {
        val deployment = routeDeployment(route)
        val sourceNetwork = SccpNetworkV1.TON_MAINNET
        val sourceLaneHash = SccpV1.laneHash(
            SccpLaneIdV1(sourceNetwork, SccpNetworkV1.SORA_TAIRA),
        )
        val destinationLaneHash = SccpV1.laneHash(
            SccpLaneIdV1(SccpNetworkV1.SORA_TAIRA, sourceNetwork),
        )
        val binding = fixtureTonDestinationBindingHash(deployment)
        val deploymentConfiguration = ByteArrayOutputStream().also { output ->
            output.write((deployment["jetton_master_code_hash"] as String).hexToBytes())
            output.write((deployment["jetton_wallet_code_hash"] as String).hexToBytes())
            output.write((deployment["route_code_hash"] as String).hexToBytes())
            output.write((deployment["embedded_verifier_code_hash"] as String).hexToBytes())
            output.write((deployment["verifier_circuit_hash"] as String).hexToBytes())
            output.write((deployment["verifier_key_hash"] as String).hexToBytes())
            output.write((deployment["proof_profile_commitment"] as String).hexToBytes())
            guardianKeys(deployment).forEach(output::write)
            output.write(tonSemanticProfileHash().hexToBytes())
            output.write(finalityAnchorHash().hexToBytes())
            output.write(binding.hexToBytes())
        }.toByteArray()
        val assetConfiguration = ByteArrayOutputStream().also { output ->
            writeLengthPrefixed(output, (route["asset_key"] as String).toByteArray(Charsets.US_ASCII))
            writeLengthPrefixed(output, (route["route_id"] as String).toByteArray(Charsets.US_ASCII))
            writeU32(output, (route["revision"] as Number).toInt())
            writeU64(output, (deployment["taira_to_token_multiplier"] as Number).toLong())
            writeU128(output, BigInteger(deployment["max_wrapped_supply"].toString()))
        }.toByteArray()
        val payload = ByteArrayOutputStream().also { output ->
            output.write("sccp:concrete-route-config:v1".toByteArray(Charsets.UTF_8))
            output.write(1)
            writeU32(output, sourceNetwork.domainId)
            writeLengthPrefixed(output, SccpV1.canonicalNetworkBytes(sourceNetwork))
            writeU32(output, -239)
            output.write(sourceLaneHash)
            output.write(destinationLaneHash)
            output.write(sha256(deploymentConfiguration))
            output.write(sha256(assetConfiguration))
        }.toByteArray()
        return sha256(payload).toUpperHex()
    }

    private fun fixtureTonDestinationBindingHash(deployment: Map<String, Any?>): String {
        val payload = ByteArrayOutputStream().also { output ->
            output.write("iroha:sccp:ton-destination-binding:v1".toByteArray(Charsets.UTF_8))
            output.write(1)
            writeLengthPrefixed(
                output,
                "ton-groth16-bls12381-v1".toByteArray(Charsets.US_ASCII),
            )
            writeLengthPrefixed(output, SccpV1.canonicalNetworkBytes(SccpNetworkV1.TON_MAINNET))
            writeU32(output, -239)
            writeU32(output, 0)
            writeU32(output, 4)
            output.write((deployment["jetton_master_code_hash"] as String).hexToBytes())
            output.write((deployment["jetton_wallet_code_hash"] as String).hexToBytes())
            output.write((deployment["route_code_hash"] as String).hexToBytes())
            output.write((deployment["embedded_verifier_code_hash"] as String).hexToBytes())
            output.write((deployment["verifier_circuit_hash"] as String).hexToBytes())
            output.write((deployment["verifier_key_hash"] as String).hexToBytes())
            output.write((deployment["proof_profile_commitment"] as String).hexToBytes())
            guardianKeys(deployment).forEach(output::write)
            output.write(tonSemanticProfileHash().hexToBytes())
            output.write(finalityAnchorHash().hexToBytes())
        }.toByteArray()
        return sha256(payload).toUpperHex()
    }

    @Suppress("UNCHECKED_CAST")
    private fun guardianKeys(deployment: Map<String, Any?>): List<ByteArray> {
        val guardians = deployment["mint_breaker_guardian_keys"] as Map<String, Any?>
        return (0..4).map { (guardians["guardian_$it"] as String).hexToBytes() }
    }

    private fun transferPayload(): MutableMap<String, Any?> = linkedMapOf(
        "version" to 1,
        "source_domain" to 0,
        "dest_domain" to 2,
        "nonce" to "7",
        "route_revision" to 1,
        "asset_home_domain" to 0,
        "asset_id_codec" to 0,
        "asset_id" to "0x786f72",
        "amount" to "1000",
        "sender_codec" to 0,
        "sender" to "0x616c696365407461697261",
        "recipient_codec" to 1,
        "recipient" to "0x${hash(0x11).take(40)}",
        "route_id_codec" to 0,
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

    private fun tonProofRequest(): MutableMap<String, Any?> {
        val key = bls12381VerifyingKey()
        val request = linkedMapOf<String, Any?>(
            "version" to 1,
            "backend" to linkedMapOf(
                "backend" to "ton_groth16_bls12381_v1",
                "family" to null,
            ),
            "source_network" to network("sora-taira"),
            "target_network" to network("ton-mainnet"),
            "public_inputs" to linkedMapOf(
                "version" to 1,
                "message_id" to "0x$MESSAGE_ID",
                "payload_hash" to prefixed(0x51),
                "target_domain" to 4,
                "commitment_root" to prefixed(0x52),
                "finality_height" to "9",
                "finality_block_hash" to prefixed(0x53),
            ),
            "verifying_key" to key,
            "verifier_key_hash" to "0x${bls12381KeyHash(key).lowercase()}",
            "semantic_proof_profile" to tonSemanticProfile(),
            "semantic_proof_profile_hash" to "0x${tonSemanticProfileHash().lowercase()}",
            "sora_finality_anchor" to finalityAnchor(),
            "sora_finality_anchor_hash" to "0x${finalityAnchorHash().lowercase()}",
            "bundle_bytes" to "0x0102",
            "statement_hash" to prefixed(0x63),
            "destination_binding_hash" to prefixed(0x64),
            "route_configuration_hash" to prefixed(0x65),
            "request_hash" to prefixed(0x66),
            "verifier_circuit_hash" to prefixed(0x25),
            "proof_profile_commitment" to "0x${tonProofProfileCommitment().lowercase()}",
        )
        request["public_signals"] = tonPublicSignals(request)
        return request
    }

    @Suppress("UNCHECKED_CAST")
    private fun tonPublicSignals(request: Map<String, Any?>): MutableMap<String, String> {
        val inputs = request["public_inputs"] as Map<String, Any?>
        val words = listOf(
            (inputs["message_id"] as String).removePrefix("0x").hexToBytes(),
            (inputs["payload_hash"] as String).removePrefix("0x").hexToBytes(),
            abiWord((inputs["target_domain"] as Number).toLong()),
            (inputs["commitment_root"] as String).removePrefix("0x").hexToBytes(),
            abiWord((inputs["finality_height"] as String).toLong()),
            (inputs["finality_block_hash"] as String).removePrefix("0x").hexToBytes(),
            abiWord(0),
            (request["statement_hash"] as String).removePrefix("0x").hexToBytes(),
            (request["destination_binding_hash"] as String).removePrefix("0x").hexToBytes(),
            (request["route_configuration_hash"] as String).removePrefix("0x").hexToBytes(),
            (request["sora_finality_anchor_hash"] as String).removePrefix("0x").hexToBytes(),
        )
        return TON_PUBLIC_SIGNAL_FIELDS.zip(TON_PUBLIC_SIGNAL_LABELS.zip(words))
            .associateTo(linkedMapOf()) { (field, labelAndWord) ->
                val (label, word) = labelAndWord
                val labelHash = sha256(label.toByteArray(Charsets.UTF_8))
                val scalar = BigInteger(1, sha256(labelHash + word)).mod(BLS12381_SCALAR_MODULUS)
                field to "0x${scalar.toFixedUnsigned(32).toUpperHex().lowercase()}"
            }
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

    @Suppress("UNCHECKED_CAST")
    private fun tonRecent(): MutableMap<String, Any?> = recent(9, MESSAGE_ID).also { recent ->
        recent["target_profile"] = "ton-mainnet"
        recent["target_domain"] = 4
        recent["route_id"] = "taira_ton_xor"
        val transfer = (recent["payload_projection"] as MutableMap<String, Any?>)["Transfer"]
            as MutableMap<String, Any?>
        transfer["dest_domain"] = 4
        transfer["recipient"] = linkedMapOf(
            "TonAccount36" to linkedMapOf(
                "workchain" to 0,
                "account" to "0x${"11".repeat(32)}",
            ),
        )
        transfer["route_id"] = canonicalProjectionText("taira_ton_xor")
    }

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

    private fun canonicalReplayWitnessArtifact(): String =
        Base64.getEncoder().encodeToString(
            canonicalReplayWitnessBytes(),
        )

    private fun canonicalReplayWitnessBytes(
        priorRecordDigest: ByteArray = ByteArray(32),
        siblingBitmap: ByteArray = ByteArray(32),
        siblings: List<ByteArray> = emptyList(),
    ): ByteArray {
        val siblingSequence = ByteArrayOutputStream().also { output ->
            writeU64(output, siblings.size.toLong())
            siblings.forEach { writeCompactField(output, it) }
        }.toByteArray()
        val payload = ByteArrayOutputStream().also { output ->
            writeCompactField(output, SccpReplayV1.emptyHashes().last())
            writeCompactField(output, priorRecordDigest)
            writeCompactField(output, siblingBitmap)
            writeCompactField(output, siblingSequence)
        }.toByteArray()
        val header = NoritoHeader(
            SchemaHash.hash16(SCCP_REPLAY_WITNESS_SCHEMA_NAME),
            payload.size,
            CRC64.compute(payload),
            NoritoCodec.DEFAULT_FLAGS,
            NoritoHeader.COMPRESSION_NONE,
        )
        return header.encode() + payload
    }

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

    private fun canonicalArtifactBytesWithTotalSize(
        schemaName: String,
        totalSize: Int,
    ): ByteArray {
        require(totalSize >= NoritoHeader.HEADER_LENGTH)
        val payload = ByteArray(totalSize - NoritoHeader.HEADER_LENGTH) { 0x5a }
        val header = NoritoHeader(
            SchemaHash.hash16(schemaName),
            payload.size,
            CRC64.compute(payload),
            NoritoCodec.DEFAULT_FLAGS,
            NoritoHeader.COMPRESSION_NONE,
        ).encode()
        return ByteArray(totalSize).also { output ->
            header.copyInto(output)
            payload.copyInto(output, NoritoHeader.HEADER_LENGTH)
        }
    }

    private fun jsonBytes(value: Any?): ByteArray =
        JsonEncoder.encode(value).toByteArray(Charsets.UTF_8)

    private class SccpNoritoExecutor(
        private val body: ByteArray,
    ) : HttpTransportExecutor {
        lateinit var request: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            this.request = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(body)
                    .addHeader("Content-Type", "application/x-norito")
                    .build(),
            )
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

    private fun tonPublicSignalSchemaHash(): String {
        val canonical = ByteArrayOutputStream().also { output ->
            output.write(1)
            writeU32(output, TON_PUBLIC_SIGNAL_LABELS.size)
            TON_PUBLIC_SIGNAL_LABELS.forEach { label ->
                writeLengthPrefixed(output, label.toByteArray(Charsets.UTF_8))
            }
        }.toByteArray()
        return sha256(
            "sccp:groth16-bls12381:public-signal-schema:v1".toByteArray(Charsets.UTF_8) +
                canonical,
        ).toUpperHex()
    }

    private fun tonProofProfileCommitment(): String = sha256(
        "sccp:ton:groth16-bls12381:proof-profile:v1".toByteArray(Charsets.UTF_8) +
            byteArrayOf(1) +
            "ietf-bls12381-compressed-g1-48-g2-96".toByteArray(Charsets.US_ASCII) +
            "groth16-a-g1-b-g2-c-g1".toByteArray(Charsets.US_ASCII) +
            "sha256-sha256-label-value-mod-r".toByteArray(Charsets.US_ASCII) +
            BLS12381_SCALAR_MODULUS.toFixedUnsigned(32) +
            tonPublicSignalSchemaHash().hexToBytes(),
    ).toUpperHex()

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

    private fun tonSemanticProfileHash(): String = keccak(
        "sccp:semantic-proof-profile:v1".toByteArray(Charsets.UTF_8) +
            byteArrayOf(1, 1, 1) +
            upper(0x25, 32).hexToBytes() +
            upper(0x26, 32).hexToBytes() +
            tonPublicSignalSchemaHash().hexToBytes(),
    ).toUpperHex()

    private fun finalityAnchorHash(protocolVersion: Int = 4): String {
        val canonical = ByteArrayOutputStream().also { output ->
            output.write(1)
            output.write(SccpNetworkV1.SORA_TAIRA.tag)
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

    private fun writeLengthPrefixed(out: ByteArrayOutputStream, value: ByteArray) {
        writeU32(out, value.size)
        out.write(value)
    }

    private fun writeCompactField(out: ByteArrayOutputStream, value: ByteArray) {
        var remaining = value.size
        do {
            var next = remaining and 0x7f
            remaining = remaining ushr 7
            if (remaining != 0) next = next or 0x80
            out.write(next)
        } while (remaining != 0)
        out.write(value)
    }

    private fun writeU64(out: ByteArrayOutputStream, value: Long) {
        repeat(8) { shift -> out.write(((value ushr (shift * 8)) and 0xff).toInt()) }
    }

    private fun writeU128(out: ByteArrayOutputStream, value: BigInteger) {
        repeat(16) { shift ->
            out.write(value.shiftRight(shift * 8).and(BigInteger.valueOf(0xff)).toInt())
        }
    }

    private fun writeU16(out: ByteArrayOutputStream, value: Int) {
        repeat(2) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
    }

    private fun keccak(value: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(value, 0, value.size)
        return ByteArray(32).also { digest.doFinal(it, 0) }
    }

    private fun sha256(value: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(value)

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

    private fun BigInteger.toFixedUnsigned(size: Int): ByteArray {
        val source = toByteArray().let {
            if (it.size > 1 && it[0] == 0.toByte()) it.copyOfRange(1, it.size) else it
        }
        require(source.size <= size)
        return ByteArray(size).also { source.copyInto(it, size - source.size) }
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
        val TAIRA_NETWORK_ID = NetworkId.parse(
            "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94",
        )
        // These authenticate this fixture's semantic commitments and deployment code hashes.
        const val DEFAULT_ROUTE_CONFIG_HASH =
            "FDCE93E148D8A9BD3BE2E7051AF681A757CA273F409073F9402F5534D32C399B"
        const val TRON_ROUTE_CONFIG_HASH =
            "09091FF86A7F8E94B2EE53A398EF9CAC12346522457C3B466F3CA4ED4EF2DB70"
        val BLS12381_SCALAR_MODULUS = BigInteger(
            "73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001",
            16,
        )
        val TON_PUBLIC_SIGNAL_LABELS = listOf(
            "sccp:groth16-bls12381:signal:message-id:v1",
            "sccp:groth16-bls12381:signal:payload-hash:v1",
            "sccp:groth16-bls12381:signal:target-domain:v1",
            "sccp:groth16-bls12381:signal:commitment-root:v1",
            "sccp:groth16-bls12381:signal:finality-height:v1",
            "sccp:groth16-bls12381:signal:finality-block-hash:v1",
            "sccp:groth16-bls12381:signal:source-domain:v1",
            "sccp:groth16-bls12381:signal:statement-hash:v1",
            "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
            "sccp:groth16-bls12381:signal:route-config-hash:v1",
            "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
        )
        val TON_PUBLIC_SIGNAL_FIELDS = listOf(
            "message_id",
            "payload_hash",
            "target_domain",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "source_domain",
            "statement_hash",
            "destination_binding_hash",
            "route_configuration_hash",
            "sora_finality_anchor_hash",
        )
        val MESSAGE_ID: String = "11".repeat(32)
    }
}
