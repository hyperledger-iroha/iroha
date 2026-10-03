// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.io.OutputStream
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyStoreV1

/** Protected product transport for exact Native-retained request/signature/proof originals.
 * Implementations must stream the body and bound the complete response while reading.
 */
fun interface KagemushaOrdinaryIncomingOriginalTransportV1 {
    suspend fun exchange(original: KagemushaOrdinaryIncomingHttpOriginalV1): ByteArray
}

/** Closed origin, immutable transport carrier. HTTP retry reuses every original and ID. */
class KagemushaOrdinaryIncomingHttpOriginalV1 private constructor(fields: List<ByteArray>,
    private val commit: Boolean, private val guard: ()->Unit) {
    val path = "/v1/kagemusha/enrollment/ordinary/lineage-cas"
    val maximumResponseBytes = KagemushaOrdinaryIncomingHttpCodecV1.MAXIMUM_RESPONSE_BYTES
    val requestId = KagemushaOrdinaryIncomingHttpCodecV1.requestId(fields[1])
    private val request = fields[1].copyOf()
    private val requestDigest = fields[4].copyOf()
    private val signature = fields[2].copyOf()
    private val proof = fields[3].copyOf()
    fun requireCurrent() = guard()
    fun writeBodyTo(output: OutputStream) {
        requireCurrent()
        KagemushaOrdinaryIncomingHttpCodecV1.writeRequestBody(request,signature,proof,commit,output)
        requireCurrent()
    }
    internal fun requestOriginalSha256(): ByteArray = requestDigest.copyOf()
    internal fun clear() { request.fill(0); signature.fill(0); proof.fill(0) }
    internal companion object {
        fun fromNative(fields: List<ByteArray>,commit: Boolean,guard: ()->Unit): KagemushaOrdinaryIncomingHttpOriginalV1 {
            KagemushaOrdinaryIncomingFrameV1.requireResponse(if (commit) 13 else 6,fields)
            check(fields[0].contentEquals(byteArrayOf(0)))
            return KagemushaOrdinaryIncomingHttpOriginalV1(fields,commit,guard)
        }
    }
}

/** Historical acknowledgement only; balances/credit consumption remain Native State custody. */
class KagemushaOrdinaryIncomingAcknowledgementV1 internal constructor(reserve: ByteArray,commit: ByteArray) {
    private val originalReserve = reserve.copyOf()
    private val originalCommit = commit.copyOf()
    fun reserveRequestOriginalSha256(): ByteArray = originalReserve.copyOf()
    fun commitRequestOriginalSha256(): ByteArray = originalCommit.copyOf()
}
internal interface OrdinaryIncomingApprovalStepV1 { suspend fun approve(); fun operationId(): ByteArray }
internal interface OrdinaryIncomingWorkflowNativeV1 {
    suspend fun refreshAccount()
    suspend fun prepareMint(finalized: ByteArray,credit: ByteArray): OrdinaryIncomingApprovalStepV1
    suspend fun prepareReceive(request: ByteArray,outgoing: ByteArray,assertion: ByteArray): OrdinaryIncomingApprovalStepV1
    suspend fun selectTerminal(reserve: ByteArray): OrdinaryIncomingApprovalStepV1
    suspend fun invoke(phase: Int,originals: List<ByteArray>): List<ByteArray>
    fun requireOpen()
    fun revoke()
}

/** Native incoming Mint/Receive lifecycle with genuine separate W2/W1 hardware approvals,
 * native proofs, immutable global CAS originals and durable StateAdvance/capture Ack.
 * Keep this object across cancellation/retry. Only exact HTTP uncertainty can redispatch;
 * an unknown Native/platform outcome cannot reset a fence or create a replacement cycle.
 */
class KagemushaOrdinaryIncomingV1 internal constructor(
    private val native: OrdinaryIncomingWorkflowNativeV1,
    private val refreshFinancialControl: suspend (Boolean)->Unit,
    private val transport: KagemushaOrdinaryIncomingOriginalTransportV1,
    private val requireOriginalOwner: ()->Unit,
    private val refreshIntegrity: suspend ()->Unit = {},
) {
    constructor(context: Context,binding: KagemushaOrdinaryIncomingTransportBindingV1,
        currentControl: KagemushaOrdinaryCurrentControlV1,integrity:KagemushaOrdinaryIntegrityRefreshV1,
        transport: KagemushaOrdinaryIncomingOriginalTransportV1,requireOriginalOwner: ()->Unit)
        : this(ActualNativeIncomingV1(binding,KagemushaAndroidHardwareAppKeyStoreV1(context.applicationContext)),
            { fresh -> if (fresh) currentControl.refreshCurrentFinancialControl()
                else currentControl.beginOrResumeCurrentFinancialControl() },transport,requireOriginalOwner,
            {integrity.refreshCurrentIntegrity();Unit})

    companion object {
        suspend fun open(context:Context,service:KagemushaAndroidOrdinaryHardwareServiceV1,
            coordinator:KagemushaNativeCoreCoordinatorAdapterV1,currentControl:KagemushaOrdinaryCurrentControlV1,
            integrity:KagemushaOrdinaryIntegrityRefreshV1,
            transport:KagemushaOrdinaryIncomingOriginalTransportV1,requireOriginalOwner:()->Unit):KagemushaOrdinaryIncomingV1 {
            requireOriginalOwner();currentControl.beginOrResumeCurrentFinancialControl()
            val binding=service.incomingTransportBinding(coordinator,KagemushaOrdinaryRuntimeJniV1)
            requireOriginalOwner();return KagemushaOrdinaryIncomingV1(context,binding,currentControl,integrity,transport,requireOriginalOwner)
        }
    }
    private class Cycle(val receive: Boolean,val originalDigests: List<ByteArray>) {
        var w2: OrdinaryIncomingApprovalStepV1? = null
        var w2Approved = false
        var candidateRetained = false
        var reserve: ByteArray? = null
        var w1: OrdinaryIncomingApprovalStepV1? = null
        var w1Approved = false
        var commitRetained = false
        var commit: ByteArray? = null
        var advanced = false
        var acknowledged: KagemushaOrdinaryIncomingAcknowledgementV1? = null
        var carrier: KagemushaOrdinaryIncomingHttpOriginalV1? = null
        var response: List<ByteArray>? = null
        var transportPhase = 0
    }
    private val running = AtomicBoolean(false)
    private var frozen = false
    private var financialRefreshPending = false
    private var cycle: Cycle? = null

    suspend fun beginOrResumeFinalizedMint(finalizedTopupOriginal: ByteArray,mintCreditOriginal: ByteArray):
        KagemushaOrdinaryIncomingAcknowledgementV1 = perform(false,listOf(finalizedTopupOriginal,mintCreditOriginal))
    suspend fun beginOrResumeReceive(receiverRequestId: ByteArray,fullOutgoingOriginal: ByteArray,
        fullReceivedAssertionOriginal: ByteArray): KagemushaOrdinaryIncomingAcknowledgementV1 =
        perform(true,listOf(receiverRequestId,fullOutgoingOriginal,fullReceivedAssertionOriginal))

    /** Explicit next cycle only after both actual durable acknowledgements completed.
     * This clears managed correlation, never a Native journal, counter, source or pending fence.
     */
    fun releaseCompletedCycle() {
        check(running.compareAndSet(false,true))
        try { current(); checkNotNull(cycle?.acknowledged); cycle = null; current() }
        finally { running.set(false) }
    }

    private suspend fun perform(receive: Boolean,inputs: List<ByteArray>): KagemushaOrdinaryIncomingAcknowledgementV1 {
        KagemushaOrdinaryIncomingFrameV1.requireRequest(if (receive) 17 else 1,inputs)
        check(running.compareAndSet(false,true)) { "An incoming cycle is already active" }
        try {
            current()
            val digests = inputs.map(::sha)
            val held = cycle?.also {
                check(it.receive == receive && it.originalDigests.indices.all { n -> same(it.originalDigests[n],digests[n]) }) {
                    "An unfinished incoming cycle cannot be replaced"
                }
            } ?: Cycle(receive,digests).also { cycle = it }
            held.acknowledged?.let { return it }
            // Local product/descriptor checks precede actual renewal; old finite Native data is
            // never consulted to authorize this renewal or replaced by managed CURRENT.
            refreshDependencies()
            if (held.w2 == null) held.w2 = actual {
                if (receive) native.prepareReceive(inputs[0],inputs[1],inputs[2]) else native.prepareMint(inputs[0],inputs[1])
            }
            if (!held.w2Approved) { actual { checkNotNull(held.w2).approve() }; held.w2Approved = true }
            if (!held.candidateRetained) { invoke(5); held.candidateRetained = true }
            if (held.reserve == null) {
                refreshDependencies() // Long genuine proof work must renew same S/W/FI before signing.
                held.reserve = exchange(held,6)
            }
            if (held.w1 == null) {
                refreshDependencies()
                held.w1 = actual { native.selectTerminal(checkNotNull(held.reserve)) }
            }
            if (!held.w1Approved) { actual { checkNotNull(held.w1).approve() }; held.w1Approved = true }
            if (!held.commitRetained) { invoke(12); held.commitRetained = true }
            if (held.commit == null) {
                refreshDependencies()
                held.commit = exchange(held,13)
            }
            if (!held.advanced) {
                refreshDependencies()
                invoke(14,listOf(checkNotNull(held.commit)))
                held.advanced = true // Native fsyncs State and retires source only after true global Ack.
            }
            invoke(15,listOf(checkNotNull(held.commit))) // Separate durable capture Ack after State fsync.
            current()
            return KagemushaOrdinaryIncomingAcknowledgementV1(checkNotNull(held.reserve),checkNotNull(held.commit))
                .also { held.acknowledged = it }
        } finally { running.set(false) }
    }
    private suspend fun refreshDependencies() {
        current()
        actual { native.refreshAccount() }
        refreshIntegrity(); current()
        val fresh = !financialRefreshPending
        financialRefreshPending = true
        try { refreshFinancialControl(fresh); current(); financialRefreshPending = false }
        catch (failure: Throwable) {
            // The actual FI workflow distinguishes HTTP-only uncertainty from unknown Native
            // effects. Resume its retained original on retry; never select a second FI nonce.
            try { current() } catch (revoked: Throwable) { freeze(revoked) }
            throw failure
        }
    }
    private suspend fun exchange(held: Cycle,phase: Int): ByteArray {
        if (held.carrier == null) {
            val fields = invoke(phase)
            if (fields[0][0] == 2.toByte()) return fields[4].copyOf() // Actual Native acknowledged-original recovery.
            held.transportPhase = phase
            held.carrier = KagemushaOrdinaryIncomingHttpOriginalV1.fromNative(fields,phase == 13) {
                current(); check(cycle === held && held.transportPhase == phase)
            }
        }
        check(held.transportPhase == phase)
        if (held.response == null) {
            val response = try { transport.exchange(checkNotNull(held.carrier)) }
            catch (failure: Throwable) { current(); throw failure } // Exact retained HTTP original only.
            current()
            held.response = try { KagemushaOrdinaryIncomingHttpCodecV1.responseOriginals(response) }
                catch (failure: Throwable) { freeze(failure) }
        }
        val acknowledged = invoke(7,checkNotNull(held.response)).single()
        check(same(acknowledged,checkNotNull(held.carrier).requestOriginalSha256())) {
            "Native acknowledged a different retained CAS request"
        }
        val result = acknowledged.copyOf()
        held.carrier?.clear(); held.carrier = null
        held.response?.forEach { it.fill(0) }; held.response = null; held.transportPhase = 0
        return result
    }
    private suspend fun invoke(phase: Int,originals: List<ByteArray> = emptyList()): List<ByteArray> = actual {
        KagemushaOrdinaryIncomingFrameV1.requireRequest(phase,originals)
        native.invoke(phase,originals).also { KagemushaOrdinaryIncomingFrameV1.requireResponse(phase,it) }
    }
    private suspend fun <T> actual(operation: suspend ()->T): T = try { current(); operation().also { current() } }
        catch (failure: Throwable) { freeze(failure) }
    private fun current() { check(!frozen); requireOriginalOwner(); native.requireOpen(); requireOriginalOwner() }
    private fun freeze(failure: Throwable): Nothing {
        frozen = true; cycle?.carrier?.clear(); cycle?.response?.forEach { it.fill(0) }
        try { native.revoke() } catch (_: Throwable) { }
        throw failure
    }
    private fun same(a: ByteArray,b: ByteArray) = MessageDigest.isEqual(a,b)
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
}

private class ActualNativeIncomingV1(private val binding: KagemushaOrdinaryIncomingTransportBindingV1,
    private val hardware: KagemushaAndroidHardwareAppKeyStoreV1): OrdinaryIncomingWorkflowNativeV1 {
    private val io=KagemushaRetainedNativeIoV1()
    override suspend fun refreshAccount() = io.call {binding.refreshCurrentAccount(KagemushaOrdinaryRuntimeJniV1)}
    override fun requireOpen() = binding.requireOpen()
    override fun revoke() {try{binding.revoke()}finally{io.retire()}}
    override suspend fun invoke(phase: Int,originals: List<ByteArray>) = io.call {binding.invoke(KagemushaOrdinaryRuntimeJniV1,phase,originals)}
    override suspend fun prepareMint(finalized: ByteArray,credit: ByteArray) = io.call {step(binding.prepareFinalizedMint(KagemushaOrdinaryRuntimeJniV1,finalized,credit))}
    override suspend fun prepareReceive(request: ByteArray,outgoing: ByteArray,assertion: ByteArray) =
        io.call {step(binding.prepareReceive(KagemushaOrdinaryRuntimeJniV1,request,outgoing,assertion))}
    override suspend fun selectTerminal(reserve: ByteArray) = io.call {step(binding.selectTerminal(KagemushaOrdinaryRuntimeJniV1,reserve))}
    private fun step(held: KagemushaNativePreparedOrdinaryIncomingApprovalV1) = object : OrdinaryIncomingApprovalStepV1 {
        override suspend fun approve() {io.call {hardware.approveOrdinaryIncoming(held)}}
        override fun operationId() = held.operationId()
    }
}
