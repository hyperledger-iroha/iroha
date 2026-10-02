// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction

/** Bounded projection of Apple's original CBOR; signature and policy admission remain native. */
internal object KagemushaAppAttestOriginalCounterV1 {
    fun read(original: ByteArray): UInt {
        require(original.size in 1..311) { "invalid original App Attest assertion size" }
        val reader = Reader(original)
        require(reader.length(5, 2) == 2) { "invalid App Attest assertion map" }
        var authenticator: ByteArray? = null
        var signature: ByteArray? = null
        repeat(2) {
            when (reader.text(32)) {
                "authenticatorData" -> {
                    require(authenticator == null) { "duplicate authenticatorData" }
                    authenticator = reader.bytes(1_024)
                }
                "signature" -> {
                    require(signature == null) { "duplicate signature" }
                    signature = reader.bytes(72)
                }
                else -> throw IllegalArgumentException("unknown App Attest assertion field")
            }
        }
        require(reader.atEnd()) { "trailing App Attest assertion bytes" }
        val auth = requireNotNull(authenticator) { "missing authenticatorData" }
        require(requireNotNull(signature) { "missing signature" }.size in 8..72) {
            "invalid App Attest signature shape"
        }
        require(auth.size in 37..206 && auth.take(32).any { it != 0.toByte() }) {
            "invalid App Attest authenticator shape"
        }
        val flags = auth[32].toInt() and 0xff
        if (auth.size == 37) {
            require(flags == 0x40) { "invalid limited App Attest flags" }
        } else {
            require(flags == 0x40 || flags == 0xc0) { "invalid extended App Attest flags" }
            requireExtensions(auth.copyOfRange(37, auth.size))
        }
        var counter = 0u
        for (index in 33..36) counter = (counter shl 8) or (auth[index].toUInt() and 0xffu)
        return counter
    }

    private fun requireExtensions(bytes: ByteArray) {
        val reader = Reader(bytes)
        require(reader.length(5, 2) == 2) { "invalid App Attest extension map" }
        var categorySeen = false
        var versionSeen = false
        repeat(2) {
            when (reader.text(32)) {
                "validationCategory" -> {
                    require(!categorySeen) { "duplicate validationCategory" }
                    val encoded = reader.bytes(4)
                    require(encoded.size == 4) { "invalid validationCategory width" }
                    var category = 0u
                    for (index in encoded.indices) {
                        category = category or ((encoded[index].toUInt() and 0xffu) shl (8 * index))
                    }
                    require(category in 1u..6u || category == 10u) { "invalid validationCategory" }
                    categorySeen = true
                }
                "bundleVersion" -> {
                    require(!versionSeen) { "duplicate bundleVersion" }
                    val version = reader.text(128)
                    require(version.isNotEmpty() && '\u0000' !in version) { "invalid bundleVersion" }
                    versionSeen = true
                }
                else -> throw IllegalArgumentException("unknown App Attest extension")
            }
        }
        require(categorySeen && versionSeen && reader.atEnd()) { "incomplete or trailing App Attest extensions" }
    }

    /** Only canonical definite lengths are accepted, matching the native original parser. */
    private class Reader(private val original: ByteArray) {
        private var offset = 0
        fun atEnd(): Boolean = offset == original.size
        fun length(major: Int, maximum: Int): Int {
            val first = take(1)[0].toInt() and 0xff
            require(first ushr 5 == major) { "unexpected App Attest CBOR type" }
            val additional = first and 31
            val value = when (additional) {
                in 0..23 -> additional.toLong()
                24, 25, 26 -> {
                    val width = 1 shl (additional - 24)
                    var number = 0L
                    for (byte in take(width)) number = (number shl 8) or (byte.toLong() and 0xffL)
                    val minimum = when (width) { 1 -> 24L; 2 -> 256L; else -> 65_536L }
                    require(number >= minimum) { "noncanonical App Attest CBOR length" }
                    number
                }
                else -> throw IllegalArgumentException("unsupported App Attest CBOR length")
            }
            require(value <= maximum.toLong()) { "App Attest CBOR field exceeds its bound" }
            return value.toInt()
        }
        fun bytes(maximum: Int): ByteArray = take(length(2, maximum))
        fun text(maximum: Int): String {
            val bytes = take(length(3, maximum))
            return try {
                Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString()
            } catch (error: java.nio.charset.CharacterCodingException) {
                throw IllegalArgumentException("invalid App Attest CBOR UTF-8", error)
            }
        }
        private fun take(count: Int): ByteArray {
            require(count >= 0 && count <= original.size - offset) { "truncated App Attest CBOR" }
            val result = original.copyOfRange(offset, offset + count)
            offset += count
            return result
        }
    }
}
