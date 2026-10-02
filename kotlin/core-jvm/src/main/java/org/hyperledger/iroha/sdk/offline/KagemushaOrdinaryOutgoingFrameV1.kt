// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest

/** Closed outgoing transport data. Framing cannot recreate a Native financial owner. */
object KagemushaOrdinaryOutgoingFrameV1 {
    const val MAXIMUM_REQUEST_BYTES = 192 * 1024
    const val MAXIMUM_SIGNED_BYTES = 32 * 1024
    const val MAXIMUM_DATA_BYTES = 128 * 1024
    const val MAXIMUM_AUTHORITY_BYTES = 128 * 1024 * 1024
    const val MAXIMUM_PROOF_BYTES = 64 * 1024 * 1024

    fun requireRequest(phase: Int, fields: List<ByteArray>) {
        when (phase) {
            5, 6, 9, 11, 12, 13, 16, 18 -> require(fields.isEmpty())
            8, 14, 15, 17 -> { require(fields.size == 1); digest(fields.single()) }
            10 -> { require(fields.size == 1 && fields.single().size in 1..4096) }
            7 -> {
                require(fields.size == 3 && fields[0].size in 1..MAXIMUM_SIGNED_BYTES &&
                    fields[1].size in 1..MAXIMUM_DATA_BYTES && fields[2].size in 1..MAXIMUM_AUTHORITY_BYTES)
            }
            else -> error("Unknown ordinary outgoing phase")
        }
    }

    fun requireResponse(phase: Int, fields: List<ByteArray>) {
        when (phase) {
            5, 7, 10, 12 -> { require(fields.size == 1); digest(fields.single()) }
            6, 13 -> {
                require(fields.size == 5 && (fields[0].contentEquals(byteArrayOf(0)) || fields[0].contentEquals(byteArrayOf(2))))
                require(fields[1].size in 1..MAXIMUM_REQUEST_BYTES && fields[2].size == 64 && fields[3].size in 1..MAXIMUM_PROOF_BYTES)
                digest(fields[4]); same(fields[4], sha(fields[1]))
            }
            8, 18 -> requireTerminal(fields)
            9, 11 -> {
                require(fields.size == 3 && fields[0].size == 1 && fields[0][0].toInt() in 0..2)
                if (fields[0][0] == 0.toByte()) require(fields[1].isEmpty() && fields[2].isEmpty())
                else require(fields[1].size in 1..4096 && fields[2].size in 1..MAXIMUM_SIGNED_BYTES)
            }
            14, 15, 16 -> require(fields.isEmpty())
            17 -> require(fields.size == 1 && fields.single().size in 1..MAXIMUM_PROOF_BYTES)
            else -> error("Unknown ordinary outgoing response")
        }
    }

    /** Fixed purpose1 W/S/C/key data. This never accepts the Bootstrap or preparation grammar. */
    fun requireTerminal(fields: List<ByteArray>) {
        require(fields.size == 14)
        digest(fields[0]); require(fields[1].size == 325 && fields[9].size == 460)
        require(fields[2].contentEquals(byteArrayOf(4)) || fields[2].contentEquals(byteArrayOf(5)))
        require(fields[3].size in 1..128)
        val alias = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(fields[3])).toString()
        require(alias.isNotBlank() && alias.toByteArray(Charsets.UTF_8).contentEquals(fields[3]))
        digest(fields[4]); digest(fields[6]); digest(fields[8]); digest(fields[12])
        require(fields[5].size == 65 && fields[5][0] == 4.toByte()); same(sha(fields[5]), fields[6])
        val cDomain = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)
        val c = fields[7]
        require(c.size == cDomain.size + 8 + 451 && c.copyOfRange(0, cDomain.size).contentEquals(cDomain))
        require(ByteBuffer.wrap(c, cDomain.size, 8).order(ByteOrder.LITTLE_ENDIAN).long == 451L)
        val body = c.copyOfRange(cDomain.size + 8, c.size)
        require(body[0] == 1.toByte() && body[1] == 0.toByte() &&
            body[2] == if (fields[2][0] == 5.toByte()) 1.toByte() else 2.toByte())
        fun cf(index: Int) = body.copyOfRange(3 + index * 32, 35 + index * 32)
        repeat(13) { digest(cf(it)) }; require(!MessageDigest.isEqual(cf(1), cf(2)))
        fun unsigned(at: Int, bytes: Int) = BigInteger(1, body.copyOfRange(at, at + bytes).reversedArray())
        require(unsigned(419, 8).signum() > 0 && unsigned(427, 8).signum() > 0 &&
            unsigned(435, 8).signum() > 0 && unsigned(443, 8) > unsigned(435, 8))
        same(sha(c), fields[4])
        if (fields[2][0] == 5.toByte()) {
            require(alias == KagemushaOrdinaryAppKeyAliasV1.originalAlias(c))
            require(fields[10].isEmpty() && fields[11].size == 1 && fields[11][0].toInt() in 1..2)
        } else {
            require(alias == java.util.Base64.getEncoder().encodeToString(fields[6]))
            require(fields[10].size == 4 && fields[11].contentEquals(byteArrayOf(0)))
        }
        val projection = KagemushaOrdinaryCashApprovalProjectionV1.requireTerminal(fields[1], fields[9],
            KagemushaOrdinaryCashApprovalOriginalBindingV1(fields[0], cf(3), cf(10), fields[6], fields[8],
                fields[1].copyOfRange(277, 309), fields[9]))
        require(projection.operationTag() == 2 || projection.operationTag() == 4)
        val s = fields[9]
        same(s.copyOfRange(59, 91), cf(6)); same(s.copyOfRange(187, 219), cf(4))
        same(s.copyOfRange(251, 283), cf(7)); same(s.copyOfRange(283, 291), body.copyOfRange(419, 427))
        same(s.copyOfRange(323, 331), body.copyOfRange(427, 435))
        require(fields[13].size in 1..MAXIMUM_SIGNED_BYTES) // FI bytes remain actual Native-admitted data.
    }
    private fun digest(raw: ByteArray) = require(raw.size == 32 && raw.any { it != 0.toByte() })
    private fun same(a: ByteArray, b: ByteArray) = require(MessageDigest.isEqual(a, b)) { "An outgoing original was substituted" }
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
}
