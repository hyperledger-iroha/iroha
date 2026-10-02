// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest

/** Sole incoming JNI field grammar. Bounds mirror the complete Model/Native originals.
 * These checks admit data only; they never construct a Native cash, clock, proof or key owner.
 */
object KagemushaOrdinaryIncomingFrameV1 {
    const val FINALIZED_MAXIMUM_BYTES = 32 * 1024 * 1024 + 4 * 1024 * 1024 + 512 * 1024
    const val CREDIT_MAXIMUM_BYTES = 7_936
    const val OUTGOING_MAXIMUM_BYTES = 16 * 1024 * 1024 + 4 * 1024 + 256 * 1024
    const val RECEIVED_ASSERTION_MAXIMUM_BYTES = 16 * 1024 * 1024 + 192 * 1024
    const val RESERVATION_BUNDLE_MAXIMUM_BYTES = 101_498_880
    const val COMMIT_BUNDLE_MAXIMUM_BYTES = 135_266_304
    const val SIGNED_MAXIMUM_BYTES = 32 * 1024
    const val DATA_MAXIMUM_BYTES = 128 * 1024
    const val AUTHORITY_MAXIMUM_BYTES = 128 * 1024 * 1024
    const val REQUEST_MAXIMUM_BYTES = 192 * 1024

    fun requireRequest(phase: Int, fields: List<ByteArray>) {
        when (phase) {
            1 -> { require(fields.size == 2); original(fields[0], FINALIZED_MAXIMUM_BYTES); original(fields[1], CREDIT_MAXIMUM_BYTES) }
            17 -> { require(fields.size == 3); digest(fields[0]); original(fields[1], OUTGOING_MAXIMUM_BYTES); original(fields[2], RECEIVED_ASSERTION_MAXIMUM_BYTES) }
            3,10 -> { require(fields.size == 1); require(fields[0].size in 8..72) }
            7 -> { require(fields.size == 3); original(fields[0], SIGNED_MAXIMUM_BYTES); original(fields[1], DATA_MAXIMUM_BYTES); original(fields[2], AUTHORITY_MAXIMUM_BYTES) }
            8,14,15 -> { require(fields.size == 1); digest(fields[0]) }
            2,4,5,6,9,11,12,13,16 -> require(fields.isEmpty())
            else -> error("Unknown ordinary incoming phase")
        }
    }
    fun responseFields(phase: Int, handle: Long, response: List<ByteArray>): List<ByteArray> {
        require(handle != 0L && response.size >= 3 && response[0].contentEquals(byteArrayOf(1,0)) &&
            response[1].contentEquals(byteArrayOf(phase.toByte())) && response[2].contentEquals(le64(handle))) {
            "Ordinary incoming Native response identity differs"
        }
        val fields = response.drop(3)
        requireResponse(phase, fields)
        return fields.map(ByteArray::copyOf)
    }
    fun requireResponse(phase: Int, fields: List<ByteArray>) {
        when (phase) {
            1,8,17 -> {
                require(fields.size == 4); digest(fields[0]); require(fields[1].size == 325 && fields[2].size == 460)
                original(fields[3], SIGNED_MAXIMUM_BYTES)
                require(MessageDigest.isEqual(fields[0], fields[1].copyOfRange(53,85)))
            }
            2,4,9,11 -> {
                require(fields.size == 3 && fields[0].size == 1 && fields[0][0].toInt() in 0..2)
                if (fields[0][0] == 0.toByte()) require(fields[1].isEmpty() && fields[2].isEmpty())
                else { require(fields[1].size in 8..72); original(fields[2], SIGNED_MAXIMUM_BYTES) }
            }
            3,5,7,10,12 -> { require(fields.size == 1); digest(fields[0]) }
            6,13 -> {
                require(fields.size == 5 && fields[0].size == 1 && fields[0][0].toInt() in listOf(0,2))
                original(fields[1], REQUEST_MAXIMUM_BYTES); require(fields[2].size == 64)
                original(fields[3], if (phase == 6) RESERVATION_BUNDLE_MAXIMUM_BYTES else COMMIT_BUNDLE_MAXIMUM_BYTES)
                digest(fields[4]); require(MessageDigest.isEqual(fields[4], sha(fields[1])))
            }
            14,15,16 -> require(fields.isEmpty())
            else -> error("Unknown ordinary incoming response phase")
        }
    }
    private fun original(raw: ByteArray, maximum: Int) = require(raw.size in 1..maximum)
    private fun digest(raw: ByteArray) = require(raw.size == 32 && raw.any { it != 0.toByte() })
    private fun le64(value: Long) = ByteArray(8) { (value ushr (it * 8)).toByte() }
    private fun sha(raw: ByteArray) = MessageDigest.getInstance("SHA-256").digest(raw)
}
