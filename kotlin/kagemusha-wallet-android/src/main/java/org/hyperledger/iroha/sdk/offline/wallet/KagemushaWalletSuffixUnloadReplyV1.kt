// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Dedicated JNI holder. Its kind73 token cannot be mistaken for an ordinary review. */
internal class KagemushaWalletSuffixUnloadReplyV1(
    @JvmField val status: Int,
    @JvmField val reason: Int,
    @JvmField val platformCode: Int,
    @JvmField val sequenceLow: Long,
    @JvmField val sequenceHigh: Long,
    @JvmField val detail: Int,
    original: ByteArray,
) {
    private val retained: ByteArray
    init {
        val maximum = when (status) {
            72 -> 256
            73 -> 12_288
            74, 77 -> 512
            75 -> KagemushaWalletSuffixUnloadV1.INSTRUCTION_MAX_BYTES
            78 -> KagemushaWalletSuffixUnloadV1.SIGNED_WIRE_MAX_BYTES
            46, 69, 70, 76 -> 32
            else -> 0
        }
        requireSuffix(original.size <= maximum)
        if (status < 0) {
            requireSuffix(sequenceLow == 0L && sequenceHigh == 0L && detail == 0 && original.isEmpty())
        } else {
            requireSuffix(reason == -1 && platformCode == 0 && sequenceHigh == 0L)
            when (status) {
                2 -> requireSuffix(sequenceLow == 0L && detail == 0 && original.isEmpty())
                46 -> requireSuffix(sequenceLow == 0L && detail == 0 && nonzeroWord(original))
                65 -> requireSuffix(sequenceLow > 0 && detail in listOf(1, 6) && original.isEmpty())
                67 -> requireSuffix(sequenceLow == 0L && detail in listOf(1, 6) && original.isEmpty())
                68 -> requireSuffix(sequenceLow != 0L && detail == 0 && original.isEmpty())
                69, 70, 76 -> requireSuffix(java.lang.Long.compareUnsigned(sequenceLow, 2) >= 0 &&
                    detail == 0 && nonzeroWord(original))
                71 -> requireSuffix(sequenceLow == 0L && detail in 1..3 && original.isEmpty())
                72, 74, 75, 77 -> requireSuffix(sequenceLow == 0L && detail == 0 && original.isNotEmpty())
                73 -> requireSuffix(sequenceLow > 0 && detail == 0 && original.isNotEmpty())
                78 -> requireSuffix(sequenceLow == 0L && detail == 0)
                else -> invalidSuffixOutput()
            }
        }
        retained = original.copyOf()
    }
    internal fun checked(): KagemushaWalletSuffixUnloadReplyV1 {
        if (status < 0) throw KagemushaWalletExceptionV1(status, reason, platformCode)
        return this
    }
    internal fun bytes(): ByteArray = retained.copyOf()
    internal fun cleanupToken(): Long? = sequenceLow.takeIf { status == 73 && it > 0 }
    override fun toString(): String = "KagemushaWalletSuffixUnloadReplyV1(status=$status, source=[REDACTED])"
    private fun nonzeroWord(bytes: ByteArray) = bytes.size == 32 && bytes.any { it != 0.toByte() }
}
