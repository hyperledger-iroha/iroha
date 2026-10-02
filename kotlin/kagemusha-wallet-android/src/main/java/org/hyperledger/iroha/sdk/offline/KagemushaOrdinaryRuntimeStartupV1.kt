// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Initial actual Native S/W read and source installation, before the first Core open.
 * The Native release/account producer must already be registered independently. This facade
 * accepts no runtime roots, public account grants, managed signer, clock or replacement key.
 * Failure consumes this process-local attempt; later logout/replacement cannot compose a new
 * Native owner in-process. No enrollment, FI, proof, balance or money capability is returned.
 */
object KagemushaOrdinaryRuntimeStartupV1 {
    @JvmStatic
    fun selectInitialAccount(): Unit =
        KagemushaCoreCoordinatorBridgeV1.selectInitialOrdinaryAccount(KagemushaOrdinaryRuntimeJniV1)
}
