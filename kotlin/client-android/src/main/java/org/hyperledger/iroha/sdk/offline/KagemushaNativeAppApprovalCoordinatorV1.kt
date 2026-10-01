// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityProviderV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityTokenOriginalV1
import org.hyperledger.iroha.sdk.crypto.keystore.requireOriginalP256DerV1

/**
 * Non-monetary app operations on the coordinator's already admitted original native owner.
 * The caller supplies an actual operation/attempt identity, never a signing subject, key or policy.
 * Missing native support remains an error; a compatible frame or receipt is not StateGuard authority.
 */
class KagemushaNativeAppApprovalCoordinatorV1 internal constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) {
    /** Read the installed source's already reserved original ID; a selector grants no enrollment authority. */
    fun originalEnrollmentAttemptId(): ByteArray = bridge.invoke(
        KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY,
        listOf(KagemushaCoreCoordinatorFrameV1.u32(11)),
    ).single().copyOf()

    /** Native Core fsyncs the separate financial secret and client nonce before exposing this carrier. */
    fun reserveOriginalIdentity(): KagemushaNativeReservedOrdinaryAppIdentityV1 {
        val fields = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(12)))
        return KagemushaNativeReservedOrdinaryAppIdentityV1.fromNative(bridge, fields)
    }

    /** Carry the reserved public original to the protected issuer, then authenticate signed C natively. */
    suspend fun prepareOriginalIdentity(transport: KagemushaOrdinaryIdentityOriginalTransportV1): KagemushaNativePreparedOrdinaryAppIdentityV1 =
        reserveOriginalIdentity().prepare(transport)

    /** Derive W and its original key selection from the native checked operation. */
    fun prepareApproval(operationId: ByteArray): KagemushaNativePreparedAppApprovalV1 {
        val id = operationId.copyOf()
        val fields = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(1), id))
        return KagemushaNativePreparedAppApprovalV1.fromNative(bridge, id, fields)
    }

    /** Derive E after raw identity admission, selected only by SHA256(full original C signing bytes). */
    fun prepareEnrollmentPossession(enrollmentOperationId: ByteArray): KagemushaNativePreparedAppEnrollmentPossessionV1 {
        val id = enrollmentOperationId.copyOf()
        val fields = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(1), id))
        return KagemushaNativePreparedAppEnrollmentPossessionV1.fromNative(bridge, id, fields)
    }
}

/**
 * Process-local W capability with no public constructor, clone or serialization path.
 * Native Core owns original selection, freshness, once-only fence, evidence and consumption.
 */
class KagemushaNativePreparedAppApprovalV1 private constructor(private val state: NativeAppPreparedStateV1) {
    /** Read-only exact W; obtaining these bytes does not recreate the native capability. */
    fun signingBytes(): ByteArray = state.signingBytes()
    /** Read-only original S correlated by SHA256(W's subject field), not a new signing subject. */
    fun financialSelectionOriginal(): ByteArray = state.financialSelectionOriginal()
    /** Recover only the original native receipt; null means the native attempt is still uninvoked. */
    fun recoverOriginalApproval(): ByteArray? = state.recover()
    /** Ask native Core to cancel the exact attempt; no local alias deletion or key replacement. */
    fun cancel() = state.cancel()

    internal fun performPlatformSigning(callback: AndroidAppSigningCallbackV1): ByteArray = state.sign(callback)
    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, id: ByteArray, fields: List<ByteArray>) =
            KagemushaNativePreparedAppApprovalV1(NativeAppPreparedStateV1(bridge,
                KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL, id, fields))
    }
}

/** Separate E capability; successful identity possession never grants monetary authorization. */
class KagemushaNativePreparedAppEnrollmentPossessionV1 private constructor(private val state: NativeAppPreparedStateV1) {
    /** Exact native E signing bytes, guarded by its original pending enrollment owner. */
    fun signingBytes(): ByteArray = state.signingBytes()
    /** Recover the same retained possession receipt; never invoke the platform again during recovery. */
    fun recoverOriginalPossession(): ByteArray? = state.recover()
    /** Read the original DER/CBOR only after native consumption, with no hardware or issuer call. */
    fun originalPlatformEvidence(): ByteArray = state.completedOriginalEvidence()
    /** Native authenticates and retains this exact opaque credential; the returned digest is data only. */
    fun acceptOriginalCredential(originalCredential: ByteArray): ByteArray = state.acceptCredential(originalCredential)
    /** Native-paired public originals select the exact certificate carrier; null token requires absent Native PI policy. */
    fun certificateRequestOriginal(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        integrityToken: KagemushaAndroidPlayIntegrityTokenOriginalV1? = null): KagemushaOrdinaryIdentityHttpOriginalV1 =
        state.certificateRequest(reservation, identity, integrityToken)
    /** Request Google's opaque original using only the paired Native policy/project and exact C/key request hash. */
    fun requestOriginalIntegrityToken(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        provider: KagemushaAndroidPlayIntegrityProviderV1): CompletableFuture<KagemushaAndroidPlayIntegrityTokenOriginalV1> =
        state.requestIntegrity(reservation, identity, provider)
    /** Exact protected HTTP response is still untrusted until same-held Native phase8 admits it. */
    suspend fun issueOriginalCredential(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        integrityToken: KagemushaAndroidPlayIntegrityTokenOriginalV1?,
        transport: KagemushaOrdinaryIdentityOriginalTransportV1): ByteArray {
        val request = certificateRequestOriginal(reservation, identity, integrityToken)
        request.requireCurrent()
        val response = transport.exchange(request)
        request.requireCurrent()
        val original = KagemushaOrdinaryIdentityHttpCodecV1.certificateResponse(response)
        request.requireCurrent()
        return acceptOriginalCredential(original)
    }
    /** Cancel only the native pending attempt; existing enrolled key custody remains native policy. */
    fun cancel() = state.cancel()

    internal fun performPlatformSigning(callback: AndroidAppSigningCallbackV1): ByteArray = state.sign(callback)
    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, id: ByteArray, fields: List<ByteArray>) =
            KagemushaNativePreparedAppEnrollmentPossessionV1(NativeAppPreparedStateV1(bridge,
                KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION, id, fields))
    }
}

internal typealias AndroidAppSigningCallbackV1 = (String, ByteArray, ByteArray, ByteArray, ByteArray,
    KagemushaAndroidAppKeyHardwarePolicyV1, () -> Unit) -> ByteArray

private class NativeAppPreparedStateV1(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
    private val method: KagemushaCoreCoordinatorMethodV1,
    id: ByteArray,
    fields: List<ByteArray>,
) {
    private val originalId = id.copyOf()
    private val original = fields.map(ByteArray::copyOf)
    private val ticket = original[0].copyOf()
    private val message = original[1].copyOf()
    private val scope = original[9].copyOf()
    private val messageDigest = sha(message)
    private var unusable = false
    private var certificateBody: ByteArray? = null

    @Synchronized fun signingBytes(): ByteArray { recheck(); return message.copyOf() }
    @Synchronized fun financialSelectionOriginal(): ByteArray { recheck(); return original[13].copyOf() }

    @Synchronized fun sign(callback: AndroidAppSigningCallbackV1): ByteArray {
        recheck()
        check(original[2].contentEquals(byteArrayOf(5))) { "Android signing requires the native Android platform" }
        val policy = when (original[11].single().toInt()) {
            1 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
            2 -> KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY
            3 -> KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX
            else -> error("Unsupported native Android app-key policy")
        }
        val fenced = invoke(2)
        return when (fenced[0].single().toInt()) {
            1 -> {
                // The native durable fence precedes the only platform invocation. A lost result
                // freezes this capability; only a genuinely retained native original can recover.
                var raw: ByteArray? = null
                try {
                    recheck()
                    val signed = callback(original[3].toString(Charsets.UTF_8), original[4].copyOf(),
                        original[5].copyOf(), original[6].copyOf(), message.copyOf(), policy, ::recheck)
                    raw = signed.copyOf()
                    requireOriginalP256DerV1(raw)
                    recheck()
                    val digest = invoke(3, raw).single()
                    same(digest, sha(raw))
                    consume(raw)
                } catch (failure: Throwable) {
                    // Native has fenced even if the platform failed before returning. A local
                    // exception cannot prove non-invocation and cannot authorize a fresh signature.
                    unusable = true
                    throw failure
                } finally {
                    raw?.fill(0)
                }
            }
            2 -> consume(fenced[1])
            3 -> checkedReceipt(fenced[2], fenced[1])
            else -> error("Invalid native app invocation fence")
        }
    }

    @Synchronized fun recover(): ByteArray? {
        recheck()
        val recovered = invoke(5)
        return when (recovered[0].single().toInt()) {
            0 -> null
            1 -> consume(recovered[1])
            2 -> checkedReceipt(recovered[2], recovered[1])
            else -> error("Invalid native app original recovery")
        }
    }

    @Synchronized fun completedOriginalEvidence(): ByteArray {
        check(method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION)
        recheck()
        val recovered = invoke(5)
        check(recovered[0].contentEquals(byteArrayOf(2))) { "Original platform evidence has not been consumed by native Core" }
        checkedReceipt(recovered[2], recovered[1])
        if (original[2][0] == 5.toByte()) {
            try { requireOriginalP256DerV1(recovered[1]) }
            catch (failure: Throwable) {
                unusable = true
                try { bridge.close() } catch (_: Throwable) { /* Preserve the original evidence failure. */ }
                throw failure
            }
        }
        recheck()
        return recovered[1].copyOf()
    }

    @Synchronized fun acceptCredential(originalCredential: ByteArray): ByteArray {
        check(method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION)
        val held = originalCredential.copyOf()
        require(held.size in 1..16 * 1024) { "Original ordinary credential exceeds its native bound" }
        recheck()
        val admitted = invoke(8, held)
        same(admitted[1], scope)
        recheck()
        return admitted[0].copyOf()
    }

    @Synchronized fun certificateRequest(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        token: KagemushaAndroidPlayIntegrityTokenOriginalV1?): KagemushaOrdinaryIdentityHttpOriginalV1 {
        val (selected, policy) = certificateInputs(reservation, identity)
        if (policy.isEmpty()) {
            require(token == null) { "Native policy did not select Play Integrity" }
        } else {
            val originalToken = checkNotNull(token) { "Native policy requires the original Google evidence" }
            require(originalToken.cloudProjectNumber == KagemushaOrdinaryIdentityHttpCodecV1.playIntegrityCloudProjectOriginal(policy)) {
                "Original Google token belongs to another Native-selected project"
            }
            same(originalToken.requestHash(), KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(selected[0])
                .playIntegrityRequestHash(original[6]))
        }
        val body = KagemushaOrdinaryIdentityHttpCodecV1.certificateBody(selected[0], selected[1], selected[2],
            selected[5], token?.opaqueToken())
        certificateBody?.let { same(it, body) }
        certificateBody = body.copyOf()
        return KagemushaOrdinaryIdentityHttpOriginalV1.certificate(originalId, body) {
            recheckCertificateInputs(reservation, identity, selected, policy)
        }
    }

    @Synchronized fun requestIntegrity(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        provider: KagemushaAndroidPlayIntegrityProviderV1): CompletableFuture<KagemushaAndroidPlayIntegrityTokenOriginalV1> {
        val (selected, policy) = certificateInputs(reservation, identity)
        check(policy.isNotEmpty()) { "Native policy did not select a Google request" }
        val project = KagemushaOrdinaryIdentityHttpCodecV1.playIntegrityCloudProjectOriginal(policy)
        val request = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(selected[0]).playIntegrityRequestHash(original[6])
        return provider.requestOriginal(project, request) { recheckCertificateInputs(reservation, identity, selected, policy) }
    }

    private fun certificateInputs(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1): Pair<List<ByteArray>, ByteArray> {
        check(method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION && original[2][0] == 5.toByte())
        recheck()
        val selected = identity.certificateOriginalsFor(bridge)
        val c = KagemushaOrdinaryAppEnrollmentPreparationV1.parseOriginal(selected[0])
        same(c.canonicalSigningBytes(), original[7]); same(c.attestationChallenge(), originalId)
        same(selected[1], original[5]); same(sha(selected[1]), original[6])
        same(selected[3], scope); same(selected[4], original[3])
        val body = message.size - 371
        same(sha(selected[2]), message.copyOfRange(body + 3 + 10 * 32, body + 35 + 10 * 32))
        val policy = reservation.certificatePolicyFor(bridge, selected[0])
        val fields = selected + listOf(completedOriginalEvidence())
        recheck()
        return fields to policy
    }

    @Synchronized private fun recheckCertificateInputs(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1, selected: List<ByteArray>, policy: ByteArray) {
        val (current, currentPolicy) = certificateInputs(reservation, identity)
        selected.indices.forEach { same(selected[it], current[it]) }
        same(policy, currentPolicy)
        recheck()
    }

    @Synchronized fun cancel() { recheck(); invoke(7); unusable = true }

    private fun consume(raw: ByteArray): ByteArray {
        recheck()
        val receipt = invoke(4).single()
        return checkedReceipt(receipt, raw)
    }

    private fun checkedReceipt(receipt: ByteArray, raw: ByteArray): ByteArray {
        try {
        // Frame validation has checked width/magic. Bind every selected original locally too;
        // the actual proof/permission/counter admission remains exclusively in native Core.
        check(receipt.size == 184)
        check(receipt[10].toInt() == if (method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL) 1 else 2)
        same(receipt.copyOfRange(11, 19), ticket)
        same(receipt.copyOfRange(19, 51), originalId)
        same(receipt.copyOfRange(51, 83), scope)
        same(receipt.copyOfRange(83, 115), messageDigest)
        same(receipt.copyOfRange(115, 147), sha(raw))
        same(receipt.copyOfRange(147, 179),
            if (method == KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL) original[8] else scope)
        if (original[2][0] == 5.toByte()) {
            check(receipt[179] == 0.toByte() && receipt.copyOfRange(180, 184).all { it == 0.toByte() })
        } else {
            check(receipt[179] == 1.toByte())
            val counter = ByteBuffer.wrap(receipt, 180, 4).order(ByteOrder.LITTLE_ENDIAN).int.toLong() and 0xffff_ffffL
            val floor = ByteBuffer.wrap(original[10]).order(ByteOrder.LITTLE_ENDIAN).int.toLong() and 0xffff_ffffL
            check(counter > floor) { "Native receipt did not advance the original identity counter" }
        }
        recheck()
        return receipt.copyOf()
        } catch (failure: Throwable) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Preserve the original correlation failure. */ }
            throw failure
        }
    }

    private fun recheck() {
        check(!unusable) { "Original app-signing outcome is uncertain or cancelled; native recovery is required" }
        val checked = invoke(6)
        same(checked[0], scope); same(checked[1], messageDigest)
    }

    private fun invoke(phase: Int, raw: ByteArray? = null): List<ByteArray> {
        check(!unusable) { "Original app-signing capability is unavailable" }
        val request = arrayListOf(KagemushaCoreCoordinatorFrameV1.u32(phase), ticket.copyOf())
        raw?.let { request.add(it.copyOf()) }
        return try { bridge.invoke(method, request) }
        catch (failure: Throwable) { unusable = true; throw failure }
    }
    private fun same(a: ByteArray, b: ByteArray) {
        if (!MessageDigest.isEqual(a, b)) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Original substitution remains the error. */ }
            error("Native receipt substituted original app selection")
        }
    }
    private fun sha(value: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(value)
}
