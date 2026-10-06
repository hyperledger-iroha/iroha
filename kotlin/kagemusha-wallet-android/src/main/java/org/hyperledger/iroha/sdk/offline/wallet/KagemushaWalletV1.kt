// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong
import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge

/** Native failure; -4 means the authenticated operation/Λ/Ω artifact loader is unavailable. */
class KagemushaWalletExceptionV1(
    @JvmField val status: Int,
    @JvmField val reason: Int = -1,
    @JvmField val platformCode: Int = 0,
) : IllegalStateException("KAGEMUSHA native status $status (reason $reason, platform $platformCode)") {
    companion object {
        /** There is deliberately no structural verifier or software-key fallback. */
        const val ARTIFACTS_UNAVAILABLE = -4
        /** Missing, stale or unauthenticated native bridge library. */
        const val BRIDGE_UNAVAILABLE = -101
    }
}

/** Explicit custody outcome. Only COMPLETE permits the original result to be delivered. */
class KagemushaWalletCallV1 internal constructor(
    @JvmField val status: Int,
    @JvmField val reason: Int,
    @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long,
    @JvmField val sequenceHigh: Long,
    @JvmField val detail: Int,
    bytes: ByteArray,
) {
    private val retainedBytes = bytes.copyOf()
    /** Exact canonical bytes returned by Rust; this accessor never assembles or signs again. */
    fun bytes(): ByteArray = retainedBytes.copyOf()
    override fun toString(): String = "KagemushaWalletCallV1(status=$status, bytes=[REDACTED])"
    companion object {
        const val UNKNOWN = 0
        const val COMPLETE = 1
        const val PENDING = 2
        const val NOT_PERFORMED = 3
        const val ARCHIVED = 4
        const val DELIVERY_DATA_LOSS = 5
        const val IDLE = 6
        const val CAUGHT_UP = 7
        const val CHECKPOINT = 8
        const val FOLDED = 9
        const val CREDIT_STATUS = 10
    }
}

/**
 * One exclusive native wallet owner. Calls may run on workers; activity and payment arrival
 * can preempt folding because this wrapper does not hold a managed lock across a native call.
 * Closing joins cooperative folding and retains every already committed payment in custody.
 *
 * [open] currently reports ArtifactsUnavailable until the authenticated native proof loader
 * is complete. The platform adapter and an ordinary filesystem are not monetary authority.
 */
class KagemushaWalletV1 private constructor(handle: Long) : Closeable {
    private val owner = AtomicLong(handle)
    private fun handle(): Long = owner.get().takeIf { it > 0 } ?: throw KagemushaWalletExceptionV1(-2)
    private fun call(operation: Int, first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf()): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeV1.call(handle(), operation, first.copyOf(), second.copyOf())
            ?: throw KagemushaWalletExceptionV1(-100)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value
    }
    /** Commit canonical Norito FrozenTransition bytes. Rust verifies every relation and binding. */
    fun commit(frozenTransition: ByteArray): KagemushaWalletCallV1 {
        require(frozenTransition.size <= 264_192) { "FrozenTransition exceeds its capsule/credential envelope bound" }
        return call(0, frozenTransition)
    }
    /** Retrieve exact retained bytes; unknown, pending and delivery loss remain distinct. */
    fun retry(operationId: ByteArray): KagemushaWalletCallV1 { word(operationId); return call(1, operationId) }
    /** Reconcile and finish the selected operation without a second debit. */
    fun resume(): KagemushaWalletCallV1 = call(2)
    /** Compute at most one checkpoint. Run off the UI thread; activity enables background work. */
    fun foldOnce(): KagemushaWalletCallV1 = call(3)
    /** Canonical CreditStatus for the immutable first Payment identity. */
    fun creditStatus(creditId: ByteArray, paymentDigest: ByteArray): KagemushaWalletCallV1 {
        word(creditId); word(paymentDigest); return call(4, creditId, paymentDigest)
    }
    /** Foreground or charging enables folding; leaving both cancels at a cooperative boundary. */
    fun setActivity(foreground: Boolean, charging: Boolean) {
        checked(KagemushaWalletNativeV1.activity(handle(), if (foreground) 1 else 0, if (charging) 1 else 0))
    }
    /** Release exclusive native custody. Never deletes key material, markers or retained bytes. */
    override fun close() {
        val handle = owner.getAndSet(0)
        if (handle > 0) checked(KagemushaWalletNativeV1.close(handle))
    }
    override fun toString(): String = "KagemushaWalletV1(owner=[REDACTED])"
    companion object {
        private fun word(bytes: ByteArray) { require(bytes.size == 32) { "identity must be exactly32 bytes" } }
        private fun checked(status: Int) { if (status != 0) throw KagemushaWalletExceptionV1(status) }
        /**
         * Open through the authenticated SDK library and artifact owner. Every identity is
         * exactly32 nonzero bytes. No existing key is generated, replaced or deleted by this API.
         * TODO(G3/G4): native authenticated loader currently returns ArtifactsUnavailable.
         */
        @JvmStatic
        fun open(platform: KagemushaWalletAndroidPlatformV1, slot: ByteArray, scheme: ByteArray, wallet: ByteArray, artifact: ByteArray): KagemushaWalletV1 {
            for (id in listOf(slot, scheme, wallet, artifact)) { word(id); require(id.any { it != 0.toByte() }) { "identity must be nonzero" } }
            if (!PrivacyNativeBridge.isNativeAvailable()) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
            val handle = try {
                if (KagemushaWalletNativeV1.revision() != 1) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
                KagemushaWalletNativeV1.open(platform, slot.copyOf(), scheme.copyOf(), wallet.copyOf(), artifact.copyOf())
            } catch (_: LinkageError) { throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE) }
            if (handle <= 0) throw KagemushaWalletExceptionV1(if (handle < 0) handle.toInt() else -100)
            return KagemushaWalletV1(handle)
        }
    }
}

/** JNI declarations share the current authenticated native library loader. */
internal object KagemushaWalletNativeV1 {
    @JvmStatic external fun revision(): Int
    @JvmStatic external fun open(platform: KagemushaWalletAndroidPlatformV1, slot: ByteArray, scheme: ByteArray, wallet: ByteArray, artifact: ByteArray): Long
    @JvmStatic external fun close(handle: Long): Int
    @JvmStatic external fun activity(handle: Long, foreground: Int, charging: Int): Int
    @JvmStatic external fun call(handle: Long, operation: Int, first: ByteArray, second: ByteArray): KagemushaWalletCallV1?
}
