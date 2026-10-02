// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/**
 * Exact existing Native `TransitionProofStatementV1` digest preimage, joined to cash S.
 *
 * The sole model transcript is BE64(40), its 40-byte NUL-terminated domain, BE64(1089),
 * and all 1089 body bytes. Body integers are little endian. This projection accepts that
 * complete original; it provides no alternative encoder, 93-cell reconstruction, State
 * proof verifier, Guard authentication, issuer admission or current financial owner.
 */
class KagemushaOrdinaryTransitionStatementProjectionV1 private constructor(preimage: ByteArray) {
    private val original = preimage.copyOf()

    fun canonicalPreimage(): ByteArray = original.copyOf()
    fun digest(): ByteArray = sha256(original)
    fun operationTag(): Int = original[56 + 132].toInt()

    companion object {
        private val domain = "iroha:kagemusha:v1:transition-statement\u0000".toByteArray(Charsets.US_ASCII)

        /**
         * Hash every byte of the exact model original and compare it with retained cash S.
         *
         * Common release/lane/profile/policy fields are correlated without converting the
         * State's logical sequence or journal revision into S's independent secure indices.
         * Native alone verifies the financial relation and both State/Guard proof parities.
         */
        @JvmStatic
        fun requireOriginal(
            canonicalModelPreimage: ByteArray,
            cashApproval: KagemushaOrdinaryCashApprovalProjectionV1,
        ): KagemushaOrdinaryTransitionStatementProjectionV1 {
            require(canonicalModelPreimage.size == 1145) { "Transition statement original width differs" }
            val original = canonicalModelPreimage.copyOf()
            require(ByteBuffer.wrap(original, 0, 8).order(ByteOrder.BIG_ENDIAN).long == 40L &&
                original.copyOfRange(8, 48).contentEquals(domain) &&
                ByteBuffer.wrap(original, 48, 8).order(ByteOrder.BIG_ENDIAN).long == 1089L) {
                "Transition statement canonical domain or length differs"
            }
            require(original[56] == 1.toByte() && original[57] == 0.toByte() &&
                original[58] == 1.toByte() && original[59] == 0.toByte()) {
                "Transition statement first-release version differs"
            }
            val operation = original[56 + 132].toInt()
            require(operation in 1..5 && operation == cashApproval.operationTag()) {
                "Transition statement operation differs from cash approval"
            }
            requireSame(sha256(original), cashApproval.transitionStatementDigest())
            val selection = cashApproval.selectionBytes()
            // These are the exact fixed model body offsets in commitments.rs, not a second
            // serialization path. Every other body byte remains bound by the full SHA above.
            requireSame(original.copyOfRange(56 + 373, 56 + 405), original.copyOfRange(56 + 405, 56 + 437))
            for ((bodyOffset, selectionOffset) in listOf(405 to 59, 501 to 251, 541 to 187, 573 to 219)) {
                requireSame(original.copyOfRange(56 + bodyOffset, 56 + bodyOffset + 32),
                    selection.copyOfRange(selectionOffset, selectionOffset + 32))
            }
            requireSame(original.copyOfRange(56 + 533, 56 + 541), selection.copyOfRange(283, 291))
            return KagemushaOrdinaryTransitionStatementProjectionV1(original)
        }

        private fun requireSame(actual: ByteArray, expected: ByteArray) = require(MessageDigest.isEqual(actual, expected)) {
            "Transition statement differs from the retained cash approval original"
        }

        private fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)
    }
}
