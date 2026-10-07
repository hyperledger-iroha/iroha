// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable
import java.util.concurrent.atomic.AtomicLong

/** Native failure; -4 means the authenticated operation/Λ/Ω artifact loader is unavailable. */
class KagemushaWalletExceptionV1(
    @JvmField val status: Int,
    @JvmField val reason: Int = -1,
    @JvmField val platformCode: Int = 0,
) : IllegalStateException("KAGEMUSHA native status $status (reason $reason, platform $platformCode)") {
    companion object {
        /** There is deliberately no structural verifier or software-key fallback. */
        const val ARTIFACTS_UNAVAILABLE = -4
        /** Native output violates the fixed status/payload contract. */
        const val INVALID_NATIVE_OUTPUT = -100
        /** Missing, stale or unauthenticated native bridge library. */
        const val BRIDGE_UNAVAILABLE = -101
    }
}

/** Native outcome with operation-specific projections; setup originals never establish monetary completion. */
class KagemushaWalletCallV1 internal constructor(
    @JvmField val status: Int,
    @JvmField val reason: Int,
    @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long,
    @JvmField val sequenceHigh: Long,
    @JvmField val detail: Int,
    bytes: ByteArray,
) {
    init {
        val carriesBytes = status == COMPLETE || status == CREDIT_STATUS || status == SETUP || status == TIME_CHALLENGE || status == ACCOUNT_CHALLENGE || status == ACTIVATION || status == CLOSE_LOADS || status == 31 || status == 33 || status == 36 || status == LEDGER_INSTRUCTION || status == LOAD_FINALITY || status == UNLOAD_CONFIRMATION || status == LOAD_PROOF_PROGRESS || status in listOf(18, 19, 23, 24, 25, 27, 28, 37, 38)
        if ((status >= 0 && status !in UNKNOWN..LOAD_PROOF_PROGRESS) || bytes.size > when (status) { ACTIVATION, ENROLLMENT_DISPATCH, CLOSE_LOADS, 36, LOAD_FINALITY -> 16_384; LEDGER_INSTRUCTION -> 65_536; 24 -> KagemushaWalletEnrollmentV1.REQUEST_MAX_BYTES; 25 -> 262_144; 28 -> 1024; 37 -> 1028; 38 -> 73_740; 31 -> 21_024; 33, UNLOAD_CONFIRMATION -> 32; LOAD_PROOF_PROGRESS -> 8; else -> 10_000 } ||
            (if (carriesBytes) bytes.isEmpty() else bytes.isNotEmpty()) ||
            ((status == TIME_CHALLENGE || status == ACCOUNT_CHALLENGE) && (bytes.size != 32 || sequenceLow <= 0 || sequenceHigh != 0L)) ||
            (status in listOf(ACTIVATION, CLOSE_LOADS, 31, 32, 34, 35, 36, LEDGER_INSTRUCTION, LOAD_FINALITY) && (sequenceLow != 0L || sequenceHigh != 0L || detail != 0)) ||
            (status == OPENED && (sequenceLow <= 0 || sequenceHigh != 0L)) ||
            ((status in 18..28 || status in 37..39) && (sequenceLow <= 0 || sequenceHigh != 0L || detail != 0)) ||
            (status == LOAD_PROOF_PROGRESS && (bytes.size != 8 || (sequenceLow in 0L..1L) || sequenceHigh != 0L || detail != 0)) ||
            (status in listOf(18, 23) && bytes.size != 32) || (status == 19 && bytes.size != 161) ||
            (status in listOf(33, UNLOAD_CONFIRMATION) && (sequenceLow == 0L || sequenceHigh != 0L || detail != 0 || bytes.size != 32 || bytes.all { it == 0.toByte() }))) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        }
    }
    private val retainedBytes = bytes.copyOf()
    /** Exact canonical bytes returned by Rust; this accessor never assembles or signs again. */
    fun bytes(): ByteArray = retainedBytes.copyOf()
    private fun invalid(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    /** Only ordinary operation outcomes can be returned from Bootstrap or Archive. */
    internal fun completion(): KagemushaWalletCallV1 {
        if (status !in UNKNOWN..PREPARING) invalid()
        return this
    }
    /** Setup originals are transport bytes, never a monetary completion. */
    internal fun original(): ByteArray {
        if (status != SETUP || sequenceLow != 0L || sequenceHigh != 0L || detail != 0 || retainedBytes.isEmpty()) invalid()
        return retainedBytes.copyOf()
    }
    internal fun feeClaimInput(beneficiary: ByteArray): KagemushaWalletSetupInputV1 {
        if (status != 31) invalid()
        return KagemushaWalletSetupInputV1(26, first = retainedBytes, second = beneficiary)
    }
    internal fun feeClaimOriginal(): ByteArray {
        if (status != 36) invalid()
        return retainedBytes.copyOf()
    }
    internal fun creditedInput(): KagemushaWalletSetupInputV1 {
        require(status == COMPLETE || status == CREDIT_STATUS) { "Receive or CreditStatus required" }
        return KagemushaWalletSetupInputV1(if (status == CREDIT_STATUS) 17 else 16, first = retainedBytes)
    }
    internal fun exchange(owner: Any): KagemushaWalletTimeExchangeV1 {
        if (status != TIME_CHALLENGE || sequenceLow <= 0 || sequenceHigh != 0L || detail != 0 ||
            retainedBytes.size != 32 || retainedBytes.all { it == 0.toByte() }) invalid()
        return KagemushaWalletTimeExchangeV1(owner, sequenceLow, retainedBytes)
    }
    internal fun timeRetained() {
        if (status != TIME_RETAINED || sequenceLow != 0L || sequenceHigh != 0L || detail != 0 || retainedBytes.isNotEmpty()) invalid()
    }
    internal fun idle() {
        if (status != IDLE || sequenceLow != 0L || sequenceHigh != 0L || detail != 0 || retainedBytes.isNotEmpty()) invalid()
    }
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
        /** Durable intent exists, but irreversible Advance has not selected it. */
        const val PREPARING = 11
        const val SETUP = 12
        const val TIME_CHALLENGE = 13
        const val TIME_RETAINED = 14
        const val ACCOUNT_CHALLENGE = 15
        const val OPENED = 16
        const val ACTIVATION = 17
        const val ENROLLMENT_READY = 26
        const val ENROLLMENT_DISPATCH = 27
        const val ENROLLMENT_ABANDONMENT = 28
        const val BACKGROUND_STATUS = 29
        const val CLOSE_LOADS = 30
        const val LEDGER_INSTRUCTION = 40
        const val LOAD_FINALITY = 41
        const val UNLOAD_CONFIRMATION = 42
        const val LOAD_PROOF_PROGRESS = 43
    }
}

/**
 * One exclusive native wallet owner. Calls may run on workers; activity and payment arrival
 * can preempt folding because this wrapper does not hold a managed lock across a native call.
 * Closing joins cooperative folding and retains every already committed payment in custody.
 *
 * Created only after account admission through an independently provisioned native runtime.
 */
class KagemushaWalletV1 internal constructor(handle: Long) : Closeable, KagemushaWalletCleanupResourceV1 {
    private val owner = AtomicLong(handle)
    private val closeGate = Any()
    private val retired = java.util.concurrent.atomic.AtomicBoolean(false)
    @Volatile private var closeFailure: Throwable? = null
    private fun handle(): Long {
        closeFailure?.let { throw it }
        check(!retired.get()) { "Native wallet owner is retired" }
        return owner.get().takeIf { it > 0 } ?: throw KagemushaWalletExceptionV1(-2)
    }
    private fun call(operation: Int, first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf()): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeV1.call(handle(), operation, first.copyOf(), second.copyOf())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value.completion()
    }
    private fun execute(input: KagemushaWalletOperationInputV1): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeV1.execute(handle(), input.requestId(), input.selector,
            input.amount.low, input.amount.high, input.first(), input.second(), input.third())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value.completion()
    }
    private fun setup(input: KagemushaWalletSetupInputV1): KagemushaWalletCallV1 {
        val value = KagemushaWalletNativeV1.setup(handle(), input.identity(), input.selector,
            input.amount.low, input.amount.high, input.token, input.first(), input.second(), input.third())
            ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (value.status < 0) throw KagemushaWalletExceptionV1(value.status, value.reason, value.platformCode)
        return value
    }
    /** Complete restart-safe native enrollment Bootstrap. */
    fun bootstrap(): KagemushaWalletCallV1 = setup(KagemushaWalletSetupInputV1(0)).completion()
    /** Freeze the exact next Load instruction under durable Native request custody.
     * Retry the same request ID and amount after uncertain delivery. */
    fun prepareLedgerLoad(requestId: ByteArray, amount: KagemushaWalletUInt128V1): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(27, identity = requestId, amount = amount))
        if (result.status != KagemushaWalletCallV1.LEDGER_INSTRUCTION) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }
    /** Produce recursive finality from the selected Native history and actual originals.
     * Receipt DATA and ordinary ledger finality alone cannot credit the wallet. */
    fun proveLoadFinality(receipt: ByteArray, eventProof: ByteArray): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(28, first = receipt, second = eventProof))
        if (result.status != KagemushaWalletCallV1.LOAD_FINALITY) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }
    /** Canonical instruction framing; ledger execution still validates every authority. */
    fun ledgerInstruction(kind: KagemushaWalletLedgerTransportV1, original: ByteArray): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(29, token = kind.tag, first = original))
        if (result.status != KagemushaWalletCallV1.LEDGER_INSTRUCTION) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }
    /** Receipt-bound Native proof progress; this never treats a server height as finality. */
    fun loadFinalityProgress(receipt: ByteArray): KagemushaWalletLoadProofProgressV1 =
        KagemushaWalletLoadProofProgressV1(setup(KagemushaWalletSetupInputV1(31, first = receipt)))
    /** Verify the next original ordinary proof for this retained Load's independent history. */
    fun ingestLoadFinality(receipt: ByteArray, original: ByteArray): KagemushaWalletLoadProofProgressV1 =
        KagemushaWalletLoadProofProgressV1(setup(KagemushaWalletSetupInputV1(32, first = receipt, second = original)))
    /** Native authenticates this exact Unload in the selected committed block's successful transaction. */
    fun confirmLedgerUnload(transactionHash: ByteArray, original: ByteArray): KagemushaWalletUnloadConfirmationV1 =
        KagemushaWalletUnloadConfirmationV1(setup(KagemushaWalletSetupInputV1(30, identity = transactionHash, first = original)))
    /** Exact retained Activate frame for ledger submission; this is not activation confirmation. */
    fun activation(): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(15))
        if (result.status != KagemushaWalletCallV1.ACTIVATION) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }
    /** Exact native signed CloseLoads frame for ledger submission, not closure confirmation.
     * Reuse requestId for exact retries; use a fresh id after a preissued Load and new complete consumer. */
    fun closeLoads(requestId: ByteArray): ByteArray {
        val result = setup(KagemushaWalletSetupInputV1(19, identity = requestId))
        if (result.status != KagemushaWalletCallV1.CLOSE_LOADS) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        return result.bytes()
    }
    /** Read both exact originals from one native-retained fee claim; null means no pending claim. */
    fun feeClaim(creditId: ByteArray): KagemushaWalletFeeClaimV1? {
        val result = setup(KagemushaWalletSetupInputV1(20, identity = creditId))
        if (result.status == 32) return null
        if (result.status != 31) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        val original = result.bytes()
        return KagemushaWalletFeeClaimV1(
            setup(KagemushaWalletSetupInputV1(21, first = original)).original(),
            setup(KagemushaWalletSetupInputV1(22, first = original)).original(),
        )
    }
    /** Canonical ledger FeeClaim; beneficiary is an original AccountId checked by Native
     * against the retained schedule. Null means no pending retained claim. */
    fun feeClaimTransport(creditId: ByteArray, beneficiary: ByteArray): ByteArray? {
        val retained = setup(KagemushaWalletSetupInputV1(20, identity = creditId))
        if (retained.status == 32) return null
        return setup(retained.feeClaimInput(beneficiary)).feeClaimOriginal()
    }
    /** Verify one bounded next native Sumeragi proof and durably select its prefix. */
    fun ingestLedgerFinality(original: ByteArray): KagemushaWalletLedgerProgressV1 =
        KagemushaWalletLedgerProgressV1(setup(KagemushaWalletSetupInputV1(23, first = original)))
    /** Selected native progress, or null before any checkpoint has been selected. */
    fun ledgerProgress(): KagemushaWalletLedgerProgressV1? {
        val result = setup(KagemushaWalletSetupInputV1(24))
        return if (result.status == 34) null else KagemushaWalletLedgerProgressV1(result)
    }
    /** Native authenticates exact World and payout-row originals against its selected tip. */
    fun acknowledgeFeePayout(creditId: ByteArray, world: ByteArray, payout: ByteArray) {
        val result = setup(KagemushaWalletSetupInputV1(25, identity = creditId, first = world, second = payout))
        if (result.status != 35) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    }
    /** Retain the native Offer nonce and return exact original bytes under this retry identity. */
    fun offer(setupId: ByteArray, amount: KagemushaWalletUInt128V1): ByteArray =
        setup(KagemushaWalletSetupInputV1(1, setupId, amount)).original()
    /** Native authenticates the Offer and derives an exact Request from its selected policy. */
    fun request(setupId: ByteArray, offer: ByteArray, feeSchedule: ByteArray? = null, feeCertificate: ByteArray? = null): ByteArray {
        require((feeSchedule == null) == (feeCertificate == null)) { "fee originals must be paired" }
        if (feeSchedule != null) require(feeSchedule.isNotEmpty() && feeCertificate!!.isNotEmpty()) { "fee originals must not be empty" }
        return setup(KagemushaWalletSetupInputV1(2, setupId, first = offer,
            second = feeSchedule ?: byteArrayOf(), third = feeCertificate ?: byteArrayOf())).original()
    }
    /** Convert durable Receive/Status bytes to canonical peer evidence, without granting a proof verdict. */
    fun credited(received: KagemushaWalletCallV1): ByteArray {
        return setup(received.creditedInput()).original()
    }
    /** Native verifies delivery evidence and derives its private Archive operation. */
    fun acceptCredited(original: ByteArray): KagemushaWalletCallV1 =
        setup(KagemushaWalletSetupInputV1(3, first = original)).completion()
    /** Only the native nonce leaves clock custody; this token has no serialized form. */
    fun beginTimeExchange(): KagemushaWalletTimeExchangeV1 =
        setup(KagemushaWalletSetupInputV1(4)).exchange(this)
    /** Consume the native token and retain the verified response before TimeAnchor refresh. */
    fun finishTimeExchange(exchange: KagemushaWalletTimeExchangeV1, anchor: ByteArray, certificate: ByteArray) {
        // Copy and bound the originals before consuming this owner's one-use challenge.
        val input = KagemushaWalletSetupInputV1(5, token = exchange.tokenFor(this), first = anchor, second = certificate)
        handle()
        exchange.consume(this)
        setup(input).timeRetained()
    }
    /** Discard an unanswered native exchange without accepting or refreshing an anchor. */
    fun cancelTimeExchange(exchange: KagemushaWalletTimeExchangeV1) {
        val input = KagemushaWalletSetupInputV1(6, token = exchange.tokenFor(this))
        handle()
        exchange.consume(this)
        setup(input).idle()
    }
    /** Frame exact original bytes for peer transport, without monetary admission. */
    fun envelope(kind: KagemushaWalletTransportKindV1, original: ByteArray): ByteArray =
        setup(KagemushaWalletSetupInputV1(6 + kind.tag, first = original)).original()
    /** Extract an expected original kind under this native wallet's scheme, without accepting value. */
    fun original(kind: KagemushaWalletTransportKindV1, envelope: ByteArray): ByteArray =
        setup(KagemushaWalletSetupInputV1(10 + kind.tag, first = envelope)).original()
    /** Load the exact ordinary-ledger receipt and compact finality evidence. */
    fun load(requestId: ByteArray, receipt: ByteArray, finality: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 0, first = receipt, second = finality))
    /** Authenticate Send and its fully folded source before fresh hardware UI confirmation. */
    fun reviewSend(request: ByteArray, destinationAccountOriginal: ByteArray): KagemushaWalletReviewV1 {
        require(request.isNotEmpty() && request.size <= 10_000)
        require(destinationAccountOriginal.isNotEmpty() && destinationAccountOriginal.size <= 4_096)
        return review(1, KagemushaWalletUInt128V1(0, 0), request, destinationAccountOriginal, KagemushaWalletReviewProjectionV1.Kind.SEND)
    }
    /** Receive selects the durably issued Request locally by the Payment's digest. */
    fun receive(requestId: ByteArray, payment: ByteArray, payerCredential: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 2, first = payment, second = payerCredential, third = certificates))
    /** Retain and forward the exact signed session Offer; Native extracts its payer originals.
     * This performs ordinary Receive authentication and never treats an Offer as payment authority.
     */
    fun receiveFromOffer(requestId: ByteArray, payment: ByteArray, offer: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, 10, first = payment, second = offer))

    /** Refresh from exact signed originals; Native derives maps and all effective values. */
    fun refresh(requestId: ByteArray, kind: KagemushaWalletRefreshKindV1, update: ByteArray, certificates: ByteArray): KagemushaWalletCallV1 =
        execute(KagemushaWalletOperationInputV1(requestId, kind.selector, first = update, second = certificates))
    /** Authenticate Unload's exact payout and optional signed charge before fresh hardware confirmation. */
    fun reviewUnload(amount: KagemushaWalletUInt128V1, quote: ByteArray? = null, certificates: ByteArray? = null): KagemushaWalletReviewV1 {
        require((quote == null) == (certificates == null)) { "quote and certificates must be supplied together" }
        if (quote != null) require(quote.isNotEmpty() && certificates!!.isNotEmpty())
        require(amount.low != 0L || amount.high != 0L)
        require((quote?.size ?: 0) <= 1_024 && (certificates?.size ?: 0) <= 10_000)
        return review(8, amount, quote ?: byteArrayOf(), certificates ?: byteArrayOf(), KagemushaWalletReviewProjectionV1.Kind.UNLOAD)
    }
    private fun review(selector: Int, amount: KagemushaWalletUInt128V1, first: ByteArray, second: ByteArray, expected: KagemushaWalletReviewProjectionV1.Kind): KagemushaWalletReviewV1 {
        val wallet = handle()
        var reply: KagemushaWalletReviewReplyV1? = null
        try {
            reply = KagemushaWalletNativeV1.review(wallet, selector, amount.low, amount.high, first.copyOf(), second.copyOf())
            return (reply ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)).review(this, expected)
        } catch (failure: KagemushaWalletExceptionV1) {
            if (failure.status == KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT) {
                val token = reply?.cleanupToken()
                try {
                    if (token == null) close()
                    else checked(KagemushaWalletNativeV1.discardReview(wallet, token))
                } catch (cleanup: Throwable) {
                    if(failure !== cleanup)failure.addSuppressed(cleanup)
                    try { close() } catch (retirement: Throwable) { if(failure !== retirement)failure.addSuppressed(retirement) }
                }
            }
            throw failure
        }
    }
    /** Consume the exact reviewed Native operation after fresh hardware confirmation. Native rechecks its whole source. */
    fun executeReviewed(review: KagemushaWalletReviewV1, requestId: ByteArray): KagemushaWalletCallV1 {
        word(requestId); require(requestId.any { it != 0.toByte() })
        val value = handle()
        val token = review.consume(this)
        try {
            val reply = KagemushaWalletNativeV1.executeReviewed(value, token, requestId.copyOf())
                ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
            if (reply.status < 0) throw KagemushaWalletExceptionV1(reply.status, reply.reason, reply.platformCode)
            return reply.completion()
        } catch (failure: Throwable) {
            // Native may have consumed this token before failure. Closed then means no unused
            // review remains; it is never treated as acknowledgement of owner release.
            try {
                val status=KagemushaWalletNativeV1.discardReview(value,token)
                if(status!=0 && status!=-2)checked(status)
            }catch(cleanup:Throwable){
                if(failure !== cleanup)failure.addSuppressed(cleanup)
                try{close()}catch(retirement:Throwable){if(failure !== retirement)failure.addSuppressed(retirement)}
            }
            throw failure
        }
    }
    /** Discard an unused review on a worker without creating intent, payment or monetary authority. */
    fun discardReview(review: KagemushaWalletReviewV1) {
        val value = handle(); val token = review.consume(this)
        checked(KagemushaWalletNativeV1.discardReview(value, token))
    }
    /** Same actual one-use discard operation, retained for the current managed integration. */
    fun cancelReview(review:KagemushaWalletReviewV1)=discardReview(review)
    /** Enter Retiring under native folded-state checks. */
    fun retire(requestId: ByteArray): KagemushaWalletCallV1 = execute(KagemushaWalletOperationInputV1(requestId, 9))
    /** Resolve the local request identity, including distinct Preparing and irreversible Pending. */
    fun requestStatus(requestId: ByteArray): KagemushaWalletCallV1 {
        word(requestId); require(requestId.any { it != 0.toByte() }); return call(5, requestId)
    }
    /** Retrieve exact retained bytes; unknown, pending and delivery loss remain distinct. */
    fun retry(operationId: ByteArray): KagemushaWalletCallV1 { word(operationId); return call(1, operationId) }
    /** Reconcile and finish the selected operation without a second debit. */
    fun resume(): KagemushaWalletCallV1 = call(2)
    /** Observe the native worker without waiting for its proof or preempting it.
     * A retained failure is thrown; later activity/payment can resume after observation. */
    fun foldStatus(): KagemushaWalletBackgroundStatusV1 =
        KagemushaWalletBackgroundStatusV1(setup(KagemushaWalletSetupInputV1(18)))
    /** Compute at most one checkpoint. Run off the UI thread; activity enables background work. */
    fun foldOnce(): KagemushaWalletCallV1 = call(3)
    /** Native ownership and proof backlog, without operation readiness. Call on a worker. */
    fun snapshot(): KagemushaWalletSnapshotV1 {
        val reply = KagemushaWalletNativeV1.snapshot(handle()) ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (reply.status < 0) throw KagemushaWalletExceptionV1(reply.status, reply.reason, reply.platformCode)
        return KagemushaWalletSnapshotV1(reply)
    }
    /** Canonical CreditStatus for the immutable first Payment identity. */
    fun creditStatus(creditId: ByteArray, paymentDigest: ByteArray): KagemushaWalletCallV1 {
        word(creditId); word(paymentDigest); return call(4, creditId, paymentDigest)
    }
    /** Foreground or charging enables folding; leaving both cancels at a cooperative boundary. */
    fun setActivity(foreground: Boolean, charging: Boolean) {
        checked(KagemushaWalletNativeV1.activity(handle(), if (foreground) 1 else 0, if (charging) 1 else 0))
    }
    /** Release exclusive native custody. Never deletes key material, markers or retained bytes. */
    override fun close() = synchronized(closeGate) { closeFailure?.let{throw it}; closeAttempt() }
    /** Explicit retry retains the same Native ID and permanently fences every operation. */
    override fun retryCleanup() = synchronized(closeGate) { closeAttempt() }
    override fun cleanupReleased():Boolean = owner.get()==0L
    private fun closeAttempt() {
        retired.set(true)
        val handle=owner.get()
        if(handle>0) {
            try{checked(KagemushaWalletNativeV1.close(handle));owner.set(0);closeFailure=null}
            catch(failure:Throwable){
                closeFailure=failure
                KagemushaWalletInstalledRuntimeV1.retainFailure(this,failure)
                throw failure
            }
        }
    }

    override fun toString(): String = "KagemushaWalletV1(owner=[REDACTED])"
    companion object {
        private fun word(bytes: ByteArray) { require(bytes.size == 32) { "identity must be exactly32 bytes" } }
        private fun checked(status: Int) { if (status != 0) throw KagemushaWalletExceptionV1(status) }

    }
}

/** JNI declarations share the current authenticated native library loader. */
internal object KagemushaWalletNativeV1 {
    @JvmStatic external fun revision(): Int
    @JvmStatic external fun openBegin(runtime: Long, credential: ByteArray, certificates: ByteArray, account: ByteArray, asset: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun openFinish(runtime: Long, signature: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun openCancel(runtime: Long): Int
    @JvmStatic external fun close(handle: Long): Int
    @JvmStatic external fun activity(handle: Long, foreground: Int, charging: Int): Int
    @JvmStatic external fun call(handle: Long, operation: Int, first: ByteArray, second: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun enrollment(runtime: Long, selector: Int, first: ByteArray, second: ByteArray, third: ByteArray, certificates: Array<ByteArray>): KagemushaWalletCallV1?
    @JvmStatic external fun setup(handle: Long, setupId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, token: Long, first: ByteArray, second: ByteArray, third: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun execute(handle: Long, requestId: ByteArray, selector: Int, amountLow: Long, amountHigh: Long, first: ByteArray, second: ByteArray, third: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun snapshot(handle: Long): KagemushaWalletSnapshotReplyV1?
    @JvmStatic external fun review(handle: Long, selector: Int, amountLow: Long, amountHigh: Long, first: ByteArray, second: ByteArray): KagemushaWalletReviewReplyV1?
    @JvmStatic external fun executeReviewed(handle: Long, token: Long, requestId: ByteArray): KagemushaWalletCallV1?
    @JvmStatic external fun discardReview(handle: Long, token: Long): Int
}

/** Existing signed policy classes; these selectors never choose proof keys or state roots. */
enum class KagemushaWalletRefreshKindV1(internal val selector: Int) {
    CREDENTIAL(3), SCHEME_POLICY(4), BLACKLIST(5), TIME_ANCHOR(6), QUOTA_SHARE(7),
}

/** Allocation-bounded copies for the fixed JNI request. Validation is not monetary admission. */
internal class KagemushaWalletOperationInputV1(
    requestId: ByteArray,
    val selector: Int,
    val amount: KagemushaWalletUInt128V1 = KagemushaWalletUInt128V1(0, 0),
    first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(), third: ByteArray = byteArrayOf(),
) {
    private val identity: ByteArray
    private val originals: List<ByteArray>
    init {
        val limits = when (selector) {
            0 -> intArrayOf(512, 16_384, 0)
            2 -> intArrayOf(10_000, 1_024, 10_000)
            3, 4 -> intArrayOf(1_024, 10_000, 0)
            5 -> intArrayOf(65_536 * 34 + 512, 10_000, 0)
            6 -> intArrayOf(512, 10_000, 0)
            7 -> intArrayOf(8_192, 10_000, 0)
            9 -> intArrayOf(0, 0, 0)
            10 -> intArrayOf(10_000, 10_000, 0)
            else -> throw IllegalArgumentException("unknown lifecycle operation")
        }
        require(requestId.size == 32 && requestId.any { it != 0.toByte() }) { "nonzero request identity must be exactly 32 bytes" }
        val nonzero = amount.low != 0L || amount.high != 0L
        require(!nonzero) { "ordinary operations have no caller amount" }
        val inputs = listOf(first, second, third)
        for (index in inputs.indices) {
            require(inputs[index].size <= limits[index]) { "original exceeds operation bound" }
            if ((selector < 8 || selector == 10) && limits[index] != 0) require(inputs[index].isNotEmpty()) { "required original is empty" }
        }
        identity = requestId.copyOf()
        originals = inputs.map { it.copyOf() }
    }
    fun requestId(): ByteArray = identity.copyOf()
    fun first(): ByteArray = originals[0].copyOf()
    fun second(): ByteArray = originals[1].copyOf()
    fun third(): ByteArray = originals[2].copyOf()
    override fun toString(): String = "KagemushaWalletOperationInputV1(originals=[REDACTED])"
}
