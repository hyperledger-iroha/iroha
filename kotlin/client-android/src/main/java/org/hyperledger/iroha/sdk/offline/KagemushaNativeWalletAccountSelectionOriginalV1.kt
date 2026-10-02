// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

/** Finite exact original W/S projection from one held installed Native account session.
 * No public constructor, copied literal, DTO or detached ID recreates this holder.
 * Every use rechecks its same coordinator/session. This is data, not signing or money authority.
 */
class KagemushaNativeWalletAccountSelectionOriginalV1 private constructor(
    private val state: NativeWalletAccountSelectionStateV1,
) {
    fun walletAccountId(): String = state.field(1).toString(Charsets.UTF_8)
    fun signatoryAccountId(): String = state.field(2).toString(Charsets.UTF_8)
    fun requireCurrent() = state.recheck()
    internal fun requireForCoordinator(bridge: KagemushaCoreCoordinatorBridgeV1) = state.requireFor(bridge)
    internal companion object {
        fun fromNative(bridge: KagemushaCoreCoordinatorBridgeV1, fields: List<ByteArray>) =
            KagemushaNativeWalletAccountSelectionOriginalV1(NativeWalletAccountSelectionStateV1(bridge, fields))
    }
}

private class NativeWalletAccountSelectionStateV1(
    private val bridge: KagemushaCoreCoordinatorBridgeV1, fields: List<ByteArray>,
) {
    private val original = fields.map(ByteArray::copyOf)
    private var unusable = false
    init { recheck() }
    @Synchronized fun field(index: Int): ByteArray {
        recheck(); return original[index].copyOf().also { recheck() }
    }
    @Synchronized fun requireFor(other: KagemushaCoreCoordinatorBridgeV1) {
        check(bridge === other) { "Current wallet selection belongs to another Native coordinator" }
        recheck()
    }
    @Synchronized fun recheck() {
        check(!unusable) { "Original Native wallet selection is unavailable" }
        val held = try {
            bridge.invoke(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(15)))
        } catch (failure: Throwable) { unusable = true; throw failure }
        if (original.size != held.size || original.indices.any { !MessageDigest.isEqual(original[it], held[it]) }) {
            unusable = true
            try { bridge.close() } catch (_: Throwable) { }
            error("Native current wallet/session substituted the original W/S selection")
        }
    }
}
