package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge

/** Exact original frames, authenticated exclusively by Native. Defensive copies are retained. */
class KagemushaWalletOpenOriginalsV1(credential: ByteArray, enrollmentCertificates: ByteArray, account: ByteArray, assetScope: ByteArray) {
    private val originals: List<ByteArray>
    init {
        val values = listOf(credential, enrollmentCertificates, account, assetScope)
        require(values.zip(listOf(1_024, 10_000, 4_096, 1_024)).all { (bytes, bound) -> bytes.isNotEmpty() && bytes.size <= bound }) { "bounded original owner frames" }
        originals = values.map { it.copyOf() }
    }
    internal fun frames(): List<ByteArray> = originals.map { it.copyOf() }
}

/** A handle returned by the embedding app's trusted native startup loader, never trust pins. */
class KagemushaWalletRuntimeV1(nativeRuntimeHandle: Long) : Closeable {
    private var owner = nativeRuntimeHandle
    private val pending = KagemushaWalletAdmissionLifetimeV1<KagemushaWalletPendingOpenV1>()
    init {
        require(owner > 0) { "native runtime handle" }
        if (!PrivacyNativeBridge.isNativeAvailable()) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
        try { if (KagemushaWalletNativeV1.revision() != 1) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE) }
        catch (_: LinkageError) { throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE) }
    }
    private fun handle(): Long = owner.takeIf { it > 0 } ?: throw KagemushaWalletExceptionV1(-2)
    private fun reply(value: KagemushaWalletCallV1?): KagemushaWalletCallV1 {
        val result = value ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (result.status < 0) throw KagemushaWalletExceptionV1(result.status, result.reason, result.platformCode)
        return result
    }
    private fun enroll(input: KagemushaWalletEnrollmentInputV1): KagemushaWalletEnrollmentReplyV1 {
        val frames = input.frames()
        val result = try {
            KagemushaWalletEnrollmentNativeV1.enroll(handle(), input.selector,
                frames[0], frames[1], frames[2], frames[3], frames[4], frames[5])
        } catch (_: LinkageError) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.BRIDGE_UNAVAILABLE)
        }
        return (result ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)).checked()
    }
    /** Begin once, or resume the SAME durable E1 intent after interrupted result delivery. */
    @Synchronized fun beginEnrollment(originals: KagemushaWalletEnrollmentOriginalsV1): KagemushaWalletEnrollmentProgressV1 =
        enroll(KagemushaWalletEnrollmentInputV1(0, originals)).progress()
    /** Resume exact slot/E1; restored intents never recreate Native's original fresh-key grant. */
    @Synchronized fun resumeEnrollment(originals: KagemushaWalletEnrollmentOriginalsV1, slot: ByteArray): KagemushaWalletEnrollmentProgressV1 =
        enroll(KagemushaWalletEnrollmentInputV1(1, originals, slot)).progress()
    /** Persist before sending; retries return the original retained request byte-for-byte. */
    @Synchronized fun retainEnrollmentRequest(originals: KagemushaWalletEnrollmentOriginalsV1,
        slot: ByteArray, request: ByteArray): ByteArray {
        val input = KagemushaWalletEnrollmentInputV1(2, originals, slot, request)
        return enroll(input).original(3, input.frames()[0])
    }
    /** Verify and durably store the initial credential under its actual marker, E1 and issuer. */
    @Synchronized fun storeEnrollmentCredential(originals: KagemushaWalletEnrollmentOriginalsV1,
        slot: ByteArray, credential: ByteArray, certificates: ByteArray): ByteArray {
        val input = KagemushaWalletEnrollmentInputV1(3, originals, slot, credential, certificates)
        val frames = input.frames()
        val stored = enroll(input).original(4, frames[0])
        if (!stored.contentEquals(frames[4])) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return stored
    }
    /** Reconcile these originals, retaining the same challenge and pending owner on retry. */
    @Synchronized fun begin(originals: KagemushaWalletOpenOriginalsV1): KagemushaWalletPendingOpenV1 {
        val id = handle()
        val frames = originals.frames()
        val result = reply(KagemushaWalletNativeV1.openBegin(id, frames[0], frames[1], frames[2], frames[3]))
        if (result.status != KagemushaWalletCallV1.ACCOUNT_CHALLENGE || result.sequenceLow != id) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        val challenge = result.bytes()
        return pending.select(challenge) { KagemushaWalletPendingOpenV1(this, challenge) }
    }
    private fun finishNative(signature: ByteArray): KagemushaWalletV1 {
        val id = handle()
        val result = reply(KagemushaWalletNativeV1.openFinish(id, signature.copyOf()))
        if (result.status != KagemushaWalletCallV1.OPENED || result.sequenceLow != id) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        val wallet = KagemushaWalletV1(id)
        owner = 0
        return wallet
    }
    @Synchronized internal fun finish(selected: KagemushaWalletPendingOpenV1, signature: ByteArray): KagemushaWalletV1 =
        pending.finish(selected) { finishNative(signature) }
    @Synchronized internal fun cancel(selected: KagemushaWalletPendingOpenV1) {
        pending.abandon(selected) { cancelNative() }
    }
    private fun cancelNative() {
        val status = KagemushaWalletNativeV1.openCancel(handle())
        if (status != 0) throw KagemushaWalletExceptionV1(status)
    }
    /** Discard a pending challenge after interrupted begin delivery, retaining native custody. */
    @Synchronized fun cancelPendingOpen() {
        pending.complete { cancelNative() }
    }
    /** Retry the retained challenge or recover interrupted finish delivery/registration. */
    @Synchronized fun retryOpenCompletion(accountSignature: ByteArray): KagemushaWalletV1 =
        pending.complete { finishNative(accountSignature) }
    /** Successful finish transfers ownership to the wallet; closing this runtime then does nothing. */
    @Synchronized override fun close() {
        val id = owner; owner = 0
        pending.clear()
        if (id != 0L) {
            val status = KagemushaWalletNativeV1.close(id)
            if (status != 0) throw KagemushaWalletExceptionV1(status)
        }
    }
}

/** Exact retained 32-byte account challenge; no decoder or authority constructor exists. */
class KagemushaWalletPendingOpenV1 internal constructor(private val runtime: KagemushaWalletRuntimeV1, challenge: ByteArray) : Closeable {
    private val original = challenge.copyOf()
    /** The existing Ed25519 account signs these exact native bytes. */
    fun challenge(): ByteArray = original.copyOf()
    /** Ordinary refusal retains this pending owner for retry; success transfers ownership. */
    @Synchronized fun finish(accountSignature: ByteArray): KagemushaWalletV1 =
        runtime.finish(this, accountSignature)
    /** Abandon this challenge without admitting ownership or releasing native custody. */
    @Synchronized override fun close() {
        runtime.cancel(this)
    }
}

/** Managed lifetime only, serialized by the runtime lock. Callbacks retain all Native authority. */
internal class KagemushaWalletAdmissionLifetimeV1<P : Any> {
    private var current: P? = null
    private var challenge: ByteArray? = null

    fun select(original: ByteArray, create: () -> P): P {
        current?.let {
            if (!checkNotNull(challenge).contentEquals(original)) {
                throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
            }
            return it
        }
        val selected = create()
        challenge = original.copyOf()
        current = selected
        return selected
    }

    fun <T> finish(selected: P, action: () -> T): T {
        if (current !== selected) throw KagemushaWalletExceptionV1(-2)
        return complete(action)
    }

    fun abandon(selected: P, action: () -> Unit) {
        // Closing a completed or explicitly cancelled wrapper is idempotent. In particular,
        // it must never cancel a subsequent challenge owned by this same runtime.
        if (current === selected) complete(action)
    }

    fun <T> complete(action: () -> T): T {
        val result = action()
        clear()
        return result
    }

    fun clear() {
        current = null
        challenge = null
    }
}
