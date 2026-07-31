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
    fun listCommitmentsUsesCanonicalDigestWrapperAndTypedResponse() {
        val executor = CapturingDaExecutor(
            """
            {
              "policies":{"version":1,"policy_hash":"$HASH","policies":[]},
              "commitments":[]
            }
            """.trimIndent(),
        )
        val client = client(executor)

        val response = client.listCommitments(
            DaModels.CommitmentQuery(
                manifestHash = DaModels.Digest32.fromHex("11".repeat(32)),
                laneId = 7,
                epoch = BigInteger("18446744073709551615"),
                sequence = BigInteger.valueOf(9),
                pagination = DaModels.Pagination(BigInteger.valueOf(3), BigInteger.ONE),
            ),
        ).join()

        assertEquals("POST", executor.lastRequest.method)
        assertEquals("/v1/da/commitments", executor.lastRequest.uri.path)
        val request = DaJson.parse(executor.lastRequest.body, "request") as Map<*, *>
        val digest = request["manifest_hash"] as List<*>
        assertEquals(1, digest.size)
        assertEquals(List(32) { 0x11L }, digest[0])
        assertEquals(BigInteger("18446744073709551615"), request["epoch"])
        assertEquals(1, response.policies.version)
        assertEquals(HASH, response.policies.policyHash)
        assertTrue(response.commitments.isEmpty())
    }

    @Test
    fun proveAndVerifyPinIntentPreserveUnsignedIntegersAndProofShape() {
        val proofJson = pinIntentProofJson()
        val proveExecutor = CapturingDaExecutor(proofJson)
        val proof = client(proveExecutor).provePinIntent(
            DaModels.PinIntentQuery(
                storageTicket = DaModels.Digest32.fromHex("22".repeat(32)),
            ),
        ).join()

        requireNotNull(proof)
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
        assertNull(client(executor).proveCommitment().join())
        assertEquals("/v1/da/commitments/prove", executor.lastRequest.uri.path)
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
    }

    @Test
    fun pinIntentAliasesUseServerUtf8ByteBound() {
        DaModels.PinIntentQuery(alias = "")
        DaModels.PinIntentQuery(alias = "é".repeat(128))
        DaModels.RetentionPolicy(
            hotRetentionSeconds = BigInteger.ZERO,
            coldRetentionSeconds = BigInteger.ZERO,
            requiredReplicas = 0,
            storageClass = DaModels.StorageClass.HOT,
            governanceTag = "",
        )
        assertFailsWith<IllegalArgumentException> {
            DaModels.PinIntentQuery(alias = "é".repeat(129))
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
