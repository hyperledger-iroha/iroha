// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyEvidenceV1

/** Read-only exact native-retained platform evidence, before issuer admission. */
class KagemushaNativeCollectedAppIdentityOriginalV1 internal constructor(
    reference: String, point: ByteArray, raw: ByteArray,
) {
    private val keyReference = reference
    private val originalPoint = point.copyOf()
    private val originalRaw = raw.copyOf()
    fun originalKeyReference(): String = keyReference
    fun publicKeySec1(): ByteArray = originalPoint.copyOf()
    fun originalAttestationBytes(): ByteArray = originalRaw.copyOf()
}

/** Retained native raw-identity admission. This is neither a final credential nor StateGuard authority. */
class KagemushaNativeRawAppIdentityAdmissionV1 internal constructor(
    reference: String, point: ByteArray, raw: ByteArray, signedAdmission: ByteArray, pendingScope: ByteArray,
) {
    private val keyReference = reference
    private val originalPoint = point.copyOf()
    private val originalRaw = raw.copyOf()
    private val originalAdmission = signedAdmission.copyOf()
    private val scope = pendingScope.copyOf()
    fun originalKeyReference(): String = keyReference
    fun publicKeySec1(): ByteArray = originalPoint.copyOf()
    fun originalAttestationBytes(): ByteArray = originalRaw.copyOf()
    fun signedRawAdmissionTransport(): ByteArray = originalAdmission.copyOf()
    fun pendingNativeScopeDigest(): ByteArray = scope.copyOf()
}

/** Opaque preparation produced only by native authentication of the held original reservation. */
class KagemushaNativePreparedOrdinaryAppIdentityV1 private constructor(
    private val state: NativeOrdinaryAppIdentityStateV1,
) {
    /** Guarded original C signing transcript; these bytes do not recreate the native capability. */
    fun originalChallengeSigningBytes(): ByteArray = state.challenge()
    /** Guarded original signed 515-byte preparation for exact HTTP transport, without admission. */
    fun originalSignedPreparationBytes(): ByteArray = state.preparation()
    /** Read exact collected evidence without generating a key or requesting issuer admission. */
    fun recoverOriginalAttestation(): KagemushaNativeCollectedAppIdentityOriginalV1? = state.recoverRaw()
    /** Read an already admitted original; an unadmitted raw original returns null. */
    fun recoverOriginalAdmission(): KagemushaNativeRawAppIdentityAdmissionV1? = state.recover()
    /** Explicitly admit the exact issuer original on this same native-retained raw attempt. */
    fun acceptOriginalRawAdmission(original: ByteArray): KagemushaNativeRawAppIdentityAdmissionV1 = state.accept(original)
    /** Exact public raw request from retained platform originals; no caller key or attestation body. */
    fun rawAttestationRequestOriginal(): KagemushaOrdinaryIdentityHttpOriginalV1 = state.request()
    internal fun certificateOriginalsFor(bridge: KagemushaCoreCoordinatorBridgeV1): List<ByteArray> =
        state.certificateOriginals(bridge)
    /** Protected public exchange followed by explicit raw314 native intake; existing admission is reused. */
    suspend fun admitOriginalAttestation(transport: KagemushaOrdinaryIdentityOriginalTransportV1): KagemushaNativeRawAppIdentityAdmissionV1 {
        recoverOriginalAdmission()?.let { return it }
        val request = rawAttestationRequestOriginal()
        request.requireCurrent()
        val response = transport.exchange(request)
        request.requireCurrent()
        val offered = KagemushaOrdinaryIdentityHttpCodecV1.rawAdmissionResponse(response)
        request.requireCurrent()
        return acceptOriginalRawAdmission(offered)
    }
    /** Only native Core may cancel a never-invoked identity attempt. */
    fun cancel() = state.cancel()
    internal fun performAndroidCollection(callback: AndroidOrdinaryIdentityCallbackV1): KagemushaNativeCollectedAppIdentityOriginalV1 =
        state.collect(callback)
    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, fields: List<ByteArray>) =
            KagemushaNativePreparedOrdinaryAppIdentityV1(NativeOrdinaryAppIdentityStateV1(bridge, fields))
    }
}

internal typealias AndroidOrdinaryIdentityCallbackV1 = (
    String, ByteArray, KagemushaAndroidAppKeyHardwarePolicyV1, Boolean, () -> Unit,
) -> KagemushaAndroidHardwareAppKeyEvidenceV1

private class NativeOrdinaryAppIdentityStateV1(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
    fields: List<ByteArray>,
) {
    private val original = fields.map(ByteArray::copyOf)
    private val ticket = original[0].copyOf()
    private val scope = original[7].copyOf()
    private val challengeDigest = original[3].copyOf()
    private var unusable = false

    @Synchronized fun request(): KagemushaOrdinaryIdentityHttpOriginalV1 {
        recheck()
        val recovery = invoke(7)
        check(stage(recovery) >= 4) { "Collect the native raw original before its HTTP request" }
        val collected = collectedRetained(recovery)
        return KagemushaOrdinaryIdentityHttpOriginalV1.rawAttestation(original[1], collected.publicKeySec1(),
            collected.originalAttestationBytes(), ::recheck)
    }

    @Synchronized fun challenge(): ByteArray { recheck(); return original[2].copyOf() }
    @Synchronized fun preparation(): ByteArray { recheck(); return original[1].copyOf() }
    @Synchronized fun certificateOriginals(other: KagemushaCoreCoordinatorBridgeV1): List<ByteArray> {
        check(bridge === other) { "Certificate originals belong to another native coordinator" }
        val admitted = checkNotNull(recover()) { "Certificate requires the same native-admitted raw original" }
        recheck()
        return listOf(original[1].copyOf(), admitted.publicKeySec1(), admitted.originalAttestationBytes(),
            admitted.pendingNativeScopeDigest(), admitted.originalKeyReference().toByteArray(Charsets.UTF_8))
    }
    @Synchronized fun cancel() { recheck(); invoke(9); unusable = true }

    @Synchronized fun recover(): KagemushaNativeRawAppIdentityAdmissionV1? {
        recheck()
        val recovered = invoke(7)
        return if (stage(recovered) == 5) admittedRetained(recovered) else null
    }

    @Synchronized fun recoverRaw(): KagemushaNativeCollectedAppIdentityOriginalV1? {
        recheck()
        val recovered = invoke(7)
        return if (stage(recovered) >= 4) collectedRetained(recovered) else null
    }

    @Synchronized fun accept(bytes: ByteArray): KagemushaNativeRawAppIdentityAdmissionV1 {
        val offered = bytes.copyOf()
        require(offered.size == 314)
        recheck()
        val retained = invoke(7)
        check(stage(retained) >= 4) { "Collect the original raw evidence before admission" }
        if (stage(retained) == 5) same(retained[5], offered)
        // Read and correlate the complete retained evidence before crossing the explicit intake.
        collectedRetained(retained)
        try {
            val accepted = invoke(6, offered)
            val completed = invoke(7)
            check(stage(completed) == 5)
            for (index in 1..4) same(completed[index], retained[index])
            same(completed[5], offered)
            same(accepted[0], completed[6]); same(accepted[1], sha(offered))
            return admittedRetained(completed)
        } catch (failure: Throwable) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Preserve the uncertain original result. */ }
            throw failure
        }
    }

    @Synchronized fun collect(callback: AndroidOrdinaryIdentityCallbackV1): KagemushaNativeCollectedAppIdentityOriginalV1 {
        recheck()
        check(original[4].contentEquals(byteArrayOf(5))) { "Android generation requires the original native Android C" }
        val policy = when (original[6].single().toInt()) {
            1 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
            2 -> KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY
            3 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX
            else -> error("Unsupported native Android identity policy")
        }
        val recovered = invoke(7)
        if (stage(recovered) >= 4) return collectedRetained(recovered)
        val alias = original[5].toString(Charsets.UTF_8)
        if (stage(recovered) >= 2) same(recovered[1], original[5])
        var recoverOnly = stage(recovered) != 0
        if (!recoverOnly) {
            val fenced = invoke(2)
            recoverOnly = fenced[0][0] == 2.toByte()
            if (recoverOnly) same(fenced[1], original[5])
        }
        try {
            // A generation fence precedes the platform call. Unknown generation/attestation
            // states may only READ the exact existing Android alias and generation-time chain.
            // No generate-if-missing or fresh attestation path is available during recovery.
            recheck()
            val evidence = callback(alias, challengeDigest.copyOf(), policy, recoverOnly, ::recheck)
            recheck()
            val point = evidence.publicKeySec1()
            same(evidence.attestedKeyId(), sha(point))
            val raw = evidence.platformAttestationOriginal()
            if (stage(recovered) < 2) invoke(3, original[5])
            if (stage(recovered) < 3) {
                val fence = invoke(4)
                if (fence[0][0] == 2.toByte()) {
                    same(fence[1], point); same(fence[2], sha(raw))
                    check(number(fence[3]) == raw.size)
                    return collectedRetained(invoke(7))
                }
            }
            val split = minOf(raw.size, KagemushaOrdinaryAppIdentityFrameV1.CHUNK_BYTES)
            val retained = invoke(5, point, raw.copyOfRange(0, split), raw.copyOfRange(split, raw.size))
            same(retained[0], sha(raw)); same(retained[1], evidence.attestedKeyId())
            return collectedRetained(invoke(7))
        } catch (failure: Throwable) {
            // A failed callback cannot prove non-invocation. Fresh native reopening may recover
            // only the existing original; this process-local capability cannot retry hardware.
            unusable = true
            throw failure
        }
    }

    private fun collectedRetained(recovery: List<ByteArray>): KagemushaNativeCollectedAppIdentityOriginalV1 {
        check(stage(recovery) >= 4)
        check(original[4].contentEquals(byteArrayOf(5))) { "Use the native Apple platform adapter for an Apple original" }
        same(recovery[1], original[5])
        val raw = readOriginalRaw(recovery[3], number(recovery[4]))
        // Shape validation grants no certificate/root/revocation/app policy verdict.
        checkNotNull(KagemushaPlatformAttestationOriginalV1.decodeCanonicalExact(raw).androidCertificateChainDer()) {
            "Original platform container is not Android"
        }
        recheck()
        return KagemushaNativeCollectedAppIdentityOriginalV1(recovery[1].toString(Charsets.UTF_8), recovery[2], raw)
    }

    private fun admittedRetained(recovery: List<ByteArray>): KagemushaNativeRawAppIdentityAdmissionV1 {
        check(stage(recovery) == 5)
        val collected = collectedRetained(recovery)
        // Recovery performs no issuer request or intake dispatch. These originals have already
        // been admitted and retained by the native attempt before its stage5 response.
        recheck()
        return KagemushaNativeRawAppIdentityAdmissionV1(collected.originalKeyReference(),
            collected.publicKeySec1(), collected.originalAttestationBytes(), recovery[5], recovery[6])
    }

    private fun readOriginalRaw(expectedDigest: ByteArray, total: Int): ByteArray {
        check(total in 1..KagemushaOrdinaryAppIdentityFrameV1.MAXIMUM_RAW_BYTES)
        val result = ByteArray(total)
        for (index in 0 until (total + KagemushaOrdinaryAppIdentityFrameV1.CHUNK_BYTES - 1) / KagemushaOrdinaryAppIdentityFrameV1.CHUNK_BYTES) {
            recheck()
            val chunk = invoke(10, KagemushaCoreCoordinatorFrameV1.u32(index))
            same(chunk[2], expectedDigest); check(number(chunk[3]) == total)
            chunk[1].copyInto(result, index * KagemushaOrdinaryAppIdentityFrameV1.CHUNK_BYTES)
        }
        same(sha(result), expectedDigest); recheck(); return result
    }

    @Synchronized private fun recheck() {
        check(!unusable) { "Original identity outcome is uncertain; native original recovery is required" }
        val checked = invoke(8)
        same(checked[0], scope); same(checked[1], challengeDigest)
    }
    private fun invoke(phase: Int, vararg extra: ByteArray): List<ByteArray> {
        check(!unusable)
        val request = arrayListOf(KagemushaCoreCoordinatorFrameV1.u32(phase), ticket.copyOf())
        extra.forEach { request.add(it.copyOf()) }
        return try { bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY, request) }
        catch (failure: Throwable) { unusable = true; throw failure }
    }
    private fun same(left: ByteArray, right: ByteArray) {
        if (!MessageDigest.isEqual(left, right)) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Preserve the original substitution failure. */ }
            error("Native ordinary identity response substituted an original")
        }
    }
    private fun stage(fields: List<ByteArray>) = fields[0].single().toInt()
    private fun number(bytes: ByteArray) = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).int
    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
}
