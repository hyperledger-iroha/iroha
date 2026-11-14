// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicLong

/** Exact future-profile admission DATA; only Native grants authority. */
class KagemushaSpartanAccountOriginalsV1(credential: ByteArray, enrollmentCertificate: ByteArray,
    account: ByteArray, assetScope: ByteArray) {
    private val originals: KagemushaWalletOpenOriginalsV1
    init {
        require(credential.size == 596 && enrollmentCertificate.size == 172)
        originals = KagemushaWalletOpenOriginalsV1(credential, enrollmentCertificate, account, assetScope)
    }
    internal fun frames(): List<ByteArray> = originals.frames()
}

/** Experimental proof backend, using the existing exclusive hardware custody owner. */
class KagemushaSpartanWalletV1 internal constructor(
    private val owner: KagemushaWalletV1,
    parameterId: ByteArray,
    slotId: ByteArray,
    private val platformOwner: Any,
) : Closeable {
    private val parameters = parameterId.copyOf()
    private val slot = slotId.copyOf()
    fun parameterId(): ByteArray = parameters.copyOf()
    fun slotId(): ByteArray = slot.copyOf()

    /** Native authenticates the complete witness and freezes a hardware-signable review. */
    fun prepare(witness: ByteArray, predecessorHead: ByteArray): KagemushaSpartanReviewV1 {
        require(witness.isNotEmpty() && witness.size <= MAX_RECORD && predecessorHead.size <= 1_048_576)
        val identity = slot.copyOf()
        val privateInput = witness.copyOf()
        val head = predecessorHead.copyOf()
        try {
            val reply = call { NativeSpartanBridge.prepare(owner.exchangeHandle(), identity, privateInput, head) }
            reply.requireShape(82, 291, token = true)
            return try {
                KagemushaSpartanReviewV1(this, reply.sequenceLow, KagemushaSpartanReviewProjectionV1(reply.bytes()))
            } catch (failure: Throwable) {
                try { discard(reply.sequenceLow) } catch (cleanup: Throwable) { failure.addSuppressed(cleanup) }
                throw failure
            }
        } finally { privateInput.fill(0) }
    }

    /** Reconcile durable selection and read the retained original. Retry only these exact Payment bytes. */
    fun output(operationId: ByteArray, part: KagemushaSpartanOutputV1): KagemushaSpartanRetainedOutputV1 {
        require(operationId.size == 32)
        val identity = slot.copyOf(); val operation = operationId.copyOf()
        val reply = call { NativeSpartanBridge.output(owner.exchangeHandle(), identity, operation, part.ordinal) }
        return KagemushaSpartanRetainedOutputV1.from(reply, part)
    }

    /** Full native proof verification; the returned Request is bound to these exact Payment bytes. */
    fun verifyPayment(payment: ByteArray): KagemushaSpartanVerifiedPaymentV1 {
        require(payment.isNotEmpty() && payment.size <= 65_536)
        val original = payment.copyOf()
        val reply = call { NativeSpartanBridge.verifyPayment(owner.exchangeHandle(), original) }
        reply.requireShape(83, 514)
        return KagemushaSpartanVerifiedPaymentV1(reply.bytes())
    }

    internal fun execute(token: Long): KagemushaSpartanExecutionV1 {
        val reply = call { NativeSpartanBridge.execute(owner.exchangeHandle(), token) }
        return KagemushaSpartanExecutionV1(reply)
    }

    internal fun discard(token: Long) {
        // Control cleanup must not queue behind the operation it cancels.
        checkedSpartanReply(NativeSpartanBridge.discard(owner.exchangeCleanupHandle(), token)).requireShape(84, 0)
    }

    /** Closing retains committed custody and exact outputs. */
    override fun close() = owner.close()

    companion object {
        internal const val MAX_RECORD = 4 * 1024 * 1024
        internal fun call(action: () -> KagemushaSpartanReplyV1?): KagemushaSpartanReplyV1 =
            checkedSpartanReply(KagemushaWalletNativeWorkerV1.shared.run { action() })
    }
}

/** Retained installed backend before the existing Ed25519 account signs Native's challenge. */
class KagemushaSpartanRuntimeV1 private constructor(
    private var owner: KagemushaWalletV1?,
    private val platformOwner: Any,
    parameterId: ByteArray,
) : Closeable {
    private val parameters = parameterId.copyOf()
    fun parameterId(): ByteArray = parameters.copyOf()
    /** Return the exact Native challenge. No payment key is created or replaced. */
    @Synchronized fun beginAccount(originals: KagemushaSpartanAccountOriginalsV1): ByteArray {
        val handle = requireNotNull(owner) { "Runtime was transferred or closed" }.exchangeHandle()
        val frames = originals.frames()
        val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.beginAccount(handle, frames[0], frames[1], frames[2], frames[3]) }
        reply.requireShape(15, 32, token = true)
        val challenge = reply.bytes()
        if (reply.sequenceLow != handle || challenge.all { it == 0.toByte() }) invalidSpartanOutput()
        return challenge
    }
    /** Finish actual Native account admission and transfer this exact custody owner once. */
    @Synchronized fun authorizeAccount(signature: ByteArray): KagemushaSpartanWalletV1 {
        require(signature.size == 64)
        val owner = requireNotNull(owner) { "Runtime was transferred or closed" }
        val handle = owner.exchangeHandle(); val original = signature.copyOf()
        val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.authorizeAccount(handle, original) }
        reply.requireShape(85, 64, token = true)
        val bytes = reply.bytes(); val slot = bytes.copyOfRange(32, 64)
        if (reply.sequenceLow != handle || !bytes.copyOfRange(0, 32).contentEquals(parameters) || slot.all { it == 0.toByte() }) invalidSpartanOutput()
        val wallet = KagemushaSpartanWalletV1(owner, parameters, slot, platformOwner)
        this.owner = null
        return wallet
    }
    @Synchronized override fun close() { owner?.close(); owner = null }
    internal companion object {
        fun install(handle: Long, platformOwner: Any, programBundle: ByteArray,
            signedManifest: ByteArray, signature: ByteArray): KagemushaSpartanRuntimeV1 {
            require(programBundle.isNotEmpty() && programBundle.size <= 256 * 1024 * 1024)
            require(signedManifest.isNotEmpty() && signedManifest.size <= 16_384 && signature.size == 64)
            val programs = programBundle.copyOf(); val manifest = signedManifest.copyOf(); val signed = signature.copyOf()
            val reply = KagemushaSpartanWalletV1.call { NativeSpartanBridge.install(handle, programs, manifest, signed) }
            reply.requireShape(81, 32, token = true)
            if (reply.sequenceLow != handle) invalidSpartanOutput()
            // This private holder provides existing close/quarantine mechanics; it is never exposed as a legacy wallet.
            return KagemushaSpartanRuntimeV1(KagemushaWalletV1(handle), platformOwner, reply.bytes())
        }
    }
}

enum class KagemushaSpartanOutputV1 { HEAD, PAYMENT, COMPLETION }
enum class KagemushaSpartanOperationV1 { BOOTSTRAP, LOAD, SEND, RECEIVE }

sealed class KagemushaSpartanRetainedOutputV1 {
    data object Unknown : KagemushaSpartanRetainedOutputV1()
    data object Pending : KagemushaSpartanRetainedOutputV1()
    data object Archived : KagemushaSpartanRetainedOutputV1()
    data object DeliveryLoss : KagemushaSpartanRetainedOutputV1()
    class Released internal constructor(bytes: ByteArray) : KagemushaSpartanRetainedOutputV1() {
        private val original = bytes.copyOf()
        fun bytes(): ByteArray = original.copyOf()
    }
    internal companion object {
        fun from(reply: KagemushaSpartanReplyV1, part: KagemushaSpartanOutputV1): KagemushaSpartanRetainedOutputV1 {
            reply.requireShape(reply.status, if (reply.status == 1) null else 0)
            return when (reply.status) {
                0 -> Unknown
                2 -> Pending
                4 -> Archived
                5 -> DeliveryLoss
                1 -> {
                    val bytes = reply.bytes()
                    // Completed Bootstrap, Load and Receive have no outgoing Payment.
                    if (bytes.isEmpty() && part != KagemushaSpartanOutputV1.PAYMENT) invalidSpartanOutput()
                    Released(bytes)
                }
                else -> invalidSpartanOutput()
            }
        }
    }
}

/** Native's exact review, suitable for confirmation UI. No caller-supplied validity flag. */
class KagemushaSpartanReviewProjectionV1 internal constructor(bytes: ByteArray) {
    private val original = bytes.copyOf()
    val operation: KagemushaSpartanOperationV1
    val amount: KagemushaWalletUInt128V1
    val fee: KagemushaWalletUInt128V1
    init {
        if (original.size != 291 || original[0] != 1.toByte() || original[1] != 0.toByte()) invalidSpartanOutput()
        operation = KagemushaSpartanOperationV1.entries.getOrNull((original[2].toInt() and 255) - 1) ?: invalidSpartanOutput()
        val view = ByteBuffer.wrap(original).order(ByteOrder.LITTLE_ENDIAN)
        amount = KagemushaWalletUInt128V1(view.getLong(131), view.getLong(139))
        fee = KagemushaWalletUInt128V1(view.getLong(147), view.getLong(155))
    }
    fun schemeId(): ByteArray = original.copyOfRange(3, 35)
    fun walletId(): ByteArray = original.copyOfRange(35, 67)
    fun operationId(): ByteArray = original.copyOfRange(67, 99)
    fun capsuleDigest(): ByteArray = original.copyOfRange(99, 131)
    fun receiverWalletId(): ByteArray = original.copyOfRange(163, 195)
    fun receiverAccountId(): ByteArray = original.copyOfRange(195, 227)
    fun beforeHead(): ByteArray = original.copyOfRange(227, 259)
    fun afterHead(): ByteArray = original.copyOfRange(259, 291)
    override fun toString(): String = "KagemushaSpartanReviewProjectionV1([REDACTED])"
}

/** Review permanently bound to one Native owner and capsule; pending execution is retryable. */
class KagemushaSpartanReviewV1 internal constructor(
    private val owner: KagemushaSpartanWalletV1, token: Long,
    val projection: KagemushaSpartanReviewProjectionV1,
) : Closeable {
    private val retained = AtomicLong(token)
    /** Invoke only after fresh hardware-backed biometric/device-credential confirmation of this exact review. */
    fun execute(): KagemushaSpartanExecutionV1 {
        val token = retained.get()
        check(token > 0 && retained.compareAndSet(token, -token)) { "Review is consumed or executing" }
        try {
            val result = owner.execute(token)
            retained.set(if (result.status == KagemushaSpartanExecutionV1.Status.PENDING) token else 0)
            return result
        } catch (failure: Throwable) {
            // A failed transport/allocation may hide Selected. Only retry the same frozen token.
            retained.set(token)
            throw failure
        }
    }
    override fun close() {
        val token = retained.get()
        if (token == 0L) return
        check(token > 0 && retained.compareAndSet(token, -token)) { "Review is executing" }
        try { owner.discard(token); retained.set(0) }
        catch (failure: Throwable) { retained.set(token); throw failure }
    }
}

/** Uncertain statuses require recovery of the same operation, never a replacement debit. */
class KagemushaSpartanExecutionV1 internal constructor(reply: KagemushaSpartanReplyV1) {
    enum class Status { RELEASED, PENDING, NOT_PERFORMED, ARCHIVED, DELIVERY_LOSS }
    val status = Status.entries.getOrNull(reply.status - 1) ?: invalidSpartanOutput()
    val sequence = KagemushaWalletUInt128V1(reply.sequenceLow, reply.sequenceHigh)
    val detail = reply.detail
    private val retained = reply.bytes()
    init { if ((status == Status.RELEASED) != retained.isNotEmpty()) invalidSpartanOutput() }
    fun completion(): ByteArray? = retained.takeIf { status == Status.RELEASED }?.copyOf()
}

class KagemushaSpartanVerifiedPaymentV1 internal constructor(bytes: ByteArray) {
    private val original = bytes.copyOf()
    init { if (original.size != 514) invalidSpartanOutput() }
    fun digest(): ByteArray = original.copyOfRange(0, 32)
    fun request(): ByteArray = original.copyOfRange(32, 514)
}

// Distinct JNI domain: the production wallet status contract is unchanged.
internal class KagemushaSpartanReplyV1(
    @JvmField val status: Int, @JvmField val reason: Int, @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long, @JvmField val sequenceHigh: Long, @JvmField val detail: Int,
    bytes: ByteArray,
) {
    private val retained = bytes.copyOf()
    fun bytes(): ByteArray = retained.copyOf()
    fun requireShape(expectedStatus: Int, count: Int?, token: Boolean = false) {
        if (status != expectedStatus || (count != null && retained.size != count) || sequenceHigh != 0L || detail != 0 ||
            (if (token) sequenceLow <= 0 else sequenceLow != 0L)) invalidSpartanOutput()
    }
    fun checked(): KagemushaSpartanReplyV1 {
        if (status < 0) throw KagemushaWalletExceptionV1(status, reason, platformCode)
        if (reason != -1 || platformCode != 0 || retained.size > KagemushaSpartanWalletV1.MAX_RECORD) invalidSpartanOutput()
        return this
    }
}
private fun checkedSpartanReply(reply: KagemushaSpartanReplyV1?): KagemushaSpartanReplyV1 = reply?.checked() ?: invalidSpartanOutput()
private fun invalidSpartanOutput(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)

internal object NativeSpartanBridge {
    @JvmStatic external fun install(handle: Long, bundle: ByteArray, manifest: ByteArray, signature: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun prepare(handle: Long, slot: ByteArray, witness: ByteArray, head: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun execute(handle: Long, token: Long): KagemushaSpartanReplyV1?
    @JvmStatic external fun output(handle: Long, slot: ByteArray, operation: ByteArray, part: Int): KagemushaSpartanReplyV1?
    @JvmStatic external fun verifyPayment(handle: Long, payment: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun discard(handle: Long, token: Long): KagemushaSpartanReplyV1?
    @JvmStatic external fun beginAccount(handle: Long, credential: ByteArray, certificate: ByteArray, account: ByteArray, asset: ByteArray): KagemushaSpartanReplyV1?
    @JvmStatic external fun authorizeAccount(handle: Long, signature: ByteArray): KagemushaSpartanReplyV1?
}
