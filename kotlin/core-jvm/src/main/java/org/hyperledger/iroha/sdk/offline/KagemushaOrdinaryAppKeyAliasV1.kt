// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.security.MessageDigest

/** Canonical naming/correlation only. A decoded C cannot admit an issuer, native owner or key. */
object KagemushaOrdinaryAppKeyAliasV1 {
    private val challengeDomain = "iroha:kagemusha:v1:ordinary-app-enrollment-challenge\u0000".toByteArray(Charsets.US_ASCII)
    private val aliasDomain = "iroha:kagemusha:v1:ordinary-app-key-alias\u0000".toByteArray(Charsets.US_ASCII)

    /** Sole ordinary identity alias: prefix plus lowercase SHA256(aliasDomain || full original C451). */
    @JvmStatic fun originalAlias(originalC: ByteArray): String {
        val c = originalC.copyOf()
        require(c.size == challengeDomain.size + 8 + 451 && c.copyOfRange(0, challengeDomain.size).contentEquals(challengeDomain))
        val body = challengeDomain.size + 8
        fun unsigned(offset: Int, width: Int) = BigInteger(1, c.copyOfRange(offset, offset + width).reversedArray())
        require(unsigned(challengeDomain.size, 8) == BigInteger.valueOf(451) &&
            c[body] == 1.toByte() && c[body + 1] == 0.toByte() && c[body + 2] == 1.toByte())
        repeat(13) { field -> require(c.copyOfRange(body + 3 + field * 32, body + 35 + field * 32).any { it != 0.toByte() }) }
        require(!c.copyOfRange(body + 35, body + 67).contentEquals(c.copyOfRange(body + 67, body + 99)))
        require(unsigned(body + 419, 8).signum() > 0 && unsigned(body + 427, 8).signum() > 0)
        val issued = unsigned(body + 435, 8); val expires = unsigned(body + 443, 8)
        require(issued.signum() > 0 && expires > issued && expires.subtract(issued) <= BigInteger.valueOf(120_000))
        val digest = MessageDigest.getInstance("SHA-256").run { update(aliasDomain); digest(c) }
        return "kagemusha-ordinary-app-v1-" + digest.joinToString("") { "%02x".format(it.toInt() and 0xff) }
    }
}
