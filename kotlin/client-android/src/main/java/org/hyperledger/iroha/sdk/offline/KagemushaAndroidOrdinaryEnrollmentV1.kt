// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyStoreV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityProviderV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityTokenOriginalV1
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine

/** Detached original enrollment result. Financial readiness requires the separate Native State and Guard publication. */
class KagemushaOrdinaryEnrollmentOriginalsV1 internal constructor(id: ByteArray, certificate: ByteArray) {
    private val originalId = id.copyOf()
    private val originalCertificate = certificate.copyOf()
    fun enrollmentId(): ByteArray = originalId.copyOf()
    fun originalRetailCertificate(): ByteArray = originalCertificate.copyOf()
}

/** Detached captured Bootstrap approval originals; genuine State/Guard publication is separate. */
class KagemushaOrdinaryBootstrapApprovalOriginalsV1 internal constructor(enrollment: KagemushaOrdinaryEnrollmentOriginalsV1,
    operation: ByteArray, message: ByteArray, selection: ByteArray, receipt: ByteArray) {
    private val id = enrollment.enrollmentId()
    private val certificate = enrollment.originalRetailCertificate()
    private val operationId = operation.copyOf()
    private val signingMessage = message.copyOf()
    private val bootstrapSelection = selection.copyOf()
    private val originalReceipt = receipt.copyOf()
    fun enrollmentId(): ByteArray = id.copyOf()
    fun originalRetailCertificate(): ByteArray = certificate.copyOf()
    fun bootstrapOperationId(): ByteArray = operationId.copyOf()
    fun originalSigningBytes(): ByteArray = signingMessage.copyOf()
    fun originalBootstrapSelection(): ByteArray = bootstrapSelection.copyOf()
    fun originalApprovalReceipt(): ByteArray = originalReceipt.copyOf()
}

/** The shared Android enrollment workflow, backed by one already installed Native account/release owner.
 * Native selects and durably retains the financial secret, C challenge, platform key intent,
 * E possession invocation, wallet invocation and FI originals. The product supplies its protected
 * HTTP transport. The public Android constructor uses only the installed Native
 * account/session custody for retail signing. No applet or OEM provisioning is involved.
 * Keep this workflow for explicit retries; uncertain invocations require Native original recovery.
 */
// The internal primary constructor is an inert workflow-test seam, never a product
// constructor or a way to install a Native account/session from a managed signer.
class KagemushaAndroidOrdinaryEnrollmentV1 internal constructor(
    private val native: KagemushaNativeAppApprovalCoordinatorV1,
    private val transport: KagemushaOrdinaryIdentityOriginalTransportV1,
    private val walletSigner: KagemushaOrdinaryWalletAccountSignerV1?,
    private val requireOriginalOwner: () -> Unit,
    private val collectOriginal: (KagemushaNativePreparedOrdinaryAppIdentityV1) -> Unit,
    private val proveOriginal: (KagemushaNativePreparedAppEnrollmentPossessionV1) -> Unit,
    private val integrity: KagemushaAndroidPlayIntegrityProviderV1,
    private val approveBootstrapOriginal: ((KagemushaNativePreparedOrdinaryBootstrapApprovalV1) -> ByteArray)? = null,
    private val nativeWalletSelection: KagemushaNativeWalletAccountSelectionOriginalV1? = null,
) {
    /** Native-account path requiring the same opaque current session, with no managed signer fallback. */
    constructor(context: Context, coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
        transport: KagemushaOrdinaryIdentityOriginalTransportV1,
        selection: KagemushaNativeWalletAccountSelectionOriginalV1,
        requireOriginalOwner: () -> Unit) : this(coordinator.appIdentityOperations(), transport, null,
            requireOriginalOwner, originalCollector(context), originalPossessionSigner(context),
            KagemushaAndroidPlayIntegrityProviderV1(context.applicationContext), originalBootstrapSigner(context),
            selection.also { coordinator.appIdentityOperations().requireCurrentWalletAccountSelection(it) })
    private val mutex = Mutex()
    private var reservation: KagemushaNativeReservedOrdinaryAppIdentityV1? = null
    private var identity: KagemushaNativePreparedOrdinaryAppIdentityV1? = null
    private var possession: KagemushaNativePreparedAppEnrollmentPossessionV1? = null
    private var integrityOriginal: CompletableFuture<KagemushaAndroidPlayIntegrityTokenOriginalV1>? = null
    private var retail: KagemushaNativeOrdinaryRetailEnrollmentV1? = null
    private var completed: KagemushaOrdinaryEnrollmentOriginalsV1? = null
    private var bootstrap: KagemushaNativePreparedOrdinaryBootstrapApprovalV1? = null
    private var capturedBootstrap: KagemushaOrdinaryBootstrapApprovalOriginalsV1? = null
    private var publishedInitialState: KagemushaOrdinaryInitialStatePublicationOriginalsV1? = null
    private var bootstrapRetiredForCash = false

    /** Complete the same Native ceremony, reusing complete originals and refusing uncertain platform/wallet work. */
    suspend fun beginOrResume(): KagemushaOrdinaryEnrollmentOriginalsV1 = mutex.withLock {
        requireWorkflowOwner()
        requireBootstrapStage()
        completed?.let { original ->
            checkNotNull(retail).originalRetailCertificate().also {
                check(java.security.MessageDigest.isEqual(it, original.originalRetailCertificate()))
            }
            requireWorkflowOwner()
            return@withLock original
        }
        val reserved = reservation ?: native.reserveOriginalIdentity().also { reservation = it }
        nativeWalletSelection?.let {
            native.requireCurrentWalletAccountSelection(it)
            check(it.walletAccountId() == reserved.accountId()) { "Original reservation differs from the current Native W" }
        }
        requireWorkflowOwner()
        val prepared = identity ?: reserved.prepare(guardedTransport()).also { identity = it }
        requireWorkflowOwner()
        collectOriginal(prepared)
        requireWorkflowOwner()
        prepared.admitOriginalAttestation(guardedTransport())
        requireWorkflowOwner()
        val heldPossession = possession ?: native.prepareEnrollmentPossession(native.originalEnrollmentAttemptId())
            .also { possession = it }
        requireWorkflowOwner()
        proveOriginal(heldPossession)
        requireWorkflowOwner()
        val token = if (reserved.originalPlayIntegrityPolicyBytes().isEmpty()) null else {
            val pending = integrityOriginal ?: heldPossession.requestOriginalIntegrityToken(reserved, prepared, integrity)
                .also { integrityOriginal = it }
            try { pending.originalResult().also { requireWorkflowOwner() } }
            catch (failure: Throwable) {
                if (integrity.invalidatedPreparedProviderOriginal(failure) && integrityOriginal === pending) {
                    integrityOriginal = null
                }
                throw failure
            }
        }
        heldPossession.issueOriginalCredential(reserved, prepared, token, guardedTransport())
        requireWorkflowOwner()
        val heldRetail = retail ?: heldPossession.prepareOriginalRetailEnrollment(reserved, prepared, guardedTransport())
            .also { retail = it }
        requireWorkflowOwner()
        val selectedNativeAccount = nativeWalletSelection
        if (selectedNativeAccount != null) {
            native.requireCurrentWalletAccountSelection(selectedNativeAccount)
            heldRetail.signOriginalNativeAccount(selectedNativeAccount)
            native.requireCurrentWalletAccountSelection(selectedNativeAccount)
        } else {
            heldRetail.signOriginalAccount { message ->
                requireWorkflowOwner()
                checkNotNull(walletSigner).signOriginal(message).also { requireWorkflowOwner() }
            }
        }
        requireWorkflowOwner()
        val id = heldRetail.completeOriginalEnrollment(guardedTransport())
        requireWorkflowOwner()
        KagemushaOrdinaryEnrollmentOriginalsV1(id, heldRetail.originalRetailCertificate())
            .also { requireWorkflowOwner(); completed = it }
    }

    /** Capture the original zero-index Bootstrap W after the same FI ceremony, with no money readiness claim.
     * The public selector is deterministic from the full original FI certificate. Native alone
     * selects the financial S, nonce, interval, invocation fence and signature/capture WAL.
     */
    suspend fun beginOrResumeBootstrapApproval(): KagemushaOrdinaryBootstrapApprovalOriginalsV1 {
        val enrollment = beginOrResume()
        return mutex.withLock {
            requireWorkflowOwner()
            requireBootstrapStage()
            val signer = checkNotNull(approveBootstrapOriginal) { "Actual hardware Bootstrap signer is unavailable" }
            val heldRetail = checkNotNull(retail)
            check(java.security.MessageDigest.isEqual(heldRetail.originalRetailCertificate(), enrollment.originalRetailCertificate()))
            val operation = java.security.MessageDigest.getInstance("SHA-256").digest(
                "iroha:kagemusha:v1:ordinary-bootstrap-operation-id\u0000".toByteArray(Charsets.US_ASCII) +
                    enrollment.originalRetailCertificate())
            val held = bootstrap ?: native.prepareOrdinaryBootstrapApproval(operation, checkNotNull(identity), heldRetail)
                .also { bootstrap = it }
            requireWorkflowOwner()
            capturedBootstrap?.let { captured ->
                check(java.security.MessageDigest.isEqual(checkNotNull(held.recoverOriginalApproval()), captured.originalApprovalReceipt()))
                requireWorkflowOwner()
                return@withLock captured
            }
            val receipt = signer(held)
            requireWorkflowOwner()
            KagemushaOrdinaryBootstrapApprovalOriginalsV1(enrollment, operation, held.signingBytes(),
                held.bootstrapSelectionOriginal(), receipt).also { requireWorkflowOwner(); capturedBootstrap = it }
        }
    }

    /** Publish or recover the actual Native initial State and paired ordinary Guard after FI/W.
     * This uses the same retained Bootstrap approval and installed Native proof profile. The
     * detached acknowledgement grants no monetary wallet capability; unavailable authentic
     * proof material remains an error and cannot be repaired with an app-generated proof.
     */
    suspend fun beginOrResumeInitialStatePublication(): KagemushaOrdinaryInitialStatePublicationOriginalsV1 {
        beginOrResumeBootstrapApproval()
        return mutex.withLock {
            requireWorkflowOwner()
            requireBootstrapStage()
            val held = checkNotNull(bootstrap)
            val published = if (publishedInitialState == null) held.publishOriginalInitialState()
                else held.recoverOriginalInitialStatePublication()
            requireWorkflowOwner()
            publishedInitialState = published
            published
        }
    }

    /** Finish the genuine initial publication once, then stop using the Bootstrap holder.
     * This local consumption fence creates no cash owner or FI authority. The wallet module
     * must still dispatch under the same coordinator to actual Native startup and cash intake.
     * Keep this workflow through retries: after retirement old entry points refuse locally.
     */
    suspend fun prepareCurrentFinancialControlHandoff() {
        val needsPublication = mutex.withLock { requireWorkflowOwner(); !bootstrapRetiredForCash }
        if (needsPublication) beginOrResumeInitialStatePublication()
        mutex.withLock {
            requireWorkflowOwner()
            checkNotNull(publishedInitialState) { "The authentic initial publication is unavailable" }
            bootstrapRetiredForCash = true
            requireWorkflowOwner()
        }
    }

    private fun requireWorkflowOwner() {
        requireOriginalOwner()
        nativeWalletSelection?.let(native::requireCurrentWalletAccountSelection)
    }

    private fun requireBootstrapStage() {
        check(!bootstrapRetiredForCash) { "The original Bootstrap holder has been retired for consuming cash handoff" }
    }

    private fun guardedTransport() = KagemushaOrdinaryIdentityOriginalTransportV1 { original ->
        requireWorkflowOwner(); original.requireCurrent()
        transport.exchange(original).also { requireWorkflowOwner(); original.requireCurrent() }
    }
    // Retain the same Google future across cancellation/HTTP retry. A complete opaque token is
    // never replaced to repair an issuer ambiguity, and cancellation does not erase its original.
    private suspend fun <T> CompletableFuture<T>.originalResult(): T = suspendCoroutine { continuation ->
        whenComplete { original, failure ->
            if (failure != null) continuation.resumeWithException(failure) else continuation.resume(original)
        }
    }
    private companion object {
        fun originalCollector(context: Context): (KagemushaNativePreparedOrdinaryAppIdentityV1) -> Unit {
            val hardware = KagemushaAndroidHardwareAppKeyStoreV1(context.applicationContext)
            return { original -> hardware.collectIdentity(original); Unit }
        }
        fun originalPossessionSigner(context: Context): (KagemushaNativePreparedAppEnrollmentPossessionV1) -> Unit {
            val hardware = KagemushaAndroidHardwareAppKeyStoreV1(context.applicationContext)
            return { original -> hardware.proveEnrollmentPossession(original); Unit }
        }
        fun originalBootstrapSigner(context: Context): (KagemushaNativePreparedOrdinaryBootstrapApprovalV1) -> ByteArray {
            val hardware = KagemushaAndroidHardwareAppKeyStoreV1(context.applicationContext)
            return hardware::approveOrdinaryBootstrap
        }
    }
}
