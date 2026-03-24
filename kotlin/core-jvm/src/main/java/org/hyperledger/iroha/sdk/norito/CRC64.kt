// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.ByteBuffer

private const val POLY = -0x3693A86A2878F0BEL // 0xC96C5795D7870F42L
private const val INIT = -1L // 0xFFFFFFFFFFFFFFFFL
private const val XOR_OUT = -1L // 0xFFFFFFFFFFFFFFFFL

/** CRC64-XZ implementation matching Rust/Python codecs. */
object CRC64 {
    private val TABLE: LongArray = buildTable()

    @JvmStatic
    fun compute(data: ByteArray): Long = compute(data, INIT)

    @JvmStatic
    fun compute(data: ByteArray, initial: Long): Long {
        var crc = initial
        for (b in data) {
            val index = ((crc xor (b.toLong() and 0xFF)) and 0xFF).toInt()
            crc = TABLE[index] xor (crc ushr 8)
        }
        return crc xor XOR_OUT
    }

    @JvmStatic
    fun compute(data: ByteBuffer): Long = compute(data, INIT)

    @JvmStatic
    fun compute(data: ByteBuffer, initial: Long): Long {
        val view = data.slice()
        var crc = initial
        while (view.hasRemaining()) {
            val index = ((crc xor (view.get().toLong() and 0xFF)) and 0xFF).toInt()
            crc = TABLE[index] xor (crc ushr 8)
        }
        return crc xor XOR_OUT
    }

    private fun buildTable(): LongArray {
        val table = LongArray(256)
        for (i in 0 until 256) {
            var crc = i.toLong()
            for (bit in 0 until 8) {
                crc = if (crc and 1L != 0L) {
                    (crc ushr 1) xor POLY
                } else {
                    crc ushr 1
                }
            }
            table[i] = crc
        }
        return table
    }
}
