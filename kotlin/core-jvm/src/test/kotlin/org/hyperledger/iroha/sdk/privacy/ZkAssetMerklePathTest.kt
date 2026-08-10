package org.hyperledger.iroha.sdk.privacy

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.client.ConfidentialAssetToriiClient
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.ZkRootsResponse
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId

class ZkAssetMerklePathTest {
    private val networkId = NetworkId.parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
    )
    private val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()

    @Test
    fun localProviderComputesAndVerifiesCurrentFrontierPath() {
        val commitments = listOf(scalarBytes(1), scalarBytes(2), scalarBytes(3))
        val root = computeRoot(commitments)
        val provider = LocalZkAssetMerklePathProvider(listOf(root), commitments)

        val path = provider.getMerklePathForCommitment("usd#bank", commitments[1]).join()

        assertEquals(1L, path.leafIndex)
        assertEquals(LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2, path.siblings.size)
        assertEquals(LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2, path.directions.size)
        assertContentEquals(root, path.rootAtHeight)
        assertEquals(true, path.verify(commitments[1], root))
        assertEquals(false, path.verify(commitments[1], scalarBytes(9)))
    }

    @Test
    fun localProviderRejectsAmbiguousOrMismatchedFrontiers() {
        val repeated = scalarBytes(4)
        val duplicateProvider = LocalZkAssetMerklePathProvider(emptyList(), listOf(repeated, repeated))
        assertFailsWith<CompletionException> {
            duplicateProvider.getMerklePathForCommitment("usd#bank", repeated).join()
        }

        val provider = LocalZkAssetMerklePathProvider(listOf(scalarBytes(9)), listOf(scalarBytes(1)))
        assertFailsWith<CompletionException> {
            provider.getMerklePathForCommitment("usd#bank", scalarBytes(1)).join()
        }

        assertFailsWith<IllegalArgumentException> {
            ZkAssetMerklePath(1, listOf(ByteArray(32)), byteArrayOf(0), ByteArray(32), 1)
        }
        assertFailsWith<IllegalArgumentException> {
            ZkAssetMerklePath(2, listOf(ByteArray(32)), byteArrayOf(0), ByteArray(32), 1)
        }
    }

    @Test
    fun toriiProviderFetchesAndValidatesNodeEndpointPaths() {
        val commitments = listOf(scalarBytes(1), scalarBytes(2))
        val root = computeRoot(commitments)
        val localPath = LocalZkAssetMerklePathProvider(listOf(root), commitments)
            .getMerklePathForCommitment("usd#bank", commitments[1])
            .join()
        val response = merklePathResponse(root, commitments[1], localPath)
        val executor = CapturingExecutor(response)
        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()
        val provider = ToriiZkAssetMerklePathProvider(client, canonicalAuth())

        val path = provider.getMerklePathForCommitment("usd#bank", commitments[1]).join()

        assertEquals("/v1/zk/merkle-path", executor.lastRequest.uri.path)
        assertEquals("""{"asset_id":"usd#bank","commitments":["${hex(commitments[1])}"]}""", executor.lastBody)
        assertEquals(1L, path.leafIndex)
        assertContentEquals(root, path.rootAtHeight)
        assertEquals(true, path.verify(commitments[1], root))
    }

    @Test
    fun toriiProviderRejectsPathCountDriftAndReorderedNodeResponses() {
        val commitments = listOf(scalarBytes(1), scalarBytes(2))
        val root = computeRoot(commitments)
        val localProvider = LocalZkAssetMerklePathProvider(listOf(root), commitments)
        val firstPath = localProvider.getMerklePathForCommitment("usd#bank", commitments[0]).join()
        val secondPath = localProvider.getMerklePathForCommitment("usd#bank", commitments[1]).join()

        val shortProvider = toriiProviderWithResponse(
            merklePathResponse(
                root,
                listOf(MerklePathResponseEntry(commitments[0], firstPath)),
            ),
        )
        val shortError = assertFailsWith<CompletionException> {
            shortProvider.getMerklePaths("usd#bank", commitments).join()
        }.cause
        require(shortError is IllegalArgumentException)
        require(shortError.message?.contains("Torii returned 1 Merkle paths for 2 commitments") == true)

        val longProvider = toriiProviderWithResponse(
            merklePathResponse(
                root,
                listOf(
                    MerklePathResponseEntry(commitments[0], firstPath),
                    MerklePathResponseEntry(commitments[1], secondPath),
                    MerklePathResponseEntry(commitments[0], firstPath),
                ),
            ),
        )
        val longError = assertFailsWith<CompletionException> {
            longProvider.getMerklePaths("usd#bank", commitments).join()
        }.cause
        require(longError is IllegalArgumentException)
        require(longError.message?.contains("Torii returned 3 Merkle paths for 2 commitments") == true)

        val reorderedProvider = toriiProviderWithResponse(
            merklePathResponse(
                root,
                listOf(
                    MerklePathResponseEntry(commitments[1], secondPath),
                    MerklePathResponseEntry(commitments[0], firstPath),
                ),
            ),
        )
        val reorderedError = assertFailsWith<CompletionException> {
            reorderedProvider.getMerklePaths("usd#bank", commitments).join()
        }.cause
        require(reorderedError is IllegalArgumentException)
        require(reorderedError.message?.contains("commitment mismatch at index 0") == true)
    }

    @Test
    fun toriiProviderRejectsMismatchedNodeCommitment() {
        val requested = scalarBytes(1)
        val root = computeRoot(listOf(requested))
        val localPath = LocalZkAssetMerklePathProvider(listOf(root), listOf(requested))
            .getMerklePathForCommitment("usd#bank", requested)
            .join()
        val executor = CapturingExecutor(merklePathResponse(root, scalarBytes(2), localPath))
        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()
        val provider = ToriiZkAssetMerklePathProvider(client, canonicalAuth())

        val error = assertFailsWith<CompletionException> {
            provider.getMerklePathForCommitment("usd#bank", requested).join()
        }.cause

        require(error is IllegalArgumentException)
        require(error.message?.contains("commitment mismatch") == true)
    }

    @Test
    fun toriiProviderRejectsNonVerifyingNodePath() {
        val commitments = listOf(scalarBytes(1), scalarBytes(2))
        val root = computeRoot(commitments)
        val localPath = LocalZkAssetMerklePathProvider(listOf(root), commitments)
            .getMerklePathForCommitment("usd#bank", commitments[1])
            .join()
        val badSiblings = localPath.siblings.toMutableList()
        badSiblings[0] = scalarBytes(9)
        val executor = CapturingExecutor(
            merklePathResponse(root, commitments[1], localPath, siblingsOverride = badSiblings),
        )
        val client = ConfidentialAssetToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()
        val provider = ToriiZkAssetMerklePathProvider(client, canonicalAuth())

        val error = assertFailsWith<CompletionException> {
            provider.getMerklePathForCommitment("usd#bank", commitments[1]).join()
        }.cause

        require(error is IllegalArgumentException)
        require(error.message?.contains("does not verify") == true)
    }

    @Test
    fun pathAccessorsReturnDefensiveCopies() {
        val commitment = scalarBytes(1)
        val provider = LocalZkAssetMerklePathProvider(emptyList(), listOf(commitment))
        val path = provider.getMerklePathForCommitment("usd#bank", commitment).join()
        val root = path.rootAtHeight
        root[0] = 99
        val sibling = path.siblings[0]
        sibling[0] = 88
        val directions = path.directions
        directions[0] = 1

        assertEquals(true, path.verify(commitment, path.rootAtHeight))
    }

    private fun scalarBytes(value: Int): ByteArray = ByteArray(32).also { it[0] = value.toByte() }

    private fun hex(bytes: ByteArray): String = ZkRootsResponse.encodeHex(bytes)

    private fun merklePathResponse(
        root: ByteArray,
        commitment: ByteArray,
        path: ZkAssetMerklePath,
        siblingsOverride: List<ByteArray> = path.siblings,
    ): String = merklePathResponse(root, listOf(MerklePathResponseEntry(commitment, path, siblingsOverride)))

    private fun toriiProviderWithResponse(responseBody: String): ToriiZkAssetMerklePathProvider {
        val client = ConfidentialAssetToriiClient.builder()
            .executor(CapturingExecutor(responseBody))
            .baseUri(URI.create("https://example.com"))
            .localSigningContext(LocalSigningContext(networkId))
            .build()
        return ToriiZkAssetMerklePathProvider(client, canonicalAuth())
    }

    private fun canonicalAuth(): ToriiCanonicalRequestAuth =
        ToriiCanonicalRequestAuth(
            "alice",
            keyPair.private,
            1_700_000_000_000L,
            "zk-provider-1",
        )

    private data class MerklePathResponseEntry(
        val commitment: ByteArray,
        val path: ZkAssetMerklePath,
        val siblings: List<ByteArray> = path.siblings,
    )

    private fun merklePathResponse(
        root: ByteArray,
        entries: List<MerklePathResponseEntry>,
    ): String {
        val treeDepth = entries.firstOrNull()?.path?.siblings?.size ?: 0
        val paths = entries.joinToString(",") { entry ->
            val siblings = entry.siblings.joinToString(",") { """"${hex(it)}"""" }
            val directions = entry.path.directions.joinToString(",") { (it.toInt() and 0xff).toString() }
            val witnessNodes = entry.path.siblings.joinToString(",") { """"${hex(it)}"""" }
            """
                {
                  "commitment": "${hex(entry.commitment)}",
                  "leaf_index": ${entry.path.leafIndex},
                  "siblings": [$siblings],
                  "directions": [$directions],
                  "witness_nodes": [$witnessNodes],
                  "root": "${hex(root)}"
                }
            """.trimIndent()
        }
        return """
            {
              "evaluated_block_height": 7,
              "evaluated_block_hash": "${"0a".repeat(32)}",
              "root": "${hex(root)}",
              "frontier_len": 2,
              "tree_depth": $treeDepth,
              "next_zero_path": null,
              "paths": [$paths]
            }
        """.trimIndent()
    }

    private class CapturingExecutor(private val responseBody: String) : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest
        var lastBody: String = ""

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            lastBody = String(request.body, StandardCharsets.UTF_8)
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(responseBody.toByteArray(StandardCharsets.UTF_8))
                    .build(),
            )
        }
    }

    private fun computeRoot(commitments: List<ByteArray>): ByteArray {
        return PrivacyNativeBridge.deriveConfidentialMerklePathV3(commitments, 0)
            .copyOfRange(0, 32)
    }
}
