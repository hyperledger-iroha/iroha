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
    /** Reconcile and sample a fresh challenge bound to these original issuer/account frames. */
    @Synchronized fun begin(originals: KagemushaWalletOpenOriginalsV1): KagemushaWalletPendingOpenV1 {
        val id = handle()
        val frames = originals.frames()
        val result = reply(KagemushaWalletNativeV1.openBegin(id, frames[0], frames[1], frames[2], frames[3]))
        if (result.status != KagemushaWalletCallV1.ACCOUNT_CHALLENGE || result.sequenceLow != id) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return KagemushaWalletPendingOpenV1(this, result.bytes())
    }
    @Synchronized internal fun finish(signature: ByteArray): KagemushaWalletV1 {
        val id = handle()
        val result = reply(KagemushaWalletNativeV1.openFinish(id, signature.copyOf()))
        if (result.status != KagemushaWalletCallV1.OPENED || result.sequenceLow != id) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        val wallet = KagemushaWalletV1(id)
        owner = 0
        return wallet
    }
    /** Discard a pending challenge after interrupted begin delivery, retaining native custody. */
    @Synchronized fun cancelPendingOpen() {
        val status = KagemushaWalletNativeV1.openCancel(handle())
        if (status != 0) throw KagemushaWalletExceptionV1(status)
    }
    /** Recover a finish after interrupted output/registration; rejected authorization needs a fresh begin. */
    fun retryOpenCompletion(accountSignature: ByteArray): KagemushaWalletV1 = finish(accountSignature)
    /** Successful finish transfers ownership to the wallet; closing this runtime then does nothing. */
    @Synchronized override fun close() {
        val id = owner; owner = 0
        if (id != 0L) {
            val status = KagemushaWalletNativeV1.close(id)
            if (status != 0) throw KagemushaWalletExceptionV1(status)
        }
    }
}

/** One-use exact 32-byte account challenge; no decoder or authority constructor exists. */
class KagemushaWalletPendingOpenV1 internal constructor(runtime: KagemushaWalletRuntimeV1, challenge: ByteArray) : Closeable {
    private var runtime: KagemushaWalletRuntimeV1? = runtime
    private val original = challenge.copyOf()
    /** The existing Ed25519 account signs these exact native bytes. */
    fun challenge(): ByteArray = original.copyOf()
    /** Failure consumes the challenge; retained runtime custody can begin afresh. */
    @Synchronized fun finish(accountSignature: ByteArray): KagemushaWalletV1 {
        val selected = runtime ?: throw KagemushaWalletExceptionV1(-2)
        runtime = null
        return selected.finish(accountSignature)
    }
    /** Abandon this challenge without admitting ownership or releasing native custody. */
    @Synchronized override fun close() {
        val selected = runtime; runtime = null
        selected?.cancelPendingOpen()
    }
}
