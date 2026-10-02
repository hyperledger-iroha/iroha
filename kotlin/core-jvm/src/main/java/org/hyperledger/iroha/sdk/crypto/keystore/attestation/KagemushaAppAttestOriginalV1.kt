// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto.keystore.attestation

import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction

/** Bounded original-byte counter projection; signatures, release and retained-floor admission remain native. */
internal object KagemushaAppAttestOriginalV1 {
    fun counter(original: ByteArray): UInt {
        require(original.size in 1..311) { "App Attest original exceeds the canonical bound" }
        val reader = Reader(original.copyOf())
        require(reader.length(5, 2) == 2) { "App Attest original is not a two-field map" }
        var auth: ByteArray? = null
        var signature: ByteArray? = null
        repeat(2) {
            when (reader.text(32)) {
                "authenticatorData" -> {
                    require(auth == null) { "duplicate App Attest authenticatorData" }
                    auth = reader.bytes(1_024)
                }
                "signature" -> {
                    require(signature == null) { "duplicate App Attest signature" }
                    signature = reader.bytes(72)
                }
                else -> throw IllegalArgumentException("unknown App Attest original field")
            }
        }
        val data = requireNotNull(auth) { "missing App Attest authenticatorData" }
        require(reader.atEnd && data.size in 37..206 &&
            requireNotNull(signature).size in 8..72 && data.take(32).any { it != 0.toByte() }) {
            "App Attest original shape differs"
        }
        if (data.size == 37) {
            require(data[32] == 0x40.toByte()) { "limited App Attest flags differ" }
        } else {
            require(data[32] == 0x40.toByte() || data[32] == 0xc0.toByte()) {
                "extended App Attest flags differ"
            }
            requireExtensions(data.copyOfRange(37, data.size))
        }
        var counter = 0u
        for (index in 33..36) counter = (counter shl 8) or (data[index].toUInt() and 0xffu)
        return counter
    }

    private fun requireExtensions(bytes: ByteArray) {
        val reader = Reader(bytes)
        require(reader.length(5, 2) == 2) { "App Attest extensions are not a two-field map" }
        var categorySeen = false
        var versionSeen = false
        repeat(2) {
            when (reader.text(32)) {
                "validationCategory" -> {
                    require(!categorySeen) { "duplicate App Attest validationCategory" }
                    categorySeen = true
                    val value = reader.bytes(4)
                    require(value.size == 4) { "App Attest category width differs" }
                    var category = 0u
                    for (index in value.indices) category = category or
                        ((value[index].toUInt() and 0xffu) shl (index * 8))
                    require(category in 1u..6u || category == 10u) { "unknown App Attest category" }
                }
                "bundleVersion" -> {
                    require(!versionSeen) { "duplicate App Attest bundleVersion" }
                    versionSeen = true
                    val value = reader.text(128)
                    require(value.isNotEmpty() && !value.contains('\u0000')) {
                        "App Attest bundle version differs"
                    }
                }
                else -> throw IllegalArgumentException("unknown App Attest extension")
            }
        }
        require(reader.atEnd && categorySeen && versionSeen) { "incomplete App Attest extensions" }
    }

    private class Reader(private val input: ByteArray) {
        private var cursor = 0
        val atEnd: Boolean get() = cursor == input.size
        fun length(major: Int, maximum: Int): Int {
            val first = take(1)[0].toInt() and 0xff
            require(first ushr 5 == major) { "App Attest CBOR type differs" }
            val argument = first and 31
            val value = if (argument < 24) argument.toLong() else {
                val width = when (argument) {
                    24 -> 1
                    25 -> 2
                    26 -> 4
                    else -> throw IllegalArgumentException("indefinite or oversized App Attest CBOR")
                }
                var result = 0L
                for (byte in take(width)) result = (result shl 8) or (byte.toLong() and 255)
                require(result >= when (width) { 1 -> 24L; 2 -> 256L; else -> 65_536L }) {
                    "nonminimal App Attest CBOR length"
                }
                result
            }
            require(value <= maximum.toLong()) { "App Attest CBOR length exceeds its bound" }
            return value.toInt()
        }
        fun bytes(maximum: Int): ByteArray = take(length(2, maximum))
        fun text(maximum: Int): String {
            val bytes = take(length(3, maximum))
            return runCatching {
                Charsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(bytes)).toString()
            }.getOrElse { throw IllegalArgumentException("malformed App Attest UTF-8", it) }
        }
        private fun take(count: Int): ByteArray {
            require(count >= 0 && count <= input.size - cursor) { "truncated App Attest CBOR" }
            return input.copyOfRange(cursor, cursor + count).also { cursor += count }
        }
    }
}
