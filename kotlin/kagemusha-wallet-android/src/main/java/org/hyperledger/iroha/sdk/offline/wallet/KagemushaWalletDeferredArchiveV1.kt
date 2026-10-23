// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Native-selected observation, never caller-supplied Archive authority. */
enum class KagemushaWalletMaintenanceWaitV1 {
    SELECTED_PENDING, ARCHIVE_FOLD, FOLD_BACKLOG, EXCHANGE_OPEN, NOT_DUE, BUSY;
    internal companion object {
        fun from(tag: Int): KagemushaWalletMaintenanceWaitV1 =
            values().getOrNull(tag - 1) ?: invalidDeferredOutput()
    }
}

/** One Native-selected maintenance turn, distinct from Payment delivery evidence. */
sealed class KagemushaWalletDeferredArchiveDrainV1 {
    object NoWork : KagemushaWalletDeferredArchiveDrainV1()
    class Blocked(val wait: KagemushaWalletMaintenanceWaitV1) : KagemushaWalletDeferredArchiveDrainV1()
    class Reconciled(val completion: KagemushaWalletCallV1) : KagemushaWalletDeferredArchiveDrainV1()
    class Executed(val completion: KagemushaWalletCallV1) : KagemushaWalletDeferredArchiveDrainV1()

    internal companion object {
        fun from(result: KagemushaWalletCallV1): KagemushaWalletDeferredArchiveDrainV1 = when (result.status) {
            KagemushaWalletCallV1.DEFERRED_NO_WORK -> NoWork
            KagemushaWalletCallV1.DEFERRED_BLOCKED -> Blocked(KagemushaWalletMaintenanceWaitV1.from(result.detail))
            KagemushaWalletCallV1.DEFERRED_COMPLETION -> {
                val bytes = result.bytes()
                if (bytes.size !in 28..10_062) invalidDeferredOutput()
                fun u32(offset: Int): Long = (0..3).fold(0L) { value, index ->
                    value or ((bytes[offset + index].toLong() and 255) shl (8 * index))
                }
                fun u64(offset: Int): Long = (0..7).fold(0L) { value, index ->
                    value or ((bytes[offset + index].toLong() and 255) shl (8 * index))
                }
                val kind = u32(0)
                val length = u32(24)
                if (kind !in listOf(1L, 2L, 3L, 4L, 5L, 10L) || length > 10_034 ||
                    bytes.size.toLong() != 28 + length) invalidDeferredOutput()
                val completion = KagemushaWalletCallV1(kind.toInt(), -1, 0, u64(8), u64(16),
                    u32(4).toInt(), bytes.copyOfRange(28, bytes.size)).completion()
                if (result.detail == 1) Reconciled(completion) else Executed(completion)
            }
            else -> invalidDeferredOutput()
        }
    }
}

private fun invalidDeferredOutput(): Nothing =
    throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
