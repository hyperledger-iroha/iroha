package org.hyperledger.iroha.sdk.client

import java.io.File
import java.math.BigInteger
import java.net.URI
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class DaToriiClientTest {
    private val keyPair by lazy { KeyPairGenerator.getInstance("Ed25519").generateKeyPair() }
    private val proofSigner by lazy {
        val publicKey = keyPair.public.encoded.takeLast(32).toByteArray()
        ToriiCanonicalRequestAuth(AccountAddress.fromAccount(publicKey, "ed25519").toI105Default(), RequestSigner.ed25519(keyPair.private))
    }
    private val proofNetwork = NetworkId.parse(HASH)

    private fun assertSignedProofRequest(request: TransportRequest) {
        fun header(name: String) = requireNotNull(request.headers[name]?.singleOrNull())
        assertEquals(AccountAddress.fromI105(proofSigner.accountId, null).canonicalHex(), header(CanonicalRequestSigner.HEADER_ACCOUNT))
        val message = CanonicalRequestSigner.canonicalRequestSignatureMessage(
            proofNetwork, request.method, request.uri, request.body,
            header(CanonicalRequestSigner.HEADER_TIMESTAMP_MS).toLong(), header(CanonicalRequestSigner.HEADER_NONCE),
        )
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(keyPair.public)
        verifier.update(message)
        assertTrue(verifier.verify(Base64.getDecoder().decode(header(CanonicalRequestSigner.HEADER_SIGNATURE))))
    }

    @Test
    fun proofRequiresSigningContextBeforeDispatch() {
        val executor = CapturingDaExecutor("null")
        val unsigned = DaToriiClient.builder().executor(executor).baseUri(URI.create("https://example.com")).build()
        assertFailsWith<IllegalStateException> { unsigned.provePinIntent() }
        assertFailsWith<IllegalStateException> { unsigned.proveCommitment() }
    }

    @Test
    fun listCommitmentsUsesSnapshotCursorAndTypedResponse() {
        val executor = CapturingDaExecutor(
            """
            {
              "policies":{"version":1,"policy_hash":"$HASH","policies":[]},
              "commitments":[],
              "next_cursor":{
                "snapshot":{"block_height":18446744073709551615,"block_hash":"$HASH"},
                "after":{"lane_id":7,"epoch":18446744073709551615,"sequence":9}
              }
            }
            """.trimIndent(),
        )
        val client = client(executor)

        val snapshot = DaModels.ListSnapshot(
            blockHeight = BigInteger("18446744073709551615"),
            blockHash = HASH,
        )
        val response = client.listCommitments(
            DaModels.CommitmentListRequest(
                limit = BigInteger("18446744073709551615"),
                cursor = DaModels.CommitmentListCursor(
                    snapshot,
                    DaModels.CommitmentKey(
                        laneId = 7,
                        epoch = BigInteger("18446744073709551615"),
                        sequence = BigInteger.valueOf(9),
                    ),
                ),
            ),
        ).join()

        assertEquals(8L * 1024 * 1024, executor.lastRequest.maximumResponseBytes)
        assertEquals("POST", executor.lastRequest.method)
        assertEquals("/v1/da/commitments", executor.lastRequest.uri.path)
        val request = DaJson.parse(executor.lastRequest.body, "request") as Map<*, *>
        assertEquals(setOf("cursor", "limit"), request.keys)
        assertEquals(BigInteger("18446744073709551615"), request["limit"])
        val cursor = request["cursor"] as Map<*, *>
        val encodedSnapshot = cursor["snapshot"] as Map<*, *>
        assertEquals(BigInteger("18446744073709551615"), encodedSnapshot["block_height"])
        assertEquals(HASH, encodedSnapshot["block_hash"])
        val after = cursor["after"] as Map<*, *>
        assertEquals(BigInteger("18446744073709551615"), after["epoch"])
        assertEquals(1, response.policies.version)
        assertEquals(HASH, response.policies.policyHash)
        assertTrue(response.commitments.isEmpty())
        assertEquals(snapshot, response.nextCursor?.snapshot)
        assertEquals(BigInteger("18446744073709551615"), response.nextCursor?.after?.epoch)
    }

    @Test
    fun proveAndVerifyPinIntentPreserveUnsignedIntegersAndProofShape() {
        val proofJson = pinIntentProofJson()
        val proveExecutor = CapturingDaExecutor(proofJson)
        val proof = client(proveExecutor).provePinIntent(
            DaModels.PinIntentQueryRequest(
                storageTicket = DaModels.Digest32.fromHex("22".repeat(32)),
            ),
        ).join()

        requireNotNull(proof)
        val proveRequest = DaJson.parse(proveExecutor.lastRequest.body, "request") as Map<*, *>
        assertEquals(setOf("storage_ticket"), proveRequest.keys)
        assertEquals(BigInteger.valueOf(42), proof.intent.epoch)
        assertEquals(2, proof.bundleLength)
        assertEquals(DaModels.MerkleDirection.RIGHT, proof.path.single().direction)

        val verifyExecutor = CapturingDaExecutor("""{"valid":true,"error":null}""")
        val response = client(verifyExecutor).verifyPinIntent(proof).join()
        assertSignedProofRequest(proveExecutor.lastRequest)
        assertSignedProofRequest(verifyExecutor.lastRequest)
        assertTrue(response.valid)
        assertNull(response.error)
        assertEquals("/v1/da/pin-intents/verify", verifyExecutor.lastRequest.uri.path)
        val posted = DaJson.parse(verifyExecutor.lastRequest.body, "request") as Map<*, *>
        val intent = posted["intent"] as Map<*, *>
        assertEquals(DaJson.parse(signedIntentJson().toByteArray(), "signed intent"), intent)
        assertTrue(intent.containsKey("alias"))
        assertNull(intent["alias"])
        assertTrue(intent.containsKey("authorization"))
        assertTrue(intent.containsKey("pin_scope_authorization"))
        val direction = ((posted["path"] as List<*>)[0] as Map<*, *>)["direction"] as Map<*, *>
        assertEquals("Right", direction["direction"])
        assertTrue(direction.containsKey("value"))
        assertNull(direction["value"])
    }

    @Test
    fun proveReturnsNullForMissingRecord() {
        val executor = CapturingDaExecutor("null")
        assertNull(
            client(executor).proveCommitment(
                DaModels.CommitmentProofRequest(
                    manifestHash = DaModels.Digest32.fromHex("11".repeat(32)),
                    laneId = 7,
                    epoch = BigInteger("18446744073709551615"),
                    sequence = BigInteger.ONE,
                ),
            ).join(),
        )
        assertEquals("/v1/da/commitments/prove", executor.lastRequest.uri.path)
        val request = DaJson.parse(executor.lastRequest.body, "request") as Map<*, *>
        assertEquals(setOf("manifest_hash", "lane_id", "epoch", "sequence"), request.keys)
        assertTrue("limit" !in request)
        assertTrue("cursor" !in request)
        assertTrue("pagination" !in request)
    }

    @Test
    fun listPinIntentsUsesLocationCursorAndRequiresResponseEnvelope() {
        val executor = CapturingDaExecutor(
            """
            {
              "intents":[],
              "next_cursor":{
                "snapshot":{"block_height":10,"block_hash":"$HASH"},
                "after":{"block_height":9,"index_in_bundle":4294967295}
              }
            }
            """.trimIndent(),
        )
        val request = DaModels.PinIntentListRequest(
            limit = BigInteger.valueOf(5),
            cursor = DaModels.PinIntentListCursor(
                DaModels.ListSnapshot(BigInteger.TEN, HASH),
                DaModels.Location(BigInteger.valueOf(9), 4_294_967_295L),
            ),
        )

        val response = client(executor).listPinIntents(request).join()

        assertTrue(response.intents.isEmpty())
        assertEquals(BigInteger.TEN, response.nextCursor?.snapshot?.blockHeight)
        assertEquals(4_294_967_295L, response.nextCursor?.after?.indexInBundle)
        val posted = DaJson.parse(executor.lastRequest.body, "request") as Map<*, *>
        assertEquals(setOf("cursor", "limit"), posted.keys)
        val cursor = posted["cursor"] as Map<*, *>
        assertEquals(
            9L,
            (cursor["after"] as Map<*, *>)["block_height"],
        )

        val finalPage = client(
            CapturingDaExecutor("""{"intents":[],"next_cursor":null}"""),
        ).listPinIntents().join()
        assertTrue(finalPage.intents.isEmpty())
        assertNull(finalPage.nextCursor)

        val legacy = CapturingDaExecutor("[]")
        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            client(legacy).listPinIntents().join()
        }
        assertTrue(error.cause is DaToriiException)
    }

    @Test
    fun proofParserRejectsLegacyLocationOnlyPayloadAndMalformedTags() {
        for (unsupported in listOf("MerkleSha256", "KzgBls12_381")) {
            assertFailsWith<IllegalArgumentException> {
                DaModels.ProofScheme.fromJson(mapOf("type" to unsupported, "value" to null), "proof_scheme")
            }
        }

        assertFailsWith<IllegalArgumentException> {
            DaJson.parsePinIntentProof(
                DaJson.parse(
                    """{"intent":{},"location":{"block_height":1,"index_in_bundle":0}}"""
                        .toByteArray(StandardCharsets.UTF_8),
                    "proof",
                ),
                "proof",
            )
        }

        val malformed = pinIntentProofJson().replace(
            """"direction":{"direction":"Right","value":null}""",
            """"direction":"Right"""",
        )
        assertFailsWith<IllegalArgumentException> {
            DaJson.parsePinIntentProof(
                DaJson.parse(malformed.toByteArray(StandardCharsets.UTF_8), "proof"),
                "proof",
            )
        }

        val inconsistentPath = pinIntentProofJson().replace(
            """"bundle_len":2""",
            """"bundle_len":1""",
        )
        assertFailsWith<IllegalArgumentException> {
            DaJson.parsePinIntentProof(
                DaJson.parse(inconsistentPath.toByteArray(StandardCharsets.UTF_8), "proof"),
                "proof",
            )
        }

        val badChecksum = pinIntentProofJson().replace(HASH, HASH.dropLast(1) + "A")
        assertFailsWith<IllegalArgumentException> {
            DaJson.parsePinIntentProof(
                DaJson.parse(badChecksum.toByteArray(StandardCharsets.UTF_8), "proof"),
                "proof",
            )
        }

        assertFailsWith<IllegalArgumentException> {
            DaJson.parseCommitmentList(
                DaJson.parse(
                    """
                    {
                      "policies":{"version":1,"policy_hash":"$HASH","policies":[]},
                      "commitments":[]
                    }
                    """.trimIndent().toByteArray(StandardCharsets.UTF_8),
                    "response",
                ),
                "response",
            )
        }

        assertFailsWith<IllegalArgumentException> {
            DaJson.parsePinIntentList(
                DaJson.parse(
                    """
                    {
                      "intents":[],
                      "next_cursor":{
                        "snapshot":{"block_height":10},
                        "after":{"block_height":9,"index_in_bundle":0}
                      }
                    }
                    """.trimIndent().toByteArray(StandardCharsets.UTF_8),
                    "response",
                ),
                "response",
            )
        }
    }

    @Test
    fun pinIntentAliasesUseServerUtf8ByteBound() {
        DaModels.PinIntentQueryRequest(alias = "")
        DaModels.PinIntentQueryRequest(alias = "é".repeat(128))
        DaModels.RetentionPolicy(
            hotRetentionSeconds = BigInteger.ZERO,
            coldRetentionSeconds = BigInteger.ZERO,
            requiredReplicas = 0,
            storageClass = DaModels.StorageClass.HOT,
            governanceTag = "",
        )
        assertFailsWith<IllegalArgumentException> {
            DaModels.PinIntentQueryRequest(alias = "é".repeat(129))
        }
        assertFailsWith<IllegalArgumentException> {
            DaModels.CommitmentListRequest(limit = BigInteger.ZERO)
        }
        assertFailsWith<IllegalArgumentException> {
            DaModels.PinIntentListRequest(
                cursor = DaModels.PinIntentListCursor(
                    DaModels.ListSnapshot(BigInteger.ONE, null),
                    DaModels.Location(BigInteger.ONE, 0),
                ),
            )
        }
    }

    @Test
    fun verifyResponseRejectsContradictoryValidityAndError() {
        val executor = CapturingDaExecutor("""{"valid":true,"error":"forged"}""")
        val proof = DaJson.parsePinIntentProof(
            DaJson.parse(pinIntentProofJson().toByteArray(StandardCharsets.UTF_8), "proof"),
            "proof",
        )
        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            client(executor).verifyPinIntent(requireNotNull(proof)).join()
        }
        assertTrue(error.cause is DaToriiException)
    }

    @Test
    fun observerErrorsCompleteTheReturnedFuture() {
        val executor = CapturingDaExecutor(
            """{"version":1,"policy_hash":"$HASH","policies":[]}""",
        )
        val client = DaToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .addObserver(object : ClientObserver {
                override fun onResponse(request: TransportRequest, response: ClientResponse) {
                    throw AssertionError("observer failed")
                }
            })
            .build()
        val error = assertFailsWith<java.util.concurrent.CompletionException> {
            client.getProofPolicies().join()
        }
        assertTrue(error.cause is AssertionError)
    }

    @Test
    fun commitmentProofPreservesRawEvenRootAndVariableLengthOperatorSignature() {
        val digest = DaModels.Digest32.fromBytes(ByteArray(32) { 0x10 })
        val retention = DaModels.RetentionPolicy(BigInteger.ONE, BigInteger.TEN, 1, DaModels.StorageClass.HOT, "da")
        val signature = "5A".repeat(3309)
        val record = DaModels.CommitmentRecord(1, BigInteger.ONE, BigInteger.ONE, digest, digest,
            DaModels.ProofScheme.SORAFS_POR_BLAKE3, digest, null, retention, digest, signature)
        fun proof(root: Any) = mapOf(
            "commitment" to (record.toJsonValue() + ("chunk_root" to root)),
            "location" to mapOf("block_height" to 1, "index_in_bundle" to 0),
            "bundle_hash" to HASH, "bundle_len" to 1, "root" to HASH, "path" to emptyList<Any>(),
        )
        val parsed = DaJson.parseCommitmentProof(proof(digest.toJsonValue()), "proof")
        assertEquals(digest, parsed.commitment.chunkRoot)
        assertEquals(signature, parsed.commitment.acknowledgementSignature)
        assertFailsWith<IllegalArgumentException> { DaJson.parseCommitmentProof(proof(HASH), "proof") }
    }

    @Test
    fun pinProofRejectsRepeatedControllersAndOversizedControllerSignatures() {
        val proof = requireNotNull(DaJson.parsePinIntentProof(
            DaJson.parse(pinIntentProofJson().toByteArray(), "proof"), "proof",
        ))
        val original = proof.intent.authorization
        val witness = original.signatures.single()
        assertFailsWith<IllegalArgumentException> { original.copy(signatures = listOf(witness, witness)) }
        assertFailsWith<IllegalArgumentException> {
            proof.intent.pinScopeAuthorization.copy(signatures = listOf(witness, witness))
        }
        assertFailsWith<IllegalArgumentException> { witness.copy(signature = "AB".repeat(4097)) }
    }

    private fun client(executor: HttpTransportExecutor): DaToriiClient =
        DaToriiClient.builder()
            .executor(executor)
            .authenticatedProofs(LocalSigningContext(proofNetwork), proofSigner)
            .baseUri(URI.create("https://example.com"))
            .build()

    private fun signedIntentJson(): String = generateSequence(File(".").canonicalFile) { it.parentFile }
        .map { File(it, "fixtures/da/ingest/pin_intent_v1.json") }.first(File::isFile).readText()

    private fun pinIntentProofJson(): String {
        return """
            {
              "intent":${signedIntentJson()},
              "location":{"block_height":10,"index_in_bundle":0},
              "bundle_hash":"$HASH",
              "bundle_len":2,
              "root":"$HASH",
              "path":[{
                "sibling":"$HASH",
                "direction":{"direction":"Right","value":null}
              }]
            }
        """.trimIndent()
    }

    @Test
    fun pinProofRetainsFullUnsignedEpochAndRejectsRemovedOwnerOrDifferentScope() {
        fun parse(text: String) = DaJson.parsePinIntentProof(DaJson.parse(text.toByteArray(), "proof"), "proof")
        val wire = pinIntentProofJson()
        val maximum = BigInteger("18446744073709551615")
        val wide = wire.replace("\"epoch\": 42", "\"epoch\": $maximum")
        val proof = requireNotNull(parse(wide))
        assertEquals(maximum, proof.intent.epoch)
        val serialized = DaJson.parse(proof.toJsonBytes(), "proof") as Map<*, *>
        assertEquals(maximum, (serialized["intent"] as Map<*, *>)["epoch"])
        assertFailsWith<IllegalArgumentException> { parse(wire.replaceFirst("\"epoch\": 42", "\"epoch\": 43")) }
        assertFailsWith<IllegalArgumentException> { parse(wire.replaceFirst("\"authorization\":", "\"owner\":")) }
    }

    private class CapturingDaExecutor(private val responseJson: String) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .addHeader("Content-Type", "application/json; charset=utf-8")
                    .setBody(responseJson.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }
    }

    companion object {
        private const val HASH =
            "hash:0F923F0F972DB7373EFB38439B74651907459ECE1EF94564CCECF063F8893D85#C1CB"
    }
}
