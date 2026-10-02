// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.requireOriginalP256DerV1

/** Only the exact final SDK wallet JNI owner may receive the actual held descriptor. */
fun interface KagemushaOrdinaryNativeIncomingEndpointV1 {
    fun incoming(phase: Int, coreHandle: Long, originals: Array<ByteArray>): Array<ByteArray>?
}

/** Private source of already Native-admitted enrolled P256 metadata. No public constructor,
 * raw alias selector, hardware policy override or software signing callback is exposed.
 */
internal class KagemushaNativeOrdinaryIncomingKeyOriginalsV1 internal constructor(
    val alias: String, challenge: ByteArray, point: ByteArray,
    val policy: KagemushaAndroidAppKeyHardwarePolicyV1, private val guard: () -> Unit,
) {
    private val generation = challenge.copyOf()
    private val originalPoint = point.copyOf()
    private val originalId = sha(point)
    fun challenge(): ByteArray { guard(); return generation.copyOf() }
    fun point(): ByteArray { guard(); return originalPoint.copyOf() }
    fun keyId(): ByteArray { guard(); return originalId.copyOf() }
    fun recheck() = guard()
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
}

/** Same actual Core descriptor and enrolled original key/FI holders after Bootstrap handoff.
 * Opaque preparations originate only from a genuine Native dispatch; data parsing is separate.
 */
class KagemushaOrdinaryIncomingTransportBindingV1 internal constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
    private val key: KagemushaNativeOrdinaryIncomingKeyOriginalsV1,
    private val retail: KagemushaNativeCompletedRetailBindingV1,
    private val requireOriginalOwner: () -> Unit,
) {
    private var active: KagemushaNativePreparedOrdinaryIncomingApprovalV1? = null
    /** Local descriptor-only gate makes genuine Native renewal reachable after finite read expiry. */
    fun requireOpen() = bridge.requireOrdinaryDescriptorOpen()
    fun revoke() = bridge.close()
    fun requireEnrolledOriginals() { requireOriginalOwner(); key.recheck(); retail.recheck(); requireOriginalOwner() }
    /** Phase16 rechecks actual installed originals/selection, refreshes real same-account S/W,
     * then rechecks the complete Native Source/session. It accepts no clock or read selector.
     */
    fun refreshCurrentAccount(endpoint: KagemushaOrdinaryNativeIncomingEndpointV1) {
        requireOpen()
        check(bridge.invokeOrdinaryIncoming(endpoint,16,emptyList()).isEmpty())
        requireOpen(); requireEnrolledOriginals()
    }
    fun invoke(endpoint: KagemushaOrdinaryNativeIncomingEndpointV1, phase: Int,
        originals: List<ByteArray> = emptyList()): List<ByteArray> {
        require(phase !in listOf(1,8,16,17)) { "Use the dedicated Native preparation/renewal entry" }
        if(phase==5 || phase==12) {
            // Native owns historical proof custody and fsyncs the complete proof result.
            // A long computation is data retention, not a fresh signing/effect grant. Keep
            // installed product/descriptor ownership, then renew actual same S/W before signs.
            requireOriginalOwner();requireOpen()
            return bridge.invokeOrdinaryIncoming(endpoint,phase,originals).also {requireOriginalOwner();requireOpen()}
        }
        requireEnrolledOriginals()
        return bridge.invokeOrdinaryIncoming(endpoint,phase,originals).also { requireEnrolledOriginals() }
    }
    fun prepareFinalizedMint(endpoint: KagemushaOrdinaryNativeIncomingEndpointV1,
        finalized: ByteArray, credit: ByteArray): KagemushaNativePreparedOrdinaryIncomingApprovalV1 =
        prepare(endpoint,1,listOf(finalized,credit),false)
    fun prepareReceive(endpoint: KagemushaOrdinaryNativeIncomingEndpointV1, request: ByteArray,
        outgoing: ByteArray, assertion: ByteArray): KagemushaNativePreparedOrdinaryIncomingApprovalV1 =
        prepare(endpoint,17,listOf(request,outgoing,assertion),false)
    fun selectTerminal(endpoint: KagemushaOrdinaryNativeIncomingEndpointV1,
        reserveRequest: ByteArray): KagemushaNativePreparedOrdinaryIncomingApprovalV1 =
        prepare(endpoint,8,listOf(reserveRequest),true)
    @Synchronized private fun prepare(endpoint: KagemushaOrdinaryNativeIncomingEndpointV1, phase: Int,
        originals: List<ByteArray>, terminal: Boolean): KagemushaNativePreparedOrdinaryIncomingApprovalV1 {
        requireEnrolledOriginals()
        val fields = bridge.invokeOrdinaryIncoming(endpoint,phase,originals)
        requireEnrolledOriginals()
        check(MessageDigest.isEqual(fields[3],retail.originalCertificate())) { "Native incoming FI original changed" }
        val w = fields[1]; val s = fields[2]
        check(MessageDigest.isEqual(w.copyOfRange(181,213),key.keyId()) &&
            MessageDigest.isEqual(w.copyOfRange(213,245),retail.credentialDigest())) {
            "Native incoming credential/app key differs from enrolled original"
        }
        val binding = KagemushaOrdinaryCashApprovalOriginalBindingV1(fields[0], w.copyOfRange(117,149),
            w.copyOfRange(149,181),key.keyId(),retail.credentialDigest(),w.copyOfRange(277,309),s)
        val projection = if (terminal) KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingTerminal(w,s,binding)
            else KagemushaOrdinaryCashApprovalProjectionV1.requireIncomingPreparation(w,s,binding)
        return KagemushaNativePreparedOrdinaryIncomingApprovalV1.fromNative(this,endpoint,key,projection,terminal)
            .also { active = it; requireEnrolledOriginals() }
    }
    internal fun requirePrepared(original: KagemushaNativePreparedOrdinaryIncomingApprovalV1) {
        check(active === original) { "Native incoming approval was replaced" }
        requireEnrolledOriginals()
    }
}

/** Exact W2/W1 capability correlation from the actual JNI owner, plus original enrolled key.
 * No caller data, constructor or external signing callback can manufacture this holder.
 */
class KagemushaNativePreparedOrdinaryIncomingApprovalV1 private constructor(
    private val binding: KagemushaOrdinaryIncomingTransportBindingV1,
    private val endpoint: KagemushaOrdinaryNativeIncomingEndpointV1,
    private val key: KagemushaNativeOrdinaryIncomingKeyOriginalsV1,
    private val projection: KagemushaOrdinaryCashApprovalProjectionV1,
    private val terminal: Boolean,
) {
    private var invoked = false
    private var retainedRaw: ByteArray? = null
    private var completed: ByteArray? = null
    fun operationId(): ByteArray { current(); return projection.operationId() }
    fun signingBytes(): ByteArray { current(); return projection.signingBytes() }
    fun selectionBytes(): ByteArray { current(); return projection.selectionBytes() }
    fun requireCurrent() = current()
    internal companion object {
        fun fromNative(binding: KagemushaOrdinaryIncomingTransportBindingV1,
            endpoint: KagemushaOrdinaryNativeIncomingEndpointV1,key: KagemushaNativeOrdinaryIncomingKeyOriginalsV1,
            projection: KagemushaOrdinaryCashApprovalProjectionV1,terminal: Boolean) =
            KagemushaNativePreparedOrdinaryIncomingApprovalV1(binding,endpoint,key,projection,terminal)
    }
    @Synchronized internal fun performPlatformSigning(sign: (String,ByteArray,ByteArray,ByteArray,ByteArray,
        KagemushaAndroidAppKeyHardwarePolicyV1,Boolean,()->Unit)->ByteArray): ByteArray {
        current()
        val recovery = binding.invoke(endpoint,if (terminal) 11 else 4)
        if (recovery[0][0] == 2.toByte()) return complete(recovery)
        if (retainedRaw == null && recovery[0][0] == 1.toByte()) retainedRaw = recovery[1].copyOf()
        if (retainedRaw == null) {
            check(!invoked) { "The original OS signing outcome is unknown; do not sign again" }
            val fence = binding.invoke(endpoint,if (terminal) 9 else 2)
            if (fence[0][0] == 2.toByte()) return complete(fence)
            if (fence[0][0] == 1.toByte()) retainedRaw = fence[1].copyOf()
            else {
                invoked = true // Native has fsynced its one-use fence before the actual OS call.
                current()
                val raw = sign(key.alias,key.challenge(),key.point(),key.keyId(),projection.signingBytes(),
                    key.policy,terminal,::current)
                requireOriginalP256DerV1(raw)
                retainedRaw = raw.copyOf()
                raw.fill(0)
            }
        }
        val raw = checkNotNull(retainedRaw)
        current()
        binding.invoke(endpoint,if (terminal) 10 else 3,listOf(raw))
        current()
        return complete(binding.invoke(endpoint,if (terminal) 11 else 4))
    }
    private fun complete(fields: List<ByteArray>): ByteArray {
        check(fields[0].contentEquals(byteArrayOf(2))) { "Native did not acknowledge the original approval" }
        retainedRaw?.let { check(MessageDigest.isEqual(it,fields[1])) { "Native substituted platform DER" } }
        completed?.let { check(MessageDigest.isEqual(it,fields[2])) }
        completed = fields[2].copyOf()
        retainedRaw?.fill(0); retainedRaw = null
        current()
        return checkNotNull(completed).copyOf()
    }
    private fun current() { binding.requirePrepared(this) }
}
