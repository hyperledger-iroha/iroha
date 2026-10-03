// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.OutputStream
import java.math.BigInteger
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyStoreV1

/** Protected product transport under the genuine S/customer/FI/session binding. Responses remain
 * untrusted until the same Native lineage owner admits their complete originals. A product must
 * implement the genuine service; this carrier neither invents an issuer origin nor installs it.
 */
fun interface KagemushaOrdinaryLineageOriginalTransportV1 {
    suspend fun exchange(original: KagemushaOrdinaryLineageHttpOriginalV1): ByteArray
}
/** Private-origin HTTP data, selected solely from one real Native request and account signature. */
class KagemushaOrdinaryLineageHttpOriginalV1 private constructor(fields: List<ByteArray>, private val guard: () -> Unit) {
    val path = "/v1/kagemusha/enrollment/ordinary/lineage-cas"
    val maximumResponseBytes = KagemushaOrdinaryLineageHttpCodecV1.MAXIMUM_RESPONSE_BYTES
    val requestId = KagemushaOrdinaryLineageHttpCodecV1.requestId(fields[1])
    private val request=fields[1].copyOf()
    private val signature=fields[2].copyOf()
    private val proof=fields[3].copyOf()
    fun requireCurrent() = guard()
    fun writeBodyTo(output:OutputStream) {
        requireCurrent();KagemushaOrdinaryLineageHttpCodecV1.writeRequestBody(request,signature,proof,output);requireCurrent()
    }
    internal fun clear(){request.fill(0);signature.fill(0);proof.fill(0)}
    internal companion object {
        fun selected(fields: List<ByteArray>, guard: () -> Unit):KagemushaOrdinaryLineageHttpOriginalV1 {
            KagemushaOrdinaryOutgoingFrameV1.requireResponse(6,fields)
            check(fields[0].contentEquals(byteArrayOf(0)))
            return KagemushaOrdinaryLineageHttpOriginalV1(fields,guard)
        }
    }
}
/** Detached exact acknowledged outgoing bytes. This cannot recreate proof, State or money custody. */
class KagemushaOrdinaryCommittedOutgoingOriginalV1 internal constructor(key: ByteArray, original: ByteArray) {
    private val request = key.copyOf(); private val bytes = original.copyOf()
    fun commitRequestOriginalSha256(): ByteArray = request.copyOf()
    fun completeOutgoingOriginal(): ByteArray = bytes.copyOf()
}

internal interface OrdinaryOutgoingApprovalStepV1 { suspend fun approve() }
internal interface OrdinaryOutgoingWorkflowNativeV1 {
    suspend fun prepare(kind:Int,business:ByteArray,amount:BigInteger?):OrdinaryOutgoingApprovalStepV1
    suspend fun terminal(reserveKey:ByteArray):OrdinaryOutgoingApprovalStepV1
    suspend fun invoke(phase:Int,fields:List<ByteArray> = emptyList()):List<ByteArray>
    fun requireOpen();fun revoke()
}
/** Genuine ordinary Send/Redemption through distinct W2/W1 approvals, released proofs, global CAS,
 * durable StateAdvance and separate FI Ack. Retain this workflow for same-original HTTP retries.
 * Pure proof results survive finite account-read expiry; phase16 genuinely renews the same S/W.
 */
class KagemushaOrdinaryOutgoingCashV1 internal constructor(
    private val native:OrdinaryOutgoingWorkflowNativeV1,
    private val financial:suspend(Boolean)->Unit,
    private val integrity:suspend()->Unit,
    private val transport:KagemushaOrdinaryLineageOriginalTransportV1,
    private val requireOriginalOwner:()->Unit,
) {
    constructor(coordinator:KagemushaNativeCoreCoordinatorAdapterV1,hardware:KagemushaAndroidHardwareAppKeyStoreV1,
        currentControl:KagemushaOrdinaryCurrentControlV1,integrity:KagemushaOrdinaryIntegrityRefreshV1,
        transport:KagemushaOrdinaryLineageOriginalTransportV1,requireOriginalOwner:()->Unit):this(
        ActualNative(coordinator,hardware),{fresh->if(fresh)currentControl.refreshCurrentFinancialControl()
            else currentControl.beginOrResumeCurrentFinancialControl()},
        {integrity.refreshCurrentIntegrity();Unit},transport,requireOriginalOwner)
    private class ActualNative(private val coordinator:KagemushaNativeCoreCoordinatorAdapterV1,
        private val hardware:KagemushaAndroidHardwareAppKeyStoreV1):OrdinaryOutgoingWorkflowNativeV1 {
        private val binding=coordinator.ordinaryOutgoingTransportBinding()
        private val io=KagemushaRetainedNativeIoV1()
        override fun requireOpen()=binding.requireOpen()
        override fun revoke(){try{binding.revoke()}finally{io.retire()}}
        override suspend fun invoke(phase:Int,fields:List<ByteArray>):List<ByteArray> = io.call {
            binding.invoke(KagemushaOrdinaryRuntimeJniV1,phase,fields)
        }
        override suspend fun prepare(kind:Int,business:ByteArray,amount:BigInteger?):OrdinaryOutgoingApprovalStepV1=io.call {
            val held=if(kind==2)coordinator.appIdentityOperations().prepareOrdinarySendApproval(business)
                else coordinator.appIdentityOperations().prepareOrdinaryRedemptionApproval(checkNotNull(amount))
            object:OrdinaryOutgoingApprovalStepV1 {override suspend fun approve(){io.call{hardware.approve(held).fill(0)}}}
        }
        override suspend fun terminal(reserveKey:ByteArray):OrdinaryOutgoingApprovalStepV1=io.call {
            val held=binding.selectTerminal(KagemushaOrdinaryRuntimeJniV1,reserveKey)
            object:OrdinaryOutgoingApprovalStepV1 {override suspend fun approve(){io.call{hardware.approveOrdinaryTerminal(held).fill(0)}}}
        }
    }
    private var financialPending=false
    private suspend fun refreshDependencies() {
        effect{invoke(16)}
        try {
            integrity();current()
            val fresh=!financialPending;financialPending=true;financial(fresh);current();financialPending=false
        }catch(failure:Throwable) {
            // The actual PI/FI workflow owns its immutable HTTP uncertainty and Native fences.
            // Retain that workflow and this proof; retry cannot select a replacement nonce.
            try{current()}catch(revoked:Throwable){freeze(revoked)}
            throw failure
        }
    }
    private val active = AtomicBoolean(false)
    @Volatile private var frozen = false
    private class Dispatch(val phase: Int, val fields: List<ByteArray>) {
        var carrier: KagemushaOrdinaryLineageHttpOriginalV1? = null
        var response: ByteArray? = null
        var intakeStarted = false
        var completed = false
    }
    private class Cycle(val kind: Int, val business: ByteArray) {
        var prepareStarted = false
        var preparation: OrdinaryOutgoingApprovalStepV1? = null
        var w2Captured = false
        var reservationProved = false
        var reserve: Dispatch? = null
        var terminal: OrdinaryOutgoingApprovalStepV1? = null
        var w1Captured = false
        var commitProved = false
        var commit: Dispatch? = null
        var stateAdvanced = false
        var acknowledged = false
        var completed: KagemushaOrdinaryCommittedOutgoingOriginalV1? = null
    }
    private var cycle: Cycle? = null

    /** Native independently admits the exact receiver request. W is never derived from S or a DTO. */
    suspend fun beginOrResumeSend(originalReceiverRequest: ByteArray): KagemushaOrdinaryCommittedOutgoingOriginalV1 {
        require(originalReceiverRequest.size in 1..4096)
        return perform(2, originalReceiverRequest.copyOf(), null)
    }
    /** Fixed positive exact-u128 redemption; Native selects the actual enrolled beneficiary W. */
    suspend fun beginOrResumeRedemption(amount: BigInteger): KagemushaOrdinaryCommittedOutgoingOriginalV1 {
        require(amount.signum() > 0 && amount.bitLength() <= 128)
        val encoded = amount.toByteArray()
        val original = ByteArray(16) { i -> if (i < encoded.size) encoded[encoded.lastIndex - i] else 0 }
        return perform(4, original, amount)
    }
    private suspend fun perform(kind: Int, business: ByteArray, amount: BigInteger?): KagemushaOrdinaryCommittedOutgoingOriginalV1 {
        check(active.compareAndSet(false, true)) { "An ordinary cash operation is already active" }
        try {
            current()
            val original = cycle ?: Cycle(kind, business.copyOf()).also { cycle = it }
            require(original.kind == kind && MessageDigest.isEqual(original.business, business)) {
                "An unfinished original cannot be replaced with another outgoing operation"
            }
            original.completed?.let { return it }
            if (!original.prepareStarted) {
                // This genuine Native/current FI workflow creates or recovers Cash from the
                // same actual published Bootstrap; hardware evidence alone cannot do so.
                financial(false);current()
                refreshDependencies()
                original.prepareStarted = true // Own the attempt before Native can fsync W2.
                original.preparation = effect {
                    native.prepare(kind,original.business,amount)
                }
            }
            if (!original.w2Captured) {
                effect { checkNotNull(original.preparation).approve() }
                original.w2Captured = true // Purpose2 capture is not a terminal/publication grant.
            }
            if (!original.reservationProved) {
                effect { invoke(5) }; original.reservationProved = true
            }
            if (original.reserve == null) {refreshDependencies();original.reserve = effect { Dispatch(6, invoke(6)) }}
            dispatch(checkNotNull(original.reserve))
            val reserveKey = checkNotNull(original.reserve).fields[4]
            if (original.terminal == null) {
                refreshDependencies()
                original.terminal = effect { native.terminal(reserveKey) }
            }
            if (!original.w1Captured) {
                effect { checkNotNull(original.terminal).approve() }
                original.w1Captured = true
            }
            if (!original.commitProved) { effect { invoke(12) }; original.commitProved = true }
            if (original.commit == null) {refreshDependencies();original.commit = effect { Dispatch(13, invoke(13)) }}
            dispatch(checkNotNull(original.commit))
            val commitKey = checkNotNull(original.commit).fields[4]
            if (!original.stateAdvanced) {
                refreshDependencies();effect { invoke(14, listOf(commitKey)) }; original.stateAdvanced = true
            }
            if (!original.acknowledged) {
                // A separately fresh actual FI/current read gates post-State-fsync acknowledgment.
                // Its HTTP uncertainty retains its own originals. No receipt or callback is invented.
                refreshDependencies()
                effect { invoke(15, listOf(commitKey)) }; original.acknowledged = true
            }
            val delivery = effect { invoke(17, listOf(commitKey)).single() }
            val completed = KagemushaOrdinaryCommittedOutgoingOriginalV1(commitKey, delivery)
            current(); original.completed = completed
            return completed
        } finally { business.fill(0); active.set(false) }
    }
    /** Release only an acknowledged cycle. A pending operation cannot be replaced or cancelled. */
    fun releaseCompletedCycle() {
        check(active.compareAndSet(false,true)) { "An ordinary cash operation is already active" }
        try {
            current();val original=checkNotNull(cycle);check(original.completed!=null && original.acknowledged)
            original.business.fill(0)
            listOfNotNull(original.reserve,original.commit).forEach { held ->
                held.fields.forEach {it.fill(0)};held.carrier?.clear();held.response?.fill(0)
            }
            cycle=null;current()
        }finally{active.set(false)}
    }
    private suspend fun dispatch(original: Dispatch) {
        if (original.completed) return
        if (original.fields[0].contentEquals(byteArrayOf(2))) { original.completed = true; return }
        if (original.carrier == null) original.carrier = KagemushaOrdinaryLineageHttpOriginalV1.selected(original.fields) {
            current(); check(!original.intakeStarted && !original.completed) { "The original lineage transport has entered Native intake" }
        }
        if (original.response == null) {
            val response = try { transport.exchange(checkNotNull(original.carrier)) }
                catch (failure: Throwable) {
                    // Sole safe retry: HTTP-only uncertainty, same request/signature/service body.
                    // Native/session failure revokes the descriptor and cannot restart its effects.
                    try { current() } catch (revoked: Throwable) { freeze(revoked) }
                    throw failure
                }
            original.response = effect {
                current(); require(response.size in 1..KagemushaOrdinaryLineageHttpCodecV1.MAXIMUM_RESPONSE_BYTES)
                response.copyOf()
            }
        }
        refreshDependencies()
        effect {
            current(); check(!original.intakeStarted)
            val originals = KagemushaOrdinaryLineageHttpCodecV1.responseOriginals(checkNotNull(original.response))
            original.intakeStarted = true // Own before any global acknowledgement may be fsynced.
            val key = invoke(7, originals).single()
            check(MessageDigest.isEqual(key, original.fields[4])) { "The acknowledged lineage request differs" }
            current(); original.completed = true; original.response?.fill(0); original.response = null
            original.carrier?.clear();original.carrier=null
        }
    }
    private suspend fun invoke(phase: Int, fields: List<ByteArray> = emptyList()): List<ByteArray> {
        current(); val response = native.invoke(phase,fields)
        current(); return response
    }
    private fun current() { check(!frozen); requireOriginalOwner(); native.requireOpen(); requireOriginalOwner() }
    private suspend fun <T> effect(body: suspend () -> T): T = try { current(); body().also { current() } } catch (failure: Throwable) { freeze(failure) }
    private fun freeze(failure: Throwable): Nothing {
        frozen = true
        try { native.revoke() } catch (_: Throwable) { }
        throw failure
    }
}
