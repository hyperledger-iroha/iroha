package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.ConfidentialAssetToriiClient
import org.hyperledger.iroha.sdk.client.ZkMerklePathRequest

/** Pair-compression function used by the zk_assets commitment tree. */
fun interface ZkAssetMerkleHasher {
    fun hashPair(left: ByteArray, right: ByteArray): ByteArray
}

/** Pasta-field compressor matching `poseidon_pair` in confidential-v2 Rust code. */
object PastaPoseidonNodeHasher : ZkAssetMerkleHasher {
    private val MODULUS = BigInteger("40000000000000000000000000000000224698fc094cf91b992d30ed00000001", 16)
    private val TWO = BigInteger.valueOf(2)
    private val THREE = BigInteger.valueOf(3)
    private val SEVEN = BigInteger.valueOf(7)
    private val THIRTEEN = BigInteger.valueOf(13)

    override fun hashPair(left: ByteArray, right: ByteArray): ByteArray {
        val lhs = littleEndianScalar(left, "left").add(SEVEN).mod(MODULUS)
        val rhs = littleEndianScalar(right, "right").add(THIRTEEN).mod(MODULUS)
        val value = TWO.multiply(pow5(lhs)).add(THREE.multiply(pow5(rhs))).mod(MODULUS)
        return scalarToLittleEndian(value)
    }

    private fun pow5(value: BigInteger): BigInteger {
        val square = value.multiply(value).mod(MODULUS)
        val fourth = square.multiply(square).mod(MODULUS)
        return fourth.multiply(value).mod(MODULUS)
    }

    private fun littleEndianScalar(bytes: ByteArray, field: String): BigInteger {
        require(bytes.size == 32) { "$field must be 32 bytes" }
        val bigEndian = bytes.copyOf().also { it.reverse() }
        val value = BigInteger(1, bigEndian)
        require(value < MODULUS) { "$field must be a canonical Pasta scalar" }
        return value
    }

    private fun scalarToLittleEndian(value: BigInteger): ByteArray {
        val bigEndian = value.mod(MODULUS).toByteArray().dropWhile { it == 0.toByte() }.toByteArray()
        require(bigEndian.size <= 32) { "scalar encoding overflow" }
        val out = ByteArray(32)
        for (i in bigEndian.indices) {
            out[i] = bigEndian[bigEndian.size - 1 - i]
        }
        return out
    }
}

/** Direction bytes and sibling hashes for one zk_assets inclusion path. */
class ZkAssetMerklePath(
    @JvmField val leafIndex: Long,
    siblings: List<ByteArray>,
    directions: ByteArray,
    rootAtHeight: ByteArray,
    @JvmField val heightOrIndex: Long,
) {
    private val _siblings: List<ByteArray> = siblings.mapIndexed { index, sibling ->
        require(sibling.size == 32) { "siblings[$index] must be 32 bytes" }
        sibling.copyOf()
    }
    private val _directions: ByteArray = directions.copyOf()
    private val _rootAtHeight: ByteArray = rootAtHeight.copyOf()

    init {
        require(leafIndex >= 0) { "leafIndex must be non-negative" }
        require(heightOrIndex >= 0) { "heightOrIndex must be non-negative" }
        require(_directions.size == _siblings.size) { "directions size must match siblings size" }
        require(_rootAtHeight.size == 32) { "rootAtHeight must be 32 bytes" }
        require(_directions.size < Long.SIZE_BITS) { "path depth must fit in leafIndex bits" }
        require((leafIndex ushr _directions.size) == 0L) { "leafIndex must fit within path depth" }
        for (i in _directions.indices) {
            require(_directions[i].toInt() == 0 || _directions[i].toInt() == 1) {
                "directions[$i] must be 0 or 1"
            }
            val expectedDirection = ((leafIndex ushr i) and 1L).toInt()
            require(_directions[i].toInt() == expectedDirection) {
                "directions[$i] must match leafIndex bit $i"
            }
        }
    }

    val siblings: List<ByteArray> get() = _siblings.map { it.copyOf() }
    val directions: ByteArray get() = _directions.copyOf()
    val rootAtHeight: ByteArray get() = _rootAtHeight.copyOf()

    fun verify(commitment: ByteArray, expectedRoot: ByteArray, hasher: ZkAssetMerkleHasher): Boolean {
        if (commitment.size != 32 || expectedRoot.size != 32 || !_rootAtHeight.contentEquals(expectedRoot)) return false
        var current = commitment.copyOf()
        for (i in _siblings.indices) {
            current = if (_directions[i].toInt() == 0) {
                hasher.hashPair(current, _siblings[i])
            } else {
                hasher.hashPair(_siblings[i], current)
            }
        }
        return current.contentEquals(expectedRoot)
    }
}

/** Source of zk_assets Merkle inclusion paths. */
interface ZkAssetMerklePathProvider {
    fun getMerklePathForCommitment(asset: String, commitment: ByteArray): CompletableFuture<ZkAssetMerklePath>
    fun getMerklePaths(asset: String, commitments: List<ByteArray>): CompletableFuture<List<ZkAssetMerklePath>>
}

/** Fetches current confidential-v2 commitment inclusion paths from Torii. */
class ToriiZkAssetMerklePathProvider(
    private val client: ConfidentialAssetToriiClient = ConfidentialAssetToriiClient.builder().build(),
    private val hasher: ZkAssetMerkleHasher = PastaPoseidonNodeHasher,
) : ZkAssetMerklePathProvider {
    override fun getMerklePathForCommitment(asset: String, commitment: ByteArray): CompletableFuture<ZkAssetMerklePath> {
        return getMerklePaths(asset, listOf(commitment)).thenApply { paths -> paths.single() }
    }

    override fun getMerklePaths(asset: String, commitments: List<ByteArray>): CompletableFuture<List<ZkAssetMerklePath>> {
        return try {
            require(asset.trim().isNotEmpty()) { "asset must not be blank" }
            val copied = commitments.mapIndexed { index, commitment ->
                require(commitment.size == 32) { "commitments[$index] must be 32 bytes" }
                commitment.copyOf()
            }
            if (copied.isEmpty()) {
                CompletableFuture.completedFuture(emptyList())
            } else {
                client.getZkAssetMerklePaths(ZkMerklePathRequest(asset, copied)).thenApply { response ->
                    val rootBytes = response.rootBytes()
                    require(response.paths.size == copied.size) {
                        "Torii returned ${response.paths.size} Merkle paths for ${copied.size} commitments"
                    }
                    response.paths.mapIndexed { index, entry ->
                        require(entry.commitmentBytes().contentEquals(copied[index])) {
                            "Torii Merkle path commitment mismatch at index $index"
                        }
                        require(entry.siblings.size == response.treeDepth) {
                            "Torii Merkle path sibling depth mismatch at index $index"
                        }
                        val path = ZkAssetMerklePath(
                            entry.leafIndex.toLong(),
                            entry.siblingBytes(),
                            entry.directions,
                            rootBytes,
                            response.frontierLen.toLong(),
                        )
                        require(path.verify(copied[index], rootBytes, hasher)) {
                            "Torii Merkle path does not verify at index $index"
                        }
                        path
                    }
                }
            }
        } catch (ex: RuntimeException) {
            failedFuture(ex)
        }
    }
}

/** Computes inclusion paths from a caller-supplied zk_assets commitment frontier. */
class LocalZkAssetMerklePathProvider(
    rootHistory: List<ByteArray>,
    commitmentHistory: List<ByteArray>,
    private val hasher: ZkAssetMerkleHasher = PastaPoseidonNodeHasher,
) : ZkAssetMerklePathProvider {
    private val roots = rootHistory.mapIndexed { index, root ->
        require(root.size == 32) { "rootHistory[$index] must be 32 bytes" }
        root.copyOf()
    }
    private val commitments = commitmentHistory.mapIndexed { index, commitment ->
        require(commitment.size == 32) { "commitmentHistory[$index] must be 32 bytes" }
        commitment.copyOf()
    }

    init {
        require(commitments.size <= CONFIDENTIAL_TREE_CAPACITY_V2) {
            "commitmentHistory exceeds confidential v2 tree capacity"
        }
    }

    override fun getMerklePathForCommitment(asset: String, commitment: ByteArray): CompletableFuture<ZkAssetMerklePath> {
        return try {
            validateAssetAndCommitment(asset, commitment)
            val matches = commitments.indices.filter { commitments[it].contentEquals(commitment) }
            require(matches.size == 1) { "commitment must appear exactly once in commitmentHistory" }
            CompletableFuture.completedFuture(computePath(matches.single()))
        } catch (ex: RuntimeException) {
            failedFuture(ex)
        }
    }

    override fun getMerklePaths(asset: String, commitments: List<ByteArray>): CompletableFuture<List<ZkAssetMerklePath>> {
        return try {
            require(asset.trim().isNotEmpty()) { "asset must not be blank" }
            val out = commitments.map { getMerklePathForCommitment(asset, it).join() }
            CompletableFuture.completedFuture(out)
        } catch (ex: RuntimeException) {
            failedFuture(ex)
        }
    }

    private fun computePath(leafIndex: Int): ZkAssetMerklePath {
        var layer = ArrayList<ByteArray>(CONFIDENTIAL_TREE_CAPACITY_V2)
        layer.addAll(commitments.map { it.copyOf() })
        while (layer.size < CONFIDENTIAL_TREE_CAPACITY_V2) layer.add(ByteArray(32))
        val siblings = ArrayList<ByteArray>(CONFIDENTIAL_TREE_DEPTH_V2)
        val directions = ByteArray(CONFIDENTIAL_TREE_DEPTH_V2)
        var currentIndex = leafIndex
        repeat(CONFIDENTIAL_TREE_DEPTH_V2) { level ->
            val isRight = currentIndex % 2 == 1
            val siblingIndex = if (isRight) currentIndex - 1 else currentIndex + 1
            directions[level] = if (isRight) 1 else 0
            siblings.add(layer[siblingIndex].copyOf())
            val next = ArrayList<ByteArray>(layer.size / 2)
            var i = 0
            while (i < layer.size) {
                next.add(hasher.hashPair(layer[i], layer[i + 1]))
                i += 2
            }
            currentIndex /= 2
            layer = next
        }
        val root = layer.single()
        if (roots.isNotEmpty()) {
            require(roots.last().contentEquals(root)) {
                "latest rootHistory entry does not match computed commitment frontier root"
            }
        }
        return ZkAssetMerklePath(leafIndex.toLong(), siblings, directions, root, commitments.size.toLong())
    }

    companion object {
        const val CONFIDENTIAL_TREE_DEPTH_V2: Int = 16
        const val CONFIDENTIAL_TREE_CAPACITY_V2: Int = 1 shl CONFIDENTIAL_TREE_DEPTH_V2
    }
}

private fun validateAssetAndCommitment(asset: String, commitment: ByteArray) {
    require(asset.trim().isNotEmpty()) { "asset must not be blank" }
    require(commitment.size == 32) { "commitment must be 32 bytes" }
}

private fun <T> failedFuture(error: Throwable): CompletableFuture<T> {
    val future = CompletableFuture<T>()
    future.completeExceptionally(error)
    return future
}
