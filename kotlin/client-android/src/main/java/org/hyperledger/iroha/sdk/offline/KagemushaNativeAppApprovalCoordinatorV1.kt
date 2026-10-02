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
 * Ordinary preparation and enrollment approvals on the coordinator's admitted Native owner.
 * The caller supplies an actual operation/attempt identity, never a signing subject, key or policy.
 * Missing native support remains an error; a compatible frame or receipt is not StateGuard authority.
 */
class KagemushaNativeAppApprovalCoordinatorV1 internal constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) {
    /** Exact current W/S from this same installed Native account/session owner.
     * Finite correlation originals grant no installation, signing or financial authority.
     */
    fun currentWalletAccountSelection(): KagemushaNativeWalletAccountSelectionOriginalV1 {
        val fields = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(15)))
        return KagemushaNativeWalletAccountSelectionOriginalV1.fromNative(bridge, fields)
    }

    internal fun requireCurrentWalletAccountSelection(original: KagemushaNativeWalletAccountSelectionOriginalV1) =
        original.requireForCoordinator(bridge)

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

    /** Native authenticates this exact receiver request and derives the entire Send preparation. */
    fun prepareOrdinarySendApproval(originalReceiverRequest: ByteArray): KagemushaNativePreparedAppApprovalV1 {
        val id = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(15), KagemushaCoreCoordinatorFrameV1.u32(2),
                originalReceiverRequest.copyOf())).single()
        return prepareApproval(id)
    }

    /** Native subtracts from its actual held State and selects its own enrolled beneficiary. */
    fun prepareOrdinaryRedemptionApproval(amount: java.math.BigInteger): KagemushaNativePreparedAppApprovalV1 {
        require(amount.signum() > 0 && amount.bitLength() <= 128) { "Ordinary amount must fit positive u128" }
        val bigEndian = amount.toByteArray()
        val littleEndian = ByteArray(16) { index ->
            if (index < bigEndian.size) bigEndian[bigEndian.lastIndex - index] else 0
        }
        val id = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(15), KagemushaCoreCoordinatorFrameV1.u32(4), littleEndian)).single()
        return prepareApproval(id)
    }

    /** Separate Bootstrap capability tied to the same retained FI completion and C/key originals. */
    internal fun prepareOrdinaryBootstrapApproval(operationId: ByteArray,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        retail: KagemushaNativeOrdinaryRetailEnrollmentV1): KagemushaNativePreparedOrdinaryBootstrapApprovalV1 {
        identity.certificateOriginalsFor(bridge) // Reject another coordinator before C19 reservation.
        val completed = retail.completedOriginalBindingFor(bridge)
        val c = identity.originalChallengeSigningBytes()
        val admission = checkNotNull(identity.recoverOriginalAdmission())
        val alias = admission.originalKeyReference().toByteArray(Charsets.UTF_8)
        val point = admission.publicKeySec1()
        val credential = completed.credentialDigest()
        fun current() {
            completed.recheck()
            check(MessageDigest.isEqual(identity.originalChallengeSigningBytes(), c))
            val held = checkNotNull(identity.recoverOriginalAdmission())
            check(MessageDigest.isEqual(held.originalKeyReference().toByteArray(Charsets.UTF_8), alias) &&
                MessageDigest.isEqual(held.publicKeySec1(), point)) { "Original Bootstrap app key changed" }
        }
        current()
        val id = operationId.copyOf()
        val fields = bridge.invokeOrdinaryBootstrapApproval(listOf(KagemushaCoreCoordinatorFrameV1.u32(8), id))
        try {
            current()
            check(MessageDigest.isEqual(fields[7], c) && MessageDigest.isEqual(fields[3], alias) &&
                MessageDigest.isEqual(fields[5], point) && MessageDigest.isEqual(fields[8], credential)) {
                "Bootstrap projection differs from its original FI enrollment"
            }
            return KagemushaNativePreparedOrdinaryBootstrapApprovalV1.fromNative(bridge, id, fields, ::current,
                completed.enrollmentId(), completed.originalCertificate())
        } catch (failure: Throwable) {
            try { bridge.close() } catch (_: Throwable) { /* Preserve the original binding failure. */ }
            throw failure
        }
    }

    /** Derive E after raw identity admission, selected only by SHA256(full original C signing bytes). */
    fun prepareEnrollmentPossession(enrollmentOperationId: ByteArray): KagemushaNativePreparedAppEnrollmentPossessionV1 {
        val id = enrollmentOperationId.copyOf()
        val fields = bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION,
            listOf(KagemushaCoreCoordinatorFrameV1.u32(1), id))
        return KagemushaNativePreparedAppEnrollmentPossessionV1.fromNative(bridge, id, fields)
    }
}

/** Private ordinary Bootstrap W/S capability. Its receipt captures approval, never State readiness. */
class KagemushaNativePreparedOrdinaryBootstrapApprovalV1 private constructor(private val state: NativeAppPreparedStateV1) {
    fun signingBytes(): ByteArray = state.signingBytes()
    fun bootstrapSelectionOriginal(): ByteArray = state.financialSelectionOriginal()
    fun recoverOriginalApproval(): ByteArray? = state.recover()
    /** Prove and durably publish only the same Native-selected initial State/Guard originals.
     * The installed authenticated profile and artifact resolver are selected in Native, never
     * supplied by this frame. A complete original W is reused without a platform invocation.
     */
    fun publishOriginalInitialState(): KagemushaOrdinaryInitialStatePublicationOriginalsV1 = state.initialPublication(recoverOnly = false)
    /** Recheck an existing original publication; this call never generates a proof or signature. */
    fun recoverOriginalInitialStatePublication(): KagemushaOrdinaryInitialStatePublicationOriginalsV1 = state.initialPublication(recoverOnly = true)
    fun cancel() = state.cancel()
    internal fun performPlatformSigning(callback: AndroidAppSigningCallbackV1): ByteArray = state.sign(callback)
    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, id: ByteArray, fields: List<ByteArray>, requireOriginal: () -> Unit,
            enrollmentId: ByteArray? = null, retailCertificate: ByteArray? = null) =
            KagemushaNativePreparedOrdinaryBootstrapApprovalV1(NativeAppPreparedStateV1(bridge,
                KagemushaCoreCoordinatorMethodV1.PREPARED_APP_OPERATION_APPROVAL, id, fields,
                ordinaryBootstrap = true, requireOriginalOwner = requireOriginal,
                completedEnrollmentId = enrollmentId, completedRetailCertificate = retailCertificate))
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
    /** Public HTTP start carrier remains selected by the same Native reservation and admitted credential. */
    fun retailStartRequestOriginal(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1): KagemushaOrdinaryIdentityHttpOriginalV1 =
        state.retailStartRequest(reservation, identity)
    /** Native authenticates and fsyncs the exact FI challenge before exposing the wallet signing digest. */
    suspend fun prepareOriginalRetailEnrollment(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1,
        transport: KagemushaOrdinaryIdentityOriginalTransportV1): KagemushaNativeOrdinaryRetailEnrollmentV1 {
        val request = retailStartRequestOriginal(reservation, identity)
        request.requireCurrent()
        val raw = transport.exchange(request)
        request.requireCurrent()
        return state.acceptRetailStart(reservation, identity, request, raw)
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
    private val ordinaryBootstrap: Boolean = false,
    private val requireOriginalOwner: () -> Unit = {},
    completedEnrollmentId: ByteArray? = null,
    completedRetailCertificate: ByteArray? = null,
) {
    private val originalId = id.copyOf()
    private val original = fields.map(ByteArray::copyOf)
    private val ticket = original[0].copyOf()
    private val message = original[1].copyOf()
    private val scope = original[9].copyOf()
    private val messageDigest = sha(message)
    private var unusable = false
    private var certificateBody: ByteArray? = null
    private var credentialOriginal: ByteArray? = null
    private var credentialDigest: ByteArray? = null
    private val initialEnrollmentId = completedEnrollmentId?.copyOf()
    private val initialRetailCertificateDigest = completedRetailCertificate?.let(::sha)
    private var initialPublicationOriginal: List<ByteArray>? = null

    @Synchronized fun initialPublication(recoverOnly: Boolean): KagemushaOrdinaryInitialStatePublicationOriginalsV1 {
        check(ordinaryBootstrap) { "Only the original ordinary Bootstrap ticket can select initial publication" }
        val enrollment = checkNotNull(initialEnrollmentId) { "Original Native FI enrollment binding is unavailable" }
        val certificate = checkNotNull(initialRetailCertificateDigest) { "Original Native FI certificate binding is unavailable" }
        recheck()
        checkNotNull(recover()) { "Native has not captured the original Bootstrap approval" }
        try {
            val published = invoke(if (recoverOnly) 10 else 9)
            same(published[0], ticket)
            same(published[1], enrollment)
            same(published[3], certificate)
            same(published[4], original[8])
            initialPublicationOriginal?.let { prior -> prior.indices.forEach { same(prior[it], published[it]) } }
            // The full retained W archive is Native-owned. Its digest is deliberately not
            // reconstructed from W signing bytes or the detached platform receipt here.
            recheck()
            initialPublicationOriginal = published.map(ByteArray::copyOf)
            return KagemushaOrdinaryInitialStatePublicationOriginalsV1(published)
        } catch (failure: Throwable) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Preserve the original publication failure. */ }
            throw failure
        }
    }

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
        credentialOriginal?.let { same(it, held) }
        credentialDigest?.let { same(it, admitted[0]) }
        credentialOriginal = held.copyOf(); credentialDigest = admitted[0].copyOf()
        return admitted[0].copyOf()
    }

    @Synchronized fun retailStartRequest(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1): KagemushaOrdinaryIdentityHttpOriginalV1 {
        val (selected, policy) = certificateInputs(reservation, identity)
        val credential = checkNotNull(credentialOriginal) { "Native has not admitted the original ordinary credential" }.copyOf()
        val digest = checkNotNull(credentialDigest).copyOf()
        val body = KagemushaOrdinaryIdentityHttpCodecV1.retailStartBody(selected[0], credential)
        return KagemushaOrdinaryIdentityHttpOriginalV1.retail(originalId, "start", body) {
            recheckCertificateInputs(reservation, identity, selected, policy)
            recheckRetailCredential(credential, digest)
        }
    }

    @Synchronized fun acceptRetailStart(reservation: KagemushaNativeReservedOrdinaryAppIdentityV1,
        identity: KagemushaNativePreparedOrdinaryAppIdentityV1, request: KagemushaOrdinaryIdentityHttpOriginalV1,
        raw: ByteArray): KagemushaNativeOrdinaryRetailEnrollmentV1 {
        request.requireCurrent()
        val (selected, _) = certificateInputs(reservation, identity)
        val credential = checkNotNull(credentialOriginal).copyOf(); val digest = checkNotNull(credentialDigest).copyOf()
        val response = KagemushaOrdinaryIdentityHttpCodecV1.retailStartResponse(raw, selected[0])
        request.requireCurrent()
        val admitted = try { bridge.invoke(method, listOf(KagemushaCoreCoordinatorFrameV1.u32(9),
            ticket.copyOf(), response[0], response[1])) }
        catch (failure: Throwable) { unusable = true; throw failure }
        same(admitted[3], scope); same(admitted[4], digest)
        request.requireCurrent()
        return KagemushaNativeOrdinaryRetailEnrollmentV1.fromNative(bridge, originalId, admitted, scope, digest) {
            recheckRetailCredential(credential, digest)
        }
    }

    @Synchronized private fun recheckRetailCredential(credential: ByteArray, digest: ByteArray) {
        recheck()
        same(checkNotNull(credentialOriginal), credential); same(checkNotNull(credentialDigest), digest)
        recheck()
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
        try { requireOriginalOwner() }
        catch (failure: Throwable) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { /* Preserve original owner revocation. */ }
            throw failure
        }
        val checked = invoke(6)
        same(checked[0], scope); same(checked[1], messageDigest)
    }

    private fun invoke(phase: Int, raw: ByteArray? = null): List<ByteArray> {
        check(!unusable) { "Original app-signing capability is unavailable" }
        val request = arrayListOf(KagemushaCoreCoordinatorFrameV1.u32(phase), ticket.copyOf())
        raw?.let { request.add(it.copyOf()) }
        return try { if (ordinaryBootstrap) bridge.invokeOrdinaryBootstrapApproval(request) else bridge.invoke(method, request) }
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
