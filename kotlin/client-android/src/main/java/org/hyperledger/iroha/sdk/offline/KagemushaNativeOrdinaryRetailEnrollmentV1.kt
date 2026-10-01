// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

/** Product wallet signer, invoked only after Native retains the exact FI challenge and invocation.
 * Sign this original raw32 message with the separately protected wallet Ed key, without prehashing.
 * Native independently verifies the original signature before it can reach the FI transport.
 */
fun interface KagemushaOrdinaryWalletAccountSignerV1 {
    fun signOriginal(rawSigningMessage: ByteArray): ByteArray
}

/** A wallet invocation was retained without a complete original; explicit Native recovery is required. */
class KagemushaNativeRetailSigningUnknownOutcomeExceptionV1(cause: Throwable? = null) : IllegalStateException(
    "The original wallet signing outcome is uncertain; Native recovery is required", cause)

/** Private Native-selected wallet/FI ceremony. Its completion is enrollment data, never money permission. */
class KagemushaNativeOrdinaryRetailEnrollmentV1 private constructor(private val state: NativeRetailEnrollmentStateV1) {
    fun accountSigningBytes(): ByteArray = state.signingBytes()
    /** Reads the exact retained wallet Ed64 without invoking a wallet signer again. */
    fun recoverOriginalAccountSignature(): ByteArray? = state.recoverSignature()
    fun signOriginalAccount(signer: KagemushaOrdinaryWalletAccountSignerV1): ByteArray = state.sign(signer)
    fun finishRequestOriginal(): KagemushaOrdinaryIdentityHttpOriginalV1 = state.finishRequest()
    /** A protected reply remains untrusted until same-ticket Native phase12 admits the full FI original. */
    suspend fun completeOriginalEnrollment(transport: KagemushaOrdinaryIdentityOriginalTransportV1): ByteArray {
        val request = finishRequestOriginal()
        request.requireCurrent()
        val raw = transport.exchange(request)
        request.requireCurrent()
        return state.acceptReply(request, raw)
    }
    /** Detached original certificate data from the retained Native completed ceremony. */
    fun originalRetailCertificate(): ByteArray = state.certificate()
    fun cancel() = state.cancel()
    internal fun completedOriginalBindingFor(bridge: KagemushaCoreCoordinatorBridgeV1): KagemushaNativeCompletedRetailBindingV1 =
        state.completedBinding(bridge)

    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, attempt: ByteArray, fields: List<ByteArray>,
            pendingScope: ByteArray, credentialDigest: ByteArray, guard: () -> Unit) =
            KagemushaNativeOrdinaryRetailEnrollmentV1(NativeRetailEnrollmentStateV1(
                bridge, attempt, fields, pendingScope, credentialDigest, guard))
    }
}

/** Private same-coordinator FI completion; detached originals alone cannot recreate it. */
internal class KagemushaNativeCompletedRetailBindingV1 internal constructor(credential: ByteArray, certificate: ByteArray,
    private val requireOriginal: () -> Unit) {
    private val digest = credential.copyOf()
    private val originalCertificate = certificate.copyOf()
    fun credentialDigest(): ByteArray { requireOriginal(); return digest.copyOf() }
    fun originalCertificate(): ByteArray { requireOriginal(); return originalCertificate.copyOf() }
    fun recheck() = requireOriginal()
}

private class NativeRetailEnrollmentStateV1(private val bridge: KagemushaCoreCoordinatorBridgeV1,
    attempt: ByteArray, fields: List<ByteArray>, pendingScope: ByteArray, credentialDigest: ByteArray,
    private val guard: () -> Unit) {
    private val id = attempt.copyOf()
    private val original = fields.map(ByteArray::copyOf)
    private val ticket = original[0].copyOf()
    private val message = original[2].copyOf()
    private val scope = pendingScope.copyOf()
    private val credential = credentialDigest.copyOf()
    private var signature: ByteArray? = null
    private var certificateOriginal: ByteArray? = null
    private var unusable = false

    init { same(original[3], scope); same(original[4], credential); current() }

    @Synchronized fun completedBinding(other: KagemushaCoreCoordinatorBridgeV1): KagemushaNativeCompletedRetailBindingV1 {
        check(bridge === other) { "Bootstrap requires the same Native FI coordinator" }
        val retained = certificate()
        return KagemushaNativeCompletedRetailBindingV1(credential, retained) {
            same(certificate(), retained)
        }
    }

    @Synchronized fun signingBytes(): ByteArray {
        current(); return message.copyOf().also { current() }
    }
    @Synchronized fun recoverSignature(): ByteArray? {
        val recovered = current()
        if (recovered[0][0] == 1.toByte()) throw KagemushaNativeRetailSigningUnknownOutcomeExceptionV1()
        return recovered[1].takeIf { it.isNotEmpty() }?.copyOf().also { current() }
    }
    @Synchronized fun sign(signer: KagemushaOrdinaryWalletAccountSignerV1): ByteArray {
        val prior = current()
        if (prior[0][0] == 1.toByte()) throw KagemushaNativeRetailSigningUnknownOutcomeExceptionV1()
        val fenced = invoke(10)
        if (fenced[0][0] == 2.toByte()) {
            same(fenced[1], checkNotNull(current()[1].takeIf { it.size == 64 }))
            return fenced[1].copyOf().also { current() }
        }
        var raw: ByteArray? = null
        try {
            current()
            raw = signer.signOriginal(message.copyOf()).copyOf()
            require(raw.size == 64) { "Wallet must return the original Ed64 signature" }
            current()
            same(invoke(11, raw).single(), sha(raw))
            same(current()[1], raw)
            return raw.copyOf().also { current() }
        } catch (failure: Throwable) {
            unusable = true
            throw KagemushaNativeRetailSigningUnknownOutcomeExceptionV1(failure)
        } finally { raw?.fill(0) }
    }
    @Synchronized fun finishRequest(): KagemushaOrdinaryIdentityHttpOriginalV1 {
        val retained = current()
        check(retained[0][0].toInt() in 2..3) { "Native has not retained the original wallet signature" }
        val raw = retained[1].copyOf()
        val body = KagemushaOrdinaryIdentityHttpCodecV1.retailFinishBody(id, raw)
        return KagemushaOrdinaryIdentityHttpOriginalV1.retail(id, "finish", body) { requireSignature(raw) }
    }
    @Synchronized private fun requireSignature(raw: ByteArray) {
        val retained = current()
        check(retained[0][0].toInt() in 2..3)
        same(retained[1], raw)
        current()
    }
    @Synchronized fun acceptReply(request: KagemushaOrdinaryIdentityHttpOriginalV1, raw: ByteArray): ByteArray {
        request.requireCurrent()
        val reply = KagemushaOrdinaryIdentityHttpCodecV1.retailFinishResponse(raw, id)
        request.requireCurrent()
        val admitted = invoke(12, reply[0])
        same(admitted[1], scope); same(admitted[0], reply[1])
        same(current()[2], reply[0])
        request.requireCurrent()
        return admitted[0].copyOf()
    }
    @Synchronized fun certificate(): ByteArray {
        val retained = current(); check(retained[0][0] == 3.toByte())
        return retained[2].copyOf().also { current() }
    }
    @Synchronized fun cancel() {
        val retained = current(); check(retained[0][0] == 0.toByte()) { "An invoked wallet ceremony cannot be cancelled" }
        invoke(14); unusable = true
    }
    private fun current(): List<ByteArray> {
        check(!unusable) { "The original Native wallet ceremony is unavailable" }
        guard()
        val retained = invoke(13)
        signature?.let { same(it, retained[1]) }
        certificateOriginal?.let { same(it, retained[2]) }
        if (retained[1].isNotEmpty()) signature = retained[1].copyOf()
        if (retained[2].isNotEmpty()) certificateOriginal = retained[2].copyOf()
        guard()
        return retained
    }
    private fun invoke(phase: Int, raw: ByteArray? = null): List<ByteArray> {
        check(!unusable)
        val request = arrayListOf(KagemushaCoreCoordinatorFrameV1.u32(phase), ticket.copyOf())
        raw?.let { request.add(it.copyOf()) }
        return try { bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION, request) }
        catch (failure: Throwable) { unusable = true; throw failure }
    }
    private fun same(a: ByteArray, b: ByteArray) {
        if (!MessageDigest.isEqual(a, b)) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Preserve the original correlation failure. */ }
            error("Native wallet ceremony substituted a retained original")
        }
    }
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
}
