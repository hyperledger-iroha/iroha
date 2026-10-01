// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyEvidenceV1
import org.hyperledger.iroha.sdk.crypto.keystore.attestation.KagemushaAndroidKeyAttestationArchiveV1

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

/** Opaque native C preparation. The caller cannot supply a C, key alias, policy or issuer verdict. */
class KagemushaNativePreparedOrdinaryAppIdentityV1 private constructor(
    private val state: NativeOrdinaryAppIdentityStateV1,
) {
    /** Guarded original C signing transcript; these bytes do not recreate the native capability. */
    fun originalChallengeSigningBytes(): ByteArray = state.challenge()
    /** Guarded original signed 515-byte preparation for exact HTTP transport, without admission. */
    fun originalSignedPreparationBytes(): ByteArray = state.preparation()
    /** Read the original retained admission without any generation or platform-attestation call. */
    fun recoverOriginalAdmission(): KagemushaNativeRawAppIdentityAdmissionV1? = state.recover()
    /** Only native Core may cancel a never-invoked identity attempt. */
    fun cancel() = state.cancel()
    internal fun performAndroidEnrollment(callback: AndroidOrdinaryIdentityCallbackV1): KagemushaNativeRawAppIdentityAdmissionV1 =
        state.enroll(callback)
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

    @Synchronized fun challenge(): ByteArray { recheck(); return original[2].copyOf() }
    @Synchronized fun preparation(): ByteArray { recheck(); return original[1].copyOf() }
    @Synchronized fun cancel() { recheck(); invoke(9); unusable = true }

    @Synchronized fun recover(): KagemushaNativeRawAppIdentityAdmissionV1? {
        recheck()
        val recovered = invoke(7)
        return if (stage(recovered) >= 4) admitRetained(recovered) else null
    }

    @Synchronized fun enroll(callback: AndroidOrdinaryIdentityCallbackV1): KagemushaNativeRawAppIdentityAdmissionV1 {
        recheck()
        check(original[4].contentEquals(byteArrayOf(5))) { "Android generation requires the original native Android C" }
        val policy = when (original[6].single().toInt()) {
            1 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
            2 -> KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY
            3 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX
            else -> error("Unsupported native Android identity policy")
        }
        val recovered = invoke(7)
        if (stage(recovered) >= 4) return admitRetained(recovered)
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
            val raw = KagemushaAndroidKeyAttestationArchiveV1.encodeOriginal(evidence.certificateChainDer()).transportBytes()
            if (stage(recovered) < 2) invoke(3, original[5])
            if (stage(recovered) < 3) {
                val fence = invoke(4)
                if (fence[0][0] == 2.toByte()) {
                    same(fence[1], point); same(fence[2], sha(raw))
                    check(number(fence[3]) == raw.size)
                    return admitRetained(invoke(7))
                }
            }
            val split = minOf(raw.size, KagemushaOrdinaryAppIdentityFrameV1.CHUNK_BYTES)
            val retained = invoke(5, point, raw.copyOfRange(0, split), raw.copyOfRange(split, raw.size))
            same(retained[0], sha(raw)); same(retained[1], evidence.attestedKeyId())
            return admitRetained(invoke(7))
        } catch (failure: Throwable) {
            // A failed callback cannot prove non-invocation. Fresh native reopening may recover
            // only the existing original; this process-local capability cannot retry hardware.
            unusable = true
            throw failure
        }
    }

    private fun admitRetained(recovery: List<ByteArray>): KagemushaNativeRawAppIdentityAdmissionV1 {
        check(stage(recovery) >= 4)
        check(original[4].contentEquals(byteArrayOf(5))) { "Use the native Apple platform adapter for an Apple original" }
        same(recovery[1], original[5])
        val raw = readOriginalRaw(recovery[3], number(recovery[4]))
        // Reconstruct only the exact retained original KMCA envelope. The independently selected
        // issuer still owns full certificate/root/revocation/app policy admission.
        KagemushaAndroidKeyAttestationArchiveV1.parseOriginal(raw)
        val accepted = invoke(6)
        val completed = invoke(7)
        check(stage(completed) == 5)
        same(completed[1], recovery[1]); same(completed[2], recovery[2])
        same(completed[3], recovery[3]); same(completed[4], recovery[4])
        same(accepted[0], completed[6]); same(accepted[1], sha(completed[5]))
        recheck()
        return KagemushaNativeRawAppIdentityAdmissionV1(completed[1].toString(Charsets.UTF_8),
            completed[2], raw, completed[5], completed[6])
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

    private fun recheck() {
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
