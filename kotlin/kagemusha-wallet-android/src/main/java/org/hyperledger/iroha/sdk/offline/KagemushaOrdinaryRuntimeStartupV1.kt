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
