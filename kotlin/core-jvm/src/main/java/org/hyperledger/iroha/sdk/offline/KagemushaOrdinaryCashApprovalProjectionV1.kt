// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest

/** The signed purpose selected by the Native financial owner, never by a platform signer. */
enum class KagemushaOrdinaryCashApprovalPurposeV1(internal val tag: Int) {
    /** State approval before an outgoing candidate or terminal body exists. */
    PREPARE_TRANSITION(2),
    /** Approval for the separately selected terminal/publication stage. */
    MONETARY_TRANSITION(1),
}

/**
 * Independently retained public originals for ordinary cash approval correlation.
 *
 * These copied bytes do not authenticate a credential, reserve a nonce, retain a current
 * lease, authorize platform signing or restore a Native financial owner.
 */
class KagemushaOrdinaryCashApprovalOriginalBindingV1(
    operationId: ByteArray,
    accountBinding: ByteArray,
    authorityPolicyDigest: ByteArray,
    attestedKeyId: ByteArray,
    enrollmentDigest: ByteArray,
    normalizedGuardDigest: ByteArray,
    originalSelection: ByteArray,
) {
    internal val fields = listOf(operationId, accountBinding, authorityPolicyDigest, attestedKeyId,
        enrollmentDigest, normalizedGuardDigest).map {
        require(it.size == 32) { "Cash approval original identity width differs" }
        it.copyOf().also { copy ->
            require(copy.any { byte -> byte != 0.toByte() }) { "Cash approval original identity is absent" }
        }
    }
    internal val selection = originalSelection.let {
        require(it.size == 460) { "Cash approval original selection width differs" }
        it.copyOf()
    }
}

/**
 * Detached projection of exact ordinary cash W325 and S460 originals.
 *
 * Preparation and terminal approval have separate entry points and signed purposes.
 * SHA-256 consumes exactly all 460 original S bytes, whose domain and length are already
 * included. The transition statement digest inside S is a separate Native State binding.
 * Parsing does not verify State/Guard, issuer or platform evidence, trusted time, nonce
 * custody, current Integrity leases or monetary authority. Bootstrap uses its own holder.
 */
class KagemushaOrdinaryCashApprovalProjectionV1 private constructor(
    val purpose: KagemushaOrdinaryCashApprovalPurposeV1,
    w: ByteArray,
    s: ByteArray,
) {
    private val originalW = w.copyOf()
    private val originalS = s.copyOf()

    fun signingBytes(): ByteArray = originalW.copyOf()
    fun selectionBytes(): ByteArray = originalS.copyOf()
    fun operationId(): ByteArray = originalW.copyOfRange(53, 85)
    fun subjectSigningDigest(): ByteArray = sha256(originalS)
    fun approvalSigningDigest(): ByteArray = sha256(originalW)
    fun transitionStatementDigest(): ByteArray = originalS.copyOfRange(332, 364)
    fun operationTag(): Int = originalS[331].toInt()
    fun logicalIndexBefore(): BigInteger = unsigned(originalS, 428, 16)
    fun logicalIndexAfter(): BigInteger = unsigned(originalS, 444, 16)

    companion object {
        private val wDomain = "iroha:kagemusha:v1:app-operation-approval\u0000".toByteArray(Charsets.US_ASCII)
        private val sDomain = "iroha:kagemusha:v1:hardware-transition-selection\u0000".toByteArray(Charsets.US_ASCII)
        private val maximumLifetime = BigInteger.valueOf(120_000)

        /** Correlate purpose 2 and a cash selection with both candidate/body commitments absent. */
        @JvmStatic
        fun requirePreparation(
            originalW: ByteArray,
            originalS: ByteArray,
            binding: KagemushaOrdinaryCashApprovalOriginalBindingV1,
        ): KagemushaOrdinaryCashApprovalProjectionV1 =
            requireProjection(KagemushaOrdinaryCashApprovalPurposeV1.PREPARE_TRANSITION, originalW, originalS, binding)

        /** Correlate purpose 1 and the commitments required by the selected terminal operation. */
        @JvmStatic
        fun requireTerminal(
            originalW: ByteArray,
            originalS: ByteArray,
            binding: KagemushaOrdinaryCashApprovalOriginalBindingV1,
        ): KagemushaOrdinaryCashApprovalProjectionV1 =
            requireProjection(KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION, originalW, originalS, binding)

        /** Distinct incoming W2 data projection. It cannot construct a Native signing holder. */
        @JvmStatic fun requireIncomingPreparation(w: ByteArray, s: ByteArray,
            binding: KagemushaOrdinaryCashApprovalOriginalBindingV1): KagemushaOrdinaryCashApprovalProjectionV1 =
            requireProjection(KagemushaOrdinaryCashApprovalPurposeV1.PREPARE_TRANSITION, w, s, binding).also {
                require(it.operationTag() == 1 || it.operationTag() == 3)
                require(unsigned(w, 317, 8).subtract(unsigned(w, 309, 8)) <= BigInteger.valueOf(10_000))
            }

        /** Distinct ordinary incoming W1. The generic/OEM terminal grammar stays unchanged. */
        @JvmStatic fun requireIncomingTerminal(w: ByteArray, s: ByteArray,
            binding: KagemushaOrdinaryCashApprovalOriginalBindingV1): KagemushaOrdinaryCashApprovalProjectionV1 =
            requireProjection(KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION, w, s, binding, true)

        // Shared Rust fixture messages use the model's purpose-selected subject before ordinary
        // credential authentication. Production callers still provide retained public bindings.
        internal fun requireModelMessageShape(
            purpose: KagemushaOrdinaryCashApprovalPurposeV1,
            w: ByteArray,
            s: ByteArray,
        ): KagemushaOrdinaryCashApprovalProjectionV1 = requireProjection(purpose, w, s, null,
            purpose == KagemushaOrdinaryCashApprovalPurposeV1.MONETARY_TRANSITION &&
                (s.getOrNull(331)?.toInt() == 1 || s.getOrNull(331)?.toInt() == 3))

        private fun requireProjection(
            purpose: KagemushaOrdinaryCashApprovalPurposeV1,
            w: ByteArray,
            s: ByteArray,
            binding: KagemushaOrdinaryCashApprovalOriginalBindingV1?,
            ordinaryIncomingTerminal: Boolean = false,
        ): KagemushaOrdinaryCashApprovalProjectionV1 {
            require(w.size == 325 && s.size == 460) { "Cash approval original message width differs" }
            val originalW = w.copyOf()
            val originalS = s.copyOf()
            requireFrame(originalW, wDomain, 275)
            requireFrame(originalS, sDomain, 403)
            require(originalW[52].toInt() == purpose.tag) { "Signed cash approval purpose differs" }
            val operation = originalS[331].toInt()
            require(operation in 1..5) { "Cash approval requires a non-Bootstrap operation" }
            for (offset in listOf(59, 91, 123, 155, 187, 219, 251, 291, 332)) requireDigest(originalS, offset)
            require(unsigned(originalS, 283, 8).signum() > 0 && unsigned(originalS, 323, 8).signum() > 0) {
                "Cash selection epoch is absent"
            }
            val before = unsigned(originalS, 428, 16)
            val after = unsigned(originalS, 444, 16)
            val next = before.add(BigInteger.ONE)
            require(next.bitLength() <= 128 && next == after) { "Cash logical indices are not exact-next" }
            val candidatePresent = present(originalS, 364)
            val terminalPresent = present(originalS, 396)
            if (purpose == KagemushaOrdinaryCashApprovalPurposeV1.PREPARE_TRANSITION) {
                require(!candidatePresent && !terminalPresent) { "Preparation carries a candidate or terminal commitment" }
            } else {
                if (ordinaryIncomingTerminal) require(operation == 1 || operation == 3) {
                    "Ordinary incoming terminal requires Mint or Receive"
                }
                val commitmentsRequired = ordinaryIncomingTerminal || operation == 2 || operation == 4
                require(candidatePresent == commitmentsRequired && terminalPresent == commitmentsRequired) {
                    "Terminal commitments differ from the cash operation"
                }
            }
            repeat(8) { requireDigest(originalW, 53 + it * 32) }
            val issued = unsigned(originalW, 309, 8)
            val expires = unsigned(originalW, 317, 8)
            require(issued.signum() > 0 && expires > issued && expires.subtract(issued) <= maximumLifetime) {
                "Cash approval original interval differs"
            }
            requireSame(originalW.copyOfRange(245, 277), sha256(originalS))
            if (binding != null) {
                requireSame(originalS.copyOfRange(155, 187), originalW.copyOfRange(213, 245))
                for ((slot, original) in listOf(53, 117, 149, 181, 213, 277).zip(binding.fields)) {
                    requireSame(originalW.copyOfRange(slot, slot + 32), original)
                }
                requireSame(originalS, binding.selection)
            }
            return KagemushaOrdinaryCashApprovalProjectionV1(purpose, originalW, originalS)
        }

        private fun requireFrame(bytes: ByteArray, domain: ByteArray, bodyLength: Int) {
            require(bytes.copyOfRange(0, domain.size).contentEquals(domain) &&
                ByteBuffer.wrap(bytes, domain.size, 8).order(ByteOrder.LITTLE_ENDIAN).long == bodyLength.toLong() &&
                bytes[domain.size + 8] == 1.toByte() && bytes[domain.size + 9] == 0.toByte()) {
                "Cash approval canonical domain, length or version differs"
            }
        }

        private fun present(bytes: ByteArray, offset: Int): Boolean =
            (offset until offset + 32).any { bytes[it] != 0.toByte() }

        private fun requireDigest(bytes: ByteArray, offset: Int) = require(present(bytes, offset)) {
            "Cash approval identity is absent"
        }

        private fun unsigned(bytes: ByteArray, offset: Int, size: Int): BigInteger =
            BigInteger(1, bytes.copyOfRange(offset, offset + size).reversedArray())

        private fun requireSame(actual: ByteArray, original: ByteArray) = require(MessageDigest.isEqual(actual, original)) {
            "A retained cash approval public original was substituted"
        }

        private fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)
    }
}
