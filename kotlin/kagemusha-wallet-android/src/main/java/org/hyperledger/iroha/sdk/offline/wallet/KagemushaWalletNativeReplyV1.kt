// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Private typed callback result; it is not a wire codec or an app signing surface. */
internal class KagemushaWalletNativeReplyV1(
    @JvmField val tag: Int,
    @JvmField val reason: Int = 4,
    @JvmField val code: Int = 0,
    bytes: ByteArray = byteArrayOf(),
) {
    private val payload = bytes.copyOf()
    fun bytes(): ByteArray = payload.copyOf()
    companion object {
        fun unavailable(reason: KagemushaWalletAndroidUnavailableV1, tag: Int = 2) =
            KagemushaWalletNativeReplyV1(tag, reason.kind.tag, reason.code)
    }
}
