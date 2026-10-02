// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Dedicated field contract implemented by the exact SDK wallet JNI owner.
 * Public bindings reject every other implementing class before exposing their descriptor.
 * Raw descriptor correlation grants no authority; Native independently authenticates custody.
 */
fun interface KagemushaOrdinaryNativeCurrentControlEndpointV1 {
    fun invoke(phase: Int, coreHandle: Long, signedOriginal: ByteArray, authorityOriginal: ByteArray): Array<ByteArray>?
}

/** Startup field contract; only the exact final SDK wallet JNI owner can dispatch. */
fun interface KagemushaOrdinaryNativeStartupEndpointV1 {
    fun startup(phase: Int, readId: Long, original: ByteArray): Array<ByteArray>?
}

/** Closed binding to an already opened coordinator. This projects transport fields only;
 * the independently installed Native owner remains the sole FI/cash authenticator.
 */
class KagemushaOrdinaryCurrentControlTransportBindingV1 internal constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) {
    /** Admit only the exact SDK wallet JNI class in the bridge's defining loader, then dispatch
     * under the same descriptor lock and revoke on any uncertain reply or linkage.
     */
    fun invoke(endpoint: KagemushaOrdinaryNativeCurrentControlEndpointV1, phase: Int,
        signedOriginal: ByteArray = ByteArray(0), authorityOriginal: ByteArray = ByteArray(0)): List<ByteArray> {
        require(phase in 1..3)
        if (phase == 3) {
            require(signedOriginal.size in 1..KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_SIGNED_CONTROL_BYTES)
            require(authorityOriginal.size in 1..KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_AUTHORITY_BYTES)
        } else require(signedOriginal.isEmpty() && authorityOriginal.isEmpty())
        return bridge.invokeOrdinaryCurrentControl(endpoint, phase, signedOriginal, authorityOriginal)
    }

    /** Execute only the retained startup reserve/fetch grammar under this same descriptor lock.
     * No caller original, descriptor projection or owner callback is accepted.
     */
    fun invokeStartup(endpoint: KagemushaOrdinaryNativeStartupEndpointV1, phase: Int,
        readId: Long = 0L): List<ByteArray> {
        require((phase == 1 && readId == 0L) || (phase == 6 && readId != 0L))
        return bridge.invokeOrdinaryStartup(endpoint, phase, readId)
    }

    /** Check local descriptor custody; genuine current session/clock/PI checks occur in Native. */
    fun requireOpen() = bridge.requireOrdinaryDescriptorOpen()

    /** Revoke this sole descriptor after startup/custody uncertainty; never reopen it here. */
    fun revoke() = bridge.close()
}
