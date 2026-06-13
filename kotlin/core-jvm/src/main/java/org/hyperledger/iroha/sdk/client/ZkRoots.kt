package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Typed request body for `POST /v1/zk/roots`. */
class ZkRootsRequest @JvmOverloads constructor(
    assetId: String,
    @JvmField val maxRoots: Int = 0,
) {
    @JvmField val assetId: String = HttpClientTransport.normalizeNonBlank(assetId, "assetId")

    init {
        require(maxRoots >= 0) { "maxRoots must be non-negative" }
    }

    internal fun toJsonBytes(): ByteArray = JsonEncoder.encode(
        linkedMapOf(
            "asset_id" to assetId,
            "max" to maxRoots,
        ),
    ).toByteArray(StandardCharsets.UTF_8)
}

/** Response body emitted by `POST /v1/zk/roots`. */
class ZkRootsResponse(
    latest: String,
    roots: List<String>,
    @JvmField val height: Int,
) {
    @JvmField val latest: String = normalizeRootHexOrEmpty(latest, "latest")
    private val normalizedRoots: List<String> = roots.mapIndexed { index, value ->
        normalizeRootHex(value, "roots[$index]")
    }

    init {
        require(height >= 0) { "height must be non-negative" }
    }

    val roots: List<String> get() = normalizedRoots.toList()

    fun getLatestRootBytes(): ByteArray? = if (latest.isEmpty()) null else decodeHex32(latest, "latest")

    fun getRootBytes(index: Int): ByteArray = decodeHex32(normalizedRoots[index], "roots[$index]")

    companion object {
        @JvmStatic
        internal fun parse(payload: ByteArray): ZkRootsResponse {
            val root = JsonParser.parse(String(payload, StandardCharsets.UTF_8).trim())
            require(root is Map<*, *>) { "zk roots response must be a JSON object" }
            val latest = root["latest"]
            val roots = root["roots"]
            val height = root["height"]
            require(latest is String) { "latest must be a string" }
            require(roots is List<*>) { "roots must be an array" }
            val rootStrings = roots.mapIndexed { index, value ->
                require(value is String) { "roots[$index] must be a string" }
                value
            }
            return ZkRootsResponse(latest, rootStrings, jsonInt(height, "height"))
        }

        @JvmStatic
        fun normalizeRootHexOrEmpty(value: String, field: String): String {
            val trimmed = value.trim()
            require(trimmed == value) { "$field must be canonical lowercase hex or empty" }
            if (trimmed.isEmpty()) return ""
            return normalizeRootHex(trimmed, field)
        }

        @JvmStatic
        fun normalizeRootHex(value: String, field: String): String {
            val normalized = HttpClientTransport.normalizeHex32(value, field)
            require(normalized == value) { "$field must be canonical lowercase hex" }
            return normalized
        }

        @JvmStatic
        fun decodeHex32(value: String, field: String): ByteArray {
            val normalized = normalizeRootHex(value, field)
            val out = ByteArray(32)
            for (i in out.indices) {
                out[i] = ((hexDigit(normalized[2 * i], field, 2 * i) shl 4) or
                    hexDigit(normalized[2 * i + 1], field, 2 * i + 1)).toByte()
            }
            return out
        }

        @JvmStatic
        fun encodeHex(bytes: ByteArray, field: String = "bytes"): String {
            require(bytes.size == 32) { "$field must be 32 bytes" }
            val out = StringBuilder(64)
            for (byte in bytes) {
                val value = byte.toInt() and 0xff
                out.append(HEX[value ushr 4])
                out.append(HEX[value and 0x0f])
            }
            return out.toString()
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

        private fun hexDigit(char: Char, field: String, index: Int): Int = when (char) {
            in '0'..'9' -> char - '0'
            in 'a'..'f' -> char - 'a' + 10
            else -> error("invalid lowercase hex digit `$char` at $field[$index]")
        }

        private val HEX = "0123456789abcdef".toCharArray()
    }
}
