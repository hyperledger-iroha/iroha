package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Typed request body for `POST /v1/zk/merkle-path`. */
class ZkMerklePathRequest(
    assetId: String,
    commitments: List<ByteArray>,
) {
    @JvmField val assetId: String = HttpClientTransport.normalizeNonBlank(assetId, "assetId")
    private val commitmentHex: List<String> = commitments.mapIndexed { index, commitment ->
        ZkRootsResponse.encodeHex(commitment, "commitments[$index]")
    }

    val commitments: List<String> get() = commitmentHex.toList()

    internal fun toJsonBytes(): ByteArray = JsonEncoder.encode(
        linkedMapOf(
            "asset_id" to assetId,
            "commitments" to commitmentHex,
        ),
    ).toByteArray(StandardCharsets.UTF_8)
}

/** One path entry returned by `POST /v1/zk/merkle-path`. */
class ZkMerklePathEntry(
    commitment: String,
    @JvmField val leafIndex: Int,
    siblings: List<String>,
    directions: ByteArray,
    witnessNodes: List<String>,
    root: String,
) {
    @JvmField val commitment: String = ZkRootsResponse.normalizeRootHex(commitment, "commitment")
    private val normalizedSiblings: List<String> = siblings.mapIndexed { index, value ->
        ZkRootsResponse.normalizeRootHex(value, "siblings[$index]")
    }
    private val directionBytes: ByteArray = directions.copyOf()
    private val normalizedWitnessNodes: List<String> = witnessNodes.mapIndexed { index, value ->
        ZkRootsResponse.normalizeRootHex(value, "witness_nodes[$index]")
    }
    @JvmField val root: String = ZkRootsResponse.normalizeRootHex(root, "root")

    init {
        require(leafIndex >= 0) { "leaf_index must be non-negative" }
        require(directionBytes.size == normalizedSiblings.size) { "directions size must match siblings size" }
        require(normalizedWitnessNodes.size == normalizedSiblings.size) {
            "witness_nodes size must match siblings size"
        }
        require(directionBytes.size < Int.SIZE_BITS) { "path depth must fit in leaf_index bits" }
        require((leafIndex ushr directionBytes.size) == 0) { "leaf_index must fit within path depth" }
        for (i in directionBytes.indices) {
            require(directionBytes[i].toInt() == 0 || directionBytes[i].toInt() == 1) {
                "directions[$i] must be 0 or 1"
            }
            val expectedDirection = (leafIndex ushr i) and 1
            require(directionBytes[i].toInt() == expectedDirection) {
                "directions[$i] must match leaf_index bit $i"
            }
        }
    }

    val siblings: List<String> get() = normalizedSiblings.toList()
    val directions: ByteArray get() = directionBytes.copyOf()
    val witnessNodes: List<String> get() = normalizedWitnessNodes.toList()

    fun commitmentBytes(): ByteArray = ZkRootsResponse.decodeHex32(commitment, "commitment")
    fun siblingBytes(): List<ByteArray> = normalizedSiblings.mapIndexed { index, value ->
        ZkRootsResponse.decodeHex32(value, "siblings[$index]")
    }
    fun rootBytes(): ByteArray = ZkRootsResponse.decodeHex32(root, "root")
}

/** Response body emitted by `POST /v1/zk/merkle-path`. */
class ZkMerklePathResponse(
    root: String,
    @JvmField val frontierLen: Int,
    @JvmField val treeDepth: Int,
    paths: List<ZkMerklePathEntry>,
) {
    @JvmField val root: String = ZkRootsResponse.normalizeRootHex(root, "root")
    private val normalizedPaths: List<ZkMerklePathEntry> = paths.toList()

    init {
        require(frontierLen >= 0) { "frontier_len must be non-negative" }
        require(treeDepth >= 0) { "tree_depth must be non-negative" }
        for ((index, path) in normalizedPaths.withIndex()) {
            require(path.root == this.root) { "paths[$index].root must match response root" }
            require(path.leafIndex < frontierLen) { "paths[$index].leaf_index must be below frontier_len" }
            require(path.siblings.size == treeDepth) {
                "paths[$index].siblings size must match tree_depth"
            }
        }
    }

    val paths: List<ZkMerklePathEntry> get() = normalizedPaths.toList()
    fun rootBytes(): ByteArray = ZkRootsResponse.decodeHex32(root, "root")

    companion object {
        @JvmStatic
        internal fun parse(payload: ByteArray): ZkMerklePathResponse {
            val root = JsonParser.parse(String(payload, StandardCharsets.UTF_8).trim())
            require(root is Map<*, *>) { "zk merkle-path response must be a JSON object" }
            val pathValues = root["paths"]
            require(pathValues is List<*>) { "paths must be an array" }
            val rootHex = jsonString(root["root"], "root")
            val entries = pathValues.mapIndexed { index, value ->
                require(value is Map<*, *>) { "paths[$index] must be an object" }
                ZkMerklePathEntry(
                    commitment = jsonString(value["commitment"], "paths[$index].commitment"),
                    leafIndex = jsonInt(value["leaf_index"], "paths[$index].leaf_index"),
                    siblings = jsonStringList(value["siblings"], "paths[$index].siblings"),
                    directions = jsonDirections(value["directions"], "paths[$index].directions"),
                    witnessNodes = jsonStringList(value["witness_nodes"], "paths[$index].witness_nodes"),
                    root = jsonString(value["root"], "paths[$index].root"),
                )
            }
            return ZkMerklePathResponse(
                rootHex,
                jsonInt(root["frontier_len"], "frontier_len"),
                jsonInt(root["tree_depth"], "tree_depth"),
                entries,
            )
        }

        private fun jsonString(value: Any?, field: String): String {
            require(value is String) { "$field must be a string" }
            return value
        }

        private fun jsonStringList(value: Any?, field: String): List<String> {
            require(value is List<*>) { "$field must be an array" }
            return value.mapIndexed { index, item ->
                require(item is String) { "$field[$index] must be a string" }
                item
            }
        }

        private fun jsonDirections(value: Any?, field: String): ByteArray {
            require(value is List<*>) { "$field must be an array" }
            return ByteArray(value.size) { index ->
                val raw = value[index]
                val parsed = when (raw) {
                    is Byte -> raw.toLong()
                    is Short -> raw.toLong()
                    is Int -> raw.toLong()
                    is Long -> raw
                    is java.math.BigInteger -> raw.takeIf {
                        it == java.math.BigInteger.ZERO || it == java.math.BigInteger.ONE
                    }?.toLong() ?: throw IllegalArgumentException("$field[$index] must be 0 or 1")
                    else -> throw IllegalArgumentException("$field[$index] must be a JSON integer")
                }
                require(parsed == 0L || parsed == 1L) { "$field[$index] must be 0 or 1" }
                parsed.toByte()
            }
        }

        private fun jsonInt(value: Any?, field: String): Int {
            val parsed = when (value) {
                is Byte -> value.toLong()
                is Short -> value.toLong()
                is Int -> value.toLong()
                is Long -> value
                is java.math.BigInteger -> {
                    require(
                        value >= java.math.BigInteger.ZERO &&
                            value <= java.math.BigInteger.valueOf(Int.MAX_VALUE.toLong()),
                    ) {
                        "$field is outside u32-compatible Int range"
                    }
                    value.toLong()
                }
                else -> throw IllegalArgumentException("$field must be a JSON integer")
            }
            require(parsed in 0..Int.MAX_VALUE) { "$field is outside u32-compatible Int range" }
            return parsed.toInt()
        }
    }
}
