package org.hyperledger.iroha.sdk.privacy

import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class ZkAssetMerklePathTest {
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
        assertEquals(true, path.verify(commitments[1], root, PastaPoseidonNodeHasher))
        assertEquals(false, path.verify(commitments[1], scalarBytes(9), PastaPoseidonNodeHasher))
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
    }

    @Test
    fun toriiProviderFailsClosedUntilNodeEndpointExists() {
        val provider = ToriiZkAssetMerklePathProvider()
        val error = assertFailsWith<CompletionException> {
            provider.getMerklePathForCommitment("usd#bank", scalarBytes(1)).join()
        }.cause

        require(error is UnsupportedOperationException)
        require(error.message?.contains("does not expose") == true)
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

        assertEquals(true, path.verify(commitment, path.rootAtHeight, PastaPoseidonNodeHasher))
    }

    private fun scalarBytes(value: Int): ByteArray = ByteArray(32).also { it[0] = value.toByte() }

    private fun computeRoot(commitments: List<ByteArray>): ByteArray {
        var layer = ArrayList<ByteArray>(LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_CAPACITY_V2)
        layer.addAll(commitments.map { it.copyOf() })
        while (layer.size < LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_CAPACITY_V2) {
            layer.add(ByteArray(32))
        }
        repeat(LocalZkAssetMerklePathProvider.CONFIDENTIAL_TREE_DEPTH_V2) {
            val next = ArrayList<ByteArray>(layer.size / 2)
            var i = 0
            while (i < layer.size) {
                next.add(PastaPoseidonNodeHasher.hashPair(layer[i], layer[i + 1]))
                i += 2
            }
            layer = next
        }
        return layer.single()
    }
}
