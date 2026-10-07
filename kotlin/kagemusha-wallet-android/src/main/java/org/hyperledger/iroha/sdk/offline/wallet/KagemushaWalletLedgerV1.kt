// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Closed original Native transports accepted by the current ledger instruction. */
enum class KagemushaWalletLedgerTransportV1(internal val tag: Long) {
    ACTIVATE(1), UNLOAD(2), CLOSE_LOADS(3),
}

/** Native-verified successful inclusion of the exact account's retained Unload instruction. */
class KagemushaWalletUnloadConfirmationV1 internal constructor(result: KagemushaWalletCallV1) {
    val heightBits: Long
    private val block: ByteArray
    init {
        if (result.status != KagemushaWalletCallV1.UNLOAD_CONFIRMATION) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        heightBits = result.sequenceLow
        block = result.bytes()
    }
    fun blockHash(): ByteArray = block.copyOf()
}

/** Native-selected receipt height and verified contiguous per-Load history, as unsigned u64 bits. */
class KagemushaWalletLoadProofProgressV1 internal constructor(result: KagemushaWalletCallV1) {
    val receiptHeightBits: Long
    val verifiedHeightBits: Long
    init {
        if (result.status != KagemushaWalletCallV1.LOAD_PROOF_PROGRESS) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        receiptHeightBits = result.sequenceLow
        var value = 0L
        for (byte in result.bytes()) value = (value shl 8) or (byte.toLong() and 255)
        if (java.lang.Long.compareUnsigned(value, receiptHeightBits) > 0) throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        verifiedHeightBits = value
    }
}
