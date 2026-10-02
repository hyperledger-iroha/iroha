// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.requireOriginalP256DerV1

/** Only the original final wallet JNI class can receive the private descriptor. */
fun interface KagemushaOrdinaryNativeOutgoingEndpointV1 {
    fun outgoing(phase: Int, coreHandle: Long, originals: Array<ByteArray>): Array<ByteArray>?
}

/** Same descriptor lock and close as ordinary enrollment/current control. No public handle. */
class KagemushaOrdinaryOutgoingTransportBindingV1 internal constructor(private val bridge: KagemushaCoreCoordinatorBridgeV1) {
    fun invoke(endpoint: KagemushaOrdinaryNativeOutgoingEndpointV1, phase: Int,
        originals: List<ByteArray> = emptyList()): List<ByteArray> = bridge.invokeOrdinaryOutgoing(endpoint, phase, originals)
    fun requireOpen() = bridge.requireOrdinaryDescriptorOpen()
    fun revoke() = bridge.close()
    fun selectTerminal(endpoint: KagemushaOrdinaryNativeOutgoingEndpointV1, reserveOriginalSha256: ByteArray): KagemushaNativePreparedOrdinaryTerminalApprovalV1 =
        KagemushaNativePreparedOrdinaryTerminalApprovalV1.selected(this, endpoint, invoke(endpoint, 8, listOf(reserveOriginalSha256)))
}

/** Distinct opaque Native W1 capability. It has no public constructor, purpose choice or cancel/reset. */
class KagemushaNativePreparedOrdinaryTerminalApprovalV1 private constructor(
    private val binding: KagemushaOrdinaryOutgoingTransportBindingV1,
    private val endpoint: KagemushaOrdinaryNativeOutgoingEndpointV1,
    fields: List<ByteArray>,
) {
    private val original = fields.map(ByteArray::copyOf)
    private var unusable = false
    init { KagemushaOrdinaryOutgoingFrameV1.requireTerminal(original); recheck() }
    @Synchronized fun signingBytes(): ByteArray { recheck(); return original[1].copyOf() }
    @Synchronized fun originalSelection(): ByteArray { recheck(); return original[9].copyOf() }
    @Synchronized fun recoverOriginalApproval(): ByteArray? {
        recheck()
        val held = call(11)
        if (held[0][0] == 0.toByte()) return null
        requireOriginalP256DerV1(held[1]); recheck()
        return held[2].copyOf() // Native has authenticated and retained the original evidence.
    }
    @Synchronized internal fun performPlatformSigning(callback: AndroidAppSigningCallbackV1): ByteArray {
        recheck()
        check(original[2].contentEquals(byteArrayOf(5))) { "Android terminal approval needs its original Android hardware key" }
        val policy = when (original[11].single().toInt()) {
            1 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
            2 -> KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY
            else -> error("Unsupported Native terminal hardware policy")
        }
        var raw: ByteArray? = null
        return try {
            val fenced = call(9) // Main fsync occurs before the sole OS invocation.
            if (fenced[0][0] == 0.toByte()) {
                recheck()
                raw = callback(original[3].toString(Charsets.UTF_8), original[4].copyOf(), original[5].copyOf(),
                    original[6].copyOf(), original[1].copyOf(), policy, ::recheck).copyOf()
                requireOriginalP256DerV1(checkNotNull(raw)); recheck()
                same(call(10, listOf(checkNotNull(raw))).single(), sha(checkNotNull(raw)))
            } else {
                // A complete retained original alone permits recovery. Native refuses a fenced
                // unknown outcome before returning this status; no platform callback runs here.
                raw = fenced[1].copyOf(); requireOriginalP256DerV1(checkNotNull(raw))
                if (fenced[0][0] == 1.toByte()) same(call(10, listOf(checkNotNull(raw))).single(), sha(checkNotNull(raw)))
            }
            recheck()
            val completed = call(11)
            require(completed[0].contentEquals(byteArrayOf(2)))
            same(completed[1], checkNotNull(raw)); recheck(); completed[2].copyOf()
        } catch (failure: Throwable) { freeze(failure) }
        finally { raw?.fill(0) }
    }
    private fun recheck() {
        check(!unusable) { "Native terminal outcome is unknown; original recovery is required" }
        try {
            binding.requireOpen()
            val checked = binding.invoke(endpoint, 18)
            KagemushaOrdinaryOutgoingFrameV1.requireTerminal(checked)
            original.indices.forEach { same(original[it], checked[it]) }
            binding.requireOpen()
        } catch (failure: Throwable) { freeze(failure) }
    }
    private fun call(phase: Int, fields: List<ByteArray> = emptyList()): List<ByteArray> {
        check(!unusable)
        return try { binding.invoke(endpoint, phase, fields) } catch (failure: Throwable) { freeze(failure) }
    }
    private fun same(a: ByteArray, b: ByteArray) = check(MessageDigest.isEqual(a, b)) { "Native terminal original changed" }
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
    private fun freeze(failure: Throwable): Nothing {
        unusable = true
        try { binding.revoke() } catch (_: Throwable) { }
        throw failure
    }
    internal companion object {
        fun selected(binding: KagemushaOrdinaryOutgoingTransportBindingV1, endpoint: KagemushaOrdinaryNativeOutgoingEndpointV1,
            fields: List<ByteArray>) = KagemushaNativePreparedOrdinaryTerminalApprovalV1(binding, endpoint, fields)
    }
}
