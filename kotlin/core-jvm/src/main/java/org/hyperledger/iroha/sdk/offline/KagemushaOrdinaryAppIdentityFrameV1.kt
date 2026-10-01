// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest

/**
 * Structural ordinary-identity frame bounds shared by JVM and Android consumers.
 * These checks admit no native method, key custody, issuer verdict or installed owner.
 */
object KagemushaOrdinaryAppIdentityFrameV1 {
    const val CHUNK_BYTES = 65_536
    const val MAXIMUM_RAW_BYTES = 131_072
    private val domain = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)

    fun requireRequest(fields: List<ByteArray>) {
        when (phase(fields)) {
            1 -> { count(fields, 2); digest(fields[1]) }
            2, 4, 6, 7, 8, 9 -> { count(fields, 2); ticket(fields[1]) }
            3 -> { count(fields, 3); ticket(fields[1]); reference(fields[2]) }
            5 -> {
                count(fields, 5); ticket(fields[1]); point(fields[2])
                require(fields[3].size in 1..CHUNK_BYTES && fields[4].size <= CHUNK_BYTES)
                require(fields[4].isEmpty() || fields[3].size == CHUNK_BYTES) { "Noncanonical original evidence chunks" }
            }
            10 -> { count(fields, 3); ticket(fields[1]); require(number(fields[2]) in 0..1) }
            else -> error("Unknown native ordinary identity phase")
        }
    }

    fun requireResponse(request: List<ByteArray>, fields: List<ByteArray>) {
        when (phase(request)) {
            1 -> preparation(request[1], fields)
            2 -> {
                count(fields, 2); require(fields[0].size == 1)
                when (fields[0][0].toInt()) {
                    1 -> require(fields[1].isEmpty())
                    2 -> reference(fields[1])
                    else -> error("Invalid original generation disposition")
                }
            }
            3 -> { count(fields, 1); digest(fields[0]); same(fields[0], sha(request[2])) }
            4 -> {
                count(fields, 4); require(fields[0].size == 1)
                when (fields[0][0].toInt()) {
                    1 -> require(fields.drop(1).all { it.isEmpty() })
                    2 -> { point(fields[1]); digest(fields[2]); rawLength(fields[3]) }
                    else -> error("Invalid original attestation disposition")
                }
            }
            5 -> {
                count(fields, 2); fields.forEach(::digest)
                same(fields[0], sha(request[3] + request[4])); same(fields[1], sha(request[2]))
            }
            6, 8 -> { count(fields, 2); fields.forEach(::digest) }
            7 -> recovery(fields)
            9 -> count(fields, 0)
            10 -> {
                count(fields, 4); same(fields[0], request[2]); digest(fields[2])
                val total = rawLength(fields[3]); val offset = number(fields[0]) * CHUNK_BYTES
                require(offset < total && fields[1].size == minOf(CHUNK_BYTES, total - offset)) {
                    "Incomplete or substituted original evidence chunk"
                }
            }
            else -> error("Unknown native ordinary identity phase")
        }
    }

    private fun preparation(id: ByteArray, fields: List<ByteArray>) {
        count(fields, 8); ticket(fields[0]); digest(fields[3]); digest(fields[7])
        require(fields[4].size == 1 && fields[4][0].toInt() in 4..5)
        val c = fields[2]
        require(c.size == domain.size + 8 + 451 && c.copyOfRange(0, domain.size).contentEquals(domain))
        require(unsigned(c.copyOfRange(domain.size, domain.size + 8)) == BigInteger.valueOf(451))
        val body = c.copyOfRange(domain.size + 8, c.size)
        require(body[0] == 1.toByte() && body[1] == 0.toByte())
        require(body[2].toInt() == if (fields[4][0] == 5.toByte()) 1 else 2)
        repeat(13) { digest(body.copyOfRange(3 + it * 32, 35 + it * 32)) }
        same(body.copyOfRange(3, 35), id)
        require(!body.copyOfRange(35, 67).contentEquals(body.copyOfRange(67, 99)))
        require(unsigned(body.copyOfRange(419, 427)).signum() > 0 && unsigned(body.copyOfRange(427, 435)).signum() > 0)
        val issue = unsigned(body.copyOfRange(435, 443)); val expiry = unsigned(body.copyOfRange(443, 451))
        require(issue.signum() > 0 && expiry > issue && expiry.subtract(issue) <= BigInteger.valueOf(120_000))
        require(fields[1].size == 515)
        same(fields[1].copyOfRange(0, 451), body); same(fields[3], sha(c))
        if (fields[4][0] == 5.toByte()) {
            reference(fields[5]); require(fields[6].size == 1 && fields[6][0].toInt() in 1..3)
            require(fields[5].toString(Charsets.UTF_8) == KagemushaOrdinaryAppKeyAliasV1.originalAlias(c))
        } else {
            require(fields[5].isEmpty() && fields[6].contentEquals(byteArrayOf(0)))
        }
    }

    private fun recovery(fields: List<ByteArray>) {
        count(fields, 7); require(fields[0].size == 1 && fields[0][0].toInt() in 0..5)
        val stage = fields[0][0].toInt()
        if (stage >= 2) reference(fields[1]) else require(fields[1].isEmpty())
        if (stage >= 4) { point(fields[2]); digest(fields[3]); rawLength(fields[4]) }
        else require(fields[2].isEmpty() && fields[3].isEmpty() && number(fields[4]) == 0)
        if (stage == 5) { require(fields[5].size == 314); digest(fields[6]) }
        else require(fields[5].isEmpty() && fields[6].isEmpty())
    }

    private fun reference(bytes: ByteArray) {
        require(bytes.size in 1..255 && bytes.none { it == 0.toByte() })
        val value = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(bytes)).toString()
        require(value.isNotBlank() && value.toByteArray(Charsets.UTF_8).contentEquals(bytes))
    }
    private fun point(bytes: ByteArray) = require(bytes.size == 65 && bytes[0] == 4.toByte())
    private fun digest(bytes: ByteArray) = require(bytes.size == 32 && bytes.any { it != 0.toByte() })
    private fun ticket(bytes: ByteArray) = require(bytes.size == 8 && bytes.any { it != 0.toByte() })
    private fun count(fields: List<ByteArray>, count: Int) = require(fields.size == count)
    private fun rawLength(bytes: ByteArray): Int = number(bytes).also { require(it in 1..MAXIMUM_RAW_BYTES) }
    private fun number(bytes: ByteArray): Int {
        require(bytes.size == 4); return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).int
    }
    private fun phase(fields: List<ByteArray>): Int { require(fields.isNotEmpty()); return number(fields[0]) }
    private fun unsigned(bytes: ByteArray) = BigInteger(1, bytes.reversedArray())
    private fun same(left: ByteArray, right: ByteArray) = require(MessageDigest.isEqual(left, right)) {
        "Native ordinary identity projection substituted original scope"
    }
    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
}
