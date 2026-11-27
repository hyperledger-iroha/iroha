// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Exact Request transport originals. IPM1 hashes cover both; only Native authenticates them.
 * The account original is canonical AccountId DATA supplied by the receiving wallet. This
 * structural codec cannot verify its identity, the Request signature, or their digest binding.
 */
class IrohaPeerWalletRequestV1(requestEnvelope: ByteArray, destinationAccountOriginal: ByteArray) {
    private val envelope = requestEnvelope.copyOf()
    private val account = destinationAccountOriginal.copyOf()
    init {
        require(envelope.size in 1..KagemushaWalletWireV1.MESSAGE_MAX_BYTES)
        require(account.size in 1..MAXIMUM_ACCOUNT_BYTES)
        require(KagemushaWalletWireV1.inspectEnvelope(envelope).kind == KagemushaWalletMessageKindV1.REQUEST)
    }
    fun requestEnvelope(): ByteArray = envelope.copyOf()
    fun destinationAccountOriginal(): ByteArray = account.copyOf()
    fun encode(): ByteArray = ByteArray(HEADER_BYTES + envelope.size + account.size).also { out ->
        MAGIC.copyInto(out)
        writeLength(out, 8, envelope.size)
        writeLength(out, 12, account.size)
        envelope.copyInto(out, HEADER_BYTES)
        account.copyInto(out, HEADER_BYTES + envelope.size)
    }
    override fun toString(): String = "IrohaPeerWalletRequestV1(originals=[REDACTED])"

    companion object {
        const val HEADER_BYTES = 16
        const val MAXIMUM_ACCOUNT_BYTES = 4_096
        const val MAXIMUM_BYTES = HEADER_BYTES + KagemushaWalletWireV1.MESSAGE_MAX_BYTES + MAXIMUM_ACCOUNT_BYTES
        private val MAGIC = "KWRQAC1\u0000".toByteArray(Charsets.US_ASCII)

        @JvmStatic fun decode(original: ByteArray): IrohaPeerWalletRequestV1 {
            require(original.size in (HEADER_BYTES + 2)..MAXIMUM_BYTES)
            val bytes = original.copyOf()
            require(bytes.copyOfRange(0, 8).contentEquals(MAGIC)) { "Request account companion is mandatory" }
            val envelopeLength = readLength(bytes, 8, KagemushaWalletWireV1.MESSAGE_MAX_BYTES)
            val accountLength = readLength(bytes, 12, MAXIMUM_ACCOUNT_BYTES)
            require(bytes.size == HEADER_BYTES + envelopeLength + accountLength) { "Request carrier length differs" }
            return IrohaPeerWalletRequestV1(bytes.copyOfRange(HEADER_BYTES, HEADER_BYTES + envelopeLength),
                bytes.copyOfRange(HEADER_BYTES + envelopeLength, bytes.size))
        }
        private fun readLength(bytes: ByteArray, offset: Int, maximum: Int): Int {
            var value = 0L
            for (i in 0..3) value = (value shl 8) or (bytes[offset + i].toLong() and 255)
            require(value in 1..maximum.toLong()) { "Request carrier field exceeds its bound" }
            return value.toInt()
        }
        private fun writeLength(bytes: ByteArray, offset: Int, value: Int) {
            for (i in 0..3) bytes[offset + i] = (value ushr (24 - i * 8)).toByte()
        }
    }
}

/** Exact schema-3 receiver originals. Only the installed Native wallet authenticates them. */
class IrohaPeerSpartanOpenRequestV3(canonicalPayload: ByteArray) {
    private val original: ByteArray
    private val envelopeEnd: Int
    private val certificatesEnd: Int

    init {
        require(canonicalPayload.size in 1_267..MAXIMUM_BYTES)
        original = canonicalPayload.copyOf()
        require(original.copyOfRange(0, 8).contentEquals("KWRQAC1\u0000".toByteArray(Charsets.US_ASCII)))
        val envelope = length(8)
        val account = length(12)
        require((envelope == 1_250L || envelope == 1_677L) && account in 1L..4_096L &&
            16L + envelope + account == original.size.toLong())
        val hasFee = envelope == 1_677L
        envelopeEnd = 16 + envelope.toInt()
        certificatesEnd = envelopeEnd - if (hasFee) 255 else 0
        val r = request()
        val c = receiverCredential()
        fun nonzero(from: Int, until: Int) = (from until until).any { r[it] != 0.toByte() }
        require(r[0] == 1.toByte() && r[1] == 0.toByte() && !nonzero(66, 130) && !nonzero(194, 210) &&
            nonzero(242, 258) && nonzero(386, 418) && nonzero(418, 482) && nonzero(258, 290) == hasFee &&
            (hasFee || !nonzero(290, 306)) && (0 until 66).all { c[it] == r[it] } &&
            (66 until 130).all { c[it] == r[it + 64] } && c[130] == 4.toByte())
        val roles = mutableSetOf<Int>()
        for (start in 1094 until certificatesEnd step 172) {
            require((0 until 34).all { original[start + it] == r[it] } && original[start + 35] == 4.toByte() &&
                roles.add(original[start + 34].toInt()))
        }
        require(roles == (if (hasFee) setOf(1, 3) else setOf(1)) &&
            (!hasFee || (0 until 66).all { original[certificatesEnd + it] == r[it] }))
    }
    fun request(): ByteArray = original.copyOfRange(16, 498)
    fun receiverCredential(): ByteArray = original.copyOfRange(498, 1094)
    fun certificates(): ByteArray = original.copyOfRange(1094, certificatesEnd)
    fun feeSchedule(): ByteArray = original.copyOfRange(certificatesEnd, envelopeEnd)
    fun destinationAccountOriginal(): ByteArray = original.copyOfRange(envelopeEnd, original.size)
    private fun length(offset: Int): Long = (offset until offset + 4).fold(0L) { size, i ->
        (size shl 8) or (original[i].toLong() and 255)
    }
    override fun toString() = "IrohaPeerSpartanOpenRequestV3(originals=[REDACTED])"
    companion object {
        const val MAXIMUM_BYTES = 16 + 1_677 + IrohaPeerWalletRequestV1.MAXIMUM_ACCOUNT_BYTES
    }
}
