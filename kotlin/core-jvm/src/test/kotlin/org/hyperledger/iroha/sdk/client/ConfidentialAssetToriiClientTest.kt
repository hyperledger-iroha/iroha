package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.Signature
import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId

class ConfidentialAssetToriiClientTest {
    private val networkId = NetworkId.parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
    )
    private val otherNetworkId = NetworkId.parse(
        "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22",
    )
    private val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()

    @Test
    fun rootsUsesCanonicalPostPathAndParsesBody() {
        val root = "01".repeat(32)
        val blockHash = "0a".repeat(32)
        val executor = CapturingExecutor(
            """
            {
              "latest": "$root",
              "roots": ["$root"],
              "evaluated_block_height": 7,
              "evaluated_block_hash": "$blockHash"
            }
            """.trimIndent(),
        )
        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()

        val auth = canonicalAuth("zk-roots-1")
        val response = client.getZkAssetRoots(ZkRootsRequest("usd#bank", 7), auth).join()

        assertEquals("POST", executor.lastRequest.method)
        assertEquals("/v1/zk/roots", executor.lastRequest.uri.path)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
        assertEquals("application/json", firstHeader(executor.lastRequest, "Content-Type"))
        assertEquals("""{"asset_id":"usd#bank","max":7}""", executor.lastBody)
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.lastRequest.replayPolicy)
        assertEquals(1, executor.requestCount)
        assertCanonicalSignature(executor.lastRequest, networkId, keyPair, "zk-roots-1", true)
        assertCanonicalSignature(executor.lastRequest, otherNetworkId, keyPair, "zk-roots-1", false)
        assertEquals(root, response.latest)
        assertEquals(listOf(root), response.roots)
        assertEquals(7, response.evaluatedBlockHeight)
        assertEquals(blockHash, response.evaluatedBlockHash)
        assertContentEquals(ByteArray(32) { 1 }, response.getLatestRootBytes())
        assertContentEquals(ByteArray(32) { 1 }, response.getRootBytes(0))
    }

    @Test
    fun merklePathsUsesCanonicalPostPathAndParsesBody() {
        val commitment = "02".repeat(32)
        val sibling = "00".repeat(32)
        val root = "03".repeat(32)
        val blockHash = "0a".repeat(32)
        val executor = CapturingExecutor(
            """
            {
              "evaluated_block_height": 7,
              "evaluated_block_hash": "$blockHash",
              "root": "$root",
              "frontier_len": 1,
              "tree_depth": 1,
              "next_zero_path": {
                "commitment": "${"00".repeat(32)}",
                "leaf_index": 1,
                "siblings": ["$sibling"],
                "directions": [1],
                "witness_nodes": ["$root"],
                "root": "$root"
              },
              "paths": [{
                "commitment": "$commitment",
                "leaf_index": 0,
                "siblings": ["$sibling"],
                "directions": [0],
                "witness_nodes": ["$root"],
                "root": "$root"
              }]
            }
            """.trimIndent(),
        )
        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()

        val response = client.getZkAssetMerklePaths(
            ZkMerklePathRequest("usd#bank", listOf(ByteArray(32) { 2 })),
            canonicalAuth("zk-paths-1"),
        ).join()

        assertEquals("POST", executor.lastRequest.method)
        assertEquals("/v1/zk/merkle-path", executor.lastRequest.uri.path)
        assertEquals("application/json", firstHeader(executor.lastRequest, "Accept"))
        assertEquals("application/json", firstHeader(executor.lastRequest, "Content-Type"))
        assertEquals("""{"asset_id":"usd#bank","commitments":["$commitment"]}""", executor.lastBody)
        assertEquals(root, response.root)
        assertEquals(7, response.evaluatedBlockHeight)
        assertEquals(blockHash, response.evaluatedBlockHash)
        assertEquals(1, response.frontierLen)
        assertEquals(1, response.treeDepth)
        assertEquals(1, response.requireNextZeroPath().leafIndex)
        assertEquals(1, response.paths.size)
        assertEquals(0, response.paths[0].leafIndex)
        assertEquals(listOf(sibling), response.paths[0].siblings)
        assertContentEquals(byteArrayOf(0), response.paths[0].directions)
        response.requireEvaluatedSnapshot(7, ByteArray(32) { 0x0a })
        assertFailsWith<IllegalArgumentException> {
            response.requireEvaluatedSnapshot(8, ByteArray(32) { 0x0a })
        }
        assertFailsWith<IllegalArgumentException> {
            response.requireEvaluatedSnapshot(7, ByteArray(32) { 0x0b })
        }
    }

    @Test
    fun emptyLatestRootIsNotNullableOnWireButNullInByteHelper() {
        val response = ZkRootsResponse.parse(
            """
            {"latest":"","roots":[],"evaluated_block_height":0,"evaluated_block_hash":"${"00".repeat(32)}"}
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )

        assertEquals("", response.latest)
        assertNull(response.getLatestRootBytes())
    }

    @Test
    fun rootsRejectNonCanonicalHexAndNullLatest() {
        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse("AA".repeat(32), emptyList(), 0, "00".repeat(32))
        }
        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse.parse("""{"latest":null,"roots":[],"evaluated_block_height":0,"evaluated_block_hash":"${"00".repeat(32)}"}""".toByteArray(StandardCharsets.UTF_8))
        }
    }

    @Test
    fun rootsAndMerklePathsRejectNumericStrings() {
        val commitment = "02".repeat(32)
        val sibling = "00".repeat(32)
        val root = "03".repeat(32)
        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse.parse("""{"latest":"","roots":[],"evaluated_block_height":"0","evaluated_block_hash":"${"00".repeat(32)}"}""".toByteArray(StandardCharsets.UTF_8))
        }
        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse.parse("""{"latest":"","roots":[],"evaluated_block_height":0.0,"evaluated_block_hash":"${"00".repeat(32)}"}""".toByteArray(StandardCharsets.UTF_8))
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "$root",
                  "frontier_len": "3",
                  "tree_depth": 1,
                  "paths": []
                }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "$root",
                  "frontier_len": 3,
                  "tree_depth": 1,
                  "paths": [{
                    "commitment": "$commitment",
                    "leaf_index": 2,
                    "siblings": ["$sibling"],
                    "directions": ["0"],
                    "witness_nodes": ["$root"],
                    "root": "$root"
                  }]
                }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
        }
    }

    @Test
    fun rootsAndMerklePathsRejectOverflowDuplicateKeysAndInconsistentShape() {
        val commitment = "02".repeat(32)
        val sibling = "00".repeat(32)
        val root = "03".repeat(32)
        val otherRoot = "04".repeat(32)

        assertFailsWith<IllegalArgumentException> {
            ZkRootsResponse.parse(
                """{"latest":"","roots":[],"evaluated_block_height":9223372036854775808,"evaluated_block_hash":"${"01".repeat(32)}"}""".toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalStateException> {
            ZkRootsResponse.parse(
                """{"latest":"","latest":"$root","roots":[],"evaluated_block_height":0,"evaluated_block_hash":"${"00".repeat(32)}"}""".toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "$root",
                  "frontier_len": 2147483648,
                  "tree_depth": 1,
                  "paths": []
                }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 0, listOf(sibling), listOf(0), listOf(root), otherRoot),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 1, listOf(sibling), listOf(0), listOf(root), root),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 1, listOf(sibling), listOf(1), listOf(root), root, frontierLen = 1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 0, listOf(sibling), listOf(0), listOf(root), root, treeDepth = 2),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            ZkMerklePathResponse.parse(
                merklePathPayload(root, commitment, 0, listOf(sibling), listOf(0), emptyList(), root),
            )
        }
    }

    @Test
    fun merklePathParserRejectsDuplicateKeysBeforeLastValueWins() {
        val commitment = "02".repeat(32)
        val sibling = "00".repeat(32)
        val root = "03".repeat(32)
        val otherRoot = "04".repeat(32)

        assertFailsWith<IllegalStateException> {
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "$root",
                  "frontier_len": 3,
                  "frontier_len": 1,
                  "tree_depth": 1,
                  "paths": []
                }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
        }
        assertFailsWith<IllegalStateException> {
            ZkMerklePathResponse.parse(
                """
                {
                  "root": "$root",
                  "frontier_len": 3,
                  "tree_depth": 1,
                  "paths": [{
                    "commitment": "$commitment",
                    "commitment": "$otherRoot",
                    "leaf_index": 0,
                    "siblings": ["$sibling"],
                    "directions": [0],
                    "witness_nodes": ["$root"],
                    "root": "$root"
                  }]
                }
                """.trimIndent().toByteArray(StandardCharsets.UTF_8),
            )
        }
    }

    @Test
    fun nonSuccessResponsesSurfaceConfidentialAssetToriiException() {
        val client = ConfidentialAssetToriiClient.builder()
            .executor(CapturingExecutor("""{"error":"not ready"}""", status = 503, message = "Unavailable"))
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()

        val error = assertFailsWith<CompletionException> {
            client.getZkAssetRoots(ZkRootsRequest("usd#bank"), canonicalAuth("zk-failure-1")).join()
        }.cause

        require(error is ConfidentialAssetToriiException)
        assertEquals(503, error.statusCode)
        require(error.message?.contains("/v1/zk/roots") == true)
    }

    @Test
    fun canonicalAuthConfigurationFailsClosedBeforeDispatch() {
        val executor = CapturingExecutor("{}")
        assertFailsWith<IllegalStateException> {
            ConfidentialAssetToriiClient.builder()
                .executor(executor)
                .baseUri(URI.create("https://example.com"))
                .build()
        }
        assertEquals(0, executor.requestCount)

        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .addHeader("x-IROHA-signature", "forged")
            .build()
        assertFailsWith<IllegalArgumentException> {
            client.getZkAssetRoots(ZkRootsRequest("usd#bank"), canonicalAuth("zk-header-1"))
        }
        assertEquals(0, executor.requestCount)
    }

    private class CapturingExecutor(
        private val responseBody: String,
        private val status: Int = 200,
        private val message: String = "",
    ) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var lastBody: String = ""
        var requestCount: Int = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requestCount++
            lastRequest = request
            lastBody = String(request.body, StandardCharsets.UTF_8)
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(status)
                    .setMessage(message)
                    .setBody(responseBody.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }
    }

    private fun firstHeader(request: TransportRequest, name: String): String? = request.headers
        .entries
        .firstOrNull { it.key.equals(name, ignoreCase = true) }
        ?.value
        ?.firstOrNull()

    private fun canonicalAuth(nonce: String): ToriiCanonicalRequestAuth =
        ToriiCanonicalRequestAuth("alice", keyPair.private, 1_700_000_000_000L, nonce)

    private fun assertCanonicalSignature(
        request: TransportRequest,
        expectedNetworkId: NetworkId,
        signingKeyPair: KeyPair,
        nonce: String,
        expected: Boolean,
    ) {
        val encoded = assertNotNull(firstHeader(request, CanonicalRequestSigner.HEADER_SIGNATURE))
        val verifier = Signature.getInstance("Ed25519")
        verifier.initVerify(signingKeyPair.public)
        verifier.update(
            CanonicalRequestSigner.canonicalRequestSignatureMessage(
                expectedNetworkId,
                request.method,
                request.uri,
                request.body,
                1_700_000_000_000L,
                nonce,
            ),
        )
        if (expected) {
            assertTrue(verifier.verify(Base64.getDecoder().decode(encoded)))
        } else {
            assertFalse(verifier.verify(Base64.getDecoder().decode(encoded)))
        }
    }

    private fun merklePathPayload(
        root: String,
        commitment: String,
        leafIndex: Int,
        siblings: List<String>,
        directions: List<Int>,
        witnessNodes: List<String>,
        pathRoot: String,
        frontierLen: Int = 3,
        treeDepth: Int = siblings.size,
    ): ByteArray {
        val siblingJson = siblings.joinToString(",") { """"$it"""" }
        val directionJson = directions.joinToString(",")
        val witnessJson = witnessNodes.joinToString(",") { """"$it"""" }
        return """
            {
              "evaluated_block_height": 7,
              "evaluated_block_hash": "${"0a".repeat(32)}",
              "root": "$root",
              "frontier_len": $frontierLen,
              "tree_depth": $treeDepth,
              "next_zero_path": null,
              "paths": [{
                "commitment": "$commitment",
                "leaf_index": $leafIndex,
                "siblings": [$siblingJson],
                "directions": [$directionJson],
                "witness_nodes": [$witnessJson],
                "root": "$pathRoot"
              }]
            }
        """.trimIndent().toByteArray(StandardCharsets.UTF_8)
    }
}
