package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class DaToriiClientTest {
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
        assertEquals(BigInteger("18446744073709551615"), proof.intent.epoch)
        assertEquals(2, proof.bundleLength)
        assertEquals(DaModels.MerkleDirection.RIGHT, proof.path.single().direction)

        val verifyExecutor = CapturingDaExecutor("""{"valid":true,"error":null}""")
        val response = client(verifyExecutor).verifyPinIntent(proof).join()
        assertTrue(response.valid)
        assertNull(response.error)
        assertEquals("/v1/da/pin-intents/verify", verifyExecutor.lastRequest.uri.path)
        val posted = DaJson.parse(verifyExecutor.lastRequest.body, "request") as Map<*, *>
        val intent = posted["intent"] as Map<*, *>
        assertEquals(BigInteger("18446744073709551615"), intent["epoch"])
        assertTrue(intent.containsKey("alias"))
        assertNull(intent["alias"])
        assertTrue(intent.containsKey("owner"))
        assertNull(intent["owner"])
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
        assertFailsWith<IllegalArgumentException> {
            DaModels.ProofScheme.fromJson(
                mapOf("type" to "KzgBls12_381", "value" to null),
                "proof_scheme",
            )
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

    private fun client(executor: HttpTransportExecutor): DaToriiClient =
        DaToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build()

    private fun pinIntentProofJson(): String {
        val digest = digestJson(0x22)
        return """
            {
              "intent":{
                "lane_id":7,
                "epoch":18446744073709551615,
                "sequence":9,
                "storage_ticket":$digest,
                "manifest_hash":$digest,
                "alias":null,
                "owner":null
              },
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

    private fun digestJson(byte: Int): String =
        "[[" + List(32) { byte.toString() }.joinToString(",") + "]]"

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
