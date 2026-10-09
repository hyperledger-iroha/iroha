package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable

/** Native-selected key target. This is evidence collection input, never an attestation verdict. */
class KagemushaWalletEnrollmentTargetV1 internal constructor(bytes: ByteArray) {
    init { require(bytes.size == 161) }
    private val original = bytes.copyOf()
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
internal object NativeEnrollmentDriver : KagemushaWalletEnrollmentDriverV1 {
    override fun call(handle: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>) =
        KagemushaWalletNativeV1.enrollment(handle, selector, first, second, third, certificates)
    override fun close(handle: Long) = KagemushaWalletNativeV1.close(handle)
}
/** Bounded sensitive originals; only the installed Native owner authenticates them.
 * Pass an empty dpopProof for BPNG's retail JWT; CBSI requires its original DPoP.
 * The signed Native product selection enforces that distinction.
 */
class KagemushaWalletEnrollmentSessionOriginalsV1(accessToken: ByteArray, dpopProof: ByteArray, attestationRootDER: ByteArray) {
    private val originals = listOf(accessToken, dpopProof, attestationRootDER).map { it.copyOf() }
    init {
        require(originals.zip(listOf(16_384,4096,16_384)).all { (bytes,bound) -> bytes.size <= bound })
        require(originals[0].isNotEmpty() && originals[2].isNotEmpty())
    }
    internal fun frames(): List<ByteArray> = originals.map { it.copyOf() }
    override fun toString() = "KagemushaWalletEnrollmentSessionOriginalsV1(originals=[REDACTED])"
}
/** Exclusive native enrollment owner, created by trusted native deployment initialization. */
class KagemushaWalletEnrollmentV1 internal constructor(handle: Long, private val driver: KagemushaWalletEnrollmentDriverV1, assetScope: ByteArray, private val platformOwner: Any, val assetScale: Int) : Closeable, KagemushaWalletCleanupResourceV1 {
    companion object {
        const val REQUEST_MAX_BYTES = 524_288
        const val RESULT_MAX_BYTES = 262_144
        const val DISPATCH_MAX_BYTES = 16_384
        const val PERMIT_MAX_BYTES = 2_048
    }
    private var owner = handle
    private var retired = false
    private var closeFailure: Throwable? = null
    init { require(handle > 0) }
    private val selectedAsset = assetScope.copyOf()
    /** Exact asset frame returned by the authenticated Native installed selection. */
    fun assetScopeOriginal(): ByteArray = selectedAsset.copyOf()
    private fun call(selector: Int, first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(), certificates: List<ByteArray> = emptyList()): KagemushaWalletCallV1 {
        if (retired) throw closeFailure ?: KagemushaWalletExceptionV1(-2)
        if (owner == 0L) throw KagemushaWalletExceptionV1(-2)
        val limits = when (selector) { 0 -> listOf(32,4096,1024); 1,5 -> listOf(64,0,0); 2,7,10,16,18 -> listOf(0,0,0); 3 -> listOf(65536,0,0); 4 -> listOf(32,65536,4096); 6 -> listOf(RESULT_MAX_BYTES,0,0); 9 -> listOf(PERMIT_MAX_BYTES,0,0); 19 -> listOf(16384,4096,16384); else -> throw IllegalArgumentException("selector") }
        require(listOf(first,second,third).zip(limits).all { (bytes,bound) -> bytes.size <= bound })
        require((selector == 3 || certificates.isEmpty()) && certificates.size <= 8 && certificates.all { it.isNotEmpty() && it.size <= 16384 })
        val value = driver.call(owner,selector,first.copyOf(),second.copyOf(),third.copyOf(),certificates.map { it.copyOf() }.toTypedArray()) ?: throw KagemushaWalletExceptionV1(-100)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status,value.reason,value.platformCode)
        val expected = when (selector) { 0 -> setOf(27); 1,2 -> setOf(19,20,21,22); 3,4 -> setOf(23,24); 5 -> setOf(24); 6 -> setOf(25); 7 -> setOf(26); 9 -> setOf(18); 10 -> setOf(28); 16 -> setOf(20,24); 18 -> setOf(20,25); 19 -> setOf(39); else -> emptySet() }
        val bound = when (value.status) { 18,23 -> 32; 19 -> 161; 24 -> REQUEST_MAX_BYTES; 25 -> RESULT_MAX_BYTES; 27 -> DISPATCH_MAX_BYTES; 28 -> 1024; else -> 0 }
        val size = value.bytes().size
        if (value.status !in expected || value.sequenceLow != owner || value.sequenceHigh != 0L || value.detail != 0 ||
            (if (bound == 0) size != 0 else size !in 1..bound) ||
            (value.status in setOf(18,23) && size != 32) || (value.status == 19 && size != 161)) throw KagemushaWalletExceptionV1(-100)
        return value
    }
    private fun exact(value: KagemushaWalletCallV1, status: Int): ByteArray {
        if (value.status != status) throw KagemushaWalletExceptionV1(-100)
        return value.bytes()
    }
    /** Native retains request identity and returns exact issuer dispatch DATA; this grants no key. */
    @Synchronized fun begin(requestId: ByteArray, account: ByteArray): ByteArray {
        require(requestId.size == 32)
        return exact(call(0,requestId,account,selectedAsset),27)
    }
    /** Authenticate a renewed session on the same provider/platform owner, preserving
     * the original attempt dates and returned evidence. Begin the same request ID again
     * before obtaining another signed permit or collecting any new effect. */
    @Synchronized fun renewSession(session: KagemushaWalletEnrollmentSessionOriginalsV1) {
        val values = session.frames()
        exact(call(19, values[0], values[1], values[2]), 39)
    }
    /** Native authenticates the signed issuer permit before returning the account challenge. */
    @Synchronized fun acceptPermit(originalPermit: ByteArray): ByteArray {
        require(originalPermit.size in 1..PERMIT_MAX_BYTES)
        return exact(call(9,originalPermit),18)
    }
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
    /** Exact authenticated E5 already retained by Native; no collection effect is repeated. */
    @Synchronized fun retainedRequest(): ByteArray? = call(16).let { if (it.status == 24) it.bytes() else null }
    /** Raw leaf-first DER and opaque Google token only; the issuer fetches its own decoded response. */
    @Synchronized fun prepareRequest(certificates: List<ByteArray>, playIntegrityToken: ByteArray): KagemushaWalletEnrollmentRequestV1 = prepared(call(3,playIntegrityToken,certificates=certificates))
    /** Native retains E5 before returning network bytes; retries return the original exactly. */
    @Synchronized fun retainRequest(accountSignature: ByteArray): ByteArray = exact(call(5,accountSignature),24)
    /** Authenticate signed E6 and durably retain its exact issuer evidence, without ledger activation. */
    @Synchronized fun acceptCredential(issuerResult: ByteArray): ByteArray {
        require(issuerResult.size in 1..RESULT_MAX_BYTES)
        return exact(call(6,issuerResult),25)
    }
    /** Exact authenticated durable E6, including issuer evidence, for interrupted delivery.
     * Only Native's explicit absence outcome permits null; provider errors are preserved. */
    @Synchronized fun retainedResult(): ByteArray? = call(18).let { if (it.status == 25) it.bytes() else null }
    /** Qualify all installed sources before transferring this handle to original wallet open. */
    @Synchronized fun loadRuntime(): KagemushaWalletRuntimeV1 {
        exact(call(7),26)
        val runtime = KagemushaWalletRuntimeV1(owner, platformOwner)
        owner = 0
        return runtime
    }
    /** Permanently abandon this unused enrollment; return exact native-retained ledger bytes.
     * Native refuses after Bootstrap commits. This does not mean the ledger accepted them. */
    @Synchronized fun abandon(): ByteArray = exact(call(10),28)
    @Synchronized override fun close() { closeFailure?.let { throw it }; closeAttempt() }
    @Synchronized override fun retryCleanup() { closeAttempt() }
    @Synchronized override fun cleanupReleased(): Boolean = owner == 0L
    private fun closeAttempt() {
        retired = true
        val handle = owner
        if (handle != 0L) {
            try {
                val status = driver.close(handle)
                if (status != 0) throw KagemushaWalletExceptionV1(status)
                owner = 0
                closeFailure = null
            } catch (failure: Throwable) {
                closeFailure = failure
                KagemushaWalletInstalledRuntimeV1.retainFailure(this, failure)
                throw failure
            }
        }
    }
}
