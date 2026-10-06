// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.NetworkId

/**
 * Exact selectors of the current finalized-load issuance read route.
 *
 * These identify one retained original; they grant no issuance, finality or balance authority.
 */
class ToriiKagemushaWalletLoadSelectionV1(
    schemeId: ByteArray,
    walletId: ByteArray,
    requestId: ByteArray,
) {
    private val scheme = checkedIdentity(schemeId)
    private val wallet = checkedIdentity(walletId)
    private val request = checkedIdentity(requestId)

    /** Owned copy of the selected scheme identity. */
    val schemeId: ByteArray get() = scheme.copyOf()
    /** Owned copy of the selected wallet incarnation. */
    val walletId: ByteArray get() = wallet.copyOf()
    /** Owned copy of the original stable Load retry identity. */
    val requestId: ByteArray get() = request.copyOf()

    internal val path: String
        get() = "/v1/kagemusha/${hex(scheme)}/wallets/${hex(wallet)}/loads/${hex(request)}"

    private fun checkedIdentity(value: ByteArray): ByteArray {
        require(value.size == 32 && value.any { it != 0.toByte() }) {
            "KAGEMUSHA load selectors must be nonzero 32-byte identities"
        }
        return value.copyOf()
    }

    private fun hex(value: ByteArray): String = buildString(value.size * 2) {
        for (byte in value) {
            val unsigned = byte.toInt() and 0xff
            append("0123456789abcdef"[unsigned ushr 4])
            append("0123456789abcdef"[unsigned and 15])
        }
    }
}

/**
 * Bounded, unverified HTTP original for the Native wallet's Load admission.
 *
 * [selection], [payerAccountId] and [networkId] retain the request's expected identities. They
 * do not assert that the response contains those identities. Native must decode the original,
 * bind its request, canonical payer, scheme and wallet to the enrolled owner, and verify any
 * original voucher and complete authenticated Load relation before Advance changes value.
 *
 * This holder exposes no decoded voucher, pending verdict, amount, finality or balance. A 200
 * response can contain an unsigned pending body and can never credit value by itself.
 */
class ToriiKagemushaWalletLoadIssuanceOriginalV1 internal constructor(
    /** Immutable selectors used for the exact signed request. */
    @JvmField val selection: ToriiKagemushaWalletLoadSelectionV1,
    /** Exact payer identity offered by the application-owned account authentication. */
    @JvmField val payerAccountId: String,
    /** Exact local signing network, never inferred from response bytes. */
    @JvmField val networkId: NetworkId,
    unverifiedResponseOriginal: ByteArray,
) {
    private val original = unverifiedResponseOriginal.copyOf()

    init {
        require(original.isNotEmpty() && original.size <= MAXIMUM_BYTES) {
            "KAGEMUSHA issuance original is empty or exceeds its bound"
        }
    }

    /** Owned copy of the complete response original for Native decoding and verification. */
    val unverifiedResponseOriginal: ByteArray get() = original.copyOf()

    companion object {
        /**
         * Finite transport capacity covering the maximum canonical account literal/frame
         * (36 KiB), fixed voucher body, bounded 1 KiB voucher and Norito overhead.
         */
        const val MAXIMUM_BYTES: Int = 64 * 1024
    }
}
