// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.KagemushaSelectionFrameV1

/** Shape/correlation checks only. No decoded field establishes a native owner or issuer admission. */
internal object KagemushaAppOwnedHardwareFrameV1 {
    private val cDomain = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)
    private val wDomain = "iroha:kagemusha:v1:app-operation-approval\u0000".toByteArray(Charsets.US_ASCII)
    private val eDomain = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII)

    fun requireRequest(fields: List<ByteArray>) {
        when (phase(fields)) {
            1 -> { count(fields, 2); digest(fields[1]) }
            2, 4, 5, 6, 7 -> { count(fields, 2); ticket(fields[1]) }
            3 -> { count(fields, 3); ticket(fields[1]); require(fields[2].size in 1..4096) }
            else -> error("Unknown native app-signing phase")
        }
    }

    fun requireResponse(method: KagemushaCoreCoordinatorMethodV1, request: List<ByteArray>, fields: List<ByteArray>) {
        when (phase(request)) {
            1 -> prepare(method, request[1], fields)
            2 -> state(fields, fenced = true, method = method, ticket = request[1])
            3 -> { count(fields, 1); digest(fields[0]); equal(fields[0], sha(request[2])) }
            4 -> { count(fields, 1); receiptShape(fields[0], method, request[1]) }
            5 -> state(fields, fenced = false, method = method, ticket = request[1])
            6 -> { count(fields, 2); fields.forEach(::digest) }
            7 -> count(fields, 0)
            else -> error("Unknown native app-signing phase")
        }
    }

    private fun prepare(method: KagemushaCoreCoordinatorMethodV1, id: ByteArray, fields: List<ByteArray>) {
        count(fields, 14); ticket(fields[0]); digest(fields[4]); digest(fields[6]); digest(fields[9]); digest(fields[12])
        require(fields[2].size == 1 && fields[2][0].toInt() in 4..5) { "Unknown native app platform" }
        val alias = fields[3]
        require(alias.size in 1..255 && alias.none { it == 0.toByte() }) { "Invalid original app-key alias" }
        val decoded = Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT).decode(ByteBuffer.wrap(alias)).toString()
        require(decoded.isNotBlank() && decoded.toByteArray(Charsets.UTF_8).contentEquals(alias))
        require(fields[5].size == 65 && fields[5][0] == 4.toByte()) { "Invalid native P-256 point" }
        equal(sha(fields[5]), fields[6])
        val c = body(fields[7], cDomain, 451, if (fields[2][0] == 5.toByte()) 1 else 2, 13)
        val cField = { index: Int -> c.copyOfRange(3 + index * 32, 35 + index * 32) }
        require(!cField(1).contentEquals(cField(2))) { "Repeated original enrollment nonce" }
        require(unsigned(c.copyOfRange(419, 427)).signum() > 0 && unsigned(c.copyOfRange(427, 435)).signum() > 0)
        interval(c.copyOfRange(435, 443), c.copyOfRange(443, 451))
        equal(sha(fields[7]), fields[4])
        if (fields[2][0] == 5.toByte()) {
            require(decoded == KagemushaOrdinaryAppKeyAliasV1.originalAlias(fields[7])) { "Native ordinary Android alias differs from original C451" }
        } else {
            require(decoded == java.util.Base64.getEncoder().encodeToString(fields[6])) { "Native App Attest key reference is not the exact canonical key ID" }
        }
        if (fields[2][0] == 5.toByte()) {
            require(fields[10].isEmpty() && fields[11].size == 1 && fields[11][0].toInt() in 1..3)
        } else {
            require(fields[10].size == 4 && fields[11].contentEquals(byteArrayOf(0)))
        }
        val w = method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL
        val message = body(fields[1], if (w) wDomain else eDomain, if (w) 275 else 371, 1, if (w) 8 else 11)
        val selected = { index: Int -> message.copyOfRange(3 + index * 32, 35 + index * 32) }
        equal(selected(0), id)
        if (w) {
            digest(fields[8]); require(fields[13].size == 460)
            equal(selected(2), cField(3)); equal(selected(3), cField(10))
            equal(selected(4), fields[6]); equal(selected(5), fields[8]); equal(selected(6), sha(fields[13]))
            interval(message.copyOfRange(259, 267), message.copyOfRange(267, 275))
            val s = fields[13]
            val before = s.copyOfRange(428, 444); val after = s.copyOfRange(444, 460)
            require(unsigned(after) == unsigned(before).add(BigInteger.ONE) && unsigned(after).bitLength() <= 128)
            KagemushaSelectionFrameV1.requireExact(s, cField(5), before, after)
            equal(s.copyOfRange(155, 187), fields[8])
            equal(s.copyOfRange(323, 331), c.copyOfRange(427, 435))
            equal(s.copyOfRange(59, 91), cField(6)); equal(s.copyOfRange(187, 219), cField(4))
            equal(s.copyOfRange(251, 283), cField(7)); equal(s.copyOfRange(283, 291), c.copyOfRange(419, 427))
        } else {
            require(fields[8].isEmpty() && fields[13].isEmpty()) { "Pending possession is not a completed credential or financial S" }
            equal(selected(0), sha(fields[7])); equal(selected(1), cField(1)); equal(selected(2), cField(2))
            equal(selected(3), cField(3)); equal(selected(4), cField(4)); equal(selected(5), cField(10))
            equal(selected(6), cField(6)); equal(selected(7), cField(7)); equal(selected(8), cField(5)); equal(selected(9), fields[6])
            require(!selected(1).contentEquals(selected(2)))
            interval(message.copyOfRange(355, 363), message.copyOfRange(363, 371))
            equal(message.copyOfRange(355, 363), c.copyOfRange(435, 443))
            equal(message.copyOfRange(363, 371), c.copyOfRange(443, 451))
        }
    }

    private fun body(message: ByteArray, domain: ByteArray, bodyBytes: Int, purpose: Int, count: Int): ByteArray {
        require(message.size == domain.size + 8 + bodyBytes && message.copyOfRange(0, domain.size).contentEquals(domain))
        require(unsigned(message.copyOfRange(domain.size, domain.size + 8)) == BigInteger.valueOf(bodyBytes.toLong()))
        val body = message.copyOfRange(domain.size + 8, message.size)
        require(body[0] == 1.toByte() && body[1] == 0.toByte() && body[2].toInt() == purpose)
        repeat(count) { digest(body.copyOfRange(3 + it * 32, 35 + it * 32)) }
        return body
    }

    private fun state(fields: List<ByteArray>, fenced: Boolean, method: KagemushaCoreCoordinatorMethodV1, ticket: ByteArray) {
        count(fields, 3); require(fields[0].size == 1)
        val state = fields[0][0].toInt()
        if ((fenced && state == 1) || (!fenced && state == 0)) {
            require(fields[1].isEmpty() && fields[2].isEmpty())
        } else if ((fenced && state == 2) || (!fenced && state == 1)) {
            require(fields[1].size in 1..4096 && fields[2].isEmpty())
        } else if ((fenced && state == 3) || (!fenced && state == 2)) {
            require(fields[1].size in 1..4096); receiptShape(fields[2], method, ticket)
        } else error("Invalid native original app-signing state")
    }

    private fun receiptShape(value: ByteArray, method: KagemushaCoreCoordinatorMethodV1, selectedTicket: ByteArray) {
        require(value.size == 184 && value.copyOfRange(0, 8).contentEquals("KGMAPP1\u0000".toByteArray(Charsets.US_ASCII)))
        require(value[8] == 1.toByte() && value[9] == 0.toByte() && value[10].toInt() in 1..2)
        require(value[10].toInt() == if (method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL) 1 else 2)
        equal(value.copyOfRange(11, 19), selectedTicket)
        ticket(value.copyOfRange(11, 19)); repeat(5) { digest(value.copyOfRange(19 + it * 32, 51 + it * 32)) }
        require(value[179].toInt() in 0..1)
        if (value[179] == 0.toByte()) require(value.copyOfRange(180, 184).all { it == 0.toByte() })
    }
    private fun phase(fields: List<ByteArray>): Int {
        require(fields.isNotEmpty() && fields[0].size == 4)
        return ByteBuffer.wrap(fields[0]).order(ByteOrder.LITTLE_ENDIAN).int
    }
    private fun count(fields: List<ByteArray>, expected: Int) = require(fields.size == expected)
    private fun ticket(value: ByteArray) = require(value.size == 8 && value.any { it != 0.toByte() })
    private fun digest(value: ByteArray) = require(value.size == 32 && value.any { it != 0.toByte() })
    private fun equal(a: ByteArray, b: ByteArray) = require(MessageDigest.isEqual(a, b)) { "Native app projection substituted original scope" }
    private fun sha(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)
    private fun unsigned(value: ByteArray) = BigInteger(1, value.reversedArray())
    private fun interval(issue: ByteArray, expiry: ByteArray) {
        val issued = unsigned(issue); val expires = unsigned(expiry)
        require(issued.signum() > 0 && expires > issued && expires.subtract(issued) <= BigInteger.valueOf(120_000))
    }
}
