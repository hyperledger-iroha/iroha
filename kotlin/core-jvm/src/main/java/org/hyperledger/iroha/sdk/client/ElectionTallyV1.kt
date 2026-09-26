package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.util.Collections

/** Exact public tally and committed snapshot returned by `POST /v1/zk/vote/tally`. */
class ElectionTallyV1(
    @JvmField val evaluatedBlockHeight: BigInteger,
    @JvmField val evaluatedBlockHash: String,
    @JvmField val finalized: Boolean,
    tally: List<BigInteger>,
) {
    /** Exact per-option weights in smallest-unit conviction arithmetic. */
    val tally: List<BigInteger> = Collections.unmodifiableList(ArrayList(tally))

    init {
        require(evaluatedBlockHeight >= BigInteger.ZERO && evaluatedBlockHeight <= U64_MAX) {
            "evaluated_block_height must fit u64"
        }
        require(HASH_PATTERN.matches(evaluatedBlockHash)) {
            "evaluated_block_hash must be canonical lowercase 32-byte hex"
        }
        require((evaluatedBlockHeight == BigInteger.ZERO) == (evaluatedBlockHash == ZERO_HASH)) {
            "the zero block hash is reserved for evaluated_block_height=0"
        }
        require(this.tally.size in 2..64) { "tally must contain 2..64 option weights" }
        var aggregate = BigInteger.ZERO
        this.tally.forEachIndexed { index, weight ->
            require(weight >= BigInteger.ZERO && weight <= U128_MAX) {
                "tally[$index] must fit u128"
            }
            aggregate = aggregate.add(weight)
            require(aggregate <= U128_MAX) { "tally aggregate exceeds u128" }
        }
    }

    companion object {
        /** Inclusive maximum response size for this fixed four-field V1 JSON schema. */
        const val MAX_RESPONSE_BYTES = 8 * 1024

        private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        private val U128_MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)
        private val ZERO_HASH = "0".repeat(64)
        private val HASH_PATTERN = Regex("^[0-9a-f]{64}$")
        private val FIELDS = setOf(
            "evaluated_block_height", "evaluated_block_hash", "finalized", "tally",
        )

        /** Parse only the canonical V1 response; JSON integer tokens remain lossless. */
        @JvmStatic
        fun parse(payload: ByteArray): ElectionTallyV1 {
            require(payload.isNotEmpty() && payload.size <= MAX_RESPONSE_BYTES) {
                "election tally response exceeds its size bound or is empty"
            }
            val json = StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(payload))
                .toString()
            val root = JsonParser.parse(json)
            require(root is Map<*, *> && root.keys == FIELDS) {
                "election tally response must have exactly four V1 fields"
            }
            val hash = root["evaluated_block_hash"]
            val finalized = root["finalized"]
            val values = root["tally"]
            require(hash is String) { "evaluated_block_hash must be a string" }
            require(finalized is Boolean) { "finalized must be a boolean" }
            require(values is List<*> && values.size in 2..64) {
                "tally must be an array of 2..64 option weights"
            }
            return ElectionTallyV1(
                jsonUnsigned(root["evaluated_block_height"], U64_MAX, "evaluated_block_height"),
                hash,
                finalized,
                values.mapIndexed { index, value ->
                    jsonUnsigned(value, U128_MAX, "tally[$index]")
                },
            )
        }

        private fun jsonUnsigned(value: Any?, maximum: BigInteger, field: String): BigInteger {
            val integer = when (value) {
                is Long -> BigInteger.valueOf(value)
                is BigInteger -> value
                else -> throw IllegalArgumentException("$field must be a JSON integer")
            }
            require(integer >= BigInteger.ZERO && integer <= maximum) {
                "$field exceeds its unsigned bound"
            }
            return integer
        }
    }
}
