package org.hyperledger.iroha.sdk.proof

import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.text.Normalizer
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/** One root-to-leaf branch in the accumulated contract-state MerkleMap. */
class ContractStateProofStepV1(
    val bit: Int,
    val prefix: ByteArray,
    val sibling: ByteArray,
)

/** Exact raw value under one canonical physical StatePath. */
class ContractStateValueInclusionProofV1(
    val version: Int,
    val path: String,
    val value: ByteArray,
    val leafCount: Long,
    val steps: List<ContractStateProofStepV1>,
) {
    companion object {
        private const val MAX_WIRE_BYTES = 1024 * 1024 + 128 * 1024
        private const val MAX_VALUE_BYTES = 1024 * 1024
        private val proofFields = setOf("version", "path", "value", "leaf_count", "steps")
        private val stepFields = setOf("bit", "prefix", "sibling")

        /** Parse exactly the closed Norito JSON membership shape, without trusting its root. */
        @JvmStatic
        fun parseJson(payload: ByteArray): ContractStateValueInclusionProofV1 {
            require(payload.size <= MAX_WIRE_BYTES) { "contract-state proof exceeds wire bound" }
            val decoder = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
            val json = decoder.decode(ByteBuffer.wrap(payload)).toString()
            val fields = exactObject(JsonParser.parse(json), proofFields, "contract-state proof")
            val version = integer(fields["version"], "version")
            require(version == 1L) { "unsupported contract-state proof version" }
            val path = fields["path"] as? String
                ?: throw IllegalArgumentException("path must be a string")
            val value = bytes(fields["value"], MAX_VALUE_BYTES, "value")
            val count = integer(fields["leaf_count"], "leaf_count")
            require(count > 0) { "leaf_count must be positive" }
            val rawSteps = fields["steps"] as? List<*>
                ?: throw IllegalArgumentException("steps must be an array")
            require(rawSteps.size <= 256) { "too many contract-state proof steps" }
            val steps = rawSteps.map { raw ->
                val step = exactObject(raw, stepFields, "contract-state proof step")
                val bit = integer(step["bit"], "bit")
                require(bit in 0L..255L) { "proof step bit exceeds 255" }
                val prefix = bytes(step["prefix"], 32, "prefix")
                require(prefix.size == 32) { "prefix must contain 32 bytes" }
                val literal = step["sibling"] as? String
                    ?: throw IllegalArgumentException("sibling must be a canonical hash literal")
                require(Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$").matches(literal)) {
                    "sibling must be a canonical hash literal"
                }
                val sibling = HashLiteral.decode(literal)
                require(HashLiteral.canonicalize(sibling) == literal) {
                    "sibling hash literal is not canonical"
                }
                ContractStateProofStepV1(bit.toInt(), prefix, sibling)
            }
            return ContractStateValueInclusionProofV1(
                version.toInt(), path, value, count, steps,
            )
        }

        @Suppress("UNCHECKED_CAST")
        private fun exactObject(value: Any?, keys: Set<String>, label: String): Map<String, Any?> {
            require(value is Map<*, *> && value.keys == keys) {
                "$label has missing or unknown fields"
            }
            return value as Map<String, Any?>
        }

        private fun integer(value: Any?, field: String): Long {
            require(value is Long && value >= 0) { "$field must be a bounded unsigned integer" }
            return value
        }

        private fun bytes(value: Any?, maximum: Int, field: String): ByteArray {
            require(value is List<*> && value.size <= maximum) { "$field must be a bounded byte array" }
            return ByteArray(value.size) { index ->
                val item = value[index]
                require(item is Long && item in 0L..255L) { "$field contains an invalid byte" }
                item.toByte()
            }
        }
    }
}

/** Verify membership under a separately authenticated accumulated root. */
object ContractStateValueProofVerifierV1 {
    private val keyDomain = "iroha:contract-state:key:v1\u0000".toByteArray(StandardCharsets.UTF_8)
    private val valueDomain = "iroha:contract-state:value:v1\u0000".toByteArray(StandardCharsets.UTF_8)
    private val leafDomain = "iroha:merkle-map:leaf:v1\u0000".toByteArray(StandardCharsets.UTF_8)
    private val branchDomain = "iroha:merkle-map:branch:v1\u0000".toByteArray(StandardCharsets.UTF_8)
    private val rootDomain = "iroha:merkle-map:root:v1\u0000".toByteArray(StandardCharsets.UTF_8)
    private const val HASH_BYTES = 32
    private const val MAX_VALUE_BYTES = 1024 * 1024

    /**
     * Check one raw stored value and physical path against [trustedRoot].
     * The caller must first authenticate that accumulated root through linked
     * Sumeragi V2 finality. An execution-witness root is not valid here.
     */
    @JvmStatic
    fun verify(
        proof: ContractStateValueInclusionProofV1,
        expectedPath: String,
        trustedRoot: ByteArray,
    ): Boolean {
        if (proof.version != 1 || !validPath(expectedPath) || proof.path != expectedPath ||
            proof.value.size > MAX_VALUE_BYTES || proof.leafCount <= 0 ||
            proof.steps.size > HASH_BYTES * 8 ||
            (proof.leafCount == 1L) != proof.steps.isEmpty() || !markedHash(trustedRoot)
        ) return false

        val key = IrohaHash.prehash(keyDomain + proof.path.toByteArray(StandardCharsets.UTF_8))
        val valueHash = IrohaHash.prehash(valueDomain + proof.value)
        var current = IrohaHash.prehash(leafDomain + key + valueHash)
        var previousBit = -1
        for (step in proof.steps) {
            if (step.bit !in 0..255 || step.bit <= previousBit ||
                step.prefix.size != HASH_BYTES || !markedHash(step.sibling) ||
                !step.prefix.contentEquals(prefix(key, step.bit))
            ) return false
            previousBit = step.bit
        }
        for (step in proof.steps.asReversed()) {
            val right = (key[step.bit / 8].toInt() and (0x80 ushr (step.bit % 8))) != 0
            val leftHash = if (right) step.sibling else current
            val rightHash = if (right) current else step.sibling
            current = IrohaHash.prehash(
                branchDomain + shortLe(step.bit) + step.prefix + leftHash + rightHash,
            )
        }
        val actualRoot = IrohaHash.prehash(rootDomain + longLe(proof.leafCount) + current)
        return actualRoot.contentEquals(trustedRoot)
    }

    private fun validPath(path: String): Boolean {
        if (path.isEmpty() || path.length > 16 * 1024) return false
        for (index in path.indices) {
            val c = path[index]
            if (Character.isHighSurrogate(c)) {
                if (index + 1 >= path.length || !Character.isLowSurrogate(path[index + 1])) return false
            } else if (Character.isLowSurrogate(c) &&
                (index == 0 || !Character.isHighSurrogate(path[index - 1]))
            ) return false
        }
        if (path.toByteArray(StandardCharsets.UTF_8).size > 16 * 1024 ||
            Normalizer.normalize(path, Normalizer.Form.NFC) != path
        ) return false
        return path.none { c ->
            c.isWhitespace() || Character.isISOControl(c) || c in "@#$" ||
                c == '\u061C' || c == '\u200E' || c == '\u200F' ||
                c in '\u202A'..'\u202E' || c in '\u2066'..'\u2069'
        }
    }

    private fun markedHash(hash: ByteArray): Boolean =
        hash.size == HASH_BYTES && (hash[HASH_BYTES - 1].toInt() and 1) == 1

    private fun prefix(key: ByteArray, bit: Int): ByteArray {
        val result = key.copyOf()
        val index = bit / 8
        val remainder = bit % 8
        result[index] = if (remainder == 0) 0.toByte() else
            (result[index].toInt() and (0xff shl (8 - remainder))).toByte()
        result.fill(0.toByte(), index + 1)
        return result
    }

    private fun shortLe(value: Int): ByteArray =
        byteArrayOf(value.toByte(), (value ushr 8).toByte())

    private fun longLe(value: Long): ByteArray =
        ByteArray(8) { index -> (value ushr (index * 8)).toByte() }
}
