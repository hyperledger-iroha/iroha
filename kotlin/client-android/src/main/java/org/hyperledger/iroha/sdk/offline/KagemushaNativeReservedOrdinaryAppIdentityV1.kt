// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

/**
 * Process-local native preparation reservation. Its public carrier contains no financial secret.
 * Only exact response intake on this same holder can produce a prepared identity capability.
 */
class KagemushaNativeReservedOrdinaryAppIdentityV1 private constructor(
    private val state: NativeOrdinaryAppReservationStateV1,
) {
    fun accountId(): String = state.field(1).toString(Charsets.UTF_8)
    fun clientNonce(): ByteArray = state.field(2)
    fun releaseId(): ByteArray = state.field(3)
    fun hardwareProfileId(): ByteArray = state.field(4)
    fun laneId(): ByteArray = state.field(5)
    fun financialAuthorityCommitment(): ByteArray = state.field(6)
    fun requestId(): String = state.field(7).toString(Charsets.UTF_8)

    /** Original native-selected policy bytes, or empty when absent; this projection grants no verdict. */
    fun originalPlayIntegrityPolicyBytes(): ByteArray = state.policy()
    internal fun certificatePolicyFor(bridge: KagemushaCoreCoordinatorBridgeV1, signedPreparation: ByteArray): ByteArray =
        state.certificatePolicy(bridge, signedPreparation)

    /** Native Core authenticates this exact 515-byte original against its fsynced reservation. */
    fun acceptOriginalSignedPreparation(original: ByteArray): KagemushaNativePreparedOrdinaryAppIdentityV1 =
        state.accept(original)

    /** Native-selected public request for protected product transport, without new randomness. */
    fun preparationRequestOriginal(): KagemushaOrdinaryIdentityHttpOriginalV1 = state.request()

    /** Exact protected HTTP followed by explicit signed-C intake on this original native holder. */
    suspend fun prepare(transport: KagemushaOrdinaryIdentityOriginalTransportV1): KagemushaNativePreparedOrdinaryAppIdentityV1 {
        val request = preparationRequestOriginal()
        request.requireCurrent()
        val response = transport.exchange(request)
        request.requireCurrent()
        return acceptOriginalSignedPreparation(state.response(response))
    }

    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, fields: List<ByteArray>) =
            KagemushaNativeReservedOrdinaryAppIdentityV1(NativeOrdinaryAppReservationStateV1(bridge, fields))
    }
}

private class NativeOrdinaryAppReservationStateV1(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
    fields: List<ByteArray>,
) {
    private val original = fields.map(ByteArray::copyOf)
    private var preparedOriginal: ByteArray? = null
    private var unusable = false

    @Synchronized fun request(): KagemushaOrdinaryIdentityHttpOriginalV1 {
        recheck()
        return KagemushaOrdinaryIdentityHttpOriginalV1.preparation(original, ::recheck)
    }
    @Synchronized fun response(raw: ByteArray): ByteArray {
        recheck()
        return KagemushaOrdinaryIdentityHttpCodecV1.signedPreparationResponse(raw, original).also { recheck() }
    }

    @Synchronized fun field(index: Int): ByteArray { recheck(); return original[index].copyOf() }

    @Synchronized fun policy(): ByteArray {
        recheck()
        val selected = invoke(listOf(KagemushaCoreCoordinatorFrameV1.u32(14), original[0])).single()
        recheck()
        return selected.copyOf()
    }

    @Synchronized fun certificatePolicy(other: KagemushaCoreCoordinatorBridgeV1, signedPreparation: ByteArray): ByteArray {
        check(bridge === other) { "Certificate policy belongs to another native coordinator" }
        recheck()
        same(checkNotNull(preparedOriginal) { "Certificate requires the originally admitted signed preparation" }, signedPreparation)
        return policy()
    }

    @Synchronized fun accept(bytes: ByteArray): KagemushaNativePreparedOrdinaryAppIdentityV1 {
        val offered = bytes.copyOf()
        recheck()
        preparedOriginal?.let { same(it, offered) }
        val fields = invoke(listOf(KagemushaCoreCoordinatorFrameV1.u32(13), original[0], offered))
        // These are correlation checks only. Native admission also verifies the account binding,
        // issuer signature, selected policies, original native clock and private secret commitment.
        val body = fields[1].copyOfRange(0, 451)
        for ((carrier, selector) in listOf(2 to 1, 3 to 6, 4 to 7, 5 to 5, 6 to 11)) {
            same(original[carrier], body.copyOfRange(3 + selector * 32, 35 + selector * 32))
        }
        same(offered, fields[1])
        recheck()
        preparedOriginal = offered.copyOf()
        return KagemushaNativePreparedOrdinaryAppIdentityV1.fromNative(bridge, fields)
    }

    @Synchronized private fun recheck() {
        check(!unusable) { "Original preparation reservation is unavailable; native recovery is required" }
        val held = invoke(listOf(KagemushaCoreCoordinatorFrameV1.u32(12)))
        original.indices.forEach { same(original[it], held[it]) }
    }

    private fun invoke(fields: List<ByteArray>): List<ByteArray> {
        check(!unusable)
        return try {
            bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY, fields.map(ByteArray::copyOf))
        } catch (failure: Throwable) { unusable = true; throw failure }
    }

    private fun same(left: ByteArray, right: ByteArray) {
        if (!MessageDigest.isEqual(left, right)) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Keep the original correlation failure. */ }
            error("Native preparation substituted the original reservation")
        }
    }
}
