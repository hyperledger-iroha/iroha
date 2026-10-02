// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import android.app.Application

/** Initial actual Native S/W read and source installation, before the first Core open.
 * The Native release/account producer must already be registered independently. This facade
 * accepts no runtime roots, public account grants, managed signer, clock or replacement key.
 * Failure consumes this process-local attempt; later logout/replacement cannot compose a new
 * Native owner in-process. No enrollment, FI, proof, balance or money capability is returned.
 */
object KagemushaOrdinaryRuntimeStartupV1 {
    private val applicationBinding = KagemushaOrdinaryApplicationBindingGateV1<Application>()

    /** Retain the actual Application/JNI identity only; no wallet or account is read here.
     * Failure is permanent for this process. A repeated call can recheck only the same Application.
     */
    @JvmStatic
    fun bindApplication(application: Application) = applicationBinding.bind(application,
        KagemushaCoreCoordinatorBridgeV1::loadOrdinaryRuntimeNativeContract) {
        try {
            KagemushaOrdinaryRuntimeJniV1.bindApplication(application)
        } catch (error: LinkageError) {
            throw IllegalStateException("Actual Native Application identity binding is unavailable", error)
        }
    }

    /** Native invokes the fixed final loan method on the measured manifest Application. Only
     * that method can obtain the checked activation's existing encrypted signatory. No managed
     * storage callback, key bytes, account selector or recovery flag enters this public method.
     */
    @JvmStatic fun admitExistingAndroidAccount(application: android.app.Application) {
        System.loadLibrary("connect_norito_bridge")
        requireExistingAndroidAccountReceipt(KagemushaOrdinaryRuntimeJniV1.existingAndroidAccount(application))
    }
    @JvmStatic
    fun selectInitialAccount(): Unit =
        KagemushaCoreCoordinatorBridgeV1.selectInitialOrdinaryAccount(KagemushaOrdinaryRuntimeJniV1)
}

/** Managed ordering only. The fixed Native receiver independently authenticates actual identity. */
internal class KagemushaOrdinaryApplicationBindingGateV1<A : Any> {
    private var attempted = false
    private var complete = false
    private var original: A? = null

    @Synchronized
    fun bind(application: A, probe: () -> Unit, bindOriginal: () -> Boolean) {
        if (attempted) {
            check(complete && original === application) { "The original Application identity binding is unavailable" }
        } else {
            attempted = true
            probe()
        }
        // A lost or refused retained recheck cannot permit another binding attempt.
        complete = false
        check(bindOriginal()) { "Actual Native Application identity binding was rejected" }
        original = application
        complete = true
    }
}

/** Pure framing after the measured product loan completed. A receipt is correlation data only. */
internal fun requireExistingAndroidAccountReceipt(reply: Array<ByteArray>?) {
    val fields = checkNotNull(reply) { "Actual existing Native account intake is unavailable" }
    require(fields.size == 3 && fields[0].contentEquals(byteArrayOf(1, 0)))
    fun account(raw: ByteArray): String {
        require(raw.isNotEmpty() && raw.size <= 512)
        val value = Charsets.UTF_8.newDecoder()
            .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
            .onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
            .decode(java.nio.ByteBuffer.wrap(raw)).toString()
        require(value == value.trim() && value.none { it.isWhitespace() || it.isISOControl() })
        return value
    }
    require(account(fields[1]) != account(fields[2])) { "Actual S and W must be distinct" }
}
