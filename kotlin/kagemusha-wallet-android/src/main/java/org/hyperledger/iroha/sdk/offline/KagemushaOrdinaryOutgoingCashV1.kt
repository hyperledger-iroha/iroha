// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

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
    private val body = KagemushaOrdinaryLineageHttpCodecV1.requestBody(fields[1], fields[2], fields[3])
    fun requireCurrent() = guard()
    fun body(): ByteArray { requireCurrent(); return body.copyOf().also { requireCurrent() } }
    internal companion object {
        fun selected(fields: List<ByteArray>, guard: () -> Unit) = KagemushaOrdinaryLineageHttpOriginalV1(fields, guard)
    }
}
/** Detached exact acknowledged outgoing bytes. This cannot recreate proof, State or money custody. */
class KagemushaOrdinaryCommittedOutgoingOriginalV1 internal constructor(key: ByteArray, original: ByteArray) {
    private val request = key.copyOf(); private val bytes = original.copyOf()
    fun commitRequestOriginalSha256(): ByteArray = request.copyOf()
    fun completeOutgoingOriginal(): ByteArray = bytes.copyOf()
}

/** One genuine ordinary outgoing operation through W2, proof, Reserve, W1, whole Commit and
 * distinct StateAdvance/FI Ack. Generic OEM WalletV1 remains separate and unavailable without its
 * real hardware provider. Software financial WAL is not represented as hardware-sealed storage.
 */
class KagemushaOrdinaryOutgoingCashV1(
    private val coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
    private val hardware: KagemushaAndroidHardwareAppKeyStoreV1,
    private val currentControl: KagemushaOrdinaryCurrentControlV1,
    private val transport: KagemushaOrdinaryLineageOriginalTransportV1,
    private val requireOriginalOwner: () -> Unit,
) {
    private val binding = coordinator.ordinaryOutgoingTransportBinding()
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
        var preparation: KagemushaNativePreparedAppApprovalV1? = null
        var w2Captured = false
        var reservationProved = false
        var reserve: Dispatch? = null
        var terminal: KagemushaNativePreparedOrdinaryTerminalApprovalV1? = null
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
                currentControl.beginOrResumeCurrentFinancialControl()
                original.prepareStarted = true // Own the attempt before Native can fsync W2.
                original.preparation = effect {
                    if (kind == 2) coordinator.appIdentityOperations().prepareOrdinarySendApproval(original.business)
                    else coordinator.appIdentityOperations().prepareOrdinaryRedemptionApproval(checkNotNull(amount))
                }
            }
            if (!original.w2Captured) {
                effect { hardware.approve(checkNotNull(original.preparation)) }
                original.w2Captured = true // Purpose2 capture is not a terminal/publication grant.
            }
            if (!original.reservationProved) {
                effect { invoke(5) }; original.reservationProved = true
            }
            if (original.reserve == null) original.reserve = effect { Dispatch(6, invoke(6)) }
            dispatch(checkNotNull(original.reserve))
            val reserveKey = checkNotNull(original.reserve).fields[4]
            if (original.terminal == null) {
                effect { invoke(16) } // Fresh genuine four-node clock before selecting W1.
                currentControl.refreshCurrentFinancialControl()
                original.terminal = effect { binding.selectTerminal(KagemushaOrdinaryRuntimeJniV1, reserveKey) }
            }
            if (!original.w1Captured) {
                effect { hardware.approveOrdinaryTerminal(checkNotNull(original.terminal)) }
                original.w1Captured = true
            }
            if (!original.commitProved) { effect { invoke(12) }; original.commitProved = true }
            if (original.commit == null) original.commit = effect { Dispatch(13, invoke(13)) }
            dispatch(checkNotNull(original.commit))
            val commitKey = checkNotNull(original.commit).fields[4]
            if (!original.stateAdvanced) {
                effect { invoke(14, listOf(commitKey)) }; original.stateAdvanced = true
            }
            if (!original.acknowledged) {
                // A separately fresh actual FI/current read gates post-State-fsync acknowledgment.
                // Its HTTP uncertainty retains its own originals. No receipt or callback is invented.
                effect { invoke(16) }
                currentControl.refreshCurrentFinancialControl()
                effect { invoke(15, listOf(commitKey)) }; original.acknowledged = true
            }
            val delivery = effect { invoke(17, listOf(commitKey)).single() }
            val completed = KagemushaOrdinaryCommittedOutgoingOriginalV1(commitKey, delivery)
            current(); original.completed = completed
            return completed
        } finally { business.fill(0); active.set(false) }
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
        effect {
            current(); check(!original.intakeStarted)
            val originals = KagemushaOrdinaryLineageHttpCodecV1.responseOriginals(checkNotNull(original.response))
            original.intakeStarted = true // Own before any global acknowledgement may be fsynced.
            val key = invoke(7, originals).single()
            check(MessageDigest.isEqual(key, original.fields[4])) { "The acknowledged lineage request differs" }
            current(); original.completed = true; original.response = null
        }
    }
    private fun invoke(phase: Int, fields: List<ByteArray> = emptyList()): List<ByteArray> {
        current(); val response = binding.invoke(KagemushaOrdinaryRuntimeJniV1, phase, fields)
        current(); return response
    }
    private fun current() { check(!frozen); requireOriginalOwner(); binding.requireOpen(); requireOriginalOwner() }
    private fun <T> effect(body: () -> T): T = try { current(); body().also { current() } } catch (failure: Throwable) { freeze(failure) }
    private fun freeze(failure: Throwable): Nothing {
        frozen = true
        try { binding.revoke() } catch (_: Throwable) { }
        throw failure
    }
}
