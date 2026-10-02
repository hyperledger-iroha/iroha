// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.crypto.keystore.*

/** Separate service purpose. It exposes evidence setup and original recovery, never wallet money. */
interface KagemushaFirstDeviceHardwareEvidenceServiceV1 {
    /** Actual startup factory authenticates compiled release/package/JNI/clock originals first.
     * Existing WAL is recovered. It may not be replaced after uncertainty, expiry or cancellation.
     */
    fun recoverOriginalOrReserve(): KagemushaFirstDeviceHardwareEvidenceSessionV1
}

/** SDK service loader's private owner. There is one original session until Native terminal
 * disposal; disposal does not instantiate a replacement. A cold loader invokes Native recovery.
 */
internal class KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1(
    private val installedNativeOwner: () -> KagemushaHardwareBootstrapNativeEndpointV1,
) : KagemushaFirstDeviceHardwareEvidenceServiceV1 {
    private var retained: KagemushaFirstDeviceHardwareEvidenceSessionV1? = null
    @Synchronized override fun recoverOriginalOrReserve(): KagemushaFirstDeviceHardwareEvidenceSessionV1 =
        retained ?: KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(installedNativeOwner()).also { retained = it }

    companion object {
        /** Authenticate before publication, releasing only the acquired view if startup fails.
         * Native keeps the original operation, WAL and any unknown platform outcome.
         */
        fun fromOwnedNativeView(
            native: KagemushaHardwareBootstrapNativeEndpointV1,
            releaseView: () -> Unit,
        ): KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1 {
            try {
                return KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1 { native }.also {
                    it.recoverOriginalOrReserve()
                }
            } catch (failure: Throwable) {
                try {
                    releaseView()
                } catch (cleanup: Throwable) {
                    if (cleanup !== failure) failure.addSuppressed(cleanup)
                }
                throw failure
            }
        }
    }
}

/** Native-selected public original carrier. The protected Core transport owns authentication.
 * No client security verdict, applet, enrolled device key or wallet session supplies authority.
 */
class KagemushaHardwareBootstrapHttpOriginalV1 internal constructor(
    val stage: Stage, val coreOrigin: String, body: ByteArray, private val requireOwner: () -> Unit,
    operation: ByteArray, private val requirePendingInvocation: () -> Unit = requireOwner,
) {
    enum class Stage { PREPARE, RAW_ATTESTATION, RECEIPT }
    private val invocationClaimed = java.util.concurrent.atomic.AtomicBoolean(false)
    private val original = body.copyOf()
    internal fun claimOriginalInvocation() {
        requireInvocationCurrent()
        check(invocationClaimed.compareAndSet(false,true)) { "Original hardware HTTP invocation already owned" }
    }
    internal fun requireInvocationCurrent() { requirePendingInvocation();requireCurrent() }
    fun body(): ByteArray { requireCurrent(); return original.copyOf() }
    fun requireCurrent() = requireOwner()
    private val originalOperation = operation.copyOf()
    val requestId: String = operation.joinToString("") { "%02x".format(it.toInt() and 255) }
    val path: String = "/v1/kagemusha/hardware-evidence/first-device/" + when(stage) {
        Stage.PREPARE -> "prepare"; Stage.RAW_ATTESTATION -> "raw-attestation"; Stage.RECEIPT -> "finish"
    }
    val maximumOriginalBytes: Int = 192 * 1024
    val maximumResponseBytes: Int = ((maximumOriginalBytes + 2) / 3) * 4 + 128
    fun decodeIssuerResponse(body: ByteArray): ByteArray {
        requireCurrent()
        return KagemushaHardwareBootstrapHttpCodecV1.response(stage, body).also { requireCurrent() }
    }
    init {
        require(operation.size == 32 && operation.any { it != 0.toByte() })
        require(body.isNotEmpty() && body.size <= 1024 * 1024)
        KagemushaHardwareBootstrapHttpCodecV1.requireOrigin(coreOrigin)
    }
}
fun interface KagemushaHardwareBootstrapOriginalTransportV1 {
    /** Product transport uses only the independently selected origin and accepted Core route.
     * Full response is untrusted until the same held Native owner authenticates its original.
     */
    fun exchange(original: KagemushaHardwareBootstrapHttpOriginalV1): CompletableFuture<ByteArray>
}

/** New private typed Native contract; no numbered existing financial frame is relabeled.
 * The JNI adapter/compiled service installer must implement these against the separate Native
 * HardwareEvidenceOwner. There is deliberately no public provider/DTO constructor.
 */
internal interface KagemushaHardwareBootstrapNativeEndpointV1 {
    fun requireOriginalCustody()
    fun requirePendingEffect(step: Int)
    fun originalOperationId(): ByteArray
    fun authoritativeDeadlineMs(): ULong
    fun googleOAuthClientId(): String
    fun completedStep(): Int
    fun pendingStep(): Int?
    fun fencePrepare(originalGoogleIdToken: ByteArray): KagemushaHardwareBootstrapHttpOriginalV1
    fun acceptChallenge(original: ByteArray)
    fun fenceKey(): HardwareKeySelection
    fun recoverKeySelection(): HardwareKeySelection
    fun captureKey(point: ByteArray, rawArchive: ByteArray)
    fun fenceRawIssuer(): KagemushaHardwareBootstrapHttpOriginalV1
    fun acceptRawAdmission(original: ByteArray)
    fun fencePossession(): HardwarePossessionSelection
    fun capturePossession(originalDer: ByteArray)
    fun fenceIntegrity(): HardwareIntegritySelection
    fun captureIntegrityOriginal(opaqueOriginal: ByteArray)
    fun fenceReceipt(): KagemushaHardwareBootstrapHttpOriginalV1
    fun acceptHardwareReceipt(original: ByteArray)
    fun originalReceipt(): ByteArray?
    fun requestCancel()
    fun disposeTerminal()
}
internal class HardwareKeySelection(alias: String, challenge: ByteArray, val policy: KagemushaAndroidAppKeyHardwarePolicyV1) {
    val alias = alias
    private val challengeOriginal = challenge.copyOf()
    fun challenge() = challengeOriginal.copyOf()
    init { require(alias.matches(Regex("kagemusha-hardware-v1-[0-9a-f]{64}")) && challenge.size == 32 && challenge.any { it != 0.toByte() }) }
}
internal class HardwarePossessionSelection(val key: HardwareKeySelection, point: ByteArray, id: ByteArray, message: ByteArray) {
    private val pointOriginal = point.copyOf(); private val idOriginal = id.copyOf(); private val messageOriginal = message.copyOf()
    fun point() = pointOriginal.copyOf(); fun keyId() = idOriginal.copyOf(); fun message() = messageOriginal.copyOf()
    init { requireOriginalAppKeyBindingV1(point, id, point, id)
        requireAppPlatformSigningMessageV1(message, KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION) }
}
internal class HardwareIntegritySelection(val project: Long, hash: ByteArray) {
    private val original = hash.copyOf(); fun hash() = original.copyOf()
    init { require(project > 0 && hash.size == 32 && hash.any { it != 0.toByte() }) }
}

/** Original process operation is retained by the Native-installed service across UI recreation.
 * A view can cancel its own future; it cannot cancel the owned Google/platform/transport future.
 * A missing original after process loss is recovered by Native WAL, never by a new invocation.
 */
class KagemushaFirstDeviceHardwareEvidenceSessionV1 private constructor(
    private val native: KagemushaHardwareBootstrapNativeEndpointV1,
) {
    private val lock = Any()
    private val operation = native.originalOperationId().copyOf()
    private var originalWorkflow: CompletableFuture<ByteArray>? = null
    private var originalGoogleSelection: HardwareIntegritySelection? = null
    private var originalGoogleFuture: CompletableFuture<KagemushaAndroidPlayIntegrityTokenOriginalV1>? = null
    init { require(operation.size == 32 && operation.any { it != 0.toByte() }); requireCurrent() }
    fun operationId(): ByteArray { requireCurrent(); return operation.copyOf() }
    /** Data for display only. No local deadline timer changes the Native attempt. */
    fun authoritativeDeadlineMs(): ULong { requireCurrent(); return native.authoritativeDeadlineMs() }
    /** Data-only recovery selection checked before any new Google OAuth UI request. A pending
     * key fence resumes by loading its exact existing key; all other unknown work refuses.
     * A retained C prefix never invokes Google identity again.
     */
    fun originalNeedsGoogleIdentity(): Boolean {
        requireCurrent()
        val pending = native.pendingStep()
        check(pending == null || pending == 2) { "Original hardware invocation needs Native recovery" }
        val completed = native.completedStep()
        check(pending != 2 || completed == 1) { "Original pending key prefix differs" }
        return (pending == null && completed == 0).also { requireCurrent() }
    }
    fun googleOAuthClientId(): String { requireCurrent(); return native.googleOAuthClientId().also { requireCurrent() } }
    private fun requireCurrent() {
        native.requireOriginalCustody()
        check(MessageDigest.isEqual(operation, native.originalOperationId())) { "First-device Native owner changed" }
    }
    fun recoverOriginalReceipt(): ByteArray? { requireCurrent(); return native.originalReceipt()?.copyOf().also { requireCurrent() } }
    fun requestCancel() { synchronized(lock) { requireCurrent(); native.requestCancel(); requireCurrent() } }
    /** Native denies disposal while any original invocation is still unknown. WAL/key stay intact. */
    fun disposeTerminal() { synchronized(lock) { requireCurrent(); native.disposeTerminal() } }

    fun collectOriginal(googleIdTokenOriginal: ByteArray, keyStore: KagemushaAndroidHardwareAppKeyStoreV1,
        integrity: KagemushaAndroidPlayIntegrityProviderV1,
        transport: KagemushaHardwareBootstrapOriginalTransportV1): CompletableFuture<ByteArray> = synchronized(lock) {
        requireCurrent()
        originalWorkflow?.let { return@synchronized detachedOriginalView(it) { it.copyOf() } }
        recoverOriginalReceipt()?.let { return@synchronized CompletableFuture.completedFuture(it) }
        // Only a pending key fence has a load-only original recovery path. Unknown HTTP,
        // possession and Google effects cannot be invoked again.
        val pending = native.pendingStep()
        check(pending == null || pending == 2) { "Original first-device invocation needs Native recovery" }
        val auth = googleIdTokenOriginal.copyOf()
        val future = CompletableFuture<ByteArray>()
        originalWorkflow = future // Retain before recovery or asynchronous/platform/network work.
        try {
            if (pending == 2) {
                check(native.completedStep() == 1) { "Original pending key prefix differs" }
                recoverOriginalKey(keyStore)
            }
            check(native.pendingStep() == null) { "Original first-device invocation needs Native recovery" }
            val completed = native.completedStep()
            check(completed in 0..5) { "Original first-device recovery stage differs" }
            // Continue from the captured key without fencing or generating a replacement.
            // Other stages resume only their exact next uninvoked effect.
            var flow = CompletableFuture.completedFuture(Unit)
            if (completed == 0) flow = flow.thenCompose {
                val prepare = native.fencePrepare(auth)
                auth.fill(0)
                exchange(prepare, transport).thenApply { original ->
                    requireCurrent(); native.acceptChallenge(original); requireCurrent(); Unit
                }
            } else auth.fill(0)
            if (completed <= 1) flow = flow.thenApply {
                val selected = native.fenceKey()
                native.requirePendingEffect(2)
                val raw = keyStore.issueExact(selected.alias, selected.challenge(), selected.policy) {
                    requireCurrent(); native.requirePendingEffect(2)
                }
                requireCurrent(); native.captureKey(raw.publicKeySec1(), raw.platformAttestationOriginal()); requireCurrent(); Unit
            }
            if (completed <= 2) flow = flow.thenCompose {
                exchange(native.fenceRawIssuer(), transport).thenApply { original ->
                    requireCurrent(); native.acceptRawAdmission(original); requireCurrent(); Unit
                }
            }
            if (completed <= 3) flow = flow.thenApply {
                keyStore.proveFirstDeviceHardwarePossession(this); requireCurrent(); Unit
            }
            if (completed <= 4) flow = flow.thenCompose {
                val selected = native.fenceIntegrity()
                val google = synchronized(lock) {
                    check(originalGoogleFuture == null) { "Original Google invocation already retained" }
                    originalGoogleSelection = selected
                    integrity.requestHardwareBootstrapOriginal(selected.project, selected.hash(),
                        { requireCurrent(); native.requirePendingEffect(5) }, ::requireCurrent).also { originalGoogleFuture = it }
                }
                google.thenApply { original ->
                    requireCurrent(); check(native.pendingStep() == 5)
                    val held = checkNotNull(originalGoogleSelection)
                    check(original.cloudProjectNumber == held.project && MessageDigest.isEqual(original.requestHash(), held.hash())) {
                        "Original Google evidence differs from the same Native C/key/PI selection"
                    }
                    native.captureIntegrityOriginal(original.opaqueToken().toByteArray(Charsets.US_ASCII)); requireCurrent(); Unit
                }
            }
            flow.thenCompose { exchange(native.fenceReceipt(), transport) }.whenComplete { original, error ->
                try {
                    requireCurrent()
                    if (error != null) throw error
                    native.acceptHardwareReceipt(checkNotNull(original)); requireCurrent()
                    future.complete(checkNotNull(native.originalReceipt()).copyOf())
                } catch (failure: Throwable) { future.completeExceptionally(failure) }
            }
        } catch (failure: Throwable) { auth.fill(0); future.completeExceptionally(failure) }
        detachedOriginalView(future) { it.copyOf() }
    }
    /** Finish a known captured C/raw/E/PI prefix without invoking Keystore or Google again.
     * A pending receipt call is unknown and cannot be resubmitted by this path.
     */
    fun completeCapturedOriginalReceipt(transport: KagemushaHardwareBootstrapOriginalTransportV1): CompletableFuture<ByteArray> = synchronized(lock) {
        requireCurrent()
        originalWorkflow?.let { return@synchronized detachedOriginalView(it) { it.copyOf() } }
        recoverOriginalReceipt()?.let { return@synchronized CompletableFuture.completedFuture(it) }
        check(native.pendingStep() == null && native.completedStep() == 5) { "Original captured hardware prefix differs" }
        val future = CompletableFuture<ByteArray>(); originalWorkflow = future
        try {
            exchange(native.fenceReceipt(), transport).whenComplete { original, error ->
                try {
                    requireCurrent(); if (error != null) throw error
                    native.acceptHardwareReceipt(checkNotNull(original)); requireCurrent()
                    future.complete(checkNotNull(native.originalReceipt()).copyOf())
                } catch (failure: Throwable) { future.completeExceptionally(failure) }
            }
        } catch (failure: Throwable) { future.completeExceptionally(failure) }
        detachedOriginalView(future) { it.copyOf() }
    }
    private fun exchange(original: KagemushaHardwareBootstrapHttpOriginalV1,
        transport: KagemushaHardwareBootstrapOriginalTransportV1): CompletableFuture<ByteArray> {
        requireCurrent(); original.requireCurrent()
        // This owned original future is never returned to the UI or cancelled on UI disposal.
        return transport.exchange(original).thenApply { body ->
            requireCurrent(); original.requireCurrent()
            check(body.isNotEmpty() && body.size <= original.maximumOriginalBytes)
            body.copyOf()
        }
    }
    internal fun performHardwarePossession(callback: (HardwarePossessionSelection, () -> Unit) -> ByteArray) {
        requireCurrent(); val selected = native.fencePossession()
        val der = callback(selected) { requireCurrent(); native.requirePendingEffect(4) }
        try { requireOriginalP256DerV1(der); requireCurrent(); native.capturePossession(der); requireCurrent() }
        finally { der.fill(0) }
    }
    /** Recovery only reads the same key after its already durable key invocation fence. */
    fun recoverOriginalKey(keyStore: KagemushaAndroidHardwareAppKeyStoreV1) = synchronized(lock) {
        requireCurrent(); check(native.pendingStep() == 2)
        val selected = native.recoverKeySelection()
        val original = checkNotNull(keyStore.recoverExact(selected.alias, selected.challenge(), selected.policy, ::requireCurrent)) {
            "Original hardware key absent; replacement is forbidden"
        }
        native.captureKey(original.publicKeySec1(), original.platformAttestationOriginal()); requireCurrent()
    }
    internal companion object {
        /** Called only by the real compiled bootstrap service installer after Native admission. */
        fun fromNativeEndpoint(native: KagemushaHardwareBootstrapNativeEndpointV1) = KagemushaFirstDeviceHardwareEvidenceSessionV1(native)
    }
}

/** Cancellation affects the view alone, preserving the operation original and its eventual capture. */
internal fun <T> detachedOriginalView(original: CompletableFuture<T>, copy: (T) -> T): CompletableFuture<T> {
    val view = CompletableFuture<T>()
    original.whenComplete { result, error ->
        try { if (error != null) view.completeExceptionally(error) else view.complete(copy(result)) }
        catch (failure: Throwable) { view.completeExceptionally(failure) }
    }
    return view
}
