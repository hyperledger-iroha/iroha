// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean

/** Product HTTP transport selected under its actual enrolled request-device/Native-session
 * authentication. It must enforce maximumResponseBytes while reading; replies remain untrusted.
 */
fun interface KagemushaOrdinaryCurrentControlOriginalTransportV1 {
    suspend fun exchange(original: KagemushaOrdinaryCurrentControlHttpOriginalV1): ByteArray
}

/** Private-origin carrier from one held Native request/signature. No public constructor
 * accepts account, status, nonce, FI policy, timestamp or original signing subject.
 */
class KagemushaOrdinaryCurrentControlHttpOriginalV1 private constructor(
    request: ByteArray, signature: ByteArray, private val guard: () -> Unit,
) {
    val path: String = "/v1/kagemusha/enrollment/ordinary/current-control"
    val maximumResponseBytes: Int = KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_RESPONSE_BYTES
    val requestId: String = KagemushaOrdinaryCurrentControlHttpCodecV1.requestId(request)
    private val originalBody = KagemushaOrdinaryCurrentControlHttpCodecV1.requestBody(request, signature)
    fun requireCurrent() = guard()
    fun body(): ByteArray { requireCurrent(); return originalBody.copyOf().also { requireCurrent() } }
    internal companion object {
        fun selected(request: ByteArray, signature: ByteArray, guard: () -> Unit) =
            KagemushaOrdinaryCurrentControlHttpOriginalV1(request, signature, guard)
    }
}
internal interface KagemushaOrdinaryRuntimeCurrentControlEndpointV1 :
    KagemushaOrdinaryNativeCurrentControlEndpointV1, KagemushaOrdinaryNativeStartupEndpointV1

/** Detached historical correlation only. It cannot recreate a live Native FI loan, cash
 * owner, clock or money capability. Duplicate return does not refresh financial authority.
 */
class KagemushaOrdinaryCurrentControlOriginalsV1 internal constructor(
    val requestId: String, requestDigest: ByteArray, signedDigest: ByteArray, authorityDigest: ByteArray,
) {
    private val request = requestDigest.copyOf()
    private val signed = signedDigest.copyOf()
    private val authority = authorityDigest.copyOf()
    fun requestOriginalDigest(): ByteArray = request.copyOf()
    fun signedControlOriginalDigest(): ByteArray = signed.copyOf()
    fun authorityOriginalDigest(): ByteArray = authority.copyOf()
}

/** Current FI workflow under the genuine installed account/cash holder. It authenticates
 * fresh S/W through Native startup, then reserves/signs/admit only the actual FI request.
 * The product must retire its Bootstrap workflow before the first consuming cash dispatch.
 * No State, approval, balance, spending-enabled flag or money capability is returned.
 */
class KagemushaOrdinaryCurrentControlV1 private constructor(
    private val invokeControl: (Int, ByteArray, ByteArray) -> List<ByteArray>,
    private val requireOpen: () -> Unit,
    private val revoke: () -> Unit,
    private val invokeStartup: (Int, Long) -> List<ByteArray>,
    private val transport: KagemushaOrdinaryCurrentControlOriginalTransportV1,
    private val requireOriginalOwner: () -> Unit,
) {
    /** Scripted workflow seam only; an internal caller cannot expose a Core descriptor. */
    internal constructor(invokeControl: (Int, ByteArray, ByteArray) -> List<ByteArray>,
        requireOpen: () -> Unit, revoke: () -> Unit,
        endpoint: KagemushaOrdinaryRuntimeCurrentControlEndpointV1,
        transport: KagemushaOrdinaryCurrentControlOriginalTransportV1,
        requireOriginalOwner: () -> Unit) : this(invokeControl, requireOpen, revoke,
            { phase, id -> endpoint.startup(phase, id, ByteArray(0))?.map { it.copyOf() }
                ?: error("Actual Native ordinary session refresh is unavailable") },
            transport, requireOriginalOwner)

    /** Same actual coordinator; neither a raw descriptor nor a managed signer is accepted. */
    constructor(coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
        transport: KagemushaOrdinaryCurrentControlOriginalTransportV1,
        requireOriginalOwner: () -> Unit) : this(coordinator.ordinaryCurrentControlTransportBinding(), transport, requireOriginalOwner)

    private constructor(binding: KagemushaOrdinaryCurrentControlTransportBindingV1,
        transport: KagemushaOrdinaryCurrentControlOriginalTransportV1, guard: () -> Unit)
        : this({ phase, signed, authority ->
            binding.invoke(KagemushaOrdinaryRuntimeJniV1, phase, signed, authority) },
            binding::requireOpen, binding::revoke,
            { phase, id -> binding.invokeStartup(KagemushaOrdinaryRuntimeJniV1, phase, id) }, transport, guard)

    private class OriginalCycle {
        var started = false
        var request: ByteArray? = null
        var carrier: KagemushaOrdinaryCurrentControlHttpOriginalV1? = null
        var response: ByteArray? = null
        @Volatile var intakeStarted = false
        var completed: KagemushaOrdinaryCurrentControlOriginalsV1? = null
    }
    private val refreshing = AtomicBoolean(false)
    @Volatile private var frozen = false
    private var cycle = OriginalCycle()

    /** Complete or resume the same original. HTTP-only uncertainty preserves request/signature
     * and UUID, with no fresh nonce, wallet read, account signing or time reset. Native alone
     * enforces both its real wallet-membership and FI elapsed-time budgets at actual intake.
     */
    suspend fun beginOrResumeCurrentFinancialControl(): KagemushaOrdinaryCurrentControlOriginalsV1 = perform(false)

    /** Explicitly obtain a new genuine current read after a completed cycle. An unfinished
     * HTTP original must be resumed unchanged; an unknown Native outcome never restarts here.
     * This method grants no managed readiness and retains no alternate clock or authority.
     */
    suspend fun refreshCurrentFinancialControl() { perform(true) }

    private suspend fun perform(freshCompletedCycle: Boolean): KagemushaOrdinaryCurrentControlOriginalsV1 {
        check(refreshing.compareAndSet(false, true)) { "A Native current FI refresh is already active" }
        try {
            try { current() } catch (failure: Throwable) { freeze(failure) }
            if (freshCompletedCycle) {
                check(!cycle.started || cycle.completed != null) { "The unfinished original must be resumed without another Native invocation" }
                if (cycle.completed != null) cycle = OriginalCycle()
            }
            val original = cycle
            original.completed?.let { return it }
            if (!original.started) {
                original.started = true
                try {
                    // Preserve the maintained authentic current-wallet path. Phase6 obtains all
                    // four signed wallet/World originals through the real Native AccountClient.
                    val reservation = startup(1, 0)
                    require(reservation.size == 6 && reservation[3].size == 32 && reservation[3].any { it != 0.toByte() })
                    require(reservation[4].size in 1..4096 && reservation[5].size in 1..4096)
                    val readId = le64(reservation[2]); require(readId != 0L)
                    val refreshed = startup(6, readId)
                    require(refreshed.size == 3 && le64(refreshed[2]) != 0L)
                    current()
                    val reserved = invokeControl(1, ByteArray(0), ByteArray(0))
                    require(reserved.size == 2 && reserved[0].size in 1..KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_REQUEST_BYTES)
                    require(reserved[1].contentEquals(
                        "iroha:kagemusha:v1:ordinary-current-fi-control-request\u0000".toByteArray(Charsets.US_ASCII) + reserved[0]))
                    original.request = reserved[0].copyOf()
                    current()
                    val signed = invokeControl(2, ByteArray(0), ByteArray(0))
                    require(signed.size == 2 && MessageDigest.isEqual(checkNotNull(original.request), signed[0]) && signed[1].size == 64)
                    current()
                    original.carrier = KagemushaOrdinaryCurrentControlHttpOriginalV1.selected(signed[0], signed[1]) {
                        current()
                        check(cycle === original && !original.intakeStarted) { "The retained Native FI HTTP original has entered intake or changed" }
                    }
                } catch (failure: Throwable) { freeze(failure) }
            }
            if (original.response == null) {
                val response = try { transport.exchange(checkNotNull(original.carrier)) }
                    catch (failure: Throwable) {
                        try { current() } catch (revoked: Throwable) { freeze(revoked) }
                        throw failure // Only HTTP uncertainty keeps the sole unchanged original.
                    }
                try {
                    current()
                    require(response.size in 1..KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_RESPONSE_BYTES)
                    original.response = response.copyOf()
                } catch (failure: Throwable) { freeze(failure) }
            }
            try {
                current()
                check(!original.intakeStarted) { "The original Native FI intake has an unknown outcome" }
                val originals = KagemushaOrdinaryCurrentControlHttpCodecV1.responseOriginals(checkNotNull(original.response))
                val acknowledgement = KagemushaOrdinaryCurrentControlOriginalsV1(checkNotNull(original.carrier).requestId,
                    sha(checkNotNull(original.request)), sha(originals[0]), sha(originals[1]))
                original.intakeStarted = true // Native may fsync before a lost return: never resend.
                check(invokeControl(3, originals[0], originals[1]).isEmpty())
                current()
                original.completed = acknowledgement
                original.response = null // Native owns authority originals; managed retains correlation only.
                return acknowledgement
            } catch (failure: Throwable) { freeze(failure) }
        } finally { refreshing.set(false) }
    }

    private fun current() { check(!frozen); requireOriginalOwner(); requireOpen(); requireOriginalOwner() }
    private fun freeze(failure: Throwable): Nothing {
        frozen = true; cycle.response = null
        try { revoke() } catch (_: Throwable) { }
        throw failure
    }
    private fun startup(phase: Int, id: Long): List<ByteArray> {
        current()
        val fields = invokeStartup(phase, id)
        current()
        require(fields.size >= 3 && fields[0].contentEquals(byteArrayOf(1,0)) &&
            fields[1].contentEquals(byteArrayOf(phase.toByte())) && fields[2].size == 8)
        return fields
    }
    private fun le64(raw: ByteArray): Long {
        require(raw.size == 8)
        var value = 0L
        for (index in raw.indices) value = value or ((raw[index].toLong() and 255L) shl (index * 8))
        return value
    }
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
}

/** Revoke the current genuine Native selection after Core teardown. This never loads,
 * opens or reconstructs an account/runtime; missing Native startup remains a failure.
 */
object KagemushaOrdinaryRuntimeLifecycleV1 {
    /** Deny only the retained pending/published original, without loading JNI or reading sources.
     * Native authenticates its held JNI/VM/loader owner; absence of account composition burns no
     * future first wallet. This notification does not close Core or revoke a registry selection.
     */
    @JvmStatic
    fun retireOriginal() {
        val retired = try {
            KagemushaOrdinaryRuntimeJniV1.retireOriginal()
        } catch (error: LinkageError) {
            throw IllegalStateException("Actual Native original retirement is unavailable", error)
        }
        check(retired) { "Actual Native original retirement was rejected" }
    }

    @JvmStatic
    fun revokeSelection(): Unit = revokeOrdinaryRuntimeSelectionV1(KagemushaOrdinaryRuntimeJniV1)
}

/** Internal scripted transport seam; the public lifecycle has no endpoint or authority input. */
internal fun revokeOrdinaryRuntimeSelectionV1(endpoint: KagemushaOrdinaryNativeStartupEndpointV1) {
    val response = try {
        endpoint.startup(5, 0L, ByteArray(0))
            ?: error("Actual Native ordinary selection revocation is unavailable")
    } catch (error: LinkageError) {
        throw IllegalStateException("Actual Native ordinary selection revocation is unavailable", error)
    }
    require(response.size == 3 && response[0].contentEquals(byteArrayOf(1, 0)) &&
        response[1].contentEquals(byteArrayOf(5)) && response[2].contentEquals(ByteArray(8))) {
        "Actual Native ordinary selection revocation reply is malformed"
    }
}

/** Sole existing exact shared Native entry names. No managed clock/signing/FI verifier. */
internal object KagemushaOrdinaryRuntimeJniV1 : KagemushaOrdinaryRuntimeCurrentControlEndpointV1,
    KagemushaOrdinaryNativeOutgoingEndpointV1, KagemushaOrdinaryNativeIncomingEndpointV1,
    KagemushaOrdinaryNativeIntegrityRefreshEndpointV1, KagemushaOrdinaryNativeMintFundingEndpointV1 {
    fun bindApplication(application: android.app.Application): Boolean = nativeBindApplicationV1(application)
    fun retireOriginal(): Boolean = nativeRetireOriginalV1()
    override fun mintFunding(phase:Int,coreHandle:Long,originals:Array<ByteArray>):Array<ByteArray>? =
        nativeMintFundingV1(phase,coreHandle,originals)
    @JvmStatic private external fun nativeMintFundingV1(phase:Int,handle:Long,originals:Array<ByteArray>):Array<ByteArray>?
    override fun integrityRefresh(phase:Int,coreHandle:Long,original:ByteArray):Array<ByteArray>? =
        nativeIntegrityRefreshV1(phase,coreHandle,original)
    @JvmStatic private external fun nativeIntegrityRefreshV1(phase:Int,handle:Long,original:ByteArray):Array<ByteArray>?
    override fun incoming(phase: Int, coreHandle: Long, originals: Array<ByteArray>): Array<ByteArray>? =
        nativeIncomingV1(phase, coreHandle, originals)
    @JvmStatic private external fun nativeIncomingV1(phase: Int, handle: Long, originals: Array<ByteArray>): Array<ByteArray>?

    override fun startup(phase: Int, readId: Long, original: ByteArray): Array<ByteArray>? =
        nativeStartupV1(phase, readId, original)
    override fun invoke(phase: Int, coreHandle: Long, signedOriginal: ByteArray, authorityOriginal: ByteArray): Array<ByteArray>? =
        nativeCurrentControlV1(phase, coreHandle, signedOriginal, authorityOriginal)
    override fun outgoing(phase: Int, coreHandle: Long, originals: Array<ByteArray>): Array<ByteArray>? =
        nativeOutgoingV1(phase, coreHandle, originals)
    @JvmStatic private external fun nativeOutgoingV1(phase: Int, handle: Long, originals: Array<ByteArray>): Array<ByteArray>?
    internal fun existingAndroidAccount(application: android.app.Application): Array<ByteArray>? =
        nativeExistingAndroidAccountV1(application)
    @JvmStatic private external fun nativeExistingAndroidAccountV1(application: android.app.Application): Array<ByteArray>?
    internal fun consumeExistingAndroidAccount(original: KagemushaOrdinaryExistingAccountIntakeV1,
        signatory: String, seed: ByteArray): Boolean = nativeConsumeExistingAndroidAccountV1(original, signatory, seed)
    @JvmStatic private external fun nativeConsumeExistingAndroidAccountV1(original: KagemushaOrdinaryExistingAccountIntakeV1,
        signatory: String, seed: ByteArray): Boolean
    @JvmStatic private external fun nativeStartupV1(phase: Int, id: Long, original: ByteArray): Array<ByteArray>?
    @JvmStatic private external fun nativeCurrentControlV1(phase: Int, handle: Long, signed: ByteArray, authority: ByteArray): Array<ByteArray>?
    @JvmStatic private external fun nativeBindApplicationV1(application: android.app.Application): Boolean
    @JvmStatic private external fun nativeRetireOriginalV1(): Boolean
}
