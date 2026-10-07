package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import org.hyperledger.iroha.sdk.privacy.PrivacyNativeBridge

/** Native-selected key target. This is evidence collection input, never an attestation verdict. */
class KagemushaWalletEnrollmentTargetV1 internal constructor(bytes: ByteArray) {
    private val original = bytes.copyOf()
    init { require(bytes.size == 161) }
    fun slot(): ByteArray = original.copyOfRange(0, 32)
    fun paymentKey(): ByteArray = original.copyOfRange(32, 97)
    fun challengeDigest(): ByteArray = original.copyOfRange(97, 129)
    fun keyBindingDigest(): ByteArray = original.copyOfRange(129, 161)
}
/** Native progress retains unavailable and pending as distinct outcomes. */
sealed class KagemushaWalletEnrollmentProgressV1 {
    class Evidence(val target: KagemushaWalletEnrollmentTargetV1) : KagemushaWalletEnrollmentProgressV1()
    object Pending : KagemushaWalletEnrollmentProgressV1()
    object Abandoned : KagemushaWalletEnrollmentProgressV1()
    object BootstrapSelected : KagemushaWalletEnrollmentProgressV1()
}
/** Either an exact retained E5 or the existing-account message for the selected originals. */
sealed class KagemushaWalletEnrollmentRequestV1(bytes: ByteArray) {
    private val original = bytes.copyOf()
    fun bytes(): ByteArray = original.copyOf()
    class AccountChallenge internal constructor(bytes: ByteArray) : KagemushaWalletEnrollmentRequestV1(bytes)
    class Retained internal constructor(bytes: ByteArray) : KagemushaWalletEnrollmentRequestV1(bytes)
}
internal interface KagemushaWalletEnrollmentDriverV1 {
    fun call(handle: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>): KagemushaWalletCallV1?
    fun close(handle: Long): Int
}
private object NativeEnrollmentDriver : KagemushaWalletEnrollmentDriverV1 {
    override fun call(handle: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>) =
        KagemushaWalletNativeV1.enrollment(handle, selector, first, second, third, certificates)
    override fun close(handle: Long) = KagemushaWalletNativeV1.close(handle)
}
/** Exclusive native enrollment owner, created by trusted native deployment initialization. */
class KagemushaWalletEnrollmentV1 internal constructor(handle: Long, private val driver: KagemushaWalletEnrollmentDriverV1) : Closeable {
    private var owner = handle
    init { require(handle > 0) }
    constructor(nativeEnrollmentHandle: Long) : this(nativeEnrollmentHandle, NativeEnrollmentDriver) {
        if (!PrivacyNativeBridge.isNativeAvailable()) throw KagemushaWalletExceptionV1(-101)
        try { if (KagemushaWalletNativeV1.revision() != 1) throw KagemushaWalletExceptionV1(-101) }
        catch (_: LinkageError) { throw KagemushaWalletExceptionV1(-101) }
    }
    private fun call(selector: Int, first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(), certificates: List<ByteArray> = emptyList()): KagemushaWalletCallV1 {
        if (owner == 0L) throw KagemushaWalletExceptionV1(-2)
        val limits = when (selector) { 0 -> listOf(32,4096,1024); 1,5 -> listOf(64,0,0); 2,7,10 -> listOf(0,0,0); 3 -> listOf(65536,0,0); 4 -> listOf(32,65536,4096); 6 -> listOf(262144,0,0); 9 -> listOf(2048,0,0); else -> throw IllegalArgumentException("selector") }
        require(listOf(first,second,third).zip(limits).all { (bytes,bound) -> bytes.size <= bound })
        require((selector == 3 || certificates.isEmpty()) && certificates.size <= 8 && certificates.all { it.isNotEmpty() && it.size <= 16384 })
        val value = driver.call(owner,selector,first.copyOf(),second.copyOf(),third.copyOf(),certificates.map { it.copyOf() }.toTypedArray()) ?: throw KagemushaWalletExceptionV1(-100)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status,value.reason,value.platformCode)
        if (value.status !in 18..28 || value.sequenceLow != owner) throw KagemushaWalletExceptionV1(-100)
        return value
    }
    private fun exact(value: KagemushaWalletCallV1, status: Int): ByteArray {
        if (value.status != status) throw KagemushaWalletExceptionV1(-100)
        return value.bytes()
    }
    /** Native retains request identity and returns exact issuer dispatch DATA; this grants no key. */
    @Synchronized fun begin(requestId: ByteArray, account: ByteArray, assetScope: ByteArray): ByteArray {
        require(requestId.size == 32)
        return exact(call(0,requestId,account,assetScope),27)
    }
    /** Native authenticates the signed issuer permit before returning the account challenge. */
    @Synchronized fun acceptPermit(originalPermit: ByteArray): ByteArray = exact(call(9,originalPermit),18)
    private fun progress(value: KagemushaWalletCallV1): KagemushaWalletEnrollmentProgressV1 = when (value.status) {
        19 -> KagemushaWalletEnrollmentProgressV1.Evidence(KagemushaWalletEnrollmentTargetV1(value.bytes()))
        20 -> KagemushaWalletEnrollmentProgressV1.Pending
        21 -> KagemushaWalletEnrollmentProgressV1.Abandoned
        22 -> KagemushaWalletEnrollmentProgressV1.BootstrapSelected
        else -> throw KagemushaWalletExceptionV1(-100)
    }
    /** A failed account signature consumes the local challenge; begin again. */
    @Synchronized fun authorize(accountSignature: ByteArray): KagemushaWalletEnrollmentProgressV1 = progress(call(1,accountSignature))
    @Synchronized fun progress(): KagemushaWalletEnrollmentProgressV1 = progress(call(2))
    private fun prepared(value: KagemushaWalletCallV1): KagemushaWalletEnrollmentRequestV1 = when (value.status) {
        23 -> KagemushaWalletEnrollmentRequestV1.AccountChallenge(value.bytes())
        24 -> KagemushaWalletEnrollmentRequestV1.Retained(value.bytes())
        else -> throw KagemushaWalletExceptionV1(-100)
    }
    /** Raw leaf-first DER and opaque Google token only; the issuer fetches its own decoded response. */
    @Synchronized fun prepareRequest(certificates: List<ByteArray>, playIntegrityToken: ByteArray): KagemushaWalletEnrollmentRequestV1 = prepared(call(3,playIntegrityToken,certificates=certificates))
    /** Native retains E5 before returning network bytes; retries return the original exactly. */
    @Synchronized fun retainRequest(accountSignature: ByteArray): ByteArray = exact(call(5,accountSignature),24)
    /** Authenticate signed E6 and durably retain its exact issuer evidence, without ledger activation. */
    @Synchronized fun acceptCredential(issuerResult: ByteArray): ByteArray = exact(call(6,issuerResult),25)
    /** Qualify all installed sources before transferring this handle to original wallet open. */
    @Synchronized fun loadRuntime(): KagemushaWalletRuntimeV1 {
        exact(call(7),26)
        val runtime = KagemushaWalletRuntimeV1(owner)
        owner = 0
        return runtime
    }
    /** Permanently abandon this unused enrollment; return exact native-retained ledger bytes.
     * Native refuses after Bootstrap commits. This does not mean the ledger accepted them. */
    @Synchronized fun abandon(): ByteArray = exact(call(10),28)
    @Synchronized override fun close() {
        val handle = owner; owner = 0
        if (handle != 0L) { val status = driver.close(handle); if (status != 0) throw KagemushaWalletExceptionV1(status) }
    }
}
